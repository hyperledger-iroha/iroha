use std::sync::OnceLock;

use super::*;
use crate::{
    generalized_bulletproof::{ProofGenerators, ProofRandomSource, ProofScalar},
    vega::{derive_t256_generators_v1, sponge::keccak256},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct TinySuiteV1;

impl ProofSuite for TinySuiteV1 {
    type Scalar = Scalar;
    type Point = Point;

    fn generators() -> &'static ProofGenerators<Self> {
        static GENERATORS: OnceLock<ProofGenerators<TinySuiteV1>> = OnceLock::new();
        GENERATORS.get_or_init(|| {
            let one = |label: &[u8]| {
                derive_t256_generators_v1(label, 1)
                    .expect("valid tiny generator")
                    .pop()
                    .expect("one tiny generator")
            };
            ProofGenerators::new(
                one(b"global-inverse-product-test-g"),
                one(b"global-inverse-product-test-h"),
                derive_t256_generators_v1(b"global-inverse-product-test-G", 16)
                    .expect("tiny G basis"),
                derive_t256_generators_v1(b"global-inverse-product-test-H", 16)
                    .expect("tiny H basis"),
            )
            .expect("independent tiny basis")
        })
    }
}

#[derive(Clone)]
struct TinySourceV1 {
    a: Vec<Vec<Scalar>>,
    u: Vec<Vec<Scalar>>,
    a_masks: Vec<Scalar>,
    u_masks: Vec<Scalar>,
    inverse_product_mask_values: Vec<Scalar>,
    inverse_product_mask_blinding: Scalar,
}

impl Drop for TinySourceV1 {
    fn drop(&mut self) {
        for plane in self.a.iter_mut().chain(&mut self.u) {
            for value in plane {
                value.clear_secret();
            }
        }
        for values in [
            &mut self.a_masks,
            &mut self.u_masks,
            &mut self.inverse_product_mask_values,
        ] {
            for value in values {
                value.clear_secret();
            }
        }
        self.inverse_product_mask_blinding.clear_secret();
    }
}

impl RnsNativeGlobalInverseProductOpeningSourceV1 for TinySourceV1 {
    fn replay_active_plane_values_v1(
        &mut self,
        ordinal: usize,
        coordinate_prefix: &[Scalar],
        a_values: &mut [Scalar],
        u_values: &mut [Scalar],
    ) -> Result<(), RnsNativeGlobalInverseProductErrorV1> {
        let a = self
            .a
            .get(ordinal)
            .ok_or(RnsNativeGlobalInverseProductErrorV1::SourceUnavailable)?;
        let u = self
            .u
            .get(ordinal)
            .ok_or(RnsNativeGlobalInverseProductErrorV1::SourceUnavailable)?;
        let expected = a.len() >> coordinate_prefix.len();
        if a.len() != u.len() || a_values.len() != expected || u_values.len() != expected {
            return Err(RnsNativeGlobalInverseProductErrorV1::SourceUnavailable);
        }
        let mut folded_a = SecretScalarsV1(a.clone());
        let mut folded_u = SecretScalarsV1(u.clone());
        if fold_prefix_v1(folded_a.as_mut_slice_v1(), coordinate_prefix)? != expected
            || fold_prefix_v1(folded_u.as_mut_slice_v1(), coordinate_prefix)? != expected
        {
            return Err(RnsNativeGlobalInverseProductErrorV1::SourceUnavailable);
        }
        a_values.copy_from_slice(&folded_a.as_slice_v1()[..expected]);
        u_values.copy_from_slice(&folded_u.as_slice_v1()[..expected]);
        Ok(())
    }

    fn take_active_plane_opening_v1(
        &mut self,
        ordinal: usize,
        a_values: &mut [Scalar],
        u_values: &mut [Scalar],
        a_commitment_mask: &mut Scalar,
        u_commitment_mask: &mut Scalar,
    ) -> Result<(), RnsNativeGlobalInverseProductErrorV1> {
        self.replay_active_plane_values_v1(ordinal, &[], a_values, u_values)?;
        let a_mask = self
            .a_masks
            .get_mut(ordinal)
            .ok_or(RnsNativeGlobalInverseProductErrorV1::SourceUnavailable)?;
        let u_mask = self
            .u_masks
            .get_mut(ordinal)
            .ok_or(RnsNativeGlobalInverseProductErrorV1::SourceUnavailable)?;
        core::mem::swap(a_commitment_mask, a_mask);
        core::mem::swap(u_commitment_mask, u_mask);
        Ok(())
    }

    fn take_inverse_product_mask_opening_v1(
        &mut self,
        values: &mut [Scalar],
        commitment_mask: &mut Scalar,
    ) -> Result<(), RnsNativeGlobalInverseProductErrorV1> {
        if values.len() != self.inverse_product_mask_values.len() {
            return Err(RnsNativeGlobalInverseProductErrorV1::SourceUnavailable);
        }
        values.copy_from_slice(&self.inverse_product_mask_values);
        core::mem::swap(commitment_mask, &mut self.inverse_product_mask_blinding);
        Ok(())
    }
}

struct KatRandomV1 {
    seed: [u8; 32],
    counter: u64,
}

impl KatRandomV1 {
    fn new(label: &[u8]) -> Self {
        Self {
            seed: keccak256(label),
            counter: 0,
        }
    }
}

impl ProofRandomSource for KatRandomV1 {
    fn fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), GeneralizedBulletproofErrorV1> {
        let mut written = 0;
        while written < destination.len() {
            let mut input = Vec::with_capacity(40);
            input.extend_from_slice(&self.seed);
            input.extend_from_slice(&self.counter.to_be_bytes());
            let block = keccak256(&input);
            self.counter = self
                .counter
                .checked_add(1)
                .ok_or(GeneralizedBulletproofErrorV1::RandomnessUnavailable)?;
            let take = (destination.len() - written).min(block.len());
            destination[written..written + take].copy_from_slice(&block[..take]);
            written += take;
        }
        Ok(())
    }
}

const TINY_GEOMETRY_V1: KernelGeometryV1 = KernelGeometryV1 {
    active_planes: 1,
    padded_planes: 2,
    coordinates: 16,
    coordinate_bits: 4,
    plane_bits: 1,
};

fn commit_vector_v1(values: &[Scalar], mask: Scalar) -> Point {
    let generators = TinySuiteV1::generators()
        .reduce(TINY_GEOMETRY_V1.coordinates)
        .expect("tiny reduced basis");
    let mut terms: Vec<(Scalar, Point)> = values
        .iter()
        .copied()
        .zip(generators.g_bold.iter().copied())
        .collect();
    terms.push((mask, generators.h));
    let commitment = multiexp::<TinySuiteV1>(&terms);
    assert!(!commitment.is_identity());
    commitment
}

struct TinyFixtureV1 {
    context: KernelContextV1,
    predecessor_binding_digest: [u8; DIGEST_BYTES_V1],
    a_commitments: Vec<Point>,
    u_commitments: Vec<Point>,
    inverse_product_mask_commitment: Point,
    source: TinySourceV1,
}

fn tiny_fixture_v1() -> TinyFixtureV1 {
    TINY_GEOMETRY_V1.validate_v1().expect("tiny geometry");
    let z = Scalar::from_u64(211);
    let mut a = Vec::new();
    let mut u = Vec::new();
    let mut a_masks = Vec::new();
    let mut u_masks = Vec::new();
    let mut a_commitments = Vec::new();
    let mut u_commitments = Vec::new();
    for plane in 0..TINY_GEOMETRY_V1.active_planes {
        let mut a_plane = Vec::new();
        let mut u_plane = Vec::new();
        for coordinate in 0..TINY_GEOMETRY_V1.coordinates {
            let value = Scalar::from_u64((1 + plane * 37 + coordinate) as u64);
            a_plane.push(value);
            u_plane.push((z - value).invert().expect("fixture value differs from z"));
        }
        let a_mask = Scalar::from_u64(1_001 + plane as u64);
        let u_mask = Scalar::from_u64(2_001 + plane as u64);
        a_commitments.push(commit_vector_v1(&a_plane, a_mask));
        u_commitments.push(commit_vector_v1(&u_plane, u_mask));
        a.push(a_plane);
        u.push(u_plane);
        a_masks.push(a_mask);
        u_masks.push(u_mask);
    }
    let inverse_product_mask_values: Vec<Scalar> = (0..TINY_GEOMETRY_V1.mask_scalars_v1().unwrap())
        .map(|index| Scalar::from_u64(3_001 + index as u64))
        .collect();
    let inverse_product_mask_blinding = Scalar::from_u64(4_001);
    let inverse_product_mask_commitment =
        commit_vector_v1(&inverse_product_mask_values, inverse_product_mask_blinding);
    TinyFixtureV1 {
        context: KernelContextV1 {
            pre_z_binding_digest: keccak256(b"tiny-pre-z-binding"),
            post_z_transcript_digest: keccak256(b"tiny-post-z-transcript"),
            z,
        },
        predecessor_binding_digest: keccak256(b"tiny-predecessor-binding"),
        a_commitments,
        u_commitments,
        inverse_product_mask_commitment,
        source: TinySourceV1 {
            a,
            u,
            a_masks,
            u_masks,
            inverse_product_mask_values,
            inverse_product_mask_blinding,
        },
    }
}

fn prove_tiny_v1(fixture: &TinyFixtureV1, source: TinySourceV1) -> Vec<u8> {
    let commitments = KernelCommitmentsV1 {
        a: &fixture.a_commitments,
        u: &fixture.u_commitments,
        inverse_product_mask: fixture.inverse_product_mask_commitment,
    };
    prove_pending_kernel_for_suite_v1::<TinySuiteV1, _, _>(
        TINY_GEOMETRY_V1,
        fixture.context,
        commitments,
        source,
        &mut KatRandomV1::new(b"tiny-global-inverse-product-proof"),
    )
    .and_then(|pending| pending.seal_v1(b"downstream"))
    .expect("valid tiny proof")
}

fn cached_tiny_wire_v1() -> &'static [u8] {
    static WIRE: OnceLock<Vec<u8>> = OnceLock::new();
    WIRE.get_or_init(|| {
        let fixture = tiny_fixture_v1();
        prove_tiny_v1(&fixture, fixture.source.clone())
    })
}

fn verify_tiny_v1<'a>(
    fixture: &TinyFixtureV1,
    wire: &'a [u8],
) -> Result<VerifiedKernelV1<'a>, RnsNativeGlobalInverseProductErrorV1> {
    verify_kernel_for_suite_v1::<TinySuiteV1>(
        TINY_GEOMETRY_V1,
        fixture.context,
        fixture.predecessor_binding_digest,
        KernelCommitmentsV1 {
            a: &fixture.a_commitments,
            u: &fixture.u_commitments,
            inverse_product_mask: fixture.inverse_product_mask_commitment,
        },
        wire,
        64 * 1024,
    )
}

#[test]
fn compact_kernel_roundtrips_and_authenticates_every_stage() {
    let fixture = tiny_fixture_v1();
    let wire = cached_tiny_wire_v1();
    let verified = verify_tiny_v1(&fixture, wire).expect("valid compact proof");
    assert_eq!(verified.residual, b"downstream");
    assert!(!verified.rho.is_zero());
    assert_eq!(
        verified.u_sum_commitment,
        fixture
            .u_commitments
            .iter()
            .copied()
            .fold(Point::identity(), |sum, point| sum + point)
    );
    for digest in [
        verified.sumcheck_transcript_digest,
        verified.endpoint_transcript_digest,
        verified.residual_digest,
        verified.binding_digest,
    ] {
        assert_ne!(digest, [0; 32]);
    }
    let view = ProofViewV1::decode_v1(wire, TINY_GEOMETRY_V1, 64 * 1024).expect("canonical view");
    assert_eq!(view.messages.len(), TINY_GEOMETRY_V1.rounds_v1() * 96);
    assert_eq!(
        view.endpoint_core.len(),
        endpoint_core_bytes_v1(TINY_GEOMETRY_V1).unwrap()
    );
}

#[test]
fn active_u_sum_is_exact_nonidentity_and_excludes_virtual_padding() {
    let fixture = tiny_fixture_v1();
    let derived = derive_active_u_sum_commitment_v1(TINY_GEOMETRY_V1, &fixture.u_commitments)
        .expect("exact active U sum");
    assert_eq!(derived, fixture.u_commitments[0]);
    assert!(matches!(
        derive_active_u_sum_commitment_v1(TINY_GEOMETRY_V1, &[]),
        Err(RnsNativeGlobalInverseProductErrorV1::InvalidGeometry)
    ));
    assert!(matches!(
        derive_active_u_sum_commitment_v1(TINY_GEOMETRY_V1, &[Point::identity()]),
        Err(RnsNativeGlobalInverseProductErrorV1::InvalidPoint)
    ));
}

#[test]
fn current_frame_binding_is_post_verification_only_and_never_fiat_shamir_input() {
    let fixture = tiny_fixture_v1();
    let wire = cached_tiny_wire_v1();
    let first = verify_tiny_v1(&fixture, wire).expect("first predecessor binding");
    let second_predecessor = keccak256(b"different-current-frame-binding");
    let second = verify_kernel_for_suite_v1::<TinySuiteV1>(
        TINY_GEOMETRY_V1,
        fixture.context,
        second_predecessor,
        KernelCommitmentsV1 {
            a: &fixture.a_commitments,
            u: &fixture.u_commitments,
            inverse_product_mask: fixture.inverse_product_mask_commitment,
        },
        wire,
        64 * 1024,
    )
    .expect("proof transcript excludes current-frame binding");
    assert_eq!(first.rho, second.rho);
    assert_eq!(
        first.sumcheck_transcript_digest,
        second.sumcheck_transcript_digest
    );
    assert_eq!(
        first.endpoint_transcript_digest,
        second.endpoint_transcript_digest
    );
    assert_ne!(first.residual_digest, second.residual_digest);
    assert_ne!(first.binding_digest, second.binding_digest);
    assert!(matches!(
        verify_kernel_for_suite_v1::<TinySuiteV1>(
            TINY_GEOMETRY_V1,
            fixture.context,
            [0; DIGEST_BYTES_V1],
            KernelCommitmentsV1 {
                a: &fixture.a_commitments,
                u: &fixture.u_commitments,
                inverse_product_mask: fixture.inverse_product_mask_commitment,
            },
            wire,
            64 * 1024,
        ),
        Err(RnsNativeGlobalInverseProductErrorV1::InvalidContext)
    ));
}

#[test]
fn transcript_message_endpoint_and_codec_mutations_fail_closed() {
    let fixture = tiny_fixture_v1();
    let wire = cached_tiny_wire_v1();
    let mut changed_message = wire.to_vec();
    changed_message[HEADER_BYTES_V1] ^= 1;
    let digest_offset = changed_message.len() - DIGEST_BYTES_V1;
    let digest = codec_digest_v1(&changed_message[..digest_offset]);
    changed_message[digest_offset..].copy_from_slice(&digest);
    assert!(verify_tiny_v1(&fixture, &changed_message).is_err());

    let mut changed_endpoint = wire.to_vec();
    let endpoint_offset = HEADER_BYTES_V1 + TINY_GEOMETRY_V1.rounds_v1() * 96;
    changed_endpoint[endpoint_offset + 17] ^= 1;
    let digest_offset = changed_endpoint.len() - DIGEST_BYTES_V1;
    let digest = codec_digest_v1(&changed_endpoint[..digest_offset]);
    changed_endpoint[digest_offset..].copy_from_slice(&digest);
    assert!(verify_tiny_v1(&fixture, &changed_endpoint).is_err());

    let mut noncanonical_scalar = wire.to_vec();
    noncanonical_scalar[HEADER_BYTES_V1..HEADER_BYTES_V1 + SCALAR_BYTES_V1].fill(0xff);
    let digest_offset = noncanonical_scalar.len() - DIGEST_BYTES_V1;
    let digest = codec_digest_v1(&noncanonical_scalar[..digest_offset]);
    noncanonical_scalar[digest_offset..].copy_from_slice(&digest);
    assert!(matches!(
        verify_tiny_v1(&fixture, &noncanonical_scalar),
        Err(RnsNativeGlobalInverseProductErrorV1::InvalidScalar)
    ));

    let mut changed_codec = wire.to_vec();
    let last = changed_codec.len() - 1;
    changed_codec[last] ^= 1;
    assert!(matches!(
        verify_tiny_v1(&fixture, &changed_codec),
        Err(RnsNativeGlobalInverseProductErrorV1::InvalidIntegrity)
    ));

    assert!(matches!(
        ProofViewV1::decode_v1(&changed_codec, TINY_GEOMETRY_V1, changed_codec.len() - 1),
        Err(RnsNativeGlobalInverseProductErrorV1::ProofCapExceeded)
    ));
    assert!(
        ProofViewV1::decode_v1(
            &changed_codec[..changed_codec.len() - 1],
            TINY_GEOMETRY_V1,
            64 * 1024,
        )
        .is_err()
    );
}

#[test]
fn commitment_splicing_and_incorrect_inverse_openings_are_rejected() {
    let fixture = tiny_fixture_v1();
    let wire = cached_tiny_wire_v1();
    let mut spliced = tiny_fixture_v1();
    spliced.a_commitments[0] = spliced.u_commitments[0];
    assert!(verify_tiny_v1(&spliced, wire).is_err());
    let mut spliced_inverse_product_mask = tiny_fixture_v1();
    spliced_inverse_product_mask.inverse_product_mask_commitment =
        spliced_inverse_product_mask.a_commitments[0];
    assert!(verify_tiny_v1(&spliced_inverse_product_mask, wire).is_err());

    let mut invalid_source = fixture.source.clone();
    invalid_source.u[0][7] += Scalar::one();
    assert!(
        prove_pending_kernel_for_suite_v1::<TinySuiteV1, _, _>(
            TINY_GEOMETRY_V1,
            fixture.context,
            KernelCommitmentsV1 {
                a: &fixture.a_commitments,
                u: &fixture.u_commitments,
                inverse_product_mask: fixture.inverse_product_mask_commitment,
            },
            invalid_source,
            &mut KatRandomV1::new(b"invalid-inverse-proof"),
        )
        .is_err()
    );
}

#[test]
fn virtual_padding_is_literal_inverse_relation_and_is_commitment_bound() {
    let fixture = tiny_fixture_v1();
    let z_inverse = fixture.context.z.invert().unwrap();
    assert_eq!(
        fixture.context.z * z_inverse - Scalar::one(),
        Scalar::zero()
    );
    let plane_point = [Scalar::from_u64(7)];
    let (_, folded_u, weights) = folded_endpoint_commitments_v1::<TinySuiteV1>(
        TINY_GEOMETRY_V1,
        KernelCommitmentsV1 {
            a: &fixture.a_commitments,
            u: &fixture.u_commitments,
            inverse_product_mask: fixture.inverse_product_mask_commitment,
        },
        z_inverse,
        &plane_point,
    )
    .expect("folded commitments");
    let padding_weight = weights[TINY_GEOMETRY_V1.active_planes..]
        .iter()
        .copied()
        .fold(Scalar::zero(), |sum, value| sum + value);
    assert!(!padding_weight.is_zero());
    let active_only = fold_commitment_v1::<TinySuiteV1>(
        &fixture.u_commitments,
        &weights[..TINY_GEOMETRY_V1.active_planes],
    )
    .unwrap();
    assert_ne!(folded_u, active_only);
}

#[test]
fn mask_telescope_and_terminal_functional_are_exact() {
    let challenges = [
        Scalar::from_u64(5),
        Scalar::from_u64(7),
        Scalar::from_u64(9),
    ];
    let masks = [
        Scalar::from_u64(11),
        Scalar::from_u64(13),
        Scalar::from_u64(17),
        Scalar::from_u64(19),
        Scalar::from_u64(23),
        Scalar::from_u64(29),
        Scalar::from_u64(31),
        Scalar::from_u64(37),
        Scalar::from_u64(41),
    ];
    let mut carry = Scalar::zero();
    for (round, challenge) in challenges.into_iter().enumerate() {
        let polynomial = apply_round_mask_v1(
            [Scalar::zero(); 4],
            carry,
            [masks[3 * round], masks[3 * round + 1], masks[3 * round + 2]],
        )
        .unwrap();
        assert_eq!(
            polynomial[0] + evaluate_cubic_v1(polynomial, Scalar::one()),
            carry
        );
        carry = evaluate_cubic_v1(polynomial, challenge);
    }
    let weights = mask_terminal_weights_v1(&challenges).unwrap();
    let functional = masks
        .into_iter()
        .zip(weights)
        .fold(Scalar::zero(), |sum, (value, weight)| sum + value * weight);
    assert_eq!(functional, carry);
}

#[test]
fn equality_weights_follow_adjacent_little_endian_fold_order() {
    let r0 = Scalar::from_u64(3);
    let r1 = Scalar::from_u64(5);
    let weights = eq_weights_v1(&[r0, r1]).unwrap();
    assert_eq!(
        weights,
        vec![
            (Scalar::one() - r0) * (Scalar::one() - r1),
            r0 * (Scalar::one() - r1),
            (Scalar::one() - r0) * r1,
            r0 * r1,
        ]
    );
}

#[test]
fn rho_endpoint_factorization_matches_the_full_power_table() {
    let rho = Scalar::from_u64(17);
    let point = [
        Scalar::from_u64(3),
        Scalar::from_u64(5),
        Scalar::from_u64(7),
        Scalar::from_u64(11),
        Scalar::from_u64(13),
    ];
    let table_len = TINY_GEOMETRY_V1.coordinates * TINY_GEOMETRY_V1.padded_planes;
    let mut table = Vec::with_capacity(table_len);
    let mut power = Scalar::one();
    for _ in 0..table_len {
        table.push(power);
        power *= rho;
    }
    assert_eq!(
        rho_endpoint_evaluation_v1(TINY_GEOMETRY_V1, rho, &point).unwrap(),
        multilinear_evaluate_v1(&table, &point).unwrap()
    );
}

#[test]
fn production_codec_and_soundness_accounting_are_exact_and_fail_closed() {
    assert_eq!(PADDING_PLANES_V1, 360);
    assert_eq!(MESSAGE_BYTES_V1, 2_784);
    assert_eq!(ENDPOINT_CORE_BYTES_V1, 1_513);
    assert_eq!(OWNED_WIRE_BYTES_V1, 4_369);
    assert_eq!(MIN_WIRE_BYTES_V1, 4_370);
    assert_eq!(
        RNS_NATIVE_GLOBAL_INVERSE_PRODUCT_RESIDUAL_MAX_BYTES_V1,
        110_115
    );
    const {
        assert!(INVERSE_PRODUCT_RELATION_VERIFIED_V1);
        assert!(!LOOKUP_MEMBERSHIP_VERIFIED_V1);
        assert!(!CROSS_FIELD_GLOBAL_LOOKUP_VERIFIED_V1);
        assert!(!RELEASE_READY_V1);
    }

    let messages = vec![0_u8; MESSAGE_BYTES_V1];
    let endpoint = vec![0_u8; ENDPOINT_CORE_BYTES_V1];
    let downstream = vec![1_u8; RNS_NATIVE_GLOBAL_INVERSE_PRODUCT_RESIDUAL_MAX_BYTES_V1];
    let exact = encode_wire_v1(
        KernelGeometryV1::PRODUCTION,
        &messages,
        &endpoint,
        &downstream,
    )
    .expect("exact parent cap");
    assert_eq!(exact.len(), PARENT_RESIDUAL_CAP_BYTES_V1);
    let one_too_many = vec![1_u8; downstream.len() + 1];
    assert!(matches!(
        encode_wire_v1(
            KernelGeometryV1::PRODUCTION,
            &messages,
            &endpoint,
            &one_too_many,
        ),
        Err(RnsNativeGlobalInverseProductErrorV1::ProofCapExceeded)
    ));

    let source = include_str!("rns_native_global_inverse_product_sumcheck.rs");
    for required in [
        "padding-A=0;padding-U=z^-1",
        "rho-nonzero-after-all-A-U-commitments",
        "per-fresh-transcript:batching-error<=(2^29-1)/(pT-1)",
        "48*2^-256",
        "standard-Keccak-ROM-query-loss",
        "exclude-current-frame-residual-and-binding-from-all-challenges",
        "dedicated-inverse-product-mask-distinct-from-global-lookup-mask",
        "pub(super) trait RnsNativeGlobalInverseProductOpeningSourceV1",
        "fn replay_active_plane_values_v1(",
        "fn take_active_plane_opening_v1(",
        "fn take_inverse_product_mask_opening_v1(",
        "impl Drop for SecretScalarV1",
        "VectorCommitmentOpening::take_mask_from_slot",
        "inverse_product_mask: previous.inverse_product_mask()",
        "u_sum_commitment: verified.u_sum_commitment",
        "pub(in super::super) const fn u_sum_commitment(&self) -> Point",
        "pub(super) struct PendingInverseCoreV1",
        "pub(super) struct ActiveUSumOpeningV1",
        "u_sum_values.as_mut_slice_v1()[index] += u_buffer.as_slice_v1()[index]",
        "u_sum_mask.add_assign_v1(u_mask.as_ref_v1())",
        "prove_pending_kernel_for_suite_v1",
        "#[path = \"rns_native_global_membership_direct.rs\"]",
        "pub(super) mod rns_native_global_membership_direct;",
    ] {
        assert!(
            source.contains(required),
            "missing source guard: {required}"
        );
    }
    assert!(!source.contains("fn prove_kernel_for_suite_v1"));
    assert_eq!(
        source
            .matches("source.replay_active_plane_values_v1(")
            .count(),
        2
    );
    assert_eq!(
        source
            .matches("source.take_active_plane_opening_v1(")
            .count(),
        1
    );
    assert_eq!(
        source
            .matches("source.take_inverse_product_mask_opening_v1(")
            .count(),
        1
    );
    assert!(!source.contains("previous.sumcheck_mask()"));
    assert!(!source.contains("fn replay_active_plane_v1("));
    assert!(!source.contains("LOOKUP_MEMBERSHIP_VERIFIED_V1: bool = true"));
    assert!(!source.contains("RELEASE_READY_V1: bool = true"));
}
