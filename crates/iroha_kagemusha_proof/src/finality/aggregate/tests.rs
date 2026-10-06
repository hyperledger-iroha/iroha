//! Real native validator-key aggregation and recomputed false output rejection.

use super::*;
use ark_bls12_381::{Fr, G1Affine, G1Projective};
use ark_ec::{AffineRepr, CurveGroup};
use ark_ff::{PrimeField, Zero};
use ark_serialize::CanonicalSerialize;
use iroha_plonk::{
    check::{CheckMode, check_circuit},
    frontend::synthesize,
};

fn fixture(n: usize) -> (Vec<[u8; 48]>, Vec<u8>, [u8; 48]) {
    let mut keys = Vec::new();
    let mut sum = G1Projective::zero();
    let q = n - (n - 1) / 3;
    let mut bitmap = vec![0; n.div_ceil(8)];
    for i in 0..n {
        let point = G1Affine::generator()
            .mul_bigint(Fr::from((i + 1) as u64).into_bigint())
            .into_affine();
        let mut bytes = Vec::new();
        point.serialize_compressed(&mut bytes).unwrap();
        keys.push(bytes.try_into().unwrap());
        if i < q {
            bitmap[i / 8] |= 1 << (i % 8);
            sum += point;
        }
    }
    let mut aggregate = Vec::new();
    sum.into_affine()
        .serialize_compressed(&mut aggregate)
        .unwrap();
    (keys, bitmap, aggregate.try_into().unwrap())
}
fn accepts(leaf: &AggregateLeafCircuit) -> bool {
    check_circuit(leaf, 16, &leaf.instances().unwrap(), CheckMode::Strict)
        .is_ok_and(|r| r.is_satisfied())
}

#[test]
fn native_quorum_witnesses_have_exact_context_and_contiguous_complete_boundaries() {
    for n in [4, 7, 31] {
        let (keys, bitmap, aggregate) = fixture(n);
        let leaves = prepare_aggregation(&keys, &bitmap, aggregate).unwrap();
        assert_eq!(leaves.len(), PROGRAM_LENGTH as usize);
        let context = leaves[0].context.digest();
        assert_eq!(
            leaves[0].endpoints()[4],
            boundary_digest_native(context, false)
        );
        assert_eq!(
            leaves[32].endpoints()[5],
            boundary_digest_native(context, true)
        );
        for pair in leaves.windows(2) {
            let left = pair[0].endpoints();
            let right = pair[1].endpoints();
            assert_eq!(&left[..2], &right[..2]);
            assert_eq!(left[3], right[2]);
            assert_eq!(left[5], right[4]);
        }
        let mut wrong = aggregate;
        wrong[2] ^= 1;
        assert!(prepare_aggregation(&keys, &bitmap, wrong).is_err());
        let mut wrong = bitmap.clone();
        wrong[0] ^= 1;
        assert!(prepare_aggregation(&keys, &wrong, aggregate).is_err());
        wrong = bitmap.clone();
        wrong.push(0);
        assert!(prepare_aggregation(&keys, &wrong, aggregate).is_err());
    }
}

#[test]
fn strict_aggregation_checks_selected_inactive_and_terminal_steps() {
    let (keys, bitmap, aggregate) = fixture(4);
    let leaves = prepare_aggregation(&keys, &bitmap, aggregate).unwrap();
    for index in [0, 1, 4, 31, 32] {
        assert!(accepts(&leaves[index]), "step {index}");
    }
    let honest = &leaves[1];
    let mut wrong = honest.clone();
    wrong.after.as_mut().unwrap().x[0] ^= 1;
    assert!(!accepts(&wrong), "recomputed false output state");
    let mut wrong = honest.clone();
    wrong.context.bitmap[0] = 0b1110;
    assert!(!accepts(&wrong), "different exact quorum with stale result");
    let mut wrong = honest.clone();
    wrong.seat.as_mut().unwrap().key[20] ^= 1;
    assert!(!accepts(&wrong), "different original key");
    let mut wrong = honest.clone();
    wrong.seat.as_mut().unwrap().path[3] += Fp::ONE;
    assert!(!accepts(&wrong), "different roster opening");
    let mut wrong = leaves[31].clone();
    wrong.seat.as_mut().unwrap().key[0] = 1;
    assert!(!accepts(&wrong), "nonzero inactive key");
    let mut wrong = leaves[32].clone();
    wrong.context.aggregate_key[30] ^= 1;
    assert!(!accepts(&wrong), "different compressed aggregate");
    let known = synthesize(honest, 16, None).unwrap();
    let unknown = synthesize(&honest.without_witnesses(), 16, None).unwrap();
    assert_eq!(known.tables.fixed(), unknown.tables.fixed());
    assert_eq!(
        known.tables.advice_assigned(),
        unknown.tables.advice_assigned()
    );
}

#[test]
#[ignore = "complete33-leaf strict arithmetic sweep over4- and31-seat committees"]
fn strict_complete_aggregation_program_at_k16() {
    for n in [4, 31] {
        let (keys, bitmap, aggregate) = fixture(n);
        let leaves = prepare_aggregation(&keys, &bitmap, aggregate).unwrap();
        for (i, leaf) in leaves.iter().enumerate() {
            assert!(accepts(leaf), "n={n}, step={i}");
        }
    }
}
