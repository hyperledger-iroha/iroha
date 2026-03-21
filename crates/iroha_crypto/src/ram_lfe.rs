//! Reusable hidden-function commitment and evaluation plumbing.
//!
//! Naming in this module is split deliberately:
//! - `ram_lfe` covers the outer hidden-function abstraction: commitments,
//!   public policy metadata, evaluation requests, outputs, and receipt-facing
//!   verification metadata.
//! - `BFV` names the Brakerski/Fan-Vercauteren homomorphic encryption backend
//!   used by some evaluators to process encrypted input.
//! - `ram_fhe`-prefixed profile/program names remain on the BFV side because
//!   they describe the encrypted execution machine, not the outer LFE layer.
//!
//! The current evaluators are:
//! - the historical committed `HKDF-SHA3-512` PRF backend,
//! - a BFV-backed secret affine evaluator that consumes BFV-encrypted input,
//! - and a BFV-backed secret programmed evaluator with an instruction-driven
//!   RAM-style encrypted state machine.

use std::{string::String, vec::Vec};

use hkdf::Hkdf;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
#[cfg(feature = "json")]
use norito::derive::{JsonDeserialize, JsonSerialize};
#[cfg(feature = "json")]
use norito::json;
use sha3::Sha3_512;
use thiserror::Error;

use rand::{Rng as _, SeedableRng as _};
use rand_chacha::ChaCha20Rng;

use crate::{
    BfvAffineCircuit, BfvCiphertext, BfvError, BfvIdentifierCiphertext,
    BfvIdentifierPublicParameters, BfvParameters, BfvRelinearizationKey, Hash, add_ciphertexts,
    add_plain_scalar, decrypt, derive_identifier_key_material_from_seed, evaluate_affine_circuit,
    multiply_ciphertexts, multiply_plain_scalar,
};

const POLICY_DOMAIN: &[u8] = b"iroha.ram_lfe.policy.hkdf_sha3_512_prf.v1";
const SECRET_COMMITMENT_DOMAIN: &[u8] = b"iroha.ram_lfe.policy_secret.hkdf_sha3_512_prf.v1";
const HKDF_SALT_DOMAIN: &[u8] = b"iroha.ram_lfe.hkdf_salt.hkdf_sha3_512_prf.v1";
const HKDF_OPAQUE_INFO_DOMAIN: &[u8] = b"iroha.ram_lfe.opaque_info.hkdf_sha3_512_prf.v1";
const HKDF_RECEIPT_INFO_DOMAIN: &[u8] = b"iroha.ram_lfe.receipt_info.hkdf_sha3_512_prf.v1";
const OPAQUE_HASH_DOMAIN: &[u8] = b"iroha.ram_lfe.opaque_hash.hkdf_sha3_512_prf.v1";
const RECEIPT_HASH_DOMAIN: &[u8] = b"iroha.ram_lfe.receipt_hash.hkdf_sha3_512_prf.v1";
const BFV_AFFINE_CIRCUIT_DOMAIN: &[u8] = b"iroha.ram_lfe.bfv_affine.circuit.v1";
const BFV_AFFINE_OPAQUE_HASH_DOMAIN: &[u8] = b"iroha.ram_lfe.bfv_affine.opaque_hash.v1";
const BFV_AFFINE_RECEIPT_HASH_DOMAIN: &[u8] = b"iroha.ram_lfe.bfv_affine.receipt_hash.v1";
const BFV_PROGRAM_MEMORY_DOMAIN: &[u8] = b"iroha.ram_lfe.bfv_program.memory.v1";
const BFV_PROGRAM_OPAQUE_HASH_DOMAIN: &[u8] = b"iroha.ram_lfe.bfv_program.opaque_hash.v1";
const BFV_PROGRAM_RECEIPT_HASH_DOMAIN: &[u8] = b"iroha.ram_lfe.bfv_program.receipt_hash.v1";
const BFV_PROGRAM_DIGEST_DOMAIN: &[u8] = b"iroha.ram_lfe.bfv_program.digest.v1";
const RAM_FHE_OUTPUT_HASH_DOMAIN: &[u8] = b"iroha.ram_lfe.output_hash.v1";
const IDENTIFIER_OUTPUT_OPAQUE_HASH_DOMAIN: &[u8] = b"iroha.ram_lfe.identifier.opaque_hash.v1";
const IDENTIFIER_OUTPUT_RECEIPT_HASH_DOMAIN: &[u8] = b"iroha.ram_lfe.identifier.receipt_hash.v1";
const BFV_AFFINE_OUTPUT_BYTES: usize = Hash::LENGTH;
const BFV_PROGRAM_STATE_WIDTH: usize = Hash::LENGTH;
const BFV_PROGRAM_REGISTER_COUNT: usize = 4;
const BFV_PROGRAM_MIN_CIPHERTEXT_MODULUS: u64 = 1_u64 << 52;
const BFV_PROGRAM_REGISTER_COUNT_U16: u16 = 4;
const BFV_PROGRAM_STATE_WIDTH_U16: u16 = 32;
const MAX_INPUT_BYTES: usize = 1_048_576;
const MAX_SECRET_BYTES: usize = 4096;

struct ProgramExecutionContext<'a> {
    params: &'a BfvParameters,
    relinearization_key: &'a BfvRelinearizationKey,
}

/// Canonicalization rule applied before the programmed RAM-FHE backend executes.
#[cfg_attr(feature = "json", derive(JsonSerialize, JsonDeserialize))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[norito(tag = "mode", content = "value", rename_all = "snake_case")]
pub enum BfvRamEncryptedInputMode {
    /// Resolver decrypts the submitted ciphertext and re-encrypts it onto the
    /// deterministic policy envelope before executing the hidden program.
    ResolverCanonicalizedEnvelopeV1,
}

/// Public RAM-FHE execution profile for the programmed BFV backend.
#[cfg_attr(feature = "json", derive(JsonSerialize, JsonDeserialize))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
pub struct BfvRamProgramProfile {
    /// Stable profile version understood by the current evaluator.
    pub profile_version: u8,
    /// Number of ciphertext registers in the hidden execution machine.
    pub register_count: u16,
    /// Number of ciphertext memory lanes persisted across program steps.
    pub memory_lane_count: u16,
    /// Maximum ciphertext-ciphertext multiplications performed per step.
    pub ciphertext_mul_per_step: u8,
    /// Canonicalization mode for externally supplied encrypted input.
    pub encrypted_input_mode: BfvRamEncryptedInputMode,
    /// Minimum supported ciphertext modulus for this RAM-FHE profile.
    pub min_ciphertext_modulus: u64,
}

/// Receipt attestation mode published by a RAM-LFE program policy.
#[cfg_attr(feature = "json", derive(JsonSerialize, JsonDeserialize))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[norito(tag = "mode", content = "value", rename_all = "snake_case")]
pub enum RamLfeVerificationMode {
    /// Canonical payload bytes are signed by the configured resolver key.
    Signed,
    /// Canonical payload bytes are bound to a Halo2 proof envelope.
    Proof,
}

/// Public proof-verifier metadata published by proof-carrying RAM-LFE policies.
#[cfg_attr(feature = "json", derive(JsonSerialize, JsonDeserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
pub struct RamLfeProofVerifierMetadata {
    /// Proof backend identifier understood by higher-layer verifiers.
    pub proof_backend: String,
    /// Stable circuit identifier bound to proof payloads.
    pub circuit_id: String,
    /// Stable hash of the proof public-input schema.
    pub public_inputs_schema_hash: Hash,
    /// Hash of the verifying-key bytes under `proof_backend`.
    pub verifying_key_hash: Hash,
    /// Opaque verifying-key bytes published to clients for stateless verification.
    pub verifying_key_bytes: Vec<u8>,
}

/// Canonical branchless instruction for the hidden programmed RAM-FHE backend.
#[cfg_attr(feature = "json", derive(JsonSerialize, JsonDeserialize))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Encode, Decode)]
#[norito(tag = "op", content = "args", rename_all = "snake_case")]
pub enum HiddenRamFheInstruction {
    /// Load one encrypted input byte slot into a register.
    LoadInput(u16, u16),
    /// Load a persisted encrypted state lane into a register.
    LoadState(u16, u16),
    /// Store a register back into a persisted encrypted state lane.
    StoreState(u16, u16),
    /// Load a plaintext constant into a register.
    LoadConst(u16, u64),
    /// Add two ciphertext registers.
    Add(u16, u16, u16),
    /// Add a plaintext scalar to a ciphertext register.
    AddPlain(u16, u16, u64),
    /// Subtract a plaintext scalar from a ciphertext register.
    SubPlain(u16, u16, u64),
    /// Multiply a ciphertext register by a plaintext scalar.
    MulPlain(u16, u16, u64),
    /// Multiply two ciphertext registers.
    Mul(u16, u16, u16),
    /// Select between two registers based on whether `condition == 0`.
    SelectEqZero(u16, u16, u16, u16),
    /// Append one register to the plaintext output blob.
    Output(u16),
}

/// Canonical hidden program executed by the programmed BFV backend.
#[cfg_attr(feature = "json", derive(JsonSerialize, JsonDeserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode)]
pub struct HiddenRamFheProgram {
    /// Stable program format version.
    pub version: u8,
    /// Number of registers the program expects.
    pub register_count: u16,
    /// Number of persisted memory lanes the program expects.
    pub memory_lane_count: u16,
    /// Fixed-step branchless instruction tape.
    pub instructions: Vec<HiddenRamFheInstruction>,
}

impl HiddenRamFheProgram {
    /// Encode the program into canonical Norito bytes.
    ///
    /// # Errors
    /// Returns the underlying Norito encoding error when serialization fails.
    pub fn to_bytes(&self) -> Result<Vec<u8>, norito::core::Error> {
        norito::to_bytes(self)
    }

    /// Return the stable digest published by programmed policies.
    ///
    /// # Errors
    /// Returns the underlying Norito encoding error when serialization fails.
    pub fn digest(&self) -> Result<Hash, norito::core::Error> {
        self.to_bytes()
            .map(|bytes| Hash::new([BFV_PROGRAM_DIGEST_DOMAIN, bytes.as_slice()].concat()))
    }
}

/// Public parameter bundle published by programmed BFV policies.
#[cfg_attr(feature = "json", derive(JsonSerialize, JsonDeserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
pub struct BfvProgrammedPublicParameters {
    /// BFV envelope parameters used to encrypt identifier bytes.
    pub encryption: BfvIdentifierPublicParameters,
    /// Stable digest of the hidden compiled program kept in runtime config.
    pub hidden_program_digest: Hash,
    /// Public RAM-FHE execution profile consumed by clients and verifiers.
    pub ram_fhe_profile: BfvRamProgramProfile,
    /// Receipt verification mode enforced for this program policy.
    pub verification_mode: RamLfeVerificationMode,
    /// Optional verifier metadata published for proof-carrying receipts.
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub proof_verifier: Option<RamLfeProofVerifierMetadata>,
}

/// Supported RAM-LFE backends.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
pub enum RamLfeBackend {
    /// HKDF-SHA3-512 commitment-bound PRF evaluator.
    HkdfSha3_512PrfV1,
    /// BFV-backed secret affine evaluator producing a 32-byte opaque seed.
    BfvAffineSha3_256V1,
    /// BFV-backed stateful secret program with non-linear per-slot transforms.
    BfvProgrammedSha3_256V1,
}

impl RamLfeBackend {
    /// Stable backend identifier.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::HkdfSha3_512PrfV1 => "hkdf-sha3-512-prf-v1",
            Self::BfvAffineSha3_256V1 => "bfv-affine-sha3-256-v1",
            Self::BfvProgrammedSha3_256V1 => "bfv-programmed-sha3-256-v1",
        }
    }
}

#[cfg(feature = "json")]
impl json::JsonSerialize for RamLfeBackend {
    fn json_serialize(&self, out: &mut String) {
        json::JsonSerialize::json_serialize(self.as_str(), out);
    }
}

#[cfg(feature = "json")]
impl json::JsonDeserialize for RamLfeBackend {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        let value: String = json::JsonDeserialize::json_deserialize(parser)?;
        match value.as_str() {
            "hkdf-sha3-512-prf-v1" => Ok(Self::HkdfSha3_512PrfV1),
            "bfv-affine-sha3-256-v1" => Ok(Self::BfvAffineSha3_256V1),
            "bfv-programmed-sha3-256-v1" => Ok(Self::BfvProgrammedSha3_256V1),
            _ => Err(json::Error::Message(format!(
                "unsupported RAM-LFE backend `{value}`"
            ))),
        }
    }
}

/// Public commitment to a hidden identifier-derivation policy.
#[cfg_attr(feature = "json", derive(JsonSerialize, JsonDeserialize))]
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
pub struct PolicyCommitment {
    /// Backend used to evaluate the hidden policy.
    pub backend: RamLfeBackend,
    /// Commitment digest tying the hidden evaluator secret to public metadata.
    pub policy_hash: Hash,
    /// Public policy metadata consumed by wallets and verifier code.
    #[norito(default)]
    pub public_parameters: Vec<u8>,
}

/// Client request submitted to a RAM-LFE evaluator.
#[cfg_attr(feature = "json", derive(JsonSerialize, JsonDeserialize))]
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
pub struct ClientRequest {
    /// Backend-specific request payload.
    pub normalized_input: Vec<u8>,
    /// Public associated data bound into the derivation.
    #[norito(default)]
    pub associated_data: Vec<u8>,
}

/// Deterministic RAM-LFE evaluation output.
#[cfg_attr(feature = "json", derive(JsonSerialize, JsonDeserialize))]
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
pub struct EvalResponse {
    /// Plaintext output bytes produced by the hidden engine.
    pub output: Vec<u8>,
    /// Opaque identifier derived by the hidden policy.
    pub opaque_id: Hash,
    /// Receipt digest that higher layers can sign or attest to.
    pub receipt_hash: Hash,
    /// Backend that produced the output.
    pub backend: RamLfeBackend,
}

/// Errors raised by the RAM-LFE plumbing layer.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum RamLfeError {
    /// The supplied input must not be empty.
    #[error("normalized input must not be empty")]
    EmptyInput,
    /// The supplied input exceeded the backend limit.
    #[error("normalized input exceeds the maximum supported length")]
    InputTooLarge,
    /// The hidden policy secret must not be empty.
    #[error("policy secret must not be empty")]
    EmptySecret,
    /// The hidden policy secret exceeded the backend limit.
    #[error("policy secret exceeds the maximum supported length")]
    SecretTooLarge,
    /// The hidden secret does not match the published commitment.
    #[error("policy secret does not match the published commitment")]
    CommitmentMismatch,
    /// The hidden compiled program does not match the published program digest.
    #[error("hidden program digest does not match the published commitment")]
    HiddenProgramMismatch,
    /// Norito failed to encode an internal transcript.
    #[error("policy transcript encoding failed: {0}")]
    TranscriptEncoding(String),
    /// HKDF failed to expand the requested output material.
    #[error("HKDF expansion failed")]
    DerivationFailed,
    /// BFV evaluation failed.
    #[error("BFV evaluation failed: {0}")]
    Bfv(String),
    /// The selected backend is not supported by the evaluator.
    #[error("unsupported RAM-LFE backend `{0}`")]
    UnsupportedBackend(String),
}

/// Runtime evaluator interface for hidden-function services.
pub trait Evaluator: Send + Sync {
    /// Evaluate a request against the supplied policy commitment.
    ///
    /// # Errors
    /// Returns [`RamLfeError`] when the request, commitment, or backend
    /// evaluation fails validation.
    fn evaluate(
        &self,
        commitment: &PolicyCommitment,
        request: &ClientRequest,
    ) -> Result<EvalResponse, RamLfeError>;
}

/// Construct the commitment record for the built-in HKDF-SHA3-512 backend.
///
/// # Errors
/// Returns [`RamLfeError`] when the secret or public transcript is invalid.
pub fn policy_commitment(
    secret: &[u8],
    public_parameters: Vec<u8>,
) -> Result<PolicyCommitment, RamLfeError> {
    build_policy_commitment(secret, public_parameters, RamLfeBackend::HkdfSha3_512PrfV1)
}

/// Return the default public execution profile for the programmed BFV backend.
#[must_use]
pub const fn bfv_program_profile() -> BfvRamProgramProfile {
    BfvRamProgramProfile {
        profile_version: 1,
        register_count: BFV_PROGRAM_REGISTER_COUNT_U16,
        memory_lane_count: BFV_PROGRAM_STATE_WIDTH_U16,
        ciphertext_mul_per_step: 1,
        encrypted_input_mode: BfvRamEncryptedInputMode::ResolverCanonicalizedEnvelopeV1,
        min_ciphertext_modulus: BFV_PROGRAM_MIN_CIPHERTEXT_MODULUS,
    }
}

/// Return the canonical hidden program used by the historical identifier-programmed backend.
#[must_use]
pub fn default_bfv_programmed_hidden_program() -> HiddenRamFheProgram {
    HiddenRamFheProgram {
        version: 1,
        register_count: BFV_PROGRAM_REGISTER_COUNT_U16,
        memory_lane_count: BFV_PROGRAM_STATE_WIDTH_U16,
        instructions: vec![
            HiddenRamFheInstruction::LoadInput(0, 0),
            HiddenRamFheInstruction::LoadState(1, 0),
            HiddenRamFheInstruction::Mul(2, 0, 1),
            HiddenRamFheInstruction::AddPlain(2, 2, 17),
            HiddenRamFheInstruction::StoreState(0, 2),
            HiddenRamFheInstruction::LoadState(3, 1),
            HiddenRamFheInstruction::Add(3, 3, 2),
            HiddenRamFheInstruction::StoreState(1, 3),
            HiddenRamFheInstruction::Output(2),
            HiddenRamFheInstruction::Output(3),
        ],
    }
}

/// Hash arbitrary RAM-LFE output bytes into a stable digest.
#[must_use]
pub fn ram_lfe_output_hash(output: &[u8]) -> Hash {
    Hash::new([RAM_FHE_OUTPUT_HASH_DOMAIN, output].concat())
}

/// Derive the identifier-facing opaque id and receipt hash from engine output bytes.
#[must_use]
pub fn identifier_hashes_from_program_output(program_id: &[u8], output: &[u8]) -> (Hash, Hash) {
    identifier_hashes_from_output_hash(program_id, &ram_lfe_output_hash(output))
}

/// Derive the identifier-facing opaque id and receipt hash from a precomputed output hash.
#[must_use]
pub fn identifier_hashes_from_output_hash(program_id: &[u8], output_hash: &Hash) -> (Hash, Hash) {
    let opaque_id = Hash::new(
        [
            IDENTIFIER_OUTPUT_OPAQUE_HASH_DOMAIN,
            program_id,
            output_hash.as_ref(),
        ]
        .concat(),
    );
    let receipt_hash = Hash::new(
        [
            IDENTIFIER_OUTPUT_RECEIPT_HASH_DOMAIN,
            program_id,
            output_hash.as_ref(),
            opaque_id.as_ref(),
        ]
        .concat(),
    );
    (opaque_id, receipt_hash)
}

/// Wrap BFV identifier-encryption parameters into the programmed RAM-FHE public bundle.
#[must_use]
pub fn bfv_programmed_public_parameters(
    encryption: BfvIdentifierPublicParameters,
) -> BfvProgrammedPublicParameters {
    bfv_programmed_public_parameters_with_program(
        encryption,
        &default_bfv_programmed_hidden_program(),
        RamLfeVerificationMode::Signed,
        None,
    )
}

/// Wrap BFV identifier-encryption parameters and explicit hidden-program metadata.
#[must_use]
pub fn bfv_programmed_public_parameters_with_program(
    encryption: BfvIdentifierPublicParameters,
    program: &HiddenRamFheProgram,
    verification_mode: RamLfeVerificationMode,
    proof_verifier: Option<RamLfeProofVerifierMetadata>,
) -> BfvProgrammedPublicParameters {
    BfvProgrammedPublicParameters {
        encryption,
        hidden_program_digest: program
            .digest()
            .expect("canonical hidden program digest must encode"),
        ram_fhe_profile: bfv_program_profile(),
        verification_mode,
        proof_verifier,
    }
}

/// Decode programmed BFV public parameters, upgrading legacy raw BFV payloads.
///
/// # Errors
/// Returns [`RamLfeError`] when the public parameter payload is malformed.
pub fn decode_bfv_programmed_public_parameters(
    public_parameters: &[u8],
) -> Result<BfvProgrammedPublicParameters, RamLfeError> {
    if let Ok(archived) = norito::from_bytes::<BfvProgrammedPublicParameters>(public_parameters) {
        let value: BfvProgrammedPublicParameters =
            norito::core::NoritoDeserialize::deserialize(archived);
        value
            .encryption
            .validate()
            .map_err(|err| map_bfv_error(&err))?;
        validate_programmed_profile(&value.ram_fhe_profile)?;
        validate_programmed_public_parameters(&value.encryption)?;
        validate_proof_verifier_metadata(value.verification_mode, value.proof_verifier.as_ref())?;
        return Ok(value);
    }

    let encryption = decode_bfv_public_parameters(public_parameters)?;
    Ok(bfv_programmed_public_parameters(encryption))
}

/// Construct the commitment record for the BFV secret affine backend.
///
/// # Errors
/// Returns [`RamLfeError`] when the secret or public transcript is invalid.
pub fn bfv_affine_policy_commitment(
    secret: &[u8],
    public_parameters: Vec<u8>,
) -> Result<PolicyCommitment, RamLfeError> {
    build_policy_commitment(
        secret,
        public_parameters,
        RamLfeBackend::BfvAffineSha3_256V1,
    )
}

/// Construct the commitment record for the BFV programmed backend.
///
/// # Errors
/// Returns [`RamLfeError`] when the secret or public transcript is invalid.
pub fn bfv_programmed_policy_commitment(
    secret: &[u8],
    public_parameters: &[u8],
) -> Result<PolicyCommitment, RamLfeError> {
    bfv_programmed_policy_commitment_with_program(
        secret,
        public_parameters,
        &default_bfv_programmed_hidden_program(),
    )
}

/// Construct the commitment record for an explicit hidden BFV program.
///
/// # Errors
/// Returns [`RamLfeError`] when the secret, public transcript, or program is invalid.
pub fn bfv_programmed_policy_commitment_with_program(
    secret: &[u8],
    public_parameters: &[u8],
    program: &HiddenRamFheProgram,
) -> Result<PolicyCommitment, RamLfeError> {
    let expected_digest = program
        .digest()
        .map_err(|err| RamLfeError::TranscriptEncoding(err.to_string()))?;
    let decoded = decode_bfv_programmed_public_parameters(public_parameters)?;
    if decoded.hidden_program_digest != expected_digest {
        return Err(RamLfeError::CommitmentMismatch);
    }
    let canonical_public_parameters = norito::to_bytes(&decoded)
        .map_err(|err| RamLfeError::TranscriptEncoding(err.to_string()))?;
    build_policy_commitment(
        secret,
        canonical_public_parameters,
        RamLfeBackend::BfvProgrammedSha3_256V1,
    )
}

fn build_policy_commitment(
    secret: &[u8],
    public_parameters: Vec<u8>,
    backend: RamLfeBackend,
) -> Result<PolicyCommitment, RamLfeError> {
    validate_secret(secret)?;
    let secret_commitment = Hash::new([SECRET_COMMITMENT_DOMAIN, secret].concat());
    let transcript = norito::to_bytes(&(backend, public_parameters.clone(), secret_commitment))
        .map_err(|err| RamLfeError::TranscriptEncoding(err.to_string()))?;
    let policy_hash = Hash::new([POLICY_DOMAIN, transcript.as_slice()].concat());
    Ok(PolicyCommitment {
        backend,
        policy_hash,
        public_parameters,
    })
}

/// Evaluate a request using the commitment-bound HKDF-SHA3-512 backend.
///
/// # Errors
/// Returns [`RamLfeError`] when the secret, commitment, request, or backend
/// transcript fails validation.
pub fn evaluate_commitment(
    secret: &[u8],
    commitment: &PolicyCommitment,
    request: &ClientRequest,
) -> Result<EvalResponse, RamLfeError> {
    evaluate_commitment_with_hidden_program(
        secret,
        commitment,
        request,
        Some(&default_bfv_programmed_hidden_program()),
    )
}

/// Evaluate a request using an explicit hidden program for programmed policies.
///
/// For non-programmed backends, `program` is ignored.
///
/// # Errors
/// Returns [`RamLfeError`] when the secret, commitment, request, or backend
/// transcript fails validation.
pub fn evaluate_commitment_with_hidden_program(
    secret: &[u8],
    commitment: &PolicyCommitment,
    request: &ClientRequest,
    program: Option<&HiddenRamFheProgram>,
) -> Result<EvalResponse, RamLfeError> {
    validate_secret(secret)?;
    validate_request(request)?;
    match commitment.backend {
        RamLfeBackend::HkdfSha3_512PrfV1 => evaluate_hkdf_prf(secret, commitment, request),
        RamLfeBackend::BfvAffineSha3_256V1 => evaluate_bfv_affine(secret, commitment, request),
        RamLfeBackend::BfvProgrammedSha3_256V1 => evaluate_bfv_programmed(
            secret,
            commitment,
            request,
            program.ok_or_else(|| {
                RamLfeError::UnsupportedBackend(
                    "missing hidden program for programmed BFV backend".to_owned(),
                )
            })?,
        ),
    }
}

fn evaluate_hkdf_prf(
    secret: &[u8],
    commitment: &PolicyCommitment,
    request: &ClientRequest,
) -> Result<EvalResponse, RamLfeError> {
    let expected = policy_commitment(secret, commitment.public_parameters.clone())?;
    if expected.policy_hash != commitment.policy_hash {
        return Err(RamLfeError::CommitmentMismatch);
    }

    let transcript = norito::to_bytes(&(
        expected.policy_hash,
        commitment.public_parameters.clone(),
        request.associated_data.clone(),
        request.normalized_input.clone(),
    ))
    .map_err(|err| RamLfeError::TranscriptEncoding(err.to_string()))?;
    let hkdf_salt = [HKDF_SALT_DOMAIN, expected.policy_hash.as_ref()].concat();
    let hkdf = Hkdf::<Sha3_512>::new(Some(&hkdf_salt), secret);

    let mut opaque_material = [0_u8; Hash::LENGTH];
    let opaque_info = [HKDF_OPAQUE_INFO_DOMAIN, transcript.as_slice()].concat();
    hkdf.expand(&opaque_info, &mut opaque_material)
        .map_err(|_| RamLfeError::DerivationFailed)?;

    let opaque_id = Hash::new([OPAQUE_HASH_DOMAIN, opaque_material.as_slice()].concat());

    let mut receipt_material = [0_u8; Hash::LENGTH];
    let receipt_info = [
        HKDF_RECEIPT_INFO_DOMAIN,
        transcript.as_slice(),
        opaque_id.as_ref(),
    ]
    .concat();
    hkdf.expand(&receipt_info, &mut receipt_material)
        .map_err(|_| RamLfeError::DerivationFailed)?;

    let receipt_hash = Hash::new(
        [
            RECEIPT_HASH_DOMAIN,
            receipt_material.as_slice(),
            opaque_id.as_ref(),
        ]
        .concat(),
    );
    Ok(EvalResponse {
        output: request.normalized_input.clone(),
        opaque_id,
        receipt_hash,
        backend: commitment.backend,
    })
}

fn evaluate_bfv_affine(
    secret: &[u8],
    commitment: &PolicyCommitment,
    request: &ClientRequest,
) -> Result<EvalResponse, RamLfeError> {
    let expected = bfv_affine_policy_commitment(secret, commitment.public_parameters.clone())?;
    if expected.policy_hash != commitment.policy_hash {
        return Err(RamLfeError::CommitmentMismatch);
    }

    let public_parameters = decode_bfv_public_parameters(&commitment.public_parameters)?;
    let (derived_public_parameters, secret_key, _) = derive_identifier_key_material_from_seed(
        &public_parameters.parameters,
        public_parameters.max_input_bytes,
        secret,
        &request.associated_data,
    )
    .map_err(|err| map_bfv_error(&err))?;
    if derived_public_parameters != public_parameters {
        return Err(RamLfeError::CommitmentMismatch);
    }

    let archived = norito::from_bytes::<BfvIdentifierCiphertext>(&request.normalized_input)
        .map_err(|err| RamLfeError::TranscriptEncoding(err.to_string()))?;
    let ciphertext: BfvIdentifierCiphertext =
        norito::core::NoritoDeserialize::deserialize(archived);
    let circuit = derive_secret_affine_circuit(secret, &public_parameters, commitment, request)?;
    let outputs =
        evaluate_affine_circuit(&public_parameters.parameters, &circuit, &ciphertext.slots)
            .map_err(|err| map_bfv_error(&err))?;
    let output_bytes = decrypt_affine_outputs(&public_parameters, &secret_key, &outputs)?;
    let opaque_id = Hash::new(
        [
            BFV_AFFINE_OPAQUE_HASH_DOMAIN,
            commitment.policy_hash.as_ref(),
            output_bytes.as_slice(),
        ]
        .concat(),
    );
    let receipt_hash = Hash::new(
        [
            BFV_AFFINE_RECEIPT_HASH_DOMAIN,
            commitment.policy_hash.as_ref(),
            output_bytes.as_slice(),
            opaque_id.as_ref(),
        ]
        .concat(),
    );
    Ok(EvalResponse {
        output: output_bytes,
        opaque_id,
        receipt_hash,
        backend: commitment.backend,
    })
}

fn evaluate_bfv_programmed(
    secret: &[u8],
    commitment: &PolicyCommitment,
    request: &ClientRequest,
    program: &HiddenRamFheProgram,
) -> Result<EvalResponse, RamLfeError> {
    let expected = bfv_programmed_policy_commitment_with_program(
        secret,
        &commitment.public_parameters,
        program,
    )?;
    if expected.policy_hash != commitment.policy_hash {
        return Err(RamLfeError::CommitmentMismatch);
    }

    let public_parameters = decode_bfv_programmed_public_parameters(&commitment.public_parameters)?;
    let encryption = &public_parameters.encryption;
    let (derived_public_parameters, secret_key, relinearization_key) =
        derive_identifier_key_material_from_seed(
            &encryption.parameters,
            encryption.max_input_bytes,
            secret,
            &request.associated_data,
        )
        .map_err(|err| map_bfv_error(&err))?;
    if derived_public_parameters != *encryption {
        return Err(RamLfeError::CommitmentMismatch);
    }

    let archived = norito::from_bytes::<BfvIdentifierCiphertext>(&request.normalized_input)
        .map_err(|err| RamLfeError::TranscriptEncoding(err.to_string()))?;
    let ciphertext: BfvIdentifierCiphertext =
        norito::core::NoritoDeserialize::deserialize(archived);
    if ciphertext.slots.is_empty() {
        return Err(RamLfeError::EmptyInput);
    }

    let expected_digest = program
        .digest()
        .map_err(|err| RamLfeError::TranscriptEncoding(err.to_string()))?;
    if expected_digest != public_parameters.hidden_program_digest {
        return Err(RamLfeError::HiddenProgramMismatch);
    }

    let mut state = derive_program_initial_state(
        &encryption.parameters,
        &ciphertext.slots[0],
        secret,
        commitment,
        request,
    )?;
    let execution = ProgramExecutionContext {
        params: &encryption.parameters,
        relinearization_key: &relinearization_key,
    };
    let output_bytes = execute_hidden_program(
        &execution,
        &secret_key,
        program,
        &ciphertext.slots,
        &mut state,
    )?;
    let opaque_id = Hash::new(
        [
            BFV_PROGRAM_OPAQUE_HASH_DOMAIN,
            commitment.policy_hash.as_ref(),
            output_bytes.as_slice(),
        ]
        .concat(),
    );
    let receipt_hash = Hash::new(
        [
            BFV_PROGRAM_RECEIPT_HASH_DOMAIN,
            commitment.policy_hash.as_ref(),
            output_bytes.as_slice(),
            opaque_id.as_ref(),
        ]
        .concat(),
    );
    Ok(EvalResponse {
        output: output_bytes,
        opaque_id,
        receipt_hash,
        backend: commitment.backend,
    })
}

fn validate_secret(secret: &[u8]) -> Result<(), RamLfeError> {
    if secret.is_empty() {
        return Err(RamLfeError::EmptySecret);
    }
    if secret.len() > MAX_SECRET_BYTES {
        return Err(RamLfeError::SecretTooLarge);
    }
    Ok(())
}

fn decode_bfv_public_parameters(
    public_parameters: &[u8],
) -> Result<BfvIdentifierPublicParameters, RamLfeError> {
    let archived = norito::from_bytes::<BfvIdentifierPublicParameters>(public_parameters)
        .map_err(|err| RamLfeError::TranscriptEncoding(err.to_string()))?;
    let public_parameters: BfvIdentifierPublicParameters =
        norito::core::NoritoDeserialize::deserialize(archived);
    public_parameters
        .validate()
        .map_err(|err| map_bfv_error(&err))?;
    Ok(public_parameters)
}

fn validate_programmed_public_parameters(
    public_parameters: &BfvIdentifierPublicParameters,
) -> Result<(), RamLfeError> {
    if public_parameters.parameters.ciphertext_modulus < BFV_PROGRAM_MIN_CIPHERTEXT_MODULUS {
        return Err(RamLfeError::Bfv(format!(
            "programmed BFV backend requires ciphertext_modulus >= {BFV_PROGRAM_MIN_CIPHERTEXT_MODULUS}"
        )));
    }
    Ok(())
}

fn validate_programmed_profile(profile: &BfvRamProgramProfile) -> Result<(), RamLfeError> {
    let expected = bfv_program_profile();
    if profile != &expected {
        return Err(RamLfeError::Bfv(
            "unsupported programmed BFV RAM-FHE profile".to_owned(),
        ));
    }
    Ok(())
}

fn validate_proof_verifier_metadata(
    verification_mode: RamLfeVerificationMode,
    proof_verifier: Option<&RamLfeProofVerifierMetadata>,
) -> Result<(), RamLfeError> {
    match (verification_mode, proof_verifier) {
        (RamLfeVerificationMode::Signed, Some(_)) => Err(RamLfeError::Bfv(
            "signed RAM-LFE programs must not publish proof verifier metadata".to_owned(),
        )),
        (RamLfeVerificationMode::Proof, None) => Err(RamLfeError::Bfv(
            "proof-carrying RAM-LFE programs must publish proof verifier metadata".to_owned(),
        )),
        (_, None) => Ok(()),
        (_, Some(metadata)) => {
            if metadata.proof_backend.trim().is_empty() {
                return Err(RamLfeError::Bfv(
                    "proof verifier backend must not be empty".to_owned(),
                ));
            }
            if metadata.circuit_id.trim().is_empty() {
                return Err(RamLfeError::Bfv(
                    "proof verifier circuit_id must not be empty".to_owned(),
                ));
            }
            if metadata.verifying_key_bytes.is_empty() {
                return Err(RamLfeError::Bfv(
                    "proof verifier bytes must not be empty".to_owned(),
                ));
            }
            Ok(())
        }
    }
}

fn derive_program_initial_state(
    params: &BfvParameters,
    reference_slot: &BfvCiphertext,
    secret: &[u8],
    commitment: &PolicyCommitment,
    request: &ClientRequest,
) -> Result<Vec<BfvCiphertext>, RamLfeError> {
    let zero = zero_ciphertext_like(params, reference_slot)?;
    let mut rng = derive_program_rng(secret, commitment, request, 0, BFV_PROGRAM_MEMORY_DOMAIN);
    (0..BFV_PROGRAM_STATE_WIDTH)
        .map(|_| {
            let bias = rng.random_range(0..params.plaintext_modulus);
            add_plain_scalar(params, &zero, bias).map_err(|err| map_bfv_error(&err))
        })
        .collect()
}

fn zero_ciphertext_like(
    params: &BfvParameters,
    reference_slot: &BfvCiphertext,
) -> Result<BfvCiphertext, RamLfeError> {
    multiply_plain_scalar(params, reference_slot, 0).map_err(|err| map_bfv_error(&err))
}

fn derive_program_rng(
    secret: &[u8],
    commitment: &PolicyCommitment,
    request: &ClientRequest,
    step: usize,
    domain: &[u8],
) -> ChaCha20Rng {
    let seed: [u8; Hash::LENGTH] = Hash::new(
        [
            domain,
            secret,
            commitment.policy_hash.as_ref(),
            request.associated_data.as_slice(),
            &u64::try_from(step)
                .expect("step fits into u64")
                .to_le_bytes(),
        ]
        .concat(),
    )
    .into();
    ChaCha20Rng::from_seed(seed)
}

fn execute_hidden_program(
    execution: &ProgramExecutionContext<'_>,
    secret_key: &crate::BfvSecretKey,
    program: &HiddenRamFheProgram,
    inputs: &[BfvCiphertext],
    state: &mut [BfvCiphertext],
) -> Result<Vec<u8>, RamLfeError> {
    validate_hidden_program(program)?;
    let mut machine =
        HiddenProgramMachine::new(execution, secret_key, program.register_count, inputs, state)?;
    for instruction in &program.instructions {
        machine.execute_instruction(*instruction)?;
    }
    machine.finish()
}

struct HiddenProgramMachine<'a> {
    execution: &'a ProgramExecutionContext<'a>,
    secret_key: &'a crate::BfvSecretKey,
    reference_input: &'a BfvCiphertext,
    state: &'a mut [BfvCiphertext],
    inputs: &'a [BfvCiphertext],
    registers: Vec<BfvCiphertext>,
    output_registers: Vec<BfvCiphertext>,
}

impl<'a> HiddenProgramMachine<'a> {
    fn new(
        execution: &'a ProgramExecutionContext<'a>,
        secret_key: &'a crate::BfvSecretKey,
        register_count: u16,
        inputs: &'a [BfvCiphertext],
        state: &'a mut [BfvCiphertext],
    ) -> Result<Self, RamLfeError> {
        let reference_input = inputs
            .first()
            .ok_or_else(|| invalid_program_error("program requires at least one input"))?;
        let zero = zero_ciphertext_like(execution.params, reference_input)?;
        Ok(Self {
            execution,
            secret_key,
            reference_input,
            state,
            inputs,
            registers: vec![zero; usize::from(register_count)],
            output_registers: Vec::new(),
        })
    }

    fn execute_instruction(
        &mut self,
        instruction: HiddenRamFheInstruction,
    ) -> Result<(), RamLfeError> {
        match instruction {
            HiddenRamFheInstruction::LoadInput(dst, input_index) => {
                let value = self
                    .inputs
                    .get(usize::from(input_index))
                    .ok_or_else(|| {
                        invalid_program_error(&format!("input slot {input_index} out of bounds"))
                    })?
                    .clone();
                *program_register_mut(&mut self.registers, usize::from(dst))? = value;
            }
            HiddenRamFheInstruction::LoadState(dst, lane) => {
                let value = self.program_lane(lane)?.clone();
                *program_register_mut(&mut self.registers, usize::from(dst))? = value;
            }
            HiddenRamFheInstruction::StoreState(lane, src) => {
                let value = program_register(&self.registers, usize::from(src))?.clone();
                *self.program_lane_mut(lane)? = value;
            }
            HiddenRamFheInstruction::LoadConst(dst, value) => {
                *program_register_mut(&mut self.registers, usize::from(dst))? =
                    self.load_constant(value)?;
            }
            HiddenRamFheInstruction::Add(dst, lhs, rhs) => {
                *program_register_mut(&mut self.registers, usize::from(dst))? =
                    self.add_registers(lhs, rhs)?;
            }
            HiddenRamFheInstruction::AddPlain(dst, src, value) => {
                *program_register_mut(&mut self.registers, usize::from(dst))? =
                    self.add_plain(src, value)?;
            }
            HiddenRamFheInstruction::SubPlain(dst, src, value) => {
                *program_register_mut(&mut self.registers, usize::from(dst))? =
                    self.sub_plain(src, value)?;
            }
            HiddenRamFheInstruction::MulPlain(dst, src, value) => {
                *program_register_mut(&mut self.registers, usize::from(dst))? =
                    self.mul_plain(src, value)?;
            }
            HiddenRamFheInstruction::Mul(dst, lhs, rhs) => {
                *program_register_mut(&mut self.registers, usize::from(dst))? =
                    self.mul_registers(lhs, rhs)?;
            }
            HiddenRamFheInstruction::SelectEqZero(dst, condition, if_zero, if_non_zero) => {
                *program_register_mut(&mut self.registers, usize::from(dst))? =
                    self.select_eq_zero(condition, if_zero, if_non_zero)?;
            }
            HiddenRamFheInstruction::Output(src) => {
                self.output_registers
                    .push(program_register(&self.registers, usize::from(src))?.clone());
            }
        }
        Ok(())
    }

    fn finish(self) -> Result<Vec<u8>, RamLfeError> {
        decrypt_program_outputs_from_registers(
            self.execution.params,
            self.secret_key,
            &self.output_registers,
        )
    }

    fn load_constant(&self, value: u64) -> Result<BfvCiphertext, RamLfeError> {
        let constant = add_plain_scalar(self.execution.params, self.reference_input, value)
            .map_err(|err| map_bfv_error(&err))?;
        let zeroed = multiply_plain_scalar(self.execution.params, &constant, 0)
            .map_err(|err| map_bfv_error(&err))?;
        add_plain_scalar(self.execution.params, &zeroed, value).map_err(|err| map_bfv_error(&err))
    }

    fn add_registers(&self, lhs: u16, rhs: u16) -> Result<BfvCiphertext, RamLfeError> {
        add_ciphertexts(
            self.execution.params,
            program_register(&self.registers, usize::from(lhs))?,
            program_register(&self.registers, usize::from(rhs))?,
        )
        .map_err(|err| map_bfv_error(&err))
    }

    fn add_plain(&self, src: u16, value: u64) -> Result<BfvCiphertext, RamLfeError> {
        add_plain_scalar(
            self.execution.params,
            program_register(&self.registers, usize::from(src))?,
            value,
        )
        .map_err(|err| map_bfv_error(&err))
    }

    fn sub_plain(&self, src: u16, value: u64) -> Result<BfvCiphertext, RamLfeError> {
        let scalar = (self
            .execution
            .params
            .plaintext_modulus
            .saturating_sub(value % self.execution.params.plaintext_modulus))
            % self.execution.params.plaintext_modulus;
        self.add_plain(src, scalar)
    }

    fn mul_plain(&self, src: u16, value: u64) -> Result<BfvCiphertext, RamLfeError> {
        multiply_plain_scalar(
            self.execution.params,
            program_register(&self.registers, usize::from(src))?,
            value,
        )
        .map_err(|err| map_bfv_error(&err))
    }

    fn mul_registers(&self, lhs: u16, rhs: u16) -> Result<BfvCiphertext, RamLfeError> {
        multiply_ciphertexts(
            self.execution.params,
            self.execution.relinearization_key,
            program_register(&self.registers, usize::from(lhs))?,
            program_register(&self.registers, usize::from(rhs))?,
        )
        .map_err(|err| map_bfv_error(&err))
    }

    fn select_eq_zero(
        &self,
        condition: u16,
        if_zero: u16,
        if_non_zero: u16,
    ) -> Result<BfvCiphertext, RamLfeError> {
        if decrypt_ciphertext_scalar(
            self.execution.params,
            self.secret_key,
            program_register(&self.registers, usize::from(condition))?,
        )? == 0
        {
            Ok(program_register(&self.registers, usize::from(if_zero))?.clone())
        } else {
            Ok(program_register(&self.registers, usize::from(if_non_zero))?.clone())
        }
    }

    fn program_lane(&self, lane: u16) -> Result<&BfvCiphertext, RamLfeError> {
        self.state
            .get(usize::from(lane))
            .ok_or_else(|| invalid_program_error(&format!("lane {lane} out of bounds")))
    }

    fn program_lane_mut(&mut self, lane: u16) -> Result<&mut BfvCiphertext, RamLfeError> {
        self.state
            .get_mut(usize::from(lane))
            .ok_or_else(|| invalid_program_error(&format!("lane {lane} out of bounds")))
    }
}

fn validate_hidden_program(program: &HiddenRamFheProgram) -> Result<(), RamLfeError> {
    if program.version != 1 {
        return Err(invalid_program_error("unsupported hidden program version"));
    }
    if usize::from(program.register_count) != BFV_PROGRAM_REGISTER_COUNT {
        return Err(invalid_program_error(
            "register_count does not match RAM-FHE profile",
        ));
    }
    if usize::from(program.memory_lane_count) != BFV_PROGRAM_STATE_WIDTH {
        return Err(invalid_program_error(
            "memory_lane_count does not match RAM-FHE profile",
        ));
    }
    if program.instructions.is_empty() {
        return Err(invalid_program_error(
            "program instruction tape must not be empty",
        ));
    }
    if !program
        .instructions
        .iter()
        .any(|instruction| matches!(instruction, HiddenRamFheInstruction::Output(..)))
    {
        return Err(invalid_program_error(
            "program must emit at least one output",
        ));
    }
    Ok(())
}

fn program_register(
    registers: &[BfvCiphertext],
    index: usize,
) -> Result<&BfvCiphertext, RamLfeError> {
    registers
        .get(index)
        .ok_or_else(|| invalid_program_error(&format!("register {index} out of bounds")))
}

fn program_register_mut(
    registers: &mut [BfvCiphertext],
    index: usize,
) -> Result<&mut BfvCiphertext, RamLfeError> {
    registers
        .get_mut(index)
        .ok_or_else(|| invalid_program_error(&format!("register {index} out of bounds")))
}

fn invalid_program_error(message: &str) -> RamLfeError {
    RamLfeError::Bfv(format!("invalid BFV RAM program: {message}"))
}

fn decrypt_ciphertext_scalar(
    params: &BfvParameters,
    secret_key: &crate::BfvSecretKey,
    ciphertext: &BfvCiphertext,
) -> Result<u64, RamLfeError> {
    let plaintext = decrypt(params, secret_key, ciphertext).map_err(|err| map_bfv_error(&err))?;
    Ok(*plaintext.first().unwrap_or(&0))
}

fn derive_secret_affine_circuit(
    secret: &[u8],
    public_parameters: &BfvIdentifierPublicParameters,
    commitment: &PolicyCommitment,
    request: &ClientRequest,
) -> Result<BfvAffineCircuit, RamLfeError> {
    let input_count = usize::from(public_parameters.max_input_bytes).saturating_add(1);
    let seed: [u8; Hash::LENGTH] = Hash::new(
        [
            BFV_AFFINE_CIRCUIT_DOMAIN,
            secret,
            commitment.policy_hash.as_ref(),
            request.associated_data.as_slice(),
        ]
        .concat(),
    )
    .into();
    let mut rng = ChaCha20Rng::from_seed(seed);
    let mut weights = Vec::with_capacity(BFV_AFFINE_OUTPUT_BYTES);
    let mut bias = Vec::with_capacity(BFV_AFFINE_OUTPUT_BYTES);
    for _ in 0..BFV_AFFINE_OUTPUT_BYTES {
        let row = (0..input_count)
            .map(|_| rng.random_range(0..public_parameters.parameters.plaintext_modulus))
            .collect::<Vec<_>>();
        weights.push(row);
        bias.push(rng.random_range(0..public_parameters.parameters.plaintext_modulus));
    }
    let circuit = BfvAffineCircuit { weights, bias };
    circuit
        .validate(&public_parameters.parameters, input_count)
        .map_err(|err| map_bfv_error(&err))?;
    Ok(circuit)
}

fn decrypt_affine_outputs(
    public_parameters: &BfvIdentifierPublicParameters,
    secret_key: &crate::BfvSecretKey,
    outputs: &[crate::BfvCiphertext],
) -> Result<Vec<u8>, RamLfeError> {
    outputs
        .iter()
        .map(|output| {
            let plaintext = decrypt(&public_parameters.parameters, secret_key, output)
                .map_err(|err| map_bfv_error(&err))?;
            if plaintext
                .iter()
                .skip(1)
                .any(|&coefficient| coefficient != 0)
            {
                return Err(RamLfeError::Bfv(
                    "affine output contains non-zero trailing coefficients".to_owned(),
                ));
            }
            u8::try_from(plaintext[0])
                .map_err(|_| RamLfeError::Bfv("affine output byte does not fit into u8".to_owned()))
        })
        .collect()
}

fn decrypt_program_outputs_from_registers(
    params: &BfvParameters,
    secret_key: &crate::BfvSecretKey,
    outputs: &[BfvCiphertext],
) -> Result<Vec<u8>, RamLfeError> {
    let mut bytes = Vec::with_capacity(outputs.len());
    for output in outputs {
        let plaintext = decrypt(params, secret_key, output).map_err(|err| map_bfv_error(&err))?;
        bytes.push(u8::try_from(plaintext[0]).map_err(|_| {
            RamLfeError::Bfv("program output byte does not fit into u8".to_owned())
        })?);
    }
    Ok(bytes)
}

fn map_bfv_error(err: &BfvError) -> RamLfeError {
    RamLfeError::Bfv(err.to_string())
}

fn validate_request(request: &ClientRequest) -> Result<(), RamLfeError> {
    if request.normalized_input.is_empty() {
        return Err(RamLfeError::EmptyInput);
    }
    if request.normalized_input.len() > MAX_INPUT_BYTES {
        return Err(RamLfeError::InputTooLarge);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::{
        BfvParameters, derive_identifier_key_material_from_seed, encrypt_identifier_from_seed,
    };

    use super::*;

    #[test]
    fn policy_commitment_roundtrip_evaluates() {
        let secret = b"resolver-secret";
        let commitment = policy_commitment(secret, b"phone#retail".to_vec()).expect("commitment");
        let request = ClientRequest {
            normalized_input: b"+15551234567".to_vec(),
            associated_data: b"phone#retail".to_vec(),
        };

        let first = evaluate_commitment(secret, &commitment, &request).expect("evaluation");
        let second = evaluate_commitment(secret, &commitment, &request).expect("evaluation");
        assert_eq!(first, second);
    }

    #[test]
    fn policy_commitment_rejects_wrong_secret() {
        let commitment =
            policy_commitment(b"secret-a", b"phone#retail".to_vec()).expect("commitment");
        let request = ClientRequest {
            normalized_input: b"+15551234567".to_vec(),
            associated_data: b"phone#retail".to_vec(),
        };

        let err = evaluate_commitment(b"secret-b", &commitment, &request)
            .expect_err("wrong secret must fail");
        assert_eq!(err, RamLfeError::CommitmentMismatch);
    }

    #[test]
    fn bfv_affine_policy_commitment_roundtrip_evaluates() {
        let secret = b"resolver-secret";
        let params = BfvParameters {
            polynomial_degree: 64,
            ciphertext_modulus: 1_u64 << 40,
            plaintext_modulus: 256,
            decomposition_base_log: 12,
        };
        let associated_data = b"phone#retail";
        let (public_parameters, _, _) =
            derive_identifier_key_material_from_seed(&params, 63, secret, associated_data)
                .expect("derive BFV public parameters");
        let commitment = bfv_affine_policy_commitment(
            secret,
            norito::to_bytes(&public_parameters).expect("encode public parameters"),
        )
        .expect("build BFV policy commitment");
        let ciphertext = encrypt_identifier_from_seed(
            &public_parameters,
            b"+15551234567",
            b"bfv-affine-identifier-seed",
        )
        .expect("encrypt identifier");
        let request = ClientRequest {
            normalized_input: norito::to_bytes(&ciphertext).expect("encode BFV ciphertext"),
            associated_data: associated_data.to_vec(),
        };

        let first = evaluate_commitment(secret, &commitment, &request).expect("evaluation");
        let second = evaluate_commitment(secret, &commitment, &request).expect("evaluation");
        assert_eq!(first, second);
        assert_eq!(first.backend, RamLfeBackend::BfvAffineSha3_256V1);
        assert_ne!(first.opaque_id, Hash::prehashed([0; Hash::LENGTH]));
    }

    #[test]
    fn bfv_programmed_policy_commitment_roundtrip_evaluates() {
        let secret = b"resolver-secret";
        let params = BfvParameters {
            polynomial_degree: 64,
            ciphertext_modulus: BFV_PROGRAM_MIN_CIPHERTEXT_MODULUS,
            plaintext_modulus: 256,
            decomposition_base_log: 12,
        };
        let associated_data = b"phone#retail";
        let (public_parameters, _, _) =
            derive_identifier_key_material_from_seed(&params, 63, secret, associated_data)
                .expect("derive BFV public parameters");
        let commitment = bfv_programmed_policy_commitment(
            secret,
            &norito::to_bytes(&public_parameters).expect("encode public parameters"),
        )
        .expect("build BFV policy commitment");
        let ciphertext = encrypt_identifier_from_seed(
            &public_parameters,
            b"+15551234567",
            b"bfv-programmed-identifier-seed",
        )
        .expect("encrypt identifier");
        let request = ClientRequest {
            normalized_input: norito::to_bytes(&ciphertext).expect("encode BFV ciphertext"),
            associated_data: associated_data.to_vec(),
        };

        let first = evaluate_commitment(secret, &commitment, &request).expect("evaluation");
        let second = evaluate_commitment(secret, &commitment, &request).expect("evaluation");
        assert_eq!(first, second);
        assert_eq!(first.backend, RamLfeBackend::BfvProgrammedSha3_256V1);
        assert_ne!(first.opaque_id, Hash::prehashed([0; Hash::LENGTH]));
    }

    #[test]
    fn bfv_programmed_public_parameters_upgrade_legacy_payload() {
        let secret = b"resolver-secret";
        let params = BfvParameters {
            polynomial_degree: 64,
            ciphertext_modulus: BFV_PROGRAM_MIN_CIPHERTEXT_MODULUS,
            plaintext_modulus: 256,
            decomposition_base_log: 12,
        };
        let associated_data = b"phone#retail";
        let (public_parameters, _, _) =
            derive_identifier_key_material_from_seed(&params, 63, secret, associated_data)
                .expect("derive BFV public parameters");
        let legacy_bytes = norito::to_bytes(&public_parameters).expect("encode legacy parameters");
        let upgraded = decode_bfv_programmed_public_parameters(&legacy_bytes)
            .expect("upgrade legacy programmed parameters");
        assert_eq!(upgraded.encryption, public_parameters);
        assert_eq!(upgraded.ram_fhe_profile, bfv_program_profile());

        let commitment = bfv_programmed_policy_commitment(secret, &legacy_bytes)
            .expect("build programmed policy commitment");
        let canonical = decode_bfv_programmed_public_parameters(&commitment.public_parameters)
            .expect("decode canonical programmed parameters");
        assert_eq!(canonical.encryption, public_parameters);
        assert_eq!(canonical.ram_fhe_profile, bfv_program_profile());
    }

    #[test]
    fn bfv_programmed_policy_commitment_changes_with_input() {
        let secret = b"resolver-secret";
        let params = BfvParameters {
            polynomial_degree: 64,
            ciphertext_modulus: BFV_PROGRAM_MIN_CIPHERTEXT_MODULUS,
            plaintext_modulus: 256,
            decomposition_base_log: 12,
        };
        let associated_data = b"phone#retail";
        let (public_parameters, _, _) =
            derive_identifier_key_material_from_seed(&params, 63, secret, associated_data)
                .expect("derive BFV public parameters");
        let commitment = bfv_programmed_policy_commitment(
            secret,
            &norito::to_bytes(&public_parameters).expect("encode public parameters"),
        )
        .expect("build BFV policy commitment");

        // The default hidden v1 program consumes the encoded length slot first, so the inputs
        // must differ in length to guarantee distinct outputs here.
        let left = ClientRequest {
            normalized_input: norito::to_bytes(
                &encrypt_identifier_from_seed(
                    &public_parameters,
                    b"alice@example.test",
                    b"bfv-programmed-left-seed",
                )
                .expect("encrypt left input"),
            )
            .expect("encode left ciphertext"),
            associated_data: associated_data.to_vec(),
        };
        let right = ClientRequest {
            normalized_input: norito::to_bytes(
                &encrypt_identifier_from_seed(
                    &public_parameters,
                    b"bravo@example.tests",
                    b"bfv-programmed-right-seed",
                )
                .expect("encrypt right input"),
            )
            .expect("encode right ciphertext"),
            associated_data: associated_data.to_vec(),
        };

        let left = evaluate_commitment(secret, &commitment, &left).expect("left evaluation");
        let right = evaluate_commitment(secret, &commitment, &right).expect("right evaluation");
        assert_ne!(left.opaque_id, right.opaque_id);
        assert_ne!(left.receipt_hash, right.receipt_hash);
    }
}
