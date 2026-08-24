/// Exhaustive source inventory of effects which may create pending work.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PendingWorkProducer {
    Sign,
    Fetch,
    Store,
    Validate,
    Apply,
    Output,
}

/// Exact signed-Proposal replay owner retained beside the ordinary body pipeline.
///
/// The stage changes only after the corresponding runtime or body-store cut
/// commits. No variant exposes replay evidence, pending bindings, or a caller-
/// selected lifecycle address.
#[allow(variant_size_differences)]
enum RemoteProposalReplayStageV1 {
    Fetch {
        work_id: EffectWorkId,
        replay: PreparedRemoteProposalFetchReplayPreAdmission,
    },
    BodyAvailable(PreparedRemoteProposalFetchReplayPreAdmission),
    StoreAdmission(PreparedRemoteProposalStoreReplayPreAdmission),
    Store {
        work_id: EffectWorkId,
        replay: PreparedRemoteProposalStoreReplayPreAdmission,
    },
    Stored {
        replay: PreparedRemoteProposalStoredReplayPreAdmission,
        ownership: RuntimeEffectOwnership,
    },
}

impl RemoteProposalReplayStageV1 {
    /// Recheck one stale ordinary Fetch against the already-installed signed origin.
    fn exactly_authenticates_fetch_rediscovery(&self, effect: &AdapterEffect) -> bool {
        match self {
            Self::Fetch { replay, .. } | Self::BodyAvailable(replay) => {
                replay.exactly_authenticates_fetch_rediscovery(effect)
            }
            Self::StoreAdmission(replay) | Self::Store { replay, .. } => {
                replay.exactly_authenticates_fetch_rediscovery(effect)
            }
            Self::Stored { replay, .. } => replay.exactly_authenticates_fetch_rediscovery(effect),
        }
    }
}

/// Exact authenticated-genesis replay owner retained beside the certified body pipeline.
///
/// Unlike an ordinary certified response, this source is already local and
/// launch-authenticated. The stage still advances through the same Store and
/// Validate ownership cuts, and no variant exposes its replay evidence or a
/// caller-selected lifecycle address.
#[allow(variant_size_differences)]
enum AuthenticatedGenesisReplayStageV1 {
    BodyAvailable(PreparedAuthenticatedGenesisFetchReplayPreAdmission),
    StoreAdmission(PreparedAuthenticatedGenesisStoreReplayPreAdmission),
    Store {
        work_id: EffectWorkId,
        replay: PreparedAuthenticatedGenesisStoreReplayPreAdmission,
    },
    Stored {
        replay: PreparedAuthenticatedGenesisStoredReplayPreAdmission,
        ownership: RuntimeEffectOwnership,
    },
}

impl AuthenticatedGenesisReplayStageV1 {
    fn store_work_id(&self) -> Option<EffectWorkId> {
        match self {
            Self::Store { work_id, .. } => Some(*work_id),
            Self::BodyAvailable(_) | Self::StoreAdmission(_) | Self::Stored { .. } => None,
        }
    }

    fn exactly_authenticates_fetch_rediscovery(&self, effect: &AdapterEffect) -> bool {
        match self {
            Self::BodyAvailable(replay) => replay.exactly_authenticates_fetch_rediscovery(effect),
            Self::StoreAdmission(replay) | Self::Store { replay, .. } => {
                replay.exactly_authenticates_fetch_rediscovery(effect)
            }
            Self::Stored { replay, .. } => replay.exactly_authenticates_fetch_rediscovery(effect),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AuthenticatedGenesisStoreReplayDispositionV1 {
    None,
    Advance,
    Retry,
}

/// Inert runtime fingerprint for one replay-authorized Validate after its
/// move-only admission owner transfers into the lifecycle registry.
#[derive(Clone, Debug, PartialEq, Eq)]
enum DurableValidateRetrySealV1 {
    /// Live admission retains the original executable runtime owner so later
    /// authority refinement can preserve its one physical lifecycle root.
    Live {
        effect: AdapterEffect,
        ownership: RuntimeEffectOwnership,
    },
    /// Cold recovery retains a registry-authenticated inert owner. The `Arc`
    /// permits transactional executor snapshots without making the move-only
    /// registry projection itself cloneable or externally reusable.
    Recovered {
        owner: Arc<RecoveredDurableValidateRetryOwnerV1>,
        frontier: RecoveredDurableValidateRetryFrontierV1,
    },
}

struct DurableValidateRetryProjectionV1 {
    seal: DurableValidateRetrySealV1,
    ownership: RuntimeEffectOwnership,
}

impl DurableValidateRetrySealV1 {
    fn seal_exact(
        effect: &AdapterEffect,
        ownership: &RuntimeEffectOwnership,
        pending: &PendingDurableValidateAdmissionV1,
    ) -> Option<Self> {
        matches!(effect, AdapterEffect::ValidateBody { .. })
            .then_some(())
            .filter(|()| pending.exactly_matches_retry(effect, ownership))?;
        Some(Self::Live {
            effect: effect.clone(),
            ownership: ownership.clone(),
        })
    }

    fn project_retry(
        &self,
        effect: &AdapterEffect,
        incoming: &RuntimeEffectOwnership,
    ) -> Result<DurableValidateRetryProjectionV1, String> {
        match self {
            Self::Live {
                effect: incumbent_effect,
                ownership: incumbent_ownership,
            } => {
                let (
                    AdapterEffect::ValidateBody {
                        tag: incumbent_tag,
                        round: incumbent_round,
                        subject: incumbent_subject,
                    },
                    AdapterEffect::ValidateBody {
                        tag: incoming_tag,
                        round: incoming_round,
                        subject: incoming_subject,
                    },
                ) = (incumbent_effect, effect)
                else {
                    return Err(
                        "durable Validate retry seal received another effect stage".to_owned()
                    );
                };
                if (incoming_tag != incumbent_tag
                    && !incoming_tag.strictly_advances(*incumbent_tag))
                    || incoming_round != incumbent_round
                    || incoming_subject != incumbent_subject
                    || incumbent_ownership
                        .exact_pending_adapter_effect_binding(incumbent_effect)
                        .is_err()
                {
                    return Err(
                        "durable Validate retry changed its exact body, tag, or incumbent owner"
                            .to_owned(),
                    );
                }
                let ownership = incumbent_ownership
                    .adopt_incumbent_body_stage_for_retry_or_authority(incoming, effect)?;
                ownership
                    .exact_pending_adapter_effect_binding(effect)
                    .map_err(|_| {
                        "durable Validate retry lost its exact adopted owner".to_owned()
                    })?;
                Ok(DurableValidateRetryProjectionV1 {
                    seal: Self::Live {
                        effect: effect.clone(),
                        ownership: ownership.clone(),
                    },
                    ownership,
                })
            }
            Self::Recovered { owner, frontier } => {
                let (frontier, ownership) =
                    owner.exactly_matches_retry(frontier, effect, incoming)?;
                Ok(DurableValidateRetryProjectionV1 {
                    seal: Self::Recovered {
                        owner: Arc::clone(owner),
                        frontier,
                    },
                    ownership,
                })
            }
        }
    }

    /// Project a durable commitment join without mutating the current seal.
    fn project_recovered_commitment_ceiling(
        &self,
        commitment: wire::ExecutionCommitment,
    ) -> Result<Option<Self>, String> {
        match self {
            Self::Live { .. } => Ok(None),
            Self::Recovered { owner, frontier } => frontier
                .project_commitment_ceiling(commitment)
                .map(|frontier| {
                    Some(Self::Recovered {
                        owner: Arc::clone(owner),
                        frontier,
                    })
                })
                .map_err(str::to_owned),
        }
    }
}

/// Atomic executor-side sink for one complete recovered Validate retry census.
///
/// The opaque registry census alone can obtain owners and feeds every one into
/// this preflight. No executor state changes until [`Self::commit`] consumes
/// the complete prepared map.
pub(in crate::sumeragi) struct PreparedRecoveredDurableValidateRetryInstallV1<'a, R> {
    executor: &'a mut V2EffectExecutor<R>,
    runtime_decision: Option<(
        wire::ConsensusRound,
        wire::ConsensusRound,
        wire::BlockSubject,
        wire::ExecutionCommitment,
    )>,
    prepared: BTreeMap<(wire::ConsensusRound, wire::BlockSubject), DurableValidateRetrySealV1>,
}

impl<R: EffectRuntime> PreparedRecoveredDurableValidateRetryInstallV1<'_, R> {
    /// Absorb one owner projected by the private complete registry census.
    pub(in crate::sumeragi) fn absorb(
        &mut self,
        owner: RecoveredDurableValidateRetryOwnerV1,
    ) -> Result<(), EffectExecutorError> {
        let key = owner.key();
        let validation_marker_is_exact = match (
            self.executor.validated_bodies.get(&key),
            self.executor.rejected_bodies.get(&key),
        ) {
            (None, None) => true,
            (Some(validated), None) => owner.exactly_matches_validated_marker(key, validated),
            (None, Some(rejected)) => rejected == owner.durable_receipt(),
            (Some(_), Some(_)) => false,
        };
        if owner
            .expected_decision()
            .is_some_and(|decision| self.runtime_decision != Some(decision))
            || self.executor.durable_bodies.get(&key) != Some(owner.durable_receipt())
            || !validation_marker_is_exact
            || self.executor.retired_rejected_bodies.contains_key(&key)
            || self
                .prepared
                .insert(
                    key,
                    DurableValidateRetrySealV1::Recovered {
                        frontier: owner.initial_retry_frontier().ok_or_else(|| {
                            EffectExecutorError::Contract(
                                "cold Validate retry owner omitted its initial frontier".to_owned(),
                            )
                        })?,
                        owner: Arc::new(owner),
                    },
                )
                .is_some()
        {
            return Err(EffectExecutorError::Contract(
                "cold Validate retry owner disagreed with runtime or body-store recovery"
                    .to_owned(),
            ));
        }
        Ok(())
    }

    /// Publish the fully preflighted census in one map replacement.
    pub(in crate::sumeragi) fn commit(self) -> Result<(), EffectExecutorError> {
        self.executor.durable_validate_retry_seals = self.prepared;
        Ok(())
    }
}

/// Inert fingerprint for one Validate row published by the direct lifecycle
/// Store-to-Validate transaction.
///
/// The lifecycle registry owns the executable pending binding. This marker
/// owns no service work and exists only so periodic reducer retransmission can
/// prove that the same physical Validate stage is already durable.
#[derive(Clone, Debug, PartialEq, Eq)]
struct PublishedLifecycleValidateRetryMarkerV1 {
    effect: AdapterEffect,
    statement: RuntimeCandidateSemanticStatement,
    durable_receipt: DurableBodyReceipt,
}

impl PublishedLifecycleValidateRetryMarkerV1 {
    fn prepare(
        effect: &AdapterEffect,
        durable_receipt: &DurableBodyReceipt,
        pending: &PendingRuntimeEffectBinding,
    ) -> Option<Self> {
        let AdapterEffect::ValidateBody {
            tag,
            round,
            subject,
        } = effect
        else {
            return None;
        };
        let statement = pending.candidate_statement()?;
        if !pending.exactly_binds_adapter_effect(effect)
            || tag.height() != round.height
            || durable_receipt.context_id() != round.context_id
            || durable_receipt.round() != *round
            || durable_receipt.subject() != *subject
            || statement.context_id() != round.context_id
            || statement.proposal_round() != *round
            || statement.subject() != Some(*subject)
        {
            return None;
        }
        Some(Self {
            effect: effect.clone(),
            statement,
            durable_receipt: durable_receipt.clone(),
        })
    }

    fn project_retry(
        &self,
        effect: &AdapterEffect,
        incoming: &RuntimeEffectOwnership,
    ) -> Result<Self, String> {
        let (
            AdapterEffect::ValidateBody {
                tag: incumbent_tag,
                round: incumbent_round,
                subject: incumbent_subject,
            },
            AdapterEffect::ValidateBody {
                tag: incoming_tag,
                round: incoming_round,
                subject: incoming_subject,
            },
        ) = (&self.effect, effect)
        else {
            return Err(
                "published lifecycle Validate marker received another effect stage".to_owned(),
            );
        };
        if (incoming_tag != incumbent_tag && !incoming_tag.strictly_advances(*incumbent_tag))
            || incoming_round != incumbent_round
            || incoming_subject != incumbent_subject
            || self.durable_receipt.round() != *incumbent_round
            || self.durable_receipt.subject() != *incumbent_subject
        {
            return Err(
                "published lifecycle Validate retry changed its exact body or regressed its tag"
                    .to_owned(),
            );
        }
        let pending = incoming
            .exact_pending_adapter_effect_binding(effect)
            .map_err(|_| {
                "published lifecycle Validate retry lost its exact runtime binding".to_owned()
            })?;
        let incoming_statement = pending.candidate_statement().ok_or_else(|| {
            "published lifecycle Validate retry omitted its candidate statement".to_owned()
        })?;
        let relation = self
            .statement
            .body_stage_authority_relation_to(incoming_statement)
            .ok_or_else(|| {
                "published lifecycle Validate retry changed its body or authority commitment"
                    .to_owned()
            })?;
        let statement = match relation {
            RuntimeFetchAuthorityRelation::Upgrade => incoming_statement,
            RuntimeFetchAuthorityRelation::Same | RuntimeFetchAuthorityRelation::Stale => {
                self.statement
            }
        };
        Ok(Self {
            effect: effect.clone(),
            statement,
            durable_receipt: self.durable_receipt.clone(),
        })
    }
}

/// Move-only preflight for installing one direct lifecycle Validate marker
/// after its LedgerV1 successor is durable.
#[must_use = "the direct lifecycle Validate marker has not crossed publication"]
pub(in crate::sumeragi) struct PreparedPublishedLifecycleValidateRetryMarkerV1 {
    durable_receipt: DurableBodyReceipt,
    marker: Option<PublishedLifecycleValidateRetryMarkerV1>,
}

impl PreparedPublishedLifecycleValidateRetryMarkerV1 {
    /// Bind the preflighted executor catalog slot to the exact Validate
    /// successor sealed by the registry after the adapter preview.
    pub(in crate::sumeragi) fn bind_validate_successor(
        mut self,
        effect: &AdapterEffect,
        pending: &PendingRuntimeEffectBinding,
    ) -> Result<Self, String> {
        let marker = PublishedLifecycleValidateRetryMarkerV1::prepare(
            effect,
            &self.durable_receipt,
            pending,
        )
        .ok_or_else(|| {
            "direct lifecycle Validate marker changed its exact Store successor".to_owned()
        })?;
        self.marker = Some(marker);
        Ok(self)
    }
}

impl<R: EffectRuntime> V2EffectExecutor<R> {
    /// Begin an atomic sink for the complete storage-authenticated census.
    pub(in crate::sumeragi) fn prepare_recovered_durable_validate_retry_install(
        &mut self,
    ) -> Result<PreparedRecoveredDurableValidateRetryInstallV1<'_, R>, EffectExecutorError> {
        self.ensure_open()?;
        let runtime_decision = self
            .runtime
            .decided_body()
            .map_err(EffectExecutorError::Runtime)?;
        if self.protected_decision.is_some()
            || !self.pending_durable_validate_admissions.is_empty()
            || !self.durable_validate_retry_seals.is_empty()
        {
            return Err(EffectExecutorError::Contract(
                "cold Validate retry census collided with live executor ownership".to_owned(),
            ));
        }
        Ok(PreparedRecoveredDurableValidateRetryInstallV1 {
            executor: self,
            runtime_decision,
            prepared: BTreeMap::new(),
        })
    }

    /// Return whether every inert Validate retry tombstone is scoped to the
    /// sole decided body which may survive until this per-height executor is
    /// consumed at rollover.
    fn durable_validate_retry_seals_are_finalization_inert(&self) -> bool {
        self.durable_validate_retry_seals
            .keys()
            .chain(self.published_lifecycle_validate_retry_markers.keys())
            .all(|key| {
                self.protected_decision
                    .is_some_and(|(_, round, subject, _)| *key == (round, subject))
            })
    }

    /// Preflight one inert retry marker before the direct Store-to-Validate
    /// transaction takes the runtime borrow and publishes its Ledger row.
    pub(in crate::sumeragi) fn prepare_published_lifecycle_validate_retry_marker(
        &self,
        durable_receipt: &DurableBodyReceipt,
    ) -> Result<PreparedPublishedLifecycleValidateRetryMarkerV1, EffectExecutorError> {
        let key = (durable_receipt.round(), durable_receipt.subject());
        let retained_body_is_exact =
            self.recovered_bodies
                .get(&key)
                .is_some_and(|(manifest, retained)| {
                    retained == durable_receipt
                        && manifest.round == durable_receipt.round()
                        && manifest.subject == durable_receipt.subject()
                        && HashOf::new(manifest) == durable_receipt.manifest_hash()
                });
        if !retained_body_is_exact
            || self.durable_bodies.get(&key) != Some(durable_receipt)
            || self.pending_durable_validate_admissions.contains_key(&key)
            || self.durable_validate_retry_seals.contains_key(&key)
            || self
                .published_lifecycle_validate_retry_markers
                .contains_key(&key)
        {
            return Err(EffectExecutorError::Contract(
                "direct lifecycle Validate marker overlaps a foreign executor body owner"
                    .to_owned(),
            ));
        }
        Ok(PreparedPublishedLifecycleValidateRetryMarkerV1 {
            durable_receipt: durable_receipt.clone(),
            marker: None,
        })
    }

    /// Install the preflighted marker after the direct successor and reducer
    /// transition have both crossed durable publication.
    pub(in crate::sumeragi) fn commit_published_lifecycle_validate_retry_marker(
        &mut self,
        prepared: PreparedPublishedLifecycleValidateRetryMarkerV1,
    ) {
        let marker = prepared
            .marker
            .expect("published direct lifecycle Validate retains its sealed marker");
        assert_eq!(marker.durable_receipt, prepared.durable_receipt);
        let key = (
            marker.durable_receipt.round(),
            marker.durable_receipt.subject(),
        );
        assert_eq!(self.durable_bodies.get(&key), Some(&marker.durable_receipt));
        assert!(!self.pending_durable_validate_admissions.contains_key(&key));
        assert!(!self.durable_validate_retry_seals.contains_key(&key));
        let previous = self
            .published_lifecycle_validate_retry_markers
            .insert(key, marker);
        assert!(previous.is_none());
    }

    /// Reinstall the same inert marker from any authenticated recovered
    /// durable Validate row before live clocks are armed.
    pub(in crate::sumeragi) fn install_recovered_published_lifecycle_validate_retry_marker(
        &mut self,
        effect: &AdapterEffect,
        pending: &PendingRuntimeEffectBinding,
        durable_receipt: &DurableBodyReceipt,
    ) -> Result<(), EffectExecutorError> {
        self.ensure_open()?;
        if self.runtime.lifecycle_live_clocks_are_armed() {
            return Err(EffectExecutorError::Contract(
                "recovered lifecycle Validate marker installation followed live clock activation"
                    .to_owned(),
            ));
        }
        let prepared = self
            .prepare_published_lifecycle_validate_retry_marker(durable_receipt)?
            .bind_validate_successor(effect, pending)
            .map_err(EffectExecutorError::Contract)?;
        self.commit_published_lifecycle_validate_retry_marker(prepared);
        Ok(())
    }

    /// Exact carrier block hashes still owned by retained missing-sidecar work.
    pub(crate) fn deferred_merge_sidecar_blocks(&self) -> BTreeSet<HashOf<BlockHeader>> {
        self.deferred_merge_work
            .keys()
            .filter_map(|work_id| {
                self.pending_applications
                    .get(work_id)
                    .map(|pending| pending.task.subject().block_hash)
            })
            .collect()
    }

    fn diagnostic_pending_work_is_exact(effect: &AdapterEffect) -> bool {
        Self::restart_effect_source(effect) != RestartEffectSource::DiagnosticOnly
            || matches!(
                Self::pending_work_producer(effect),
                None | Some(PendingWorkProducer::Output)
            )
    }

    /// Park one signed or diagnostic output before any external service I/O.
    fn park_lifecycle_output_admission(
        &mut self,
        effect: AdapterEffect,
        ownership: RuntimeEffectOwnership,
    ) -> Result<(), EffectExecutorError> {
        if let Some(existing) = self
            .pending_lifecycle_output_admissions
            .values()
            .find(|pending| pending.exactly_matches_retry(&effect, &ownership))
        {
            let _ = existing;
            return Ok(());
        }
        self.ensure_pending_slot()?;
        let pending =
            PendingLifecycleOutputAdmissionV1::seal_exact(effect, ownership).map_err(|_| {
                EffectExecutorError::Contract(
                    "signed lifecycle output omitted its exact runtime binding".to_owned(),
                )
            })?;
        let key = pending.key();
        match self.pending_lifecycle_output_admissions.entry(key) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(pending);
                Ok(())
            }
            std::collections::btree_map::Entry::Occupied(_) => Err(EffectExecutorError::Contract(
                "lifecycle output admission key collided with a foreign owner".to_owned(),
            )),
        }
    }

    fn pending_work_producer(effect: &AdapterEffect) -> Option<PendingWorkProducer> {
        match effect {
            AdapterEffect::Sign { .. } => Some(PendingWorkProducer::Sign),
            AdapterEffect::FetchBody { .. } => Some(PendingWorkProducer::Fetch),
            AdapterEffect::StoreBody { .. } => Some(PendingWorkProducer::Store),
            AdapterEffect::ValidateBody { .. } => Some(PendingWorkProducer::Validate),
            AdapterEffect::Apply { .. } => Some(PendingWorkProducer::Apply),
            AdapterEffect::Broadcast(_)
            | AdapterEffect::ReportEquivocation { .. }
            | AdapterEffect::ReportInvalidCertifiedBody { .. } => Some(PendingWorkProducer::Output),
            AdapterEffect::EnterView { .. } => None,
        }
    }

    fn allocate_work_id(&mut self) -> Result<EffectWorkId, EffectExecutorError> {
        let id = EffectWorkId(self.next_work_id);
        self.next_work_id = self
            .next_work_id
            .checked_add(1)
            .ok_or(EffectExecutorError::WorkIdExhausted)?;
        Ok(id)
    }

    /// Count live service/admission work. Inert Validate retry tombstones are
    /// deliberately excluded and are bounded by view/lock/Decision cleanup.
    fn pending_work(&self) -> usize {
        self.pending_signatures
            .len()
            .checked_add(self.pending_fetches.len())
            .and_then(|total| total.checked_add(self.pending_stores.len()))
            .and_then(|total| total.checked_add(self.pending_durable_validate_admissions.len()))
            .and_then(|total| {
                total.checked_add(usize::from(self.pending_live_wal_sign_admission.is_some()))
            })
            .and_then(|total| total.checked_add(self.pending_lifecycle_output_admissions.len()))
            .and_then(|total| total.checked_add(self.pending_applications.len()))
            .unwrap_or(usize::MAX)
    }

    fn install_pending_durable_validate_admission(
        &mut self,
        key: (wire::ConsensusRound, wire::BlockSubject),
        effect: &AdapterEffect,
        ownership: &RuntimeEffectOwnership,
        pending: PendingDurableValidateAdmissionV1,
    ) -> Result<(), EffectExecutorError> {
        if self.pending_durable_validate_admissions.contains_key(&key)
            || self.durable_validate_retry_seals.contains_key(&key)
            || self
                .published_lifecycle_validate_retry_markers
                .contains_key(&key)
        {
            return Err(EffectExecutorError::Contract(
                "durable Validate duplicated its exact lifecycle admission owner".to_owned(),
            ));
        }
        let seal = DurableValidateRetrySealV1::seal_exact(effect, ownership, &pending).ok_or_else(
            || {
                EffectExecutorError::Contract(
                    "durable Validate could not seal its exact post-admission retry owner"
                        .to_owned(),
                )
            },
        )?;
        let previous = self
            .pending_durable_validate_admissions
            .insert(key, pending);
        debug_assert!(previous.is_none());
        let previous = self.durable_validate_retry_seals.insert(key, seal);
        debug_assert!(previous.is_none());
        Ok(())
    }

    /// Execute the exact output service callback after lifecycle ordering grants the row.
    fn execute_lifecycle_output_service<S: V2EffectServices>(
        &mut self,
        effect: &AdapterEffect,
        ownership: &RuntimeEffectOwnership,
        services: &mut S,
    ) -> Result<LifecycleOutputServiceDispositionV1, EffectExecutorError> {
        match effect {
            AdapterEffect::Broadcast(message) => {
                message
                    .validate_version()
                    .map_err(|error| EffectExecutorError::Contract(error.to_string()))?;
                let proposal_round = match &message.payload {
                    wire::ConsensusMessageV2Payload::Proposal(proposal) => {
                        if proposal.round.context_id != self.context.id()
                            || proposal.round.height != self.context.height
                        {
                            return Err(EffectExecutorError::Contract(
                                "outbound Proposal changed the frozen height context".to_owned(),
                            ));
                        }
                        Some(proposal.round)
                    }
                    _ => None,
                };
                let disposition = services
                    .broadcast_consensus(message.clone())
                    .map_err(service_error)?;
                if let Some(proposal_round) = proposal_round
                    && disposition == ConsensusBroadcastDisposition::ExactServiceAccepted
                {
                    self.runtime
                        .complete_active_view_producer_after_proposal_fanout(
                            proposal_round,
                            ownership,
                        )
                        .map_err(EffectExecutorError::Runtime)?;
                }
                Ok(match disposition {
                    ConsensusBroadcastDisposition::ExactServiceAccepted => {
                        LifecycleOutputServiceDispositionV1::Accepted
                    }
                    ConsensusBroadcastDisposition::SourceRetained => {
                        LifecycleOutputServiceDispositionV1::SourceRetained
                    }
                })
            }
            AdapterEffect::ReportEquivocation { evidence } => {
                evidence
                    .validate_structure(&self.context)
                    .map_err(|reason| {
                        EffectExecutorError::Contract(format!(
                            "ReportEquivocation carried invalid evidence: {reason}"
                        ))
                    })?;
                services
                    .report_equivocation(evidence.to_wire())
                    .map_err(service_error)?;
                Ok(LifecycleOutputServiceDispositionV1::Accepted)
            }
            AdapterEffect::ReportInvalidCertifiedBody {
                subject,
                certificate,
            } => {
                services
                    .report_invalid_certified_body(*subject, certificate.clone())
                    .map_err(service_error)?;
                Ok(LifecycleOutputServiceDispositionV1::Accepted)
            }
            AdapterEffect::Sign { .. }
            | AdapterEffect::FetchBody { .. }
            | AdapterEffect::StoreBody { .. }
            | AdapterEffect::ValidateBody { .. }
            | AdapterEffect::Apply { .. }
            | AdapterEffect::EnterView { .. } => Err(EffectExecutorError::Contract(
                "non-output effect crossed the lifecycle output settlement seam".to_owned(),
            )),
        }
    }
}

impl V2EffectExecutor<SerializedV2Runtime> {
    /// Return whether a signed/diagnostic output is parked at the lifecycle cut.
    pub(in crate::sumeragi) fn has_pending_lifecycle_output_admissions(&self) -> bool {
        !self.pending_lifecycle_output_admissions.is_empty()
    }

    /// Settle each initially parked lifecycle output once in binding-key order.
    pub(in crate::sumeragi) fn settle_pending_lifecycle_output_admissions<S: V2EffectServices>(
        &mut self,
        owner: &mut ProductionLifecycleOwnerV1,
        services: &mut S,
    ) -> Result<usize, EffectExecutorError> {
        self.ensure_open()?;
        let keys = self
            .pending_lifecycle_output_admissions
            .keys()
            .copied()
            .collect::<Vec<_>>();
        let mut completed = 0usize;
        for key in keys {
            let Some(pending) = self.pending_lifecycle_output_admissions.remove(&key) else {
                continue;
            };
            let settlement =
                owner.settle_lifecycle_output_admission(pending, |effect, ownership| {
                    self.execute_lifecycle_output_service(effect, ownership, services)
                });
            match settlement {
                ProductionLifecycleOutputAdmissionSettlementV1::Completed
                | ProductionLifecycleOutputAdmissionSettlementV1::AlreadyCompleted => {
                    completed = completed.saturating_add(1);
                }
                ProductionLifecycleOutputAdmissionSettlementV1::Deferred(pending) => {
                    let previous = self
                        .pending_lifecycle_output_admissions
                        .insert(key, pending);
                    debug_assert!(previous.is_none());
                }
                ProductionLifecycleOutputAdmissionSettlementV1::Failed { failure, pending } => {
                    let previous = self
                        .pending_lifecycle_output_admissions
                        .insert(key, pending);
                    debug_assert!(previous.is_none());
                    let error = match failure {
                        ProductionLifecycleOutputAdmissionFailureV1::Service(error) => error,
                        ProductionLifecycleOutputAdmissionFailureV1::Projection(error) => {
                            EffectExecutorError::Contract(format!(
                                "lifecycle output admission projection failed: {error:?}"
                            ))
                        }
                        ProductionLifecycleOutputAdmissionFailureV1::Registry(reason) => {
                            EffectExecutorError::Contract(format!(
                                "lifecycle output registry settlement failed: {reason:?}"
                            ))
                        }
                        ProductionLifecycleOutputAdmissionFailureV1::Durability => {
                            EffectExecutorError::Contract(
                                "lifecycle output terminal publication failed".to_owned(),
                            )
                        }
                    };
                    return Err(self.close(error, services));
                }
            }
        }
        Ok(completed)
    }

    /// Return whether an exact durable Validate owner is parked at lifecycle admission.
    pub(in crate::sumeragi) fn has_pending_durable_validate_admissions(&self) -> bool {
        !self.pending_durable_validate_admissions.is_empty()
    }

    /// Return whether one exact post-fsync live-WAL Sign is parked at lifecycle admission.
    pub(in crate::sumeragi) fn has_pending_live_wal_sign_admission(&self) -> bool {
        self.pending_live_wal_sign_admission.is_some()
    }

    /// Settle the exact post-fsync live-WAL Sign before generic signed-output dispatch.
    pub(in crate::sumeragi) fn settle_pending_live_wal_sign_admission<S: V2EffectServices>(
        &mut self,
        owner: &mut ProductionLifecycleOwnerV1,
        services: &mut S,
    ) -> Result<usize, EffectExecutorError> {
        self.ensure_open()?;
        let Some(pending) = self.pending_live_wal_sign_admission.take() else {
            return Ok(0);
        };
        match owner.settle_live_wal_sign_admission(pending) {
            ProductionLiveWalSignAdmissionSettlementV1::Admitted(AdmissionDecision::Admitted {
                ..
            })
            | ProductionLiveWalSignAdmissionSettlementV1::Rebound(AdmissionDecision::Retry {
                ..
            }) => Ok(1),
            ProductionLiveWalSignAdmissionSettlementV1::Admitted(decision)
            | ProductionLiveWalSignAdmissionSettlementV1::Rebound(decision) => Err(self.close(
                EffectExecutorError::Contract(format!(
                    "live WAL Sign settlement committed an invalid logical decision: {decision:?}"
                )),
                services,
            )),
            ProductionLiveWalSignAdmissionSettlementV1::Returned {
                decision: AdmissionDecision::WaitForCapacity(_),
                pending,
            } => {
                self.pending_live_wal_sign_admission = Some(pending);
                Ok(0)
            }
            ProductionLiveWalSignAdmissionSettlementV1::Returned {
                decision:
                    AdmissionDecision::Retry { .. }
                    | AdmissionDecision::ReplayTerminal { .. }
                    | AdmissionDecision::StutterTerminal { .. },
                pending: _,
            } => Ok(0),
            ProductionLiveWalSignAdmissionSettlementV1::Returned { decision, pending } => {
                self.pending_live_wal_sign_admission = Some(pending);
                Err(self.close(
                    EffectExecutorError::Contract(format!(
                        "live WAL Sign admission returned a terminally invalid decision: {decision:?}"
                    )),
                    services,
                ))
            }
            ProductionLiveWalSignAdmissionSettlementV1::Failed { failure, pending } => {
                self.pending_live_wal_sign_admission = Some(pending);
                let error = match failure {
                    ProductionLiveWalSignAdmissionFailureV1::Projection(error) => {
                        EffectExecutorError::Contract(format!(
                            "live WAL Sign admission projection failed: {error:?}"
                        ))
                    }
                    ProductionLiveWalSignAdmissionFailureV1::Registry => {
                        EffectExecutorError::Contract(
                            "live WAL Sign registry settlement failed".to_owned(),
                        )
                    }
                    ProductionLiveWalSignAdmissionFailureV1::Durability => {
                        EffectExecutorError::Contract(
                            "live WAL Sign admission publication failed".to_owned(),
                        )
                    }
                };
                Err(self.close(error, services))
            }
        }
    }

    /// Settle each currently pending durable Validate owner once, in body-key order.
    ///
    /// Capacity waits restore the exact move-only owner. An exact logical
    /// `Retry` consumes the duplicate because the incumbent ordinal and
    /// registry carrier already own execution. Any other non-committing
    /// decision is a production invariant violation and closes the shared
    /// output gate while preserving the pending owner for restart diagnosis.
    pub(in crate::sumeragi) fn settle_pending_durable_validate_admissions<S: V2EffectServices>(
        &mut self,
        owner: &mut ProductionLifecycleOwnerV1,
        services: &mut S,
    ) -> Result<usize, EffectExecutorError> {
        self.ensure_open()?;
        let pending_keys = self
            .pending_durable_validate_admissions
            .keys()
            .copied()
            .collect::<Vec<_>>();
        let mut made_ready = 0usize;
        for key in pending_keys {
            let Some(pending) = self.pending_durable_validate_admissions.remove(&key) else {
                continue;
            };
            match owner.settle_durable_validate_admission(pending) {
                ProductionDurableValidateAdmissionSettlementV1::Admitted(
                    AdmissionDecision::Admitted { .. },
                )
                | ProductionDurableValidateAdmissionSettlementV1::Rebound(
                    AdmissionDecision::Retry { .. },
                ) => {
                    made_ready = made_ready.saturating_add(1);
                }
                ProductionDurableValidateAdmissionSettlementV1::Admitted(decision)
                | ProductionDurableValidateAdmissionSettlementV1::Rebound(decision) => {
                    return Err(self.close(
                        EffectExecutorError::Contract(format!(
                            "durable Validate settlement committed an invalid logical decision: {decision:?}"
                        )),
                        services,
                    ));
                }
                ProductionDurableValidateAdmissionSettlementV1::Returned {
                    decision: AdmissionDecision::WaitForCapacity(_),
                    pending,
                } => {
                    let previous = self
                        .pending_durable_validate_admissions
                        .insert(key, pending);
                    debug_assert!(previous.is_none());
                }
                ProductionDurableValidateAdmissionSettlementV1::Returned {
                    decision:
                        AdmissionDecision::Retry { .. }
                        | AdmissionDecision::ReplayTerminal { .. }
                        | AdmissionDecision::StutterTerminal { .. },
                    pending: _,
                } => {}
                ProductionDurableValidateAdmissionSettlementV1::Returned { decision, pending } => {
                    let previous = self
                        .pending_durable_validate_admissions
                        .insert(key, pending);
                    debug_assert!(previous.is_none());
                    return Err(self.close(
                        EffectExecutorError::Contract(format!(
                            "durable Validate admission returned a terminally invalid decision: {decision:?}"
                        )),
                        services,
                    ));
                }
                ProductionDurableValidateAdmissionSettlementV1::Failed { failure, pending } => {
                    let previous = self
                        .pending_durable_validate_admissions
                        .insert(key, pending);
                    debug_assert!(previous.is_none());
                    return Err(self.close(
                        EffectExecutorError::Contract(format!(
                            "durable Validate admission failed before commit: {failure:?}"
                        )),
                        services,
                    ));
                }
            }
        }
        Ok(made_ready)
    }
}

impl<R: EffectRuntime> V2EffectExecutor<R> {
    fn payload_chunk_reconstruction_task<S: V2EffectServices>(
        &mut self,
        work_id: EffectWorkId,
        services: &mut S,
    ) -> Result<BodyFetchTask, EffectTransportError> {
        let task = self
            .pending_fetches
            .get(&work_id)
            .ok_or(EffectTransportError::UnknownWork(work_id))?
            .task
            .clone();
        if task.certified_request.is_none() {
            return Ok(task);
        }
        let key = (task.round, task.subject);
        match self.remote_proposal_replay.get(&key) {
            Some(RemoteProposalReplayStageV1::Fetch {
                work_id: replay_work_id,
                ..
            }) if *replay_work_id == work_id => Ok(task),
            Some(_) => Err(self.fail_closed_transport(
                "certificate-backed payload chunks conflict with the retained Proposal replay stage",
                services,
            )),
            None => {
                // A certificate-only Fetch gets its body-frame replay
                // authority from the authenticated CertifiedBodyResponse
                // lifecycle. Generic chunk reconstruction has no equivalent
                // authority to carry through Store and Validate, so leave
                // the exact request/fetch live for that typed response path.
                Err(EffectTransportError::WrongFetchKind)
            }
        }
    }

    fn accept_payload_chunk_inner<S: V2EffectServices>(
        &mut self,
        work_id: EffectWorkId,
        chunk: wire::PayloadChunk,
        authenticated_sender: &PeerId,
        services: &mut S,
    ) -> Result<(), EffectTransportError> {
        if self.output_guard.restart_required() {
            return Err(EffectTransportError::FailClosed(
                "process restart is required after a fatal consensus failure".to_owned(),
            ));
        }
        if let Some(reason) = &self.fatal_reason {
            return Err(EffectTransportError::FailClosed(reason.clone()));
        }
        let task = self.payload_chunk_reconstruction_task(work_id, services)?;
        let manifest = task
            .manifest
            .as_ref()
            .ok_or(EffectTransportError::WrongFetchKind)?;
        let authenticated =
            authenticate_payload_chunk(&self.context, manifest, chunk, authenticated_sender)?;
        match services.accept_authenticated_chunk(&task, authenticated) {
            Ok(AuthenticatedChunkDisposition::Accepted) => {}
            Ok(AuthenticatedChunkDisposition::Rejected) => {
                self.reject_noncanonical_reconstruction(work_id, services)?;
                return Err(EffectTransportError::BodyMismatch(
                    "authenticated chunks reconstructed invalid or noncanonical body data",
                ));
            }
            Err(error) => {
                let reason = EffectExecutorError::Service(error.to_string()).to_string();
                self.fatal_reason = Some(reason.clone());
                services.fail_closed(&reason);
                return Err(EffectTransportError::FailClosed(reason));
            }
        }
        Ok(())
    }

    /// Retain the exact resultless wire of the already-authenticated staged
    /// genesis as a process-local acquisition source.
    ///
    /// Genesis is signed once with a fixed view-zero header, while its
    /// consensus Proposal may be reissued in later views with a new manifest.
    /// The opaque installed authority, canonical bytes, and subject remain one
    /// value until the certified Fetch projects its Store replay lineage.
    pub(in crate::sumeragi) fn install_authenticated_genesis_body(
        &mut self,
        authenticated_genesis: &super::v2_context::AuthenticatedGenesisBodyV1,
    ) -> Result<(), EffectExecutorError> {
        self.ensure_open()?;
        let installed = InstalledAuthenticatedGenesisReplayAuthorityV1::install(
            authenticated_genesis,
            &self.context,
        )
        .map_err(|reason| EffectExecutorError::Contract(reason.to_owned()))?;
        let subject = installed.subject();
        let canonical_wire = Arc::clone(installed.canonical_wire());
        let genesis_round = wire::ConsensusRound {
            context_id: self.context.id(),
            height: self.context.height,
            view: 0,
        };
        ReadyBody::derive(
            &self.context,
            genesis_round,
            subject,
            Arc::clone(&canonical_wire),
        )
        .map_err(|error| EffectExecutorError::Contract(error.to_string()))?;
        if let Some(retained) = self.authenticated_genesis_body.as_ref() {
            if retained.subject() == subject
                && retained.canonical_wire().as_ref() == canonical_wire.as_ref()
            {
                return Ok(());
            }
            return Err(EffectExecutorError::Contract(
                "authenticated staged genesis changed after executor construction".to_owned(),
            ));
        }
        self.authenticated_genesis_body = Some(installed);
        Ok(())
    }

    /// Install synthetic authenticated-genesis authority for executor fixtures.
    #[cfg(test)]
    fn install_authenticated_genesis_body_for_test(
        &mut self,
        authenticated_genesis: &iroha_data_model::block::SignedBlock,
    ) -> Result<(), EffectExecutorError> {
        self.ensure_open()?;
        let installed = InstalledAuthenticatedGenesisReplayAuthorityV1::for_test(
            authenticated_genesis,
            &self.context,
        )
        .ok_or_else(|| {
            EffectExecutorError::Contract(
                "synthetic authenticated staged genesis is not canonical".to_owned(),
            )
        })?;
        if let Some(retained) = self.authenticated_genesis_body.as_ref() {
            return (retained.subject() == installed.subject()
                && retained.canonical_wire().as_ref() == installed.canonical_wire().as_ref())
            .then_some(())
            .ok_or_else(|| {
                EffectExecutorError::Contract(
                    "authenticated staged genesis changed after executor construction".to_owned(),
                )
            });
        }
        self.authenticated_genesis_body = Some(installed);
        Ok(())
    }

    fn stored_replay_incumbent_validate_ownership(
        &self,
        key: (wire::ConsensusRound, wire::BlockSubject),
        effect: &AdapterEffect,
    ) -> Result<Option<RuntimeEffectOwnership>, EffectExecutorError> {
        if self.authenticated_genesis_replay.contains_key(&key)
            && self.remote_proposal_replay.contains_key(&key)
        {
            return Err(EffectExecutorError::Contract(
                "one stored body retained two replay lineages".to_owned(),
            ));
        }
        let Some(receipt) = self.durable_bodies.get(&key) else {
            if self.authenticated_genesis_replay.contains_key(&key)
                || self.remote_proposal_replay.contains_key(&key)
            {
                return Err(EffectExecutorError::Contract(
                    "stored body replay lost its durable receipt during retention".to_owned(),
                ));
            }
            return Ok(None);
        };
        let incumbent = match (
            self.remote_proposal_replay.get(&key),
            self.authenticated_genesis_replay.get(&key),
        ) {
            (Some(RemoteProposalReplayStageV1::Stored { replay, ownership }), None) => {
                replay.project_incumbent_validate_ownership(receipt, ownership, effect)
            }
            (None, Some(AuthenticatedGenesisReplayStageV1::Stored { replay, ownership })) => {
                replay.project_incumbent_validate_ownership(receipt, ownership, effect)
            }
            _ => return Ok(None),
        };
        incumbent.map(Some).ok_or_else(|| {
            EffectExecutorError::Contract(
                "stored body replay could not project its incumbent Validate owner".to_owned(),
            )
        })
    }

    /// Rejoin a refined Proposal body owner to the complete durable QC behind it.
    ///
    /// Runtime candidate statements are intentionally process-local. They may
    /// prove positional refinement, but only the reducer-authenticated QC may
    /// become replay authority in a published lifecycle row.
    fn exact_remote_proposal_validate_authority_certificate(
        &self,
        effect: &AdapterEffect,
        ownership: &RuntimeEffectOwnership,
    ) -> Result<Option<wire::QuorumCertificate>, EffectExecutorError> {
        let pending = ownership
            .exact_pending_adapter_effect_binding(effect)
            .map_err(|_| {
                EffectExecutorError::Contract(
                    "Proposal Validate lost its exact runtime binding before replay refinement"
                        .to_owned(),
                )
            })?;
        let statement = pending.candidate_statement().ok_or_else(|| {
            EffectExecutorError::Contract(
                "Proposal Validate omitted its route-neutral candidate statement".to_owned(),
            )
        })?;
        let Some(phase) = statement.phase() else {
            return statement
                .execution_commitment()
                .is_none()
                .then_some(None)
                .ok_or_else(|| {
                    EffectExecutorError::Contract(
                        "ordinary Proposal Validate carried a commitment without quorum authority"
                            .to_owned(),
                    )
                });
        };
        let certificate = self
            .runtime
            .durable_body_authority_certificate()
            .map_err(EffectExecutorError::Runtime)?
            .ok_or_else(|| {
                EffectExecutorError::Contract(
                    "authority-refined Proposal Validate omitted its durable QC".to_owned(),
                )
            })?;
        self.runtime
            .verify_certificate(&self.context, &certificate)
            .map_err(|reason| {
                EffectExecutorError::Contract(format!(
                    "authority-refined Proposal Validate carried an invalid durable QC: {reason}"
                ))
            })?;
        if certificate.phase != phase
            || certificate.round != statement.round()
            || certificate.proposal_round != statement.proposal_round()
            || certificate.subject != statement.subject().expect("body statement has one subject")
            || Some(certificate.execution_commitment) != statement.execution_commitment()
        {
            return Err(EffectExecutorError::Contract(
                "authority-refined Proposal Validate changed its durable QC coordinates".to_owned(),
            ));
        }
        let protected = match phase {
            wire::GlobalPhase::Prepare => {
                self.protected_decision.is_none()
                    && self.protected_lock
                        == Some((certificate.proposal_round, certificate.subject))
            }
            wire::GlobalPhase::Commit => {
                self.protected_decision
                    == Some((
                        certificate.round,
                        certificate.proposal_round,
                        certificate.subject,
                        certificate.execution_commitment,
                    ))
            }
        };
        if !protected {
            return Err(EffectExecutorError::Contract(
                "authority-refined Proposal Validate is not the protected durable body".to_owned(),
            ));
        }
        Ok(Some(certificate))
    }

    fn preflight_authenticated_genesis_store_completion(
        &self,
        key: (wire::ConsensusRound, wire::BlockSubject),
        pending: &PendingStore,
        work_id: EffectWorkId,
    ) -> Result<bool, EffectExecutorError> {
        match self.authenticated_genesis_replay.get(&key) {
            Some(AuthenticatedGenesisReplayStageV1::Store {
                work_id: retained,
                replay,
            }) => {
                let effect = AdapterEffect::StoreBody {
                    tag: pending.task.tag(),
                    round: key.0,
                    subject: key.1,
                };
                if *retained != work_id
                    || !replay.exactly_matches_retry(&effect, pending.task.ownership())
                {
                    return Err(EffectExecutorError::Contract(
                        "authenticated-genesis Store completion changed its replay owner"
                            .to_owned(),
                    ));
                }
                Ok(true)
            }
            Some(AuthenticatedGenesisReplayStageV1::BodyAvailable(_))
            | Some(AuthenticatedGenesisReplayStageV1::StoreAdmission(_)) => {
                Err(EffectExecutorError::Contract(
                    "authenticated-genesis Store completion preceded its retained stage".to_owned(),
                ))
            }
            Some(AuthenticatedGenesisReplayStageV1::Stored { .. }) | None => Ok(false),
        }
    }

    fn commit_authenticated_genesis_store_completion(
        &mut self,
        key: (wire::ConsensusRound, wire::BlockSubject),
        work_id: EffectWorkId,
        receipt: DurableBodyReceipt,
        ownership: RuntimeEffectOwnership,
    ) -> Result<(), EffectExecutorError> {
        let Some(AuthenticatedGenesisReplayStageV1::Store {
            work_id: retained,
            replay,
        }) = self.authenticated_genesis_replay.remove(&key)
        else {
            return Err(EffectExecutorError::Contract(
                "preflighted authenticated-genesis Store replay disappeared".to_owned(),
            ));
        };
        if retained != work_id {
            let previous = self.authenticated_genesis_replay.insert(
                key,
                AuthenticatedGenesisReplayStageV1::Store {
                    work_id: retained,
                    replay,
                },
            );
            debug_assert!(previous.is_none());
            return Err(EffectExecutorError::Contract(
                "authenticated-genesis Store work ID changed before commit".to_owned(),
            ));
        }
        let stored = match replay.bind_durable_body(receipt.clone()) {
            Ok(stored) => stored,
            Err(error) => {
                let previous = self.authenticated_genesis_replay.insert(
                    key,
                    AuthenticatedGenesisReplayStageV1::Store {
                        work_id,
                        replay: error.into_store(),
                    },
                );
                debug_assert!(previous.is_none());
                return Err(EffectExecutorError::Contract(
                    "authenticated-genesis Store completion changed its durable body".to_owned(),
                ));
            }
        };
        if !stored.exactly_retains_owned_store(&receipt, &ownership) {
            return Err(EffectExecutorError::Contract(
                "authenticated-genesis Store completion changed its runtime owner".to_owned(),
            ));
        }
        let previous = self.authenticated_genesis_replay.insert(
            key,
            AuthenticatedGenesisReplayStageV1::Stored {
                replay: stored,
                ownership,
            },
        );
        debug_assert!(previous.is_none());
        Ok(())
    }

    fn prepare_authenticated_genesis_store_replay(
        &mut self,
        key: (wire::ConsensusRound, wire::BlockSubject),
        effect: &AdapterEffect,
        ownership: &RuntimeEffectOwnership,
    ) -> Result<AuthenticatedGenesisStoreReplayDispositionV1, EffectExecutorError> {
        if self.authenticated_genesis_replay.contains_key(&key)
            && self.remote_proposal_replay.contains_key(&key)
        {
            return Err(EffectExecutorError::Contract(
                "one body stage retained both Proposal and authenticated-genesis replay".to_owned(),
            ));
        }
        match self.authenticated_genesis_replay.get(&key) {
            Some(AuthenticatedGenesisReplayStageV1::StoreAdmission(replay)) => {
                if !replay.exactly_matches_retry(effect, ownership) {
                    return Err(EffectExecutorError::Contract(
                        "authenticated-genesis Store retry changed its projected replay owner"
                            .to_owned(),
                    ));
                }
                return Ok(AuthenticatedGenesisStoreReplayDispositionV1::Advance);
            }
            Some(AuthenticatedGenesisReplayStageV1::Store { replay, .. }) => {
                if !replay.exactly_matches_retry(effect, ownership) {
                    return Err(EffectExecutorError::Contract(
                        "authenticated-genesis Store retry changed its exact replay owner"
                            .to_owned(),
                    ));
                }
                return Ok(AuthenticatedGenesisStoreReplayDispositionV1::Retry);
            }
            Some(AuthenticatedGenesisReplayStageV1::Stored {
                replay,
                ownership: stored_ownership,
            }) => {
                let receipt = self.durable_bodies.get(&key).ok_or_else(|| {
                    EffectExecutorError::Contract(
                        "durable authenticated-genesis replay lost its body receipt".to_owned(),
                    )
                })?;
                if ownership != stored_ownership
                    || !replay.exactly_retains_owned_store(receipt, stored_ownership)
                    || !replay.exactly_matches_retry(effect, receipt)
                {
                    return Err(EffectExecutorError::Contract(
                        "durable authenticated-genesis Store retry changed its replay owner"
                            .to_owned(),
                    ));
                }
                return Ok(AuthenticatedGenesisStoreReplayDispositionV1::Retry);
            }
            Some(AuthenticatedGenesisReplayStageV1::BodyAvailable(_)) | None => {}
        }
        let Some(AuthenticatedGenesisReplayStageV1::BodyAvailable(fetch)) =
            self.authenticated_genesis_replay.remove(&key)
        else {
            return Ok(AuthenticatedGenesisStoreReplayDispositionV1::None);
        };
        let store = match fetch.project_store(effect.clone(), ownership.clone()) {
            Ok(store) => store,
            Err(error) => {
                let previous = self.authenticated_genesis_replay.insert(
                    key,
                    AuthenticatedGenesisReplayStageV1::BodyAvailable(error.into_fetch()),
                );
                debug_assert!(previous.is_none());
                return Err(EffectExecutorError::Contract(
                    "authenticated-genesis Fetch could not project its exact Store successor"
                        .to_owned(),
                ));
            }
        };
        let previous = self.authenticated_genesis_replay.insert(
            key,
            AuthenticatedGenesisReplayStageV1::StoreAdmission(store),
        );
        debug_assert!(previous.is_none());
        Ok(AuthenticatedGenesisStoreReplayDispositionV1::Advance)
    }

    fn commit_authenticated_genesis_store_replay(
        &mut self,
        key: (wire::ConsensusRound, wire::BlockSubject),
        ownership: RuntimeEffectOwnership,
    ) -> Result<(), EffectExecutorError> {
        let Some(AuthenticatedGenesisReplayStageV1::StoreAdmission(store)) =
            self.authenticated_genesis_replay.remove(&key)
        else {
            return Err(EffectExecutorError::Contract(
                "serialized Store lost its authenticated-genesis replay stage".to_owned(),
            ));
        };
        let stage = if let Some(receipt) = self.durable_bodies.get(&key).cloned() {
            let stored = match store.bind_durable_body(receipt.clone()) {
                Ok(stored) => stored,
                Err(error) => {
                    let previous = self.authenticated_genesis_replay.insert(
                        key,
                        AuthenticatedGenesisReplayStageV1::StoreAdmission(error.into_store()),
                    );
                    debug_assert!(previous.is_none());
                    return Err(EffectExecutorError::Contract(
                        "authenticated-genesis Store could not bind its durable body".to_owned(),
                    ));
                }
            };
            if !stored.exactly_retains_owned_store(&receipt, &ownership) {
                return Err(EffectExecutorError::Contract(
                    "authenticated-genesis Store changed its retained runtime owner".to_owned(),
                ));
            }
            AuthenticatedGenesisReplayStageV1::Stored {
                replay: stored,
                ownership,
            }
        } else {
            let Some(work_id) = self.pending_stores.iter().find_map(|(work_id, pending)| {
                (pending.task.manifest.round == key.0 && pending.task.manifest.subject == key.1)
                    .then_some(*work_id)
            }) else {
                let previous = self.authenticated_genesis_replay.insert(
                    key,
                    AuthenticatedGenesisReplayStageV1::StoreAdmission(store),
                );
                debug_assert!(previous.is_none());
                return Err(EffectExecutorError::Contract(
                    "authenticated-genesis Store installed neither durable nor pending work"
                        .to_owned(),
                ));
            };
            AuthenticatedGenesisReplayStageV1::Store {
                work_id,
                replay: store,
            }
        };
        let previous = self.authenticated_genesis_replay.insert(key, stage);
        debug_assert!(previous.is_none());
        Ok(())
    }

    fn commit_remote_proposal_body_available_replay(
        &mut self,
        key: (wire::ConsensusRound, wire::BlockSubject),
        replay: Option<PreparedRemoteProposalFetchReplayPreAdmission>,
    ) {
        if let Some(replay) = replay {
            let previous = self
                .remote_proposal_replay
                .insert(key, RemoteProposalReplayStageV1::BodyAvailable(replay));
            assert!(
                previous.is_none(),
                "remote Proposal replay preflight keeps its body key vacant"
            );
        }
    }

    /// Prove every signed-Proposal replay token is attached to its exact
    /// executor-owned physical stage before a view or Decision retires it.
    fn preflight_remote_proposal_replay_indexes(&self) -> Result<(), EffectExecutorError> {
        if self
            .authenticated_genesis_replay
            .keys()
            .any(|key| self.remote_proposal_replay.contains_key(key))
        {
            return Err(EffectExecutorError::Contract(
                "one physical body stage retained two replay lineages".to_owned(),
            ));
        }
        for (key, stage) in &self.remote_proposal_replay {
            let exact = match stage {
                RemoteProposalReplayStageV1::Fetch { work_id, .. } => self
                    .pending_fetches
                    .get(work_id)
                    .is_some_and(|pending| (pending.task.round, pending.task.subject) == *key),
                RemoteProposalReplayStageV1::BodyAvailable(_) => {
                    self.body_pipeline_owners.contains_key(key)
                        && self.retained_body_manifest_hash(*key)?.is_some()
                }
                // StoreAdmission exists only inside one serialized StoreBody
                // call. Observing it at a later control boundary would mean
                // the move-only projection escaped that transaction.
                RemoteProposalReplayStageV1::StoreAdmission(_) => false,
                RemoteProposalReplayStageV1::Store { work_id, .. } => {
                    self.pending_stores.get(work_id).is_some_and(|pending| {
                        (pending.task.manifest.round, pending.task.manifest.subject) == *key
                    })
                }
                RemoteProposalReplayStageV1::Stored { replay, ownership } => {
                    self.durable_bodies.get(key).is_some_and(|receipt| {
                        (receipt.round(), receipt.subject()) == *key
                            && replay.exactly_retains_owned_store(receipt, ownership)
                    })
                }
            };
            if !exact {
                return Err(EffectExecutorError::Contract(
                    "remote Proposal replay is detached from its exact physical body stage"
                        .to_owned(),
                ));
            }
        }
        for (key, stage) in &self.authenticated_genesis_replay {
            let exact = match stage {
                AuthenticatedGenesisReplayStageV1::BodyAvailable(_) => {
                    self.body_pipeline_owners.contains_key(key)
                        && self.retained_body_manifest_hash(*key)?.is_some()
                }
                AuthenticatedGenesisReplayStageV1::StoreAdmission(_) => false,
                AuthenticatedGenesisReplayStageV1::Store { work_id, replay } => {
                    self.pending_stores.get(work_id).is_some_and(|pending| {
                        let effect = AdapterEffect::StoreBody {
                            tag: pending.task.tag(),
                            round: key.0,
                            subject: key.1,
                        };
                        (pending.task.manifest.round, pending.task.manifest.subject) == *key
                            && replay.exactly_matches_retry(&effect, pending.task.ownership())
                    })
                }
                AuthenticatedGenesisReplayStageV1::Stored { replay, ownership } => {
                    self.durable_bodies.get(key).is_some_and(|receipt| {
                        (receipt.round(), receipt.subject()) == *key
                            && replay.exactly_retains_owned_store(receipt, ownership)
                    })
                }
            };
            if !exact {
                return Err(EffectExecutorError::Contract(
                    "authenticated-genesis replay is detached from its physical body stage"
                        .to_owned(),
                ));
            }
        }
        Ok(())
    }

    fn validate_body<S: V2EffectServices>(
        &mut self,
        tag: EventTag,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
        ownership: RuntimeEffectOwnership,
        _services: &mut S,
    ) -> Result<(), EffectExecutorError> {
        let key = (round, subject);
        let effect = AdapterEffect::ValidateBody {
            tag,
            round,
            subject,
        };
        if let Some(pending) = self.pending_durable_validate_admissions.get(&key) {
            if !pending.exactly_matches_retry(&effect, &ownership) {
                return Err(EffectExecutorError::Contract(
                    "ValidateBody retry changed its exact pending lifecycle owner".to_owned(),
                ));
            }
            return Ok(());
        }
        if let Some(marker) = self
            .published_lifecycle_validate_retry_markers
            .get_mut(&key)
        {
            *marker = marker
                .project_retry(&effect, &ownership)
                .map_err(EffectExecutorError::Contract)?;
            return Ok(());
        }
        if let Some(seal) = self.durable_validate_retry_seals.get_mut(&key) {
            let projected = seal
                .project_retry(&effect, &ownership)
                .map_err(EffectExecutorError::Contract)?;
            *seal = projected.seal;
            return Ok(());
        }
        let receipt = self.durable_bodies.get(&key).cloned().ok_or_else(|| {
            EffectExecutorError::Contract(
                "ValidateBody has no matching durable body receipt".to_owned(),
            )
        })?;
        if self.authenticated_genesis_replay.contains_key(&key)
            && self.remote_proposal_replay.contains_key(&key)
        {
            return Err(EffectExecutorError::Contract(
                "ValidateBody retained two incompatible replay authorities".to_owned(),
            ));
        }
        match self.authenticated_genesis_replay.get(&key) {
            Some(AuthenticatedGenesisReplayStageV1::BodyAvailable(_))
            | Some(AuthenticatedGenesisReplayStageV1::StoreAdmission(_))
            | Some(AuthenticatedGenesisReplayStageV1::Store { .. }) => {
                return Err(EffectExecutorError::Contract(
                    "authenticated-genesis ValidateBody preceded its durable Store replay"
                        .to_owned(),
                ));
            }
            Some(AuthenticatedGenesisReplayStageV1::Stored {
                replay,
                ownership: store_ownership,
            }) => {
                if !replay.exactly_retains_owned_store(&receipt, store_ownership) {
                    return Err(EffectExecutorError::Contract(
                        "authenticated-genesis ValidateBody changed its Store lineage".to_owned(),
                    ));
                }
            }
            None => {}
        }
        if self.authenticated_genesis_replay.contains_key(&key) {
            let Some(AuthenticatedGenesisReplayStageV1::Stored {
                replay: stored,
                ownership: store_ownership,
            }) = self.authenticated_genesis_replay.remove(&key)
            else {
                unreachable!("preflighted authenticated-genesis Store replay remains installed")
            };
            let validate_ownership = ownership;
            let validate = match self.protected_decision {
                Some((decision_round, proposal_round, decision_subject, execution_commitment))
                    if proposal_round == round && decision_subject == subject =>
                {
                    stored.project_validate_after_durable_decision(
                        effect.clone(),
                        validate_ownership.clone(),
                        decision_round,
                        proposal_round,
                        decision_subject,
                        execution_commitment,
                    )
                }
                Some(_) => {
                    unreachable!("a retained Decision Validate has the protected genesis body key")
                }
                None => stored.project_validate(effect.clone(), validate_ownership.clone()),
            };
            let validate = match validate {
                Ok(validate) => validate,
                Err(error) => {
                    let previous = self.authenticated_genesis_replay.insert(
                        key,
                        AuthenticatedGenesisReplayStageV1::Stored {
                            replay: error.into_stored(),
                            ownership: store_ownership,
                        },
                    );
                    debug_assert!(previous.is_none());
                    return Err(EffectExecutorError::Contract(
                        "authenticated-genesis Store could not project its Validate successor"
                            .to_owned(),
                    ));
                }
            };
            return self.install_pending_durable_validate_admission(
                key,
                &effect,
                &validate_ownership,
                validate.into_pending_durable_validate_admission(),
            );
        }
        match self.remote_proposal_replay.get(&key) {
            Some(RemoteProposalReplayStageV1::Fetch { .. })
            | Some(RemoteProposalReplayStageV1::BodyAvailable(_))
            | Some(RemoteProposalReplayStageV1::StoreAdmission(_))
            | Some(RemoteProposalReplayStageV1::Store { .. }) => {
                return Err(EffectExecutorError::Contract(
                    "Proposal ValidateBody preceded its exact durable Store replay stage"
                        .to_owned(),
                ));
            }
            Some(RemoteProposalReplayStageV1::Stored {
                replay,
                ownership: store_ownership,
            }) => {
                if !replay.exactly_retains_owned_store(&receipt, store_ownership) {
                    return Err(EffectExecutorError::Contract(
                        "Proposal ValidateBody changed its durable Store lineage".to_owned(),
                    ));
                }
            }
            None => {
                return Err(EffectExecutorError::Contract(
                    "ValidateBody omitted its mandatory lifecycle replay owner".to_owned(),
                ));
            }
        }
        let authority_certificate =
            self.exact_remote_proposal_validate_authority_certificate(&effect, &ownership)?;
        let Some(RemoteProposalReplayStageV1::Stored {
            replay: stored,
            ownership: store_ownership,
        }) = self.remote_proposal_replay.remove(&key)
        else {
            unreachable!("preflighted Proposal Store replay remains installed")
        };
        let validate_ownership = ownership;
        let validate = match self.protected_decision {
            Some((decision_round, proposal_round, decision_subject, execution_commitment))
                if proposal_round == round && decision_subject == subject =>
            {
                let decision_certificate = authority_certificate.as_ref().filter(|certificate| {
                    certificate.phase == wire::GlobalPhase::Commit
                        && certificate.round == decision_round
                        && certificate.proposal_round == proposal_round
                        && certificate.subject == decision_subject
                        && certificate.execution_commitment == execution_commitment
                });
                let Some(decision_certificate) = decision_certificate else {
                    let previous = self.remote_proposal_replay.insert(
                        key,
                        RemoteProposalReplayStageV1::Stored {
                            replay: stored,
                            ownership: store_ownership,
                        },
                    );
                    debug_assert!(previous.is_none());
                    return Err(EffectExecutorError::Contract(
                        "Decision-refined Proposal Validate lost its exact CommitQC".to_owned(),
                    ));
                };
                stored.project_validate_after_durable_decision(
                    effect.clone(),
                    validate_ownership.clone(),
                    decision_certificate,
                )
            }
            Some(_) => unreachable!("a retained Decision Validate has the protected body key"),
            None => stored.project_validate(
                effect.clone(),
                validate_ownership.clone(),
                authority_certificate.as_ref(),
            ),
        };
        let validate = match validate {
            Ok(validate) => validate,
            Err(error) => {
                let previous = self.remote_proposal_replay.insert(
                    key,
                    RemoteProposalReplayStageV1::Stored {
                        replay: error.into_stored(),
                        ownership: store_ownership,
                    },
                );
                debug_assert!(previous.is_none());
                return Err(EffectExecutorError::Contract(
                    "Proposal Store replay could not project its exact Validate successor"
                        .to_owned(),
                ));
            }
        };
        self.install_pending_durable_validate_admission(
            key,
            &effect,
            &validate_ownership,
            validate.into_pending_durable_validate_admission(),
        )
    }
}

impl V2EffectExecutor<SerializedV2Runtime> {
    /// Seal the exact pending certified Fetch for its first durable lifecycle admission.
    ///
    /// The authenticated response supplies a canonical manifest even when the
    /// certificate-backed Fetch began manifest-less. No executor owner is
    /// retired here; the returned replay-bound carrier is consumed only by the
    /// coordinator/registry/LedgerV1 admission transaction.
    pub(in crate::sumeragi) fn prepare_lifecycle_certified_fetch_admission(
        &self,
        candidate: &CertifiedResponsePriorityCandidate,
        response: &wire::CertifiedBodyResponse,
        active_context: LifecycleContext,
        verified: &VerifiedHeightContext,
    ) -> Result<
        super::v2_lifecycle_coordinator::PreparedCertifiedFetchAdmissionV1,
        EffectTransportError,
    > {
        self.validate_lifecycle_ingress_selector_authority()?;
        if !candidate.matches_authenticated_response(response, &candidate.authenticated_responder)
            || candidate.response_hash != HashOf::new(response)
            || candidate.canonical_manifest_hash != HashOf::new(&response.manifest)
        {
            return Err(EffectTransportError::BodyMismatch(
                "certified Fetch admission differs from fresh selector authority",
            ));
        }
        let pending = self
            .pending_fetches
            .get(&candidate.work_id)
            .ok_or(EffectTransportError::UnknownWork(candidate.work_id))?;
        if pending.task.id() != candidate.work_id
            || pending.request_hash != Some(candidate.request_hash)
            || pending.task.certified_request().map(HashOf::new) != Some(candidate.request_hash)
            || pending.task.round != candidate.round
            || pending.task.subject != candidate.subject
            || !pending
                .task
                .matches_reconstructed_manifest(&response.manifest)
        {
            return Err(EffectTransportError::BodyMismatch(
                "certified Fetch admission lost its exact pending request owner",
            ));
        }
        let effect = pending.task.adapter_effect();
        let binding = pending
            .task
            .ownership()
            .exact_pending_adapter_effect_binding(&effect)
            .map_err(|_| {
                EffectTransportError::FailClosed(
                    "pending certified Fetch lost its lifecycle binding".to_owned(),
                )
            })?;
        if &binding != candidate.pending_effect_binding() {
            return Err(EffectTransportError::BodyMismatch(
                "certified Fetch admission changed its pending effect binding",
            ));
        }
        super::v2_lifecycle_coordinator::PreparedCertifiedFetchAdmissionV1::prepare(
            active_context,
            verified,
            effect,
            binding,
            response.manifest.clone(),
            candidate.request_hash,
        )
        .map_err(|error| {
            EffectTransportError::FailClosed(format!(
                "pending certified Fetch lifecycle projection failed: {error:?}"
            ))
        })
    }

    /// Preview one ordinary certified Fetch-to-Store reducer transition.
    pub(in crate::sumeragi) fn prepare_certified_fetch_store_adapter(
        &mut self,
        tag: EventTag,
        manifest: &wire::PayloadManifest,
    ) -> Result<super::v2::CertifiedFetchStoreAdapterPreparationV1<'_>, super::v2::AdapterError>
    {
        self.runtime.prepare_certified_fetch_store(tag, manifest)
    }

    /// Preview one ordinary durable Store-to-Validate reducer transition.
    pub(in crate::sumeragi) fn prepare_durable_store_validate_adapter(
        &mut self,
        tag: EventTag,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
        receipt: &DurableBodyReceipt,
    ) -> Result<super::v2::DurableStoreValidateAdapterPreparationV1<'_>, super::v2::AdapterError>
    {
        self.runtime
            .prepare_durable_store_validate(tag, round, subject, receipt)
    }
}
