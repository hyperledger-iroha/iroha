//! Bounded base-field vanishing-mask replay for the canonical DEEP construction.
//!
//! For a stripe x=a*g^j, x^N=a^N. Thus C(x)+(x^N-1)r(x) is an N-point
//! FFT of C_k+(a^N-1)r_k, twisted by a^k. Only one 301-column stripe is
//! live, even though the full domain has 128 stripes. Public columns never
//! enter this owner. The immutable coefficient view also feeds DEEP composition.
//!
//! Fresh coefficients use caller-supplied cryptographic entropy and canonical
//! rejection sampling, with an explicit, bounded failure path. Owned secrets
//! have fixed zeroizing allocations, no Clone, Debug, seed export or serialization.
//! Borrowed input, incidental arithmetic copies and caller-owned callback buffers
//! remain outside that erasure/accounting guarantee.
//! The producer consumes this owner for commitments and the exact full AIR
//! quotient. TODO: Qualify complete generated artifacts and independently review
//! the transcript security reduction.

use rand::TryCryptoRng;
use rayon::prelude::*;

use super::{
    FriDomain, GOLDILOCKS_MODULUS, add_mod,
    compact_public_columns::{COMMITTED_COLUMN_COUNT, SourceTraceColumns},
    deep_geometry::{COSET_OFFSET, LDE_ROOT, LDE_ROWS, QUERY_COUNT, TRACE_ROWS},
    field_pow, mul_mod,
    polynomial_field::PolynomialField,
    secret_polynomial::SecretPolynomial,
    sub_mod,
};
use crate::{
    Error, Result,
    cyclotomic::{self, Domain},
    field::GoldilocksFp4V1 as F,
};

/// Worst-case base opening closure: two shifts of 64 queries and two Fp4 points.
pub(super) const TRACE_MASK_COEFFICIENTS: usize = 2 * (QUERY_COUNT + F::COEFFICIENTS);
/// Fp4 quotient blinding evaluated at the queries and the first OOD point.
pub(super) const QUOTIENT_MASK_COEFFICIENTS: usize = QUERY_COUNT + 1;

/// Explicit local payload/work policy; not consensus parameters or a proof field.
#[derive(Clone, Copy, Debug)]
pub(super) struct ReplayLimits {
    pub(super) max_payload_bytes: usize,
    pub(super) max_work_units: usize,
    pub(super) max_full_passes: usize,
}

/// Shared exact payload extents and conservative structural work, before entropy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct MaskedReplayPlan {
    rows: usize,
    width: usize,
    trace_mask: usize,
    quotient_mask: usize,
    stripes: usize,
    source_width: usize,
    max_selected: usize,
    domain: FriDomain,
    trace_domain: Domain,
    max_passes: usize,
    /// Includes the borrowed physical source, all owned coefficients/entropy,
    /// one stripe and the maximum simultaneously retained selected-row buffer.
    /// Excludes allocator metadata, Rayon internals and callback-owned buffers.
    pub(super) payload_bytes: usize,
    /// Initialization, validation, bounded RNG attempts, transforms and erasure.
    /// A structural upper bound, not machine instructions, time or peak RSS.
    pub(super) work_units: usize,
    pub(super) coefficient_bytes: usize,
    pub(super) stripe_bytes: usize,
    pub(super) entropy_bytes: usize,
    pub(super) source_bytes: usize,
    pub(super) selected_bytes: usize,
    pub(super) maximum_column_transforms: usize,
    pub(super) entropy_attempts: usize,
}

impl MaskedReplayPlan {
    /// The sole full candidate geometry. No proof can choose these dimensions.
    pub(super) fn new(limits: ReplayLimits) -> Result<Self> {
        Self::with_shape(
            TRACE_ROWS,
            COMMITTED_COLUMN_COUNT,
            342,
            TRACE_MASK_COEFFICIENTS,
            QUOTIENT_MASK_COEFFICIENTS,
            limits,
        )
    }

    pub(super) fn lde_rows(self) -> usize {
        self.rows * self.stripes
    }
    pub(super) fn stripes(self) -> usize {
        self.stripes
    }

    pub(super) fn is_candidate_geometry(self) -> bool {
        self.rows == TRACE_ROWS
            && self.width == COMMITTED_COLUMN_COUNT
            && self.source_width == 342
            && self.trace_mask == TRACE_MASK_COEFFICIENTS
            && self.quotient_mask == QUOTIENT_MASK_COEFFICIENTS
    }

    // Private shape injection is solely for small arithmetic regressions below.
    // Root orientation and 128 stripes remain the actual candidate's geometry.
    fn with_shape(
        rows: usize,
        width: usize,
        source_width: usize,
        trace_mask: usize,
        quotient_mask: usize,
        limits: ReplayLimits,
    ) -> Result<Self> {
        if !rows.is_power_of_two()
            || rows > TRACE_ROWS
            || width == 0
            || width > COMMITTED_COLUMN_COUNT
            || source_width < width
            || source_width > 342
            || trace_mask == 0
            || trace_mask > rows
            || quotient_mask == 0
            || quotient_mask > rows
            || limits.max_full_passes == 0
        {
            return Err(invalid(
                "masked replay requires bounded exact source and mask extents",
            ));
        }
        let stripes = LDE_ROWS / TRACE_ROWS;
        let lde_rows = mul(rows, stripes)?;
        let domain =
            FriDomain::from_lde_parameters(LDE_ROOT, LDE_ROWS.ilog2(), lde_rows, COSET_OFFSET)?;
        let trace_domain = Domain {
            log_size: rows.ilog2(),
            generator: field_pow(domain.generator, stripes as u64),
        };
        let cells = mul(rows, width)?;
        let coefficient_bytes = mul(cells, size_of::<u64>())?;
        let source_bytes = mul(mul(rows, source_width)?, size_of::<u64>())?;
        let entropy_coordinates = add(
            mul(width, trace_mask)?,
            mul(add(quotient_mask, mul(2, rows)?)?, 4)?,
        )?;
        let entropy_bytes = mul(entropy_coordinates, size_of::<u64>())?;
        let max_selected = 2 * QUERY_COUNT;
        let selected_bytes = mul(mul(max_selected, width)?, size_of::<u64>())?;
        let payload_bytes = add(
            add(source_bytes, mul(2, coefficient_bytes)?)?,
            add(entropy_bytes, selected_bytes)?,
        )?;
        // One rejection has probability (2^32-1)/2^64 < 2^-32. This
        // union bound concerns entropy exhaustion only, not proof security.
        let coordinate_log =
            usize::BITS as usize - (entropy_coordinates - 1).leading_zeros() as usize;
        let entropy_attempts = add(
            fastpq_isi::FASTPQ_REQUIRED_SECURITY_BITS_V1 as usize,
            coordinate_log,
        )?
        .div_ceil(32);
        let maximum_column_transforms = mul(width, add(1, mul(stripes, limits.max_full_passes)?)?)?;
        let transforms = mul(maximum_column_transforms, transform_work(rows)?)?;
        let entropy_work = mul(entropy_coordinates, add(entropy_attempts, 3)?)?;
        // Per pass: effective coefficient formation/twist, overwrite/erasure,
        // plus a full maximum selected-row scan/copy for every stripe.
        let replay_work = mul(
            limits.max_full_passes,
            add(
                mul(mul(cells, stripes)?, 8)?,
                mul(mul(max_selected, stripes)?, add(width, 2)?)?,
            )?,
        )?;
        let work_units = add(
            add(transforms, entropy_work)?,
            add(
                replay_work,
                add(mul(mul(rows, source_width)?, 4)?, mul(cells, 3)?)?,
            )?,
        )?;
        limit(
            "max_deep_replay_payload_bytes",
            payload_bytes,
            limits.max_payload_bytes,
        )?;
        limit(
            "max_deep_replay_work_units",
            work_units,
            limits.max_work_units,
        )?;
        limit(
            "max_deep_replay_addressable_bytes",
            payload_bytes,
            isize::MAX as usize,
        )?;
        Ok(Self {
            rows,
            width,
            source_width,
            trace_mask,
            quotient_mask,
            stripes,
            max_selected,
            domain,
            trace_domain,
            max_passes: limits.max_full_passes,
            payload_bytes,
            work_units,
            coefficient_bytes,
            stripe_bytes: coefficient_bytes,
            entropy_bytes,
            source_bytes,
            selected_bytes,
            maximum_column_transforms,
            entropy_attempts,
        })
    }
}

/// Move-only fresh masks retained with the replay that consumes them.
struct ReplayEntropy {
    trace: SecretPolynomial<u64>,
    quotient: SecretPolynomial<F>,
    composition: SecretPolynomial<F>,
}

impl ReplayEntropy {
    fn sample(plan: MaskedReplayPlan, rng: &mut impl TryCryptoRng) -> Result<Self> {
        let mut trace = SecretPolynomial::zeroed(plan.width * plan.trace_mask)?;
        let mut quotient = SecretPolynomial::zeroed(plan.quotient_mask)?;
        let mut composition = SecretPolynomial::zeroed(2 * plan.rows)?;
        for value in trace.iter_mut() {
            *value = sample_base(rng, plan.entropy_attempts)?;
        }
        for value in quotient.iter_mut().chain(composition.iter_mut()) {
            let mut coordinates = zeroize::Zeroizing::new([0; 4]);
            for coordinate in coordinates.iter_mut() {
                *coordinate = sample_base(rng, plan.entropy_attempts)?;
            }
            *value = F::new(*coordinates).expect("rejection-sampled canonical coordinates");
        }
        Ok(Self {
            trace,
            quotient,
            composition,
        })
    }
}

fn sample_base(rng: &mut impl TryCryptoRng, attempts: usize) -> Result<u64> {
    for _ in 0..attempts {
        let value = rng
            .try_next_u64()
            .map_err(|_| invalid("DEEP private entropy source failed"))?;
        if value < GOLDILOCKS_MODULUS {
            return Ok(value);
        }
    }
    Err(invalid("DEEP private entropy rejection budget exhausted"))
}

/// Retained unmasked coefficients plus fresh private masks; no duplicated masked matrix.
pub(super) struct MaskedTraceReplay {
    plan: MaskedReplayPlan,
    coefficients: SecretPolynomial<u64>,
    entropy: ReplayEntropy,
    remaining_passes: usize,
}

/// A borrowed stripe with current/next rows indexed in the same execution subgroup.
pub(super) struct MaskedStripe<'a> {
    plan: MaskedReplayPlan,
    stripe: usize,
    values: &'a [u64],
}

impl MaskedStripe<'_> {
    pub(super) fn stripe_index(&self) -> usize {
        self.stripe
    }
    pub(super) fn rows(&self) -> usize {
        self.plan.rows
    }
    pub(super) fn global_index(&self, row: usize) -> usize {
        assert!(row < self.plan.rows);
        self.stripe + row * self.plan.stripes
    }
    pub(super) fn point(&self, row: usize) -> u64 {
        self.plan.domain.point(self.global_index(row))
    }
    pub(super) fn fill_row(&self, row: usize, output: &mut [u64]) -> Result<()> {
        if row >= self.plan.rows || output.len() != self.plan.width {
            return Err(invalid("masked stripe requires an exact bounded row"));
        }
        for (value, column) in output
            .iter_mut()
            .zip(self.values.chunks_exact(self.plan.rows))
        {
            *value = column[row];
        }
        Ok(())
    }
}

/// Selected rows retain clearing ownership until the final proof DTO is constructed.
#[cfg(test)]
pub(super) struct SelectedMaskedRows {
    width: usize,
    values: SecretPolynomial<u64>,
}
#[cfg(test)]
impl SelectedMaskedRows {
    pub(super) fn rows(&self) -> impl Iterator<Item = &[u64]> {
        self.values.chunks_exact(self.width)
    }
}

impl MaskedTraceReplay {
    /// Resource planning precedes complete source validation and entropy sampling.
    pub(super) fn new(
        limits: ReplayLimits,
        columns: &[&[u64]],
        rng: &mut impl TryCryptoRng,
    ) -> Result<Self> {
        let plan = MaskedReplayPlan::new(limits)?;
        let source = SourceTraceColumns::new(columns)?;
        Self::from_validated_columns(
            plan,
            |index| {
                source
                    .committed_column(index)
                    .expect("fixed projection index")
            },
            rng,
        )
    }

    fn from_validated_columns<'a>(
        plan: MaskedReplayPlan,
        column: impl Fn(usize) -> &'a [u64] + Sync,
        rng: &mut impl TryCryptoRng,
    ) -> Result<Self> {
        let entropy = ReplayEntropy::sample(plan, rng)?;
        let mut coefficients = SecretPolynomial::zeroed(plan.coefficient_bytes / size_of::<u64>())?;
        coefficients
            .par_chunks_mut(plan.rows)
            .enumerate()
            .for_each(|(index, output)| {
                output.copy_from_slice(column(index));
                cyclotomic::ifft(output, plan.trace_domain);
            });
        Ok(Self {
            plan,
            coefficients,
            entropy,
            remaining_passes: plan.max_passes,
        })
    }

    /// Small arithmetic fixture only; rejected by every fixed candidate entry point.
    #[cfg(test)]
    pub(super) fn arithmetic_fixture(
        columns: &[&[u64]],
        rows: usize,
        passes: usize,
    ) -> Result<Self> {
        use rand::SeedableRng;
        if rows > 16 || columns.iter().any(|column| column.len() != rows) {
            return Err(invalid(
                "small masked arithmetic fixture has invalid dimensions",
            ));
        }
        let plan = MaskedReplayPlan::with_shape(
            rows,
            columns.len(),
            columns.len(),
            rows.min(3),
            rows.min(2),
            ReplayLimits {
                max_payload_bytes: usize::MAX,
                max_work_units: usize::MAX,
                max_full_passes: passes,
            },
        )?;
        for (column, values) in columns.iter().enumerate() {
            for (row, &value) in values.iter().enumerate() {
                value.validate("deep_arithmetic_fixture", &[column, row])?;
            }
        }
        let mut rng = rand::rngs::StdRng::from_seed([91; 32]);
        Self::from_validated_columns(plan, |column| columns[column], &mut rng)
    }

    /// Fixed candidate coefficient view; excludes the smaller unit-test geometries.
    pub(super) fn has_candidate_geometry(&self) -> bool {
        self.plan.is_candidate_geometry()
    }
    pub(super) fn width(&self) -> usize {
        self.plan.width
    }
    pub(super) fn coefficient_extent(&self) -> usize {
        self.plan.rows + self.plan.trace_mask
    }
    pub(super) fn coefficient(&self, column: usize, degree: usize) -> u64 {
        assert!(column < self.plan.width && degree < self.coefficient_extent());
        if degree >= self.plan.rows {
            self.entropy.trace[column * self.plan.trace_mask + degree - self.plan.rows]
        } else {
            let value = self.coefficients[column * self.plan.rows + degree];
            if degree < self.plan.trace_mask {
                sub_mod(
                    value,
                    self.entropy.trace[column * self.plan.trace_mask + degree],
                )
            } else {
                value
            }
        }
    }
    pub(super) fn composition_mask(&self) -> &[F] {
        &self.entropy.composition
    }
    pub(super) fn quotient_mask(&self) -> &[F] {
        &self.entropy.quotient
    }
    pub(super) fn ensure_pass_available(&self) -> Result<()> {
        if self.remaining_passes == 0 {
            Err(invalid("DEEP masked replay pass budget exhausted"))
        } else {
            Ok(())
        }
    }

    pub(super) fn plan(&self) -> MaskedReplayPlan {
        self.plan
    }

    /// Charge a full preplanned pass, also on a callback/allocator error.
    pub(super) fn visit_all(
        &mut self,
        visit: impl FnMut(MaskedStripe<'_>) -> Result<()>,
    ) -> Result<()> {
        self.replay_selected(|_| true, visit)
    }
    /// Replay an exact nested source-root domain using the same stripe arithmetic.
    /// The numerator uses 4N points (four stripes); the commitment domain uses 128N.
    pub(super) fn visit_subdomain(
        &mut self,
        rows: usize,
        mut visit: impl FnMut(MaskedStripe<'_>, usize) -> Result<()>,
    ) -> Result<()> {
        let full = self.plan.rows * self.plan.stripes;
        if !rows.is_power_of_two() || rows < self.plan.rows || rows > full {
            return Err(invalid(
                "masked replay subdomain must be an exact nested subgroup",
            ));
        }
        let step = full / rows;
        self.replay_selected(|stripe| stripe % step == 0, |stripe| visit(stripe, step))
    }

    /// Regenerate exact caller-selected natural rows, charging one full pass.
    pub(super) fn visit_selected_stripes(
        &mut self,
        indices: &[usize],
        visit: impl FnMut(MaskedStripe<'_>) -> Result<()>,
    ) -> Result<()> {
        if indices.is_empty()
            || indices.len() > self.plan.max_selected
            || indices.iter().any(|&i| i >= self.plan.lde_rows())
            || indices.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Err(invalid(
                "selected masked rows must be sorted, unique and bounded",
            ));
        }
        let mut selected = [false; LDE_ROWS / TRACE_ROWS];
        for &index in indices {
            selected[index % self.plan.stripes] = true;
        }
        self.replay_selected(|stripe| selected[stripe], visit)
    }
    fn replay_selected(
        &mut self,
        selected: impl Fn(usize) -> bool,
        mut visit: impl FnMut(MaskedStripe<'_>) -> Result<()>,
    ) -> Result<()> {
        self.ensure_pass_available()?;
        self.remaining_passes -= 1;
        let mut values = SecretPolynomial::zeroed(self.plan.stripe_bytes / size_of::<u64>())?;
        for stripe in 0..self.plan.stripes {
            if !selected(stripe) {
                continue;
            }
            let offset = self.plan.domain.point(stripe);
            let vanishing = sub_mod(field_pow(offset, self.plan.rows as u64), 1);
            values
                .par_chunks_mut(self.plan.rows)
                .zip(self.coefficients.par_chunks(self.plan.rows))
                .zip(self.entropy.trace.par_chunks(self.plan.trace_mask))
                .for_each(|((output, coefficients), mask)| {
                    let mut power = 1;
                    for (degree, (value, &coefficient)) in
                        output.iter_mut().zip(coefficients).enumerate()
                    {
                        let effective = if degree < mask.len() {
                            add_mod(coefficient, mul_mod(vanishing, mask[degree]))
                        } else {
                            coefficient
                        };
                        *value = mul_mod(effective, power);
                        power = mul_mod(power, offset);
                    }
                    cyclotomic::fft(output, self.plan.trace_domain);
                });
            visit(MaskedStripe {
                plan: self.plan,
                stripe,
                values: &values,
            })?;
        }
        Ok(())
    }

    /// At most 128 rows, preserving duplicates/order and replaying each stripe once.
    #[cfg(test)]
    pub(super) fn selected_rows(&mut self, indices: &[usize]) -> Result<SelectedMaskedRows> {
        if indices.len() > self.plan.max_selected
            || indices
                .iter()
                .any(|&i| i >= self.plan.rows * self.plan.stripes)
        {
            return Err(invalid(
                "DEEP masked row selection exceeds the fixed query budget",
            ));
        }
        let width = self.plan.width;
        let mut values = SecretPolynomial::zeroed(indices.len() * width)?;
        if !indices.is_empty() {
            let mut selected = [false; LDE_ROWS / TRACE_ROWS];
            for &index in indices {
                selected[index % self.plan.stripes] = true;
            }
            self.replay_selected(
                |stripe| selected[stripe],
                |stripe| {
                    for (&index, row) in indices.iter().zip(values.chunks_exact_mut(width)) {
                        if index % stripe.plan.stripes == stripe.stripe {
                            stripe.fill_row(index / stripe.plan.stripes, row)?;
                        }
                    }
                    Ok(())
                },
            )?;
        }
        Ok(SelectedMaskedRows { width, values })
    }
}

fn transform_work(rows: usize) -> Result<usize> {
    add(mul(rows, add(mul(12, rows.ilog2() as usize)?, 16)?)?, 4096)
}
fn add(a: usize, b: usize) -> Result<usize> {
    a.checked_add(b)
        .ok_or_else(|| invalid("DEEP replay resource addition overflow"))
}
fn mul(a: usize, b: usize) -> Result<usize> {
    a.checked_mul(b)
        .ok_or_else(|| invalid("DEEP replay resource multiplication overflow"))
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
#[path = "deep_masked_replay/tests.rs"]
mod tests;
