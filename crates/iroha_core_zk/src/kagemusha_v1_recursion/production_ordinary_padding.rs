//! Native-owned parser operands for inactive ordinary Mint slots.
//!
//! These are private construction data, not credits, authorizations or proof admission. The
//! Bootstrap owner requires the exact zero-State; the incoming owner also uses parser operands
//! for its inactive Mint slot under a zero selector. No real mint or payer balance is needed to
//! initialize a zero balance. Actual Guard verification and its complete history remain mandatory.

use super::*;
use iroha_data_model::{
    account::AccountId,
    kagemusha::{
        KAGEMUSHA_XCHACHA20POLY1305_TAG_BYTES_V1, KagemushaEncryptedCreditEnvelopeV1,
        KagemushaLifecycleBindingV1, KagemushaMintAuthorizationContextV1,
        KagemushaMintAuthorizationStatementV1, KagemushaMintCreditStatementV1,
        KagemushaOperationKindV1, KagemushaPairedProofV1, kagemusha_ciphertext_digest_v1,
        kagemusha_credit_opening_canonical_len_v1,
    },
};

pub(super) struct BootstrapMintPadding {
    pub(super) authorization: KagemushaMintAuthorizationV1,
    pub(super) credit: KagemushaMintCreditV1,
}

fn marker(state: &KagemushaStateV1, role: u8) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"iroha:kagemusha:v1:ordinary-bootstrap-inactive-mint\0");
    hash.update(state.release_id);
    hash.update(state.suite_id);
    hash.update(state.vk_digest);
    hash.update(state.state_nonce_commitment);
    hash.update([role]);
    hash.finalize().into()
}

#[allow(clippy::too_many_arguments)]
pub(super) fn bootstrap_mint_padding(
    state: &KagemushaStateV1,
    recipient: &AccountId,
    artifact_manifest_digest: [u8; 32],
    genesis_authorization_id: [u8; 32],
    eq_authorization_protocol: &PlonkProtocol<EqAffine>,
    ep_authorization_protocol: &PlonkProtocol<EpAffine>,
    eq_mint_protocol: &PlonkProtocol<EqAffine>,
    ep_mint_protocol: &PlonkProtocol<EpAffine>,
    eq_history: &KagemushaEqAccumulatorV1,
    ep_history: &KagemushaEpAccumulatorV1,
) -> Result<BootstrapMintPadding, KagemushaArtifactGenerationErrorV1> {
    state
        .validate()
        .map_err(|error| proving_error(error.to_string()))?;
    if state.balance != 0
        || state.logical_sequence != 0
        || state.secure_index != 0
        || state.next_one_use_key_reference != [0; 32]
        || artifact_manifest_digest == [0; 32]
        || genesis_authorization_id == [0; 32]
    {
        return Err(proving_error(
            "inactive Bootstrap mint operands require the exact zero State",
        ));
    }
    inactive_mint_parser_operands(
        state,
        recipient,
        artifact_manifest_digest,
        genesis_authorization_id,
        eq_authorization_protocol,
        ep_authorization_protocol,
        eq_mint_protocol,
        ep_mint_protocol,
        eq_history,
        ep_history,
    )
}

/// Parser-only operands for an inactive ordinary Mint slot of an actual selected State.
/// These invalid AEAD/proof bytes cannot fund or authorize Mint; the active selector stays zero.
#[allow(clippy::too_many_arguments)]
pub(super) fn inactive_mint_parser_operands(
    state: &KagemushaStateV1,
    recipient: &AccountId,
    artifact_manifest_digest: [u8; 32],
    genesis_authorization_id: [u8; 32],
    eq_authorization_protocol: &PlonkProtocol<EqAffine>,
    ep_authorization_protocol: &PlonkProtocol<EpAffine>,
    eq_mint_protocol: &PlonkProtocol<EqAffine>,
    ep_mint_protocol: &PlonkProtocol<EpAffine>,
    eq_history: &KagemushaEqAccumulatorV1,
    ep_history: &KagemushaEpAccumulatorV1,
) -> Result<BootstrapMintPadding, KagemushaArtifactGenerationErrorV1> {
    state.validate().map_err(|e| proving_error(e.to_string()))?;
    if artifact_manifest_digest == [0; 32] || genesis_authorization_id == [0; 32] {
        return Err(proving_error(
            "inactive Mint parser release identity is absent",
        ));
    }
    let mut public_x25519_basepoint = [0; 32];
    public_x25519_basepoint[0] = 9;
    // Known public metadata and invalid AEAD data deliberately give this slot no credit.
    // Its canonical size is still identical to a real envelope's parser input.
    let envelope = KagemushaEncryptedCreditEnvelopeV1 {
        version: 1,
        ephemeral_x25519_public_key: public_x25519_basepoint,
        nonce: [0; 24],
        ciphertext_and_tag: vec![
            0;
            kagemusha_credit_opening_canonical_len_v1()
                .map_err(|e| proving_error(e.to_string()))?
                .checked_add(KAGEMUSHA_XCHACHA20POLY1305_TAG_BYTES_V1)
                .ok_or_else(|| proving_error(
                    "inactive envelope length overflow"
                ))?
        ],
    };
    let encrypted = envelope
        .canonical_bytes_against_recipient_key(public_x25519_basepoint)
        .map_err(|e| proving_error(e.to_string()))?;
    let ciphertext_digest = kagemusha_ciphertext_digest_v1(&encrypted);
    let context = KagemushaMintAuthorizationContextV1 {
        version: 1,
        operation_id: marker(state, 1),
        release_id: state.release_id,
        suite_id: state.suite_id,
        vk_digest: state.vk_digest,
        artifact_manifest_digest,
        network_id: state.lane.network_id,
        asset: state.lane.asset.clone(),
        asset_incarnation: state.asset_incarnation,
        scale: state.lane.scale,
        liability_pool_id: state.liability_pool_id,
        amount: 1,
        payer: recipient.clone(),
        recipient: recipient.clone(),
        hardware_credential_id: marker(state, 2),
        hardware_profile_id: state.hardware_profile_id,
        policy_epoch: state.policy_epoch,
        recipient_credential_commitment: marker(state, 3),
        credit_commitment: marker(state, 4),
        recipient_one_time_key: public_x25519_basepoint,
    };
    context
        .validate_shape()
        .map_err(|e| proving_error(e.to_string()))?;
    let mut statement = KagemushaMintCreditStatementV1 {
        version: 1,
        lifecycle: KagemushaLifecycleBindingV1 {
            version: 1,
            network_id: context.network_id,
            protocol_version: state.protocol_version,
            suite_id: state.suite_id,
            vk_digest: state.vk_digest,
            release_id: state.release_id,
            asset: context.asset.clone(),
            asset_incarnation: state.asset_incarnation,
            scale: context.scale,
            liability_pool_id: context.liability_pool_id,
            hardware_profile_id: state.hardware_profile_id,
            policy_epoch: state.policy_epoch,
            operation_kind: KagemushaOperationKindV1::MintFold,
            request_id: [0; 32],
            receiver_lane_commitment: [0; 32],
            credit_id: [0; 32],
            ciphertext_digest,
        },
        recipient_credential_commitment: context.recipient_credential_commitment,
        authorization_context_digest: context
            .canonical_digest()
            .map_err(|e| proving_error(e.to_string()))?,
        mint_authorization_digest: [0; 32],
        amount: context.amount,
        issuance_commitment: marker(state, 5),
        recipient: context.recipient.clone(),
        credit_commitment: context.credit_commitment,
        minted_at_ms: 1,
    }
    .seal_credit_id()
    .map_err(|e| proving_error(e.to_string()))?;
    let authorization_statement = KagemushaMintAuthorizationStatementV1 {
        version: 1,
        context,
        issuance_commitment: statement.issuance_commitment,
        credit_id: statement.lifecycle.credit_id,
        ciphertext_digest,
    };
    let mut authorization = KagemushaMintAuthorizationV1 {
        version: 1,
        proof: parser_pair(
            eq_authorization_protocol,
            ep_authorization_protocol,
            authorization_statement
                .canonical_digest()
                .map_err(|e| proving_error(e.to_string()))?,
            eq_history,
            ep_history,
        )?,
        statement: authorization_statement,
    };
    // The common paired-proof position is the recipient commitment exposed by
    // MintAuthorization, including inactive parser operands. Keep the exact original
    // metadata consistent with the canonical payload assembled by the State SHA graph.
    // This is a public commitment marker, never a credential-verification audit.
    authorization.proof.guard_eq_credential_audit = authorization
        .statement
        .context
        .recipient_credential_commitment;
    statement.mint_authorization_digest = authorization
        .canonical_digest()
        .map_err(|e| proving_error(e.to_string()))?;
    let mut proof = parser_pair(
        eq_mint_protocol,
        ep_mint_protocol,
        statement
            .canonical_digest()
            .map_err(|e| proving_error(e.to_string()))?,
        eq_history,
        ep_history,
    )?;
    proof.guard_eq_credential_audit = marker(state, 6);
    proof.guard_ep_credential_audit = marker(state, 7);
    let credit = KagemushaMintCreditV1 {
        version: 1,
        statement,
        finality_certificate_binding: proof.guard_eq_credential_audit,
        finality_authority_head: proof.guard_ep_credential_audit,
        finality_genesis_authorization_id: genesis_authorization_id,
        finality_proof_binding_digest: marker(state, 8),
        proof,
        encrypted_credit: encrypted,
        artifact_manifest_digest,
    };
    credit
        .validate_shape_against_authorization(&authorization)
        .map_err(|e| proving_error(e.to_string()))?;
    Ok(BootstrapMintPadding {
        authorization,
        credit,
    })
}

fn parser_pair(
    eq_protocol: &PlonkProtocol<EqAffine>,
    ep_protocol: &PlonkProtocol<EpAffine>,
    semantic_digest: [u8; 32],
    eq_history: &KagemushaEqAccumulatorV1,
    ep_history: &KagemushaEpAccumulatorV1,
) -> Result<KagemushaPairedProofV1, KagemushaArtifactGenerationErrorV1> {
    use halo2_proofs::halo2curves::group::{GroupEncoding as _, prime::PrimeCurveAffine as _};
    Ok(KagemushaPairedProofV1 {
        version: 1,
        eq_protocol_digest: native_parent_protocol_digest_v1(
            eq_protocol,
            KagemushaPastaParityV1::Eq,
        )
        .map_err(proving_error)?,
        ep_protocol_digest: native_parent_protocol_digest_v1(
            ep_protocol,
            KagemushaPastaParityV1::Ep,
        )
        .map_err(proving_error)?,
        semantic_digest,
        guard_eq_credential_audit: [1; 32],
        guard_ep_credential_audit: [2; 32],
        eq_deferred_audit: [3; 32],
        ep_deferred_audit: [4; 32],
        eq_proof: super::super::dummy_ordinary_proof_bytes(
            eq_protocol,
            EqAffine::generator().to_bytes().as_ref(),
            KagemushaPastaParityV1::Eq,
        )?,
        ep_proof: super::super::dummy_ordinary_proof_bytes(
            ep_protocol,
            EpAffine::generator().to_bytes().as_ref(),
            KagemushaPastaParityV1::Ep,
        )?,
        eq_history: eq_history.as_bytes().to_vec(),
        ep_history: ep_history.as_bytes().to_vec(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use halo2_base::gates::{
        GateInstructions as _, RangeInstructions as _, circuit::builder::BaseCircuitBuilder,
    };
    use halo2_proofs::plonk::keygen_vk;
    use snark_verifier::system::halo2::Config as ProtocolConfig;

    fn zero_state() -> KagemushaStateV1 {
        parser_state(0, 0, 0)
    }

    fn parser_state(balance: u128, sequence: u128, index: u128) -> KagemushaStateV1 {
        use crate::kagemusha_v1_poseidon::{
            KAGEMUSHA_STATE_DOMAIN_V1, KagemushaPoseidonFieldV1, digest_limbs, empty_replay_root,
            encode, from_u128, hash, paired_commitment,
        };

        fn commitment<F: KagemushaPoseidonFieldV1>(state: &KagemushaStateV1) -> F {
            let mut inputs = vec![
                F::from(u64::from(state.version)),
                F::from(u64::from(state.protocol_version)),
            ];
            for digest in [
                state.suite_id,
                state.vk_digest,
                state.release_id,
                *state.asset_incarnation.as_bytes(),
                state.liability_pool_id,
                state.hardware_profile_id,
            ] {
                inputs.extend(digest_limbs::<F>(digest));
            }
            inputs.push(F::from(state.policy_epoch));
            inputs.extend(digest_limbs::<F>(state.lane.normalized_network_id()));
            inputs.extend(digest_limbs::<F>(state.lane.normalized_asset_id().unwrap()));
            inputs.push(F::from(u64::from(state.lane.scale)));
            inputs.extend(digest_limbs::<F>(state.lane.device_lane_id));
            inputs.extend([
                from_u128::<F>(state.balance),
                from_u128::<F>(state.logical_sequence),
                from_u128::<F>(state.secure_index),
                from_u128::<F>(state.hardware_epoch.generation),
            ]);
            for digest in [
                state.hardware_epoch.epoch_id,
                state.device_policy_binding.device_key_reference,
                state.device_policy_binding.hardware_policy_id,
                state.next_one_use_key_reference,
                state.state_nonce_commitment,
            ] {
                inputs.extend(digest_limbs::<F>(digest));
            }
            inputs.push(empty_replay_root::<F>());
            hash(KAGEMUSHA_STATE_DOMAIN_V1, &inputs)
        }

        // Reuse identity fields only. The projection fixture's placeholder commitments
        // are replaced with the actual canonical empty replay roots and Native Poseidon.
        let mut state = crate::kagemusha_v1_recursion::tests::state_verification_fixture()
            .0
            .successor;
        state.balance = balance;
        state.logical_sequence = sequence;
        state.secure_index = index;
        state.consumed_credit_root = iroha_data_model::kagemusha::KagemushaPastaStateCommitmentV1 {
            eq: encode(empty_replay_root::<Fp>()),
            ep: encode(empty_replay_root::<Fq>()),
        };
        let (components, head) =
            paired_commitment(commitment::<Fp>(&state), commitment::<Fq>(&state));
        state.state_commitment_components = components;
        state.state_commitment = head;
        state
            .validate()
            .expect("actual mathematical zero-State commitment");
        state
    }

    #[test]
    fn inactive_bootstrap_mint_operands_are_canonical_parsers_without_proof_authority() {
        // Real locally compiled test protocols exercise the maintained IPA parser at k=16.
        // They are neither signed release keys nor a financial/platform admission fixture.
        let eq_parameters = super::super::super::canonical_kagemusha_eq_parameters_v1();
        let ep_parameters = super::super::super::canonical_kagemusha_ep_parameters_v1();
        macro_rules! protocol {
            ($field:ty, $parameters:expr, $width:expr) => {{
                let mut circuit = BaseCircuitBuilder::<$field>::new(false)
                    .use_k(crate::kagemusha_v1_recursion::KAGEMUSHA_RECURSION_IPA_K_V1 as usize)
                    .use_lookup_bits(
                        (crate::kagemusha_v1_recursion::KAGEMUSHA_RECURSION_IPA_K_V1 - 1) as usize,
                    )
                    .use_instance_columns(1);
                let range = circuit.range_chip();
                let cells = (0..$width)
                    .map(|index| {
                        let value = <$field>::from(index as u64 + 1);
                        let cell = circuit.main(0).load_witness(value);
                        range.gate().assert_is_const(circuit.main(0), &cell, &value);
                        cell
                    })
                    .collect();
                circuit.assigned_instances = vec![cells];
                circuit.calculate_params(Some(9));
                let key = keygen_vk(&$parameters, &circuit).expect("actual test verifying key");
                compile(
                    &$parameters,
                    &key,
                    ProtocolConfig::ipa().with_num_instance(vec![$width]),
                )
            }};
        }
        let eq_authorization = protocol!(
            Fp,
            eq_parameters,
            MINT_AUTHORIZATION_PUBLIC_INSTANCE_COUNT_V1
        );
        let ep_authorization = protocol!(
            Fq,
            ep_parameters,
            MINT_AUTHORIZATION_PUBLIC_INSTANCE_COUNT_V1
        );
        let eq_mint = protocol!(
            Fp,
            eq_parameters,
            KAGEMUSHA_MINT_AUTHORITY_PUBLIC_INSTANCE_COUNT_V1
        );
        let ep_mint = protocol!(
            Fq,
            ep_parameters,
            KAGEMUSHA_MINT_AUTHORITY_PUBLIC_INSTANCE_COUNT_V1
        );
        let state = zero_state();
        let recipient = crate::kagemusha_v1_recursion::tests::compact_mint_credit_fixture()
            .statement
            .recipient;
        let eq_history = initial_kagemusha_eq_accumulator_v1(&eq_parameters).unwrap();
        let ep_history = initial_kagemusha_ep_accumulator_v1(&ep_parameters).unwrap();
        let make = |state: &KagemushaStateV1, manifest, genesis| {
            bootstrap_mint_padding(
                state,
                &recipient,
                manifest,
                genesis,
                &eq_authorization,
                &ep_authorization,
                &eq_mint,
                &ep_mint,
                &eq_history,
                &ep_history,
            )
        };
        let mut invalid_state = state.clone();
        invalid_state.version = 0;
        let expected_reason = invalid_state.validate().unwrap_err().to_string();
        assert!(matches!(
            make(&invalid_state, [21; 32], [22; 32]),
            Err(KagemushaArtifactGenerationErrorV1::CircuitBuild(reason))
                if reason == expected_reason
        ));
        let nonzero = parser_state(17, 3, 4);
        assert!(make(&nonzero, [21; 32], [22; 32]).is_err());
        let inactive = inactive_mint_parser_operands(
            &nonzero,
            &recipient,
            [21; 32],
            [22; 32],
            &eq_authorization,
            &ep_authorization,
            &eq_mint,
            &ep_mint,
            &eq_history,
            &ep_history,
        )
        .expect("same real nonzero State supplies only an inactive parser slot");
        assert_eq!(nonzero.balance, 17);
        assert_eq!(nonzero.logical_sequence, 3);
        assert_eq!(nonzero.secure_index, 4);
        assert_eq!(
            inactive.authorization.statement.context.release_id,
            nonzero.release_id
        );
        assert_eq!(
            inactive.credit.statement.lifecycle.liability_pool_id,
            nonzero.liability_pool_id
        );
        assert!(
            inactive_mint_parser_operands(
                &nonzero,
                &recipient,
                [0; 32],
                [22; 32],
                &eq_authorization,
                &ep_authorization,
                &eq_mint,
                &ep_mint,
                &eq_history,
                &ep_history
            )
            .is_err()
        );
        let padding = make(&state, [21; 32], [22; 32]).expect("zero-State parser operands");
        let authorization = &padding.authorization;
        let credit = &padding.credit;
        credit
            .validate_shape_against_authorization(authorization)
            .expect("complete canonical schema and exact authorization joins");
        assert_eq!(
            authorization.proof.guard_eq_credential_audit,
            authorization
                .statement
                .context
                .recipient_credential_commitment
        );
        assert_eq!(authorization.statement.context.recipient, recipient);
        assert_eq!(authorization.statement.context.release_id, state.release_id);
        assert_eq!(
            authorization.statement.context.hardware_profile_id,
            state.hardware_profile_id
        );
        assert_eq!(credit.proof.eq_history, eq_history.as_bytes());
        assert_eq!(credit.proof.ep_history, ep_history.as_bytes());
        let canonical = norito::encode_canonical(credit).unwrap();
        assert_eq!(
            norito::decode_canonical::<KagemushaMintCreditV1>(&canonical).unwrap(),
            *credit
        );

        let eq_authorization_instances = mint_authorization_public_instances_v1::<Fp>(
            &authorization.statement,
            authorization.proof.guard_ep_credential_audit,
            authorization.proof.eq_deferred_audit,
            authorization.proof.ep_deferred_audit,
            eq_history.as_bytes(),
        )
        .unwrap();
        let ep_authorization_instances = mint_authorization_public_instances_v1::<Fq>(
            &authorization.statement,
            authorization.proof.guard_ep_credential_audit,
            authorization.proof.eq_deferred_audit,
            authorization.proof.ep_deferred_audit,
            ep_history.as_bytes(),
        )
        .unwrap();
        let request =
            crate::kagemusha_v1_recursion::KagemushaMintFinalityHelperVerificationRequestV1 {
                eq_protocol_digest: credit.proof.eq_protocol_digest,
                ep_protocol_digest: credit.proof.ep_protocol_digest,
                statement: &credit.statement,
                semantic_digest: credit.proof.semantic_digest,
                proof: &credit.proof,
                finality_certificate_binding: credit.finality_certificate_binding,
                finality_authority_head: credit.finality_authority_head,
                finality_genesis_authorization_id: credit.finality_genesis_authorization_id,
                finality_proof_binding_digest: credit.finality_proof_binding_digest,
                artifact_manifest_digest: credit.artifact_manifest_digest,
            };
        let eq_mint_instances =
            native_backend::mint_public_instances::<Fp>(&request, eq_history.as_bytes()).unwrap();
        let ep_mint_instances =
            native_backend::mint_public_instances::<Fq>(&request, ep_history.as_bytes()).unwrap();
        // All four proof transcripts parse under their actual compiled protocol. Their false
        // equations must fail terminal decision, so shape-valid padding can grant no mint.
        for (protocol, proof, instances) in [
            (
                &eq_authorization,
                &authorization.proof.eq_proof,
                &eq_authorization_instances,
            ),
            (&eq_mint, &credit.proof.eq_proof, &eq_mint_instances),
        ] {
            let current = KagemushaEqAccumulatorV1::from_native(
                &verify_eq_succinct_protocol(&eq_parameters, protocol, proof, instances)
                    .expect("canonical Eq inactive proof parser"),
            )
            .unwrap();
            assert!(decide_kagemusha_eq_accumulator_v1(&eq_parameters, &current).is_err());
        }
        for (protocol, proof, instances) in [
            (
                &ep_authorization,
                &authorization.proof.ep_proof,
                &ep_authorization_instances,
            ),
            (&ep_mint, &credit.proof.ep_proof, &ep_mint_instances),
        ] {
            let current = KagemushaEpAccumulatorV1::from_native(
                &verify_ep_succinct_protocol(&ep_parameters, protocol, proof, instances)
                    .expect("canonical Ep inactive proof parser"),
            )
            .unwrap();
            assert!(decide_kagemusha_ep_accumulator_v1(&ep_parameters, &current).is_err());
        }
        for mutation in 0..4 {
            let mut foreign = state.clone();
            match mutation {
                0 => foreign.balance = 1,
                1 => foreign.logical_sequence = 1,
                2 => foreign.secure_index = 1,
                _ => foreign.next_one_use_key_reference = [23; 32],
            }
            assert!(make(&foreign, [21; 32], [22; 32]).is_err());
        }
        assert!(make(&state, [0; 32], [22; 32]).is_err());
        assert!(make(&state, [21; 32], [0; 32]).is_err());
        let mut substituted = credit.clone();
        substituted.statement.amount += 1;
        assert!(
            substituted
                .validate_shape_against_authorization(authorization)
                .is_err()
        );
    }
}
