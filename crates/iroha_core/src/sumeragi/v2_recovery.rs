//! Crash-safe selection of the one active Sumeragi v2 height context.
//!
//! A fresh chain consumes the signed, staged genesis bootstrap. Restart first
//! inspects Kura's immutable finality sidecars: a missing sidecar at the durable
//! tip means application/finality for that exact height must resume, while a
//! present sidecar authorizes construction of exactly one successor context.
//! Context records are persisted before the height WAL is opened.
use super::{
    v2::{
        AdapterError, RecoveredLifecycleOwnerKuraBindingV1, RecoveredLifecycleStorageAuthorityV1,
        VerifiedHeightContext,
    },
    v2_body_store::{BlockSignaturePolicy, V2BodyStore},
    v2_context::{
        AuthenticatedGenesisBodyV1, GenesisV2Bootstrap, StagedGenesisNexusAmxContext,
        V2ContextBuildError, build_successor_height_context_from_state,
    },
    v2_context_store::{PersistedHeightContext, V2ContextStore, V2ContextStoreError},
    v2_core::{
        CanonicalIdentityProjection, IDENTITY_DOMAIN_CONTEXT, IDENTITY_DOMAIN_DURABLE_ARTIFACT,
        IDENTITY_DOMAIN_SUBJECT, IDENTITY_KIND_BLOCK_HEADER, IDENTITY_KIND_FINALITY_ARTIFACT,
        IDENTITY_KIND_SNAPSHOT_BOOTSTRAP_RECORD, IDENTITY_KIND_WIRE_HEIGHT_CONTEXT,
        ProductionDurablePredecessorIdentityProjection,
        production_durable_predecessor_identity_kernel,
    },
    v2_first_release_recovery::{LifecycleContext, LifecycleDigest},
    v2_lane_work::durable_lane_completion_matches_finality_during_startup,
    v2_lifecycle_coordinator::LifecycleLedgerError,
};
use crate::{
    kura::{
        CommitManifestBindingState, ExactReplayBoundary, Kura, KuraInstanceIdentity,
        KuraV2CommitReceipt, V2StartupFinalityVerificationSession, V2StartupReplayStorageBinding,
    },
    state::{
        State, WorldReadOnly, live_consensus_key_pop_for_peer,
        public_lane_validator_record_matches_key,
    },
};
use iroha_crypto::{Hash, HashOf, KeyPair, PublicKey};
use iroha_data_model::{
    account::AccountId,
    block::{BlockHeader, consensus_v2 as wire},
    nexus::PublicLaneValidatorStatus,
};
use mv::storage::StorageReadOnly;
use std::{
    num::NonZeroUsize,
    path::{Path, PathBuf},
};
use thiserror::Error;
/// Authenticated boundary between generic Kura replay and one recoverable v2 tip.
///
/// Every full-body height through [`Self::complete_prefix_height`] has an exact WSV checkpoint,
/// a checkpoint-bound commit manifest, and a cryptographically verified finality artifact. The
/// only height outside that prefix may be the durable tip interrupted between Kura publication
/// and finality-sidecar publication.
#[derive(Clone, Debug)]
pub struct V2StartupReplayPlan {
    durable_height: usize,
    durable_boundary_hash: Hash,
    storage_binding: Option<V2StartupReplayStorageBinding>,
    audited_bootstrap_prefix_height: usize,
    complete_prefix_height: usize,
    pending_tip_height: Option<u64>,
}
impl PartialEq for V2StartupReplayPlan {
    fn eq(&self, other: &Self) -> bool {
        self.durable_height == other.durable_height
            && self.durable_boundary_hash == other.durable_boundary_hash
            && self.audited_bootstrap_prefix_height == other.audited_bootstrap_prefix_height
            && self.complete_prefix_height == other.complete_prefix_height
            && self.pending_tip_height == other.pending_tip_height
    }
}
impl Eq for V2StartupReplayPlan {}
const V2_STARTUP_REPLAY_BOUNDARY_HASH_DOMAIN: &[u8] =
    b"iroha:sumeragi:v2:startup-replay-boundary:v1\0";
const V2_EMERGENCY_FAST_REPLAY_BOUNDARY_HASH_DOMAIN: &[u8] =
    b"iroha:sumeragi:v2:emergency-fast-replay-boundary:v1\0";
fn v2_startup_replay_boundary_hash(boundary: &ExactReplayBoundary) -> Hash {
    let count = boundary.count.to_le_bytes();
    let mut chunks = Vec::with_capacity(boundary.hashes.len().saturating_add(2));
    chunks.push(V2_STARTUP_REPLAY_BOUNDARY_HASH_DOMAIN);
    chunks.push(count.as_slice());
    chunks.extend(boundary.hashes.iter().map(|hash| hash.as_ref().as_slice()));
    Hash::new_from_chunks(&chunks)
}
fn v2_emergency_fast_replay_boundary_hash(
    count: u64,
    tip_hash: Option<HashOf<BlockHeader>>,
) -> Hash {
    let count = count.to_le_bytes();
    Hash::new_from_chunks(&[
        V2_EMERGENCY_FAST_REPLAY_BOUNDARY_HASH_DOMAIN,
        count.as_slice(),
        tip_hash
            .as_ref()
            .map_or(&[][..], |hash| hash.as_ref().as_slice()),
    ])
}
/// Non-forgeable startup authorization for one exact imported snapshot lineage.
///
/// Construction is private to the v2 boundary verifier. The token owns the retained bootstrap
/// record and the complete signed State block-hash vector that were matched to Kura, so consuming
/// it is evidence that outer snapshot authentication, typed lineage validation, and first-full
/// finality checks all completed before the provisional store is promoted.
#[derive(Debug)]
pub struct AuthenticatedV2SnapshotStartup {
    record: wire::SnapshotV2BootstrapRecord,
    block_hashes: Vec<HashOf<BlockHeader>>,
    first_height_context: PersistedHeightContext,
}
impl AuthenticatedV2SnapshotStartup {
    /// Frozen consensus mode authenticated by the retained bootstrap lineage.
    #[must_use]
    pub const fn mode(&self) -> wire::ConsensusMode {
        self.record.context.mode
    }
    /// Consume the authorization into the exact evidence verified by the boundary.
    pub(crate) fn into_parts(
        self,
    ) -> (
        wire::SnapshotV2BootstrapRecord,
        Vec<HashOf<BlockHeader>>,
        PersistedHeightContext,
    ) {
        (self.record, self.block_hashes, self.first_height_context)
    }
}
impl V2StartupReplayPlan {
    /// Total canonical height durably recorded by Kura.
    #[must_use]
    pub const fn durable_height(&self) -> usize {
        self.durable_height
    }
    /// Highest historical height supplied by the typed audited snapshot import.
    #[must_use]
    pub const fn audited_bootstrap_prefix_height(&self) -> usize {
        self.audited_bootstrap_prefix_height
    }
    /// First executable full-body height after an audited snapshot prefix, when present.
    #[must_use]
    pub fn first_full_body_height(&self) -> Option<u64> {
        (self.audited_bootstrap_prefix_height < self.durable_height)
            .then(|| self.audited_bootstrap_prefix_height.saturating_add(1))
            .and_then(|height| u64::try_from(height).ok())
    }
    /// Return whether every durable height belongs to the audited snapshot import.
    #[must_use]
    pub const fn is_entirely_audited_snapshot_import(&self) -> bool {
        self.durable_height > 0 && self.audited_bootstrap_prefix_height == self.durable_height
    }
    /// Return whether startup is about to cross the audited snapshot boundary from this state.
    #[must_use]
    pub const fn requires_snapshot_bootstrap_at(&self, state_height: usize) -> bool {
        self.audited_bootstrap_prefix_height > 0
            && state_height == self.audited_bootstrap_prefix_height
    }
    /// Highest height which generic replay is permitted to execute.
    #[must_use]
    pub const fn complete_prefix_height(&self) -> usize {
        self.complete_prefix_height
    }
    /// Sole incomplete durable tip which must resume through the v2 Apply service.
    #[must_use]
    pub const fn pending_tip_height(&self) -> Option<u64> {
        self.pending_tip_height
    }
    fn validate_exact_kura_boundary(&self, kura: &Kura) -> Result<(), V2StartupReplayError> {
        let binding =
            self.storage_binding
                .as_ref()
                .ok_or(V2StartupReplayError::InvalidReplayMetadata {
                    height: u64::try_from(self.durable_height)?,
                    reason: "startup replay plan has no Kura-minted storage identity binding",
                })?;
        let binding_matches = if let Some(boundary) = binding.strict_replay_boundary() {
            usize::try_from(boundary.count)? == self.durable_height
                && v2_startup_replay_boundary_hash(boundary) == self.durable_boundary_hash
        } else if let Some((count, tip_hash)) = binding.emergency_fast_boundary() {
            usize::try_from(count)? == self.durable_height
                && v2_emergency_fast_replay_boundary_hash(count, tip_hash)
                    == self.durable_boundary_hash
        } else {
            false
        };
        if !binding_matches {
            return Err(V2StartupReplayError::InvalidReplayMetadata {
                height: u64::try_from(self.durable_height)?,
                reason: "startup replay plan disagrees with its Kura-minted storage binding",
            });
        }
        kura.validate_v2_startup_replay_storage_binding(binding)?;
        Ok(())
    }
    /// Validate that a restored WSV can be reconciled without skipping an incomplete height.
    ///
    /// # Errors
    ///
    /// Returns an error when WSV is ahead of Kura or lies beyond the authenticated prefix at a
    /// height other than the one recoverable durable tip. Emergency Fast mode also rejects a WSV
    /// behind the complete prefix instead of silently turning startup into historical replay.
    pub fn validate_restored_state_height(
        &self,
        state_height: usize,
    ) -> Result<(), V2StartupReplayError> {
        let emergency_fast = self
            .storage_binding
            .as_ref()
            .is_some_and(|binding| binding.emergency_fast_boundary().is_some());
        if state_height > self.durable_height {
            return Err(V2StartupReplayError::StateHeightOutsidePlan {
                state_height,
                durable_height: self.durable_height,
                complete_prefix_height: self.complete_prefix_height,
                pending_tip_height: self.pending_tip_height,
            });
        }
        if state_height > self.complete_prefix_height
            && self.pending_tip_height != u64::try_from(state_height).ok()
        {
            return Err(V2StartupReplayError::StateHeightOutsidePlan {
                state_height,
                durable_height: self.durable_height,
                complete_prefix_height: self.complete_prefix_height,
                pending_tip_height: self.pending_tip_height,
            });
        }
        if emergency_fast && state_height < self.complete_prefix_height {
            // Reconstructing WSV from historical bodies defeats the emergency startup bound and
            // would consume auxiliary evidence that Fast deliberately did not authenticate.
            // Require an already-current snapshot, or the exact predecessor of one pending tip.
            return Err(V2StartupReplayError::StateHeightOutsidePlan {
                state_height,
                durable_height: self.durable_height,
                complete_prefix_height: self.complete_prefix_height,
                pending_tip_height: self.pending_tip_height,
            });
        }
        Ok(())
    }
}
/// Select the startup replay boundary for Kura's configured initialization mode.
///
/// Strict mode inspects every durable height. Full bodies are trusted for generic replay only
/// after all replay/finality sidecars form one exact authenticated tuple. A missing tuple is a
/// recoverable crash image solely at the durable tip; an interior gap, multiple-height suffix,
/// impossible publication order, or corrupt binding fails closed. Only heights inside Kura's
/// typed audited-import boundary are exempt from the sidecar requirement.
///
/// Emergency Fast mode trusts the stable canonical journal, checks only the active tip needed by
/// recovery, and requires an already-current State snapshot so historical replay never begins.
///
/// # Errors
///
/// Returns [`V2StartupReplayError`] for malformed Kura metadata or a non-tip recovery gap.
pub fn plan_v2_startup_replay(kura: &Kura) -> Result<V2StartupReplayPlan, V2StartupReplayError> {
    if kura.emergency_fast_startup_enabled() {
        return plan_emergency_fast_v2_startup_replay(kura);
    }
    let planned = (|| {
        let mut startup_verification = kura.begin_v2_startup_finality_verification()?;
        if startup_verification.is_none() {
            kura.refresh_v2_startup_finality_verification()?;
            startup_verification = kura.begin_v2_startup_finality_verification()?;
        }
        let startup_verification =
            startup_verification.ok_or(V2StartupReplayError::InvalidReplayMetadata {
                height: u64::try_from(kura.exact_durable_blocks_count()?)?,
                reason: "Kura could not bind its verified startup storage inventory",
            })?;
        let mut plan = plan_v2_startup_replay_inner(kura, &startup_verification)?;
        plan.storage_binding = Some(startup_verification.storage_binding()?);
        Ok(plan)
    })();
    if planned.is_err() {
        kura.finish_v2_startup_finality_verification();
    }
    planned
}
/// Build a bounded emergency replay plan without scanning historical finality or replay sidecars.
///
/// Fast mode trusts the durable canonical journal and never enters consensus recovery. It requires
/// State to be restored at the exact durable height and leaves all active-height and finality
/// inspection to the next Strict restart.
fn plan_emergency_fast_v2_startup_replay(
    kura: &Kura,
) -> Result<V2StartupReplayPlan, V2StartupReplayError> {
    let storage_binding = kura.emergency_fast_startup_replay_binding()?;
    let (count, tip_hash) = storage_binding.emergency_fast_boundary().ok_or(
        V2StartupReplayError::InvalidReplayMetadata {
            height: 0,
            reason: "Kura returned a Strict binding for emergency Fast startup",
        },
    )?;
    let durable_height = usize::try_from(count)?;
    let durable_boundary_hash = v2_emergency_fast_replay_boundary_hash(count, tip_hash);
    iroha_logger::warn!(
        durable_height,
        "Sumeragi emergency Fast startup skipped all finality, checkpoint, manifest, and active-height recovery inspection"
    );
    Ok(V2StartupReplayPlan {
        durable_height,
        durable_boundary_hash,
        storage_binding: Some(storage_binding),
        audited_bootstrap_prefix_height: 0,
        complete_prefix_height: durable_height,
        pending_tip_height: None,
    })
}
fn plan_v2_startup_replay_inner(
    kura: &Kura,
    startup_verification: &V2StartupFinalityVerificationSession<'_>,
) -> Result<V2StartupReplayPlan, V2StartupReplayError> {
    let durable_boundary = startup_verification.replay_boundary();
    let durable_height = usize::try_from(durable_boundary.count)?;
    let durable_boundary_hash = v2_startup_replay_boundary_hash(durable_boundary);
    let durable_height_u64 = u64::try_from(durable_height)?;
    let mut complete_prefix_height = 0_usize;
    let mut audited_bootstrap_prefix_height = 0_usize;
    let mut previous_finality: Option<(u64, Hash, Option<Hash>)> = None;
    for height_index in 1..=durable_height {
        let nonzero = NonZeroUsize::new(height_index)
            .expect("startup replay iteration begins at non-zero height");
        let height = u64::try_from(height_index)?;
        if kura.is_audited_snapshot_import_height(nonzero) {
            if previous_finality.is_some() {
                return Err(V2StartupReplayError::InvalidReplayMetadata {
                    height,
                    reason: "audited snapshot import appears after an authenticated executable height",
                });
            }
            complete_prefix_height = height_index;
            audited_bootstrap_prefix_height = height_index;
            continue;
        }
        if startup_verification.is_hash_only_height(height) {
            return Err(V2StartupReplayError::InvalidReplayMetadata {
                height,
                reason: "zero-length unavailable body is outside the typed audited snapshot import",
            });
        }
        let checkpoint = startup_verification.wsv_checkpoint(height);
        let manifest = startup_verification.commit_manifest(height);
        let finality = startup_verification.finality_projection(height);
        match (checkpoint, manifest, finality) {
            (Some(_), Some(manifest), Some(finality)) => {
                if startup_verification.commit_manifest_binding_state(height, manifest)
                    != CommitManifestBindingState::Bound
                {
                    return Err(V2StartupReplayError::InvalidReplayMetadata {
                        height,
                        reason: "finality exists before the checkpoint published its manifest digest",
                    });
                }
                if !finality.binds_manifest(manifest) {
                    return Err(V2StartupReplayError::InvalidReplayMetadata {
                        height,
                        reason: "commit manifest does not bind the exact authenticated v2 finality artifact",
                    });
                }
                if previous_finality.is_none() && audited_bootstrap_prefix_height > 0 {
                    let anchor_height = u64::try_from(audited_bootstrap_prefix_height)?;
                    let anchor_hash = startup_verification.canonical_hash(anchor_height).ok_or(
                        V2StartupReplayError::InvalidReplayMetadata {
                            height,
                            reason: "hash-only snapshot prefix has no durable anchor hash",
                        },
                    )?;
                    let anchor_matches =
                        finality
                            .snapshot_bootstrap()
                            .is_some_and(|(height, block_hash)| {
                                height == anchor_height && block_hash == anchor_hash
                            });
                    if !anchor_matches {
                        return Err(V2StartupReplayError::InvalidReplayMetadata {
                            height,
                            reason: "first full-body artifact is not bound to the audited snapshot tip",
                        });
                    }
                } else if finality.snapshot_bootstrap().is_some() {
                    return Err(V2StartupReplayError::InvalidReplayMetadata {
                        height,
                        reason: "snapshot bootstrap anchor appears outside the first executable height",
                    });
                }
                if let Some((parent_height, parent_commit_qc_hash, parent_successor_authority_hash)) =
                    previous_finality
                    && parent_height.checked_add(1) == Some(height)
                {
                    if finality.parent_commit_qc_hash() != Some(parent_commit_qc_hash) {
                        return Err(V2StartupReplayError::FinalityChainMismatch { height });
                    }
                    if parent_successor_authority_hash != Some(finality.inherited_authority_hash())
                    {
                        return Err(V2StartupReplayError::FinalityAuthorityLineageMismatch {
                            height,
                        });
                    }
                }
                // Lane sidecars for historical incarnations may be retired by
                // canonical lifecycle changes. Only the durable tip is the
                // live crash boundary whose exact lane evidence must gate
                // successor activation.
                if height == durable_height_u64 {
                    let artifact = startup_verification
                        .durable_tip_finality_artifact(height)
                        .ok_or(V2StartupReplayError::InvalidReplayMetadata {
                            height,
                            reason: "durable-tip finality projection has no authenticated artifact",
                        })?;
                    match durable_lane_completion_matches_finality_during_startup(
                        startup_verification,
                        artifact,
                    ) {
                        Ok(true) => {}
                        Ok(false) => {
                            return Ok(V2StartupReplayPlan {
                                durable_height,
                                durable_boundary_hash,
                                storage_binding: None,
                                audited_bootstrap_prefix_height,
                                complete_prefix_height,
                                pending_tip_height: Some(height),
                            });
                        }
                        Err(_) => {
                            return Err(V2StartupReplayError::InvalidReplayMetadata {
                                height,
                                reason: "durable lane completion evidence conflicts with finalized ownership",
                            });
                        }
                    }
                }
                complete_prefix_height = height_index;
                previous_finality = Some((
                    height,
                    finality.commit_qc_hash(),
                    finality.successor_authority_hash(),
                ));
            }
            (_, _, Some(_)) => {
                return Err(V2StartupReplayError::InvalidReplayMetadata {
                    height,
                    reason: "finality exists without a complete checkpoint-bound manifest",
                });
            }
            (None, Some(_), None) => {
                return Err(V2StartupReplayError::InvalidReplayMetadata {
                    height,
                    reason: "commit manifest exists before its WSV checkpoint",
                });
            }
            (Some(_), Some(manifest), None) => {
                if startup_verification.commit_manifest_binding_state(height, manifest)
                    == CommitManifestBindingState::Mismatched
                {
                    return Err(V2StartupReplayError::InvalidReplayMetadata {
                        height,
                        reason: "checkpoint names a different commit manifest",
                    });
                }
                if height != durable_height_u64 {
                    return Err(V2StartupReplayError::IncompleteInteriorHeight {
                        height,
                        durable_height: durable_height_u64,
                    });
                }
                return Ok(V2StartupReplayPlan {
                    durable_height,
                    durable_boundary_hash,
                    storage_binding: None,
                    audited_bootstrap_prefix_height,
                    complete_prefix_height,
                    pending_tip_height: Some(height),
                });
            }
            (None, None, None) | (Some(_), None, None) => {
                if height != durable_height_u64 {
                    return Err(V2StartupReplayError::IncompleteInteriorHeight {
                        height,
                        durable_height: durable_height_u64,
                    });
                }
                return Ok(V2StartupReplayPlan {
                    durable_height,
                    durable_boundary_hash,
                    storage_binding: None,
                    audited_bootstrap_prefix_height,
                    complete_prefix_height,
                    pending_tip_height: Some(height),
                });
            }
        }
    }
    Ok(V2StartupReplayPlan {
        durable_height,
        durable_boundary_hash,
        storage_binding: None,
        audited_bootstrap_prefix_height,
        complete_prefix_height,
        pending_tip_height: None,
    })
}
/// Authenticate the first executable context after an audited snapshot import.
///
/// This check must run after the audited snapshot has been authenticated and before generic Kura
/// replay executes the first full body. It binds the snapshot's exact WSV, canonical Kura tip,
/// commit topology, live BLS keys, and frozen Nexus/AMX inputs to any already-durable first
/// finality artifact. This function is strictly read-only: the token-consuming Kura finalizer
/// publishes the immutable context only after it claims the provisional transition. A process may
/// not infer this trust root from local configuration or from a self-signed post-snapshot artifact.
///
/// # Errors
///
/// Returns an error when a hash-only prefix is not covered by the restored state, when the
/// authenticated snapshot record differs from any live or durable input, or when a conflicting
/// immutable height record already exists. Failure never creates or updates the context store.
pub fn authenticate_v2_snapshot_replay_boundary(
    kura: &Kura,
    state: &State,
    plan: &V2StartupReplayPlan,
) -> Result<(), V2StartupReplayError> {
    let state_height = state.committed_height();
    if plan.audited_bootstrap_prefix_height() == 0 {
        return Ok(());
    }
    if state_height < plan.audited_bootstrap_prefix_height() {
        return Err(snapshot_bootstrap_error(format!(
            "restored WSV height {state_height} does not cover audited hash-only prefix height {}",
            plan.audited_bootstrap_prefix_height()
        )));
    }
    authenticate_snapshot_hash_vector(kura, state)?;
    let record = state.authenticated_snapshot_v2_bootstrap().ok_or_else(|| {
        snapshot_bootstrap_error(
            "restored WSV with hash-only history has no retained authenticated v2 bootstrap lineage",
        )
    })?;
    if state_height > plan.audited_bootstrap_prefix_height() {
        authenticate_persisted_snapshot_boundary(kura, state, plan, record)?;
        return Ok(());
    }
    let _verified = authenticate_snapshot_bootstrap_record(kura, state, plan, record)?;
    // Compare every externally authenticated input before minting a publication capability. In
    // particular, a forged first artifact must not leave behind a context-store mutation.
    if let Some(first_full_height) = plan.first_full_body_height() {
        let first_full_index = NonZeroUsize::new(usize::try_from(first_full_height)?)
            .expect("first executable height is non-zero");
        let first_full_block = kura.get_block(first_full_index).ok_or_else(|| {
            snapshot_bootstrap_error(format!(
                "first full-body Kura height {first_full_height} has no canonical block body"
            ))
        })?;
        let anchor = record
            .context
            .snapshot_bootstrap
            .as_ref()
            .expect("verified snapshot record contains its anchor");
        if first_full_block.header().prev_block_hash() != Some(anchor.snapshot_block_hash) {
            return Err(snapshot_bootstrap_error(format!(
                "first full-body block at height {first_full_height} does not extend the authenticated snapshot tip {}",
                anchor.snapshot_block_hash
            )));
        }
        if let Some((artifact, _receipt)) =
            kura.v2_finality_artifact_with_receipt(first_full_height)?
        {
            authenticate_first_full_artifact(record, first_full_height, &artifact)?;
        }
    }
    Ok(())
}
/// Authenticate an imported snapshot startup and mint its single-use Kura authorization.
///
/// Ordinary full-body startup returns `Ok(None)`. An imported hash-only prefix returns a token only
/// after the exact signed State vector, retained original lineage, Kura anchor, immutable context,
/// and any required first-full finality artifact agree.
///
/// # Errors
///
/// Returns an error for any snapshot lineage, hash-vector, anchor, context, or finality mismatch.
pub fn authenticate_v2_snapshot_startup(
    kura: &Kura,
    state: &State,
    plan: &V2StartupReplayPlan,
) -> Result<Option<AuthenticatedV2SnapshotStartup>, V2StartupReplayError> {
    if plan.audited_bootstrap_prefix_height() == 0 {
        return Ok(None);
    }
    authenticate_v2_snapshot_replay_boundary(kura, state, plan)?;
    let payload = state
        .authenticated_snapshot_bootstrap_payload()
        .ok_or_else(|| {
            snapshot_bootstrap_error(
                "v2 boundary has no non-forgeable outer snapshot authentication payload",
            )
        })?;
    let record = payload.record().clone();
    let block_hashes = payload.block_hashes().to_vec();
    if state.authenticated_snapshot_v2_bootstrap() != Some(&record)
        || !state.committed_block_hashes_match(&block_hashes)
    {
        return Err(snapshot_bootstrap_error(
            "outer snapshot authentication payload differs from the verified live State",
        ));
    }
    let verified = VerifiedHeightContext::snapshot_bootstrap(&record)
        .map_err(|error| snapshot_bootstrap_error(error.to_string()))?;
    Ok(Some(AuthenticatedV2SnapshotStartup {
        record,
        block_hashes,
        first_height_context: PersistedHeightContext::from_verified(&verified),
    }))
}
/// Return the authenticated frozen mode for a ledger with a durable hash-only history prefix.
///
/// Both the original audited snapshot and every later signed snapshot retain the exact original
/// bootstrap lineage. The immutable first-height context and first full finality artifact are
/// cross-checked before its mode is returned.
///
/// # Errors
///
/// Returns an error when the immutable boundary record, first full artifact, Kura anchor, or live
/// network identity disagree.
pub fn authenticated_v2_snapshot_startup_mode(
    kura: &Kura,
    state: &State,
    plan: &V2StartupReplayPlan,
) -> Result<Option<wire::ConsensusMode>, V2StartupReplayError> {
    authenticate_v2_snapshot_startup(kura, state, plan)
        .map(|authorization| authorization.map(|authorization| authorization.mode()))
}
fn authenticate_persisted_snapshot_boundary(
    kura: &Kura,
    state: &State,
    plan: &V2StartupReplayPlan,
    record: &wire::SnapshotV2BootstrapRecord,
) -> Result<VerifiedHeightContext, V2StartupReplayError> {
    let verified = VerifiedHeightContext::snapshot_bootstrap(record)
        .map_err(|error| snapshot_bootstrap_error(error.to_string()))?;
    if record.context.network_id != *state.network_id_ref() {
        return Err(snapshot_bootstrap_error(
            "retained snapshot bootstrap lineage belongs to another chain",
        ));
    }
    let first_full_height = plan.first_full_body_height().ok_or_else(|| {
        snapshot_bootstrap_error(
            "state advanced beyond an all-hash-only prefix without a first executable height",
        )
    })?;
    if record.context.height != first_full_height {
        return Err(snapshot_bootstrap_error(format!(
            "retained snapshot context height {} differs from first full Kura height {first_full_height}",
            record.context.height
        )));
    }
    let persisted = V2ContextStore::load_from_root_read_only(
        kura.sumeragi_v2_storage_root(),
        first_full_height,
    )
    .map_err(|error| snapshot_bootstrap_error(error.to_string()))?;
    match persisted.as_ref() {
        Some(persisted)
            if persisted.context() != &record.context
                || persisted.proofs_of_possession() != record.validator_set_pops =>
        {
            return Err(snapshot_bootstrap_error(
                "retained snapshot bootstrap lineage differs from the immutable first-height context",
            ));
        }
        // Before the single-use startup authorization is consumed, the token-owning Kura
        // finalizer is deliberately the only writer allowed to publish this context. Absence is
        // therefore expected only while the exact imported-prefix metadata is still provisional.
        None if kura.provisional_snapshot_bootstrap_metadata().is_some() => {}
        None => {
            return Err(snapshot_bootstrap_error(
                "finalized snapshot lineage is missing its immutable first-height context",
            ));
        }
        Some(_) => {}
    }
    let anchor = record
        .context
        .snapshot_bootstrap
        .as_ref()
        .expect("verified persisted snapshot context contains an anchor");
    let expected_anchor_height = u64::try_from(plan.audited_bootstrap_prefix_height())?;
    let anchor_index = NonZeroUsize::new(plan.audited_bootstrap_prefix_height())
        .expect("snapshot prefix height is non-zero");
    let kura_anchor = kura
        .get_durable_block_hash(anchor_index)
        .ok_or_else(|| snapshot_bootstrap_error("Kura snapshot anchor hash is missing"))?;
    if anchor.snapshot_height != expected_anchor_height || anchor.snapshot_block_hash != kura_anchor
    {
        return Err(snapshot_bootstrap_error(
            "persisted snapshot context differs from Kura's authenticated hash-only anchor",
        ));
    }
    let (artifact, _receipt) = kura
        .v2_finality_artifact_with_receipt(first_full_height)?
        .ok_or_else(|| {
            snapshot_bootstrap_error(
                "retained snapshot bootstrap lineage has no verified first full finality artifact",
            )
        })?;
    authenticate_first_full_artifact(record, first_full_height, &artifact)?;
    Ok(verified)
}
fn authenticate_first_full_artifact(
    record: &wire::SnapshotV2BootstrapRecord,
    first_full_height: u64,
    artifact: &wire::finality::V2FinalityArtifact,
) -> Result<(), V2StartupReplayError> {
    if artifact.height != first_full_height
        || artifact.height_context != record.context
        || artifact.validator_set_pops != record.validator_set_pops
    {
        return Err(snapshot_bootstrap_error(format!(
            "first full-body finality at height {first_full_height} differs from the retained authenticated snapshot context or validator proofs"
        )));
    }
    let anchor = record
        .context
        .snapshot_bootstrap
        .as_ref()
        .expect("verified snapshot bootstrap record contains an anchor");
    if artifact.subject.parent_block_hash != Some(anchor.snapshot_block_hash) {
        return Err(snapshot_bootstrap_error(format!(
            "first full-body finality at height {first_full_height} does not extend the retained snapshot anchor"
        )));
    }
    Ok(())
}
fn authenticate_snapshot_hash_vector(
    kura: &Kura,
    state: &State,
) -> Result<(), V2StartupReplayError> {
    let state_height = state.committed_height();
    let durable_height = kura.exact_durable_blocks_count()?;
    if state_height > durable_height {
        return Err(snapshot_bootstrap_error(format!(
            "restored WSV block-hash vector length {} exceeds Kura height {}",
            state_height, durable_height
        )));
    }
    state.try_for_each_committed_block_hash(|height, state_hash| {
        let kura_hash = kura.get_durable_block_hash(height).ok_or_else(|| {
            snapshot_bootstrap_error(format!(
                "Kura block-hash vector is missing restored WSV height {}",
                height.get()
            ))
        })?;
        if state_hash != kura_hash {
            return Err(snapshot_bootstrap_error(format!(
                "restored WSV and Kura block-hash vectors differ at height {}",
                height.get()
            )));
        }
        Ok(())
    })?;
    Ok(())
}
fn authenticate_snapshot_bootstrap_record(
    kura: &Kura,
    state: &State,
    plan: &V2StartupReplayPlan,
    record: &wire::SnapshotV2BootstrapRecord,
) -> Result<VerifiedHeightContext, V2StartupReplayError> {
    let verified = VerifiedHeightContext::snapshot_bootstrap(record)
        .map_err(|error| snapshot_bootstrap_error(error.to_string()))?;
    let anchor = record
        .context
        .snapshot_bootstrap
        .as_ref()
        .expect("cryptographically verified bootstrap context contains an anchor");
    let anchor_height = u64::try_from(plan.audited_bootstrap_prefix_height())?;
    if anchor.snapshot_height != anchor_height {
        return Err(snapshot_bootstrap_error(format!(
            "snapshot anchor height {} differs from Kura hash-only prefix height {anchor_height}",
            anchor.snapshot_height
        )));
    }
    if record.context.network_id != *state.network_id_ref() {
        return Err(snapshot_bootstrap_error(format!(
            "snapshot bootstrap network id {} differs from live network id {}",
            record.context.network_id,
            state.network_id_ref()
        )));
    }
    let anchor_index = NonZeroUsize::new(plan.audited_bootstrap_prefix_height())
        .expect("authenticated snapshot prefix is non-zero");
    let kura_anchor = kura
        .get_durable_block_hash(anchor_index)
        .ok_or_else(|| snapshot_bootstrap_error("Kura has no durable hash-only anchor tip"))?;
    if anchor.snapshot_block_hash != kura_anchor {
        return Err(snapshot_bootstrap_error(format!(
            "snapshot anchor block {} differs from Kura hash-only tip {kura_anchor}",
            anchor.snapshot_block_hash
        )));
    }
    let state_tip = state.latest_block_hash_fast().ok_or_else(|| {
        snapshot_bootstrap_error("authenticated non-zero snapshot WSV has no committed tip")
    })?;
    if anchor.snapshot_block_hash != state_tip {
        return Err(snapshot_bootstrap_error(format!(
            "snapshot anchor block {} differs from restored WSV tip {state_tip}",
            anchor.snapshot_block_hash
        )));
    }
    let live_state_hash = crate::snapshot::canonical_state_snapshot_hash(state);
    if anchor.snapshot_state_hash != live_state_hash {
        return Err(snapshot_bootstrap_error(format!(
            "snapshot anchor WSV hash {:?} differs from restored canonical WSV hash {live_state_hash:?}",
            anchor.snapshot_state_hash
        )));
    }
    let block_cadence_ms = {
        let state_view = state.view();
        state_view
            .world()
            .parameters()
            .sumeragi()
            .block_cadence_ms()
            .get()
    };
    if block_cadence_ms == 0
        || anchor
            .snapshot_block_creation_time_ms
            .checked_add(block_cadence_ms)
            .is_none()
    {
        return Err(snapshot_bootstrap_error(
            "snapshot parent timestamp plus committed block cadence is not a positive representable wire timestamp",
        ));
    }
    let live_nexus_amx = committed_nexus_amx_context_hash(state);
    if record.context.nexus_amx_context_hash != live_nexus_amx {
        return Err(snapshot_bootstrap_error(format!(
            "snapshot bootstrap Nexus/AMX hash {:?} differs from restored WSV projection {live_nexus_amx:?}",
            record.context.nexus_amx_context_hash
        )));
    }
    let live_execution_policy = state
        .execution_policy_digest_v1()
        .map(Hash::prehashed)
        .map_err(|error| {
            snapshot_bootstrap_error(format!(
                "failed to derive restored execution-policy identity: {error}"
            ))
        })?;
    if record.context.execution_policy_hash != live_execution_policy {
        return Err(snapshot_bootstrap_error(format!(
            "snapshot bootstrap execution-policy hash {:?} differs from restored local policy {live_execution_policy:?}",
            record.context.execution_policy_hash
        )));
    }
    let commit_topology = state.commit_topology_snapshot();
    let roster_matches = commit_topology.len() == record.context.roster.len()
        && record
            .context
            .roster
            .iter()
            .zip(&commit_topology)
            .all(|(entry, peer)| entry.validator == *peer);
    if !roster_matches {
        return Err(snapshot_bootstrap_error(
            "snapshot bootstrap roster differs from the restored commit topology",
        ));
    }
    wire::finality::verify_validator_power_roster_pops(
        &record.context.roster,
        &record.validator_set_pops,
    )
    .map_err(|error| snapshot_bootstrap_error(error.to_string()))?;
    {
        let state_view = state.view();
        let world = state_view.world();
        for (index, (entry, expected_pop)) in record
            .context
            .roster
            .iter()
            .zip(&record.validator_set_pops)
            .enumerate()
        {
            let live_pop = live_consensus_key_pop_for_peer(
                world,
                &entry.validator,
                record.context.height,
            )
            .ok_or_else(|| {
                snapshot_bootstrap_error(format!(
                    "snapshot bootstrap validator {index} has no live BLS proof at height {}",
                    record.context.height
                ))
            })?;
            if live_pop != *expected_pop {
                return Err(snapshot_bootstrap_error(format!(
                    "snapshot bootstrap validator {index} proof differs from the restored live key"
                )));
            }
        }
    }
    Ok(verified)
}
fn snapshot_bootstrap_error(reason: impl Into<String>) -> V2StartupReplayError {
    V2StartupReplayError::SnapshotBootstrapAuthentication {
        reason: reason.into(),
    }
}
/// Fail-closed classification error for the startup replay boundary.
#[derive(Debug, Error)]
pub enum V2StartupReplayError {
    /// Kura sidecar or canonical-block validation failed.
    #[error(transparent)]
    Kura(#[from] crate::kura::Error),
    /// A persisted height cannot be represented locally or on the wire.
    #[error(transparent)]
    Integer(#[from] std::num::TryFromIntError),
    /// A missing replay/finality tuple was found before the durable tip.
    #[error(
        "Sumeragi v2 durable height {height} is incomplete inside the canonical prefix ending at {durable_height}"
    )]
    IncompleteInteriorHeight {
        /// Incomplete non-tip height.
        height: u64,
        /// Durable Kura tip.
        durable_height: u64,
    },
    /// Sidecars exist in an impossible order or do not form one exact tuple.
    #[error("invalid Sumeragi v2 replay metadata at height {height}: {reason}")]
    InvalidReplayMetadata {
        /// Affected height.
        height: u64,
        /// Stable fail-closed diagnostic.
        reason: &'static str,
    },
    /// Consecutive finality artifacts do not carry the exact parent CommitQC.
    #[error("Sumeragi v2 finality chain mismatch at height {height}")]
    FinalityChainMismatch {
        /// Child height whose frozen context names another parent.
        height: u64,
    },
    /// Consecutive artifacts disagree on the complete predecessor-authenticated authority.
    #[error("Sumeragi v2 finality authority lineage mismatch at height {height}")]
    FinalityAuthorityLineageMismatch {
        /// Child height whose election authority does not descend from its parent artifact.
        height: u64,
    },
    /// Restored WSV cannot be reached by replaying the authenticated prefix plus at most one tip.
    #[error(
        "Sumeragi v2 restored WSV height {state_height} is outside startup plan: durable={durable_height}, authenticated_prefix={complete_prefix_height}, pending_tip={pending_tip_height:?}"
    )]
    StateHeightOutsidePlan {
        /// Restored WSV height.
        state_height: usize,
        /// Durable Kura height.
        durable_height: usize,
        /// Highest generic-replay-safe height.
        complete_prefix_height: usize,
        /// Sole incomplete tip, when present.
        pending_tip_height: Option<u64>,
    },
    /// Audited snapshot state does not authenticate the exact first executable v2 context.
    #[error("Sumeragi v2 snapshot bootstrap authentication failed: {reason}")]
    SnapshotBootstrapAuthentication {
        /// Fail-closed diagnostic identifying the mismatched trust input.
        reason: String,
    },
}
/// Fully verified active-height inputs selected before network ingress opens.
pub(crate) struct RecoveredV2Height {
    verified_context: VerifiedHeightContext,
    context_store: V2ContextStore,
    signature_policy: BlockSignaturePolicy,
    lifecycle_storage_authority: RecoveredLifecycleStorageAuthorityV1,
    authenticated_genesis: Option<AuthenticatedGenesisBodyV1>,
    pending_kura_apply: Option<PendingKuraApply>,
    successor_activation: Option<RecoveredSuccessorActivationAuthority>,
    staged_genesis_nexus_amx_context: Option<StagedGenesisNexusAmxContext>,
}
/// Move-only permit proving recovery selected one exact Kura/context/policy tuple.
///
/// Only this module can construct the permit. The lifecycle adapter may consume
/// it to mint the storage authority, but cannot substitute any of the three
/// authenticated inputs or manufacture another permit from raw roots.
pub(in crate::sumeragi) struct RecoveredLifecycleStorageMintPermitV1 {
    kura_identity: KuraInstanceIdentity,
    genesis_account: AccountId,
    context_id: wire::HeightContextId,
    height: wire::Height,
    signature_policy: BlockSignaturePolicy,
}
impl RecoveredLifecycleStorageMintPermitV1 {
    fn new(
        kura: &Kura,
        verified: &VerifiedHeightContext,
        signature_policy: &BlockSignaturePolicy,
        genesis_account: &AccountId,
    ) -> Self {
        Self {
            kura_identity: kura.instance_identity(),
            genesis_account: genesis_account.clone(),
            context_id: verified.context().id(),
            height: verified.context().height,
            signature_policy: signature_policy.clone(),
        }
    }
    /// Construct the exact recovery permit for a sibling-module lifecycle fixture.
    ///
    /// Shipping code can mint this capability only inside recovery. The
    /// test-only bridge lets the production-factory fixture exercise the same
    /// consuming boundary without exposing a raw-root constructor.
    #[cfg(test)]
    pub(in crate::sumeragi) fn for_test(
        kura: &Kura,
        verified: &VerifiedHeightContext,
        signature_policy: &BlockSignaturePolicy,
        genesis_account: &AccountId,
    ) -> Self {
        Self::new(kura, verified, signature_policy, genesis_account)
    }
    /// Consume the permit while comparing every recovery-authenticated input.
    pub(in crate::sumeragi) fn authorizes(
        self,
        kura: &Kura,
        verified: &VerifiedHeightContext,
        signature_policy: &BlockSignaturePolicy,
        genesis_account: &AccountId,
    ) -> bool {
        self.kura_identity.matches(kura)
            && &self.genesis_account == genesis_account
            && self.context_id == verified.context().id()
            && self.height == verified.context().height
            && &self.signature_policy == signature_policy
    }
}
/// Exact durable predecessor identity retained across successor construction.
///
/// The artifact hash content-addresses the complete context, subject, CommitQC,
/// and artifact bytes after every independent receipt field is checked below.
/// A height/context pair alone is not sufficient: two same-height artifacts
/// can name different blocks, subjects, certificates, or artifact bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct DurableV2PredecessorIdentity {
    height: wire::Height,
    block_hash: HashOf<BlockHeader>,
    artifact_hash: HashOf<wire::finality::V2FinalityArtifact>,
}
fn successor_typed_identity<T>(
    domain: u8,
    kind: u8,
    hash: HashOf<T>,
) -> CanonicalIdentityProjection {
    CanonicalIdentityProjection::from_bytes(domain, kind, *hash.as_ref())
}
/// Project all bits of an authenticated height-context identifier.
pub(crate) fn successor_context_refinement_projection(
    context_id: wire::HeightContextId,
) -> CanonicalIdentityProjection {
    successor_typed_identity(
        IDENTITY_DOMAIN_CONTEXT,
        IDENTITY_KIND_WIRE_HEIGHT_CONTEXT,
        context_id.0,
    )
}
/// Project all bits of a canonical predecessor or snapshot block hash.
pub(crate) fn successor_block_refinement_projection(
    block_hash: HashOf<BlockHeader>,
) -> CanonicalIdentityProjection {
    successor_typed_identity(
        IDENTITY_DOMAIN_SUBJECT,
        IDENTITY_KIND_BLOCK_HEADER,
        block_hash,
    )
}
/// Project all bits of an authenticated audited-snapshot bootstrap record.
pub(crate) fn snapshot_record_refinement_projection(
    record_hash: HashOf<wire::SnapshotV2BootstrapRecord>,
) -> CanonicalIdentityProjection {
    successor_typed_identity(
        IDENTITY_DOMAIN_DURABLE_ARTIFACT,
        IDENTITY_KIND_SNAPSHOT_BOOTSTRAP_RECORD,
        record_hash,
    )
}
impl DurableV2PredecessorIdentity {
    /// Authenticate the complete immutable projection shared by a finality artifact and receipt.
    pub(crate) fn authenticate(
        artifact: &wire::finality::V2FinalityArtifact,
        receipt: &KuraV2CommitReceipt,
    ) -> Result<Self, V2RecoveryError> {
        let identity = Self {
            height: artifact.height,
            block_hash: artifact.block_hash,
            artifact_hash: HashOf::new(artifact),
        };
        if receipt.height() != identity.height
            || receipt.block_hash() != identity.block_hash
            || receipt.context_id() != artifact.context_id()
            || receipt.subject() != artifact.subject
            || receipt.certificate() != artifact.commit_qc.as_ref()
            || receipt.artifact_hash() != identity.artifact_hash
        {
            return Err(V2RecoveryError::DurablePredecessorAuthorityMismatch(
                artifact.height,
            ));
        }
        if !production_durable_predecessor_identity_kernel(identity.refinement_projection()) {
            return Err(V2RecoveryError::DurablePredecessorAuthorityMismatch(
                artifact.height,
            ));
        }
        Ok(identity)
    }
    /// Lossless primitive identity consumed by the shared production/Verus kernel.
    pub(crate) fn refinement_projection(self) -> ProductionDurablePredecessorIdentityProjection {
        ProductionDurablePredecessorIdentityProjection {
            height: self.height,
            block_hash: successor_typed_identity(
                IDENTITY_DOMAIN_SUBJECT,
                IDENTITY_KIND_BLOCK_HEADER,
                self.block_hash,
            ),
            artifact_hash: successor_typed_identity(
                IDENTITY_DOMAIN_DURABLE_ARTIFACT,
                IDENTITY_KIND_FINALITY_ARTIFACT,
                self.artifact_hash,
            ),
        }
    }
    /// Durable predecessor height.
    pub(crate) const fn height(self) -> wire::Height {
        self.height
    }
    /// Build a deterministic synthetic identity for runner ownership tests.
    #[cfg(test)]
    pub(crate) fn for_test(height: wire::Height, label: &[u8]) -> Self {
        let block_hash =
            HashOf::from_untyped_unchecked(Hash::new([b"test predecessor block", label].concat()));
        Self {
            height,
            block_hash,
            artifact_hash: HashOf::from_untyped_unchecked(Hash::new(
                [b"test predecessor artifact", label].concat(),
            )),
        }
    }
}
/// One-shot authority to publish a successor derived from a complete durable tip.
#[derive(Debug)]
pub(crate) struct DurableSuccessorActivationAuthority {
    predecessor: DurableV2PredecessorIdentity,
    successor_context_id: wire::HeightContextId,
}
impl DurableSuccessorActivationAuthority {
    /// Exact durable predecessor which owns this successor construction.
    pub(crate) const fn predecessor(&self) -> DurableV2PredecessorIdentity {
        self.predecessor
    }
    /// Frozen successor context authenticated from that predecessor.
    pub(crate) const fn successor_context_id(&self) -> wire::HeightContextId {
        self.successor_context_id
    }
    /// Consume the one-shot authority into its exact predecessor and successor context.
    pub(crate) const fn into_parts(self) -> (DurableV2PredecessorIdentity, wire::HeightContextId) {
        (self.predecessor, self.successor_context_id)
    }
    /// Build synthetic activation authority for runner boundary tests.
    #[cfg(test)]
    pub(crate) const fn for_test(
        predecessor: DurableV2PredecessorIdentity,
        successor_context_id: wire::HeightContextId,
    ) -> Self {
        Self {
            predecessor,
            successor_context_id,
        }
    }
}
/// Complete durable-tip evidence retained until predecessor retirement authorizes publication.
///
/// Unlike [`DurableSuccessorActivationAuthority`], this recovery-only owner keeps the exact Kura
/// finality artifact and its durable receipt alive beside the successor activation token. It is
/// deliberately move-only: startup may transfer the authority to the runner, but cannot copy the
/// predecessor evidence into a second publication path.
#[must_use]
pub(crate) struct RecoveredCompleteTipActivationAuthority {
    artifact: wire::finality::V2FinalityArtifact,
    receipt: KuraV2CommitReceipt,
    verified_predecessor: VerifiedHeightContext,
    predecessor_signature_policy: BlockSignaturePolicy,
    activation: DurableSuccessorActivationAuthority,
    lifecycle_storage: CanonicalCompleteTipLifecycleStorageV1,
    kura_identity: Option<KuraInstanceIdentity>,
}
impl std::fmt::Debug for RecoveredCompleteTipActivationAuthority {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let predecessor_signature_policy = match &self.predecessor_signature_policy {
            BlockSignaturePolicy::RotatingLeader => "rotating-leader",
            BlockSignaturePolicy::GenesisAuthority(_) => "genesis-authority",
        };
        formatter
            .debug_struct("RecoveredCompleteTipActivationAuthority")
            .field("predecessor", &self.activation.predecessor())
            .field(
                "successor_context_id",
                &self.activation.successor_context_id(),
            )
            .field(
                "predecessor_lifecycle_context_id",
                &self.lifecycle_storage.predecessor.context_id,
            )
            .field(
                "successor_lifecycle_context_id",
                &self.lifecycle_storage.successor.context_id,
            )
            .field(
                "predecessor_signature_policy",
                &predecessor_signature_policy,
            )
            .finish_non_exhaustive()
    }
}
/// One Kura-derived, context-addressed lifecycle publication target.
struct CanonicalLifecycleHeightStorageV1 {
    context_id: wire::HeightContextId,
    height: wire::Height,
    root: PathBuf,
}
impl CanonicalLifecycleHeightStorageV1 {
    fn from_kura(kura: &Kura, context_id: wire::HeightContextId, height: wire::Height) -> Self {
        Self {
            context_id,
            height,
            root: kura
                .sumeragi_v2_storage_root()
                .join("lifecycle-v1")
                .join(hex::encode(context_id.0.as_ref())),
        }
    }
}
/// Kura-derived lifecycle publication targets for one CompleteTip rollover.
///
/// Construction is private to recovery and derives both context-addressed roots
/// from the `Kura` used to authenticate the retained finality artifact. The
/// lifecycle ledger may compare an opened predecessor store with this target,
/// but no caller can substitute either raw path after CompleteTip authority is
/// minted.
struct CanonicalCompleteTipLifecycleStorageV1 {
    predecessor: CanonicalLifecycleHeightStorageV1,
    successor: CanonicalLifecycleHeightStorageV1,
    body_store_root: PathBuf,
    serve_payload_directory_authority:
        Option<crate::kura::KuraV2CertifiedServePayloadDirectoryAuthority>,
}
impl CanonicalCompleteTipLifecycleStorageV1 {
    fn from_kura(
        kura: &Kura,
        predecessor_context_id: wire::HeightContextId,
        predecessor_height: wire::Height,
        successor_context_id: wire::HeightContextId,
        successor_height: wire::Height,
    ) -> Self {
        Self {
            predecessor: CanonicalLifecycleHeightStorageV1::from_kura(
                kura,
                predecessor_context_id,
                predecessor_height,
            ),
            successor: CanonicalLifecycleHeightStorageV1::from_kura(
                kura,
                successor_context_id,
                successor_height,
            ),
            body_store_root: kura.sumeragi_v2_storage_root().join("bodies"),
            serve_payload_directory_authority: None,
        }
    }
}
impl RecoveredCompleteTipActivationAuthority {
    fn authenticate(
        artifact: wire::finality::V2FinalityArtifact,
        receipt: KuraV2CommitReceipt,
        verified_predecessor: VerifiedHeightContext,
        predecessor_signature_policy: BlockSignaturePolicy,
        verified_successor: &VerifiedHeightContext,
        activation: DurableSuccessorActivationAuthority,
        kura: &Kura,
    ) -> Result<Self, V2RecoveryError> {
        let lifecycle_storage = CanonicalCompleteTipLifecycleStorageV1::from_kura(
            kura,
            artifact.context_id(),
            artifact.height,
            verified_successor.context().id(),
            verified_successor.context().height,
        );
        let mut authenticated = Self::authenticate_exact(
            artifact,
            receipt,
            verified_predecessor,
            predecessor_signature_policy,
            verified_successor.context().id(),
            activation,
            lifecycle_storage,
            Some(kura.instance_identity()),
        )?;
        if !kura.emergency_fast_startup_enabled() {
            authenticated
                .lifecycle_storage
                .serve_payload_directory_authority =
                Some(kura.mint_v2_certified_serve_payload_directory_authority(
                    authenticated.verified_predecessor.context(),
                )?);
        }
        Ok(authenticated)
    }
    fn authenticate_exact(
        artifact: wire::finality::V2FinalityArtifact,
        receipt: KuraV2CommitReceipt,
        verified_predecessor: VerifiedHeightContext,
        predecessor_signature_policy: BlockSignaturePolicy,
        successor_context_id: wire::HeightContextId,
        activation: DurableSuccessorActivationAuthority,
        lifecycle_storage: CanonicalCompleteTipLifecycleStorageV1,
        kura_identity: Option<KuraInstanceIdentity>,
    ) -> Result<Self, V2RecoveryError> {
        if verified_predecessor.context() != &artifact.height_context
            || verified_predecessor.proofs_of_possession() != artifact.validator_set_pops.as_slice()
        {
            return Err(V2RecoveryError::ParentContextMismatch(artifact.height));
        }
        let predecessor = DurableV2PredecessorIdentity::authenticate(&artifact, &receipt)?;
        if activation.predecessor() != predecessor {
            return Err(V2RecoveryError::DurablePredecessorAuthorityMismatch(
                artifact.height,
            ));
        }
        if activation.successor_context_id() != successor_context_id {
            return Err(
                V2RecoveryError::RecoveredCompleteTipSuccessorAuthorityMismatch {
                    predecessor_height: artifact.height,
                },
            );
        }
        Ok(Self {
            artifact,
            receipt,
            verified_predecessor,
            predecessor_signature_policy,
            activation,
            lifecycle_storage,
            kura_identity,
        })
    }
    /// Exact durable predecessor whose lifecycle ledger must retire before publication.
    pub(crate) const fn predecessor(&self) -> DurableV2PredecessorIdentity {
        self.activation.predecessor()
    }
    /// Frozen successor context authenticated by the retained predecessor evidence.
    pub(crate) const fn successor_context_id(&self) -> wire::HeightContextId {
        self.activation.successor_context_id()
    }
    /// Recheck the complete retained predecessor finality evidence against one
    /// exact lifecycle replay authority.
    ///
    /// The comparison exposes no artifact, receipt, certificate, or activation
    /// parts. Lifecycle retirement uses it to bind a terminal Apply row to the
    /// same Kura-authenticated Commit decision which created the successor.
    pub(in crate::sumeragi) fn authorizes_terminal_apply_replay(
        &self,
        replay: &crate::sumeragi::v2_first_release_recovery::LifecycleReplayAuthorityV1,
    ) -> bool {
        DurableV2PredecessorIdentity::authenticate(&self.artifact, &self.receipt)
            .is_ok_and(|predecessor| predecessor == self.activation.predecessor())
            && replay.exactly_matches_complete_tip_finality(
                &self.artifact.height_context,
                &self.artifact.subject,
                &self.artifact.commit_qc,
            )
    }
    /// Return whether height-one CompleteTip may retire an empty genesis ledger.
    ///
    /// Signed genesis uses the authenticated bootstrap rather than the ordinary
    /// Decision body lifecycle, so its canonical height-one ledger is empty.
    /// The exception remains bound to the Kura-authenticated genesis artifact,
    /// genesis body-signature policy, and exact lifecycle context.
    pub(in crate::sumeragi) fn authorizes_empty_genesis_lifecycle(
        &self,
        context: LifecycleContext,
    ) -> bool {
        let verified = self.verified_predecessor.context();
        self.artifact.height == 1
            && self.artifact.height_context == *verified
            && verified.height == 1
            && verified.parent_commit_qc.is_none()
            && verified.snapshot_bootstrap.is_none()
            && matches!(
                &self.predecessor_signature_policy,
                BlockSignaturePolicy::GenesisAuthority(_)
            )
            && context.height() == 1
            && context.id().as_bytes() == self.artifact.context_id().0.as_ref()
    }
    /// Return whether an exact physically present non-genesis frame may be
    /// retired behind this canonical CompleteTip.
    ///
    /// A canonical-sync node can retain unrelated height-local work without
    /// ever owning the Decision Apply path for the block that finalized. Unlike
    /// signed genesis, this path requires rotating-leader finality and is useful
    /// only together with the store-minted physical-frame capability retained by
    /// the lifecycle ledger. A missing path therefore cannot borrow this Kura
    /// authority.
    pub(in crate::sumeragi) fn authorizes_retired_lifecycle(
        &self,
        context: LifecycleContext,
    ) -> bool {
        let verified = self.verified_predecessor.context();
        self.artifact.height > 1
            && self.artifact.height_context == *verified
            && verified.height == self.artifact.height
            && matches!(
                &self.predecessor_signature_policy,
                BlockSignaturePolicy::RotatingLeader
            )
            && DurableV2PredecessorIdentity::authenticate(&self.artifact, &self.receipt)
                .is_ok_and(|predecessor| predecessor == self.activation.predecessor())
            && context.height() == self.artifact.height
            && context.id().as_bytes() == self.artifact.context_id().0.as_ref()
    }
    /// Compare one opened lifecycle-ledger root with the exact Kura-bound
    /// predecessor target retained at CompleteTip authentication.
    pub(in crate::sumeragi) fn authorizes_predecessor_lifecycle_root(&self, root: &Path) -> bool {
        self.lifecycle_storage.predecessor.root == root
            && self.lifecycle_storage.predecessor.root != self.lifecycle_storage.successor.root
            && self.lifecycle_storage.predecessor.context_id == self.artifact.context_id()
            && self.lifecycle_storage.predecessor.height == self.artifact.height
            && self.lifecycle_storage.successor.context_id == self.activation.successor_context_id()
            && self.artifact.height.checked_add(1) == Some(self.lifecycle_storage.successor.height)
            && self.lifecycle_storage.predecessor.context_id
                != self.lifecycle_storage.successor.context_id
            && self.lifecycle_storage.body_store_root != self.lifecycle_storage.predecessor.root
            && self.lifecycle_storage.body_store_root != self.lifecycle_storage.successor.root
            && self.verified_predecessor.context() == &self.artifact.height_context
            && self.verified_predecessor.proofs_of_possession()
                == self.artifact.validator_set_pops.as_slice()
    }
    /// Compare one unopened successor target with the exact Kura-derived H+1 target.
    ///
    /// The caller supplies no roster or body authority: CompleteTip recovery
    /// publishes only an empty LedgerV1 successor, whose stable identity is the
    /// already-authenticated context id and height retained here.
    pub(in crate::sumeragi) fn authorizes_successor_lifecycle_target(
        &self,
        root: &Path,
        context: LifecycleContext,
    ) -> bool {
        self.lifecycle_storage.successor.root == root
            && self.lifecycle_storage.successor.context_id.0.as_ref() == context.id().as_bytes()
            && self.lifecycle_storage.successor.height == context.height()
            && self.lifecycle_storage.successor.context_id == self.activation.successor_context_id()
            && self.artifact.height.checked_add(1) == Some(context.height())
            && self.lifecycle_storage.predecessor.root != root
    }
    /// Reauthenticate one sealed verified H+1 context against CompleteTip.
    ///
    /// The context id fixes every wire field. The predecessor context, parent
    /// CommitQC, and exact next-roster proof sequence are checked separately so
    /// a different verified-context owner cannot borrow the canonical H+1
    /// lifecycle target merely by naming its height.
    pub(in crate::sumeragi) fn authorizes_verified_successor(
        &self,
        verified: &VerifiedHeightContext,
    ) -> bool {
        let expected_proofs = self
            .artifact
            .height_context
            .next_epoch_snapshot
            .as_ref()
            .map_or(self.artifact.validator_set_pops.as_slice(), |snapshot| {
                snapshot.validator_set_pops.as_slice()
            });
        verified.context().id() == self.activation.successor_context_id()
            && verified.context().height == self.lifecycle_storage.successor.height
            && verified.context().parent_commit_qc.as_ref() == Some(&self.artifact.commit_qc)
            && verified.verified_predecessor_context() == Some(&self.artifact.height_context)
            && verified.proofs_of_possession() == expected_proofs
    }
    /// Compare the unlaunched H+1 body owner with the Kura-derived body root.
    pub(in crate::sumeragi) fn authorizes_successor_body_store(
        &self,
        store: &V2BodyStore,
        verified: &VerifiedHeightContext,
    ) -> bool {
        self.authorizes_verified_successor(verified)
            && store.matches_lifecycle_storage_root(
                &self.lifecycle_storage.body_store_root,
                verified.context(),
                &BlockSignaturePolicy::RotatingLeader,
            )
    }
    /// Compare the successor lifecycle owner with the Kura that minted CompleteTip.
    pub(in crate::sumeragi) fn authorizes_successor_kura(
        &self,
        binding: Option<&RecoveredLifecycleOwnerKuraBindingV1>,
    ) -> bool {
        match (&self.kura_identity, binding) {
            (Some(expected), Some(actual)) => actual.matches_identity(expected),
            #[cfg(test)]
            (None, None) => true,
            _ => false,
        }
    }
    /// Compare the live predecessor-store owner with the Kura that authenticated CompleteTip.
    pub(in crate::sumeragi) fn authorizes_predecessor_kura(&self, kura: &Kura) -> bool {
        self.kura_identity
            .as_ref()
            .is_some_and(|expected| expected.matches(kura))
    }
    /// Compare every caller-visible predecessor-storage input in one closed oracle.
    ///
    /// Only the local signer remains caller-selected. Roots, contexts, PoPs,
    /// and the body signature policy must be the exact values retained when
    /// this CompleteTip authority was minted from Kura.
    pub(in crate::sumeragi) fn authorizes_predecessor_storage_inputs(
        &self,
        predecessor_root: &Path,
        successor_root: &Path,
        successor_context: LifecycleContext,
        body_store_root: &Path,
        verified_predecessor: &VerifiedHeightContext,
        signature_policy: &BlockSignaturePolicy,
    ) -> bool {
        self.authorizes_predecessor_lifecycle_root(predecessor_root)
            && self.authorizes_successor_lifecycle_target(successor_root, successor_context)
            && self.lifecycle_storage.body_store_root == body_store_root
            && self.verified_predecessor.context() == verified_predecessor.context()
            && self.verified_predecessor.proofs_of_possession()
                == verified_predecessor.proofs_of_possession()
            && &self.predecessor_signature_policy == signature_policy
    }
    /// Consume CompleteTip into the authenticated predecessor ledger/body/payload cut.
    ///
    /// The exact Kura-derived roots, predecessor context, and signature policy
    /// never cross this boundary as caller-supplied values. The local signer is
    /// used only to reauthenticate its frozen-roster Serve retention authority.
    pub(in crate::sumeragi) fn into_kura_bound_canonical_predecessor_storage(
        mut self,
        kura: &Kura,
        local_signer: &KeyPair,
    ) -> Result<
        crate::sumeragi::v2_first_release_recovery::AuthenticatedCompleteTipPredecessorStorageV1,
        crate::sumeragi::v2_first_release_recovery::CompleteTipPredecessorStorageErrorV1,
    > {
        let authority = self
            .lifecycle_storage
            .serve_payload_directory_authority
            .take()
            .ok_or_else(|| {
                LifecycleLedgerError::InvalidLedger(
                    "CompleteTip has no recovery-minted Certified-Serve payload authority"
                        .to_owned(),
                )
            })?;
        self.into_canonical_predecessor_storage_at(
            crate::sumeragi::v2_first_release_recovery::CompleteTipPayloadStoreOpenTargetV1::Kura {
                kura,
                authority,
            },
            local_signer,
        )
    }
    /// Open a raw-root CompleteTip fixture without exposing that path in production.
    #[cfg(test)]
    pub(in crate::sumeragi) fn into_canonical_predecessor_storage(
        self,
        local_signer: &KeyPair,
    ) -> Result<
        crate::sumeragi::v2_first_release_recovery::AuthenticatedCompleteTipPredecessorStorageV1,
        crate::sumeragi::v2_first_release_recovery::CompleteTipPredecessorStorageErrorV1,
    > {
        self.into_canonical_predecessor_storage_at(
            crate::sumeragi::v2_first_release_recovery::CompleteTipPayloadStoreOpenTargetV1::FixtureRoot,
            local_signer,
        )
    }
    fn into_canonical_predecessor_storage_at(
        self,
        payload_store_target: crate::sumeragi::v2_first_release_recovery::CompleteTipPayloadStoreOpenTargetV1<'_>,
        local_signer: &KeyPair,
    ) -> Result<
        crate::sumeragi::v2_first_release_recovery::AuthenticatedCompleteTipPredecessorStorageV1,
        crate::sumeragi::v2_first_release_recovery::CompleteTipPredecessorStorageErrorV1,
    > {
        let predecessor_root = self.lifecycle_storage.predecessor.root.clone();
        let successor_root = self.lifecycle_storage.successor.root.clone();
        let mut successor_context_id = [0_u8; 32];
        successor_context_id
            .copy_from_slice(self.lifecycle_storage.successor.context_id.0.as_ref());
        let successor_context = LifecycleContext::new(
            LifecycleDigest::new(successor_context_id),
            self.lifecycle_storage.successor.height,
        );
        let body_store_root = self.lifecycle_storage.body_store_root.clone();
        let verified_predecessor = self.verified_predecessor.clone();
        let signature_policy = self.predecessor_signature_policy.clone();
        crate::sumeragi::v2_first_release_recovery::open_complete_tip_predecessor_storage(
            payload_store_target,
            &predecessor_root,
            &successor_root,
            successor_context,
            &body_store_root,
            verified_predecessor,
            signature_policy,
            local_signer,
            self,
        )
    }
    /// Build exact recovered complete-tip authority for runner boundary tests.
    #[cfg(test)]
    pub(crate) fn authenticate_for_test(
        artifact: wire::finality::V2FinalityArtifact,
        receipt: KuraV2CommitReceipt,
        successor_context_id: wire::HeightContextId,
        activation: DurableSuccessorActivationAuthority,
    ) -> Result<Self, V2RecoveryError> {
        let verified_predecessor = VerifiedHeightContext::genesis(
            artifact.height_context.clone(),
            artifact.validator_set_pops.clone(),
        )?;
        let lifecycle_storage = CanonicalCompleteTipLifecycleStorageV1 {
            predecessor: CanonicalLifecycleHeightStorageV1 {
                context_id: artifact.context_id(),
                height: artifact.height,
                root: PathBuf::from("test-only-unbound-complete-tip-lifecycle-root"),
            },
            successor: CanonicalLifecycleHeightStorageV1 {
                context_id: successor_context_id,
                height: artifact.height.saturating_add(1),
                root: PathBuf::from("test-only-unbound-complete-tip-successor-root"),
            },
            body_store_root: PathBuf::from("test-only-unbound-complete-tip-body-root"),
            serve_payload_directory_authority: None,
        };
        Self::authenticate_exact(
            artifact,
            receipt,
            verified_predecessor,
            BlockSignaturePolicy::RotatingLeader,
            successor_context_id,
            activation,
            lifecycle_storage,
            None,
        )
    }
    /// Build exact CompleteTip authority bound to one test lifecycle root.
    #[cfg(test)]
    pub(in crate::sumeragi) fn authenticate_for_lifecycle_test(
        artifact: wire::finality::V2FinalityArtifact,
        receipt: KuraV2CommitReceipt,
        successor_context_id: wire::HeightContextId,
        activation: DurableSuccessorActivationAuthority,
        predecessor_root: &Path,
    ) -> Result<Self, V2RecoveryError> {
        let verified_predecessor = VerifiedHeightContext::genesis(
            artifact.height_context.clone(),
            artifact.validator_set_pops.clone(),
        )?;
        let predecessor_context_id = artifact.context_id();
        let predecessor_height = artifact.height;
        Self::authenticate_exact(
            artifact,
            receipt,
            verified_predecessor,
            BlockSignaturePolicy::RotatingLeader,
            successor_context_id,
            activation,
            CanonicalCompleteTipLifecycleStorageV1 {
                predecessor: CanonicalLifecycleHeightStorageV1 {
                    context_id: predecessor_context_id,
                    height: predecessor_height,
                    root: predecessor_root.to_path_buf(),
                },
                successor: CanonicalLifecycleHeightStorageV1 {
                    context_id: successor_context_id,
                    height: predecessor_height.saturating_add(1),
                    root: predecessor_root.join("test-only-successor"),
                },
                body_store_root: predecessor_root.join("test-only-body-root"),
                serve_payload_directory_authority: None,
            },
            None,
        )
    }
    /// Build exact test authority using the same Kura-derived target pair as production.
    #[cfg(test)]
    pub(in crate::sumeragi) fn authenticate_for_canonical_lifecycle_test(
        artifact: wire::finality::V2FinalityArtifact,
        receipt: KuraV2CommitReceipt,
        verified_predecessor: VerifiedHeightContext,
        predecessor_signature_policy: BlockSignaturePolicy,
        successor_context_id: wire::HeightContextId,
        activation: DurableSuccessorActivationAuthority,
        kura: &Kura,
    ) -> Result<Self, V2RecoveryError> {
        let lifecycle_storage = CanonicalCompleteTipLifecycleStorageV1::from_kura(
            kura,
            artifact.context_id(),
            artifact.height,
            successor_context_id,
            artifact.height.saturating_add(1),
        );
        let mut authenticated = Self::authenticate_exact(
            artifact,
            receipt,
            verified_predecessor,
            predecessor_signature_policy,
            successor_context_id,
            activation,
            lifecycle_storage,
            Some(kura.instance_identity()),
        )?;
        if !kura.emergency_fast_startup_enabled() {
            authenticated
                .lifecycle_storage
                .serve_payload_directory_authority =
                Some(kura.mint_v2_certified_serve_payload_directory_authority(
                    authenticated.verified_predecessor.context(),
                )?);
        }
        Ok(authenticated)
    }
}
/// Distinct one-shot authority for the first executable height after an audited snapshot.
///
/// Snapshot bootstrap has no historical CommitQC or Kura finality receipt. Keeping the complete
/// authenticated record in a separate type prevents it from being reused as durable predecessor
/// authority merely because its anchor has the same numeric height.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct SnapshotSuccessorActivationAuthority {
    record_hash: HashOf<wire::SnapshotV2BootstrapRecord>,
    snapshot_height: wire::Height,
    snapshot_block_hash: HashOf<BlockHeader>,
    successor_context_id: wire::HeightContextId,
}
impl SnapshotSuccessorActivationAuthority {
    fn new(record: &wire::SnapshotV2BootstrapRecord) -> Self {
        let anchor = record
            .context
            .snapshot_bootstrap
            .as_ref()
            .expect("verified snapshot activation authority retains its anchor");
        Self {
            record_hash: HashOf::new(record),
            snapshot_height: anchor.snapshot_height,
            snapshot_block_hash: anchor.snapshot_block_hash,
            successor_context_id: record.context.id(),
        }
    }
    /// Build exact synthetic snapshot authority for the status publication boundary.
    #[cfg(test)]
    pub(crate) const fn for_test(
        record_hash: HashOf<wire::SnapshotV2BootstrapRecord>,
        snapshot_height: wire::Height,
        snapshot_block_hash: HashOf<BlockHeader>,
        successor_context_id: wire::HeightContextId,
    ) -> Self {
        Self {
            record_hash,
            snapshot_height,
            snapshot_block_hash,
            successor_context_id,
        }
    }
    /// Imported snapshot height which anchors the first executable context.
    pub(crate) const fn snapshot_anchor_height(&self) -> wire::Height {
        self.snapshot_height
    }
    /// Frozen first executable context authenticated by the snapshot envelope.
    #[cfg(test)]
    pub(crate) const fn successor_context_id(&self) -> wire::HeightContextId {
        self.successor_context_id
    }
    /// Consume the exact record identity, imported anchor, and first executable context.
    pub(crate) const fn into_parts(
        self,
    ) -> (
        HashOf<wire::SnapshotV2BootstrapRecord>,
        wire::Height,
        HashOf<BlockHeader>,
        wire::HeightContextId,
    ) {
        (
            self.record_hash,
            self.snapshot_height,
            self.snapshot_block_hash,
            self.successor_context_id,
        )
    }
}
/// Typed startup activation source selected before network ingress opens.
#[derive(Debug)]
#[allow(variant_size_differences, clippy::large_enum_variant)]
pub(crate) enum RecoveredSuccessorActivationAuthority {
    /// A complete Kura tip retaining its exact finality artifact and receipt.
    CompleteTip(RecoveredCompleteTipActivationAuthority),
    /// An authenticated hash-only snapshot boundary without historical finality authority.
    SnapshotBootstrap(SnapshotSuccessorActivationAuthority),
}
/// Verified successor plus the exact durable authority which derived it.
pub(crate) struct VerifiedSuccessorHeight {
    verified_context: VerifiedHeightContext,
    activation: DurableSuccessorActivationAuthority,
    kura_identity: KuraInstanceIdentity,
}
impl VerifiedSuccessorHeight {
    /// Borrow the successor's frozen context in recovery fixtures.
    #[cfg(test)]
    pub(crate) const fn context(&self) -> &wire::HeightContext {
        self.verified_context.context()
    }
    /// Consume the successor into its runtime context and one-shot activation authority.
    pub(crate) fn into_parts(self) -> (VerifiedHeightContext, DurableSuccessorActivationAuthority) {
        (self.verified_context, self.activation)
    }

    /// Consume the verified successor into its runner parts and exact lifecycle storage seal.
    ///
    /// The Kura comparison is retained from the same State which authenticated
    /// successor construction. The signature policy is fixed to rotating leader
    /// for every post-genesis height; neither policy nor a storage root crosses
    /// this boundary as a caller-selected input.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(in crate::sumeragi) fn into_parts_with_lifecycle_storage_authority(
        self,
        kura: &Kura,
        genesis_account: &AccountId,
    ) -> Result<
        (
            VerifiedHeightContext,
            DurableSuccessorActivationAuthority,
            RecoveredLifecycleStorageAuthorityV1,
        ),
        V2RecoveryError,
    > {
        let Self {
            verified_context,
            activation,
            kura_identity,
        } = self;
        if !kura_identity.matches(kura) {
            return Err(V2RecoveryError::SuccessorLifecycleStorageKuraMismatch {
                height: verified_context.context().height,
            });
        }
        let signature_policy = BlockSignaturePolicy::RotatingLeader;
        let permit = RecoveredLifecycleStorageMintPermitV1::new(
            kura,
            &verified_context,
            &signature_policy,
            genesis_account,
        );
        let lifecycle_storage_authority =
            RecoveredLifecycleStorageAuthorityV1::mint_from_recovered_height(
                kura,
                &verified_context,
                &signature_policy,
                genesis_account,
                permit,
            )?;
        Ok((verified_context, activation, lifecycle_storage_authority))
    }
}
/// Canonical Kura tip which WAL/body replay must bind before ingress opens.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[must_use]
pub(crate) struct PendingKuraApply {
    context_id: wire::HeightContextId,
    height: wire::Height,
    state_height: wire::Height,
    block_hash: HashOf<BlockHeader>,
}
impl PendingKuraApply {
    /// Construct a pending-tip expectation for boundary unit tests.
    #[cfg(test)]
    pub(crate) const fn for_test(
        context_id: wire::HeightContextId,
        height: wire::Height,
        block_hash: HashOf<BlockHeader>,
    ) -> Self {
        Self {
            context_id,
            height,
            state_height: height.saturating_sub(1),
            block_hash,
        }
    }
    /// Frozen context identifier expected from the replayed Decision record.
    pub(crate) const fn context_id(self) -> wire::HeightContextId {
        self.context_id
    }
    /// Interrupted application height.
    pub(crate) const fn height(self) -> wire::Height {
        self.height
    }
    /// Committed state height authenticated when recovery selected this tip.
    pub(crate) const fn state_height(self) -> wire::Height {
        self.state_height
    }
    /// Canonical block already durable in Kura.
    pub(crate) const fn block_hash(self) -> HashOf<BlockHeader> {
        self.block_hash
    }
}
impl RecoveredV2Height {
    /// Borrow the exact verified context selected for this process lifetime.
    #[cfg(test)]
    pub(crate) const fn verified_context(&self) -> &VerifiedHeightContext {
        &self.verified_context
    }
    /// Return the Kura tip which reducer/body replay must prove exact before
    /// the caller opens network ingress.
    pub(crate) const fn pending_kura_apply(&self) -> Option<PendingKuraApply> {
        self.pending_kura_apply
    }
    /// Return the typed startup activation source selected by recovery.
    #[cfg(test)]
    pub(crate) const fn successor_activation(
        &self,
    ) -> Option<&RecoveredSuccessorActivationAuthority> {
        self.successor_activation.as_ref()
    }
    /// Consume recovery output into the height runner's owned parts.
    pub(in crate::sumeragi) fn into_parts(
        self,
    ) -> (
        VerifiedHeightContext,
        V2ContextStore,
        BlockSignaturePolicy,
        RecoveredLifecycleStorageAuthorityV1,
        Option<AuthenticatedGenesisBodyV1>,
        Option<RecoveredSuccessorActivationAuthority>,
        Option<StagedGenesisNexusAmxContext>,
    ) {
        (
            self.verified_context,
            self.context_store,
            self.signature_policy,
            self.lifecycle_storage_authority,
            self.authenticated_genesis,
            self.successor_activation,
            self.staged_genesis_nexus_amx_context,
        )
    }
}
/// Select and verify the only active v2 height after a fresh start or crash.
///
/// The caller must invoke this before opening consensus ingress. A context is
/// never inferred from mutable local configuration: height one comes from
/// signed genesis, and every successor is checked against the durable parent
/// artifact and current finalized state.
#[cfg(test)]
pub(crate) fn recover_active_height(
    kura: &Kura,
    state: &State,
    fresh_genesis: Option<GenesisV2Bootstrap>,
    genesis_public_key: PublicKey,
) -> Result<RecoveredV2Height, V2RecoveryError> {
    let replay_plan = plan_v2_startup_replay(kura)?;
    recover_active_height_with_plan(kura, state, fresh_genesis, genesis_public_key, replay_plan)
}
struct StartupFinalityInventoryCleanup<'a>(&'a Kura);
impl Drop for StartupFinalityInventoryCleanup<'_> {
    fn drop(&mut self) {
        self.0.finish_v2_startup_finality_verification();
    }
}
/// Select the active height from the exact replay plan already authenticated
/// by startup before the Sumeragi worker was launched.
pub(crate) fn recover_active_height_with_plan(
    kura: &Kura,
    state: &State,
    fresh_genesis: Option<GenesisV2Bootstrap>,
    genesis_public_key: PublicKey,
    replay_plan: V2StartupReplayPlan,
) -> Result<RecoveredV2Height, V2RecoveryError> {
    // Recovery consumes the O(H) startup-only inventory. Clear it on every
    // success and error exit; the fixed-size runtime LRU remains available.
    let _inventory_cleanup = StartupFinalityInventoryCleanup(kura);
    replay_plan.validate_exact_kura_boundary(kura)?;
    let storage_root = kura.sumeragi_v2_storage_root();
    let context_store = V2ContextStore::open(&storage_root)?;
    let durable_height = u64::try_from(replay_plan.durable_height())?;
    let state_height = u64::try_from(state.committed_height())?;
    let genesis_account = AccountId::new(genesis_public_key.clone());
    replay_plan.validate_restored_state_height(state.committed_height())?;
    if durable_height == 0 {
        if state_height != 0 {
            return Err(V2RecoveryError::StateKuraMismatch {
                state_height,
                durable_height,
            });
        }
        let fresh_genesis = fresh_genesis.ok_or(V2RecoveryError::MissingFreshGenesis)?;
        let (verified_context, staged_genesis_nexus_amx_context, authenticated_genesis) =
            fresh_genesis.into_parts();
        if !authenticated_genesis.authorizes(&genesis_public_key) {
            return Err(V2RecoveryError::FreshGenesisAuthorityMismatch);
        }
        ensure_execution_policy_matches_context(state, verified_context.context())?;
        context_store.persist(&PersistedHeightContext::from_verified(&verified_context))?;
        let signature_policy = BlockSignaturePolicy::GenesisAuthority(genesis_public_key);
        let lifecycle_storage_mint = RecoveredLifecycleStorageMintPermitV1::new(
            kura,
            &verified_context,
            &signature_policy,
            &genesis_account,
        );
        let lifecycle_storage_authority =
            RecoveredLifecycleStorageAuthorityV1::mint_from_recovered_height(
                kura,
                &verified_context,
                &signature_policy,
                &genesis_account,
                lifecycle_storage_mint,
            )?;
        return Ok(RecoveredV2Height {
            verified_context,
            context_store,
            signature_policy,
            lifecycle_storage_authority,
            authenticated_genesis: Some(authenticated_genesis),
            pending_kura_apply: None,
            successor_activation: None,
            staged_genesis_nexus_amx_context: Some(staged_genesis_nexus_amx_context),
        });
    }
    if state_height > durable_height || durable_height.saturating_sub(state_height) > 1 {
        return Err(V2RecoveryError::StateKuraMismatch {
            state_height,
            durable_height,
        });
    }
    verify_state_kura_prefix(kura, state, state_height)?;
    authenticate_v2_snapshot_replay_boundary(kura, state, &replay_plan)?;
    // A ledger imported entirely as an audited hash-only snapshot has no historical v2
    // CommitQC from which to derive a successor. Its authenticated snapshot envelope is the sole
    // explicit trust root for the first executable height; freeze that exact record before any
    // WAL or network ingress can open.
    if replay_plan.is_entirely_audited_snapshot_import() {
        if state_height != durable_height {
            return Err(V2RecoveryError::StateKuraMismatch {
                state_height,
                durable_height,
            });
        }
        let bootstrap = state
            .authenticated_snapshot_v2_bootstrap()
            .ok_or(V2RecoveryError::MissingSnapshotBootstrap)?;
        ensure_execution_policy_matches_context(state, &bootstrap.context)?;
        let record = context_store.load(bootstrap.context.height)?.ok_or(
            V2RecoveryError::MissingActiveContext(bootstrap.context.height),
        )?;
        if record.context() != &bootstrap.context
            || record.proofs_of_possession() != bootstrap.validator_set_pops
        {
            return Err(V2RecoveryError::SnapshotBootstrapContextMismatch(
                bootstrap.context.height,
            ));
        }
        let verified_context = VerifiedHeightContext::snapshot_bootstrap(bootstrap)?;
        let successor_activation = Some(RecoveredSuccessorActivationAuthority::SnapshotBootstrap(
            SnapshotSuccessorActivationAuthority::new(bootstrap),
        ));
        let signature_policy = BlockSignaturePolicy::RotatingLeader;
        let lifecycle_storage_mint = RecoveredLifecycleStorageMintPermitV1::new(
            kura,
            &verified_context,
            &signature_policy,
            &genesis_account,
        );
        let lifecycle_storage_authority =
            RecoveredLifecycleStorageAuthorityV1::mint_from_recovered_height(
                kura,
                &verified_context,
                &signature_policy,
                &genesis_account,
                lifecycle_storage_mint,
            )?;
        return Ok(RecoveredV2Height {
            verified_context,
            context_store,
            signature_policy,
            lifecycle_storage_authority,
            authenticated_genesis: None,
            pending_kura_apply: None,
            successor_activation,
            staged_genesis_nexus_amx_context: None,
        });
    }
    if replay_plan.pending_tip_height().is_none() {
        if state_height != durable_height {
            return Err(V2RecoveryError::FinalityAheadOfState {
                finality_height: durable_height,
                state_height,
            });
        }
        let (parent_artifact, parent_receipt) = kura
            .v2_finality_artifact_with_receipt(durable_height)?
            .ok_or(V2RecoveryError::MissingCompleteTipFinality(durable_height))?;
        let predecessor_record = context_store
            .load(durable_height)?
            .ok_or(V2RecoveryError::MissingActiveContext(durable_height))?;
        let verified_predecessor = verify_persisted_height(
            kura,
            state,
            &context_store,
            predecessor_record,
            durable_height,
        )?;
        let predecessor_signature_policy = if durable_height == 1 {
            BlockSignaturePolicy::GenesisAuthority(genesis_public_key.clone())
        } else {
            BlockSignaturePolicy::RotatingLeader
        };
        let successor =
            build_verified_successor(state, &context_store, &parent_artifact, &parent_receipt)?;
        let (verified_context, activation) = successor.into_parts();
        let complete_tip_activation = RecoveredCompleteTipActivationAuthority::authenticate(
            parent_artifact,
            parent_receipt,
            verified_predecessor,
            predecessor_signature_policy,
            &verified_context,
            activation,
            kura,
        )?;
        let signature_policy = BlockSignaturePolicy::RotatingLeader;
        let lifecycle_storage_mint = RecoveredLifecycleStorageMintPermitV1::new(
            kura,
            &verified_context,
            &signature_policy,
            &genesis_account,
        );
        let lifecycle_storage_authority =
            RecoveredLifecycleStorageAuthorityV1::mint_from_recovered_height(
                kura,
                &verified_context,
                &signature_policy,
                &genesis_account,
                lifecycle_storage_mint,
            )?;
        return Ok(RecoveredV2Height {
            verified_context,
            context_store,
            signature_policy,
            lifecycle_storage_authority,
            authenticated_genesis: None,
            pending_kura_apply: None,
            successor_activation: Some(RecoveredSuccessorActivationAuthority::CompleteTip(
                complete_tip_activation,
            )),
            staged_genesis_nexus_amx_context: None,
        });
    }
    if replay_plan.pending_tip_height() != Some(durable_height) {
        return Err(V2RecoveryError::MissingRecoverableTip(durable_height));
    }
    if state_height == durable_height {
        let checkpoint = kura.wsv_checkpoint(durable_height)?.ok_or(
            V2RecoveryError::AppliedPendingTipWithoutCheckpoint(durable_height),
        )?;
        let actual = crate::snapshot::canonical_state_snapshot_hash(state);
        if checkpoint.state_hash() != actual {
            return Err(V2RecoveryError::AppliedPendingTipCheckpointMismatch {
                height: durable_height,
                expected: checkpoint.state_hash(),
                actual,
            });
        }
    }
    // A pending canonical tip is the deliberate crash window either before
    // global finality publication or after global finality but before every
    // canonical lane ownership has its certificate and application receipt.
    // Resume exactly that height from its already-persisted context and WAL.
    let record = context_store
        .load(durable_height)?
        .ok_or(V2RecoveryError::MissingActiveContext(durable_height))?;
    let verified_context =
        verify_persisted_height(kura, state, &context_store, record, durable_height)?;
    let signature_policy = if durable_height == 1 {
        BlockSignaturePolicy::GenesisAuthority(genesis_public_key.clone())
    } else {
        BlockSignaturePolicy::RotatingLeader
    };
    let durable_index = NonZeroUsize::new(usize::try_from(durable_height)?)
        .ok_or(V2RecoveryError::MissingKuraPrefix(durable_height))?;
    let block_hash = kura
        .get_durable_block_hash(durable_index)
        .ok_or(V2RecoveryError::MissingKuraPrefix(durable_height))?;
    let pending_kura_apply = Some(PendingKuraApply {
        context_id: verified_context.context().id(),
        height: durable_height,
        state_height,
        block_hash,
    });
    let lifecycle_storage_mint = RecoveredLifecycleStorageMintPermitV1::new(
        kura,
        &verified_context,
        &signature_policy,
        &genesis_account,
    );
    let lifecycle_storage_authority =
        RecoveredLifecycleStorageAuthorityV1::mint_from_recovered_height(
            kura,
            &verified_context,
            &signature_policy,
            &genesis_account,
            lifecycle_storage_mint,
        )?;
    Ok(RecoveredV2Height {
        verified_context,
        context_store,
        signature_policy,
        lifecycle_storage_authority,
        authenticated_genesis: None,
        pending_kura_apply,
        successor_activation: None,
        staged_genesis_nexus_amx_context: None,
    })
}
fn verify_state_kura_prefix(
    kura: &Kura,
    state: &State,
    state_height: u64,
) -> Result<(), V2RecoveryError> {
    let Some(nonzero_height) = NonZeroUsize::new(usize::try_from(state_height)?) else {
        return Ok(());
    };
    let state_hash = state
        .committed_block_hash_at_height(state_height)
        .ok_or(V2RecoveryError::MissingStateTip(state_height))?;
    let kura_hash = kura
        .get_durable_block_hash(nonzero_height)
        .ok_or(V2RecoveryError::MissingKuraPrefix(state_height))?;
    if state_hash != kura_hash {
        return Err(V2RecoveryError::StateKuraHashMismatch {
            height: state_height,
            state_hash,
            kura_hash,
        });
    }
    Ok(())
}
/// Build or reopen the unique successor of one just-finalized height and
/// persist its immutable context before its safety WAL is opened.
pub(crate) fn build_verified_successor(
    state: &State,
    context_store: &V2ContextStore,
    parent_artifact: &wire::finality::V2FinalityArtifact,
    parent_receipt: &KuraV2CommitReceipt,
) -> Result<VerifiedSuccessorHeight, V2RecoveryError> {
    ensure_execution_policy_matches_context(state, &parent_artifact.height_context)?;
    let parent_height = parent_artifact.height;
    let predecessor = DurableV2PredecessorIdentity::authenticate(parent_artifact, parent_receipt)?;
    let state_height = u64::try_from(state.committed_height())?;
    let state_block_hash = state.committed_block_hash_at_height(state_height);
    if state_height != parent_height || state_block_hash != Some(predecessor.block_hash) {
        return Err(V2RecoveryError::FinalizedStatePredecessorMismatch {
            expected_height: parent_height,
            actual_height: state_height,
            expected_block_hash: predecessor.block_hash,
            actual_block_hash: state_block_hash,
        });
    }
    let parent_record = context_store
        .load(parent_height)?
        .ok_or(V2RecoveryError::MissingParentContext(parent_height))?;
    if parent_record.context() != &parent_artifact.height_context {
        return Err(V2RecoveryError::ParentContextMismatch(parent_height));
    }
    let target_height = parent_height
        .checked_add(1)
        .ok_or(V2RecoveryError::HeightOverflow)?;
    let state_view = state.view();
    let expected = build_successor_height_context_from_state(
        parent_artifact,
        &state_view,
        committed_nexus_amx_context_hash(state),
    )?;
    if expected.height != target_height {
        return Err(V2RecoveryError::HeightOverflow);
    }
    let record = match context_store.load(target_height)? {
        Some(record) => {
            if record.context() != &expected {
                return Err(V2RecoveryError::ConflictingDerivedContext(target_height));
            }
            record
        }
        None => {
            let proofs = successor_proofs_of_possession(parent_artifact);
            let verified = VerifiedHeightContext::successor(
                expected,
                proofs,
                parent_artifact,
                parent_receipt,
                parent_record.proofs_of_possession(),
            )?;
            context_store.persist(&PersistedHeightContext::from_verified(&verified))?;
            return Ok(VerifiedSuccessorHeight {
                activation: DurableSuccessorActivationAuthority {
                    predecessor,
                    successor_context_id: verified.context().id(),
                },
                verified_context: verified,
                kura_identity: state.kura().instance_identity(),
            });
        }
    };
    let verified_context = VerifiedHeightContext::successor(
        record.context().clone(),
        record.proofs_of_possession().to_vec(),
        parent_artifact,
        parent_receipt,
        parent_record.proofs_of_possession(),
    )?;
    Ok(VerifiedSuccessorHeight {
        activation: DurableSuccessorActivationAuthority {
            predecessor,
            successor_context_id: verified_context.context().id(),
        },
        verified_context,
        kura_identity: state.kura().instance_identity(),
    })
}
fn verify_persisted_height(
    kura: &Kura,
    state: &State,
    context_store: &V2ContextStore,
    record: PersistedHeightContext,
    height: wire::Height,
) -> Result<VerifiedHeightContext, V2RecoveryError> {
    ensure_execution_policy_matches_context(state, record.context())?;
    if height == 1 {
        return VerifiedHeightContext::genesis(
            record.context().clone(),
            record.proofs_of_possession().to_vec(),
        )
        .map_err(Into::into);
    }
    if record.context().snapshot_bootstrap.is_some() {
        let bootstrap = wire::SnapshotV2BootstrapRecord {
            version: wire::SnapshotV2BootstrapRecord::VERSION,
            context: record.context().clone(),
            validator_set_pops: record.proofs_of_possession().to_vec(),
        };
        let verified = VerifiedHeightContext::snapshot_bootstrap(&bootstrap)?;
        let anchor = bootstrap
            .context
            .snapshot_bootstrap
            .as_ref()
            .expect("verified snapshot bootstrap contains its anchor");
        if anchor.snapshot_height.checked_add(1) != Some(height) {
            return Err(V2RecoveryError::SnapshotBootstrapContextMismatch(height));
        }
        let anchor_index = NonZeroUsize::new(usize::try_from(anchor.snapshot_height)?)
            .ok_or(V2RecoveryError::SnapshotBootstrapContextMismatch(height))?;
        if !kura.is_audited_snapshot_import_height(anchor_index) {
            return Err(V2RecoveryError::SnapshotBootstrapParentIsNotAuditedImport {
                height,
                parent_height: anchor.snapshot_height,
            });
        }
        let kura_anchor = kura
            .get_durable_block_hash(anchor_index)
            .ok_or(V2RecoveryError::MissingKuraPrefix(anchor.snapshot_height))?;
        if kura_anchor != anchor.snapshot_block_hash {
            return Err(V2RecoveryError::SnapshotBootstrapAnchorMismatch {
                height,
                expected: anchor.snapshot_block_hash,
                actual: kura_anchor,
            });
        }
        if bootstrap.context.network_id != *state.network_id_ref() {
            return Err(V2RecoveryError::SnapshotBootstrapContextMismatch(height));
        }
        let state_height = u64::try_from(state.committed_height())?;
        if state_height == anchor.snapshot_height {
            let authenticated = state
                .authenticated_snapshot_v2_bootstrap()
                .ok_or(V2RecoveryError::MissingSnapshotBootstrap)?;
            if authenticated != &bootstrap {
                return Err(V2RecoveryError::SnapshotBootstrapContextMismatch(height));
            }
        } else if state_height != height {
            return Err(V2RecoveryError::StateKuraMismatch {
                state_height,
                durable_height: height,
            });
        } else if let Some(authenticated) = state.authenticated_snapshot_v2_bootstrap()
            && authenticated != &bootstrap
        {
            return Err(V2RecoveryError::SnapshotBootstrapContextMismatch(height));
        }
        return Ok(verified);
    }
    let parent_height = height
        .checked_sub(1)
        .ok_or(V2RecoveryError::HeightOverflow)?;
    let (parent_artifact, parent_receipt) = kura
        .v2_finality_artifact_with_receipt(parent_height)?
        .ok_or(V2RecoveryError::MissingParentFinality(parent_height))?;
    let parent_record = context_store
        .load(parent_height)?
        .ok_or(V2RecoveryError::MissingParentContext(parent_height))?;
    if parent_record.context() != &parent_artifact.height_context {
        return Err(V2RecoveryError::ParentContextMismatch(parent_height));
    }
    // Before state application, the successor projection is still
    // recomputable and must match the immutable record. After state application
    // the record is the only pre-state snapshot; the matching WAL, body marker,
    // and canonical Kura block complete the crash-recovery binding.
    let state_height = u64::try_from(state.committed_height())?;
    if state_height.saturating_add(1) == height {
        let state_view = state.view();
        let expected = build_successor_height_context_from_state(
            &parent_artifact,
            &state_view,
            committed_nexus_amx_context_hash(state),
        )?;
        if record.context() != &expected {
            return Err(V2RecoveryError::ConflictingDerivedContext(height));
        }
    }
    VerifiedHeightContext::successor(
        record.context().clone(),
        record.proofs_of_possession().to_vec(),
        &parent_artifact,
        &parent_receipt,
        parent_record.proofs_of_possession(),
    )
    .map_err(Into::into)
}
fn successor_proofs_of_possession(parent: &wire::finality::V2FinalityArtifact) -> Vec<Vec<u8>> {
    parent
        .height_context
        .next_epoch_snapshot
        .as_ref()
        .map_or_else(
            || parent.validator_set_pops.clone(),
            |snapshot| snapshot.validator_set_pops.clone(),
        )
}
pub(crate) fn committed_nexus_amx_context_hash(state: &State) -> Hash {
    let view = state.view();
    let active_validators = view
        .world()
        .public_lane_validators()
        .iter()
        .filter(|(key, record)| public_lane_validator_record_matches_key(key, record))
        .filter(|(_, record)| matches!(record.status, PublicLaneValidatorStatus::Active))
        .map(|(key, record)| (key.clone(), record.clone()))
        .collect::<Vec<_>>();
    let retained_lane_lineage = view
        .lane_incarnation_lineage
        .iter()
        .map(
            |(&lane_id, lineage)| iroha_config::parameters::actual::SumeragiV2LaneLifecycleEntry {
                lane_id,
                generation: lineage.generation,
                incarnation: lineage.incarnation,
                activation_height: lineage.activation_height,
            },
        )
        .collect::<Vec<_>>();
    iroha_config::parameters::actual::sumeragi_v2_nexus_amx_context_hash(
        &view.nexus,
        &view.pipeline,
        &active_validators,
        &retained_lane_lineage,
    )
}
pub(crate) fn committed_execution_policy_hash(state: &State) -> Result<Hash, V2RecoveryError> {
    state
        .execution_policy_digest_v1()
        .map(Hash::prehashed)
        .map_err(|error| V2RecoveryError::ExecutionPolicy(error.to_string()))
}
fn ensure_execution_policy_matches_context(
    state: &State,
    context: &wire::HeightContext,
) -> Result<(), V2RecoveryError> {
    let actual = committed_execution_policy_hash(state)?;
    if context.execution_policy_hash != actual {
        return Err(V2RecoveryError::ExecutionPolicyMismatch {
            expected: context.execution_policy_hash,
            actual,
        });
    }
    Ok(())
}
/// Fail-closed active-height selection error.
#[derive(Debug, Error)]
pub(crate) enum V2RecoveryError {
    /// Startup replay-boundary classification failed.
    #[error(transparent)]
    StartupReplay(#[from] V2StartupReplayError),
    /// Kura operation failed.
    #[error(transparent)]
    Kura(#[from] crate::kura::Error),
    /// Immutable context-store operation failed.
    #[error(transparent)]
    ContextStore(#[from] V2ContextStoreError),
    /// Height context construction failed.
    #[error(transparent)]
    Context(#[from] V2ContextBuildError),
    /// Cryptographic context verification failed.
    #[error(transparent)]
    Adapter(#[from] AdapterError),
    /// Local storage height cannot be represented on the wire.
    #[error(transparent)]
    Integer(#[from] std::num::TryFromIntError),
    /// Local execution policy could not be represented canonically.
    #[error("failed to derive the local V1 execution-policy identity: {0}")]
    ExecutionPolicy(String),
    /// Authenticated recovery context was created under a different boot execution policy.
    #[error(
        "authenticated execution-policy hash {expected:?} differs from local policy {actual:?}"
    )]
    ExecutionPolicyMismatch {
        /// Hash carried by signed genesis or persisted authenticated context.
        expected: Hash,
        /// Hash derived from the restored process-local policy snapshot.
        actual: Hash,
    },
    /// Empty Kura/WSV startup did not carry the signed genesis bootstrap.
    #[error("fresh Sumeragi v2 storage is missing its signed genesis bootstrap")]
    MissingFreshGenesis,
    /// Recovery's genesis signature policy key differs from the staged signed body.
    #[error("fresh Sumeragi v2 genesis authority differs from its staged signed body")]
    FreshGenesisAuthorityMismatch,
    /// A hash-only snapshot boundary lacks its authenticated first executable context.
    #[error("Sumeragi v2 hash-only snapshot boundary has no authenticated bootstrap record")]
    MissingSnapshotBootstrap,
    /// Immutable recovery context differs from the authenticated snapshot record.
    #[error("Sumeragi v2 snapshot bootstrap context mismatch at height {0}")]
    SnapshotBootstrapContextMismatch(wire::Height),
    /// A snapshot-anchored recovery context names a parent outside the typed imported prefix.
    #[error(
        "Sumeragi v2 snapshot bootstrap context at height {height} names unaudited parent height {parent_height}"
    )]
    SnapshotBootstrapParentIsNotAuditedImport {
        /// First executable height.
        height: wire::Height,
        /// Claimed audited snapshot height.
        parent_height: wire::Height,
    },
    /// Persisted bootstrap anchor differs from Kura's canonical hash-only tip.
    #[error(
        "Sumeragi v2 snapshot bootstrap anchor mismatch at height {height}: expected {expected}, actual {actual}"
    )]
    SnapshotBootstrapAnchorMismatch {
        /// First executable height.
        height: wire::Height,
        /// Block hash authenticated by the snapshot record.
        expected: HashOf<BlockHeader>,
        /// Durable Kura hash at the claimed anchor height.
        actual: HashOf<BlockHeader>,
    },
    /// State and Kura heights cannot arise from one interrupted apply.
    #[error(
        "Sumeragi v2 WSV height {state_height} is inconsistent with Kura height {durable_height}"
    )]
    StateKuraMismatch {
        /// Committed WSV height.
        state_height: u64,
        /// Durable canonical Kura height.
        durable_height: u64,
    },
    /// WSV height is non-zero but its committed hash journal has no tip.
    #[error("Sumeragi v2 WSV hash journal is missing its height {0} tip")]
    MissingStateTip(u64),
    /// Kura does not contain the WSV prefix height despite compatible counts.
    #[error("Sumeragi v2 Kura is missing the WSV prefix at height {0}")]
    MissingKuraPrefix(u64),
    /// WSV and Kura have different canonical hashes at their common prefix.
    #[error(
        "Sumeragi v2 WSV/Kura hash mismatch at height {height}: WSV {state_hash}, Kura {kura_hash}"
    )]
    StateKuraHashMismatch {
        /// Highest height applied to WSV.
        height: u64,
        /// WSV's committed block hash.
        state_hash: iroha_crypto::HashOf<iroha_data_model::block::BlockHeader>,
        /// Kura's durable block hash.
        kura_hash: iroha_crypto::HashOf<iroha_data_model::block::BlockHeader>,
    },
    /// A finality artifact exists for state which was not committed.
    #[error("Sumeragi v2 finality height {finality_height} is ahead of WSV height {state_height}")]
    FinalityAheadOfState {
        /// Durable sidecar height.
        finality_height: u64,
        /// Committed WSV height.
        state_height: u64,
    },
    /// A sidecar-complete durable tip disappeared between replay planning and use.
    #[error("missing authenticated Sumeragi v2 finality at complete tip height {0}")]
    MissingCompleteTipFinality(u64),
    /// Planner did not identify the only durable tip as recoverable.
    #[error("Sumeragi v2 durable tip {0} is neither complete nor recoverable")]
    MissingRecoverableTip(u64),
    /// WSV claims an incomplete tip was already applied without its exact persisted checkpoint.
    #[error("applied Sumeragi v2 pending tip {0} has no WSV checkpoint")]
    AppliedPendingTipWithoutCheckpoint(u64),
    /// Restored WSV differs from the checkpoint persisted for an incomplete tip.
    #[error(
        "applied Sumeragi v2 pending tip {height} checkpoint mismatch: expected {expected}, actual {actual}"
    )]
    AppliedPendingTipCheckpointMismatch {
        /// Interrupted tip height.
        height: u64,
        /// Durable Kura checkpoint hash.
        expected: Hash,
        /// Restored WSV hash.
        actual: Hash,
    },
    /// Interrupted active height has no immutable context record.
    #[error("missing Sumeragi v2 active context at height {0}")]
    MissingActiveContext(wire::Height),
    /// Durable parent artifact has no matching immutable context record.
    #[error("missing Sumeragi v2 parent context at height {0}")]
    MissingParentContext(wire::Height),
    /// Interrupted successor lacks its durable parent finality artifact.
    #[error("missing Sumeragi v2 parent finality artifact at height {0}")]
    MissingParentFinality(wire::Height),
    /// Parent record and finality artifact disagree.
    #[error("Sumeragi v2 parent context record differs from finality at height {0}")]
    ParentContextMismatch(wire::Height),
    /// A Kura finality receipt does not bind every field of the supplied parent artifact.
    #[error("Sumeragi v2 durable predecessor authority mismatch at height {0}")]
    DurablePredecessorAuthorityMismatch(wire::Height),
    /// Complete-tip recovery's activation token does not name the verified successor context.
    #[error(
        "Sumeragi v2 recovered complete-tip successor authority mismatch after predecessor height {predecessor_height}"
    )]
    RecoveredCompleteTipSuccessorAuthorityMismatch {
        /// Durable predecessor height whose successor binding changed.
        predecessor_height: wire::Height,
    },
    /// Finalized state does not end at the exact block named by the durable predecessor receipt.
    #[error(
        "Sumeragi v2 finalized state does not match durable predecessor: expected height {expected_height} block {expected_block_hash}, actual height {actual_height} block {actual_block_hash:?}"
    )]
    FinalizedStatePredecessorMismatch {
        /// Height authenticated by the durable finality artifact and receipt.
        expected_height: wire::Height,
        /// Current committed WSV height.
        actual_height: wire::Height,
        /// Block authenticated by the durable finality artifact and receipt.
        expected_block_hash: HashOf<BlockHeader>,
        /// Current committed WSV tip, if its hash journal is populated.
        actual_block_hash: Option<HashOf<BlockHeader>>,
    },
    /// Persisted successor differs from the unique projection of finalized state.
    #[error("persisted Sumeragi v2 context conflicts with finalized state at height {0}")]
    ConflictingDerivedContext(wire::Height),
    /// Successor storage projection was asked to cross the Kura instance used for verification.
    #[error("Sumeragi v2 successor lifecycle storage changed Kura ownership at height {height}")]
    SuccessorLifecycleStorageKuraMismatch {
        /// Verified successor height whose storage owner changed.
        height: wire::Height,
    },
    /// Height arithmetic overflowed.
    #[error("Sumeragi v2 height overflow")]
    HeightOverflow,
}
/// Build the exact clean-height-one recovery boundary used by the lifecycle
/// runner's CompleteTip restart regression.
#[cfg(all(test, feature = "bls"))]
pub(in crate::sumeragi) fn production_empty_genesis_complete_tip_fixture_for_test() -> (
    std::sync::Arc<crate::kura::Kura>,
    std::sync::Arc<crate::state::State>,
    crate::sumeragi::v2::VerifiedHeightContext,
    crate::sumeragi::v2::RecoveredLifecycleStorageAuthorityV1,
    iroha_crypto::KeyPair,
    crate::sumeragi::v2_lifecycle_coordinator::RetiredRecoveredCompleteTipActivationAuthorityV1,
) {
    tests::production_empty_genesis_complete_tip_fixture()
}

#[cfg(test)]
mod tests {
    include!("v2_recovery_tests.rs");
}
