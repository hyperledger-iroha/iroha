//! Non-authorizing five-slice partial-MSM arithmetic for a phone Claim candidate.
//!
//! Each fixed k15 proof binds one fifth of a claimed ordered source inventory to
//! explicit public point/coefficient limbs and proves its endpoint. The source
//! count and cells are not yet authenticated by the original parent proofs.
//! TODO: Bind every slice to one circuit-authenticated global source inventory
//! and challenge, verify the child proofs in an exact contiguous join, and
//! connect that root to the unchanged external Claim and terminal relation.

use ff::{Field as _, PrimeField as _, WithSmallOrderMulGroup};
use halo2_base::{
    AssignedValue,
    QuantumCell::{Constant, Existing},
    gates::{
        GateInstructions as _, RangeInstructions as _,
        circuit::{BaseCircuitParams, BaseConfig, builder::BaseCircuitBuilder},
    },
    utils::{BigPrimeField, CurveAffineExt},
};
use halo2_ecc::fields::{FieldChip as _, fp::FpChip};
use halo2_proofs::{
    circuit::{Layouter, V1},
    plonk::{Circuit, ConstraintSystem, Error as PlonkError},
};

use crate::zk::{
    pasta_cycle_loader::{DeferredBatchedEquationWitnessV1, LIMB_BITS, LIMBS, PastaCycleEccChip},
    pasta_dense_msm::{PastaDenseMsmConfigV1, PastaDenseMsmJobsV1},
};

/// Maximum complete Claim source inventory in either Pasta parity.
const CLAIM_MAX_SOURCES_V1: u32 = 1_008;
const CLAIM_SLICE_COUNT_V1: usize = 5;
/// `ceil(1008 / 5)`: the fixed public carrier capacity of every slice.
const CLAIM_SLICE_SLOTS_V1: usize = 202;
const CLAIM_SLICE_K_V1: usize = 15;
const MINIMUM_UNUSABLE_ROWS: usize = 9;
const DENSE_ROWS_PER_SOURCE_V1: usize = 130;
const DENSE_ROWS_PER_JOB_V1: usize = 3;

/// One original source/aggregate-coefficient pair in canonical carrier order.
#[derive(Clone, Copy)]
pub(super) struct PartialMsmSourceV1<C: CurveAffineExt> {
    pub(super) point: C,
    pub(super) coefficient: C::ScalarExt,
}

/// Fixed-size arithmetic statement for slice `I` of a claimed inventory.
///
/// Inactive slots must be exactly `(generator, zero)` and are public. A proof
/// alone does not authenticate the original source cells, their full-inventory
/// Fiat-Shamir challenge, or the neighboring slices.
#[derive(Clone, Copy)]
pub(super) struct PartialMsmFiveSliceStatementV1<C: CurveAffineExt> {
    pub(super) total_sources: u32,
    pub(super) source_start: u32,
    pub(super) source_end: u32,
    pub(super) active_sources: u32,
    pub(super) start: C,
    pub(super) endpoint: C,
    pub(super) sources: [PartialMsmSourceV1<C>; CLAIM_SLICE_SLOTS_V1],
}

impl<C: CurveAffineExt> PartialMsmFiveSliceStatementV1<C> {
    fn validate_shape<const I: usize>(&self) -> Result<(), String> {
        if I >= CLAIM_SLICE_COUNT_V1 || !(1..=CLAIM_MAX_SOURCES_V1).contains(&self.total_sources) {
            return Err("partial MSM slice has invalid index or source count".to_owned());
        }
        let count = u64::from(self.total_sources);
        let expected_start = ((I as u64) * count / CLAIM_SLICE_COUNT_V1 as u64) as u32;
        let expected_end = (((I + 1) as u64) * count / CLAIM_SLICE_COUNT_V1 as u64) as u32;
        let expected_active = expected_end - expected_start;
        if self.source_start != expected_start
            || self.source_end != expected_end
            || self.active_sources != expected_active
            || self.active_sources as usize > CLAIM_SLICE_SLOTS_V1
            || bool::from(self.start.is_identity())
            || bool::from(self.endpoint.is_identity())
        {
            return Err("partial MSM slice has invalid interval or endpoint".to_owned());
        }
        for (index, source) in self.sources.iter().enumerate() {
            if index < self.active_sources as usize {
                if bool::from(source.point.is_identity()) {
                    return Err("partial MSM slice has an identity active source".to_owned());
                }
            } else if source.point != C::generator() || source.coefficient != C::ScalarExt::ZERO {
                return Err("partial MSM slice has a noncanonical padding source".to_owned());
            }
        }
        Ok(())
    }

    /// Exact public instances: total, slice index, interval, active count,
    /// endpoints, then all original and canonical-padding source-major cells.
    pub(super) fn public_instances<const I: usize>(&self) -> Result<Vec<C::Base>, String>
    where
        C::Base: BigPrimeField,
        C::ScalarExt: BigPrimeField,
    {
        self.validate_shape::<I>()?;
        Ok(self.public_instances_unchecked::<I>())
    }

    fn public_instances_unchecked<const I: usize>(&self) -> Vec<C::Base>
    where
        C::Base: BigPrimeField,
        C::ScalarExt: BigPrimeField,
    {
        let mut public = Vec::with_capacity(9 + CLAIM_SLICE_SLOTS_V1 * 4);
        for value in [
            u64::from(self.total_sources),
            I as u64,
            u64::from(self.source_start),
            u64::from(self.source_end),
            u64::from(self.active_sources),
        ] {
            public.push(C::Base::from(value));
        }
        for point in [self.start, self.endpoint] {
            public.extend(compressed_limbs::<C>(point).map(C::Base::from_u128));
        }
        for source in &self.sources {
            public.extend(compressed_limbs::<C>(source.point).map(C::Base::from_u128));
            public.extend(scalar_limbs::<C>(source.coefficient).map(C::Base::from_u128));
        }
        public
    }
}

fn two_limbs(bytes: &[u8]) -> [u128; 2] {
    std::array::from_fn(|half| {
        u128::from_le_bytes(
            bytes[half * 16..(half + 1) * 16]
                .try_into()
                .expect("Pasta field encoding is exactly 32 bytes"),
        )
    })
}

fn compressed_limbs<C: CurveAffineExt>(point: C) -> [u128; 2] {
    two_limbs(point.to_bytes().as_ref())
}

fn scalar_limbs<C: CurveAffineExt>(scalar: C::ScalarExt) -> [u128; 2] {
    two_limbs(scalar.to_repr().as_ref())
}

/// One parity's fixed k15 slice relation. It is not a monetary proof.
#[derive(Clone)]
pub(super) struct PartialMsmFiveSliceCircuitV1<C: CurveAffineExt, const I: usize>
where
    C::Base: BigPrimeField,
{
    builder: BaseCircuitBuilder<C::Base>,
    dense_jobs: PastaDenseMsmJobsV1<C>,
}

#[derive(Clone)]
pub(super) struct PartialMsmFiveSliceConfigV1<F: halo2_base::utils::ScalarField> {
    base: BaseConfig<F>,
    dense: PastaDenseMsmConfigV1,
}

impl<C, const I: usize> PartialMsmFiveSliceCircuitV1<C, I>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField + halo2_base::utils::ScalarField + WithSmallOrderMulGroup<3>,
    C::ScalarExt: BigPrimeField + WithSmallOrderMulGroup<3>,
{
    /// Build an exact padded fifth of at most 1,008 original source cells.
    pub(super) fn build(statement: &PartialMsmFiveSliceStatementV1<C>) -> Result<Self, String> {
        statement.validate_shape::<I>()?;
        Self::build_arithmetic(statement)
    }

    #[cfg(test)]
    fn build_unchecked_for_test(
        statement: &PartialMsmFiveSliceStatementV1<C>,
    ) -> Result<Self, String> {
        Self::build_arithmetic(statement)
    }

    fn build_arithmetic(statement: &PartialMsmFiveSliceStatementV1<C>) -> Result<Self, String> {
        if I >= CLAIM_SLICE_COUNT_V1 {
            return Err("partial MSM slice index exceeds five-slice layout".to_owned());
        }
        // The dense equation includes both endpoints as sources. At full
        // capacity it consumes 204*130+3 = 26,523 of 32,759 usable rows.
        let dense_rows =
            (CLAIM_SLICE_SLOTS_V1 + 2) * DENSE_ROWS_PER_SOURCE_V1 + DENSE_ROWS_PER_JOB_V1;
        if dense_rows > (1 << CLAIM_SLICE_K_V1) - MINIMUM_UNUSABLE_ROWS {
            return Err("partial MSM slice exceeds its fixed k15 dense lane".to_owned());
        }
        let mut builder = BaseCircuitBuilder::new(false)
            .use_k(CLAIM_SLICE_K_V1)
            .use_lookup_bits(CLAIM_SLICE_K_V1 - 1)
            .use_instance_columns(1);
        let range = builder.range_chip();
        let base = FpChip::<C::Base, C::Base>::new(&range, LIMB_BITS, LIMBS);
        let scalar = FpChip::<C::Base, C::ScalarExt>::new(&range, LIMB_BITS, LIMBS);
        let chip = PastaCycleEccChip::<C>::new(&base, &scalar);
        let mut context = std::mem::take(builder.pool(0));
        let ctx = context.main();
        let gate = range.gate();
        let total = ctx.load_witness(C::Base::from(u64::from(statement.total_sources)));
        let index = ctx.load_constant(C::Base::from(I as u64));
        let first = ctx.load_witness(C::Base::from(u64::from(statement.source_start)));
        let last = ctx.load_witness(C::Base::from(u64::from(statement.source_end)));
        let active = ctx.load_witness(C::Base::from(u64::from(statement.active_sources)));
        range.range_check(ctx, total, 10);
        range.range_check(ctx, first, 10);
        range.range_check(ctx, last, 10);
        range.range_check(ctx, active, 8);
        let count_zero = gate.is_zero(ctx, total);
        gate.assert_is_const(ctx, &count_zero, &C::Base::ZERO);
        let count_valid = range.is_less_than_safe(ctx, total, u64::from(CLAIM_MAX_SOURCES_V1) + 1);
        gate.assert_is_const(ctx, &count_valid, &C::Base::ONE);
        let active_valid = range.is_less_than_safe(ctx, active, CLAIM_SLICE_SLOTS_V1 as u64 + 1);
        gate.assert_is_const(ctx, &active_valid, &C::Base::ONE);
        let difference = gate.sub(ctx, last, first);
        ctx.constrain_equal(&difference, &active);

        // Bounded Euclidean remainders make both floor divisions exact. Every
        // integer is at most 5,040, so the field equations cannot wrap.
        let count = u64::from(statement.total_sources);
        let first_remainder = ctx.load_witness(C::Base::from((I as u64 * count) % 5));
        let last_remainder = ctx.load_witness(C::Base::from(((I + 1) as u64 * count) % 5));
        for remainder in [first_remainder, last_remainder] {
            range.range_check(ctx, remainder, 3);
            let valid = range.is_less_than_safe(ctx, remainder, 5);
            gate.assert_is_const(ctx, &valid, &C::Base::ONE);
        }
        let first_dividend = gate.mul_add(
            ctx,
            Existing(first),
            Constant(C::Base::from(5)),
            Existing(first_remainder),
        );
        let first_product = gate.mul(ctx, Existing(index), Existing(total));
        ctx.constrain_equal(&first_dividend, &first_product);
        let last_dividend = gate.mul_add(
            ctx,
            Existing(last),
            Constant(C::Base::from(5)),
            Existing(last_remainder),
        );
        let next_index = ctx.load_constant(C::Base::from((I + 1) as u64));
        let last_product = gate.mul(ctx, Existing(next_index), Existing(total));
        ctx.constrain_equal(&last_dividend, &last_product);

        // The existing machine proves a zero sum. Add the public start with
        // coefficient +1 and the public endpoint with coefficient -1; its
        // identity equation is exactly the desired partial-MSM relation.
        let mut points = Vec::with_capacity(CLAIM_SLICE_SLOTS_V1 + 2);
        let mut coefficients = Vec::with_capacity(CLAIM_SLICE_SLOTS_V1 + 2);
        points.push(statement.start);
        coefficients.push(C::ScalarExt::ONE);
        for source in &statement.sources {
            points.push(source.point);
            coefficients.push(source.coefficient);
        }
        points.push(statement.endpoint);
        coefficients.push(-C::ScalarExt::ONE);
        let assigned = chip.assign_deferred_batched_equation_v1(
            &mut context,
            &DeferredBatchedEquationWitnessV1 {
                sources: points,
                challenge: C::ScalarExt::ONE,
                aggregate_coefficients: coefficients,
            },
        )?;
        let one = scalar.load_constant(context.main(), C::ScalarExt::ONE);
        let negative_one = scalar.load_constant(context.main(), -C::ScalarExt::ONE);
        scalar.assert_equal(
            context.main(),
            assigned.aggregate_coefficients[0].clone(),
            one,
        );
        scalar.assert_equal(
            context.main(),
            assigned.aggregate_coefficients[CLAIM_SLICE_SLOTS_V1 + 1].clone(),
            negative_one,
        );
        let mut public: Vec<AssignedValue<C::Base>> =
            Vec::with_capacity(9 + CLAIM_SLICE_SLOTS_V1 * 4);
        public.extend([total, index, first, last, active]);
        for position in [0, CLAIM_SLICE_SLOTS_V1 + 1] {
            public
                .extend(chip.assigned_point_u128_limbs(&mut context, &assigned.sources[position]));
        }
        let dummy_limbs = compressed_limbs::<C>(C::generator());
        for slot in 0..CLAIM_SLICE_SLOTS_V1 {
            let point_limbs =
                chip.assigned_point_u128_limbs(&mut context, &assigned.sources[slot + 1]);
            let scalar_limbs = chip.assigned_scalar_u128_limbs(
                &mut context,
                &assigned.aggregate_coefficients[slot + 1],
            );
            let ctx = context.main();
            let enabled = range.is_less_than(ctx, Constant(C::Base::from(slot as u64)), active, 8);
            for (limb, expected) in point_limbs.iter().zip(dummy_limbs) {
                let difference = gate.sub(ctx, *limb, Constant(C::Base::from_u128(expected)));
                let invalid_padding = gate.mul_not(ctx, enabled, difference);
                gate.assert_is_const(ctx, &invalid_padding, &C::Base::ZERO);
            }
            for limb in &scalar_limbs {
                let invalid_padding = gate.mul_not(ctx, enabled, *limb);
                gate.assert_is_const(ctx, &invalid_padding, &C::Base::ZERO);
            }
            public.extend(point_limbs);
            public.extend(scalar_limbs);
        }
        let mut dense_jobs = PastaDenseMsmJobsV1::default();
        chip.constrain_deferred_batched_equation_with_lanes_v1(
            &mut context,
            &assigned,
            &mut dense_jobs,
            1,
        )?;
        *builder.pool(0) = context;
        builder.assigned_instances = vec![public];
        super::base_packing::finalize_base_params_v1(&mut builder, MINIMUM_UNUSABLE_ROWS)?;
        dense_jobs
            .validate_capacity_with_lanes((1 << CLAIM_SLICE_K_V1) - MINIMUM_UNUSABLE_ROWS, 1)?;
        Ok(Self {
            builder,
            dense_jobs,
        })
    }
}

impl<C, const I: usize> Circuit<C::Base> for PartialMsmFiveSliceCircuitV1<C, I>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField + halo2_base::utils::ScalarField + WithSmallOrderMulGroup<3>,
    C::ScalarExt: BigPrimeField + WithSmallOrderMulGroup<3>,
{
    type Config = PartialMsmFiveSliceConfigV1<C::Base>;
    type FloorPlanner = V1;
    type Params = BaseCircuitParams;

    fn params(&self) -> Self::Params {
        self.builder.config_params.clone()
    }

    fn without_witnesses(&self) -> Self {
        Self {
            builder: self.builder.deep_clone().unknown(true),
            dense_jobs: self.dense_jobs.unknown(),
        }
    }

    fn configure_with_params(
        meta: &mut ConstraintSystem<C::Base>,
        params: Self::Params,
    ) -> Self::Config {
        let usable_rows = (1_usize << params.k) - MINIMUM_UNUSABLE_ROWS;
        let mut base = BaseConfig::configure(meta, params);
        base.set_usable_rows(usable_rows);
        PartialMsmFiveSliceConfigV1 {
            base,
            dense: PastaDenseMsmConfigV1::configure_with_lanes::<C>(meta, 1),
        }
    }

    fn configure(_: &mut ConstraintSystem<C::Base>) -> Self::Config {
        unreachable!("partial MSM slice uses authenticated Base parameters")
    }

    fn synthesize_for_measurement(
        &self,
        config: Self::Config,
        layouter: impl Layouter<C::Base>,
    ) -> Result<(), PlonkError> {
        let result = self.synthesize(config, layouter);
        self.builder.reset_synthesis_state();
        result
    }

    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<C::Base>,
    ) -> Result<(), PlonkError> {
        <BaseCircuitBuilder<C::Base> as Circuit<C::Base>>::synthesize(
            &self.builder,
            config.base,
            layouter.namespace(|| "KAGEMUSHA five-slice partial MSM Base"),
        )?;
        self.dense_jobs.synthesize(
            &config.dense,
            &mut layouter,
            &self.builder.core().copy_manager,
            self.builder.witness_gen_only(),
            (1 << CLAIM_SLICE_K_V1) - MINIMUM_UNUSABLE_ROWS,
        )
    }
}

#[cfg(test)]
#[path = "partial_msm_shard_tests.rs"]
mod tests;
