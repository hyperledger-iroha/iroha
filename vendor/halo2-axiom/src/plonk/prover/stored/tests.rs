//! Ordinary-prover prefix oracles and ownership/failure tests for the private stored runner.
//!
//! This recording backend intentionally retains plaintext test copies. It is not the encrypted
//! Core backend and proves neither its security nor a complete stored proof or resource bound.

use super::*;
use crate::poly::stored_advice::{StoredLookupSideV1, StoredPolynomialRoleV1};
use crate::{
    circuit::{Layouter, Value, floor_planner::V1},
    plonk::{
        Advice, Assigned, Challenge, Column, Error, FirstPhase, Fixed, Instance, SecondPhase,
        Selector, create_proof_consuming, keygen_pk, keygen_vk_custom,
    },
    poly::{
        Rotation,
        commitment::ParamsProver,
        ipa::{commitment::IPACommitmentScheme, multiopen::ProverIPA},
        stored_advice::{
            StoredPastaFieldV1, StoredPolynomialLayoutV1, StoredPolynomialSnapshotV1,
            assignment::StoredAssignmentErrorV1,
        },
    },
    transcript::{Blake2bWrite, Challenge255, Transcript, TranscriptWriterBuffer},
};
use ff::{FromUniformBytes, PrimeField};
use halo2curves::pasta::{EpAffine, EqAffine};
use rand_chacha::ChaCha20Rng;
use rand_core::{Error as RngError, SeedableRng};
use std::{
    io,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{Arc, Mutex},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Event<C: CurveAffine> {
    CommonScalar(C::Scalar),
    CommonPoint(C),
    WriteScalar(C::Scalar),
    WritePoint(C),
    Challenge(C::Scalar),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Fault {
    CreateLookupRole(StoredLookupSideV1),
    Create,
    CreateColumn(u32),
    Write(u64),
    Seal,
    Read(u64),
    PanicRead(u64),
    Transcript(usize),
}
struct Log<C: CurveAffine> {
    live_producers: usize,
    producer_drops: usize,
    measurement_passes: usize,
    synthesis_passes: usize,
    provider_drops: usize,
    rng_drops: usize,
    transcript_drops: usize,
    created: usize,
    writer_drops: usize,
    snapshot_drops: usize,
    writes: usize,
    reads: usize,
    rng_calls: usize,
    events: Vec<Event<C>>,
    sealed: Vec<(StoredPolynomialLayoutV1, Vec<[u8; 32]>)>,
    after_phase: Option<(usize, [u8; 64])>,
    fault: Option<Fault>,
    // Models a shared backend substituting public receipt metadata after a successful read.
    snapshot_layout_override: Option<(u64, StoredPolynomialLayoutV1)>,
}
impl<C: CurveAffine> Default for Log<C> {
    fn default() -> Self {
        Self {
            live_producers: 0,
            producer_drops: 0,
            measurement_passes: 0,
            synthesis_passes: 0,
            provider_drops: 0,
            rng_drops: 0,
            transcript_drops: 0,
            created: 0,
            writer_drops: 0,
            snapshot_drops: 0,
            writes: 0,
            reads: 0,
            rng_calls: 0,
            events: Vec::new(),
            sealed: Vec::new(),
            after_phase: None,
            fault: None,
            snapshot_layout_override: None,
        }
    }
}
struct Shared<C: CurveAffine> {
    log: Mutex<Log<C>>,
    rng: Mutex<ChaCha20Rng>,
}
impl<C: CurveAffine> Shared<C> {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            log: Mutex::new(Log::default()),
            rng: Mutex::new(ChaCha20Rng::from_seed([37; 32])),
        })
    }
    fn fault(&self) -> Option<Fault> {
        self.log.lock().unwrap().fault
    }
}
struct Rng<C: CurveAffine>(Arc<Shared<C>>);
impl<C: CurveAffine> Rng<C> {
    fn check(&self) {
        let mut log = self.0.log.lock().unwrap();
        assert_eq!(
            log.live_producers, 0,
            "proof randomness before final producer drop"
        );
        log.rng_calls += 1;
    }
}
impl<C: CurveAffine> RngCore for Rng<C> {
    fn next_u32(&mut self) -> u32 {
        self.check();
        self.0.rng.lock().unwrap().next_u32()
    }
    fn next_u64(&mut self) -> u64 {
        self.check();
        self.0.rng.lock().unwrap().next_u64()
    }
    fn fill_bytes(&mut self, output: &mut [u8]) {
        self.check();
        self.0.rng.lock().unwrap().fill_bytes(output);
    }
    fn try_fill_bytes(&mut self, output: &mut [u8]) -> Result<(), RngError> {
        self.fill_bytes(output);
        Ok(())
    }
}
impl<C: CurveAffine> Drop for Rng<C> {
    fn drop(&mut self) {
        self.0.log.lock().unwrap().rng_drops += 1;
    }
}

struct RecordingTranscript<C: CurveAffine>
where
    C::Scalar: FromUniformBytes<64>,
{
    inner: Blake2bWrite<Vec<u8>, C, Challenge255<C>>,
    shared: Arc<Shared<C>>,
}
impl<C: CurveAffine> RecordingTranscript<C>
where
    C::Scalar: FromUniformBytes<64>,
{
    fn new(shared: &Arc<Shared<C>>) -> Self {
        Self {
            inner: Blake2bWrite::init(Vec::new()),
            shared: Arc::clone(shared),
        }
    }
    fn record(&mut self, event: Event<C>) -> io::Result<()> {
        let (index, fault) = {
            let log = self.shared.log.lock().unwrap();
            (log.events.len(), log.fault)
        };
        if fault == Some(Fault::Transcript(index)) {
            return Err(io::Error::other("injected prefix write failure"));
        }
        let mut log = self.shared.log.lock().unwrap();
        if index > 0 {
            assert_eq!(
                log.live_producers, 0,
                "instance prefix before producer drop"
            );
        }
        log.events.push(event);
        Ok(())
    }
}
impl<C: CurveAffine> Transcript<C, Challenge255<C>> for RecordingTranscript<C>
where
    C::Scalar: FromUniformBytes<64>,
{
    fn squeeze_challenge(&mut self) -> Challenge255<C> {
        let challenge = self.inner.squeeze_challenge();
        let mut log = self.shared.log.lock().unwrap();
        assert_eq!(log.live_producers, 0);
        log.events.push(Event::Challenge(challenge.get_scalar()));
        if log.after_phase.is_none() {
            let mut cloned = self.shared.rng.lock().unwrap().clone();
            let mut next = [0; 64];
            cloned.fill_bytes(&mut next);
            let calls = log.rng_calls;
            log.after_phase = Some((calls, next));
        }
        challenge
    }
    fn common_point(&mut self, point: C) -> io::Result<()> {
        self.record(Event::CommonPoint(point))?;
        self.inner.common_point(point)
    }
    fn common_scalar(&mut self, scalar: C::Scalar) -> io::Result<()> {
        self.record(Event::CommonScalar(scalar))?;
        self.inner.common_scalar(scalar)
    }
}
impl<C: CurveAffine> TranscriptWrite<C, Challenge255<C>> for RecordingTranscript<C>
where
    C::Scalar: FromUniformBytes<64>,
{
    fn write_point(&mut self, point: C) -> io::Result<()> {
        self.record(Event::WritePoint(point))?;
        self.inner.write_point(point)
    }
    fn write_scalar(&mut self, scalar: C::Scalar) -> io::Result<()> {
        self.record(Event::WriteScalar(scalar))?;
        self.inner.write_scalar(scalar)
    }
}
impl<C: CurveAffine> Drop for RecordingTranscript<C>
where
    C::Scalar: FromUniformBytes<64>,
{
    fn drop(&mut self) {
        self.shared.log.lock().unwrap().transcript_drops += 1;
    }
}

struct Provider<C: CurveAffine> {
    shared: Arc<Shared<C>>,
    ordinal: u64,
}
struct Writer<C: CurveAffine> {
    shared: Arc<Shared<C>>,
    layout: StoredPolynomialLayoutV1,
    values: Vec<[u8; 32]>,
    next: u64,
}
struct Snapshot<C: CurveAffine> {
    shared: Arc<Shared<C>>,
    layout: StoredPolynomialLayoutV1,
    values: Vec<[u8; 32]>,
}
impl<C: CurveAffine> Provider<C> {
    fn new(shared: &Arc<Shared<C>>) -> Self {
        Self {
            shared: Arc::clone(shared),
            ordinal: 0,
        }
    }
}
impl<C: CurveAffine> Drop for Provider<C> {
    fn drop(&mut self) {
        self.shared.log.lock().unwrap().provider_drops += 1;
    }
}
impl<C: CurveAffine> Drop for Writer<C> {
    fn drop(&mut self) {
        self.shared.log.lock().unwrap().writer_drops += 1;
    }
}
impl<C: CurveAffine> Drop for Snapshot<C> {
    fn drop(&mut self) {
        self.shared.log.lock().unwrap().snapshot_drops += 1;
    }
}
impl<C: CurveAffine> StoredPolynomialProviderV1 for Provider<C> {
    type Writer = Writer<C>;
    fn create(
        &mut self,
        field: StoredPastaFieldV1,
        basis: StoredPolynomialBasisV1,
        k: u32,
        role: StoredPolynomialRoleV1,
    ) -> Result<Self::Writer, StoredPolynomialErrorV1> {
        if self.shared.fault() == Some(Fault::Create)
            || matches!(role, StoredPolynomialRoleV1::Advice { column, .. }
                if self.shared.fault() == Some(Fault::CreateColumn(column)))
        {
            return Err(StoredPolynomialErrorV1::Storage);
        }
        let role = match self.shared.fault() {
            Some(Fault::CreateLookupRole(side)) => StoredPolynomialRoleV1::LookupCompressed {
                lookup: match role {
                    StoredPolynomialRoleV1::Advice { column, .. } => column,
                    StoredPolynomialRoleV1::LookupCompressed { lookup, .. }
                    | StoredPolynomialRoleV1::LookupSorted { lookup, .. }
                    | StoredPolynomialRoleV1::LookupLeftoverTable { lookup }
                    | StoredPolynomialRoleV1::LookupPermuted { lookup, .. }
                    | StoredPolynomialRoleV1::LookupProduct { lookup } => lookup,
                    StoredPolynomialRoleV1::CopyPermutationProduct { set } => set,
                    StoredPolynomialRoleV1::Instance { column } => column,
                    StoredPolynomialRoleV1::QuotientAliasedPart { part, .. } => part,
                    StoredPolynomialRoleV1::QuotientPiece { piece } => piece,
                    StoredPolynomialRoleV1::VanishingRandom
                    | StoredPolynomialRoleV1::QuotientNumerator => 0,
                },
                side,
            },
            _ => role,
        };
        let layout = StoredPolynomialLayoutV1::new([23; 32], self.ordinal, field, basis, k, role)?;
        self.ordinal += 1;
        self.shared.log.lock().unwrap().created += 1;
        Ok(Writer {
            shared: Arc::clone(&self.shared),
            layout,
            values: Vec::new(),
            next: 0,
        })
    }
}
impl<C: CurveAffine> StoredPolynomialWriterV1 for Writer<C> {
    type Snapshot = Snapshot<C>;
    fn layout(&self) -> StoredPolynomialLayoutV1 {
        self.layout
    }
    fn write_chunk(
        &mut self,
        chunk: u64,
        values: &[[u8; 32]],
    ) -> Result<(), StoredPolynomialErrorV1> {
        if self.shared.fault() == Some(Fault::Write(chunk)) {
            return Err(StoredPolynomialErrorV1::Storage);
        }
        assert_eq!(self.next, chunk);
        assert_eq!(values.len(), self.layout.chunk_scalar_count(chunk)?);
        self.values.extend_from_slice(values);
        self.next += 1;
        self.shared.log.lock().unwrap().writes += 1;
        Ok(())
    }
    fn seal(mut self) -> Result<Self::Snapshot, StoredPolynomialErrorV1> {
        if self.shared.fault() == Some(Fault::Seal) {
            return Err(StoredPolynomialErrorV1::Storage);
        }
        assert_eq!(self.next as usize, self.layout.chunk_count());
        self.shared
            .log
            .lock()
            .unwrap()
            .sealed
            .push((self.layout, self.values.clone()));
        Ok(Snapshot {
            shared: Arc::clone(&self.shared),
            layout: self.layout,
            values: std::mem::take(&mut self.values),
        })
    }
}
impl<C: CurveAffine> StoredPolynomialSnapshotV1 for Snapshot<C> {
    fn layout(&self) -> StoredPolynomialLayoutV1 {
        match self.shared.log.lock().unwrap().snapshot_layout_override {
            Some((ordinal, replacement)) if ordinal == self.layout.ordinal() => replacement,
            _ => self.layout,
        }
    }
    fn with_chunk<V>(
        &mut self,
        expected: StoredPolynomialLayoutV1,
        chunk: u64,
        consume: impl FnOnce(&[[u8; 32]]) -> Result<V, StoredPolynomialErrorV1>,
    ) -> Result<V, StoredPolynomialErrorV1> {
        assert_eq!(self.layout, expected);
        let fault = self.shared.fault();
        assert_ne!(
            fault,
            Some(Fault::PanicRead(chunk)),
            "injected stored read unwind"
        );
        if fault == Some(Fault::Read(chunk)) {
            return Err(StoredPolynomialErrorV1::Storage);
        }
        let mut log = self.shared.log.lock().unwrap();
        assert_eq!(
            log.live_producers, 0,
            "advice commitment before producer drop"
        );
        log.reads += 1;
        drop(log);
        let start = chunk as usize * 256;
        consume(&self.values[start..start + expected.chunk_scalar_count(chunk)?])
    }
    fn with_column<V>(
        &mut self,
        _: StoredPolynomialLayoutV1,
        _: impl FnOnce(&[[u8; 32]]) -> Result<V, StoredPolynomialErrorV1>,
    ) -> Result<V, StoredPolynomialErrorV1> {
        panic!("prefix must never use backend full-column materialization")
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct CircuitParams {
    extra_advice: bool,
    second_phase: bool,
}
#[derive(Clone)]
struct Config {
    advice: [Column<Advice>; 2],
    instances: [Column<Instance>; 3],
    fixed: Column<Fixed>,
    selector: Selector,
    challenge: Challenge,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Mode {
    #[default]
    Good,
    Reference,
    Unknown,
    Backwards,
    NextPhase,
    IgnoredQuery,
    Error,
    Panic,
}
struct Producer<C: CurveAffine> {
    shared: Arc<Shared<C>>,
    params: CircuitParams,
    mode: Mode,
    last_row: usize,
}
impl<C: CurveAffine> Producer<C> {
    fn new(shared: &Arc<Shared<C>>, last_row: usize) -> Self {
        shared.log.lock().unwrap().live_producers += 1;
        Self {
            shared: Arc::clone(shared),
            params: CircuitParams::default(),
            mode: Mode::Good,
            last_row,
        }
    }
    fn run(
        &self,
        config: Config,
        mut layouter: impl Layouter<C::Scalar>,
        measurement: bool,
    ) -> Result<(), Error> {
        if measurement {
            self.shared.log.lock().unwrap().measurement_passes += 1;
        } else {
            self.shared.log.lock().unwrap().synthesis_passes += 1;
        }
        layouter.assign_region(
            || "prefix absolute advice",
            |mut region| {
                config.selector.enable(&mut region, 0)?;
                region.assign_fixed(config.fixed, 0, C::Scalar::from(7));
                let _ = region.instance_value(config.instances[0], 0)?;
                for column in 0..2 {
                    for row in [0, 1, self.last_row] {
                        let assigned = input::<C::Scalar>(column, row);
                        let value = if !measurement && self.mode == Mode::Unknown {
                            Value::unknown()
                        } else {
                            Value::known(assigned)
                        };
                        if !measurement && self.mode == Mode::Reference {
                            let _ = region.assign_advice(config.advice[column], row, value);
                        } else {
                            region.assign_advice_discarding_value(
                                config.advice[column],
                                row,
                                value,
                            );
                        }
                    }
                }
                if !measurement {
                    match self.mode {
                        Mode::Backwards => {
                            region.assign_advice_discarding_value(
                                config.advice[0],
                                1,
                                Value::known(Assigned::Zero),
                            );
                        }
                        Mode::NextPhase => region.next_phase(),
                        Mode::IgnoredQuery => {
                            let _ = region.instance_value(config.instances[0], 7);
                        }
                        Mode::Error => return Err(Error::Synthesis),
                        Mode::Panic => panic!("injected original producer unwind"),
                        _ => (),
                    }
                }
                Ok(())
            },
        )?;
        let _ = layouter.get_challenge(config.challenge);
        Ok(())
    }
}
fn input<F: Field + From<u64>>(column: usize, row: usize) -> Assigned<F> {
    match (column, row) {
        (0, 0) => Assigned::Trivial(F::from(17)),
        (1, 0) => Assigned::Rational(F::from(34), F::from(2)),
        (_, 1) => Assigned::Rational(F::from(31), F::ZERO),
        (0, _) => Assigned::Zero,
        _ => Assigned::Rational(F::from(33), F::from(3)),
    }
}
impl<C: CurveAffine> Drop for Producer<C> {
    fn drop(&mut self) {
        let mut log = self.shared.log.lock().unwrap();
        log.live_producers -= 1;
        log.producer_drops += 1;
    }
}
impl<C: CurveAffine> Circuit<C::Scalar> for Producer<C> {
    type Config = Config;
    type FloorPlanner = V1;
    #[cfg(feature = "circuit-params")]
    type Params = CircuitParams;
    #[cfg(feature = "circuit-params")]
    fn params(&self) -> Self::Params {
        self.params
    }
    fn without_witnesses(&self) -> Self {
        Self::new(&self.shared, self.last_row)
    }
    #[cfg(feature = "circuit-params")]
    fn configure_with_params(
        meta: &mut ConstraintSystem<C::Scalar>,
        params: Self::Params,
    ) -> Config {
        let config = Self::configure(meta);
        if params.extra_advice {
            meta.advice_column();
        }
        if params.second_phase {
            meta.advice_column_in(SecondPhase);
        }
        config
    }
    fn configure(meta: &mut ConstraintSystem<C::Scalar>) -> Config {
        let advice = [meta.advice_column(), meta.advice_column()];
        let instances = [
            meta.instance_column(),
            meta.instance_column(),
            meta.instance_column(),
        ];
        let fixed = meta.fixed_column();
        let selector = meta.selector();
        let challenge = meta.challenge_usable_after(FirstPhase);
        meta.enable_equality(advice[0]);
        meta.enable_equality(advice[1]);
        meta.create_gate("same first value", |meta| {
            let q = meta.query_selector(selector);
            let left = meta.query_advice(advice[0], Rotation::cur());
            let right = meta.query_advice(advice[1], Rotation::cur());
            vec![q * (left - right)]
        });
        Config {
            advice,
            instances,
            fixed,
            selector,
            challenge,
        }
    }
    fn synthesize_for_measurement(
        &self,
        config: Config,
        layouter: impl Layouter<C::Scalar>,
    ) -> Result<(), Error> {
        self.run(config, layouter, true)
    }
    fn synthesize(&self, config: Config, layouter: impl Layouter<C::Scalar>) -> Result<(), Error> {
        self.run(config, layouter, false)
    }
}

fn key<C>(params: &ParamsIPA<C>, compress: bool) -> ProvingKey<C>
where
    C: CurveAffine,
    C::Scalar: FromUniformBytes<64>,
{
    let shared = Shared::<C>::new();
    let circuit = Producer::new(&shared, 3);
    let vk = keygen_vk_custom(params, &circuit, compress).unwrap();
    keygen_pk(params, vk, &circuit).unwrap()
}
fn columns<F: Field + From<u64>>() -> [Vec<F>; 3] {
    [
        vec![F::from(17)],
        vec![F::from(5), F::from(6)],
        vec![F::from(9)],
    ]
}

fn compare<C, const Q: bool, const M: u64>(compress: bool)
where
    C: CurveAffine + crate::SerdeCurveAffine,
    C::Scalar: StoredAssignmentFieldV1
        + WithSmallOrderMulGroup<3>
        + FromUniformBytes<64>
        + crate::SerdePrimeField,
{
    let params = ParamsIPA::<C>::new(4);
    let pk = key(&params, compress);
    let expected_vk = pk.get_vk().to_bytes(crate::SerdeFormat::Processed);
    let values = columns::<C::Scalar>();
    let instances = values.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let ordinary = Shared::new();
    let ordinary_producer = Producer::new(&ordinary, 3);
    let mut ordinary_transcript = RecordingTranscript::new(&ordinary);
    let ordinary_vk = create_proof_consuming::<
        IPACommitmentScheme<C>,
        ProverIPA<'_, C, Q, M>,
        Challenge255<C>,
        _,
        _,
        _,
    >(
        &params,
        pk.clone(),
        ordinary_producer,
        &[&instances],
        Rng(Arc::clone(&ordinary)),
        &mut ordinary_transcript,
    )
    .unwrap();
    assert_eq!(
        ordinary_vk.to_bytes(crate::SerdeFormat::Processed),
        expected_vk
    );
    let stored = Shared::new();
    let producer = Producer::new(&stored, 3);
    let mut pending =
        prepare_single_phase_stored_ipa_prefix_v1::<C, _, _, _, Challenge255<C>, _, Q, M>(
            &params,
            pk,
            producer,
            &instances,
            Provider::new(&stored),
            Rng(Arc::clone(&stored)),
            RecordingTranscript::new(&stored),
        )
        .unwrap();
    let stored_log = stored.log.lock().unwrap();
    let ordinary_log = ordinary.log.lock().unwrap();
    assert_eq!(
        stored_log.events,
        ordinary_log.events[..stored_log.events.len()]
    );
    assert_eq!(stored_log.after_phase, ordinary_log.after_phase);
    assert_eq!(stored_log.producer_drops, 1);
    assert_eq!(stored_log.measurement_passes, 1);
    assert_eq!(stored_log.synthesis_passes, 1);
    assert_eq!(stored_log.provider_drops, 0);
    assert_eq!(stored_log.rng_drops, 0);
    assert_eq!(stored_log.transcript_drops, 0);
    let phase = stored_log.after_phase.unwrap();
    assert_eq!(stored_log.rng_calls, phase.0);
    // Independent ordinary dense assignment/tail oracle, not another stored-phase invocation.
    let mut expected_rng = ChaCha20Rng::from_seed([37; 32]);
    let usable = 16 - (pending.pk.vk.cs.blinding_factors() + 1);
    for (column, (layout, actual)) in stored_log.sealed.iter().enumerate() {
        assert_eq!(layout.advice_coordinates().unwrap().0 as usize, column);
        let mut expected = vec![C::Scalar::ZERO; 16];
        for row in [0, 1, 3] {
            expected[row] = input::<C::Scalar>(column, row).evaluate();
        }
        for scalar in &mut expected[usable..] {
            *scalar = C::Scalar::random(&mut expected_rng);
        }
        assert_eq!(
            *actual,
            expected.iter().map(PrimeField::to_repr).collect::<Vec<_>>()
        );
    }
    assert_eq!(stored_log.sealed.len(), 2);
    drop(ordinary_log);
    drop(stored_log);
    assert!(std::ptr::eq(pending.params, &params));
    assert!(std::ptr::eq(pending.advice.params().unwrap(), &params));
    assert_eq!(
        pending.pk.get_vk().to_bytes(crate::SerdeFormat::Processed),
        expected_vk
    );
    assert_eq!(pending.instances.len(), 3);
    let challenge = pending.advice.challenges().unwrap().collect::<Vec<_>>();
    assert_eq!(challenge.len(), 1);
    let mut next = [0; 64];
    pending.rng.fill_bytes(&mut next);
    assert_eq!(next, phase.1);
    drop(pending);
    let log = stored.log.lock().unwrap();
    assert_eq!(log.provider_drops, 1);
    assert_eq!(log.rng_drops, 1);
    assert_eq!(log.transcript_drops, 1);
    assert_eq!(log.snapshot_drops, 2);
    assert_eq!(log.writer_drops, 2);
}

#[test]
fn eq_prefix_matches_ordinary_direct_query_and_hybrid_both_selector_modes() {
    for compressed in [true, false] {
        compare::<EqAffine, false, 0>(compressed);
        compare::<EqAffine, true, 0>(compressed);
        compare::<EqAffine, true, 6>(compressed);
    }
}
#[test]
fn ep_prefix_matches_ordinary_direct_query_and_hybrid_both_selector_modes() {
    for compressed in [true, false] {
        compare::<EpAffine, false, 0>(compressed);
        compare::<EpAffine, true, 0>(compressed);
        compare::<EpAffine, true, 6>(compressed);
    }
}

fn assert_dropped<C: CurveAffine>(shared: &Arc<Shared<C>>) {
    let log = shared.log.lock().unwrap();
    assert_eq!(log.live_producers, 0);
    assert_eq!(log.producer_drops, 1);
    assert_eq!(log.provider_drops, 1);
    assert_eq!(log.rng_drops, 1);
    assert_eq!(log.transcript_drops, 1);
    assert_eq!(log.writer_drops, log.created);
    assert_eq!(log.snapshot_drops, log.sealed.len());
}

fn preflights<C>()
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
{
    let params = ParamsIPA::<C>::new(4);
    let pk = key(&params, true);
    let values = columns::<C::Scalar>();
    let normal = values.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let oversized = vec![C::Scalar::ONE; 16];
    for case in 0..6 {
        let shared = Shared::new();
        let mut producer = Producer::new(&shared, 3);
        let mut instances = normal.clone();
        match case {
            0 => {
                instances.pop();
            }
            1 => instances.push(&[]),
            2 => instances[1] = &oversized,
            3 => producer.params.extra_advice = true,
            4 => producer.params.second_phase = true,
            _ => (),
        }
        let bad_params = ParamsIPA::<C>::new(5);
        let chosen = if case == 5 { &bad_params } else { &params };
        let result =
            prepare_single_phase_stored_ipa_prefix_v1::<C, _, _, _, Challenge255<C>, _, true, 6>(
                chosen,
                pk.clone(),
                producer,
                &instances,
                Provider::new(&shared),
                Rng(Arc::clone(&shared)),
                RecordingTranscript::new(&shared),
            );
        assert!(result.is_err());
        drop(result);
        assert_dropped(&shared);
        let log = shared.log.lock().unwrap();
        assert_eq!(log.created, 0);
        assert_eq!(log.writes, 0);
        assert_eq!(log.rng_calls, 0);
        assert!(log.events.is_empty());
    }
    // This key really contains a second-phase advice column, rather than merely a
    // producer configuration that disagrees with a first-phase key.
    #[cfg(feature = "circuit-params")]
    {
        let key_shared = Shared::<C>::new();
        let mut key_producer = Producer::new(&key_shared, 3);
        key_producer.params.second_phase = true;
        let vk = keygen_vk_custom(&params, &key_producer, true).unwrap();
        let multiphase_pk = keygen_pk(&params, vk, &key_producer).unwrap();
        assert!(
            multiphase_pk
                .vk
                .cs
                .advice_column_phase
                .iter()
                .any(|phase| phase.to_u8() != 0)
        );
        let shared = Shared::new();
        let mut producer = Producer::new(&shared, 3);
        producer.params.second_phase = true;
        let result =
            prepare_single_phase_stored_ipa_prefix_v1::<C, _, _, _, Challenge255<C>, _, true, 6>(
                &params,
                multiphase_pk,
                producer,
                &normal,
                Provider::new(&shared),
                Rng(Arc::clone(&shared)),
                RecordingTranscript::new(&shared),
            )
            .map(|_| ());
        assert_eq!(result, Err(StoredPrefixErrorV1::Admission));
        assert_dropped(&shared);
        let log = shared.log.lock().unwrap();
        assert!(log.events.is_empty());
        assert_eq!(log.created, 0);
        assert_eq!(log.rng_calls, 0);
    }
    for invalid_mask in [false, true] {
        let shared = Shared::new();
        let producer = Producer::new(&shared, 3);
        let result = if invalid_mask {
            prepare_single_phase_stored_ipa_prefix_v1::<C, _, _, _, Challenge255<C>, _, true, 8>(
                &params,
                pk.clone(),
                producer,
                &normal,
                Provider::new(&shared),
                Rng(Arc::clone(&shared)),
                RecordingTranscript::new(&shared),
            )
            .map(|_| ())
        } else {
            prepare_single_phase_stored_ipa_prefix_v1::<C, _, _, _, Challenge255<C>, _, false, 6>(
                &params,
                pk.clone(),
                producer,
                &normal,
                Provider::new(&shared),
                Rng(Arc::clone(&shared)),
                RecordingTranscript::new(&shared),
            )
            .map(|_| ())
        };
        assert_eq!(result, Err(StoredPrefixErrorV1::Instances));
        assert_dropped(&shared);
        let log = shared.log.lock().unwrap();
        assert!(log.events.is_empty());
        assert_eq!(log.created, 0);
        assert_eq!(log.rng_calls, 0);
    }
}
#[test]
#[cfg(feature = "circuit-params")]
fn both_pasta_prefix_preflights_reject_instances_parameters_and_structural_key_mismatch_without_side_effects()
 {
    preflights::<EqAffine>();
    preflights::<EpAffine>();
}

fn producer_failures<C>()
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
{
    let params = ParamsIPA::<C>::new(4);
    let pk = key(&params, true);
    let values = columns::<C::Scalar>();
    let instances = values.iter().map(Vec::as_slice).collect::<Vec<_>>();
    for mode in [
        Mode::Reference,
        Mode::Unknown,
        Mode::Backwards,
        Mode::NextPhase,
        Mode::IgnoredQuery,
        Mode::Error,
        Mode::Panic,
    ] {
        let shared = Shared::new();
        let mut producer = Producer::new(&shared, 3);
        producer.mode = mode;
        let result = catch_unwind(AssertUnwindSafe(|| {
            prepare_single_phase_stored_ipa_prefix_v1::<C, _, _, _, Challenge255<C>, _, true, 6>(
                &params,
                pk.clone(),
                producer,
                &instances,
                Provider::new(&shared),
                Rng(Arc::clone(&shared)),
                RecordingTranscript::new(&shared),
            )
            .map(|_| ())
        }));
        if mode == Mode::Panic {
            assert!(result.is_err());
        } else {
            let error = match mode {
                Mode::Reference => {
                    StoredSynthesisErrorV1::Phase(StoredPhaseErrorV1::ReferenceReturn)
                }
                Mode::Unknown => StoredSynthesisErrorV1::Phase(StoredPhaseErrorV1::UnknownValue),
                Mode::Backwards => StoredSynthesisErrorV1::Phase(StoredPhaseErrorV1::Assignment(
                    StoredAssignmentErrorV1::NonMonotonic,
                )),
                Mode::NextPhase => StoredSynthesisErrorV1::PhaseAdvance,
                Mode::IgnoredQuery => StoredSynthesisErrorV1::Instances,
                Mode::Error => StoredSynthesisErrorV1::Synthesis,
                _ => unreachable!(),
            };
            assert_eq!(result.unwrap(), Err(StoredPrefixErrorV1::Synthesis(error)));
        }
        assert_dropped(&shared);
        let log = shared.log.lock().unwrap();
        assert_eq!(log.events.len(), 1);
        assert_eq!(log.rng_calls, 0);
        assert_eq!(log.reads, 0);
    }
}
#[test]
fn both_pasta_prefix_rejects_ignored_assignment_errors_phase_advance_and_producer_unwind() {
    producer_failures::<EqAffine>();
    producer_failures::<EpAffine>();
}

fn backend_failures<C>()
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
{
    let params = ParamsIPA::<C>::new(9);
    let pk = key(&params, true);
    let values = columns::<C::Scalar>();
    let instances = values.iter().map(Vec::as_slice).collect::<Vec<_>>();
    for fault in [
        Fault::Create,
        Fault::Write(0),
        Fault::Write(1),
        Fault::Seal,
        Fault::Read(1),
        Fault::PanicRead(1),
        Fault::Transcript(0),
        Fault::Transcript(1),
        Fault::Transcript(4),
    ] {
        let shared = Shared::new();
        shared.log.lock().unwrap().fault = Some(fault);
        let producer = Producer::new(&shared, 300);
        let result = catch_unwind(AssertUnwindSafe(|| {
            prepare_single_phase_stored_ipa_prefix_v1::<C, _, _, _, Challenge255<C>, _, true, 6>(
                &params,
                pk.clone(),
                producer,
                &instances,
                Provider::new(&shared),
                Rng(Arc::clone(&shared)),
                RecordingTranscript::new(&shared),
            )
            .map(|_| ())
        }));
        if matches!(fault, Fault::PanicRead(_)) {
            assert!(result.is_err());
        } else {
            assert!(result.unwrap().is_err());
        }
        assert_dropped(&shared);
        let log = shared.log.lock().unwrap();
        assert!(
            log.after_phase.is_none(),
            "no phase challenge after a failed prefix"
        );
        if matches!(fault, Fault::Read(1) | Fault::PanicRead(1)) {
            assert_eq!(
                log.reads, 1,
                "second-chunk failure must follow one successful read"
            );
        }
    }
}
fn direct_instance_transcript_failure<C>()
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
{
    let params = ParamsIPA::<C>::new(4);
    let pk = key(&params, true);
    let values = columns::<C::Scalar>();
    let instances = values.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let shared = Shared::new();
    shared.log.lock().unwrap().fault = Some(Fault::Transcript(1));
    let result =
        prepare_single_phase_stored_ipa_prefix_v1::<C, _, _, _, Challenge255<C>, _, false, 0>(
            &params,
            pk,
            Producer::new(&shared, 3),
            &instances,
            Provider::new(&shared),
            Rng(Arc::clone(&shared)),
            RecordingTranscript::new(&shared),
        )
        .map(|_| ());
    assert_eq!(result, Err(StoredPrefixErrorV1::Transcript));
    assert_dropped(&shared);
    let log = shared.log.lock().unwrap();
    assert_eq!(log.events.len(), 1);
    assert_eq!(log.rng_calls, 0);
    assert_eq!(log.reads, 0);
}
#[test]
fn both_pasta_prefix_store_and_transcript_failures_destroy_every_owned_continuation() {
    backend_failures::<EqAffine>();
    backend_failures::<EpAffine>();
    direct_instance_transcript_failure::<EqAffine>();
    direct_instance_transcript_failure::<EpAffine>();
}

#[path = "coefficient_tests.rs"]
mod coefficients;

#[path = "auxiliary_tests.rs"]
mod auxiliary;

fn reject_lookup_prefix_writers<C>()
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
{
    let params = ParamsIPA::<C>::new(4);
    let pk = key(&params, true);
    let values = columns::<C::Scalar>();
    let instances = values.iter().map(Vec::as_slice).collect::<Vec<_>>();
    for side in [StoredLookupSideV1::Input, StoredLookupSideV1::Table] {
        let shared = Shared::<C>::new();
        shared.log.lock().unwrap().fault = Some(Fault::CreateLookupRole(side));
        let result =
            prepare_single_phase_stored_ipa_prefix_v1::<C, _, _, _, Challenge255<C>, _, true, 6>(
                &params,
                pk.clone(),
                Producer::new(&shared, 3),
                &instances,
                Provider::new(&shared),
                Rng(Arc::clone(&shared)),
                RecordingTranscript::new(&shared),
            );
        assert!(result.is_err());
        drop(result);
        assert_dropped(&shared);
        let log = shared.log.lock().unwrap();
        assert_eq!(log.created, pk.vk.cs.num_advice_columns);
        assert_eq!(log.measurement_passes, 0);
        assert_eq!(log.synthesis_passes, 0);
        assert_eq!(log.writes, 0);
        assert_eq!(log.reads, 0);
        assert_eq!(log.rng_calls, 0);
        assert!(log.sealed.is_empty());
        assert_eq!(
            log.events.len(),
            1,
            "only the original VK binding preceded admission"
        );
    }
}

#[test]
fn both_pasta_prefix_rejects_lookup_writers_before_synthesis_and_drops_protocol_owners() {
    reject_lookup_prefix_writers::<EqAffine>();
    reject_lookup_prefix_writers::<EpAffine>();
}

#[path = "quotient_inverse_tests.rs"]
mod quotient_inverse;
