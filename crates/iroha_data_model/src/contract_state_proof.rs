//! Exact-value membership in an accumulated durable smart-contract state map.
//!
//! This map is separate from Sumeragi's per-block execution-witness roots.
//! A caller must authenticate its root through a committed consensus field before
//! an inclusion proof establishes a finalized value.

use iroha_crypto::{Hash, MerkleMap, MerkleMapError, MerkleMapProof, MerkleMapProofStep};
use iroha_model_base::state_path::StatePath;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

/// First-release proof layout and root-domain version.
pub const CONTRACT_STATE_VALUE_PROOF_VERSION_V1: u8 = 1;
/// Maximum raw value copied into one proof response.
pub const MAX_CONTRACT_STATE_PROOF_VALUE_BYTES_V1: usize = 1024 * 1024;
/// Bound for one canonical encoded inclusion proof, including path and branches.
pub const MAX_CONTRACT_STATE_PROOF_WIRE_BYTES_V1: usize =
    MAX_CONTRACT_STATE_PROOF_VALUE_BYTES_V1 + 128 * 1024;
const KEY_DOMAIN: &[u8] = b"iroha:contract-state:key:v1\0";
const VALUE_DOMAIN: &[u8] = b"iroha:contract-state:value:v1\0";

/// Hash one exact canonical physical state path for the accumulated map.
#[must_use]
pub fn contract_state_key_hash_v1(path: &StatePath) -> Hash {
    Hash::new_from_chunks(&[KEY_DOMAIN, path.as_ref().as_bytes()])
}

/// Hash the unmodified bytes stored under a physical state path.
#[must_use]
pub fn contract_state_value_hash_v1(value: &[u8]) -> Hash {
    Hash::new_from_chunks(&[VALUE_DOMAIN, value])
}

/// One canonical compressed-map branch, from root toward the exact leaf.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Decode,
    Encode,
    IntoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::contract_state_proof::ContractStateProofStepV1")]
pub struct ContractStateProofStepV1 {
    /// MSB-first split bit in `0..256`.
    pub bit: u16,
    /// Raw shared prefix with bits at and after `bit` cleared.
    pub prefix: [u8; Hash::LENGTH],
    /// Other subtree's authenticated hash.
    pub sibling: Hash,
}

/// Proof that one exact physical key holds one exact byte string in an
/// accumulated contract-state map root.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Decode,
    Encode,
    IntoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_data_model::contract_state_proof::ContractStateValueInclusionProofV1"
)]
pub struct ContractStateValueInclusionProofV1 {
    /// Exact proof layout version.
    pub version: u8,
    /// Canonical physical key, including any contract-address scope prefix.
    pub path: StatePath,
    /// Unmodified value bytes in durable `smart_contract_state`.
    pub value: Vec<u8>,
    /// Entry count bound into the accumulated map root.
    pub leaf_count: u64,
    /// Compressed-map path from root to leaf.
    pub steps: Vec<ContractStateProofStepV1>,
}

impl ContractStateValueInclusionProofV1 {
    /// Verify one exact key and value against an independently authenticated
    /// accumulated contract-state root.
    #[must_use]
    pub fn verify(&self, expected_path: &StatePath, expected_root: Hash) -> bool {
        if self.version != CONTRACT_STATE_VALUE_PROOF_VERSION_V1
            || &self.path != expected_path
            || self.value.len() > MAX_CONTRACT_STATE_PROOF_VALUE_BYTES_V1
        {
            return false;
        }
        MerkleMapProof {
            key: contract_state_key_hash_v1(&self.path),
            value: contract_state_value_hash_v1(&self.value),
            len: self.leaf_count,
            steps: self
                .steps
                .iter()
                .map(|step| MerkleMapProofStep {
                    bit: step.bit,
                    prefix: step.prefix,
                    sibling: step.sibling,
                })
                .collect(),
        }
        .verify(expected_root)
    }
}

/// Decode and verify one canonical proof under a strict allocation budget.
///
/// The `expected_root` must already be authenticated by a trusted finality
/// chain. In particular, an execution-witness root is not a valid substitute.
///
/// # Errors
/// Rejects an oversized wire frame or noncanonical or over-budget decode, or a
/// proof with the wrong version, exact path, value size, or authenticated root.
pub fn decode_verified_contract_state_value_inclusion_proof_v1(
    bytes: &[u8],
    expected_path: &StatePath,
    expected_root: Hash,
) -> Result<ContractStateValueInclusionProofV1, norito::Error> {
    if bytes.len() > MAX_CONTRACT_STATE_PROOF_WIRE_BYTES_V1 {
        return Err(norito::Error::Message(
            "contract-state proof exceeds first-release wire bound".into(),
        ));
    }
    let limits = norito::DecodeLimits::new(
        MAX_CONTRACT_STATE_PROOF_WIRE_BYTES_V1,
        MAX_CONTRACT_STATE_PROOF_WIRE_BYTES_V1,
        MAX_CONTRACT_STATE_PROOF_WIRE_BYTES_V1,
        MAX_CONTRACT_STATE_PROOF_WIRE_BYTES_V1,
        16,
    );
    let proof: ContractStateValueInclusionProofV1 =
        norito::decode_canonical_with_limits(bytes, limits)?;
    if !proof.verify(expected_path, expected_root) {
        return Err(norito::Error::Message(
            "contract-state value proof does not match trusted root and exact key".into(),
        ));
    }
    Ok(proof)
}

/// Persistent, history-independent commitment to every current physical
/// smart-contract state key/value pair.
#[derive(Clone, Default)]
pub struct ContractStateMapV1 {
    map: MerkleMap,
}

impl ContractStateMapV1 {
    /// Construct an empty accumulated map.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Cold-capture every current physical key/value pair, including entries
    /// that no transaction touched in the current block. Call this only from
    /// the authoritative State owner at initialization or recovery.
    ///
    /// # Errors
    /// Rejects a duplicate physical key or an entry-count overflow while
    /// inserting the supplied current-state entries.
    pub fn capture<'a>(
        entries: impl IntoIterator<Item = (&'a StatePath, &'a [u8])>,
    ) -> Result<Self, MerkleMapError> {
        let mut map = Self::new();
        for (path, value) in entries {
            map.replace(path, None, Some(value))?;
        }
        Ok(map)
    }

    /// Root of every current entry, including untouched historical values.
    #[must_use]
    pub fn root(&self) -> Hash {
        self.map.root()
    }

    /// Number of current keys.
    #[must_use]
    pub fn len(&self) -> u64 {
        self.map.len()
    }

    /// Whether the accumulated map has no entries.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// Apply an exact preimage-checked insert, update, or removal.
    ///
    /// `None` is absence; an empty byte string is a present value.
    /// A failed preimage check leaves the map unchanged.
    ///
    /// # Errors
    /// Rejects a mismatch between `expected` and the current value, or an
    /// update whose entry count cannot be represented by the commitment.
    pub fn replace(
        &mut self,
        path: &StatePath,
        expected: Option<&[u8]>,
        after: Option<&[u8]>,
    ) -> Result<bool, MerkleMapError> {
        self.map.replace(
            contract_state_key_hash_v1(path),
            expected.map(contract_state_value_hash_v1),
            after.map(contract_state_value_hash_v1),
        )
    }

    /// Build an inclusion proof only when the supplied raw value matches this
    /// map version. The value cap is checked before it is copied.
    #[must_use]
    pub fn proof(
        &self,
        path: &StatePath,
        value: &[u8],
    ) -> Option<ContractStateValueInclusionProofV1> {
        if value.len() > MAX_CONTRACT_STATE_PROOF_VALUE_BYTES_V1 {
            return None;
        }
        let membership = self.map.proof(&contract_state_key_hash_v1(path))?;
        if membership.value != contract_state_value_hash_v1(value) {
            return None;
        }
        Some(ContractStateValueInclusionProofV1 {
            version: CONTRACT_STATE_VALUE_PROOF_VERSION_V1,
            path: path.clone(),
            value: value.to_vec(),
            leaf_count: membership.len,
            steps: membership
                .steps
                .into_iter()
                .map(|step| ContractStateProofStepV1 {
                    bit: step.bit,
                    prefix: step.prefix,
                    sibling: step.sibling,
                })
                .collect(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn inclusion_binds_exact_path_value_and_accumulated_history() {
        let a = StatePath::from_str("sc/alpha/Balance").unwrap();
        let b = StatePath::from_str("sc/beta/Balance").unwrap();
        let mut map = ContractStateMapV1::new();
        map.replace(&a, None, Some(b"one")).unwrap();
        let first_root = map.root();
        map.replace(&b, None, Some(b"two")).unwrap();
        let current_root = map.root();
        let proof = map.proof(&a, b"one").unwrap();
        // Independent Python Blake2b-256 vector shared by the JS, Kotlin,
        // and Swift verifier tests; Iroha's low-bit marker is applied per hash.
        assert_eq!(
            current_root.to_string(),
            "194a8961806570284bf970836427142baa1ddcab853f1ee2c3ae672e1da8acb3"
        );
        assert_eq!(proof.steps.len(), 1);
        assert_eq!(proof.steps[0].bit, 0);
        assert_eq!(
            proof.steps[0].sibling.to_string(),
            "482931df820458f6bf299fa0e88e37f2378b3e8558c2c254ad03755ed8790947"
        );
        assert!(proof.verify(&a, current_root));
        assert!(!proof.verify(&a, first_root));
        assert!(!proof.verify(&b, current_root));
        assert!(map.proof(&a, b"wrong").is_none());
        let mut tampered = proof.clone();
        tampered.value = b"other".to_vec();
        assert!(!tampered.verify(&a, current_root));
        tampered = proof.clone();
        tampered.leaf_count += 1;
        assert!(!tampered.verify(&a, current_root));
        tampered = proof.clone();
        tampered.version += 1;
        assert!(!tampered.verify(&a, current_root));
        let bare_payload = proof.encode();
        assert!(
            decode_verified_contract_state_value_inclusion_proof_v1(
                &bare_payload,
                &a,
                current_root,
            )
            .is_err(),
            "a bare proof payload cannot stand in for canonical Norito wire",
        );
        let encoded = norito::encode_canonical(&proof).expect("canonical proof encodes");
        let decoded =
            decode_verified_contract_state_value_inclusion_proof_v1(&encoded, &a, current_root)
                .expect("canonical proof decodes");
        assert_eq!(proof, decoded);
        let json = norito::json::to_json(&proof).expect("proof JSON encodes");
        let decoded_json: ContractStateValueInclusionProofV1 =
            norito::json::from_json(&json).expect("proof JSON decodes");
        assert_eq!(decoded_json, proof);
        let captured =
            ContractStateMapV1::capture([(&b, b"two".as_slice()), (&a, b"one".as_slice())])
                .unwrap();
        assert_eq!(captured.root(), current_root);
        assert!(captured.proof(&a, b"one").unwrap().verify(&a, current_root));
    }
}
