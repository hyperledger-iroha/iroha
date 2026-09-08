//! Transcript inventory checks against the pinned dependency's actual BGH19 reader.
//!
//! These canonical scalar/point bytes are parser fixtures, not valid opening or monetary proofs.

use super::*;
use halo2_proofs::halo2curves::{
    group::Curve as _,
    pasta::{EpAffine, EqAffine},
};
use snark_verifier::{
    pcs::{PolynomialCommitmentScheme, Query},
    util::arithmetic::{Domain, FieldExt, Rotation, root_of_unity},
};

type NativeTranscript<C, R> = PoseidonTranscript<
    C,
    NativeLoader,
    R,
    KAGEMUSHA_IPA_POSEIDON_WIDTH_V1,
    KAGEMUSHA_IPA_POSEIDON_RATE_V1,
    KAGEMUSHA_IPA_POSEIDON_FULL_ROUNDS_V1,
    KAGEMUSHA_IPA_POSEIDON_PARTIAL_ROUNDS_V1,
>;

fn assert_reader_inventory<C: CurveAffine>()
where
    C::ScalarExt: FieldExt,
{
    for k in [12, 16] {
        let point = |value| (C::generator() * C::ScalarExt::from(value)).to_affine();
        let key = IpaSuccinctVerifyingKey::new(
            Domain::new(k, root_of_unity::<C::ScalarExt>(k)),
            point(1),
            point(2),
            Some(point(3)),
        );
        let wrapped_previous = (1_i32 << k) - 1;
        let cases = [
            (vec![(0, -1), (1, wrapped_previous)], 2),
            (vec![(0, -1), (1, -1)], 1),
            (vec![(0, -1), (0, -1), (1, -1)], 1),
            (vec![(0, -1), (0, 1), (1, 1), (1, -1)], 1),
            (vec![(0, -1), (1, wrapped_previous), (2, 0)], 3),
        ];
        for (inventory, expected_sets) in cases {
            let queries = inventory
                .iter()
                .map(|&(poly, rotation)| Query::new(poly, Rotation(rotation)))
                .collect::<Vec<_>>();
            let predicted_sets = ordinary_ipa_rotation_set_count_v1(inventory.iter().copied());
            assert_eq!(predicted_sets, expected_sets);

            let mut bytes = Vec::new();
            // F, one evaluation per rotation set, S, two points per IPA round,
            // c, blind, and the final basis point are the exact PCS read order.
            bytes.extend_from_slice(point(4).to_bytes().as_ref());
            for index in 0..expected_sets {
                bytes.extend_from_slice(C::ScalarExt::from(10 + index as u64).to_repr().as_ref());
            }
            bytes.extend_from_slice(point(5).to_bytes().as_ref());
            for index in 0..2 * k {
                bytes.extend_from_slice(point(20 + index as u64).to_bytes().as_ref());
            }
            bytes.extend_from_slice(C::ScalarExt::from(6).to_repr().as_ref());
            bytes.extend_from_slice(C::ScalarExt::from(7).to_repr().as_ref());
            bytes.extend_from_slice(point(8).to_bytes().as_ref());

            // Isolate the opening portion of the shared ordinary-proof byte predictor.
            let profile = ordinary_ipa_proof_profile_from_counts_v1(k, 0, 0, 0, predicted_sets)
                .expect("bounded opening inventory");
            assert_eq!(profile.byte_len, bytes.len());
            let (reader, position) = ExactReader::new(&bytes);
            let mut transcript =
                NativeTranscript::<C, _>::new::<KAGEMUSHA_IPA_POSEIDON_SECURE_MDS_V1>(reader);
            <IpaAs<C, Bgh19> as PolynomialCommitmentScheme<C, NativeLoader>>::read_proof(
                &key,
                &queries,
                &mut transcript,
            )
            .expect("actual BGH19 reader consumes the predicted rotation-set inventory");
            assert_eq!(position.get(), profile.byte_len);

            let (reader, _) = ExactReader::new(&bytes[..bytes.len() - 1]);
            let mut transcript =
                NativeTranscript::<C, _>::new::<KAGEMUSHA_IPA_POSEIDON_SECURE_MDS_V1>(reader);
            assert!(
                <IpaAs<C, Bgh19> as PolynomialCommitmentScheme<C, NativeLoader>>::read_proof(
                    &key,
                    &queries,
                    &mut transcript,
                )
                .is_err(),
                "a truncated final point must fail in the actual dependency reader",
            );
        }
    }
}

#[test]
fn eq_bgh19_rotation_inventory_matches_actual_reader() {
    assert_reader_inventory::<EqAffine>();
}

#[test]
fn ep_bgh19_rotation_inventory_matches_actual_reader() {
    assert_reader_inventory::<EpAffine>();
}
