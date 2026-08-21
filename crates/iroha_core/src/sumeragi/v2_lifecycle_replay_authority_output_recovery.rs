/// Authenticated cold execution authority for one exact live lifecycle output row.
///
/// The complete effect, reconstructed ordinal-free pending binding, canonical
/// admission, and durable address remain inseparable.  This is intentionally
/// distinct from live [`RuntimeEffectOwnership`]: positional runtime ownership
/// is process-local and is not serialized, so cold open must never synthesize it.
#[must_use = "authenticated lifecycle output recovery must enter its exact registry row"]
pub(in crate::sumeragi) struct AuthenticatedRecoveredLifecycleOutputV1 {
    effect: AdapterEffect,
    pending: PendingRuntimeEffectBinding,
    candidate: CandidateAdmission,
    owner: OwnerId,
    ordinal: u128,
    invalid_body: Option<InvalidBodyReplaySourceV1>,
}

impl core::fmt::Debug for AuthenticatedRecoveredLifecycleOutputV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("AuthenticatedRecoveredLifecycleOutputV1")
            .field("owner", &self.owner)
            .field("ordinal", &self.ordinal)
            .field("work_class", &self.candidate.work_class)
            .finish_non_exhaustive()
    }
}

impl AuthenticatedRecoveredLifecycleOutputV1 {
    /// Borrow the authenticated output while its exact durable owner stays sealed.
    pub(in crate::sumeragi) const fn effect(&self) -> &AdapterEffect {
        &self.effect
    }

    /// Borrow the exact canonical candidate while the move-only execution seal remains owned.
    pub(in crate::sumeragi) const fn candidate(&self) -> &CandidateAdmission {
        &self.candidate
    }

    /// Exact durable owner retained by this cold carrier.
    pub(in crate::sumeragi) const fn owner(&self) -> OwnerId {
        self.owner
    }

    /// Exact immutable ledger ordinal retained by this cold carrier.
    pub(in crate::sumeragi) const fn ordinal(&self) -> u128 {
        self.ordinal
    }

    /// Return whether this carrier still requires one revalidated rejection marker.
    pub(in crate::sumeragi) const fn requires_rejected_body_marker(&self) -> bool {
        self.invalid_body.is_some()
    }

    /// Compare the exact persisted rejection marker with this report's private body binding.
    pub(in crate::sumeragi) fn exactly_matches_rejected_body_outcome(
        &self,
        outcome: &DurableBodyValidationOutcome,
    ) -> bool {
        self.invalid_body
            .as_ref()
            .is_some_and(|source| source.exactly_matches_rejected_body_outcome(outcome))
    }

    /// Recheck the complete cold source immediately before external output I/O.
    pub(in crate::sumeragi) fn authenticates_settlement(
        &self,
        verified: &VerifiedHeightContext,
    ) -> bool {
        if !self.pending.exactly_binds_adapter_effect(&self.effect)
            || *self.pending.causal_lifecycle_key()
                != Hash::prehashed(*self.owner.causal_root().digest().as_bytes())
        {
            return false;
        }
        match &self.effect {
            AdapterEffect::Broadcast(message) => {
                !matches!(
                    &message.payload,
                    wire::ConsensusMessageV2Payload::Proposal(_)
                ) && verified.verify_consensus_message(message).is_ok()
            }
            AdapterEffect::ReportEquivocation { evidence } => verified
                .authenticate_recovered_equivocation(&evidence.to_wire())
                .is_ok_and(|authenticated| authenticated.to_wire() == evidence.to_wire()),
            AdapterEffect::ReportInvalidCertifiedBody {
                subject,
                certificate,
            } => {
                certificate.subject == *subject
                    && verified.verify_quorum_certificate(certificate).is_ok()
                    && self.invalid_body.as_ref().is_some_and(|source| {
                        source.certificate == *certificate
                            && source.cryptographically_authenticates(verified)
                    })
            }
            AdapterEffect::Sign { .. }
            | AdapterEffect::FetchBody { .. }
            | AdapterEffect::StoreBody { .. }
            | AdapterEffect::ValidateBody { .. }
            | AdapterEffect::Apply { .. }
            | AdapterEffect::EnterView { .. } => false,
        }
    }
}

impl InvalidBodyReplaySourceV1 {
    fn origin_body_frame(&self, context: LifecycleContext) -> BodyFrameBindingV1 {
        BodyFrameBindingV1 {
            context: *context.id().as_bytes(),
            round_height: self.outcome.manifest.round.height,
            round_view: self.outcome.manifest.round.view,
            subject: *block_subject(self.outcome.manifest.subject).as_bytes(),
            manifest: *HashOf::new(&self.outcome.manifest).as_ref(),
            frame: self.outcome.body_frame_hash,
        }
    }

    fn exactly_descends_from_validate(
        &self,
        context: LifecycleContext,
        parent_authority: &LifecycleReplayAuthorityV1,
        parent_payload: DurablePayloadReference,
    ) -> bool {
        let ReplayPayloadBindingV1::BodyFrame(expected_frame) = &parent_authority.payload else {
            return false;
        };
        let LifecycleReplaySourceV1::BodyPipeline(parent_source) = &parent_authority.source else {
            return false;
        };
        let origin_shape = self.validation_origin.project(
            context,
            LifecycleStageKind::ValidateBody,
            &ReplayPayloadBindingV1::BodyFrame(*expected_frame),
        );
        parent_authority.format_version == REPLAY_AUTHORITY_FORMAT_VERSION
            && parent_authority.is_bounded_canonical()
            && parent_payload
                == DurablePayloadReference::BodyFrame(expected_frame.durable_reference())
            && *expected_frame == self.origin_body_frame(context)
            && parent_source == &self.validation_origin
            && origin_shape.is_ok_and(|shape| {
                shape.work_class == LifecycleWorkClass::Validate
                    && shape.stage_kind == LifecycleStageKind::ValidateBody
            })
    }

    fn cryptographically_authenticates(&self, verified: &VerifiedHeightContext) -> bool {
        if verified
            .verify_quorum_certificate(&self.certificate)
            .is_err()
        {
            return false;
        }
        match &self.validation_origin.origin {
            BodyPipelineOriginV1::Proposal(proposal) => verified
                .verify_consensus_message(&wire::ConsensusMessageV2::new(
                    wire::ConsensusMessageV2Payload::Proposal(proposal.clone()),
                ))
                .is_ok(),
            BodyPipelineOriginV1::Certified { certificate, .. } => {
                certificate == &self.certificate
                    && verified.verify_quorum_certificate(certificate).is_ok()
            }
            BodyPipelineOriginV1::LocalBody(_) | BodyPipelineOriginV1::RecoveredDecision { .. } => {
                false
            }
        }
    }

    fn exactly_matches_rejected_body_outcome(
        &self,
        outcome: &DurableBodyValidationOutcome,
    ) -> bool {
        let durable = outcome.durable_body();
        outcome
            .rejection_identity()
            .is_some_and(|identity| identity.canonical_code() == self.outcome.rejection_code)
            && durable.context_id() == self.outcome.manifest.round.context_id
            && durable.round() == self.outcome.manifest.round
            && durable.subject() == self.outcome.manifest.subject
            && durable.manifest_hash() == HashOf::new(&self.outcome.manifest)
            && *durable.frame_hash().as_ref() == self.outcome.body_frame_hash
    }
}

/// Re-authenticate one exact output row from its canonical V1 replay source.
///
/// `invalid_parent` is present only for `Validate -> InvalidBodyReport` and is
/// already required to be the same-owner adjacent predecessor by LedgerV1.
/// Complete replay-family equality and body-frame identity are still checked
/// here before a report carrier is returned.
#[allow(clippy::too_many_arguments)]
pub(super) fn authenticate_durable_lifecycle_output(
    verified: &VerifiedHeightContext,
    context: LifecycleContext,
    key: LifecycleKey,
    owner: OwnerId,
    ordinal: u128,
    work_class: LifecycleWorkClass,
    stage: LifecycleStage,
    terminal: Option<TerminalOutcome>,
    reconstruction_source: LifecycleDigest,
    payload: DurablePayloadReference,
    continuation: super::schema::DurableContinuation,
    authority: &LifecycleReplayAuthorityV1,
    invalid_parent: Option<(&LifecycleReplayAuthorityV1, DurablePayloadReference)>,
) -> Option<AuthenticatedRecoveredLifecycleOutputV1> {
    if terminal.is_some()
        || payload != DurablePayloadReference::None
        || continuation != super::schema::DurableContinuation::None
        || reconstruction_source != owner.causal_root().digest()
        || super::projection::lifecycle_context(verified.context()) != context
        || !authority.structurally_matches_record(context, key, work_class, stage, payload)
    {
        return None;
    }

    let (effect, invalid_body) = match &authority.source {
        LifecycleReplaySourceV1::ConsensusBroadcast(message)
            if work_class == LifecycleWorkClass::Broadcast
                && invalid_parent.is_none()
                && !matches!(
                    &message.payload,
                    wire::ConsensusMessageV2Payload::Proposal(_)
                ) =>
        {
            verified.verify_consensus_message(message).ok()?;
            let effect = AdapterEffect::Broadcast(message.clone());
            (exact_signed_broadcast_authority(&effect).as_ref() == Some(authority))
                .then_some((effect, None))?
        }
        LifecycleReplaySourceV1::Equivocation(persisted)
            if work_class == LifecycleWorkClass::EquivocationReport && invalid_parent.is_none() =>
        {
            let evidence = verified
                .authenticate_recovered_equivocation(persisted)
                .ok()?;
            let effect = AdapterEffect::ReportEquivocation { evidence };
            (exact_signed_equivocation_authority(&effect).as_ref() == Some(authority))
                .then_some((effect, None))?
        }
        LifecycleReplaySourceV1::InvalidCertifiedBody(source)
            if work_class == LifecycleWorkClass::InvalidBodyReport =>
        {
            let (parent_authority, parent_payload) = invalid_parent?;
            if !source.cryptographically_authenticates(verified)
                || !source.exactly_descends_from_validate(context, parent_authority, parent_payload)
            {
                return None;
            }
            (
                AdapterEffect::ReportInvalidCertifiedBody {
                    subject: source.certificate.subject,
                    certificate: source.certificate.clone(),
                },
                Some(source.clone()),
            )
        }
        LifecycleReplaySourceV1::Wal(_)
        | LifecycleReplaySourceV1::ConsensusBroadcast(_)
        | LifecycleReplaySourceV1::BodyPipeline(_)
        | LifecycleReplaySourceV1::Equivocation(_)
        | LifecycleReplaySourceV1::InvalidCertifiedBody(_)
        | LifecycleReplaySourceV1::CertifiedServeStorage(_) => return None,
    };

    let pending = PendingRuntimeEffectBinding::from_durable_lifecycle_output(
        DurableLifecycleOutputPendingMintPermit::new(),
        Hash::prehashed(*owner.causal_root().digest().as_bytes()),
        &effect,
    )?;
    let projected = super::projection::authority_free_admission_projection(
        context, verified, &effect, &pending,
    )
    .ok()?;
    let candidate = candidate_from_authorized_projection(
        context,
        projected,
        DurablePayloadReference::None,
        authority.clone(),
    )?;
    (candidate.key == key
        && candidate.causal_root == owner.causal_root()
        && candidate.work_class == work_class
        && candidate.stage == stage
        && candidate.initial_state == InitialLifecycleState::Ready
        && candidate.reconstruction_source == reconstruction_source
        && candidate.payload == payload
        && candidate.producer_turn.is_none()
        && candidate.replay_authority == *authority)
        .then_some(AuthenticatedRecoveredLifecycleOutputV1 {
            effect,
            pending,
            candidate,
            owner,
            ordinal,
            invalid_body,
        })
}
