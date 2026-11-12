//! Operator routes for the boundaries of the universal root's history
//! (`specs/runtime_upgrades.md`, "Checkpoint records"; `specs/live_runtime_upgrades.md` §7.3,
//! §13): what the root committed, and this node's signed statements of the adoption anchor and
//! of a startup floor.
//!
//! A node signs a statement only after deriving the record from, or checking it against, its own
//! certified history (`iroha_core::runtime::checkpoint`). The statements are the authority of a
//! record, so the operator collects one from every validator and any account relays them in
//! `RatifyRuntimeAdoptionAnchorV1` or `RaiseRuntimeStartupFloorV1`. The validator key never
//! leaves the node.

use std::sync::Arc;

use iroha_core::{
    kura::Kura,
    runtime::checkpoint::{
        self, RecordAttestationError, adoption_record, attest_anchor, attest_floor,
    },
    snapshot::checkpoints::SnapshotCheckpoints,
    state::{StateReadOnly, WorldReadOnly, upgrade_checkpoint::UpgradeCheckpointV1},
    sumeragi::certified_chain::CertifiedChain,
};
use iroha_crypto::{HashOf, KeyPair, PublicKey};
use iroha_data_model::{
    block::BlockHeader,
    parameter::RuntimeCheckpointPolicyV1,
    runtime::{
        adoption::SignedAdoptionAttestationV1,
        checkpoint::{
            RuntimeAdoptionAnchorV1, RuntimeBoundaryV1, RuntimeStartupFloorV1,
            SignedStartupFloorAttestationV1, StartupFloorV1,
        },
        lifecycle::AdoptionRecordV1,
    },
};
use mv::storage::StorageReadOnly as _;
use norito::derive::{NoritoDeserialize, NoritoSerialize};

use crate::{
    NoritoJson,
    json_macros::{JsonDeserialize, JsonSerialize},
};

fn refused(message: impl Into<String>) -> crate::Error {
    crate::Error::Query(iroha_data_model::ValidationFail::QueryFailed(
        iroha_data_model::query::error::QueryExecutionFail::Conversion(message.into()),
    ))
}

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    JsonSerialize,
    JsonDeserialize,
    NoritoSerialize,
    NoritoDeserialize,
    norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_torii::runtime_boundaries::RuntimeBoundaryBlock")]
/// One height of the root's history and the hash of the block committed at it.
pub struct RuntimeBoundaryBlock {
    /// Block height.
    pub height: u64,
    /// Hash of the block at that height, 32-byte lowercase hex.
    pub block_hash_hex: String,
}

impl RuntimeBoundaryBlock {
    fn at(height: u64, block_hash: &HashOf<BlockHeader>) -> Self {
        let hash: &[u8; 32] = block_hash.as_ref();
        Self {
            height,
            block_hash_hex: hex::encode(hash),
        }
    }

    fn of(checkpoint: &UpgradeCheckpointV1) -> Self {
        Self::at(checkpoint.height, &checkpoint.block_hash)
    }
}

#[derive(
    Debug, JsonSerialize, JsonDeserialize, NoritoSerialize, NoritoDeserialize, norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_torii::runtime_boundaries::RuntimeBoundariesResponse")]
/// What the universal root committed about the boundaries of its history, and what this node
/// holds.
pub struct RuntimeBoundariesResponse {
    /// Whether the root has activated the kernel's lifecycle State
    /// (`iroha.runtime.lifecycle.v1`), without which it commits no boundary.
    pub lifecycle_installed: bool,
    /// The committed checkpoint policy, if the root has one it reads.
    pub policy: Option<RuntimeCheckpointPolicyV1>,
    /// The ratified adoption anchor.
    pub anchor: Option<RuntimeAdoptionAnchorV1>,
    /// The committed startup floors by height: the floor in force and any floor waiting out its
    /// notice.
    pub floors: Vec<RuntimeStartupFloorV1>,
    /// The height at or below which no release re-executes a block, at the committed tip's
    /// time.
    pub history_boundary: Option<RuntimeBoundaryBlock>,
    /// The upgrade checkpoint this node's block store records.
    pub node_checkpoint: Option<RuntimeBoundaryBlock>,
    /// The checkpoints this node holds, by height; empty without a snapshot store.
    pub held_checkpoints: Vec<RuntimeBoundaryBlock>,
}

/// GET /v1/runtime/boundaries — what the root committed about the boundaries of its history:
/// whether it has activated the lifecycle State, its checkpoint policy, its ratified adoption
/// anchor and startup floors, and the history boundary they set; and the upgrade checkpoint and
/// the checkpoints this node holds.
///
/// # Errors
/// The node's snapshot store cannot be read.
pub fn handle_runtime_boundaries(
    state: &iroha_core::state::State,
    kura: &Kura,
    checkpoints: Option<&SnapshotCheckpoints>,
) -> Result<RuntimeBoundariesResponse, crate::Error> {
    let view = state.view();
    let world = view.world();
    let selection = world.runtime_selection();
    let now_ms = state.latest_block_creation_time_ms_fast().unwrap_or(0);
    let floors = world
        .runtime_boundaries()
        .iter()
        .filter_map(|(_, boundary)| match boundary {
            RuntimeBoundaryV1::StartupFloor(floor) => Some(floor.clone()),
            RuntimeBoundaryV1::AdoptionAnchor(_) => None,
        })
        .collect();
    let held_checkpoints = match checkpoints {
        Some(checkpoints) => checkpoints
            .held(view.chain_id(), *view.network_id())
            .map_err(|error| refused(format!("snapshot store: {error}")))?
            .into_iter()
            .map(|held| RuntimeBoundaryBlock::at(held.height, &held.block_hash))
            .collect(),
        None => Vec::new(),
    };
    Ok(RuntimeBoundariesResponse {
        lifecycle_installed: iroha_core::runtime::migration::lifecycle_installed(selection),
        policy: checkpoint::committed_policy(selection, WorldReadOnly::parameters(world)),
        anchor: checkpoint::committed_anchor(world),
        floors,
        history_boundary: checkpoint::history_boundary(world, now_ms)
            .as_ref()
            .map(RuntimeBoundaryBlock::of),
        node_checkpoint: kura.upgrade_checkpoint().map(RuntimeBoundaryBlock::of),
        held_checkpoints,
    })
}

#[derive(
    Debug, JsonSerialize, JsonDeserialize, NoritoSerialize, NoritoDeserialize, norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_torii::runtime_boundaries::RuntimeAnchorStatementResponse")]
/// This node's adoption record and its signed statement of it.
pub struct RuntimeAnchorStatementResponse {
    /// The adoption record this node derived from its own store.
    pub record: AdoptionRecordV1,
    /// This node's statement of the record, signed with its validator key.
    pub attestation: SignedAdoptionAttestationV1,
}

/// The current committee: the validators every boundary needs a statement from.
fn current_committee(view: &impl StateReadOnly) -> Vec<PublicKey> {
    view.commit_topology()
        .iter()
        .map(|peer| peer.public_key().clone())
        .collect()
}

fn statement_refused(error: &RecordAttestationError) -> crate::Error {
    refused(format!("this node does not sign: {error}"))
}

/// POST /v1/runtime/boundaries/anchor/attest — this node derives the adoption record from its
/// own store (the upgrade checkpoint it was configured to adopt the foundation at, the certified
/// World roots around it, the committee of its height and the founded selection) and signs its
/// statement of it (`specs/live_runtime_upgrades.md` §13).
///
/// Every validator derives the same record only if the stores agree, so the statements a
/// ratification collects name one record.
///
/// # Errors
/// The node holds no validator key, records no upgrade checkpoint, its key is not a member of the
/// current committee, or its history does not certify the record.
pub fn handle_runtime_attest_anchor(
    state: &iroha_core::state::State,
    kura: &Kura,
    signer: Option<&KeyPair>,
) -> Result<RuntimeAnchorStatementResponse, crate::Error> {
    let signer =
        signer.ok_or_else(|| refused("this node holds no runtime attestation key".to_owned()))?;
    let view = state.view();
    let network = *view.network_id();
    let configured = kura.upgrade_checkpoint();
    let history =
        CertifiedChain::new(&view).map_err(|error| refused(format!("certified chain: {error}")))?;
    let record = adoption_record(&history, &network, configured)
        .map_err(|error| statement_refused(&error))?;
    let attestation = attest_anchor(
        &history,
        &network,
        configured,
        &current_committee(&view),
        &record,
        signer,
    )
    .map_err(|error| statement_refused(&error))?;
    Ok(RuntimeAnchorStatementResponse {
        record,
        attestation,
    })
}

#[derive(
    Debug,
    Clone,
    Copy,
    JsonSerialize,
    JsonDeserialize,
    NoritoSerialize,
    NoritoDeserialize,
    norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_torii::runtime_boundaries::RuntimeFloorAttestRequest")]
/// Body of POST /v1/runtime/boundaries/floor/attest: the checkpoint height to vouch for as the
/// startup floor.
pub struct RuntimeFloorAttestRequest {
    /// Height of the checkpoint.
    pub height: u64,
}

#[derive(
    Debug, JsonSerialize, JsonDeserialize, NoritoSerialize, NoritoDeserialize, norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_torii::runtime_boundaries::RuntimeFloorStatementResponse")]
/// The startup floor this node states from its own history, and its signed statement of it.
pub struct RuntimeFloorStatementResponse {
    /// The floor: this network, the height, and the block and committed time the node stores
    /// there.
    pub floor: StartupFloorV1,
    /// This node's statement of the floor, signed with its validator key.
    pub attestation: SignedStartupFloorAttestationV1,
}

/// POST /v1/runtime/boundaries/floor/attest — this node states the startup floor at the height
/// in the body from its own history (the block and committed time it stores there) and signs its
/// statement, only while it holds a checkpoint at exactly that height
/// (`specs/live_runtime_upgrades.md` §7.3, "Advancing the floor").
///
/// # Errors
/// The node holds no validator key, its key is not a member of the current committee, it stores
/// no certified block at the height, or it holds no checkpoint there.
pub fn handle_runtime_attest_floor(
    state: &iroha_core::state::State,
    signer: Option<&KeyPair>,
    checkpoints: Option<&SnapshotCheckpoints>,
    NoritoJson(request): NoritoJson<RuntimeFloorAttestRequest>,
) -> Result<RuntimeFloorStatementResponse, crate::Error> {
    use iroha_core::runtime::checkpoint::History as _;
    let signer =
        signer.ok_or_else(|| refused("this node holds no runtime attestation key".to_owned()))?;
    let view = state.view();
    let network = *view.network_id();
    let history =
        CertifiedChain::new(&view).map_err(|error| refused(format!("certified chain: {error}")))?;
    let (Some(block_hash), Some(committed_ms)) = (
        history.block_hash(request.height),
        history.committed_ms(request.height),
    ) else {
        return Err(refused(format!(
            "this node stores no certified block at height {}",
            request.height
        )));
    };
    let floor = StartupFloorV1 {
        network,
        height: request.height,
        block_hash,
        committed_ms,
    };
    let holds = checkpoints.is_some_and(|checkpoints| {
        checkpoints.holds(view.chain_id(), network, request.height, &block_hash)
    });
    let attestation = attest_floor(
        &history,
        &network,
        &current_committee(&view),
        holds,
        &floor,
        signer,
    )
    .map_err(|error| statement_refused(&error))?;
    Ok(RuntimeFloorStatementResponse { floor, attestation })
}

/// The node's snapshot store, shared by the boundary routes.
pub type SharedSnapshotCheckpoints = Option<Arc<SnapshotCheckpoints>>;

#[cfg(test)]
mod tests {
    use iroha_core::{
        kura::Kura,
        query::store::LiveQueryStore,
        state::{State, World},
    };
    use iroha_crypto::Algorithm;

    use super::*;

    fn blank() -> (State, Arc<Kura>) {
        let kura = Kura::blank_kura_for_testing();
        let state = State::new_for_testing(
            World::default(),
            Arc::clone(&kura),
            LiveQueryStore::start_test(),
        );
        (state, kura)
    }

    #[test]
    fn a_root_without_the_lifecycle_state_reports_no_boundary() {
        let (state, kura) = blank();
        let response = handle_runtime_boundaries(&state, &kura, None).expect("boundaries");
        assert!(!response.lifecycle_installed);
        assert_eq!(response.policy, None);
        assert_eq!(response.anchor, None);
        assert!(response.floors.is_empty());
        assert_eq!(response.history_boundary, None);
        assert_eq!(response.node_checkpoint, None);
        assert!(response.held_checkpoints.is_empty());
    }

    #[test]
    fn a_node_signs_nothing_without_its_key_or_its_certified_history() {
        let (state, kura) = blank();
        let key = KeyPair::from_seed(vec![1; 32], Algorithm::Ed25519);
        assert!(handle_runtime_attest_anchor(&state, &kura, None).is_err());
        // A blank store certifies no block, so the node derives no adoption record.
        assert!(handle_runtime_attest_anchor(&state, &kura, Some(&key)).is_err());
        let request = || NoritoJson(RuntimeFloorAttestRequest { height: 1 });
        assert!(handle_runtime_attest_floor(&state, None, None, request()).is_err());
        assert!(handle_runtime_attest_floor(&state, Some(&key), None, request()).is_err());
    }
}
