//! Ironwood payment preparation from authenticated generic State rows.
use iroha_data_model::{
    account::AccountId,
    asset::{AssetBalanceScope, AssetDefinitionId, AssetId, AssetValue},
    privacy::{
        PrivacyIronwoodPoolBootstrapV1, PrivacyNamespaceScopeV1, PrivacyPoolIdV1,
        PrivacyPoolNamespaceV1, PrivacyRootRoleV1, PrivacyRuntimeNamespaceV1,
        PrivacyRuntimeProtocolIdV1,
    },
    runtime::definition::SchemaV1,
    sumeragi_finality::{
        FinalityError, KeyedStateRowsV1, SelectedStateSchemaV1, VerifiedSumeragiBlock,
        keyed_schema::TableShape, keyed_state::StateKeyV1,
    },
};
use norito::codec::{DecodeAll, Encode};

use crate::privacy_state::{
    PrivacyCommitmentKeyV1, PrivacyOrchardPoolStateV1, PrivacyRootHeadKeyV1,
    PrivacyRootHeadRecordV1, PrivacyRootKeyV1, PrivacyRootProvenanceV1, PrivacyStateItemRecordV1,
};

/// Derive the immutable Ironwood successor codec for a base storage or event codec.
///
/// # Errors
/// The predecessor codec cannot be canonically encoded.
pub fn ironwood_successor_codec_v1(previous: &SchemaV1) -> Result<SchemaV1, norito::Error> {
    let encoded = norito::encode_canonical(previous)?;
    let label = b"recursive-codec-extension";
    let digest = iroha_crypto::Hash::new_from_chunks(&[
        b"iroha:runtime:ironwood:v1\0",
        &(label.len() as u64).to_le_bytes(),
        label,
        &(encoded.len() as u64).to_le_bytes(),
        &encoded,
    ]);
    Ok(SchemaV1::Codec(format!(
        "semantic:iroha.ironwood.v1.{}@{}",
        hex::encode(digest.as_ref()),
        layout()
    )))
}

fn layout() -> String {
    format!(
        "{}.{}.{}",
        norito::core::VERSION_MAJOR,
        norito::core::VERSION_MINOR,
        norito::core::default_encode_flags()
    )
}

fn compiled_codec<T: norito::NoritoSchema>(extended: bool) -> Result<String, FinalityError> {
    let name = format!("norito:{}@{}", T::nominal_name(), layout());
    if !extended {
        return Ok(name);
    }
    match ironwood_successor_codec_v1(&SchemaV1::Codec(name)).map_err(|_| invalid())? {
        SchemaV1::Codec(name) => Ok(name),
        _ => Err(invalid()),
    }
}

/// An independently pinned keyed State schema with verified Ironwood and asset table codecs.
pub struct IronwoodStateSchemaV1(SelectedStateSchemaV1);

impl IronwoodStateSchemaV1 {
    /// Portable schema authority for checking additional rows in a speculative batch.
    #[must_use]
    pub fn selected(&self) -> &SelectedStateSchemaV1 {
        &self.0
    }

    /// Validate a deployment-supplied selection and storage definitions against the independently
    /// chosen schema pin, then check that this SDK implements all four table codecs.
    /// Construct once per selection and reuse the result across proofs.
    ///
    /// # Errors
    /// A malformed bundle, repeated descriptor, missing storage, wrong schema or unsupported codec.
    pub fn from_bundle(frame: &[u8], expected_digest: [u8; 32]) -> Result<Self, FinalityError> {
        Self::from_selected(
            SelectedStateSchemaV1::from_bundle(frame, expected_digest)?,
            expected_digest,
        )
    }

    /// Check the actual decoder identities before making any Ironwood interpretation.
    ///
    /// # Errors
    /// The schema pin, table shape, or a key/value codec differs from the compiled protocol.
    pub fn from_selected(
        schema: SelectedStateSchemaV1,
        expected_digest: [u8; 32],
    ) -> Result<Self, FinalityError> {
        if schema.digest() != expected_digest {
            return Err(invalid());
        }
        let expected = [
            (
                "world.privacy_commitments",
                compiled_codec::<PrivacyCommitmentKeyV1>(true)?,
                compiled_codec::<PrivacyStateItemRecordV1>(true)?,
            ),
            (
                "world.privacy_root_heads",
                compiled_codec::<PrivacyRootHeadKeyV1>(true)?,
                compiled_codec::<PrivacyRootHeadRecordV1>(true)?,
            ),
            (
                "world.privacy_roots",
                compiled_codec::<PrivacyRootKeyV1>(true)?,
                compiled_codec::<PrivacyRootProvenanceV1>(true)?,
            ),
            (
                "world.assets",
                compiled_codec::<AssetId>(false)?,
                compiled_codec::<AssetValue>(false)?,
            ),
        ];
        for (table, key, value) in expected {
            let (_, descriptor) = schema.table(table).ok_or_else(invalid)?;
            if descriptor.shape() != TableShape::Table
                || descriptor.key_schema() != key
                || descriptor.value_schema() != value
            {
                return Err(invalid());
            }
        }
        Ok(Self(schema))
    }
}

fn invalid() -> FinalityError {
    FinalityError("Ironwood payment rows differ from the certified pool or balance scope".into())
}

fn row(table: &str, key: &impl Encode) -> StateKeyV1 {
    StateKeyV1 {
        table: table.into(),
        key: key.encode(),
    }
}

fn decode<T: DecodeAll>(bytes: &[u8]) -> Result<T, FinalityError> {
    let result = norito::core::with_decode_limits_scope(
        norito::canonical_decode_limits(bytes.len()),
        || norito::codec::decode_adaptive::<T>(bytes),
    )
    .map_err(|_| invalid())?;
    if result.encode() != bytes {
        return Err(invalid());
    }
    Ok(result)
}

fn namespace(pool: PrivacyPoolIdV1) -> PrivacyRuntimeNamespaceV1 {
    PrivacyRuntimeNamespaceV1::new(
        PrivacyRuntimeProtocolIdV1::IronwoodHalo2ActionsV1,
        PrivacyNamespaceScopeV1::Pool(PrivacyPoolNamespaceV1 { pool_id: pool }),
    )
}

/// Derive the first two generic selectors solely from the independently selected pool ID.
///
/// # Errors
/// Invalid pool or note-root namespace.
pub fn ironwood_pool_rows(pool: PrivacyPoolIdV1) -> Result<Vec<StateKeyV1>, FinalityError> {
    if pool.is_zero() {
        return Err(invalid());
    }
    let ns = namespace(pool);
    Ok(vec![
        row(
            "world.privacy_commitments",
            &PrivacyCommitmentKeyV1::orchard_pool_state(ns).map_err(|_| invalid())?,
        ),
        row(
            "world.privacy_root_heads",
            &PrivacyRootHeadKeyV1::new(ns, PrivacyRootRoleV1::NoteCommitmentAnchor)
                .map_err(|_| invalid())?,
        ),
    ])
}

/// Typed Ironwood context established by the initial two State lookups.
/// Its subsequent lookups always verify against the block and schema that certified this context.
pub struct IronwoodPoolContextV1<'a> {
    block: &'a VerifiedSumeragiBlock,
    schema: &'a IronwoodStateSchemaV1,
    pool: PrivacyPoolIdV1,
    payer: AccountId,
    state: PrivacyOrchardPoolStateV1,
    head: PrivacyRootHeadRecordV1,
}

impl<'a> IronwoodPoolContextV1<'a> {
    /// Authenticate the exact pool state and anchor head, then check their Ironwood semantics.
    ///
    /// # Errors
    /// Foreign root, pool, schema, asset, invalid bootstrap, or inconsistent head.
    pub fn verify(
        block: &'a VerifiedSumeragiBlock,
        schema: &'a IronwoodStateSchemaV1,
        pool: PrivacyPoolIdV1,
        asset: &AssetDefinitionId,
        payer: &AccountId,
        proof: &KeyedStateRowsV1,
    ) -> Result<Self, FinalityError> {
        if block.height() < 2
            || block.root_scope() != iroha_data_model::block::consensus::SumeragiRootScope::Global
        {
            return Err(invalid());
        }
        proof.verify(block, &schema.0, &ironwood_pool_rows(pool)?)?;
        let [record, head] = proof.rows.as_slice() else {
            return Err(invalid());
        };
        let record =
            decode::<PrivacyStateItemRecordV1>(record.value.as_deref().ok_or_else(invalid)?)?;
        let state = record.orchard_pool_state_ref().ok_or_else(invalid)?;
        let head: PrivacyRootHeadRecordV1 = decode(head.value.as_deref().ok_or_else(invalid)?)?;
        state.validate().map_err(|_| invalid())?;
        if state.asset_definition_id() != asset
            || state.public_balance_scope() != AssetBalanceScope::Global
            || state.reserve_account() == payer
            || state.epoch() != head.epoch()
            || state.root() != head.root()
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
        Ok(Self {
            block,
            schema,
            pool,
            payer: payer.clone(),
            state: state.clone(),
            head,
        })
    }

    /// Authenticated pool anchor, epoch, reserve, asset and selected public balance scope.
    #[must_use]
    pub fn state(&self) -> &PrivacyOrchardPoolStateV1 {
        &self.state
    }

    fn asset_key(&self, owner: &AccountId) -> StateKeyV1 {
        row(
            "world.assets",
            &AssetId::with_scope(
                self.state.asset_definition_id().clone(),
                owner.clone(),
                AssetBalanceScope::Global,
            ),
        )
    }

    /// Derive the exact two balance selectors and the root provenance selector in canonical order.
    ///
    /// # Errors
    /// Invalid head/root namespace.
    pub fn remaining_rows(&self) -> Result<Vec<StateKeyV1>, FinalityError> {
        let provenance = PrivacyRootKeyV1::new(
            namespace(self.pool),
            PrivacyRootRoleV1::NoteCommitmentAnchor,
            self.head.epoch(),
            self.head.root(),
        )
        .map_err(|_| invalid())?;
        let mut balances = [
            self.asset_key(&self.payer),
            self.asset_key(self.state.reserve_account()),
        ];
        balances.sort_by(|a, b| a.key.cmp(&b.key));
        let [first, second] = balances;
        Ok(vec![first, second, row("world.privacy_roots", &provenance)])
    }

    /// Authenticate provenance and the payer and reserve balances. An absent balance proves zero.
    ///
    /// # Errors
    /// Foreign block or schema, missing provenance, or any absent, extra or substituted selector.
    pub fn verify_balances(
        &self,
        proof: &KeyedStateRowsV1,
    ) -> Result<(Option<AssetValue>, Option<AssetValue>), FinalityError> {
        proof.verify(self.block, &self.schema.0, &self.remaining_rows()?)?;
        let [first, second, provenance] = proof.rows.as_slice() else {
            return Err(invalid());
        };
        if provenance.value.as_deref() != Some(self.head.provenance().encode().as_slice()) {
            return Err(invalid());
        }
        let (payer, reserve) = if first.position.key == self.asset_key(&self.payer).key {
            (first, second)
        } else {
            (second, first)
        };
        Ok((
            payer.value.as_deref().map(decode).transpose()?,
            reserve.value.as_deref().map(decode).transpose()?,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pool_selectors_are_derived_in_canonical_order_for_the_exact_namespace() {
        assert!(ironwood_pool_rows(PrivacyPoolIdV1::new([0; 32])).is_err());
        let first = ironwood_pool_rows(PrivacyPoolIdV1::new([1; 32])).unwrap();
        let other = ironwood_pool_rows(PrivacyPoolIdV1::new([2; 32])).unwrap();
        assert_eq!(first.len(), 2);
        assert_eq!(first[0].table, "world.privacy_commitments");
        assert_eq!(first[1].table, "world.privacy_root_heads");
        assert_ne!(first[0].key, other[0].key);
        assert_ne!(first[1].key, other[1].key);
        let key: PrivacyCommitmentKeyV1 = decode(&first[0].key).unwrap();
        assert_eq!(row(&first[0].table, &key), first[0]);
        let mut malformed = first[0].key.clone();
        malformed.push(0);
        assert!(decode::<PrivacyCommitmentKeyV1>(&malformed).is_err());
    }
}
