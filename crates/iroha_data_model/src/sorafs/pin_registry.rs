use iroha_primitives::numeric::Quantity;
use iroha_schema::IntoSchema;
#[cfg(feature = "json")]
use mv::json::JsonKeyCodec;
use norito::codec::{Decode, Encode};

use super::capacity::ProviderId;
#[cfg(feature = "json")]
use crate::{DeriveJsonDeserialize, DeriveJsonSerialize};
use crate::{account::AccountId, asset::AssetDefinitionId, metadata::Metadata};

/// Exact byte length of a canonical first-release manifest root CID.
pub const MANIFEST_ROOT_CID_LENGTH: usize = sorafs_manifest::MAX_MANIFEST_ROOT_CID_BYTES;

/// Canonical binary `CIDv1` identifying the content DAG root of a manifest.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, IntoSchema)]
#[repr(transparent)]
pub struct ManifestRootCid([u8; MANIFEST_ROOT_CID_LENGTH]);

impl ManifestRootCid {
    /// Constructs a root CID after validating the complete first-release layout.
    ///
    /// # Errors
    ///
    /// Returns [`ManifestRootCidError`] when any CID header field is not canonical
    /// or the BLAKE3-256 digest is the all-zero sentinel.
    pub fn new(bytes: [u8; MANIFEST_ROOT_CID_LENGTH]) -> Result<Self, ManifestRootCidError> {
        validate_manifest_root_cid_bytes(&bytes)?;
        Ok(Self(bytes))
    }

    /// Builds the canonical first-release root CID for a non-zero BLAKE3 digest.
    ///
    /// # Errors
    ///
    /// Returns an [`ManifestRootCidErrorKind::InertDigest`] error for the
    /// all-zero digest.
    pub fn from_blake3_digest(digest: [u8; 32]) -> Result<Self, ManifestRootCidError> {
        if digest.iter().all(|byte| *byte == 0) {
            return Err(ManifestRootCidError::new(
                ManifestRootCidErrorKind::InertDigest,
                0,
            ));
        }
        let mut bytes = [0_u8; MANIFEST_ROOT_CID_LENGTH];
        bytes[..4].copy_from_slice(&[1, 0x71, 0x1f, 32]);
        bytes[4..].copy_from_slice(&digest);
        Ok(Self(bytes))
    }

    /// Returns the binary CID bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; MANIFEST_ROOT_CID_LENGTH] {
        &self.0
    }

    /// Copies a canonical-width CID from a byte slice.
    ///
    /// # Errors
    ///
    /// Returns [`ManifestRootCidError`] when `bytes` is not the exact canonical
    /// 36-byte CIDv1/dag-cbor/BLAKE3-256 representation.
    pub fn try_from_slice(bytes: &[u8]) -> Result<Self, ManifestRootCidError> {
        let found = bytes.len();
        let bytes = bytes.try_into().map_err(|_| {
            ManifestRootCidError::new(ManifestRootCidErrorKind::InvalidLength, found)
        })?;
        Self::new(bytes)
    }
}

impl TryFrom<&[u8]> for ManifestRootCid {
    type Error = ManifestRootCidError;

    fn try_from(bytes: &[u8]) -> Result<Self, Self::Error> {
        Self::try_from_slice(bytes)
    }
}

impl TryFrom<Vec<u8>> for ManifestRootCid {
    type Error = ManifestRootCidError;

    fn try_from(bytes: Vec<u8>) -> Result<Self, Self::Error> {
        Self::try_from_slice(&bytes)
    }
}

impl norito::NoritoSerialize for ManifestRootCid {
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), norito::core::Error> {
        norito::NoritoSerialize::serialize(&self.0, writer)
    }

    fn encoded_len_hint(&self) -> Option<usize> {
        norito::NoritoSerialize::encoded_len_hint(&self.0)
    }

    fn encoded_len_exact(&self) -> Option<usize> {
        norito::NoritoSerialize::encoded_len_exact(&self.0)
    }
}

impl<'de> norito::NoritoDeserialize<'de> for ManifestRootCid {
    fn deserialize(archived: &'de norito::core::Archived<Self>) -> Self {
        Self::try_deserialize(archived)
            .expect("archived manifest root CID must use the canonical first-release layout")
    }

    fn try_deserialize(
        archived: &'de norito::core::Archived<Self>,
    ) -> Result<Self, norito::core::Error> {
        let bytes = <[u8; MANIFEST_ROOT_CID_LENGTH] as norito::NoritoDeserialize>::try_deserialize(
            archived.cast(),
        )?;
        Self::new(bytes).map_err(|error| norito::core::Error::Message(error.to_string()))
    }
}

#[cfg(feature = "json")]
impl norito::json::JsonSerialize for ManifestRootCid {
    fn json_serialize(&self, out: &mut String) {
        crate::json_helpers::fixed_bytes::serialize(&self.0, out);
    }
}

#[cfg(feature = "json")]
impl norito::json::JsonDeserialize for ManifestRootCid {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        let bytes = crate::json_helpers::fixed_bytes::deserialize(parser)?;
        Self::new(bytes).map_err(|error| norito::json::Error::Message(error.to_string()))
    }
}

impl<'a> norito::core::DecodeFromSlice<'a> for ManifestRootCid {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let mut cursor = bytes;
        let start_len = cursor.len();
        let value = Self::decode(&mut cursor)?;
        Ok((value, start_len - cursor.len()))
    }
}

/// Error returned when a manifest root CID is not canonical for the first release.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ManifestRootCidError {
    /// Failed canonical-layout rule.
    pub kind: ManifestRootCidErrorKind,
    /// Observed value, or zero when the digest itself is inert.
    pub found: usize,
}

impl ManifestRootCidError {
    const fn new(kind: ManifestRootCidErrorKind, found: usize) -> Self {
        Self { kind, found }
    }
}

impl std::fmt::Display for ManifestRootCidError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.kind {
            ManifestRootCidErrorKind::InvalidLength => write!(
                formatter,
                "manifest root CID must contain exactly {MANIFEST_ROOT_CID_LENGTH} bytes (found {})",
                self.found
            ),
            ManifestRootCidErrorKind::InvalidVersion => write!(
                formatter,
                "manifest root CID version must be 1 (found {})",
                self.found
            ),
            ManifestRootCidErrorKind::InvalidCodec => write!(
                formatter,
                "manifest root CID codec must be dag-cbor 0x71 (found {:#x})",
                self.found
            ),
            ManifestRootCidErrorKind::InvalidMultihash => write!(
                formatter,
                "manifest root CID multihash must be BLAKE3-256 0x1f (found {:#x})",
                self.found
            ),
            ManifestRootCidErrorKind::InvalidDigestLength => write!(
                formatter,
                "manifest root CID digest length must be 32 bytes (found {})",
                self.found
            ),
            ManifestRootCidErrorKind::InertDigest => {
                formatter.write_str("manifest root CID digest must not be all zero")
            }
        }
    }
}

impl std::error::Error for ManifestRootCidError {}

/// Canonical-layout rule violated by a manifest root CID.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ManifestRootCidErrorKind {
    /// CID byte width differs from the fixed first-release layout.
    InvalidLength,
    /// CID version is not the canonical single-byte `CIDv1` value.
    InvalidVersion,
    /// CID codec is not the canonical single-byte dag-cbor value.
    InvalidCodec,
    /// CID multihash is not the canonical single-byte BLAKE3-256 value.
    InvalidMultihash,
    /// CID digest-length byte is not 32.
    InvalidDigestLength,
    /// CID carries an inert all-zero digest.
    InertDigest,
}

fn validate_manifest_root_cid_bytes(
    bytes: &[u8; MANIFEST_ROOT_CID_LENGTH],
) -> Result<(), ManifestRootCidError> {
    if bytes[0] != 1 {
        return Err(ManifestRootCidError::new(
            ManifestRootCidErrorKind::InvalidVersion,
            usize::from(bytes[0]),
        ));
    }
    if bytes[1] != 0x71 {
        return Err(ManifestRootCidError::new(
            ManifestRootCidErrorKind::InvalidCodec,
            usize::from(bytes[1]),
        ));
    }
    if bytes[2] != 0x1f {
        return Err(ManifestRootCidError::new(
            ManifestRootCidErrorKind::InvalidMultihash,
            usize::from(bytes[2]),
        ));
    }
    if bytes[3] != 32 {
        return Err(ManifestRootCidError::new(
            ManifestRootCidErrorKind::InvalidDigestLength,
            usize::from(bytes[3]),
        ));
    }
    if bytes[4..].iter().all(|byte| *byte == 0) {
        return Err(ManifestRootCidError::new(
            ManifestRootCidErrorKind::InertDigest,
            0,
        ));
    }
    Ok(())
}

/// Canonical BLAKE3-256 digest of a `sorafs_manifest::ManifestV1`.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema, Default,
)]
#[repr(transparent)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
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

/// XOR fee payment recorded when a public pin manifest is admitted.
#[allow(missing_copy_implementations)]
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct PinFeePayment {
    /// Account whose balance paid for the public pin.
    pub paid_by: AccountId,
    /// Asset definition used to collect the fee.
    pub fee_asset_id: AssetDefinitionId,
    /// Account that received the fee.
    pub treasury_account_id: AccountId,
    /// Nominal fee amount.
    pub amount: Quantity,
}

/// Registry record capturing the lifecycle of a manifest pin request.
#[allow(missing_copy_implementations)]
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct PinManifestRecord {
    /// Canonical manifest digest (BLAKE3-256 of Norito encoding).
    pub digest: ManifestDigest,
    /// Canonical `CIDv1` of the content DAG root described by the manifest.
    pub root_cid: ManifestRootCid,
    /// Chunker profile handle used to produce the CAR commitment.
    pub chunker: ChunkerProfileHandle,
    /// SHA3-256 digest of the ordered chunk metadata emitted during build.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub chunk_digest_sha3_256: [u8; 32],
    /// Merkle root of the canonical Proof-of-Retrievability tree.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub por_root: [u8; 32],
    /// Total payload length covered by the manifest.
    pub content_length: u64,
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
    /// Public pin fee payment metadata, present only after on-chain fee collection.
    #[cfg_attr(feature = "json", norito(skip_serializing_if = "Option::is_none"))]
    pub pin_fee_payment: Option<PinFeePayment>,
}

/// Finalized block anchor for one coherent pin-manifest query result.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct PinManifestFinalizedCursorV1 {
    /// Finalized block height observed by the immutable state view.
    pub height: u64,
    /// Finalized block hash resolved from that same immutable state view.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub block_hash: [u8; 32],
}

/// One authoritative pin manifest anchored to finalized chain state.
#[allow(missing_copy_implementations)]
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct PinManifestFinalizedRecordV1 {
    /// Finalized state anchor at which the manifest was read.
    pub finalized_cursor: PinManifestFinalizedCursorV1,
    /// Chain-authoritative pin-manifest lifecycle record.
    pub manifest: PinManifestRecord,
}

impl PinManifestRecord {
    /// Construct a new pending record from the supplied fields.
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub fn new(
        digest: ManifestDigest,
        root_cid: ManifestRootCid,
        chunker: ChunkerProfileHandle,
        chunk_digest_sha3_256: [u8; 32],
        por_root: [u8; 32],
        content_length: u64,
        policy: PinPolicy,
        submitted_by: AccountId,
        submitted_epoch: u64,
        alias: Option<ManifestAliasBinding>,
        successor_of: Option<ManifestDigest>,
        metadata: Metadata,
    ) -> Self {
        Self {
            digest,
            root_cid,
            chunker,
            chunk_digest_sha3_256,
            por_root,
            content_length,
            policy,
            submitted_by,
            submitted_epoch,
            alias,
            successor_of,
            metadata,
            status: PinStatus::Pending,
            retirement_reason: None,
            council_envelope_digest: None,
            pin_fee_payment: None,
        }
    }

    /// Record the public pin fee payment associated with this manifest.
    pub fn record_pin_fee_payment(&mut self, payment: PinFeePayment) {
        self.pin_fee_payment = Some(payment);
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

/// Governance identity of the exact provider-ingest completion signer policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema, Hash)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct ProviderIngestCompletionSignerPolicyV1 {
    /// Stable governance identity for this provider-owner signing policy.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub policy_id: [u8; 32],
    /// Monotonic policy revision beginning at one.
    pub revision: u64,
    /// Digest of the preceding tuple's governed leaf policy, absent only at revision one.
    #[cfg_attr(
        feature = "json",
        norito(with = "crate::json_helpers::fixed_bytes::option")
    )]
    pub predecessor_digest: Option<[u8; 32]>,
    /// Digest of the exact governed signer, key, and validity leaf policy.
    ///
    /// The canonical chain identity is the complete tuple of policy id, revision,
    /// predecessor digest, and this leaf-policy digest.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub policy_digest: [u8; 32],
}

impl ProviderIngestCompletionSignerPolicyV1 {
    /// Return whether every canonical identity component is non-zero.
    #[must_use]
    pub const fn is_valid(&self) -> bool {
        let mut policy_id_is_nonzero = false;
        let mut policy_digest_is_nonzero = false;
        let mut index = 0;
        while index < 32 {
            if self.policy_id[index] != 0 {
                policy_id_is_nonzero = true;
            }
            if self.policy_digest[index] != 0 {
                policy_digest_is_nonzero = true;
            }
            index += 1;
        }
        let predecessor_is_canonical = match (self.revision, self.predecessor_digest) {
            (1, None) => true,
            (2.., Some(digest)) => {
                let mut nonzero = false;
                let mut index = 0;
                while index < 32 {
                    if digest[index] != 0 {
                        nonzero = true;
                    }
                    index += 1;
                }
                nonzero
            }
            _ => false,
        };
        policy_id_is_nonzero && predecessor_is_canonical && policy_digest_is_nonzero
    }
}

/// Chain-authoritative owner and governed signer policy for provider ingest.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct ProviderIngestCompletionAuthorityV1 {
    /// Current registered owner authorized to complete this provider's work.
    pub provider_owner: AccountId,
    /// Exact governed completion-signer policy active for this owner.
    pub signer_policy: ProviderIngestCompletionSignerPolicyV1,
}

impl ProviderIngestCompletionAuthorityV1 {
    /// Construct one exact provider-ingest completion authority.
    #[must_use]
    pub const fn new(
        provider_owner: AccountId,
        signer_policy: ProviderIngestCompletionSignerPolicyV1,
    ) -> Self {
        Self {
            provider_owner,
            signer_policy,
        }
    }

    /// Return whether the governed signer policy has a canonical identity.
    #[must_use]
    pub const fn is_valid(&self) -> bool {
        self.signer_policy.is_valid()
    }
}

/// Finalized committed-chain anchor carried by a provider completion.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema, Hash)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct ProviderIngestFinalizedAnchorV1 {
    /// One-based committed block height.
    pub height: u64,
    /// Exact committed block hash at `height`.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub block_hash: [u8; 32],
}

impl ProviderIngestFinalizedAnchorV1 {
    /// Return whether this anchor can identify a committed block.
    #[must_use]
    pub const fn is_valid(&self) -> bool {
        if self.height == 0 {
            return false;
        }
        let mut index = 0;
        while index < 32 {
            if self.block_hash[index] != 0 {
                return true;
            }
            index += 1;
        }
        false
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

/// Provider-scoped completion recorded for a replication assignment.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct ReplicationOrderCompletionRecord {
    /// Provider assignment that completed ingestion.
    pub provider_id: ProviderId,
    /// Registered provider owner that authorized the completion transaction.
    pub completed_by: AccountId,
    /// Epoch (inclusive) when this provider completed ingestion.
    pub completion_epoch: u64,
    /// Exact order-scoped assignment revision accepted at commit.
    pub assignment_revision: u64,
    /// Exact provider owner and governed signer policy accepted at commit.
    pub completion_authority: ProviderIngestCompletionAuthorityV1,
    /// Finalized committed-chain prefix on which completion preparation was based.
    pub finalized_anchor: ProviderIngestFinalizedAnchorV1,
}

/// Record stored for each issued replication order.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct ReplicationOrderRecord {
    /// Identifier of the replication order.
    pub order_id: ReplicationOrderId,
    /// Manifest digest targeted by the order.
    pub manifest_digest: ManifestDigest,
    /// Content root CID bound into the canonical replication payload.
    pub manifest_root_cid: ManifestRootCid,
    /// Account that issued the order.
    pub issued_by: AccountId,
    /// Epoch (inclusive) when the order was issued.
    pub issued_epoch: u64,
    /// Deadline epoch for completing ingestion.
    pub deadline_epoch: u64,
    /// Canonical Norito payload describing the replication order.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::base64_vec"))]
    pub canonical_order: Vec<u8>,
    /// Monotonic revision of the canonical provider assignment set.
    pub assignment_revision: u64,
    /// Provider-scoped completions in authoritative transaction order.
    pub provider_completions: Vec<ReplicationOrderCompletionRecord>,
    /// Current lifecycle status for the order.
    pub status: ReplicationOrderStatus,
}

impl ReplicationOrderRecord {
    /// Return the completion recorded for `provider_id`, if any.
    #[must_use]
    pub fn provider_completion(
        &self,
        provider_id: ProviderId,
    ) -> Option<&ReplicationOrderCompletionRecord> {
        self.provider_completions
            .iter()
            .find(|completion| completion.provider_id == provider_id)
    }

    /// Mark the order as expired at the supplied epoch.
    pub fn expire(&mut self, expiration_epoch: u64) {
        self.status = ReplicationOrderStatus::Expired(expiration_epoch);
    }
}

#[cfg(test)]
mod tests {
    use iroha_primitives::numeric::Numeric;

    use super::*;

    fn fixture_account() -> AccountId {
        AccountId::new(
            "ed0120BDF918243253B1E731FA096194C8928DA37C4D3226F97EEBD18CF5523D758D6C"
                .parse()
                .expect("public key"),
        )
    }

    #[derive(Encode)]
    struct ForgedPinFeePayment {
        paid_by: AccountId,
        fee_asset_id: AssetDefinitionId,
        treasury_account_id: AccountId,
        amount: Numeric,
    }

    #[test]
    fn manifest_digest_round_trip() {
        let digest = ManifestDigest::new([0xAB; 32]);
        let encoded = digest.encode();
        let mut slice = &encoded[..];
        let decoded = ManifestDigest::decode(&mut slice).expect("decode manifest digest");
        assert_eq!(digest, decoded);
    }

    #[test]
    fn pin_fee_payment_rejects_forged_negative_amount() {
        let forged = ForgedPinFeePayment {
            paid_by: fixture_account(),
            fee_asset_id: AssetDefinitionId::derive_from_components(
                crate::domain::DomainId::try_new("sora", "universal").expect("domain id"),
                "xor".parse().expect("asset name"),
            ),
            treasury_account_id: fixture_account(),
            amount: Numeric::new(-1_i32, 0),
        };
        let encoded = forged.encode();
        let mut input = encoded.as_slice();
        assert!(
            PinFeePayment::decode(&mut input).is_err(),
            "pin fee payment must reject a forged negative amount"
        );
    }

    #[test]
    fn manifest_root_cid_enforces_exact_width() {
        let bytes: [u8; MANIFEST_ROOT_CID_LENGTH] =
            sorafs_manifest::canonical_manifest_root_cid([0xA5; 32])
                .try_into()
                .expect("canonical CID width");
        let cid = ManifestRootCid::new(bytes).expect("canonical CID");
        assert_eq!(ManifestRootCid::try_from_slice(cid.as_bytes()), Ok(cid));
        assert_eq!(
            ManifestRootCid::try_from_slice(&cid.as_bytes()[..35]),
            Err(ManifestRootCidError::new(
                ManifestRootCidErrorKind::InvalidLength,
                35,
            ))
        );
    }

    #[test]
    fn manifest_root_cid_rejects_noncanonical_headers_and_inert_digest() {
        let canonical: [u8; MANIFEST_ROOT_CID_LENGTH] =
            sorafs_manifest::canonical_manifest_root_cid([0xA5; 32])
                .try_into()
                .expect("canonical CID width");
        for (index, replacement, expected) in [
            (
                0,
                2,
                ManifestRootCidError::new(ManifestRootCidErrorKind::InvalidVersion, 2),
            ),
            (
                1,
                0x70,
                ManifestRootCidError::new(ManifestRootCidErrorKind::InvalidCodec, 0x70),
            ),
            (
                2,
                0x12,
                ManifestRootCidError::new(ManifestRootCidErrorKind::InvalidMultihash, 0x12),
            ),
            (
                3,
                31,
                ManifestRootCidError::new(ManifestRootCidErrorKind::InvalidDigestLength, 31),
            ),
        ] {
            let mut malformed = canonical;
            malformed[index] = replacement;
            assert_eq!(ManifestRootCid::new(malformed), Err(expected));
        }

        let mut inert = canonical;
        inert[4..].fill(0);
        assert_eq!(
            ManifestRootCid::new(inert),
            Err(ManifestRootCidError::new(
                ManifestRootCidErrorKind::InertDigest,
                0,
            ))
        );
        assert_eq!(
            ManifestRootCid::from_blake3_digest([0; 32]),
            Err(ManifestRootCidError::new(
                ManifestRootCidErrorKind::InertDigest,
                0,
            ))
        );
    }

    #[test]
    fn manifest_root_cid_decoders_reject_noncanonical_values() {
        let canonical = ManifestRootCid::from_blake3_digest([0xA5; 32]).expect("canonical CID");
        let encoded = canonical.encode();
        let mut slice = encoded.as_slice();
        assert_eq!(
            ManifestRootCid::decode(&mut slice).expect("decode canonical CID"),
            canonical
        );

        let mut malformed = *canonical.as_bytes();
        malformed[1] = 0x70;
        let encoded = malformed.encode();
        let mut slice = encoded.as_slice();
        assert!(ManifestRootCid::decode(&mut slice).is_err());

        #[cfg(feature = "json")]
        {
            let value = norito::json::to_value(&canonical).expect("canonical CID JSON value");
            assert_eq!(
                norito::json::from_value::<ManifestRootCid>(value)
                    .expect("decode canonical CID JSON"),
                canonical
            );
            let value = norito::json::to_value(&malformed).expect("malformed CID JSON value");
            assert!(norito::json::from_value::<ManifestRootCid>(value).is_err());
        }
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
            ManifestRootCid::try_from(sorafs_manifest::canonical_manifest_root_cid([0xA5; 32]))
                .expect("canonical root CID"),
            chunker,
            chunk_digest,
            [0xCE; 32],
            1_048_576,
            PinPolicy::default(),
            AccountId::new(
                "ed0120BDF918243253B1E731FA096194C8928DA37C4D3226F97EEBD18CF5523D758D6C"
                    .parse()
                    .expect("public key"),
            ),
            42,
            None,
            None,
            Metadata::default(),
        );
        assert!(matches!(record.status, PinStatus::Pending));
        assert_eq!(record.chunk_digest_sha3_256, chunk_digest);
        assert_eq!(record.por_root, [0xCE; 32]);
        assert_eq!(record.content_length, 1_048_576);

        record.approve(64, Some([2; 32]));
        assert!(record.status.is_active());

        record.retire(128, Some("superseded".into()));
        assert!(matches!(record.status, PinStatus::Retired(128)));
        assert_eq!(record.retirement_reason.as_deref(), Some("superseded"));
    }

    #[test]
    fn provider_ingest_completion_context_enforces_canonical_policy_chain_and_anchor() {
        let revision_one = ProviderIngestCompletionSignerPolicyV1 {
            policy_id: [0x91; 32],
            revision: 1,
            predecessor_digest: None,
            policy_digest: [0x92; 32],
        };
        assert!(revision_one.is_valid());
        assert!(
            ProviderIngestCompletionAuthorityV1::new(fixture_account(), revision_one).is_valid()
        );

        let mut revision_one_with_predecessor = revision_one;
        revision_one_with_predecessor.predecessor_digest = Some([0x90; 32]);
        assert!(!revision_one_with_predecessor.is_valid());
        let revision_two = ProviderIngestCompletionSignerPolicyV1 {
            revision: 2,
            predecessor_digest: Some(revision_one.policy_digest),
            policy_digest: [0x93; 32],
            ..revision_one
        };
        assert!(revision_two.is_valid());
        assert!(
            !ProviderIngestCompletionSignerPolicyV1 {
                predecessor_digest: Some([0; 32]),
                ..revision_two
            }
            .is_valid()
        );

        assert!(
            ProviderIngestFinalizedAnchorV1 {
                height: 1,
                block_hash: [0x94; 32],
            }
            .is_valid()
        );
        assert!(
            !ProviderIngestFinalizedAnchorV1 {
                height: 0,
                block_hash: [0x94; 32],
            }
            .is_valid()
        );
        assert!(
            !ProviderIngestFinalizedAnchorV1 {
                height: 1,
                block_hash: [0; 32],
            }
            .is_valid()
        );
    }

    #[test]
    fn replication_order_record_stores_canonical_payload() {
        let payload = vec![0xAA, 0xBB, 0xCC];
        let mut record = ReplicationOrderRecord {
            order_id: ReplicationOrderId::new([0x44; 32]),
            manifest_digest: ManifestDigest::new([0x55; 32]),
            manifest_root_cid: ManifestRootCid::try_from(
                sorafs_manifest::canonical_manifest_root_cid([0x56; 32]),
            )
            .expect("canonical root CID"),
            issued_by: AccountId::new(
                "ed0120BDF918243253B1E731FA096194C8928DA37C4D3226F97EEBD18CF5523D758D6C"
                    .parse()
                    .expect("public key"),
            ),
            issued_epoch: 10,
            deadline_epoch: 20,
            canonical_order: payload.clone(),
            assignment_revision: 1,
            provider_completions: Vec::new(),
            status: ReplicationOrderStatus::Pending,
        };

        assert!(record.status.is_pending());
        assert_eq!(record.canonical_order, payload);

        let provider_id = ProviderId::new([0x66; 32]);
        let completed_by = record.issued_by.clone();
        record
            .provider_completions
            .push(ReplicationOrderCompletionRecord {
                provider_id,
                completed_by: completed_by.clone(),
                completion_epoch: 20,
                assignment_revision: 1,
                completion_authority: ProviderIngestCompletionAuthorityV1::new(
                    completed_by,
                    ProviderIngestCompletionSignerPolicyV1 {
                        policy_id: [0x91; 32],
                        revision: 1,
                        predecessor_digest: None,
                        policy_digest: [0x92; 32],
                    },
                ),
                finalized_anchor: ProviderIngestFinalizedAnchorV1 {
                    height: 20,
                    block_hash: [0x93; 32],
                },
            });
        record.status = ReplicationOrderStatus::Completed(20);
        assert_eq!(
            record
                .provider_completion(provider_id)
                .map(|completion| completion.completion_epoch),
            Some(20)
        );
        assert!(matches!(
            record.status,
            ReplicationOrderStatus::Completed(epoch) if epoch == 20
        ));
        record.expire(43);
        assert!(matches!(
            record.status,
            ReplicationOrderStatus::Expired(epoch) if epoch == 43
        ));
    }

    #[test]
    fn manifest_digest_matches_sorafs_manifest_digest() {
        use sorafs_manifest::{
            BLAKE3_256_MULTIHASH_CODE, ChunkingProfileV1, DagCodecId, ManifestBuilder, PinPolicy,
            ProfileId, StorageClass,
        };

        let manifest = ManifestBuilder::new()
            .root_cid(sorafs_manifest::canonical_manifest_root_cid([0xAA; 32]))
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
            .chunk_digest_sha3_256([0xAC; 32])
            .por_root([0xAD; 32])
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
