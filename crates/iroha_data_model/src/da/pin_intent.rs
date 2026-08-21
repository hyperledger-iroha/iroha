#[cfg(feature = "json")]
use crate::{DeriveJsonDeserialize, DeriveJsonSerialize};
use crate::{
    da::{
        commitment::{DaCommitmentLocation, MerklePathItem},
        ingest::DaIngestAuthorizationV1,
        types::StorageTicketId,
    },
    nexus::LaneId,
    sorafs::pin_registry::ManifestDigest,
};
use core::cmp::Ordering;
use iroha_crypto::{Hash, HashOf};
use iroha_schema::IntoSchema;
use norito::{
    codec::{Decode, Encode},
    to_bytes,
};
/// Pin intent emitted by the DA ingest pipeline to seed the `SoraFS` registry.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema, Hash)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct DaPinIntent {
    /// Lane associated with the blob.
    pub lane_id: LaneId,
    /// Epoch the blob belongs to.
    pub epoch: u64,
    /// Monotonic sequence within the lane/epoch scope.
    pub sequence: u64,
    /// Storage ticket issued for this blob.
    pub storage_ticket: StorageTicketId,
    /// Canonical manifest digest (BLAKE3 over encoded `DaManifestV1`).
    pub manifest_hash: ManifestDigest,
    /// Optional alias to register in the `SoraFS` registry.
    #[norito(required)]
    pub alias: Option<String>,
    /// Consensus-verifiable account authorization and quota charge identity.
    ///
    /// This is a required V1 wire field. Pre-release owner-only pin intents do not
    /// decode and there is no unsigned sidecar representation.
    pub authorization: DaIngestAuthorizationV1,
}
impl DaPinIntent {
    /// Construct a fully authorised pin intent.
    #[must_use]
    pub fn new(
        lane_id: LaneId,
        epoch: u64,
        sequence: u64,
        storage_ticket: StorageTicketId,
        manifest_hash: ManifestDigest,
        authorization: DaIngestAuthorizationV1,
    ) -> Self {
        Self {
            lane_id,
            epoch,
            sequence,
            storage_ticket,
            manifest_hash,
            alias: None,
            authorization,
        }
    }
}
/// Bundle of pin intents embedded into a block payload.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct DaPinIntentBundle {
    /// Bundle layout version.
    pub version: u16,
    /// Ordered pin intent entries.
    pub intents: Vec<DaPinIntent>,
}
impl DaPinIntentBundle {
    /// Initial version identifier for on-chain pin intent bundles.
    pub const VERSION_V1: u16 = 1;
    /// Construct a bundle using the latest supported version.
    #[must_use]
    pub fn new(intents: Vec<DaPinIntent>) -> Self {
        let mut intents = intents;
        intents.sort_by_cached_key(|intent| {
            (
                intent.lane_id.as_u32(),
                intent.epoch,
                intent.sequence,
                *intent.storage_ticket.as_bytes(),
                *intent.manifest_hash.as_bytes(),
                intent.alias.clone(),
                to_bytes(&intent.authorization)
                    .expect("DA ingest authorization must have a canonical Norito encoding"),
            )
        });
        Self {
            version: Self::VERSION_V1,
            intents,
        }
    }
    /// Returns `true` if there are no pin intents in the bundle.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.intents.is_empty()
    }
    /// Canonical Merkle root over the bundled pin intents.
    ///
    /// Leaves and internal nodes use distinct, versioned hash domains. Odd leaves are promoted
    /// unchanged to the next layer instead of being duplicated.
    #[must_use]
    pub fn merkle_root(&self) -> Option<Hash> {
        if self.intents.is_empty() {
            return None;
        }
        let mut layer: Vec<Hash> = self.intents.iter().map(pin_intent_leaf_hash).collect();
        while layer.len() > 1 {
            let mut next = Vec::with_capacity(layer.len().div_ceil(2));
            let mut iter = layer.chunks(2);
            for pair in iter.by_ref() {
                let combined = if pair.len() == 1 {
                    pair[0]
                } else {
                    pin_intent_internal_hash(&pair[0], &pair[1])
                };
                next.push(combined);
            }
            layer = next;
        }
        layer.pop()
    }
    /// Header commitment to the V1 tree shape, leaf count, and Merkle root.
    #[must_use]
    pub fn merkle_commitment(&self) -> Option<HashOf<Self>> {
        let leaf_count = u32::try_from(self.intents.len()).ok()?;
        let root = self.merkle_root()?;
        Some(pin_intent_merkle_commitment(
            self.version,
            leaf_count,
            &root,
        ))
    }
}
impl Default for DaPinIntentBundle {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}
impl PartialOrd for DaPinIntentBundle {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for DaPinIntentBundle {
    fn cmp(&self, other: &Self) -> Ordering {
        self.version
            .cmp(&other.version)
            .then_with(|| self.intents.cmp(&other.intents))
    }
}
/// Pin intent annotated with its position inside a block payload.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct DaPinIntentWithLocation {
    /// Pin intent contents.
    pub intent: DaPinIntent,
    /// Block height and index of the intent within the bundled payload.
    pub location: DaCommitmentLocation,
}
/// Merkle membership proof for a DA pin intent inside a committed block bundle.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema, Hash)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct DaPinIntentProof {
    /// Pin intent covered by the proof.
    pub intent: DaPinIntent,
    /// Position of the intent inside the block bundle.
    pub location: DaCommitmentLocation,
    /// Header commitment to the tree version, leaf count, and Merkle root.
    pub bundle_hash: HashOf<DaPinIntentBundle>,
    /// Total number of intents in the bundle.
    pub bundle_len: u32,
    /// Merkle root derived from the ordered intent list.
    pub root: Hash,
    /// Merkle path connecting the intent leaf to `root`.
    pub path: Vec<MerklePathItem>,
}
const DA_PIN_INTENT_MERKLE_LEAF_DOMAIN_V1: &[u8] = b"iroha:da:pin-intent-merkle:leaf:v1\0";
const DA_PIN_INTENT_MERKLE_INTERNAL_DOMAIN_V1: &[u8] = b"iroha:da:pin-intent-merkle:internal:v1\0";
const DA_PIN_INTENT_MERKLE_COMMITMENT_DOMAIN_V1: &[u8] =
    b"iroha:da:pin-intent-merkle:commitment:v1\0";
/// Hash a pin intent into its domain-separated Merkle leaf value.
#[must_use]
pub fn pin_intent_leaf_hash(intent: &DaPinIntent) -> Hash {
    let encoded = to_bytes(intent).expect("DaPinIntent Norito encoding must succeed");
    let mut preimage =
        Vec::with_capacity(DA_PIN_INTENT_MERKLE_LEAF_DOMAIN_V1.len() + encoded.len());
    preimage.extend_from_slice(DA_PIN_INTENT_MERKLE_LEAF_DOMAIN_V1);
    preimage.extend_from_slice(&encoded);
    Hash::new(preimage)
}
/// Hash two child nodes into a domain-separated pin-intent Merkle node.
#[must_use]
pub fn pin_intent_internal_hash(left: &Hash, right: &Hash) -> Hash {
    let mut preimage =
        Vec::with_capacity(DA_PIN_INTENT_MERKLE_INTERNAL_DOMAIN_V1.len() + Hash::LENGTH * 2);
    preimage.extend_from_slice(DA_PIN_INTENT_MERKLE_INTERNAL_DOMAIN_V1);
    preimage.extend_from_slice(left.as_ref());
    preimage.extend_from_slice(right.as_ref());
    Hash::new(preimage)
}
/// Reconstruct the header commitment for a DA pin-intent Merkle tree.
#[must_use]
pub fn pin_intent_merkle_commitment(
    version: u16,
    leaf_count: u32,
    root: &Hash,
) -> HashOf<DaPinIntentBundle> {
    let mut preimage = Vec::with_capacity(
        DA_PIN_INTENT_MERKLE_COMMITMENT_DOMAIN_V1.len()
            + core::mem::size_of::<u16>()
            + core::mem::size_of::<u32>()
            + Hash::LENGTH,
    );
    preimage.extend_from_slice(DA_PIN_INTENT_MERKLE_COMMITMENT_DOMAIN_V1);
    preimage.extend_from_slice(&version.to_le_bytes());
    preimage.extend_from_slice(&leaf_count.to_le_bytes());
    preimage.extend_from_slice(root.as_ref());
    HashOf::from_untyped_unchecked(Hash::new(preimage))
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        NetworkId,
        account::AccountId,
        block::BlockHeader,
        da::{
            ingest::{DaIngestAuthorizationV1, DaIngestSignatureV1},
            types::{BlobDigest, StorageTicketId},
        },
        nexus::LaneId,
        sorafs::pin_registry::ManifestDigest,
    };
    use iroha_crypto::{Algorithm, HashOf, KeyPair, Signature};
    fn test_authorization(lane: LaneId, epoch: u64, sequence: u64) -> DaIngestAuthorizationV1 {
        let key_pair = KeyPair::try_from_seed(vec![0xD9; 32], Algorithm::Ed25519)
            .expect("valid deterministic pin-intent key");
        let mut authorization = DaIngestAuthorizationV1 {
            network_id: NetworkId::from_genesis_hash(
                HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0xDA; 32])),
            ),
            owner: AccountId::new(key_pair.public_key().clone()),
            lane_id: lane,
            epoch,
            sequence,
            payload_hash: BlobDigest::new([0xDB; 32]),
            payload_bytes: 1,
            request_content_hash: Hash::prehashed([0xDC; 32]),
            signatures: Vec::new(),
        };
        authorization.signatures.push(DaIngestSignatureV1 {
            signer: key_pair.public_key().clone(),
            signature: Signature::try_new(key_pair.private_key(), &authorization.signing_digest())
                .expect("sign deterministic pin-intent authorization"),
        });
        authorization
    }

    #[test]
    #[cfg(feature = "json")]
    fn pin_intent_json_requires_explicit_alias_and_rejects_unknown_fields() {
        let intent = DaPinIntent::new(
            LaneId::new(1),
            1,
            1,
            StorageTicketId::new([1; 32]),
            ManifestDigest::new([2; 32]),
            test_authorization(LaneId::new(1), 1, 1),
        );
        let mut missing = norito::json::to_value(&intent).expect("serialize DA pin intent");
        missing
            .as_object_mut()
            .expect("DA pin intent JSON object")
            .remove("alias");
        assert!(
            norito::json::from_value::<DaPinIntent>(missing).is_err(),
            "the first-release pin intent must require its nullable alias slot"
        );

        let mut unknown = norito::json::to_value(&intent).expect("serialize DA pin intent");
        unknown
            .as_object_mut()
            .expect("DA pin intent JSON object")
            .insert("pre_release_field".to_owned(), norito::json::Value::Null);
        assert!(
            norito::json::from_value::<DaPinIntent>(unknown).is_err(),
            "the first-release pin intent must reject unknown fields"
        );

        let bundle = DaPinIntentBundle::new(vec![intent]);
        let mut unknown_bundle =
            norito::json::to_value(&bundle).expect("serialize DA pin-intent bundle");
        unknown_bundle
            .as_object_mut()
            .expect("DA pin-intent bundle JSON object")
            .insert("pre_release_field".to_owned(), norito::json::Value::Null);
        assert!(
            norito::json::from_value::<DaPinIntentBundle>(unknown_bundle).is_err(),
            "the first-release pin-intent bundle must reject unknown fields"
        );
    }

    #[test]
    fn bundle_sorts_intents_deterministically() {
        let mut first = DaPinIntent::new(
            LaneId::new(2),
            2,
            1,
            StorageTicketId::new([2; 32]),
            ManifestDigest::new([2; 32]),
            test_authorization(LaneId::new(2), 2, 1),
        );
        first.alias = Some("z-alias".to_string());
        let mut second = DaPinIntent::new(
            LaneId::new(1),
            2,
            3,
            StorageTicketId::new([1; 32]),
            ManifestDigest::new([1; 32]),
            test_authorization(LaneId::new(1), 2, 3),
        );
        second.alias = Some("a-alias".to_string());
        let third = DaPinIntent::new(
            LaneId::new(1),
            1,
            9,
            StorageTicketId::new([3; 32]),
            ManifestDigest::new([3; 32]),
            test_authorization(LaneId::new(1), 1, 9),
        );
        let bundle = DaPinIntentBundle::new(vec![first.clone(), second.clone(), third.clone()]);
        let ordered: Vec<(u32, u64, u64, StorageTicketId)> = bundle
            .intents
            .iter()
            .map(|intent| {
                (
                    intent.lane_id.as_u32(),
                    intent.epoch,
                    intent.sequence,
                    intent.storage_ticket,
                )
            })
            .collect();
        assert_eq!(
            ordered,
            vec![
                (
                    third.lane_id.as_u32(),
                    third.epoch,
                    third.sequence,
                    third.storage_ticket
                ),
                (
                    second.lane_id.as_u32(),
                    second.epoch,
                    second.sequence,
                    second.storage_ticket
                ),
                (
                    first.lane_id.as_u32(),
                    first.epoch,
                    first.sequence,
                    first.storage_ticket
                )
            ]
        );
    }
    #[test]
    fn pin_intent_merkle_domains_are_disjoint() {
        let intent = DaPinIntent::new(
            LaneId::new(1),
            1,
            1,
            StorageTicketId::new([1; 32]),
            ManifestDigest::new([2; 32]),
            test_authorization(LaneId::new(1), 1, 1),
        );
        let encoded = to_bytes(&intent).expect("encode pin intent");
        assert_ne!(pin_intent_leaf_hash(&intent), Hash::new(encoded));
        let left = Hash::new(b"left");
        let right = Hash::new(b"right");
        let mut untagged = Vec::with_capacity(Hash::LENGTH * 2);
        untagged.extend_from_slice(left.as_ref());
        untagged.extend_from_slice(right.as_ref());
        assert_ne!(pin_intent_internal_hash(&left, &right), Hash::new(untagged));
    }
    #[test]
    fn pin_intent_merkle_commitment_binds_version_leaf_count_and_root() {
        let root = Hash::new(b"root");
        let baseline = pin_intent_merkle_commitment(DaPinIntentBundle::VERSION_V1, 3, &root);
        assert_ne!(
            baseline,
            pin_intent_merkle_commitment(DaPinIntentBundle::VERSION_V1 + 1, 3, &root)
        );
        assert_ne!(
            baseline,
            pin_intent_merkle_commitment(DaPinIntentBundle::VERSION_V1, 4, &root)
        );
        assert_ne!(
            baseline,
            pin_intent_merkle_commitment(
                DaPinIntentBundle::VERSION_V1,
                3,
                &Hash::new(b"other-root"),
            )
        );
        assert!(DaPinIntentBundle::default().merkle_commitment().is_none());
    }
}
