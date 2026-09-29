//! Bridge-related data types for wrapped assets and receipts. Feature-gated behind `bridge`.

use crate::proof::ProofBox;
use crate::{DeriveJsonDeserialize, DeriveJsonSerialize};
use iroha_model_base::topology::LaneId;
use iroha_primitives::numeric::Quantity;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
use std::{string::String, vec::Vec};
/// SCCP network profile shared by the SCCP v1 data model.
pub mod sccp;
pub use sccp::SccpNetworkV1;
/// Definition metadata for a wrapped asset originating from another chain.
///
/// Stored alongside an Iroha asset definition to bind it to its origin.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Decode,
    Encode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(no_fast_from_json)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::bridge::WrappedAssetDef")]
pub struct WrappedAssetDef {
    /// Origin chain identifier (canonical bytes, e.g., "btc", "evm-eth").
    pub origin_chain: Vec<u8>,
    /// Origin asset identifier on the origin chain (canonical bytes).
    pub origin_asset_id: Vec<u8>,
    /// Bridge lane identifier that minted this wrapped asset (canonical bytes).
    pub bridge_id: Vec<u8>,
}
/// A receipt emitted by the bridge lane to record a cross-chain action.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Decode,
    Encode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(no_fast_from_json)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::bridge::BridgeReceipt")]
pub struct BridgeReceipt {
    /// Lane identifier (e.g., "btc→iroha", "iroha↔evm").
    pub lane: LaneId,
    /// Direction of the action: "lock", "mint", "burn", or "release".
    pub direction: Vec<u8>,
    /// Source transaction or message hash (32 bytes canonical).
    pub source_tx: [u8; 32],
    /// Optional destination transaction hash, if known.
    pub dest_tx: Option<[u8; 32]>,
    /// Hash of the verification proof submitted for this action.
    pub proof_hash: [u8; 32],
    /// Exact non-negative amount transferred in the asset's native precision.
    pub amount: Quantity,
    /// Canonical Iroha asset id bytes.
    pub asset_id: Vec<u8>,
    /// Recipient identifier bytes (Iroha account id or external address payload).
    pub recipient: Vec<u8>,
}
/// Hash function used by bridge Merkle proofs.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Decode,
    Encode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(no_fast_from_json)]
#[norito(deny_unknown_fields)]
#[norito(tag = "hash_function", content = "value")]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::bridge::BridgeHashFunction")]
pub enum BridgeHashFunction {
    /// SHA-256 (ICS-style hash-only light clients).
    Sha256,
    /// Blake2b (mirrors Iroha’s internal hash).
    Blake2b,
}
/// Height range covered by a bridge proof artifact.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Decode,
    Encode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(no_fast_from_json)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::bridge::BridgeProofRange")]
pub struct BridgeProofRange {
    /// Inclusive start height of the batch.
    pub start_height: u64,
    /// Inclusive end height of the batch.
    pub end_height: u64,
}
impl BridgeProofRange {
    /// Returns `true` if the range is non-empty and ordered.
    #[must_use]
    pub const fn is_valid(&self) -> bool {
        self.start_height <= self.end_height
    }
    /// Length of the covered window (`end_height - start_height + 1`).
    #[must_use]
    pub const fn len(&self) -> u64 {
        self.end_height
            .saturating_sub(self.start_height)
            .saturating_add(1)
    }
    /// Returns `true` when the range is empty.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len() == 0
    }
}
/// ICS-style proof payload (hash-only light client).
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Decode,
    Encode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(no_fast_from_json)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::bridge::BridgeIcsProof")]
pub struct BridgeIcsProof {
    /// Exact verifier manifest commitment selected for this proof.
    pub verifier_manifest_hash: [u8; 32],
    /// State root advertised by the counterparty chain.
    pub state_root: [u8; 32],
    /// Leaf hash being proven.
    pub leaf_hash: [u8; 32],
    /// Compact Merkle path from leaf to root.
    pub proof: iroha_crypto::MerkleProof<[u8; 32]>,
    /// Hash function used when computing parent nodes.
    pub hash_function: BridgeHashFunction,
}
/// Transparent ZK proof payload (rolling recursive proof).
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Decode,
    Encode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(no_fast_from_json)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::bridge::BridgeTransparentProof")]
pub struct BridgeTransparentProof {
    /// Exact verifier manifest commitment selected for this proof.
    pub verifier_manifest_hash: [u8; 32],
    /// Opaque proof bytes tagged with backend identifier.
    pub proof: ProofBox,
    /// Optional recursion depth claimed by the prover.
    pub recursion_depth: Option<u32>,
}
/// Bridge proof payload kinds supported by the data model.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Decode,
    Encode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(no_fast_from_json)]
#[norito(deny_unknown_fields)]
#[norito(tag = "kind", content = "payload")]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::bridge::BridgeProofPayload")]
pub enum BridgeProofPayload {
    /// ICS-23-style inclusion proof against a state root.
    #[codec(index = 0)]
    Ics(BridgeIcsProof),
    /// Transparent recursive ZK proof.
    #[codec(index = 1)]
    TransparentZk(BridgeTransparentProof),
}
/// Typed verifier binding computed from a bridge proof payload.
///
/// This value is not stored independently in [`BridgeProof`]. Keeping the
/// commitment beside the payload that defines its meaning keeps its role explicit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum BridgeProofBinding {
    /// Commitment to the verifier manifest used by a generic proof backend.
    VerifierManifest([u8; 32]),
}
impl BridgeProofBinding {
    /// Return the bound commitment bytes.
    #[must_use]
    pub const fn hash(self) -> [u8; 32] {
        match self {
            Self::VerifierManifest(hash) => hash,
        }
    }
    /// Return whether the binding carries a nonzero commitment.
    #[must_use]
    pub fn is_well_formed(self) -> bool {
        self.hash().iter().any(|byte| *byte != 0)
    }
}
impl BridgeProofPayload {
    /// Return the role-preserving verifier binding carried by this payload.
    #[must_use]
    pub const fn binding(&self) -> BridgeProofBinding {
        match self {
            Self::Ics(proof) => BridgeProofBinding::VerifierManifest(proof.verifier_manifest_hash),
            Self::TransparentZk(proof) => {
                BridgeProofBinding::VerifierManifest(proof.verifier_manifest_hash)
            }
        }
    }
}
/// Bridge proof artifact with a payload-owned verifier binding.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Decode,
    Encode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(no_fast_from_json)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::bridge::BridgeProof")]
pub struct BridgeProof {
    /// Height range covered by this proof.
    pub range: BridgeProofRange,
    /// Proof payload (generic ICS or transparent ZK).
    pub payload: BridgeProofPayload,
}
impl BridgeProof {
    /// Return the role-preserving verifier binding carried by the payload.
    #[must_use]
    pub const fn binding(&self) -> BridgeProofBinding {
        self.payload.binding()
    }
    /// Return a backend label suitable for hashing/id construction.
    #[must_use]
    pub fn backend_label(&self) -> String {
        match &self.payload {
            BridgeProofPayload::Ics(_) => "bridge/ics23".to_owned(),
            BridgeProofPayload::TransparentZk(p) => {
                format!("bridge/{}", p.proof.backend)
            }
        }
    }
}
/// Stored bridge proof record with size metadata and commitment.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Decode,
    Encode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(no_fast_from_json)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::bridge::BridgeProofRecord")]
pub struct BridgeProofRecord {
    /// Recorded proof artifact.
    pub proof: BridgeProof,
    /// Hash commitment for the proof bytes (backend-specific).
    pub commitment: [u8; 32],
    /// Total encoded size of the stored proof (bytes).
    pub size_bytes: u32,
}
#[cfg(test)]
mod tests {
    use super::*;
    use iroha_primitives::numeric::Numeric;
    use iroha_version::DecodeAll;
    #[test]
    fn bridge_proof_range_helpers_cover_valid_invalid_and_saturating_cases() {
        let valid = BridgeProofRange {
            start_height: 5,
            end_height: 7,
        };
        assert!(valid.is_valid());
        assert_eq!(valid.len(), 3);
        assert!(!valid.is_empty());
        let invalid = BridgeProofRange {
            start_height: 9,
            end_height: 4,
        };
        assert!(!invalid.is_valid());
        assert_eq!(invalid.len(), 1);
        assert!(!invalid.is_empty());
        let saturated = BridgeProofRange {
            start_height: u64::MAX,
            end_height: u64::MAX,
        };
        assert!(saturated.is_valid());
        assert_eq!(saturated.len(), 1);
    }
    #[test]
    fn bridge_proof_backend_label_matches_payload_kind() {
        let leaves = vec![[0xA1; 32], [0xB2; 32]];
        let tree = iroha_crypto::MerkleTree::<[u8; 32]>::from_hashed_leaves_sha256(leaves.clone());
        let root_bytes: [u8; 32] = *tree.root().expect("root").as_ref();
        let ics = BridgeProof {
            range: BridgeProofRange {
                start_height: 1,
                end_height: 1,
            },
            payload: BridgeProofPayload::Ics(BridgeIcsProof {
                verifier_manifest_hash: [0x11; 32],
                state_root: root_bytes,
                leaf_hash: leaves[0],
                proof: tree.get_proof(0).expect("proof"),
                hash_function: BridgeHashFunction::Sha256,
            }),
        };
        assert_eq!(ics.backend_label(), "bridge/ics23");
        let transparent = BridgeProof {
            range: BridgeProofRange {
                start_height: 2,
                end_height: 3,
            },
            payload: BridgeProofPayload::TransparentZk(BridgeTransparentProof {
                verifier_manifest_hash: [0x22; 32],
                proof: ProofBox::new("halo2/mock".into(), vec![0xDE, 0xAD, 0xBE, 0xEF]),
                recursion_depth: Some(2),
            }),
        };
        assert_eq!(transparent.backend_label(), "bridge/halo2/mock");
        for (payload, expected_index) in [(&ics.payload, 0_u32), (&transparent.payload, 1)] {
            let encoded = payload.encode();
            // Decode consumes a complete payload; the enum tag is only its fixed-width prefix.
            let tag_bytes = encoded
                .get(..4)
                .expect("bridge payload contains its variant tag")
                .try_into()
                .expect("four-byte variant tag");
            let decoded_index = u32::from_le_bytes(tag_bytes);
            assert_eq!(decoded_index, expected_index);
            assert_eq!(
                BridgeProofPayload::decode(&mut encoded.as_slice())
                    .expect("complete bridge payload decodes"),
                *payload
            );
        }
    }
    #[test]
    fn bridge_proof_binding_preserves_commitment_role() {
        let manifest_hash = [0x31; 32];
        let transparent = BridgeProof {
            range: BridgeProofRange {
                start_height: 1,
                end_height: 1,
            },
            payload: BridgeProofPayload::TransparentZk(BridgeTransparentProof {
                verifier_manifest_hash: manifest_hash,
                proof: ProofBox::new("halo2/mock".into(), vec![1]),
                recursion_depth: None,
            }),
        };
        assert_eq!(
            transparent.binding(),
            BridgeProofBinding::VerifierManifest(manifest_hash)
        );
        assert!(transparent.binding().is_well_formed());
        let mut bit_flipped = manifest_hash;
        bit_flipped[17] ^= 0x80;
        assert_ne!(
            transparent.binding(),
            BridgeProofBinding::VerifierManifest(bit_flipped)
        );
        assert!(!BridgeProofBinding::VerifierManifest([0; 32]).is_well_formed());
    }
    #[test]
    fn wrapped_asset_roundtrip() {
        let def = WrappedAssetDef {
            origin_chain: b"btc".to_vec(),
            origin_asset_id: b"btc:mainnet".to_vec(),
            bridge_id: b"btc->iroha".to_vec(),
        };
        let buf = def.encode();
        let dec = WrappedAssetDef::decode_all(&mut &buf[..]).expect("decode");
        assert_eq!(def, dec);
    }
    #[test]
    fn receipt_roundtrip() {
        let r = BridgeReceipt {
            lane: LaneId::from(1),
            direction: b"mint".to_vec(),
            source_tx: [0x11; 32],
            dest_tx: Some([0x22; 32]),
            proof_hash: [0x33; 32],
            amount: 42_u64.into(),
            asset_id: b"wBTC#btc".to_vec(),
            recipient: b"alice@main".to_vec(),
        };
        let buf = r.encode();
        let dec = BridgeReceipt::decode_all(&mut &buf[..]).expect("decode");
        assert_eq!(r, dec);
    }
    #[derive(Encode)]
    struct ForgedBridgeReceipt {
        lane: LaneId,
        direction: Vec<u8>,
        source_tx: [u8; 32],
        dest_tx: Option<[u8; 32]>,
        proof_hash: [u8; 32],
        amount: Numeric,
        asset_id: Vec<u8>,
        recipient: Vec<u8>,
    }
    #[test]
    fn bridge_receipt_rejects_negative_numeric_amount() {
        let forged = ForgedBridgeReceipt {
            lane: LaneId::from(1),
            direction: b"mint".to_vec(),
            source_tx: [0x11; 32],
            dest_tx: None,
            proof_hash: [0x33; 32],
            amount: Numeric::new(-1_i32, 0),
            asset_id: b"wBTC#btc".to_vec(),
            recipient: b"alice@main".to_vec(),
        };
        let encoded = forged.encode();
        assert!(
            BridgeReceipt::decode_all(&mut encoded.as_slice()).is_err(),
            "a negative signed payload must not decode as a bridge amount"
        );
    }

    #[test]
    fn bridge_receipt_json_rejects_unknown_fields() {
        let receipt = BridgeReceipt {
            lane: LaneId::from(1),
            direction: b"mint".to_vec(),
            source_tx: [0x11; 32],
            dest_tx: None,
            proof_hash: [0x33; 32],
            amount: 42_u64.into(),
            asset_id: b"wBTC#btc".to_vec(),
            recipient: b"alice@main".to_vec(),
        };
        let canonical = norito::json::to_json(&receipt).expect("serialize bridge receipt JSON");
        assert_eq!(
            norito::json::from_json::<BridgeReceipt>(&canonical)
                .expect("canonical bridge receipt JSON decodes"),
            receipt
        );
        let hostile = canonical.replacen('{', "{\"adversarial_extension\":null,", 1);
        assert_ne!(hostile, canonical);
        assert!(
            norito::json::from_json::<BridgeReceipt>(&hostile).is_err(),
            "signed receipt JSON must reject unknown fields"
        );
    }
    #[test]
    fn bridge_proof_roundtrip() {
        let leaves = vec![[0xAA; 32], [0xBB; 32]];
        let tree = iroha_crypto::MerkleTree::<[u8; 32]>::from_hashed_leaves_sha256(leaves.clone());
        let root_bytes: [u8; 32] = *tree.root().expect("root").as_ref();
        let proof = tree.get_proof(0).expect("proof");
        let proof = BridgeProof {
            range: BridgeProofRange {
                start_height: 1,
                end_height: 2,
            },
            payload: BridgeProofPayload::Ics(BridgeIcsProof {
                verifier_manifest_hash: [0x55; 32],
                state_root: root_bytes,
                leaf_hash: leaves[0],
                proof,
                hash_function: BridgeHashFunction::Sha256,
            }),
        };
        let buf = proof.encode();
        let dec = BridgeProof::decode_all(&mut &buf[..]).expect("decode");
        assert_eq!(proof, dec);
    }

    #[test]
    fn bridge_proof_json_rejects_unknown_fields_at_every_typed_boundary() {
        let leaves = vec![[0xAA; 32], [0xBB; 32]];
        let tree = iroha_crypto::MerkleTree::<[u8; 32]>::from_hashed_leaves_sha256(leaves.clone());
        let proof = BridgeProof {
            range: BridgeProofRange {
                start_height: 1,
                end_height: 1,
            },
            payload: BridgeProofPayload::Ics(BridgeIcsProof {
                verifier_manifest_hash: [0x55; 32],
                state_root: *tree.root().expect("root").as_ref(),
                leaf_hash: leaves[0],
                proof: tree.get_proof(0).expect("proof"),
                hash_function: BridgeHashFunction::Sha256,
            }),
        };
        let canonical = norito::json::to_json(&proof).expect("serialize bridge proof JSON");
        assert_eq!(
            norito::json::from_json::<BridgeProof>(&canonical).expect("canonical JSON decodes"),
            proof
        );
        let mut retired_pin_value =
            norito::json::to_value(&proof).expect("serialize bridge proof value");
        let norito::json::Value::Object(retired_pin_object) = &mut retired_pin_value else {
            panic!("bridge proof JSON must be an object")
        };
        retired_pin_object.insert("pinned".into(), norito::json::Value::Bool(true));
        let retired_pin =
            norito::json::to_json(&retired_pin_value).expect("serialize retired pin field");
        assert!(
            norito::json::from_json::<BridgeProof>(&retired_pin).is_err(),
            "retired caller-controlled retention hint must fail closed"
        );
        for path in [
            Vec::<&str>::new(),
            vec!["range"],
            vec!["payload"],
            vec!["payload", "payload"],
            vec!["payload", "payload", "hash_function"],
        ] {
            let mut hostile = norito::json::to_value(&proof).expect("serialize bridge proof value");
            let mut current = &mut hostile;
            for field in &path {
                let norito::json::Value::Object(object) = current else {
                    panic!("bridge proof JSON path component `{field}` is not an object")
                };
                current = object
                    .get_mut(*field)
                    .unwrap_or_else(|| panic!("bridge proof JSON path component `{field}` absent"));
            }
            let norito::json::Value::Object(object) = current else {
                panic!("bridge proof JSON target at {path:?} is not an object")
            };
            object.insert("adversarial_extension".into(), norito::json::Value::Null);
            let hostile_json =
                norito::json::to_json(&hostile).expect("serialize hostile bridge proof JSON");
            assert!(
                norito::json::from_json::<BridgeProof>(&hostile_json).is_err(),
                "unknown field at {path:?} must reject"
            );
        }
        let duplicate = canonical.replacen("\"range\":", "\"range\":null,\"range\":", 1);
        assert_ne!(duplicate, canonical);
        assert!(norito::json::from_json::<BridgeProof>(&duplicate).is_err());
    }
    #[test]
    fn bridge_proof_decoder_rejects_legacy_truncated_and_trailing_encodings() {
        #[derive(Encode)]
        struct LegacyBridgeProof {
            range: BridgeProofRange,
            manifest_hash: [u8; 32],
            payload: BridgeProofPayload,
            pinned: bool,
        }
        #[derive(Encode)]
        struct CallerPinnedBridgeProof {
            range: BridgeProofRange,
            payload: BridgeProofPayload,
            pinned: bool,
        }
        let proof = BridgeProof {
            range: BridgeProofRange {
                start_height: 5,
                end_height: 6,
            },
            payload: BridgeProofPayload::TransparentZk(BridgeTransparentProof {
                verifier_manifest_hash: [0x51; 32],
                proof: ProofBox::new("stark/mock".into(), vec![7, 8, 9]),
                recursion_depth: Some(3),
            }),
        };
        let canonical = proof.encode();
        assert_eq!(
            BridgeProof::decode_all(&mut canonical.as_slice())
                .expect("canonical bridge proof decodes"),
            proof.clone()
        );
        for end in 0..canonical.len() {
            let mut truncated: &[u8] = &canonical[..end];
            assert!(
                BridgeProof::decode_all(&mut truncated).is_err(),
                "truncated bridge proof unexpectedly decoded at byte {end}"
            );
        }
        let mut trailing = canonical;
        trailing.push(0);
        assert!(BridgeProof::decode_all(&mut trailing.as_slice()).is_err());
        let legacy = LegacyBridgeProof {
            range: proof.range,
            manifest_hash: [0xff; 32],
            payload: proof.payload.clone(),
            pinned: true,
        }
        .encode();
        assert!(BridgeProof::decode_all(&mut legacy.as_slice()).is_err());
        let caller_pinned = CallerPinnedBridgeProof {
            range: proof.range,
            payload: proof.payload,
            pinned: true,
        }
        .encode();
        assert!(
            BridgeProof::decode_all(&mut caller_pinned.as_slice()).is_err(),
            "retired caller-controlled retention field must fail binary decoding"
        );
    }
    #[test]
    fn bridge_proof_transparent_zk_roundtrip() {
        let proof = BridgeProof {
            range: BridgeProofRange {
                start_height: 8,
                end_height: 8,
            },
            payload: BridgeProofPayload::TransparentZk(BridgeTransparentProof {
                verifier_manifest_hash: [0x61; 32],
                proof: ProofBox::new("stark/mock".into(), vec![9, 8, 7, 6]),
                recursion_depth: Some(1),
            }),
        };
        let buf = proof.encode();
        let dec = BridgeProof::decode_all(&mut &buf[..]).expect("decode");
        assert_eq!(proof, dec);
    }
    #[test]
    fn bridge_proof_record_roundtrip() {
        let proof = BridgeProof {
            range: BridgeProofRange {
                start_height: 3,
                end_height: 4,
            },
            payload: BridgeProofPayload::TransparentZk(BridgeTransparentProof {
                verifier_manifest_hash: [0x71; 32],
                proof: ProofBox::new("groth16/mock".into(), vec![1, 2, 3, 4]),
                recursion_depth: None,
            }),
        };
        let record = BridgeProofRecord {
            proof,
            commitment: [0x81; 32],
            size_bytes: 4096,
        };
        let buf = record.encode();
        let dec = BridgeProofRecord::decode_all(&mut &buf[..]).expect("decode");
        assert_eq!(record, dec);
    }
}

#[cfg(test)]
mod captured_bridge_schema_tests;
