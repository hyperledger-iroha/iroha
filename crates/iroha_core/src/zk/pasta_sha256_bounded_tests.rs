//! Constraint-level tests for fixed-capacity, variable-length SHA-256 jobs.
//!
//! These tests exercise the actual Base/Table16 circuit, not a full recursive mint proof. Shape
//! comparisons inspect assigned constraint schedules and copy graphs; they are not key-generation
//! or production proving-resource qualification.

use std::collections::BTreeSet;

use ff::Field as _;
use halo2_base::{
    ContextCell,
    gates::circuit::{BaseCircuitParams, BaseConfig, builder::BaseCircuitBuilder},
};
use halo2_proofs::{
    circuit::V1,
    dev::MockProver,
    halo2curves::pasta::{Fp, Fq},
    plonk::Circuit,
};

use super::*;

const TEST_K: u32 = 16;
const UNUSABLE_ROWS: usize = 9;

#[derive(Clone, Debug)]
struct BoundedConfig<F: ScalarField> {
    base: BaseConfig<F>,
    sha: PastaSha256ConfigV1,
}

#[derive(Clone)]
struct BoundedCircuit<F: BigPrimeField> {
    builder: BaseCircuitBuilder<F>,
    jobs: PastaSha256JobsV1<F>,
    instances: Vec<F>,
}

impl<F: BigPrimeField> Circuit<F> for BoundedCircuit<F> {
    type Config = BoundedConfig<F>;
    type FloorPlanner = V1;
    type Params = BaseCircuitParams;

    fn params(&self) -> Self::Params {
        self.builder.config_params.clone()
    }

    fn without_witnesses(&self) -> Self {
        Self {
            builder: self.builder.deep_clone().unknown(true),
            jobs: self.jobs.unknown(),
            instances: self.instances.clone(),
        }
    }

    fn configure_with_params(meta: &mut ConstraintSystem<F>, params: Self::Params) -> Self::Config {
        let usable_rows = (1_usize << params.k) - UNUSABLE_ROWS;
        let mut base = BaseConfig::configure(meta, params);
        base.set_usable_rows(usable_rows);
        Self::Config {
            base,
            sha: PastaSha256ConfigV1::configure(meta),
        }
    }

    fn configure(_: &mut ConstraintSystem<F>) -> Self::Config {
        unreachable!("bounded SHA tests require explicit Base parameters")
    }

    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<F>,
    ) -> Result<(), Error> {
        <BaseCircuitBuilder<F> as Circuit<F>>::synthesize(
            &self.builder,
            config.base,
            layouter.namespace(|| "bounded SHA Base"),
        )?;
        self.jobs.synthesize(
            &config.sha,
            &mut layouter,
            &self.builder.core().copy_manager,
            (1_usize << TEST_K) - UNUSABLE_ROWS,
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mutation {
    None,
    Length,
    ZeroTail,
    Marker,
    BitLength,
    FinalSelector,
    NonFinalSnapshot,
    Chain,
}

fn digest_words<F: BigPrimeField>(message: &[u8]) -> [F; DIGEST_SIZE] {
    let digest: [u8; 32] = Sha256::digest(message).into();
    std::array::from_fn(|index| {
        F::from(u64::from(u32::from_be_bytes(
            digest[index * 4..index * 4 + 4]
                .try_into()
                .expect("native SHA word"),
        )))
    })
}

fn assign_message<F: BigPrimeField>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    message: &[u8],
) -> Vec<PastaSha256ByteV1<F>> {
    message
        .iter()
        .map(|byte| {
            let cell = ctx.load_witness(F::from(u64::from(*byte)));
            PastaSha256ByteV1::range_checked(ctx, range, cell)
        })
        .collect()
}

/// Change every Base copy of a target, so a negative test cannot pass merely by breaking one
/// existing copy edge. The constraints consuming the changed value must reject it themselves.
fn replace_base_equivalence_class<F: BigPrimeField>(
    builder: &mut BaseCircuitBuilder<F>,
    target: AssignedValue<F>,
    value: F,
) {
    let target = target.cell.expect("mutation target has a virtual cell");
    let equalities = builder
        .core()
        .copy_manager
        .lock()
        .expect("copy manager")
        .advice_equalities
        .clone();
    let mut cells = BTreeSet::from([target]);
    loop {
        let previous_len = cells.len();
        for (left, right) in &equalities {
            if cells.contains(left) || cells.contains(right) {
                cells.insert(*left);
                cells.insert(*right);
            }
        }
        if cells.len() == previous_len {
            break;
        }
    }
    for cell in cells {
        assert_eq!(cell.type_id(), target.type_id());
        assert_eq!(cell.context_id(), 0);
        builder
            .main(0)
            .replace_advice_with_trivial(cell.offset(), value);
    }
}

fn bounded_circuit<F: BigPrimeField>(
    capacities_and_lengths: &[(usize, usize)],
    mutation: Mutation,
) -> BoundedCircuit<F> {
    let mut builder = BaseCircuitBuilder::<F>::new(false)
        .use_k(TEST_K as usize)
        .use_lookup_bits(15)
        .use_instance_columns(1);
    let range = builder.range_chip();
    let mut jobs = PastaSha256JobsV1::default();
    let mut public_cells = Vec::new();
    let mut instances = Vec::new();
    let prefix = b"fixed job before bounded SHA";
    let assigned_prefix = assign_message(builder.main(0), &range, prefix);
    public_cells.extend(
        jobs.digest_constrained(builder.main(0), &assigned_prefix)
            .expect("fixed prefix SHA"),
    );
    instances.extend(digest_words::<F>(prefix));
    let mut first_length = None;
    for (index, &(capacity, length)) in capacities_and_lengths.iter().enumerate() {
        assert!(length <= capacity);
        let mut bytes = (0..capacity)
            .map(|offset| {
                if offset < length {
                    u8::try_from(offset % 256)
                        .expect("byte remainder")
                        .wrapping_mul(37)
                        .wrapping_add(11)
                } else {
                    0
                }
            })
            .collect::<Vec<_>>();
        instances.extend(digest_words::<F>(&bytes[..length]));
        if index == 0 && mutation == Mutation::ZeroTail {
            assert!(length < capacity);
            bytes[capacity - 1] = 1;
        }
        let message = assign_message(builder.main(0), &range, &bytes);
        let length_cell = builder
            .main(0)
            .load_witness(F::from(u64::try_from(length).expect("test length")));
        if index == 0 {
            first_length = Some(length_cell);
        }
        public_cells.extend(
            jobs.digest_bounded_constrained(builder.main(0), &range, &message, length_cell)
                .expect("bounded SHA relation construction"),
        );
    }
    let suffix = b"fixed job after bounded SHA";
    let assigned_suffix = assign_message(builder.main(0), &range, suffix);
    public_cells.extend(
        jobs.digest_constrained(builder.main(0), &assigned_suffix)
            .expect("fixed suffix SHA"),
    );
    instances.extend(digest_words::<F>(suffix));
    match mutation {
        Mutation::None | Mutation::ZeroTail => {}
        Mutation::Length => {
            let length = first_length.expect("first bounded length");
            replace_base_equivalence_class(&mut builder, length, *length.value() + F::ONE);
        }
        Mutation::Marker => {
            jobs = jobs.with_source_xor(1, capacities_and_lengths[0].1, 1);
        }
        Mutation::BitLength => {
            let length = capacities_and_lengths[0].1;
            let padded_length = length + canonical_padding_suffix(length).unwrap().len();
            jobs = jobs.with_source_xor(1, padded_length - 1, 1);
        }
        Mutation::FinalSelector => {
            let selector = jobs.jobs[1].bounded.as_ref().unwrap().final_block_selectors[0];
            assert_eq!(*selector.value(), F::ZERO);
            replace_base_equivalence_class(&mut builder, selector, F::ONE);
        }
        Mutation::NonFinalSnapshot => {
            // At length 65 the first block is not the selected digest. Mutating all its Base
            // copies must nevertheless fail the independent Table16 snapshot binding.
            let snapshot = jobs.jobs[1].bounded.as_ref().unwrap().block_outputs[0][0];
            replace_base_equivalence_class(&mut builder, snapshot, *snapshot.value() + F::ONE);
        }
        Mutation::Chain => {
            // One fixed-prefix block precedes the bounded job, so its second block is global 2.
            jobs = jobs.with_broken_chain(2);
        }
    }
    builder.assigned_instances = vec![public_cells];
    builder.calculate_params(Some(UNUSABLE_ROWS));
    BoundedCircuit {
        builder,
        jobs,
        instances,
    }
}

fn assert_case<F: BigPrimeField>(cases: &[(usize, usize)], mutation: Mutation, expected: bool) {
    let circuit = bounded_circuit::<F>(cases, mutation);
    let result = MockProver::run(TEST_K, &circuit, vec![circuit.instances.clone()])
        .expect("actual bounded Base/Table16 synthesis")
        .verify();
    assert_eq!(result.is_ok(), expected, "{mutation:?}: {result:?}");
}

#[test]
fn bounded_sha_padding_boundaries_match_standard_sha_in_both_fields() {
    let mut cases = vec![(0, 0), (55, 55), (56, 56), (63, 63), (64, 64)];
    cases.extend([0, 1, 55, 56, 63, 64, 65, 119, 120, 127, 128].map(|length| (128, length)));
    assert_case::<Fp>(&cases, Mutation::None, true);
    assert_case::<Fq>(&cases, Mutation::None, true);
}

#[test]
fn bounded_sha_rejects_length_tail_padding_selection_snapshot_and_chain_mutations() {
    for mutation in [
        Mutation::Length,
        Mutation::ZeroTail,
        Mutation::Marker,
        Mutation::BitLength,
        Mutation::FinalSelector,
        Mutation::NonFinalSnapshot,
        Mutation::Chain,
    ] {
        assert_case::<Fp>(&[(128, 65)], mutation, false);
        assert_case::<Fq>(&[(128, 65)], mutation, false);
    }
}

fn assert_same_shape<F: BigPrimeField>(
    left: &mut BoundedCircuit<F>,
    right: &mut BoundedCircuit<F>,
) {
    let a = &left.builder.config_params;
    let b = &right.builder.config_params;
    assert_eq!(a.k, b.k);
    assert_eq!(a.num_advice_per_phase, b.num_advice_per_phase);
    assert_eq!(a.num_fixed, b.num_fixed);
    assert_eq!(a.num_lookup_advice_per_phase, b.num_lookup_advice_per_phase);
    assert_eq!(a.lookup_bits, b.lookup_bits);
    assert_eq!(a.num_instance_columns, b.num_instance_columns);
    assert_eq!(
        left.builder.main(0).advice_len(),
        right.builder.main(0).advice_len()
    );
    assert_eq!(
        left.builder.main(0).selector.iter().collect::<Vec<_>>(),
        right.builder.main(0).selector.iter().collect::<Vec<_>>()
    );
    assert_eq!(
        left.builder.statistics().total_lookup_advice_per_phase,
        right.builder.statistics().total_lookup_advice_per_phase
    );
    let left_copy = left.builder.core().copy_manager.lock().unwrap();
    let right_copy = right.builder.core().copy_manager.lock().unwrap();
    assert_eq!(left_copy.advice_equalities, right_copy.advice_equalities);
    assert_eq!(
        left_copy.constant_equalities.iter().collect::<Vec<_>>(),
        right_copy.constant_equalities.iter().collect::<Vec<_>>()
    );
    assert_eq!(left.jobs.shape(), right.jobs.shape());
    assert_eq!(left.jobs.capacity_profile(), right.jobs.capacity_profile());
    for (left_job, right_job) in left.jobs.jobs.iter().zip(&right.jobs.jobs) {
        let cells = |job: &PastaSha256JobV1<F>| -> Vec<Option<ContextCell>> {
            let mut cells = job
                .message
                .iter()
                .map(|byte| byte.assigned().and_then(|a| a.cell))
                .collect::<Vec<_>>();
            cells.extend(job.output_words.iter().map(|word| word.cell));
            if let Some(bounded) = &job.bounded {
                assert!(bounded.padded.iter().all(|byte| byte.assigned().is_some()));
                cells.extend(
                    bounded
                        .padded
                        .iter()
                        .map(|byte| byte.assigned().and_then(|a| a.cell)),
                );
                cells.extend(bounded.block_outputs.iter().flatten().map(|word| word.cell));
                cells.extend(
                    bounded
                        .final_block_selectors
                        .iter()
                        .map(|selector| selector.cell),
                );
            }
            cells
        };
        assert_eq!(left_job.bounded.is_some(), right_job.bounded.is_some());
        assert_eq!(cells(left_job), cells(right_job));
    }
}

fn shape_cases<F: BigPrimeField>() {
    let mut baseline = bounded_circuit::<F>(&[(128, 0)], Mutation::None);
    for length in [1, 55, 56, 63, 64, 65, 119, 120, 127, 128] {
        let mut candidate = bounded_circuit::<F>(&[(128, length)], Mutation::None);
        assert_same_shape(&mut baseline, &mut candidate);
    }
    let mut witnessless = baseline.without_witnesses();
    assert!(witnessless.jobs.use_unknown);
    assert_same_shape(&mut baseline, &mut witnessless);
}

#[test]
fn bounded_sha_lengths_and_witness_stripping_preserve_exact_constraint_shape() {
    shape_cases::<Fp>();
    shape_cases::<Fq>();
}

#[test]
fn bounded_sha_capacity_accounts_for_all_snapshots_without_changing_fixed_jobs() {
    let circuit = bounded_circuit::<Fp>(&[(128, 0)], Mutation::None);
    assert_eq!(circuit.jobs.shape(), vec![28, 128, 27]);
    assert_eq!(circuit.jobs.compression_blocks().unwrap(), 5);
    let bounded = circuit.jobs.jobs[1].clone();
    let mut repeated = PastaSha256JobsV1::default();
    repeated.jobs = vec![bounded; 50];
    let expected_rows = 30 * SHA256_ROWS_PER_BLOCK_V1
        + 50 * SHA256_ROWS_PER_JOB_V1
        + 100 * SHA256_ROWS_PER_EXTRA_SNAPSHOT_V1;
    assert_eq!(
        repeated.capacity_profile().unwrap(),
        (50, 150, expected_rows)
    );
    assert!(repeated.validate_capacity(SHA256_TABLE_ROWS_V1).is_err());
    assert!(repeated.validate_capacity(expected_rows).is_ok());
}

#[test]
fn bounded_sha_rejects_native_over_capacity_and_non_byte_inputs() {
    let mut builder = BaseCircuitBuilder::<Fp>::new(false)
        .use_k(TEST_K as usize)
        .use_lookup_bits(15);
    let range = builder.range_chip();
    let message = assign_message(builder.main(0), &range, &[0; 8]);
    let length = builder.main(0).load_witness(Fp::from(9));
    let mut jobs = PastaSha256JobsV1::default();
    assert!(
        jobs.digest_bounded_constrained(builder.main(0), &range, &message, length)
            .is_err()
    );
    let source = builder.main(0).load_witness(Fp::from(256));
    let invalid = PastaSha256ByteV1::range_checked(builder.main(0), &range, source);
    let length = builder.main(0).load_witness(Fp::ONE);
    assert!(
        jobs.digest_bounded_constrained(builder.main(0), &range, &[invalid], length)
            .is_err()
    );
    assert!(jobs.jobs.is_empty());
}
