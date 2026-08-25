use halo2_proofs::{
    dev::MockProver,
    halo2curves::{
        ff::{Field, PrimeField},
        pasta::{Fp, Fq},
    },
    plonk::{Circuit, ConstraintSystem},
};
use iroha_data_model::{
    NetworkId,
    asset::AssetDefinitionId,
    offline::{
        OFFLINE_CASH_HALO2_K_V1, OfflineCashRecursivePairBindingV1, OfflineCashTransferStatementV1,
    },
};
use sha2::Digest as _;

use super::{
    OfflineCashHalo2ParityV1,
    protocol::{
        OfflineCashHalo2CircuitRoleV1, OfflineCashRecursionActivationPreflightErrorV1,
        preflight_offline_cash_recursion_activation_v1,
    },
    state_abi::{
        AMOUNT_WORD_START, CONTEXT_WORD_START, LINK_WORD_START, OfflineCashStateAbiErrorV1,
        OfflineCashStateLeafPublicInstancesV1, OfflineCashStateOperationV1,
        OfflineCashStatePublicInstancesV1, PARENT_0_WORD_START, PARENT_1_WORD_START,
        RELEASE_WORD_START, REQUEST_WORD_START, RESULT_WORD_START, SCALE_WORD, SEMANTIC_WORD_START,
        STATE_ABI_WORDS, STATE_INSTANCE_CELLS, STATE_INSTANCE_CELLS_MAX, STATE_LEAF_ABI_WORDS,
        STATE_LEAF_INSTANCE_CELLS, STATE_OPERATION_WORD, STATE_WORDS_PER_INSTANCE,
        TRANSITION_WORD_START, pack_words_as_field,
    },
    state_circuit::{OfflineCashEpStateLeafCircuitV1, OfflineCashEqStateLeafCircuitV1},
    state_relation::{
        ASSET_ID_FRAME_BYTES_V1, BALANCE_HEAD_MESSAGE_BYTES_V1, CREDIT_HEAD_MESSAGE_BYTES_V1,
        NETWORK_ID_FRAME_BYTES_V1, OfflineCashStatePrivateWitnessV1,
        RECEIVE_OPENING_MESSAGE_BYTES_V1, RECEIVE_SEMANTIC_MESSAGE_BYTES_V1,
        RECEIVE_TRANSITION_MESSAGE_BYTES_V1, SEND_SEMANTIC_DIGEST_PREFIX_BYTES_V1,
        SEND_SEMANTIC_MESSAGE_BYTES_V1, SEND_SPLIT_RECEIVER_BRANCH_MESSAGE_BYTES_V1,
        SEND_SPLIT_RECEIVER_BRANCH_V1, SEND_SPLIT_SEED_MESSAGE_BYTES_V1,
        SEND_SPLIT_SENDER_BRANCH_MESSAGE_BYTES_V1, SEND_SPLIT_SENDER_BRANCH_V1,
        SEND_TRANSITION_MESSAGE_BYTES_V1, STATE_HEAD_FRAME_VERSION_V1,
        STATE_LINEAGE_MESSAGE_BYTES_V1, balance_head_message_v1, credit_head_message_v1,
        offline_cash_balance_head_v1, offline_cash_credit_head_v1, offline_cash_receive_opening_v1,
        offline_cash_receive_semantic_digest_v1, offline_cash_receive_transition_digest_v1,
        offline_cash_send_split_openings_v1, offline_cash_send_split_seed_v1,
        offline_cash_state_lineage_digest_v1, receive_opening_message_v1,
        receive_semantic_message_v1, receive_transition_message_v1, send_split_branch_message_v1,
        send_split_seed_message_v1, state_lineage_message_v1,
    },
    state_transition::{
        OfflineCashStateContextV1, ReceiveFoldOutputV1, STATE_CONTEXT_MESSAGE_BYTES_V1,
        state_context_message_v1,
    },
};
use crate::zk::pasta_ipa_recursion::{
    PastaIpaInstanceQueryV1, PastaIpaProofShapeV1, pasta_ipa_augmented_proof_shape_v1,
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

fn canonical_recursive_pair_binding() -> OfflineCashRecursivePairBindingV1 {
    OfflineCashRecursivePairBindingV1::new_state(
        [0xC3; 32],
        [0xD4; 32],
        &OfflineCashRecursivePairBindingV1::new_guard_bundle([0xA1; 32], [0xB2; 32])
            .expect("canonical GuardBundle pair binding"),
    )
    .expect("canonical recursive pair binding")
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
    let mut statement = super::terminal_tests::payment(&release, &request)
        .reconstruct_statement(&request)
        .expect("reconstructed statement");
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
    let recursive_pair_binding = canonical_recursive_pair_binding();
    let leaf = OfflineCashStateLeafPublicInstancesV1::send_split(&context, &statement, parity)
        .expect("canonical send StateLeaf ABI");
    let instances = OfflineCashStatePublicInstancesV1::from_leaf(leaf, &recursive_pair_binding)
        .expect("canonical final State ABI");
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
        &statement,
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
    let recursive_pair_binding = canonical_recursive_pair_binding();
    let leaf = OfflineCashStateLeafPublicInstancesV1::receive_fold(&output, parity)
        .expect("canonical receive StateLeaf ABI");
    let instances = OfflineCashStatePublicInstancesV1::from_leaf(leaf, &recursive_pair_binding)
        .expect("canonical final State ABI");
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

fn packed_leaf_fields_from_words<F: PrimeField>(words: &[u32; STATE_ABI_WORDS]) -> Vec<F> {
    (0..STATE_LEAF_INSTANCE_CELLS)
        .map(|cell_index| {
            let start = cell_index * STATE_WORDS_PER_INSTANCE;
            let end = (start + STATE_WORDS_PER_INSTANCE).min(STATE_LEAF_ABI_WORDS);
            pack_words_as_field::<F>(&words[start..end])
        })
        .collect()
}

#[test]
fn state_abi_is_229_words_packed_into_33_of_at_most_50_cells() {
    assert_eq!(STATE_ABI_WORDS, 229);
    assert_eq!(STATE_WORDS_PER_INSTANCE, 7);
    assert_eq!(STATE_INSTANCE_CELLS, 33);
    assert_eq!(STATE_LEAF_ABI_WORDS, 93);
    assert_eq!(STATE_LEAF_INSTANCE_CELLS, 14);
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
fn state_leaf_is_exactly_the_93_word_non_circular_semantic_prefix() {
    let full = send_instances(OfflineCashHalo2ParityV1::Eq);
    let leaf = full.state_leaf();
    assert_eq!(leaf.words(), &full.words()[..STATE_LEAF_ABI_WORDS]);
    assert_eq!(
        leaf.field_instances::<Fp>().len(),
        STATE_LEAF_INSTANCE_CELLS
    );

    let alternate_binding = OfflineCashRecursivePairBindingV1::new_state(
        [0x31; 32],
        [0x51; 32],
        &OfflineCashRecursivePairBindingV1::new_guard_bundle([0x32; 32], [0x52; 32])
            .expect("alternate GuardBundle pair binding"),
    )
    .expect("alternate State recursive pair binding");
    let alternate = OfflineCashStatePublicInstancesV1::from_leaf(leaf.clone(), &alternate_binding)
        .expect("alternate final State ABI");

    assert_eq!(alternate.state_leaf(), leaf);
    assert_ne!(
        &alternate.words()[STATE_LEAF_ABI_WORDS..],
        &full.words()[STATE_LEAF_ABI_WORDS..]
    );
}

#[test]
fn eq_and_ep_share_semantics_and_recursive_pair_but_pin_parity_protocol() {
    let eq = send_instances(OfflineCashHalo2ParityV1::Eq);
    let ep = send_instances(OfflineCashHalo2ParityV1::Ep);
    let parity_word = 3;
    let protocol_words = 16..24;
    let recursive_pair_words = 93..STATE_ABI_WORDS;
    for index in 0..STATE_ABI_WORDS {
        if index == parity_word || protocol_words.contains(&index) {
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
    assert_eq!(
        &eq.words()[recursive_pair_words.clone()],
        &ep.words()[recursive_pair_words]
    );

    let output = receive_output();
    let recursive_pair_binding = canonical_recursive_pair_binding();
    let eq_receive = OfflineCashStatePublicInstancesV1::receive_fold(
        &output,
        OfflineCashHalo2ParityV1::Eq,
        &recursive_pair_binding,
    )
    .expect("Eq receive STATE ABI");
    let ep_receive = OfflineCashStatePublicInstancesV1::receive_fold(
        &output,
        OfflineCashHalo2ParityV1::Ep,
        &recursive_pair_binding,
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
fn strict_constructors_reject_context_pair_binding_receive_and_parity_substitution() {
    let release = super::terminal_tests::authenticated_release();
    let request = super::terminal_tests::request(&release);
    let payment = super::terminal_tests::payment(&release, &request);
    let statement = payment
        .reconstruct_statement(&request)
        .expect("reconstructed statement");
    let wrong_context = OfflineCashStateContextV1::new(
        [0xA5; 32],
        statement.network_id.clone(),
        statement.asset.clone(),
        statement.scale,
    )
    .expect("different valid context");
    assert_eq!(
        OfflineCashStatePublicInstancesV1::send_split(
            &wrong_context,
            &statement,
            OfflineCashHalo2ParityV1::Eq,
            &canonical_recursive_pair_binding(),
        ),
        Err(OfflineCashStateAbiErrorV1::ContextMismatch)
    );
    let context = OfflineCashStateContextV1::new(
        statement.release_id,
        statement.network_id.clone(),
        statement.asset.clone(),
        statement.scale,
    )
    .expect("matching context");
    let invalid_recursive_pair_binding =
        OfflineCashRecursivePairBindingV1::new_guard_bundle([0xC3; 32], [0xD4; 32])
            .expect("canonical GuardBundle recursive pair binding");
    assert_eq!(
        OfflineCashStatePublicInstancesV1::send_split(
            &context,
            &statement,
            OfflineCashHalo2ParityV1::Eq,
            &invalid_recursive_pair_binding,
        ),
        Err(OfflineCashStateAbiErrorV1::InvalidRecursivePairBinding)
    );
    let mut invalid_receive = receive_output();
    invalid_receive.next_head = invalid_receive.balance_parent;
    assert_eq!(
        OfflineCashStatePublicInstancesV1::receive_fold(
            &invalid_receive,
            OfflineCashHalo2ParityV1::Ep,
            &canonical_recursive_pair_binding(),
        ),
        Err(OfflineCashStateAbiErrorV1::InvalidReceiveOutput)
    );
    let (ep, ep_witness) = receive_fixture(OfflineCashHalo2ParityV1::Ep);
    assert!(matches!(
        OfflineCashEqStateLeafCircuitV1::new(ep.state_leaf(), ep_witness),
        Err(OfflineCashStateAbiErrorV1::ParityMismatch)
    ));
}

#[test]
fn both_fixed_k16_public_binding_circuits_accept_exact_instances() {
    let (eq, eq_witness) = send_fixture(OfflineCashHalo2ParityV1::Eq);
    let eq_public = eq.state_leaf().field_instances::<Fp>().to_vec();
    let eq_circuit = OfflineCashEqStateLeafCircuitV1::new(eq.state_leaf(), eq_witness)
        .expect("Eq StateLeaf circuit");
    MockProver::run(OFFLINE_CASH_HALO2_K_V1, &eq_circuit, vec![eq_public])
        .expect("Eq STATE synthesis")
        .assert_satisfied();

    let (ep, ep_witness) = send_fixture(OfflineCashHalo2ParityV1::Ep);
    let ep_public = ep.state_leaf().field_instances::<Fq>().to_vec();
    let ep_circuit = OfflineCashEpStateLeafCircuitV1::new(ep.state_leaf(), ep_witness)
        .expect("Ep StateLeaf circuit");
    MockProver::run(OFFLINE_CASH_HALO2_K_V1, &ep_circuit, vec![ep_public])
        .expect("Ep STATE synthesis")
        .assert_satisfied();
}

#[test]
fn circuit_rejects_closed_operation_parent_substitution_padding_and_224_bit_overflow() {
    let (eq, invalid_operation_witness) = send_fixture(OfflineCashHalo2ParityV1::Eq);
    let mut invalid_operation_words = *eq.words();
    invalid_operation_words[STATE_OPERATION_WORD] = 3;
    let invalid_operation_public = packed_leaf_fields_from_words::<Fp>(&invalid_operation_words);
    let invalid_operation_circuit = OfflineCashEqStateLeafCircuitV1::from_words_for_test(
        invalid_operation_words[..STATE_LEAF_ABI_WORDS]
            .try_into()
            .expect("leaf semantic words"),
        invalid_operation_witness,
    );
    assert!(
        MockProver::run(
            OFFLINE_CASH_HALO2_K_V1,
            &invalid_operation_circuit,
            vec![invalid_operation_public],
        )
        .expect("invalid-operation synthesis")
        .verify()
        .is_err()
    );

    let (eq, parent_witness) = send_fixture(OfflineCashHalo2ParityV1::Eq);
    let valid_public = eq.state_leaf().field_instances::<Fp>().to_vec();
    let circuit = OfflineCashEqStateLeafCircuitV1::new(eq.state_leaf(), parent_witness)
        .expect("Eq StateLeaf circuit");

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
    let padding_circuit = OfflineCashEqStateLeafCircuitV1::new(eq.state_leaf(), padding_witness)
        .expect("Eq StateLeaf circuit");
    let mut nonzero_padding = valid_public.clone();
    let padding_coefficient = (0..2).fold(Fp::ONE, |value, _| value * Fp::from(1_u64 << 32));
    nonzero_padding[STATE_LEAF_INSTANCE_CELLS - 1] += padding_coefficient;
    assert!(
        MockProver::run(
            OFFLINE_CASH_HALO2_K_V1,
            &padding_circuit,
            vec![nonzero_padding],
        )
        .expect("padding-substitution synthesis")
        .verify()
        .is_err()
    );

    let (eq, overflow_witness) = send_fixture(OfflineCashHalo2ParityV1::Eq);
    let overflow_circuit = OfflineCashEqStateLeafCircuitV1::new(eq.state_leaf(), overflow_witness)
        .expect("Eq StateLeaf circuit");
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
fn state_leaf_shapes_are_parity_symmetric_and_use_the_reviewed_direct_query() {
    let eq = configured_state_shape::<Fp, OfflineCashEqStateLeafCircuitV1>(
        PastaIpaInstanceQueryV1::Direct,
    );
    let ep = configured_state_shape::<Fq, OfflineCashEpStateLeafCircuitV1>(
        PastaIpaInstanceQueryV1::Direct,
    );
    assert_eq!(eq, ep);
    assert_eq!(eq.k(), OFFLINE_CASH_HALO2_K_V1);
    assert_eq!(eq.instance_columns(), 1);
    assert_eq!(eq.instance_queries(), 1);
    assert!(eq.permutation_columns() > 13);

    for (parity, shape) in [
        (OfflineCashHalo2ParityV1::Eq, &eq),
        (OfflineCashHalo2ParityV1::Ep, &ep),
    ] {
        preflight_offline_cash_recursion_activation_v1(
            parity,
            OfflineCashHalo2CircuitRoleV1::StateLeaf,
            shape,
        )
        .expect("StateLeaf is an internal direct-instance role");
    }

    let queried = configured_state_shape::<Fp, OfflineCashEqStateLeafCircuitV1>(
        PastaIpaInstanceQueryV1::Queried,
    );
    assert!(matches!(
        preflight_offline_cash_recursion_activation_v1(
            OfflineCashHalo2ParityV1::Eq,
            OfflineCashHalo2CircuitRoleV1::StateLeaf,
            &queried,
        ),
        Err(
            OfflineCashRecursionActivationPreflightErrorV1::InvalidInstanceQuery {
                actual: PastaIpaInstanceQueryV1::Queried,
            }
        )
    ));
    assert_eq!(
        queried.ordinary_proof_bytes(),
        eq.ordinary_proof_bytes() + 32
    );
}

#[test]
fn production_backend_connects_typed_state_verification_but_remains_fail_closed() {
    let backend = include_str!("halo2_backend.rs");
    assert!(backend.contains("terminal_verify_eq_outer_and_carried_v1"));
    assert!(backend.contains("terminal_verify_ep_outer_and_carried_v1"));
    assert!(backend.contains("offline_cash_eq_lineage_instance_column_v1"));
    assert!(backend.contains("offline_cash_ep_lineage_instance_column_v1"));
    assert!(!backend.contains(concat!("verify_augmented_", "ipa_proof_v1")));
    assert!(!backend.contains(concat!("OfflineCashIpa", "HistoryV1")));
    assert!(!backend.contains(concat!("decide_eq_", "history_v1")));
    assert!(!backend.contains(concat!("decide_ep_", "history_v1")));
    assert!(backend.contains("OfflineCashEqStateCircuitV1"));
    assert!(backend.contains("OfflineCashEpStateCircuitV1"));
    assert!(backend.contains("OfflineCashStatePublicInstancesV1"));
    assert!(backend.contains("authorize_verified_credit"));
    assert!(backend.contains("PRODUCTION_ACTIVATION_BLOCKER_V1"));
    assert!(backend.contains("drop(artifacts)"));
    assert!(!backend.contains(".authenticate_state_verifier"));
    assert!(backend.contains("34-artifact STATE/GuardBundle/helper/P-256 inventory"));
    assert!(!backend.contains("VerificationUnavailable"));

    let circuit = include_str!("state_circuit.rs");
    assert!(!circuit.contains("verify_proof"));
    assert!(!circuit.contains("create_proof"));
    assert!(circuit.contains("final `State` wrapper recursively authenticates this leaf"));
    assert!(!circuit.contains("remain deferred"));
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
    assert!(relation.matches(".zeroize();").count() >= 17);
    assert!(relation.contains("decode_canonical::<OfflineCashTransferStatementV1>"));
    assert!(relation.contains("canonical_transition_digest_message"));
    assert!(relation.contains("canonical_semantic_digest_message"));
    let protocol = include_str!("protocol.rs");
    assert!(protocol.contains("axiom-floorplanner-v1-two-pass+witnessless-measurement"));
    assert!(protocol.contains("jobs-ordered(6,6,5,6,6,2,2,5,6,7,5,8,7)-blocks"));
    assert!(protocol.contains("nonzero-9x32byte-private-bindings+send-seed-op1"));
    assert!(protocol.contains("send-seed+branch-openings+context+transition+semantic-op1-gated"));
    assert!(protocol.contains(
        "network72+asset72-exact-canonical-frame-private-witness+fixed-payload-offset-bindings"
    ));
    assert!(!protocol.contains("send-transition+semantic-deferred"));

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
        6
    );
    assert!(relation_circuit.contains("fn bind_send_sha_digest_v1<F: PrimeField>("));
    assert!(relation_circuit.contains("enabled * send * (word - reconstructed)"));
    assert_eq!(
        relation_circuit
            .matches("constrain_send_statement_message_v1(")
            .count(),
        2
    );
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

    let (instances, witness) = send_fixture(OfflineCashHalo2ParityV1::Eq);
    let public = instances
        .relation_public()
        .expect("canonical send public relation");
    assert_eq!(witness.network_id_frame.len(), NETWORK_ID_FRAME_BYTES_V1);
    assert_eq!(witness.asset_id_frame.len(), ASSET_ID_FRAME_BYTES_V1);
    assert_eq!(
        witness.send_transition_message.len(),
        SEND_TRANSITION_MESSAGE_BYTES_V1
    );
    assert_eq!(
        witness.send_semantic_message.len(),
        SEND_SEMANTIC_MESSAGE_BYTES_V1
    );
    assert_eq!(
        <[u8; 32]>::from(sha2::Sha256::digest(
            witness.send_transition_message.as_slice(),
        )),
        public.transition_digest
    );
    assert_eq!(
        <[u8; 32]>::from(sha2::Sha256::digest(
            witness.send_semantic_message.as_slice(),
        )),
        public.semantic_digest
    );
    let context_message = state_context_message_v1(
        &public.release_id,
        &witness.network_id_frame,
        &witness.asset_id_frame,
        public.scale,
    );
    assert_eq!(context_message.len(), STATE_CONTEXT_MESSAGE_BYTES_V1);
    assert_eq!(
        <[u8; 32]>::from(sha2::Sha256::digest(context_message.as_slice())),
        public.context_digest
    );
    let network = norito::decode_canonical::<NetworkId>(&witness.network_id_frame)
        .expect("canonical NetworkId frame");
    let asset = norito::decode_canonical::<AssetDefinitionId>(&witness.asset_id_frame)
        .expect("canonical asset frame");
    assert_eq!(
        norito::encode_canonical(&network)
            .expect("re-encode NetworkId")
            .as_slice(),
        witness.network_id_frame.as_slice()
    );
    assert_eq!(
        norito::encode_canonical(&asset)
            .expect("re-encode asset")
            .as_slice(),
        witness.asset_id_frame.as_slice()
    );
}

#[test]
fn host_canonical_statement_corridor_matches_circuit_send_digests() {
    let (instances, witness) = send_fixture(OfflineCashHalo2ParityV1::Eq);
    let public = instances
        .relation_public()
        .expect("canonical SendSplit public relation");
    let statement_frame = witness
        .send_semantic_message
        .get(SEND_SEMANTIC_DIGEST_PREFIX_BYTES_V1..)
        .expect("fixed semantic digest prefix");
    let statement = norito::decode_canonical::<OfflineCashTransferStatementV1>(statement_frame)
        .expect("host canonical statement decode");
    assert_eq!(
        norito::encode_canonical(&statement)
            .expect("host canonical statement re-encode")
            .as_slice(),
        statement_frame,
        "canonical decoding must round-trip the exact frame consumed by the circuit"
    );

    let transition_message = statement
        .canonical_transition_digest_message()
        .expect("source-authoritative transition message");
    let semantic_message = statement
        .canonical_semantic_digest_message()
        .expect("source-authoritative semantic message");
    assert_eq!(
        transition_message.as_slice(),
        witness.send_transition_message.as_slice()
    );
    assert_eq!(
        semantic_message.as_slice(),
        witness.send_semantic_message.as_slice()
    );
    assert_eq!(
        <[u8; 32]>::from(sha2::Sha256::digest(transition_message.as_slice())),
        public.transition_digest
    );
    assert_eq!(
        <[u8; 32]>::from(sha2::Sha256::digest(semantic_message.as_slice())),
        public.semantic_digest
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

    let host_mutations: [(&str, fn(&mut OfflineCashStatePrivateWitnessV1)); 9] = [
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
        (
            "noncanonical NetworkId frame",
            OfflineCashStatePrivateWitnessV1::corrupt_network_frame_for_test,
        ),
        (
            "noncanonical asset frame",
            OfflineCashStatePrivateWitnessV1::corrupt_asset_frame_for_test,
        ),
        (
            "SendSplit transition message",
            OfflineCashStatePrivateWitnessV1::corrupt_send_transition_message_for_test,
        ),
        (
            "SendSplit semantic length",
            OfflineCashStatePrivateWitnessV1::corrupt_send_semantic_length_for_test,
        ),
        (
            "reordered SendSplit semantic fields",
            OfflineCashStatePrivateWitnessV1::reorder_send_semantic_fields_for_test,
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

    let (instances, mut witness) = send_fixture(OfflineCashHalo2ParityV1::Eq);
    for invalid_len in [
        SEND_TRANSITION_MESSAGE_BYTES_V1 - 1,
        SEND_TRANSITION_MESSAGE_BYTES_V1 + 1,
    ] {
        assert_eq!(
            witness.replace_send_transition_message_for_test(&vec![0; invalid_len]),
            Err(OfflineCashStateAbiErrorV1::InvalidPrivateWitness),
            "accepted non-exact transition preimage length {invalid_len}"
        );
    }
    assert_eq!(witness.validate_against(&instances), Ok(()));
}

#[test]
fn circuit_relation_rejects_private_witness_substitution() {
    let circuit_mutations: [(&str, fn(&mut OfflineCashStatePrivateWitnessV1)); 10] = [
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
        (
            "noncanonical NetworkId frame",
            OfflineCashStatePrivateWitnessV1::corrupt_network_frame_for_test,
        ),
        (
            "noncanonical asset frame",
            OfflineCashStatePrivateWitnessV1::corrupt_asset_frame_for_test,
        ),
        (
            "SendSplit transition message",
            OfflineCashStatePrivateWitnessV1::corrupt_send_transition_message_for_test,
        ),
        (
            "SendSplit semantic length",
            OfflineCashStatePrivateWitnessV1::corrupt_send_semantic_length_for_test,
        ),
        (
            "reordered SendSplit semantic fields",
            OfflineCashStatePrivateWitnessV1::reorder_send_semantic_fields_for_test,
        ),
    ];
    for (label, mutate) in circuit_mutations {
        let (instances, mut witness) = send_fixture(OfflineCashHalo2ParityV1::Eq);
        let public = instances.state_leaf().field_instances::<Fp>().to_vec();
        mutate(&mut witness);
        let circuit = OfflineCashEqStateLeafCircuitV1::from_words_for_test(
            *instances.state_leaf().words(),
            witness,
        );
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
fn ep_circuit_rejects_noncanonical_and_reordered_send_preimages() {
    let circuit_mutations: [(&str, fn(&mut OfflineCashStatePrivateWitnessV1)); 5] = [
        (
            "noncanonical NetworkId frame",
            OfflineCashStatePrivateWitnessV1::corrupt_network_frame_for_test,
        ),
        (
            "noncanonical asset frame",
            OfflineCashStatePrivateWitnessV1::corrupt_asset_frame_for_test,
        ),
        (
            "SendSplit transition message",
            OfflineCashStatePrivateWitnessV1::corrupt_send_transition_message_for_test,
        ),
        (
            "SendSplit semantic length",
            OfflineCashStatePrivateWitnessV1::corrupt_send_semantic_length_for_test,
        ),
        (
            "reordered SendSplit semantic fields",
            OfflineCashStatePrivateWitnessV1::reorder_send_semantic_fields_for_test,
        ),
    ];
    for (label, mutate) in circuit_mutations {
        let (instances, mut witness) = send_fixture(OfflineCashHalo2ParityV1::Ep);
        let public = instances.state_leaf().field_instances::<Fq>().to_vec();
        mutate(&mut witness);
        let circuit = OfflineCashEpStateLeafCircuitV1::from_words_for_test(
            *instances.state_leaf().words(),
            witness,
        );
        assert!(
            MockProver::run(OFFLINE_CASH_HALO2_K_V1, &circuit, vec![public])
                .unwrap_or_else(|error| panic!("Ep {label} substitution synthesis: {error}"))
                .verify()
                .is_err(),
            "Ep circuit accepted substituted {label}"
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
    let public = instances.state_leaf().field_instances::<Fp>().to_vec();
    let circuit = OfflineCashEqStateLeafCircuitV1::from_words_for_test(
        *instances.state_leaf().words(),
        witness,
    );
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
        let public = packed_leaf_fields_from_words::<Fp>(&words);
        let circuit = OfflineCashEqStateLeafCircuitV1::from_words_for_test(
            words[..STATE_LEAF_ABI_WORDS]
                .try_into()
                .expect("leaf semantic words"),
            witness,
        );
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
        let public = instances.state_leaf().field_instances::<Fp>().to_vec();
        let circuit = OfflineCashEqStateLeafCircuitV1::from_words_for_test(
            *instances.state_leaf().words(),
            witness,
        );
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
        &canonical_recursive_pair_binding(),
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
    let public_fields = instances.state_leaf().field_instances::<Fp>().to_vec();
    let circuit = OfflineCashEqStateLeafCircuitV1::from_words_for_test(
        *instances.state_leaf().words(),
        witness,
    );
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
    let public = instances.state_leaf().field_instances::<Fp>().to_vec();
    for rejected_field in 0..10 {
        witness.zero_rejected_field_for_test(rejected_field);
    }
    let circuit = OfflineCashEqStateLeafCircuitV1::from_words_for_test(
        *instances.state_leaf().words(),
        witness,
    );
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
    let circuit = OfflineCashEqStateLeafCircuitV1::new(instances.state_leaf(), witness)
        .expect("Eq StateLeaf circuit");
    assert!(circuit.has_witness_for_test());
    let witnessless = <OfflineCashEqStateLeafCircuitV1 as Circuit<Fp>>::without_witnesses(&circuit);
    assert!(!witnessless.has_witness_for_test());

    let (instances, witness) = receive_fixture(OfflineCashHalo2ParityV1::Ep);
    let circuit = OfflineCashEpStateLeafCircuitV1::new(instances.state_leaf(), witness)
        .expect("Ep StateLeaf circuit");
    assert!(circuit.has_witness_for_test());
    let witnessless = <OfflineCashEpStateLeafCircuitV1 as Circuit<Fq>>::without_witnesses(&circuit);
    assert!(!witnessless.has_witness_for_test());
}
