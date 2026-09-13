//! Independent active ordinary-product trajectory and consuming-owner regressions.
//!
//! Plaintext test banks and guarded-slot observations do not qualify encrypted Core storage,
//! a complete stored proof, upstream MSM scratch erasure, or process RSS. The oracle invokes
//! the existing ordinary permutation and lookup implementations on the original RNG state.

use super::*;
use crate::plonk::prover::stored::products::{
    scratch_bytes as product_scratch_bytes, take_clear_observations as take_product_clears,
};
use crate::poly::stored_advice::STORED_SCALARS_PER_CHUNK_V1;

#[derive(Clone, Copy)]
struct ProductConfig {
    advice: [Column<Advice>; 3],
    fixed: [Column<Fixed>; 2],
    instances: [Column<Instance>; 4],
}
struct ProductCircuit<C: CurveAffine, const COPY: bool, const LOOKUPS: bool>(Producer<C>);
impl<C: CurveAffine, const COPY: bool, const LOOKUPS: bool> ProductCircuit<C, COPY, LOOKUPS> {
    fn run(
        &self,
        config: ProductConfig,
        mut layouter: impl Layouter<C::Scalar>,
        measurement: bool,
    ) -> Result<(), Error> {
        if measurement {
            self.0.shared.log.lock().unwrap().measurement_passes += 1;
        } else {
            self.0.shared.log.lock().unwrap().synthesis_passes += 1;
        }
        let first = layouter.assign_region(
            || "products copy cycle",
            |mut region| {
                let mut cells = Vec::new();
                for (column, advice) in config.advice.iter().enumerate() {
                    for row in 0..=self.0.last_row {
                        let value = if row == 0 {
                            C::Scalar::ZERO
                        } else {
                            C::Scalar::from((column * 13 + row + 1) as u64)
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
                for fixed in config.fixed {
                    cells.push(region.assign_fixed(fixed, 0, C::Scalar::ZERO));
                }
                if COPY {
                    for pair in cells.windows(2) {
                        region.constrain_equal(pair[0], pair[1]);
                    }
                }
                Ok(cells[0])
            },
        )?;
        if COPY {
            layouter.constrain_instance(first, config.instances[0], 0);
            layouter.constrain_instance(first, config.instances[2], 0);
        }
        Ok(())
    }
}
impl<C: CurveAffine, const COPY: bool, const LOOKUPS: bool> Circuit<C::Scalar>
    for ProductCircuit<C, COPY, LOOKUPS>
{
    type Config = ProductConfig;
    type FloorPlanner = V1;
    #[cfg(feature = "circuit-params")]
    type Params = ();
    fn without_witnesses(&self) -> Self {
        Self(self.0.without_witnesses())
    }
    fn configure(meta: &mut ConstraintSystem<C::Scalar>) -> ProductConfig {
        let config = ProductConfig {
            advice: std::array::from_fn(|_| meta.advice_column()),
            fixed: std::array::from_fn(|_| meta.fixed_column()),
            instances: std::array::from_fn(|_| meta.instance_column()),
        };
        // The copy cycle crosses advice, fixed and instance sets. A fourth-degree gate
        // fixes a two-column set width even when the lookup inventory is empty.
        meta.create_gate("fixed product degree", |meta| {
            let a = meta.query_advice(config.advice[0], Rotation::cur());
            let term = a.clone() * a.clone() * a.clone() * a;
            vec![term.clone() - term]
        });
        if COPY {
            for column in config.advice {
                meta.enable_equality(column);
            }
            for column in config.fixed {
                meta.enable_equality(column);
            }
            for column in config.instances {
                meta.enable_equality(column);
            }
        }
        if LOOKUPS {
            for lookup in 0..2 {
                meta.lookup_any("product instance membership", |meta| {
                    vec![(
                        meta.query_instance(config.instances[2 * lookup], Rotation::cur()),
                        meta.query_instance(config.instances[2 * lookup + 1], Rotation::cur()),
                    )]
                });
            }
        }
        config
    }
    fn synthesize_for_measurement(
        &self,
        config: ProductConfig,
        layouter: impl Layouter<C::Scalar>,
    ) -> Result<(), Error> {
        self.run(config, layouter, true)
    }
    fn synthesize(
        &self,
        config: ProductConfig,
        layouter: impl Layouter<C::Scalar>,
    ) -> Result<(), Error> {
        self.run(config, layouter, false)
    }
}
fn product_key<C, const COPY: bool, const LOOKUPS: bool>(params: &ParamsIPA<C>) -> ProvingKey<C>
where
    C: CurveAffine,
    C::Scalar: FromUniformBytes<64>,
{
    let shared = Shared::<C>::new();
    let producer = ProductCircuit::<C, COPY, LOOKUPS>(Producer::new(&shared, 3));
    let vk = keygen_vk_custom(params, &producer, true).unwrap();
    keygen_pk(params, vk, &producer).unwrap()
}

// This explicitly test-only challenge carrier allows zero-denominator fixtures without
// guessing a Blake2b preimage or changing either production implementation's challenge API.
struct ProductChallenge<C: CurveAffine>(C::Scalar);
impl<C: CurveAffine> EncodedChallenge<C> for ProductChallenge<C> {
    type Input = C::Scalar;
    fn new(value: &C::Scalar) -> Self {
        Self(*value)
    }
    fn get_scalar(&self) -> C::Scalar {
        self.0
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ProductChange {
    Field,
    K,
    Basis,
    Context,
    CosetBasis,
    Ordinal,
    Exhausted,
    Insufficient,
    Index,
    WrongRole,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ProductFault {
    None,
    CreateLayout(ProductChange),
    WriterSecond,
    WriterAfterWrite,
    SqueezePanic(usize),
    SqueezeDrift(usize, u64),
    PointDrift(usize, u64),
    RngDrift(usize, u64),
}
#[derive(Clone)]
struct ProductControls<C: CurveAffine> {
    base: Controls,
    fault: Arc<Mutex<ProductFault>>,
    forced: Arc<Mutex<Vec<C::Scalar>>>,
}
impl<C: CurveAffine> ProductControls<C> {
    fn new() -> Self {
        Self {
            base: Controls::new(),
            fault: Arc::new(Mutex::new(ProductFault::None)),
            forced: Arc::new(Mutex::new(Vec::new())),
        }
    }
}
struct ProductTranscript<C: CurveAffine>
where
    C::Scalar: FromUniformBytes<64>,
{
    inner: BoundaryTranscript<C>,
    controls: ProductControls<C>,
}
impl<C: CurveAffine> ProductTranscript<C>
where
    C::Scalar: FromUniformBytes<64>,
{
    fn new(shared: &Arc<Shared<C>>, controls: &ProductControls<C>) -> Self {
        Self {
            inner: BoundaryTranscript {
                inner: RecordingTranscript::new(shared),
                controls: controls.base.clone(),
            },
            controls: controls.clone(),
        }
    }
}
impl<C: CurveAffine> Transcript<C, ProductChallenge<C>> for ProductTranscript<C>
where
    C::Scalar: FromUniformBytes<64>,
{
    fn squeeze_challenge(&mut self) -> ProductChallenge<C> {
        let index = self.inner.inner.shared.log.lock().unwrap().events.len();
        let fault = *self.controls.fault.lock().unwrap();
        assert_ne!(
            fault,
            ProductFault::SqueezePanic(index),
            "injected product challenge unwind"
        );
        let mut value = self.inner.squeeze_challenge().get_scalar();
        let mut forced = self.controls.forced.lock().unwrap();
        if !forced.is_empty() {
            value = forced.remove(0);
            *self
                .inner
                .inner
                .shared
                .log
                .lock()
                .unwrap()
                .events
                .last_mut()
                .unwrap() = Event::Challenge(value);
        }
        drop(forced);
        if let ProductFault::SqueezeDrift(trigger, victim) = fault {
            if trigger == index {
                override_receipt(&self.inner.inner.shared, victim);
            }
        }
        ProductChallenge(value)
    }
    fn common_point(&mut self, point: C) -> io::Result<()> {
        self.inner.common_point(point)
    }
    fn common_scalar(&mut self, value: C::Scalar) -> io::Result<()> {
        self.inner.common_scalar(value)
    }
}
impl<C: CurveAffine> TranscriptWrite<C, ProductChallenge<C>> for ProductTranscript<C>
where
    C::Scalar: FromUniformBytes<64>,
{
    fn write_point(&mut self, point: C) -> io::Result<()> {
        let index = self.inner.inner.shared.log.lock().unwrap().events.len();
        let result = self.inner.write_point(point);
        if let ProductFault::PointDrift(trigger, victim) = *self.controls.fault.lock().unwrap() {
            if result.is_ok() && trigger == index {
                override_receipt(&self.inner.inner.shared, victim);
            }
        }
        result
    }
    fn write_scalar(&mut self, value: C::Scalar) -> io::Result<()> {
        self.inner.write_scalar(value)
    }
}
struct ProductRng<C: CurveAffine> {
    inner: BoundaryRng<C>,
    controls: ProductControls<C>,
}
impl<C: CurveAffine> ProductRng<C> {
    fn after(&self, index: usize) {
        if let ProductFault::RngDrift(trigger, victim) = *self.controls.fault.lock().unwrap() {
            if trigger == index {
                override_receipt(&self.inner.inner.0, victim);
            }
        }
    }
    fn index(&self) -> usize {
        self.inner.inner.0.log.lock().unwrap().rng_calls
    }
}
impl<C: CurveAffine> RngCore for ProductRng<C> {
    fn next_u32(&mut self) -> u32 {
        let i = self.index();
        let v = self.inner.next_u32();
        self.after(i);
        v
    }
    fn next_u64(&mut self) -> u64 {
        let i = self.index();
        let v = self.inner.next_u64();
        self.after(i);
        v
    }
    fn fill_bytes(&mut self, bytes: &mut [u8]) {
        let i = self.index();
        self.inner.fill_bytes(bytes);
        self.after(i);
    }
    fn try_fill_bytes(&mut self, bytes: &mut [u8]) -> Result<(), RngError> {
        let i = self.index();
        let r = self.inner.try_fill_bytes(bytes);
        self.after(i);
        r
    }
}
struct ProductProvider<C: CurveAffine> {
    inner: PermutedProvider<C>,
    controls: ProductControls<C>,
}
struct ProductWriter<C: CurveAffine> {
    inner: PermutedWriter<C>,
    controls: ProductControls<C>,
    observations: std::cell::Cell<usize>,
}
fn product_role(role: StoredPolynomialRoleV1) -> bool {
    matches!(
        role,
        StoredPolynomialRoleV1::CopyPermutationProduct { .. }
            | StoredPolynomialRoleV1::LookupProduct { .. }
    )
}
fn product_change(
    layout: StoredPolynomialLayoutV1,
    change: ProductChange,
) -> StoredPolynomialLayoutV1 {
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
        if change == ProductChange::Basis {
            StoredPolynomialBasisV1::Lagrange
        } else if change == ProductChange::CosetBasis {
            StoredPolynomialBasisV1::CosetPart {
                extension_log: 1,
                part: 0,
            }
        } else {
            layout.basis()
        },
        if change == ProductChange::K {
            layout.k() + 1
        } else {
            layout.k()
        },
        match (change, layout.role()) {
            (ProductChange::Index, StoredPolynomialRoleV1::CopyPermutationProduct { set }) => {
                StoredPolynomialRoleV1::CopyPermutationProduct { set: set + 1 }
            }
            (ProductChange::Index, StoredPolynomialRoleV1::LookupProduct { lookup }) => {
                StoredPolynomialRoleV1::LookupProduct { lookup: lookup + 1 }
            }
            (ProductChange::WrongRole, StoredPolynomialRoleV1::CopyPermutationProduct { set }) => {
                StoredPolynomialRoleV1::LookupProduct { lookup: set }
            }
            (ProductChange::WrongRole, StoredPolynomialRoleV1::LookupProduct { lookup }) => {
                StoredPolynomialRoleV1::CopyPermutationProduct { set: lookup }
            }
            _ => layout.role(),
        },
    )
    .unwrap()
}
impl<C: CurveAffine> StoredPolynomialProviderV1 for ProductProvider<C> {
    type Writer = ProductWriter<C>;
    fn create(
        &mut self,
        field: StoredPastaFieldV1,
        basis: StoredPolynomialBasisV1,
        k: u32,
        role: StoredPolynomialRoleV1,
    ) -> Result<Self::Writer, StoredPolynomialErrorV1> {
        let mut inner = self.inner.create(field, basis, k, role)?;
        if product_role(role) {
            if let ProductFault::CreateLayout(change) = *self.controls.fault.lock().unwrap() {
                inner.inner.inner.layout = product_change(inner.inner.inner.layout, change);
            }
        }
        Ok(ProductWriter {
            inner,
            controls: self.controls.clone(),
            observations: std::cell::Cell::new(0),
        })
    }
}
impl<C: CurveAffine> StoredPolynomialWriterV1 for ProductWriter<C> {
    type Snapshot = PermutedSnapshot<C>;
    fn layout(&self) -> StoredPolynomialLayoutV1 {
        let count = self.observations.get();
        self.observations.set(count + 1);
        let layout = self.inner.layout();
        let fault = *self.controls.fault.lock().unwrap();
        if product_role(layout.role())
            && ((fault == ProductFault::WriterSecond && count > 0)
                || (fault == ProductFault::WriterAfterWrite && self.inner.inner.inner.next > 0))
        {
            different_context(layout)
        } else {
            layout
        }
    }
    fn write_chunk(
        &mut self,
        chunk: u64,
        values: &[[u8; 32]],
    ) -> Result<(), StoredPolynomialErrorV1> {
        self.inner.write_chunk(chunk, values)
    }
    fn seal(self) -> Result<Self::Snapshot, StoredPolynomialErrorV1> {
        self.inner.seal()
    }
}
macro_rules! product_member {
    ($params:expr,$pk:expr,$instances:expr,$shared:expr,$controls:expr,$copy:tt,$lookups:tt,$q:ident,$mask:ident) => {
        prepare_single_phase_stored_ipa_prefix_v1::<C, _, _, _, ProductChallenge<C>, _, $q, $mask>(
            $params,
            $pk,
            ProductCircuit::<C, $copy, $lookups>(Producer::new($shared, 3)),
            $instances,
            ProductProvider {
                inner: PermutedProvider {
                    inner: MemberProvider {
                        inner: Provider::new($shared),
                        fault: Arc::clone(&$controls.base.backend),
                        window: Arc::clone(&$controls.base.window),
                    },
                    controls: $controls.base.clone(),
                },
                controls: $controls.clone(),
            },
            ProductRng {
                inner: BoundaryRng {
                    inner: Rng(Arc::clone($shared)),
                    controls: $controls.base.clone(),
                },
                controls: $controls.clone(),
            },
            ProductTranscript::new($shared, $controls),
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
fn replay_product_prefix<C>(events: &[Event<C>], transcript: &mut ProductTranscript<C>)
where
    C: CurveAffine,
    C::Scalar: FromUniformBytes<64>,
{
    for event in events {
        match *event {
            Event::CommonScalar(v) => transcript.common_scalar(v).unwrap(),
            Event::CommonPoint(v) => transcript.common_point(v).unwrap(),
            Event::WriteScalar(v) => transcript.write_scalar(v).unwrap(),
            Event::WritePoint(v) => transcript.write_point(v).unwrap(),
            Event::Challenge(v) => assert_eq!(transcript.squeeze_challenge().get_scalar(), v),
        }
    }
}

fn ordinary_products<C, const COPY: bool, const LOOKUPS: bool, const Q: bool, const M: u64>(
    k: u32,
    zero: bool,
) where
    C: CurveAffine + crate::SerdeCurveAffine,
    C::Scalar: StoredAssignmentFieldV1
        + WithSmallOrderMulGroup<3>
        + FromUniformBytes<64>
        + crate::SerdePrimeField,
{
    use crate::plonk::lookup::prover::stored_products_ordinary_oracle;
    let params = ParamsIPA::<C>::new(k);
    let pk = product_key::<C, COPY, LOOKUPS>(&params);
    let n = 1_usize << k;
    let usable = n - (pk.vk.cs.blinding_factors() + 1);
    let values = fixture_values::<C::Scalar>(usable, false);
    let instances = values.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let shared = Shared::<C>::new();
    let controls = ProductControls::new();
    let member = product_member!(
        &params, pk, &instances, &shared, &controls, COPY, LOOKUPS, Q, M
    );
    let advice_layouts = member
        .compressed
        .inner
        .advice
        .layouts()
        .unwrap()
        .collect::<Vec<_>>();
    let challenges = member
        .compressed
        .inner
        .advice
        .challenges()
        .unwrap()
        .collect::<Vec<_>>();
    let (prefix, sealed) = {
        let log = shared.log.lock().unwrap();
        (log.events.clone(), log.sealed.clone())
    };
    let advice = advice_layouts
        .iter()
        .map(|layout| {
            let encoded = &sealed
                .iter()
                .find(|(identity, _)| identity == layout)
                .unwrap()
                .1;
            member.compressed.inner.pk.vk.domain.lagrange_from_vec(
                encoded
                    .iter()
                    .map(|v| Option::<C::Scalar>::from(C::Scalar::from_repr(*v)).unwrap())
                    .collect(),
            )
        })
        .collect::<Vec<_>>();
    let dense_instances = values
        .iter()
        .map(|values| {
            let mut full = values.clone();
            full.resize(n, C::Scalar::ZERO);
            member.compressed.inner.pk.vk.domain.lagrange_from_vec(full)
        })
        .collect::<Vec<_>>();
    let sets = if COPY {
        member
            .compressed
            .inner
            .pk
            .vk
            .cs
            .permutation
            .columns
            .len()
            .div_ceil(member.compressed.inner.pk.vk.cs_degree - 2)
    } else {
        0
    };
    if COPY {
        assert!(sets >= 3, "fixture must cross several original copy sets");
    }
    let count = if LOOKUPS { 2 } else { 0 };
    let oracle_shared = Shared::<C>::new();
    let oracle_controls = ProductControls::new();
    let mut oracle_transcript = ProductTranscript::new(&oracle_shared, &oracle_controls);
    replay_product_prefix(&prefix, &mut oracle_transcript);
    let mut oracle_rng = shared.rng.lock().unwrap().clone();
    if zero {
        *controls.forced.lock().unwrap() = vec![C::Scalar::ZERO, C::Scalar::ZERO];
        *oracle_controls.forced.lock().unwrap() = vec![C::Scalar::ZERO, C::Scalar::ZERO];
    }
    let oracle = catch_unwind(AssertUnwindSafe(|| {
        stored_products_ordinary_oracle(
            &member.compressed.inner.pk,
            &params,
            member.compressed.theta,
            &advice,
            &dense_instances,
            &challenges,
            &mut oracle_rng,
            &mut oracle_transcript,
        )
    }));
    let mut permuted = member.commit_permuted_lookups(1 << 20).unwrap();
    let pair_prefix = shared.log.lock().unwrap().events.len();
    let fixed_ptr = permuted.compressed.inner.pk.fixed_polys.as_ptr();
    let sigma_ptr = permuted.compressed.inner.pk.permutation.polys.as_ptr();
    let theta = *permuted.compressed.theta;
    let vk = permuted
        .compressed
        .inner
        .pk
        .get_vk()
        .to_bytes(crate::SerdeFormat::Processed);
    let previous = permuted
        .lookups
        .iter_mut()
        .map(|pair| {
            [&mut pair.input, &mut pair.table].map(|side| {
                (
                    side.coefficient.layout,
                    read_compressed::<C, _>(
                        &mut side.coefficient.snapshot,
                        side.coefficient.layout,
                    ),
                    (side.blind.0).0,
                    side.commitment,
                )
            })
        })
        .collect::<Vec<_>>();
    let first = permuted.compressed.inner.provider.inner.inner.inner.ordinal;
    let old_live = {
        let log = shared.log.lock().unwrap();
        log.sealed.len() - log.snapshot_drops
    };
    assert_eq!(old_live, 2 * advice_layouts.len() + 6 * count);
    // This cap admits exactly the peak live set; it would refuse another unnecessary
    // preprocessing receipt retained after a lookup product's seal and transcript write.
    *controls.base.backend.lock().unwrap() =
        MemberFault::Capacity(old_live + sets + usize::from(count > 0));
    let budget =
        product_scratch_bytes::<C, PermutedSnapshot<C>, ProductWriter<C>>(k, sets, count).unwrap();
    take_product_clears();
    let actual = catch_unwind(AssertUnwindSafe(|| permuted.commit_products(budget)));
    if zero && LOOKUPS && cfg!(feature = "sanity-checks") {
        // The active ordinary implementation asserts the terminal lookup invariant for
        // this intentionally degenerate challenge. The stored path must fail identically.
        assert!(oracle.is_err());
        assert!(actual.is_err());
        assert_eq!(
            shared.log.lock().unwrap().events,
            oracle_shared.log.lock().unwrap().events
        );
        let mut a = [0; 64];
        let mut b = [0; 64];
        shared.rng.lock().unwrap().clone().fill_bytes(&mut a);
        oracle_rng.fill_bytes(&mut b);
        assert_eq!(a, b);
        assert_dropped(&shared);
        let (_, fz, _, ez, _, bz) = take_product_clears();
        assert!(fz && ez && bz);
        return;
    }
    let oracle = oracle.unwrap().unwrap();
    let mut actual = actual.unwrap().unwrap();
    assert_eq!(oracle.pairs.len(), count);
    for (before, pair) in previous.iter().zip(&oracle.pairs) {
        assert_eq!(before[0].1, pair.input_coefficient);
        assert_eq!(before[1].1, pair.table_coefficient);
        assert_eq!(before[0].2, pair.input_blind.0);
        assert_eq!(before[1].2, pair.table_blind.0);
    }
    assert_eq!(*actual.beta, oracle.beta);
    assert_eq!(*actual.gamma, oracle.gamma);
    if zero {
        assert_eq!(*actual.beta, C::Scalar::ZERO);
        assert_eq!(*actual.gamma, C::Scalar::ZERO);
    }
    assert_eq!(*actual.theta, theta);
    assert_eq!(actual.usable_rows, usable);
    assert_eq!(actual.permutations.len(), sets);
    assert_eq!(actual.lookups.len(), count);
    assert_eq!(actual.inner.pk.fixed_polys.as_ptr(), fixed_ptr);
    assert_eq!(actual.inner.pk.permutation.polys.as_ptr(), sigma_ptr);
    assert!(actual.inner.pk.fixed_values.is_empty());
    assert!(actual.inner.pk.permutation.permutations.is_empty());
    assert_eq!(
        actual
            .inner
            .pk
            .get_vk()
            .to_bytes(crate::SerdeFormat::Processed),
        vk
    );
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
        challenges
    );
    assert!(Arc::ptr_eq(
        &actual.inner.provider.inner.inner.inner.shared,
        &shared
    ));
    assert!(Arc::ptr_eq(&actual.inner.rng.inner.inner.0, &shared));
    assert!(Arc::ptr_eq(
        &actual.inner.transcript.inner.inner.shared,
        &shared
    ));
    let ordinary_events = oracle_shared.log.lock().unwrap().events.clone();
    assert_eq!(shared.log.lock().unwrap().events, ordinary_events);
    assert_eq!(ordinary_events[pair_prefix], Event::Challenge(oracle.beta));
    assert_eq!(
        ordinary_events[pair_prefix + 1],
        Event::Challenge(oracle.gamma)
    );
    let points = ordinary_events[pair_prefix + 2..]
        .iter()
        .map(|event| match event {
            Event::WritePoint(p) => *p,
            _ => panic!("non-product event"),
        })
        .collect::<Vec<_>>();
    assert_eq!(points.len(), sets + count);
    let mut chained = C::Scalar::ONE;
    let mut nontrivial = false;
    for (index, (product, expected)) in actual
        .permutations
        .iter_mut()
        .zip(&oracle.permutations)
        .enumerate()
    {
        assert_eq!(
            product.coefficient.layout.role(),
            StoredPolynomialRoleV1::CopyPermutationProduct { set: index as u32 }
        );
        assert_eq!(product.coefficient.layout.ordinal(), first + index as u64);
        assert_eq!(
            product.coefficient.layout.basis(),
            StoredPolynomialBasisV1::Coefficient
        );
        let coefficients = read_compressed::<C, _>(
            &mut product.coefficient.snapshot,
            product.coefficient.layout,
        );
        assert_eq!(coefficients, expected.coefficient);
        assert_eq!((product.blind.0).0, expected.blind.0);
        assert_eq!(product.commitment, points[index]);
        let lagrange = product_lagrange(&actual.inner.pk, &coefficients);
        assert_eq!(lagrange[0], chained, "set {index} restarted last_z");
        chained = lagrange[usable];
        nontrivial |= chained != C::Scalar::ONE;
        assert!(lagrange[usable + 1..].iter().any(|v| *v != C::Scalar::ZERO));
        assert_eq!(
            params
                .commit_lagrange(&lagrange, product.blind.0)
                .to_affine(),
            product.commitment
        );
        if zero {
            assert_eq!(lagrange[1], C::Scalar::ZERO);
        }
    }
    if COPY {
        assert!(
            nontrivial,
            "copy fixture failed to exercise cross-set state"
        );
    }
    for (index, ((lookup, expected), before)) in actual
        .lookups
        .iter_mut()
        .zip(&oracle.lookups)
        .zip(&previous)
        .enumerate()
    {
        for (side, (coefficient, blind)) in
            [&mut lookup.input, &mut lookup.table].into_iter().zip([
                (&expected.input_coefficient, expected.input_blind),
                (&expected.table_coefficient, expected.table_blind),
            ])
        {
            let original = if side.coefficient.layout.role() == before[0].0.role() {
                &before[0]
            } else {
                &before[1]
            };
            assert_eq!(side.coefficient.layout, original.0);
            assert_eq!(
                read_compressed::<C, _>(&mut side.coefficient.snapshot, side.coefficient.layout),
                *coefficient
            );
            assert_eq!(*coefficient, original.1);
            assert_eq!((side.blind.0).0, blind.0);
            assert_eq!(blind.0, original.2);
            assert_eq!(side.commitment, original.3);
        }
        let product = &mut lookup.product;
        assert_eq!(
            product.coefficient.layout.role(),
            StoredPolynomialRoleV1::LookupProduct {
                lookup: index as u32
            }
        );
        assert_eq!(
            product.coefficient.layout.ordinal(),
            first + (sets + index) as u64
        );
        let coefficients = read_compressed::<C, _>(
            &mut product.coefficient.snapshot,
            product.coefficient.layout,
        );
        assert_eq!(coefficients, expected.product_coefficient);
        assert_eq!((product.blind.0).0, expected.product_blind.0);
        assert_eq!(product.commitment, points[sets + index]);
        let lagrange = product_lagrange(&actual.inner.pk, &coefficients);
        assert_eq!(lagrange[0], C::Scalar::ONE);
        if zero {
            assert_eq!(lagrange[1], C::Scalar::ZERO);
        } else {
            assert_eq!(lagrange[usable], C::Scalar::ONE);
        }
        assert!(lagrange[usable + 1..].iter().any(|v| *v != C::Scalar::ZERO));
        assert_eq!(
            params
                .commit_lagrange(&lagrange, product.blind.0)
                .to_affine(),
            product.commitment
        );
    }
    assert_eq!(
        actual.inner.transcript.squeeze_challenge().get_scalar(),
        oracle_transcript.squeeze_challenge().get_scalar()
    );
    let mut a = [0; 64];
    let mut b = [0; 64];
    actual.inner.rng.fill_bytes(&mut a);
    oracle_rng.fill_bytes(&mut b);
    assert_eq!(a, b);
    let actual_bytes = std::mem::replace(
        &mut actual.inner.transcript.inner.inner.inner,
        Blake2bWrite::init(Vec::new()),
    )
    .finalize();
    let oracle_bytes = std::mem::replace(
        &mut oracle_transcript.inner.inner.inner,
        Blake2bWrite::init(Vec::new()),
    )
    .finalize();
    assert_eq!(actual_bytes, oracle_bytes);
    let log = shared.log.lock().unwrap();
    assert_eq!(
        log.sealed.len() - log.snapshot_drops,
        2 * advice_layouts.len() + sets + 3 * count
    );
    drop(log);
    drop(actual);
    assert_dropped(&shared);
    let (fields, fz, encoded_slots, ez, blinds, bz) = take_product_clears();
    assert!(fz && ez && bz);
    if sets + count > 0 {
        assert!(fields >= n + 4 * STORED_SCALARS_PER_CHUNK_V1 + 1);
        // The shared cleanup observer counts initialized 32-byte slots.
        assert!(encoded_slots >= STORED_SCALARS_PER_CHUNK_V1);
        assert!(blinds >= sets + 3 * count);
    }
}

fn product_matrix<C>()
where
    C: CurveAffine + crate::SerdeCurveAffine,
    C::Scalar: StoredAssignmentFieldV1
        + WithSmallOrderMulGroup<3>
        + FromUniformBytes<64>
        + crate::SerdePrimeField,
{
    for k in [4, 8, 9] {
        ordinary_products::<C, true, true, false, 0>(k, false);
        ordinary_products::<C, true, true, true, 0>(k, false);
        ordinary_products::<C, true, true, true, 6>(k, false);
    }
    ordinary_products::<C, true, false, false, 0>(4, false);
    ordinary_products::<C, false, true, true, 6>(4, false);
    ordinary_products::<C, false, false, false, 0>(4, false);
    ordinary_products::<C, true, false, false, 0>(4, true);
    ordinary_products::<C, true, true, true, 6>(9, true);
}
#[test]
fn both_pasta_products_match_actual_ordinary_pairs_beta_gamma_copy_sets_and_lookups() {
    product_matrix::<EqAffine>();
    product_matrix::<EpAffine>();
}

fn product_lagrange<C: CurveAffine>(
    pk: &ProvingKey<C>,
    coefficients: &[C::Scalar],
) -> crate::poly::Polynomial<C::Scalar, crate::poly::LagrangeCoeff> {
    // Independent evaluation of the actual ordinary coefficient result; this does not
    // reproduce either product recurrence or use the stored inverse-transform routine.
    let omega = pk.vk.domain.get_omega();
    pk.vk.domain.lagrange_from_vec(
        (0..coefficients.len())
            .map(|row| {
                crate::arithmetic::eval_polynomial(coefficients, omega.pow_vartime([row as u64]))
            })
            .collect(),
    )
}

fn product_backend_faults<C>(metadata: bool)
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
{
    const Q: bool = false;
    const M: u64 = 0;
    let params = ParamsIPA::<C>::new(9);
    let pk = product_key::<C, true, true>(&params);
    let usable = 512 - (pk.vk.cs.blinding_factors() + 1);
    let sets = pk
        .vk
        .cs
        .permutation
        .columns
        .len()
        .div_ceil(pk.vk.cs_degree - 2);
    let values = fixture_values::<C::Scalar>(usable, false);
    let instances = values.iter().map(Vec::as_slice).collect::<Vec<_>>();
    for case in 0..if metadata { 32 } else { 20 } {
        let shared = Shared::<C>::new();
        let controls = ProductControls::new();
        let mut permuted = product_member!(
            &params,
            pk.clone(),
            &instances,
            &shared,
            &controls,
            true,
            true,
            Q,
            M
        )
        .commit_permuted_lookups(1 << 20)
        .unwrap();
        let advice = permuted
            .compressed
            .inner
            .advice
            .layouts()
            .unwrap()
            .collect::<Vec<_>>();
        let advice0 = advice[0].ordinal();
        let advice2 = advice[2].ordinal();
        let original = permuted.compressed.lookups[0].input.layout.ordinal();
        let remaining = permuted.compressed.lookups[1].table.layout.ordinal();
        let input = permuted.lookups[0].input.lagrange.layout.ordinal();
        let table = permuted.lookups[0].table.lagrange.layout.ordinal();
        let input_coefficient = permuted.lookups[0].input.coefficient.layout.ordinal();
        let table_coefficient = permuted.lookups[1].table.coefficient.layout.ordinal();
        let coefficient = shared
            .log
            .lock()
            .unwrap()
            .sealed
            .iter()
            .find(|(l, _)| {
                l.basis() == StoredPolynomialBasisV1::Coefficient && l.role() == advice[2].role()
            })
            .unwrap()
            .0
            .ordinal();
        // Same underlying bank, independent ownership. Its bytes must survive every
        // consuming proof failure, including errors after an earlier product is complete.
        let mut writer = permuted
            .compressed
            .inner
            .provider
            .create(
                C::Scalar::STORED_FIELD,
                StoredPolynomialBasisV1::Coefficient,
                9,
                StoredPolynomialRoleV1::LookupProduct { lookup: 77 },
            )
            .unwrap();
        let survivor_layout = writer.layout();
        let expected = vec![C::Scalar::from(99).to_repr(); 512];
        for chunk in 0..2 {
            writer
                .write_chunk(
                    chunk,
                    &expected[chunk as usize * 256..(chunk as usize + 1) * 256],
                )
                .unwrap();
        }
        let mut survivor = writer.seal().unwrap();
        let first = permuted.compressed.inner.provider.inner.inner.inner.ordinal;
        let first_lookup = first + sets as u64;
        let (events, draws, reads, writes, sealed, live) = {
            let l = shared.log.lock().unwrap();
            (
                l.events.clone(),
                l.rng_calls,
                l.reads,
                l.writes,
                l.sealed.clone(),
                l.sealed.len() - l.snapshot_drops,
            )
        };
        let mut panic = false;
        let mut preflight = false;
        if metadata {
            let changes = [
                ProductChange::Field,
                ProductChange::K,
                ProductChange::Basis,
                ProductChange::Context,
                ProductChange::CosetBasis,
                ProductChange::Ordinal,
                ProductChange::Exhausted,
                ProductChange::Insufficient,
                ProductChange::Index,
                ProductChange::WrongRole,
            ];
            match case {
                0..10 => {
                    *controls.fault.lock().unwrap() = ProductFault::CreateLayout(changes[case]);
                    preflight = true;
                }
                10 => {
                    *controls.fault.lock().unwrap() = ProductFault::WriterSecond;
                    preflight = true;
                }
                11 => *controls.fault.lock().unwrap() = ProductFault::WriterAfterWrite,
                12..16 => {
                    *controls.base.backend.lock().unwrap() = MemberFault::SnapshotLayout(
                        [advice2, coefficient, remaining, table_coefficient][case - 12],
                    );
                    preflight = true;
                }
                16 => {
                    *controls.base.backend.lock().unwrap() =
                        MemberFault::AfterRead(advice0, 0, remaining)
                }
                17 => {
                    *controls.base.backend.lock().unwrap() =
                        MemberFault::AfterRead(advice0, 1, coefficient)
                }
                18 => {
                    *controls.base.backend.lock().unwrap() =
                        MemberFault::AfterRead(original, 0, advice2)
                }
                19 => {
                    *controls.base.backend.lock().unwrap() =
                        MemberFault::AfterRead(input, 0, table_coefficient)
                }
                20 => {
                    *controls.base.backend.lock().unwrap() =
                        MemberFault::AfterRead(table, 1, input_coefficient)
                }
                21 => {
                    *controls.base.backend.lock().unwrap() =
                        MemberFault::AfterSeal(first, remaining)
                }
                22 => {
                    *controls.base.backend.lock().unwrap() =
                        MemberFault::AfterSeal(first + 1, first)
                }
                23 => {
                    *controls.base.backend.lock().unwrap() =
                        MemberFault::AfterSeal(first_lookup, input)
                }
                24 => {
                    *controls.base.backend.lock().unwrap() =
                        MemberFault::AfterSeal(first_lookup, original)
                }
                25 => {
                    *controls.base.backend.lock().unwrap() =
                        MemberFault::AfterSeal(first_lookup + 1, input_coefficient)
                }
                26 => {
                    *controls.base.backend.lock().unwrap() =
                        MemberFault::AfterSeal(first_lookup + 1, first_lookup)
                }
                27 => {
                    *controls.base.backend.lock().unwrap() =
                        MemberFault::AfterRead(remaining, 0, first_lookup)
                }
                28 => {
                    *controls.base.backend.lock().unwrap() =
                        MemberFault::AfterSeal(first_lookup + 1, first_lookup + 1)
                }
                29 => {
                    permuted.lookups.swap(0, 1);
                    preflight = true;
                }
                30 => {
                    permuted.compressed.lookups.swap(0, 1);
                    preflight = true;
                }
                31 => {
                    let input = &mut permuted.lookups[0].input;
                    std::mem::swap(&mut input.lagrange.layout, &mut input.coefficient.layout);
                    preflight = true;
                }
                _ => unreachable!(),
            }
        } else {
            *controls.base.backend.lock().unwrap() = match case {
                0 => {
                    preflight = true;
                    MemberFault::Create(first)
                }
                1 => MemberFault::Create(first_lookup + 1),
                2 => MemberFault::Write(first, 1),
                3 => MemberFault::Write(first_lookup, 1),
                4 => MemberFault::Seal(first_lookup + 1),
                5 => MemberFault::Read(advice0, 1),
                6 => MemberFault::Read(original, 1),
                7 => MemberFault::Read(table, 1),
                8 => {
                    panic = true;
                    MemberFault::PanicWrite(first_lookup, 1)
                }
                9 => {
                    panic = true;
                    MemberFault::PanicSeal(first + 1)
                }
                10 => {
                    panic = true;
                    MemberFault::PanicRead(advice0, 1)
                }
                11 => {
                    panic = true;
                    MemberFault::PanicRead(input, 1)
                }
                12 => MemberFault::Encoding(advice0, 1),
                13 => MemberFault::Encoding(original, 1),
                14 => MemberFault::Encoding(input, 1),
                15 => MemberFault::ShortChunk(advice0, 1),
                16 => MemberFault::ShortChunk(table, 1),
                17 => MemberFault::Capacity(live + sets),
                18 => MemberFault::Read(remaining, 1),
                19 => MemberFault::Seal(first),
                _ => unreachable!(),
            };
        }
        take_product_clears();
        let result = catch_unwind(AssertUnwindSafe(|| {
            permuted
                .commit_products(
                    product_scratch_bytes::<C, PermutedSnapshot<C>, ProductWriter<C>>(9, sets, 2)
                        .unwrap(),
                )
                .map(|_| ())
        }));
        if panic {
            assert!(result.is_err(), "unreached backend unwind {case}");
        } else {
            assert!(
                result.unwrap().is_err(),
                "accepted backend metadata={metadata} case={case}"
            );
        }
        assert!(!controls.base.window.load(Ordering::SeqCst));
        if preflight {
            let log = shared.log.lock().unwrap();
            assert_eq!(log.events, events);
            assert_eq!(log.rng_calls, draws);
            assert_eq!(log.reads, reads);
            assert_eq!(log.writes, writes);
        }
        assert_eq!(&shared.log.lock().unwrap().sealed[..sealed.len()], &sealed);
        *controls.base.backend.lock().unwrap() = MemberFault::None;
        *controls.fault.lock().unwrap() = ProductFault::None;
        assert_eq!(survivor.layout(), survivor_layout);
        assert_owned_cleanup_with_survivor(&shared, &mut survivor, &expected);
        drop(survivor);
        assert_dropped(&shared);
        let (_, fz, _, ez, _, bz) = take_product_clears();
        assert!(fz && ez && bz, "metadata={metadata} case={case}");
    }
}
#[test]
fn both_pasta_products_reject_current_remaining_completed_and_original_receipt_substitution() {
    product_backend_faults::<EqAffine>(true);
    product_backend_faults::<EpAffine>(true);
}
#[test]
fn both_pasta_products_destroy_partial_outputs_on_backend_refusal_encoding_capacity_and_unwind() {
    product_backend_faults::<EqAffine>(false);
    product_backend_faults::<EpAffine>(false);
}

fn product_boundary_faults<C>()
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
{
    const Q: bool = false;
    const M: u64 = 0;
    let params = ParamsIPA::<C>::new(4);
    let pk = product_key::<C, true, true>(&params);
    let b = pk.vk.cs.blinding_factors();
    let usable = 16 - b - 1;
    let sets = pk
        .vk
        .cs
        .permutation
        .columns
        .len()
        .div_ceil(pk.vk.cs_degree - 2);
    let total = sets + 2;
    let values = fixture_values::<C::Scalar>(usable, false);
    let instances = values.iter().map(Vec::as_slice).collect::<Vec<_>>();
    // Every challenge boundary, all commitment write errors/unwinds, sample boundaries in
    // copy/lookup tails and blinds, and successful external calls that substitute a receipt.
    for case in 0..(15 + 2 * total) {
        let shared = Shared::<C>::new();
        let controls = ProductControls::new();
        let permuted = product_member!(
            &params,
            pk.clone(),
            &instances,
            &shared,
            &controls,
            true,
            true,
            Q,
            M
        )
        .commit_permuted_lookups(1 << 20)
        .unwrap();
        let source = permuted
            .compressed
            .inner
            .advice
            .layouts()
            .unwrap()
            .next()
            .unwrap()
            .ordinal();
        let remaining = permuted.lookups[1].table.coefficient.layout.ordinal();
        let current = permuted.lookups[0].input.lagrange.layout.ordinal();
        let first = permuted.compressed.inner.provider.inner.inner.inner.ordinal;
        let (events, draws) = {
            let l = shared.log.lock().unwrap();
            (l.events.len(), l.rng_calls)
        };
        let mut sampled = CountedRng {
            inner: shared.rng.lock().unwrap().clone(),
            calls: 0,
        };
        let mut sample_starts = Vec::new();
        for _ in 0..total * (b + 1) {
            sample_starts.push(draws + sampled.calls);
            let _ = C::Scalar::random(&mut sampled);
        }
        let mut panic = false;
        if case < 2 {
            panic = true;
            *controls.fault.lock().unwrap() = ProductFault::SqueezePanic(events + case);
        } else if case < 4 {
            *controls.fault.lock().unwrap() = ProductFault::SqueezeDrift(
                events + case - 2,
                if case == 2 { source } else { remaining },
            );
        } else if case < 8 {
            panic = true;
            let sample = [0, b, sets * (b + 1), (total - 1) * (b + 1) + b][case - 4];
            *controls.base.boundary.lock().unwrap() = BoundaryFault::Rng(sample_starts[sample]);
        } else if case < 11 {
            let sample = [0, b, sets * (b + 1)][case - 8];
            *controls.fault.lock().unwrap() = ProductFault::RngDrift(
                sample_starts[sample],
                if case == 10 { first } else { remaining },
            );
        } else if case < 11 + total {
            shared.log.lock().unwrap().fault = Some(Fault::Transcript(events + 2 + case - 11));
        } else if case < 11 + 2 * total {
            panic = true;
            *controls.base.boundary.lock().unwrap() =
                BoundaryFault::Transcript(events + 2 + case - 11 - total);
        } else {
            let (index, victim) = [
                (0, remaining),
                (sets, current),
                (total - 1, remaining),
                (total - 1, first),
            ][case - 11 - 2 * total];
            *controls.fault.lock().unwrap() = ProductFault::PointDrift(events + 2 + index, victim);
        }
        take_product_clears();
        let result = catch_unwind(AssertUnwindSafe(|| {
            permuted
                .commit_products(
                    product_scratch_bytes::<C, PermutedSnapshot<C>, ProductWriter<C>>(4, sets, 2)
                        .unwrap(),
                )
                .map(|_| ())
        }));
        if panic {
            assert!(result.is_err(), "unreached external unwind {case}");
        } else {
            assert!(
                result.unwrap().is_err(),
                "external receipt substitution escaped {case}"
            );
        }
        assert!(!controls.base.window.load(Ordering::SeqCst));
        assert_dropped(&shared);
        let log = shared.log.lock().unwrap();
        if case < 4 {
            assert_eq!(log.rng_calls, draws);
        }
        if case < 2 {
            assert_eq!(log.events.len(), events + case);
        }
        if (11..11 + total).contains(&case) {
            assert_eq!(log.events.len(), events + 2 + case - 11);
        }
        if (11 + total..11 + 2 * total).contains(&case) {
            assert_eq!(log.events.len(), events + 2 + case - 11 - total);
        }
        drop(log);
        let (_, fz, _, ez, _, bz) = take_product_clears();
        assert!(fz && ez && bz, "external case {case}");
    }
}
#[test]
fn both_pasta_product_challenges_randomness_and_each_transcript_write_fail_closed() {
    product_boundary_faults::<EqAffine>();
    product_boundary_faults::<EpAffine>();
}

fn product_preflight<C>()
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
{
    const Q: bool = false;
    const M: u64 = 0;
    let params = ParamsIPA::<C>::new(4);
    let other_params = ParamsIPA::<C>::new(5);
    let pk = product_key::<C, true, true>(&params);
    let usable = 16 - pk.vk.cs.blinding_factors() - 1;
    let sets = pk
        .vk
        .cs
        .permutation
        .columns
        .len()
        .div_ceil(pk.vk.cs_degree - 2);
    let values = fixture_values::<C::Scalar>(usable, false);
    let instances = values.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let budget =
        product_scratch_bytes::<C, PermutedSnapshot<C>, ProductWriter<C>>(4, sets, 2).unwrap();
    assert!(budget >= ((16 + 8 + 4 * 256 + 1) * std::mem::size_of::<C::Scalar>() + 256 * 32));
    assert!(product_scratch_bytes::<C, PermutedSnapshot<C>, ProductWriter<C>>(33, 1, 1).is_err());
    assert!(
        product_scratch_bytes::<C, PermutedSnapshot<C>, ProductWriter<C>>(4, usize::MAX, 1)
            .is_err()
    );
    for case in 0..16 {
        let shared = Shared::<C>::new();
        let controls = ProductControls::new();
        let mut permuted = product_member!(
            &params,
            pk.clone(),
            &instances,
            &shared,
            &controls,
            true,
            true,
            Q,
            M
        )
        .commit_permuted_lookups(1 << 20)
        .unwrap();
        let mut actual_budget = budget;
        match case {
            0 => actual_budget = 0,
            1 => actual_budget = budget - 1,
            2 => permuted.usable_rows += 1,
            3 => permuted.compressed.inner.params = &other_params,
            4 => permuted.compressed.inner.pk.vk.cs_degree = 2,
            5 => {
                permuted.lookups.pop();
            }
            6 => {
                permuted.compressed.lookups.pop();
            }
            7 => permuted
                .compressed
                .inner
                .pk
                .fixed_values
                .pop()
                .map(drop)
                .unwrap(),
            8 => permuted
                .compressed
                .inner
                .pk
                .permutation
                .permutations
                .pop()
                .map(drop)
                .unwrap(),
            9 => {
                permuted.compressed.inner.pk.permutation.permutations[0]
                    .values
                    .pop();
            }
            10 => {
                permuted.compressed.inner.pk.vk.cs.permutation.columns[0] =
                    Column::<Instance>::new(99, Instance).into();
            }
            11 => {
                permuted.compressed.inner.instances = &[];
            }
            12 => {
                let old = permuted.lookups[0].input.lagrange.layout;
                permuted.lookups[0].input.coefficient.layout = old;
            }
            13 => {
                let old = permuted.compressed.lookups[0].input.layout;
                permuted.compressed.lookups[0].table.layout = old;
            }
            14 => {
                permuted.compressed.inner.provider.inner.inner.inner.ordinal = u64::MAX - 1;
            }
            15 => {
                let first = permuted
                    .compressed
                    .inner
                    .advice
                    .layouts()
                    .unwrap()
                    .next()
                    .unwrap()
                    .ordinal();
                override_receipt(&shared, first);
            }
            _ => unreachable!(),
        }
        let (created, reads, writes, draws, events) = {
            let log = shared.log.lock().unwrap();
            (
                log.created,
                log.reads,
                log.writes,
                log.rng_calls,
                log.events.clone(),
            )
        };
        take_product_clears();
        assert!(
            permuted.commit_products(actual_budget).is_err(),
            "preflight {case}"
        );
        let log = shared.log.lock().unwrap();
        assert_eq!(log.reads, reads);
        assert_eq!(log.writes, writes);
        assert_eq!(log.rng_calls, draws);
        assert_eq!(log.events, events);
        // The real provider's returned high-water mark is observed through one factory.
        assert_eq!(log.created, created + usize::from(case == 14));
        drop(log);
        assert_dropped(&shared);
        let (_, fz, _, ez, _, bz) = take_product_clears();
        assert!(fz && ez && bz);
    }
}
#[test]
fn both_pasta_product_budget_key_geometry_inventory_and_ordinal_preflights_do_no_witness_io() {
    product_preflight::<EqAffine>();
    product_preflight::<EpAffine>();
}

fn product_empty<C>()
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
{
    let params = ParamsIPA::<C>::new(4);
    let pk = membership_key::<C, false, true>(&params);
    let values = [
        vec![C::Scalar::ZERO],
        vec![C::Scalar::ZERO],
        vec![C::Scalar::ZERO],
        vec![C::Scalar::ZERO],
    ];
    let instances = values.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let shared = Shared::<C>::new();
    let permuted =
        prepare_single_phase_stored_ipa_prefix_v1::<C, _, _, _, Challenge255<C>, _, false, 0>(
            &params,
            pk,
            MembershipCircuit::<C, false, true>(EmptyLookupProducer(Producer::new(&shared, 0))),
            &instances,
            Provider::new(&shared),
            Rng(Arc::clone(&shared)),
            RecordingTranscript::new(&shared),
        )
        .unwrap()
        .stage_advice_coefficients()
        .unwrap()
        .compress_lookups(0)
        .unwrap()
        .sort_lookup_values(0)
        .unwrap()
        .prepare_lookup_membership(0)
        .unwrap()
        .commit_permuted_lookups(0)
        .unwrap();
    assert_eq!(
        product_scratch_bytes::<C, Snapshot<C>, Writer<C>>(4, 0, 0).unwrap(),
        0
    );
    let (events, reads, writes, created, draws) = {
        let log = shared.log.lock().unwrap();
        (
            log.events.clone(),
            log.reads,
            log.writes,
            log.created,
            log.rng_calls,
        )
    };
    let ordinary = Shared::<C>::new();
    let mut transcript = RecordingTranscript::new(&ordinary);
    replay_prefix(&events, &mut transcript);
    let beta = transcript.squeeze_challenge().get_scalar();
    let gamma = transcript.squeeze_challenge().get_scalar();
    take_product_clears();
    let actual = permuted.commit_products(0).unwrap();
    assert_eq!(*actual.beta, beta);
    assert_eq!(*actual.gamma, gamma);
    assert!(actual.permutations.is_empty() && actual.lookups.is_empty());
    assert_eq!(actual.inner.advice.layouts().unwrap().len(), 0);
    assert!(actual.inner.pk.fixed_values.is_empty());
    assert!(actual.inner.pk.permutation.permutations.is_empty());
    let log = shared.log.lock().unwrap();
    assert_eq!(log.events, ordinary.log.lock().unwrap().events);
    assert_eq!(
        (log.reads, log.writes, log.created, log.rng_calls),
        (reads, writes, created, draws)
    );
    drop(log);
    assert_eq!(take_product_clears(), (0, true, 0, true, 0, true));
    drop(actual);
    assert_dropped(&shared);
}
#[test]
fn zero_advice_zero_product_owner_squeezes_only_beta_gamma_with_zero_budget_and_no_storage() {
    product_empty::<EqAffine>();
    product_empty::<EpAffine>();
}

#[test]
fn copy_and_lookup_product_roles_have_distinct_fixed_tag5_tag6_grammar_and_reject_advice() {
    use blake2b_simd::Params;
    let mut digests = std::collections::BTreeSet::new();
    for field in [StoredPastaFieldV1::Fp, StoredPastaFieldV1::Fq] {
        for k in [4_u32, 9] {
            for index in [0_u32, 1] {
                for (tag, role) in [
                    (
                        5,
                        StoredPolynomialRoleV1::CopyPermutationProduct { set: index },
                    ),
                    (6, StoredPolynomialRoleV1::LookupProduct { lookup: index }),
                ] {
                    let layout = StoredPolynomialLayoutV1::new(
                        [23; 32],
                        57,
                        field,
                        StoredPolynomialBasisV1::Coefficient,
                        k,
                        role,
                    )
                    .unwrap();
                    let mut hash = Params::new()
                        .hash_length(32)
                        .personal(b"Halo2PolyStoreV1")
                        .to_state();
                    hash.update(b"polynomial.snapshot.v1\0canonical-primefield-repr\0zero-tail\0");
                    hash.update(&[23; 32]);
                    hash.update(&57_u64.to_le_bytes());
                    hash.update(&[match field {
                        StoredPastaFieldV1::Fp => 0,
                        StoredPastaFieldV1::Fq => 1,
                    }]);
                    hash.update(&[1]);
                    hash.update(&0_u32.to_le_bytes());
                    hash.update(&0_u32.to_le_bytes());
                    hash.update(&k.to_le_bytes());
                    hash.update(&[tag]);
                    hash.update(&index.to_le_bytes());
                    hash.update(&[0]);
                    hash.update(&(1_u64 << k).to_le_bytes());
                    hash.update(&256_u64.to_le_bytes());
                    hash.update(&32_u64.to_le_bytes());
                    assert_eq!(
                        layout.context_digest().as_slice(),
                        hash.finalize().as_bytes()
                    );
                    assert!(digests.insert(layout.context_digest()));
                    assert_eq!(
                        layout.advice_coordinates(),
                        Err(StoredPolynomialErrorV1::Context)
                    );
                    assert!(
                        StoredPolynomialLayoutV1::new(
                            [0; 32],
                            57,
                            field,
                            StoredPolynomialBasisV1::Coefficient,
                            k,
                            role
                        )
                        .is_err()
                    );
                    assert_ne!(
                        layout.context_digest(),
                        StoredPolynomialLayoutV1::new(
                            [23; 32],
                            58,
                            field,
                            StoredPolynomialBasisV1::Coefficient,
                            k,
                            role
                        )
                        .unwrap()
                        .context_digest()
                    );
                }
            }
        }
    }
    assert_eq!(digests.len(), 16);
}

#[path = "vanishing_tests.rs"]
mod vanishing;
