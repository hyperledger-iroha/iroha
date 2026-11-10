//! Selected State row originals and lookup witnesses bound to one certified execution.

use super::{
    FinalityError, SelectedStateSchemaV1, VerifiedSumeragiBlock,
    keyed_state::{StateKeyV1, StateLookupProofV1},
};
use crate::block::BlockHeader;
use iroha_crypto::HashOf;
use norito::{
    Decode, Encode,
    derive::{JsonDeserialize, JsonSerialize},
};

/// Maximum number of exact lookups in one portable response.
pub const MAX_STATE_PROOF_ROWS_V1: usize = 32;
/// Reader bound for a registry table identifier.
pub const MAX_STATE_PROOF_TABLE_BYTES_V1: usize = 256;
/// Reader bound for one canonical key. A cell uses the empty key.
pub const MAX_STATE_PROOF_KEY_BYTES_V1: usize = 4096;
/// Maximum portable selected-row response. Smaller Torii limits may also apply.
pub const MAX_STATE_ROWS_FRAME_BYTES_V1: usize = 32 * 1024 * 1024;

/// Original bytes and witness for one requested key. `None` denotes proven absence.
#[derive(Clone, Debug, Encode, Decode, JsonSerialize, JsonDeserialize, norito::NoritoSchema)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::sumeragi_finality::KeyedStateRowV1")]
pub struct KeyedStateRowV1 {
    /// Exact table identity and bare Norito key.
    pub position: StateKeyV1,
    /// Original value bytes if present, including a possible empty value.
    #[norito(required)]
    pub value: Option<Vec<u8>>,
    /// Membership or absence witness for this position.
    pub witness: StateLookupProofV1,
}

/// Closed batch of lookup proofs for a single certified post-execution State.
#[derive(Clone, Debug, Encode, Decode, JsonSerialize, JsonDeserialize, norito::NoritoSchema)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::sumeragi_finality::KeyedStateRowsV1")]
pub struct KeyedStateRowsV1 {
    /// Current certified non-genesis height requested by the reader.
    pub height: u64,
    /// Canonical Iroha header hash at this height.
    pub block_hash: HashOf<BlockHeader>,
    /// Schema and commitment profile digest bound by the keyed root.
    pub schema_hash: [u8; 32],
    /// Exact rows in the request's strictly ascending (table, key) order.
    pub rows: Vec<KeyedStateRowV1>,
}

impl KeyedStateRowsV1 {
    /// Verify every requested presence or absence against an independently certified Global block.
    /// `expected_rows` and the schema pin come from the caller. Success authenticates this cut only.
    ///
    /// # Errors
    /// Rejects a wrong block, scope or schema, incomplete or reordered selection, or invalid witness.
    pub fn verify(
        &self,
        block: &VerifiedSumeragiBlock,
        schema: &SelectedStateSchemaV1,
        expected_rows: &[StateKeyV1],
    ) -> Result<(), FinalityError> {
        if block.root_scope() != crate::block::consensus::SumeragiRootScope::Global
            || self.height < 2
            || self.height != block.height()
            || self.block_hash != block.header().hash()
            || self.schema_hash != schema.digest()
        {
            return Err(FinalityError(
                "State rows differ from the selected certified cut".into(),
            ));
        }
        self.verify_rows(
            block.execution().world_state_root.as_ref(),
            schema,
            expected_rows,
        )
    }

    fn verify_rows(
        &self,
        root: &[u8; 32],
        schema: &SelectedStateSchemaV1,
        expected: &[StateKeyV1],
    ) -> Result<(), FinalityError> {
        if expected.is_empty()
            || expected.len() > MAX_STATE_PROOF_ROWS_V1
            || self.rows.len() != expected.len()
            || expected
                .windows(2)
                .any(|pair| (&pair[0].table, &pair[0].key) >= (&pair[1].table, &pair[1].key))
        {
            return Err(FinalityError(
                "State row selection is empty, oversized, or unordered".into(),
            ));
        }
        for (row, key) in self.rows.iter().zip(expected) {
            if schema.check_key(&key.table, &key.key).is_err()
                || key.table.is_empty()
                || key.table.len() > MAX_STATE_PROOF_TABLE_BYTES_V1
                || key.key.len() > MAX_STATE_PROOF_KEY_BYTES_V1
                || row.position != *key
                || !row
                    .witness
                    .verify(&self.schema_hash, root, key, row.value.as_deref())
            {
                return Err(FinalityError(
                    "State row differs from its certified position or value".into(),
                ));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sumeragi_finality::{
        keyed_state::KeyedStateTreeV1, test_fixtures::NativeFinalityFixture,
    };

    #[test]
    fn a_batch_requires_every_independently_pinned_row_under_the_same_certified_root() {
        let (selection, definitions) = crate::sumeragi_finality::keyed_schema::tests::fixture();
        let schema = SelectedStateSchemaV1::from_selection(&selection, &definitions).unwrap();
        let mut tree = KeyedStateTreeV1::new(
            schema.digest(),
            &iroha_allocation::AllocationBudget::new(128 * 1024),
        );
        let keys = [0, 1].map(|id| StateKeyV1 {
            table: "world.assets".into(),
            key: vec![id],
        });
        tree.replace(&keys[0], Some(&[])).unwrap();
        let mut chain = NativeFinalityFixture::start("keyed-read");
        let block = chain.block_with_submitted_work(chain.next_header());
        let cert = chain.certify_with_world_root(block, tree.root());
        let tip = chain.verifier().verify_retained_decision(&cert).unwrap();
        let proof = KeyedStateRowsV1 {
            height: tip.height(),
            block_hash: tip.header().hash(),
            schema_hash: schema.digest(),
            rows: keys
                .iter()
                .map(|k| KeyedStateRowV1 {
                    position: k.clone(),
                    value: tree.get(k).map(<[u8]>::to_vec),
                    witness: tree.prove(k).unwrap(),
                })
                .collect(),
        };
        let bytes = norito::encode_canonical(&proof).unwrap();
        let proof: KeyedStateRowsV1 = norito::decode_canonical(&bytes).unwrap();
        proof.verify(&tip, &schema, &keys).unwrap();
        let mut other = selection.clone();
        other.installed_storage.clear();
        let wrong_schema = SelectedStateSchemaV1::from_selection(&other, &definitions).unwrap();
        assert!(proof.verify(&tip, &wrong_schema, &keys).is_err());
        for (table, key) in [("world.absent", vec![]), ("world.epoch", vec![1])] {
            let position = StateKeyV1 {
                table: table.into(),
                key,
            };
            let witness = tree.prove(&position).unwrap();
            assert!(witness.verify(&schema.digest(), &tree.root(), &position, None));
            let mut bad = proof.clone();
            bad.rows = vec![KeyedStateRowV1 {
                position: position.clone(),
                value: None,
                witness,
            }];
            assert!(bad.verify(&tip, &schema, &[position]).is_err());
        }
        assert!(proof.verify(&tip, &schema, &keys[..1]).is_err());
        let mut forged = proof.clone();
        forged.rows[0].value = None;
        assert!(forged.verify(&tip, &schema, &keys).is_err());
        let mut wrong = proof.clone();
        wrong.rows.swap(0, 1);
        assert!(wrong.verify(&tip, &schema, &keys).is_err());
        let wrong = chain.block_with_submitted_work(chain.next_header());
        let cert = chain.certify_with_world_root(wrong, tree.root());
        let another_tip = chain.verifier().verify_retained_decision(&cert).unwrap();
        assert!(proof.verify(&another_tip, &schema, &keys).is_err());
    }
}
