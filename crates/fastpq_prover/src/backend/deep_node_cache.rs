//! Per-attempt internal Merkle nodes with exact geometry, context and opening binding.
//!
//! Only internal digests survive the first commitment traversal. After transcript
//! queries, the same masked polynomials regenerate queried leaves and leaf-level
//! siblings. The original canonical multiproof reconstructs the committed root
//! before any opening leaves this owner. No cache is shared between attempts.

use fastpq_isi::GoldilocksDigest384V1 as Digest;
use zeroize::Zeroize;

use super::{
    deep_binding::{Context, Oracle},
    deep_geometry::{LDE_ROWS, QUERY_COUNT},
    masked_quotient::{checked_add as add, checked_mul as mul},
    merkle_multiproof::{MultiproofLimits, MultiproofPlan},
    secret_polynomial::SecretPolynomial,
};
use crate::{DigestExecutionV1, Error, Result};

const MAX_LEAVES: usize = 2 * QUERY_COUNT;
const MAX_SIBLINGS: usize = QUERY_COUNT * 23;

/// Fixed oracle geometry; all arithmetic precedes allocation or private writes.
#[derive(Clone, Copy)]
pub(super) struct NodeCachePlan {
    oracle: Oracle,
    leaves: usize,
    nodes: usize,
    pub(super) payload_bytes: usize,
    pub(super) work_units: usize,
}
impl NodeCachePlan {
    pub(super) fn new(oracle: Oracle) -> Result<Self> {
        let (_, _, leaves, _) = oracle.shape().map_err(binding_error)?;
        if oracle == Oracle::Terminal {
            return Err(invalid("terminal values are retained without a node cache"));
        }
        Self::with_shape(oracle, leaves)
    }
    fn with_shape(oracle: Oracle, leaves: usize) -> Result<Self> {
        if !leaves.is_power_of_two() || leaves > LDE_ROWS {
            return Err(invalid(
                "internal-node cache requires bounded binary geometry",
            ));
        }
        let nodes = leaves.saturating_sub(1).max(1);
        let payload_bytes = add(mul(nodes, 48)?, mul(nodes.div_ceil(64), 8)?)?;
        // Initialization, canonical/coverage checks, insertion and erasure. The
        // old replay work remains charged separately as a conservative bound.
        let work_units = mul(payload_bytes, 8)?;
        Ok(Self {
            oracle,
            leaves,
            nodes,
            payload_bytes,
            work_units,
        })
    }
    pub(super) fn start(self, binding: &Context) -> Result<PendingNodes> {
        Ok(PendingNodes {
            plan: self,
            binding: binding.clone(),
            nodes: ClearingNodes(SecretPolynomial::zeroed(self.nodes)?),
            coverage: SecretPolynomial::zeroed(self.nodes.div_ceil(64))?,
            count: 0,
            failed: false,
        })
    }
    fn slot(self, level: usize, index: usize) -> Result<usize> {
        if self.leaves == 1 {
            return if level == 1 && index == 0 {
                Ok(0)
            } else {
                Err(invalid("cached singleton coordinate differs"))
            };
        }
        if level == 0 || level > self.leaves.ilog2() as usize || index >= self.leaves >> level {
            return Err(invalid(
                "cached node coordinate differs from fixed geometry",
            ));
        }
        Ok(self.leaves - (self.leaves >> (level - 1)) + index)
    }
}

/// Additional opening scratch, conservatively summed with the previous phase
/// plan. Includes both verifier frontiers and all public plan/index allocations.
pub(super) fn opening_payload_bytes(oracle: Oracle) -> Result<usize> {
    let (_, _, _, leaf_bytes) = oracle.shape().map_err(binding_error)?;
    add(
        mul(MAX_LEAVES + QUERY_COUNT, leaf_bytes)?,
        add(
            mul(MAX_LEAVES + QUERY_COUNT + 2 * MAX_SIBLINGS, 48)?,
            // Canonical plan, selected indices and two reconstruction frontiers;
            // Vec capacities are bounded by explicit reservations/the fixed plan.
            mul(8 * MAX_SIBLINGS + 8 * MAX_LEAVES, size_of::<usize>() + 48)?,
        )?,
    )
}

/// Fixed private-derived digest storage; no Clone or Debug exposure.
struct ClearingNodes(SecretPolynomial<[u64; 6]>);
impl Drop for ClearingNodes {
    fn drop(&mut self) {
        self.0.iter_mut().for_each(Zeroize::zeroize);
        #[cfg(test)]
        ERASURES.with(|observed| {
            let (cells, bad) = observed.get();
            observed.set((
                cells + self.0.len(),
                bad + self.0.iter().filter(|v| **v != [0; 6]).count(),
            ));
        });
    }
}
#[cfg(test)]
thread_local! {
    static ERASURES: core::cell::Cell<(usize, usize)> = const { core::cell::Cell::new((0, 0)) };
}

/// Guarded opening payload, including callback failure and unwinding.
struct ClearingPayload(SecretPolynomial<u8>);
impl ClearingPayload {
    fn zeroed(bytes: usize) -> Result<Self> {
        Ok(Self(SecretPolynomial::zeroed(bytes)?))
    }
}
impl core::ops::Deref for ClearingPayload {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        &self.0
    }
}
impl core::ops::DerefMut for ClearingPayload {
    fn deref_mut(&mut self) -> &mut [u8] {
        &mut self.0
    }
}
impl Drop for ClearingPayload {
    fn drop(&mut self) {
        self.0.zeroize();
        #[cfg(test)]
        PAYLOAD_ERASURES.with(|observed| {
            let (bytes, bad) = observed.get();
            observed.set((
                bytes + self.0.len(),
                bad + self.0.iter().filter(|v| **v != 0).count(),
            ));
        });
    }
}
#[cfg(test)]
thread_local! { static PAYLOAD_ERASURES: core::cell::Cell<(usize, usize)> = const { core::cell::Cell::new((0, 0)) }; }

/// Poisoned on every rejected write, including after complete coverage.
pub(super) struct PendingNodes {
    binding: Context,
    plan: NodeCachePlan,
    nodes: ClearingNodes,
    coverage: SecretPolynomial<u64>,
    count: usize,
    failed: bool,
}
impl PendingNodes {
    pub(super) fn leaves(&self) -> usize {
        self.plan.leaves
    }
    pub(super) fn record(&mut self, level: usize, index: usize, value: Digest) -> Result<()> {
        if self.failed {
            return Err(invalid("internal-node cache is poisoned"));
        }
        self.failed = true;
        let slot = self.plan.slot(level, index)?;
        let bit = 1_u64 << (slot % 64);
        if self.coverage[slot / 64] & bit != 0 {
            return Err(invalid("internal-node cache received a duplicate write"));
        }
        self.coverage[slot / 64] |= bit;
        self.nodes.0[slot] = value.words();
        self.count += 1;
        self.failed = false;
        Ok(())
    }
    pub(super) fn finish(self, root: Digest) -> Result<CompletedNodes> {
        if self.failed
            || self.count != self.plan.nodes
            || self.nodes.0[self.plan.nodes - 1] != root.words()
        {
            return Err(invalid(
                "internal-node cache is incomplete or has another root",
            ));
        }
        Ok(CompletedNodes {
            plan: self.plan,
            nodes: self.nodes,
            root,
            binding: self.binding,
        })
    }
}

/// Complete immutable nodes; bind the original context before opening them.
pub(super) struct CompletedNodes {
    binding: Context,
    plan: NodeCachePlan,
    nodes: ClearingNodes,
    root: Digest,
}
impl CompletedNodes {
    pub(super) fn bind(
        self,
        binding: &Context,
        oracle: Oracle,
        root: Digest,
    ) -> Result<CommittedNodes<'_>> {
        if !self.binding.same_attempt(binding)
            || self.plan.oracle != oracle
            || self.root != root
            || oracle.shape().map_err(binding_error)?.2 != self.plan.leaves
        {
            return Err(invalid("internal-node cache differs from committed oracle"));
        }
        Ok(CommittedNodes {
            nodes: self,
            binding,
        })
    }
}

/// Borrows precisely the immutable attempt context; openings cannot supply a
/// replacement context, oracle identity or root.
pub(super) struct CommittedNodes<'a> {
    nodes: CompletedNodes,
    binding: &'a Context,
}
impl CommittedNodes<'_> {
    pub(super) fn oracle(&self) -> Oracle {
        self.nodes.plan.oracle
    }
    pub(super) fn open(
        self,
        queries: &[usize],
        execution: DigestExecutionV1,
        regenerate: impl FnOnce(&[usize], &mut [u8]) -> Result<()>,
    ) -> Result<CachedOpening> {
        let plan = MultiproofPlan::new(
            self.nodes.plan.leaves,
            queries,
            MultiproofLimits {
                max_depth: 23,
                max_queried_leaves: QUERY_COUNT,
                max_siblings: MAX_SIBLINGS,
                max_parent_hashes: MAX_SIBLINGS,
            },
        )?;
        let mut selected = Vec::new();
        selected
            .try_reserve_exact(MAX_LEAVES)
            .map_err(|_| invalid("cache leaf-index allocation failed"))?;
        selected.extend_from_slice(queries);
        selected.extend(
            plan.sibling_positions()
                .iter()
                .filter(|p| p.level == 0)
                .map(|p| p.index),
        );
        selected.sort_unstable();
        selected.dedup();
        if selected.len() > MAX_LEAVES {
            return Err(invalid("cache selected-leaf budget exceeded"));
        }
        let oracle = self.nodes.plan.oracle;
        let leaf_bytes = oracle.shape().map_err(binding_error)?.3;
        let mut payloads = ClearingPayload::zeroed(mul(selected.len(), leaf_bytes)?)?;
        regenerate(&selected, &mut payloads)?;
        let mut hashes = SecretPolynomial::zeroed(selected.len())?;
        super::deep_leaf_batch::hash(
            self.binding,
            oracle,
            &selected,
            &payloads,
            leaf_bytes,
            &mut hashes,
            execution,
        )?;
        let mut leaves = SecretPolynomial::<Digest>::zeroed(queries.len())?;
        let mut siblings = SecretPolynomial::<Digest>::zeroed(plan.sibling_positions().len())?;
        for (&index, value) in queries.iter().zip(leaves.iter_mut()) {
            *value = digest(
                hashes[selected
                    .binary_search(&index)
                    .expect("queried leaf included")],
            );
        }
        for (position, value) in plan.sibling_positions().iter().zip(siblings.iter_mut()) {
            *value = if position.level == 0 {
                digest(
                    hashes[selected
                        .binary_search(&position.index)
                        .expect("leaf sibling included")],
                )
            } else {
                digest(self.nodes.nodes.0[self.nodes.plan.slot(position.level, position.index)?])
            };
        }
        plan.verify_with(
            self.nodes.root,
            &leaves,
            &siblings,
            |level, index, left, right| {
                self.binding
                    .hash_parent(oracle, level as u32, index as u32, left, right)
                    .map_err(binding_error)
            },
        )?;
        let mut values = ClearingPayload::zeroed(mul(queries.len(), leaf_bytes)?)?;
        for (&index, output) in queries.iter().zip(values.chunks_exact_mut(leaf_bytes)) {
            let at = selected
                .binary_search(&index)
                .expect("queried leaf included")
                * leaf_bytes;
            output.copy_from_slice(&payloads[at..at + leaf_bytes]);
        }
        Ok(CachedOpening {
            root: self.nodes.root,
            siblings: siblings.to_vec(),
            values,
            leaf_bytes,
        })
    }
}

/// Only authenticated disclosures survive the consumed cache owner.
pub(super) struct CachedOpening {
    pub(super) root: Digest,
    pub(super) siblings: Vec<Digest>,
    values: ClearingPayload,
    leaf_bytes: usize,
}
impl CachedOpening {
    pub(super) fn values(&self) -> impl Iterator<Item = &[u8]> {
        self.values.chunks_exact(self.leaf_bytes)
    }
}
fn digest(words: [u64; 6]) -> Digest {
    Digest::new(words).expect("cache stores canonical digest outputs")
}
fn binding_error(error: super::deep_binding::BindingError) -> Error {
    Error::InvalidTraceShape {
        details: format!("DEEP node cache: {error}"),
    }
}
fn invalid(details: &'static str) -> Error {
    Error::InvalidTraceShape {
        details: details.to_owned(),
    }
}

#[cfg(test)]
#[path = "deep_node_cache/tests.rs"]
mod tests;
