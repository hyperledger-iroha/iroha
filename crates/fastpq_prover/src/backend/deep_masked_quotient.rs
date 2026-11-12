//! Full AIR quotient construction from bounded base-field masked replay.
//!
//! Four N-row stripes cover the exact 4N numerator coset. The existing 923-slot
//! evaluator consumes reconstructed public columns and current/next masked rows.
//! Its full numerator is interpolated before exact division by X^N-1; every
//! remainder coefficient must vanish. The fresh replay-owned T then randomizes
//! both quotient chunks without truncating the unequal high chunk.
//!
//! The numerator coset has a base-field offset, so the masked trace, the public
//! columns and every fixed polynomial take base-field values on it. All 923
//! slots are therefore evaluated in the base field; only their weighting by the
//! extension-field alphas leaves it. Each stripe's rows are evaluated by a fixed
//! number of contiguous jobs, each with its own workspace, and every value
//! lands at its own row: neither worker count nor completion order changes a
//! coefficient.
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
    compact_transfer_air::{
        CompactTransferAir, PolynomialAirEvaluator, PolynomialPreparationCost,
        PreparedPolynomialAir,
    },
    deep_geometry::{COSET_OFFSET, DeepGeometry, TRACE_ROWS},
    deep_masked_replay::{
        MaskedReplayPlan, MaskedTraceReplay, QUOTIENT_MASK_COEFFICIENTS, TRACE_MASK_COEFFICIENTS,
    },
    plan_arithmetic::{checked_add as add, checked_mul as mul, transform_work},
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
use rayon::prelude::*;

/// Contiguous jobs evaluating one stripe of the numerator coset. Admission,
/// workspace count and output order do not depend on the worker pool size.
pub(super) const NUMERATOR_JOBS: usize = 32;

/// Explicit arithmetic budget; no production default or admission capability.
#[derive(Clone, Copy, Debug)]
pub(super) struct QuotientLimits {
    pub(super) max_payload_bytes: usize,
    pub(super) max_work_units: usize,
}

/// Conservative payload bytes and work units of one complete quotient build.
fn quotient_charges(
    replay: &MaskedReplayPlan,
    public: &PolynomialPreparationCost,
    division: &VanishingDivisionPlan,
    pair: &PairMaskingPlan,
    rows: usize,
    cycle: usize,
) -> Result<(usize, usize)> {
    let word = size_of::<u64>();
    // Known-column cycle + selector roots and two selector work vectors, all
    // base-field words on the base-field coset.
    let reconstruction_bytes = mul(cycle * PUBLIC_COLUMN_COUNT + 3 * PHYSICAL_HASH_ROWS, word)?;
    // Every job owns both complete rows, both retained rows and its residues,
    // in base-field words. The public preparation charges one graph workspace;
    // each further job owns another.
    let job_cells = add(
        mul(
            NUMERATOR_JOBS,
            SLOT_COUNT + 2 * COLUMN_COUNT + 2 * COMMITTED_COLUMN_COUNT,
        )?,
        mul(NUMERATOR_JOBS - 1, public.scratch_cells)?,
    )?;
    let job_owners = mul(
        NUMERATOR_JOBS,
        size_of::<NumeratorWorkspace<'_, '_>>() + size_of::<Result<()>>(),
    )?;
    // Numerator values, one stripe of them, IFFT lanes, coefficients/remainder,
    // quotient and blinded chunks are all charged, even where lifetimes do not
    // overlap.
    let arithmetic_bytes = add(
        add(
            mul(3 * rows + TRACE_ROWS, F::BYTES)?,
            add(mul(job_cells, word)?, job_owners)?,
        )?,
        add(division.payload_bytes(), pair.payload_bytes())?,
    )?;
    let payload_bytes = add(
        replay.payload_bytes,
        add(
            public.payload_bytes,
            add(reconstruction_bytes, arithmetic_bytes)?,
        )?,
    )?;
    let cycle_work = mul(cycle, 12 * PHYSICAL_HASH_ROWS + 4096 + PUBLIC_COLUMN_COUNT)?;
    let point_work = mul(
        rows,
        add(public.point_work_units, 2 * SLOT_COUNT + 4 * COLUMN_COUNT)?,
    )?;
    // Initialization and erasure of every further job workspace.
    let job_work = mul(job_cells, 2)?;
    let work_units = add(
        replay.work_units,
        add(
            public.work_units,
            add(
                cycle_work,
                add(
                    add(point_work, job_work)?,
                    add(
                        transform_work(rows)?,
                        add(division.work_units(), pair.work_units())?,
                    )?,
                )?,
            )?,
        )?,
    )?;
    Ok((payload_bytes, work_units))
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
        if domain.base_offset() != Some(COSET_OFFSET) {
            return Err(invalid(
                "DEEP numerator coset must have its fixed base-field offset",
            ));
        }
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
        let (payload_bytes, work_units) =
            quotient_charges(&replay, &public, &division, &pair, rows, cycle)?;
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
            known.push(public.evaluate(self.domain.base_point(index)?)?);
        }
        let prepared = self.air.prepare_polynomial_evaluator(self.domain)?;
        let mut workspaces = reserved(NUMERATOR_JOBS)?;
        for _ in 0..NUMERATOR_JOBS {
            workspaces.push(NumeratorWorkspace::new(&prepared)?);
        }
        let rotation = self.domain.numerator_rotation(TRACE_ROWS)?;
        let cycle = self.cycle;
        let numerator = interpolate_numerator(
            replay,
            self.domain,
            &mut workspaces,
            |index, _point, retained, rotated, workspace| {
                reconstruct(retained, &known[index % cycle], &mut workspace.current)?;
                reconstruct(
                    rotated,
                    &known[(index + rotation) % cycle],
                    &mut workspace.next,
                )?;
                workspace.evaluator.evaluate_into(
                    index,
                    &workspace.current,
                    &workspace.next,
                    &mut workspace.residues,
                )?;
                // Keep the exact slot order within the row. Jobs never reduce
                // into a shared accumulator.
                Ok(workspace
                    .residues
                    .iter()
                    .zip(alphas)
                    .fold(F::ZERO, |sum, (&residue, &alpha)| {
                        sum.add(alpha.mul_base(residue))
                    }))
            },
        )?;
        // Every job has joined. Clear all owned arithmetic scratch before
        // retaining the numerator coefficients and division remainder.
        drop(workspaces);
        validate_coefficients(&numerator, self.numerator_bound, "deep_full_numerator")?;
        let quotient = self.division.divide(&numerator)?;
        let chunks = self
            .pair
            .apply(quotient.coefficients(), replay.quotient_mask())?;
        Ok(DeepMaskedQuotient {
            chunks,
            #[cfg(test)]
            degree_bounds: self.pair.degree_bounds(),
        })
    }
}

/// Exact blinded quotient chunks; all owned coefficients retain clearing storage.
pub(super) struct DeepMaskedQuotient {
    chunks: [SecretPolynomial<F>; 2],
    #[cfg(test)]
    degree_bounds: [usize; 2],
}
impl DeepMaskedQuotient {
    pub(super) fn chunks(&self) -> [&[F]; 2] {
        [&self.chunks[0], &self.chunks[1]]
    }
    #[cfg(test)]
    pub(super) fn degree_bounds(&self) -> [usize; 2] {
        self.degree_bounds
    }
}

/// Exact private buffers of one numerator job: the prepared evaluator's graph
/// workspace, both complete base-field rows and all 923 residues.
struct NumeratorWorkspace<'cache, 'source> {
    evaluator: PolynomialAirEvaluator<'cache, 'source>,
    current: SecretPolynomial<u64>,
    next: SecretPolynomial<u64>,
    residues: SecretPolynomial<u64>,
}

impl<'cache, 'source> NumeratorWorkspace<'cache, 'source> {
    fn new(prepared: &'cache PreparedPolynomialAir<'source>) -> Result<Self> {
        Ok(Self {
            evaluator: prepared.evaluator()?,
            current: SecretPolynomial::zeroed(COLUMN_COUNT)?,
            next: SecretPolynomial::zeroed(COLUMN_COUNT)?,
            residues: SecretPolynomial::zeroed(SLOT_COUNT)?,
        })
    }
}

fn reconstruct(
    retained: &[u64],
    public: &[u64; PUBLIC_COLUMN_COUNT],
    output: &mut [u64],
) -> Result<()> {
    if retained.len() != COMMITTED_COLUMN_COUNT || output.len() != COLUMN_COUNT {
        return Err(invalid(
            "DEEP numerator row reconstruction requires complete columns",
        ));
    }
    for (&column, &value) in COMMITTED_COLUMNS.iter().zip(retained) {
        output[column] = value;
    }
    for (&column, &value) in PUBLIC_COLUMNS.iter().zip(public) {
        output[column] = value;
    }
    Ok(())
}

/// Evaluate the numerator on every stripe of its coset and interpolate it.
///
/// `evaluate(index, point, current, next, workspace)` returns the value at
/// domain index `index`, given the retained row there and at its trace
/// rotation. Each stripe is split into [`NUMERATOR_JOBS`] contiguous row
/// ranges, one per workspace; a job also owns the two retained rows it reads.
///
/// The same loop is tested against independently constructed small polynomial
/// relations. The production caller above fixes the actual 923-slot AIR.
fn interpolate_numerator<S: Send>(
    replay: &mut MaskedTraceReplay,
    domain: PolynomialDomain,
    workspaces: &mut [S],
    evaluate: impl Fn(usize, F, &[u64], &[u64], &mut S) -> Result<F> + Sync,
) -> Result<SecretPolynomial<F>> {
    if workspaces.len() != NUMERATOR_JOBS {
        return Err(invalid(
            "DEEP numerator needs exactly one workspace per fixed job",
        ));
    }
    let width = replay.width();
    let stripe_rows = replay.plan().lde_rows() / replay.plan().stripes();
    let mut retained = SecretPolynomial::<u64>::zeroed(2 * NUMERATOR_JOBS * width)?;
    let mut stripe_values = SecretPolynomial::<F>::zeroed(stripe_rows)?;
    let mut values = SecretPolynomial::zeroed(domain.rows())?;
    replay.visit_subdomain(domain.rows(), |stripe, step| {
        if stripe.rows() != stripe_rows {
            return Err(invalid(
                "DEEP numerator stripe differs from its replay plan",
            ));
        }
        // One job owns one caller workspace and its own pair of retained rows.
        let mut jobs: Vec<(&mut S, &mut [u64])> = reserved(NUMERATOR_JOBS)?;
        jobs.extend(workspaces.iter_mut().zip(retained.chunks_mut(2 * width)));
        evaluate_parallel_rows(&mut stripe_values, &mut jobs, |row, (workspace, rows)| {
            let (current, next) = rows.split_at_mut(width);
            let index = stripe.global_index(row) / step;
            let point = F::embed_base(stripe.point(row));
            if domain.point(index)? != point {
                return Err(invalid("DEEP numerator replay domain orientation differs"));
            }
            stripe.fill_row(row, current)?;
            stripe.fill_row((row + 1) % stripe_rows, next)?;
            evaluate(index, point, current, next, workspace)
        })?;
        for (row, &value) in stripe_values.iter().enumerate() {
            values[stripe.global_index(row) / step] = value;
        }
        Ok(())
    })?;
    domain.interpolate(&values)
}

/// Fill disjoint contiguous ranges with at most one fixed workspace per job.
/// Results are collected by range index, then checked serially, preserving the
/// earliest row error independently of Rayon scheduling. All jobs join before
/// returning; the caller's guarded output and scratch erase on any failure.
pub(super) fn evaluate_parallel_rows<S: Send>(
    values: &mut [F],
    workspaces: &mut [S],
    evaluate: impl Fn(usize, &mut S) -> Result<F> + Sync,
) -> Result<()> {
    if values.is_empty() || workspaces.len() != NUMERATOR_JOBS {
        return Err(invalid(
            "parallel numerator needs nonempty rows and exactly 32 workspaces",
        ));
    }
    let rows_per_job = values.len().div_ceil(NUMERATOR_JOBS);
    let mut results: Vec<Result<()>> = reserved(NUMERATOR_JOBS)?;
    values
        .par_chunks_mut(rows_per_job)
        .zip(workspaces.par_iter_mut())
        .enumerate()
        .map(|(job, (rows, workspace))| {
            let start = job * rows_per_job;
            for (offset, value) in rows.iter_mut().enumerate() {
                *value = evaluate(start + offset, workspace)?;
            }
            Ok(())
        })
        .collect_into_vec(&mut results);
    for result in results {
        result?;
    }
    Ok(())
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
