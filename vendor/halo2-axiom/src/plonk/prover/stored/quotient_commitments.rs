//! Original quotient blind/commitment/write sequence and ChallengeX over retained snapshots.
//!
//! All original proof owners remain attached. The scratch bound covers only this continuation;
//! inherited key/storage/MSM allocations and complete proof qualification remain separate work.
//! TODO: qualify the complete stored proof and exact authenticated Core integration.

use super::{
    lookup::StoredLookupErrorV1,
    lookup_permuted::{PermutedPolynomialV1, SecretLookupBlindV1},
    quotient_inverse::QuotientCoefficientsPendingStoredIpaProverV1,
};
use crate::{
    arithmetic::CurveAffine,
    plonk::{ChallengeX, ProvingKey, circuit::Any},
    poly::{
        Coeff, EvaluationDomain, Polynomial,
        commitment::{Blind, Params, ParamsProver},
        stored_advice::{
            STORED_MAX_K_V1, STORED_SCALARS_PER_CHUNK_V1, StoredLookupSideV1,
            StoredPolynomialBasisV1, StoredPolynomialErrorV1, StoredPolynomialLayoutV1,
            StoredPolynomialProviderV1, StoredPolynomialRoleV1, StoredPolynomialSnapshotV1,
            StoredPolynomialWriterV1, assignment::StoredAssignmentFieldV1,
        },
    },
    transcript::{EncodedChallenge, TranscriptWrite},
};
use ff::{Field, PrimeField, WithSmallOrderMulGroup};
use group::{Curve, Group};
use rand_core::RngCore;
use std::{
    ptr,
    sync::atomic::{Ordering, compiler_fence},
};

const TILE: usize = STORED_SCALARS_PER_CHUNK_V1;
type SnapshotOf<P> =
    <<P as StoredPolynomialProviderV1>::Writer as StoredPolynomialWriterV1>::Snapshot;

/// Closed original coefficient owner, sole quotient blinds, ordinary points and ChallengeX.
#[allow(dead_code)]
pub(crate) struct QuotientCommitmentsPendingStoredIpaProverV1<
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
    pub(super) inner:
        QuotientCoefficientsPendingStoredIpaProverV1<'params, 'instances, C, P, R, T, E, Q, M>,
    pub(super) blinds: Vec<SecretLookupBlindV1<C::Scalar>>,
    pub(super) commitments: Vec<C>,
    pub(super) x: ChallengeX<C>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct Geometry {
    n: usize,
    m: usize,
    q: usize,
    e: u32,
    inherited: usize,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Sampling,
    Committing,
    Normalizing,
    Writing,
    Squeezing,
    Finished,
}
struct Progress {
    phase: Phase,
    sampled: usize,
    committed: usize,
    written: usize,
    greatest: u64,
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum StageEvent {
    SampleStart(usize),
    SampleEnd(usize),
    ReadStart(usize, usize),
    ReadEnd(usize, usize),
    CommitStart(usize),
    CommitEnd(usize),
    NormalizeStart,
    NormalizeEnd,
    WriteStart(usize),
    WriteEnd(usize),
    SqueezeStart,
    SqueezeEnd,
}
#[cfg(test)]
thread_local! {
    static CLEARS: std::cell::Cell<(usize,bool)> = const { std::cell::Cell::new((0,true)) };
    static REUSE: std::cell::Cell<(usize,usize,usize,usize,usize)> = const { std::cell::Cell::new((0,0,0,0,0)) };
    static EVENTS: std::cell::RefCell<Vec<StageEvent>> = const { std::cell::RefCell::new(Vec::new()) };
}
#[cfg(test)]
pub(super) fn take_clear_observations() -> (usize, bool) {
    CLEARS.with(|v| v.replace((0, true)))
}
#[cfg(test)]
pub(super) fn take_reuse_observations() -> (usize, usize, usize, usize, usize) {
    REUSE.with(|v| v.replace((0, 0, 0, 0, 0)))
}
#[cfg(test)]
pub(super) fn take_stage_observations() -> Vec<StageEvent> {
    EVENTS.with(|v| std::mem::take(&mut *v.borrow_mut()))
}
#[cfg(test)]
fn event(value: StageEvent) {
    EVENTS.with(|v| v.borrow_mut().push(value));
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
struct Column<F: StoredAssignmentFieldV1>(Polynomial<F, Coeff>);
impl<F: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3>> Column<F> {
    fn new(domain: &EvaluationDomain<F>, n: usize) -> Result<Self, StoredLookupErrorV1> {
        let mut values = reserve(n)?;
        values.resize(n, F::ZERO);
        #[cfg(test)]
        REUSE.with(|v| {
            let (a, _, _, _, c) = v.get();
            let p = values.as_ptr() as usize;
            v.set((a + 1, values.capacity(), p, p, c));
        });
        Ok(Self(domain.coeff_from_vec(values)))
    }
}
impl<F: StoredAssignmentFieldV1> Column<F> {
    fn clear(&mut self) {
        for value in &mut self.0.values {
            // SAFETY: exclusively owned initialized Copy Pasta fields admit ZERO.
            unsafe { ptr::write_volatile(value, F::ZERO) };
        }
        compiler_fence(Ordering::SeqCst);
        #[cfg(test)]
        CLEARS.with(|v| {
            let (n, z) = v.get();
            v.set((n + self.0.len(), z && self.0.iter().all(|x| *x == F::ZERO)));
        });
    }
}
impl<F: StoredAssignmentFieldV1> Drop for Column<F> {
    fn drop(&mut self) {
        self.clear();
    }
}
struct Workspace<C: CurveAffine>
where
    C::Scalar: StoredAssignmentFieldV1,
{
    column: Column<C::Scalar>,
    blinds: Vec<SecretLookupBlindV1<C::Scalar>>,
    projective: Vec<C::Curve>,
    affine: Vec<C>,
    geometry: Geometry,
    progress: Progress,
    x: Option<ChallengeX<C>>,
}
fn payload<C: CurveAffine>(
    fields: usize,
    blinds: usize,
    projective: usize,
    affine: usize,
) -> Result<usize, StoredLookupErrorV1>
where
    C::Scalar: StoredAssignmentFieldV1,
{
    let arrays = add(
        add(
            mul(fields, std::mem::size_of::<C::Scalar>())?,
            mul(
                blinds,
                std::mem::size_of::<SecretLookupBlindV1<C::Scalar>>(),
            )?,
        )?,
        add(
            mul(projective, std::mem::size_of::<C::Curve>())?,
            mul(affine, std::mem::size_of::<C>())?,
        )?,
    )?;
    add(arrays, std::mem::size_of::<Workspace<C>>())
}
fn geometry<C: CurveAffine>(pk: &ProvingKey<C>) -> Result<Geometry, StoredLookupErrorV1> {
    let d = &pk.vk.domain;
    let cs = &pk.vk.cs;
    if d.k() > STORED_MAX_K_V1 || d.extended_k() > STORED_MAX_K_V1 || cs.degree() < 3 {
        return Err(StoredLookupErrorV1::Context);
    }
    let e = d
        .extended_k()
        .checked_sub(d.k())
        .filter(|e| *e != 0)
        .ok_or(StoredLookupErrorV1::Context)?;
    let n = 1_usize << d.k();
    let m = 1_usize << e;
    let q = d.get_quotient_poly_degree();
    if d.get_n() != n as u64
        || q == 0
        || q > m
        || q != cs.degree() - 1
        || d.extended_len() != mul(n, m)?
    {
        return Err(StoredLookupErrorV1::Context);
    }
    for count in [
        cs.num_advice_columns,
        cs.num_instance_columns,
        cs.num_fixed_columns,
        cs.lookups.len(),
        cs.permutation.columns.len(),
        q,
    ] {
        u32::try_from(count).map_err(|_| StoredLookupErrorV1::Context)?;
    }
    let inherited = add(
        add(
            add(cs.num_advice_columns, cs.num_instance_columns)?,
            cs.permutation.columns.len().div_ceil(cs.degree() - 2),
        )?,
        add(mul(3, cs.lookups.len())?, 1)?,
    )?;
    if add(inherited, q)? > 512 {
        return Err(StoredPolynomialErrorV1::Capacity.into());
    }
    Ok(Geometry {
        n,
        m,
        q,
        e,
        inherited,
    })
}
/// Logical new payload; actual capacities are checked again before any entropy or I/O.
pub(super) fn scratch_bytes<C, P>(pk: &ProvingKey<C>) -> Result<usize, StoredLookupErrorV1>
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1,
    P: StoredPolynomialProviderV1,
{
    let g = geometry(pk)?;
    payload::<C>(g.n, g.q, g.q, g.q)
}

fn validate_owner<'params, 'instances, C, P, R, T, E, const Q: bool, const M: u64>(
    owner: &QuotientCoefficientsPendingStoredIpaProverV1<'params, 'instances, C, P, R, T, E, Q, M>,
    expected: Geometry,
    greatest: u64,
) -> Result<(), StoredLookupErrorV1>
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3>,
    P: StoredPolynomialProviderV1,
{
    let inner = &owner.inner;
    let cs = &inner.pk.vk.cs;
    let d = &inner.pk.vk.domain;
    let g = geometry(&inner.pk)?;
    if g != expected
        || owner.pieces.len() != g.q
        || inner.params.k() != d.k()
        || inner.params.n() != g.n as u64
        || inner.params.get_g().len() != g.n
        || inner.params.get_g_lagrange().len() != g.n
        || inner.pk.vk.cs_degree != cs.degree()
        || cs
            .blinding_factors()
            .checked_add(1)
            .and_then(|b| g.n.checked_sub(b))
            != Some(inner.usable_rows)
        || cs.advice_column_phase.len() != cs.num_advice_columns
        || cs.challenge_phase.len() != cs.num_challenges
        || cs.permutation.columns.len().div_ceil(cs.degree() - 2) != inner.permutations.len()
        || cs.lookups.len() != inner.lookups.len()
        || cs.lookups.len() != inner.pk.ev.lookups.len()
        || inner.pk.fixed_polys.len() != cs.num_fixed_columns
        || inner.pk.permutation.polys.len() != cs.permutation.columns.len()
        || !inner.pk.fixed_values.is_empty()
        || !inner.pk.permutation.permutations.is_empty()
        || inner
            .pk
            .fixed_polys
            .iter()
            .chain(&inner.pk.permutation.polys)
            .any(|p| p.len() != g.n)
        || inner.pk.l0.len() != g.n
        || inner.pk.l_last.len() != g.n
        || inner.pk.l_active_row.len() != g.n
        || inner.instances.len() != cs.num_instance_columns
        || inner.instance_coefficients.len() != cs.num_instance_columns
        || inner.instances.iter().any(|v| v.len() > inner.usable_rows)
        || (!Q && M != 0)
        || (cs.num_instance_columns < 64 && M >> cs.num_instance_columns != 0)
    {
        return Err(StoredLookupErrorV1::Context);
    }
    for (index, column) in cs.permutation.columns.iter().enumerate() {
        let valid = match column.column_type() {
            Any::Advice(kind) => cs
                .advice_column_phase
                .get(column.index())
                .is_some_and(|phase| phase.to_u8() == kind.phase()),
            Any::Fixed => column.index() < cs.num_fixed_columns,
            Any::Instance => column.index() < cs.num_instance_columns,
        };
        if !valid || cs.permutation.columns[..index].contains(column) {
            return Err(StoredLookupErrorV1::Context);
        }
    }
    inner.advice.validate_live_receipts()?;
    if !ptr::eq(inner.advice.params()?, inner.params)
        || inner.advice.layouts()?.len() != cs.num_advice_columns
        || inner.advice.challenges()?.count() != cs.num_challenges
        || inner.advice.greatest_ordinal()? != Some(greatest)
        || inner.advice.proof_context()?.is_none_or(|context| {
            StoredPolynomialLayoutV1::new(
                context,
                inner.random.coefficient.layout.ordinal(),
                C::Scalar::STORED_FIELD,
                StoredPolynomialBasisV1::Coefficient,
                d.k(),
                StoredPolynomialRoleV1::VanishingRandom,
            )
            .map_or(true, |layout| layout != inner.random.coefficient.layout)
        })
    {
        return Err(StoredPolynomialErrorV1::Context.into());
    }
    let mut previous = None;
    for (index, (layout, phase)) in inner
        .advice
        .layouts()?
        .zip(&cs.advice_column_phase)
        .enumerate()
    {
        if layout.advice_coordinates()? != (index as u32, phase.to_u8())
            || previous.is_some_and(|last| layout.ordinal() <= last)
        {
            return Err(StoredPolynomialErrorV1::Context.into());
        }
        previous = Some(layout.ordinal());
    }
    let original = inner.random.coefficient.layout;
    let mut check =
        |value: &PermutedPolynomialV1<SnapshotOf<P>>, role| -> Result<(), StoredLookupErrorV1> {
            let layout = value.layout;
            if value.snapshot.layout() != layout
                || layout.role() != role
                || layout.basis() != StoredPolynomialBasisV1::Coefficient
                || layout.field() != C::Scalar::STORED_FIELD
                || layout.k() != d.k()
                || !layout.same_proof_context(original)
                || previous.is_some_and(|last| layout.ordinal() <= last)
            {
                return Err(StoredPolynomialErrorV1::Context.into());
            }
            previous = Some(layout.ordinal());
            Ok(())
        };
    for (index, lookup) in inner.lookups.iter().enumerate() {
        check(
            &lookup.input.coefficient,
            StoredPolynomialRoleV1::LookupPermuted {
                lookup: index as u32,
                side: StoredLookupSideV1::Input,
            },
        )?;
        check(
            &lookup.table.coefficient,
            StoredPolynomialRoleV1::LookupPermuted {
                lookup: index as u32,
                side: StoredLookupSideV1::Table,
            },
        )?;
    }
    for (index, product) in inner.permutations.iter().enumerate() {
        check(
            &product.coefficient,
            StoredPolynomialRoleV1::CopyPermutationProduct { set: index as u32 },
        )?;
    }
    for (index, lookup) in inner.lookups.iter().enumerate() {
        check(
            &lookup.product.coefficient,
            StoredPolynomialRoleV1::LookupProduct {
                lookup: index as u32,
            },
        )?;
    }
    for (index, instance) in inner.instance_coefficients.iter().enumerate() {
        check(
            instance,
            StoredPolynomialRoleV1::Instance {
                column: index as u32,
            },
        )?;
    }
    check(
        &inner.random.coefficient,
        StoredPolynomialRoleV1::VanishingRandom,
    )?;
    for (index, piece) in owner.pieces.iter().enumerate() {
        check(
            piece,
            StoredPolynomialRoleV1::QuotientPiece {
                piece: index as u32,
            },
        )?;
    }
    if previous != Some(greatest) {
        return Err(StoredPolynomialErrorV1::Context.into());
    }
    Ok(())
}

impl<C: CurveAffine> Workspace<C>
where
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3>,
{
    fn new(pk: &ProvingKey<C>, g: Geometry, greatest: u64) -> Result<Self, StoredLookupErrorV1> {
        let column = Column::new(&pk.vk.domain, g.n)?;
        let mut blinds = reserve(g.q)?;
        blinds.resize_with(g.q, || SecretLookupBlindV1(Blind(C::Scalar::ZERO)));
        let mut projective = reserve(g.q)?;
        projective.resize(g.q, C::Curve::identity());
        let mut affine = reserve(g.q)?;
        affine.resize(g.q, C::identity());
        Ok(Self {
            column,
            blinds,
            projective,
            affine,
            geometry: g,
            progress: Progress {
                phase: Phase::Sampling,
                sampled: 0,
                committed: 0,
                written: 0,
                greatest,
            },
            x: None,
        })
    }
    fn actual_payload(&self) -> Result<usize, StoredLookupErrorV1> {
        payload::<C>(
            self.column.0.values.capacity(),
            self.blinds.capacity(),
            self.projective.capacity(),
            self.affine.capacity(),
        )
    }
    fn validate_progress(&self) -> Result<(), StoredLookupErrorV1> {
        let q = self.geometry.q;
        let p = &self.progress;
        if self.column.0.len() != self.geometry.n
            || self.blinds.len() != q
            || self.affine.len() != q
            || p.sampled > q
            || p.committed > p.sampled
            || p.written > p.committed
        {
            return Err(StoredLookupErrorV1::Context);
        }
        let valid = match p.phase {
            Phase::Sampling => {
                p.committed == 0 && p.written == 0 && self.projective.len() == q && self.x.is_none()
            }
            Phase::Committing => {
                p.sampled == q && p.written == 0 && self.projective.len() == q && self.x.is_none()
            }
            Phase::Normalizing => {
                p.sampled == q
                    && p.committed == q
                    && p.written == 0
                    && self.projective.len() == q
                    && self.x.is_none()
            }
            Phase::Writing => {
                p.sampled == q && p.committed == q && self.projective.is_empty() && self.x.is_none()
            }
            Phase::Squeezing => {
                p.sampled == q
                    && p.committed == q
                    && p.written == q
                    && self.projective.is_empty()
                    && self.x.is_none()
            }
            Phase::Finished => {
                p.sampled == q
                    && p.committed == q
                    && p.written == q
                    && self.projective.is_empty()
                    && self.x.is_some()
            }
        };
        if !valid {
            return Err(StoredLookupErrorV1::Context);
        }
        Ok(())
    }
}

impl<'params, 'instances, C, P, R, T, E, const Q: bool, const M: u64>
    QuotientCoefficientsPendingStoredIpaProverV1<'params, 'instances, C, P, R, T, E, Q, M>
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3>,
    P: StoredPolynomialProviderV1,
{
    fn validate_commitment_work(&self, work: &Workspace<C>) -> Result<(), StoredLookupErrorV1> {
        work.validate_progress()?;
        validate_owner(self, work.geometry, work.progress.greatest)
    }
    fn read_commitment_chunk(
        &mut self,
        work: &mut Workspace<C>,
        piece: usize,
        chunk: usize,
    ) -> Result<(), StoredLookupErrorV1> {
        self.validate_commitment_work(work)?;
        if work.progress.phase != Phase::Committing
            || piece != work.progress.committed
            || piece >= work.geometry.q
        {
            return Err(StoredLookupErrorV1::Context);
        }
        let value = self
            .pieces
            .get_mut(piece)
            .ok_or(StoredLookupErrorV1::Context)?;
        let expected = value.layout;
        let length = expected.chunk_scalar_count(chunk as u64)?;
        let start = mul(chunk, TILE)?;
        let end = add(start, length)?;
        let output = work
            .column
            .0
            .values
            .get_mut(start..end)
            .ok_or(StoredLookupErrorV1::Context)?;
        // Recheck the immediate source after the whole retained-bank sweep.
        if value.snapshot.layout() != expected {
            return Err(StoredPolynomialErrorV1::Context.into());
        }
        let mut decoded = false;
        #[cfg(test)]
        event(StageEvent::ReadStart(piece, chunk));
        value.snapshot.with_chunk(expected, chunk as u64, |bytes| {
            if bytes.len() != output.len()
                || bytes
                    .iter()
                    .any(|bytes| !expected.field().is_canonical(bytes))
            {
                return Err(StoredPolynomialErrorV1::Encoding);
            }
            for (out, bytes) in output.iter_mut().zip(bytes) {
                *out = Option::<C::Scalar>::from(C::Scalar::from_repr(*bytes))
                    .ok_or(StoredPolynomialErrorV1::Encoding)?;
            }
            decoded = true;
            Ok(())
        })?;
        if !decoded || value.snapshot.layout() != expected {
            return Err(StoredPolynomialErrorV1::Context.into());
        }
        self.validate_commitment_work(work)?;
        #[cfg(test)]
        event(StageEvent::ReadEnd(piece, chunk));
        Ok(())
    }
}

impl<'params, 'instances, C, P, R, T, E, const Q: bool, const M: u64>
    QuotientCoefficientsPendingStoredIpaProverV1<'params, 'instances, C, P, R, T, E, Q, M>
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3>,
    P: StoredPolynomialProviderV1,
    R: RngCore,
    E: EncodedChallenge<C>,
    T: TranscriptWrite<C, E>,
{
    /// Commit all original quotient pieces and squeeze exactly the original ChallengeX.
    ///
    /// All q blinds are sampled before reads/commitments; all commitments precede normalization
    /// and transcript writes. One guarded n-field buffer is reused without new provider handles.
    /// The admitted new payload excludes inherited proof owners, backend/MSM allocations and RSS.
    pub(crate) fn commit_quotient(
        mut self,
        scratch_limit_bytes: usize,
    ) -> Result<
        QuotientCommitmentsPendingStoredIpaProverV1<'params, 'instances, C, P, R, T, E, Q, M>,
        StoredLookupErrorV1,
    > {
        let g = geometry(&self.inner.pk)?;
        let greatest = self
            .pieces
            .last()
            .ok_or(StoredLookupErrorV1::Context)?
            .layout
            .ordinal();
        validate_owner(&self, g, greatest)?;
        if payload::<C>(g.n, g.q, g.q, g.q)? > scratch_limit_bytes {
            return Err(StoredLookupErrorV1::ScratchLimit);
        }
        // The ordinary construct evicts only this thread's scalar FFT scratch before MSM.
        crate::fft::recursive::clear_scratch::<C::Scalar>();
        validate_owner(&self, g, greatest)?;
        let mut work = Workspace::new(&self.inner.pk, g, greatest)?;
        if work.actual_payload()? > scratch_limit_bytes {
            return Err(StoredLookupErrorV1::ScratchLimit);
        }
        self.validate_commitment_work(&work)?;
        for piece in 0..g.q {
            self.validate_commitment_work(&work)?;
            #[cfg(test)]
            event(StageEvent::SampleStart(piece));
            work.blinds[piece].0 = Blind(C::Scalar::random(&mut self.inner.rng));
            work.progress.sampled += 1;
            self.validate_commitment_work(&work)?;
            #[cfg(test)]
            event(StageEvent::SampleEnd(piece));
        }
        work.progress.phase = Phase::Committing;
        self.validate_commitment_work(&work)?;
        for piece in 0..g.q {
            for chunk in 0..g.n.div_ceil(TILE) {
                self.read_commitment_chunk(&mut work, piece, chunk)?;
            }
            self.validate_commitment_work(&work)?;
            #[cfg(test)]
            {
                event(StageEvent::CommitStart(piece));
                REUSE.with(|v| {
                    let (a, c, p, _, calls) = v.get();
                    v.set((a, c, p, work.column.0.values.as_ptr() as usize, calls + 1));
                });
            }
            work.projective[piece] = self
                .inner
                .params
                .commit(&work.column.0, work.blinds[piece].0);
            work.progress.committed += 1;
            self.validate_commitment_work(&work)?;
            work.column.clear();
            self.validate_commitment_work(&work)?;
            #[cfg(test)]
            event(StageEvent::CommitEnd(piece));
        }
        work.progress.phase = Phase::Normalizing;
        self.validate_commitment_work(&work)?;
        #[cfg(test)]
        event(StageEvent::NormalizeStart);
        C::Curve::batch_normalize(&work.projective, &mut work.affine);
        self.validate_commitment_work(&work)?;
        #[cfg(test)]
        event(StageEvent::NormalizeEnd);
        // Projective commitments are public results and are no longer needed after normalization.
        work.projective = Vec::new();
        work.progress.phase = Phase::Writing;
        self.validate_commitment_work(&work)?;
        for piece in 0..g.q {
            self.validate_commitment_work(&work)?;
            #[cfg(test)]
            event(StageEvent::WriteStart(piece));
            self.inner
                .transcript
                .write_point(work.affine[piece])
                .map_err(|_| StoredLookupErrorV1::Transcript)?;
            work.progress.written += 1;
            self.validate_commitment_work(&work)?;
            #[cfg(test)]
            event(StageEvent::WriteEnd(piece));
        }
        work.progress.phase = Phase::Squeezing;
        self.validate_commitment_work(&work)?;
        #[cfg(test)]
        event(StageEvent::SqueezeStart);
        work.x = Some(self.inner.transcript.squeeze_challenge_scalar());
        work.progress.phase = Phase::Finished;
        self.validate_commitment_work(&work)?;
        #[cfg(test)]
        event(StageEvent::SqueezeEnd);
        let Workspace {
            blinds, affine, x, ..
        } = work;
        Ok(QuotientCommitmentsPendingStoredIpaProverV1 {
            inner: self,
            blinds,
            commitments: affine,
            x: x.ok_or(StoredLookupErrorV1::Context)?,
        })
    }
}

// Share the original complete receipt sweep with the scalar/opening continuation. Its callers
// receive no geometry, mutable receipt, blind, or alternate owner-construction capability.
impl<'params, 'instances, C, P, R, T, E, const Q: bool, const M: u64>
    QuotientCommitmentsPendingStoredIpaProverV1<'params, 'instances, C, P, R, T, E, Q, M>
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3>,
    P: StoredPolynomialProviderV1,
{
    pub(super) fn validate_evaluation_owner(&self) -> Result<(), StoredLookupErrorV1> {
        let geometry = geometry(&self.inner.inner.pk)?;
        if self.blinds.len() != geometry.q || self.commitments.len() != geometry.q {
            return Err(StoredLookupErrorV1::Context);
        }
        let greatest = self
            .inner
            .pieces
            .last()
            .ok_or(StoredLookupErrorV1::Context)?
            .layout
            .ordinal();
        validate_owner(&self.inner, geometry, greatest)
    }
}
