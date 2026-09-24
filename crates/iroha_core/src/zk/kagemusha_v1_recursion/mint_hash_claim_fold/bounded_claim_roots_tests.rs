//! Fixed-capacity Base-cell bridge to the active-prefix recursive SHA claim.

use super::*;
use crate::zk::{
    kagemusha_v1_poseidon::decode,
    kagemusha_v1_recursion::mint_hash_shard::KagemushaMintHashPlanV1,
    pasta_sha256::{PastaSha256ByteV1, PastaSha256JobsV1},
};
use halo2_base::gates::circuit::builder::BaseCircuitBuilder;
use halo2_proofs::{
    dev::MockProver,
    halo2curves::pasta::{Fp, Fq},
};

const K: usize = 14;
const RELEASE: DigestV1 = [0x45; 32];
const CAPACITY: usize = 80;

fn assigned_message<F: KagemushaPoseidonFieldV1>(
    builder: &mut BaseCircuitBuilder<F>,
    message: &[u8],
) -> Vec<PastaSha256ByteV1<F>> {
    let range = builder.range_chip();
    let ctx = builder.main(0);
    message
        .iter()
        .map(|byte| {
            let value = ctx.load_witness(F::from(u64::from(*byte)));
            PastaSha256ByteV1::range_checked(ctx, &range, value)
        })
        .collect()
}

fn active_message(length: usize) -> Vec<u8> {
    (0..length)
        .map(|index| (index as u8).wrapping_mul(37).wrapping_add(11))
        .collect()
}

fn native_plan<F: KagemushaPoseidonFieldV1>(length: usize) -> KagemushaMintHashClaimPlanV1 {
    let parity = if F::IS_EQ_PARITY {
        KagemushaPastaParityV1::Eq
    } else {
        KagemushaPastaParityV1::Ep
    };
    let messages = vec![b"head".to_vec(), active_message(length), b"tail".to_vec()];
    let leaves = KagemushaMintHashPlanV1::from_messages(RELEASE, parity, [1; 32], messages)
        .expect("active-prefix SHA leaves");
    KagemushaMintHashClaimPlanV1::from_leaves::<F>(RELEASE, leaves.leaves())
        .expect("native active-prefix plan")
}

fn bridge_case<F: KagemushaPoseidonFieldV1>(
    length: usize,
    expected_length: usize,
    xor_first_byte: u8,
) -> (bool, usize, String) {
    let mut builder = BaseCircuitBuilder::<F>::new(false)
        .use_k(K)
        .use_lookup_bits(K - 1)
        .use_instance_columns(1);
    let mut jobs = PastaSha256JobsV1::default();
    let prefix = assigned_message(&mut builder, b"head");
    let range = builder.range_chip();
    jobs.digest_constrained(builder.main(0), &prefix)
        .expect("prefix");
    let mut bytes = active_message(length);
    if let Some(first) = bytes.first_mut() {
        *first ^= xor_first_byte;
    }
    let logical = bytes.clone();
    bytes.resize(CAPACITY, 0);
    let bounded = assigned_message(&mut builder, &bytes);
    let message_len = builder.main(0).load_witness(F::from(length as u64));
    jobs.digest_bounded_constrained(builder.main(0), &range, &bounded, message_len)
        .expect("bounded Base/Table8 job");
    let suffix = assigned_message(&mut builder, b"tail");
    jobs.digest_constrained(builder.main(0), &suffix)
        .expect("suffix");
    assert_eq!(
        jobs.bounded_claim_messages().expect("logical queue"),
        vec![b"head".to_vec(), logical, b"tail".to_vec()]
    );
    assert!(jobs.canonical_messages().is_err());
    let release = RELEASE
        .chunks_exact(16)
        .map(|chunk| {
            let limb = u128::from_le_bytes(chunk.try_into().expect("digest limb"));
            builder.main(0).load_constant(F::from_u128(limb))
        })
        .collect::<Vec<_>>();
    let roots = constrain_typed_sha_claim_roots_v1(
        builder.main(0),
        &range,
        &jobs,
        [release[0], release[1]],
    )
    .expect("bounded claim root bridge");
    builder.assigned_instances = vec![vec![roots.message_root, roots.terminal_root, roots.stages]];
    let shape = format!("{:?}", builder.main(0).selector.iter().collect::<Vec<_>>());
    let advice = builder.main(0).advice_len();
    builder.calculate_params(Some(9));
    let expected = native_plan::<F>(expected_length);
    let public = vec![
        decode::<F>(expected.expected_message_root).expect("native message root"),
        decode::<F>(expected.expected_terminal_root).expect("native terminal root"),
        F::from(expected.total_stages),
    ];
    let valid = MockProver::run(K as u32, &builder, vec![public])
        .expect("Base-only bounded claim bridge synthesis")
        .verify()
        .is_ok();
    (valid, advice, shape)
}

#[test]
fn bounded_claim_roots_bind_exact_active_prefix_and_selected_terminal_in_both_parities() {
    for length in [3, 65] {
        assert!(bridge_case::<Fp>(length, length, 0).0);
        assert!(bridge_case::<Fq>(length, length, 0).0);
    }
    assert!(!bridge_case::<Fp>(3, 65, 0).0);
    assert!(!bridge_case::<Fq>(3, 65, 0).0);
    assert!(!bridge_case::<Fp>(65, 3, 0).0);
    assert!(!bridge_case::<Fq>(65, 3, 0).0);
    assert!(!bridge_case::<Fp>(65, 65, 1).0);
    assert!(!bridge_case::<Fq>(65, 65, 1).0);
}

#[test]
fn bounded_claim_root_bridge_has_one_shape_across_selected_blocks() {
    let (_, fp_short_advice, fp_short_selector) = bridge_case::<Fp>(3, 3, 0);
    let (_, fp_long_advice, fp_long_selector) = bridge_case::<Fp>(65, 65, 0);
    assert_eq!(
        (fp_short_advice, fp_short_selector),
        (fp_long_advice, fp_long_selector)
    );
    let (_, fq_short_advice, fq_short_selector) = bridge_case::<Fq>(3, 3, 0);
    let (_, fq_long_advice, fq_long_selector) = bridge_case::<Fq>(65, 65, 0);
    assert_eq!(
        (fq_short_advice, fq_short_selector),
        (fq_long_advice, fq_long_selector)
    );
}
