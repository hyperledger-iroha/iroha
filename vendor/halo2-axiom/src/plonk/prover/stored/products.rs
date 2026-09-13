//! Consuming ordinary beta/gamma, copy products and lookup products over authenticated receipts.
//!
//! One guarded degree-sized column and fixed tiles feed ordinary Lagrange commitments. Product
//! coefficients and sole blinds survive; consumed lookup Lagrange inputs retire only after the
//! product seals and its transcript point succeeds. The next vanishing/y handoff stages instances and retires advice Lagrange receipts; quotient and openings remain.
//! The admitted known payload includes baseline FFT twiddles, not MSM/backend or whole RSS.

use super::{
    CoefficientPendingStoredIpaProverV1,
    lookup::{CompressedLookupV1, LookupCompressedPendingStoredIpaProverV1, StoredLookupErrorV1},
    lookup_permuted::{
        LookupPermutedPendingStoredIpaProverV1, PermutedLookupV1, PermutedPolynomialV1,
        SecretLookupBlindV1,
    },
    lookup_sort::Encoded,
};
use crate::{
    arithmetic::CurveAffine,
    plonk::{ChallengeBeta, ChallengeGamma, ChallengeTheta, ProvingKey, circuit::Any},
    poly::{
        EvaluationDomain, LagrangeCoeff, Polynomial,
        commitment::{Blind, Params},
        ipa::commitment::ParamsIPA,
        stored_advice::{
            STORED_MAX_K_V1, STORED_SCALARS_PER_CHUNK_V1, StoredLookupSideV1,
            StoredPolynomialBasisV1, StoredPolynomialErrorV1, StoredPolynomialLayoutV1,
            StoredPolynomialProviderV1, StoredPolynomialRoleV1, StoredPolynomialSnapshotV1,
            StoredPolynomialWriterV1, assignment::StoredAssignmentFieldV1,
            phase::CoefficientStoredAdviceV1,
        },
    },
    transcript::{EncodedChallenge, TranscriptWrite},
};
use ff::{BatchInverter, Field, PrimeField, WithSmallOrderMulGroup};
use group::Curve;
use rand_core::RngCore;
use std::{
    ptr,
    sync::atomic::{Ordering, compiler_fence},
};

const TILE: usize = STORED_SCALARS_PER_CHUNK_V1;
type SnapshotOf<P> =
    <<P as StoredPolynomialProviderV1>::Writer as StoredPolynomialWriterV1>::Snapshot;

/// A retained coefficient polynomial and its one original commitment blind.
pub(super) struct StoredProductV1<C: CurveAffine, S>
where
    C::Scalar: StoredAssignmentFieldV1,
{
    pub(super) coefficient: PermutedPolynomialV1<S>,
    pub(super) blind: SecretLookupBlindV1<C::Scalar>,
    pub(super) commitment: C,
}
/// One completed lookup: original permuted coefficients/blinds and the new product.
pub(super) struct ProductLookupV1<C: CurveAffine, S>
where
    C::Scalar: StoredAssignmentFieldV1,
{
    pub(super) input: StoredProductV1<C, S>,
    pub(super) table: StoredProductV1<C, S>,
    pub(super) product: StoredProductV1<C, S>,
}
/// Inseparable original proof continuation immediately before vanishing's random commitment.
#[allow(dead_code)]
pub(crate) struct ProductsPendingStoredIpaProverV1<
    'params,
    'instances,
    C,
    P,
    R,
    T,
    E,
    const QUERY_INSTANCE: bool,
    const INSTANCE_MASK: u64,
> where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1,
    P: StoredPolynomialProviderV1,
{
    pub(super) inner: CoefficientPendingStoredIpaProverV1<
        'params,
        'instances,
        C,
        P,
        R,
        T,
        E,
        QUERY_INSTANCE,
        INSTANCE_MASK,
    >,
    pub(super) theta: ChallengeTheta<C>,
    pub(super) beta: ChallengeBeta<C>,
    pub(super) gamma: ChallengeGamma<C>,
    pub(super) usable_rows: usize,
    pub(super) permutations: Vec<StoredProductV1<C, SnapshotOf<P>>>,
    pub(super) lookups: Vec<ProductLookupV1<C, SnapshotOf<P>>>,
}

fn reserve<V>(count: usize) -> Result<Vec<V>, StoredLookupErrorV1> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| StoredLookupErrorV1::Allocation)?;
    Ok(values)
}
struct Fields<F: StoredAssignmentFieldV1>(Vec<F>);
impl<F: StoredAssignmentFieldV1> Fields<F> {
    fn new(count: usize) -> Result<Self, StoredLookupErrorV1> {
        let mut values = reserve(count)?;
        values.resize(count, F::ZERO);
        Ok(Self(values))
    }
    fn clear(&mut self) {
        for value in &mut self.0 {
            // SAFETY: every exclusively owned initialized Copy field admits ZERO.
            unsafe {
                ptr::write_volatile(value, F::ZERO);
            }
        }
        compiler_fence(Ordering::SeqCst);
        #[cfg(test)]
        FIELD_CLEARS.with(|s| {
            let (n, z) = s.get();
            s.set((n + self.0.len(), z && self.0.iter().all(|v| *v == F::ZERO)));
        });
    }
}
impl<F: StoredAssignmentFieldV1> Drop for Fields<F> {
    fn drop(&mut self) {
        self.clear();
    }
}
struct Column<F: StoredAssignmentFieldV1>(Polynomial<F, LagrangeCoeff>);
impl<F: StoredAssignmentFieldV1> Column<F> {
    fn new(domain: &EvaluationDomain<F>) -> Result<Self, StoredLookupErrorV1>
    where
        F: WithSmallOrderMulGroup<3>,
    {
        let n = usize::try_from(domain.get_n()).map_err(|_| StoredLookupErrorV1::Context)?;
        let mut values = reserve(n)?;
        values.resize(n, F::ZERO);
        Ok(Self(domain.lagrange_from_vec(values)))
    }
    fn clear(&mut self) {
        for value in &mut self.0.values {
            // SAFETY: every exclusively owned initialized Copy field admits ZERO.
            unsafe {
                ptr::write_volatile(value, F::ZERO);
            }
        }
        compiler_fence(Ordering::SeqCst);
        #[cfg(test)]
        FIELD_CLEARS.with(|s| {
            let (n, z) = s.get();
            s.set((
                n + self.0.values.len(),
                z && self.0.values.iter().all(|v| *v == F::ZERO),
            ));
        });
    }
}
impl<F: StoredAssignmentFieldV1> Drop for Column<F> {
    fn drop(&mut self) {
        self.clear();
    }
}
struct Scratch<F: StoredAssignmentFieldV1> {
    column: Column<F>,
    source: Fields<F>,
    numerator: Fields<F>,
    denominator: Fields<F>,
    inversion: Fields<F>,
    last_z: Fields<F>,
    encoded: Encoded,
}
impl<F: StoredAssignmentFieldV1> Scratch<F> {
    fn new(domain: &EvaluationDomain<F>) -> Result<Self, StoredLookupErrorV1>
    where
        F: WithSmallOrderMulGroup<3>,
    {
        Ok(Self {
            column: Column::new(domain)?,
            source: Fields::new(TILE)?,
            numerator: Fields::new(TILE)?,
            denominator: Fields::new(TILE)?,
            inversion: Fields::new(TILE)?,
            last_z: Fields::new(1)?,
            encoded: Encoded::new()?,
        })
    }
}
#[cfg(test)]
thread_local! { static FIELD_CLEARS:std::cell::Cell<(usize,bool)>=const {std::cell::Cell::new((0,true))}; }
#[cfg(test)]
pub(super) fn take_clear_observations() -> (usize, bool, usize, bool, usize, bool) {
    let (own, zero) = FIELD_CLEARS.with(|s| s.replace((0, true)));
    let (other, other_zero, bytes, bytes_zero, blinds, blinds_zero) =
        super::lookup_permuted::take_clear_observations();
    (
        own + other,
        zero && other_zero,
        bytes,
        bytes_zero,
        blinds,
        blinds_zero,
    )
}
struct ActiveProduct<C: CurveAffine, W, S>
where
    C::Scalar: StoredAssignmentFieldV1,
{
    layout: StoredPolynomialLayoutV1,
    writer: Option<W>,
    snapshot: Option<S>,
    blind: Option<SecretLookupBlindV1<C::Scalar>>,
    commitment: Option<C>,
}
struct ActiveLookup<C: CurveAffine, S>
where
    C::Scalar: StoredAssignmentFieldV1,
{
    original: CompressedLookupV1<S>,
    pair: PermutedLookupV1<C, S>,
}
/// Minimum known incremental payload, including the existing FFT's public n/2 twiddles.
/// The entry point additionally admits actual adapter Vec capacities before beta/gamma.
/// Original owner capacities, key/parameter tables, backend and MSM scratch are additional.
pub(super) fn scratch_bytes<C, S, W>(
    k: u32,
    sets: usize,
    lookups: usize,
) -> Result<usize, StoredLookupErrorV1>
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1,
{
    if k > STORED_MAX_K_V1 {
        return Err(StoredLookupErrorV1::Context);
    }
    if sets
        .checked_add(lookups)
        .ok_or(StoredLookupErrorV1::Context)?
        == 0
    {
        return Ok(0);
    }
    let n = 1_usize.checked_shl(k).ok_or(StoredLookupErrorV1::Context)?;
    payload_bytes::<C, S, W>(
        n,
        n.checked_add(4 * TILE + 1)
            .ok_or(StoredLookupErrorV1::Context)?,
        TILE,
        sets,
        lookups,
    )
}
fn payload_bytes<C, S, W>(
    n: usize,
    field_slots: usize,
    encoded_slots: usize,
    sets: usize,
    lookups: usize,
) -> Result<usize, StoredLookupErrorV1>
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1,
{
    field_slots
        .checked_add(n / 2)
        .and_then(|v| v.checked_mul(std::mem::size_of::<C::Scalar>()))
        .and_then(|v| encoded_slots.checked_mul(32).and_then(|b| v.checked_add(b)))
        .and_then(|v| {
            sets.checked_mul(std::mem::size_of::<StoredProductV1<C, S>>())
                .and_then(|m| v.checked_add(m))
        })
        .and_then(|v| {
            lookups
                .checked_mul(std::mem::size_of::<ProductLookupV1<C, S>>())
                .and_then(|m| v.checked_add(m))
        })
        .and_then(|v| v.checked_add(std::mem::size_of::<ActiveProduct<C, W, S>>()))
        .and_then(|v| v.checked_add(std::mem::size_of::<ActiveLookup<C, S>>()))
        .and_then(|v| v.checked_add(std::mem::size_of::<Scratch<C::Scalar>>()))
        .and_then(|v| v.checked_add(std::mem::size_of::<C::Curve>()))
        .and_then(|v| v.checked_add(2 * std::mem::size_of::<C>()))
        .ok_or(StoredLookupErrorV1::Context)
}
struct Work<'params, 'instances, C: CurveAffine, P: StoredPolynomialProviderV1>
where
    C::Scalar: StoredAssignmentFieldV1,
{
    params: &'params ParamsIPA<C>,
    pk: ProvingKey<C>,
    advice: Option<CoefficientStoredAdviceV1<'params, C, SnapshotOf<P>>>,
    provider: P,
    instances: &'instances [&'instances [C::Scalar]],
    original: std::vec::IntoIter<CompressedLookupV1<SnapshotOf<P>>>,
    pairs: std::vec::IntoIter<PermutedLookupV1<C, SnapshotOf<P>>>,
    permutations: Vec<StoredProductV1<C, SnapshotOf<P>>>,
    lookups: Vec<ProductLookupV1<C, SnapshotOf<P>>>,
    current: Option<ActiveLookup<C, SnapshotOf<P>>>,
    output: Option<ActiveProduct<C, P::Writer, SnapshotOf<P>>>,
    scratch: Option<Scratch<C::Scalar>>,
    n: usize,
    usable: usize,
    sets: usize,
    count: usize,
    old_end: Option<u64>,
    preprocessing_retired: bool,
}

// The context comes only from the original phase owner, including its first real zero-advice
// writer. Reconstruct a public comparison label without exposing private layout fields.
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
            .is_ok_and(|expected| layout.same_proof_context(expected))
    })
}
fn check_receipt<S: StoredPolynomialSnapshotV1>(
    layout: StoredPolynomialLayoutV1,
    snapshot: &S,
    role: StoredPolynomialRoleV1,
    basis: StoredPolynomialBasisV1,
    context: Option<[u8; 32]>,
    k: u32,
    field: crate::poly::stored_advice::StoredPastaFieldV1,
) -> Result<(), StoredLookupErrorV1> {
    if layout.role() != role
        || layout.basis() != basis
        || layout.k() != k
        || layout.field() != field
        || !context_matches(layout, context)
        || snapshot.layout() != layout
    {
        return Err(StoredPolynomialErrorV1::Context.into());
    }
    Ok(())
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

impl<'params, 'instances, C, P> Work<'params, 'instances, C, P>
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3>,
    P: StoredPolynomialProviderV1,
{
    fn advice(
        &self,
    ) -> Result<&CoefficientStoredAdviceV1<'params, C, SnapshotOf<P>>, StoredLookupErrorV1> {
        self.advice
            .as_ref()
            .ok_or(StoredPolynomialErrorV1::Poisoned.into())
    }
    fn allocated_payload(&self) -> Result<usize, StoredLookupErrorV1> {
        let Some(scratch) = &self.scratch else {
            return Ok(0);
        };
        let slots = [
            scratch.column.0.values.capacity(),
            scratch.source.0.capacity(),
            scratch.numerator.0.capacity(),
            scratch.denominator.0.capacity(),
            scratch.inversion.0.capacity(),
            scratch.last_z.0.capacity(),
        ]
        .into_iter()
        .try_fold(0_usize, |sum, cap| {
            sum.checked_add(cap).ok_or(StoredLookupErrorV1::Context)
        })?;
        payload_bytes::<C, SnapshotOf<P>, P::Writer>(
            self.n,
            slots,
            scratch.encoded.0.capacity(),
            self.permutations.capacity(),
            self.lookups.capacity(),
        )
    }
    fn validate(&self) -> Result<(), StoredLookupErrorV1> {
        let advice = self.advice()?;
        advice.validate_live_receipts()?;
        let cs = &self.pk.vk.cs;
        let k = self.pk.vk.domain.k();
        let m = cs.permutation.columns.len();
        if k > STORED_MAX_K_V1
            || self.params.k() != k
            || !std::ptr::eq(self.params, advice.params()?)
            || cs.challenge_phase.len() != cs.num_challenges
            || advice.challenges()?.count() != cs.num_challenges
            || self.params.n() != self.n as u64
            || self.n != 1_usize << k
            || cs
                .blinding_factors()
                .checked_add(1)
                .and_then(|b| self.n.checked_sub(b))
                != Some(self.usable)
            || self.pk.vk.cs_degree < 3
            || m.div_ceil(self.pk.vk.cs_degree - 2) != self.sets
            || cs.lookups.len() != self.count
            || self.permutations.len() > self.sets
            || self.pk.fixed_polys.len() != cs.num_fixed_columns
            || self.pk.fixed_polys.iter().any(|p| p.len() != self.n)
            || self.pk.permutation.polys.len() != m
            || self.pk.permutation.polys.iter().any(|p| p.len() != self.n)
            || self.instances.len() != cs.num_instance_columns
            || self.instances.iter().any(|p| p.len() > self.usable)
            || advice.layouts()?.len() != cs.num_advice_columns
            || cs.advice_column_phase.len() != cs.num_advice_columns
            || self
                .lookups
                .len()
                .checked_add(usize::from(self.current.is_some()))
                .and_then(|n| n.checked_add(self.original.len()))
                != Some(self.count)
            || self.original.len() != self.pairs.len()
        {
            return Err(StoredLookupErrorV1::Context);
        }
        if self.preprocessing_retired {
            if !self.pk.fixed_values.is_empty()
                || !self.pk.permutation.permutations.is_empty()
                || self.permutations.len() != self.sets
            {
                return Err(StoredLookupErrorV1::Context);
            }
        } else if self.pk.fixed_values.len() != cs.num_fixed_columns
            || self.pk.fixed_values.iter().any(|p| p.len() != self.n)
            || self.pk.permutation.permutations.len() != m
            || self
                .pk
                .permutation
                .permutations
                .iter()
                .any(|p| p.len() != self.n)
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
        for (layout, phase) in advice.layouts()?.zip(&cs.advice_column_phase) {
            if layout.advice_coordinates()?.1 != phase.to_u8() {
                return Err(StoredLookupErrorV1::Context);
            }
        }
        let context = advice.proof_context()?;
        let field = C::Scalar::STORED_FIELD;
        let old = |layout: StoredPolynomialLayoutV1,
                   snapshot: &SnapshotOf<P>,
                   role,
                   basis|
         -> Result<(), StoredLookupErrorV1> {
            check_receipt(layout, snapshot, role, basis, context, k, field)?;
            if self.old_end.is_none_or(|last| layout.ordinal() > last) {
                return Err(StoredPolynomialErrorV1::Context.into());
            }
            Ok(())
        };
        for (i, lookup) in self.lookups.iter().enumerate() {
            for (value, side) in [
                (&lookup.input, StoredLookupSideV1::Input),
                (&lookup.table, StoredLookupSideV1::Table),
            ] {
                old(
                    value.coefficient.layout,
                    &value.coefficient.snapshot,
                    StoredPolynomialRoleV1::LookupPermuted {
                        lookup: i as u32,
                        side,
                    },
                    StoredPolynomialBasisV1::Coefficient,
                )?;
            }
        }
        let offset = self.lookups.len();
        let originals = self
            .current
            .iter()
            .map(|v| &v.original)
            .chain(self.original.as_slice());
        let pairs = self
            .current
            .iter()
            .map(|v| &v.pair)
            .chain(self.pairs.as_slice());
        let mut last_compressed = None;
        for (i, lookup) in originals.enumerate() {
            for (value, side) in [
                (&lookup.input, StoredLookupSideV1::Input),
                (&lookup.table, StoredLookupSideV1::Table),
            ] {
                old(
                    value.layout,
                    &value.snapshot,
                    StoredPolynomialRoleV1::LookupCompressed {
                        lookup: (offset + i) as u32,
                        side,
                    },
                    StoredPolynomialBasisV1::Lagrange,
                )?;
                if last_compressed.is_some_and(|last| value.layout.ordinal() <= last) {
                    return Err(StoredPolynomialErrorV1::Context.into());
                }
                last_compressed = Some(value.layout.ordinal());
            }
        }
        let mut last_permuted = last_compressed;
        for (i, lookup) in pairs.enumerate() {
            for (value, side, basis) in [
                (
                    &lookup.input.lagrange,
                    StoredLookupSideV1::Input,
                    StoredPolynomialBasisV1::Lagrange,
                ),
                (
                    &lookup.table.lagrange,
                    StoredLookupSideV1::Table,
                    StoredPolynomialBasisV1::Lagrange,
                ),
                (
                    &lookup.input.coefficient,
                    StoredLookupSideV1::Input,
                    StoredPolynomialBasisV1::Coefficient,
                ),
                (
                    &lookup.table.coefficient,
                    StoredLookupSideV1::Table,
                    StoredPolynomialBasisV1::Coefficient,
                ),
            ] {
                old(
                    value.layout,
                    &value.snapshot,
                    StoredPolynomialRoleV1::LookupPermuted {
                        lookup: (offset + i) as u32,
                        side,
                    },
                    basis,
                )?;
                if last_permuted.is_some_and(|last| value.layout.ordinal() <= last) {
                    return Err(StoredPolynomialErrorV1::Context.into());
                }
                last_permuted = Some(value.layout.ordinal());
            }
        }
        let mut last = self.old_end;
        let mut output =
            |value: &StoredProductV1<C, SnapshotOf<P>>, role| -> Result<(), StoredLookupErrorV1> {
                check_receipt(
                    value.coefficient.layout,
                    &value.coefficient.snapshot,
                    role,
                    StoredPolynomialBasisV1::Coefficient,
                    context,
                    k,
                    field,
                )?;
                if last.is_some_and(|old| value.coefficient.layout.ordinal() <= old) {
                    return Err(StoredPolynomialErrorV1::Context.into());
                }
                last = Some(value.coefficient.layout.ordinal());
                Ok(())
            };
        for (i, value) in self.permutations.iter().enumerate() {
            output(
                value,
                StoredPolynomialRoleV1::CopyPermutationProduct { set: i as u32 },
            )?;
        }
        for (i, value) in self.lookups.iter().enumerate() {
            output(
                &value.product,
                StoredPolynomialRoleV1::LookupProduct { lookup: i as u32 },
            )?;
        }
        if let Some(active) = &self.output {
            let role = if self.permutations.len() < self.sets {
                StoredPolynomialRoleV1::CopyPermutationProduct {
                    set: self.permutations.len() as u32,
                }
            } else {
                StoredPolynomialRoleV1::LookupProduct {
                    lookup: self.lookups.len() as u32,
                }
            };
            if active.layout.role() != role
                || active.layout.basis() != StoredPolynomialBasisV1::Coefficient
                || active.layout.k() != k
                || active.layout.field() != field
                || !context_matches(active.layout, context)
                || last.is_some_and(|old| active.layout.ordinal() <= old)
                || active
                    .writer
                    .as_ref()
                    .is_some_and(|w| w.layout() != active.layout)
                || active
                    .snapshot
                    .as_ref()
                    .is_some_and(|s| s.layout() != active.layout)
                || active.writer.is_some() == active.snapshot.is_some()
            {
                return Err(StoredPolynomialErrorV1::Context.into());
            }
        }
        Ok(())
    }
    fn begin_output(&mut self) -> Result<(), StoredLookupErrorV1> {
        self.validate()?;
        if self.output.is_some() {
            return Err(StoredLookupErrorV1::Context);
        }
        let advice = self
            .advice
            .take()
            .ok_or(StoredPolynomialErrorV1::Poisoned)?;
        let (advice, writer, layout) = if self.permutations.len() < self.sets {
            advice.create_copy_product_writer(&mut self.provider, self.permutations.len() as u32)?
        } else {
            advice.create_lookup_product_writer(&mut self.provider, self.lookups.len() as u32)?
        };
        self.advice = Some(advice);
        let remaining = self
            .sets
            .checked_add(self.count)
            .and_then(|n| n.checked_sub(self.permutations.len()))
            .and_then(|n| n.checked_sub(self.lookups.len()))
            .ok_or(StoredLookupErrorV1::Context)?;
        if remaining == 0 {
            return Err(StoredLookupErrorV1::Context);
        }
        layout
            .ordinal()
            .checked_add(u64::try_from(remaining).map_err(|_| StoredLookupErrorV1::Context)?)
            .ok_or(StoredPolynomialErrorV1::Capacity)?;
        self.output = Some(ActiveProduct {
            layout,
            writer: Some(writer),
            snapshot: None,
            blind: None,
            commitment: None,
        });
        self.validate()
    }
    fn begin_lookup(&mut self) -> Result<(), StoredLookupErrorV1> {
        self.validate()?;
        if self.current.is_some() || self.output.is_some() || self.permutations.len() != self.sets {
            return Err(StoredLookupErrorV1::Context);
        }
        let original = self.original.next().ok_or(StoredLookupErrorV1::Context)?;
        let pair = self.pairs.next().ok_or(StoredLookupErrorV1::Context)?;
        self.current = Some(ActiveLookup { original, pair });
        self.validate()
    }
    fn read_copy(
        &mut self,
        column: crate::plonk::Column<Any>,
        chunk: u64,
    ) -> Result<(), StoredLookupErrorV1> {
        self.validate()?;
        let start = usize::try_from(chunk)
            .map_err(|_| StoredLookupErrorV1::Context)?
            .checked_mul(TILE)
            .ok_or(StoredLookupErrorV1::Context)?;
        let count = self
            .n
            .checked_sub(start)
            .ok_or(StoredLookupErrorV1::Context)?
            .min(TILE);
        let scratch = self.scratch.as_mut().ok_or(StoredLookupErrorV1::Context)?;
        scratch.source.clear();
        match column.column_type() {
            Any::Advice(_) => {
                let advice = self
                    .advice
                    .take()
                    .ok_or(StoredPolynomialErrorV1::Poisoned)?;
                self.advice = Some(advice.copy_lagrange_chunk_into(
                    u32::try_from(column.index()).map_err(|_| StoredLookupErrorV1::Context)?,
                    chunk,
                    &mut scratch.source.0[..count],
                )?);
            }
            Any::Fixed => scratch.source.0[..count]
                .copy_from_slice(&self.pk.fixed_values[column.index()][start..start + count]),
            Any::Instance => {
                for (offset, out) in scratch.source.0[..count].iter_mut().enumerate() {
                    *out = self.instances[column.index()]
                        .get(start + offset)
                        .copied()
                        .unwrap_or(C::Scalar::ZERO);
                }
            }
        }
        self.validate()
    }
    fn read_lookup(&mut self, which: usize, chunk: u64) -> Result<(), StoredLookupErrorV1> {
        self.validate()?;
        let count = (self.n - (chunk as usize) * TILE).min(TILE);
        let scratch = self.scratch.as_mut().ok_or(StoredLookupErrorV1::Context)?;
        scratch.source.clear();
        let source = self.current.as_mut().ok_or(StoredLookupErrorV1::Context)?;
        let (layout, snapshot) = match which {
            0 => (
                source.original.input.layout,
                &mut source.original.input.snapshot,
            ),
            1 => (
                source.original.table.layout,
                &mut source.original.table.snapshot,
            ),
            2 => (
                source.pair.input.lagrange.layout,
                &mut source.pair.input.lagrange.snapshot,
            ),
            3 => (
                source.pair.table.lagrange.layout,
                &mut source.pair.table.lagrange.snapshot,
            ),
            _ => return Err(StoredLookupErrorV1::Context),
        };
        read_chunk(snapshot, layout, chunk, &mut scratch.source.0[..count])?;
        self.validate()
    }
    fn copy_values(
        &mut self,
        beta: ChallengeBeta<C>,
        gamma: ChallengeGamma<C>,
    ) -> Result<(), StoredLookupErrorV1> {
        self.validate()?;
        let set = self.permutations.len();
        let width = self.pk.vk.cs_degree - 2;
        let first = set.checked_mul(width).ok_or(StoredLookupErrorV1::Context)?;
        let end = first
            .checked_add(width)
            .ok_or(StoredLookupErrorV1::Context)?
            .min(self.pk.vk.cs.permutation.columns.len());
        if first >= end || self.preprocessing_retired {
            return Err(StoredLookupErrorV1::Context);
        }
        {
            let scratch = self.scratch.as_mut().ok_or(StoredLookupErrorV1::Context)?;
            scratch.column.clear();
            scratch.column.0.values[0] = scratch.last_z.0[0];
        }
        for chunk in 0..self.usable.div_ceil(TILE) {
            let start = chunk * TILE;
            let count = (self.usable - start).min(TILE);
            {
                let scratch = self.scratch.as_mut().ok_or(StoredLookupErrorV1::Context)?;
                scratch.numerator.0[..count].fill(C::Scalar::ONE);
                scratch.denominator.0[..count].fill(C::Scalar::ONE);
            }
            for global in first..end {
                let column = self.pk.vk.cs.permutation.columns[global];
                self.read_copy(column, chunk as u64)?;
                let omega = self.pk.vk.domain.get_omega();
                // The exponent is the original global column index, never a tile/set index.
                let mut deltaomega = <C::Scalar as PrimeField>::DELTA.pow_vartime([global as u64])
                    * omega.pow_vartime([start as u64]);
                let scratch = self.scratch.as_mut().ok_or(StoredLookupErrorV1::Context)?;
                let sigma = &self.pk.permutation.permutations[global];
                for i in 0..count {
                    let value = scratch.source.0[i];
                    scratch.numerator.0[i] *= deltaomega * *beta + *gamma + value;
                    scratch.denominator.0[i] *= *beta * sigma[start + i] + *gamma + value;
                    deltaomega *= omega;
                }
                scratch.source.clear();
            }
            self.extend_prefix(start, count)?;
        }
        let scratch = self.scratch.as_mut().ok_or(StoredLookupErrorV1::Context)?;
        scratch.last_z.0[0] = scratch.column.0.values[self.usable];
        self.validate()
    }
    fn lookup_tile(
        &mut self,
        chunk: usize,
        beta: ChallengeBeta<C>,
        gamma: ChallengeGamma<C>,
    ) -> Result<usize, StoredLookupErrorV1> {
        let start = chunk * TILE;
        let count = (self.usable - start).min(TILE);
        {
            let scratch = self.scratch.as_mut().ok_or(StoredLookupErrorV1::Context)?;
            scratch.numerator.0[..count].fill(C::Scalar::ONE);
            scratch.denominator.0[..count].fill(C::Scalar::ONE);
        }
        for which in 0..4 {
            self.read_lookup(which, chunk as u64)?;
            let scratch = self.scratch.as_mut().ok_or(StoredLookupErrorV1::Context)?;
            let challenge = if which % 2 == 0 { *beta } else { *gamma };
            let target = if which < 2 {
                &mut scratch.numerator.0
            } else {
                &mut scratch.denominator.0
            };
            for i in 0..count {
                target[i] *= scratch.source.0[i] + challenge;
            }
            scratch.source.clear();
        }
        Ok(count)
    }
    fn extend_prefix(&mut self, start: usize, count: usize) -> Result<(), StoredLookupErrorV1> {
        let scratch = self.scratch.as_mut().ok_or(StoredLookupErrorV1::Context)?;
        // The same ff inversion primitive already used by bounded advice assignment. Zeros
        // retain ordinary batch-inversion semantics; this introduces no zero-denominator gate.
        BatchInverter::invert_with_external_scratch(
            &mut scratch.denominator.0[..count],
            &mut scratch.inversion.0[..count],
        );
        for i in 0..count {
            scratch.column.0.values[start + i + 1] = scratch.column.0.values[start + i]
                * scratch.denominator.0[i]
                * scratch.numerator.0[i];
        }
        scratch.numerator.clear();
        scratch.denominator.clear();
        scratch.inversion.clear();
        self.validate()
    }
    fn lookup_values(
        &mut self,
        beta: ChallengeBeta<C>,
        gamma: ChallengeGamma<C>,
    ) -> Result<(), StoredLookupErrorV1> {
        self.validate()?;
        {
            let scratch = self.scratch.as_mut().ok_or(StoredLookupErrorV1::Context)?;
            scratch.column.clear();
            scratch.column.0.values[0] = C::Scalar::ONE;
        }
        for chunk in 0..self.usable.div_ceil(TILE) {
            let count = self.lookup_tile(chunk, beta, gamma)?;
            self.extend_prefix(chunk * TILE, count)?;
        }
        self.validate()
    }
    fn tails<R: RngCore>(&mut self, rng: &mut R) -> Result<(), StoredLookupErrorV1> {
        self.validate()?;
        for row in self.usable + 1..self.n {
            let value = C::Scalar::random(&mut *rng);
            self.scratch
                .as_mut()
                .ok_or(StoredLookupErrorV1::Context)?
                .column
                .0
                .values[row] = value;
            self.validate()?;
        }
        Ok(())
    }
    #[cfg(feature = "sanity-checks")]
    fn lookup_sanity(
        &mut self,
        beta: ChallengeBeta<C>,
        gamma: ChallengeGamma<C>,
    ) -> Result<(), StoredLookupErrorV1> {
        self.validate()?;
        assert_eq!(
            self.scratch
                .as_ref()
                .ok_or(StoredLookupErrorV1::Context)?
                .column
                .0
                .values[0],
            C::Scalar::ONE
        );
        for chunk in 0..self.usable.div_ceil(TILE) {
            let count = self.lookup_tile(chunk, beta, gamma)?;
            let scratch = self.scratch.as_mut().ok_or(StoredLookupErrorV1::Context)?;
            for i in 0..count {
                let row = chunk * TILE + i;
                assert_eq!(
                    scratch.column.0.values[row + 1] * scratch.denominator.0[i],
                    scratch.column.0.values[row] * scratch.numerator.0[i]
                );
            }
            scratch.numerator.clear();
            scratch.denominator.clear();
        }
        assert_eq!(
            self.scratch
                .as_ref()
                .ok_or(StoredLookupErrorV1::Context)?
                .column
                .0
                .values[self.usable],
            C::Scalar::ONE
        );
        self.validate()
    }
    fn commit<R: RngCore, T: TranscriptWrite<C, E>, E: EncodedChallenge<C>>(
        &mut self,
        rng: &mut R,
        transcript: &mut T,
        copy: bool,
    ) -> Result<StoredProductV1<C, SnapshotOf<P>>, StoredLookupErrorV1> {
        self.validate()?;
        let blind = SecretLookupBlindV1(Blind(C::Scalar::random(&mut *rng)));
        self.output
            .as_mut()
            .ok_or(StoredLookupErrorV1::Context)?
            .blind = Some(blind);
        self.validate()?;
        let scratch = self.scratch.as_ref().ok_or(StoredLookupErrorV1::Context)?;
        let blind = self
            .output
            .as_ref()
            .and_then(|v| v.blind.as_ref())
            .ok_or(StoredLookupErrorV1::Context)?;
        let projective = self.params.commit_lagrange(&scratch.column.0, blind.0);
        self.validate()?;
        // Ordinary copy sets normalize after the inverse transform; lookups normalize before.
        let point = if copy {
            None
        } else {
            Some(projective.to_affine())
        };
        self.validate()?;
        self.pk.vk.domain.stored_column_transform_in_place(
            &mut self
                .scratch
                .as_mut()
                .ok_or(StoredLookupErrorV1::Context)?
                .column
                .0
                .values,
            true,
            None,
        );
        self.validate()?;
        let point = match point {
            Some(point) => point,
            None => projective.to_affine(),
        };
        self.output
            .as_mut()
            .ok_or(StoredLookupErrorV1::Context)?
            .commitment = Some(point);
        self.validate()?;
        let layout = self
            .output
            .as_ref()
            .ok_or(StoredLookupErrorV1::Context)?
            .layout;
        for chunk in 0..layout.chunk_count() {
            self.validate()?;
            let count = layout.chunk_scalar_count(chunk as u64)?;
            let start = chunk * TILE;
            let scratch = self.scratch.as_mut().ok_or(StoredLookupErrorV1::Context)?;
            for (encoded, value) in scratch.encoded.0[..count]
                .iter_mut()
                .zip(&scratch.column.0.values[start..start + count])
            {
                *encoded = value.to_repr();
            }
            let writer = self
                .output
                .as_mut()
                .and_then(|v| v.writer.as_mut())
                .ok_or(StoredLookupErrorV1::Context)?;
            writer.write_chunk(chunk as u64, &scratch.encoded.0[..count])?;
            scratch.encoded.clear();
            self.validate()?;
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
        self.validate()?;
        transcript
            .write_point(point)
            .map_err(|_| StoredLookupErrorV1::Transcript)?;
        self.validate()?;
        self.scratch
            .as_mut()
            .ok_or(StoredLookupErrorV1::Context)?
            .column
            .clear();
        let mut output = self.output.take().ok_or(StoredLookupErrorV1::Context)?;
        Ok(StoredProductV1 {
            coefficient: PermutedPolynomialV1 {
                layout: output.layout,
                snapshot: output.snapshot.take().ok_or(StoredLookupErrorV1::Context)?,
            },
            blind: output.blind.take().ok_or(StoredLookupErrorV1::Context)?,
            commitment: output
                .commitment
                .take()
                .ok_or(StoredLookupErrorV1::Context)?,
        })
    }
    fn finish_lookup(
        &mut self,
        product: StoredProductV1<C, SnapshotOf<P>>,
    ) -> Result<(), StoredLookupErrorV1> {
        // The product is already sealed, committed and transcript-bound. No callback occurs
        // during these moves; the next full sweep checks every retained/remaining receipt.
        let ActiveLookup { original, pair } =
            self.current.take().ok_or(StoredLookupErrorV1::Context)?;
        let super::lookup_permuted::PermutedLookupColumnV1 {
            lagrange: input_lagrange,
            coefficient: input_coefficient,
            blind: input_blind,
            commitment: input_commitment,
        } = pair.input;
        let super::lookup_permuted::PermutedLookupColumnV1 {
            lagrange: table_lagrange,
            coefficient: table_coefficient,
            blind: table_blind,
            commitment: table_commitment,
        } = pair.table;
        self.lookups.push(ProductLookupV1 {
            input: StoredProductV1 {
                coefficient: input_coefficient,
                blind: input_blind,
                commitment: input_commitment,
            },
            table: StoredProductV1 {
                coefficient: table_coefficient,
                blind: table_blind,
                commitment: table_commitment,
            },
            product,
        });
        drop((original, input_lagrange, table_lagrange));
        self.validate()
    }
}

impl<'params, 'instances, C, P, R, T, E, const QUERY_INSTANCE: bool, const INSTANCE_MASK: u64>
    LookupPermutedPendingStoredIpaProverV1<
        'params,
        'instances,
        C,
        P,
        R,
        T,
        E,
        QUERY_INSTANCE,
        INSTANCE_MASK,
    >
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3>,
    P: StoredPolynomialProviderV1,
    R: RngCore,
    T: TranscriptWrite<C, E>,
    E: EncodedChallenge<C>,
{
    /// Squeeze the original beta/gamma, then commit all copy sets followed by all lookups.
    /// The entire original continuation and partial outputs are consumed on error or unwind.
    /// Known adapter allocation/budget/ordinal admission precedes beta/gamma; the unchanged
    /// FFT still allocates counted public twiddles internally after randomness is consumed.
    pub(crate) fn commit_products(
        self,
        scratch_limit_bytes: usize,
    ) -> Result<
        ProductsPendingStoredIpaProverV1<
            'params,
            'instances,
            C,
            P,
            R,
            T,
            E,
            QUERY_INSTANCE,
            INSTANCE_MASK,
        >,
        StoredLookupErrorV1,
    > {
        let LookupPermutedPendingStoredIpaProverV1 {
            compressed,
            usable_rows,
            lookups: pairs,
        } = self;
        let LookupCompressedPendingStoredIpaProverV1 {
            inner,
            theta,
            lookups: original,
        } = compressed;
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
        if k > STORED_MAX_K_V1 || pk.vk.cs_degree < 3 {
            return Err(StoredLookupErrorV1::Context);
        }
        let n = 1_usize.checked_shl(k).ok_or(StoredLookupErrorV1::Context)?;
        let usable = n
            .checked_sub(
                pk.vk
                    .cs
                    .blinding_factors()
                    .checked_add(1)
                    .ok_or(StoredLookupErrorV1::Context)?,
            )
            .ok_or(StoredLookupErrorV1::Context)?;
        if usable_rows != usable {
            return Err(StoredLookupErrorV1::Context);
        }
        let count = pk.vk.cs.lookups.len();
        let sets = pk
            .vk
            .cs
            .permutation
            .columns
            .len()
            .div_ceil(pk.vk.cs_degree - 2);
        u32::try_from(count).map_err(|_| StoredLookupErrorV1::Context)?;
        u32::try_from(sets).map_err(|_| StoredLookupErrorV1::Context)?;
        u32::try_from(pk.vk.cs.num_advice_columns).map_err(|_| StoredLookupErrorV1::Context)?;
        let total = sets
            .checked_add(count)
            .ok_or(StoredLookupErrorV1::Context)?;
        let old_end = advice.product_ordinal_boundary(total)?;
        if scratch_bytes::<C, SnapshotOf<P>, P::Writer>(k, sets, count)? > scratch_limit_bytes {
            return Err(StoredLookupErrorV1::ScratchLimit);
        }
        let permutations = reserve(sets)?;
        let lookups = reserve(count)?;
        let scratch = if total == 0 {
            None
        } else {
            Some(Scratch::new(&pk.vk.domain)?)
        };
        let mut work = Work {
            params,
            pk,
            advice: Some(advice),
            provider,
            instances,
            original: original.into_iter(),
            pairs: pairs.into_iter(),
            permutations,
            lookups,
            current: None,
            output: None,
            scratch,
            n,
            usable,
            sets,
            count,
            old_end,
            preprocessing_retired: false,
        };
        if work.allocated_payload()? > scratch_limit_bytes {
            return Err(StoredLookupErrorV1::ScratchLimit);
        }
        work.validate()?;
        if total != 0 {
            work.scratch
                .as_mut()
                .ok_or(StoredLookupErrorV1::Context)?
                .last_z
                .0[0] = C::Scalar::ONE;
            if sets == 0 {
                work.begin_lookup()?;
            }
            work.begin_output()?;
        }
        let beta: ChallengeBeta<C> = transcript.squeeze_challenge_scalar();
        work.validate()?;
        let gamma: ChallengeGamma<C> = transcript.squeeze_challenge_scalar();
        work.validate()?;
        for _ in 0..sets {
            if work.output.is_none() {
                work.begin_output()?;
            }
            work.copy_values(beta, gamma)?;
            work.tails(&mut rng)?;
            let product = work.commit(&mut rng, &mut transcript, true)?;
            work.permutations.push(product);
            work.validate()?;
        }
        // Same last-use boundary as the ordinary consuming prover; coefficients remain.
        drop(std::mem::take(&mut work.pk.fixed_values));
        work.pk.permutation.drop_lagrange_polynomials();
        work.preprocessing_retired = true;
        work.validate()?;
        for _ in 0..count {
            if work.current.is_none() {
                work.begin_lookup()?;
            }
            if work.output.is_none() {
                work.begin_output()?;
            }
            work.lookup_values(beta, gamma)?;
            work.tails(&mut rng)?;
            #[cfg(feature = "sanity-checks")]
            work.lookup_sanity(beta, gamma)?;
            let product = work.commit(&mut rng, &mut transcript, false)?;
            work.finish_lookup(product)?;
        }
        work.validate()?;
        if work.current.is_some()
            || work.output.is_some()
            || work.original.len() != 0
            || work.pairs.len() != 0
        {
            return Err(StoredLookupErrorV1::Context);
        }
        let Work {
            params,
            pk,
            advice,
            provider,
            instances,
            permutations,
            lookups,
            ..
        } = work;
        Ok(ProductsPendingStoredIpaProverV1 {
            inner: CoefficientPendingStoredIpaProverV1 {
                params,
                pk,
                advice: advice.ok_or(StoredPolynomialErrorV1::Poisoned)?,
                provider,
                rng,
                transcript,
                instances,
                _challenge,
            },
            theta,
            beta,
            gamma,
            usable_rows: usable,
            permutations,
            lookups,
        })
    }
}
