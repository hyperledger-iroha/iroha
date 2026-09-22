//! Original scalar transcript sequence and an immutable, owner-local opening schedule.
//!
//! The original coefficient, key, parameter, RNG, transcript, snapshot and blind owners survive
//! this consuming transition. H is a recipe over the original quotient pieces, with one derived
//! blind; no extra polynomial handle or permanent H column exists. The scratch accounting covers
//! only this stage's initialized buffers and metadata, not inherited owners, allocator overhead,
//! backend allocations or process RSS. TODO: implement the stored IPA multiopening suffix and
//! exact Core constructor integration before exposing a complete proof entry point.

use super::{
    lookup::StoredLookupErrorV1,
    lookup_permuted::{PermutedPolynomialV1, SecretLookupBlindV1},
    quotient_commitments::QuotientCommitmentsPendingStoredIpaProverV1,
};
use crate::{
    arithmetic::CurveAffine,
    plonk::ProvingKey,
    poly::{
        Rotation,
        commitment::Blind,
        stored_advice::{
            STORED_MAX_K_V1, STORED_SCALARS_PER_CHUNK_V1, StoredPolynomialErrorV1,
            StoredPolynomialProviderV1, StoredPolynomialSnapshotV1,
            assignment::StoredAssignmentFieldV1,
        },
    },
    transcript::{EncodedChallenge, TranscriptWrite},
};
use ff::{Field, PrimeField, WithSmallOrderMulGroup};
use std::{
    ptr,
    sync::atomic::{Ordering, compiler_fence},
};

const TILE: usize = STORED_SCALARS_PER_CHUNK_V1;

/// Logical identity within one closed proof owner; never an independent snapshot authority.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum StoredOpeningSourceV1 {
    /// Original coefficient advice column.
    Advice(usize),
    /// Original instance coefficient column.
    Instance(usize),
    /// Original fixed coefficient column in the retained key.
    Fixed(usize),
    /// Original permutation sigma coefficient column in the retained key.
    Permutation(usize),
    /// Original copy-product coefficient column.
    CopyProduct(usize),
    /// Original lookup-product coefficient column.
    LookupProduct(usize),
    /// Original permuted lookup-input coefficient column.
    LookupInput(usize),
    /// Original permuted lookup-table coefficient column.
    LookupTable(usize),
    /// H, derived lazily by the original reverse quotient-piece fold.
    Quotient,
    /// Original random vanishing polynomial.
    Random,
}

/// Selector for a sole original blind, or the sole derived H blind, without copying it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum StoredOpeningBlindV1 {
    /// Ordinary Blind::default(), which is ONE, for instance, fixed and sigma sources.
    Default,
    /// Original advice blind retained by its phase owner.
    Advice(usize),
    /// Original copy-product blind.
    CopyProduct(usize),
    /// Original lookup-product blind.
    LookupProduct(usize),
    /// Original permuted-input blind.
    LookupInput(usize),
    /// Original permuted-table blind.
    LookupTable(usize),
    /// The single guarded derived H blind.
    Quotient,
    /// Original random vanishing blind.
    Random,
}

/// Immutable query metadata; physical polynomial/blind selection remains with its owner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct StoredOpeningQueryV1<F> {
    source: StoredOpeningSourceV1,
    point: F,
}
impl<F: Copy> StoredOpeningQueryV1<F> {
    /// The owner's logical source identity; repeated queries retain the same identity.
    pub(super) fn source(&self) -> StoredOpeningSourceV1 {
        self.source
    }
    /// Original challenge point with the exact query rotation.
    pub(super) fn point(&self) -> F {
        self.point
    }
    /// The original blind selected by this source, without materializing it.
    pub(super) fn blind_source(&self) -> StoredOpeningBlindV1 {
        use StoredOpeningBlindV1 as B;
        use StoredOpeningSourceV1 as S;
        match self.source {
            S::Advice(i) => B::Advice(i),
            S::Instance(_) | S::Fixed(_) | S::Permutation(_) => B::Default,
            S::CopyProduct(i) => B::CopyProduct(i),
            S::LookupProduct(i) => B::LookupProduct(i),
            S::LookupInput(i) => B::LookupInput(i),
            S::LookupTable(i) => B::LookupTable(i),
            S::Quotient => B::Quotient,
            S::Random => B::Random,
        }
    }
}

/// Complete scalar-write continuation with original owners and one immutable opening plan.
#[allow(dead_code)]
pub(crate) struct ProofEvaluationsPendingStoredIpaProverV1<
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
    inner: QuotientCommitmentsPendingStoredIpaProverV1<'params, 'instances, C, P, R, T, E, Q, M>,
    plan: Vec<StoredOpeningQueryV1<C::Scalar>>,
    h_blind: SecretLookupBlindV1<C::Scalar>,
}

fn add(a: usize, b: usize) -> Result<usize, StoredLookupErrorV1> {
    a.checked_add(b).ok_or(StoredLookupErrorV1::Context)
}
fn mul(a: usize, b: usize) -> Result<usize, StoredLookupErrorV1> {
    a.checked_mul(b).ok_or(StoredLookupErrorV1::Context)
}
fn reserve<V>(count: usize) -> Result<Vec<V>, StoredLookupErrorV1> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| StoredLookupErrorV1::Allocation)?;
    Ok(values)
}

#[cfg(test)]
thread_local! {
    static CLEARS: std::cell::Cell<(usize, bool)> = const { std::cell::Cell::new((0, true)) };
}
/// Observe initialized field slots cleared by this continuation only.
#[cfg(test)]
pub(super) fn take_clear_observations() -> (usize, bool) {
    CLEARS.with(|v| v.replace((0, true)))
}
fn clear<F: StoredAssignmentFieldV1>(values: &mut [F]) {
    for value in values.iter_mut() {
        // SAFETY: these exclusively owned, initialized Copy field elements admit ZERO.
        unsafe { ptr::write_volatile(value, F::ZERO) };
    }
    compiler_fence(Ordering::SeqCst);
    #[cfg(test)]
    CLEARS.with(|v| {
        let (count, zero) = v.get();
        v.set((
            count + values.len(),
            zero && values.iter().all(|v| *v == F::ZERO),
        ));
    });
}
struct Fields<F: StoredAssignmentFieldV1>(Vec<F>);
impl<F: StoredAssignmentFieldV1> Fields<F> {
    fn new(count: usize) -> Result<Self, StoredLookupErrorV1> {
        let mut values = reserve(count)?;
        values.resize(count, F::ZERO);
        Ok(Self(values))
    }
}
impl<F: StoredAssignmentFieldV1> Drop for Fields<F> {
    fn drop(&mut self) {
        clear(&mut self.0);
    }
}
struct Destination<'a, F: StoredAssignmentFieldV1> {
    values: &'a mut [F],
    keep: bool,
}
impl<F: StoredAssignmentFieldV1> Drop for Destination<'_, F> {
    fn drop(&mut self) {
        if !self.keep {
            clear(self.values);
        }
    }
}

#[derive(Clone, Copy)]
struct Schedule {
    n: usize,
    instance_end: usize,
    advice_end: usize,
    copy_sets: usize,
    lookup_start: usize,
    fixed_start: usize,
    sigma_start: usize,
    h: usize,
    total: usize,
    batch: usize,
}
impl Schedule {
    fn for_key<C: CurveAffine, const Q: bool, const M: u64>(
        pk: &ProvingKey<C>,
    ) -> Result<Self, StoredLookupErrorV1> {
        let cs = &pk.vk.cs;
        if pk.vk.domain.k() > STORED_MAX_K_V1
            || cs.degree() < 3
            || (!Q && M != 0)
            || (cs.num_instance_columns < 64 && M >> cs.num_instance_columns != 0)
            || cs.advice_column_phase.len() != cs.num_advice_columns
            || cs.challenge_phase.len() != cs.num_challenges
            || cs.advice_column_phase.iter().any(|p| p.to_u8() != 0)
            || cs.challenge_phase.iter().any(|p| p.to_u8() != 0)
            || cs
                .instance_queries
                .iter()
                .any(|(c, _)| c.index() >= cs.num_instance_columns)
            || cs
                .advice_queries
                .iter()
                .any(|(c, _)| c.index() >= cs.num_advice_columns || c.column_type().phase() != 0)
            || cs
                .fixed_queries
                .iter()
                .any(|(c, _)| c.index() >= cs.num_fixed_columns)
        {
            return Err(StoredLookupErrorV1::Context);
        }
        let n = 1_usize
            .checked_shl(pk.vk.domain.k())
            .ok_or(StoredLookupErrorV1::Context)?;
        if pk.vk.domain.get_n() != n as u64 {
            return Err(StoredLookupErrorV1::Context);
        }
        // The ordinary last rotation uses i32; reject an unrepresentable rotation rather than
        // retaining an accidental wrapping cast in this closed continuation.
        i32::try_from(add(cs.blinding_factors(), 1)?).map_err(|_| StoredLookupErrorV1::Context)?;
        let instance_end = if Q { cs.instance_queries.len() } else { 0 };
        let advice_end = add(instance_end, cs.advice_queries.len())?;
        let copy_sets = cs.permutation.columns.len().div_ceil(cs.degree() - 2);
        let copy_queries = add(mul(2, copy_sets)?, copy_sets.saturating_sub(1))?;
        let lookup_start = add(advice_end, copy_queries)?;
        let fixed_start = add(lookup_start, mul(5, cs.lookups.len())?)?;
        let sigma_start = add(fixed_start, cs.fixed_queries.len())?;
        let h = add(sigma_start, cs.permutation.columns.len())?;
        let total = add(h, 2)?;
        let batch = instance_end
            .max(cs.advice_queries.len())
            .max(cs.fixed_queries.len())
            .max(if copy_sets == 0 { 0 } else { 3 })
            .max(if cs.lookups.is_empty() { 0 } else { 5 })
            .max(1);
        Ok(Self {
            n,
            instance_end,
            advice_end,
            copy_sets,
            lookup_start,
            fixed_start,
            sigma_start,
            h,
            total,
            batch,
        })
    }
}

struct Workspace<F: StoredAssignmentFieldV1> {
    column: Fields<F>,
    batch: Fields<F>,
}
fn payload<F: StoredAssignmentFieldV1>(
    column_capacity: usize,
    batch_capacity: usize,
    plan_capacity: usize,
) -> Result<usize, StoredLookupErrorV1> {
    add(
        add(
            mul(
                add(column_capacity, batch_capacity)?,
                std::mem::size_of::<F>(),
            )?,
            mul(
                plan_capacity,
                std::mem::size_of::<StoredOpeningQueryV1<F>>(),
            )?,
        )?,
        add(
            std::mem::size_of::<Workspace<F>>(),
            add(
                std::mem::size_of::<Vec<StoredOpeningQueryV1<F>>>(),
                add(
                    std::mem::size_of::<SecretLookupBlindV1<F>>(),
                    std::mem::size_of::<Schedule>(),
                )?,
            )?,
        )?,
    )
}
/// Minimum new stage payload; actual vector capacities are admitted again before reads or writes.
pub(super) fn scratch_bytes<C, P, const Q: bool, const M: u64>(
    pk: &ProvingKey<C>,
) -> Result<usize, StoredLookupErrorV1>
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1,
    P: StoredPolynomialProviderV1,
{
    let s = Schedule::for_key::<C, Q, M>(pk)?;
    payload::<C::Scalar>(s.n, s.batch, s.total)
}

fn opening_plan<C: CurveAffine, const Q: bool, const M: u64>(
    pk: &ProvingKey<C>,
    x: C::Scalar,
    s: Schedule,
) -> Result<Vec<StoredOpeningQueryV1<C::Scalar>>, StoredLookupErrorV1> {
    use StoredOpeningSourceV1 as S;
    let mut plan = reserve(s.total)?;
    let cs = &pk.vk.cs;
    let d = &pk.vk.domain;
    let x_next = d.rotate_omega(x, Rotation::next());
    let x_prev = d.rotate_omega(x, Rotation::prev());
    let last =
        i32::try_from(add(cs.blinding_factors(), 1)?).map_err(|_| StoredLookupErrorV1::Context)?;
    let x_last = d.rotate_omega(x, Rotation(-last));
    let mut push = |source, point| plan.push(StoredOpeningQueryV1 { source, point });
    if Q {
        for &(c, rotation) in &cs.instance_queries {
            push(S::Instance(c.index()), d.rotate_omega(x, rotation));
        }
    }
    for &(c, rotation) in &cs.advice_queries {
        push(S::Advice(c.index()), d.rotate_omega(x, rotation));
    }
    for set in 0..s.copy_sets {
        push(S::CopyProduct(set), x);
        push(S::CopyProduct(set), x_next);
    }
    // The ordinary opening iterator emits these last-point queries in reverse set order.
    for set in (0..s.copy_sets.saturating_sub(1)).rev() {
        push(S::CopyProduct(set), x_last);
    }
    for lookup in 0..cs.lookups.len() {
        push(S::LookupProduct(lookup), x);
        push(S::LookupInput(lookup), x);
        push(S::LookupTable(lookup), x);
        push(S::LookupInput(lookup), x_prev);
        push(S::LookupProduct(lookup), x_next);
    }
    for &(c, rotation) in &cs.fixed_queries {
        push(S::Fixed(c.index()), d.rotate_omega(x, rotation));
    }
    for column in 0..cs.permutation.columns.len() {
        push(S::Permutation(column), x);
    }
    push(S::Quotient, x);
    push(S::Random, x);
    if plan.len() != s.total {
        return Err(StoredLookupErrorV1::Context);
    }
    Ok(plan)
}

fn read_snapshot<F: StoredAssignmentFieldV1, S: StoredPolynomialSnapshotV1>(
    value: &mut PermutedPolynomialV1<S>,
    chunk: usize,
    output: &mut [F],
) -> Result<(), StoredLookupErrorV1> {
    let expected = value.layout;
    if value.snapshot.layout() != expected
        || output.len() != expected.chunk_scalar_count(chunk as u64)?
    {
        return Err(StoredPolynomialErrorV1::Context.into());
    }
    let mut decoded = false;
    value.snapshot.with_chunk(expected, chunk as u64, |bytes| {
        if decoded
            || bytes.len() != output.len()
            || bytes.iter().any(|b| !expected.field().is_canonical(b))
        {
            return Err(StoredPolynomialErrorV1::Encoding);
        }
        for (out, bytes) in output.iter_mut().zip(bytes) {
            *out =
                Option::<F>::from(F::from_repr(*bytes)).ok_or(StoredPolynomialErrorV1::Encoding)?;
        }
        decoded = true;
        Ok(())
    })?;
    if !decoded || value.snapshot.layout() != expected {
        return Err(StoredPolynomialErrorV1::Context.into());
    }
    Ok(())
}

impl<'params, 'instances, C, P, R, T, E, const Q: bool, const M: u64>
    ProofEvaluationsPendingStoredIpaProverV1<'params, 'instances, C, P, R, T, E, Q, M>
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3>,
    P: StoredPolynomialProviderV1,
{
    fn validate(&self) -> Result<Schedule, StoredLookupErrorV1> {
        self.inner.validate_evaluation_owner()?;
        let s = Schedule::for_key::<C, Q, M>(&self.inner.inner.inner.pk)?;
        if self.plan.len() != s.total {
            return Err(StoredLookupErrorV1::Context);
        }
        Ok(s)
    }

    /// Original opening order; no mutable plan, blind, snapshot or key accessor is exposed.
    pub(super) fn opening_plan(
        &self,
    ) -> Result<&[StoredOpeningQueryV1<C::Scalar>], StoredLookupErrorV1> {
        self.validate()?;
        Ok(&self.plan)
    }

    // The only source selectors accepted here are from the private retained plan or a retained
    // quotient-piece index. This method consumes self so advice's sole owner crosses every read.
    fn copy_source_chunk(
        mut self,
        source: StoredOpeningSourceV1,
        chunk: usize,
        output: &mut [C::Scalar],
    ) -> Result<Self, StoredLookupErrorV1> {
        use StoredOpeningSourceV1 as S;
        let s = self.validate()?;
        let start = mul(chunk, TILE)?;
        let end = add(start, output.len())?;
        if start >= s.n || end > s.n || output.len() != (s.n - start).min(TILE) {
            return Err(StoredLookupErrorV1::Context);
        }
        if let S::Advice(column) = source {
            self.inner.inner.inner.advice =
                self.inner.inner.inner.advice.copy_coefficient_chunk_into(
                    u32::try_from(column).map_err(|_| StoredLookupErrorV1::Context)?,
                    chunk as u64,
                    output,
                )?;
        } else {
            let inner = &mut self.inner.inner.inner;
            match source {
                S::Fixed(column) => output.copy_from_slice(
                    inner
                        .pk
                        .fixed_polys
                        .get(column)
                        .ok_or(StoredLookupErrorV1::Context)?
                        .values
                        .get(start..end)
                        .ok_or(StoredLookupErrorV1::Context)?,
                ),
                S::Permutation(column) => output.copy_from_slice(
                    inner
                        .pk
                        .permutation
                        .polys
                        .get(column)
                        .ok_or(StoredLookupErrorV1::Context)?
                        .values
                        .get(start..end)
                        .ok_or(StoredLookupErrorV1::Context)?,
                ),
                _ => {
                    let value = match source {
                        S::Instance(i) => inner.instance_coefficients.get_mut(i),
                        S::CopyProduct(i) => {
                            inner.permutations.get_mut(i).map(|v| &mut v.coefficient)
                        }
                        S::LookupProduct(i) => {
                            inner.lookups.get_mut(i).map(|v| &mut v.product.coefficient)
                        }
                        S::LookupInput(i) => {
                            inner.lookups.get_mut(i).map(|v| &mut v.input.coefficient)
                        }
                        S::LookupTable(i) => {
                            inner.lookups.get_mut(i).map(|v| &mut v.table.coefficient)
                        }
                        S::Random => Some(&mut inner.random.coefficient),
                        _ => None,
                    }
                    .ok_or(StoredLookupErrorV1::Context)?;
                    read_snapshot(value, chunk, output)?;
                }
            }
        }
        self.validate()?;
        Ok(self)
    }

    fn copy_source(
        mut self,
        source: StoredOpeningSourceV1,
        output: &mut [C::Scalar],
    ) -> Result<Self, StoredLookupErrorV1> {
        let s = self.validate()?;
        if output.len() != s.n {
            return Err(StoredLookupErrorV1::Context);
        }
        for (chunk, values) in output.chunks_mut(TILE).enumerate() {
            self = self.copy_source_chunk(source, chunk, values)?;
        }
        self.validate()?;
        Ok(self)
    }

    /// Copy one source selected solely by an index in this owner's immutable opening schedule.
    ///
    /// An exact n-field destination is required; all supplied initialized slots are cleared on
    /// error or unwind, including invalid index/length and late receipt failures. Success retains
    /// the output with its caller. H uses one bounded tile, folds pieces in ordinary reverse order,
    /// and creates no store handle, ordinal, commitment, transcript item or proof randomness.
    pub(super) fn copy_opening_coefficients(
        mut self,
        query_index: usize,
        output: &mut [C::Scalar],
    ) -> Result<Self, StoredLookupErrorV1> {
        let mut destination = Destination {
            values: output,
            keep: false,
        };
        let s = self.validate()?;
        if destination.values.len() != s.n {
            return Err(StoredLookupErrorV1::Context);
        }
        let source = self
            .plan
            .get(query_index)
            .ok_or(StoredLookupErrorV1::Context)?
            .source;
        if source == StoredOpeningSourceV1::Quotient {
            let mut tile = Fields::<C::Scalar>::new(s.n.min(TILE))?;
            // There is no retained capacity beyond this method. Reject an allocator excess before
            // any read; this bounds its only added field allocation by one physical chunk.
            if tile.0.capacity() > s.n.min(TILE) {
                return Err(StoredLookupErrorV1::ScratchLimit);
            }
            clear(destination.values);
            let xn = self.inner.x.pow([s.n as u64]);
            for piece in (0..self.inner.inner.pieces.len()).rev() {
                for (chunk, values) in destination.values.chunks_mut(TILE).enumerate() {
                    self.validate()?;
                    let length = values.len();
                    read_snapshot(
                        &mut self.inner.inner.pieces[piece],
                        chunk,
                        &mut tile.0[..length],
                    )?;
                    self.validate()?;
                    for (value, coefficient) in values.iter_mut().zip(&tile.0[..length]) {
                        *value = *value * xn + coefficient;
                    }
                    clear(&mut tile.0);
                }
            }
            self.validate()?;
        } else {
            self = self.copy_source(source, destination.values)?;
        }
        self.validate()?;
        destination.keep = true;
        Ok(self)
    }

    #[cfg(test)]
    pub(super) fn observed_inner(
        &self,
    ) -> &QuotientCommitmentsPendingStoredIpaProverV1<'params, 'instances, C, P, R, T, E, Q, M>
    {
        &self.inner
    }
    #[cfg(test)]
    pub(super) fn observed_h_blind(&self) -> Blind<C::Scalar> {
        self.h_blind.0
    }
}

impl<'params, 'instances, C, P, R, T, E, const Q: bool, const M: u64>
    ProofEvaluationsPendingStoredIpaProverV1<'params, 'instances, C, P, R, T, E, Q, M>
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3>,
    P: StoredPolynomialProviderV1,
    E: EncodedChallenge<C>,
    T: TranscriptWrite<C, E>,
{
    fn write_batch(
        mut self,
        queries: impl IntoIterator<Item = usize>,
        work: &mut Workspace<C::Scalar>,
    ) -> Result<Self, StoredLookupErrorV1> {
        self.validate()?;
        let mut count = 0;
        for query in queries {
            let request = *self.plan.get(query).ok_or(StoredLookupErrorV1::Context)?;
            if request.source == StoredOpeningSourceV1::Quotient || count >= work.batch.0.len() {
                return Err(StoredLookupErrorV1::Context);
            }
            self = self.copy_source(request.source, &mut work.column.0)?;
            // Serial Horner is algebraically identical over the exact prime field and avoids the
            // ordinary helper's extra thread-count allocation. Every initialized result is guarded.
            let result = &mut work.batch.0[count];
            *result = C::Scalar::ZERO;
            for coefficient in work.column.0.iter().rev() {
                *result = *result * request.point + coefficient;
            }
            clear(&mut work.column.0);
            self.validate()?;
            count += 1;
        }
        for index in 0..count {
            self.validate()?;
            self.inner
                .inner
                .inner
                .transcript
                .write_scalar(work.batch.0[index])
                .map_err(|_| StoredLookupErrorV1::Transcript)?;
            self.validate()?;
        }
        clear(&mut work.batch.0);
        self.validate()?;
        Ok(self)
    }
}

impl<'params, 'instances, C, P, R, T, E, const Q: bool, const M: u64>
    QuotientCommitmentsPendingStoredIpaProverV1<'params, 'instances, C, P, R, T, E, Q, M>
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3>,
    P: StoredPolynomialProviderV1,
    E: EncodedChallenge<C>,
    T: TranscriptWrite<C, E>,
{
    /// Consume the original x owner through exactly the ordinary scalar transcript sequence.
    ///
    /// Scalar order and opening-query order differ for copy and lookup arguments. The immutable
    /// plan records opening order; this transition indexes it in the original scalar batches.
    /// It performs no proof-RNG draw, point write, challenge squeeze, provider write or new handle.
    pub(crate) fn evaluate_and_plan(
        self,
        scratch_limit_bytes: usize,
    ) -> Result<
        ProofEvaluationsPendingStoredIpaProverV1<'params, 'instances, C, P, R, T, E, Q, M>,
        StoredLookupErrorV1,
    > {
        self.validate_evaluation_owner()?;
        let s = Schedule::for_key::<C, Q, M>(&self.inner.inner.pk)?;
        if payload::<C::Scalar>(s.n, s.batch, s.total)? > scratch_limit_bytes {
            return Err(StoredLookupErrorV1::ScratchLimit);
        }
        let plan = opening_plan::<C, Q, M>(&self.inner.inner.pk, *self.x, s)?;
        let mut work = Workspace {
            column: Fields::new(s.n)?,
            batch: Fields::new(s.batch)?,
        };
        if payload::<C::Scalar>(
            work.column.0.capacity(),
            work.batch.0.capacity(),
            plan.capacity(),
        )? > scratch_limit_bytes
        {
            return Err(StoredLookupErrorV1::ScratchLimit);
        }
        let mut owner = ProofEvaluationsPendingStoredIpaProverV1 {
            inner: self,
            plan,
            h_blind: SecretLookupBlindV1(Blind(C::Scalar::ZERO)),
        };
        owner.validate()?;
        owner = owner.write_batch(0..s.instance_end, &mut work)?;
        owner = owner.write_batch(s.instance_end..s.advice_end, &mut work)?;
        owner = owner.write_batch(s.fixed_start..s.sigma_start, &mut work)?;
        // Ordinary vanishing.evaluate derives H/blind here, then writes only random(x), not H(x).
        let xn = owner.inner.x.pow([s.n as u64]);
        for blind in owner.inner.blinds.iter().rev() {
            owner.h_blind.0 = owner.h_blind.0 * Blind(xn) + blind.0;
        }
        owner.validate()?;
        owner = owner.write_batch([s.h + 1], &mut work)?;
        for index in s.sigma_start..s.h {
            owner = owner.write_batch([index], &mut work)?;
        }
        for set in 0..s.copy_sets {
            let start = s.advice_end + 2 * set;
            // Ordinary copy evaluates (x, next), writes both, and only then evaluates/writes last.
            owner = owner.write_batch([start, start + 1], &mut work)?;
            if set + 1 < s.copy_sets {
                let last = s.advice_end + 2 * s.copy_sets + (s.copy_sets - 2 - set);
                owner = owner.write_batch([last], &mut work)?;
            }
        }
        for start in (s.lookup_start..s.fixed_start).step_by(5) {
            owner = owner.write_batch(
                [start, start + 4, start + 1, start + 3, start + 2],
                &mut work,
            )?;
        }
        owner.validate()?;
        // Drop initialized scalar scratch before final receipt sweep; every external owner remains.
        drop(work);
        owner.validate()?;
        Ok(owner)
    }
}
