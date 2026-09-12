//! Differential native BUS schedule, absolute-coordinate and seeded-proof checks.
//!
//! The reference function below is retained verbatim from the pre-change synthesizer except
//! for its free-function receiver. Full Base/Claim integration and resource qualification are
//! separate gates; this fixture preserves actual returned source Cells under real V1 placement.

use std::collections::BTreeMap;

use ff::Field;

use super::*;
use halo2_base::{
    ContextCell,
    halo2_proofs::{
        circuit::V1,
        dev::MockProver,
        halo2curves::pasta::{Fp, Fq},
        plonk::{Any, Assigned, Assignment, Challenge, Circuit, FloorPlanner, Instance, Selector},
    },
};

const K: u32 = 9;
const USABLE: usize = (1 << K) - 9;
const SOURCE_ID: &str = "native-monotone-poseidon-test";
#[allow(clippy::too_many_arguments)]
fn reference_synthesize<F: KagemushaPoseidonFieldV1>(
    jobs: &PastaNativePoseidonJobsV1<F>,
    config: &PastaNativePoseidonConfigV1,
    layouter: &mut impl Layouter<F>,
    copy_manager: &SharedCopyConstraintManager<F>,
    witness_gen_only: bool,
    usable_rows: usize,
    mutation: Option<NativePoseidonMutation>,
) -> Result<(), Error> {
    let rows = jobs.required_rows().map_err(|_| Error::Synthesis)?;
    if config.lanes.len() != jobs.lane_count
        || usable_rows != jobs.usable_rows
        || rows > usable_rows
    {
        return Err(Error::Synthesis);
    }
    if rows == 0 {
        return Ok(());
    }
    let physical = if witness_gen_only {
        None
    } else {
        Some(copy_manager.lock().map_err(|_| Error::Synthesis)?)
    };
    layouter.assign_region(
        || "native Pasta exact raw Poseidon",
        |mut region| {
            for row in 0..rows {
                let round = row % PERMUTATION_ROWS;
                let active = round < ROUNDS;
                for i in 0..WIDTH {
                    region.assign_fixed(
                        config.constants[i],
                        row,
                        if active {
                            jobs.spec.constants[round][i]
                        } else {
                            F::ZERO
                        },
                    );
                }
                region.assign_fixed(
                    config.round_mode,
                    row,
                    if !active {
                        F::ZERO
                    } else if full_round(round) {
                        F::from(2)
                    } else {
                        F::ONE
                    },
                );
                let mut code = 0;
                let mut bus_slot = false;
                for lane in 0..jobs.lane_count {
                    for output in [false, true] {
                        let start = bridge_start(lane, output);
                        if round == start {
                            code = bridge_code(lane, output);
                        }
                        bus_slot |= (start..start + WIDTH).contains(&round);
                    }
                }
                region.assign_fixed(config.bridge_mode, row, F::from(code));
                if !bus_slot {
                    region.assign_advice_discarding_value(
                        config.bus,
                        row,
                        if jobs.use_unknown {
                            Value::unknown()
                        } else {
                            Value::known(F::ZERO)
                        },
                    );
                }
            }
            for (lane, columns) in config.lanes.iter().enumerate() {
                for block in 0..rows / PERMUTATION_ROWS {
                    let job_index = block * jobs.lane_count + lane;
                    let job = jobs.jobs.get(job_index);
                    // The final unused lane still has its complete round trace and BUS
                    // bridges under the shared fixed schedule, with no external copies.
                    let mut state =
                        job.map_or([F::ZERO; WIDTH], |job| job.input.map(|cell| *cell.value()));
                    for round in 0..=ROUNDS {
                        let row = block * PERMUTATION_ROWS + round;
                        for i in 0..WIDTH {
                            let mut value = state[i];
                            if mutation
                                == Some(NativePoseidonMutation::Trace {
                                    job: job_index,
                                    round,
                                    column: i,
                                })
                            {
                                value += F::ONE;
                            }
                            region.assign_advice_discarding_value(
                                columns[i],
                                row,
                                if jobs.use_unknown {
                                    Value::unknown()
                                } else {
                                    Value::known(value)
                                },
                            );
                            if round == 0 || round == ROUNDS {
                                let output = round == ROUNDS;
                                let mut value = state[i];
                                if mutation
                                    == Some(NativePoseidonMutation::Bus {
                                        job: job_index,
                                        output,
                                        column: i,
                                    })
                                {
                                    value += F::ONE;
                                }
                                let bus_row =
                                    block * PERMUTATION_ROWS + bridge_start(lane, output) + i;
                                let assigned = region.assign_advice_discarding_value(
                                    config.bus,
                                    bus_row,
                                    if jobs.use_unknown {
                                        Value::unknown()
                                    } else {
                                        Value::known(value)
                                    },
                                );
                                if let (Some(job), Some(physical)) = (job, &physical) {
                                    let bridge = if output { job.output[i] } else { job.input[i] };
                                    let virtual_cell = bridge.cell.ok_or(Error::Synthesis)?;
                                    let target = physical
                                        .assigned_advices
                                        .resolve(&virtual_cell)
                                        .ok_or(Error::Synthesis)?;
                                    // Only BUS participates in equality. Its fixed bridge gate
                                    // binds the original Base cell to the local state column.
                                    region.constrain_equal(assigned, target);
                                }
                            }
                        }
                        if round < ROUNDS {
                            state = jobs.spec.transition(state, round);
                        }
                    }
                }
            }
            Ok(())
        },
    )
}

#[derive(Clone)]
struct Fixture<F: KagemushaPoseidonFieldV1, const OLD: bool> {
    jobs: PastaNativePoseidonJobsV1<F>,
    values: Vec<F>,
    padding: usize,
    mutation: Option<NativePoseidonMutation>,
    omit_source: bool,
}

#[derive(Clone)]
struct Config {
    source: Column<Advice>,
    native: PastaNativePoseidonConfigV1,
}

impl<F: KagemushaPoseidonFieldV1, const OLD: bool> Circuit<F> for Fixture<F, OLD> {
    type Config = Config;
    type FloorPlanner = V1;
    type Params = usize;

    fn params(&self) -> usize {
        self.jobs.lane_count
    }
    fn without_witnesses(&self) -> Self {
        Self {
            jobs: self.jobs.clone().unknown(),
            values: self.values.clone(),
            padding: self.padding,
            mutation: self.mutation,
            omit_source: self.omit_source,
        }
    }
    fn configure(_: &mut ConstraintSystem<F>) -> Config {
        unreachable!("parameterized native fixture")
    }
    fn configure_with_params(meta: &mut ConstraintSystem<F>, lanes: usize) -> Config {
        let source = meta.advice_column();
        meta.enable_equality(source);
        Config {
            source,
            native: PastaNativePoseidonConfigV1::configure::<F>(meta, lanes),
        }
    }
    fn synthesize(&self, config: Config, mut layouter: impl Layouter<F>) -> Result<(), Error> {
        // Bigger advice area places this region first under V1, forcing a nonzero native start.
        if self.padding != 0 {
            layouter.assign_region(
                || "native column prefix",
                |mut region| {
                    for row in 0..self.padding {
                        for columns in &config.native.lanes {
                            for column in columns {
                                region.assign_advice_discarding_value(
                                    *column,
                                    row,
                                    Value::known(F::ZERO),
                                );
                            }
                        }
                        region.assign_advice_discarding_value(
                            config.native.bus,
                            row,
                            Value::known(F::ZERO),
                        );
                    }
                    Ok(())
                },
            )?;
        }
        let manager: SharedCopyConstraintManager<F> = Default::default();
        layouter.assign_region(
            || "original physical bridge cells",
            |mut region| {
                let mut copies = manager.lock().map_err(|_| Error::Synthesis)?;
                for (row, value) in self.values.iter().copied().enumerate() {
                    let cell = region.assign_advice_discarding_value(
                        config.source,
                        row,
                        if self.jobs.use_unknown {
                            Value::unknown()
                        } else {
                            Value::known(value)
                        },
                    );
                    if !self.omit_source || row != 0 {
                        copies
                            .assigned_advices
                            .insert(ContextCell::new(SOURCE_ID, 0, row), cell);
                    }
                }
                Ok(())
            },
        )?;
        if OLD {
            reference_synthesize(
                &self.jobs,
                &config.native,
                &mut layouter,
                &manager,
                false,
                USABLE,
                self.mutation,
            )
        } else {
            self.jobs.synthesize_inner(
                &config.native,
                &mut layouter,
                &manager,
                false,
                USABLE,
                self.mutation,
            )
        }
    }
}

fn assigned<F: KagemushaPoseidonFieldV1>(values: &mut Vec<F>, value: F) -> AssignedValue<F> {
    let row = values.len();
    values.push(value);
    AssignedValue {
        value: Assigned::Trivial(value),
        cell: Some(ContextCell::new(SOURCE_ID, 0, row)),
    }
}

fn fixture<F: KagemushaPoseidonFieldV1, const OLD: bool>(
    lanes: usize,
    count: usize,
    padding: usize,
) -> Fixture<F, OLD> {
    let mut jobs = PastaNativePoseidonJobsV1::new(lanes, USABLE).unwrap();
    let mut values = Vec::new();
    for job in 0..count {
        let input = std::array::from_fn(|column| F::from((100 * job + column + 1) as u64));
        let output = jobs.spec.permutation(input);
        jobs.jobs.push(PermutationJob {
            input: input.map(|value| assigned(&mut values, value)),
            output: output.map(|value| assigned(&mut values, value)),
        });
        jobs.hashes.push(HashInventory {
            input_count: 0,
            first_permutation: job,
            permutations: 1,
        });
    }
    Fixture {
        jobs,
        values,
        padding,
        mutation: None,
        omit_source: false,
    }
}

#[derive(Debug)]
struct Record<F> {
    advice: BTreeMap<(usize, usize), Option<F>>,
    advice_order: Vec<(usize, usize)>,
    fixed: Vec<(usize, usize, F)>,
    selectors: Vec<(usize, usize)>,
    copies: Vec<(Column<Any>, usize, Column<Any>, usize)>,
    last: BTreeMap<usize, usize>,
    nonmonotone: Vec<(usize, usize, usize)>,
}

impl<F> Default for Record<F> {
    fn default() -> Self {
        Self {
            advice: BTreeMap::new(),
            advice_order: Vec::new(),
            fixed: Vec::new(),
            selectors: Vec::new(),
            copies: Vec::new(),
            last: BTreeMap::new(),
            nonmonotone: Vec::new(),
        }
    }
}

impl<F: KagemushaPoseidonFieldV1> Assignment<F> for Record<F> {
    fn enter_region<NR: Into<String>, N: FnOnce() -> NR>(&mut self, _: N) {}
    fn annotate_column<A: FnOnce() -> AR, AR: Into<String>>(&mut self, _: A, _: Column<Any>) {}
    fn exit_region(&mut self) {}
    fn enable_selector<A: FnOnce() -> AR, AR: Into<String>>(
        &mut self,
        _: A,
        selector: &Selector,
        row: usize,
    ) -> Result<(), Error> {
        self.selectors.push((selector.index(), row));
        Ok(())
    }
    fn query_instance(&self, _: Column<Instance>, _: usize) -> Result<Value<F>, Error> {
        Ok(Value::unknown())
    }
    fn assign_advice<'v>(
        &mut self,
        _: Column<Advice>,
        _: usize,
        _: Value<Assigned<F>>,
    ) -> Value<&'v Assigned<F>> {
        panic!("native fixture requested a retained advice reference")
    }
    fn assign_advice_discarding_value(
        &mut self,
        column: Column<Advice>,
        row: usize,
        to: Value<Assigned<F>>,
    ) {
        let mut value = None;
        to.map(|assigned| {
            value = Some(assigned.evaluate());
        });
        let index = column.index();
        assert!(
            self.advice.insert((index, row), value).is_none(),
            "duplicate physical advice write"
        );
        self.advice_order.push((index, row));
        if let Some(previous) = self.last.insert(index, row) {
            if row <= previous {
                self.nonmonotone.push((index, previous, row));
            }
        }
    }
    fn assign_fixed(&mut self, column: Column<Fixed>, row: usize, value: Assigned<F>) {
        self.fixed.push((column.index(), row, value.evaluate()));
    }
    fn copy(&mut self, left: Column<Any>, left_row: usize, right: Column<Any>, right_row: usize) {
        self.copies.push((left, left_row, right, right_row));
    }
    fn fill_from_row(
        &mut self,
        _: Column<Fixed>,
        _: usize,
        _: Value<Assigned<F>>,
    ) -> Result<(), Error> {
        Err(Error::Synthesis)
    }
    fn get_challenge(&self, _: Challenge) -> Value<F> {
        Value::unknown()
    }
    fn push_namespace<NR: Into<String>, N: FnOnce() -> NR>(&mut self, _: N) {}
    fn pop_namespace(&mut self, _: Option<String>) {}
    fn next_phase(&mut self) {
        panic!("native fixture unexpectedly changed phase");
    }
}

fn record<F: KagemushaPoseidonFieldV1, const OLD: bool>(
    circuit: &Fixture<F, OLD>,
) -> Result<(Record<F>, Config), Error> {
    let mut cs = ConstraintSystem::default();
    let config = Fixture::<F, OLD>::configure_with_params(&mut cs, circuit.params());
    let mut recorder = Record::default();
    V1::synthesize(&mut recorder, circuit, config.clone(), vec![])?;
    Ok((recorder, config))
}

fn differential<F: KagemushaPoseidonFieldV1>() {
    for (lanes, count, padding) in [(1, 3, 201), (2, 5, 201), (2, 6, 201), (2, 1, 0)] {
        let old = fixture::<F, true>(lanes, count, padding);
        let new = fixture::<F, false>(lanes, count, padding);
        let (reference, _) = record(&old).unwrap();
        let (actual, config) = record(&new).unwrap();
        assert_eq!(actual.advice, reference.advice);
        assert_eq!(
            actual.fixed, reference.fixed,
            "fixed assignment order changed"
        );
        assert_eq!(actual.selectors, reference.selectors);
        assert_eq!(
            actual.copies, reference.copies,
            "permutation insertion order changed"
        );
        assert!(
            !reference.nonmonotone.is_empty(),
            "control must expose old BUS ordering"
        );
        assert!(
            actual.nonmonotone.is_empty(),
            "new absolute assignments must be monotone per column"
        );
        let bus = config.native.bus.index();
        let native_rows = new.jobs.required_rows().unwrap();
        let bus_rows: Vec<_> = actual
            .advice_order
            .iter()
            .filter_map(|(column, row)| (*column == bus).then_some(*row))
            .collect();
        assert_eq!(bus_rows, (0..padding + native_rows).collect::<Vec<_>>());
        for columns in &config.native.lanes {
            for column in columns {
                let old_rows: Vec<_> = reference
                    .advice_order
                    .iter()
                    .filter(|(index, _)| *index == column.index())
                    .copied()
                    .collect();
                let new_rows: Vec<_> = actual
                    .advice_order
                    .iter()
                    .filter(|(index, _)| *index == column.index())
                    .copied()
                    .collect();
                assert_eq!(
                    old_rows, new_rows,
                    "per-column trace assignment order changed"
                );
            }
        }
        MockProver::run(K, &old, vec![]).unwrap().assert_satisfied();
        MockProver::run(K, &new, vec![]).unwrap().assert_satisfied();
        let (old_unknown, _) = record(&old.without_witnesses()).unwrap();
        let (new_unknown, _) = record(&new.without_witnesses()).unwrap();
        assert_eq!(old_unknown.advice, new_unknown.advice);
        assert_eq!(old_unknown.fixed, new_unknown.fixed);
        assert_eq!(old_unknown.copies, new_unknown.copies);
        assert!(new_unknown.nonmonotone.is_empty());
    }
}

#[test]
fn fp_monotone_bus_preserves_values_fixed_copies_and_absolute_placement() {
    differential::<Fp>();
}
#[test]
fn fq_monotone_bus_preserves_values_fixed_copies_and_absolute_placement() {
    differential::<Fq>();
}

fn mutation_differential<F: KagemushaPoseidonFieldV1>() {
    // Five active jobs leave the second lane of the third block inactive.
    for job in [0, 1, 4, 5] {
        for column in 0..WIDTH {
            let mut mutations = vec![
                NativePoseidonMutation::Bus {
                    job,
                    output: false,
                    column,
                },
                NativePoseidonMutation::Bus {
                    job,
                    output: true,
                    column,
                },
            ];
            mutations.extend(
                [0, 1, 32, ROUNDS].map(|round| NativePoseidonMutation::Trace {
                    job,
                    round,
                    column,
                }),
            );
            for mutation in mutations {
                let mut old = fixture::<F, true>(2, 5, 0);
                let mut new = fixture::<F, false>(2, 5, 0);
                old.mutation = Some(mutation);
                new.mutation = Some(mutation);
                let (reference, _) = record(&old).unwrap();
                let (actual, _) = record(&new).unwrap();
                assert_eq!(actual.advice, reference.advice);
                assert_eq!(actual.fixed, reference.fixed);
                assert_eq!(actual.copies, reference.copies);
                assert!(actual.nonmonotone.is_empty());
                assert!(
                    MockProver::run(K, &new, vec![]).unwrap().verify().is_err(),
                    "mutated trace/BUS must remain constrained"
                );
            }
        }
    }
    let mut old = fixture::<F, true>(2, 5, 0);
    let mut new = fixture::<F, false>(2, 5, 0);
    // Corrupt the claimed output and its source; the computed BUS output must not follow it.
    for circuit in [&mut old.jobs, &mut new.jobs] {
        circuit.jobs[4].output[0].value = Assigned::Trivial(F::from(999));
    }
    old.values[4 * 6 + 3] = F::from(999);
    new.values[4 * 6 + 3] = F::from(999);
    let (reference, _) = record(&old).unwrap();
    let (actual, _) = record(&new).unwrap();
    assert_eq!(actual.advice, reference.advice);
    assert!(MockProver::run(K, &new, vec![]).unwrap().verify().is_err());
}

#[test]
fn fp_trace_bus_and_claimed_output_mutations_keep_original_semantics() {
    mutation_differential::<Fp>();
}
#[test]
fn fq_trace_bus_and_claimed_output_mutations_keep_original_semantics() {
    mutation_differential::<Fq>();
}

#[test]
fn missing_source_stays_an_error_and_block_payload_is_fixed_and_cleared() {
    for count in [1, 5, 6] {
        let mut old = fixture::<Fp, true>(2, count, 0);
        let mut new = fixture::<Fp, false>(2, count, 0);
        old.omit_source = true;
        new.omit_source = true;
        assert!(record(&old).is_err());
        BLOCK_WITNESS_CLEARS.with(|value| value.set((0, true)));
        assert!(record(&new).is_err());
        let (cleared_blocks, all_zero) = BLOCK_WITNESS_CLEARS.with(|value| value.get());
        assert!(cleared_blocks >= count.div_ceil(2));
        assert!(
            all_zero,
            "synthesis-error cleanup must clear every owned block field"
        );
    }
    assert_eq!(std::mem::size_of::<NativePoseidonBlockWitness<Fp>>(), 480);
    assert_eq!(std::mem::size_of::<NativePoseidonBlockWitness<Fq>>(), 480);
    for panic in [false, true] {
        BLOCK_WITNESS_CLEARS.with(|value| value.set((0, true)));
        let result = std::panic::catch_unwind(|| {
            let mut block = NativePoseidonBlockWitness::<Fq>::zeroed();
            block.endpoints.fill([Fq::ONE; WIDTH]);
            block.state.fill(Fq::ONE);
            assert!(!panic, "injected block unwind");
        });
        assert_eq!(result.is_err(), panic);
        assert_eq!(BLOCK_WITNESS_CLEARS.with(|value| value.get()), (1, true));
    }
}

#[test]
fn monotone_native_keys_and_seeded_proofs_match_prior_schedule_both_pasta_fields() {
    use halo2_base::halo2_proofs::{
        SerdeFormat,
        halo2curves::pasta::{EpAffine, EqAffine},
        plonk::{create_proof, keygen_pk2, verify_proof},
        poly::{
            VerificationStrategy as _,
            commitment::ParamsProver,
            ipa::{
                commitment::{IPACommitmentScheme, ParamsIPA},
                multiopen::{ProverIPA, VerifierIPA},
                strategy::SingleStrategy,
            },
        },
        transcript::{
            Blake2bRead, Blake2bWrite, Challenge255, TranscriptReadBuffer, TranscriptWriterBuffer,
        },
    };
    macro_rules! check {
        ($curve:ty, $field:ty) => {{
            for compressed in [false, true] {
                let old = fixture::<$field, true>(2, 5, 201);
                let new = fixture::<$field, false>(2, 5, 201);
                let params = ParamsIPA::<$curve>::new(K);
                let old_pk = keygen_pk2(&params, &old, compressed).unwrap();
                let new_pk = keygen_pk2(&params, &new, compressed).unwrap();
                assert_eq!(
                    old_pk.to_bytes(SerdeFormat::Processed),
                    new_pk.to_bytes(SerdeFormat::Processed)
                );
                assert_eq!(
                    old_pk.get_vk().to_bytes(SerdeFormat::Processed),
                    new_pk.get_vk().to_bytes(SerdeFormat::Processed)
                );
                let instances: &[&[&[$field]]] = &[&[]];
                let seed =
                    iroha_crypto::kagemusha::KagemushaRecoverySeedV1::from_unsealed([79; 32])
                        .unwrap();
                let mut old_transcript =
                    Blake2bWrite::<_, $curve, Challenge255<$curve>>::init(Vec::new());
                create_proof::<IPACommitmentScheme<$curve>, ProverIPA<$curve>, _, _, _, _>(
                    &params,
                    &old_pk,
                    &[old],
                    instances,
                    seed.rng(b"native-bus-monotone-test", &[0; 32]).unwrap(),
                    &mut old_transcript,
                )
                .unwrap();
                let mut new_transcript =
                    Blake2bWrite::<_, $curve, Challenge255<$curve>>::init(Vec::new());
                create_proof::<IPACommitmentScheme<$curve>, ProverIPA<$curve>, _, _, _, _>(
                    &params,
                    &new_pk,
                    &[new],
                    instances,
                    seed.rng(b"native-bus-monotone-test", &[0; 32]).unwrap(),
                    &mut new_transcript,
                )
                .unwrap();
                let old_bytes = old_transcript.finalize();
                let new_bytes = new_transcript.finalize();
                assert_eq!(old_bytes, new_bytes);
                let mut reader =
                    Blake2bRead::<_, $curve, Challenge255<$curve>>::init(new_bytes.as_slice());
                verify_proof::<IPACommitmentScheme<$curve>, VerifierIPA<$curve>, _, _, _>(
                    &params,
                    new_pk.get_vk(),
                    SingleStrategy::new(&params),
                    instances,
                    &mut reader,
                )
                .unwrap();
                let mut corrupted = new_bytes;
                let last = corrupted.len() - 1;
                corrupted[last] ^= 1;
                let mut reader =
                    Blake2bRead::<_, $curve, Challenge255<$curve>>::init(corrupted.as_slice());
                assert!(
                    verify_proof::<IPACommitmentScheme<$curve>, VerifierIPA<$curve>, _, _, _>(
                        &params,
                        new_pk.get_vk(),
                        SingleStrategy::new(&params),
                        instances,
                        &mut reader
                    )
                    .is_err()
                );
            }
        }};
    }
    check!(EqAffine, Fp);
    check!(EpAffine, Fq);
}
