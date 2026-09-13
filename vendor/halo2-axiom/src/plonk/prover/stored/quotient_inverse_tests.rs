//! Independent dense ordinary inverse, complete-owner preservation and callback fault tests.
//!
//! Plaintext fixtures model ownership and callback boundaries only. They neither authenticate
//! encrypted Core storage nor qualify complete proof bytes, process memory or device execution.

use super::*;
use crate::plonk::prover::stored::lookup::StoredLookupErrorV1;
use crate::{
    plonk::{Expression, ProvingKey},
    poly::{
        EvaluationDomain,
        stored_advice::{StoredPolynomialErrorV1, StoredPolynomialWriterV1},
    },
};
use ff::WithSmallOrderMulGroup;
use std::{
    collections::BTreeMap,
    sync::atomic::{AtomicBool, Ordering},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum IoKind {
    Create,
    Read,
    Write,
    Seal,
    DropWriter,
    DropSnapshot,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct IoEvent {
    kind: IoKind,
    layout: StoredPolynomialLayoutV1,
    chunk: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Change {
    Field,
    K,
    Context,
    Role,
    Part,
    Extension,
    Ordinal,
    Exhausted,
    Insufficient,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Action {
    Error,
    Panic,
    Short,
    Long,
    Encoding,
    Drift(u64),
    Change(Change),
}
#[derive(Default)]
struct Bank {
    active: bool,
    live: BTreeMap<u64, StoredPolynomialLayoutV1>,
    overrides: BTreeMap<u64, StoredPolynomialLayoutV1>,
    events: Vec<IoEvent>,
    fault: Option<(usize, Action)>,
    ordinal_jump: Option<(usize, u64)>,
    peak: usize,
}
struct Controls {
    bank: Mutex<Bank>,
    window: AtomicBool,
    protocol_armed: AtomicBool,
}
impl Controls {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            bank: Mutex::new(Bank::default()),
            window: AtomicBool::new(false),
            protocol_armed: AtomicBool::new(false),
        })
    }
    fn arm(&self, fault: Option<(usize, Action)>) {
        let mut bank = self.bank.lock().unwrap();
        bank.active = true;
        bank.events.clear();
        bank.fault = fault;
        bank.peak = bank.live.len();
    }
    fn event(&self, kind: IoKind, layout: StoredPolynomialLayoutV1, chunk: u64) -> Option<Action> {
        assert!(
            !self.window.load(Ordering::SeqCst),
            "nested inverse backend operation"
        );
        let mut bank = self.bank.lock().unwrap();
        if !bank.active {
            return None;
        }
        let index = bank.events.len();
        bank.events.push(IoEvent {
            kind,
            layout,
            chunk,
        });
        if bank.fault.is_some_and(|(target, _)| target == index) {
            bank.fault.take().map(|(_, action)| action)
        } else {
            None
        }
    }
    fn layout(&self, expected: StoredPolynomialLayoutV1) -> StoredPolynomialLayoutV1 {
        self.bank
            .lock()
            .unwrap()
            .overrides
            .get(&expected.ordinal())
            .copied()
            .unwrap_or(expected)
    }
    fn after(&self, action: Option<Action>, expected: StoredPolynomialLayoutV1) {
        match action {
            Some(Action::Drift(ordinal)) => {
                let mut bank = self.bank.lock().unwrap();
                let original = *bank.live.get(&ordinal).expect("drift victim remains live");
                bank.overrides
                    .insert(ordinal, changed(original, Change::Context));
            }
            Some(Action::Change(change)) => {
                self.bank
                    .lock()
                    .unwrap()
                    .overrides
                    .insert(expected.ordinal(), changed(expected, change));
            }
            _ => (),
        }
    }
    fn retire(&self, kind: IoKind, expected: StoredPolynomialLayoutV1) {
        assert!(
            self.bank
                .lock()
                .unwrap()
                .live
                .remove(&expected.ordinal())
                .is_some()
        );
        let action = self.event(kind, expected, 0);
        // A one-shot panic after unlocking exercises real unwinding without poisoning the
        // monitor or creating an unrelated second panic during owner destruction.
        if action == Some(Action::Panic) {
            panic!("injected inverse receipt destructor unwind");
        }
        self.after(action, expected);
    }
}
fn changed(layout: StoredPolynomialLayoutV1, change: Change) -> StoredPolynomialLayoutV1 {
    let role = match (layout.role(), change) {
        (_, Change::Role) => StoredPolynomialRoleV1::Instance { column: 777 },
        (
            StoredPolynomialRoleV1::QuotientAliasedPart {
                part,
                extension_log,
            },
            Change::Part,
        ) => StoredPolynomialRoleV1::QuotientAliasedPart {
            part: part ^ 1,
            extension_log,
        },
        (
            StoredPolynomialRoleV1::QuotientAliasedPart {
                part,
                extension_log,
            },
            Change::Extension,
        ) => StoredPolynomialRoleV1::QuotientAliasedPart {
            part,
            extension_log: extension_log + 1,
        },
        (StoredPolynomialRoleV1::QuotientPiece { piece }, Change::Part) => {
            StoredPolynomialRoleV1::QuotientPiece { piece: piece + 1 }
        }
        (StoredPolynomialRoleV1::QuotientNumerator, Change::Extension) => layout.role(),
        (_, Change::Extension) => StoredPolynomialRoleV1::QuotientAliasedPart {
            part: 0,
            extension_log: 2,
        },
        _ => layout.role(),
    };
    let basis = match (layout.basis(), change) {
        (
            StoredPolynomialBasisV1::CosetPart {
                extension_log,
                part,
            },
            Change::Part,
        ) => StoredPolynomialBasisV1::CosetPart {
            extension_log,
            part: part ^ 1,
        },
        (
            StoredPolynomialBasisV1::CosetPart {
                extension_log,
                part,
            },
            Change::Extension,
        ) => StoredPolynomialBasisV1::CosetPart {
            extension_log: extension_log + 1,
            part,
        },
        _ => layout.basis(),
    };
    StoredPolynomialLayoutV1::new(
        if change == Change::Context {
            [42; 32]
        } else {
            [23; 32]
        },
        match change {
            Change::Ordinal => 0,
            Change::Exhausted => u64::MAX,
            Change::Insufficient => u64::MAX - 1,
            _ => layout.ordinal(),
        },
        if change == Change::Field {
            match layout.field() {
                StoredPastaFieldV1::Fp => StoredPastaFieldV1::Fq,
                StoredPastaFieldV1::Fq => StoredPastaFieldV1::Fp,
            }
        } else {
            layout.field()
        },
        basis,
        if change == Change::K {
            layout.k() + 1
        } else {
            layout.k()
        },
        role,
    )
    .unwrap()
}
struct Window(Arc<Controls>);
impl Window {
    fn open(controls: &Arc<Controls>) -> Self {
        assert!(!controls.window.swap(true, Ordering::SeqCst));
        Self(Arc::clone(controls))
    }
}
impl Drop for Window {
    fn drop(&mut self) {
        self.0.window.store(false, Ordering::SeqCst);
    }
}
struct InverseProvider<C: CurveAffine> {
    inner: Provider<C>,
    controls: Arc<Controls>,
}
struct InverseWriter<C: CurveAffine> {
    inner: Option<Writer<C>>,
    expected: StoredPolynomialLayoutV1,
    transferred: bool,
    controls: Arc<Controls>,
}
struct InverseSnapshot<C: CurveAffine> {
    inner: Snapshot<C>,
    expected: StoredPolynomialLayoutV1,
    controls: Arc<Controls>,
}
impl<C: CurveAffine> StoredPolynomialProviderV1 for InverseProvider<C> {
    type Writer = InverseWriter<C>;
    fn create(
        &mut self,
        field: StoredPastaFieldV1,
        basis: StoredPolynomialBasisV1,
        k: u32,
        role: StoredPolynomialRoleV1,
    ) -> Result<Self::Writer, StoredPolynomialErrorV1> {
        let jump = {
            let mut bank = self.controls.bank.lock().unwrap();
            if bank.active
                && bank
                    .ordinal_jump
                    .is_some_and(|(target, _)| target == bank.events.len())
            {
                bank.ordinal_jump.take().map(|(_, ordinal)| ordinal)
            } else {
                None
            }
        };
        if let Some(ordinal) = jump {
            self.inner.ordinal = ordinal;
        }
        let expected =
            StoredPolynomialLayoutV1::new([23; 32], self.inner.ordinal, field, basis, k, role)?;
        let action = self.controls.event(IoKind::Create, expected, 0);
        match action {
            Some(Action::Error) => return Err(StoredPolynomialErrorV1::Storage),
            Some(Action::Panic) => panic!("injected inverse writer creation unwind"),
            _ => (),
        }
        if self.controls.bank.lock().unwrap().live.len() >= 512 {
            return Err(StoredPolynomialErrorV1::Capacity);
        }
        let inner = self.inner.create(field, basis, k, role)?;
        assert_eq!(inner.layout(), expected);
        {
            let mut bank = self.controls.bank.lock().unwrap();
            assert!(bank.live.insert(expected.ordinal(), expected).is_none());
            bank.peak = bank.peak.max(bank.live.len());
        }
        self.controls.after(action, expected);
        Ok(InverseWriter {
            inner: Some(inner),
            expected,
            transferred: false,
            controls: Arc::clone(&self.controls),
        })
    }
}
impl<C: CurveAffine> StoredPolynomialWriterV1 for InverseWriter<C> {
    type Snapshot = InverseSnapshot<C>;
    fn layout(&self) -> StoredPolynomialLayoutV1 {
        self.controls.layout(self.inner.as_ref().unwrap().layout())
    }
    fn write_chunk(
        &mut self,
        chunk: u64,
        values: &[[u8; 32]],
    ) -> Result<(), StoredPolynomialErrorV1> {
        let action = self.controls.event(IoKind::Write, self.expected, chunk);
        match action {
            Some(Action::Error) => return Err(StoredPolynomialErrorV1::Storage),
            Some(Action::Panic) => panic!("injected inverse chunk write unwind"),
            _ => (),
        }
        self.inner.as_mut().unwrap().write_chunk(chunk, values)?;
        self.controls.after(action, self.expected);
        Ok(())
    }
    fn seal(mut self) -> Result<Self::Snapshot, StoredPolynomialErrorV1> {
        let action = self.controls.event(IoKind::Seal, self.expected, 0);
        match action {
            Some(Action::Error) => return Err(StoredPolynomialErrorV1::Storage),
            Some(Action::Panic) => panic!("injected inverse seal unwind"),
            _ => (),
        }
        let inner = self.inner.take().unwrap().seal()?;
        self.controls.after(action, self.expected);
        self.transferred = true;
        Ok(InverseSnapshot {
            inner,
            expected: self.expected,
            controls: Arc::clone(&self.controls),
        })
    }
}
impl<C: CurveAffine> Drop for InverseWriter<C> {
    fn drop(&mut self) {
        if !self.transferred {
            self.controls.retire(IoKind::DropWriter, self.expected);
        }
    }
}
impl<C: CurveAffine> StoredPolynomialSnapshotV1 for InverseSnapshot<C> {
    fn layout(&self) -> StoredPolynomialLayoutV1 {
        self.controls.layout(self.inner.layout())
    }
    fn with_chunk<V>(
        &mut self,
        expected: StoredPolynomialLayoutV1,
        chunk: u64,
        consume: impl FnOnce(&[[u8; 32]]) -> Result<V, StoredPolynomialErrorV1>,
    ) -> Result<V, StoredPolynomialErrorV1> {
        let action = self.controls.event(IoKind::Read, self.expected, chunk);
        match action {
            Some(Action::Error) => return Err(StoredPolynomialErrorV1::Storage),
            Some(Action::Panic) => panic!("injected inverse snapshot read unwind"),
            _ => (),
        }
        let result = {
            let _window = Window::open(&self.controls);
            self.inner
                .with_chunk(expected, chunk, |values| match action {
                    Some(Action::Short) => consume(&values[..values.len() - 1]),
                    Some(Action::Long) | Some(Action::Encoding) => {
                        let mut changed = values.to_vec();
                        if action == Some(Action::Long) {
                            changed.push([0; 32]);
                        } else {
                            *changed.last_mut().unwrap() = [255; 32];
                        }
                        consume(&changed)
                    }
                    _ => consume(values),
                })
        };
        if result.is_ok() {
            self.controls.after(action, self.expected);
        }
        result
    }
    fn with_column<V>(
        &mut self,
        _: StoredPolynomialLayoutV1,
        _: impl FnOnce(&[[u8; 32]]) -> Result<V, StoredPolynomialErrorV1>,
    ) -> Result<V, StoredPolynomialErrorV1> {
        panic!("inverse requested backend full-column materialization")
    }
}
impl<C: CurveAffine> Drop for InverseSnapshot<C> {
    fn drop(&mut self) {
        self.controls.retire(IoKind::DropSnapshot, self.expected);
    }
}
struct QuietRng<C: CurveAffine> {
    inner: Rng<C>,
    controls: Arc<Controls>,
}
impl<C: CurveAffine> RngCore for QuietRng<C> {
    fn next_u32(&mut self) -> u32 {
        assert!(!self.controls.protocol_armed.load(Ordering::SeqCst));
        self.inner.next_u32()
    }
    fn next_u64(&mut self) -> u64 {
        assert!(!self.controls.protocol_armed.load(Ordering::SeqCst));
        self.inner.next_u64()
    }
    fn fill_bytes(&mut self, bytes: &mut [u8]) {
        assert!(!self.controls.protocol_armed.load(Ordering::SeqCst));
        self.inner.fill_bytes(bytes)
    }
    fn try_fill_bytes(&mut self, bytes: &mut [u8]) -> Result<(), RngError> {
        assert!(!self.controls.protocol_armed.load(Ordering::SeqCst));
        self.inner.try_fill_bytes(bytes)
    }
}
struct QuietTranscript<C: CurveAffine>
where
    C::Scalar: FromUniformBytes<64>,
{
    inner: RecordingTranscript<C>,
    controls: Arc<Controls>,
}
impl<C: CurveAffine> Transcript<C, Challenge255<C>> for QuietTranscript<C>
where
    C::Scalar: FromUniformBytes<64>,
{
    fn squeeze_challenge(&mut self) -> Challenge255<C> {
        assert!(!self.controls.protocol_armed.load(Ordering::SeqCst));
        self.inner.squeeze_challenge()
    }
    fn common_point(&mut self, point: C) -> io::Result<()> {
        assert!(!self.controls.protocol_armed.load(Ordering::SeqCst));
        self.inner.common_point(point)
    }
    fn common_scalar(&mut self, scalar: C::Scalar) -> io::Result<()> {
        assert!(!self.controls.protocol_armed.load(Ordering::SeqCst));
        self.inner.common_scalar(scalar)
    }
}
impl<C: CurveAffine> TranscriptWrite<C, Challenge255<C>> for QuietTranscript<C>
where
    C::Scalar: FromUniformBytes<64>,
{
    fn write_point(&mut self, point: C) -> io::Result<()> {
        assert!(!self.controls.protocol_armed.load(Ordering::SeqCst));
        self.inner.write_point(point)
    }
    fn write_scalar(&mut self, scalar: C::Scalar) -> io::Result<()> {
        assert!(!self.controls.protocol_armed.load(Ordering::SeqCst));
        self.inner.write_scalar(scalar)
    }
}

#[derive(Clone)]
struct InverseConfig {
    advice: Vec<Column<Advice>>,
    fixed: Vec<Column<Fixed>>,
    instances: Vec<Column<Instance>>,
}
struct InverseCircuit<C: CurveAffine, const DEGREE: usize, const MIXED: bool, const I: usize>(
    Producer<C>,
);
impl<C: CurveAffine, const DEGREE: usize, const MIXED: bool, const I: usize>
    InverseCircuit<C, DEGREE, MIXED, I>
{
    fn run(
        &self,
        config: InverseConfig,
        mut layouter: impl Layouter<C::Scalar>,
        measurement: bool,
    ) -> Result<(), Error> {
        if measurement {
            self.0.shared.log.lock().unwrap().measurement_passes += 1;
        } else {
            self.0.shared.log.lock().unwrap().synthesis_passes += 1;
        }
        if !MIXED {
            return Ok(());
        }
        let first = layouter.assign_region(
            || "inverse retained copy and lookup inputs",
            |mut region| {
                let mut cells = Vec::new();
                for (column, advice) in config.advice.iter().enumerate() {
                    for row in 0..=self.0.last_row {
                        let value = if row == 0 {
                            C::Scalar::ZERO
                        } else {
                            C::Scalar::from((13 * column + row + 1) as u64)
                        };
                        let cell = region.assign_advice_discarding_value(
                            *advice,
                            row,
                            Value::known(value),
                        );
                        if row == 0 {
                            cells.push(cell);
                        }
                    }
                }
                for fixed in &config.fixed {
                    cells.push(region.assign_fixed(*fixed, 0, C::Scalar::ZERO));
                }
                for pair in cells.windows(2) {
                    region.constrain_equal(pair[0], pair[1]);
                }
                Ok(cells[0])
            },
        )?;
        layouter.constrain_instance(first, config.instances[0], 0);
        Ok(())
    }
}
impl<C: CurveAffine, const DEGREE: usize, const MIXED: bool, const I: usize> Circuit<C::Scalar>
    for InverseCircuit<C, DEGREE, MIXED, I>
{
    type Config = InverseConfig;
    type FloorPlanner = V1;
    #[cfg(feature = "circuit-params")]
    type Params = ();
    fn without_witnesses(&self) -> Self {
        Self(self.0.without_witnesses())
    }
    fn configure(meta: &mut ConstraintSystem<C::Scalar>) -> Self::Config {
        let config = InverseConfig {
            advice: (0..if MIXED { 3 } else { 0 })
                .map(|_| meta.advice_column())
                .collect(),
            fixed: (0..if MIXED { 2 } else { 0 })
                .map(|_| meta.fixed_column())
                .collect(),
            instances: (0..I).map(|_| meta.instance_column()).collect(),
        };
        meta.set_minimum_degree(DEGREE);
        // A deliberately unsatisfied genuine constant gate yields a nonzero high inverse
        // tail when q<m; inverse parity must not silently add a tail-zero validity rule.
        meta.create_gate("inverse constant-one nonzero remainder", |_| {
            vec![Expression::Constant(C::Scalar::ONE)]
        });
        if MIXED {
            assert!(I >= 4 && DEGREE >= 4);
            let challenge = meta.challenge_usable_after(FirstPhase);
            meta.create_gate("inverse mixed graph", |meta| {
                let a = meta.query_advice(config.advice[0], Rotation::cur());
                let b = meta.query_advice(config.advice[1], Rotation::next());
                let fixed = meta.query_fixed(config.fixed[0], Rotation::prev());
                let instance = meta.query_instance(config.instances[3], Rotation::next());
                let challenge = meta.query_challenge(challenge);
                vec![a.clone() * a.clone() * a * b + challenge * fixed + instance]
            });
            for column in &config.advice {
                meta.enable_equality(*column);
            }
            for column in &config.fixed {
                meta.enable_equality(*column);
            }
            for column in &config.instances {
                meta.enable_equality(*column);
            }
            for index in 0..2 {
                meta.lookup_any("inverse identity membership", |meta| {
                    let expression = meta.query_instance(config.instances[index], Rotation::cur());
                    vec![(expression.clone(), expression)]
                });
            }
        }
        config
    }
    fn synthesize_for_measurement(
        &self,
        config: Self::Config,
        layouter: impl Layouter<C::Scalar>,
    ) -> Result<(), Error> {
        self.run(config, layouter, true)
    }
    fn synthesize(
        &self,
        config: Self::Config,
        layouter: impl Layouter<C::Scalar>,
    ) -> Result<(), Error> {
        self.run(config, layouter, false)
    }
}
fn inverse_key<C, const DEGREE: usize, const MIXED: bool, const I: usize>(
    params: &ParamsIPA<C>,
) -> ProvingKey<C>
where
    C: CurveAffine,
    C::Scalar: FromUniformBytes<64>,
{
    let shared = Shared::<C>::new();
    let circuit = InverseCircuit::<C, DEGREE, MIXED, I>(Producer::new(&shared, 3));
    let vk = keygen_vk_custom(params, &circuit, true).unwrap();
    keygen_pk(params, vk, &circuit).unwrap()
}
macro_rules! inverse_member {
    ($params:expr,$pk:expr,$instances:expr,$shared:expr,$controls:expr) => {
        prepare_single_phase_stored_ipa_prefix_v1::<C, _, _, _, Challenge255<C>, _, Q, M>(
            $params,
            $pk,
            InverseCircuit::<C, DEGREE, MIXED, I>(Producer::new($shared, 3)),
            $instances,
            InverseProvider {
                inner: Provider::new($shared),
                controls: Arc::clone($controls),
            },
            QuietRng {
                inner: Rng(Arc::clone($shared)),
                controls: Arc::clone($controls),
            },
            QuietTranscript {
                inner: RecordingTranscript::new($shared),
                controls: Arc::clone($controls),
            },
        )
        .unwrap()
        .stage_advice_coefficients()
        .unwrap()
        .compress_lookups(1 << 26)
        .unwrap()
        .sort_lookup_values(1 << 26)
        .unwrap()
        .prepare_lookup_membership(1 << 26)
        .unwrap()
    };
}
macro_rules! inverse_input {
    ($params:expr,$pk:expr,$instances:expr,$shared:expr,$controls:expr) => {
        inverse_member!($params, $pk, $instances, $shared, $controls)
            .commit_permuted_lookups(1 << 26)
            .unwrap()
            .commit_products(1 << 26)
            .unwrap()
            .commit_vanishing_and_stage_coefficients(1 << 26)
            .unwrap()
            .evaluate_quotient_numerator(1 << 26)
            .unwrap()
    };
}
fn values<C: CurveAffine, const I: usize>(empty: bool) -> Vec<Vec<C::Scalar>> {
    (0..I)
        .map(|column| {
            if empty {
                Vec::new()
            } else {
                vec![
                    C::Scalar::ZERO,
                    C::Scalar::from((column + 1) as u64),
                    C::Scalar::from((2 * column + 3) as u64),
                ]
            }
        })
        .collect()
}
fn stored_values<C: CurveAffine>(
    shared: &Arc<Shared<C>>,
    expected: StoredPolynomialLayoutV1,
) -> Vec<C::Scalar>
where
    C::Scalar: StoredAssignmentFieldV1,
{
    let log = shared.log.lock().unwrap();
    let (_, values) = log
        .sealed
        .iter()
        .find(|(layout, _)| *layout == expected)
        .expect("actual sealed receipt");
    values
        .iter()
        .map(|encoded| Option::<C::Scalar>::from(C::Scalar::from_repr(*encoded)).unwrap())
        .collect()
}
fn read_values<C: CurveAffine>(
    snapshot: &mut InverseSnapshot<C>,
    expected: StoredPolynomialLayoutV1,
) -> Vec<C::Scalar>
where
    C::Scalar: StoredAssignmentFieldV1,
{
    let mut result = Vec::new();
    for chunk in 0..expected.chunk_count() as u64 {
        snapshot
            .with_chunk(expected, chunk, |values| {
                assert_eq!(values.len(), expected.chunk_scalar_count(chunk)?);
                for encoded in values {
                    result.push(Option::<C::Scalar>::from(C::Scalar::from_repr(*encoded)).unwrap());
                }
                Ok(())
            })
            .unwrap();
    }
    result
}
fn public_values<C: CurveAffine>(
    pk: &ProvingKey<C>,
) -> Vec<(Vec<C::Scalar>, *const C::Scalar, usize)> {
    pk.fixed_polys
        .iter()
        .chain(&pk.permutation.polys)
        .chain([&pk.l0, &pk.l_last, &pk.l_active_row])
        .map(|p| (p.to_vec(), p.values.as_ptr(), p.values.capacity()))
        .collect()
}
fn sentinel<C: CurveAffine>(provider: &mut InverseProvider<C>, k: u32) -> InverseSnapshot<C>
where
    C::Scalar: StoredAssignmentFieldV1,
{
    let mut writer = provider
        .create(
            C::Scalar::STORED_FIELD,
            StoredPolynomialBasisV1::Coefficient,
            k,
            StoredPolynomialRoleV1::Instance { column: 777 },
        )
        .unwrap();
    let layout = writer.layout();
    for chunk in 0..layout.chunk_count() as u64 {
        writer
            .write_chunk(
                chunk,
                &vec![C::Scalar::from(61).to_repr(); layout.chunk_scalar_count(chunk).unwrap()],
            )
            .unwrap();
    }
    writer.seal().unwrap()
}
fn check_sentinel<C: CurveAffine>(snapshot: &mut InverseSnapshot<C>)
where
    C::Scalar: StoredAssignmentFieldV1,
{
    let expected = snapshot.layout();
    assert!(
        read_values(snapshot, expected)
            .iter()
            .all(|value| *value == C::Scalar::from(61))
    );
}

fn assert_lifecycle(
    events: &[IoEvent],
    raw: &[StoredPolynomialLayoutV1],
    q: usize,
    n: usize,
) -> Vec<StoredPolynomialLayoutV1> {
    let m = raw.len();
    let chunks = n.div_ceil(256);
    let aliases = events
        .iter()
        .filter(|event| {
            event.kind == IoKind::Create
                && matches!(
                    event.layout.role(),
                    StoredPolynomialRoleV1::QuotientAliasedPart { .. }
                )
        })
        .map(|event| event.layout)
        .collect::<Vec<_>>();
    let pieces = events
        .iter()
        .filter(|event| {
            event.kind == IoKind::Create
                && matches!(
                    event.layout.role(),
                    StoredPolynomialRoleV1::QuotientPiece { .. }
                )
        })
        .map(|event| event.layout)
        .collect::<Vec<_>>();
    assert_eq!(aliases.len(), m);
    assert_eq!(pieces.len(), q);
    let position = |kind, layout| {
        events
            .iter()
            .position(|event| event.kind == kind && event.layout == layout)
            .unwrap()
    };
    for (part, layout) in aliases.iter().enumerate() {
        assert_eq!(
            layout.role(),
            StoredPolynomialRoleV1::QuotientAliasedPart {
                part: part as u32,
                extension_log: (m as u32).ilog2()
            }
        );
        assert_eq!(layout.basis(), StoredPolynomialBasisV1::Coefficient);
        assert!(position(IoKind::Create, *layout) < position(IoKind::Read, raw[part]));
        assert!(position(IoKind::Seal, *layout) < position(IoKind::DropSnapshot, raw[part]));
        if part + 1 < m {
            assert!(
                position(IoKind::DropSnapshot, raw[part])
                    < position(IoKind::Create, aliases[part + 1])
            );
        }
    }
    let first_alias_read = events
        .iter()
        .position(|event| {
            event.kind == IoKind::Read
                && matches!(
                    event.layout.role(),
                    StoredPolynomialRoleV1::QuotientAliasedPart { .. }
                )
        })
        .unwrap();
    for (piece, layout) in pieces.iter().enumerate() {
        assert_eq!(
            layout.role(),
            StoredPolynomialRoleV1::QuotientPiece {
                piece: piece as u32
            }
        );
        assert!(position(IoKind::Create, *layout) < first_alias_read);
    }
    let last_piece_seal = pieces
        .iter()
        .map(|layout| position(IoKind::Seal, *layout))
        .max()
        .unwrap();
    for layout in &aliases {
        assert!(position(IoKind::DropSnapshot, *layout) > last_piece_seal);
    }
    let actual_mix_io = events
        .iter()
        .filter(|event| {
            (event.kind == IoKind::Read
                && matches!(
                    event.layout.role(),
                    StoredPolynomialRoleV1::QuotientAliasedPart { .. }
                ))
                || (event.kind == IoKind::Write
                    && matches!(
                        event.layout.role(),
                        StoredPolynomialRoleV1::QuotientPiece { .. }
                    ))
        })
        .copied()
        .collect::<Vec<_>>();
    let mut expected_mix_io = Vec::new();
    for chunk in 0..chunks {
        for layout in &aliases {
            expected_mix_io.push(IoEvent {
                kind: IoKind::Read,
                layout: *layout,
                chunk: chunk as u64,
            });
        }
        for layout in &pieces {
            expected_mix_io.push(IoEvent {
                kind: IoKind::Write,
                layout: *layout,
                chunk: chunk as u64,
            });
        }
    }
    assert_eq!(actual_mix_io, expected_mix_io);
    for (kind, count) in [
        (IoKind::Create, m + q),
        (IoKind::Read, 2 * m * chunks),
        (IoKind::Write, (m + q) * chunks),
        (IoKind::Seal, m + q),
        (IoKind::DropSnapshot, 2 * m),
        (IoKind::DropWriter, 0),
    ] {
        assert_eq!(
            events.iter().filter(|event| event.kind == kind).count(),
            count,
            "{kind:?}"
        );
    }
    aliases
}
fn inverse_success<
    C,
    const DEGREE: usize,
    const MIXED: bool,
    const I: usize,
    const Q: bool,
    const M: u64,
>(
    k: u32,
    empty: bool,
) where
    C: CurveAffine + crate::SerdeCurveAffine,
    C::Scalar: StoredAssignmentFieldV1
        + WithSmallOrderMulGroup<3>
        + FromUniformBytes<64>
        + crate::SerdePrimeField,
{
    let params = ParamsIPA::<C>::new(k);
    let pk = inverse_key::<C, DEGREE, MIXED, I>(&params);
    assert_eq!(pk.vk.cs.degree(), DEGREE);
    let values = values::<C, I>(empty);
    let instances = values.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let shared = Shared::<C>::new();
    let controls = Controls::new();
    let member = inverse_member!(&params, pk, &instances, &shared, &controls);
    let domain = &member.compressed.inner.pk.vk.domain;
    let n = domain.get_n() as usize;
    let m = domain.extended_len() / n;
    let q = domain.get_quotient_poly_degree();
    let advice = member
        .compressed
        .inner
        .advice
        .layouts()
        .unwrap()
        .map(|layout| domain.lagrange_from_vec(stored_values::<C>(&shared, layout)))
        .collect::<Vec<_>>();
    let dense_instances = values
        .iter()
        .map(|values| {
            let mut values = values.clone();
            values.resize(n, C::Scalar::ZERO);
            domain.lagrange_from_vec(values)
        })
        .collect::<Vec<_>>();
    let challenges = member
        .compressed
        .inner
        .advice
        .challenges()
        .unwrap()
        .collect::<Vec<_>>();
    let oracle_shared = Shared::<C>::new();
    oracle_shared.log.lock().unwrap().events = shared.log.lock().unwrap().events.clone();
    let mut oracle_transcript = RecordingTranscript {
        inner: member.compressed.inner.transcript.inner.inner.clone(),
        shared: Arc::clone(&oracle_shared),
    };
    let mut oracle_rng = shared.rng.lock().unwrap().clone();
    let ordinary = crate::plonk::lookup::prover::stored_quotient_ordinary_oracle(
        &member.compressed.inner.pk,
        &params,
        member.compressed.theta,
        &advice,
        &dense_instances,
        &challenges,
        &mut oracle_rng,
        &mut oracle_transcript,
    )
    .unwrap();
    assert_eq!(ordinary.numerator, ordinary.borrowed_numerator);
    let mut dense = domain.empty_extended();
    dense.values.copy_from_slice(&ordinary.numerator);
    let divided = domain.divide_by_vanishing_poly(dense);
    let expected = domain.extended_to_coeff(divided.clone());
    assert_eq!(expected.len(), n * m);
    if !MIXED && q < m {
        assert!(
            expected[n * q..]
                .iter()
                .any(|value| *value != C::Scalar::ZERO),
            "fixture must retain a nonzero discarded tail"
        );
    }
    let expected_aliases = (0..m)
        .map(|part| {
            let values =
                domain.lagrange_from_vec((0..n).map(|row| divided[row * m + part]).collect());
            domain
                .extended_part_to_coeff(
                    values,
                    domain.get_extended_omega().pow_vartime([part as u64]),
                )
                .to_vec()
        })
        .collect::<Vec<_>>();
    let input = member
        .commit_permuted_lookups(1 << 26)
        .unwrap()
        .commit_products(1 << 26)
        .unwrap()
        .commit_vanishing_and_stage_coefficients(1 << 26)
        .unwrap()
        .evaluate_quotient_numerator(1 << 26)
        .unwrap();
    let raw = input.parts.iter().map(|p| p.layout).collect::<Vec<_>>();
    for (part, layout) in raw.iter().enumerate() {
        assert_eq!(
            stored_values::<C>(&shared, *layout),
            (0..n)
                .map(|row| ordinary.numerator[row * m + part])
                .collect::<Vec<_>>()
        );
    }
    let original = controls
        .bank
        .lock()
        .unwrap()
        .live
        .iter()
        .filter(|(_, layout)| layout.role() != StoredPolynomialRoleV1::QuotientNumerator)
        .map(|(ordinal, layout)| (*ordinal, *layout))
        .collect::<BTreeMap<_, _>>();
    let original_values = original
        .values()
        .map(|layout| (*layout, stored_values::<C>(&shared, *layout)))
        .collect::<Vec<_>>();
    let public = public_values(&input.inner.pk);
    let fixed_pointer = input.inner.pk.fixed_polys.as_ptr();
    let sigma_pointer = input.inner.pk.permutation.polys.as_ptr();
    let ev = format!("{:?}", input.inner.pk.ev);
    let vk = input
        .inner
        .pk
        .get_vk()
        .to_bytes(crate::SerdeFormat::Processed);
    let advice_layouts = input.inner.advice.layouts().unwrap().collect::<Vec<_>>();
    let phase_challenges = input.inner.advice.challenges().unwrap().collect::<Vec<_>>();
    let products = input
        .inner
        .permutations
        .iter()
        .chain(
            input
                .inner
                .lookups
                .iter()
                .flat_map(|lookup| [&lookup.input, &lookup.table, &lookup.product]),
        )
        .chain(std::iter::once(&input.inner.random))
        .map(|p| (p.coefficient.layout, (p.blind.0).0, p.commitment))
        .collect::<Vec<_>>();
    let protocol = (
        *input.inner.theta,
        *input.inner.beta,
        *input.inner.gamma,
        *input.inner.y,
    );
    let calls = shared.log.lock().unwrap().rng_calls;
    let events_before = shared.log.lock().unwrap().events.clone();
    assert_eq!(events_before, oracle_shared.log.lock().unwrap().events);
    controls.arm(None);
    controls.protocol_armed.store(true, Ordering::SeqCst);
    crate::plonk::prover::stored::quotient_inverse::take_clear_observations();
    crate::plonk::prover::stored::quotient_inverse::take_reuse_observations();
    let mut actual = input.stage_quotient_coefficients(1 << 26).unwrap();
    assert_eq!(shared.log.lock().unwrap().rng_calls, calls);
    assert_eq!(shared.log.lock().unwrap().events, events_before);
    assert_eq!(actual.pieces.len(), q);
    assert!(std::ptr::eq(actual.inner.params, &params));
    assert_eq!(actual.inner.instances.as_ptr(), instances.as_ptr());
    assert_eq!(
        actual.inner.advice.layouts().unwrap().collect::<Vec<_>>(),
        advice_layouts
    );
    assert_eq!(
        actual
            .inner
            .advice
            .challenges()
            .unwrap()
            .collect::<Vec<_>>(),
        phase_challenges
    );
    assert_eq!(public_values(&actual.inner.pk), public);
    assert_eq!(actual.inner.pk.fixed_polys.as_ptr(), fixed_pointer);
    assert_eq!(actual.inner.pk.permutation.polys.as_ptr(), sigma_pointer);
    assert_eq!(format!("{:?}", actual.inner.pk.ev), ev);
    assert_eq!(
        actual
            .inner
            .pk
            .get_vk()
            .to_bytes(crate::SerdeFormat::Processed),
        vk
    );
    assert_eq!(
        (
            *actual.inner.theta,
            *actual.inner.beta,
            *actual.inner.gamma,
            *actual.inner.y
        ),
        protocol
    );
    assert_eq!(
        actual
            .inner
            .permutations
            .iter()
            .chain(actual.inner.lookups.iter().flat_map(|lookup| [
                &lookup.input,
                &lookup.table,
                &lookup.product
            ]))
            .chain(std::iter::once(&actual.inner.random))
            .map(|p| (p.coefficient.layout, (p.blind.0).0, p.commitment))
            .collect::<Vec<_>>(),
        products
    );
    assert!(Arc::ptr_eq(&actual.inner.rng.inner.0, &shared));
    assert!(Arc::ptr_eq(&actual.inner.transcript.inner.shared, &shared));
    let events = controls.bank.lock().unwrap().events.clone();
    let aliases = assert_lifecycle(&events, &raw, q, n);
    for (layout, expected) in aliases.iter().zip(expected_aliases) {
        assert_eq!(stored_values::<C>(&shared, *layout), expected);
    }
    let (allocations, capacity, alias_pointer, mix_pointer) =
        crate::plonk::prover::stored::quotient_inverse::take_reuse_observations();
    assert_eq!(allocations, 1);
    assert!(capacity >= n.max(m * n.min(256)));
    assert_eq!(alias_pointer, mix_pointer);
    assert_ne!(alias_pointer, 0);
    let (fields, all_zero, bytes, bytes_zero) =
        crate::plonk::prover::stored::quotient_inverse::take_clear_observations();
    assert!(fields >= n.max(m * n.min(256)) && bytes >= 256 * 32 && all_zero && bytes_zero);
    let bank = controls.bank.lock().unwrap();
    assert_eq!(bank.live.len(), original.len() + q);
    assert_eq!(bank.peak, original.len() + m + q);
    for (ordinal, layout) in &original {
        assert_eq!(bank.live.get(ordinal), Some(layout));
    }
    assert!(
        raw.iter()
            .chain(&aliases)
            .all(|layout| !bank.live.contains_key(&layout.ordinal()))
    );
    drop(bank);
    for (layout, values) in original_values {
        assert_eq!(stored_values::<C>(&shared, layout), values);
    }
    let mut previous = aliases.last().unwrap().ordinal();
    for (piece, polynomial) in actual.pieces.iter_mut().enumerate() {
        assert_eq!(
            polynomial.layout.role(),
            StoredPolynomialRoleV1::QuotientPiece {
                piece: piece as u32
            }
        );
        assert_eq!(
            polynomial.layout.basis(),
            StoredPolynomialBasisV1::Coefficient
        );
        assert!(polynomial.layout.ordinal() > previous);
        previous = polynomial.layout.ordinal();
        assert_eq!(
            read_values(&mut polynomial.snapshot, polynomial.layout),
            expected[piece * n..(piece + 1) * n]
        );
    }
    assert_eq!(
        actual.inner.advice.greatest_ordinal().unwrap(),
        Some(previous)
    );
    assert_eq!(
        actual.inner.transcript.inner.inner.clone().finalize(),
        oracle_transcript.inner.clone().finalize()
    );
    controls.protocol_armed.store(false, Ordering::SeqCst);
    assert_eq!(
        actual.inner.transcript.squeeze_challenge().get_scalar(),
        oracle_transcript.squeeze_challenge().get_scalar()
    );
    let mut next_a = [0; 64];
    let mut next_b = [0; 64];
    actual.inner.rng.fill_bytes(&mut next_a);
    oracle_rng.fill_bytes(&mut next_b);
    assert_eq!(next_a, next_b);
    drop(actual);
    assert_dropped(&shared);
    assert!(controls.bank.lock().unwrap().live.is_empty());
}
fn inverse_matrix<C>()
where
    C: CurveAffine + crate::SerdeCurveAffine,
    C::Scalar: StoredAssignmentFieldV1
        + WithSmallOrderMulGroup<3>
        + FromUniformBytes<64>
        + crate::SerdePrimeField,
{
    for k in [4, 8, 9] {
        inverse_success::<C, 3, false, 0, false, 0>(k, true);
        inverse_success::<C, 4, false, 0, false, 0>(k, true);
        inverse_success::<C, 5, false, 0, false, 0>(k, true);
        inverse_success::<C, 6, false, 0, false, 0>(k, true);
        inverse_success::<C, 8, false, 0, false, 0>(k, true);
        inverse_success::<C, 9, false, 0, false, 0>(k, true);
        inverse_success::<C, 4, true, 4, false, 0>(k, false);
        inverse_success::<C, 6, true, 4, true, 0>(k, false);
        inverse_success::<C, 6, true, 4, true, 6>(k, true);
    }
}
#[test]
fn both_pasta_inverse_matches_complete_ordinary_trajectory_all_aliases_and_nonzero_discarded_tail_preserving_original_owners()
 {
    inverse_matrix::<EqAffine>();
    inverse_matrix::<EpAffine>();
}
#[test]
fn both_pasta_inverse_reachable_498_instance_profile_uses_exact_512_handles_and_one_reused_field_allocation()
 {
    inverse_success::<EqAffine, 6, false, 498, false, 0>(4, false);
    inverse_success::<EpAffine, 6, false, 498, false, 0>(4, false);
}

fn stage_role(role: StoredPolynomialRoleV1) -> u8 {
    match role {
        StoredPolynomialRoleV1::QuotientNumerator => 0,
        StoredPolynomialRoleV1::QuotientAliasedPart { .. } => 1,
        StoredPolynomialRoleV1::QuotientPiece { .. } => 2,
        _ => 3,
    }
}
fn original_role(role: StoredPolynomialRoleV1) -> u8 {
    match role {
        StoredPolynomialRoleV1::Advice { .. } => 0,
        StoredPolynomialRoleV1::Instance { .. } => 1,
        StoredPolynomialRoleV1::CopyPermutationProduct { .. } => 2,
        StoredPolynomialRoleV1::LookupPermuted {
            side: StoredLookupSideV1::Input,
            ..
        } => 3,
        StoredPolynomialRoleV1::LookupPermuted {
            side: StoredLookupSideV1::Table,
            ..
        } => 4,
        StoredPolynomialRoleV1::LookupProduct { .. } => 5,
        StoredPolynomialRoleV1::VanishingRandom => 6,
        StoredPolynomialRoleV1::QuotientNumerator => 7,
        StoredPolynomialRoleV1::QuotientAliasedPart { .. } => 8,
        StoredPolynomialRoleV1::QuotientPiece { .. } => 9,
        _ => panic!("unexpected retained inverse role"),
    }
}
fn inverse_fault_cases(
    events: &[IoEvent],
    initial: &BTreeMap<u64, StoredPolynomialLayoutV1>,
    random: u64,
    all: bool,
) -> Vec<(usize, Action)> {
    let mut cases = Vec::new();
    let mut representatives = BTreeMap::new();
    for (index, event) in events.iter().enumerate() {
        representatives
            .entry((event.kind, stage_role(event.layout.role())))
            .or_insert((index, index))
            .1 = index;
        if all {
            if matches!(
                event.kind,
                IoKind::Create | IoKind::Read | IoKind::Write | IoKind::Seal
            ) {
                cases.push((index, Action::Error));
            }
            cases.push((index, Action::Panic));
            cases.push((index, Action::Drift(random)));
            if event.kind == IoKind::Read {
                for action in [Action::Short, Action::Long, Action::Encoding] {
                    cases.push((index, action));
                }
            }
        }
    }
    assert_eq!(
        representatives.len(),
        10,
        "all raw/alias/piece callback classes are exercised"
    );
    let mut live = initial.clone();
    for (index, event) in events.iter().enumerate() {
        if event.kind == IoKind::DropSnapshot {
            assert!(live.remove(&event.layout.ordinal()).is_some());
        }
        if all
            && representatives
                .values()
                .any(|(first, last)| index == *first || index == *last)
        {
            let mut victims = BTreeMap::new();
            for (ordinal, layout) in &live {
                if layout.role() == (StoredPolynomialRoleV1::Instance { column: 777 }) {
                    continue;
                }
                victims
                    .entry(original_role(layout.role()))
                    .or_insert((*ordinal, *ordinal))
                    .1 = *ordinal;
            }
            // At representative early/late boundaries, mutate the first/last live receipt
            // of every original/raw/alias/final class, including untouched future owners.
            for (first, last) in victims.into_values() {
                cases.push((index, Action::Drift(first)));
                cases.push((index, Action::Drift(last)));
            }
        }
        if event.kind == IoKind::Create {
            assert!(live.insert(event.layout.ordinal(), event.layout).is_none());
        }
    }
    if !all {
        for ((kind, _), (_, last)) in representatives {
            if matches!(kind, IoKind::Read | IoKind::Write) {
                assert_eq!(
                    events[last].chunk, 1,
                    "late boundary must be the second chunk"
                );
                cases.push((last, Action::Error));
                cases.push((last, Action::Panic));
                cases.push((last, Action::Drift(random)));
                if kind == IoKind::Read {
                    cases.push((last, Action::Encoding));
                }
            }
        }
    }
    let mut unique = Vec::new();
    for case in cases {
        if !unique.contains(&case) {
            unique.push(case);
        }
    }
    unique
}
fn inverse_failures<C>(k: u32, all: bool)
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
{
    const DEGREE: usize = 6;
    const MIXED: bool = true;
    const I: usize = 4;
    const Q: bool = false;
    const M: u64 = 0;
    let params = ParamsIPA::<C>::new(k);
    let pk = inverse_key::<C, DEGREE, MIXED, I>(&params);
    let values = values::<C, I>(false);
    let instances = values.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let shared = Shared::<C>::new();
    let controls = Controls::new();
    let mut input = inverse_input!(&params, pk.clone(), &instances, &shared, &controls);
    let other = sentinel(&mut input.inner.provider, k);
    let initial = controls.bank.lock().unwrap().live.clone();
    let random = input.inner.random.coefficient.layout.ordinal();
    controls.arm(None);
    let success = input.stage_quotient_coefficients(1 << 26).unwrap();
    let events = controls.bank.lock().unwrap().events.clone();
    let cases = inverse_fault_cases(&events, &initial, random, all);
    assert_eq!(events.len(), if all { 71 } else { 100 });
    assert_eq!(cases.len(), if all { 549 } else { 14 });
    drop(success);
    drop(other);
    assert_dropped(&shared);
    for (target, action) in cases {
        let shared = Shared::<C>::new();
        let controls = Controls::new();
        let mut input = inverse_input!(&params, pk.clone(), &instances, &shared, &controls);
        let mut other = sentinel(&mut input.inner.provider, k);
        let calls = shared.log.lock().unwrap().rng_calls;
        let protocol = shared.log.lock().unwrap().events.clone();
        controls.arm(Some((target, action)));
        controls.protocol_armed.store(true, Ordering::SeqCst);
        crate::plonk::prover::stored::quotient_inverse::take_clear_observations();
        let result = catch_unwind(AssertUnwindSafe(|| {
            input.stage_quotient_coefficients(1 << 26)
        }));
        assert!(
            matches!(&result, Err(_) | Ok(Err(_))),
            "accepted inverse callback {target} {:?} {action:?}",
            events[target]
        );
        assert!(
            controls.bank.lock().unwrap().fault.is_none(),
            "selected callback was not reached"
        );
        assert_eq!(controls.bank.lock().unwrap().events[target], events[target]);
        assert_eq!(controls.bank.lock().unwrap().live.len(), 1);
        assert!(!controls.window.load(Ordering::SeqCst));
        assert_eq!(shared.log.lock().unwrap().rng_calls, calls);
        assert_eq!(shared.log.lock().unwrap().events, protocol);
        let (fields, zero, bytes, bytes_zero) =
            crate::plonk::prover::stored::quotient_inverse::take_clear_observations();
        assert!(
            fields >= ((1_usize << k).max(8 * (1_usize << k).min(256)))
                && bytes >= 8192
                && zero
                && bytes_zero
        );
        check_sentinel(&mut other);
        drop(result);
        drop(other);
        assert_dropped(&shared);
        assert!(controls.bank.lock().unwrap().live.is_empty());
    }
}
#[test]
fn both_pasta_inverse_every_storage_boundary_and_earlier_future_original_writer_or_output_mutation_fails_closed()
 {
    inverse_failures::<EqAffine>(4, true);
    inverse_failures::<EpAffine>(4, true);
}
#[test]
fn both_pasta_inverse_late_raw_alias_and_piece_chunk_errors_unwinds_destroy_all_owned_banks() {
    inverse_failures::<EqAffine>(9, false);
    inverse_failures::<EpAffine>(9, false);
}
fn inverse_metadata<C>()
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
{
    const DEGREE: usize = 6;
    const MIXED: bool = true;
    const I: usize = 4;
    const Q: bool = true;
    const M: u64 = 6;
    let params = ParamsIPA::<C>::new(4);
    let pk = inverse_key::<C, DEGREE, MIXED, I>(&params);
    let values = values::<C, I>(false);
    let instances = values.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let shared = Shared::<C>::new();
    let controls = Controls::new();
    let mut input = inverse_input!(&params, pk.clone(), &instances, &shared, &controls);
    let other = sentinel(&mut input.inner.provider, 4);
    controls.arm(None);
    let success = input.stage_quotient_coefficients(1 << 26).unwrap();
    let events = controls.bank.lock().unwrap().events.clone();
    let mut representatives = BTreeMap::new();
    for (index, event) in events.iter().enumerate() {
        if matches!(event.kind, IoKind::Create | IoKind::Write | IoKind::Seal) {
            representatives
                .entry((event.kind, stage_role(event.layout.role())))
                .or_insert(index);
        }
    }
    assert_eq!(representatives.len(), 6);
    drop(success);
    drop(other);
    assert_dropped(&shared);
    for target in representatives.into_values() {
        for change in [
            Change::Field,
            Change::K,
            Change::Context,
            Change::Role,
            Change::Part,
            Change::Extension,
            Change::Ordinal,
            Change::Exhausted,
            Change::Insufficient,
        ] {
            let shared = Shared::<C>::new();
            let controls = Controls::new();
            let mut input = inverse_input!(&params, pk.clone(), &instances, &shared, &controls);
            let mut other = sentinel(&mut input.inner.provider, 4);
            controls.arm(Some((target, Action::Change(change))));
            controls.protocol_armed.store(true, Ordering::SeqCst);
            crate::plonk::prover::stored::quotient_inverse::take_clear_observations();
            let result = input.stage_quotient_coefficients(1 << 26);
            assert!(
                result.is_err(),
                "accepted {change:?} at {:?}",
                events[target]
            );
            assert!(controls.bank.lock().unwrap().fault.is_none());
            assert_eq!(controls.bank.lock().unwrap().live.len(), 1);
            let (_, zero, _, bytes_zero) =
                crate::plonk::prover::stored::quotient_inverse::take_clear_observations();
            assert!(zero && bytes_zero);
            assert!(!controls.window.load(Ordering::SeqCst));
            check_sentinel(&mut other);
            drop(result);
            drop(other);
            assert_dropped(&shared);
        }
    }
}
#[test]
fn both_pasta_inverse_alias_and_piece_writer_metadata_substitutions_never_return_reusable_success()
{
    inverse_metadata::<EqAffine>();
    inverse_metadata::<EpAffine>();
}

fn inverse_preflight<C>()
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
{
    const DEGREE: usize = 6;
    const MIXED: bool = true;
    const I: usize = 4;
    const Q: bool = false;
    const M: u64 = 0;
    let params = ParamsIPA::<C>::new(4);
    let alternate = ParamsIPA::<C>::new(4);
    let wrong_k = ParamsIPA::<C>::new(5);
    let pk = inverse_key::<C, DEGREE, MIXED, I>(&params);
    let values = values::<C, I>(false);
    let instances = values.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let too_long = vec![C::Scalar::ZERO; 17];
    let bad_instances = vec![too_long.as_slice(); I];
    for case in 0..45 {
        let shared = Shared::<C>::new();
        let controls = Controls::new();
        let mut input = inverse_input!(&params, pk.clone(), &instances, &shared, &controls);
        let mut other = sentinel(&mut input.inner.provider, 4);
        let minimum = crate::plonk::prover::stored::quotient_inverse::scratch_bytes::<
            C,
            InverseProvider<C>,
        >(&input.inner.pk)
        .unwrap();
        assert!(
            minimum > (8 * 16 + 8) * 32 + 8192,
            "budget includes metadata and FFT scratch beyond field and encoded allocations"
        );
        let mut budget = 1 << 26;
        match case {
            0 => budget = 0,
            1 => budget = minimum - 1,
            2 => input.inner.usable_rows += 1,
            3 => input.inner.params = &alternate,
            4 => input.inner.params = &wrong_k,
            5 => input.inner.pk.vk.cs_degree = 2,
            6 => {
                input.inner.pk.fixed_polys.pop().unwrap();
            }
            7 => {
                input.inner.pk.permutation.polys.pop().unwrap();
            }
            8 => {
                input.inner.pk.fixed_polys[0].values.pop();
            }
            9 => {
                input.inner.pk.permutation.polys[0].values.pop();
            }
            10 => {
                input.inner.pk.l0.values.pop();
            }
            11 => {
                input.inner.pk.l_last.values.pop();
            }
            12 => {
                input.inner.pk.l_active_row.values.pop();
            }
            13 => input.inner.pk.fixed_values.push(
                input
                    .inner
                    .pk
                    .vk
                    .domain
                    .lagrange_from_vec(vec![C::Scalar::ZERO; 16]),
            ),
            14 => input.inner.pk.permutation.permutations.push(
                input
                    .inner
                    .pk
                    .vk
                    .domain
                    .lagrange_from_vec(vec![C::Scalar::ZERO; 16]),
            ),
            15 => {
                input.inner.instance_coefficients.pop().unwrap();
            }
            16 => input.inner.instances = &[],
            17 => input.inner.instances = &bad_instances,
            18 => {
                input.inner.permutations.pop().unwrap();
            }
            19 => {
                input.inner.lookups.pop().unwrap();
            }
            20 => {
                input.inner.pk.ev.lookups.pop().unwrap();
            }
            21 => {
                input.inner.pk.vk.cs.advice_column_phase.pop().unwrap();
            }
            22 => {
                input.inner.pk.vk.cs.challenge_phase.pop().unwrap();
            }
            23 => input.inner.pk.vk.cs.num_advice_columns += 1,
            24 => input.inner.pk.vk.cs.num_challenges += 1,
            25 => {
                input.inner.pk.vk.cs.permutation.columns[0] =
                    Column::<Instance>::new(99, Instance).into()
            }
            26 => {
                input.inner.pk.vk.cs.permutation.columns[1] =
                    input.inner.pk.vk.cs.permutation.columns[0]
            }
            27 => {
                input.inner.instance_coefficients[0].layout =
                    input.inner.instance_coefficients[1].layout
            }
            28 => {
                input.inner.random.coefficient.layout = input.inner.instance_coefficients[0].layout
            }
            29 => controls.after(
                Some(Action::Drift(
                    input
                        .inner
                        .advice
                        .layouts()
                        .unwrap()
                        .last()
                        .unwrap()
                        .ordinal(),
                )),
                input.parts[0].layout,
            ),
            30 => input.inner.pk.vk.cs.num_instance_columns = usize::MAX,
            31 => controls.after(
                Some(Action::Drift(
                    input.inner.random.coefficient.layout.ordinal(),
                )),
                input.parts[0].layout,
            ),
            32 => {
                input.inner.pk.vk.cs.lookups.pop().unwrap();
            }
            33 => input.inner.pk.vk.domain = EvaluationDomain::new(4, 3),
            34 => {
                input.parts.pop().unwrap();
            }
            35 => input.parts.swap(0, 1),
            36 => input.parts[0].layout = input.parts[1].layout,
            37 => input.parts[0].layout = changed(input.parts[0].layout, Change::Part),
            38 => input.parts[0].layout = changed(input.parts[0].layout, Change::Extension),
            39 => input.parts[0].layout = changed(input.parts[0].layout, Change::Field),
            40 => input.parts[0].layout = changed(input.parts[0].layout, Change::K),
            41 => input.parts[0].layout = changed(input.parts[0].layout, Change::Context),
            42 => controls.after(
                Some(Action::Drift(input.parts.last().unwrap().layout.ordinal())),
                input.parts[0].layout,
            ),
            43 => input.parts[0].layout = changed(input.parts[0].layout, Change::Ordinal),
            44 => {
                // Start from the genuine numerator owner, then append an extra live raw
                // receipt from its own provider. Inventory admission must reject it.
                let original = input.parts[0].layout;
                let mut writer = input
                    .inner
                    .provider
                    .create(
                        original.field(),
                        original.basis(),
                        original.k(),
                        original.role(),
                    )
                    .unwrap();
                let layout = writer.layout();
                for chunk in 0..layout.chunk_count() as u64 {
                    writer
                        .write_chunk(
                            chunk,
                            &vec![
                                C::Scalar::ZERO.to_repr();
                                layout.chunk_scalar_count(chunk).unwrap()
                            ],
                        )
                        .unwrap();
                }
                input.parts.push(
                    crate::plonk::prover::stored::lookup_permuted::PermutedPolynomialV1 {
                        layout,
                        snapshot: writer.seal().unwrap(),
                    },
                );
            }
            _ => unreachable!(),
        }
        let (created, reads, writes, calls, protocol) = {
            let log = shared.log.lock().unwrap();
            (
                log.created,
                log.reads,
                log.writes,
                log.rng_calls,
                log.events.clone(),
            )
        };
        controls.arm(None);
        controls.protocol_armed.store(true, Ordering::SeqCst);
        crate::plonk::prover::stored::quotient_inverse::take_clear_observations();
        let result = input.stage_quotient_coefficients(budget);
        assert!(result.is_err(), "inverse preflight {case} accepted");
        let log = shared.log.lock().unwrap();
        assert_eq!(
            (log.created, log.reads, log.writes, log.rng_calls),
            (created, reads, writes, calls)
        );
        assert_eq!(log.events, protocol);
        drop(log);
        let bank = controls.bank.lock().unwrap();
        assert_eq!(bank.live.len(), 1);
        assert!(
            bank.events
                .iter()
                .all(|event| event.kind == IoKind::DropSnapshot)
        );
        drop(bank);
        let (_, zero, _, bytes_zero) =
            crate::plonk::prover::stored::quotient_inverse::take_clear_observations();
        assert!(zero && bytes_zero);
        assert!(!controls.window.load(Ordering::SeqCst));
        check_sentinel(&mut other);
        drop(result);
        drop(other);
        assert_dropped(&shared);
        assert!(controls.bank.lock().unwrap().live.is_empty());
    }
}
#[test]
fn both_pasta_inverse_budget_original_key_geometry_context_and_raw_bank_preflights_refuse_before_backend_io()
 {
    inverse_preflight::<EqAffine>();
    inverse_preflight::<EpAffine>();
}

fn inverse_capacity_refusal<C, const I: usize>(with_sentinel: bool, preflight: bool)
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
{
    const DEGREE: usize = 6;
    const MIXED: bool = false;
    const Q: bool = false;
    const M: u64 = 0;
    let params = ParamsIPA::<C>::new(4);
    let pk = inverse_key::<C, DEGREE, MIXED, I>(&params);
    let values = values::<C, I>(false);
    let instances = values.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let shared = Shared::<C>::new();
    let controls = Controls::new();
    let mut input = inverse_input!(&params, pk, &instances, &shared, &controls);
    assert_eq!(
        controls.bank.lock().unwrap().live.len(),
        I + 1 + 8,
        "actual numerator entry"
    );
    let mut other = with_sentinel.then(|| sentinel(&mut input.inner.provider, 4));
    let minimum = crate::plonk::prover::stored::quotient_inverse::scratch_bytes::<
        C,
        InverseProvider<C>,
    >(&input.inner.pk);
    assert_eq!(minimum.is_err(), preflight);
    let calls = shared.log.lock().unwrap().rng_calls;
    let protocol = shared.log.lock().unwrap().events.clone();
    controls.arm(None);
    controls.protocol_armed.store(true, Ordering::SeqCst);
    crate::plonk::prover::stored::quotient_inverse::take_clear_observations();
    let result = input.stage_quotient_coefficients(1 << 26);
    assert!(matches!(
        result,
        Err(StoredLookupErrorV1::Store(
            StoredPolynomialErrorV1::Capacity
        ))
    ));
    let bank = controls.bank.lock().unwrap();
    assert_eq!(bank.live.len(), usize::from(with_sentinel));
    if preflight {
        assert!(
            bank.events
                .iter()
                .all(|event| event.kind == IoKind::DropSnapshot)
        );
    } else {
        assert_eq!(bank.peak, 512);
        let rejected = bank
            .events
            .iter()
            .rposition(|event| event.kind == IoKind::Create)
            .unwrap();
        assert_eq!(
            bank.events[rejected].layout.role(),
            StoredPolynomialRoleV1::QuotientPiece { piece: 4 }
        );
        assert!(
            bank.events[rejected + 1..]
                .iter()
                .all(|event| matches!(event.kind, IoKind::DropWriter | IoKind::DropSnapshot))
        );
    }
    drop(bank);
    assert_eq!(shared.log.lock().unwrap().rng_calls, calls);
    assert_eq!(shared.log.lock().unwrap().events, protocol);
    let (_, zero, _, bytes_zero) =
        crate::plonk::prover::stored::quotient_inverse::take_clear_observations();
    assert!(zero && bytes_zero);
    if let Some(snapshot) = other.as_mut() {
        check_sentinel(snapshot);
    }
    drop(other);
    assert_dropped(&shared);
    assert!(controls.bank.lock().unwrap().live.is_empty());
}
#[test]
fn both_pasta_inverse_exact_512_provider_limit_and_503_instance_numerator_entry_refuse_without_leaking_owners()
 {
    inverse_capacity_refusal::<EqAffine, 498>(true, false);
    inverse_capacity_refusal::<EpAffine, 498>(true, false);
    inverse_capacity_refusal::<EqAffine, 499>(true, true);
    inverse_capacity_refusal::<EpAffine, 499>(true, true);
    inverse_capacity_refusal::<EqAffine, 503>(false, true);
    inverse_capacity_refusal::<EpAffine, 503>(false, true);
}

fn inverse_ordinal_gaps<C>()
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
{
    const DEGREE: usize = 6;
    const MIXED: bool = false;
    const I: usize = 0;
    const Q: bool = false;
    const M: u64 = 0;
    let params = ParamsIPA::<C>::new(4);
    let pk = inverse_key::<C, DEGREE, MIXED, I>(&params);
    let instances: Vec<&[C::Scalar]> = Vec::new();
    let shared = Shared::<C>::new();
    let controls = Controls::new();
    let mut input = inverse_input!(&params, pk.clone(), &instances, &shared, &controls);
    let other = sentinel(&mut input.inner.provider, 4);
    controls.arm(None);
    let actual = input.stage_quotient_coefficients(1 << 26).unwrap();
    let events = controls.bank.lock().unwrap().events.clone();
    let expected = actual
        .pieces
        .iter()
        .map(|piece| stored_values::<C>(&shared, piece.layout))
        .collect::<Vec<_>>();
    let creates = events
        .iter()
        .enumerate()
        .filter(|(_, event)| event.kind == IoKind::Create)
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    assert_eq!(creates.len(), 13);
    drop(actual);
    drop(other);
    assert_dropped(&shared);
    let gaps =
        (0..creates.len()).flat_map(|position| {
            let exact = u64::MAX - (creates.len() - position) as u64;
            // Every position admits the exact remaining count. Before the final factory,
            // one additional gap must fail immediately, before another scalar operation.
            std::iter::once((position, exact, true))
                .chain((position + 1 < creates.len()).then_some((position, exact + 1, false)))
        });
    for (position, ordinal, accepted) in gaps {
        let shared = Shared::<C>::new();
        let controls = Controls::new();
        let mut input = inverse_input!(&params, pk.clone(), &instances, &shared, &controls);
        let mut other = sentinel(&mut input.inner.provider, 4);
        let calls = shared.log.lock().unwrap().rng_calls;
        let protocol = shared.log.lock().unwrap().events.clone();
        controls.arm(None);
        controls.bank.lock().unwrap().ordinal_jump = Some((creates[position], ordinal));
        controls.protocol_armed.store(true, Ordering::SeqCst);
        crate::plonk::prover::stored::quotient_inverse::take_clear_observations();
        let result = input.stage_quotient_coefficients(1 << 26);
        assert_eq!(result.is_ok(), accepted);
        assert!(controls.bank.lock().unwrap().ordinal_jump.is_none());
        let seen = controls.bank.lock().unwrap().events.clone();
        assert_eq!(seen[creates[position]].layout.ordinal(), ordinal);
        if let Ok(mut actual) = result {
            assert_eq!(
                actual.inner.advice.greatest_ordinal().unwrap(),
                Some(u64::MAX - 1)
            );
            assert_eq!(actual.inner.provider.inner.ordinal, u64::MAX);
            assert_eq!(actual.pieces.last().unwrap().layout.ordinal(), u64::MAX - 1);
            assert!(seen[creates[position]+1..].iter().any(|event|event.kind==IoKind::Read&&stage_role(event.layout.role())==1));
            assert_eq!(controls.bank.lock().unwrap().live.len(), 7);
            for (piece, expected) in actual.pieces.iter_mut().zip(&expected) {
                assert_eq!(&read_values(&mut piece.snapshot, piece.layout), expected);
            }
            drop(actual);
        } else {
            assert!(
                seen[creates[position] + 1..]
                    .iter()
                    .all(|event| matches!(event.kind, IoKind::DropWriter | IoKind::DropSnapshot))
            );
        }
        assert_eq!(controls.bank.lock().unwrap().live.len(), 1);
        assert_eq!(shared.log.lock().unwrap().rng_calls, calls);
        assert_eq!(shared.log.lock().unwrap().events, protocol);
        let (_, zero, _, bytes_zero) =
            crate::plonk::prover::stored::quotient_inverse::take_clear_observations();
        assert!(zero && bytes_zero);
        check_sentinel(&mut other);
        drop(other);
        assert_dropped(&shared);
    }
}
#[test]
fn both_pasta_inverse_actual_factory_gaps_refuse_early_exhaustion_and_accept_final_max_minus_one_before_alias_reads()
 {
    inverse_ordinal_gaps::<EqAffine>();
    inverse_ordinal_gaps::<EpAffine>();
}

#[path = "quotient_commitments_tests.rs"]
mod quotient_commitments;
