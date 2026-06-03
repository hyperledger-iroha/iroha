//! Zero-knowledge envelope types (Norito TLV payloads).

use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

use crate::{AssetDefinitionId, ChainId, account::AccountId, proof::VerifyingKeyId};

/// Canonical ZK-ACE circuit identifier for post-quantum authorization v0.
pub const ZK_ACE_PQ_AUTHORIZATION_V0_CIRCUIT_ID: &str = "zk_ace_pq_authorization_v0";

/// Production backend label used by ZK-ACE authorization v0.
pub const ZK_ACE_PQ_AUTHORIZATION_V0_BACKEND: &str = "stark/fri/sha256-goldilocks";

/// Domain tag used when deriving ZK-ACE identity commitments and replay nullifiers.
pub const ZK_ACE_PQ_AUTHORIZATION_V0_DOMAIN_TAG: &str = "iroha:zk-ace:pq-authorization:v0";

/// First executable ZK-ACE action class.
pub const ZK_ACE_PQ_AUTHORIZATION_V0_ACTION_TRANSFER: &str = "transparent_asset_transfer";

/// Maximum source accounts that one ZK-ACE identity commitment may authorize.
pub const ZK_ACE_MAX_ALLOWED_ACCOUNTS: usize = 16;

/// Number of bytes packed into each Goldilocks field limb for ZK-ACE hashes.
pub const ZK_ACE_PACKED_LIMB_BYTES: usize = 7;

/// Default maximum proof payload size accepted by generic OpenVerify admission.
pub const OPEN_VERIFY_DEFAULT_MAX_PROOF_BYTES: usize = 64 * 1024 * 1024;

/// Default maximum public-input metadata size accepted by generic OpenVerify admission.
pub const OPEN_VERIFY_DEFAULT_MAX_PUBLIC_INPUT_BYTES: usize = 1024 * 1024;

/// Default maximum auxiliary metadata size for non-admission OpenVerify callers.
pub const OPEN_VERIFY_DEFAULT_MAX_AUX_BYTES: usize = 64 * 1024;

/// Backend tag for zero-knowledge verifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
pub enum BackendTag {
    /// Halo2 IPA over Pasta curves
    Halo2IpaPasta,
    /// Halo2 over BN254 (optional)
    Halo2Bn254,
    /// Groth16 generic backend marker.
    Groth16,
    /// STARK/FRI (transparent, no trusted setup)
    Stark,
    /// Unknown/unsupported backend
    Unsupported,
    /// Zcash Orchard Halo2/IPA action-bundle backend.
    Halo2IpaOrchard,
    /// Penumbra MASP Groth16 backend over BLS12-377/Decaf377.
    Groth16Bls12377,
    /// Monero FCMP++ curve-tree membership backend.
    FcmpPlusPlusCurveTree,
    /// Lattice polynomial-commitment/SIS backend.
    LatticePcsSis,
    /// Miden STARK note-transaction backend.
    MidenStark,
    /// Aztec plonkish private-kernel backend.
    AztecPlonkishPrivateKernel,
    /// Post-quantum MASP STARK/FRI backend.
    PqMaspStarkFri,
    /// Anonymous PGC k-out-of-n backend.
    AnonymousPgc,
    /// VeRange transparent range-proof backend.
    VeRange,
    /// zkAt policy-private authenticator backend.
    ZkAt,
    /// ZK-AMS recursive anonymous admission backend.
    RecursiveAnonymousAdmission,
    /// Vega existing-credential ZK backend.
    VegaExistingCredentialZk,
    /// Silent-threshold anonymous credential backend.
    SilentThresholdAnoncred,
    /// ZK-X.509 on-chain identity backend.
    ZkX509,
    /// SIS-with-hints anonymous credential backend.
    SisWithHints,
}

impl BackendTag {
    /// Return the canonical JSON/catalog label for this backend tag.
    #[must_use]
    pub const fn canonical_label(self) -> &'static str {
        match self {
            BackendTag::Halo2IpaPasta => "halo2-ipa-pasta",
            BackendTag::Halo2Bn254 => "halo2-bn254",
            BackendTag::Groth16 => "groth16",
            BackendTag::Stark => "stark",
            BackendTag::Unsupported => "unsupported",
            BackendTag::Halo2IpaOrchard => "halo2-ipa-orchard",
            BackendTag::Groth16Bls12377 => "groth16-bls12-377",
            BackendTag::FcmpPlusPlusCurveTree => "fcmp-plus-plus-curve-tree",
            BackendTag::LatticePcsSis => "lattice-pcs-sis",
            BackendTag::MidenStark => "miden-stark",
            BackendTag::AztecPlonkishPrivateKernel => "aztec-plonkish-private-kernel",
            BackendTag::PqMaspStarkFri => "pq-masp-stark-fri",
            BackendTag::AnonymousPgc => "anonymous-pgc",
            BackendTag::VeRange => "verange",
            BackendTag::ZkAt => "zkat",
            BackendTag::RecursiveAnonymousAdmission => "recursive-anonymous-admission",
            BackendTag::VegaExistingCredentialZk => "vega-existing-credential-zk",
            BackendTag::SilentThresholdAnoncred => "silent-threshold-anoncred",
            BackendTag::ZkX509 => "zk-x509",
            BackendTag::SisWithHints => "sis-with-hints",
        }
    }

    /// Return true for exact protocol-family tags that are cataloged but not
    /// production-admissible until a real engine and external audit are wired.
    #[must_use]
    pub const fn is_pending_production_backend(self) -> bool {
        matches!(
            self,
            BackendTag::Halo2IpaOrchard
                | BackendTag::Groth16Bls12377
                | BackendTag::FcmpPlusPlusCurveTree
                | BackendTag::LatticePcsSis
                | BackendTag::MidenStark
                | BackendTag::AztecPlonkishPrivateKernel
                | BackendTag::PqMaspStarkFri
                | BackendTag::AnonymousPgc
                | BackendTag::VeRange
                | BackendTag::ZkAt
                | BackendTag::RecursiveAnonymousAdmission
                | BackendTag::VegaExistingCredentialZk
                | BackendTag::SilentThresholdAnoncred
                | BackendTag::ZkX509
                | BackendTag::SisWithHints
        )
    }

    /// Parse a backend label from catalog, SDK, CLI, or Torii input into the
    /// closest explicit backend tag.
    ///
    /// Exact pending-production protocol families are checked before broad
    /// backend-family aliases so labels such as `halo2/ipa/orchard` and
    /// `groth16/bls12-377` remain fail-closed instead of collapsing into
    /// generic Halo2 or Groth16 families.
    #[must_use]
    pub fn from_catalog_label(raw: &str) -> Self {
        let label = raw.trim().to_ascii_lowercase();
        if label.is_empty() {
            return BackendTag::Unsupported;
        }
        let compact = label
            .chars()
            .filter(|ch| ch.is_ascii_alphanumeric())
            .collect::<String>();

        if label == "unsupported" || compact == "unsupported" {
            return BackendTag::Unsupported;
        }

        if compact.contains("pqmasp") || compact.contains("postquantummasp") {
            return BackendTag::PqMaspStarkFri;
        }
        if compact.contains("anonymouspgc") || compact.contains("pgckoutofn") {
            return BackendTag::AnonymousPgc;
        }
        if compact.contains("verange") {
            return BackendTag::VeRange;
        }
        if compact.contains("zkat") || compact.contains("policyprivateauthenticator") {
            return BackendTag::ZkAt;
        }
        if compact.contains("zkams") || compact.contains("recursiveanonymousadmission") {
            return BackendTag::RecursiveAnonymousAdmission;
        }
        if compact.contains("vega") || compact.contains("existingcredentialzk") {
            return BackendTag::VegaExistingCredentialZk;
        }
        if compact.contains("silentthreshold") || compact.contains("thresholdanonymouscredential") {
            return BackendTag::SilentThresholdAnoncred;
        }
        if compact.contains("zkx509") || compact.contains("x509") || compact.contains("zkvmx509") {
            return BackendTag::ZkX509;
        }
        if compact.contains("siswithhints")
            || compact.contains("sishints")
            || compact.contains("latticeanonymouscredentials")
        {
            return BackendTag::SisWithHints;
        }
        if compact.contains("orchard") || compact.contains("zcashorchard") {
            return BackendTag::Halo2IpaOrchard;
        }
        if compact.contains("penumbra")
            || compact.contains("masp")
            || compact.contains("bls12377")
            || compact.contains("decaf377")
        {
            return BackendTag::Groth16Bls12377;
        }
        if compact.contains("fcmp") || compact.contains("monero") || compact.contains("curvetree") {
            return BackendTag::FcmpPlusPlusCurveTree;
        }
        if compact.contains("lattice") || compact.contains("pcssis") || compact.contains("jindo") {
            return BackendTag::LatticePcsSis;
        }
        if compact.contains("miden") {
            return BackendTag::MidenStark;
        }
        if compact.contains("aztec") {
            return BackendTag::AztecPlonkishPrivateKernel;
        }

        if compact.contains("halo2") && compact.contains("bn254") {
            return BackendTag::Halo2Bn254;
        }
        if compact.contains("groth16") {
            return BackendTag::Groth16;
        }
        if compact.contains("stark") {
            return BackendTag::Stark;
        }
        if compact == "halo2ipa"
            || compact == "halo2ipapasta"
            || compact == "halo2pasta"
            || (compact.contains("halo2") && (compact.contains("ipa") || compact.contains("pasta")))
        {
            return BackendTag::Halo2IpaPasta;
        }

        BackendTag::Unsupported
    }

    /// Return true when a raw backend label names a cataloged protocol family
    /// whose production engine and audit gates are still pending.
    #[must_use]
    pub fn is_pending_production_backend_label(raw: &str) -> bool {
        Self::from_catalog_label(raw).is_pending_production_backend()
    }
}

#[cfg(feature = "json")]
impl norito::json::JsonSerialize for BackendTag {
    fn json_serialize(&self, out: &mut String) {
        norito::json::write_json_string(self.canonical_label(), out);
    }
}

#[cfg(feature = "json")]
impl norito::json::JsonDeserialize for BackendTag {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        Ok(BackendTag::from_catalog_label(&parser.parse_string()?))
    }
}

/// Size and policy bounds for validating an [`OpenVerifyEnvelope`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpenVerifyEnvelopeBounds {
    /// Maximum public-input metadata bytes.
    pub max_public_input_bytes: usize,
    /// Maximum backend proof bytes.
    pub max_proof_bytes: usize,
    /// Maximum auxiliary metadata bytes.
    pub max_aux_bytes: usize,
    /// Whether auxiliary bytes are allowed at all.
    pub allow_aux: bool,
    /// Whether the verifier-key hash must be non-zero.
    pub require_nonzero_vk_hash: bool,
    /// Whether exact protocol-family backends that are cataloged but pending
    /// production engine/audit gates may pass this generic shape validation.
    pub allow_pending_production_backends: bool,
}

impl Default for OpenVerifyEnvelopeBounds {
    fn default() -> Self {
        Self {
            max_public_input_bytes: OPEN_VERIFY_DEFAULT_MAX_PUBLIC_INPUT_BYTES,
            max_proof_bytes: OPEN_VERIFY_DEFAULT_MAX_PROOF_BYTES,
            max_aux_bytes: OPEN_VERIFY_DEFAULT_MAX_AUX_BYTES,
            allow_aux: false,
            require_nonzero_vk_hash: true,
            allow_pending_production_backends: false,
        }
    }
}

/// Validation failure for a generic [`OpenVerifyEnvelope`] admission check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenVerifyEnvelopeValidationError {
    /// The envelope uses the explicit unsupported backend marker.
    UnsupportedBackend,
    /// The envelope uses a cataloged production backend whose engine/audit gate
    /// is not enabled for generic chain admission.
    PendingProductionBackend {
        /// Backend that remains fail-closed.
        backend: BackendTag,
    },
    /// The circuit identifier is empty or whitespace-only.
    EmptyCircuitId,
    /// The verifier-key hash is all zeros.
    ZeroVerifierKeyHash,
    /// The public-input metadata is empty.
    EmptyPublicInputs,
    /// The public-input metadata exceeds configured bounds.
    PublicInputsTooLarge {
        /// Observed public-input metadata length.
        len: usize,
        /// Configured maximum public-input metadata length.
        max: usize,
    },
    /// The proof byte payload is empty.
    EmptyProofBytes,
    /// The proof byte payload exceeds configured bounds.
    ProofBytesTooLarge {
        /// Observed proof byte length.
        len: usize,
        /// Configured maximum proof byte length.
        max: usize,
    },
    /// The envelope carries auxiliary metadata where admission forbids it.
    NonEmptyAux,
    /// The auxiliary metadata exceeds configured bounds.
    AuxTooLarge {
        /// Observed auxiliary metadata length.
        len: usize,
        /// Configured maximum auxiliary metadata length.
        max: usize,
    },
}

impl core::fmt::Display for OpenVerifyEnvelopeValidationError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::UnsupportedBackend => write!(f, "OpenVerifyEnvelope backend is unsupported"),
            Self::PendingProductionBackend { backend } => write!(
                f,
                "OpenVerifyEnvelope backend {} is pending production engine and audit gates",
                backend.canonical_label()
            ),
            Self::EmptyCircuitId => write!(f, "OpenVerifyEnvelope circuit id is empty"),
            Self::ZeroVerifierKeyHash => {
                write!(f, "OpenVerifyEnvelope verifier-key hash is zero")
            }
            Self::EmptyPublicInputs => {
                write!(f, "OpenVerifyEnvelope public inputs are empty")
            }
            Self::PublicInputsTooLarge { len, max } => write!(
                f,
                "OpenVerifyEnvelope public inputs length {len} exceeds maximum {max}"
            ),
            Self::EmptyProofBytes => write!(f, "OpenVerifyEnvelope proof bytes are empty"),
            Self::ProofBytesTooLarge { len, max } => write!(
                f,
                "OpenVerifyEnvelope proof bytes length {len} exceeds maximum {max}"
            ),
            Self::NonEmptyAux => {
                write!(f, "OpenVerifyEnvelope auxiliary bytes must be empty")
            }
            Self::AuxTooLarge { len, max } => write!(
                f,
                "OpenVerifyEnvelope auxiliary bytes length {len} exceeds maximum {max}"
            ),
        }
    }
}

impl std::error::Error for OpenVerifyEnvelopeValidationError {}

/// Envelope for open-verify operations (canonical `SignedQuery` layout).
///
/// This structure is serialized with Norito and used as the TLV payload for
/// `&NoritoBytes` pointer-ABI types passed to IVM verify syscalls or host vendor bridges.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct OpenVerifyEnvelope {
    /// Backend tag string (e.g., `halo2-ipa-pasta`).
    pub backend: BackendTag,
    /// Circuit identifier string (backend-specific; opaque to host).
    pub circuit_id: String,
    /// Domain-separated verifying-key hash.
    ///
    /// Generic codecs may still represent an unavailable key binding as all
    /// zeros, but chain admission for registered proof attachments requires an
    /// exact match with the active verifier-key commitment.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub vk_hash: [u8; 32],
    /// Public-input metadata bytes (opaque; backend-specific canonical encoding).
    ///
    /// For backends that separate schema from values (e.g., `stark/fri` wrappers),
    /// this field carries the stable schema descriptor while concrete values are
    /// stored inside backend-specific payloads.
    pub public_inputs: Vec<u8>,
    /// Proof bytes (opaque, backend-specific canonical encoding).
    pub proof_bytes: Vec<u8>,
    /// Opaque aux map encoded as JSON bytes (for small structured extras).
    ///
    /// Production chain proof-admission paths require this to be empty unless a
    /// future instruction explicitly defines and validates auxiliary semantics.
    pub aux: Vec<u8>,
}

impl OpenVerifyEnvelope {
    /// Create a new envelope with required fields; `aux` defaults to empty.
    pub fn new(
        backend: BackendTag,
        circuit_id: impl Into<String>,
        vk_hash: [u8; 32],
        public_inputs: Vec<u8>,
        proof_bytes: Vec<u8>,
    ) -> Self {
        Self {
            backend,
            circuit_id: circuit_id.into(),
            vk_hash,
            public_inputs,
            proof_bytes,
            aux: Vec::new(),
        }
    }

    /// Validate the generic shape required before registered proof admission.
    ///
    /// Backend-specific verifiers must still validate circuit semantics,
    /// public-input interpretation, verifier-key registry matching, and proof
    /// verification. This method only enforces the shared fail-closed envelope
    /// invariants that all admitted proof paths rely on.
    pub fn validate_for_admission(&self) -> Result<(), OpenVerifyEnvelopeValidationError> {
        self.validate_with_bounds(OpenVerifyEnvelopeBounds::default())
    }

    /// Validate generic shape and size invariants with caller-provided bounds.
    pub fn validate_with_bounds(
        &self,
        bounds: OpenVerifyEnvelopeBounds,
    ) -> Result<(), OpenVerifyEnvelopeValidationError> {
        if self.backend == BackendTag::Unsupported {
            return Err(OpenVerifyEnvelopeValidationError::UnsupportedBackend);
        }
        if self.backend.is_pending_production_backend() && !bounds.allow_pending_production_backends
        {
            return Err(
                OpenVerifyEnvelopeValidationError::PendingProductionBackend {
                    backend: self.backend,
                },
            );
        }
        if self.circuit_id.trim().is_empty() {
            return Err(OpenVerifyEnvelopeValidationError::EmptyCircuitId);
        }
        if bounds.require_nonzero_vk_hash && self.vk_hash.iter().all(|byte| *byte == 0) {
            return Err(OpenVerifyEnvelopeValidationError::ZeroVerifierKeyHash);
        }
        if self.public_inputs.is_empty() {
            return Err(OpenVerifyEnvelopeValidationError::EmptyPublicInputs);
        }
        if self.public_inputs.len() > bounds.max_public_input_bytes {
            return Err(OpenVerifyEnvelopeValidationError::PublicInputsTooLarge {
                len: self.public_inputs.len(),
                max: bounds.max_public_input_bytes,
            });
        }
        if self.proof_bytes.is_empty() {
            return Err(OpenVerifyEnvelopeValidationError::EmptyProofBytes);
        }
        if self.proof_bytes.len() > bounds.max_proof_bytes {
            return Err(OpenVerifyEnvelopeValidationError::ProofBytesTooLarge {
                len: self.proof_bytes.len(),
                max: bounds.max_proof_bytes,
            });
        }
        if !bounds.allow_aux && !self.aux.is_empty() {
            return Err(OpenVerifyEnvelopeValidationError::NonEmptyAux);
        }
        if self.aux.len() > bounds.max_aux_bytes {
            return Err(OpenVerifyEnvelopeValidationError::AuxTooLarge {
                len: self.aux.len(),
                max: bounds.max_aux_bytes,
            });
        }
        Ok(())
    }
}

// Note: Norito serialization is derived via `Encode`/`Decode` (packed structs compatible)

/// STARK/FRI proof payload embedded inside [`OpenVerifyEnvelope::proof_bytes`] when
/// [`OpenVerifyEnvelope::backend`] is [`BackendTag::Stark`].
///
/// This wrapper carries:
/// - `public_inputs`: public inputs expressed as 32-byte words, column-major (matching
///   the instance-column layout used by Halo2 envelopes), and
/// - `envelope_bytes`: backend-native proof bytes (typically a Norito-encoded STARK/FRI
///   envelope such as `StarkVerifyEnvelopeV1`).
///
/// Higher-level flows (governance voting, `Executable::IvmProved`, etc.) interpret the public
/// inputs according to the circuit/policy definitions and must validate their semantics.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct StarkFriOpenProofV1 {
    /// Version tag for format evolution.
    pub version: u16,
    /// Public inputs encoded as 32-byte words, column-major.
    pub public_inputs: Vec<Vec<[u8; 32]>>,
    /// Backend-native proof envelope bytes.
    pub envelope_bytes: Vec<u8>,
}

/// Canonical public input record proven by `zk_ace_pq_authorization_v0`.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ZkAcePublicInputsV1 {
    /// Public-input schema version.
    pub version: u16,
    /// On-chain identity commitment being authorized.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub identity_commitment: [u8; 32],
    /// Digest of the visible action fields.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub tx_digest: [u8; 32],
    /// Chain id bound into the replay-nullifier domain.
    pub chain_id: ChainId,
    /// Domain separation tag.
    pub domain_tag: String,
    /// Action class authorized by this proof.
    pub action_class: String,
    /// Replay-prevention nullifier derived inside the circuit.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub replay_nullifier: [u8; 32],
    /// Policy hash bound to the registered identity record.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub policy_hash: [u8; 32],
    /// Source account whose transfer authority is proven.
    pub from: AccountId,
    /// Destination account.
    pub to: AccountId,
    /// Transparent asset definition being transferred.
    pub asset: AssetDefinitionId,
    /// Transparent amount being transferred.
    pub amount: u128,
    /// Verifier key that must validate the proof.
    pub verifier_key_id: VerifyingKeyId,
}

/// Private witness used by the ZK-ACE prover.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ZkAceWitnessV1 {
    /// External DIDP identity root.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub identity_root: [u8; 32],
    /// Identity blinding factor.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub identity_blinding: [u8; 32],
    /// Replay secret used to derive per-action nullifiers.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub replay_secret: [u8; 32],
}

/// Canonical byte packing used by ZK-ACE Poseidon2-domain hashing.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ZkAcePackedBytesV1 {
    /// Original byte length before padding.
    pub length: u64,
    /// Little-endian 7-byte Goldilocks limbs.
    pub limbs: Vec<u64>,
}

impl ZkAcePublicInputsV1 {
    /// Construct v1 public inputs for the transparent-transfer action.
    #[allow(clippy::too_many_arguments)]
    pub fn transparent_transfer(
        identity_commitment: [u8; 32],
        tx_digest: [u8; 32],
        chain_id: ChainId,
        replay_nullifier: [u8; 32],
        policy_hash: [u8; 32],
        from: AccountId,
        to: AccountId,
        asset: AssetDefinitionId,
        amount: u128,
        verifier_key_id: VerifyingKeyId,
    ) -> Self {
        Self {
            version: 1,
            identity_commitment,
            tx_digest,
            chain_id,
            domain_tag: ZK_ACE_PQ_AUTHORIZATION_V0_DOMAIN_TAG.to_owned(),
            action_class: ZK_ACE_PQ_AUTHORIZATION_V0_ACTION_TRANSFER.to_owned(),
            replay_nullifier,
            policy_hash,
            from,
            to,
            asset,
            amount,
            verifier_key_id,
        }
    }
}

/// Pack arbitrary bytes into canonical 7-byte Goldilocks limbs.
#[must_use]
pub fn zk_ace_pack_bytes_to_field_limbs(bytes: &[u8]) -> ZkAcePackedBytesV1 {
    let mut limbs = Vec::with_capacity(bytes.len().div_ceil(ZK_ACE_PACKED_LIMB_BYTES));
    let mut offset = 0usize;
    while offset < bytes.len() {
        let take = core::cmp::min(ZK_ACE_PACKED_LIMB_BYTES, bytes.len() - offset);
        let mut chunk = [0u8; 8];
        chunk[..take].copy_from_slice(&bytes[offset..offset + take]);
        limbs.push(u64::from_le_bytes(chunk));
        offset += take;
    }
    ZkAcePackedBytesV1 {
        length: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
        limbs,
    }
}

/// Domain-separated Poseidon2 hash over already canonical byte parts.
#[must_use]
pub fn zk_ace_poseidon2_domain_hash(domain: &[u8], parts: &[&[u8]]) -> [u8; 32] {
    let words = zk_ace_poseidon2_domain_words(domain, parts);
    let mut sponge = fastpq_isi::poseidon::PoseidonSponge::new();
    sponge.absorb_slice(&words);

    let mut out = [0u8; 32];
    for chunk in out.chunks_exact_mut(core::mem::size_of::<u64>()) {
        chunk.copy_from_slice(&sponge.squeeze_element().to_le_bytes());
    }
    out
}

/// Canonical Goldilocks field preimage used by ZK-ACE Poseidon2-domain hashing.
#[must_use]
pub fn zk_ace_poseidon2_domain_words(domain: &[u8], parts: &[&[u8]]) -> Vec<u64> {
    let mut words = Vec::new();
    let domain = zk_ace_pack_bytes_to_field_limbs(domain);
    words.push(domain.length);
    words.extend_from_slice(&domain.limbs);
    words.push(u64::try_from(parts.len()).unwrap_or(u64::MAX));
    for part in parts {
        let packed = zk_ace_pack_bytes_to_field_limbs(part);
        words.push(packed.length);
        words.extend_from_slice(&packed.limbs);
    }
    words
}

fn zk_ace_poseidon_bytes(domain: &[u8], parts: &[&[u8]]) -> [u8; 32] {
    zk_ace_poseidon2_domain_hash(domain, parts)
}

/// Derive a private prover-side AIR statement digest from public inputs and witness.
pub fn derive_zk_ace_air_statement_digest(
    public_inputs: &ZkAcePublicInputsV1,
    witness: &ZkAceWitnessV1,
) -> Result<[u8; 32], norito::Error> {
    let public_bytes = norito::to_bytes(public_inputs)?;
    Ok(zk_ace_poseidon2_domain_hash(
        b"zk-ace.air-statement.v1",
        &[
            &public_bytes,
            &witness.identity_root,
            &witness.identity_blinding,
            &witness.replay_secret,
        ],
    ))
}

/// Derive the verifier-side public AIR word for a ZK-ACE proof.
pub fn derive_zk_ace_air_public_digest(
    public_inputs: &ZkAcePublicInputsV1,
) -> Result<[u8; 32], norito::Error> {
    let mut buf = Vec::new();
    buf.extend_from_slice(b"zk-ace.air-public.v1");
    buf.extend_from_slice(&norito::to_bytes(public_inputs)?);
    Ok(zk_ace_poseidon2_domain_hash(
        b"zk-ace.air-public-digest.v1",
        &[&buf],
    ))
}

/// Derive the ZK-ACE identity commitment from its private witness components.
pub fn derive_zk_ace_identity_commitment(
    identity_root: &[u8; 32],
    identity_blinding: &[u8; 32],
    domain_tag: &str,
) -> [u8; 32] {
    zk_ace_poseidon_bytes(
        b"zk-ace.identity-commitment.v1",
        &[identity_root, identity_blinding, domain_tag.as_bytes()],
    )
}

/// Derive the ZK-ACE replay nullifier for a specific action.
pub fn derive_zk_ace_replay_nullifier(
    replay_secret: &[u8; 32],
    tx_digest: &[u8; 32],
    chain_id: &ChainId,
    action_class: &str,
    domain_tag: &str,
) -> [u8; 32] {
    zk_ace_poseidon_bytes(
        b"zk-ace.replay-nullifier.v1",
        &[
            replay_secret,
            tx_digest,
            chain_id.as_str().as_bytes(),
            action_class.as_bytes(),
            domain_tag.as_bytes(),
        ],
    )
}

/// Derive the action digest for a ZK-ACE-authorized transparent asset transfer.
pub fn derive_zk_ace_transfer_digest(
    from: &AccountId,
    to: &AccountId,
    asset: &AssetDefinitionId,
    amount: u128,
    chain_id: &ChainId,
    action_class: &str,
    policy_hash: &[u8; 32],
) -> [u8; 32] {
    zk_ace_poseidon_bytes(
        b"zk-ace.transparent-transfer.v1",
        &[
            from.to_string().as_bytes(),
            to.to_string().as_bytes(),
            asset.to_string().as_bytes(),
            &amount.to_be_bytes(),
            chain_id.as_str().as_bytes(),
            action_class.as_bytes(),
            policy_hash,
        ],
    )
}

/// Hash canonical public inputs into a STARK public-input word.
pub fn derive_zk_ace_public_inputs_digest(
    public_inputs: &ZkAcePublicInputsV1,
) -> Result<[u8; 32], norito::Error> {
    let bytes = norito::to_bytes(public_inputs)?;
    Ok(zk_ace_poseidon2_domain_hash(
        b"zk-ace.public-inputs.v1",
        &[&bytes],
    ))
}

/// Stable schema hash for ZK-ACE v0 transparent-transfer public inputs.
#[must_use]
pub fn zk_ace_public_inputs_schema_hash_v1() -> [u8; 32] {
    zk_ace_poseidon2_domain_hash(
        b"zk-ace.public-inputs-schema.v1",
        &[
            ZK_ACE_PQ_AUTHORIZATION_V0_CIRCUIT_ID.as_bytes(),
            ZK_ACE_PQ_AUTHORIZATION_V0_ACTION_TRANSFER.as_bytes(),
            ZK_ACE_PQ_AUTHORIZATION_V0_DOMAIN_TAG.as_bytes(),
            b"version:u32",
            b"identity_commitment:bytes32",
            b"tx_digest:bytes32",
            b"chain_id:string",
            b"domain_tag:string",
            b"action_class:string",
            b"replay_nullifier:bytes32",
            b"policy_hash:bytes32",
            b"from:account_id",
            b"to:account_id",
            b"asset:asset_definition_id",
            b"amount:u128",
            b"verifier_key_id:verifying_key_id",
        ],
    )
}

#[cfg(test)]
mod tests {
    use std::str::FromStr as _;

    use iroha_crypto::{Algorithm, KeyPair};

    use super::*;
    use crate::{domain::DomainId, name::Name};

    fn account(seed: u8) -> AccountId {
        let key_pair = KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519);
        AccountId::new(key_pair.public_key().clone())
    }

    fn asset_definition_id() -> AssetDefinitionId {
        AssetDefinitionId::new(
            DomainId::try_new("wonderland", "universal").expect("domain"),
            Name::from_str("xor").expect("asset name"),
        )
    }

    fn valid_open_verify_admission_envelope() -> OpenVerifyEnvelope {
        OpenVerifyEnvelope::new(
            BackendTag::Stark,
            "stark/fri/sha256-goldilocks:zk_ace_pq_authorization_v0",
            [0x55; 32],
            vec![0x01, 0x02],
            vec![0x03, 0x04, 0x05],
        )
    }

    #[cfg(feature = "json")]
    fn assert_json_roundtrip<T>(value: &T)
    where
        T: PartialEq
            + core::fmt::Debug
            + norito::json::JsonSerialize
            + norito::json::JsonDeserialize,
    {
        let json = norito::json::to_json(value).expect("serialize to json");
        let decoded: T = norito::json::from_json(&json).expect("deserialize from json");
        assert_eq!(&decoded, value);
    }

    #[test]
    fn backend_tag_norito_discriminants_preserve_legacy_order_and_pending_range() {
        for (backend, expected_tag) in [
            (BackendTag::Halo2IpaPasta, 0u32),
            (BackendTag::Halo2Bn254, 1),
            (BackendTag::Groth16, 2),
            (BackendTag::Stark, 3),
            (BackendTag::Unsupported, 4),
            (BackendTag::Halo2IpaOrchard, 5),
            (BackendTag::Groth16Bls12377, 6),
            (BackendTag::FcmpPlusPlusCurveTree, 7),
            (BackendTag::LatticePcsSis, 8),
            (BackendTag::MidenStark, 9),
            (BackendTag::AztecPlonkishPrivateKernel, 10),
            (BackendTag::PqMaspStarkFri, 11),
            (BackendTag::AnonymousPgc, 12),
            (BackendTag::VeRange, 13),
            (BackendTag::ZkAt, 14),
            (BackendTag::RecursiveAnonymousAdmission, 15),
            (BackendTag::VegaExistingCredentialZk, 16),
            (BackendTag::SilentThresholdAnoncred, 17),
            (BackendTag::ZkX509, 18),
            (BackendTag::SisWithHints, 19),
        ] {
            let encoded = norito::to_bytes(&backend).expect("encode backend tag");
            assert_eq!(
                encoded.as_slice(),
                expected_tag.to_le_bytes().as_slice(),
                "{} must keep its Norito enum discriminant",
                backend.canonical_label()
            );
            let decoded: BackendTag =
                norito::decode_from_bytes(&encoded).expect("decode backend tag");
            assert_eq!(decoded, backend);
        }
    }

    #[test]
    fn backend_tag_catalog_label_parser_preserves_pending_protocol_families() {
        for (label, expected) in [
            ("halo2-ipa-orchard", BackendTag::Halo2IpaOrchard),
            ("halo2/ipa/orchard", BackendTag::Halo2IpaOrchard),
            ("orchard", BackendTag::Halo2IpaOrchard),
            ("anonymous-pgc", BackendTag::AnonymousPgc),
            ("anonymous-pgc-k-out-of-n", BackendTag::AnonymousPgc),
            ("verange-transparent-range", BackendTag::VeRange),
            ("zkAt policy-private authenticator", BackendTag::ZkAt),
            (
                "recursive-anonymous-admission",
                BackendTag::RecursiveAnonymousAdmission,
            ),
            (
                "zk-ams-recursive-admission-v0",
                BackendTag::RecursiveAnonymousAdmission,
            ),
            (
                "vega-existing-credential-zk",
                BackendTag::VegaExistingCredentialZk,
            ),
            (
                "threshold-anonymous-credentials",
                BackendTag::SilentThresholdAnoncred,
            ),
            (
                "silent-threshold-anoncred",
                BackendTag::SilentThresholdAnoncred,
            ),
            ("zkvm-x509-identity", BackendTag::ZkX509),
            ("zk-x509-onchain-identity-v0", BackendTag::ZkX509),
            ("sis-with-hints", BackendTag::SisWithHints),
            ("lattice-anonymous-credentials", BackendTag::SisWithHints),
            ("groth16-bls12-377", BackendTag::Groth16Bls12377),
            ("groth16/bls12-377", BackendTag::Groth16Bls12377),
            ("penumbra-masp", BackendTag::Groth16Bls12377),
            ("halo2/ipa/penumbra", BackendTag::Groth16Bls12377),
            ("halo2/ipa/masp", BackendTag::Groth16Bls12377),
            ("monero-fcmp++", BackendTag::FcmpPlusPlusCurveTree),
            (
                "fcmp-plus-plus-curve-tree",
                BackendTag::FcmpPlusPlusCurveTree,
            ),
            ("halo2/ipa/monero", BackendTag::FcmpPlusPlusCurveTree),
            ("halo2/ipa/curve-tree", BackendTag::FcmpPlusPlusCurveTree),
            ("lattice-pcs-sis", BackendTag::LatticePcsSis),
            ("jindo-lattice-pcs-zk", BackendTag::LatticePcsSis),
            ("miden-stark", BackendTag::MidenStark),
            (
                "aztec-plonkish-private-kernel",
                BackendTag::AztecPlonkishPrivateKernel,
            ),
            ("pq-masp-stark-fri", BackendTag::PqMaspStarkFri),
            ("post-quantum-masp", BackendTag::PqMaspStarkFri),
        ] {
            assert_eq!(
                BackendTag::from_catalog_label(label),
                expected,
                "{label} must parse to its exact pending backend tag",
            );
            assert!(
                BackendTag::is_pending_production_backend_label(label),
                "{label} must remain marked as pending production",
            );
        }
    }

    #[test]
    fn backend_tag_catalog_label_parser_preserves_supported_legacy_families() {
        for (label, expected) in [
            ("halo2-ipa-pasta", BackendTag::Halo2IpaPasta),
            ("halo2/ipa", BackendTag::Halo2IpaPasta),
            ("halo2/pasta/ipa/vote-bool", BackendTag::Halo2IpaPasta),
            ("halo2-bn254", BackendTag::Halo2Bn254),
            ("halo2/bn254", BackendTag::Halo2Bn254),
            ("groth16", BackendTag::Groth16),
            ("groth16/bn254", BackendTag::Groth16),
            ("stark", BackendTag::Stark),
            ("stark/fri/sha256-goldilocks", BackendTag::Stark),
        ] {
            assert_eq!(
                BackendTag::from_catalog_label(label),
                expected,
                "{label} must keep legacy backend-family mapping",
            );
        }
        assert_eq!(
            BackendTag::from_catalog_label("unknown/privacy/backend"),
            BackendTag::Unsupported
        );
    }

    #[test]
    fn open_verify_envelope_admission_validation_accepts_canonical_shape() {
        valid_open_verify_admission_envelope()
            .validate_for_admission()
            .expect("valid envelope");
    }

    #[test]
    fn open_verify_envelope_admission_validation_rejects_adversarial_shapes() {
        use OpenVerifyEnvelopeValidationError::{
            EmptyCircuitId, EmptyProofBytes, EmptyPublicInputs, NonEmptyAux, ProofBytesTooLarge,
            PublicInputsTooLarge, UnsupportedBackend, ZeroVerifierKeyHash,
        };

        let cases: [(
            &str,
            fn(&mut OpenVerifyEnvelope),
            OpenVerifyEnvelopeValidationError,
        ); 8] = [
            (
                "unsupported backend",
                |env| env.backend = BackendTag::Unsupported,
                UnsupportedBackend,
            ),
            (
                "empty circuit id",
                |env| env.circuit_id = " \t\n".to_owned(),
                EmptyCircuitId,
            ),
            (
                "zero verifier-key hash",
                |env| env.vk_hash = [0; 32],
                ZeroVerifierKeyHash,
            ),
            (
                "empty public inputs",
                |env| env.public_inputs.clear(),
                EmptyPublicInputs,
            ),
            (
                "oversized public inputs",
                |env| env.public_inputs = vec![0xAA; 4],
                PublicInputsTooLarge { len: 4, max: 3 },
            ),
            (
                "empty proof bytes",
                |env| env.proof_bytes.clear(),
                EmptyProofBytes,
            ),
            (
                "oversized proof bytes",
                |env| env.proof_bytes = vec![0xBB; 5],
                ProofBytesTooLarge { len: 5, max: 4 },
            ),
            (
                "non-empty aux",
                |env| env.aux = b"side-channel".to_vec(),
                NonEmptyAux,
            ),
        ];

        for (name, mutate, expected) in cases {
            let mut envelope = valid_open_verify_admission_envelope();
            mutate(&mut envelope);
            let err = envelope
                .validate_with_bounds(OpenVerifyEnvelopeBounds {
                    max_public_input_bytes: 3,
                    max_proof_bytes: 4,
                    ..OpenVerifyEnvelopeBounds::default()
                })
                .unwrap_err();
            assert_eq!(err, expected, "{name}");
        }
    }

    #[test]
    fn open_verify_envelope_admission_rejects_pending_production_backends() {
        use OpenVerifyEnvelopeValidationError::PendingProductionBackend;

        for backend in [
            BackendTag::Halo2IpaOrchard,
            BackendTag::Groth16Bls12377,
            BackendTag::FcmpPlusPlusCurveTree,
            BackendTag::LatticePcsSis,
            BackendTag::MidenStark,
            BackendTag::AztecPlonkishPrivateKernel,
            BackendTag::PqMaspStarkFri,
            BackendTag::AnonymousPgc,
            BackendTag::VeRange,
            BackendTag::ZkAt,
            BackendTag::RecursiveAnonymousAdmission,
            BackendTag::VegaExistingCredentialZk,
            BackendTag::SilentThresholdAnoncred,
            BackendTag::ZkX509,
            BackendTag::SisWithHints,
        ] {
            let mut envelope = valid_open_verify_admission_envelope();
            envelope.backend = backend;

            assert_eq!(
                envelope.validate_for_admission().unwrap_err(),
                PendingProductionBackend { backend },
                "{} must remain fail-closed for chain admission",
                backend.canonical_label(),
            );

            envelope
                .validate_with_bounds(OpenVerifyEnvelopeBounds {
                    allow_pending_production_backends: true,
                    ..OpenVerifyEnvelopeBounds::default()
                })
                .expect("explicit non-admission bounds can inspect pending backend envelopes");
        }
    }

    #[cfg(feature = "json")]
    #[test]
    fn backend_tag_json_roundtrips_exact_pending_production_families() {
        for backend in [
            BackendTag::Halo2IpaOrchard,
            BackendTag::Groth16Bls12377,
            BackendTag::FcmpPlusPlusCurveTree,
            BackendTag::LatticePcsSis,
            BackendTag::MidenStark,
            BackendTag::AztecPlonkishPrivateKernel,
            BackendTag::PqMaspStarkFri,
            BackendTag::AnonymousPgc,
            BackendTag::VeRange,
            BackendTag::ZkAt,
            BackendTag::RecursiveAnonymousAdmission,
            BackendTag::VegaExistingCredentialZk,
            BackendTag::SilentThresholdAnoncred,
            BackendTag::ZkX509,
            BackendTag::SisWithHints,
        ] {
            assert_json_roundtrip(&backend);
        }
    }

    #[test]
    fn open_verify_envelope_validation_allows_explicit_aux_and_zero_key_only_when_configured() {
        let mut envelope = valid_open_verify_admission_envelope();
        envelope.vk_hash = [0; 32];
        envelope.aux = b"bounded-aux".to_vec();

        envelope
            .validate_with_bounds(OpenVerifyEnvelopeBounds {
                allow_aux: true,
                require_nonzero_vk_hash: false,
                max_aux_bytes: envelope.aux.len(),
                ..OpenVerifyEnvelopeBounds::default()
            })
            .expect("custom non-admission bounds can allow zero key and aux");

        let err = envelope
            .validate_with_bounds(OpenVerifyEnvelopeBounds {
                allow_aux: true,
                require_nonzero_vk_hash: false,
                max_aux_bytes: envelope.aux.len() - 1,
                ..OpenVerifyEnvelopeBounds::default()
            })
            .unwrap_err();
        assert_eq!(
            err,
            OpenVerifyEnvelopeValidationError::AuxTooLarge {
                len: envelope.aux.len(),
                max: envelope.aux.len() - 1,
            }
        );
    }

    #[test]
    fn zk_ace_packing_and_hash_vectors_are_stable() {
        let packed = zk_ace_pack_bytes_to_field_limbs(b"ABCDEFGH");
        assert_eq!(packed.length, 8);
        assert_eq!(packed.limbs, vec![0x0047_4645_4443_4241, 0x48]);

        let witness = ZkAceWitnessV1 {
            identity_root: [0x11; 32],
            identity_blinding: [0x22; 32],
            replay_secret: [0x33; 32],
        };
        let policy_hash = [0x44; 32];
        let chain_id: ChainId = "boi-test-chain".parse().expect("chain id");
        let from = account(1);
        let to = account(2);
        let asset = asset_definition_id();
        let verifier_key_id = VerifyingKeyId::new(
            ZK_ACE_PQ_AUTHORIZATION_V0_BACKEND,
            ZK_ACE_PQ_AUTHORIZATION_V0_CIRCUIT_ID,
        );
        let identity_commitment = derive_zk_ace_identity_commitment(
            &witness.identity_root,
            &witness.identity_blinding,
            ZK_ACE_PQ_AUTHORIZATION_V0_DOMAIN_TAG,
        );
        let tx_digest = derive_zk_ace_transfer_digest(
            &from,
            &to,
            &asset,
            17,
            &chain_id,
            ZK_ACE_PQ_AUTHORIZATION_V0_ACTION_TRANSFER,
            &policy_hash,
        );
        let replay_nullifier = derive_zk_ace_replay_nullifier(
            &witness.replay_secret,
            &tx_digest,
            &chain_id,
            ZK_ACE_PQ_AUTHORIZATION_V0_ACTION_TRANSFER,
            ZK_ACE_PQ_AUTHORIZATION_V0_DOMAIN_TAG,
        );
        let public_inputs = ZkAcePublicInputsV1::transparent_transfer(
            identity_commitment,
            tx_digest,
            chain_id,
            replay_nullifier,
            policy_hash,
            from,
            to,
            asset,
            17,
            verifier_key_id,
        );
        let public_digest =
            derive_zk_ace_public_inputs_digest(&public_inputs).expect("public digest");
        let air_public_digest =
            derive_zk_ace_air_public_digest(&public_inputs).expect("air public digest");
        let air_statement_digest = derive_zk_ace_air_statement_digest(&public_inputs, &witness)
            .expect("air statement digest");
        let schema_hash = zk_ace_public_inputs_schema_hash_v1();

        assert_eq!(
            hex::encode(identity_commitment),
            "9cb1c494eaf171b6ce218d3c7c6de88cdc8228f9b4eda310a325b4b2c1cbd68f"
        );
        assert_eq!(
            hex::encode(tx_digest),
            "f5e3f7120d12b98f65f088b419db9607d40eedfd412f767062cc4f1e18527036"
        );
        assert_eq!(
            hex::encode(replay_nullifier),
            "1ddaf81b2865d10fdc5b597f0283c675a76928bfd171eadb6410aacb971cefc1"
        );
        assert_eq!(
            hex::encode(public_digest),
            "2873792251b35ebcb9b9357b46bb38d0022dd7e6fb8091f2d5d85677bab52389"
        );
        assert_eq!(
            hex::encode(air_public_digest),
            "248c2c007fcfd20ab285bdad0490ed7b7b046001614b4d2aa4b6021d6c952bc1"
        );
        assert_eq!(
            hex::encode(air_statement_digest),
            "7c1cfdf8ec0e2a4c1a10eeca670558293c8468dd8ade96bd86bf1f95e2dc34f4"
        );
        assert_eq!(
            hex::encode(schema_hash),
            "2f265a860aa24df7d6703513fb95cb9b6323eae70203cbb32b53bd6e4fd1325c"
        );
    }

    #[cfg(feature = "json")]
    #[test]
    fn zk_ace_json_roundtrips_public_proof_witness_and_packing() {
        let witness = ZkAceWitnessV1 {
            identity_root: [0x11; 32],
            identity_blinding: [0x22; 32],
            replay_secret: [0x33; 32],
        };
        let policy_hash = [0x44; 32];
        let chain_id: ChainId = "boi-test-chain".parse().expect("chain id");
        let from = account(1);
        let to = account(2);
        let asset = asset_definition_id();
        let verifier_key_id = VerifyingKeyId::new(
            ZK_ACE_PQ_AUTHORIZATION_V0_BACKEND,
            ZK_ACE_PQ_AUTHORIZATION_V0_CIRCUIT_ID,
        );
        let identity_commitment = derive_zk_ace_identity_commitment(
            &witness.identity_root,
            &witness.identity_blinding,
            ZK_ACE_PQ_AUTHORIZATION_V0_DOMAIN_TAG,
        );
        let tx_digest = derive_zk_ace_transfer_digest(
            &from,
            &to,
            &asset,
            17,
            &chain_id,
            ZK_ACE_PQ_AUTHORIZATION_V0_ACTION_TRANSFER,
            &policy_hash,
        );
        let replay_nullifier = derive_zk_ace_replay_nullifier(
            &witness.replay_secret,
            &tx_digest,
            &chain_id,
            ZK_ACE_PQ_AUTHORIZATION_V0_ACTION_TRANSFER,
            ZK_ACE_PQ_AUTHORIZATION_V0_DOMAIN_TAG,
        );
        let public_inputs = ZkAcePublicInputsV1::transparent_transfer(
            identity_commitment,
            tx_digest,
            chain_id,
            replay_nullifier,
            policy_hash,
            from,
            to,
            asset,
            17,
            verifier_key_id,
        );
        let packed = zk_ace_pack_bytes_to_field_limbs(b"ABCDEFGH");
        let open_proof = StarkFriOpenProofV1 {
            version: 1,
            public_inputs: vec![vec![[0xAA; 32], [0xBB; 32]], vec![[0xCC; 32]]],
            envelope_bytes: vec![0x01, 0x02, 0x03, 0x04],
        };

        assert_json_roundtrip(&public_inputs);
        assert_json_roundtrip(&witness);
        assert_json_roundtrip(&packed);
        assert_json_roundtrip(&open_proof);
    }
}
