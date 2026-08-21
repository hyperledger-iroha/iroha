/// Stable process-local identity of one runtime-bound lifecycle output handoff.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(in crate::sumeragi) struct LifecycleOutputAdmissionKeyV1 {
    causal_lifecycle_key: [u8; 32],
    effect_identity: [u8; 32],
}

/// One runtime output whose exact binding must rejoin an existing lifecycle
/// row or enter the direct-signed admission transaction before service I/O.
#[derive(Debug)]
#[must_use = "a lifecycle output handoff must be settled or retained intact"]
pub(in crate::sumeragi) struct PendingLifecycleOutputAdmissionV1 {
    effect: AdapterEffect,
    pending: PendingRuntimeEffectBinding,
    ownership: RuntimeEffectOwnership,
}

/// Ownership-preserving failure while sealing one lifecycle output handoff.
#[derive(Debug)]
pub(in crate::sumeragi) struct PendingLifecycleOutputAdmissionErrorV1 {
    _effect: AdapterEffect,
    _ownership: RuntimeEffectOwnership,
}

/// Exact output execution authority retained beside one admitted registry row.
#[derive(Debug)]
#[must_use = "an admitted lifecycle output must execute or fail closed"]
pub(super) struct PreparedLifecycleOutputExecutionV1 {
    effect: AdapterEffect,
    ownership: RuntimeEffectOwnership,
}

/// Ownership-preserving direct-signed preparation failure.
#[derive(Debug)]
pub(super) struct LifecycleOutputAdmissionPreparationErrorV1 {
    pub(super) failure: AdapterEffectAdmissionError,
    pub(super) pending: PendingLifecycleOutputAdmissionV1,
}

impl PendingLifecycleOutputAdmissionV1 {
    /// Seal exactly one output class with its mandatory runtime binding.
    #[allow(clippy::result_large_err)]
    pub(in crate::sumeragi) fn seal_exact(
        effect: AdapterEffect,
        ownership: RuntimeEffectOwnership,
    ) -> Result<Self, PendingLifecycleOutputAdmissionErrorV1> {
        let pending = match ownership.exact_pending_adapter_effect_binding(&effect) {
            Ok(pending) => pending,
            Err(_) => {
                return Err(PendingLifecycleOutputAdmissionErrorV1 {
                    _effect: effect,
                    _ownership: ownership,
                });
            }
        };
        if !matches!(
            effect,
            AdapterEffect::Broadcast(_)
                | AdapterEffect::ReportEquivocation { .. }
                | AdapterEffect::ReportInvalidCertifiedBody { .. }
        ) {
            return Err(PendingLifecycleOutputAdmissionErrorV1 {
                _effect: effect,
                _ownership: ownership,
            });
        }
        Ok(Self {
            effect,
            pending,
            ownership,
        })
    }

    /// Return the non-authorizing map identity used only for bounded retention.
    pub(in crate::sumeragi) fn key(&self) -> LifecycleOutputAdmissionKeyV1 {
        LifecycleOutputAdmissionKeyV1 {
            causal_lifecycle_key: *self.pending.causal_lifecycle_key().as_ref(),
            effect_identity: *self.pending.exact_effect_identity().as_ref(),
        }
    }

    /// Project only the immutable runtime lifecycle owner for rank census.
    pub(in crate::sumeragi) fn lifecycle_owner(&self) -> RuntimeLifecycleOwner {
        self.ownership.owner().clone()
    }

    /// Recheck an exact retry without replacing the retained move-only owner.
    pub(in crate::sumeragi) fn exactly_matches_retry(
        &self,
        effect: &AdapterEffect,
        ownership: &RuntimeEffectOwnership,
    ) -> bool {
        ownership
            .exact_pending_adapter_effect_binding(effect)
            .is_ok_and(|pending| {
                self.effect == *effect
                    && self.pending == pending
                    && self
                        .ownership
                        .exactly_matches_bound_effect_occurrence(ownership, effect)
            })
    }

    /// Consume a handoff whose registry row was already installed by a live
    /// WAL or rejected-Validate successor transaction.
    pub(super) fn into_existing_execution(self) -> PreparedLifecycleOutputExecutionV1 {
        debug_assert!(self.pending.exactly_binds_adapter_effect(&self.effect));
        PreparedLifecycleOutputExecutionV1 {
            effect: self.effect,
            ownership: self.ownership,
        }
    }

    /// Prepare a genuinely direct output for the sole admission transaction.
    /// Proposal output must already be WAL-owned because its canonical chunk
    /// payload is not contained in a payload-free direct-signed LedgerV1 row.
    #[allow(clippy::result_large_err)]
    pub(super) fn prepare_direct_signed(
        self,
        active_context: LifecycleContext,
        verified: &VerifiedHeightContext,
    ) -> Result<
        (
            PreparedLifecycleAdmissionV1,
            PreparedLifecycleOutputExecutionV1,
        ),
        LifecycleOutputAdmissionPreparationErrorV1,
    > {
        let is_direct = matches!(
            &self.effect,
            AdapterEffect::Broadcast(message)
                if !matches!(
                    &message.payload,
                    wire::ConsensusMessageV2Payload::Proposal(_)
                )
        ) || matches!(&self.effect, AdapterEffect::ReportEquivocation { .. });
        if !is_direct {
            return Err(LifecycleOutputAdmissionPreparationErrorV1 {
                failure: AdapterEffectAdmissionError::InvalidCarrier,
                pending: self,
            });
        }
        let Self {
            effect,
            pending,
            ownership,
        } = self;
        let execution = PreparedLifecycleOutputExecutionV1 {
            effect: effect.clone(),
            ownership,
        };
        match PreparedLifecycleAdmissionV1::direct_signed(active_context, verified, effect, pending)
        {
            Ok(prepared) => Ok((prepared, execution)),
            Err(PreparedLifecycleAdmissionErrorV1::Binding(error)) => {
                let BoundAdapterEffectErrorV1 { effect, pending } = error;
                Err(LifecycleOutputAdmissionPreparationErrorV1 {
                    failure: AdapterEffectAdmissionError::UnboundEffect,
                    pending: PendingLifecycleOutputAdmissionV1 {
                        effect,
                        pending,
                        ownership: execution.ownership,
                    },
                })
            }
            Err(PreparedLifecycleAdmissionErrorV1::Projection { failure, bound }) => {
                let BoundAdapterEffectV1 {
                    effect,
                    pending,
                    replay_origin: _,
                } = bound;
                Err(LifecycleOutputAdmissionPreparationErrorV1 {
                    failure,
                    pending: PendingLifecycleOutputAdmissionV1 {
                        effect,
                        pending,
                        ownership: execution.ownership,
                    },
                })
            }
        }
    }

    /// Reclaim a non-committing direct admission with its exact execution owner.
    pub(super) fn reclaim_returned(
        prepared: PreparedLifecycleAdmissionV1,
        execution: PreparedLifecycleOutputExecutionV1,
    ) -> Self {
        let PreparedLifecycleAdmissionV1 {
            owner,
            candidate: _,
        } = prepared;
        let PreparedLifecycleAdmissionOwnerV1::DirectSigned(bound) = owner else {
            unreachable!("direct output settlement returned another admission origin")
        };
        let BoundAdapterEffectV1 {
            effect,
            pending,
            replay_origin: _,
        } = bound;
        assert_eq!(effect, execution.effect);
        assert_eq!(pending, execution.exact_pending_binding());
        Self {
            effect,
            pending,
            ownership: execution.ownership,
        }
    }
}

impl PreparedLifecycleOutputExecutionV1 {
    /// Re-derive the mandatory pending binding from the retained runtime owner.
    fn exact_pending_binding(&self) -> PendingRuntimeEffectBinding {
        self.ownership
            .exact_pending_adapter_effect_binding(&self.effect)
            .expect("sealed lifecycle output execution retains its exact binding")
    }
    /// Execute without separating the effect from its exact runtime owner.
    pub(super) fn execute_with<T, E>(
        &self,
        execute: impl FnOnce(&AdapterEffect, &RuntimeEffectOwnership) -> Result<T, E>,
    ) -> Result<T, E> {
        execute(&self.effect, &self.ownership)
    }

    /// Restore the exact bounded handoff when lifecycle ordering defers I/O.
    pub(super) fn into_pending(self) -> PendingLifecycleOutputAdmissionV1 {
        let pending = self.exact_pending_binding();
        PendingLifecycleOutputAdmissionV1 {
            effect: self.effect,
            pending,
            ownership: self.ownership,
        }
    }
}

/// Closed classification of the replay source retained by one bound adapter effect.
///
/// Every variant carries a required canonical replay authority.  The enum is
/// deliberately private: only the origin-specific replay seals may mint one,
/// and admission never accepts an authority or an origin tag as caller parts.
#[derive(Debug)]
enum BoundAdapterReplayOriginV1 {
    /// An effect emitted only after its live WAL record was fsynced.
    LiveWal(LifecycleReplayAuthorityV1),
    /// A deterministic invalid-body report descended from its rejected Validate.
    InvalidBodyReport(LifecycleReplayAuthorityV1),
    /// A complete signed Broadcast or authenticated equivocation report.
    DirectSigned(LifecycleReplayAuthorityV1),
}
impl BoundAdapterReplayOriginV1 {
    const fn authority(&self) -> &LifecycleReplayAuthorityV1 {
        match self {
            Self::LiveWal(authority)
            | Self::InvalidBodyReport(authority)
            | Self::DirectSigned(authority) => authority,
        }
    }
    fn exactly_matches_bound_effect(
        &self,
        effect: &AdapterEffect,
        pending: &PendingRuntimeEffectBinding,
    ) -> bool {
        pending.exactly_binds_adapter_effect(effect)
            && match self {
                Self::LiveWal(authority) => authority.is_live_wal_origin(),
                Self::InvalidBodyReport(authority) => authority.is_invalid_body_report_origin(),
                Self::DirectSigned(authority) => {
                    authority.is_direct_signed_origin()
                        && exact_direct_signed_admission_authority(effect, pending).as_ref()
                            == Some(authority)
                }
            }
    }
}
/// One executor-bound adapter effect with mandatory pending and replay authority.
///
/// This value is move-only.  It has no optional binding conversion and no
/// effect/owner parts accessor.  A concrete registry row may be constructed
/// only while the same value still authorizes the exact prepared candidate.
#[derive(Debug)]
#[must_use = "a bound adapter effect has not entered lifecycle admission"]
pub(super) struct BoundAdapterEffectV1 {
    effect: AdapterEffect,
    pending: PendingRuntimeEffectBinding,
    replay_origin: BoundAdapterReplayOriginV1,
}

/// Origin-specific companion retained beside one live-WAL admission.
///
/// Most live WAL children are published by an already-closed lifecycle
/// transition. A locally assembled Proposal is different: its standalone WAL
/// owner must retain the consumed local-body lineage beside it until the
/// lifecycle Sign carrier advances to its signed Broadcast.
#[derive(Debug)]
enum PreparedLiveWalCompanionV1 {
    None,
    LocalProposal(LocalProposalIntentReplayEvidenceV1),
}

/// One WAL-bound admission plus any move-only lineage required by its live
/// process carrier.
#[derive(Debug)]
pub(super) struct PreparedLiveWalAdmissionV1 {
    bound: BoundAdapterEffectV1,
    companion: PreparedLiveWalCompanionV1,
}

impl PreparedLiveWalAdmissionV1 {
    fn bound_only(bound: BoundAdapterEffectV1) -> Self {
        Self {
            bound,
            companion: PreparedLiveWalCompanionV1::None,
        }
    }

    fn local_proposal(
        bound: BoundAdapterEffectV1,
        companion: LocalProposalIntentReplayEvidenceV1,
    ) -> Result<Self, (BoundAdapterEffectV1, LocalProposalIntentReplayEvidenceV1)> {
        if companion.exactly_matches_live_wal_sign_effect(&bound.effect) {
            Ok(Self {
                bound,
                companion: PreparedLiveWalCompanionV1::LocalProposal(companion),
            })
        } else {
            Err((bound, companion))
        }
    }

    fn exactly_authorizes_candidate(
        &self,
        active_context: LifecycleContext,
        candidate: &CandidateAdmission,
    ) -> bool {
        self.bound
            .exactly_authorizes_candidate(active_context, candidate)
            && match &self.companion {
                PreparedLiveWalCompanionV1::None => true,
                PreparedLiveWalCompanionV1::LocalProposal(companion) => {
                    companion.exactly_matches_live_wal_sign_effect(&self.bound.effect)
                        && matches!(
                            &self.bound.effect,
                            AdapterEffect::Sign {
                                request: SignRequest::Proposal(_),
                                ..
                            }
                        )
                }
            }
    }
}
/// Ownership-preserving failure while binding mandatory replay authority.
#[derive(Debug)]
pub(super) struct BoundAdapterEffectErrorV1 {
    effect: AdapterEffect,
    pending: PendingRuntimeEffectBinding,
}
#[cfg(test)]
impl BoundAdapterEffectErrorV1 {
    /// Check that a failed bind returned the submitted effect and its foreign owner.
    pub(super) fn returns_foreign_owner_for_test(
        &self,
        submitted: &AdapterEffect,
        actual_owner: &AdapterEffect,
    ) -> bool {
        self.effect == *submitted
            && !self.pending.exactly_binds_adapter_effect(submitted)
            && self.pending.exactly_binds_adapter_effect(actual_owner)
    }
}
impl BoundAdapterEffectV1 {
    /// Bind one child emitted only after its canonical safety-WAL frame was fsynced.
    #[allow(clippy::result_large_err)]
    pub(in crate::sumeragi) fn bind_live_wal(
        effect: AdapterEffect,
        pending: PendingRuntimeEffectBinding,
        authority: LifecycleReplayAuthorityV1,
    ) -> Result<
        Self,
        (
            AdapterEffect,
            PendingRuntimeEffectBinding,
            LifecycleReplayAuthorityV1,
        ),
    > {
        let is_wal_child = matches!(
            effect,
            AdapterEffect::Sign { .. } | AdapterEffect::Apply { .. } | AdapterEffect::Broadcast(_)
        );
        let bound = Self {
            effect,
            pending,
            replay_origin: BoundAdapterReplayOriginV1::LiveWal(authority),
        };
        if !is_wal_child || !bound.validates() {
            let Self {
                effect,
                pending,
                replay_origin,
            } = bound;
            let BoundAdapterReplayOriginV1::LiveWal(authority) = replay_origin else {
                unreachable!("live-WAL bind retained another replay origin")
            };
            return Err((effect, pending, authority));
        }
        Ok(bound)
    }
    /// Bind one complete signed effect to its only canonical direct replay origin.
    #[allow(clippy::result_large_err)]
    pub(super) fn bind_direct_signed(
        effect: AdapterEffect,
        pending: PendingRuntimeEffectBinding,
    ) -> Result<Self, BoundAdapterEffectErrorV1> {
        let Some(authority) = exact_direct_signed_admission_authority(&effect, &pending) else {
            return Err(BoundAdapterEffectErrorV1 { effect, pending });
        };
        let bound = Self {
            effect,
            pending,
            replay_origin: BoundAdapterReplayOriginV1::DirectSigned(authority),
        };
        if !bound.validates() {
            let Self {
                effect,
                pending,
                replay_origin: _,
            } = bound;
            return Err(BoundAdapterEffectErrorV1 { effect, pending });
        }
        Ok(bound)
    }
    /// Bind one report only while the canonical rejected-Validate replay seal
    /// supplies the private one-shot permit and exact authority.
    #[allow(clippy::result_large_err)]
    pub(in crate::sumeragi) fn bind_invalid_body_report(
        _permit: InvalidBodyReportBoundEffectPermit,
        effect: AdapterEffect,
        pending: PendingRuntimeEffectBinding,
        authority: LifecycleReplayAuthorityV1,
    ) -> Result<
        Self,
        (
            AdapterEffect,
            PendingRuntimeEffectBinding,
            LifecycleReplayAuthorityV1,
        ),
    > {
        let is_report = matches!(effect, AdapterEffect::ReportInvalidCertifiedBody { .. });
        let bound = Self {
            effect,
            pending,
            replay_origin: BoundAdapterReplayOriginV1::InvalidBodyReport(authority),
        };
        if !is_report || !bound.validates() {
            let Self {
                effect,
                pending,
                replay_origin,
            } = bound;
            let BoundAdapterReplayOriginV1::InvalidBodyReport(authority) = replay_origin else {
                unreachable!("invalid-body bind retained another replay origin")
            };
            return Err((effect, pending, authority));
        }
        Ok(bound)
    }
    fn validates(&self) -> bool {
        self.replay_origin
            .exactly_matches_bound_effect(&self.effect, &self.pending)
    }
    fn project_candidate(
        &self,
        active_context: LifecycleContext,
        verified: &VerifiedHeightContext,
    ) -> Result<CandidateAdmission, AdapterEffectAdmissionError> {
        if !self.validates() {
            return Err(AdapterEffectAdmissionError::UnboundEffect);
        }
        let projected = projection::authority_free_admission_projection(
            active_context,
            verified,
            &self.effect,
            &self.pending,
        )?;
        let candidate = CandidateAdmission::new(
            projected.key,
            projected.causal_root,
            projected.work_class,
            projected.stage,
            projected.initial_state,
            projected.reconstruction_source,
            DurablePayloadReference::None,
            self.replay_origin.authority().clone(),
            projected.physical_geometry,
            None,
        );
        if !candidate.replay_authority_is_exact(active_context)
            || !self.exactly_authorizes_candidate(active_context, &candidate)
        {
            return Err(AdapterEffectAdmissionError::InvalidCarrier);
        }
        Ok(candidate)
    }
    fn exactly_authorizes_candidate(
        &self,
        active_context: LifecycleContext,
        candidate: &CandidateAdmission,
    ) -> bool {
        self.validates()
            && candidate.causal_root
                == super::CausalRoot::new(digest_from_hash(self.pending.causal_lifecycle_key()))
            && candidate.reconstruction_source == candidate.causal_root.digest()
            && candidate.payload == DurablePayloadReference::None
            && candidate.replay_authority == *self.replay_origin.authority()
            && candidate.replay_authority_is_exact(active_context)
            && candidate.physical_geometry.normalized().is_ok_and(
                |(physical, universe, consumed)| {
                    let slot =
                        PhysicalSlotId::for_capacity(candidate.work_class.capacity_class(), 0);
                    physical
                        == BTreeMap::from([(
                            slot,
                            digest_from_hash(self.pending.exact_effect_identity()),
                        )])
                        && universe == std::collections::BTreeSet::from([slot])
                        && consumed == universe
                },
            )
    }
    #[cfg(test)]
    /// Check the retained exact effect and mandatory replay origin.
    pub(super) fn exactly_binds_for_test(&self, effect: &AdapterEffect) -> bool {
        self.effect == *effect && self.validates()
    }
    #[cfg(test)]
    fn replace_with_foreign_origin_for_test(&mut self) -> bool {
        let BoundAdapterReplayOriginV1::DirectSigned(authority) = core::mem::replace(
            &mut self.replay_origin,
            BoundAdapterReplayOriginV1::DirectSigned(
                exact_direct_signed_admission_authority(&self.effect, &self.pending)
                    .expect("exact direct-signed fixture retains canonical authority"),
            ),
        ) else {
            return false;
        };
        self.replay_origin = BoundAdapterReplayOriginV1::InvalidBodyReport(authority);
        !self.validates()
    }
    #[cfg(test)]
    fn has_foreign_origin_for_test(&self, effect: &AdapterEffect) -> bool {
        self.effect == *effect
            && self.pending.exactly_binds_adapter_effect(effect)
            && matches!(
                &self.replay_origin,
                BoundAdapterReplayOriginV1::InvalidBodyReport(authority)
                    if authority.is_direct_signed_origin()
            )
            && !self.validates()
    }
}
/// One move-only, replay-authorized candidate ready for the atomic admission cut.
///
/// Candidate shape and durable replay metadata are derived together from the
/// bound owner.  Callers cannot supply a detached candidate or replace the
/// mandatory replay origin after preparation.
#[derive(Debug)]
#[must_use = "prepared lifecycle admission must be admitted, returned, or failed intact"]
pub(in crate::sumeragi) struct PreparedLifecycleAdmissionV1 {
    owner: PreparedLifecycleAdmissionOwnerV1,
    candidate: CandidateAdmission,
}
pub(super) enum PreparedLifecycleAdmissionOwnerV1 {
    /// A child effect authenticated by one fsynced safety-WAL frame.
    LiveWal(PreparedLiveWalAdmissionV1),
    /// Durable Validate work descended from one locally assembled body.
    LocalBody(PreparedLocalBodyValidateReplayPreAdmission),
    /// Durable Validate work descended from one authenticated Proposal.
    RemoteProposal(PreparedRemoteProposalValidateReplayPreAdmission),
    /// One deterministic rejected-Validate diagnostic child.
    InvalidBodyReport(BoundAdapterEffectV1),
    /// One complete signed Broadcast or authenticated equivocation report.
    DirectSigned(BoundAdapterEffectV1),
}
impl fmt::Debug for PreparedLifecycleAdmissionOwnerV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::LiveWal(_) => "LiveWal(..)",
            Self::LocalBody(_) => "LocalBody(..)",
            Self::RemoteProposal(_) => "RemoteProposal(..)",
            Self::InvalidBodyReport(_) => "InvalidBodyReport(..)",
            Self::DirectSigned(_) => "DirectSigned(..)",
        })
    }
}
/// Ownership-preserving failure before a prepared admission exists.
#[derive(Debug)]
pub(super) enum PreparedLifecycleAdmissionErrorV1 {
    /// The effect and pending owner did not mint one mandatory replay origin.
    Binding(BoundAdapterEffectErrorV1),
    /// Exact runtime projection rejected the bound owner or active context.
    Projection {
        /// Closed projection failure.
        failure: AdapterEffectAdmissionError,
        /// The complete bound owner returned unchanged.
        bound: BoundAdapterEffectV1,
    },
}
impl PreparedLifecycleAdmissionV1 {
    /// Prepare one exact signed effect without exposing a generic candidate seam.
    #[allow(clippy::result_large_err)]
    pub(super) fn direct_signed(
        active_context: LifecycleContext,
        verified: &VerifiedHeightContext,
        effect: AdapterEffect,
        pending: PendingRuntimeEffectBinding,
    ) -> Result<Self, PreparedLifecycleAdmissionErrorV1> {
        let bound = BoundAdapterEffectV1::bind_direct_signed(effect, pending)
            .map_err(PreparedLifecycleAdmissionErrorV1::Binding)?;
        let candidate = match bound.project_candidate(active_context, verified) {
            Ok(candidate) => candidate,
            Err(failure) => {
                return Err(PreparedLifecycleAdmissionErrorV1::Projection { failure, bound });
            }
        };
        Ok(Self {
            owner: PreparedLifecycleAdmissionOwnerV1::DirectSigned(bound),
            candidate,
        })
    }
    /// Borrow the derived candidate only inside the adjacent atomic transaction.
    pub(super) const fn candidate(&self) -> &CandidateAdmission {
        &self.candidate
    }
    /// Recheck the complete prepared owner against its active context.
    pub(super) fn validates(&self, active_context: LifecycleContext) -> bool {
        match &self.owner {
            PreparedLifecycleAdmissionOwnerV1::LiveWal(live) => {
                live.exactly_authorizes_candidate(active_context, &self.candidate)
            }
            PreparedLifecycleAdmissionOwnerV1::InvalidBodyReport(bound)
            | PreparedLifecycleAdmissionOwnerV1::DirectSigned(bound) => {
                bound.exactly_authorizes_candidate(active_context, &self.candidate)
            }
            PreparedLifecycleAdmissionOwnerV1::LocalBody(validate) => {
                validate.exactly_authorizes_candidate(active_context, &self.candidate)
            }
            PreparedLifecycleAdmissionOwnerV1::RemoteProposal(validate) => {
                validate.exactly_authorizes_candidate(active_context, &self.candidate)
            }
        }
    }
    /// Consume the prepared owner only inside the adjacent registry transaction.
    pub(super) fn into_owner(self) -> PreparedLifecycleAdmissionOwnerV1 {
        self.owner
    }
    fn is_live_wal_local_proposal(&self) -> bool {
        matches!(
            &self.owner,
            PreparedLifecycleAdmissionOwnerV1::LiveWal(PreparedLiveWalAdmissionV1 {
                bound,
                companion: PreparedLiveWalCompanionV1::LocalProposal(companion),
            }) if companion.exactly_matches_live_wal_sign_effect(&bound.effect)
        ) && self.validates(LifecycleContext::new(
            self.candidate.key.context(),
            self.candidate.key.round().height(),
        ))
    }
    /// Reconstitute the exact prepared owner after a reversible registry failure.
    pub(super) fn from_returned_bound(
        active_context: LifecycleContext,
        candidate: CandidateAdmission,
        bound: BoundAdapterEffectV1,
    ) -> Option<Self> {
        let owner = match &bound.replay_origin {
            BoundAdapterReplayOriginV1::LiveWal(_) => PreparedLifecycleAdmissionOwnerV1::LiveWal(
                PreparedLiveWalAdmissionV1::bound_only(bound),
            ),
            BoundAdapterReplayOriginV1::InvalidBodyReport(_) => {
                PreparedLifecycleAdmissionOwnerV1::InvalidBodyReport(bound)
            }
            BoundAdapterReplayOriginV1::DirectSigned(_) => {
                PreparedLifecycleAdmissionOwnerV1::DirectSigned(bound)
            }
        };
        let prepared = Self { owner, candidate };
        prepared.validates(active_context).then_some(prepared)
    }
    pub(super) fn from_returned_live_wal(
        active_context: LifecycleContext,
        candidate: CandidateAdmission,
        live: PreparedLiveWalAdmissionV1,
    ) -> Option<Self> {
        let prepared = Self {
            owner: PreparedLifecycleAdmissionOwnerV1::LiveWal(live),
            candidate,
        };
        prepared.validates(active_context).then_some(prepared)
    }

    /// Prepare one live Proposal Sign from its WAL-derived standalone owner
    /// while retaining the consumed local-body lineage as companion evidence.
    #[allow(clippy::result_large_err)]
    pub(in crate::sumeragi) fn live_wal_local_proposal(
        active_context: LifecycleContext,
        verified: &VerifiedHeightContext,
        effect: AdapterEffect,
        pending: PendingRuntimeEffectBinding,
        authority: LifecycleReplayAuthorityV1,
        companion: LocalProposalIntentReplayEvidenceV1,
    ) -> Result<
        Self,
        (
            AdapterEffectAdmissionError,
            AdapterEffect,
            PendingRuntimeEffectBinding,
            LifecycleReplayAuthorityV1,
            LocalProposalIntentReplayEvidenceV1,
        ),
    > {
        let bound = match BoundAdapterEffectV1::bind_live_wal(effect, pending, authority) {
            Ok(bound) => bound,
            Err((effect, pending, authority)) => {
                return Err((
                    AdapterEffectAdmissionError::UnboundEffect,
                    effect,
                    pending,
                    authority,
                    companion,
                ));
            }
        };
        let live = match PreparedLiveWalAdmissionV1::local_proposal(bound, companion) {
            Ok(live) => live,
            Err((bound, companion)) => {
                let BoundAdapterEffectV1 {
                    effect,
                    pending,
                    replay_origin,
                } = bound;
                let BoundAdapterReplayOriginV1::LiveWal(authority) = replay_origin else {
                    unreachable!("local Proposal admission retains live WAL replay")
                };
                return Err((
                    AdapterEffectAdmissionError::InvalidCarrier,
                    effect,
                    pending,
                    authority,
                    companion,
                ));
            }
        };
        let candidate = match live.bound.project_candidate(active_context, verified) {
            Ok(candidate) => candidate,
            Err(failure) => {
                let PreparedLiveWalAdmissionV1 { bound, companion } = live;
                let PreparedLiveWalCompanionV1::LocalProposal(companion) = companion else {
                    unreachable!("local Proposal admission retains its companion")
                };
                let BoundAdapterEffectV1 {
                    effect,
                    pending,
                    replay_origin,
                } = bound;
                let BoundAdapterReplayOriginV1::LiveWal(authority) = replay_origin else {
                    unreachable!("local Proposal admission retains live WAL replay")
                };
                return Err((failure, effect, pending, authority, companion));
            }
        };
        let prepared = Self {
            owner: PreparedLifecycleAdmissionOwnerV1::LiveWal(live),
            candidate,
        };
        if !prepared.validates(active_context) {
            let Self {
                owner,
                candidate: _,
            } = prepared;
            let PreparedLifecycleAdmissionOwnerV1::LiveWal(live) = owner else {
                unreachable!("local Proposal admission retains live WAL owner")
            };
            let PreparedLiveWalAdmissionV1 { bound, companion } = live;
            let PreparedLiveWalCompanionV1::LocalProposal(companion) = companion else {
                unreachable!("local Proposal admission retains its companion")
            };
            let BoundAdapterEffectV1 {
                effect,
                pending,
                replay_origin,
            } = bound;
            let BoundAdapterReplayOriginV1::LiveWal(authority) = replay_origin else {
                unreachable!("local Proposal admission retains live WAL replay")
            };
            return Err((
                AdapterEffectAdmissionError::InvalidCarrier,
                effect,
                pending,
                authority,
                companion,
            ));
        }
        Ok(prepared)
    }
    /// Reconstitute the exact durable Validate owner after reversible failure.
    pub(super) fn from_returned_durable_validate(
        active_context: LifecycleContext,
        candidate: CandidateAdmission,
        validate: PreparedDurableValidateAdmissionV1,
    ) -> Option<Self> {
        let owner = match validate {
            PreparedDurableValidateAdmissionV1::LocalBody(validate) => {
                PreparedLifecycleAdmissionOwnerV1::LocalBody(validate)
            }
            PreparedDurableValidateAdmissionV1::RemoteProposal(validate) => {
                PreparedLifecycleAdmissionOwnerV1::RemoteProposal(validate)
            }
        };
        let prepared = Self { owner, candidate };
        prepared.validates(active_context).then_some(prepared)
    }
    #[cfg(test)]
    /// Compare a prepared owner with the exact effect retained by a fixture.
    pub(super) fn exactly_binds_for_test(&self, effect: &AdapterEffect) -> bool {
        matches!(
            &self.owner,
            PreparedLifecycleAdmissionOwnerV1::DirectSigned(bound)
                if bound.exactly_binds_for_test(effect)
                    && bound.exactly_authorizes_candidate(
                        LifecycleContext::new(
                            self.candidate.key.context(),
                            self.candidate.key.round().height(),
                        ),
                        &self.candidate,
                    )
        )
    }
    #[cfg(test)]
    /// Replace only the replay-origin class for an exact rejection test.
    pub(super) fn replace_with_foreign_origin_for_test(&mut self) -> bool {
        match &mut self.owner {
            PreparedLifecycleAdmissionOwnerV1::DirectSigned(bound) => {
                bound.replace_with_foreign_origin_for_test()
            }
            PreparedLifecycleAdmissionOwnerV1::LiveWal(_)
            | PreparedLifecycleAdmissionOwnerV1::LocalBody(_)
            | PreparedLifecycleAdmissionOwnerV1::RemoteProposal(_)
            | PreparedLifecycleAdmissionOwnerV1::InvalidBodyReport(_) => false,
        }
    }
    #[cfg(test)]
    /// Check that a rejected fixture still retains its exact foreign origin.
    pub(super) fn has_foreign_origin_for_test(&self, effect: &AdapterEffect) -> bool {
        matches!(
            &self.owner,
            PreparedLifecycleAdmissionOwnerV1::DirectSigned(bound)
                if bound.has_foreign_origin_for_test(effect)
        )
    }
}
/// Closed authenticated-Proposal replay preflight for one ordinary Fetch.
///
/// The sole constructor consumes the exact runtime ownership which already
/// carries the receiver-authenticated signed Proposal seal. No caller can
/// supply a Proposal, ingress carrier, pending root, or replay source parts.
/// Dropping this token is publication-inert.
#[must_use = "remote Proposal Fetch replay evidence has not entered lifecycle admission"]
pub(in crate::sumeragi) struct PreparedRemoteProposalFetchReplayPreAdmission {
    effect: AdapterEffect,
    pending: PendingRuntimeEffectBinding,
    replay_evidence: RemoteProposalFetchReplayEvidenceV1,
}
/// Closed ordinary Store successor of one authenticated Proposal Fetch.
#[must_use = "remote Proposal Store replay evidence still requires its durable body receipt"]
pub(in crate::sumeragi) struct PreparedRemoteProposalStoreReplayPreAdmission {
    effect: AdapterEffect,
    pending: PendingRuntimeEffectBinding,
    replay_evidence: RemoteProposalStoreReplayEvidenceV1,
}
/// Closed durable Store replay evidence waiting for its exact Validate owner.
#[must_use = "durable remote Proposal replay evidence still requires its Validate successor"]
pub(in crate::sumeragi) struct PreparedRemoteProposalStoredReplayPreAdmission {
    store_effect: AdapterEffect,
    store_pending: PendingRuntimeEffectBinding,
    durable_receipt: DurableBodyReceipt,
    replay_evidence: RemoteProposalStoredReplayEvidenceV1,
}
/// Closed local-genesis Fetch replay awaiting its exact Store successor.
#[must_use = "authenticated genesis Fetch replay evidence has not advanced to Store"]
pub(in crate::sumeragi) struct PreparedAuthenticatedGenesisFetchReplayPreAdmission {
    effect: AdapterEffect,
    pending: PendingRuntimeEffectBinding,
    replay_evidence: super::replay_authority::AuthenticatedGenesisFetchReplayEvidenceV1,
}
/// Closed local-genesis Store replay awaiting its exact durable body frame.
#[must_use = "authenticated genesis Store replay evidence still requires durability"]
pub(in crate::sumeragi) struct PreparedAuthenticatedGenesisStoreReplayPreAdmission {
    effect: AdapterEffect,
    pending: PendingRuntimeEffectBinding,
    replay_evidence: super::replay_authority::AuthenticatedGenesisStoreReplayEvidenceV1,
}
/// Closed durable local-genesis Store replay awaiting its exact Validate successor.
#[must_use = "durable authenticated genesis replay still requires Validate admission"]
pub(in crate::sumeragi) struct PreparedAuthenticatedGenesisStoredReplayPreAdmission {
    store_effect: AdapterEffect,
    store_pending: PendingRuntimeEffectBinding,
    durable_receipt: DurableBodyReceipt,
    replay_evidence: super::replay_authority::AuthenticatedGenesisStoredReplayEvidenceV1,
}
/// Closed canonical Validate replay evidence from one signed remote Proposal.
#[must_use = "remote Proposal Validate replay evidence has not entered lifecycle admission"]
pub(in crate::sumeragi) struct PreparedRemoteProposalValidateReplayPreAdmission {
    effect: AdapterEffect,
    pending: PendingRuntimeEffectBinding,
    durable_receipt: DurableBodyReceipt,
    replay_evidence: RemoteProposalValidateReplayEvidenceV1,
}
/// Closed canonical Validate replay evidence from one locally assembled body.
#[must_use = "local body Validate replay evidence has not entered lifecycle admission"]
pub(in crate::sumeragi) struct PreparedLocalBodyValidateReplayPreAdmission {
    effect: AdapterEffect,
    pending: PendingRuntimeEffectBinding,
    durable_receipt: DurableBodyReceipt,
    replay_evidence: LocalValidateReplayEvidenceV1,
}
fn durable_validate_origin_exactly_authorizes_candidate(
    valid: bool,
    effect: &AdapterEffect,
    pending: &PendingRuntimeEffectBinding,
    durable_receipt: &DurableBodyReceipt,
    origin_is_exact: bool,
    active_context: LifecycleContext,
    candidate: &CandidateAdmission,
) -> bool {
    let AdapterEffect::ValidateBody { round, subject, .. } = effect else {
        return false;
    };
    let Some(payload) = durable_validate_body_payload(durable_receipt) else {
        return false;
    };
    let slot = PhysicalSlotId::for_capacity(LifecycleWorkClass::Validate.capacity_class(), 0);
    valid
        && active_context.height() == round.height
        && active_context.id().as_bytes() == round.context_id.0.as_ref()
        && durable_receipt.subject() == *subject
        && pending.exactly_binds_adapter_effect(effect)
        && candidate.causal_root
            == super::CausalRoot::new(digest_from_hash(pending.causal_lifecycle_key()))
        && candidate.reconstruction_source == candidate.causal_root.digest()
        && candidate.work_class == LifecycleWorkClass::Validate
        && candidate.stage
            == LifecycleStage::new(
                LifecycleStageKind::ValidateBody,
                PredecessorScope::Independent,
            )
        && candidate.initial_state == InitialLifecycleState::Ready
        && candidate.payload == payload
        && origin_is_exact
        && candidate.replay_authority_is_exact(active_context)
        && candidate
            .physical_geometry
            .normalized()
            .is_ok_and(|(physical, universe, consumed)| {
                physical
                    == BTreeMap::from([(slot, digest_from_hash(pending.exact_effect_identity()))])
                    && universe == std::collections::BTreeSet::from([slot])
                    && consumed == universe
            })
}
/// Origin-specific durable Validate owner accepted by the single admission cut.
#[must_use = "durable Validate admission must be installed or returned intact"]
pub(super) enum PreparedDurableValidateAdmissionV1 {
    RemoteProposal(PreparedRemoteProposalValidateReplayPreAdmission),
    LocalBody(PreparedLocalBodyValidateReplayPreAdmission),
}
/// Move-only owner of one origin-specific durable Validate awaiting lifecycle settlement.
///
/// This wrapper exposes neither the effect, pending binding, replay evidence,
/// nor projected candidate. Only the production lifecycle owner may prepare
/// it against its coheld logical and verified height contexts.
#[must_use = "pending durable Validate admission must be settled or retained intact"]
pub(in crate::sumeragi) struct PendingDurableValidateAdmissionV1 {
    validate: PreparedDurableValidateAdmissionV1,
}

enum PendingLiveWalSignAdmissionStateV1 {
    Sealed {
        wal: SealedLiveWalPersistedEffectV1,
        companion: LocalProposalIntentReplayEvidenceV1,
    },
    Prepared(PreparedLifecycleAdmissionV1),
}

/// Move-only local Proposal Sign awaiting the lifecycle owner's atomic WAL
/// admission transaction.
///
/// Before first projection this owns the exact post-fsync WAL seal and the
/// consumed local-body lineage. A capacity or duplicate retry retains the
/// already-projected five-origin admission intact, so no WAL identity needs to
/// be reconstructed from decoded metadata.
#[must_use = "pending live WAL Sign admission must be settled or retained intact"]
pub(in crate::sumeragi) struct PendingLiveWalSignAdmissionV1 {
    state: PendingLiveWalSignAdmissionStateV1,
}

impl fmt::Debug for PendingLiveWalSignAdmissionV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PendingLiveWalSignAdmissionV1")
            .field(
                "state",
                &match &self.state {
                    PendingLiveWalSignAdmissionStateV1::Sealed { .. } => "sealed",
                    PendingLiveWalSignAdmissionStateV1::Prepared(_) => "prepared",
                },
            )
            .finish_non_exhaustive()
    }
}

impl PendingLiveWalSignAdmissionV1 {
    /// Join the exact local ProposalIntent lineage to its post-fsync WAL seal.
    pub(in crate::sumeragi) fn from_local_proposal(
        wal: SealedLiveWalPersistedEffectV1,
        companion: LocalProposalIntentReplayEvidenceV1,
    ) -> Self {
        Self {
            state: PendingLiveWalSignAdmissionStateV1::Sealed { wal, companion },
        }
    }

    /// Prepare only against the production owner's coheld verified context.
    #[allow(clippy::result_large_err)]
    pub(super) fn prepare(
        self,
        active_context: LifecycleContext,
        verified: &VerifiedHeightContext,
    ) -> Result<PreparedLifecycleAdmissionV1, (AdapterEffectAdmissionError, Self)> {
        match self.state {
            PendingLiveWalSignAdmissionStateV1::Prepared(prepared) => Ok(prepared),
            PendingLiveWalSignAdmissionStateV1::Sealed { wal, companion } => wal
                .into_live_local_proposal_admission(active_context, verified, companion)
                .map_err(|(wal, companion, failure)| {
                    (
                        failure,
                        Self {
                            state: PendingLiveWalSignAdmissionStateV1::Sealed { wal, companion },
                        },
                    )
                }),
        }
    }

    /// Retain an already-projected admission across Retry/Capacity/failure.
    pub(super) fn reclaim_returned(prepared: PreparedLifecycleAdmissionV1) -> Self {
        assert!(
            prepared.is_live_wal_local_proposal(),
            "live WAL Sign settlement returned another prepared origin"
        );
        Self {
            state: PendingLiveWalSignAdmissionStateV1::Prepared(prepared),
        }
    }
}
impl fmt::Debug for PreparedDurableValidateAdmissionV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PreparedDurableValidateAdmissionV1")
            .field(
                "origin",
                &match self {
                    Self::RemoteProposal(_) => "remote_proposal",
                    Self::LocalBody(_) => "local_body",
                },
            )
            .finish_non_exhaustive()
    }
}
impl fmt::Debug for PendingDurableValidateAdmissionV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PendingDurableValidateAdmissionV1")
            .field(
                "origin",
                &match &self.validate {
                    PreparedDurableValidateAdmissionV1::RemoteProposal(_) => "remote_proposal",
                    PreparedDurableValidateAdmissionV1::LocalBody(_) => "local_body",
                },
            )
            .finish_non_exhaustive()
    }
}
/// Ownership-preserving failure from the authenticated Proposal Fetch cut.
pub(in crate::sumeragi) struct RemoteProposalFetchReplayPreAdmissionError {
    _effect: AdapterEffect,
    _ownership: RuntimeEffectOwnership,
}
/// Ownership-preserving failure from the fixed Fetch-to-Store projection.
pub(in crate::sumeragi) struct RemoteProposalStoreReplayPreAdmissionError {
    _fetch: PreparedRemoteProposalFetchReplayPreAdmission,
    _effect: AdapterEffect,
    _ownership: RuntimeEffectOwnership,
}
/// Ownership-preserving failure from the exact durable-receipt join.
pub(in crate::sumeragi) struct RemoteProposalDurableReplayPreAdmissionError {
    _store: PreparedRemoteProposalStoreReplayPreAdmission,
    _durable_receipt: DurableBodyReceipt,
}
/// Ownership-preserving failure from the fixed Store-to-Validate projection.
pub(in crate::sumeragi) struct RemoteProposalValidateReplayPreAdmissionError {
    _stored: PreparedRemoteProposalStoredReplayPreAdmission,
    _effect: AdapterEffect,
    _ownership: RuntimeEffectOwnership,
}
/// Ownership-preserving failure from the launch-authenticated genesis Fetch cut.
pub(in crate::sumeragi) struct AuthenticatedGenesisFetchReplayPreAdmissionError {
    _effect: AdapterEffect,
    _ownership: RuntimeEffectOwnership,
    _manifest: wire::PayloadManifest,
}
/// Ownership-preserving failure from authenticated-genesis Fetch-to-Store projection.
pub(in crate::sumeragi) struct AuthenticatedGenesisStoreReplayPreAdmissionError {
    fetch: PreparedAuthenticatedGenesisFetchReplayPreAdmission,
    _effect: AdapterEffect,
    _ownership: RuntimeEffectOwnership,
}
/// Ownership-preserving failure from the authenticated-genesis durable-frame join.
pub(in crate::sumeragi) struct AuthenticatedGenesisDurableReplayPreAdmissionError {
    store: PreparedAuthenticatedGenesisStoreReplayPreAdmission,
    _durable_receipt: DurableBodyReceipt,
}
/// Ownership-preserving failure from authenticated-genesis Store-to-Validate projection.
pub(in crate::sumeragi) struct AuthenticatedGenesisValidateReplayPreAdmissionError {
    stored: PreparedAuthenticatedGenesisStoredReplayPreAdmission,
    _effect: AdapterEffect,
    _ownership: RuntimeEffectOwnership,
}
/// Ownership-preserving failure from the local durable-Validate sealing cut.
pub(in crate::sumeragi) struct LocalBodyValidateReplayPreAdmissionError {
    _effect: AdapterEffect,
    _ownership: RuntimeEffectOwnership,
    _durable_receipt: DurableBodyReceipt,
    _replay_evidence: LocalValidateReplayEvidenceV1,
}
impl RemoteProposalStoreReplayPreAdmissionError {
    /// Return the exact Fetch owner when Store projection does not commit.
    pub(in crate::sumeragi) fn into_fetch(self) -> PreparedRemoteProposalFetchReplayPreAdmission {
        self._fetch
    }
}
impl RemoteProposalDurableReplayPreAdmissionError {
    /// Return the exact Store owner when durable receipt binding does not commit.
    pub(in crate::sumeragi) fn into_store(self) -> PreparedRemoteProposalStoreReplayPreAdmission {
        self._store
    }
}
impl RemoteProposalValidateReplayPreAdmissionError {
    /// Return the exact durable Store owner when Validate projection does not commit.
    pub(in crate::sumeragi) fn into_stored(self) -> PreparedRemoteProposalStoredReplayPreAdmission {
        self._stored
    }
}
impl AuthenticatedGenesisStoreReplayPreAdmissionError {
    /// Return the exact Fetch owner when Store projection does not commit.
    pub(in crate::sumeragi) fn into_fetch(
        self,
    ) -> PreparedAuthenticatedGenesisFetchReplayPreAdmission {
        self.fetch
    }
}
impl AuthenticatedGenesisDurableReplayPreAdmissionError {
    /// Return the exact Store owner when durable receipt binding does not commit.
    pub(in crate::sumeragi) fn into_store(
        self,
    ) -> PreparedAuthenticatedGenesisStoreReplayPreAdmission {
        self.store
    }
}
impl AuthenticatedGenesisValidateReplayPreAdmissionError {
    /// Return the exact durable Store owner when Validate projection does not commit.
    pub(in crate::sumeragi) fn into_stored(
        self,
    ) -> PreparedAuthenticatedGenesisStoredReplayPreAdmission {
        self.stored
    }
}
impl PreparedAuthenticatedGenesisFetchReplayPreAdmission {
    /// Seal one certified Fetch under the exact installed launch-authenticated genesis body.
    #[allow(clippy::result_large_err)]
    pub(in crate::sumeragi) fn seal_exact_fetch(
        context: &super::replay_authority::InstalledAuthenticatedGenesisReplayAuthorityV1,
        effect: AdapterEffect,
        ownership: RuntimeEffectOwnership,
        manifest: wire::PayloadManifest,
    ) -> Result<Self, AuthenticatedGenesisFetchReplayPreAdmissionError> {
        let Ok(pending) = ownership.exact_pending_adapter_effect_binding(&effect) else {
            return Err(AuthenticatedGenesisFetchReplayPreAdmissionError {
                _effect: effect,
                _ownership: ownership,
                _manifest: manifest,
            });
        };
        let Some(replay_evidence) = context.seal_exact_fetch(&effect, &pending, &manifest) else {
            return Err(AuthenticatedGenesisFetchReplayPreAdmissionError {
                _effect: effect,
                _ownership: ownership,
                _manifest: manifest,
            });
        };
        let sealed = Self {
            effect,
            pending,
            replay_evidence,
        };
        debug_assert!(sealed.validates());
        Ok(sealed)
    }
    fn validates(&self) -> bool {
        self.replay_evidence
            .exactly_matches_fetch_pending(&self.effect, &self.pending)
    }
    /// Authenticate a byte-identical certified Fetch rediscovery.
    pub(in crate::sumeragi) fn exactly_authenticates_fetch_rediscovery(
        &self,
        effect: &AdapterEffect,
    ) -> bool {
        self.validates() && self.replay_evidence.exactly_matches_fetch(effect)
    }
    /// Recheck an exact certified Fetch retry without replacing the local genesis origin.
    pub(in crate::sumeragi) fn exactly_matches_retry(
        &self,
        effect: &AdapterEffect,
        ownership: &RuntimeEffectOwnership,
    ) -> bool {
        ownership
            .exact_pending_adapter_effect_binding(effect)
            .is_ok_and(|pending| {
                self.validates() && self.effect == *effect && self.pending == pending
            })
    }
    /// Consume the Fetch origin only through its exact inherited Store successor.
    #[allow(clippy::result_large_err)]
    pub(in crate::sumeragi) fn project_store(
        self,
        effect: AdapterEffect,
        ownership: RuntimeEffectOwnership,
    ) -> Result<
        PreparedAuthenticatedGenesisStoreReplayPreAdmission,
        AuthenticatedGenesisStoreReplayPreAdmissionError,
    > {
        let Ok(pending) = ownership.exact_pending_adapter_effect_binding(&effect) else {
            return Err(AuthenticatedGenesisStoreReplayPreAdmissionError {
                fetch: self,
                _effect: effect,
                _ownership: ownership,
            });
        };
        let Self {
            effect: fetch_effect,
            pending: fetch_pending,
            replay_evidence,
        } = self;
        match replay_evidence.project_store(&fetch_effect, &fetch_pending, &effect, &pending) {
            Ok(replay_evidence) => Ok(PreparedAuthenticatedGenesisStoreReplayPreAdmission {
                effect,
                pending,
                replay_evidence,
            }),
            Err(replay_evidence) => Err(AuthenticatedGenesisStoreReplayPreAdmissionError {
                fetch: Self {
                    effect: fetch_effect,
                    pending: fetch_pending,
                    replay_evidence,
                },
                _effect: effect,
                _ownership: ownership,
            }),
        }
    }
}
impl PreparedAuthenticatedGenesisStoreReplayPreAdmission {
    fn validates(&self) -> bool {
        self.replay_evidence
            .exactly_matches_store_pending(&self.effect, &self.pending)
    }
    /// Authenticate the original certified Fetch after it has projected Store.
    pub(in crate::sumeragi) fn exactly_authenticates_fetch_rediscovery(
        &self,
        effect: &AdapterEffect,
    ) -> bool {
        self.validates() && self.replay_evidence.exactly_matches_origin_fetch(effect)
    }
    /// Recheck an exact Store retry without replacing its local genesis origin.
    pub(in crate::sumeragi) fn exactly_matches_retry(
        &self,
        effect: &AdapterEffect,
        ownership: &RuntimeEffectOwnership,
    ) -> bool {
        ownership
            .exact_pending_adapter_effect_binding(effect)
            .is_ok_and(|pending| {
                self.validates() && self.effect == *effect && self.pending == pending
            })
    }
    /// Join the exact store-minted BodyFrame without exposing either input.
    #[allow(clippy::result_large_err)]
    pub(in crate::sumeragi) fn bind_durable_body(
        self,
        durable_receipt: DurableBodyReceipt,
    ) -> Result<
        PreparedAuthenticatedGenesisStoredReplayPreAdmission,
        AuthenticatedGenesisDurableReplayPreAdmissionError,
    > {
        let Self {
            effect,
            pending,
            replay_evidence,
        } = self;
        match replay_evidence.bind_durable_body(&effect, &pending, &durable_receipt) {
            Ok(replay_evidence) => Ok(PreparedAuthenticatedGenesisStoredReplayPreAdmission {
                store_effect: effect,
                store_pending: pending,
                durable_receipt,
                replay_evidence,
            }),
            Err(replay_evidence) => Err(AuthenticatedGenesisDurableReplayPreAdmissionError {
                store: Self {
                    effect,
                    pending,
                    replay_evidence,
                },
                _durable_receipt: durable_receipt,
            }),
        }
    }
}
impl PreparedAuthenticatedGenesisStoredReplayPreAdmission {
    fn validates(&self) -> bool {
        self.replay_evidence.exactly_matches_store_pending(
            &self.store_effect,
            &self.durable_receipt,
            &self.store_pending,
        )
    }
    /// Authenticate the original certified Fetch after durable Store completion.
    pub(in crate::sumeragi) fn exactly_authenticates_fetch_rediscovery(
        &self,
        effect: &AdapterEffect,
    ) -> bool {
        self.validates() && self.replay_evidence.exactly_matches_origin_fetch(effect)
    }
    /// Recheck the exact durable Store retry without accepting another frame.
    pub(in crate::sumeragi) fn exactly_matches_retry(
        &self,
        store_effect: &AdapterEffect,
        durable_receipt: &DurableBodyReceipt,
    ) -> bool {
        self.validates()
            && self.store_effect == *store_effect
            && self.durable_receipt == *durable_receipt
    }
    /// Prove that this durable replay retains the exact runtime Store owner.
    pub(in crate::sumeragi) fn exactly_retains_owned_store(
        &self,
        durable_receipt: &DurableBodyReceipt,
        ownership: &RuntimeEffectOwnership,
    ) -> bool {
        ownership
            .exact_pending_adapter_effect_binding(&self.store_effect)
            .is_ok_and(|pending| {
                self.exactly_matches_retry(&self.store_effect, durable_receipt)
                    && pending == self.store_pending
            })
    }
    /// Derive the same-body Validate runtime owner from this exact Store owner.
    pub(in crate::sumeragi) fn project_incumbent_validate_ownership(
        &self,
        durable_receipt: &DurableBodyReceipt,
        store_ownership: &RuntimeEffectOwnership,
        validate_effect: &AdapterEffect,
    ) -> Option<RuntimeEffectOwnership> {
        let (
            AdapterEffect::StoreBody {
                tag: store_tag,
                round: store_round,
                subject: store_subject,
            },
            AdapterEffect::ValidateBody {
                tag: validate_tag,
                round: validate_round,
                subject: validate_subject,
            },
        ) = (&self.store_effect, validate_effect)
        else {
            return None;
        };
        if !self.exactly_retains_owned_store(durable_receipt, store_ownership)
            || store_tag.height() != validate_tag.height()
            || (store_tag != validate_tag && !validate_tag.strictly_advances(*store_tag))
            || store_round != validate_round
            || store_subject != validate_subject
        {
            return None;
        }
        store_ownership
            .rebind_as_inherited_adapter_effect(validate_effect)
            .ok()
    }
    #[allow(clippy::result_large_err)]
    fn project_validate_inner(
        self,
        effect: AdapterEffect,
        ownership: RuntimeEffectOwnership,
    ) -> Result<
        PreparedLocalBodyValidateReplayPreAdmission,
        AuthenticatedGenesisValidateReplayPreAdmissionError,
    > {
        let Ok(pending) = ownership.exact_pending_adapter_effect_binding(&effect) else {
            return Err(AuthenticatedGenesisValidateReplayPreAdmissionError {
                stored: self,
                _effect: effect,
                _ownership: ownership,
            });
        };
        if !self.replay_evidence.exactly_projects_validate(
            &self.store_effect,
            &self.store_pending,
            &self.durable_receipt,
            &effect,
            &pending,
        ) {
            return Err(AuthenticatedGenesisValidateReplayPreAdmissionError {
                stored: self,
                _effect: effect,
                _ownership: ownership,
            });
        }
        let Self {
            store_effect,
            store_pending,
            durable_receipt,
            replay_evidence,
        } = self;
        let replay_evidence = replay_evidence
            .project_validate(
                &store_effect,
                &store_pending,
                &durable_receipt,
                &effect,
                &pending,
            )
            .expect("preflighted authenticated-genesis Validate projection is exact");
        let validate = PreparedLocalBodyValidateReplayPreAdmission {
            effect,
            pending,
            durable_receipt,
            replay_evidence,
        };
        debug_assert!(validate.validates());
        Ok(validate)
    }
    /// Consume the durable Store family only through its exact Validate successor.
    #[allow(clippy::result_large_err)]
    pub(in crate::sumeragi) fn project_validate(
        self,
        effect: AdapterEffect,
        ownership: RuntimeEffectOwnership,
    ) -> Result<
        PreparedLocalBodyValidateReplayPreAdmission,
        AuthenticatedGenesisValidateReplayPreAdmissionError,
    > {
        self.project_validate_inner(effect, ownership)
    }
    /// Consume the durable Store family through its exact Commit-authorized Validate.
    #[allow(clippy::result_large_err, clippy::too_many_arguments)]
    pub(in crate::sumeragi) fn project_validate_after_durable_decision(
        self,
        effect: AdapterEffect,
        ownership: RuntimeEffectOwnership,
        decision_round: wire::ConsensusRound,
        proposal_round: wire::ConsensusRound,
        subject: wire::BlockSubject,
        execution_commitment: wire::ExecutionCommitment,
    ) -> Result<
        PreparedLocalBodyValidateReplayPreAdmission,
        AuthenticatedGenesisValidateReplayPreAdmissionError,
    > {
        if !ownership.binds_durable_decision_authority(
            decision_round,
            proposal_round,
            subject,
            execution_commitment,
        ) {
            return Err(AuthenticatedGenesisValidateReplayPreAdmissionError {
                stored: self,
                _effect: effect,
                _ownership: ownership,
            });
        }
        self.project_validate_inner(effect, ownership)
    }
}
impl PreparedRemoteProposalFetchReplayPreAdmission {
    /// Consume one runtime-owned ordinary Fetch carrying exact signed-Proposal evidence.
    #[allow(clippy::result_large_err)]
    pub(in crate::sumeragi) fn seal_exact_fetch(
        effect: AdapterEffect,
        ownership: RuntimeEffectOwnership,
    ) -> Result<Self, RemoteProposalFetchReplayPreAdmissionError> {
        let Ok(pending) = ownership.exact_pending_adapter_effect_binding(&effect) else {
            return Err(RemoteProposalFetchReplayPreAdmissionError {
                _effect: effect,
                _ownership: ownership,
            });
        };
        let Some(replay_evidence) = ownership.exact_remote_proposal_fetch_replay(&effect) else {
            return Err(RemoteProposalFetchReplayPreAdmissionError {
                _effect: effect,
                _ownership: ownership,
            });
        };
        let sealed = Self {
            effect,
            pending,
            replay_evidence,
        };
        if !sealed.validates() {
            let Self {
                effect,
                pending: _,
                replay_evidence: _,
            } = sealed;
            return Err(RemoteProposalFetchReplayPreAdmissionError {
                _effect: effect,
                _ownership: ownership,
            });
        }
        Ok(sealed)
    }
    fn validates(&self) -> bool {
        self.replay_evidence
            .exactly_matches_fetch_pending(&self.effect, &self.pending)
    }
    /// Authenticate a byte-identical periodic Fetch without minting another origin.
    pub(in crate::sumeragi) fn exactly_authenticates_fetch_rediscovery(
        &self,
        effect: &AdapterEffect,
    ) -> bool {
        self.validates() && self.replay_evidence.exactly_matches_fetch(effect)
    }
    /// Recheck an exact retry without replacing the retained signed origin.
    pub(in crate::sumeragi) fn exactly_matches_retry(
        &self,
        effect: &AdapterEffect,
        ownership: &RuntimeEffectOwnership,
    ) -> bool {
        let Ok(pending) = ownership.exact_pending_adapter_effect_binding(effect) else {
            return false;
        };
        let Some(candidate) = ownership.exact_remote_proposal_fetch_replay(effect) else {
            return false;
        };
        self.validates()
            && self.effect == *effect
            && self.pending == pending
            && self
                .replay_evidence
                .exactly_matches_retry(&candidate, effect)
    }
    /// Consume the Fetch origin only through its exact ordinary Store successor.
    #[allow(clippy::result_large_err)]
    pub(in crate::sumeragi) fn project_store(
        self,
        effect: AdapterEffect,
        ownership: RuntimeEffectOwnership,
    ) -> Result<
        PreparedRemoteProposalStoreReplayPreAdmission,
        RemoteProposalStoreReplayPreAdmissionError,
    > {
        let Ok(pending) = ownership.exact_pending_adapter_effect_binding(&effect) else {
            return Err(RemoteProposalStoreReplayPreAdmissionError {
                _fetch: self,
                _effect: effect,
                _ownership: ownership,
            });
        };
        let Self {
            effect: fetch_effect,
            pending: fetch_pending,
            replay_evidence,
        } = self;
        match replay_evidence.project_exact_store(&effect, &pending) {
            Ok(replay_evidence) => {
                let store = PreparedRemoteProposalStoreReplayPreAdmission {
                    effect,
                    pending,
                    replay_evidence,
                };
                debug_assert!(store.validates());
                Ok(store)
            }
            Err(replay_evidence) => Err(RemoteProposalStoreReplayPreAdmissionError {
                _fetch: Self {
                    effect: fetch_effect,
                    pending: fetch_pending,
                    replay_evidence,
                },
                _effect: effect,
                _ownership: ownership,
            }),
        }
    }
}
impl PreparedRemoteProposalStoreReplayPreAdmission {
    fn validates(&self) -> bool {
        self.replay_evidence
            .exactly_matches_store_pending(&self.effect, &self.pending)
    }
    /// Authenticate the byte-identical Fetch origin after it has projected Store.
    pub(in crate::sumeragi) fn exactly_authenticates_fetch_rediscovery(
        &self,
        effect: &AdapterEffect,
    ) -> bool {
        self.validates() && self.replay_evidence.exactly_matches_origin_fetch(effect)
    }
    /// Recheck an exact Store retry without replacing its Proposal origin.
    pub(in crate::sumeragi) fn exactly_matches_retry(
        &self,
        effect: &AdapterEffect,
        ownership: &RuntimeEffectOwnership,
    ) -> bool {
        ownership
            .exact_pending_adapter_effect_binding(effect)
            .is_ok_and(|pending| {
                self.validates()
                    && self.effect == *effect
                    && self.pending == pending
                    && self
                        .replay_evidence
                        .exactly_matches_store_pending(effect, &pending)
            })
    }
    /// Join the exact store-minted BodyFrame without exposing either input.
    #[allow(clippy::result_large_err)]
    pub(in crate::sumeragi) fn bind_durable_body(
        self,
        durable_receipt: DurableBodyReceipt,
    ) -> Result<
        PreparedRemoteProposalStoredReplayPreAdmission,
        RemoteProposalDurableReplayPreAdmissionError,
    > {
        let Self {
            effect,
            pending,
            replay_evidence,
        } = self;
        match replay_evidence.bind_durable_body(&effect, &durable_receipt) {
            Ok(replay_evidence) => {
                let stored = PreparedRemoteProposalStoredReplayPreAdmission {
                    store_effect: effect,
                    store_pending: pending,
                    durable_receipt,
                    replay_evidence,
                };
                debug_assert!(stored.validates());
                Ok(stored)
            }
            Err(replay_evidence) => Err(RemoteProposalDurableReplayPreAdmissionError {
                _store: Self {
                    effect,
                    pending,
                    replay_evidence,
                },
                _durable_receipt: durable_receipt,
            }),
        }
    }
}
impl PreparedRemoteProposalStoredReplayPreAdmission {
    fn validates(&self) -> bool {
        self.store_pending
            .exactly_binds_adapter_effect(&self.store_effect)
            && self
                .replay_evidence
                .exactly_matches_store(&self.store_effect, &self.durable_receipt)
    }
    /// Authenticate the byte-identical Fetch origin after durable Store completion.
    pub(in crate::sumeragi) fn exactly_authenticates_fetch_rediscovery(
        &self,
        effect: &AdapterEffect,
    ) -> bool {
        self.validates() && self.replay_evidence.exactly_matches_origin_fetch(effect)
    }
    /// Recheck the exact durable Store retry without releasing its Proposal
    /// lineage or accepting a replacement body frame.
    pub(in crate::sumeragi) fn exactly_matches_retry(
        &self,
        store_effect: &AdapterEffect,
        durable_receipt: &DurableBodyReceipt,
    ) -> bool {
        self.validates()
            && self.store_effect == *store_effect
            && self.durable_receipt == *durable_receipt
            && self
                .store_pending
                .exactly_binds_adapter_effect(store_effect)
            && self
                .replay_evidence
                .exactly_matches_store(store_effect, durable_receipt)
    }
    /// Prove that the executor retained the complete runtime owner from the
    /// Store which minted this durable replay token.
    pub(in crate::sumeragi) fn exactly_retains_owned_store(
        &self,
        durable_receipt: &DurableBodyReceipt,
        ownership: &RuntimeEffectOwnership,
    ) -> bool {
        ownership
            .exact_pending_adapter_effect_binding(&self.store_effect)
            .is_ok_and(|pending| {
                self.exactly_matches_retry(&self.store_effect, durable_receipt)
                    && pending == self.store_pending
            })
    }
    /// Derive the only same-body Validate incumbent from the retained Store
    /// owner without releasing the replay token or minting a new causal root.
    pub(in crate::sumeragi) fn project_incumbent_validate_ownership(
        &self,
        durable_receipt: &DurableBodyReceipt,
        store_ownership: &RuntimeEffectOwnership,
        validate_effect: &AdapterEffect,
    ) -> Option<RuntimeEffectOwnership> {
        let (
            AdapterEffect::StoreBody {
                tag: store_tag,
                round: store_round,
                subject: store_subject,
            },
            AdapterEffect::ValidateBody {
                tag: validate_tag,
                round: validate_round,
                subject: validate_subject,
            },
        ) = (&self.store_effect, validate_effect)
        else {
            return None;
        };
        if !self.exactly_retains_owned_store(durable_receipt, store_ownership)
            || store_tag.height() != validate_tag.height()
            || (store_tag != validate_tag && !validate_tag.strictly_advances(*store_tag))
            || store_round != validate_round
            || store_subject != validate_subject
        {
            return None;
        }
        store_ownership
            .rebind_as_inherited_adapter_effect(validate_effect)
            .ok()
    }
    /// Consume the durable Store family only through its exact Validate successor.
    #[allow(clippy::result_large_err)]
    pub(in crate::sumeragi) fn project_validate(
        self,
        effect: AdapterEffect,
        ownership: RuntimeEffectOwnership,
    ) -> Result<
        PreparedRemoteProposalValidateReplayPreAdmission,
        RemoteProposalValidateReplayPreAdmissionError,
    > {
        let Ok(pending) = ownership.exact_pending_adapter_effect_binding(&effect) else {
            return Err(RemoteProposalValidateReplayPreAdmissionError {
                _stored: self,
                _effect: effect,
                _ownership: ownership,
            });
        };
        let Self {
            store_effect,
            store_pending,
            durable_receipt,
            replay_evidence,
        } = self;
        match replay_evidence.project_exact_validate(
            &store_effect,
            &durable_receipt,
            &effect,
            &pending,
        ) {
            Ok(replay_evidence) => {
                let validate = PreparedRemoteProposalValidateReplayPreAdmission {
                    effect,
                    pending,
                    durable_receipt,
                    replay_evidence,
                };
                debug_assert!(validate.validates());
                Ok(validate)
            }
            Err(replay_evidence) => Err(RemoteProposalValidateReplayPreAdmissionError {
                _stored: Self {
                    store_effect,
                    store_pending,
                    durable_receipt,
                    replay_evidence,
                },
                _effect: effect,
                _ownership: ownership,
            }),
        }
    }
    /// Consume the ordinary Proposal Store family through the exact
    /// Commit-authorized Validate retained for one durable Decision.
    #[allow(clippy::result_large_err, clippy::too_many_arguments)]
    pub(in crate::sumeragi) fn project_validate_after_durable_decision(
        self,
        effect: AdapterEffect,
        ownership: RuntimeEffectOwnership,
        decision_round: wire::ConsensusRound,
        proposal_round: wire::ConsensusRound,
        subject: wire::BlockSubject,
        execution_commitment: wire::ExecutionCommitment,
    ) -> Result<
        PreparedRemoteProposalValidateReplayPreAdmission,
        RemoteProposalValidateReplayPreAdmissionError,
    > {
        let Ok(pending) = ownership.exact_pending_adapter_effect_binding(&effect) else {
            return Err(RemoteProposalValidateReplayPreAdmissionError {
                _stored: self,
                _effect: effect,
                _ownership: ownership,
            });
        };
        if !ownership.binds_durable_decision_authority(
            decision_round,
            proposal_round,
            subject,
            execution_commitment,
        ) {
            return Err(RemoteProposalValidateReplayPreAdmissionError {
                _stored: self,
                _effect: effect,
                _ownership: ownership,
            });
        }
        let Self {
            store_effect,
            store_pending,
            durable_receipt,
            replay_evidence,
        } = self;
        match replay_evidence.project_exact_validate_after_durable_decision(
            &store_effect,
            &durable_receipt,
            &effect,
            &pending,
        ) {
            Ok(replay_evidence) => {
                let validate = PreparedRemoteProposalValidateReplayPreAdmission {
                    effect,
                    pending,
                    durable_receipt,
                    replay_evidence,
                };
                debug_assert!(validate.validates());
                Ok(validate)
            }
            Err(replay_evidence) => Err(RemoteProposalValidateReplayPreAdmissionError {
                _stored: Self {
                    store_effect,
                    store_pending,
                    durable_receipt,
                    replay_evidence,
                },
                _effect: effect,
                _ownership: ownership,
            }),
        }
    }
}
#[allow(dead_code)]
impl PreparedRemoteProposalValidateReplayPreAdmission {
    fn validates(&self) -> bool {
        self.replay_evidence.exactly_matches_validate_pending(
            &self.effect,
            &self.durable_receipt,
            &self.pending,
        )
    }
    fn exactly_authorizes_candidate(
        &self,
        active_context: LifecycleContext,
        candidate: &CandidateAdmission,
    ) -> bool {
        durable_validate_origin_exactly_authorizes_candidate(
            self.validates(),
            &self.effect,
            &self.pending,
            &self.durable_receipt,
            candidate.replay_authority.is_remote_proposal_origin(),
            active_context,
            candidate,
        )
    }
    /// Recheck an exact Validate retry without replacing the retained family.
    pub(in crate::sumeragi) fn exactly_matches_retry(
        &self,
        effect: &AdapterEffect,
        ownership: &RuntimeEffectOwnership,
    ) -> bool {
        ownership
            .exact_pending_adapter_effect_binding(effect)
            .is_ok_and(|pending| {
                self.validates()
                    && self.effect == *effect
                    && self.pending == pending
                    && self.replay_evidence.exactly_matches_validate_pending(
                        effect,
                        &self.durable_receipt,
                        &pending,
                    )
            })
    }
    /// Project this exact remote Proposal owner into the single lifecycle
    /// admission boundary without releasing its body-frame replay evidence.
    #[allow(clippy::result_large_err)]
    pub(in crate::sumeragi) fn prepare_lifecycle_admission(
        self,
        active_context: LifecycleContext,
        verified: &VerifiedHeightContext,
    ) -> Result<PreparedLifecycleAdmissionV1, Self> {
        PreparedDurableValidateAdmissionV1::RemoteProposal(self)
            .prepare(active_context, verified)
            .map_err(|validate| match validate {
                PreparedDurableValidateAdmissionV1::RemoteProposal(validate) => validate,
                PreparedDurableValidateAdmissionV1::LocalBody(_) => {
                    unreachable!("remote Proposal preparation retains its exact origin")
                }
            })
    }
    /// Close this authenticated Proposal lineage into the sole owner-facing
    /// durable Validate admission input.
    pub(in crate::sumeragi) fn into_pending_durable_validate_admission(
        self,
    ) -> PendingDurableValidateAdmissionV1 {
        PendingDurableValidateAdmissionV1 {
            validate: PreparedDurableValidateAdmissionV1::RemoteProposal(self),
        }
    }
    /// Consume the exact remote-Proposal Validate pre-admission into its
    /// closed durable carrier without accepting a manifest, receipt, pending
    /// binding, or installed digest from the caller.
    ///
    /// The address is structural admission output only. This conversion does
    /// not install it, and failure returns the complete move-only token.
    #[allow(clippy::result_large_err)]
    fn into_durable_validate_carrier(
        self,
        address: ConcreteWorkAddress,
    ) -> Result<(DurableValidateBody, LifecycleDigest), Self> {
        if !self.validates()
            || address.owner.causal_root()
                != super::CausalRoot::new(digest_from_hash(self.pending.causal_lifecycle_key()))
            || address.slot.capacity_class() != Some(LifecycleWorkClass::Validate.capacity_class())
        {
            return Err(self);
        }
        let digest = digest_from_hash(self.pending.exact_effect_identity());
        let expected_manifest_hash = self.durable_receipt.manifest_hash();
        let carrier = DurableValidateBody {
            address,
            effect: self.effect,
            pending: self.pending,
            durable_receipt: self.durable_receipt,
            expected_manifest_hash,
            replay_evidence: DurableValidateReplayEvidenceV1::remote_proposal(self.replay_evidence),
        };
        if !carrier.validates(digest) {
            let DurableValidateBody {
                address: _,
                effect,
                pending,
                durable_receipt,
                expected_manifest_hash: _,
                replay_evidence,
            } = carrier;
            let DurableValidateReplayEvidenceV1::RemoteProposal(replay_evidence) = replay_evidence
            else {
                unreachable!("remote adoption retains its exact replay variant")
            };
            return Err(Self {
                effect,
                pending,
                durable_receipt,
                replay_evidence,
            });
        }
        Ok((carrier, digest))
    }
}
impl PreparedLocalBodyValidateReplayPreAdmission {
    /// Consume the exact local Validate owner and its Store-projected replay seal.
    #[allow(clippy::result_large_err)]
    pub(in crate::sumeragi) fn seal_exact_validate(
        effect: AdapterEffect,
        ownership: RuntimeEffectOwnership,
        durable_receipt: DurableBodyReceipt,
        replay_evidence: LocalValidateReplayEvidenceV1,
    ) -> Result<Self, LocalBodyValidateReplayPreAdmissionError> {
        let Ok(pending) = ownership.exact_pending_adapter_effect_binding(&effect) else {
            return Err(LocalBodyValidateReplayPreAdmissionError {
                _effect: effect,
                _ownership: ownership,
                _durable_receipt: durable_receipt,
                _replay_evidence: replay_evidence,
            });
        };
        let prepared = Self {
            effect,
            pending,
            durable_receipt,
            replay_evidence,
        };
        if !prepared.validates() {
            let Self {
                effect,
                pending: _,
                durable_receipt,
                replay_evidence,
            } = prepared;
            return Err(LocalBodyValidateReplayPreAdmissionError {
                _effect: effect,
                _ownership: ownership,
                _durable_receipt: durable_receipt,
                _replay_evidence: replay_evidence,
            });
        }
        Ok(prepared)
    }
    fn validates(&self) -> bool {
        self.replay_evidence.exactly_matches_validate_pending(
            &self.effect,
            &self.durable_receipt,
            &self.pending,
        )
    }
    fn exactly_authorizes_candidate(
        &self,
        active_context: LifecycleContext,
        candidate: &CandidateAdmission,
    ) -> bool {
        durable_validate_origin_exactly_authorizes_candidate(
            self.validates(),
            &self.effect,
            &self.pending,
            &self.durable_receipt,
            self.replay_evidence
                .exactly_authorizes_local_admission_authority(
                    active_context,
                    &candidate.replay_authority,
                ),
            active_context,
            candidate,
        )
    }
    /// Recheck an exact local Validate retry without replacing the retained
    /// local-body replay family.
    pub(in crate::sumeragi) fn exactly_matches_retry(
        &self,
        effect: &AdapterEffect,
        ownership: &RuntimeEffectOwnership,
    ) -> bool {
        ownership
            .exact_pending_adapter_effect_binding(effect)
            .is_ok_and(|pending| {
                self.validates()
                    && self.effect == *effect
                    && self.pending == pending
                    && self.replay_evidence.exactly_matches_validate_pending(
                        effect,
                        &self.durable_receipt,
                        &pending,
                    )
            })
    }
    /// Project this exact local body owner into the single lifecycle admission boundary.
    #[allow(clippy::result_large_err)]
    pub(in crate::sumeragi) fn prepare_lifecycle_admission(
        self,
        active_context: LifecycleContext,
        verified: &VerifiedHeightContext,
    ) -> Result<PreparedLifecycleAdmissionV1, Self> {
        PreparedDurableValidateAdmissionV1::LocalBody(self)
            .prepare(active_context, verified)
            .map_err(|validate| match validate {
                PreparedDurableValidateAdmissionV1::LocalBody(validate) => validate,
                PreparedDurableValidateAdmissionV1::RemoteProposal(_) => {
                    unreachable!("local body preparation retains its exact origin")
                }
            })
    }
    /// Close this local-body lineage into the sole owner-facing durable
    /// Validate admission input.
    pub(in crate::sumeragi) fn into_pending_durable_validate_admission(
        self,
    ) -> PendingDurableValidateAdmissionV1 {
        PendingDurableValidateAdmissionV1 {
            validate: PreparedDurableValidateAdmissionV1::LocalBody(self),
        }
    }
    #[allow(clippy::result_large_err)]
    fn into_durable_validate_carrier(
        self,
        address: ConcreteWorkAddress,
    ) -> Result<(DurableValidateBody, LifecycleDigest), Self> {
        if !self.validates()
            || address.owner.causal_root()
                != super::CausalRoot::new(digest_from_hash(self.pending.causal_lifecycle_key()))
            || address.slot.capacity_class() != Some(LifecycleWorkClass::Validate.capacity_class())
        {
            return Err(self);
        }
        let digest = digest_from_hash(self.pending.exact_effect_identity());
        let expected_manifest_hash = self.durable_receipt.manifest_hash();
        let carrier = DurableValidateBody {
            address,
            effect: self.effect,
            pending: self.pending,
            durable_receipt: self.durable_receipt,
            expected_manifest_hash,
            replay_evidence: DurableValidateReplayEvidenceV1::local_body(self.replay_evidence),
        };
        if !carrier.validates(digest) {
            let DurableValidateBody {
                address: _,
                effect,
                pending,
                durable_receipt,
                expected_manifest_hash: _,
                replay_evidence,
            } = carrier;
            let DurableValidateReplayEvidenceV1::LocalBody(replay_evidence) = replay_evidence
            else {
                unreachable!("local adoption retains its exact replay variant")
            };
            return Err(Self {
                effect,
                pending,
                durable_receipt,
                replay_evidence,
            });
        }
        Ok((carrier, digest))
    }
}
impl PreparedDurableValidateAdmissionV1 {
    fn project_candidate(
        &self,
        verified: &VerifiedHeightContext,
    ) -> Result<CandidateAdmission, AdapterEffectAdmissionError> {
        match self {
            Self::RemoteProposal(validate) => {
                DurableValidateReplayEvidenceV1::remote_proposal(validate.replay_evidence.clone())
                    .project_sealed_validate_successor_candidate(
                        SealedBodySuccessorProjectionPermit::new(),
                        verified,
                        &validate.effect,
                        &validate.durable_receipt,
                        &validate.pending,
                    )
            }
            Self::LocalBody(validate) => {
                DurableValidateReplayEvidenceV1::local_body(validate.replay_evidence.clone())
                    .project_sealed_validate_successor_candidate(
                        SealedBodySuccessorProjectionPermit::new(),
                        verified,
                        &validate.effect,
                        &validate.durable_receipt,
                        &validate.pending,
                    )
            }
        }
    }
    #[allow(clippy::result_large_err)]
    fn prepare(
        self,
        active_context: LifecycleContext,
        verified: &VerifiedHeightContext,
    ) -> Result<PreparedLifecycleAdmissionV1, Self> {
        self.prepare_classified(active_context, verified)
            .map_err(|(_, validate)| validate)
    }
    #[allow(clippy::result_large_err)]
    fn prepare_classified(
        self,
        active_context: LifecycleContext,
        verified: &VerifiedHeightContext,
    ) -> Result<PreparedLifecycleAdmissionV1, (AdapterEffectAdmissionError, Self)> {
        let candidate = match self.project_candidate(verified) {
            Ok(candidate) => candidate,
            Err(failure) => return Err((failure, self)),
        };
        let owner = match self {
            Self::RemoteProposal(validate) => {
                PreparedLifecycleAdmissionOwnerV1::RemoteProposal(validate)
            }
            Self::LocalBody(validate) => PreparedLifecycleAdmissionOwnerV1::LocalBody(validate),
        };
        let prepared = PreparedLifecycleAdmissionV1 { owner, candidate };
        if prepared.validates(active_context) {
            Ok(prepared)
        } else {
            let validate = match prepared.owner {
                PreparedLifecycleAdmissionOwnerV1::RemoteProposal(validate) => {
                    Self::RemoteProposal(validate)
                }
                PreparedLifecycleAdmissionOwnerV1::LocalBody(validate) => Self::LocalBody(validate),
                PreparedLifecycleAdmissionOwnerV1::LiveWal(_)
                | PreparedLifecycleAdmissionOwnerV1::InvalidBodyReport(_)
                | PreparedLifecycleAdmissionOwnerV1::DirectSigned(_) => {
                    unreachable!("durable preparation retains a durable origin")
                }
            };
            let failure =
                if active_context != super::projection::lifecycle_context(verified.context()) {
                    AdapterEffectAdmissionError::ForeignContext
                } else {
                    AdapterEffectAdmissionError::InvalidCarrier
                };
            Err((failure, validate))
        }
    }
    fn exactly_authorizes_candidate(
        &self,
        active_context: LifecycleContext,
        candidate: &CandidateAdmission,
    ) -> bool {
        match self {
            Self::RemoteProposal(validate) => {
                validate.exactly_authorizes_candidate(active_context, candidate)
            }
            Self::LocalBody(validate) => {
                validate.exactly_authorizes_candidate(active_context, candidate)
            }
        }
    }
    #[allow(clippy::result_large_err)]
    fn into_durable_validate_carrier(
        self,
        address: ConcreteWorkAddress,
    ) -> Result<(DurableValidateBody, LifecycleDigest), Self> {
        match self {
            Self::RemoteProposal(validate) => validate
                .into_durable_validate_carrier(address)
                .map_err(Self::RemoteProposal),
            Self::LocalBody(validate) => validate
                .into_durable_validate_carrier(address)
                .map_err(Self::LocalBody),
        }
    }
    fn from_returned_carrier(carrier: DurableValidateBody) -> Result<Self, DurableValidateBody> {
        let DurableValidateBody {
            address,
            effect,
            pending,
            durable_receipt,
            expected_manifest_hash,
            replay_evidence,
        } = carrier;
        if expected_manifest_hash != durable_receipt.manifest_hash() {
            return Err(DurableValidateBody {
                address,
                effect,
                pending,
                durable_receipt,
                expected_manifest_hash,
                replay_evidence,
            });
        }
        match replay_evidence {
            DurableValidateReplayEvidenceV1::RemoteProposal(replay_evidence) => Ok(
                Self::RemoteProposal(PreparedRemoteProposalValidateReplayPreAdmission {
                    effect,
                    pending,
                    durable_receipt,
                    replay_evidence,
                }),
            ),
            DurableValidateReplayEvidenceV1::LocalBody(replay_evidence) => Ok(Self::LocalBody(
                PreparedLocalBodyValidateReplayPreAdmission {
                    effect,
                    pending,
                    durable_receipt,
                    replay_evidence,
                },
            )),
            DurableValidateReplayEvidenceV1::Certified(replay_evidence) => {
                Err(DurableValidateBody {
                    address,
                    effect,
                    pending,
                    durable_receipt,
                    expected_manifest_hash,
                    replay_evidence: DurableValidateReplayEvidenceV1::Certified(replay_evidence),
                })
            }
            DurableValidateReplayEvidenceV1::RecoveredStandalone(replay_evidence) => {
                Err(DurableValidateBody {
                    address,
                    effect,
                    pending,
                    durable_receipt,
                    expected_manifest_hash,
                    replay_evidence: DurableValidateReplayEvidenceV1::RecoveredStandalone(
                        replay_evidence,
                    ),
                })
            }
        }
    }
}
impl PendingDurableValidateAdmissionV1 {
    /// Recheck an idempotent local-proposal retry without accepting a remote origin.
    pub(in crate::sumeragi) fn exactly_matches_local_body_retry(
        &self,
        tag: EventTag,
        manifest: &wire::PayloadManifest,
        durable_receipt: &DurableBodyReceipt,
    ) -> bool {
        let PreparedDurableValidateAdmissionV1::LocalBody(validate) = &self.validate else {
            return false;
        };
        let effect = AdapterEffect::ValidateBody {
            tag,
            round: manifest.round,
            subject: manifest.subject,
        };
        validate.validates()
            && validate.effect == effect
            && validate.durable_receipt == *durable_receipt
            && validate.durable_receipt.manifest_hash() == HashOf::new(manifest)
    }

    /// Recheck a duplicate Validate against this exact pending origin without
    /// exposing either origin-specific owner.
    pub(in crate::sumeragi) fn exactly_matches_retry(
        &self,
        effect: &AdapterEffect,
        ownership: &RuntimeEffectOwnership,
    ) -> bool {
        match &self.validate {
            PreparedDurableValidateAdmissionV1::RemoteProposal(validate) => {
                validate.exactly_matches_retry(effect, ownership)
            }
            PreparedDurableValidateAdmissionV1::LocalBody(validate) => {
                validate.exactly_matches_retry(effect, ownership)
            }
        }
    }

    /// Prepare this exact origin only against the lifecycle owner's coheld
    /// logical context and verified height context.
    #[allow(clippy::result_large_err)]
    pub(super) fn prepare(
        self,
        active_context: LifecycleContext,
        verified: &VerifiedHeightContext,
    ) -> Result<PreparedLifecycleAdmissionV1, (AdapterEffectAdmissionError, Self)> {
        self.validate
            .prepare_classified(active_context, verified)
            .map_err(|(failure, validate)| (failure, Self { validate }))
    }

    /// Recover the exact pending origin returned by the adjacent atomic
    /// admission transaction. A different prepared-owner class is an internal
    /// transaction invariant violation, not a caller-visible extraction path.
    pub(super) fn reclaim_returned(prepared: PreparedLifecycleAdmissionV1) -> Self {
        let PreparedLifecycleAdmissionV1 {
            owner,
            candidate: _,
        } = prepared;
        let validate = match owner {
            PreparedLifecycleAdmissionOwnerV1::LocalBody(validate) => {
                PreparedDurableValidateAdmissionV1::LocalBody(validate)
            }
            PreparedLifecycleAdmissionOwnerV1::RemoteProposal(validate) => {
                PreparedDurableValidateAdmissionV1::RemoteProposal(validate)
            }
            PreparedLifecycleAdmissionOwnerV1::LiveWal(_)
            | PreparedLifecycleAdmissionOwnerV1::InvalidBodyReport(_)
            | PreparedLifecycleAdmissionOwnerV1::DirectSigned(_) => {
                unreachable!("durable Validate settlement returned another prepared origin")
            }
        };
        Self { validate }
    }

    #[cfg(test)]
    /// Check the retained origin class and exact Validate effect without
    /// exposing either production owner.
    pub(super) fn exactly_retains_for_test(
        &self,
        effect: &AdapterEffect,
        remote_proposal: bool,
    ) -> bool {
        match &self.validate {
            PreparedDurableValidateAdmissionV1::RemoteProposal(validate) => {
                remote_proposal && validate.effect == *effect && validate.validates()
            }
            PreparedDurableValidateAdmissionV1::LocalBody(validate) => {
                !remote_proposal && validate.effect == *effect && validate.validates()
            }
        }
    }

    /// Report whether this sealed test-only origin can emit `LocalProposalReady`.
    #[cfg(test)]
    pub(in crate::sumeragi) fn projects_local_proposal_handoff_for_test(&self) -> bool {
        match &self.validate {
            PreparedDurableValidateAdmissionV1::RemoteProposal(_) => false,
            PreparedDurableValidateAdmissionV1::LocalBody(validate) => validate
                .replay_evidence
                .projects_local_proposal_handoff_for_test(),
        }
    }
}
