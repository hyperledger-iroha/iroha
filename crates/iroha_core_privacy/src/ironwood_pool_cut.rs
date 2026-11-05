//! Certified Global Ironwood pool anchors and public reserve/payer balances.
//! The complete World hash list requires all-ledger read authority at its producer.
use crate::privacy_state::{
    PrivacyCommitmentKeyV1, PrivacyRootHeadKeyV1, PrivacyRootHeadRecordV1, PrivacyRootKeyV1,
    PrivacyStateItemRecordV1,
};
use iroha_data_model::{
    NetworkId,
    account::AccountId,
    asset::{AssetBalanceScope, AssetDefinitionId, AssetId, AssetValue},
    privacy::{PrivacyIronwoodPoolBootstrapV1, PrivacyPoolIdV1, PrivacyRootRoleV1},
    sumeragi_finality::{
        FinalityError, VerifiedSumeragiBlock, WorldStateSchemaPolicyV1, WorldStateSnapshotV1,
    },
};

// Serialization-only reference wrapper; encodes the owner's exact semantic preimage.
mod borrowed {
    pub struct Value<'a, T>(pub &'a T);
    impl<T: norito::core::SerializePayload> norito::core::SerializePayload for Value<'_, T> {
        fn serialize(&self, out: &mut norito::core::Encoder<'_>) -> Result<(), norito::Error> {
            self.0.serialize(out)
        }
        fn encoded_len_hint(&self) -> Option<usize> {
            self.0.encoded_len_hint()
        }
        fn encoded_len_exact(&self) -> Option<usize> {
            self.0.encoded_len_exact()
        }
    }
    impl<T: norito::json::JsonSerialize> norito::json::JsonSerialize for Value<'_, T> {
        fn json_serialize(&self, out: &mut String) {
            self.0.json_serialize(out);
        }
        fn json_serialize_to(
            &self,
            out: &mut dyn norito::json::JsonWriteSink,
        ) -> Result<(), norito::json::BoundedJsonError> {
            self.0.json_serialize_to(out)
        }
    }
}

/// Data-only proof; only `verify` grants authenticated interpretation.
#[derive(
    Clone,
    Debug,
    norito::Encode,
    norito::Decode,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
    norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_core_privacy::IronwoodPoolCutProofV1")]
pub struct IronwoodPoolCutProofV1 {
    /// Complete original World hash preimages.
    pub world: WorldStateSnapshotV1,
    /// Canonical compact pool state record.
    pub state: PrivacyStateItemRecordV1,
    /// Canonical retained current root head.
    pub head: PrivacyRootHeadRecordV1,
    /// Payer's exact Global balance row; absence proves zero.
    pub payer_balance: Option<AssetValue>,
    /// Reserve's exact Global balance row; absence proves zero.
    pub reserve_balance: Option<AssetValue>,
}
/// Borrowed serialization of the same canonical proof without cloning a complete snapshot.
#[derive(norito::derive::NoritoSerialize, norito::derive::JsonSerialize)]
pub struct IronwoodPoolCutProofRefV1<'a> {
    world: borrowed::Value<'a, WorldStateSnapshotV1>,
    state: borrowed::Value<'a, PrivacyStateItemRecordV1>,
    head: borrowed::Value<'a, PrivacyRootHeadRecordV1>,
    payer_balance: Option<borrowed::Value<'a, AssetValue>>,
    reserve_balance: Option<borrowed::Value<'a, AssetValue>>,
}
impl norito::NoritoSchema for IronwoodPoolCutProofRefV1<'_> {
    fn nominal_name() -> String {
        IronwoodPoolCutProofV1::nominal_name()
    }
    fn frame_name() -> String {
        IronwoodPoolCutProofV1::frame_name()
    }
}
impl<'a> IronwoodPoolCutProofRefV1<'a> {
    /// Borrow the exact funded originals at one locked certified cut.
    pub fn new(
        world: &'a WorldStateSnapshotV1,
        state: &'a PrivacyStateItemRecordV1,
        head: &'a PrivacyRootHeadRecordV1,
        payer_balance: Option<&'a AssetValue>,
        reserve_balance: Option<&'a AssetValue>,
    ) -> Self {
        Self {
            world: borrowed::Value(world),
            state: borrowed::Value(state),
            head: borrowed::Value(head),
            payer_balance: payer_balance.map(borrowed::Value),
            reserve_balance: reserve_balance.map(borrowed::Value),
        }
    }
}
impl IronwoodPoolCutProofV1 {
    /// Authenticate the exact pool/asset/payer under an independently verified block and schema.
    /// This proves a historical retained anchor, never freshness or execution success.
    /// # Errors
    /// Foreign network/schema/scope, wrong typed originals, altered balances or missing root.
    pub fn verify(
        &self,
        network: NetworkId,
        schemas: impl WorldStateSchemaPolicyV1,
        pool: PrivacyPoolIdV1,
        asset: &AssetDefinitionId,
        payer: &AccountId,
        block: &VerifiedSumeragiBlock,
    ) -> Result<&crate::privacy_state::PrivacyOrchardPoolStateV1, FinalityError> {
        let invalid =
            || FinalityError("Ironwood pool cut differs from selected certified scope".into());
        if block.height() < 2
            || block.root_scope() != iroha_data_model::block::consensus::SumeragiRootScope::Global
            || block.commitment().schedule.current.network_id != network
            || !schemas.accepts_schema(self.world.schema_hash)
        {
            return Err(invalid());
        }
        let state = self.state.orchard_pool_state_ref().ok_or_else(invalid)?;
        state.validate().map_err(|_| invalid())?;
        if state.asset_definition_id() != asset
            || state.public_balance_scope() != AssetBalanceScope::Global
            || state.reserve_account() == payer
            || state.root() != self.head.root()
            || state.epoch() != self.head.epoch()
        {
            return Err(invalid());
        }
        let bootstrap = PrivacyIronwoodPoolBootstrapV1::new(
            pool,
            asset.clone(),
            AssetBalanceScope::Global,
            state.reserve_account().clone(),
        )
        .map_err(|_| invalid())?;
        if bootstrap.digest().map_err(|_| invalid())?.as_bytes()
            != state.bootstrap_digest().as_bytes()
        {
            return Err(invalid());
        }
        let namespace = bootstrap.namespace();
        let key = PrivacyCommitmentKeyV1::orchard_pool_state(namespace).map_err(|_| invalid())?;
        let head_key =
            PrivacyRootHeadKeyV1::new(namespace, PrivacyRootRoleV1::NoteCommitmentAnchor)
                .map_err(|_| invalid())?;
        let root_key = PrivacyRootKeyV1::new(
            namespace,
            PrivacyRootRoleV1::NoteCommitmentAnchor,
            self.head.epoch(),
            self.head.root(),
        )
        .map_err(|_| invalid())?;
        let world = self.world.authenticate(block)?;
        world.verify_table_value("world.privacy_commitments", &key, &self.state)?;
        world.verify_table_value("world.privacy_root_heads", &head_key, &self.head)?;
        world.verify_table_value("world.privacy_roots", &root_key, self.head.provenance())?;
        for (owner, balance) in [
            (payer, &self.payer_balance),
            (state.reserve_account(), &self.reserve_balance),
        ] {
            let key = AssetId::with_scope(asset.clone(), owner.clone(), AssetBalanceScope::Global);
            match balance {
                Some(value) => world.verify_table_value("world.assets", &key, value)?,
                None => world.verify_asset_absent(&key)?,
            }
        }
        Ok(state)
    }
}
