// Exact archive predecessor custody before execution and original capture after it.

struct CapturedReputationCandidate {
    next_state: ReputationReconstructionStateV1,
    authority_policy_history: Vec<ReputationJournalAuthorityPolicyRecordV1>,
}

/// Lifetime-free custody of one exact intended candidate and archive predecessor.
/// It reserves no consensus authority and retains no physical index or State guard.
pub(crate) struct ReputationCandidateCapture {
    archive: Arc<ReputationFinalizedArchive>,
    kura: Arc<Kura>,
    key: ReputationFinalizedArchiveKeyV1,
    finalized_at_unix_ms: u64,
    generation: u64,
    previous: Option<ReputationReconstructionStateV1>,
    capture_attempted: bool,
    captured: Option<CapturedReputationCandidate>,
    plan: Option<Option<PreparedReputationState>>,
    // Logical custody outlives all original projections and prepared material.
    reservation: ArchiveCaptureReservation,
}

impl std::fmt::Debug for ReputationCandidateCapture {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ReputationCandidateCapture")
            .field("key", &self.key)
            .field("captured", &self.captured.is_some())
            .field("prepared", &self.plan.is_some())
            .finish_non_exhaustive()
    }
}

impl ReputationFinalizedArchive {
    /// Hold an actual reader across a deterministic candidate-preparation probe.
    #[cfg(test)]
    pub(crate) fn with_index_reader_for_test<R>(&self, action: impl FnOnce() -> R) -> R {
        let _reader = self.read_index().expect("test archive reader");
        action()
    }

    /// Reserve the authenticated original archive cut before executing this candidate.
    /// Busy readers and writers return their actual release observation; no State is read.
    pub(crate) fn try_reserve_candidate(
        self: &Arc<Self>,
        key: ReputationFinalizedArchiveKeyV1,
        finalized_at_unix_ms: u64,
        kura: &Arc<Kura>,
    ) -> Result<ReputationCandidateCapture, ReputationFinalizedArchiveError> {
        key.validate()?;
        if finalized_at_unix_ms == 0 || finalized_at_unix_ms == u64::MAX {
            return Err(ReputationFinalizedArchiveError::FinalityAuthentication {
                reason: "candidate header has no valid deterministic timestamp",
            });
        }
        let index = self
            .index
            .try_write()
            .map_err(|error| self.index_lock_error(error))?;
        if index.requires_reopen {
            return Err(ReputationFinalizedArchiveError::ArchiveUnavailable {
                reason: CHECKPOINT_PUBLICATION_REOPEN_REQUIRED_REASON,
            });
        }
        self.capture_gate
            .ensure_unreserved()
            .map_err(|wait| ReputationFinalizedArchiveError::CaptureReserved { wait })?;
        self.verify_storage_boundaries()?;
        require_contiguous_capture_key_in_index(&index, &key)?;
        let reservation = self
            .capture_gate
            .try_reserve()
            .map_err(|wait| ReputationFinalizedArchiveError::CaptureReserved { wait })?;
        let previous = key
            .height
            .checked_sub(1)
            .map(|height| {
                self.latest_reconstruction_state_at_or_before_in_index(
                    &index,
                    &key.network_id,
                    height,
                )
            })
            .transpose()?
            .flatten();
        let generation = index.generation;
        drop(index);
        Ok(ReputationCandidateCapture {
            archive: Arc::clone(self),
            kura: Arc::clone(kura),
            key,
            finalized_at_unix_ms,
            generation,
            previous,
            capture_attempted: false,
            captured: None,
            plan: None,
            reservation,
        })
    }
}

impl ReputationCandidateCapture {
    /// Capture once from the original executed State, using only the retained predecessor.
    /// Failure remains a recovery boundary; a later call cannot substitute another State view.
    pub(crate) fn capture_original(
        &mut self,
        state_ro: &impl StateReadOnly,
    ) -> Result<(), ReputationFinalizedArchiveError> {
        if self.capture_attempted {
            return Err(ReputationFinalizedArchiveError::FinalityAuthentication {
                reason: "candidate reputation State capture was already attempted",
            });
        }
        self.capture_attempted = true;
        let (key, finalized_at_unix_ms) = candidate_capture_key(state_ro, &self.kura)?;
        if key != self.key || finalized_at_unix_ms != self.finalized_at_unix_ms {
            return Err(ReputationFinalizedArchiveError::FinalityAuthentication {
                reason: "candidate reputation State differs from its reserved exact identity",
            });
        }
        self.captured = Some(self.archive.capture_original_successor(
            state_ro,
            key,
            finalized_at_unix_ms,
            self.previous.as_ref(),
        )?);
        Ok(())
    }

    /// Prepare insertion after detaching State journals, retaining exact capture on refusal.
    pub(crate) fn try_prepare(&mut self) -> Result<(), ReputationFinalizedArchiveError> {
        if self.plan.is_some() {
            return Ok(());
        }
        let captured = self.captured.as_ref().ok_or(
            ReputationFinalizedArchiveError::FinalityAuthentication {
                reason: "candidate reputation preparation has no original successful capture",
            },
        )?;
        let index = self.archive.try_write_reserved_index(&self.reservation)?;
        if index.generation != self.generation {
            return Err(
                ReputationFinalizedArchiveError::QualificationBoundaryChanged {
                    boundary: "reserved candidate archive",
                },
            );
        }
        let material = self.archive.prepare_captured_material(
            &index,
            &captured.next_state,
            &captured.authority_policy_history,
        )?;
        drop(index);
        let captured = self
            .captured
            .take()
            .expect("same successful original capture");
        self.plan = Some(material.map(|material| material.finish(captured.next_state)));
        Ok(())
    }

    /// Transfer only a completely prepared insertion, returning this exact owner otherwise.
    pub(crate) fn into_prepared(self) -> Result<PreparedReputationCapture, Self> {
        if self.plan.is_none() {
            return Err(self);
        }
        let Self {
            archive,
            kura,
            key,
            finalized_at_unix_ms,
            plan,
            reservation,
            ..
        } = self;
        Ok(PreparedReputationCapture {
            insertion: OwnedReputationInsertion {
                archive,
                key,
                finalized_at_unix_ms,
                state: plan.expect("checked prepared state"),
                reservation,
            },
            kura,
        })
    }
}

// All fallible preparation borrows the original successor. Only successful
// material consumes it, so a local refusal cannot discard or recapture execution.
struct PreparedReputationMaterial {
    policies: Vec<PreparedReputationPolicy>,
    anchor: PersistedReputationFinalizedAnchorV1,
    anchor_digest: [u8; 32],
    anchor_bytes: Vec<u8>,
    anchor_path: PathBuf,
    full_projection: Option<ReputationFinalizedProjectionV1>,
    total_bytes: u64,
    anchor_count: usize,
    generation: u64,
}

impl PreparedReputationMaterial {
    fn finish(self, next_state: ReputationReconstructionStateV1) -> PreparedReputationState {
        let Self {
            policies,
            anchor,
            anchor_digest,
            anchor_bytes,
            anchor_path,
            full_projection,
            total_bytes,
            anchor_count,
            generation,
        } = self;
        PreparedReputationState {
            policies,
            anchor,
            anchor_digest,
            anchor_bytes,
            anchor_path,
            next_state,
            full_projection,
            total_bytes,
            anchor_count,
            generation,
        }
    }
}
