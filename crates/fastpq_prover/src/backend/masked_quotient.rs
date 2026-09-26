//! Explicit masked trace preparation and the shared full coefficient AIR quotient.
//!
//! This validation-scoped arithmetic chooses no hiding coins, transcript, PCS,
//! proof geometry or admission. The caller supplies private mask coefficients and
//! retains/clears borrowed inputs. Owned private coefficients, evaluations and
//! scratch use fixed-size zeroizing storage and have no value-bearing Debug.
//! Caller copies, transient arithmetic values and hardware state are not erased
//! by this owned-allocation boundary; erasure is not a zero-knowledge argument.
//!
//! TODO: Integrate authenticated coefficient commitments, degree padding, entropy,
//! transcript order and the final PCS before any production proof registration.
//! The quotient plan also accepts validated complete unmasked coefficients; both
//! callers use the same full polynomial evaluation and exact division.

use rayon::prelude::*;

#[cfg(test)]
use super::coefficient_masking::{MaskingLimits, MaskingPlan, MaskingShape};
use super::{
    air_degree::{AirDegreeBounds, SLOT_COUNT},
    compact_transfer_air::{CompactTransferAir, PolynomialAirEvaluator, PreparedPolynomialAir},
    polynomial_division::{ExactQuotient, VanishingDivisionPlan},
    polynomial_field::PolynomialField,
    polynomial_transform::{PolynomialDomain, PolynomialLanes, reserved, validate_coefficients},
    secret_polynomial::SecretPolynomial,
};
use crate::gadgets::compact_smt_air::{COLUMN_COUNT, PHYSICAL_ROW_COUNT};
use crate::{Error, Result, field::GoldilocksFp4V1 as F};

/// Fixed contiguous job count; admission and output order do not depend on pool size.
pub(super) const NUMERATOR_JOBS: usize = 32;

/// Explicit arithmetic resource policy; this has no production default or wire representation.
#[derive(Clone, Copy, Debug)]
pub(super) struct MaskedQuotientLimits {
    /// Simultaneous declared payload bytes, including borrowed inputs for the active phase.
    pub(super) max_payload_bytes: usize,
    /// Conservative checked arithmetic/inspection/erasure work, not elapsed time or RSS.
    pub(super) max_work_units: usize,
    /// Maximum exact transform extent, also capped by the existing source root.
    pub(super) max_interpolation_rows: usize,
    /// Maximum exact private mask coefficient extent per column.
    #[cfg(test)]
    pub(super) max_mask_coefficients: usize,
    /// Maximum exact masked coefficient extent per column, including supplied padding.
    #[cfg(test)]
    pub(super) max_masked_coefficients: usize,
}

/// Prepared private coefficients for the exact complete trace; no Debug or Clone.
#[cfg(test)]
pub(super) struct PreparedMaskedTrace {
    columns: Vec<SecretPolynomial<F>>,
    degree_bounds: [usize; COLUMN_COUNT],
}

#[cfg(test)]
impl PreparedMaskedTrace {
    /// Interpolate actual subgroup values and apply each explicitly supplied mask.
    ///
    /// All dimensions/resources and every input coordinate are checked before
    /// private arithmetic. Caller slices stay unchanged and caller-owned. The
    /// returned coefficients must be committed before deriving constraint weights;
    /// this function supplies neither that commitment nor fresh random masks.
    pub(super) fn prepare(
        trace: &[&[F]],
        masks: &[&[F]],
        shapes: &[MaskingShape],
        limits: MaskedQuotientLimits,
    ) -> Result<Self> {
        if trace.len() != COLUMN_COUNT
            || masks.len() != COLUMN_COUNT
            || shapes.len() != COLUMN_COUNT
        {
            return Err(invalid(
                "masked trace requires exactly 342 columns, shapes and explicit masks",
            ));
        }
        let mask_limits = MaskingLimits {
            max_trace_coefficients: PHYSICAL_ROW_COUNT,
            max_mask_coefficients: limits.max_mask_coefficients,
            max_output_coefficients: limits.max_masked_coefficients,
            max_output_bytes: limits.max_payload_bytes,
            max_work_units: limits.max_work_units,
        };
        let mut plans = reserved(COLUMN_COUNT)?;
        let mut coefficient_cells = 0;
        let mut input_cells = checked_mul(COLUMN_COUNT, PHYSICAL_ROW_COUNT)?;
        let mut work = 0;
        for column in 0..COLUMN_COUNT {
            let shape = shapes[column];
            if shape.trace_coefficients != PHYSICAL_ROW_COUNT
                || trace[column].len() != PHYSICAL_ROW_COUNT
                || masks[column].len() != shape.mask_coefficients
            {
                return Err(invalid(
                    "masked trace inputs differ from their exact full trace and mask extents",
                ));
            }
            let plan = MaskingPlan::<F>::new(shape, mask_limits)?;
            coefficient_cells = checked_add(coefficient_cells, plan.output_coefficients())?;
            input_cells = checked_add(input_cells, masks[column].len())?;
            work = checked_add(
                work,
                checked_add(plan.work_units(), transform_work(PHYSICAL_ROW_COUNT)?)?,
            )?;
            plans.push(plan);
        }
        // Full borrowed inputs + all retained masked coefficients + one lane and
        // one interpolated coefficient workspace. Exact boxed buffers do not grow.
        let cells = checked_add(
            checked_add(input_cells, coefficient_cells)?,
            checked_mul(2, PHYSICAL_ROW_COUNT)?,
        )?;
        let bytes = checked_mul(cells, F::BYTES)?;
        work = checked_add(
            work,
            checked_mul(4, checked_add(input_cells, coefficient_cells)?)?,
        )?;
        check_resources(limits, bytes, work)?;
        let domain = PolynomialDomain::new(
            PHYSICAL_ROW_COUNT,
            F::ONE,
            limits.max_interpolation_rows,
            limits.max_payload_bytes,
        )?;
        // A malformed late coordinate cannot leave already processed columns or
        // begin an FFT before all caller inputs have passed the complete preflight.
        for column in 0..COLUMN_COUNT {
            for (row, &value) in trace[column].iter().enumerate() {
                value.validate("masked_trace_subgroup_values", &[column, row])?;
            }
            validate_coefficients(
                masks[column],
                shapes[column].mask_degree_bound,
                "masked_trace_private_mask",
            )?;
        }
        let mut columns = reserved(COLUMN_COUNT)?;
        for column in 0..COLUMN_COUNT {
            let coefficients = domain.interpolate(trace[column])?;
            let mut masked = SecretPolynomial::zeroed(plans[column].output_coefficients())?;
            plans[column].apply_into(&coefficients, masks[column], &mut masked)?;
            columns.push(masked);
        }
        Ok(Self {
            columns,
            degree_bounds: core::array::from_fn(|column| plans[column].masked_degree_bound()),
        })
    }

    /// Borrow exact private coefficients for the caller's authenticated commitment owner.
    pub(super) fn column(&self, index: usize) -> Result<&[F]> {
        self.columns
            .get(index)
            .map(|column| &**column)
            .ok_or(Error::QueryIndexOutOfRange {
                index,
                len: COLUMN_COUNT,
            })
    }

    /// Explicit exclusive bounds; commitment padding/degree authentication remains external.
    pub(super) fn degree_bounds(&self) -> &[usize; COLUMN_COUNT] {
        &self.degree_bounds
    }
}

/// Complete validated coefficient view shared by masked and unmasked callers.
struct CoefficientTrace<'a> {
    columns: [&'a [F]; COLUMN_COUNT],
    degree_bounds: [usize; COLUMN_COUNT],
    coefficient_cells: usize,
}

impl<'a> CoefficientTrace<'a> {
    fn new(columns: &[&'a [F]], degree_bounds: &[usize]) -> Result<Self> {
        let columns: [&[F]; COLUMN_COUNT] = columns
            .try_into()
            .map_err(|_| invalid("coefficient quotient requires exactly 342 columns"))?;
        let degree_bounds: [usize; COLUMN_COUNT] = degree_bounds
            .try_into()
            .map_err(|_| invalid("coefficient quotient requires exactly 342 degree bounds"))?;
        let mut coefficient_cells = 0;
        for column in 0..COLUMN_COUNT {
            if degree_bounds[column] == 0 {
                return Err(invalid(
                    "coefficient quotient needs positive exclusive degree bounds",
                ));
            }
            validate_coefficients(
                columns[column],
                degree_bounds[column],
                "coefficient_quotient_source",
            )?;
            coefficient_cells = checked_add(coefficient_cells, columns[column].len())?;
        }
        Ok(Self {
            columns,
            degree_bounds,
            coefficient_cells,
        })
    }

    fn column(&self, index: usize) -> Result<&'a [F]> {
        self.columns
            .get(index)
            .copied()
            .ok_or(Error::QueryIndexOutOfRange {
                index,
                len: COLUMN_COUNT,
            })
    }

    fn degree_bounds(&self) -> &[usize; COLUMN_COUNT] {
        &self.degree_bounds
    }
}

/// Borrow-bound shared full numerator plan, with no value-bearing Debug representation.
pub(super) struct MaskedQuotientPlan<'a> {
    air: &'a CompactTransferAir,
    trace: CoefficientTrace<'a>,
    domain: PolynomialDomain,
    rotation: usize,
    degrees: AirDegreeBounds,
    division: VanishingDivisionPlan,
    payload_bytes: usize,
    #[cfg(test)]
    work_units: usize,
}

impl<'a> MaskedQuotientPlan<'a> {
    /// Bind actual AIR/trace owners and explicit arithmetic geometry before allocating LDEs.
    #[cfg(test)]
    pub(super) fn new(
        air: &'a CompactTransferAir,
        trace: &'a PreparedMaskedTrace,
        interpolation_rows: usize,
        offset: F,
        quotient_extent: usize,
        limits: MaskedQuotientLimits,
    ) -> Result<Self> {
        let columns: [&[F]; COLUMN_COUNT] = core::array::from_fn(|column| &*trace.columns[column]);
        Self::from_coefficients(
            air,
            &columns,
            trace.degree_bounds(),
            interpolation_rows,
            offset,
            quotient_extent,
            limits,
        )
    }

    /// Bind complete canonical coefficient columns without applying a mask adapter.
    ///
    /// Every declared padding coefficient is checked before transform allocation.
    /// A caller providing unmasked columns owns all fixed-column reconstruction;
    /// this shared arithmetic does not infer public columns or authenticate inputs.
    pub(super) fn from_coefficients(
        air: &'a CompactTransferAir,
        columns: &[&'a [F]],
        degree_bounds: &[usize],
        interpolation_rows: usize,
        offset: F,
        quotient_extent: usize,
        limits: MaskedQuotientLimits,
    ) -> Result<Self> {
        let trace = CoefficientTrace::new(columns, degree_bounds)?;
        let degrees = air.numerator_degree_bounds(trace.degree_bounds())?;
        if interpolation_rows < degrees.combined_numerator() {
            return Err(invalid(
                "numerator interpolation would alias high full-polynomial coefficients",
            ));
        }
        // The full degree bound does not license truncation of a longer supplied
        // padded coefficient array. The exact extent must fit as well.
        if trace
            .columns
            .iter()
            .any(|column| column.len() > interpolation_rows)
        {
            return Err(invalid(
                "masked coefficient extent exceeds the exact numerator transform",
            ));
        }
        let domain = PolynomialDomain::new(
            interpolation_rows,
            offset,
            limits.max_interpolation_rows,
            limits.max_payload_bytes,
        )?;
        let rotation = domain.numerator_rotation(PHYSICAL_ROW_COUNT)?;
        let division = VanishingDivisionPlan::new(
            PHYSICAL_ROW_COUNT,
            interpolation_rows,
            degrees.combined_numerator(),
            quotient_extent,
            limits.max_payload_bytes,
            limits.max_work_units,
        )?;
        let public = air.polynomial_preparation_cost(domain)?;
        // All retained trace/fixed lanes plus five complete L-sized buffers cover
        // numerator values/coefficients, IFFT lanes, division remainder and one
        // transient column transform. The returned quotient is counted separately.
        let trace_lanes = checked_mul(COLUMN_COUNT, interpolation_rows)?;
        // The public preparation charge includes one graph workspace. Charge all
        // additional fixed job workspaces, each with its own current/next rows and
        // residues. The caller's alpha slice remains a separate borrowed input.
        let scratch_cells = checked_add(
            SLOT_COUNT,
            checked_add(
                checked_mul(NUMERATOR_JOBS, SLOT_COUNT + 2 * COLUMN_COUNT)?,
                checked_mul(NUMERATOR_JOBS - 1, public.scratch_cells)?,
            )?,
        )?;
        let private_cells = checked_add(
            trace.coefficient_cells,
            checked_add(
                trace_lanes,
                checked_add(
                    checked_mul(5, interpolation_rows)?,
                    checked_add(quotient_extent, scratch_cells)?,
                )?,
            )?,
        )?;
        let descriptors = checked_mul(
            NUMERATOR_JOBS,
            core::mem::size_of::<NumeratorScratch<'_, '_>>() + core::mem::size_of::<Result<()>>(),
        )?;
        let payload_bytes = checked_add(
            checked_add(checked_mul(private_cells, F::BYTES)?, public.payload_bytes)?,
            descriptors,
        )?;
        let transform_work = checked_mul(COLUMN_COUNT + 1, transform_work(interpolation_rows)?)?;
        let mut work_units = checked_add(transform_work, public.work_units)?;
        work_units = checked_add(
            work_units,
            checked_mul(
                NUMERATOR_JOBS - 1,
                public.scratch_cells + SLOT_COUNT + 2 * COLUMN_COUNT,
            )?,
        )?;
        // The shared borrowed view checks every coordinate and declared padding
        // before a transform. Charge this pass independently of FFT inspection.
        work_units = checked_add(work_units, checked_mul(8, trace.coefficient_cells)?)?;
        work_units = checked_add(
            work_units,
            checked_mul(
                interpolation_rows,
                checked_add(public.point_work_units, 2 * SLOT_COUNT)?,
            )?,
        )?;
        work_units = checked_add(work_units, division.work_units())?;
        // Include owned input/result/scratch coordinate inspections/erasure; this
        // conservative lifetime charge may count a buffer again in its sub-owner.
        work_units = checked_add(work_units, payload_bytes / core::mem::size_of::<u64>())?;
        check_resources(limits, payload_bytes, work_units)?;
        Ok(Self {
            air,
            trace,
            domain,
            rotation,
            degrees,
            division,
            payload_bytes,
            #[cfg(test)]
            work_units,
        })
    }

    /// Declared simultaneous payload bound, not allocator overhead or measured peak RSS.
    pub(super) const fn payload_bytes(&self) -> usize {
        self.payload_bytes
    }

    /// Declared structural work including cleanup of private owned buffers.
    #[cfg(test)]
    pub(super) const fn work_units(&self) -> usize {
        self.work_units
    }

    /// Build the actual alpha-weighted full numerator and exact zero-remainder quotient.
    ///
    /// Alpha is explicit arithmetic input; the caller must establish commitment
    /// order and independent challenges. Zero/cancelling weights do not prove
    /// individual AIR constraints, and no acceptance result is returned here.
    pub(super) fn build(self, alpha: &[F]) -> Result<MaskedAirQuotient> {
        if alpha.len() != SLOT_COUNT {
            return Err(invalid(
                "masked numerator requires exactly 923 constraint weights",
            ));
        }
        for (slot, &value) in alpha.iter().enumerate() {
            value.validate("masked_numerator_alpha", &[slot])?;
        }
        let mut columns: Vec<PolynomialLanes> = reserved(COLUMN_COUNT)?;
        for column in 0..COLUMN_COUNT {
            columns.push(
                self.domain
                    .evaluate(self.trace.column(column)?, self.trace.degree_bounds[column])?,
            );
        }
        let prepared = self.air.prepare_polynomial_evaluator(self.domain)?;
        let mut workspaces = reserved(NUMERATOR_JOBS)?;
        for _ in 0..NUMERATOR_JOBS {
            workspaces.push(NumeratorScratch::new(&prepared)?);
        }
        let mut values = SecretPolynomial::zeroed(self.domain.rows())?;
        evaluate_parallel_rows(&mut values, &mut workspaces, |index, workspace| {
            let next_index = (index + self.rotation) % self.domain.rows();
            for (column, lanes) in columns.iter().enumerate() {
                workspace.current[column] = lanes.value(index)?;
                workspace.next[column] = lanes.value(next_index)?;
            }
            workspace.evaluator.evaluate_into(
                index,
                &workspace.current,
                &workspace.next,
                &mut workspace.residues,
            )?;
            // Keep the exact existing per-row slot order; workers never reduce
            // into a shared accumulator or merge floating/field partial sums.
            Ok(workspace
                .residues
                .iter()
                .zip(alpha)
                .fold(F::ZERO, |sum, (&value, &weight)| sum.add(value.mul(weight))))
        })?;
        // Rayon has joined every range. Clear all owned arithmetic scratch before
        // retaining the numerator coefficients and division remainder.
        drop(workspaces);
        let numerator = self.domain.interpolate(&values)?;
        validate_coefficients(
            &numerator,
            self.degrees.combined_numerator(),
            "masked_full_numerator_coefficients",
        )?;
        let quotient = self.division.divide(&numerator)?;
        // Production retains only the checked quotient; erase the full numerator now.
        #[cfg(not(test))]
        drop(numerator);
        Ok(MaskedAirQuotient {
            #[cfg(test)]
            numerator,
            quotient,
            #[cfg(test)]
            domain: self.domain,
            #[cfg(test)]
            degrees: self.degrees,
        })
    }
}

/// Exact private buffers for one fixed contiguous numerator range.
struct NumeratorScratch<'cache, 'source> {
    evaluator: PolynomialAirEvaluator<'cache, 'source>,
    current: SecretPolynomial<F>,
    next: SecretPolynomial<F>,
    residues: SecretPolynomial<F>,
}

impl<'cache, 'source> NumeratorScratch<'cache, 'source> {
    fn new(prepared: &'cache PreparedPolynomialAir<'source>) -> Result<Self> {
        Ok(Self {
            evaluator: prepared.evaluator()?,
            current: SecretPolynomial::zeroed(COLUMN_COUNT)?,
            next: SecretPolynomial::zeroed(COLUMN_COUNT)?,
            residues: SecretPolynomial::zeroed(SLOT_COUNT)?,
        })
    }
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

/// Owned exact quotient, with full numerator retained only for independent test checks.
/// Private storage has no Debug or unguarded raw buffer transfer.
pub(super) struct MaskedAirQuotient {
    #[cfg(test)]
    numerator: SecretPolynomial<F>,
    quotient: ExactQuotient,
    #[cfg(test)]
    domain: PolynomialDomain,
    #[cfg(test)]
    degrees: AirDegreeBounds,
}

impl MaskedAirQuotient {
    /// Transfer the guarded exact quotient, dropping any test-only numerator.
    pub(super) fn into_quotient(self) -> ExactQuotient {
        self.quotient
    }

    /// Borrow full numerator coefficients including required high zero padding.
    #[cfg(test)]
    pub(super) fn numerator(&self) -> &[F] {
        &self.numerator
    }
    /// Borrow the exact quotient whose full remainder was checked.
    pub(super) fn quotient(&self) -> &ExactQuotient {
        &self.quotient
    }
    /// Public arithmetic domain retained for reproducible independent checking.
    #[cfg(test)]
    pub(super) const fn domain(&self) -> PolynomialDomain {
        self.domain
    }
    /// Source-derived full polynomial degree obligations, not a PCS proof.
    #[cfg(test)]
    pub(super) fn degrees(&self) -> &AirDegreeBounds {
        &self.degrees
    }
}

/// Conservative four-lane FFT/IFFT, twist, inspection and cleanup work.
pub(super) fn transform_work(rows: usize) -> Result<usize> {
    if !rows.is_power_of_two() {
        return Err(invalid("transform work requires a power-of-two extent"));
    }
    checked_add(
        checked_mul(checked_mul(64, rows)?, rows.ilog2() as usize + 1)?,
        4096,
    )
}

pub(super) fn checked_add(left: usize, right: usize) -> Result<usize> {
    left.checked_add(right)
        .ok_or_else(|| invalid("masked quotient resource count overflow"))
}
pub(super) fn checked_mul(left: usize, right: usize) -> Result<usize> {
    left.checked_mul(right)
        .ok_or_else(|| invalid("masked quotient resource count overflow"))
}
fn check_resources(limits: MaskedQuotientLimits, bytes: usize, work: usize) -> Result<()> {
    for (limit, actual, max) in [
        (
            "max_masked_quotient_payload_bytes",
            bytes,
            limits.max_payload_bytes,
        ),
        (
            "max_masked_quotient_work_units",
            work,
            limits.max_work_units,
        ),
        (
            "max_masked_quotient_addressable_bytes",
            bytes,
            isize::MAX as usize,
        ),
    ] {
        if actual > max {
            return Err(Error::VerifierLimitExceeded { limit, actual, max });
        }
    }
    Ok(())
}
fn invalid(details: &'static str) -> Error {
    Error::InvalidTraceShape {
        details: details.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn limits() -> MaskedQuotientLimits {
        MaskedQuotientLimits {
            max_payload_bytes: 1 << 30,
            max_work_units: 1 << 30,
            max_interpolation_rows: 524_288,
            max_mask_coefficients: 1,
            max_masked_coefficients: PHYSICAL_ROW_COUNT + 1,
        }
    }

    #[test]
    fn parallel_rows_preserve_serial_field_results_and_range_ownership() {
        let weighted = |index: usize, slot: usize| {
            F::new([index as u64 + 1, slot as u64 + 3, 5, 7])
                .unwrap()
                .mul(F::new([11, 13, slot as u64 + 17, 19]).unwrap())
        };
        for threads in [1, 2, 6] {
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .unwrap();
            for length in [1, 31, 32, 33, 65, 257] {
                let mut values = SecretPolynomial::zeroed(length).unwrap();
                let mut workspaces = (0..NUMERATOR_JOBS)
                    .map(|_| (None, SecretPolynomial::zeroed(SLOT_COUNT).unwrap()))
                    .collect::<Vec<(Option<usize>, SecretPolynomial<F>)>>();
                pool.install(|| {
                    evaluate_parallel_rows(&mut values, &mut workspaces, |index, workspace| {
                        if let Some(previous) = workspace.0 {
                            assert_eq!(index, previous + 1);
                        }
                        workspace.0 = Some(index);
                        for (slot, value) in workspace.1.iter_mut().enumerate() {
                            *value = weighted(index, slot);
                        }
                        Ok(workspace.1.iter().copied().fold(F::ZERO, F::add))
                    })
                })
                .unwrap();
                for (index, &value) in values.iter().enumerate() {
                    assert_eq!(
                        value,
                        (0..SLOT_COUNT).fold(F::ZERO, |sum, slot| sum.add(weighted(index, slot)))
                    );
                }
                let chunk = length.div_ceil(NUMERATOR_JOBS);
                let jobs = length.div_ceil(chunk);
                assert_eq!(
                    workspaces
                        .iter()
                        .filter(|workspace| workspace.0.is_some())
                        .count(),
                    jobs
                );
            }
        }
    }

    #[test]
    fn parallel_failures_join_all_ranges_and_return_the_earliest_row_error() {
        use std::sync::atomic::{AtomicBool, Ordering};
        for threads in [1, 2, 6] {
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .unwrap();
            let visited = (0..65).map(|_| AtomicBool::new(false)).collect::<Vec<_>>();
            let mut values = SecretPolynomial::zeroed(65).unwrap();
            let mut workspaces = [(); NUMERATOR_JOBS];
            let result = pool.install(|| {
                evaluate_parallel_rows(&mut values, &mut workspaces, |index, _| {
                    visited[index].store(true, Ordering::SeqCst);
                    if [3, 7, 11].contains(&index) {
                        return Err(Error::QueryIndexOutOfRange { index, len: 65 });
                    }
                    Ok(F::embed_base(index as u64))
                })
            });
            assert!(matches!(
                result,
                Err(Error::QueryIndexOutOfRange { index: 3, len: 65 })
            ));
            assert!(visited[64].load(Ordering::SeqCst));
            assert_eq!(values[64], F::embed_base(64));
        }
        let mut output = [F::ONE];
        assert!(
            evaluate_parallel_rows(&mut output, &mut [(); NUMERATOR_JOBS - 1], |_, _| panic!(
                "invalid shape executed"
            ))
            .is_err()
        );
        assert!(
            evaluate_parallel_rows(&mut [], &mut [(); NUMERATOR_JOBS], |_, _| panic!(
                "empty shape executed"
            ))
            .is_err()
        );
        assert_eq!(output, [F::ONE]);
    }

    #[test]
    fn parallel_owned_workspaces_erase_on_failure_and_unwind() {
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };
        use zeroize::Zeroize;
        #[derive(Default)]
        struct Tracked {
            value: u64,
            cleared: Option<Arc<AtomicUsize>>,
        }
        impl Zeroize for Tracked {
            fn zeroize(&mut self) {
                self.value.zeroize();
                if let Some(cleared) = &self.cleared {
                    assert_eq!(self.value, 0);
                    cleared.fetch_add(1, Ordering::SeqCst);
                }
            }
        }
        for panic in [false, true] {
            let cleared = Arc::new(AtomicUsize::new(0));
            let outcome = std::panic::catch_unwind(|| {
                let mut workspaces = (0..NUMERATOR_JOBS)
                    .map(|_| {
                        let mut scratch = SecretPolynomial::<Tracked>::zeroed(2).unwrap();
                        for value in scratch.iter_mut() {
                            value.value = 71;
                            value.cleared = Some(Arc::clone(&cleared));
                        }
                        scratch
                    })
                    .collect::<Vec<_>>();
                let mut values = SecretPolynomial::zeroed(65).unwrap();
                let result = evaluate_parallel_rows(&mut values, &mut workspaces, |index, _| {
                    if index == 0 {
                        assert!(!panic, "test-only parallel failure");
                        return Err(invalid("test-only row failure"));
                    }
                    Ok(F::ONE)
                });
                assert!(result.is_err());
            });
            assert_eq!(outcome.is_err(), panic);
            assert_eq!(cleared.load(Ordering::SeqCst), 2 * NUMERATOR_JOBS);
        }
    }

    #[test]
    fn parallel_scratch_is_charged_before_allocation_independently_of_pool_size() {
        use crate::gadgets::compact_smt_air::{PublicStatement, PublicUpdate};
        let statement = PublicStatement {
            updates: [PublicUpdate {
                old_leaf: [0; 8],
                new_leaf: [0; 8],
                path: 1,
            }; 2],
            old_root: [0; 8],
            new_root: [0; 8],
        };
        let air = CompactTransferAir::new(&statement, None).unwrap();
        let coefficient = [F::ONE];
        let columns = [&coefficient[..]; COLUMN_COUNT];
        let rows = 4 * PHYSICAL_ROW_COUNT;
        let extent = 2 * PHYSICAL_ROW_COUNT;
        let mut policy = limits();
        policy.max_payload_bytes = usize::MAX;
        policy.max_work_units = usize::MAX;
        let make = |policy| {
            MaskedQuotientPlan::from_coefficients(
                &air,
                &columns,
                &[1; COLUMN_COUNT],
                rows,
                F::embed_base(7),
                extent,
                policy,
            )
        };
        let plan = make(policy).unwrap();
        let public = air.polynomial_preparation_cost(plan.domain).unwrap();
        let cells = COLUMN_COUNT
            + COLUMN_COUNT * rows
            + 5 * rows
            + extent
            + SLOT_COUNT
            + NUMERATOR_JOBS * (SLOT_COUNT + 2 * COLUMN_COUNT)
            + (NUMERATOR_JOBS - 1) * public.scratch_cells;
        let descriptors = NUMERATOR_JOBS
            * (core::mem::size_of::<NumeratorScratch<'_, '_>>()
                + core::mem::size_of::<Result<()>>());
        assert_eq!(
            plan.payload_bytes(),
            cells * F::BYTES + public.payload_bytes + descriptors
        );
        let expected = (plan.payload_bytes(), plan.work_units());
        for threads in [1, 6] {
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .unwrap();
            assert_eq!(
                pool.install(|| {
                    let plan = make(policy).unwrap();
                    (plan.payload_bytes(), plan.work_units())
                }),
                expected
            );
        }
        assert!(
            make(MaskedQuotientLimits {
                max_payload_bytes: expected.0,
                max_work_units: expected.1,
                ..policy
            })
            .is_ok()
        );
        for policy in [
            MaskedQuotientLimits {
                max_payload_bytes: expected.0 - 1,
                ..policy
            },
            MaskedQuotientLimits {
                max_work_units: expected.1 - 1,
                ..policy
            },
        ] {
            assert!(matches!(
                make(policy),
                Err(Error::VerifierLimitExceeded { .. })
            ));
        }
    }

    #[test]
    fn last_column_coordinate_preflight_precedes_every_private_transform() {
        let mut policy = limits();
        policy.max_payload_bytes = usize::try_from(2_u64 * 1024 * 1024 * 1024).unwrap();
        policy.max_work_units = usize::MAX;
        let trace = vec![F::ZERO; PHYSICAL_ROW_COUNT];
        let valid_mask = [F::ONE];
        let shape = MaskingShape {
            trace_coefficients: PHYSICAL_ROW_COUNT,
            trace_degree_bound: PHYSICAL_ROW_COUNT,
            mask_coefficients: 1,
            mask_degree_bound: 1,
        };
        let shapes = vec![shape; COLUMN_COUNT];
        for coordinate in 0..4 {
            let mut words = [0; 4];
            words[coordinate] = super::super::GOLDILOCKS_MODULUS;
            let bad = F::from_coefficients_unchecked_for_test(words);
            let bad_mask = [bad];
            let trace_refs = vec![&trace[..]; COLUMN_COUNT];
            let mut mask_refs = vec![&valid_mask[..]; COLUMN_COUNT];
            mask_refs[COLUMN_COUNT - 1] = &bad_mask;
            assert!(matches!(
                PreparedMaskedTrace::prepare(&trace_refs, &mask_refs, &shapes, policy),
                Err(Error::NonCanonicalGoldilocksElement {
                    context: "masked_trace_private_mask",
                    ..
                })
            ));
            mask_refs[COLUMN_COUNT - 1] = &valid_mask;
            let mut bad_trace = trace.clone();
            bad_trace[PHYSICAL_ROW_COUNT - 1] = bad;
            let mut trace_refs = trace_refs;
            trace_refs[COLUMN_COUNT - 1] = &bad_trace;
            assert!(matches!(
                PreparedMaskedTrace::prepare(&trace_refs, &mask_refs, &shapes, policy),
                Err(Error::NonCanonicalGoldilocksElement {
                    context: "masked_trace_subgroup_values",
                    ..
                })
            ));
        }
    }

    #[test]
    fn exact_complete_shapes_and_resource_arithmetic_reject_before_fft() {
        assert!(PreparedMaskedTrace::prepare(&[], &[], &[], limits()).is_err());
        let shape = MaskingShape {
            trace_coefficients: PHYSICAL_ROW_COUNT,
            trace_degree_bound: PHYSICAL_ROW_COUNT,
            mask_coefficients: 1,
            mask_degree_bound: 1,
        };
        let shapes = vec![shape; COLUMN_COUNT];
        let empty: [&[F]; COLUMN_COUNT] = [&[]; COLUMN_COUNT];
        assert!(PreparedMaskedTrace::prepare(&empty, &empty, &shapes, limits()).is_err());
        assert!(checked_add(usize::MAX, 1).is_err());
        assert!(checked_mul(usize::MAX, 2).is_err());
        assert!(transform_work(0).is_err());
        assert!(transform_work(3).is_err());
        assert!(transform_work(1_usize << (usize::BITS - 1)).is_err());
        assert_eq!(transform_work(8).unwrap(), 64 * 8 * 4 + 4096);
        assert!(check_resources(limits(), (1 << 30) + 1, 0).is_err());
        assert!(check_resources(limits(), 0, (1 << 30) + 1).is_err());
        assert!(check_resources(limits(), 1 << 30, 1 << 30).is_ok());
    }
}
