//! Exact subset interpolation and its allocation-free checked-share boundary.

use super::*;
use crate::test_allocations::without_allocations;

const PAYLOAD: &[u8] = b"original-checked-partial-interpolation";

fn signed_partials() -> (AdaptiveFixture, Vec<DasRenPartialSignature<BeaconPurpose>>) {
    let fixture = adaptive_fixture();
    let mut rng = ChaCha20Rng::from_seed([0x6d; 32]);
    let partials = (1..=fixture.transcript.session().committee_size())
        .map(|index| {
            adaptive_secret(&fixture, index)
                .sign_payload_with_rng(&fixture.transcript, PAYLOAD, &mut rng)
                .expect("actual session-bound partial proof")
        })
        .collect();
    (fixture, partials)
}

#[test]
fn checked_interpolation_has_no_allocation_and_preserves_every_accepted_subset() {
    let (fixture, partials) = signed_partials();
    // The three fixture dealers have unblinded constants 11, 12 and 13.
    // This independent direct BLS signature pins the exact bytes, rather than
    // only comparing two executions of the changed interpolation routine.
    let expected = sign(fixture.transcript.session(), 36, PAYLOAD);
    let expected_seed = fixture
        .transcript
        .finalized_seed(PAYLOAD, &expected)
        .expect("direct group signature is valid");
    let mut accepted = 0;
    for mask in 1_u8..16 {
        if mask.count_ones() < u32::from(fixture.transcript.session().threshold()) {
            continue;
        }
        // Input ownership and real representation proofs precede the measured
        // interpolation boundary, exactly as in combine_partial_signatures.
        let subset: Vec<_> = partials
            .iter()
            .enumerate()
            .filter(|(index, _)| mask & (1_u8 << *index) != 0)
            .map(|(_, partial)| *partial)
            .collect();
        for partial in &subset {
            fixture
                .transcript
                .verify_partial_signature(PAYLOAD, partial)
                .expect("checked original proof");
        }
        let point = without_allocations(|| interpolate_partial_signatures(&subset))
            .expect("borrowed original checked shares interpolate");
        assert_eq!(point.to_affine().to_compressed(), *expected.as_bytes());
        let actual = fixture
            .transcript
            .combine_partial_signatures(PAYLOAD, &subset)
            .expect("all canonical threshold-or-larger subsets remain accepted");
        assert_eq!(actual.as_bytes(), expected.as_bytes());
        assert_eq!(
            fixture.transcript.finalized_seed(PAYLOAD, &actual).unwrap(),
            expected_seed
        );
        accepted += 1;
    }
    assert_eq!(accepted, 11, "six pairs, four triples and the complete set");
}

#[test]
fn borrowed_lagrange_indices_preserve_minimum_maximum_and_sparse_geometry() {
    let all: [u16; THRESHOLD_BLS_MAX_COMMITTEE_SIZE_V1 as usize] =
        core::array::from_fn(|index| u16::try_from(index + 1).unwrap());
    let sparse: [u16; THRESHOLD_BLS_MAX_COMMITTEE_SIZE_V1 as usize] =
        core::array::from_fn(|index| u16::try_from(index * 2 + 1).unwrap());
    for committee in [4_usize, 7, usize::from(THRESHOLD_BLS_MAX_COMMITTEE_SIZE_V1)] {
        let threshold = (committee - 1) / 3 + 1;
        for indices in [
            &all[..threshold],
            &all[committee - threshold..committee],
            &sparse[..threshold],
            &all[..committee],
        ] {
            assert!(indices.iter().all(|index| usize::from(*index) <= committee));
            for degree in 0..threshold {
                // p(x) = 1 + 2x + ... + (degree + 1)x^degree.
                // Reconstructing p(0) must yield one for every permitted size,
                // including sparse seats and accepted supersets of threshold.
                let constant = without_allocations(|| {
                    indices.iter().fold(Scalar::from(0_u64), |sum, index| {
                        let x = Scalar::from(u64::from(*index));
                        let value = (0..=degree).rev().fold(Scalar::from(0_u64), |acc, power| {
                            acc * x + Scalar::from(u64::try_from(power + 1).unwrap())
                        });
                        sum + value * lagrange_at_zero(*index, indices.iter().copied()).unwrap()
                    })
                });
                assert_eq!(constant, Scalar::from(1_u64));
            }
        }
    }
}

#[test]
fn interpolation_keeps_count_session_order_and_proof_error_precedence() {
    let (fixture, partials) = signed_partials();
    let combine = |subset: &[DasRenPartialSignature<BeaconPurpose>]| {
        fixture
            .transcript
            .combine_partial_signatures(PAYLOAD, subset)
    };
    for subset in [
        &partials[..0],
        &partials[..1],
        &[partials[0]; 5][..],
        &[partials[1], partials[0]][..],
        &[partials[0], partials[0]][..],
    ] {
        assert_eq!(
            combine(subset),
            Err(ThresholdBlsError::NonCanonicalPartialSignatureSet)
        );
    }

    let mut forged = partials[0];
    forged.z_r = (decode_scalar(&forged.z_r).unwrap() + Scalar::from(1_u64)).to_bytes_be();
    // Proof failure in an earlier share wins over a later duplicate index.
    assert_eq!(
        combine(&[forged, partials[1], partials[1]]),
        Err(ThresholdBlsError::InvalidPartialSignatureProof)
    );
    // Count rejection still precedes the first proof.
    assert_eq!(
        combine(&[forged]),
        Err(ThresholdBlsError::NonCanonicalPartialSignatureSet)
    );
    // A share's context/index is checked before its proof.
    let mut foreign = forged;
    foreign.session_id = binding(99);
    assert_eq!(
        combine(&[foreign, partials[1]]),
        Err(ThresholdBlsError::NonCanonicalPartialSignatureSet)
    );
    let mut absent = forged;
    absent.index = 0;
    assert_eq!(
        combine(&[absent, partials[1]]),
        Err(ThresholdBlsError::NonCanonicalPartialSignatureSet)
    );
    absent.index = fixture.transcript.session().committee_size() + 1;
    assert_eq!(
        combine(&[absent, partials[1]]),
        Err(ThresholdBlsError::NonCanonicalPartialSignatureSet)
    );
    assert_eq!(
        fixture
            .transcript
            .combine_partial_signatures(b"foreign-pulse", &partials[..2]),
        Err(ThresholdBlsError::InvalidPartialSignatureProof)
    );
}
