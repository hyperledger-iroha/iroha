use iroha_schema::IntoSchema;
#[cfg(feature = "json")]
use mv::json::JsonKeyCodec;
use norito::codec::{Decode, Encode};

use super::capacity::ProviderId;
#[cfg(feature = "json")]
use crate::{DeriveJsonDeserialize, DeriveJsonSerialize};
use crate::{account::AccountId, metadata::Metadata};

/// Canonical BLAKE3-256 digest of a `sorafs_manifest::ManifestV1`.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema, Default,
)]
#[repr(transparent)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(transparent))]
pub struct ManifestDigest(
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))] pub [u8; 32],
);

impl ManifestDigest {
    /// Construct a new manifest digest wrapper.
    #[must_use]
    pub const fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Access the raw digest bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Computes the canonical digest for a [`sorafs_manifest::ManifestV1`].
    ///
    /// # Errors
    ///
    /// Returns [`norito::core::Error`] if the manifest payload cannot be encoded.
    pub fn from_manifest(
        manifest: &sorafs_manifest::ManifestV1,
    ) -> Result<Self, norito::core::Error> {
        manifest.digest().map(|hash| Self(*hash.as_bytes()))
    }
}

impl<'a> norito::core::DecodeFromSlice<'a> for ManifestDigest {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let mut cursor = bytes;
        let start_len = cursor.len();
        let value = Self::decode(&mut cursor)?;
        let consumed = start_len - cursor.len();
        Ok((value, consumed))
    }
}

#[allow(dead_code)]
fn _assert_manifest_digest_decode<'a>()
where
    ManifestDigest: norito::core::DecodeFromSlice<'a>,
{
}

#[cfg(feature = "json")]
impl JsonKeyCodec for ManifestDigest {
    fn encode_json_key(&self, out: &mut String) {
        self.as_bytes().encode_json_key(out);
    }

    fn decode_json_key(encoded: &str) -> Result<Self, norito::json::Error> {
        <[u8; 32] as JsonKeyCodec>::decode_json_key(encoded).map(Self)
    }
}

/// Registry handle describing the chunker profile selected for a manifest.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct ChunkerProfileHandle {
    /// Numeric profile identifier (`ProfileId` from the registry).
    pub profile_id: u32,
    /// Namespace that scopes the profile registry (`sorafs`).
    pub namespace: String,
    /// Human-readable profile name (e.g., `sf1`).
    pub name: String,
    /// Semantic version string of the parameter set.
    pub semver: String,
    /// Multihash code used when deriving chunk digests.
    pub multihash_code: u64,
}

impl ChunkerProfileHandle {
    /// Format the canonical handle string (`namespace.name@semver`).
    #[must_use]
    pub fn to_handle(&self) -> String {
        format!("{}.{}@{}", self.namespace, self.name, self.semver)
    }
}

/// Storage replication policy negotiated with the pin registry.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct PinPolicy {
    /// Minimum number of replicas the governance policy requires.
    pub min_replicas: u16,
    /// Storage tier requested for retention.
    pub storage_class: StorageClass,
    /// Epoch (inclusive) until which the manifest must remain pinned.
    pub retention_epoch: u64,
}

impl Default for PinPolicy {
    fn default() -> Self {
        Self {
            min_replicas: 1,
            storage_class: StorageClass::Hot,
            retention_epoch: 0,
        }
    }
}

/// Storage tier classification for `SoraFS` replicas.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema, Default, Hash,
)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "type", content = "value"))]
pub enum StorageClass {
    /// Low-latency replicas servicing developer workflows.
    #[default]
    Hot,
    /// Cost-optimised replicas with relaxed latency.
    Warm,
    /// Archival replicas retained for compliance.
    Cold,
}

/// Optional alias binding approved alongside a manifest.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema, Default)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct ManifestAliasBinding {
    /// Alias name (e.g., `docs`).
    pub name: String,
    /// Alias namespace (e.g., `sora`).
    pub namespace: String,
    /// Alias proof payload encoded as Norito.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::base64_vec"))]
    pub proof: Vec<u8>,
}

impl<'a> norito::core::DecodeFromSlice<'a> for ManifestAliasBinding {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let mut cursor = bytes;
        let start_len = cursor.len();
        let value = Self::decode(&mut cursor)?;
        let consumed = start_len - cursor.len();
        Ok((value, consumed))
    }
}

/// Lifecycle status of a manifest within the pin registry.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "status", content = "value"))]
pub enum PinStatus {
    /// Manifest registered and awaiting governance approval.
    Pending,
    /// Manifest approved and eligible for replication, with the effective epoch.
    Approved(
        /// Epoch (inclusive) when replication enters the required set.
        u64,
    ),
    /// Manifest retired and no longer part of the required replication set.
    Retired(
        /// Epoch (inclusive) when the manifest left the active set.
        u64,
    ),
}

impl PinStatus {
    /// Returns true if the manifest currently requires replication.
    #[must_use]
    pub const fn is_active(&self) -> bool {
        matches!(self, Self::Approved(_))
    }
}

/// Registry record capturing the lifecycle of a manifest pin request.
#[allow(missing_copy_implementations)]
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct PinManifestRecord {
    /// Canonical manifest digest (BLAKE3-256 of Norito encoding).
    pub digest: ManifestDigest,
    /// Chunker profile handle used to produce the CAR commitment.
    pub chunker: ChunkerProfileHandle,
    /// SHA3-256 digest of the ordered chunk metadata emitted during build.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub chunk_digest_sha3_256: [u8; 32],
    /// Replication policy bound to the manifest.
    pub policy: PinPolicy,
    /// Submitter that initiated the pin request.
    pub submitted_by: AccountId,
    /// Epoch when the request was recorded (inclusive).
    pub submitted_epoch: u64,
    /// Optional alias binding approved with the manifest.
    pub alias: Option<ManifestAliasBinding>,
    /// Optional predecessor manifest digest forming a succession chain.
    #[cfg_attr(feature = "json", norito(skip_serializing_if = "Option::is_none"))]
    pub successor_of: Option<ManifestDigest>,
    /// Optional metadata attached during registration.
    pub metadata: Metadata,
    /// Latest lifecycle status for the manifest.
    pub status: PinStatus,
    /// Optional human-readable explanation recorded alongside retirement.
    #[cfg_attr(feature = "json", norito(skip_serializing_if = "Option::is_none"))]
    pub retirement_reason: Option<String>,
    /// Optional digest of the `manifest_signatures.json` envelope attached during approval.
    #[cfg_attr(
        feature = "json",
        norito(with = "crate::json_helpers::fixed_bytes::option")
    )]
    pub council_envelope_digest: Option<[u8; 32]>,
}

impl PinManifestRecord {
    /// Construct a new pending record from the supplied fields.
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub fn new(
        digest: ManifestDigest,
        chunker: ChunkerProfileHandle,
        chunk_digest_sha3_256: [u8; 32],
        policy: PinPolicy,
        submitted_by: AccountId,
        submitted_epoch: u64,
        alias: Option<ManifestAliasBinding>,
        successor_of: Option<ManifestDigest>,
        metadata: Metadata,
    ) -> Self {
        Self {
            digest,
            chunker,
            chunk_digest_sha3_256,
            policy,
            submitted_by,
            submitted_epoch,
            alias,
            successor_of,
            metadata,
            status: PinStatus::Pending,
            retirement_reason: None,
            council_envelope_digest: None,
        }
    }

    /// Transition the record into an approved state with the provided epoch and envelope digest.
    pub fn approve(&mut self, approved_epoch: u64, envelope_digest: Option<[u8; 32]>) {
        self.status = PinStatus::Approved(approved_epoch);
        self.retirement_reason = None;
        if let Some(digest) = envelope_digest {
            self.council_envelope_digest = Some(digest);
        }
    }

    /// Transition the record into a retired state.
    pub fn retire(&mut self, retired_epoch: u64, reason: Option<String>) {
        self.status = PinStatus::Retired(retired_epoch);
        self.retirement_reason = reason;
    }
}

/// Canonical identifier for a manifest alias (`namespace/name`).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema, Hash)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct ManifestAliasId {
    /// Alias namespace (e.g., `sora`).
    pub namespace: String,
    /// Alias value (e.g., `docs`).
    pub name: String,
}

impl ManifestAliasId {
    /// Construct a new alias identifier.
    #[must_use]
    pub fn new(namespace: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            namespace: namespace.into(),
            name: name.into(),
        }
    }

    /// Returns a human-readable `namespace/name` label.
    #[must_use]
    pub fn as_label(&self) -> String {
        format!("{}/{}", self.namespace, self.name)
    }
}

impl From<&ManifestAliasBinding> for ManifestAliasId {
    fn from(binding: &ManifestAliasBinding) -> Self {
        Self::new(binding.namespace.clone(), binding.name.clone())
    }
}

/// Registry record describing an approved alias binding.
#[allow(missing_copy_implementations)]
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct ManifestAliasRecord {
    /// Canonical alias binding payload (includes namespace, name, proof).
    pub binding: ManifestAliasBinding,
    /// Manifest digest bound to the alias.
    pub manifest: ManifestDigest,
    /// Account that bound the alias.
    pub bound_by: AccountId,
    /// Epoch (inclusive) when the alias binding became active.
    pub bound_epoch: u64,
    /// Epoch (inclusive) when the alias binding expires unless renewed.
    pub expiry_epoch: u64,
}

impl ManifestAliasRecord {
    /// Create a new alias record from the supplied binding.
    #[must_use]
    pub fn new(
        binding: ManifestAliasBinding,
        manifest: ManifestDigest,
        bound_by: AccountId,
        bound_epoch: u64,
        expiry_epoch: u64,
    ) -> Self {
        Self {
            binding,
            manifest,
            bound_by,
            bound_epoch,
            expiry_epoch,
        }
    }

    /// Returns the canonical alias identifier.
    #[must_use]
    pub fn alias_id(&self) -> ManifestAliasId {
        ManifestAliasId::from(&self.binding)
    }

    /// Returns `true` if the record refers to the supplied manifest digest.
    #[must_use]
    pub fn targets_manifest(&self, digest: &ManifestDigest) -> bool {
        &self.manifest == digest
    }
}

/// Unique identifier assigned to replication orders.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema, Hash, Default,
)]
#[repr(transparent)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(transparent))]
pub struct ReplicationOrderId(
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))] pub [u8; 32],
);

impl ReplicationOrderId {
    /// Construct a new replication order identifier.
    #[must_use]
    pub const fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Access the raw identifier bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Lifecycle status for replication orders.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "status", content = "detail"))]
pub enum ReplicationOrderStatus {
    /// Order is outstanding and awaits completion.
    Pending,
    /// Order finished at the supplied epoch.
    Completed(
        /// Epoch (inclusive) when replication completed.
        u64,
    ),
    /// Order expired without satisfying redundancy or past the deadline.
    Expired(
        /// Epoch (inclusive) when the order expired.
        u64,
    ),
}

impl ReplicationOrderStatus {
    /// Returns `true` when the order is still pending.
    #[must_use]
    pub const fn is_pending(&self) -> bool {
        matches!(self, Self::Pending)
    }
}

/// Status reported by a storage provider for an issued order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "status", content = "value"))]
pub enum ReplicationReceiptStatus {
    /// Provider accepted the assignment and is ingesting the manifest.
    Accepted,
    /// Provider completed ingestion and `PoR` sampling.
    Completed,
    /// Provider rejected the assignment (capacity issues, policy mismatch, etc.).
    Rejected,
}

/// Provider acknowledgement persisted by the registry.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct ReplicationReceiptRecord {
    /// Provider responding to the order.
    pub provider: ProviderId,
    /// Reported status for the replication attempt.
    pub status: ReplicationReceiptStatus,
    /// Unix timestamp (seconds) when the status was recorded.
    pub timestamp: u64,
    /// Optional digest of the `PoR` sample bundle.
    #[cfg_attr(
        feature = "json",
        norito(with = "crate::json_helpers::fixed_bytes::option")
    )]
    pub por_sample_digest: Option<[u8; 32]>,
}

impl ReplicationReceiptRecord {
    /// Returns true when the receipt reports a completed replication.
    #[must_use]
    pub const fn is_completed(&self) -> bool {
        matches!(self.status, ReplicationReceiptStatus::Completed)
    }
}

/// Record stored for each issued replication order.
#[allow(missing_copy_implementations)]
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct ReplicationOrderRecord {
    /// Identifier of the replication order.
    pub order_id: ReplicationOrderId,
    /// Manifest digest targeted by the order.
    pub manifest_digest: ManifestDigest,
    /// Account that issued the order.
    pub issued_by: AccountId,
    /// Epoch (inclusive) when the order was issued.
    pub issued_epoch: u64,
    /// Deadline epoch for completing ingestion.
    pub deadline_epoch: u64,
    /// Canonical Norito payload describing the replication order.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::base64_vec"))]
    pub canonical_order: Vec<u8>,
    /// Current lifecycle status for the order.
    pub status: ReplicationOrderStatus,
}

impl ReplicationOrderRecord {
    /// Mark the order as completed at the supplied epoch.
    pub fn complete(&mut self, completion_epoch: u64) {
        self.status = ReplicationOrderStatus::Completed(completion_epoch);
    }
}

#[cfg(test)]
mod tests {
    use core::str::FromStr;

    use super::*;

    #[test]
    fn manifest_digest_round_trip() {
        let digest = ManifestDigest::new([0xAB; 32]);
        let encoded = digest.encode();
        let mut slice = &encoded[..];
        let decoded = ManifestDigest::decode(&mut slice).expect("decode manifest digest");
        assert_eq!(digest, decoded);
    }

    #[test]
    fn pin_manifest_record_state_transitions() {
        let digest = ManifestDigest::new([1; 32]);
        let chunk_digest = [0xCD; 32];
        let chunker = ChunkerProfileHandle {
            profile_id: 1,
            namespace: "sorafs".into(),
            name: "sf1".into(),
            semver: "1.0.0".into(),
            multihash_code: 0x1f,
        };
        let mut record = PinManifestRecord::new(
            digest,
            chunker,
            chunk_digest,
            PinPolicy::default(),
            AccountId::from_str(
                "ed0120BDF918243253B1E731FA096194C8928DA37C4D3226F97EEBD18CF5523D758D6C@wonderland",
            )
            .expect("parse account id"),
            42,
            None,
            None,
            Metadata::default(),
        );
        assert!(matches!(record.status, PinStatus::Pending));
        assert_eq!(record.chunk_digest_sha3_256, chunk_digest);

        record.approve(64, Some([2; 32]));
        assert!(record.status.is_active());

        record.retire(128, Some("superseded".into()));
        assert!(matches!(record.status, PinStatus::Retired(128)));
        assert_eq!(record.retirement_reason.as_deref(), Some("superseded"));
    }

    #[test]
    fn replication_order_record_stores_canonical_payload() {
        let payload = vec![0xAA, 0xBB, 0xCC];
        let mut record = ReplicationOrderRecord {
            order_id: ReplicationOrderId::new([0x44; 32]),
            manifest_digest: ManifestDigest::new([0x55; 32]),
            issued_by: AccountId::from_str(
                "ed0120BDF918243253B1E731FA096194C8928DA37C4D3226F97EEBD18CF5523D758D6C@wonderland",
            )
            .expect("parse account id"),
            issued_epoch: 10,
            deadline_epoch: 20,
            canonical_order: payload.clone(),
            status: ReplicationOrderStatus::Pending,
        };

        assert!(record.status.is_pending());
        assert_eq!(record.canonical_order, payload);

        record.complete(42);
        assert!(matches!(
            record.status,
            ReplicationOrderStatus::Completed(epoch) if epoch == 42
        ));
    }

    #[test]
    fn manifest_digest_matches_sorafs_manifest_digest() {
        use sorafs_manifest::{
            BLAKE3_256_MULTIHASH_CODE, ChunkingProfileV1, DagCodecId, ManifestBuilder, PinPolicy,
            ProfileId, StorageClass,
        };

        let manifest = ManifestBuilder::new()
            .root_cid(vec![0x01, 0x02, 0x03])
            .dag_codec(DagCodecId(0x71))
            .chunking_profile(ChunkingProfileV1 {
                profile_id: ProfileId(7),
                namespace: "sorafs".into(),
                name: "sf1".into(),
                semver: "1.0.0".into(),
                min_size: 16 * 1024,
                target_size: 32 * 1024,
                max_size: 64 * 1024,
                break_mask: 0b1111,
                multihash_code: BLAKE3_256_MULTIHASH_CODE,
                aliases: vec!["sorafs.sf1@1.0.0".into()],
            })
            .content_length(1_048_576)
            .car_digest([0xAB; 32])
            .car_size(1_100_000)
            .pin_policy(PinPolicy {
                min_replicas: 2,
                storage_class: StorageClass::Hot,
                retention_epoch: 24,
            })
            .build()
            .expect("build manifest");

        let digest = ManifestDigest::from_manifest(&manifest).expect("compute digest");
        let expected = manifest.digest().expect("compute manifest digest");

        assert_eq!(digest.as_bytes(), expected.as_bytes());
    }
}
