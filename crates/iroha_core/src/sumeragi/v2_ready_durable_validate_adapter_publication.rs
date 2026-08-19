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
    child_pending: Option<PendingRuntimeEffectBinding>,
    replay_evidence: Option<InvalidBodyReportReplayEvidenceV1>,
    registry_work: Option<PreparedInvalidBodyReportRegistryWork>,
}
/// Adapter-owned Decision/Validate Apply replay and staged reducer commit.
#[must_use = "Validate Apply replay has not entered lifecycle publication"]
pub(in crate::sumeragi) struct PreparedDurableValidateApplyAdapterReplay<'a> {
    prepared: PreparedDirectValidationSucceededApply<'a>,
    child_pending: Option<PendingRuntimeEffectBinding>,
    replay_evidence: Option<DurableValidateApplyReplayEvidenceV1>,
    validated_receipt: ValidatedBodyReceipt,
    registry_work: Option<PreparedValidateApplyRegistryWork>,
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
    /// Return the authenticated context and generation only for a Busy branch.
    pub(in crate::sumeragi) const fn busy_fence_identity(
        &self,
    ) -> Option<(wire::HeightContextId, u64)> {
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
}
impl<'a> PreparedReadyDurableValidateAdapterPublication<'a> {
    /// Consume only the validated Apply branch into authenticated Decision-WAL replay.
    #[allow(clippy::result_large_err)]
    pub(in crate::sumeragi) fn seal_decision_validate_apply_replay(
        self,
        validate_origin: DurableValidateReplayEvidenceV1,
        validate_effect: &AdapterEffect,
        validate_pending: &PendingRuntimeEffectBinding,
        receipt: &ValidatedBodyReceipt,
    ) -> Result<PreparedDurableValidateApplyAdapterReplay<'a>, Self> {
        let Self(state) = self;
        let prepared = match state {
            ReadyDurableValidateAdapterPublicationState::ValidatedApply(prepared) => prepared,
            other => return Err(Self(other)),
        };
        let Some(capability) = prepared.authenticated_decision_capability() else {
            return Err(Self(
                ReadyDurableValidateAdapterPublicationState::ValidatedApply(prepared),
            ));
        };
        let apply_effect = prepared.apply_effect();
        let Some(child_pending) =
            validate_pending.project_validate_apply_successor(validate_effect, apply_effect)
        else {
            return Err(Self(
                ReadyDurableValidateAdapterPublicationState::ValidatedApply(prepared),
            ));
        };
        let Some(replay_evidence) = DurableValidateReplayEvidenceV1::seal_decision_apply(
            capability,
            validate_origin,
            validate_effect,
            validate_pending,
            receipt,
            apply_effect,
            &child_pending,
        ) else {
            return Err(Self(
                ReadyDurableValidateAdapterPublicationState::ValidatedApply(prepared),
            ));
        };
        Ok(PreparedDurableValidateApplyAdapterReplay {
            prepared,
            child_pending: Some(child_pending),
            replay_evidence: Some(replay_evidence),
            validated_receipt: receipt.clone(),
            registry_work: None,
        })
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
            child_pending: Some(child_pending),
            replay_evidence: Some(replay_evidence),
            registry_work: None,
        };
        if !sealed.exactly_matches(validate_effect, validate_pending, receipt) {
            let PreparedInvalidBodyReportAdapterReplay { prepared, .. } = sealed;
            return Err(Self(
                ReadyDurableValidateAdapterPublicationState::RejectedReport(prepared),
            ));
        }
        Ok(sealed)
    }
}
impl PreparedInvalidBodyReportAdapterReplay<'_> {
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
            && projected.as_ref() == self.child_pending.as_ref()
            && self.child_pending.as_ref().is_some_and(|child_pending| {
                self.replay_evidence.as_ref().is_some_and(|replay| {
                    replay.exactly_matches(
                        validate_effect,
                        validate_pending,
                        receipt,
                        report_effect,
                        child_pending,
                    )
                })
            })
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
            .as_ref()
            .ok_or(AdapterEffectAdmissionError::InvalidCarrier)?
            .project_sealed_invalid_body_report_candidate(
                permit,
                verified,
                validate_effect,
                validate_pending,
                receipt,
                self.prepared.report_effect(),
                self.child_pending
                    .as_ref()
                    .ok_or(AdapterEffectAdmissionError::InvalidCarrier)?,
            )
    }
    /// Consume the nested replay envelope into closed registry work.
    #[allow(clippy::result_large_err)]
    pub(in crate::sumeragi) fn prepare_registry_work(
        mut self,
        permit: InvalidBodyReportRegistryWorkProjectionPermit,
    ) -> Result<Self, Self> {
        if self.registry_work.is_some() {
            return Err(self);
        }
        let evidence = self
            .replay_evidence
            .take()
            .expect("exact report replay retains its evidence");
        let child_pending = self
            .child_pending
            .take()
            .expect("exact report replay retains its child pending owner");
        match evidence.into_registry_work(
            permit,
            self.prepared.report_effect.clone(),
            child_pending,
        ) {
            Ok(work) => {
                self.registry_work = Some(work);
                Ok(self)
            }
            Err((evidence, _effect, child_pending)) => {
                self.replay_evidence = Some(evidence);
                self.child_pending = Some(child_pending);
                Err(self)
            }
        }
    }
    /// Borrow the closed registry work for coordinate preflight.
    pub(in crate::sumeragi) const fn registry_work(
        &self,
    ) -> Option<&PreparedInvalidBodyReportRegistryWork> {
        self.registry_work.as_ref()
    }
    /// Commit the already-published report branch into the serialized adapter.
    pub(in crate::sumeragi) fn commit_after_lifecycle_publication(self) {
        let Self { prepared, .. } = self;
        let PreparedDirectValidationFailedReport {
            _adapter: adapter,
            next_reducer,
            next_registry,
            event,
            core_effect,
            report_effect: _,
            next_fence_generation,
        } = prepared;
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
    /// Install the preflighted report carrier and commit reducer state after
    /// the matching lifecycle successor is durable.
    pub(in crate::sumeragi) fn install_registry_and_commit_adapter(
        mut self,
        reservation: LiveValidateReportRegistryReservation<'_>,
    ) {
        let work = self
            .registry_work
            .take()
            .expect("pre-fsync report publication retains closed registry work");
        work.install_into(reservation);
        self.commit_after_lifecycle_publication();
    }
}
impl PreparedDurableValidateApplyAdapterReplay<'_> {
    pub(in crate::sumeragi) fn project_candidate(
        &self,
        verified: &VerifiedHeightContext,
        validate_effect: &AdapterEffect,
        validate_pending: &PendingRuntimeEffectBinding,
        receipt: &ValidatedBodyReceipt,
    ) -> Result<CandidateAdmission, AdapterEffectAdmissionError> {
        let child_pending = self
            .child_pending
            .as_ref()
            .ok_or(AdapterEffectAdmissionError::InvalidCarrier)?;
        self.replay_evidence
            .as_ref()
            .ok_or(AdapterEffectAdmissionError::InvalidCarrier)?
            .project_candidate(
                verified,
                validate_effect,
                validate_pending,
                receipt,
                self.prepared.apply_effect(),
                child_pending,
            )
    }
    #[allow(clippy::result_large_err)]
    pub(in crate::sumeragi) fn prepare_registry_work(
        mut self,
        permit: ValidateApplyRegistryWorkProjectionPermit,
        candidate: CandidateAdmission,
    ) -> Result<Self, Self> {
        if self.registry_work.is_some() {
            return Err(self);
        }
        let evidence = self
            .replay_evidence
            .take()
            .expect("exact Apply replay retains evidence");
        let pending = self
            .child_pending
            .take()
            .expect("exact Apply replay retains child pending");
        match evidence.into_registry_work(
            permit,
            self.prepared.apply_effect.clone(),
            pending,
            self.validated_receipt.clone(),
            candidate,
        ) {
            Ok(work) => {
                self.registry_work = Some(work);
                Ok(self)
            }
            Err((evidence, _effect, pending, _receipt, _candidate)) => {
                self.replay_evidence = Some(evidence);
                self.child_pending = Some(pending);
                Err(self)
            }
        }
    }
    pub(in crate::sumeragi) const fn registry_work(
        &self,
    ) -> Option<&PreparedValidateApplyRegistryWork> {
        self.registry_work.as_ref()
    }
    pub(in crate::sumeragi) fn install_registry_and_commit_adapter(
        mut self,
        reservation: LiveValidateApplyRegistryReservation<'_>,
    ) {
        let work = self
            .registry_work
            .take()
            .expect("pre-fsync Apply publication retains closed registry work");
        work.install_into(reservation);
        let PreparedDirectValidationSucceededApply {
            _adapter: adapter,
            next_reducer,
            next_registry,
            event,
            core_effect,
            apply_effect: _,
            next_fence_generation,
        } = self.prepared;
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
impl PreparedReadyDurableValidateAdapterPublication<'_> {
    /// Commit an already-durable inactive/no-effect branch. The caller has
    /// preflighted the discriminator and published the matching lifecycle
    /// tombstone, so every remaining assignment is infallible.
    pub(in crate::sumeragi) fn commit_no_successor_after_lifecycle_publication(self) {
        match self.0 {
            ReadyDurableValidateAdapterPublicationState::ValidatedInactive(prepared) => {
                let PreparedDirectValidationSucceededInactive {
                    _adapter: adapter,
                    next_reducer,
                    next_registry,
                    disposition: _,
                } = prepared;
                adapter.reducer = next_reducer;
                adapter.registry = next_registry;
            }
            ReadyDurableValidateAdapterPublicationState::RejectedInactive(prepared) => {
                let PreparedDirectValidationFailedInactive {
                    _adapter: adapter,
                    next_reducer,
                    next_registry,
                    disposition: _,
                } = prepared;
                adapter.reducer = next_reducer;
                adapter.registry = next_registry;
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
            _ => unreachable!("sealed no-successor projection fixes its adapter branch"),
        }
    }
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
