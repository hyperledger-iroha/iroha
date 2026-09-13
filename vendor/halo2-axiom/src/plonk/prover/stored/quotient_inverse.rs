//! Closed original-domain quotient division and inverse mixing before commitment randomness.
//!
//! The original numerator owner, all input iterators, aliases, writers and final pieces remain
//! in one consuming continuation. Aliases have a distinct interpretation from ordinary pieces.
//! TODO: compile and qualify this private continuation with all exact predecessor overlays.

use super::{
    lookup::StoredLookupErrorV1, lookup_permuted::PermutedPolynomialV1,
    quotient::QuotientNumeratorPendingStoredIpaProverV1,
    vanishing::VanishingPendingStoredIpaProverV1,
};
use crate::{
    arithmetic::CurveAffine,
    plonk::{ProvingKey, circuit::Any},
    poly::{
        commitment::Params,
        stored_advice::{
            STORED_MAX_K_V1, STORED_SCALARS_PER_CHUNK_V1, StoredLookupSideV1,
            StoredPolynomialBasisV1, StoredPolynomialErrorV1, StoredPolynomialLayoutV1,
            StoredPolynomialProviderV1, StoredPolynomialRoleV1, StoredPolynomialSnapshotV1,
            StoredPolynomialWriterV1, assignment::StoredAssignmentFieldV1,
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
type SnapshotOf<P> =
    <<P as StoredPolynomialProviderV1>::Writer as StoredPolynomialWriterV1>::Snapshot;

/// Original proof owner and its exact ordinary quotient coefficient prefix, before commitments.
#[allow(dead_code)]
pub(crate) struct QuotientCoefficientsPendingStoredIpaProverV1<
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
    pub(super) pieces: Vec<PermutedPolynomialV1<SnapshotOf<P>>>,
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
fn capacity() -> StoredLookupErrorV1 {
    StoredPolynomialErrorV1::Capacity.into()
}

#[cfg(test)]
thread_local! {
    static CLEARS: std::cell::Cell<(usize, bool, usize, bool)> = const { std::cell::Cell::new((0, true, 0, true)) };
    static REUSE: std::cell::Cell<(usize, usize, usize, usize)> = const { std::cell::Cell::new((0, 0, 0, 0)) };
}
/// Test-only count/zero checks for initialized field slots and encoded bytes cleared by this stage.
#[cfg(test)]
pub(super) fn take_clear_observations() -> (usize, bool, usize, bool) {
    CLEARS.with(|v| v.replace((0, true, 0, true)))
}
/// Test-only field allocation count/capacity and pointers observed during the two stages.
#[cfg(test)]
pub(super) fn take_reuse_observations() -> (usize, usize, usize, usize) {
    REUSE.with(|v| v.replace((0, 0, 0, 0)))
}
fn clear_fields<F: StoredAssignmentFieldV1>(values: &mut [F]) {
    for value in values.iter_mut() {
        // SAFETY: exclusive initialized slots of the sealed Copy Pasta fields admit ZERO.
        unsafe { ptr::write_volatile(value, F::ZERO) };
    }
    compiler_fence(Ordering::SeqCst);
    #[cfg(test)]
    CLEARS.with(|v| {
        let (n, z, b, bz) = v.get();
        v.set((
            n + values.len(),
            z && values.iter().all(|x| *x == F::ZERO),
            b,
            bz,
        ));
    });
}
struct Fields<F: StoredAssignmentFieldV1>(Vec<F>);
impl<F: StoredAssignmentFieldV1> Fields<F> {
    fn new(count: usize) -> Result<Self, StoredLookupErrorV1> {
        let mut values = reserve(count)?;
        values.resize(count, F::ZERO);
        #[cfg(test)]
        REUSE.with(|v| {
            let (n, _, a, b) = v.get();
            v.set((n + 1, values.capacity(), a, b));
        });
        Ok(Self(values))
    }
}
impl<F: StoredAssignmentFieldV1> Drop for Fields<F> {
    fn drop(&mut self) {
        clear_fields(&mut self.0);
    }
}
struct Encoded(Vec<[u8; 32]>);
impl Encoded {
    fn new() -> Result<Self, StoredLookupErrorV1> {
        let mut bytes = reserve(TILE)?;
        bytes.resize(TILE, [0; 32]);
        Ok(Self(bytes))
    }
    fn clear(&mut self) {
        for value in &mut self.0 {
            for byte in value {
                // SAFETY: exclusive initialized byte slots remain valid when written as zero.
                unsafe { ptr::write_volatile(byte, 0) };
            }
        }
        compiler_fence(Ordering::SeqCst);
        #[cfg(test)]
        CLEARS.with(|v| {
            let (n, z, b, bz) = v.get();
            v.set((
                n,
                z,
                b + self.0.len() * 32,
                bz && self.0.iter().flatten().all(|x| *x == 0),
            ));
        });
    }
}
impl Drop for Encoded {
    fn drop(&mut self) {
        self.clear();
    }
}

/// This intermediate deliberately has no conversion to an ordinary coefficient receipt.
struct Alias<S> {
    layout: StoredPolynomialLayoutV1,
    snapshot: S,
}
struct Pending<W> {
    layout: StoredPolynomialLayoutV1,
    writer: W,
}
#[derive(Clone, Copy)]
struct Geometry {
    n: usize,
    m: usize,
    q: usize,
    e: u32,
    inherited: usize,
    fields: usize,
}
struct Counters {
    raw_start: usize,
    raw_floor: u64,
    alias_sealed: usize,
    alias_retired: usize,
    writer_count: usize,
    input_end: u64,
    last_created: u64,
    remaining: usize,
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
        .ok_or(StoredLookupErrorV1::Context)?;
    if e == 0 {
        return Err(StoredLookupErrorV1::Context);
    }
    let n = 1_usize << d.k();
    let m = 1_usize << e;
    let q = d.get_quotient_poly_degree();
    if q == 0 || q > m || q != cs.degree() - 1 || d.extended_len() != mul(n, m)? {
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
    if add(add(inherited, m)?, q)? > HANDLES {
        return Err(capacity());
    }
    let fields = n.max(mul(m, n.min(TILE))?);
    Ok(Geometry {
        n,
        m,
        q,
        e,
        inherited,
        fields,
    })
}
fn metadata_bytes<S, W>(
    aliases: usize,
    writers: usize,
    pieces: usize,
) -> Result<usize, StoredLookupErrorV1> {
    let arrays = add(
        add(
            mul(aliases, std::mem::size_of::<Option<Alias<S>>>())?,
            mul(writers, std::mem::size_of::<Option<Pending<W>>>())?,
        )?,
        mul(pieces, std::mem::size_of::<PermutedPolynomialV1<S>>())?,
    )?;
    // Count newly introduced fixed wrappers/counters as well as allocated capacities. Original
    // inner proof and the input iterator's inherited allocation are not duplicated or charged.
    let fixed = std::mem::size_of::<Geometry>()
        + std::mem::size_of::<Counters>()
        + std::mem::size_of::<Option<Pending<W>>>()
        + std::mem::size_of::<Vec<Option<Alias<S>>>>()
        + std::mem::size_of::<Vec<Option<Pending<W>>>>()
        + std::mem::size_of::<Vec<PermutedPolynomialV1<S>>>()
        + std::mem::size_of::<std::vec::IntoIter<PermutedPolynomialV1<S>>>()
        + std::mem::size_of::<Vec<[u8; 32]>>()
        + std::mem::size_of::<Vec<u8>>();
    add(arrays, fixed)
}
fn payload<C, P>(
    g: &Geometry,
    fields: usize,
    encoded: usize,
    aliases: usize,
    writers: usize,
    pieces: usize,
) -> Result<usize, StoredLookupErrorV1>
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1,
    P: StoredPolynomialProviderV1,
{
    let fields = mul(
        add(fields, (g.n / 2).max(g.m / 2))?,
        std::mem::size_of::<C::Scalar>(),
    )?;
    add(
        add(
            metadata_bytes::<SnapshotOf<P>, P::Writer>(aliases, writers, pieces)?,
            fields,
        )?,
        mul(encoded, 32)?,
    )
}
/// Test-only public geometry budget estimate; it cannot manufacture a numerator owner.
#[cfg(test)]
pub(super) fn scratch_bytes<C, P>(pk: &ProvingKey<C>) -> Result<usize, StoredLookupErrorV1>
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1,
    P: StoredPolynomialProviderV1,
{
    let g = geometry(pk)?;
    payload::<C, P>(&g, g.fields, TILE, g.m, g.q, g.q)
}

struct Work<'params, 'instances, C, P, R, T, E, const Q: bool, const M: u64>
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1,
    P: StoredPolynomialProviderV1,
{
    inner: Option<VanishingPendingStoredIpaProverV1<'params, 'instances, C, P, R, T, E, Q, M>>,
    raw: std::vec::IntoIter<PermutedPolynomialV1<SnapshotOf<P>>>,
    aliases: Vec<Option<Alias<SnapshotOf<P>>>>,
    alias_writer: Option<Pending<P::Writer>>,
    writers: Vec<Option<Pending<P::Writer>>>,
    pieces: Vec<PermutedPolynomialV1<SnapshotOf<P>>>,
    g: Geometry,
    c: Counters,
}

impl<'params, 'instances, C, P, R, T, E, const Q: bool, const M: u64>
    Work<'params, 'instances, C, P, R, T, E, Q, M>
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3>,
    P: StoredPolynomialProviderV1,
{
    fn inner(
        &self,
    ) -> Result<
        &VanishingPendingStoredIpaProverV1<'params, 'instances, C, P, R, T, E, Q, M>,
        StoredLookupErrorV1,
    > {
        self.inner.as_ref().ok_or(StoredLookupErrorV1::Context)
    }
    fn check(
        &self,
        layout: StoredPolynomialLayoutV1,
        live: StoredPolynomialLayoutV1,
        role: StoredPolynomialRoleV1,
        basis: StoredPolynomialBasisV1,
    ) -> Result<(), StoredLookupErrorV1> {
        let inner = self.inner()?;
        let original = inner.random.coefficient.layout;
        if layout != live
            || layout.role() != role
            || layout.basis() != basis
            || layout.field() != C::Scalar::STORED_FIELD
            || layout.k() != inner.pk.vk.domain.k()
            || !layout.same_proof_context(original)
        {
            return Err(StoredPolynomialErrorV1::Context.into());
        }
        Ok(())
    }
    fn validate(&self) -> Result<(), StoredLookupErrorV1> {
        let inner = self.inner()?;
        let cs = &inner.pk.vk.cs;
        let d = &inner.pk.vk.domain;
        let g = geometry(&inner.pk)?;
        if g.n != self.g.n
            || g.m != self.g.m
            || g.q != self.g.q
            || g.e != self.g.e
            || g.inherited != self.g.inherited
            || inner.params.k() != d.k()
            || inner.params.n() != g.n as u64
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
            || self.aliases.len() != g.m
            || self.writers.len() != g.q
            || self.c.raw_start > g.m
            || self.raw.len() != g.m - self.c.raw_start
            || self.c.alias_sealed < self.c.raw_start
            || self.c.alias_sealed > self.c.raw_start + 1
            || self.c.alias_sealed > g.m
            || self.c.alias_retired > self.c.alias_sealed
            || (self.c.alias_retired != 0 && self.pieces.len() != g.q)
            || self.c.writer_count > g.q
            || self.pieces.len() > self.c.writer_count
            || (self.c.writer_count != 0 && self.c.raw_start != g.m)
            || (self.alias_writer.is_some()
                && (self.c.raw_start == g.m || self.c.alias_sealed != self.c.raw_start))
        {
            return Err(StoredLookupErrorV1::Context);
        }
        let created = add(
            add(
                self.c.alias_sealed,
                usize::from(self.alias_writer.is_some()),
            )?,
            self.c.writer_count,
        )?;
        if add(self.c.remaining, created)? != add(g.m, g.q)? {
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
            || inner.advice.greatest_ordinal()? != Some(self.c.last_created)
            || inner.advice.proof_context()?.is_none_or(|context| {
                StoredPolynomialLayoutV1::new(
                    context,
                    inner.random.coefficient.layout.ordinal(),
                    C::Scalar::STORED_FIELD,
                    StoredPolynomialBasisV1::Coefficient,
                    d.k(),
                    StoredPolynomialRoleV1::VanishingRandom,
                )
                .map_or(true, |expected| expected != inner.random.coefficient.layout)
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
        let mut original = |value: &PermutedPolynomialV1<SnapshotOf<P>>,
                            role|
         -> Result<(), StoredLookupErrorV1> {
            self.check(
                value.layout,
                value.snapshot.layout(),
                role,
                StoredPolynomialBasisV1::Coefficient,
            )?;
            if previous.is_some_and(|last| value.layout.ordinal() <= last) {
                return Err(StoredPolynomialErrorV1::Context.into());
            }
            previous = Some(value.layout.ordinal());
            Ok(())
        };
        for (index, lookup) in inner.lookups.iter().enumerate() {
            original(
                &lookup.input.coefficient,
                StoredPolynomialRoleV1::LookupPermuted {
                    lookup: index as u32,
                    side: StoredLookupSideV1::Input,
                },
            )?;
            original(
                &lookup.table.coefficient,
                StoredPolynomialRoleV1::LookupPermuted {
                    lookup: index as u32,
                    side: StoredLookupSideV1::Table,
                },
            )?;
        }
        for (index, product) in inner.permutations.iter().enumerate() {
            original(
                &product.coefficient,
                StoredPolynomialRoleV1::CopyPermutationProduct { set: index as u32 },
            )?;
        }
        for (index, lookup) in inner.lookups.iter().enumerate() {
            original(
                &lookup.product.coefficient,
                StoredPolynomialRoleV1::LookupProduct {
                    lookup: index as u32,
                },
            )?;
        }
        for (index, instance) in inner.instance_coefficients.iter().enumerate() {
            original(
                instance,
                StoredPolynomialRoleV1::Instance {
                    column: index as u32,
                },
            )?;
        }
        original(
            &inner.random.coefficient,
            StoredPolynomialRoleV1::VanishingRandom,
        )?;
        let original_end = previous.ok_or(StoredLookupErrorV1::Context)?;
        if self.c.raw_floor < original_end
            || self.c.input_end < self.c.raw_floor
            || self.c.last_created < self.c.input_end
        {
            return Err(StoredPolynomialErrorV1::Context.into());
        }
        let mut last = self.c.raw_floor;
        for (offset, raw) in self.raw.as_slice().iter().enumerate() {
            self.check(
                raw.layout,
                raw.snapshot.layout(),
                StoredPolynomialRoleV1::QuotientNumerator,
                StoredPolynomialBasisV1::CosetPart {
                    extension_log: g.e,
                    part: (self.c.raw_start + offset) as u32,
                },
            )?;
            if raw.layout.ordinal() <= last || raw.layout.ordinal() > self.c.input_end {
                return Err(StoredPolynomialErrorV1::Context.into());
            }
            last = raw.layout.ordinal();
        }
        if last != self.c.input_end {
            return Err(StoredPolynomialErrorV1::Context.into());
        }
        let mut last = self.c.input_end;
        for (part, alias) in self.aliases.iter().enumerate() {
            let must_exist = part >= self.c.alias_retired && part < self.c.alias_sealed;
            if alias.is_some() != must_exist {
                return Err(StoredLookupErrorV1::Context);
            }
            if let Some(alias) = alias {
                self.check(
                    alias.layout,
                    alias.snapshot.layout(),
                    StoredPolynomialRoleV1::QuotientAliasedPart {
                        part: part as u32,
                        extension_log: g.e,
                    },
                    StoredPolynomialBasisV1::Coefficient,
                )?;
                if alias.layout.ordinal() <= last || alias.layout.ordinal() > self.c.last_created {
                    return Err(StoredPolynomialErrorV1::Context.into());
                }
                last = alias.layout.ordinal();
            }
        }
        if let Some(pending) = &self.alias_writer {
            self.check(
                pending.layout,
                pending.writer.layout(),
                StoredPolynomialRoleV1::QuotientAliasedPart {
                    part: self.c.raw_start as u32,
                    extension_log: g.e,
                },
                StoredPolynomialBasisV1::Coefficient,
            )?;
            if pending.layout.ordinal() <= last || pending.layout.ordinal() != self.c.last_created {
                return Err(StoredPolynomialErrorV1::Context.into());
            }
            last = pending.layout.ordinal();
        }
        for piece in 0..g.q {
            if let Some(value) = self.pieces.get(piece) {
                if self.writers[piece].is_some() {
                    return Err(StoredLookupErrorV1::Context);
                }
                self.check(
                    value.layout,
                    value.snapshot.layout(),
                    StoredPolynomialRoleV1::QuotientPiece {
                        piece: piece as u32,
                    },
                    StoredPolynomialBasisV1::Coefficient,
                )?;
                if value.layout.ordinal() <= last || value.layout.ordinal() > self.c.last_created {
                    return Err(StoredPolynomialErrorV1::Context.into());
                }
                last = value.layout.ordinal();
            } else if let Some(value) = &self.writers[piece] {
                if piece >= self.c.writer_count {
                    return Err(StoredLookupErrorV1::Context);
                }
                self.check(
                    value.layout,
                    value.writer.layout(),
                    StoredPolynomialRoleV1::QuotientPiece {
                        piece: piece as u32,
                    },
                    StoredPolynomialBasisV1::Coefficient,
                )?;
                if value.layout.ordinal() <= last || value.layout.ordinal() > self.c.last_created {
                    return Err(StoredPolynomialErrorV1::Context.into());
                }
                last = value.layout.ordinal();
            } else if piece < self.c.writer_count {
                return Err(StoredLookupErrorV1::Context);
            }
        }
        if last != self.c.last_created {
            return Err(StoredPolynomialErrorV1::Context.into());
        }
        Ok(())
    }
}

#[derive(Clone, Copy)]
enum Output {
    Alias(u32),
    Piece(u32),
}
impl<'params, 'instances, C, P, R, T, E, const Q: bool, const M: u64>
    Work<'params, 'instances, C, P, R, T, E, Q, M>
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3>,
    P: StoredPolynomialProviderV1,
{
    fn create(&mut self, output: Output) -> Result<(), StoredLookupErrorV1> {
        self.validate()?;
        match output {
            Output::Alias(part)
                if part as usize == self.c.raw_start
                    && self.alias_writer.is_none()
                    && self.c.alias_sealed == self.c.raw_start
                    && self.c.raw_start < self.g.m =>
            {
                ()
            }
            Output::Piece(piece)
                if piece as usize == self.c.writer_count
                    && self.c.writer_count < self.g.q
                    && self.c.raw_start == self.g.m
                    && self.c.alias_sealed == self.g.m =>
            {
                ()
            }
            _ => return Err(StoredLookupErrorV1::Context),
        }
        self.c.remaining = self
            .c
            .remaining
            .checked_sub(1)
            .ok_or(StoredLookupErrorV1::Context)?;
        // During this consuming phase handoff the other live receipts remain in Work. On error
        // every moved original field drops here and the outer consuming method drops Work.
        let VanishingPendingStoredIpaProverV1 {
            params,
            pk,
            advice,
            mut provider,
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
        } = self.inner.take().ok_or(StoredLookupErrorV1::Context)?;
        let (advice, writer, layout) = match output {
            Output::Alias(part) => {
                advice.create_quotient_alias_writer(&mut provider, self.g.e, part)?
            }
            Output::Piece(piece) => advice.create_quotient_piece_writer(&mut provider, piece)?,
        };
        self.inner = Some(VanishingPendingStoredIpaProverV1 {
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
        });
        self.c.last_created = layout.ordinal();
        let pending = Pending { layout, writer };
        match output {
            Output::Alias(_) => self.alias_writer = Some(pending),
            Output::Piece(piece) => {
                self.writers[piece as usize] = Some(pending);
                self.c.writer_count += 1;
            }
        }
        layout
            .ordinal()
            .checked_add(u64::try_from(self.c.remaining).map_err(|_| capacity())?)
            .and_then(|v| v.checked_add(1))
            .ok_or_else(capacity)?;
        self.validate()?;
        // All old/new owners were checked; catch drift in the actual factory result during
        // the retained-bank metadata sweep as well.
        let live = match output {
            Output::Alias(_) => &self.alias_writer,
            Output::Piece(piece) => &self.writers[piece as usize],
        }
        .as_ref()
        .ok_or(StoredLookupErrorV1::Context)?;
        if live.layout != layout || live.writer.layout() != layout {
            return Err(StoredPolynomialErrorV1::Context.into());
        }
        Ok(())
    }
    fn read_raw(
        &mut self,
        chunk: usize,
        output: &mut [C::Scalar],
    ) -> Result<(), StoredLookupErrorV1> {
        self.validate()?;
        let raw = self
            .raw
            .as_mut_slice()
            .first_mut()
            .ok_or(StoredLookupErrorV1::Context)?;
        let expected = raw.layout;
        if output.len() != expected.chunk_scalar_count(chunk as u64)?
            || raw.snapshot.layout() != expected
        {
            return Err(StoredPolynomialErrorV1::Context.into());
        }
        let mut decoded = false;
        raw.snapshot.with_chunk(expected, chunk as u64, |bytes| {
            if bytes.len() != output.len()
                || bytes.iter().any(|v| !expected.field().is_canonical(v))
            {
                return Err(StoredPolynomialErrorV1::Encoding);
            }
            for (out, value) in output.iter_mut().zip(bytes) {
                *out = Option::<C::Scalar>::from(C::Scalar::from_repr(*value))
                    .ok_or(StoredPolynomialErrorV1::Encoding)?;
            }
            decoded = true;
            Ok(())
        })?;
        if !decoded || raw.snapshot.layout() != expected {
            return Err(StoredPolynomialErrorV1::Context.into());
        }
        self.validate()
    }
    fn write_alias(
        &mut self,
        values: &[C::Scalar],
        encoded: &mut Encoded,
    ) -> Result<(), StoredLookupErrorV1> {
        if values.len() != self.g.n {
            return Err(StoredLookupErrorV1::Context);
        }
        for (chunk, fields) in values.chunks(TILE).enumerate() {
            self.validate()?;
            for (out, value) in encoded.0.iter_mut().zip(fields) {
                *out = value.to_repr();
            }
            let pending = self
                .alias_writer
                .as_mut()
                .ok_or(StoredLookupErrorV1::Context)?;
            let expected = pending.layout;
            if pending.writer.layout() != expected {
                return Err(StoredPolynomialErrorV1::Context.into());
            }
            pending
                .writer
                .write_chunk(chunk as u64, &encoded.0[..fields.len()])?;
            if pending.writer.layout() != expected {
                return Err(StoredPolynomialErrorV1::Context.into());
            }
            self.validate()?;
        }
        self.validate()?;
        let Pending { layout, writer } = self
            .alias_writer
            .take()
            .ok_or(StoredLookupErrorV1::Context)?;
        if writer.layout() != layout {
            return Err(StoredPolynomialErrorV1::Context.into());
        }
        let snapshot = writer.seal()?;
        if snapshot.layout() != layout {
            return Err(StoredPolynomialErrorV1::Context.into());
        }
        let part = self.c.raw_start;
        self.aliases[part] = Some(Alias { layout, snapshot });
        self.c.alias_sealed += 1;
        self.validate()?;
        if self.aliases[part]
            .as_ref()
            .ok_or(StoredLookupErrorV1::Context)?
            .snapshot
            .layout()
            != layout
        {
            return Err(StoredPolynomialErrorV1::Context.into());
        }
        // Successful replacement seal precedes the old raw-part destructor.
        self.validate()?;
        let retired = self.raw.next().ok_or(StoredLookupErrorV1::Context)?;
        if retired.snapshot.layout() != retired.layout {
            return Err(StoredPolynomialErrorV1::Context.into());
        }
        self.c.raw_floor = retired.layout.ordinal();
        self.c.raw_start += 1;
        drop(retired);
        self.validate()
    }
    fn read_alias(
        &mut self,
        part: usize,
        chunk: usize,
        len: usize,
        slab: &mut [C::Scalar],
    ) -> Result<(), StoredLookupErrorV1> {
        self.validate()?;
        if part >= self.g.m || slab.len() != mul(self.g.m, len)? {
            return Err(StoredLookupErrorV1::Context);
        }
        let m = self.g.m;
        let alias = self.aliases[part]
            .as_mut()
            .ok_or(StoredLookupErrorV1::Context)?;
        let expected = alias.layout;
        if len != expected.chunk_scalar_count(chunk as u64)? || alias.snapshot.layout() != expected
        {
            return Err(StoredPolynomialErrorV1::Context.into());
        }
        let mut decoded = false;
        alias.snapshot.with_chunk(expected, chunk as u64, |bytes| {
            if bytes.len() != len || bytes.iter().any(|v| !expected.field().is_canonical(v)) {
                return Err(StoredPolynomialErrorV1::Encoding);
            }
            for (row, value) in bytes.iter().enumerate() {
                slab[row * m + part] = Option::<C::Scalar>::from(C::Scalar::from_repr(*value))
                    .ok_or(StoredPolynomialErrorV1::Encoding)?;
            }
            decoded = true;
            Ok(())
        })?;
        if !decoded || alias.snapshot.layout() != expected {
            return Err(StoredPolynomialErrorV1::Context.into());
        }
        self.validate()
    }
    fn write_pieces(
        &mut self,
        chunk: usize,
        len: usize,
        slab: &[C::Scalar],
        encoded: &mut Encoded,
    ) -> Result<(), StoredLookupErrorV1> {
        if slab.len() != mul(self.g.m, len)? || len > TILE {
            return Err(StoredLookupErrorV1::Context);
        }
        for piece in 0..self.g.q {
            self.validate()?;
            for (row, out) in encoded.0[..len].iter_mut().enumerate() {
                *out = slab[row * self.g.m + piece].to_repr();
            }
            let pending = self.writers[piece]
                .as_mut()
                .ok_or(StoredLookupErrorV1::Context)?;
            let expected = pending.layout;
            if len != expected.chunk_scalar_count(chunk as u64)?
                || pending.writer.layout() != expected
            {
                return Err(StoredPolynomialErrorV1::Context.into());
            }
            pending
                .writer
                .write_chunk(chunk as u64, &encoded.0[..len])?;
            if pending.writer.layout() != expected {
                return Err(StoredPolynomialErrorV1::Context.into());
            }
            self.validate()?;
        }
        Ok(())
    }
    fn finish(&mut self) -> Result<(), StoredLookupErrorV1> {
        for piece in 0..self.g.q {
            self.validate()?;
            let Pending { layout, writer } = self.writers[piece]
                .take()
                .ok_or(StoredLookupErrorV1::Context)?;
            if writer.layout() != layout {
                return Err(StoredPolynomialErrorV1::Context.into());
            }
            let snapshot = writer.seal()?;
            if snapshot.layout() != layout {
                return Err(StoredPolynomialErrorV1::Context.into());
            }
            self.pieces.push(PermutedPolynomialV1 { layout, snapshot });
            self.validate()?;
            if self.pieces[piece].snapshot.layout() != layout {
                return Err(StoredPolynomialErrorV1::Context.into());
            }
        }
        // All final outputs are sealed before the first alias is retired. Each destructor
        // runs with every original owner, remaining alias and final output checked afterwards.
        for part in 0..self.g.m {
            self.validate()?;
            let retired = self.aliases[part]
                .take()
                .ok_or(StoredLookupErrorV1::Context)?;
            if retired.snapshot.layout() != retired.layout {
                return Err(StoredPolynomialErrorV1::Context.into());
            }
            self.c.alias_retired += 1;
            drop(retired);
            self.validate()?;
        }
        Ok(())
    }
}

impl<'params, 'instances, C, P, R, T, E, const Q: bool, const M: u64>
    QuotientNumeratorPendingStoredIpaProverV1<'params, 'instances, C, P, R, T, E, Q, M>
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3>,
    P: StoredPolynomialProviderV1,
{
    /// Consume exact undivided numerator parts into the ordinary coefficient prefix.
    ///
    /// All divisors, roots, normalization and prefix length come from the original key domain.
    /// One capacity-admitted initialized field allocation is reused for parts and row-major
    /// mixing. The stated scratch budget excludes inherited owners and whole-process memory.
    /// No RNG or transcript operation occurs; quotient blinds/commitments are a later stage.
    pub(crate) fn stage_quotient_coefficients(
        self,
        scratch_limit_bytes: usize,
    ) -> Result<
        QuotientCoefficientsPendingStoredIpaProverV1<'params, 'instances, C, P, R, T, E, Q, M>,
        StoredLookupErrorV1,
    > {
        let g = geometry(&self.inner.pk)?;
        if self.parts.len() != g.m {
            return Err(StoredLookupErrorV1::Context);
        }
        self.inner.advice.validate_live_receipts()?;
        let input_end = self
            .parts
            .last()
            .ok_or(StoredLookupErrorV1::Context)?
            .layout
            .ordinal();
        if self.inner.advice.greatest_ordinal()? != Some(input_end) {
            return Err(StoredPolynomialErrorV1::Context.into());
        }
        let opportunities = add(g.m, g.q)?;
        self.inner.advice.quotient_ordinal_boundary(opportunities)?;
        if payload::<C, P>(&g, g.fields, TILE, g.m, g.q, g.q)? > scratch_limit_bytes {
            return Err(StoredLookupErrorV1::ScratchLimit);
        }
        let mut aliases = reserve(g.m)?;
        aliases.resize_with(g.m, || None);
        let mut writers = reserve(g.q)?;
        writers.resize_with(g.q, || None);
        let pieces = reserve(g.q)?;
        let mut fields = Fields::<C::Scalar>::new(g.fields)?;
        let mut encoded = Encoded::new()?;
        if payload::<C, P>(
            &g,
            fields.0.capacity(),
            encoded.0.capacity(),
            aliases.capacity(),
            writers.capacity(),
            pieces.capacity(),
        )? > scratch_limit_bytes
        {
            return Err(StoredLookupErrorV1::ScratchLimit);
        }
        let original_end = self.inner.random.coefficient.layout.ordinal();
        let mut work = Work {
            inner: Some(self.inner),
            raw: self.parts.into_iter(),
            aliases,
            alias_writer: None,
            writers,
            pieces,
            g,
            c: Counters {
                raw_start: 0,
                raw_floor: original_end,
                alias_sealed: 0,
                alias_retired: 0,
                writer_count: 0,
                input_end,
                last_created: input_end,
                remaining: opportunities,
            },
        };
        work.validate()?;
        #[cfg(test)]
        REUSE.with(|v| {
            let (n, c, _, b) = v.get();
            v.set((n, c, fields.0.as_ptr() as usize, b));
        });
        for part in 0..g.m {
            work.create(Output::Alias(part as u32))?;
            for (chunk, output) in fields.0[..g.n].chunks_mut(TILE).enumerate() {
                work.read_raw(chunk, output)?;
            }
            work.validate()?;
            work.inner()?
                .pk
                .vk
                .domain
                .stored_quotient_part_inverse_in_place(part as u32, &mut fields.0[..g.n]);
            work.validate()?;
            work.write_alias(&fields.0[..g.n], &mut encoded)?;
            clear_fields(&mut fields.0);
            encoded.clear();
        }
        if work.raw.len() != 0 || work.c.alias_sealed != g.m {
            return Err(StoredLookupErrorV1::Context);
        }
        // The original field allocation now holds at most min(n,256) rows of m aliases.
        #[cfg(test)]
        REUSE.with(|v| {
            let (n, c, a, _) = v.get();
            v.set((n, c, a, fields.0.as_ptr() as usize));
        });
        for piece in 0..g.q {
            work.create(Output::Piece(piece as u32))?;
        }
        for start in (0..g.n).step_by(TILE) {
            let len = TILE.min(g.n - start);
            let chunk = start / TILE;
            let slab = &mut fields.0[..mul(g.m, len)?];
            for part in 0..g.m {
                work.read_alias(part, chunk, len, slab)?;
            }
            work.validate()?;
            for row in slab.chunks_exact_mut(g.m) {
                work.inner()?
                    .pk
                    .vk
                    .domain
                    .stored_quotient_piece_mix_in_place(row);
            }
            work.validate()?;
            work.write_pieces(chunk, len, slab, &mut encoded)?;
            clear_fields(slab);
            encoded.clear();
        }
        work.finish()?;
        work.validate()?;
        if work.c.remaining != 0 || work.c.alias_retired != g.m || work.pieces.len() != g.q {
            return Err(StoredLookupErrorV1::Context);
        }
        let Work { inner, pieces, .. } = work;
        Ok(QuotientCoefficientsPendingStoredIpaProverV1 {
            inner: inner.ok_or(StoredLookupErrorV1::Context)?,
            pieces,
        })
    }
}
