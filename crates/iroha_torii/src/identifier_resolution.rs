//! Identifier resolution service plumbing for app-facing endpoints.

use std::{
    collections::BTreeMap,
    sync::RwLock,
    time::{SystemTime, UNIX_EPOCH},
    vec::Vec,
};

use iroha_crypto::{
    BfvIdentifierCiphertext, BfvIdentifierPublicParameters, BfvProgrammedPublicParameters,
    BfvRamProgramProfile, ClientRequest, EvalResponse, Hash, HiddenRamFheProgram, KeyPair,
    RamLfeBackend, RamLfeError, RamLfeVerificationMode, Signature, SignatureOf,
    decode_bfv_programmed_public_parameters, evaluate_commitment_with_hidden_program,
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
    ram_lfe::{
        RamLfeExecutionReceiptPayload, RamLfeOutputOpening, RamLfeOutputOpeningPayload,
        RamLfeProgramId, RamLfeProgramPolicy, RamLfeReceiptAttestation,
    },
};
use thiserror::Error;

#[derive(Debug, Clone)]
struct ProgramRuntime {
    secret: Vec<u8>,
    hidden_program: HiddenRamFheProgram,
    signer: KeyPair,
    receipt_ttl_ms: Option<u64>,
}

/// In-process RAM-LFE runtime used by Torii app endpoints.
#[derive(Debug, Default)]
pub struct IdentifierResolutionService {
    program_runtimes: RwLock<BTreeMap<RamLfeProgramId, ProgramRuntime>>,
}

/// Draft returned by RAM-LFE execution before route-specific projection.
#[derive(Debug, Clone)]
pub struct RamLfeExecutionDraft {
    pub output: Vec<u8>,
    pub opaque_hash: Hash,
    pub receipt_hash: Hash,
    pub executed_at_ms: u64,
    pub expires_at_ms: Option<u64>,
    pub backend: RamLfeBackend,
    pub output_hash: Hash,
    pub input_ciphertext_hash: Hash,
    pub output_ciphertext_hash: Hash,
    pub associated_data_hash: Hash,
    pub program_digest: Hash,
    pub parameter_digest: Hash,
    pub evaluation_key_digest: Hash,
    pub verification_mode: RamLfeVerificationMode,
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
    pub input_ciphertext_hash: Hash,
    pub output_ciphertext_hash: Hash,
    pub program_digest: Hash,
    pub parameter_digest: Hash,
    pub evaluation_key_digest: Hash,
    pub verification_mode: RamLfeVerificationMode,
    pub opening: RamLfeOutputOpening,
}

#[derive(Debug, Error)]
pub enum IdentifierResolutionError {
    #[error("RAM-LFE program {0} is not configured in the Torii runtime")]
    UnknownProgram(RamLfeProgramId),
    #[error("resolver signing key does not match the policy public key")]
    SignerMismatch,
    #[error("identifier policy does not publish BFV input-encryption parameters")]
    MissingFheParameters,
    #[error("identifier policy BFV parameters are invalid: {0}")]
    InvalidFheParameters(String),
    #[error("RAM-LFE backend {0:?} does not yet support Torii app execution receipts")]
    UnsupportedBackend(RamLfeBackend),
    #[error("RAM-LFE output opening is missing")]
    MissingOutputOpening,
    #[error("RAM-LFE output opening is invalid: {0}")]
    InvalidOutputOpening(String),
    #[error("resolver BFV key material does not match the policy commitment")]
    FheKeyMismatch,
    #[error("encrypted identifier input is not valid UTF-8")]
    InvalidUtf8,
    #[error("RAM-LFE evaluation failed: {0}")]
    Evaluation(#[from] RamLfeError),
    #[error("identifier policy transcript encoding failed: {0}")]
    Encoding(String),
    #[error("Torii cannot issue proof-mode RAM-LFE receipts without prover runtime support")]
    ProofModeUnsupported,
}

impl IdentifierResolutionService {
    /// Create an empty resolver service.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Register in-process program material for RAM-LFE execution.
    pub fn register_program_runtime(
        &self,
        program_id: RamLfeProgramId,
        secret: Vec<u8>,
        hidden_program: HiddenRamFheProgram,
        signer: KeyPair,
        receipt_ttl_ms: Option<u64>,
    ) {
        self.program_runtimes
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(
                program_id,
                ProgramRuntime {
                    secret,
                    hidden_program,
                    signer,
                    receipt_ttl_ms,
                },
            );
    }

    /// Execute one RAM-LFE program from a BFV ciphertext envelope.
    pub fn execute_encrypted(
        &self,
        program_policy: &RamLfeProgramPolicy,
        ciphertext: &BfvIdentifierCiphertext,
    ) -> Result<RamLfeExecutionDraft, IdentifierResolutionError> {
        if program_policy.commitment.backend != RamLfeBackend::BfvProgrammedSha3_256V1 {
            return Err(IdentifierResolutionError::UnsupportedBackend(
                program_policy.commitment.backend,
            ));
        }
        self.execute_request_payload(
            program_policy,
            norito::to_bytes(ciphertext)
                .map_err(|err| IdentifierResolutionError::Encoding(err.to_string()))?,
        )
    }

    fn execute_request_payload(
        &self,
        program_policy: &RamLfeProgramPolicy,
        request_payload: Vec<u8>,
    ) -> Result<RamLfeExecutionDraft, IdentifierResolutionError> {
        let runtime = self.runtime(program_policy)?;
        let associated_data = program_id_bytes(&program_policy.program_id);
        let request = ClientRequest {
            normalized_input: request_payload,
            associated_data: associated_data.clone(),
        };
        let EvalResponse {
            output,
            opaque_id,
            receipt_hash,
            backend,
        } = evaluate_commitment_with_hidden_program(
            &runtime.secret,
            &program_policy.commitment,
            &request,
            Some(&runtime.hidden_program),
        )?;
        let output_hash = ram_lfe_output_hash(&output);
        let input_ciphertext_hash = Hash::new(&request.normalized_input);
        let output_ciphertext_hash = output_hash;
        let programmed_public_parameters = decode_programmed_public_parameters(program_policy)?
            .ok_or(IdentifierResolutionError::UnsupportedBackend(
                program_policy.commitment.backend,
            ))?;
        let executed_at_ms = now_ms();
        let expires_at_ms = runtime
            .receipt_ttl_ms
            .and_then(|ttl| executed_at_ms.checked_add(ttl));
        Ok(RamLfeExecutionDraft {
            output,
            opaque_hash: opaque_id,
            receipt_hash,
            executed_at_ms,
            expires_at_ms,
            backend,
            output_hash,
            input_ciphertext_hash,
            output_ciphertext_hash,
            associated_data_hash: Hash::new(associated_data),
            program_digest: programmed_public_parameters.hidden_program_digest,
            parameter_digest: programmed_public_parameters.parameter_digest,
            evaluation_key_digest: programmed_public_parameters.evaluation_key_digest,
            verification_mode: program_policy.verification_mode,
        })
    }

    /// Evaluate a BFV-encrypted identifier request under the selected policy.
    pub fn derive_encrypted(
        &self,
        _policy: &IdentifierPolicy,
        program_policy: &RamLfeProgramPolicy,
        ciphertext: &BfvIdentifierCiphertext,
        opening: RamLfeOutputOpening,
    ) -> Result<IdentifierResolutionDraft, IdentifierResolutionError> {
        let execution = self.execute_encrypted(program_policy, ciphertext)?;
        validate_output_opening(&opening, &execution, program_policy)?;
        let program_id_bytes = program_id_bytes(&program_policy.program_id);
        let (opaque_id, receipt_hash) = identifier_hashes_from_output_hash(
            &program_id_bytes,
            &opening.payload.opened_output_hash,
        );
        Ok(IdentifierResolutionDraft {
            opaque_id: OpaqueAccountId::from_hash(opaque_id),
            receipt_hash,
            resolved_at_ms: execution.executed_at_ms,
            expires_at_ms: execution.expires_at_ms,
            backend: execution.backend,
            output_hash: execution.output_hash,
            input_ciphertext_hash: execution.input_ciphertext_hash,
            output_ciphertext_hash: execution.output_ciphertext_hash,
            program_digest: execution.program_digest,
            parameter_digest: execution.parameter_digest,
            evaluation_key_digest: execution.evaluation_key_digest,
            verification_mode: execution.verification_mode,
            opening,
        })
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

    /// Sign a generic RAM-LFE execution receipt.
    pub fn issue_execution_receipt(
        &self,
        program_policy: &RamLfeProgramPolicy,
        draft: &RamLfeExecutionDraft,
    ) -> Result<iroha_data_model::ram_lfe::RamLfeExecutionReceipt, IdentifierResolutionError> {
        let runtime = self.runtime(program_policy)?;
        if runtime.signer.public_key() != &program_policy.resolver_public_key {
            return Err(IdentifierResolutionError::SignerMismatch);
        }
        if draft.verification_mode != RamLfeVerificationMode::Signed {
            return Err(IdentifierResolutionError::ProofModeUnsupported);
        }

        let payload = RamLfeExecutionReceiptPayload {
            program_id: program_policy.program_id.clone(),
            program_digest: draft.program_digest,
            backend: draft.backend,
            verification_mode: draft.verification_mode,
            input_ciphertext_hash: draft.input_ciphertext_hash,
            output_ciphertext_hash: draft.output_ciphertext_hash,
            parameter_digest: draft.parameter_digest,
            evaluation_key_digest: draft.evaluation_key_digest,
            output_hash: draft.output_hash,
            associated_data_hash: draft.associated_data_hash,
            executed_at_ms: draft.executed_at_ms,
            expires_at_ms: draft.expires_at_ms,
        };
        let signature: Signature = SignatureOf::new(runtime.signer.private_key(), &payload).into();
        Ok(iroha_data_model::ram_lfe::RamLfeExecutionReceipt {
            payload,
            attestation: RamLfeReceiptAttestation::Signed(signature),
        })
    }

    /// Sign the externally verifiable opening for an executed RAM-LFE output.
    pub fn issue_output_opening(
        &self,
        program_policy: &RamLfeProgramPolicy,
        draft: &RamLfeExecutionDraft,
    ) -> Result<RamLfeOutputOpening, IdentifierResolutionError> {
        let runtime = self.runtime(program_policy)?;
        if runtime.signer.public_key() != &program_policy.output_opening_public_key {
            return Err(IdentifierResolutionError::SignerMismatch);
        }

        let payload = RamLfeOutputOpeningPayload {
            program_id: program_policy.program_id.clone(),
            input_ciphertext_hash: draft.input_ciphertext_hash,
            output_ciphertext_hash: draft.output_ciphertext_hash,
            parameter_digest: draft.parameter_digest,
            evaluation_key_digest: draft.evaluation_key_digest,
            opened_output_hash: ram_lfe_output_hash(&draft.output),
            opened_at_ms: draft.executed_at_ms,
            expires_at_ms: draft.expires_at_ms,
        };
        let signature: Signature = SignatureOf::new(runtime.signer.private_key(), &payload).into();
        Ok(RamLfeOutputOpening { payload, signature })
    }

    fn issue_receipt(
        &self,
        policy: &IdentifierPolicy,
        program_policy: &RamLfeProgramPolicy,
        draft: &IdentifierResolutionDraft,
        uaid: UniversalAccountId,
        account_id: AccountId,
    ) -> Result<IdentifierResolutionReceipt, IdentifierResolutionError> {
        let runtime = self.runtime(program_policy)?;
        if runtime.signer.public_key() != &program_policy.resolver_public_key {
            return Err(IdentifierResolutionError::SignerMismatch);
        }
        if draft.verification_mode != RamLfeVerificationMode::Signed {
            return Err(IdentifierResolutionError::ProofModeUnsupported);
        }

        let execution = RamLfeExecutionReceiptPayload {
            program_id: program_policy.program_id.clone(),
            program_digest: draft.program_digest,
            backend: draft.backend,
            verification_mode: draft.verification_mode,
            input_ciphertext_hash: draft.input_ciphertext_hash,
            output_ciphertext_hash: draft.output_ciphertext_hash,
            parameter_digest: draft.parameter_digest,
            evaluation_key_digest: draft.evaluation_key_digest,
            output_hash: draft.output_hash,
            associated_data_hash: Hash::new(program_id_bytes(&program_policy.program_id)),
            executed_at_ms: draft.resolved_at_ms,
            expires_at_ms: draft.expires_at_ms,
        };
        let payload = IdentifierResolutionReceiptPayload {
            policy_id: policy.id.clone(),
            execution,
            opening: draft.opening.clone(),
            opaque_id: draft.opaque_id,
            receipt_hash: draft.receipt_hash,
            uaid,
            account_id,
        };
        let signature: Signature = SignatureOf::new(runtime.signer.private_key(), &payload).into();

        Ok(IdentifierResolutionReceipt {
            payload,
            attestation: RamLfeReceiptAttestation::Signed(signature),
        })
    }

    fn runtime(
        &self,
        program_policy: &RamLfeProgramPolicy,
    ) -> Result<ProgramRuntime, IdentifierResolutionError> {
        self.program_runtimes
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(&program_policy.program_id)
            .cloned()
            .ok_or_else(|| {
                IdentifierResolutionError::UnknownProgram(program_policy.program_id.clone())
            })
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or(0)
}

fn validate_output_opening(
    opening: &RamLfeOutputOpening,
    execution: &RamLfeExecutionDraft,
    program_policy: &RamLfeProgramPolicy,
) -> Result<(), IdentifierResolutionError> {
    let payload = &opening.payload;
    if payload.program_id != program_policy.program_id {
        return Err(IdentifierResolutionError::InvalidOutputOpening(format!(
            "opening program {} does not match policy {}",
            payload.program_id, program_policy.program_id
        )));
    }
    if payload.input_ciphertext_hash != execution.input_ciphertext_hash {
        return Err(IdentifierResolutionError::InvalidOutputOpening(
            "input ciphertext hash mismatch".to_owned(),
        ));
    }
    if payload.output_ciphertext_hash != execution.output_ciphertext_hash {
        return Err(IdentifierResolutionError::InvalidOutputOpening(
            "output ciphertext hash mismatch".to_owned(),
        ));
    }
    if payload.parameter_digest != execution.parameter_digest {
        return Err(IdentifierResolutionError::InvalidOutputOpening(
            "parameter digest mismatch".to_owned(),
        ));
    }
    if payload.evaluation_key_digest != execution.evaluation_key_digest {
        return Err(IdentifierResolutionError::InvalidOutputOpening(
            "evaluation-key digest mismatch".to_owned(),
        ));
    }
    if payload.opened_output_hash == Hash::prehashed([0; Hash::LENGTH]) {
        return Err(IdentifierResolutionError::InvalidOutputOpening(
            "opened output hash must not be zero".to_owned(),
        ));
    }
    let now = now_ms();
    if payload.opened_at_ms > now {
        return Err(IdentifierResolutionError::InvalidOutputOpening(
            "opening timestamp is in the future".to_owned(),
        ));
    }
    if payload
        .expires_at_ms
        .is_some_and(|expires_at_ms| expires_at_ms <= payload.opened_at_ms || expires_at_ms <= now)
    {
        return Err(IdentifierResolutionError::InvalidOutputOpening(
            "opening is expired or has an invalid expiry".to_owned(),
        ));
    }
    opening
        .verify_signature(&program_policy.output_opening_public_key)
        .map_err(|err| IdentifierResolutionError::InvalidOutputOpening(err.to_string()))
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

pub(crate) fn program_id_bytes(program_id: &RamLfeProgramId) -> Vec<u8> {
    norito::to_bytes(program_id).expect("RAM-LFE program id encoding must succeed")
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use iroha_crypto::{
        Algorithm, BfvEvaluationKeyBundle, Hash, PolicyCommitment, RamLfeBackend,
        RamLfeVerificationMode, Signature, SignatureOf,
        bfv_programmed_policy_commitment_with_program,
        bfv_programmed_public_parameters_with_program, default_bfv_programmed_hidden_program,
        derive_identifier_key_material_from_seed, encrypt_identifier_from_seed,
        ram_lfe_bfv_parameters_v1, ram_lfe_output_hash,
    };
    use sha2::{Digest as _, Sha256};

    use iroha_data_model::ram_lfe::{
        RamLfeOutputOpening, RamLfeOutputOpeningPayload, RamLfeProgramId, RamLfeProgramPolicy,
        RamLfeReceiptAttestation,
    };
    use norito::codec::Encode as _;

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
        let (public_parameters, _, relinearization_key) = derive_identifier_key_material_from_seed(
            &params,
            63,
            secret,
            &program_id_bytes(&program_id),
        )
        .expect("identifier BFV parameters");
        let evaluation_keys = BfvEvaluationKeyBundle {
            relinearization_key,
            rotation_keys: Vec::new(),
            galois_keys: Vec::new(),
            bootstrap_key: None,
        };
        let programmed_public_parameters = bfv_programmed_public_parameters_with_program(
            public_parameters,
            evaluation_keys,
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

    fn sample_identifier_bfv_parameters() -> iroha_crypto::BfvParameters {
        ram_lfe_bfv_parameters_v1()
    }

    fn sample_program_id(policy_id: &IdentifierPolicyId) -> RamLfeProgramId {
        format!("{}_{}", policy_id.kind, policy_id.business_rule)
            .parse()
            .expect("program id")
    }

    fn encrypted_identifier(
        program_policy: &RamLfeProgramPolicy,
        input: &[u8],
        seed: &[u8],
    ) -> BfvIdentifierCiphertext {
        let public_parameters =
            decode_bfv_public_parameters(program_policy).expect("decode BFV params");
        encrypt_identifier_from_seed(&public_parameters, input, seed).expect("encrypt input")
    }

    fn opening_for_execution(
        program_policy: &RamLfeProgramPolicy,
        signer: &KeyPair,
        execution: &RamLfeExecutionDraft,
    ) -> RamLfeOutputOpening {
        let payload = RamLfeOutputOpeningPayload {
            program_id: program_policy.program_id.clone(),
            input_ciphertext_hash: execution.input_ciphertext_hash,
            output_ciphertext_hash: execution.output_ciphertext_hash,
            parameter_digest: execution.parameter_digest,
            evaluation_key_digest: execution.evaluation_key_digest,
            opened_output_hash: ram_lfe_output_hash(&execution.output),
            opened_at_ms: execution.executed_at_ms,
            expires_at_ms: execution.expires_at_ms,
        };
        RamLfeOutputOpening {
            signature: SignatureOf::new(signer.private_key(), &payload).into(),
            payload,
        }
    }

    fn bogus_opening(
        program_policy: &RamLfeProgramPolicy,
        signer: &KeyPair,
    ) -> RamLfeOutputOpening {
        let payload = RamLfeOutputOpeningPayload {
            program_id: program_policy.program_id.clone(),
            input_ciphertext_hash: Hash::new(b"input"),
            output_ciphertext_hash: Hash::new(b"output"),
            parameter_digest: Hash::new(b"parameters"),
            evaluation_key_digest: Hash::new(b"evaluation-keys"),
            opened_output_hash: Hash::new(b"opened-output"),
            opened_at_ms: now_ms(),
            expires_at_ms: None,
        };
        RamLfeOutputOpening {
            signature: SignatureOf::new(signer.private_key(), &payload).into(),
            payload,
        }
    }

    fn shared_identifier_receipt_fixture() -> norito::json::Value {
        let fixture_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/soracloud/identifier_receipt_vectors_v1.json");
        let fixture = std::fs::read_to_string(&fixture_path)
            .unwrap_or_else(|err| panic!("failed to read {}: {err}", fixture_path.display()));
        norito::json::from_str(&fixture)
            .unwrap_or_else(|err| panic!("failed to parse {}: {err}", fixture_path.display()))
    }

    fn fixture_get<'a>(value: &'a norito::json::Value, field: &str) -> &'a norito::json::Value {
        value
            .get(field)
            .unwrap_or_else(|| panic!("fixture field `{field}` is missing"))
    }

    fn fixture_object<'a>(value: &'a norito::json::Value, field: &str) -> &'a norito::json::Value {
        let item = fixture_get(value, field);
        item.as_object()
            .unwrap_or_else(|| panic!("fixture field `{field}` must be an object"));
        item
    }

    fn fixture_array<'a>(value: &'a norito::json::Value, field: &str) -> &'a [norito::json::Value] {
        fixture_get(value, field)
            .as_array()
            .unwrap_or_else(|| panic!("fixture field `{field}` must be an array"))
    }

    fn fixture_str<'a>(value: &'a norito::json::Value, field: &str) -> &'a str {
        fixture_get(value, field)
            .as_str()
            .unwrap_or_else(|| panic!("fixture field `{field}` must be a string"))
    }

    fn fixture_u64(value: &norito::json::Value, field: &str) -> u64 {
        fixture_get(value, field)
            .as_u64()
            .unwrap_or_else(|| panic!("fixture field `{field}` must be an unsigned integer"))
    }

    fn fixture_optional_u64(value: &norito::json::Value, field: &str) -> Option<u64> {
        fixture_get(value, field).as_u64()
    }

    fn receipt_from_fixture(receipt: &norito::json::Value) -> IdentifierResolutionReceipt {
        IdentifierResolutionReceipt {
            payload: payload_from_fixture(fixture_object(receipt, "payload")),
            attestation: attestation_from_fixture(fixture_object(receipt, "attestation")),
        }
    }

    fn payload_from_fixture(payload: &norito::json::Value) -> IdentifierResolutionReceiptPayload {
        let opening = fixture_object(payload, "opening");
        IdentifierResolutionReceiptPayload {
            policy_id: IdentifierPolicyId::from_str(fixture_str(payload, "policy_id"))
                .expect("valid policy id"),
            execution: RamLfeExecutionReceiptPayload {
                program_id: RamLfeProgramId::from_str(fixture_str(
                    fixture_object(payload, "execution"),
                    "program_id",
                ))
                .expect("valid program id"),
                program_digest: hash_hex(fixture_str(
                    fixture_object(payload, "execution"),
                    "program_digest",
                )),
                backend: ram_lfe_backend(fixture_str(
                    fixture_object(payload, "execution"),
                    "backend",
                )),
                verification_mode: verification_mode(fixture_str(
                    fixture_object(payload, "execution"),
                    "verification_mode",
                )),
                input_ciphertext_hash: hash_hex(fixture_str(
                    fixture_object(payload, "execution"),
                    "input_ciphertext_hash",
                )),
                output_ciphertext_hash: hash_hex(fixture_str(
                    fixture_object(payload, "execution"),
                    "output_ciphertext_hash",
                )),
                parameter_digest: hash_hex(fixture_str(
                    fixture_object(payload, "execution"),
                    "parameter_digest",
                )),
                evaluation_key_digest: hash_hex(fixture_str(
                    fixture_object(payload, "execution"),
                    "evaluation_key_digest",
                )),
                output_hash: hash_hex(fixture_str(
                    fixture_object(payload, "execution"),
                    "output_hash",
                )),
                associated_data_hash: hash_hex(fixture_str(
                    fixture_object(payload, "execution"),
                    "associated_data_hash",
                )),
                executed_at_ms: fixture_u64(fixture_object(payload, "execution"), "executed_at_ms"),
                expires_at_ms: fixture_optional_u64(
                    fixture_object(payload, "execution"),
                    "expires_at_ms",
                ),
            },
            opening: RamLfeOutputOpening {
                payload: opening_payload_from_fixture(fixture_object(opening, "payload")),
                signature: Signature::from_hex(fixture_str(opening, "signature"))
                    .expect("valid opening signature hex"),
            },
            opaque_id: OpaqueAccountId::from_str(fixture_str(payload, "opaque_id"))
                .expect("valid opaque id"),
            receipt_hash: hash_hex(fixture_str(payload, "receipt_hash")),
            uaid: UniversalAccountId::from_str(fixture_str(payload, "uaid")).expect("valid uaid"),
            account_id: AccountId::parse_encoded(fixture_str(payload, "account_id"))
                .expect("valid account id")
                .into_account_id(),
        }
    }

    fn opening_payload_from_fixture(payload: &norito::json::Value) -> RamLfeOutputOpeningPayload {
        RamLfeOutputOpeningPayload {
            program_id: RamLfeProgramId::from_str(fixture_str(payload, "program_id"))
                .expect("valid program id"),
            input_ciphertext_hash: hash_hex(fixture_str(payload, "input_ciphertext_hash")),
            output_ciphertext_hash: hash_hex(fixture_str(payload, "output_ciphertext_hash")),
            parameter_digest: hash_hex(fixture_str(payload, "parameter_digest")),
            evaluation_key_digest: hash_hex(fixture_str(payload, "evaluation_key_digest")),
            opened_output_hash: hash_hex(fixture_str(payload, "opened_output_hash")),
            opened_at_ms: fixture_u64(payload, "opened_at_ms"),
            expires_at_ms: fixture_optional_u64(payload, "expires_at_ms"),
        }
    }

    fn attestation_from_fixture(attestation: &norito::json::Value) -> RamLfeReceiptAttestation {
        match fixture_str(attestation, "kind") {
            "signed" => RamLfeReceiptAttestation::Signed(
                Signature::from_hex(fixture_str(attestation, "signature"))
                    .expect("valid receipt signature hex"),
            ),
            other => panic!("unsupported fixture attestation kind `{other}`"),
        }
    }

    fn shared_fixture_program_policy(
        payload: &IdentifierResolutionReceiptPayload,
        signer: &KeyPair,
    ) -> RamLfeProgramPolicy {
        RamLfeProgramPolicy::new(
            payload.execution.program_id.clone(),
            payload.account_id.clone(),
            payload.execution.backend,
            payload.execution.verification_mode,
            PolicyCommitment {
                backend: payload.execution.backend,
                policy_hash: Hash::new(b"shared-identifier-receipt-fixture-policy"),
                public_parameters: Vec::new(),
            },
            signer.public_key().clone(),
        )
    }

    fn ram_lfe_backend(raw: &str) -> RamLfeBackend {
        match raw {
            "hkdf-sha3-512-prf-v1" => RamLfeBackend::HkdfSha3_512PrfV1,
            "bfv-affine-sha3-256-v1" => RamLfeBackend::BfvAffineSha3_256V1,
            "bfv-programmed-sha3-256-v1" => RamLfeBackend::BfvProgrammedSha3_256V1,
            other => panic!("unsupported RAM-LFE backend `{other}`"),
        }
    }

    fn verification_mode(raw: &str) -> RamLfeVerificationMode {
        match raw {
            "signed" => RamLfeVerificationMode::Signed,
            "proof" => RamLfeVerificationMode::Proof,
            other => panic!("unsupported verification mode `{other}`"),
        }
    }

    fn public_key_literal(raw: &str) -> PublicKey {
        let literal = raw
            .trim()
            .strip_prefix("ed25519:")
            .unwrap_or_else(|| raw.trim());
        PublicKey::from_str(literal).expect("valid public key literal")
    }

    fn hash_hex(value: &str) -> Hash {
        Hash::from_str(value).expect("valid hash")
    }

    fn sha256_hex(bytes: &[u8]) -> String {
        hex::encode_upper(Sha256::digest(bytes))
    }

    #[test]
    fn torii_issue_claim_receipt_matches_shared_identifier_fixture() {
        let fixture = shared_identifier_receipt_fixture();
        assert_eq!(
            fixture_str(&fixture, "vector_set"),
            "identifier-receipt-attestation-v1"
        );
        let fixture_receipt = receipt_from_fixture(fixture_object(&fixture, "receipt"));
        let fixture_payload = &fixture_receipt.payload;
        let policy = IdentifierPolicy::new(
            fixture_payload.policy_id.clone(),
            fixture_payload.account_id.clone(),
            IdentifierNormalization::PhoneE164,
            fixture_payload.execution.program_id.clone(),
        );
        let signing_seed = hex::decode(fixture_str(&fixture, "signing_seed_hex"))
            .expect("fixture signing seed must be hex");
        let signer = KeyPair::from_seed(signing_seed, Algorithm::Ed25519);
        let mut program_policy = shared_fixture_program_policy(&fixture_payload, &signer);
        let service = IdentifierResolutionService::new();
        service.register_program_runtime(
            program_policy.program_id.clone(),
            b"shared-identifier-receipt-fixture".to_vec(),
            default_bfv_programmed_hidden_program(),
            signer.clone(),
            None,
        );
        let draft = IdentifierResolutionDraft {
            opaque_id: fixture_payload.opaque_id,
            receipt_hash: fixture_payload.receipt_hash,
            resolved_at_ms: fixture_payload.execution.executed_at_ms,
            expires_at_ms: fixture_payload.execution.expires_at_ms,
            backend: fixture_payload.execution.backend,
            output_hash: fixture_payload.execution.output_hash,
            input_ciphertext_hash: fixture_payload.execution.input_ciphertext_hash,
            output_ciphertext_hash: fixture_payload.execution.output_ciphertext_hash,
            program_digest: fixture_payload.execution.program_digest,
            parameter_digest: fixture_payload.execution.parameter_digest,
            evaluation_key_digest: fixture_payload.execution.evaluation_key_digest,
            verification_mode: fixture_payload.execution.verification_mode,
            opening: fixture_payload.opening.clone(),
        };

        let issued = service
            .issue_claim_receipt(
                &policy,
                &program_policy,
                &draft,
                fixture_payload.uaid,
                fixture_payload.account_id.clone(),
            )
            .expect("Torii must issue fixture receipt");
        assert_eq!(fixture_payload, &issued.payload);
        assert_eq!(
            fixture_str(&fixture, "canonical_payload_sha256"),
            sha256_hex(&issued.payload.encode())
        );
        let RamLfeReceiptAttestation::Signed(signature) = &issued.attestation else {
            panic!("issued fixture receipt must be signed");
        };
        assert_eq!(
            fixture_str(
                fixture_object(fixture_object(&fixture, "receipt"), "attestation"),
                "signature"
            ),
            hex::encode_upper(signature.payload()),
        );
        let signed_attestation_vector = fixture_array(&fixture, "attestation_vectors")
            .iter()
            .find(|vector| fixture_str(vector, "name") == "signed-resolver-attestation")
            .expect("fixture signed attestation vector");
        assert_eq!(
            fixture_str(signed_attestation_vector, "expected_attestation_sha256"),
            sha256_hex(&issued.attestation.encode()),
        );
        issued
            .verify(&program_policy.resolver_public_key)
            .expect("issued fixture signature must verify");

        let mut wrong_resolver_policy = program_policy.clone();
        wrong_resolver_policy.resolver_public_key = public_key_literal(fixture_str(
            fixture_array(&fixture, "negative_cases")
                .iter()
                .find(|case| fixture_str(case, "name") == "wrong-resolver-key")
                .expect("fixture wrong-resolver-key case"),
            "value",
        ));
        let err = service
            .issue_claim_receipt(
                &policy,
                &wrong_resolver_policy,
                &draft,
                fixture_payload.uaid,
                fixture_payload.account_id.clone(),
            )
            .expect_err("mismatched fixture resolver key must reject at Torii signing");
        assert!(matches!(err, IdentifierResolutionError::SignerMismatch));

        let mut proof_draft = draft;
        proof_draft.verification_mode = RamLfeVerificationMode::Proof;
        program_policy.verification_mode = RamLfeVerificationMode::Proof;
        let err = service
            .issue_claim_receipt(
                &policy,
                &program_policy,
                &proof_draft,
                fixture_payload.uaid,
                fixture_payload.account_id.clone(),
            )
            .expect_err("Torii cannot issue proof-mode receipts without prover support");
        assert!(matches!(
            err,
            IdentifierResolutionError::ProofModeUnsupported
        ));
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
        service.register_program_runtime(
            program_policy.program_id.clone(),
            secret,
            default_bfv_programmed_hidden_program(),
            signer.clone(),
            Some(30_000),
        );

        let ciphertext = encrypted_identifier(
            &program_policy,
            b"+15551234567",
            b"derive-and-sign-ciphertext",
        );
        let execution = service
            .execute_encrypted(&program_policy, &ciphertext)
            .expect("execute encrypted input");
        let opening = opening_for_execution(&program_policy, &signer, &execution);
        let draft = service
            .derive_encrypted(&policy, &program_policy, &ciphertext, opening)
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

        let RamLfeReceiptAttestation::Signed(signature) = &receipt.attestation else {
            panic!("receipt attestation must be signed");
        };
        SignatureOf::<IdentifierResolutionReceiptPayload>::from_signature(signature.clone())
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

        let ciphertext =
            encrypted_identifier(&program_policy, b"alice@example.com", b"missing-runtime");
        let opening = bogus_opening(&program_policy, &signer);
        let err = service
            .derive_encrypted(&policy, &program_policy, &ciphertext, opening)
            .expect_err("missing runtime must fail");
        assert!(matches!(
            err,
            IdentifierResolutionError::UnknownProgram(found)
                if found == program_policy.program_id
        ));
    }

    #[test]
    fn programmed_backend_rejects_mismatched_runtime_hidden_program() {
        let service = IdentifierResolutionService::new();
        let owner = AccountId::new(KeyPair::random().public_key().clone());
        let signer = KeyPair::random();
        let policy_id: IdentifierPolicyId = "phone#retail".parse().expect("policy id");
        let secret = b"hidden-phone-policy".to_vec();
        let (_, program_policy) = sample_policy_bundle(policy_id.clone(), owner, &signer, &secret);
        let mut mismatched_program = default_bfv_programmed_hidden_program();
        mismatched_program
            .instructions
            .pop()
            .expect("default program has instructions");
        service.register_program_runtime(
            program_policy.program_id.clone(),
            secret,
            mismatched_program,
            signer,
            Some(30_000),
        );

        let ciphertext = encrypted_identifier(
            &program_policy,
            b"+15551234567",
            b"mismatched-hidden-program-ciphertext",
        );
        let err = service
            .execute_encrypted(&program_policy, &ciphertext)
            .expect_err("runtime hidden program digest mismatch must fail");
        assert!(matches!(
            err,
            IdentifierResolutionError::Evaluation(RamLfeError::CommitmentMismatch)
        ));
    }

    #[test]
    fn derive_rejects_replayed_output_opening_for_different_ciphertext() {
        let service = IdentifierResolutionService::new();
        let owner = AccountId::new(KeyPair::random().public_key().clone());
        let signer = KeyPair::random();
        let policy_id: IdentifierPolicyId = "phone#retail".parse().expect("policy id");
        let secret = b"hidden-phone-policy".to_vec();
        let (policy, program_policy) = sample_policy_bundle(policy_id, owner, &signer, &secret);
        service.register_program_runtime(
            program_policy.program_id.clone(),
            secret,
            default_bfv_programmed_hidden_program(),
            signer.clone(),
            Some(30_000),
        );

        let first_ciphertext =
            encrypted_identifier(&program_policy, b"+15551234567", b"opening-replay-first");
        let first_execution = service
            .execute_encrypted(&program_policy, &first_ciphertext)
            .expect("execute first encrypted input");
        let replayed_opening = opening_for_execution(&program_policy, &signer, &first_execution);
        let second_ciphertext =
            encrypted_identifier(&program_policy, b"+15557654321", b"opening-replay-second");

        let err = service
            .derive_encrypted(
                &policy,
                &program_policy,
                &second_ciphertext,
                replayed_opening,
            )
            .expect_err("opening bound to one ciphertext must not verify for another");
        assert!(matches!(
            err,
            IdentifierResolutionError::InvalidOutputOpening(message)
                if message.contains("input ciphertext hash mismatch")
        ));
    }

    #[test]
    fn derive_rejects_tampered_output_opening_signature() {
        let service = IdentifierResolutionService::new();
        let owner = AccountId::new(KeyPair::random().public_key().clone());
        let signer = KeyPair::random();
        let policy_id: IdentifierPolicyId = "phone#retail".parse().expect("policy id");
        let secret = b"hidden-phone-policy".to_vec();
        let (policy, program_policy) = sample_policy_bundle(policy_id, owner, &signer, &secret);
        service.register_program_runtime(
            program_policy.program_id.clone(),
            secret,
            default_bfv_programmed_hidden_program(),
            signer.clone(),
            Some(30_000),
        );

        let ciphertext = encrypted_identifier(
            &program_policy,
            b"+15551234567",
            b"tampered-opening-signature",
        );
        let execution = service
            .execute_encrypted(&program_policy, &ciphertext)
            .expect("execute encrypted input");
        let mut opening = opening_for_execution(&program_policy, &signer, &execution);
        opening.payload.opened_output_hash = Hash::new(b"tampered-opened-output");

        let err = service
            .derive_encrypted(&policy, &program_policy, &ciphertext, opening)
            .expect_err("payload mutation must invalidate output-opening signature");
        assert!(matches!(
            err,
            IdentifierResolutionError::InvalidOutputOpening(_)
        ));
    }

    #[test]
    fn derive_rejects_signed_zero_output_opening_hash() {
        let service = IdentifierResolutionService::new();
        let owner = AccountId::new(KeyPair::random().public_key().clone());
        let signer = KeyPair::random();
        let policy_id: IdentifierPolicyId = "phone#retail".parse().expect("policy id");
        let secret = b"hidden-phone-policy".to_vec();
        let (policy, program_policy) = sample_policy_bundle(policy_id, owner, &signer, &secret);
        service.register_program_runtime(
            program_policy.program_id.clone(),
            secret,
            default_bfv_programmed_hidden_program(),
            signer.clone(),
            Some(30_000),
        );

        let ciphertext =
            encrypted_identifier(&program_policy, b"+15551234567", b"zero-opening-hash");
        let execution = service
            .execute_encrypted(&program_policy, &ciphertext)
            .expect("execute encrypted input");
        let mut opening = opening_for_execution(&program_policy, &signer, &execution);
        opening.payload.opened_output_hash = Hash::prehashed([0; Hash::LENGTH]);
        opening.signature = SignatureOf::new(signer.private_key(), &opening.payload).into();

        let err = service
            .derive_encrypted(&policy, &program_policy, &ciphertext, opening)
            .expect_err("zero opened-output hash must be rejected even when signed");
        assert!(matches!(
            err,
            IdentifierResolutionError::InvalidOutputOpening(message)
                if message.contains("opened output hash")
        ));
    }

    #[test]
    fn derive_rejects_future_output_opening_timestamp() {
        let service = IdentifierResolutionService::new();
        let owner = AccountId::new(KeyPair::random().public_key().clone());
        let signer = KeyPair::random();
        let policy_id: IdentifierPolicyId = "phone#retail".parse().expect("policy id");
        let secret = b"hidden-phone-policy".to_vec();
        let (policy, program_policy) = sample_policy_bundle(policy_id, owner, &signer, &secret);
        service.register_program_runtime(
            program_policy.program_id.clone(),
            secret,
            default_bfv_programmed_hidden_program(),
            signer.clone(),
            Some(30_000),
        );

        let ciphertext = encrypted_identifier(&program_policy, b"+15551234567", b"future-opening");
        let execution = service
            .execute_encrypted(&program_policy, &ciphertext)
            .expect("execute encrypted input");
        let mut opening = opening_for_execution(&program_policy, &signer, &execution);
        opening.payload.opened_at_ms = now_ms().saturating_add(60_000);
        opening.payload.expires_at_ms = opening.payload.opened_at_ms.checked_add(60_000);
        opening.signature = SignatureOf::new(signer.private_key(), &opening.payload).into();

        let err = service
            .derive_encrypted(&policy, &program_policy, &ciphertext, opening)
            .expect_err("future-dated output opening must be rejected");
        assert!(matches!(
            err,
            IdentifierResolutionError::InvalidOutputOpening(message)
                if message.contains("future")
        ));
    }

    #[test]
    fn derive_rejects_output_opening_signed_by_wrong_verifier_key() {
        let service = IdentifierResolutionService::new();
        let owner = AccountId::new(KeyPair::random().public_key().clone());
        let signer = KeyPair::random();
        let wrong_opening_verifier = KeyPair::random();
        let policy_id: IdentifierPolicyId = "phone#retail".parse().expect("policy id");
        let secret = b"hidden-phone-policy".to_vec();
        let (policy, mut program_policy) = sample_policy_bundle(policy_id, owner, &signer, &secret);
        program_policy.output_opening_public_key = wrong_opening_verifier.public_key().clone();
        service.register_program_runtime(
            program_policy.program_id.clone(),
            secret,
            default_bfv_programmed_hidden_program(),
            signer.clone(),
            Some(30_000),
        );

        let ciphertext = encrypted_identifier(
            &program_policy,
            b"+15551234567",
            b"wrong-opening-verifier-key",
        );
        let execution = service
            .execute_encrypted(&program_policy, &ciphertext)
            .expect("execute encrypted input");
        let opening = opening_for_execution(&program_policy, &signer, &execution);

        let err = service
            .derive_encrypted(&policy, &program_policy, &ciphertext, opening)
            .expect_err("opening signed by a non-authorized key must be rejected");
        assert!(matches!(
            err,
            IdentifierResolutionError::InvalidOutputOpening(_)
        ));
    }

    #[test]
    fn derive_rejects_expired_output_opening() {
        let service = IdentifierResolutionService::new();
        let owner = AccountId::new(KeyPair::random().public_key().clone());
        let signer = KeyPair::random();
        let policy_id: IdentifierPolicyId = "phone#retail".parse().expect("policy id");
        let secret = b"hidden-phone-policy".to_vec();
        let (policy, program_policy) = sample_policy_bundle(policy_id, owner, &signer, &secret);
        service.register_program_runtime(
            program_policy.program_id.clone(),
            secret,
            default_bfv_programmed_hidden_program(),
            signer.clone(),
            Some(30_000),
        );

        let ciphertext = encrypted_identifier(&program_policy, b"+15551234567", b"expired-opening");
        let execution = service
            .execute_encrypted(&program_policy, &ciphertext)
            .expect("execute encrypted input");
        let mut opening = opening_for_execution(&program_policy, &signer, &execution);
        opening.payload.expires_at_ms = Some(opening.payload.opened_at_ms);
        opening.signature = SignatureOf::new(signer.private_key(), &opening.payload).into();

        let err = service
            .derive_encrypted(&policy, &program_policy, &ciphertext, opening)
            .expect_err("expired output opening must be rejected");
        assert!(matches!(
            err,
            IdentifierResolutionError::InvalidOutputOpening(message)
                if message.contains("expired") || message.contains("invalid expiry")
        ));
    }

    #[test]
    fn derive_rejects_output_opening_program_id_mismatch() {
        let service = IdentifierResolutionService::new();
        let owner = AccountId::new(KeyPair::random().public_key().clone());
        let signer = KeyPair::random();
        let policy_id: IdentifierPolicyId = "phone#retail".parse().expect("policy id");
        let secret = b"hidden-phone-policy".to_vec();
        let (policy, program_policy) = sample_policy_bundle(policy_id, owner, &signer, &secret);
        service.register_program_runtime(
            program_policy.program_id.clone(),
            secret,
            default_bfv_programmed_hidden_program(),
            signer.clone(),
            Some(30_000),
        );

        let ciphertext = encrypted_identifier(
            &program_policy,
            b"+15551234567",
            b"mismatched-opening-program",
        );
        let execution = service
            .execute_encrypted(&program_policy, &ciphertext)
            .expect("execute encrypted input");
        let mut opening = opening_for_execution(&program_policy, &signer, &execution);
        opening.payload.program_id = "other_phone_program".parse().expect("program id");
        opening.signature = SignatureOf::new(signer.private_key(), &opening.payload).into();

        let err = service
            .derive_encrypted(&policy, &program_policy, &ciphertext, opening)
            .expect_err("opening for another program must be rejected");
        assert!(matches!(
            err,
            IdentifierResolutionError::InvalidOutputOpening(message)
                if message.contains("opening program")
        ));
    }

    #[test]
    fn execute_rejects_non_programmed_commitment_backend() {
        let service = IdentifierResolutionService::new();
        let owner = AccountId::new(KeyPair::random().public_key().clone());
        let signer = KeyPair::random();
        let policy_id: IdentifierPolicyId = "phone#retail".parse().expect("policy id");
        let secret = b"hidden-phone-policy".to_vec();
        let (_, mut program_policy) = sample_policy_bundle(policy_id, owner, &signer, &secret);
        service.register_program_runtime(
            program_policy.program_id.clone(),
            secret,
            default_bfv_programmed_hidden_program(),
            signer,
            Some(30_000),
        );
        let ciphertext = encrypted_identifier(
            &program_policy,
            b"+15551234567",
            b"non-programmed-backend-ciphertext",
        );
        program_policy.commitment.backend = RamLfeBackend::BfvAffineSha3_256V1;

        let err = service
            .execute_encrypted(&program_policy, &ciphertext)
            .expect_err("Torii execution must reject non-programmed commitment backends");
        assert!(matches!(
            err,
            IdentifierResolutionError::UnsupportedBackend(RamLfeBackend::BfvAffineSha3_256V1)
        ));
    }

    #[test]
    fn issue_execution_receipt_rejects_resolver_signer_mismatch() {
        let service = IdentifierResolutionService::new();
        let owner = AccountId::new(KeyPair::random().public_key().clone());
        let signer = KeyPair::random();
        let wrong_signer = KeyPair::random();
        let policy_id: IdentifierPolicyId = "phone#retail".parse().expect("policy id");
        let secret = b"hidden-phone-policy".to_vec();
        let (_, mut program_policy) = sample_policy_bundle(policy_id, owner, &signer, &secret);
        service.register_program_runtime(
            program_policy.program_id.clone(),
            secret,
            default_bfv_programmed_hidden_program(),
            signer,
            Some(30_000),
        );
        let ciphertext = encrypted_identifier(
            &program_policy,
            b"+15551234567",
            b"signer-mismatch-execution-ciphertext",
        );
        let draft = service
            .execute_encrypted(&program_policy, &ciphertext)
            .expect("execute encrypted input");
        program_policy.resolver_public_key = wrong_signer.public_key().clone();

        let err = service
            .issue_execution_receipt(&program_policy, &draft)
            .expect_err("receipt signing must fail when runtime key differs from policy key");
        assert!(matches!(err, IdentifierResolutionError::SignerMismatch));
    }

    #[test]
    fn issue_claim_receipt_rejects_proof_mode_draft_without_prover_runtime() {
        let service = IdentifierResolutionService::new();
        let owner = AccountId::new(KeyPair::random().public_key().clone());
        let signer = KeyPair::random();
        let policy_id: IdentifierPolicyId = "phone#retail".parse().expect("policy id");
        let secret = b"hidden-phone-policy".to_vec();
        let (policy, program_policy) =
            sample_policy_bundle(policy_id, owner.clone(), &signer, &secret);
        service.register_program_runtime(
            program_policy.program_id.clone(),
            secret,
            default_bfv_programmed_hidden_program(),
            signer.clone(),
            Some(30_000),
        );
        let ciphertext = encrypted_identifier(
            &program_policy,
            b"+15551234567",
            b"proof-mode-claim-receipt-ciphertext",
        );
        let execution = service
            .execute_encrypted(&program_policy, &ciphertext)
            .expect("execute encrypted input");
        let opening = opening_for_execution(&program_policy, &signer, &execution);
        let mut draft = service
            .derive_encrypted(&policy, &program_policy, &ciphertext, opening)
            .expect("derive encrypted identifier");
        draft.verification_mode = RamLfeVerificationMode::Proof;

        let err = service
            .issue_claim_receipt(
                &policy,
                &program_policy,
                &draft,
                UniversalAccountId::from_hash(Hash::new(b"uaid")),
                owner,
            )
            .expect_err("Torii must not issue signed claim receipts for proof-mode drafts");
        assert!(matches!(
            err,
            IdentifierResolutionError::ProofModeUnsupported
        ));
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
        service.register_program_runtime(
            program_policy.program_id.clone(),
            secret,
            default_bfv_programmed_hidden_program(),
            signer.clone(),
            Some(30_000),
        );

        let ciphertext = encrypted_identifier(
            &program_policy,
            b"+15551234567",
            b"deterministic-programmed-ciphertext",
        );
        let execution = service
            .execute_encrypted(&program_policy, &ciphertext)
            .expect("execute encrypted input");
        let opening = opening_for_execution(&program_policy, &signer, &execution);
        let first = service
            .derive_encrypted(&policy, &program_policy, &ciphertext, opening.clone())
            .expect("first derive");
        let second = service
            .derive_encrypted(&policy, &program_policy, &ciphertext, opening)
            .expect("second derive");
        assert_eq!(first.opaque_id, second.opaque_id);
        assert_eq!(first.receipt_hash, second.receipt_hash);
        assert_eq!(first.backend, RamLfeBackend::BfvProgrammedSha3_256V1);
    }

    #[test]
    fn programmed_backend_resolves_encrypted_input() {
        let service = IdentifierResolutionService::new();
        let owner = AccountId::new(KeyPair::random().public_key().clone());
        let signer = KeyPair::random();
        let policy_id: IdentifierPolicyId = "phone#retail".parse().expect("policy id");
        let secret = b"hidden-phone-policy".to_vec();
        let (policy, program_policy) =
            sample_policy_bundle(policy_id.clone(), owner, &signer, &secret);
        service.register_program_runtime(
            program_policy.program_id.clone(),
            secret,
            default_bfv_programmed_hidden_program(),
            signer.clone(),
            Some(30_000),
        );

        let ciphertext = encrypted_identifier(
            &program_policy,
            b"+15551234567",
            b"programmed-bfv-ciphertext",
        );
        let execution = service
            .execute_encrypted(&program_policy, &ciphertext)
            .expect("execute encrypted input");
        let opening = opening_for_execution(&program_policy, &signer, &execution);
        let encrypted = service
            .derive_encrypted(&policy, &program_policy, &ciphertext, opening)
            .expect("encrypted derive");

        assert_eq!(encrypted.backend, RamLfeBackend::BfvProgrammedSha3_256V1);
    }
}
