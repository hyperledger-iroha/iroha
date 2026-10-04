//! Inline dealer generation against an independent former allocating implementation.

use super::dealer_secret_inline::{
    inline_coefficients, observe_retirement, try_inline_coefficients,
};
use super::*;
use crate::test_allocations::{
    allocations_during, with_deallocation_observation, without_allocations,
};
use rand_core::TryRngCore;

// Independent former Vec owner; never passed into the production importer.
pub(super) struct ReferenceDealerSecret<P: ThresholdBlsPurpose> {
    parameters_digest: [u8; 32],
    session_id: [u8; 32],
    dealer_index: u16,
    coefficients: Zeroizing<Vec<DasRenSecretCoefficientV1>>,
    marker: PhantomData<P>,
}
impl<P: ThresholdBlsPurpose> ReferenceDealerSecret<P> {
    pub fn private_share(
        &self,
        parameters: &AdaptiveThresholdBlsParameters<P>,
        dealer: &ValidatedDealerCommitment<P>,
        recipient_index: u16,
    ) -> Result<DasRenPrivateShare<P>, ThresholdBlsError> {
        if self.parameters_digest != parameters.digest()
            || self.session_id != *parameters.session().session_id()
            || self.dealer_index != dealer.dealer_index
            || dealer.parameters_digest != self.parameters_digest
        {
            return Err(ThresholdBlsError::SessionMismatch);
        }
        validate_participant_index(parameters.session(), recipient_index)?;
        let mut components = [Scalar::from(0_u64); 3];
        let x = Scalar::from(u64::from(recipient_index));
        let mut power = Scalar::from(1_u64);
        for coefficient in self.coefficients.iter() {
            for component in 0..3 {
                components[component] += decode_scalar(&coefficient[component])? * power;
            }
            power *= x;
        }
        DasRenPrivateShare::from_components(
            parameters,
            dealer,
            recipient_index,
            components[0].to_bytes_be(),
            components[1].to_bytes_be(),
            components[2].to_bytes_be(),
        )
    }
}

// Preserve the previous allocating importer only as an independent test reference.
// It retains the original scalar/point/proof operation order and RNG consumption.
pub(super) fn reference_import<P: ThresholdBlsPurpose, R: TryCryptoRng + ?Sized>(
    parameters: &AdaptiveThresholdBlsParameters<P>,
    dealer_index: u16,
    coefficients: Zeroizing<Vec<DasRenSecretCoefficientV1>>,
    rng: &mut R,
) -> Result<(ReferenceDealerSecret<P>, ValidatedDealerCommitment<P>), ThresholdBlsError> {
    validate_participant_index(parameters.session(), dealer_index)?;
    if coefficients.len() != usize::from(parameters.session().threshold()) {
        return Err(ThresholdBlsError::InvalidCoefficientCommitment);
    }
    let zero = Scalar::from(0_u64).to_bytes_be();
    if coefficients[0][1] != zero || coefficients[0][2] != zero {
        return Err(ThresholdBlsError::InvalidCoefficientCommitment);
    }
    let h_generator = parameters.h_point()?;
    let v_generator = parameters.v_point()?;
    let mut commitment_bytes = Vec::with_capacity(coefficients.len());
    for coefficient in coefficients.iter() {
        let secret = decode_scalar(&coefficient[0])?;
        let h_blinding = decode_scalar(&coefficient[1])?;
        let v_blinding = decode_scalar(&coefficient[2])?;
        if commitment_bytes.is_empty() && secret == Scalar::from(0_u64) {
            return Err(ThresholdBlsError::InvalidCoefficientCommitment);
        }
        let point = G2Projective::generator() * secret
            + G2Projective::from(h_generator) * h_blinding
            + G2Projective::from(v_generator) * v_blinding;
        if bool::from(point.is_identity()) {
            return Err(ThresholdBlsError::InvalidCoefficientCommitment);
        }
        commitment_bytes.push(point.to_affine().to_compressed());
    }
    let nonce_bytes = Zeroizing::new(random_nonzero_scalar_bytes(rng)?);
    let nonce = decode_scalar(&nonce_bytes)?;
    let proof_commitment = (G2Projective::generator() * nonce)
        .to_affine()
        .to_compressed();
    let parsed_coefficients = commitment_bytes
        .iter()
        .map(|bytes| DasRenCoefficientCommitment::from_bytes(parameters, *bytes))
        .collect::<Result<Vec<_>, _>>()?;
    let provisional =
        DasRenSchnorrPok::from_bytes(proof_commitment, Scalar::from(0_u64).to_bytes_be())?;
    let challenge =
        dealer_pok_challenge(parameters, dealer_index, &parsed_coefficients, &provisional)?;
    let constant = decode_scalar(&coefficients[0][0])?;
    let proof_response = (nonce + challenge * constant).to_bytes_be();
    let validated = DasRenDealerCommitment::verify(
        parameters,
        dealer_index,
        &commitment_bytes,
        proof_commitment,
        proof_response,
    )?;
    Ok((
        ReferenceDealerSecret {
            parameters_digest: parameters.digest(),
            session_id: *parameters.session().session_id(),
            dealer_index,
            coefficients,
            marker: PhantomData,
        },
        validated,
    ))
}

pub(super) struct TrackedRng {
    inner: ChaCha20Rng,
    fail_after: Option<usize>,
    pub(super) calls: usize,
    pub(super) bytes: usize,
}
impl TrackedRng {
    pub(super) fn new(seed: u8, fail_after: Option<usize>) -> Self {
        Self {
            inner: ChaCha20Rng::from_seed([seed; 32]),
            fail_after,
            calls: 0,
            bytes: 0,
        }
    }
}
impl TryRngCore for TrackedRng {
    type Error = &'static str;
    fn try_next_u32(&mut self) -> Result<u32, Self::Error> {
        let mut bytes = [0; 4];
        self.try_fill_bytes(&mut bytes)?;
        Ok(u32::from_le_bytes(bytes))
    }
    fn try_next_u64(&mut self) -> Result<u64, Self::Error> {
        let mut bytes = [0; 8];
        self.try_fill_bytes(&mut bytes)?;
        Ok(u64::from_le_bytes(bytes))
    }
    fn try_fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), Self::Error> {
        let call = self.calls;
        self.calls += 1;
        self.bytes += destination.len();
        if self.fail_after.is_some_and(|limit| call >= limit) {
            return Err("deterministic original RNG refusal");
        }
        self.inner
            .try_fill_bytes(destination)
            .map_err(|never| match never {})
    }
}
impl TryCryptoRng for TrackedRng {}

pub(super) fn parameters<P: ThresholdBlsPurpose>(n: u16) -> AdaptiveThresholdBlsParameters<P> {
    AdaptiveThresholdBlsParameters::derive(
        &ThresholdBlsSession::new(binding(1), binding(2), binding(3), n, (n - 1) / 3 + 1).unwrap(),
    )
    .unwrap()
}

pub(super) fn coefficients(
    threshold: u16,
    index: u16,
) -> Zeroizing<Vec<DasRenSecretCoefficientV1>> {
    Zeroizing::new(
        (0..threshold)
            .map(|degree| {
                [
                    Scalar::from(10 + u64::from(index) + 13 * u64::from(degree)).to_bytes_be(),
                    Scalar::from(if degree == 0 {
                        0
                    } else {
                        100 + 3 * u64::from(index) + u64::from(degree)
                    })
                    .to_bytes_be(),
                    Scalar::from(if degree == 0 {
                        0
                    } else {
                        200 + 5 * u64::from(index) + u64::from(degree)
                    })
                    .to_bytes_be(),
                ]
            })
            .collect(),
    )
}

pub(super) fn reference_generate<P: ThresholdBlsPurpose, R: TryCryptoRng + ?Sized>(
    parameters: &AdaptiveThresholdBlsParameters<P>,
    index: u16,
    rng: &mut R,
) -> Result<(ReferenceDealerSecret<P>, ValidatedDealerCommitment<P>), ThresholdBlsError> {
    validate_participant_index(parameters.session(), index)?;
    let mut coefficients = Zeroizing::new(Vec::with_capacity(usize::from(
        parameters.session().threshold(),
    )));
    for degree in 0..parameters.session().threshold() {
        coefficients.push([
            random_nonzero_scalar_bytes(rng)?,
            if degree == 0 {
                [0; 32]
            } else {
                random_nonzero_scalar_bytes(rng)?
            },
            if degree == 0 {
                [0; 32]
            } else {
                random_nonzero_scalar_bytes(rng)?
            },
        ]);
    }
    reference_import(parameters, index, coefficients, rng)
}

fn assert_same_rng(actual: &mut TrackedRng, expected: &mut TrackedRng) {
    assert_eq!(
        (actual.calls, actual.bytes),
        (expected.calls, expected.bytes)
    );
    let mut actual_tail = [0; 64];
    let mut expected_tail = [0; 64];
    actual.try_fill_bytes(&mut actual_tail).unwrap();
    expected.try_fill_bytes(&mut expected_tail).unwrap();
    assert_eq!(
        actual_tail, expected_tail,
        "same original CSPRNG stream position"
    );
}

#[test]
fn dealer_import_scratch_is_allocation_free_with_exact_reference_proofs_and_shares() {
    fn check<P: ThresholdBlsPurpose>() {
        for n in [4, THRESHOLD_BLS_MAX_COMMITTEE_SIZE_V1] {
            let parameters = parameters::<P>(n);
            let source = coefficients(parameters.session().threshold(), n);
            let reference_source = source.clone();
            let source = inline_coefficients(source);
            let mut actual_rng = TrackedRng::new(0x51, None);
            let mut reference_rng = TrackedRng::new(0x51, None);
            let (actual, allocations) = allocations_during(|| {
                DasRenDealerSecret::from_coefficients_with_rng(
                    &parameters,
                    n,
                    source,
                    &mut actual_rng,
                )
            });
            assert_eq!(allocations, 0, "both importer scratch buffers are inline");
            let (actual, commitment) = actual.unwrap();
            let backing_start = core::ptr::from_ref(&actual.coefficients) as usize;
            let prefix_start = actual.coefficients.as_ptr() as usize;
            assert!(prefix_start >= backing_start);
            assert!(
                prefix_start + core::mem::size_of_val(actual.coefficients.as_slice())
                    <= backing_start + core::mem::size_of::<DasRenSecretCoefficientsV1>()
            );
            assert_eq!(
                actual.coefficients.len(),
                usize::from(parameters.session().threshold())
            );
            assert_eq!(actual.coefficients.values.len(), MAX_DEALER_COEFFICIENTS_V1);
            let (expected, reference_allocations) = allocations_during(|| {
                reference_import(&parameters, n, reference_source, &mut reference_rng)
            });
            assert!(
                reference_allocations >= 2,
                "the independent former importer allocates both scratch Vecs, with possible Vec growth"
            );
            eprintln!(
                "dealer importer role={} n={n}: actual={allocations}, reference={reference_allocations}",
                P::ROLE_TAG
            );
            let (expected, reference_commitment) = expected.unwrap();
            assert_eq!(commitment, reference_commitment);
            assert_eq!(
                actual.coefficients.as_slice(),
                expected.coefficients.as_slice()
            );
            assert_eq!(actual.parameters_digest, expected.parameters_digest);
            assert_eq!(actual.session_id, expected.session_id);
            assert_same_rng(&mut actual_rng, &mut reference_rng);
            for index in 1..=n {
                let share =
                    without_allocations(|| actual.private_share(&parameters, &commitment, index))
                        .unwrap();
                let reference_share = expected
                    .private_share(&parameters, &reference_commitment, index)
                    .unwrap();
                assert_eq!(share.dealer_index(), reference_share.dealer_index());
                assert_eq!(share.recipient_index(), reference_share.recipient_index());
                assert_eq!(
                    *share.components_for_authenticated_encryption(),
                    *reference_share.components_for_authenticated_encryption()
                );
            }
        }
    }
    check::<BeaconPurpose>();
    check::<TleReleasePurpose>();
}

#[test]
fn dealer_generation_is_allocation_free_with_exact_zeroizing_retirement() {
    fn check<P: ThresholdBlsPurpose>() {
        for n in [4, THRESHOLD_BLS_MAX_COMMITTEE_SIZE_V1] {
            let parameters = parameters::<P>(n);
            let mut actual_rng = TrackedRng::new(0x52, None);
            let mut expected_rng = TrackedRng::new(0x52, None);
            let (generated, allocations) = allocations_during(|| {
                DasRenDealerSecret::generate_with_rng(&parameters, n, &mut actual_rng)
            });
            assert_eq!(
                allocations, 0,
                "original secret and every public scratch owner are inline"
            );
            let (actual, commitment) = generated.unwrap();
            let (expected, reference_allocations) =
                allocations_during(|| reference_generate(&parameters, n, &mut expected_rng));
            assert!(
                reference_allocations >= 3,
                "original secret vector plus both former scratch vectors"
            );
            eprintln!(
                "dealer generation role={} n={n}: actual={allocations}, reference={reference_allocations}",
                P::ROLE_TAG
            );
            let (expected, reference_commitment) = expected.unwrap();
            assert_eq!(commitment, reference_commitment);
            assert_eq!(
                actual.coefficients.as_slice(),
                expected.coefficients.as_slice()
            );
            assert_same_rng(&mut actual_rng, &mut expected_rng);
            assert_eq!(
                actual.coefficients.len(),
                usize::from(parameters.session().threshold())
            );
            let original_layout = expected.coefficients.capacity()
                * core::mem::size_of::<DasRenSecretCoefficientV1>();
            let (((), deallocations), retired) = observe_retirement(|| {
                with_deallocation_observation(original_layout, || drop(actual))
            });
            assert_eq!(
                deallocations, 0,
                "inline secret has no heap backing to deallocate"
            );
            assert_eq!(retired.drops, 1);
            assert!(
                retired.all_erased,
                "every live physical slot is erased before drop completes"
            );
            let ((), reference_deallocations) =
                with_deallocation_observation(original_layout, || drop(expected));
            assert_eq!(
                reference_deallocations, 1,
                "independent former Vec retires once"
            );
        }
    }
    check::<BeaconPurpose>();
    check::<TleReleasePurpose>();
}

#[test]
fn dealer_scratch_preserves_index_degree_scalar_point_and_rng_refusal_precedence() {
    fn check<P: ThresholdBlsPurpose>() {
        for n in [4, THRESHOLD_BLS_MAX_COMMITTEE_SIZE_V1] {
            let parameters = parameters::<P>(n);
            let valid = coefficients(parameters.session().threshold(), 1);
            let compare = |index,
                           source: Zeroizing<Vec<DasRenSecretCoefficientV1>>,
                           expected_error,
                           expected_rng_calls| {
                let reference_source = source.clone();
                let source = try_inline_coefficients(source);
                let mut actual_rng = TrackedRng::new(0x53, Some(0));
                let mut reference_rng = TrackedRng::new(0x53, Some(0));
                let actual = without_allocations(|| {
                    source.and_then(|source| {
                        DasRenDealerSecret::from_coefficients_with_rng(
                            &parameters,
                            index,
                            source,
                            &mut actual_rng,
                        )
                    })
                })
                .map(|_| ());
                let expected =
                    reference_import(&parameters, index, reference_source, &mut reference_rng)
                        .map(|_| ());
                assert_eq!(actual, Err(expected_error));
                assert_eq!(actual, expected);
                assert_eq!(actual_rng.calls, expected_rng_calls);
                assert_eq!(
                    (actual_rng.calls, actual_rng.bytes),
                    (reference_rng.calls, reference_rng.bytes)
                );
            };
            compare(
                0,
                Zeroizing::new(Vec::new()),
                ThresholdBlsError::InvalidParticipantIndex,
                0,
            );
            compare(
                n + 1,
                valid.clone(),
                ThresholdBlsError::InvalidParticipantIndex,
                0,
            );
            let mut short = valid.clone();
            short.pop();
            compare(1, short, ThresholdBlsError::InvalidCoefficientCommitment, 0);
            let mut long = valid.clone();
            long.push(valid[0]);
            compare(1, long, ThresholdBlsError::InvalidCoefficientCommitment, 0);
            let mut wrong_constant_blinding = valid.clone();
            wrong_constant_blinding[0][0] = [0xff; 32];
            wrong_constant_blinding[0][1] = Scalar::from(1).to_bytes_be();
            compare(
                1,
                wrong_constant_blinding,
                ThresholdBlsError::InvalidCoefficientCommitment,
                0,
            );
            let mut malformed_scalar = valid.clone();
            malformed_scalar[0][0] = [0xff; 32];
            compare(1, malformed_scalar, ThresholdBlsError::InvalidScalar, 0);
            let mut inert_constant = valid.clone();
            inert_constant[0][0] = [0; 32];
            compare(
                1,
                inert_constant,
                ThresholdBlsError::InvalidCoefficientCommitment,
                0,
            );
            let mut identity_point = valid.clone();
            identity_point[1] = [[0; 32]; 3];
            compare(
                1,
                identity_point,
                ThresholdBlsError::InvalidCoefficientCommitment,
                0,
            );
            compare(1, valid, ThresholdBlsError::RandomnessUnavailable, 1);
        }
    }
    check::<BeaconPurpose>();
    check::<TleReleasePurpose>();
}

#[test]
fn inline_generated_dealers_preserve_complete_threshold_signatures_for_both_roles() {
    const PAYLOAD: &[u8] = b"same-dealer-generated-threshold-signature";
    fn check<P: ThresholdBlsPurpose>() {
        for n in [4, THRESHOLD_BLS_MAX_COMMITTEE_SIZE_V1] {
            let parameters = parameters::<P>(n);
            let threshold = parameters.session().threshold();
            let q = n - (n - 1) / 3;
            let indices: Vec<_> = (1..=q).collect();
            let mut dealers = Vec::new();
            let mut reference_dealers = Vec::new();
            let mut source_coefficients = Vec::new();
            let mut group_constant = 0;
            for index in &indices {
                let source = coefficients(threshold, *index);
                group_constant += 10 + u64::from(*index);
                let mut actual_rng = TrackedRng::new(0x54, None);
                let mut expected_rng = TrackedRng::new(0x54, None);
                let imported = inline_coefficients(source.clone());
                let (actual, dealer) = without_allocations(|| {
                    DasRenDealerSecret::from_coefficients_with_rng(
                        &parameters,
                        *index,
                        imported,
                        &mut actual_rng,
                    )
                })
                .unwrap();
                let (_, reference) =
                    reference_import(&parameters, *index, source.clone(), &mut expected_rng)
                        .unwrap();
                assert_eq!(dealer, reference);
                assert_same_rng(&mut actual_rng, &mut expected_rng);
                drop(actual);
                dealers.push(dealer);
                reference_dealers.push(reference);
                source_coefficients.push(source);
            }
            let transcript = AdaptiveThresholdBlsPublicTranscript::from_qualified_dealers(
                &parameters,
                &dealers,
                &indices,
                binding(91),
            )
            .unwrap();
            let reference = AdaptiveThresholdBlsPublicTranscript::from_qualified_dealers(
                &parameters,
                &reference_dealers,
                &indices,
                binding(91),
            )
            .unwrap();
            assert_eq!(transcript, reference);
            let mut partials = Vec::new();
            for index in n - threshold + 1..=n {
                let x = Scalar::from(u64::from(index));
                let components: [Scalar; 3] = core::array::from_fn(|component| {
                    source_coefficients
                        .iter()
                        .fold(Scalar::from(0), |sum, dealer| {
                            sum + dealer
                                .iter()
                                .rev()
                                .fold(Scalar::from(0), |value, coefficient| {
                                    value * x + decode_scalar(&coefficient[component]).unwrap()
                                })
                        })
                });
                let share = AdaptiveThresholdBlsSecretShare::from_components(
                    &transcript,
                    index,
                    components[0].to_bytes_be(),
                    components[1].to_bytes_be(),
                    components[2].to_bytes_be(),
                )
                .unwrap();
                let reference_share = AdaptiveThresholdBlsSecretShare::from_components(
                    &reference,
                    index,
                    components[0].to_bytes_be(),
                    components[1].to_bytes_be(),
                    components[2].to_bytes_be(),
                )
                .unwrap();
                let mut actual_rng = TrackedRng::new(0x55, None);
                let mut expected_rng = TrackedRng::new(0x55, None);
                let partial = share
                    .sign_payload_with_rng(&transcript, PAYLOAD, &mut actual_rng)
                    .unwrap();
                let expected = reference_share
                    .sign_payload_with_rng(&reference, PAYLOAD, &mut expected_rng)
                    .unwrap();
                assert_eq!(partial, expected);
                assert_same_rng(&mut actual_rng, &mut expected_rng);
                partials.push(partial);
            }
            let actual =
                without_allocations(|| transcript.combine_partial_signatures(PAYLOAD, &partials))
                    .unwrap();
            let expected = sign(parameters.session(), group_constant, PAYLOAD);
            assert_eq!(actual.as_bytes(), expected.as_bytes());
            transcript
                .group_public_key()
                .verify_payload(parameters.session(), PAYLOAD, &actual)
                .unwrap();
        }
    }
    check::<BeaconPurpose>();
    check::<TleReleasePurpose>();
}
