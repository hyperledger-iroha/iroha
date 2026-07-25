//! Portable, finality-bound active-receiver snapshot primitives.

use iroha_crypto::Hash;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

use super::KagemushaDevicePublicKeyV2;
use crate::{account::AccountId, asset::AssetDefinitionId};

/// Current active-receiver snapshot format.
pub const KAGEMUSHA_ACTIVE_RECEIVER_SNAPSHOT_VERSION_V1: u16 = 1;
/// Maximum active or ambiguous receiver tuples committed by one block.
pub const KAGEMUSHA_ACTIVE_RECEIVER_SNAPSHOT_MAX_LEAVES_V1: usize = 65_536;
/// Maximum depth of the canonical balanced receiver tree.
pub const KAGEMUSHA_ACTIVE_RECEIVER_SNAPSHOT_MAX_SIBLINGS_V1: usize = 16;
/// Exact depth of the execution-witness sparse tree.
pub const KAGEMUSHA_ACTIVE_RECEIVER_WITNESS_SIBLINGS_V1: usize = 256;
/// Fixed synthetic execution-witness key committed by every post-upgrade block.
pub const KAGEMUSHA_ACTIVE_RECEIVER_WITNESS_KEY_V1: &[u8] =
    b"\xd3iroha:kagemusha:active-receiver:v1";

const RECEIVER_LEAF_DOMAIN_V1: &[u8] = b"iroha:kagemusha:active-receiver:leaf:v1\0";
const RECEIVER_EMPTY_DOMAIN_V1: &[u8] = b"iroha:kagemusha:active-receiver:empty:v1\0";
const RECEIVER_NODE_DOMAIN_V1: &[u8] = b"iroha:kagemusha:active-receiver:node:v1\0";

/// Receiver identity used to select an active registration.
///
/// The P-256 key is intentionally excluded. A second current registration for
/// the same account/device/asset tuple therefore creates an ambiguous entry
/// instead of allowing the requester to select one of several keys.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[norito(deny_unknown_fields)]
pub struct KagemushaActiveReceiverKeyV1 {
    /// Recipient account.
    pub account_id: AccountId,
    /// Platform device identifier.
    pub device_id: String,
    /// Canonical offline-cash asset definition.
    pub asset_definition_id: AssetDefinitionId,
}

/// Consensus-derived value for one unique active native registration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[norito(deny_unknown_fields)]
pub struct KagemushaActiveReceiverValueV1 {
    /// Hash of the canonical public registration archive.
    pub registration_hash: Hash,
    /// Hash of the exact native registration-state archive selected at this block.
    pub registration_state_hash: Hash,
    /// Policy hash recorded by native admission.
    pub admission_policy_hash: Hash,
    /// Current policy hash used to derive this snapshot.
    pub current_policy_hash: Hash,
    /// Native admission block height.
    pub admission_height: u64,
    /// Hash of the admitting signed transaction, retained as audit provenance.
    pub admission_transaction_hash: Hash,
    /// Exact receiver P-256 key currently authorized for this tuple.
    pub public_key: KagemushaDevicePublicKeyV2,
    /// Registration validity limit in Unix milliseconds.
    pub expires_at_ms: u64,
    /// Consensus observed the canonical account at end of block.
    pub account_exists: bool,
    /// Consensus observed the canonical asset definition at end of block.
    pub asset_definition_exists: bool,
}

/// One receiver tuple committed by the canonical balanced snapshot tree.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[norito(deny_unknown_fields)]
pub struct KagemushaActiveReceiverActiveEntryV1 {
    /// Tuple key.
    pub key: KagemushaActiveReceiverKeyV1,
    /// Authenticated registration projection.
    pub value: KagemushaActiveReceiverValueV1,
}

/// Ambiguity marker for a tuple with multiple current native registrations.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[norito(deny_unknown_fields)]
pub struct KagemushaActiveReceiverAmbiguousEntryV1 {
    /// Tuple key.
    pub key: KagemushaActiveReceiverKeyV1,
    /// Number of conflicting active native records.
    pub candidate_count: u32,
    /// Hash of their canonically sorted state hashes.
    pub candidates_digest: Hash,
}

/// One receiver tuple committed by the canonical balanced snapshot tree.
#[expect(
    clippy::large_enum_variant,
    reason = "the public snapshot entry keeps its canonical Norito variant payloads inline"
)]
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[norito(
    tag = "kind",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum KagemushaActiveReceiverEntryV1 {
    /// Exactly one current, governed native registration exists.
    #[codec(index = 0)]
    Active(KagemushaActiveReceiverActiveEntryV1),
    /// More than one current registration owns the tuple; routing must fail closed.
    #[codec(index = 1)]
    Ambiguous(KagemushaActiveReceiverAmbiguousEntryV1),
}

impl KagemushaActiveReceiverEntryV1 {
    /// Return the tuple key shared by every entry variant.
    #[must_use]
    pub fn key(&self) -> &KagemushaActiveReceiverKeyV1 {
        match self {
            Self::Active(entry) => &entry.key,
            Self::Ambiguous(entry) => &entry.key,
        }
    }

    /// Return the active value, rejecting ambiguous entries.
    #[must_use]
    pub fn active_value(&self) -> Option<&KagemushaActiveReceiverValueV1> {
        match self {
            Self::Active(entry) => Some(&entry.value),
            Self::Ambiguous(_) => None,
        }
    }
}

/// Availability state bound into every per-block receiver commitment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[norito(
    tag = "status",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum KagemushaActiveReceiverSnapshotStatusV1 {
    /// A canonical governed policy was available for evaluation.
    #[codec(index = 0)]
    Available(Hash),
    /// Policy or protected state was unavailable/corrupt; no receiver may be routed.
    #[codec(index = 1)]
    Unavailable(Hash),
}

/// Exact value stored under the fixed synthetic execution-witness key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[norito(deny_unknown_fields)]
pub struct KagemushaActiveReceiverSnapshotCommitmentV1 {
    /// Snapshot format version.
    pub version: u16,
    /// Block whose end state was evaluated.
    pub evaluated_height: u64,
    /// Authenticated block creation time used for policy and expiry checks.
    pub evaluated_at_ms: u64,
    /// Whether the governed state was usable.
    pub status: KagemushaActiveReceiverSnapshotStatusV1,
    /// Number of real receiver entries (padding excluded).
    pub leaf_count: u32,
    /// Root of the canonical balanced receiver tree.
    pub tree_root: Hash,
}

/// Balanced-tree membership proof for one exact receiver entry.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[norito(deny_unknown_fields)]
pub struct KagemushaActiveReceiverMembershipProofV1 {
    /// Zero-based index in canonical key order.
    pub leaf_index: u32,
    /// Number of real leaves in the tree.
    pub leaf_count: u32,
    /// Siblings from leaf level to root.
    pub siblings: Vec<Hash>,
}

/// Sparse-SMT proof that the fixed snapshot commitment is an ordinary write.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[norito(deny_unknown_fields)]
pub struct KagemushaActiveReceiverWitnessProofV1 {
    /// Fixed raw execution-witness key.
    pub key: Vec<u8>,
    /// Exact canonical encoded snapshot commitment.
    pub value: Vec<u8>,
    /// Exactly 256 siblings from leaf level to the ordinary-write root.
    pub siblings: Vec<Hash>,
}

/// Complete derived snapshot used internally by consensus and proof serving.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KagemushaActiveReceiverSnapshotV1 {
    /// Fixed-key value committed by the execution witness.
    pub commitment: KagemushaActiveReceiverSnapshotCommitmentV1,
    /// Entries in canonical key order.
    pub entries: Vec<KagemushaActiveReceiverEntryV1>,
}

impl KagemushaActiveReceiverSnapshotV1 {
    /// Build and validate one canonical available snapshot.
    ///
    /// # Errors
    ///
    /// Returns an error for a zero height or policy hash, an oversized or
    /// duplicate entry set, an invalid entry, or a snapshot tree that cannot
    /// be encoded canonically.
    pub fn available(
        evaluated_height: u64,
        evaluated_at_ms: u64,
        policy_hash: Hash,
        mut entries: Vec<KagemushaActiveReceiverEntryV1>,
    ) -> Result<Self, String> {
        if evaluated_height == 0 || is_zero_hash(policy_hash) {
            return Err("active-receiver snapshot height and policy must be nonzero".into());
        }
        if entries.len() > KAGEMUSHA_ACTIVE_RECEIVER_SNAPSHOT_MAX_LEAVES_V1 {
            return Err("active-receiver snapshot exceeds its consensus leaf bound".into());
        }
        entries.sort_by(|left, right| left.key().cmp(right.key()));
        if entries
            .windows(2)
            .any(|pair| pair[0].key() == pair[1].key())
        {
            return Err("active-receiver snapshot contains duplicate tuple keys".into());
        }
        for entry in &entries {
            validate_entry(entry, policy_hash, evaluated_height, evaluated_at_ms)?;
        }
        let leaf_count = u32::try_from(entries.len())
            .map_err(|_| "active-receiver leaf count does not fit u32")?;
        let tree_root = receiver_tree_root(&entries)?;
        Ok(Self {
            commitment: KagemushaActiveReceiverSnapshotCommitmentV1 {
                version: KAGEMUSHA_ACTIVE_RECEIVER_SNAPSHOT_VERSION_V1,
                evaluated_height,
                evaluated_at_ms,
                status: KagemushaActiveReceiverSnapshotStatusV1::Available(policy_hash),
                leaf_count,
                tree_root,
            },
            entries,
        })
    }

    /// Build a deterministic fail-closed snapshot for unusable governed state.
    ///
    /// # Errors
    ///
    /// Returns an error when the evaluated height is zero or the failure
    /// reason is empty.
    pub fn unavailable(
        evaluated_height: u64,
        evaluated_at_ms: u64,
        reason: &[u8],
    ) -> Result<Self, String> {
        if evaluated_height == 0 || reason.is_empty() {
            return Err("unavailable active-receiver snapshot inputs are invalid".into());
        }
        Ok(Self {
            commitment: KagemushaActiveReceiverSnapshotCommitmentV1 {
                version: KAGEMUSHA_ACTIVE_RECEIVER_SNAPSHOT_VERSION_V1,
                evaluated_height,
                evaluated_at_ms,
                status: KagemushaActiveReceiverSnapshotStatusV1::Unavailable(Hash::new(reason)),
                leaf_count: 0,
                tree_root: receiver_empty_hash(),
            },
            entries: Vec::new(),
        })
    }

    /// Return one active entry and its exact canonical membership path.
    ///
    /// # Errors
    ///
    /// Returns an error when the snapshot is unavailable, the key is absent
    /// or ambiguous, or the entry or proof cannot be encoded canonically.
    pub fn active_membership(
        &self,
        key: &KagemushaActiveReceiverKeyV1,
    ) -> Result<
        (
            KagemushaActiveReceiverEntryV1,
            KagemushaActiveReceiverMembershipProofV1,
        ),
        String,
    > {
        if !matches!(
            self.commitment.status,
            KagemushaActiveReceiverSnapshotStatusV1::Available(_)
        ) {
            return Err("active-receiver snapshot is unavailable".into());
        }
        let index = self
            .entries
            .binary_search_by(|entry| entry.key().cmp(key))
            .map_err(|_| "receiver tuple is not active in the evaluated snapshot")?;
        if self.entries[index].active_value().is_none() {
            return Err("receiver tuple has multiple active registrations".into());
        }
        let proof = receiver_membership_proof(&self.entries, index)?;
        Ok((self.entries[index].clone(), proof))
    }
}

impl KagemushaActiveReceiverMembershipProofV1 {
    /// Verify one exact entry against a snapshot commitment.
    #[must_use]
    pub fn verify(
        &self,
        entry: &KagemushaActiveReceiverEntryV1,
        commitment: &KagemushaActiveReceiverSnapshotCommitmentV1,
    ) -> bool {
        if commitment.version != KAGEMUSHA_ACTIVE_RECEIVER_SNAPSHOT_VERSION_V1
            || self.leaf_count == 0
            || self.leaf_count != commitment.leaf_count
            || self.leaf_index >= self.leaf_count
            || usize::try_from(self.leaf_count)
                .ok()
                .is_none_or(|count| count > KAGEMUSHA_ACTIVE_RECEIVER_SNAPSHOT_MAX_LEAVES_V1)
        {
            return false;
        }
        let Ok(count) = usize::try_from(self.leaf_count) else {
            return false;
        };
        let expected_depth = count.next_power_of_two().trailing_zeros() as usize;
        if self.siblings.len() != expected_depth
            || self.siblings.len() > KAGEMUSHA_ACTIVE_RECEIVER_SNAPSHOT_MAX_SIBLINGS_V1
        {
            return false;
        }
        let Ok(mut current) = receiver_leaf_hash(entry) else {
            return false;
        };
        let mut index = self.leaf_index as usize;
        for (level, sibling) in self.siblings.iter().copied().enumerate() {
            let Ok(level) = u16::try_from(level) else {
                return false;
            };
            current = if index & 1 == 0 {
                receiver_node_hash(level, current, sibling)
            } else {
                receiver_node_hash(level, sibling, current)
            };
            index /= 2;
        }
        current == commitment.tree_root
    }
}

impl KagemushaActiveReceiverWitnessProofV1 {
    /// Verify the fixed synthetic write against an ordinary-write SMT root.
    #[must_use]
    pub fn verify(&self, expected_ordinary_writes_root: Hash) -> bool {
        if self.key != KAGEMUSHA_ACTIVE_RECEIVER_WITNESS_KEY_V1
            || self.siblings.len() != KAGEMUSHA_ACTIVE_RECEIVER_WITNESS_SIBLINGS_V1
        {
            return false;
        }
        let Ok(commitment) =
            norito::decode_from_bytes::<KagemushaActiveReceiverSnapshotCommitmentV1>(&self.value)
        else {
            return false;
        };
        if commitment.version != KAGEMUSHA_ACTIVE_RECEIVER_SNAPSHOT_VERSION_V1
            || norito::to_bytes(&commitment).ok().as_deref() != Some(self.value.as_slice())
        {
            return false;
        }
        let path = Hash::new(&self.key);
        let value_hash = Hash::new(&self.value);
        let mut leaf_preimage = Vec::with_capacity(1 + 2 * Hash::LENGTH);
        leaf_preimage.push(0);
        leaf_preimage.extend_from_slice(path.as_ref());
        leaf_preimage.extend_from_slice(value_hash.as_ref());
        let mut current = Hash::new(leaf_preimage);
        for (level, sibling) in self.siblings.iter().copied().enumerate() {
            let path_bit = 255_usize.saturating_sub(level);
            let byte = path.as_ref()[path_bit / 8];
            let right = byte & (1_u8 << (path_bit % 8)) != 0;
            current = if right {
                ordinary_smt_node_hash(sibling, current)
            } else {
                ordinary_smt_node_hash(current, sibling)
            };
        }
        current == expected_ordinary_writes_root
    }

    /// Decode and return the exact canonical snapshot commitment.
    ///
    /// # Errors
    ///
    /// Returns an error when the stored value is not a valid canonical Norito
    /// encoding of [`KagemushaActiveReceiverSnapshotCommitmentV1`].
    pub fn commitment(&self) -> Result<KagemushaActiveReceiverSnapshotCommitmentV1, String> {
        let commitment = norito::decode_from_bytes(&self.value)
            .map_err(|error| format!("active-receiver commitment is invalid: {error}"))?;
        if norito::to_bytes(&commitment).map_err(|error| error.to_string())? != self.value {
            return Err("active-receiver commitment is non-canonical".into());
        }
        Ok(commitment)
    }
}

fn validate_entry(
    entry: &KagemushaActiveReceiverEntryV1,
    policy_hash: Hash,
    evaluated_height: u64,
    evaluated_at_ms: u64,
) -> Result<(), String> {
    if entry.key().device_id.is_empty() {
        return Err("active-receiver device id must not be empty".into());
    }
    match entry {
        KagemushaActiveReceiverEntryV1::Active(entry) => {
            let value = &entry.value;
            if is_zero_hash(value.registration_hash)
                || is_zero_hash(value.registration_state_hash)
                || is_zero_hash(value.admission_policy_hash)
                || is_zero_hash(value.current_policy_hash)
                || is_zero_hash(value.admission_transaction_hash)
                || value.admission_policy_hash != policy_hash
                || value.current_policy_hash != policy_hash
                || value.admission_height == 0
                || value.admission_height > evaluated_height
                || value.expires_at_ms <= evaluated_at_ms
                || !value.account_exists
                || !value.asset_definition_exists
            {
                return Err("active-receiver value is inconsistent with its snapshot".into());
            }
        }
        KagemushaActiveReceiverEntryV1::Ambiguous(entry) => {
            if entry.candidate_count < 2 || is_zero_hash(entry.candidates_digest) {
                return Err("ambiguous receiver entry has invalid candidate provenance".into());
            }
        }
    }
    Ok(())
}

fn receiver_tree_root(entries: &[KagemushaActiveReceiverEntryV1]) -> Result<Hash, String> {
    if entries.is_empty() {
        return Ok(receiver_empty_hash());
    }
    let width = entries.len().next_power_of_two();
    let mut current = entries
        .iter()
        .map(receiver_leaf_hash)
        .collect::<Result<Vec<_>, _>>()?;
    current.resize(width, receiver_empty_hash());
    let depth = width.trailing_zeros() as usize;
    for level in 0..depth {
        let level = u16::try_from(level).map_err(|_| "receiver tree level does not fit u16")?;
        current = current
            .chunks_exact(2)
            .map(|pair| receiver_node_hash(level, pair[0], pair[1]))
            .collect();
    }
    current
        .first()
        .copied()
        .ok_or_else(|| "active-receiver tree has no root".into())
}

fn receiver_membership_proof(
    entries: &[KagemushaActiveReceiverEntryV1],
    leaf_index: usize,
) -> Result<KagemushaActiveReceiverMembershipProofV1, String> {
    if entries.is_empty() || leaf_index >= entries.len() {
        return Err("active-receiver membership index is out of range".into());
    }
    let width = entries.len().next_power_of_two();
    let depth = width.trailing_zeros() as usize;
    let mut current = entries
        .iter()
        .map(receiver_leaf_hash)
        .collect::<Result<Vec<_>, _>>()?;
    current.resize(width, receiver_empty_hash());
    let mut index = leaf_index;
    let mut siblings = Vec::with_capacity(depth);
    for level in 0..depth {
        siblings.push(current[index ^ 1]);
        let level = u16::try_from(level).map_err(|_| "receiver tree level does not fit u16")?;
        current = current
            .chunks_exact(2)
            .map(|pair| receiver_node_hash(level, pair[0], pair[1]))
            .collect();
        index /= 2;
    }
    Ok(KagemushaActiveReceiverMembershipProofV1 {
        leaf_index: u32::try_from(leaf_index)
            .map_err(|_| "active-receiver membership index does not fit u32")?,
        leaf_count: u32::try_from(entries.len())
            .map_err(|_| "active-receiver leaf count does not fit u32")?,
        siblings,
    })
}

fn receiver_leaf_hash(entry: &KagemushaActiveReceiverEntryV1) -> Result<Hash, String> {
    let encoded = norito::to_bytes(entry).map_err(|error| error.to_string())?;
    Ok(domain_hash(RECEIVER_LEAF_DOMAIN_V1, &[&encoded]))
}

fn receiver_empty_hash() -> Hash {
    domain_hash(RECEIVER_EMPTY_DOMAIN_V1, &[])
}

fn receiver_node_hash(level: u16, left: Hash, right: Hash) -> Hash {
    domain_hash(
        RECEIVER_NODE_DOMAIN_V1,
        &[&level.to_le_bytes(), left.as_ref(), right.as_ref()],
    )
}

fn ordinary_smt_node_hash(left: Hash, right: Hash) -> Hash {
    let mut preimage = Vec::with_capacity(1 + 2 * Hash::LENGTH);
    preimage.push(1);
    preimage.extend_from_slice(left.as_ref());
    preimage.extend_from_slice(right.as_ref());
    Hash::new(preimage)
}

fn domain_hash(domain: &[u8], chunks: &[&[u8]]) -> Hash {
    let capacity = domain.len() + chunks.iter().map(|chunk| chunk.len()).sum::<usize>();
    let mut preimage = Vec::with_capacity(capacity);
    preimage.extend_from_slice(domain);
    for chunk in chunks {
        preimage.extend_from_slice(chunk);
    }
    Hash::new(preimage)
}

fn is_zero_hash(hash: Hash) -> bool {
    hash.as_ref().iter().all(|byte| *byte == 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::AccountId;

    fn key(device: &str) -> KagemushaActiveReceiverKeyV1 {
        let (public_key, _) =
            iroha_crypto::KeyPair::try_from_seed(vec![0xDA; 32], iroha_crypto::Algorithm::Ed25519)
                .expect("receiver fixture seed derives a checked Ed25519 keypair")
                .into_parts();
        KagemushaActiveReceiverKeyV1 {
            account_id: AccountId::new(public_key),
            device_id: device.to_owned(),
            asset_definition_id: AssetDefinitionId::new(
                crate::domain::DomainId::try_new("wonderland", "universal").expect("asset domain"),
                "sbd".parse().expect("asset name"),
            ),
        }
    }

    fn value(seed: u8, policy: Hash) -> KagemushaActiveReceiverValueV1 {
        KagemushaActiveReceiverValueV1 {
            registration_hash: Hash::new([seed, 1]),
            registration_state_hash: Hash::new([seed, 2]),
            admission_policy_hash: policy,
            current_policy_hash: policy,
            admission_height: 1,
            admission_transaction_hash: Hash::new([seed, 3]),
            public_key: KagemushaDevicePublicKeyV2::from_sec1_bytes(&[
                0x04, 0x6b, 0x17, 0xd1, 0xf2, 0xe1, 0x2c, 0x42, 0x47, 0xf8, 0xbc, 0xe6, 0xe5, 0x63,
                0xa4, 0x40, 0xf2, 0x77, 0x03, 0x7d, 0x81, 0x2d, 0xeb, 0x33, 0xa0, 0xf4, 0xa1, 0x39,
                0x45, 0xd8, 0x98, 0xc2, 0x96, 0x4f, 0xe3, 0x42, 0xe2, 0xfe, 0x1a, 0x7f, 0x9b, 0x8e,
                0xe7, 0xeb, 0x4a, 0x7c, 0x0f, 0x9e, 0x16, 0x2b, 0xce, 0x33, 0x57, 0x6b, 0x31, 0x5e,
                0xce, 0xcb, 0xb6, 0x40, 0x68, 0x37, 0xbf, 0x51, 0xf5,
            ])
            .expect("P-256 key"),
            expires_at_ms: 10_000,
            account_exists: true,
            asset_definition_exists: true,
        }
    }

    #[test]
    fn membership_binds_canonical_order_and_rejects_tampering() {
        let policy = Hash::new(b"policy");
        let a = KagemushaActiveReceiverEntryV1::Active(KagemushaActiveReceiverActiveEntryV1 {
            key: key("a"),
            value: value(1, policy),
        });
        let b = KagemushaActiveReceiverEntryV1::Active(KagemushaActiveReceiverActiveEntryV1 {
            key: key("b"),
            value: value(2, policy),
        });
        let snapshot = KagemushaActiveReceiverSnapshotV1::available(
            7,
            1_000,
            policy,
            vec![b.clone(), a.clone()],
        )
        .expect("snapshot");
        assert_eq!(snapshot.entries[0].key().device_id, "a");
        let (entry, proof) = snapshot.active_membership(&key("b")).expect("proof");
        assert!(proof.verify(&entry, &snapshot.commitment));
        let mut tampered = proof.clone();
        tampered.siblings[0] = Hash::new(b"tampered");
        assert!(!tampered.verify(&entry, &snapshot.commitment));
    }

    #[test]
    fn duplicate_tuple_is_rejected_and_ambiguous_tuple_is_not_routable() {
        let policy = Hash::new(b"policy");
        let active = KagemushaActiveReceiverEntryV1::Active(KagemushaActiveReceiverActiveEntryV1 {
            key: key("same"),
            value: value(1, policy),
        });
        assert!(
            KagemushaActiveReceiverSnapshotV1::available(
                7,
                1_000,
                policy,
                vec![active.clone(), active],
            )
            .is_err()
        );
        let ambiguous =
            KagemushaActiveReceiverEntryV1::Ambiguous(KagemushaActiveReceiverAmbiguousEntryV1 {
                key: key("same"),
                candidate_count: 2,
                candidates_digest: Hash::new(b"two candidates"),
            });
        let snapshot =
            KagemushaActiveReceiverSnapshotV1::available(7, 1_000, policy, vec![ambiguous])
                .expect("ambiguous snapshot");
        assert!(snapshot.active_membership(&key("same")).is_err());
    }

    #[test]
    fn unix_epoch_block_time_is_a_valid_consensus_snapshot_time() {
        let policy = Hash::new(b"policy");
        let available = KagemushaActiveReceiverSnapshotV1::available(1, 0, policy, Vec::new())
            .expect("Unix-epoch block time is representable");
        assert_eq!(available.commitment.evaluated_at_ms, 0);
        let unavailable = KagemushaActiveReceiverSnapshotV1::unavailable(
            1,
            0,
            b"governed receiver policy unavailable",
        )
        .expect("fail-closed Unix-epoch snapshot is representable");
        assert_eq!(unavailable.commitment.evaluated_at_ms, 0);
    }
}
