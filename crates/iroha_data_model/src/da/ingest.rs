use iroha_crypto::{Hash, KeyPair, PublicKey, Signature};
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

#[cfg(feature = "json")]
use crate::{DeriveJsonDeserialize, DeriveJsonSerialize};
use crate::{
    NetworkId,
    account::AccountId,
    da::types::{
        BlobClass, BlobCodec, BlobDigest, Compression, DaRentQuote, ErasureProfile, ExtraMetadata,
        FecScheme, MetadataEncryption, MetadataVisibility, RetentionPolicy, StorageTicketId,
    },
    nexus::LaneId,
    sorafs::pin_registry::StorageClass,
};

/// Domain separator for version-one DA ingest request signatures.
pub const DA_INGEST_REQUEST_SIGNING_DOMAIN_V1: &[u8] = b"iroha:da-ingest-request:v1\0";

/// Domain separator for the immutable request-content commitment carried into consensus.
pub const DA_INGEST_REQUEST_CONTENT_DOMAIN_V1: &[u8] = b"iroha:da-ingest-request:content:v1\0";

/// One canonical account-controller signature over a DA ingest authorization.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct DaIngestSignatureV1 {
    /// Account-controller key that produced the signature.
    pub signer: PublicKey,
    /// Signature over [`DaIngestAuthorizationV1::signing_digest`].
    pub signature: Signature,
}

/// Minimal immutable DA admission authorization committed into a block sidecar.
///
/// The request-content commitment keeps the consensus payload compact while the
/// signed quota identity remains independently verifiable by every validator.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct DaIngestAuthorizationV1 {
    /// Exact genesis-derived network identity authorising this admission.
    pub network_id: NetworkId,
    /// Account whose deterministic consensus quota is charged.
    pub owner: AccountId,
    /// Nexus lane the blob is attached to.
    pub lane_id: LaneId,
    /// Epoch the blob belongs to.
    pub epoch: u64,
    /// Monotonic sequence scoped to `(lane_id, epoch)` and used as the replay nonce.
    pub sequence: u64,
    /// BLAKE3 commitment to the canonical, decompressed payload bytes.
    pub payload_hash: BlobDigest,
    /// Exact canonical payload length charged to the owner's quota.
    pub payload_bytes: u64,
    /// Commitment to every remaining signed request field.
    pub request_content_hash: Hash,
    /// Canonically signer-key-ordered account-controller witnesses.
    pub signatures: Vec<DaIngestSignatureV1>,
}

impl DaIngestAuthorizationV1 {
    /// Compute the exact digest signed by every account-controller witness.
    #[must_use]
    pub fn signing_digest(&self) -> [u8; 32] {
        let mut hasher = blake3::Hasher::new();
        hasher.update(DA_INGEST_REQUEST_SIGNING_DOMAIN_V1);
        hasher.update(self.network_id.as_bytes());
        let owner = self
            .owner
            .to_account_address()
            .and_then(|address| address.canonical_bytes())
            .expect("a validated AccountId must have canonical controller bytes");
        hash_len_prefixed(&mut hasher, &owner);
        hasher.update(&self.lane_id.as_u32().to_le_bytes());
        hasher.update(&self.epoch.to_le_bytes());
        hasher.update(&self.sequence.to_le_bytes());
        hasher.update(self.payload_hash.as_bytes());
        hasher.update(&self.payload_bytes.to_le_bytes());
        hasher.update(self.request_content_hash.as_ref());
        *hasher.finalize().as_bytes()
    }

    /// Return whether witnesses are non-empty, strictly signer ordered, and individually valid.
    #[must_use]
    pub fn has_valid_canonical_signatures(&self) -> bool {
        if self.signatures.is_empty()
            || self
                .signatures
                .windows(2)
                .any(|pair| pair[0].signer >= pair[1].signer)
        {
            return false;
        }
        let digest = self.signing_digest();
        self.signatures
            .iter()
            .all(|witness| witness.signature.verify(&witness.signer, &digest).is_ok())
    }
}

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
    /// Exact genesis-derived network identity authorising this request.
    pub network_id: NetworkId,
    /// Authenticated account whose consensus DA quota is charged.
    pub owner: AccountId,
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
    /// BLAKE3 commitment to the canonical, decompressed payload bytes.
    pub payload_hash: BlobDigest,
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
    /// Canonically signer-key-ordered account-controller witnesses.
    pub signatures: Vec<DaIngestSignatureV1>,
}

/// Canonical version-one intent signed by a DA ingest submitter.
///
/// Signer witnesses live on [`DaIngestRequest`] so every controller key signs
/// one identical digest. Every request field that can affect admission,
/// storage, accounting, or the resulting manifest is committed.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct DaIngestRequestIntentV1 {
    /// Exact genesis-derived network identity authorising this intent.
    pub network_id: NetworkId,
    /// Authenticated account whose consensus DA quota is charged.
    pub owner: AccountId,
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
    /// BLAKE3 commitment to the canonical, decompressed payload bytes.
    pub payload_hash: BlobDigest,
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
    blob_class: BlobClass,
    codec: &'a BlobCodec,
    erasure_profile: ErasureProfile,
    retention_policy: &'a RetentionPolicy,
    chunk_size: u32,
    compression: Compression,
    norito_manifest: &'a Option<Vec<u8>>,
    payload: &'a Vec<u8>,
    metadata: &'a ExtraMetadata,
}

impl<'a> From<&'a DaIngestRequestIntentV1> for DaIngestRequestIntentRefV1<'a> {
    fn from(intent: &'a DaIngestRequestIntentV1) -> Self {
        Self {
            client_blob_id: &intent.client_blob_id,
            blob_class: intent.blob_class,
            codec: &intent.codec,
            erasure_profile: intent.erasure_profile,
            retention_policy: &intent.retention_policy,
            chunk_size: intent.chunk_size,
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
            blob_class: request.blob_class,
            codec: &request.codec,
            erasure_profile: request.erasure_profile,
            retention_policy: &request.retention_policy,
            chunk_size: request.chunk_size,
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

fn da_ingest_request_content_hash(intent: &DaIngestRequestIntentRefV1<'_>) -> Hash {
    let mut hasher = blake3::Hasher::new();
    hasher.update(DA_INGEST_REQUEST_CONTENT_DOMAIN_V1);
    hasher.update(intent.client_blob_id.as_bytes());

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
    Hash::prehashed(*hasher.finalize().as_bytes())
}

impl DaIngestRequestIntentV1 {
    fn unsigned_authorization(&self) -> DaIngestAuthorizationV1 {
        DaIngestAuthorizationV1 {
            network_id: self.network_id,
            owner: self.owner.clone(),
            lane_id: self.lane_id,
            epoch: self.epoch,
            sequence: self.sequence,
            payload_hash: self.payload_hash,
            payload_bytes: self.total_size,
            request_content_hash: da_ingest_request_content_hash(&self.into()),
            signatures: Vec::new(),
        }
    }

    /// Compute the domain-separated digest signed by each account-controller key.
    #[must_use]
    pub fn signing_digest(&self) -> [u8; 32] {
        self.unsigned_authorization().signing_digest()
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
            network_id: self.network_id,
            owner: self.owner,
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
            payload_hash: self.payload_hash,
            compression: self.compression,
            norito_manifest: self.norito_manifest,
            payload: self.payload,
            metadata: self.metadata,
            signatures: vec![DaIngestSignatureV1 {
                signer: key_pair.public_key().clone(),
                signature,
            }],
        })
    }
}

impl DaIngestRequest {
    /// Project the compact immutable authorization committed into the pin-intent sidecar.
    #[must_use]
    pub fn authorization(&self) -> DaIngestAuthorizationV1 {
        DaIngestAuthorizationV1 {
            network_id: self.network_id,
            owner: self.owner.clone(),
            lane_id: self.lane_id,
            epoch: self.epoch,
            sequence: self.sequence,
            payload_hash: self.payload_hash,
            payload_bytes: self.total_size,
            request_content_hash: da_ingest_request_content_hash(&self.into()),
            signatures: self.signatures.clone(),
        }
    }

    /// Compute the domain-separated digest covering every signable request field.
    #[must_use]
    pub fn signing_digest(&self) -> [u8; 32] {
        self.authorization().signing_digest()
    }

    /// Add one account-controller witness and restore canonical signer ordering.
    ///
    /// # Errors
    ///
    /// Returns an error when signing fails or the signer is already present.
    pub fn try_add_signature(&mut self, key_pair: &KeyPair) -> Result<(), iroha_crypto::Error> {
        let signer = key_pair.public_key();
        if self
            .signatures
            .iter()
            .any(|witness| &witness.signer == signer)
        {
            return Err(iroha_crypto::Error::Other(
                "duplicate DA ingest authorization signer".to_owned(),
            ));
        }
        let signature = Signature::try_new(key_pair.private_key(), &self.signing_digest())?;
        self.signatures.push(DaIngestSignatureV1 {
            signer: signer.clone(),
            signature,
        });
        self.signatures
            .sort_by(|left, right| left.signer.cmp(&right.signer));
        Ok(())
    }

    /// Verify that every request witness is canonical and cryptographically valid.
    ///
    /// # Errors
    ///
    /// Returns [`iroha_crypto::Error::BadSignature`] for an empty, duplicate,
    /// non-canonical, or invalid witness set.
    pub fn verify_signatures(&self) -> Result<(), iroha_crypto::Error> {
        if self.authorization().has_valid_canonical_signatures() {
            Ok(())
        } else {
            Err(iroha_crypto::Error::BadSignature)
        }
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
