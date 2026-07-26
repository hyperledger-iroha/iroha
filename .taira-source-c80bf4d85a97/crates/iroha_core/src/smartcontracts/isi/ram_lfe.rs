//! Generic RAM-LFE program-policy instruction handlers.

use iroha_crypto::{
    BfvIdentifierPublicParameters, Hash, RamLfeBackend, RamLfeVerificationMode,
    decode_bfv_programmed_public_parameters,
};
use iroha_data_model::{
    proof::VerifyingKeyBox,
    ram_lfe::{
        RamLfeExecutionReceipt, RamLfeExecutionReceiptPayload, RamLfeProgramPolicy,
        RamLfeReceiptAttestation,
    },
    zk::{BackendTag, OpenVerifyEnvelope},
};
use iroha_telemetry::metrics;

use super::prelude::*;

/// Execution handlers for RAM-LFE program-policy ISIs.
pub mod isi {
    use super::*;
    use crate::state::StateTransaction;

    impl Execute for iroha_data_model::isi::ram_lfe::RegisterRamLfeProgramPolicy {
        #[metrics(+"register_ram_lfe_program_policy")]
        fn execute(
            self,
            authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            let policy = self.policy;
            if authority != &policy.owner {
                return Err(Error::InvariantViolation(
                    "Only the program owner can register a RAM-LFE program policy"
                        .to_owned()
                        .into(),
                ));
            }
            if state_transaction
                .world
                .ram_lfe_program_policies
                .get(&policy.program_id)
                .is_some()
            {
                return Err(Error::InvariantViolation(
                    format!(
                        "RAM-LFE program policy {} is already registered",
                        policy.program_id
                    )
                    .into(),
                ));
            }
            if policy.backend != policy.commitment.backend {
                return Err(Error::InvariantViolation(
                    format!(
                        "RAM-LFE program policy {} backend does not match commitment backend",
                        policy.program_id
                    )
                    .into(),
                ));
            }
            validate_program_policy(&policy)?;
            state_transaction
                .world
                .ram_lfe_program_policies
                .insert(policy.program_id.clone(), policy);
            Ok(())
        }
    }

    impl Execute for iroha_data_model::isi::ram_lfe::ActivateRamLfeProgramPolicy {
        #[metrics(+"activate_ram_lfe_program_policy")]
        fn execute(
            self,
            authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            let policy = state_transaction
                .world
                .ram_lfe_program_policies
                .get_mut(&self.program_id)
                .ok_or_else(|| {
                    Error::InvariantViolation(
                        format!(
                            "RAM-LFE program policy {} is not registered",
                            self.program_id
                        )
                        .into(),
                    )
                })?;
            ensure_program_policy_owner(authority, policy)?;
            if policy.active {
                return Err(Error::InvariantViolation(
                    format!(
                        "RAM-LFE program policy {} is already active",
                        policy.program_id
                    )
                    .into(),
                ));
            }
            validate_program_policy(policy)?;
            policy.active = true;
            Ok(())
        }
    }

    impl Execute for iroha_data_model::isi::ram_lfe::DeactivateRamLfeProgramPolicy {
        #[metrics(+"deactivate_ram_lfe_program_policy")]
        fn execute(
            self,
            authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            let policy = state_transaction
                .world
                .ram_lfe_program_policies
                .get_mut(&self.program_id)
                .ok_or_else(|| {
                    Error::InvariantViolation(
                        format!(
                            "RAM-LFE program policy {} is not registered",
                            self.program_id
                        )
                        .into(),
                    )
                })?;
            ensure_program_policy_owner(authority, policy)?;
            if !policy.active {
                return Err(Error::InvariantViolation(
                    format!(
                        "RAM-LFE program policy {} is already inactive",
                        policy.program_id
                    )
                    .into(),
                ));
            }
            policy.active = false;
            Ok(())
        }
    }

    fn ensure_program_policy_owner(
        authority: &AccountId,
        policy: &RamLfeProgramPolicy,
    ) -> Result<(), Error> {
        if authority == &policy.owner {
            return Ok(());
        }
        Err(Error::InvariantViolation(
            "Only the program owner can mutate this RAM-LFE program policy"
                .to_owned()
                .into(),
        ))
    }
}

/// Validate a RAM-LFE program policy before storing or restoring it.
pub(crate) fn validate_program_policy(policy: &RamLfeProgramPolicy) -> Result<(), Error> {
    if policy.backend != policy.commitment.backend {
        return Err(Error::InvariantViolation(
            format!(
                "RAM-LFE program policy {} backend does not match commitment backend",
                policy.program_id
            )
            .into(),
        ));
    }
    match policy.backend {
        RamLfeBackend::HkdfSha3_512PrfV1 => {
            if policy.verification_mode == RamLfeVerificationMode::Proof {
                return Err(Error::InvariantViolation(
                    format!(
                        "RAM-LFE program policy {} cannot use proof verification with backend {}",
                        policy.program_id,
                        policy.backend.as_str()
                    )
                    .into(),
                ));
            }
        }
        RamLfeBackend::BfvAffineSha3_256V1 => {
            if policy.commitment.public_parameters.is_empty() {
                return Err(Error::InvariantViolation(
                    format!(
                        "RAM-LFE program policy {} is missing BFV public parameters",
                        policy.program_id
                    )
                    .into(),
                ));
            }
            let archived = norito::from_bytes::<BfvIdentifierPublicParameters>(
                &policy.commitment.public_parameters,
            )
            .map_err(|err| {
                Error::InvariantViolation(
                    format!(
                        "RAM-LFE program policy {} has invalid BFV public parameters: {err}",
                        policy.program_id
                    )
                    .into(),
                )
            })?;
            let public_parameters: BfvIdentifierPublicParameters =
                norito::core::NoritoDeserialize::deserialize(archived);
            public_parameters.validate().map_err(|err| {
                Error::InvariantViolation(
                    format!(
                        "RAM-LFE program policy {} has invalid BFV public parameters: {err}",
                        policy.program_id
                    )
                    .into(),
                )
            })?;
            if policy.verification_mode == RamLfeVerificationMode::Proof {
                return Err(Error::InvariantViolation(
                    format!(
                        "RAM-LFE program policy {} cannot use proof verification with backend {}",
                        policy.program_id,
                        policy.backend.as_str()
                    )
                    .into(),
                ));
            }
        }
        RamLfeBackend::BfvProgrammedSha3_256V1 => {
            decode_bfv_programmed_public_parameters(&policy.commitment.public_parameters).map_err(
                |err| {
                    Error::InvariantViolation(
                        format!(
                            "RAM-LFE program policy {} has invalid programmed public parameters: {err}",
                            policy.program_id
                        )
                        .into(),
                    )
                },
            )?;
        }
    }
    Ok(())
}

/// Validate a stateless RAM-LFE execution receipt against the published program policy and clock.
///
/// This mirrors the attestation checks used during identifier-claim admission,
/// but without any identifier-specific ledger binding checks.
pub fn validate_execution_receipt_at(
    receipt: &RamLfeExecutionReceipt,
    program_policy: &RamLfeProgramPolicy,
    now_ms: u64,
) -> Result<(), String> {
    let payload = &receipt.payload;
    if payload.program_id != program_policy.program_id {
        return Err(format!(
            "RAM-LFE receipt program {} does not match program policy {}",
            payload.program_id, program_policy.program_id
        ));
    }
    if payload.backend != program_policy.backend {
        return Err(format!(
            "RAM-LFE receipt backend {} does not match program policy {} backend {}",
            payload.backend.as_str(),
            program_policy.program_id,
            program_policy.backend.as_str()
        ));
    }
    if payload.verification_mode != program_policy.verification_mode {
        return Err(format!(
            "RAM-LFE receipt verification mode does not match program policy {}",
            program_policy.program_id
        ));
    }
    if program_policy.commitment.backend != program_policy.backend {
        return Err(format!(
            "RAM-LFE program policy {} backend does not match its commitment backend",
            program_policy.program_id
        ));
    }
    if let Some(expires_at_ms) = payload.expires_at_ms {
        if expires_at_ms <= payload.executed_at_ms {
            return Err(format!(
                "RAM-LFE receipt for program {} expires at or before execution time",
                program_policy.program_id
            ));
        }
        if expires_at_ms <= now_ms {
            return Err(format!(
                "RAM-LFE receipt for program {} is expired",
                program_policy.program_id
            ));
        }
    }
    if payload.executed_at_ms > now_ms {
        return Err(format!(
            "RAM-LFE receipt for program {} was executed in the future",
            program_policy.program_id
        ));
    }

    let public_parameters = match program_policy.backend {
        RamLfeBackend::BfvProgrammedSha3_256V1 => {
            decode_bfv_programmed_public_parameters(&program_policy.commitment.public_parameters)
                .map_err(|err| {
                    format!(
                        "RAM-LFE program policy {} has invalid programmed public parameters: {err}",
                        program_policy.program_id
                    )
                })?
        }
        _ => {
            return Err(format!(
                "RAM-LFE program policy {} uses unsupported backend {} for stateless receipt verification",
                program_policy.program_id,
                program_policy.backend.as_str()
            ));
        }
    };
    if public_parameters.hidden_program_digest != payload.program_digest {
        return Err(format!(
            "RAM-LFE receipt program digest does not match program policy {}",
            program_policy.program_id
        ));
    }
    if public_parameters.parameter_digest != payload.parameter_digest {
        return Err(format!(
            "RAM-LFE receipt parameter digest does not match program policy {}",
            program_policy.program_id
        ));
    }
    if public_parameters.evaluation_key_digest != payload.evaluation_key_digest {
        return Err(format!(
            "RAM-LFE receipt evaluation-key digest does not match program policy {}",
            program_policy.program_id
        ));
    }
    if payload.output_hash != payload.output_ciphertext_hash {
        return Err(format!(
            "RAM-LFE receipt output_hash does not match output_ciphertext_hash for program {}",
            program_policy.program_id
        ));
    }
    if public_parameters.verification_mode != program_policy.verification_mode {
        return Err(format!(
            "RAM-LFE program policy {} verification metadata is inconsistent",
            program_policy.program_id
        ));
    }
    if payload.associated_data_hash != expected_associated_data_hash(program_policy)? {
        return Err(format!(
            "RAM-LFE receipt associated_data_hash does not match program policy {}",
            program_policy.program_id
        ));
    }

    match program_policy.verification_mode {
        RamLfeVerificationMode::Signed => {
            if !matches!(&receipt.attestation, RamLfeReceiptAttestation::Signed(_)) {
                return Err(format!(
                    "RAM-LFE receipt for program {} must carry a signed attestation",
                    program_policy.program_id
                ));
            }
            receipt
                .verify_signature(&program_policy.resolver_public_key)
                .map_err(|err| {
                    format!(
                        "RAM-LFE receipt signature is invalid for program {}: {err}",
                        program_policy.program_id
                    )
                })?;
        }
        RamLfeVerificationMode::Proof => {
            let RamLfeReceiptAttestation::Proof(proof) = &receipt.attestation else {
                return Err(format!(
                    "RAM-LFE receipt for program {} must carry a proof attestation",
                    program_policy.program_id
                ));
            };
            verify_execution_proof(
                proof,
                payload,
                public_parameters.proof_verifier.as_ref().ok_or_else(|| {
                    format!(
                        "RAM-LFE program policy {} is missing proof verifier metadata",
                        program_policy.program_id
                    )
                })?,
            )?;
        }
    }

    Ok(())
}

fn expected_associated_data_hash(program_policy: &RamLfeProgramPolicy) -> Result<Hash, String> {
    norito::to_bytes(&program_policy.program_id)
        .map(Hash::new)
        .map_err(|err| {
            format!(
                "Failed to encode RAM-LFE program id {}: {err}",
                program_policy.program_id
            )
        })
}

fn verify_execution_proof(
    proof: &iroha_data_model::proof::ProofBox,
    execution: &RamLfeExecutionReceiptPayload,
    verifier: &iroha_crypto::RamLfeProofVerifierMetadata,
) -> Result<(), String> {
    let envelope: OpenVerifyEnvelope = norito::decode_from_bytes(&proof.bytes)
        .map_err(|_| "RAM-LFE proof receipt must use an OpenVerifyEnvelope payload".to_owned())?;
    if proof.backend.as_str() != verifier.proof_backend {
        return Err(format!(
            "RAM-LFE proof backend {} does not match verifier backend {}",
            proof.backend.as_str(),
            verifier.proof_backend
        ));
    }
    if envelope.backend != BackendTag::Halo2IpaPasta {
        return Err("RAM-LFE proof envelope backend tag must be Halo2 IPA Pasta".to_owned());
    }
    if envelope.circuit_id != verifier.circuit_id {
        return Err(format!(
            "RAM-LFE proof circuit {} does not match verifier circuit {}",
            envelope.circuit_id, verifier.circuit_id
        ));
    }
    if Hash::new(&envelope.public_inputs) != verifier.public_inputs_schema_hash {
        return Err(
            "RAM-LFE proof public-input schema hash does not match verifier metadata".to_owned(),
        );
    }
    if !envelope.aux.is_empty() {
        return Err("RAM-LFE proof envelope auxiliary bytes must be empty".to_owned());
    }

    let verifying_key = VerifyingKeyBox::new(
        verifier.proof_backend.clone().into(),
        verifier.verifying_key_bytes.clone(),
    );
    if envelope.vk_hash == [0u8; Hash::LENGTH] {
        return Err("RAM-LFE proof envelope verifier-key hash must be non-zero".to_owned());
    }
    if envelope.vk_hash != crate::zk::hash_vk(&verifying_key) {
        return Err("RAM-LFE verifier metadata contains a mismatched verifying key".to_owned());
    }
    let expected_instances = expected_execution_payload_hash_instances(
        execution
            .payload_hash()
            .map_err(|err| format!("Failed to encode RAM-LFE execution receipt payload: {err}"))?,
    );
    let actual_instances = crate::zk::extract_pasta_instance_columns_bytes(&envelope.proof_bytes)
        .ok_or_else(|| {
        "RAM-LFE proof does not expose the expected Halo2 public instances".to_owned()
    })?;
    if actual_instances != expected_instances {
        return Err(
            "RAM-LFE proof public instances do not match the execution payload hash".to_owned(),
        );
    }
    if !crate::zk::verify_backend(&verifier.proof_backend, proof, Some(&verifying_key)) {
        return Err("RAM-LFE proof verification failed".to_owned());
    }
    Ok(())
}

fn expected_execution_payload_hash_instances(payload_hash: Hash) -> Vec<Vec<[u8; 32]>> {
    let bytes: &[u8; 32] = payload_hash.as_ref();
    (0..4)
        .map(|index| {
            let mut scalar = [0u8; 32];
            let start = index * 8;
            let end = start + 8;
            scalar[..8].copy_from_slice(&bytes[start..end]);
            vec![scalar]
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::str::FromStr as _;

    use iroha_crypto::{Algorithm, KeyPair, PolicyCommitment, RamLfeProofVerifierMetadata};
    use iroha_data_model::{
        account::AccountId,
        proof::ProofBox,
        ram_lfe::{RamLfeProgramId, RamLfeReceiptAttestation},
        zk::OpenVerifyEnvelope,
    };

    use super::*;

    fn checked_keypair() -> KeyPair {
        KeyPair::try_random().expect("RAM-LFE fixture key generation should succeed")
    }

    fn checked_account_id() -> AccountId {
        AccountId::new(checked_keypair().public_key().clone())
    }

    #[test]
    fn checked_keypair_preserves_default_algorithm() {
        assert_eq!(checked_keypair().algorithm(), Algorithm::default());
    }

    fn sample_policy() -> RamLfeProgramPolicy {
        let owner = checked_account_id();
        let resolver = checked_keypair();
        RamLfeProgramPolicy::new(
            RamLfeProgramId::from_str("test_program").expect("program id"),
            owner,
            RamLfeBackend::BfvProgrammedSha3_256V1,
            RamLfeVerificationMode::Signed,
            PolicyCommitment {
                backend: RamLfeBackend::BfvProgrammedSha3_256V1,
                policy_hash: Hash::new(b"policy"),
                public_parameters: Vec::new(),
            },
            resolver.public_key().clone(),
        )
    }

    fn sample_receipt(
        policy: &RamLfeProgramPolicy,
        executed_at_ms: u64,
        expires_at_ms: Option<u64>,
    ) -> RamLfeExecutionReceipt {
        RamLfeExecutionReceipt {
            payload: RamLfeExecutionReceiptPayload {
                program_id: policy.program_id.clone(),
                program_digest: Hash::new(b"program"),
                backend: policy.backend,
                verification_mode: policy.verification_mode,
                input_ciphertext_hash: Hash::new(b"input-ciphertext"),
                output_ciphertext_hash: Hash::new(b"output-ciphertext"),
                parameter_digest: Hash::new(b"parameters"),
                evaluation_key_digest: Hash::new(b"evaluation-keys"),
                output_hash: Hash::new(b"output"),
                associated_data_hash: Hash::new(b"associated-data"),
                executed_at_ms,
                expires_at_ms,
            },
            attestation: RamLfeReceiptAttestation::Proof(ProofBox::new(
                "unsupported".into(),
                vec![0xAA],
            )),
        }
    }

    #[test]
    fn validate_execution_receipt_rejects_expired_receipts() {
        let policy = sample_policy();
        let receipt = sample_receipt(&policy, 100, Some(200));
        let err = validate_execution_receipt_at(&receipt, &policy, 200).expect_err("expired");
        assert!(err.contains("is expired"));
    }

    #[test]
    fn validate_execution_receipt_rejects_malformed_expiry() {
        let policy = sample_policy();
        let receipt = sample_receipt(&policy, 100, Some(100));
        let err = validate_execution_receipt_at(&receipt, &policy, 150).expect_err("bad expiry");
        assert!(err.contains("expires at or before execution time"));
    }

    #[test]
    fn validate_execution_receipt_rejects_future_execution_time() {
        let policy = sample_policy();
        let receipt = sample_receipt(&policy, 200, None);
        let err = validate_execution_receipt_at(&receipt, &policy, 100).expect_err("future");
        assert!(err.contains("executed in the future"));
    }

    fn sample_proof_payload() -> RamLfeExecutionReceiptPayload {
        RamLfeExecutionReceiptPayload {
            program_id: RamLfeProgramId::from_str("proof_program").expect("program id"),
            program_digest: Hash::new(b"program"),
            backend: RamLfeBackend::BfvProgrammedSha3_256V1,
            verification_mode: RamLfeVerificationMode::Proof,
            input_ciphertext_hash: Hash::new(b"input-ciphertext"),
            output_ciphertext_hash: Hash::new(b"output-ciphertext"),
            parameter_digest: Hash::new(b"parameters"),
            evaluation_key_digest: Hash::new(b"evaluation-keys"),
            output_hash: Hash::new(b"output"),
            associated_data_hash: Hash::new(b"associated-data"),
            executed_at_ms: 100,
            expires_at_ms: None,
        }
    }

    fn sample_proof_verifier() -> RamLfeProofVerifierMetadata {
        RamLfeProofVerifierMetadata {
            proof_backend: crate::zk::ZK_BACKEND_HALO2_IPA.to_owned(),
            circuit_id: "halo2/pasta/ipa/tiny-add".to_owned(),
            public_inputs_schema_hash: Hash::new(b"ram-lfe-proof-schema"),
            verifying_key_bytes: b"ram-lfe-proof-vk".to_vec(),
        }
    }

    fn sample_proof_box(
        verifier: &RamLfeProofVerifierMetadata,
        mutate: impl FnOnce(&mut OpenVerifyEnvelope),
    ) -> ProofBox {
        let vk = VerifyingKeyBox::new(
            verifier.proof_backend.clone().into(),
            verifier.verifying_key_bytes.clone(),
        );
        let mut envelope = OpenVerifyEnvelope {
            backend: BackendTag::Halo2IpaPasta,
            circuit_id: verifier.circuit_id.clone(),
            vk_hash: crate::zk::hash_vk(&vk),
            public_inputs: b"ram-lfe-proof-schema".to_vec(),
            proof_bytes: vec![0xCA, 0xFE],
            aux: Vec::new(),
        };
        mutate(&mut envelope);
        ProofBox::new(
            verifier.proof_backend.clone().into(),
            norito::to_bytes(&envelope).expect("encode OpenVerifyEnvelope"),
        )
    }

    #[test]
    fn verify_execution_proof_rejects_noncanonical_envelope_metadata_before_proof_decode() {
        let verifier = sample_proof_verifier();
        let execution = sample_proof_payload();

        let bad_backend = sample_proof_box(&verifier, |envelope| {
            envelope.backend = BackendTag::Stark;
        });
        let err = verify_execution_proof(&bad_backend, &execution, &verifier)
            .expect_err("wrong envelope backend tag must reject before proof parsing");
        assert!(err.contains("backend tag"), "unexpected error: {err}");

        let aux = sample_proof_box(&verifier, |envelope| {
            envelope.aux = b"unbound-ram-lfe-proof-metadata".to_vec();
        });
        let err = verify_execution_proof(&aux, &execution, &verifier)
            .expect_err("non-empty auxiliary bytes must reject before proof parsing");
        assert!(err.contains("auxiliary bytes"), "unexpected error: {err}");

        let zero_vk_hash = sample_proof_box(&verifier, |envelope| {
            envelope.vk_hash = [0u8; Hash::LENGTH];
        });
        let err = verify_execution_proof(&zero_vk_hash, &execution, &verifier)
            .expect_err("zero verifier-key hash must reject before proof parsing");
        assert!(err.contains("non-zero"), "unexpected error: {err}");

        let schema_drift = sample_proof_box(&verifier, |envelope| {
            envelope.public_inputs.extend_from_slice(b":schema-drift");
        });
        let err = verify_execution_proof(&schema_drift, &execution, &verifier)
            .expect_err("public-input schema drift must reject before proof parsing");
        assert!(
            err.contains("public-input schema hash"),
            "unexpected error: {err}"
        );

        let wrong_vk_hash = sample_proof_box(&verifier, |envelope| {
            envelope.vk_hash = [0xA5; Hash::LENGTH];
        });
        let err = verify_execution_proof(&wrong_vk_hash, &execution, &verifier)
            .expect_err("wrong verifier-key hash must reject before proof parsing");
        assert!(
            err.contains("mismatched verifying key"),
            "unexpected error: {err}"
        );
    }
}
