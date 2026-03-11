use core::{cmp::Ordering, convert::TryFrom};

use fastpq_isi::poseidon::{FIELD_MODULUS, PoseidonSponge};
use iroha_crypto::Hash;
use iroha_primitives::numeric::Numeric;
use iroha_schema::IntoSchema;
use norito::{
    codec::{Decode, Encode},
    to_bytes,
};
use thiserror::Error;

use super::{
    OFFLINE_PROOF_REQUEST_VERSION_V1, OfflinePlatformProof, OfflineProofRequestCounter,
    OfflineProofRequestReplay, OfflineProofRequestSum, OfflineSpendReceipt,
};
use crate::metadata::Metadata;

const DOMAIN_TAG: &[u8] = b"iroha.offline.receipt.merkle.v1";
const PACKED_LIMB_BYTES: usize = 7;
const FIELD_MODULUS_U128: u128 = FIELD_MODULUS as u128;

/// Current version for aggregate proof envelopes.
pub const AGGREGATE_PROOF_VERSION_V1: u16 = 1;
/// Version for recursive transparent aggregate proof envelopes.
pub const AGGREGATE_PROOF_VERSION_V2: u16 = 2;
/// Metadata key recording the FASTPQ parameter set name.
pub const AGGREGATE_PROOF_METADATA_PARAMETER_SET: &str = "fastpq.parameter_set";
/// Metadata key recording the sum circuit identifier.
pub const AGGREGATE_PROOF_METADATA_SUM_CIRCUIT: &str = "fastpq.circuit.sum";
/// Metadata key recording the counter circuit identifier.
pub const AGGREGATE_PROOF_METADATA_COUNTER_CIRCUIT: &str = "fastpq.circuit.counter";
/// Metadata key recording the replay circuit identifier.
pub const AGGREGATE_PROOF_METADATA_REPLAY_CIRCUIT: &str = "fastpq.circuit.replay";
/// Metadata key recording the aggregate backend for recursive v2 proofs.
pub const AGGREGATE_PROOF_METADATA_OFFLINE_AGGREGATE_BACKEND: &str = "offline.aggregate.backend";
/// Metadata key recording the aggregate recursive circuit identifier.
pub const AGGREGATE_PROOF_METADATA_OFFLINE_AGGREGATE_CIRCUIT_ID: &str =
    "offline.aggregate.circuit_id";
/// Metadata key recording canonical aggregate public inputs (base64-encoded).
pub const AGGREGATE_PROOF_METADATA_OFFLINE_AGGREGATE_PUBLIC_INPUTS_B64: &str =
    "offline.aggregate.public_inputs_b64";
/// Metadata key recording recursive fold depth for aggregate v2 proofs.
pub const AGGREGATE_PROOF_METADATA_OFFLINE_AGGREGATE_RECURSION_DEPTH: &str =
    "offline.aggregate.recursion_depth";
/// Version of the offline FASTPQ aggregate proof payloads.
pub const OFFLINE_FASTPQ_PROOF_VERSION_V1: u16 = 1;
/// Domain tag for deterministic FASTPQ sum proofs.
pub const OFFLINE_FASTPQ_SUM_PROOF_DOMAIN: &[u8] = b"iroha.offline.fastpq.sum.v1";
/// Domain tag for deterministic FASTPQ sum-proof nonce derivation.
pub const OFFLINE_FASTPQ_SUM_NONCE_DOMAIN: &[u8] = b"iroha.offline.fastpq.sum.nonce.v1";
/// Domain tag for deterministic FASTPQ counter proofs.
pub const OFFLINE_FASTPQ_COUNTER_PROOF_DOMAIN: &[u8] = b"iroha.offline.fastpq.counter.v1";
/// Domain tag for deterministic FASTPQ replay proofs.
pub const OFFLINE_FASTPQ_REPLAY_PROOF_DOMAIN: &[u8] = b"iroha.offline.fastpq.replay.v1";
/// Domain tag for deterministic FASTPQ replay log chaining.
pub const OFFLINE_FASTPQ_REPLAY_CHAIN_DOMAIN: &[u8] = b"iroha.offline.fastpq.replay.chain.v1";

/// Digest produced by the Poseidon receipt tree.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct PoseidonDigest {
    bytes: [u8; Hash::LENGTH],
}

impl PoseidonDigest {
    /// Digest representing the zero field element.
    #[must_use]
    pub const fn zero() -> Self {
        Self {
            bytes: [0u8; Hash::LENGTH],
        }
    }

    /// Construct a digest from a field element.
    #[must_use]
    pub fn from_field(element: u64) -> Self {
        let mut bytes = [0u8; Hash::LENGTH];
        bytes[Hash::LENGTH - 8..].copy_from_slice(&element.to_be_bytes());
        Self { bytes }
    }

    /// Return the raw digest bytes (big-endian field encoding).
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; Hash::LENGTH] {
        &self.bytes
    }

    /// Convert to an uppercase hex string.
    #[must_use]
    pub fn to_hex_upper(&self) -> String {
        hex::encode_upper(self.as_bytes())
    }

    /// Parse a digest from a hex string.
    ///
    /// # Errors
    ///
    /// Returns [`OfflineReceiptMerkleError::InvalidHex`] if `hex_str` cannot be decoded
    /// or does not contain exactly [`Hash::LENGTH`] bytes.
    pub fn from_hex(hex_str: &str) -> Result<Self, OfflineReceiptMerkleError> {
        let mut bytes = [0u8; Hash::LENGTH];
        let decoded = hex::decode(hex_str)
            .map_err(|err| OfflineReceiptMerkleError::InvalidHex(err.to_string()))?;
        if decoded.len() != Hash::LENGTH {
            return Err(OfflineReceiptMerkleError::InvalidHex(format!(
                "expected {} bytes (got {})",
                Hash::LENGTH,
                decoded.len()
            )));
        }
        bytes.copy_from_slice(&decoded);
        Ok(Self { bytes })
    }
}

/// Aggregate proof payload attached to `OfflineToOnlineTransfer` bundles.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct AggregateProofEnvelope {
    /// Version of the aggregate-proof format.
    pub version: u16,
    /// Poseidon Merkle root covering all receipts.
    pub receipts_root: PoseidonDigest,
    /// Chaum–Pedersen/Poseidon proof attesting to the commitment delta.
    #[norito(default)]
    pub proof_sum: Option<Vec<u8>>,
    /// Proof that counters advanced contiguously.
    #[norito(default)]
    pub proof_counter: Option<Vec<u8>>,
    /// Proof that receiver anti-replay logs advanced correctly.
    #[norito(default)]
    pub proof_replay: Option<Vec<u8>>,
    /// Reserved metadata for future options (e.g., circuit selectors).
    #[norito(default)]
    pub metadata: Metadata,
}

/// Deterministic sum proof attached to an offline aggregate proof envelope.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct OfflineFastpqSumProof {
    /// Proof format version.
    pub version: u16,
    /// Poseidon Merkle root for the receipts covered by the proof.
    pub receipts_root: PoseidonDigest,
    /// Compressed Ristretto point `R` for the Schnorr proof.
    pub r_point: [u8; 32],
    /// Scalar `s` for the Schnorr proof.
    pub s_scalar: [u8; 32],
}

/// Deterministic counter-contiguity proof attached to an offline aggregate proof envelope.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct OfflineFastpqCounterProof {
    /// Proof format version.
    pub version: u16,
    /// Poseidon Merkle root for the receipts covered by the proof.
    pub receipts_root: PoseidonDigest,
    /// Counter checkpoint used when building the proof.
    pub counter_checkpoint: u64,
    /// Digest over the checkpoint + counters sequence.
    pub digest: Hash,
}

/// Deterministic replay-log proof attached to an offline aggregate proof envelope.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct OfflineFastpqReplayProof {
    /// Proof format version.
    pub version: u16,
    /// Poseidon Merkle root for the receipts covered by the proof.
    pub receipts_root: PoseidonDigest,
    /// Replay log head used when hashing the receipt tx ids.
    pub replay_log_head: Hash,
    /// Replay log tail expected after hashing all tx ids.
    pub replay_log_tail: Hash,
    /// Digest over the replay chain inputs.
    pub digest: Hash,
}

impl Default for AggregateProofEnvelope {
    fn default() -> Self {
        Self {
            version: AGGREGATE_PROOF_VERSION_V1,
            receipts_root: PoseidonDigest::zero(),
            proof_sum: None,
            proof_counter: None,
            proof_replay: None,
            metadata: Metadata::default(),
        }
    }
}

impl AggregateProofEnvelope {
    /// Build an aggregate proof envelope from receipts and optional proof bytes.
    ///
    /// # Errors
    ///
    /// Returns [`OfflineReceiptMerkleError`] if receipts cannot be hashed.
    pub fn from_receipts(
        receipts: &[OfflineSpendReceipt],
        proof_sum: Option<Vec<u8>>,
        proof_counter: Option<Vec<u8>>,
        proof_replay: Option<Vec<u8>>,
        metadata: Metadata,
    ) -> Result<Self, OfflineReceiptMerkleError> {
        let receipts_root = compute_receipts_root(receipts)?;
        Ok(Self {
            version: AGGREGATE_PROOF_VERSION_V1,
            receipts_root,
            proof_sum,
            proof_counter,
            proof_replay,
            metadata,
        })
    }

    /// Build an aggregate proof envelope from FASTPQ witness requests.
    ///
    /// # Errors
    ///
    /// Returns [`AggregateProofEnvelopeError`] if the proof requests disagree.
    pub fn from_proof_requests(
        sum: Option<&OfflineProofRequestSum>,
        counter: Option<&OfflineProofRequestCounter>,
        replay: Option<&OfflineProofRequestReplay>,
        proof_sum: Option<Vec<u8>>,
        proof_counter: Option<Vec<u8>>,
        proof_replay: Option<Vec<u8>>,
        metadata: Metadata,
    ) -> Result<Self, AggregateProofEnvelopeError> {
        let header = merge_request_headers(sum, counter, replay)?;
        if header.version != OFFLINE_PROOF_REQUEST_VERSION_V1 {
            return Err(AggregateProofEnvelopeError::UnsupportedRequestVersion {
                version: header.version,
            });
        }
        Ok(Self {
            version: AGGREGATE_PROOF_VERSION_V1,
            receipts_root: header.receipts_root,
            proof_sum,
            proof_counter,
            proof_replay,
            metadata,
        })
    }
}

/// Canonical leaf inputs for the receipt Poseidon tree.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct OfflineReceiptLeaf {
    /// Sender transaction identifier hash.
    pub tx_id: Hash,
    /// Amount transferred in the receipt.
    pub amount: Numeric,
    /// Hardware counter captured in the receipt.
    pub counter: u64,
    /// Poseidon-ready hash of the receiver account.
    pub receiver_hash: Hash,
    /// Hash of the invoice identifier supplied by the payee.
    pub invoice_hash: Hash,
    /// Hash of the platform proof (App Attest / `KeyMint` / Provisioned manifest).
    pub platform_proof_hash: Hash,
}

impl OfflineReceiptLeaf {
    /// Build a leaf from an `OfflineSpendReceipt`.
    ///
    /// # Errors
    ///
    /// Returns [`OfflineReceiptMerkleError::Serialization`] if hashing the receipt payload
    /// fails during Norito serialization.
    pub fn from_receipt(receipt: &OfflineSpendReceipt) -> Result<Self, OfflineReceiptMerkleError> {
        let receiver_bytes = to_bytes(&receipt.to)?;
        let receiver_hash = Hash::new(receiver_bytes);
        let invoice_hash = Hash::new(receipt.invoice_id.as_bytes());
        let platform_bytes = to_bytes(&receipt.platform_proof)?;
        let platform_proof_hash = Hash::new(platform_bytes);
        Ok(Self {
            tx_id: receipt.tx_id,
            amount: receipt.amount.clone(),
            counter: receipt.platform_proof.counter(),
            receiver_hash,
            invoice_hash,
            platform_proof_hash,
        })
    }

    /// Compute the Poseidon field element for this leaf.
    pub fn digest_field(&self) -> u64 {
        let mut sponge = PoseidonSponge::new();
        absorb_tag(&mut sponge, DOMAIN_TAG);
        sponge.absorb(hash_to_field(&self.tx_id));
        sponge.absorb(numeric_to_field(&self.amount));
        sponge.absorb(scale_to_field(&self.amount));
        sponge.absorb(reduce_u64(self.counter));
        sponge.absorb(hash_to_field(&self.receiver_hash));
        sponge.absorb(hash_to_field(&self.invoice_hash));
        sponge.absorb(hash_to_field(&self.platform_proof_hash));
        sponge.squeeze()
    }
}

/// Deterministic builder for Poseidon receipt Merkle roots.
#[derive(Debug, Default, Clone)]
pub struct OfflineReceiptMerkleBuilder {
    leaves: Vec<LeafEntry>,
}

impl OfflineReceiptMerkleBuilder {
    /// Create an empty builder.
    pub fn new() -> Self {
        Self { leaves: Vec::new() }
    }

    /// Push a receipt directly and compute its leaf hash in-place.
    ///
    /// # Errors
    ///
    /// Returns [`OfflineReceiptMerkleError::Serialization`] if the receipt cannot be
    /// converted into a Poseidon-ready leaf.
    pub fn push_receipt(
        &mut self,
        receipt: &OfflineSpendReceipt,
    ) -> Result<(), OfflineReceiptMerkleError> {
        let leaf = OfflineReceiptLeaf::from_receipt(receipt)?;
        self.push_leaf(&leaf);
        Ok(())
    }

    /// Push a precomputed leaf.
    pub fn push_leaf(&mut self, leaf: &OfflineReceiptLeaf) {
        let digest = leaf.digest_field();
        self.leaves.push(LeafEntry {
            counter: leaf.counter,
            tx_id: leaf.tx_id,
            digest,
        });
    }

    /// Finalise the builder and produce the Poseidon root.
    pub fn finalize(mut self) -> PoseidonDigest {
        if self.leaves.is_empty() {
            return PoseidonDigest::zero();
        }
        self.leaves.sort_by(|a, b| match a.counter.cmp(&b.counter) {
            Ordering::Equal => a.tx_id.cmp(&b.tx_id),
            other => other,
        });
        let mut level: Vec<u64> = self.leaves.into_iter().map(|entry| entry.digest).collect();
        while level.len() > 1 {
            if level.len() % 2 == 1 {
                level.push(0);
            }
            let mut next = Vec::with_capacity(level.len() / 2);
            for chunk in level.chunks_exact(2) {
                next.push(hash_node(chunk[0], chunk[1]));
            }
            level = next;
        }
        PoseidonDigest::from_field(level[0])
    }
}

/// Convenience helper that hashes an ordered slice of receipts into a Poseidon root.
///
/// # Errors
///
/// Returns [`OfflineReceiptMerkleError::Serialization`] if any receipt fails to convert
/// into a Poseidon leaf.
pub fn compute_receipts_root(
    receipts: &[OfflineSpendReceipt],
) -> Result<PoseidonDigest, OfflineReceiptMerkleError> {
    let mut builder = OfflineReceiptMerkleBuilder::new();
    for receipt in receipts {
        builder.push_receipt(receipt)?;
    }
    Ok(builder.finalize())
}

impl OfflinePlatformProof {
    /// Extract the monotonic counter value carried by this proof.
    pub fn counter(&self) -> u64 {
        match self {
            Self::AppleAppAttest(proof) => proof.counter,
            Self::AndroidMarkerKey(proof) => proof.counter,
            Self::Provisioned(proof) => proof.counter,
        }
    }
}

/// Errors that may occur when hashing receipts into the Poseidon tree.
#[derive(Debug, Error)]
pub enum OfflineReceiptMerkleError {
    /// Norito serialization failed while hashing a field.
    #[error("failed to serialize Norito payload: {0}")]
    Serialization(#[from] norito::Error),
    /// Hex input could not be parsed.
    #[error("invalid hex: {0}")]
    InvalidHex(String),
}

/// Errors returned while assembling aggregate proof envelopes.
#[derive(Debug, Copy, Clone, Error)]
pub enum AggregateProofEnvelopeError {
    /// No proof requests were provided to infer the receipts root.
    #[error("aggregate proof requires at least one proof request header")]
    MissingRequestHeader,
    /// The request headers do not match.
    #[error("proof request headers disagree")]
    HeaderMismatch,
    /// The proof request version is unsupported.
    #[error("unsupported proof request version {version}")]
    UnsupportedRequestVersion {
        /// Unsupported version tag.
        version: u16,
    },
}

#[derive(Debug, Copy, Clone)]
struct LeafEntry {
    counter: u64,
    tx_id: Hash,
    digest: u64,
}

fn hash_to_field(hash: &Hash) -> u64 {
    reduce_bytes(hash.as_ref())
}

fn numeric_to_field(value: &Numeric) -> u64 {
    let mantissa = value
        .try_mantissa_u128()
        .expect("mantissa must fit into u128 for poseidon reduction");
    reduce_u128(mantissa)
}

fn scale_to_field(value: &Numeric) -> u64 {
    reduce_u64(u64::from(value.scale()))
}

fn reduce_bytes(bytes: &[u8]) -> u64 {
    let mut acc = 0u128;
    for byte in bytes {
        acc = ((acc << 8) | u128::from(*byte)) % FIELD_MODULUS_U128;
    }
    u64::try_from(acc).expect("field element must fit into 64 bits")
}

fn reduce_u128(value: u128) -> u64 {
    (value % FIELD_MODULUS_U128) as u64
}

fn reduce_u64(value: u64) -> u64 {
    let modulus = FIELD_MODULUS;
    if value >= modulus {
        value - modulus
    } else {
        value
    }
}

fn merge_request_headers(
    sum: Option<&OfflineProofRequestSum>,
    counter: Option<&OfflineProofRequestCounter>,
    replay: Option<&OfflineProofRequestReplay>,
) -> Result<super::OfflineProofRequestHeader, AggregateProofEnvelopeError> {
    let mut header: Option<super::OfflineProofRequestHeader> = None;
    for request in [
        sum.map(|req| &req.header),
        counter.map(|req| &req.header),
        replay.map(|req| &req.header),
    ]
    .into_iter()
    .flatten()
    {
        if let Some(existing) = &header {
            if existing != request {
                return Err(AggregateProofEnvelopeError::HeaderMismatch);
            }
        } else {
            header = Some(request.clone());
        }
    }
    header.ok_or(AggregateProofEnvelopeError::MissingRequestHeader)
}

fn hash_node(left: u64, right: u64) -> u64 {
    let mut sponge = PoseidonSponge::new();
    absorb_tag(&mut sponge, DOMAIN_TAG);
    sponge.absorb(left);
    sponge.absorb(right);
    sponge.squeeze()
}

fn absorb_tag(sponge: &mut PoseidonSponge, tag: &[u8]) {
    sponge.absorb((tag.len() as u64) % FIELD_MODULUS);
    for limb in pack_bytes(tag) {
        sponge.absorb(limb);
    }
}

fn pack_bytes(bytes: &[u8]) -> Vec<u64> {
    if bytes.is_empty() {
        return Vec::new();
    }
    let mut limbs = Vec::with_capacity(bytes.len().div_ceil(PACKED_LIMB_BYTES));
    let mut offset = 0usize;
    while offset < bytes.len() {
        let remaining = bytes.len() - offset;
        let take = remaining.min(PACKED_LIMB_BYTES);
        let mut chunk = [0u8; 8];
        chunk[..take].copy_from_slice(&bytes[offset..offset + take]);
        let limb = u64::from_le_bytes(chunk);
        debug_assert!(limb < FIELD_MODULUS);
        limbs.push(limb);
        offset += take;
    }
    limbs
}

#[cfg(test)]
mod tests {
    use std::{fs, path::Path, str::FromStr};

    use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
    use iroha_crypto::{Algorithm, KeyPair, Signature};
    use iroha_primitives::json::Json;
    use norito::json::Value;

    use super::*;
    use crate::{
        account::AccountId,
        asset::{AssetDefinitionId, AssetId},
        domain::DomainId,
        metadata::Metadata,
        name::Name,
        offline::{
            AppleAppAttestProof, OfflineAllowanceCommitment, OfflineProofBlindingSeed,
            OfflineProofRequestCounter, OfflineProofRequestHeader, OfflineProofRequestSum,
            OfflineWalletCertificate, OfflineWalletPolicy,
        },
    };

    fn sample_account() -> AccountId {
        let _domain: DomainId = "wonderland".parse().expect("domain");
        let key_pair = KeyPair::from_seed(vec![0x01; 32], Algorithm::Ed25519);
        AccountId::new(key_pair.public_key().clone())
    }

    fn sample_asset(owner: &AccountId) -> AssetId {
        let definition: AssetDefinitionId = "usd#wonderland".parse().expect("definition");
        AssetId::new(definition, owner.clone())
    }

    fn sample_certificate(owner: &AccountId, asset: &AssetId) -> OfflineWalletCertificate {
        let spend_key = KeyPair::from_seed(vec![0x02; 32], Algorithm::Ed25519);
        let operator = KeyPair::from_seed(vec![0x03; 32], Algorithm::Ed25519);
        let operator_account = AccountId::new(operator.public_key().clone());
        OfflineWalletCertificate {
            controller: owner.clone(),
            operator: operator_account,
            allowance: OfflineAllowanceCommitment {
                asset: asset.clone(),
                amount: Numeric::new(1_000, 0),
                commitment: vec![0u8; 32],
            },
            spend_public_key: spend_key.public_key().clone(),
            attestation_report: vec![],
            issued_at_ms: 0,
            expires_at_ms: 1,
            policy: OfflineWalletPolicy {
                max_balance: Numeric::new(10_000, 0),
                max_tx_value: Numeric::new(1_000, 0),
                expires_at_ms: 1,
            },
            operator_signature: Signature::new(operator.private_key(), b"certificate"),
            metadata: Metadata::default(),
            verdict_id: None,
            attestation_nonce: None,
            refresh_at_ms: None,
        }
    }

    fn sample_receipt(counter: u64, seed: &str) -> OfflineSpendReceipt {
        let account = sample_account();
        let asset = sample_asset(&account);
        let cert = sample_certificate(&account, &asset);
        let certificate_id = cert.certificate_id();
        let key_pair = KeyPair::from_seed(vec![0xAA; 32], Algorithm::Ed25519);
        OfflineSpendReceipt {
            tx_id: Hash::new(format!("tx-{seed}").as_bytes()),
            from: account.clone(),
            to: account.clone(),
            asset,
            amount: Numeric::new(250, 0),
            issued_at_ms: 1,
            invoice_id: format!("invoice-{seed}"),
            platform_proof: OfflinePlatformProof::AppleAppAttest(AppleAppAttestProof {
                key_id: seed.to_string(),
                counter,
                assertion: vec![1, 2, 3],
                challenge_hash: Hash::new(b"challenge"),
            }),
            platform_snapshot: None,
            sender_certificate_id: certificate_id,
            sender_signature: Signature::new(key_pair.private_key(), b"receipt"),
            build_claim: None,
        }
    }

    fn fixture_certificate(owner: &AccountId, asset: &AssetId) -> OfflineWalletCertificate {
        let spend_key = KeyPair::from_seed(vec![0x10; 32], Algorithm::Ed25519);
        let operator = KeyPair::from_seed(vec![0x11; 32], Algorithm::Ed25519);
        let operator_account = AccountId::new(operator.public_key().clone());
        OfflineWalletCertificate {
            controller: owner.clone(),
            operator: operator_account,
            allowance: OfflineAllowanceCommitment {
                asset: asset.clone(),
                amount: Numeric::new(1_000, 0),
                commitment: vec![0u8; 32],
            },
            spend_public_key: spend_key.public_key().clone(),
            attestation_report: vec![1, 2, 3],
            issued_at_ms: 1,
            expires_at_ms: 2,
            policy: OfflineWalletPolicy {
                max_balance: Numeric::new(10_000, 0),
                max_tx_value: Numeric::new(1_000, 0),
                expires_at_ms: 2,
            },
            operator_signature: Signature::new(operator.private_key(), b"certificate"),
            metadata: Metadata::default(),
            verdict_id: None,
            attestation_nonce: None,
            refresh_at_ms: None,
        }
    }

    fn hash_from_hex(hex: &str) -> Hash {
        let bytes = hex::decode(hex).expect("valid hex");
        let mut array = [0u8; Hash::LENGTH];
        array.copy_from_slice(&bytes);
        Hash::prehashed(array)
    }

    fn metadata_from_fixture(value: &Value) -> Metadata {
        let obj = value.as_object().expect("metadata object");
        let mut metadata = Metadata::default();
        for (key, entry) in obj {
            let name = Name::from_str(key).expect("metadata key");
            let value = entry
                .as_str()
                .unwrap_or_else(|| panic!("metadata value `{key}` must be a string"));
            metadata.insert(name, Json::new(value));
        }
        metadata
    }

    fn fixture_account(value: &str) -> AccountId {
        match AccountId::parse_encoded(value) {
            Ok(parsed) => parsed.into_account_id(),
            Err(err) => panic!(
                "fixture account `{value}` must be an encoded account literal (I105 or `sora...`): {err}"
            ),
        }
    }

    fn fixture_asset(value: &str) -> AssetId {
        AssetId::parse_encoded(value).unwrap_or_else(|err| {
            panic!(
                "fixture asset `{value}` must be an encoded asset literal (`norito:<hex>`): {err}"
            )
        })
    }

    fn fixture_receipt(
        entry: &Value,
        certificate: &OfflineWalletCertificate,
    ) -> OfflineSpendReceipt {
        let certificate_id = certificate.certificate_id();
        let obj = entry.as_object().expect("receipt object");
        let from = obj
            .get("from")
            .and_then(Value::as_str)
            .map(fixture_account)
            .expect("receipt from");
        let to = obj
            .get("to")
            .and_then(Value::as_str)
            .map(fixture_account)
            .expect("receipt to");
        let asset = obj
            .get("asset")
            .and_then(Value::as_str)
            .map(fixture_asset)
            .expect("receipt asset");
        let amount = obj
            .get("amount")
            .and_then(Value::as_str)
            .and_then(|value| Numeric::from_str(value).ok())
            .expect("receipt amount");
        let issued_at_ms = obj
            .get("issued_at_ms")
            .and_then(Value::as_u64)
            .expect("receipt issued_at_ms");
        let invoice_id = obj
            .get("invoice_id")
            .and_then(Value::as_str)
            .expect("receipt invoice")
            .to_string();
        let tx_id_hex = obj
            .get("tx_id_hex")
            .and_then(Value::as_str)
            .expect("receipt tx_id_hex");
        let tx_id = hash_from_hex(tx_id_hex);

        let proof_obj = obj
            .get("platform_proof")
            .and_then(Value::as_object)
            .expect("platform_proof");
        let kind = proof_obj
            .get("kind")
            .and_then(Value::as_str)
            .expect("platform_proof kind");
        let platform_proof = match kind {
            "apple_app_attest" => {
                let key_id = proof_obj
                    .get("key_id")
                    .and_then(Value::as_str)
                    .expect("platform_proof key_id");
                let counter = proof_obj
                    .get("counter")
                    .and_then(Value::as_u64)
                    .expect("platform_proof counter");
                let assertion_b64 = proof_obj
                    .get("assertion_b64")
                    .and_then(Value::as_str)
                    .expect("platform_proof assertion_b64");
                let assertion = BASE64
                    .decode(assertion_b64)
                    .expect("platform_proof assertion base64");
                let challenge_hex = proof_obj
                    .get("challenge_hash_hex")
                    .and_then(Value::as_str)
                    .expect("platform_proof challenge_hash_hex");
                OfflinePlatformProof::AppleAppAttest(AppleAppAttestProof {
                    key_id: key_id.to_string(),
                    counter,
                    assertion,
                    challenge_hash: hash_from_hex(challenge_hex),
                })
            }
            other => panic!("unsupported platform proof kind {other}"),
        };

        OfflineSpendReceipt {
            tx_id,
            from,
            to,
            asset,
            amount,
            issued_at_ms,
            invoice_id,
            platform_proof,
            platform_snapshot: None,
            sender_certificate_id: certificate_id,
            sender_signature: Signature::from_bytes(&[0; 64]),
            build_claim: None,
        }
    }

    #[test]
    fn merkle_root_deterministic() {
        let receipt_a = sample_receipt(5, "a");
        let receipt_b = sample_receipt(6, "b");
        let leaf_a = OfflineReceiptLeaf::from_receipt(&receipt_a).expect("leaf a");
        let leaf_b = OfflineReceiptLeaf::from_receipt(&receipt_b).expect("leaf b");

        assert_eq!(
            hex::encode_upper(leaf_a.platform_proof_hash.as_ref()),
            "A8AE2E5B21547ADA98E767615901C1B40B28EF7E4A1E26E53A978C45103F810F"
        );
        assert_eq!(
            hex::encode_upper(leaf_b.platform_proof_hash.as_ref()),
            "6509B6E9E782793D4A84C76C011BCAF76E8AC967EAE60C780112F41A081D64D3"
        );
        let mut builder = OfflineReceiptMerkleBuilder::new();
        builder.push_receipt(&receipt_a).expect("leaf");
        builder.push_receipt(&receipt_b).expect("leaf");
        let root = builder.finalize();
        assert_eq!(
            root.to_hex_upper(),
            "000000000000000000000000000000000000000000000000CE303ECAFE17326C"
        );
    }

    #[test]
    fn poseidon_vectors_match_snapshot() {
        let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .expect("workspace root");
        let vectors_path = workspace_root.join("artifacts/offline_poseidon/vectors.json");
        let bytes =
            fs::read(&vectors_path).expect("offline Poseidon vectors must exist in artifacts/");
        let fixture: Value =
            norito::json::from_slice(&bytes).expect("vectors.json should be valid JSON");

        assert_eq!(
            fixture.get("version").and_then(Value::as_u64),
            Some(1),
            "vectors.json version mismatch"
        );
        assert_eq!(
            fixture.get("domain_tag").and_then(Value::as_str),
            Some("iroha.offline.receipt.merkle.v1"),
            "domain tag mismatch"
        );

        let cases = fixture
            .get("cases")
            .and_then(Value::as_array)
            .expect("cases array must be present");
        for case in cases {
            let name = case
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("<unnamed>");
            let leaves = case
                .get("leaves")
                .and_then(Value::as_array)
                .unwrap_or_else(|| panic!("case {name} missing leaves array"));
            let mut builder = OfflineReceiptMerkleBuilder::new();
            for leaf in leaves {
                let amount_mantissa = leaf
                    .get("amount_mantissa")
                    .and_then(Value::as_i64)
                    .unwrap_or_else(|| panic!("case {name} missing amount_mantissa"));
                let amount_scale = leaf
                    .get("amount_scale")
                    .and_then(Value::as_u64)
                    .unwrap_or_else(|| panic!("case {name} missing amount_scale"));
                let counter = leaf
                    .get("counter")
                    .and_then(Value::as_u64)
                    .unwrap_or_else(|| panic!("case {name} missing counter"));
                let leaf_entry = OfflineReceiptLeaf {
                    tx_id: parse_hash_field(leaf, "tx_id_hex", name),
                    amount: Numeric::new(
                        u128::try_from(amount_mantissa).unwrap_or_else(|_| {
                            panic!("case {name} negative amount not supported")
                        }),
                        u32::try_from(amount_scale)
                            .unwrap_or_else(|_| panic!("case {name} amount_scale too large")),
                    ),
                    counter,
                    receiver_hash: parse_hash_field(leaf, "receiver_hash_hex", name),
                    invoice_hash: parse_hash_field(leaf, "invoice_hash_hex", name),
                    platform_proof_hash: parse_hash_field(leaf, "platform_proof_hash_hex", name),
                };
                builder.push_leaf(&leaf_entry);
            }
            let computed_root = builder.finalize();
            let expected_hex = case
                .get("root_hex")
                .and_then(Value::as_str)
                .unwrap_or_else(|| panic!("case {name} missing root_hex"));
            let expected_root = PoseidonDigest::from_hex(expected_hex)
                .unwrap_or_else(|err| panic!("case {name} invalid root_hex: {err}"));
            assert_eq!(
                computed_root, expected_root,
                "Poseidon root mismatch for case {name}"
            );
        }
    }

    #[test]
    fn aggregate_envelope_from_receipts_uses_root() {
        let receipts = vec![sample_receipt(5, "a"), sample_receipt(6, "b")];
        let proof_sum = vec![0xAA, 0xBB];
        let proof_counter = vec![0xCC];
        let metadata = Metadata::default();
        let envelope = AggregateProofEnvelope::from_receipts(
            &receipts,
            Some(proof_sum.clone()),
            Some(proof_counter.clone()),
            None,
            metadata,
        )
        .expect("envelope");
        let expected_root = compute_receipts_root(&receipts).expect("root");
        assert_eq!(envelope.receipts_root, expected_root);
        assert_eq!(envelope.proof_sum, Some(proof_sum));
        assert_eq!(envelope.proof_counter, Some(proof_counter));
    }

    #[test]
    fn aggregate_envelope_from_requests_rejects_mismatch() {
        let account = sample_account();
        let asset = sample_asset(&account);
        let header = OfflineProofRequestHeader {
            version: OFFLINE_PROOF_REQUEST_VERSION_V1,
            bundle_id: Hash::new(b"bundle-1"),
            certificate_id: Hash::new(b"cert-1"),
            receipts_root: PoseidonDigest::zero(),
        };
        let sum = OfflineProofRequestSum {
            header: header.clone(),
            initial_commitment: OfflineAllowanceCommitment {
                asset: asset.clone(),
                amount: Numeric::new(1, 0),
                commitment: vec![0u8; 32],
            },
            resulting_commitment: vec![0u8; 32],
            claimed_delta: Numeric::new(1, 0),
            receipt_amounts: vec![Numeric::new(1, 0)],
            blinding_seeds: vec![OfflineProofBlindingSeed::derive(header.certificate_id, 1)],
        };
        let counter_header = OfflineProofRequestHeader {
            bundle_id: Hash::new(b"bundle-2"),
            ..header
        };
        let counter = OfflineProofRequestCounter {
            header: counter_header,
            counter_checkpoint: 0,
            counters: vec![1],
        };
        let result = AggregateProofEnvelope::from_proof_requests(
            Some(&sum),
            Some(&counter),
            None,
            None,
            None,
            None,
            Metadata::default(),
        );
        assert!(matches!(
            result,
            Err(AggregateProofEnvelopeError::HeaderMismatch)
        ));
    }

    #[test]
    fn aggregate_proof_fixture_matches_receipts_root() {
        let _default_domain_guard =
            crate::account::address::default_domain_guard(Some("wonderland"));

        let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .expect("workspace root");
        let fixture_path =
            workspace_root.join("fixtures/offline_bundle/aggregate_proof_fixture.json");
        let bytes = fs::read(&fixture_path).expect("aggregate proof fixture must exist");
        let fixture: Value =
            norito::json::from_slice(&bytes).expect("fixture should be valid JSON");

        let root_hex = fixture
            .get("receipts_root_hex")
            .and_then(Value::as_str)
            .expect("receipts_root_hex");
        let expected_root = PoseidonDigest::from_hex(root_hex).expect("valid receipts_root_hex");
        let metadata_value = fixture.get("metadata").expect("metadata");
        let metadata = metadata_from_fixture(metadata_value);
        let proof_sum = fixture
            .get("proof_sum_hex")
            .and_then(Value::as_str)
            .map(|hex| hex::decode(hex).expect("proof_sum_hex"));
        let proof_counter = fixture
            .get("proof_counter_hex")
            .and_then(Value::as_str)
            .map(|hex| hex::decode(hex).expect("proof_counter_hex"));
        let proof_replay = fixture
            .get("proof_replay_hex")
            .and_then(Value::as_str)
            .map(|hex| hex::decode(hex).expect("proof_replay_hex"));
        let receipts = fixture
            .get("receipts")
            .and_then(Value::as_array)
            .expect("receipts array");
        let first_receipt = receipts.first().expect("fixture receipt");
        let asset = first_receipt
            .get("asset")
            .and_then(Value::as_str)
            .map(fixture_asset)
            .expect("fixture asset");
        let controller = first_receipt
            .get("from")
            .and_then(Value::as_str)
            .map(fixture_account)
            .expect("fixture controller");
        let certificate = fixture_certificate(&controller, &asset);
        let parsed_receipts: Vec<_> = receipts
            .iter()
            .map(|entry| fixture_receipt(entry, &certificate))
            .collect();
        let computed_root = compute_receipts_root(&parsed_receipts).expect("root");
        assert_eq!(computed_root, expected_root);
        let envelope = AggregateProofEnvelope::from_receipts(
            &parsed_receipts,
            proof_sum.clone(),
            proof_counter.clone(),
            proof_replay.clone(),
            metadata,
        )
        .expect("envelope");
        assert_eq!(envelope.receipts_root, expected_root);
        assert_eq!(envelope.proof_sum, proof_sum);
        assert_eq!(envelope.proof_counter, proof_counter);
        assert_eq!(envelope.proof_replay, proof_replay);
    }

    fn parse_hash_field(entry: &Value, key: &str, case: &str) -> Hash {
        let hex = entry
            .get(key)
            .and_then(Value::as_str)
            .unwrap_or_else(|| panic!("case {case} missing {key}"));
        Hash::from_str(hex).unwrap_or_else(|err| panic!("case {case} invalid {key} ({hex}): {err}"))
    }
}
