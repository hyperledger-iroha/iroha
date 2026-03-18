//! Identifier resolution service plumbing for app-facing endpoints.

use std::{
    collections::BTreeMap,
    sync::RwLock,
    time::{SystemTime, UNIX_EPOCH},
    vec::Vec,
};

use iroha_crypto::{
    BfvError, BfvIdentifierCiphertext, BfvIdentifierPublicParameters, ClientRequest, EvalResponse,
    Hash, KeyPair, RamLfeBackend, RamLfeError, Signature, SignatureOf, decrypt_identifier,
    derive_identifier_key_material_from_seed, encrypt_identifier_from_seed, evaluate_commitment,
};
use iroha_data_model::{
    account::OpaqueAccountId,
    identifier::{
        IdentifierClaimRecord, IdentifierNormalization, IdentifierPolicy, IdentifierPolicyId,
        IdentifierResolutionReceipt, IdentifierResolutionReceiptPayload,
    },
    nexus::UniversalAccountId,
    prelude::*,
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
        normalized_input: &str,
    ) -> Result<IdentifierResolutionDraft, IdentifierResolutionError> {
        let runtime = self.runtime(policy)?;
        let request_payload = match policy.commitment.backend {
            RamLfeBackend::HkdfSha3_512PrfV1 => normalized_input.as_bytes().to_vec(),
            RamLfeBackend::BfvAffineSha3_256V1 => {
                let public_parameters = decode_bfv_public_parameters(policy)?;
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
            associated_data: policy_id_bytes(&policy.id),
        };
        let EvalResponse {
            opaque_id,
            receipt_hash,
            backend,
        } = evaluate_commitment(&runtime.secret, &policy.commitment, &request)?;
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
        })
    }

    /// Evaluate a BFV-encrypted identifier request under the selected policy.
    pub fn derive_encrypted(
        &self,
        policy: &IdentifierPolicy,
        ciphertext: &BfvIdentifierCiphertext,
    ) -> Result<IdentifierResolutionDraft, IdentifierResolutionError> {
        match policy.commitment.backend {
            RamLfeBackend::HkdfSha3_512PrfV1 => {
                let raw = self.decrypt_input(policy, ciphertext)?;
                self.derive(policy, &raw)
            }
            RamLfeBackend::BfvAffineSha3_256V1 => {
                let runtime = self.runtime(policy)?;
                let request = ClientRequest {
                    normalized_input: norito::to_bytes(ciphertext)
                        .map_err(|err| IdentifierResolutionError::Encoding(err.to_string()))?,
                    associated_data: policy_id_bytes(&policy.id),
                };
                let EvalResponse {
                    opaque_id,
                    receipt_hash,
                    backend,
                } = evaluate_commitment(&runtime.secret, &policy.commitment, &request)?;
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
                })
            }
        }
    }

    /// Decrypt BFV-wrapped identifier input published against the policy commitment.
    pub fn decrypt_input(
        &self,
        policy: &IdentifierPolicy,
        ciphertext: &BfvIdentifierCiphertext,
    ) -> Result<String, IdentifierResolutionError> {
        let runtime = self.runtime(policy)?;
        let public_parameters = decode_bfv_public_parameters(policy)?;
        let associated_data = policy_id_bytes(&policy.id);
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
        draft: &IdentifierResolutionDraft,
        claim: &IdentifierClaimRecord,
    ) -> Result<IdentifierResolutionReceipt, IdentifierResolutionError> {
        self.issue_receipt(policy, draft, claim.uaid, claim.account_id.clone())
    }

    /// Sign a receipt for a prospective claim before the ledger binding exists.
    pub fn issue_claim_receipt(
        &self,
        policy: &IdentifierPolicy,
        draft: &IdentifierResolutionDraft,
        uaid: UniversalAccountId,
        account_id: AccountId,
    ) -> Result<IdentifierResolutionReceipt, IdentifierResolutionError> {
        self.issue_receipt(policy, draft, uaid, account_id)
    }

    fn issue_receipt(
        &self,
        policy: &IdentifierPolicy,
        draft: &IdentifierResolutionDraft,
        uaid: UniversalAccountId,
        account_id: AccountId,
    ) -> Result<IdentifierResolutionReceipt, IdentifierResolutionError> {
        let runtime = self.runtime(policy)?;
        if runtime.signer.public_key() != &policy.resolver_public_key {
            return Err(IdentifierResolutionError::SignerMismatch);
        }

        let payload = IdentifierResolutionReceiptPayload {
            policy_id: policy.id.clone(),
            opaque_id: draft.opaque_id,
            receipt_hash: draft.receipt_hash,
            uaid,
            account_id,
            resolved_at_ms: draft.resolved_at_ms,
            expires_at_ms: draft.expires_at_ms,
        };
        let signature: Signature = SignatureOf::new(runtime.signer.private_key(), &payload).into();

        Ok(IdentifierResolutionReceipt {
            policy_id: payload.policy_id.clone(),
            opaque_id: payload.opaque_id,
            receipt_hash: payload.receipt_hash,
            uaid: payload.uaid,
            account_id: payload.account_id.clone(),
            resolved_at_ms: payload.resolved_at_ms,
            expires_at_ms: payload.expires_at_ms,
            signature,
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
    policy: &IdentifierPolicy,
) -> Result<BfvIdentifierPublicParameters, IdentifierResolutionError> {
    if policy.commitment.public_parameters.is_empty() {
        return Err(IdentifierResolutionError::MissingFheParameters);
    }
    let archived =
        norito::from_bytes::<BfvIdentifierPublicParameters>(&policy.commitment.public_parameters)
            .map_err(|err| IdentifierResolutionError::Encoding(err.to_string()))?;
    let public_parameters: BfvIdentifierPublicParameters =
        norito::core::NoritoDeserialize::deserialize(archived);
    public_parameters
        .validate()
        .map_err(|err| IdentifierResolutionError::InvalidFheParameters(err.to_string()))?;
    Ok(public_parameters)
}

pub(crate) fn policy_id_bytes(policy_id: &IdentifierPolicyId) -> Vec<u8> {
    policy_id.to_string().into_bytes()
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
        BfvParameters, Hash, SignatureOf, bfv_affine_policy_commitment,
        encrypt_identifier_from_seed,
    };

    use super::*;

    fn sample_policy(
        policy_id: IdentifierPolicyId,
        owner: AccountId,
        signer: &KeyPair,
        secret: &[u8],
    ) -> IdentifierPolicy {
        let params = sample_identifier_bfv_parameters();
        let (public_parameters, _, _) = derive_identifier_key_material_from_seed(
            &params,
            63,
            secret,
            &policy_id_bytes(&policy_id),
        )
        .expect("identifier BFV parameters");
        IdentifierPolicy::new(
            policy_id.clone(),
            owner,
            IdentifierNormalization::PhoneE164,
            bfv_affine_policy_commitment(
                secret,
                norito::to_bytes(&public_parameters).expect("encode BFV parameters"),
            )
            .expect("policy commitment"),
            signer.public_key().clone(),
        )
    }

    fn sample_identifier_bfv_parameters() -> BfvParameters {
        BfvParameters {
            polynomial_degree: 64,
            ciphertext_modulus: 1_u64 << 40,
            plaintext_modulus: 256,
            decomposition_base_log: 12,
        }
    }

    #[test]
    fn derive_and_sign_receipt_roundtrip() {
        let service = IdentifierResolutionService::new();
        let owner = AccountId::new(KeyPair::random().public_key().clone());
        let signer = KeyPair::random();
        let policy_id: IdentifierPolicyId = "phone#retail".parse().expect("policy id");
        let secret = b"hidden-phone-policy".to_vec();
        let policy = sample_policy(policy_id.clone(), owner.clone(), &signer, &secret);
        service.register_policy_runtime(policy_id.clone(), secret, signer.clone(), Some(30_000));

        let draft = service
            .derive(&policy, "+15551234567")
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
            .sign_receipt(&policy, &draft, &claim)
            .expect("sign receipt");
        let payload = IdentifierResolutionReceiptPayload {
            policy_id,
            opaque_id: draft.opaque_id,
            receipt_hash: draft.receipt_hash,
            uaid: claim.uaid,
            account_id: owner,
            resolved_at_ms: draft.resolved_at_ms,
            expires_at_ms: draft.expires_at_ms,
        };

        SignatureOf::<IdentifierResolutionReceiptPayload>::from_signature(
            receipt.signature.clone(),
        )
        .verify(&policy.resolver_public_key, &payload)
        .expect("receipt signature should verify");
        assert_eq!(receipt.opaque_id, draft.opaque_id);
        assert_eq!(receipt.receipt_hash, draft.receipt_hash);
        assert_eq!(receipt.uaid, claim.uaid);
        assert_eq!(receipt.account_id, claim.account_id);
    }

    #[test]
    fn derive_rejects_unregistered_policy() {
        let service = IdentifierResolutionService::new();
        let owner = AccountId::new(KeyPair::random().public_key().clone());
        let signer = KeyPair::random();
        let policy_id: IdentifierPolicyId = "email#retail".parse().expect("policy id");
        let policy = sample_policy(policy_id.clone(), owner, &signer, b"hidden-email-policy");

        let err = service
            .derive(&policy, "alice@example.com")
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
        let policy = sample_policy(policy_id.clone(), owner, &signer, &secret);
        service.register_policy_runtime(policy_id.clone(), secret, signer, Some(30_000));

        let public_parameters = decode_bfv_public_parameters(&policy).expect("decode BFV params");
        let ciphertext = encrypt_identifier_from_seed(
            &public_parameters,
            b"+15551234567",
            b"identifier-bfv-ciphertext",
        )
        .expect("encrypt identifier input");

        let decrypted = service
            .decrypt_input(&policy, &ciphertext)
            .expect("decrypt input");
        assert_eq!(decrypted, "+15551234567");
    }
}
