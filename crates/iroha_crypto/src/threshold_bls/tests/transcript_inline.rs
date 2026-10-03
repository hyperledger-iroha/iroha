//! Inline transcript custody with real minimum and maximum degree DKG proofs.

use super::*;
use crate::test_allocations::without_allocations;

const PAYLOAD: &[u8] = b"inline-retained-adaptive-transcript";

struct Fixture {
    parameters: AdaptiveThresholdBlsParameters<BeaconPurpose>,
    dealers: Vec<ValidatedDealerCommitment<BeaconPurpose>>,
    indices: Vec<u16>,
    coefficients: Vec<Vec<[Scalar; 3]>>,
    group_constant: u64,
}

fn fixture(committee: u16, qualified: u16) -> Fixture {
    let session = ThresholdBlsSession::new(
        binding(1),
        binding(u8::try_from(committee).unwrap()),
        binding(3),
        committee,
        (committee - 1) / 3 + 1,
    )
    .unwrap();
    let parameters = AdaptiveThresholdBlsParameters::derive(&session).unwrap();
    let mut rng = ChaCha20Rng::from_seed([0x79; 32]);
    let mut dealers = Vec::new();
    let mut coefficients = Vec::new();
    let mut group_constant = 0;
    for index in 1..=qualified {
        let values: Vec<_> = (0..session.threshold())
            .map(|degree| {
                [
                    Scalar::from(10 + u64::from(index) + 13 * u64::from(degree)),
                    Scalar::from(if degree == 0 {
                        0
                    } else {
                        100 + 3 * u64::from(index) + u64::from(degree)
                    }),
                    Scalar::from(if degree == 0 {
                        0
                    } else {
                        200 + 5 * u64::from(index) + u64::from(degree)
                    }),
                ]
            })
            .collect();
        let bytes = Zeroizing::new(
            values
                .iter()
                .map(|components| components.map(|value| value.to_bytes_be()))
                .collect(),
        );
        let (secret, dealer) =
            DasRenDealerSecret::from_coefficients_with_rng(&parameters, index, bytes, &mut rng)
                .expect("actual exact-degree dealer and constant proof");
        drop(secret);
        dealers.push(dealer);
        coefficients.push(values);
        group_constant += 10 + u64::from(index);
    }
    Fixture {
        parameters,
        dealers,
        indices: (1..=qualified).collect(),
        coefficients,
        group_constant,
    }
}

fn secret_components(fixture: &Fixture, index: u16) -> [Scalar; 3] {
    let x = Scalar::from(u64::from(index));
    core::array::from_fn(|component| {
        fixture
            .coefficients
            .iter()
            .fold(Scalar::from(0_u64), |sum, dealer| {
                sum + dealer
                    .iter()
                    .rev()
                    .fold(Scalar::from(0_u64), |acc, coefficient| {
                        acc * x + coefficient[component]
                    })
            })
    })
}

fn assert_inline_slice<T, Owner>(owner: &Owner, values: &[T]) {
    assert!(!values.is_empty());
    let start = core::ptr::from_ref(owner) as usize;
    let end = start + core::mem::size_of_val(owner);
    let slice_start = values.as_ptr() as usize;
    assert!(slice_start >= start);
    assert!(slice_start + core::mem::size_of_val(values) <= end);
}

fn check_exact_bindings_and_signatures(
    fixture: &Fixture,
    transcript: &AdaptiveThresholdBlsPublicTranscript<BeaconPurpose>,
) {
    let session = fixture.parameters.session();
    let expected_key = key(session, fixture.group_constant);
    assert_eq!(transcript.group_public_key(), &expected_key);
    let mut expected_hash = Sha256::new();
    expected_hash.update(ADAPTIVE_TRANSCRIPT_DOMAIN_V1);
    expected_hash.update(THRESHOLD_BLS_PROTOCOL_VERSION_V1.to_be_bytes());
    expected_hash.update(fixture.parameters.digest());
    expected_hash.update(binding(90));
    expected_hash.update(u32::try_from(fixture.dealers.len()).unwrap().to_be_bytes());
    for dealer in &fixture.dealers {
        expected_hash.update(dealer.dealer_index().to_be_bytes());
        for coefficient in dealer.coefficients() {
            expected_hash.update(coefficient.as_bytes());
        }
        expected_hash.update(dealer.constant_proof().commitment_bytes());
        expected_hash.update(dealer.constant_proof().response_bytes());
    }
    expected_hash.update(expected_key.as_bytes());
    let h = fixture.parameters.h_point().unwrap();
    let v = fixture.parameters.v_point().unwrap();
    let mut rng = ChaCha20Rng::from_seed([0x61; 32]);
    let mut partials = Vec::new();
    for (offset, share) in transcript.public_shares().iter().enumerate() {
        let index = u16::try_from(offset + 1).unwrap();
        let components = secret_components(fixture, index);
        let expected_point = (G2Projective::generator() * components[0]
            + G2Projective::from(h) * components[1]
            + G2Projective::from(v) * components[2])
            .to_affine()
            .to_compressed();
        let mut participant = Sha256::new();
        participant.update(ADAPTIVE_PARTICIPANT_DOMAIN_V1);
        participant.update(session.canonical_bytes());
        participant.update(index.to_be_bytes());
        let participant: [u8; 32] = participant.finalize().into();
        assert_eq!(share.index(), index);
        assert_eq!(share.participant_hash(), &participant);
        assert_eq!(share.as_bytes(), &expected_point);
        expected_hash.update(index.to_be_bytes());
        expected_hash.update(participant);
        expected_hash.update(expected_point);
        let secret = AdaptiveThresholdBlsSecretShare::from_components(
            transcript,
            index,
            components[0].to_bytes_be(),
            components[1].to_bytes_be(),
            components[2].to_bytes_be(),
        )
        .unwrap();
        partials.push(
            secret
                .sign_payload_with_rng(transcript, PAYLOAD, &mut rng)
                .unwrap(),
        );
    }
    let expected_hash: [u8; 32] = expected_hash.finalize().into();
    assert_eq!(transcript.transcript_hash(), &expected_hash);
    let expected = sign(session, fixture.group_constant, PAYLOAD);
    let expected_seed = transcript.finalized_seed(PAYLOAD, &expected).unwrap();
    let threshold = usize::from(session.threshold());
    for subset in [
        &partials[..threshold],
        &partials[partials.len() - threshold..],
        partials.as_slice(),
    ] {
        let signature = transcript
            .combine_partial_signatures(PAYLOAD, subset)
            .unwrap();
        assert_eq!(signature.as_bytes(), expected.as_bytes());
        assert_eq!(
            transcript.finalized_seed(PAYLOAD, &signature).unwrap(),
            expected_seed
        );
    }
}

#[test]
fn inline_transcript_finalization_and_clone_allocate_no_backing_at_minimum_and_maximum() {
    for committee in [4, THRESHOLD_BLS_MAX_COMMITTEE_SIZE_V1] {
        let minimum = committee - (committee - 1) / 3;
        for qualified in [minimum, committee] {
            let fixture = fixture(committee, qualified);
            let transcript = without_allocations(|| {
                AdaptiveThresholdBlsPublicTranscript::from_qualified_dealers(
                    &fixture.parameters,
                    &fixture.dealers,
                    &fixture.indices,
                    binding(90),
                )
            })
            .expect("finalization borrows the already verified input graph");
            assert_eq!(transcript.qualified_indices(), fixture.indices);
            assert_eq!(transcript.public_shares().len(), usize::from(committee));
            assert_eq!(transcript.ensure_adaptive_protocol_ready(), Ok(()));
            assert_inline_slice(&transcript, transcript.qualified_indices());
            assert_inline_slice(&transcript, transcript.public_shares());
            let cloned = without_allocations(|| std::hint::black_box(transcript.clone()));
            assert_eq!(cloned, transcript);
            assert_inline_slice(&cloned, cloned.qualified_indices());
            assert_inline_slice(&cloned, cloned.public_shares());
            assert_ne!(
                cloned.public_shares().as_ptr(),
                transcript.public_shares().as_ptr()
            );
            assert_ne!(
                cloned.qualified_indices().as_ptr(),
                transcript.qualified_indices().as_ptr()
            );
            // Each independent owner exposes only its initialized live entries.
            drop(transcript);
            check_exact_bindings_and_signatures(&fixture, &cloned);
        }
    }
}

#[test]
fn inline_transcript_layout_has_a_fixed_six_kib_owner_bound() {
    type Transcript = AdaptiveThresholdBlsPublicTranscript<BeaconPurpose>;
    type Shares = ArrayVec<AdaptiveThresholdBlsPublicShare<BeaconPurpose>, 31>;
    type Indices = ArrayVec<u16, 31>;
    let owner = core::mem::size_of::<Transcript>();
    let shares = core::mem::size_of::<Shares>();
    let indices = core::mem::size_of::<Indices>();
    assert_eq!(
        core::mem::size_of::<AdaptiveThresholdBlsPublicShare<BeaconPurpose>>(),
        162
    );
    assert!(
        owner <= 6 * 1024,
        "the complete retained owner must stay bounded"
    );
    assert!(owner >= shares + indices);
    assert_eq!(Shares::new().len(), 0);
    assert_eq!(Indices::new().len(), 0);
    eprintln!(
        "inline transcript owner={owner} shares={shares} indices={indices} original+clone={}",
        owner * 2
    );
}

#[test]
fn inline_transcript_preserves_qualified_set_and_point_failure_precedence() {
    let fixture = fixture(4, 3);
    let make = |dealers: &[ValidatedDealerCommitment<BeaconPurpose>], indices: &[u16], event| {
        AdaptiveThresholdBlsPublicTranscript::from_qualified_dealers(
            &fixture.parameters,
            dealers,
            indices,
            event,
        )
    };
    assert_eq!(make(&[], &[], [0; 32]), Err(ThresholdBlsError::ZeroBinding));
    let oversized: Vec<_> = fixture.dealers.iter().cycle().take(5).cloned().collect();
    assert_eq!(
        make(&oversized, &[1, 2, 3, 4, 5], binding(90)),
        Err(ThresholdBlsError::NonCanonicalQualifiedSet)
    );
    let mut corrupted = fixture.dealers.clone();
    corrupted[0].coefficients[0].bytes = [0; THRESHOLD_BLS_PUBLIC_KEY_BYTES];
    assert_eq!(
        make(&corrupted[..2], &[1, 2], binding(90)),
        Err(ThresholdBlsError::NonCanonicalQualifiedSet)
    );
    assert_eq!(
        make(&corrupted, &[1, 1, 3], binding(90)),
        Err(ThresholdBlsError::NonCanonicalQualifiedSet)
    );
    assert_eq!(
        make(&corrupted, &[1, 2, 3], binding(90)),
        Err(ThresholdBlsError::InvalidCoefficientCommitment)
    );
    // Group degeneracy still rejects before any per-seat evaluation.
    let mut degenerate = fixture.dealers.clone();
    degenerate[0].coefficients[0].bytes = (G2Projective::generator() * -Scalar::from(25_u64))
        .to_affine()
        .to_compressed();
    degenerate[0].coefficients[1].bytes = [0; THRESHOLD_BLS_PUBLIC_KEY_BYTES];
    assert_eq!(
        make(&degenerate, &[1, 2, 3], binding(90)),
        Err(ThresholdBlsError::InvalidPublicKey)
    );
}
