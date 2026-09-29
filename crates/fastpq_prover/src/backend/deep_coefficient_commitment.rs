//! Existing typed quotient/FRI/terminal commitments from bounded coefficient replay.
//!
//! This owner streams canonical leaves and the exact natural-order Merkle tree,
//! retaining internal Merkle nodes until transcript-selected openings regenerate
//! their leaves and authenticate the existing minimal frontier. The terminal
//! retains all 128 values. No full oracle or leaf-digest array is stored.
//! The producer consumes this owner for roots and openings.
//! TODO: Cryptographically qualify the complete construction; this owner computes
//! commitments, while the independent engine verifies the complete proof.

use super::{
    deep_binding::{BindingError, Context, Oracle},
    deep_coefficient_replay::{CoefficientReplay, CoefficientReplayPlan, CoefficientStripe},
    deep_geometry::{FRI_ARITIES, FRI_DEGREES, FRI_LENGTHS, QUERY_COUNT},
    deep_node_cache::{CommittedNodes, CompletedNodes, NodeCachePlan},
    deep_striped_merkle::{StreamLimits, StripedMerklePlan},
    secret_polynomial::SecretPolynomial,
};
use crate::{Error, Result, field::GoldilocksFp4V1 as F};
use fastpq_isi::GoldilocksDigest384V1 as Digest;

/// Exact oracle identity, source geometry and checked active-phase payload bound.
pub(super) struct CoefficientCommitmentPlan<'a> {
    digest_execution: crate::DigestExecutionV1,
    replay: CoefficientReplayPlan,
    oracle: Oracle,
    queries: &'a [usize],
    tree: StripedMerklePlan,
    fields: usize,
    retained: usize,
    pub(super) payload_bytes: usize,
    pub(super) leaf_hashes: usize,
    pub(super) parent_hashes: usize,
}
impl<'a> CoefficientCommitmentPlan<'a> {
    /// Public Context/cache and other retained phase owners are charged by the
    /// enclosing producer. This plan includes borrowed coefficients, one stripe,
    /// tree stacks/frontier, selected values, leaf packing and exact frame bytes.
    pub(super) fn new(
        replay: CoefficientReplayPlan,
        binding: &Context,
        oracle: Oracle,
        queries: &'a [usize],
        limits: StreamLimits,
    ) -> Result<Self> {
        let (rows, degree, width, arity, terminal) = match oracle {
            Oracle::QuotientAndMask => (FRI_LENGTHS[0], FRI_DEGREES[0], 3, 1, false),
            Oracle::Fri(round @ 0..=4) => {
                let r = usize::from(round);
                (FRI_LENGTHS[r], FRI_DEGREES[r], 1, FRI_ARITIES[r], false)
            }
            Oracle::Terminal => (FRI_LENGTHS[5], FRI_DEGREES[5], 1, 1, true),
            _ => {
                return Err(invalid(
                    "DEEP coefficient commitment requires an exact quotient or FRI oracle",
                ));
            }
        };
        if replay.rows() != rows
            || replay.degree() != degree
            || replay.width() != width
            || replay.arity() != arity
            || (oracle == Oracle::QuotientAndMask
                && !queries.is_empty()
                && queries.len() != QUERY_COUNT)
            || (terminal && !queries.is_empty())
        {
            return Err(invalid(
                "DEEP coefficient commitment source or query shape differs from fixed oracle",
            ));
        }
        let fields = if terminal { rows } else { width * arity };
        let retained = if terminal {
            rows
        } else {
            mul(queries.len(), fields)?
        };
        let tree = StripedMerklePlan::new(
            if terminal { 1 } else { rows / arity },
            if terminal { 1 } else { replay.stripes() },
            queries,
            limits,
        )?;
        let hashing =
            super::deep_leaf_batch::payload_bytes(binding, oracle, mul(fields, F::BYTES)?.max(96))?;
        let payload_bytes = add(
            replay.payload_bytes,
            add(
                tree.payload_bytes,
                add(mul(add(retained, fields)?, F::BYTES)?, hashing)?,
            )?,
        )?;
        limit(payload_bytes, limits.max_payload_bytes)?;
        let leaf_hashes = tree.leaf_hashes;
        let parent_hashes = tree.parent_hashes;
        Ok(Self {
            digest_execution: limits.digest_execution,
            replay,
            oracle,
            queries,
            tree,
            fields,
            retained,
            payload_bytes,
            leaf_hashes,
            parent_hashes,
        })
    }

    /// Consume one preplanned replay pass; failed traversals return no commitment.
    #[cfg(test)]
    pub(super) fn build(
        self,
        replay: &mut CoefficientReplay<'_>,
        binding: &Context,
    ) -> Result<CoefficientCommitment> {
        self.build_inner(replay, binding, false)
    }
    /// Root phase: retain each nonterminal tree's internal nodes once.
    pub(super) fn commit(
        self,
        replay: &mut CoefficientReplay<'_>,
        binding: &Context,
    ) -> Result<CoefficientCommitment> {
        if !self.queries.is_empty() {
            return Err(invalid("cached coefficient commitment requires root phase"));
        }
        self.build_inner(replay, binding, true)
    }
    fn build_inner(
        self,
        replay: &mut CoefficientReplay<'_>,
        binding: &Context,
        retain_nodes: bool,
    ) -> Result<CoefficientCommitment> {
        if replay.plan() != self.replay {
            return Err(invalid(
                "DEEP coefficient commitment replay differs from plan",
            ));
        }
        replay.ensure_pass_available()?;
        let mut tree = if retain_nodes && self.oracle != Oracle::Terminal {
            self.tree
                .start_cached(NodeCachePlan::new(self.oracle)?.start(binding)?)?
        } else {
            self.tree.start()?
        };
        let mut values = SecretPolynomial::zeroed(self.fields)?;
        let capacity = if self.oracle == Oracle::Terminal {
            1
        } else {
            super::deep_leaf_batch::CAPACITY
        };
        let mut bytes = SecretPolynomial::zeroed(capacity * self.fields * F::BYTES)?;
        // Every oracle uses the explicitly selected bulk-leaf executor. The
        // terminal retains its one-leaf duplicated-parent Merkle geometry.
        let mut leaves = SecretPolynomial::<[u64; 6]>::zeroed(capacity)?;
        let mut selected = SecretPolynomial::zeroed(self.retained)?;
        let parent = |level: usize, index: usize, left, right| {
            binding
                .hash_parent_at(self.oracle, level, index, left, right)
                .map_err(binding_error)
        };
        let batch_parent = |level, indices: &[usize], left: &[[u64; 6]], right: &mut [[u64; 6]]| {
            super::deep_parent_batch::hash_in_place(
                binding,
                self.oracle,
                level,
                indices,
                left,
                right,
                self.digest_execution,
            )
        };
        if self.oracle == Oracle::Terminal {
            gather_terminal(replay, &mut selected)?;
            pack(&selected, &mut bytes)?;
            super::deep_leaf_batch::hash(
                binding,
                self.oracle,
                &[0],
                &bytes,
                self.fields * F::BYTES,
                &mut leaves,
                self.digest_execution,
            )?;
            tree.push_batch(&[0], &mut leaves, batch_parent, parent)?;
        } else {
            let packer = RowPacker {
                oracle: self.oracle,
                fields: self.fields,
                queries: self.queries,
            };
            replay.visit_all(|stripe| {
                let rows = if self.oracle == Oracle::QuotientAndMask {
                    stripe.rows()
                } else {
                    stripe.fiber_rows()
                };
                for start in (0..rows).step_by(capacity) {
                    let count = (rows - start).min(capacity);
                    let mut indices = [0; super::deep_leaf_batch::CAPACITY];
                    packer.pack(
                        &stripe,
                        start,
                        &mut indices[..count],
                        &mut bytes,
                        &mut values,
                        &mut selected,
                    )?;
                    super::deep_leaf_batch::hash(
                        binding,
                        self.oracle,
                        &indices[..count],
                        &bytes[..count * self.fields * F::BYTES],
                        self.fields * F::BYTES,
                        &mut leaves[..count],
                        self.digest_execution,
                    )?;
                    tree.push_batch(
                        &indices[..count],
                        &mut leaves[..count],
                        batch_parent,
                        parent,
                    )?;
                }
                Ok(())
            })?;
        }
        let result = tree.finish(parent)?;
        Ok(CoefficientCommitment {
            cache: result.cache,
            root: result.root,
            siblings: result.siblings,
            selected,
            fields: self.fields,
        })
    }
}

/// Retain every terminal value at its natural-order position in `selected`.
fn gather_terminal(replay: &mut CoefficientReplay<'_>, selected: &mut [F]) -> Result<()> {
    replay.visit_all(|stripe| {
        for row in 0..stripe.rows() {
            selected[stripe.global_index(row)] = stripe.value(0, row)?;
        }
        Ok(())
    })
}

/// Leaf layout of one nonterminal oracle, copied out of its consumed plan.
struct RowPacker<'q> {
    oracle: Oracle,
    fields: usize,
    queries: &'q [usize],
}
impl RowPacker<'_> {
    /// Pack one bounded batch of consecutive stripe rows starting at `start`.
    ///
    /// Writes each row's tree index into `indices`, its canonical leaf into the
    /// matching `bytes` prefix and every queried row's fields into `selected`.
    fn pack(
        &self,
        stripe: &CoefficientStripe<'_>,
        start: usize,
        indices: &mut [usize],
        bytes: &mut [u8],
        values: &mut [F],
        selected: &mut [F],
    ) -> Result<()> {
        let leaf_bytes = self.fields * F::BYTES;
        for (offset, packed) in bytes[..indices.len() * leaf_bytes]
            .chunks_exact_mut(leaf_bytes)
            .enumerate()
        {
            let row = start + offset;
            let index = stripe.global_index(row);
            indices[offset] = index;
            if self.oracle == Oracle::QuotientAndMask {
                for (column, value) in values.iter_mut().enumerate() {
                    *value = stripe.value(column, row)?;
                }
            } else {
                stripe.fiber(row, values)?;
            }
            pack(values, packed)?;
            if let Ok(position) = self.queries.binary_search(&index) {
                selected[position * self.fields..(position + 1) * self.fields]
                    .copy_from_slice(values);
            }
        }
        Ok(())
    }
}

/// Public commitment/frontier, with retained field values under clearing ownership.
pub(super) struct CoefficientCommitment {
    pub(super) cache: Option<CompletedNodes>,
    pub(super) root: Digest,
    pub(super) siblings: Vec<Digest>,
    selected: SecretPolynomial<F>,
    fields: usize,
}
impl CoefficientCommitment {
    pub(super) fn openings(&self) -> impl Iterator<Item = &[F]> {
        self.selected.chunks_exact(self.fields)
    }
    pub(super) fn terminal(&self) -> Result<&[F]> {
        if self.fields != FRI_LENGTHS[5] || self.selected.len() != FRI_LENGTHS[5] {
            return Err(invalid(
                "DEEP coefficient commitment is not the complete terminal",
            ));
        }
        Ok(&self.selected)
    }
}
/// Regenerate complete selected quotient triples or FRI fibers, then authenticate
/// them with the original context-bound internal nodes before exposing fields.
pub(super) fn open_cached(
    cache: CommittedNodes<'_>,
    replay: &mut CoefficientReplay<'_>,
    queries: &[usize],
    execution: crate::DigestExecutionV1,
) -> Result<CoefficientCommitment> {
    let oracle = cache.oracle();
    let plan = replay.plan();
    let (_, _, leaves, leaf_bytes) = oracle.shape().map_err(binding_error)?;
    if oracle == Oracle::Row
        || oracle == Oracle::Terminal
        || leaves != plan.rows() / plan.arity()
        || leaf_bytes != plan.width() * plan.arity() * F::BYTES
    {
        return Err(invalid(
            "cached coefficient source differs from exact oracle",
        ));
    }
    let fields = leaf_bytes / F::BYTES;
    let opened = cache.open(queries, execution, |indices, output| {
        let mut values = SecretPolynomial::zeroed(fields)?;
        replay.visit_selected_stripes(indices, |stripe| {
            for (&index, target) in indices.iter().zip(output.chunks_exact_mut(leaf_bytes)) {
                if index % plan.stripes() != stripe.stripe_index() {
                    continue;
                }
                let row = index / plan.stripes();
                if oracle == Oracle::QuotientAndMask {
                    for (column, value) in values.iter_mut().enumerate() {
                        *value = stripe.value(column, row)?;
                    }
                } else {
                    stripe.fiber(row, &mut values)?;
                }
                pack(&values, target)?;
            }
            Ok(())
        })
    })?;
    let mut selected = SecretPolynomial::zeroed(queries.len() * fields)?;
    for (bytes, values) in opened.values().zip(selected.chunks_exact_mut(fields)) {
        for (encoded, value) in bytes.chunks_exact(F::BYTES).zip(values) {
            *value = F::from_le_bytes(encoded.try_into().expect("exact Fp4 bytes"))
                .ok_or_else(|| invalid("cached coefficient opening is noncanonical"))?;
        }
    }
    Ok(CoefficientCommitment {
        root: opened.root,
        siblings: opened.siblings,
        selected,
        fields,
        cache: None,
    })
}
fn pack(values: &[F], output: &mut [u8]) -> Result<()> {
    if output.len() != values.len() * F::BYTES {
        return Err(invalid(
            "DEEP coefficient leaf packing has wrong exact length",
        ));
    }
    for (&value, bytes) in values.iter().zip(output.chunks_exact_mut(F::BYTES)) {
        bytes.copy_from_slice(&value.to_le_bytes());
    }
    Ok(())
}
#[allow(
    clippy::needless_pass_by_value,
    reason = "`map_err` adapter; the sibling test module passes it point-free"
)]
fn binding_error(error: BindingError) -> Error {
    Error::InvalidTraceShape {
        details: format!("DEEP coefficient commitment: {error}"),
    }
}
fn add(a: usize, b: usize) -> Result<usize> {
    a.checked_add(b)
        .ok_or_else(|| invalid("DEEP coefficient commitment payload overflow"))
}
fn mul(a: usize, b: usize) -> Result<usize> {
    a.checked_mul(b)
        .ok_or_else(|| invalid("DEEP coefficient commitment payload overflow"))
}
fn limit(actual: usize, max: usize) -> Result<()> {
    if actual > max {
        Err(Error::VerifierLimitExceeded {
            limit: "max_deep_coefficient_commitment_payload_bytes",
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
#[path = "deep_coefficient_commitment/tests.rs"]
mod tests;
