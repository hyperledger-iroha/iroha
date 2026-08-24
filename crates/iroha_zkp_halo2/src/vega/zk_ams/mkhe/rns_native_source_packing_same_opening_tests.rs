use std::{
    cell::{Cell, RefCell},
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
    sync::OnceLock,
};

use super::*;

// Full focused-module execution ledger after the public-fixture cache below.
// A "relation MSM" is a `prepare_relation_v1` path which reaches the fixed
// 16,384-term secret multiexponentiation.  Successful replay also counts the
// two receipt/finish rejection probes which stop immediately before that MSM.
// A full commitment scan additionally counts the two cached point-root scans,
// the independent nonidentity reconstruction scan, and the three replay/error
// probes which stop after collecting all points.
const EXPECTED_FOCUSED_PROOF_WIRES_V1: usize = 2;
const EXPECTED_FOCUSED_RELATION_MSMS_V1: usize = 11;
const EXPECTED_FOCUSED_SUCCESSFUL_REPLAYS_V1: usize = 13;
const EXPECTED_FOCUSED_FULL_COMMITMENT_SCANS_V1: usize = 17;
const FIXTURE_PUBLIC_OWNER_POINTS_V1: usize = 2;
const FIXTURE_PUBLIC_RADIX_POINTS_PER_OWNER_V1: usize = RADIX_LOW_DIGITS_V1 + 1;
const FIXTURE_PUBLIC_POINT_CACHE_POINTS_V1: usize = FIXTURE_PUBLIC_OWNER_POINTS_V1
    + FIXTURE_PUBLIC_OWNER_POINTS_V1 * FIXTURE_PUBLIC_RADIX_POINTS_PER_OWNER_V1;

const _: () = {
    assert!(RNS_NATIVE_SOURCE_PACKING_SAME_OPENING_SOURCE_SETTLED_V1);
};

fn digest_v1(tag: u8) -> [u8; DIGEST_BYTES_V1] {
    [tag; DIGEST_BYTES_V1]
}

fn safe_core_v1() -> RnsNativeSourcePackingSafeCoreV1 {
    RnsNativeSourcePackingSafeCoreV1 {
        terminal_predecessor_context_binding_digest: digest_v1(10),
        candidate_pre_direct_inventory_context_digest: digest_v1(11),
        candidate_pre_direct_inventory_root: digest_v1(12),
        existing_radix_candidate_root: digest_v1(13),
        direct_core_safe_digest: digest_v1(14),
    }
}

fn outer_bindings_v1(tag: u8) -> RnsNativeSourcePackingCombinedOuterBindingsV1 {
    let mut bindings = RnsNativeSourcePackingCombinedOuterBindingsV1 {
        source_statement_anchor_digest: digest_v1(tag),
        source_final_aggregation_schedule_digest: digest_v1(tag.wrapping_add(1)),
        enclosing_packing_binding_digest: digest_v1(tag.wrapping_add(2)),
        inventory_prior_context_digest: digest_v1(tag.wrapping_add(3)),
        inventory_root: digest_v1(tag.wrapping_add(4)),
        inventory_continuation_digest: digest_v1(tag.wrapping_add(5)),
        inventory_binding_digest: digest_v1(tag.wrapping_add(6)),
        direct_binding_digest: digest_v1(tag.wrapping_add(7)),
        comparator_binding_digest: digest_v1(tag.wrapping_add(8)),
        comparator_range_carry_binding_digest: digest_v1(tag.wrapping_add(9)),
        small_sign_disjointness_binding_digest: digest_v1(tag.wrapping_add(10)),
        q_mask_linear_relations_binding_digest: digest_v1(tag.wrapping_add(11)),
        existing_radix_binding_digest: digest_v1(tag.wrapping_add(12)),
        radix_complement_binding_digest: digest_v1(tag.wrapping_add(13)),
        centering_subtraction_binding_digest: digest_v1(tag.wrapping_add(14)),
        global_lookup_pre_z_binding_digest: digest_v1(tag.wrapping_add(15)),
        global_lookup_post_z_binding_digest: digest_v1(tag.wrapping_add(16)),
        global_inverse_product_binding_digest: digest_v1(tag.wrapping_add(17)),
        global_membership_binding_digest: digest_v1(tag.wrapping_add(18)),
        combined_outer_binding_digest: [0; DIGEST_BYTES_V1],
    };
    bindings.combined_outer_binding_digest = bindings.canonical_combined_outer_binding_digest_v1();
    bindings
}

fn context_v1() -> RnsNativeSourcePackingSameOpeningContextV1 {
    let mut context = RnsNativeSourcePackingSameOpeningContextV1 {
        profile_manifest_digest: canonical_profile_manifest_digest_v1()
            .expect("canonical profile manifest"),
        source_binding_digest: digest_v1(3),
        main_snapshot_digest: digest_v1(4),
        nonce_snapshot_digest: digest_v1(5),
        source_receipt_digest: [0; DIGEST_BYTES_V1],
        source_formula_digest: digest_v1(7),
        source_mapping_digest: digest_v1(8),
        safe_core: safe_core_v1(),
    };
    context.source_receipt_digest = canonical_source_receipt_digest_v1(context);
    context
}

fn with_canonical_receipt_v1(
    mut context: RnsNativeSourcePackingSameOpeningContextV1,
) -> RnsNativeSourcePackingSameOpeningContextV1 {
    context.source_receipt_digest = canonical_source_receipt_digest_v1(context);
    context
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FixtureModeV1 {
    IdentityQ,
    NonIdentityQ,
    NonIdentitySignedQ,
    NonIdentitySignedQAtWrongOwner,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ReplayActionV1 {
    Success,
    PointError,
    Error,
    Panic,
    FinishError,
    WrongReceipt,
}

fn owner_mask_v1(mode: FixtureModeV1, ordinal: usize) -> Scalar {
    match mode {
        FixtureModeV1::IdentityQ => Scalar::zero(),
        FixtureModeV1::NonIdentityQ if ordinal == 0 => Scalar::one(),
        FixtureModeV1::NonIdentityQ => Scalar::zero(),
        FixtureModeV1::NonIdentitySignedQ if ordinal == DIFFERENCE_GROUPS_V1 => Scalar::one(),
        FixtureModeV1::NonIdentitySignedQ => Scalar::zero(),
        FixtureModeV1::NonIdentitySignedQAtWrongOwner if ordinal == DIFFERENCE_GROUPS_V1 + 1 => {
            Scalar::one()
        }
        FixtureModeV1::NonIdentitySignedQAtWrongOwner => Scalar::zero(),
    }
}

fn radix_share_v1(digit: usize) -> Scalar {
    static SHARES_V1: OnceLock<[Scalar; RADIX_LOW_DIGITS_V1 + 1]> = OnceLock::new();
    SHARES_V1.get_or_init(|| {
        let inverse_eighteen = Scalar::from_u64(18).inverse().expect("18 is nonzero");
        let inverse_radix = Scalar::from_u64(RADIX_BASE_V1)
            .inverse()
            .expect("radix is nonzero");
        let mut shares = [Scalar::zero(); RADIX_LOW_DIGITS_V1 + 1];
        let mut share = inverse_eighteen;
        for destination in &mut shares {
            *destination = share;
            share *= inverse_radix;
        }
        shares
    })[digit]
}

fn owner_mask_index_v1(mode: FixtureModeV1, ordinal: usize) -> usize {
    let mask = owner_mask_v1(mode, ordinal);
    if mask == Scalar::zero() {
        0
    } else {
        assert_eq!(
            mask,
            Scalar::one(),
            "fixture cache only admits the public mask values zero and one"
        );
        1
    }
}

/// Exact deterministic public fixture points retained process-locally.
///
/// This cache contains no witness, nonce, RNG state, replay owner, derived-mask
/// provider, one-shot capability, or runtime mask. The two owner points and
/// their public radix multiples are reconstructed with the exact operations
/// previously repeated at every source access.
fn fixture_public_points_v1() -> &'static [Point; FIXTURE_PUBLIC_POINT_CACHE_POINTS_V1] {
    static POINTS_V1: OnceLock<[Point; FIXTURE_PUBLIC_POINT_CACHE_POINTS_V1]> = OnceLock::new();
    POINTS_V1.get_or_init(|| {
        let generators = ZkAmsT256BulletproofSuiteV1::generators();
        let owners = [
            generators.g_bold[0] + generators.h.mul_scalar(Scalar::zero()),
            generators.g_bold[0] + generators.h.mul_scalar(Scalar::one()),
        ];
        let mut points = [owners[0]; FIXTURE_PUBLIC_POINT_CACHE_POINTS_V1];
        points[0] = owners[0];
        points[1] = owners[1];
        for mask in 0..FIXTURE_PUBLIC_OWNER_POINTS_V1 {
            for digit in 0..FIXTURE_PUBLIC_RADIX_POINTS_PER_OWNER_V1 {
                points[FIXTURE_PUBLIC_OWNER_POINTS_V1
                    + mask * FIXTURE_PUBLIC_RADIX_POINTS_PER_OWNER_V1
                    + digit] = owners[mask].mul_scalar(radix_share_v1(digit));
            }
        }
        points
    })
}

fn owner_value_and_mask_commitment_v1(mode: FixtureModeV1, ordinal: usize) -> Point {
    fixture_public_points_v1()[owner_mask_index_v1(mode, ordinal)]
}

fn radix_scaled_owner_value_and_mask_commitment_v1(
    mode: FixtureModeV1,
    ordinal: usize,
    digit: usize,
) -> Point {
    let mask = owner_mask_index_v1(mode, ordinal);
    fixture_public_points_v1()
        [FIXTURE_PUBLIC_OWNER_POINTS_V1 + mask * FIXTURE_PUBLIC_RADIX_POINTS_PER_OWNER_V1 + digit]
}

struct FixtureReplaySourceV1 {
    mode: FixtureModeV1,
    action: ReplayActionV1,
    authenticated_source_axes: RnsNativeSourcePackingAuthenticatedSourceAxesV1,
    schedule_digest: [u8; DIGEST_BYTES_V1],
    difference_low_reads: Cell<usize>,
    difference_top_reads: Cell<usize>,
    signed_reads: Cell<usize>,
    replayed: bool,
    drop_flag: Option<Rc<Cell<bool>>>,
    touch_count: Option<Rc<Cell<usize>>>,
}

impl FixtureReplaySourceV1 {
    fn new_v1(
        mode: FixtureModeV1,
        action: ReplayActionV1,
        drop_flag: Option<Rc<Cell<bool>>>,
    ) -> Self {
        let context = context_v1();
        Self {
            mode,
            action,
            authenticated_source_axes: context.authenticated_source_axes_v1(),
            schedule_digest: canonical_replay_schedule_digest_v1(context)
                .expect("canonical replay schedule"),
            difference_low_reads: Cell::new(0),
            difference_top_reads: Cell::new(0),
            signed_reads: Cell::new(0),
            replayed: false,
            drop_flag,
            touch_count: None,
        }
    }

    fn success_v1(mode: FixtureModeV1) -> Self {
        Self::new_v1(mode, ReplayActionV1::Success, None)
    }

    fn success_for_context_v1(
        mode: FixtureModeV1,
        context: RnsNativeSourcePackingSameOpeningContextV1,
    ) -> Self {
        let mut source = Self::success_v1(mode);
        source.authenticated_source_axes = context.authenticated_source_axes_v1();
        source.schedule_digest =
            canonical_replay_schedule_digest_v1(context).expect("canonical replay schedule");
        source
    }

    fn with_touch_count_v1(mut self, touch_count: Rc<Cell<usize>>) -> Self {
        self.touch_count = Some(touch_count);
        self
    }

    fn touch_v1(&self) {
        if let Some(count) = &self.touch_count {
            count.set(count.get() + 1);
        }
    }

    fn receipt_v1(&self) -> RnsNativeSourcePackingReplayReceiptV1 {
        RnsNativeSourcePackingReplayReceiptV1 {
            source_binding_digest: self.authenticated_source_axes.source_binding_digest,
            canonical_replay_schedule_digest: self.schedule_digest,
            owner_count: OWNERS_V1 as u16,
            coordinates: VECTOR_COORDINATES_V1 as u16,
        }
    }
}

impl Drop for FixtureReplaySourceV1 {
    fn drop(&mut self) {
        if let Some(flag) = &self.drop_flag {
            flag.set(true);
        }
    }
}

impl RnsNativeSourcePackingAggregateReplayV1 for FixtureReplaySourceV1 {
    fn authenticated_source_axes_v1(&self) -> RnsNativeSourcePackingAuthenticatedSourceAxesV1 {
        self.touch_v1();
        self.authenticated_source_axes
    }

    fn canonical_replay_schedule_digest_v1(&self) -> [u8; DIGEST_BYTES_V1] {
        self.touch_v1();
        self.schedule_digest
    }

    fn difference_low_commitment_v1(
        &self,
        group: usize,
        digit: usize,
    ) -> Result<Point, RnsNativeSourcePackingSameOpeningErrorV1> {
        self.touch_v1();
        if group >= DIFFERENCE_GROUPS_V1 || digit >= RADIX_LOW_DIGITS_V1 {
            return Err(RnsNativeSourcePackingSameOpeningErrorV1::InvalidGeometry);
        }
        if self.action == ReplayActionV1::PointError && group == 0 && digit == 0 {
            return Err(RnsNativeSourcePackingSameOpeningErrorV1::SourceUnavailable);
        }
        self.difference_low_reads
            .set(self.difference_low_reads.get() + 1);
        Ok(radix_scaled_owner_value_and_mask_commitment_v1(
            self.mode, group, digit,
        ))
    }

    fn difference_top_commitment_v1(
        &self,
        group: usize,
    ) -> Result<Point, RnsNativeSourcePackingSameOpeningErrorV1> {
        self.touch_v1();
        if group >= DIFFERENCE_GROUPS_V1 {
            return Err(RnsNativeSourcePackingSameOpeningErrorV1::InvalidGeometry);
        }
        self.difference_top_reads
            .set(self.difference_top_reads.get() + 1);
        Ok(radix_scaled_owner_value_and_mask_commitment_v1(
            self.mode,
            group,
            RADIX_LOW_DIGITS_V1,
        ))
    }

    fn signed_commitment_v1(
        &self,
        record: usize,
        role: RnsNativeSignedSourceRoleV1,
        plane: usize,
    ) -> Result<Point, RnsNativeSourcePackingSameOpeningErrorV1> {
        self.touch_v1();
        if record >= RECORDS_V1 || plane >= PLANES_PER_SIGNED_ROLE_V1 {
            return Err(RnsNativeSourcePackingSameOpeningErrorV1::InvalidGeometry);
        }
        let role_ordinal = role as usize;
        let signed = (record * SIGNED_ROLES_V1 + role_ordinal) * PLANES_PER_SIGNED_ROLE_V1 + plane;
        let ordinal = DIFFERENCE_GROUPS_V1 + signed;
        self.signed_reads.set(self.signed_reads.get() + 1);
        Ok(owner_value_and_mask_commitment_v1(self.mode, ordinal))
    }

    fn replay_tau_aggregate_v1(
        &mut self,
        tau: Scalar,
        destination: &mut ZeroizingT256ScalarVecV1,
    ) -> Result<RnsNativeSourcePackingReplayReceiptV1, RnsNativeSourcePackingSameOpeningErrorV1>
    {
        self.touch_v1();
        if self.replayed || tau.is_zero() || destination.len() != VECTOR_COORDINATES_V1 {
            return Err(RnsNativeSourcePackingSameOpeningErrorV1::InvalidGeometry);
        }
        self.replayed = true;
        for value in destination.as_mut_slice() {
            value.clear_secret();
        }
        destination.as_mut_slice()[0] = Scalar::from_u64(99);
        match self.action {
            ReplayActionV1::Error => {
                return Err(RnsNativeSourcePackingSameOpeningErrorV1::SourceUnavailable);
            }
            ReplayActionV1::Panic => panic!("fixture replay panic after confidential write"),
            ReplayActionV1::Success
            | ReplayActionV1::PointError
            | ReplayActionV1::FinishError
            | ReplayActionV1::WrongReceipt => {}
        }
        let mut power = Scalar::one();
        let mut aggregate = ZeroizingScalarSlotV1::zero_v1();
        for _ in 0..OWNERS_V1 {
            *aggregate.as_mut() += power;
            power *= tau;
        }
        destination.as_mut_slice()[0] = *aggregate.as_ref();
        let mut receipt = self.receipt_v1();
        if self.action == ReplayActionV1::WrongReceipt {
            receipt.owner_count -= 1;
        }
        Ok(receipt)
    }

    fn finish_v1(
        self,
    ) -> Result<RnsNativeSourcePackingReplayReceiptV1, RnsNativeSourcePackingSameOpeningErrorV1>
    {
        self.touch_v1();
        if !self.replayed
            || self.action != ReplayActionV1::Success
            || self.difference_low_reads.get() != DIFFERENCE_GROUPS_V1 * RADIX_LOW_DIGITS_V1
            || self.difference_top_reads.get() != DIFFERENCE_GROUPS_V1
            || self.signed_reads.get() != SIGNED_OWNERS_V1
        {
            return Err(RnsNativeSourcePackingSameOpeningErrorV1::SourceUnavailable);
        }
        Ok(self.receipt_v1())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MaskActionV1 {
    Success,
    ErrorFirst,
    PanicFirst,
    ErrorLast,
    FinishError,
    WrongReceipt,
}

struct FixtureMaskSourceV1 {
    mode: FixtureModeV1,
    action: MaskActionV1,
    next: usize,
    point_root: [u8; DIGEST_BYTES_V1],
    schedule_digest: [u8; DIGEST_BYTES_V1],
    binding_digest: [u8; DIGEST_BYTES_V1],
    drop_flag: Option<Rc<Cell<bool>>>,
    take_count: Option<Rc<Cell<usize>>>,
}

impl FixtureMaskSourceV1 {
    fn new_v1(
        mode: FixtureModeV1,
        action: MaskActionV1,
        point_root: [u8; DIGEST_BYTES_V1],
        drop_flag: Option<Rc<Cell<bool>>>,
    ) -> Self {
        Self {
            mode,
            action,
            next: 0,
            point_root,
            schedule_digest: canonical_replay_schedule_digest_v1(context_v1())
                .expect("canonical replay schedule"),
            binding_digest: digest_v1(40),
            drop_flag,
            take_count: None,
        }
    }

    fn with_take_count_v1(mut self, take_count: Rc<Cell<usize>>) -> Self {
        self.take_count = Some(take_count);
        self
    }

    fn receipt_v1(&self) -> RnsNativeSourcePackingMaskReceiptV1 {
        RnsNativeSourcePackingMaskReceiptV1 {
            opening_binding_digest: self.binding_digest,
            point_root: self.point_root,
            canonical_replay_schedule_digest: self.schedule_digest,
            owner_count: OWNERS_V1 as u16,
        }
    }
}

impl Drop for FixtureMaskSourceV1 {
    fn drop(&mut self) {
        if let Some(flag) = &self.drop_flag {
            flag.set(true);
        }
    }
}

impl RnsNativeSourcePackingDerivedMaskSourceV1 for FixtureMaskSourceV1 {
    fn opening_binding_digest_v1(&self) -> [u8; DIGEST_BYTES_V1] {
        self.binding_digest
    }

    fn point_root_v1(&self) -> [u8; DIGEST_BYTES_V1] {
        self.point_root
    }

    fn canonical_replay_schedule_digest_v1(&self) -> [u8; DIGEST_BYTES_V1] {
        self.schedule_digest
    }

    fn take_next_mask_v1(
        &mut self,
        expected: RnsNativeSourcePackingOwnerCoordinateV1,
        destination: &mut Scalar,
    ) -> Result<(), RnsNativeSourcePackingSameOpeningErrorV1> {
        if let Some(count) = &self.take_count {
            count.set(count.get() + 1);
        }
        if self.next >= OWNERS_V1 || expected != owner_coordinate_v1(self.next)? {
            return Err(RnsNativeSourcePackingSameOpeningErrorV1::MaskUnavailable);
        }
        *destination = owner_mask_v1(self.mode, self.next);
        if self.next == 0 {
            match self.action {
                MaskActionV1::ErrorFirst => {
                    return Err(RnsNativeSourcePackingSameOpeningErrorV1::MaskUnavailable);
                }
                MaskActionV1::PanicFirst => panic!("fixture mask panic after confidential write"),
                MaskActionV1::Success
                | MaskActionV1::ErrorLast
                | MaskActionV1::FinishError
                | MaskActionV1::WrongReceipt => {}
            }
        }
        if self.next == OWNERS_V1 - 1 && self.action == MaskActionV1::ErrorLast {
            *destination = Scalar::from_u64(99);
            return Err(RnsNativeSourcePackingSameOpeningErrorV1::MaskUnavailable);
        }
        self.next += 1;
        Ok(())
    }

    fn finish_v1(
        self,
    ) -> Result<RnsNativeSourcePackingMaskReceiptV1, RnsNativeSourcePackingSameOpeningErrorV1> {
        if self.next != OWNERS_V1
            || !matches!(
                self.action,
                MaskActionV1::Success | MaskActionV1::WrongReceipt
            )
        {
            return Err(RnsNativeSourcePackingSameOpeningErrorV1::MaskUnavailable);
        }
        let mut receipt = self.receipt_v1();
        if self.action == MaskActionV1::WrongReceipt {
            receipt.owner_count -= 1;
        }
        Ok(receipt)
    }
}

struct DeterministicRngV1(u64);

impl ProofRandomSource for DeterministicRngV1 {
    fn fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), GeneralizedBulletproofErrorV1> {
        for byte in destination {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            *byte = self.0 as u8;
        }
        Ok(())
    }
}

struct UnavailableRngV1;

impl ProofRandomSource for UnavailableRngV1 {
    fn fill_bytes(&mut self, _destination: &mut [u8]) -> Result<(), GeneralizedBulletproofErrorV1> {
        Err(GeneralizedBulletproofErrorV1::RandomnessUnavailable)
    }
}

struct CountingUnavailableRngV1(Rc<Cell<usize>>);

impl ProofRandomSource for CountingUnavailableRngV1 {
    fn fill_bytes(&mut self, _destination: &mut [u8]) -> Result<(), GeneralizedBulletproofErrorV1> {
        self.0.set(self.0.get() + 1);
        Err(GeneralizedBulletproofErrorV1::RandomnessUnavailable)
    }
}

struct FixtureCombinedPredecessorV1<'a> {
    successor: &'a [u8],
    safe_core: RnsNativeSourcePackingSafeCoreV1,
    outer_bindings: RnsNativeSourcePackingCombinedOuterBindingsV1,
    access_log: Option<Rc<RefCell<Vec<u8>>>>,
}

impl<'a> FixtureCombinedPredecessorV1<'a> {
    fn new_v1(
        successor: &'a [u8],
        safe_core: RnsNativeSourcePackingSafeCoreV1,
        outer_bindings: RnsNativeSourcePackingCombinedOuterBindingsV1,
    ) -> Self {
        Self {
            successor,
            safe_core,
            outer_bindings,
            access_log: None,
        }
    }

    fn with_access_log_v1(mut self, access_log: Rc<RefCell<Vec<u8>>>) -> Self {
        self.access_log = Some(access_log);
        self
    }

    fn record_access_v1(&self, step: u8) {
        if let Some(log) = &self.access_log {
            log.borrow_mut().push(step);
        }
    }
}

impl<'a> RnsNativeSourcePackingCombinedDirectMembershipPredecessorV1<'a>
    for FixtureCombinedPredecessorV1<'a>
{
    fn same_opening_successor_v1(&self) -> &'a [u8] {
        self.record_access_v1(1);
        self.successor
    }

    fn successor_independent_safe_core_v1(&self) -> RnsNativeSourcePackingSafeCoreV1 {
        self.record_access_v1(2);
        self.safe_core
    }

    fn combined_outer_bindings_v1(&self) -> RnsNativeSourcePackingCombinedOuterBindingsV1 {
        self.record_access_v1(3);
        self.outer_bindings
    }
}

fn combined_predecessor_v1<'a>(
    successor: &'a [u8],
    context: RnsNativeSourcePackingSameOpeningContextV1,
    outer_bindings: RnsNativeSourcePackingCombinedOuterBindingsV1,
) -> FixtureCombinedPredecessorV1<'a> {
    FixtureCombinedPredecessorV1::new_v1(successor, context.safe_core, outer_bindings)
}

fn point_root_v1(mode: FixtureModeV1) -> [u8; DIGEST_BYTES_V1] {
    static IDENTITY_Q_ROOT_V1: OnceLock<[u8; DIGEST_BYTES_V1]> = OnceLock::new();
    static NONIDENTITY_Q_ROOT_V1: OnceLock<[u8; DIGEST_BYTES_V1]> = OnceLock::new();
    static NONIDENTITY_SIGNED_Q_ROOT_V1: OnceLock<[u8; DIGEST_BYTES_V1]> = OnceLock::new();
    static NONIDENTITY_SIGNED_Q_WRONG_OWNER_ROOT_V1: OnceLock<[u8; DIGEST_BYTES_V1]> =
        OnceLock::new();

    let root = match mode {
        FixtureModeV1::IdentityQ => &IDENTITY_Q_ROOT_V1,
        FixtureModeV1::NonIdentityQ => &NONIDENTITY_Q_ROOT_V1,
        FixtureModeV1::NonIdentitySignedQ => &NONIDENTITY_SIGNED_Q_ROOT_V1,
        FixtureModeV1::NonIdentitySignedQAtWrongOwner => &NONIDENTITY_SIGNED_Q_WRONG_OWNER_ROOT_V1,
    };
    *root.get_or_init(|| {
        collect_commitments_v1(&FixtureReplaySourceV1::success_v1(mode))
            .expect("fixture point set")
            .point_root
    })
}

/// Public, process-local output of one real fixture proof and verification.
///
/// This cache contains no prepared relation, replay destination, mask, nonce,
/// one-shot capability, receipt owner, or RNG state.  Every retained field was
/// already public after the equation verified.
struct PublicFixtureArtifactV1 {
    wire: Vec<u8>,
    point_root: [u8; DIGEST_BYTES_V1],
    manifest_digest: [u8; DIGEST_BYTES_V1],
    source_context_digest: [u8; DIGEST_BYTES_V1],
    replay_receipt_digest: [u8; DIGEST_BYTES_V1],
    pre_challenge_binding_digest: [u8; DIGEST_BYTES_V1],
    tau_digest: [u8; DIGEST_BYTES_V1],
    q_digest: [u8; DIGEST_BYTES_V1],
    proof_digest: [u8; DIGEST_BYTES_V1],
    residual_digest: [u8; DIGEST_BYTES_V1],
    binding_digest: [u8; DIGEST_BYTES_V1],
}

impl PublicFixtureArtifactV1 {
    fn frame_v1(&self) -> FrameViewV1<'_> {
        FrameViewV1::decode_v1(&self.wire, FUTURE_DIRECT_MEMBERSHIP_PARENT_CAP_BYTES_V1)
            .expect("cached public fixture frame")
    }

    fn residual_v1(&self) -> &[u8] {
        self.frame_v1().residual
    }

    fn equation_verified_v1(&self) -> EquationVerifiedKernelV1<'_> {
        let frame = self.frame_v1();
        EquationVerifiedKernelV1 {
            residual: frame.residual,
            manifest_digest: self.manifest_digest,
            source_context_digest: self.source_context_digest,
            point_root: self.point_root,
            replay_receipt_digest: self.replay_receipt_digest,
            pre_challenge_binding_digest: self.pre_challenge_binding_digest,
            tau_digest: self.tau_digest,
            q_digest: self.q_digest,
            proof_digest: self.proof_digest,
            codec_digest: frame.codec_digest,
            codec_offset: frame.codec_offset,
        }
    }
}

fn build_public_fixture_v1(
    mode: FixtureModeV1,
    residual: &[u8],
    seed: u64,
    outer_bindings: RnsNativeSourcePackingCombinedOuterBindingsV1,
) -> PublicFixtureArtifactV1 {
    let context = context_v1();
    let point_root = point_root_v1(mode);
    let replay_drop = Rc::new(Cell::new(false));
    let mask_drop = Rc::new(Cell::new(false));
    let mut rng = DeterministicRngV1(seed);
    let wire = prove_rns_native_source_packing_same_opening_kernel_v1(
        context,
        FixtureReplaySourceV1::new_v1(mode, ReplayActionV1::Success, Some(Rc::clone(&replay_drop))),
        FixtureMaskSourceV1::new_v1(
            mode,
            MaskActionV1::Success,
            point_root,
            Some(Rc::clone(&mask_drop)),
        ),
        residual,
        &mut rng,
    )
    .expect("cached fixture proof");
    assert!(replay_drop.get());
    assert!(mask_drop.get());

    let access_log = Rc::new(RefCell::new(Vec::new()));
    let source_touches = Rc::new(Cell::new(0));
    let verify_drop = Rc::new(Cell::new(false));
    let verified = verify_rns_native_source_packing_same_opening_v1(
        combined_predecessor_v1(&wire, context, outer_bindings)
            .with_access_log_v1(Rc::clone(&access_log)),
        context,
        FixtureReplaySourceV1::new_v1(mode, ReplayActionV1::Success, Some(Rc::clone(&verify_drop)))
            .with_touch_count_v1(Rc::clone(&source_touches)),
    )
    .expect("cached fixture verification");
    assert_eq!(&*access_log.borrow(), &[1, 2, 3]);
    assert!(source_touches.get() > 0);
    assert!(verify_drop.get());
    assert_eq!(verified.residual(), residual);
    assert_eq!(verified.point_root(), point_root);

    let manifest_digest = verified.manifest_digest;
    let source_context_digest = verified.source_context_digest;
    let replay_receipt_digest = verified.replay_receipt_digest;
    let pre_challenge_binding_digest = verified.pre_challenge_binding_digest;
    let tau_digest = verified.tau_digest;
    let q_digest = verified.q_digest;
    let proof_digest = verified.proof_digest;
    let residual_digest = verified.residual_digest;
    let binding_digest = verified.binding_digest;
    drop(verified);
    PublicFixtureArtifactV1 {
        wire,
        point_root,
        manifest_digest,
        source_context_digest,
        replay_receipt_digest,
        pre_challenge_binding_digest,
        tau_digest,
        q_digest,
        proof_digest,
        residual_digest,
        binding_digest,
    }
}

fn identity_public_fixture_v1() -> &'static PublicFixtureArtifactV1 {
    static FIXTURE_V1: OnceLock<PublicFixtureArtifactV1> = OnceLock::new();
    FIXTURE_V1.get_or_init(|| {
        build_public_fixture_v1(
            FixtureModeV1::IdentityQ,
            &[0xa5, 0x5a, 0x11],
            0x1234_5678_9abc_def1,
            outer_bindings_v1(92),
        )
    })
}

fn nonidentity_public_fixture_v1() -> &'static PublicFixtureArtifactV1 {
    static FIXTURE_V1: OnceLock<PublicFixtureArtifactV1> = OnceLock::new();
    FIXTURE_V1.get_or_init(|| {
        build_public_fixture_v1(
            FixtureModeV1::NonIdentitySignedQ,
            &[0x31, 0x41, 0x59],
            0x0ddc_0ffe_e15e_beef,
            outer_bindings_v1(91),
        )
    })
}

fn assert_fixture_equation_rejects_v1(
    artifact: &PublicFixtureArtifactV1,
    mode: FixtureModeV1,
    wire: &[u8],
) {
    let frame = FrameViewV1::decode_v1(wire, FUTURE_DIRECT_MEMBERSHIP_PARENT_CAP_BYTES_V1)
        .expect("mutated fixture frame");
    let tau = derive_tau_v1(artifact.pre_challenge_binding_digest)
        .expect("cached fixture tau derivation");
    let mut aggregate_mask = Scalar::zero();
    let mut power = Scalar::one();
    for ordinal in 0..OWNERS_V1 {
        aggregate_mask += power * owner_mask_v1(mode, ordinal);
        power *= tau;
    }
    let h = ZkAmsT256BulletproofSuiteV1::generators().h;
    let q = h.mul_scalar(aggregate_mask);
    let challenge =
        derive_schnorr_challenge_v1(artifact.pre_challenge_binding_digest, tau, &q, &frame.a)
            .expect("mutated fixture challenge");
    assert_ne!(
        h.mul_scalar(frame.z),
        frame.a + q.mul_scalar(challenge),
        "mutated public frame must fail the exact Schnorr equation"
    );
}

fn rewrite_codec_v1(wire: &mut [u8]) {
    let codec_offset = wire.len() - CODEC_DIGEST_BYTES_V1;
    let digest = codec_digest_v1(&wire[..codec_offset]);
    wire[codec_offset..].copy_from_slice(&digest);
}

fn invoke_replay_once_v1(
    mut source: FixtureReplaySourceV1,
) -> Result<(), RnsNativeSourcePackingSameOpeningErrorV1> {
    let mut destination = zero_aggregate_values_v1()?;
    source.replay_tau_aggregate_v1(Scalar::from_u64(7), &mut destination)?;
    Ok(())
}

#[test]
fn exact_owner_order_geometry_and_cap_are_settled() {
    assert_eq!(DIFFERENCE_GROUPS_V1, 344);
    assert_eq!(SIGNED_OWNERS_V1, 1_032);
    assert_eq!(OWNERS_V1, 1_376);
    assert_eq!(VECTOR_COORDINATES_V1, 16_384);
    assert_eq!(ERROR_POLYNOMIAL_DEGREE_V1, 1_375);
    assert_eq!(SCHNORR_PAYLOAD_BYTES_V1, 65);
    assert_eq!(HEADER_BYTES_V1, 28);
    assert_eq!(OWNED_WIRE_BYTES_V1, 125);
    assert_eq!(MIN_WIRE_BYTES_V1, 126);
    assert_eq!(EXPECTED_FOCUSED_PROOF_WIRES_V1, 2);
    assert_eq!(EXPECTED_FOCUSED_RELATION_MSMS_V1, 11);
    assert_eq!(EXPECTED_FOCUSED_SUCCESSFUL_REPLAYS_V1, 13);
    assert_eq!(EXPECTED_FOCUSED_FULL_COMMITMENT_SCANS_V1, 17);
    assert_eq!(FUTURE_DIRECT_MEMBERSHIP_PARENT_CAP_BYTES_V1, 108_464);
    assert_eq!(
        FUTURE_DIRECT_MEMBERSHIP_PARENT_CAP_BYTES_V1,
        RNS_NATIVE_GLOBAL_MEMBERSHIP_RESIDUAL_MAX_BYTES_V1
    );
    assert_eq!(
        RNS_NATIVE_SOURCE_PACKING_SAME_OPENING_SUCCESSOR_MAX_BYTES_V1,
        108_339
    );
    assert_eq!(
        owner_coordinate_v1(0).expect("D0"),
        RnsNativeSourcePackingOwnerCoordinateV1::Difference { group: 0 }
    );
    assert_eq!(
        owner_coordinate_v1(343).expect("D343"),
        RnsNativeSourcePackingOwnerCoordinateV1::Difference { group: 343 }
    );
    assert_eq!(
        owner_coordinate_v1(344).expect("r first"),
        RnsNativeSourcePackingOwnerCoordinateV1::Signed {
            record: 0,
            role: RnsNativeSignedSourceRoleV1::R,
            plane: 0,
        }
    );
    assert_eq!(
        owner_coordinate_v1(351).expect("r last plane"),
        RnsNativeSourcePackingOwnerCoordinateV1::Signed {
            record: 0,
            role: RnsNativeSignedSourceRoleV1::R,
            plane: 7,
        }
    );
    assert_eq!(
        owner_coordinate_v1(352).expect("e0 first"),
        RnsNativeSourcePackingOwnerCoordinateV1::Signed {
            record: 0,
            role: RnsNativeSignedSourceRoleV1::E0,
            plane: 0,
        }
    );
    assert_eq!(
        owner_coordinate_v1(1_375).expect("last owner"),
        RnsNativeSourcePackingOwnerCoordinateV1::Signed {
            record: 42,
            role: RnsNativeSignedSourceRoleV1::E1,
            plane: 7,
        }
    );
    assert!(owner_coordinate_v1(OWNERS_V1).is_err());
}

#[test]
fn public_fixture_point_cache_matches_uncached_derivation_exactly() {
    fn assert_send_sync<T: Send + Sync>() {}

    assert_eq!(FIXTURE_PUBLIC_OWNER_POINTS_V1, 2);
    assert_eq!(FIXTURE_PUBLIC_RADIX_POINTS_PER_OWNER_V1, 18);
    assert_eq!(FIXTURE_PUBLIC_POINT_CACHE_POINTS_V1, 38);
    assert_eq!(core::mem::size_of::<Point>(), 96);
    assert_eq!(
        core::mem::size_of::<[Point; FIXTURE_PUBLIC_POINT_CACHE_POINTS_V1]>(),
        3_648
    );
    assert_send_sync::<[Point; FIXTURE_PUBLIC_POINT_CACHE_POINTS_V1]>();

    let cached = fixture_public_points_v1();
    assert_eq!(cached.len(), FIXTURE_PUBLIC_POINT_CACHE_POINTS_V1);
    let generators = ZkAmsT256BulletproofSuiteV1::generators();
    for mask in 0..FIXTURE_PUBLIC_OWNER_POINTS_V1 {
        let expected_owner =
            generators.g_bold[0] + generators.h.mul_scalar(Scalar::from_u64(mask as u64));
        assert_eq!(cached[mask], expected_owner);
        assert_eq!(
            cached[mask].to_non_identity_wire_bytes(),
            expected_owner.to_non_identity_wire_bytes()
        );
        for digit in 0..FIXTURE_PUBLIC_RADIX_POINTS_PER_OWNER_V1 {
            let expected = expected_owner.mul_scalar(radix_share_v1(digit));
            let cached_index = FIXTURE_PUBLIC_OWNER_POINTS_V1
                + mask * FIXTURE_PUBLIC_RADIX_POINTS_PER_OWNER_V1
                + digit;
            assert_eq!(cached[cached_index], expected);
            assert_eq!(
                cached[cached_index].to_non_identity_wire_bytes(),
                expected.to_non_identity_wire_bytes()
            );
        }
    }
}

#[test]
fn all_1376_owner_ordinals_match_an_independent_oracle() {
    for ordinal in 0..OWNERS_V1 {
        let expected = if ordinal < DIFFERENCE_GROUPS_V1 {
            RnsNativeSourcePackingOwnerCoordinateV1::Difference {
                group: ordinal as u16,
            }
        } else {
            let signed_unit = ordinal - DIFFERENCE_GROUPS_V1;
            let record = signed_unit / 24;
            let within_record = signed_unit % 24;
            let role = match within_record / 8 {
                0 => RnsNativeSignedSourceRoleV1::R,
                1 => RnsNativeSignedSourceRoleV1::E0,
                2 => RnsNativeSignedSourceRoleV1::E1,
                _ => unreachable!("three signed roles"),
            };
            RnsNativeSourcePackingOwnerCoordinateV1::Signed {
                record: record as u8,
                role,
                plane: (within_record % 8) as u8,
            }
        };
        assert_eq!(owner_coordinate_v1(ordinal).expect("valid owner"), expected);
    }
}

#[test]
fn d_and_signed_source_transposes_pin_slots_offsets_coordinates_and_signed_values() {
    let difference_cases = [
        (0, 0, 0, 0, 0, 0),
        (0, 63, 0, 0, 63, 0),
        (0, 64, 0, 0, 0, 32),
        (7, 16_383, 0, 7, 511, 8_160),
        (8, 0, 1, 0, 896, 0),
        (343, 16_383, 42, 7, 38_143, 8_160),
    ];
    for (g_abs, coordinate, record, g_local, source_slot, byte_offset) in difference_cases {
        let index = difference_source_index_v1(g_abs, coordinate).expect("valid D index");
        assert_eq!(usize::from(index.owner_ordinal), g_abs);
        assert_eq!(usize::from(index.g_abs), g_abs);
        assert_eq!(usize::from(index.record), record);
        assert_eq!(usize::from(index.g_local), g_local);
        assert_eq!(usize::from(index.coordinate), coordinate);
        assert_eq!(index.source_slot as usize, source_slot);
        assert_eq!(usize::from(index.byte_offset), byte_offset);
    }
    assert!(difference_source_index_v1(DIFFERENCE_GROUPS_V1, 0).is_err());
    assert!(difference_source_index_v1(0, VECTOR_COORDINATES_V1).is_err());
    assert_eq!(
        difference_scalar_from_be_bytes_v1(&[0; DIFFERENCE_SCALAR_BYTES_V1])
            .expect("canonical zero"),
        Scalar::zero()
    );
    assert_eq!(
        difference_scalar_from_be_bytes_v1(&Scalar::one().to_be_bytes())
            .expect("canonical positive"),
        Scalar::one()
    );
    assert!(difference_scalar_from_be_bytes_v1(&VEGA_T256_SCALAR_MODULUS_BE_V1).is_err());

    let signed_cases = [
        (0, 0, 0, RnsNativeSignedSourceRoleV1::R, 0, 0, 0, 512, 0),
        (
            0,
            1_023,
            0,
            RnsNativeSignedSourceRoleV1::R,
            0,
            0,
            1_023,
            512,
            8_184,
        ),
        (0, 1_024, 0, RnsNativeSignedSourceRoleV1::R, 0, 1, 0, 513, 0),
        (
            7,
            16_383,
            0,
            RnsNativeSignedSourceRoleV1::R,
            7,
            15,
            1_023,
            639,
            8_184,
        ),
        (8, 0, 0, RnsNativeSignedSourceRoleV1::E0, 0, 0, 0, 640, 0),
        (
            23,
            16_383,
            0,
            RnsNativeSignedSourceRoleV1::E1,
            7,
            15,
            1_023,
            895,
            8_184,
        ),
        (24, 0, 1, RnsNativeSignedSourceRoleV1::R, 0, 0, 0, 1_408, 0),
        (
            1_031,
            16_383,
            42,
            RnsNativeSignedSourceRoleV1::E1,
            7,
            15,
            1_023,
            38_527,
            8_184,
        ),
    ];
    for (
        signed_unit,
        coordinate,
        record,
        role,
        plane,
        local_block,
        coefficient_in_block,
        source_slot,
        byte_offset,
    ) in signed_cases
    {
        let index = signed_source_index_v1(signed_unit, coordinate).expect("valid signed index");
        assert_eq!(usize::from(index.owner_ordinal), 344 + signed_unit);
        assert_eq!(usize::from(index.signed_unit), signed_unit);
        assert_eq!(usize::from(index.record), record);
        assert_eq!(index.role, role);
        assert_eq!(usize::from(index.plane), plane);
        assert_eq!(usize::from(index.coordinate), coordinate);
        assert_eq!(usize::from(index.local_block), local_block);
        assert_eq!(
            usize::from(index.coefficient_in_block),
            coefficient_in_block
        );
        assert_eq!(index.source_slot as usize, source_slot);
        assert_eq!(usize::from(index.byte_offset), byte_offset);
    }
    assert!(signed_source_index_v1(SIGNED_OWNERS_V1, 0).is_err());
    assert!(signed_source_index_v1(0, VECTOR_COORDINATES_V1).is_err());

    for (signed, expected) in [
        (i64::MIN, -Scalar::from_u64(1_u64 << 63)),
        (-9, -Scalar::from_u64(9)),
        (-1, -Scalar::one()),
        (0, Scalar::zero()),
        (1, Scalar::one()),
        (9, Scalar::from_u64(9)),
        (i64::MAX, Scalar::from_u64(i64::MAX as u64)),
    ] {
        assert_eq!(
            signed_scalar_from_twos_complement_be_i64_v1(&signed.to_be_bytes()),
            expected
        );
    }
}

#[test]
fn generic_fixture_accepts_the_valid_identity_q_case() {
    // Every fixture commitment is exactly `<[1,0,...],G>` and every derived
    // mask is zero, so the verifier's recomputed Q is the identity.
    let artifact = identity_public_fixture_v1();
    assert_eq!(artifact.residual_v1(), &[0xa5, 0x5a, 0x11]);
    assert_eq!(
        artifact.wire.len(),
        OWNED_WIRE_BYTES_V1 + artifact.residual_v1().len()
    );
    assert_eq!(artifact.point_root, point_root_v1(FixtureModeV1::IdentityQ));
    assert_ne!(artifact.binding_digest, [0; DIGEST_BYTES_V1]);
}

#[test]
fn combined_predecessor_is_successor_first_safe_core_checked_and_outer_bound_post_equation() {
    let context = context_v1();
    let artifact = identity_public_fixture_v1();
    let wire = &artifact.wire;
    let outer_bindings = outer_bindings_v1(92);
    let baseline = finalize_verified_kernel_v1(artifact.equation_verified_v1(), outer_bindings)
        .expect("cached baseline combined binding");
    assert_eq!(baseline.residual_digest, artifact.residual_digest);
    assert_eq!(baseline.binding_digest, artifact.binding_digest);

    let mismatch_log = Rc::new(RefCell::new(Vec::new()));
    let mismatch_touches = Rc::new(Cell::new(0));
    let mismatch = verify_rns_native_source_packing_same_opening_v1(
        FixtureCombinedPredecessorV1::new_v1(
            wire,
            RnsNativeSourcePackingSafeCoreV1 {
                terminal_predecessor_context_binding_digest: digest_v1(99),
                ..context.safe_core
            },
            outer_bindings,
        )
        .with_access_log_v1(Rc::clone(&mismatch_log)),
        context,
        FixtureReplaySourceV1::success_v1(FixtureModeV1::IdentityQ)
            .with_touch_count_v1(Rc::clone(&mismatch_touches)),
    );
    assert!(matches!(
        mismatch,
        Err(RnsNativeSourcePackingSameOpeningErrorV1::InvalidContext)
    ));
    assert_eq!(&*mismatch_log.borrow(), &[1, 2]);
    assert_eq!(mismatch_touches.get(), 0);

    let outer_mutations = [
        (
            "statement anchor",
            RnsNativeSourcePackingCombinedOuterBindingsV1 {
                source_statement_anchor_digest: digest_v1(150),
                ..outer_bindings
            },
        ),
        (
            "final aggregation schedule",
            RnsNativeSourcePackingCombinedOuterBindingsV1 {
                source_final_aggregation_schedule_digest: digest_v1(151),
                ..outer_bindings
            },
        ),
        (
            "packing",
            RnsNativeSourcePackingCombinedOuterBindingsV1 {
                enclosing_packing_binding_digest: digest_v1(152),
                ..outer_bindings
            },
        ),
        (
            "inventory prior context",
            RnsNativeSourcePackingCombinedOuterBindingsV1 {
                inventory_prior_context_digest: digest_v1(153),
                ..outer_bindings
            },
        ),
        (
            "inventory root",
            RnsNativeSourcePackingCombinedOuterBindingsV1 {
                inventory_root: digest_v1(154),
                ..outer_bindings
            },
        ),
        (
            "inventory continuation",
            RnsNativeSourcePackingCombinedOuterBindingsV1 {
                inventory_continuation_digest: digest_v1(155),
                ..outer_bindings
            },
        ),
        (
            "inventory binding",
            RnsNativeSourcePackingCombinedOuterBindingsV1 {
                inventory_binding_digest: digest_v1(156),
                ..outer_bindings
            },
        ),
        (
            "direct binding",
            RnsNativeSourcePackingCombinedOuterBindingsV1 {
                direct_binding_digest: digest_v1(157),
                ..outer_bindings
            },
        ),
        (
            "comparator binding",
            RnsNativeSourcePackingCombinedOuterBindingsV1 {
                comparator_binding_digest: digest_v1(158),
                ..outer_bindings
            },
        ),
        (
            "range binding",
            RnsNativeSourcePackingCombinedOuterBindingsV1 {
                comparator_range_carry_binding_digest: digest_v1(159),
                ..outer_bindings
            },
        ),
        (
            "small sign binding",
            RnsNativeSourcePackingCombinedOuterBindingsV1 {
                small_sign_disjointness_binding_digest: digest_v1(160),
                ..outer_bindings
            },
        ),
        (
            "q mask binding",
            RnsNativeSourcePackingCombinedOuterBindingsV1 {
                q_mask_linear_relations_binding_digest: digest_v1(161),
                ..outer_bindings
            },
        ),
        (
            "existing radix binding",
            RnsNativeSourcePackingCombinedOuterBindingsV1 {
                existing_radix_binding_digest: digest_v1(162),
                ..outer_bindings
            },
        ),
        (
            "radix complement binding",
            RnsNativeSourcePackingCombinedOuterBindingsV1 {
                radix_complement_binding_digest: digest_v1(163),
                ..outer_bindings
            },
        ),
        (
            "centering binding",
            RnsNativeSourcePackingCombinedOuterBindingsV1 {
                centering_subtraction_binding_digest: digest_v1(164),
                ..outer_bindings
            },
        ),
        (
            "lookup pre z binding",
            RnsNativeSourcePackingCombinedOuterBindingsV1 {
                global_lookup_pre_z_binding_digest: digest_v1(165),
                ..outer_bindings
            },
        ),
        (
            "lookup post z binding",
            RnsNativeSourcePackingCombinedOuterBindingsV1 {
                global_lookup_post_z_binding_digest: digest_v1(166),
                ..outer_bindings
            },
        ),
        (
            "inverse binding",
            RnsNativeSourcePackingCombinedOuterBindingsV1 {
                global_inverse_product_binding_digest: digest_v1(167),
                ..outer_bindings
            },
        ),
        (
            "membership binding",
            RnsNativeSourcePackingCombinedOuterBindingsV1 {
                global_membership_binding_digest: digest_v1(168),
                ..outer_bindings
            },
        ),
    ];
    for (label, mut changed_outer_bindings) in outer_mutations {
        changed_outer_bindings.combined_outer_binding_digest =
            changed_outer_bindings.canonical_combined_outer_binding_digest_v1();
        let changed =
            finalize_verified_kernel_v1(artifact.equation_verified_v1(), changed_outer_bindings)
                .expect("changed combined binding");
        assert_eq!(artifact.residual_v1(), changed.residual, "{label} residual");
        assert_eq!(
            artifact.point_root, changed.point_root,
            "{label} point root"
        );
        assert_eq!(
            artifact.pre_challenge_binding_digest, changed.pre_challenge_binding_digest,
            "{label} pre-challenge"
        );
        assert_eq!(artifact.tau_digest, changed.tau_digest, "{label} tau");
        assert_eq!(artifact.proof_digest, changed.proof_digest, "{label} proof");
        assert_ne!(
            artifact.residual_digest, changed.residual_digest,
            "{label} post-equation residual binding"
        );
        assert_ne!(
            artifact.binding_digest, changed.binding_digest,
            "{label} final binding"
        );
    }
    assert!(matches!(
        finalize_verified_kernel_v1(
            artifact.equation_verified_v1(),
            RnsNativeSourcePackingCombinedOuterBindingsV1 {
                combined_outer_binding_digest: digest_v1(169),
                ..outer_bindings
            },
        ),
        Err(RnsNativeSourcePackingSameOpeningErrorV1::InvalidContext)
    ));

    let zero_outer_log = Rc::new(RefCell::new(Vec::new()));
    let zero_outer_touches = Rc::new(Cell::new(0));
    let zero_outer = verify_rns_native_source_packing_same_opening_v1(
        combined_predecessor_v1(
            wire,
            context,
            RnsNativeSourcePackingCombinedOuterBindingsV1 {
                combined_outer_binding_digest: [0; DIGEST_BYTES_V1],
                ..outer_bindings
            },
        )
        .with_access_log_v1(Rc::clone(&zero_outer_log)),
        context,
        FixtureReplaySourceV1::success_v1(FixtureModeV1::IdentityQ)
            .with_touch_count_v1(Rc::clone(&zero_outer_touches)),
    );
    assert!(matches!(
        zero_outer,
        Err(RnsNativeSourcePackingSameOpeningErrorV1::InvalidContext)
    ));
    assert_eq!(&*zero_outer_log.borrow(), &[1, 2, 3]);
    assert!(zero_outer_touches.get() > 0);
}

#[test]
fn every_typed_safe_core_axis_changes_prechallenge_and_invalidates_the_old_proof() {
    let context = context_v1();
    let artifact = nonidentity_public_fixture_v1();
    let wire = &artifact.wire;
    let point_root = artifact.point_root;
    let manifest = manifest_digest_v1();
    let source_context = source_context_digest_v1(context).expect("source context");
    let replay_schedule =
        canonical_replay_schedule_digest_v1(context).expect("canonical replay schedule");
    let baseline_prechallenge =
        pre_challenge_binding_digest_v1(manifest, source_context, point_root, context.safe_core)
            .expect("baseline pre-challenge");
    let baseline_tau = derive_tau_v1(baseline_prechallenge).expect("baseline tau");
    let safe_core = context.safe_core;
    let mutations = [
        RnsNativeSourcePackingSafeCoreV1 {
            terminal_predecessor_context_binding_digest: digest_v1(21),
            ..safe_core
        },
        RnsNativeSourcePackingSafeCoreV1 {
            candidate_pre_direct_inventory_context_digest: digest_v1(22),
            ..safe_core
        },
        RnsNativeSourcePackingSafeCoreV1 {
            candidate_pre_direct_inventory_root: digest_v1(23),
            ..safe_core
        },
        RnsNativeSourcePackingSafeCoreV1 {
            existing_radix_candidate_root: digest_v1(24),
            ..safe_core
        },
        RnsNativeSourcePackingSafeCoreV1 {
            direct_core_safe_digest: digest_v1(25),
            ..safe_core
        },
    ];
    let proof_rejection_context = RnsNativeSourcePackingSameOpeningContextV1 {
        safe_core: mutations[0],
        ..context
    };
    for mutated_safe_core in mutations {
        let mutated_context = RnsNativeSourcePackingSameOpeningContextV1 {
            safe_core: mutated_safe_core,
            ..context
        };
        assert_eq!(
            source_context_digest_v1(mutated_context).expect("source-only context"),
            source_context
        );
        assert_eq!(
            canonical_replay_schedule_digest_v1(mutated_context)
                .expect("source-only canonical replay schedule"),
            replay_schedule
        );
        let mutated_prechallenge = pre_challenge_binding_digest_v1(
            manifest,
            source_context,
            point_root,
            mutated_safe_core,
        )
        .expect("mutated pre-challenge");
        assert_ne!(mutated_prechallenge, baseline_prechallenge);
        assert_ne!(
            derive_tau_v1(mutated_prechallenge).expect("mutated tau"),
            baseline_tau
        );
    }
    assert!(matches!(
        verify_rns_native_source_packing_same_opening_v1(
            combined_predecessor_v1(wire, proof_rejection_context, outer_bindings_v1(92)),
            proof_rejection_context,
            FixtureReplaySourceV1::success_v1(FixtureModeV1::NonIdentitySignedQ),
        ),
        Err(RnsNativeSourcePackingSameOpeningErrorV1::InvalidProof)
    ));
}

#[test]
fn every_mutable_canonical_safe_source_axis_changes_schedule_context_tau_and_old_proof() {
    let context = context_v1();
    let artifact = nonidentity_public_fixture_v1();
    let wire = &artifact.wire;
    let point_root = artifact.point_root;
    let manifest = manifest_digest_v1();
    let baseline_schedule =
        canonical_replay_schedule_digest_v1(context).expect("baseline canonical schedule");
    let baseline_source_context = source_context_digest_v1(context).expect("baseline context");
    let baseline_prechallenge = pre_challenge_binding_digest_v1(
        manifest,
        baseline_source_context,
        point_root,
        context.safe_core,
    )
    .expect("baseline pre-challenge");
    let baseline_tau = derive_tau_v1(baseline_prechallenge).expect("baseline tau");

    let mutations = [
        (
            "source binding",
            with_canonical_receipt_v1(RnsNativeSourcePackingSameOpeningContextV1 {
                source_binding_digest: digest_v1(31),
                ..context
            }),
        ),
        (
            "main snapshot",
            with_canonical_receipt_v1(RnsNativeSourcePackingSameOpeningContextV1 {
                main_snapshot_digest: digest_v1(32),
                ..context
            }),
        ),
        (
            "nonce snapshot",
            with_canonical_receipt_v1(RnsNativeSourcePackingSameOpeningContextV1 {
                nonce_snapshot_digest: digest_v1(33),
                ..context
            }),
        ),
        (
            "source formula",
            RnsNativeSourcePackingSameOpeningContextV1 {
                source_formula_digest: digest_v1(34),
                ..context
            },
        ),
        (
            "source mapping",
            RnsNativeSourcePackingSameOpeningContextV1 {
                source_mapping_digest: digest_v1(35),
                ..context
            },
        ),
    ];
    let proof_rejection_context = mutations[0].1;
    for (label, mutated_context) in mutations {
        mutated_context.validate_v1().expect(label);
        let mutated_schedule = canonical_replay_schedule_digest_v1(mutated_context)
            .expect("mutated canonical schedule");
        assert_ne!(mutated_schedule, baseline_schedule, "{label} schedule");
        let mutated_source_context =
            source_context_digest_v1(mutated_context).expect("mutated source context");
        assert_ne!(
            mutated_source_context, baseline_source_context,
            "{label} source context"
        );
        let mutated_prechallenge = pre_challenge_binding_digest_v1(
            manifest,
            mutated_source_context,
            point_root,
            mutated_context.safe_core,
        )
        .expect("mutated pre-challenge");
        assert_ne!(
            mutated_prechallenge, baseline_prechallenge,
            "{label} pre-challenge"
        );
        assert_ne!(
            derive_tau_v1(mutated_prechallenge).expect("mutated tau"),
            baseline_tau,
            "{label} tau"
        );
    }
    assert!(matches!(
        verify_rns_native_source_packing_same_opening_v1(
            combined_predecessor_v1(wire, proof_rejection_context, outer_bindings_v1(92)),
            proof_rejection_context,
            FixtureReplaySourceV1::success_for_context_v1(
                FixtureModeV1::NonIdentitySignedQ,
                proof_rejection_context,
            ),
        ),
        Err(RnsNativeSourcePackingSameOpeningErrorV1::InvalidProof)
    ));

    let invalid_profile = with_canonical_receipt_v1(RnsNativeSourcePackingSameOpeningContextV1 {
        profile_manifest_digest: digest_v1(36),
        ..context
    });
    assert!(matches!(
        invalid_profile.validate_v1(),
        Err(RnsNativeSourcePackingSameOpeningErrorV1::InvalidContext)
    ));
    let invalid_receipt = RnsNativeSourcePackingSameOpeningContextV1 {
        source_receipt_digest: digest_v1(37),
        ..context
    };
    assert!(matches!(
        invalid_receipt.validate_v1(),
        Err(RnsNativeSourcePackingSameOpeningErrorV1::InvalidContext)
    ));
}

#[test]
fn malformed_or_over_cap_successor_rejects_before_source_touch_or_outer_binding() {
    let context = context_v1();
    for malformed in [
        vec![0_u8; 1],
        vec![0_u8; FUTURE_DIRECT_MEMBERSHIP_PARENT_CAP_BYTES_V1 + 1],
    ] {
        let access_log = Rc::new(RefCell::new(Vec::new()));
        let touch_count = Rc::new(Cell::new(0));
        let result = verify_rns_native_source_packing_same_opening_v1(
            combined_predecessor_v1(&malformed, context, outer_bindings_v1(94))
                .with_access_log_v1(Rc::clone(&access_log)),
            context,
            FixtureReplaySourceV1::success_v1(FixtureModeV1::IdentityQ)
                .with_touch_count_v1(Rc::clone(&touch_count)),
        );
        assert!(matches!(
            result,
            Err(RnsNativeSourcePackingSameOpeningErrorV1::InvalidHeader)
                | Err(RnsNativeSourcePackingSameOpeningErrorV1::ProofCapExceeded)
        ));
        assert_eq!(touch_count.get(), 0);
        assert_eq!(&*access_log.borrow(), &[1, 2]);
    }
}

#[test]
fn nonidentity_fixture_reconstructs_d_and_aggregates_masks_in_exact_order() {
    let context = context_v1();
    let commitments = collect_commitments_v1(&FixtureReplaySourceV1::success_v1(
        FixtureModeV1::NonIdentityQ,
    ))
    .expect("nonidentity point set");
    assert_eq!(
        commitments.owners[0],
        owner_value_and_mask_commitment_v1(FixtureModeV1::NonIdentityQ, 0)
    );
    assert_eq!(
        commitments.owners[343],
        owner_value_and_mask_commitment_v1(FixtureModeV1::NonIdentityQ, 343)
    );
    assert_eq!(
        commitments.owners[344],
        owner_value_and_mask_commitment_v1(FixtureModeV1::NonIdentityQ, 344)
    );
    let pre_challenge = pre_challenge_binding_digest_v1(
        manifest_digest_v1(),
        source_context_digest_v1(context).expect("source context"),
        commitments.point_root,
        context.safe_core,
    )
    .expect("pre-challenge binding");
    let tau = derive_tau_v1(pre_challenge).expect("tau");
    let aggregate = aggregate_masks_v1(
        tau,
        commitments.point_root,
        canonical_replay_schedule_digest_v1(context).expect("canonical replay schedule"),
        FixtureMaskSourceV1::new_v1(
            FixtureModeV1::NonIdentityQ,
            MaskActionV1::Success,
            commitments.point_root,
            None,
        ),
    )
    .expect("sequential derived masks");
    let mut expected = Scalar::zero();
    let mut power = Scalar::one();
    for ordinal in 0..OWNERS_V1 {
        expected += power * owner_mask_v1(FixtureModeV1::NonIdentityQ, ordinal);
        power *= tau;
    }
    assert_eq!(aggregate.get(), expected);
}

#[test]
fn nonidentity_q_full_proof_accepts_and_wrong_a_or_z_fail_closed() {
    let context = context_v1();
    let artifact = nonidentity_public_fixture_v1();
    let wire = &artifact.wire;
    assert_eq!(artifact.residual_v1(), &[0x31, 0x41, 0x59]);
    assert_ne!(artifact.binding_digest, [0; DIGEST_BYTES_V1]);

    let decoded = FrameViewV1::decode_v1(wire, FUTURE_DIRECT_MEMBERSHIP_PARENT_CAP_BYTES_V1)
        .expect("valid frame");
    let h = ZkAmsT256BulletproofSuiteV1::generators().h;
    let mut changed_a = decoded.a + h;
    if changed_a.is_identity() {
        changed_a += h;
    }
    let mut wrong_a = artifact.wire.clone();
    wrong_a[HEADER_BYTES_V1..HEADER_BYTES_V1 + POINT_BYTES_V1]
        .copy_from_slice(&non_identity_point_bytes_v1(&changed_a).expect("changed A"));
    rewrite_codec_v1(&mut wrong_a);
    assert_fixture_equation_rejects_v1(artifact, FixtureModeV1::NonIdentitySignedQ, &wrong_a);

    let mut wrong_z = artifact.wire.clone();
    let changed_z = decoded.z + Scalar::one();
    wrong_z[HEADER_BYTES_V1 + POINT_BYTES_V1..HEADER_BYTES_V1 + SCHNORR_PAYLOAD_BYTES_V1]
        .copy_from_slice(&changed_z.to_le_bytes());
    rewrite_codec_v1(&mut wrong_z);
    let wrong_z_access_log = Rc::new(RefCell::new(Vec::new()));
    assert!(matches!(
        verify_rns_native_source_packing_same_opening_v1(
            combined_predecessor_v1(&wrong_z, context, outer_bindings_v1(91))
                .with_access_log_v1(Rc::clone(&wrong_z_access_log)),
            context,
            FixtureReplaySourceV1::success_v1(FixtureModeV1::NonIdentitySignedQ),
        ),
        Err(RnsNativeSourcePackingSameOpeningErrorV1::InvalidProof)
    ));
    assert_eq!(&*wrong_z_access_log.borrow(), &[1, 2]);
}

#[test]
fn nonidentity_signed_owner_cannot_be_moved_to_the_next_signed_slot() {
    let context = context_v1();
    let artifact = nonidentity_public_fixture_v1();
    assert!(matches!(
        verify_rns_native_source_packing_same_opening_v1(
            combined_predecessor_v1(&artifact.wire, context, outer_bindings_v1(91)),
            context,
            FixtureReplaySourceV1::success_v1(FixtureModeV1::NonIdentitySignedQAtWrongOwner,),
        ),
        Err(RnsNativeSourcePackingSameOpeningErrorV1::InvalidProof)
    ));
}

#[test]
fn frame_is_exact_bounded_and_rejects_identity_a() {
    let h = ZkAmsT256BulletproofSuiteV1::generators().h;
    let minimum = encode_frame_v1(&h, &Scalar::one(), &[1]).expect("minimum frame");
    assert_eq!(minimum.len(), MIN_WIRE_BYTES_V1);
    let decoded = FrameViewV1::decode_v1(&minimum, FUTURE_DIRECT_MEMBERSHIP_PARENT_CAP_BYTES_V1)
        .expect("minimum decode");
    assert_eq!(decoded.residual, &[1]);
    assert!(!decoded.a.is_identity());
    assert!(matches!(
        FrameViewV1::decode_v1(&minimum, minimum.len() - 1),
        Err(RnsNativeSourcePackingSameOpeningErrorV1::ProofCapExceeded)
    ));
    assert!(matches!(
        FrameViewV1::decode_v1(&minimum, FUTURE_DIRECT_MEMBERSHIP_PARENT_CAP_BYTES_V1 + 1,),
        Err(RnsNativeSourcePackingSameOpeningErrorV1::ProofCapExceeded)
    ));

    for end in 0..minimum.len() {
        assert!(
            FrameViewV1::decode_v1(
                &minimum[..end],
                FUTURE_DIRECT_MEMBERSHIP_PARENT_CAP_BYTES_V1,
            )
            .is_err()
        );
    }
    let mut trailing = minimum.clone();
    trailing.push(0);
    assert!(
        FrameViewV1::decode_v1(&trailing, FUTURE_DIRECT_MEMBERSHIP_PARENT_CAP_BYTES_V1,).is_err()
    );

    for header_offset in [0, 4, 5, 6, 8, 12, 14, 16, 18, 20, 21, 22, 23, 24] {
        let mut changed_header = minimum.clone();
        changed_header[header_offset] ^= 1;
        rewrite_codec_v1(&mut changed_header);
        assert!(matches!(
            FrameViewV1::decode_v1(
                &changed_header,
                FUTURE_DIRECT_MEMBERSHIP_PARENT_CAP_BYTES_V1,
            ),
            Err(RnsNativeSourcePackingSameOpeningErrorV1::InvalidHeader)
        ));
    }

    let mut changed_codec = minimum.clone();
    *changed_codec.last_mut().expect("codec byte") ^= 1;
    assert!(matches!(
        FrameViewV1::decode_v1(&changed_codec, FUTURE_DIRECT_MEMBERSHIP_PARENT_CAP_BYTES_V1,),
        Err(RnsNativeSourcePackingSameOpeningErrorV1::InvalidIntegrity)
    ));

    let mut identity_a = minimum.clone();
    identity_a[HEADER_BYTES_V1..HEADER_BYTES_V1 + POINT_BYTES_V1].fill(0);
    identity_a[HEADER_BYTES_V1] = 0x40;
    rewrite_codec_v1(&mut identity_a);
    assert!(matches!(
        FrameViewV1::decode_v1(&identity_a, FUTURE_DIRECT_MEMBERSHIP_PARENT_CAP_BYTES_V1,),
        Err(RnsNativeSourcePackingSameOpeningErrorV1::InvalidPoint)
    ));

    let mut noncanonical_a = minimum.clone();
    noncanonical_a[HEADER_BYTES_V1..HEADER_BYTES_V1 + POINT_BYTES_V1].fill(0xff);
    rewrite_codec_v1(&mut noncanonical_a);
    assert!(matches!(
        FrameViewV1::decode_v1(
            &noncanonical_a,
            FUTURE_DIRECT_MEMBERSHIP_PARENT_CAP_BYTES_V1,
        ),
        Err(RnsNativeSourcePackingSameOpeningErrorV1::InvalidPoint)
    ));

    let mut invalid_scalar = minimum.clone();
    invalid_scalar[HEADER_BYTES_V1 + POINT_BYTES_V1..HEADER_BYTES_V1 + SCHNORR_PAYLOAD_BYTES_V1]
        .fill(0xff);
    rewrite_codec_v1(&mut invalid_scalar);
    assert!(matches!(
        FrameViewV1::decode_v1(
            &invalid_scalar,
            FUTURE_DIRECT_MEMBERSHIP_PARENT_CAP_BYTES_V1,
        ),
        Err(RnsNativeSourcePackingSameOpeningErrorV1::InvalidScalar)
    ));

    let maximum_residual =
        vec![7_u8; RNS_NATIVE_SOURCE_PACKING_SAME_OPENING_SUCCESSOR_MAX_BYTES_V1];
    let maximum = encode_frame_v1(&h, &Scalar::one(), &maximum_residual).expect("maximum frame");
    assert_eq!(maximum.len(), FUTURE_DIRECT_MEMBERSHIP_PARENT_CAP_BYTES_V1);
    FrameViewV1::decode_v1(&maximum, FUTURE_DIRECT_MEMBERSHIP_PARENT_CAP_BYTES_V1)
        .expect("maximum decode");
    let excessive = vec![9_u8; RNS_NATIVE_SOURCE_PACKING_SAME_OPENING_SUCCESSOR_MAX_BYTES_V1 + 1];
    assert!(encode_frame_v1(&h, &Scalar::one(), &excessive).is_err());
}

#[test]
fn tau_and_c_are_bounded_nonzero_and_q_encoding_is_identity_aware() {
    let binding = digest_v1(70);
    let tau = derive_tau_v1(binding).expect("tau");
    assert!(!tau.is_zero());
    let q = Point::identity();
    let a = ZkAmsT256BulletproofSuiteV1::generators().h;
    let challenge = derive_schnorr_challenge_v1(binding, tau, &q, &a).expect("challenge");
    assert!(!challenge.is_zero());
    let identity = identity_aware_point_bytes_v1(&q).expect("identity encoding");
    assert_eq!(identity[0], 0);
    assert!(identity[1..].iter().all(|byte| *byte == 0));
    let nonidentity = identity_aware_point_bytes_v1(&a).expect("point encoding");
    assert_eq!(nonidentity[0], 1);
    assert!(derive_schnorr_challenge_v1(binding, tau, &q, &Point::identity()).is_err());
}

#[test]
fn replay_destination_clears_after_error_and_unwind() {
    let before = crate::vega::bulletproof_t256::zeroizing_t256_scalar_vec_drop_count_v1();
    let error_drop = Rc::new(Cell::new(false));
    let result = invoke_replay_once_v1(FixtureReplaySourceV1::new_v1(
        FixtureModeV1::IdentityQ,
        ReplayActionV1::Error,
        Some(Rc::clone(&error_drop)),
    ));
    assert!(matches!(
        result,
        Err(RnsNativeSourcePackingSameOpeningErrorV1::SourceUnavailable)
    ));
    assert!(error_drop.get());
    let after_error = crate::vega::bulletproof_t256::zeroizing_t256_scalar_vec_drop_count_v1();
    assert!(after_error > before);

    let panic_drop = Rc::new(Cell::new(false));
    let panic_result = catch_unwind(AssertUnwindSafe(|| {
        let _ = invoke_replay_once_v1(FixtureReplaySourceV1::new_v1(
            FixtureModeV1::IdentityQ,
            ReplayActionV1::Panic,
            Some(Rc::clone(&panic_drop)),
        ));
    }));
    assert!(panic_result.is_err());
    assert!(panic_drop.get());
    let after_panic = crate::vega::bulletproof_t256::zeroizing_t256_scalar_vec_drop_count_v1();
    assert!(after_panic > after_error);
}

#[test]
fn source_point_and_finish_failures_are_one_shot_and_fail_closed() {
    let point_drop = Rc::new(Cell::new(false));
    let point_error = prepare_relation_v1(
        context_v1(),
        FixtureReplaySourceV1::new_v1(
            FixtureModeV1::IdentityQ,
            ReplayActionV1::PointError,
            Some(Rc::clone(&point_drop)),
        ),
    );
    assert!(matches!(
        point_error,
        Err(RnsNativeSourcePackingSameOpeningErrorV1::SourceUnavailable)
    ));
    assert!(point_drop.get());

    let before = crate::vega::bulletproof_t256::zeroizing_t256_scalar_vec_drop_count_v1();
    let finish_drop = Rc::new(Cell::new(false));
    let finish_error = prepare_relation_v1(
        context_v1(),
        FixtureReplaySourceV1::new_v1(
            FixtureModeV1::IdentityQ,
            ReplayActionV1::FinishError,
            Some(Rc::clone(&finish_drop)),
        ),
    );
    assert!(matches!(
        finish_error,
        Err(RnsNativeSourcePackingSameOpeningErrorV1::SourceUnavailable)
    ));
    assert!(finish_drop.get());
    assert!(crate::vega::bulletproof_t256::zeroizing_t256_scalar_vec_drop_count_v1() > before);
}

#[test]
fn typed_preflight_and_source_or_mask_receipt_mismatches_fail_before_reuse() {
    let context = context_v1();
    let preflight_touches = Rc::new(Cell::new(0));
    let preflight_drop = Rc::new(Cell::new(false));
    let invalid_context = RnsNativeSourcePackingSameOpeningContextV1 {
        safe_core: RnsNativeSourcePackingSafeCoreV1 {
            terminal_predecessor_context_binding_digest: [0; DIGEST_BYTES_V1],
            ..context.safe_core
        },
        ..context
    };
    assert!(matches!(
        prepare_relation_v1(
            invalid_context,
            FixtureReplaySourceV1::new_v1(
                FixtureModeV1::IdentityQ,
                ReplayActionV1::Success,
                Some(Rc::clone(&preflight_drop)),
            )
            .with_touch_count_v1(Rc::clone(&preflight_touches)),
        ),
        Err(RnsNativeSourcePackingSameOpeningErrorV1::InvalidContext)
    ));
    assert_eq!(preflight_touches.get(), 0);
    assert!(preflight_drop.get());

    let schedule_drop = Rc::new(Cell::new(false));
    let schedule_touches = Rc::new(Cell::new(0));
    let mut wrong_schedule_source = FixtureReplaySourceV1::new_v1(
        FixtureModeV1::IdentityQ,
        ReplayActionV1::Success,
        Some(Rc::clone(&schedule_drop)),
    )
    .with_touch_count_v1(Rc::clone(&schedule_touches));
    wrong_schedule_source.schedule_digest = digest_v1(38);
    assert!(matches!(
        prepare_relation_v1(context, wrong_schedule_source),
        Err(RnsNativeSourcePackingSameOpeningErrorV1::InvalidContext)
    ));
    assert_eq!(schedule_touches.get(), 2);
    assert!(schedule_drop.get());

    let source_axes_drop = Rc::new(Cell::new(false));
    let source_axes_touches = Rc::new(Cell::new(0));
    let mut wrong_source_axes = FixtureReplaySourceV1::new_v1(
        FixtureModeV1::IdentityQ,
        ReplayActionV1::Success,
        Some(Rc::clone(&source_axes_drop)),
    )
    .with_touch_count_v1(Rc::clone(&source_axes_touches));
    wrong_source_axes
        .authenticated_source_axes
        .source_formula_digest = digest_v1(40);
    assert!(matches!(
        prepare_relation_v1(context, wrong_source_axes),
        Err(RnsNativeSourcePackingSameOpeningErrorV1::InvalidContext)
    ));
    // One typed-axis read is the preflight itself; no schedule, commitment, or
    // replay accessor is reached after the mismatch.
    assert_eq!(source_axes_touches.get(), 1);
    assert!(source_axes_drop.get());

    let replay_drop = Rc::new(Cell::new(false));
    assert!(matches!(
        prepare_relation_v1(
            context,
            FixtureReplaySourceV1::new_v1(
                FixtureModeV1::IdentityQ,
                ReplayActionV1::WrongReceipt,
                Some(Rc::clone(&replay_drop)),
            ),
        ),
        Err(RnsNativeSourcePackingSameOpeningErrorV1::InvalidContext)
    ));
    assert!(replay_drop.get());

    let point_root = point_root_v1(FixtureModeV1::IdentityQ);
    let preflight_takes = Rc::new(Cell::new(0));
    let mask_preflight_drop = Rc::new(Cell::new(false));
    assert!(matches!(
        aggregate_masks_v1(
            Scalar::from_u64(3),
            point_root,
            canonical_replay_schedule_digest_v1(context).expect("canonical replay schedule"),
            FixtureMaskSourceV1::new_v1(
                FixtureModeV1::IdentityQ,
                MaskActionV1::Success,
                [0; DIGEST_BYTES_V1],
                Some(Rc::clone(&mask_preflight_drop)),
            )
            .with_take_count_v1(Rc::clone(&preflight_takes)),
        ),
        Err(RnsNativeSourcePackingSameOpeningErrorV1::InvalidContext)
    ));
    assert_eq!(preflight_takes.get(), 0);
    assert!(mask_preflight_drop.get());

    let schedule_takes = Rc::new(Cell::new(0));
    let mask_schedule_drop = Rc::new(Cell::new(false));
    let mut wrong_mask_schedule = FixtureMaskSourceV1::new_v1(
        FixtureModeV1::IdentityQ,
        MaskActionV1::Success,
        point_root,
        Some(Rc::clone(&mask_schedule_drop)),
    )
    .with_take_count_v1(Rc::clone(&schedule_takes));
    wrong_mask_schedule.schedule_digest = digest_v1(39);
    assert!(matches!(
        aggregate_masks_v1(
            Scalar::from_u64(4),
            point_root,
            canonical_replay_schedule_digest_v1(context).expect("canonical replay schedule"),
            wrong_mask_schedule,
        ),
        Err(RnsNativeSourcePackingSameOpeningErrorV1::InvalidContext)
    ));
    assert_eq!(schedule_takes.get(), 0);
    assert!(mask_schedule_drop.get());

    let receipt_drop = Rc::new(Cell::new(false));
    assert!(matches!(
        aggregate_masks_v1(
            Scalar::from_u64(5),
            point_root,
            canonical_replay_schedule_digest_v1(context).expect("canonical replay schedule"),
            FixtureMaskSourceV1::new_v1(
                FixtureModeV1::IdentityQ,
                MaskActionV1::WrongReceipt,
                point_root,
                Some(Rc::clone(&receipt_drop)),
            ),
        ),
        Err(RnsNativeSourcePackingSameOpeningErrorV1::InvalidContext)
    ));
    assert!(receipt_drop.get());
}

#[test]
fn mask_destination_and_provider_clear_after_error_and_unwind() {
    let point_root = digest_v1(71);
    let schedule =
        canonical_replay_schedule_digest_v1(context_v1()).expect("canonical replay schedule");
    let before = zeroizing_mask_slot_drop_count_v1();
    let error_drop = Rc::new(Cell::new(false));
    let result = aggregate_masks_v1(
        Scalar::from_u64(3),
        point_root,
        schedule,
        FixtureMaskSourceV1::new_v1(
            FixtureModeV1::NonIdentityQ,
            MaskActionV1::ErrorFirst,
            point_root,
            Some(Rc::clone(&error_drop)),
        ),
    );
    assert!(matches!(
        result,
        Err(RnsNativeSourcePackingSameOpeningErrorV1::MaskUnavailable)
    ));
    assert!(error_drop.get());
    let after_error = zeroizing_mask_slot_drop_count_v1();
    assert!(after_error > before);

    let panic_drop = Rc::new(Cell::new(false));
    let panic_result = catch_unwind(AssertUnwindSafe(|| {
        let _ = aggregate_masks_v1(
            Scalar::from_u64(5),
            point_root,
            schedule,
            FixtureMaskSourceV1::new_v1(
                FixtureModeV1::NonIdentityQ,
                MaskActionV1::PanicFirst,
                point_root,
                Some(Rc::clone(&panic_drop)),
            ),
        );
    }));
    assert!(panic_result.is_err());
    assert!(panic_drop.get());
    assert!(zeroizing_mask_slot_drop_count_v1() > after_error);

    let finish_drop = Rc::new(Cell::new(false));
    let finish_error = aggregate_masks_v1(
        Scalar::from_u64(7),
        point_root,
        schedule,
        FixtureMaskSourceV1::new_v1(
            FixtureModeV1::NonIdentityQ,
            MaskActionV1::FinishError,
            point_root,
            Some(Rc::clone(&finish_drop)),
        ),
    );
    assert!(matches!(
        finish_error,
        Err(RnsNativeSourcePackingSameOpeningErrorV1::MaskUnavailable)
    ));
    assert!(finish_drop.get());
}

#[test]
fn source_and_later_mask_failures_do_not_touch_schnorr_rng() {
    let context = context_v1();
    let point_root = point_root_v1(FixtureModeV1::IdentityQ);

    let source_rng_touches = Rc::new(Cell::new(0));
    let mut source_rng = CountingUnavailableRngV1(Rc::clone(&source_rng_touches));
    let source_drop = Rc::new(Cell::new(false));
    assert!(matches!(
        prove_rns_native_source_packing_same_opening_kernel_v1(
            context,
            FixtureReplaySourceV1::new_v1(
                FixtureModeV1::IdentityQ,
                ReplayActionV1::Error,
                Some(Rc::clone(&source_drop)),
            ),
            FixtureMaskSourceV1::new_v1(
                FixtureModeV1::IdentityQ,
                MaskActionV1::Success,
                point_root,
                None,
            ),
            &[1],
            &mut source_rng,
        ),
        Err(RnsNativeSourcePackingSameOpeningErrorV1::SourceUnavailable)
    ));
    assert!(source_drop.get());
    assert_eq!(source_rng_touches.get(), 0);

    let mask_rng_touches = Rc::new(Cell::new(0));
    let mut mask_rng = CountingUnavailableRngV1(Rc::clone(&mask_rng_touches));
    let mask_drop = Rc::new(Cell::new(false));
    let take_count = Rc::new(Cell::new(0));
    let zeroizing_slots_before = zeroizing_mask_slot_drop_count_v1();
    assert!(matches!(
        prove_rns_native_source_packing_same_opening_kernel_v1(
            context,
            FixtureReplaySourceV1::success_v1(FixtureModeV1::IdentityQ),
            FixtureMaskSourceV1::new_v1(
                FixtureModeV1::IdentityQ,
                MaskActionV1::ErrorLast,
                point_root,
                Some(Rc::clone(&mask_drop)),
            )
            .with_take_count_v1(Rc::clone(&take_count)),
            &[1],
            &mut mask_rng,
        ),
        Err(RnsNativeSourcePackingSameOpeningErrorV1::MaskUnavailable)
    ));
    assert!(mask_drop.get());
    assert_eq!(take_count.get(), OWNERS_V1);
    assert!(zeroizing_mask_slot_drop_count_v1() > zeroizing_slots_before);
    assert_eq!(mask_rng_touches.get(), 0);
}

#[test]
fn entropy_failure_has_no_deterministic_fallback() {
    assert!(matches!(
        sample_nonzero_scalar_v1(&mut UnavailableRngV1),
        Err(RnsNativeSourcePackingSameOpeningErrorV1::RandomnessUnavailable)
    ));

    let point_root = point_root_v1(FixtureModeV1::IdentityQ);
    let replay_drop = Rc::new(Cell::new(false));
    let mask_drop = Rc::new(Cell::new(false));
    let mut rng = UnavailableRngV1;
    let result = prove_rns_native_source_packing_same_opening_kernel_v1(
        context_v1(),
        FixtureReplaySourceV1::new_v1(
            FixtureModeV1::IdentityQ,
            ReplayActionV1::Success,
            Some(Rc::clone(&replay_drop)),
        ),
        FixtureMaskSourceV1::new_v1(
            FixtureModeV1::IdentityQ,
            MaskActionV1::Success,
            point_root,
            Some(Rc::clone(&mask_drop)),
        ),
        &[1],
        &mut rng,
    );
    assert!(matches!(
        result,
        Err(RnsNativeSourcePackingSameOpeningErrorV1::RandomnessUnavailable)
    ));
    assert!(replay_drop.get());
    assert!(mask_drop.get());
}

#[test]
fn transcript_chronology_and_production_unavailability_are_explicit() {
    let source = include_str!("rns_native_source_packing_same_opening.rs");
    let test_source = include_str!("rns_native_source_packing_same_opening_tests.rs");
    for needle in [
        "const PRODUCTION_COMBINED_DIRECT_MEMBERSHIP_PREDECESSOR_AVAILABLE_V1: bool = false;",
        "RNS_NATIVE_SOURCE_PACKING_SAME_OPENING_SOURCE_SETTLED_V1: bool = true;",
        "const PRODUCTION_AUTHENTICATED_REPLAY_OWNER_AVAILABLE_V1: bool = false;",
        "const PRODUCTION_DERIVED_MASK_OWNER_AVAILABLE_V1: bool = false;",
        "const GLOBAL_MEMBERSHIP_CHILD_DECLARED_V1: bool = true;",
        "const COMPOSITE_ACCEPTANCE_AVAILABLE_V1: bool = false;",
        "const RELEASE_READY_V1: bool = false;",
        "combined_direct_membership_predecessor_adapter: Infallible",
        "authenticated_aggregate_replay_owner: Infallible",
        "derived_mask_owner: Infallible",
        "pub(super) trait RnsNativeSourcePackingCombinedDirectMembershipPredecessorV1<'proof>",
        "pub(super) struct RnsNativeSourcePackingSafeCoreV1",
        "terminal_predecessor_context_binding_digest",
        "candidate_pre_direct_inventory_context_digest",
        "candidate_pre_direct_inventory_root",
        "existing_radix_candidate_root",
        "direct_core_safe_digest",
        "pub(super) fn canonical_replay_schedule_digest_v1(",
        "CANONICAL_REPLAY_SCHEDULE_DOMAIN_V1",
        "canonical_source_receipt_digest_v1",
        "fn successor_independent_safe_core_v1(&self)",
        "pub(super) struct RnsNativeSourcePackingCombinedOuterBindingsV1",
        "source_statement_anchor_digest",
        "source_final_aggregation_schedule_digest",
        "enclosing_packing_binding_digest",
        "inventory_prior_context_digest",
        "inventory_root",
        "inventory_continuation_digest",
        "inventory_binding_digest",
        "direct_binding_digest",
        "comparator_binding_digest",
        "comparator_range_carry_binding_digest",
        "small_sign_disjointness_binding_digest",
        "q_mask_linear_relations_binding_digest",
        "existing_radix_binding_digest",
        "radix_complement_binding_digest",
        "centering_subtraction_binding_digest",
        "global_lookup_pre_z_binding_digest",
        "global_lookup_post_z_binding_digest",
        "global_inverse_product_binding_digest",
        "global_membership_binding_digest",
        "combined_outer_binding_digest",
        "COMBINED_OUTER_BINDING_DOMAIN_V1",
        "pub(super) fn canonical_combined_outer_binding_digest_v1",
        "fn combined_outer_bindings_v1(&self)",
        "pub(super) trait RnsNativeSourcePackingAggregateReplayV1: Sized",
        "RnsNativeSourcePackingAuthenticatedSourceAxesV1",
        "destination: &mut ZeroizingT256ScalarVecV1",
        "fn difference_source_index_v1",
        "fn signed_source_index_v1",
        "fn signed_scalar_from_twos_complement_be_i64_v1",
        "pub(super) trait RnsNativeSourcePackingDerivedMaskSourceV1: Sized",
        "source.take_next_mask_v1(owner_coordinate_v1(ordinal)?, mask.as_mut())?;",
        "for attempt in 0..MAX_CHALLENGE_ATTEMPTS_V1",
        "let q = identity_aware_point_bytes_v1(q)?;",
        "let a = non_identity_point_bytes_v1(a)?;",
        "FUTURE_DIRECT_MEMBERSHIP_PARENT_CAP_BYTES_V1: usize =",
        "RNS_NATIVE_GLOBAL_MEMBERSHIP_RESIDUAL_MAX_BYTES_V1",
        "RNS_NATIVE_SOURCE_PACKING_SAME_OPENING_SUCCESSOR_MAX_BYTES_V1 == 108_339",
        "direct-frame-already-charged-before-comparator-chain",
        "child-is-declared-source-settled-and-non-authorizing",
        "legacy-344-source-order-Csrc-masks-are-not-D-packing-masks",
    ] {
        assert!(source.contains(needle), "missing invariant: {needle}");
    }
    for removed in [
        "CURRENT_MEMBERSHIP_ONLY_RESIDUAL_MAX_BYTES_V1",
        "PLANNED_DIRECT_CASCADE_OWNED_BYTES_V1",
        "140735-32271",
        "source_replay_schedule_digest",
        "terminal_predecessor_binding_digest",
        "combined_direct_membership_core_transcript_digest",
        "enclosing_inventory_binding_digest",
        "enclosing_radix_binding_digest",
    ] {
        assert!(
            !source.contains(removed),
            "stale unsafe contract: {removed}"
        );
    }

    let prepare_path = source
        .split_once("fn prepare_relation_v1")
        .expect("prepare function")
        .1
        .split_once("fn aggregate_masks_v1")
        .expect("prepare boundary")
        .0;
    let canonical_schedule = prepare_path
        .find("canonical_replay_schedule_digest_v1(context)")
        .expect("canonical replay schedule");
    let source_preflight = prepare_path
        .find("source.authenticated_source_axes_v1()")
        .expect("source preflight");
    let points = prepare_path
        .find("collect_commitments_v1")
        .expect("actual point root");
    let binding = prepare_path
        .find("pre_challenge_binding_digest_v1")
        .expect("pre-challenge binding");
    let tau = prepare_path.find("derive_tau_v1").expect("tau");
    let replay = prepare_path
        .find("replay_tau_aggregate_v1")
        .expect("source replay");
    let q = prepare_path.find("let q =").expect("Q");
    assert!(
        canonical_schedule < source_preflight
            && source_preflight < points
            && points < binding
            && binding < tau
            && tau < replay
            && replay < q
    );

    let context_definition = source
        .split_once("pub(super) struct RnsNativeSourcePackingSameOpeningContextV1")
        .expect("context definition")
        .1
        .split_once("/// Canonical signed-source role order")
        .expect("context boundary")
        .0;
    let source_context_path = source
        .split_once("fn source_context_digest_v1")
        .expect("source context path")
        .1
        .split_once("fn non_identity_point_bytes_v1")
        .expect("source context boundary")
        .0;
    let manifest_path = source
        .split_once("fn manifest_digest_v1")
        .expect("manifest path")
        .1
        .split_once("fn source_context_digest_v1")
        .expect("manifest boundary")
        .0;
    let replay_schedule_path = source
        .split_once("fn canonical_replay_schedule_digest_v1(\n    context:")
        .expect("canonical replay schedule path")
        .1
        .split_once("fn source_context_digest_v1")
        .expect("canonical replay schedule boundary")
        .0;
    for required in [
        "context.profile_manifest_digest",
        "context.source_binding_digest",
        "context.main_snapshot_digest",
        "context.nonce_snapshot_digest",
        "context.source_receipt_digest",
        "context.source_formula_digest",
        "context.source_mapping_digest",
        "for ordinal in 0..OWNERS_V1",
        "owner_coordinate_v1(ordinal)?",
    ] {
        assert!(
            replay_schedule_path.contains(required),
            "canonical replay schedule omits {required}"
        );
    }
    for forbidden in [
        "safe_core",
        "point_root",
        "source_statement_anchor_digest",
        "source_final_aggregation_schedule_digest",
        "inventory_prior_context_digest",
        "inventory_binding_digest",
        "direct_binding_digest",
        "combined_outer_binding_digest",
    ] {
        assert!(
            !replay_schedule_path.contains(forbidden),
            "canonical replay schedule contains unsafe {forbidden}"
        );
    }

    let prover = source
        .split_once("pub(super) fn prove_rns_native_source_packing_same_opening_kernel_v1")
        .expect("prover")
        .1
        .split_once("struct VerifiedKernelV1")
        .expect("prover boundary")
        .0;
    let prepare_call = prover.find("prepare_relation_v1").expect("prepare");
    let masks = prover.find("aggregate_masks_v1").expect("masks");
    let entropy = prover.find("sample_nonzero_scalar_v1").expect("entropy");
    let challenge = prover
        .find("derive_schnorr_challenge_v1")
        .expect("Schnorr challenge");
    assert!(prepare_call < masks && masks < entropy && entropy < challenge);

    let pre_challenge = source
        .split_once("fn pre_challenge_binding_digest_v1")
        .expect("pre-challenge function")
        .1
        .split_once("fn derive_tau_v1")
        .expect("pre-challenge boundary")
        .0;
    let manifest = pre_challenge
        .find("hash.update(&manifest_digest)")
        .expect("manifest position");
    let source_context = pre_challenge
        .find("hash.update(&source_context_digest)")
        .expect("source context position");
    let point_root = pre_challenge
        .find("hash.update(&point_root)")
        .expect("point root position");
    let terminal = pre_challenge
        .find("hash.update(&safe_core.terminal_predecessor_context_binding_digest)")
        .expect("terminal predecessor position");
    let candidate_context = pre_challenge
        .find("hash.update(&safe_core.candidate_pre_direct_inventory_context_digest)")
        .expect("candidate context position");
    let candidate_root = pre_challenge
        .find("hash.update(&safe_core.candidate_pre_direct_inventory_root)")
        .expect("candidate root position");
    let radix_root = pre_challenge
        .find("hash.update(&safe_core.existing_radix_candidate_root)")
        .expect("radix root position");
    let direct_core = pre_challenge
        .find("hash.update(&safe_core.direct_core_safe_digest)")
        .expect("direct core position");
    assert!(
        manifest < source_context
            && source_context < point_root
            && point_root < terminal
            && terminal < candidate_context
            && candidate_context < candidate_root
            && candidate_root < radix_root
            && radix_root < direct_core
    );
    let forbidden_pre_tau = [
        "source_statement_anchor_digest",
        "source_final_aggregation_schedule_digest",
        "source_replay_schedule_digest",
        "inventory_prior_context_digest",
        "hash.update(&safe_core.inventory_root)",
        "packing_binding_digest",
        "inventory_binding_digest",
        "direct_binding_digest",
        "comparator_binding_digest",
        "membership_binding_digest",
        "radix_binding_digest",
        "combined_outer_binding_digest",
        "residual_digest",
        "codec_digest",
        "proof_digest",
        "same_opening_successor_v1",
        "downstream_residual",
        "wire",
        "FrameViewV1",
    ];
    let tau_path = source
        .split_once("fn derive_tau_v1")
        .expect("tau path")
        .1
        .split_once("fn derive_schnorr_challenge_v1")
        .expect("tau boundary")
        .0;
    for path in [
        context_definition,
        source_context_path,
        prepare_path,
        pre_challenge,
        tau_path,
    ] {
        for forbidden in forbidden_pre_tau {
            assert!(
                !path.contains(forbidden),
                "pre-tau path contains forbidden {forbidden}"
            );
        }
    }
    assert!(manifest_path.contains("TRANSCRIPT_LANGUAGE_V1"));
    assert!(source.contains("canonical-replay-schedule=H"));

    let schnorr = source
        .split_once("fn derive_schnorr_challenge_v1")
        .expect("challenge function")
        .1
        .split_once("fn scalar_digest_v1")
        .expect("challenge boundary")
        .0;
    let pre_challenge = schnorr
        .find("low.update(&pre_challenge_binding_digest)")
        .expect("pre-challenge in c");
    let tau = schnorr
        .find("low.update(&tau.to_le_bytes())")
        .expect("tau in c");
    let q = schnorr.find("low.update(&q)").expect("Q in c");
    let a = schnorr.find("low.update(&a)").expect("A in c");
    let attempt = schnorr
        .find("low.update(&[attempt])")
        .expect("attempt in c");
    assert!(pre_challenge < tau && tau < q && q < a && a < attempt);
    for excluded in [
        "z_bytes",
        "residual_digest",
        "codec_digest",
        "packing_binding_digest",
        "inventory_binding_digest",
        "direct_binding_digest",
        "comparator_binding_digest",
        "membership_binding_digest",
        "radix_binding_digest",
        "combined_outer_binding_digest",
        "final_binding_digest",
    ] {
        assert!(!schnorr.contains(excluded));
    }

    let fixture_point_cache_index = test_source
        .split_once("fn owner_mask_index_v1")
        .expect("public fixture point cache index")
        .1
        .split_once("fn fixture_public_points_v1")
        .expect("public fixture point cache index boundary")
        .0;
    for required in [
        "if mask == Scalar::zero()",
        "assert_eq!(\n            mask,\n            Scalar::one()",
        "fixture cache only admits the public mask values zero and one",
    ] {
        assert!(
            fixture_point_cache_index.contains(required),
            "public fixture point cache index omitted {required}"
        );
    }

    let detached_handoff = source
        .find("pub(super) fn verify_rns_native_source_packing_same_opening_v1")
        .expect("detached fixture handoff");
    assert!(
        source[detached_handoff.saturating_sub(80)..detached_handoff].contains("#[cfg(test)]"),
        "detached context/replay entry must remain test-only"
    );
    let handoff = source
        .split_once("pub(super) fn verify_rns_native_source_packing_same_opening_v1")
        .expect("combined handoff")
        .1
        .split_once("#[cfg(test)]")
        .expect("handoff boundary")
        .0;
    let successor = handoff
        .find("previous.same_opening_successor_v1()")
        .expect("combined successor");
    let core = handoff
        .find("previous.successor_independent_safe_core_v1()")
        .expect("typed safe core");
    let equation = handoff
        .find("verify_equation_kernel_v1")
        .expect("same-opening equation");
    let outer = handoff
        .find("previous.combined_outer_bindings_v1()")
        .expect("combined outer bindings");
    let finalize = handoff
        .find("finalize_verified_kernel_v1")
        .expect("post-equation finalization");
    assert!(successor < core && core < equation && equation < outer && outer < finalize);

    let owned_handoff = source
        .split_once("pub(super) fn verify_rns_native_source_packing_same_opening_owned_v2")
        .expect("owned combined handoff")
        .1
        .split_once("#[cfg(test)]")
        .expect("owned handoff boundary")
        .0;
    let owned_successor = owned_handoff
        .find("previous.same_opening_successor_v1()")
        .expect("owned successor");
    let owned_context = owned_handoff
        .find("previous.authenticated_same_opening_context_v2()")
        .expect("owned context");
    let owned_core = owned_handoff
        .find("previous.successor_independent_safe_core_v1()")
        .expect("owned safe core");
    let owned_replay = owned_handoff
        .find("previous.begin_authenticated_replay_v2()")
        .expect("owned replay");
    let owned_equation = owned_handoff
        .find("verify_equation_kernel_v1")
        .expect("owned equation");
    let owned_outer = owned_handoff
        .find("previous.combined_outer_bindings_v1()")
        .expect("owned outer bindings");
    let owned_finalize = owned_handoff
        .find("finalize_verified_kernel_v1")
        .expect("owned finalization");
    assert!(
        owned_successor < owned_context
            && owned_context < owned_core
            && owned_core < owned_replay
            && owned_replay < owned_equation
            && owned_equation < owned_outer
            && owned_outer < owned_finalize
    );
    let owned_signature = source
        .split_once("pub(super) fn verify_rns_native_source_packing_same_opening_owned_v2")
        .expect("owned handoff signature")
        .1
        .split_once("where")
        .expect("owned handoff signature boundary")
        .0;
    assert!(!owned_signature.contains("context:"));
    assert!(!owned_signature.contains("replay_source:"));

    let finalizer = source
        .split_once("fn finalize_verified_kernel_v1")
        .expect("finalizer")
        .1
        .split_once("/// Move-only evidence")
        .expect("finalizer boundary")
        .0;
    assert_eq!(
        finalizer
            .matches("combined_outer_bindings.digests_v1()")
            .count(),
        2
    );
    let outer_order = source
        .split_once("fn component_digests_v1(self) -> [[u8; DIGEST_BYTES_V1]; 19]")
        .expect("outer order")
        .1
        .split_once("fn canonical_combined_outer_binding_digest_v1")
        .expect("outer order boundary")
        .0;
    let mut previous = 0;
    for (index, field) in [
        "self.source_statement_anchor_digest",
        "self.source_final_aggregation_schedule_digest",
        "self.enclosing_packing_binding_digest",
        "self.inventory_prior_context_digest",
        "self.inventory_root",
        "self.inventory_continuation_digest",
        "self.inventory_binding_digest",
        "self.direct_binding_digest",
        "self.comparator_binding_digest",
        "self.comparator_range_carry_binding_digest",
        "self.small_sign_disjointness_binding_digest",
        "self.q_mask_linear_relations_binding_digest",
        "self.existing_radix_binding_digest",
        "self.radix_complement_binding_digest",
        "self.centering_subtraction_binding_digest",
        "self.global_lookup_pre_z_binding_digest",
        "self.global_lookup_post_z_binding_digest",
        "self.global_inverse_product_binding_digest",
        "self.global_membership_binding_digest",
    ]
    .into_iter()
    .enumerate()
    {
        let position = outer_order.find(field).expect("outer field order");
        assert!(index == 0 || previous < position, "outer order for {field}");
        previous = position;
    }
    assert!(source.contains(
        "self.combined_outer_binding_digest\n                != self.canonical_combined_outer_binding_digest_v1()"
    ));

    let executable_tests = test_source
        .split_once(
            "#[test]\nfn transcript_chronology_and_production_unavailability_are_explicit()",
        )
        .expect("test source chronology boundary")
        .0;
    assert_eq!(
        executable_tests
            .matches("prove_rns_native_source_packing_same_opening_kernel_v1(")
            .count(),
        4,
        "one cached builder plus three fresh cleanup/error prover call sites"
    );
    assert_eq!(
        executable_tests
            .matches("verify_rns_native_source_packing_same_opening_v1(")
            .count(),
        8,
        "one cached builder, five full mutation checks, and two early guards"
    );
    assert_eq!(
        executable_tests.matches("collect_commitments_v1(").count(),
        2,
        "one cached point-root builder and one independent reconstruction scan"
    );
    let cached_artifact = test_source
        .split_once("struct PublicFixtureArtifactV1")
        .expect("public cached artifact")
        .1
        .split_once("impl PublicFixtureArtifactV1")
        .expect("cached artifact boundary")
        .0;
    for forbidden in [
        "PreparedRelationV1",
        "Zeroizing",
        "FixtureReplaySourceV1",
        "FixtureMaskSourceV1",
        "DeterministicRngV1",
        "Scalar",
        "Point",
    ] {
        assert!(
            !cached_artifact.contains(forbidden),
            "cached artifact retains forbidden owner: {forbidden}"
        );
    }
    let fixture_point_cache = test_source
        .split_once("fn fixture_public_points_v1()")
        .expect("public fixture point cache")
        .1
        .split_once("fn owner_value_and_mask_commitment_v1")
        .expect("public fixture point cache boundary")
        .0;
    for required in [
        "static POINTS_V1: OnceLock<[Point; FIXTURE_PUBLIC_POINT_CACHE_POINTS_V1]>",
        "generators.h.mul_scalar(Scalar::zero())",
        "generators.h.mul_scalar(Scalar::one())",
        "owners[mask].mul_scalar(radix_share_v1(digit))",
    ] {
        assert!(
            fixture_point_cache.contains(required),
            "public fixture point cache omitted {required}"
        );
    }
    for forbidden in [
        "owner_value_and_mask_commitment_v1",
        "FixtureReplaySourceV1",
        "FixtureMaskSourceV1",
        "Zeroizing",
        "DeterministicRngV1",
        "Rc<",
        "Cell<",
        "Vec",
        "Box",
        "unsafe",
        "transmute",
    ] {
        assert!(
            !fixture_point_cache.contains(forbidden),
            "public fixture point cache retained forbidden state: {forbidden}"
        );
    }
    assert_eq!(
        test_source
            .lines()
            .filter(|line| {
                line.trim_start().starts_with(
                    "static POINTS_V1: OnceLock<[Point; FIXTURE_PUBLIC_POINT_CACHE_POINTS_V1]>",
                )
            })
            .count(),
        1
    );
    for declaration in test_source
        .lines()
        .filter(|line| line.trim_start().starts_with("static ") && line.contains("OnceLock<"))
    {
        assert!(
            declaration.contains("OnceLock<[Scalar;")
                || declaration.contains("OnceLock<[u8; DIGEST_BYTES_V1]>")
                || declaration.contains("OnceLock<PublicFixtureArtifactV1>")
                || declaration.contains(
                    "static POINTS_V1: OnceLock<[Point; FIXTURE_PUBLIC_POINT_CACHE_POINTS_V1]>",
                ),
            "unexpected process-local cache: {declaration}"
        );
    }

    let parent = include_str!("../mkhe.rs");
    assert!(parent.contains("mod rns_native_source_packing_same_opening;"));
}
