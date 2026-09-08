//! Ignored attribution of retained reciprocal Base allocations, not process RSS.
//!
//! This uses the compact-batch fixture and production encoding/GLV entry points.
//! It measures Base synthesis only: queued dense rows, carrier authentication,
//! whole-Claim construction, keys, and proofs are outside this diagnostic.

use super::*;
use halo2_base::{
    ContextCell,
    gates::circuit::{BaseCircuitParams, BaseConfig, builder::BaseCircuitBuilder},
};
use halo2_proofs::{
    circuit::{Cell, Layouter, floor_planner::V1},
    dev::MockProver,
    halo2curves::pasta::{EpAffine, EqAffine},
    plonk::{Circuit, ConstraintSystem, Error},
};
use std::{mem, time::Instant};

const DIAGNOSTIC_K: usize = 16;
const UNUSABLE_ROWS: usize = 9;

/// Entries and capacities are counts unless the field name says bytes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct AllocationCounts {
    contexts: usize,
    advice: usize,
    rational_advice: usize,
    // Numerators, zero-mask bytes, rational positions, denominators.
    advice_storage_len: [usize; 4],
    advice_storage_capacity: [usize; 4],
    // Numerator, zero-mask, selector segment counts (not segment bytes).
    segments: [usize; 3],
    selector_bytes_len_capacity: [usize; 2],
    lookup_buckets: usize,
    lookup_rows_len_capacity: [usize; 2],
    advice_equalities_len_capacity: [usize; 2],
    constant_equalities: usize,
    distinct_constants: usize,
    constant_cells_len_capacity: [usize; 2],
    physical_advice_cells: usize,
    physical_advice_contexts: usize,
    physical_advice_runs: usize,
    // Sum of run-vector capacities in bytes; excludes BTree nodes and allocator overhead.
    physical_advice_run_vector_capacity_bytes: usize,
    physical_constants: usize,
}

impl AllocationCounts {
    fn virtual_only(mut self) -> Self {
        self.physical_advice_cells = 0;
        self.physical_advice_contexts = 0;
        self.physical_advice_runs = 0;
        self.physical_advice_run_vector_capacity_bytes = 0;
        self.physical_constants = 0;
        self
    }
}

fn snapshot<F: BigPrimeField>(builder: &BaseCircuitBuilder<F>) -> AllocationCounts {
    let mut counts = AllocationCounts::default();
    for ctx in builder
        .core()
        .phase_manager
        .iter()
        .flat_map(|phase| &phase.threads)
    {
        counts.contexts += 1;
        counts.advice += ctx.advice_len();
        counts.rational_advice += ctx.rational_advice_len();
        let lengths = [
            ctx.advice_len(),
            ctx.advice_zero_mask_bytes_len(),
            ctx.advice_rational_position_slots_len(),
            ctx.advice_denominator_slots_len(),
        ];
        let capacities = ctx.advice_storage_capacities();
        for index in 0..4 {
            counts.advice_storage_len[index] += lengths[index];
            counts.advice_storage_capacity[index] += capacities[index];
            assert!(lengths[index] <= capacities[index]);
        }
        counts.segments[0] += ctx.advice_numerator_segment_count();
        counts.segments[1] += ctx.advice_zero_mask_segment_count();
        counts.segments[2] += ctx.selector_segment_count();
        counts.selector_bytes_len_capacity[0] += ctx.selector_storage_bytes_len();
        counts.selector_bytes_len_capacity[1] += ctx.selector_checked_capacity_bytes().unwrap();
    }
    for manager in builder.lookup_manager() {
        let buckets = manager.cells_to_lookup.lock().unwrap();
        counts.lookup_buckets += buckets.len();
        for rows in buckets.values() {
            counts.lookup_rows_len_capacity[0] += rows.len();
            counts.lookup_rows_len_capacity[1] += rows.capacity();
        }
    }
    let copies = builder.core().copy_manager.lock().unwrap();
    counts.advice_equalities_len_capacity = [
        copies.advice_equalities.len(),
        copies.advice_equalities.capacity(),
    ];
    counts.constant_equalities = copies.constant_equalities.len();
    counts.distinct_constants = copies.constant_equalities.distinct_len();
    counts.constant_cells_len_capacity = [
        copies.constant_equalities.checked_cell_len().unwrap(),
        copies.constant_equalities.checked_cell_capacity().unwrap(),
    ];
    counts.physical_advice_cells = copies.assigned_advices.len();
    counts.physical_advice_contexts = copies.assigned_advices.context_count();
    counts.physical_advice_runs = copies.assigned_advices.run_count();
    counts.physical_advice_run_vector_capacity_bytes = copies
        .assigned_advices
        .checked_run_capacity_bytes()
        .expect("physical run-vector capacity fits usize");
    assert!(counts.physical_advice_contexts <= counts.physical_advice_runs);
    assert!(counts.physical_advice_runs <= counts.physical_advice_cells);
    if counts.physical_advice_cells == 0 {
        assert_eq!(counts.physical_advice_contexts, 0);
        assert_eq!(counts.physical_advice_runs, 0);
        assert_eq!(counts.physical_advice_run_vector_capacity_bytes, 0);
    }
    counts.physical_constants = copies.assigned_constants.len();
    assert_eq!(
        counts.constant_cells_len_capacity[0],
        counts.constant_equalities
    );
    counts
}

fn report<F: BigPrimeField>(
    builder: &BaseCircuitBuilder<F>,
    parity: &str,
    sources: usize,
    stage: &str,
    work_started: Instant,
) -> AllocationCounts {
    let work_us = work_started.elapsed().as_micros();
    let stats_started = Instant::now();
    let counts = snapshot(builder);
    let stats_us = stats_started.elapsed().as_micros();
    eprintln!(
        "RECIPROCAL_ALLOCATION parity={parity} sources={sources} stage={stage} work_us={work_us} stats_us={stats_us} counts={counts:?}"
    );
    counts
}

/// Mutually exclusive canonical-width bins: zero, one, other <=64, 65..128, >128.
fn width_bin<F: BigPrimeField>(value: F) -> usize {
    if value == F::ZERO {
        0
    } else if value == F::ONE {
        1
    } else {
        // This ignored fixture is instantiated only for the two Pasta fields,
        // whose canonical PrimeField representation is 32-byte little endian.
        let repr = value.to_repr();
        let bytes = repr.as_ref();
        if bytes[8..].iter().all(|byte| *byte == 0) {
            2
        } else if bytes[16..].iter().all(|byte| *byte == 0) {
            3
        } else {
            4
        }
    }
}

fn histogram<F: BigPrimeField>(builder: &BaseCircuitBuilder<F>, parity: &str, sources: usize) {
    // Check bin boundaries in each actual field; no graph/value collection is cloned.
    assert_eq!(width_bin(F::ZERO), 0);
    assert_eq!(width_bin(F::ONE), 1);
    assert_eq!(width_bin(F::from(u64::MAX)), 2);
    assert_eq!(width_bin(F::from(u64::MAX) + F::ONE), 3);
    assert_eq!(width_bin(F::from_u128(u128::MAX)), 3);
    assert_eq!(width_bin(F::from_u128(u128::MAX) + F::ONE), 4);
    let started = Instant::now();
    let mut numerators = [0_usize; 5];
    let mut values = [0_usize; 5];
    let mut visited = 0;
    for ctx in builder
        .core()
        .phase_manager
        .iter()
        .flat_map(|phase| &phase.threads)
    {
        for offset in 0..ctx.advice_len() {
            let assigned = ctx.get(isize::try_from(offset).unwrap()).value;
            numerators[width_bin(assigned.numerator())] += 1;
            values[width_bin(assigned.evaluate())] += 1;
            visited += 1;
        }
    }
    assert_eq!(numerators.iter().sum::<usize>(), visited);
    assert_eq!(values.iter().sum::<usize>(), visited);
    eprintln!(
        "RECIPROCAL_HISTOGRAM parity={parity} sources={sources} stage=glv_queue visited={visited} numerators={numerators:?} values={values:?} scan_us={} bins=zero,one,other_le64,65_to128,over128",
        started.elapsed().as_micros()
    );
}

/// Observe the existing graph with the same measurement reset used by Claim.
struct AllocationCircuit<F: BigPrimeField> {
    builder: BaseCircuitBuilder<F>,
    parity: &'static str,
    sources: usize,
    passes: RefCell<Vec<AllocationCounts>>,
}

impl<F: BigPrimeField> Circuit<F> for AllocationCircuit<F> {
    type Config = BaseConfig<F>;
    type FloorPlanner = V1;
    type Params = BaseCircuitParams;

    fn params(&self) -> Self::Params {
        self.builder.config_params.clone()
    }

    fn without_witnesses(&self) -> Self {
        unreachable!("the diagnostic measures its existing graph without cloning")
    }

    fn configure_with_params(meta: &mut ConstraintSystem<F>, params: Self::Params) -> Self::Config {
        let usable_rows = (1_usize << params.k) - UNUSABLE_ROWS;
        let mut config = BaseConfig::configure(meta, params);
        config.set_usable_rows(usable_rows);
        config
    }

    fn configure(_: &mut ConstraintSystem<F>) -> Self::Config {
        unreachable!("the diagnostic uses explicit k=16 Base parameters")
    }

    fn synthesize_for_measurement(
        &self,
        config: Self::Config,
        layouter: impl Layouter<F>,
    ) -> Result<(), Error> {
        let started = Instant::now();
        // V1's measurement layouter discards advice values. The shared graph
        // is read once and every physical coordinate is reset before replay.
        let result = self.builder.synthesize(config, layouter);
        let measured = report(
            &self.builder,
            self.parity,
            self.sources,
            "measurement_assigned",
            started,
        );
        let reset_started = Instant::now();
        self.builder.reset_synthesis_state();
        let reset = report(
            &self.builder,
            self.parity,
            self.sources,
            "measurement_reset",
            reset_started,
        );
        self.passes.borrow_mut().extend([measured, reset]);
        result
    }

    fn synthesize(&self, config: Self::Config, layouter: impl Layouter<F>) -> Result<(), Error> {
        let started = Instant::now();
        let result = self.builder.synthesize(config, layouter);
        let assigned = report(
            &self.builder,
            self.parity,
            self.sources,
            "final_assigned",
            started,
        );
        self.passes.borrow_mut().push(assigned);
        result
    }
}

fn check_allocations<C>(parity: &'static str, source_count: usize)
where
    C: CurveAffineExt,
    Outer<C>: BigPrimeField + ff::WithSmallOrderMulGroup<3>,
    Inner<C>: BigPrimeField + ff::WithSmallOrderMulGroup<3>,
{
    let started = Instant::now();
    let mut builder = BaseCircuitBuilder::<Outer<C>>::new(false)
        .use_k(DIAGNOSTIC_K)
        .use_lookup_bits(DIAGNOSTIC_K - 1);
    let range = builder.range_chip();
    let base = FpChip::<Outer<C>, Outer<C>>::new(&range, LIMB_BITS, LIMBS);
    let scalar = FpChip::<Outer<C>, Inner<C>>::new(&range, LIMB_BITS, LIMBS);
    let chip = PastaCycleEccChip::<C>::new(&base, &scalar);
    let empty = report(&builder, parity, source_count, "empty", started);
    assert_eq!(empty.advice, 0);
    eprintln!(
        "RECIPROCAL_TYPE_BYTES parity={parity} sources={source_count} field={} assigned_value={} context_cell={} equality_pair={} physical_cell_payload={} rational_position={} rss_measured=false",
        mem::size_of::<Outer<C>>(),
        mem::size_of::<AssignedValue<Outer<C>>>(),
        mem::size_of::<ContextCell>(),
        mem::size_of::<(ContextCell, ContextCell)>(),
        mem::size_of::<Cell>(),
        mem::size_of::<u32>(),
    );
    let generator = C::generator();
    let doubled = (generator.to_curve() + generator.to_curve()).to_affine();
    // Repeat the existing (2*g - 1*(2*g)) fixture. An odd final source has
    // coefficient zero. This is synthetic shape evidence, not production value distribution.
    let witness = DeferredBatchedEquationWitnessV1 {
        sources: (0..source_count)
            .map(|index| if index % 2 == 0 { generator } else { doubled })
            .collect(),
        challenge: Inner::<C>::from(13),
        aggregate_coefficients: (0..source_count)
            .map(|index| {
                if source_count % 2 == 1 && index + 1 == source_count {
                    Inner::<C>::ZERO
                } else if index % 2 == 0 {
                    Inner::<C>::from(2)
                } else {
                    -Inner::<C>::ONE
                }
            })
            .collect(),
    };
    let mut ctx = mem::take(builder.pool(0));
    let started = Instant::now();
    let assigned = chip
        .assign_deferred_batched_equation_v1(&mut ctx, &witness)
        .unwrap();
    assert_eq!(assigned.sources.len(), source_count);
    *builder.pool(0) = ctx;
    let points = report(
        &builder,
        parity,
        source_count,
        "point_coefficient_assignment",
        started,
    );
    ctx = mem::take(builder.pool(0));
    let started = Instant::now();
    let _ = chip.assigned_scalar_u128_limbs(&mut ctx, &assigned.challenge);
    for (point, coefficient) in assigned
        .sources
        .iter()
        .zip(&assigned.aggregate_coefficients)
    {
        let _ = chip.assigned_point_u128_limbs(&mut ctx, point);
        let _ = chip.assigned_scalar_u128_limbs(&mut ctx, coefficient);
    }
    *builder.pool(0) = ctx;
    let encoded = report(&builder, parity, source_count, "u128_encoding", started);
    ctx = mem::take(builder.pool(0));
    let started = Instant::now();
    let mut dense_jobs = PastaDenseMsmJobsV1::default();
    chip.constrain_deferred_batched_equation_v1(&mut ctx, &assigned, &mut dense_jobs)
        .unwrap();
    let (jobs, sources, rows) = dense_jobs.capacity_profile().unwrap();
    assert_eq!((jobs, sources), (1, source_count));
    *builder.pool(0) = ctx;
    let queued = report(&builder, parity, source_count, "glv_queue", started);
    assert!(points.advice > empty.advice);
    assert!(encoded.advice > points.advice);
    assert!(queued.advice > encoded.advice);
    eprintln!(
        "RECIPROCAL_DENSE_QUEUE parity={parity} sources={source_count} jobs={jobs} maximum_rows={rows} dense_rows_assigned=false"
    );
    histogram(&builder, parity, source_count);
    builder.calculate_params(Some(UNUSABLE_ROWS));
    eprintln!(
        "RECIPROCAL_BASE_PARAMS parity={parity} sources={source_count} params={:?}",
        builder.config_params
    );
    let circuit = AllocationCircuit {
        builder,
        parity,
        sources: source_count,
        passes: RefCell::new(Vec::new()),
    };
    MockProver::run(DIAGNOSTIC_K as u32, &circuit, vec![])
        .unwrap()
        .assert_satisfied();
    let passes = circuit.passes.borrow();
    assert_eq!(
        passes.len(),
        3,
        "one measurement, one reset, one final assignment"
    );
    for pass in passes.iter() {
        assert_eq!(
            pass.virtual_only(),
            queued.virtual_only(),
            "synthesis retains the virtual graph"
        );
    }
    assert_eq!(passes[0].physical_advice_cells, queued.advice);
    assert!(passes[0].physical_advice_contexts > 0);
    assert!(passes[0].physical_advice_runs > 0);
    assert!(passes[0].physical_advice_run_vector_capacity_bytes > 0);
    assert_eq!(passes[1].physical_advice_cells, 0);
    assert_eq!(passes[1].physical_advice_contexts, 0);
    assert_eq!(passes[1].physical_advice_runs, 0);
    assert_eq!(passes[1].physical_constants, 0);
    assert_eq!(
        passes[1].physical_advice_run_vector_capacity_bytes, 0,
        "PhysicalAdviceMap clear drops all owned run vectors; this is not an RSS assertion"
    );
    assert_eq!(passes[2].physical_advice_cells, queued.advice);
    assert_eq!(
        passes[2].physical_advice_contexts,
        passes[0].physical_advice_contexts
    );
    assert_eq!(
        passes[2].physical_advice_runs,
        passes[0].physical_advice_runs
    );
    assert!(passes[2].physical_advice_run_vector_capacity_bytes > 0);
    assert_eq!(passes[2].physical_constants, passes[0].physical_constants);
    eprintln!("RECIPROCAL_ALLOCATION_CASE_PASS parity={parity} sources={source_count}");
}

#[test]
#[ignore = "allocation attribution at 2/32/865/1008 sources; no whole-Claim proof or RSS qualification"]
fn reciprocal_compact_batch_allocation_diagnostic_in_both_parities() {
    for source_count in [2, 32, 865, 1008] {
        check_allocations::<EqAffine>("eq", source_count);
        check_allocations::<EpAffine>("ep", source_count);
    }
}
