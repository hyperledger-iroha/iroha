//! Reusable identifier-policy commitment and evaluation plumbing.
//!
//! This module wires commitment-bound identifier evaluators, including:
//! - the historical committed `HKDF-SHA3-512` PRF backend, and
//! - a BFV-backed secret affine evaluator that consumes BFV-encrypted input.

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
    BfvAffineCircuit, BfvError, BfvIdentifierCiphertext, BfvIdentifierPublicParameters, Hash,
    decrypt, derive_identifier_key_material_from_seed, evaluate_affine_circuit,
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
const BFV_AFFINE_OUTPUT_BYTES: usize = Hash::LENGTH;
const MAX_INPUT_BYTES: usize = 1_048_576;
const MAX_SECRET_BYTES: usize = 4096;

/// Supported RAM-LFE backends.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
pub enum RamLfeBackend {
    /// HKDF-SHA3-512 commitment-bound PRF evaluator.
    HkdfSha3_512PrfV1,
    /// BFV-backed secret affine evaluator producing a 32-byte opaque seed.
    BfvAffineSha3_256V1,
}

impl RamLfeBackend {
    /// Stable backend identifier.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::HkdfSha3_512PrfV1 => "hkdf-sha3-512-prf-v1",
            Self::BfvAffineSha3_256V1 => "bfv-affine-sha3-256-v1",
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
pub struct EvalResponse {
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
    fn evaluate(
        &self,
        commitment: &PolicyCommitment,
        request: &ClientRequest,
    ) -> Result<EvalResponse, RamLfeError>;
}

/// Construct the commitment record for the built-in HKDF-SHA3-512 backend.
pub fn policy_commitment(
    secret: &[u8],
    public_parameters: Vec<u8>,
) -> Result<PolicyCommitment, RamLfeError> {
    build_policy_commitment(secret, public_parameters, RamLfeBackend::HkdfSha3_512PrfV1)
}

/// Construct the commitment record for the BFV secret affine backend.
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
pub fn evaluate_commitment(
    secret: &[u8],
    commitment: &PolicyCommitment,
    request: &ClientRequest,
) -> Result<EvalResponse, RamLfeError> {
    validate_secret(secret)?;
    validate_request(request)?;
    match commitment.backend {
        RamLfeBackend::HkdfSha3_512PrfV1 => evaluate_hkdf_prf(secret, commitment, request),
        RamLfeBackend::BfvAffineSha3_256V1 => evaluate_bfv_affine(secret, commitment, request),
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
    .map_err(map_bfv_error)?;
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
            .map_err(map_bfv_error)?;
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
    public_parameters.validate().map_err(map_bfv_error)?;
    Ok(public_parameters)
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
        .map_err(map_bfv_error)?;
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
                .map_err(map_bfv_error)?;
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

fn map_bfv_error(err: BfvError) -> RamLfeError {
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
}
