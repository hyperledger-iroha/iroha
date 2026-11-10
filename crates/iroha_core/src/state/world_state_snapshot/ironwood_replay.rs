//! Operator-local Ironwood originals at the exact replayed certified pre-tail World cut.

use super::*;
use iroha_allocation::AllocationReservation;
use iroha_core_privacy::{
    ironwood_replay::{IronwoodReplayOriginalsV1, MAX_IRONWOOD_REPLAY_ORIGINALS_BYTES_V1},
    privacy_state::{PrivacyCommitmentKeyV1, PrivacyRootHeadKeyV1, PrivacyRootKeyV1},
};
use iroha_data_model::{
    asset::{AssetBalanceScope, AssetId, AssetValue},
    block::consensus::SumeragiRootScope,
    isi::privacy::SubmitPrivacyIronwoodProofV1,
    privacy::{
        PrivacyIronwoodPoolBootstrapV1, PrivacyNamespaceScopeV1, PrivacyPoolIdV1,
        PrivacyPoolNamespaceV1, PrivacyRootRoleV1, PrivacyRuntimeNamespaceV1,
        PrivacyRuntimeProtocolIdV1,
    },
    sumeragi_finality::{ironwood_world_schema_v1, lthash16_world_profile_v1},
};

struct FundedOriginal<T> {
    value: T,
    _retained: AllocationReservation,
}

// Copy through the native bounded codec so neither spare source capacity nor an unchecked
// Clone can escape the caller's finite pool. The returned reservation outlives the owned value.
fn funded_copy<T>(
    source: &T,
    budget: &AllocationBudget,
) -> Result<FundedOriginal<T>, WorldStateSnapshotError>
where
    T: norito::NoritoSerialize,
    for<'de> T: norito::NoritoDeserialize<'de>,
{
    let length = norito::canonical_frame_len(source).map_err(|error| error.to_string())?;
    if length == 0 || length > MAX_IRONWOOD_REPLAY_ORIGINALS_BYTES_V1 {
        return Err("Ironwood original exceeds its canonical frame bound".into());
    }
    let limits = norito::canonical_decode_limits(length);
    let retained = budget.try_reserve_bytes(
        length
            .checked_add(limits.max_total_allocated_bytes())
            .ok_or("Ironwood original allocation geometry overflows")?,
    )?;
    let mut counters = budget.try_reserve(norito::core::DecodeBudgetContext::allocation_layout())?;
    let context = norito::core::DecodeBudgetContext::from_reservation(limits, &mut counters)?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(length)
        .map_err(WorldStateSnapshotError::Allocator)?;
    bytes.resize(length, 0);
    let mut writer = std::io::Cursor::new(bytes.as_mut_slice());
    norito::core::write_canonical_to_writer(source, &mut writer)
        .map_err(|error| error.to_string())?;
    if writer.position() != length as u64 {
        return Err("Ironwood original changed during canonical copy".into());
    }
    let value = context
        .with(|| norito::decode_canonical_for_admission(&bytes, limits))
        .map_err(WorldStateSnapshotError::Encoding)?;
    Ok(FundedOriginal {
        value,
        _retained: retained,
    })
}

fn require_global(world: &impl WorldReadOnly) -> Result<(), WorldStateSnapshotError> {
    let root = crate::sumeragi::lanes::routing::read_routing_root_scope(world)
        .map_err(WorldStateSnapshotError::RootScopeRead)?;
    if root != Some(SumeragiRootScope::Global) {
        return Err("Ironwood replay requires the authenticated Global root".into());
    }
    Ok(())
}

fn require_balance(
    snapshot: &WorldStateSnapshotV1,
    key: &AssetId,
    balance: Option<&AssetValue>,
) -> Result<(), String> {
    let key_hash = hash_value(key)?;
    if let Some(value) = balance {
        require_target(
            snapshot,
            "world.assets",
            WorldStateElementKindV1::Table,
            Some(key_hash),
            hash_value(value)?,
        )
    } else if snapshot.entries.iter().any(|row| {
        row.field_id == "world.assets"
            && row.kind == WorldStateElementKindV1::Table
            && row.key_hash == Some(key_hash)
    }) {
        Err("Ironwood balance absence differs from the certified cut".into())
    } else {
        Ok(())
    }
}

fn require_selection(
    snapshot: &WorldStateSnapshotV1,
    selection: &iroha_data_model::runtime::definition::RootSelectionV1,
) -> Result<(), WorldStateSnapshotError> {
    if snapshot.schema_hash != ironwood_world_schema_v1()
        || snapshot.schema_hash != selection_schema(selection)?
        || selection.commitment_profile != lthash16_world_profile_v1()
        || selection
            .bindings
            .binary_search_by(|binding| {
                binding
                    .wire_id
                    .as_str()
                    .cmp(SubmitPrivacyIronwoodProofV1::WIRE_ID)
            })
            .is_err()
    {
        return Err("Ironwood replay runtime selection differs from the supported World".into());
    }
    require_target(
        snapshot,
        "world.runtime_selection",
        WorldStateElementKindV1::Cell,
        None,
        hash_value(selection)?,
    )?;
    Ok(())
}

fn pool_originals<'a>(
    snapshot: &WorldStateSnapshotV1,
    world: &'a WorldBlock<'_>,
    namespace: PrivacyRuntimeNamespaceV1,
    asset: &AssetDefinitionId,
    reserve: &AccountId,
) -> Result<
    (
        &'a iroha_core_privacy::privacy_state::PrivacyStateItemRecordV1,
        &'a iroha_core_privacy::privacy_state::PrivacyRootHeadRecordV1,
    ),
    WorldStateSnapshotError,
> {
    let state_key = PrivacyCommitmentKeyV1::orchard_pool_state(namespace).map_err(str::to_owned)?;
    let record = world
        .privacy_commitments
        .get(&state_key)
        .ok_or("Ironwood pool is absent")?;
    let state = record
        .orchard_pool_state_ref()
        .ok_or("Ironwood pool record has wrong role")?;
    let head_key = PrivacyRootHeadKeyV1::new(namespace, PrivacyRootRoleV1::NoteCommitmentAnchor)
        .map_err(str::to_owned)?;
    let head = world
        .privacy_root_heads
        .get(&head_key)
        .ok_or("Ironwood root head is absent")?;
    let root_key = PrivacyRootKeyV1::new(
        namespace,
        PrivacyRootRoleV1::NoteCommitmentAnchor,
        head.epoch(),
        head.root(),
    )
    .map_err(str::to_owned)?;
    let provenance = world
        .privacy_roots
        .get(&root_key)
        .ok_or("Ironwood root provenance is absent")?;
    if state.asset_definition_id() != asset
        || state.public_balance_scope() != AssetBalanceScope::Global
        || state.reserve_account() != reserve
        || state.epoch() != head.epoch()
        || state.root() != head.root()
        || provenance != head.provenance()
    {
        return Err("Ironwood pool, governed reserve and root originals disagree".into());
    }
    for (field, key, value) in [
        (
            "world.privacy_commitments",
            hash_value(&state_key)?,
            hash_value(record)?,
        ),
        (
            "world.privacy_root_heads",
            hash_value(&head_key)?,
            hash_value(head)?,
        ),
        (
            "world.privacy_roots",
            hash_value(&root_key)?,
            hash_value(provenance)?,
        ),
    ] {
        require_target(
            snapshot,
            field,
            WorldStateElementKindV1::Table,
            Some(key),
            value,
        )?;
    }
    Ok((record, head))
}

fn require_bootstrap(
    pool: PrivacyPoolIdV1,
    asset: &AssetDefinitionId,
    reserve: &AccountId,
    state: &iroha_core_privacy::privacy_state::PrivacyOrchardPoolStateV1,
    budget: &AllocationBudget,
) -> Result<(), WorldStateSnapshotError> {
    let asset_copy = funded_copy(asset, budget)?;
    let reserve_copy = funded_copy(reserve, budget)?;
    let bootstrap = PrivacyIronwoodPoolBootstrapV1::new(
        pool,
        asset_copy.value,
        AssetBalanceScope::Global,
        reserve_copy.value,
    )
    .map_err(|error| error.to_string())?;
    if bootstrap
        .digest()
        .map_err(|error| error.to_string())?
        .as_bytes()
        != state.bootstrap_digest().as_bytes()
    {
        return Err("Ironwood bootstrap binding differs from the selected pool".into());
    }
    Ok(())
}

fn balance_originals<'a>(
    snapshot: &WorldStateSnapshotV1,
    world: &'a WorldBlock<'_>,
    asset: &AssetDefinitionId,
    owners: [&AccountId; 2],
    budget: &AllocationBudget,
) -> Result<[Option<&'a AssetValue>; 2], WorldStateSnapshotError> {
    let mut balances = [None, None];
    for (index, owner) in owners.into_iter().enumerate() {
        let asset_copy = funded_copy(asset, budget)?;
        let owner_copy = funded_copy(owner, budget)?;
        let key = AssetId::with_scope(
            asset_copy.value,
            owner_copy.value,
            AssetBalanceScope::Global,
        );
        let balance = world.assets.get(&key);
        require_balance(snapshot, &key, balance)?;
        balances[index] = balance;
    }
    Ok(balances)
}

fn with_funded_originals<T>(
    snapshot: &WorldStateSnapshotV1,
    selection: &iroha_data_model::runtime::definition::RootSelectionV1,
    record: &iroha_core_privacy::privacy_state::PrivacyStateItemRecordV1,
    head: &iroha_core_privacy::privacy_state::PrivacyRootHeadRecordV1,
    balances: [Option<&AssetValue>; 2],
    budget: &AllocationBudget,
    consume: impl FnOnce(&WorldStateSnapshotV1, &IronwoodReplayOriginalsV1) -> Result<T, String>,
) -> Result<T, WorldStateSnapshotError> {
    // Bound the aggregate before allocating any retained originals. Individual frame
    // headers make this slightly stricter than the final DTO's independent 4 MiB cap.
    let mut total = 0usize;
    for length in [
        norito::canonical_frame_len(selection),
        norito::canonical_frame_len(record),
        norito::canonical_frame_len(head),
    ] {
        total = total
            .checked_add(length.map_err(|error| error.to_string())?)
            .ok_or("Ironwood originals byte count overflows")?;
    }
    for balance in balances.into_iter().flatten() {
        total = total
            .checked_add(norito::canonical_frame_len(balance).map_err(|error| error.to_string())?)
            .ok_or("Ironwood originals byte count overflows")?;
    }
    if total > MAX_IRONWOOD_REPLAY_ORIGINALS_BYTES_V1 {
        return Err("Ironwood originals exceed the aggregate canonical frame bound".into());
    }
    let selection = funded_copy(selection, budget)?;
    let state = funded_copy(record, budget)?;
    let head = funded_copy(head, budget)?;
    let mut payer_copy = balances[0]
        .map(|value| {
            funded_copy(value, budget).map(|owner| FundedOriginal {
                value: Some(owner.value),
                _retained: owner._retained,
            })
        })
        .transpose()?;
    let mut reserve_copy = balances[1]
        .map(|value| {
            funded_copy(value, budget).map(|owner| FundedOriginal {
                value: Some(owner.value),
                _retained: owner._retained,
            })
        })
        .transpose()?;
    // Move physical values only after every fallible copy has acquired its owner. The
    // remaining reservations stay in these stack owners until originals has been dropped.
    let originals = IronwoodReplayOriginalsV1 {
        selection: selection.value,
        state: state.value,
        head: head.value,
        payer_balance: payer_copy.as_mut().and_then(|owner| owner.value.take()),
        reserve_balance: reserve_copy.as_mut().and_then(|owner| owner.value.take()),
    };
    if norito::canonical_frame_len(&originals).map_err(|error| error.to_string())?
        > MAX_IRONWOOD_REPLAY_ORIGINALS_BYTES_V1
    {
        return Err("Ironwood originals exceed their canonical frame bound".into());
    }
    consume(snapshot, &originals).map_err(WorldStateSnapshotError::from)
}

impl State {
    /// Borrow bounded World and typed Ironwood originals from one replayed certified cut.
    ///
    /// This operator-local API requires the integrating owner to hold the copied node inputs
    /// and full-World outputs in private custody. The consumer independently authenticates the
    /// frames, selected runtime and certified block before preparing any transaction. No read
    /// authority or ledger verdict is conveyed by producing these unauthenticated originals.
    ///
    /// # Errors
    /// Refuses a foreign, stale or unavailable native cut, a mismatched pool/asset/reserve,
    /// changed pre-tail originals, unsupported storage selection, bounded encoding failure,
    /// or the original physical allocation, decoder or State reader refusal.
    #[allow(clippy::too_many_arguments)]
    pub fn with_replayed_ironwood_context_v1<T>(
        &self,
        tip: &CurrentCertifiedCut,
        pool: PrivacyPoolIdV1,
        asset: &AssetDefinitionId,
        payer: &AccountId,
        reserve: &AccountId,
        budget: &AllocationBudget,
        consume: impl FnOnce(&WorldStateSnapshotV1, &IronwoodReplayOriginalsV1) -> Result<T, String>,
    ) -> Result<T, WorldStateSnapshotError> {
        if tip.height() < 2
            || tip.root_scope() != SumeragiRootScope::Global
            || pool.is_zero()
            || payer == reserve
        {
            return Err(
                "Ironwood replay requires a Global successor cut and distinct owners".into(),
            );
        }
        let namespace = PrivacyRuntimeNamespaceV1::new(
            PrivacyRuntimeProtocolIdV1::IronwoodHalo2ActionsV1,
            PrivacyNamespaceScopeV1::Pool(PrivacyPoolNamespaceV1 { pool_id: pool }),
        );
        self.with_native_world_state_snapshot_cut_v1(tip, None, budget, |snapshot, world| {
            require_global(world)?;
            let selection = world.runtime_selection.get();
            require_selection(snapshot, selection)?;
            let (record, head) = pool_originals(snapshot, world, namespace, asset, reserve)?;
            let state = record
                .orchard_pool_state_ref()
                .ok_or("Ironwood pool record has wrong role")?;
            require_bootstrap(pool, asset, reserve, state, budget)?;
            let balances = balance_originals(snapshot, world, asset, [payer, reserve], budget)?;
            with_funded_originals(snapshot, selection, record, head, balances, budget, consume)
        })
    }
}

#[cfg(test)]
#[path = "ironwood_replay_tests.rs"]
mod tests;
