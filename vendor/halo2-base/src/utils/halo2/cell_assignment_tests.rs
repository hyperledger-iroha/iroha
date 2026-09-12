//! Cell-only assignment regressions against the frozen original assignment algorithms.

use std::{
    collections::BTreeSet,
    marker::PhantomData,
    sync::{Arc, Mutex},
};

use crate::{
    QuantumCell,
    ff::{Field, WithSmallOrderMulGroup},
    gates::{
        circuit::{BaseCircuitParams, BaseConfig, builder::BaseCircuitBuilder},
        flex_gate::GateInstructions,
        range::RangeInstructions,
    },
    halo2_proofs::{
        circuit::{Cell, Layouter, Value, floor_planner::V1},
        halo2curves::{
            CurveAffine,
            pasta::{EpAffine, EqAffine, Fp, Fq},
        },
        plonk::{
            Advice, Any, Assigned, Assignment, Challenge, Circuit, Column, ConstraintSystem, Error,
            Fixed, FloorPlanner, Instance, Selector,
        },
    },
    utils::ScalarField,
    virtual_region::manager::VirtualRegionManager,
};

use super::raw_assign_advice_cell;

mod baseline;

#[derive(Clone, Debug, PartialEq, Eq)]
enum Payload<F> {
    Unknown,
    Zero,
    Trivial(F),
    Rational(F, F),
}

fn payload<F: Field>(value: Value<Assigned<F>>) -> Payload<F> {
    let mut result = Payload::Unknown;
    value.map(|value| {
        result = match value {
            Assigned::Zero => Payload::Zero,
            Assigned::Trivial(value) => Payload::Trivial(value),
            Assigned::Rational(numerator, denominator) => Payload::Rational(numerator, denominator),
        };
    });
    result
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Event<F> {
    Advice(Column<Advice>, usize, Payload<F>),
    Fixed(Column<Fixed>, usize, Payload<F>),
    Selector(usize, usize),
    Copy(Column<Any>, usize, Column<Any>, usize),
    Fill(Column<Fixed>, usize, Payload<F>),
}

struct Recorder<F> {
    events: Vec<Event<F>>,
    references: usize,
    reject_references: bool,
}

impl<F> Recorder<F> {
    fn new(reject_references: bool) -> Self {
        Self {
            events: Vec::new(),
            references: 0,
            reject_references,
        }
    }
}

impl<F: Field> Assignment<F> for Recorder<F> {
    fn enter_region<NR, N>(&mut self, _: N)
    where
        NR: Into<String>,
        N: FnOnce() -> NR,
    {
    }

    fn exit_region(&mut self) {}

    fn annotate_column<A, AR>(&mut self, _: A, _: Column<Any>)
    where
        A: FnOnce() -> AR,
        AR: Into<String>,
    {
    }

    fn enable_selector<A, AR>(&mut self, _: A, selector: &Selector, row: usize) -> Result<(), Error>
    where
        A: FnOnce() -> AR,
        AR: Into<String>,
    {
        self.events.push(Event::Selector(selector.index(), row));
        Ok(())
    }

    fn query_instance(&self, _: Column<Instance>, _: usize) -> Result<Value<F>, Error> {
        // These fixtures expose instances by copy constraints and never query their values.
        Err(Error::BoundsFailure)
    }

    fn assign_advice<'v>(
        &mut self,
        column: Column<Advice>,
        row: usize,
        value: Value<Assigned<F>>,
    ) -> Value<&'v Assigned<F>> {
        assert!(
            !self.reject_references,
            "candidate requested an advice reference"
        );
        self.references += 1;
        self.events.push(Event::Advice(column, row, payload(value)));
        // The frozen baseline reads only .cell(), never the value. Like a measurement
        // backend, this recorder provides no reference and never fabricates a witness.
        Value::unknown()
    }

    fn assign_advice_discarding_value(
        &mut self,
        column: Column<Advice>,
        row: usize,
        value: Value<Assigned<F>>,
    ) {
        self.events.push(Event::Advice(column, row, payload(value)));
    }

    fn assign_fixed(&mut self, column: Column<Fixed>, row: usize, value: Assigned<F>) {
        self.events
            .push(Event::Fixed(column, row, payload(Value::known(value))));
    }

    fn copy(&mut self, left: Column<Any>, left_row: usize, right: Column<Any>, right_row: usize) {
        self.events
            .push(Event::Copy(left, left_row, right, right_row));
    }

    fn fill_from_row(
        &mut self,
        column: Column<Fixed>,
        row: usize,
        value: Value<Assigned<F>>,
    ) -> Result<(), Error> {
        self.events.push(Event::Fill(column, row, payload(value)));
        Ok(())
    }

    fn get_challenge(&self, _: Challenge) -> Value<F> {
        Value::unknown()
    }

    fn push_namespace<NR, N>(&mut self, _: N)
    where
        NR: Into<String>,
        N: FnOnce() -> NR,
    {
    }

    fn pop_namespace(&mut self, _: Option<String>) {}

    fn next_phase(&mut self) {
        panic!("first-phase fixture unexpectedly advanced phase")
    }
}

#[derive(Clone)]
struct BaseFixture<F: ScalarField, const BASELINE: bool> {
    builder: BaseCircuitBuilder<F>,
}

impl<F: ScalarField, const BASELINE: bool> Circuit<F> for BaseFixture<F, BASELINE> {
    type Config = BaseConfig<F>;
    type FloorPlanner = V1;
    type Params = BaseCircuitParams;

    fn params(&self) -> Self::Params {
        self.builder.config_params.clone()
    }

    fn without_witnesses(&self) -> Self {
        Self {
            builder: self.builder.deep_clone().unknown(true),
        }
    }

    fn configure_with_params(meta: &mut ConstraintSystem<F>, params: Self::Params) -> Self::Config {
        BaseConfig::configure(meta, params)
    }

    fn configure(_: &mut ConstraintSystem<F>) -> Self::Config {
        unreachable!("fixture requires explicit parameters")
    }

    fn synthesize_for_measurement(
        &self,
        config: Self::Config,
        layouter: impl Layouter<F>,
    ) -> Result<(), Error> {
        let result = self.synthesize(config, layouter);
        self.builder.reset_synthesis_state();
        result
    }

    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<F>,
    ) -> Result<(), Error> {
        if !BASELINE {
            return <BaseCircuitBuilder<F> as Circuit<F>>::synthesize(
                &self.builder,
                config,
                layouter,
            );
        }
        // Exact fixture-specific composition of the original Base producer. The
        // old gate and lookup bodies are frozen in baseline.rs from source preimages.
        assert!(!self.builder.witness_gen_only());
        config.initialize(&mut layouter);
        let crate::gates::circuit::MaybeRangeConfig::WithRange(range) = &config.base else {
            panic!("baseline fixture must exercise range lookups");
        };
        assert!(
            range.q_lookup[0].is_none(),
            "fixture requires dedicated lookup advice"
        );
        layouter.assign_region(
            || "BaseCircuitBuilder generated circuit",
            |mut region| {
                let phase = &self.builder.core().phase_manager[0];
                let mut manager = phase.copy_manager.lock().unwrap();
                let points = baseline::gate::<F, 4>(
                    &phase.threads,
                    &config.gate().basic_gates[0],
                    &mut region,
                    &mut manager,
                    config.gate().max_rows,
                    phase.use_unknown(),
                );
                drop(manager);
                let mut old = phase.break_points.borrow_mut();
                if let Some(old) = &*old {
                    assert_eq!(old, &points);
                } else {
                    *old = Some(points);
                }
                drop(old);
                let columns = range.lookup_advice[0]
                    .iter()
                    .map(|column| [*column])
                    .collect();
                baseline::lookups(&self.builder.lookup_manager()[0], &columns, &mut region);
                self.builder
                    .core()
                    .copy_manager
                    .assign_raw(config.constants(), &mut region);
                Ok(())
            },
        )?;
        self.builder
            .assign_instances(&config.instance, layouter.namespace(|| "expose"));
        Ok(())
    }
}

fn fixture<F: ScalarField, const BASELINE: bool>() -> BaseFixture<F, BASELINE> {
    let params = BaseCircuitParams {
        k: 6,
        num_advice_per_phase: vec![2],
        num_fixed: 1,
        num_lookup_advice_per_phase: vec![2],
        lookup_bits: Some(4),
        num_instance_columns: 1,
    };
    let mut builder = BaseCircuitBuilder::<F>::new(false).use_params(params);
    // Force a physical column boundary independently of gate/lookup helper growth.
    builder
        .main(0)
        .assign_witnesses((0..80).map(|i| F::from((i % 8) as u64)));
    // Keep variants, including x/0, through the actual gate assigner. The mock
    // recorder and actual IPA prover both preserve the existing zero convention.
    for value in [
        Assigned::Zero,
        Assigned::Rational(F::from(6), F::from(2)),
        Assigned::Rational(F::from(11), F::ZERO),
    ] {
        builder
            .main(0)
            .assign_cell(QuantumCell::WitnessFraction(value));
    }
    let range = builder.range_chip();
    let sum = range.gate().add(
        builder.main(0),
        QuantumCell::Constant(F::from(3)),
        QuantumCell::Witness(F::from(5)),
    );
    range.range_check(builder.main(0), sum, 4);
    // Repeated lookup entries exercise duplicate copy edges and striped columns.
    range.range_check(builder.main(0), sum, 4);
    builder.assigned_instances = vec![vec![sum]];
    BaseFixture { builder }
}

fn record<F: ScalarField, const BASELINE: bool>(fixture: &BaseFixture<F, BASELINE>) -> Recorder<F> {
    let mut meta = ConstraintSystem::default();
    let config = BaseFixture::<F, BASELINE>::configure_with_params(&mut meta, fixture.params());
    let mut recorder = Recorder::new(!BASELINE);
    V1::synthesize(&mut recorder, fixture, config, meta.constants().clone()).unwrap();
    recorder
}

fn compare_records<F: ScalarField>() {
    let candidate = fixture::<F, false>();
    let original = fixture::<F, true>();
    let actual = record(&candidate);
    let expected = record(&original);
    assert_eq!(actual.references, 0);
    assert!(
        expected.references > 80,
        "old gate/lookup branches must actually run"
    );
    assert_eq!(
        actual.events, expected.events,
        "ordered physical assignments and constraints drifted"
    );
    assert_eq!(
        candidate.builder.break_points(),
        original.builder.break_points()
    );
    assert!(!candidate.builder.break_points()[0].is_empty());
    assert!(
        actual
            .events
            .iter()
            .any(|event| matches!(event, Event::Selector(..)))
    );
    assert!(
        actual
            .events
            .iter()
            .any(|event| matches!(event, Event::Fixed(..)))
    );
    assert!(actual.events.iter().any(|event| matches!(event,
        Event::Copy(left, _, right, _) if *left != *right)));
    let advice_columns = actual
        .events
        .iter()
        .filter_map(|event| match event {
            Event::Advice(column, _, _) => Some(column.index()),
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(
        advice_columns.len(),
        4,
        "both gate and both dedicated lookup columns must be assigned"
    );
    assert!(actual.events.iter().any(|event| matches!(event,
        Event::Advice(_, _, Payload::Rational(_, denominator)) if *denominator == F::ZERO)));
    let first_map = candidate
        .builder
        .core()
        .copy_manager
        .lock()
        .unwrap()
        .assigned_advices
        .len();
    let again = record(&candidate);
    assert_eq!(again.events, actual.events);
    assert_eq!(
        candidate
            .builder
            .core()
            .copy_manager
            .lock()
            .unwrap()
            .assigned_advices
            .len(),
        first_map
    );
    assert!(
        !candidate.builder.witness_gen_only(),
        "candidate must preserve constraint mode"
    );
    let unknown_candidate = candidate.without_witnesses();
    let unknown_original = original.without_witnesses();
    let actual_unknown = record(&unknown_candidate);
    let expected_unknown = record(&unknown_original);
    assert_eq!(actual_unknown.references, 0);
    assert!(expected_unknown.references > 80);
    assert_eq!(actual_unknown.events, expected_unknown.events);
    assert!(
        actual_unknown
            .events
            .iter()
            .any(|event| matches!(event, Event::Advice(_, _, Payload::Unknown)))
    );
}

#[test]
fn cell_only_base_preserves_ordered_records_in_fp() {
    compare_records::<Fp>();
}

#[test]
fn cell_only_base_preserves_ordered_records_in_fq() {
    compare_records::<Fq>();
}

#[derive(Clone)]
struct OffsetFixture<F> {
    cells: Arc<Mutex<Vec<Cell>>>,
    marker: PhantomData<F>,
}

impl<F: Field> Circuit<F> for OffsetFixture<F> {
    type Config = Column<Advice>;
    type FloorPlanner = V1;
    type Params = ();

    fn without_witnesses(&self) -> Self {
        self.clone()
    }

    fn configure(meta: &mut ConstraintSystem<F>) -> Self::Config {
        let advice = meta.advice_column();
        meta.enable_equality(advice);
        advice
    }

    fn synthesize(
        &self,
        column: Self::Config,
        mut layouter: impl Layouter<F>,
    ) -> Result<(), Error> {
        // V1 places the larger region first on this shared column. The second
        // region must therefore receive a nonzero absolute starting row.
        layouter.assign_region(
            || "ten-row prefix",
            |mut region| {
                for row in 0..10 {
                    raw_assign_advice_cell(&mut region, column, row, Value::known(F::ZERO));
                }
                Ok(())
            },
        )?;
        layouter.assign_region(
            || "offset cell helper",
            |mut region| {
                let a = raw_assign_advice_cell(&mut region, column, 0, Value::known(F::ONE));
                let b = raw_assign_advice_cell(&mut region, column, 2, Value::known(F::ONE));
                region.constrain_equal(a, b);
                *self.cells.lock().unwrap() = vec![a, b];
                Ok(())
            },
        )
    }
}

#[test]
fn cell_only_helper_returns_actual_offset_cells() {
    let fixture = OffsetFixture::<Fp> {
        cells: Arc::default(),
        marker: PhantomData,
    };
    let mut meta = ConstraintSystem::default();
    let config = OffsetFixture::configure(&mut meta);
    let mut recorder = Recorder::<Fp>::new(true);
    V1::synthesize(&mut recorder, &fixture, config, vec![]).unwrap();
    let cells = fixture.cells.lock().unwrap();
    assert_eq!(cells.len(), 2);
    assert_eq!(cells[0].row_offset, 10);
    assert_eq!(cells[1].row_offset, 12);
    assert_eq!(cells[0].column, config.into());
    assert_eq!(cells[1].column, config.into());
    assert!(
        recorder
            .events
            .contains(&Event::Copy(config.into(), 10, config.into(), 12))
    );
    assert!(
        recorder
            .events
            .contains(&Event::Advice(config, 10, Payload::Trivial(Fp::ONE)))
    );
    assert!(
        recorder
            .events
            .contains(&Event::Advice(config, 12, Payload::Trivial(Fp::ONE)))
    );
    assert_eq!(recorder.references, 0);
}

fn compare_real_ipa<C>()
where
    C: CurveAffine,
    C::ScalarExt: ScalarField + WithSmallOrderMulGroup<3>,
{
    use crate::halo2_proofs::{
        SerdeFormat,
        plonk::{create_proof, keygen_pk, keygen_vk, verify_proof},
        poly::{
            commitment::ParamsProver,
            ipa::{
                commitment::{IPACommitmentScheme, ParamsIPA},
                multiopen::{ProverIPA, VerifierIPA},
                strategy::SingleStrategy,
            },
            strategy::VerificationStrategy,
        },
        transcript::{
            Blake2bRead, Blake2bWrite, Challenge255, TranscriptReadBuffer, TranscriptWriterBuffer,
        },
    };
    use rand_chacha::{ChaCha20Rng, rand_core::SeedableRng};

    let params = ParamsIPA::<C>::new(6);
    let original = fixture::<C::ScalarExt, true>();
    let candidate = fixture::<C::ScalarExt, false>();
    let original_vk = keygen_vk(&params, &original).unwrap();
    let candidate_vk = keygen_vk(&params, &candidate).unwrap();
    assert_eq!(
        original_vk.to_bytes(SerdeFormat::Processed),
        candidate_vk.to_bytes(SerdeFormat::Processed)
    );
    let original_pk = keygen_pk(&params, original_vk, &original).unwrap();
    let candidate_pk = keygen_pk(&params, candidate_vk, &candidate).unwrap();
    assert_eq!(
        original_pk.to_bytes(SerdeFormat::Processed),
        candidate_pk.to_bytes(SerdeFormat::Processed)
    );
    let public = [C::ScalarExt::from(8)];
    let columns: [&[C::ScalarExt]; 1] = [&public];
    let seed = [109; 32];
    let mut reference = Blake2bWrite::<_, C, Challenge255<C>>::init(Vec::new());
    create_proof::<IPACommitmentScheme<C>, ProverIPA<'_, C>, _, _, _, _>(
        &params,
        &original_pk,
        &[original],
        &[&columns],
        ChaCha20Rng::from_seed(seed),
        &mut reference,
    )
    .unwrap();
    let reference = reference.finalize();
    let mut actual = Blake2bWrite::<_, C, Challenge255<C>>::init(Vec::new());
    create_proof::<IPACommitmentScheme<C>, ProverIPA<'_, C>, _, _, _, _>(
        &params,
        &candidate_pk,
        &[candidate],
        &[&columns],
        ChaCha20Rng::from_seed(seed),
        &mut actual,
    )
    .unwrap();
    let actual = actual.finalize();
    assert_eq!(
        actual, reference,
        "cell-only assignment changed the seeded complete proof"
    );
    let verify = |public: &[C::ScalarExt], proof: &[u8]| {
        let columns = [public];
        let mut transcript = Blake2bRead::<_, C, Challenge255<C>>::init(proof);
        verify_proof::<IPACommitmentScheme<C>, VerifierIPA<'_, C>, _, _, _>(
            &params,
            candidate_pk.get_vk(),
            SingleStrategy::<C>::new(&params),
            &[&columns],
            &mut transcript,
        )
    };
    verify(&public, &actual).unwrap();
    assert!(verify(&[C::ScalarExt::from(9)], &actual).is_err());
    let mut changed_proof = actual.clone();
    changed_proof[..32].fill(0xff);
    assert!(verify(&public, &changed_proof).is_err());
}

#[test]
fn cell_only_base_preserves_eq_ipa_keys_proof_and_rejection() {
    compare_real_ipa::<EqAffine>();
}

#[test]
fn cell_only_base_preserves_ep_ipa_keys_proof_and_rejection() {
    compare_real_ipa::<EpAffine>();
}
