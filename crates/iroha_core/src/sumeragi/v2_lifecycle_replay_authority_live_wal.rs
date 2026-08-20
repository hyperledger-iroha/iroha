/// Non-decodable live authority for one exact fsynced WAL continuation.
///
/// Payload-free stages retain their complete canonical V1 envelope. `Apply`
/// deliberately remains a source-only seal until the closed Validate registry
/// join supplies its store-authenticated body frame. Neither state exposes a
/// locator, action, encoded source, or parts API.
#[derive(PartialEq, Eq)]
#[must_use = "a live WAL replay seal must remain joined to its persisted continuation"]
struct LiveWalPersistedReplaySealV1 {
    wal_identity: LiveWalFrameIdentity,
    state: LiveWalPersistedReplayStateV1,
}
#[derive(PartialEq, Eq)]
enum LiveWalPersistedReplayStateV1 {
    Canonical {
        stage: LifecycleStageKind,
        authority: LifecycleReplayAuthorityV1,
    },
    ApplyPending {
        context: LifecycleContext,
        source: WalReplaySourceV1,
    },
}
/// Exact live WAL continuation kept inseparable from its adapter effect.
///
/// This move-only envelope is the linear transport returned by the adapter's
/// specialized post-fsync cuts. It exposes only fixed pending-binding and
/// receipt-bound equality joins; neither the effect nor its replay seal can be
/// extracted.
#[must_use = "a sealed live WAL effect has not entered lifecycle pre-admission"]
pub(in crate::sumeragi) struct SealedLiveWalPersistedEffectV1 {
    effect: AdapterEffect,
    replay: LiveWalPersistedReplaySealV1,
    pending: LiveWalPersistedPendingV1,
}
#[derive(PartialEq, Eq)]
enum LiveWalPersistedPendingV1 {
    PayloadFree(PendingRuntimeEffectBinding),
    ValidateSignBound(PendingRuntimeEffectBinding),
    ApplyPending,
    ApplyBound(PendingRuntimeEffectBinding),
}
impl SealedLiveWalPersistedEffectV1 {
    /// Consume the adapter's one record-checked live continuation cause.
    pub(in crate::sumeragi) fn from_exact_live_append(
        cause: ExactLiveWalPersistedContinuationCause,
    ) -> Option<Self> {
        let (wal_identity, effect, pending) = match cause {
            ExactLiveWalPersistedContinuationCause::PayloadFree {
                wal_identity,
                effect,
                pending,
            } => (
                wal_identity,
                effect,
                LiveWalPersistedPendingV1::PayloadFree(pending),
            ),
            ExactLiveWalPersistedContinuationCause::Apply {
                wal_identity,
                effect,
            } => (
                wal_identity,
                effect,
                LiveWalPersistedPendingV1::ApplyPending,
            ),
        };
        let replay =
            LiveWalPersistedReplaySealV1::from_exact_persisted_effect(wal_identity, &effect)?;
        let sealed = Self {
            effect,
            replay,
            pending,
        };
        sealed.exactly_matches_effect().then_some(sealed)
    }
    /// Replace the frame-derived placeholder owner of one exact vote-sign
    /// continuation with the predecessor-derived binding sealed by the Ready
    /// Validate adapter preflight.
    ///
    /// The caller cannot provide a WAL identity or effect. Failure returns both
    /// move-only inputs intact; success keeps the bound pending value nested in
    /// this replay envelope.
    #[allow(clippy::result_large_err)]
    pub(in crate::sumeragi) fn bind_exact_validate_sign_pending(
        self,
        pending: PendingRuntimeEffectBinding,
    ) -> Result<Self, (Self, PendingRuntimeEffectBinding)> {
        if !matches!(&self.pending, LiveWalPersistedPendingV1::PayloadFree(_))
            || !matches!(
                &self.effect,
                AdapterEffect::Sign {
                    request: SignRequest::Vote(vote),
                    ..
                } if matches!(vote.phase, wire::GlobalPhase::Prepare | wire::GlobalPhase::Commit)
            )
            || !pending.exactly_binds_adapter_effect(&self.effect)
            || !self
                .replay
                .exactly_matches_payload_free_effect(&self.effect)
        {
            return Err((self, pending));
        }
        let Self { effect, replay, .. } = self;
        Ok(Self {
            effect,
            replay,
            pending: LiveWalPersistedPendingV1::ValidateSignBound(pending),
        })
    }
    /// Recheck the sealed post-append Validate-to-Sign binding without
    /// releasing its effect or pending owner.
    pub(in crate::sumeragi) fn exactly_binds_validate_sign_pending(&self) -> bool {
        matches!(
            &self.pending,
            LiveWalPersistedPendingV1::ValidateSignBound(pending)
                if pending.exactly_binds_adapter_effect(&self.effect)
        ) && matches!(
            &self.effect,
            AdapterEffect::Sign {
                request: SignRequest::Vote(vote),
                ..
            } if matches!(vote.phase, wire::GlobalPhase::Prepare | wire::GlobalPhase::Commit)
        ) && self
            .replay
            .exactly_matches_payload_free_effect(&self.effect)
    }
    /// Recheck one initial local Proposal Sign against its frame-derived owner.
    ///
    /// This comparison-only seam lets the adapter retain the complete live
    /// seal behind an opaque handoff. It exposes neither the effect nor the
    /// locator-derived pending binding, and deliberately does not accept the
    /// inherited local-body owner used before the ProposalIntent WAL append.
    pub(in crate::sumeragi) fn exactly_binds_payload_free_proposal_sign(
        &self,
        effect: &AdapterEffect,
    ) -> bool {
        self.effect == *effect
            && matches!(
                &self.effect,
                AdapterEffect::Sign {
                    request: SignRequest::Proposal(proposal),
                    ..
                } if proposal.signature.is_empty()
            )
            && matches!(
                &self.pending,
                LiveWalPersistedPendingV1::PayloadFree(pending)
                    if pending.exactly_binds_adapter_effect(&self.effect)
            )
            && self
                .replay
                .exactly_matches_payload_free_effect(&self.effect)
    }
    /// Recheck one body-frame-completed Apply without exposing its WAL source
    /// or predecessor-derived pending owner.
    pub(in crate::sumeragi) fn exactly_binds_completed_apply(
        &self,
        receipt: &DurableBodyReceipt,
    ) -> bool {
        matches!(
            &self.pending,
            LiveWalPersistedPendingV1::ApplyBound(pending)
                if pending.exactly_binds_adapter_effect(&self.effect)
        ) && matches!(
            &self.replay.state,
            LiveWalPersistedReplayStateV1::Canonical { stage, .. }
                if *stage == LifecycleStageKind::ApplyDecision
        ) && self
            .replay
            .exactly_matches_apply_effect(&self.effect, receipt)
    }
    /// Project the exact WAL-owned Apply child with its retained BodyFrame.
    pub(in crate::sumeragi) fn project_sealed_validate_apply_candidate(
        &self,
        _permit: &SealedValidateApplyProjectionPermit,
        verified: &VerifiedHeightContext,
        receipt: &DurableBodyReceipt,
    ) -> Result<CandidateAdmission, AdapterEffectAdmissionError> {
        if !self.exactly_binds_completed_apply(receipt) {
            return Err(AdapterEffectAdmissionError::InvalidCarrier);
        }
        let LiveWalPersistedPendingV1::ApplyBound(pending) = &self.pending else {
            return Err(AdapterEffectAdmissionError::InvalidCarrier);
        };
        let LiveWalPersistedReplayStateV1::Canonical { authority, stage } = &self.replay.state
        else {
            return Err(AdapterEffectAdmissionError::InvalidCarrier);
        };
        if *stage != LifecycleStageKind::ApplyDecision {
            return Err(AdapterEffectAdmissionError::InvalidCarrier);
        }
        let active_context = super::projection::lifecycle_context(verified.context());
        let Some(frame) = durable_body_frame_reference(active_context, receipt) else {
            return Err(AdapterEffectAdmissionError::InvalidCarrier);
        };
        let projected = super::projection::authority_free_admission_projection(
            active_context,
            verified,
            &self.effect,
            pending,
        )?;
        candidate_from_authorized_projection(
            active_context,
            projected,
            DurablePayloadReference::BodyFrame(frame),
            authority.clone(),
        )
        .ok_or(AdapterEffectAdmissionError::InvalidCarrier)
    }
    /// Consume the completed Apply seal into closed ordinary registry work.
    #[allow(clippy::result_large_err)]
    pub(in crate::sumeragi) fn into_live_validate_apply_work(
        self,
        permit: LiveValidateApplyWorkProjectionPermit,
        receipt: &DurableBodyReceipt,
    ) -> Result<PreparedLiveValidateApplyRegistryWork, Self> {
        if !self.exactly_binds_completed_apply(receipt) {
            return Err(self);
        }
        let Self {
            effect,
            replay,
            pending,
        } = self;
        let LiveWalPersistedPendingV1::ApplyBound(pending) = pending else {
            unreachable!("completed live Apply retains its bound pending owner")
        };
        let LiveWalPersistedReplayStateV1::Canonical {
            authority: replay_authority,
            ..
        } = &replay.state
        else {
            unreachable!("completed live Apply retains canonical WAL authority")
        };
        match PreparedLiveValidateApplyRegistryWork::from_exact(
            permit,
            effect,
            pending,
            replay_authority.clone(),
        ) {
            Ok(work) => Ok(work),
            Err((_error, effect, pending)) => Err(Self {
                effect,
                replay,
                pending: LiveWalPersistedPendingV1::ApplyBound(pending),
            }),
        }
    }
    /// Project the exact replay-authorized Sign child without releasing its
    /// nested effect or predecessor-derived pending owner.
    ///
    /// Only the body-transition module can mint `permit`. In particular this
    /// path does not repeat ordinary-to-Commit refinement after the opaque
    /// registered-Prepare capability was consumed before WAL append.
    pub(in crate::sumeragi) fn project_sealed_validate_sign_candidate(
        &self,
        _permit: &SealedValidateSignProjectionPermit,
        verified: &VerifiedHeightContext,
    ) -> Result<CandidateAdmission, AdapterEffectAdmissionError> {
        if !self.exactly_binds_validate_sign_pending() {
            return Err(AdapterEffectAdmissionError::InvalidCarrier);
        }
        let LiveWalPersistedPendingV1::ValidateSignBound(pending) = &self.pending else {
            return Err(AdapterEffectAdmissionError::InvalidCarrier);
        };
        let LiveWalPersistedReplayStateV1::Canonical { authority, stage } = &self.replay.state
        else {
            return Err(AdapterEffectAdmissionError::InvalidCarrier);
        };
        if !matches!(
            stage,
            LifecycleStageKind::SignPrepareVote | LifecycleStageKind::SignCommitVote
        ) {
            return Err(AdapterEffectAdmissionError::InvalidCarrier);
        }
        let active_context = super::projection::lifecycle_context(verified.context());
        let projected = super::projection::authority_free_admission_projection(
            active_context,
            verified,
            &self.effect,
            pending,
        )?;
        candidate_from_authorized_projection(
            active_context,
            projected,
            DurablePayloadReference::None,
            authority.clone(),
        )
        .ok_or(AdapterEffectAdmissionError::InvalidCarrier)
    }
    /// Consume the exact nested Validate-to-Sign continuation into one closed
    /// ordinary registry carrier.
    ///
    /// The caller receives neither effect nor pending parts. The one-shot
    /// permit is minted only after the fixed transaction has staged the exact
    /// child and is ready to reserve its concrete address.
    #[allow(clippy::result_large_err)]
    pub(in crate::sumeragi) fn into_live_validate_sign_work(
        self,
        permit: LiveValidateSignWorkProjectionPermit,
    ) -> Result<PreparedLiveValidateSignRegistryWork, Self> {
        if !self.exactly_binds_validate_sign_pending() {
            return Err(self);
        }
        let Self {
            effect,
            replay,
            pending,
        } = self;
        let LiveWalPersistedPendingV1::ValidateSignBound(pending) = pending else {
            unreachable!("exact live Validate-to-Sign seal retains its bound pending owner")
        };
        let LiveWalPersistedReplayStateV1::Canonical {
            authority: replay_authority,
            ..
        } = &replay.state
        else {
            unreachable!("exact live Validate-to-Sign seal retains canonical WAL authority")
        };
        match PreparedLiveValidateSignRegistryWork::from_exact(
            permit,
            effect,
            pending,
            replay_authority.clone(),
        ) {
            Ok(work) => Ok(work),
            Err((_error, effect, pending)) => Err(Self {
                effect,
                replay,
                pending: LiveWalPersistedPendingV1::ValidateSignBound(pending),
            }),
        }
    }

    /// Consume one exact local `ProposalIntent` continuation into its
    /// standalone WAL-owned lifecycle admission.
    ///
    /// The local-body composite remains nested in the prepared admission as
    /// companion provenance. Its inherited pending owner is intentionally not
    /// substituted for the locator-derived WAL owner, so live admission and
    /// cold reconstruction select the same causal root.
    #[allow(clippy::result_large_err)]
    pub(in crate::sumeragi) fn into_live_local_proposal_admission(
        self,
        active_context: LifecycleContext,
        verified: &VerifiedHeightContext,
        companion: LocalProposalIntentReplayEvidenceV1,
    ) -> Result<
        super::work_registry::PreparedLifecycleAdmissionV1,
        (
            Self,
            LocalProposalIntentReplayEvidenceV1,
            AdapterEffectAdmissionError,
        ),
    > {
        if !matches!(&self.pending, LiveWalPersistedPendingV1::PayloadFree(_))
            || !companion.exactly_matches_live_wal_sign_effect(&self.effect)
            || !matches!(
                &self.effect,
                AdapterEffect::Sign {
                    request: SignRequest::Proposal(_),
                    ..
                }
            )
            || !self
                .replay
                .exactly_matches_payload_free_effect(&self.effect)
        {
            return Err((self, companion, AdapterEffectAdmissionError::InvalidCarrier));
        }
        let Self {
            effect,
            replay,
            pending,
        } = self;
        let LiveWalPersistedPendingV1::PayloadFree(pending) = pending else {
            unreachable!("local Proposal WAL seal retains its locator-derived pending owner")
        };
        let LiveWalPersistedReplayStateV1::Canonical {
            authority: replay_authority,
            ..
        } = &replay.state
        else {
            unreachable!("local Proposal WAL seal retains canonical replay authority")
        };
        match super::work_registry::PreparedLifecycleAdmissionV1::live_wal_local_proposal(
            active_context,
            verified,
            effect,
            pending,
            replay_authority.clone(),
            companion,
        ) {
            Ok(prepared) => Ok(prepared),
            Err((failure, effect, pending, _authority, companion)) => Err((
                Self {
                    effect,
                    replay,
                    pending: LiveWalPersistedPendingV1::PayloadFree(pending),
                },
                companion,
                failure,
            )),
        }
    }
    /// Compare the complete test effect and expected inherited causal key
    /// without releasing either sealed value.
    #[cfg(test)]
    pub(in crate::sumeragi) fn exactly_matches_validate_sign_for_test(
        &self,
        effect: &AdapterEffect,
        causal_key: &iroha_crypto::Hash,
    ) -> bool {
        self.effect == *effect
            && matches!(
                &self.pending,
                LiveWalPersistedPendingV1::ValidateSignBound(pending)
                    if pending.causal_lifecycle_key() == causal_key
            )
            && self.exactly_binds_validate_sign_pending()
    }
    /// Complete `Apply` only from the exact retained Validate causal owner.
    ///
    /// The work-registry Validate join is its sole production caller and keeps
    /// the receipt-bound completion private to that atomic transition.
    #[allow(clippy::result_large_err)]
    pub(super) fn complete_exact_apply(
        self,
        predecessor_effect: &AdapterEffect,
        predecessor_pending: &PendingRuntimeEffectBinding,
        child_pending: PendingRuntimeEffectBinding,
        receipt: &DurableBodyReceipt,
    ) -> Result<Self, (Self, PendingRuntimeEffectBinding)> {
        if !matches!(&self.pending, LiveWalPersistedPendingV1::ApplyPending)
            || !predecessor_pending
                .project_validate_apply_successor(predecessor_effect, &self.effect)
                .is_some_and(|expected| expected == child_pending)
        {
            return Err((self, child_pending));
        }
        let Self {
            effect,
            replay,
            pending: LiveWalPersistedPendingV1::ApplyPending,
        } = self
        else {
            unreachable!("preflight admitted only the pending Apply state")
        };
        match replay.complete_exact_apply(&effect, receipt) {
            Ok(replay) => Ok(Self {
                effect,
                replay,
                pending: LiveWalPersistedPendingV1::ApplyBound(child_pending),
            }),
            Err(replay) => Err((
                Self {
                    effect,
                    replay,
                    pending: LiveWalPersistedPendingV1::ApplyPending,
                },
                child_pending,
            )),
        }
    }
    fn exactly_matches_effect(&self) -> bool {
        match &self.pending {
            LiveWalPersistedPendingV1::PayloadFree(pending) => {
                pending.exactly_binds_adapter_effect(&self.effect)
                    && self
                        .replay
                        .exactly_matches_payload_free_effect(&self.effect)
            }
            LiveWalPersistedPendingV1::ValidateSignBound(pending) => {
                pending.exactly_binds_adapter_effect(&self.effect)
                    && self
                        .replay
                        .exactly_matches_payload_free_effect(&self.effect)
            }
            LiveWalPersistedPendingV1::ApplyPending => {
                self.replay.exactly_matches_persisted_effect(&self.effect)
            }
            LiveWalPersistedPendingV1::ApplyBound(_) => false,
        }
    }
}
impl LiveWalPersistedReplaySealV1 {
    /// Seal one exact effect released by an acknowledged live WAL append.
    ///
    /// This mint accepts the complete opaque runtime identity, never locator
    /// scalars. Its only production caller is the adapter's closed persisted-
    /// continuation conversion after matching the exact reducer WAL record.
    fn from_exact_persisted_effect(
        wal_identity: LiveWalFrameIdentity,
        effect: &AdapterEffect,
    ) -> Option<Self> {
        let LiveWalReplayProjectionV1 {
            context,
            stage,
            source,
        } = exact_live_wal_replay_projection(&wal_identity, effect)?;
        let state = if stage == LifecycleStageKind::ApplyDecision {
            canonical_wal_source(&source)
                .then_some(LiveWalPersistedReplayStateV1::ApplyPending { context, source })?
        } else {
            let authority = canonical_replay_authority(
                context,
                LifecycleReplaySourceV1::Wal(source),
                stage,
                ReplayPayloadBindingV1::None,
            )?;
            LiveWalPersistedReplayStateV1::Canonical { stage, authority }
        };
        let seal = Self {
            wal_identity,
            state,
        };
        seal.exactly_matches_persisted_effect(effect)
            .then_some(seal)
    }
    /// Recheck the exact locator, action, tag, and payload-free V1 envelope.
    fn exactly_matches_payload_free_effect(&self, effect: &AdapterEffect) -> bool {
        matches!(
            &self.state,
            LiveWalPersistedReplayStateV1::Canonical { stage, .. }
                if *stage != LifecycleStageKind::ApplyDecision
        ) && self.exactly_matches_persisted_effect(effect)
    }
    /// Complete only an exact pending `Apply` source with one durable body frame.
    ///
    /// The work registry calls this through its closed validated-completion
    /// join; no public constructor accepts a receipt or body-frame parts.
    #[allow(clippy::result_large_err)]
    fn complete_exact_apply(
        self,
        effect: &AdapterEffect,
        receipt: &DurableBodyReceipt,
    ) -> Result<Self, Self> {
        let Self {
            wal_identity,
            state,
        } = self;
        let (context, source) = match state {
            LiveWalPersistedReplayStateV1::ApplyPending { context, source } => (context, source),
            other => {
                return Err(Self {
                    wal_identity,
                    state: other,
                });
            }
        };
        let Some(frame) = durable_body_frame_reference(context, receipt) else {
            return Err(Self {
                wal_identity,
                state: LiveWalPersistedReplayStateV1::ApplyPending { context, source },
            });
        };
        let payload =
            ReplayPayloadBindingV1::from_payload(DurablePayloadReference::BodyFrame(frame));
        let Some(authority) = canonical_replay_authority(
            context,
            LifecycleReplaySourceV1::Wal(source.clone()),
            LifecycleStageKind::ApplyDecision,
            payload,
        ) else {
            return Err(Self {
                wal_identity,
                state: LiveWalPersistedReplayStateV1::ApplyPending { context, source },
            });
        };
        let completed = Self {
            wal_identity,
            state: LiveWalPersistedReplayStateV1::Canonical {
                stage: LifecycleStageKind::ApplyDecision,
                authority,
            },
        };
        if completed.exactly_matches_apply_effect(effect, receipt) {
            Ok(completed)
        } else {
            let Self { wal_identity, .. } = completed;
            Err(Self {
                wal_identity,
                state: LiveWalPersistedReplayStateV1::ApplyPending { context, source },
            })
        }
    }
    /// Recheck the exact live WAL source and receipt-bound `Apply` envelope.
    fn exactly_matches_apply_effect(
        &self,
        effect: &AdapterEffect,
        receipt: &DurableBodyReceipt,
    ) -> bool {
        if !matches!(
            &self.state,
            LiveWalPersistedReplayStateV1::Canonical {
                stage: LifecycleStageKind::ApplyDecision,
                ..
            }
        ) {
            return false;
        }
        let Some(LiveWalReplayProjectionV1 {
            context,
            stage: LifecycleStageKind::ApplyDecision,
            source,
        }) = exact_live_wal_replay_projection(&self.wal_identity, effect)
        else {
            return false;
        };
        let Some(frame) = durable_body_frame_reference(context, receipt) else {
            return false;
        };
        let payload =
            ReplayPayloadBindingV1::from_payload(DurablePayloadReference::BodyFrame(frame));
        let Some(expected) = canonical_replay_authority(
            context,
            LifecycleReplaySourceV1::Wal(source),
            LifecycleStageKind::ApplyDecision,
            payload,
        ) else {
            return false;
        };
        matches!(
            &self.state,
            LiveWalPersistedReplayStateV1::Canonical {
                stage: LifecycleStageKind::ApplyDecision,
                authority,
            } if authority == &expected
        )
    }
    fn exactly_matches_persisted_effect(&self, effect: &AdapterEffect) -> bool {
        let Some(LiveWalReplayProjectionV1 {
            context,
            stage,
            source,
        }) = exact_live_wal_replay_projection(&self.wal_identity, effect)
        else {
            return false;
        };
        match &self.state {
            LiveWalPersistedReplayStateV1::Canonical {
                stage: retained_stage,
                authority,
            } => {
                *retained_stage == stage
                    && stage != LifecycleStageKind::ApplyDecision
                    && canonical_replay_authority(
                        context,
                        LifecycleReplaySourceV1::Wal(source),
                        stage,
                        ReplayPayloadBindingV1::None,
                    )
                    .is_some_and(|expected| &expected == authority)
            }
            LiveWalPersistedReplayStateV1::ApplyPending {
                context: retained_context,
                source: retained_source,
            } => {
                stage == LifecycleStageKind::ApplyDecision
                    && *retained_context == context
                    && retained_source == &source
                    && canonical_wal_source(retained_source)
            }
        }
    }
}
struct LiveWalReplayProjectionV1 {
    context: LifecycleContext,
    stage: LifecycleStageKind,
    source: WalReplaySourceV1,
}
