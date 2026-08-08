use iroha_crypto::{KeyPair, PublicKey, Signature};
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

#[cfg(feature = "json")]
use crate::{DeriveJsonDeserialize, DeriveJsonSerialize};
use crate::{
    da::types::{
        BlobClass, BlobCodec, BlobDigest, Compression, DaRentQuote, ErasureProfile, ExtraMetadata,
        FecScheme, MetadataEncryption, MetadataVisibility, RetentionPolicy, StorageTicketId,
    },
    nexus::LaneId,
    sorafs::pin_registry::StorageClass,
};

/// Domain separator for version-one DA ingest request signatures.
pub const DA_INGEST_REQUEST_SIGNING_DOMAIN_V1: &[u8] = b"iroha:da-ingest-request:v1\0";

/// Summary of the 2D erasure layout captured in DA manifests/receipts.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema, Default,
)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(
    all(feature = "ffi_export", not(feature = "ffi_import")),
    derive(iroha_ffi::FfiType)
)]
#[cfg_attr(
    all(feature = "ffi_export", not(feature = "ffi_import")),
    ffi_type(opaque)
)]
pub struct DaStripeLayout {
    /// Total row stripes (data + column parity).
    pub total_stripes: u32,
    /// Total shards per stripe (data + row parity).
    pub shards_per_stripe: u32,
    /// Number of column-parity stripes across the matrix.
    #[norito(default)]
    pub row_parity_stripes: u16,
}

/// Norito payload accepted by the Torii `/v1/da/ingest` endpoint.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(
    all(feature = "ffi_export", not(feature = "ffi_import")),
    derive(iroha_ffi::FfiType)
)]
#[cfg_attr(
    all(feature = "ffi_export", not(feature = "ffi_import")),
    ffi_type(opaque)
)]
pub struct DaIngestRequest {
    /// Caller-supplied blob identifier (BLAKE3 digest or equivalent).
    pub client_blob_id: BlobDigest,
    /// Nexus lane the blob is attached to.
    pub lane_id: LaneId,
    /// Epoch the blob belongs to.
    pub epoch: u64,
    /// Monotonic sequence scoped to `(lane_id, epoch)` used for replay detection.
    pub sequence: u64,
    /// Semantic classification of the blob.
    pub blob_class: BlobClass,
    /// Codec label describing the payload.
    pub codec: BlobCodec,
    /// Erasure profile requested for chunking.
    pub erasure_profile: ErasureProfile,
    /// Retention policy requested/negotiated for the blob.
    pub retention_policy: RetentionPolicy,
    /// Chunk size in bytes (power-of-two, aligned with erasure profile).
    pub chunk_size: u32,
    /// Total payload size in bytes.
    pub total_size: u64,
    /// Compression applied to the payload. Defaults to identity (no compression).
    #[norito(default)]
    pub compression: Compression,
    /// Optional pre-generated Norito manifest supplied by the caller.
    #[cfg_attr(
        feature = "json",
        norito(with = "crate::json_helpers::base64_vec::option")
    )]
    pub norito_manifest: Option<Vec<u8>>,
    /// Raw payload bytes to be chunked and replicated.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::base64_vec"))]
    pub payload: Vec<u8>,
    /// Additional metadata entries for governance/analytics.
    pub metadata: ExtraMetadata,
    /// Public key of the submitter.
    pub submitter: PublicKey,
    /// Signature over the canonical digest of this request.
    pub signature: Signature,
}

/// Canonical version-one intent signed by a DA ingest submitter.
///
/// The submitter key is deliberately not duplicated in this payload: signature
/// verification already binds the intent to the `submitter` key carried by
/// [`DaIngestRequest`]. Every request field that can affect admission, storage,
/// accounting, or the resulting manifest is included.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct DaIngestRequestIntentV1 {
    /// Caller-supplied blob identifier.
    pub client_blob_id: BlobDigest,
    /// Nexus lane the blob is attached to.
    pub lane_id: LaneId,
    /// Epoch the blob belongs to.
    pub epoch: u64,
    /// Monotonic sequence scoped to `(lane_id, epoch)`.
    pub sequence: u64,
    /// Semantic classification of the blob.
    pub blob_class: BlobClass,
    /// Codec label describing the payload.
    pub codec: BlobCodec,
    /// Erasure profile requested for chunking.
    pub erasure_profile: ErasureProfile,
    /// Retention policy requested for the blob.
    pub retention_policy: RetentionPolicy,
    /// Chunk size in bytes.
    pub chunk_size: u32,
    /// Canonical payload size in bytes.
    pub total_size: u64,
    /// Compression applied to the transported payload.
    pub compression: Compression,
    /// Optional caller-provided Norito manifest.
    pub norito_manifest: Option<Vec<u8>>,
    /// Transported payload bytes.
    pub payload: Vec<u8>,
    /// Additional governance and analytics metadata.
    pub metadata: ExtraMetadata,
}

#[derive(Clone, Copy)]
struct DaIngestRequestIntentRefV1<'a> {
    client_blob_id: &'a BlobDigest,
    lane_id: LaneId,
    epoch: u64,
    sequence: u64,
    blob_class: BlobClass,
    codec: &'a BlobCodec,
    erasure_profile: ErasureProfile,
    retention_policy: &'a RetentionPolicy,
    chunk_size: u32,
    total_size: u64,
    compression: Compression,
    norito_manifest: &'a Option<Vec<u8>>,
    payload: &'a Vec<u8>,
    metadata: &'a ExtraMetadata,
}

impl<'a> From<&'a DaIngestRequestIntentV1> for DaIngestRequestIntentRefV1<'a> {
    fn from(intent: &'a DaIngestRequestIntentV1) -> Self {
        Self {
            client_blob_id: &intent.client_blob_id,
            lane_id: intent.lane_id,
            epoch: intent.epoch,
            sequence: intent.sequence,
            blob_class: intent.blob_class,
            codec: &intent.codec,
            erasure_profile: intent.erasure_profile,
            retention_policy: &intent.retention_policy,
            chunk_size: intent.chunk_size,
            total_size: intent.total_size,
            compression: intent.compression,
            norito_manifest: &intent.norito_manifest,
            payload: &intent.payload,
            metadata: &intent.metadata,
        }
    }
}

impl<'a> From<&'a DaIngestRequest> for DaIngestRequestIntentRefV1<'a> {
    fn from(request: &'a DaIngestRequest) -> Self {
        Self {
            client_blob_id: &request.client_blob_id,
            lane_id: request.lane_id,
            epoch: request.epoch,
            sequence: request.sequence,
            blob_class: request.blob_class,
            codec: &request.codec,
            erasure_profile: request.erasure_profile,
            retention_policy: &request.retention_policy,
            chunk_size: request.chunk_size,
            total_size: request.total_size,
            compression: request.compression,
            norito_manifest: &request.norito_manifest,
            payload: &request.payload,
            metadata: &request.metadata,
        }
    }
}

fn hash_len_prefixed(hasher: &mut blake3::Hasher, bytes: &[u8]) {
    let len = u64::try_from(bytes.len()).expect("in-memory DA field length must fit into u64");
    hasher.update(&len.to_le_bytes());
    hasher.update(bytes);
}

fn hash_tagged_u16(hasher: &mut blake3::Hasher, tag: u8, value: u16) {
    hasher.update(&[tag]);
    hasher.update(&value.to_le_bytes());
}

fn da_ingest_signing_digest(intent: &DaIngestRequestIntentRefV1<'_>) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(DA_INGEST_REQUEST_SIGNING_DOMAIN_V1);
    hasher.update(intent.client_blob_id.as_bytes());
    hasher.update(&intent.lane_id.as_u32().to_le_bytes());
    hasher.update(&intent.epoch.to_le_bytes());
    hasher.update(&intent.sequence.to_le_bytes());

    match intent.blob_class {
        BlobClass::TaikaiSegment => hash_tagged_u16(&mut hasher, 0, 0),
        BlobClass::NexusLaneSidecar => hash_tagged_u16(&mut hasher, 1, 0),
        BlobClass::GovernanceArtifact => hash_tagged_u16(&mut hasher, 2, 0),
        BlobClass::Custom(value) => hash_tagged_u16(&mut hasher, 3, value),
    }
    hash_len_prefixed(&mut hasher, intent.codec.0.as_bytes());

    hasher.update(&intent.erasure_profile.data_shards.to_le_bytes());
    hasher.update(&intent.erasure_profile.parity_shards.to_le_bytes());
    hasher.update(&intent.erasure_profile.row_parity_stripes.to_le_bytes());
    hasher.update(&intent.erasure_profile.chunk_alignment.to_le_bytes());
    match intent.erasure_profile.fec_scheme {
        FecScheme::Rs12_10 => hash_tagged_u16(&mut hasher, 0, 0),
        FecScheme::RsWin14_10 => hash_tagged_u16(&mut hasher, 1, 0),
        FecScheme::Rs18_14 => hash_tagged_u16(&mut hasher, 2, 0),
        FecScheme::Custom(value) => hash_tagged_u16(&mut hasher, 3, value),
    }

    hasher.update(&intent.retention_policy.hot_retention_secs.to_le_bytes());
    hasher.update(&intent.retention_policy.cold_retention_secs.to_le_bytes());
    hasher.update(&intent.retention_policy.required_replicas.to_le_bytes());
    let storage_class = match intent.retention_policy.storage_class {
        StorageClass::Hot => 0,
        StorageClass::Warm => 1,
        StorageClass::Cold => 2,
    };
    hasher.update(&[storage_class]);
    hash_len_prefixed(
        &mut hasher,
        intent.retention_policy.governance_tag.0.as_bytes(),
    );

    hasher.update(&intent.chunk_size.to_le_bytes());
    hasher.update(&intent.total_size.to_le_bytes());
    let compression = match intent.compression {
        Compression::Identity => 0,
        Compression::Gzip => 1,
        Compression::Deflate => 2,
        Compression::Zstd => 3,
    };
    hasher.update(&[compression]);

    match intent.norito_manifest {
        Some(manifest) => {
            hasher.update(&[1]);
            hash_len_prefixed(&mut hasher, manifest);
        }
        None => {
            hasher.update(&[0]);
        }
    }
    hash_len_prefixed(&mut hasher, intent.payload);

    let metadata_count =
        u64::try_from(intent.metadata.items.len()).expect("DA metadata count must fit into u64");
    hasher.update(&metadata_count.to_le_bytes());
    for entry in &intent.metadata.items {
        hash_len_prefixed(&mut hasher, entry.key.as_bytes());
        hash_len_prefixed(&mut hasher, &entry.value);
        let visibility = match entry.visibility {
            MetadataVisibility::Public => 0,
            MetadataVisibility::GovernanceOnly => 1,
        };
        hasher.update(&[visibility]);
        match &entry.encryption {
            MetadataEncryption::None => {
                hasher.update(&[0]);
            }
            MetadataEncryption::ChaCha20Poly1305(envelope) => {
                hasher.update(&[1]);
                match &envelope.key_label {
                    Some(label) => {
                        hasher.update(&[1]);
                        hash_len_prefixed(&mut hasher, label.as_bytes());
                    }
                    None => {
                        hasher.update(&[0]);
                    }
                }
            }
        }
    }
    *hasher.finalize().as_bytes()
}

impl DaIngestRequestIntentV1 {
    /// Compute the domain-separated digest signed by the DA submitter.
    #[must_use]
    pub fn signing_digest(&self) -> [u8; 32] {
        da_ingest_signing_digest(&self.into())
    }

    /// Sign this intent and construct the corresponding ingest request.
    ///
    /// # Errors
    ///
    /// Returns an error when the configured signing backend rejects the key or
    /// cannot create a signature.
    pub fn try_sign(self, key_pair: &KeyPair) -> Result<DaIngestRequest, iroha_crypto::Error> {
        let signature = Signature::try_new(key_pair.private_key(), &self.signing_digest())?;
        Ok(DaIngestRequest {
            client_blob_id: self.client_blob_id,
            lane_id: self.lane_id,
            epoch: self.epoch,
            sequence: self.sequence,
            blob_class: self.blob_class,
            codec: self.codec,
            erasure_profile: self.erasure_profile,
            retention_policy: self.retention_policy,
            chunk_size: self.chunk_size,
            total_size: self.total_size,
            compression: self.compression,
            norito_manifest: self.norito_manifest,
            payload: self.payload,
            metadata: self.metadata,
            submitter: key_pair.public_key().clone(),
            signature,
        })
    }
}

impl DaIngestRequest {
    /// Compute the domain-separated digest covering every signable request field.
    #[must_use]
    pub fn signing_digest(&self) -> [u8; 32] {
        da_ingest_signing_digest(&self.into())
    }

    /// Verify the request signature against its declared submitter.
    ///
    /// # Errors
    ///
    /// Returns a cryptographic verification error when the signature, key, or
    /// signed intent is invalid.
    pub fn verify_signature(&self) -> Result<(), iroha_crypto::Error> {
        self.signature
            .verify(&self.submitter, &self.signing_digest())
    }
}

/// Ingest receipt returned once Torii accepts the blob.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(
    all(feature = "ffi_export", not(feature = "ffi_import")),
    derive(iroha_ffi::FfiType)
)]
#[cfg_attr(
    all(feature = "ffi_export", not(feature = "ffi_import")),
    ffi_type(opaque)
)]
pub struct DaIngestReceipt {
    /// Caller-supplied blob identifier echoed back to the submitter.
    pub client_blob_id: BlobDigest,
    /// Nexus lane associated with the blob.
    pub lane_id: LaneId,
    /// Epoch recorded for the blob.
    pub epoch: u64,
    /// Blake3 digest of the raw payload.
    pub blob_hash: BlobDigest,
    /// Merkle root computed from chunk commitments.
    pub chunk_root: BlobDigest,
    /// Blake3 digest of the canonical Norito manifest.
    pub manifest_hash: BlobDigest,
    /// Storage ticket identifier issued by the orchestrator.
    pub storage_ticket: StorageTicketId,
    /// Norito-encoded PDP commitment derived from the accepted payload.
    #[cfg_attr(
        feature = "json",
        norito(with = "crate::json_helpers::base64_vec::option")
    )]
    #[norito(default)]
    pub pdp_commitment: Option<Vec<u8>>,
    /// Erasure layout summary for the admitted manifest.
    #[norito(default)]
    pub stripe_layout: DaStripeLayout,
    /// Unix timestamp (seconds) when the blob was accepted.
    pub queued_at_unix: u64,
    /// Rent and incentive breakdown quoted at ingest time.
    #[norito(default)]
    pub rent_quote: DaRentQuote,
    /// Signature generated by the Torii DA service.
    pub operator_signature: Signature,
}
