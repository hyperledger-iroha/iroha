//! Generic RAM-LFE program-policy instruction handlers.

use iroha_crypto::{
    Hash, RamLfeBackend, RamLfeVerificationMode, decode_bfv_programmed_public_parameters,
};
use iroha_data_model::{
    proof::VerifyingKeyBox,
    ram_lfe::{RamLfeExecutionReceipt, RamLfeExecutionReceiptPayload, RamLfeProgramPolicy},
    zk::OpenVerifyEnvelope,
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

/// Validate a stateless RAM-LFE execution receipt against the published program policy.
///
/// This mirrors the attestation checks used during identifier-claim admission,
/// but without any identifier-specific ledger binding checks.
pub fn validate_execution_receipt(
    receipt: &RamLfeExecutionReceipt,
    program_policy: &RamLfeProgramPolicy,
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
            if receipt.proof.is_some() {
                return Err(format!(
                    "RAM-LFE receipt for program {} must not include a proof in signed mode",
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
            if receipt.signature.is_some() {
                return Err(format!(
                    "RAM-LFE receipt for program {} must not include a signature in proof mode",
                    program_policy.program_id
                ));
            }
            verify_execution_proof(
                receipt.proof.as_ref().ok_or_else(|| {
                    format!(
                        "RAM-LFE receipt for program {} is missing a proof",
                        program_policy.program_id
                    )
                })?,
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
    if envelope.circuit_id != verifier.circuit_id {
        return Err(format!(
            "RAM-LFE proof circuit {} does not match verifier circuit {}",
            envelope.circuit_id, verifier.circuit_id
        ));
    }
    if Hash::prehashed(envelope.vk_hash) != verifier.verifying_key_hash {
        return Err("RAM-LFE proof verifying-key hash does not match verifier metadata".to_owned());
    }
    if Hash::new(&envelope.public_inputs) != verifier.public_inputs_schema_hash {
        return Err(
            "RAM-LFE proof public-input schema hash does not match verifier metadata".to_owned(),
        );
    }

    let verifying_key = VerifyingKeyBox::new(
        verifier.proof_backend.clone().into(),
        verifier.verifying_key_bytes.clone(),
    );
    if Hash::prehashed(crate::zk::hash_vk(&verifying_key)) != verifier.verifying_key_hash {
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
