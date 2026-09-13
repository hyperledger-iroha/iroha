//! Reusable retained-graph tiles for the consuming quotient part owner.
//!
//! Context comes from the original random coefficient receipt and retained key dimensions.
//! Query metadata does not invent a live coset receipt for an uncached column. The concrete
//! source owns all coefficient receipts, public transforms and cache leases, and may evict a
//! cached coset after copying a complete query tile. Its backend windows close before return.
//! Initialized field scratch is `(queries * 256 + intermediates + 256)` fields, with actual
//! vector capacity admitted once and reused. Original graph/CS/PK, query metadata, backend
//! windows, caller output and arithmetic temporaries are separate memory costs.

use super::super::{StoredExpressionErrorV1, StoredRowTileV1, TILE, clear_fields};
use super::{evaluate_calculation, field_count, scratch_bytes};
use crate::{
    arithmetic::CurveAffine,
    plonk::evaluation::{GraphEvaluator, ValueSource},
    poly::stored_advice::{
        STORED_MAX_K_V1, StoredPolynomialBasisV1, StoredPolynomialLayoutV1, StoredPolynomialRoleV1,
        assignment::StoredAssignmentFieldV1,
    },
};
use ff::Field;

/// Original owner identity and physical key dimensions, without cache-derived placeholders.
#[derive(Clone, Copy, Debug)]
pub(crate) struct StoredCosetGraphContextV1<'a> {
    /// Exact always-retained random coefficient receipt, never a synthetic coset layout.
    pub(crate) original: StoredPolynomialLayoutV1,
    /// Actual original-key extended-domain logarithm minus base k.
    pub(crate) extension_log: u32,
    /// Original key advice phases in physical-column order, including unused columns.
    pub(crate) advice_phases: &'a [u8],
    /// Original key's complete fixed-column count, including materialized selectors.
    pub(crate) fixed_columns: usize,
    /// Original key's complete instance-column count.
    pub(crate) instance_columns: usize,
    /// Original key challenge phases in physical challenge order.
    pub(crate) challenge_phases: &'a [u8],
}

impl StoredCosetGraphContextV1<'_> {
    fn validate<F: StoredAssignmentFieldV1>(&self) -> Result<(), StoredExpressionErrorV1> {
        let k = self.original.k();
        if self.original.field() != F::STORED_FIELD
            || self.original.basis() != StoredPolynomialBasisV1::Coefficient
            || self.original.role() != StoredPolynomialRoleV1::VanishingRandom
            || k > STORED_MAX_K_V1
            || self.extension_log == 0
            || self.extension_log > STORED_MAX_K_V1 - k
            || self.advice_phases.iter().any(|phase| *phase > 2)
            || self.challenge_phases.iter().any(|phase| *phase > 2)
        {
            return Err(StoredExpressionErrorV1::Context);
        }
        Ok(())
    }
}

/// Private owner interface; a successful copy has authenticated every requested scalar.
///
/// Implementations bind the complete original owner, key dimensions and current part. A
/// cache hit still consumes the concrete owner's scheduled private-query ordinal opportunity;
/// no scalar or callback borrowing a live backend window may survive a copy call. On errors
/// and unwinding the enclosing consuming owner destroys every original/cache/output handle.
/// This helper clears its workspace, but does not manufacture transactional source ownership.
pub(crate) trait StoredCosetGraphSourceV1<F: StoredAssignmentFieldV1> {
    /// Validate the exact original receipt, key dimensions, phases, current part and all owners.
    /// This performs no plaintext read and must reject unbound public fixed/cached sources.
    fn validate_context(
        &mut self,
        context: &StoredCosetGraphContextV1<'_>,
        part: u32,
    ) -> Result<(), StoredExpressionErrorV1>;

    /// Copy one full rotated advice tile after closing all authenticated backend windows.
    fn copy_advice_query_into(
        &mut self,
        column: usize,
        rotation: i32,
        tile: StoredRowTileV1,
        destination: &mut [F],
    ) -> Result<(), StoredExpressionErrorV1>;

    /// Copy one full rotated fixed tile from this same original owner's transformed public key.
    fn copy_fixed_query_into(
        &mut self,
        column: usize,
        rotation: i32,
        tile: StoredRowTileV1,
        destination: &mut [F],
    ) -> Result<(), StoredExpressionErrorV1>;

    /// Copy one full rotated instance tile after closing all authenticated backend windows.
    fn copy_instance_query_into(
        &mut self,
        column: usize,
        rotation: i32,
        tile: StoredRowTileV1,
        destination: &mut [F],
    ) -> Result<(), StoredExpressionErrorV1>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum QueryKind {
    Advice,
    Fixed,
    Instance,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Query {
    kind: QueryKind,
    column: usize,
    rotation: usize,
}

impl Query {
    fn source(value: ValueSource) -> Option<Self> {
        let (kind, column, rotation) = match value {
            ValueSource::Advice(column, rotation) => (QueryKind::Advice, column, rotation),
            ValueSource::Fixed(column, rotation) => (QueryKind::Fixed, column, rotation),
            ValueSource::Instance(column, rotation) => (QueryKind::Instance, column, rotation),
            _ => return None,
        };
        Some(Self {
            kind,
            column,
            rotation,
        })
    }
}

/// Immutable graph borrow and public query metadata; plans may be local to split owner borrows.
pub(crate) struct StoredCosetGraphPlanV1<'graph, 'context, C: CurveAffine> {
    graph: &'graph GraphEvaluator<C>,
    context: StoredCosetGraphContextV1<'context>,
    queries: Vec<Query>,
    field_count: usize,
    private_queries: usize,
    planning_temporary_bytes: usize,
}

impl<C: CurveAffine> StoredCosetGraphPlanV1<'_, '_, C> {
    /// Exact original metadata admitted before any source operation.
    pub(crate) fn context(&self) -> StoredCosetGraphContextV1<'_> {
        self.context
    }

    /// Minimum reusable initialized workspace length, including all source kinds.
    pub(crate) fn field_count(&self) -> usize {
        self.field_count
    }

    /// Logical initialized field payload; the workspace separately reports actual capacity.
    pub(crate) fn scratch_bytes(&self) -> usize {
        // Planning checks this multiplication before constructing the immutable plan.
        self.field_count * std::mem::size_of::<C::Scalar>()
    }

    /// Actual retained query-vector allocation, separate from guarded field scratch.
    pub(crate) fn metadata_bytes(&self) -> usize {
        self.queries.capacity() * std::mem::size_of::<Query>()
    }

    /// Actual capacity of temporary definition flags used while preparing this graph.
    /// The flags are already dropped; the owner includes this earlier allocation in its peak.
    pub(crate) fn planning_temporary_bytes(&self) -> usize {
        self.planning_temporary_bytes
    }

    /// One opportunity per distinct advice/instance query, including concrete cache hits.
    /// The consuming owner multiplies this by tile/part counts with checked arithmetic.
    pub(crate) fn maximum_private_query_requests(&self) -> usize {
        self.private_queries
    }
}

/// Admit the original graph without rebuilding expressions, receipts or arithmetic order.
///
/// All intermediate definitions, physical dimensions, phase ranges and workspace arithmetic
/// are checked before any source operation or witness-field allocation. Equal rotation values
/// at distinct original graph indices remain distinct query requests, preserving the bound.
pub(crate) fn prepare_stored_coset_graph_v1<'graph, 'context, C>(
    graph: &'graph GraphEvaluator<C>,
    context: StoredCosetGraphContextV1<'context>,
    scratch_limit_bytes: usize,
) -> Result<StoredCosetGraphPlanV1<'graph, 'context, C>, StoredExpressionErrorV1>
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1,
{
    context.validate::<C::Scalar>()?;
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
    let mut queries = Vec::new();
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
                    ValueSource::Challenge(index) if index >= context.challenge_phases.len() => {
                        return Err(StoredExpressionErrorV1::Context);
                    }
                    _ => (),
                }
                if let Some(query) = Query::source(source) {
                    let count = match query.kind {
                        QueryKind::Advice => context.advice_phases.len(),
                        QueryKind::Fixed => context.fixed_columns,
                        QueryKind::Instance => context.instance_columns,
                    };
                    if query.column >= count || query.rotation >= graph.rotations.len() {
                        return Err(StoredExpressionErrorV1::Context);
                    }
                    if let Err(index) = queries.binary_search(&query) {
                        let count = queries
                            .len()
                            .checked_add(1)
                            .ok_or(StoredExpressionErrorV1::Plan)?;
                        if scratch_bytes::<C::Scalar>(field_count(count, graph.num_intermediates)?)?
                            > scratch_limit_bytes
                        {
                            return Err(StoredExpressionErrorV1::ScratchLimit);
                        }
                        queries
                            .try_reserve(1)
                            .map_err(|_| StoredExpressionErrorV1::Allocation)?;
                        queries.insert(index, query);
                    }
                }
                Ok(())
            })();
        });
        result?;
        written[info.target] = true;
    }
    let field_count = field_count(queries.len(), graph.num_intermediates)?;
    // The checked byte arithmetic is also required for an empty graph.
    scratch_bytes::<C::Scalar>(field_count)?;
    let private_queries = queries
        .iter()
        .filter(|query| query.kind != QueryKind::Fixed)
        .count();
    let planning_temporary_bytes = written
        .capacity()
        .checked_mul(std::mem::size_of::<bool>())
        .ok_or(StoredExpressionErrorV1::Plan)?;
    Ok(StoredCosetGraphPlanV1 {
        graph,
        context,
        queries,
        field_count,
        private_queries,
        planning_temporary_bytes,
    })
}

/// One capacity-admitted field allocation reusable across every graph and tile of the owner.
pub(crate) struct StoredCosetGraphWorkspaceV1<F: StoredAssignmentFieldV1> {
    fields: Vec<F>,
    capacity_bytes: usize,
}

impl<F: StoredAssignmentFieldV1> StoredCosetGraphWorkspaceV1<F> {
    /// Allocate once for the greatest prepared graph, rejecting actual allocator overcapacity.
    pub(crate) fn new(
        field_count: usize,
        scratch_limit_bytes: usize,
    ) -> Result<Self, StoredExpressionErrorV1> {
        if scratch_bytes::<F>(field_count)? > scratch_limit_bytes {
            return Err(StoredExpressionErrorV1::ScratchLimit);
        }
        let mut fields = Vec::<F>::new();
        fields
            .try_reserve_exact(field_count)
            .map_err(|_| StoredExpressionErrorV1::Allocation)?;
        let capacity_bytes = scratch_bytes::<F>(fields.capacity())?;
        if capacity_bytes > scratch_limit_bytes {
            return Err(StoredExpressionErrorV1::ScratchLimit);
        }
        fields.resize(field_count, F::ZERO);
        Ok(Self {
            fields,
            capacity_bytes,
        })
    }

    /// Actual owned field-buffer capacity, including allocator surplus, checked at creation.
    pub(crate) fn scratch_bytes(&self) -> usize {
        self.capacity_bytes
    }
}

impl<F: StoredAssignmentFieldV1> Drop for StoredCosetGraphWorkspaceV1<F> {
    fn drop(&mut self) {
        clear_fields(&mut self.fields);
    }
}

struct TileWorkspace<'a, F: StoredAssignmentFieldV1>(&'a mut [F]);
impl<F: StoredAssignmentFieldV1> Drop for TileWorkspace<'_, F> {
    fn drop(&mut self) {
        clear_fields(self.0);
    }
}

/// Execute one full tile with the original operation order, then clear the reusable workspace.
///
/// Each distinct query is copied exactly once, in advice/fixed/instance and column/rotation
/// index order. All backend windows close before arithmetic or the complete-tile consumer.
/// Rotation is the ordinary Euclidean base-domain rotation inside this exact part, with no
/// extended-domain rotation multiplier. `previous` is the caller's exact outer-fold tile.
/// Empty graphs produce zero; no arithmetic reassociation or new zero shortcuts are used.
/// Every exit/unwind clears all initialized workspace fields, including unused larger-plan
/// slots. The caller must abort the consuming proof after any error; this is not a retry API.
#[allow(clippy::too_many_arguments)]
pub(crate) fn with_stored_coset_graph_chunk_v1<C, A, R>(
    plan: &StoredCosetGraphPlanV1<'_, '_, C>,
    part: u32,
    tile: StoredRowTileV1,
    workspace: &mut StoredCosetGraphWorkspaceV1<C::Scalar>,
    source: &mut A,
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
    A: StoredCosetGraphSourceV1<C::Scalar>,
{
    let scratch = TileWorkspace(&mut workspace.fields);
    let context = &plan.context;
    let size = context.original.scalar_count();
    if part >= (1_u32 << context.extension_log) {
        return Err(StoredExpressionErrorV1::Context);
    }
    if tile.start >= size
        || tile.start % TILE != 0
        || tile.len != TILE.min(size - tile.start)
        || previous.len() != tile.len
    {
        return Err(StoredExpressionErrorV1::Tile);
    }
    if challenges.len() != context.challenge_phases.len() {
        return Err(StoredExpressionErrorV1::Context);
    }
    if scratch.0.len() < plan.field_count {
        return Err(StoredExpressionErrorV1::ScratchLimit);
    }
    source.validate_context(context, part)?;
    for (slot, query) in plan.queries.iter().enumerate() {
        source.validate_context(context, part)?;
        let rotation = plan.graph.rotations[query.rotation];
        let destination = &mut scratch.0[slot * TILE..slot * TILE + tile.len];
        match query.kind {
            QueryKind::Advice => {
                source.copy_advice_query_into(query.column, rotation, tile, destination)?
            }
            QueryKind::Fixed => {
                source.copy_fixed_query_into(query.column, rotation, tile, destination)?
            }
            QueryKind::Instance => {
                source.copy_instance_query_into(query.column, rotation, tile, destination)?
            }
        }
        source.validate_context(context, part)?;
    }
    let intermediate_start = plan.queries.len() * TILE;
    let output_start = intermediate_start + plan.graph.num_intermediates;
    for (row, previous_value) in previous.iter().enumerate() {
        for info in &plan.graph.calculations {
            let get = |value: ValueSource| {
                if let Some(query) = Query::source(value) {
                    let slot = plan
                        .queries
                        .binary_search(&query)
                        .expect("immutable admitted coset query has a complete copied tile");
                    return scratch.0[slot * TILE + row];
                }
                match value {
                    ValueSource::Constant(index) => plan.graph.constants[index],
                    ValueSource::Intermediate(index) => scratch.0[intermediate_start + index],
                    ValueSource::Challenge(index) => challenges[index],
                    ValueSource::Beta() => beta,
                    ValueSource::Gamma() => gamma,
                    ValueSource::Theta() => theta,
                    ValueSource::Y() => y,
                    ValueSource::PreviousValue() => *previous_value,
                    ValueSource::Advice(_, _)
                    | ValueSource::Fixed(_, _)
                    | ValueSource::Instance(_, _) => {
                        unreachable!("all admitted polynomial queries were resolved above")
                    }
                }
            };
            let value = evaluate_calculation(&info.calculation, get);
            scratch.0[intermediate_start + info.target] = value;
        }
        scratch.0[output_start + row] = plan
            .graph
            .calculations
            .last()
            .map(|info| scratch.0[intermediate_start + info.target])
            .unwrap_or(C::Scalar::ZERO);
    }
    source.validate_context(context, part)?;
    let result = consume(&scratch.0[output_start..output_start + tile.len]);
    source.validate_context(context, part)?;
    result
}

#[cfg(test)]
#[path = "coset_tests.rs"]
mod tests;
