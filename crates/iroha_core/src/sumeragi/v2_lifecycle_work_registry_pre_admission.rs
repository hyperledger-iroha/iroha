/// Closed, move-only replay-evidence preflight for one directly signed effect.
///
/// This inert token owns the exact runtime effect and pending binding together
/// with the only replay wrapper valid for its closed class. It has no install,
/// commit, parts, or effect access; a future admission transaction must consume
/// the whole value without weakening this boundary.
#[cfg_attr(not(test), allow(dead_code))]
#[must_use = "direct signed replay evidence has not entered lifecycle admission"]
pub(super) struct PreparedDirectSignedReplayPreAdmission {
    effect: AdapterEffect,
    pending: PendingRuntimeEffectBinding,
    replay_evidence: DirectSignedReplayEvidenceV1,
}
enum DirectSignedReplayEvidenceV1 {
    Broadcast(SignedBroadcastReplayEvidenceV1),
    ReportEquivocation(SignedEquivocationReplayEvidenceV1),
}
enum DirectSignedReplayPreAdmissionFailure {
    UnsupportedEffect,
    InvalidReplayEvidence,
}
/// Opaque ownership-preserving failure from direct signed replay preflight.
pub(super) struct DirectSignedReplayPreAdmissionError {
    _effect: AdapterEffect,
    _pending: PendingRuntimeEffectBinding,
    _failure: DirectSignedReplayPreAdmissionFailure,
}
#[cfg_attr(not(test), allow(dead_code))]
impl PreparedDirectSignedReplayPreAdmission {
    /// Seal exactly one Broadcast or ReportEquivocation effect and its binding.
    #[allow(clippy::result_large_err)]
    pub(super) fn seal_exact(
        effect: AdapterEffect,
        pending: PendingRuntimeEffectBinding,
    ) -> Result<Self, DirectSignedReplayPreAdmissionError> {
        let replay_evidence = match &effect {
            AdapterEffect::Broadcast(_) => {
                SignedBroadcastReplayEvidenceV1::from_exact_effect(&effect, &pending)
                    .map(DirectSignedReplayEvidenceV1::Broadcast)
                    .ok_or(DirectSignedReplayPreAdmissionFailure::InvalidReplayEvidence)
            }
            AdapterEffect::ReportEquivocation { .. } => {
                SignedEquivocationReplayEvidenceV1::from_exact_effect(&effect, &pending)
                    .map(DirectSignedReplayEvidenceV1::ReportEquivocation)
                    .ok_or(DirectSignedReplayPreAdmissionFailure::InvalidReplayEvidence)
            }
            _ => Err(DirectSignedReplayPreAdmissionFailure::UnsupportedEffect),
        };
        let replay_evidence = match replay_evidence {
            Ok(replay_evidence) => replay_evidence,
            Err(failure) => {
                return Err(DirectSignedReplayPreAdmissionError {
                    _effect: effect,
                    _pending: pending,
                    _failure: failure,
                });
            }
        };
        let sealed = Self {
            effect,
            pending,
            replay_evidence,
        };
        if !sealed.validates() {
            let Self {
                effect,
                pending,
                replay_evidence: _,
            } = sealed;
            return Err(DirectSignedReplayPreAdmissionError {
                _effect: effect,
                _pending: pending,
                _failure: DirectSignedReplayPreAdmissionFailure::InvalidReplayEvidence,
            });
        }
        Ok(sealed)
    }
    fn validates(&self) -> bool {
        match (&self.effect, &self.replay_evidence) {
            (AdapterEffect::Broadcast(_), DirectSignedReplayEvidenceV1::Broadcast(evidence)) => {
                evidence.exactly_matches_effect(&self.effect, &self.pending)
            }
            (
                AdapterEffect::ReportEquivocation { .. },
                DirectSignedReplayEvidenceV1::ReportEquivocation(evidence),
            ) => evidence.exactly_matches_effect(&self.effect, &self.pending),
            _ => false,
        }
    }
}
/// Closed classification of the replay source retained by one bound adapter effect.
///
/// Every variant carries a required canonical replay authority.  The enum is
/// deliberately private: only the origin-specific replay seals may mint one,
/// and admission never accepts an authority or an origin tag as caller parts.
#[derive(Debug)]
#[allow(dead_code)]
enum BoundAdapterReplayOriginV1 {
    /// An effect emitted only after its live WAL record was fsynced.
    LiveWal(LifecycleReplayAuthorityV1),
    /// Store or Validate work descended from one locally assembled body.
    LocalBody(LifecycleReplayAuthorityV1),
    /// Fetch, Store, or Validate work descended from an authenticated Proposal.
    RemoteProposal(LifecycleReplayAuthorityV1),
    /// A deterministic invalid-body report descended from its rejected Validate.
    InvalidBodyReport(LifecycleReplayAuthorityV1),
    /// A complete signed Broadcast or authenticated equivocation report.
    DirectSigned(LifecycleReplayAuthorityV1),
}
impl BoundAdapterReplayOriginV1 {
    const fn authority(&self) -> &LifecycleReplayAuthorityV1 {
        match self {
            Self::LiveWal(authority)
            | Self::LocalBody(authority)
            | Self::RemoteProposal(authority)
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
                Self::LocalBody(authority) => authority.is_local_body_origin(),
                Self::RemoteProposal(authority) => authority.is_remote_proposal_origin(),
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
        self.replay_origin = BoundAdapterReplayOriginV1::RemoteProposal(authority);
        !self.validates()
    }
    #[cfg(test)]
    fn has_foreign_origin_for_test(&self, effect: &AdapterEffect) -> bool {
        self.effect == *effect
            && self.pending.exactly_binds_adapter_effect(effect)
            && matches!(
                &self.replay_origin,
                BoundAdapterReplayOriginV1::RemoteProposal(authority)
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
#[derive(Debug)]
pub(super) enum PreparedLifecycleAdmissionOwnerV1 {
    Bound(BoundAdapterEffectV1),
    DurableValidate(PreparedDurableValidateAdmissionV1),
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
            owner: PreparedLifecycleAdmissionOwnerV1::Bound(bound),
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
            PreparedLifecycleAdmissionOwnerV1::Bound(bound) => {
                bound.exactly_authorizes_candidate(active_context, &self.candidate)
            }
            PreparedLifecycleAdmissionOwnerV1::DurableValidate(validate) => {
                validate.exactly_authorizes_candidate(active_context, &self.candidate)
            }
        }
    }
    /// Consume the prepared owner only inside the adjacent registry transaction.
    pub(super) fn into_owner(self) -> PreparedLifecycleAdmissionOwnerV1 {
        self.owner
    }
    /// Reconstitute the exact prepared owner after a reversible registry failure.
    pub(super) fn from_returned_bound(
        active_context: LifecycleContext,
        candidate: CandidateAdmission,
        bound: BoundAdapterEffectV1,
    ) -> Option<Self> {
        let prepared = Self {
            owner: PreparedLifecycleAdmissionOwnerV1::Bound(bound),
            candidate,
        };
        prepared.validates(active_context).then_some(prepared)
    }
    /// Reconstitute the exact durable Validate owner after reversible failure.
    pub(super) fn from_returned_durable_validate(
        active_context: LifecycleContext,
        candidate: CandidateAdmission,
        validate: PreparedDurableValidateAdmissionV1,
    ) -> Option<Self> {
        let prepared = Self {
            owner: PreparedLifecycleAdmissionOwnerV1::DurableValidate(validate),
            candidate,
        };
        prepared.validates(active_context).then_some(prepared)
    }
    #[cfg(test)]
    /// Compare a prepared owner with the exact effect retained by a fixture.
    pub(super) fn exactly_binds_for_test(&self, effect: &AdapterEffect) -> bool {
        matches!(
            &self.owner,
            PreparedLifecycleAdmissionOwnerV1::Bound(bound)
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
            PreparedLifecycleAdmissionOwnerV1::Bound(bound) => {
                bound.replace_with_foreign_origin_for_test()
            }
            PreparedLifecycleAdmissionOwnerV1::DurableValidate(_) => false,
        }
    }
    #[cfg(test)]
    /// Check that a rejected fixture still retains its exact foreign origin.
    pub(super) fn has_foreign_origin_for_test(&self, effect: &AdapterEffect) -> bool {
        matches!(
            &self.owner,
            PreparedLifecycleAdmissionOwnerV1::Bound(bound)
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
#[cfg_attr(not(test), allow(dead_code))]
#[must_use = "remote Proposal Fetch replay evidence has not entered lifecycle admission"]
pub(in crate::sumeragi) struct PreparedRemoteProposalFetchReplayPreAdmission {
    effect: AdapterEffect,
    pending: PendingRuntimeEffectBinding,
    replay_evidence: RemoteProposalFetchReplayEvidenceV1,
}
/// Closed ordinary Store successor of one authenticated Proposal Fetch.
#[cfg_attr(not(test), allow(dead_code))]
#[must_use = "remote Proposal Store replay evidence still requires its durable body receipt"]
pub(in crate::sumeragi) struct PreparedRemoteProposalStoreReplayPreAdmission {
    effect: AdapterEffect,
    pending: PendingRuntimeEffectBinding,
    replay_evidence: RemoteProposalStoreReplayEvidenceV1,
}
/// Closed durable Store replay evidence waiting for its exact Validate owner.
#[cfg_attr(not(test), allow(dead_code))]
#[must_use = "durable remote Proposal replay evidence still requires its Validate successor"]
pub(in crate::sumeragi) struct PreparedRemoteProposalStoredReplayPreAdmission {
    store_effect: AdapterEffect,
    store_pending: PendingRuntimeEffectBinding,
    durable_receipt: DurableBodyReceipt,
    replay_evidence: RemoteProposalStoredReplayEvidenceV1,
}
/// Closed canonical Validate replay evidence from one signed remote Proposal.
#[cfg_attr(not(test), allow(dead_code))]
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
/// Origin-specific durable Validate owner accepted by the single admission cut.
#[must_use = "durable Validate admission must be installed or returned intact"]
pub(super) enum PreparedDurableValidateAdmissionV1 {
    RemoteProposal(PreparedRemoteProposalValidateReplayPreAdmission),
    LocalBody(PreparedLocalBodyValidateReplayPreAdmission),
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
/// Ownership-preserving failure from the local durable-Validate sealing cut.
pub(in crate::sumeragi) struct LocalBodyValidateReplayPreAdmissionError {
    _effect: AdapterEffect,
    _ownership: RuntimeEffectOwnership,
    _durable_receipt: DurableBodyReceipt,
    _replay_evidence: LocalValidateReplayEvidenceV1,
}
#[allow(dead_code)]
impl PreparedRemoteProposalFetchReplayPreAdmission {
    /// Consume one runtime-owned ordinary Fetch carrying exact signed-Proposal evidence.
    #[allow(clippy::result_large_err)]
    pub(in crate::sumeragi) fn seal_exact_fetch(
        effect: AdapterEffect,
        ownership: RuntimeEffectOwnership,
    ) -> Result<Self, RemoteProposalFetchReplayPreAdmissionError> {
        let Some(pending) = ownership.pending_adapter_effect_binding(&effect) else {
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
    /// Recheck an exact retry without replacing the retained signed origin.
    fn exactly_matches_retry(
        &self,
        effect: &AdapterEffect,
        ownership: &RuntimeEffectOwnership,
    ) -> bool {
        let Some(pending) = ownership.pending_adapter_effect_binding(effect) else {
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
        let Some(pending) = ownership.pending_adapter_effect_binding(&effect) else {
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
#[allow(dead_code)]
impl PreparedRemoteProposalStoreReplayPreAdmission {
    fn validates(&self) -> bool {
        self.replay_evidence
            .exactly_matches_store_pending(&self.effect, &self.pending)
    }
    /// Recheck an exact Store retry without replacing its Proposal origin.
    fn exactly_matches_retry(
        &self,
        effect: &AdapterEffect,
        ownership: &RuntimeEffectOwnership,
    ) -> bool {
        ownership
            .pending_adapter_effect_binding(effect)
            .is_some_and(|pending| {
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
#[allow(dead_code)]
impl PreparedRemoteProposalStoredReplayPreAdmission {
    fn validates(&self) -> bool {
        self.store_pending
            .exactly_binds_adapter_effect(&self.store_effect)
            && self
                .replay_evidence
                .exactly_matches_store(&self.store_effect, &self.durable_receipt)
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
        let Some(pending) = ownership.pending_adapter_effect_binding(&effect) else {
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
    /// Recheck an exact Validate retry without replacing the retained family.
    fn exactly_matches_retry(
        &self,
        effect: &AdapterEffect,
        ownership: &RuntimeEffectOwnership,
    ) -> bool {
        ownership
            .pending_adapter_effect_binding(effect)
            .is_some_and(|pending| {
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
        let Some(pending) = ownership.pending_adapter_effect_binding(&effect) else {
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
    /// Project this exact local body owner into the single lifecycle admission boundary.
    #[allow(clippy::result_large_err)]
    pub(in crate::sumeragi) fn prepare_lifecycle_admission(
        self,
        active_context: LifecycleContext,
        verified: &VerifiedHeightContext,
    ) -> Result<PreparedLifecycleAdmissionV1, Self> {
        PreparedDurableValidateAdmissionV1::LocalBody(self).prepare(active_context, verified)
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
            let DurableValidateReplayEvidenceV1::LocalBody(replay_evidence) = replay_evidence else {
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
    fn validates(&self) -> bool {
        match self {
            Self::RemoteProposal(validate) => validate.validates(),
            Self::LocalBody(validate) => validate.validates(),
        }
    }
    fn project_candidate(
        &self,
        verified: &VerifiedHeightContext,
    ) -> Result<CandidateAdmission, AdapterEffectAdmissionError> {
        match self {
            Self::RemoteProposal(validate) => {
                DurableValidateReplayEvidenceV1::remote_proposal(
                    validate.replay_evidence.clone(),
                )
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
        let candidate = match self.project_candidate(verified) {
            Ok(candidate) => candidate,
            Err(_) => return Err(self),
        };
        let prepared = PreparedLifecycleAdmissionV1 {
            owner: PreparedLifecycleAdmissionOwnerV1::DurableValidate(self),
            candidate,
        };
        if prepared.validates(active_context) {
            Ok(prepared)
        } else {
            let PreparedLifecycleAdmissionOwnerV1::DurableValidate(validate) = prepared.owner else {
                unreachable!("durable preparation retains a durable owner")
            };
            Err(validate)
        }
    }
    fn exactly_authorizes_candidate(
        &self,
        active_context: LifecycleContext,
        candidate: &CandidateAdmission,
    ) -> bool {
        let (effect, pending, durable_receipt, expected_origin) = match self {
            Self::RemoteProposal(validate) => (
                &validate.effect,
                &validate.pending,
                &validate.durable_receipt,
                BoundAdapterReplayOriginClassV1::RemoteProposal,
            ),
            Self::LocalBody(validate) => (
                &validate.effect,
                &validate.pending,
                &validate.durable_receipt,
                BoundAdapterReplayOriginClassV1::LocalBody,
            ),
        };
        let AdapterEffect::ValidateBody { round, subject, .. } = effect else {
            return false;
        };
        let Some(payload) = durable_validate_body_payload(durable_receipt) else {
            return false;
        };
        let origin_is_exact = match expected_origin {
            BoundAdapterReplayOriginClassV1::RemoteProposal => {
                candidate.replay_authority.is_remote_proposal_origin()
            }
            BoundAdapterReplayOriginClassV1::LocalBody => {
                candidate.replay_authority.is_local_body_origin()
            }
        };
        let slot = PhysicalSlotId::for_capacity(LifecycleWorkClass::Validate.capacity_class(), 0);
        self.validates()
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
            && candidate.physical_geometry.normalized().is_ok_and(
                |(physical, universe, consumed)| {
                    physical
                        == BTreeMap::from([(
                            slot,
                            digest_from_hash(pending.exact_effect_identity()),
                        )])
                        && universe == std::collections::BTreeSet::from([slot])
                        && consumed == universe
                },
            )
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
            DurableValidateReplayEvidenceV1::RemoteProposal(replay_evidence) => {
                Ok(Self::RemoteProposal(
                    PreparedRemoteProposalValidateReplayPreAdmission {
                        effect,
                        pending,
                        durable_receipt,
                        replay_evidence,
                    },
                ))
            }
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
#[derive(Clone, Copy)]
enum BoundAdapterReplayOriginClassV1 {
    RemoteProposal,
    LocalBody,
}
