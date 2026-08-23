//! Opaque adapter/runtime bridge for an interrupted canonical Kura tip.

use std::{sync::Arc, time::Instant};

use super::{
    AdapterEffect, AdapterError, AuthenticatedRecoveredAdapterStartup, Kura,
    ProductionLifecycleAdapterStartupStateV1, ProductionLifecycleAdapterStartupV1,
    ProductionLifecycleOwnerStartupErrorV1, RecoveredAdapterStartup,
    RecoveredLifecycleLocalProposalAttemptV1, RecoveredLifecycleOwnerFactoryInputsV1,
    RecoveredLifecycleStorageAuthorityV1, RecoveredWalDecisionFetch,
    RecoveredWalDecisionFetchReplayEvidenceV1, RecoveredWalFrameIdentity,
    RecoveredWalStartupAuthorityV1, VerifiedHeightContext,
};
use crate::sumeragi::v2_lifecycle_coordinator::ProductionLifecycleOwnerV1;

/// Recovered adapter startup joined to one interrupted canonical Kura tip.
///
/// The expected tip cannot be separated from the still-unclassified WAL
/// authority. Its sole consuming authentication below accepts only the exact
/// Decision-owned Fetch and converts it into the opaque replay seal retained by
/// lifecycle launch.
#[must_use = "pending Kura startup must authenticate its Decision Fetch"]
pub(crate) struct PendingKuraRecoveredAdapterStartupV1 {
    startup: RecoveredAdapterStartup,
    expected: crate::sumeragi::v2_recovery::PendingKuraApply,
}

/// Exact interrupted-tip replay authority after the Decision WAL frontier is authenticated.
///
/// The ordinary recovered Decision-Fetch branch is removed from the embedded
/// startup and retained here instead. Callers can neither project the raw
/// effect nor reopen it as ordinary lifecycle Fetch work.
#[must_use = "pending Kura replay must enter the storage-only lifecycle owner"]
pub(crate) struct AuthenticatedRecoveredPendingKuraAdapterStartupV1 {
    startup: AuthenticatedRecoveredAdapterStartup,
    replay: RecoveredPendingKuraApplyReplayV1,
}

/// Move-only Decision-Fetch authority awaiting runtime ownership installation.
#[must_use = "pending Kura replay must enter serialized runtime startup"]
pub(in crate::sumeragi) struct RecoveredPendingKuraApplyReplayV1 {
    expected: crate::sumeragi::v2_recovery::PendingKuraApply,
    fetch: RecoveredWalDecisionFetch,
}

/// Runtime-observed interrupted-tip effect retained until preactivation verification.
///
/// This value has no effect or evidence accessor. The sole consuming install
/// method rechecks its original verified context and WAL replay evidence before
/// the executor may advance the local-only recovery pipeline.
#[must_use = "pending Kura replay must be installed before activation"]
pub(in crate::sumeragi) struct PreparedRecoveredPendingKuraApplyReplayV1 {
    expected: crate::sumeragi::v2_recovery::PendingKuraApply,
    verified: VerifiedHeightContext,
    wal_identity: RecoveredWalFrameIdentity,
    replay_evidence: RecoveredWalDecisionFetchReplayEvidenceV1,
    effect: AdapterEffect,
    deferred_validated_marker: Option<DeferredPendingKuraValidatedMarkerV1>,
}

/// Move-only exact validation marker withheld from ordinary reducer replay.
///
/// Construction is private to the body-store open transaction after the
/// authenticated pending-Kura Decision Fetch, durable body, canonical
/// manifest, CommitQC, and validated receipt all rejoin. The sole consuming
/// projection below keeps this authority sealed beside the staged direct
/// reducer transition until its exact Apply successor is committed.
#[must_use = "a deferred pending-Kura marker must enter its exact validation transition"]
#[derive(Debug, PartialEq, Eq)]
#[cfg_attr(test, derive(Clone))]
pub(crate) struct DeferredPendingKuraValidatedMarkerV1 {
    tag: crate::sumeragi::v2::reducer::EventTag,
    round: crate::sumeragi::v2::wire::ConsensusRound,
    subject: crate::sumeragi::v2::wire::BlockSubject,
    manifest_hash: iroha_crypto::HashOf<crate::sumeragi::v2::wire::PayloadManifest>,
    durable: crate::sumeragi::v2_body_store::DurableBodyReceipt,
    validated: crate::sumeragi::v2_body_store::ValidatedBodyReceipt,
    certificate: crate::sumeragi::v2::wire::QuorumCertificate,
}

/// Staged pending-Kura validation plus its predecessor-derived Apply owner.
///
/// The marker and adapter borrow are inseparable. Dropping this value is inert;
/// its only consuming method commits the already-preflighted reducer state and
/// returns the exact owned Apply effect for the next bounded recovery stage.
#[must_use = "a prepared pending-Kura validation must commit its exact Apply successor"]
pub(in crate::sumeragi) struct PreparedPendingKuraValidatedApplyV1<'a> {
    prepared: super::PreparedDirectValidationSucceededApply<'a>,
    child_ownership: crate::sumeragi::v2_runtime::RuntimeEffectOwnership,
    _marker: DeferredPendingKuraValidatedMarkerV1,
}

/// Opaque move-only Apply child emitted by the deferred validation commit.
///
/// Its effect and ownership can be separated only by the executor-private
/// permit after the outer recovery stage has advanced to Apply.
#[must_use = "the exact pending-Kura Apply successor must enter executor dispatch"]
pub(crate) struct PendingKuraValidatedApplySuccessorV1 {
    effect: AdapterEffect,
    ownership: crate::sumeragi::v2_runtime::RuntimeEffectOwnership,
}

/// Installed interrupted-tip identity retained through no-clock lane recovery.
///
/// The expected canonical tip remains opaque. Lifecycle activation uses it to
/// reauthenticate State and Kura after the local Apply completes, while the
/// optional pre-Apply height-one Nexus/AMX capability is consumed before lane
/// startup can cross the applied-height boundary.
#[must_use = "installed pending Kura identity must remain with its lifecycle height"]
pub(in crate::sumeragi) struct InstalledPendingKuraApplyV1 {
    expected: crate::sumeragi::v2_recovery::PendingKuraApply,
    genesis: Option<crate::sumeragi::v2_effects::VerifiedPendingGenesisNexusAmxContext>,
}

impl InstalledPendingKuraApplyV1 {
    /// Consume the replayed height-one projection into lane-work startup.
    pub(in crate::sumeragi) fn take_genesis(
        &mut self,
    ) -> Option<crate::sumeragi::v2_effects::VerifiedPendingGenesisNexusAmxContext> {
        self.genesis.take()
    }

    /// Borrow the opaque expected tip only inside lifecycle authentication.
    pub(in crate::sumeragi) const fn expected(
        &self,
    ) -> crate::sumeragi::v2_recovery::PendingKuraApply {
        self.expected
    }
}

// The production PendingKura branch consumes this sealed startup through the
// dedicated no-clock lane-recovery/finalization lifecycle.

#[cfg_attr(not(test), allow(dead_code))]
impl RecoveredAdapterStartup {
    /// Bind an interrupted canonical Kura tip before classifying the WAL frontier.
    ///
    /// Only the exact recovered height may cross this boundary. The complete
    /// startup is returned unchanged on mismatch so no ordinary authentication
    /// path can accidentally consume a foreign pending-tip expectation.
    pub(crate) fn bind_pending_kura_apply(
        self,
        expected: crate::sumeragi::v2_recovery::PendingKuraApply,
    ) -> Result<PendingKuraRecoveredAdapterStartupV1, (AdapterError, Self)> {
        if expected.context_id() != self.adapter.wire_context.id()
            || expected.height() != self.adapter.wire_context.height
        {
            return Err((AdapterError::RecoveredPendingKuraApplyMismatch, self));
        }
        Ok(PendingKuraRecoveredAdapterStartupV1 {
            startup: self,
            expected,
        })
    }
}

#[cfg_attr(not(test), allow(dead_code))]
impl PendingKuraRecoveredAdapterStartupV1 {
    /// Authenticate the sole Decision Fetch without admitting ordinary Fetch recovery.
    ///
    /// The shared final-WAL classifier remains the only parser and cryptographic
    /// authenticator. This wrapper accepts only its Decision-Fetch result, then
    /// replaces the ordinary authority with `None` while retaining the complete
    /// Fetch in a move-only interrupted-tip seal.
    pub(crate) fn authenticate_final_wal_startup_authority(
        self,
    ) -> Result<AuthenticatedRecoveredPendingKuraAdapterStartupV1, AdapterError> {
        let Self { startup, expected } = self;
        let authenticated = startup
            .authenticate_final_wal_startup_authority()
            .map_err(|(error, _startup)| error)?;
        let AuthenticatedRecoveredAdapterStartup {
            adapter,
            effects,
            authority,
            validation_authority,
            factory_owner,
        } = authenticated;
        let RecoveredWalStartupAuthorityV1::DecisionFetch(fetch) = authority else {
            return Err(AdapterError::RecoveredPendingKuraApplyMismatch);
        };
        if !effects.is_empty() {
            return Err(AdapterError::RecoveredPendingKuraApplyMismatch);
        }
        if !matches!(
            &fetch.effect,
            AdapterEffect::FetchBody { subject, .. }
                if subject.block_hash == expected.block_hash()
        ) {
            return Err(AdapterError::RecoveredPendingKuraApplyMismatch);
        }
        Ok(AuthenticatedRecoveredPendingKuraAdapterStartupV1 {
            startup: AuthenticatedRecoveredAdapterStartup {
                adapter,
                effects,
                authority: RecoveredWalStartupAuthorityV1::None,
                validation_authority,
                factory_owner,
            },
            replay: RecoveredPendingKuraApplyReplayV1 { expected, fetch },
        })
    }
}

#[cfg(test)]
impl AuthenticatedRecoveredPendingKuraAdapterStartupV1 {
    pub(super) fn is_storage_only_for_test(&self) -> bool {
        self.startup.effects.is_empty()
            && matches!(
                &self.startup.authority,
                RecoveredWalStartupAuthorityV1::None
            )
    }

    pub(super) const fn expected_for_test(&self) -> crate::sumeragi::v2_recovery::PendingKuraApply {
        self.replay.expected
    }

    pub(super) fn into_runtime_startup_for_test(self) -> ProductionLifecycleAdapterStartupV1 {
        let Self { startup, replay } = self;
        let AuthenticatedRecoveredAdapterStartup {
            adapter,
            effects,
            authority,
            validation_authority: _,
            factory_owner: _,
        } = startup;
        assert!(matches!(authority, RecoveredWalStartupAuthorityV1::None));
        ProductionLifecycleAdapterStartupV1::recovered(adapter, effects)
            .with_pending_kura_apply_replay(replay)
    }
}

impl ProductionLifecycleAdapterStartupV1 {
    /// Attach the exact interrupted-tip seal before any launch authority is minted.
    pub(in crate::sumeragi) fn with_pending_kura_apply_replay(
        mut self,
        replay: RecoveredPendingKuraApplyReplayV1,
    ) -> Self {
        match &mut self.state {
            ProductionLifecycleAdapterStartupStateV1::Recovered {
                effects,
                pending_kura_apply,
                local_proposal_attempt: None,
                leader_wire_launch_prepared: false,
                ..
            } if effects.is_empty() && pending_kura_apply.is_none() => {
                *pending_kura_apply = Some(replay);
                self
            }
            ProductionLifecycleAdapterStartupStateV1::Recovered { .. } => {
                panic!("pending Kura replay must attach to one pristine storage-only startup")
            }
            #[cfg(test)]
            ProductionLifecycleAdapterStartupStateV1::Fixture => {
                panic!("fixture startup cannot retain pending Kura replay")
            }
        }
    }

    /// Consume the sealed adapter startup directly into the serialized runtime.
    pub(in crate::sumeragi) fn into_serialized_runtime(
        self,
        started_at: Instant,
        round_timeout: std::time::Duration,
        queue_config: crate::sumeragi::v2_runtime::RuntimeQueueConfig,
        lifecycle_ordinals: crate::sumeragi::v2_runtime::RuntimeLifecycleOrdinalSource,
    ) -> Result<
        (
            crate::sumeragi::v2_runtime::SerializedV2Runtime,
            Option<PreparedRecoveredPendingKuraApplyReplayV1>,
            Option<RecoveredLifecycleLocalProposalAttemptV1>,
        ),
        crate::sumeragi::v2_runtime::RuntimeConfigError,
    > {
        match self.state {
            ProductionLifecycleAdapterStartupStateV1::Recovered {
                adapter,
                effects,
                pending_kura_apply,
                local_proposal_attempt,
                leader_wire_launch_prepared: true,
            } if effects.is_empty()
                && (pending_kura_apply.is_none() || local_proposal_attempt.is_none()) =>
            {
                let pending = pending_kura_apply
                    .map(|replay| {
                        let RecoveredPendingKuraApplyReplayV1 { expected, fetch } = replay;
                        let RecoveredWalDecisionFetch {
                            wal_identity,
                            replay_evidence,
                            effect,
                        } = fetch;
                        let verified = VerifiedHeightContext {
                            context: adapter.wire_context.clone(),
                            proofs_of_possession: adapter.proofs_of_possession.clone(),
                            parent_verification: adapter.parent_verification.clone(),
                        };
                        if expected.context_id() != verified.context().id()
                            || expected.height() != verified.context().height
                            || !replay_evidence.exactly_matches_recovered_decision_fetch(
                                &verified,
                                wal_identity,
                                &effect,
                            )
                        {
                            return Err(
                                crate::sumeragi::v2_runtime::RuntimeConfigError::InvalidLifecycleOwnership,
                            );
                        }
                        Ok((expected, verified, wal_identity, replay_evidence, effect))
                    })
                    .transpose()?;
                let (startup_effects, pending) = match pending {
                    None => (Vec::new(), None),
                    Some((expected, verified, wal_identity, replay_evidence, effect)) => (
                        vec![effect],
                        Some((expected, verified, wal_identity, replay_evidence)),
                    ),
                };
                let (runtime, mut returned_effects) =
                    crate::sumeragi::v2_runtime::SerializedV2Runtime::new_with_lifecycle_ordinals(
                        adapter,
                        startup_effects,
                        started_at,
                        round_timeout,
                        queue_config,
                        lifecycle_ordinals,
                    )?;
                let replay = match pending {
                    None if returned_effects.is_empty() => None,
                    Some((expected, verified, wal_identity, replay_evidence))
                        if returned_effects.len() == 1
                            && replay_evidence.exactly_matches_recovered_decision_fetch(
                                &verified,
                                wal_identity,
                                &returned_effects[0],
                            ) =>
                    {
                        let effect = returned_effects
                            .pop()
                            .expect("one exact pending Kura effect was compared above");
                        Some(PreparedRecoveredPendingKuraApplyReplayV1 {
                            expected,
                            verified,
                            wal_identity,
                            replay_evidence,
                            effect,
                            deferred_validated_marker: None,
                        })
                    }
                    None | Some(_) => {
                        return Err(
                            crate::sumeragi::v2_runtime::RuntimeConfigError::InvalidLifecycleOwnership,
                        );
                    }
                };
                Ok((runtime, replay, local_proposal_attempt))
            }
            ProductionLifecycleAdapterStartupStateV1::Recovered { .. } => {
                Err(crate::sumeragi::v2_runtime::RuntimeConfigError::InvalidLifecycleOwnership)
            }
            #[cfg(test)]
            ProductionLifecycleAdapterStartupStateV1::Fixture => {
                Err(crate::sumeragi::v2_runtime::RuntimeConfigError::InvalidLifecycleOwnership)
            }
        }
    }
}

#[cfg_attr(not(test), allow(dead_code))]
impl PreparedRecoveredPendingKuraApplyReplayV1 {
    /// Defer the sole exact validated marker until the staged recovery Validate runs.
    ///
    /// The ordinary cold-open path replays validated markers into the reducer.
    /// Pending-Kura recovery instead owns an explicit
    /// Fetch -> Store -> Validate -> Apply sequence. Replaying this marker early
    /// would make the staged Validate a duplicate and strand the recovery at
    /// Apply, so this sealed WAL authority classifies and consumes exactly one
    /// marker deferral while the body store remains exclusively borrowed.
    pub(in crate::sumeragi) fn classify_and_defer_validated_marker(
        &mut self,
        key: (
            crate::sumeragi::v2::wire::ConsensusRound,
            crate::sumeragi::v2::wire::BlockSubject,
        ),
        manifest: &crate::sumeragi::v2::wire::PayloadManifest,
        durable: &crate::sumeragi::v2_body_store::DurableBodyReceipt,
        validated: &crate::sumeragi::v2_body_store::ValidatedBodyReceipt,
    ) -> Result<bool, &'static str> {
        if !self
            .replay_evidence
            .exactly_matches_recovered_decision_fetch(
                &self.verified,
                self.wal_identity,
                &self.effect,
            )
        {
            return Err("pending Kura marker deferral lost its sealed WAL replay authority");
        }
        let AdapterEffect::FetchBody {
            round,
            subject,
            manifest: advertised_manifest,
            certificate: Some(certificate),
            ..
        } = &self.effect
        else {
            return Err("pending Kura marker deferral lost its certified FetchBody");
        };
        let expected_key = (*round, *subject);
        if key != expected_key {
            return Ok(false);
        }
        if self.deferred_validated_marker.is_some() {
            return Err("pending Kura marker deferral matched more than one validated marker");
        }
        let context = self.verified.context();
        if self.expected.context_id() != context.id()
            || self.expected.height() != context.height
            || self.expected.block_hash() != subject.block_hash
            || certificate.phase != crate::sumeragi::v2::wire::GlobalPhase::Commit
            || certificate.proposal_round != *round
            || certificate.subject != *subject
            || certificate.validate(context).is_err()
            || manifest.validate(context).is_err()
            || manifest.round != *round
            || manifest.subject != *subject
            || advertised_manifest
                .as_ref()
                .is_some_and(|advertised| advertised != manifest)
            || durable.context_id() != context.id()
            || durable.round() != *round
            || durable.subject() != *subject
            || durable.manifest_hash() != iroha_crypto::HashOf::new(manifest)
            || validated.durable() != durable
            || validated.execution_commitment() != certificate.execution_commitment
        {
            return Err("pending Kura validated marker changed its exact recovered authority");
        }
        self.deferred_validated_marker = Some(DeferredPendingKuraValidatedMarkerV1 {
            tag: match &self.effect {
                AdapterEffect::FetchBody { tag, .. } => *tag,
                _ => unreachable!("the exact pending-Kura effect was matched above"),
            },
            round: *round,
            subject: *subject,
            manifest_hash: iroha_crypto::HashOf::new(manifest),
            durable: durable.clone(),
            validated: validated.clone(),
            certificate: certificate.clone(),
        });
        Ok(true)
    }

    /// Require the body-store open transaction to have deferred its sole marker.
    pub(in crate::sumeragi) const fn validated_marker_was_deferred(&self) -> bool {
        self.deferred_validated_marker.is_some()
    }

    /// Install the exact runtime-observed Fetch into closed-ingress pending-tip recovery.
    ///
    /// Verification precedes effect consumption and repeats the canonical WAL
    /// replay join against the exact executor context. The effect vector exists
    /// only inside this consuming call and cannot be returned on either path.
    pub(in crate::sumeragi) fn install(
        self,
        executor: &mut crate::sumeragi::v2_effects::V2EffectExecutor<
            crate::sumeragi::v2_runtime::SerializedV2Runtime,
        >,
        services: &mut crate::sumeragi::v2_worker::ProductionV2Services,
    ) -> Result<InstalledPendingKuraApplyV1, crate::sumeragi::v2_effects::EffectExecutorError> {
        let Self {
            expected,
            verified,
            wal_identity,
            replay_evidence,
            effect,
            deferred_validated_marker,
        } = self;
        let Some(deferred_validated_marker) = deferred_validated_marker else {
            return Err(
                crate::sumeragi::v2_effects::EffectExecutorError::PendingApplyRecoveryMismatch(
                    "pending Kura replay omitted its exact deferred validation marker".to_owned(),
                ),
            );
        };
        if executor.context() != verified.context()
            || !replay_evidence.exactly_matches_recovered_decision_fetch(
                &verified,
                wal_identity,
                &effect,
            )
        {
            return Err(
                crate::sumeragi::v2_effects::EffectExecutorError::PendingApplyRecoveryMismatch(
                    "pending Kura replay changed its verified WAL Decision Fetch".to_owned(),
                ),
            );
        }
        let effects = vec![effect];
        let genesis = executor.verify_pending_kura_apply_replay(
            expected,
            &effects,
            deferred_validated_marker,
        )?;
        executor.consume_pending_tip_recovery_effects(effects, services)?;
        Ok(InstalledPendingKuraApplyV1 { expected, genesis })
    }

    #[cfg(test)]
    pub(super) const fn expected_for_test(&self) -> crate::sumeragi::v2_recovery::PendingKuraApply {
        self.expected
    }

    #[cfg(test)]
    pub(super) fn is_exact_for_test(&self) -> bool {
        self.replay_evidence
            .exactly_matches_recovered_decision_fetch(
                &self.verified,
                self.wal_identity,
                &self.effect,
            )
    }
}

impl DeferredPendingKuraValidatedMarkerV1 {
    #[cfg(test)]
    pub(in crate::sumeragi) fn for_test(
        tag: crate::sumeragi::v2::reducer::EventTag,
        manifest: &crate::sumeragi::v2::wire::PayloadManifest,
        durable: &crate::sumeragi::v2_body_store::DurableBodyReceipt,
        validated: &crate::sumeragi::v2_body_store::ValidatedBodyReceipt,
        certificate: &crate::sumeragi::v2::wire::QuorumCertificate,
    ) -> Self {
        Self {
            tag,
            round: manifest.round,
            subject: manifest.subject,
            manifest_hash: iroha_crypto::HashOf::new(manifest),
            durable: durable.clone(),
            validated: validated.clone(),
            certificate: certificate.clone(),
        }
    }

    /// Rejoin this marker with every independently reconstructed pending-tip field.
    pub(in crate::sumeragi) fn exactly_matches_recovery(
        &self,
        context: &crate::sumeragi::v2::wire::HeightContext,
        expected: crate::sumeragi::v2_recovery::PendingKuraApply,
        replay_tag: crate::sumeragi::v2::reducer::EventTag,
        manifest: &crate::sumeragi::v2::wire::PayloadManifest,
        durable: &crate::sumeragi::v2_body_store::DurableBodyReceipt,
        validated: &crate::sumeragi::v2_body_store::ValidatedBodyReceipt,
        certificate: &crate::sumeragi::v2::wire::QuorumCertificate,
    ) -> bool {
        self.tag == replay_tag
            && self.round == manifest.round
            && self.subject == manifest.subject
            && self.subject.block_hash == expected.block_hash()
            && expected.context_id() == context.id()
            && expected.height() == context.height
            && self.manifest_hash == iroha_crypto::HashOf::new(manifest)
            && self.durable == *durable
            && self.validated == *validated
            && self.validated.durable() == &self.durable
            && self.certificate == *certificate
            && self.certificate.phase == crate::sumeragi::v2::wire::GlobalPhase::Commit
            && self.certificate.proposal_round == self.round
            && self.certificate.subject == self.subject
            && self.certificate.execution_commitment == self.validated.execution_commitment()
            && self.certificate.validate(context).is_ok()
    }

    /// Seal one exact direct successful-validation preview and its Apply owner.
    ///
    /// Failure returns this marker unchanged. The adapter preview is drop-inert,
    /// so no reducer, registry, or fence state changes before the returned
    /// composite reaches its infallible commit tail.
    #[allow(clippy::result_large_err)]
    pub(in crate::sumeragi) fn prepare_apply<'a>(
        self,
        adapter: &'a mut super::SumeragiV2Adapter,
        predecessor: &AdapterEffect,
        ownership: &crate::sumeragi::v2_runtime::RuntimeEffectOwnership,
    ) -> Result<PreparedPendingKuraValidatedApplyV1<'a>, (Self, AdapterError)> {
        let AdapterEffect::ValidateBody {
            tag,
            round,
            subject,
        } = predecessor
        else {
            return Err((self, AdapterError::RecoveredPendingKuraApplyMismatch));
        };
        if *tag != self.tag || *round != self.round || *subject != self.subject {
            return Err((self, AdapterError::RecoveredPendingKuraApplyMismatch));
        }
        if !ownership.binds_durable_decision_authority(
            self.certificate.round,
            self.certificate.proposal_round,
            self.subject,
            self.certificate.execution_commitment,
        ) {
            return Err((self, AdapterError::RecoveredPendingKuraApplyMismatch));
        }
        let validate_pending = match ownership.exact_pending_adapter_effect_binding(predecessor) {
            Ok(pending) => pending,
            Err(_) => return Err((self, AdapterError::RecoveredPendingKuraApplyMismatch)),
        };
        let prepared = match adapter.prepare_direct_validation_succeeded(
            self.tag,
            self.round,
            self.subject,
            &self.validated,
        ) {
            Ok(super::DirectValidationSucceededPreparation::Apply(prepared)) => prepared,
            Ok(other) => {
                drop(other);
                return Err((self, AdapterError::RecoveredPendingKuraApplyMismatch));
            }
            Err(error) => return Err((self, error)),
        };
        let apply_effect = prepared.apply_effect().clone();
        if !matches!(
            &apply_effect,
            AdapterEffect::Apply {
                tag,
                subject,
                certificate,
            } if *tag == self.tag
                && *subject == self.subject
                && certificate == &self.certificate
        ) {
            drop(prepared);
            return Err((self, AdapterError::RecoveredPendingKuraApplyMismatch));
        }
        let Some(child_pending) =
            validate_pending.project_validate_apply_successor(predecessor, &apply_effect)
        else {
            drop(prepared);
            return Err((self, AdapterError::RecoveredPendingKuraApplyMismatch));
        };
        let child_ownership = match ownership.rebind_as_inherited_adapter_effect(&apply_effect) {
            Ok(ownership) => ownership,
            Err(_) => {
                drop(prepared);
                return Err((self, AdapterError::RecoveredPendingKuraApplyMismatch));
            }
        };
        if child_ownership
            .exact_pending_adapter_effect_binding(&apply_effect)
            .ok()
            .as_ref()
            != Some(&child_pending)
        {
            drop(prepared);
            return Err((self, AdapterError::RecoveredPendingKuraApplyMismatch));
        }
        Ok(PreparedPendingKuraValidatedApplyV1 {
            prepared,
            child_ownership,
            _marker: self,
        })
    }
}

impl PreparedPendingKuraValidatedApplyV1<'_> {
    /// Commit the staged reducer validation and release its exact owned Apply.
    pub(in crate::sumeragi) fn commit(self) -> PendingKuraValidatedApplySuccessorV1 {
        let Self {
            prepared,
            child_ownership,
            _marker: _,
        } = self;
        let super::PreparedDirectValidationSucceededApply {
            _adapter: adapter,
            next_reducer,
            next_registry,
            event,
            core_effect,
            apply_effect,
            next_fence_generation,
        } = prepared;
        debug_assert!(child_ownership.exactly_binds_adapter_effect(&apply_effect));
        adapter.reducer = next_reducer;
        adapter.registry = next_registry;
        adapter.reducer_fence_generation = next_fence_generation;
        adapter.record_reducer_outcome(
            &event,
            crate::sumeragi::v2::reducer::StepDisposition::Applied,
            core::slice::from_ref(&core_effect),
        );
        adapter.log_body_progress(
            &event,
            crate::sumeragi::v2::reducer::StepDisposition::Applied,
            1,
        );
        PendingKuraValidatedApplySuccessorV1 {
            effect: apply_effect,
            ownership: child_ownership,
        }
    }
}

impl PendingKuraValidatedApplySuccessorV1 {
    /// Release the exact child only to the pending-tip executor continuation.
    pub(in crate::sumeragi) fn consume_for_executor(
        self,
        _permit: crate::sumeragi::v2_effects::PendingKuraApplySuccessorExecutorPermitV1,
    ) -> (
        AdapterEffect,
        crate::sumeragi::v2_runtime::RuntimeEffectOwnership,
    ) {
        (self.effect, self.ownership)
    }
}

#[allow(
    dead_code,
    reason = "the pending-Kura recovery-plan cutover retains this sealed production bridge"
)]
impl AuthenticatedRecoveredPendingKuraAdapterStartupV1 {
    /// Bind the exact runner dependencies without exposing the embedded startup.
    #[allow(clippy::result_large_err, clippy::too_many_arguments)]
    pub(in crate::sumeragi) fn bind_production_lifecycle_owner_factory_inputs_v1(
        &self,
        permit: crate::sumeragi::v2_runner::RecoveredLifecycleOwnerFactoryDependencyPermitV1,
        storage: RecoveredLifecycleStorageAuthorityV1,
        state: Arc<crate::state::State>,
        queue: Arc<crate::queue::Queue>,
        kura: Arc<Kura>,
        provider_ingest_finalized_archive: Option<
            Arc<crate::query::provider_ingest_finalized::ProviderIngestFinalizedArchiveV1>,
        >,
        reputation_finalized_archive: Option<
            Arc<crate::query::reputation_finalized::ReputationFinalizedArchive>,
        >,
        events_sender: crate::EventsSender,
    ) -> Result<RecoveredLifecycleOwnerFactoryInputsV1, ProductionLifecycleOwnerStartupErrorV1>
    {
        self.startup
            .bind_production_lifecycle_owner_factory_inputs_v1(
                permit,
                storage,
                state,
                queue,
                kura,
                provider_ingest_finalized_archive,
                reputation_finalized_archive,
                events_sender,
            )
    }

    /// Open only the storage lifecycle branch and attach the pending-tip replay seal.
    ///
    /// The embedded ordinary authority was replaced with `None` by the exact
    /// classifier above. The resulting owner therefore cannot install a live
    /// recovered Fetch row before the closed-ingress interrupted-tip path runs.
    #[allow(clippy::result_large_err, clippy::too_many_arguments)]
    pub(in crate::sumeragi) fn open_production_lifecycle_owner_v1(
        self,
        config: &iroha_config::parameters::actual::SumeragiV2Config,
        reply_route_source_capacity: usize,
        factory_inputs: RecoveredLifecycleOwnerFactoryInputsV1,
        body_store: crate::sumeragi::v2_body_store::QuarantinedV2BodyStore,
    ) -> Result<ProductionLifecycleOwnerV1, ProductionLifecycleOwnerStartupErrorV1> {
        let Self { startup, replay } = self;
        startup
            .open_production_lifecycle_owner_v1(
                config,
                reply_route_source_capacity,
                factory_inputs,
                body_store,
            )
            .map(|owner| owner.with_pending_kura_apply_replay(replay))
    }
}
