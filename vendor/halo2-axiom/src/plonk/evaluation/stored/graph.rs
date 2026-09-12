//! Bounded row-major execution of the retained quotient graph over stored advice tiles.
//!
//! The immutable graph keeps its existing calculation order, compacted target indices and
//! field operations. Each distinct advice column/rotation pair is prefetched once per tile;
//! only one row of graph intermediates is retained. Initialized field scratch is exactly
//! `(queries * 256 + intermediates + 256) * size_of::<F>()`, separately from the backend window,
//! borrowed keys/graphs, metadata, caller copies and arithmetic temporaries. Nothing here
//! constructs the quotient's permutation/lookup relations, stores its output, or proves RSS.
//!
//! TODO: Integrate this helper into the complete stored quotient owner while preserving the
//! outer y-fold and coset-part order. No caller may seal a successful prefix after a tile error.

use super::{
    StoredAdviceInputV1, StoredExpressionContextV1, StoredExpressionErrorV1, StoredRowTileV1, TILE,
    clear_fields, read_advice, rotated_first, validate_context, validate_inputs,
};
use ff::Field;

use crate::{
    arithmetic::CurveAffine,
    plonk::evaluation::{Calculation, GraphEvaluator, ValueSource},
    poly::{
        LagrangeCoeff, Polynomial,
        stored_advice::{StoredAdviceSnapshotV1, assignment::StoredAssignmentFieldV1},
    },
};

/// Borrowed finalized graph and authenticated domain, with only public query-index metadata.
pub(crate) struct StoredGraphPlanV1<'graph, 'context, C: CurveAffine> {
    graph: &'graph GraphEvaluator<C>,
    context: StoredExpressionContextV1<'context>,
    advice_queries: Vec<(usize, usize)>,
    field_count: usize,
    scratch_bytes: usize,
}

impl<C: CurveAffine> StoredGraphPlanV1<'_, '_, C> {
    /// Initialized owned field payload, excluding public query metadata and backend memory.
    pub(crate) fn scratch_bytes(&self) -> usize {
        self.scratch_bytes
    }

    /// Maximum authenticated reads per tile, independent of the number of graph calculations.
    pub(crate) fn maximum_chunk_reads(&self) -> usize {
        self.advice_queries.len()
            * if self.context.domain.scalar_count() <= TILE {
                1
            } else {
                2
            }
    }
}

fn field_count(queries: usize, intermediates: usize) -> Result<usize, StoredExpressionErrorV1> {
    queries
        .checked_mul(TILE)
        .and_then(|fields| fields.checked_add(intermediates))
        .and_then(|fields| fields.checked_add(TILE))
        .ok_or(StoredExpressionErrorV1::Plan)
}

fn scratch_bytes<F>(fields: usize) -> Result<usize, StoredExpressionErrorV1> {
    fields
        .checked_mul(std::mem::size_of::<F>())
        .ok_or(StoredExpressionErrorV1::Plan)
}

/// Admit exact graph references and query dimensions without cloning or rebuilding its graph.
///
/// Finalized graphs must define every intermediate before reading it. This also ensures that
/// row-major execution never reads a previous row's scratch value. Public dimensions are
/// validated before any backend access; malformed or excessive plans fail without allocation
/// of witness fields. The caller supplies the authoritative proving key's graph and bindings.
pub(crate) fn prepare_stored_graph_v1<'graph, 'context, C>(
    graph: &'graph GraphEvaluator<C>,
    context: StoredExpressionContextV1<'context>,
    scratch_limit_bytes: usize,
) -> Result<StoredGraphPlanV1<'graph, 'context, C>, StoredExpressionErrorV1>
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1,
{
    validate_context::<C::Scalar>(&context)?;
    if graph.calculation_indices.is_some() {
        return Err(StoredExpressionErrorV1::Plan);
    }
    if scratch_bytes::<C::Scalar>(field_count(0, graph.num_intermediates)?)? > scratch_limit_bytes {
        return Err(StoredExpressionErrorV1::ScratchLimit);
    }
    let mut written = Vec::new();
    written
        .try_reserve_exact(graph.num_intermediates)
        .map_err(|_| StoredExpressionErrorV1::Allocation)?;
    written.resize(graph.num_intermediates, false);
    let mut advice_queries = Vec::new();
    for info in &graph.calculations {
        if info.target >= graph.num_intermediates {
            return Err(StoredExpressionErrorV1::Plan);
        }
        let mut result = Ok(());
        info.calculation.for_each_source(|source| {
            if result.is_err() {
                return;
            }
            result = (|| {
                match source {
                    ValueSource::Constant(index) if index >= graph.constants.len() => {
                        return Err(StoredExpressionErrorV1::Plan);
                    }
                    ValueSource::Intermediate(index)
                        if !written.get(index).copied().unwrap_or(false) =>
                    {
                        return Err(StoredExpressionErrorV1::Plan);
                    }
                    ValueSource::Fixed(column, rotation) => {
                        if column >= context.fixed_columns || rotation >= graph.rotations.len() {
                            return Err(StoredExpressionErrorV1::Context);
                        }
                    }
                    ValueSource::Instance(column, rotation) => {
                        if column >= context.instance_columns || rotation >= graph.rotations.len() {
                            return Err(StoredExpressionErrorV1::Context);
                        }
                    }
                    ValueSource::Advice(column, rotation) => {
                        if column >= context.advice.len() || rotation >= graph.rotations.len() {
                            return Err(StoredExpressionErrorV1::Context);
                        }
                        // Index ordering is public; it does not reorder the graph's arithmetic.
                        if let Err(index) = advice_queries.binary_search(&(column, rotation)) {
                            let count = advice_queries
                                .len()
                                .checked_add(1)
                                .ok_or(StoredExpressionErrorV1::Plan)?;
                            let bytes = scratch_bytes::<C::Scalar>(field_count(
                                count,
                                graph.num_intermediates,
                            )?)?;
                            if bytes > scratch_limit_bytes {
                                return Err(StoredExpressionErrorV1::ScratchLimit);
                            }
                            advice_queries
                                .try_reserve(1)
                                .map_err(|_| StoredExpressionErrorV1::Allocation)?;
                            advice_queries.insert(index, (column, rotation));
                        }
                    }
                    ValueSource::Challenge(index) if index >= context.challenge_phases.len() => {
                        return Err(StoredExpressionErrorV1::Context);
                    }
                    _ => (),
                }
                Ok(())
            })();
        });
        result?;
        written[info.target] = true;
    }
    let field_count = field_count(advice_queries.len(), graph.num_intermediates)?;
    Ok(StoredGraphPlanV1 {
        graph,
        context,
        advice_queries,
        field_count,
        scratch_bytes: scratch_bytes::<C::Scalar>(field_count)?,
    })
}

struct GraphScratch<F: StoredAssignmentFieldV1>(Vec<F>);
impl<F: StoredAssignmentFieldV1> GraphScratch<F> {
    fn zeroed(count: usize) -> Result<Self, StoredExpressionErrorV1> {
        let mut fields = Vec::new();
        fields
            .try_reserve_exact(count)
            .map_err(|_| StoredExpressionErrorV1::Allocation)?;
        fields.resize(count, F::ZERO);
        Ok(Self(fields))
    }
}
impl<F: StoredAssignmentFieldV1> Drop for GraphScratch<F> {
    fn drop(&mut self) {
        clear_fields(&mut self.0);
    }
}

/// Evaluate one full base-domain or exact coset-part tile with one row of graph intermediates.
///
/// Each query's complete tile is authenticated before graph execution starts. `previous` holds
/// the caller's current outer y-fold values for this same tile. No callback is nested and no
/// complete column is materialized. On any error or unwind all initialized owned fields clear;
/// the consumer is called only after every output row has been computed successfully.
#[allow(clippy::too_many_arguments)]
pub(crate) fn with_stored_graph_chunk_v1<C, S, R>(
    plan: &StoredGraphPlanV1<'_, '_, C>,
    tile: StoredRowTileV1,
    advice: &mut [StoredAdviceInputV1<'_, S>],
    fixed: &[Polynomial<C::Scalar, LagrangeCoeff>],
    instance: &[Polynomial<C::Scalar, LagrangeCoeff>],
    challenges: &[C::Scalar],
    beta: C::Scalar,
    gamma: C::Scalar,
    theta: C::Scalar,
    y: C::Scalar,
    previous: &[C::Scalar],
    consume: impl FnOnce(&[C::Scalar]) -> Result<R, StoredExpressionErrorV1>,
) -> Result<R, StoredExpressionErrorV1>
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1,
    S: StoredAdviceSnapshotV1,
{
    let context = &plan.context;
    validate_inputs(context, tile, advice, fixed, instance, challenges)?;
    if previous.len() != tile.len {
        return Err(StoredExpressionErrorV1::Tile);
    }
    let size = context.domain.scalar_count();
    let mut scratch = GraphScratch::<C::Scalar>::zeroed(plan.field_count)?;
    for (slot, &(column, rotation)) in plan.advice_queries.iter().enumerate() {
        read_advice(
            &mut advice[column],
            tile,
            plan.graph.rotations[rotation],
            &mut scratch.0[slot * TILE..(slot + 1) * TILE],
        )?;
    }
    let intermediate_start = plan.advice_queries.len() * TILE;
    let output_start = intermediate_start + plan.graph.num_intermediates;
    for (row, previous_value) in previous.iter().enumerate() {
        for info in &plan.graph.calculations {
            let get = |source: ValueSource| match source {
                ValueSource::Constant(index) => plan.graph.constants[index],
                ValueSource::Intermediate(index) => scratch.0[intermediate_start + index],
                ValueSource::Advice(column, rotation) => {
                    let slot = plan
                        .advice_queries
                        .binary_search(&(column, rotation))
                        .expect("immutable admitted graph query has a prefetched slot");
                    scratch.0[slot * TILE + row]
                }
                ValueSource::Fixed(column, rotation) => {
                    fixed[column]
                        [rotated_first(tile.start + row, plan.graph.rotations[rotation], size)]
                }
                ValueSource::Instance(column, rotation) => {
                    instance[column]
                        [rotated_first(tile.start + row, plan.graph.rotations[rotation], size)]
                }
                ValueSource::Challenge(index) => challenges[index],
                ValueSource::Beta() => beta,
                ValueSource::Gamma() => gamma,
                ValueSource::Theta() => theta,
                ValueSource::Y() => y,
                ValueSource::PreviousValue() => *previous_value,
            };
            // Keep every original Calculation operation, including Horner's factor/start order.
            let value = match &info.calculation {
                Calculation::Add(a, b) => get(*a) + get(*b),
                Calculation::Sub(a, b) => get(*a) - get(*b),
                Calculation::Mul(a, b) => get(*a) * get(*b),
                Calculation::Square(value) => get(*value).square(),
                Calculation::Double(value) => get(*value).double(),
                Calculation::Negate(value) => -get(*value),
                Calculation::Store(value) => get(*value),
                Calculation::Horner(start, parts, factor) => {
                    let factor = get(*factor);
                    let mut value = get(*start);
                    for part in parts {
                        value = value * factor + get(*part);
                    }
                    value
                }
            };
            scratch.0[intermediate_start + info.target] = value;
        }
        scratch.0[output_start + row] = plan
            .graph
            .calculations
            .last()
            .map(|info| scratch.0[intermediate_start + info.target])
            .unwrap_or(C::Scalar::ZERO);
    }
    consume(&scratch.0[output_start..output_start + tile.len])
}

#[cfg(test)]
mod tests;
