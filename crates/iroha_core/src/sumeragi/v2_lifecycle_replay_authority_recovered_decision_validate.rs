/// Canonical recovered-Decision lineage retained by one live Validate row.
///
/// Unlike standalone body replay, this value keeps the authenticated WAL
/// Decision locator and complete Store/Validate/Apply family inseparable.
/// The exact Validate pending binding is retained only as an inert fingerprint;
/// executable Apply authority remains sealed in the adapter and registry cuts.
#[derive(Clone, Debug, PartialEq, Eq)]
#[must_use = "recovered Decision Validate replay must remain attached to its durable carrier"]
pub(in crate::sumeragi) struct RecoveredDecisionValidateReplayEvidenceV1 {
    lineage: RecoveredDecisionApplyReplayLineageV1,
    validate_pending: DirectSignedPendingBindingV1,
}

impl RecoveredDecisionApplyReplayLineageV1 {
    /// Project the exact recovered Store-to-Validate replay evidence.
    ///
    /// The caller supplies only the reducer-derived successor pair. The WAL
    /// locator, CommitQC, manifest, and BodyFrame remain fixed by this sealed
    /// lineage, and the predecessor binding must derive the submitted
    /// Validate binding exactly.
    pub(super) fn project_recovered_store_validate_evidence(
        &self,
        verified: &VerifiedHeightContext,
        receipt: &DurableBodyReceipt,
        store_effect: &AdapterEffect,
        store_pending: &PendingRuntimeEffectBinding,
        store_candidate: &CandidateAdmission,
        validate_effect: &AdapterEffect,
        validate_pending: &PendingRuntimeEffectBinding,
    ) -> Option<DurableValidateReplayEvidenceV1> {
        let context = super::projection::lifecycle_context(verified.context());
        if receipt.context_id() != verified.context().id() || !self.is_stage_closed(context) {
            return None;
        }
        if self
            .project_recovered_fetch_store_candidate(verified, receipt, store_effect, store_pending)
            .as_ref()
            != Some(store_candidate)
        {
            return None;
        }
        let projected =
            store_pending.project_store_validate_successor(store_effect, validate_effect)?;
        if projected != *validate_pending
            || !validate_pending.exactly_binds_adapter_effect(validate_effect)
        {
            return None;
        }
        let evidence = RecoveredDecisionValidateReplayEvidenceV1 {
            lineage: self.clone(),
            validate_pending: DirectSignedPendingBindingV1::from_exact_effect(
                validate_effect,
                validate_pending,
            )?,
        };
        evidence
            .exactly_matches_validate_pending(validate_effect, receipt, validate_pending)
            .then(|| {
                self.project_recovered_store_validate_candidate(
                    verified,
                    receipt,
                    store_candidate,
                    validate_effect,
                    validate_pending,
                )
            })
            .flatten()
            .map(|_| DurableValidateReplayEvidenceV1::RecoveredDecision(evidence))
    }
    /// Project the exact Validate candidate after its recovered Store was authenticated.
    pub(super) fn project_recovered_store_validate_candidate(
        &self,
        verified: &VerifiedHeightContext,
        receipt: &DurableBodyReceipt,
        store_candidate: &CandidateAdmission,
        validate_effect: &AdapterEffect,
        validate_pending: &PendingRuntimeEffectBinding,
    ) -> Option<CandidateAdmission> {
        let context = super::projection::lifecycle_context(verified.context());
        if !self.is_stage_closed(context)
            || receipt.context_id() != verified.context().id()
            || store_candidate.work_class != LifecycleWorkClass::Store
            || store_candidate.stage.kind() != LifecycleStageKind::StoreBody
        {
            return None;
        }
        let payload =
            DurablePayloadReference::BodyFrame(durable_body_frame_reference(context, receipt)?);
        let authority = self
            .body
            .authority_for(context, LifecycleStageKind::ValidateBody)?;
        let candidate = candidate_from_authorized_projection(
            context,
            super::projection::authority_free_admission_projection(
                context,
                verified,
                validate_effect,
                validate_pending,
            )
            .ok()?,
            payload,
            authority,
        )?;
        (candidate.work_class == LifecycleWorkClass::Validate
            && candidate.stage.kind() == LifecycleStageKind::ValidateBody
            && candidate.initial_state == InitialLifecycleState::Ready
            && candidate.payload == payload
            && candidate.causal_root == store_candidate.causal_root
            && candidate.reconstruction_source == store_candidate.reconstruction_source
            && candidate.replay_authority_is_exact(context)
            && recovered_decision_body_continuation_is_exact(
                super::schema::DurableContinuationEdge::StoreToValidate,
                &store_candidate.replay_authority,
                store_candidate.payload,
                &candidate.replay_authority,
                candidate.payload,
            ) == Some(true))
        .then_some(candidate)
    }
}

impl RecoveredDecisionApplyCandidateLineageV1 {
    /// Compare the exact durable crash cut after Store advanced to live Validate.
    pub(super) fn exactly_matches_live_validate_records(
        &self,
        owner: OwnerId,
        store: &LifecycleLedgerRecordV1,
        validate: &LifecycleLedgerRecordV1,
    ) -> bool {
        store.ordinal() < validate.ordinal()
            && Self::candidate_matches_record(
                &self.store,
                store,
                owner,
                Some(TerminalOutcome::Advanced),
                super::schema::DurableContinuation::successor(
                    super::schema::DurableContinuationEdge::StoreToValidate,
                    validate.ordinal(),
                ),
            )
            && Self::candidate_matches_record(
                &self.validate,
                validate,
                owner,
                None,
                super::schema::DurableContinuation::None,
            )
    }
    /// Advance one exact live Validate and append its recovered Decision Apply child.
    ///
    /// Unrelated rows may occupy ordinals between either predecessor and the
    /// new Apply. The typed continuations, owner, body frame, and WAL lineage
    /// authenticate the chain rather than physical adjacency.
    pub(super) fn successor_records_after_live_validate(
        &self,
        owner: OwnerId,
        store: &LifecycleLedgerRecordV1,
        validate: &LifecycleLedgerRecordV1,
        apply_ordinal: u128,
    ) -> Option<[LifecycleLedgerRecordV1; 3]> {
        if !self.exactly_matches_live_validate_records(owner, store, validate)
            || validate.ordinal() >= apply_ordinal
        {
            return None;
        }
        let store = LifecycleLedgerRecordV1::new(
            self.store.key,
            owner,
            store.ordinal(),
            self.store.work_class,
            self.store.stage,
            Some(TerminalOutcome::Advanced),
            self.store.reconstruction_source,
            self.store.payload,
            self.store.replay_authority.clone(),
            super::schema::DurableContinuation::successor(
                super::schema::DurableContinuationEdge::StoreToValidate,
                validate.ordinal(),
            ),
        )
        .ok()?;
        let validate = LifecycleLedgerRecordV1::new(
            self.validate.key,
            owner,
            validate.ordinal(),
            self.validate.work_class,
            self.validate.stage,
            Some(TerminalOutcome::Advanced),
            self.validate.reconstruction_source,
            self.validate.payload,
            self.validate.replay_authority.clone(),
            super::schema::DurableContinuation::successor(
                super::schema::DurableContinuationEdge::ValidateToApply,
                apply_ordinal,
            ),
        )
        .ok()?;
        let apply = LifecycleLedgerRecordV1::new(
            self.apply.key,
            owner,
            apply_ordinal,
            self.apply.work_class,
            self.apply.stage,
            None,
            self.apply.reconstruction_source,
            self.apply.payload,
            self.apply.replay_authority.clone(),
            super::schema::DurableContinuation::None,
        )
        .ok()?;
        Some([store, validate, apply])
    }
}

impl PreparedDurableCertifiedBodyPipelineStartupV1 {
    /// Install beside the exact ordinary Validate carrier whose authority is
    /// the sealed recovered-Decision WAL/body projection.
    pub(super) fn install_alongside_recovered_decision_validate(
        self,
        registry: &mut ConcreteLifecycleWorkRegistry,
        validate: &super::wal_recovery::RecoveredDecisionValidateInstalledSealV1,
    ) -> Result<(), Self> {
        let works = self
            .entries
            .iter()
            .map(|entry| (entry.work.address(), entry.work.digest()))
            .collect::<Vec<_>>();
        if self.entries.iter().any(|entry| entry.candidate.is_some())
            || !registry
                .preflights_recovered_body_pipeline_alongside_decision_validate(validate, &works)
        {
            return Err(self);
        }
        for entry in self.entries {
            assert!(
                registry
                    .install_recovered_durable_body_pipeline(entry.work)
                    .is_ok(),
                "preflighted recovered Decision Validate-plus-body installation is infallible"
            );
        }
        Ok(())
    }
}
