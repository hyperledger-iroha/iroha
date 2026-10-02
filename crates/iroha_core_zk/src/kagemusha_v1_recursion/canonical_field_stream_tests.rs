//! Both-field exact canonical length/header/CRC/routing tests. Inert proof data grants no authority.
use super::super::super::guard_bundle::assign_bytes;
use super::*;
use crate::kagemusha_v1_poseidon::from_u128;
use halo2_base::gates::circuit::builder::BaseCircuitBuilder;
use halo2_proofs::{
    dev::MockProver,
    halo2curves::pasta::{Fp, Fq},
};
use iroha_data_model::kagemusha::{
    KagemushaOrdinaryFinancialHeadV1, KagemushaOrdinaryMintPairedProofV1,
};
const K: usize = 17;
fn compact_case<F: KagemushaPoseidonFieldV1>(length: u128, wrong: bool) -> bool {
    let mut b = BaseCircuitBuilder::<F>::new(false)
        .use_k(10)
        .use_lookup_bits(8)
        .use_instance_columns(1);
    let range = b.range_chip();
    let ctx = b.main(0);
    let len = ctx.load_witness(from_u128::<F>(length));
    let out = compact_u32_v1(ctx, &range, len).unwrap();
    let mut bytes = Vec::new();
    norito::core::write_len_to_vec_with_flags(
        &mut bytes,
        length as u64,
        norito::core::header_flags::COMPACT_LEN,
    );
    let used = bytes.len();
    bytes.resize(5, 0);
    if wrong {
        bytes[0] ^= 1;
    }
    let mut instances = vec![F::from(used as u64)];
    instances.extend(bytes.iter().map(|v| F::from(u64::from(*v))));
    let mut cells = vec![out.actual_len()];
    cells.extend(out.bytes().iter().map(|v| {
        range
            .gate()
            .add(ctx, v.quantum_cell(), QuantumCell::Constant(F::ZERO))
    }));
    b.assigned_instances = vec![cells];
    b.calculate_params(Some(9));
    MockProver::run(10, &b, vec![instances])
        .unwrap()
        .verify()
        .is_ok()
}
#[test]
fn ordinary_compact_field_lengths_cover_every_width_with_one_topology_both_fields() {
    for n in [
        0,
        1,
        127,
        128,
        16383,
        16384,
        2097151,
        2097152,
        268435455,
        268435456,
        u64::from(u32::MAX),
    ] {
        assert!(compact_case::<Fp>(u128::from(n), false));
        assert!(compact_case::<Fq>(u128::from(n), false));
        assert!(!compact_case::<Fp>(u128::from(n), true));
        assert!(!compact_case::<Fq>(u128::from(n), true));
    }
    assert!(!compact_case::<Fp>(1_u128 << 32, false));
    assert!(!compact_case::<Fq>(1_u128 << 32, false));
}
fn frame_case<
    F: KagemushaPoseidonFieldV1,
    T: iroha_data_model::kagemusha::KagemushaOrdinaryCanonicalFieldStreamV1,
>(
    v: &T,
    capacities: &[usize],
    mutate: bool,
) -> bool {
    let grammar = KagemushaOrdinaryCanonicalFieldStreamGrammarV1::from_original(v).unwrap();
    let mut b = BaseCircuitBuilder::<F>::new(false)
        .use_k(K)
        .use_lookup_bits(16)
        .use_instance_columns(1);
    let range = b.range_chip();
    let ctx = b.main(0);
    let fields = grammar
        .fields()
        .iter()
        .zip(capacities)
        .enumerate()
        .map(|(i, (raw, capacity))| {
            assert!(raw.len() <= *capacity);
            let mut padded = raw.clone();
            padded.resize(*capacity, 0);
            if mutate && i == 1 {
                padded[0] ^= 1;
            }
            let assigned = assign_bytes(ctx, &range, &padded);
            let len = ctx.load_witness(F::from(raw.len() as u64));
            KagemushaBoundedByteStreamV1::constrain(ctx, &range, assigned, len).unwrap()
        })
        .collect::<Vec<_>>();
    let out = struct_frame_v1(ctx, &range, &grammar, &fields).unwrap();
    let mut expected = norito::encode_canonical(v).unwrap();
    let len = expected.len();
    expected.resize(out.bytes().len(), 0);
    let mut cells = vec![out.actual_len()];
    cells.extend(out.bytes().iter().map(|v| {
        range
            .gate()
            .add(ctx, v.quantum_cell(), QuantumCell::Constant(F::ZERO))
    }));
    let mut instances = vec![F::from(len as u64)];
    instances.extend(expected.into_iter().map(|v| F::from(u64::from(v))));
    b.assigned_instances = vec![cells];
    b.calculate_params(Some(9));
    MockProver::run(K as u32, &b, vec![instances])
        .unwrap()
        .verify()
        .is_ok()
}
#[test]
fn ordinary_bounded_struct_stream_matches_full_high_u128_state_head_and_crc() {
    let head = KagemushaOrdinaryFinancialHeadV1 {
        state_commitment: [5; 32],
        logical_sequence: (1_u128 << 100) + 17,
        state_original_sha256: [6; 32],
    };
    let cap = [32, 16, 32];
    assert!(frame_case::<Fp, _>(&head, &cap, false));
    assert!(frame_case::<Fq, _>(&head, &cap, false));
    assert!(!frame_case::<Fp, _>(&head, &cap, true));
    assert!(!frame_case::<Fq, _>(&head, &cap, true));
}
#[test]
fn ordinary_bounded_proof_data_stream_matches_actual_bare_vector_fields_at_compact_boundary() {
    // Codec data only: these proof bytes are deliberately inert and never admitted as IPA proof.
    for width in [119, 120, 127, 128] {
        let proof = KagemushaOrdinaryMintPairedProofV1 {
            version: 1,
            eq_protocol_digest: [1; 32],
            ep_protocol_digest: [2; 32],
            statement_digest: [3; 32],
            approval_original_digest: [4; 32],
            eq_proof: vec![17; width],
            ep_proof: vec![19; 73],
            eq_history: vec![21; 544],
            ep_history: vec![23; 544],
        };
        let cap = [2, 32, 32, 32, 32, 136, 136, 552, 552];
        assert!(frame_case::<Fp, _>(&proof, &cap, false));
        assert!(frame_case::<Fq, _>(&proof, &cap, false));
    }
}
