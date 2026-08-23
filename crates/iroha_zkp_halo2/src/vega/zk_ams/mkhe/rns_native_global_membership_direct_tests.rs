use std::sync::{
    Arc, OnceLock,
    atomic::{AtomicUsize, Ordering},
};

use super::*;
use crate::{
    generalized_bulletproof::{ProofGenerators, ProofRandomSource, ProofScalar, multiexp},
    vega::{derive_t256_generators_v1, sponge::keccak256},
};

use super::super::super::super::rns_native_transcript::ZkAmsMkheRnsNativePreGlobalLookupCapabilityV1;

#[test]
fn clean_global_root_v2_has_the_exact_acyclic_frame_kat() {
    let capability = ZkAmsMkheRnsNativePreGlobalLookupCapabilityV1::test_fixture_v1(
        [1; DIGEST_BYTES_V1],
        [2; DIGEST_BYTES_V1],
    )
    .expect("pre-global capability");
    let pre_global_capability_digest = capability
        .sole_z_binding_digest_v1()
        .expect("sole-z binding");
    assert_eq!(
        pre_global_capability_digest,
        [
            0x75, 0x16, 0x33, 0xbb, 0x9a, 0x2e, 0x68, 0x02, 0xb3, 0x8d, 0xb5, 0xda, 0xe0, 0xba,
            0x84, 0x58, 0x21, 0xb2, 0x2b, 0x78, 0x44, 0x2d, 0x1d, 0x2e, 0x55, 0x22, 0x62, 0xd3,
            0x60, 0xc0, 0xd1, 0x22,
        ]
    );
    let root = verified_global_lookup_core_root_v2(RnsNativeGlobalLookupCleanCoreV2 {
        pre_global_capability_digest,
        pre_z_binding_digest: [3; DIGEST_BYTES_V1],
        z: [4; SCALAR_BYTES_V1],
        post_z_transcript_digest: [5; DIGEST_BYTES_V1],
        existing_inverse_root: [6; DIGEST_BYTES_V1],
        added_inverse_root: [7; DIGEST_BYTES_V1],
        alias_root: [8; DIGEST_BYTES_V1],
        global_inverse_root: [9; DIGEST_BYTES_V1],
        inverse_rho_challenge_digest: [10; DIGEST_BYTES_V1],
        inverse_sumcheck_transcript_digest: [11; DIGEST_BYTES_V1],
        inverse_endpoint_transcript_digest: [12; DIGEST_BYTES_V1],
        u_sum: [13; POINT_BYTES_V1],
        multiplicity: [14; POINT_BYTES_V1],
        membership_transcript_digest: [15; DIGEST_BYTES_V1],
        chronology_tag: capability
            .global_lookup_chronology_tag_v2()
            .expect("chronology tag"),
    })
    .expect("clean global root");
    assert_eq!(
        root.root,
        [
            0xfb, 0x86, 0xc6, 0x87, 0xc8, 0xeb, 0x47, 0x6e, 0x04, 0xa6, 0x6f, 0x08, 0xa5, 0x35,
            0x38, 0xcf, 0x09, 0x15, 0xb7, 0x0b, 0x09, 0x35, 0x38, 0xa4, 0x0f, 0xa6, 0xb2, 0x9c,
            0x23, 0x55, 0xbe, 0x28,
        ]
    );
    assert_eq!(VERIFIED_GLOBAL_LOOKUP_CORE_ROOT_PREIMAGE_BYTES_V2, 800);
}

#[test]
fn clean_global_root_v2_surface_excludes_every_cyclic_or_successor_binding() {
    let source = include_str!("rns_native_global_membership_direct.rs");
    let hash = source
        .split_once("fn verified_global_lookup_core_root_v2(")
        .expect("clean-root hash")
        .1
        .split_once("fn challenge_outside_table_v1")
        .expect("clean-root hash boundary")
        .0;
    let mut previous = 0;
    for (index, label) in [
        "pre-global-capability",
        "pre-z-binding",
        "z",
        "post-z-transcript",
        "existing-inverse-root",
        "added-inverse-root",
        "alias-root",
        "global-inverse-root",
        "inverse-rho",
        "inverse-sumcheck",
        "inverse-endpoint",
        "u-sum",
        "multiplicity",
        "membership-transcript",
    ]
    .into_iter()
    .enumerate()
    {
        let position = hash
            .find(&format!("b\"{label}\""))
            .unwrap_or_else(|| panic!("clean-root label missing: {label}"));
        assert!(
            index == 0 || previous < position,
            "clean-root order: {label}"
        );
        previous = position;
    }
    for forbidden in [
        "claimed_global",
        "zero_padding_root",
        "terminal_transcript_digest",
        "prior_context_digest",
        "inventory_root",
        "continuation_digest",
        "codec_digest",
        "residual_digest",
        "successor",
        "direct_core",
        "source_packing",
    ] {
        assert!(
            !hash.contains(forbidden),
            "cyclic clean-root input: {forbidden}"
        );
    }
    assert!(source.contains("VERIFIED_GLOBAL_LOOKUP_CORE_ROOT_PREIMAGE_BYTES_V2: usize = 800"));
    assert!(source.contains("membership.u_sum_commitment() != inverse.u_sum_commitment()"));
    const {
        assert!(!MULTIPLICITY_NONNEGATIVE_RANGE_VERIFIED_V1);
        assert!(!CROSS_FIELD_GLOBAL_LOOKUP_VERIFIED_V1);
        assert!(!RELEASE_READY_V1);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct TinyMembershipSuiteV1;

impl ProofSuite for TinyMembershipSuiteV1 {
    type Scalar = Scalar;
    type Point = Point;

    fn generators() -> &'static ProofGenerators<Self> {
        static GENERATORS: OnceLock<ProofGenerators<TinyMembershipSuiteV1>> = OnceLock::new();
        GENERATORS.get_or_init(|| {
            let points =
                derive_t256_generators_v1(b"rns-native-global-membership-direct-tiny-suite-v1", 18)
                    .expect("tiny membership generators");
            ProofGenerators::new(
                points[0],
                points[1],
                points[2..10].to_vec(),
                points[10..18].to_vec(),
            )
            .expect("valid tiny membership basis")
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct TinyCombinedSuiteV1;

impl ProofSuite for TinyCombinedSuiteV1 {
    type Scalar = Scalar;
    type Point = Point;

    fn generators() -> &'static ProofGenerators<Self> {
        static GENERATORS: OnceLock<ProofGenerators<TinyCombinedSuiteV1>> = OnceLock::new();
        GENERATORS.get_or_init(|| {
            let points = derive_t256_generators_v1(
                b"rns-native-global-inverse-membership-combined-tiny-suite-v1",
                34,
            )
            .expect("tiny combined generators");
            ProofGenerators::new(
                points[0],
                points[1],
                points[2..18].to_vec(),
                points[18..34].to_vec(),
            )
            .expect("valid tiny combined basis")
        })
    }
}

struct KatRandomV1 {
    seed: [u8; DIGEST_BYTES_V1],
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
            let count = (destination.len() - written).min(block.len());
            destination[written..written + count].copy_from_slice(&block[..count]);
            written += count;
        }
        Ok(())
    }
}

const TINY_GEOMETRY_V1: MembershipGeometryV1 = MembershipGeometryV1 {
    active_planes: 3,
    u_coordinates: 4,
    table_values: 8,
};

const TINY_COMBINED_INVERSE_GEOMETRY_V1: super::super::KernelGeometryV1 =
    super::super::KernelGeometryV1 {
        active_planes: 1,
        padded_planes: 2,
        coordinates: 16,
        coordinate_bits: 4,
        plane_bits: 1,
    };
const TINY_COMBINED_MEMBERSHIP_GEOMETRY_V1: MembershipGeometryV1 = MembershipGeometryV1 {
    active_planes: 1,
    u_coordinates: 16,
    table_values: 16,
};

fn commit_vector_v1(values: &[Scalar], mask: Scalar, width: usize) -> Point {
    assert!(values.len() <= width);
    let generators = TinyMembershipSuiteV1::generators()
        .reduce(width)
        .expect("valid tiny commitment width");
    let mut terms: Vec<(Scalar, Point)> = values
        .iter()
        .copied()
        .zip(generators.g_bold.iter().copied())
        .collect();
    terms.push((mask, generators.h));
    let commitment = multiexp::<TinyMembershipSuiteV1>(&terms);
    assert!(!commitment.is_identity());
    commitment
}

struct TinyOpeningSourceV1 {
    u_sum_values: Vec<Scalar>,
    u_sum_mask: Scalar,
    multiplicity_values: Vec<Scalar>,
    multiplicity_mask: Scalar,
    probe: TinySourceProbeV1,
    fail_u_sum_after_write: bool,
    fail_multiplicity_after_write: bool,
}

#[derive(Clone)]
struct TinySourceProbeV1 {
    u_sum_calls: Arc<AtomicUsize>,
    multiplicity_calls: Arc<AtomicUsize>,
    drop_calls: Arc<AtomicUsize>,
    nonzero_before_drop: Arc<AtomicUsize>,
    wiped_drops: Arc<AtomicUsize>,
}

impl TinySourceProbeV1 {
    fn new_v1() -> Self {
        Self {
            u_sum_calls: Arc::new(AtomicUsize::new(0)),
            multiplicity_calls: Arc::new(AtomicUsize::new(0)),
            drop_calls: Arc::new(AtomicUsize::new(0)),
            nonzero_before_drop: Arc::new(AtomicUsize::new(0)),
            wiped_drops: Arc::new(AtomicUsize::new(0)),
        }
    }
}

impl Drop for TinyOpeningSourceV1 {
    fn drop(&mut self) {
        if self.u_sum_values.iter().any(|value| !value.is_zero())
            || self
                .multiplicity_values
                .iter()
                .any(|value| !value.is_zero())
            || !self.u_sum_mask.is_zero()
            || !self.multiplicity_mask.is_zero()
        {
            self.probe
                .nonzero_before_drop
                .fetch_add(1, Ordering::SeqCst);
        }
        for value in self
            .u_sum_values
            .iter_mut()
            .chain(&mut self.multiplicity_values)
        {
            value.clear_secret();
        }
        self.u_sum_mask.clear_secret();
        self.multiplicity_mask.clear_secret();
        self.probe.drop_calls.fetch_add(1, Ordering::SeqCst);
        if self.u_sum_values.iter().all(|value| value.is_zero())
            && self.multiplicity_values.iter().all(|value| value.is_zero())
            && self.u_sum_mask.is_zero()
            && self.multiplicity_mask.is_zero()
        {
            self.probe.wiped_drops.fetch_add(1, Ordering::SeqCst);
        }
    }
}

impl RnsNativeGlobalMembershipOpeningSourceV1 for TinyOpeningSourceV1 {
    fn take_u_sum_opening_v1(
        &mut self,
        values: &mut [Scalar],
        commitment_mask: &mut Scalar,
    ) -> Result<(), RnsNativeGlobalMembershipDirectErrorV1> {
        if values.len() != self.u_sum_values.len()
            || self.probe.u_sum_calls.fetch_add(1, Ordering::SeqCst) != 0
        {
            return Err(RnsNativeGlobalMembershipDirectErrorV1::SourceUnavailable);
        }
        values.copy_from_slice(&self.u_sum_values);
        core::mem::swap(commitment_mask, &mut self.u_sum_mask);
        if self.fail_u_sum_after_write {
            return Err(RnsNativeGlobalMembershipDirectErrorV1::SourceUnavailable);
        }
        Ok(())
    }

    fn take_multiplicity_opening_v1(
        &mut self,
        values: &mut [Scalar],
        commitment_mask: &mut Scalar,
    ) -> Result<(), RnsNativeGlobalMembershipDirectErrorV1> {
        if values.len() != self.multiplicity_values.len()
            || self.probe.multiplicity_calls.fetch_add(1, Ordering::SeqCst) != 0
        {
            return Err(RnsNativeGlobalMembershipDirectErrorV1::SourceUnavailable);
        }
        values.copy_from_slice(&self.multiplicity_values);
        core::mem::swap(commitment_mask, &mut self.multiplicity_mask);
        if self.fail_multiplicity_after_write {
            return Err(RnsNativeGlobalMembershipDirectErrorV1::SourceUnavailable);
        }
        Ok(())
    }
}

#[derive(Clone)]
struct TinyFixtureV1 {
    context: MembershipContextV1,
    predecessor_inverse_binding_digest: [u8; DIGEST_BYTES_V1],
    active_u: Vec<Point>,
    multiplicity_commitment: Point,
    u_sum_values: Vec<Scalar>,
    u_sum_mask: Scalar,
    multiplicity_values: Vec<Scalar>,
    multiplicity_mask: Scalar,
}

impl TinyFixtureV1 {
    fn source_v1(&self) -> (TinyOpeningSourceV1, TinySourceProbeV1) {
        self.source_with_failures_v1(false, false)
    }

    fn source_with_failures_v1(
        &self,
        fail_u_sum_after_write: bool,
        fail_multiplicity_after_write: bool,
    ) -> (TinyOpeningSourceV1, TinySourceProbeV1) {
        let probe = TinySourceProbeV1::new_v1();
        let source = TinyOpeningSourceV1 {
            u_sum_values: self.u_sum_values.clone(),
            u_sum_mask: self.u_sum_mask,
            multiplicity_values: self.multiplicity_values.clone(),
            multiplicity_mask: self.multiplicity_mask,
            probe: probe.clone(),
            fail_u_sum_after_write,
            fail_multiplicity_after_write,
        };
        (source, probe)
    }
}

fn tiny_fixture_v1() -> TinyFixtureV1 {
    TINY_GEOMETRY_V1.validate_v1().expect("tiny geometry");
    let z = Scalar::from_u64(23);
    let a_planes = [[0_u64, 1, 2, 3], [2_u64, 3, 4, 5], [4_u64, 5, 6, 7]];
    let u_masks = [
        Scalar::from_u64(101),
        Scalar::from_u64(102),
        Scalar::from_u64(103),
    ];
    let mut active_u = Vec::with_capacity(TINY_GEOMETRY_V1.active_planes);
    let mut u_sum_values = vec![Scalar::zero(); TINY_GEOMETRY_V1.u_coordinates];
    for (a_plane, mask) in a_planes.into_iter().zip(u_masks) {
        let u_plane: Vec<Scalar> = a_plane
            .into_iter()
            .map(|value| {
                (z - Scalar::from_u64(value))
                    .invert()
                    .expect("z is outside the tiny table")
            })
            .collect();
        for (sum, value) in u_sum_values.iter_mut().zip(&u_plane) {
            *sum += *value;
        }
        active_u.push(commit_vector_v1(
            &u_plane,
            mask,
            TINY_GEOMETRY_V1.u_coordinates,
        ));
    }
    let u_sum_mask = u_masks
        .into_iter()
        .fold(Scalar::zero(), |sum, mask| sum + mask);
    let multiplicity_values = vec![
        Scalar::one(),
        Scalar::one(),
        Scalar::from_u64(2),
        Scalar::from_u64(2),
        Scalar::from_u64(2),
        Scalar::from_u64(2),
        Scalar::one(),
        Scalar::one(),
    ];
    let multiplicity_mask = Scalar::from_u64(401);
    let multiplicity_commitment = commit_vector_v1(
        &multiplicity_values,
        multiplicity_mask,
        TINY_GEOMETRY_V1.table_values,
    );
    TinyFixtureV1 {
        context: MembershipContextV1 {
            pre_z_binding_digest: keccak256(b"tiny-membership-pre-z-binding"),
            post_z_transcript_digest: keccak256(b"tiny-membership-post-z-transcript"),
            inverse_rho_challenge_digest: keccak256(b"tiny-membership-inverse-rho"),
            inverse_sumcheck_transcript_digest: keccak256(b"tiny-membership-inverse-sumcheck"),
            inverse_endpoint_transcript_digest: keccak256(b"tiny-membership-inverse-endpoint"),
            z,
        },
        predecessor_inverse_binding_digest: keccak256(
            b"tiny-membership-predecessor-inverse-binding",
        ),
        active_u,
        multiplicity_commitment,
        u_sum_values,
        u_sum_mask,
        multiplicity_values,
        multiplicity_mask,
    }
}

#[derive(Clone)]
struct CombinedInverseProbeV1 {
    full_opening_calls: Arc<AtomicUsize>,
    mask_opening_calls: Arc<AtomicUsize>,
    drop_calls: Arc<AtomicUsize>,
    wiped_drops: Arc<AtomicUsize>,
}

impl CombinedInverseProbeV1 {
    fn new_v1() -> Self {
        Self {
            full_opening_calls: Arc::new(AtomicUsize::new(0)),
            mask_opening_calls: Arc::new(AtomicUsize::new(0)),
            drop_calls: Arc::new(AtomicUsize::new(0)),
            wiped_drops: Arc::new(AtomicUsize::new(0)),
        }
    }
}

struct CombinedInverseSourceV1 {
    a: Vec<Scalar>,
    u: Vec<Scalar>,
    a_mask: Scalar,
    u_mask: Scalar,
    inverse_mask_values: Vec<Scalar>,
    inverse_mask_blinding: Scalar,
    probe: CombinedInverseProbeV1,
}

impl Drop for CombinedInverseSourceV1 {
    fn drop(&mut self) {
        for value in self
            .a
            .iter_mut()
            .chain(&mut self.u)
            .chain(&mut self.inverse_mask_values)
        {
            value.clear_secret();
        }
        self.a_mask.clear_secret();
        self.u_mask.clear_secret();
        self.inverse_mask_blinding.clear_secret();
        self.probe.drop_calls.fetch_add(1, Ordering::SeqCst);
        if self
            .a
            .iter()
            .chain(&self.u)
            .chain(&self.inverse_mask_values)
            .all(|value| value.is_zero())
            && self.a_mask.is_zero()
            && self.u_mask.is_zero()
            && self.inverse_mask_blinding.is_zero()
        {
            self.probe.wiped_drops.fetch_add(1, Ordering::SeqCst);
        }
    }
}

impl super::super::RnsNativeGlobalInverseProductOpeningSourceV1 for CombinedInverseSourceV1 {
    fn replay_active_plane_values_v1(
        &mut self,
        ordinal: usize,
        coordinate_prefix: &[Scalar],
        a_values: &mut [Scalar],
        u_values: &mut [Scalar],
    ) -> Result<(), super::super::RnsNativeGlobalInverseProductErrorV1> {
        if ordinal != 0 {
            return Err(super::super::RnsNativeGlobalInverseProductErrorV1::SourceUnavailable);
        }
        let expected = self.a.len() >> coordinate_prefix.len();
        if self.a.len() != self.u.len() || a_values.len() != expected || u_values.len() != expected
        {
            return Err(super::super::RnsNativeGlobalInverseProductErrorV1::SourceUnavailable);
        }
        let mut folded_a = super::super::SecretScalarsV1(self.a.clone());
        let mut folded_u = super::super::SecretScalarsV1(self.u.clone());
        if super::super::fold_prefix_v1(folded_a.as_mut_slice_v1(), coordinate_prefix)? != expected
            || super::super::fold_prefix_v1(folded_u.as_mut_slice_v1(), coordinate_prefix)?
                != expected
        {
            return Err(super::super::RnsNativeGlobalInverseProductErrorV1::SourceUnavailable);
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
    ) -> Result<(), super::super::RnsNativeGlobalInverseProductErrorV1> {
        if self.probe.full_opening_calls.fetch_add(1, Ordering::SeqCst) != 0 {
            return Err(super::super::RnsNativeGlobalInverseProductErrorV1::SourceUnavailable);
        }
        self.replay_active_plane_values_v1(ordinal, &[], a_values, u_values)?;
        core::mem::swap(a_commitment_mask, &mut self.a_mask);
        core::mem::swap(u_commitment_mask, &mut self.u_mask);
        Ok(())
    }

    fn take_inverse_product_mask_opening_v1(
        &mut self,
        values: &mut [Scalar],
        commitment_mask: &mut Scalar,
    ) -> Result<(), super::super::RnsNativeGlobalInverseProductErrorV1> {
        if values.len() != self.inverse_mask_values.len()
            || self.probe.mask_opening_calls.fetch_add(1, Ordering::SeqCst) != 0
        {
            return Err(super::super::RnsNativeGlobalInverseProductErrorV1::SourceUnavailable);
        }
        values.copy_from_slice(&self.inverse_mask_values);
        core::mem::swap(commitment_mask, &mut self.inverse_mask_blinding);
        Ok(())
    }
}

#[derive(Clone, Copy)]
enum MultiplicityBehaviorV1 {
    Succeed,
    ErrorAfterWrite,
    PanicAfterWrite,
}

#[derive(Clone)]
struct CombinedMultiplicityProbeV1 {
    calls: Arc<AtomicUsize>,
    drop_calls: Arc<AtomicUsize>,
    wiped_drops: Arc<AtomicUsize>,
}

impl CombinedMultiplicityProbeV1 {
    fn new_v1() -> Self {
        Self {
            calls: Arc::new(AtomicUsize::new(0)),
            drop_calls: Arc::new(AtomicUsize::new(0)),
            wiped_drops: Arc::new(AtomicUsize::new(0)),
        }
    }
}

struct CombinedMultiplicitySourceV1 {
    values: Vec<Scalar>,
    mask: Scalar,
    behavior: MultiplicityBehaviorV1,
    probe: CombinedMultiplicityProbeV1,
}

impl Drop for CombinedMultiplicitySourceV1 {
    fn drop(&mut self) {
        for value in &mut self.values {
            value.clear_secret();
        }
        self.mask.clear_secret();
        self.probe.drop_calls.fetch_add(1, Ordering::SeqCst);
        if self.values.iter().all(|value| value.is_zero()) && self.mask.is_zero() {
            self.probe.wiped_drops.fetch_add(1, Ordering::SeqCst);
        }
    }
}

impl RnsNativeGlobalMultiplicityOpeningSourceV1 for CombinedMultiplicitySourceV1 {
    fn take_multiplicity_opening_v1(
        &mut self,
        values: &mut [Scalar],
        commitment_mask: &mut Scalar,
    ) -> Result<(), RnsNativeGlobalMembershipDirectErrorV1> {
        if values.len() != self.values.len() || self.probe.calls.fetch_add(1, Ordering::SeqCst) != 0
        {
            return Err(RnsNativeGlobalMembershipDirectErrorV1::SourceUnavailable);
        }
        for (destination, source) in values.iter_mut().zip(&mut self.values) {
            core::mem::swap(destination, source);
        }
        core::mem::swap(commitment_mask, &mut self.mask);
        match self.behavior {
            MultiplicityBehaviorV1::Succeed => Ok(()),
            MultiplicityBehaviorV1::ErrorAfterWrite => {
                Err(RnsNativeGlobalMembershipDirectErrorV1::SourceUnavailable)
            }
            MultiplicityBehaviorV1::PanicAfterWrite => panic!("test multiplicity source panic"),
        }
    }
}

struct TinyCombinedFixtureV1 {
    inverse_context: super::super::KernelContextV1,
    predecessor_binding_digest: [u8; DIGEST_BYTES_V1],
    a_values: Vec<Scalar>,
    u_values: Vec<Scalar>,
    a_mask: Scalar,
    u_mask: Scalar,
    inverse_mask_values: Vec<Scalar>,
    inverse_mask_blinding: Scalar,
    a_commitments: Vec<Point>,
    u_commitments: Vec<Point>,
    inverse_mask_commitment: Point,
    multiplicity_values: Vec<Scalar>,
    multiplicity_mask: Scalar,
    multiplicity_commitment: Point,
}

impl TinyCombinedFixtureV1 {
    fn inverse_source_v1(&self) -> (CombinedInverseSourceV1, CombinedInverseProbeV1) {
        let probe = CombinedInverseProbeV1::new_v1();
        (
            CombinedInverseSourceV1 {
                a: self.a_values.clone(),
                u: self.u_values.clone(),
                a_mask: self.a_mask,
                u_mask: self.u_mask,
                inverse_mask_values: self.inverse_mask_values.clone(),
                inverse_mask_blinding: self.inverse_mask_blinding,
                probe: probe.clone(),
            },
            probe,
        )
    }

    fn multiplicity_source_v1(
        &self,
        behavior: MultiplicityBehaviorV1,
    ) -> (CombinedMultiplicitySourceV1, CombinedMultiplicityProbeV1) {
        let probe = CombinedMultiplicityProbeV1::new_v1();
        (
            CombinedMultiplicitySourceV1 {
                values: self.multiplicity_values.clone(),
                mask: self.multiplicity_mask,
                behavior,
                probe: probe.clone(),
            },
            probe,
        )
    }
}

fn combined_commit_vector_v1(values: &[Scalar], mask: Scalar) -> Point {
    let generators = TinyCombinedSuiteV1::generators()
        .reduce(TINY_COMBINED_MEMBERSHIP_GEOMETRY_V1.table_values)
        .expect("tiny combined reduced basis");
    let mut terms: Vec<(Scalar, Point)> = values
        .iter()
        .copied()
        .zip(generators.g_bold.iter().copied())
        .collect();
    terms.push((mask, generators.h));
    let commitment = multiexp::<TinyCombinedSuiteV1>(&terms);
    assert!(!commitment.is_identity());
    commitment
}

fn tiny_combined_fixture_v1() -> TinyCombinedFixtureV1 {
    TINY_COMBINED_INVERSE_GEOMETRY_V1
        .validate_v1()
        .expect("tiny combined inverse geometry");
    TINY_COMBINED_MEMBERSHIP_GEOMETRY_V1
        .validate_v1()
        .expect("tiny combined membership geometry");
    let z = Scalar::from_u64(211);
    let a_values: Vec<Scalar> = (0..TINY_COMBINED_INVERSE_GEOMETRY_V1.coordinates)
        .map(|value| Scalar::from_u64(value as u64))
        .collect();
    let u_values: Vec<Scalar> = a_values
        .iter()
        .copied()
        .map(|value| (z - value).invert().expect("z is outside the table"))
        .collect();
    let a_mask = Scalar::from_u64(5_001);
    let u_mask = Scalar::from_u64(5_002);
    let inverse_mask_values: Vec<Scalar> =
        (0..TINY_COMBINED_INVERSE_GEOMETRY_V1.mask_scalars_v1().unwrap())
            .map(|index| Scalar::from_u64(6_001 + index as u64))
            .collect();
    let inverse_mask_blinding = Scalar::from_u64(6_101);
    let multiplicity_values =
        vec![Scalar::one(); TINY_COMBINED_MEMBERSHIP_GEOMETRY_V1.table_values];
    let multiplicity_mask = Scalar::from_u64(7_001);
    TinyCombinedFixtureV1 {
        inverse_context: super::super::KernelContextV1 {
            pre_z_binding_digest: keccak256(b"tiny-combined-pre-z-binding"),
            post_z_transcript_digest: keccak256(b"tiny-combined-post-z-transcript"),
            z,
        },
        predecessor_binding_digest: keccak256(b"tiny-combined-predecessor-binding"),
        a_commitments: vec![combined_commit_vector_v1(&a_values, a_mask)],
        u_commitments: vec![combined_commit_vector_v1(&u_values, u_mask)],
        inverse_mask_commitment: combined_commit_vector_v1(
            &inverse_mask_values,
            inverse_mask_blinding,
        ),
        multiplicity_commitment: combined_commit_vector_v1(&multiplicity_values, multiplicity_mask),
        a_values,
        u_values,
        a_mask,
        u_mask,
        inverse_mask_values,
        inverse_mask_blinding,
        multiplicity_values,
        multiplicity_mask,
    }
}

fn prove_tiny_with_source_v1(
    fixture: &TinyFixtureV1,
    source: TinyOpeningSourceV1,
    random_label: &[u8],
) -> Result<Vec<u8>, RnsNativeGlobalMembershipDirectErrorV1> {
    prove_kernel_for_suite_v1::<TinyMembershipSuiteV1, _, _>(
        TINY_GEOMETRY_V1,
        fixture.context,
        MembershipCommitmentsV1 {
            active_u: &fixture.active_u,
            multiplicity: fixture.multiplicity_commitment,
        },
        source,
        b"next-stage",
        &mut KatRandomV1::new(random_label),
    )
}

struct CachedTinyProofV1 {
    wire: Vec<u8>,
    source_probe: TinySourceProbeV1,
}

fn cached_tiny_proof_v1() -> &'static CachedTinyProofV1 {
    static PROOF: OnceLock<CachedTinyProofV1> = OnceLock::new();
    PROOF.get_or_init(|| {
        let fixture = tiny_fixture_v1();
        let (source, source_probe) = fixture.source_v1();
        let wire =
            prove_tiny_with_source_v1(&fixture, source, b"tiny-global-membership-direct-proof")
                .expect("valid cached tiny membership proof");
        CachedTinyProofV1 { wire, source_probe }
    })
}

struct CachedTinyCombinedProofV1 {
    wire: Vec<u8>,
    inverse_probe: CombinedInverseProbeV1,
    multiplicity_probe: CombinedMultiplicityProbeV1,
}

fn cached_tiny_combined_proof_v1() -> &'static CachedTinyCombinedProofV1 {
    static PROOF: OnceLock<CachedTinyCombinedProofV1> = OnceLock::new();
    PROOF.get_or_init(|| {
        let fixture = tiny_combined_fixture_v1();
        let (inverse_source, inverse_probe) = fixture.inverse_source_v1();
        let (multiplicity_source, multiplicity_probe) =
            fixture.multiplicity_source_v1(MultiplicityBehaviorV1::Succeed);
        let wire =
            prove_combined_for_suites_v1::<TinyCombinedSuiteV1, TinyCombinedSuiteV1, _, _, _>(
                TINY_COMBINED_INVERSE_GEOMETRY_V1,
                TINY_COMBINED_MEMBERSHIP_GEOMETRY_V1,
                fixture.inverse_context,
                super::super::KernelCommitmentsV1 {
                    a: &fixture.a_commitments,
                    u: &fixture.u_commitments,
                    inverse_product_mask: fixture.inverse_mask_commitment,
                },
                inverse_source,
                fixture.multiplicity_commitment,
                multiplicity_source,
                b"combined-next-stage",
                &mut KatRandomV1::new(b"tiny-combined-inverse-membership-proof"),
            )
            .expect("valid tiny combined proof");
        CachedTinyCombinedProofV1 {
            wire,
            inverse_probe,
            multiplicity_probe,
        }
    })
}

fn verify_tiny_combined_inverse_v1<'a>(
    fixture: &TinyCombinedFixtureV1,
    wire: &'a [u8],
) -> Result<super::super::VerifiedKernelV1<'a>, super::super::RnsNativeGlobalInverseProductErrorV1>
{
    super::super::verify_kernel_for_suite_v1::<TinyCombinedSuiteV1>(
        TINY_COMBINED_INVERSE_GEOMETRY_V1,
        fixture.inverse_context,
        fixture.predecessor_binding_digest,
        super::super::KernelCommitmentsV1 {
            a: &fixture.a_commitments,
            u: &fixture.u_commitments,
            inverse_product_mask: fixture.inverse_mask_commitment,
        },
        wire,
        64 * 1024,
    )
}

fn verify_tiny_combined_membership_v1<'a>(
    fixture: &TinyCombinedFixtureV1,
    inverse: &super::super::VerifiedKernelV1<'a>,
) -> Result<VerifiedKernelV1<'a>, RnsNativeGlobalMembershipDirectErrorV1> {
    verify_kernel_with_u_sum_for_suite_v1::<TinyCombinedSuiteV1>(
        TINY_COMBINED_MEMBERSHIP_GEOMETRY_V1,
        MembershipContextV1 {
            pre_z_binding_digest: fixture.inverse_context.pre_z_binding_digest,
            post_z_transcript_digest: fixture.inverse_context.post_z_transcript_digest,
            inverse_rho_challenge_digest: super::super::rho_challenge_digest_v1(inverse.rho),
            inverse_sumcheck_transcript_digest: inverse.sumcheck_transcript_digest,
            inverse_endpoint_transcript_digest: inverse.endpoint_transcript_digest,
            z: fixture.inverse_context.z,
        },
        inverse.binding_digest,
        inverse.u_sum_commitment,
        fixture.multiplicity_commitment,
        inverse.residual,
        64 * 1024,
    )
}

fn verify_tiny_v1<'a>(
    fixture: &TinyFixtureV1,
    wire: &'a [u8],
) -> Result<VerifiedKernelV1<'a>, RnsNativeGlobalMembershipDirectErrorV1> {
    verify_kernel_for_suite_v1::<TinyMembershipSuiteV1>(
        TINY_GEOMETRY_V1,
        fixture.context,
        fixture.predecessor_inverse_binding_digest,
        MembershipCommitmentsV1 {
            active_u: &fixture.active_u,
            multiplicity: fixture.multiplicity_commitment,
        },
        wire,
        64 * 1024,
    )
}

fn refresh_codec_v1(wire: &mut [u8]) {
    let codec_offset = wire
        .len()
        .checked_sub(CODEC_DIGEST_BYTES_V1)
        .expect("membership wire contains a codec digest");
    let codec = codec_digest_v1(&wire[..codec_offset]);
    wire[codec_offset..].copy_from_slice(&codec);
}

#[test]
fn pending_inverse_builds_membership_first_then_seals_a_verifiable_envelope() {
    let fixture = tiny_combined_fixture_v1();
    let cached = cached_tiny_combined_proof_v1();
    assert_eq!(
        cached
            .inverse_probe
            .full_opening_calls
            .load(Ordering::SeqCst),
        1
    );
    assert_eq!(
        cached
            .inverse_probe
            .mask_opening_calls
            .load(Ordering::SeqCst),
        1
    );
    assert_eq!(cached.inverse_probe.drop_calls.load(Ordering::SeqCst), 1);
    assert_eq!(cached.inverse_probe.wiped_drops.load(Ordering::SeqCst), 1);
    assert_eq!(cached.multiplicity_probe.calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        cached.multiplicity_probe.drop_calls.load(Ordering::SeqCst),
        1
    );
    assert_eq!(
        cached.multiplicity_probe.wiped_drops.load(Ordering::SeqCst),
        1
    );

    let inverse = verify_tiny_combined_inverse_v1(&fixture, &cached.wire)
        .expect("combined inverse core verifies");
    assert_eq!(inverse.u_sum_commitment, fixture.u_commitments[0]);
    let membership = verify_tiny_combined_membership_v1(&fixture, &inverse)
        .expect("nested direct membership verifies");
    assert_eq!(membership.residual, b"combined-next-stage");
    assert_eq!(membership.u_sum_commitment, inverse.u_sum_commitment);
}

#[test]
fn combined_handoff_commitments_and_nested_core_mutations_fail_closed() {
    let fixture = tiny_combined_fixture_v1();
    let wire = &cached_tiny_combined_proof_v1().wire;
    let inverse = verify_tiny_combined_inverse_v1(&fixture, wire).expect("valid inverse");
    let context = MembershipContextV1 {
        pre_z_binding_digest: fixture.inverse_context.pre_z_binding_digest,
        post_z_transcript_digest: fixture.inverse_context.post_z_transcript_digest,
        inverse_rho_challenge_digest: super::super::rho_challenge_digest_v1(inverse.rho),
        inverse_sumcheck_transcript_digest: inverse.sumcheck_transcript_digest,
        inverse_endpoint_transcript_digest: inverse.endpoint_transcript_digest,
        z: fixture.inverse_context.z,
    };
    assert!(
        verify_kernel_with_u_sum_for_suite_v1::<TinyCombinedSuiteV1>(
            TINY_COMBINED_MEMBERSHIP_GEOMETRY_V1,
            context,
            inverse.binding_digest,
            inverse.u_sum_commitment + TinyCombinedSuiteV1::generators().g_bold[0],
            fixture.multiplicity_commitment,
            inverse.residual,
            64 * 1024,
        )
        .is_err()
    );
    assert!(
        verify_kernel_with_u_sum_for_suite_v1::<TinyCombinedSuiteV1>(
            TINY_COMBINED_MEMBERSHIP_GEOMETRY_V1,
            context,
            inverse.binding_digest,
            inverse.u_sum_commitment,
            fixture.multiplicity_commitment + TinyCombinedSuiteV1::generators().g_bold[0],
            inverse.residual,
            64 * 1024,
        )
        .is_err()
    );

    let inverse_view =
        super::super::ProofViewV1::decode_v1(wire, TINY_COMBINED_INVERSE_GEOMETRY_V1, 64 * 1024)
            .expect("combined inverse view");
    let child_offset = inverse_view.residual.as_ptr() as usize - wire.as_ptr() as usize;
    let child_len = inverse_view.residual.len();
    let mut changed = wire.to_vec();
    changed[child_offset + HEADER_BYTES_V1 + 7] ^= 1;
    let child_codec_offset = child_offset + child_len - CODEC_DIGEST_BYTES_V1;
    let child_codec = codec_digest_v1(&changed[child_offset..child_codec_offset]);
    changed[child_codec_offset..child_offset + child_len].copy_from_slice(&child_codec);
    let inverse_codec_offset = changed.len() - CODEC_DIGEST_BYTES_V1;
    let inverse_codec = super::super::codec_digest_v1(&changed[..inverse_codec_offset]);
    changed[inverse_codec_offset..].copy_from_slice(&inverse_codec);
    let changed_inverse = verify_tiny_combined_inverse_v1(&fixture, &changed)
        .expect("inverse challenges exclude the nested frame");
    assert!(verify_tiny_combined_membership_v1(&fixture, &changed_inverse).is_err());
}

#[test]
fn combined_geometry_mismatch_rejects_before_any_one_shot_opening() {
    let fixture = tiny_combined_fixture_v1();
    let (inverse_source, inverse_probe) = fixture.inverse_source_v1();
    let (multiplicity_source, multiplicity_probe) =
        fixture.multiplicity_source_v1(MultiplicityBehaviorV1::Succeed);
    let mismatched_membership = MembershipGeometryV1 {
        active_planes: 2,
        ..TINY_COMBINED_MEMBERSHIP_GEOMETRY_V1
    };
    assert!(matches!(
        prove_combined_for_suites_v1::<TinyCombinedSuiteV1, TinyCombinedSuiteV1, _, _, _>(
            TINY_COMBINED_INVERSE_GEOMETRY_V1,
            mismatched_membership,
            fixture.inverse_context,
            super::super::KernelCommitmentsV1 {
                a: &fixture.a_commitments,
                u: &fixture.u_commitments,
                inverse_product_mask: fixture.inverse_mask_commitment,
            },
            inverse_source,
            fixture.multiplicity_commitment,
            multiplicity_source,
            b"next",
            &mut KatRandomV1::new(b"unreachable-geometry-mismatch"),
        ),
        Err(RnsNativeGlobalCombinedProverErrorV1::Membership(
            RnsNativeGlobalMembershipDirectErrorV1::InvalidGeometry
        ))
    ));
    assert_eq!(inverse_probe.full_opening_calls.load(Ordering::SeqCst), 0);
    assert_eq!(inverse_probe.mask_opening_calls.load(Ordering::SeqCst), 0);
    assert_eq!(inverse_probe.drop_calls.load(Ordering::SeqCst), 1);
    assert_eq!(inverse_probe.wiped_drops.load(Ordering::SeqCst), 1);
    assert_eq!(multiplicity_probe.calls.load(Ordering::SeqCst), 0);
    assert_eq!(multiplicity_probe.drop_calls.load(Ordering::SeqCst), 1);
    assert_eq!(multiplicity_probe.wiped_drops.load(Ordering::SeqCst), 1);
}

#[test]
fn pending_u_sum_and_separate_m_source_zeroize_on_error_and_unwind() {
    for behavior in [
        MultiplicityBehaviorV1::ErrorAfterWrite,
        MultiplicityBehaviorV1::PanicAfterWrite,
    ] {
        reset_secret_cleanup_audit_v1();
        let fixture = tiny_combined_fixture_v1();
        let (multiplicity, probe) = fixture.multiplicity_source_v1(behavior);
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut source = PendingMembershipOpeningSourceV1 {
                u_sum: Some(super::super::ActiveUSumOpeningV1::from_test_values_v1(
                    fixture.u_values.clone(),
                    fixture.u_mask,
                )),
                multiplicity,
            };
            let mut u_values = SecretScalarsV1::try_zeroed_v1(fixture.u_values.len()).unwrap();
            let mut m_values =
                SecretScalarsV1::try_zeroed_v1(fixture.multiplicity_values.len()).unwrap();
            let mut u_mask = SecretScalarV1::zero_v1();
            let mut m_mask = SecretScalarV1::zero_v1();
            source
                .take_u_sum_opening_v1(u_values.as_mut_slice_v1(), u_mask.as_mut_v1())
                .expect("pending U sum is available once");
            source.take_multiplicity_opening_v1(m_values.as_mut_slice_v1(), m_mask.as_mut_v1())
        }));
        match behavior {
            MultiplicityBehaviorV1::ErrorAfterWrite => assert!(matches!(
                outcome,
                Ok(Err(
                    RnsNativeGlobalMembershipDirectErrorV1::SourceUnavailable
                ))
            )),
            MultiplicityBehaviorV1::PanicAfterWrite => assert!(outcome.is_err()),
            MultiplicityBehaviorV1::Succeed => unreachable!(),
        }
        assert_eq!(probe.calls.load(Ordering::SeqCst), 1);
        assert_eq!(probe.drop_calls.load(Ordering::SeqCst), 1);
        assert_eq!(probe.wiped_drops.load(Ordering::SeqCst), 1);
        let cleanup = secret_cleanup_audit_v1();
        assert_eq!(cleanup.vector_drops, 2);
        assert_eq!(cleanup.vector_nonzero_before_clear, 2);
        assert_eq!(cleanup.vector_zero_after_clear, 2);
        assert_eq!(cleanup.scalar_drops, 2);
        assert_eq!(cleanup.scalar_nonzero_before_clear, 2);
        assert_eq!(cleanup.scalar_zero_after_clear, 2);
    }
}

#[test]
fn tiny_direct_membership_roundtrips_and_consumes_each_opening_once() {
    let fixture = tiny_fixture_v1();
    let q_z = table_inverse_weights_v1(TINY_GEOMETRY_V1, fixture.context.z)
        .expect("tiny inverse weights");
    let u_total = fixture
        .u_sum_values
        .iter()
        .copied()
        .fold(Scalar::zero(), |sum, value| sum + value);
    let multiplicity_weighted_total = fixture
        .multiplicity_values
        .iter()
        .copied()
        .zip(q_z)
        .fold(Scalar::zero(), |sum, (value, weight)| sum + value * weight);
    assert_eq!(u_total, multiplicity_weighted_total);
    assert_eq!(
        fixture
            .multiplicity_values
            .iter()
            .copied()
            .fold(Scalar::zero(), |sum, value| sum + value),
        Scalar::from_u64(TINY_GEOMETRY_V1.active_lookup_values_v1().unwrap())
    );

    let derived_u_sum = derive_u_sum_commitment_v1(TINY_GEOMETRY_V1, &fixture.active_u).unwrap();
    let zero_padded_opening = commit_vector_v1(
        &fixture.u_sum_values,
        fixture.u_sum_mask,
        TINY_GEOMETRY_V1.table_values,
    );
    assert_eq!(derived_u_sum, zero_padded_opening);

    let cached = cached_tiny_proof_v1();
    assert_eq!(cached.source_probe.u_sum_calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        cached
            .source_probe
            .multiplicity_calls
            .load(Ordering::SeqCst),
        1
    );
    assert_eq!(cached.source_probe.drop_calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        cached
            .source_probe
            .nonzero_before_drop
            .load(Ordering::SeqCst),
        1
    );
    assert_eq!(cached.source_probe.wiped_drops.load(Ordering::SeqCst), 1);
    let verified = verify_tiny_v1(&fixture, &cached.wire).expect("valid tiny membership proof");
    assert_eq!(verified.residual, b"next-stage");
    assert_eq!(verified.u_sum_commitment, derived_u_sum);
    let verified_from_inverse_handoff =
        verify_kernel_with_u_sum_for_suite_v1::<TinyMembershipSuiteV1>(
            TINY_GEOMETRY_V1,
            fixture.context,
            fixture.predecessor_inverse_binding_digest,
            derived_u_sum,
            fixture.multiplicity_commitment,
            &cached.wire,
            64 * 1024,
        )
        .expect("derived inverse U-sum handoff verifies the same core");
    assert_eq!(
        verified_from_inverse_handoff.transcript_digest,
        verified.transcript_digest
    );
    assert_eq!(
        verified_from_inverse_handoff.binding_digest,
        verified.binding_digest
    );
    assert!(
        verify_kernel_with_u_sum_for_suite_v1::<TinyMembershipSuiteV1>(
            TINY_GEOMETRY_V1,
            fixture.context,
            fixture.predecessor_inverse_binding_digest,
            derived_u_sum + TinyMembershipSuiteV1::generators().g_bold[0],
            fixture.multiplicity_commitment,
            &cached.wire,
            64 * 1024,
        )
        .is_err()
    );
    for digest in [
        verified.transcript_digest,
        verified.residual_digest,
        verified.binding_digest,
    ] {
        assert_ne!(digest, [0; DIGEST_BYTES_V1]);
    }
    let view = ProofViewV1::decode_v1(&cached.wire, TINY_GEOMETRY_V1, 64 * 1024)
        .expect("canonical tiny view");
    assert_eq!(view.core.len(), 787);
    assert_eq!(core_bytes_v1(TINY_GEOMETRY_V1).unwrap(), 787);
    assert_eq!(TINY_GEOMETRY_V1.challenge_count_v1().unwrap(), 7);
}

#[test]
fn active_u_aggregation_excludes_virtual_points_and_requires_exact_cardinality() {
    let fixture = tiny_fixture_v1();
    assert!(matches!(
        derive_u_sum_commitment_v1(TINY_GEOMETRY_V1, &fixture.active_u[..2]),
        Err(RnsNativeGlobalMembershipDirectErrorV1::InvalidGeometry)
    ));
    let virtual_u =
        vec![fixture.context.z.invert().expect("nonzero z"); TINY_GEOMETRY_V1.u_coordinates];
    let virtual_commitment = commit_vector_v1(
        &virtual_u,
        Scalar::from_u64(997),
        TINY_GEOMETRY_V1.u_coordinates,
    );
    let mut active_plus_virtual = fixture.active_u.clone();
    active_plus_virtual.push(virtual_commitment);
    assert!(matches!(
        derive_u_sum_commitment_v1(TINY_GEOMETRY_V1, &active_plus_virtual),
        Err(RnsNativeGlobalMembershipDirectErrorV1::InvalidGeometry)
    ));

    let wire = &cached_tiny_proof_v1().wire;
    let mut replaced = fixture.clone();
    replaced.active_u[0] = virtual_commitment;
    assert!(verify_tiny_v1(&replaced, wire).is_err());
}

#[test]
fn matching_commitments_cannot_hide_wrong_membership_or_wrong_total() {
    let fixture = tiny_fixture_v1();

    let mut wrong_log_derivative = fixture.clone();
    wrong_log_derivative.u_sum_values[0] += Scalar::one();
    wrong_log_derivative.active_u[0] += TinyMembershipSuiteV1::generators().g_bold[0];
    let (source, _) = wrong_log_derivative.source_v1();
    assert!(
        prove_tiny_with_source_v1(&wrong_log_derivative, source, b"wrong-log-derivative",).is_err()
    );

    let mut wrong_total = fixture.clone();
    wrong_total.multiplicity_values[0] += Scalar::one();
    wrong_total.multiplicity_commitment = commit_vector_v1(
        &wrong_total.multiplicity_values,
        wrong_total.multiplicity_mask,
        TINY_GEOMETRY_V1.table_values,
    );
    let (source, _) = wrong_total.source_v1();
    assert!(prove_tiny_with_source_v1(&wrong_total, source, b"wrong-total").is_err());

    let mut wrong_distribution = fixture.clone();
    wrong_distribution.multiplicity_values[0] += Scalar::one();
    wrong_distribution.multiplicity_values[1] -= Scalar::one();
    wrong_distribution.multiplicity_commitment = commit_vector_v1(
        &wrong_distribution.multiplicity_values,
        wrong_distribution.multiplicity_mask,
        TINY_GEOMETRY_V1.table_values,
    );
    let (source, _) = wrong_distribution.source_v1();
    assert!(
        prove_tiny_with_source_v1(&wrong_distribution, source, b"wrong-distribution",).is_err()
    );

    let mut wrong_opening = fixture.clone();
    let (mut source, _) = wrong_opening.source_v1();
    source.u_sum_mask += Scalar::one();
    assert!(prove_tiny_with_source_v1(&wrong_opening, source, b"wrong-opening").is_err());
    wrong_opening.multiplicity_commitment = wrong_opening.active_u[0];
    let (source, _) = wrong_opening.source_v1();
    assert!(prove_tiny_with_source_v1(&wrong_opening, source, b"spliced-multiplicity",).is_err());
}

#[test]
fn inverse_binding_enters_only_after_the_membership_transcript_verifies() {
    let fixture = tiny_fixture_v1();
    let wire = &cached_tiny_proof_v1().wire;
    let first = verify_tiny_v1(&fixture, wire).expect("first predecessor binding");
    let second_predecessor = keccak256(b"different-inverse-binding");
    let second = verify_kernel_for_suite_v1::<TinyMembershipSuiteV1>(
        TINY_GEOMETRY_V1,
        fixture.context,
        second_predecessor,
        MembershipCommitmentsV1 {
            active_u: &fixture.active_u,
            multiplicity: fixture.multiplicity_commitment,
        },
        wire,
        64 * 1024,
    )
    .expect("predecessor binding is excluded from Fiat-Shamir");
    assert_eq!(first.transcript_digest, second.transcript_digest);
    assert_eq!(first.u_sum_commitment, second.u_sum_commitment);
    assert_ne!(first.residual_digest, second.residual_digest);
    assert_ne!(first.binding_digest, second.binding_digest);
    assert!(matches!(
        verify_kernel_for_suite_v1::<TinyMembershipSuiteV1>(
            TINY_GEOMETRY_V1,
            fixture.context,
            [0; DIGEST_BYTES_V1],
            MembershipCommitmentsV1 {
                active_u: &fixture.active_u,
                multiplicity: fixture.multiplicity_commitment,
            },
            wire,
            64 * 1024,
        ),
        Err(RnsNativeGlobalMembershipDirectErrorV1::InvalidContext)
    ));
}

#[test]
fn every_noncircular_inverse_digest_and_z_are_fiat_shamir_bound() {
    let fixture = tiny_fixture_v1();
    let wire = &cached_tiny_proof_v1().wire;
    let changed = keccak256(b"changed-safe-inverse-digest");
    let contexts = [
        MembershipContextV1 {
            pre_z_binding_digest: changed,
            ..fixture.context
        },
        MembershipContextV1 {
            post_z_transcript_digest: changed,
            ..fixture.context
        },
        MembershipContextV1 {
            inverse_rho_challenge_digest: changed,
            ..fixture.context
        },
        MembershipContextV1 {
            inverse_sumcheck_transcript_digest: changed,
            ..fixture.context
        },
        MembershipContextV1 {
            inverse_endpoint_transcript_digest: changed,
            ..fixture.context
        },
        MembershipContextV1 {
            z: fixture.context.z + Scalar::one(),
            ..fixture.context
        },
    ];
    for context in contexts {
        assert!(
            verify_kernel_for_suite_v1::<TinyMembershipSuiteV1>(
                TINY_GEOMETRY_V1,
                context,
                fixture.predecessor_inverse_binding_digest,
                MembershipCommitmentsV1 {
                    active_u: &fixture.active_u,
                    multiplicity: fixture.multiplicity_commitment,
                },
                wire,
                64 * 1024,
            )
            .is_err()
        );
    }
}

#[test]
fn z_inside_the_table_is_rejected_before_any_opening_is_taken() {
    let mut fixture = tiny_fixture_v1();
    fixture.context.z = Scalar::from_u64(3);
    let (source, probe) = fixture.source_v1();
    assert!(matches!(
        prove_tiny_with_source_v1(&fixture, source, b"z-inside-table"),
        Err(RnsNativeGlobalMembershipDirectErrorV1::InvalidContext)
    ));
    assert_eq!(probe.u_sum_calls.load(Ordering::SeqCst), 0);
    assert_eq!(probe.multiplicity_calls.load(Ordering::SeqCst), 0);
    assert_eq!(probe.drop_calls.load(Ordering::SeqCst), 1);
    assert_eq!(probe.nonzero_before_drop.load(Ordering::SeqCst), 1);
    assert_eq!(probe.wiped_drops.load(Ordering::SeqCst), 1);
    assert!(matches!(
        verify_tiny_v1(&fixture, &cached_tiny_proof_v1().wire),
        Err(RnsNativeGlobalMembershipDirectErrorV1::InvalidContext)
    ));
    assert!(matches!(
        table_inverse_weights_v1(TINY_GEOMETRY_V1, fixture.context.z),
        Err(RnsNativeGlobalMembershipDirectErrorV1::InvalidContext)
    ));
}

#[test]
fn verifier_authenticates_the_pre_z_multiplicity_commitment() {
    let mut fixture = tiny_fixture_v1();
    fixture.multiplicity_commitment += TinyMembershipSuiteV1::generators().g_bold[0];
    assert!(verify_tiny_v1(&fixture, &cached_tiny_proof_v1().wire).is_err());
}

#[test]
fn current_residual_is_excluded_from_core_challenges_but_bound_after_verification() {
    let fixture = tiny_fixture_v1();
    let original_wire = &cached_tiny_proof_v1().wire;
    let original = verify_tiny_v1(&fixture, original_wire).expect("original residual");
    let mut changed_wire = original_wire.to_vec();
    let residual_offset = HEADER_BYTES_V1 + core_bytes_v1(TINY_GEOMETRY_V1).unwrap();
    changed_wire[residual_offset] ^= 1;
    refresh_codec_v1(&mut changed_wire);
    let changed = verify_tiny_v1(&fixture, &changed_wire).expect("changed residual");
    assert_eq!(original.transcript_digest, changed.transcript_digest);
    assert_eq!(original.u_sum_commitment, changed.u_sum_commitment);
    assert_ne!(original.residual, changed.residual);
    assert_ne!(original.residual_digest, changed.residual_digest);
    assert_ne!(original.binding_digest, changed.binding_digest);
}

#[test]
fn every_serialized_point_and_scalar_requires_a_canonical_encoding() {
    let fixture = tiny_fixture_v1();
    let wire = &cached_tiny_proof_v1().wire;
    let log_n = TINY_GEOMETRY_V1.log_n_v1().unwrap();
    let fixed_points = 13;
    let first_scalar = fixed_points * POINT_BYTES_V1;
    let first_ipa_point = first_scalar + 3 * SCALAR_BYTES_V1;
    let first_final_scalar = first_ipa_point + 2 * log_n * POINT_BYTES_V1;
    let point_offsets = (0..fixed_points)
        .map(|index| index * POINT_BYTES_V1)
        .chain((0..2 * log_n).map(|index| first_ipa_point + index * POINT_BYTES_V1))
        .collect::<Vec<_>>();
    let scalar_offsets = (0..3)
        .map(|index| first_scalar + index * SCALAR_BYTES_V1)
        .chain((0..2).map(|index| first_final_scalar + index * SCALAR_BYTES_V1))
        .collect::<Vec<_>>();
    assert_eq!(point_offsets.len(), 19);
    assert_eq!(scalar_offsets.len(), PROOF_SCALARS_V1);
    assert_eq!(
        first_final_scalar + 2 * SCALAR_BYTES_V1,
        core_bytes_v1(TINY_GEOMETRY_V1).unwrap()
    );

    for core_offset in point_offsets {
        let mut mutated = wire.to_vec();
        let start = HEADER_BYTES_V1 + core_offset;
        mutated[start..start + POINT_BYTES_V1].fill(0);
        refresh_codec_v1(&mut mutated);
        assert!(matches!(
            verify_tiny_v1(&fixture, &mutated),
            Err(RnsNativeGlobalMembershipDirectErrorV1::InvalidPoint)
        ));
    }
    for core_offset in scalar_offsets {
        let mut mutated = wire.to_vec();
        let start = HEADER_BYTES_V1 + core_offset;
        mutated[start..start + SCALAR_BYTES_V1].fill(0xff);
        refresh_codec_v1(&mut mutated);
        assert!(matches!(
            verify_tiny_v1(&fixture, &mutated),
            Err(RnsNativeGlobalMembershipDirectErrorV1::InvalidScalar)
        ));
    }
}

#[test]
fn each_one_shot_source_failure_wipes_source_and_caller_destinations() {
    let fixture = tiny_fixture_v1();
    for (fail_u_sum, fail_multiplicity, expected_m_calls, expected_nonzero) in
        [(true, false, 0, 1), (false, true, 1, 2)]
    {
        reset_secret_cleanup_audit_v1();
        let (source, probe) = fixture.source_with_failures_v1(fail_u_sum, fail_multiplicity);
        assert!(matches!(
            prove_tiny_with_source_v1(&fixture, source, b"source-failure"),
            Err(RnsNativeGlobalMembershipDirectErrorV1::SourceUnavailable)
        ));
        assert_eq!(probe.u_sum_calls.load(Ordering::SeqCst), 1);
        assert_eq!(
            probe.multiplicity_calls.load(Ordering::SeqCst),
            expected_m_calls
        );
        assert_eq!(probe.drop_calls.load(Ordering::SeqCst), 1);
        assert_eq!(probe.nonzero_before_drop.load(Ordering::SeqCst), 1);
        assert_eq!(probe.wiped_drops.load(Ordering::SeqCst), 1);
        let cleanup = secret_cleanup_audit_v1();
        assert_eq!(cleanup.vector_drops, 2);
        assert_eq!(cleanup.vector_nonzero_before_clear, expected_nonzero);
        assert_eq!(cleanup.vector_zero_after_clear, 2);
        assert_eq!(cleanup.scalar_drops, 2);
        assert_eq!(cleanup.scalar_nonzero_before_clear, expected_nonzero);
        assert_eq!(cleanup.scalar_zero_after_clear, 2);
    }
}

#[test]
fn deterministic_transcript_kat_reproduces_the_cached_wire_exactly() {
    let fixture = tiny_fixture_v1();
    let (source, probe) = fixture.source_v1();
    let repeated =
        prove_tiny_with_source_v1(&fixture, source, b"tiny-global-membership-direct-proof")
            .expect("second and final valid tiny proof");
    assert_eq!(repeated.as_slice(), cached_tiny_proof_v1().wire.as_slice());
    assert_eq!(probe.u_sum_calls.load(Ordering::SeqCst), 1);
    assert_eq!(probe.multiplicity_calls.load(Ordering::SeqCst), 1);
    assert_eq!(probe.drop_calls.load(Ordering::SeqCst), 1);
    assert_eq!(probe.nonzero_before_drop.load(Ordering::SeqCst), 1);
    assert_eq!(probe.wiped_drops.load(Ordering::SeqCst), 1);
    let first = verify_tiny_v1(&fixture, &cached_tiny_proof_v1().wire).unwrap();
    let second = verify_tiny_v1(&fixture, &repeated).unwrap();
    assert_eq!(first.transcript_digest, second.transcript_digest);
    assert_eq!(first.residual_digest, second.residual_digest);
    assert_eq!(first.binding_digest, second.binding_digest);
}

#[test]
fn table_weights_are_a_checked_single_batch_inversion() {
    let fixture = tiny_fixture_v1();
    let weights = table_inverse_weights_v1(TINY_GEOMETRY_V1, fixture.context.z).unwrap();
    assert_eq!(weights.len(), TINY_GEOMETRY_V1.table_values);
    for (y, weight) in weights.into_iter().enumerate() {
        assert_eq!(
            (fixture.context.z - Scalar::from_u64(y as u64)) * weight,
            Scalar::one()
        );
    }
    let source = include_str!("rns_native_global_membership_direct.rs");
    assert_eq!(source.matches(".invert()").count(), 1);
}

#[test]
fn core_codec_header_truncation_and_cap_mutations_fail_closed() {
    let fixture = tiny_fixture_v1();
    let wire = &cached_tiny_proof_v1().wire;

    let mut changed_core = wire.to_vec();
    changed_core[HEADER_BYTES_V1 + 7] ^= 1;
    refresh_codec_v1(&mut changed_core);
    assert!(verify_tiny_v1(&fixture, &changed_core).is_err());

    let mut changed_codec = wire.to_vec();
    let last = changed_codec.len() - 1;
    changed_codec[last] ^= 1;
    assert!(matches!(
        verify_tiny_v1(&fixture, &changed_codec),
        Err(RnsNativeGlobalMembershipDirectErrorV1::InvalidIntegrity)
    ));
    assert!(matches!(
        ProofViewV1::decode_v1(wire, TINY_GEOMETRY_V1, wire.len() - 1),
        Err(RnsNativeGlobalMembershipDirectErrorV1::ProofCapExceeded)
    ));
    assert!(ProofViewV1::decode_v1(&wire[..wire.len() - 1], TINY_GEOMETRY_V1, 64 * 1024).is_err());

    let mut changed_header = wire.to_vec();
    changed_header[18] ^= 1;
    assert!(verify_tiny_v1(&fixture, &changed_header).is_err());
}

#[test]
fn production_wire_accounting_and_retired_total_rejection_are_exact() {
    assert_eq!(ACTIVE_PLANES_V1, 32_408);
    assert_eq!(U_COORDINATES_V1, 16_384);
    assert_eq!(TABLE_VALUES_V1, 32_768);
    assert_eq!(ACTIVE_LOOKUP_VALUES_V1, 530_972_672);
    assert_eq!(RETIRED_38_LIMB_ACTIVE_LOOKUP_VALUES_V1, 520_486_912);
    assert_eq!(VECTOR_COMMITMENTS_V1, 2);
    assert_eq!(ACTIVE_MULTIPLICATION_GATES_V1, 0);
    assert_eq!(CONSTRAINTS_V1, 2);
    assert_eq!(PROOF_POINTS_V1, 43);
    assert_eq!(PROOF_SCALARS_V1, 5);
    assert_eq!(CORE_BYTES_V1, 1_579);
    assert_eq!(HEADER_BYTES_V1, 40);
    assert_eq!(CODEC_DIGEST_BYTES_V1, 32);
    assert_eq!(OWNED_WIRE_BYTES_V1, 1_651);
    assert_eq!(MIN_WIRE_BYTES_V1, 1_652);
    assert_eq!(PARENT_RESIDUAL_CAP_BYTES_V1, 110_115);
    assert_eq!(RNS_NATIVE_GLOBAL_MEMBERSHIP_RESIDUAL_MAX_BYTES_V1, 108_464);
    assert_eq!(GBP_CHALLENGES_V1, 19);
    assert!(u64_is_strictly_below_scalar_modulus_v1(
        ACTIVE_LOOKUP_VALUES_V1
    ));
    assert!(u64_is_strictly_below_scalar_modulus_v1(
        TABLE_VALUES_V1 as u64
    ));

    let core = vec![0_u8; CORE_BYTES_V1];
    let downstream = vec![1_u8; RNS_NATIVE_GLOBAL_MEMBERSHIP_RESIDUAL_MAX_BYTES_V1];
    let exact = encode_wire_v1(MembershipGeometryV1::PRODUCTION, &core, &downstream)
        .expect("exact production cap");
    assert_eq!(exact.len(), PARENT_RESIDUAL_CAP_BYTES_V1);
    let one_too_many = vec![1_u8; downstream.len() + 1];
    assert!(matches!(
        encode_wire_v1(MembershipGeometryV1::PRODUCTION, &core, &one_too_many,),
        Err(RnsNativeGlobalMembershipDirectErrorV1::ProofCapExceeded)
    ));

    let retired = MembershipGeometryV1 {
        active_planes: 31_768,
        u_coordinates: U_COORDINATES_V1,
        table_values: TABLE_VALUES_V1,
    };
    assert_eq!(
        retired.active_lookup_values_v1().unwrap(),
        RETIRED_38_LIMB_ACTIVE_LOOKUP_VALUES_V1
    );
    assert!(matches!(
        retired.validate_v1(),
        Err(RnsNativeGlobalMembershipDirectErrorV1::RetiredGeometry)
    ));

    let mut retired_header = encode_wire_v1(MembershipGeometryV1::PRODUCTION, &core, b"x")
        .expect("minimal production envelope");
    retired_header[28..36].copy_from_slice(&RETIRED_38_LIMB_ACTIVE_LOOKUP_VALUES_V1.to_be_bytes());
    assert!(matches!(
        ProofViewV1::decode_v1(
            &retired_header,
            MembershipGeometryV1::PRODUCTION,
            PARENT_RESIDUAL_CAP_BYTES_V1,
        ),
        Err(RnsNativeGlobalMembershipDirectErrorV1::RetiredGeometry)
    ));
}

#[test]
fn source_contract_cycle_exclusions_and_release_boundary_remain_explicit() {
    let source = include_str!("rns_native_global_membership_direct.rs");
    for required in [
        "U-sum=coordinatewise-sum-of-exact-active-plane-order",
        "exclude-all-360-inverse-sumcheck-virtual-planes",
        "M-commitment=canonical-T256-G[0..32768)-plus-H-mask",
        "CG-only-constraint0=sum-v-U_sum[v]-sum-y-Q_z[y]*M[y]=0",
        "CG-only-constraint1=sum-y-M[y]-530972672=0",
        "Q_z-batch-inverts-all-32768-checked-nonzero-denominators-with-one-field-inversion",
        "move-only-source",
        "take-already-aggregated-U-sum-opening-exactly-once",
        "inverse-core-handoff-owns-zeroizing-U-sum-values-and-mask",
        "take-M-opening-exactly-once",
        "membership-token-is-move-only-and-owns-inverse-predecessor",
        "exclude-post-z-binding,inverse-residual,inverse-binding,inverse-codec",
        "admit-predecessor-inverse-binding-only-after-core-verification",
        "retired-38-limb-Q-mask-blocks=1520",
        "retired-total=520486912-is-invalid",
        "current-40-limb-Q-mask-blocks=1600",
        "current-total=530972672",
        "retired-total-is-stale-geometry-not-a-semantic-subset",
        "assumptions=A-and-M-commitments-fixed-before-parent-z",
        "SHAKE256-RFC9380-derived-T256-G/H-multigenerator-discrete-relation-and-basis-independence",
        "generalized-BP-knowledge-soundness-in-the-Keccak-ROM",
        "accepted-compact-inverse-fixes-U[p,v]=(z-A[p,v])^-1",
        "table-embeddings-0..32767-are-distinct",
        "N=530972672<pT",
        "actual-pole-residues-are-in-1..530972672-and-therefore-nonzero-mod-pT",
        "H(X)=P_A'(X)P_T(X)-P_A(X)*sum_y(M[y]*P_T(X)/(X-y))",
        "sum-M=N-cancels-leading-term",
        "invalid-membership-implies-H-nonzero-and-degree<=530972672+32768-2=531005438",
        "H-identically-zero-implies-no-outside-table-pole-and-M-residues-equal-the-actual-multiplicities",
        "ideal-parent-z-conditioned-outside-table-error<=531005438/(pT-32768)",
        "challenge-exhaustion-fails-closed",
        "union-parent-z-wide-reduction-and-bounded-rejection,compact-inverse,and-generalized-BP-errors",
        "19-GBP-accepted-nonzero-wide-reduction-challenges-each-bounded-to-128-attempts",
        "standard-Keccak-ROM-query-loss",
        "pub(super) trait RnsNativeGlobalMembershipOpeningSourceV1",
        "pub(super) trait RnsNativeGlobalMultiplicityOpeningSourceV1",
        "fn take_u_sum_opening_v1(",
        "fn take_multiplicity_opening_v1(",
        "impl Drop for SecretScalarsV1",
        "impl Drop for SecretScalarV1",
        "derive_u_sum_commitment_v1",
        "verify_kernel_with_u_sum_for_suite_v1",
        "VectorCommitmentOpening::take_mask_from_slot",
        "u64_is_strictly_below_scalar_modulus_v1",
        "previous: super::RnsNativeGlobalInverseProductPrerequisiteV1",
        "previous.u_sum_commitment()",
        "post_z.multiplicity()",
        "pub(in super::super::super) fn verify_rns_native_global_membership_v1",
        "prove_combined_for_suites_v1",
        "PendingMembershipOpeningSourceV1",
        "let membership_wire = prove_kernel_for_suite_v1",
        "Ok(inverse_core.seal_v1(&membership_wire)?)",
    ] {
        assert!(
            source.contains(required),
            "missing source guard: {required}"
        );
    }
    assert_eq!(source.matches("source.take_u_sum_opening_v1(").count(), 1);
    assert_eq!(
        source
            .matches("source.take_multiplicity_opening_v1(")
            .count(),
        1
    );
    let token = source
        .find("pub(in super::super::super) struct RnsNativeGlobalMembershipPrerequisiteV1")
        .expect("move-only membership prerequisite");
    let token_attributes = &source[token.saturating_sub(512)..token];
    assert!(!token_attributes.contains("derive(Clone"));
    assert!(!token_attributes.contains("derive(Copy"));
    assert!(!source.contains("DIRECT_MEMBERSHIP_RELATION_VERIFIED_V1: bool = false"));
    assert!(!source.contains("MULTIPLICITY_NONNEGATIVE_RANGE_VERIFIED_V1: bool = true"));
    assert!(!source.contains("CROSS_FIELD_GLOBAL_LOOKUP_VERIFIED_V1: bool = true"));
    assert!(!source.contains("RELEASE_READY_V1: bool = true"));
    let membership_first = source
        .find("let membership_wire = prove_kernel_for_suite_v1")
        .expect("membership proof construction");
    let inverse_seal = source
        .find("Ok(inverse_core.seal_v1(&membership_wire)?)")
        .expect("inverse envelope seal");
    assert!(membership_first < inverse_seal);

    let inverse_parent = include_str!("rns_native_global_inverse_product_sumcheck.rs");
    assert_eq!(
        inverse_parent
            .matches("mod rns_native_global_membership_direct;")
            .count(),
        1
    );
}
