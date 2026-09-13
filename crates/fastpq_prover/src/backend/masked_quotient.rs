//! Explicit masked trace preparation and the full coefficient AIR quotient.
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

use super::{
    air_degree::{AirDegreeBounds, SLOT_COUNT},
    coefficient_masking::{MaskingLimits, MaskingPlan, MaskingShape},
    compact_transfer_air::CompactTransferAir,
    polynomial_division::{ExactQuotient, VanishingDivisionPlan},
    polynomial_field::PolynomialField,
    polynomial_transform::{PolynomialDomain, PolynomialLanes, reserved, validate_coefficients},
    secret_polynomial::SecretPolynomial,
};
use crate::gadgets::compact_smt_air::{COLUMN_COUNT, PHYSICAL_ROW_COUNT};
use crate::{Error, Result, field::GoldilocksFp4V1 as F};

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
    pub(super) max_mask_coefficients: usize,
    /// Maximum exact masked coefficient extent per column, including supplied padding.
    pub(super) max_masked_coefficients: usize,
}

/// Prepared private coefficients for the exact complete trace; no Debug or Clone.
pub(super) struct PreparedMaskedTrace {
    columns: Vec<SecretPolynomial<F>>,
    degree_bounds: [usize; COLUMN_COUNT],
    coefficient_cells: usize,
}

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
            coefficient_cells,
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

/// Borrow-bound full numerator plan, with no witness-bearing Debug representation.
pub(super) struct MaskedQuotientPlan<'a> {
    air: &'a CompactTransferAir,
    trace: &'a PreparedMaskedTrace,
    domain: PolynomialDomain,
    rotation: usize,
    degrees: AirDegreeBounds,
    division: VanishingDivisionPlan,
    payload_bytes: usize,
    work_units: usize,
}

impl<'a> MaskedQuotientPlan<'a> {
    /// Bind actual AIR/trace owners and explicit arithmetic geometry before allocating LDEs.
    pub(super) fn new(
        air: &'a CompactTransferAir,
        trace: &'a PreparedMaskedTrace,
        interpolation_rows: usize,
        offset: F,
        quotient_extent: usize,
        limits: MaskedQuotientLimits,
    ) -> Result<Self> {
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
        let private_cells = checked_add(
            trace.coefficient_cells,
            checked_add(
                trace_lanes,
                checked_add(
                    checked_mul(5, interpolation_rows)?,
                    checked_add(quotient_extent, 2 * SLOT_COUNT + 2 * COLUMN_COUNT)?,
                )?,
            )?,
        )?;
        let payload_bytes =
            checked_add(checked_mul(private_cells, F::BYTES)?, public.payload_bytes)?;
        let transform_work = checked_mul(COLUMN_COUNT + 1, transform_work(interpolation_rows)?)?;
        let mut work_units = checked_add(transform_work, public.work_units)?;
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
            work_units,
        })
    }

    /// Declared simultaneous payload bound, not allocator overhead or measured peak RSS.
    pub(super) const fn payload_bytes(&self) -> usize {
        self.payload_bytes
    }

    /// Declared structural work including cleanup of private owned buffers.
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
        let mut evaluator = self.air.prepare_polynomial_evaluator(self.domain)?;
        let mut residues = SecretPolynomial::zeroed(SLOT_COUNT)?;
        let mut values = SecretPolynomial::zeroed(self.domain.rows())?;
        let mut current = SecretPolynomial::zeroed(COLUMN_COUNT)?;
        let mut next = SecretPolynomial::zeroed(COLUMN_COUNT)?;
        for index in 0..self.domain.rows() {
            let next_index = (index + self.rotation) % self.domain.rows();
            for column in 0..COLUMN_COUNT {
                current[column] = columns[column].value(index)?;
                next[column] = columns[column].value(next_index)?;
            }
            evaluator.evaluate_into(index, &current, &next, &mut residues)?;
            values[index] = residues
                .iter()
                .zip(alpha)
                .fold(F::ZERO, |sum, (&value, &weight)| sum.add(value.mul(weight)));
        }
        let numerator = self.domain.interpolate(&values)?;
        validate_coefficients(
            &numerator,
            self.degrees.combined_numerator(),
            "masked_full_numerator_coefficients",
        )?;
        let quotient = self.division.divide(&numerator)?;
        Ok(MaskedAirQuotient {
            numerator,
            quotient,
            domain: self.domain,
            degrees: self.degrees,
        })
    }
}

/// Owned private full numerator and exact quotient; no Debug or raw buffer transfer.
pub(super) struct MaskedAirQuotient {
    numerator: SecretPolynomial<F>,
    quotient: ExactQuotient,
    domain: PolynomialDomain,
    degrees: AirDegreeBounds,
}

impl MaskedAirQuotient {
    /// Borrow full numerator coefficients including required high zero padding.
    pub(super) fn numerator(&self) -> &[F] {
        &self.numerator
    }
    /// Borrow the exact quotient whose full remainder was checked.
    pub(super) fn quotient(&self) -> &ExactQuotient {
        &self.quotient
    }
    /// Public arithmetic domain retained for reproducible independent checking.
    pub(super) const fn domain(&self) -> PolynomialDomain {
        self.domain
    }
    /// Source-derived full polynomial degree obligations, not a PCS proof.
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
