//! Source-bound preflight for the inactive private DEEP producer candidate.
//!
//! This records the current vanishing-mask degree and byte obligations, and the
//! checked floor for a complete materialized base-field row LDE. It performs
//! no FFT, LDE, commitment, transcript step or proof construction. Every plan
//! carries a typed refusal: zero masks disclose private rows, extension-field
//! masks cannot use the base-field row wire, and base-field masks still lack
//! independent entropy, quotient blinding and a committed composition mask.
//!
//! TODO: Replace the mask/opening design with a reviewed hiding construction,
//! then account for its complete AIR, quotient, DEEP, FRI and external-memory
//! workspace before allowing any producer or production admission.

use core::mem::size_of;

use super::{
    coefficient_masking::MaskingShape,
    compact_protocol::FixedAir,
    compact_public_columns::{
        COMMITTED_COLUMN_COUNT, COMMITTED_COLUMNS, PUBLIC_COLUMNS, PUBLIC_POLYNOMIAL_DEGREE,
        SourceTraceColumns,
    },
    compact_transfer_air::CompactTransferAir,
    deep_geometry::{CONSTRAINTS, DeepGeometry, FRI_DEGREES, LDE_ROWS, TRACE_ROWS},
    deep_proof::{MAX_FRAME_BYTES, PROOF_BYTE_TARGET, QUERY_COUNT},
    polynomial_field::PolynomialField,
};
use crate::{Error, Result, field::GoldilocksFp4V1 as F, gadgets::compact_smt_air::COLUMN_COUNT};

/// Existing AXT inner-payload ceiling; the carrier and public context use part of it.
const AXT_INNER_PAYLOAD_CEILING_BYTES: usize = 1024 * 1024;
/// Largest mask coefficient extent admitted for this bounded diagnostic.
const MAX_MASK_COEFFICIENTS: usize = TRACE_ROWS;

/// Explicit cap for additional materialized row-LDE payload, excluding borrowed source.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct DeepProverLimits {
    /// Maximum bytes available for the complete base-field row LDE alone.
    pub(super) max_complete_row_lde_bytes: usize,
}

/// Reason no current mask can authorize a private proof in the fixed profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum DeepPrivateProofRefusal {
    /// The supplied mask exceeds even the screened doubled FRI degree.
    MaskExceedsFriDegree {
        /// Index in the ordered 301-column commitment.
        retained_column: usize,
        /// Corresponding original 342-column source index.
        source_column: usize,
        /// Exact exclusive degree of C + (X^N - 1)M from the highest mask term.
        trace_degree_bound: usize,
        /// Fixed exclusive degree bound of the first FRI layer.
        fri_degree_bound: usize,
    },
    /// The high quotient half cannot fit the doubled first FRI degree.
    QuotientExceedsFriDegree {
        /// Exclusive degree bound of the complete zero-remainder quotient.
        quotient_degree_bound: usize,
        /// Exclusive degree bound of the high half after the X^N split.
        high_half_degree_bound: usize,
        /// Fixed exclusive degree bound of the first FRI layer.
        fri_degree_bound: usize,
    },
    /// A non-base-field mask needs wider rows that exceed the byte ceiling.
    ExtensionMaskCannotUseBaseRows {
        /// Index in the ordered 301-column commitment.
        retained_column: usize,
        /// Corresponding original 342-column source index.
        source_column: usize,
    },
    /// At least one retained private column remains unmasked at opened rows.
    UnmaskedPrivateOpenings,
    /// Compatible degree alone cannot replace the missing hiding construction.
    MissingReviewedHidingAndComposition,
}

/// Checked arithmetic envelope for the fixed candidate, with no proving authority.
pub(super) struct DeepProverPlan {
    /// Additional payload bytes for one complete 301-column base-field row LDE.
    pub(super) complete_row_lde_floor_bytes: usize,
    /// Full 923-slot AIR mixture's exclusive numerator bound.
    pub(super) numerator_degree_bound: usize,
    /// Quotient bound conditional on checked zero remainder, not a quotient proof.
    pub(super) conditional_quotient_degree_bound: usize,
    /// Raw row-opening bytes if the 64 complete rows were widened to Fp4.
    pub(super) widened_row_opening_bytes: usize,
    /// Two maximum-size child frames, before any AXT carrier or public context.
    pub(super) two_max_child_frames_bytes: usize,
    /// The current proof target; successful wire-size qualification is separate.
    pub(super) proof_byte_target: usize,
    /// The current AXT inner-payload ceiling; exact bundle encoding is separate.
    pub(super) axt_inner_payload_ceiling_bytes: usize,
    refusal: DeepPrivateProofRefusal,
}

impl DeepProverPlan {
    /// Inspect complete source, current masks and full AIR degrees before any FFT/LDE.
    ///
    /// The source view has already checked every physical cell and omitted public
    /// column. Mask shapes use the same N and exclusive bounds as the existing
    /// `coefficient_masking` owner; supplied coefficients are checked in full.
    /// Its highest nonzero term gives the exact masked trace degree because the
    /// original subgroup interpolant has degree <N. This diagnostic has no path
    /// that authorizes an unmasked private proof.
    pub(super) fn new(
        source: &SourceTraceColumns<'_>,
        air: &CompactTransferAir,
        masks: &[&[F]],
        shapes: &[MaskingShape],
        limits: DeepProverLimits,
    ) -> Result<Self> {
        let schema = air.schema();
        if schema.trace_rows != TRACE_ROWS
            || schema.width != COLUMN_COUNT
            || schema.constraints != CONSTRAINTS
            || masks.len() != COMMITTED_COLUMN_COUNT
            || shapes.len() != COMMITTED_COLUMN_COUNT
        {
            return Err(invalid(
                "DEEP preflight requires the complete fixed source, AIR and masks",
            ));
        }
        DeepGeometry::new()?;
        let complete_row_lde_floor_bytes = complete_row_lde_floor_bytes()?;
        if complete_row_lde_floor_bytes > limits.max_complete_row_lde_bytes {
            return Err(Error::VerifierLimitExceeded {
                limit: "max_deep_complete_row_lde_bytes",
                actual: complete_row_lde_floor_bytes,
                max: limits.max_complete_row_lde_bytes,
            });
        }
        if MAX_FRAME_BYTES > PROOF_BYTE_TARGET {
            return Err(invalid(
                "DEEP maximum frame exceeds the fixed proof byte target",
            ));
        }
        let widened_row_opening_bytes =
            checked_product(&[QUERY_COUNT, COMMITTED_COLUMN_COUNT, size_of::<F>()])?;
        let two_max_child_frames_bytes = checked_product(&[2, MAX_FRAME_BYTES])?;
        if two_max_child_frames_bytes > AXT_INNER_PAYLOAD_CEILING_BYTES {
            return Err(Error::VerifierLimitExceeded {
                limit: "max_axt_inner_payload_bytes_before_carrier",
                actual: two_max_child_frames_bytes,
                max: AXT_INNER_PAYLOAD_CEILING_BYTES,
            });
        }
        let mut degrees = [PUBLIC_POLYNOMIAL_DEGREE + 1; COLUMN_COUNT];
        let mut first_degree_excess = None;
        let mut first_extension = None;
        let mut any_unmasked = false;
        for retained_column in 0..COMMITTED_COLUMN_COUNT {
            let source_column = COMMITTED_COLUMNS[retained_column];
            if source.committed_column(retained_column)?.len() != TRACE_ROWS {
                return Err(invalid(
                    "DEEP retained source column has another row extent",
                ));
            }
            let shape = shapes[retained_column];
            let mask = masks[retained_column];
            if shape.trace_coefficients != TRACE_ROWS
                || shape.trace_degree_bound != TRACE_ROWS
                || shape.mask_coefficients != mask.len()
                || mask.is_empty()
                || mask.len() > MAX_MASK_COEFFICIENTS
                || shape.mask_degree_bound == 0
                || shape.mask_degree_bound > mask.len()
            {
                return Err(invalid(
                    "DEEP mask differs from the bounded vanishing-mask shape",
                ));
            }
            let mut highest_nonzero = None;
            for (coefficient, &value) in mask.iter().enumerate() {
                value.validate("deep_prover_mask", &[retained_column, coefficient])?;
                if coefficient >= shape.mask_degree_bound && value != F::ZERO {
                    return Err(invalid("DEEP mask has nonzero declared degree padding"));
                }
                if value != F::ZERO {
                    highest_nonzero = Some(coefficient);
                    if value.coefficients()[1..].iter().any(|&word| word != 0)
                        && first_extension.is_none()
                    {
                        first_extension =
                            Some(DeepPrivateProofRefusal::ExtensionMaskCannotUseBaseRows {
                                retained_column,
                                source_column,
                            });
                    }
                }
            }
            degrees[source_column] = if let Some(highest) = highest_nonzero {
                let degree_bound = checked_sum(&[TRACE_ROWS, highest, 1])?;
                if degree_bound > FRI_DEGREES[0] && first_degree_excess.is_none() {
                    first_degree_excess = Some(DeepPrivateProofRefusal::MaskExceedsFriDegree {
                        retained_column,
                        source_column,
                        trace_degree_bound: degree_bound,
                        fri_degree_bound: FRI_DEGREES[0],
                    });
                }
                degree_bound
            } else {
                any_unmasked = true;
                TRACE_ROWS
            };
        }
        // The omitted public columns must remain verifier-known polynomials,
        // never a claimant-supplied mask or a physical-row constant at OOD points.
        if PUBLIC_COLUMNS
            .iter()
            .any(|&column| degrees[column] != PUBLIC_POLYNOMIAL_DEGREE + 1)
        {
            return Err(invalid("DEEP public columns cannot be masked"));
        }
        let numerator = air.numerator_degree_bounds(&degrees)?;
        let conditional_quotient_degree_bound = numerator.conditional_quotients().combined;
        let high_half_degree_bound = conditional_quotient_degree_bound.saturating_sub(TRACE_ROWS);
        let quotient_refusal = (high_half_degree_bound > FRI_DEGREES[0]).then_some(
            DeepPrivateProofRefusal::QuotientExceedsFriDegree {
                quotient_degree_bound: conditional_quotient_degree_bound,
                high_half_degree_bound,
                fri_degree_bound: FRI_DEGREES[0],
            },
        );
        let refusal = first_degree_excess
            .or(quotient_refusal)
            .or(first_extension)
            .unwrap_or(if any_unmasked {
                DeepPrivateProofRefusal::UnmaskedPrivateOpenings
            } else {
                DeepPrivateProofRefusal::MissingReviewedHidingAndComposition
            });
        Ok(Self {
            complete_row_lde_floor_bytes,
            numerator_degree_bound: numerator.combined_numerator(),
            conditional_quotient_degree_bound,
            widened_row_opening_bytes,
            two_max_child_frames_bytes,
            proof_byte_target: PROOF_BYTE_TARGET,
            axt_inner_payload_ceiling_bytes: AXT_INNER_PAYLOAD_CEILING_BYTES,
            refusal,
        })
    }

    /// Refuse every current private-proof candidate with its specific obstruction.
    pub(super) const fn require_private_proof(
        &self,
    ) -> core::result::Result<(), DeepPrivateProofRefusal> {
        Err(self.refusal)
    }
}

/// Checked base-field payload floor for a complete in-memory retained-row LDE.
fn complete_row_lde_floor_bytes() -> Result<usize> {
    checked_product(&[COMMITTED_COLUMN_COUNT, LDE_ROWS, size_of::<u64>()])
}

fn checked_product(factors: &[usize]) -> Result<usize> {
    factors.iter().try_fold(1usize, |product, &factor| {
        product
            .checked_mul(factor)
            .ok_or_else(|| invalid("DEEP preflight byte count overflows"))
    })
}

fn checked_sum(terms: &[usize]) -> Result<usize> {
    terms.iter().try_fold(0usize, |sum, &term| {
        sum.checked_add(term)
            .ok_or_else(|| invalid("DEEP preflight degree overflows"))
    })
}

fn invalid(details: &'static str) -> Error {
    Error::InvalidTraceShape {
        details: details.to_owned(),
    }
}

#[cfg(test)]
#[path = "deep_prover_plan/tests.rs"]
mod tests;
