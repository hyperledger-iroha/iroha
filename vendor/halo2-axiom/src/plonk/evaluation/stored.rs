//! Fallible, bounded tiles for the ordinary lookup expression evaluator.
//!
//! Instructions preserve the original expression's left-before-right postorder and its exact
//! field operations. Each advice leaf occurrence reads at most two authenticated chunks per
//! tile, without nested callbacks or a decoded advice bank. The plan's live-slot count times
//! 256 canonical fields bounds initialized witness scratch; storage's plaintext window, public
//! plan metadata, borrowed fixed/instance banks, consumer copies and arithmetic temporaries are
//! separate. One caller executes a proof's tiles serially under its existing backend lease.
//!
//! TODO: Connect lookup compression and its bounded argument/output owners to an admitted stored
//! prover. This is not the optimized quotient GraphEvaluator, a permutation product builder, or
//! evidence of complete-prover memory/performance qualification. A larger proof must abort after
//! any tile error and drop any incomplete output writer rather than seal a successful prefix.

use std::{
    ptr,
    sync::atomic::{Ordering, compiler_fence},
};

use crate::{
    plonk::Expression,
    poly::{
        LagrangeCoeff, Polynomial,
        stored_advice::{
            STORED_SCALARS_PER_CHUNK_V1, StoredAdviceErrorV1, StoredAdviceLayoutV1,
            StoredAdviceSnapshotV1, StoredPolynomialBasisV1, assignment::StoredAssignmentFieldV1,
        },
    },
};

const TILE: usize = STORED_SCALARS_PER_CHUNK_V1;

/// Nonsecret failure at this internal expression-consumer boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum StoredExpressionErrorV1 {
    /// Invalid trusted dimensions, expression leaf, phase, basis or proof binding.
    Context,
    /// A selector remains, the expression is invalid, or planning arithmetic overflowed.
    Plan,
    /// The aligned output tile is outside the admitted domain.
    Tile,
    /// The retained field scratch exceeds the caller's explicit limit.
    ScratchLimit,
    /// Checked allocation of public planning metadata or guarded fields failed.
    Allocation,
    /// The authenticated backend or its canonical decoder failed.
    Store(StoredAdviceErrorV1),
    /// The complete-tile consumer rejected the result.
    Consumer,
}

impl From<StoredAdviceErrorV1> for StoredExpressionErrorV1 {
    fn from(error: StoredAdviceErrorV1) -> Self {
        Self::Store(error)
    }
}

/// Trusted per-proof bindings, borrowed immutably for the plan's lifetime.
///
/// `domain` is supplied by the proof owner, never inferred from a snapshot under examination.
/// Advice bindings are complete and in physical-column order, with unique snapshot ordinals.
/// Fixed and instance owners must supply the same base-domain or exact coset-part interpretation;
/// the `LagrangeCoeff` Rust marker alone does not prove that interpretation or key identity.
#[derive(Clone, Copy, Debug)]
pub(crate) struct StoredExpressionContextV1<'a> {
    pub(crate) domain: StoredAdviceLayoutV1,
    pub(crate) advice: &'a [StoredAdviceLayoutV1],
    pub(crate) fixed_columns: usize,
    pub(crate) instance_columns: usize,
    pub(crate) challenge_phases: &'a [u8],
}

/// Borrowed backend owner paired with independently supplied exact trusted metadata.
pub(crate) struct StoredAdviceInputV1<'a, S> {
    pub(crate) expected: StoredAdviceLayoutV1,
    pub(crate) snapshot: &'a mut S,
}

/// One aligned, nonempty tile; its final length is exactly the remaining domain rows.
#[derive(Clone, Copy, Debug)]
pub(crate) struct StoredRowTileV1 {
    pub(crate) start: usize,
    pub(crate) len: usize,
}

#[derive(Clone, Copy, Debug)]
enum Instruction<F> {
    Constant {
        target: usize,
        value: F,
    },
    Fixed {
        target: usize,
        column: usize,
        rotation: i32,
    },
    Advice {
        target: usize,
        column: usize,
        rotation: i32,
    },
    Instance {
        target: usize,
        column: usize,
        rotation: i32,
    },
    Challenge {
        target: usize,
        index: usize,
    },
    Negated {
        target: usize,
    },
    Sum {
        left: usize,
        right: usize,
    },
    Product {
        left: usize,
        right: usize,
    },
    Scaled {
        target: usize,
        scalar: F,
    },
}

/// Immutable postorder plan with fixed witness-slot pressure, independent of domain length.
#[derive(Debug)]
pub(crate) struct StoredExpressionPlanV1<'a, F> {
    context: StoredExpressionContextV1<'a>,
    instructions: Vec<Instruction<F>>,
    slots: usize,
    scratch_bytes: usize,
    advice_leaves: usize,
}

impl<F> StoredExpressionPlanV1<'_, F> {
    /// Exact initialized field payload retained during evaluation (excluding backend memory).
    pub(crate) fn scratch_bytes(&self) -> usize {
        self.scratch_bytes
    }

    /// Worst-case authenticated callbacks for one tile; short domains need only one per leaf.
    pub(crate) fn maximum_chunk_reads(&self) -> usize {
        self.advice_leaves
            * if self.context.domain.scalar_count() <= TILE {
                1
            } else {
                2
            }
    }
}

fn checked_scratch_bytes<F>(slots: usize) -> Result<usize, StoredExpressionErrorV1> {
    slots
        .checked_mul(TILE)
        .and_then(|count| count.checked_mul(std::mem::size_of::<F>()))
        .ok_or(StoredExpressionErrorV1::Plan)
}

fn push<T>(items: &mut Vec<T>, item: T) -> Result<(), StoredExpressionErrorV1> {
    items
        .try_reserve(1)
        .map_err(|_| StoredExpressionErrorV1::Allocation)?;
    items.push(item);
    Ok(())
}

fn validate_context<F: StoredAssignmentFieldV1>(
    context: &StoredExpressionContextV1<'_>,
) -> Result<(), StoredExpressionErrorV1> {
    let domain = context.domain;
    if domain.field() != F::STORED_FIELD
        || domain.basis() == StoredPolynomialBasisV1::Coefficient
        || context.challenge_phases.iter().any(|phase| *phase > 2)
    {
        return Err(StoredExpressionErrorV1::Context);
    }
    for (column, layout) in context.advice.iter().enumerate() {
        if usize::try_from(layout.column()).ok() != Some(column)
            || !layout.same_proof_context(domain)
            || layout.field() != domain.field()
            || layout.k() != domain.k()
            || layout.basis() != domain.basis()
            || context.advice[..column]
                .iter()
                .any(|previous| previous.ordinal() == layout.ordinal())
        {
            return Err(StoredExpressionErrorV1::Context);
        }
    }
    Ok(())
}

/// Compile iteratively, preserving the original tree and rejecting over-budget live scratch.
/// Constants occupy ordinary slots; no zero shortcuts, common-expression cache, reassociation,
/// or change in arithmetic dispatch is introduced. Plan allocation failures are fallible.
pub(crate) fn prepare_stored_expression_v1<'a, F: StoredAssignmentFieldV1>(
    expression: &Expression<F>,
    admitted: StoredExpressionContextV1<'a>,
    scratch_limit_bytes: usize,
) -> Result<StoredExpressionPlanV1<'a, F>, StoredExpressionErrorV1> {
    validate_context::<F>(&admitted)?;
    enum Visit<'e, F> {
        Enter(&'e Expression<F>),
        Finish(&'e Expression<F>),
    }
    let mut pending = Vec::new();
    push(&mut pending, Visit::Enter(expression))?;
    let mut instructions = Vec::new();
    let mut depth = 0_usize;
    let mut slots = 0;
    let mut advice_leaves = 0_usize;
    while let Some(visit) = pending.pop() {
        let instruction = match visit {
            Visit::Enter(node) => {
                match node {
                    Expression::Negated(child) | Expression::Scaled(child, _) => {
                        push(&mut pending, Visit::Finish(node))?;
                        push(&mut pending, Visit::Enter(child))?;
                        continue;
                    }
                    Expression::Sum(left, right) | Expression::Product(left, right) => {
                        push(&mut pending, Visit::Finish(node))?;
                        push(&mut pending, Visit::Enter(right))?;
                        push(&mut pending, Visit::Enter(left))?;
                        continue;
                    }
                    _ => (),
                }
                let target = depth;
                depth = depth.checked_add(1).ok_or(StoredExpressionErrorV1::Plan)?;
                slots = slots.max(depth);
                if checked_scratch_bytes::<F>(slots)? > scratch_limit_bytes {
                    return Err(StoredExpressionErrorV1::ScratchLimit);
                }
                match node {
                    Expression::Constant(value) => Instruction::Constant {
                        target,
                        value: *value,
                    },
                    Expression::Selector(_) => return Err(StoredExpressionErrorV1::Plan),
                    Expression::Fixed(query) => {
                        if query.column_index() >= admitted.fixed_columns {
                            return Err(StoredExpressionErrorV1::Context);
                        }
                        Instruction::Fixed {
                            target,
                            column: query.column_index(),
                            rotation: query.rotation().0,
                        }
                    }
                    Expression::Advice(query) => {
                        if admitted
                            .advice
                            .get(query.column_index())
                            .map(|layout| layout.phase())
                            != Some(query.phase())
                        {
                            return Err(StoredExpressionErrorV1::Context);
                        }
                        advice_leaves = advice_leaves
                            .checked_add(1)
                            .ok_or(StoredExpressionErrorV1::Plan)?;
                        advice_leaves
                            .checked_mul(2)
                            .ok_or(StoredExpressionErrorV1::Plan)?;
                        Instruction::Advice {
                            target,
                            column: query.column_index(),
                            rotation: query.rotation().0,
                        }
                    }
                    Expression::Instance(query) => {
                        if query.column_index() >= admitted.instance_columns {
                            return Err(StoredExpressionErrorV1::Context);
                        }
                        Instruction::Instance {
                            target,
                            column: query.column_index(),
                            rotation: query.rotation().0,
                        }
                    }
                    Expression::Challenge(query) => {
                        if admitted.challenge_phases.get(query.index()) != Some(&query.phase()) {
                            return Err(StoredExpressionErrorV1::Context);
                        }
                        Instruction::Challenge {
                            target,
                            index: query.index(),
                        }
                    }
                    _ => return Err(StoredExpressionErrorV1::Plan),
                }
            }
            Visit::Finish(node) => match node {
                Expression::Negated(_) => Instruction::Negated {
                    target: depth.checked_sub(1).ok_or(StoredExpressionErrorV1::Plan)?,
                },
                Expression::Scaled(_, scalar) => Instruction::Scaled {
                    target: depth.checked_sub(1).ok_or(StoredExpressionErrorV1::Plan)?,
                    scalar: *scalar,
                },
                Expression::Sum(_, _) | Expression::Product(_, _) => {
                    let left = depth.checked_sub(2).ok_or(StoredExpressionErrorV1::Plan)?;
                    let right = depth - 1;
                    depth -= 1;
                    if matches!(node, Expression::Sum(_, _)) {
                        Instruction::Sum { left, right }
                    } else {
                        Instruction::Product { left, right }
                    }
                }
                _ => return Err(StoredExpressionErrorV1::Plan),
            },
        };
        push(&mut instructions, instruction)?;
    }
    if depth != 1 {
        return Err(StoredExpressionErrorV1::Plan);
    }
    Ok(StoredExpressionPlanV1 {
        context: admitted,
        instructions,
        slots,
        scratch_bytes: checked_scratch_bytes::<F>(slots)?,
        advice_leaves,
    })
}

struct Scratch<F: StoredAssignmentFieldV1>(Vec<F>);

fn clear_fields<F: StoredAssignmentFieldV1>(values: &mut [F]) {
    for value in values.iter_mut() {
        // SAFETY: exclusive initialized slots of the sealed, Copy Pasta field implementations.
        unsafe { ptr::write_volatile(value, F::ZERO) };
    }
    compiler_fence(Ordering::SeqCst);
    #[cfg(test)]
    CLEAR_OBSERVATION.with(|record| {
        let (count, all_zero) = record.get();
        record.set((
            count + values.len(),
            all_zero && values.iter().all(|value| *value == F::ZERO),
        ));
    });
}

#[cfg(test)]
thread_local! {
    static CLEAR_OBSERVATION: std::cell::Cell<(usize, bool)> = const { std::cell::Cell::new((0, true)) };
}

impl<F: StoredAssignmentFieldV1> Scratch<F> {
    fn new(slots: usize) -> Result<Self, StoredExpressionErrorV1> {
        let count = slots
            .checked_mul(TILE)
            .ok_or(StoredExpressionErrorV1::Plan)?;
        let mut fields = Vec::new();
        fields
            .try_reserve_exact(count)
            .map_err(|_| StoredExpressionErrorV1::Allocation)?;
        fields.resize(count, F::ZERO);
        Ok(Self(fields))
    }

    fn slot(&mut self, index: usize) -> &mut [F] {
        &mut self.0[index * TILE..(index + 1) * TILE]
    }
}

impl<F: StoredAssignmentFieldV1> Drop for Scratch<F> {
    fn drop(&mut self) {
        clear_fields(&mut self.0);
    }
}

// i64 preserves Euclidean rotation for every i32, unlike the old evaluator's overflowing
// i32 row-plus-rotation on extreme inputs. There is deliberately no extended-domain rot_scale.
fn rotated_first(start: usize, rotation: i32, size: usize) -> usize {
    (start + i64::from(rotation).rem_euclid(size as i64) as usize) % size
}

fn read_advice<F: StoredAssignmentFieldV1, S: StoredAdviceSnapshotV1>(
    input: &mut StoredAdviceInputV1<'_, S>,
    tile: StoredRowTileV1,
    rotation: i32,
    destination: &mut [F],
) -> Result<(), StoredExpressionErrorV1> {
    let layout = input.expected;
    let size = layout.scalar_count();
    if tile.start >= size
        || tile.start % TILE != 0
        || tile.len != TILE.min(size - tile.start)
        || destination.len() < tile.len
    {
        return Err(StoredExpressionErrorV1::Tile);
    }
    if input.snapshot.layout() != layout {
        return Err(StoredExpressionErrorV1::Context);
    }
    let first = rotated_first(tile.start, rotation, size);
    if size <= TILE {
        // Both sides of a small-domain wrap live in this single authenticated chunk.
        input.snapshot.with_chunk(layout, 0, |encoded| {
            if encoded.len() != size {
                return Err(StoredAdviceErrorV1::Encoding);
            }
            for (offset, value) in destination[..tile.len].iter_mut().enumerate() {
                *value = Option::<F>::from(F::from_repr(encoded[(first + offset) % size]))
                    .ok_or(StoredAdviceErrorV1::Encoding)?;
            }
            Ok(())
        })?;
    } else {
        let mut output = 0;
        let mut row = first;
        // tile.len <= 256 and a power-of-two domain imply at most two iterations.
        while output < tile.len {
            let chunk = row / TILE;
            let within = row % TILE;
            let take = (TILE - within).min(tile.len - output);
            input.snapshot.with_chunk(layout, chunk as u64, |encoded| {
                if encoded.len() != layout.chunk_scalar_count(chunk as u64)? {
                    return Err(StoredAdviceErrorV1::Encoding);
                }
                for offset in 0..take {
                    destination[output + offset] =
                        Option::<F>::from(F::from_repr(encoded[within + offset]))
                            .ok_or(StoredAdviceErrorV1::Encoding)?;
                }
                Ok(())
            })?;
            output += take;
            row = (row + take) % size;
        }
    }
    if input.snapshot.layout() != layout {
        return Err(StoredExpressionErrorV1::Context);
    }
    Ok(())
}

// Shared preflight for expression and future graph tiles; no backend plaintext is opened here.
fn validate_inputs<F: StoredAssignmentFieldV1, S: StoredAdviceSnapshotV1>(
    context: &StoredExpressionContextV1<'_>,
    tile: StoredRowTileV1,
    advice: &[StoredAdviceInputV1<'_, S>],
    fixed: &[Polynomial<F, LagrangeCoeff>],
    instance: &[Polynomial<F, LagrangeCoeff>],
    challenges: &[F],
) -> Result<(), StoredExpressionErrorV1> {
    let size = context.domain.scalar_count();
    if tile.start >= size || tile.start % TILE != 0 || tile.len != TILE.min(size - tile.start) {
        return Err(StoredExpressionErrorV1::Tile);
    }
    if advice.len() != context.advice.len()
        || fixed.len() != context.fixed_columns
        || instance.len() != context.instance_columns
        || challenges.len() != context.challenge_phases.len()
        || fixed
            .iter()
            .chain(instance)
            .any(|column| column.len() != size)
        || advice.iter().zip(context.advice).any(|(input, expected)| {
            input.expected != *expected || input.snapshot.layout() != *expected
        })
    {
        return Err(StoredExpressionErrorV1::Context);
    }
    Ok(())
}

/// Evaluate one complete tile and expose its root slot only after all reads/arithmetic succeed.
/// All initialized owned fields are cleared on success, error and unwinding, including freed
/// right-operand slots. The backend callback closes before the next leaf or consumer executes.
pub(crate) fn with_stored_expression_chunk_v1<F, S, R>(
    plan: &StoredExpressionPlanV1<'_, F>,
    tile: StoredRowTileV1,
    advice: &mut [StoredAdviceInputV1<'_, S>],
    fixed: &[Polynomial<F, LagrangeCoeff>],
    instance: &[Polynomial<F, LagrangeCoeff>],
    challenges: &[F],
    consume: impl FnOnce(&[F]) -> Result<R, StoredExpressionErrorV1>,
) -> Result<R, StoredExpressionErrorV1>
where
    F: StoredAssignmentFieldV1,
    S: StoredAdviceSnapshotV1,
{
    validate_inputs(&plan.context, tile, advice, fixed, instance, challenges)?;
    let size = plan.context.domain.scalar_count();
    let mut scratch = Scratch::<F>::new(plan.slots)?;
    for instruction in &plan.instructions {
        match *instruction {
            Instruction::Constant { target, value } => scratch.slot(target)[..tile.len].fill(value),
            Instruction::Challenge { target, index } => {
                scratch.slot(target)[..tile.len].fill(challenges[index])
            }
            Instruction::Advice {
                target,
                column,
                rotation,
            } => read_advice(&mut advice[column], tile, rotation, scratch.slot(target))?,
            Instruction::Fixed {
                target,
                column,
                rotation,
            }
            | Instruction::Instance {
                target,
                column,
                rotation,
            } => {
                let source = if matches!(instruction, Instruction::Fixed { .. }) {
                    &fixed[column]
                } else {
                    &instance[column]
                };
                let first = rotated_first(tile.start, rotation, size);
                for (offset, value) in scratch.slot(target)[..tile.len].iter_mut().enumerate() {
                    *value = source[(first + offset) % size];
                }
            }
            Instruction::Negated { target } => {
                for value in &mut scratch.slot(target)[..tile.len] {
                    *value = -*value;
                }
            }
            Instruction::Scaled { target, scalar } => {
                for value in &mut scratch.slot(target)[..tile.len] {
                    *value = *value * scalar;
                }
            }
            Instruction::Sum { left, right } | Instruction::Product { left, right } => {
                for row in 0..tile.len {
                    let a = scratch.0[left * TILE + row];
                    let b = scratch.0[right * TILE + row];
                    scratch.0[left * TILE + row] = if matches!(instruction, Instruction::Sum { .. })
                    {
                        a + &b
                    } else {
                        a * b
                    };
                }
                clear_fields(scratch.slot(right));
            }
        }
    }
    consume(&scratch.0[..tile.len])
}

/// Bounded execution of the existing finalized quotient graph.
pub(crate) mod graph;

#[cfg(test)]
mod tests;
