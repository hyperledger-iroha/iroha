//! Exact Ironwood pool and Global balance originals at one funded certified cut.
use super::*;
use iroha_core_privacy::{ironwood_pool_cut::IronwoodPoolCutProofRefV1, privacy_state::*};
use iroha_data_model::{
    asset::{AssetBalanceScope, AssetId},
    privacy::{
        PrivacyNamespaceScopeV1, PrivacyPoolIdV1, PrivacyPoolNamespaceV1, PrivacyRootRoleV1,
        PrivacyRuntimeNamespaceV1, PrivacyRuntimeProtocolIdV1,
    },
};
impl State {
    /// Borrow original typed pool/root/balance rows under the complete certified World snapshot.
    /// Caller must establish current all-ledger read authority before and after publication.
    /// # Errors
    /// Unavailable exact cut, foreign scope, missing pool, changed pre-tail originals or bounds.
    pub fn with_native_ironwood_pool_cut_v1<T>(
        &self,
        tip: &CommittedBlock,
        pool: PrivacyPoolIdV1,
        payer: &AccountId,
        budget: &AllocationBudget,
        consume: impl FnOnce(IronwoodPoolCutProofRefV1<'_>) -> Result<T, String>,
    ) -> Result<T, WorldStateSnapshotError> {
        if tip.root_scope() != iroha_data_model::block::consensus::SumeragiRootScope::Global
            || pool.is_zero()
        {
            return Err("Ironwood cut requires Global pool".into());
        }
        let namespace = PrivacyRuntimeNamespaceV1::new(
            PrivacyRuntimeProtocolIdV1::IronwoodHalo2ActionsV1,
            PrivacyNamespaceScopeV1::Pool(PrivacyPoolNamespaceV1 { pool_id: pool }),
        );
        self.with_native_world_state_snapshot_cut_v1(tip, None, budget, |snapshot, world| {
            let state_key =
                PrivacyCommitmentKeyV1::orchard_pool_state(namespace).map_err(str::to_owned)?;
            let record = world
                .privacy_commitments
                .get(&state_key)
                .ok_or("Ironwood pool is absent")?;
            let state = record
                .orchard_pool_state_ref()
                .ok_or("Ironwood pool record has wrong role")?;
            let head_key =
                PrivacyRootHeadKeyV1::new(namespace, PrivacyRootRoleV1::NoteCommitmentAnchor)
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
            require_target(
                snapshot,
                "world.privacy_commitments",
                WorldStateElementKindV1::Table,
                Some(hash_value(&state_key)?),
                hash_value(record)?,
            )?;
            require_target(
                snapshot,
                "world.privacy_root_heads",
                WorldStateElementKindV1::Table,
                Some(hash_value(&head_key)?),
                hash_value(head)?,
            )?;
            require_target(
                snapshot,
                "world.privacy_roots",
                WorldStateElementKindV1::Table,
                Some(hash_value(&root_key)?),
                hash_value(head.provenance())?,
            )?;
            if state.public_balance_scope() != AssetBalanceScope::Global
                || state.root() != head.root()
                || state.epoch() != head.epoch()
                || state.reserve_account() == payer
            {
                return Err("Ironwood pool originals disagree".into());
            }
            let balances = [payer, state.reserve_account()].map(|owner| {
                let key = AssetId::with_scope(
                    state.asset_definition_id().clone(),
                    owner.clone(),
                    AssetBalanceScope::Global,
                );
                let balance = world.assets.get(&key);
                let hash = hash_value(&key)?;
                if let Some(value) = balance {
                    require_target(
                        snapshot,
                        "world.assets",
                        WorldStateElementKindV1::Table,
                        Some(hash),
                        hash_value(value)?,
                    )?;
                } else if snapshot
                    .entries
                    .iter()
                    .any(|row| row.field_id == "world.assets" && row.key_hash == Some(hash))
                {
                    return Err("Ironwood balance absence differs from certified cut".to_owned());
                }
                Ok(balance)
            });
            let [payer, reserve] = balances;
            consume(IronwoodPoolCutProofRefV1::new(
                snapshot, record, head, payer?, reserve?,
            ))
            .map_err(WorldStateSnapshotError::from)
        })
    }
}
