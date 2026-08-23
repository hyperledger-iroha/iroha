//! Opaque adapter/runtime bridge for an interrupted canonical Kura tip.

use std::{sync::Arc, time::Instant};

use super::{
    AdapterEffect, AdapterError, AuthenticatedRecoveredAdapterStartup, Kura,
    ProductionLifecycleAdapterStartupStateV1, ProductionLifecycleAdapterStartupV1,
    ProductionLifecycleOwnerStartupErrorV1, RecoveredAdapterStartup,
    RecoveredLifecycleLocalProposalAttemptV1, RecoveredLifecycleOwnerFactoryInputsV1,
    RecoveredLifecycleStorageAuthorityV1, RecoveredWalDecisionFetchReplayEvidenceV1,
    RecoveredWalFrameIdentity, RecoveredWalStartupAuthorityV1, VerifiedHeightContext,
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
/// The ordinary recovered Decision-Fetch branch remains in the embedded
/// startup so owner-open can reconstruct its exact recovered Apply carrier.
/// This wrapper retains only cloneable WAL provenance for the later no-clock
/// pending-tip join; it cannot project or dispatch ordinary lifecycle work.
#[must_use = "pending Kura replay must enter the recovered Decision Apply owner"]
pub(crate) struct AuthenticatedRecoveredPendingKuraAdapterStartupV1 {
    startup: AuthenticatedRecoveredAdapterStartup,
    replay: RecoveredPendingKuraApplyReplayV1,
}

/// Inert Decision-Fetch provenance awaiting runtime ownership installation.
#[must_use = "pending Kura replay must enter serialized runtime startup"]
pub(in crate::sumeragi) struct RecoveredPendingKuraApplyReplayV1 {
    expected: crate::sumeragi::v2_recovery::PendingKuraApply,
    wal_identity: RecoveredWalFrameIdentity,
    replay_evidence: RecoveredWalDecisionFetchReplayEvidenceV1,
    effect: AdapterEffect,
    apply_carrier:
        Option<crate::sumeragi::v2_lifecycle_coordinator::RecoveredPendingKuraApplyCarrierPermitV1>,
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
    apply_carrier:
        crate::sumeragi::v2_lifecycle_coordinator::RecoveredPendingKuraApplyCarrierPermitV1,
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
    /// Authenticate the sole Decision Fetch and retain its inert replay provenance.
    ///
    /// The shared final-WAL classifier remains the only parser and cryptographic
    /// authenticator. This wrapper accepts only its Decision-Fetch result. The
    /// move-only authority remains embedded for the ordinary recovered-Decision
    /// Apply fast-forward, while cloneable provenance is retained for the later
    /// pending-tip join.
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
        let replay = RecoveredPendingKuraApplyReplayV1 {
            expected,
            wal_identity: fetch.wal_identity,
            replay_evidence: fetch.replay_evidence.clone(),
            effect: fetch.effect.clone(),
            apply_carrier: None,
        };
        Ok(AuthenticatedRecoveredPendingKuraAdapterStartupV1 {
            startup: AuthenticatedRecoveredAdapterStartup {
                adapter,
                effects,
                authority: RecoveredWalStartupAuthorityV1::DecisionFetch(fetch),
                validation_authority,
                factory_owner,
            },
            replay,
        })
    }
}

impl RecoveredPendingKuraApplyReplayV1 {
    /// Join inert WAL provenance to the exact recovered Apply owner census.
    pub(in crate::sumeragi) fn bind_recovered_apply_carrier(
        mut self,
        permit: crate::sumeragi::v2_lifecycle_coordinator::RecoveredPendingKuraApplyCarrierPermitV1,
    ) -> Self {
        assert!(
            self.apply_carrier.is_none(),
            "pending Kura replay binds one recovered Apply carrier"
        );
        self.apply_carrier = Some(permit);
        self
    }
}

#[cfg(test)]
impl AuthenticatedRecoveredPendingKuraAdapterStartupV1 {
    pub(super) fn retains_decision_fetch_for_test(&self) -> bool {
        self.startup.effects.is_empty()
            && matches!(
                &self.startup.authority,
                RecoveredWalStartupAuthorityV1::DecisionFetch(_)
            )
    }

    pub(super) const fn expected_for_test(&self) -> crate::sumeragi::v2_recovery::PendingKuraApply {
        self.replay.expected
    }

    #[allow(clippy::result_large_err, clippy::too_many_arguments)]
    pub(super) fn open_production_lifecycle_owner_v1_with_store_for_test(
        self,
        config: &iroha_config::parameters::actual::SumeragiV2Config,
        reply_route_source_capacity: usize,
        ledger_root: &std::path::Path,
        serve_payload_root: &std::path::Path,
        body_store: crate::sumeragi::v2_body_store::RevalidatedV2BodyStore,
        local_signer: &iroha_crypto::KeyPair,
    ) -> Result<ProductionLifecycleOwnerV1, ProductionLifecycleOwnerStartupErrorV1> {
        let Self { startup, replay } = self;
        startup
            .open_production_lifecycle_owner_v1_with_store_for_test(
                config,
                reply_route_source_capacity,
                ledger_root,
                serve_payload_root,
                body_store,
                local_signer,
            )
            .and_then(|owner| {
                owner
                    .with_pending_kura_apply_replay(replay)
                    .map_err(ProductionLifecycleOwnerStartupErrorV1::pending_kura_recovered_apply)
            })
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
                panic!("pending Kura replay must attach to one pristine recovered Apply startup")
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
                        let RecoveredPendingKuraApplyReplayV1 {
                            expected,
                            wal_identity,
                            replay_evidence,
                            effect,
                            apply_carrier,
                        } = replay;
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
                        let apply_carrier = apply_carrier.ok_or(
                            crate::sumeragi::v2_runtime::RuntimeConfigError::InvalidLifecycleOwnership,
                        )?;
                        Ok(PreparedRecoveredPendingKuraApplyReplayV1 {
                            expected,
                            verified,
                            wal_identity,
                            replay_evidence,
                            effect,
                            apply_carrier,
                        })
                    })
                    .transpose()?;
                let (runtime, returned_effects) =
                    crate::sumeragi::v2_runtime::SerializedV2Runtime::new_with_lifecycle_ordinals(
                        adapter,
                        Vec::new(),
                        started_at,
                        round_timeout,
                        queue_config,
                        lifecycle_ordinals,
                    )?;
                if !returned_effects.is_empty() {
                    return Err(
                        crate::sumeragi::v2_runtime::RuntimeConfigError::InvalidLifecycleOwnership,
                    );
                }
                Ok((runtime, pending, local_proposal_attempt))
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
    /// Join the exact WAL provenance to the already-fast-forwarded Apply boundary.
    ///
    /// Verification precedes effect consumption and repeats the canonical WAL
    /// replay join against the exact executor context. Owner-open already
    /// reconstructed Store, Validate, and the typed Ready Apply carrier, so the
    /// provenance is authenticated without dispatching its Fetch a second time.
    pub(in crate::sumeragi) fn install(
        self,
        executor: &mut crate::sumeragi::v2_effects::V2EffectExecutor<
            crate::sumeragi::v2_runtime::SerializedV2Runtime,
        >,
    ) -> Result<InstalledPendingKuraApplyV1, crate::sumeragi::v2_effects::EffectExecutorError> {
        let Self {
            expected,
            verified,
            wal_identity,
            replay_evidence,
            effect,
            apply_carrier,
        } = self;
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
        let genesis = executor.verify_pending_kura_recovered_apply_replay(
            expected,
            &effects,
            apply_carrier,
        )?;
        Ok(InstalledPendingKuraApplyV1 { expected, genesis })
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

    /// Open the recovered Decision Apply branch and attach the pending-tip replay seal.
    ///
    /// The embedded move-only Decision Fetch reconstructs and publishes the
    /// exact Ready Apply carrier. Only then is the inert provenance attached
    /// for closed-ingress interrupted-tip completion.
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
            .and_then(|owner| {
                owner
                    .with_pending_kura_apply_replay(replay)
                    .map_err(ProductionLifecycleOwnerStartupErrorV1::pending_kura_recovered_apply)
            })
    }
}
