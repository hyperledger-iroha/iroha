//! Crash-safe selection of the one active Sumeragi v2 height context.
//!
//! A fresh chain consumes the signed, staged genesis bootstrap. Restart first
//! inspects Kura's immutable finality sidecars: a missing sidecar at the durable
//! tip means application/finality for that exact height must resume, while a
//! present sidecar authorizes construction of exactly one successor context.
//! Context records are persisted before the height WAL is opened.

use std::num::NonZeroUsize;

use iroha_crypto::{Hash, HashOf, PublicKey};
use iroha_data_model::{
    block::{BlockHeader, consensus_v2 as wire},
    nexus::PublicLaneValidatorStatus,
};
use mv::storage::StorageReadOnly;
use thiserror::Error;

use super::{
    v2::{AdapterError, VerifiedHeightContext},
    v2_body_store::BlockSignaturePolicy,
    v2_context::{
        GenesisV2Bootstrap, StagedGenesisNexusAmxContext, V2ContextBuildError,
        build_successor_height_context_from_state,
    },
    v2_context_store::{PersistedHeightContext, V2ContextStore, V2ContextStoreError},
};
use crate::{
    kura::{CommitManifestBindingState, Kura, KuraV2CommitReceipt},
    state::{
        State, WorldReadOnly, live_consensus_key_pop_for_peer,
        public_lane_validator_record_matches_key,
    },
};

/// Authenticated boundary between generic Kura replay and one recoverable v2 tip.
///
/// Every full-body height through [`Self::complete_prefix_height`] has an exact WSV checkpoint,
/// a checkpoint-bound commit manifest, and a cryptographically verified finality artifact. The
/// only height outside that prefix may be the durable tip interrupted between Kura publication
/// and finality-sidecar publication.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct V2StartupReplayPlan {
    durable_height: usize,
    audited_bootstrap_prefix_height: usize,
    complete_prefix_height: usize,
    pending_tip_height: Option<u64>,
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

    /// Validate that a restored WSV can be reconciled without skipping an incomplete height.
    ///
    /// # Errors
    ///
    /// Returns an error when WSV is ahead of Kura or lies beyond the authenticated prefix at a
    /// height other than the one recoverable durable tip.
    pub fn validate_restored_state_height(
        &self,
        state_height: usize,
    ) -> Result<(), V2StartupReplayError> {
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
        Ok(())
    }
}

/// Inspect every durable Kura height and select the only safe generic-replay boundary.
///
/// Full bodies are trusted for generic replay only after all replay/finality sidecars form one
/// exact authenticated tuple. A missing tuple is a recoverable crash image solely at the durable
/// tip; an interior gap, multiple-height suffix, impossible publication order, or corrupt binding
/// fails closed. Only heights inside Kura's typed audited-import boundary are exempt from the
/// sidecar requirement, whether or not a legacy body happens to remain locally available.
///
/// # Errors
///
/// Returns [`V2StartupReplayError`] for malformed Kura metadata or a non-tip recovery gap.
pub fn plan_v2_startup_replay(kura: &Kura) -> Result<V2StartupReplayPlan, V2StartupReplayError> {
    let durable_height = kura.exact_durable_blocks_count()?;
    let durable_height_u64 = u64::try_from(durable_height)?;
    let mut complete_prefix_height = 0_usize;
    let mut audited_bootstrap_prefix_height = 0_usize;
    let mut previous_finality: Option<(u64, wire::finality::V2FinalityArtifact)> = None;

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
        if kura.is_hash_only_block_height(nonzero) {
            return Err(V2StartupReplayError::InvalidReplayMetadata {
                height,
                reason: "zero-length unavailable body is outside the typed audited snapshot import",
            });
        }

        let checkpoint = kura.wsv_checkpoint(height)?;
        let manifest = kura.commit_manifest(height)?;
        let finality = kura.v2_finality_artifact_with_receipt(height)?;

        match (checkpoint.as_ref(), manifest.as_ref(), finality.as_ref()) {
            (Some(_), Some(manifest), Some((artifact, _))) => {
                if kura.commit_manifest_binding_state(manifest)?
                    != CommitManifestBindingState::Bound
                {
                    return Err(V2StartupReplayError::InvalidReplayMetadata {
                        height,
                        reason: "finality exists before the checkpoint published its manifest digest",
                    });
                }
                if !manifest.binds_authenticated_v2_commit_authority(artifact) {
                    return Err(V2StartupReplayError::InvalidReplayMetadata {
                        height,
                        reason: "commit manifest does not bind the exact authenticated v2 finality artifact",
                    });
                }
                if previous_finality.is_none() && audited_bootstrap_prefix_height > 0 {
                    let anchor_height = u64::try_from(audited_bootstrap_prefix_height)?;
                    let anchor_index = NonZeroUsize::new(audited_bootstrap_prefix_height)
                        .expect("non-empty audited prefix has a non-zero tip");
                    let anchor_hash = kura.get_durable_block_hash(anchor_index).ok_or(
                        V2StartupReplayError::InvalidReplayMetadata {
                            height,
                            reason: "hash-only snapshot prefix has no durable anchor hash",
                        },
                    )?;
                    let anchor_matches = artifact
                        .height_context
                        .snapshot_bootstrap
                        .as_ref()
                        .is_some_and(|anchor| {
                            anchor.snapshot_height == anchor_height
                                && anchor.snapshot_block_hash == anchor_hash
                        });
                    if !anchor_matches {
                        return Err(V2StartupReplayError::InvalidReplayMetadata {
                            height,
                            reason: "first full-body artifact is not bound to the audited snapshot tip",
                        });
                    }
                } else if artifact.height_context.snapshot_bootstrap.is_some() {
                    return Err(V2StartupReplayError::InvalidReplayMetadata {
                        height,
                        reason: "snapshot bootstrap anchor appears outside the first executable height",
                    });
                }
                if let Some((parent_height, parent)) = previous_finality.as_ref()
                    && parent_height.checked_add(1) == Some(height)
                    && artifact.height_context.parent_commit_qc.as_ref() != Some(&parent.commit_qc)
                {
                    return Err(V2StartupReplayError::FinalityChainMismatch { height });
                }
                complete_prefix_height = height_index;
                previous_finality = Some((height, artifact.clone()));
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
                if kura.commit_manifest_binding_state(manifest)?
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
                    audited_bootstrap_prefix_height,
                    complete_prefix_height,
                    pending_tip_height: Some(height),
                });
            }
        }
    }

    Ok(V2StartupReplayPlan {
        durable_height,
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
        || state.committed_block_hashes_snapshot() != block_hashes
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
/// chain identity disagree.
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
    if record.context.chain_id != *state.chain_id_ref() {
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
    let state_hashes = state.committed_block_hashes_snapshot();
    if state_hashes.len() != state.committed_height() {
        return Err(snapshot_bootstrap_error(format!(
            "restored WSV block-hash vector length {} differs from committed height {}",
            state_hashes.len(),
            state.committed_height()
        )));
    }
    let durable_height = kura.exact_durable_blocks_count()?;
    if state_hashes.len() > durable_height {
        return Err(snapshot_bootstrap_error(format!(
            "restored WSV block-hash vector length {} exceeds Kura height {}",
            state_hashes.len(),
            durable_height
        )));
    }
    for (index, state_hash) in state_hashes.into_iter().enumerate() {
        let height = index
            .checked_add(1)
            .and_then(NonZeroUsize::new)
            .expect("enumerated block height is non-zero");
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
    }
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
    if record.context.chain_id != *state.chain_id_ref() {
        return Err(snapshot_bootstrap_error(format!(
            "snapshot bootstrap chain id {} differs from live chain id {}",
            record.context.chain_id,
            state.chain_id_ref()
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
    pending_kura_apply: Option<PendingKuraApply>,
    successor_activation_parent: Option<wire::Height>,
    staged_genesis_nexus_amx_context: Option<StagedGenesisNexusAmxContext>,
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

    /// Return the complete durable parent whose recovered successor must cross
    /// the same live activation boundary as an uninterrupted rollover.
    pub(crate) const fn successor_activation_parent(&self) -> Option<wire::Height> {
        self.successor_activation_parent
    }

    /// Consume recovery output into the height runner's owned parts.
    pub(crate) fn into_parts(
        self,
    ) -> (
        VerifiedHeightContext,
        V2ContextStore,
        BlockSignaturePolicy,
        Option<StagedGenesisNexusAmxContext>,
    ) {
        (
            self.verified_context,
            self.context_store,
            self.signature_policy,
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
pub(crate) fn recover_active_height(
    kura: &Kura,
    state: &State,
    fresh_genesis: Option<GenesisV2Bootstrap>,
    genesis_public_key: PublicKey,
) -> Result<RecoveredV2Height, V2RecoveryError> {
    let storage_root = kura.sumeragi_v2_storage_root();
    let context_store = V2ContextStore::open(&storage_root)?;
    let replay_plan = plan_v2_startup_replay(kura)?;
    let durable_height = u64::try_from(replay_plan.durable_height())?;
    let state_height = u64::try_from(state.committed_height())?;
    replay_plan.validate_restored_state_height(state.committed_height())?;

    if durable_height == 0 {
        if state_height != 0 {
            return Err(V2RecoveryError::StateKuraMismatch {
                state_height,
                durable_height,
            });
        }
        let fresh_genesis = fresh_genesis.ok_or(V2RecoveryError::MissingFreshGenesis)?;
        let (verified_context, staged_genesis_nexus_amx_context) = fresh_genesis.into_parts();
        context_store.persist(&PersistedHeightContext::from_verified(&verified_context))?;
        return Ok(RecoveredV2Height {
            verified_context,
            context_store,
            signature_policy: BlockSignaturePolicy::GenesisAuthority(genesis_public_key),
            pending_kura_apply: None,
            successor_activation_parent: None,
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
        let successor_activation_parent = bootstrap
            .context
            .snapshot_bootstrap
            .as_ref()
            .map(|anchor| anchor.snapshot_height);
        let verified_context = VerifiedHeightContext::snapshot_bootstrap(bootstrap)?;
        return Ok(RecoveredV2Height {
            verified_context,
            context_store,
            signature_policy: BlockSignaturePolicy::RotatingLeader,
            pending_kura_apply: None,
            successor_activation_parent,
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
        let verified_context =
            build_verified_successor(state, &context_store, &parent_artifact, &parent_receipt)?;
        return Ok(RecoveredV2Height {
            verified_context,
            context_store,
            signature_policy: BlockSignaturePolicy::RotatingLeader,
            pending_kura_apply: None,
            successor_activation_parent: Some(durable_height),
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

    // A canonical block without its v2 sidecar is the deliberate crash window
    // between Kura/WSV application and finality-artifact persistence. Resume
    // exactly that height from its already-persisted context and WAL.
    let record = context_store
        .load(durable_height)?
        .ok_or(V2RecoveryError::MissingActiveContext(durable_height))?;
    let verified_context =
        verify_persisted_height(kura, state, &context_store, record, durable_height)?;
    let signature_policy = if durable_height == 1 {
        BlockSignaturePolicy::GenesisAuthority(genesis_public_key)
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
    Ok(RecoveredV2Height {
        verified_context,
        context_store,
        signature_policy,
        pending_kura_apply,
        successor_activation_parent: None,
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
        .committed_block_hashes_snapshot()
        .last()
        .copied()
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
) -> Result<VerifiedHeightContext, V2RecoveryError> {
    let parent_height = parent_artifact.height;
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
            return Ok(verified);
        }
    };
    VerifiedHeightContext::successor(
        record.context().clone(),
        record.proofs_of_possession().to_vec(),
        parent_artifact,
        parent_receipt,
        parent_record.proofs_of_possession(),
    )
    .map_err(Into::into)
}

fn verify_persisted_height(
    kura: &Kura,
    state: &State,
    context_store: &V2ContextStore,
    record: PersistedHeightContext,
    height: wire::Height,
) -> Result<VerifiedHeightContext, V2RecoveryError> {
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
        if bootstrap.context.chain_id != *state.chain_id_ref() {
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
    let lane_lifecycle = view
        .nexus
        .lane_catalog
        .lanes()
        .iter()
        .map(
            |lane| iroha_config::parameters::actual::SumeragiV2LaneLifecycleEntry {
                lane_id: lane.id,
                incarnation: *view
                    .lane_incarnations
                    .get(&lane.id)
                    .expect("validated state view has every active lane incarnation"),
                activation_height: *view
                    .lane_incarnation_activation_heights
                    .get(&lane.id)
                    .expect("validated state view has every lane activation height"),
            },
        )
        .collect::<Vec<_>>();
    iroha_config::parameters::actual::sumeragi_v2_nexus_amx_context_hash(
        &view.nexus,
        &view.pipeline,
        &active_validators,
        &lane_lifecycle,
    )
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
    /// Empty Kura/WSV startup did not carry the signed genesis bootstrap.
    #[error("fresh Sumeragi v2 storage is missing its signed genesis bootstrap")]
    MissingFreshGenesis,
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
    /// Persisted successor differs from the unique projection of finalized state.
    #[error("persisted Sumeragi v2 context conflicts with finalized state at height {0}")]
    ConflictingDerivedContext(wire::Height),
    /// Height arithmetic overflowed.
    #[error("Sumeragi v2 height overflow")]
    HeightOverflow,
}

#[cfg(test)]
mod tests {
    use std::{
        io::Write,
        num::{NonZeroU64, NonZeroUsize},
        path::{Path, PathBuf},
        sync::Arc,
    };

    use iroha_config::parameters::actual::LaneConfig;
    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair, Signature};
    use iroha_data_model::{
        ChainId,
        block::{BlockHeader, SignedBlock, consensus_v2 as wire},
        consensus::{ConsensusKeyId, ConsensusKeyRecord, ConsensusKeyRole, ConsensusKeyStatus},
        peer::PeerId,
    };

    use super::{
        V2RecoveryError, V2StartupReplayError, authenticate_v2_snapshot_replay_boundary,
        authenticate_v2_snapshot_startup, authenticated_v2_snapshot_startup_mode,
        build_verified_successor, committed_nexus_amx_context_hash, plan_v2_startup_replay,
        recover_active_height, successor_proofs_of_possession,
    };
    use crate::{
        block::{CommittedBlock, ValidBlock},
        kura::{CommitManifest, Kura},
        query::store::LiveQueryStore,
        snapshot::AuthenticatedSnapshotBootstrapPayload,
        state::{State, World},
        sumeragi::{
            network_topology::Topology,
            v2::VerifiedHeightContext,
            v2_context_store::{PersistedHeightContext, V2ContextStore},
        },
    };

    fn verified_context() -> (VerifiedHeightContext, Vec<KeyPair>) {
        let mut keys = (1_u8..=4)
            .map(|seed| {
                KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                    .expect("deterministic BLS key")
            })
            .collect::<Vec<_>>();
        keys.sort_by(|left, right| left.public_key().cmp(right.public_key()));
        let roster = keys
            .iter()
            .map(|key| wire::ValidatorPower {
                validator: PeerId::new(key.public_key().clone()),
                power: 1,
            })
            .collect::<Vec<_>>();
        let context = wire::HeightContext {
            chain_id: ChainId::from("sumeragi-v2-recovery-test"),
            protocol_version: wire::PROTOCOL_VERSION,
            height: 1,
            epoch: 0,
            epoch_end_height: u64::MAX,
            next_epoch_snapshot: None,
            mode: wire::ConsensusMode::Permissioned,
            parent_commit_qc: None,
            snapshot_bootstrap: None,
            quorum: wire::DualQuorum::from_roster(&roster).expect("fixture quorum"),
            roster,
            nexus_amx_context_hash: Hash::new(b"recovery fixture Nexus/AMX"),
            da_layout: wire::DataAvailabilityLayout {
                encoding: wire::PayloadEncoding::Plain,
                chunk_size_bytes: 1024,
                data_shards: 0,
                parity_shards: 0,
                max_payload_size_bytes: 4096,
                max_chunk_count: 4,
            },
            leader_seed: [0x31; 32],
        };
        let proofs = keys
            .iter()
            .map(|key| {
                iroha_crypto::bls_normal_pop_prove(key.private_key())
                    .expect("BLS proof of possession")
            })
            .collect();
        (
            VerifiedHeightContext::genesis(context, proofs).expect("verified context"),
            keys,
        )
    }

    fn state_for(kura: &Arc<Kura>, chain_id: ChainId) -> State {
        State::new_with_chain_for_testing(
            World::new(),
            Arc::clone(kura),
            LiveQueryStore::start_test(),
            chain_id,
        )
    }

    fn state_with_consensus_keys(kura: &Arc<Kura>, chain_id: ChainId, keys: &[KeyPair]) -> State {
        let mut world = World::new();
        for (index, key) in keys.iter().enumerate() {
            let id = ConsensusKeyId::new(ConsensusKeyRole::Validator, format!("validator{index}"));
            let record = ConsensusKeyRecord {
                id: id.clone(),
                public_key: key.public_key().clone(),
                pop: Some(
                    iroha_crypto::bls_normal_pop_prove(key.private_key())
                        .expect("BLS proof of possession"),
                ),
                activation_height: 0,
                expiry_height: None,
                hsm: None,
                replaces: None,
                status: ConsensusKeyStatus::Active,
            };
            world.consensus_keys.insert(id.clone(), record.clone());
            world
                .consensus_keys_by_pk
                .insert(record.public_key.to_string(), vec![id]);
        }
        State::new_with_chain_for_testing(
            world,
            Arc::clone(kura),
            LiveQueryStore::start_test(),
            chain_id,
        )
    }

    fn dummy_block(
        key: &KeyPair,
        height: u64,
        parent: Option<HashOf<BlockHeader>>,
    ) -> CommittedBlock {
        dummy_block_with_time(key, height, parent, height)
    }

    fn dummy_block_with_time(
        key: &KeyPair,
        height: u64,
        parent: Option<HashOf<BlockHeader>>,
        creation_time_ms: u64,
    ) -> CommittedBlock {
        let valid = ValidBlock::new_dummy_and_modify_header(key.private_key(), |header| {
            header.set_height(NonZeroU64::new(height).expect("non-zero height"));
            header.set_prev_block_hash(parent);
            header.creation_time_ms = creation_time_ms;
            header.merkle_root = None;
        });
        valid.commit_unchecked().unpack(|_| {})
    }

    fn commit_to_state(state: &State, block: &CommittedBlock, context: &wire::HeightContext) {
        let topology = Topology::new(context.roster.iter().map(|entry| entry.validator.clone()));
        let mut state_block = state.block(block.as_ref().header());
        let _events = state_block.apply_without_execution(block, topology.as_ref().to_owned());
        state_block.commit().expect("commit synthetic state block");
    }

    fn execution_commitment(seed: u8) -> wire::ExecutionCommitment {
        wire::ExecutionCommitment::without_topups(
            Hash::new([seed, 1]),
            Hash::new([seed, 2]),
            Hash::new([seed, 3]),
            Hash::new([seed, 4]),
        )
    }

    fn authenticated_artifact_for(
        context: wire::HeightContext,
        block: &SignedBlock,
        keys: &[KeyPair],
    ) -> wire::finality::V2FinalityArtifact {
        let subject = wire::BlockSubject {
            parent_block_hash: block.header().prev_block_hash(),
            block_hash: block.hash(),
            payload_hash: block
                .canonical_proposal_wire_hash()
                .expect("canonical proposal block wire"),
        };
        let round = wire::ConsensusRound {
            context_id: context.id(),
            height: context.height,
            view: 0,
        };
        let mut exact_execution_commitment = execution_commitment(0xB6);
        exact_execution_commitment.executed_block_wire_hash = block
            .executed_block_wire_hash()
            .expect("canonical executed block wire");
        let unsigned_vote = wire::Vote {
            round,
            phase: wire::GlobalPhase::Commit,
            subject,
            execution_commitment: exact_execution_commitment,
            signer: 0,
            signature: Vec::new(),
        };
        let preimage = unsigned_vote.signature_preimage();
        let shares = keys[..3]
            .iter()
            .map(|key| {
                Signature::new(key.private_key(), &preimage)
                    .payload()
                    .to_vec()
            })
            .collect::<Vec<_>>();
        let share_refs = shares.iter().map(Vec::as_slice).collect::<Vec<_>>();
        let commit_qc = wire::QuorumCertificate {
            round,
            phase: wire::GlobalPhase::Commit,
            subject,
            execution_commitment: exact_execution_commitment,
            signers: vec![0, 1, 2],
            aggregate_signature: iroha_crypto::bls_normal_aggregate_signatures(&share_refs)
                .expect("aggregate CommitQC"),
        };
        let validator_set_pops = keys
            .iter()
            .map(|key| {
                iroha_crypto::bls_normal_pop_prove(key.private_key())
                    .expect("fixture validator PoP")
            })
            .collect();
        wire::finality::V2FinalityArtifact::new(context, subject, commit_qc, validator_set_pops)
    }

    fn persist_checkpoint_and_manifest(
        kura: &Kura,
        state: &State,
        artifact: &wire::finality::V2FinalityArtifact,
    ) {
        artifact.verify().expect("authenticated fixture artifact");
        let checkpoint = crate::snapshot::canonical_state_snapshot_hash(state);
        kura.store_wsv_checkpoint(artifact.height, artifact.block_hash, checkpoint)
            .expect("persist WSV checkpoint");
        kura.store_commit_manifest(
            CommitManifest::new(
                artifact.height,
                artifact.block_hash,
                None,
                None,
                checkpoint,
                None,
            )
            .with_authenticated_v2_commit_authority(artifact),
        )
        .expect("persist checkpoint-bound v2 commit manifest");
    }

    fn persist_complete_height(
        kura: &Kura,
        state: &State,
        artifact: &wire::finality::V2FinalityArtifact,
    ) {
        persist_checkpoint_and_manifest(kura, state, artifact);
        let _commit_receipt = kura
            .store_v2_finality_artifact(artifact)
            .expect("persist authenticated v2 finality");
    }

    fn hash_only_snapshot_boundary(
        anchor_height: u64,
        install_record: bool,
    ) -> (
        Arc<Kura>,
        State,
        wire::SnapshotV2BootstrapRecord,
        Vec<KeyPair>,
    ) {
        assert!(anchor_height > 0);
        let (genesis_context, keys) = verified_context();
        let kura = Kura::blank_kura_for_testing();
        let mut state =
            state_with_consensus_keys(&kura, genesis_context.context().chain_id.clone(), &keys);
        let mut parent = None;
        for height in 1..=anchor_height {
            let block = dummy_block(&keys[0], height, parent);
            parent = Some(block.as_ref().hash());
            commit_to_state(&state, &block, genesis_context.context());
        }
        let hashes = state.committed_block_hashes_snapshot();
        let record = snapshot_record_for_state(&state, &genesis_context, &keys, anchor_height);
        let payload =
            AuthenticatedSnapshotBootstrapPayload::for_testing(record.clone(), hashes.clone());
        kura.install_authenticated_snapshot_prefix_for_testing(&payload)
            .expect("publish authenticated hash-only snapshot tail");
        for height in 1..=anchor_height {
            let index = NonZeroUsize::new(usize::try_from(height).expect("fixture height fits"))
                .expect("fixture height is non-zero");
            assert!(kura.is_hash_only_block_height(index));
        }

        assert_eq!(
            state.commit_topology_snapshot(),
            record
                .context
                .roster
                .iter()
                .map(|entry| entry.validator.clone())
                .collect::<Vec<_>>()
        );
        if install_record {
            state.set_authenticated_snapshot_v2_bootstrap_for_testing(record.clone());
        }
        (kura, state, record, keys)
    }

    fn snapshot_record_for_state(
        state: &State,
        genesis_context: &VerifiedHeightContext,
        keys: &[KeyPair],
        anchor_height: u64,
    ) -> wire::SnapshotV2BootstrapRecord {
        let mut context = genesis_context.context().clone();
        context.height = anchor_height + 1;
        context.parent_commit_qc = None;
        context.snapshot_bootstrap = Some(wire::SnapshotBootstrapAnchor {
            snapshot_height: anchor_height,
            snapshot_block_hash: state
                .latest_block_hash_fast()
                .expect("non-empty snapshot has a tip"),
            snapshot_block_creation_time_ms: anchor_height,
            snapshot_state_hash: crate::snapshot::canonical_state_snapshot_hash(&state),
        });
        context.nexus_amx_context_hash = committed_nexus_amx_context_hash(&state);
        let record = wire::SnapshotV2BootstrapRecord {
            version: wire::SnapshotV2BootstrapRecord::VERSION,
            context,
            validator_set_pops: keys
                .iter()
                .map(|key| {
                    iroha_crypto::bls_normal_pop_prove(key.private_key())
                        .expect("fixture validator PoP")
                })
                .collect(),
        };
        VerifiedHeightContext::snapshot_bootstrap(&record)
            .expect("fixture snapshot bootstrap is valid");
        record
    }

    fn complete_first_post_snapshot_height(
        kura: &Kura,
        state: &State,
        record: &wire::SnapshotV2BootstrapRecord,
        keys: &[KeyPair],
    ) -> CommittedBlock {
        let anchor = record
            .context
            .snapshot_bootstrap
            .as_ref()
            .expect("fixture anchor");
        let block = dummy_block(
            &keys[0],
            record.context.height,
            Some(anchor.snapshot_block_hash),
        );
        kura.store_block(block.clone())
            .expect("persist first post-snapshot block");
        commit_to_state(state, &block, &record.context);
        let artifact = authenticated_artifact_for(record.context.clone(), block.as_ref(), keys);
        persist_complete_height(kura, state, &artifact);
        block
    }

    fn store_context(kura: &Kura, height: u64) -> PersistedHeightContext {
        V2ContextStore::open(kura.sumeragi_v2_storage_root())
            .expect("open context store")
            .load(height)
            .expect("read context store")
            .expect("persisted context exists")
    }

    fn model_successful_snapshot_finalization(
        kura: &Kura,
        record: &wire::SnapshotV2BootstrapRecord,
    ) {
        let verified = VerifiedHeightContext::snapshot_bootstrap(record)
            .expect("fixture snapshot bootstrap is valid");
        V2ContextStore::open(kura.sumeragi_v2_storage_root())
            .expect("open context store after authentication")
            .persist(&PersistedHeightContext::from_verified(&verified))
            .expect("publish the exact token-owned first-height context");
    }

    fn storage_tree(root: &Path) -> Vec<(PathBuf, Option<Vec<u8>>)> {
        fn visit(root: &Path, directory: &Path, entries: &mut Vec<(PathBuf, Option<Vec<u8>>)>) {
            let Ok(read_dir) = std::fs::read_dir(directory) else {
                return;
            };
            let mut paths = read_dir
                .map(|entry| entry.expect("read storage tree entry").path())
                .collect::<Vec<_>>();
            paths.sort();
            for path in paths {
                let relative = path
                    .strip_prefix(root)
                    .expect("walk remains below storage root")
                    .to_owned();
                if path.is_dir() {
                    entries.push((relative, None));
                    visit(root, &path, entries);
                } else {
                    entries.push((
                        relative,
                        Some(std::fs::read(&path).expect("read storage tree file")),
                    ));
                }
            }
        }

        let mut entries = Vec::new();
        visit(root, root, &mut entries);
        entries
    }

    fn primary_lane_blocks_dir(kura: &Kura) -> PathBuf {
        LaneConfig::default()
            .primary()
            .blocks_dir(kura.store_root())
    }

    #[test]
    fn all_hash_only_snapshot_recovers_exact_authenticated_successor() {
        let (kura, state, record, keys) = hash_only_snapshot_boundary(3, true);
        let plan = plan_v2_startup_replay(kura.as_ref()).expect("plan snapshot import");
        let authorization = authenticate_v2_snapshot_startup(kura.as_ref(), &state, &plan)
            .expect("authenticate snapshot startup")
            .expect("snapshot startup mints an authorization");
        assert_eq!(authorization.mode(), record.context.mode);
        model_successful_snapshot_finalization(kura.as_ref(), &record);

        let recovered =
            recover_active_height(kura.as_ref(), &state, None, keys[0].public_key().clone())
                .expect("authenticated all-hash-only snapshot must open its first context");
        assert_eq!(recovered.verified_context().context(), &record.context);
        assert_eq!(
            recovered.verified_context().proofs_of_possession(),
            record.validator_set_pops
        );
        assert!(recovered.pending_kura_apply().is_none());
        assert_eq!(recovered.successor_activation_parent(), Some(3));

        let store =
            V2ContextStore::open(kura.sumeragi_v2_storage_root()).expect("open context store");
        let persisted = store
            .load(record.context.height)
            .expect("load context")
            .expect("bootstrap context was persisted before ingress");
        assert_eq!(persisted.context(), &record.context);
        assert_eq!(persisted.proofs_of_possession(), record.validator_set_pops);
    }

    #[test]
    fn audited_snapshot_prefix_classifies_retained_legacy_bodies_without_sidecars() {
        let (genesis_context, keys) = verified_context();
        let kura = Kura::blank_kura_for_testing();
        let mut state =
            state_with_consensus_keys(&kura, genesis_context.context().chain_id.clone(), &keys);
        let mut parent = None;
        for height in 1..=3 {
            let block = dummy_block(&keys[0], height, parent);
            parent = Some(block.as_ref().hash());
            if height <= 2 {
                kura.store_block(block.clone())
                    .expect("retain legacy snapshot body");
            }
            commit_to_state(&state, &block, genesis_context.context());
        }
        let retained_body_path = primary_lane_blocks_dir(kura.as_ref()).join("blocks.data");
        let retained_body_bytes =
            std::fs::read(&retained_body_path).expect("read exact retained legacy body journal");
        assert!(!retained_body_bytes.is_empty());
        let record = snapshot_record_for_state(&state, &genesis_context, &keys, 3);
        let payload = AuthenticatedSnapshotBootstrapPayload::for_testing(
            record.clone(),
            state.committed_block_hashes_snapshot(),
        );
        kura.install_authenticated_snapshot_prefix_for_testing(&payload)
            .expect("publish mixed retained/hash-only audited prefix");
        state.set_authenticated_snapshot_v2_bootstrap_for_testing(record.clone());

        assert_eq!(
            std::fs::read(&retained_body_path).expect("reread retained legacy body journal"),
            retained_body_bytes,
            "typed import publication must preserve every exact retained body byte"
        );
        for height in 1..=3 {
            assert!(
                kura.get_block(NonZeroUsize::new(height).expect("non-zero height"))
                    .is_none(),
                "typed imported history is never exposed for executable replay even when exact legacy bytes remain retained"
            );
        }
        let plan = plan_v2_startup_replay(kura.as_ref())
            .expect("the complete typed import is exempt from executable sidecars");
        assert_eq!(plan.audited_bootstrap_prefix_height(), 3);
        assert_eq!(plan.complete_prefix_height(), 3);
        assert_eq!(plan.pending_tip_height(), None);

        authenticate_v2_snapshot_startup(kura.as_ref(), &state, &plan)
            .expect("authenticate mixed imported prefix")
            .expect("snapshot startup requires finalization");
        model_successful_snapshot_finalization(kura.as_ref(), &record);
        recover_active_height(kura.as_ref(), &state, None, keys[0].public_key().clone())
            .expect("retained bodies inside the typed import are historical, not executable");
    }

    #[test]
    fn untyped_zero_length_placeholder_is_never_a_replay_exemption() {
        let kura = Kura::blank_kura_for_testing();
        let hash = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0xC4; 32]));
        kura.extend_hash_only_suffix_from_verified_snapshot(&[hash])
            .expect("publish local snapshot placeholder without audited import authority");
        let height = NonZeroUsize::new(1).expect("non-zero height");
        assert!(kura.is_hash_only_block_height(height));
        assert!(!kura.is_audited_snapshot_import_height(height));
        assert!(matches!(
            plan_v2_startup_replay(kura.as_ref()),
            Err(V2StartupReplayError::InvalidReplayMetadata { height: 1, .. })
        ));
    }

    #[test]
    fn all_hash_only_snapshot_without_authenticated_record_fails_closed() {
        let (kura, state, _record, _keys) = hash_only_snapshot_boundary(2, false);
        let plan = plan_v2_startup_replay(kura.as_ref()).expect("plan snapshot import");
        let storage_root = kura.sumeragi_v2_storage_root();
        let tree_before = storage_tree(&storage_root);

        assert!(matches!(
            authenticate_v2_snapshot_startup(kura.as_ref(), &state, &plan),
            Err(V2StartupReplayError::SnapshotBootstrapAuthentication { .. })
        ));
        assert_eq!(
            storage_tree(&storage_root),
            tree_before,
            "failed token minting must leave the complete storage tree unchanged"
        );
    }

    #[test]
    fn arbitrary_self_signed_first_roster_is_rejected_before_state_or_context_mutation() {
        let (kura, state, record, _keys) = hash_only_snapshot_boundary(2, true);
        let before_height = state.committed_height();
        let before_wsv = crate::snapshot::canonical_state_snapshot_hash(&state);
        let anchor = record
            .context
            .snapshot_bootstrap
            .as_ref()
            .expect("fixture anchor");

        let mut attacker_keys = (81_u8..=84)
            .map(|seed| {
                KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                    .expect("deterministic attacker BLS key")
            })
            .collect::<Vec<_>>();
        attacker_keys.sort_by(|left, right| left.public_key().cmp(right.public_key()));
        let attacker_roster = attacker_keys
            .iter()
            .map(|key| wire::ValidatorPower {
                validator: PeerId::new(key.public_key().clone()),
                power: 1,
            })
            .collect::<Vec<_>>();
        let mut attacker_context = record.context.clone();
        attacker_context.roster = attacker_roster;
        attacker_context.quorum = wire::DualQuorum::from_roster(&attacker_context.roster)
            .expect("attacker roster is internally valid");
        let block = dummy_block(
            &attacker_keys[0],
            record.context.height,
            Some(anchor.snapshot_block_hash),
        );
        kura.store_block(block.clone())
            .expect("persist attacker first full body");
        let attacker_artifact =
            authenticated_artifact_for(attacker_context, block.as_ref(), &attacker_keys);
        persist_complete_height(kura.as_ref(), &state, &attacker_artifact);
        let plan = plan_v2_startup_replay(kura.as_ref())
            .expect("self-signed artifact is structurally complete but not snapshot-authorized");
        let storage_root = kura.sumeragi_v2_storage_root();
        let tree_before = storage_tree(&storage_root);

        assert!(matches!(
            authenticate_v2_snapshot_startup(kura.as_ref(), &state, &plan),
            Err(V2StartupReplayError::SnapshotBootstrapAuthentication { .. })
        ));
        assert_eq!(state.committed_height(), before_height);
        assert_eq!(
            crate::snapshot::canonical_state_snapshot_hash(&state),
            before_wsv
        );
        assert_eq!(
            storage_tree(&storage_root),
            tree_before,
            "attacker artifact must be rejected before any storage publication"
        );
    }

    #[test]
    fn anchor_snapshot_reopens_pending_first_full_block_without_parent_finality() {
        let (kura, state, record, keys) = hash_only_snapshot_boundary(2, true);
        let anchor = record
            .context
            .snapshot_bootstrap
            .as_ref()
            .expect("fixture anchor");
        let all_hash_only_plan =
            plan_v2_startup_replay(kura.as_ref()).expect("plan hash-only snapshot");
        authenticate_v2_snapshot_startup(kura.as_ref(), &state, &all_hash_only_plan)
            .expect("authenticate first executable context")
            .expect("snapshot startup requires finalization");
        model_successful_snapshot_finalization(kura.as_ref(), &record);

        let block = dummy_block(
            &keys[0],
            record.context.height,
            Some(anchor.snapshot_block_hash),
        );
        kura.store_block(block.clone())
            .expect("persist first post-snapshot block");

        let recovered =
            recover_active_height(kura.as_ref(), &state, None, keys[0].public_key().clone())
                .expect("anchor-height snapshot must reopen its exact pending first block");
        assert_eq!(recovered.verified_context().context(), &record.context);
        let pending = recovered
            .pending_kura_apply()
            .expect("missing finality sidecar must reopen exact Apply pipeline");
        assert_eq!(pending.height(), record.context.height);
        assert_eq!(pending.context_id(), record.context.id());
        assert_eq!(pending.block_hash(), block.as_ref().hash());
        assert_eq!(
            state.committed_height(),
            usize::try_from(anchor.snapshot_height).expect("fixture height fits usize")
        );
    }

    #[test]
    fn later_snapshot_before_first_full_finality_is_rejected_without_mutation() {
        let (kura, state, record, keys) = hash_only_snapshot_boundary(2, true);
        let anchor = record
            .context
            .snapshot_bootstrap
            .as_ref()
            .expect("fixture anchor");
        let all_hash_only_plan =
            plan_v2_startup_replay(kura.as_ref()).expect("plan hash-only snapshot");
        authenticate_v2_snapshot_startup(kura.as_ref(), &state, &all_hash_only_plan)
            .expect("authenticate first executable context")
            .expect("snapshot startup requires finalization");
        model_successful_snapshot_finalization(kura.as_ref(), &record);

        let block = dummy_block(
            &keys[0],
            record.context.height,
            Some(anchor.snapshot_block_hash),
        );
        kura.store_block(block.clone())
            .expect("persist first post-snapshot block");
        commit_to_state(&state, &block, &record.context);
        let artifact = authenticated_artifact_for(record.context.clone(), block.as_ref(), &keys);
        persist_checkpoint_and_manifest(kura.as_ref(), &state, &artifact);
        let state_hash_before = crate::snapshot::canonical_state_snapshot_hash(&state);
        let hashes_before = state.committed_block_hashes_snapshot();
        let store =
            V2ContextStore::open(kura.sumeragi_v2_storage_root()).expect("open context store");
        let context_before = store
            .load(record.context.height)
            .expect("read immutable context")
            .expect("authenticated context exists");

        assert!(matches!(
            recover_active_height(kura.as_ref(), &state, None, keys[0].public_key().clone()),
            Err(V2RecoveryError::StartupReplay(
                V2StartupReplayError::SnapshotBootstrapAuthentication { .. }
            ))
        ));
        assert_eq!(
            crate::snapshot::canonical_state_snapshot_hash(&state),
            state_hash_before,
            "rejected lineage must not mutate WSV"
        );
        assert_eq!(state.committed_block_hashes_snapshot(), hashes_before);
        assert_eq!(
            store
                .load(record.context.height)
                .expect("reload immutable context")
                .expect("context remains present"),
            context_before
        );
        assert!(
            kura.v2_finality_artifact(record.context.height)
                .expect("read finality")
                .is_none(),
            "failed startup authentication must not publish missing finality"
        );
    }

    #[test]
    fn later_snapshot_requires_retained_original_bootstrap_lineage() {
        let (kura, state, record, keys) = hash_only_snapshot_boundary(2, false);
        let verified = VerifiedHeightContext::snapshot_bootstrap(&record)
            .expect("fixture bootstrap context is valid");
        let store =
            V2ContextStore::open(kura.sumeragi_v2_storage_root()).expect("open context store");
        store
            .persist(&PersistedHeightContext::from_verified(&verified))
            .expect("persist original boundary context");
        complete_first_post_snapshot_height(kura.as_ref(), &state, &record, &keys);
        let plan = plan_v2_startup_replay(kura.as_ref()).expect("plan complete first height");

        assert!(matches!(
            authenticate_v2_snapshot_replay_boundary(kura.as_ref(), &state, &plan),
            Err(V2StartupReplayError::SnapshotBootstrapAuthentication { .. })
        ));
        assert!(matches!(
            authenticated_v2_snapshot_startup_mode(kura.as_ref(), &state, &plan),
            Err(V2StartupReplayError::SnapshotBootstrapAuthentication { .. })
        ));
    }

    #[test]
    fn later_signed_lineage_without_immutable_first_context_fails_closed_read_only() {
        let (kura, state, record, keys) = hash_only_snapshot_boundary(2, true);
        complete_first_post_snapshot_height(kura.as_ref(), &state, &record, &keys);
        assert!(
            V2ContextStore::load_from_root_read_only(
                kura.sumeragi_v2_storage_root(),
                record.context.height,
            )
            .expect("read context store")
            .is_none(),
            "fixture starts without node-local immutable context"
        );
        let plan = plan_v2_startup_replay(kura.as_ref()).expect("plan complete first height");
        let storage_root = kura.sumeragi_v2_storage_root();
        let tree_before = storage_tree(&storage_root);

        assert!(matches!(
            authenticate_v2_snapshot_startup(kura.as_ref(), &state, &plan),
            Err(V2StartupReplayError::SnapshotBootstrapAuthentication { .. })
        ));
        assert_eq!(storage_tree(&storage_root), tree_before);
        assert!(
            V2ContextStore::load_from_root_read_only(storage_root, record.context.height)
                .expect("read context store after failed authentication")
                .is_none(),
            "failed reauthentication must not publish an immutable first-height context"
        );
    }

    #[test]
    fn finalized_later_snapshot_rejects_a_missing_immutable_first_height_context() {
        let (kura, state, record, keys) = hash_only_snapshot_boundary(2, true);
        model_successful_snapshot_finalization(kura.as_ref(), &record);
        assert!(
            !kura.provisional_snapshot_bootstrap_pending(),
            "fixture must exercise the post-finalization trust boundary"
        );

        let mut parent = complete_first_post_snapshot_height(kura.as_ref(), &state, &record, &keys);
        let context_store =
            V2ContextStore::open(kura.sumeragi_v2_storage_root()).expect("open context store");
        for _ in 0..3 {
            let parent_height = parent.as_ref().header().height().get();
            let (parent_artifact, parent_receipt) = kura
                .v2_finality_artifact_with_receipt(parent_height)
                .expect("read parent finality")
                .expect("parent finality exists");
            let verified =
                build_verified_successor(&state, &context_store, &parent_artifact, &parent_receipt)
                    .expect("derive exact post-snapshot successor context");
            let context = verified.context().clone();
            let block = dummy_block(&keys[0], context.height, Some(parent.as_ref().hash()));
            kura.store_block(block.clone())
                .expect("persist later full post-snapshot block");
            commit_to_state(&state, &block, &context);
            let artifact = authenticated_artifact_for(context, block.as_ref(), &keys);
            persist_complete_height(kura.as_ref(), &state, &artifact);
            parent = block;
        }
        kura.publish_exact_commit_marker_for_tests()
            .expect("publish the exact full-height test commit marker");
        assert_eq!(
            kura.exact_durable_blocks_count()
                .expect("exact durable count"),
            usize::try_from(record.context.height + 3).expect("fixture height fits usize")
        );
        let context_path = kura
            .sumeragi_v2_storage_root()
            .join("contexts")
            .join(format!("{:020}.norito", record.context.height));
        std::fs::remove_file(&context_path)
            .expect("remove the immutable context to model post-finalization loss");
        let plan = plan_v2_startup_replay(kura.as_ref()).expect("plan complete first height");
        let state_hash_before = crate::snapshot::canonical_state_snapshot_hash(&state);
        let hashes_before = state.committed_block_hashes_snapshot();
        let storage_root = kura.store_root();
        let storage_before = storage_tree(&storage_root);

        assert!(matches!(
            authenticate_v2_snapshot_replay_boundary(kura.as_ref(), &state, &plan),
            Err(V2StartupReplayError::SnapshotBootstrapAuthentication { .. })
        ));
        assert_eq!(
            crate::snapshot::canonical_state_snapshot_hash(&state),
            state_hash_before,
            "missing immutable context rejection must not mutate WSV"
        );
        assert_eq!(state.committed_block_hashes_snapshot(), hashes_before);
        assert_eq!(
            storage_tree(&storage_root),
            storage_before,
            "post-eviction missing immutable context rejection must keep all Kura bytes read-only"
        );
    }

    #[test]
    fn later_snapshot_rejects_lineage_changed_from_immutable_first_height() {
        let (kura, mut state, record, keys) = hash_only_snapshot_boundary(2, true);
        let initial_plan =
            plan_v2_startup_replay(kura.as_ref()).expect("plan initial hash-only snapshot");
        authenticate_v2_snapshot_startup(kura.as_ref(), &state, &initial_plan)
            .expect("authenticate original boundary context")
            .expect("snapshot startup requires finalization");
        model_successful_snapshot_finalization(kura.as_ref(), &record);
        complete_first_post_snapshot_height(kura.as_ref(), &state, &record, &keys);

        let mut substituted = record.clone();
        substituted.context.leader_seed[0] ^= 0x80;
        VerifiedHeightContext::snapshot_bootstrap(&substituted)
            .expect("substituted lineage is internally self-consistent");
        state.set_authenticated_snapshot_v2_bootstrap_for_testing(substituted);
        let plan = plan_v2_startup_replay(kura.as_ref()).expect("plan complete first height");

        assert!(matches!(
            authenticate_v2_snapshot_replay_boundary(kura.as_ref(), &state, &plan),
            Err(V2StartupReplayError::SnapshotBootstrapAuthentication { .. })
        ));
        assert_eq!(
            store_context(kura.as_ref(), record.context.height).context(),
            &record.context,
            "conflicting signed lineage must not replace the immutable original"
        );
    }

    #[test]
    fn later_snapshot_uses_historical_lineage_not_current_topology_or_anchor_wsv() {
        let (kura, mut state, record, keys) = hash_only_snapshot_boundary(2, true);
        let initial_plan =
            plan_v2_startup_replay(kura.as_ref()).expect("plan initial hash-only snapshot");
        authenticate_v2_snapshot_startup(kura.as_ref(), &state, &initial_plan)
            .expect("authenticate original boundary context")
            .expect("snapshot startup requires finalization");
        model_successful_snapshot_finalization(kura.as_ref(), &record);
        complete_first_post_snapshot_height(kura.as_ref(), &state, &record, &keys);

        let changed_topology = (91_u8..=94)
            .map(|seed| {
                let key = KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                    .expect("deterministic replacement BLS key");
                PeerId::new(key.public_key().clone())
            })
            .collect::<Vec<_>>();
        {
            let mut topology = state.commit_topology.block();
            topology.clear();
            topology.extend(changed_topology.clone());
            topology.commit();
        }
        assert_ne!(
            state.commit_topology_snapshot(),
            record
                .context
                .roster
                .iter()
                .map(|entry| entry.validator.clone())
                .collect::<Vec<_>>()
        );
        assert_ne!(
            crate::snapshot::canonical_state_snapshot_hash(&state),
            record
                .context
                .snapshot_bootstrap
                .as_ref()
                .expect("fixture anchor")
                .snapshot_state_hash,
            "fixture must model a later WSV, not the original anchor WSV"
        );
        state.set_authenticated_snapshot_v2_bootstrap_for_testing(record.clone());

        let plan = plan_v2_startup_replay(kura.as_ref()).expect("plan complete first height");
        authenticate_v2_snapshot_replay_boundary(kura.as_ref(), &state, &plan)
            .expect("historical lineage is authenticated by its first full finality");
        assert_eq!(
            authenticated_v2_snapshot_startup_mode(kura.as_ref(), &state, &plan)
                .expect("derive retained signed mode"),
            Some(record.context.mode)
        );
    }

    #[test]
    fn hash_only_snapshot_rejects_an_intermediate_hash_vector_substitution() {
        let (genesis_context, keys) = verified_context();
        let kura = Kura::blank_kura_for_testing();
        let mut state =
            state_with_consensus_keys(&kura, genesis_context.context().chain_id.clone(), &keys);
        let mut parent = None;
        for height in 1..=3 {
            let block = dummy_block(&keys[0], height, parent);
            parent = Some(block.as_ref().hash());
            commit_to_state(&state, &block, genesis_context.context());
        }
        let mut substituted_hashes = state.committed_block_hashes_snapshot();
        substituted_hashes[0] = HashOf::from_untyped_unchecked(Hash::prehashed([0xE1; 32]));
        assert_eq!(
            substituted_hashes.last(),
            state.committed_block_hashes_snapshot().last(),
            "adversarial vector preserves the signed tip"
        );
        let record = snapshot_record_for_state(&state, &genesis_context, &keys, 3);
        let payload = AuthenticatedSnapshotBootstrapPayload::for_testing(
            record.clone(),
            substituted_hashes.clone(),
        );
        kura.install_authenticated_snapshot_prefix_for_testing(&payload)
            .expect("publish adversarial hash-only vector fixture");
        state.set_authenticated_snapshot_v2_bootstrap_for_testing(record.clone());
        let plan = plan_v2_startup_replay(kura.as_ref()).expect("plan hash-only snapshot");
        let storage_root = kura.sumeragi_v2_storage_root();
        let tree_before = storage_tree(&storage_root);

        assert!(matches!(
            authenticate_v2_snapshot_startup(kura.as_ref(), &state, &plan),
            Err(V2StartupReplayError::SnapshotBootstrapAuthentication { .. })
        ));
        assert_eq!(
            storage_tree(&storage_root),
            tree_before,
            "hash-vector substitution must fail before any storage publication"
        );
    }

    #[test]
    fn replay_body_preflight_rejects_a_later_unavailable_evicted_body_without_partial_state() {
        let (mut verified, keys) = verified_context();
        let kura = Kura::blank_kura_for_testing_with_blocks_in_memory(
            NonZeroUsize::new(1).expect("non-zero body retention"),
        );
        let state = state_with_consensus_keys(&kura, verified.context().chain_id.clone(), &keys);
        let store =
            V2ContextStore::open(kura.sumeragi_v2_storage_root()).expect("open context store");
        store
            .persist(&PersistedHeightContext::from_verified(&verified))
            .expect("persist height-one context");
        let mut parent = None;
        for height in 1..=4 {
            let context = verified.context().clone();
            assert_eq!(context.height, height);
            let block = dummy_block(&keys[0], height, parent);
            parent = Some(block.as_ref().hash());
            kura.store_block(block.clone())
                .expect("persist canonical replay fixture block");
            commit_to_state(&state, &block, &context);
            let artifact = authenticated_artifact_for(context, block.as_ref(), &keys);
            persist_complete_height(kura.as_ref(), &state, &artifact);
            if height < 4 {
                let (parent_artifact, parent_receipt) = kura
                    .v2_finality_artifact_with_receipt(height)
                    .expect("read parent finality")
                    .expect("parent finality exists");
                verified =
                    build_verified_successor(&state, &store, &parent_artifact, &parent_receipt)
                        .expect("derive exact successor context");
            }
        }
        let evicted_height = NonZeroUsize::new(2).expect("non-zero evicted height");
        let payload_len = kura
            .advertise_required_replicas_for_bench(evicted_height)
            .expect("height two is inline and advertizable");
        assert!(
            kura.evict_block_bodies_for_bench(payload_len)
                .expect("evict finalized historical body")
                >= payload_len
        );
        kura.remove_evicted_block_sidecar_for_testing(evicted_height)
            .expect("remove local DA cache to model remote-only unavailability");
        assert!(
            !kura.is_hash_only_block_height(evicted_height),
            "ordinary eviction retains a non-zero canonical index length"
        );
        assert!(kura.get_block(evicted_height).is_none());
        let plan = plan_v2_startup_replay(kura.as_ref())
            .expect("verified sidecars keep the evicted height finality-complete");
        assert_eq!(plan.complete_prefix_height(), 4);

        for _ in 0..3 {
            state.block_hashes.block_and_revert().commit_for_tests();
        }
        let state_hashes_before = state.committed_block_hashes_snapshot();
        let state_wsv_before = crate::snapshot::canonical_state_snapshot_hash(&state);
        assert_eq!(state.committed_height(), 1);
        assert!(
            crate::state::preflight_v2_replay_body_availability(
                kura.as_ref(),
                &state,
                2,
                plan.complete_prefix_height(),
            )
            .is_err()
        );
        assert_eq!(
            state.committed_block_hashes_snapshot(),
            state_hashes_before,
            "whole-range preflight must fail before replaying any earlier body"
        );
        assert_eq!(
            crate::snapshot::canonical_state_snapshot_hash(&state),
            state_wsv_before
        );
    }

    #[test]
    fn successor_pops_are_copied_only_from_the_durable_parent_artifact() {
        let (verified, current_keys) = verified_context();
        let current_context = verified.context().clone();
        let block = dummy_block(&current_keys[0], current_context.height, None);

        let parent =
            authenticated_artifact_for(current_context.clone(), block.as_ref(), &current_keys);
        parent.verify().expect("authenticated non-boundary parent");
        assert_eq!(
            successor_proofs_of_possession(&parent),
            parent.validator_set_pops,
            "non-boundary recovery must retain the exact historical PoP bytes"
        );

        let mut next_keys = (21_u8..=24)
            .map(|seed| {
                KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                    .expect("deterministic next-epoch BLS key")
            })
            .collect::<Vec<_>>();
        next_keys.sort_by(|left, right| left.public_key().cmp(right.public_key()));
        let next_roster = next_keys
            .iter()
            .map(|key| wire::ValidatorPower {
                validator: PeerId::new(key.public_key().clone()),
                power: 1,
            })
            .collect::<Vec<_>>();
        let next_pops = next_keys
            .iter()
            .map(|key| {
                iroha_crypto::bls_normal_pop_prove(key.private_key()).expect("valid next-epoch PoP")
            })
            .collect::<Vec<_>>();

        let mut boundary_context = current_context;
        boundary_context.epoch_end_height = boundary_context.height;
        boundary_context.next_epoch_snapshot = Some(wire::finality::FinalizedNextEpochSnapshot {
            epoch: boundary_context.epoch + 1,
            epoch_end_height: u64::MAX,
            mode: boundary_context.mode,
            quorum: wire::DualQuorum::from_roster(&next_roster).expect("valid next-epoch quorum"),
            roster: next_roster,
            validator_set_pops: next_pops.clone(),
            leader_seed: [0x73; 32],
        });
        let boundary_parent =
            authenticated_artifact_for(boundary_context, block.as_ref(), &current_keys);
        boundary_parent
            .verify()
            .expect("old roster authenticates the complete boundary snapshot");
        assert_eq!(
            successor_proofs_of_possession(&boundary_parent),
            next_pops,
            "boundary recovery must use the authenticated successor PoPs"
        );
        assert_ne!(
            successor_proofs_of_possession(&boundary_parent),
            boundary_parent.validator_set_pops,
            "next-epoch PoPs must not be reconstructed from the current roster"
        );
    }

    #[test]
    fn durable_block_before_wsv_reopens_only_its_persisted_height_context() {
        let (verified, keys) = verified_context();
        let context = verified.context().clone();
        let kura = Kura::blank_kura_for_testing();
        let state = state_for(&kura, context.chain_id.clone());
        let block = dummy_block(&keys[0], 1, None);
        kura.store_block(block.clone())
            .expect("persist canonical block");
        let store =
            V2ContextStore::open(kura.sumeragi_v2_storage_root()).expect("open context store");
        store
            .persist(&PersistedHeightContext::from_verified(&verified))
            .expect("persist active context");

        let recovered =
            recover_active_height(kura.as_ref(), &state, None, keys[0].public_key().clone())
                .expect("resume interrupted height");
        assert_eq!(recovered.verified_context().context(), &context);
        let pending = recovered
            .pending_kura_apply()
            .expect("durable tip requires replay binding");
        assert_eq!(pending.context_id(), context.id());
        assert_eq!(pending.height(), 1);
        assert_eq!(pending.block_hash(), block.as_ref().hash());
        assert_eq!(state.committed_height(), 0);
        assert_eq!(
            kura.exact_durable_blocks_count()
                .expect("read exact durable height"),
            1
        );
    }

    #[test]
    fn checkpoint_before_finality_reopens_same_height_without_reapplying() {
        let (verified, keys) = verified_context();
        let context = verified.context().clone();
        let kura = Kura::blank_kura_for_testing();
        let state = state_for(&kura, context.chain_id.clone());
        let block = dummy_block(&keys[0], 1, None);
        kura.store_block(block.clone())
            .expect("persist canonical block");
        commit_to_state(&state, &block, &context);
        let checkpoint = crate::snapshot::canonical_state_snapshot_hash(&state);
        kura.store_wsv_checkpoint(1, block.as_ref().hash(), checkpoint)
            .expect("persist interrupted post-WSV checkpoint");
        let store =
            V2ContextStore::open(kura.sumeragi_v2_storage_root()).expect("open context store");
        store
            .persist(&PersistedHeightContext::from_verified(&verified))
            .expect("persist active context");

        let recovered =
            recover_active_height(kura.as_ref(), &state, None, keys[0].public_key().clone())
                .expect("resume finality sidecar window");
        assert_eq!(recovered.verified_context().context(), &context);
        assert_eq!(
            recovered
                .pending_kura_apply()
                .expect("missing finality requires replay binding")
                .block_hash(),
            block.as_ref().hash()
        );
        assert_eq!(state.committed_height(), 1);
        assert!(
            kura.v2_finality_artifact(1)
                .expect("read finality")
                .is_none()
        );
    }

    #[test]
    fn applied_tip_without_persisted_checkpoint_fails_closed() {
        let (verified, keys) = verified_context();
        let context = verified.context().clone();
        let kura = Kura::blank_kura_for_testing();
        let state = state_for(&kura, context.chain_id.clone());
        let block = dummy_block(&keys[0], 1, None);
        kura.store_block(block.clone())
            .expect("persist canonical block");
        commit_to_state(&state, &block, &context);
        let store =
            V2ContextStore::open(kura.sumeragi_v2_storage_root()).expect("open context store");
        store
            .persist(&PersistedHeightContext::from_verified(&verified))
            .expect("persist active context");

        assert!(matches!(
            recover_active_height(kura.as_ref(), &state, None, keys[0].public_key().clone()),
            Err(V2RecoveryError::AppliedPendingTipWithoutCheckpoint(1))
        ));
    }

    #[test]
    fn finality_without_checkpoint_and_manifest_fails_closed() {
        let (verified, keys) = verified_context();
        let context = verified.context().clone();
        let kura = Kura::blank_kura_for_testing();
        let state = state_for(&kura, context.chain_id.clone());
        let block = dummy_block(&keys[0], 1, None);
        kura.store_block(block.clone())
            .expect("persist canonical block");
        let artifact = authenticated_artifact_for(context.clone(), block.as_ref(), &keys);
        let _receipt = kura
            .store_v2_finality_artifact(&artifact)
            .expect("persist finality");

        assert!(matches!(
            recover_active_height(kura.as_ref(), &state, None, keys[0].public_key().clone(),),
            Err(V2RecoveryError::StartupReplay(
                V2StartupReplayError::InvalidReplayMetadata { height: 1, .. }
            ))
        ));
    }

    #[test]
    fn parent_finality_and_immutable_context_mismatch_fails_closed() {
        let (verified, keys) = verified_context();
        let context = verified.context().clone();
        let kura = Kura::blank_kura_for_testing();
        let state = state_for(&kura, context.chain_id.clone());
        let block = dummy_block(&keys[0], 1, None);
        kura.store_block(block.clone())
            .expect("persist canonical block");
        commit_to_state(&state, &block, &context);
        let artifact = authenticated_artifact_for(context.clone(), block.as_ref(), &keys);
        persist_complete_height(kura.as_ref(), &state, &artifact);

        let mut different = context;
        different.leader_seed[0] ^= 0x80;
        let proofs = keys
            .iter()
            .map(|key| {
                iroha_crypto::bls_normal_pop_prove(key.private_key())
                    .expect("BLS proof of possession")
            })
            .collect();
        let different = VerifiedHeightContext::genesis(different, proofs)
            .expect("different context is independently valid");
        let store =
            V2ContextStore::open(kura.sumeragi_v2_storage_root()).expect("open context store");
        store
            .persist(&PersistedHeightContext::from_verified(&different))
            .expect("persist mismatching context");

        assert!(matches!(
            recover_active_height(kura.as_ref(), &state, None, keys[0].public_key().clone(),),
            Err(V2RecoveryError::ParentContextMismatch(1))
        ));
    }

    #[test]
    fn missing_context_for_interrupted_durable_block_fails_closed() {
        let (verified, keys) = verified_context();
        let context = verified.context().clone();
        let kura = Kura::blank_kura_for_testing();
        let state = state_for(&kura, context.chain_id);
        kura.store_block(dummy_block(&keys[0], 1, None))
            .expect("persist canonical block");

        assert!(matches!(
            recover_active_height(kura.as_ref(), &state, None, keys[0].public_key().clone(),),
            Err(V2RecoveryError::MissingActiveContext(1))
        ));
    }

    #[test]
    fn equal_wsv_and_kura_heights_with_different_hashes_fail_closed() {
        let (verified, keys) = verified_context();
        let context = verified.context().clone();
        let kura = Kura::blank_kura_for_testing();
        let state = state_for(&kura, context.chain_id);
        let state_block = dummy_block_with_time(&keys[0], 1, None, 1);
        let kura_block = dummy_block_with_time(&keys[0], 1, None, 2);
        assert_ne!(state_block.as_ref().hash(), kura_block.as_ref().hash());
        commit_to_state(&state, &state_block, verified.context());
        kura.store_block(kura_block)
            .expect("persist conflicting Kura tip");

        assert!(matches!(
            recover_active_height(kura.as_ref(), &state, None, keys[0].public_key().clone(),),
            Err(V2RecoveryError::StateKuraHashMismatch { height: 1, .. })
        ));
    }

    #[test]
    fn startup_plan_never_generic_replays_a_kura_first_tip() {
        let (verified, keys) = verified_context();
        let context = verified.context().clone();
        let kura = Kura::blank_kura_for_testing();
        let state = state_for(&kura, context.chain_id.clone());
        let first = dummy_block(&keys[0], 1, None);
        kura.store_block(first.clone())
            .expect("persist first canonical block");
        commit_to_state(&state, &first, &context);
        let artifact = authenticated_artifact_for(context, first.as_ref(), &keys);
        persist_complete_height(kura.as_ref(), &state, &artifact);

        let second = dummy_block(&keys[0], 2, Some(first.as_ref().hash()));
        kura.store_block(second)
            .expect("persist Kura-first successor tip");

        let plan = plan_v2_startup_replay(kura.as_ref()).expect("classify exact pending tip");
        assert_eq!(plan.durable_height(), 2);
        assert_eq!(plan.complete_prefix_height(), 1);
        assert_eq!(plan.pending_tip_height(), Some(2));
        plan.validate_restored_state_height(0)
            .expect("empty state can replay complete prefix");
        plan.validate_restored_state_height(1)
            .expect("complete prefix state is valid");
        plan.validate_restored_state_height(2)
            .expect("checkpointed snapshot may already contain the sole tip");
        assert!(matches!(
            plan.validate_restored_state_height(3),
            Err(V2StartupReplayError::StateHeightOutsidePlan { .. })
        ));
    }

    #[test]
    fn startup_plan_propagates_a_corrupt_exact_durable_index_count() {
        let (_verified, keys) = verified_context();
        let kura = Kura::blank_kura_for_testing();
        kura.store_block(dummy_block(&keys[0], 1, None))
            .expect("persist canonical block");
        let index_path = primary_lane_blocks_dir(kura.as_ref()).join("blocks.index");
        let mut index = std::fs::OpenOptions::new()
            .append(true)
            .open(&index_path)
            .expect("open durable index for adversarial corruption");
        index
            .write_all(&[0xA5])
            .expect("append a partial index entry");
        index.sync_all().expect("sync corrupt durable index");

        assert!(matches!(
            plan_v2_startup_replay(kura.as_ref()),
            Err(V2StartupReplayError::Kura(_))
        ));
    }

    #[test]
    fn startup_plan_rejects_an_incomplete_interior_height() {
        let (verified, keys) = verified_context();
        let context = verified.context().clone();
        let kura = Kura::blank_kura_for_testing();
        let state = state_for(&kura, context.chain_id);
        let first = dummy_block(&keys[0], 1, None);
        kura.store_block(first.clone())
            .expect("persist first canonical block");
        commit_to_state(&state, &first, verified.context());
        let artifact =
            authenticated_artifact_for(verified.context().clone(), first.as_ref(), &keys);
        // Model a crash after manifest publication but before finality, followed by an
        // impossible later durable block. The gap is interior and must never be treated as a
        // multi-height recovery suffix.
        persist_checkpoint_and_manifest(kura.as_ref(), &state, &artifact);
        kura.store_block(dummy_block(&keys[0], 2, Some(first.as_ref().hash())))
            .expect("persist impossible later block");

        assert!(matches!(
            plan_v2_startup_replay(kura.as_ref()),
            Err(V2StartupReplayError::IncompleteInteriorHeight {
                height: 1,
                durable_height: 2,
            })
        ));
    }

    #[test]
    fn startup_plan_accepts_each_post_checkpoint_crash_window_as_one_tip() {
        let (verified, keys) = verified_context();
        let context = verified.context().clone();
        let kura = Kura::blank_kura_for_testing();
        let state = state_for(&kura, context.chain_id);
        let block = dummy_block(&keys[0], 1, None);
        kura.store_block(block.clone())
            .expect("persist canonical block");
        commit_to_state(&state, &block, verified.context());
        let artifact =
            authenticated_artifact_for(verified.context().clone(), block.as_ref(), &keys);
        let checkpoint = crate::snapshot::canonical_state_snapshot_hash(&state);
        kura.store_wsv_checkpoint(1, block.as_ref().hash(), checkpoint)
            .expect("persist checkpoint-only crash image");
        let checkpoint_only =
            plan_v2_startup_replay(kura.as_ref()).expect("checkpoint-only tip is recoverable");
        assert_eq!(checkpoint_only.complete_prefix_height(), 0);
        assert_eq!(checkpoint_only.pending_tip_height(), Some(1));

        kura.store_commit_manifest(
            CommitManifest::new(1, block.as_ref().hash(), None, None, checkpoint, None)
                .with_authenticated_v2_commit_authority(&artifact),
        )
        .expect("persist manifest-only-before-finality crash image");
        let manifest_only =
            plan_v2_startup_replay(kura.as_ref()).expect("manifest tip is recoverable");
        assert_eq!(manifest_only.complete_prefix_height(), 0);
        assert_eq!(manifest_only.pending_tip_height(), Some(1));

        let _commit_receipt = kura
            .store_v2_finality_artifact(&artifact)
            .expect("complete finality publication");
        let complete = plan_v2_startup_replay(kura.as_ref()).expect("complete tuple is replayable");
        assert_eq!(complete.complete_prefix_height(), 1);
        assert_eq!(complete.pending_tip_height(), None);
    }

    #[test]
    fn deferred_sidecar_recovery_requires_a_fresh_plan_and_snapshot_boundary_authentication() {
        let (kura, state, record, keys) = hash_only_snapshot_boundary(2, true);
        let anchor = record
            .context
            .snapshot_bootstrap
            .as_ref()
            .expect("fixture snapshot anchor");
        let first_full = dummy_block(
            &keys[0],
            record.context.height,
            Some(anchor.snapshot_block_hash),
        );
        kura.store_block(first_full.clone())
            .expect("persist interrupted first post-snapshot block");

        let prefinalization_plan =
            plan_v2_startup_replay(kura.as_ref()).expect("classify pre-finalization crash image");
        assert_eq!(prefinalization_plan.complete_prefix_height(), 2);
        assert_eq!(prefinalization_plan.pending_tip_height(), Some(3));
        let authorization =
            authenticate_v2_snapshot_startup(kura.as_ref(), &state, &prefinalization_plan)
                .expect("authenticate original snapshot boundary")
                .expect("imported prefix mints a finalization authorization");

        // Model deferred stage recovery publishing a complete, internally valid sidecar tuple
        // after the token was minted. The recovered artifact preserves the snapshot anchor, so
        // replay planning alone accepts it, but substitutes another frozen first-height context.
        // Startup must discard the old plan and authenticate the recovered tuple against the
        // original signed snapshot before replay.
        let mut substituted_context = record.context.clone();
        substituted_context.leader_seed[0] ^= 0x80;
        let substituted_artifact =
            authenticated_artifact_for(substituted_context, first_full.as_ref(), &keys);
        persist_complete_height(kura.as_ref(), &state, &substituted_artifact);

        let recovered_plan =
            plan_v2_startup_replay(kura.as_ref()).expect("reclassify recovered sidecar tuple");
        assert_eq!(recovered_plan.complete_prefix_height(), 3);
        assert_eq!(recovered_plan.pending_tip_height(), None);
        assert_ne!(
            recovered_plan, prefinalization_plan,
            "deferred recovery changed the executable replay boundary"
        );
        assert!(matches!(
            authenticate_v2_snapshot_replay_boundary(kura.as_ref(), &state, &recovered_plan),
            Err(V2StartupReplayError::SnapshotBootstrapAuthentication { .. })
        ));
        drop(authorization);
    }

    #[test]
    fn startup_plan_rejects_finality_bound_to_an_unauthenticated_manifest() {
        let (verified, keys) = verified_context();
        let context = verified.context().clone();
        let kura = Kura::blank_kura_for_testing();
        let state = state_for(&kura, context.chain_id);
        let block = dummy_block(&keys[0], 1, None);
        kura.store_block(block.clone())
            .expect("persist canonical block");
        commit_to_state(&state, &block, verified.context());
        let artifact =
            authenticated_artifact_for(verified.context().clone(), block.as_ref(), &keys);
        let checkpoint = crate::snapshot::canonical_state_snapshot_hash(&state);
        kura.store_wsv_checkpoint(1, block.as_ref().hash(), checkpoint)
            .expect("persist WSV checkpoint");
        kura.store_commit_manifest(CommitManifest::new(
            1,
            block.as_ref().hash(),
            None,
            None,
            checkpoint,
            None,
        ))
        .expect("persist checkpoint-bound but authority-free manifest");
        let _commit_receipt = kura
            .store_v2_finality_artifact(&artifact)
            .expect("persist independently authenticated finality");

        assert!(matches!(
            plan_v2_startup_replay(kura.as_ref()),
            Err(V2StartupReplayError::InvalidReplayMetadata { height: 1, .. })
        ));
    }

    #[test]
    fn finalized_tip_derives_one_idempotent_successor_context() {
        let (verified, keys) = verified_context();
        let context = verified.context().clone();
        let kura = Kura::blank_kura_for_testing();
        let state = state_with_consensus_keys(&kura, context.chain_id.clone(), &keys);
        let block = dummy_block(&keys[0], 1, None);
        kura.store_block(block.clone())
            .expect("persist canonical block");
        commit_to_state(&state, &block, &context);
        let artifact = authenticated_artifact_for(context.clone(), block.as_ref(), &keys);
        persist_complete_height(kura.as_ref(), &state, &artifact);
        let store =
            V2ContextStore::open(kura.sumeragi_v2_storage_root()).expect("open context store");
        store
            .persist(&PersistedHeightContext::from_verified(&verified))
            .expect("persist parent context");

        let first =
            recover_active_height(kura.as_ref(), &state, None, keys[0].public_key().clone())
                .expect("derive successor");
        assert_eq!(first.verified_context().context().height, 2);
        assert_eq!(
            first.verified_context().context().parent_commit_qc,
            Some(artifact.commit_qc.clone())
        );
        assert!(first.pending_kura_apply().is_none());
        assert_eq!(first.successor_activation_parent(), Some(1));
        let first_context = first.verified_context().context().clone();
        drop(first);

        let repeated =
            recover_active_height(kura.as_ref(), &state, None, keys[0].public_key().clone())
                .expect("reopen identical successor");
        assert_eq!(repeated.verified_context().context(), &first_context);
        assert!(repeated.pending_kura_apply().is_none());
        assert_eq!(repeated.successor_activation_parent(), Some(1));
    }
}
