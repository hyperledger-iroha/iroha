//! Descriptor-derived field-buffer envelope of the owned-witness prover.
//!
//! A proof peaks while the quotient recombines its `d - 1` residue cosets ([`super::quotient`]).
//! It then holds the key, every column the opening reads (the advice and instance coefficients,
//! each permutation product, each lookup's committed `A'`, `S'` and `z`, and `R`), the quotient
//! workspace, the three coset masks, and the residues beside their recombined coefficients.
//!
//! The earlier core proving phases hold less. Advice commitment holds the evaluation columns
//! in place of the coefficients. The lookup and permutation commitments add, at most, both
//! instance forms, a fourth column per lookup and three sorting or product columns, fewer than
//! the workspace's own instance, lookup and coset-power columns plus the masks and residues
//! they do not hold. The opening holds the quotient pieces and a few polynomial and generator
//! columns in place of the workspace, masks and residues.
//!
//! A descriptor cannot bound a circuit's own synthesis allocations or the frontend's pending
//! `Assigned::Rational` records and denominator buffers. Callers must account for those
//! separately for their source. `tests/proving_memory.rs` measures synthesis plus proving for
//! one circuit that assigns evaluated field values; it is not a generic synthesis bound.
//!
//! The pinned parameters and the MSM scratch are separate byte budgets that callers add.

use super::{ProverError, quotient};
use crate::{
    cs::ProtocolDescriptor,
    protocol::{Protocol, ProtocolError},
};

/// Field columns of `n` elements budgeted for the core owned-witness prover, for a key
/// that keeps no coset cache.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProvingColumns {
    /// The proving key: its fixed, `sigma` and three mask columns, each held in both its
    /// evaluation and its coefficient form, plus both FFT twiddle directions rounded up
    /// together to whole columns.
    pub key: usize,
    /// Core proving buffers on top of the key; source-specific synthesis needs a separate bound.
    pub proof: usize,
}

impl ProvingColumns {
    /// Every column held at the peak.
    #[must_use]
    pub fn total(self) -> usize {
        self.key + self.proof
    }
}

/// The core proving field-buffer envelope of `descriptor`, whose quotient holds `quotient_columns`
/// coset columns at once, as in [`super::ProverConfig::quotient_columns`] (`None` is the fused
/// plan). Circuit-owned synthesis work and pending rational assignments are not included.
///
/// # Errors
///
/// A malformed descriptor expression or an overflowing shape.
pub fn peak_proving_columns(
    descriptor: &ProtocolDescriptor,
    quotient_columns: Option<usize>,
) -> Result<ProvingColumns, ProverError> {
    let protocol = Protocol::new(descriptor)?;
    let shape = protocol.shape();
    let overflow = || ProverError::from(ProtocolError::Overflow);
    let key = shape
        .num_fixed
        .checked_add(shape.permutation_columns)
        .and_then(|columns| columns.checked_add(3))
        .and_then(|columns| columns.checked_mul(2))
        // The key owns the forward and inverse twiddles: 2 * (n - 1) fields.
        // Round their combined storage up to whole columns (one only when n = 2).
        .and_then(|columns| columns.checked_add(2 - usize::from(shape.n == 2)))
        .ok_or_else(overflow)?;
    let workspace = match quotient_columns {
        None => quotient::fused_workspace_columns(shape, false)?,
        Some(limit) => quotient::bounded::bounded_quotient_columns(descriptor, limit)?,
    };
    // Advice and instance coefficients, permutation products, each lookup's `A'`, `S'` and
    // `z`, `R`, the workspace, the coset masks, then the residues and the recombined `h`.
    let proof = [
        shape.num_advice,
        shape.num_instance,
        shape.permutation_sets,
        shape.lookups.checked_mul(3).ok_or_else(overflow)?,
        1,
        workspace,
        3,
        shape.quotient_pieces.checked_mul(2).ok_or_else(overflow)?,
    ]
    .into_iter()
    .try_fold(0_usize, usize::checked_add)
    .ok_or_else(overflow)?;
    Ok(ProvingColumns { key, proof })
}
