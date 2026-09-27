//! Full AIR quotient construction from bounded base-field masked replay.
//!
//! Four N-row stripes cover the exact 4N numerator coset. The existing 923-slot
//! evaluator consumes reconstructed public columns and current/next masked rows.
//! Its full numerator is interpolated before exact division by X^N-1; every
//! remainder coefficient must vanish. The fresh replay-owned T then randomizes
//! both quotient chunks without truncating the unequal high chunk.
//!
//! The producer binds this owner to its actual row challenge.
//! TODO: Complete full producer/PCS qualification. Arithmetic success supplies
//! neither source authority nor independent security review.

use super::{
    air_degree::SLOT_COUNT,
    compact_public_columns::{
        COMMITTED_COLUMN_COUNT, COMMITTED_COLUMNS, PUBLIC_COLUMN_COUNT, PUBLIC_COLUMNS,
        PUBLIC_POLYNOMIAL_DEGREE, PublicColumnReconstruction,
    },
    compact_transfer_air::CompactTransferAir,
    deep_geometry::{COSET_OFFSET, DeepGeometry, TRACE_ROWS},
    deep_masked_replay::{
        MaskedReplayPlan, MaskedTraceReplay, QUOTIENT_MASK_COEFFICIENTS, TRACE_MASK_COEFFICIENTS,
    },
    masked_quotient::{checked_add as add, checked_mul as mul, transform_work},
    polynomial_division::VanishingDivisionPlan,
    polynomial_field::PolynomialField,
    polynomial_transform::{PolynomialDomain, reserved, validate_coefficients},
    quotient_pair_masking::{PairMaskingLimits, PairMaskingPlan, PairMaskingShape},
    secret_polynomial::SecretPolynomial,
};
use crate::{
    Error, Result,
    field::GoldilocksFp4V1 as F,
    gadgets::compact_smt_air::{COLUMN_COUNT, PHYSICAL_HASH_ROWS},
};

/// Explicit arithmetic budget; no production default or admission capability.
#[derive(Clone, Copy, Debug)]
pub(super) struct QuotientLimits {
    pub(super) max_payload_bytes: usize,
    pub(super) max_work_units: usize,
}

/// Binds source geometry, full AIR degrees and all allocation extents before replay.
pub(super) struct DeepQuotientPlan<'a> {
    air: &'a CompactTransferAir,
    replay: MaskedReplayPlan,
    domain: PolynomialDomain,
    numerator_bound: usize,
    division: VanishingDivisionPlan,
    pair: PairMaskingPlan,
    cycle: usize,
    pub(super) payload_bytes: usize,
    pub(super) work_units: usize,
}

impl<'a> DeepQuotientPlan<'a> {
    /// Charge the complete active source/replay, public preparation, numerator,
    /// division and chunk buffers. Some sub-owner lifetimes are conservatively
    /// summed; this is a payload upper bound, not allocator/RSS measurement.
    pub(super) fn new(
        air: &'a CompactTransferAir,
        replay: MaskedReplayPlan,
        limits: QuotientLimits,
    ) -> Result<Self> {
        if !replay.is_candidate_geometry() {
            return Err(invalid(
                "DEEP quotient needs the exact masked replay geometry",
            ));
        }
        let mut degrees = [PUBLIC_POLYNOMIAL_DEGREE + 1; COLUMN_COUNT];
        for column in COMMITTED_COLUMNS {
            degrees[column] = TRACE_ROWS + TRACE_MASK_COEFFICIENTS;
        }
        let degrees = air.numerator_degree_bounds(&degrees)?;
        let numerator_bound = degrees.combined_numerator();
        let rows = numerator_bound
            .checked_next_power_of_two()
            .ok_or_else(|| invalid("DEEP numerator size overflow"))?;
        let domain = PolynomialDomain::new(
            rows,
            F::embed_base(COSET_OFFSET),
            4 * TRACE_ROWS,
            limits.max_payload_bytes,
        )?;
        domain.numerator_rotation(TRACE_ROWS)?;
        let quotient_bound = degrees.conditional_quotients().combined;
        let division = VanishingDivisionPlan::new(
            TRACE_ROWS,
            rows,
            numerator_bound,
            quotient_bound,
            limits.max_payload_bytes,
            limits.max_work_units,
        )?;
        let pair = PairMaskingPlan::new(
            PairMaskingShape {
                split: TRACE_ROWS,
                quotient_coefficients: quotient_bound,
                quotient_degree_bound: quotient_bound,
                mask_coefficients: QUOTIENT_MASK_COEFFICIENTS,
                mask_degree_bound: QUOTIENT_MASK_COEFFICIENTS,
            },
            PairMaskingLimits {
                max_chunk_degree_bound: 2 * TRACE_ROWS,
                max_payload_bytes: limits.max_payload_bytes,
                max_work_units: limits.max_work_units,
            },
        )?;
        let public = air.polynomial_preparation_cost(domain)?;
        let cycle = rows / (TRACE_ROWS / PHYSICAL_HASH_ROWS);
        // Known-column cycle + selector roots and two selector work vectors.
        let reconstruction_bytes = add(
            mul(
                cycle * PUBLIC_COLUMN_COUNT + 2 * PHYSICAL_HASH_ROWS,
                F::BYTES,
            )?,
            PHYSICAL_HASH_ROWS * 8,
        )?;
        // Numerator values, IFFT lanes, coefficients/remainder, quotient and
        // blinded chunks are all charged, even where lifetimes do not overlap.
        let arithmetic_bytes = add(
            mul(3 * rows + SLOT_COUNT + 2 * COLUMN_COUNT, F::BYTES)?,
            add(division.payload_bytes(), pair.payload_bytes())?,
        )?;
        let payload_bytes = add(
            replay.payload_bytes,
            add(
                public.payload_bytes,
                add(
                    reconstruction_bytes,
                    add(arithmetic_bytes, 2 * COMMITTED_COLUMN_COUNT * 8)?,
                )?,
            )?,
        )?;
        let cycle_work = mul(cycle, 12 * PHYSICAL_HASH_ROWS + 4096 + PUBLIC_COLUMN_COUNT)?;
        let point_work = mul(
            rows,
            add(public.point_work_units, 2 * SLOT_COUNT + 4 * COLUMN_COUNT)?,
        )?;
        let work_units = add(
            replay.work_units,
            add(
                public.work_units,
                add(
                    cycle_work,
                    add(
                        point_work,
                        add(
                            transform_work(rows)?,
                            add(division.work_units(), pair.work_units())?,
                        )?,
                    )?,
                )?,
            )?,
        )?;
        limit(
            "max_deep_quotient_payload_bytes",
            payload_bytes,
            limits.max_payload_bytes,
        )?;
        limit(
            "max_deep_quotient_work_units",
            work_units,
            limits.max_work_units,
        )?;
        limit(
            "max_deep_quotient_addressable_bytes",
            payload_bytes,
            isize::MAX as usize,
        )?;
        Ok(Self {
            air,
            replay,
            domain,
            numerator_bound,
            division,
            pair,
            cycle,
            payload_bytes,
            work_units,
        })
    }

    /// Evaluate the actual full relation and reject a nonzero polynomial remainder.
    /// Alpha must come after the trace commitment; this arithmetic accepts only
    /// explicit canonical coefficients and does not itself derive challenges.
    pub(super) fn build(
        self,
        replay: &mut MaskedTraceReplay,
        alphas: &[F],
    ) -> Result<DeepMaskedQuotient> {
        if replay.plan() != self.replay
            || !replay.has_candidate_geometry()
            || alphas.len() != SLOT_COUNT
        {
            return Err(invalid(
                "DEEP quotient source or complete alpha shape differs from its plan",
            ));
        }
        replay.ensure_pass_available()?;
        for (slot, &alpha) in alphas.iter().enumerate() {
            alpha.validate("deep_quotient_alpha", &[slot])?;
        }
        let public = PublicColumnReconstruction::new(&DeepGeometry::polynomial_parameters())?;
        let mut known = reserved(self.cycle)?;
        for index in 0..self.cycle {
            known.push(public.evaluate(self.domain.point(index)?)?);
        }
        let mut evaluator = self.air.prepare_polynomial_evaluator(self.domain)?;
        let mut current = SecretPolynomial::zeroed(COLUMN_COUNT)?;
        let mut next = SecretPolynomial::zeroed(COLUMN_COUNT)?;
        let mut residues = SecretPolynomial::zeroed(SLOT_COUNT)?;
        let rotation = self.domain.numerator_rotation(TRACE_ROWS)?;
        let numerator =
            interpolate_numerator(replay, self.domain, |index, _point, retained, rotated| {
                reconstruct(retained, &known[index % self.cycle], &mut current)?;
                reconstruct(rotated, &known[(index + rotation) % self.cycle], &mut next)?;
                evaluator.evaluate_into(index, &current, &next, &mut residues)?;
                Ok(residues
                    .iter()
                    .zip(alphas)
                    .fold(F::ZERO, |sum, (&residue, &alpha)| {
                        sum.add(residue.mul(alpha))
                    }))
            })?;
        validate_coefficients(&numerator, self.numerator_bound, "deep_full_numerator")?;
        let quotient = self.division.divide(&numerator)?;
        let chunks = self
            .pair
            .apply(quotient.coefficients(), replay.quotient_mask())?;
        Ok(DeepMaskedQuotient {
            chunks,
            degree_bounds: self.pair.degree_bounds(),
        })
    }
}

/// Exact blinded quotient chunks; all owned coefficients retain clearing storage.
pub(super) struct DeepMaskedQuotient {
    chunks: [SecretPolynomial<F>; 2],
    degree_bounds: [usize; 2],
}
impl DeepMaskedQuotient {
    pub(super) fn chunks(&self) -> [&[F]; 2] {
        [&self.chunks[0], &self.chunks[1]]
    }
    pub(super) fn degree_bounds(&self) -> [usize; 2] {
        self.degree_bounds
    }
}

fn reconstruct(
    retained: &[u64],
    public: &[F; PUBLIC_COLUMN_COUNT],
    output: &mut [F],
) -> Result<()> {
    if retained.len() != COMMITTED_COLUMN_COUNT || output.len() != COLUMN_COUNT {
        return Err(invalid(
            "DEEP numerator row reconstruction requires complete columns",
        ));
    }
    for (&column, &value) in COMMITTED_COLUMNS.iter().zip(retained) {
        output[column] = F::embed_base(value);
    }
    for (&column, &value) in PUBLIC_COLUMNS.iter().zip(public) {
        output[column] = value;
    }
    Ok(())
}

// Shared arithmetic loop also tested against independently constructed small
// polynomial relations. The production wrapper above fixes the actual 923 AIR.
fn interpolate_numerator(
    replay: &mut MaskedTraceReplay,
    domain: PolynomialDomain,
    mut evaluate: impl FnMut(usize, F, &[u64], &[u64]) -> Result<F>,
) -> Result<SecretPolynomial<F>> {
    let width = replay.width();
    let mut current = SecretPolynomial::zeroed(width)?;
    let mut next = SecretPolynomial::zeroed(width)?;
    let mut values = SecretPolynomial::zeroed(domain.rows())?;
    replay.visit_subdomain(domain.rows(), |stripe, step| {
        for row in 0..stripe.rows() {
            let index = stripe.global_index(row) / step;
            let point = F::embed_base(stripe.point(row));
            if domain.point(index)? != point {
                return Err(invalid("DEEP numerator replay domain orientation differs"));
            }
            stripe.fill_row(row, &mut current)?;
            stripe.fill_row((row + 1) % stripe.rows(), &mut next)?;
            values[index] = evaluate(index, point, &current, &next)?;
        }
        Ok(())
    })?;
    domain.interpolate(&values)
}

fn invalid(details: &'static str) -> Error {
    Error::InvalidTraceShape {
        details: details.to_owned(),
    }
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

#[cfg(test)]
#[path = "deep_masked_quotient/tests.rs"]
mod tests;
