/// Opaque adapter-owned half of one fixed Ready Validate preview join.
///
/// Construction requires a non-forgeable registry authority. The wrapper has
/// no accessor or commit surface and can only be retained beside the consumed
/// registry token by the registry-owned join.
#[allow(dead_code)]
#[must_use = "a sealed Ready Validate adapter preview retains its adapter borrow"]
pub(crate) struct SealedReadyDurableValidateAdapterPreview<'a>(
    ReadyDurableValidateAdapterPreviewKind<'a>,
);
/// Closed adapter classifications accepted by the fixed registry join.
#[allow(variant_size_differences, clippy::large_enum_variant)]
#[allow(dead_code)]
enum ReadyDurableValidateAdapterPreviewKind<'a> {
    /// Successful validation is blocked by the current reducer fence.
    ValidatedBusy(PreparedDirectValidationSucceededBusy<'a>),
    /// Successful validation is inactive or idempotently complete.
    ValidatedInactive(PreparedDirectValidationSucceededInactive<'a>),
    /// Successful validation applied without a child effect.
    ValidatedNoEffect(PreparedDirectValidationSucceededNoEffect<'a>),
    /// Successful validation prepared one exact decision application.
    ValidatedApply(PreparedDirectValidationSucceededApply<'a>),
    /// Successful validation prepared one exact safety-WAL request.
    ValidatedPersist(PreparedDirectValidationSucceededPersist<'a>),
    /// Deterministic rejection is blocked by the current reducer fence.
    RejectedBusy(PreparedDirectValidationFailedBusy<'a>),
    /// Deterministic rejection is inactive or idempotently complete.
    RejectedInactive(PreparedDirectValidationFailedInactive<'a>),
    /// Deterministic rejection applied without a child effect.
    RejectedNoEffect(PreparedDirectValidationFailedNoEffect<'a>),
    /// Deterministic rejection prepared one exact invalid-body report.
    RejectedReport(PreparedDirectValidationFailedReport<'a>),
}
/// Closed publication shape retained after the adapter finishes every
/// fallible, non-durable Ready Validate refinement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) enum ReadyDurableValidateAdapterPublicationKind {
    /// Successful validation remains behind the sampled reducer fence.
    ValidatedBusy,
    /// Successful validation was inactive or idempotently complete.
    ValidatedInactive,
    /// Successful validation applied without a child effect.
    ValidatedNoEffect,
    /// Successful validation prepared one exact decision application.
    ValidatedApply,
    /// Successful validation prepared one encoded WAL payload and Sign continuation.
    ValidatedPersist,
    /// Deterministic rejection remains behind the sampled reducer fence.
    RejectedBusy,
    /// Deterministic rejection was inactive or idempotently complete.
    RejectedInactive,
    /// Deterministic rejection applied without a child effect.
    RejectedNoEffect,
    /// Deterministic rejection prepared one exact invalid-body report.
    RejectedReport,
}
/// Opaque, drop-inert adapter publication preflight for one Ready Validate row.
///
/// Every variant retains the exclusive adapter borrow and all staged state.
/// The sole public observation is its closed discriminator; no encoded WAL
/// bytes, reducer event, effect, or installation authority can escape.
#[allow(dead_code)]
#[must_use = "a prepared Ready Validate publication has not published any state"]
pub(crate) struct PreparedReadyDurableValidateAdapterPublication<'a>(
    ReadyDurableValidateAdapterPublicationState<'a>,
);
/// Adapter-owned half of one closed invalid-body replay pre-admission cut.
///
/// The rejected preview, report effect, derived child binding, and canonical
/// runtime evidence remain private and keep the exclusive adapter borrow.
/// Dropping this value publishes no effect and restores no already-published
/// state because the underlying preview is itself mutation-free.
#[allow(dead_code)]
#[must_use = "invalid-body adapter replay authority has not entered pre-admission"]
pub(in crate::sumeragi) struct PreparedInvalidBodyReportAdapterReplay<'a> {
    prepared: PreparedDirectValidationFailedReport<'a>,
    child_pending: PendingRuntimeEffectBinding,
    replay_evidence: InvalidBodyReportReplayEvidenceV1,
}
/// Adapter-owned report publication after canonical replay evidence has minted
/// one mandatory-bound concrete child.
///
/// All reducer and wire-registry mutations remain staged until the fixed
/// LedgerV1 transaction installs the report row through its exclusive
/// parent/child registry reservation.
#[must_use = "prepared invalid-body report adapter publication awaits LedgerV1 fsync"]
pub(in crate::sumeragi) struct PreparedInvalidBodyReportAdapterPublication<'a> {
    adapter: &'a mut SumeragiV2Adapter,
    next_reducer: reducer::Reducer,
    next_registry: WireRegistry,
    event: reducer::Event,
    core_effect: reducer::Effect,
    next_fence_generation: u64,
    registry_work: PreparedLiveValidateReportRegistryWork,
}
/// Adapter-owned validated Apply publication bound to the exact retained live
/// Decision WAL frame and durable Validate predecessor.
#[must_use = "prepared Validate Apply publication awaits lifecycle settlement"]
pub(in crate::sumeragi) struct PreparedReadyDurableValidateApplyPublication<'a> {
    adapter: &'a mut SumeragiV2Adapter,
    next_reducer: reducer::Reducer,
    next_registry: WireRegistry,
    event: reducer::Event,
    core_effect: reducer::Effect,
    next_fence_generation: u64,
    persisted_apply: Option<SealedLiveWalPersistedEffectV1>,
    registry_work: Option<PreparedLiveValidateApplyRegistryWork>,
}
#[cfg_attr(not(test), allow(dead_code))]
impl PreparedReadyDurableValidateAdapterPublication<'_> {
    /// Return only the exact closed publication discriminator.
    pub(crate) const fn kind(&self) -> ReadyDurableValidateAdapterPublicationKind {
        match &self.0 {
            ReadyDurableValidateAdapterPublicationState::ValidatedBusy(_) => {
                ReadyDurableValidateAdapterPublicationKind::ValidatedBusy
            }
            ReadyDurableValidateAdapterPublicationState::ValidatedInactive(_) => {
                ReadyDurableValidateAdapterPublicationKind::ValidatedInactive
            }
            ReadyDurableValidateAdapterPublicationState::ValidatedNoEffect(_) => {
                ReadyDurableValidateAdapterPublicationKind::ValidatedNoEffect
            }
            ReadyDurableValidateAdapterPublicationState::ValidatedApply(_) => {
                ReadyDurableValidateAdapterPublicationKind::ValidatedApply
            }
            ReadyDurableValidateAdapterPublicationState::ValidatedPersist(_) => {
                ReadyDurableValidateAdapterPublicationKind::ValidatedPersist
            }
            ReadyDurableValidateAdapterPublicationState::RejectedBusy(_) => {
                ReadyDurableValidateAdapterPublicationKind::RejectedBusy
            }
            ReadyDurableValidateAdapterPublicationState::RejectedInactive(_) => {
                ReadyDurableValidateAdapterPublicationKind::RejectedInactive
            }
            ReadyDurableValidateAdapterPublicationState::RejectedNoEffect(_) => {
                ReadyDurableValidateAdapterPublicationKind::RejectedNoEffect
            }
            ReadyDurableValidateAdapterPublicationState::RejectedReport(_) => {
                ReadyDurableValidateAdapterPublicationKind::RejectedReport
            }
        }
    }
    /// Return whether this preflight retains the supplied exact emitted child.
    ///
    /// No effect bytes or staged adapter state escape this equality oracle.
    /// Branches which emit no child always return `false`.
    pub(crate) fn matches_exact_successor_effect(&self, effect: &AdapterEffect) -> bool {
        match &self.0 {
            ReadyDurableValidateAdapterPublicationState::ValidatedApply(prepared) => {
                prepared.apply_effect() == effect
            }
            ReadyDurableValidateAdapterPublicationState::ValidatedPersist(prepared) => {
                &prepared.sign_effect == effect
            }
            ReadyDurableValidateAdapterPublicationState::RejectedReport(prepared) => {
                prepared.report_effect() == effect
            }
            ReadyDurableValidateAdapterPublicationState::ValidatedBusy(_)
            | ReadyDurableValidateAdapterPublicationState::ValidatedInactive(_)
            | ReadyDurableValidateAdapterPublicationState::ValidatedNoEffect(_)
            | ReadyDurableValidateAdapterPublicationState::RejectedBusy(_)
            | ReadyDurableValidateAdapterPublicationState::RejectedInactive(_)
            | ReadyDurableValidateAdapterPublicationState::RejectedNoEffect(_) => false,
        }
    }
    /// Return the context and generation retained by the exact `Busy` branch.
    ///
    /// All non-Busy branches return `None`; no staged reducer or registry state
    /// escapes this observation.
    pub(in crate::sumeragi) fn busy_reducer_fence(&self) -> Option<(wire::HeightContextId, u64)> {
        match &self.0 {
            ReadyDurableValidateAdapterPublicationState::ValidatedBusy(prepared) => {
                Some((prepared.context_id(), prepared.generation()))
            }
            ReadyDurableValidateAdapterPublicationState::RejectedBusy(prepared) => {
                Some((prepared.context_id(), prepared.generation()))
            }
            ReadyDurableValidateAdapterPublicationState::ValidatedInactive(_)
            | ReadyDurableValidateAdapterPublicationState::ValidatedNoEffect(_)
            | ReadyDurableValidateAdapterPublicationState::ValidatedApply(_)
            | ReadyDurableValidateAdapterPublicationState::ValidatedPersist(_)
            | ReadyDurableValidateAdapterPublicationState::RejectedInactive(_)
            | ReadyDurableValidateAdapterPublicationState::RejectedNoEffect(_)
            | ReadyDurableValidateAdapterPublicationState::RejectedReport(_) => None,
        }
    }
}
impl<'a> PreparedReadyDurableValidateAdapterPublication<'a> {
    /// Commit an exact inactive or effect-free Validate branch after LedgerV1.
    ///
    /// The fixed registry join proves the branch before durable publication;
    /// reaching another variant here is therefore a violated internal contract.
    pub(in crate::sumeragi) fn commit_no_successor_after_durable_ledger(self) {
        match self.0 {
            ReadyDurableValidateAdapterPublicationState::ValidatedInactive(prepared) => {
                let PreparedDirectValidationSucceededInactive {
                    _adapter: adapter,
                    next_reducer,
                    next_registry,
                    event,
                    disposition,
                    next_fence_generation,
                } = prepared;
                let disposition = match disposition {
                    DirectValidationSucceededInactive::Stutter(
                        DirectValidationSucceededStutter::NoMatchingWork,
                    ) => reducer::StepDisposition::Ignored(reducer::IgnoreReason::NoMatchingWork),
                    DirectValidationSucceededInactive::Stutter(
                        DirectValidationSucceededStutter::Duplicate,
                    ) => reducer::StepDisposition::Ignored(reducer::IgnoreReason::Duplicate),
                    DirectValidationSucceededInactive::Superseded(reason) => {
                        reducer::StepDisposition::Ignored(reason)
                    }
                };
                adapter.reducer = next_reducer;
                adapter.registry = next_registry;
                adapter.reducer_fence_generation = next_fence_generation;
                adapter.record_reducer_outcome(&event, disposition, &[]);
                adapter.log_body_progress(&event, disposition, 0);
            }
            ReadyDurableValidateAdapterPublicationState::ValidatedNoEffect(prepared) => {
                let PreparedDirectValidationSucceededNoEffect {
                    _adapter: adapter,
                    next_reducer,
                    next_registry,
                    event,
                    next_fence_generation,
                } = prepared;
                adapter.reducer = next_reducer;
                adapter.registry = next_registry;
                adapter.reducer_fence_generation = next_fence_generation;
                adapter.record_reducer_outcome(&event, reducer::StepDisposition::Applied, &[]);
                adapter.log_body_progress(&event, reducer::StepDisposition::Applied, 0);
            }
            ReadyDurableValidateAdapterPublicationState::RejectedInactive(prepared) => {
                let PreparedDirectValidationFailedInactive {
                    _adapter: adapter,
                    next_reducer,
                    next_registry,
                    event,
                    disposition,
                    next_fence_generation,
                } = prepared;
                let disposition = match disposition {
                    DirectValidationFailedInactive::Stutter(
                        DirectValidationFailedStutter::NoMatchingWork,
                    ) => reducer::StepDisposition::Ignored(reducer::IgnoreReason::NoMatchingWork),
                    DirectValidationFailedInactive::Stutter(
                        DirectValidationFailedStutter::Duplicate,
                    ) => reducer::StepDisposition::Ignored(reducer::IgnoreReason::Duplicate),
                    DirectValidationFailedInactive::Superseded(reason) => {
                        reducer::StepDisposition::Ignored(reason)
                    }
                };
                adapter.reducer = next_reducer;
                adapter.registry = next_registry;
                adapter.reducer_fence_generation = next_fence_generation;
                adapter.record_reducer_outcome(&event, disposition, &[]);
                adapter.log_body_progress(&event, disposition, 0);
            }
            ReadyDurableValidateAdapterPublicationState::RejectedNoEffect(prepared) => {
                let PreparedDirectValidationFailedNoEffect {
                    _adapter: adapter,
                    next_reducer,
                    next_registry,
                    event,
                    next_fence_generation,
                } = prepared;
                adapter.reducer = next_reducer;
                adapter.registry = next_registry;
                adapter.reducer_fence_generation = next_fence_generation;
                adapter.record_reducer_outcome(&event, reducer::StepDisposition::Applied, &[]);
                adapter.log_body_progress(&event, reducer::StepDisposition::Applied, 0);
            }
            ReadyDurableValidateAdapterPublicationState::ValidatedBusy(_)
            | ReadyDurableValidateAdapterPublicationState::ValidatedApply(_)
            | ReadyDurableValidateAdapterPublicationState::ValidatedPersist(_)
            | ReadyDurableValidateAdapterPublicationState::RejectedBusy(_)
            | ReadyDurableValidateAdapterPublicationState::RejectedReport(_) => {
                unreachable!("sealed no-successor transition retained another adapter branch")
            }
        }
    }
    /// Consume only the exact rejected-report branch into canonical replay evidence.
    ///
    /// The caller is the fixed registry join and supplies only values borrowed
    /// from its still-installed completion. Failure returns this publication
    /// unchanged so both authority borrows remain retryable and drop-inert.
    #[allow(clippy::result_large_err)]
    pub(in crate::sumeragi) fn seal_invalid_body_report_replay(
        self,
        validate_origin: DurableValidateReplayEvidenceV1,
        validate_effect: &AdapterEffect,
        validate_pending: &PendingRuntimeEffectBinding,
        receipt: &DurableBodyReceipt,
    ) -> Result<PreparedInvalidBodyReportAdapterReplay<'a>, Self> {
        let Self(state) = self;
        let prepared = match state {
            ReadyDurableValidateAdapterPublicationState::RejectedReport(prepared) => prepared,
            other => return Err(Self(other)),
        };
        let Some(capability) = prepared.registered_prepare_report_capability() else {
            return Err(Self(
                ReadyDurableValidateAdapterPublicationState::RejectedReport(prepared),
            ));
        };
        let report_effect = prepared.report_effect();
        let child_pending = validate_pending
            .project_validate_report_invalid_certified_body_successor(
                validate_effect,
                report_effect,
            )
            .or_else(|| {
                validate_pending
                    .project_validate_report_invalid_certified_body_with_registered_prepare(
                        validate_effect,
                        report_effect,
                        &capability,
                    )
            });
        let Some(child_pending) = child_pending else {
            return Err(Self(
                ReadyDurableValidateAdapterPublicationState::RejectedReport(prepared),
            ));
        };
        let Some(replay_evidence) = DurableValidateReplayEvidenceV1::seal_invalid_body_report(
            capability,
            validate_origin,
            validate_effect,
            validate_pending,
            receipt,
            report_effect,
            &child_pending,
        ) else {
            return Err(Self(
                ReadyDurableValidateAdapterPublicationState::RejectedReport(prepared),
            ));
        };
        let sealed = PreparedInvalidBodyReportAdapterReplay {
            prepared,
            child_pending,
            replay_evidence,
        };
        if !sealed.exactly_matches(validate_effect, validate_pending, receipt) {
            let PreparedInvalidBodyReportAdapterReplay { prepared, .. } = sealed;
            return Err(Self(
                ReadyDurableValidateAdapterPublicationState::RejectedReport(prepared),
            ));
        }
        Ok(sealed)
    }

    /// Bind only the exact validated Apply branch to the registry-minted
    /// predecessor and the adapter's retained Decision-WAL replay seal.
    #[allow(clippy::result_large_err)]
    pub(in crate::sumeragi) fn bind_validate_apply_predecessor(
        self,
        predecessor: ReadyValidateApplyPredecessorAuthority<'_>,
    ) -> Result<PreparedReadyDurableValidateApplyPublication<'a>, Self> {
        let Self(state) = self;
        let PreparedDirectValidationSucceededApply {
            _adapter: adapter,
            next_reducer,
            next_registry,
            event,
            core_effect,
            apply_effect,
            next_fence_generation,
        } = match state {
            ReadyDurableValidateAdapterPublicationState::ValidatedApply(prepared) => prepared,
            other => return Err(Self(other)),
        };
        let Some(persisted) = adapter.pending_live_decision_apply.take() else {
            return Err(Self(
                ReadyDurableValidateAdapterPublicationState::ValidatedApply(
                    PreparedDirectValidationSucceededApply {
                        _adapter: adapter,
                        next_reducer,
                        next_registry,
                        event,
                        core_effect,
                        apply_effect,
                        next_fence_generation,
                    },
                ),
            ));
        };
        let persisted = match predecessor.bind_persisted_apply(persisted, &apply_effect) {
            Ok(persisted) => persisted,
            Err(persisted) => {
                adapter.pending_live_decision_apply = Some(persisted);
                return Err(Self(
                    ReadyDurableValidateAdapterPublicationState::ValidatedApply(
                        PreparedDirectValidationSucceededApply {
                            _adapter: adapter,
                            next_reducer,
                            next_registry,
                            event,
                            core_effect,
                            apply_effect,
                            next_fence_generation,
                        },
                    ),
                ));
            }
        };
        Ok(PreparedReadyDurableValidateApplyPublication {
            adapter,
            next_reducer,
            next_registry,
            event,
            core_effect,
            next_fence_generation,
            persisted_apply: Some(persisted),
            registry_work: None,
        })
    }
}
impl<'a> PreparedInvalidBodyReportAdapterReplay<'a> {
    /// Compare the complete adapter report and canonical replay envelope with
    /// one already-retained Validate origin without exposing either side.
    pub(in crate::sumeragi) fn exactly_matches(
        &self,
        validate_effect: &AdapterEffect,
        validate_pending: &PendingRuntimeEffectBinding,
        receipt: &DurableBodyReceipt,
    ) -> bool {
        let Some(capability) = self.prepared.registered_prepare_report_capability() else {
            return false;
        };
        let report_effect = self.prepared.report_effect();
        let projected = validate_pending
            .project_validate_report_invalid_certified_body_successor(
                validate_effect,
                report_effect,
            )
            .or_else(|| {
                validate_pending
                    .project_validate_report_invalid_certified_body_with_registered_prepare(
                        validate_effect,
                        report_effect,
                        &capability,
                    )
            });
        capability.exactly_matches_report(report_effect)
            && projected.as_ref() == Some(&self.child_pending)
            && self.replay_evidence.exactly_matches(
                validate_effect,
                validate_pending,
                receipt,
                report_effect,
                &self.child_pending,
            )
    }
    /// Project the exact report candidate while this adapter half remains
    /// nested under its registry-owned replay token.
    ///
    /// The transition module's private non-Copy permit prevents any raw report
    /// effect or pending binding from reaching a generic admission path.
    pub(in crate::sumeragi) fn project_invalid_body_report_candidate(
        &self,
        permit: &SealedInvalidBodyReportProjectionPermit,
        verified: &VerifiedHeightContext,
        validate_effect: &AdapterEffect,
        validate_pending: &PendingRuntimeEffectBinding,
        receipt: &DurableBodyReceipt,
    ) -> Result<CandidateAdmission, AdapterEffectAdmissionError> {
        if !self.exactly_matches(validate_effect, validate_pending, receipt) {
            return Err(AdapterEffectAdmissionError::InvalidCarrier);
        }
        self.replay_evidence
            .project_sealed_invalid_body_report_candidate(
                permit,
                verified,
                validate_effect,
                validate_pending,
                receipt,
                self.prepared.report_effect(),
                &self.child_pending,
            )
    }

    /// Consume the sealed replay envelope into closed mandatory-bound registry
    /// work while retaining all staged adapter state.
    #[allow(clippy::result_large_err)]
    pub(in crate::sumeragi) fn prepare_registry_work(
        self,
        permit: LiveValidateReportWorkProjectionPermit,
    ) -> Result<PreparedInvalidBodyReportAdapterPublication<'a>, Self> {
        let Self {
            prepared,
            child_pending,
            replay_evidence,
        } = self;
        let PreparedDirectValidationFailedReport {
            _adapter: adapter,
            next_reducer,
            next_registry,
            event,
            core_effect,
            report_effect,
            next_fence_generation,
        } = prepared;
        let registry_work = match replay_evidence.into_live_validate_report_work(
            permit,
            report_effect,
            child_pending,
        ) {
            Ok(work) => work,
            Err((replay_evidence, report_effect, child_pending)) => {
                return Err(Self {
                    prepared: PreparedDirectValidationFailedReport {
                        _adapter: adapter,
                        next_reducer,
                        next_registry,
                        event,
                        core_effect,
                        report_effect,
                        next_fence_generation,
                    },
                    child_pending,
                    replay_evidence,
                });
            }
        };
        Ok(PreparedInvalidBodyReportAdapterPublication {
            adapter,
            next_reducer,
            next_registry,
            event,
            core_effect,
            next_fence_generation,
            registry_work,
        })
    }
}
impl PreparedInvalidBodyReportAdapterPublication<'_> {
    /// Match the bound child against the staged lifecycle coordinates.
    pub(in crate::sumeragi) fn registry_work_matches(
        &self,
        owner: super::v2_lifecycle_coordinator::OwnerId,
        ordinal: u128,
        slot: super::v2_lifecycle_coordinator::PhysicalSlotId,
        digest: LifecycleDigest,
    ) -> bool {
        self.registry_work
            .validates_publication(owner, ordinal, slot, digest)
    }

    /// Install the prechecked report row and staged reducer state after fsync.
    pub(in crate::sumeragi) fn install_registry_and_commit_adapter(
        self,
        reservation: LiveValidateReportRegistryReservation<'_>,
    ) {
        let Self {
            adapter,
            next_reducer,
            next_registry,
            event,
            core_effect,
            next_fence_generation,
            registry_work,
        } = self;
        reservation.install_live_report(registry_work);
        adapter.reducer = next_reducer;
        adapter.registry = next_registry;
        adapter.reducer_fence_generation = next_fence_generation;
        adapter.record_reducer_outcome(
            &event,
            reducer::StepDisposition::Applied,
            core::slice::from_ref(&core_effect),
        );
        adapter.log_body_progress(&event, reducer::StepDisposition::Applied, 1);
    }
}
impl PreparedReadyDurableValidateApplyPublication<'_> {
    /// Project the exact body-frame-completed WAL Apply candidate.
    pub(in crate::sumeragi) fn project_validate_apply_candidate(
        &self,
        permit: &SealedValidateApplyProjectionPermit,
        verified: &VerifiedHeightContext,
        receipt: &DurableBodyReceipt,
    ) -> Result<CandidateAdmission, AdapterEffectAdmissionError> {
        self.persisted_apply
            .as_ref()
            .ok_or(AdapterEffectAdmissionError::InvalidCarrier)?
            .project_sealed_validate_apply_candidate(permit, verified, receipt)
    }
    /// Consume the completed WAL seal into closed ordinary Apply registry work.
    #[allow(clippy::result_large_err)]
    pub(in crate::sumeragi) fn prepare_registry_work(
        mut self,
        permit: LiveValidateApplyWorkProjectionPermit,
        receipt: &DurableBodyReceipt,
    ) -> Result<Self, Self> {
        if self.registry_work.is_some() {
            return Err(self);
        }
        let persisted = self
            .persisted_apply
            .take()
            .expect("prepared Validate Apply retains its completed WAL seal");
        match persisted.into_live_validate_apply_work(permit, receipt) {
            Ok(work) => {
                self.registry_work = Some(work);
                Ok(self)
            }
            Err(persisted) => {
                self.persisted_apply = Some(persisted);
                Err(self)
            }
        }
    }
    /// Match the closed Apply row against the staged lifecycle coordinates.
    pub(in crate::sumeragi) fn registry_work_matches(
        &self,
        owner: super::v2_lifecycle_coordinator::OwnerId,
        ordinal: u128,
        slot: super::v2_lifecycle_coordinator::PhysicalSlotId,
        digest: LifecycleDigest,
    ) -> bool {
        self.persisted_apply.is_none()
            && self
                .registry_work
                .as_ref()
                .is_some_and(|work| work.validates_publication(owner, ordinal, slot, digest))
    }
    /// Detach the prevalidated Apply admission for the registry's typed parent join.
    pub(in crate::sumeragi) fn take_registry_work(
        &mut self,
    ) -> Option<PreparedLiveValidateApplyRegistryWork> {
        self.registry_work.take()
    }
    /// Restore an unchanged Apply admission after a pre-fsync registry rejection.
    pub(in crate::sumeragi) fn restore_registry_work(
        &mut self,
        work: PreparedLiveValidateApplyRegistryWork,
    ) {
        assert!(
            self.registry_work.replace(work).is_none(),
            "failed typed Apply join restores one vacant adapter owner"
        );
    }
    /// Install the prechecked Apply row and staged validation transition after fsync.
    pub(in crate::sumeragi) fn install_registry_and_commit_adapter(
        self,
        reservation: LiveValidateApplyRegistryReservation<'_>,
    ) {
        assert!(
            self.registry_work.is_none(),
            "typed Apply admission moved into the pre-fsync registry reservation"
        );
        reservation.install_live_apply();
        self.adapter.reducer = self.next_reducer;
        self.adapter.registry = self.next_registry;
        self.adapter.reducer_fence_generation = self.next_fence_generation;
        self.adapter.record_reducer_outcome(
            &self.event,
            reducer::StepDisposition::Applied,
            core::slice::from_ref(&self.core_effect),
        );
        self.adapter
            .log_body_progress(&self.event, reducer::StepDisposition::Applied, 1);
    }
}
/// Private retained authority for every prepared publication branch.
#[allow(variant_size_differences, clippy::large_enum_variant)]
#[allow(dead_code)]
enum ReadyDurableValidateAdapterPublicationState<'a> {
    ValidatedBusy(PreparedDirectValidationSucceededBusy<'a>),
    ValidatedInactive(PreparedDirectValidationSucceededInactive<'a>),
    ValidatedNoEffect(PreparedDirectValidationSucceededNoEffect<'a>),
    ValidatedApply(PreparedDirectValidationSucceededApply<'a>),
    ValidatedPersist(PreparedReadyDurableValidatePersistPublication<'a>),
    RejectedBusy(PreparedDirectValidationFailedBusy<'a>),
    RejectedInactive(PreparedDirectValidationFailedInactive<'a>),
    RejectedNoEffect(PreparedDirectValidationFailedNoEffect<'a>),
    RejectedReport(PreparedDirectValidationFailedReport<'a>),
}
/// Purely preflighted `Persist -> Persisted -> Sign` branch.
///
/// The encoded payload is retained but unappended. The reducer and registry are
/// the exact post-acknowledgement clones, so the sole atomic consumer can order
/// WAL sync before ledger publication without another reducer call.
#[allow(dead_code)]
struct PreparedReadyDurableValidatePersistPublication<'a> {
    _adapter: &'a mut SumeragiV2Adapter,
    next_reducer: reducer::Reducer,
    next_registry: WireRegistry,
    validation_event: reducer::Event,
    persist_effect: reducer::Effect,
    expected_wal_sequence: u64,
    encoded_wal_payload: Vec<u8>,
    persisted_event: reducer::Event,
    sign_core_effect: reducer::Effect,
    sign_effect: AdapterEffect,
    registered_prepare: Option<RegisteredPrepareValidateSignCapability>,
    next_fence_generation: u64,
}
// READY_DURABLE_VALIDATE_LIVE_SIGN_BEGIN
/// Pre-WAL Ready-Validate vote publication bound to the exact installed
/// Validate predecessor without exposing either pending binding.
#[must_use = "a bound Validate Sign intent has not reached the safety WAL"]
#[allow(dead_code)]
pub(in crate::sumeragi) struct PreparedReadyDurableValidateBoundSignPublication<'a> {
    prepared: PreparedReadyDurableValidatePersistPublication<'a>,
    child_pending: PendingRuntimeEffectBinding,
}
/// Post-fsync Ready-Validate vote authority.
///
/// The reducer, registry, and fence remain staged until the lifecycle-ledger
/// publication commits. The live adapter retains only the armed
/// persistence identifier for the fsynced frame. Dropping this token cannot
/// undo its WAL append, so it permanently closes the borrowed adapter and
/// requires restart.
#[must_use = "a persisted Validate Sign still requires lifecycle-ledger publication"]
#[allow(dead_code)]
pub(in crate::sumeragi) struct PreparedReadyDurableValidatePersistedSign<'a> {
    adapter: &'a mut SumeragiV2Adapter,
    next_reducer: Option<reducer::Reducer>,
    next_registry: Option<WireRegistry>,
    committed_status: Option<wire::SumeragiV2Status>,
    validation_event: Option<reducer::Event>,
    persist_effect: Option<reducer::Effect>,
    persisted_event: Option<reducer::Event>,
    sign_core_effect: Option<reducer::Effect>,
    persisted_sign: Option<SealedLiveWalPersistedEffectV1>,
    registry_work: Option<PreparedLiveValidateSignRegistryWork>,
    next_fence_generation: u64,
    armed: bool,
}
/// Opaque ownership-retaining failure from the real live-WAL Sign append.
///
/// A pre-WAL contract mismatch retains the complete bound token. Any append
/// attempt instead retains the adapter borrow in a permanently fail-closed
/// state because the filesystem result may be durable or ambiguous.
#[must_use = "a failed Validate Sign append still owns its sealed adapter authority"]
#[allow(dead_code)]
pub(in crate::sumeragi) struct ReadyDurableValidateSignWalError<'a> {
    failure: ReadyDurableValidateSignWalFailure<'a>,
}
#[allow(variant_size_differences, clippy::large_enum_variant)]
#[allow(dead_code)]
enum ReadyDurableValidateSignWalFailure<'a> {
    PreWal {
        _prepared: PreparedReadyDurableValidateBoundSignPublication<'a>,
    },
    PostWal {
        _adapter: &'a mut SumeragiV2Adapter,
        _error: AdapterError,
    },
}
impl Drop for PreparedReadyDurableValidatePersistedSign<'_> {
    fn drop(&mut self) {
        if self.armed {
            self.adapter.fail_closed = true;
        }
    }
}
impl PreparedReadyDurableValidatePersistedSign<'_> {
    fn post_wal_publication_is_exact(&self) -> bool {
        let (
            Some(next_reducer),
            Some(_next_registry),
            Some(validation_event),
            Some(persist_effect),
            Some(persisted_event),
            Some(sign_core_effect),
            Some(persisted_sign),
        ) = (
            self.next_reducer.as_ref(),
            self.next_registry.as_ref(),
            self.validation_event.as_ref(),
            self.persist_effect.as_ref(),
            self.persisted_event.as_ref(),
            self.sign_core_effect.as_ref(),
            self.persisted_sign.as_ref(),
        )
        else {
            return false;
        };
        let reducer::Event::ValidationCompleted {
            tag: validation_tag,
            valid: true,
            ..
        } = validation_event
        else {
            return false;
        };
        let reducer::Effect::Persist {
            tag: persist_tag,
            entry,
        } = persist_effect
        else {
            return false;
        };
        let reducer::Event::Persisted {
            tag: persisted_tag,
            id,
        } = persisted_event
        else {
            return false;
        };
        let reducer::Effect::Sign {
            tag: sign_tag,
            message,
        } = sign_core_effect
        else {
            return false;
        };
        self.armed
            && !self.adapter.fail_closed
            && self.adapter.pending_persistence_id == Some(entry.id().get())
            && self.committed_status.as_ref().is_some_and(|status| {
                status.height_context_id == self.adapter.wire_context.id()
                    && status.height == self.adapter.wire_context.height
                    && status.pending_persistence_id.is_none()
                    && !status.restart_required
            })
            && self.registry_work.is_none()
            && persisted_sign.exactly_binds_validate_sign_pending()
            && validation_tag == persist_tag
            && persist_tag == persisted_tag
            && persisted_tag == sign_tag
            && *id == entry.id()
            && next_reducer.pending_persistence_record().is_none()
            && next_reducer.awaiting_signature() == Some(message)
    }
    /// Project the already-authorized post-WAL child without repeating pending
    /// refinement or exposing effect/replay parts.
    pub(in crate::sumeragi) fn project_validate_sign_candidate(
        &self,
        permit: &SealedValidateSignProjectionPermit,
        verified: &VerifiedHeightContext,
    ) -> Result<CandidateAdmission, AdapterEffectAdmissionError> {
        if !self.post_wal_publication_is_exact() {
            return Err(AdapterEffectAdmissionError::InvalidCarrier);
        }
        self.persisted_sign
            .as_ref()
            .expect("exact post-WAL publication retains its Sign seal")
            .project_sealed_validate_sign_candidate(permit, verified)
    }
    /// Consume the nested Sign seal into one closed ordinary registry carrier.
    ///
    /// Failure returns the complete armed token. Success still publishes
    /// nothing and is safe to retain across the subsequent LedgerV1 fsync.
    #[allow(clippy::result_large_err)]
    #[inline(never)]
    pub(in crate::sumeragi) fn prepare_registry_work(
        mut self: Box<Self>,
        permit: LiveValidateSignWorkProjectionPermit,
    ) -> Result<Box<Self>, Box<Self>> {
        if !self.post_wal_publication_is_exact() {
            return Err(self);
        }
        let persisted = self
            .persisted_sign
            .take()
            .expect("exact post-WAL publication retains its Sign seal");
        match persisted.into_live_validate_sign_work(permit) {
            Ok(work) => {
                let valid = work.validates_exact();
                self.registry_work = Some(work);
                if valid { Ok(self) } else { Err(self) }
            }
            Err(persisted) => {
                self.persisted_sign = Some(persisted);
                Err(self)
            }
        }
    }
    /// Match the closed prepared work against the staged coordinator digest.
    pub(in crate::sumeragi) fn registry_work_matches(
        &self,
        owner: super::v2_lifecycle_coordinator::OwnerId,
        ordinal: u128,
        slot: super::v2_lifecycle_coordinator::PhysicalSlotId,
        digest: LifecycleDigest,
    ) -> bool {
        self.armed
            && self.persisted_sign.is_none()
            && self
                .registry_work
                .as_ref()
                .is_some_and(|work| work.validates_publication(owner, ordinal, slot, digest))
    }
    /// Publish the prechecked registry child and staged adapter state after
    /// exact LedgerV1 fsync.
    ///
    /// No fallible operation is permitted here. The caller has already swapped
    /// the staged lifecycle coordinator, while the retained exclusive registry
    /// reservation proves the parent absent and child vacant.
    #[inline(never)]
    pub(in crate::sumeragi) fn install_registry_and_commit_adapter(
        mut self: Box<Self>,
        reservation: Box<LiveValidateSignRegistryReservation<'_>>,
    ) {
        let work = self
            .registry_work
            .take()
            .expect("pre-fsync publication retains one exact ordinary Sign carrier");
        let next_reducer = self
            .next_reducer
            .take()
            .expect("pre-fsync publication retains staged reducer state");
        let next_registry = self
            .next_registry
            .take()
            .expect("pre-fsync publication retains staged wire registry state");
        let committed_status = self
            .committed_status
            .take()
            .expect("pre-fsync publication retains its exact committed status");
        let validation_event = self
            .validation_event
            .take()
            .expect("pre-fsync publication retains ValidationCompleted");
        let persist_effect = self
            .persist_effect
            .take()
            .expect("pre-fsync publication retains its Persist effect");
        let persisted_event = self
            .persisted_event
            .take()
            .expect("pre-fsync publication retains its Persisted event");
        let sign_core_effect = self
            .sign_core_effect
            .take()
            .expect("pre-fsync publication retains its Sign effect");
        let reservation = *reservation;
        work.install_into(reservation);
        self.adapter.reducer = next_reducer;
        self.adapter.registry = next_registry;
        self.adapter.pending_persistence_id = None;
        self.adapter.reducer_fence_generation = self.next_fence_generation;
        self.adapter.record_reducer_outcome(
            &validation_event,
            reducer::StepDisposition::Applied,
            core::slice::from_ref(&persist_effect),
        );
        self.adapter
            .log_body_progress(&validation_event, reducer::StepDisposition::Applied, 1);
        self.adapter.record_reducer_outcome(
            &persisted_event,
            reducer::StepDisposition::Applied,
            core::slice::from_ref(&sign_core_effect),
        );
        self.armed = false;
        if self.adapter.status_publication_enabled {
            super::status::set_v2_status(committed_status);
        }
    }
}
#[cfg(test)]
impl PreparedReadyDurableValidatePersistedSign<'_> {
    /// Install the already-fsynced staged adapter state after a test-owned
    /// durable-ledger settlement.
    ///
    /// Production must use `install_registry_and_commit_adapter`; this seam
    /// exists only for adapter-unit fixtures whose scope deliberately excludes
    /// the lifecycle registry while still exercising the sealed preview and
    /// real WAL append path.
    fn commit_after_test_durable_ledger_settlement(mut self: Box<Self>) {
        assert!(self.post_wal_publication_is_exact());
        assert!(self.registry_work.is_none());
        let next_reducer = self
            .next_reducer
            .take()
            .expect("test settlement retains staged reducer state");
        let next_registry = self
            .next_registry
            .take()
            .expect("test settlement retains staged registry state");
        let committed_status = self
            .committed_status
            .take()
            .expect("test settlement retains exact committed status");
        let validation_event = self
            .validation_event
            .take()
            .expect("test settlement retains ValidationCompleted");
        let persist_effect = self
            .persist_effect
            .take()
            .expect("test settlement retains its Persist effect");
        let persisted_event = self
            .persisted_event
            .take()
            .expect("test settlement retains its Persisted event");
        let sign_core_effect = self
            .sign_core_effect
            .take()
            .expect("test settlement retains its Sign effect");
        let _persisted_sign = self
            .persisted_sign
            .take()
            .expect("test settlement consumes the sealed Sign continuation");
        self.adapter.reducer = next_reducer;
        self.adapter.registry = next_registry;
        self.adapter.pending_persistence_id = None;
        self.adapter.reducer_fence_generation = self.next_fence_generation;
        self.adapter.record_reducer_outcome(
            &validation_event,
            reducer::StepDisposition::Applied,
            core::slice::from_ref(&persist_effect),
        );
        self.adapter
            .log_body_progress(&validation_event, reducer::StepDisposition::Applied, 1);
        self.adapter.record_reducer_outcome(
            &persisted_event,
            reducer::StepDisposition::Applied,
            core::slice::from_ref(&sign_core_effect),
        );
        self.armed = false;
        if self.adapter.status_publication_enabled {
            super::status::set_v2_status(committed_status);
        }
    }
}
#[cfg(test)]
impl SumeragiV2Adapter {
    /// Settle one successful Ready Validate through the sealed WAL path for
    /// runtime-ordering tests which deliberately do not construct a lifecycle
    /// registry owner.
    pub(crate) fn settle_ready_validate_succeeded_for_runtime_test(
        &mut self,
        tag: reducer::EventTag,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
        validated: &ValidatedBodyReceipt,
    ) -> Vec<AdapterEffect> {
        let preview = self
            .prepare_direct_validation_succeeded(tag, round, subject, validated)
            .expect("prepare the exact sealed successful-validation transition");
        let sealed = SealedReadyDurableValidateAdapterPreview(match preview {
            DirectValidationSucceededPreparation::Busy(prepared) => {
                ReadyDurableValidateAdapterPreviewKind::ValidatedBusy(prepared)
            }
            DirectValidationSucceededPreparation::Inactive(prepared) => {
                ReadyDurableValidateAdapterPreviewKind::ValidatedInactive(prepared)
            }
            DirectValidationSucceededPreparation::NoEffect(prepared) => {
                ReadyDurableValidateAdapterPreviewKind::ValidatedNoEffect(prepared)
            }
            DirectValidationSucceededPreparation::Apply(prepared) => {
                ReadyDurableValidateAdapterPreviewKind::ValidatedApply(prepared)
            }
            DirectValidationSucceededPreparation::Persist(prepared) => {
                ReadyDurableValidateAdapterPreviewKind::ValidatedPersist(prepared)
            }
        });
        let publication = sealed
            .preflight_publication()
            .expect("preflight the sealed successful-validation publication");
        match publication.kind() {
            ReadyDurableValidateAdapterPublicationKind::ValidatedInactive
            | ReadyDurableValidateAdapterPublicationKind::ValidatedNoEffect => {
                publication.commit_no_successor_after_durable_ledger();
                Vec::new()
            }
            ReadyDurableValidateAdapterPublicationKind::ValidatedPersist => {
                let ReadyDurableValidateAdapterPublicationState::ValidatedPersist(prepared) =
                    &publication.0
                else {
                    unreachable!("Persist discriminator retains one exact publication")
                };
                let sign = prepared.sign_effect.clone();
                let store = AdapterEffect::StoreBody {
                    tag,
                    round,
                    subject,
                };
                let validate = AdapterEffect::ValidateBody {
                    tag,
                    round,
                    subject,
                };
                let owner = super::v2_runtime::bind_adapter_effect_batch_ownership(
                    core::slice::from_ref(&store),
                    vec![super::v2_runtime::RuntimeEffectOwnership::fresh_for_test(
                        tag,
                        u128::from(round.view).saturating_add(90_001),
                    )],
                )
                .expect("bind one ordinary Store owner")
                .pop()
                .expect("one ordinary Store owner");
                let store_pending = owner
                    .exact_pending_adapter_effect_binding(&store)
                    .expect("bind ordinary Store predecessor");
                let validate_pending = store_pending
                    .project_store_validate_successor(&store, &validate)
                    .expect("project ordinary Validate predecessor");
                let bound = publication
                    .bind_validate_sign_predecessor(
                        ReadyValidateSignPredecessorAuthority::for_test(
                            &validate,
                            &validate_pending,
                        ),
                    )
                    .unwrap_or_else(|_| panic!("bind the exact sealed Validate predecessor"));
                let persisted = bound
                    .append_live_wal()
                    .unwrap_or_else(|_| panic!("append and fsync the exact Validate WAL frame"));
                Box::new(persisted).commit_after_test_durable_ledger_settlement();
                vec![sign]
            }
            ReadyDurableValidateAdapterPublicationKind::ValidatedBusy => {
                panic!("runtime test settlement cannot bypass a live reducer fence")
            }
            ReadyDurableValidateAdapterPublicationKind::ValidatedApply => {
                panic!("runtime test settlement requires the dedicated Apply fixture")
            }
            ReadyDurableValidateAdapterPublicationKind::RejectedBusy
            | ReadyDurableValidateAdapterPublicationKind::RejectedInactive
            | ReadyDurableValidateAdapterPublicationKind::RejectedNoEffect
            | ReadyDurableValidateAdapterPublicationKind::RejectedReport => {
                unreachable!("successful validation cannot produce a rejected publication")
            }
        }
    }
}
impl<'a> PreparedReadyDurableValidateAdapterPublication<'a> {
    /// Bind only a `ValidatedPersist` branch to the registry-minted exact
    /// Validate predecessor. A wrong branch or failed refinement returns this
    /// whole publication unchanged and performs no WAL I/O.
    #[allow(clippy::result_large_err)]
    pub(in crate::sumeragi) fn bind_validate_sign_predecessor(
        self,
        predecessor: ReadyValidateSignPredecessorAuthority<'_>,
    ) -> Result<PreparedReadyDurableValidateBoundSignPublication<'a>, Self> {
        let Self(state) = self;
        let prepared = match state {
            ReadyDurableValidateAdapterPublicationState::ValidatedPersist(prepared) => prepared,
            other => return Err(Self(other)),
        };
        match prepared.bind_validate_sign_predecessor(predecessor) {
            Ok(bound) => Ok(bound),
            Err(prepared) => Err(Self(
                ReadyDurableValidateAdapterPublicationState::ValidatedPersist(prepared),
            )),
        }
    }
}
impl<'a> PreparedReadyDurableValidatePersistPublication<'a> {
    fn bind_validate_sign_predecessor(
        self,
        predecessor: ReadyValidateSignPredecessorAuthority<'_>,
    ) -> Result<PreparedReadyDurableValidateBoundSignPublication<'a>, Self> {
        let Some(child_pending) =
            predecessor.project_successor(&self.sign_effect, self.registered_prepare.as_ref())
        else {
            return Err(self);
        };
        if !child_pending.exactly_binds_adapter_effect(&self.sign_effect) {
            return Err(self);
        }
        Ok(PreparedReadyDurableValidateBoundSignPublication {
            prepared: self,
            child_pending,
        })
    }
}
#[allow(dead_code)]
impl<'a> PreparedReadyDurableValidateBoundSignPublication<'a> {
    fn pre_wal_is_exact(&self) -> bool {
        let prepared = &self.prepared;
        let reducer::Effect::Persist { entry, .. } = &prepared.persist_effect else {
            return false;
        };
        !prepared._adapter.fail_closed
            && prepared._adapter.replay_complete
            && prepared._adapter.pending_persistence_id.is_none()
            && prepared
                ._adapter
                .reducer
                .pending_persistence_record()
                .is_none()
            && prepared._adapter.reducer.awaiting_signature().is_none()
            && prepared.next_reducer.pending_persistence_record().is_none()
            && prepared.child_pending_is_exact(&self.child_pending)
            && prepared
                ._adapter
                .wal
                .recovered_records()
                .last()
                .map_or(prepared.expected_wal_sequence == 0, |record| {
                    record.sequence().checked_add(1) == Some(prepared.expected_wal_sequence)
                })
            && prepared.expected_wal_sequence.checked_add(1) == Some(entry.id().get())
    }
    /// Append and fsync the exact pre-encoded WAL payload, then mint the live
    /// frame identity solely from the returned receipt and retained frame.
    #[allow(clippy::result_large_err, clippy::too_many_lines)]
    pub(in crate::sumeragi) fn append_live_wal(
        self,
    ) -> Result<PreparedReadyDurableValidatePersistedSign<'a>, ReadyDurableValidateSignWalError<'a>>
    {
        if !self.pre_wal_is_exact() {
            iroha_logger::error!("Ready Validate Sign append rejected pre-WAL exactness");
            return Err(ReadyDurableValidateSignWalError {
                failure: ReadyDurableValidateSignWalFailure::PreWal { _prepared: self },
            });
        }
        let Self {
            prepared,
            child_pending,
        } = self;
        let PreparedReadyDurableValidatePersistPublication {
            _adapter: adapter,
            mut next_reducer,
            mut next_registry,
            validation_event,
            persist_effect,
            expected_wal_sequence,
            encoded_wal_payload,
            persisted_event,
            sign_core_effect,
            sign_effect,
            registered_prepare: _,
            next_fence_generation,
        } = prepared;
        let reducer::Effect::Persist { entry, .. } = &persist_effect else {
            unreachable!("pre-WAL exactness retained one Persist effect")
        };
        let persistence_id = entry.id().get();
        adapter.pending_persistence_id = Some(persistence_id);
        let receipt = match adapter.wal.append(&encoded_wal_payload) {
            Ok(receipt) => receipt,
            Err(error) => {
                iroha_logger::error!(?error, "Ready Validate Sign WAL append failed");
                adapter.fail_closed = true;
                return Err(ReadyDurableValidateSignWalError {
                    failure: ReadyDurableValidateSignWalFailure::PostWal {
                        _adapter: adapter,
                        _error: error.into(),
                    },
                });
            }
        };
        // TODO: Add a test-only crash injection immediately after this
        // successful append receipt to exercise the restart-only ambiguous
        // boundary without weakening the production WAL API.
        let frame_sequence = receipt.sequence();
        let frame_hash = receipt.frame_hash();
        let post_wal: Result<SealedLiveWalPersistedEffectV1, AdapterError> = (|| {
            let frame = adapter.wal.recovered_records().last().ok_or(
                AdapterError::WalFrameIdentityMismatch {
                    frame_sequence,
                    persistence_id,
                    frame_hash,
                },
            )?;
            if frame_sequence != expected_wal_sequence
                || frame.payload() != encoded_wal_payload.as_slice()
            {
                return Err(AdapterError::WalFrameIdentityMismatch {
                    frame_sequence,
                    persistence_id,
                    frame_hash,
                });
            }
            let wal_identity =
                LiveWalFrameIdentity::from_append_receipt(frame, receipt, persistence_id).ok_or(
                    AdapterError::WalFrameIdentityMismatch {
                        frame_sequence,
                        persistence_id,
                        frame_hash,
                    },
                )?;
            let wal_pending = PendingRuntimeEffectBinding::from_exact_live_wal_append(
                &wal_identity,
                &sign_effect,
            )
            .ok_or(AdapterError::LiveWalReplayCauseMismatch)?;
            let persisted_sign = SealedLiveWalPersistedEffectV1::from_exact_live_append(
                ExactLiveWalPersistedContinuationCause::PayloadFree {
                    wal_identity,
                    effect: sign_effect,
                    pending: wal_pending,
                },
            )
            .ok_or(AdapterError::LiveWalReplayCauseMismatch)?;
            persisted_sign
                .bind_exact_validate_sign_pending(child_pending)
                .map_err(|_| AdapterError::LiveWalReplayCauseMismatch)
        })();
        let persisted_sign = match post_wal {
            Ok(persisted_sign) => persisted_sign,
            Err(error) => {
                iroha_logger::error!(?error, "Ready Validate Sign post-WAL seal failed");
                adapter.fail_closed = true;
                return Err(ReadyDurableValidateSignWalError {
                    failure: ReadyDurableValidateSignWalFailure::PostWal {
                        _adapter: adapter,
                        _error: error,
                    },
                });
            }
        };
        debug_assert!(persisted_sign.exactly_binds_validate_sign_pending());
        // Build the exact post-commit status before LifecycleLedgerV1 can
        // advance. Temporarily installing the already-preflighted reducer and
        // registry is publication-inert: the old adapter state and armed WAL
        // marker are restored before this block returns. A conversion failure
        // is post-WAL and therefore closes the adapter for restart.
        let armed_persistence_id = adapter.pending_persistence_id.take();
        core::mem::swap(&mut adapter.reducer, &mut next_reducer);
        core::mem::swap(&mut adapter.registry, &mut next_registry);
        let prior_last_progress = adapter.last_progress;
        adapter.record_reducer_outcome(
            &validation_event,
            reducer::StepDisposition::Applied,
            core::slice::from_ref(&persist_effect),
        );
        adapter.record_reducer_outcome(
            &persisted_event,
            reducer::StepDisposition::Applied,
            core::slice::from_ref(&sign_core_effect),
        );
        let committed_status = adapter.status();
        adapter.last_progress = prior_last_progress;
        core::mem::swap(&mut adapter.registry, &mut next_registry);
        core::mem::swap(&mut adapter.reducer, &mut next_reducer);
        adapter.pending_persistence_id = armed_persistence_id;
        let committed_status = match committed_status {
            Ok(status) => status,
            Err(error) => {
                iroha_logger::error!(?error, "Ready Validate Sign post-WAL status failed");
                adapter.fail_closed = true;
                return Err(ReadyDurableValidateSignWalError {
                    failure: ReadyDurableValidateSignWalFailure::PostWal {
                        _adapter: adapter,
                        _error: error,
                    },
                });
            }
        };
        Ok(PreparedReadyDurableValidatePersistedSign {
            adapter,
            next_reducer: Some(next_reducer),
            next_registry: Some(next_registry),
            committed_status: Some(committed_status),
            validation_event: Some(validation_event),
            persist_effect: Some(persist_effect),
            persisted_event: Some(persisted_event),
            sign_core_effect: Some(sign_core_effect),
            persisted_sign: Some(persisted_sign),
            registry_work: None,
            next_fence_generation,
            armed: true,
        })
    }
}
impl PreparedReadyDurableValidatePersistPublication<'_> {
    fn child_pending_is_exact(&self, pending: &PendingRuntimeEffectBinding) -> bool {
        pending.exactly_binds_adapter_effect(&self.sign_effect)
    }
}
// READY_DURABLE_VALIDATE_LIVE_SIGN_END
impl<'a> SealedReadyDurableValidateAdapterPreview<'a> {
    /// Consume the sealed preview into a fully checked, still-inert publication.
    ///
    /// The Persist branch encodes its exact WAL payload and simulates the matching
    /// acknowledgement on cloned state. No WAL append, live adapter mutation,
    /// lifecycle publication, or externally visible effect occurs.
    pub(crate) fn preflight_publication(
        self,
    ) -> Result<PreparedReadyDurableValidateAdapterPublication<'a>, AdapterError> {
        let state = match self.0 {
            ReadyDurableValidateAdapterPreviewKind::ValidatedBusy(prepared) => {
                ReadyDurableValidateAdapterPublicationState::ValidatedBusy(prepared)
            }
            ReadyDurableValidateAdapterPreviewKind::ValidatedInactive(prepared) => {
                ReadyDurableValidateAdapterPublicationState::ValidatedInactive(prepared)
            }
            ReadyDurableValidateAdapterPreviewKind::ValidatedNoEffect(prepared) => {
                ReadyDurableValidateAdapterPublicationState::ValidatedNoEffect(prepared)
            }
            ReadyDurableValidateAdapterPreviewKind::ValidatedApply(prepared) => {
                ReadyDurableValidateAdapterPublicationState::ValidatedApply(prepared)
            }
            ReadyDurableValidateAdapterPreviewKind::ValidatedPersist(prepared) => {
                ReadyDurableValidateAdapterPublicationState::ValidatedPersist(
                    prepared.preflight_publication()?,
                )
            }
            ReadyDurableValidateAdapterPreviewKind::RejectedBusy(prepared) => {
                ReadyDurableValidateAdapterPublicationState::RejectedBusy(prepared)
            }
            ReadyDurableValidateAdapterPreviewKind::RejectedInactive(prepared) => {
                ReadyDurableValidateAdapterPublicationState::RejectedInactive(prepared)
            }
            ReadyDurableValidateAdapterPreviewKind::RejectedNoEffect(prepared) => {
                ReadyDurableValidateAdapterPublicationState::RejectedNoEffect(prepared)
            }
            ReadyDurableValidateAdapterPreviewKind::RejectedReport(prepared) => {
                ReadyDurableValidateAdapterPublicationState::RejectedReport(prepared)
            }
        };
        Ok(PreparedReadyDurableValidateAdapterPublication(state))
    }
}
impl<'a> PreparedDirectValidationSucceededPersist<'a> {
    fn preflight_publication(
        self,
    ) -> Result<PreparedReadyDurableValidatePersistPublication<'a>, AdapterError> {
        let Self {
            _adapter: adapter,
            mut next_reducer,
            mut next_registry,
            event: validation_event,
            persist_effect,
            next_fence_generation,
        } = self;
        let (tag, entry) = match &persist_effect {
            reducer::Effect::Persist { tag, entry } => (*tag, entry.clone()),
            _ => return Err(AdapterError::ReadyDurableValidatePublicationContractViolation),
        };
        let expected_vote = match entry.record() {
            reducer::WalRecord::PrepareIntent(vote) if vote.phase() == reducer::Phase::Prepare => {
                *vote
            }
            reducer::WalRecord::LockAndCommit { prepare, vote }
                if prepare.phase() == reducer::Phase::Prepare
                    && vote.phase() == reducer::Phase::Commit
                    && prepare.subject() == vote.subject()
                    && prepare.proposal_round() == vote.proposal_round() =>
            {
                *vote
            }
            _ => return Err(AdapterError::ReadyDurableValidatePublicationContractViolation),
        };
        if next_reducer.pending_persistence_record() != Some(entry.record()) {
            return Err(AdapterError::ReadyDurableValidatePublicationContractViolation);
        }
        let expected_wal_sequence = match adapter.wal.recovered_records().last() {
            Some(record) => record
                .sequence()
                .checked_add(1)
                .ok_or(AdapterError::ReadyDurableValidatePublicationContractViolation)?,
            None => 0,
        };
        if expected_wal_sequence.checked_add(1) != Some(entry.id().get()) {
            return Err(AdapterError::ReadyDurableValidatePublicationContractViolation);
        }
        let encoded_wal_payload =
            next_registry.encode_wal_entry(&entry, adapter.aggregator.as_ref())?;
        let pre_ack_fence = ReducerFenceProjection {
            pending_persistence: next_reducer.pending_persistence_record().cloned(),
            awaiting_signature: next_reducer.awaiting_signature().cloned(),
            replay_complete: adapter.replay_complete,
        };
        let persisted_event = reducer::Event::Persisted {
            tag,
            id: entry.id(),
        };
        let continuation = next_reducer.step(persisted_event.clone())?;
        if continuation.disposition() != reducer::StepDisposition::Applied {
            return Err(AdapterError::ReadyDurableValidatePublicationContractViolation);
        }
        let mut continuation_effects = continuation.into_effects();
        if continuation_effects.len() != 1 {
            return Err(AdapterError::ReadyDurableValidatePublicationContractViolation);
        }
        let sign_core_effect = continuation_effects
            .pop()
            .expect("one checked validation persistence continuation remains");
        let sign_effect = match &sign_core_effect {
            reducer::Effect::Sign {
                tag: sign_tag,
                message: reducer::SignableMessage::Vote(vote),
            } if *sign_tag == tag && *vote == expected_vote => AdapterEffect::Sign {
                tag: *sign_tag,
                request: SignRequest::Vote(next_registry.unsigned_vote_to_wire(*vote)?),
            },
            _ => return Err(AdapterError::ReadyDurableValidatePublicationContractViolation),
        };
        let registered_prepare = match entry.record() {
            reducer::WalRecord::PrepareIntent(_) => None,
            reducer::WalRecord::LockAndCommit { .. } => Some(
                RegisteredPrepareValidateSignCapability::from_staged_lock_and_commit(
                    entry.record(),
                    &next_registry,
                    &sign_effect,
                )
                .ok_or(AdapterError::ReadyDurableValidatePublicationContractViolation)?,
            ),
            _ => return Err(AdapterError::ReadyDurableValidatePublicationContractViolation),
        };
        if next_reducer.pending_persistence_record().is_some()
            || next_reducer.awaiting_signature()
                != Some(&reducer::SignableMessage::Vote(expected_vote))
        {
            return Err(AdapterError::ReadyDurableValidatePublicationContractViolation);
        }
        let post_ack_fence = ReducerFenceProjection {
            pending_persistence: next_reducer.pending_persistence_record().cloned(),
            awaiting_signature: next_reducer.awaiting_signature().cloned(),
            replay_complete: adapter.replay_complete,
        };
        let next_fence_generation = if post_ack_fence == pre_ack_fence {
            next_fence_generation
        } else {
            next_fence_generation
                .checked_add(1)
                .filter(|next| *next != u64::MAX)
                .ok_or(AdapterError::ReducerFenceGenerationExhausted)?
        };
        Ok(PreparedReadyDurableValidatePersistPublication {
            _adapter: adapter,
            next_reducer,
            next_registry,
            validation_event,
            persist_effect,
            expected_wal_sequence,
            encoded_wal_payload,
            persisted_event,
            sign_core_effect,
            sign_effect,
            registered_prepare,
            next_fence_generation,
        })
    }
}
