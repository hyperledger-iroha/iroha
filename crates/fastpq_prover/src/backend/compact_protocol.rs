//! Private compact AIR engine for bounded candidate verification.
//!
//! The prover commits complete base-field LDE rows before full-Fp4 column
//! mixing, commits the mixed oracle before independent Fp4 constraint alphas,
//! then commits the quotient before joint trace/quotient FRI challenges. The
//! verifier checks only bounded authenticated openings and caller-fixed AIR and
//! public polynomials. It has no witness, trace construction, FFT or LDE replay.
//!
//! TODO: Independently qualify the connected SMT/public statement relation,
//! proximity/Fiat-Shamir/query security and the concrete six-lane construction,
//! and close the proof-size and resource gaps before production admission. The
//! fixed 375-query profile remains unqualified. Production still requires replay;
//! explicit offline byte budgets are not production admission limits. This
//! proof is not a zero-knowledge claim.

use fastpq_isi::{FASTPQ_FINAL_V1, GoldilocksDigest384V1 as Digest};
use iroha_data_model::privacy::GoldilocksDigest384V1 as WireDigest;
#[cfg(test)]
use norito::DeserializePayload;
use norito::{NoritoDeserialize, NoritoSerialize};
use rayon::prelude::*;

use super::{
    AirQuotientDomain, ExecutionMode, FriDomain, GOLDILOCKS_MODULUS, GoldilocksFp4V1,
    JointFriBatch, MerkleTreeRoleV1, fixed_domain::FixedTraceDomain,
};
#[cfg(test)]
use crate::proof::PublicIO;
use crate::{
    Error, Result,
    fft::Planner,
    proof::{FriQueryOpening, VerifyLimits, compact_fri_support},
};

#[path = "compact_protocol/shared_openings.rs"]
pub(super) mod shared_openings;

#[cfg(test)]
#[path = "compact_protocol/metal_diagnostic.rs"]
pub(super) mod metal_diagnostic;
#[path = "compact_protocol/profile.rs"]
mod profile;
#[cfg(test)]
#[path = "compact_protocol/test_fixture.rs"]
mod test_fixture;
use profile::{Binding, ProtocolTranscript};

const MAX_CONSTRAINTS: usize = 1024;
/// Maximum independently allocated row/evaluator workspaces in one proof phase.
pub(super) const MAX_PROVER_JOBS: usize = 32;

/// Exact trusted relation geometry and circuit identity; never taken from a proof.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct FixedAirSchema {
    /// Subgroup order, including all physical padding.
    pub(super) trace_rows: usize,
    /// Complete committed base-column width in canonical order.
    pub(super) width: usize,
    /// Exact independently mixed numerator count in canonical order.
    pub(super) constraints: usize,
    /// Versioned circuit, column, slot, selector and public-packing identity.
    pub(super) identity: &'static str,
}

/// Prepared prover callback; mutable captures may retain per-proof scratch space.
pub(super) type ProverEvaluator<'a> =
    Box<dyn FnMut(usize, u64, &[u64], &[u64]) -> Result<Vec<u64>> + Send + 'a>;

/// Immutable prover preparation shared across jobs; each evaluator owns its scratch.
pub(super) trait PreparedAir: Sync {
    /// Create a worker-local evaluator borrowing only immutable prepared data.
    fn evaluator(&self) -> ProverEvaluator<'_>;
}

/// A caller-authenticated fixed relation with bounded public-input evaluation.
///
/// Every returned numerator must be a fixed polynomial of degree below3N when
/// composed with degree-below-N trace columns. Its satisfied quotient therefore
/// has degree below2N, the exact joint-FRI bound used by this engine. The relation
/// implementation and schema review must establish this contract; a proof field
/// or arbitrary witness opcode cannot declare or change it.
pub(super) trait FixedAir: Sync {
    /// Return the fixed geometry and exact constraint/schema identity.
    fn schema(&self) -> FixedAirSchema;
    /// Exact canonical public bytes, already authenticated by the surrounding caller.
    fn statement_bytes(&self) -> &[u8];
    /// Evaluate every base-field numerator at x from complete current/next rows.
    fn evaluate(&self, point: u64, current: &[u64], next: &[u64]) -> Result<Vec<u64>>;
    /// Prepare prover-only acceleration without changing the verifier relation.
    fn prepare_prover(&self) -> Result<Box<dyn PreparedAir + '_>>
    where
        Self: Sized,
    {
        Ok(Box::new(DirectPrepared { relation: self }))
    }
}

struct DirectPrepared<'a, R: FixedAir + ?Sized> {
    relation: &'a R,
}

impl<R: FixedAir + ?Sized> PreparedAir for DirectPrepared<'_, R> {
    fn evaluator(&self) -> ProverEvaluator<'_> {
        Box::new(move |_, point, current, next| self.relation.evaluate(point, current, next))
    }
}

/// Private typed proof; its Norito schema is distinct from production ProofV1.
#[derive(Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize, norito::NoritoSchema)]
#[norito_schema(
    name = "fastpq_prover::backend::compact_protocol::CompactProof",
    frame = "fastpq_prover::compact_v1::SinglePhaseProofV1"
)]
pub(super) struct CompactProof {
    row_root: WireDigest,
    mixed_root: WireDigest,
    quotient_root: WireDigest,
    fri_roots: Vec<WireDigest>,
    queries: Vec<CompactQuery>,
}

#[derive(Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct CompactQuery {
    index: u32,
    current: Vec<u64>,
    next: Vec<u64>,
    current_path: Vec<WireDigest>,
    next_path: Vec<WireDigest>,
    mixed: GoldilocksFp4V1,
    mixed_path: Vec<WireDigest>,
    quotient: GoldilocksFp4V1,
    quotient_path: Vec<WireDigest>,
    fri: FriQueryOpening,
}

/// Measured successful verification work; no counter depends on a private trace.
#[cfg(test)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct VerificationWork {
    /// Exact canonical framed proof bytes, counted before cryptographic hashing.
    pub(super) proof_bytes: usize,
    /// Transcript initializations after the complete resource/canonical preflight.
    pub(super) transcripts: usize,
    /// Bounded relation evaluations, one per transcript query.
    pub(super) air_evaluations: usize,
    /// Complete row leaves hashed from authenticated openings.
    pub(super) row_leaves: usize,
    /// Full production FRI query-chain checks reused by this prototype.
    pub(super) fri_queries: usize,
}

struct Geometry {
    schema: FixedAirSchema,
    lde_rows: usize,
    domain: FriDomain,
    fri_lengths: Vec<usize>,
    terminal_degree: usize,
}

impl Geometry {
    fn new(relation: &impl FixedAir) -> Result<Self> {
        let schema = relation.schema();
        if schema.width == 0
            || schema.width > 512
            || schema.constraints == 0
            || schema.constraints > MAX_CONSTRAINTS
            || schema.identity.is_empty()
            || schema.identity.len() > 256
        {
            return Err(shape("compact relation has an unsupported fixed schema"));
        }
        FixedTraceDomain::new(&FASTPQ_FINAL_V1, schema.trace_rows)?;
        let lde_rows = schema.trace_rows * FASTPQ_FINAL_V1.fri.blowup_factor as usize;
        let domain = FriDomain::from_lde_parameters(
            FASTPQ_FINAL_V1.lde_root,
            FASTPQ_FINAL_V1.lde_log_size,
            lde_rows,
            FASTPQ_FINAL_V1.omega_coset,
        )?;
        let fri_lengths = compact_fri_support::layer_lengths(
            lde_rows,
            FASTPQ_FINAL_V1.fri.arity,
            FASTPQ_FINAL_V1.fri.max_reductions,
        )?;
        let terminal_degree = compact_fri_support::terminal_degree_bound(
            lde_rows,
            FASTPQ_FINAL_V1.fri.blowup_factor,
            FASTPQ_FINAL_V1.fri.arity,
            &fri_lengths,
        )?;
        let geometry = Self {
            schema,
            lde_rows,
            domain,
            fri_lengths,
            terminal_degree,
        };
        profile::check_geometry(&geometry)?;
        Ok(geometry)
    }
}

// Canonical compact-length Norito framing: fields carry a varint byte length,
// vectors carry an eight-byte count and framed elements. All arithmetic is
// checked even though normal proving fixes the geometry before using it.
fn wire_add(left: usize, right: usize) -> Result<usize> {
    left.checked_add(right)
        .ok_or_else(|| shape("compact wire-size arithmetic overflow"))
}

fn wire_field(payload: usize) -> Result<usize> {
    let bits = (usize::BITS - payload.leading_zeros()).max(1) as usize;
    wire_add(payload, bits.div_ceil(7))
}

fn wire_vector(count: usize, element: usize) -> Result<usize> {
    let elements = count
        .checked_mul(wire_field(element)?)
        .ok_or_else(|| shape("compact wire-size arithmetic overflow"))?;
    wire_add(8, elements)
}

fn wire_struct(fields: &[usize]) -> Result<usize> {
    fields
        .iter()
        .try_fold(0, |size, &field| wire_add(size, wire_field(field)?))
}

/// Exact fixed-layout size of the prover's temporary repeated openings.
/// This internal representation is not the caller's shared-output byte budget.
fn repeated_wire_bytes(geometry: &Geometry) -> Result<usize> {
    let depth = geometry.lde_rows.ilog2() as usize;
    let layers = geometry.fri_lengths.len();
    let terminal = *geometry
        .fri_lengths
        .last()
        .ok_or_else(|| shape("compact wire size needs a terminal layer"))?;
    let mut rounds = 8;
    for &length in geometry.fri_lengths.iter().take(layers - 1) {
        let round = wire_struct(&[
            4,
            4,
            wire_vector(2, 32)?,
            32,
            wire_vector((length / 2).ilog2() as usize, 48)?,
        ])?;
        rounds = wire_add(rounds, wire_field(round)?)?;
    }
    let fri = wire_struct(&[
        4,
        rounds,
        4,
        wire_vector(terminal, 32)?,
        wire_vector(1, 48)?,
    ])?;
    let row = wire_vector(geometry.schema.width, 8)?;
    let path = wire_vector(depth, 48)?;
    let query = wire_struct(&[4, row, row, path, path, 32, path, 32, path, fri])?;
    wire_add(
        norito::core::Header::SIZE,
        wire_struct(&[
            48,
            48,
            48,
            wire_vector(layers, 48)?,
            wire_vector(profile::QUERY_COUNT, query)?,
        ])?,
    )
}

/// Evaluate at most 32 contiguous ranges, preserving both row and error order.
/// Each range owns one scratch workspace; Rayon cannot subdivide its evaluator.
fn collect_prover_rows<T: Send>(
    length: usize,
    evaluate: impl Fn(std::ops::Range<usize>) -> Result<Vec<T>> + Sync + Send,
) -> Result<Vec<T>> {
    if length == 0 {
        return Err(shape("compact prover needs nonempty row ranges"));
    }
    let rows_per_job = length.div_ceil(MAX_PROVER_JOBS);
    let chunks: Vec<Result<Vec<T>>> = (0..length.div_ceil(rows_per_job))
        .into_par_iter()
        .map(|job| {
            let start = job * rows_per_job;
            let end = start.saturating_add(rows_per_job).min(length);
            let rows = evaluate(start..end)?;
            if rows.len() != end - start {
                return Err(shape("compact prover range returned another row count"));
            }
            Ok(rows)
        })
        .collect();
    let mut rows = Vec::with_capacity(length);
    for chunk in chunks {
        rows.extend(chunk?);
    }
    if rows.len() != length {
        return Err(shape("compact prover range returned another row count"));
    }
    Ok(rows)
}

/// Reusable committed prover data. It is never constructed by verification.
struct PreparedTrace {
    geometry: Geometry,
    columns: Vec<Vec<u64>>,
    rows: CommittedTree,
    binding: Binding,
    bound_statement: Vec<u8>,
}

struct CommittedTree {
    levels: Vec<Vec<Digest>>,
    leaf_count: usize,
}

impl CommittedTree {
    fn root(&self) -> Digest {
        self.levels.last().expect("nonempty tree")[0]
    }

    fn path(&self, mut index: usize) -> Result<Vec<WireDigest>> {
        if index >= self.leaf_count {
            return Err(Error::QueryIndexOutOfRange {
                index,
                len: self.leaf_count,
            });
        }
        let mut path = Vec::with_capacity(self.levels.len() - 1);
        for level in &self.levels[..self.levels.len() - 1] {
            path.push(WireDigest::from(level[index ^ 1]));
            index >>= 1;
        }
        Ok(path)
    }
}

/// Prove the exact fixed AIR using canonical base subgroup evaluations.
#[cfg(test)]
pub(super) fn prove(relation: &impl FixedAir, columns: &[Vec<u64>]) -> Result<CompactProof> {
    let trace = prepare_trace(relation, columns)?;
    prove_prepared(relation, &trace)
}
fn prepare_trace(relation: &impl FixedAir, columns: &[Vec<u64>]) -> Result<PreparedTrace> {
    let geometry = Geometry::new(relation)?;
    check_limit(
        "max_compact_statement_bytes",
        relation.statement_bytes().len(),
        VerifyLimits::default().max_batch_bytes,
    )?;
    if columns.len() != geometry.schema.width {
        return Err(shape(
            "compact prover requires the complete fixed column width",
        ));
    }
    for (column, values) in columns.iter().enumerate() {
        if values.len() != geometry.schema.trace_rows {
            return Err(shape(
                "compact prover columns must have the exact subgroup length",
            ));
        }
        for (row, &value) in values.iter().enumerate() {
            canonical_base(value, "compact_base_trace", &[column, row])?;
        }
    }
    let binding = Binding::new(relation, &geometry)?;
    let bound_statement = relation.statement_bytes().to_vec();
    let planner = Planner::new(&FASTPQ_FINAL_V1);
    let mut coefficients = columns.to_vec();
    planner.ifft_columns(&mut coefficients);
    let columns = planner.lde_columns(&coefficients);
    drop(coefficients);
    let leaves = collect_prover_rows(geometry.lde_rows, |indices| {
        let mut row = vec![0; geometry.schema.width];
        indices
            .map(|index| {
                fill_row(&columns, index, &mut row);
                binding.row(index, &row)
            })
            .collect()
    })?;
    let rows = binding.tree(&leaves, MerkleTreeRoleV1::AirTrace)?;
    Ok(PreparedTrace {
        geometry,
        columns,
        rows,
        binding,
        bound_statement,
    })
}

fn prove_prepared(relation: &impl FixedAir, trace: &PreparedTrace) -> Result<CompactProof> {
    let geometry = &trace.geometry;
    if relation.schema() != geometry.schema {
        return Err(shape("prepared compact trace has another fixed schema"));
    }
    if trace.bound_statement != relation.statement_bytes() {
        return Err(shape(
            "prepared compact V1 row tree belongs to another public statement",
        ));
    }
    let binding = &trace.binding;
    check_limit(
        "max_compact_statement_bytes",
        relation.statement_bytes().len(),
        VerifyLimits::default().max_batch_bytes,
    )?;
    let mut transcript = binding.transcript(relation, geometry, trace.rows.root())?;
    let mixing = transcript.columns()?;
    let mixed: Vec<_> = (0..geometry.lde_rows)
        .into_par_iter()
        .with_min_len(64)
        .map(|index| {
            trace
                .columns
                .iter()
                .zip(&mixing)
                .fold(GoldilocksFp4V1::ZERO, |sum, (column, coefficient)| {
                    sum.add(coefficient.mul_base(column[index]))
                })
        })
        .collect();
    let mixed_leaves = {
        let results: Vec<Result<Digest>> = mixed
            .par_iter()
            .enumerate()
            .map(|(i, &v)| binding.mixed(i, v))
            .collect();
        results.into_iter().collect::<Result<Vec<Digest>>>()?
    };
    let mixed_tree = binding.tree(&mixed_leaves, MerkleTreeRoleV1::Lde)?;
    drop(mixed_leaves);
    let alphas = transcript.alphas(mixed_tree.root())?;
    let weights = AirQuotientDomain::new(&FASTPQ_FINAL_V1, geometry.lde_rows)?;
    let prepared = relation.prepare_prover()?;
    let quotients = collect_prover_rows(geometry.lde_rows, |indices| {
        let mut evaluate = prepared.evaluator();
        let mut current = vec![0; geometry.schema.width];
        let mut next = vec![0; geometry.schema.width];
        indices
            .map(|index| {
                fill_row(&trace.columns, index, &mut current);
                fill_row(
                    &trace.columns,
                    next_index(index, geometry.lde_rows),
                    &mut next,
                );
                let residues = evaluate(index, geometry.domain.point(index), &current, &next)?;
                Ok(combine(&residues, &alphas)?.mul_base(weights.weights_at(index)?.all_rows))
            })
            .collect()
    })?;
    drop(prepared); // No evaluator remains; release all prover-only fixed LDEs.
    let quotient_leaves = {
        let results: Vec<Result<Digest>> = quotients
            .par_iter()
            .enumerate()
            .map(|(i, &v)| binding.quotient(i, v))
            .collect();
        results.into_iter().collect::<Result<Vec<Digest>>>()?
    };
    let quotient_tree = binding.tree(&quotient_leaves, MerkleTreeRoleV1::AirComposition)?;
    drop(quotient_leaves);
    let joint = transcript.joint(quotient_tree.root())?;
    let (mut fri, indices) = fold_protocol_layers(
        &joint.values(&quotients, &mixed)?,
        geometry,
        binding,
        &mut transcript,
    )?;
    let chains = fri.open_query_chains(&indices, FASTPQ_FINAL_V1.fri.arity)?;
    let mut queries = Vec::with_capacity(indices.len());
    let mut current = vec![0; geometry.schema.width];
    let mut next = current.clone();
    for (index, fri) in indices.into_iter().zip(chains) {
        fill_row(&trace.columns, index, &mut current);
        let next_index = next_index(index, geometry.lde_rows);
        fill_row(&trace.columns, next_index, &mut next);
        queries.push(CompactQuery {
            index: index as u32,
            current: current.clone(),
            next: next.clone(),
            current_path: trace.rows.path(index)?,
            next_path: trace.rows.path(next_index)?,
            mixed: mixed[index],
            mixed_path: mixed_tree.path(index)?,
            quotient: quotients[index],
            quotient_path: quotient_tree.path(index)?,
            fri,
        });
    }
    Ok(CompactProof {
        row_root: trace.rows.root().into(),
        mixed_root: mixed_tree.root().into(),
        quotient_root: quotient_tree.root().into(),
        fri_roots: fri.roots.into_iter().map(WireDigest::from).collect(),
        queries,
    })
}

// The fixed binding owns every commitment and transcript message; the FRI
// fold arithmetic and retained opening owner share one deterministic schedule.
fn fold_protocol_layers(
    evaluations: &[GoldilocksFp4V1],
    geometry: &Geometry,
    binding: &Binding,
    transcript: &mut ProtocolTranscript,
) -> Result<(super::FriOpeningLayers, Vec<usize>)> {
    if evaluations.len() != geometry.lde_rows {
        return Err(shape("compact FRI requires the exact initial domain"));
    }
    let mut current = evaluations.to_vec();
    let mut domain = geometry.domain;
    let mut layer_values = Vec::with_capacity(geometry.fri_lengths.len());
    let mut roots = Vec::with_capacity(geometry.fri_lengths.len());
    let mut betas = Vec::with_capacity(geometry.fri_lengths.len() - 1);
    let mut opening_levels = Vec::with_capacity(geometry.fri_lengths.len());
    for round in 0..geometry.fri_lengths.len() - 1 {
        if current.len() != geometry.fri_lengths[round] {
            return Err(shape(
                "compact FRI layer length differs from fixed geometry",
            ));
        }
        let leaves = {
            let half = current.len() / 2;
            let results: Vec<Result<Digest>> = (0..half)
                .into_par_iter()
                .map(|i| binding.fri(round, i, &[current[i], current[i + half]]))
                .collect();
            results.into_iter().collect::<Result<Vec<Digest>>>()?
        };
        let tree = binding.tree(&leaves, MerkleTreeRoleV1::Fri(round as u32))?;
        let root = tree.root();
        opening_levels.push(tree.levels);
        roots.push(root);
        let beta = transcript.beta(round, root)?;
        betas.push(beta);
        let next = super::fold_round(&current, 2, beta, domain)?;
        layer_values.push(current);
        current = next;
        domain = domain.folded(2);
    }
    let final_round = geometry.fri_lengths.len() - 1;
    if current.len() != geometry.fri_lengths[final_round] {
        return Err(shape(
            "compact FRI terminal length differs from fixed geometry",
        ));
    }
    let terminal_leaf = binding.fri(final_round, 0, &current)?;
    let tree = binding.tree(&[terminal_leaf], MerkleTreeRoleV1::Fri(final_round as u32))?;
    let root = tree.root();
    opening_levels.push(tree.levels);
    roots.push(root);
    layer_values.push(current);
    let indices = transcript.queries(root)?;
    Ok((
        super::FriOpeningLayers {
            layer_values,
            roots,
            betas,
            opening_trees: Some(super::fri_openings::FriOpeningTrees::from_levels(
                opening_levels,
                ExecutionMode::Cpu,
            )?),
        },
        indices,
    ))
}

/// Verify using only bounded rows/oracle/FRI openings and known public AIR.
///
/// The supplied limits are trusted caller policy. Replay defaults do not admit
/// the fixed compact geometry; diagnostic envelopes must be selected explicitly.
#[cfg(test)]
pub(super) fn verify(
    relation: &impl FixedAir,
    proof: &CompactProof,
    limits: VerifyLimits,
) -> Result<VerificationWork> {
    let mut work = VerificationWork::default();
    verify_recorded(relation, proof, limits, &mut work)?;
    Ok(work)
}

#[cfg(test)]
fn verify_recorded(
    relation: &impl FixedAir,
    proof: &CompactProof,
    limits: VerifyLimits,
    work: &mut VerificationWork,
) -> Result<()> {
    let shared = shared_openings::from_compact(relation, proof, limits)?;
    let mut actual = shared_openings::SharedVerificationWork::default();
    let result = shared_openings::verify_shared_recorded(relation, &shared, limits, &mut actual);
    work.proof_bytes = actual.proof_bytes;
    work.transcripts = actual.transcripts;
    work.air_evaluations = actual.air_evaluations;
    work.row_leaves = actual.row_leaves;
    work.fri_queries = if result.is_ok() {
        profile::QUERY_COUNT
    } else {
        0
    };
    result
}

fn preflight(
    relation: &impl FixedAir,
    proof: &CompactProof,
    limits: VerifyLimits,
    geometry: &Geometry,
) -> Result<usize> {
    check_limit(
        "max_compact_statement_bytes",
        relation.statement_bytes().len(),
        limits.max_batch_bytes,
    )?;
    check_limit(
        "max_air_row_values",
        geometry.schema.width,
        limits.max_air_row_values,
    )?;
    check_limit(
        "max_fri_layers",
        proof.fri_roots.len(),
        limits.max_fri_layers,
    )?;
    check_limit("max_queries", proof.queries.len(), limits.max_queries)?;
    if proof.fri_roots.len() != geometry.fri_lengths.len()
        || proof.queries.len() != profile::QUERY_COUNT
    {
        return Err(shape(
            "compact proof needs the exact canonical layer and query counts",
        ));
    }
    let path_depth = geometry.lde_rows.ilog2() as usize;
    for query in &proof.queries {
        if query.index as usize >= geometry.lde_rows
            || query.current.len() != geometry.schema.width
            || query.next.len() != geometry.schema.width
        {
            return Err(shape(
                "compact opening has another exact index or row width",
            ));
        }
        for path in [
            &query.current_path,
            &query.next_path,
            &query.mixed_path,
            &query.quotient_path,
        ] {
            check_limit("max_query_path_len", path.len(), limits.max_query_path_len)?;
            if path.len() != path_depth {
                return Err(shape("compact oracle opening has another path depth"));
            }
        }
        check_limit(
            "max_fri_layers",
            query.fri.rounds.len(),
            limits.max_fri_layers,
        )?;
        if query.fri.rounds.len() + 1 != geometry.fri_lengths.len() {
            return Err(shape("compact FRI opening has another fold count"));
        }
        for (round, opening) in query.fri.rounds.iter().enumerate() {
            check_limit(
                "max_fri_round_values",
                opening.values.len(),
                limits.max_fri_round_values,
            )?;
            check_limit(
                "max_query_path_len",
                opening.merkle_path.len(),
                limits.max_query_path_len,
            )?;
            if opening.values.len() != 2
                || opening.merkle_path.len() != (geometry.fri_lengths[round] / 2).ilog2() as usize
            {
                return Err(shape(
                    "compact FRI group has another exact width or path depth",
                ));
            }
        }
        check_limit(
            "max_fri_round_values",
            query.fri.final_values.len(),
            limits.max_fri_round_values,
        )?;
        check_limit(
            "max_query_path_len",
            query.fri.final_merkle_path.len(),
            limits.max_query_path_len,
        )?;
        if query.fri.final_values.len()
            != *geometry.fri_lengths.last().expect("nonempty FRI geometry")
            || query.fri.final_merkle_path.len() != 1
        {
            return Err(shape(
                "compact FRI terminal must open its complete four-value leaf",
            ));
        }
    }
    // Count the exact canonical framed encoding with a counting sink, not an
    // allocation based on an untrusted size hint. All enclosing lengths and
    // exact per-opening dimensions have already been bounded above.
    let _flags = norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
    let bytes = norito::core::encoded_frame_len(proof)?;
    check_limit("max_proof_bytes", bytes, limits.max_proof_bytes)?;
    for (position, query) in proof.queries.iter().enumerate() {
        for (side, row) in [&query.current, &query.next].into_iter().enumerate() {
            for (column, &value) in row.iter().enumerate() {
                canonical_base(value, "compact_opening_row", &[position, side, column])?;
            }
        }
        canonical_extension(query.mixed, &[position, 0])?;
        canonical_extension(query.quotient, &[position, 1])?;
        for (round, opening) in query.fri.rounds.iter().enumerate() {
            for (value, &field) in opening.values.iter().enumerate() {
                canonical_extension(field, &[position, round, value])?;
            }
            canonical_extension(opening.folded_value, &[position, round, 2])?;
        }
        for (value, &field) in query.fri.final_values.iter().enumerate() {
            canonical_extension(field, &[position, geometry.fri_lengths.len(), value])?;
        }
    }
    Ok(bytes)
}
fn combine(residues: &[u64], alphas: &[GoldilocksFp4V1]) -> Result<GoldilocksFp4V1> {
    if residues.len() != alphas.len() {
        return Err(shape("fixed AIR returned another exact numerator count"));
    }
    let mut value = GoldilocksFp4V1::ZERO;
    for (index, (&residue, &alpha)) in residues.iter().zip(alphas).enumerate() {
        canonical_base(residue, "compact_relation_numerator", &[index])?;
        value = value.add(alpha.mul_base(residue));
    }
    Ok(value)
}

fn fill_row(columns: &[Vec<u64>], index: usize, row: &mut [u64]) {
    for (value, column) in row.iter_mut().zip(columns) {
        *value = column[index];
    }
}

fn next_index(index: usize, lde_rows: usize) -> usize {
    (index + FASTPQ_FINAL_V1.fri.blowup_factor as usize) % lde_rows
}
fn canonical_base(value: u64, context: &'static str, indices: &[usize]) -> Result<()> {
    if value >= GOLDILOCKS_MODULUS {
        return Err(Error::NonCanonicalGoldilocksElement {
            context,
            indices: indices.to_vec(),
        });
    }
    Ok(())
}

fn canonical_extension(value: GoldilocksFp4V1, indices: &[usize]) -> Result<()> {
    for (lane, coefficient) in value.coefficients().into_iter().enumerate() {
        if coefficient >= GOLDILOCKS_MODULUS {
            let mut indices = indices.to_vec();
            indices.push(lane);
            return Err(Error::NonCanonicalGoldilocksElement {
                context: "compact_opening_fp4",
                indices,
            });
        }
    }
    Ok(())
}

fn check_limit(limit: &'static str, actual: usize, max: usize) -> Result<()> {
    if actual > max {
        return Err(Error::VerifierLimitExceeded { limit, actual, max });
    }
    Ok(())
}

fn shape(details: &str) -> Error {
    Error::InvalidTraceShape {
        details: details.to_owned(),
    }
}

/// One private-preimage/public-marked-digest relation for the first integration.
///
/// The 680 hash slots are followed by eight independently mixed public digest
/// limb equalities at fixed export row407. This does not claim a public preimage
/// or zero knowledge; it demonstrates meaningful public binding in the engine.
#[cfg(test)]
pub(super) struct HashDigestAir {
    ledger: super::compact_hash_quotient::CompactHashQuotient,
    public_digest: [u8; 32],
    public_limbs: [u64; 8],
    export_point: u64,
}

#[cfg(test)]
impl HashDigestAir {
    /// Fix the complete public digest before any proof challenge.
    pub(super) fn new(public_digest: [u8; 32]) -> Result<Self> {
        let generator = FixedTraceDomain::new(&FASTPQ_FINAL_V1, 512)?.generator;
        Ok(Self {
            ledger: super::compact_hash_quotient::CompactHashQuotient::new(&FASTPQ_FINAL_V1, 512)?,
            public_limbs: core::array::from_fn(|limb| {
                u64::from(u32::from_le_bytes(
                    public_digest[4 * limb..4 * limb + 4]
                        .try_into()
                        .expect("fixed u32 public digest limb"),
                ))
            }),
            public_digest,
            export_point: super::field_pow(generator, 407),
        })
    }

    fn finish_residues(
        &self,
        point: u64,
        digest: &[u64; 8],
        hash: super::compact_hash_quotient::HashNumerators<u64>,
    ) -> Result<Vec<u64>> {
        canonical_base(point, "compact_public_digest_point", &[])?;
        let mask = if point == self.export_point {
            1
        } else {
            let numerator = super::mul_mod(
                super::sub_mod(super::field_pow(point, 512), 1),
                self.export_point,
            );
            let denominator = super::mul_mod(512, super::sub_mod(point, self.export_point));
            super::mul_mod(numerator, super::field_inverse(denominator))
        };
        let mut residues = Vec::with_capacity(688);
        residues.extend(hash.local);
        residues.extend(hash.transitions);
        residues.extend(
            digest
                .iter()
                .zip(self.public_limbs)
                .map(|(&value, expected)| super::mul_mod(mask, super::sub_mod(value, expected))),
        );
        Ok(residues)
    }
}

#[cfg(test)]
impl FixedAir for HashDigestAir {
    fn schema(&self) -> FixedAirSchema {
        FixedAirSchema {
            trace_rows: 512,
            width: 310,
            constraints: 688,
            identity: "compact-blake2b256:physical512:columns310:hash680:public-marked-digest8:v1",
        }
    }

    fn statement_bytes(&self) -> &[u8] {
        &self.public_digest
    }

    fn evaluate(&self, point: u64, current: &[u64], next: &[u64]) -> Result<Vec<u64>> {
        let current = crate::gadgets::compact_trace_columns::decode_hash_row(current)?;
        let next = crate::gadgets::compact_trace_columns::decode_hash_row(next)?;
        self.finish_residues(
            point,
            &current.digest,
            self.ledger.evaluate(point, &current, &next)?,
        )
    }

    fn prepare_prover(&self) -> Result<Box<dyn PreparedAir + '_>> {
        Ok(Box::new(PreparedHashDigest {
            relation: self,
            cycle: self.ledger.prepare_prover_masks()?,
        }))
    }
}

#[cfg(test)]
struct PreparedHashDigest<'a> {
    relation: &'a HashDigestAir,
    cycle: super::compact_hash_quotient::ProverMaskCycle<'a>,
}

#[cfg(test)]
impl PreparedAir for PreparedHashDigest<'_> {
    fn evaluator(&self) -> ProverEvaluator<'_> {
        let mut scratch = self.relation.ledger.evaluation_scratch::<u64>();
        Box::new(move |index, point, current, next| {
            let current = crate::gadgets::compact_trace_columns::decode_hash_row(current)?;
            let next = crate::gadgets::compact_trace_columns::decode_hash_row(next)?;
            let hash = self
                .cycle
                .evaluate_with_scratch(index, &current, &next, &mut scratch)?;
            self.relation.finish_residues(point, &current.digest, hash)
        })
    }
}

#[cfg(test)]
mod tests {
    // Isolate the derived replay byte ceiling from the independently retained
    // raw-replay default query ceiling. This is an explicit test policy.
    fn byte_limit_policy() -> crate::VerifyLimits {
        crate::VerifyLimits {
            max_queries: 375,
            ..crate::VerifyLimits::default()
        }
    }

    use super::*;
    use crate::gadgets::{
        compact_blake2b_air::{CompactHashWitness, CompactRow},
        compact_trace_columns::hash_row_cells,
    };

    use super::test_fixture::{FixedColumnsAir, false_fixture, fixture};
    #[test]
    fn raw_replay_default_query_ceiling_does_not_admit_final_geometry() {
        let policy = crate::VerifyLimits::default();
        assert_eq!(policy.max_queries, 136);
        assert_eq!(
            policy.max_proof_bytes,
            fastpq_isi::resource_limits::FASTPQ_DEFAULT_MAX_PROOF_PAYLOAD_BYTES_V1
        );
        let mut work = VerificationWork::default();
        assert!(matches!(
            verify_recorded(
                &FixedColumnsAir::new(7),
                &fixture().compact,
                policy,
                &mut work
            ),
            Err(Error::VerifierLimitExceeded {
                limit: "max_queries",
                actual: 375,
                max: 136
            })
        ));
        assert_eq!(work, VerificationWork::default());
    }

    fn diagnostic_limits() -> VerifyLimits {
        test_fixture::limits()
    }

    #[test]
    fn canonical_wire_sizes_match_the_exact_final_opening_shape() {
        struct ShapeOnly(FixedAirSchema);
        impl FixedAir for ShapeOnly {
            fn schema(&self) -> FixedAirSchema {
                self.0
            }
            fn statement_bytes(&self) -> &[u8] {
                b"wire-size-only"
            }
            fn evaluate(&self, _: u64, _: &[u64], _: &[u64]) -> Result<Vec<u64>> {
                panic!("shape accounting must not evaluate any AIR")
            }
        }
        let digest = WireDigest::new([0; 6]).unwrap();
        // Fp4 payloads are the canonical 32-byte carrier, without struct framing.
        for (rows, width, constraints) in [(65_536, 342, 923)] {
            let air = ShapeOnly(FixedAirSchema {
                trace_rows: rows,
                width,
                constraints,
                identity: "test-only-shape-accounting",
            });
            let geometry = Geometry::new(&air).unwrap();
            let depth = geometry.lde_rows.ilog2() as usize;
            let proof = CompactProof {
                row_root: digest,
                mixed_root: digest,
                quotient_root: digest,
                fri_roots: vec![digest; geometry.fri_lengths.len()],
                queries: (0..profile::QUERY_COUNT as u32)
                    .map(|index| CompactQuery {
                        index,
                        current: vec![0; width],
                        next: vec![0; width],
                        current_path: vec![digest; depth],
                        next_path: vec![digest; depth],
                        mixed: GoldilocksFp4V1::ZERO,
                        mixed_path: vec![digest; depth],
                        quotient: GoldilocksFp4V1::ZERO,
                        quotient_path: vec![digest; depth],
                        fri: FriQueryOpening {
                            initial_index: index,
                            rounds: geometry.fri_lengths[..geometry.fri_lengths.len() - 1]
                                .iter()
                                .enumerate()
                                .map(|(round, &length)| crate::proof::FriRoundOpening {
                                    round: round as u32,
                                    index: 0,
                                    values: vec![GoldilocksFp4V1::ZERO; 2],
                                    folded_value: GoldilocksFp4V1::ZERO,
                                    merkle_path: vec![digest; (length / 2).ilog2() as usize],
                                })
                                .collect(),
                            final_index: 0,
                            final_values: vec![
                                GoldilocksFp4V1::ZERO;
                                *geometry.fri_lengths.last().unwrap()
                            ],
                            final_merkle_path: vec![digest; 1],
                        },
                    })
                    .collect(),
            };
            let limits = VerifyLimits {
                max_proof_bytes: 16 * 1024 * 1024,
                max_queries: 375,
                ..byte_limit_policy()
            };
            let _flags =
                norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
            let counted = preflight(&air, &proof, limits, &geometry).unwrap();
            assert_eq!(counted, norito::core::to_bytes(&proof).unwrap().len());
            assert_eq!(
                counted,
                expanded_wire_bytes(profile::QUERY_COUNT, width, depth, &geometry.fri_lengths)
            );
            assert_eq!(counted, 7_791_716);
            assert_eq!(repeated_wire_bytes(&geometry).unwrap(), counted);
            assert!(matches!(
                preflight(&air, &proof, byte_limit_policy(), &geometry),
                Err(Error::VerifierLimitExceeded {
                    limit: "max_proof_bytes",
                    ..
                })
            ));
        }
    }

    #[test]
    fn canonical_wire_arithmetic_checks_prefixes_and_overflow() {
        assert_eq!(wire_field(0).unwrap(), 1);
        assert_eq!(wire_field(127).unwrap(), 128);
        assert_eq!(wire_field(128).unwrap(), 130);
        assert_eq!(wire_vector(0, 32).unwrap(), 8);
        assert_eq!(wire_struct(&[4, 32, 32]).unwrap(), 71);
        assert!(wire_field(usize::MAX).is_err());
        assert!(wire_vector(usize::MAX, 32).is_err());
        assert!(wire_struct(&[usize::MAX]).is_err());
    }

    #[test]
    fn prover_row_jobs_are_bounded_and_preserve_rows_and_first_error() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let jobs = AtomicUsize::new(0);
        let rows = collect_prover_rows(1_003, |indices| {
            jobs.fetch_add(1, Ordering::Relaxed);
            Ok(indices.map(|index| index * 3).collect::<Vec<_>>())
        })
        .unwrap();
        assert_eq!(jobs.load(Ordering::Relaxed), MAX_PROVER_JOBS);
        assert_eq!(rows, (0..1_003).map(|index| index * 3).collect::<Vec<_>>());
        assert!(matches!(
            collect_prover_rows::<usize>(1_003, |indices| {
                let start = indices.start;
                Err(Error::QueryIndexOutOfRange {
                    index: start,
                    len: 0,
                })
            }),
            Err(Error::QueryIndexOutOfRange { index: 0, len: 0 })
        ));
        assert!(collect_prover_rows::<usize>(0, |_| panic!("empty ranges never run")).is_err());
        assert!(collect_prover_rows::<usize>(1, |_| Ok(Vec::new())).is_err());
        // An excess row in one job cannot compensate for a missing row in the
        // next job and silently shift every subsequent oracle coordinate.
        assert!(
            collect_prover_rows::<usize>(2, |indices| {
                Ok(if indices.start == 0 {
                    vec![0, 1]
                } else {
                    Vec::new()
                })
            })
            .is_err()
        );
    }

    fn expanded_wire_bytes(queries: usize, width: usize, depth: usize, layers: &[usize]) -> usize {
        // Canonical flags0x02: u64 Vec counts and canonical unsigned LEB field
        // lengths. This formula reproduces both retired 136-query size controls
        // (1,737,603 and 2,826,491) and independently fixes final shape7,791,716.
        let field =
            |body: usize| body + ((usize::BITS - body.leading_zeros()).max(1) as usize).div_ceil(7);
        let vector = |count: usize, body: usize| 8 + count * field(body);
        let round = |length: usize| {
            2 * field(4)
                + field(vector(2, 32))
                + field(32)
                + field(vector((length / 2).ilog2() as usize, 48))
        };
        let rounds = 8 + layers[..layers.len() - 1]
            .iter()
            .map(|&length| field(round(length)))
            .sum::<usize>();
        let fri = field(4)
            + field(rounds)
            + field(4)
            + field(vector(*layers.last().unwrap(), 32))
            + field(vector(1, 48));
        let query = field(4)
            + 2 * field(vector(width, 8))
            + 4 * field(vector(depth, 48))
            + 2 * field(32)
            + field(fri);
        norito::core::Header::SIZE
            + 3 * field(48)
            + field(vector(layers.len(), 48))
            + field(vector(queries, query))
    }

    fn mutate_digest(value: WireDigest) -> WireDigest {
        let mut words = value.words();
        words[0] = super::super::add_mod(words[0], 1);
        WireDigest::new(words).unwrap()
    }

    fn rejected_before_hashing(proof: &CompactProof, limits: VerifyLimits) {
        let relation = FixedColumnsAir {
            public: fixture().digest,
        };
        let mut work = VerificationWork::default();
        assert!(verify_recorded(&relation, proof, limits, &mut work).is_err());
        assert_eq!(work, VerificationWork::default());
    }

    #[test]
    fn full_geometry_columns_proof_verifies_without_private_trace_replay() {
        struct VerifyOnly(FixedColumnsAir);
        impl FixedAir for VerifyOnly {
            fn schema(&self) -> FixedAirSchema {
                self.0.schema()
            }
            fn statement_bytes(&self) -> &[u8] {
                self.0.statement_bytes()
            }
            fn evaluate(&self, point: u64, current: &[u64], next: &[u64]) -> Result<Vec<u64>> {
                self.0.evaluate(point, current, next)
            }
            fn prepare_prover(&self) -> Result<Box<dyn PreparedAir + '_>> {
                panic!("bounded verifier must never prepare or replay a private trace")
            }
        }
        let fixture = fixture();
        let relation = VerifyOnly(FixedColumnsAir {
            public: fixture.digest,
        });
        let started = std::time::Instant::now();
        let work = verify(&relation, &fixture.compact, diagnostic_limits()).unwrap();
        assert_eq!(work.transcripts, 1);
        assert_eq!(work.air_evaluations, 375);
        assert!(work.row_leaves <= 750);
        assert_eq!(work.fri_queries, 375);
        assert!(work.proof_bytes > byte_limit_policy().max_proof_bytes);
        assert_eq!(fixture.compact.queries.len(), 375);
        assert!(
            fixture
                .compact
                .queries
                .iter()
                .all(|query| query.current.len() == 342 && query.next.len() == 342)
        );
        eprintln!(
            "compact_single_hash_verify={:?}; work={work:?}; canonical_queries=375; replay_byte_budget_admitted=false; profile_security_qualified=false",
            started.elapsed()
        );
        rejected_before_hashing(&fixture.compact, byte_limit_policy());
    }

    #[test]
    fn shared_full_geometry_proof_verifies_without_replay_and_false_claim_fails_degree() {
        use shared_openings::{from_compact, verify_shared};
        struct VerifyOnly(FixedColumnsAir);
        impl FixedAir for VerifyOnly {
            fn schema(&self) -> FixedAirSchema {
                self.0.schema()
            }
            fn statement_bytes(&self) -> &[u8] {
                self.0.statement_bytes()
            }
            fn evaluate(&self, point: u64, current: &[u64], next: &[u64]) -> Result<Vec<u64>> {
                self.0.evaluate(point, current, next)
            }
            fn prepare_prover(&self) -> Result<Box<dyn PreparedAir + '_>> {
                panic!("shared verification must not prepare a private trace")
            }
        }
        let fixture = fixture();
        let air = VerifyOnly(FixedColumnsAir {
            public: fixture.digest,
        });
        let shared = from_compact(&air, &fixture.compact, diagnostic_limits()).unwrap();
        let started = std::time::Instant::now();
        let work = verify_shared(&air, &shared, diagnostic_limits()).unwrap();
        let encoded = norito::core::to_bytes(&shared).unwrap();
        assert_eq!(
            shared_openings::codec::decode_and_verify(&air, &encoded, diagnostic_limits()).unwrap(),
            work,
        );
        assert_eq!(work.transcripts, 1);
        assert_eq!(work.air_evaluations, 375);
        assert!(work.row_leaves <= 750);
        assert_eq!(work.oracle_leaves, 750);
        assert_eq!(work.terminal_degree_checks, 1);
        assert!(work.proof_bytes < norito::encode_canonical(&fixture.compact).unwrap().len());
        assert!(matches!(
            verify_shared(&air, &shared, byte_limit_policy()),
            Err(Error::VerifierLimitExceeded {
                limit: "max_proof_bytes",
                ..
            })
        ));
        eprintln!(
            "compact_shared_hash_verify={:?}; work={work:?}; production_security_qualified=false",
            started.elapsed()
        );
        let false_air = VerifyOnly(FixedColumnsAir {
            public: false_fixture().digest,
        });
        assert!(verify_shared(&false_air, &shared, diagnostic_limits()).is_err());
        let false_shared =
            from_compact(&false_air, &false_fixture().compact, diagnostic_limits()).unwrap();
        assert!(matches!(
            verify_shared(&false_air, &false_shared, diagnostic_limits()),
            Err(Error::FriTerminalDegreeMismatch { .. })
        ));
        assert!(matches!(
            shared_openings::codec::decode_and_verify(
                &false_air,
                &norito::core::to_bytes(&false_shared).unwrap(),
                diagnostic_limits()
            ),
            Err(Error::FriTerminalDegreeMismatch { .. })
        ));
    }

    #[test]
    fn parallel_prover_preserves_complete_proof_bytes_across_worker_counts() {
        let air = FixedColumnsAir::new(7);
        let columns = air.columns();
        let mut previous = None;
        for workers in [1, 4] {
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(workers)
                .build()
                .unwrap();
            let proof = pool.install(|| prove(&air, &columns)).unwrap();
            let shared = shared_openings::from_compact(&air, &proof, diagnostic_limits()).unwrap();
            let encoded = norito::encode_canonical(&shared).unwrap();
            if let Some(expected) = &previous {
                assert_eq!(&encoded, expected, "workers={workers}");
            }
            previous = Some(encoded);
        }
    }

    #[test]
    fn a_fresh_proof_for_a_false_constant_claim_fails_the_fixture_air_relation() {
        let fixture = fixture();
        let valid_relation = FixedColumnsAir {
            public: fixture.digest,
        };
        let false_relation = FixedColumnsAir {
            public: false_fixture().digest,
        };
        assert!(verify(&false_relation, &fixture.compact, diagnostic_limits()).is_err());
        assert!(
            verify(
                &valid_relation,
                &false_fixture().compact,
                diagnostic_limits()
            )
            .is_err()
        );
        let mut work = VerificationWork::default();
        let result = verify_recorded(
            &false_relation,
            &false_fixture().compact,
            diagnostic_limits(),
            &mut work,
        );
        // Roots/challenges/opened quotient are internally consistent for this
        // false statement. Its high-degree quotient must fail the actual FRI
        // terminal bound, rather than merely a transcript mismatch.
        assert!(
            matches!(result, Err(Error::FriTerminalDegreeMismatch { .. })),
            "{result:?}"
        );
        assert_eq!(work.air_evaluations, 375);
    }

    #[test]
    fn roots_columns_paths_oracle_coordinates_and_fri_values_are_bound() {
        let fixture = fixture();
        let relation = FixedColumnsAir {
            public: fixture.digest,
        };
        for root in 0..4 {
            let mut proof = fixture.compact.clone();
            let target = match root {
                0 => &mut proof.row_root,
                1 => &mut proof.mixed_root,
                2 => &mut proof.quotient_root,
                _ => &mut proof.fri_roots[0],
            };
            *target = mutate_digest(*target);
            assert!(
                verify(&relation, &proof, diagnostic_limits()).is_err(),
                "root={root}"
            );
        }
        for side in 0..2 {
            for column in [
                0, 31, 32, 63, 64, 79, 80, 271, 272, 275, 276, 299, 300, 301, 302, 309, 310, 341,
            ] {
                let mut proof = fixture.compact.clone();
                let row = if side == 0 {
                    &mut proof.queries[0].current
                } else {
                    &mut proof.queries[0].next
                };
                row[column] = super::super::add_mod(row[column], 1);
                assert!(
                    verify(&relation, &proof, diagnostic_limits()).is_err(),
                    "side={side}, column={column}"
                );
            }
        }
        for role in 0..4 {
            let mut proof = fixture.compact.clone();
            let query = &mut proof.queries[0];
            let path = match role {
                0 => &mut query.current_path,
                1 => &mut query.next_path,
                2 => &mut query.mixed_path,
                _ => &mut query.quotient_path,
            };
            path[0] = mutate_digest(path[0]);
            assert!(
                verify(&relation, &proof, diagnostic_limits()).is_err(),
                "path role={role}"
            );
        }
        for oracle in 0..4 {
            for lane in 0..4 {
                let mut proof = fixture.compact.clone();
                let query = &mut proof.queries[0];
                let value = match oracle {
                    0 => &mut query.mixed,
                    1 => &mut query.quotient,
                    2 => &mut query.fri.rounds[0].values[0],
                    _ => &mut query.fri.final_values[0],
                };
                let mut coefficients = value.coefficients();
                coefficients[lane] = super::super::add_mod(coefficients[lane], 1);
                *value = GoldilocksFp4V1::new(coefficients).unwrap();
                assert!(
                    verify(&relation, &proof, diagnostic_limits()).is_err(),
                    "oracle={oracle}, lane={lane}"
                );
            }
        }
        let mut proof = fixture.compact.clone();
        proof.queries.swap(0, 1);
        assert!(verify(&relation, &proof, diagnostic_limits()).is_err());
        let mut proof = fixture.compact.clone();
        proof.queries[0].fri.rounds[0].index ^= 1 << 20;
        assert!(verify(&relation, &proof, diagnostic_limits()).is_err());
    }

    #[test]
    fn resource_shapes_and_all_fp4_coefficients_are_rejected_before_hashing() {
        let base = &fixture().compact;
        for malformed in 0..8 {
            let mut proof = base.clone();
            match malformed {
                0 => {
                    proof.queries.pop();
                }
                1 => proof.queries.push(proof.queries[0].clone()),
                2 => {
                    proof.fri_roots.pop();
                }
                3 => proof.queries[0].current.push(0),
                4 => {
                    proof.queries[0].next.pop();
                }
                5 => proof.queries[0].mixed_path.push(WireDigest::default()),
                6 => proof.queries[0].fri.rounds[0]
                    .values
                    .push(GoldilocksFp4V1::ZERO),
                _ => {
                    proof.queries[0].fri.final_values.pop();
                }
            }
            rejected_before_hashing(&proof, diagnostic_limits());
        }
        for column in [0, 31, 80, 272, 300, 309, 310, 341] {
            let mut proof = base.clone();
            proof.queries[0].current[column] = GOLDILOCKS_MODULUS;
            rejected_before_hashing(&proof, diagnostic_limits());
        }
        for lane in 0..4 {
            let mut coefficients = [0; 4];
            coefficients[lane] = GOLDILOCKS_MODULUS;
            let bad = GoldilocksFp4V1::from_coefficients_unchecked_for_test(coefficients);
            for target in 0..5 {
                let mut proof = base.clone();
                let query = &mut proof.queries[0];
                match target {
                    0 => query.mixed = bad,
                    1 => query.quotient = bad,
                    2 => query.fri.rounds[0].values[0] = bad,
                    3 => query.fri.rounds[0].folded_value = bad,
                    _ => query.fri.final_values[0] = bad,
                }
                rejected_before_hashing(&proof, diagnostic_limits());
            }
        }
        for limits in [
            VerifyLimits {
                max_queries: 374,
                ..diagnostic_limits()
            },
            VerifyLimits {
                max_air_row_values: 341,
                ..diagnostic_limits()
            },
            VerifyLimits {
                max_query_path_len: 18,
                ..diagnostic_limits()
            },
            VerifyLimits {
                max_fri_layers: 17,
                ..diagnostic_limits()
            },
            VerifyLimits {
                max_fri_round_values: 3,
                ..diagnostic_limits()
            },
            VerifyLimits {
                max_batch_bytes: 31,
                ..diagnostic_limits()
            },
        ] {
            rejected_before_hashing(base, limits);
        }
    }

    #[test]
    fn fixed_public_digest_constraints_cover_every_bit_at_exact_export_position() {
        let bytes = [71; 83];
        let witness = CompactHashWitness::from_bytes(&bytes).unwrap();
        let digest = *iroha_crypto::Hash::new(bytes).as_ref();
        let valid = HashDigestAir::new(digest).unwrap();
        let current = hash_row_cells(&witness.rows()[407]);
        let next = hash_row_cells(&CompactRow::zero());
        assert!(
            valid
                .evaluate(valid.export_point, &current, &next)
                .unwrap()
                .iter()
                .all(|&value| value == 0)
        );
        for bit in 0..256 {
            let mut changed = digest;
            changed[bit / 8] ^= 1 << (bit % 8);
            let relation = HashDigestAir::new(changed).unwrap();
            let residues = relation
                .evaluate(relation.export_point, &current, &next)
                .unwrap();
            assert_ne!(residues[680 + bit / 32], 0, "public bit={bit}");
            assert_eq!(
                residues[680..].iter().filter(|&&value| value != 0).count(),
                1
            );
        }
    }

    #[test]
    fn schema_statement_and_root_order_bind_independent_extension_challenges() {
        struct StatementOnly([u8; 32]);
        impl FixedAir for StatementOnly {
            fn schema(&self) -> FixedAirSchema {
                FixedAirSchema {
                    trace_rows: 65_536,
                    width: 342,
                    constraints: 923,
                    identity: "complete-final-statement-challenge-test:v1",
                }
            }
            fn statement_bytes(&self) -> &[u8] {
                &self.0
            }
            fn evaluate(&self, _: u64, _: &[u64], _: &[u64]) -> Result<Vec<u64>> {
                panic!("challenge binding does not evaluate AIR")
            }
        }
        let relation = StatementOnly([17; 32]);
        let root = Digest::new([1, 2, 3, 4, 5, 6]).unwrap();
        let first = |relation: &StatementOnly, root: Digest, mixed: Digest| {
            let geometry = Geometry::new(relation).unwrap();
            let binding = Binding::new(relation, &geometry).unwrap();
            let mut transcript = binding.transcript(relation, &geometry, root).unwrap();
            let mix = transcript.columns().unwrap();
            let alpha = transcript.alphas(mixed).unwrap();
            (mix, alpha)
        };
        let baseline = first(&relation, root, root);
        assert_eq!(baseline, first(&relation, root, root));
        assert!(baseline.0.iter().any(|value| {
            value.coefficients()[1..]
                .iter()
                .any(|&coefficient| coefficient != 0)
        }));
        let changed_root = Digest::new([2, 2, 3, 4, 5, 6]).unwrap();
        assert_ne!(baseline.0, first(&relation, changed_root, root).0);
        let later = first(&relation, root, changed_root);
        assert_eq!(baseline.0, later.0);
        assert_ne!(baseline.1, later.1);
        assert_ne!(baseline.0, first(&StatementOnly([18; 32]), root, root).0);
        for (index, coefficient) in baseline.1.iter().enumerate().take(16) {
            assert!(!baseline.1[..index].contains(coefficient));
        }
        let one = GoldilocksFp4V1::new([1, 2, 3, 4]).unwrap();
        assert_eq!(combine(&[7], &[one]).unwrap(), one.mul_base(7));
        assert!(combine(&[7], &[]).is_err());
        assert!(combine(&[GOLDILOCKS_MODULUS], &[one]).is_err());
    }

    #[test]
    fn canonical_norito_frame_and_verification_ignore_ambient_layout_flags() {
        let fixture = fixture();
        let relation = FixedColumnsAir {
            public: fixture.digest,
        };
        let canonical = || {
            let _flags =
                norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
            norito::core::to_bytes(&fixture.compact).unwrap()
        };
        let expected = canonical();
        let decoded = norito::core::from_bytes::<CompactProof>(&expected).unwrap();
        let decoded = CompactProof::try_deserialize(decoded).unwrap();
        assert_eq!(decoded, fixture.compact);
        for flags in [
            0,
            norito::core::header_flags::PACKED_SEQ,
            norito::core::header_flags::PACKED_STRUCT | norito::core::header_flags::COMPACT_LEN,
        ] {
            let _ambient = norito::core::DecodeFlagsGuard::enter(flags);
            let before = norito::core::to_bytes(&vec![1_u64, 2, 3]).unwrap();
            let work = verify(&relation, &decoded, diagnostic_limits()).unwrap();
            assert_eq!(
                work.proof_bytes,
                norito::encode_canonical(&fixture.shared).unwrap().len()
            );
            assert_eq!(canonical(), expected);
            assert_eq!(before, norito::core::to_bytes(&vec![1_u64, 2, 3]).unwrap());
        }
    }

    #[test]
    fn generic_engine_default_prover_and_exact_schema_errors_are_exercised() {
        let air = FixedColumnsAir::new(7);
        let proof = &fixture().compact;
        let work = verify(&air, proof, diagnostic_limits()).unwrap();
        assert_eq!(work.air_evaluations, 375);
        assert_eq!(work.fri_queries, 375);
        assert!(prove(&air, &[]).is_err());
        assert!(prove(&air, &vec![vec![0; 3]; 342]).is_err());
        let mut columns = air.columns();
        columns[341][65_535] = GOLDILOCKS_MODULUS;
        assert!(prove(&air, &columns).is_err());
        drop(columns);
        let geometry = Geometry::new(&air).unwrap();
        assert_eq!(next_index(geometry.lde_rows - 1, geometry.lde_rows), 7);
        // A terminal tree is the sole one-leaf final tree. Every constructor and
        // parent check below uses its exact canonical context and role.
        let binding = Binding::new(&air, &geometry).unwrap();
        let role = MerkleTreeRoleV1::Fri(17);
        let leaf = binding.fri(17, 0, &[GoldilocksFp4V1::ZERO; 4]).unwrap();
        let sole = binding.tree(&[leaf], role).unwrap();
        assert_eq!(sole.path(0).unwrap(), vec![WireDigest::from(leaf)]);
        assert!(sole.path(1).is_err());
        assert!(sole.path(2).is_err());
        assert!(binding.tree(&[], role).is_err());
        assert!(binding.tree(&[leaf, leaf], role).is_err());
        let plan = crate::backend::merkle_multiproof::MultiproofPlan::new(
            1,
            &[0],
            crate::backend::merkle_multiproof::MultiproofLimits {
                max_depth: 1,
                max_queried_leaves: 1,
                max_siblings: 1,
                max_parent_hashes: 1,
            },
        )
        .unwrap();
        binding
            .verify_tree(&plan, role, sole.root(), &[leaf], &[leaf])
            .unwrap();
        assert!(
            binding
                .verify_tree(&plan, MerkleTreeRoleV1::Lde, sole.root(), &[leaf], &[leaf])
                .is_err()
        );
    }
}
