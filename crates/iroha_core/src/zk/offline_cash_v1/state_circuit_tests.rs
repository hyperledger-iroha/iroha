use halo2_proofs::{
    dev::MockProver,
    halo2curves::{
        ff::{Field, PrimeField},
        group::prime::PrimeCurveAffine as _,
        pasta::{EpAffine, EqAffine, Fp, Fq},
    },
    plonk::{Circuit, ConstraintSystem},
};
use iroha_data_model::offline::OFFLINE_CASH_HALO2_K_V1;
use sha2::Digest as _;

use super::{
    protocol::{
        preflight_offline_cash_recursion_activation_v1, OfflineCashHalo2CircuitRoleV1,
        OfflineCashRecursionActivationPreflightErrorV1,
    },
    state_abi::{
        pack_words_as_field, OfflineCashStateAbiErrorV1, OfflineCashStateOperationV1,
        OfflineCashStatePublicInstancesV1, AMOUNT_WORD_START, CONTEXT_WORD_START, LINK_WORD_START,
        PARENT_0_WORD_START, PARENT_1_WORD_START, RELEASE_WORD_START, REQUEST_WORD_START,
        RESULT_WORD_START, SCALE_WORD, SEMANTIC_WORD_START, STATE_ABI_WORDS, STATE_INSTANCE_CELLS,
        STATE_INSTANCE_CELLS_MAX, STATE_OPERATION_WORD, STATE_WORDS_PER_INSTANCE,
        TRANSITION_WORD_START,
    },
    state_circuit::{OfflineCashEpStateCircuitV1, OfflineCashEqStateCircuitV1},
    state_relation::{
        balance_head_message_v1, credit_head_message_v1, offline_cash_balance_head_v1,
        offline_cash_credit_head_v1, offline_cash_receive_opening_v1,
        offline_cash_receive_semantic_digest_v1, offline_cash_receive_transition_digest_v1,
        offline_cash_send_split_openings_v1, offline_cash_send_split_seed_v1,
        offline_cash_state_lineage_digest_v1, receive_opening_message_v1,
        receive_semantic_message_v1, receive_transition_message_v1, send_split_branch_message_v1,
        send_split_seed_message_v1, state_lineage_message_v1, OfflineCashStatePrivateWitnessV1,
        BALANCE_HEAD_MESSAGE_BYTES_V1, CREDIT_HEAD_MESSAGE_BYTES_V1,
        RECEIVE_OPENING_MESSAGE_BYTES_V1, RECEIVE_SEMANTIC_MESSAGE_BYTES_V1,
        RECEIVE_TRANSITION_MESSAGE_BYTES_V1, SEND_SPLIT_RECEIVER_BRANCH_MESSAGE_BYTES_V1,
        SEND_SPLIT_RECEIVER_BRANCH_V1, SEND_SPLIT_SEED_MESSAGE_BYTES_V1,
        SEND_SPLIT_SENDER_BRANCH_MESSAGE_BYTES_V1, SEND_SPLIT_SENDER_BRANCH_V1,
        STATE_HEAD_FRAME_VERSION_V1, STATE_LINEAGE_MESSAGE_BYTES_V1,
    },
    state_transition::{OfflineCashStateContextV1, ReceiveFoldOutputV1},
    OfflineCashHalo2ParityV1,
};
use crate::zk::pasta_ipa_recursion::{
    pasta_ipa_augmented_proof_shape_v1, PastaIpaInstanceQueryV1, PastaIpaProofShapeV1,
};

fn configured_state_shape<F, C>(instance_query: PastaIpaInstanceQueryV1) -> PastaIpaProofShapeV1
where
    F: PrimeField,
    C: Circuit<F>,
{
    let mut constraints = ConstraintSystem::<F>::default();
    let _ = C::configure(&mut constraints);
    pasta_ipa_augmented_proof_shape_v1(&constraints, OFFLINE_CASH_HALO2_K_V1, instance_query)
        .expect("configured STATE proof shape")
}

fn canonical_eq_history() -> Vec<u8> {
    let history = super::halo2_primitives::test_support::history_from_eq_parts(
        std::array::from_fn(|index| Fp::from((index + 1) as u64)),
        EqAffine::generator(),
    )
    .expect("canonical Eq history");
    super::halo2_primitives::test_support::encode_history(&history).to_vec()
}

fn canonical_ep_history() -> Vec<u8> {
    let history = super::halo2_primitives::test_support::history_from_ep_parts(
        std::array::from_fn(|index| Fq::from((index + 1) as u64)),
        EpAffine::generator(),
    )
    .expect("canonical Ep history");
    super::halo2_primitives::test_support::encode_history(&history).to_vec()
}

fn send_fixture(
    parity: OfflineCashHalo2ParityV1,
) -> (
    OfflineCashStatePublicInstancesV1,
    OfflineCashStatePrivateWitnessV1,
) {
    send_fixture_at_sequence(parity, 7)
}

fn send_fixture_at_sequence(
    parity: OfflineCashHalo2ParityV1,
    guard_sequence: u64,
) -> (
    OfflineCashStatePublicInstancesV1,
    OfflineCashStatePrivateWitnessV1,
) {
    let release = super::terminal_tests::authenticated_release();
    let request = super::terminal_tests::request(&release);
    let mut statement = super::terminal_tests::payment(&release, &request).statement;
    let context = OfflineCashStateContextV1::new(
        statement.release_id,
        statement.network_id.clone(),
        statement.asset.clone(),
        statement.scale,
    )
    .expect("matching STATE context");
    let before_amount = 19_001;
    let after_amount = 10_000;
    let before_opening = [0x41; 32];
    let wallet_binding = [0x44; 32];
    let guard_device_id = [0x45; 32];
    let hardware_policy_id = [0x46; 32];
    let lineage_digest = [0x47; 32];
    statement.sender_before = offline_cash_balance_head_v1(
        &context.digest(),
        &wallet_binding,
        &guard_device_id,
        &hardware_policy_id,
        guard_sequence,
        &lineage_digest,
        before_amount,
        &before_opening,
    );
    let split_seed = offline_cash_send_split_seed_v1(
        &context.digest(),
        &wallet_binding,
        &statement.sender_before,
        &before_opening,
        guard_sequence,
        &statement.request_digest,
        &statement.receiver_before,
        &request.recipient_key_reference,
        statement.amount,
    );
    let (after_opening, credit_opening) = offline_cash_send_split_openings_v1(&split_seed);
    statement.credit_commitment = offline_cash_credit_head_v1(
        &context.digest(),
        &statement.request_digest,
        &statement.receiver_before,
        &request.recipient_key_reference,
        statement.amount,
        &credit_opening,
    );
    let next_lineage_digest = offline_cash_state_lineage_digest_v1(
        OfflineCashStateOperationV1::SendSplit,
        &context.digest(),
        &statement.sender_before,
        &lineage_digest,
        guard_sequence,
        guard_sequence.wrapping_add(1),
        &statement.request_digest,
        &statement.receiver_before,
        &statement.credit_commitment,
        statement.amount,
    );
    statement.sender_after = offline_cash_balance_head_v1(
        &context.digest(),
        &wallet_binding,
        &guard_device_id,
        &hardware_policy_id,
        guard_sequence.wrapping_add(1),
        &next_lineage_digest,
        after_amount,
        &after_opening,
    );
    statement.transition_digest = [0; 32];
    statement = statement.seal_transition().expect("seal relation fixture");
    let history = match parity {
        OfflineCashHalo2ParityV1::Eq => canonical_eq_history(),
        OfflineCashHalo2ParityV1::Ep => canonical_ep_history(),
    };
    let instances =
        OfflineCashStatePublicInstancesV1::send_split(&context, &statement, parity, &history)
            .expect("canonical send STATE ABI");
    let witness = OfflineCashStatePrivateWitnessV1::send_split(
        before_amount,
        after_amount,
        before_opening,
        *after_opening,
        *credit_opening,
        wallet_binding,
        guard_device_id,
        hardware_policy_id,
        guard_sequence,
        lineage_digest,
        next_lineage_digest,
        *split_seed,
        request.recipient_key_reference,
    )
    .expect("canonical send witness");
    (instances, witness)
}

fn send_instances(parity: OfflineCashHalo2ParityV1) -> OfflineCashStatePublicInstancesV1 {
    send_fixture(parity).0
}

fn receive_fixture(
    parity: OfflineCashHalo2ParityV1,
) -> (
    OfflineCashStatePublicInstancesV1,
    OfflineCashStatePrivateWitnessV1,
) {
    let context_digest = [0x12; 32];
    let request_digest = [0x13; 32];
    let transfer = 9_001;
    let before_amount = 10_000;
    let after_amount = 19_001;
    let before_opening = [0x51; 32];
    let credit_opening = [0x53; 32];
    let wallet_binding = [0x54; 32];
    let guard_device_id = [0x55; 32];
    let hardware_policy_id = [0x56; 32];
    let recipient_key_reference = [0x57; 32];
    let guard_sequence = 9;
    let lineage_digest = [0x58; 32];
    let balance_parent = offline_cash_balance_head_v1(
        &context_digest,
        &wallet_binding,
        &guard_device_id,
        &hardware_policy_id,
        guard_sequence,
        &lineage_digest,
        before_amount,
        &before_opening,
    );
    let credit_parent = offline_cash_credit_head_v1(
        &context_digest,
        &request_digest,
        &balance_parent,
        &recipient_key_reference,
        transfer,
        &credit_opening,
    );
    let send_transition_digest = [0x17; 32];
    let after_opening = offline_cash_receive_opening_v1(
        &context_digest,
        &before_opening,
        &credit_opening,
        &request_digest,
        &send_transition_digest,
        transfer,
    );
    let next_lineage_digest = offline_cash_state_lineage_digest_v1(
        OfflineCashStateOperationV1::ReceiveFold,
        &context_digest,
        &balance_parent,
        &lineage_digest,
        guard_sequence,
        guard_sequence + 1,
        &request_digest,
        &credit_parent,
        &send_transition_digest,
        transfer,
    );
    let next_head = offline_cash_balance_head_v1(
        &context_digest,
        &wallet_binding,
        &guard_device_id,
        &hardware_policy_id,
        guard_sequence + 1,
        &next_lineage_digest,
        after_amount,
        &after_opening,
    );
    let receive_transition_digest = offline_cash_receive_transition_digest_v1(
        &context_digest,
        &balance_parent,
        &credit_parent,
        &request_digest,
        &send_transition_digest,
        transfer,
        after_amount,
        &next_head,
    );
    let output = ReceiveFoldOutputV1 {
        release_id: [0x11; 32],
        context_digest,
        request_digest,
        payment_digest: [0x99; 32],
        amount: transfer,
        scale: 4,
        balance_parent,
        credit_parent,
        next_head,
        send_transition_digest,
        receive_transition_digest,
    };
    let history = match parity {
        OfflineCashHalo2ParityV1::Eq => canonical_eq_history(),
        OfflineCashHalo2ParityV1::Ep => canonical_ep_history(),
    };
    let instances = OfflineCashStatePublicInstancesV1::receive_fold(&output, parity, &history)
        .expect("canonical receive STATE ABI");
    let witness = OfflineCashStatePrivateWitnessV1::receive_fold(
        before_amount,
        after_amount,
        before_opening,
        *after_opening,
        credit_opening,
        wallet_binding,
        guard_device_id,
        hardware_policy_id,
        guard_sequence,
        lineage_digest,
        next_lineage_digest,
        recipient_key_reference,
    )
    .expect("canonical receive witness");
    (instances, witness)
}

fn receive_output() -> ReceiveFoldOutputV1 {
    let (instances, _witness) = receive_fixture(OfflineCashHalo2ParityV1::Eq);
    let public = instances
        .relation_public()
        .expect("receive public relation");
    ReceiveFoldOutputV1 {
        release_id: public.release_id,
        context_digest: public.context_digest,
        request_digest: public.request_digest,
        payment_digest: [0x99; 32],
        amount: public.transfer,
        scale: public.scale,
        balance_parent: public.parent_0,
        credit_parent: public.parent_1,
        next_head: public.result,
        send_transition_digest: public.link,
        receive_transition_digest: public.transition_digest,
    }
}

fn packed_fields_from_words<F: PrimeField>(words: &[u32; STATE_ABI_WORDS]) -> Vec<F> {
    (0..STATE_INSTANCE_CELLS)
        .map(|cell_index| {
            let start = cell_index * STATE_WORDS_PER_INSTANCE;
            let end = (start + STATE_WORDS_PER_INSTANCE).min(STATE_ABI_WORDS);
            pack_words_as_field::<F>(&words[start..end])
        })
        .collect()
}

#[test]
fn state_abi_is_229_words_packed_into_33_of_at_most_50_cells() {
    assert_eq!(STATE_ABI_WORDS, 229);
    assert_eq!(STATE_WORDS_PER_INSTANCE, 7);
    assert_eq!(STATE_INSTANCE_CELLS, 33);
    assert_eq!(STATE_INSTANCE_CELLS_MAX, 50);
    assert!(STATE_INSTANCE_CELLS <= STATE_INSTANCE_CELLS_MAX);
    assert!(Fp::CAPACITY >= 224);
    assert!(Fq::CAPACITY >= 224);

    let eq = send_instances(OfflineCashHalo2ParityV1::Eq);
    let packed_bytes = eq.packed_cell_bytes();
    assert_eq!(
        OfflineCashStatePublicInstancesV1::unpack_cell_bytes(&packed_bytes),
        Ok(*eq.words())
    );
    for (cell_index, expected) in packed_bytes.iter().enumerate() {
        let start = cell_index * STATE_WORDS_PER_INSTANCE;
        let end = (start + STATE_WORDS_PER_INSTANCE).min(STATE_ABI_WORDS);
        let fp = pack_words_as_field::<Fp>(&eq.words()[start..end]).to_repr();
        let fq = pack_words_as_field::<Fq>(&eq.words()[start..end]).to_repr();
        assert_eq!(&fp.as_ref()[..28], expected);
        assert_eq!(&fq.as_ref()[..28], expected);
        assert!(fp.as_ref()[28..].iter().all(|byte| *byte == 0));
        assert!(fq.as_ref()[28..].iter().all(|byte| *byte == 0));
        assert_eq!(&fp.as_ref()[..], &fq.as_ref()[..]);
    }

    let last = packed_bytes.last().expect("last packed cell");
    assert!(last[20..].iter().all(|byte| *byte == 0));
    let mut noncanonical = packed_bytes;
    noncanonical[STATE_INSTANCE_CELLS - 1][20] = 1;
    assert_eq!(
        OfflineCashStatePublicInstancesV1::unpack_cell_bytes(&noncanonical),
        Err(OfflineCashStateAbiErrorV1::NonCanonicalPacking)
    );
    assert_eq!(
        OfflineCashStatePublicInstancesV1::unpack_cell_bytes(
            &packed_bytes[..STATE_INSTANCE_CELLS - 1]
        ),
        Err(OfflineCashStateAbiErrorV1::NonCanonicalPacking)
    );
}

#[test]
fn eq_and_ep_share_semantics_but_pin_parity_protocol_and_history() {
    let eq = send_instances(OfflineCashHalo2ParityV1::Eq);
    let ep = send_instances(OfflineCashHalo2ParityV1::Ep);
    let parity_word = 3;
    let protocol_words = 16..24;
    let history_words = 93..STATE_ABI_WORDS;
    for index in 0..STATE_ABI_WORDS {
        if index == parity_word || protocol_words.contains(&index) || history_words.contains(&index)
        {
            continue;
        }
        assert_eq!(
            eq.words()[index],
            ep.words()[index],
            "semantic word {index}"
        );
    }
    assert_ne!(eq.words()[parity_word], ep.words()[parity_word]);
    assert_ne!(
        &eq.words()[protocol_words.clone()],
        &ep.words()[protocol_words]
    );
    assert_ne!(
        &eq.words()[history_words.clone()],
        &ep.words()[history_words]
    );

    let output = receive_output();
    let eq_receive = OfflineCashStatePublicInstancesV1::receive_fold(
        &output,
        OfflineCashHalo2ParityV1::Eq,
        &canonical_eq_history(),
    )
    .expect("Eq receive STATE ABI");
    let ep_receive = OfflineCashStatePublicInstancesV1::receive_fold(
        &output,
        OfflineCashHalo2ParityV1::Ep,
        &canonical_ep_history(),
    )
    .expect("Ep receive STATE ABI");
    assert_eq!(
        eq_receive.operation(),
        Ok(OfflineCashStateOperationV1::ReceiveFold)
    );
    assert_eq!(
        ep_receive.operation(),
        Ok(OfflineCashStateOperationV1::ReceiveFold)
    );
    assert_eq!(&eq_receive.words()[24..93], &ep_receive.words()[24..93]);
}

#[test]
fn strict_constructors_reject_context_history_receive_and_parity_substitution() {
    let release = super::terminal_tests::authenticated_release();
    let request = super::terminal_tests::request(&release);
    let payment = super::terminal_tests::payment(&release, &request);
    let wrong_context = OfflineCashStateContextV1::new(
        [0xA5; 32],
        payment.statement.network_id.clone(),
        payment.statement.asset.clone(),
        payment.statement.scale,
    )
    .expect("different valid context");
    assert_eq!(
        OfflineCashStatePublicInstancesV1::send_split(
            &wrong_context,
            &payment.statement,
            OfflineCashHalo2ParityV1::Eq,
            &canonical_eq_history(),
        ),
        Err(OfflineCashStateAbiErrorV1::ContextMismatch)
    );
    let context = OfflineCashStateContextV1::new(
        payment.statement.release_id,
        payment.statement.network_id.clone(),
        payment.statement.asset.clone(),
        payment.statement.scale,
    )
    .expect("matching context");
    assert_eq!(
        OfflineCashStatePublicInstancesV1::send_split(
            &context,
            &payment.statement,
            OfflineCashHalo2ParityV1::Eq,
            &[0; 544],
        ),
        Err(OfflineCashStateAbiErrorV1::InvalidHistory)
    );
    let mut invalid_receive = receive_output();
    invalid_receive.next_head = invalid_receive.balance_parent;
    assert_eq!(
        OfflineCashStatePublicInstancesV1::receive_fold(
            &invalid_receive,
            OfflineCashHalo2ParityV1::Ep,
            &canonical_ep_history(),
        ),
        Err(OfflineCashStateAbiErrorV1::InvalidReceiveOutput)
    );
    let (ep, ep_witness) = receive_fixture(OfflineCashHalo2ParityV1::Ep);
    assert!(matches!(
        OfflineCashEqStateCircuitV1::new(ep, ep_witness),
        Err(OfflineCashStateAbiErrorV1::ParityMismatch)
    ));
}

#[test]
fn both_fixed_k16_public_binding_circuits_accept_exact_instances() {
    let (eq, eq_witness) = send_fixture(OfflineCashHalo2ParityV1::Eq);
    let eq_public = eq.field_instances::<Fp>().to_vec();
    let eq_circuit = OfflineCashEqStateCircuitV1::new(eq, eq_witness).expect("Eq circuit");
    MockProver::run(OFFLINE_CASH_HALO2_K_V1, &eq_circuit, vec![eq_public])
        .expect("Eq STATE synthesis")
        .assert_satisfied();

    let (ep, ep_witness) = receive_fixture(OfflineCashHalo2ParityV1::Ep);
    let ep_public = ep.field_instances::<Fq>().to_vec();
    let ep_circuit = OfflineCashEpStateCircuitV1::new(ep, ep_witness).expect("Ep circuit");
    MockProver::run(OFFLINE_CASH_HALO2_K_V1, &ep_circuit, vec![ep_public])
        .expect("Ep STATE synthesis")
        .assert_satisfied();
}

#[test]
fn circuit_rejects_closed_operation_parent_substitution_padding_and_224_bit_overflow() {
    let (eq, invalid_operation_witness) = send_fixture(OfflineCashHalo2ParityV1::Eq);
    let mut invalid_operation_words = *eq.words();
    invalid_operation_words[STATE_OPERATION_WORD] = 3;
    let invalid_operation_public = packed_fields_from_words::<Fp>(&invalid_operation_words);
    let invalid_operation_circuit = OfflineCashEqStateCircuitV1::from_words_for_test(
        invalid_operation_words,
        invalid_operation_witness,
    );
    assert!(MockProver::run(
        OFFLINE_CASH_HALO2_K_V1,
        &invalid_operation_circuit,
        vec![invalid_operation_public],
    )
    .expect("invalid-operation synthesis")
    .verify()
    .is_err());

    let (eq, parent_witness) = send_fixture(OfflineCashHalo2ParityV1::Eq);
    let valid_public = eq.field_instances::<Fp>().to_vec();
    let circuit = OfflineCashEqStateCircuitV1::new(eq, parent_witness).expect("Eq circuit");

    let mut substituted_parent = valid_public.clone();
    let parent_lane = 48 % STATE_WORDS_PER_INSTANCE;
    let parent_coefficient =
        (0..parent_lane).fold(Fp::ONE, |value, _| value * Fp::from(1_u64 << 32));
    substituted_parent[48 / STATE_WORDS_PER_INSTANCE] += parent_coefficient;
    assert!(
        MockProver::run(OFFLINE_CASH_HALO2_K_V1, &circuit, vec![substituted_parent],)
            .expect("parent-substitution synthesis")
            .verify()
            .is_err()
    );

    let (eq, padding_witness) = send_fixture(OfflineCashHalo2ParityV1::Eq);
    let padding_circuit =
        OfflineCashEqStateCircuitV1::new(eq, padding_witness).expect("Eq circuit");
    let mut nonzero_padding = valid_public.clone();
    let padding_coefficient = (0..5).fold(Fp::ONE, |value, _| value * Fp::from(1_u64 << 32));
    nonzero_padding[STATE_INSTANCE_CELLS - 1] += padding_coefficient;
    assert!(MockProver::run(
        OFFLINE_CASH_HALO2_K_V1,
        &padding_circuit,
        vec![nonzero_padding],
    )
    .expect("padding-substitution synthesis")
    .verify()
    .is_err());

    let (eq, overflow_witness) = send_fixture(OfflineCashHalo2ParityV1::Eq);
    let overflow_circuit =
        OfflineCashEqStateCircuitV1::new(eq, overflow_witness).expect("Eq circuit");
    let mut overflow = valid_public;
    overflow[0] = -Fp::ONE;
    assert!(
        MockProver::run(OFFLINE_CASH_HALO2_K_V1, &overflow_circuit, vec![overflow])
            .expect("224-bit-overflow synthesis")
            .verify()
            .is_err()
    );
}

#[test]
fn state_shapes_exceed_the_recursive_activation_cap_for_both_parities() {
    let eq =
        configured_state_shape::<Fp, OfflineCashEqStateCircuitV1>(PastaIpaInstanceQueryV1::Direct);
    let ep =
        configured_state_shape::<Fq, OfflineCashEpStateCircuitV1>(PastaIpaInstanceQueryV1::Direct);
    assert_eq!(eq, ep);
    assert_eq!(eq.k(), OFFLINE_CASH_HALO2_K_V1);
    assert_eq!(eq.instance_columns(), 1);
    assert_eq!(eq.instance_queries(), 1);
    assert!(eq.permutation_columns() > 13);
    assert!(eq.augmented_proof_bytes() > 3_200);

    for (parity, shape) in [
        (OfflineCashHalo2ParityV1::Eq, &eq),
        (OfflineCashHalo2ParityV1::Ep, &ep),
    ] {
        let error = preflight_offline_cash_recursion_activation_v1(
            parity,
            OfflineCashHalo2CircuitRoleV1::State,
            shape,
        )
        .expect_err("the current STATE binding shape must remain non-activating");
        assert!(matches!(
            error,
            OfflineCashRecursionActivationPreflightErrorV1::ProofSizeExceeded {
                parity: actual_parity,
                circuit_role: OfflineCashHalo2CircuitRoleV1::State,
                actual,
                maximum: 3_200,
            } if actual_parity == parity && actual == shape.augmented_proof_bytes()
        ));
    }

    let queried =
        configured_state_shape::<Fp, OfflineCashEqStateCircuitV1>(PastaIpaInstanceQueryV1::Queried);
    assert_eq!(
        queried.augmented_proof_bytes(),
        eq.augmented_proof_bytes() + 32
    );
}

#[test]
fn production_backend_remains_disconnected_and_fail_closed() {
    let backend = include_str!("halo2_backend.rs");
    assert!(backend.contains("VerificationUnavailable"));
    assert!(!backend.contains("OfflineCashEqStateCircuitV1"));
    assert!(!backend.contains("OfflineCashEpStateCircuitV1"));
    assert!(!backend.contains("state_circuit"));
    assert!(backend.contains("FULL_STATE_TYPED_PUBLIC_INSTANCES_REQUIRED_BEFORE_ACTIVATION"));
    assert!(backend.contains("SEND_SPLIT_STATE_RELATION_REQUIRED_BEFORE_ACTIVATION"));

    let circuit = include_str!("state_circuit.rs");
    assert!(!circuit.contains("verify_proof"));
    assert!(!circuit.contains("create_proof"));
    assert!(circuit.contains("Recursive parents, helper proofs, and canonical `SendSplit`"));
    assert_eq!(circuit.matches("type FloorPlanner = V1;").count(), 2);
    assert_eq!(circuit.matches("fn synthesize_for_measurement(").count(), 2);
    assert_eq!(
        circuit
            .matches("Self::default().synthesize(config, layouter)")
            .count(),
        2
    );

    let relation = include_str!("state_relation.rs");
    let witness_start = relation
        .find("pub(super) struct OfflineCashStatePrivateWitnessV1")
        .expect("private witness declaration");
    let witness_prefix = &relation[witness_start.saturating_sub(80)..witness_start];
    assert!(!witness_prefix.contains("Clone"));
    assert!(relation.contains("impl Drop for OfflineCashStatePrivateWitnessV1"));
    assert!(relation.matches(".zeroize();").count() >= 13);
    assert!(relation.contains("Canonical Norito transition/semantic reconstruction and recursive"));
    let protocol = include_str!("protocol.rs");
    assert!(protocol.contains("axiom-floorplanner-v1-two-pass+witnessless-measurement"));
    assert!(protocol.contains("jobs-ordered(6,6,5,6,6,2,2,5,6,7)-blocks"));
    assert!(protocol.contains("nonzero-9x32byte-private-bindings+send-seed-op1"));
    assert!(protocol.contains("send-seed+branch-openings-op1-gated"));
    assert!(protocol.contains("send-transition+semantic-deferred"));

    let relation_circuit = include_str!("state_relation_circuit.rs");
    assert_eq!(
        relation_circuit
            .matches("bind_receive_sha_digest_v1(")
            .count(),
        3
    );
    assert!(relation_circuit.contains("fn bind_receive_sha_digest_v1<F: PrimeField>("));
    assert!(relation_circuit.contains("enabled * receive * (word - reconstructed)"));
    assert_eq!(
        relation_circuit.matches("bind_send_sha_digest_v1(").count(),
        3
    );
    assert!(relation_circuit.contains("fn bind_send_sha_digest_v1<F: PrimeField>("));
    assert!(relation_circuit.contains("enabled * send * (word - reconstructed)"));
    assert!(relation_circuit.contains("nonzero SendSplit private binding terminal"));
    assert!(relation_circuit.contains("enabled * send * (sum * inverse"));
    assert!(relation_circuit.contains("OfflineCashStateShaByteV1::constrained"));
    assert!(!relation_circuit.contains("verify_proof"));
}

#[test]
fn canonical_frames_pin_lengths_version_and_sha_byte_order() {
    let balance = balance_head_message_v1(
        &[1; 32], &[2; 32], &[3; 32], &[4; 32], 7, &[5; 32], 0x1122, &[6; 32],
    );
    let credit = credit_head_message_v1(&[1; 32], &[2; 32], &[3; 32], &[4; 32], 0x1122, &[5; 32]);
    assert_eq!(balance.len(), BALANCE_HEAD_MESSAGE_BYTES_V1);
    assert_eq!(credit.len(), CREDIT_HEAD_MESSAGE_BYTES_V1);
    assert_eq!(balance.len(), 335);
    assert_eq!(credit.len(), 278);
    let balance_domain_len = u64::from_le_bytes(balance[..8].try_into().expect("length"));
    let version_offset = 8 + usize::try_from(balance_domain_len).expect("domain length");
    assert_eq!(
        &balance[version_offset..version_offset + 8],
        &2_u64.to_le_bytes()
    );
    assert_eq!(
        &balance[version_offset + 8..version_offset + 10],
        &STATE_HEAD_FRAME_VERSION_V1.to_le_bytes()
    );
    let balance_digest: [u8; 32] = sha2::Sha256::digest(balance.as_slice()).into();
    let credit_digest: [u8; 32] = sha2::Sha256::digest(credit.as_slice()).into();
    assert_eq!(
        offline_cash_balance_head_v1(
            &[1; 32], &[2; 32], &[3; 32], &[4; 32], 7, &[5; 32], 0x1122, &[6; 32],
        ),
        balance_digest
    );
    assert_eq!(
        offline_cash_credit_head_v1(&[1; 32], &[2; 32], &[3; 32], &[4; 32], 0x1122, &[5; 32]),
        credit_digest
    );

    let split_seed_message = send_split_seed_message_v1(
        &[1; 32], &[2; 32], &[3; 32], &[4; 32], 7, &[5; 32], &[6; 32], &[7; 32], 0x1122,
    );
    assert_eq!(split_seed_message.len(), SEND_SPLIT_SEED_MESSAGE_BYTES_V1);
    let split_seed: [u8; 32] = sha2::Sha256::digest(split_seed_message.as_slice()).into();
    let sender_branch = send_split_branch_message_v1(&split_seed, SEND_SPLIT_SENDER_BRANCH_V1);
    let receiver_branch = send_split_branch_message_v1(&split_seed, SEND_SPLIT_RECEIVER_BRANCH_V1);
    assert_eq!(
        sender_branch.len(),
        SEND_SPLIT_SENDER_BRANCH_MESSAGE_BYTES_V1
    );
    assert_eq!(
        receiver_branch.len(),
        SEND_SPLIT_RECEIVER_BRANCH_MESSAGE_BYTES_V1
    );
    assert_eq!(
        hex::encode(split_seed),
        "be98802d9f45e7c0a1470f611185d8907b525d3b6e4159db881a78d7e41fd831"
    );
    assert_eq!(
        hex::encode(sha2::Sha256::digest(sender_branch.as_slice())),
        "963a6c85fb7c86521d2e47356d31aee1b09de55611b9e6b4271263ee9f78ce75"
    );
    assert_eq!(
        hex::encode(sha2::Sha256::digest(receiver_branch.as_slice())),
        "caa36422b5b03f7e3c4b9fe8c250a5e0e73415d6a00d27b2678e25fbf8f331fa"
    );
    assert_eq!(
        *offline_cash_send_split_seed_v1(
            &[1; 32], &[2; 32], &[3; 32], &[4; 32], 7, &[5; 32], &[6; 32], &[7; 32], 0x1122,
        ),
        split_seed
    );
    let (sender_opening, receiver_opening) = offline_cash_send_split_openings_v1(&split_seed);
    assert_eq!(
        hex::encode(*sender_opening),
        "963a6c85fb7c86521d2e47356d31aee1b09de55611b9e6b4271263ee9f78ce75"
    );
    assert_eq!(
        hex::encode(*receiver_opening),
        "caa36422b5b03f7e3c4b9fe8c250a5e0e73415d6a00d27b2678e25fbf8f331fa"
    );

    let receive_opening =
        receive_opening_message_v1(&[1; 32], &[2; 32], &[3; 32], &[4; 32], &[5; 32], 0x1122);
    let receive_transition = receive_transition_message_v1(
        &[1; 32], &[2; 32], &[3; 32], &[4; 32], &[5; 32], 0x1122, 0x3344, &[6; 32],
    );
    let receive_semantic = receive_semantic_message_v1(
        &[1; 32], &[2; 32], &[3; 32], &[4; 32], &[5; 32], &[6; 32], &[7; 32], &[8; 32], 0x1122, 4,
    );
    assert_eq!(receive_opening.len(), RECEIVE_OPENING_MESSAGE_BYTES_V1);
    assert_eq!(
        receive_transition.len(),
        RECEIVE_TRANSITION_MESSAGE_BYTES_V1
    );
    assert_eq!(receive_semantic.len(), RECEIVE_SEMANTIC_MESSAGE_BYTES_V1);
    let lineage = state_lineage_message_v1(
        OfflineCashStateOperationV1::SendSplit,
        &[1; 32],
        &[2; 32],
        &[3; 32],
        7,
        8,
        &[4; 32],
        &[5; 32],
        &[6; 32],
        0x1122,
    );
    assert_eq!(lineage.len(), STATE_LINEAGE_MESSAGE_BYTES_V1);
    assert_eq!(lineage.len(), 361);
    assert_eq!(receive_opening.len(), 273);
    assert_eq!(receive_transition.len(), 341);
    assert_eq!(receive_semantic.len(), 430);
    let semantic_domain_len = u64::from_le_bytes(receive_semantic[..8].try_into().expect("length"));
    let semantic_version_offset = 8 + usize::try_from(semantic_domain_len).expect("domain length");
    assert_eq!(
        &receive_semantic[semantic_version_offset..semantic_version_offset + 8],
        &2_u64.to_le_bytes()
    );
    assert_eq!(
        &receive_semantic[semantic_version_offset + 8..semantic_version_offset + 10],
        &STATE_HEAD_FRAME_VERSION_V1.to_le_bytes()
    );
    let receive_opening_digest: [u8; 32] = sha2::Sha256::digest(receive_opening.as_slice()).into();
    let receive_transition_digest: [u8; 32] =
        sha2::Sha256::digest(receive_transition.as_slice()).into();
    let receive_semantic_digest: [u8; 32] =
        sha2::Sha256::digest(receive_semantic.as_slice()).into();
    assert_eq!(
        hex::encode(receive_opening_digest),
        "06cb58b5d3585500ee3af873c28ae9ea74e593cd51c191e425af5a1ed826cd22"
    );
    assert_eq!(
        hex::encode(receive_transition_digest),
        "ff4f39c6e0b7f8a291bfdebe964d86685219763485acb1147248becdba0f8a04"
    );
    assert_eq!(
        hex::encode(receive_semantic_digest),
        "6d2be4a1992be479531a35fd11047114b14af3aa78d96eb5ba2281091f5aa007"
    );
    assert_eq!(
        *offline_cash_receive_opening_v1(&[1; 32], &[2; 32], &[3; 32], &[4; 32], &[5; 32], 0x1122,),
        receive_opening_digest
    );
    assert_eq!(
        offline_cash_receive_transition_digest_v1(
            &[1; 32], &[2; 32], &[3; 32], &[4; 32], &[5; 32], 0x1122, 0x3344, &[6; 32],
        ),
        receive_transition_digest
    );
    assert_eq!(
        offline_cash_receive_semantic_digest_v1(
            &[1; 32], &[2; 32], &[3; 32], &[4; 32], &[5; 32], &[6; 32], &[7; 32], &[8; 32], 0x1122,
            4,
        ),
        receive_semantic_digest
    );
}

#[test]
fn private_witness_host_gate_rejects_conservation_and_all_three_head_substitutions() {
    let (instances, mut witness) = send_fixture(OfflineCashHalo2ParityV1::Eq);
    witness.corrupt_after_amount_for_test();
    assert_eq!(
        witness.validate_against(&instances),
        Err(OfflineCashStateAbiErrorV1::InvalidPrivateWitness)
    );

    let (instances, mut witness) = send_fixture(OfflineCashHalo2ParityV1::Eq);
    witness.corrupt_before_opening_for_test();
    assert_eq!(
        witness.validate_against(&instances),
        Err(OfflineCashStateAbiErrorV1::InvalidPrivateWitness)
    );

    let (instances, mut witness) = receive_fixture(OfflineCashHalo2ParityV1::Ep);
    witness.corrupt_credit_opening_for_test();
    assert_eq!(
        witness.validate_against(&instances),
        Err(OfflineCashStateAbiErrorV1::InvalidPrivateWitness)
    );

    for rejected_field in 0..10 {
        let (instances, mut witness) = send_fixture(OfflineCashHalo2ParityV1::Eq);
        witness.zero_rejected_field_for_test(rejected_field);
        assert_eq!(
            witness.validate_against(&instances),
            Err(OfflineCashStateAbiErrorV1::InvalidPrivateWitness),
            "zero private field {rejected_field}"
        );
    }

    let host_mutations: [(&str, fn(&mut OfflineCashStatePrivateWitnessV1)); 4] = [
        (
            "guard sequence",
            OfflineCashStatePrivateWitnessV1::corrupt_guard_sequence_for_test,
        ),
        (
            "current lineage",
            OfflineCashStatePrivateWitnessV1::corrupt_lineage_for_test,
        ),
        (
            "successor lineage",
            OfflineCashStatePrivateWitnessV1::corrupt_next_lineage_for_test,
        ),
        (
            "split seed",
            OfflineCashStatePrivateWitnessV1::corrupt_send_split_seed_for_test,
        ),
    ];
    for (label, mutate) in host_mutations {
        let (instances, mut witness) = send_fixture(OfflineCashHalo2ParityV1::Eq);
        mutate(&mut witness);
        assert_eq!(
            witness.validate_against(&instances),
            Err(OfflineCashStateAbiErrorV1::InvalidPrivateWitness),
            "accepted substituted {label}"
        );
    }
}

#[test]
fn circuit_relation_rejects_private_witness_substitution() {
    let circuit_mutations: [(&str, fn(&mut OfflineCashStatePrivateWitnessV1)); 5] = [
        (
            "amount",
            OfflineCashStatePrivateWitnessV1::corrupt_after_amount_for_test,
        ),
        (
            "guard sequence",
            OfflineCashStatePrivateWitnessV1::corrupt_guard_sequence_for_test,
        ),
        (
            "current lineage",
            OfflineCashStatePrivateWitnessV1::corrupt_lineage_for_test,
        ),
        (
            "successor lineage",
            OfflineCashStatePrivateWitnessV1::corrupt_next_lineage_for_test,
        ),
        (
            "split seed",
            OfflineCashStatePrivateWitnessV1::corrupt_send_split_seed_for_test,
        ),
    ];
    for (label, mutate) in circuit_mutations {
        let (instances, mut witness) = send_fixture(OfflineCashHalo2ParityV1::Eq);
        let public = instances.field_instances::<Fp>().to_vec();
        mutate(&mut witness);
        let circuit = OfflineCashEqStateCircuitV1::from_words_for_test(*instances.words(), witness);
        assert!(
            MockProver::run(OFFLINE_CASH_HALO2_K_V1, &circuit, vec![public])
                .unwrap_or_else(|error| panic!("{label} substitution synthesis: {error}"))
                .verify()
                .is_err(),
            "circuit accepted substituted {label}"
        );
    }
}

#[test]
fn exact_next_sequence_rejects_u64_overflow_in_host_and_circuit() {
    let (instances, witness) = send_fixture_at_sequence(OfflineCashHalo2ParityV1::Eq, u64::MAX);
    assert_eq!(
        witness.validate_against(&instances),
        Err(OfflineCashStateAbiErrorV1::InvalidPrivateWitness)
    );
    let public = instances.field_instances::<Fp>().to_vec();
    let circuit = OfflineCashEqStateCircuitV1::from_words_for_test(*instances.words(), witness);
    assert!(
        MockProver::run(OFFLINE_CASH_HALO2_K_V1, &circuit, vec![public])
            .expect("sequence-overflow synthesis")
            .verify()
            .is_err(),
        "circuit accepted u64::MAX exact-next predecessor"
    );
}

#[test]
fn receive_relation_rejects_each_public_preimage_and_operation_substitution() {
    for (label, word) in [
        ("operation", STATE_OPERATION_WORD),
        ("release", RELEASE_WORD_START),
        ("semantic", SEMANTIC_WORD_START),
        ("context", CONTEXT_WORD_START),
        ("request", REQUEST_WORD_START),
        ("parent-0", PARENT_0_WORD_START),
        ("parent-1", PARENT_1_WORD_START),
        ("result", RESULT_WORD_START),
        ("link", LINK_WORD_START),
        ("transition", TRANSITION_WORD_START),
        ("amount", AMOUNT_WORD_START),
        ("scale", SCALE_WORD),
    ] {
        let (instances, witness) = receive_fixture(OfflineCashHalo2ParityV1::Eq);
        let mut words = *instances.words();
        if word == STATE_OPERATION_WORD {
            words[word] = OfflineCashStateOperationV1::SendSplit as u32;
        } else {
            words[word] ^= 1;
        }
        let public = packed_fields_from_words::<Fp>(&words);
        let circuit = OfflineCashEqStateCircuitV1::from_words_for_test(words, witness);
        assert!(
            MockProver::run(OFFLINE_CASH_HALO2_K_V1, &circuit, vec![public])
                .unwrap_or_else(|error| panic!("{label} mutation synthesis failed: {error}"))
                .verify()
                .is_err(),
            "ReceiveFold accepted mutated {label}"
        );
    }
}

#[test]
fn receive_relation_rejects_each_private_opening_substitution() {
    for opening in 0..3 {
        let (instances, mut witness) = receive_fixture(OfflineCashHalo2ParityV1::Eq);
        match opening {
            0 => witness.corrupt_before_opening_for_test(),
            1 => witness.corrupt_after_opening_for_test(),
            2 => witness.corrupt_credit_opening_for_test(),
            _ => unreachable!(),
        }
        let public = instances.field_instances::<Fp>().to_vec();
        let circuit = OfflineCashEqStateCircuitV1::from_words_for_test(*instances.words(), witness);
        assert!(
            MockProver::run(OFFLINE_CASH_HALO2_K_V1, &circuit, vec![public])
                .expect("opening-substitution synthesis")
                .verify()
                .is_err(),
            "ReceiveFold accepted mutated opening {opening}"
        );
    }
}

#[test]
fn receive_opening_derivation_rejects_an_otherwise_self_consistent_successor() {
    let (base, _base_witness) = receive_fixture(OfflineCashHalo2ParityV1::Eq);
    let public = base.relation_public().expect("receive public relation");
    let alternate_after_opening = [0xA9; 32];
    let alternate_next_lineage = offline_cash_state_lineage_digest_v1(
        OfflineCashStateOperationV1::ReceiveFold,
        &public.context_digest,
        &public.parent_0,
        &[0x58; 32],
        9,
        10,
        &public.request_digest,
        &public.parent_1,
        &public.link,
        public.transfer,
    );
    let alternate_result = offline_cash_balance_head_v1(
        &public.context_digest,
        &[0x54; 32],
        &[0x55; 32],
        &[0x56; 32],
        10,
        &alternate_next_lineage,
        19_001,
        &alternate_after_opening,
    );
    let alternate_transition = offline_cash_receive_transition_digest_v1(
        &public.context_digest,
        &public.parent_0,
        &public.parent_1,
        &public.request_digest,
        &public.link,
        public.transfer,
        19_001,
        &alternate_result,
    );
    let output = ReceiveFoldOutputV1 {
        release_id: public.release_id,
        context_digest: public.context_digest,
        request_digest: public.request_digest,
        payment_digest: [0x99; 32],
        amount: public.transfer,
        scale: public.scale,
        balance_parent: public.parent_0,
        credit_parent: public.parent_1,
        next_head: alternate_result,
        send_transition_digest: public.link,
        receive_transition_digest: alternate_transition,
    };
    let instances = OfflineCashStatePublicInstancesV1::receive_fold(
        &output,
        OfflineCashHalo2ParityV1::Eq,
        &canonical_eq_history(),
    )
    .expect("self-consistent alternate receive public ABI");
    let witness = OfflineCashStatePrivateWitnessV1::receive_fold(
        10_000,
        19_001,
        [0x51; 32],
        alternate_after_opening,
        [0x53; 32],
        [0x54; 32],
        [0x55; 32],
        [0x56; 32],
        9,
        [0x58; 32],
        alternate_next_lineage,
        [0x57; 32],
    )
    .expect("alternate receive witness");
    let public_fields = instances.field_instances::<Fp>().to_vec();
    let circuit = OfflineCashEqStateCircuitV1::from_words_for_test(*instances.words(), witness);
    assert!(
        MockProver::run(OFFLINE_CASH_HALO2_K_V1, &circuit, vec![public_fields],)
            .expect("alternate-opening synthesis")
            .verify()
            .is_err(),
        "ReceiveFold accepted a prover-chosen successor opening"
    );
}

#[test]
fn circuit_relation_rejects_zero_private_bindings_and_openings() {
    let (instances, mut witness) = send_fixture(OfflineCashHalo2ParityV1::Eq);
    let public = instances.field_instances::<Fp>().to_vec();
    for rejected_field in 0..10 {
        witness.zero_rejected_field_for_test(rejected_field);
    }
    let circuit = OfflineCashEqStateCircuitV1::from_words_for_test(*instances.words(), witness);
    assert!(
        MockProver::run(OFFLINE_CASH_HALO2_K_V1, &circuit, vec![public])
            .expect("zero-binding adversarial synthesis")
            .verify()
            .is_err()
    );

    let relation_circuit = include_str!("state_relation_circuit.rs");
    for label in [
        "nonzero STATE wallet binding",
        "nonzero STATE guard device",
        "nonzero STATE hardware policy",
        "nonzero STATE current lineage",
        "nonzero STATE successor lineage",
        "nonzero STATE before opening",
        "nonzero STATE after opening",
        "nonzero STATE credit opening",
        "nonzero STATE recipient key reference",
    ] {
        assert!(relation_circuit.contains(label), "missing {label}");
    }
    assert!(relation_circuit.contains("STATE_NONZERO_BINDING_MAX_ROTATION_V1: i32 = 1"));
    assert!(relation_circuit.contains("STATE_NONZERO_BINDING_MAX_ROTATION_V1 <= 1"));
    let running_sum_gate = relation_circuit
        .split("offline cash STATE nonzero binding running sum")
        .nth(1)
        .and_then(|suffix| {
            suffix
                .split("offline cash STATE nonzero private binding terminal")
                .next()
        })
        .expect("bounded nonzero running-sum gate source");
    assert!(running_sum_gate.contains("Rotation::cur()"));
    assert!(running_sum_gate.contains("Rotation::next()"));
    assert!(!running_sum_gate.contains("Rotation(i32::try_from"));
    assert!(relation_circuit.contains("for (row, source) in words.iter().copied().enumerate()"));
    assert!(relation_circuit.contains("q_binding_sum_step.enable(&mut region, row)"));
    assert!(relation_circuit.contains(".map(|(sum, limb)| sum + limb)"));
    assert!(relation_circuit.contains("row + 1"));
    assert!(relation_circuit.contains(".enable(&mut region, DIGEST_WORDS)"));
}

#[test]
fn witnessless_circuit_keeps_exact_shape_without_private_values() {
    let (instances, witness) = send_fixture(OfflineCashHalo2ParityV1::Eq);
    let circuit = OfflineCashEqStateCircuitV1::new(instances, witness).expect("Eq circuit");
    assert!(circuit.has_witness_for_test());
    let witnessless = <OfflineCashEqStateCircuitV1 as Circuit<Fp>>::without_witnesses(&circuit);
    assert!(!witnessless.has_witness_for_test());

    let (instances, witness) = receive_fixture(OfflineCashHalo2ParityV1::Ep);
    let circuit = OfflineCashEpStateCircuitV1::new(instances, witness).expect("Ep circuit");
    assert!(circuit.has_witness_for_test());
    let witnessless = <OfflineCashEpStateCircuitV1 as Circuit<Fq>>::without_witnesses(&circuit);
    assert!(!witnessless.has_witness_for_test());
}
