//! Existing typed quotient/FRI/terminal commitments from bounded coefficient replay.
//!
//! This owner streams canonical leaves and the exact natural-order Merkle tree,
//! then retains only selected values and the existing minimal frontier. The
//! terminal retains all 128 values. No full oracle or full digest tree is stored.
//! The test-only candidate producer consumes this owner for roots and openings.
//! TODO: Execute and cryptographically qualify the complete construction before
//! admission; this owner computes commitments, not proof acceptance.

use super::{
    deep_binding::{BindingError, Context, Oracle},
    deep_coefficient_replay::{CoefficientReplay, CoefficientReplayPlan},
    deep_geometry::{FRI_ARITIES, FRI_DEGREES, FRI_LENGTHS, QUERY_COUNT},
    deep_striped_merkle::{StreamLimits, StripedMerklePlan},
    secret_polynomial::SecretPolynomial,
};
use crate::{Error, Result, field::GoldilocksFp4V1 as F};
use fastpq_isi::GoldilocksDigest384V1 as Digest;

/// Exact oracle identity, source geometry and checked active-phase payload bound.
pub(super) struct CoefficientCommitmentPlan<'a> {
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
        let hashing = if terminal {
            add(
                mul(fields, F::BYTES)?,
                binding.tree_frame_bytes(oracle).map_err(binding_error)?,
            )?
        } else {
            super::deep_leaf_batch::payload_bytes(binding, oracle, mul(fields, F::BYTES)?)?
        };
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
    pub(super) fn build(
        self,
        replay: &mut CoefficientReplay<'_>,
        binding: &Context,
    ) -> Result<CoefficientCommitment> {
        if replay.plan() != self.replay {
            return Err(invalid(
                "DEEP coefficient commitment replay differs from plan",
            ));
        }
        replay.ensure_pass_available()?;
        let mut tree = self.tree.start()?;
        let mut values = SecretPolynomial::zeroed(self.fields)?;
        let capacity = if self.oracle == Oracle::Terminal {
            1
        } else {
            super::deep_leaf_batch::CAPACITY
        };
        let mut bytes = SecretPolynomial::zeroed(capacity * self.fields * F::BYTES)?;
        // Terminal hashes directly; nonterminal batch digest storage is included
        // in the shared hashing plan before either private allocation is made.
        let mut leaves =
            SecretPolynomial::<[u64; 6]>::zeroed(if self.oracle == Oracle::Terminal {
                0
            } else {
                capacity
            })?;
        let mut selected = SecretPolynomial::zeroed(self.retained)?;
        let parent = |level: usize, index: usize, left, right| {
            binding
                .hash_parent(self.oracle, level as u32, index as u32, left, right)
                .map_err(binding_error)
        };
        if self.oracle == Oracle::Terminal {
            replay.visit_all(|stripe| {
                for row in 0..stripe.rows() {
                    selected[stripe.global_index(row)] = stripe.value(0, row)?;
                }
                Ok(())
            })?;
            pack(&selected, &mut bytes)?;
            let leaf = binding
                .hash_leaf(self.oracle, 0, &bytes)
                .map_err(binding_error)?;
            tree.push(0, leaf, parent)?;
        } else {
            replay.visit_all(|stripe| {
                let rows = if self.oracle == Oracle::QuotientAndMask {
                    stripe.rows()
                } else {
                    stripe.fiber_rows()
                };
                for start in (0..rows).step_by(capacity) {
                    let count = (rows - start).min(capacity);
                    let mut indices = [0; super::deep_leaf_batch::CAPACITY];
                    for (offset, packed) in bytes[..count * self.fields * F::BYTES]
                        .chunks_exact_mut(self.fields * F::BYTES)
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
                            stripe.fiber(row, &mut values)?;
                        }
                        pack(&values, packed)?;
                        if let Ok(position) = self.queries.binary_search(&index) {
                            selected[position * self.fields..(position + 1) * self.fields]
                                .copy_from_slice(&values);
                        }
                    }
                    super::deep_leaf_batch::hash(
                        binding,
                        self.oracle,
                        &indices[..count],
                        &bytes[..count * self.fields * F::BYTES],
                        self.fields * F::BYTES,
                        &mut leaves[..count],
                    )?;
                    for (&index, &words) in indices[..count].iter().zip(leaves[..count].iter()) {
                        tree.push(
                            index,
                            Digest::new(words).expect("canonical leaf hash"),
                            parent,
                        )?;
                    }
                }
                Ok(())
            })?;
        }
        let result = tree.finish(parent)?;
        Ok(CoefficientCommitment {
            root: result.root,
            siblings: result.siblings,
            selected,
            fields: self.fields,
        })
    }
}

/// Public commitment/frontier, with retained field values under clearing ownership.
pub(super) struct CoefficientCommitment {
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
