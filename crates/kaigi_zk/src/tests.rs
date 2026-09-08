//! Final API regressions retained from the retired helper and circuit surface.
use super::*;
use authorization_v1::{
    KaigiAuthorizationActionV1, KaigiAuthorizationContextV1, KaigiAuthorizationOutputsV1,
    KaigiAuthorizationPublicInputsV1, KaigiAuthorizationWitnessV1, compute_authorization_v1,
};
use halo2_proofs::halo2curves::ff::PrimeField;
use usage_v1::{KaigiUsageContextV1, KaigiUsageOutputsV1, compute_usage_v1};

fn context() -> KaigiAuthorizationContextV1 {
    KaigiAuthorizationContextV1 {
        network_id: array::from_fn(|index| index as u8 + 1),
        call_id: [1, 2, 3, 4, 5, 6],
        host_id: [11, 12, 13, 14, 15, 16],
        subject_id: [11, 12, 13, 14, 15, 16],
        participation_sequence: 0,
        action: KaigiAuthorizationActionV1::HostCreate,
        pre_roster_root: array::from_fn(|index| index as u8 + 33),
    }
}

#[test]
fn final_commitment_nullifier_authorization_and_usage_are_distinct() {
    let context = context();
    let witness = KaigiAuthorizationWitnessV1::take_blinding(&mut Scalar::from(31).to_repr())
        .expect("nonzero canonical witness");
    let auth = compute_authorization_v1(&context, &witness).unwrap();
    let usage = compute_usage_v1(
        &KaigiUsageContextV1 {
            network_id: context.network_id,
            call_id: context.call_id,
            host_id: context.host_id,
            pre_roster_root: context.pre_roster_root,
            segment_index: 2,
            duration_ms: 1200,
            billed_gas: 345,
        },
        &witness,
    )
    .unwrap();
    assert_eq!(usage.host_commitment, auth.commitment);
    let values = [
        auth.commitment,
        auth.nullifier,
        auth.authorization,
        usage.usage_commitment,
    ];
    for (index, left) in values.iter().enumerate() {
        for right in &values[index + 1..] {
            assert_ne!(left, right);
        }
    }
}

#[test]
fn framed_poseidon_rejects_a_collision_in_the_retired_quintic_compressor() {
    // The earlier separable compressor admitted chosen-input collisions. Keep
    // its attack as a test-only oracle against the final framed construction.
    let inverse_five_exponent = [
        0xe0f0_f3f0_cccc_cccd,
        0x4e9e_e0c9_a10a_60e2,
        0x3333_3333_3333_3333,
        0x3333_3333_3333_3333,
    ];
    let inverse_three = Option::<Scalar>::from(Scalar::from(3).invert()).unwrap();
    let fifth_root = (Scalar::from(2) * inverse_three).pow_vartime(inverse_five_exponent);
    let first = [Scalar::ONE - Scalar::from(7), -Scalar::from(13)];
    let second = [-Scalar::from(7), fifth_root - Scalar::from(13)];
    let retired = |[left, right]: [Scalar; 2]| {
        Scalar::from(2) * (left + Scalar::from(7)).pow_vartime([5])
            + Scalar::from(3) * (right + Scalar::from(13)).pow_vartime([5])
    };
    assert_eq!(retired(first), retired(second));
    assert_ne!(
        relation_v1::sponge(0x4b41_4947_4956_3143, &first),
        relation_v1::sponge(0x4b41_4947_4956_3143, &second)
    );
}

#[test]
fn raw_pasta_output_encoding_preserves_bit_248_and_rejects_noncanonical_scalars() {
    let mut high_repr = [0; 32];
    high_repr[31] = 1;
    let high = Option::<Scalar>::from(Scalar::from_repr(high_repr)).unwrap();
    // Keep the collision demonstration for the removed marker-bearing carrier.
    assert_eq!(
        iroha_crypto::Hash::prehashed(Scalar::ZERO.to_repr()),
        iroha_crypto::Hash::prehashed(high.to_repr())
    );
    let auth = KaigiAuthorizationOutputsV1 {
        commitment: Scalar::ZERO,
        nullifier: high,
        authorization: -Scalar::ONE,
    };
    let encoded = auth.canonical_bytes();
    assert_ne!(encoded[0], encoded[1]);
    for (bytes, scalar) in
        encoded
            .into_iter()
            .zip([auth.commitment, auth.nullifier, auth.authorization])
    {
        assert_eq!(bytes, scalar.to_repr());
        assert_eq!(
            Option::<Scalar>::from(Scalar::from_repr(bytes)),
            Some(scalar)
        );
    }
    let usage = KaigiUsageOutputsV1 {
        host_commitment: high,
        usage_commitment: -Scalar::ONE,
    };
    for (bytes, scalar) in usage
        .canonical_bytes()
        .into_iter()
        .zip([usage.host_commitment, usage.usage_commitment])
    {
        assert_eq!(bytes, scalar.to_repr());
        assert_eq!(
            Option::<Scalar>::from(Scalar::from_repr(bytes)),
            Some(scalar)
        );
    }
    assert!(bool::from(Scalar::from_repr([255; 32]).is_none()));
}

#[test]
fn exact_root_instance_limbs_match_every_input_byte() {
    let context = context();
    let witness =
        KaigiAuthorizationWitnessV1::take_blinding(&mut Scalar::from(31).to_repr()).unwrap();
    let outputs = compute_authorization_v1(&context, &witness).unwrap();
    let instance = KaigiAuthorizationPublicInputsV1 { context, outputs }.instance();
    for (bytes, scalar) in context
        .pre_roster_root
        .chunks_exact(8)
        .zip(&instance[24..28])
    {
        let expected = u64::from_le_bytes(bytes.try_into().unwrap());
        let repr = scalar.to_repr();
        assert_eq!(u64::from_le_bytes(repr[..8].try_into().unwrap()), expected);
        assert!(repr[8..].iter().all(|&byte| byte == 0));
        assert_eq!(*scalar, Scalar::from(expected));
    }
    // Both circuit modules additionally prove key generation, positive
    // satisfiability, and rejection after every root/output row is mutated.
}
