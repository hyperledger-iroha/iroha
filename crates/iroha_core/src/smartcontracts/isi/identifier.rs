//! Hidden-function-backed identifier policy instruction handlers.

use iroha_crypto::{
    Hash, RamLfeBackend, RamLfeVerificationMode, decode_bfv_programmed_public_parameters,
    identifier_hashes_from_output_hash,
};
use iroha_data_model::{
    identifier::{IdentifierClaimRecord, IdentifierPolicy, IdentifierResolutionReceipt},
    prelude::*,
    proof::VerifyingKeyBox,
    ram_lfe::{RamLfeExecutionReceiptPayload, RamLfeProgramPolicy},
    zk::OpenVerifyEnvelope,
};
use iroha_telemetry::metrics;

use super::prelude::*;

/// Execution handlers for identifier-policy ISIs.
pub mod isi {
    use super::*;
    use crate::state::StateTransaction;

    impl Execute for iroha_data_model::isi::identifier::RegisterIdentifierPolicy {
        #[metrics(+"register_identifier_policy")]
        fn execute(
            self,
            authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            let policy = self.policy;
            if authority != &policy.owner {
                return Err(Error::InvariantViolation(
                    "Only the policy owner can register an identifier policy"
                        .to_owned()
                        .into(),
                ));
            }
            if state_transaction
                .world
                .identifier_policies
                .get(&policy.id)
                .is_some()
            {
                return Err(Error::InvariantViolation(
                    format!("Identifier policy {} is already registered", policy.id).into(),
                ));
            }
            state_transaction
                .world
                .identifier_policies
                .insert(policy.id.clone(), policy);
            Ok(())
        }
    }

    impl Execute for iroha_data_model::isi::identifier::ActivateIdentifierPolicy {
        #[metrics(+"activate_identifier_policy")]
        fn execute(
            self,
            authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            let policy = state_transaction
                .world
                .identifier_policies
                .get_mut(&self.policy_id)
                .ok_or_else(|| {
                    Error::InvariantViolation(
                        format!("Identifier policy {} is not registered", self.policy_id).into(),
                    )
                })?;
            if authority != &policy.owner {
                return Err(Error::InvariantViolation(
                    "Only the policy owner can activate an identifier policy"
                        .to_owned()
                        .into(),
                ));
            }
            if policy.active {
                return Err(Error::InvariantViolation(
                    format!("Identifier policy {} is already active", policy.id).into(),
                ));
            }
            policy.active = true;
            Ok(())
        }
    }

    impl Execute for iroha_data_model::isi::identifier::ClaimIdentifier {
        #[metrics(+"claim_identifier")]
        fn execute(
            self,
            authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            let receipt = self.receipt;
            let receipt_payload = receipt.payload.clone();
            let policy = state_transaction
                .world
                .identifier_policies
                .get(&receipt_payload.policy_id)
                .cloned()
                .ok_or_else(|| {
                    Error::InvariantViolation(
                        format!(
                            "Identifier policy {} is not registered",
                            receipt_payload.policy_id
                        )
                        .into(),
                    )
                })?;
            let program_policy = state_transaction
                .world
                .ram_lfe_program_policies
                .get(&policy.program_id)
                .cloned()
                .ok_or_else(|| {
                    Error::InvariantViolation(
                        format!(
                            "RAM-LFE program policy {} referenced by identifier policy {} is not registered",
                            policy.program_id, policy.id
                        )
                        .into(),
                    )
                })?;
            ensure_policy_authorized(authority, &policy, Some(&self.account))?;
            if !policy.active {
                return Err(Error::InvariantViolation(
                    format!("Identifier policy {} is not active", policy.id).into(),
                ));
            }
            if !program_policy.active {
                return Err(Error::InvariantViolation(
                    format!(
                        "RAM-LFE program policy {} referenced by identifier policy {} is not active",
                        program_policy.program_id, policy.id
                    )
                    .into(),
                ));
            }
            if let Some(expires_at_ms) = receipt.expires_at_ms()
                && expires_at_ms <= receipt.resolved_at_ms()
            {
                return Err(Error::InvariantViolation(
                    "identifier receipt expiry must be greater than resolved_at_ms"
                        .to_owned()
                        .into(),
                ));
            }
            if receipt_payload.receipt_hash == Hash::prehashed([0; Hash::LENGTH]) {
                return Err(Error::InvariantViolation(
                    "Identifier receipt hash must not be zero".to_owned().into(),
                ));
            }
            validate_program_receipt(&receipt, &policy, &program_policy)?;

            let uaid = *state_transaction
                .world
                .account(&self.account)
                .map_err(Error::from)?
                .uaid()
                .ok_or_else(|| {
                    Error::InvariantViolation(
                        format!("Account {} does not have a UAID", self.account).into(),
                    )
                })?;
            if receipt_payload.account_id != self.account {
                return Err(Error::InvariantViolation(
                    format!(
                        "Identifier receipt account {} does not match claim account {}",
                        receipt_payload.account_id, self.account
                    )
                    .into(),
                ));
            }
            if receipt_payload.uaid != uaid {
                return Err(Error::InvariantViolation(
                    format!(
                        "Identifier receipt UAID {} does not match account {} UAID {uaid}",
                        receipt_payload.uaid, self.account
                    )
                    .into(),
                ));
            }
            let now_ms = state_transaction.block_unix_timestamp_ms();
            if receipt.resolved_at_ms() > now_ms {
                return Err(Error::InvariantViolation(
                    format!(
                        "Identifier receipt for policy {} was issued in the future ({}) relative to block time ({now_ms})",
                        policy.id,
                        receipt.resolved_at_ms()
                    )
                    .into(),
                ));
            }
            if receipt
                .expires_at_ms()
                .is_some_and(|expires_at_ms| expires_at_ms <= now_ms)
            {
                return Err(Error::InvariantViolation(
                    format!(
                        "Identifier receipt for policy {} expired at or before block time {now_ms}",
                        policy.id
                    )
                    .into(),
                ));
            }
            evict_expired_identifier_binding(
                state_transaction,
                &receipt_payload.opaque_id,
                now_ms,
            )?;
            if let Some(existing_uaid) = state_transaction
                .world
                .opaque_uaids
                .get(&receipt_payload.opaque_id)
            {
                if existing_uaid != &uaid {
                    return Err(Error::InvariantViolation(
                        format!(
                            "Opaque identifier {} is already bound to UAID {existing_uaid}",
                            receipt_payload.opaque_id
                        )
                        .into(),
                    ));
                }
            }

            if let Some(existing_claim) = state_transaction
                .world
                .identifier_claims
                .get(&receipt_payload.opaque_id)
            {
                if existing_claim.policy_id != receipt_payload.policy_id
                    || existing_claim.uaid != uaid
                    || existing_claim.account_id != self.account
                {
                    return Err(Error::InvariantViolation(
                        format!(
                            "Opaque identifier {} is already claimed under a different binding",
                            receipt_payload.opaque_id
                        )
                        .into(),
                    ));
                }
            }

            let details = state_transaction
                .world
                .account_mut(&self.account)
                .map_err(Error::from)?;
            let mut opaque_ids = details.opaque_ids().to_vec();
            if !opaque_ids.contains(&receipt_payload.opaque_id) {
                opaque_ids.push(receipt_payload.opaque_id);
                details.set_opaque_ids(opaque_ids);
            }

            state_transaction
                .world
                .opaque_uaids
                .insert(receipt_payload.opaque_id, uaid);
            state_transaction.world.identifier_claims.insert(
                receipt_payload.opaque_id,
                IdentifierClaimRecord {
                    policy_id: receipt_payload.policy_id,
                    opaque_id: receipt_payload.opaque_id,
                    receipt_hash: receipt_payload.receipt_hash,
                    uaid,
                    account_id: self.account,
                    verified_at_ms: receipt.resolved_at_ms(),
                    expires_at_ms: receipt.expires_at_ms(),
                },
            );
            Ok(())
        }
    }

    impl Execute for iroha_data_model::isi::identifier::RevokeIdentifier {
        #[metrics(+"revoke_identifier")]
        fn execute(
            self,
            authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            let claim = state_transaction
                .world
                .identifier_claims
                .get(&self.opaque_id)
                .cloned()
                .ok_or_else(|| {
                    Error::InvariantViolation(
                        format!("Identifier claim {} is not registered", self.opaque_id).into(),
                    )
                })?;
            if claim.policy_id != self.policy_id {
                return Err(Error::InvariantViolation(
                    format!(
                        "Opaque identifier {} is not registered under policy {}",
                        self.opaque_id, self.policy_id
                    )
                    .into(),
                ));
            }

            let policy = state_transaction
                .world
                .identifier_policies
                .get(&self.policy_id)
                .cloned();
            if let Some(policy) = &policy {
                ensure_policy_authorized(authority, policy, Some(&claim.account_id))?;
            } else if authority != &claim.account_id {
                return Err(Error::InvariantViolation(
                    "Only the claimed account can revoke an identifier when the policy is missing"
                        .to_owned()
                        .into(),
                ));
            }

            let details = state_transaction
                .world
                .account_mut(&claim.account_id)
                .map_err(Error::from)?;
            let retained: Vec<_> = details
                .opaque_ids()
                .iter()
                .copied()
                .filter(|opaque| opaque != &self.opaque_id)
                .collect();
            details.set_opaque_ids(retained);

            state_transaction.world.opaque_uaids.remove(self.opaque_id);
            state_transaction
                .world
                .identifier_claims
                .remove(self.opaque_id);
            Ok(())
        }
    }

    fn ensure_policy_authorized(
        authority: &AccountId,
        policy: &IdentifierPolicy,
        account: Option<&AccountId>,
    ) -> Result<(), Error> {
        if authority == &policy.owner || account.is_some_and(|account| authority == account) {
            return Ok(());
        }
        Err(Error::InvariantViolation(
            "Authority is not allowed to mutate this identifier binding"
                .to_owned()
                .into(),
        ))
    }

    fn evict_expired_identifier_binding(
        state_transaction: &mut StateTransaction<'_, '_>,
        opaque_id: &OpaqueAccountId,
        now_ms: u64,
    ) -> Result<(), Error> {
        let Some(existing_claim) = state_transaction
            .world
            .identifier_claims
            .get(opaque_id)
            .cloned()
        else {
            return Ok(());
        };
        if !existing_claim
            .expires_at_ms
            .is_some_and(|expires_at_ms| expires_at_ms <= now_ms)
        {
            return Ok(());
        }

        let retained: Vec<_> = state_transaction
            .world
            .account(&existing_claim.account_id)
            .map_err(Error::from)?
            .opaque_ids()
            .iter()
            .copied()
            .filter(|existing| existing != opaque_id)
            .collect();
        state_transaction
            .world
            .account_mut(&existing_claim.account_id)
            .map_err(Error::from)?
            .set_opaque_ids(retained);

        state_transaction
            .world
            .opaque_uaids
            .remove(opaque_id.clone());
        state_transaction
            .world
            .identifier_claims
            .remove(opaque_id.clone());
        Ok(())
    }

    fn validate_program_receipt(
        receipt: &IdentifierResolutionReceipt,
        policy: &IdentifierPolicy,
        program_policy: &RamLfeProgramPolicy,
    ) -> Result<(), Error> {
        let execution = &receipt.payload.execution;
        if execution.program_id != policy.program_id
            || execution.program_id != program_policy.program_id
        {
            return Err(Error::InvariantViolation(
                format!(
                    "Identifier receipt program {} does not match identifier policy {} program {}",
                    execution.program_id, policy.id, policy.program_id
                )
                .into(),
            ));
        }
        if execution.backend != program_policy.backend {
            return Err(Error::InvariantViolation(
                format!(
                    "Identifier receipt backend {} does not match program policy {} backend {}",
                    execution.backend.as_str(),
                    program_policy.program_id,
                    program_policy.backend.as_str()
                )
                .into(),
            ));
        }
        if execution.verification_mode != program_policy.verification_mode {
            return Err(Error::InvariantViolation(
                format!(
                    "Identifier receipt verification mode does not match program policy {}",
                    program_policy.program_id
                )
                .into(),
            ));
        }
        if program_policy.commitment.backend != program_policy.backend {
            return Err(Error::InvariantViolation(
                format!(
                    "RAM-LFE program policy {} backend does not match its commitment backend",
                    program_policy.program_id
                )
                .into(),
            ));
        }

        let public_parameters = match program_policy.backend {
            RamLfeBackend::BfvProgrammedSha3_256V1 => decode_bfv_programmed_public_parameters(
                &program_policy.commitment.public_parameters,
            )
            .map_err(|err| {
                Error::InvariantViolation(
                    format!(
                        "RAM-LFE program policy {} has invalid programmed public parameters: {err}",
                        program_policy.program_id
                    )
                    .into(),
                )
            })?,
            _ => {
                return Err(Error::InvariantViolation(
                    format!(
                        "RAM-LFE program policy {} uses unsupported backend {} for identifier claims",
                        program_policy.program_id,
                        program_policy.backend.as_str()
                    )
                    .into(),
                ));
            }
        };
        if public_parameters.hidden_program_digest != execution.program_digest {
            return Err(Error::InvariantViolation(
                format!(
                    "Identifier receipt program digest does not match program policy {}",
                    program_policy.program_id
                )
                .into(),
            ));
        }
        if public_parameters.verification_mode != program_policy.verification_mode {
            return Err(Error::InvariantViolation(
                format!(
                    "RAM-LFE program policy {} verification metadata is inconsistent",
                    program_policy.program_id
                )
                .into(),
            ));
        }

        let expected_hashes = expected_identifier_hashes(policy, execution)?;
        if receipt.payload.opaque_id != OpaqueAccountId::from(expected_hashes.0) {
            return Err(Error::InvariantViolation(
                format!(
                    "Identifier receipt opaque_id does not match program output hash for policy {}",
                    policy.id
                )
                .into(),
            ));
        }
        if receipt.payload.receipt_hash != expected_hashes.1 {
            return Err(Error::InvariantViolation(
                format!(
                    "Identifier receipt hash does not match program output hash for policy {}",
                    policy.id
                )
                .into(),
            ));
        }

        match program_policy.verification_mode {
            RamLfeVerificationMode::Signed => {
                if receipt.proof.is_some() {
                    return Err(Error::InvariantViolation(
                        format!(
                            "Identifier receipt for policy {} must not include a proof in signed mode",
                            policy.id
                        )
                        .into(),
                    ));
                }
                receipt
                    .verify(&program_policy.resolver_public_key)
                    .map_err(|err| {
                        Error::InvariantViolation(
                            format!(
                                "Identifier receipt signature is invalid for policy {}: {err}",
                                policy.id
                            )
                            .into(),
                        )
                    })?;
            }
            RamLfeVerificationMode::Proof => {
                if receipt.signature.is_some() {
                    return Err(Error::InvariantViolation(
                        format!(
                            "Identifier receipt for policy {} must not include a signature in proof mode",
                            policy.id
                        )
                        .into(),
                    ));
                }
                verify_execution_proof(
                    receipt.proof.as_ref().ok_or_else(|| {
                        Error::InvariantViolation(
                            format!(
                                "Identifier receipt for policy {} is missing a proof",
                                policy.id
                            )
                            .into(),
                        )
                    })?,
                    execution,
                    public_parameters.proof_verifier.as_ref().ok_or_else(|| {
                        Error::InvariantViolation(
                            format!(
                                "RAM-LFE program policy {} is missing proof verifier metadata",
                                program_policy.program_id
                            )
                            .into(),
                        )
                    })?,
                )?;
            }
        }

        Ok(())
    }

    fn expected_identifier_hashes(
        policy: &IdentifierPolicy,
        execution: &RamLfeExecutionReceiptPayload,
    ) -> Result<(Hash, Hash), Error> {
        let program_id_bytes = norito::to_bytes(&policy.program_id).map_err(|err| {
            Error::InvariantViolation(
                format!(
                    "Failed to encode RAM-LFE program id {} for identifier policy {}: {err}",
                    policy.program_id, policy.id
                )
                .into(),
            )
        })?;
        Ok(identifier_hashes_from_output_hash(
            &program_id_bytes,
            &execution.output_hash,
        ))
    }

    fn verify_execution_proof(
        proof: &iroha_data_model::proof::ProofBox,
        execution: &RamLfeExecutionReceiptPayload,
        verifier: &iroha_crypto::RamLfeProofVerifierMetadata,
    ) -> Result<(), Error> {
        let envelope: OpenVerifyEnvelope =
            norito::decode_from_bytes(&proof.bytes).map_err(|_| {
                Error::InvariantViolation(
                    "RAM-LFE proof receipt must use an OpenVerifyEnvelope payload"
                        .to_owned()
                        .into(),
                )
            })?;
        if proof.backend.as_str() != verifier.proof_backend {
            return Err(Error::InvariantViolation(
                format!(
                    "RAM-LFE proof backend {} does not match verifier backend {}",
                    proof.backend.as_str(),
                    verifier.proof_backend
                )
                .into(),
            ));
        }
        if envelope.circuit_id != verifier.circuit_id {
            return Err(Error::InvariantViolation(
                format!(
                    "RAM-LFE proof circuit {} does not match verifier circuit {}",
                    envelope.circuit_id, verifier.circuit_id
                )
                .into(),
            ));
        }
        if Hash::prehashed(envelope.vk_hash) != verifier.verifying_key_hash {
            return Err(Error::InvariantViolation(
                "RAM-LFE proof verifying-key hash does not match verifier metadata"
                    .to_owned()
                    .into(),
            ));
        }
        if Hash::new(&envelope.public_inputs) != verifier.public_inputs_schema_hash {
            return Err(Error::InvariantViolation(
                "RAM-LFE proof public-input schema hash does not match verifier metadata"
                    .to_owned()
                    .into(),
            ));
        }

        let verifying_key = VerifyingKeyBox::new(
            verifier.proof_backend.clone().into(),
            verifier.verifying_key_bytes.clone(),
        );
        if Hash::prehashed(crate::zk::hash_vk(&verifying_key)) != verifier.verifying_key_hash {
            return Err(Error::InvariantViolation(
                "RAM-LFE verifier metadata contains a mismatched verifying key"
                    .to_owned()
                    .into(),
            ));
        }
        let expected_instances =
            expected_execution_payload_hash_instances(execution.payload_hash().map_err(|err| {
                Error::InvariantViolation(
                    format!("Failed to encode RAM-LFE execution receipt payload: {err}").into(),
                )
            })?);
        let actual_instances = crate::zk::extract_pasta_instance_columns_bytes(
            &envelope.proof_bytes,
        )
        .ok_or_else(|| {
            Error::InvariantViolation(
                "RAM-LFE proof does not expose the expected Halo2 public instances"
                    .to_owned()
                    .into(),
            )
        })?;
        if actual_instances != expected_instances {
            return Err(Error::InvariantViolation(
                "RAM-LFE proof public instances do not match the execution payload hash"
                    .to_owned()
                    .into(),
            ));
        }
        if !crate::zk::verify_backend(&verifier.proof_backend, proof, Some(&verifying_key)) {
            return Err(Error::InvariantViolation(
                "RAM-LFE proof verification failed".to_owned().into(),
            ));
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
}

#[cfg(test)]
mod tests {
    use iroha_crypto::{
        BfvParameters, Hash, KeyPair, RamLfeBackend, RamLfeVerificationMode, Signature,
        SignatureOf, bfv_programmed_policy_commitment_with_program,
        bfv_programmed_public_parameters_with_program, decode_bfv_programmed_public_parameters,
        default_bfv_programmed_hidden_program, derive_identifier_key_material_from_seed,
        identifier_hashes_from_output_hash, ram_lfe_output_hash,
    };
    use iroha_data_model::{
        IntoKeyValue,
        account::{Account, AccountId, OpaqueAccountId},
        block::BlockHeader,
        domain::DomainId,
        identifier::{
            IdentifierNormalization, IdentifierPolicy, IdentifierPolicyId,
            IdentifierResolutionReceipt, IdentifierResolutionReceiptPayload,
        },
        isi::identifier::{
            ActivateIdentifierPolicy, ClaimIdentifier, RegisterIdentifierPolicy, RevokeIdentifier,
        },
        isi::ram_lfe::{ActivateRamLfeProgramPolicy, RegisterRamLfeProgramPolicy},
        metadata::Metadata,
        nexus::UniversalAccountId,
        prelude::Domain,
        ram_lfe::{RamLfeExecutionReceiptPayload, RamLfeProgramId, RamLfeProgramPolicy},
    };
    use mv::storage::StorageReadOnly;
    use nonzero_ext::nonzero;

    use crate::{
        kura::Kura, prelude::World, query::store::LiveQueryStore, smartcontracts::Execute,
        state::State,
    };

    fn test_state() -> State {
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        State::new_for_testing(World::default(), kura, query)
    }

    fn seed_domain(state: &mut State, domain_id: &DomainId, owner: &AccountId) {
        let domain = Domain {
            id: domain_id.clone(),
            logo: None,
            metadata: Metadata::default(),
            owned_by: owner.clone(),
        };
        state.world.domains.insert(domain_id.clone(), domain);
    }

    fn seed_account_with_uaid(
        state: &mut State,
        account_id: &AccountId,
        domain_id: &DomainId,
        uaid: UniversalAccountId,
    ) {
        let account = Account {
            id: account_id.clone(),
            metadata: Metadata::default(),
            label: None,
            uaid: Some(uaid),
            opaque_ids: Vec::new(),
        };
        let (account_id, account_value) = account.into_key_value();
        let _ = domain_id;
        state
            .world
            .accounts
            .insert(account_id.clone(), account_value);
        state.world.uaid_accounts.insert(uaid, account_id.clone());
    }

    fn claim_receipt(
        policy_id: &IdentifierPolicyId,
        program_policy: &RamLfeProgramPolicy,
        resolver: &KeyPair,
        uaid: UniversalAccountId,
        account_id: &AccountId,
        resolved_at_ms: u64,
        expires_at_ms: Option<u64>,
        output_seed: &[u8],
    ) -> IdentifierResolutionReceipt {
        let programmed_parameters =
            decode_bfv_programmed_public_parameters(&program_policy.commitment.public_parameters)
                .expect("decode programmed parameters");
        let output_hash = ram_lfe_output_hash(output_seed);
        let program_id_bytes =
            norito::to_bytes(&program_policy.program_id).expect("encode program id");
        let (opaque_id, receipt_hash) =
            identifier_hashes_from_output_hash(&program_id_bytes, &output_hash);
        let execution = RamLfeExecutionReceiptPayload {
            program_id: program_policy.program_id.clone(),
            program_digest: programmed_parameters.hidden_program_digest,
            backend: program_policy.backend,
            verification_mode: program_policy.verification_mode,
            output_hash,
            associated_data_hash: Hash::new([]),
            executed_at_ms: resolved_at_ms,
            expires_at_ms,
        };
        let payload = IdentifierResolutionReceiptPayload {
            policy_id: policy_id.clone(),
            execution,
            opaque_id: OpaqueAccountId::from(opaque_id),
            receipt_hash,
            uaid,
            account_id: account_id.clone(),
        };
        let signature: Signature = SignatureOf::new(resolver.private_key(), &payload).into();
        IdentifierResolutionReceipt {
            payload,
            signature: Some(signature),
            proof: None,
        }
    }

    fn sample_program_policy(
        owner: &AccountId,
        resolver: &KeyPair,
        program_id: &RamLfeProgramId,
    ) -> RamLfeProgramPolicy {
        let secret = b"resolver-secret";
        let params = BfvParameters {
            polynomial_degree: 64,
            ciphertext_modulus: 1_u64 << 52,
            plaintext_modulus: 256,
            decomposition_base_log: 12,
        };
        let hidden_program = default_bfv_programmed_hidden_program();
        let (encryption, _, _) = derive_identifier_key_material_from_seed(
            &params,
            63,
            secret,
            program_id.to_string().as_bytes(),
        )
        .expect("derive programmed public parameters");
        let public_parameters = bfv_programmed_public_parameters_with_program(
            encryption,
            &hidden_program,
            RamLfeVerificationMode::Signed,
            None,
        );
        let encoded_public_parameters =
            norito::to_bytes(&public_parameters).expect("encode programmed public parameters");
        let commitment = bfv_programmed_policy_commitment_with_program(
            secret,
            &encoded_public_parameters,
            &hidden_program,
        )
        .expect("build programmed policy commitment");
        RamLfeProgramPolicy::new(
            program_id.clone(),
            owner.clone(),
            RamLfeBackend::BfvProgrammedSha3_256V1,
            RamLfeVerificationMode::Signed,
            commitment,
            resolver.public_key().clone(),
        )
    }

    fn register_and_activate_program_policy(
        owner: &AccountId,
        tx: &mut crate::state::StateTransaction<'_, '_>,
        policy: RamLfeProgramPolicy,
    ) {
        RegisterRamLfeProgramPolicy {
            policy: policy.clone(),
        }
        .execute(owner, tx)
        .expect("register program policy");
        ActivateRamLfeProgramPolicy {
            program_id: policy.program_id,
        }
        .execute(owner, tx)
        .expect("activate program policy");
    }

    #[test]
    fn identifier_claim_and_revoke_update_indexes() {
        let mut state = test_state();
        let domain_id: DomainId = DomainId::try_new("directory", "universal").expect("domain id");
        let owner = AccountId::new(KeyPair::random().public_key().clone());
        let uaid = UniversalAccountId::from_hash(Hash::new(b"uaid-owner"));
        seed_domain(&mut state, &domain_id, &owner);
        seed_account_with_uaid(&mut state, &owner, &domain_id, uaid);

        let resolver = KeyPair::random();
        let policy_id: IdentifierPolicyId = "phone#retail".parse().expect("policy id");
        let program_id: RamLfeProgramId = "phone_retail".parse().expect("program id");
        let program_policy = sample_program_policy(&owner, &resolver, &program_id);
        let policy = IdentifierPolicy::new(
            policy_id.clone(),
            owner.clone(),
            IdentifierNormalization::PhoneE164,
            program_id.clone(),
        );

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        register_and_activate_program_policy(&owner, &mut tx, program_policy.clone());
        RegisterIdentifierPolicy {
            policy: policy.clone(),
        }
        .execute(&owner, &mut tx)
        .expect("register policy");
        ActivateIdentifierPolicy {
            policy_id: policy_id.clone(),
        }
        .execute(&owner, &mut tx)
        .expect("activate policy");
        let receipt = claim_receipt(
            &policy_id,
            &program_policy,
            &resolver,
            uaid,
            &owner,
            0,
            Some(1_800_000_000),
            b"+15551234567",
        );
        let opaque_id = receipt.payload.opaque_id;
        ClaimIdentifier {
            account: owner.clone(),
            receipt,
        }
        .execute(&owner, &mut tx)
        .expect("claim identifier");
        tx.apply();
        block.commit().expect("commit block");

        let claims = state.world.identifier_claims.view();
        let claim = claims.get(&opaque_id).expect("claim should be indexed");
        assert_eq!(claim.policy_id, policy_id);
        assert_eq!(claim.uaid, uaid);
        assert_eq!(claim.account_id, owner);
        assert_ne!(claim.receipt_hash, Hash::prehashed([0; Hash::LENGTH]));
        assert_eq!(
            state.world.opaque_uaids.view().get(&opaque_id),
            Some(&uaid),
            "opaque id should resolve to the seeded UAID"
        );
        assert!(
            state
                .world
                .accounts
                .view()
                .get(&owner)
                .expect("account exists")
                .opaque_ids()
                .contains(&opaque_id),
            "account should advertise claimed opaque id"
        );

        let header = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        RevokeIdentifier {
            policy_id: claim.policy_id.clone(),
            opaque_id,
        }
        .execute(&owner, &mut tx)
        .expect("revoke identifier");
        tx.apply();
        block.commit().expect("commit block");

        assert!(
            state
                .world
                .identifier_claims
                .view()
                .get(&opaque_id)
                .is_none(),
            "claim index should be cleared after revoke"
        );
        assert!(
            state.world.opaque_uaids.view().get(&opaque_id).is_none(),
            "opaque index should be cleared after revoke"
        );
        assert!(
            !state
                .world
                .accounts
                .view()
                .get(&owner)
                .expect("account exists")
                .opaque_ids()
                .contains(&opaque_id),
            "account should no longer advertise revoked opaque id"
        );
    }

    #[test]
    fn claim_identifier_rejects_accounts_without_uaid() {
        let mut state = test_state();
        let domain_id: DomainId = DomainId::try_new("directory", "universal").expect("domain id");
        let owner = AccountId::new(KeyPair::random().public_key().clone());
        seed_domain(&mut state, &domain_id, &owner);
        let account = Account {
            id: owner.clone(),
            metadata: Metadata::default(),
            label: None,
            uaid: None,
            opaque_ids: Vec::new(),
        };
        let (account_id, account_value) = account.into_key_value();
        state
            .world
            .accounts
            .insert(account_id.clone(), account_value);

        let resolver = KeyPair::random();
        let policy_id: IdentifierPolicyId = "email#retail".parse().expect("policy id");
        let program_id: RamLfeProgramId = "email_retail".parse().expect("program id");
        let program_policy = sample_program_policy(&owner, &resolver, &program_id);
        let policy = IdentifierPolicy::new(
            policy_id.clone(),
            owner.clone(),
            IdentifierNormalization::EmailAddress,
            program_id.clone(),
        );

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        register_and_activate_program_policy(&owner, &mut tx, program_policy.clone());
        RegisterIdentifierPolicy { policy }
            .execute(&owner, &mut tx)
            .expect("register policy");
        ActivateIdentifierPolicy {
            policy_id: policy_id.clone(),
        }
        .execute(&owner, &mut tx)
        .expect("activate policy");
        let receipt = claim_receipt(
            &policy_id,
            &program_policy,
            &resolver,
            UniversalAccountId::from_hash(Hash::new(b"uaid-missing")),
            &owner,
            0,
            None,
            b"alice@example.com",
        );
        let err = ClaimIdentifier {
            account: owner.clone(),
            receipt,
        }
        .execute(&owner, &mut tx)
        .expect_err("accounts without a UAID must be rejected");

        assert!(
            err.to_string().contains("does not have a UAID"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn claim_identifier_rejects_invalid_receipt_signature() {
        let mut state = test_state();
        let domain_id: DomainId = DomainId::try_new("directory", "universal").expect("domain id");
        let owner = AccountId::new(KeyPair::random().public_key().clone());
        let uaid = UniversalAccountId::from_hash(Hash::new(b"uaid-invalid-signature"));
        seed_domain(&mut state, &domain_id, &owner);
        seed_account_with_uaid(&mut state, &owner, &domain_id, uaid);

        let resolver = KeyPair::random();
        let wrong_resolver = KeyPair::random();
        let policy_id: IdentifierPolicyId = "phone#retail".parse().expect("policy id");
        let program_id: RamLfeProgramId = "phone_retail".parse().expect("program id");
        let program_policy = sample_program_policy(&owner, &resolver, &program_id);
        let policy = IdentifierPolicy::new(
            policy_id.clone(),
            owner.clone(),
            IdentifierNormalization::PhoneE164,
            program_id.clone(),
        );

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        register_and_activate_program_policy(&owner, &mut tx, program_policy.clone());
        RegisterIdentifierPolicy { policy }
            .execute(&owner, &mut tx)
            .expect("register policy");
        ActivateIdentifierPolicy {
            policy_id: policy_id.clone(),
        }
        .execute(&owner, &mut tx)
        .expect("activate policy");

        let err = ClaimIdentifier {
            account: owner.clone(),
            receipt: claim_receipt(
                &policy_id,
                &program_policy,
                &wrong_resolver,
                uaid,
                &owner,
                0,
                Some(60_000),
                b"+15551234567",
            ),
        }
        .execute(&owner, &mut tx)
        .expect_err("claim must reject a receipt signed by a different resolver");

        assert!(
            err.to_string().contains("signature is invalid"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn claim_identifier_rejects_zero_receipt_hash() {
        let mut state = test_state();
        let domain_id: DomainId = DomainId::try_new("directory", "universal").expect("domain id");
        let owner = AccountId::new(KeyPair::random().public_key().clone());
        let uaid = UniversalAccountId::from_hash(Hash::new(b"uaid-zero-receipt-hash"));
        seed_domain(&mut state, &domain_id, &owner);
        seed_account_with_uaid(&mut state, &owner, &domain_id, uaid);

        let resolver = KeyPair::random();
        let policy_id: IdentifierPolicyId = "phone#retail".parse().expect("policy id");
        let program_id: RamLfeProgramId = "phone_retail".parse().expect("program id");
        let program_policy = sample_program_policy(&owner, &resolver, &program_id);
        let policy = IdentifierPolicy::new(
            policy_id.clone(),
            owner.clone(),
            IdentifierNormalization::PhoneE164,
            program_id.clone(),
        );

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        register_and_activate_program_policy(&owner, &mut tx, program_policy.clone());
        RegisterIdentifierPolicy { policy }
            .execute(&owner, &mut tx)
            .expect("register policy");
        ActivateIdentifierPolicy {
            policy_id: policy_id.clone(),
        }
        .execute(&owner, &mut tx)
        .expect("activate policy");

        let mut receipt = claim_receipt(
            &policy_id,
            &program_policy,
            &resolver,
            uaid,
            &owner,
            0,
            Some(60_000),
            b"+15551234567",
        );
        receipt.payload.receipt_hash = Hash::prehashed([0; Hash::LENGTH]);
        receipt.signature = Some(SignatureOf::new(resolver.private_key(), &receipt.payload).into());
        let err = ClaimIdentifier {
            account: owner.clone(),
            receipt,
        }
        .execute(&owner, &mut tx)
        .expect_err("claim must reject a zero receipt hash");

        assert!(
            err.to_string().contains("receipt hash must not be zero"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn claim_identifier_rejects_expired_receipts() {
        let mut state = test_state();
        let domain_id: DomainId = DomainId::try_new("directory", "universal").expect("domain id");
        let owner = AccountId::new(KeyPair::random().public_key().clone());
        let uaid = UniversalAccountId::from_hash(Hash::new(b"uaid-expired-receipt"));
        seed_domain(&mut state, &domain_id, &owner);
        seed_account_with_uaid(&mut state, &owner, &domain_id, uaid);

        let resolver = KeyPair::random();
        let policy_id: IdentifierPolicyId = "email#retail".parse().expect("policy id");
        let program_id: RamLfeProgramId = "email_retail".parse().expect("program id");
        let program_policy = sample_program_policy(&owner, &resolver, &program_id);
        let policy = IdentifierPolicy::new(
            policy_id.clone(),
            owner.clone(),
            IdentifierNormalization::EmailAddress,
            program_id.clone(),
        );

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 11, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        register_and_activate_program_policy(&owner, &mut tx, program_policy.clone());
        RegisterIdentifierPolicy { policy }
            .execute(&owner, &mut tx)
            .expect("register policy");
        ActivateIdentifierPolicy {
            policy_id: policy_id.clone(),
        }
        .execute(&owner, &mut tx)
        .expect("activate policy");

        let err = ClaimIdentifier {
            account: owner.clone(),
            receipt: claim_receipt(
                &policy_id,
                &program_policy,
                &resolver,
                uaid,
                &owner,
                5,
                Some(10),
                b"alice@example.com",
            ),
        }
        .execute(&owner, &mut tx)
        .expect_err("claim must reject a receipt that is already expired");

        assert!(
            err.to_string().contains("expired at or before block time"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn expired_identifier_claim_can_be_reclaimed_by_new_uaid() {
        let mut state = test_state();
        let domain_id: DomainId = DomainId::try_new("directory", "universal").expect("domain id");
        let owner = AccountId::new(KeyPair::random().public_key().clone());
        let replacement = AccountId::new(KeyPair::random().public_key().clone());
        let owner_uaid = UniversalAccountId::from_hash(Hash::new(b"uaid-owner-expired"));
        let replacement_uaid =
            UniversalAccountId::from_hash(Hash::new(b"uaid-replacement-expired"));
        seed_domain(&mut state, &domain_id, &owner);
        seed_account_with_uaid(&mut state, &owner, &domain_id, owner_uaid);
        seed_account_with_uaid(&mut state, &replacement, &domain_id, replacement_uaid);

        let resolver = KeyPair::random();
        let policy_id: IdentifierPolicyId = "email#retail".parse().expect("policy id");
        let program_id: RamLfeProgramId = "email_retail".parse().expect("program id");
        let program_policy = sample_program_policy(&owner, &resolver, &program_id);
        let policy = IdentifierPolicy::new(
            policy_id.clone(),
            owner.clone(),
            IdentifierNormalization::EmailAddress,
            program_id.clone(),
        );
        let output_seed = b"shared-identifier-value";

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        register_and_activate_program_policy(&owner, &mut tx, program_policy.clone());
        RegisterIdentifierPolicy { policy }
            .execute(&owner, &mut tx)
            .expect("register policy");
        ActivateIdentifierPolicy {
            policy_id: policy_id.clone(),
        }
        .execute(&owner, &mut tx)
        .expect("activate policy");
        let expired_receipt = claim_receipt(
            &policy_id,
            &program_policy,
            &resolver,
            owner_uaid,
            &owner,
            0,
            Some(50),
            output_seed,
        );
        let opaque_id = expired_receipt.payload.opaque_id;
        ClaimIdentifier {
            account: owner.clone(),
            receipt: expired_receipt,
        }
        .execute(&owner, &mut tx)
        .expect("claim initial identifier");
        tx.apply();
        block.commit().expect("commit first block");

        let header = BlockHeader::new(nonzero!(2_u64), None, None, None, 100, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        let replacement_receipt = claim_receipt(
            &policy_id,
            &program_policy,
            &resolver,
            replacement_uaid,
            &replacement,
            100,
            Some(200),
            output_seed,
        );
        ClaimIdentifier {
            account: replacement.clone(),
            receipt: replacement_receipt,
        }
        .execute(&replacement, &mut tx)
        .expect("reclaim expired identifier");
        tx.apply();
        block.commit().expect("commit second block");

        let claim = state
            .world
            .identifier_claims
            .view()
            .get(&opaque_id)
            .cloned()
            .expect("claim should be re-bound");
        assert_eq!(claim.account_id, replacement);
        assert_eq!(claim.uaid, replacement_uaid);
        assert_eq!(claim.expires_at_ms, Some(200));
        assert_eq!(
            state.world.opaque_uaids.view().get(&opaque_id),
            Some(&replacement_uaid),
            "opaque index should point at the replacement UAID"
        );
        assert!(
            !state
                .world
                .accounts
                .view()
                .get(&owner)
                .expect("owner exists")
                .opaque_ids()
                .contains(&opaque_id),
            "expired owner binding should be removed from the old account"
        );
        assert!(
            state
                .world
                .accounts
                .view()
                .get(&replacement)
                .expect("replacement exists")
                .opaque_ids()
                .contains(&opaque_id),
            "replacement account should advertise the reclaimed opaque identifier"
        );
    }
}
