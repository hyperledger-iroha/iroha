//! Identifier resolution service plumbing for app-facing endpoints.

use std::{
    collections::BTreeMap,
    sync::RwLock,
    time::{SystemTime, UNIX_EPOCH},
    vec::Vec,
};

use iroha_crypto::{
    BfvError, BfvIdentifierCiphertext, BfvIdentifierPublicParameters,
    BfvProgrammedPublicParameters, BfvRamProgramProfile, ClientRequest, EvalResponse, Hash,
    KeyPair, RamLfeBackend, RamLfeError, RamLfeVerificationMode, Signature, SignatureOf,
    decode_bfv_programmed_public_parameters, decrypt_identifier,
    derive_identifier_key_material_from_seed, encrypt_identifier_from_seed, evaluate_commitment,
    identifier_hashes_from_output_hash, ram_lfe_output_hash,
};
use iroha_data_model::{
    account::OpaqueAccountId,
    identifier::{
        IdentifierClaimRecord, IdentifierNormalization, IdentifierPolicy, IdentifierPolicyId,
        IdentifierResolutionReceipt, IdentifierResolutionReceiptPayload,
    },
    nexus::UniversalAccountId,
    prelude::*,
    ram_lfe::{RamLfeExecutionReceiptPayload, RamLfeProgramId, RamLfeProgramPolicy},
};
use thiserror::Error;

#[derive(Debug, Clone)]
struct PolicyRuntime {
    secret: Vec<u8>,
    signer: KeyPair,
    receipt_ttl_ms: Option<u64>,
}

/// In-process identifier resolver used by Torii app endpoints.
#[derive(Debug, Default)]
pub struct IdentifierResolutionService {
    // TODO: migrate this runtime map to `RamLfeProgramId` when
    // `torii.identifier_resolver` is replaced by the generic `torii.ram_lfe`
    // runtime config. The current map remains keyed by identifier policy id so
    // the existing config surface keeps working while the generic Torii API is
    // still being wired.
    policy_runtimes: RwLock<BTreeMap<IdentifierPolicyId, PolicyRuntime>>,
}

/// Draft returned by hidden-function evaluation before ledger binding lookup.
#[derive(Debug, Clone)]
pub struct IdentifierResolutionDraft {
    pub opaque_id: OpaqueAccountId,
    pub receipt_hash: Hash,
    pub resolved_at_ms: u64,
    pub expires_at_ms: Option<u64>,
    pub backend: RamLfeBackend,
    pub output_hash: Hash,
    pub program_digest: Hash,
    pub verification_mode: RamLfeVerificationMode,
}

#[derive(Debug, Error)]
pub enum IdentifierResolutionError {
    #[error("identifier policy {0} is not configured in the resolver service")]
    UnknownPolicy(IdentifierPolicyId),
    #[error("resolver signing key does not match the policy public key")]
    SignerMismatch,
    #[error("identifier policy does not publish BFV input-encryption parameters")]
    MissingFheParameters,
    #[error("identifier policy BFV parameters are invalid: {0}")]
    InvalidFheParameters(String),
    #[error("resolver BFV key material does not match the policy commitment")]
    FheKeyMismatch,
    #[error("encrypted identifier input is not valid UTF-8")]
    InvalidUtf8,
    #[error("RAM-LFE evaluation failed: {0}")]
    Evaluation(#[from] RamLfeError),
    #[error("BFV input decryption failed: {0}")]
    Fhe(#[from] BfvError),
    #[error("identifier policy transcript encoding failed: {0}")]
    Encoding(String),
}

impl IdentifierResolutionService {
    /// Create an empty resolver service.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Register in-process policy material for identifier resolution.
    pub fn register_policy_runtime(
        &self,
        policy_id: IdentifierPolicyId,
        secret: Vec<u8>,
        signer: KeyPair,
        receipt_ttl_ms: Option<u64>,
    ) {
        self.policy_runtimes
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(
                policy_id,
                PolicyRuntime {
                    secret,
                    signer,
                    receipt_ttl_ms,
                },
            );
    }

    /// Derive the opaque identifier for a normalized input under the given policy.
    pub fn derive(
        &self,
        policy: &IdentifierPolicy,
        program_policy: &RamLfeProgramPolicy,
        normalized_input: &str,
    ) -> Result<IdentifierResolutionDraft, IdentifierResolutionError> {
        let runtime = self.runtime(policy)?;
        let request_payload = match program_policy.commitment.backend {
            RamLfeBackend::HkdfSha3_512PrfV1 => normalized_input.as_bytes().to_vec(),
            RamLfeBackend::BfvAffineSha3_256V1 | RamLfeBackend::BfvProgrammedSha3_256V1 => {
                let public_parameters = decode_bfv_public_parameters(program_policy)?;
                let ciphertext = encrypt_identifier_from_seed(
                    &public_parameters,
                    normalized_input.as_bytes(),
                    &derive_plaintext_encryption_seed(policy, normalized_input),
                )?;
                norito::to_bytes(&ciphertext)
                    .map_err(|err| IdentifierResolutionError::Encoding(err.to_string()))?
            }
        };
        let request = ClientRequest {
            normalized_input: request_payload,
            associated_data: program_id_bytes(&program_policy.program_id),
        };
        let EvalResponse {
            output, backend, ..
        } = evaluate_commitment(&runtime.secret, &program_policy.commitment, &request)?;
        let output_hash = ram_lfe_output_hash(&output);
        let program_id_bytes = program_id_bytes(&program_policy.program_id);
        let (opaque_id, receipt_hash) =
            identifier_hashes_from_output_hash(&program_id_bytes, &output_hash);
        let programmed_public_parameters = decode_programmed_public_parameters(program_policy)?
            .ok_or(IdentifierResolutionError::MissingFheParameters)?;
        let resolved_at_ms = now_ms();
        let expires_at_ms = runtime
            .receipt_ttl_ms
            .and_then(|ttl| resolved_at_ms.checked_add(ttl));
        Ok(IdentifierResolutionDraft {
            opaque_id: OpaqueAccountId::from_hash(opaque_id),
            receipt_hash,
            resolved_at_ms,
            expires_at_ms,
            backend,
            output_hash,
            program_digest: programmed_public_parameters.hidden_program_digest,
            verification_mode: program_policy.verification_mode,
        })
    }

    /// Evaluate a BFV-encrypted identifier request under the selected policy.
    pub fn derive_encrypted(
        &self,
        policy: &IdentifierPolicy,
        program_policy: &RamLfeProgramPolicy,
        ciphertext: &BfvIdentifierCiphertext,
    ) -> Result<IdentifierResolutionDraft, IdentifierResolutionError> {
        match program_policy.commitment.backend {
            RamLfeBackend::HkdfSha3_512PrfV1 | RamLfeBackend::BfvProgrammedSha3_256V1 => {
                // Canonicalize onto the resolver's deterministic envelope so
                // receipt hashes stay stable across semantically equivalent BFV encryptions.
                let raw = self.decrypt_input(policy, program_policy, ciphertext)?;
                self.derive(policy, program_policy, &raw)
            }
            RamLfeBackend::BfvAffineSha3_256V1 => {
                let runtime = self.runtime(policy)?;
                let request = ClientRequest {
                    normalized_input: norito::to_bytes(ciphertext)
                        .map_err(|err| IdentifierResolutionError::Encoding(err.to_string()))?,
                    associated_data: program_id_bytes(&program_policy.program_id),
                };
                let EvalResponse {
                    output, backend, ..
                } = evaluate_commitment(&runtime.secret, &program_policy.commitment, &request)?;
                let output_hash = ram_lfe_output_hash(&output);
                let program_id_bytes = program_id_bytes(&program_policy.program_id);
                let (opaque_id, receipt_hash) =
                    identifier_hashes_from_output_hash(&program_id_bytes, &output_hash);
                let programmed_public_parameters =
                    decode_programmed_public_parameters(program_policy)?
                        .ok_or(IdentifierResolutionError::MissingFheParameters)?;
                let resolved_at_ms = now_ms();
                let expires_at_ms = runtime
                    .receipt_ttl_ms
                    .and_then(|ttl| resolved_at_ms.checked_add(ttl));
                Ok(IdentifierResolutionDraft {
                    opaque_id: OpaqueAccountId::from_hash(opaque_id),
                    receipt_hash,
                    resolved_at_ms,
                    expires_at_ms,
                    backend,
                    output_hash,
                    program_digest: programmed_public_parameters.hidden_program_digest,
                    verification_mode: program_policy.verification_mode,
                })
            }
        }
    }

    /// Decrypt BFV-wrapped identifier input published against the policy commitment.
    pub fn decrypt_input(
        &self,
        policy: &IdentifierPolicy,
        program_policy: &RamLfeProgramPolicy,
        ciphertext: &BfvIdentifierCiphertext,
    ) -> Result<String, IdentifierResolutionError> {
        let runtime = self.runtime(policy)?;
        let public_parameters = decode_bfv_public_parameters(program_policy)?;
        let associated_data = program_id_bytes(&program_policy.program_id);
        let (expected_public_parameters, secret_key, _) = derive_identifier_key_material_from_seed(
            &public_parameters.parameters,
            public_parameters.max_input_bytes,
            &runtime.secret,
            &associated_data,
        )?;
        if expected_public_parameters != public_parameters {
            return Err(IdentifierResolutionError::FheKeyMismatch);
        }
        let plaintext = decrypt_identifier(&public_parameters, &secret_key, ciphertext)?;
        String::from_utf8(plaintext).map_err(|_| IdentifierResolutionError::InvalidUtf8)
    }

    /// Sign a receipt binding a derived opaque identifier to the current ledger target.
    pub fn sign_receipt(
        &self,
        policy: &IdentifierPolicy,
        program_policy: &RamLfeProgramPolicy,
        draft: &IdentifierResolutionDraft,
        claim: &IdentifierClaimRecord,
    ) -> Result<IdentifierResolutionReceipt, IdentifierResolutionError> {
        self.issue_receipt(
            policy,
            program_policy,
            draft,
            claim.uaid,
            claim.account_id.clone(),
        )
    }

    /// Sign a receipt for a prospective claim before the ledger binding exists.
    pub fn issue_claim_receipt(
        &self,
        policy: &IdentifierPolicy,
        program_policy: &RamLfeProgramPolicy,
        draft: &IdentifierResolutionDraft,
        uaid: UniversalAccountId,
        account_id: AccountId,
    ) -> Result<IdentifierResolutionReceipt, IdentifierResolutionError> {
        self.issue_receipt(policy, program_policy, draft, uaid, account_id)
    }

    fn issue_receipt(
        &self,
        policy: &IdentifierPolicy,
        program_policy: &RamLfeProgramPolicy,
        draft: &IdentifierResolutionDraft,
        uaid: UniversalAccountId,
        account_id: AccountId,
    ) -> Result<IdentifierResolutionReceipt, IdentifierResolutionError> {
        let runtime = self.runtime(policy)?;
        if runtime.signer.public_key() != &program_policy.resolver_public_key {
            return Err(IdentifierResolutionError::SignerMismatch);
        }

        let execution = RamLfeExecutionReceiptPayload {
            program_id: program_policy.program_id.clone(),
            program_digest: draft.program_digest,
            backend: draft.backend,
            verification_mode: draft.verification_mode,
            output_hash: draft.output_hash,
            associated_data_hash: Hash::new(program_id_bytes(&program_policy.program_id)),
            executed_at_ms: draft.resolved_at_ms,
            expires_at_ms: draft.expires_at_ms,
        };
        let payload = IdentifierResolutionReceiptPayload {
            policy_id: policy.id.clone(),
            execution,
            opaque_id: draft.opaque_id,
            receipt_hash: draft.receipt_hash,
            uaid,
            account_id,
        };
        let signature: Signature = SignatureOf::new(runtime.signer.private_key(), &payload).into();

        Ok(IdentifierResolutionReceipt {
            payload,
            signature: Some(signature),
            proof: None,
        })
    }

    fn runtime(
        &self,
        policy: &IdentifierPolicy,
    ) -> Result<PolicyRuntime, IdentifierResolutionError> {
        self.policy_runtimes
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(&policy.id)
            .cloned()
            .ok_or_else(|| IdentifierResolutionError::UnknownPolicy(policy.id.clone()))
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or(0)
}

pub(crate) fn decode_bfv_public_parameters(
    program_policy: &RamLfeProgramPolicy,
) -> Result<BfvIdentifierPublicParameters, IdentifierResolutionError> {
    if program_policy.commitment.public_parameters.is_empty() {
        return Err(IdentifierResolutionError::MissingFheParameters);
    }
    match program_policy.commitment.backend {
        RamLfeBackend::BfvProgrammedSha3_256V1 => Ok(decode_bfv_programmed_public_parameters(
            &program_policy.commitment.public_parameters,
        )
        .map_err(|err| IdentifierResolutionError::InvalidFheParameters(err.to_string()))?
        .encryption),
        RamLfeBackend::HkdfSha3_512PrfV1 | RamLfeBackend::BfvAffineSha3_256V1 => {
            let archived = norito::from_bytes::<BfvIdentifierPublicParameters>(
                &program_policy.commitment.public_parameters,
            )
            .map_err(|err| IdentifierResolutionError::Encoding(err.to_string()))?;
            let public_parameters: BfvIdentifierPublicParameters =
                norito::core::NoritoDeserialize::deserialize(archived);
            public_parameters
                .validate()
                .map_err(|err| IdentifierResolutionError::InvalidFheParameters(err.to_string()))?;
            Ok(public_parameters)
        }
    }
}

pub(crate) fn decode_programmed_public_parameters(
    program_policy: &RamLfeProgramPolicy,
) -> Result<Option<BfvProgrammedPublicParameters>, IdentifierResolutionError> {
    if program_policy.commitment.backend != RamLfeBackend::BfvProgrammedSha3_256V1 {
        return Ok(None);
    }
    if program_policy.commitment.public_parameters.is_empty() {
        return Err(IdentifierResolutionError::MissingFheParameters);
    }
    decode_bfv_programmed_public_parameters(&program_policy.commitment.public_parameters)
        .map(Some)
        .map_err(|err| IdentifierResolutionError::InvalidFheParameters(err.to_string()))
}

pub(crate) fn decode_ram_fhe_profile(
    program_policy: &RamLfeProgramPolicy,
) -> Result<Option<BfvRamProgramProfile>, IdentifierResolutionError> {
    Ok(decode_programmed_public_parameters(program_policy)?.map(|value| value.ram_fhe_profile))
}

pub(crate) fn policy_id_bytes(policy_id: &IdentifierPolicyId) -> Vec<u8> {
    policy_id.to_string().into_bytes()
}

pub(crate) fn program_id_bytes(program_id: &RamLfeProgramId) -> Vec<u8> {
    norito::to_bytes(program_id).expect("RAM-LFE program id encoding must succeed")
}

fn derive_plaintext_encryption_seed(
    policy: &IdentifierPolicy,
    normalized_input: &str,
) -> [u8; Hash::LENGTH] {
    Hash::new(
        [
            b"iroha.identifier_resolution.plaintext_bfv.v1".as_slice(),
            policy.id.to_string().as_bytes(),
            normalized_input.as_bytes(),
        ]
        .concat(),
    )
    .into()
}

#[cfg(test)]
mod tests {
    use iroha_crypto::{
        BfvParameters, Hash, RamLfeBackend, RamLfeVerificationMode, SignatureOf,
        bfv_programmed_policy_commitment_with_program,
        bfv_programmed_public_parameters_with_program, default_bfv_programmed_hidden_program,
        encrypt_identifier_from_seed,
    };
    use iroha_data_model::ram_lfe::{RamLfeProgramId, RamLfeProgramPolicy};

    use super::*;

    fn sample_policy_bundle(
        policy_id: IdentifierPolicyId,
        owner: AccountId,
        signer: &KeyPair,
        secret: &[u8],
    ) -> (IdentifierPolicy, RamLfeProgramPolicy) {
        let backend = RamLfeBackend::BfvProgrammedSha3_256V1;
        let params = sample_identifier_bfv_parameters();
        let program_id = sample_program_id(&policy_id);
        let hidden_program = default_bfv_programmed_hidden_program();
        let (public_parameters, _, _) = derive_identifier_key_material_from_seed(
            &params,
            63,
            secret,
            &program_id_bytes(&program_id),
        )
        .expect("identifier BFV parameters");
        let programmed_public_parameters = bfv_programmed_public_parameters_with_program(
            public_parameters,
            &hidden_program,
            RamLfeVerificationMode::Signed,
            None,
        );
        let encoded_public_parameters =
            norito::to_bytes(&programmed_public_parameters).expect("encode BFV parameters");
        let program_policy = RamLfeProgramPolicy::new(
            program_id.clone(),
            owner.clone(),
            backend,
            RamLfeVerificationMode::Signed,
            bfv_programmed_policy_commitment_with_program(
                secret,
                &encoded_public_parameters,
                &hidden_program,
            )
            .expect("policy commitment"),
            signer.public_key().clone(),
        );
        let policy = IdentifierPolicy::new(
            policy_id.clone(),
            owner,
            IdentifierNormalization::PhoneE164,
            program_id,
        );
        (policy, program_policy)
    }

    fn sample_identifier_bfv_parameters() -> BfvParameters {
        BfvParameters {
            polynomial_degree: 64,
            ciphertext_modulus: 1_u64 << 52,
            plaintext_modulus: 256,
            decomposition_base_log: 12,
        }
    }

    fn sample_program_id(policy_id: &IdentifierPolicyId) -> RamLfeProgramId {
        format!("{}_{}", policy_id.kind, policy_id.business_rule)
            .parse()
            .expect("program id")
    }

    #[test]
    fn derive_and_sign_receipt_roundtrip() {
        let service = IdentifierResolutionService::new();
        let owner = AccountId::new(KeyPair::random().public_key().clone());
        let signer = KeyPair::random();
        let policy_id: IdentifierPolicyId = "phone#retail".parse().expect("policy id");
        let secret = b"hidden-phone-policy".to_vec();
        let (policy, program_policy) =
            sample_policy_bundle(policy_id.clone(), owner.clone(), &signer, &secret);
        service.register_policy_runtime(policy_id.clone(), secret, signer.clone(), Some(30_000));

        let draft = service
            .derive(&policy, &program_policy, "+15551234567")
            .expect("derive opaque identifier");
        let claim = IdentifierClaimRecord {
            policy_id: policy_id.clone(),
            opaque_id: draft.opaque_id,
            receipt_hash: draft.receipt_hash,
            uaid: UniversalAccountId::from_hash(Hash::new(b"uaid")),
            account_id: owner.clone(),
            verified_at_ms: draft.resolved_at_ms,
            expires_at_ms: None,
        };

        let receipt = service
            .sign_receipt(&policy, &program_policy, &draft, &claim)
            .expect("sign receipt");

        SignatureOf::<IdentifierResolutionReceiptPayload>::from_signature(
            receipt.signature.clone().expect("signature"),
        )
        .verify(&program_policy.resolver_public_key, &receipt.payload)
        .expect("receipt signature should verify");
        assert_eq!(receipt.payload.policy_id, policy_id);
        assert_eq!(receipt.payload.opaque_id, draft.opaque_id);
        assert_eq!(receipt.payload.receipt_hash, draft.receipt_hash);
        assert_eq!(receipt.payload.uaid, claim.uaid);
        assert_eq!(receipt.payload.account_id, owner);
    }

    #[test]
    fn derive_rejects_unregistered_policy() {
        let service = IdentifierResolutionService::new();
        let owner = AccountId::new(KeyPair::random().public_key().clone());
        let signer = KeyPair::random();
        let policy_id: IdentifierPolicyId = "email#retail".parse().expect("policy id");
        let (policy, program_policy) =
            sample_policy_bundle(policy_id.clone(), owner, &signer, b"hidden-email-policy");

        let err = service
            .derive(&policy, &program_policy, "alice@example.com")
            .expect_err("missing runtime must fail");
        assert!(matches!(
            err,
            IdentifierResolutionError::UnknownPolicy(found) if found == policy_id
        ));
    }

    #[test]
    fn decrypts_encrypted_identifier_input() {
        let service = IdentifierResolutionService::new();
        let owner = AccountId::new(KeyPair::random().public_key().clone());
        let signer = KeyPair::random();
        let policy_id: IdentifierPolicyId = "phone#retail".parse().expect("policy id");
        let secret = b"hidden-phone-policy".to_vec();
        let (policy, program_policy) =
            sample_policy_bundle(policy_id.clone(), owner, &signer, &secret);
        service.register_policy_runtime(policy_id.clone(), secret, signer, Some(30_000));

        let public_parameters =
            decode_bfv_public_parameters(&program_policy).expect("decode BFV params");
        let ciphertext = encrypt_identifier_from_seed(
            &public_parameters,
            b"+15551234567",
            b"identifier-bfv-ciphertext",
        )
        .expect("encrypt identifier input");

        let decrypted = service
            .decrypt_input(&policy, &program_policy, &ciphertext)
            .expect("decrypt input");
        assert_eq!(decrypted, "+15551234567");
    }

    #[test]
    fn programmed_backend_derives_deterministic_receipts() {
        let service = IdentifierResolutionService::new();
        let owner = AccountId::new(KeyPair::random().public_key().clone());
        let signer = KeyPair::random();
        let policy_id: IdentifierPolicyId = "phone#retail".parse().expect("policy id");
        let secret = b"hidden-phone-policy".to_vec();
        let (policy, program_policy) =
            sample_policy_bundle(policy_id.clone(), owner, &signer, &secret);
        service.register_policy_runtime(policy_id, secret, signer, Some(30_000));

        let first = service
            .derive(&policy, &program_policy, "+15551234567")
            .expect("first derive");
        let second = service
            .derive(&policy, &program_policy, "+15551234567")
            .expect("second derive");
        assert_eq!(first.opaque_id, second.opaque_id);
        assert_eq!(first.receipt_hash, second.receipt_hash);
        assert_eq!(first.backend, RamLfeBackend::BfvProgrammedSha3_256V1);
    }

    #[test]
    fn programmed_backend_matches_plaintext_and_encrypted_resolution() {
        let service = IdentifierResolutionService::new();
        let owner = AccountId::new(KeyPair::random().public_key().clone());
        let signer = KeyPair::random();
        let policy_id: IdentifierPolicyId = "phone#retail".parse().expect("policy id");
        let secret = b"hidden-phone-policy".to_vec();
        let (policy, program_policy) =
            sample_policy_bundle(policy_id.clone(), owner, &signer, &secret);
        service.register_policy_runtime(policy_id, secret, signer, Some(30_000));

        let plaintext = service
            .derive(&policy, &program_policy, "+15551234567")
            .expect("plaintext derive");
        let public_parameters =
            decode_bfv_public_parameters(&program_policy).expect("decode BFV params");
        let ciphertext = encrypt_identifier_from_seed(
            &public_parameters,
            b"+15551234567",
            b"programmed-bfv-ciphertext",
        )
        .expect("encrypt identifier input");
        let encrypted = service
            .derive_encrypted(&policy, &program_policy, &ciphertext)
            .expect("encrypted derive");

        assert_eq!(plaintext.opaque_id, encrypted.opaque_id);
        assert_eq!(plaintext.receipt_hash, encrypted.receipt_hash);
        assert_eq!(encrypted.backend, RamLfeBackend::BfvProgrammedSha3_256V1);
    }
}
