//! Non-authorizing, bounded partial-MSM proof prerequisite for KAGEMUSHA.
//!
//! A shard proves `endpoint = start + sum(source[i] * coefficient[i])` using the
//! existing constrained dense zero-sum machine. Its public source cells use
//! exactly the compressed-point/two-u128-scalar layout of the deferred carrier.
//! No current monetary relation verifies these shards or proves that their
//! intervals cover its complete authenticated source namespace. Start and
//! endpoint must be nonidentity. A future contiguous-cover verifier must pin
//! one nonidentity chain offset, pass every endpoint to the next shard, and
//! require the final endpoint to equal that same offset for a zero aggregate.

use ff::{Field as _, PrimeField as _, WithSmallOrderMulGroup};
use halo2_base::{
    AssignedValue,
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

/// The shard uses one dense lane at a smaller domain than the live k16 relations.
const PARTIAL_MSM_SHARD_K_V1: usize = 14;
const MINIMUM_UNUSABLE_ROWS: usize = 9;
/// A fixed shard size is part of its circuit type and verifier protocol.
const PARTIAL_MSM_SHARD_MAX_SOURCES_V1: usize = 32;

/// One original source/aggregate-coefficient pair in canonical carrier order.
#[derive(Clone, Copy)]
pub(super) struct PartialMsmSourceV1<C: CurveAffineExt> {
    pub(super) point: C,
    pub(super) coefficient: C::ScalarExt,
}

/// Fixed-size, non-authorizing partial-MSM statement.
///
/// The two indices name a half-open interval in a future authenticated full
/// carrier. This proof binds the named bytes but does not establish that such a
/// carrier exists or that adjacent shards have no gaps or overlaps.
#[derive(Clone, Copy)]
pub(super) struct PartialMsmShardStatementV1<C: CurveAffineExt, const N: usize> {
    pub(super) source_start: u32,
    pub(super) source_end: u32,
    pub(super) start: C,
    pub(super) endpoint: C,
    pub(super) sources: [PartialMsmSourceV1<C>; N],
}

impl<C: CurveAffineExt, const N: usize> PartialMsmShardStatementV1<C, N> {
    fn validate_shape(&self) -> Result<(), String> {
        if !(1..=PARTIAL_MSM_SHARD_MAX_SOURCES_V1).contains(&N)
            || self.source_start.checked_add(N as u32) != Some(self.source_end)
            || bool::from(self.start.is_identity())
            || bool::from(self.endpoint.is_identity())
            || self
                .sources
                .iter()
                .any(|source| bool::from(source.point.is_identity()))
        {
            return Err("partial MSM shard has invalid interval or identity source".to_owned());
        }
        Ok(())
    }

    /// Exact public instances: interval, endpoints, then the source-major carrier.
    pub(super) fn public_instances(&self) -> Result<Vec<C::Base>, String>
    where
        C::Base: BigPrimeField,
        C::ScalarExt: BigPrimeField,
    {
        self.validate_shape()?;
        let mut public = Vec::with_capacity(6 + N * 4);
        public.push(C::Base::from(u64::from(self.source_start)));
        public.push(C::Base::from(u64::from(self.source_end)));
        for point in [self.start, self.endpoint] {
            public.extend(compressed_limbs::<C>(point).map(C::Base::from_u128));
        }
        for source in &self.sources {
            public.extend(compressed_limbs::<C>(source.point).map(C::Base::from_u128));
            public.extend(scalar_limbs::<C>(source.coefficient).map(C::Base::from_u128));
        }
        Ok(public)
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

/// One parity's lower-k partial-MSM relation. It is not a monetary proof.
#[derive(Clone)]
pub(super) struct PartialMsmShardCircuitV1<C: CurveAffineExt, const N: usize>
where
    C::Base: BigPrimeField,
{
    builder: BaseCircuitBuilder<C::Base>,
    dense_jobs: PastaDenseMsmJobsV1<C>,
}

#[derive(Clone)]
pub(super) struct PartialMsmShardConfigV1<F: halo2_base::utils::ScalarField> {
    base: BaseConfig<F>,
    dense: PastaDenseMsmConfigV1,
}

impl<C, const N: usize> PartialMsmShardCircuitV1<C, N>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField + halo2_base::utils::ScalarField + WithSmallOrderMulGroup<3>,
    C::ScalarExt: BigPrimeField + WithSmallOrderMulGroup<3>,
{
    /// Build a shard from exactly its public statement; the circuit independently
    /// constrains the claimed endpoint through the dense zero-sum relation.
    pub(super) fn build(statement: &PartialMsmShardStatementV1<C, N>) -> Result<Self, String> {
        statement.validate_shape()?;
        let mut builder = BaseCircuitBuilder::new(false)
            .use_k(PARTIAL_MSM_SHARD_K_V1)
            .use_lookup_bits(PARTIAL_MSM_SHARD_K_V1 - 1)
            .use_instance_columns(1);
        let range = builder.range_chip();
        let base = FpChip::<C::Base, C::Base>::new(&range, LIMB_BITS, LIMBS);
        let scalar = FpChip::<C::Base, C::ScalarExt>::new(&range, LIMB_BITS, LIMBS);
        let chip = PastaCycleEccChip::<C>::new(&base, &scalar);
        let mut context = std::mem::take(builder.pool(0));
        let first = context
            .main()
            .load_witness(C::Base::from(u64::from(statement.source_start)));
        let last = context
            .main()
            .load_witness(C::Base::from(u64::from(statement.source_end)));
        range.range_check(context.main(), first, 32);
        range.range_check(context.main(), last, 32);
        let difference = range.gate().sub(context.main(), last, first);
        range
            .gate()
            .assert_is_const(context.main(), &difference, &C::Base::from(N as u64));

        // The existing machine proves a zero sum. Add the public start with
        // coefficient +1 and the public endpoint with coefficient -1; its
        // identity equation is exactly the desired partial-MSM relation.
        let mut points = Vec::with_capacity(N + 2);
        let mut coefficients = Vec::with_capacity(N + 2);
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
            assigned.aggregate_coefficients[N + 1].clone(),
            negative_one,
        );
        let mut public: Vec<AssignedValue<C::Base>> = Vec::with_capacity(6 + N * 4);
        public.extend([first, last]);
        for index in [0, N + 1] {
            public.extend(chip.assigned_point_u128_limbs(&mut context, &assigned.sources[index]));
        }
        for index in 1..=N {
            public.extend(chip.assigned_point_u128_limbs(&mut context, &assigned.sources[index]));
            public.extend(
                chip.assigned_scalar_u128_limbs(
                    &mut context,
                    &assigned.aggregate_coefficients[index],
                ),
            );
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
        dense_jobs.validate_capacity_with_lanes(
            (1 << PARTIAL_MSM_SHARD_K_V1) - MINIMUM_UNUSABLE_ROWS,
            1,
        )?;
        Ok(Self {
            builder,
            dense_jobs,
        })
    }
}

impl<C, const N: usize> Circuit<C::Base> for PartialMsmShardCircuitV1<C, N>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField + halo2_base::utils::ScalarField + WithSmallOrderMulGroup<3>,
    C::ScalarExt: BigPrimeField + WithSmallOrderMulGroup<3>,
{
    type Config = PartialMsmShardConfigV1<C::Base>;
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
        PartialMsmShardConfigV1 {
            base,
            dense: PastaDenseMsmConfigV1::configure_with_lanes::<C>(meta, 1),
        }
    }

    fn configure(_: &mut ConstraintSystem<C::Base>) -> Self::Config {
        unreachable!("partial MSM shard uses authenticated Base parameters")
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
            layouter.namespace(|| "KAGEMUSHA partial MSM shard Base"),
        )?;
        self.dense_jobs.synthesize(
            &config.dense,
            &mut layouter,
            &self.builder.core().copy_manager,
            self.builder.witness_gen_only(),
            (1 << PARTIAL_MSM_SHARD_K_V1) - MINIMUM_UNUSABLE_ROWS,
        )
    }
}

#[cfg(test)]
#[path = "partial_msm_shard_tests.rs"]
mod tests;
