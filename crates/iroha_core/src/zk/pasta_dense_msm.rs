//! Dense normalized-GLV MSM audit for paired-Pasta recursion circuits.
//!
//! The Base circuit records one canonical Pasta point and scalar per source,
//! proves a 128-bit normalized GLV decomposition, and retains paired,
//! limb-aligned segment cells.  A source-major state machine then consumes
//! each recorded pair exactly once.  The configured physical lanes share one
//! fixed base-4 schedule column and each use a single equality-enabled bus.  Rows where
//! the bus is not loading Base data expose the lane's offset and terminal
//! accumulator coordinates, so the same copy column closes the cross-lane ring
//! without equality-enabling the accumulator columns.  The machine uses no
//! lookup tables and only current/next rotations.
//!
//! For each source the machine loads `R`, consumes the two normalized scalars
//! least-significant bit first, adds the joint digit to the accumulator, and
//! doubles `R` on the same row.  Segment recurrences `p = b + 2 p'`, together
//! with a zero terminal quotient, range-constrain and bind every bit without a
//! separate packing region.
//!
//! A large logical MSM is split into at most four source-ordered physical
//! lanes. Independent jobs are placed on the least-loaded lanes. The accumulator
//! starts at a deterministic on-curve non-identity offset `D`; equality
//! constraints copy every lane endpoint into the next lane start and close the
//! last endpoint back to `D`.  The ring closes exactly when the original unsplit
//! MSM is the identity.  Offset selection scans the complete source order to
//! avoid every exceptional affine addition, and the circuit independently
//! requires a nonzero denominator for each active addition.
use ff::{Field as _, PrimeField, WithSmallOrderMulGroup};
use halo2_base::{
    AssignedValue, Context,
    QuantumCell::{Constant, Existing},
    gates::{GateChip, GateInstructions},
    halo2_proofs::{
        circuit::{Cell, Layouter, Value},
        halo2curves::{
            CurveAffine, CurveExt,
            group::{Curve as _, Group as _},
        },
        plonk::{Advice, Column, ConstraintSystem, Error, Expression, Fixed},
        poly::Rotation,
    },
    utils::{BigPrimeField, CurveAffineExt, biguint_to_fe, modulus},
    virtual_region::copy_constraints::SharedCopyConstraintManager,
};
use halo2_ecc::{
    bigint::ProperCrtUint,
    fields::{FieldChip, Selectable, fp::FpChip},
};
const GLV_BITS: usize = 128;
const SEGMENT_BITS: usize = 7;
const SEGMENTS_PER_SCALAR: usize = 19;
const SCALARS_PER_SOURCE: usize = 2;
// The two seven-bit GLV segments are packed into one Base cell and copied on
// the segment's first operation row.  This removes the former two staging
// rows per segment without weakening either scalar's bit recurrence.
const ROWS_PER_SOURCE: usize = 2 + GLV_BITS;
const DENSE_COLUMNS: usize = 37;
const DENSE_LANES: usize = 4;
const PACKED_TAG_RADIX: u64 = 4;
const K16_USABLE_ROWS: usize = (1 << 16) - 9;
const ROWS_PER_JOB: usize = 3;
const K16_MAX_SOURCES_PER_LANE: usize = (K16_USABLE_ROWS - ROWS_PER_JOB) / ROWS_PER_SOURCE;
// A lane begins with start/count/source-x/source-y.  The first operation loads
// the packed segment; the following non-last and final operation rows expose
// the two offset coordinates on the otherwise-unused bus.
const OFFSET_X_BRIDGE_ROW: usize = 5;
const OFFSET_Y_BRIDGE_ROW: usize = 4 + SEGMENT_BITS - 1;
const OFFSET_RETRIES: u64 = 256;
const LIMB_BITS: usize = 86;
const BUS: usize = 0;
const MODE_LOAD_X: usize = 1;
const MODE_LOAD_Y: usize = 2;
const SEGMENT_START: usize = 3;
const JOB_ENDPOINT: usize = 4;
const MODE_OP: usize = 5;
const MODE_COUNT: usize = 6;
const ACC_X: usize = 7;
const ACC_Y: usize = 8;
const OFFSET_X: usize = 9;
const OFFSET_Y: usize = 10;
const SOURCE_X: usize = 11;
const SOURCE_Y: usize = 12;
const PART_1: usize = 13;
const PART_2: usize = 14;
const REMAINING_BITS: usize = 15;
const REMAINING_SEGMENTS: usize = 16;
const REMAINING_SOURCES: usize = 17;
const BIT_1: usize = 18;
const BIT_2: usize = 19;
const ADD_LAMBDA: usize = 20;
const ADD_INVERSE: usize = 21;
const DOUBLE_LAMBDA: usize = 22;
const DOUBLE_INVERSE: usize = 23;
const LAST_BIT: usize = 24;
const LAST_BIT_INVERSE: usize = 25;
const LAST_SEGMENT: usize = 26;
const LAST_SEGMENT_INVERSE: usize = 27;
const LAST_SOURCE: usize = 28;
const LAST_SOURCE_INVERSE: usize = 29;
const SHORT_SEGMENT: usize = 30;
const SHORT_SEGMENT_INVERSE: usize = 31;
const START: usize = 32;
const DIGIT_ACTIVE: usize = 33;
const DIGIT_X: usize = 34;
const DIGIT_Y: usize = 35;
const SOURCE_ENDPOINT: usize = 36;
type Base<C> = <C as CurveAffine>::Base;
type Scalar<C> = <C as CurveAffine>::ScalarExt;
// These are strict, field-wide bounds from the centered Pasta GLV lattice
// intervals, not maxima claimed to be attained by a particular scalar.
const FP_NORMALIZED_SUM_STRICT_BOUND: u128 = 0xf2da_3417_69f0_6933_12f1_e389_4000_0000;
const FQ_NORMALIZED_SUM_STRICT_BOUND: u128 = 0xf2da_3417_69a8_b971_1fd8_2955_4000_0001;
#[derive(Clone, Copy)]
struct EndoParameters {
    gamma1: [u64; 4],
    gamma2: [u64; 4],
    b1: [u64; 4],
    b2: [u64; 4],
}
// Generated by the same Pasta endomorphism derivation used by
// halo2curves-axiom.  These constants only create host witnesses; the circuit
// independently proves the resulting scalar relation.
const ENDO_PARAMS_EQ: EndoParameters = EndoParameters {
    gamma1: [0x32c4_9e4c_0000_0003, 0x279a_7459_02a2_654e, 0x1, 0x0],
    gamma2: [0x31f0_2568_0000_0002, 0x4f34_e8b2_0663_89a4, 0x2, 0x0],
    b1: [0x8cb1_2793_0000_0001, 0x49e6_9d16_40a8_9953, 0x0, 0x0],
    b2: [0x0c7c_095a_0000_0001, 0x93cd_3a2c_8198_e269, 0x0, 0x0],
};
const ENDO_PARAMS_EP: EndoParameters = EndoParameters {
    gamma1: [0x32c4_9e4b_ffff_ffff, 0x279a_7459_02a2_654e, 0x1, 0x0],
    gamma2: [0x31f0_2568_0000_0002, 0x4f34_e8b2_0663_89a4, 0x2, 0x0],
    b1: [0x8cb1_2793_0000_0000, 0x49e6_9d16_40a8_9953, 0x0, 0x0],
    b2: [0x0c7c_095a_0000_0001, 0x93cd_3a2c_8198_e269, 0x0, 0x0],
};
/// One source/coefficient pair entering the dense reciprocal audit.
#[derive(Clone, Debug)]
pub(super) struct PastaDenseMsmSourceV1<C>
where
    C: CurveAffineExt,
    Base<C>: BigPrimeField,
{
    /// Canonical non-identity host point used to populate raw witnesses.
    pub(super) point: C,
    /// Base-circuit x coordinate of `point`.
    pub(super) x: AssignedValue<Base<C>>,
    /// Base-circuit y coordinate of `point`.
    pub(super) y: AssignedValue<Base<C>>,
    /// Canonical scalar coefficient multiplying `point`.
    pub(super) coefficient: ProperCrtUint<Base<C>>,
}
#[derive(Clone, Debug)]
struct ConstrainedDenseSource<C>
where
    C: CurveAffineExt,
    Base<C>: BigPrimeField,
{
    r: C,
    r_x: AssignedValue<Base<C>>,
    r_y: AssignedValue<Base<C>>,
    paired_segments: [AssignedValue<Base<C>>; SEGMENTS_PER_SCALAR],
    bits: [[bool; GLV_BITS]; SCALARS_PER_SOURCE],
}
#[derive(Clone, Debug)]
struct DenseMsmJob<C>
where
    C: CurveAffineExt,
    Base<C>: BigPrimeField,
{
    start_tag: AssignedValue<Base<C>>,
    source_count_tags: Vec<AssignedValue<Base<C>>>,
    /// Physical lane selected for each source-ordered logical shard.
    physical_lanes: Vec<usize>,
    sources: Vec<ConstrainedDenseSource<C>>,
}
/// Circuit-owned normalized-GLV dense MSM jobs.
#[derive(Clone, Debug)]
pub(super) struct PastaDenseMsmJobsV1<C>
where
    C: CurveAffineExt,
    Base<C>: BigPrimeField,
{
    jobs: Vec<DenseMsmJob<C>>,
    use_unknown: bool,
}
impl<C> Default for PastaDenseMsmJobsV1<C>
where
    C: CurveAffineExt,
    Base<C>: BigPrimeField,
{
    fn default() -> Self {
        Self {
            jobs: Vec::new(),
            use_unknown: false,
        }
    }
}
/// One to four disjoint advice lanes with one packed fixed schedule for the dense audit.
///
/// Each lane has exactly one equality-enabled bus.  It binds source data to
/// the Base graph and multiplexes the offset/terminal coordinates used to join
/// lane endpoints into the ring whose closure enforces the original logical
/// MSM identity.
#[derive(Clone, Debug)]
pub(crate) struct PastaDenseMsmConfigV1 {
    lanes: Vec<PastaDenseMsmLaneConfigV1>,
    packed_schedule: Column<Fixed>,
}
#[derive(Clone, Copy, Debug)]
struct PastaDenseMsmLaneConfigV1 {
    columns: [Column<Advice>; DENSE_COLUMNS],
}
impl PastaDenseMsmConfigV1 {
    /// Allocate the historical four advice lanes and install the dense gate.
    pub(crate) fn configure<C>(meta: &mut ConstraintSystem<Base<C>>) -> Self
    where
        C: CurveAffineExt,
        Base<C>: BigPrimeField + WithSmallOrderMulGroup<3>,
    {
        Self::configure_with_lanes::<C>(meta, DENSE_LANES)
    }
    /// Allocate exactly `lane_count` advice lanes and install the dense gate.
    ///
    /// # Panics
    ///
    /// Panics unless `lane_count` is in `1..=4`.  Circuit configuration is static, so an invalid
    /// lane count is a programmer error rather than a prover-controlled synthesis failure.
    pub(crate) fn configure_with_lanes<C>(
        meta: &mut ConstraintSystem<Base<C>>,
        lane_count: usize,
    ) -> Self
    where
        C: CurveAffineExt,
        Base<C>: BigPrimeField + WithSmallOrderMulGroup<3>,
    {
        assert!(
            (1..=DENSE_LANES).contains(&lane_count),
            "Paired Pasta dense MSM lane count must be in 1..={DENSE_LANES}"
        );
        let lanes = (0..lane_count)
            .map(|_| {
                let columns = std::array::from_fn(|_| meta.advice_column());
                meta.enable_equality(columns[BUS]);
                PastaDenseMsmLaneConfigV1 { columns }
            })
            .collect::<Vec<_>>();
        let packed_schedule = meta.fixed_column();
        let lane_columns = lanes.iter().map(|lane| lane.columns).collect::<Vec<_>>();
        meta.create_gate("Paired Pasta dense MSM machines", |meta| {
            let packed = meta.query_fixed(packed_schedule, Rotation::cur());
            let zero = Expression::Constant(Base::<C>::ZERO);
            let one = Expression::Constant(Base::<C>::ONE);
            let two = Expression::Constant(Base::<C>::from(2));
            let mut packed_tags = zero.clone();
            let mut radix = Base::<C>::ONE;
            let mut constraints = Vec::new();
            for columns in lane_columns {
                let current =
                    std::array::from_fn(|index| meta.query_advice(columns[index], Rotation::cur()));
                let next = std::array::from_fn(|index| {
                    meta.query_advice(columns[index], Rotation::next())
                });
                let active_mode = [MODE_LOAD_X, MODE_LOAD_Y, MODE_OP, MODE_COUNT]
                    .into_iter()
                    .map(|index| current[index].clone())
                    .fold(zero.clone(), |sum, mode| sum + mode);
                // The fixed schedule authenticates the lane shape without a
                // lane-local selector: 1 is the start row, 2 is every active
                // machine row, and 0 leaves the lane disabled.  Range
                // constraints make the base-4 decomposition unique.
                let tag = current[START].clone() + two.clone() * active_mode;
                constraints.push(
                    packed.clone()
                        * tag.clone()
                        * (tag.clone() - one.clone())
                        * (tag.clone() - two.clone()),
                );
                constraints.extend(
                    dense_machine_constraints::<C>(&current, &next)
                        .into_iter()
                        // `packed` is also required here: advice is randomized
                        // in Halo2's blinding rows, while the fixed schedule is
                        // zero there.  The fixed-zero factor keeps every
                        // off-schedule lane completely disabled.
                        .map(|constraint| packed.clone() * tag.clone() * constraint),
                );
                packed_tags = packed_tags + Expression::Constant(radix) * tag;
                radix *= Base::<C>::from(PACKED_TAG_RADIX);
            }
            // A zero fixed row intentionally remains disabled.  On every
            // scheduled row, the bounded digits and this equality uniquely
            // authenticate every configured lane tag at once.
            constraints.push(packed.clone() * (packed - packed_tags));
            constraints
        });
        Self {
            lanes,
            packed_schedule,
        }
    }
    fn lane_count(&self) -> usize {
        self.lanes.len()
    }
}
fn dense_machine_constraints<C>(
    current: &[Expression<Base<C>>; DENSE_COLUMNS],
    next: &[Expression<Base<C>>; DENSE_COLUMNS],
) -> Vec<Expression<Base<C>>>
where
    C: CurveAffineExt,
    Base<C>: BigPrimeField + WithSmallOrderMulGroup<3>,
{
    let one = Expression::Constant(Base::<C>::ONE);
    let two = Expression::Constant(Base::<C>::from(2));
    let three = Expression::Constant(Base::<C>::from(3));
    let five = Expression::Constant(Base::<C>::from(5));
    let seven = Expression::Constant(Base::<C>::from(7));
    let nineteen = Expression::Constant(Base::<C>::from(19));
    let half = Expression::Constant(
        Option::<Base<C>>::from(Base::<C>::from(2).invert()).expect("two is nonzero"),
    );
    let bus = current[BUS].clone();
    let load_x = current[MODE_LOAD_X].clone();
    let load_y = current[MODE_LOAD_Y].clone();
    let segment_start = current[SEGMENT_START].clone();
    let job_endpoint = current[JOB_ENDPOINT].clone();
    let op = current[MODE_OP].clone();
    let count = current[MODE_COUNT].clone();
    let modes = [load_x.clone(), load_y.clone(), op.clone(), count.clone()];
    let enabled = modes
        .iter()
        .cloned()
        .fold(Expression::Constant(Base::<C>::ZERO), |sum, mode| {
            sum + mode
        });
    let start = current[START].clone();
    let passive = count.clone() + load_x.clone() + load_y.clone();
    let live = passive.clone() + op.clone();
    let acc_x = current[ACC_X].clone();
    let acc_y = current[ACC_Y].clone();
    let offset_x = current[OFFSET_X].clone();
    let offset_y = current[OFFSET_Y].clone();
    let source_x = current[SOURCE_X].clone();
    let source_y = current[SOURCE_Y].clone();
    let part_1 = current[PART_1].clone();
    let part_2 = current[PART_2].clone();
    let remaining_bits = current[REMAINING_BITS].clone();
    let remaining_segments = current[REMAINING_SEGMENTS].clone();
    let remaining_sources = current[REMAINING_SOURCES].clone();
    let bit_1 = current[BIT_1].clone();
    let bit_2 = current[BIT_2].clone();
    let add_lambda = current[ADD_LAMBDA].clone();
    let add_inverse = current[ADD_INVERSE].clone();
    let double_lambda = current[DOUBLE_LAMBDA].clone();
    let double_inverse = current[DOUBLE_INVERSE].clone();
    let last_bit = current[LAST_BIT].clone();
    let last_bit_inverse = current[LAST_BIT_INVERSE].clone();
    let last_segment = current[LAST_SEGMENT].clone();
    let last_segment_inverse = current[LAST_SEGMENT_INVERSE].clone();
    let last_source = current[LAST_SOURCE].clone();
    let last_source_inverse = current[LAST_SOURCE_INVERSE].clone();
    let short_segment = current[SHORT_SEGMENT].clone();
    let short_segment_inverse = current[SHORT_SEGMENT_INVERSE].clone();
    let digit_active = current[DIGIT_ACTIVE].clone();
    let digit_x = current[DIGIT_X].clone();
    let digit_y = current[DIGIT_Y].clone();
    let source_endpoint = current[SOURCE_ENDPOINT].clone();
    let both = bit_1.clone() * bit_2.clone();
    let beta = Expression::Constant(Base::<C>::ZETA);
    let beta_squared = Expression::Constant(Base::<C>::ZETA.square());
    let expected_digit_x = source_x.clone()
        * (bit_1.clone() + beta * bit_2.clone() + two.clone() * beta_squared * both.clone());
    let expected_digit_y =
        source_y.clone() * (bit_1.clone() + bit_2.clone() - three.clone() * both.clone());
    let delta_x = digit_x.clone() - acc_x.clone();
    let delta_y = digit_y.clone() - acc_y.clone();
    let double_denominator = two.clone() * source_y.clone();
    let mut constraints = Vec::new();
    for mode in modes {
        constraints.push(mode.clone() * (mode - one.clone()));
    }
    constraints.push(enabled.clone() * (enabled.clone() - one.clone()));
    constraints.push(start.clone() * (start.clone() - one.clone()));
    constraints.extend([
        start.clone() - bus.clone() * (one.clone() - enabled.clone()),
        segment_start.clone() * (segment_start.clone() - one.clone()),
        segment_start.clone() * (one.clone() - op.clone()),
        digit_active.clone() - op.clone() * (bit_1.clone() + bit_2.clone() - both),
        digit_x.clone() - op.clone() * expected_digit_x,
        digit_y.clone() - op.clone() * expected_digit_y,
        source_endpoint.clone() - op.clone() * last_bit.clone() * last_segment.clone(),
        job_endpoint.clone() - source_endpoint.clone() * last_source.clone(),
    ]);
    let segment_pair_radix = Expression::Constant(pow2::<Base<C>>(LIMB_BITS));
    constraints.extend([
        // Each pair of GLV segments is copied from the Base graph once.  The
        // independent bit recurrences below range-constrain both packed
        // components.  The 2^86 radix matches the proper-integer limb width:
        // together with the two limb reconstruction equations, it makes the
        // split injective even though the Base-side segment witnesses are not
        // range-checked individually.
        segment_start.clone()
            * (bus.clone() - part_1.clone() - segment_pair_radix * part_2.clone()),
        // Non-loading operation rows expose the lane offset.  The second and
        // last bits of the first segment provide stable x/y bridge cells.
        op.clone()
            * (one.clone() - segment_start.clone())
            * (one.clone() - last_bit.clone())
            * (bus.clone() - offset_x.clone()),
        op.clone()
            * (one.clone() - job_endpoint.clone())
            * last_bit.clone()
            * (bus.clone() - offset_y.clone()),
        // The last operation exposes terminal x in its bus and terminal y in
        // the following inactive row's bus.  Both are tied to the accumulator
        // recurrence rather than accepted as free copy witnesses.
        op.clone() * job_endpoint.clone() * (bus.clone() - next[ACC_X].clone()),
        op.clone() * job_endpoint * (next[BUS].clone() - next[ACC_Y].clone()),
    ]);
    constraints.extend([
        next[MODE_COUNT].clone() - start.clone(),
        next[MODE_LOAD_X].clone()
            - count.clone()
            - source_endpoint.clone() * (one.clone() - last_source.clone()),
        next[MODE_LOAD_Y].clone() - load_x.clone(),
        next[MODE_OP].clone()
            - load_y.clone()
            - op.clone() * (one.clone() - source_endpoint.clone()),
        next[SEGMENT_START].clone()
            - load_y.clone()
            - op.clone() * last_bit.clone() * (one.clone() - last_segment.clone()),
    ]);
    // `D` is injected on the authenticated start row, then carried unchanged.
    constraints.extend([
        (one.clone() - start.clone()) * next[OFFSET_X].clone() - live.clone() * offset_x.clone(),
        (one.clone() - start.clone()) * next[OFFSET_Y].clone() - live.clone() * offset_y.clone(),
        start.clone()
            * (next[OFFSET_Y].clone() * next[OFFSET_Y].clone()
                - next[OFFSET_X].clone() * next[OFFSET_X].clone() * next[OFFSET_X].clone()
                - five.clone()),
        start.clone() * (next[OFFSET_Y].clone() * add_inverse.clone() - one.clone()),
    ]);
    constraints.extend([
        load_y.clone()
            * (bus.clone() * bus.clone()
                - source_x.clone() * source_x.clone() * source_x.clone()
                - five),
        load_y.clone() * (bus.clone() * add_inverse.clone() - one.clone()),
    ]);
    constraints.extend([
        op.clone() * bit_1.clone() * (bit_1.clone() - one.clone()),
        op.clone() * bit_2.clone() * (bit_2.clone() - one.clone()),
        op.clone() * (one.clone() - digit_active.clone()) * add_inverse.clone(),
        op.clone() * (delta_x * add_inverse.clone() - digit_active.clone()),
        op.clone() * (add_lambda.clone() - delta_y * add_inverse.clone()),
        op.clone() * (double_denominator.clone() * double_inverse.clone() - one.clone()),
        op.clone()
            * (double_lambda.clone() * double_denominator
                - three * source_x.clone() * source_x.clone()),
    ]);
    constraints.extend([
        next[ACC_X].clone()
            - start.clone() * next[OFFSET_X].clone()
            - passive.clone() * acc_x.clone()
            - op.clone()
                * (digit_active.clone() * add_lambda.clone() * add_lambda.clone()
                    + acc_x.clone() * (one.clone() - two.clone() * digit_active.clone())
                    - digit_x),
        next[ACC_Y].clone()
            - start.clone() * next[OFFSET_Y].clone()
            - passive.clone() * acc_y.clone()
            - op.clone()
                * (digit_active.clone() * add_lambda * (acc_x.clone() - next[ACC_X].clone())
                    + acc_y.clone() * (one.clone() - two.clone() * digit_active)),
        next[SOURCE_X].clone()
            - load_x.clone() * bus.clone()
            - load_y.clone() * source_x.clone()
            - op.clone()
                * (double_lambda.clone() * double_lambda.clone() - two.clone() * source_x.clone()),
        next[SOURCE_Y].clone()
            - load_y.clone() * bus.clone()
            - op.clone() * (double_lambda * (source_x - next[SOURCE_X].clone()) - source_y),
        op.clone()
            * (one.clone() - last_bit.clone())
            * (next[PART_1].clone() - half.clone() * (part_1.clone() - bit_1.clone())),
        op.clone()
            * (one.clone() - last_bit.clone())
            * (next[PART_2].clone() - half * (part_2.clone() - bit_2.clone())),
        op.clone() * last_bit.clone() * (part_1 - bit_1),
        op.clone() * last_bit.clone() * (part_2 - bit_2),
    ]);
    constraints.extend([
        next[REMAINING_BITS].clone()
            - load_y.clone() * seven.clone()
            - op.clone()
                * (remaining_bits.clone() - one.clone())
                * (one.clone() - last_bit.clone())
            - op.clone()
                * last_bit.clone()
                * (one.clone() - last_segment.clone())
                * (seven.clone()
                    - Expression::Constant(Base::<C>::from(5)) * next[SHORT_SEGMENT].clone()),
        next[REMAINING_SEGMENTS].clone()
            - count.clone() * nineteen.clone()
            - load_x.clone() * nineteen
            - load_y.clone() * remaining_segments.clone()
            - op.clone() * (remaining_segments.clone() - last_bit.clone()),
        next[REMAINING_SOURCES].clone()
            - count.clone() * current[BUS].clone()
            - (load_x.clone() + load_y.clone()) * remaining_sources.clone()
            - op.clone() * (remaining_sources.clone() - source_endpoint.clone()),
    ]);
    let bits_minus_one = remaining_bits - one.clone();
    constraints.extend([
        op.clone() * (bits_minus_one.clone() * last_bit_inverse - (one.clone() - last_bit.clone())),
        op.clone() * bits_minus_one.clone() * last_bit.clone(),
        op.clone() * last_bit.clone() * (last_bit.clone() - one.clone()),
    ]);
    let segments_minus_one = remaining_segments.clone() - one.clone();
    constraints.extend([
        op.clone()
            * (segments_minus_one.clone() * last_segment_inverse
                - (one.clone() - last_segment.clone())),
        op.clone() * segments_minus_one * last_segment.clone(),
        op.clone() * last_segment.clone() * (last_segment.clone() - one.clone()),
    ]);
    let sources_minus_one = remaining_sources - one.clone();
    constraints.extend([
        source_endpoint.clone()
            * (sources_minus_one.clone() * last_source_inverse
                - (one.clone() - last_source.clone())),
        source_endpoint.clone() * sources_minus_one * last_source.clone(),
        source_endpoint.clone() * last_source.clone() * (last_source.clone() - one.clone()),
    ]);
    let segments_minus_seven = remaining_segments - Expression::Constant(Base::<C>::from(7));
    constraints.extend([
        op.clone()
            * (segments_minus_seven.clone() * short_segment_inverse
                - (one.clone() - short_segment.clone())),
        op.clone() * segments_minus_seven * short_segment.clone(),
        op * short_segment.clone() * (short_segment - one),
    ]);
    constraints
}
fn dense_lane_count_with_limit(
    source_count: usize,
    configured_lanes: usize,
) -> Result<usize, String> {
    validate_configured_lane_count(configured_lanes)?;
    if source_count == 0 {
        return Err("Paired Pasta dense MSM requires at least one source".to_owned());
    }
    let lanes = source_count.div_ceil(K16_MAX_SOURCES_PER_LANE);
    if lanes > configured_lanes {
        return Err(format!(
            "Paired Pasta dense MSM requires {lanes} physical lanes for {source_count} sources; only {configured_lanes} are configured"
        ));
    }
    Ok(lanes)
}
#[cfg(test)]
fn dense_lane_count(source_count: usize) -> Result<usize, String> {
    dense_lane_count_with_limit(source_count, DENSE_LANES)
}
fn validate_configured_lane_count(lane_count: usize) -> Result<(), String> {
    if !(1..=DENSE_LANES).contains(&lane_count) {
        return Err(format!(
            "Paired Pasta dense MSM configured lane count must be in 1..={DENSE_LANES}; got {lane_count}"
        ));
    }
    Ok(())
}
fn dense_shard_bounds(source_count: usize, lane_count: usize, lane: usize) -> (usize, usize) {
    debug_assert!(lane_count > 0 && lane < lane_count && lane_count <= source_count);
    let base = source_count / lane_count;
    let remainder = source_count % lane_count;
    let start = lane * base + lane.min(remainder);
    let len = base + usize::from(lane < remainder);
    (start, start + len)
}
fn dense_shard_rows(
    source_count: usize,
    lane_count: usize,
    logical_lane: usize,
) -> Result<usize, String> {
    let (start, end) = dense_shard_bounds(source_count, lane_count, logical_lane);
    (end - start)
        .checked_mul(ROWS_PER_SOURCE)
        .and_then(|rows| rows.checked_add(ROWS_PER_JOB))
        .ok_or_else(|| "dense MSM row count overflow".to_owned())
}
#[derive(Clone, Debug)]
struct DenseSchedulingAlternative {
    shard_rows: Vec<usize>,
    total_rows: usize,
}
#[derive(Clone, Debug)]
struct DenseSchedulingJob {
    original_index: usize,
    source_count: usize,
    alternatives: Vec<DenseSchedulingAlternative>,
    minimum_total_rows: usize,
}
fn dense_schedule_key(mut lane_rows: [usize; DENSE_LANES]) -> [usize; DENSE_LANES] {
    lane_rows.sort_unstable();
    lane_rows
}
fn enumerate_dense_job_assignments(
    shard_rows: &[usize],
    lane_rows: [usize; DENSE_LANES],
) -> Vec<([usize; DENSE_LANES], Vec<usize>)> {
    fn recurse(
        shard_rows: &[usize],
        logical_lane: usize,
        lane_rows: &mut [usize; DENSE_LANES],
        used_lanes: &mut [bool; DENSE_LANES],
        assignment: &mut Vec<usize>,
        candidates: &mut Vec<([usize; DENSE_LANES], Vec<usize>)>,
    ) {
        if logical_lane == shard_rows.len() {
            if !candidates
                .iter()
                .any(|(candidate_rows, _)| *candidate_rows == *lane_rows)
            {
                candidates.push((*lane_rows, assignment.clone()));
            }
            return;
        }
        let rows = shard_rows[logical_lane];
        let mut equivalent_loads = Vec::with_capacity(DENSE_LANES);
        for physical_lane in 0..DENSE_LANES {
            if used_lanes[physical_lane] || equivalent_loads.contains(&lane_rows[physical_lane]) {
                continue;
            }
            equivalent_loads.push(lane_rows[physical_lane]);
            let Some(next_rows) = lane_rows[physical_lane].checked_add(rows) else {
                continue;
            };
            if next_rows > K16_USABLE_ROWS {
                continue;
            }
            let previous_rows = lane_rows[physical_lane];
            lane_rows[physical_lane] = next_rows;
            used_lanes[physical_lane] = true;
            assignment.push(physical_lane);
            recurse(
                shard_rows,
                logical_lane + 1,
                lane_rows,
                used_lanes,
                assignment,
                candidates,
            );
            assignment.pop();
            used_lanes[physical_lane] = false;
            lane_rows[physical_lane] = previous_rows;
        }
    }

    let mut candidate_rows = lane_rows;
    let mut used_lanes = [false; DENSE_LANES];
    let mut assignment = Vec::with_capacity(shard_rows.len());
    let mut candidates = Vec::new();
    recurse(
        shard_rows,
        0,
        &mut candidate_rows,
        &mut used_lanes,
        &mut assignment,
        &mut candidates,
    );
    // Best-fit ordering usually finds a solution without backtracking.  The
    // exhaustive fallback remains complete and deterministic.
    candidates.sort_by(
        |(left_rows, left_assignment), (right_rows, right_assignment)| {
            dense_schedule_key(*right_rows)
                .cmp(&dense_schedule_key(*left_rows))
                .then_with(|| left_assignment.cmp(right_assignment))
        },
    );
    candidates
}
fn enumerate_dense_job_choices(
    job: &DenseSchedulingJob,
    lane_rows: [usize; DENSE_LANES],
) -> Vec<([usize; DENSE_LANES], Vec<usize>)> {
    let mut candidates = job
        .alternatives
        .iter()
        .flat_map(|alternative| enumerate_dense_job_assignments(&alternative.shard_rows, lane_rows))
        .collect::<Vec<_>>();
    candidates.sort_by(
        |(left_rows, left_assignment), (right_rows, right_assignment)| {
            left_assignment
                .len()
                .cmp(&right_assignment.len())
                .then_with(|| dense_schedule_key(*right_rows).cmp(&dense_schedule_key(*left_rows)))
                .then_with(|| left_assignment.cmp(right_assignment))
        },
    );
    candidates
}
fn remaining_dense_jobs_fit_individually(
    order: &[usize],
    jobs: &[DenseSchedulingJob],
    lane_rows: [usize; DENSE_LANES],
) -> bool {
    let mut capacities = lane_rows.map(|rows| K16_USABLE_ROWS - rows);
    capacities.sort_unstable_by(|left, right| right.cmp(left));
    order.iter().all(|job_index| {
        jobs[*job_index].alternatives.iter().any(|alternative| {
            // Balanced logical shards are already ordered largest first.
            alternative
                .shard_rows
                .iter()
                .zip(capacities.iter().copied())
                .all(|(rows, capacity)| *rows <= capacity)
        })
    })
}
fn search_dense_job_schedule(
    position: usize,
    order: &[usize],
    jobs: &[DenseSchedulingJob],
    lane_rows: [usize; DENSE_LANES],
    assignments: &mut [Vec<usize>],
    failed_states: &mut std::collections::HashSet<(usize, [usize; DENSE_LANES])>,
) -> bool {
    const MEMO_LIMIT: usize = 16_384;
    if position == order.len() {
        return true;
    }
    let state = (position, dense_schedule_key(lane_rows));
    if failed_states.contains(&state) {
        return false;
    }
    let job_index = order[position];
    for (next_rows, assignment) in enumerate_dense_job_choices(&jobs[job_index], lane_rows) {
        if !remaining_dense_jobs_fit_individually(&order[position + 1..], jobs, next_rows) {
            continue;
        }
        assignments[jobs[job_index].original_index] = assignment;
        if search_dense_job_schedule(
            position + 1,
            order,
            jobs,
            next_rows,
            assignments,
            failed_states,
        ) {
            return true;
        }
        assignments[jobs[job_index].original_index].clear();
    }
    // Memoization is only an optimization.  Capping it bounds scheduler
    // memory; omitted states are still explored exactly, so no feasible plan
    // can become a false rejection.
    if failed_states.len() < MEMO_LIMIT {
        failed_states.insert(state);
    }
    false
}
fn plan_dense_jobs_with_lanes(
    job_source_counts: &[usize],
    configured_lanes: usize,
) -> Result<Vec<Vec<usize>>, String> {
    validate_configured_lane_count(configured_lanes)?;
    if job_source_counts.is_empty() {
        return Ok(Vec::new());
    }
    let minimum_shard_rows = ROWS_PER_SOURCE
        .checked_add(ROWS_PER_JOB)
        .ok_or_else(|| "dense MSM row count overflow".to_owned())?;
    let maximum_shards = (K16_USABLE_ROWS / minimum_shard_rows)
        .checked_mul(configured_lanes)
        .ok_or_else(|| "dense MSM row capacity overflow".to_owned())?;
    if job_source_counts.len() > maximum_shards {
        return Err(format!(
            "Paired Pasta dense MSM requires at least {} logical shards, exceeding the {configured_lanes}-lane k=16 shard capacity of {maximum_shards}",
            job_source_counts.len(),
        ));
    }
    let mut jobs = Vec::with_capacity(job_source_counts.len());
    let mut all_rows = 0_usize;
    for (original_index, source_count) in job_source_counts.iter().copied().enumerate() {
        let minimum_lane_count = dense_lane_count_with_limit(source_count, configured_lanes)?;
        let maximum_lane_count = configured_lanes.min(source_count);
        let alternatives = (minimum_lane_count..=maximum_lane_count)
            .map(|lane_count| {
                let shard_rows = (0..lane_count)
                    .map(|logical_lane| {
                        dense_shard_rows(source_count, lane_count, logical_lane)
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                if shard_rows.iter().any(|rows| *rows > K16_USABLE_ROWS) {
                    return Err(format!(
                        "Paired Pasta dense MSM job {original_index} has a logical shard exceeding the k=16 absolute maximum of {K16_USABLE_ROWS}"
                    ));
                }
                let total_rows = shard_rows.iter().try_fold(0_usize, |total, rows| {
                    total
                        .checked_add(*rows)
                        .ok_or_else(|| "dense MSM row count overflow".to_owned())
                })?;
                Ok(DenseSchedulingAlternative {
                    shard_rows,
                    total_rows,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let minimum_total_rows = alternatives
            .first()
            .map(|alternative| alternative.total_rows)
            .ok_or_else(|| "dense MSM lane assignment failed".to_owned())?;
        all_rows = all_rows
            .checked_add(minimum_total_rows)
            .ok_or_else(|| "dense MSM row count overflow".to_owned())?;
        jobs.push(DenseSchedulingJob {
            original_index,
            source_count,
            alternatives,
            minimum_total_rows,
        });
    }
    let total_capacity = K16_USABLE_ROWS
        .checked_mul(configured_lanes)
        .ok_or_else(|| "dense MSM row capacity overflow".to_owned())?;
    if all_rows > total_capacity {
        return Err(format!(
            "Paired Pasta dense MSM requires {all_rows} total rows, exceeding the {configured_lanes}-lane k=16 absolute capacity of {total_capacity}"
        ));
    }
    let mut order = (0..jobs.len()).collect::<Vec<_>>();
    order.sort_by(|left, right| {
        jobs[*right]
            .source_count
            .cmp(&jobs[*left].source_count)
            .then_with(|| {
                jobs[*right]
                    .minimum_total_rows
                    .cmp(&jobs[*left].minimum_total_rows)
            })
            .then_with(|| left.cmp(right))
    });
    let mut assignments = vec![Vec::new(); jobs.len()];
    let mut failed_states = std::collections::HashSet::new();
    let mut initial_lane_rows = [K16_USABLE_ROWS; DENSE_LANES];
    initial_lane_rows[..configured_lanes].fill(0);
    // The search enumerates every capacity-respecting injective placement for
    // each job.  It removes only permutations of equal-load physical lanes,
    // which are equivalent because jobs interact solely through lane load.
    if !search_dense_job_schedule(
        0,
        &order,
        &jobs,
        initial_lane_rows,
        &mut assignments,
        &mut failed_states,
    ) {
        return Err(format!(
            "Paired Pasta dense MSM logical shards do not fit in {configured_lanes} k=16 lanes of {K16_USABLE_ROWS} usable rows"
        ));
    }
    Ok(assignments)
}
#[cfg(test)]
fn plan_dense_jobs(job_source_counts: &[usize]) -> Result<Vec<Vec<usize>>, String> {
    plan_dense_jobs_with_lanes(job_source_counts, DENSE_LANES)
}
impl<C> PastaDenseMsmJobsV1<C>
where
    C: CurveAffineExt,
    Base<C>: BigPrimeField + WithSmallOrderMulGroup<3>,
    Scalar<C>: BigPrimeField + WithSmallOrderMulGroup<3>,
{
    /// Queue one source-ordered MSM and prove its normalized GLV bindings in the Base circuit.
    pub(super) fn queue_constrained(
        &mut self,
        ctx: &mut Context<Base<C>>,
        scalar_chip: &FpChip<'_, Base<C>, Scalar<C>>,
        sources: &[PastaDenseMsmSourceV1<C>],
    ) -> Result<(), String> {
        self.queue_constrained_with_lanes(ctx, scalar_chip, sources, DENSE_LANES)
    }
    /// Queue one source-ordered MSM against an explicitly configured physical-lane bound.
    ///
    /// Planning uses the same bound as synthesis before any per-source Base graph is allocated,
    /// so a circuit configured with fewer than four lanes cannot accept a four-lane plan and fail
    /// only after the expensive witness graph has been built.
    pub(super) fn queue_constrained_with_lanes(
        &mut self,
        ctx: &mut Context<Base<C>>,
        scalar_chip: &FpChip<'_, Base<C>, Scalar<C>>,
        sources: &[PastaDenseMsmSourceV1<C>],
        configured_lanes: usize,
    ) -> Result<(), String> {
        validate_configured_lane_count(configured_lanes)?;
        if sources.is_empty() {
            return Err("Paired Pasta dense MSM requires at least one source".to_owned());
        }
        let source_count = sources.len();
        // Authenticate the absolute k=16 geometry before constructing any
        // source's Base constraint graph.  In particular, an oversized or
        // cumulatively overfilled carrier fails using only public lengths.
        // Caller-specific usable-row limits (including smaller test circuits)
        // remain authoritative in `validate_capacity` during synthesis.
        let mut job_source_counts = Vec::with_capacity(self.jobs.len() + 1);
        for job in &self.jobs {
            let job_lane_count = job.source_count_tags.len();
            if job.physical_lanes.len() != job_lane_count
                || job_lane_count == 0
                || job_lane_count > job.sources.len()
            {
                return Err("dense MSM lane assignment shape is invalid".to_owned());
            }
            let mut used = [false; DENSE_LANES];
            for physical_lane in job.physical_lanes.iter().copied() {
                if physical_lane >= DENSE_LANES || used[physical_lane] {
                    return Err("dense MSM physical lane assignment is invalid".to_owned());
                }
                used[physical_lane] = true;
            }
            job_source_counts.push(job.sources.len());
        }
        job_source_counts.push(source_count);
        let mut planned_lanes = plan_dense_jobs_with_lanes(&job_source_counts, configured_lanes)?;
        let physical_lanes = planned_lanes
            .pop()
            .ok_or_else(|| "dense MSM lane assignment failed".to_owned())?;
        let lane_count = physical_lanes.len();
        if planned_lanes.len() != self.jobs.len()
            || lane_count == 0
            || lane_count > configured_lanes
            || lane_count > source_count
        {
            return Err("dense MSM lane assignment shape is invalid".to_owned());
        }
        let mut constrained = Vec::new();
        constrained
            .try_reserve_exact(source_count)
            .map_err(|_| "Paired Pasta dense MSM source allocation failed".to_owned())?;
        for (source_index, source) in sources.iter().enumerate() {
            constrained.push(
                constrain_source(ctx, scalar_chip, source)
                    .map_err(|error| format!("dense MSM source {source_index}: {error}"))?,
            );
        }
        // Global offset selection is also an early completeness check.  It is
        // deliberately performed over the original source order before the
        // trace is split across physical lanes.
        choose_offset::<C>(&constrained)?;
        let start_tag = ctx.load_constant(Base::<C>::ONE);
        let mut replacement_source_count_tags = Vec::with_capacity(self.jobs.len());
        for (job, replacement) in self.jobs.iter().zip(&planned_lanes) {
            if replacement.len() == job.source_count_tags.len() {
                replacement_source_count_tags.push(job.source_count_tags.clone());
                continue;
            }
            let tags = (0..replacement.len())
                .map(|lane| {
                    let (start, end) =
                        dense_shard_bounds(job.sources.len(), replacement.len(), lane);
                    u64::try_from(end - start)
                        .map(Base::<C>::from)
                        .map(|count| ctx.load_constant(count))
                        .map_err(|_| "dense MSM source count exceeds u64".to_owned())
                })
                .collect::<Result<Vec<_>, _>>()?;
            replacement_source_count_tags.push(tags);
        }
        let source_count_tags = (0..lane_count)
            .map(|lane| {
                let (start, end) = dense_shard_bounds(source_count, lane_count, lane);
                u64::try_from(end - start)
                    .map(Base::<C>::from)
                    .map(|count| ctx.load_constant(count))
                    .map_err(|_| "dense MSM source count exceeds u64".to_owned())
            })
            .collect::<Result<Vec<_>, _>>()?;
        // Commit the complete plan only after all fallible planning and source
        // construction has succeeded.  Logical-lane order within every job is
        // retained; only its physical placement changes.
        for ((job, replacement), replacement_tags) in self
            .jobs
            .iter_mut()
            .zip(planned_lanes)
            .zip(replacement_source_count_tags)
        {
            job.physical_lanes = replacement;
            job.source_count_tags = replacement_tags;
        }
        self.jobs.push(DenseMsmJob {
            start_tag,
            source_count_tags,
            physical_lanes,
            sources: constrained,
        });
        Ok(())
    }
    /// Preserve job shape while hiding all raw witnesses.
    pub(super) fn unknown(&self) -> Self {
        let mut clone = self.clone();
        clone.use_unknown = true;
        clone
    }
    /// Return the exact maximum raw-row count across the historical four physical lanes.
    pub(super) fn required_rows(&self) -> Result<usize, String> {
        self.required_rows_with_lanes(DENSE_LANES)
    }
    /// Return the exact maximum raw-row count across `configured_lanes` physical lanes.
    pub(super) fn required_rows_with_lanes(
        &self,
        configured_lanes: usize,
    ) -> Result<usize, String> {
        validate_configured_lane_count(configured_lanes)?;
        let mut lane_rows = vec![0_usize; configured_lanes];
        for job in &self.jobs {
            let lane_count = job.source_count_tags.len();
            if job.physical_lanes.len() != lane_count
                || lane_count == 0
                || lane_count > job.sources.len()
            {
                return Err("dense MSM lane assignment shape is invalid".to_owned());
            }
            let mut used = vec![false; configured_lanes];
            for (logical_lane, physical_lane) in job.physical_lanes.iter().copied().enumerate() {
                if physical_lane >= configured_lanes {
                    return Err(format!(
                        "dense MSM physical lane index {physical_lane} exceeds configured lane count {configured_lanes}"
                    ));
                }
                if used[physical_lane] {
                    return Err("dense MSM physical lane assignment is invalid".to_owned());
                }
                used[physical_lane] = true;
                let job_rows = dense_shard_rows(job.sources.len(), lane_count, logical_lane)?;
                lane_rows[physical_lane] = lane_rows[physical_lane]
                    .checked_add(job_rows)
                    .ok_or_else(|| "dense MSM row count overflow".to_owned())?;
            }
        }
        Ok(lane_rows.into_iter().max().unwrap_or(0))
    }
    /// Return the exact queued-job, source, and row geometry used by the
    /// authenticated composite-circuit capacity check for four lanes.
    pub(super) fn capacity_profile(&self) -> Result<(usize, usize, usize), String> {
        self.capacity_profile_with_lanes(DENSE_LANES)
    }
    /// Return queued-job, source, and row geometry for `configured_lanes` lanes.
    pub(super) fn capacity_profile_with_lanes(
        &self,
        configured_lanes: usize,
    ) -> Result<(usize, usize, usize), String> {
        let sources = self.jobs.iter().try_fold(0_usize, |total, job| {
            total
                .checked_add(job.sources.len())
                .ok_or_else(|| "dense MSM source count overflow".to_owned())
        })?;
        Ok((
            self.jobs.len(),
            sources,
            self.required_rows_with_lanes(configured_lanes)?,
        ))
    }
    /// Reject jobs which exceed the four-lane authenticated usable-row budget.
    pub(super) fn validate_capacity(&self, usable_rows: usize) -> Result<(), String> {
        self.validate_capacity_with_lanes(usable_rows, DENSE_LANES)
    }
    /// Reject jobs which exceed the configured physical-lane or usable-row budget.
    pub(super) fn validate_capacity_with_lanes(
        &self,
        usable_rows: usize,
        configured_lanes: usize,
    ) -> Result<(), String> {
        let required = self.required_rows_with_lanes(configured_lanes)?;
        if required > usable_rows {
            return Err(format!(
                "Paired Pasta dense MSM requires {required} usable rows, exceeding {usable_rows}"
            ));
        }
        Ok(())
    }
    /// Realize all queued dense MSMs after Base synthesis.
    pub(super) fn synthesize(
        &self,
        config: &PastaDenseMsmConfigV1,
        layouter: &mut impl Layouter<Base<C>>,
        copy_manager: &SharedCopyConstraintManager<Base<C>>,
        witness_gen_only: bool,
        usable_rows: usize,
    ) -> Result<(), Error> {
        let configured_lanes = config.lane_count();
        self.validate_capacity_with_lanes(usable_rows, configured_lanes)
            .map_err(|_| Error::Synthesis)?;
        let physical_cells = if witness_gen_only {
            None
        } else {
            // Base synthesis is complete, so the virtual-to-physical map is
            // immutable for this pass. Keep a guard instead of cloning the
            // multi-million-entry map beside the dense trace.
            Some(copy_manager.lock().map_err(|_| Error::Synthesis)?)
        };
        let mut lane_rows = (0..configured_lanes)
            .map(|_| Vec::<RawRow<Base<C>>>::new())
            .collect::<Vec<_>>();
        let mut rings = Vec::<Vec<LaneEndpoint>>::with_capacity(self.jobs.len());
        for (job_index, job) in self.jobs.iter().enumerate() {
            let lane_count = job.source_count_tags.len();
            let mut offset = choose_offset::<C>(&job.sources).map_err(|_| Error::Synthesis)?;
            let mut endpoints = Vec::with_capacity(lane_count);
            for (logical_lane, physical_lane) in job.physical_lanes.iter().copied().enumerate() {
                let (source_start, source_end) =
                    dense_shard_bounds(job.sources.len(), lane_count, logical_lane);
                let rows_for_lane = &mut lane_rows[physical_lane];
                let row_start = rows_for_lane.len();
                let (mut rows, terminal) = build_job_lane_rows::<C>(
                    job,
                    job_index,
                    logical_lane,
                    source_start,
                    source_end,
                    offset,
                )?;
                endpoints.push(LaneEndpoint {
                    lane: physical_lane,
                    offset_x_row: row_start + OFFSET_X_BRIDGE_ROW,
                    offset_y_row: row_start + OFFSET_Y_BRIDGE_ROW,
                    terminal_x_row: row_start + rows.len() - 2,
                    terminal_y_row: row_start + rows.len() - 1,
                });
                rows_for_lane.append(&mut rows);
                offset = terminal;
            }
            rings.push(endpoints);
        }
        layouter.assign_region(
            || "Paired Pasta dense normalized-GLV MSM",
            |mut region| {
                let mut buses = (0..configured_lanes)
                    .map(|_| Vec::<Cell>::new())
                    .collect::<Vec<_>>();
                let schedule_rows = lane_rows.iter().map(Vec::len).max().unwrap_or(0);
                for row_index in 0..schedule_rows {
                    region.assign_fixed(
                        config.packed_schedule,
                        row_index,
                        packed_enable_tag_at(&lane_rows, row_index)?,
                    );
                }
                for (lane, rows) in lane_rows.iter().enumerate() {
                    let lane_config = &config.lanes[lane];
                    buses[lane].reserve(rows.len());
                    for (row_index, row) in rows.iter().enumerate() {
                        for column in 0..DENSE_COLUMNS {
                            let value = if self.use_unknown {
                                Value::unknown()
                            } else {
                                Value::known(row.values[column])
                            };
                            let cell = region
                                .assign_advice(lane_config.columns[column], row_index, value)
                                .cell();
                            if column == BUS {
                                buses[lane].push(cell);
                            }
                        }
                    }
                }
                if let Some(physical_cells) = &physical_cells {
                    for (lane, rows) in lane_rows.iter().enumerate() {
                        for (row_index, row) in rows.iter().enumerate() {
                            let Some(binding) = row.binding else {
                                continue;
                            };
                            let virtual_value = match binding {
                                BusBinding::Start { job } => self.jobs[job].start_tag,
                                BusBinding::SourceCount { job, lane } => {
                                    self.jobs[job].source_count_tags[lane]
                                }
                                BusBinding::SourceX { job, source } => {
                                    self.jobs[job].sources[source].r_x
                                }
                                BusBinding::SourceY { job, source } => {
                                    self.jobs[job].sources[source].r_y
                                }
                                BusBinding::Segment {
                                    job,
                                    source,
                                    segment,
                                } => self.jobs[job].sources[source].paired_segments[segment],
                            };
                            bind_virtual(
                                &mut region,
                                buses[lane][row_index],
                                virtual_value,
                                &physical_cells.assigned_advices,
                            )?;
                        }
                    }
                }
                for endpoints in &rings {
                    for (index, endpoint) in endpoints.iter().enumerate() {
                        let next = endpoints[(index + 1) % endpoints.len()];
                        region.constrain_equal(
                            buses[endpoint.lane][endpoint.terminal_x_row],
                            buses[next.lane][next.offset_x_row],
                        );
                        region.constrain_equal(
                            buses[endpoint.lane][endpoint.terminal_y_row],
                            buses[next.lane][next.offset_y_row],
                        );
                    }
                }
                Ok(())
            },
        )
    }
}
#[derive(Clone, Copy, Debug)]
struct LaneEndpoint {
    lane: usize,
    offset_x_row: usize,
    offset_y_row: usize,
    terminal_x_row: usize,
    terminal_y_row: usize,
}
#[derive(Clone, Copy, Debug)]
enum BusBinding {
    Start {
        job: usize,
    },
    SourceCount {
        job: usize,
        lane: usize,
    },
    SourceX {
        job: usize,
        source: usize,
    },
    SourceY {
        job: usize,
        source: usize,
    },
    Segment {
        job: usize,
        source: usize,
        segment: usize,
    },
}
#[derive(Clone, Debug)]
struct RawRow<F: PrimeField> {
    values: [F; DENSE_COLUMNS],
    binding: Option<BusBinding>,
    enable_tag: u64,
}
fn packed_enable_tag_at<F: PrimeField>(
    lane_rows: &[Vec<RawRow<F>>],
    row_index: usize,
) -> Result<F, Error> {
    if lane_rows.len() > DENSE_LANES {
        return Err(Error::Synthesis);
    }
    let mut packed = F::ZERO;
    let mut radix = F::ONE;
    for rows in lane_rows {
        let tag = rows.get(row_index).map_or(0, |row| row.enable_tag);
        if tag > 2 {
            return Err(Error::Synthesis);
        }
        packed += radix * F::from(tag);
        radix *= F::from(PACKED_TAG_RADIX);
    }
    Ok(packed)
}
fn constrain_source<C>(
    ctx: &mut Context<Base<C>>,
    scalar_chip: &FpChip<'_, Base<C>, Scalar<C>>,
    source: &PastaDenseMsmSourceV1<C>,
) -> Result<ConstrainedDenseSource<C>, String>
where
    C: CurveAffineExt,
    Base<C>: BigPrimeField + WithSmallOrderMulGroup<3>,
    Scalar<C>: BigPrimeField + WithSmallOrderMulGroup<3>,
{
    if bool::from(source.point.is_identity()) {
        return Err("identity is not a dense MSM source".to_owned());
    }
    let (host_x, host_y) = source.point.into_coordinates();
    if *source.x.value() != host_x || *source.y.value() != host_y {
        return Err("host point does not match its Base coordinate cells".to_owned());
    }
    if !ctx.witness_gen_only()
        && (source.x.cell.is_none()
            || source.y.cell.is_none()
            || source
                .coefficient
                .limbs()
                .iter()
                .any(|limb| limb.cell.is_none()))
    {
        return Err("keygen dense MSM source is missing a Base virtual-cell identity".to_owned());
    }
    let scalar_integer = source.coefficient.value();
    if scalar_integer >= modulus::<Scalar<C>>() {
        return Err("dense MSM coefficient is not a canonical scalar".to_owned());
    }
    let scalar = biguint_to_fe::<Scalar<C>>(&scalar_integer);
    let decomposition = decompose_pasta_scalar::<C>(&scalar)?;
    let normalized = normalize_decomposition::<C>(decomposition)?;
    let gate = scalar_chip.gate();
    // Reassert the source curve equation at this API boundary.  This keeps the
    // dense module sound even if integration stops using EccChip assignment.
    let x_squared = gate.mul(ctx, Existing(source.x), Existing(source.x));
    let x_cubed = gate.mul(ctx, Existing(x_squared), Existing(source.x));
    let rhs = gate.add(ctx, Existing(x_cubed), Constant(Base::<C>::from(5)));
    let y_squared = gate.mul(ctx, Existing(source.y), Existing(source.y));
    ctx.constrain_equal(&y_squared, &rhs);
    let sign = ctx.load_witness(Base::<C>::from(normalized.negative as u64));
    let opposite = ctx.load_witness(Base::<C>::from(normalized.opposite as u64));
    gate.assert_bit(ctx, sign);
    gate.assert_bit(ctx, opposite);
    let segment_values = [
        scalar_segments(normalized.v1),
        scalar_segments(normalized.v2),
    ];
    let segments = segment_values
        .map(|values| values.map(|value| ctx.load_witness(Base::<C>::from(u64::from(value)))));
    let v1 = load_segmented_scalar(ctx, scalar_chip, normalized.v1, &segments[0]);
    let v2 = load_segmented_scalar(ctx, scalar_chip, normalized.v2, &segments[1]);
    let paired_segments = std::array::from_fn(|segment| {
        gate.mul_add(
            ctx,
            Existing(segments[1][segment]),
            Constant(pow2::<Base<C>>(LIMB_BITS)),
            Existing(segments[0][segment]),
        )
    });
    let zeta = scalar_chip.load_constant(ctx, Scalar::<C>::ZETA);
    let zeta_v2_unreduced = scalar_chip.mul_no_carry(ctx, v2.clone(), zeta);
    let zeta_v2 = scalar_chip.carry_mod(ctx, zeta_v2_unreduced);
    let combined_unreduced = scalar_chip.add_no_carry(ctx, v1, zeta_v2);
    let combined = scalar_chip.carry_mod(ctx, combined_unreduced);
    let zeta_squared = scalar_chip.load_constant(ctx, Scalar::<C>::ZETA.square());
    let opposite_unreduced = scalar_chip.mul_no_carry(ctx, combined.clone(), zeta_squared);
    let opposite_value = scalar_chip.carry_mod(ctx, opposite_unreduced);
    let unsigned = scalar_chip.select(ctx, opposite_value, combined, opposite);
    let negative = scalar_chip.negate(ctx, unsigned.clone());
    let signed = scalar_chip.select(ctx, negative, unsigned, sign);
    scalar_chip.assert_equal(ctx, source.coefficient.clone(), signed);
    let beta_squared_x = gate.mul(ctx, Existing(source.x), Constant(Base::<C>::ZETA.square()));
    let r_x = <GateChip<Base<C>> as GateInstructions<Base<C>>>::select(
        gate,
        ctx,
        Existing(beta_squared_x),
        Existing(source.x),
        Existing(opposite),
    );
    let negative_y = gate.neg(ctx, Existing(source.y));
    let r_y = <GateChip<Base<C>> as GateInstructions<Base<C>>>::select(
        gate,
        ctx,
        Existing(negative_y),
        Existing(source.y),
        Existing(sign),
    );
    let mut r = source.point.to_curve();
    if normalized.opposite {
        r = r.endo().endo();
    }
    if normalized.negative {
        r = -r;
    }
    let r = r.to_affine();
    let (expected_r_x, expected_r_y) = r.into_coordinates();
    debug_assert_eq!(*r_x.value(), expected_r_x);
    debug_assert_eq!(*r_y.value(), expected_r_y);
    Ok(ConstrainedDenseSource {
        r,
        r_x,
        r_y,
        paired_segments,
        bits: [scalar_bits(normalized.v1), scalar_bits(normalized.v2)],
    })
}
fn load_segmented_scalar<F, S>(
    ctx: &mut Context<F>,
    scalar_chip: &FpChip<'_, F, S>,
    value: u128,
    segments: &[AssignedValue<F>; SEGMENTS_PER_SCALAR],
) -> ProperCrtUint<F>
where
    F: BigPrimeField,
    S: BigPrimeField,
{
    let assigned = scalar_chip.load_private(ctx, S::from_u128(value));
    let low = scalar_chip.gate().inner_product(
        ctx,
        segments[..13].iter().copied().map(Existing),
        (0..13).map(|index| Constant(pow2::<F>(SEGMENT_BITS * index))),
    );
    let high = scalar_chip.gate().inner_product(
        ctx,
        segments[13..].iter().copied().map(Existing),
        (0..6).map(|index| Constant(pow2::<F>(SEGMENT_BITS * index))),
    );
    ctx.constrain_equal(&low, &assigned.limbs()[0]);
    ctx.constrain_equal(&high, &assigned.limbs()[1]);
    scalar_chip
        .gate()
        .assert_is_const(ctx, &assigned.limbs()[2], &F::ZERO);
    assigned
}
fn pow2<F: PrimeField>(exponent: usize) -> F {
    (0..exponent).fold(F::ONE, |value, _| value + value)
}
fn scalar_segments(value: u128) -> [u8; SEGMENTS_PER_SCALAR] {
    std::array::from_fn(|segment| {
        let (start, width) = segment_spec(segment);
        ((value >> start) & ((1_u128 << width) - 1)) as u8
    })
}
fn scalar_bits(value: u128) -> [bool; GLV_BITS] {
    std::array::from_fn(|bit| ((value >> bit) & 1) == 1)
}
fn segment_spec(segment: usize) -> (usize, usize) {
    match segment {
        0..=11 => (SEGMENT_BITS * segment, SEGMENT_BITS),
        12 => (84, 2),
        13..=18 => (LIMB_BITS + SEGMENT_BITS * (segment - 13), SEGMENT_BITS),
        _ => unreachable!("normalized GLV scalar has exactly nineteen segments"),
    }
}
#[derive(Clone, Copy, Debug)]
struct ScalarDecomposition {
    a1: u128,
    k1_negative: bool,
    a2: u128,
    k2_negative_flag: bool,
}
#[derive(Clone, Copy, Debug)]
struct NormalizedDecomposition {
    v1: u128,
    v2: u128,
    negative: bool,
    opposite: bool,
}
fn normalize_decomposition<C>(
    decomposition: ScalarDecomposition,
) -> Result<NormalizedDecomposition, String>
where
    C: CurveAffineExt,
{
    // halo2curves' second Boolean names the sign of the internal k2.
    // In the public relation it maps to a positive ζ coefficient when set.
    let opposite = decomposition.k1_negative == decomposition.k2_negative_flag;
    let (v1, v2) = if opposite {
        (
            decomposition.a2,
            decomposition
                .a1
                .checked_add(decomposition.a2)
                .ok_or_else(|| "opposite-sign Pasta GLV sum overflowed u128".to_owned())?,
        )
    } else {
        (decomposition.a1, decomposition.a2)
    };
    let strict_bound = match <C::Curve as CurveExt>::CURVE_ID {
        "vesta" => FP_NORMALIZED_SUM_STRICT_BOUND,
        "pallas" => FQ_NORMALIZED_SUM_STRICT_BOUND,
        _ => return Err("dense MSM supports only the Pasta cycle".to_owned()),
    };
    if v1 >= strict_bound || v2 >= strict_bound {
        return Err("normalized Pasta GLV magnitude exceeds its audited bound".to_owned());
    }
    Ok(NormalizedDecomposition {
        v1,
        v2,
        negative: decomposition.k1_negative,
        opposite,
    })
}
fn decompose_pasta_scalar<C>(scalar: &Scalar<C>) -> Result<ScalarDecomposition, String>
where
    C: CurveAffineExt,
    Scalar<C>: BigPrimeField + WithSmallOrderMulGroup<3>,
{
    let params = match <C::Curve as CurveExt>::CURVE_ID {
        "vesta" => ENDO_PARAMS_EQ,
        "pallas" => ENDO_PARAMS_EP,
        _ => return Err("dense MSM supports only the Pasta cycle".to_owned()),
    };
    let input = field_limbs(scalar)?;
    let c1 = mul_512(params.gamma2, input);
    let c2 = mul_512(params.gamma1, input);
    let q1 = mul_512([c1[4], c1[5], c1[6], c1[7]], params.b1);
    let q2 = mul_512([c2[4], c2[5], c2[6], c2[7]], params.b2);
    let q1 = field_from_limbs::<Scalar<C>>([q1[0], q1[1], q1[2], q1[3]])?;
    let q2 = field_from_limbs::<Scalar<C>>([q2[0], q2[1], q2[2], q2[3]])?;
    let k2 = q2 - q1;
    let k1 = *scalar + k2 * Scalar::<C>::ZETA;
    let k1_negative = field_is_negative(&k1)?;
    let k2_negative_flag = field_is_negative(&k2)?;
    let k1_magnitude = if k1_negative { -k1 } else { k1 };
    let k2_magnitude = if k2_negative_flag { -k2 } else { k2 };
    let decomposition = ScalarDecomposition {
        a1: field_lower_u128(&k1_magnitude)?,
        k1_negative,
        a2: field_lower_u128(&k2_magnitude)?,
        k2_negative_flag,
    };
    debug_assert_eq!(*scalar, decomposition_relation::<Scalar<C>>(decomposition));
    Ok(decomposition)
}
fn decomposition_relation<S>(decomposition: ScalarDecomposition) -> S
where
    S: BigPrimeField + WithSmallOrderMulGroup<3>,
{
    let first = S::from_u128(decomposition.a1);
    let first = if decomposition.k1_negative {
        -first
    } else {
        first
    };
    let second = S::ZETA * S::from_u128(decomposition.a2);
    let second = if decomposition.k2_negative_flag {
        second
    } else {
        -second
    };
    first + second
}
fn field_limbs<F: PrimeField>(value: &F) -> Result<[u64; 4], String> {
    let repr = value.to_repr();
    let bytes = repr.as_ref();
    if bytes.len() != 32 {
        return Err("Pasta scalar representation is not 32 bytes".to_owned());
    }
    Ok(std::array::from_fn(|index| {
        let start = index * 8;
        u64::from_le_bytes(
            bytes[start..start + 8]
                .try_into()
                .expect("fixed eight-byte limb"),
        )
    }))
}
fn field_from_limbs<F: PrimeField>(limbs: [u64; 4]) -> Result<F, String> {
    let mut repr = F::Repr::default();
    let bytes = repr.as_mut();
    if bytes.len() != 32 {
        return Err("Pasta scalar representation is not 32 bytes".to_owned());
    }
    for (index, limb) in limbs.into_iter().enumerate() {
        bytes[index * 8..(index + 1) * 8].copy_from_slice(&limb.to_le_bytes());
    }
    Option::<F>::from(F::from_repr(repr))
        .ok_or_else(|| "GLV quotient limb is not a canonical Pasta scalar".to_owned())
}
fn field_lower_u128<F: PrimeField>(value: &F) -> Result<u128, String> {
    let limbs = field_limbs(value)?;
    if limbs[2] != 0 || limbs[3] != 0 {
        return Err("Pasta GLV magnitude exceeds 128 bits".to_owned());
    }
    Ok(u128::from(limbs[0]) | (u128::from(limbs[1]) << 64))
}
fn field_is_negative<F: PrimeField>(value: &F) -> Result<bool, String> {
    let limbs = field_limbs(value)?;
    let (_, borrow) = subtract_with_borrow(u64::MAX, limbs[0], 0);
    let (_, borrow) = subtract_with_borrow(u64::MAX, limbs[1], borrow);
    let (_, borrow) = subtract_with_borrow(u64::MAX, limbs[2], borrow);
    let (_, borrow) = subtract_with_borrow(0, limbs[3], borrow);
    Ok(borrow & 1 != 0)
}
fn subtract_with_borrow(a: u64, b: u64, borrow: u64) -> (u64, u64) {
    let result = (a as u128).wrapping_sub((b as u128) + ((borrow >> 63) as u128));
    (result as u64, (result >> 64) as u64)
}
fn multiply_add(a: u64, b: u64, c: u64, carry: u64) -> (u64, u64) {
    let result = (a as u128) + (b as u128) * (c as u128) + (carry as u128);
    (result as u64, (result >> 64) as u64)
}
fn multiply_add_no_carry(a: u64, b: u64, c: u64) -> (u64, u64) {
    let result = (a as u128) + (b as u128) * (c as u128);
    (result as u64, (result >> 64) as u64)
}
fn mul_512(a: [u64; 4], b: [u64; 4]) -> [u64; 8] {
    let (r0, carry) = multiply_add_no_carry(0, a[0], b[0]);
    let (r1, carry) = multiply_add_no_carry(carry, a[0], b[1]);
    let (r2, carry) = multiply_add_no_carry(carry, a[0], b[2]);
    let (r3, carry_out) = multiply_add_no_carry(carry, a[0], b[3]);
    let (r1, carry) = multiply_add_no_carry(r1, a[1], b[0]);
    let (r2, carry) = multiply_add(r2, a[1], b[1], carry);
    let (r3, carry) = multiply_add(r3, a[1], b[2], carry);
    let (r4, carry_out) = multiply_add(carry_out, a[1], b[3], carry);
    let (r2, carry) = multiply_add_no_carry(r2, a[2], b[0]);
    let (r3, carry) = multiply_add(r3, a[2], b[1], carry);
    let (r4, carry) = multiply_add(r4, a[2], b[2], carry);
    let (r5, carry_out) = multiply_add(carry_out, a[2], b[3], carry);
    let (r3, carry) = multiply_add_no_carry(r3, a[3], b[0]);
    let (r4, carry) = multiply_add(r4, a[3], b[1], carry);
    let (r5, carry) = multiply_add(r5, a[3], b[2], carry);
    let (r6, r7) = multiply_add(carry_out, a[3], b[3], carry);
    [r0, r1, r2, r3, r4, r5, r6, r7]
}
fn choose_offset<C>(sources: &[ConstrainedDenseSource<C>]) -> Result<C::Curve, String>
where
    C: CurveAffineExt,
    Base<C>: BigPrimeField,
    Scalar<C>: BigPrimeField,
{
    for retry in 1..=OFFSET_RETRIES {
        let offset = C::Curve::generator() * Scalar::<C>::from(retry);
        let mut accumulator = offset;
        let mut valid = true;
        for source in sources {
            let mut running_source = source.r.to_curve();
            for bit in 0..GLV_BITS {
                let Some(addend) = joint_digit_point::<C>(
                    &running_source,
                    source.bits[0][bit],
                    source.bits[1][bit],
                ) else {
                    running_source = running_source.double();
                    continue;
                };
                let (accumulator_x, _) = affine_coordinates::<C>(&accumulator)?;
                let (addend_x, _) = affine_coordinates::<C>(&addend)?;
                if accumulator_x == addend_x {
                    valid = false;
                    break;
                }
                accumulator += addend;
                running_source = running_source.double();
            }
            if !valid {
                break;
            }
        }
        if valid {
            return Ok(offset);
        }
    }
    Err(format!(
        "failed to find a complete affine offset in {OFFSET_RETRIES} deterministic retries"
    ))
}
fn joint_digit_point<C>(source: &C::Curve, bit_1: bool, bit_2: bool) -> Option<C::Curve>
where
    C: CurveAffineExt,
    Base<C>: BigPrimeField,
{
    match (bit_1, bit_2) {
        (false, false) => None,
        (true, false) => Some(*source),
        (false, true) => Some(source.endo()),
        (true, true) => Some(-source.endo().endo()),
    }
}
fn affine_coordinates<C>(point: &C::Curve) -> Result<(Base<C>, Base<C>), String>
where
    C: CurveAffineExt,
{
    let affine = point.to_affine();
    if bool::from(affine.is_identity()) {
        return Err("dense MSM affine trace reached identity".to_owned());
    }
    Ok(affine.into_coordinates())
}
#[derive(Clone, Copy, Debug)]
struct MachineWitness<F: PrimeField> {
    acc_x: F,
    acc_y: F,
    offset_x: F,
    offset_y: F,
    source_x: F,
    source_y: F,
    part_1: F,
    part_2: F,
    remaining_bits: F,
    remaining_segments: F,
    remaining_sources: F,
}
impl<F: PrimeField> Default for MachineWitness<F> {
    fn default() -> Self {
        Self {
            acc_x: F::ZERO,
            acc_y: F::ZERO,
            offset_x: F::ZERO,
            offset_y: F::ZERO,
            source_x: F::ZERO,
            source_y: F::ZERO,
            part_1: F::ZERO,
            part_2: F::ZERO,
            remaining_bits: F::ZERO,
            remaining_segments: F::ZERO,
            remaining_sources: F::ZERO,
        }
    }
}
fn raw_row<F: PrimeField>(
    state: MachineWitness<F>,
    bus: F,
    mode: Option<usize>,
    binding: Option<BusBinding>,
) -> RawRow<F> {
    let mut values = [F::ZERO; DENSE_COLUMNS];
    values[BUS] = bus;
    if let Some(mode) = mode {
        values[mode] = F::ONE;
    }
    values[ACC_X] = state.acc_x;
    values[ACC_Y] = state.acc_y;
    values[OFFSET_X] = state.offset_x;
    values[OFFSET_Y] = state.offset_y;
    values[SOURCE_X] = state.source_x;
    values[SOURCE_Y] = state.source_y;
    values[PART_1] = state.part_1;
    values[PART_2] = state.part_2;
    values[REMAINING_BITS] = state.remaining_bits;
    values[REMAINING_SEGMENTS] = state.remaining_segments;
    values[REMAINING_SOURCES] = state.remaining_sources;
    let enable_tag = if matches!(binding, Some(BusBinding::Start { .. })) {
        1
    } else if mode.is_some() {
        2
    } else {
        0
    };
    RawRow {
        values,
        binding,
        enable_tag,
    }
}
fn indicator_witness<F: PrimeField>(value: F, expected: F) -> Result<(F, F), Error> {
    let difference = value - expected;
    if difference == F::ZERO {
        Ok((F::ONE, F::ZERO))
    } else {
        let inverse = Option::<F>::from(difference.invert()).ok_or(Error::Synthesis)?;
        Ok((F::ZERO, inverse))
    }
}
fn segment_integer<C>(source: &ConstrainedDenseSource<C>, scalar: usize, segment: usize) -> u64
where
    C: CurveAffineExt,
    Base<C>: BigPrimeField,
{
    let (bit_start, width) = segment_spec(segment);
    (0..width).fold(0_u64, |value, bit| {
        value | (u64::from(source.bits[scalar][bit_start + bit]) << bit)
    })
}
fn build_job_lane_rows<C>(
    job: &DenseMsmJob<C>,
    job_index: usize,
    lane: usize,
    source_start: usize,
    source_end: usize,
    offset: C::Curve,
) -> Result<(Vec<RawRow<Base<C>>>, C::Curve), Error>
where
    C: CurveAffineExt,
    Base<C>: BigPrimeField + WithSmallOrderMulGroup<3>,
    Scalar<C>: BigPrimeField,
{
    debug_assert!(source_start < source_end && source_end <= job.sources.len());
    let (offset_x, offset_y) = affine_coordinates::<C>(&offset).map_err(|_| Error::Synthesis)?;
    let mut rows = Vec::new();
    let mut state = MachineWitness::<Base<C>>::default();
    let mut start = raw_row(
        state,
        Base::<C>::ONE,
        None,
        Some(BusBinding::Start { job: job_index }),
    );
    start.values[START] = Base::<C>::ONE;
    start.values[ADD_INVERSE] =
        Option::<Base<C>>::from(offset_y.invert()).ok_or(Error::Synthesis)?;
    rows.push(start);
    state.acc_x = offset_x;
    state.acc_y = offset_y;
    state.offset_x = offset_x;
    state.offset_y = offset_y;
    let source_count = u64::try_from(source_end - source_start).map_err(|_| Error::Synthesis)?;
    rows.push(raw_row(
        state,
        Base::<C>::from(source_count),
        Some(MODE_COUNT),
        Some(BusBinding::SourceCount {
            job: job_index,
            lane,
        }),
    ));
    state.remaining_sources = Base::<C>::from(source_count);
    state.remaining_segments = Base::<C>::from(SEGMENTS_PER_SCALAR as u64);
    let mut accumulator = offset;
    for (source_offset, source) in job.sources[source_start..source_end].iter().enumerate() {
        let source_index = source_start + source_offset;
        let (r_x, r_y) = source.r.into_coordinates();
        rows.push(raw_row(
            state,
            r_x,
            Some(MODE_LOAD_X),
            Some(BusBinding::SourceX {
                job: job_index,
                source: source_index,
            }),
        ));
        state.source_x = r_x;
        state.source_y = Base::<C>::ZERO;
        state.remaining_segments = Base::<C>::from(SEGMENTS_PER_SCALAR as u64);
        let mut load_y = raw_row(
            state,
            r_y,
            Some(MODE_LOAD_Y),
            Some(BusBinding::SourceY {
                job: job_index,
                source: source_index,
            }),
        );
        load_y.values[ADD_INVERSE] =
            Option::<Base<C>>::from(r_y.invert()).ok_or(Error::Synthesis)?;
        rows.push(load_y);
        state.source_y = r_y;
        let mut running_source = source.r.to_curve();
        for segment in 0..SEGMENTS_PER_SCALAR {
            let (bit_start, width) = segment_spec(segment);
            let part_1 = segment_integer(source, 0, segment);
            let part_2 = segment_integer(source, 1, segment);
            state.part_1 = Base::<C>::from(part_1);
            state.part_2 = Base::<C>::from(part_2);
            state.remaining_bits = Base::<C>::from(width as u64);
            for local_bit in 0..width {
                let bit = bit_start + local_bit;
                let bit_1 = source.bits[0][bit];
                let bit_2 = source.bits[1][bit];
                let segment_start = local_bit == 0;
                let segment_binding = segment_start.then_some(BusBinding::Segment {
                    job: job_index,
                    source: source_index,
                    segment,
                });
                let mut operation = raw_row(
                    state,
                    if segment_start {
                        *source.paired_segments[segment].value()
                    } else {
                        Base::<C>::ZERO
                    },
                    Some(MODE_OP),
                    segment_binding,
                );
                operation.values[SEGMENT_START] = Base::<C>::from(segment_start as u64);
                operation.values[BIT_1] = Base::<C>::from(bit_1 as u64);
                operation.values[BIT_2] = Base::<C>::from(bit_2 as u64);
                let (last_bit, last_bit_inverse) =
                    indicator_witness(state.remaining_bits, Base::<C>::ONE)?;
                let (last_segment, last_segment_inverse) =
                    indicator_witness(state.remaining_segments, Base::<C>::ONE)?;
                let (last_source, last_source_inverse) =
                    indicator_witness(state.remaining_sources, Base::<C>::ONE)?;
                operation.values[LAST_BIT] = last_bit;
                operation.values[LAST_BIT_INVERSE] = last_bit_inverse;
                operation.values[LAST_SEGMENT] = last_segment;
                operation.values[LAST_SEGMENT_INVERSE] = last_segment_inverse;
                operation.values[LAST_SOURCE] = last_source;
                operation.values[LAST_SOURCE_INVERSE] = last_source_inverse;
                operation.values[SOURCE_ENDPOINT] = last_bit * last_segment;
                let job_endpoint = last_bit * last_segment * last_source;
                operation.values[JOB_ENDPOINT] = job_endpoint;
                let (short_segment, short_segment_inverse) =
                    indicator_witness(state.remaining_segments, Base::<C>::from(7))?;
                operation.values[SHORT_SEGMENT] = short_segment;
                operation.values[SHORT_SEGMENT_INVERSE] = short_segment_inverse;
                let (source_current_x, source_current_y) =
                    affine_coordinates::<C>(&running_source).map_err(|_| Error::Synthesis)?;
                debug_assert_eq!(state.source_x, source_current_x);
                debug_assert_eq!(state.source_y, source_current_y);
                let double_denominator = source_current_y + source_current_y;
                let double_inverse =
                    Option::<Base<C>>::from(double_denominator.invert()).ok_or(Error::Synthesis)?;
                operation.values[DOUBLE_INVERSE] = double_inverse;
                operation.values[DOUBLE_LAMBDA] =
                    Base::<C>::from(3) * source_current_x.square() * double_inverse;
                if let Some(addend) = joint_digit_point::<C>(&running_source, bit_1, bit_2) {
                    let (accumulator_x, accumulator_y) =
                        affine_coordinates::<C>(&accumulator).map_err(|_| Error::Synthesis)?;
                    let (addend_x, addend_y) =
                        affine_coordinates::<C>(&addend).map_err(|_| Error::Synthesis)?;
                    let delta_x = addend_x - accumulator_x;
                    let add_inverse =
                        Option::<Base<C>>::from(delta_x.invert()).ok_or(Error::Synthesis)?;
                    operation.values[ADD_INVERSE] = add_inverse;
                    operation.values[ADD_LAMBDA] = (addend_y - accumulator_y) * add_inverse;
                    operation.values[DIGIT_ACTIVE] = Base::<C>::ONE;
                    operation.values[DIGIT_X] = addend_x;
                    operation.values[DIGIT_Y] = addend_y;
                    accumulator += addend;
                }
                running_source = running_source.double();
                let (next_source_x, next_source_y) =
                    affine_coordinates::<C>(&running_source).map_err(|_| Error::Synthesis)?;
                state.source_x = next_source_x;
                state.source_y = next_source_y;
                let (next_acc_x, next_acc_y) =
                    affine_coordinates::<C>(&accumulator).map_err(|_| Error::Synthesis)?;
                operation.values[BUS] = if job_endpoint == Base::<C>::ONE {
                    next_acc_x
                } else if segment_start {
                    *source.paired_segments[segment].value()
                } else if last_bit == Base::<C>::ONE {
                    offset_y
                } else {
                    offset_x
                };
                rows.push(operation);
                state.acc_x = next_acc_x;
                state.acc_y = next_acc_y;
                state.part_1 = Base::<C>::from(part_1 >> (local_bit + 1));
                state.part_2 = Base::<C>::from(part_2 >> (local_bit + 1));
                state.remaining_bits = Base::<C>::from((width - local_bit - 1) as u64);
                if local_bit + 1 == width {
                    state.remaining_segments -= Base::<C>::ONE;
                    if segment + 1 == SEGMENTS_PER_SCALAR {
                        state.remaining_sources -= Base::<C>::ONE;
                    }
                }
            }
        }
    }
    // The final operation constrains this otherwise inactive bus cell to the
    // terminal accumulator's y coordinate.
    rows.push(raw_row(state, state.acc_y, None, None));
    debug_assert_eq!(
        rows.len(),
        (source_end - source_start) * ROWS_PER_SOURCE + ROWS_PER_JOB
    );
    Ok((rows, accumulator))
}
#[cfg(test)]
fn build_job_rows<C>(job: &DenseMsmJob<C>, job_index: usize) -> Result<Vec<RawRow<Base<C>>>, Error>
where
    C: CurveAffineExt,
    Base<C>: BigPrimeField + WithSmallOrderMulGroup<3>,
    Scalar<C>: BigPrimeField,
{
    let offset = choose_offset::<C>(&job.sources).map_err(|_| Error::Synthesis)?;
    build_job_lane_rows::<C>(job, job_index, 0, 0, job.sources.len(), offset).map(|(rows, _)| rows)
}
fn bind_virtual<F: BigPrimeField>(
    region: &mut halo2_base::halo2_proofs::circuit::Region<'_, F>,
    raw: Cell,
    virtual_value: AssignedValue<F>,
    physical_cells: &std::collections::HashMap<halo2_base::ContextCell, Cell>,
) -> Result<(), Error> {
    let virtual_cell = virtual_value.cell.ok_or(Error::Synthesis)?;
    let physical = *physical_cells.get(&virtual_cell).ok_or(Error::Synthesis)?;
    region.constrain_equal(raw, physical);
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    use der_parser::num_bigint::BigUint;
    use halo2_base::gates::circuit::{BaseCircuitParams, BaseConfig, builder::BaseCircuitBuilder};
    use halo2_base::halo2_proofs::{
        circuit::{Layouter, SimpleFloorPlanner, V1},
        dev::MockProver,
        halo2curves::pasta::{EpAffine, Eq, EqAffine, Fp, Fq},
        plonk::{Assigned, Circuit},
    };
    fn edge_scalars<F: BigPrimeField>() -> Vec<F> {
        let modulus: BigUint = modulus::<F>();
        let half: BigUint = &modulus >> 1;
        let zero = &modulus - &modulus;
        let one = &zero + 1_u8;
        vec![
            zero,
            one,
            &half - 1_u8,
            half,
            (&modulus + 1_u8) >> 1,
            &modulus - 2_u8,
            &modulus - 1_u8,
        ]
        .into_iter()
        .map(|value| biguint_to_fe::<F>(&value))
        .collect()
    }
    fn assert_decomposition_edges<C>(strict_bound: u128)
    where
        C: CurveAffineExt,
        Scalar<C>: BigPrimeField + WithSmallOrderMulGroup<3>,
    {
        for scalar in edge_scalars::<Scalar<C>>() {
            let decomposition = decompose_pasta_scalar::<C>(&scalar).expect("Pasta decomposition");
            assert_eq!(scalar, decomposition_relation(decomposition));
            let normalized =
                normalize_decomposition::<C>(decomposition).expect("normalized decomposition");
            assert!(normalized.v1 < strict_bound);
            assert!(normalized.v2 < strict_bound);
        }
    }
    #[test]
    fn exact_pasta_bounds_fit_128_bits() {
        assert!(FP_NORMALIZED_SUM_STRICT_BOUND < u128::MAX);
        assert!(FQ_NORMALIZED_SUM_STRICT_BOUND < u128::MAX);
        assert_decomposition_edges::<EqAffine>(FP_NORMALIZED_SUM_STRICT_BOUND);
        assert_decomposition_edges::<EpAffine>(FQ_NORMALIZED_SUM_STRICT_BOUND);
    }
    #[test]
    fn normalized_relation_matches_original_scalar_in_both_modes() {
        for scalar in edge_scalars::<Fp>() {
            let decomposition =
                decompose_pasta_scalar::<EqAffine>(&scalar).expect("Fp decomposition");
            let normalized =
                normalize_decomposition::<EqAffine>(decomposition).expect("Fp normalization");
            let mut relation =
                Fp::from_u128(normalized.v1) + Fp::ZETA * Fp::from_u128(normalized.v2);
            if normalized.opposite {
                relation *= Fp::ZETA.square();
            }
            if normalized.negative {
                relation = -relation;
            }
            assert_eq!(scalar, relation);
        }
        for scalar in edge_scalars::<Fq>() {
            let decomposition =
                decompose_pasta_scalar::<EpAffine>(&scalar).expect("Fq decomposition");
            let normalized =
                normalize_decomposition::<EpAffine>(decomposition).expect("Fq normalization");
            let mut relation =
                Fq::from_u128(normalized.v1) + Fq::ZETA * Fq::from_u128(normalized.v2);
            if normalized.opposite {
                relation *= Fq::ZETA.square();
            }
            if normalized.negative {
                relation = -relation;
            }
            assert_eq!(scalar, relation);
        }
    }
    #[test]
    fn candidate_coordinate_formula_covers_all_joint_digits() {
        let r_x = Fp::from(17);
        let r_y = Fp::from(23);
        for (bit_1, bit_2, expected_x, expected_y) in [
            (0_u64, 0_u64, Fp::ZERO, Fp::ZERO),
            (1, 0, r_x, r_y),
            (0, 1, Fp::ZETA * r_x, r_y),
            (1, 1, Fp::ZETA.square() * r_x, -r_y),
        ] {
            let both = Fp::from(bit_1 * bit_2);
            let q_x = r_x
                * (Fp::from(bit_1)
                    + Fp::ZETA * Fp::from(bit_2)
                    + Fp::from(2) * Fp::ZETA.square() * both);
            let q_y = r_y * (Fp::from(bit_1 + bit_2) - Fp::from(3) * both);
            assert_eq!(q_x, expected_x);
            assert_eq!(q_y, expected_y);
        }
    }
    #[test]
    fn segment_layout_reconstructs_both_86_bit_limbs() {
        let value = 0xf2da_3417_69f0_6933_12f1_e389_3fff_ffff_u128;
        let segments = scalar_segments(value);
        let low = segments[..13]
            .iter()
            .enumerate()
            .fold(0_u128, |sum, (index, segment)| {
                sum + (u128::from(*segment) << (SEGMENT_BITS * index))
            });
        let high = segments[13..]
            .iter()
            .enumerate()
            .fold(0_u128, |sum, (index, segment)| {
                sum + (u128::from(*segment) << (SEGMENT_BITS * index))
            });
        assert_eq!(low, value & ((1_u128 << LIMB_BITS) - 1));
        assert_eq!(high, value >> LIMB_BITS);
    }
    #[test]
    fn configuration_uses_one_fixed_schedule_and_masks_blinding_rows() {
        let mut meta = ConstraintSystem::<Fq>::default();
        let config = PastaDenseMsmConfigV1::configure::<EqAffine>(&mut meta);
        assert_eq!(config.lane_count(), DENSE_LANES);
        assert_eq!(meta.num_advice_columns(), DENSE_LANES * DENSE_COLUMNS);
        assert_eq!(meta.num_fixed_columns(), 1);
        assert_eq!(meta.num_selectors(), 0);
        assert_eq!(meta.permutation().get_columns().len(), DENSE_LANES);
        // The fixed schedule factor on every lane constraint is necessary to
        // mask randomized advice in Halo2's blinding rows.  It adds one degree
        // while removing three fixed columns.
        assert_eq!(meta.degree(), 6);
        assert_eq!(248 * ROWS_PER_SOURCE + 3, 32_243);
    }
    #[test]
    fn configuration_can_allocate_exactly_one_physical_lane() {
        let mut meta = ConstraintSystem::<Fq>::default();
        let config = PastaDenseMsmConfigV1::configure_with_lanes::<EqAffine>(&mut meta, 1);
        assert_eq!(config.lane_count(), 1);
        assert_eq!(meta.num_advice_columns(), DENSE_COLUMNS);
        assert_eq!(meta.num_fixed_columns(), 1);
        assert_eq!(meta.num_selectors(), 0);
        assert_eq!(meta.permutation().get_columns().len(), 1);
        assert_eq!(meta.degree(), 6);
    }
    #[test]
    fn packed_schedule_uses_bounded_base_four_lane_digits() {
        let row = |tag| RawRow {
            values: [Fq::ZERO; DENSE_COLUMNS],
            binding: None,
            enable_tag: tag,
        };
        let lane_rows = vec![vec![row(1)], vec![row(2)], vec![], vec![row(1)]];
        assert_eq!(
            packed_enable_tag_at(&lane_rows, 0).expect("bounded tags pack"),
            Fq::from(1 + 2 * 4 + 4 * 4 * 4)
        );
        assert_eq!(
            packed_enable_tag_at(&lane_rows, 1).expect("missing rows are disabled"),
            Fq::ZERO
        );
        let invalid = vec![vec![row(3)]];
        assert!(packed_enable_tag_at(&invalid, 0).is_err());
    }
    #[test]
    fn k16_lane_geometry_enforces_the_authenticated_capacity() {
        assert_eq!(K16_USABLE_ROWS, 65_527);
        assert_eq!(K16_MAX_SOURCES_PER_LANE, 504);
        assert_eq!(dense_lane_count_with_limit(1_008, 2), Ok(2));
        assert!(dense_lane_count_with_limit(1_009, 2).is_err());
        assert_eq!(dense_lane_count(1_512), Ok(3));
        assert_eq!(dense_lane_count(2_016), Ok(4));
        let two_lane_rows = (0..2)
            .map(|lane| dense_shard_rows(1_008, 2, lane).expect("bounded two-lane shard"))
            .collect::<Vec<_>>();
        assert_eq!(two_lane_rows, vec![65_523, 65_523]);
        assert!(two_lane_rows.iter().all(|rows| *rows <= K16_USABLE_ROWS));
        assert_eq!(
            plan_dense_jobs_with_lanes(&[1_008], 2),
            Ok(vec![vec![0, 1]])
        );
        assert!(plan_dense_jobs_with_lanes(&[1_009], 2).is_err());
        let shard_rows = (0..3)
            .map(|lane| {
                let (start, end) = dense_shard_bounds(1_512, 3, lane);
                (end - start) * ROWS_PER_SOURCE + ROWS_PER_JOB
            })
            .collect::<Vec<_>>();
        assert_eq!(shard_rows, vec![65_523, 65_523, 65_523]);
        assert!(shard_rows.into_iter().all(|rows| rows <= K16_USABLE_ROWS));
        assert!(dense_lane_count(2_017).is_err());
        let point = Eq::generator().to_affine();
        let jobs = PastaDenseMsmJobsV1 {
            jobs: vec![DenseMsmJob {
                start_tag: assigned(Fq::ONE),
                source_count_tags: vec![
                    assigned(Fq::from(504)),
                    assigned(Fq::from(504)),
                    assigned(Fq::from(504)),
                ],
                physical_lanes: vec![0, 1, 2],
                sources: vec![unit_scalar_source(point); 1_512],
            }],
            use_unknown: false,
        };
        assert_eq!(jobs.capacity_profile(), Ok((1, 1_512, 65_523)));
    }
    fn assert_dense_plan_fits(source_counts: &[usize], assignments: &[Vec<usize>]) {
        assert_eq!(assignments.len(), source_counts.len());
        let mut lane_rows = [0_usize; DENSE_LANES];
        for (source_count, assignment) in source_counts.iter().copied().zip(assignments) {
            let lane_count = assignment.len();
            assert!(
                (dense_lane_count(source_count).expect("bounded source count")
                    ..=DENSE_LANES.min(source_count))
                    .contains(&lane_count)
            );
            let mut used = [false; DENSE_LANES];
            for (logical_lane, physical_lane) in assignment.iter().copied().enumerate() {
                assert!(physical_lane < DENSE_LANES);
                assert!(!used[physical_lane]);
                used[physical_lane] = true;
                lane_rows[physical_lane] +=
                    dense_shard_rows(source_count, lane_count, logical_lane)
                        .expect("bounded shard rows");
            }
        }
        assert!(lane_rows.into_iter().all(|rows| rows <= K16_USABLE_ROWS));
    }
    #[test]
    fn global_lane_plan_fixes_the_online_greedy_counterexample() {
        let source_counts = [50, 100, 100, 100, 50, 450];
        let assignments = plan_dense_jobs(&source_counts).expect("global packing exists");
        assert_dense_plan_fits(&source_counts, &assignments);
        assert_eq!(
            assignments,
            plan_dense_jobs(&source_counts).expect("global packing is deterministic")
        );
    }
    #[test]
    fn global_lane_plan_can_split_the_fixed_shard_counterexample() {
        let source_counts = [303; 5];
        let assignments = plan_dense_jobs(&source_counts).expect("one job can be split");
        assert_dense_plan_fits(&source_counts, &assignments);
        assert!(assignments.iter().any(|assignment| assignment.len() > 1));
    }
    #[test]
    fn global_lane_plan_rejects_an_impossible_shard_count() {
        // Every nonempty logical shard costs at least 133 rows, so four lanes
        // can hold at most 1,968 independent one-source jobs.
        let source_counts = vec![1; 1_969];
        assert!(plan_dense_jobs(&source_counts).is_err());
    }
    #[test]
    fn global_lane_plan_accepts_the_exact_source_boundary() {
        let source_counts = [2_016];
        let assignments = plan_dense_jobs(&source_counts).expect("four exact shards fit");
        assert_dense_plan_fits(&source_counts, &assignments);
        assert_eq!(assignments[0].len(), DENSE_LANES);
        assert!(plan_dense_jobs(&[2_017]).is_err());
    }
    #[test]
    fn independent_jobs_can_use_disjoint_physical_lanes() {
        let point = Eq::generator().to_affine();
        let jobs = PastaDenseMsmJobsV1 {
            jobs: vec![
                DenseMsmJob {
                    start_tag: assigned(Fq::ONE),
                    source_count_tags: vec![assigned(Fq::from(322))],
                    physical_lanes: vec![0],
                    sources: vec![unit_scalar_source(point); 322],
                },
                DenseMsmJob {
                    start_tag: assigned(Fq::ONE),
                    source_count_tags: vec![
                        assigned(Fq::from(430)),
                        assigned(Fq::from(430)),
                        assigned(Fq::from(430)),
                    ],
                    physical_lanes: vec![1, 2, 3],
                    sources: vec![unit_scalar_source(point); 1_290],
                },
            ],
            use_unknown: false,
        };
        assert_eq!(jobs.capacity_profile(), Ok((2, 1_612, 55_903)));
        assert!(jobs.validate_capacity(K16_USABLE_ROWS).is_ok());
    }
    #[test]
    fn one_lane_capacity_rejects_a_preassigned_other_lane() {
        let point = Eq::generator().to_affine();
        let jobs = PastaDenseMsmJobsV1 {
            jobs: vec![DenseMsmJob {
                start_tag: assigned(Fq::ONE),
                source_count_tags: vec![assigned(Fq::from(2))],
                physical_lanes: vec![0],
                sources: vec![unit_scalar_source(point); 2],
            }],
            use_unknown: false,
        };
        let expected_rows = 2 * ROWS_PER_SOURCE + ROWS_PER_JOB;
        assert_eq!(jobs.required_rows_with_lanes(1), Ok(expected_rows));
        assert_eq!(
            jobs.capacity_profile_with_lanes(1),
            Ok((1, 2, expected_rows))
        );
        assert!(jobs.validate_capacity_with_lanes(expected_rows, 1).is_ok());
        assert!(
            jobs.validate_capacity_with_lanes(expected_rows - 1, 1)
                .is_err()
        );

        let mut wrong_lane = jobs.clone();
        wrong_lane.jobs[0].physical_lanes[0] = 1;
        let error = wrong_lane
            .required_rows_with_lanes(1)
            .expect_err("lane one must not fit a one-lane configuration");
        assert!(error.contains("lane index 1 exceeds configured lane count 1"));
        assert_eq!(wrong_lane.required_rows(), Ok(expected_rows));
        assert!(jobs.required_rows_with_lanes(0).is_err());
        assert!(jobs.required_rows_with_lanes(DENSE_LANES + 1).is_err());
    }
    const QUEUE_TEST_K: u32 = 9;
    const QUEUE_TEST_UNUSABLE_ROWS: usize = 9;
    #[derive(Clone, Debug)]
    struct DenseQueueConfig<F: halo2_base::utils::ScalarField> {
        base: BaseConfig<F>,
        dense: PastaDenseMsmConfigV1,
    }
    #[derive(Clone)]
    struct DenseQueueCircuit<C>
    where
        C: CurveAffineExt,
        Base<C>: BigPrimeField,
    {
        builder: BaseCircuitBuilder<Base<C>>,
        jobs: PastaDenseMsmJobsV1<C>,
    }
    impl<C> Circuit<Base<C>> for DenseQueueCircuit<C>
    where
        C: CurveAffineExt,
        Base<C>: BigPrimeField + WithSmallOrderMulGroup<3>,
        Scalar<C>: BigPrimeField + WithSmallOrderMulGroup<3>,
    {
        type Config = DenseQueueConfig<Base<C>>;
        type FloorPlanner = V1;
        type Params = BaseCircuitParams;
        fn params(&self) -> Self::Params {
            self.builder.config_params.clone()
        }
        fn without_witnesses(&self) -> Self {
            Self {
                builder: self.builder.deep_clone().unknown(true),
                jobs: self.jobs.unknown(),
            }
        }
        fn configure_with_params(
            meta: &mut ConstraintSystem<Base<C>>,
            params: Self::Params,
        ) -> Self::Config {
            let usable_rows = (1_usize << params.k) - QUEUE_TEST_UNUSABLE_ROWS;
            let mut base = BaseConfig::configure(meta, params);
            base.set_usable_rows(usable_rows);
            DenseQueueConfig {
                base,
                dense: PastaDenseMsmConfigV1::configure::<C>(meta),
            }
        }
        fn configure(_: &mut ConstraintSystem<Base<C>>) -> Self::Config {
            unreachable!("dense queue test uses parameterized Base config")
        }
        fn synthesize(
            &self,
            config: Self::Config,
            mut layouter: impl Layouter<Base<C>>,
        ) -> Result<(), Error> {
            <BaseCircuitBuilder<Base<C>> as Circuit<Base<C>>>::synthesize(
                &self.builder,
                config.base,
                layouter.namespace(|| "dense queue Base"),
            )?;
            self.jobs.synthesize(
                &config.dense,
                &mut layouter,
                &self.builder.core().copy_manager,
                self.builder.witness_gen_only(),
                (1_usize << QUEUE_TEST_K) - QUEUE_TEST_UNUSABLE_ROWS,
            )
        }
    }
    fn queue_source<C>(
        ctx: &mut Context<Base<C>>,
        scalar_chip: &FpChip<'_, Base<C>, Scalar<C>>,
        point: C,
    ) -> PastaDenseMsmSourceV1<C>
    where
        C: CurveAffineExt,
        Base<C>: BigPrimeField,
        Scalar<C>: BigPrimeField,
    {
        let (x, y) = point.into_coordinates();
        let coefficient = scalar_chip.load_private(ctx, Scalar::<C>::ONE);
        let coefficient: ProperCrtUint<Base<C>> =
            scalar_chip.enforce_less_than(ctx, coefficient).into();
        PastaDenseMsmSourceV1 {
            point,
            x: ctx.load_witness(x),
            y: ctx.load_witness(y),
            coefficient,
        }
    }
    fn dense_queue_circuit<C>(tamper_copy_binding: bool) -> DenseQueueCircuit<C>
    where
        C: CurveAffineExt,
        Base<C>: BigPrimeField + WithSmallOrderMulGroup<3>,
        Scalar<C>: BigPrimeField + WithSmallOrderMulGroup<3>,
    {
        let mut builder = BaseCircuitBuilder::<Base<C>>::new(false)
            .use_k(QUEUE_TEST_K as usize)
            .use_lookup_bits(8);
        let range = builder.range_chip();
        let scalar_chip = FpChip::<Base<C>, Scalar<C>>::new(&range, LIMB_BITS, 3);
        let point = C::Curve::generator().to_affine();
        let sources = [
            queue_source(builder.main(0), &scalar_chip, point),
            queue_source(builder.main(0), &scalar_chip, -point),
        ];
        let mut jobs = PastaDenseMsmJobsV1::default();
        jobs.queue_constrained(builder.main(0), &scalar_chip, &sources)
            .expect("cancelling dense queue");
        assert_eq!(jobs.required_rows(), Ok(2 * ROWS_PER_SOURCE + 3));
        if tamper_copy_binding {
            let replacement = (C::Curve::generator() * Scalar::<C>::from(2_u64)).to_affine();
            jobs.jobs[0].sources[0].r = replacement;
            jobs.jobs[0].sources[1].r = -replacement;
        }
        builder.calculate_params(Some(QUEUE_TEST_UNUSABLE_ROWS));
        DenseQueueCircuit { builder, jobs }
    }
    fn assert_dense_queue_copy_binding<C>()
    where
        C: CurveAffineExt,
        Base<C>: BigPrimeField + WithSmallOrderMulGroup<3>,
        Scalar<C>: BigPrimeField + WithSmallOrderMulGroup<3>,
    {
        let valid = dense_queue_circuit::<C>(false);
        MockProver::run(QUEUE_TEST_K, &valid, vec![])
            .expect("Base-to-dense queue synthesis")
            .assert_satisfied();
        let tampered = dense_queue_circuit::<C>(true);
        assert!(
            MockProver::run(QUEUE_TEST_K, &tampered, vec![])
                .expect("tampered Base-to-dense queue synthesis")
                .verify()
                .is_err(),
            "changing the dense trace while retaining the Base cells must violate a copy constraint"
        );
    }
    #[test]
    fn eq_base_queue_copy_manager_dense_synthesis_is_bound() {
        assert_dense_queue_copy_binding::<EqAffine>();
    }
    #[test]
    fn ep_base_queue_copy_manager_dense_synthesis_is_bound() {
        assert_dense_queue_copy_binding::<EpAffine>();
    }
    fn assigned(value: Fq) -> AssignedValue<Fq> {
        AssignedValue {
            value: Assigned::Trivial(value),
            cell: None,
        }
    }
    fn unit_scalar_source(point: EqAffine) -> ConstrainedDenseSource<EqAffine> {
        let (x, y) = point.into_coordinates();
        let mut bits = [[false; GLV_BITS]; SCALARS_PER_SOURCE];
        bits[0][0] = true;
        let mut paired_segments = [assigned(Fq::ZERO); SEGMENTS_PER_SCALAR];
        paired_segments[0] = assigned(Fq::ONE);
        ConstrainedDenseSource {
            r: point,
            r_x: assigned(x),
            r_y: assigned(y),
            paired_segments,
            bits,
        }
    }
    #[derive(Clone)]
    struct DenseRowsCircuit {
        lane_rows: Vec<Vec<RawRow<Fq>>>,
    }
    impl Circuit<Fq> for DenseRowsCircuit {
        type Config = PastaDenseMsmConfigV1;
        type FloorPlanner = SimpleFloorPlanner;
        type Params = ();
        fn without_witnesses(&self) -> Self {
            self.clone()
        }
        fn configure(meta: &mut ConstraintSystem<Fq>) -> Self::Config {
            PastaDenseMsmConfigV1::configure::<EqAffine>(meta)
        }
        fn synthesize(
            &self,
            config: Self::Config,
            mut layouter: impl Layouter<Fq>,
        ) -> Result<(), Error> {
            layouter.assign_region(
                || "dense test rows",
                |mut region| {
                    let mut buses = Vec::with_capacity(self.lane_rows.len());
                    let schedule_rows = self.lane_rows.iter().map(Vec::len).max().unwrap_or(0);
                    for row_index in 0..schedule_rows {
                        region.assign_fixed(
                            config.packed_schedule,
                            row_index,
                            packed_enable_tag_at(&self.lane_rows, row_index)?,
                        );
                    }
                    for (lane, rows) in self.lane_rows.iter().enumerate() {
                        let lane_config = &config.lanes[lane];
                        let mut lane_buses = Vec::with_capacity(rows.len());
                        for (row_index, row) in rows.iter().enumerate() {
                            for (column, value) in row.values.iter().copied().enumerate() {
                                let cell = region
                                    .assign_advice(
                                        lane_config.columns[column],
                                        row_index,
                                        Value::known(value),
                                    )
                                    .cell();
                                if column == BUS {
                                    lane_buses.push(cell);
                                }
                            }
                        }
                        buses.push(lane_buses);
                    }
                    for lane in 0..self.lane_rows.len() {
                        let next = (lane + 1) % self.lane_rows.len();
                        let terminal_x = self.lane_rows[lane].len() - 2;
                        let terminal_y = self.lane_rows[lane].len() - 1;
                        region.constrain_equal(
                            buses[lane][terminal_x],
                            buses[next][OFFSET_X_BRIDGE_ROW],
                        );
                        region.constrain_equal(
                            buses[lane][terminal_y],
                            buses[next][OFFSET_Y_BRIDGE_ROW],
                        );
                    }
                    Ok(())
                },
            )
        }
    }
    fn build_test_lane_rows(
        sources: Vec<ConstrainedDenseSource<EqAffine>>,
        lane_count: usize,
    ) -> Vec<Vec<RawRow<Fq>>> {
        let source_count = sources.len();
        let job = DenseMsmJob {
            start_tag: assigned(Fq::ONE),
            source_count_tags: (0..lane_count)
                .map(|lane| {
                    let (start, end) = dense_shard_bounds(source_count, lane_count, lane);
                    assigned(Fq::from((end - start) as u64))
                })
                .collect(),
            physical_lanes: (0..lane_count).collect(),
            sources,
        };
        let mut offset = choose_offset::<EqAffine>(&job.sources).expect("complete global offset");
        (0..lane_count)
            .map(|lane| {
                let (start, end) = dense_shard_bounds(source_count, lane_count, lane);
                let (rows, terminal) =
                    build_job_lane_rows::<EqAffine>(&job, 0, lane, start, end, offset)
                        .expect("complete lane trace");
                offset = terminal;
                rows
            })
            .collect()
    }
    #[test]
    fn accumulator_ring_accepts_a_cross_lane_cancellation() {
        let point = Eq::generator().to_affine();
        let lane_rows = build_test_lane_rows(
            vec![
                unit_scalar_source(point),
                unit_scalar_source(point),
                unit_scalar_source(-point),
                unit_scalar_source(-point),
            ],
            2,
        );
        let prover = MockProver::run(9, &DenseRowsCircuit { lane_rows }, vec![])
            .expect("two-lane mock prover runs");
        prover.assert_satisfied();
    }
    #[test]
    fn accumulator_ring_rejects_a_nonzero_cross_lane_result() {
        let point = Eq::generator().to_affine();
        let lane_rows = build_test_lane_rows(
            vec![
                unit_scalar_source(point),
                unit_scalar_source(point),
                unit_scalar_source(-point),
                unit_scalar_source(point),
            ],
            2,
        );
        let prover = MockProver::run(9, &DenseRowsCircuit { lane_rows }, vec![])
            .expect("two-lane mock prover runs");
        assert!(prover.verify().is_err());
    }
    #[test]
    fn source_major_machine_accepts_a_cancelling_pair() {
        let point = Eq::generator().to_affine();
        let sources = vec![unit_scalar_source(point), unit_scalar_source(-point)];
        let job = DenseMsmJob {
            start_tag: assigned(Fq::ONE),
            source_count_tags: vec![assigned(Fq::from(2))],
            physical_lanes: vec![0],
            sources,
        };
        let rows = build_job_rows::<EqAffine>(&job, 0).expect("complete affine trace");
        assert_eq!(rows.len(), 2 * ROWS_PER_SOURCE + 3);
        let prover = MockProver::run(
            9,
            &DenseRowsCircuit {
                lane_rows: vec![rows],
            },
            vec![],
        )
        .expect("mock prover runs");
        prover.assert_satisfied();
    }
    #[test]
    fn source_major_machine_rejects_a_forged_paired_segment() {
        let point = Eq::generator().to_affine();
        let sources = vec![unit_scalar_source(point), unit_scalar_source(-point)];
        let job = DenseMsmJob {
            start_tag: assigned(Fq::ONE),
            source_count_tags: vec![assigned(Fq::from(2))],
            physical_lanes: vec![0],
            sources,
        };
        let mut rows = build_job_rows::<EqAffine>(&job, 0).expect("complete affine trace");
        // start, count, x, y, then the first packed-segment operation.
        rows[4].values[BUS] += Fq::ONE;
        let prover = MockProver::run(
            9,
            &DenseRowsCircuit {
                lane_rows: vec![rows],
            },
            vec![],
        )
        .expect("mock prover runs");
        assert!(prover.verify().is_err());
    }
    #[test]
    fn source_major_machine_rejects_a_nonzero_result() {
        let point = Eq::generator().to_affine();
        let job = DenseMsmJob {
            start_tag: assigned(Fq::ONE),
            source_count_tags: vec![assigned(Fq::ONE)],
            physical_lanes: vec![0],
            sources: vec![unit_scalar_source(point)],
        };
        let rows = build_job_rows::<EqAffine>(&job, 0).expect("complete affine trace");
        let prover = MockProver::run(
            9,
            &DenseRowsCircuit {
                lane_rows: vec![rows],
            },
            vec![],
        )
        .expect("mock prover runs");
        assert!(prover.verify().is_err());
    }
    #[test]
    fn fixed_start_tag_rejects_a_forged_initial_mode() {
        let point = Eq::generator().to_affine();
        let sources = vec![unit_scalar_source(point), unit_scalar_source(-point)];
        let job = DenseMsmJob {
            start_tag: assigned(Fq::ONE),
            source_count_tags: vec![assigned(Fq::from(2))],
            physical_lanes: vec![0],
            sources,
        };
        let mut rows = build_job_rows::<EqAffine>(&job, 0).expect("complete affine trace");
        rows[0].values[MODE_COUNT] = Fq::ONE;
        rows[0].values[START] = Fq::ZERO;
        let prover = MockProver::run(
            9,
            &DenseRowsCircuit {
                lane_rows: vec![rows],
            },
            vec![],
        )
        .expect("mock prover runs");
        assert!(prover.verify().is_err());
    }
}
