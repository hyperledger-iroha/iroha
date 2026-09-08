//! Private compact AIR engine for bounded candidate verification.
//!
//! The test prover commits complete base-field LDE rows before full-Fp4 column
//! mixing, commits the mixed oracle before independent Fp4 constraint alphas,
//! then commits the quotient before joint trace/quotient FRI challenges. The
//! verifier checks only bounded authenticated openings and caller-fixed AIR and
//! public polynomials. It has no witness, trace construction, FFT or LDE replay.
//!
//! TODO: Integrate the complete SMT/public statement relation, independently
//! qualify proximity/Fiat-Shamir/query security, and measure the final schema and
//! resource envelope before any production switch. The canonical 136-query
//! profile remains unqualified. Production verification still requires replay;
//! test-only diagnostic byte budgets are not production admission limits. This
//! proof is not a zero-knowledge claim.

use fastpq_isi::{FASTPQ_FINAL_V1, GoldilocksDigest384V1 as Digest};
use iroha_data_model::privacy::GoldilocksDigest384V1 as WireDigest;
#[cfg(test)]
use norito::DeserializePayload;
use norito::{NoritoDeserialize, NoritoSerialize};
#[cfg(test)]
use rayon::prelude::*;

use super::{
    AirQuotientDomain, FriDomain, GOLDILOCKS_MODULUS, GoldilocksFp4V1, JointFriBatch,
    MerkleTreeRoleV1, fixed_domain::FixedTraceDomain,
};
#[cfg(test)]
use super::{
    ExecutionMode, MerkleNodeCache, Transcript, build_merkle_levels_with_mode,
    hash_air_composition_leaf, hash_air_trace_row, hash_air_trace_rows_with_mode,
    hash_fp4_single_leaves_with_role, hash_lde_chunk_fp4, sample_queries,
};
use crate::{
    Error, Result,
    proof::{VerifyLimits, compact_fri_support},
};
#[cfg(test)]
use crate::{
    fft::Planner,
    proof::{FriQueryOpening, PublicIO},
};

#[path = "compact_protocol/shared_openings.rs"]
pub(super) mod shared_openings;

#[path = "compact_protocol/profile.rs"]
mod profile;
#[cfg(test)]
use profile::ProtocolTranscript;
use profile::{Binding, Protocol};

#[cfg(test)]
const PROTOCOL_TAG: &str = "fastpq:prototype:compact-single-phase:v1";
const MAX_CONSTRAINTS: usize = 1024;

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
#[cfg(test)]
pub(super) type ProverEvaluator<'a> =
    Box<dyn FnMut(usize, u64, &[u64], &[u64]) -> Result<Vec<u64>> + Send + 'a>;

/// Immutable prover preparation shared across jobs; each evaluator owns its scratch.
#[cfg(test)]
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
    #[cfg(test)]
    fn prepare_prover(&self) -> Result<Box<dyn PreparedAir + '_>>
    where
        Self: Sized,
    {
        Ok(Box::new(DirectPrepared { relation: self }))
    }
}

#[cfg(test)]
struct DirectPrepared<'a, R: FixedAir + ?Sized> {
    relation: &'a R,
}

#[cfg(test)]
impl<R: FixedAir + ?Sized> PreparedAir for DirectPrepared<'_, R> {
    fn evaluator(&self) -> ProverEvaluator<'_> {
        Box::new(move |_, point, current, next| self.relation.evaluate(point, current, next))
    }
}

/// Private typed proof; its Norito schema is distinct from production ProofV1.
#[cfg(test)]
#[derive(Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
#[norito(schema_name = "fastpq_prover::compact_prototype::SinglePhaseProofV1")]
pub(super) struct CompactProof {
    row_root: WireDigest,
    mixed_root: WireDigest,
    quotient_root: WireDigest,
    fri_roots: Vec<WireDigest>,
    queries: Vec<CompactQuery>,
}

#[cfg(test)]
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
    protocol: Protocol,
    schema: FixedAirSchema,
    lde_rows: usize,
    domain: FriDomain,
    fri_lengths: Vec<usize>,
    terminal_degree: usize,
}

impl Geometry {
    #[cfg(test)]
    fn new(relation: &impl FixedAir) -> Result<Self> {
        Self::for_protocol(relation, Protocol::Prototype)
    }

    fn for_protocol(relation: &impl FixedAir, protocol: Protocol) -> Result<Self> {
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
            protocol,
            schema,
            lde_rows,
            domain,
            fri_lengths,
            terminal_degree,
        };
        protocol.check_geometry(&geometry)?;
        Ok(geometry)
    }
}

/// Reusable committed prover data. It is never constructed by verification.
#[cfg(test)]
struct PreparedTrace {
    geometry: Geometry,
    columns: Vec<Vec<u64>>,
    rows: CommittedTree,
    binding: Binding,
    bound_statement: Option<Vec<u8>>,
}

#[cfg(test)]
struct CommittedTree {
    levels: Vec<Vec<Digest>>,
    leaf_count: usize,
}

#[cfg(test)]
impl CommittedTree {
    fn from_leaves(leaves: &[Digest], role: MerkleTreeRoleV1) -> Result<Self> {
        if leaves.is_empty() || !leaves.len().is_power_of_two() {
            return Err(shape(
                "compact prototype requires nonempty power-of-two trees",
            ));
        }
        Ok(Self {
            levels: build_merkle_levels_with_mode(leaves, role, ExecutionMode::Cpu)?,
            leaf_count: leaves.len(),
        })
    }

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

#[cfg(test)]
fn prepare_trace(relation: &impl FixedAir, columns: &[Vec<u64>]) -> Result<PreparedTrace> {
    prepare_trace_for(relation, columns, Protocol::Prototype)
}

#[cfg(test)]
fn prepare_trace_for(
    relation: &impl FixedAir,
    columns: &[Vec<u64>],
    protocol: Protocol,
) -> Result<PreparedTrace> {
    let geometry = Geometry::for_protocol(relation, protocol)?;
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
    let bound_statement =
        (protocol == Protocol::ShakeCandidate).then(|| relation.statement_bytes().to_vec());
    let planner = Planner::new(&FASTPQ_FINAL_V1);
    let mut coefficients = columns.to_vec();
    planner.ifft_columns(&mut coefficients);
    let columns = planner.lde_columns(&coefficients);
    drop(coefficients);
    let leaves = if protocol == Protocol::Prototype {
        hash_air_trace_rows_with_mode(&columns, ExecutionMode::Cpu)?
    } else {
        let results: Vec<Result<Digest>> = (0..geometry.lde_rows)
            .into_par_iter()
            .map_init(
                || vec![0; geometry.schema.width],
                |row, index| {
                    fill_row(&columns, index, row);
                    binding.row(index, row)
                },
            )
            .collect();
        results.into_iter().collect::<Result<_>>()?
    };
    let rows = binding.tree(&leaves, MerkleTreeRoleV1::AirTrace)?;
    Ok(PreparedTrace {
        geometry,
        columns,
        rows,
        binding,
        bound_statement,
    })
}

#[cfg(test)]
fn prove_prepared(relation: &impl FixedAir, trace: &PreparedTrace) -> Result<CompactProof> {
    let geometry = &trace.geometry;
    if relation.schema() != geometry.schema {
        return Err(shape("prepared compact trace has another fixed schema"));
    }
    if trace
        .bound_statement
        .as_deref()
        .is_some_and(|statement| statement != relation.statement_bytes())
    {
        return Err(shape(
            "prepared SHAKE row tree belongs to another public statement",
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
    let mixed_leaves = if geometry.protocol == Protocol::Prototype {
        hash_fp4_single_leaves_with_role(super::LDE_COMMITMENT_ROLE_V1, &mixed)?
    } else {
        let results: Vec<Result<Digest>> = mixed
            .par_iter()
            .enumerate()
            .map(|(i, &v)| binding.mixed(i, v))
            .collect();
        results.into_iter().collect::<Result<_>>()?
    };
    let mixed_tree = binding.tree(&mixed_leaves, MerkleTreeRoleV1::Lde)?;
    drop(mixed_leaves);
    let alphas = transcript.alphas(mixed_tree.root())?;
    let weights = AirQuotientDomain::new(&FASTPQ_FINAL_V1, geometry.lde_rows)?;
    let prepared = relation.prepare_prover()?;
    let mut current = vec![0; geometry.schema.width];
    let mut next = current.clone();
    let quotient_results: Vec<Result<GoldilocksFp4V1>> = (0..geometry.lde_rows)
        .into_par_iter()
        .with_min_len(64)
        .map_init(
            || {
                (
                    prepared.evaluator(),
                    vec![0; geometry.schema.width],
                    vec![0; geometry.schema.width],
                )
            },
            |(evaluate, current, next), index| {
                fill_row(&trace.columns, index, current);
                fill_row(&trace.columns, next_index(index, geometry.lde_rows), next);
                let residues = evaluate(index, geometry.domain.point(index), current, next)?;
                Ok(combine(&residues, &alphas)?.mul_base(weights.weights_at(index)?.all_rows))
            },
        )
        .collect();
    // Indexed collection fixes row order; select any errors in that same order.
    let quotients = quotient_results.into_iter().collect::<Result<Vec<_>>>()?;
    drop(prepared); // No evaluator remains; release all prover-only fixed LDEs.
    let quotient_leaves = if geometry.protocol == Protocol::Prototype {
        hash_fp4_single_leaves_with_role(super::AIR_COMPOSITION_COMMITMENT_ROLE_V1, &quotients)?
    } else {
        let results: Vec<Result<Digest>> = quotients
            .par_iter()
            .enumerate()
            .map(|(i, &v)| binding.quotient(i, v))
            .collect();
        results.into_iter().collect::<Result<_>>()?
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

// Hash and transcript orchestration differ by descriptor; the fold arithmetic,
// domain schedule and retained opening owner are shared by both implementations.
#[cfg(test)]
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
        let leaves = if geometry.protocol == Protocol::Prototype {
            super::hash_fri_leaves_with_mode(round, &current, 2, ExecutionMode::Cpu)?
        } else {
            let half = current.len() / 2;
            let results: Vec<Result<Digest>> = (0..half)
                .into_par_iter()
                .map(|i| binding.fri(round, i, &[current[i], current[i + half]]))
                .collect();
            results.into_iter().collect::<Result<_>>()?
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
/// The supplied limits are trusted caller policy. Production defaults remain
/// 512 KiB; a larger diagnostic envelope must be selected explicitly by tests.
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
    let geometry = Geometry::new(relation)?;
    let proof_bytes = preflight(relation, proof, limits, &geometry)?;
    work.proof_bytes = proof_bytes;
    work.transcripts += 1;
    let mut transcript = initialise_transcript(relation, &geometry, proof.row_root.as_fastpq())?;
    let mixing = challenges(&mut transcript, "compact:column-mix", geometry.schema.width);
    transcript.append_message("compact:mixed-root", &proof.mixed_root.to_le_bytes());
    let alphas = challenges(
        &mut transcript,
        "compact:constraint-alpha",
        geometry.schema.constraints,
    );
    transcript.append_message("compact:quotient-root", &proof.quotient_root.to_le_bytes());
    let joint =
        JointFriBatch::from_transcript(&FASTPQ_FINAL_V1, geometry.lde_rows, &mut transcript)?;
    let mut betas = Vec::with_capacity(proof.fri_roots.len() - 1);
    for (round, root) in proof.fri_roots.iter().enumerate() {
        if round + 1 == proof.fri_roots.len() {
            transcript.append_fri_final(root.as_fastpq());
        } else {
            transcript.append_fri_layer(round, root.as_fastpq());
            betas.push(transcript.challenge_beta(round));
        }
    }
    let indices = sample_queries(
        geometry.lde_rows,
        FASTPQ_FINAL_V1.fri.queries as usize,
        &mut transcript,
    )?;
    let weights = AirQuotientDomain::new(&FASTPQ_FINAL_V1, geometry.lde_rows)?;
    let mut cache = MerkleNodeCache::default();
    for (position, (query, &index)) in proof.queries.iter().zip(&indices).enumerate() {
        if query.index as usize != index {
            return Err(Error::QueryMismatch { index: position });
        }
        for (row_index, row, path) in [
            (index, &query.current, &query.current_path),
            (
                next_index(index, geometry.lde_rows),
                &query.next,
                &query.next_path,
            ),
        ] {
            let leaf = hash_air_trace_row(row_index, row)?;
            work.row_leaves += 1;
            authenticate(
                &mut cache,
                MerkleTreeRoleV1::AirTrace,
                proof.row_root,
                leaf,
                row_index,
                path,
                position,
            )?;
        }
        let mixed = query
            .current
            .iter()
            .zip(&mixing)
            .fold(GoldilocksFp4V1::ZERO, |sum, (&value, coefficient)| {
                sum.add(coefficient.mul_base(value))
            });
        if mixed != query.mixed {
            return Err(Error::QueryMismatch { index: position });
        }
        authenticate(
            &mut cache,
            MerkleTreeRoleV1::Lde,
            proof.mixed_root,
            hash_lde_chunk_fp4(index, &[query.mixed])?,
            index,
            &query.mixed_path,
            position,
        )?;
        let residues =
            relation.evaluate(geometry.domain.point(index), &query.current, &query.next)?;
        work.air_evaluations += 1;
        let quotient = combine(&residues, &alphas)?.mul_base(weights.weights_at(index)?.all_rows);
        if quotient != query.quotient {
            return Err(Error::AirOpeningMismatch { index: position });
        }
        authenticate(
            &mut cache,
            MerkleTreeRoleV1::AirComposition,
            proof.quotient_root,
            hash_air_composition_leaf(index, query.quotient)?,
            index,
            &query.quotient_path,
            position,
        )?;
        compact_fri_support::verify_query(
            &mut cache,
            &query.fri,
            compact_fri_support::Context {
                query_pos: position,
                initial_index: index,
                initial_value: joint.value_at(index, query.quotient, query.mixed)?,
                fri_layers: &proof.fri_roots,
                betas: &betas,
                fri_layer_lengths: &geometry.fri_lengths,
                terminal_degree_bound: geometry.terminal_degree,
                arity: FASTPQ_FINAL_V1.fri.arity,
                domain: geometry.domain,
            },
        )?;
        work.fri_queries += 1;
    }
    Ok(())
}

#[cfg(test)]
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
        || proof.queries.len() != geometry.protocol.query_count(geometry.lde_rows)
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

#[cfg(test)]
fn initialise_transcript(
    relation: &impl FixedAir,
    geometry: &Geometry,
    row_root: Digest,
) -> Result<Transcript> {
    let _flags = norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
    let schema = geometry.schema;
    // Norito's nominal Vec encoding provides the sequence schema; bare byte
    // slices do not implement NoritoSerialize. Both callers preflight the
    // complete public statement before this bounded copy or any hashing.
    let statement = relation.statement_bytes().to_vec();
    let frame = norito::core::to_bytes(&(
        PROTOCOL_TAG,
        schema.identity,
        schema.trace_rows as u32,
        geometry.lde_rows as u32,
        schema.width as u32,
        schema.constraints as u32,
        2_u32,
        statement,
    ))?;
    let mut transcript =
        Transcript::initialise(&PublicIO::default(), FASTPQ_FINAL_V1.name, 1, PROTOCOL_TAG)?;
    transcript.append_message("compact:fixed-schema-and-statement", &frame);
    transcript.append_message("compact:full-row-root", &row_root.to_le_bytes());
    Ok(transcript)
}

#[cfg(test)]
fn challenges(transcript: &mut Transcript, tag: &str, count: usize) -> Vec<GoldilocksFp4V1> {
    (0..count)
        .map(|index| transcript.challenge_extension(&format!("{tag}:{index}")))
        .collect()
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

#[cfg(test)]
fn fill_row(columns: &[Vec<u64>], index: usize, row: &mut [u64]) {
    for (value, column) in row.iter_mut().zip(columns) {
        *value = column[index];
    }
}

fn next_index(index: usize, lde_rows: usize) -> usize {
    (index + FASTPQ_FINAL_V1.fri.blowup_factor as usize) % lde_rows
}

#[cfg(test)]
fn authenticate(
    cache: &mut MerkleNodeCache,
    role: MerkleTreeRoleV1,
    root: WireDigest,
    leaf: Digest,
    index: usize,
    path: &[WireDigest],
    position: usize,
) -> Result<()> {
    let native: Vec<_> = path.iter().copied().map(WireDigest::as_fastpq).collect();
    if !cache.verify_path(role, root.as_fastpq(), leaf, index, &native)? {
        return Err(Error::QueryMerklePathMismatch { index: position });
    }
    Ok(())
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
    use super::*;
    use crate::gadgets::{
        compact_blake2b_air::{CompactHashWitness, CompactRow},
        compact_trace_columns::hash_row_cells,
    };
    use std::sync::OnceLock;

    struct Fixture {
        digest: [u8; 32],
        proof: CompactProof,
        false_digest: [u8; 32],
        false_proof: CompactProof,
    }

    fn fixture() -> &'static Fixture {
        static FIXTURE: OnceLock<Fixture> = OnceLock::new();
        FIXTURE.get_or_init(|| {
            let started = std::time::Instant::now();
            let bytes: Vec<_> = (0..83).map(|index| (index * 73 + 11) as u8).collect();
            let witness = CompactHashWitness::from_bytes(&bytes).unwrap();
            let digest = *iroha_crypto::Hash::new(&bytes).as_ref();
            let relation = HashDigestAir::new(digest).unwrap();
            let mut columns = vec![vec![0; 512]; 310];
            for (row, values) in witness.rows().iter().enumerate() {
                for (column, value) in columns.iter_mut().zip(hash_row_cells(values)) {
                    column[row] = value;
                }
            }
            let trace = prepare_trace(&relation, &columns).unwrap();
            let proof = prove_prepared(&relation, &trace).unwrap();
            let mut false_digest = digest;
            false_digest[31] ^= 1; // Clear the mandatory Iroha marker bit248.
            let false_relation = HashDigestAir::new(false_digest).unwrap();
            let false_proof = prove_prepared(&false_relation, &trace).unwrap();
            assert_eq!(proof.row_root, false_proof.row_root);
            assert_ne!(proof.mixed_root, false_proof.mixed_root);
            eprintln!(
                "compact_single_hash_two_statement_proofs={:?}",
                started.elapsed()
            );
            // The fixture retains only public digests and bounded proof objects.
            // Base witness, coefficients, LDE rows and all prover trees drop here.
            Fixture {
                digest,
                proof,
                false_digest,
                false_proof,
            }
        })
    }

    fn diagnostic_limits() -> VerifyLimits {
        // Explicit private measurement envelope only. Production remains512KiB;
        // neither query count nor any production admission setting is changed.
        VerifyLimits {
            max_proof_bytes: 2 * 1024 * 1024,
            ..VerifyLimits::default()
        }
    }

    #[test]
    fn canonical_wire_sizes_match_complete_hash_and_transfer_opening_shapes() {
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
        for (rows, width, constraints, expected_bytes) in
            [(512, 310, 688, 1_737_603), (65_536, 342, 923, 2_826_491)]
        {
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
                queries: (0..FASTPQ_FINAL_V1.fri.queries)
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
                max_proof_bytes: 4 * 1024 * 1024,
                ..VerifyLimits::default()
            };
            let _flags =
                norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
            let counted = preflight(&air, &proof, limits, &geometry).unwrap();
            assert_eq!(counted, norito::core::to_bytes(&proof).unwrap().len());
            assert_eq!(counted, expected_bytes, "rows={rows}; width={width}");
            assert!(matches!(
                preflight(&air, &proof, VerifyLimits::default(), &geometry),
                Err(Error::VerifierLimitExceeded {
                    limit: "max_proof_bytes",
                    ..
                })
            ));
        }
    }

    fn mutate_digest(value: WireDigest) -> WireDigest {
        let mut words = value.words();
        words[0] = super::super::add_mod(words[0], 1);
        WireDigest::new(words).unwrap()
    }

    fn rejected_before_hashing(proof: &CompactProof, limits: VerifyLimits) {
        let relation = HashDigestAir::new(fixture().digest).unwrap();
        let mut work = VerificationWork::default();
        assert!(verify_recorded(&relation, proof, limits, &mut work).is_err());
        assert_eq!(work, VerificationWork::default());
    }

    #[test]
    fn end_to_end_public_digest_proof_verifies_without_private_trace_replay() {
        struct VerifyOnly(HashDigestAir);
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
        let relation = VerifyOnly(HashDigestAir::new(fixture.digest).unwrap());
        let started = std::time::Instant::now();
        let work = verify(&relation, &fixture.proof, diagnostic_limits()).unwrap();
        assert_eq!(work.transcripts, 1);
        assert_eq!(work.air_evaluations, 136);
        assert_eq!(work.row_leaves, 272);
        assert_eq!(work.fri_queries, 136);
        assert!(work.proof_bytes > VerifyLimits::default().max_proof_bytes);
        assert_eq!(fixture.proof.queries.len(), 136);
        assert!(
            fixture
                .proof
                .queries
                .iter()
                .all(|query| query.current.len() == 310 && query.next.len() == 310)
        );
        eprintln!(
            "compact_single_hash_verify={:?}; work={work:?}; canonical_queries=136; production_512KiB_admitted=false; profile_security_qualified=false",
            started.elapsed()
        );
        rejected_before_hashing(&fixture.proof, VerifyLimits::default());
    }

    #[test]
    fn shared_full_hash_proof_verifies_without_replay_and_false_proof_fails_degree() {
        use shared_openings::{from_compact, verify_shared};
        struct VerifyOnly(HashDigestAir);
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
        let air = VerifyOnly(HashDigestAir::new(fixture.digest).unwrap());
        let shared = from_compact(&air, &fixture.proof, diagnostic_limits()).unwrap();
        let started = std::time::Instant::now();
        let work = verify_shared(&air, &shared, diagnostic_limits()).unwrap();
        let encoded = norito::core::to_bytes(&shared).unwrap();
        assert_eq!(
            shared_openings::codec::decode_and_verify(&air, &encoded, diagnostic_limits()).unwrap(),
            work,
        );
        assert_eq!(work.transcripts, 1);
        assert_eq!(work.air_evaluations, 136);
        assert!(work.row_leaves <= 272);
        assert_eq!(work.oracle_leaves, 272);
        assert_eq!(work.terminal_degree_checks, 1);
        assert!(work.proof_bytes < 1_737_603);
        assert!(matches!(
            verify_shared(&air, &shared, VerifyLimits::default()),
            Err(Error::VerifierLimitExceeded {
                limit: "max_proof_bytes",
                ..
            })
        ));
        eprintln!(
            "compact_shared_hash_verify={:?}; work={work:?}; production_security_qualified=false",
            started.elapsed()
        );
        let false_air = VerifyOnly(HashDigestAir::new(fixture.false_digest).unwrap());
        assert!(verify_shared(&false_air, &shared, diagnostic_limits()).is_err());
        let false_shared =
            from_compact(&false_air, &fixture.false_proof, diagnostic_limits()).unwrap();
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
        let bytes: Vec<_> = (0..83).map(|index| (index * 73 + 11) as u8).collect();
        let witness = CompactHashWitness::from_bytes(&bytes).unwrap();
        let relation = HashDigestAir::new(*iroha_crypto::Hash::new(&bytes).as_ref()).unwrap();
        let mut columns = vec![vec![0; 512]; 310];
        for (row, values) in witness.rows().iter().enumerate() {
            for (column, value) in columns.iter_mut().zip(hash_row_cells(values)) {
                column[row] = value;
            }
        }
        let mut previous = None;
        for workers in [1, 4] {
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(workers)
                .build()
                .unwrap();
            let proof = pool.install(|| prove(&relation, &columns)).unwrap();
            let _flags =
                norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
            let encoded = norito::core::to_bytes(&proof).unwrap();
            if let Some(expected) = &previous {
                assert_eq!(&encoded, expected, "workers={workers}");
            }
            previous = Some(encoded);
        }
    }

    #[test]
    fn a_fresh_proof_for_a_false_public_digest_fails_the_actual_air_relation() {
        let fixture = fixture();
        let valid_relation = HashDigestAir::new(fixture.digest).unwrap();
        let false_relation = HashDigestAir::new(fixture.false_digest).unwrap();
        assert!(verify(&false_relation, &fixture.proof, diagnostic_limits()).is_err());
        assert!(verify(&valid_relation, &fixture.false_proof, diagnostic_limits()).is_err());
        let mut work = VerificationWork::default();
        let result = verify_recorded(
            &false_relation,
            &fixture.false_proof,
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
        assert_eq!(work.air_evaluations, 1);
    }

    #[test]
    fn roots_columns_paths_oracle_coordinates_and_fri_values_are_bound() {
        let fixture = fixture();
        let relation = HashDigestAir::new(fixture.digest).unwrap();
        for root in 0..4 {
            let mut proof = fixture.proof.clone();
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
                0, 31, 32, 63, 64, 79, 80, 271, 272, 275, 276, 299, 300, 301, 302, 309,
            ] {
                let mut proof = fixture.proof.clone();
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
            let mut proof = fixture.proof.clone();
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
                let mut proof = fixture.proof.clone();
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
        let mut proof = fixture.proof.clone();
        proof.queries.swap(0, 1);
        assert!(verify(&relation, &proof, diagnostic_limits()).is_err());
        let mut proof = fixture.proof.clone();
        proof.queries[0].fri.rounds[0].index ^= 1 << 20;
        assert!(verify(&relation, &proof, diagnostic_limits()).is_err());
    }

    #[test]
    fn resource_shapes_and_all_fp4_coefficients_are_rejected_before_hashing() {
        let base = &fixture().proof;
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
        for column in [0, 31, 80, 272, 300, 309] {
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
                max_queries: 135,
                ..diagnostic_limits()
            },
            VerifyLimits {
                max_air_row_values: 309,
                ..diagnostic_limits()
            },
            VerifyLimits {
                max_query_path_len: 11,
                ..diagnostic_limits()
            },
            VerifyLimits {
                max_fri_layers: 10,
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
        let relation = HashDigestAir::new([17; 32]).unwrap();
        let geometry = Geometry::new(&relation).unwrap();
        let root = Digest::new([1, 2, 3, 4, 5, 6]).unwrap();
        let first = |relation: &HashDigestAir, root: Digest, mixed: Digest| {
            let mut transcript = initialise_transcript(relation, &geometry, root).unwrap();
            let mix = challenges(&mut transcript, "compact:column-mix", 310);
            transcript.append_message("compact:mixed-root", &mixed.to_le_bytes());
            let alpha = challenges(&mut transcript, "compact:constraint-alpha", 688);
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
        assert_ne!(
            baseline.0,
            first(&HashDigestAir::new([18; 32]).unwrap(), root, root).0
        );
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
        let relation = HashDigestAir::new(fixture.digest).unwrap();
        let canonical = || {
            let _flags =
                norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
            norito::core::to_bytes(&fixture.proof).unwrap()
        };
        let expected = canonical();
        let decoded = norito::core::from_bytes::<CompactProof>(&expected).unwrap();
        let decoded = CompactProof::try_deserialize(decoded).unwrap();
        assert_eq!(decoded, fixture.proof);
        for flags in [
            0,
            norito::core::header_flags::PACKED_SEQ,
            norito::core::header_flags::PACKED_STRUCT | norito::core::header_flags::COMPACT_LEN,
        ] {
            let _ambient = norito::core::DecodeFlagsGuard::enter(flags);
            let before = norito::core::to_bytes(&vec![1_u64, 2, 3]).unwrap();
            let work = verify(&relation, &decoded, diagnostic_limits()).unwrap();
            assert_eq!(work.proof_bytes, expected.len());
            assert_eq!(canonical(), expected);
            assert_eq!(before, norito::core::to_bytes(&vec![1_u64, 2, 3]).unwrap());
        }
    }

    #[test]
    fn generic_engine_default_prover_and_exact_schema_errors_are_exercised() {
        struct ZeroAir;
        impl FixedAir for ZeroAir {
            fn schema(&self) -> FixedAirSchema {
                FixedAirSchema {
                    trace_rows: 4,
                    width: 1,
                    constraints: 1,
                    identity: "compact-prototype-zero-column:v1",
                }
            }
            fn statement_bytes(&self) -> &[u8] {
                b"the committed column is zero"
            }
            fn evaluate(&self, _: u64, current: &[u64], _: &[u64]) -> Result<Vec<u64>> {
                Ok(vec![current[0]])
            }
        }
        let proof = prove(&ZeroAir, &[vec![0; 4]]).unwrap();
        let work = verify(&ZeroAir, &proof, VerifyLimits::default()).unwrap();
        assert_eq!(work.air_evaluations, 32);
        assert_eq!(work.fri_queries, 32);
        assert!(prove(&ZeroAir, &[]).is_err());
        assert!(prove(&ZeroAir, &[vec![0; 3]]).is_err());
        assert!(prove(&ZeroAir, &[vec![GOLDILOCKS_MODULUS; 4]]).is_err());
        let geometry = Geometry::new(&ZeroAir).unwrap();
        assert_eq!(next_index(31, geometry.lde_rows), 7);
        let leaves = [Digest::new([1; 6]).unwrap(), Digest::new([2; 6]).unwrap()];
        let tree = CommittedTree::from_leaves(&leaves, MerkleTreeRoleV1::AirTrace).unwrap();
        assert!(tree.path(2).is_err());
        assert!(CommittedTree::from_leaves(&[], MerkleTreeRoleV1::AirTrace).is_err());
        let sole = CommittedTree::from_leaves(&leaves[..1], MerkleTreeRoleV1::AirTrace).unwrap();
        assert_eq!(sole.path(0).unwrap(), vec![WireDigest::from(leaves[0])]);
        assert!(sole.path(1).is_err());
        let mut cache = MerkleNodeCache::default();
        authenticate(
            &mut cache,
            MerkleTreeRoleV1::AirTrace,
            tree.root().into(),
            leaves[0],
            0,
            &tree.path(0).unwrap(),
            0,
        )
        .unwrap();
        assert!(
            authenticate(
                &mut cache,
                MerkleTreeRoleV1::Lde,
                tree.root().into(),
                leaves[0],
                0,
                &tree.path(0).unwrap(),
                0
            )
            .is_err()
        );
    }
}
