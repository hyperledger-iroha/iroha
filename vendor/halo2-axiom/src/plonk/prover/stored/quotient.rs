//! Consuming ordinary quotient numerator with bounded authenticated coset caching.
//!
//! Public preprocessing moves through explicit part states in its original scalar allocations.
//! Private coefficients remain authenticated and owned; cache eviction never retires an original.
//! TODO: compile and qualify this private continuation together with its complete predecessors.

use super::{
    lookup::StoredLookupErrorV1,
    lookup_permuted::PermutedPolynomialV1,
    lookup_sort::Encoded,
    products::{ProductLookupV1, StoredProductV1},
    vanishing::VanishingPendingStoredIpaProverV1,
};
use crate::{
    arithmetic::CurveAffine,
    plonk::{
        ProvingKey,
        circuit::Any,
        evaluation::{
            Evaluator,
            stored::{
                StoredExpressionErrorV1, StoredRowTileV1,
                graph::coset::{
                    StoredCosetGraphContextV1, StoredCosetGraphPlanV1, StoredCosetGraphSourceV1,
                    StoredCosetGraphWorkspaceV1, prepare_stored_coset_graph_v1,
                    with_stored_coset_graph_chunk_v1,
                },
            },
        },
    },
    poly::{
        commitment::Params,
        ipa::commitment::ParamsIPA,
        stored_advice::{
            STORED_MAX_K_V1, STORED_SCALARS_PER_CHUNK_V1, StoredLookupSideV1,
            StoredPolynomialBasisV1, StoredPolynomialErrorV1, StoredPolynomialLayoutV1,
            StoredPolynomialProviderV1, StoredPolynomialRoleV1, StoredPolynomialSnapshotV1,
            StoredPolynomialWriterV1, assignment::StoredAssignmentFieldV1,
            phase::CoefficientOnlyStoredAdviceV1,
        },
    },
};
use ff::{Field, PrimeField, WithSmallOrderMulGroup};
use std::{
    ptr,
    sync::atomic::{Ordering, compiler_fence},
};

const TILE: usize = STORED_SCALARS_PER_CHUNK_V1;
const HANDLES: usize = 512;
const RELATION_TILES: usize = 16;
type SnapshotOf<P> =
    <<P as StoredPolynomialProviderV1>::Writer as StoredPolynomialWriterV1>::Snapshot;

/// Undivided numerator parts inseparable from every original coefficient, blind and protocol owner.
#[allow(dead_code)]
pub(crate) struct QuotientNumeratorPendingStoredIpaProverV1<
    'params,
    'instances,
    C,
    P,
    R,
    T,
    E,
    const Q: bool,
    const M: u64,
> where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1,
    P: StoredPolynomialProviderV1,
{
    pub(super) inner: VanishingPendingStoredIpaProverV1<'params, 'instances, C, P, R, T, E, Q, M>,
    pub(super) parts: Vec<PermutedPolynomialV1<SnapshotOf<P>>>,
}

fn reserve<V>(count: usize) -> Result<Vec<V>, StoredLookupErrorV1> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| StoredLookupErrorV1::Allocation)?;
    Ok(values)
}
fn add(a: usize, b: usize) -> Result<usize, StoredLookupErrorV1> {
    a.checked_add(b).ok_or(StoredLookupErrorV1::Context)
}
fn mul(a: usize, b: usize) -> Result<usize, StoredLookupErrorV1> {
    a.checked_mul(b).ok_or(StoredLookupErrorV1::Context)
}
fn capacity_error() -> StoredLookupErrorV1 {
    StoredPolynomialErrorV1::Capacity.into()
}
fn clear<F: StoredAssignmentFieldV1>(values: &mut [F]) {
    for value in values.iter_mut() {
        // SAFETY: initialized exclusive slots of sealed Copy Pasta fields admit ZERO.
        unsafe { ptr::write_volatile(value, F::ZERO) };
    }
    compiler_fence(Ordering::SeqCst);
    #[cfg(test)]
    FIELD_CLEARS.with(|v| {
        let (count, zero) = v.get();
        v.set((
            count + values.len(),
            zero && values.iter().all(|v| *v == F::ZERO),
        ));
    });
}
struct Fields<F: StoredAssignmentFieldV1>(Vec<F>);
impl<F: StoredAssignmentFieldV1> Fields<F> {
    fn zeroed(count: usize) -> Result<Self, StoredLookupErrorV1> {
        let mut fields = reserve(count)?;
        fields.resize(count, F::ZERO);
        Ok(Self(fields))
    }
    fn bytes(&self) -> Result<usize, StoredLookupErrorV1> {
        mul(self.0.capacity(), std::mem::size_of::<F>())
    }
}
impl<F: StoredAssignmentFieldV1> Drop for Fields<F> {
    fn drop(&mut self) {
        clear(&mut self.0);
    }
}

#[cfg(test)]
thread_local! {
    static FIELD_CLEARS: std::cell::Cell<(usize, bool)> = const { std::cell::Cell::new((0, true)) };
    static CACHE_OBSERVATIONS: std::cell::Cell<(usize, usize, usize, usize, usize)> = const { std::cell::Cell::new((0, 0, 0, 0, 0)) };
}
#[cfg(test)]
pub(super) fn take_clear_observations() -> (usize, bool, usize, bool, usize, bool) {
    let (n, z) = FIELD_CLEARS.with(|v| v.replace((0, true)));
    let (o, oz, b, bz, l, lz) = super::lookup_permuted::take_clear_observations();
    (n + o, z && oz, b, bz, l, lz)
}
/// Test-only cache hits, misses, evictions, peak live entries and consumed opportunities.
#[cfg(test)]
pub(super) fn take_cache_observations() -> (usize, usize, usize, usize, usize) {
    CACHE_OBSERVATIONS.with(|v| v.replace((0, 0, 0, 0, 0)))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Source {
    Advice(u32),
    Instance(u32),
    Copy(u32),
    LookupInput(u32),
    LookupTable(u32),
    LookupProduct(u32),
}
struct CacheEntry<S> {
    source: Source,
    polynomial: PermutedPolynomialV1<S>,
}
struct PublicColumn<F: StoredAssignmentFieldV1> {
    fields: Fields<F>,
    part: Option<u32>,
}
/// Only this private owner may interpret the moved original public allocations as cosets.
struct PublicBank<F: StoredAssignmentFieldV1> {
    fixed: Vec<PublicColumn<F>>,
    sigma: Vec<PublicColumn<F>>,
    masks: [PublicColumn<F>; 3],
}
impl<F: StoredAssignmentFieldV1> PublicBank<F> {
    fn empty(fixed: usize, sigma: usize) -> Result<Self, StoredLookupErrorV1> {
        Ok(Self {
            fixed: reserve(fixed)?,
            sigma: reserve(sigma)?,
            masks: std::array::from_fn(|_| PublicColumn {
                fields: Fields(Vec::new()),
                part: None,
            }),
        })
    }
}

/// All old witness owners remain here while graph plans borrow the separately retained evaluator.
struct Work<'params, 'instances, 'ev, C: CurveAffine, P: StoredPolynomialProviderV1>
where
    C::Scalar: StoredAssignmentFieldV1,
{
    params: &'params ParamsIPA<C>,
    pk: ProvingKey<C>,
    evaluator: &'ev Evaluator<C>,
    advice: Option<CoefficientOnlyStoredAdviceV1<'params, C, SnapshotOf<P>>>,
    provider: P,
    instances: &'instances [&'instances [C::Scalar]],
    permutations: Vec<StoredProductV1<C, SnapshotOf<P>>>,
    lookups: Vec<ProductLookupV1<C, SnapshotOf<P>>>,
    instance_coefficients: Vec<PermutedPolynomialV1<SnapshotOf<P>>>,
    random: StoredProductV1<C, SnapshotOf<P>>,
    public: PublicBank<C::Scalar>,
    public_moved: bool,
    cache: Vec<Option<CacheEntry<SnapshotOf<P>>>>,
    cache_limit: usize,
    victim: usize,
    parts: Vec<PermutedPolynomialV1<SnapshotOf<P>>>,
    n: usize,
    usable: usize,
    extension_log: u32,
    part: u32,
    old_end: u64,
    remaining: usize,
}

impl<'params, 'instances, 'ev, C, P> Work<'params, 'instances, 'ev, C, P>
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3>,
    P: StoredPolynomialProviderV1,
{
    fn advice(
        &self,
    ) -> Result<&CoefficientOnlyStoredAdviceV1<'params, C, SnapshotOf<P>>, StoredLookupErrorV1>
    {
        self.advice
            .as_ref()
            .ok_or_else(|| StoredPolynomialErrorV1::Poisoned.into())
    }
    fn context_matches(&self, layout: StoredPolynomialLayoutV1) -> bool {
        self.advice
            .as_ref()
            .and_then(|a| a.proof_context().ok().flatten())
            .is_some_and(|context| {
                StoredPolynomialLayoutV1::new(
                    context,
                    layout.ordinal(),
                    layout.field(),
                    layout.basis(),
                    layout.k(),
                    layout.role(),
                )
                .is_ok_and(|expected| expected.same_proof_context(layout))
            })
    }
    fn coefficient(
        &self,
        source: Source,
    ) -> Result<&PermutedPolynomialV1<SnapshotOf<P>>, StoredLookupErrorV1> {
        let value = match source {
            Source::Advice(_) => None,
            Source::Instance(i) => self.instance_coefficients.get(i as usize),
            Source::Copy(i) => self.permutations.get(i as usize).map(|v| &v.coefficient),
            Source::LookupInput(i) => self.lookups.get(i as usize).map(|v| &v.input.coefficient),
            Source::LookupTable(i) => self.lookups.get(i as usize).map(|v| &v.table.coefficient),
            Source::LookupProduct(i) => {
                self.lookups.get(i as usize).map(|v| &v.product.coefficient)
            }
        };
        value.ok_or(StoredLookupErrorV1::Context)
    }
    fn coefficient_mut(
        &mut self,
        source: Source,
    ) -> Result<&mut PermutedPolynomialV1<SnapshotOf<P>>, StoredLookupErrorV1> {
        let value = match source {
            Source::Advice(_) => None,
            Source::Instance(i) => self.instance_coefficients.get_mut(i as usize),
            Source::Copy(i) => self
                .permutations
                .get_mut(i as usize)
                .map(|v| &mut v.coefficient),
            Source::LookupInput(i) => self
                .lookups
                .get_mut(i as usize)
                .map(|v| &mut v.input.coefficient),
            Source::LookupTable(i) => self
                .lookups
                .get_mut(i as usize)
                .map(|v| &mut v.table.coefficient),
            Source::LookupProduct(i) => self
                .lookups
                .get_mut(i as usize)
                .map(|v| &mut v.product.coefficient),
        };
        value.ok_or(StoredLookupErrorV1::Context)
    }
    fn source_layout(
        &self,
        source: Source,
    ) -> Result<StoredPolynomialLayoutV1, StoredLookupErrorV1> {
        match source {
            Source::Advice(column) => self
                .advice()?
                .layouts()?
                .nth(column as usize)
                .ok_or(StoredLookupErrorV1::Context),
            _ => Ok(self.coefficient(source)?.layout),
        }
    }
    fn role(&self, source: Source) -> Result<StoredPolynomialRoleV1, StoredLookupErrorV1> {
        Ok(match source {
            Source::Advice(column) => StoredPolynomialRoleV1::Advice {
                column,
                phase: self
                    .pk
                    .vk
                    .cs
                    .advice_column_phase
                    .get(column as usize)
                    .ok_or(StoredLookupErrorV1::Context)?
                    .to_u8(),
            },
            Source::Instance(column) => StoredPolynomialRoleV1::Instance { column },
            Source::Copy(set) => StoredPolynomialRoleV1::CopyPermutationProduct { set },
            Source::LookupInput(lookup) => StoredPolynomialRoleV1::LookupPermuted {
                lookup,
                side: StoredLookupSideV1::Input,
            },
            Source::LookupTable(lookup) => StoredPolynomialRoleV1::LookupPermuted {
                lookup,
                side: StoredLookupSideV1::Table,
            },
            Source::LookupProduct(lookup) => StoredPolynomialRoleV1::LookupProduct { lookup },
        })
    }
    fn check_polynomial(
        &self,
        value: &PermutedPolynomialV1<SnapshotOf<P>>,
        role: StoredPolynomialRoleV1,
        basis: StoredPolynomialBasisV1,
    ) -> Result<(), StoredLookupErrorV1> {
        if value.layout.role() != role
            || value.layout.basis() != basis
            || value.layout.field() != C::Scalar::STORED_FIELD
            || value.layout.k() != self.pk.vk.domain.k()
            || !self.context_matches(value.layout)
            || value.snapshot.layout() != value.layout
        {
            return Err(StoredPolynomialErrorV1::Context.into());
        }
        Ok(())
    }
    fn validate<const Q: bool, const M: u64>(&self) -> Result<(), StoredLookupErrorV1> {
        let cs = &self.pk.vk.cs;
        let domain = &self.pk.vk.domain;
        let k = domain.k();
        let p = cs.permutation.columns.len();
        let m = 1_usize
            .checked_shl(self.extension_log)
            .ok_or(StoredLookupErrorV1::Context)?;
        if k > STORED_MAX_K_V1
            || domain.extended_k() > STORED_MAX_K_V1
            || self.extension_log == 0
            || domain.extended_k().checked_sub(k) != Some(self.extension_log)
            || domain.extended_len() != mul(self.n, m)?
            || self.part as usize >= m
            || self.params.k() != k
            || self.params.n() != self.n as u64
            || self.n != 1_usize << k
            || self.params.get_g_lagrange().len() != self.n
            || cs
                .blinding_factors()
                .checked_add(1)
                .and_then(|b| self.n.checked_sub(b))
                != Some(self.usable)
            || self.pk.vk.cs_degree != cs.degree()
            || cs.degree() < 3
            || p.div_ceil(cs.degree() - 2) != self.permutations.len()
            || cs.lookups.len() != self.lookups.len()
            || self.evaluator.lookups.len() != self.lookups.len()
            || cs.advice_column_phase.len() != cs.num_advice_columns
            || cs.challenge_phase.len() != cs.num_challenges
            || self.pk.fixed_polys.len() != cs.num_fixed_columns
            || self.pk.permutation.polys.len() != p
            || !self.pk.fixed_values.is_empty()
            || !self.pk.permutation.permutations.is_empty()
            || self.instances.len() != cs.num_instance_columns
            || self.instance_coefficients.len() != self.instances.len()
            || self.instances.iter().any(|v| v.len() > self.usable)
            || (!Q && M != 0)
            || (cs.num_instance_columns < 64 && M >> cs.num_instance_columns != 0)
            || self.parts.len() > self.part as usize + 1
            || self.cache_limit > self.cache.len()
            || self.cache[self.cache_limit..].iter().any(Option::is_some)
        {
            return Err(StoredLookupErrorV1::Context);
        }
        if self.public_moved {
            if self.public.fixed.len() != cs.num_fixed_columns
                || self.public.sigma.len() != p
                || self
                    .pk
                    .fixed_polys
                    .iter()
                    .chain(&self.pk.permutation.polys)
                    .any(|v| !v.is_empty())
                || !self.pk.l0.is_empty()
                || !self.pk.l_last.is_empty()
                || !self.pk.l_active_row.is_empty()
                || self
                    .public
                    .fixed
                    .iter()
                    .chain(&self.public.sigma)
                    .chain(&self.public.masks)
                    .any(|v| {
                        v.fields.0.len() != self.n || v.part.is_some_and(|part| part != self.part)
                    })
            {
                return Err(StoredLookupErrorV1::Context);
            }
        } else if self
            .pk
            .fixed_polys
            .iter()
            .chain(&self.pk.permutation.polys)
            .any(|v| v.len() != self.n)
            || self.pk.l0.len() != self.n
            || self.pk.l_last.len() != self.n
            || self.pk.l_active_row.len() != self.n
        {
            return Err(StoredLookupErrorV1::Context);
        }
        for (i, column) in cs.permutation.columns.iter().enumerate() {
            let valid = match column.column_type() {
                Any::Advice(kind) => cs
                    .advice_column_phase
                    .get(column.index())
                    .is_some_and(|phase| phase.to_u8() == kind.phase()),
                Any::Fixed => column.index() < cs.num_fixed_columns,
                Any::Instance => column.index() < cs.num_instance_columns,
            };
            if !valid || cs.permutation.columns[..i].contains(column) {
                return Err(StoredLookupErrorV1::Context);
            }
        }
        let advice = self.advice()?;
        advice.validate_live_receipts()?;
        if !ptr::eq(advice.params()?, self.params)
            || advice.layouts()?.len() != cs.num_advice_columns
            || advice.challenges()?.count() != cs.num_challenges
        {
            return Err(StoredLookupErrorV1::Context);
        }
        let mut last = None;
        for (layout, phase) in advice.layouts()?.zip(&cs.advice_column_phase) {
            if layout.advice_coordinates()?.1 != phase.to_u8()
                || last.is_some_and(|old| layout.ordinal() <= old)
            {
                return Err(StoredPolynomialErrorV1::Context.into());
            }
            last = Some(layout.ordinal());
        }
        let mut ordered = |value: &PermutedPolynomialV1<SnapshotOf<P>>,
                           role|
         -> Result<(), StoredLookupErrorV1> {
            self.check_polynomial(value, role, StoredPolynomialBasisV1::Coefficient)?;
            if last.is_some_and(|old| value.layout.ordinal() <= old)
                || value.layout.ordinal() > self.old_end
            {
                return Err(StoredPolynomialErrorV1::Context.into());
            }
            last = Some(value.layout.ordinal());
            Ok(())
        };
        for (i, lookup) in self.lookups.iter().enumerate() {
            ordered(
                &lookup.input.coefficient,
                self.role(Source::LookupInput(i as u32))?,
            )?;
            ordered(
                &lookup.table.coefficient,
                self.role(Source::LookupTable(i as u32))?,
            )?;
        }
        for (i, value) in self.permutations.iter().enumerate() {
            ordered(&value.coefficient, self.role(Source::Copy(i as u32))?)?;
        }
        for (i, lookup) in self.lookups.iter().enumerate() {
            ordered(
                &lookup.product.coefficient,
                self.role(Source::LookupProduct(i as u32))?,
            )?;
        }
        for (i, value) in self.instance_coefficients.iter().enumerate() {
            ordered(value, self.role(Source::Instance(i as u32))?)?;
        }
        ordered(
            &self.random.coefficient,
            StoredPolynomialRoleV1::VanishingRandom,
        )?;
        if last != Some(self.old_end) {
            return Err(StoredPolynomialErrorV1::Context.into());
        }
        let end = advice
            .greatest_ordinal()?
            .ok_or(StoredLookupErrorV1::Context)?;
        let mut last_part = self.old_end;
        for (part, value) in self.parts.iter().enumerate() {
            self.check_polynomial(
                value,
                StoredPolynomialRoleV1::QuotientNumerator,
                StoredPolynomialBasisV1::CosetPart {
                    extension_log: self.extension_log,
                    part: part as u32,
                },
            )?;
            if value.layout.ordinal() <= last_part || value.layout.ordinal() > end {
                return Err(StoredPolynomialErrorV1::Context.into());
            }
            last_part = value.layout.ordinal();
        }
        for (slot, entry) in self.cache.iter().enumerate() {
            if let Some(entry) = entry {
                self.check_polynomial(
                    &entry.polynomial,
                    self.role(entry.source)?,
                    StoredPolynomialBasisV1::CosetPart {
                        extension_log: self.extension_log,
                        part: self.part,
                    },
                )?;
                let ordinal = entry.polynomial.layout.ordinal();
                if ordinal <= self.old_end
                    || ordinal > end
                    || self.parts.iter().any(|v| v.layout.ordinal() == ordinal)
                    || self.cache[..slot].iter().flatten().any(|v| {
                        v.source == entry.source || v.polynomial.layout.ordinal() == ordinal
                    })
                {
                    return Err(StoredPolynomialErrorV1::Context.into());
                }
            }
        }
        Ok(())
    }
    fn opportunity(&mut self) -> Result<(), StoredLookupErrorV1> {
        self.remaining = self
            .remaining
            .checked_sub(1)
            .ok_or(StoredLookupErrorV1::Context)?;
        #[cfg(test)]
        CACHE_OBSERVATIONS.with(|v| {
            let (h, m, e, p, o) = v.get();
            v.set((h, m, e, p, o + 1));
        });
        Ok(())
    }
    fn check_remaining(&self, layout: StoredPolynomialLayoutV1) -> Result<(), StoredLookupErrorV1> {
        layout
            .ordinal()
            .checked_add(u64::try_from(self.remaining).map_err(|_| capacity_error())?)
            .and_then(|v| v.checked_add(1))
            .ok_or_else(capacity_error)?;
        Ok(())
    }
    fn evict<const Q: bool, const M: u64>(
        &mut self,
        slot: usize,
    ) -> Result<(), StoredLookupErrorV1> {
        self.validate::<Q, M>()?;
        if let Some(old) = self.cache[slot].take() {
            drop(old);
            #[cfg(test)]
            CACHE_OBSERVATIONS.with(|v| {
                let (h, m, e, p, o) = v.get();
                v.set((h, m, e + 1, p, o));
            });
        }
        self.validate::<Q, M>()
    }
    fn prune<const Q: bool, const M: u64>(
        &mut self,
        retain: impl Fn(Source) -> bool,
    ) -> Result<(), StoredLookupErrorV1> {
        for slot in 0..self.cache.len() {
            if self.cache[slot]
                .as_ref()
                .is_some_and(|entry| !retain(entry.source))
            {
                self.evict::<Q, M>(slot)?;
            }
        }
        self.validate::<Q, M>()
    }
}

fn read_chunk<F: StoredAssignmentFieldV1, S: StoredPolynomialSnapshotV1>(
    snapshot: &mut S,
    expected: StoredPolynomialLayoutV1,
    chunk: u64,
    output: &mut [F],
) -> Result<(), StoredLookupErrorV1> {
    if snapshot.layout() != expected || output.len() != expected.chunk_scalar_count(chunk)? {
        return Err(StoredPolynomialErrorV1::Context.into());
    }
    let mut decoded = false;
    snapshot.with_chunk(expected, chunk, |encoded| {
        if encoded.len() != output.len()
            || encoded.iter().any(|v| !expected.field().is_canonical(v))
        {
            return Err(StoredPolynomialErrorV1::Encoding);
        }
        for (target, value) in output.iter_mut().zip(encoded) {
            *target =
                Option::<F>::from(F::from_repr(*value)).ok_or(StoredPolynomialErrorV1::Encoding)?;
        }
        decoded = true;
        Ok(())
    })?;
    if !decoded || snapshot.layout() != expected {
        return Err(StoredPolynomialErrorV1::Context.into());
    }
    Ok(())
}

impl<'params, 'instances, 'ev, C, P> Work<'params, 'instances, 'ev, C, P>
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3>,
    P: StoredPolynomialProviderV1,
{
    fn validate_writer<const Q: bool, const M: u64>(
        &self,
        writer: &P::Writer,
        layout: StoredPolynomialLayoutV1,
    ) -> Result<(), StoredLookupErrorV1> {
        if writer.layout() != layout {
            return Err(StoredPolynomialErrorV1::Context.into());
        }
        self.validate::<Q, M>()?;
        if writer.layout() != layout {
            return Err(StoredPolynomialErrorV1::Context.into());
        }
        Ok(())
    }
    fn create<const Q: bool, const M: u64>(
        &mut self,
        source: Option<Source>,
    ) -> Result<(P::Writer, StoredPolynomialLayoutV1), StoredLookupErrorV1> {
        self.validate::<Q, M>()?;
        let source_layout = source.map(|s| self.source_layout(s)).transpose()?;
        let advice = self.advice.take().ok_or(StoredLookupErrorV1::Context)?;
        let (advice, writer, layout) = match source_layout {
            Some(source) => advice.create_coset_writer(
                &mut self.provider,
                source,
                self.extension_log,
                self.part,
            )?,
            None => {
                advice.create_quotient_writer(&mut self.provider, self.extension_log, self.part)?
            }
        };
        self.advice = Some(advice);
        self.check_remaining(layout)?;
        self.validate_writer::<Q, M>(&writer, layout)?;
        Ok((writer, layout))
    }
    fn seal<const Q: bool, const M: u64>(
        &mut self,
        mut writer: P::Writer,
        layout: StoredPolynomialLayoutV1,
        values: &[C::Scalar],
        encoded: &mut Encoded,
    ) -> Result<PermutedPolynomialV1<SnapshotOf<P>>, StoredLookupErrorV1> {
        if values.len() != self.n {
            return Err(StoredLookupErrorV1::Context);
        }
        for (chunk, values) in values.chunks(TILE).enumerate() {
            self.validate_writer::<Q, M>(&writer, layout)?;
            for (out, value) in encoded.0.iter_mut().zip(values) {
                *out = value.to_repr();
            }
            writer.write_chunk(chunk as u64, &encoded.0[..values.len()])?;
            self.validate_writer::<Q, M>(&writer, layout)?;
        }
        self.validate_writer::<Q, M>(&writer, layout)?;
        let snapshot = writer.seal()?;
        if snapshot.layout() != layout {
            return Err(StoredPolynomialErrorV1::Context.into());
        }
        self.validate::<Q, M>()?;
        if snapshot.layout() != layout {
            return Err(StoredPolynomialErrorV1::Context.into());
        }
        Ok(PermutedPolynomialV1 { layout, snapshot })
    }
    /// One scheduled request, including hits; at most one writer and one guarded transform column.
    fn ensure<const Q: bool, const M: u64>(
        &mut self,
        source: Source,
        column: &mut Fields<C::Scalar>,
        encoded: &mut Encoded,
    ) -> Result<usize, StoredLookupErrorV1> {
        self.validate::<Q, M>()?;
        self.opportunity()?;
        if let Some(slot) = self.cache[..self.cache_limit]
            .iter()
            .position(|entry| entry.as_ref().is_some_and(|v| v.source == source))
        {
            #[cfg(test)]
            CACHE_OBSERVATIONS.with(|v| {
                let (h, m, e, p, o) = v.get();
                v.set((h + 1, m, e, p, o));
            });
            return Ok(slot);
        }
        if self.cache_limit == 0 {
            return Err(capacity_error());
        }
        let slot = if let Some(empty) = self.cache[..self.cache_limit]
            .iter()
            .position(Option::is_none)
        {
            empty
        } else {
            let slot = self.victim % self.cache_limit;
            self.victim = (slot + 1) % self.cache_limit;
            self.evict::<Q, M>(slot)?;
            slot
        };
        let expected = self.source_layout(source)?;
        let (writer, layout) = self.create::<Q, M>(Some(source))?;
        if column.0.len() != self.n {
            return Err(StoredLookupErrorV1::Context);
        }
        for (chunk, output) in column.0.chunks_mut(TILE).enumerate() {
            self.validate_writer::<Q, M>(&writer, layout)?;
            match source {
                Source::Advice(index) => {
                    let advice = self.advice.take().ok_or(StoredLookupErrorV1::Context)?;
                    self.advice =
                        Some(advice.copy_coefficient_chunk_into(index, chunk as u64, output)?);
                }
                _ => read_chunk(
                    &mut self.coefficient_mut(source)?.snapshot,
                    expected,
                    chunk as u64,
                    output,
                )?,
            }
            self.validate_writer::<Q, M>(&writer, layout)?;
        }
        let factor = self
            .pk
            .vk
            .domain
            .get_extended_omega()
            .pow_vartime([self.part as u64]);
        self.pk
            .vk
            .domain
            .stored_column_transform_in_place(&mut column.0, false, Some(factor));
        self.validate_writer::<Q, M>(&writer, layout)?;
        let polynomial = self.seal::<Q, M>(writer, layout, &column.0, encoded)?;
        self.cache[slot] = Some(CacheEntry { source, polynomial });
        clear(&mut column.0);
        #[cfg(test)]
        CACHE_OBSERVATIONS.with(|v| {
            let (h, m, e, p, o) = v.get();
            let live = self.cache.iter().flatten().count();
            v.set((h, m + 1, e, p.max(live), o));
        });
        self.validate::<Q, M>()?;
        Ok(slot)
    }
    /// Copy a rotated tile without retaining a backend window or demanding another cache lease.
    fn read_cached<const Q: bool, const M: u64>(
        &mut self,
        slot: usize,
        rotation: i32,
        tile: StoredRowTileV1,
        output: &mut [C::Scalar],
        read: &mut [C::Scalar],
    ) -> Result<(), StoredLookupErrorV1> {
        if tile.start >= self.n
            || tile.len != TILE.min(self.n - tile.start)
            || output.len() != tile.len
            || read.len() < TILE
        {
            return Err(StoredLookupErrorV1::Context);
        }
        let mut copied = 0;
        while copied < tile.len {
            self.validate::<Q, M>()?;
            let row = (tile.start as i64 + copied as i64 + i64::from(rotation))
                .rem_euclid(self.n as i64) as usize;
            let chunk = row / TILE;
            let count = TILE.min(self.n - chunk * TILE);
            let entry = self
                .cache
                .get_mut(slot)
                .and_then(Option::as_mut)
                .ok_or(StoredLookupErrorV1::Context)?;
            read_chunk(
                &mut entry.polynomial.snapshot,
                entry.polynomial.layout,
                chunk as u64,
                &mut read[..count],
            )?;
            self.validate::<Q, M>()?;
            let offset = row % TILE;
            let take = (count - offset).min(tile.len - copied);
            output[copied..copied + take].copy_from_slice(&read[offset..offset + take]);
            copied += take;
        }
        clear(read);
        self.validate::<Q, M>()
    }
    fn move_public<const Q: bool, const M: u64>(&mut self) -> Result<(), StoredLookupErrorV1> {
        self.validate::<Q, M>()?;
        for poly in &mut self.pk.fixed_polys {
            self.public.fixed.push(PublicColumn {
                fields: Fields(std::mem::take(&mut poly.values)),
                part: None,
            });
        }
        for poly in &mut self.pk.permutation.polys {
            self.public.sigma.push(PublicColumn {
                fields: Fields(std::mem::take(&mut poly.values)),
                part: None,
            });
        }
        for (target, source) in self.public.masks.iter_mut().zip([
            &mut self.pk.l0,
            &mut self.pk.l_last,
            &mut self.pk.l_active_row,
        ]) {
            target.fields.0 = std::mem::take(&mut source.values);
        }
        self.public_moved = true;
        self.validate::<Q, M>()
    }
    fn public_part<const Q: bool, const M: u64>(
        &mut self,
        inverse: bool,
    ) -> Result<(), StoredLookupErrorV1> {
        self.validate::<Q, M>()?;
        let factor = self
            .pk
            .vk
            .domain
            .get_extended_omega()
            .pow_vartime([self.part as u64]);
        for column in self.public.fixed.iter_mut().chain(&mut self.public.masks) {
            if column.part != if inverse { Some(self.part) } else { None } {
                return Err(StoredLookupErrorV1::Context);
            }
            self.pk.vk.domain.stored_column_transform_in_place(
                &mut column.fields.0,
                inverse,
                Some(factor),
            );
            column.part = if inverse { None } else { Some(self.part) };
        }
        self.validate::<Q, M>()
    }
    fn sigma_part<const Q: bool, const M: u64>(
        &mut self,
        start: usize,
        end: usize,
        inverse: bool,
    ) -> Result<(), StoredLookupErrorV1> {
        self.validate::<Q, M>()?;
        let factor = self
            .pk
            .vk
            .domain
            .get_extended_omega()
            .pow_vartime([self.part as u64]);
        for column in self
            .public
            .sigma
            .get_mut(start..end)
            .ok_or(StoredLookupErrorV1::Context)?
        {
            if column.part != if inverse { Some(self.part) } else { None } {
                return Err(StoredLookupErrorV1::Context);
            }
            self.pk.vk.domain.stored_column_transform_in_place(
                &mut column.fields.0,
                inverse,
                Some(factor),
            );
            column.part = if inverse { None } else { Some(self.part) };
        }
        self.validate::<Q, M>()
    }
    fn restore_public<const Q: bool, const M: u64>(&mut self) -> Result<(), StoredLookupErrorV1> {
        self.validate::<Q, M>()?;
        if self
            .public
            .fixed
            .iter()
            .chain(&self.public.sigma)
            .chain(&self.public.masks)
            .any(|v| v.part.is_some())
        {
            return Err(StoredLookupErrorV1::Context);
        }
        for (target, source) in self.pk.fixed_polys.iter_mut().zip(&mut self.public.fixed) {
            target.values = std::mem::take(&mut source.fields.0);
        }
        for (target, source) in self
            .pk
            .permutation
            .polys
            .iter_mut()
            .zip(&mut self.public.sigma)
        {
            target.values = std::mem::take(&mut source.fields.0);
        }
        for (target, source) in [
            &mut self.pk.l0,
            &mut self.pk.l_last,
            &mut self.pk.l_active_row,
        ]
        .into_iter()
        .zip(&mut self.public.masks)
        {
            target.values = std::mem::take(&mut source.fields.0);
        }
        self.public_moved = false;
        self.validate::<Q, M>()
    }
    fn fixed<const Q: bool, const M: u64>(
        &self,
        column: usize,
        rotation: i32,
        tile: StoredRowTileV1,
        output: &mut [C::Scalar],
    ) -> Result<(), StoredLookupErrorV1> {
        self.validate::<Q, M>()?;
        let source = self
            .public
            .fixed
            .get(column)
            .ok_or(StoredLookupErrorV1::Context)?;
        if source.part != Some(self.part)
            || output.len() != tile.len
            || tile.start + tile.len > self.n
        {
            return Err(StoredLookupErrorV1::Context);
        }
        for (i, target) in output.iter_mut().enumerate() {
            let row = (tile.start as i64 + i as i64 + i64::from(rotation)).rem_euclid(self.n as i64)
                as usize;
            *target = source.fields.0[row];
        }
        self.validate::<Q, M>()
    }
}

fn expression_error(error: StoredLookupErrorV1) -> StoredExpressionErrorV1 {
    use crate::poly::stored_advice::phase::StoredPhaseErrorV1;
    match error {
        StoredLookupErrorV1::Store(e)
        | StoredLookupErrorV1::Phase(StoredPhaseErrorV1::Store(e)) => {
            StoredExpressionErrorV1::Store(e)
        }
        StoredLookupErrorV1::Expression(e) => e,
        StoredLookupErrorV1::ScratchLimit => StoredExpressionErrorV1::ScratchLimit,
        StoredLookupErrorV1::Allocation => StoredExpressionErrorV1::Allocation,
        _ => StoredExpressionErrorV1::Context,
    }
}
struct GraphSource<
    'a,
    'params,
    'instances,
    'ev,
    C: CurveAffine,
    P: StoredPolynomialProviderV1,
    const Q: bool,
    const M: u64,
> where
    C::Scalar: StoredAssignmentFieldV1,
{
    work: &'a mut Work<'params, 'instances, 'ev, C, P>,
    column: &'a mut Fields<C::Scalar>,
    encoded: &'a mut Encoded,
    read: &'a mut [C::Scalar],
}
impl<C, P, const Q: bool, const M: u64> StoredCosetGraphSourceV1<C::Scalar>
    for GraphSource<'_, '_, '_, '_, C, P, Q, M>
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3>,
    P: StoredPolynomialProviderV1,
{
    fn validate_context(
        &mut self,
        context: &StoredCosetGraphContextV1<'_>,
        part: u32,
    ) -> Result<(), StoredExpressionErrorV1> {
        self.work.validate::<Q, M>().map_err(expression_error)?;
        let cs = &self.work.pk.vk.cs;
        if context.original != self.work.random.coefficient.layout
            || context.extension_log != self.work.extension_log
            || part != self.work.part
            || context.fixed_columns != cs.num_fixed_columns
            || context.instance_columns != cs.num_instance_columns
            || context.advice_phases.len() != cs.advice_column_phase.len()
            || context.challenge_phases.len() != cs.challenge_phase.len()
            || context
                .advice_phases
                .iter()
                .zip(&cs.advice_column_phase)
                .any(|(a, b)| *a != b.to_u8())
            || context
                .challenge_phases
                .iter()
                .zip(&cs.challenge_phase)
                .any(|(a, b)| *a != b.to_u8())
            || !self.work.public_moved
            || self
                .work
                .public
                .fixed
                .iter()
                .chain(&self.work.public.masks)
                .any(|v| v.part != Some(part))
        {
            return Err(StoredExpressionErrorV1::Context);
        }
        Ok(())
    }
    fn copy_advice_query_into(
        &mut self,
        column: usize,
        rotation: i32,
        tile: StoredRowTileV1,
        destination: &mut [C::Scalar],
    ) -> Result<(), StoredExpressionErrorV1> {
        let source =
            Source::Advice(u32::try_from(column).map_err(|_| StoredExpressionErrorV1::Context)?);
        let slot = self
            .work
            .ensure::<Q, M>(source, self.column, self.encoded)
            .map_err(expression_error)?;
        self.work
            .read_cached::<Q, M>(slot, rotation, tile, destination, self.read)
            .map_err(expression_error)
    }
    fn copy_fixed_query_into(
        &mut self,
        column: usize,
        rotation: i32,
        tile: StoredRowTileV1,
        destination: &mut [C::Scalar],
    ) -> Result<(), StoredExpressionErrorV1> {
        self.work
            .fixed::<Q, M>(column, rotation, tile, destination)
            .map_err(expression_error)
    }
    fn copy_instance_query_into(
        &mut self,
        column: usize,
        rotation: i32,
        tile: StoredRowTileV1,
        destination: &mut [C::Scalar],
    ) -> Result<(), StoredExpressionErrorV1> {
        let source =
            Source::Instance(u32::try_from(column).map_err(|_| StoredExpressionErrorV1::Context)?);
        let slot = self
            .work
            .ensure::<Q, M>(source, self.column, self.encoded)
            .map_err(expression_error)?;
        self.work
            .read_cached::<Q, M>(slot, rotation, tile, destination, self.read)
            .map_err(expression_error)
    }
}

struct Geometry {
    n: usize,
    m: usize,
    extension_log: u32,
    sources: usize,
    inherited: usize,
    cache: usize,
}
fn geometry<C: CurveAffine>(pk: &ProvingKey<C>) -> Result<Geometry, StoredLookupErrorV1> {
    let domain = &pk.vk.domain;
    let cs = &pk.vk.cs;
    if domain.k() > STORED_MAX_K_V1 || domain.extended_k() > STORED_MAX_K_V1 || cs.degree() < 3 {
        return Err(StoredLookupErrorV1::Context);
    }
    let extension_log = domain
        .extended_k()
        .checked_sub(domain.k())
        .ok_or(StoredLookupErrorV1::Context)?;
    if extension_log == 0 {
        return Err(StoredLookupErrorV1::Context);
    }
    let n = 1_usize << domain.k();
    let m = 1_usize << extension_log;
    for count in [
        cs.num_advice_columns,
        cs.num_instance_columns,
        cs.lookups.len(),
        cs.permutation.columns.len(),
        cs.num_fixed_columns,
        cs.num_challenges,
    ] {
        u32::try_from(count).map_err(|_| StoredLookupErrorV1::Context)?;
    }
    let sets = cs.permutation.columns.len().div_ceil(cs.degree() - 2);
    let sources = add(
        add(cs.num_advice_columns, cs.num_instance_columns)?,
        add(sets, mul(3, cs.lookups.len())?)?,
    )?;
    let inherited = add(sources, 1)?;
    // Retained but unqueried private columns need no extra coset lease. Final admission
    // adds that one working lease only after the exact retained graphs are prepared.
    let required = add(inherited, m)?;
    if required > HANDLES {
        return Err(capacity_error());
    }
    let cache = sources.min(
        HANDLES
            .checked_sub(add(inherited, 1)?)
            .ok_or_else(capacity_error)?,
    );
    Ok(Geometry {
        n,
        m,
        extension_log,
        sources,
        inherited,
        cache,
    })
}
/// Exact per-tile requests, independent of cache hits and scalar contents.
fn private_requests<C: CurveAffine>(
    pk: &ProvingKey<C>,
    plans: &[StoredCosetGraphPlanV1<'_, '_, C>],
) -> Result<usize, StoredLookupErrorV1> {
    let cs = &pk.vk.cs;
    let sets = cs.permutation.columns.len().div_ceil(cs.degree() - 2);
    let private_columns = cs
        .permutation
        .columns
        .iter()
        .filter(|column| !matches!(column.column_type(), Any::Fixed))
        .count();
    let first = plans.first().ok_or(StoredLookupErrorV1::Context)?;
    let mut requests = add(
        first.maximum_private_query_requests(),
        add(mul(3, sets)?, private_columns)?,
    )?;
    for plan in &plans[1..] {
        requests = add(requests, add(plan.maximum_private_query_requests(), 3)?)?;
    }
    Ok(requests)
}

fn admit_working_cache(g: &Geometry, requests: usize) -> Result<(), StoredLookupErrorV1> {
    if add(add(g.inherited, g.m)?, usize::from(requests != 0))? > HANDLES {
        return Err(capacity_error());
    }
    Ok(())
}

fn phase_metadata<C: CurveAffine>(
    pk: &ProvingKey<C>,
) -> Result<(Vec<u8>, Vec<u8>), StoredLookupErrorV1> {
    let mut advice = reserve(pk.vk.cs.num_advice_columns)?;
    advice.extend(pk.vk.cs.advice_column_phase.iter().map(|v| v.to_u8()));
    let mut challenges = reserve(pk.vk.cs.num_challenges)?;
    challenges.extend(pk.vk.cs.challenge_phase.iter().map(|v| v.to_u8()));
    Ok((advice, challenges))
}
/// Charged new metadata plus separately known baseline FFT public twiddle payload.
fn metadata_bytes<C, S, W>(
    g: &Geometry,
    cache: usize,
    parts: usize,
    fixed: usize,
    sigma: usize,
    phase_bytes: usize,
    plan_bytes: usize,
    query_bytes: usize,
    planning_temp: usize,
    placeholder_constants: usize,
) -> Result<usize, StoredLookupErrorV1>
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1,
{
    let mut bytes = mul(cache, std::mem::size_of::<Option<CacheEntry<S>>>())?;
    bytes = add(
        bytes,
        mul(parts, std::mem::size_of::<PermutedPolynomialV1<S>>())?,
    )?;
    bytes = add(
        bytes,
        mul(
            add(fixed, sigma)?,
            std::mem::size_of::<PublicColumn<C::Scalar>>(),
        )?,
    )?;
    bytes = add(bytes, std::mem::size_of::<PublicBank<C::Scalar>>())?;
    bytes = add(bytes, std::mem::size_of::<W>())?;
    bytes = add(bytes, std::mem::size_of::<PermutedPolynomialV1<S>>())?;
    bytes = add(
        bytes,
        mul(RELATION_TILES + 3, std::mem::size_of::<Fields<C::Scalar>>())?,
    )?;
    bytes = add(bytes, phase_bytes)?;
    bytes = add(bytes, add(plan_bytes, query_bytes)?)?;
    // Planning flags and later FFT twiddles do not overlap, but charging both is conservative.
    bytes = add(bytes, planning_temp)?;
    // Taking the original evaluator leaves a default graph with three public constants in
    // the inaccessible PK skeleton. Its newly allocated capacity is still part of our budget.
    bytes = add(
        bytes,
        mul(placeholder_constants, std::mem::size_of::<C::Scalar>())?,
    )?;
    add(bytes, mul(g.n / 2, std::mem::size_of::<C::Scalar>())?)
}

/// Public metadata budget estimate for tests; it cannot construct or substitute any proof owner.
#[cfg(test)]
pub(super) fn scratch_bytes<C, P>(
    pk: &ProvingKey<C>,
    original: StoredPolynomialLayoutV1,
) -> Result<usize, StoredLookupErrorV1>
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1,
    P: StoredPolynomialProviderV1,
{
    let g = geometry(pk)?;
    let (advice, challenge) = phase_metadata(pk)?;
    let context = StoredCosetGraphContextV1 {
        original,
        extension_log: g.extension_log,
        advice_phases: &advice,
        fixed_columns: pk.vk.cs.num_fixed_columns,
        instance_columns: pk.vk.cs.num_instance_columns,
        challenge_phases: &challenge,
    };
    let mut plans = reserve(add(pk.ev.lookups.len(), 1)?)?;
    plans.push(prepare_stored_coset_graph_v1(
        &pk.ev.custom_gates,
        context,
        usize::MAX,
    )?);
    for graph in &pk.ev.lookups {
        plans.push(prepare_stored_coset_graph_v1(graph, context, usize::MAX)?);
    }
    admit_working_cache(&g, private_requests(pk, &plans)?)?;
    let fields = plans.iter().map(|p| p.field_count()).max().unwrap_or(0);
    let query = plans
        .iter()
        .try_fold(0, |n, p| add(n, p.metadata_bytes()))?;
    let temporary = plans
        .iter()
        .map(|p| p.planning_temporary_bytes())
        .max()
        .unwrap_or(0);
    let placeholder = Evaluator::<C>::default();
    let metadata = metadata_bytes::<C, SnapshotOf<P>, P::Writer>(
        &g,
        g.cache,
        g.m,
        pk.fixed_polys.len(),
        pk.permutation.polys.len(),
        add(advice.capacity(), challenge.capacity())?,
        mul(plans.capacity(), std::mem::size_of_val(&plans[0]))?,
        query,
        temporary,
        placeholder.custom_gates.constants.capacity(),
    )?;
    let field_slots = add(
        add(mul(2, g.n)?, mul(RELATION_TILES, TILE)?)?,
        add(fields, pk.vk.cs.num_challenges)?,
    )?;
    add(
        add(
            metadata,
            mul(field_slots, std::mem::size_of::<C::Scalar>())?,
        )?,
        mul(TILE, 32)?,
    )
}

impl<'params, 'instances, C, P, R, T, E, const Q: bool, const M: u64>
    VanishingPendingStoredIpaProverV1<'params, 'instances, C, P, R, T, E, Q, M>
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3>,
    P: StoredPolynomialProviderV1,
{
    /// Consume all retained owners into undivided extended numerator parts without new randomness.
    ///
    /// The budget admits new initialized payload and actual adapter capacities, including known
    /// FFT twiddles. Inherited advice/product vectors, original public key/parameters/evaluator,
    /// backend windows, allocator bookkeeping, stack scalar copies and process RSS are separate.
    /// Narrow cache profiles can repeat FFTs; this method makes no latency qualification claim.
    pub(crate) fn evaluate_quotient_numerator(
        self,
        scratch_limit: usize,
    ) -> Result<
        QuotientNumeratorPendingStoredIpaProverV1<'params, 'instances, C, P, R, T, E, Q, M>,
        StoredLookupErrorV1,
    > {
        let g = geometry(&self.pk)?;
        self.advice.validate_live_receipts()?;
        let (advice_phases, challenge_phases) = phase_metadata(&self.pk)?;
        let VanishingPendingStoredIpaProverV1 {
            params,
            mut pk,
            advice,
            provider,
            rng,
            transcript,
            instances,
            theta,
            beta,
            gamma,
            y,
            usable_rows,
            permutations,
            lookups,
            instance_coefficients,
            random,
            _challenge,
        } = self;
        let evaluator = std::mem::take(&mut pk.ev);
        let original = random.coefficient.layout;
        let context = StoredCosetGraphContextV1 {
            original,
            extension_log: g.extension_log,
            advice_phases: &advice_phases,
            fixed_columns: pk.vk.cs.num_fixed_columns,
            instance_columns: pk.vk.cs.num_instance_columns,
            challenge_phases: &challenge_phases,
        };
        let mut plans = reserve(add(evaluator.lookups.len(), 1)?)?;
        plans.push(prepare_stored_coset_graph_v1(
            &evaluator.custom_gates,
            context,
            scratch_limit,
        )?);
        for graph in &evaluator.lookups {
            plans.push(prepare_stored_coset_graph_v1(
                graph,
                context,
                scratch_limit,
            )?);
        }
        let maximum_fields = plans.iter().map(|p| p.field_count()).max().unwrap_or(0);
        let query_metadata = plans
            .iter()
            .try_fold(0, |n, p| add(n, p.metadata_bytes()))?;
        let planning_temporary = plans
            .iter()
            .map(|p| p.planning_temporary_bytes())
            .max()
            .unwrap_or(0);
        let plan_bytes = mul(plans.capacity(), std::mem::size_of_val(&plans[0]))?;
        let phase_bytes = add(advice_phases.capacity(), challenge_phases.capacity())?;
        let requests = private_requests(&pk, &plans)?;
        admit_working_cache(&g, requests)?;
        let opportunities = mul(g.m, add(mul(g.n.div_ceil(TILE), requests)?, 1)?)?;
        advice.quotient_ordinal_boundary(opportunities)?;
        let mut cache = reserve(g.cache)?;
        cache.resize_with(g.cache, || None);
        let parts = reserve(g.m)?;
        let public = PublicBank::empty(pk.fixed_polys.len(), pk.permutation.polys.len())?;
        let metadata = metadata_bytes::<C, SnapshotOf<P>, P::Writer>(
            &g,
            cache.capacity(),
            parts.capacity(),
            public.fixed.capacity(),
            public.sigma.capacity(),
            phase_bytes,
            plan_bytes,
            query_metadata,
            planning_temporary,
            pk.ev.custom_gates.constants.capacity(),
        )?;
        let logical_fields = add(
            add(mul(2, g.n)?, mul(RELATION_TILES, TILE)?)?,
            add(maximum_fields, pk.vk.cs.num_challenges)?,
        )?;
        if add(
            add(
                metadata,
                mul(logical_fields, std::mem::size_of::<C::Scalar>())?,
            )?,
            mul(TILE, 32)?,
        )? > scratch_limit
        {
            return Err(StoredLookupErrorV1::ScratchLimit);
        }
        let mut accumulator = Fields::zeroed(g.n)?;
        let mut column = Fields::zeroed(g.n)?;
        let mut tiles: [Fields<C::Scalar>; RELATION_TILES] =
            std::array::from_fn(|_| Fields(Vec::new()));
        for tile in &mut tiles {
            *tile = Fields::zeroed(TILE)?;
        }
        let mut challenges = Fields::zeroed(pk.vk.cs.num_challenges)?;
        for (slot, value) in challenges.0.iter_mut().zip(advice.challenges()?) {
            *slot = value;
        }
        let mut workspace = StoredCosetGraphWorkspaceV1::new(maximum_fields, scratch_limit)?;
        let mut encoded = Encoded::new()?;
        let mut actual = add(metadata, add(accumulator.bytes()?, column.bytes()?)?)?;
        actual = add(actual, add(challenges.bytes()?, workspace.scratch_bytes())?)?;
        for tile in &tiles {
            actual = add(actual, tile.bytes()?)?;
        }
        actual = add(actual, mul(encoded.0.capacity(), 32)?)?;
        if actual > scratch_limit {
            return Err(StoredLookupErrorV1::ScratchLimit);
        }
        let mut work = Work {
            params,
            pk,
            evaluator: &evaluator,
            advice: Some(advice),
            provider,
            instances,
            permutations,
            lookups,
            instance_coefficients,
            random,
            public,
            public_moved: false,
            cache,
            cache_limit: g.cache,
            victim: 0,
            parts,
            n: g.n,
            usable: usable_rows,
            extension_log: g.extension_log,
            part: 0,
            old_end: original.ordinal(),
            remaining: opportunities,
        };
        work.validate::<Q, M>()?;
        if work.advice()?.greatest_ordinal()? != Some(original.ordinal()) {
            return Err(StoredPolynomialErrorV1::Context.into());
        }
        work.move_public::<Q, M>()?;
        let beta_value = *beta;
        let gamma_value = *gamma;
        let theta_value = *theta;
        let y_value = *y;
        let width = work.pk.vk.cs.degree() - 2;
        let last_rotation = -i32::try_from(work.pk.vk.cs.blinding_factors() + 1)
            .map_err(|_| StoredLookupErrorV1::Context)?;
        for part in 0..g.m {
            work.part = part as u32;
            work.cache_limit = g.sources.min(
                HANDLES
                    .checked_sub(add(add(g.inherited, part)?, 1)?)
                    .ok_or_else(capacity_error)?,
            );
            work.victim = 0;
            work.validate::<Q, M>()?;
            work.public_part::<Q, M>(false)?;
            clear(&mut accumulator.0);
            // Original custom gate graph folds every polynomial into the zero initial value.
            for start in (0..g.n).step_by(TILE) {
                let tile = StoredRowTileV1 {
                    start,
                    len: TILE.min(g.n - start),
                };
                tiles[0].0[..tile.len].copy_from_slice(&accumulator.0[start..start + tile.len]);
                let (previous, read) = tiles.split_at_mut(RELATION_TILES - 1);
                let mut source = GraphSource::<C, P, Q, M> {
                    work: &mut work,
                    column: &mut column,
                    encoded: &mut encoded,
                    read: &mut read[0].0,
                };
                with_stored_coset_graph_chunk_v1(
                    &plans[0],
                    part as u32,
                    tile,
                    &mut workspace,
                    &mut source,
                    &challenges.0,
                    beta_value,
                    gamma_value,
                    theta_value,
                    y_value,
                    &previous[0].0[..tile.len],
                    |values| {
                        accumulator.0[start..start + tile.len].copy_from_slice(values);
                        Ok(())
                    },
                )?;
            }
            // Finish all boundaries and links before any set relation, at most two product cosets.
            let sets = work.permutations.len();
            if sets != 0 {
                for boundary in 0..2 {
                    let set = if boundary == 0 { 0 } else { sets - 1 };
                    work.prune::<Q, M>(
                        |source| !matches!(source,Source::Copy(s) if s as usize!=set),
                    )?;
                    for start in (0..g.n).step_by(TILE) {
                        let tile = StoredRowTileV1 {
                            start,
                            len: TILE.min(g.n - start),
                        };
                        let slot = work.ensure::<Q, M>(
                            Source::Copy(set as u32),
                            &mut column,
                            &mut encoded,
                        )?;
                        let (values, read) = tiles.split_at_mut(RELATION_TILES - 1);
                        work.read_cached::<Q, M>(
                            slot,
                            0,
                            tile,
                            &mut values[0].0[..tile.len],
                            &mut read[0].0,
                        )?;
                        for row in 0..tile.len {
                            let z = values[0].0[row];
                            let mask = work.public.masks[boundary].fields.0[start + row];
                            let term = if boundary == 0 {
                                (C::Scalar::ONE - z) * mask
                            } else {
                                (z * z - z) * mask
                            };
                            accumulator.0[start + row] =
                                accumulator.0[start + row] * y_value + term;
                        }
                    }
                }
                for set in 1..sets {
                    work.prune::<Q,M>(|source| !matches!(source,Source::Copy(s) if s as usize!=set && s as usize!=set-1))?;
                    for start in (0..g.n).step_by(TILE) {
                        let tile = StoredRowTileV1 {
                            start,
                            len: TILE.min(g.n - start),
                        };
                        let (values, read) = tiles.split_at_mut(RELATION_TILES - 1);
                        let current = work.ensure::<Q, M>(
                            Source::Copy(set as u32),
                            &mut column,
                            &mut encoded,
                        )?;
                        work.read_cached::<Q, M>(
                            current,
                            0,
                            tile,
                            &mut values[0].0[..tile.len],
                            &mut read[0].0,
                        )?;
                        let previous = work.ensure::<Q, M>(
                            Source::Copy((set - 1) as u32),
                            &mut column,
                            &mut encoded,
                        )?;
                        work.read_cached::<Q, M>(
                            previous,
                            last_rotation,
                            tile,
                            &mut values[1].0[..tile.len],
                            &mut read[0].0,
                        )?;
                        for row in 0..tile.len {
                            accumulator.0[start + row] = accumulator.0[start + row] * y_value
                                + (values[0].0[row] - values[1].0[row])
                                    * work.public.masks[0].fields.0[start + row];
                        }
                    }
                }
            }
            work.prune::<Q, M>(|source| !matches!(source, Source::Copy(_)))?;
            // Transform one original sigma set outside every row tile; restore before the next set.
            for set in 0..sets {
                let first = set * width;
                let end = (first + width).min(work.pk.vk.cs.permutation.columns.len());
                work.sigma_part::<Q, M>(first, end, false)?;
                for start in (0..g.n).step_by(TILE) {
                    let tile = StoredRowTileV1 {
                        start,
                        len: TILE.min(g.n - start),
                    };
                    let (values, read) = tiles.split_at_mut(RELATION_TILES - 1);
                    let slot =
                        work.ensure::<Q, M>(Source::Copy(set as u32), &mut column, &mut encoded)?;
                    work.read_cached::<Q, M>(
                        slot,
                        1,
                        tile,
                        &mut values[0].0[..tile.len],
                        &mut read[0].0,
                    )?;
                    work.read_cached::<Q, M>(
                        slot,
                        0,
                        tile,
                        &mut values[1].0[..tile.len],
                        &mut read[0].0,
                    )?;
                    for index in first..end {
                        let original_column = work.pk.vk.cs.permutation.columns[index];
                        match original_column.column_type() {
                            Any::Fixed => work.fixed::<Q, M>(
                                original_column.index(),
                                0,
                                tile,
                                &mut values[2].0[..tile.len],
                            )?,
                            kind => {
                                let source = match kind {
                                    Any::Advice(_) => {
                                        Source::Advice(original_column.index() as u32)
                                    }
                                    Any::Instance => {
                                        Source::Instance(original_column.index() as u32)
                                    }
                                    Any::Fixed => unreachable!(),
                                };
                                let slot =
                                    work.ensure::<Q, M>(source, &mut column, &mut encoded)?;
                                work.read_cached::<Q, M>(
                                    slot,
                                    0,
                                    tile,
                                    &mut values[2].0[..tile.len],
                                    &mut read[0].0,
                                )?;
                            }
                        }
                        let omega = work.pk.vk.domain.get_omega();
                        let mut beta_term = work
                            .pk
                            .vk
                            .domain
                            .get_extended_omega()
                            .pow_vartime([part as u64])
                            * omega.pow_vartime([start as u64]);
                        let delta = beta_value
                            * C::Scalar::ZETA
                            * C::Scalar::DELTA.pow_vartime([index as u64]);
                        let sigma = &work.public.sigma[index];
                        if sigma.part != Some(part as u32) {
                            return Err(StoredLookupErrorV1::Context);
                        }
                        for row in 0..tile.len {
                            let column_value = values[2].0[row];
                            values[0].0[row] *= column_value
                                + beta_value * sigma.fields.0[start + row]
                                + gamma_value;
                            values[1].0[row] *= column_value + delta * beta_term + gamma_value;
                            beta_term *= omega;
                        }
                    }
                    for row in 0..tile.len {
                        accumulator.0[start + row] = accumulator.0[start + row] * y_value
                            + (values[0].0[row] - values[1].0[row])
                                * work.public.masks[2].fields.0[start + row];
                    }
                }
                work.sigma_part::<Q, M>(first, end, true)?;
                work.prune::<Q, M>(|source| !matches!(source, Source::Copy(_)))?;
            }
            // Each lookup graph and its five constraints retain original lookup/Horner order.
            for lookup in 0..work.lookups.len() {
                for start in (0..g.n).step_by(TILE) {
                    let tile = StoredRowTileV1 {
                        start,
                        len: TILE.min(g.n - start),
                    };
                    let (values, read) = tiles.split_at_mut(RELATION_TILES - 1);
                    clear(&mut values[1].0);
                    let mut source = GraphSource::<C, P, Q, M> {
                        work: &mut work,
                        column: &mut column,
                        encoded: &mut encoded,
                        read: &mut read[0].0,
                    };
                    let (result, previous) = values.split_at_mut(1);
                    with_stored_coset_graph_chunk_v1(
                        &plans[lookup + 1],
                        part as u32,
                        tile,
                        &mut workspace,
                        &mut source,
                        &challenges.0,
                        beta_value,
                        gamma_value,
                        theta_value,
                        y_value,
                        &previous[0].0[..tile.len],
                        |output| {
                            result[0].0[..tile.len].copy_from_slice(output);
                            Ok(())
                        },
                    )?;
                    let product = work.ensure::<Q, M>(
                        Source::LookupProduct(lookup as u32),
                        &mut column,
                        &mut encoded,
                    )?;
                    work.read_cached::<Q, M>(
                        product,
                        0,
                        tile,
                        &mut values[2].0[..tile.len],
                        &mut read[0].0,
                    )?;
                    work.read_cached::<Q, M>(
                        product,
                        1,
                        tile,
                        &mut values[3].0[..tile.len],
                        &mut read[0].0,
                    )?;
                    let input = work.ensure::<Q, M>(
                        Source::LookupInput(lookup as u32),
                        &mut column,
                        &mut encoded,
                    )?;
                    work.read_cached::<Q, M>(
                        input,
                        0,
                        tile,
                        &mut values[4].0[..tile.len],
                        &mut read[0].0,
                    )?;
                    work.read_cached::<Q, M>(
                        input,
                        -1,
                        tile,
                        &mut values[5].0[..tile.len],
                        &mut read[0].0,
                    )?;
                    let table = work.ensure::<Q, M>(
                        Source::LookupTable(lookup as u32),
                        &mut column,
                        &mut encoded,
                    )?;
                    work.read_cached::<Q, M>(
                        table,
                        0,
                        tile,
                        &mut values[6].0[..tile.len],
                        &mut read[0].0,
                    )?;
                    for row in 0..tile.len {
                        let index = start + row;
                        let z = values[2].0[row];
                        let next = values[3].0[row];
                        let input = values[4].0[row];
                        let previous = values[5].0[row];
                        let table = values[6].0[row];
                        let l0 = work.public.masks[0].fields.0[index];
                        let last = work.public.masks[1].fields.0[index];
                        let active = work.public.masks[2].fields.0[index];
                        let value = &mut accumulator.0[index];
                        *value = *value * y_value + (C::Scalar::ONE - z) * l0;
                        *value = *value * y_value + (z * z - z) * last;
                        *value = *value * y_value
                            + (next * (input + beta_value) * (table + gamma_value)
                                - z * values[0].0[row])
                                * active;
                        *value = *value * y_value + (input - table) * l0;
                        *value = *value * y_value + (input - table) * (input - previous) * active;
                    }
                }
                work.prune::<Q, M>(|source| {
                    !matches!(
                        source,
                        Source::LookupInput(_) | Source::LookupTable(_) | Source::LookupProduct(_)
                    )
                })?;
            }
            work.public_part::<Q, M>(true)?;
            work.opportunity()?;
            let (writer, layout) = work.create::<Q, M>(None)?;
            let polynomial = work.seal::<Q, M>(writer, layout, &accumulator.0, &mut encoded)?;
            work.parts.push(polynomial);
            work.validate::<Q, M>()?;
            // External cache destructors run while every original and completed output is swept.
            work.prune::<Q, M>(|_| false)?;
            let expected = mul(g.m - part - 1, add(mul(g.n.div_ceil(TILE), requests)?, 1)?)?;
            if work.remaining != expected {
                return Err(StoredLookupErrorV1::Context);
            }
        }
        if work.remaining != 0 || work.parts.len() != g.m || work.cache.iter().any(Option::is_some)
        {
            return Err(StoredLookupErrorV1::Context);
        }
        work.restore_public::<Q, M>()?;
        work.validate::<Q, M>()?;
        drop(plans);
        let Work {
            params,
            mut pk,
            advice,
            provider,
            instances,
            permutations,
            lookups,
            instance_coefficients,
            random,
            parts,
            ..
        } = work;
        pk.ev = evaluator;
        let advice = advice.ok_or(StoredLookupErrorV1::Context)?;
        let inner = VanishingPendingStoredIpaProverV1 {
            params,
            pk,
            advice,
            provider,
            rng,
            transcript,
            instances,
            theta,
            beta,
            gamma,
            y,
            usable_rows,
            permutations,
            lookups,
            instance_coefficients,
            random,
            _challenge,
        };
        Ok(QuotientNumeratorPendingStoredIpaProverV1 { inner, parts })
    }
}
