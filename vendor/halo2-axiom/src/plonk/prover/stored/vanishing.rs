//! Consuming instance coefficients, actual random vanishing commitment/y, and advice handoff.
//!
//! The original proof owner survives inseparably into the quotient boundary. No instance
//! commitment repeats and no advice blind is copied. TODO: key-bound quotient and openings.
use super::{
    CoefficientPendingStoredIpaProverV1,
    lookup::StoredLookupErrorV1,
    lookup_permuted::{PermutedPolynomialV1, SecretLookupBlindV1},
    lookup_sort::Encoded,
    products::{ProductLookupV1, ProductsPendingStoredIpaProverV1, StoredProductV1},
};
use crate::{
    arithmetic::CurveAffine,
    plonk::{ChallengeBeta, ChallengeGamma, ChallengeTheta, ChallengeY, ProvingKey, circuit::Any},
    poly::{
        Coeff, Polynomial,
        commitment::{Blind, Params, ParamsProver},
        ipa::commitment::ParamsIPA,
        stored_advice::{
            STORED_MAX_K_V1, STORED_SCALARS_PER_CHUNK_V1, StoredLookupSideV1,
            StoredPolynomialBasisV1, StoredPolynomialErrorV1, StoredPolynomialLayoutV1,
            StoredPolynomialProviderV1, StoredPolynomialRoleV1, StoredPolynomialSnapshotV1,
            StoredPolynomialWriterV1,
            assignment::StoredAssignmentFieldV1,
            phase::{
                CoefficientHandoffAllocationV1, CoefficientOnlyStoredAdviceV1,
                CoefficientStoredAdviceV1,
            },
        },
    },
    transcript::{EncodedChallenge, TranscriptWrite},
};
use ff::{Field, PrimeField, WithSmallOrderMulGroup};
use group::Curve;
use rand_core::RngCore;
use std::{
    marker::PhantomData,
    ptr,
    sync::atomic::{Ordering, compiler_fence},
};
const TILE: usize = STORED_SCALARS_PER_CHUNK_V1;
type SnapshotOf<P> =
    <<P as StoredPolynomialProviderV1>::Writer as StoredPolynomialWriterV1>::Snapshot;
/// Complete retained inputs immediately before key-bound quotient construction.
#[allow(dead_code)]
pub(crate) struct VanishingPendingStoredIpaProverV1<
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
    pub(super) params: &'params ParamsIPA<C>,
    pub(super) pk: ProvingKey<C>,
    pub(super) advice: CoefficientOnlyStoredAdviceV1<'params, C, SnapshotOf<P>>,
    pub(super) provider: P,
    pub(super) rng: R,
    pub(super) transcript: T,
    pub(super) instances: &'instances [&'instances [C::Scalar]],
    pub(super) theta: ChallengeTheta<C>,
    pub(super) beta: ChallengeBeta<C>,
    pub(super) gamma: ChallengeGamma<C>,
    pub(super) y: ChallengeY<C>,
    pub(super) usable_rows: usize,
    pub(super) permutations: Vec<StoredProductV1<C, SnapshotOf<P>>>,
    pub(super) lookups: Vec<ProductLookupV1<C, SnapshotOf<P>>>,
    pub(super) instance_coefficients: Vec<PermutedPolynomialV1<SnapshotOf<P>>>,
    pub(super) random: StoredProductV1<C, SnapshotOf<P>>,
    pub(super) _challenge: PhantomData<E>,
}
fn reserve<V>(count: usize) -> Result<Vec<V>, StoredLookupErrorV1> {
    let mut v = Vec::new();
    v.try_reserve_exact(count)
        .map_err(|_| StoredLookupErrorV1::Allocation)?;
    Ok(v)
}
struct Column<F: StoredAssignmentFieldV1>(Polynomial<F, Coeff>);
impl<F: StoredAssignmentFieldV1> Column<F> {
    fn new(domain: &crate::poly::EvaluationDomain<F>) -> Result<Self, StoredLookupErrorV1>
    where
        F: WithSmallOrderMulGroup<3>,
    {
        let n = domain.get_n() as usize;
        let mut values = reserve(n)?;
        values.resize(n, F::ZERO);
        Ok(Self(domain.coeff_from_vec(values)))
    }
    fn clear(&mut self) {
        for v in &mut self.0.values {
            // SAFETY: every exclusively owned initialized Copy field admits ZERO.
            unsafe { ptr::write_volatile(v, F::ZERO) };
        }
        compiler_fence(Ordering::SeqCst);
        #[cfg(test)]
        FIELD_CLEARS.with(|s| {
            let (n, z) = s.get();
            s.set((n + self.0.len(), z && self.0.iter().all(|v| *v == F::ZERO)));
        });
    }
}
impl<F: StoredAssignmentFieldV1> Drop for Column<F> {
    fn drop(&mut self) {
        self.clear()
    }
}
struct Scratch<F: StoredAssignmentFieldV1> {
    column: Column<F>,
    encoded: Encoded,
}
struct Output<C: CurveAffine, W, S>
where
    C::Scalar: StoredAssignmentFieldV1,
{
    layout: StoredPolynomialLayoutV1,
    writer: Option<W>,
    snapshot: Option<S>,
    blind: Option<SecretLookupBlindV1<C::Scalar>>,
    point: Option<C>,
}
#[cfg(test)]
thread_local! {static FIELD_CLEARS:std::cell::Cell<(usize,bool)>=const{std::cell::Cell::new((0,true))};}
#[cfg(test)]
pub(super) fn take_clear_observations() -> (usize, bool, usize, bool, usize, bool) {
    let (n, z) = FIELD_CLEARS.with(|v| v.replace((0, true)));
    let (o, oz, b, bz, l, lz) = super::lookup_permuted::take_clear_observations();
    (n + o, z && oz, b, bz, l, lz)
}
/// Logical minimum; actual adapter capacities are admitted again before any new field sample.
pub(super) fn scratch_bytes<C, S, W>(
    k: u32,
    advice: usize,
    instances: usize,
) -> Result<usize, StoredLookupErrorV1>
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1,
{
    if k > STORED_MAX_K_V1 {
        return Err(StoredLookupErrorV1::Context);
    }
    let n = 1_usize.checked_shl(k).ok_or(StoredLookupErrorV1::Context)?;
    payload::<C, S, W>(
        n,
        n,
        TILE,
        instances,
        CoefficientHandoffAllocationV1::<C, S>::minimum_payload(advice)?,
        instances != 0,
    )
}
fn payload<C, S, W>(
    n: usize,
    fields: usize,
    bytes: usize,
    instances: usize,
    handoff: usize,
    fft: bool,
) -> Result<usize, StoredLookupErrorV1>
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1,
{
    fields
        .checked_add(if fft { n / 2 } else { 0 })
        .and_then(|n| n.checked_mul(std::mem::size_of::<C::Scalar>()))
        .and_then(|v| bytes.checked_mul(32).and_then(|b| v.checked_add(b)))
        .and_then(|v| {
            instances
                .checked_mul(std::mem::size_of::<PermutedPolynomialV1<S>>())
                .and_then(|b| v.checked_add(b))
        })
        .and_then(|v| v.checked_add(handoff))
        .and_then(|v| v.checked_add(std::mem::size_of::<Scratch<C::Scalar>>()))
        .and_then(|v| v.checked_add(std::mem::size_of::<Output<C, W, S>>()))
        .and_then(|v| v.checked_add(std::mem::size_of::<StoredProductV1<C, S>>()))
        .and_then(|v| v.checked_add(std::mem::size_of::<C::Curve>()))
        .and_then(|v| v.checked_add(std::mem::size_of::<C>()))
        .ok_or(StoredLookupErrorV1::Context)
}
struct Work<'params, 'instances, C: CurveAffine, P: StoredPolynomialProviderV1>
where
    C::Scalar: StoredAssignmentFieldV1,
{
    params: &'params ParamsIPA<C>,
    pk: ProvingKey<C>,
    advice: Option<CoefficientStoredAdviceV1<'params, C, SnapshotOf<P>>>,
    retired: Option<CoefficientOnlyStoredAdviceV1<'params, C, SnapshotOf<P>>>,
    handoff: Option<CoefficientHandoffAllocationV1<'params, C, SnapshotOf<P>>>,
    provider: P,
    instances: &'instances [&'instances [C::Scalar]],
    permutations: Vec<StoredProductV1<C, SnapshotOf<P>>>,
    lookups: Vec<ProductLookupV1<C, SnapshotOf<P>>>,
    instance_coefficients: Vec<PermutedPolynomialV1<SnapshotOf<P>>>,
    random: Option<StoredProductV1<C, SnapshotOf<P>>>,
    output: Option<Output<C, P::Writer, SnapshotOf<P>>>,
    scratch: Scratch<C::Scalar>,
    n: usize,
    usable: usize,
    old_end: Option<u64>,
}
fn context_matches(layout: StoredPolynomialLayoutV1, context: Option<[u8; 32]>) -> bool {
    context.is_some_and(|context| {
        context != [0; 32]
            && StoredPolynomialLayoutV1::new(
                context,
                layout.ordinal(),
                layout.field(),
                layout.basis(),
                layout.k(),
                layout.role(),
            )
            .is_ok_and(|e| layout.same_proof_context(e))
    })
}
fn check<S: StoredPolynomialSnapshotV1>(
    v: &PermutedPolynomialV1<S>,
    role: StoredPolynomialRoleV1,
    context: Option<[u8; 32]>,
    k: u32,
    field: crate::poly::stored_advice::StoredPastaFieldV1,
) -> Result<(), StoredLookupErrorV1> {
    if v.layout.role() != role
        || v.layout.basis() != StoredPolynomialBasisV1::Coefficient
        || v.layout.k() != k
        || v.layout.field() != field
        || !context_matches(v.layout, context)
        || v.snapshot.layout() != v.layout
    {
        return Err(StoredPolynomialErrorV1::Context.into());
    }
    Ok(())
}
impl<'params, 'instances, C, P> Work<'params, 'instances, C, P>
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3>,
    P: StoredPolynomialProviderV1,
{
    fn context(&self) -> Result<Option<[u8; 32]>, StoredLookupErrorV1> {
        match (&self.advice, &self.retired) {
            (Some(a), None) => Ok(a.proof_context()?),
            (None, Some(a)) => Ok(a.proof_context()?),
            _ => Err(StoredPolynomialErrorV1::Poisoned.into()),
        }
    }
    fn validate<const Q: bool, const M: u64>(&self) -> Result<(), StoredLookupErrorV1> {
        let cs = &self.pk.vk.cs;
        let k = self.pk.vk.domain.k();
        let m = cs.permutation.columns.len();
        if k > STORED_MAX_K_V1
            || self.params.k() != k
            || self.params.n() != self.n as u64
            || self.n != 1_usize << k
            || cs
                .blinding_factors()
                .checked_add(1)
                .and_then(|b| self.n.checked_sub(b))
                != Some(self.usable)
            || self.pk.vk.cs_degree < 3
            || m.div_ceil(self.pk.vk.cs_degree - 2) != self.permutations.len()
            || cs.lookups.len() != self.lookups.len()
            || cs.advice_column_phase.len() != cs.num_advice_columns
            || cs.challenge_phase.len() != cs.num_challenges
            || self.pk.fixed_polys.len() != cs.num_fixed_columns
            || self.pk.fixed_polys.iter().any(|v| v.len() != self.n)
            || self.pk.permutation.polys.len() != m
            || self.pk.permutation.polys.iter().any(|v| v.len() != self.n)
            || !self.pk.fixed_values.is_empty()
            || !self.pk.permutation.permutations.is_empty()
            || self.pk.l0.len() != self.n
            || self.pk.l_last.len() != self.n
            || self.pk.l_active_row.len() != self.n
            || self.instances.len() != cs.num_instance_columns
            || self.instances.iter().any(|v| v.len() > self.usable)
            || (!Q && M != 0)
            || (cs.num_instance_columns < 64 && M >> cs.num_instance_columns != 0)
            || self.instance_coefficients.len() > self.instances.len()
        {
            return Err(StoredLookupErrorV1::Context);
        }
        for (i, c) in cs.permutation.columns.iter().enumerate() {
            let valid = match c.column_type() {
                Any::Advice(kind) => cs
                    .advice_column_phase
                    .get(c.index())
                    .is_some_and(|p| p.to_u8() == kind.phase()),
                Any::Fixed => c.index() < cs.num_fixed_columns,
                Any::Instance => c.index() < cs.num_instance_columns,
            };
            if !valid || cs.permutation.columns[..i].contains(c) {
                return Err(StoredLookupErrorV1::Context);
            }
        }
        let mut last = None;
        match (&self.advice, &self.retired) {
            (Some(a), None) => {
                a.validate_live_receipts()?;
                if !ptr::eq(a.params()?, self.params)
                    || a.challenges()?.count() != cs.num_challenges
                    || a.layouts()?.len() != cs.num_advice_columns
                {
                    return Err(StoredLookupErrorV1::Context);
                }
                for (l, p) in a.coefficient_layouts()?.zip(&cs.advice_column_phase) {
                    if l.advice_coordinates()?.1 != p.to_u8() {
                        return Err(StoredLookupErrorV1::Context);
                    }
                    last = Some(l.ordinal());
                }
            }
            (None, Some(a)) => {
                a.validate_live_receipts()?;
                if !ptr::eq(a.params()?, self.params)
                    || a.challenges()?.count() != cs.num_challenges
                    || a.layouts()?.len() != cs.num_advice_columns
                {
                    return Err(StoredLookupErrorV1::Context);
                }
                for (l, p) in a.layouts()?.zip(&cs.advice_column_phase) {
                    if l.advice_coordinates()?.1 != p.to_u8() {
                        return Err(StoredLookupErrorV1::Context);
                    }
                    last = Some(l.ordinal());
                }
            }
            _ => return Err(StoredPolynomialErrorV1::Poisoned.into()),
        }
        let context = self.context()?;
        let field = C::Scalar::STORED_FIELD;
        let mut old =
            |v: &PermutedPolynomialV1<SnapshotOf<P>>, role| -> Result<(), StoredLookupErrorV1> {
                check(v, role, context, k, field)?;
                if last.is_some_and(|l| v.layout.ordinal() <= l)
                    || self.old_end.is_none_or(|end| v.layout.ordinal() > end)
                {
                    return Err(StoredPolynomialErrorV1::Context.into());
                }
                last = Some(v.layout.ordinal());
                Ok(())
            };
        for (i, l) in self.lookups.iter().enumerate() {
            for (v, side) in [
                (&l.input, StoredLookupSideV1::Input),
                (&l.table, StoredLookupSideV1::Table),
            ] {
                old(
                    &v.coefficient,
                    StoredPolynomialRoleV1::LookupPermuted {
                        lookup: i as u32,
                        side,
                    },
                )?;
            }
        }
        for (i, v) in self.permutations.iter().enumerate() {
            old(
                &v.coefficient,
                StoredPolynomialRoleV1::CopyPermutationProduct { set: i as u32 },
            )?;
        }
        for (i, v) in self.lookups.iter().enumerate() {
            old(
                &v.product.coefficient,
                StoredPolynomialRoleV1::LookupProduct { lookup: i as u32 },
            )?;
        }
        let mut last = self.old_end;
        for (i, v) in self.instance_coefficients.iter().enumerate() {
            check(
                v,
                StoredPolynomialRoleV1::Instance { column: i as u32 },
                context,
                k,
                field,
            )?;
            if last.is_some_and(|old| v.layout.ordinal() <= old) {
                return Err(StoredPolynomialErrorV1::Context.into());
            }
            last = Some(v.layout.ordinal());
        }
        if let Some(v) = &self.random {
            if self.instance_coefficients.len() != self.instances.len() || self.output.is_some() {
                return Err(StoredLookupErrorV1::Context);
            }
            check(
                &v.coefficient,
                StoredPolynomialRoleV1::VanishingRandom,
                context,
                k,
                field,
            )?;
            if last.is_some_and(|old| v.coefficient.layout.ordinal() <= old) {
                return Err(StoredPolynomialErrorV1::Context.into());
            }
            last = Some(v.coefficient.layout.ordinal());
        }
        if let Some(v) = &self.output {
            let role = if self.instance_coefficients.len() < self.instances.len() {
                StoredPolynomialRoleV1::Instance {
                    column: self.instance_coefficients.len() as u32,
                }
            } else {
                StoredPolynomialRoleV1::VanishingRandom
            };
            if v.layout.role() != role
                || v.layout.basis() != StoredPolynomialBasisV1::Coefficient
                || v.layout.k() != k
                || v.layout.field() != field
                || !context_matches(v.layout, context)
                || last.is_some_and(|old| v.layout.ordinal() <= old)
                || v.writer.is_some() == v.snapshot.is_some()
                || v.writer.as_ref().is_some_and(|w| w.layout() != v.layout)
                || v.snapshot.as_ref().is_some_and(|s| s.layout() != v.layout)
            {
                return Err(StoredPolynomialErrorV1::Context.into());
            }
            last = Some(v.layout.ordinal());
        }
        let greatest = match (&self.advice, &self.retired) {
            (Some(a), None) => a.product_ordinal_boundary(0)?,
            (None, Some(a)) => a.greatest_ordinal()?,
            _ => return Err(StoredPolynomialErrorV1::Poisoned.into()),
        };
        if last != greatest {
            return Err(StoredPolynomialErrorV1::Context.into());
        }
        Ok(())
    }
    fn begin<const Q: bool, const M: u64>(&mut self) -> Result<(), StoredLookupErrorV1> {
        self.validate::<Q, M>()?;
        if self.output.is_some() || self.random.is_some() {
            return Err(StoredLookupErrorV1::Context);
        }
        let a = self
            .advice
            .take()
            .ok_or(StoredPolynomialErrorV1::Poisoned)?;
        let (a, writer, layout) = if self.instance_coefficients.len() < self.instances.len() {
            a.create_instance_writer(&mut self.provider, self.instance_coefficients.len() as u32)?
        } else {
            a.create_vanishing_writer(&mut self.provider)?
        };
        self.advice = Some(a);
        let remaining = self
            .instances
            .len()
            .checked_add(1)
            .and_then(|n| n.checked_sub(self.instance_coefficients.len()))
            .ok_or(StoredLookupErrorV1::Context)?;
        layout
            .ordinal()
            .checked_add(u64::try_from(remaining).map_err(|_| StoredLookupErrorV1::Context)?)
            .ok_or(StoredLookupErrorV1::Context)?;
        self.output = Some(Output {
            layout,
            writer: Some(writer),
            snapshot: None,
            blind: None,
            point: None,
        });
        self.validate::<Q, M>()
    }
    fn seal<const Q: bool, const M: u64>(&mut self) -> Result<(), StoredLookupErrorV1> {
        self.validate::<Q, M>()?;
        let layout = self
            .output
            .as_ref()
            .ok_or(StoredLookupErrorV1::Context)?
            .layout;
        for chunk in 0..layout.chunk_count() {
            self.validate::<Q, M>()?;
            let count = layout.chunk_scalar_count(chunk as u64)?;
            let start = chunk * TILE;
            for (b, v) in self.scratch.encoded.0[..count]
                .iter_mut()
                .zip(&self.scratch.column.0.values[start..start + count])
            {
                *b = v.to_repr()
            }
            self.output
                .as_mut()
                .and_then(|v| v.writer.as_mut())
                .ok_or(StoredLookupErrorV1::Context)?
                .write_chunk(chunk as u64, &self.scratch.encoded.0[..count])?;
            self.scratch.encoded.clear();
            self.validate::<Q, M>()?;
        }
        let writer = self
            .output
            .as_mut()
            .and_then(|v| v.writer.take())
            .ok_or(StoredLookupErrorV1::Context)?;
        let snapshot = writer.seal()?;
        self.output
            .as_mut()
            .ok_or(StoredLookupErrorV1::Context)?
            .snapshot = Some(snapshot);
        self.validate::<Q, M>()
    }
}
impl<'params, 'instances, C, P, R, T, E, const Q: bool, const M: u64>
    ProductsPendingStoredIpaProverV1<'params, 'instances, C, P, R, T, E, Q, M>
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3>,
    P: StoredPolynomialProviderV1,
    R: RngCore,
    T: TranscriptWrite<C, E>,
    E: EncodedChallenge<C>,
{
    /// Stage all original instances, commit actual vanishing randomness/y, then move advice blinds.
    /// All original and partial ownership is consumed on failure/unwind; no proof is exposed.
    pub(crate) fn commit_vanishing_and_stage_coefficients(
        self,
        scratch_limit_bytes: usize,
    ) -> Result<
        VanishingPendingStoredIpaProverV1<'params, 'instances, C, P, R, T, E, Q, M>,
        StoredLookupErrorV1,
    > {
        let ProductsPendingStoredIpaProverV1 {
            inner,
            theta,
            beta,
            gamma,
            usable_rows,
            permutations,
            lookups,
        } = self;
        let CoefficientPendingStoredIpaProverV1 {
            params,
            pk,
            advice,
            provider,
            mut rng,
            mut transcript,
            instances,
            _challenge,
        } = inner;
        let k = pk.vk.domain.k();
        if k > STORED_MAX_K_V1 {
            return Err(StoredLookupErrorV1::Context);
        }
        let n = 1_usize.checked_shl(k).ok_or(StoredLookupErrorV1::Context)?;
        u32::try_from(instances.len()).map_err(|_| StoredLookupErrorV1::Context)?;
        u32::try_from(lookups.len()).map_err(|_| StoredLookupErrorV1::Context)?;
        u32::try_from(permutations.len()).map_err(|_| StoredLookupErrorV1::Context)?;
        let total = instances
            .len()
            .checked_add(1)
            .ok_or(StoredLookupErrorV1::Context)?;
        let old_end = advice.product_ordinal_boundary(total)?;
        if scratch_bytes::<C, SnapshotOf<P>, P::Writer>(
            k,
            pk.vk.cs.num_advice_columns,
            instances.len(),
        )? > scratch_limit_bytes
        {
            return Err(StoredLookupErrorV1::ScratchLimit);
        }
        let handoff = advice.prepare_coefficient_only_handoff()?;
        let instance_coefficients = reserve(instances.len())?;
        let scratch = Scratch {
            column: Column::new(&pk.vk.domain)?,
            encoded: Encoded::new()?,
        };
        if payload::<C, SnapshotOf<P>, P::Writer>(
            n,
            scratch.column.0.values.capacity(),
            scratch.encoded.0.capacity(),
            instance_coefficients.capacity(),
            handoff.payload_bytes()?,
            !instances.is_empty(),
        )? > scratch_limit_bytes
        {
            return Err(StoredLookupErrorV1::ScratchLimit);
        }
        let mut work = Work {
            params,
            pk,
            advice: Some(advice),
            retired: None,
            handoff: Some(handoff),
            provider,
            instances,
            permutations,
            lookups,
            instance_coefficients,
            random: None,
            output: None,
            scratch,
            n,
            usable: usable_rows,
            old_end,
        };
        work.validate::<Q, M>()?;
        for index in 0..instances.len() {
            work.begin::<Q, M>()?;
            work.scratch.column.clear();
            work.scratch.column.0.values[..instances[index].len()]
                .copy_from_slice(instances[index]);
            work.validate::<Q, M>()?;
            work.pk.vk.domain.stored_column_transform_in_place(
                &mut work.scratch.column.0.values,
                true,
                None,
            );
            work.validate::<Q, M>()?;
            work.seal::<Q, M>()?;
            let mut output = work.output.take().ok_or(StoredLookupErrorV1::Context)?;
            work.instance_coefficients.push(PermutedPolynomialV1 {
                layout: output.layout,
                snapshot: output.snapshot.take().ok_or(StoredLookupErrorV1::Context)?,
            });
            work.scratch.column.clear();
            work.validate::<Q, M>()?;
        }
        work.begin::<Q, M>()?;
        work.scratch.column.clear();
        for index in 0..n {
            work.scratch.column.0.values[index] = C::Scalar::random(&mut rng);
            work.validate::<Q, M>()?;
        }
        work.output
            .as_mut()
            .ok_or(StoredLookupErrorV1::Context)?
            .blind = Some(SecretLookupBlindV1(Blind(C::Scalar::random(&mut rng))));
        work.validate::<Q, M>()?;
        let blind = work
            .output
            .as_ref()
            .and_then(|v| v.blind.as_ref())
            .ok_or(StoredLookupErrorV1::Context)?;
        let point = work.params.commit(&work.scratch.column.0, blind.0);
        work.validate::<Q, M>()?;
        let point = point.to_affine();
        work.output
            .as_mut()
            .ok_or(StoredLookupErrorV1::Context)?
            .point = Some(point);
        work.validate::<Q, M>()?;
        work.seal::<Q, M>()?;
        transcript
            .write_point(point)
            .map_err(|_| StoredLookupErrorV1::Transcript)?;
        work.validate::<Q, M>()?;
        let y: ChallengeY<C> = transcript.squeeze_challenge_scalar();
        work.validate::<Q, M>()?;
        let mut output = work.output.take().ok_or(StoredLookupErrorV1::Context)?;
        work.random = Some(StoredProductV1 {
            coefficient: PermutedPolynomialV1 {
                layout: output.layout,
                snapshot: output.snapshot.take().ok_or(StoredLookupErrorV1::Context)?,
            },
            blind: output.blind.take().ok_or(StoredLookupErrorV1::Context)?,
            commitment: output.point.take().ok_or(StoredLookupErrorV1::Context)?,
        });
        work.scratch.column.clear();
        work.validate::<Q, M>()?;
        let advice = work
            .advice
            .take()
            .ok_or(StoredPolynomialErrorV1::Poisoned)?;
        let allocation = work.handoff.take().ok_or(StoredLookupErrorV1::Context)?;
        work.retired = Some(advice.into_coefficient_only(allocation)?);
        work.validate::<Q, M>()?;
        let Work {
            params,
            pk,
            retired,
            provider,
            instances,
            permutations,
            lookups,
            instance_coefficients,
            random,
            ..
        } = work;
        Ok(VanishingPendingStoredIpaProverV1 {
            params,
            pk,
            advice: retired.ok_or(StoredPolynomialErrorV1::Poisoned)?,
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
            random: random.ok_or(StoredLookupErrorV1::Context)?,
            _challenge,
        })
    }
}
