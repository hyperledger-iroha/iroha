// Production retention of the finalized provider-ingest archive.
//
// The archive stops accepting records at its entry or byte bound, and a refused capture holds the
// committed decision, so reaching a bound stops this node committing. One supervised worker calls
// `reconcile_retention_once` to stay below the bounds: it runs off the commit path, does a bounded
// amount of work per call, and needs the deployment-owned sealed authority to approve every
// checkpoint. Without an authority the archive keeps manual retention, and the worker can only
// warn through `capacity`.
//
// Commit capture never waits on any of this beyond one short in-memory swap:
//
// 1. Copy the bounded prefix under a brief reader.
// 2. With no lock held, check the prefix against what this process already proved, and derive
//    the checkpoint. Every input is immutable, because captures only append above the fence.
// 3. With no lock held, advance the sealed authority and read it back.
// 4. With no lock held, publish the checkpoint file.
// 5. Under the writer, swap the base into the index and rebase the commit's retained walk.
// 6. With no lock held, unlink the pruned files. Their capacity is freed only after this, so a
//    reopen, which counts files before recovery can remove any, never finds too many.
//
// A crash after step 3 reopens through `recover_approved_retention`, which finishes the approved
// compaction whether or not captures have landed since. A failure in a running process leaves
// the authority ahead, and the next step finishes that same approval first.

/// Most anchor records one compaction prunes.
const RETENTION_MAX_PREFIX_ENTRIES: usize = 1_024;
/// Most canonical record bytes one compaction prunes. A single larger record still goes through,
/// so an oversized record cannot stop retention.
const RETENTION_MAX_PREFIX_BYTES: u64 = 64 * 1024 * 1024;
// Retention starts when either bound is three quarters used. The fractions leave a quarter of the
// bound for the worker to catch up, and keep half of it for anything still reading history. They
// scale with the configured bounds, so an operator sets retention depth by setting those.
const fn retention_high_watermark(maximum: u64) -> u64 {
    maximum - maximum / 4
}

const fn retention_low_watermark(maximum: u64) -> u64 {
    maximum / 2
}

/// Point-in-time use of the archive's two capacity bounds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[must_use]
pub struct ProviderIngestFinalizedArchiveCapacityV1 {
    retained_entries: usize,
    maximum_entries: usize,
    total_bytes: u64,
    maximum_total_bytes: u64,
}

impl ProviderIngestFinalizedArchiveCapacityV1 {
    /// Return the retained record and virtual-base count that capture checks against its bound.
    #[must_use]
    pub const fn retained_entries(&self) -> usize {
        self.retained_entries
    }
    /// Return the configured entry bound.
    #[must_use]
    pub const fn maximum_entries(&self) -> usize {
        self.maximum_entries
    }
    /// Return the retained canonical bytes that capture checks against its bound.
    #[must_use]
    pub const fn total_bytes(&self) -> u64 {
        self.total_bytes
    }
    /// Return the configured byte bound.
    #[must_use]
    pub const fn maximum_total_bytes(&self) -> u64 {
        self.maximum_total_bytes
    }
    /// Return whether retention is due: either bound is at least three quarters used.
    #[must_use]
    pub fn at_high_watermark(&self) -> bool {
        let retained = u64::try_from(self.retained_entries).unwrap_or(u64::MAX);
        let maximum = u64::try_from(self.maximum_entries).unwrap_or(u64::MAX);
        retained >= retention_high_watermark(maximum)
            || self.total_bytes >= retention_high_watermark(self.maximum_total_bytes)
    }
}

/// What one bounded retention step found or did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[must_use]
#[allow(
    variant_size_differences,
    reason = "Returned once per step and consumed at once, so boxing would only add an allocation."
)]
pub enum ProviderIngestFinalizedArchiveRetentionStepV1 {
    /// Neither bound needs retention, and no authority was contacted.
    Idle,
    /// Retention is due, but the oldest pinned height stops the floor moving further.
    Pinned {
        /// Oldest retained height that something still reads.
        pinned_height: u64,
        /// Current use of the entry bound, so the caller can report how close the bound is.
        retained_entries: usize,
    },
    /// Retention is due, but no fresh walk has proved this archive's anchors yet. Qualification
    /// does that, so this clears on its own.
    AwaitingProof,
    /// One bounded prefix was approved, installed and unlinked.
    Compacted(ProviderIngestFinalizedArchiveCompactionOutcomeV1),
}

// What the single running compaction has yet to finish. This mutex serializes compactions, and
// only their own worker waits on it: nothing on the commit path takes it.
#[derive(Debug, Default)]
struct ArchiveRetentionProgressV1 {
    // Set before sealed approval and cleared only after publication and cleanup or an
    // authoritative readback confirms no approval ahead of the installed base.
    approval_may_be_pending: bool,
    // A cycle runs from the high watermark down to the low one, then ends.
    in_cycle: bool,
    // An installed compaction's files not yet unlinked. Popped oldest first, so an interruption
    // leaves a contiguous remainder, as the startup cleanup expects.
    obsolete_records: Vec<((NetworkId, u64), ArchiveRecordEntryV1)>,
    obsolete_checkpoint: Option<ArchiveVirtualBaseV1>,
}

/// Work one retention step may do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RetentionLimitsV1 {
    max_prefix_entries: usize,
    max_prefix_bytes: u64,
}

impl RetentionLimitsV1 {
    const PRODUCTION: Self = Self {
        max_prefix_entries: RETENTION_MAX_PREFIX_ENTRIES,
        max_prefix_bytes: RETENTION_MAX_PREFIX_BYTES,
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(
    variant_size_differences,
    reason = "Local to one retention step, so boxing would only add an allocation."
)]
enum RetentionPlanV1 {
    Idle,
    Pinned {
        pinned_height: u64,
    },
    Compact {
        target: ProviderIngestFinalizedArchiveKeyV1,
        // This compaction reaches the low watermark. Ending the cycle here keeps steady commits
        // from turning every later step into another small compaction and authority write.
        completes_cycle: bool,
    },
}

/// Choose the next retention floor from counts alone, so the choice is reproducible and takes
/// one short reader.
///
/// Compaction keeps the tip record, so a commit's retained walk stays valid, and a pinned height
/// stays resolvable as the floor itself. A pin below the current floor is ignored: that history
/// is already gone, so holding the floor there would protect nothing.
fn plan_retention(
    index: &ArchiveIndexV1,
    network_id: &NetworkId,
    bounds: ProviderIngestFinalizedArchiveBoundsV1,
    continuing: bool,
    pinned_heights: &[u64],
    limits: RetentionLimitsV1,
) -> RetentionPlanV1 {
    let retained_entries = u64::try_from(retained_archive_entries(index)).unwrap_or(u64::MAX);
    let maximum_entries = u64::try_from(bounds.max_archive_entries()).unwrap_or(u64::MAX);
    let low_entries = retention_low_watermark(maximum_entries);
    let low_bytes = retention_low_watermark(bounds.max_total_bytes());
    let due = retained_entries >= retention_high_watermark(maximum_entries)
        || index.total_bytes >= retention_high_watermark(bounds.max_total_bytes());
    let above_low = retained_entries > low_entries || index.total_bytes > low_bytes;
    // A base and the tip record must fit below the low watermark, or a cycle could never end.
    if low_entries < 2 || !(due || (continuing && above_low)) {
        return RetentionPlanV1::Idle;
    }
    let Ok(Some(floor)) = activation_floor_from_index(index, network_id) else {
        return RetentionPlanV1::Idle;
    };
    let Some(tip_height) = index.tip_height(network_id) else {
        return RetentionPlanV1::Idle;
    };
    let pin = pinned_heights
        .iter()
        .copied()
        .filter(|height| *height >= floor.height)
        .min();
    // The first compaction adds a virtual base, and capture counts it as an entry.
    let mut remaining_entries =
        retained_entries.saturating_add(u64::from(!index.virtual_bases.contains_key(network_id)));
    let mut remaining_bytes = index.total_bytes;
    let mut pruned_entries = 0_usize;
    let mut pruned_bytes = 0_u64;
    let mut target = None;
    for (_, entry) in index.by_height.range((
        std::ops::Bound::Included((*network_id, 0)),
        std::ops::Bound::Included((*network_id, u64::MAX)),
    )) {
        let key = entry.record.material.key;
        if (remaining_entries <= low_entries && remaining_bytes <= low_bytes)
            || key.height >= tip_height
            || pruned_entries >= limits.max_prefix_entries
            || pruned_bytes >= limits.max_prefix_bytes
        {
            break;
        }
        if let Some(pinned_height) = pin
            && key.height > pinned_height
        {
            return target.map_or(RetentionPlanV1::Pinned { pinned_height }, |target| {
                RetentionPlanV1::Compact {
                    target,
                    completes_cycle: false,
                }
            });
        }
        target = Some(key);
        remaining_entries = remaining_entries.saturating_sub(1);
        remaining_bytes = remaining_bytes.saturating_sub(entry.canonical_bytes);
        pruned_entries = pruned_entries.saturating_add(1);
        pruned_bytes = pruned_bytes.saturating_add(entry.canonical_bytes);
    }
    let completes_cycle = remaining_entries <= low_entries && remaining_bytes <= low_bytes;
    target.map_or(RetentionPlanV1::Idle, |target| RetentionPlanV1::Compact {
        target,
        completes_cycle,
    })
}

impl ProviderIngestFinalizedArchiveV1 {
    /// Return the current use of this archive's capacity bounds.
    ///
    /// # Errors
    ///
    /// Returns an integrity error if the archive lock is poisoned.
    pub fn capacity(
        &self,
    ) -> Result<ProviderIngestFinalizedArchiveCapacityV1, ProviderIngestFinalizedArchiveErrorV1>
    {
        let index = self.read_index()?;
        Ok(ProviderIngestFinalizedArchiveCapacityV1 {
            retained_entries: retained_archive_entries(&index),
            maximum_entries: self.bounds.max_archive_entries(),
            total_bytes: index.total_bytes,
            maximum_total_bytes: self.bounds.max_total_bytes(),
        })
    }

    /// Run one bounded retention step, keeping this archive below its capacity bounds.
    ///
    /// `pinned_heights` lists retained anchors that something still reads. The floor never rises
    /// above the oldest one at or above the current floor, so a pinned height stays resolvable.
    ///
    /// # Errors
    ///
    /// Returns a typed archive, Kura, or authority failure. Nothing is unlinked before its
    /// checkpoint is approved and published, so the caller retries with a later step.
    pub fn reconcile_retention_once(
        &self,
        state_ro: &impl StateReadOnly,
        kura: &Kura,
        binding: &ProviderIngestFinalizedArchiveRetentionAuthorityBindingV1,
        authority: &dyn ProviderIngestFinalizedArchiveRetentionAuthorityV1,
        pinned_heights: &[u64],
    ) -> Result<ProviderIngestFinalizedArchiveRetentionStepV1, ProviderIngestFinalizedArchiveErrorV1>
    {
        self.reconcile_retention_with_limits(
            state_ro,
            kura,
            binding,
            authority,
            pinned_heights,
            RetentionLimitsV1::PRODUCTION,
        )
    }

    /// Reconcile a held startup commit after freeing only history no pending job needs.
    ///
    /// The caller must retain exclusive ownership of the authenticated outbox while
    /// supplying `pending_admission_keys`; no job may be admitted until this returns.
    ///
    /// # Errors
    /// Returns the original capacity refusal if retained job anchors prevent progress,
    /// or the exact qualification, authority, or reconciliation failure.
    pub fn reconcile_certified_state_tip_with_retention(
        &self,
        state_ro: &impl StateReadOnly,
        kura: &Kura,
        binding: &ProviderIngestFinalizedArchiveRetentionAuthorityBindingV1,
        authority: &dyn ProviderIngestFinalizedArchiveRetentionAuthorityV1,
        pending_admission_keys: &[ProviderIngestFinalizedArchiveKeyV1],
    ) -> Result<
        ProviderIngestFinalizedArchiveReconcileOutcomeV1,
        ProviderIngestFinalizedArchiveErrorV1,
    > {
        let pending_admission_heights = pending_admission_keys
            .iter()
            .map(|key| {
                let exact = self.resolve_exact_key(&key.network_id, key.height, key.block_hash)?;
                if exact != *key {
                    return Err(ProviderIngestFinalizedArchiveErrorV1::InvalidKey {
                        reason: "pending job admission key differs from the exact archived anchor",
                    });
                }
                Ok(key.height)
            })
            .collect::<Result<Vec<_>, ProviderIngestFinalizedArchiveErrorV1>>()?;
        let mut refusal = match self.reconcile_certified_state_tip(state_ro, kura) {
            Err(
                error @ (ProviderIngestFinalizedArchiveErrorV1::ArchiveCapacityExceeded { .. }
                | ProviderIngestFinalizedArchiveErrorV1::ArchiveBytesExceeded { .. }),
            ) => error,
            other => return other,
        };
        // Retention reads the authenticated coverage recorded by this qualification.
        let _coverage = self.qualify_against_certified_tip(state_ro, kura, u64::MAX)?;
        loop {
            let step = self.reconcile_retention_with_limits(
                state_ro,
                kura,
                binding,
                authority,
                &pending_admission_heights,
                RetentionLimitsV1 {
                    max_prefix_entries: 16,
                    max_prefix_bytes: RETENTION_MAX_PREFIX_BYTES,
                },
            )?;
            if !matches!(
                step,
                ProviderIngestFinalizedArchiveRetentionStepV1::Compacted(_)
            ) {
                return Err(refusal);
            }
            refusal = match self.reconcile_certified_state_tip(state_ro, kura) {
                Err(
                    error @ (ProviderIngestFinalizedArchiveErrorV1::ArchiveCapacityExceeded {
                        ..
                    }
                    | ProviderIngestFinalizedArchiveErrorV1::ArchiveBytesExceeded { .. }),
                ) => error,
                other => return other,
            };
        }
    }

    fn reconcile_retention_with_limits(
        &self,
        state_ro: &impl StateReadOnly,
        kura: &Kura,
        binding: &ProviderIngestFinalizedArchiveRetentionAuthorityBindingV1,
        authority: &dyn ProviderIngestFinalizedArchiveRetentionAuthorityV1,
        pinned_heights: &[u64],
        limits: RetentionLimitsV1,
    ) -> Result<ProviderIngestFinalizedArchiveRetentionStepV1, ProviderIngestFinalizedArchiveErrorV1>
    {
        let network_id = state_ro.network_id();
        let mut progress = self.retention.lock();
        // A second checkpoint before the first one's cleanup would leave three on disk, which
        // the next open must refuse. So nothing new starts until this is done.
        self.finish_pending_retention_cleanup(&mut progress)?;
        // Identity and coverage are read under one reader, because captures extend coverage
        // under the writer.
        let (plan, retained_entries, base_digest, proved_through) = {
            let index = self.read_index()?;
            (
                plan_retention(
                    &index,
                    network_id,
                    self.bounds,
                    progress.in_cycle,
                    pinned_heights,
                    limits,
                ),
                retained_archive_entries(&index),
                index
                    .virtual_bases
                    .get(network_id)
                    .map(|base| base.checkpoint.checkpoint_digest),
                self.authentication
                    .covered_through(&index.content_identity(network_id)),
            )
        };
        // A failed publication may already have irreversibly advanced the authority. Check
        // it before an Idle/Pinned return; the old plan cannot cancel that approval.
        let interrupted_approval = if progress.approval_may_be_pending {
            assert_retention_authority_identity(binding, authority)?;
            let approved = load_retention_approval(binding, authority, network_id)?
                .filter(|record| Some(record.proposal().checkpoint_digest()) != base_digest);
            if approved.is_none() {
                progress.approval_may_be_pending = false;
            }
            approved
        } else {
            None
        };
        let (planned, completes_cycle) = if let Some(record) = interrupted_approval.as_ref() {
            (record.proposal().fence().key, false)
        } else {
            match plan {
                RetentionPlanV1::Idle => {
                    progress.in_cycle = false;
                    return Ok(ProviderIngestFinalizedArchiveRetentionStepV1::Idle);
                }
                RetentionPlanV1::Pinned { pinned_height } => {
                    return Ok(ProviderIngestFinalizedArchiveRetentionStepV1::Pinned {
                        pinned_height,
                        retained_entries,
                    });
                }
                RetentionPlanV1::Compact {
                    target,
                    completes_cycle,
                } => (target, completes_cycle),
            }
        };
        // Pruning relies on what a fresh walk and later captures proved, and retries once
        // qualification has done its walk.
        let Some(proved_through) = proved_through.filter(|through| *through >= planned.height)
        else {
            return Ok(ProviderIngestFinalizedArchiveRetentionStepV1::AwaitingProof);
        };
        progress.in_cycle = true;
        assert_retention_authority_identity(binding, authority)?;
        // An approval ahead of the installed base names the only compaction the authority will
        // accept next, whatever this step would have planned.
        let approved = match interrupted_approval {
            Some(record) => Some(record),
            None => load_retention_approval(binding, authority, network_id)?
                .filter(|record| Some(record.proposal().checkpoint_digest()) != base_digest),
        };
        progress.approval_may_be_pending |= approved.is_some();
        let target = approved
            .as_ref()
            .map_or(planned, |record| record.proposal().fence().key);
        if approved.is_some() && pinned_heights.iter().any(|height| *height < target.height) {
            return Err(
                ProviderIngestFinalizedArchiveErrorV1::InvalidRetentionFence {
                    reason: "approved retention would retire a currently required admission anchor",
                },
            );
        }
        if proved_through < target.height {
            return Ok(ProviderIngestFinalizedArchiveRetentionStepV1::AwaitingProof);
        }
        let snapshot = self.retention_snapshot(&target)?;
        if snapshot.by_height.is_empty() {
            // An approval at or below the installed floor means the authority lost its lineage.
            return Err(if approved.is_some() {
                ProviderIngestFinalizedArchiveErrorV1::RetentionAuthorityRollback
            } else {
                ProviderIngestFinalizedArchiveErrorV1::UnknownExactAnchor {
                    network_id: *network_id,
                    height: target.height,
                }
            });
        }
        let certified =
            CertifiedArchiveView::new(state_ro, kura).map_err(certified_authentication_error)?;
        let certified_block_id =
            authenticate_proved_retention_anchors(&snapshot, &target, network_id, &certified)?;
        certified
            .verify_unchanged_through(target.height)
            .map_err(certified_authentication_error)?;
        drop(certified);
        let fence = match approved.as_ref() {
            Some(record) => *record.proposal().fence(),
            None => ProviderIngestFinalizedArchiveRetentionFenceV1::try_new(
                target,
                certified_block_id,
                snapshot.generation,
            )?,
        };
        if fence.certified_block_id != certified_block_id {
            return Err(
                ProviderIngestFinalizedArchiveErrorV1::InvalidRetentionFence {
                    reason: "retention fence substituted the certified block identity",
                },
            );
        }
        let prepared = prepare_retention_from_snapshot(&snapshot, &fence, self.bounds)?;
        let proposal = compaction_proposal(&prepared, &fence)?;
        // Finishing an older approval says nothing about this step's plan.
        let finishes_cycle = completes_cycle && approved.is_none();
        if approved.is_some_and(|record| *record.proposal() != proposal) {
            return Err(ProviderIngestFinalizedArchiveErrorV1::RetentionProposalMismatch);
        }
        drop(snapshot);
        progress.approval_may_be_pending = true;
        let outcome = self.approve_and_publish_compaction(
            &mut progress,
            prepared,
            &proposal,
            binding,
            authority,
        )?;
        progress.approval_may_be_pending = false;
        if finishes_cycle {
            progress.in_cycle = false;
        }
        Ok(ProviderIngestFinalizedArchiveRetentionStepV1::Compacted(
            outcome,
        ))
    }

    // Copy one retention prefix under a brief reader. Nothing above the fence comes along, so
    // the file, chain and digest work that follows holds no index lock, and a commit's capture
    // cannot be refused on its account.
    fn retention_snapshot(
        &self,
        key: &ProviderIngestFinalizedArchiveKeyV1,
    ) -> Result<ArchiveIndexV1, ProviderIngestFinalizedArchiveErrorV1> {
        self.verify_storage_boundaries()?;
        let index = self.read_index()?;
        let network_id = key.network_id;
        Ok(ArchiveIndexV1 {
            by_height: index
                .by_height
                .range((
                    std::ops::Bound::Included((network_id, 0)),
                    std::ops::Bound::Included((network_id, key.height)),
                ))
                .map(|(subject, entry)| (*subject, entry.clone()))
                .collect(),
            virtual_bases: index
                .virtual_bases
                .get(&network_id)
                .map(|base| (network_id, base.clone()))
                .into_iter()
                .collect(),
            total_bytes: index.total_bytes,
            generation: index.generation,
            ..ArchiveIndexV1::default()
        })
    }

    // Advance the sealed authority, then publish and install. Recovery at the next open finishes
    // whatever a crash interrupts from here on.
    fn approve_and_publish_compaction(
        &self,
        progress: &mut ArchiveRetentionProgressV1,
        prepared: PreparedArchiveCompactionV1,
        proposal: &ProviderIngestFinalizedArchiveCompactionProposalV1,
        binding: &ProviderIngestFinalizedArchiveRetentionAuthorityBindingV1,
        authority: &dyn ProviderIngestFinalizedArchiveRetentionAuthorityV1,
    ) -> Result<
        ProviderIngestFinalizedArchiveCompactionOutcomeV1,
        ProviderIngestFinalizedArchiveErrorV1,
    > {
        let fence = proposal.fence();
        let network_id = &fence.key.network_id;
        let current = load_retention_approval(binding, authority, network_id)?;
        let expected_checkpoint = prepared
            .previous_base
            .as_ref()
            .map(|base| base.checkpoint.checkpoint_digest);
        let next = if let Some(current) = current
            .as_ref()
            .filter(|record| record.proposal() == proposal)
        {
            validate_approval_for_prepared(
                current,
                binding,
                &prepared,
                proposal,
                expected_checkpoint,
            )?;
            current.clone()
        } else {
            if let Some(current) = &current {
                validate_retention_approval_record(current, binding, network_id)?;
            }
            validate_retention_authority_predecessor(current.as_ref(), expected_checkpoint, fence)?;
            let sequence = current.as_ref().map_or(Ok(1), |record| {
                record.sequence().checked_add(1).ok_or(
                    ProviderIngestFinalizedArchiveErrorV1::InvalidRetentionApproval {
                        reason: "approval sequence overflowed",
                    },
                )
            })?;
            let next = ProviderIngestFinalizedArchiveRetentionApprovalRecordV1::try_new(
                sequence,
                binding.qualification(),
                proposal.clone(),
                current
                    .as_ref()
                    .map(ProviderIngestFinalizedArchiveRetentionApprovalRecordV1::revision),
                expected_checkpoint,
            )?;
            compare_and_read_back_retention_approval(
                binding,
                authority,
                network_id,
                current.as_ref(),
                &next,
            )?;
            next
        };
        let readback = load_retention_approval(binding, authority, network_id)?;
        if readback.as_ref() != Some(&next) {
            return Err(ProviderIngestFinalizedArchiveErrorV1::RetentionAuthorityEquivocation);
        }
        self.publish_approved_compaction(progress, prepared)
    }

    fn publish_approved_compaction(
        &self,
        progress: &mut ArchiveRetentionProgressV1,
        prepared: PreparedArchiveCompactionV1,
    ) -> Result<
        ProviderIngestFinalizedArchiveCompactionOutcomeV1,
        ProviderIngestFinalizedArchiveErrorV1,
    > {
        let checkpoint_path = self
            .checkpoints
            .join(checkpoint_file_name(prepared.checkpoint.checkpoint_digest));
        self.verify_storage_boundaries()?;
        publish_immutable_bytes(
            #[cfg(windows)]
            &self.native,
            &self.checkpoints,
            self.checkpoints_identity,
            &checkpoint_path,
            &prepared.canonical_bytes,
        )?;
        if load_checkpoint_at(&checkpoint_path, self.bounds)? != prepared.checkpoint {
            return Err(ProviderIngestFinalizedArchiveErrorV1::CheckpointDigestMismatch);
        }
        let outcome = self.install_published_compaction(&prepared, checkpoint_path)?;
        let PreparedArchiveCompactionV1 {
            mut obsolete,
            previous_base,
            ..
        } = prepared;
        obsolete.reverse();
        progress.obsolete_records = obsolete;
        progress.obsolete_checkpoint = previous_base;
        self.finish_pending_retention_cleanup(progress)?;
        Ok(outcome)
    }

    // The only part of a compaction under the writer: in-memory bookkeeping over a bounded
    // prefix, no file or authority work, and nothing that can panic and poison the index.
    fn install_published_compaction(
        &self,
        prepared: &PreparedArchiveCompactionV1,
        checkpoint_path: PathBuf,
    ) -> Result<
        ProviderIngestFinalizedArchiveCompactionOutcomeV1,
        ProviderIngestFinalizedArchiveErrorV1,
    > {
        let checkpoint = &prepared.checkpoint;
        let floor = checkpoint.material.retention_floor;
        let network_id = floor.network_id;
        let prior_digest = prepared
            .previous_base
            .as_ref()
            .map(|base| base.checkpoint.checkpoint_digest);
        let checkpoint_bytes = bounded_bytes_len(&prepared.canonical_bytes);
        let mut index = self.write_index()?;
        let replaced_base = index
            .virtual_bases
            .get(&network_id)
            .map(|base| (base.checkpoint.checkpoint_digest, base.canonical_bytes));
        if replaced_base.map(|(digest, _)| digest) != prior_digest {
            return Err(
                ProviderIngestFinalizedArchiveErrorV1::QualificationBoundaryChanged {
                    boundary: "archive",
                },
            );
        }
        let mut removed_bytes = replaced_base.map_or(0, |(_, bytes)| bytes);
        for (subject, entry) in &prepared.obsolete {
            // Captures only append above the fence and this worker is the only compactor, so a
            // difference here means the archive is not what this compaction was derived from.
            let current = index
                .by_height
                .get(subject)
                .filter(|current| {
                    current.record.record_digest == entry.record.record_digest
                        && current.path == entry.path
                })
                .ok_or(ProviderIngestFinalizedArchiveErrorV1::RetentionProposalMismatch)?;
            removed_bytes = removed_bytes.checked_add(current.canonical_bytes).ok_or(
                ProviderIngestFinalizedArchiveErrorV1::ArchiveBytesExceeded {
                    observed: u64::MAX,
                    maximum: self.bounds.max_total_bytes(),
                },
            )?;
        }
        let total_bytes = index
            .total_bytes
            .checked_sub(removed_bytes)
            .and_then(|bytes| bytes.checked_add(checkpoint_bytes))
            .ok_or(
                ProviderIngestFinalizedArchiveErrorV1::ArchiveBytesExceeded {
                    observed: u64::MAX,
                    maximum: self.bounds.max_total_bytes(),
                },
            )?;
        // Nothing below can fail. The capacity stays held until `finish_pending_retention_cleanup`
        // has removed the files.
        index.pending_unlink_entries = prepared.obsolete.len();
        index.pending_unlink_bytes = removed_bytes;
        let before = index.content_identity(&network_id);
        index.virtual_bases.insert(
            network_id,
            ArchiveVirtualBaseV1 {
                checkpoint: checkpoint.clone(),
                path: checkpoint_path,
                canonical_bytes: checkpoint_bytes,
            },
        );
        for (subject, _) in &prepared.obsolete {
            index.by_height.remove(subject);
        }
        index.total_bytes = total_bytes;
        // The checkpoint carries the same history the walk derived, and the walk's state is that
        // of the tip above it. So the walk stands over the new base, and the next commit keeps
        // validating only its own successor.
        let walk_stands = index
            .tip_walks
            .get(&network_id)
            .is_some_and(|walk| walk.base_digest == prior_digest);
        if walk_stands {
            if let Some(walk) = index.tip_walks.get_mut(&network_id) {
                walk.base_digest = Some(checkpoint.checkpoint_digest);
            }
        } else {
            index.tip_walks.remove(&network_id);
        }
        let after = index.content_identity(&network_id);
        self.authentication.note_compaction(&before, after);
        Ok(ProviderIngestFinalizedArchiveCompactionOutcomeV1 {
            retention_floor: floor,
            checkpoint_digest: checkpoint.checkpoint_digest,
            pruned_entries: checkpoint.material.pruned_entries,
            pruned_bytes: checkpoint.material.pruned_bytes,
            generation: index.generation,
        })
    }

    // Unlink what an installed compaction left behind. Readers already cannot reach these files,
    // and a crash leaves them to the startup cleanup. A failure stays pending for the next step.
    fn finish_pending_retention_cleanup(
        &self,
        progress: &mut ArchiveRetentionProgressV1,
    ) -> Result<(), ProviderIngestFinalizedArchiveErrorV1> {
        if progress.obsolete_records.is_empty() && progress.obsolete_checkpoint.is_none() {
            return Ok(());
        }
        while let Some((_, entry)) = progress.obsolete_records.last() {
            already_unlinked_is_done(unlink_verified_archive_file(
                #[cfg(windows)]
                &self.native,
                &self.records,
                self.records_identity,
                &entry.path,
                #[cfg(windows)]
                &norito::to_bytes(&entry.record)
                    .map_err(ProviderIngestFinalizedArchiveErrorV1::Encode)?,
            ))?;
            progress.obsolete_records.pop();
        }
        if let Some(previous) = &progress.obsolete_checkpoint {
            already_unlinked_is_done(unlink_verified_archive_file(
                #[cfg(windows)]
                &self.native,
                &self.checkpoints,
                self.checkpoints_identity,
                &previous.path,
                #[cfg(windows)]
                &norito::to_bytes(&previous.checkpoint)
                    .map_err(ProviderIngestFinalizedArchiveErrorV1::Encode)?,
            ))?;
            progress.obsolete_checkpoint = None;
        }
        {
            let mut index = self.write_index()?;
            index.pending_unlink_entries = 0;
            index.pending_unlink_bytes = 0;
        }
        self.verify_storage_boundaries()
    }
}

// An unlink that failed only at its directory sync has already removed the file, so a retry finds
// nothing to do. The caller's closing boundary check still covers the namespace.
fn already_unlinked_is_done(
    result: Result<(), ProviderIngestFinalizedArchiveErrorV1>,
) -> Result<(), ProviderIngestFinalizedArchiveErrorV1> {
    match result {
        Err(
            ProviderIngestFinalizedArchiveErrorV1::Read { source, .. }
            | ProviderIngestFinalizedArchiveErrorV1::Write { source, .. },
        ) if source.kind() == io::ErrorKind::NotFound => Ok(()),
        other => other,
    }
}

// The checkpoint a fence names, derived from a prefix snapshot alone. Its bytes depend only on
// the base, the prefix and the fence, so a restart reconstructs exactly what was approved.
fn prepare_retention_from_snapshot(
    snapshot: &ArchiveIndexV1,
    fence: &ProviderIngestFinalizedArchiveRetentionFenceV1,
    bounds: ProviderIngestFinalizedArchiveBoundsV1,
) -> Result<PreparedArchiveCompactionV1, ProviderIngestFinalizedArchiveErrorV1> {
    if fence.expected_archive_generation > snapshot.generation {
        return Err(
            ProviderIngestFinalizedArchiveErrorV1::RetentionFenceGenerationMismatch {
                expected: fence.expected_archive_generation,
                observed: snapshot.generation,
            },
        );
    }
    validate_index_coverage(snapshot, bounds)?;
    let prepared = prepare_archive_compaction(snapshot, fence, bounds)?;
    validate_prepared_compaction_capacity(snapshot, &prepared, bounds)?;
    Ok(prepared)
}

// Check a prefix against what this process already proved. A step only prunes anchors covered by
// proof, so it needs no commit certificates: each hash must still be in the durable journal, and
// the target's block id comes from a read down from the nearest verified height. Returns that id,
// which the next open's walk checks again.
fn authenticate_proved_retention_anchors<V: StateReadOnly + ?Sized>(
    snapshot: &ArchiveIndexV1,
    target: &ProviderIngestFinalizedArchiveKeyV1,
    network_id: &NetworkId,
    certified: &CertifiedArchiveView<'_, V>,
) -> Result<[u8; 32], ProviderIngestFinalizedArchiveErrorV1> {
    if let Some(base) = snapshot.virtual_bases.get(network_id) {
        let floor = &base.checkpoint.material.retention_floor;
        certified
            .require_journal_anchor(floor.height, &floor.block_hash)
            .map_err(certified_authentication_error)?;
    }
    for entry in snapshot.by_height.values() {
        let key = &entry.record.material.key;
        certified
            .require_journal_anchor(key.height, &key.block_hash)
            .map_err(certified_authentication_error)?;
    }
    let terminal = certified
        .committed_anchor(target.height)
        .map_err(certified_authentication_error)?;
    if *terminal.block_hash().as_ref() != target.block_hash
        || terminal.block_time_ms() != target.finalized_at_unix_ms
    {
        return Err(
            ProviderIngestFinalizedArchiveErrorV1::ArchiveKuraAnchorMismatch {
                network_id: target.network_id,
                height: target.height,
                reason: "archive identity or timestamp differs from the committed block",
            },
        );
    }
    Ok(*terminal.id().0.as_ref())
}
