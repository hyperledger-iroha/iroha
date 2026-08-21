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

/// Inert runtime fingerprint for one replay-authorized Validate after its
/// move-only admission owner transfers into the lifecycle registry.
#[derive(Clone, Debug, PartialEq, Eq)]
struct DurableValidateRetrySealV1 {
    effect: AdapterEffect,
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
        Some(Self {
            effect: effect.clone(),
            ownership: ownership.clone(),
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
            return Err("durable Validate retry seal received another effect stage".to_owned());
        };
        if (incoming_tag != incumbent_tag && !incoming_tag.strictly_advances(*incumbent_tag))
            || incoming_round != incumbent_round
            || incoming_subject != incumbent_subject
            || self
                .ownership
                .exact_pending_adapter_effect_binding(&self.effect)
                .is_err()
        {
            return Err(
                "durable Validate retry changed its exact body, tag, or incumbent owner".to_owned(),
            );
        }
        let ownership = self
            .ownership
            .adopt_incumbent_body_stage_for_retry_or_authority(incoming, effect)?;
        ownership
            .exact_pending_adapter_effect_binding(effect)
            .map_err(|_| "durable Validate retry lost its exact adopted owner".to_owned())?;
        Ok(Self {
            effect: effect.clone(),
            ownership,
        })
    }
}

impl<R: EffectRuntime> V2EffectExecutor<R> {
    /// Return whether every inert Validate retry tombstone is scoped to the
    /// sole decided body which may survive until this per-height executor is
    /// consumed at rollover.
    fn durable_validate_retry_seals_are_finalization_inert(&self) -> bool {
        self.durable_validate_retry_seals.keys().all(|key| {
            self.protected_decision
                .is_some_and(|(_, round, subject, _)| *key == (round, subject))
        })
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
                        ProductionLifecycleOutputAdmissionFailureV1::Registry => {
                            EffectExecutorError::Contract(
                                "lifecycle output registry settlement failed".to_owned(),
                            )
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
        if let Some(seal) = self.durable_validate_retry_seals.get_mut(&key) {
            *seal = seal
                .project_retry(&effect, &ownership)
                .map_err(EffectExecutorError::Contract)?;
            return Ok(());
        }
        let receipt = self.durable_bodies.get(&key).cloned().ok_or_else(|| {
            EffectExecutorError::Contract(
                "ValidateBody has no matching durable body receipt".to_owned(),
            )
        })?;
        if let Some(recovery) = self.pending_tip_recovery.as_ref() {
            if recovery.stage() != PendingKuraApplyRecoveryStage::DeterministicValidation
                || recovery.replay_tag() != tag
                || recovery.durable_round() != round
                || recovery.durable_subject() != subject
                || recovery.durable_receipt() != &receipt
                || self.validated_bodies.get(&key) != Some(recovery.validated_receipt())
            {
                return Err(EffectExecutorError::Contract(
                    "PendingKura ValidateBody changed its exact recovered validation owner"
                        .to_owned(),
                ));
            }
            // The independently fsynced marker is the terminal authority for
            // this closed-ingress recovery stage. The enclosing recovery
            // transition advances to Apply after this exact catalog stutter;
            // it must not mint ordinary Proposal/local-body admission work.
            return Ok(());
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
                stored.project_validate_after_durable_decision(
                    effect.clone(),
                    validate_ownership.clone(),
                    decision_round,
                    proposal_round,
                    decision_subject,
                    execution_commitment,
                )
            }
            Some(_) => unreachable!("a retained Decision Validate has the protected body key"),
            None => stored.project_validate(effect.clone(), validate_ownership.clone()),
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
