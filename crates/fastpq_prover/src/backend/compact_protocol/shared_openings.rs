//! Private shared openings for caller-fixed diagnostic compact transcripts.
//!
//! Transcript-derived indices select sorted unique complete rows and binary FRI
//! groups. Every tree has one canonical minimal sibling frontier, with roles,
//! round numbers, node levels and indices inherited from the existing hashes.
//! Complete terminal values appear once; their sole-leaf root still hashes the
//! leaf with itself. No witness, trace, FFT, path expansion or authentication
//! bypass is used by verification. One canonical V1 DTO uses the fixed complete
//! context and six-lane transcript for every commitment and verifier message.
//!
//! TODO: Qualify the selected transcript profile and complete resource envelope
//! before production admission. The candidate verifier raises no default limit;
//! sharing Merkle paths does not guarantee the 512 KiB production byte target.

#[cfg(test)]
use std::collections::BTreeMap;
use std::collections::BTreeSet;

use super::*;
use crate::backend::{
    fold_fri_coset,
    merkle_multiproof::{MultiproofLimits, MultiproofPlan},
};

#[cfg(test)]
use crate::backend::merkle_multiproof::SiblingPosition;

#[path = "shared_openings/codec.rs"]
pub(in crate::backend) mod codec;

/// Internal complete-opening payload shared by candidate verification and test codecs.
/// Canonical serialization advertises the final fixed layout and full six-lane roots.
#[derive(Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
#[norito(schema_name = "fastpq_prover::compact_v1::SharedProofV1")]
pub(in crate::backend) struct SharedProof {
    row_root: WireDigest,
    mixed_root: WireDigest,
    quotient_root: WireDigest,
    fri_roots: Vec<WireDigest>,
    rows: Vec<SharedRow>,
    queries: Vec<SharedQuery>,
    row_siblings: Vec<WireDigest>,
    mixed_siblings: Vec<WireDigest>,
    quotient_siblings: Vec<WireDigest>,
    rounds: Vec<SharedRound>,
    terminal_values: Vec<GoldilocksFp4V1>,
}

#[derive(Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct SharedRow {
    index: u32,
    values: Vec<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct SharedQuery {
    index: u32,
    mixed: GoldilocksFp4V1,
    quotient: GoldilocksFp4V1,
}

#[derive(Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct SharedRound {
    groups: Vec<SharedGroup>,
    siblings: Vec<WireDigest>,
}

#[derive(Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct SharedGroup {
    index: u32,
    values: [GoldilocksFp4V1; 2],
}

/// Actual successful shared-verifier work, with no private-domain reconstruction.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(in crate::backend) struct SharedVerificationWork {
    /// Exact canonical framed Norito wire bytes.
    pub(in crate::backend) proof_bytes: usize,
    /// Transcript initialization occurs only after full bounded canonical preflight.
    pub(in crate::backend) transcripts: usize,
    /// Distinct complete row leaf hashes.
    pub(in crate::backend) row_leaves: usize,
    /// Distinct mixed and quotient leaf hashes.
    pub(in crate::backend) oracle_leaves: usize,
    /// Distinct binary FRI group leaves, including the sole terminal leaf.
    pub(in crate::backend) fri_leaves: usize,
    /// Shared internal Merkle hashes after successful reconstruction.
    pub(in crate::backend) parent_hashes: usize,
    /// Exact relation evaluations at transcript query indices.
    pub(in crate::backend) air_evaluations: usize,
    /// Whole-terminal polynomial degree checks.
    pub(in crate::backend) terminal_degree_checks: usize,
}

struct Challenges {
    mixing: Vec<GoldilocksFp4V1>,
    alphas: Vec<GoldilocksFp4V1>,
    joint: JointFriBatch,
    betas: Vec<GoldilocksFp4V1>,
    indices: Vec<usize>,
}

// This is deliberately the parent's current sequence verbatim. Shared wire
// positions/values are not appended: they represent the same committed proof.
#[cfg(test)]
fn replay_challenges(
    relation: &impl FixedAir,
    geometry: &Geometry,
    row_root: WireDigest,
    mixed_root: WireDigest,
    quotient_root: WireDigest,
    fri_roots: &[WireDigest],
) -> Result<Challenges> {
    let binding = Binding::new(relation, geometry)?;
    replay_bound_challenges(
        relation,
        geometry,
        &binding,
        row_root,
        mixed_root,
        quotient_root,
        fri_roots,
    )
}

fn replay_bound_challenges(
    relation: &impl FixedAir,
    geometry: &Geometry,
    binding: &Binding,
    row_root: WireDigest,
    mixed_root: WireDigest,
    quotient_root: WireDigest,
    fri_roots: &[WireDigest],
) -> Result<Challenges> {
    if fri_roots.len() != geometry.fri_lengths.len() {
        return Err(shape("shared transcript requires the exact FRI root count"));
    }
    let mut transcript = binding.transcript(relation, geometry, row_root.as_fastpq())?;
    let mixing = transcript.columns()?;
    let alphas = transcript.alphas(mixed_root.as_fastpq())?;
    let joint = transcript.joint(quotient_root.as_fastpq())?;
    let mut betas = Vec::with_capacity(fri_roots.len() - 1);
    for (round, root) in fri_roots[..fri_roots.len() - 1].iter().enumerate() {
        betas.push(transcript.beta(round, root.as_fastpq())?);
    }
    let indices = transcript.queries(fri_roots.last().unwrap().as_fastpq())?;
    Ok(Challenges {
        mixing,
        alphas,
        joint,
        betas,
        indices,
    })
}

struct IndexedPlan {
    indices: Vec<usize>,
    plan: MultiproofPlan,
}

struct OpeningPlans {
    rows: IndexedPlan,
    queries: IndexedPlan,
    rounds: Vec<IndexedPlan>,
    terminal: MultiproofPlan,
}

fn plan_limits(leaves: usize, capacity: usize, limits: VerifyLimits) -> Result<MultiproofLimits> {
    let depth = (leaves.ilog2() as usize).max(1);
    let work = capacity
        .checked_mul(depth)
        .ok_or_else(|| shape("shared frontier bound overflow"))?;
    Ok(MultiproofLimits {
        max_depth: limits.max_query_path_len,
        max_queried_leaves: capacity,
        max_siblings: work,
        max_parent_hashes: work,
    })
}

fn indexed_plan(
    leaves: usize,
    indices: Vec<usize>,
    capacity: usize,
    limits: VerifyLimits,
) -> Result<IndexedPlan> {
    let plan = MultiproofPlan::new(leaves, &indices, plan_limits(leaves, capacity, limits)?)?;
    Ok(IndexedPlan { indices, plan })
}

fn opening_plans(
    geometry: &Geometry,
    queries: &[usize],
    limits: VerifyLimits,
) -> Result<OpeningPlans> {
    let row_indices = queries
        .iter()
        .copied()
        .chain(
            queries
                .iter()
                .map(|&index| next_index(index, geometry.lde_rows)),
        )
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let row_capacity = queries
        .len()
        .checked_mul(2)
        .ok_or_else(|| shape("shared row bound overflow"))?
        .min(geometry.lde_rows);
    let rows = indexed_plan(geometry.lde_rows, row_indices, row_capacity, limits)?;
    let query_plan = indexed_plan(geometry.lde_rows, queries.to_vec(), queries.len(), limits)?;
    let mut current = queries.to_vec();
    let mut rounds = Vec::with_capacity(geometry.fri_lengths.len() - 1);
    for &length in &geometry.fri_lengths[..geometry.fri_lengths.len() - 1] {
        let leaves = length / 2;
        let indices = current
            .iter()
            .map(|&index| index % leaves)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        rounds.push(indexed_plan(
            leaves,
            indices.clone(),
            queries.len().min(leaves),
            limits,
        )?);
        current = indices;
    }
    let terminal = MultiproofPlan::new(1, &[0], plan_limits(1, 1, limits)?)?;
    Ok(OpeningPlans {
        rows,
        queries: query_plan,
        rounds,
        terminal,
    })
}

fn exact_indices(indices: impl Iterator<Item = u32>, expected: &[usize]) -> Result<()> {
    if !indices
        .map(|index| index as usize)
        .eq(expected.iter().copied())
    {
        return Err(shape(
            "shared table indices differ from the exact transcript-derived set",
        ));
    }
    Ok(())
}

fn exact_frontier(siblings: &[WireDigest], plan: &MultiproofPlan) -> Result<()> {
    if siblings.len() != plan.work().siblings {
        return Err(shape(
            "shared frontier needs exactly the derived sibling count",
        ));
    }
    Ok(())
}
#[cfg(test)]
pub(in crate::backend) fn from_compact(
    relation: &impl FixedAir,
    proof: &CompactProof,
    limits: VerifyLimits,
) -> Result<SharedProof> {
    let geometry = Geometry::new(relation)?;
    preflight(relation, proof, limits, &geometry)?;
    let binding = Binding::new(relation, &geometry)?;
    let challenges = replay_bound_challenges(
        relation,
        &geometry,
        &binding,
        proof.row_root,
        proof.mixed_root,
        proof.quotient_root,
        &proof.fri_roots,
    )?;
    exact_indices(
        proof.queries.iter().map(|query| query.index),
        &challenges.indices,
    )?;
    let plans = opening_plans(&geometry, &challenges.indices, limits)?;
    let mut row_values = BTreeMap::new();
    let mut row_paths = Vec::with_capacity(2 * proof.queries.len());
    let mut mixed_paths = Vec::with_capacity(proof.queries.len());
    let mut quotient_paths = Vec::with_capacity(proof.queries.len());
    let mut group_values: Vec<BTreeMap<usize, [GoldilocksFp4V1; 2]>> =
        (0..plans.rounds.len()).map(|_| BTreeMap::new()).collect();
    let mut group_paths: Vec<Vec<(usize, &[WireDigest])>> =
        (0..plans.rounds.len()).map(|_| Vec::new()).collect();
    let terminal_values = proof.queries[0].fri.final_values.clone();
    let mut terminal_paths = Vec::with_capacity(proof.queries.len());
    for (position, query) in proof.queries.iter().enumerate() {
        let initial = challenges.indices[position];
        for (index, values, path) in [
            (initial, &query.current, query.current_path.as_slice()),
            (
                next_index(initial, geometry.lde_rows),
                &query.next,
                query.next_path.as_slice(),
            ),
        ] {
            insert_equal(&mut row_values, index, values.clone())?;
            row_paths.push((index, path));
        }
        mixed_paths.push((initial, query.mixed_path.as_slice()));
        quotient_paths.push((initial, query.quotient_path.as_slice()));
        if query.fri.initial_index as usize != initial || query.fri.final_values != terminal_values
        {
            return Err(shape(
                "legacy shared FRI initial/terminal occurrence mismatch",
            ));
        }
        let mut index = initial;
        let mut value = challenges
            .joint
            .value_at(index, query.quotient, query.mixed)?;
        let mut domain = geometry.domain;
        for (round, opening) in query.fri.rounds.iter().enumerate() {
            let leaves = geometry.fri_lengths[round] / 2;
            let group = index % leaves;
            if opening.round as usize != round
                || opening.index as usize != index
                || opening.values[index / leaves] != value
            {
                return Err(shape("legacy shared FRI round/index/carry mismatch"));
            }
            let values: [GoldilocksFp4V1; 2] = opening
                .values
                .as_slice()
                .try_into()
                .map_err(|_| shape("legacy FRI group has another width"))?;
            insert_equal(&mut group_values[round], group, values)?;
            group_paths[round].push((group, opening.merkle_path.as_slice()));
            value = fold_fri_coset(
                &values,
                challenges.betas[round],
                domain.point(group),
                domain.coset_generator(leaves),
            )?;
            if value != opening.folded_value {
                return Err(shape("legacy shared FRI folded value mismatch"));
            }
            index = group;
            domain = domain.folded(2);
        }
        if query.fri.final_index as usize != index
            || terminal_values.get(index).copied() != Some(value)
        {
            return Err(shape("legacy shared FRI final carry mismatch"));
        }
        terminal_paths.push((0, query.fri.final_merkle_path.as_slice()));
    }
    let rows = row_values
        .into_iter()
        .map(|(index, values)| SharedRow {
            index: index as u32,
            values,
        })
        .collect::<Vec<_>>();
    exact_indices(rows.iter().map(|row| row.index), &plans.rows.indices)?;
    let row_leaves = rows
        .iter()
        .map(|row| {
            Ok((
                row.index as usize,
                binding.row(row.index as usize, &row.values)?,
            ))
        })
        .collect::<Result<Vec<_>>>()?;
    let queries = proof
        .queries
        .iter()
        .map(|query| SharedQuery {
            index: query.index,
            mixed: query.mixed,
            quotient: query.quotient,
        })
        .collect::<Vec<_>>();
    let mixed_leaves = queries
        .iter()
        .map(|query| {
            Ok((
                query.index as usize,
                binding.mixed(query.index as usize, query.mixed)?,
            ))
        })
        .collect::<Result<Vec<_>>>()?;
    let quotient_leaves = queries
        .iter()
        .map(|query| {
            Ok((
                query.index as usize,
                binding.quotient(query.index as usize, query.quotient)?,
            ))
        })
        .collect::<Result<Vec<_>>>()?;
    let row_siblings = extract_frontier(
        &binding,
        geometry.lde_rows,
        &plans.rows,
        MerkleTreeRoleV1::AirTrace,
        proof.row_root,
        &row_leaves,
        &row_paths,
    )?;
    let mixed_siblings = extract_frontier(
        &binding,
        geometry.lde_rows,
        &plans.queries,
        MerkleTreeRoleV1::Lde,
        proof.mixed_root,
        &mixed_leaves,
        &mixed_paths,
    )?;
    let quotient_siblings = extract_frontier(
        &binding,
        geometry.lde_rows,
        &plans.queries,
        MerkleTreeRoleV1::AirComposition,
        proof.quotient_root,
        &quotient_leaves,
        &quotient_paths,
    )?;
    let mut rounds = Vec::with_capacity(plans.rounds.len());
    for (round, values) in group_values.into_iter().enumerate() {
        let groups = values
            .into_iter()
            .map(|(index, values)| SharedGroup {
                index: index as u32,
                values,
            })
            .collect::<Vec<_>>();
        exact_indices(
            groups.iter().map(|group| group.index),
            &plans.rounds[round].indices,
        )?;
        let leaves = groups
            .iter()
            .map(|group| {
                Ok((
                    group.index as usize,
                    binding.fri(round, group.index as usize, &group.values)?,
                ))
            })
            .collect::<Result<Vec<_>>>()?;
        let siblings = extract_frontier(
            &binding,
            geometry.fri_lengths[round] / 2,
            &plans.rounds[round],
            MerkleTreeRoleV1::Fri(round as u32),
            proof.fri_roots[round],
            &leaves,
            &group_paths[round],
        )?;
        rounds.push(SharedRound { groups, siblings });
    }
    let final_round = rounds.len();
    let terminal_leaf = binding.fri(final_round, 0, &terminal_values)?;
    let terminal_plan = IndexedPlan {
        indices: vec![0],
        plan: plans.terminal,
    };
    extract_frontier(
        &binding,
        1,
        &terminal_plan,
        MerkleTreeRoleV1::Fri(final_round as u32),
        proof.fri_roots[final_round],
        &[(0, terminal_leaf)],
        &terminal_paths,
    )?;
    Ok(SharedProof {
        row_root: proof.row_root,
        mixed_root: proof.mixed_root,
        quotient_root: proof.quotient_root,
        fri_roots: proof.fri_roots.clone(),
        rows,
        queries,
        row_siblings,
        mixed_siblings,
        quotient_siblings,
        rounds,
        terminal_values,
    })
}

#[cfg(test)]
fn insert_equal<K: Ord, V: PartialEq>(map: &mut BTreeMap<K, V>, key: K, value: V) -> Result<()> {
    match map.entry(key) {
        std::collections::btree_map::Entry::Vacant(entry) => {
            entry.insert(value);
        }
        std::collections::btree_map::Entry::Occupied(entry) if entry.get() == &value => {}
        std::collections::btree_map::Entry::Occupied(_) => {
            return Err(shape("conflicting duplicate shared opening occurrence"));
        }
    }
    Ok(())
}

#[cfg(test)]
fn extract_frontier(
    binding: &Binding,
    leaf_count: usize,
    plan: &IndexedPlan,
    role: MerkleTreeRoleV1,
    root: WireDigest,
    leaves: &[(usize, Digest)],
    paths: &[(usize, &[WireDigest])],
) -> Result<Vec<WireDigest>> {
    let depth = (leaf_count.ilog2() as usize).max(1);
    let mut nodes = BTreeMap::new();
    for &(index, digest) in leaves {
        insert_equal(&mut nodes, SiblingPosition { level: 0, index }, digest)?;
    }
    if leaf_count == 1 {
        insert_equal(
            &mut nodes,
            SiblingPosition { level: 0, index: 1 },
            leaves[0].1,
        )?;
    }
    for &(index, path) in paths {
        if index >= leaf_count || path.len() != depth {
            return Err(shape("legacy shared path has another exact geometry"));
        }
        for (level, digest) in path.iter().enumerate() {
            insert_equal(
                &mut nodes,
                SiblingPosition {
                    level,
                    index: (index >> level) ^ 1,
                },
                digest.as_fastpq(),
            )?;
        }
    }
    let mut current = plan.indices.clone();
    for level in 0..depth {
        let parents = current
            .iter()
            .map(|index| index / 2)
            .collect::<BTreeSet<_>>();
        for &index in &parents {
            let left = nodes
                .get(&SiblingPosition {
                    level,
                    index: 2 * index,
                })
                .copied()
                .ok_or_else(|| shape("missing shared left child"))?;
            let right = nodes
                .get(&SiblingPosition {
                    level,
                    index: 2 * index + 1,
                })
                .copied()
                .ok_or_else(|| shape("missing shared right child"))?;
            let digest = binding.parent(role, level + 1, index, left, right)?;
            insert_equal(
                &mut nodes,
                SiblingPosition {
                    level: level + 1,
                    index,
                },
                digest,
            )?;
        }
        current = parents.into_iter().collect();
    }
    if nodes
        .get(&SiblingPosition {
            level: depth,
            index: 0,
        })
        .copied()
        != Some(root.as_fastpq())
    {
        return Err(Error::QueryMerklePathMismatch { index: 0 });
    }
    plan.plan
        .sibling_positions()
        .iter()
        .map(|position| {
            nodes
                .get(position)
                .copied()
                .map(WireDigest::from)
                .ok_or_else(|| shape("missing canonical shared frontier node"))
        })
        .collect()
}

fn preflight_shared(
    relation: &impl FixedAir,
    proof: &SharedProof,
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
    let query_count = profile::QUERY_COUNT;
    check_limit("max_queries", proof.queries.len(), limits.max_queries)?;
    if proof.queries.len() != query_count
        || proof.fri_roots.len() != geometry.fri_lengths.len()
        || proof.rounds.len() + 1 != geometry.fri_lengths.len()
    {
        return Err(shape(
            "shared proof needs exact canonical query/layer counts",
        ));
    }
    let row_capacity = query_count
        .checked_mul(2)
        .ok_or_else(|| shape("shared row count overflow"))?
        .min(geometry.lde_rows);
    if proof.rows.len() < query_count || proof.rows.len() > row_capacity {
        return Err(shape(
            "shared complete row table exceeds its exact query bounds",
        ));
    }
    sorted_indices(proof.rows.iter().map(|row| row.index), geometry.lde_rows)?;
    sorted_indices(
        proof.queries.iter().map(|query| query.index),
        geometry.lde_rows,
    )?;
    for row in &proof.rows {
        if row.values.len() != geometry.schema.width {
            return Err(shape("shared complete row has another width"));
        }
    }
    bounded_frontier(&proof.row_siblings, geometry.lde_rows, row_capacity, limits)?;
    bounded_frontier(
        &proof.mixed_siblings,
        geometry.lde_rows,
        query_count,
        limits,
    )?;
    bounded_frontier(
        &proof.quotient_siblings,
        geometry.lde_rows,
        query_count,
        limits,
    )?;
    for (round, opening) in proof.rounds.iter().enumerate() {
        let leaf_count = geometry.fri_lengths[round] / 2;
        if opening.groups.is_empty() || opening.groups.len() > query_count.min(leaf_count) {
            return Err(shape(
                "shared FRI group count exceeds its derived upper bound",
            ));
        }
        check_limit("max_fri_round_values", 2, limits.max_fri_round_values)?;
        sorted_indices(opening.groups.iter().map(|group| group.index), leaf_count)?;
        bounded_frontier(
            &opening.siblings,
            leaf_count,
            query_count.min(leaf_count),
            limits,
        )?;
    }
    let terminal_len = *geometry
        .fri_lengths
        .last()
        .expect("validated nonempty FRI geometry");
    check_limit(
        "max_fri_round_values",
        proof.terminal_values.len(),
        limits.max_fri_round_values,
    )?;
    if proof.terminal_values.len() != terminal_len {
        return Err(shape(
            "shared terminal must contain the complete natural-order evaluations",
        ));
    }
    let _flags = norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
    let bytes = norito::core::encoded_frame_len(proof)?;
    check_limit("max_proof_bytes", bytes, limits.max_proof_bytes)?;
    for (row, opening) in proof.rows.iter().enumerate() {
        for (column, &value) in opening.values.iter().enumerate() {
            canonical_base(value, "shared_complete_row", &[row, column])?;
        }
    }
    for (query, opening) in proof.queries.iter().enumerate() {
        canonical_extension(opening.mixed, &[query, 0])?;
        canonical_extension(opening.quotient, &[query, 1])?;
    }
    for (round, opening) in proof.rounds.iter().enumerate() {
        for (group, opening) in opening.groups.iter().enumerate() {
            for (position, &value) in opening.values.iter().enumerate() {
                canonical_extension(value, &[round, group, position])?;
            }
        }
    }
    for (position, &value) in proof.terminal_values.iter().enumerate() {
        canonical_extension(value, &[proof.rounds.len(), position])?;
    }
    Ok(bytes)
}

fn sorted_indices(indices: impl Iterator<Item = u32>, length: usize) -> Result<()> {
    let mut previous = None;
    for index in indices {
        if index as usize >= length || previous.is_some_and(|previous| previous >= index) {
            return Err(shape(
                "shared table must use sorted unique in-range indices",
            ));
        }
        previous = Some(index);
    }
    Ok(())
}

fn bounded_frontier(
    siblings: &[WireDigest],
    leaves: usize,
    capacity: usize,
    limits: VerifyLimits,
) -> Result<()> {
    let bounds = plan_limits(leaves, capacity, limits)?;
    check_limit(
        "max_query_path_len",
        (leaves.ilog2() as usize).max(1),
        bounds.max_depth,
    )?;
    check_limit("max_shared_siblings", siblings.len(), bounds.max_siblings)
}

/// Verify exact shared openings directly, using only bounded authenticated data.
pub(in crate::backend) fn verify_shared(
    relation: &impl FixedAir,
    proof: &SharedProof,
    limits: VerifyLimits,
) -> Result<SharedVerificationWork> {
    let mut work = SharedVerificationWork::default();
    verify_shared_recorded(relation, proof, limits, &mut work)?;
    Ok(work)
}

/// Prove the fixed six-lane compact V1 and encode only canonical shared openings.
/// The caller supplies diagnostic limits; this does not qualify a production profile.
#[cfg(test)]
pub(in crate::backend) fn prove_shared(
    relation: &impl FixedAir,
    columns: &[Vec<u64>],
    limits: VerifyLimits,
) -> Result<SharedProof> {
    let trace = prepare_trace(relation, columns)?;
    let proof = prove_prepared(relation, &trace)?;
    drop(trace);
    from_compact(relation, &proof, limits)
}
pub(super) fn verify_shared_recorded(
    relation: &impl FixedAir,
    proof: &SharedProof,
    limits: VerifyLimits,
    work: &mut SharedVerificationWork,
) -> Result<()> {
    let geometry = Geometry::new(relation)?;
    work.proof_bytes = preflight_shared(relation, proof, limits, &geometry)?;
    let binding = Binding::new(relation, &geometry)?;
    work.transcripts += 1;
    let challenges = replay_bound_challenges(
        relation,
        &geometry,
        &binding,
        proof.row_root,
        proof.mixed_root,
        proof.quotient_root,
        &proof.fri_roots,
    )?;
    let plans = opening_plans(&geometry, &challenges.indices, limits)?;
    exact_indices(
        proof.queries.iter().map(|query| query.index),
        &challenges.indices,
    )?;
    exact_indices(proof.rows.iter().map(|row| row.index), &plans.rows.indices)?;
    exact_frontier(&proof.row_siblings, &plans.rows.plan)?;
    exact_frontier(&proof.mixed_siblings, &plans.queries.plan)?;
    exact_frontier(&proof.quotient_siblings, &plans.queries.plan)?;
    for (opening, plan) in proof.rounds.iter().zip(&plans.rounds) {
        exact_indices(
            opening.groups.iter().map(|group| group.index),
            &plan.indices,
        )?;
        exact_frontier(&opening.siblings, &plan.plan)?;
    }
    // Every table set/count and frontier is now exact. No leaf or node hash has
    // run; proof-supplied indices cannot redirect an authentication plan.
    let row_leaves = proof
        .rows
        .iter()
        .map(|row| {
            work.row_leaves += 1;
            binding.row(row.index as usize, &row.values)
        })
        .collect::<Result<Vec<_>>>()?;
    authenticate_shared(
        &binding,
        &plans.rows.plan,
        MerkleTreeRoleV1::AirTrace,
        proof.row_root,
        &row_leaves,
        &proof.row_siblings,
        work,
    )?;
    let mixed_leaves = proof
        .queries
        .iter()
        .map(|query| {
            work.oracle_leaves += 1;
            binding.mixed(query.index as usize, query.mixed)
        })
        .collect::<Result<Vec<_>>>()?;
    authenticate_shared(
        &binding,
        &plans.queries.plan,
        MerkleTreeRoleV1::Lde,
        proof.mixed_root,
        &mixed_leaves,
        &proof.mixed_siblings,
        work,
    )?;
    let quotient_leaves = proof
        .queries
        .iter()
        .map(|query| {
            work.oracle_leaves += 1;
            binding.quotient(query.index as usize, query.quotient)
        })
        .collect::<Result<Vec<_>>>()?;
    authenticate_shared(
        &binding,
        &plans.queries.plan,
        MerkleTreeRoleV1::AirComposition,
        proof.quotient_root,
        &quotient_leaves,
        &proof.quotient_siblings,
        work,
    )?;
    for (round, opening) in proof.rounds.iter().enumerate() {
        let leaves = opening
            .groups
            .iter()
            .map(|group| {
                work.fri_leaves += 1;
                binding.fri(round, group.index as usize, &group.values)
            })
            .collect::<Result<Vec<_>>>()?;
        authenticate_shared(
            &binding,
            &plans.rounds[round].plan,
            MerkleTreeRoleV1::Fri(round as u32),
            proof.fri_roots[round],
            &leaves,
            &opening.siblings,
            work,
        )?;
    }
    let final_round = proof.rounds.len();
    work.fri_leaves += 1;
    let terminal_leaf = binding.fri(final_round, 0, &proof.terminal_values)?;
    authenticate_shared(
        &binding,
        &plans.terminal,
        MerkleTreeRoleV1::Fri(final_round as u32),
        proof.fri_roots[final_round],
        &[terminal_leaf],
        &[],
        work,
    )?;
    let weights = AirQuotientDomain::new(&FASTPQ_FINAL_V1, geometry.lde_rows)?;
    let mut carries = Vec::with_capacity(proof.queries.len());
    for (position, query) in proof.queries.iter().enumerate() {
        let index = query.index as usize;
        let current = shared_row(&proof.rows, index)?;
        let next = shared_row(&proof.rows, next_index(index, geometry.lde_rows))?;
        let mixed = current
            .iter()
            .zip(&challenges.mixing)
            .fold(GoldilocksFp4V1::ZERO, |sum, (&value, coefficient)| {
                sum.add(coefficient.mul_base(value))
            });
        if mixed != query.mixed {
            return Err(Error::QueryMismatch { index: position });
        }
        let residues = relation.evaluate(geometry.domain.point(index), current, next)?;
        work.air_evaluations += 1;
        let quotient =
            combine(&residues, &challenges.alphas)?.mul_base(weights.weights_at(index)?.all_rows);
        if quotient != query.quotient {
            return Err(Error::AirOpeningMismatch { index: position });
        }
        carries.push((
            index,
            challenges
                .joint
                .value_at(index, query.quotient, query.mixed)?,
        ));
    }
    let mut domain = geometry.domain;
    for (round, opening) in proof.rounds.iter().enumerate() {
        let output_len = geometry.fri_lengths[round] / 2;
        let folded = opening
            .groups
            .iter()
            .map(|group| {
                fold_fri_coset(
                    &group.values,
                    challenges.betas[round],
                    domain.point(group.index as usize),
                    domain.coset_generator(output_len),
                )
            })
            .collect::<Result<Vec<_>>>()?;
        for (position, (index, value)) in carries.iter_mut().enumerate() {
            let group_index = *index % output_len;
            let group = opening
                .groups
                .binary_search_by_key(&(group_index as u32), |group| group.index)
                .map_err(|_| shape("missing derived FRI group"))?;
            if opening.groups[group].values[*index / output_len] != *value {
                return Err(Error::QueryMismatch { index: position });
            }
            *index = group_index;
            *value = folded[group];
        }
        domain = domain.folded(2);
    }
    for (position, (index, value)) in carries.into_iter().enumerate() {
        if proof.terminal_values.get(index).copied() != Some(value) {
            return Err(Error::QueryMismatch { index: position });
        }
    }
    work.terminal_degree_checks += 1;
    if !domain.evaluations_have_degree_below(&proof.terminal_values, geometry.terminal_degree)? {
        return Err(Error::FriTerminalDegreeMismatch {
            degree_bound: geometry.terminal_degree,
        });
    }
    Ok(())
}

fn shared_row(rows: &[SharedRow], index: usize) -> Result<&[u64]> {
    let position = rows
        .binary_search_by_key(&(index as u32), |row| row.index)
        .map_err(|_| shape("missing derived complete row"))?;
    Ok(&rows[position].values)
}

fn authenticate_shared(
    binding: &Binding,
    plan: &MultiproofPlan,
    role: MerkleTreeRoleV1,
    root: WireDigest,
    leaves: &[Digest],
    siblings: &[WireDigest],
    work: &mut SharedVerificationWork,
) -> Result<()> {
    let siblings = siblings
        .iter()
        .copied()
        .map(WireDigest::as_fastpq)
        .collect::<Vec<_>>();
    let actual = binding.verify_tree(plan, role, root.as_fastpq(), leaves, &siblings)?;
    work.parent_hashes += actual.parent_hashes;
    Ok(())
}

#[cfg(test)]
mod tests {

    use super::*;

    use super::super::test_fixture::FixedColumnsAir;
    pub(super) use super::super::test_fixture::fixture;
    pub(super) fn air() -> FixedColumnsAir {
        FixedColumnsAir::new(7)
    }
    fn conversion_limits() -> VerifyLimits {
        super::super::test_fixture::limits()
    }
    fn diagnostic_limits() -> VerifyLimits {
        super::super::test_fixture::limits()
    }

    fn changed_field(value: GoldilocksFp4V1, coefficient: usize) -> GoldilocksFp4V1 {
        let mut coefficients = value.coefficients();
        coefficients[coefficient] = crate::backend::add_mod(coefficients[coefficient], 1);
        GoldilocksFp4V1::new(coefficients).unwrap()
    }

    fn changed_digest(value: WireDigest, coefficient: usize) -> WireDigest {
        let mut words = value.words();
        words[coefficient] = crate::backend::add_mod(words[coefficient], 1);
        WireDigest::new(words).unwrap()
    }

    #[test]
    fn canonical_shared_conversion_preserves_transcript_and_authenticated_results() {
        let fixture = fixture();
        let relation = air();
        let compact_before = fixture.compact.clone();
        let converted = from_compact(&relation, &fixture.compact, conversion_limits()).unwrap();
        assert_eq!(converted, fixture.shared);
        assert_eq!(fixture.compact, compact_before);
        let geometry = Geometry::new(&relation).unwrap();
        let challenges = replay_challenges(
            &relation,
            &geometry,
            converted.row_root,
            converted.mixed_root,
            converted.quotient_root,
            &converted.fri_roots,
        )
        .unwrap();
        assert_eq!(
            challenges.indices,
            fixture
                .compact
                .queries
                .iter()
                .map(|query| query.index as usize)
                .collect::<Vec<_>>()
        );
        let work = verify_shared(&relation, &converted, diagnostic_limits()).unwrap();
        let plans = opening_plans(&geometry, &challenges.indices, diagnostic_limits()).unwrap();
        assert_eq!(work.transcripts, 1);
        assert_eq!(work.row_leaves, plans.rows.indices.len());
        assert!(work.row_leaves <= 2 * challenges.indices.len());
        assert_eq!(work.oracle_leaves, 2 * challenges.indices.len());
        assert_eq!(work.air_evaluations, challenges.indices.len());
        assert_eq!(work.terminal_degree_checks, 1);
        assert_eq!(
            work.fri_leaves,
            1 + plans
                .rounds
                .iter()
                .map(|round| round.indices.len())
                .sum::<usize>()
        );
        assert_eq!(
            work.parent_hashes,
            plans.rows.plan.work().parent_hashes
                + 2 * plans.queries.plan.work().parent_hashes
                + plans
                    .rounds
                    .iter()
                    .map(|round| round.plan.work().parent_hashes)
                    .sum::<usize>()
                + 1
        );
        let full_bytes = norito::core::to_bytes(&fixture.compact).unwrap().len();
        let shared_bytes = norito::core::to_bytes(&converted).unwrap().len();
        assert_eq!(work.proof_bytes, shared_bytes);
        assert!(shared_bytes < full_bytes);
        eprintln!(
            "compact_shared_tiny_legacy_bytes={full_bytes}; shared_bytes={shared_bytes}; work={work:?}"
        );
    }

    #[test]
    fn every_root_coordinate_and_extension_coordinate_is_bound() {
        let baseline = &fixture().shared;
        let relation = air();
        for coefficient in 0..6 {
            for root in 0..3 + baseline.fri_roots.len() {
                let mut proof = baseline.clone();
                let value = match root {
                    0 => &mut proof.row_root,
                    1 => &mut proof.mixed_root,
                    2 => &mut proof.quotient_root,
                    _ => &mut proof.fri_roots[root - 3],
                };
                *value = changed_digest(*value, coefficient);
                assert!(
                    verify_shared(&relation, &proof, diagnostic_limits()).is_err(),
                    "root={root}, coefficient={coefficient}"
                );
            }
        }
        for coefficient in 0..4 {
            for field in 0..8 {
                let mut proof = baseline.clone();
                let value = match field {
                    0 => &mut proof.queries[0].mixed,
                    1 => &mut proof.queries[0].quotient,
                    2 => &mut proof.rounds[0].groups[0].values[0],
                    3 => &mut proof.rounds[0].groups[0].values[1],
                    _ => &mut proof.terminal_values[field - 4],
                };
                *value = changed_field(*value, coefficient);
                assert!(
                    verify_shared(&relation, &proof, diagnostic_limits()).is_err(),
                    "field={field}, coefficient={coefficient}"
                );
            }
        }
        let mut changed_row = baseline.clone();
        changed_row.rows[0].values[0] += 1;
        assert!(verify_shared(&relation, &changed_row, diagnostic_limits()).is_err());
        let false_statement = FixedColumnsAir::new(8);
        assert!(verify_shared(&false_statement, baseline, diagnostic_limits()).is_err());
    }

    #[test]
    fn shared_siblings_are_exact_and_every_digest_coordinate_is_checked() {
        fn select(proof: &mut SharedProof, frontier: usize) -> &mut Vec<WireDigest> {
            match frontier {
                0 => &mut proof.row_siblings,
                1 => &mut proof.mixed_siblings,
                2 => &mut proof.quotient_siblings,
                _ => &mut proof.rounds[frontier - 3].siblings,
            }
        }
        let baseline = &fixture().shared;
        let relation = air();
        for frontier in 0..3 + baseline.rounds.len() {
            let mut extra = baseline.clone();
            select(&mut extra, frontier).push(WireDigest::new([1; 6]).unwrap());
            let mut work = SharedVerificationWork::default();
            assert!(
                verify_shared_recorded(&relation, &extra, diagnostic_limits(), &mut work).is_err()
            );
            assert_eq!(
                work.row_leaves + work.oracle_leaves + work.fri_leaves + work.parent_hashes,
                0
            );
            if select(&mut extra, frontier).len() == 1 {
                continue; // Fully covered trees legitimately have no siblings.
            }
            let mut missing = baseline.clone();
            select(&mut missing, frontier).pop();
            let mut work = SharedVerificationWork::default();
            assert!(
                verify_shared_recorded(&relation, &missing, diagnostic_limits(), &mut work)
                    .is_err()
            );
            assert_eq!(
                work.row_leaves + work.oracle_leaves + work.fri_leaves + work.parent_hashes,
                0
            );
            for coefficient in 0..6 {
                let mut changed = baseline.clone();
                let values = select(&mut changed, frontier);
                values[0] = changed_digest(values[0], coefficient);
                assert!(
                    verify_shared(&relation, &changed, diagnostic_limits()).is_err(),
                    "frontier={frontier}, coefficient={coefficient}"
                );
            }
        }
    }

    fn assert_preflight_rejection(proof: &SharedProof, limits: VerifyLimits) {
        let mut work = SharedVerificationWork::default();
        assert!(verify_shared_recorded(&air(), proof, limits, &mut work).is_err());
        assert_eq!(work.transcripts, 0);
        assert_eq!(
            work.row_leaves
                + work.oracle_leaves
                + work.fri_leaves
                + work.parent_hashes
                + work.air_evaluations,
            0
        );
    }

    #[test]
    fn malformed_tables_fields_and_resource_limits_reject_before_transcript_hashing() {
        let baseline = &fixture().shared;
        for table in 0..3 {
            let mut reordered = baseline.clone();
            let mut duplicate = baseline.clone();
            match table {
                0 => {
                    reordered.rows.swap(0, 1);
                    duplicate.rows[1].index = duplicate.rows[0].index;
                }
                1 => {
                    reordered.queries.swap(0, 1);
                    duplicate.queries[1].index = duplicate.queries[0].index;
                }
                _ => {
                    reordered.rounds[0].groups.swap(0, 1);
                    duplicate.rounds[0].groups[1].index = duplicate.rounds[0].groups[0].index;
                }
            }
            assert_preflight_rejection(&reordered, diagnostic_limits());
            assert_preflight_rejection(&duplicate, diagnostic_limits());
        }
        let mut noncanonical_row = baseline.clone();
        noncanonical_row.rows[0].values[0] = GOLDILOCKS_MODULUS;
        assert_preflight_rejection(&noncanonical_row, diagnostic_limits());
        for lane in 0..4 {
            for field in 0..4 {
                let mut proof = baseline.clone();
                let mut coefficients = [0; 4];
                coefficients[lane] = GOLDILOCKS_MODULUS;
                let invalid = GoldilocksFp4V1::from_coefficients_unchecked_for_test(coefficients);
                match field {
                    0 => proof.queries[0].mixed = invalid,
                    1 => proof.queries[0].quotient = invalid,
                    2 => proof.rounds[0].groups[0].values[1] = invalid,
                    _ => proof.terminal_values[3] = invalid,
                }
                assert_preflight_rejection(&proof, diagnostic_limits());
            }
        }
        let mut wrong_width = baseline.clone();
        wrong_width.rows[0].values.push(0);
        assert_preflight_rejection(&wrong_width, diagnostic_limits());
        let mut wrong_terminal = baseline.clone();
        wrong_terminal.terminal_values.pop();
        assert_preflight_rejection(&wrong_terminal, diagnostic_limits());
        let mut huge_frontier = baseline.clone();
        huge_frontier.row_siblings = vec![WireDigest::new([0; 6]).unwrap(); 14_251];
        assert_preflight_rejection(&huge_frontier, diagnostic_limits());
        let exact_bytes = norito::core::to_bytes(baseline).unwrap().len();
        for limits in [
            VerifyLimits {
                max_proof_bytes: exact_bytes - 1,
                ..diagnostic_limits()
            },
            VerifyLimits {
                max_queries: 374,
                ..diagnostic_limits()
            },
            VerifyLimits {
                max_query_path_len: 18,
                ..diagnostic_limits()
            },
            VerifyLimits {
                max_fri_round_values: 1,
                ..diagnostic_limits()
            },
            VerifyLimits {
                max_air_row_values: 0,
                ..diagnostic_limits()
            },
        ] {
            assert_preflight_rejection(baseline, limits);
        }
    }

    #[test]
    fn transcript_derived_sets_reject_missing_extra_and_other_in_range_indices_before_leaves() {
        let baseline = &fixture().shared;
        let mut missing = baseline.clone();
        missing.rows.pop();
        let mut extra = baseline.clone();
        let absent = (0..524_288)
            .find(|index| !extra.rows.iter().any(|row| row.index == *index))
            .unwrap();
        extra.rows.push(SharedRow {
            index: absent,
            values: air().row(),
        });
        extra.rows.sort_by_key(|row| row.index);
        let mut other_query = baseline.clone();
        let absent_query = (0..524_288)
            .find(|index| {
                !other_query
                    .queries
                    .iter()
                    .any(|query| query.index == *index)
            })
            .unwrap();
        other_query.queries[0].index = absent_query;
        other_query.queries.sort_by_key(|query| query.index);
        let mut missing_group = baseline.clone();
        missing_group.rounds[0].groups.pop();
        for proof in [missing, extra, other_query, missing_group] {
            let mut work = SharedVerificationWork::default();
            assert!(
                verify_shared_recorded(&air(), &proof, diagnostic_limits(), &mut work).is_err()
            );
            assert_eq!(
                work.row_leaves + work.oracle_leaves + work.fri_leaves + work.parent_hashes,
                0
            );
        }
    }

    #[test]
    fn converter_rejects_conflicting_duplicates_and_discarded_legacy_fields() {
        let baseline = &fixture().compact;
        let relation = air();
        let mut rows = BTreeMap::new();
        let row = air().row();
        insert_equal(&mut rows, 17, row.clone()).unwrap();
        insert_equal(&mut rows, 17, row.clone()).unwrap();
        let mut conflict = row;
        conflict[341] += 1;
        assert!(insert_equal(&mut rows, 17, conflict).is_err());
        for mutation in 0..8 {
            let mut proof = baseline.clone();
            match mutation {
                0 => proof.queries[0].fri.initial_index ^= 1,
                1 => proof.queries[0].fri.rounds[0].round ^= 1,
                2 => proof.queries[0].fri.rounds[0].index ^= 1,
                3 => {
                    proof.queries[0].fri.rounds[0].folded_value =
                        changed_field(proof.queries[0].fri.rounds[0].folded_value, 3)
                }
                4 => proof.queries[0].fri.final_index ^= 1,
                5 => {
                    proof.queries[0].fri.final_values[0] =
                        changed_field(proof.queries[0].fri.final_values[0], 2)
                }
                6 => {
                    proof.queries[0].current_path[0] =
                        changed_digest(proof.queries[0].current_path[0], 5)
                }
                7 => {
                    proof.queries[0].fri.final_merkle_path[0] =
                        changed_digest(proof.queries[0].fri.final_merkle_path[0], 4)
                }
                _ => unreachable!(),
            }
            assert!(
                from_compact(&relation, &proof, conversion_limits()).is_err(),
                "mutation={mutation}"
            );
        }
        // The first layer with fewer than375 group positions guarantees a
        // duplicated group, without an assumption about transcript collisions.
        let geometry = Geometry::new(&relation).unwrap();
        let round = geometry
            .fri_lengths
            .iter()
            .position(|&length| length / 2 < 375)
            .unwrap();
        let leaves = geometry.fri_lengths[round] / 2;
        let duplicate_group = baseline
            .queries
            .iter()
            .enumerate()
            .find_map(|(position, query)| {
                baseline.queries[..position]
                    .iter()
                    .any(|other| other.index as usize % leaves == query.index as usize % leaves)
                    .then_some(position)
            })
            .unwrap();
        let mut group_conflict = baseline.clone();
        let values = &mut group_conflict.queries[duplicate_group].fri.rounds[round].values;
        values[0] = changed_field(values[0], 1);
        assert!(from_compact(&relation, &group_conflict, conversion_limits()).is_err());
    }

    #[test]
    fn freshly_proved_false_claim_passes_openings_and_fails_actual_terminal_degree() {
        let relation = air();
        let false_compact = prove(&relation, &FixedColumnsAir::new(8).columns()).unwrap();
        let false_shared = from_compact(&relation, &false_compact, conversion_limits()).unwrap();
        let mut work = SharedVerificationWork::default();
        assert!(matches!(
            verify_shared_recorded(&relation, &false_shared, diagnostic_limits(), &mut work),
            Err(Error::FriTerminalDegreeMismatch { degree_bound: 1 })
        ));
        assert_eq!(work.air_evaluations, 375);
        assert_eq!(work.terminal_degree_checks, 1);
        assert!(work.parent_hashes > 0 && work.fri_leaves > 0);
    }

    #[test]
    fn nonconstant_next_row_relation_rejects_coherently_reproved_stride_one_and_swapped_rows() {
        use crate::backend::{field_pow, mul_mod, sub_mod};

        const TRACE_ROWS: usize = 65_536;
        const LDE_ROWS: usize = 524_288;

        #[derive(Clone, Copy, Debug)]
        enum NextMapping {
            Correct,
            StrideOne,
            Swapped,
        }

        // All923 test slots are real polynomials: the selected recurrence,
        // 341 zero current columns,341 zero next columns, then240 nonzero
        // multiples of the recurrence. No production relation is replaced.
        fn slots(recurrence: u64, current: &[u64], next: &[u64]) -> Result<Vec<u64>> {
            let mut slots = Vec::with_capacity(923);
            slots.push(recurrence);
            slots.extend_from_slice(&current[1..]);
            slots.extend_from_slice(&next[1..]);
            slots.extend((2..=241).map(|scale| mul_mod(recurrence, scale)));
            assert_eq!(slots.len(), 923);
            Ok(slots)
        }

        struct RecurrenceAir {
            mapping: NextMapping,
            trace_generator: u64,
            lde_generator: u64,
        }

        struct PreparedRecurrence {
            mapping: NextMapping,
            trace_generator: u64,
            values: Vec<u64>,
        }

        impl PreparedAir for PreparedRecurrence {
            fn evaluator(&self) -> ProverEvaluator<'_> {
                Box::new(move |index, _, current, next| {
                    // The real engine always supplies physical stride-eight
                    // rows. Select the deliberately wrong committed row here
                    // to model the faulty producer mapping explicitly.
                    assert_eq!(current.len(), 342);
                    assert_eq!(current[0], self.values[index]);
                    assert!(current[1..].iter().all(|&value| value == 0));
                    assert_eq!(next.len(), 342);
                    assert_eq!(next[0], self.values[(index + 8) % LDE_ROWS]);
                    assert!(next[1..].iter().all(|&value| value == 0));
                    let (left, right) = match self.mapping {
                        NextMapping::Correct => (current[0], next[0]),
                        NextMapping::StrideOne => (current[0], self.values[(index + 1) % LDE_ROWS]),
                        NextMapping::Swapped => (next[0], current[0]),
                    };
                    slots(
                        sub_mod(right, mul_mod(self.trace_generator, left)),
                        current,
                        next,
                    )
                })
            }
        }

        impl FixedAir for RecurrenceAir {
            fn schema(&self) -> FixedAirSchema {
                FixedAirSchema {
                    trace_rows: TRACE_ROWS,
                    width: 342,
                    constraints: 923,
                    identity: "shared-opening-test:next-row-recurrence:v1",
                }
            }

            fn statement_bytes(&self) -> &[u8] {
                b"shared-opening-test:next-equals-trace-generator-times-current"
            }

            fn evaluate(&self, point: u64, current: &[u64], next: &[u64]) -> Result<Vec<u64>> {
                if current.len() != 342 || next.len() != 342 {
                    return Err(shape("recurrence fixture requires complete342-column rows"));
                }
                let (left, right) = match self.mapping {
                    NextMapping::Correct => (next[0], current[0]),
                    // This adversarial producer uses f(X)=X^8. Its alternate
                    // next value is known exactly at x*g_L, so all quotient,
                    // FRI and Merkle data can be recomputed coherently for the
                    // wrong stride instead of merely corrupting a path.
                    NextMapping::StrideOne => {
                        (field_pow(mul_mod(point, self.lde_generator), 8), current[0])
                    }
                    NextMapping::Swapped => (current[0], next[0]),
                };
                slots(
                    sub_mod(left, mul_mod(self.trace_generator, right)),
                    current,
                    next,
                )
            }

            fn prepare_prover(&self) -> Result<Box<dyn PreparedAir + '_>> {
                let exponent = match self.mapping {
                    NextMapping::Correct => 1,
                    NextMapping::StrideOne => 8,
                    NextMapping::Swapped => TRACE_ROWS - 1,
                };
                let values = (0..LDE_ROWS)
                    .map(|index| {
                        let point = mul_mod(
                            FASTPQ_FINAL_V1.omega_coset,
                            field_pow(self.lde_generator, index as u64),
                        );
                        field_pow(point, exponent as u64)
                    })
                    .collect();
                Ok(Box::new(PreparedRecurrence {
                    mapping: self.mapping,
                    trace_generator: self.trace_generator,
                    values,
                }))
            }
        }

        let trace_generator = FixedTraceDomain::new(&FASTPQ_FINAL_V1, TRACE_ROWS)
            .unwrap()
            .generator;
        let domain = FriDomain::from_lde_parameters(
            FASTPQ_FINAL_V1.lde_root,
            FASTPQ_FINAL_V1.lde_log_size,
            LDE_ROWS,
            FASTPQ_FINAL_V1.omega_coset,
        )
        .unwrap();
        let lde_generator = domain.coset_generator(1);
        assert_eq!(field_pow(lde_generator, 8), trace_generator);
        assert_eq!(next_index(LDE_ROWS - 1, LDE_ROWS), 7);
        let intended = RecurrenceAir {
            mapping: NextMapping::Correct,
            trace_generator,
            lde_generator,
        };
        for (mapping, exponent) in [
            (NextMapping::Correct, 1),
            (NextMapping::StrideOne, 8),
            (NextMapping::Swapped, TRACE_ROWS - 1),
        ] {
            let producer = RecurrenceAir {
                mapping,
                trace_generator,
                lde_generator,
            };
            // Keeping the schema and public bytes identical models a faulty
            // or malicious producer; it cannot advertise a replacement AIR.
            assert_eq!(producer.schema(), intended.schema());
            assert_eq!(producer.statement_bytes(), intended.statement_bytes());
            let trace = (0..TRACE_ROWS)
                .map(|index| field_pow(trace_generator, (index * exponent) as u64))
                .collect::<Vec<_>>();
            assert_ne!(trace[0], trace[1], "each probe must be nonconstant");
            let mut columns = vec![vec![0; TRACE_ROWS]; 342];
            columns[0] = trace;
            let prepared = prepare_trace(&producer, &columns).unwrap();
            drop(columns);
            let compact = prove_prepared(&producer, &prepared).unwrap();
            let shared = from_compact(&producer, &compact, conversion_limits()).unwrap();
            // Each exponent is below N. The deliberately wrong recurrence is
            // satisfied by its own monomial, so even its full FRI degree check
            // passes; the intended next-row relation must be what rejects it.
            let producer_work = verify_shared(&producer, &shared, diagnostic_limits()).unwrap();
            assert_eq!(producer_work.terminal_degree_checks, 1);
            assert_eq!(producer_work.air_evaluations, 375);
            assert!((375..=750).contains(&shared.rows.len()));
            for index in shared.rows.iter().map(|row| row.index as usize) {
                let row = shared_row(&shared.rows, index).unwrap();
                assert_eq!(row.len(), 342);
                assert_eq!(row[0], field_pow(domain.point(index), exponent as u64));
                assert!(row[1..].iter().all(|&value| value == 0));
            }
            let mut work = SharedVerificationWork::default();
            let result = verify_shared_recorded(&intended, &shared, diagnostic_limits(), &mut work);
            match mapping {
                NextMapping::Correct => {
                    result.unwrap();
                    assert_eq!(work, producer_work);
                }
                NextMapping::StrideOne | NextMapping::Swapped => {
                    assert!(
                        matches!(&result, Err(Error::AirOpeningMismatch { .. })),
                        "mapping={mapping:?}, result={result:?}"
                    );
                    assert_eq!(work.row_leaves, producer_work.row_leaves);
                    assert_eq!(work.oracle_leaves, producer_work.oracle_leaves);
                    assert_eq!(work.fri_leaves, producer_work.fri_leaves);
                    assert_eq!(work.parent_hashes, producer_work.parent_hashes);
                    assert!(work.air_evaluations > 0);
                    assert_eq!(work.terminal_degree_checks, 0);

                    // The AIR probe above retained physical wire openings.
                    // Also model a producer exporting its wrong mapping: every
                    // replacement is a real committed row with a genuine path.
                    // Its path authenticates at the mapped coordinate, while
                    // the intended current/next coordinate must reject it.
                    let mut remapped = compact.clone();
                    let remap = |values: &mut Vec<u64>,
                                 path: &mut Vec<WireDigest>,
                                 intended_index,
                                 mapped_index| {
                        *values = prepared
                            .columns
                            .iter()
                            .map(|column| column[mapped_index])
                            .collect();
                        *path = prepared.rows.path(mapped_index).unwrap();
                        let native_path = path
                            .iter()
                            .copied()
                            .map(WireDigest::as_fastpq)
                            .collect::<Vec<_>>();
                        let plan = |index| {
                            MultiproofPlan::new(
                                LDE_ROWS,
                                &[index],
                                MultiproofLimits {
                                    max_depth: 19,
                                    max_queried_leaves: 1,
                                    max_siblings: 19,
                                    max_parent_hashes: 19,
                                },
                            )
                            .unwrap()
                        };
                        prepared
                            .binding
                            .verify_tree(
                                &plan(mapped_index),
                                MerkleTreeRoleV1::AirTrace,
                                prepared.rows.root(),
                                &[prepared.binding.row(mapped_index, values).unwrap()],
                                &native_path,
                            )
                            .unwrap();
                        assert!(
                            prepared
                                .binding
                                .verify_tree(
                                    &plan(intended_index),
                                    MerkleTreeRoleV1::AirTrace,
                                    prepared.rows.root(),
                                    &[prepared.binding.row(intended_index, values).unwrap()],
                                    &native_path
                                )
                                .is_err()
                        );
                    };
                    for query in &mut remapped.queries {
                        let index = query.index as usize;
                        let physical_next = (index + 8) % LDE_ROWS;
                        match mapping {
                            NextMapping::StrideOne => remap(
                                &mut query.next,
                                &mut query.next_path,
                                physical_next,
                                (index + 1) % LDE_ROWS,
                            ),
                            NextMapping::Swapped => {
                                remap(
                                    &mut query.current,
                                    &mut query.current_path,
                                    index,
                                    physical_next,
                                );
                                remap(&mut query.next, &mut query.next_path, physical_next, index);
                            }
                            NextMapping::Correct => unreachable!(),
                        }
                    }
                    assert!(from_compact(&producer, &remapped, conversion_limits()).is_err());
                }
            }
        }
    }

    #[test]
    fn final_domain_retains_sole_terminal_duplicate_node_and_complete_values() {
        let relation = air();
        let proof = &fixture().shared;
        assert_eq!(proof.terminal_values.len(), 4);
        assert_eq!(proof.rounds.len(), 17);
        let geometry = Geometry::new(&relation).unwrap();
        let binding = Binding::new(&relation, &geometry).unwrap();
        let work = verify_shared(&relation, proof, diagnostic_limits()).unwrap();
        assert_eq!(work.air_evaluations, 375);
        assert_eq!(work.terminal_degree_checks, 1);
        let leaf = binding.fri(17, 0, &proof.terminal_values).unwrap();
        let root = binding
            .parent(MerkleTreeRoleV1::Fri(17), 1, 0, leaf, leaf)
            .unwrap();
        assert_eq!(root, proof.fri_roots[17].as_fastpq());
        assert_ne!(leaf, root);
        // Full-coverage empty-frontier law is a plan invariant independent of
        // an accepted proof geometry; preserve it without a small proof path.
        let full = MultiproofPlan::new(
            8,
            &(0..8).collect::<Vec<_>>(),
            MultiproofLimits {
                max_depth: 3,
                max_queried_leaves: 8,
                max_siblings: 0,
                max_parent_hashes: 7,
            },
        )
        .unwrap();
        assert_eq!(full.work().siblings, 0);
    }

    #[test]
    fn canonical_norito_roundtrip_and_every_ambient_layout_preserve_shared_verification() {
        let proof = &fixture().shared;
        let canonical = || {
            let _flags =
                norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
            norito::core::to_bytes(proof).unwrap()
        };
        let expected = canonical();
        let archived = norito::core::from_bytes::<SharedProof>(&expected).unwrap();
        let decoded = SharedProof::try_deserialize(archived).unwrap();
        assert_eq!(&decoded, proof);
        for flags in
            (u8::MIN..=u8::MAX).filter(|&flags| norito::core::validate_header_flags(flags).is_ok())
        {
            let _ambient = norito::core::DecodeFlagsGuard::enter(flags);
            let work = verify_shared(&air(), &decoded, diagnostic_limits()).unwrap();
            assert_eq!(work.proof_bytes, expected.len());
            assert_eq!(canonical(), expected);
            assert_eq!(norito::core::get_decode_flags(), flags);
        }
    }
}

#[cfg(test)]
#[path = "shared_openings/compact_diagnostic.rs"]
mod compact_diagnostic;
