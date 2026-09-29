//! Natural-order binary commitments from stripe-ordered leaf replay.
//!
//! Stripe s emits indices s+B*j. Maintain log2(B) pending subtree digests
//! per row j, then stream each completed B-leaf subtree into one upper stack.
//! For N=65536/B=128 this uses 7*N digest slots instead of a complete M-leaf
//! tree. Every leaf/parent keeps its natural coordinate and existing hash role.
//! First commitments retain bounded internal nodes; selected-leaf replay later
//! reconstructs the original canonical frontier without rehashing the full tree.
//! No full leaf-value oracle is retained. Digest owners are erased on drop.
//!
//! The producer binds each completed root into the typed DEEP transcript.
//! A computed commitment or frontier alone supplies no hiding or source authority.

use super::{
    deep_node_cache::{CommittedNodes, CompletedNodes, NodeCachePlan, PendingNodes},
    merkle_multiproof::{MultiproofLimits, MultiproofPlan, SiblingPosition},
    secret_polynomial::SecretPolynomial,
};
use crate::{Error, Result};
use fastpq_isi::GoldilocksDigest384V1 as Digest;

/// Local digest-stream budget, fixed before leaf replay.
#[derive(Clone, Copy, Debug)]
pub(super) struct StreamLimits {
    pub(super) digest_execution: crate::DigestExecutionV1,
    pub(super) max_payload_bytes: usize,
    pub(super) max_hashes: usize,
}

/// Exact fixed-buffer payload and hash counts, including frontier output copying.
pub(super) struct StripedMerklePlan {
    leaves: usize,
    stripes: usize,
    rows: usize,
    lower_levels: usize,
    upper_levels: usize,
    openings: Option<MultiproofPlan>,
    pub(super) payload_bytes: usize,
    pub(super) leaf_hashes: usize,
    pub(super) parent_hashes: usize,
}

impl StripedMerklePlan {
    /// At most the candidate's fixed M leaves/64 queries; no proof-selected shape.
    /// Smaller powers of two serve other fixed FRI layers and arithmetic tests.
    pub(super) fn new(
        leaves: usize,
        stripes: usize,
        queries: &[usize],
        limits: StreamLimits,
    ) -> Result<Self> {
        if !leaves.is_power_of_two()
            || !stripes.is_power_of_two()
            || stripes > leaves
            || leaves > super::deep_geometry::LDE_ROWS
            || queries.len() > super::deep_geometry::QUERY_COUNT
        {
            return Err(invalid(
                "striped commitment requires bounded exact tree dimensions",
            ));
        }
        let rows = leaves / stripes;
        let lower_levels = stripes.ilog2() as usize;
        let upper_levels = rows.ilog2() as usize + 1;
        let openings = if queries.is_empty() {
            None
        } else {
            Some(MultiproofPlan::new(
                leaves,
                queries,
                MultiproofLimits {
                    max_depth: 23,
                    max_queried_leaves: 64,
                    max_siblings: 64 * 23,
                    max_parent_hashes: 64 * 23,
                },
            )?)
        };
        let siblings = openings.as_ref().map_or(0, |plan| plan.work().siblings);
        let slots = add(mul(rows, lower_levels)?, upper_levels)?;
        // Stack digests; guarded frontier + final public frontier; plan-owned
        // query positions and sibling coordinates. Vec metadata/allocator/RSS
        // are excluded, matching the other construction payload plans.
        let payload_bytes = add(
            mul(add(slots, mul(2, siblings)?)?, 48)?,
            openings
                .as_ref()
                .map(MultiproofPlan::owned_payload_bytes)
                .transpose()?
                .unwrap_or(0),
        )?;
        let parent_hashes = leaves.saturating_sub(1).max(1);
        limit(
            "max_deep_stream_payload_bytes",
            payload_bytes,
            limits.max_payload_bytes,
        )?;
        limit(
            "max_deep_stream_hashes",
            add(leaves, parent_hashes)?,
            limits.max_hashes,
        )?;
        limit(
            "max_deep_stream_addressable_bytes",
            payload_bytes,
            isize::MAX as usize,
        )?;
        Ok(Self {
            leaves,
            stripes,
            rows,
            lower_levels,
            upper_levels,
            openings,
            payload_bytes,
            leaf_hashes: leaves,
            parent_hashes,
        })
    }

    pub(super) fn start(self) -> Result<StripedMerkle> {
        self.start_inner(None)
    }

    pub(super) fn start_cached(self, cache: PendingNodes) -> Result<StripedMerkle> {
        if cache.leaves() != self.leaves {
            return Err(invalid("cached tree shape differs from canonical stream"));
        }
        self.start_inner(Some(cache))
    }

    fn start_inner(self, cache: Option<PendingNodes>) -> Result<StripedMerkle> {
        let lower = SecretPolynomial::zeroed(self.rows * self.lower_levels)?;
        let upper = SecretPolynomial::zeroed(self.upper_levels)?;
        let siblings = SecretPolynomial::zeroed(
            self.openings
                .as_ref()
                .map_or(0, |plan| plan.work().siblings),
        )?;
        Ok(StripedMerkle {
            plan: self,
            lower,
            upper,
            siblings,
            seen: 0,
            captured: 0,
            parents: 0,
            failed: false,
            cache,
        })
    }
}

/// A single deterministic traversal; failed or incomplete streams cannot finish.
pub(super) struct StripedMerkle {
    plan: StripedMerklePlan,
    lower: SecretPolynomial<[u64; 6]>,
    upper: SecretPolynomial<[u64; 6]>,
    siblings: SecretPolynomial<[u64; 6]>,
    seen: usize,
    captured: usize,
    parents: usize,
    failed: bool,
    cache: Option<PendingNodes>,
}

/// Public commitment and canonical frontier after the entire traversal succeeds.
pub(super) struct StreamedCommitment {
    pub(super) cache: Option<CompletedNodes>,
    pub(super) root: Digest,
    pub(super) siblings: Vec<Digest>,
    #[cfg(test)]
    pub(super) leaf_hashes: usize,
    #[cfg(test)]
    pub(super) parent_hashes: usize,
}

impl StripedMerkle {
    /// Accept each natural index exactly once in the declared stripe order.
    /// The hash callback receives unchanged natural binary parent coordinates.
    #[cfg(test)]
    pub(super) fn push(
        &mut self,
        index: usize,
        leaf: Digest,
        mut hash: impl FnMut(usize, usize, Digest, Digest) -> Result<Digest>,
    ) -> Result<()> {
        if self.failed || self.seen >= self.plan.leaves {
            self.failed = true;
            return Err(invalid(
                "striped commitment stream is failed or already complete",
            ));
        }
        // Poison before callbacks; an error cannot leave a reusable partial tree.
        self.failed = true;
        let stripe = self.seen / self.plan.rows;
        let row = self.seen % self.plan.rows;
        if index != stripe + row * self.plan.stripes {
            return Err(invalid(
                "striped commitment leaf arrived outside canonical replay order",
            ));
        }
        let mut value = leaf;
        self.capture(0, index, value)?;
        let mut level = 0;
        while level < self.plan.lower_levels {
            let slot = level * self.plan.rows + row;
            if (stripe >> level) & 1 == 0 {
                self.lower[slot] = value.words();
                self.seen += 1;
                self.failed = false;
                return Ok(());
            }
            let left = digest(self.lower[slot]);
            self.lower[slot] = [0; 6];
            level += 1;
            value = hash(level, index >> level, left, value)?;
            self.parents += 1;
            self.capture(level, index >> level, value)?;
        }
        let mut upper = 0;
        while (row >> upper) & 1 == 1 {
            let left = digest(self.upper[upper]);
            self.upper[upper] = [0; 6];
            upper += 1;
            level += 1;
            value = hash(level, index >> level, left, value)?;
            self.parents += 1;
            self.capture(level, index >> level, value)?;
        }
        self.upper[upper] = value.words();
        self.seen += 1;
        self.failed = false;
        Ok(())
    }

    /// Merge a bounded run from one stripe. Lower parents at a given level are
    /// independent across rows and can be hashed together; the final upper stack
    /// retains its canonical row order. Caller-owned clearing leaf storage is
    /// reused for the intermediate parents. Any malformed run or callback error
    /// poisons this traversal before another root can be returned.
    pub(super) fn push_batch(
        &mut self,
        indices: &[usize],
        values: &mut [[u64; 6]],
        mut lower_hash: impl FnMut(usize, &[usize], &[[u64; 6]], &mut [[u64; 6]]) -> Result<()>,
        mut upper_hash: impl FnMut(usize, usize, Digest, Digest) -> Result<Digest>,
    ) -> Result<()> {
        use zeroize::Zeroize;

        if self.failed || self.seen >= self.plan.leaves {
            self.failed = true;
            return Err(invalid(
                "striped commitment stream is failed or already complete",
            ));
        }
        self.failed = true;
        let stripe = self.seen / self.plan.rows;
        let row = self.seen % self.plan.rows;
        if indices.is_empty()
            || indices.len() > super::deep_leaf_batch::CAPACITY
            || indices.len() != values.len()
            || indices.len() > self.plan.rows - row
            || indices
                .iter()
                .enumerate()
                .any(|(offset, &index)| index != stripe + (row + offset) * self.plan.stripes)
            || values.iter().any(|&words| Digest::new(words).is_none())
        {
            return Err(invalid(
                "striped commitment batch differs from canonical replay order",
            ));
        }
        for (&index, &words) in indices.iter().zip(values.iter()) {
            self.capture(0, index, digest(words))?;
        }
        let mut parent_indices = [0; super::deep_leaf_batch::CAPACITY];
        for level in 0..self.plan.lower_levels {
            let start = level * self.plan.rows + row;
            let end = start + values.len();
            if (stripe >> level) & 1 == 0 {
                self.lower[start..end].copy_from_slice(values);
                self.seen += values.len();
                self.failed = false;
                return Ok(());
            }
            for (parent, &index) in parent_indices.iter_mut().zip(indices) {
                *parent = index >> (level + 1);
            }
            lower_hash(
                level + 1,
                &parent_indices[..values.len()],
                &self.lower[start..end],
                values,
            )?;
            // The fixed allocation remains guarded on both success and failure.
            // Clear consumed slots now without shrinking the guarded owner.
            for consumed in &mut self.lower[start..end] {
                consumed.zeroize();
            }
            if values.iter().any(|&words| Digest::new(words).is_none()) {
                return Err(invalid(
                    "striped parent executor returned a noncanonical digest",
                ));
            }
            self.parents += values.len();
            for (&index, &words) in parent_indices.iter().zip(values.iter()) {
                self.capture(level + 1, index, digest(words))?;
            }
        }
        for (offset, (&index, words)) in indices.iter().zip(values.iter_mut()).enumerate() {
            let mut value = digest(*words);
            let mut upper = 0;
            let mut level = self.plan.lower_levels;
            while ((row + offset) >> upper) & 1 == 1 {
                let left = digest(self.upper[upper]);
                self.upper[upper] = [0; 6];
                upper += 1;
                level += 1;
                value = upper_hash(level, index >> level, left, value)?;
                self.parents += 1;
                self.capture(level, index >> level, value)?;
            }
            *words = value.words();
            self.upper[upper] = *words;
        }
        self.seen += values.len();
        self.failed = false;
        Ok(())
    }

    fn capture(&mut self, level: usize, index: usize, value: Digest) -> Result<()> {
        if level > 0
            && let Some(cache) = &mut self.cache
        {
            cache.record(level, index, value)?;
        }
        let Some(plan) = &self.plan.openings else {
            return Ok(());
        };
        if let Ok(position) = plan
            .sibling_positions()
            .binary_search(&SiblingPosition { level, index })
        {
            self.siblings[position] = value.words();
            self.captured += 1;
        }
        Ok(())
    }

    /// Preserve the existing duplicated sole-leaf parent rule and exact counts.
    pub(super) fn finish(
        mut self,
        mut hash: impl FnMut(usize, usize, Digest, Digest) -> Result<Digest>,
    ) -> Result<StreamedCommitment> {
        if self.failed || self.seen != self.plan.leaves || self.captured != self.siblings.len() {
            return Err(invalid(
                "striped commitment traversal or frontier is incomplete",
            ));
        }
        let mut root = digest(self.upper[self.plan.upper_levels - 1]);
        if self.plan.leaves == 1 {
            root = hash(1, 0, root, root)?;
            self.capture(1, 0, root)?;
            self.parents += 1;
        }
        if self.parents != self.plan.parent_hashes {
            return Err(invalid(
                "striped commitment parent count differs from its plan",
            ));
        }
        let mut siblings = Vec::new();
        siblings
            .try_reserve_exact(self.siblings.len())
            .map_err(|_| invalid("striped commitment frontier allocation failed"))?;
        siblings.extend(self.siblings.iter().copied().map(digest));
        let cache = self
            .cache
            .take()
            .map(|cache| cache.finish(root))
            .transpose()?;
        Ok(StreamedCommitment {
            cache,
            root,
            siblings,
            #[cfg(test)]
            leaf_hashes: self.seen,
            #[cfg(test)]
            parent_hashes: self.parents,
        })
    }
}

/// Shared phase plan for actual fixed DEEP row commitments/openings.
/// Public Context/cache storage is borrowed and belongs to the outer producer;
/// this owner charges its exact temporary canonical hash frame separately.
pub(super) struct RowCommitmentPlan<'a> {
    digest_execution: crate::DigestExecutionV1,
    replay: super::deep_masked_replay::MaskedReplayPlan,
    queries: &'a [usize],
    tree: StripedMerklePlan,
    pub(super) payload_bytes: usize,
}
impl<'a> RowCommitmentPlan<'a> {
    pub(super) fn new(
        replay: super::deep_masked_replay::MaskedReplayPlan,
        binding: &super::deep_binding::Context,
        queries: &'a [usize],
        limits: StreamLimits,
    ) -> Result<Self> {
        if !replay.is_candidate_geometry()
            || (!queries.is_empty() && queries.len() != super::deep_geometry::QUERY_COUNT)
        {
            return Err(invalid(
                "DEEP row commitment requires fixed replay geometry and complete queries",
            ));
        }
        let tree = StripedMerklePlan::new(replay.lde_rows(), replay.stripes(), queries, limits)?;
        let payload_bytes = row_payload(replay, binding, queries.len(), tree.payload_bytes)?;
        limit(
            "max_deep_row_commitment_payload_bytes",
            payload_bytes,
            limits.max_payload_bytes,
        )?;
        Ok(Self {
            digest_execution: limits.digest_execution,
            replay,
            queries,
            tree,
            payload_bytes,
        })
    }
    pub(super) fn commit(
        self,
        replay: &mut super::deep_masked_replay::MaskedTraceReplay,
        binding: &super::deep_binding::Context,
    ) -> Result<RowCommitment> {
        if !self.queries.is_empty()
            || replay.plan() != self.replay
            || !replay.has_candidate_geometry()
        {
            return Err(invalid(
                "cached row commitment requires its exact root phase",
            ));
        }
        replay.ensure_pass_available()?;
        let cache = NodeCachePlan::new(super::deep_binding::Oracle::Row)?.start(binding)?;
        stream_rows(
            replay,
            binding,
            self.queries,
            self.tree,
            self.digest_execution,
            Some(cache),
        )
    }
    #[cfg(test)]
    pub(super) fn build(
        self,
        replay: &mut super::deep_masked_replay::MaskedTraceReplay,
        binding: &super::deep_binding::Context,
    ) -> Result<RowCommitment> {
        if replay.plan() != self.replay || !replay.has_candidate_geometry() {
            return Err(invalid("DEEP row commitment replay differs from its plan"));
        }
        replay.ensure_pass_available()?;
        stream_rows(
            replay,
            binding,
            self.queries,
            self.tree,
            self.digest_execution,
            None,
        )
    }
}

/// Complete public row commitment and final selected-opening DTOs after success.
pub(super) struct RowCommitment {
    pub(super) cache: Option<CompletedNodes>,
    pub(super) root: Digest,
    pub(super) siblings: Vec<Digest>,
    pub(super) rows: Vec<super::deep_proof::RowOpening>,
}

fn row_payload(
    replay: super::deep_masked_replay::MaskedReplayPlan,
    binding: &super::deep_binding::Context,
    queries: usize,
    tree_bytes: usize,
) -> Result<usize> {
    use super::{compact_public_columns::COMMITTED_COLUMN_COUNT as WIDTH, deep_binding::Oracle};
    let batch = super::deep_leaf_batch::payload_bytes(binding, Oracle::Row, (WIDTH * 8).max(96))?;
    // Current scalar row, selected guarded rows + final row DTOs,
    // and one transient Vec consumed by RowValues::new during final conversion.
    let rows = add(
        mul(add(2, queries)?, WIDTH * 8)?,
        mul(queries, size_of::<super::deep_proof::RowOpening>())?,
    )?;
    add(replay.payload_bytes, add(tree_bytes, add(batch, rows)?)?)
}

fn stream_rows(
    replay: &mut super::deep_masked_replay::MaskedTraceReplay,
    binding: &super::deep_binding::Context,
    queries: &[usize],
    tree: StripedMerklePlan,
    execution: crate::DigestExecutionV1,
    cache: Option<PendingNodes>,
) -> Result<RowCommitment> {
    use super::{
        compact_public_columns::COMMITTED_COLUMN_COUNT as WIDTH,
        deep_binding::Oracle,
        deep_proof::{RowOpening, RowValues},
    };
    const BATCH: usize = super::deep_leaf_batch::CAPACITY;
    if replay.width() != WIDTH
        || tree.leaves != replay.plan().lde_rows()
        || tree.stripes != replay.plan().stripes()
    {
        return Err(invalid("DEEP row stream shape differs from replay"));
    }
    if tree
        .openings
        .as_ref()
        .map_or(&[][..], MultiproofPlan::queried_indices)
        != queries
    {
        return Err(invalid(
            "DEEP row frontier differs from selected row positions",
        ));
    }
    let mut stream = if let Some(cache) = cache {
        tree.start_cached(cache)?
    } else {
        tree.start()?
    };
    let mut row = SecretPolynomial::zeroed(WIDTH)?;
    let mut bytes = SecretPolynomial::zeroed(BATCH * WIDTH * 8)?;
    let mut leaves = SecretPolynomial::<[u64; 6]>::zeroed(BATCH)?;
    let mut selected = SecretPolynomial::zeroed(queries.len() * WIDTH)?;
    let parent = |level: usize, index: usize, left, right| {
        binding
            .hash_parent_at(Oracle::Row, level, index, left, right)
            .map_err(binding_error)
    };
    replay.visit_all(|stripe| {
        for start in (0..stripe.rows()).step_by(BATCH) {
            let count = (stripe.rows() - start).min(BATCH);
            let mut indices = [0; BATCH];
            for (offset, packed) in bytes[..count * WIDTH * 8]
                .chunks_exact_mut(WIDTH * 8)
                .enumerate()
            {
                let index = stripe.global_index(start + offset);
                indices[offset] = index;
                stripe.fill_row(start + offset, &mut row)?;
                for (&value, word) in row.iter().zip(packed.chunks_exact_mut(8)) {
                    word.copy_from_slice(&value.to_le_bytes());
                }
                if let Ok(position) = queries.binary_search(&index) {
                    selected[position * WIDTH..(position + 1) * WIDTH].copy_from_slice(&row);
                }
            }
            super::deep_leaf_batch::hash(
                binding,
                Oracle::Row,
                &indices[..count],
                &bytes[..count * WIDTH * 8],
                WIDTH * 8,
                &mut leaves[..count],
                execution,
            )?;
            stream.push_batch(
                &indices[..count],
                &mut leaves[..count],
                |level, indices, left, right| {
                    super::deep_parent_batch::hash_in_place(
                        binding,
                        Oracle::Row,
                        level,
                        indices,
                        left,
                        right,
                        execution,
                    )
                },
                parent,
            )?;
        }
        Ok(())
    })?;
    let finished = stream.finish(parent)?;
    let mut rows = Vec::new();
    rows.try_reserve_exact(queries.len())
        .map_err(|_| invalid("DEEP final row opening allocation failed"))?;
    for (&index, values) in queries.iter().zip(selected.chunks_exact(WIDTH)) {
        rows.push(RowOpening {
            index: u32::try_from(index)
                .map_err(|_| invalid("DEEP row opening index exceeds u32"))?,
            values: RowValues::new(values.to_vec())?,
        });
    }
    Ok(RowCommitment {
        cache: finished.cache,
        root: finished.root,
        siblings: finished.siblings,
        rows,
    })
}

/// Regenerate only queried rows and leaf siblings under the committed mask owner.
pub(super) fn open_cached_rows(
    cache: CommittedNodes<'_>,
    replay: &mut super::deep_masked_replay::MaskedTraceReplay,
    queries: &[usize],
    execution: crate::DigestExecutionV1,
) -> Result<RowCommitment> {
    use super::{
        compact_public_columns::COMMITTED_COLUMN_COUNT as WIDTH,
        deep_binding::Oracle,
        deep_proof::{RowOpening, RowValues},
    };
    if cache.oracle() != Oracle::Row
        || !replay.has_candidate_geometry()
        || queries.len() != super::deep_geometry::QUERY_COUNT
    {
        return Err(invalid("cached row openings differ from the exact profile"));
    }
    let opened = cache.open(queries, execution, |indices, output| {
        let mut row = SecretPolynomial::zeroed(WIDTH)?;
        replay.visit_selected_stripes(indices, |stripe| {
            for (&index, target) in indices.iter().zip(output.chunks_exact_mut(WIDTH * 8)) {
                if index % replay_stripes() == stripe.stripe_index() {
                    stripe.fill_row(index / replay_stripes(), &mut row)?;
                    for (&value, word) in row.iter().zip(target.chunks_exact_mut(8)) {
                        word.copy_from_slice(&value.to_le_bytes());
                    }
                }
            }
            Ok(())
        })
    })?;
    let mut rows = Vec::new();
    rows.try_reserve_exact(queries.len())
        .map_err(|_| invalid("cached row opening allocation failed"))?;
    for (&index, bytes) in queries.iter().zip(opened.values()) {
        let values = bytes
            .chunks_exact(8)
            .map(|word| u64::from_le_bytes(word.try_into().expect("exact row cell")))
            .collect::<Vec<_>>();
        rows.push(RowOpening {
            index: u32::try_from(index)
                .map_err(|_| invalid("cached row opening index exceeds u32"))?,
            values: RowValues::new(values)?,
        });
    }
    Ok(RowCommitment {
        root: opened.root,
        siblings: opened.siblings,
        rows,
        cache: None,
    })
}
fn replay_stripes() -> usize {
    super::deep_geometry::LDE_ROWS / super::deep_geometry::TRACE_ROWS
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "`map_err` adapter; the sibling test module passes it point-free"
)]
fn binding_error(error: super::deep_binding::BindingError) -> Error {
    Error::InvalidTraceShape {
        details: format!("DEEP streamed commitment: {error}"),
    }
}

fn digest(words: [u64; 6]) -> Digest {
    Digest::new(words).expect("digest scratch contains canonical hash outputs")
}
fn add(a: usize, b: usize) -> Result<usize> {
    a.checked_add(b)
        .ok_or_else(|| invalid("striped commitment resource overflow"))
}
fn mul(a: usize, b: usize) -> Result<usize> {
    a.checked_mul(b)
        .ok_or_else(|| invalid("striped commitment resource overflow"))
}
fn limit(name: &'static str, actual: usize, max: usize) -> Result<()> {
    if actual > max {
        Err(Error::VerifierLimitExceeded {
            limit: name,
            actual,
            max,
        })
    } else {
        Ok(())
    }
}
fn invalid(details: &'static str) -> Error {
    Error::InvalidTraceShape {
        details: details.to_owned(),
    }
}

#[cfg(test)]
#[path = "deep_striped_merkle/tests.rs"]
mod tests;
