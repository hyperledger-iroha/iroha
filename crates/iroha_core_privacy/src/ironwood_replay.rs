//! Operator-local Ironwood preparation from a certified complete World and its typed originals.
//! The full World contains sensitive row hashes. Only the verified pool and two selected
//! balances cross the operator's custody boundary.

use crate::ironwood_state_rows::validate_pool_state;
use crate::privacy_state::{
    PrivacyCommitmentKeyV1, PrivacyOrchardPoolStateV1, PrivacyRootHeadKeyV1,
    PrivacyRootHeadRecordV1, PrivacyRootKeyV1, PrivacyStateItemRecordV1,
};
use iroha_crypto::Hash;
use iroha_data_model::{
    NetworkId,
    account::AccountId,
    asset::{AssetBalanceScope, AssetDefinitionId, AssetId, AssetValue},
    block::consensus::SumeragiRootScope,
    isi::privacy::SubmitPrivacyIronwoodProofV1,
    privacy::{PrivacyPoolIdV1, PrivacyRootRoleV1},
    runtime::definition::{RootSelectionV1, SelectionId},
    sumeragi_finality::{
        FinalityError, FinalityReadError, VerifiedSumeragiBlock, WorldStateSnapshotV1,
        ironwood_world_schema_v1, lthash16_world_profile_v1, world_state_selection_schema_v1,
    },
};

/// Maximum canonical operator-local frame for selection, typed pool records and balances.
pub const MAX_IRONWOOD_REPLAY_ORIGINALS_BYTES_V1: usize = 4 * 1024 * 1024;

/// Unauthenticated typed preimages held by the replay operator for one Global cut.
/// The separately retained World frame must remain in the same custody boundary.
#[derive(Clone, Debug, norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core_privacy::ironwood_replay::IronwoodReplayOriginalsV1")]
pub struct IronwoodReplayOriginalsV1 {
    /// Exact native runtime selection cell at the cut.
    pub selection: RootSelectionV1,
    /// Compact frontier and governed bootstrap of the selected pool.
    pub state: PrivacyStateItemRecordV1,
    /// Current retained anchor head, including its root-history provenance preimage.
    pub head: PrivacyRootHeadRecordV1,
    /// Selected payer's Global asset row or explicit absence.
    pub payer_balance: Option<AssetValue>,
    /// Governed reserve's Global asset row or explicit absence.
    pub reserve_balance: Option<AssetValue>,
}

/// Verified typed preparation inputs. Construction consumes bounded originals and authenticates
/// each typed preimage against the exact same independently certified decision.
pub struct VerifiedIronwoodReplayContextV1 {
    state: PrivacyOrchardPoolStateV1,
    payer_balance: Option<AssetValue>,
    reserve_balance: Option<AssetValue>,
}

impl VerifiedIronwoodReplayContextV1 {
    /// Verified compact frontier, reserve, asset, epoch and anchor.
    #[must_use]
    pub fn state(&self) -> &PrivacyOrchardPoolStateV1 {
        &self.state
    }
    /// Payer's certified Global balance. Verified absence means zero.
    #[must_use]
    pub fn payer_balance(&self) -> &Option<AssetValue> {
        &self.payer_balance
    }
    /// Reserve's certified Global balance. Verified absence means zero.
    #[must_use]
    pub fn reserve_balance(&self) -> &Option<AssetValue> {
        &self.reserve_balance
    }
}

fn invalid() -> FinalityError {
    FinalityError(
        "Ironwood World replay differs from the qualified selection, cut or typed originals".into(),
    )
}

/// Verify two operator-owned originals at the independently admitted Global successor.
/// Supply the selection from the reviewed governed change and the schema from the compiled release.
/// The caller owns the trusted genesis/checkpoint, intended height, privacy proof profile,
/// and any freshness policy. The World and DTO are never returned or retained.
/// # Errors
/// Foreign scope, schema, profile or selection, missing or changed preimages, or original
/// canonical decoder refusal. A missing or oversized local artifact grants no ledger verdict.
#[allow(clippy::too_many_arguments)]
pub fn verify_world_replay_context_v1(
    block: &VerifiedSumeragiBlock,
    world_frame: &[u8],
    originals_frame: &[u8],
    network: NetworkId,
    selection: SelectionId,
    schema: Hash,
    pool: PrivacyPoolIdV1,
    asset: &AssetDefinitionId,
    payer: &AccountId,
) -> Result<VerifiedIronwoodReplayContextV1, FinalityReadError> {
    if block.height() < 2
        || block.root_scope() != SumeragiRootScope::Global
        || block.commitment().schedule.current.network_id != network
        || pool.is_zero()
        || schema != ironwood_world_schema_v1()
        || originals_frame.is_empty()
        || originals_frame.len() > MAX_IRONWOOD_REPLAY_ORIGINALS_BYTES_V1
    {
        return Err(invalid().into());
    }
    let originals: IronwoodReplayOriginalsV1 = norito::decode_canonical_for_admission(
        originals_frame,
        norito::canonical_decode_limits(originals_frame.len()),
    )?;
    if originals.selection.commitment_profile != lthash16_world_profile_v1()
        || originals.selection.id().map_err(|_| invalid())? != selection
        || world_state_selection_schema_v1(&originals.selection)? != schema
        || originals
            .selection
            .bindings
            .binary_search_by(|b| {
                b.wire_id
                    .as_str()
                    .cmp(SubmitPrivacyIronwoodProofV1::WIRE_ID)
            })
            .is_err()
    {
        return Err(invalid().into());
    }
    let snapshot = WorldStateSnapshotV1::decode_bounded_canonical(world_frame)?;
    if snapshot.schema_hash != schema {
        return Err(invalid().into());
    }
    let world = snapshot.authenticate(block)?;
    world.verify_cell_value("world.runtime_selection", &originals.selection)?;
    let state = originals
        .state
        .orchard_pool_state_ref()
        .ok_or_else(invalid)?;
    let ns = validate_pool_state(pool, asset, payer, state, &originals.head)?;
    let anchor = PrivacyRootRoleV1::NoteCommitmentAnchor;
    let state_key = PrivacyCommitmentKeyV1::orchard_pool_state(ns).map_err(|_| invalid())?;
    let head_key = PrivacyRootHeadKeyV1::new(ns, anchor).map_err(|_| invalid())?;
    let root_key = PrivacyRootKeyV1::new(ns, anchor, originals.head.epoch(), originals.head.root())
        .map_err(|_| invalid())?;
    world.verify_table_value("world.privacy_commitments", &state_key, &originals.state)?;
    world.verify_table_value("world.privacy_root_heads", &head_key, &originals.head)?;
    world.verify_table_value(
        "world.privacy_roots",
        &root_key,
        originals.head.provenance(),
    )?;
    for (owner, balance) in [
        (payer, &originals.payer_balance),
        (state.reserve_account(), &originals.reserve_balance),
    ] {
        let key = AssetId::with_scope(asset.clone(), owner.clone(), AssetBalanceScope::Global);
        match balance {
            Some(value) => world.verify_table_value("world.assets", &key, value)?,
            None => world.verify_asset_absent(&key)?,
        }
    }
    Ok(VerifiedIronwoodReplayContextV1 {
        state: state.clone(),
        payer_balance: originals.payer_balance,
        reserve_balance: originals.reserve_balance,
    })
}
