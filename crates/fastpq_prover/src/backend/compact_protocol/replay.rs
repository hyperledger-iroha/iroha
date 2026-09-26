//! Bounded trace evaluation for the existing unmasked compact candidate.
//!
//! For M = B*N, stripe s contains the natural LDE indices s+B*j. Twisting
//! the N coefficients by (omega*root_M^s)^k and using the existing N-point
//! FFT gives exactly those evaluations. A next-trace-row rotation by B is
//! therefore a cyclic next row within the same stripe. Only one stripe is
//! live at a time; digest trees and scalar/extension oracles have separate
//! charges. This changes storage and replay work, never masking or the wire.

use fastpq_isi::FASTPQ_FINAL_V1;
use rayon::prelude::*;

use super::{canonical_base, collect_prover_rows, shape};
use crate::{
    Result,
    backend::{
        FriDomain, fixed_domain::FixedTraceDomain, mul_mod, secret_polynomial::SecretPolynomial,
    },
    cyclotomic::{self, Domain},
};

/// Shared allocation and work preflight, with no private data or allocation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::backend) struct TraceReplayPlan {
    pub(in crate::backend) trace_rows: usize,
    pub(in crate::backend) width: usize,
    pub(in crate::backend) lde_rows: usize,
    pub(in crate::backend) stripes: usize,
    /// Exact payload of the retained coefficient matrix.
    pub(in crate::backend) coefficient_bytes: usize,
    /// Exact payload of the single reusable stripe allocation.
    pub(in crate::backend) stripe_bytes: usize,
    /// Exact simultaneously owned trace payload: coefficients plus one stripe.
    pub(in crate::backend) peak_trace_bytes: usize,
    /// One IFFT per column, plus at most four complete stripe passes.
    pub(in crate::backend) maximum_column_transforms: usize,
    trace_domain: Domain,
    lde_domain: FriDomain,
}

impl TraceReplayPlan {
    pub(in crate::backend) fn new(trace_rows: usize, width: usize) -> Result<Self> {
        if width == 0 || width > 512 {
            return Err(shape(
                "trace replay requires a bounded nonempty column width",
            ));
        }
        let fixed = FixedTraceDomain::new(&FASTPQ_FINAL_V1, trace_rows)?;
        let stripes = FASTPQ_FINAL_V1.fri.blowup_factor as usize;
        let checked = || shape("trace replay resource arithmetic overflow");
        let lde_rows = trace_rows.checked_mul(stripes).ok_or_else(checked)?;
        let coefficient_bytes = trace_rows
            .checked_mul(width)
            .and_then(|cells| cells.checked_mul(size_of::<u64>()))
            .filter(|bytes| *bytes <= isize::MAX as usize)
            .ok_or_else(checked)?;
        let peak_trace_bytes = coefficient_bytes.checked_mul(2).ok_or_else(checked)?;
        let maximum_column_transforms = stripes
            .checked_mul(4)
            .and_then(|transforms| transforms.checked_add(1))
            .and_then(|transforms| transforms.checked_mul(width))
            .ok_or_else(checked)?;
        Ok(Self {
            trace_rows,
            width,
            lde_rows,
            stripes,
            coefficient_bytes,
            stripe_bytes: coefficient_bytes,
            peak_trace_bytes,
            maximum_column_transforms,
            trace_domain: Domain {
                log_size: trace_rows.ilog2(),
                generator: fixed.generator,
            },
            lde_domain: FriDomain::from_lde_parameters(
                FASTPQ_FINAL_V1.lde_root,
                FASTPQ_FINAL_V1.lde_log_size,
                lde_rows,
                FASTPQ_FINAL_V1.omega_coset,
            )?,
        })
    }
}

/// Retained coefficients; both private matrices use fixed zeroizing storage.
pub(super) struct TraceReplay {
    plan: TraceReplayPlan,
    coefficients: SecretPolynomial<u64>,
}

/// Borrowed complete-column stripe; cannot outlive the single replay allocation.
pub(super) struct TraceStripe<'a> {
    plan: TraceReplayPlan,
    stripe: usize,
    values: &'a [u64],
}

impl TraceStripe<'_> {
    pub(super) fn global_index(&self, row: usize) -> usize {
        debug_assert!(row < self.plan.trace_rows);
        self.stripe + row * self.plan.stripes
    }

    pub(super) fn columns(&self) -> impl Iterator<Item = &[u64]> {
        self.values.chunks_exact(self.plan.trace_rows)
    }

    pub(super) fn fill_row(&self, row: usize, output: &mut [u64]) {
        assert_eq!(output.len(), self.plan.width);
        for (value, column) in output.iter_mut().zip(self.columns()) {
            *value = column[row];
        }
    }
}

impl TraceReplay {
    /// Validate every source cell before allocating or transforming private data.
    pub(super) fn new(plan: TraceReplayPlan, columns: &[Vec<u64>]) -> Result<Self> {
        if columns.len() != plan.width {
            return Err(shape(
                "trace replay requires the complete fixed column width",
            ));
        }
        for (column, values) in columns.iter().enumerate() {
            if values.len() != plan.trace_rows {
                return Err(shape("trace replay requires the exact subgroup length"));
            }
            for (row, &value) in values.iter().enumerate() {
                canonical_base(value, "compact_base_trace", &[column, row])?;
            }
        }
        let mut coefficients = SecretPolynomial::zeroed(plan.coefficient_bytes / size_of::<u64>())?;
        coefficients
            .par_chunks_mut(plan.trace_rows)
            .zip(columns.par_iter())
            .for_each(|(output, source)| {
                output.copy_from_slice(source);
                cyclotomic::ifft(output, plan.trace_domain);
            });
        Ok(Self { plan, coefficients })
    }

    pub(super) fn replay_all(
        &self,
        visit: impl FnMut(TraceStripe<'_>) -> Result<()>,
    ) -> Result<()> {
        self.replay_selected(|_| true, visit)
    }

    /// Evaluate one bounded batch per stripe and restore natural-domain order.
    /// The output oracle is charged separately from the two trace matrices.
    pub(super) fn collect_rows<T: Clone + Send>(
        &self,
        initial: T,
        evaluate: impl Fn(&TraceStripe<'_>, core::ops::Range<usize>) -> Result<Vec<T>> + Sync,
    ) -> Result<Vec<T>> {
        let mut output = vec![initial; self.plan.lde_rows];
        self.replay_all(|stripe| {
            let values =
                collect_prover_rows(self.plan.trace_rows, |indices| evaluate(&stripe, indices))?;
            for (row, value) in values.into_iter().enumerate() {
                output[stripe.global_index(row)] = value;
            }
            Ok(())
        })?;
        Ok(output)
    }

    fn replay_selected(
        &self,
        selected: impl Fn(usize) -> bool,
        mut visit: impl FnMut(TraceStripe<'_>) -> Result<()>,
    ) -> Result<()> {
        let mut values = SecretPolynomial::zeroed(self.plan.stripe_bytes / size_of::<u64>())?;
        for stripe in 0..self.plan.stripes {
            if !selected(stripe) {
                continue;
            }
            let offset = self.plan.lde_domain.point(stripe);
            values
                .par_chunks_mut(self.plan.trace_rows)
                .zip(self.coefficients.par_chunks(self.plan.trace_rows))
                .for_each(|(output, coefficients)| {
                    let mut power = 1;
                    for (value, &coefficient) in output.iter_mut().zip(coefficients) {
                        *value = mul_mod(coefficient, power);
                        power = mul_mod(power, offset);
                    }
                    cyclotomic::fft(output, self.plan.trace_domain);
                });
            visit(TraceStripe {
                plan: self.plan,
                stripe,
                values: &values,
            })?;
        }
        Ok(())
    }

    /// Replay each selected stripe once, retaining rows in caller order.
    /// Duplicate indices are allowed because repeated proof queries own copies.
    pub(super) fn selected_rows(&self, indices: &[usize]) -> Result<Vec<Vec<u64>>> {
        if indices.len() > self.plan.lde_rows
            || indices.iter().any(|&index| index >= self.plan.lde_rows)
        {
            return Err(shape("trace replay row selection exceeds its fixed domain"));
        }
        if indices.is_empty() {
            return Ok(Vec::new());
        }
        let mut selected = vec![false; self.plan.stripes];
        for &index in indices {
            selected[index % self.plan.stripes] = true;
        }
        let mut rows = vec![vec![0; self.plan.width]; indices.len()];
        self.replay_selected(
            |stripe| selected[stripe],
            |stripe| {
                for (&index, row) in indices.iter().zip(&mut rows) {
                    if index % self.plan.stripes == stripe.stripe {
                        stripe.fill_row(index / self.plan.stripes, row);
                    }
                }
                Ok(())
            },
        )?;
        Ok(rows)
    }
}

#[cfg(test)]
#[path = "replay_tests.rs"]
mod tests;
