//! Independent ordinary trajectory and last-use retirement regressions through typed y.
//!
//! Monitored plaintext banks exercise ownership and fault boundaries, not encrypted storage,
//! full stored proofs, allocator RSS, physical devices, or upstream MSM scratch erasure.

use super::*;
use crate::plonk::prover::stored::vanishing::{
    scratch_bytes as vanishing_scratch_bytes, take_clear_observations as take_vanishing_clears,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum VanishingFault {
    None,
    Create(u64, ProductChange),
    WriterSecond(u64),
    WriterAfterWrite(u64),
    DropDrift(u64, u64),
    DropPanic(u64),
}
#[derive(Clone)]
struct VanishingControls<C: CurveAffine> {
    product: ProductControls<C>,
    fault: Arc<Mutex<VanishingFault>>,
    drops: Arc<Mutex<Vec<(StoredPolynomialLayoutV1, usize, usize)>>>,
    peaks: Arc<Mutex<Vec<usize>>>,
}
impl<C: CurveAffine> VanishingControls<C> {
    fn new() -> Self {
        Self {
            product: ProductControls::new(),
            fault: Arc::new(Mutex::new(VanishingFault::None)),
            drops: Arc::new(Mutex::new(Vec::new())),
            peaks: Arc::new(Mutex::new(Vec::new())),
        }
    }
}
struct VanishingProvider<C: CurveAffine> {
    inner: ProductProvider<C>,
    controls: VanishingControls<C>,
}
struct VanishingWriter<C: CurveAffine> {
    inner: ProductWriter<C>,
    controls: VanishingControls<C>,
    observations: std::cell::Cell<usize>,
}
struct VanishingSnapshot<C: CurveAffine> {
    inner: PermutedSnapshot<C>,
    controls: VanishingControls<C>,
}
fn vanishing_change(
    layout: StoredPolynomialLayoutV1,
    change: ProductChange,
) -> StoredPolynomialLayoutV1 {
    let new_role = match (change, layout.role()) {
        (ProductChange::Index, StoredPolynomialRoleV1::Instance { column }) => {
            StoredPolynomialRoleV1::Instance { column: column + 1 }
        }
        (
            ProductChange::Index | ProductChange::WrongRole,
            StoredPolynomialRoleV1::VanishingRandom,
        ) => StoredPolynomialRoleV1::Instance { column: 0 },
        (ProductChange::WrongRole, _) => StoredPolynomialRoleV1::VanishingRandom,
        _ => layout.role(),
    };
    let altered = product_change(layout, change);
    StoredPolynomialLayoutV1::new(
        if change == ProductChange::Context {
            [41; 32]
        } else {
            [23; 32]
        },
        altered.ordinal(),
        altered.field(),
        altered.basis(),
        altered.k(),
        new_role,
    )
    .unwrap()
}
impl<C: CurveAffine> StoredPolynomialProviderV1 for VanishingProvider<C> {
    type Writer = VanishingWriter<C>;
    fn create(
        &mut self,
        field: StoredPastaFieldV1,
        basis: StoredPolynomialBasisV1,
        k: u32,
        role: StoredPolynomialRoleV1,
    ) -> Result<Self::Writer, StoredPolynomialErrorV1> {
        let mut inner = self.inner.create(field, basis, k, role)?;
        let ordinal = inner.layout().ordinal();
        if let VanishingFault::Create(target, change) = *self.controls.fault.lock().unwrap() {
            if ordinal == target {
                inner.inner.inner.inner.layout =
                    vanishing_change(inner.inner.inner.inner.layout, change);
            }
        }
        let shared = &inner.inner.inner.inner.shared;
        let log = shared.log.lock().unwrap();
        self.controls
            .peaks
            .lock()
            .unwrap()
            .push(log.sealed.len() - log.snapshot_drops + log.created - log.writer_drops);
        drop(log);
        Ok(VanishingWriter {
            inner,
            controls: self.controls.clone(),
            observations: std::cell::Cell::new(0),
        })
    }
}
impl<C: CurveAffine> StoredPolynomialWriterV1 for VanishingWriter<C> {
    type Snapshot = VanishingSnapshot<C>;
    fn layout(&self) -> StoredPolynomialLayoutV1 {
        let layout = self.inner.layout();
        let calls = self.observations.get();
        self.observations.set(calls + 1);
        let fault = *self.controls.fault.lock().unwrap();
        if (fault == VanishingFault::WriterSecond(layout.ordinal()) && calls > 0)
            || (fault == VanishingFault::WriterAfterWrite(layout.ordinal())
                && self.inner.inner.inner.inner.next > 0)
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
        Ok(VanishingSnapshot {
            inner: self.inner.seal()?,
            controls: self.controls,
        })
    }
}
impl<C: CurveAffine> StoredPolynomialSnapshotV1 for VanishingSnapshot<C> {
    fn layout(&self) -> StoredPolynomialLayoutV1 {
        self.inner.layout()
    }
    fn with_chunk<V>(
        &mut self,
        expected: StoredPolynomialLayoutV1,
        chunk: u64,
        consume: impl FnOnce(&[[u8; 32]]) -> Result<V, StoredPolynomialErrorV1>,
    ) -> Result<V, StoredPolynomialErrorV1> {
        self.inner.with_chunk(expected, chunk, consume)
    }
    fn with_column<V>(
        &mut self,
        _: StoredPolynomialLayoutV1,
        _: impl FnOnce(&[[u8; 32]]) -> Result<V, StoredPolynomialErrorV1>,
    ) -> Result<V, StoredPolynomialErrorV1> {
        panic!("vanishing requested backend column materialization")
    }
}
impl<C: CurveAffine> Drop for VanishingSnapshot<C> {
    fn drop(&mut self) {
        // Read immutable fixture identity directly: layout fault callbacks are not cleanup.
        let original = self.inner.inner.inner.layout;
        let shared = &self.inner.inner.inner.shared;
        let log = shared.log.lock().unwrap();
        self.controls
            .drops
            .lock()
            .unwrap()
            .push((original, log.events.len(), log.rng_calls));
        drop(log);
        let fault = *self.controls.fault.lock().unwrap();
        if let VanishingFault::DropDrift(trigger, victim) = fault {
            if trigger == original.ordinal() {
                *self.controls.fault.lock().unwrap() = VanishingFault::None;
                override_receipt(shared, victim);
            }
        }
        if fault == VanishingFault::DropPanic(original.ordinal()) {
            // One shot: no second panic while the consumed owner unwinds.
            *self.controls.fault.lock().unwrap() = VanishingFault::None;
            panic!("injected last-use snapshot destructor unwind");
        }
    }
}
fn vanishing_provider<C: CurveAffine>(
    shared: &Arc<Shared<C>>,
    controls: &VanishingControls<C>,
) -> VanishingProvider<C> {
    VanishingProvider {
        inner: ProductProvider {
            inner: PermutedProvider {
                inner: MemberProvider {
                    inner: Provider::new(shared),
                    fault: Arc::clone(&controls.product.base.backend),
                    window: Arc::clone(&controls.product.base.window),
                },
                controls: controls.product.base.clone(),
            },
            controls: controls.product.clone(),
        },
        controls: controls.clone(),
    }
}
macro_rules! vanishing_member {
    ($params:expr,$pk:expr,$instances:expr,$shared:expr,$controls:expr,$copy:tt,$lookups:tt,$q:ident,$mask:ident) => {
        prepare_single_phase_stored_ipa_prefix_v1::<C, _, _, _, ProductChallenge<C>, _, $q, $mask>(
            $params,
            $pk,
            ProductCircuit::<C, $copy, $lookups>(Producer::new($shared, 3)),
            $instances,
            vanishing_provider($shared, $controls),
            ProductRng {
                inner: BoundaryRng {
                    inner: Rng(Arc::clone($shared)),
                    controls: $controls.product.base.clone(),
                },
                controls: $controls.product.clone(),
            },
            ProductTranscript::new($shared, &$controls.product),
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
fn sealed_values<C: CurveAffine>(
    shared: &Arc<Shared<C>>,
    layout: StoredPolynomialLayoutV1,
) -> Vec<C::Scalar>
where
    C::Scalar: StoredAssignmentFieldV1,
{
    shared
        .log
        .lock()
        .unwrap()
        .sealed
        .iter()
        .find(|(l, _)| *l == layout)
        .unwrap()
        .1
        .iter()
        .map(|v| Option::<C::Scalar>::from(C::Scalar::from_repr(*v)).unwrap())
        .collect()
}
fn assert_vanishing_survivor<C: CurveAffine>(
    shared: &Arc<Shared<C>>,
    survivor: &mut VanishingSnapshot<C>,
    expected: &[[u8; 32]],
) {
    assert_owned_cleanup_with_survivor(shared, &mut survivor.inner, expected);
}

fn ordinary_vanishing<C, const COPY: bool, const LOOKUPS: bool, const Q: bool, const M: u64>(
    k: u32,
    empty_prefixes: bool,
) where
    C: CurveAffine + crate::SerdeCurveAffine,
    C::Scalar: StoredAssignmentFieldV1
        + WithSmallOrderMulGroup<3>
        + FromUniformBytes<64>
        + crate::SerdePrimeField,
{
    use crate::plonk::{
        lookup::prover::stored_products_ordinary_oracle,
        vanishing::stored_vanishing_ordinary_oracle,
    };
    let params = ParamsIPA::<C>::new(k);
    let pk = product_key::<C, COPY, LOOKUPS>(&params);
    let n = 1_usize << k;
    let usable = n - (pk.vk.cs.blinding_factors() + 1);
    let values = if empty_prefixes {
        std::array::from_fn(|_| Vec::new())
    } else {
        fixture_values::<C::Scalar>(usable, false)
    };
    let instances = values.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let shared = Shared::<C>::new();
    let controls = VanishingControls::new();
    let member = vanishing_member!(
        &params, pk, &instances, &shared, &controls, COPY, LOOKUPS, Q, M
    );
    let advice_layouts = member
        .compressed
        .inner
        .advice
        .layouts()
        .unwrap()
        .collect::<Vec<_>>();
    let advice = advice_layouts
        .iter()
        .map(|layout| {
            member
                .compressed
                .inner
                .pk
                .vk
                .domain
                .lagrange_from_vec(sealed_values(&shared, *layout))
        })
        .collect::<Vec<_>>();
    let challenges = member
        .compressed
        .inner
        .advice
        .challenges()
        .unwrap()
        .collect::<Vec<_>>();
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
    let expected_advice = advice
        .iter()
        .map(|p| {
            member
                .compressed
                .inner
                .pk
                .vk
                .domain
                .lagrange_to_coeff(p.clone())
                .to_vec()
        })
        .collect::<Vec<_>>();
    let expected_instances = dense_instances
        .iter()
        .map(|p| {
            member
                .compressed
                .inner
                .pk
                .vk
                .domain
                .lagrange_to_coeff(p.clone())
                .to_vec()
        })
        .collect::<Vec<_>>();
    let oracle_shared = Shared::<C>::new();
    let oracle_controls = ProductControls::new();
    let mut oracle_transcript = ProductTranscript::new(&oracle_shared, &oracle_controls);
    replay_product_prefix(&shared.log.lock().unwrap().events, &mut oracle_transcript);
    // Continue the true original RNG before pairs; no reseed or reconstructed product algorithm.
    let mut oracle_rng = shared.rng.lock().unwrap().clone();
    let ordinary_products = stored_products_ordinary_oracle(
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
    let ordinary = stored_vanishing_ordinary_oracle(
        &params,
        &member.compressed.inner.pk.vk.domain,
        &mut oracle_rng,
        &mut oracle_transcript,
    )
    .unwrap();
    let products = member
        .commit_permuted_lookups(1 << 20)
        .unwrap()
        .commit_products(1 << 20)
        .unwrap();
    let sets = products.permutations.len();
    let lookups = products.lookups.len();
    let before_point = shared.log.lock().unwrap().events.len();
    let first = products.inner.provider.inner.inner.inner.inner.ordinal;
    let masks = [
        products.inner.pk.l0.to_vec(),
        products.inner.pk.l_last.to_vec(),
        products.inner.pk.l_active_row.to_vec(),
    ];
    let mask_ptrs = [
        products.inner.pk.l0.values.as_ptr(),
        products.inner.pk.l_last.values.as_ptr(),
        products.inner.pk.l_active_row.values.as_ptr(),
    ];
    let evaluator = format!("{:?}", products.inner.pk.ev);
    let fixed = products.inner.pk.fixed_polys.as_ptr();
    let sigma = products.inner.pk.permutation.polys.as_ptr();
    let vk = products
        .inner
        .pk
        .get_vk()
        .to_bytes(crate::SerdeFormat::Processed);
    let theta = *products.theta;
    let before_reads = shared.log.lock().unwrap().reads;
    let entry = 2 * advice_layouts.len() + 3 * lookups + sets;
    {
        let log = shared.log.lock().unwrap();
        assert_eq!(log.sealed.len() - log.snapshot_drops, entry);
    }
    controls.peaks.lock().unwrap().clear();
    controls.drops.lock().unwrap().clear();
    *controls.product.base.backend.lock().unwrap() =
        MemberFault::Capacity(entry + instances.len() + 1);
    take_vanishing_clears();
    let mut actual = products
        .commit_vanishing_and_stage_coefficients(1 << 20)
        .unwrap();
    assert!(std::ptr::eq(actual.params, &params));
    assert_eq!(actual.instances.as_ptr(), instances.as_ptr());
    assert_eq!(
        [
            actual.pk.l0.to_vec(),
            actual.pk.l_last.to_vec(),
            actual.pk.l_active_row.to_vec()
        ],
        masks
    );
    assert_eq!(
        [
            actual.pk.l0.values.as_ptr(),
            actual.pk.l_last.values.as_ptr(),
            actual.pk.l_active_row.values.as_ptr()
        ],
        mask_ptrs
    );
    assert_eq!(format!("{:?}", actual.pk.ev), evaluator);
    assert_eq!(actual.pk.fixed_polys.as_ptr(), fixed);
    assert_eq!(actual.pk.permutation.polys.as_ptr(), sigma);
    assert!(actual.pk.fixed_values.is_empty() && actual.pk.permutation.permutations.is_empty());
    assert_eq!(
        actual.pk.get_vk().to_bytes(crate::SerdeFormat::Processed),
        vk
    );
    assert_eq!(
        (*actual.theta, *actual.beta, *actual.gamma, *actual.y),
        (
            theta,
            ordinary_products.beta,
            ordinary_products.gamma,
            ordinary.y
        )
    );
    assert_eq!(actual.usable_rows, usable);
    assert_eq!(
        actual.advice.challenges().unwrap().collect::<Vec<_>>(),
        challenges
    );
    assert_eq!(actual.advice.proof_context().unwrap(), Some([23; 32]));
    assert!(std::ptr::eq(actual.advice.params().unwrap(), &params));
    let coefficient_layouts = actual.advice.layouts().unwrap().collect::<Vec<_>>();
    assert_eq!(coefficient_layouts.len(), expected_advice.len());
    for (layout, expected) in coefficient_layouts.iter().zip(expected_advice) {
        assert_eq!(layout.basis(), StoredPolynomialBasisV1::Coefficient);
        assert_eq!(sealed_values(&shared, *layout), expected);
    }
    assert_eq!(actual.instance_coefficients.len(), instances.len());
    for (column, (actual, expected)) in actual
        .instance_coefficients
        .iter_mut()
        .zip(expected_instances)
        .enumerate()
    {
        assert_eq!(
            actual.layout.role(),
            StoredPolynomialRoleV1::Instance {
                column: column as u32
            }
        );
        assert_eq!(actual.layout.ordinal(), first + column as u64);
        assert_eq!(actual.layout.basis(), StoredPolynomialBasisV1::Coefficient);
        assert_eq!(
            read_compressed::<C, _>(&mut actual.snapshot, actual.layout),
            expected
        );
    }
    assert_eq!(
        actual.random.coefficient.layout.role(),
        StoredPolynomialRoleV1::VanishingRandom
    );
    assert_eq!(
        actual.random.coefficient.layout.ordinal(),
        first + instances.len() as u64
    );
    assert_eq!(
        read_compressed::<C, _>(
            &mut actual.random.coefficient.snapshot,
            actual.random.coefficient.layout
        ),
        ordinary.random_coefficient
    );
    assert_eq!((actual.random.blind.0).0, ordinary.random_blind.0);
    assert_eq!(
        actual.random.commitment,
        params
            .commit(
                &actual
                    .pk
                    .vk
                    .domain
                    .coeff_from_vec(ordinary.random_coefficient),
                ordinary.random_blind
            )
            .to_affine()
    );
    let domain = &actual.pk.vk.domain;
    for (actual, expected) in actual
        .permutations
        .iter_mut()
        .zip(&ordinary_products.permutations)
    {
        assert_eq!(
            read_compressed::<C, _>(&mut actual.coefficient.snapshot, actual.coefficient.layout),
            expected.coefficient
        );
        assert_eq!((actual.blind.0).0, expected.blind.0);
        assert_eq!(
            actual.commitment,
            params
                .commit(
                    &domain.coeff_from_vec(expected.coefficient.clone()),
                    expected.blind
                )
                .to_affine()
        );
    }
    for (actual, expected) in actual.lookups.iter_mut().zip(&ordinary_products.lookups) {
        for (actual, expected, blind) in [
            (
                &mut actual.input,
                &expected.input_coefficient,
                expected.input_blind,
            ),
            (
                &mut actual.table,
                &expected.table_coefficient,
                expected.table_blind,
            ),
            (
                &mut actual.product,
                &expected.product_coefficient,
                expected.product_blind,
            ),
        ] {
            assert_eq!(
                read_compressed::<C, _>(
                    &mut actual.coefficient.snapshot,
                    actual.coefficient.layout
                ),
                *expected
            );
            assert_eq!((actual.blind.0).0, blind.0);
            assert_eq!(
                actual.commitment,
                params
                    .commit(&domain.coeff_from_vec(expected.clone()), blind)
                    .to_affine()
            );
        }
    }
    let expected_events = oracle_shared.log.lock().unwrap().events.clone();
    assert_eq!(shared.log.lock().unwrap().events, expected_events);
    assert_eq!(
        expected_events[before_point],
        Event::WritePoint(actual.random.commitment)
    );
    assert_eq!(
        expected_events[before_point + 1],
        Event::Challenge(ordinary.y)
    );
    assert_eq!(expected_events.len(), before_point + 2);
    assert_eq!(
        controls.peaks.lock().unwrap().iter().max().copied(),
        Some(entry + instances.len() + 1)
    );
    let drops = controls.drops.lock().unwrap().clone();
    assert_eq!(drops.len(), advice_layouts.len());
    for (layout, events, _) in drops {
        assert!(advice_layouts.contains(&layout));
        assert_eq!(
            events,
            before_point + 2,
            "advice retired before y or an output was retired"
        );
    }
    assert!(Arc::ptr_eq(
        &actual.provider.inner.inner.inner.inner.shared,
        &shared
    ));
    assert!(Arc::ptr_eq(&actual.rng.inner.inner.0, &shared));
    assert!(Arc::ptr_eq(&actual.transcript.inner.inner.shared, &shared));
    // Stage performs no backend reads; reads above are explicitly test-owned inspections.
    let inspected_chunks = (instances.len() + 1 + sets + 3 * lookups) * n.div_ceil(256);
    assert_eq!(
        shared.log.lock().unwrap().reads,
        before_reads + inspected_chunks
    );
    {
        let log = shared.log.lock().unwrap();
        assert_eq!(
            log.sealed.len() - log.snapshot_drops,
            advice_layouts.len() + instances.len() + 3 * lookups + sets + 1
        );
    }
    assert_eq!(
        actual.transcript.inner.inner.inner.clone().finalize(),
        oracle_transcript.inner.inner.inner.clone().finalize()
    );
    assert_eq!(
        actual.transcript.squeeze_challenge().get_scalar(),
        oracle_transcript.squeeze_challenge().get_scalar()
    );
    let mut next_a = [0; 64];
    let mut next_b = [0; 64];
    actual.rng.fill_bytes(&mut next_a);
    oracle_rng.fill_bytes(&mut next_b);
    assert_eq!(next_a, next_b);
    drop(actual);
    assert_dropped(&shared);
    let (fields, fields_zero, bytes, bytes_zero, blinds, blinds_zero) = take_vanishing_clears();
    assert!(fields >= n && bytes >= 256 && blinds >= 1 && fields_zero && bytes_zero && blinds_zero);
}
fn vanishing_matrix<C>()
where
    C: CurveAffine + crate::SerdeCurveAffine,
    C::Scalar: StoredAssignmentFieldV1
        + WithSmallOrderMulGroup<3>
        + FromUniformBytes<64>
        + crate::SerdePrimeField,
{
    for k in [4, 8, 9] {
        ordinary_vanishing::<C, true, true, false, 0>(k, false);
        ordinary_vanishing::<C, true, true, true, 0>(k, false);
        ordinary_vanishing::<C, true, true, true, 6>(k, false);
    }
    ordinary_vanishing::<C, false, false, false, 0>(4, true);
    ordinary_vanishing::<C, false, true, true, 0>(4, true);
    ordinary_vanishing::<C, true, false, true, 6>(9, false);
}
#[test]
fn both_pasta_vanishing_and_instance_coefficients_match_actual_ordinary_trajectory_through_y() {
    vanishing_matrix::<EqAffine>();
    vanishing_matrix::<EpAffine>();
}

macro_rules! vanishing_products {
    ($params:expr,$pk:expr,$instances:expr,$shared:expr,$controls:expr) => {
        vanishing_member!(
            $params, $pk, $instances, $shared, $controls, true, true, Q, M
        )
        .commit_permuted_lookups(1 << 20)
        .unwrap()
        .commit_products(1 << 20)
        .unwrap()
    };
}
fn vanishing_failures<C>(metadata: bool)
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
{
    const Q: bool = false;
    const M: u64 = 0;
    let params = ParamsIPA::<C>::new(9);
    let pk = product_key::<C, true, true>(&params);
    let values = fixture_values::<C::Scalar>(512 - (pk.vk.cs.blinding_factors() + 1), false);
    let instances = values.iter().map(Vec::as_slice).collect::<Vec<_>>();
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
    // 20 created layouts +4 writer changes +17 old receipts +11 seal changes +6 last-use drops.
    for case in 0..if metadata { 58 } else { 16 } {
        let shared = Shared::<C>::new();
        let controls = VanishingControls::new();
        let mut products = vanishing_products!(&params, pk.clone(), &instances, &shared, &controls);
        let advice = products.inner.advice.layouts().unwrap().collect::<Vec<_>>();
        let coefficients = products
            .inner
            .advice
            .coefficient_layouts()
            .unwrap()
            .collect::<Vec<_>>();
        let mut retained = advice
            .iter()
            .chain(&coefficients)
            .map(|l| l.ordinal())
            .collect::<Vec<_>>();
        retained.extend(
            products
                .permutations
                .iter()
                .map(|p| p.coefficient.layout.ordinal()),
        );
        for l in &products.lookups {
            retained.extend([
                l.input.coefficient.layout.ordinal(),
                l.table.coefficient.layout.ordinal(),
                l.product.coefficient.layout.ordinal(),
            ]);
        }
        assert_eq!(retained.len(), 17);
        let mut writer = products
            .inner
            .provider
            .create(
                C::Scalar::STORED_FIELD,
                StoredPolynomialBasisV1::Coefficient,
                9,
                StoredPolynomialRoleV1::Instance { column: 77 },
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
        let first = products.inner.provider.inner.inner.inner.inner.ordinal;
        let random = first + 4;
        let (events, draws, reads, writes, sealed, live) = {
            let log = shared.log.lock().unwrap();
            (
                log.events.clone(),
                log.rng_calls,
                log.reads,
                log.writes,
                log.sealed.clone(),
                log.sealed.len() - log.snapshot_drops,
            )
        };
        let mut panic = false;
        let mut before_random = false;
        if metadata {
            match case {
                0..20 => {
                    *controls.fault.lock().unwrap() = VanishingFault::Create(
                        if case < 10 || case == 17 {
                            first
                        } else {
                            random
                        },
                        changes[case % 10],
                    );
                    before_random = true;
                }
                20..24 => {
                    let ordinal = if case < 22 { first } else { random };
                    *controls.fault.lock().unwrap() = if case % 2 == 0 {
                        VanishingFault::WriterSecond(ordinal)
                    } else {
                        VanishingFault::WriterAfterWrite(ordinal)
                    };
                    before_random = case < 23;
                }
                24..41 => {
                    *controls.product.base.backend.lock().unwrap() =
                        MemberFault::SnapshotLayout(retained[case - 24]);
                    before_random = true;
                }
                41..46 => {
                    *controls.product.base.backend.lock().unwrap() = MemberFault::AfterSeal(
                        first + (case - 41) as u64,
                        retained[(case - 41) * 3],
                    );
                    before_random = case < 45;
                }
                46..50 => {
                    *controls.product.base.backend.lock().unwrap() =
                        MemberFault::AfterSeal(random, first + (case - 46) as u64);
                }
                50 => {
                    *controls.product.base.backend.lock().unwrap() =
                        MemberFault::AfterSeal(random, random)
                }
                51 => {
                    *controls.product.base.backend.lock().unwrap() =
                        MemberFault::AfterSeal(first + 3, first)
                }
                52..55 => {
                    *controls.fault.lock().unwrap() = VanishingFault::DropDrift(
                        advice[case - 52].ordinal(),
                        [
                            coefficients[2].ordinal(),
                            products.lookups[1].product.coefficient.layout.ordinal(),
                            random,
                        ][case - 52],
                    );
                }
                55..58 => {
                    *controls.fault.lock().unwrap() =
                        VanishingFault::DropPanic(advice[case - 55].ordinal());
                    panic = true;
                }
                _ => unreachable!(),
            }
        } else {
            *controls.product.base.backend.lock().unwrap() = match case {
                0 => {
                    before_random = true;
                    MemberFault::Create(first)
                }
                1 => {
                    before_random = true;
                    MemberFault::Create(first + 3)
                }
                2 => {
                    before_random = true;
                    MemberFault::Create(random)
                }
                3 => {
                    before_random = true;
                    MemberFault::Write(first, 1)
                }
                4 => {
                    before_random = true;
                    MemberFault::Write(first + 3, 1)
                }
                5 => MemberFault::Write(random, 1),
                6 => {
                    before_random = true;
                    MemberFault::Seal(first + 3)
                }
                7 => MemberFault::Seal(random),
                8 => {
                    panic = true;
                    before_random = true;
                    MemberFault::PanicWrite(first + 3, 1)
                }
                9 => {
                    panic = true;
                    MemberFault::PanicWrite(random, 1)
                }
                10 => {
                    panic = true;
                    before_random = true;
                    MemberFault::PanicSeal(first + 3)
                }
                11 => {
                    panic = true;
                    MemberFault::PanicSeal(random)
                }
                12 => {
                    before_random = true;
                    MemberFault::Capacity(live)
                }
                13 => {
                    before_random = true;
                    MemberFault::Capacity(live + 2)
                }
                14 => {
                    before_random = true;
                    MemberFault::Capacity(live + 4)
                }
                15 => {
                    before_random = true;
                    products.inner.provider.inner.inner.inner.inner.ordinal = u64::MAX - 1;
                    MemberFault::None
                }
                _ => unreachable!(),
            };
        }
        take_vanishing_clears();
        let result = catch_unwind(AssertUnwindSafe(|| {
            products
                .commit_vanishing_and_stage_coefficients(1 << 20)
                .map(|_| ())
        }));
        if panic {
            assert!(result.is_err(), "unreached unwind {case}");
        } else {
            assert!(
                result.unwrap().is_err(),
                "accepted metadata={metadata} case={case}"
            );
        }
        let log = shared.log.lock().unwrap();
        assert_eq!(log.reads, reads);
        assert_eq!(&log.sealed[..sealed.len()], &sealed);
        if before_random {
            assert_eq!(log.rng_calls, draws);
            assert_eq!(log.events, events);
        }
        if metadata && (24..41).contains(&case) {
            assert_eq!(log.writes, writes);
        }
        drop(log);
        assert!(!controls.product.base.window.load(Ordering::SeqCst));
        *controls.product.base.backend.lock().unwrap() = MemberFault::None;
        *controls.fault.lock().unwrap() = VanishingFault::None;
        assert_eq!(survivor.layout(), survivor_layout);
        assert_vanishing_survivor(&shared, &mut survivor, &expected);
        drop(survivor);
        assert_dropped(&shared);
        let (_, fz, _, ez, _, bz) = take_vanishing_clears();
        assert!(fz && ez && bz);
    }
}
#[test]
fn both_pasta_vanishing_rejects_all_receipt_classes_and_output_metadata_through_last_use_drops() {
    vanishing_failures::<EqAffine>(true);
    vanishing_failures::<EpAffine>(true);
}
#[test]
fn both_pasta_vanishing_partial_instance_random_writes_seals_capacity_and_unwinds_destroy_owner() {
    vanishing_failures::<EqAffine>(false);
    vanishing_failures::<EpAffine>(false);
}

fn vanishing_boundaries<C>()
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
{
    const Q: bool = false;
    const M: u64 = 0;
    let params = ParamsIPA::<C>::new(4);
    let pk = product_key::<C, true, true>(&params);
    let values = fixture_values::<C::Scalar>(16 - (pk.vk.cs.blinding_factors() + 1), false);
    let instances = values.iter().map(Vec::as_slice).collect::<Vec<_>>();
    // Every completed coefficient sample plus the blind, point error/unwind, y unwind,
    // then retained-receipt drift after first/middle/last/blind samples, point and y.
    for case in 0..44 {
        let shared = Shared::<C>::new();
        let controls = VanishingControls::new();
        let mut products = vanishing_products!(&params, pk.clone(), &instances, &shared, &controls);
        let advice = products.inner.advice.layouts().unwrap().collect::<Vec<_>>();
        let coefficients = products
            .inner
            .advice
            .coefficient_layouts()
            .unwrap()
            .collect::<Vec<_>>();
        let mut writer = products
            .inner
            .provider
            .create(
                C::Scalar::STORED_FIELD,
                StoredPolynomialBasisV1::Coefficient,
                4,
                StoredPolynomialRoleV1::Instance { column: 77 },
            )
            .unwrap();
        let sentinel_values = [C::Scalar::from(99).to_repr(); 16];
        writer.write_chunk(0, &sentinel_values).unwrap();
        let mut survivor = writer.seal().unwrap();
        let first = products.inner.provider.inner.inner.inner.inner.ordinal;
        let random = first + 4;
        let victims = [
            advice[0].ordinal(),
            coefficients[2].ordinal(),
            products.permutations[4].coefficient.layout.ordinal(),
            products.lookups[1].table.coefficient.layout.ordinal(),
            first + 3,
            random,
        ];
        let point = shared.log.lock().unwrap().events.len();
        let draws = shared.log.lock().unwrap().rng_calls;
        let reads = shared.log.lock().unwrap().reads;
        let mut count = CountedRng {
            inner: shared.rng.lock().unwrap().clone(),
            calls: 0,
        };
        let mut starts = Vec::new();
        for _ in 0..17 {
            starts.push(draws + count.calls);
            let _ = C::Scalar::random(&mut count);
        }
        let panic = case < 17 || case == 18 || case == 19;
        match case {
            0..17 => {
                *controls.product.base.boundary.lock().unwrap() = BoundaryFault::Rng(starts[case])
            }
            17 => shared.log.lock().unwrap().fault = Some(Fault::Transcript(point)),
            18 => {
                *controls.product.base.boundary.lock().unwrap() = BoundaryFault::Transcript(point)
            }
            19 => *controls.product.fault.lock().unwrap() = ProductFault::SqueezePanic(point + 1),
            20..32 => {
                let relative = case - 20;
                let sample = [0, 8, 15, 16][relative / 3];
                *controls.product.fault.lock().unwrap() =
                    ProductFault::RngDrift(starts[sample], victims[[0, 3, 4][relative % 3]]);
            }
            32..38 => {
                *controls.product.fault.lock().unwrap() =
                    ProductFault::PointDrift(point, victims[case - 32])
            }
            38..44 => {
                *controls.product.fault.lock().unwrap() =
                    ProductFault::SqueezeDrift(point + 1, victims[case - 38])
            }
            _ => unreachable!(),
        }
        controls.drops.lock().unwrap().clear();
        take_vanishing_clears();
        let result = catch_unwind(AssertUnwindSafe(|| {
            products
                .commit_vanishing_and_stage_coefficients(1 << 20)
                .map(|_| ())
        }));
        if panic {
            assert!(result.is_err(), "unreached boundary unwind {case}");
        } else {
            assert!(result.unwrap().is_err(), "accepted boundary {case}");
        }
        assert_eq!(shared.log.lock().unwrap().reads, reads);
        if case < 17 {
            assert_eq!(shared.log.lock().unwrap().rng_calls, starts[case]);
        }
        assert_vanishing_survivor(&shared, &mut survivor, &sentinel_values);
        drop(survivor);
        assert_dropped(&shared);
        let (_, fz, _, ez, _, bz) = take_vanishing_clears();
        assert!(fz && ez && bz);
    }
}
#[test]
fn both_pasta_vanishing_each_coefficient_blind_point_y_and_external_drift_boundary_fails_closed() {
    vanishing_boundaries::<EqAffine>();
    vanishing_boundaries::<EpAffine>();
}

fn vanishing_preflight<C>()
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
{
    const Q: bool = false;
    const M: u64 = 0;
    let params = ParamsIPA::<C>::new(4);
    let alternate = ParamsIPA::<C>::new(4);
    let pk = product_key::<C, true, true>(&params);
    let values = fixture_values::<C::Scalar>(16 - (pk.vk.cs.blinding_factors() + 1), false);
    let instances = values.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let too_long = vec![C::Scalar::ZERO; 17];
    let bad_instances = vec![too_long.as_slice(); 4];
    let min =
        vanishing_scratch_bytes::<C, VanishingSnapshot<C>, VanishingWriter<C>>(4, 3, 4).unwrap();
    for case in 0..17 {
        let shared = Shared::<C>::new();
        let controls = VanishingControls::new();
        let mut products = vanishing_products!(&params, pk.clone(), &instances, &shared, &controls);
        let mut budget = 1 << 20;
        match case {
            0 => budget = min - 1,
            1 => products.inner.params = &alternate,
            2 => products.usable_rows += 1,
            3 => products.inner.pk.vk.cs_degree = 2,
            4 => products.inner.pk.vk.cs.num_advice_columns += 1,
            5 => products.inner.pk.vk.cs.num_instance_columns += 1,
            6 => products.inner.pk.vk.cs.num_challenges += 1,
            7 => products.inner.pk.fixed_values.push(
                products
                    .inner
                    .pk
                    .vk
                    .domain
                    .lagrange_from_vec(vec![C::Scalar::ZERO; 16]),
            ),
            8 => products.inner.pk.permutation.permutations.push(
                products
                    .inner
                    .pk
                    .vk
                    .domain
                    .lagrange_from_vec(vec![C::Scalar::ZERO; 16]),
            ),
            9 => {
                products.inner.pk.fixed_polys.pop();
            }
            10 => {
                products.inner.pk.permutation.polys.pop();
            }
            11 => {
                products.inner.pk.l0.values.pop();
            }
            12 => products.permutations.swap(0, 4),
            13 => products.lookups.swap(0, 1),
            14 => products.inner.instances = &bad_instances,
            15 => {
                products.inner.pk.vk.cs.permutation.columns[0] =
                    products.inner.pk.vk.cs.permutation.columns[1]
            }
            16 => products
                .inner
                .pk
                .vk
                .cs
                .advice_column_phase
                .pop()
                .map(|_| ())
                .unwrap(),
            _ => unreachable!(),
        }
        let before = {
            let l = shared.log.lock().unwrap();
            (l.created, l.reads, l.writes, l.rng_calls, l.events.clone())
        };
        take_vanishing_clears();
        let result = products.commit_vanishing_and_stage_coefficients(budget);
        assert!(result.is_err(), "accepted public preflight {case}");
        let l = shared.log.lock().unwrap();
        assert_eq!(
            (l.created, l.reads, l.writes, l.rng_calls, l.events.clone()),
            before
        );
        drop(l);
        assert_dropped(&shared);
        let (_, fz, _, ez, _, bz) = take_vanishing_clears();
        assert!(fz && ez && bz);
    }
    assert!(
        vanishing_scratch_bytes::<C, VanishingSnapshot<C>, VanishingWriter<C>>(u32::MAX, 3, 4)
            .is_err()
    );
    assert!(
        vanishing_scratch_bytes::<C, VanishingSnapshot<C>, VanishingWriter<C>>(4, usize::MAX, 4)
            .is_err()
    );
    assert!(
        vanishing_scratch_bytes::<C, VanishingSnapshot<C>, VanishingWriter<C>>(4, 3, usize::MAX)
            .is_err()
    );
}
#[test]
fn both_pasta_vanishing_budget_geometry_key_and_inventory_preflights_consume_without_backend_io() {
    vanishing_preflight::<EqAffine>();
    vanishing_preflight::<EpAffine>();
}

fn vanishing_no_reads<C>()
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
{
    const Q: bool = false;
    const M: u64 = 0;
    let params = ParamsIPA::<C>::new(4);
    let pk = product_key::<C, true, true>(&params);
    let values = fixture_values::<C::Scalar>(16 - (pk.vk.cs.blinding_factors() + 1), false);
    let instances = values.iter().map(Vec::as_slice).collect::<Vec<_>>();
    for case in 0..4 {
        let shared = Shared::<C>::new();
        let controls = VanishingControls::new();
        let products = vanishing_products!(&params, pk.clone(), &instances, &shared, &controls);
        let ordinal = products
            .inner
            .advice
            .layouts()
            .unwrap()
            .next()
            .unwrap()
            .ordinal();
        *controls.product.base.backend.lock().unwrap() = match case {
            0 => MemberFault::Read(ordinal, 0),
            1 => MemberFault::PanicRead(ordinal, 0),
            2 => MemberFault::Encoding(ordinal, 0),
            3 => MemberFault::ShortChunk(ordinal, 0),
            _ => unreachable!(),
        };
        let reads = shared.log.lock().unwrap().reads;
        let actual = products
            .commit_vanishing_and_stage_coefficients(1 << 20)
            .unwrap();
        assert_eq!(
            shared.log.lock().unwrap().reads,
            reads,
            "handoff unexpectedly decoded an old receipt"
        );
        drop(actual);
        assert_dropped(&shared);
    }
}
#[test]
fn both_pasta_vanishing_and_handoff_never_enter_armed_backend_decoder_or_column_windows() {
    vanishing_no_reads::<EqAffine>();
    vanishing_no_reads::<EpAffine>();
}

struct VanishingEmptyCircuit<C: CurveAffine, const I: usize>(Producer<C>);
impl<C: CurveAffine, const I: usize> Circuit<C::Scalar> for VanishingEmptyCircuit<C, I> {
    type Config = Vec<Column<Instance>>;
    type FloorPlanner = V1;
    #[cfg(feature = "circuit-params")]
    type Params = ();
    fn without_witnesses(&self) -> Self {
        Self(self.0.without_witnesses())
    }
    fn configure(meta: &mut ConstraintSystem<C::Scalar>) -> Self::Config {
        (0..I).map(|_| meta.instance_column()).collect()
    }
    fn synthesize_for_measurement(
        &self,
        _: Self::Config,
        _: impl Layouter<C::Scalar>,
    ) -> Result<(), Error> {
        self.0.shared.log.lock().unwrap().measurement_passes += 1;
        Ok(())
    }
    fn synthesize(&self, _: Self::Config, _: impl Layouter<C::Scalar>) -> Result<(), Error> {
        self.0.shared.log.lock().unwrap().synthesis_passes += 1;
        Ok(())
    }
}
fn vanishing_zero_source<C, const I: usize, const Q: bool, const M: u64>(near_end: bool)
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
{
    use crate::plonk::{
        lookup::prover::stored_products_ordinary_oracle,
        vanishing::stored_vanishing_ordinary_oracle,
    };
    let params = ParamsIPA::<C>::new(4);
    let key_shared = Shared::<C>::new();
    let circuit = VanishingEmptyCircuit::<C, I>(Producer::new(&key_shared, 0));
    let vk = keygen_vk_custom(&params, &circuit, true).unwrap();
    let pk = keygen_pk(&params, vk, &circuit).unwrap();
    drop(circuit);
    let values = (0..I)
        .map(|column| {
            if column % 2 == 0 {
                Vec::new()
            } else {
                vec![C::Scalar::from(column as u64), C::Scalar::ZERO]
            }
        })
        .collect::<Vec<_>>();
    let instances = values.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let shared = Shared::<C>::new();
    let controls = VanishingControls::new();
    let member =
        prepare_single_phase_stored_ipa_prefix_v1::<C, _, _, _, ProductChallenge<C>, _, Q, M>(
            &params,
            pk,
            VanishingEmptyCircuit::<C, I>(Producer::new(&shared, 0)),
            &instances,
            vanishing_provider(&shared, &controls),
            ProductRng {
                inner: BoundaryRng {
                    inner: Rng(Arc::clone(&shared)),
                    controls: controls.product.base.clone(),
                },
                controls: controls.product.clone(),
            },
            ProductTranscript::new(&shared, &controls.product),
        )
        .unwrap()
        .stage_advice_coefficients()
        .unwrap()
        .compress_lookups(0)
        .unwrap()
        .sort_lookup_values(0)
        .unwrap()
        .prepare_lookup_membership(0)
        .unwrap();
    let dense = values
        .iter()
        .map(|v| {
            let mut p = v.clone();
            p.resize(16, C::Scalar::ZERO);
            member.compressed.inner.pk.vk.domain.lagrange_from_vec(p)
        })
        .collect::<Vec<_>>();
    let coefficients = dense
        .iter()
        .map(|p| {
            member
                .compressed
                .inner
                .pk
                .vk
                .domain
                .lagrange_to_coeff(p.clone())
                .to_vec()
        })
        .collect::<Vec<_>>();
    let ordinary_shared = Shared::<C>::new();
    let ordinary_controls = ProductControls::new();
    let mut transcript = ProductTranscript::new(&ordinary_shared, &ordinary_controls);
    replay_product_prefix(&shared.log.lock().unwrap().events, &mut transcript);
    let mut rng = shared.rng.lock().unwrap().clone();
    let old = stored_products_ordinary_oracle(
        &member.compressed.inner.pk,
        &params,
        member.compressed.theta,
        &[],
        &dense,
        &[],
        &mut rng,
        &mut transcript,
    )
    .unwrap();
    let oracle = stored_vanishing_ordinary_oracle(
        &params,
        &member.compressed.inner.pk.vk.domain,
        &mut rng,
        &mut transcript,
    )
    .unwrap();
    let mut products = member
        .commit_permuted_lookups(0)
        .unwrap()
        .commit_products(0)
        .unwrap();
    assert_eq!(products.inner.advice.proof_context().unwrap(), None);
    assert_eq!(
        products.inner.advice.product_ordinal_boundary(0).unwrap(),
        None
    );
    assert_eq!(shared.log.lock().unwrap().created, 0);
    if near_end {
        assert_eq!(I, 0);
        products.inner.provider.inner.inner.inner.inner.ordinal = u64::MAX - 1;
    }
    let first = products.inner.provider.inner.inner.inner.inner.ordinal;
    let mut actual = products
        .commit_vanishing_and_stage_coefficients(1 << 20)
        .unwrap();
    assert_eq!(actual.advice.proof_context().unwrap(), Some([23; 32]));
    assert_eq!(
        actual.advice.greatest_ordinal().unwrap(),
        Some(first + I as u64)
    );
    assert_eq!(actual.advice.layouts().unwrap().len(), 0);
    assert_eq!(
        (*actual.beta, *actual.gamma, *actual.y),
        (old.beta, old.gamma, oracle.y)
    );
    assert_eq!(actual.instance_coefficients.len(), I);
    for (actual, expected) in actual.instance_coefficients.iter_mut().zip(coefficients) {
        assert_eq!(
            read_compressed::<C, _>(&mut actual.snapshot, actual.layout),
            expected
        );
    }
    assert_eq!(actual.random.coefficient.layout.ordinal(), first + I as u64);
    assert_eq!(
        read_compressed::<C, _>(
            &mut actual.random.coefficient.snapshot,
            actual.random.coefficient.layout
        ),
        oracle.random_coefficient
    );
    assert_eq!((actual.random.blind.0).0, oracle.random_blind.0);
    assert_eq!(
        shared.log.lock().unwrap().events,
        ordinary_shared.log.lock().unwrap().events
    );
    assert_eq!(
        actual.transcript.inner.inner.inner.clone().finalize(),
        transcript.inner.inner.inner.clone().finalize()
    );
    let mut a = [0; 64];
    let mut b = [0; 64];
    actual.rng.fill_bytes(&mut a);
    rng.fill_bytes(&mut b);
    assert_eq!(a, b);
    assert_eq!(shared.log.lock().unwrap().created, I + 1);
    assert_eq!(
        controls.peaks.lock().unwrap().iter().max().copied(),
        Some(I + 1)
    );
    drop(actual);
    assert_dropped(&shared);
}
fn zero_source_matrix<C>()
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3> + FromUniformBytes<64>,
{
    vanishing_zero_source::<C, 0, false, 0>(false);
    vanishing_zero_source::<C, 0, true, 0>(false);
    vanishing_zero_source::<C, 0, false, 0>(true);
    vanishing_zero_source::<C, 4, false, 0>(false);
    vanishing_zero_source::<C, 4, true, 0>(false);
    vanishing_zero_source::<C, 4, true, 6>(false);
}
#[test]
fn both_pasta_zero_sources_establish_only_actual_first_context_and_still_commit_randomness_y() {
    zero_source_matrix::<EqAffine>();
    zero_source_matrix::<EpAffine>();
}

#[test]
fn instance_and_vanishing_roles_bind_tag7_tag8_field_column_basis_coset_and_reject_advice() {
    use blake2b_simd::Params;
    let mut digests = std::collections::BTreeSet::new();
    for field in [StoredPastaFieldV1::Fp, StoredPastaFieldV1::Fq] {
        for k in [4_u32, 9] {
            for (tag, index, role) in [
                (7_u8, 0_u32, StoredPolynomialRoleV1::Instance { column: 0 }),
                (7, 1, StoredPolynomialRoleV1::Instance { column: 1 }),
                (
                    7,
                    u32::MAX,
                    StoredPolynomialRoleV1::Instance { column: u32::MAX },
                ),
                (8, 0, StoredPolynomialRoleV1::VanishingRandom),
            ] {
                for (basis, basis_tag, extension, part) in [
                    (StoredPolynomialBasisV1::Lagrange, 0_u8, 0_u32, 0_u32),
                    (StoredPolynomialBasisV1::Coefficient, 1, 0, 0),
                    (
                        StoredPolynomialBasisV1::CosetPart {
                            extension_log: 1,
                            part: 0,
                        },
                        2,
                        1,
                        0,
                    ),
                    (
                        StoredPolynomialBasisV1::CosetPart {
                            extension_log: 1,
                            part: 1,
                        },
                        2,
                        1,
                        1,
                    ),
                ] {
                    let layout =
                        StoredPolynomialLayoutV1::new([23; 32], 57, field, basis, k, role).unwrap();
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
                    hash.update(&[basis_tag]);
                    hash.update(&extension.to_le_bytes());
                    hash.update(&part.to_le_bytes());
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
                        StoredPolynomialLayoutV1::new([0; 32], 57, field, basis, k, role).is_err()
                    );
                    assert_ne!(
                        layout.context_digest(),
                        StoredPolynomialLayoutV1::new([23; 32], 58, field, basis, k, role)
                            .unwrap()
                            .context_digest()
                    );
                    for old in [
                        StoredPolynomialRoleV1::Advice {
                            column: index,
                            phase: 0,
                        },
                        StoredPolynomialRoleV1::LookupCompressed {
                            lookup: index,
                            side: StoredLookupSideV1::Input,
                        },
                        StoredPolynomialRoleV1::LookupPermuted {
                            lookup: index,
                            side: StoredLookupSideV1::Input,
                        },
                        StoredPolynomialRoleV1::CopyPermutationProduct { set: index },
                        StoredPolynomialRoleV1::LookupProduct { lookup: index },
                    ] {
                        assert_ne!(
                            layout.context_digest(),
                            StoredPolynomialLayoutV1::new([23; 32], 57, field, basis, k, old)
                                .unwrap()
                                .context_digest()
                        );
                    }
                }
            }
        }
    }
    assert_eq!(digests.len(), 64);
}

#[path = "quotient_tests.rs"]
mod quotient;
