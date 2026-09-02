// Real CommitWrapper no-commit branch generation and native-verifier regression.

use ff::Field as _;
use halo2_base::{gates::circuit::builder::BaseCircuitBuilder, utils::BigPrimeField};
use halo2_proofs::{
    halo2curves::pasta::{EpAffine, EqAffine, Fp, Fq},
    plonk::{ProvingKey, VerifyingKey, keygen_pk, keygen_vk},
    poly::{commitment::ParamsProver as _, ipa::commitment::ParamsIPA},
};
use iroha_crypto::{Algorithm, KeyPair};
use iroha_data_model::{
    account::AccountId,
    offline::{
        OFFLINE_CASH_ACCEPTANCE_TICKET_MIN_RESERVED_INBOX_BYTES_V1, OFFLINE_CASH_HALO2_K_V1,
        OFFLINE_CASH_HARDWARE_REQUIRED_CAPABILITIES_V1, OFFLINE_CASH_PAYMENT_OUTBOX_MIN_BYTES_V1,
        OFFLINE_CASH_WIRE_VERSION_V1, OfflineCashAcceptanceIntentAuthorizationStatementV1,
        OfflineCashAcceptanceIntentAuthorizationV1, OfflineCashAcceptanceIntentV1,
        OfflineCashAcceptanceTicketV1, OfflineCashArtifactBindingV1, OfflineCashArtifactRoleV1,
        OfflineCashCommitCertificateV1, OfflineCashCommitEvidenceV1, OfflineCashDevicePublicKeyV1,
        OfflineCashHardwareCredentialV1, OfflineCashHardwarePlatformClassV1,
        OfflineCashHardwareProfileV1, OfflineCashLifecycleBindingV1,
        OfflineCashNoCommitClosureStatementV1, OfflineCashOperationKindV1,
        OfflineCashOutboxReservationV1, OfflineCashPaymentRequestV1,
        OfflineCashTrustedCommitTimeV1, offline_cash_device_key_reference_v1,
        offline_cash_suite_commitment_v1,
    },
};
use sha2::{Digest as _, Sha256};
use snark_verifier::{
    system::halo2::{Config, compile},
    verifier::plonk::PlonkProtocol,
};

use super::super::super::{
    OfflineCashCommitEvidenceOpeningGenerationV1, OfflineCashCommitWrapperEpGenerationWitnessV1,
    OfflineCashCommitWrapperEqGenerationWitnessV1, OfflineCashCommitWrapperGenerationPublicV1,
    OfflineCashCommitWrapperGenerationWitnessV1,
    OfflineCashCommitWrapperPrivateGenerationWitnessV1, OfflineCashEpAccumulatorV1,
    OfflineCashEqAccumulatorV1, OfflineCashGeneratedCommitWrapperArtifactsV1,
    OfflineCashGeneratedCommitWrapperEnvelopeV1, OfflineCashGuardBundleRelationWitnessV1,
    OfflineCashLoadedEpCommitWrapperArtifactsV1, OfflineCashLoadedEqCommitWrapperArtifactsV1,
    OfflineCashMintFinalityArtifactsV1, OfflineCashNoCommitClosureGenerationPublicV1,
    OfflineCashNormalizedGuardStatementV1, OfflineCashOperationV1, OfflineCashPastaParityV1,
    OfflineCashRecursionArtifactsV1, fold_offline_cash_ep_accumulators_v1,
    fold_offline_cash_eq_accumulators_v1, generate_offline_cash_commit_wrapper_artifacts_v1,
    initial_offline_cash_ep_accumulator_v1, initial_offline_cash_eq_accumulator_v1,
    prove_offline_cash_commit_wrapper_v1,
};
use super::super::super::{commit_wrapper, generation, guard_bundle, state_relation};
use super::super::{
    OfflineCashAuthenticatedRecursiveVerifierV1, verify_ep_succinct_protocol,
    verify_eq_succinct_protocol,
};
use super::{sign, signing_key};
use crate::zk::{
    offline_cash_v1_poseidon::{OfflineCashPoseidonFieldV1, digest_limbs, encode, from_u128},
    offline_cash_v1_state::{DigestV1, OfflineCashStateV1},
};

const AMOUNT: u128 = 7;
const JOURNAL_BEFORE: u128 = 11;
const AUTHORIZATION_COUNTER_BEFORE: u128 = 19;

fn tagged(tag: u8) -> DigestV1 {
    [tag; 32]
}

fn binding(role: OfflineCashArtifactRoleV1, bytes: &[u8]) -> OfflineCashArtifactBindingV1 {
    OfflineCashArtifactBindingV1 {
        role,
        sha256: Sha256::digest(bytes).into(),
        byte_len: u64::try_from(bytes.len()).expect("test artifact length fits u64"),
    }
}

fn carrier_circuit<F: BigPrimeField>(instances: &[F]) -> BaseCircuitBuilder<F> {
    let mut builder = BaseCircuitBuilder::new(false)
        .use_k(usize::try_from(OFFLINE_CASH_HALO2_K_V1).expect("k fits usize"))
        .use_lookup_bits(
            usize::try_from(OFFLINE_CASH_HALO2_K_V1 - 1).expect("lookup bits fit usize"),
        )
        .use_instance_columns(1);
    let public = instances
        .iter()
        .copied()
        .map(|value| builder.main(0).load_witness(value))
        .collect();
    builder.assigned_instances = vec![public];
    builder.calculate_params(Some(9));
    builder
}

struct EqCarrierKeys {
    proving_key: ProvingKey<EqAffine>,
    verifying_key: VerifyingKey<EqAffine>,
    protocol: PlonkProtocol<EqAffine>,
    protocol_digest: DigestV1,
}

struct EpCarrierKeys {
    proving_key: ProvingKey<EpAffine>,
    verifying_key: VerifyingKey<EpAffine>,
    protocol: PlonkProtocol<EpAffine>,
    protocol_digest: DigestV1,
}

fn eq_carrier_keys(parameters: &ParamsIPA<EqAffine>, count: usize) -> EqCarrierKeys {
    let circuit = carrier_circuit(&vec![Fp::ZERO; count]);
    let verifying_key = keygen_vk(parameters, &circuit).expect("Eq carrier VK");
    let proving_key =
        keygen_pk(parameters, verifying_key.clone(), &circuit).expect("Eq carrier PK");
    let protocol = compile(
        parameters,
        &verifying_key,
        Config::ipa().with_num_instance(vec![count]),
    );
    let protocol_digest =
        super::super::native_parent_protocol_digest_v1(&protocol, OfflineCashPastaParityV1::Eq)
            .expect("Eq carrier protocol digest");
    EqCarrierKeys {
        proving_key,
        verifying_key,
        protocol,
        protocol_digest,
    }
}

fn ep_carrier_keys(parameters: &ParamsIPA<EpAffine>, count: usize) -> EpCarrierKeys {
    let circuit = carrier_circuit(&vec![Fq::ZERO; count]);
    let verifying_key = keygen_vk(parameters, &circuit).expect("Ep carrier VK");
    let proving_key =
        keygen_pk(parameters, verifying_key.clone(), &circuit).expect("Ep carrier PK");
    let protocol = compile(
        parameters,
        &verifying_key,
        Config::ipa().with_num_instance(vec![count]),
    );
    let protocol_digest =
        super::super::native_parent_protocol_digest_v1(&protocol, OfflineCashPastaParityV1::Ep)
            .expect("Ep carrier protocol digest");
    EpCarrierKeys {
        proving_key,
        verifying_key,
        protocol,
        protocol_digest,
    }
}

struct CarrierKeys {
    eq: EqCarrierKeys,
    ep: EpCarrierKeys,
}

struct CarrierProof {
    eq_instances: Vec<Vec<Fp>>,
    ep_instances: Vec<Vec<Fq>>,
    eq_proof: Vec<u8>,
    ep_proof: Vec<u8>,
    eq_history: OfflineCashEqAccumulatorV1,
    ep_history: OfflineCashEpAccumulatorV1,
    eq_current: OfflineCashEqAccumulatorV1,
    ep_current: OfflineCashEpAccumulatorV1,
}

fn prove_carrier(
    eq_parameters: &ParamsIPA<EqAffine>,
    ep_parameters: &ParamsIPA<EpAffine>,
    keys: &CarrierKeys,
    eq_instances: Vec<Fp>,
    ep_instances: Vec<Fq>,
) -> CarrierProof {
    let eq_proof = super::super::super::real_handoff_qualification_tests::create_eq_proof(
        eq_parameters,
        &keys.eq.proving_key,
        carrier_circuit(&eq_instances),
        &eq_instances,
    );
    let ep_proof = super::super::super::real_handoff_qualification_tests::create_ep_proof(
        ep_parameters,
        &keys.ep.proving_key,
        carrier_circuit(&ep_instances),
        &ep_instances,
    );
    let eq_current = OfflineCashEqAccumulatorV1::from_native(
        &verify_eq_succinct_protocol(eq_parameters, &keys.eq.protocol, &eq_proof, &eq_instances)
            .expect("verify Eq carrier proof"),
    )
    .expect("encode Eq carrier accumulator");
    let ep_current = OfflineCashEpAccumulatorV1::from_native(
        &verify_ep_succinct_protocol(ep_parameters, &keys.ep.protocol, &ep_proof, &ep_instances)
            .expect("verify Ep carrier proof"),
    )
    .expect("encode Ep carrier accumulator");
    CarrierProof {
        eq_instances: vec![eq_instances],
        ep_instances: vec![ep_instances],
        eq_proof,
        ep_proof,
        eq_history: initial_offline_cash_eq_accumulator_v1(eq_parameters)
            .expect("Eq carrier history"),
        ep_history: initial_offline_cash_ep_accumulator_v1(ep_parameters)
            .expect("Ep carrier history"),
        eq_current,
        ep_current,
    }
}

fn set_digest<F: OfflineCashPoseidonFieldV1>(instances: &mut [F], offset: usize, digest: DigestV1) {
    instances[offset..offset + 2].copy_from_slice(&digest_limbs::<F>(digest));
}

#[allow(clippy::too_many_arguments)]
fn candidate_instances<F: OfflineCashPoseidonFieldV1>(
    relation: &OfflineCashGuardBundleRelationWitnessV1,
    transport_digest: DigestV1,
    eq_state_protocol: DigestV1,
    ep_state_protocol: DigestV1,
    guard_eq_protocol: DigestV1,
    guard_ep_protocol: DigestV1,
    guard_eq_audit: DigestV1,
    guard_ep_audit: DigestV1,
    history: &[u8; super::super::super::OFFLINE_CASH_HISTORY_ACCUMULATOR_BYTES_V1],
) -> Vec<F> {
    let guard = &relation.statement;
    let mut instances = vec![F::ZERO; state_relation::PUBLIC_INSTANCE_COUNT];
    instances[state_relation::public_instance::OPERATION] = F::from(2);
    instances[state_relation::public_instance::AMOUNT] = from_u128(guard.amount);
    instances[state_relation::public_instance::PROTOCOL_VERSION] =
        F::from(u64::from(guard.protocol_version));
    instances[state_relation::public_instance::POLICY_EPOCH] = F::from(guard.policy_epoch);
    instances[state_relation::public_instance::ASSET_SCALE] = F::from(u64::from(guard.asset_scale));
    for (offset, digest) in [
        (
            state_relation::public_instance::TRANSPORT_LO,
            transport_digest,
        ),
        (
            state_relation::public_instance::GUARD_LO,
            relation.statement_digest(),
        ),
        (
            state_relation::public_instance::PREDECESSOR_OUTER_LO,
            guard.predecessor_state_commitment,
        ),
        (
            state_relation::public_instance::SUCCESSOR_OUTER_LO,
            guard.successor_state_commitment,
        ),
        (
            state_relation::public_instance::RELEASE_LO,
            guard.release_id,
        ),
        (
            state_relation::public_instance::LIABILITY_POOL_LO,
            guard.liability_pool_id,
        ),
        (
            state_relation::public_instance::PEER_CREDIT_LO,
            guard.peer_credit_id,
        ),
        (
            state_relation::public_instance::PEER_RECIPIENT_LANE_LO,
            guard.peer_recipient_lane_id,
        ),
        (
            state_relation::public_instance::EQ_PROTOCOL_LO,
            eq_state_protocol,
        ),
        (
            state_relation::public_instance::EP_PROTOCOL_LO,
            ep_state_protocol,
        ),
        (
            state_relation::public_instance::GUARD_EQ_PROTOCOL_LO,
            guard_eq_protocol,
        ),
        (
            state_relation::public_instance::GUARD_EP_PROTOCOL_LO,
            guard_ep_protocol,
        ),
        (
            state_relation::public_instance::GUARD_EQ_CREDENTIAL_AUDIT_LO,
            guard_eq_audit,
        ),
        (
            state_relation::public_instance::GUARD_EP_CREDENTIAL_AUDIT_LO,
            guard_ep_audit,
        ),
        (
            state_relation::public_instance::LIFECYCLE_LO,
            guard.lifecycle_binding_digest,
        ),
        (
            state_relation::public_instance::PRECOMMIT_LO,
            guard.precommit_binding_digest,
        ),
        (
            state_relation::public_instance::PREDECESSOR_SUITE_LO,
            guard.predecessor_suite_id,
        ),
        (
            state_relation::public_instance::PREDECESSOR_VK_LO,
            guard.predecessor_vk_digest,
        ),
        (
            state_relation::public_instance::SUCCESSOR_SUITE_LO,
            guard.successor_suite_id,
        ),
        (
            state_relation::public_instance::SUCCESSOR_VK_LO,
            guard.successor_vk_digest,
        ),
        (
            state_relation::public_instance::ASSET_INCARNATION_LO,
            *guard.asset_incarnation.as_bytes(),
        ),
        (
            state_relation::public_instance::HARDWARE_PROFILE_LO,
            guard.hardware_profile_id,
        ),
        (
            state_relation::public_instance::NETWORK_LO,
            guard.network_id,
        ),
        (state_relation::public_instance::ASSET_LO, guard.asset_id),
    ] {
        set_digest(&mut instances, offset, digest);
    }
    instances.extend(history.chunks_exact(16).map(|chunk| {
        from_u128::<F>(u128::from_le_bytes(
            chunk.try_into().expect("history limb width"),
        ))
    }));
    assert_eq!(
        instances.len(),
        state_relation::PUBLIC_INSTANCE_COUNT
            + super::super::super::deferred_parent::accumulator_limb_count()
    );
    instances
}

fn hardware_profile() -> OfflineCashHardwareProfileV1 {
    let key = signing_key();
    let public_key = OfflineCashDevicePublicKeyV1::from_sec1_bytes(
        key.verifying_key().to_encoded_point(false).as_bytes(),
    )
    .expect("profile key");
    OfflineCashHardwareProfileV1 {
        version: OFFLINE_CASH_WIRE_VERSION_V1,
        protocol_version: OFFLINE_CASH_WIRE_VERSION_V1,
        hardware_profile_id: [0; 32],
        provider_id: tagged(0x71),
        platform_class: OfflineCashHardwarePlatformClassV1::DedicatedSecureElement,
        product_class_digest: tagged(0x72),
        firmware_policy_digest: tagged(0x73),
        enrollment_attestation_verifier_digest: tagged(0x74),
        attestation_trust_roots_digest: tagged(0x75),
        allowed_suite_commitment: offline_cash_suite_commitment_v1(tagged(0x76)),
        policy_epoch: 1,
        governance_credential_public_key: public_key,
        capability_mask: OFFLINE_CASH_HARDWARE_REQUIRED_CAPABILITIES_V1,
        qualification_report_digest: tagged(0x77),
        valid_from_ms: 1,
        expires_at_ms: 100_000,
    }
    .seal_hardware_profile_id()
    .expect("hardware profile")
}

fn request(predecessor: &OfflineCashStateV1) -> OfflineCashPaymentRequestV1 {
    let key = signing_key();
    let device_public_key = OfflineCashDevicePublicKeyV1::from_sec1_bytes(
        key.verifying_key().to_encoded_point(false).as_bytes(),
    )
    .expect("request device key");
    let mut credential = OfflineCashHardwareCredentialV1 {
        version: OFFLINE_CASH_WIRE_VERSION_V1,
        credential_id: [0; 32],
        network_id: predecessor.lane.network_id,
        hardware_profile_id: predecessor.hardware_profile_id,
        suite_id: predecessor.suite_id,
        firmware_policy_digest: tagged(0x81),
        policy_epoch: predecessor.policy_epoch,
        lane_commitment: tagged(0x82),
        hardware_epoch_id: tagged(0x83),
        hardware_epoch_generation: 1,
        device_public_key,
        device_key_reference: offline_cash_device_key_reference_v1(&device_public_key),
        issued_at_ms: 1,
        expires_at_ms: 100_000,
        governance_signature: sign(&key, b"placeholder credential signature"),
    }
    .seal_credential_id()
    .expect("credential id");
    credential.governance_signature = sign(
        &key,
        &credential
            .canonical_signing_bytes()
            .expect("credential signing bytes"),
    );
    let mut request = OfflineCashPaymentRequestV1 {
        version: OFFLINE_CASH_WIRE_VERSION_V1,
        release_id: predecessor.release_id,
        network_id: predecessor.lane.network_id,
        asset: predecessor.lane.asset.clone(),
        asset_incarnation: predecessor.asset_incarnation,
        scale: predecessor.lane.scale,
        liability_pool_id: predecessor.liability_pool_id,
        recipient: AccountId::new(
            KeyPair::from_seed(vec![0x84; 32], Algorithm::Ed25519)
                .public_key()
                .clone(),
        ),
        amount: AMOUNT,
        hardware_credential: credential,
        request_id: tagged(0x85),
        issued_at_ms: 10,
        expires_at_ms: 90_000,
        signature: sign(&key, b"placeholder request signature"),
    };
    request.signature = sign(
        &key,
        &request
            .canonical_signing_bytes()
            .expect("request signing bytes"),
    );
    request.validate_shape().expect("request shape");
    request
}

fn acceptance_ticket(
    request: &OfflineCashPaymentRequestV1,
    intent: &OfflineCashAcceptanceIntentV1,
) -> OfflineCashAcceptanceTicketV1 {
    let key = signing_key();
    let mut recipient_one_time_key = [0; 32];
    recipient_one_time_key[0] = 9;
    let mut ticket = OfflineCashAcceptanceTicketV1 {
        version: OFFLINE_CASH_WIRE_VERSION_V1,
        network_id: request.network_id,
        request_id: request.request_id,
        request_digest: request.canonical_digest().expect("request digest"),
        acceptance_ticket_id: tagged(0x86),
        asset: request.asset.clone(),
        asset_incarnation: request.asset_incarnation,
        scale: request.scale,
        intent_digest: intent
            .canonical_digest_against(request)
            .expect("intent digest"),
        exact_amount: intent.exact_amount,
        reserved_inbox_bytes: OFFLINE_CASH_ACCEPTANCE_TICKET_MIN_RESERVED_INBOX_BYTES_V1,
        recipient_one_time_key,
        hardware_profile_id: request.hardware_credential.hardware_profile_id,
        policy_epoch: request.hardware_credential.policy_epoch,
        issued_at_ms: 20,
        expires_at_ms: 80_000,
        signature: sign(&key, b"placeholder ticket signature"),
    };
    ticket.signature = sign(
        &key,
        &ticket
            .canonical_signing_bytes()
            .expect("ticket signing bytes"),
    );
    ticket
        .validate_shape_against(request, intent)
        .expect("ticket shape");
    ticket
}

#[allow(clippy::too_many_arguments)]
fn guard_relation(
    predecessor: &OfflineCashStateV1,
    successor: &OfflineCashStateV1,
    credential: crate::zk::offline_cash_v1_recursion::OfflineCashPlatformCredentialStatementV1,
    device_secret: DigestV1,
    amount: u128,
    peer_credit_id: DigestV1,
    peer_recipient_lane_id: DigestV1,
    lifecycle: DigestV1,
    precommit: DigestV1,
    sender_authorization: DigestV1,
    transition_intent: DigestV1,
    transition_effect: DigestV1,
    recovery_record: DigestV1,
    durable_outbox_effect: DigestV1,
) -> OfflineCashGuardBundleRelationWitnessV1 {
    let relation = OfflineCashGuardBundleRelationWitnessV1 {
        statement: OfflineCashNormalizedGuardStatementV1 {
            version: OFFLINE_CASH_WIRE_VERSION_V1,
            protocol_version: predecessor.protocol_version,
            predecessor_suite_id: predecessor.suite_id,
            predecessor_vk_digest: predecessor.vk_digest,
            successor_suite_id: predecessor.suite_id,
            successor_vk_digest: predecessor.vk_digest,
            operation: OfflineCashOperationV1::SendSplit,
            amount,
            peer_credit_id,
            peer_recipient_lane_id,
            mint_finality_proof_binding_digest: [0; 32],
            release_id: predecessor.release_id,
            network_id: *predecessor.lane.network_id.as_bytes(),
            asset_id: predecessor.lane.normalized_asset_id().expect("asset id"),
            asset_incarnation: predecessor.asset_incarnation,
            asset_scale: predecessor.lane.scale,
            liability_pool_id: predecessor.liability_pool_id,
            hardware_profile_id: predecessor.hardware_profile_id,
            policy_epoch: predecessor.policy_epoch,
            lane_id: predecessor.lane.device_lane_id,
            predecessor_state_commitment: predecessor.state_commitment,
            successor_state_commitment: successor.state_commitment,
            predecessor_state_nonce_commitment: predecessor.state_nonce_commitment,
            successor_state_nonce_commitment: successor.state_nonce_commitment,
            predecessor_logical_sequence: predecessor.logical_sequence,
            successor_logical_sequence: successor.logical_sequence,
            predecessor_hardware_epoch_generation: predecessor.hardware_epoch.generation,
            successor_hardware_epoch_generation: successor.hardware_epoch.generation,
            predecessor_hardware_epoch_id: predecessor.hardware_epoch.epoch_id,
            successor_hardware_epoch_id: successor.hardware_epoch.epoch_id,
            predecessor_key_reference: predecessor.device_policy_binding.device_key_reference,
            successor_key_reference: successor.device_policy_binding.device_key_reference,
            predecessor_hardware_policy_id: predecessor.device_policy_binding.hardware_policy_id,
            successor_hardware_policy_id: successor.device_policy_binding.hardware_policy_id,
            journal_revision_before: JOURNAL_BEFORE,
            journal_revision_after: JOURNAL_BEFORE + 1,
            lifecycle_binding_digest: lifecycle,
            precommit_binding_digest: precommit,
            terminal_commit_binding_digest: [0; 32],
            sender_one_time_authorization_digest: sender_authorization,
            suite_upgrade_authorization_digest: [0; 32],
            transition_intent_digest: transition_intent,
            transition_effect_digest: transition_effect,
            recovery_record_digest: recovery_record,
            durable_inbox_effect_digest: credential.canonical_empty_effect_digest,
            durable_outbox_effect_digest: durable_outbox_effect,
        },
        canonical_empty_effect_digest: credential.canonical_empty_effect_digest,
        predecessor_credential: credential,
        successor_credential: credential,
        predecessor_device_authority_secret: device_secret,
        successor_device_authority_secret: device_secret,
    };
    relation
        .validate()
        .expect("valid no-commit test Guard relation");
    relation
}

fn private_transition(
    predecessor: OfflineCashStateV1,
    successor: OfflineCashStateV1,
    request: &OfflineCashPaymentRequestV1,
    intent: OfflineCashAcceptanceIntentV1,
    ticket: Option<OfflineCashAcceptanceTicketV1>,
    one_use_hardware_authorization: DigestV1,
    sender_one_time_opening: DigestV1,
) -> OfflineCashCommitWrapperPrivateGenerationWitnessV1 {
    let lifecycle = OfflineCashLifecycleBindingV1 {
        version: OFFLINE_CASH_WIRE_VERSION_V1,
        network_id: predecessor.lane.network_id,
        protocol_version: predecessor.protocol_version,
        suite_id: predecessor.suite_id,
        vk_digest: predecessor.vk_digest,
        release_id: predecessor.release_id,
        asset: predecessor.lane.asset.clone(),
        asset_incarnation: predecessor.asset_incarnation,
        scale: predecessor.lane.scale,
        liability_pool_id: predecessor.liability_pool_id,
        hardware_profile_id: predecessor.hardware_profile_id,
        policy_epoch: predecessor.policy_epoch,
        operation_kind: OfflineCashOperationKindV1::SendSplit,
        request_id: request.request_id,
        acceptance_ticket_id: ticket
            .as_ref()
            .map_or(tagged(0x87), |value| value.acceptance_ticket_id),
        credit_id: tagged(0x88),
        ciphertext_digest: tagged(0x89),
    };
    let outbox_reservation = OfflineCashOutboxReservationV1 {
        reservation_id: tagged(0x8A),
        operation_kind: OfflineCashOperationKindV1::SendSplit,
        reserved_outbox_bytes: OFFLINE_CASH_PAYMENT_OUTBOX_MIN_BYTES_V1,
        issued_at_ms: 10,
        expires_at_ms: 90_000,
    };
    let commit_evidence =
        OfflineCashCommitEvidenceV1::TrustedTime(OfflineCashTrustedCommitTimeV1 {
            time_evidence_commitment: tagged(0x8B),
        });
    let commit_certificate = OfflineCashCommitCertificateV1 {
        version: OFFLINE_CASH_WIRE_VERSION_V1,
        certificate_id: [0; 32],
        candidate_envelope_digest: tagged(0x8C),
        lifecycle_binding_digest: lifecycle.canonical_digest().expect("lifecycle digest"),
        transition_nullifier: tagged(0x8D),
        outbox_reservation_commitment: outbox_reservation
            .canonical_commitment()
            .expect("outbox commitment"),
        commit_evidence,
        hardware_profile_id: predecessor.hardware_profile_id,
        policy_epoch: predecessor.policy_epoch,
        hardware_terminal_commitment: tagged(0x8E),
    }
    .seal_certificate_id()
    .expect("certificate id");
    OfflineCashCommitWrapperPrivateGenerationWitnessV1 {
        lifecycle,
        predecessor,
        successor,
        request: Some(request.clone()),
        acceptance_intent: Some(intent),
        acceptance_ticket: ticket,
        outbox_reservation,
        commit_certificate,
        commit_evidence_opening: OfflineCashCommitEvidenceOpeningGenerationV1 {
            opening: tagged(0x8F),
            trusted_commit_time_ms: 100,
            lease_id: [0; 32],
            lease_valid_from_ms: 0,
            lease_expires_at_ms: 0,
        },
        one_use_hardware_authorization,
        sender_one_time_opening,
        terminal_envelope_digest: tagged(0x90),
        journal_revision_before: JOURNAL_BEFORE,
        journal_revision_after: JOURNAL_BEFORE + 1,
        authorization_counter_before: AUTHORIZATION_COUNTER_BEFORE,
        authorization_counter_after: AUTHORIZATION_COUNTER_BEFORE + 1,
        hardware_profile: hardware_profile(),
        hardware_credential: request.hardware_credential,
    }
}

struct WrapperFolds {
    eq_candidate: super::super::super::OfflineCashEqFoldProofV1,
    ep_candidate: super::super::super::OfflineCashEpFoldProofV1,
    eq_guard: super::super::super::OfflineCashEqFoldProofV1,
    ep_guard: super::super::super::OfflineCashEpFoldProofV1,
    eq_merge: super::super::super::OfflineCashEqFoldProofV1,
    ep_merge: super::super::super::OfflineCashEpFoldProofV1,
    eq_successor: OfflineCashEqAccumulatorV1,
    ep_successor: OfflineCashEpAccumulatorV1,
}

fn wrapper_folds(
    eq_parameters: &ParamsIPA<EqAffine>,
    ep_parameters: &ParamsIPA<EpAffine>,
    candidate: &CarrierProof,
    guard: &CarrierProof,
) -> WrapperFolds {
    let eq_candidate = fold_offline_cash_eq_accumulators_v1(
        eq_parameters,
        &candidate.eq_current,
        &candidate.eq_history,
    )
    .expect("fold Eq candidate");
    let ep_candidate = fold_offline_cash_ep_accumulators_v1(
        ep_parameters,
        &candidate.ep_current,
        &candidate.ep_history,
    )
    .expect("fold Ep candidate");
    let eq_guard =
        fold_offline_cash_eq_accumulators_v1(eq_parameters, &guard.eq_current, &guard.eq_history)
            .expect("fold Eq Guard");
    let ep_guard =
        fold_offline_cash_ep_accumulators_v1(ep_parameters, &guard.ep_current, &guard.ep_history)
            .expect("fold Ep Guard");
    let eq_merge = fold_offline_cash_eq_accumulators_v1(
        eq_parameters,
        eq_candidate.successor(),
        eq_guard.successor(),
    )
    .expect("merge Eq histories");
    let ep_merge = fold_offline_cash_ep_accumulators_v1(
        ep_parameters,
        ep_candidate.successor(),
        ep_guard.successor(),
    )
    .expect("merge Ep histories");
    WrapperFolds {
        eq_candidate: eq_candidate.proof().clone(),
        ep_candidate: ep_candidate.proof().clone(),
        eq_guard: eq_guard.proof().clone(),
        ep_guard: ep_guard.proof().clone(),
        eq_merge: eq_merge.proof().clone(),
        ep_merge: ep_merge.proof().clone(),
        eq_successor: eq_merge.successor().clone(),
        ep_successor: ep_merge.successor().clone(),
    }
}

#[allow(clippy::too_many_arguments)]
fn wrapper_witness<'a>(
    public: OfflineCashCommitWrapperGenerationPublicV1,
    private_transition: OfflineCashCommitWrapperPrivateGenerationWitnessV1,
    guard_relation: OfflineCashGuardBundleRelationWitnessV1,
    enabled_hardware_profiles: [[u8; 32]; super::super::super::OFFLINE_CASH_COMMIT_WRAPPER_ENABLED_PROFILE_SLOTS_V1],
    candidate_keys: &'a CarrierKeys,
    guard_keys: &'a CarrierKeys,
    candidate: &'a CarrierProof,
    guard: &'a CarrierProof,
    folds: &'a WrapperFolds,
) -> OfflineCashCommitWrapperGenerationWitnessV1<'a> {
    OfflineCashCommitWrapperGenerationWitnessV1 {
        public,
        private_transition,
        terminal_guard_relation: guard_relation,
        enabled_hardware_profiles,
        eq: OfflineCashCommitWrapperEqGenerationWitnessV1 {
            candidate_protocol: &candidate_keys.eq.protocol,
            candidate_instances: &candidate.eq_instances,
            candidate_proof: &candidate.eq_proof,
            candidate_history: &candidate.eq_history,
            candidate_history_fold_proof: &folds.eq_candidate,
            terminal_guard_protocol: &guard_keys.eq.protocol,
            terminal_guard_instances: &guard.eq_instances,
            terminal_guard_proof: &guard.eq_proof,
            terminal_guard_history: &guard.eq_history,
            terminal_guard_history_fold_proof: &folds.eq_guard,
            merge_fold_proof: &folds.eq_merge,
            successor_history: &folds.eq_successor,
        },
        ep: OfflineCashCommitWrapperEpGenerationWitnessV1 {
            candidate_protocol: &candidate_keys.ep.protocol,
            candidate_instances: &candidate.ep_instances,
            candidate_proof: &candidate.ep_proof,
            candidate_history: &candidate.ep_history,
            candidate_history_fold_proof: &folds.ep_candidate,
            terminal_guard_protocol: &guard_keys.ep.protocol,
            terminal_guard_instances: &guard.ep_instances,
            terminal_guard_proof: &guard.ep_proof,
            terminal_guard_history: &guard.ep_history,
            terminal_guard_history_fold_proof: &folds.ep_guard,
            merge_fold_proof: &folds.ep_merge,
            successor_history: &folds.ep_successor,
        },
    }
}

fn load_wrapper_artifacts(
    generated: &OfflineCashGeneratedCommitWrapperArtifactsV1,
    release_id: DigestV1,
    profile_digest: DigestV1,
    manifest_digest: DigestV1,
    suite_id: DigestV1,
    vk_digest: DigestV1,
) -> (
    OfflineCashLoadedEqCommitWrapperArtifactsV1,
    OfflineCashLoadedEpCommitWrapperArtifactsV1,
) {
    let eq_verifying_key = generation::read_eq_commit_wrapper_vk(
        &generated.eq_verifying_key,
        generated.eq_circuit_params.clone(),
    )
    .expect("decode Eq wrapper VK");
    let eq_proving_key = generation::read_eq_commit_wrapper_pk(
        &generated.eq_proving_key,
        generated.eq_circuit_params.clone(),
    )
    .expect("decode Eq wrapper PK");
    let ep_verifying_key = generation::read_ep_commit_wrapper_vk(
        &generated.ep_verifying_key,
        generated.ep_circuit_params.clone(),
    )
    .expect("decode Ep wrapper VK");
    let ep_proving_key = generation::read_ep_commit_wrapper_pk(
        &generated.ep_proving_key,
        generated.ep_circuit_params.clone(),
    )
    .expect("decode Ep wrapper PK");
    (
        OfflineCashLoadedEqCommitWrapperArtifactsV1 {
            parameters: ParamsIPA::new(OFFLINE_CASH_HALO2_K_V1),
            proving_key: eq_proving_key,
            verifying_key: eq_verifying_key,
            circuit_params: generated.eq_circuit_params.clone(),
            protocol_digest: generated.eq_protocol_digest,
            release_id,
            profile_digest,
            artifact_manifest_digest: manifest_digest,
            suite_id,
            vk_digest,
            enabled_hardware_profiles: generated.enabled_hardware_profiles,
        },
        OfflineCashLoadedEpCommitWrapperArtifactsV1 {
            parameters: ParamsIPA::new(OFFLINE_CASH_HALO2_K_V1),
            proving_key: ep_proving_key,
            verifying_key: ep_verifying_key,
            circuit_params: generated.ep_circuit_params.clone(),
            protocol_digest: generated.ep_protocol_digest,
            release_id,
            profile_digest,
            artifact_manifest_digest: manifest_digest,
            suite_id,
            vk_digest,
            enabled_hardware_profiles: generated.enabled_hardware_profiles,
        },
    )
}

#[allow(clippy::too_many_arguments)]
fn authenticated_verifier(
    eq: &OfflineCashLoadedEqCommitWrapperArtifactsV1,
    ep: &OfflineCashLoadedEpCommitWrapperArtifactsV1,
    generated: &OfflineCashGeneratedCommitWrapperArtifactsV1,
    candidate_keys: &CarrierKeys,
    guard_keys: &CarrierKeys,
    release_id: DigestV1,
    profile_digest: DigestV1,
    manifest_digest: DigestV1,
    suite_id: DigestV1,
    vk_digest: DigestV1,
    empty_effect: DigestV1,
) -> OfflineCashAuthenticatedRecursiveVerifierV1 {
    let eq_wrapper_protocol = compile(
        &eq.parameters,
        &eq.verifying_key,
        Config::ipa().with_num_instance(vec![
            super::super::super::COMMIT_WRAPPER_PUBLIC_INSTANCE_COUNT_V1,
        ]),
    );
    let ep_wrapper_protocol = compile(
        &ep.parameters,
        &ep.verifying_key,
        Config::ipa().with_num_instance(vec![
            super::super::super::COMMIT_WRAPPER_PUBLIC_INSTANCE_COUNT_V1,
        ]),
    );
    let eq_wrapper_binding = binding(
        OfflineCashArtifactRoleV1::CommitWrapperVkEq,
        &generated.eq_verifying_key,
    );
    let ep_wrapper_binding = binding(
        OfflineCashArtifactRoleV1::CommitWrapperVkEp,
        &generated.ep_verifying_key,
    );
    let placeholder = |role, tag| OfflineCashArtifactBindingV1 {
        role,
        sha256: tagged(tag),
        byte_len: 1,
    };
    let artifacts = OfflineCashRecursionArtifactsV1 {
        release_id,
        profile_digest,
        eq_protocol_digest: candidate_keys.eq.protocol_digest,
        ep_protocol_digest: candidate_keys.ep.protocol_digest,
        commit_wrapper_eq_protocol_digest: generated.eq_protocol_digest,
        commit_wrapper_ep_protocol_digest: generated.ep_protocol_digest,
        mint_authorization_eq_protocol_digest: encode(Fp::from(0x31)),
        mint_authorization_ep_protocol_digest: encode(Fq::from(0x32)),
        guard_bundle_eq_protocol_digest: guard_keys.eq.protocol_digest,
        guard_bundle_ep_protocol_digest: guard_keys.ep.protocol_digest,
        guard_bundle_verifying_key_eq: binding(
            OfflineCashArtifactRoleV1::GuardBundleVkEq,
            &guard_keys
                .eq
                .verifying_key
                .to_bytes(halo2_proofs::SerdeFormat::Processed),
        ),
        guard_bundle_verifying_key_ep: binding(
            OfflineCashArtifactRoleV1::GuardBundleVkEp,
            &guard_keys
                .ep
                .verifying_key
                .to_bytes(halo2_proofs::SerdeFormat::Processed),
        ),
        commit_wrapper_verifying_key_eq: eq_wrapper_binding,
        commit_wrapper_verifying_key_ep: ep_wrapper_binding,
        mint_finality: OfflineCashMintFinalityArtifactsV1 {
            proving_key_eq: placeholder(OfflineCashArtifactRoleV1::MintCreditPkEq, 0xA1),
            verifying_key_eq: placeholder(OfflineCashArtifactRoleV1::MintCreditVkEq, 0xA2),
            proving_key_ep: placeholder(OfflineCashArtifactRoleV1::MintCreditPkEp, 0xA3),
            verifying_key_ep: placeholder(OfflineCashArtifactRoleV1::MintCreditVkEp, 0xA4),
        },
        artifact_manifest_digest: manifest_digest,
        canonical_empty_effect_digest: empty_effect,
    };
    OfflineCashAuthenticatedRecursiveVerifierV1 {
        artifacts,
        eq_parameters: eq.parameters.clone(),
        ep_parameters: ep.parameters.clone(),
        eq_state_protocol: candidate_keys.eq.protocol.clone(),
        ep_state_protocol: candidate_keys.ep.protocol.clone(),
        eq_wrapper_protocol,
        ep_wrapper_protocol,
        eq_mint_authorization_protocol: candidate_keys.eq.protocol.clone(),
        ep_mint_authorization_protocol: candidate_keys.ep.protocol.clone(),
        eq_mint_protocol: candidate_keys.eq.protocol.clone(),
        ep_mint_protocol: candidate_keys.ep.protocol.clone(),
        eq_protocol_digest: candidate_keys.eq.protocol_digest,
        ep_protocol_digest: candidate_keys.ep.protocol_digest,
        guard_eq_protocol_digest: guard_keys.eq.protocol_digest,
        guard_ep_protocol_digest: guard_keys.ep.protocol_digest,
        wrapper_eq_protocol_digest: generated.eq_protocol_digest,
        wrapper_ep_protocol_digest: generated.ep_protocol_digest,
        mint_authorization_eq_protocol_digest: encode(Fp::from(0x31)),
        mint_authorization_ep_protocol_digest: encode(Fq::from(0x32)),
        mint_eq_protocol_digest: encode(Fp::from(0x33)),
        mint_ep_protocol_digest: encode(Fq::from(0x34)),
        mint_genesis_roster_id: tagged(0xA5),
        release_id,
        suite_id,
        vk_set_digest: vk_digest,
        artifact_manifest_digest: manifest_digest,
        wrapper_eq_binding: eq_wrapper_binding,
        wrapper_ep_binding: ep_wrapper_binding,
    }
}

#[test]
#[ignore = "real k=16 paired CommitWrapper key generation and two branch proofs"]
fn real_no_commit_generation_branch_is_accepted_only_by_its_authenticated_release() {
    let eq_parameters = ParamsIPA::<EqAffine>::new(OFFLINE_CASH_HALO2_K_V1);
    let ep_parameters = ParamsIPA::<EpAffine>::new(OFFLINE_CASH_HALO2_K_V1);
    let release_id = tagged(0x41);
    let manifest_digest = tagged(0x42);
    let profile_digest = tagged(0x43);
    let (credential, device_secret) =
        super::super::super::real_handoff_qualification_tests::credential_witness(
            0,
            release_id,
            tagged(0x44),
        );
    let predecessor =
        super::super::super::real_handoff_qualification_tests::aggregate_state_with_balance(
            release_id,
            &credential.statement,
            tagged(0x45),
            25,
            3,
        );
    let prepared_successor =
        super::super::super::real_handoff_qualification_tests::aggregate_state_with_balance(
            release_id,
            &credential.statement,
            tagged(0x46),
            predecessor.balance - AMOUNT,
            predecessor.logical_sequence + 1,
        );
    let request = request(&predecessor);
    let request_digest = request.canonical_digest().expect("request digest");
    let one_use_hardware_authorization = tagged(0x47);
    let sender_one_time_opening = tagged(0x48);
    let prepared_authorization = commit_wrapper::canonical_prepared_one_use_authorization_digest_v1(
        OfflineCashOperationV1::SendSplit,
        one_use_hardware_authorization,
        &predecessor,
        JOURNAL_BEFORE,
        AUTHORIZATION_COUNTER_BEFORE,
    );
    let intent_id = tagged(0x49);
    let sender_one_time_commitment = commit_wrapper::canonical_sender_one_time_commitment_v1(
        sender_one_time_opening,
        prepared_authorization,
        request_digest,
        intent_id,
        AMOUNT,
    );
    let intent = OfflineCashAcceptanceIntentV1 {
        version: OFFLINE_CASH_WIRE_VERSION_V1,
        request_digest,
        intent_id,
        exact_amount: AMOUNT,
        sender_one_time_commitment,
    };
    let authorization_statement = OfflineCashAcceptanceIntentAuthorizationStatementV1 {
        version: OFFLINE_CASH_WIRE_VERSION_V1,
        intent,
        release_id,
        suite_id: predecessor.suite_id,
        vk_digest: predecessor.vk_digest,
        artifact_manifest_digest: manifest_digest,
    };
    let authorization_semantic = authorization_statement
        .canonical_digest_against(&request)
        .expect("authorization semantic digest");
    let intent_digest = intent
        .canonical_digest_against(&request)
        .expect("intent digest");
    let delivery_slot = tagged(0x4A);
    let authorization_precommit = commit_wrapper::canonical_precommit_binding_digest_v1(
        authorization_semantic,
        request_digest,
        intent_digest,
        [0; 32],
        AMOUNT,
        delivery_slot,
        prepared_authorization,
    );
    let authorization_guard = guard_relation(
        &predecessor,
        &prepared_successor,
        credential.statement,
        device_secret,
        AMOUNT,
        tagged(0x4B),
        tagged(0x4C),
        authorization_semantic,
        authorization_precommit,
        [0; 32],
        authorization_semantic,
        tagged(0x4D),
        tagged(0x4E),
        delivery_slot,
    );

    let candidate_eq_keys = eq_carrier_keys(
        &eq_parameters,
        state_relation::PUBLIC_INSTANCE_COUNT
            + super::super::super::deferred_parent::accumulator_limb_count(),
    );
    let candidate_ep_keys = ep_carrier_keys(
        &ep_parameters,
        state_relation::PUBLIC_INSTANCE_COUNT
            + super::super::super::deferred_parent::accumulator_limb_count(),
    );
    let candidate_keys = CarrierKeys {
        eq: candidate_eq_keys,
        ep: candidate_ep_keys,
    };
    let guard_keys = CarrierKeys {
        eq: eq_carrier_keys(
            &eq_parameters,
            guard_bundle::GUARD_RECURSIVE_PUBLIC_INSTANCE_COUNT_V1,
        ),
        ep: ep_carrier_keys(
            &ep_parameters,
            guard_bundle::GUARD_RECURSIVE_PUBLIC_INSTANCE_COUNT_V1,
        ),
    };
    let guard_eq_audit = encode(Fp::from(0x51));
    let guard_ep_audit = encode(Fq::from(0x52));
    let eq_initial =
        initial_offline_cash_eq_accumulator_v1(&eq_parameters).expect("initial Eq history");
    let ep_initial =
        initial_offline_cash_ep_accumulator_v1(&ep_parameters).expect("initial Ep history");
    let candidate = prove_carrier(
        &eq_parameters,
        &ep_parameters,
        &candidate_keys,
        candidate_instances::<Fp>(
            &authorization_guard,
            authorization_semantic,
            candidate_keys.eq.protocol_digest,
            candidate_keys.ep.protocol_digest,
            guard_keys.eq.protocol_digest,
            guard_keys.ep.protocol_digest,
            guard_eq_audit,
            guard_ep_audit,
            eq_initial.as_bytes(),
        ),
        candidate_instances::<Fq>(
            &authorization_guard,
            authorization_semantic,
            candidate_keys.eq.protocol_digest,
            candidate_keys.ep.protocol_digest,
            guard_keys.eq.protocol_digest,
            guard_keys.ep.protocol_digest,
            guard_eq_audit,
            guard_ep_audit,
            ep_initial.as_bytes(),
        ),
    );
    let authorization_guard_proof = prove_carrier(
        &eq_parameters,
        &ep_parameters,
        &guard_keys,
        super::super::super::real_handoff_qualification_tests::guard_public_instances::<Fp>(
            &authorization_guard,
            guard_eq_audit,
            guard_ep_audit,
            eq_initial.as_bytes(),
        ),
        super::super::super::real_handoff_qualification_tests::guard_public_instances::<Fq>(
            &authorization_guard,
            guard_eq_audit,
            guard_ep_audit,
            ep_initial.as_bytes(),
        ),
    );
    let authorization_folds = wrapper_folds(
        &eq_parameters,
        &ep_parameters,
        &candidate,
        &authorization_guard_proof,
    );
    let mut enabled_profiles =
        [[0; 32]; super::super::super::OFFLINE_CASH_COMMIT_WRAPPER_ENABLED_PROFILE_SLOTS_V1];
    enabled_profiles[0] = predecessor.hardware_profile_id;
    let authorization_public =
        OfflineCashCommitWrapperGenerationPublicV1::AcceptanceIntentAuthorization(
            super::super::super::OfflineCashAcceptanceIntentAuthorizationGenerationPublicV1 {
                request: request.clone(),
                statement: authorization_statement.clone(),
                guard_eq_credential_audit: guard_eq_audit,
                guard_ep_credential_audit: guard_ep_audit,
            },
        );
    let authorization_private = private_transition(
        predecessor.clone(),
        prepared_successor.clone(),
        &request,
        intent,
        None,
        one_use_hardware_authorization,
        sender_one_time_opening,
    );
    let authorization_witness = wrapper_witness(
        authorization_public,
        authorization_private,
        authorization_guard.clone(),
        enabled_profiles,
        &candidate_keys,
        &guard_keys,
        &candidate,
        &authorization_guard_proof,
        &authorization_folds,
    );
    let generated =
        generate_offline_cash_commit_wrapper_artifacts_v1(authorization_witness.clone())
            .expect("generate real wrapper keys from authorization branch");
    let (eq_wrapper, ep_wrapper) = load_wrapper_artifacts(
        &generated,
        release_id,
        profile_digest,
        manifest_digest,
        predecessor.suite_id,
        predecessor.vk_digest,
    );
    let generated_authorization =
        prove_offline_cash_commit_wrapper_v1(&eq_wrapper, &ep_wrapper, authorization_witness)
            .expect("prove real acceptance-authorization branch");
    let OfflineCashGeneratedCommitWrapperEnvelopeV1::AcceptanceIntentAuthorization(
        authorization_proof,
    ) = generated_authorization.proof
    else {
        panic!("authorization prover returned wrong branch")
    };
    let authorization = OfflineCashAcceptanceIntentAuthorizationV1 {
        version: OFFLINE_CASH_WIRE_VERSION_V1,
        statement: authorization_statement,
        proof: authorization_proof,
    };
    authorization
        .validate_shape_against(&request)
        .expect("generated authorization shape");

    let ticket = acceptance_ticket(&request, &intent);
    let ticket_digest = ticket
        .canonical_digest_against(&request, &intent)
        .expect("ticket digest");
    let authorization_digest = authorization
        .canonical_digest_against(&request)
        .expect("authorization digest");
    let hardware_recovery_nonce = tagged(0x53);
    let recovery_id = commit_wrapper::canonical_no_commit_recovery_id_v1(
        prepared_authorization,
        request_digest,
        ticket_digest,
        request.hardware_credential.lane_commitment,
        hardware_recovery_nonce,
    );
    let cancellation_successor = commit_wrapper::canonical_no_commit_cancellation_successor_v1(
        prepared_authorization,
        recovery_id,
        authorization_digest,
        ticket_digest,
        delivery_slot,
        JOURNAL_BEFORE + 1,
        AUTHORIZATION_COUNTER_BEFORE + 1,
    );
    let cancellation_nullifier =
        commit_wrapper::canonical_predecessor_conflict_nullifier_v1(prepared_authorization);
    let provisional_guard = guard_relation(
        &predecessor,
        &predecessor,
        credential.statement,
        device_secret,
        0,
        [0; 32],
        [0; 32],
        tagged(0x54),
        tagged(0x55),
        cancellation_successor,
        recovery_id,
        authorization_digest,
        ticket_digest,
        delivery_slot,
    );
    let hardware_binding =
        commit_wrapper::canonical_no_commit_hardware_binding_v1(&provisional_guard.statement);
    let closure_statement = OfflineCashNoCommitClosureStatementV1 {
        version: OFFLINE_CASH_WIRE_VERSION_V1,
        release_id,
        suite_id: predecessor.suite_id,
        vk_digest: predecessor.vk_digest,
        artifact_manifest_digest: manifest_digest,
        sender_hardware_binding_commitment: hardware_binding,
        request_id: request.request_id,
        request_digest,
        acceptance_ticket_id: ticket.acceptance_ticket_id,
        ticket_digest,
        intent_authorization_digest: authorization_digest,
        intent_digest,
        exact_amount: AMOUNT,
        sender_one_time_commitment,
        recovery_id,
        cancellation_nullifier,
        equivalent_delivery_slot_commitment: delivery_slot,
    };
    let closure_semantic = closure_statement
        .canonical_digest()
        .expect("closure semantic digest");
    let closure_precommit = commit_wrapper::canonical_precommit_binding_digest_v1(
        closure_semantic,
        request_digest,
        intent_digest,
        ticket_digest,
        AMOUNT,
        delivery_slot,
        prepared_authorization,
    );
    let closure_guard = guard_relation(
        &predecessor,
        &predecessor,
        credential.statement,
        device_secret,
        0,
        [0; 32],
        [0; 32],
        closure_semantic,
        closure_precommit,
        cancellation_successor,
        recovery_id,
        authorization_digest,
        ticket_digest,
        delivery_slot,
    );
    assert_eq!(
        commit_wrapper::canonical_no_commit_hardware_binding_v1(&closure_guard.statement),
        hardware_binding
    );
    let closure_guard_proof = prove_carrier(
        &eq_parameters,
        &ep_parameters,
        &guard_keys,
        super::super::super::real_handoff_qualification_tests::guard_public_instances::<Fp>(
            &closure_guard,
            guard_eq_audit,
            guard_ep_audit,
            eq_initial.as_bytes(),
        ),
        super::super::super::real_handoff_qualification_tests::guard_public_instances::<Fq>(
            &closure_guard,
            guard_eq_audit,
            guard_ep_audit,
            ep_initial.as_bytes(),
        ),
    );
    let closure_folds = wrapper_folds(
        &eq_parameters,
        &ep_parameters,
        &candidate,
        &closure_guard_proof,
    );
    let closure_witness = wrapper_witness(
        OfflineCashCommitWrapperGenerationPublicV1::NoCommitClosure(
            OfflineCashNoCommitClosureGenerationPublicV1 {
                statement: closure_statement,
                request: request.clone(),
                intent_authorization: authorization,
                acceptance_ticket: ticket.clone(),
                guard_eq_credential_audit: guard_eq_audit,
                guard_ep_credential_audit: guard_ep_audit,
                hardware_recovery_nonce,
            },
        ),
        private_transition(
            predecessor.clone(),
            prepared_successor,
            &request,
            intent,
            Some(ticket),
            one_use_hardware_authorization,
            sender_one_time_opening,
        ),
        closure_guard,
        enabled_profiles,
        &candidate_keys,
        &guard_keys,
        &candidate,
        &closure_guard_proof,
        &closure_folds,
    );
    let generated_closure =
        prove_offline_cash_commit_wrapper_v1(&eq_wrapper, &ep_wrapper, closure_witness)
            .expect("prove real no-commit branch");
    let OfflineCashGeneratedCommitWrapperEnvelopeV1::NoCommitClosure(closure) =
        generated_closure.proof
    else {
        panic!("no-commit prover returned wrong branch")
    };

    let verifier = authenticated_verifier(
        &eq_wrapper,
        &ep_wrapper,
        &generated,
        &candidate_keys,
        &guard_keys,
        release_id,
        profile_digest,
        manifest_digest,
        predecessor.suite_id,
        predecessor.vk_digest,
        credential.statement.canonical_empty_effect_digest,
    );
    assert_eq!(verifier.release_id, closure.statement.release_id);
    assert_eq!(verifier.suite_id, closure.statement.suite_id);
    assert_eq!(verifier.vk_set_digest, closure.statement.vk_digest);
    assert_eq!(
        verifier.artifact_manifest_digest,
        closure.statement.artifact_manifest_digest
    );
    assert_eq!(
        verifier.wrapper_eq_protocol_digest(),
        closure.proof.eq_protocol_digest
    );
    assert_eq!(
        verifier.wrapper_ep_protocol_digest(),
        closure.proof.ep_protocol_digest
    );
    verifier
        .verify_no_commit_closure_and_decide(&closure)
        .expect("authenticated native verifier accepts generated no-commit closure");

    let mut wrong_protocol = closure.clone();
    wrong_protocol.proof.eq_protocol_digest = candidate_keys.eq.protocol_digest;
    assert!(
        verifier
            .verify_no_commit_closure_and_decide(&wrong_protocol)
            .is_err()
    );
    let mut wrong_release = closure.clone();
    wrong_release.statement.release_id[0] ^= 1;
    assert!(
        verifier
            .verify_no_commit_closure_and_decide(&wrong_release)
            .is_err()
    );
    let mut wrong_vk = closure.clone();
    wrong_vk.statement.vk_digest[0] ^= 1;
    assert!(
        verifier
            .verify_no_commit_closure_and_decide(&wrong_vk)
            .is_err()
    );
    let mut wrong_manifest = closure;
    wrong_manifest.statement.artifact_manifest_digest[0] ^= 1;
    assert!(
        verifier
            .verify_no_commit_closure_and_decide(&wrong_manifest)
            .is_err()
    );
}
