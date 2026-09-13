//! Independent ordinary numerator trajectory, bounded cache and consuming-failure regressions.
//!
//! Plaintext test receipts expose ownership and callback boundaries only. These tests do not
//! qualify an encrypted Core adapter, a full stored proof, device memory, or hardware execution.

use super::*;
use crate::plonk::prover::stored::quotient::{
    take_cache_observations, take_clear_observations as take_quotient_clears,
};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum IoKind {
    Create,
    Write,
    Seal,
    Read,
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
enum IoAction {
    Error,
    Panic,
    Short,
    Long,
    Encoding,
    Drift(u64),
    Change(ProductChange),
}
#[derive(Default)]
struct QuotientBank {
    active: bool,
    live: BTreeMap<u64, StoredPolynomialLayoutV1>,
    overrides: BTreeMap<u64, StoredPolynomialLayoutV1>,
    events: Vec<IoEvent>,
    fault: Option<(usize, IoAction)>,
    ordinal_jump: Option<(usize, u64)>,
    peak: usize,
    capacity: usize,
}
#[derive(Clone)]
struct QuotientControls<C: CurveAffine> {
    vanishing: VanishingControls<C>,
    bank: Arc<Mutex<QuotientBank>>,
}
impl<C: CurveAffine> QuotientControls<C> {
    fn new() -> Self {
        Self {
            vanishing: VanishingControls::new(),
            bank: Arc::new(Mutex::new(QuotientBank {
                capacity: 512,
                ..Default::default()
            })),
        }
    }
    fn arm(&self, fault: Option<(usize, IoAction)>) {
        let mut bank = self.bank.lock().unwrap();
        bank.active = true;
        bank.events.clear();
        bank.fault = fault;
        bank.peak = bank.live.len();
    }
    fn event(
        &self,
        kind: IoKind,
        layout: StoredPolynomialLayoutV1,
        chunk: u64,
    ) -> Option<IoAction> {
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
    fn layout(&self, original: StoredPolynomialLayoutV1) -> StoredPolynomialLayoutV1 {
        self.bank
            .lock()
            .unwrap()
            .overrides
            .get(&original.ordinal())
            .copied()
            .unwrap_or(original)
    }
    fn drift(&self, ordinal: u64) {
        let mut bank = self.bank.lock().unwrap();
        let layout = *bank
            .live
            .get(&ordinal)
            .expect("fault victim is still owned");
        bank.overrides.insert(ordinal, different_context(layout));
    }
    fn after(&self, action: Option<IoAction>, original: StoredPolynomialLayoutV1) {
        match action {
            Some(IoAction::Drift(victim)) => self.drift(victim),
            Some(IoAction::Change(change)) => {
                self.bank
                    .lock()
                    .unwrap()
                    .overrides
                    .insert(original.ordinal(), quotient_change(original, change));
            }
            _ => {}
        }
    }
    fn drop_receipt(&self, kind: IoKind, original: StoredPolynomialLayoutV1) {
        assert!(
            self.bank
                .lock()
                .unwrap()
                .live
                .remove(&original.ordinal())
                .is_some()
        );
        let action = self.event(kind, original, 0);
        // Faults are removed while the lock is held, but raised only after unlocking. A
        // one-shot destructor failure never poisons the monitor or panics again on unwind.
        if action == Some(IoAction::Panic) {
            panic!("injected quotient receipt destructor unwind");
        }
        self.after(action, original);
    }
}
fn quotient_change(
    layout: StoredPolynomialLayoutV1,
    change: ProductChange,
) -> StoredPolynomialLayoutV1 {
    let basis = match change {
        ProductChange::Basis if layout.role() != StoredPolynomialRoleV1::QuotientNumerator => {
            StoredPolynomialBasisV1::Coefficient
        }
        ProductChange::Basis | ProductChange::CosetBasis => match layout.basis() {
            StoredPolynomialBasisV1::CosetPart {
                extension_log,
                part,
            } => StoredPolynomialBasisV1::CosetPart {
                extension_log,
                part: part ^ 1,
            },
            _ => StoredPolynomialBasisV1::CosetPart {
                extension_log: 1,
                part: 0,
            },
        },
        _ => layout.basis(),
    };
    let role = match change {
        ProductChange::Index | ProductChange::WrongRole => {
            StoredPolynomialRoleV1::Instance { column: 79 }
        }
        _ => layout.role(),
    };
    StoredPolynomialLayoutV1::new(
        if change == ProductChange::Context {
            [41; 32]
        } else {
            [23; 32]
        },
        match change {
            ProductChange::Ordinal => 0,
            ProductChange::Exhausted => u64::MAX,
            ProductChange::Insufficient => u64::MAX - 1,
            _ => layout.ordinal(),
        },
        if change == ProductChange::Field {
            match layout.field() {
                StoredPastaFieldV1::Fp => StoredPastaFieldV1::Fq,
                StoredPastaFieldV1::Fq => StoredPastaFieldV1::Fp,
            }
        } else {
            layout.field()
        },
        basis,
        if change == ProductChange::K {
            layout.k() + 1
        } else {
            layout.k()
        },
        role,
    )
    .unwrap()
}
struct QuotientProvider<C: CurveAffine> {
    inner: VanishingProvider<C>,
    controls: QuotientControls<C>,
}
struct QuotientWriter<C: CurveAffine> {
    inner: Option<VanishingWriter<C>>,
    transferred: bool,
    original: StoredPolynomialLayoutV1,
    controls: QuotientControls<C>,
}
struct QuotientSnapshot<C: CurveAffine> {
    inner: VanishingSnapshot<C>,
    original: StoredPolynomialLayoutV1,
    controls: QuotientControls<C>,
}
impl<C: CurveAffine> StoredPolynomialProviderV1 for QuotientProvider<C> {
    type Writer = QuotientWriter<C>;
    fn create(
        &mut self,
        field: StoredPastaFieldV1,
        basis: StoredPolynomialBasisV1,
        k: u32,
        role: StoredPolynomialRoleV1,
    ) -> Result<Self::Writer, StoredPolynomialErrorV1> {
        // An unrelated backend user may advance the real global cursor between factories.
        // Change the actual provider cursor, not merely its reported metadata.
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
            self.inner.inner.inner.inner.inner.ordinal = ordinal;
        }
        let original = StoredPolynomialLayoutV1::new(
            [23; 32],
            self.inner.inner.inner.inner.inner.ordinal,
            field,
            basis,
            k,
            role,
        )?;
        let action = self.controls.event(IoKind::Create, original, 0);
        match action {
            Some(IoAction::Error) => return Err(StoredPolynomialErrorV1::Storage),
            Some(IoAction::Panic) => panic!("injected quotient create unwind"),
            _ => {}
        }
        {
            let bank = self.controls.bank.lock().unwrap();
            if bank.live.len() >= bank.capacity {
                return Err(StoredPolynomialErrorV1::Capacity);
            }
        }
        let inner = self.inner.create(field, basis, k, role)?;
        assert_eq!(inner.layout(), original);
        {
            let mut bank = self.controls.bank.lock().unwrap();
            assert!(bank.live.insert(original.ordinal(), original).is_none());
            bank.peak = bank.peak.max(bank.live.len());
        }
        self.controls.after(action, original);
        Ok(QuotientWriter {
            inner: Some(inner),
            transferred: false,
            original,
            controls: self.controls.clone(),
        })
    }
}
impl<C: CurveAffine> StoredPolynomialWriterV1 for QuotientWriter<C> {
    type Snapshot = QuotientSnapshot<C>;
    fn layout(&self) -> StoredPolynomialLayoutV1 {
        let inner = self.inner.as_ref().unwrap().layout();
        assert_eq!(inner, self.original);
        self.controls.layout(inner)
    }
    fn write_chunk(
        &mut self,
        chunk: u64,
        values: &[[u8; 32]],
    ) -> Result<(), StoredPolynomialErrorV1> {
        let action = self.controls.event(IoKind::Write, self.original, chunk);
        match action {
            Some(IoAction::Error) => return Err(StoredPolynomialErrorV1::Storage),
            Some(IoAction::Panic) => panic!("injected quotient write unwind"),
            _ => {}
        }
        self.inner.as_mut().unwrap().write_chunk(chunk, values)?;
        self.controls.after(action, self.original);
        Ok(())
    }
    fn seal(mut self) -> Result<Self::Snapshot, StoredPolynomialErrorV1> {
        let action = self.controls.event(IoKind::Seal, self.original, 0);
        match action {
            Some(IoAction::Error) => return Err(StoredPolynomialErrorV1::Storage),
            Some(IoAction::Panic) => panic!("injected quotient seal unwind"),
            _ => {}
        }
        let inner = self.inner.take().unwrap().seal()?;
        self.controls.after(action, self.original);
        self.transferred = true;
        Ok(QuotientSnapshot {
            inner,
            original: self.original,
            controls: self.controls.clone(),
        })
    }
}
impl<C: CurveAffine> Drop for QuotientWriter<C> {
    fn drop(&mut self) {
        if !self.transferred {
            self.controls
                .drop_receipt(IoKind::DropWriter, self.original);
        }
    }
}
impl<C: CurveAffine> StoredPolynomialSnapshotV1 for QuotientSnapshot<C> {
    fn layout(&self) -> StoredPolynomialLayoutV1 {
        self.controls.layout(self.inner.layout())
    }
    fn with_chunk<V>(
        &mut self,
        expected: StoredPolynomialLayoutV1,
        chunk: u64,
        consume: impl FnOnce(&[[u8; 32]]) -> Result<V, StoredPolynomialErrorV1>,
    ) -> Result<V, StoredPolynomialErrorV1> {
        let action = self.controls.event(IoKind::Read, self.original, chunk);
        match action {
            Some(IoAction::Error) => return Err(StoredPolynomialErrorV1::Storage),
            Some(IoAction::Panic) => panic!("injected quotient read unwind"),
            _ => {}
        }
        let result = self
            .inner
            .with_chunk(expected, chunk, |values| match action {
                Some(IoAction::Short) => consume(&values[..values.len() - 1]),
                Some(IoAction::Long) | Some(IoAction::Encoding) => {
                    let mut changed = values.to_vec();
                    if action == Some(IoAction::Long) {
                        changed.push([0; 32]);
                    } else {
                        *changed.last_mut().unwrap() = [255; 32];
                    }
                    consume(&changed)
                }
                _ => consume(values),
            });
        if result.is_ok() {
            self.controls.after(action, self.original);
        }
        result
    }
    fn with_column<V>(
        &mut self,
        _: StoredPolynomialLayoutV1,
        _: impl FnOnce(&[[u8; 32]]) -> Result<V, StoredPolynomialErrorV1>,
    ) -> Result<V, StoredPolynomialErrorV1> {
        panic!("quotient requested backend full-column materialization")
    }
}
impl<C: CurveAffine> Drop for QuotientSnapshot<C> {
    fn drop(&mut self) {
        self.controls
            .drop_receipt(IoKind::DropSnapshot, self.original);
    }
}
fn quotient_provider<C: CurveAffine>(
    shared: &Arc<Shared<C>>,
    controls: &QuotientControls<C>,
) -> QuotientProvider<C> {
    QuotientProvider {
        inner: vanishing_provider(shared, &controls.vanishing),
        controls: controls.clone(),
    }
}

struct QuotientCircuit<C: CurveAffine, const COPY: bool, const LOOKUPS: bool>(
    ProductCircuit<C, COPY, LOOKUPS>,
);
impl<C: CurveAffine, const COPY: bool, const LOOKUPS: bool> Circuit<C::Scalar>
    for QuotientCircuit<C, COPY, LOOKUPS>
{
    type Config = ProductConfig;
    type FloorPlanner = V1;
    #[cfg(feature = "circuit-params")]
    type Params = ();
    fn without_witnesses(&self) -> Self {
        Self(self.0.without_witnesses())
    }
    fn configure(meta: &mut ConstraintSystem<C::Scalar>) -> Self::Config {
        let config = ProductCircuit::<C, COPY, LOOKUPS>::configure(meta);
        let challenge = meta.challenge_usable_after(crate::plonk::FirstPhase);
        // Nontrivial numerator values exercise retained graph Horner order even for a circuit
        // with an unsatisfied constraint. This is evaluator parity, not full-proof validity.
        meta.create_gate("quotient mixed rotations and phase challenge", |meta| {
            let a = meta.query_advice(config.advice[0], Rotation::cur());
            let b = meta.query_advice(config.advice[1], Rotation::next());
            let c = meta.query_advice(config.advice[2], Rotation::prev());
            let fixed = meta.query_fixed(config.fixed[0], Rotation::prev());
            let instance = meta.query_instance(config.instances[3], Rotation::next());
            let challenge = meta.query_challenge(challenge);
            vec![
                a.clone() * b - c + challenge.clone() * instance,
                a.clone() * a - fixed + challenge,
            ]
        });
        meta.create_gate("quotient second gate fold", |meta| {
            vec![
                -meta.query_advice(config.advice[2], Rotation::cur())
                    + meta.query_instance(config.instances[0], Rotation::prev()),
            ]
        });
        if LOOKUPS {
            meta.lookup_any("quotient mixed tuple membership", |meta| {
                let a = meta.query_advice(config.advice[0], Rotation::cur());
                let b = meta.query_fixed(config.fixed[1], Rotation::next())
                    + meta.query_instance(config.instances[3], Rotation::next());
                vec![(a.clone(), a), (b.clone(), b)]
            });
        }
        config
    }
    fn synthesize_for_measurement(
        &self,
        config: Self::Config,
        layouter: impl Layouter<C::Scalar>,
    ) -> Result<(), Error> {
        self.0.run(config, layouter, true)
    }
    fn synthesize(
        &self,
        config: Self::Config,
        layouter: impl Layouter<C::Scalar>,
    ) -> Result<(), Error> {
        self.0.run(config, layouter, false)
    }
}
fn quotient_key<C, const COPY: bool, const LOOKUPS: bool>(params: &ParamsIPA<C>) -> ProvingKey<C>
where
    C: CurveAffine,
    C::Scalar: FromUniformBytes<64>,
{
    let shared = Shared::<C>::new();
    let circuit = QuotientCircuit(ProductCircuit::<C, COPY, LOOKUPS>(Producer::new(
        &shared, 3,
    )));
    let vk = keygen_vk_custom(params, &circuit, true).unwrap();
    keygen_pk(params, vk, &circuit).unwrap()
}
macro_rules! quotient_member {
    ($params:expr,$pk:expr,$instances:expr,$shared:expr,$controls:expr,$copy:tt,$lookups:tt,$q:ident,$mask:ident) => {
        prepare_single_phase_stored_ipa_prefix_v1::<C, _, _, _, ProductChallenge<C>, _, $q, $mask>(
            $params,
            $pk,
            QuotientCircuit(ProductCircuit::<C, $copy, $lookups>(Producer::new(
                $shared, 3,
            ))),
            $instances,
            quotient_provider($shared, $controls),
            ProductRng {
                inner: BoundaryRng {
                    inner: Rng(Arc::clone($shared)),
                    controls: $controls.vanishing.product.base.clone(),
                },
                controls: $controls.vanishing.product.clone(),
            },
            ProductTranscript::new($shared, &$controls.vanishing.product),
        )
        .unwrap()
        .stage_advice_coefficients()
        .unwrap()
        .compress_lookups(1 << 20)
        .unwrap()
        .sort_lookup_values(1 << 20)
        .unwrap()
        .prepare_lookup_membership(1 << 20)
        .unwrap()
    };
}
macro_rules! quotient_vanishing {
    ($params:expr,$pk:expr,$instances:expr,$shared:expr,$controls:expr) => {
        quotient_member!(
            $params, $pk, $instances, $shared, $controls, true, true, Q, M
        )
        .commit_permuted_lookups(1 << 20)
        .unwrap()
        .commit_products(1 << 20)
        .unwrap()
        .commit_vanishing_and_stage_coefficients(1 << 20)
        .unwrap()
    };
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
fn ordinary_quotient<C, const COPY: bool, const LOOKUPS: bool, const Q: bool, const M: u64>(
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
    let pk = quotient_key::<C, COPY, LOOKUPS>(&params);
    let n = 1_usize << k;
    let usable = n - (pk.vk.cs.blinding_factors() + 1);
    let values = if empty {
        std::array::from_fn(|_| Vec::new())
    } else {
        fixture_values::<C::Scalar>(usable, false)
    };
    let instances = values.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let shared = Shared::<C>::new();
    let controls = QuotientControls::new();
    let member = quotient_member!(
        &params, pk, &instances, &shared, &controls, COPY, LOOKUPS, Q, M
    );
    let advice = member
        .compressed
        .inner
        .advice
        .layouts()
        .unwrap()
        .map(|layout| {
            member
                .compressed
                .inner
                .pk
                .vk
                .domain
                .lagrange_from_vec(sealed_values(&shared, layout))
        })
        .collect::<Vec<_>>();
    let challenges = member
        .compressed
        .inner
        .advice
        .challenges()
        .unwrap()
        .collect::<Vec<_>>();
    assert_eq!(challenges.len(), 1);
    let dense_instances = values
        .iter()
        .map(|prefix| {
            let mut padded = prefix.clone();
            padded.resize(n, C::Scalar::ZERO);
            member
                .compressed
                .inner
                .pk
                .vk
                .domain
                .lagrange_from_vec(padded)
        })
        .collect::<Vec<_>>();
    let oracle_shared = Shared::<C>::new();
    let oracle_controls = ProductControls::new();
    let mut oracle_transcript = ProductTranscript::new(&oracle_shared, &oracle_controls);
    replay_product_prefix(&shared.log.lock().unwrap().events, &mut oracle_transcript);
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
    assert!(ordinary.numerator.iter().any(|v| *v != C::Scalar::ZERO));
    let vanishing = member
        .commit_permuted_lookups(1 << 20)
        .unwrap()
        .commit_products(1 << 20)
        .unwrap()
        .commit_vanishing_and_stage_coefficients(1 << 20)
        .unwrap();
    let public = public_values(&vanishing.pk);
    let fixed_bank = vanishing.pk.fixed_polys.as_ptr();
    let sigma_bank = vanishing.pk.permutation.polys.as_ptr();
    let evaluator = format!("{:?}", vanishing.pk.ev);
    let vk = vanishing
        .pk
        .get_vk()
        .to_bytes(crate::SerdeFormat::Processed);
    let original_advice = vanishing.advice.layouts().unwrap().collect::<Vec<_>>();
    let originals = controls.bank.lock().unwrap().live.clone();
    let old_end = vanishing.advice.greatest_ordinal().unwrap().unwrap();
    let m = 1_usize << (vanishing.pk.vk.domain.extended_k() - k);
    let before_rng = shared.log.lock().unwrap().rng_calls;
    let before_events = shared.log.lock().unwrap().events.clone();
    controls.arm(None);
    take_quotient_clears();
    take_cache_observations();
    let mut actual = vanishing.evaluate_quotient_numerator(1 << 24).unwrap();
    assert_eq!(shared.log.lock().unwrap().rng_calls, before_rng);
    assert_eq!(shared.log.lock().unwrap().events, before_events);
    assert_eq!(actual.parts.len(), m);
    assert_eq!(
        public_values(&actual.inner.pk),
        public,
        "public coefficients and their original allocations must be restored"
    );
    assert_eq!(actual.inner.pk.fixed_polys.as_ptr(), fixed_bank);
    assert_eq!(actual.inner.pk.permutation.polys.as_ptr(), sigma_bank);
    assert_eq!(format!("{:?}", actual.inner.pk.ev), evaluator);
    assert_eq!(
        actual
            .inner
            .pk
            .get_vk()
            .to_bytes(crate::SerdeFormat::Processed),
        vk
    );
    assert_eq!(
        actual.inner.advice.layouts().unwrap().collect::<Vec<_>>(),
        original_advice
    );
    assert_eq!(
        actual
            .inner
            .advice
            .challenges()
            .unwrap()
            .collect::<Vec<_>>(),
        challenges
    );
    assert_eq!(actual.inner.instances.as_ptr(), instances.as_ptr());
    assert!(std::ptr::eq(actual.inner.params, &params));
    assert_eq!(
        (*actual.inner.beta, *actual.inner.gamma, *actual.inner.y),
        (
            ordinary.products.beta,
            ordinary.products.gamma,
            ordinary.vanishing.y
        )
    );
    let events = controls.bank.lock().unwrap().events.clone();
    let bank = controls.bank.lock().unwrap();
    assert_eq!(bank.live.len(), originals.len() + m);
    assert!(bank.peak <= 512);
    for (ordinal, layout) in &originals {
        assert_eq!(bank.live.get(ordinal), Some(layout));
    }
    assert!(
        events
            .iter()
            .filter(|e| e.kind == IoKind::DropSnapshot)
            .all(|e| !originals.contains_key(&e.layout.ordinal()))
    );
    drop(bank);
    let (hits, misses, evictions, peak, opportunities) = take_cache_observations();
    assert!(hits > 0 && misses > 0 && evictions > 0 && peak > 0);
    assert!(opportunities >= hits + misses);
    let mut prior = old_end;
    for (part, polynomial) in actual.parts.iter_mut().enumerate() {
        assert_eq!(
            polynomial.layout.role(),
            StoredPolynomialRoleV1::QuotientNumerator
        );
        assert_eq!(
            polynomial.layout.basis(),
            StoredPolynomialBasisV1::CosetPart {
                extension_log: (m as u32).ilog2(),
                part: part as u32
            }
        );
        assert!(polynomial.layout.ordinal() > prior);
        prior = polynomial.layout.ordinal();
        let expected = (0..n)
            .map(|j| ordinary.numerator[j * m + part])
            .collect::<Vec<_>>();
        assert_eq!(
            read_compressed::<C, _>(&mut polynomial.snapshot, polynomial.layout),
            expected
        );
    }
    assert!(actual.inner.advice.greatest_ordinal().unwrap().unwrap() >= prior);
    for (layout, expected) in original_advice.iter().zip(&ordinary.advice_coefficient) {
        assert_eq!(sealed_values(&shared, *layout), *expected);
    }
    for (value, expected) in actual
        .inner
        .instance_coefficients
        .iter()
        .zip(&ordinary.instance_coefficient)
    {
        assert_eq!(sealed_values(&shared, value.layout), *expected);
    }
    for (value, expected) in actual
        .inner
        .permutations
        .iter()
        .zip(&ordinary.products.permutations)
    {
        assert_eq!(
            sealed_values(&shared, value.coefficient.layout),
            expected.coefficient
        );
        assert_eq!((value.blind.0).0, expected.blind.0);
        assert_eq!(
            value.commitment,
            params
                .commit(
                    &actual
                        .inner
                        .pk
                        .vk
                        .domain
                        .coeff_from_vec(expected.coefficient.clone()),
                    expected.blind
                )
                .to_affine()
        );
    }
    for (lookup, expected) in actual.inner.lookups.iter().zip(&ordinary.products.lookups) {
        for (value, coefficient, blind) in [
            (
                &lookup.input,
                &expected.input_coefficient,
                expected.input_blind,
            ),
            (
                &lookup.table,
                &expected.table_coefficient,
                expected.table_blind,
            ),
            (
                &lookup.product,
                &expected.product_coefficient,
                expected.product_blind,
            ),
        ] {
            assert_eq!(
                sealed_values(&shared, value.coefficient.layout),
                *coefficient
            );
            assert_eq!((value.blind.0).0, blind.0);
            assert_eq!(
                value.commitment,
                params
                    .commit(
                        &actual
                            .inner
                            .pk
                            .vk
                            .domain
                            .coeff_from_vec(coefficient.clone()),
                        blind
                    )
                    .to_affine()
            );
        }
    }
    assert_eq!(
        sealed_values(&shared, actual.inner.random.coefficient.layout),
        ordinary.vanishing.random_coefficient
    );
    assert_eq!(
        (actual.inner.random.blind.0).0,
        ordinary.vanishing.random_blind.0
    );
    assert_eq!(
        actual.inner.random.commitment,
        params
            .commit(
                &actual
                    .inner
                    .pk
                    .vk
                    .domain
                    .coeff_from_vec(ordinary.vanishing.random_coefficient),
                ordinary.vanishing.random_blind
            )
            .to_affine()
    );
    assert_eq!(
        shared.log.lock().unwrap().events,
        oracle_shared.log.lock().unwrap().events
    );
    assert_eq!(
        actual.inner.transcript.inner.inner.inner.clone().finalize(),
        oracle_transcript.inner.inner.inner.clone().finalize()
    );
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
    let (fields, zero, bytes, bytes_zero, blinds, blinds_zero) = take_quotient_clears();
    assert!(fields >= n && bytes >= 256 && blinds >= 1 && zero && bytes_zero && blinds_zero);
}
fn quotient_matrix<C>()
where
    C: CurveAffine + crate::SerdeCurveAffine,
    C::Scalar: StoredAssignmentFieldV1
        + WithSmallOrderMulGroup<3>
        + FromUniformBytes<64>
        + crate::SerdePrimeField,
{
    for k in [4, 8, 9] {
        ordinary_quotient::<C, true, true, false, 0>(k, false);
        ordinary_quotient::<C, true, true, true, 0>(k, false);
        ordinary_quotient::<C, true, true, true, 6>(k, false);
    }
    ordinary_quotient::<C, false, false, false, 0>(4, true);
    ordinary_quotient::<C, false, true, true, 0>(4, true);
    ordinary_quotient::<C, true, false, true, 6>(9, false);
}
#[test]
fn both_pasta_quotient_parts_match_actual_whole_ordinary_trajectory_restore_public_allocations_and_preserve_all_owners()
 {
    quotient_matrix::<EqAffine>();
    quotient_matrix::<EpAffine>();
}

fn quotient_sentinel<C>(
    provider: &mut QuotientProvider<C>,
    k: u32,
    column: u32,
) -> QuotientSnapshot<C>
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1,
{
    let mut writer = provider
        .create(
            C::Scalar::STORED_FIELD,
            StoredPolynomialBasisV1::Coefficient,
            k,
            StoredPolynomialRoleV1::Instance { column },
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
fn inspect_quotient_sentinel<C>(snapshot: &mut QuotientSnapshot<C>)
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1,
{
    let layout = snapshot.layout();
    snapshot
        .with_chunk(layout, 0, |values| {
            assert!(values.iter().all(|v| *v == C::Scalar::from(61).to_repr()));
            Ok(())
        })
        .unwrap();
}
fn role_class(role: StoredPolynomialRoleV1) -> u8 {
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
        StoredPolynomialRoleV1::QuotientNumerator => 6,
        _ => panic!("unexpected quotient scalar I/O role"),
    }
}
fn boundary_cases(events: &[IoEvent], victim: u64) -> Vec<(usize, IoAction)> {
    // Derive representatives from the real successful execution, not from a second cache or
    // quotient implementation. Each source role gets its first coefficient and coset read;
    // output writers get create/write/seal/drop checks. Late output cases are added below.
    let mut selected = BTreeMap::new();
    for (index, event) in events.iter().enumerate() {
        if event.kind == IoKind::DropWriter {
            continue;
        }
        let basis = u8::from(matches!(
            event.layout.basis(),
            StoredPolynomialBasisV1::CosetPart { .. }
        ));
        selected
            .entry((event.kind as u8, role_class(event.layout.role()), basis))
            .or_insert(index);
    }
    assert_eq!(
        selected.len(),
        39,
        "six source classes and the output writer are covered"
    );
    let mut cases = Vec::new();
    for index in selected.into_values() {
        match events[index].kind {
            IoKind::Read => {
                for action in [
                    IoAction::Error,
                    IoAction::Panic,
                    IoAction::Short,
                    IoAction::Long,
                    IoAction::Encoding,
                    IoAction::Drift(victim),
                ] {
                    cases.push((index, action));
                }
            }
            IoKind::DropSnapshot => {
                for action in [IoAction::Panic, IoAction::Drift(victim)] {
                    cases.push((index, action));
                }
            }
            IoKind::Create | IoKind::Write | IoKind::Seal => {
                for action in [IoAction::Error, IoAction::Panic, IoAction::Drift(victim)] {
                    cases.push((index, action));
                }
            }
            IoKind::DropWriter => unreachable!(),
        }
    }
    for role in 0..=6 {
        if let Some((index, _)) = events.iter().enumerate().rfind(|(_, event)| {
            event.kind == IoKind::Read && role_class(event.layout.role()) == role
        }) {
            cases.push((index, IoAction::Encoding));
        }
    }
    for kind in [IoKind::Write, IoKind::Seal] {
        let index = events
            .iter()
            .rposition(|event| {
                event.kind == kind
                    && event.layout.role() == StoredPolynomialRoleV1::QuotientNumerator
            })
            .unwrap();
        cases.push((index, IoAction::Error));
        cases.push((index, IoAction::Drift(victim)));
    }
    assert_eq!(
        cases.len(),
        157,
        "all prepared representative boundary failures remain selected"
    );
    cases
}
fn quotient_io_failures<C>()
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
{
    const Q: bool = false;
    const M: u64 = 0;
    let params = ParamsIPA::<C>::new(9);
    let pk = quotient_key::<C, true, true>(&params);
    let values = fixture_values::<C::Scalar>(512 - (pk.vk.cs.blinding_factors() + 1), false);
    let instances = values.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let shared = Shared::<C>::new();
    let controls = QuotientControls::new();
    let mut owner = quotient_vanishing!(&params, pk, &instances, &shared, &controls);
    let sentinel = quotient_sentinel(&mut owner.provider, 9, 77);
    let victim = owner.random.coefficient.layout.ordinal();
    controls.arm(None);
    let actual = owner.evaluate_quotient_numerator(1 << 24).unwrap();
    let events = controls.bank.lock().unwrap().events.clone();
    // All six private source kinds occur, and numerator outputs are never read by this stage.
    for role in 0..6 {
        for coefficient in [false, true] {
            assert!(events.iter().any(|event| event.kind == IoKind::Read
                && role_class(event.layout.role()) == role
                && (event.layout.basis() == StoredPolynomialBasisV1::Coefficient) == coefficient));
        }
    }
    assert!(!events.iter().any(|event| event.kind == IoKind::Read
        && event.layout.role() == StoredPolynomialRoleV1::QuotientNumerator));
    let cases = boundary_cases(&events, victim);
    drop(actual);
    drop(sentinel);
    assert_dropped(&shared);
    for (target, action) in cases {
        let shared = Shared::<C>::new();
        let controls = QuotientControls::new();
        let pk = quotient_key::<C, true, true>(&params);
        let mut owner = quotient_vanishing!(&params, pk, &instances, &shared, &controls);
        let mut sentinel = quotient_sentinel(&mut owner.provider, 9, 77);
        assert_eq!(owner.random.coefficient.layout.ordinal(), victim);
        let before_rng = shared.log.lock().unwrap().rng_calls;
        let before_events = shared.log.lock().unwrap().events.clone();
        take_quotient_clears();
        controls.arm(Some((target, action)));
        let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
            owner.evaluate_quotient_numerator(1 << 24)
        }));
        assert!(
            matches!(&result, Err(_) | Ok(Err(_))),
            "boundary {target} {:?} admitted failure {action:?}",
            events[target]
        );
        assert!(
            controls.bank.lock().unwrap().fault.is_none(),
            "selected callback was not reached"
        );
        assert_eq!(controls.bank.lock().unwrap().events[target], events[target]);
        assert_eq!(shared.log.lock().unwrap().rng_calls, before_rng);
        assert_eq!(shared.log.lock().unwrap().events, before_events);
        assert_eq!(controls.bank.lock().unwrap().live.len(), 1);
        assert!(
            !controls
                .vanishing
                .product
                .base
                .window
                .load(Ordering::SeqCst)
        );
        let (fields, zero, bytes, bytes_zero, blinds, blinds_zero) = take_quotient_clears();
        assert!(zero && bytes_zero && blinds_zero && blinds >= 1);
        // Allocation preflight precedes the first backend event, even a failed writer creation.
        assert!(fields >= 512 && bytes >= 256);
        inspect_quotient_sentinel(&mut sentinel);
        drop(result);
        drop(sentinel);
        assert_dropped(&shared);
        assert!(controls.bank.lock().unwrap().live.is_empty());
    }
}
#[test]
fn both_pasta_quotient_all_storage_source_kinds_and_late_output_errors_unwinds_destroy_owner_preserving_unrelated_bank()
 {
    quotient_io_failures::<EqAffine>();
    quotient_io_failures::<EpAffine>();
}

fn quotient_writer_metadata<C>()
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
{
    const Q: bool = true;
    const M: u64 = 6;
    let params = ParamsIPA::<C>::new(4);
    let pk = quotient_key::<C, true, true>(&params);
    let values = fixture_values::<C::Scalar>(16 - (pk.vk.cs.blinding_factors() + 1), false);
    let instances = values.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let shared = Shared::<C>::new();
    let controls = QuotientControls::new();
    let mut owner = quotient_vanishing!(&params, pk, &instances, &shared, &controls);
    let sentinel = quotient_sentinel(&mut owner.provider, 4, 77);
    controls.arm(None);
    let actual = owner.evaluate_quotient_numerator(1 << 24).unwrap();
    let events = controls.bank.lock().unwrap().events.clone();
    let mut targets = BTreeMap::new();
    for (index, event) in events.iter().enumerate() {
        if matches!(event.kind, IoKind::Create | IoKind::Write | IoKind::Seal) {
            targets
                .entry((event.kind as u8, role_class(event.layout.role())))
                .or_insert(index);
        }
    }
    assert_eq!(
        targets.len(),
        21,
        "create/write/seal for six source roles and numerator"
    );
    drop(actual);
    drop(sentinel);
    assert_dropped(&shared);
    for target in targets.into_values() {
        for change in [
            ProductChange::Field,
            ProductChange::K,
            ProductChange::Basis,
            ProductChange::Context,
            ProductChange::Ordinal,
            ProductChange::Exhausted,
            ProductChange::Insufficient,
            ProductChange::Index,
            ProductChange::WrongRole,
        ] {
            let shared = Shared::<C>::new();
            let controls = QuotientControls::new();
            let pk = quotient_key::<C, true, true>(&params);
            let mut owner = quotient_vanishing!(&params, pk, &instances, &shared, &controls);
            let mut sentinel = quotient_sentinel(&mut owner.provider, 4, 77);
            controls.arm(Some((target, IoAction::Change(change))));
            take_quotient_clears();
            let result = owner.evaluate_quotient_numerator(1 << 24);
            assert!(
                result.is_err(),
                "accepted {change:?} after {:?}",
                events[target]
            );
            assert!(controls.bank.lock().unwrap().fault.is_none());
            assert_eq!(controls.bank.lock().unwrap().live.len(), 1);
            let (_, zero, _, bytes_zero, _, blinds_zero) = take_quotient_clears();
            assert!(zero && bytes_zero && blinds_zero);
            inspect_quotient_sentinel(&mut sentinel);
            drop(result);
            drop(sentinel);
            assert_dropped(&shared);
        }
    }
}
#[test]
fn both_pasta_quotient_every_source_and_output_writer_rejects_metadata_substitution_before_reusable_success()
 {
    quotient_writer_metadata::<EqAffine>();
    quotient_writer_metadata::<EpAffine>();
}

fn quotient_no_protocol_events<C>()
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
{
    const Q: bool = true;
    const M: u64 = 0;
    let params = ParamsIPA::<C>::new(4);
    for rng in [false, true] {
        let pk = quotient_key::<C, true, true>(&params);
        let values = fixture_values::<C::Scalar>(16 - (pk.vk.cs.blinding_factors() + 1), false);
        let instances = values.iter().map(Vec::as_slice).collect::<Vec<_>>();
        let shared = Shared::<C>::new();
        let controls = QuotientControls::new();
        let owner = quotient_vanishing!(&params, pk, &instances, &shared, &controls);
        let (calls, events) = {
            let log = shared.log.lock().unwrap();
            (log.rng_calls, log.events.clone())
        };
        let fault = if rng {
            BoundaryFault::Rng(calls)
        } else {
            BoundaryFault::Transcript(events.len())
        };
        *controls.vanishing.product.base.boundary.lock().unwrap() = fault;
        controls.arm(None);
        let actual = owner.evaluate_quotient_numerator(1 << 24).unwrap();
        assert_eq!(shared.log.lock().unwrap().rng_calls, calls);
        assert_eq!(shared.log.lock().unwrap().events, events);
        assert_eq!(
            *controls.vanishing.product.base.boundary.lock().unwrap(),
            fault,
            "numerator must never touch armed protocol boundary"
        );
        *controls.vanishing.product.base.boundary.lock().unwrap() = BoundaryFault::None;
        drop(actual);
        assert_dropped(&shared);
    }
}
#[test]
fn both_pasta_quotient_no_rng_or_transcript_boundary_is_called() {
    quotient_no_protocol_events::<EqAffine>();
    quotient_no_protocol_events::<EpAffine>();
}

#[derive(Clone)]
struct PressureConfig {
    advice: Vec<Column<Advice>>,
    instances: Vec<Column<Instance>>,
    fixed: Column<Fixed>,
}
struct PressureCircuit<
    C: CurveAffine,
    const A: usize,
    const I: usize,
    const L: bool,
    const QUERY: bool,
>(Producer<C>);
impl<C: CurveAffine, const A: usize, const I: usize, const L: bool, const QUERY: bool>
    PressureCircuit<C, A, I, L, QUERY>
{
    fn run(
        &self,
        config: PressureConfig,
        mut layouter: impl Layouter<C::Scalar>,
        measurement: bool,
    ) -> Result<(), Error> {
        if measurement {
            self.0.shared.log.lock().unwrap().measurement_passes += 1;
        } else {
            self.0.shared.log.lock().unwrap().synthesis_passes += 1;
        }
        layouter.assign_region(
            || "reachable bounded-cache pressure",
            |mut region| {
                for (column, advice) in config.advice.iter().enumerate() {
                    for row in 0..=self.0.last_row {
                        region.assign_advice_discarding_value(
                            *advice,
                            row,
                            Value::known(if row == 0 {
                                C::Scalar::ZERO
                            } else {
                                C::Scalar::from((column + row + 1) as u64)
                            }),
                        );
                    }
                }
                region.assign_fixed(config.fixed, 0, C::Scalar::from(7));
                Ok(())
            },
        )
    }
}
impl<C: CurveAffine, const A: usize, const I: usize, const L: bool, const QUERY: bool>
    Circuit<C::Scalar> for PressureCircuit<C, A, I, L, QUERY>
{
    type Config = PressureConfig;
    type FloorPlanner = V1;
    #[cfg(feature = "circuit-params")]
    type Params = ();
    fn without_witnesses(&self) -> Self {
        Self(self.0.without_witnesses())
    }
    fn configure(meta: &mut ConstraintSystem<C::Scalar>) -> Self::Config {
        let config = PressureConfig {
            advice: (0..A).map(|_| meta.advice_column()).collect(),
            instances: (0..I).map(|_| meta.instance_column()).collect(),
            fixed: meta.fixed_column(),
        };
        meta.set_minimum_degree(6);
        if L {
            assert!(A > 0 && I > 0);
            meta.enable_equality(config.advice[0]);
            meta.lookup_any("pressure identity instance lookup", |meta| {
                let instance = meta.query_instance(config.instances[0], Rotation::cur());
                vec![(instance.clone(), instance)]
            });
        }
        meta.create_gate("every pressure column queried", |meta| {
            let mut sum = meta.query_fixed(config.fixed, Rotation::cur());
            if QUERY {
                for advice in &config.advice {
                    sum = sum + meta.query_advice(*advice, Rotation::cur());
                }
                for instance in &config.instances {
                    sum = sum + meta.query_instance(*instance, Rotation::cur());
                }
            }
            vec![sum]
        });
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
fn pressure_key<C, const A: usize, const I: usize, const L: bool, const QUERY: bool>(
    params: &ParamsIPA<C>,
) -> ProvingKey<C>
where
    C: CurveAffine,
    C::Scalar: FromUniformBytes<64>,
{
    let shared = Shared::<C>::new();
    let circuit = PressureCircuit::<C, A, I, L, QUERY>(Producer::new(&shared, 3));
    let vk = keygen_vk_custom(params, &circuit, true).unwrap();
    keygen_pk(params, vk, &circuit).unwrap()
}
macro_rules! pressure_member {
    ($params:expr,$pk:expr,$instances:expr,$shared:expr,$controls:expr) => {
        prepare_single_phase_stored_ipa_prefix_v1::<C, _, _, _, ProductChallenge<C>, _, Q, 0>(
            $params,
            $pk,
            PressureCircuit::<C, A, I, L, QUERY>(Producer::new($shared, 3)),
            $instances,
            quotient_provider($shared, $controls),
            ProductRng {
                inner: BoundaryRng {
                    inner: Rng(Arc::clone($shared)),
                    controls: $controls.vanishing.product.base.clone(),
                },
                controls: $controls.vanishing.product.clone(),
            },
            ProductTranscript::new($shared, &$controls.vanishing.product),
        )
        .unwrap()
        .stage_advice_coefficients()
        .unwrap()
        .compress_lookups(1 << 20)
        .unwrap()
        .sort_lookup_values(1 << 20)
        .unwrap()
        .prepare_lookup_membership(1 << 20)
        .unwrap()
    };
}
fn pressure_trajectory<
    C,
    const A: usize,
    const I: usize,
    const L: bool,
    const Q: bool,
    const QUERY: bool,
>()
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
{
    let params = ParamsIPA::<C>::new(4);
    let pk = pressure_key::<C, A, I, L, QUERY>(&params);
    assert_eq!(pk.vk.cs.degree(), 6);
    assert_eq!(pk.vk.domain.extended_k(), 7);
    assert_eq!(pk.vk.cs.num_advice_columns, A);
    assert_eq!(pk.vk.cs.num_instance_columns, I);
    assert_eq!(pk.vk.cs.lookups.len(), usize::from(L));
    assert_eq!(pk.vk.cs.permutation.columns.len(), usize::from(L));
    let values = (0..I)
        .map(|column| vec![C::Scalar::ZERO, C::Scalar::from((column + 1) as u64)])
        .collect::<Vec<_>>();
    let instances = values.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let shared = Shared::<C>::new();
    let controls = QuotientControls::new();
    let member = pressure_member!(&params, pk, &instances, &shared, &controls);
    let advice = member
        .compressed
        .inner
        .advice
        .layouts()
        .unwrap()
        .map(|layout| {
            member
                .compressed
                .inner
                .pk
                .vk
                .domain
                .lagrange_from_vec(sealed_values(&shared, layout))
        })
        .collect::<Vec<_>>();
    let dense = values
        .iter()
        .map(|prefix| {
            let mut padded = prefix.clone();
            padded.resize(16, C::Scalar::ZERO);
            member
                .compressed
                .inner
                .pk
                .vk
                .domain
                .lagrange_from_vec(padded)
        })
        .collect::<Vec<_>>();
    let oracle_shared = Shared::<C>::new();
    let oracle_controls = ProductControls::new();
    let mut transcript = ProductTranscript::new(&oracle_shared, &oracle_controls);
    replay_product_prefix(&shared.log.lock().unwrap().events, &mut transcript);
    let mut rng = shared.rng.lock().unwrap().clone();
    let ordinary = crate::plonk::lookup::prover::stored_quotient_ordinary_oracle(
        &member.compressed.inner.pk,
        &params,
        member.compressed.theta,
        &advice,
        &dense,
        &[],
        &mut rng,
        &mut transcript,
    )
    .unwrap();
    assert_eq!(ordinary.numerator, ordinary.borrowed_numerator);
    assert!(
        ordinary
            .numerator
            .iter()
            .any(|value| *value != C::Scalar::ZERO)
    );
    let owner = member
        .commit_permuted_lookups(1 << 20)
        .unwrap()
        .commit_products(1 << 20)
        .unwrap()
        .commit_vanishing_and_stage_coefficients(1 << 20)
        .unwrap();
    let d = A + I + 4 * usize::from(L);
    let inherited = d + 1;
    assert_eq!(controls.bank.lock().unwrap().live.len(), inherited);
    let originals = controls.bank.lock().unwrap().live.clone();
    let public = public_values(&owner.pk);
    let before = shared.log.lock().unwrap().events.clone();
    let calls = shared.log.lock().unwrap().rng_calls;
    controls.arm(None);
    take_cache_observations();
    take_quotient_clears();
    let mut actual = owner.evaluate_quotient_numerator(1 << 24).unwrap();
    assert_eq!(shared.log.lock().unwrap().events, before);
    assert_eq!(shared.log.lock().unwrap().rng_calls, calls);
    assert_eq!(public_values(&actual.inner.pk), public);
    assert_eq!(actual.parts.len(), 8);
    let (hits, misses, evictions, peak, opportunities) = take_cache_observations();
    let events = controls.bank.lock().unwrap().events.clone();
    let mut cache = BTreeMap::new();
    let mut part_peaks = [0_usize; 8];
    let mut part_creates = [0_usize; 8];
    for event in &events {
        if originals.contains_key(&event.layout.ordinal()) {
            continue;
        }
        if let StoredPolynomialBasisV1::CosetPart {
            extension_log,
            part,
        } = event.layout.basis()
        {
            assert_eq!(extension_log, 3);
            if event.layout.role() != StoredPolynomialRoleV1::QuotientNumerator {
                if event.kind == IoKind::Create {
                    assert!(
                        cache.values().all(|p| *p == part),
                        "old part cache survived into next part"
                    );
                    assert!(cache.insert(event.layout.ordinal(), part).is_none());
                    part_peaks[part as usize] = part_peaks[part as usize].max(cache.len());
                    part_creates[part as usize] += 1;
                    assert!(cache.len() <= d.min(512 - inherited - part as usize - 1));
                } else if event.kind == IoKind::DropSnapshot {
                    assert_eq!(cache.remove(&event.layout.ordinal()), Some(part));
                }
            }
        }
    }
    assert!(cache.is_empty());
    assert_eq!(part_creates.iter().sum::<usize>(), misses);
    assert_eq!(
        evictions, misses,
        "all temporary cache receipts must retire before returning"
    );
    assert_eq!(part_peaks.iter().max().copied().unwrap(), peak);
    assert_eq!(controls.bank.lock().unwrap().live.len(), inherited + 8);
    assert!(controls.bank.lock().unwrap().peak <= 512);
    if A == 0 && I == 502 {
        assert_eq!(part_peaks, [8, 7, 6, 5, 4, 3, 2, 1]);
        assert_eq!(part_creates, [502; 8]);
        assert_eq!(misses, 4016);
        assert_eq!(hits, 0);
        assert!(opportunities >= misses);
        assert_eq!(controls.bank.lock().unwrap().peak, 512);
    } else if A == 250 {
        assert_eq!(inherited, 256);
        assert!(
            part_peaks[7] <= 248 && part_creates[7] > part_peaks[7],
            "late shrinking capacity never forced a reload"
        );
        assert!(hits > 0 && misses > peak);
    } else {
        assert!(d == 0 || !QUERY);
        assert_eq!((hits, misses, evictions, peak), (0, 0, 0, 0));
        assert_eq!(part_creates, [0; 8]);
        if I == 503 {
            assert_eq!(inherited, 504);
            assert_eq!(controls.bank.lock().unwrap().peak, 512);
            assert_eq!(
                opportunities, 8,
                "unused instance columns need no coset writer"
            );
        }
    }
    for (ordinal, layout) in &originals {
        assert_eq!(
            controls.bank.lock().unwrap().live.get(ordinal),
            Some(layout)
        );
    }
    assert_eq!(actual.inner.instance_coefficients.len(), I);
    for (part, polynomial) in actual.parts.iter_mut().enumerate() {
        let expected = (0..16)
            .map(|j| ordinary.numerator[j * 8 + part])
            .collect::<Vec<_>>();
        assert_eq!(
            read_compressed::<C, _>(&mut polynomial.snapshot, polynomial.layout),
            expected
        );
    }
    assert_eq!(
        shared.log.lock().unwrap().events,
        oracle_shared.log.lock().unwrap().events
    );
    assert_eq!(
        actual.inner.transcript.inner.inner.inner.clone().finalize(),
        transcript.inner.inner.inner.clone().finalize()
    );
    let mut next_a = [0; 64];
    let mut next_b = [0; 64];
    actual.inner.rng.fill_bytes(&mut next_a);
    rng.fill_bytes(&mut next_b);
    assert_eq!(next_a, next_b);
    drop(actual);
    assert_dropped(&shared);
    assert!(controls.bank.lock().unwrap().live.is_empty());
    let (_, zero, _, bytes_zero, _, blinds_zero) = take_quotient_clears();
    assert!(zero && bytes_zero && blinds_zero);
}
#[test]
fn both_pasta_quotient_reachable_cache_pressure_shrinks_to_one_slot_and_matches_actual_ordinary_numerator()
 {
    pressure_trajectory::<EqAffine, 0, 502, false, false, true>();
    pressure_trajectory::<EpAffine, 0, 502, false, false, true>();
}
#[test]
fn both_pasta_quotient_mixed_advice_lookup_pressure_and_empty_source_bank_match_actual_ordinary() {
    pressure_trajectory::<EqAffine, 250, 1, true, false, true>();
    pressure_trajectory::<EpAffine, 250, 1, true, true, true>();
    pressure_trajectory::<EqAffine, 0, 0, false, false, true>();
    pressure_trajectory::<EpAffine, 0, 0, false, true, true>();
}

#[test]
fn both_pasta_quotient_populated_unqueried_instance_bank_needs_only_final_output_handles() {
    pressure_trajectory::<EqAffine, 0, 503, false, false, false>();
    pressure_trajectory::<EpAffine, 0, 503, false, false, false>();
}

fn quotient_preflight<C>()
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
{
    const Q: bool = false;
    const M: u64 = 0;
    let params = ParamsIPA::<C>::new(4);
    let alternate = ParamsIPA::<C>::new(4);
    let wrong_k = ParamsIPA::<C>::new(5);
    let pk = quotient_key::<C, true, true>(&params);
    let values = fixture_values::<C::Scalar>(16 - (pk.vk.cs.blinding_factors() + 1), false);
    let instances = values.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let too_long = vec![C::Scalar::ZERO; 17];
    let bad_instances = vec![too_long.as_slice(); 4];
    for case in 0..35 {
        let shared = Shared::<C>::new();
        let controls = QuotientControls::new();
        let mut owner = quotient_vanishing!(&params, pk.clone(), &instances, &shared, &controls);
        let mut sentinel = quotient_sentinel(&mut owner.provider, 4, 77);
        let minimum =
            crate::plonk::prover::stored::quotient::scratch_bytes::<C, QuotientProvider<C>>(
                &owner.pk,
                owner.random.coefficient.layout,
            )
            .unwrap();
        assert!(minimum > ((2 * 16 + 16 * 256) * 32 + 256 * 32));
        let mut budget = 1 << 24;
        match case {
            0 => budget = 0,
            1 => budget = minimum - 1,
            2 => owner.usable_rows += 1,
            3 => owner.params = &alternate,
            4 => owner.params = &wrong_k,
            5 => owner.pk.vk.cs_degree = 2,
            6 => {
                owner.pk.fixed_polys.pop().unwrap();
            }
            7 => {
                owner.pk.permutation.polys.pop().unwrap();
            }
            8 => {
                owner.pk.fixed_polys[0].values.pop();
            }
            9 => {
                owner.pk.permutation.polys[0].values.pop();
            }
            10 => {
                owner.pk.l0.values.pop();
            }
            11 => {
                owner.pk.l_last.values.pop();
            }
            12 => {
                owner.pk.l_active_row.values.pop();
            }
            13 => owner.pk.fixed_values.push(
                owner
                    .pk
                    .vk
                    .domain
                    .lagrange_from_vec(vec![C::Scalar::ZERO; 16]),
            ),
            14 => owner.pk.permutation.permutations.push(
                owner
                    .pk
                    .vk
                    .domain
                    .lagrange_from_vec(vec![C::Scalar::ZERO; 16]),
            ),
            15 => {
                owner.instance_coefficients.pop().unwrap();
            }
            16 => owner.instances = &[],
            17 => owner.instances = &bad_instances,
            18 => {
                owner.permutations.pop().unwrap();
            }
            19 => {
                owner.lookups.pop().unwrap();
            }
            20 => {
                owner.pk.ev.lookups.pop().unwrap();
            }
            21 => {
                owner.pk.vk.cs.advice_column_phase.pop().unwrap();
            }
            22 => {
                owner.pk.vk.cs.challenge_phase.pop().unwrap();
            }
            23 => owner.pk.vk.cs.num_advice_columns += 1,
            24 => owner.pk.vk.cs.num_challenges += 1,
            25 => {
                owner.pk.vk.cs.permutation.columns[0] = Column::<Instance>::new(99, Instance).into()
            }
            26 => owner.pk.vk.cs.permutation.columns[1] = owner.pk.vk.cs.permutation.columns[0],
            27 => owner.instance_coefficients[0].layout = owner.instance_coefficients[1].layout,
            28 => owner.random.coefficient.layout = owner.instance_coefficients[0].layout,
            29 => controls.drift(owner.advice.layouts().unwrap().last().unwrap().ordinal()),
            30 => owner.pk.vk.cs.num_instance_columns = usize::MAX,
            31 => owner.pk.ev.custom_gates.calculations[0].target = usize::MAX,
            32 => controls.drift(owner.random.coefficient.layout.ordinal()),
            33 => {
                owner.pk.vk.cs.lookups.pop().unwrap();
            }
            34 => owner.pk.vk.domain = crate::poly::EvaluationDomain::new(4, 3),
            _ => unreachable!(),
        }
        let (created, reads, writes, calls, events) = {
            let log = shared.log.lock().unwrap();
            (
                log.created,
                log.reads,
                log.writes,
                log.rng_calls,
                log.events.clone(),
            )
        };
        take_quotient_clears();
        controls.arm(None);
        let result = owner.evaluate_quotient_numerator(budget);
        assert!(result.is_err(), "quotient preflight {case} was accepted");
        let log = shared.log.lock().unwrap();
        assert_eq!(
            (log.created, log.reads, log.writes, log.rng_calls),
            (created, reads, writes, calls)
        );
        assert_eq!(log.events, events);
        drop(log);
        assert!(
            controls
                .bank
                .lock()
                .unwrap()
                .events
                .iter()
                .all(|event| event.kind == IoKind::DropSnapshot)
        );
        assert_eq!(controls.bank.lock().unwrap().live.len(), 1);
        let (_, zero, _, bytes_zero, _, blinds_zero) = take_quotient_clears();
        assert!(zero && bytes_zero && blinds_zero);
        inspect_quotient_sentinel(&mut sentinel);
        drop(result);
        drop(sentinel);
        assert_dropped(&shared);
    }
}
#[test]
fn both_pasta_quotient_budget_key_geometry_metadata_and_owner_preflights_fail_before_scalar_io() {
    quotient_preflight::<EqAffine>();
    quotient_preflight::<EpAffine>();
}

fn quotient_provider_capacity<C, const I: usize>()
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
{
    const Q: bool = false;
    const A: usize = 0;
    const L: bool = false;
    const QUERY: bool = true;
    let params = ParamsIPA::<C>::new(4);
    let pk = pressure_key::<C, A, I, L, QUERY>(&params);
    let values = (0..I)
        .map(|i| vec![C::Scalar::from((i + 1) as u64)])
        .collect::<Vec<_>>();
    let instances = values.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let shared = Shared::<C>::new();
    let controls = QuotientControls::new();
    let mut owner = pressure_member!(&params, pk, &instances, &shared, &controls)
        .commit_permuted_lookups(1 << 20)
        .unwrap()
        .commit_products(1 << 20)
        .unwrap()
        .commit_vanishing_and_stage_coefficients(1 << 20)
        .unwrap();
    assert_eq!(controls.bank.lock().unwrap().live.len(), I + 1);
    let mut sentinel = quotient_sentinel(&mut owner.provider, 4, 777);
    let (calls, events) = {
        let log = shared.log.lock().unwrap();
        (log.rng_calls, log.events.clone())
    };
    controls.arm(None);
    take_quotient_clears();
    let result = owner.evaluate_quotient_numerator(1 << 24);
    assert!(
        result.is_err(),
        "admitted an unavailable required cache slot"
    );
    let events_after = controls.bank.lock().unwrap().events.clone();
    if I == 503 {
        // A real reachable prefix owns 504 receipts. Its eight outputs leave no room
        // for the one private coset required by this actual instance-query graph.
        assert!(
            events_after
                .iter()
                .all(|event| event.kind == IoKind::DropSnapshot)
        );
    } else {
        assert_eq!(I, 502);
        assert_eq!(controls.bank.lock().unwrap().peak, 512);
        assert!(
            events_after
                .iter()
                .any(|event| event.kind == IoKind::Create)
        );
        assert!(events_after.iter().any(|event| event.kind == IoKind::Read));
        // The planner cannot count an unrelated backend client's live receipt. The
        // provider must refuse capacity normally, without freeing that other receipt.
    }
    assert_eq!(controls.bank.lock().unwrap().live.len(), 1);
    assert_eq!(shared.log.lock().unwrap().rng_calls, calls);
    assert_eq!(shared.log.lock().unwrap().events, events);
    let (_, zero, _, bytes_zero, _, blinds_zero) = take_quotient_clears();
    assert!(zero && bytes_zero && blinds_zero);
    inspect_quotient_sentinel(&mut sentinel);
    drop(result);
    drop(sentinel);
    assert_dropped(&shared);
    assert!(controls.bank.lock().unwrap().live.is_empty());
}
#[test]
fn both_pasta_quotient_actual_private_query_requires_one_slot_and_provider_capacity_preserves_other_clients()
 {
    quotient_provider_capacity::<EqAffine, 503>();
    quotient_provider_capacity::<EpAffine, 503>();
    quotient_provider_capacity::<EqAffine, 502>();
    quotient_provider_capacity::<EpAffine, 502>();
}

fn quotient_ordinals<C>()
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
{
    const Q: bool = false;
    const M: u64 = 0;
    let params = ParamsIPA::<C>::new(4);
    let pk = quotient_key::<C, true, true>(&params);
    let values = fixture_values::<C::Scalar>(16 - (pk.vk.cs.blinding_factors() + 1), false);
    let instances = values.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let shared = Shared::<C>::new();
    let controls = QuotientControls::new();
    let owner = quotient_vanishing!(&params, pk.clone(), &instances, &shared, &controls);
    controls.arm(None);
    take_cache_observations();
    let mut actual = owner.evaluate_quotient_numerator(1 << 24).unwrap();
    let baseline = controls.bank.lock().unwrap().events.clone();
    let first = baseline
        .iter()
        .position(|event| event.kind == IoKind::Create)
        .unwrap();
    let last = baseline
        .iter()
        .rposition(|event| event.kind == IoKind::Create)
        .unwrap();
    assert_eq!(
        baseline[last].layout.role(),
        StoredPolynomialRoleV1::QuotientNumerator
    );
    let expected = actual
        .parts
        .iter_mut()
        .map(|polynomial| read_compressed::<C, _>(&mut polynomial.snapshot, polynomial.layout))
        .collect::<Vec<_>>();
    let (hits, _, _, _, _) = take_cache_observations();
    assert!(
        hits > 0,
        "this regression must consume opportunities on cache hits"
    );
    drop(actual);
    assert_dropped(&shared);
    for final_output in [false, true] {
        let shared = Shared::<C>::new();
        let controls = QuotientControls::new();
        let mut owner = quotient_vanishing!(&params, pk.clone(), &instances, &shared, &controls);
        let mut sentinel = quotient_sentinel(&mut owner.provider, 4, 77);
        let originals = controls.bank.lock().unwrap().live.clone();
        let public = public_values(&owner.pk);
        let (calls, events) = {
            let log = shared.log.lock().unwrap();
            (log.rng_calls, log.events.clone())
        };
        // The sentinel advances the cursor but does not add a numerator-stage callback.
        let target = if final_output { last } else { first };
        controls.arm(None);
        controls.bank.lock().unwrap().ordinal_jump = Some((target, u64::MAX - 1));
        take_cache_observations();
        take_quotient_clears();
        let result = owner.evaluate_quotient_numerator(1 << 24);
        assert!(controls.bank.lock().unwrap().ordinal_jump.is_none());
        assert_eq!(shared.log.lock().unwrap().rng_calls, calls);
        assert_eq!(shared.log.lock().unwrap().events, events);
        if final_output {
            let mut actual = result.unwrap();
            assert_eq!(actual.parts.last().unwrap().layout.ordinal(), u64::MAX - 1);
            assert_eq!(
                actual.inner.advice.greatest_ordinal().unwrap(),
                Some(u64::MAX - 1)
            );
            assert_eq!(
                actual.inner.provider.inner.inner.inner.inner.inner.ordinal,
                u64::MAX
            );
            assert_eq!(public_values(&actual.inner.pk), public);
            assert!(take_cache_observations().0 > 0);
            for (polynomial, expected) in actual.parts.iter_mut().zip(&expected) {
                assert_eq!(
                    read_compressed::<C, _>(&mut polynomial.snapshot, polynomial.layout),
                    *expected
                );
            }
            assert_eq!(
                controls.bank.lock().unwrap().live.len(),
                originals.len() + expected.len()
            );
            drop(actual);
        } else {
            assert!(
                result.is_err(),
                "a real first-factory gap exhausted future opportunities"
            );
            let log = controls.bank.lock().unwrap();
            assert_eq!(
                log.events
                    .iter()
                    .filter(|event| event.kind == IoKind::Create)
                    .count(),
                1
            );
            assert!(
                log.events.iter().all(|event| !matches!(
                    event.kind,
                    IoKind::Read | IoKind::Write | IoKind::Seal
                ))
            );
            drop(log);
            drop(result);
        }
        assert_eq!(controls.bank.lock().unwrap().live.len(), 1);
        let (_, zero, _, bytes_zero, _, blinds_zero) = take_quotient_clears();
        assert!(zero && bytes_zero && blinds_zero);
        inspect_quotient_sentinel(&mut sentinel);
        drop(sentinel);
        assert_dropped(&shared);
        assert!(controls.bank.lock().unwrap().live.is_empty());
    }
}
#[test]
fn both_pasta_quotient_actual_global_cursor_gap_rechecks_future_work_and_final_max_minus_one_follows_cache_hits()
 {
    quotient_ordinals::<EqAffine>();
    quotient_ordinals::<EpAffine>();
}
