//! Bounded Fp4 coefficient replay and exact existing FRI coefficient folding.
//!
//! Every candidate layer has M/D=64. Four base-field D-point FFTs evaluate
//! one Fp4 polynomial on a stripe. Grouped FRI fibers remain within that stripe,
//! so neither commitments nor openings need a full M-element oracle allocation.
//! Consecutive arity-r coefficient blocks fold by Horner at beta, matching the
//! existing inverse-fiber transform exactly. This adds storage scheduling only.
//! TODO: Integrate the complete transcript and qualify the private producer;
//! these private arithmetic owners do not activate any proof format or gate.

use super::{
    FriDomain,
    deep_geometry::{COSET_OFFSET, FRI_ARITIES, FRI_DEGREES, FRI_LENGTHS, LDE_ROOT, LDE_ROWS},
    field_pow, mul_mod,
    polynomial_field::PolynomialField,
    secret_polynomial::SecretPolynomial,
};
use crate::{
    Error, Result,
    cyclotomic::{self, Domain},
    field::GoldilocksFp4V1 as F,
};
use rayon::prelude::*;

/// Local exact payload and conservative structural work limits.
#[derive(Clone, Copy, Debug)]
pub(super) struct CoefficientLimits {
    pub(super) max_payload_bytes: usize,
    pub(super) max_work_units: usize,
    pub(super) max_full_passes: usize,
}

/// Trusted layer geometry; callers cannot supply roots, offsets or fold degrees.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct CoefficientReplayPlan {
    degree: usize,
    rows: usize,
    width: usize,
    stripes: usize,
    arity: usize,
    domain: FriDomain,
    fft: Domain,
    max_passes: usize,
    pub(super) payload_bytes: usize,
    pub(super) work_units: usize,
    pub(super) stripe_bytes: usize,
    pub(super) maximum_column_transforms: usize,
}
impl CoefficientReplayPlan {
    /// Three complete oracles committed together before z: Q0, Q1 and R.
    pub(super) fn quotient_and_mask(limits: CoefficientLimits) -> Result<Self> {
        Self::with_shape(FRI_LENGTHS[0], FRI_DEGREES[0], 3, 1, limits)
    }
    /// One existing FRI source layer, with its exact grouped-leaf arity.
    pub(super) fn fri(round: usize, limits: CoefficientLimits) -> Result<Self> {
        if round >= FRI_ARITIES.len() {
            return Err(invalid(
                "DEEP coefficient replay round is outside the fixed FRI schedule",
            ));
        }
        Self::with_shape(
            FRI_LENGTHS[round],
            FRI_DEGREES[round],
            1,
            FRI_ARITIES[round],
            limits,
        )
    }
    /// Complete terminal polynomial (degree below two) on its 128-point domain.
    pub(super) fn terminal(limits: CoefficientLimits) -> Result<Self> {
        Self::with_shape(FRI_LENGTHS[5], FRI_DEGREES[5], 1, 1, limits)
    }
    fn with_shape(
        rows: usize,
        degree: usize,
        width: usize,
        arity: usize,
        limits: CoefficientLimits,
    ) -> Result<Self> {
        if !rows.is_power_of_two()
            || !degree.is_power_of_two()
            || rows > LDE_ROWS
            || rows / degree != 64
            || rows < degree
            || !matches!(width, 1 | 3)
            || !matches!(arity, 1 | 2 | 4 | 8 | 16)
            || arity > degree
            || limits.max_full_passes == 0
        {
            return Err(invalid(
                "DEEP coefficient replay needs a fixed bounded source-root layer",
            ));
        }
        let stripes = rows / degree;
        let exponent = LDE_ROWS / rows;
        let domain = FriDomain::from_lde_parameters(
            LDE_ROOT,
            LDE_ROWS.ilog2(),
            rows,
            field_pow(COSET_OFFSET, exponent as u64),
        )?;
        let fft = Domain {
            log_size: degree.ilog2(),
            generator: field_pow(domain.generator, stripes as u64),
        };
        let stripe_bytes = mul(mul(width, degree)?, F::BYTES)?;
        // At most one complete borrowed coefficient vector per lane and one
        // fixed four-base-lane stripe. Callback/commitment/output buffers are
        // separately charged by the enclosing phase plan.
        let payload_bytes = mul(2, stripe_bytes)?;
        let maximum_column_transforms = mul(mul(width, 4)?, mul(stripes, limits.max_full_passes)?)?;
        let work_units = add(
            mul(
                maximum_column_transforms,
                add(
                    mul(degree, add(mul(12, degree.ilog2() as usize)?, 16)?)?,
                    4096,
                )?,
            )?,
            mul(width * degree, 4)?,
        )?;
        limit(
            "max_deep_coefficient_payload_bytes",
            payload_bytes,
            limits.max_payload_bytes,
        )?;
        limit(
            "max_deep_coefficient_work_units",
            work_units,
            limits.max_work_units,
        )?;
        limit(
            "max_deep_coefficient_addressable_bytes",
            payload_bytes,
            isize::MAX as usize,
        )?;
        Ok(Self {
            degree,
            rows,
            width,
            stripes,
            arity,
            domain,
            fft,
            max_passes: limits.max_full_passes,
            payload_bytes,
            work_units,
            stripe_bytes,
            maximum_column_transforms,
        })
    }
    pub(super) fn width(self) -> usize {
        self.width
    }
    pub(super) fn rows(self) -> usize {
        self.rows
    }
    pub(super) fn degree(self) -> usize {
        self.degree
    }
    pub(super) fn stripes(self) -> usize {
        self.stripes
    }
    pub(super) fn arity(self) -> usize {
        self.arity
    }
    pub(super) fn domain(self) -> FriDomain {
        self.domain
    }
}

/// Immutable borrowed sources, with complete canonicality checked before any FFT.
pub(super) struct CoefficientReplay<'a> {
    plan: CoefficientReplayPlan,
    sources: &'a [&'a [F]],
    remaining_passes: usize,
}
impl<'a> CoefficientReplay<'a> {
    pub(super) fn new(plan: CoefficientReplayPlan, sources: &'a [&'a [F]]) -> Result<Self> {
        if sources.len() != plan.width || sources.iter().any(|source| source.len() > plan.degree) {
            return Err(invalid(
                "DEEP replay source coefficients exceed fixed width or extent",
            ));
        }
        for (column, source) in sources.iter().enumerate() {
            for (degree, &value) in source.iter().enumerate() {
                value.validate("deep_replay_coefficients", &[column, degree])?;
            }
        }
        Ok(Self {
            plan,
            sources,
            remaining_passes: plan.max_passes,
        })
    }
    pub(super) fn plan(&self) -> CoefficientReplayPlan {
        self.plan
    }
    pub(super) fn ensure_pass_available(&self) -> Result<()> {
        if self.remaining_passes == 0 {
            Err(invalid("DEEP coefficient replay pass budget exhausted"))
        } else {
            Ok(())
        }
    }
    pub(super) fn visit_all(
        &mut self,
        mut visit: impl FnMut(CoefficientStripe<'_>) -> Result<()>,
    ) -> Result<()> {
        self.ensure_pass_available()?;
        self.remaining_passes -= 1;
        let mut lanes = SecretPolynomial::<u64>::zeroed(self.plan.stripe_bytes / 8)?;
        for stripe in 0..self.plan.stripes {
            let offset = self.plan.domain.point(stripe);
            lanes
                .par_chunks_mut(self.plan.degree)
                .enumerate()
                .for_each(|(lane, output)| {
                    let source = self.sources[lane / 4];
                    let coordinate = lane % 4;
                    let mut power = 1;
                    for (degree, value) in output.iter_mut().enumerate() {
                        *value = mul_mod(
                            source
                                .get(degree)
                                .map_or(0, |value| value.coefficients()[coordinate]),
                            power,
                        );
                        power = mul_mod(power, offset);
                    }
                    cyclotomic::fft(output, self.plan.fft);
                });
            visit(CoefficientStripe {
                plan: self.plan,
                stripe,
                lanes: &lanes,
            })?;
        }
        Ok(())
    }
}

/// Complete polynomials' values at one stripe; every fiber is already resident.
pub(super) struct CoefficientStripe<'a> {
    plan: CoefficientReplayPlan,
    stripe: usize,
    lanes: &'a [u64],
}
impl CoefficientStripe<'_> {
    pub(super) fn rows(&self) -> usize {
        self.plan.degree
    }
    pub(super) fn global_index(&self, row: usize) -> usize {
        assert!(row < self.plan.degree);
        self.stripe + row * self.plan.stripes
    }
    pub(super) fn value(&self, column: usize, row: usize) -> Result<F> {
        if column >= self.plan.width || row >= self.plan.degree {
            return Err(invalid("DEEP coefficient stripe index exceeds its shape"));
        }
        Ok(F::new(core::array::from_fn(|coordinate| {
            self.lanes[(column * 4 + coordinate) * self.plan.degree + row]
        }))
        .expect("base FFT preserves canonical coordinates"))
    }
    pub(super) fn fiber_rows(&self) -> usize {
        self.plan.degree / self.plan.arity
    }
    /// A grouped leaf is [f(x), f(x*omega), ...] in the verifier's exact order.
    pub(super) fn fiber(&self, row: usize, output: &mut [F]) -> Result<()> {
        if self.plan.width != 1 || row >= self.fiber_rows() || output.len() != self.plan.arity {
            return Err(invalid(
                "DEEP FRI fiber needs its exact single-oracle shape",
            ));
        }
        for (position, value) in output.iter_mut().enumerate() {
            *value = self.value(0, row + position * self.fiber_rows())?;
        }
        Ok(())
    }
}

/// Fold exact coefficient blocks, retaining full Fp4 challenges and zero padding.
/// This is the coefficient form of FriFoldPlan::fold_coset, not a new FRI rule.
pub(super) fn fold_coefficients(
    round: usize,
    coefficients: &[F],
    beta: F,
    max_payload_bytes: usize,
) -> Result<SecretPolynomial<F>> {
    if round >= FRI_ARITIES.len() || coefficients.len() != FRI_DEGREES[round] {
        return Err(invalid(
            "DEEP FRI coefficient fold requires its exact complete source extent",
        ));
    }
    let output = FRI_DEGREES[round + 1];
    let bytes = mul(add(coefficients.len(), output)?, F::BYTES)?;
    limit("max_deep_fold_payload_bytes", bytes, max_payload_bytes)?;
    beta.validate("deep_coefficient_fold_beta", &[])?;
    for (degree, &value) in coefficients.iter().enumerate() {
        value.validate("deep_coefficient_fold_source", &[degree])?;
    }
    let mut result = SecretPolynomial::zeroed(output)?;
    for (destination, block) in result
        .iter_mut()
        .zip(coefficients.chunks_exact(FRI_ARITIES[round]))
    {
        *destination = block
            .iter()
            .rev()
            .fold(F::ZERO, |sum, &value| sum.mul(beta).add(value));
    }
    Ok(result)
}

fn add(a: usize, b: usize) -> Result<usize> {
    a.checked_add(b)
        .ok_or_else(|| invalid("DEEP coefficient resource overflow"))
}
fn mul(a: usize, b: usize) -> Result<usize> {
    a.checked_mul(b)
        .ok_or_else(|| invalid("DEEP coefficient resource overflow"))
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
#[path = "deep_coefficient_replay/tests.rs"]
mod tests;
