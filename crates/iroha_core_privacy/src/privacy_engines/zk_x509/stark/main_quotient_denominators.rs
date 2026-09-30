//! Public periodic vanishing inverses for one canonical MAIN quotient stripe.
//!
//! If the stripe root has order S and native size N divides S, then
//! `(shift * root^j)^N - 1` has exact period S/N. Logical witness padding and
//! masked polynomial degrees do not change the native vanishing polynomial.
//! The independent verifier continues to evaluate its denominator directly.

use super::*;

/// A stripe owns only its public period, never a full quotient-row table.
pub(super) struct MainQuotientDenominatorsV1 {
    native_log2: u8,
    rows: usize,
    inverses: Vec<F>,
}

impl MainQuotientDenominatorsV1 {
    fn period_v1(
        native_log2: u8,
        stripe: main_quotient_stripes::MainQuotientStripeV1,
    ) -> Result<usize, ZkX509StarkErrorV1> {
        if !(MIN_TRACE_LOG2..=ZK_X509_MAX_NATIVE_TRACE_LOG2_V1).contains(&native_log2) {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        let full_rows = stripe
            .rows
            .checked_mul(stripe.count)
            .filter(|rows| rows.is_power_of_two())
            .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
        let evaluation_log2 =
            u8::try_from(full_rows.ilog2()).map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?;
        if evaluation_log2 > ZK_X509_MAIN_COMMON_LDE_LOG2_V1 {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        let expected = main_quotient_stripes::MainQuotientStripeV1::new_v1(
            native_log2,
            evaluation_log2,
            stripe.ordinal,
        )?;
        if stripe.rows != expected.rows
            || stripe.count != expected.count
            || stripe.next_stride != expected.next_stride
            || stripe.root != expected.root
            || stripe.shift != expected.shift
        {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        Ok(stripe.next_stride)
    }

    /// Charge the public table and owner before allocating the private cache.
    pub(super) fn payload_bound_v1(
        native_log2: u8,
        stripe: main_quotient_stripes::MainQuotientStripeV1,
    ) -> Result<usize, ZkX509StarkErrorV1> {
        Self::period_v1(native_log2, stripe)?
            .checked_mul(core::mem::size_of::<F>())
            .and_then(|bytes| bytes.checked_add(core::mem::size_of::<Self>()))
            .ok_or(ZkX509StarkErrorV1::ProfileMismatch)
    }

    /// Validate every public shape/root/shift before constructing the period.
    pub(super) fn new_v1(
        native_log2: u8,
        stripe: main_quotient_stripes::MainQuotientStripeV1,
    ) -> Result<Self, ZkX509StarkErrorV1> {
        let period = Self::period_v1(native_log2, stripe)?;
        let native_rows = 1_usize << native_log2;
        let step = stripe.root.pow(native_rows as u128);
        let mut power = stripe.shift.pow(native_rows as u128);
        let mut inverses = Vec::new();
        inverses
            .try_reserve_exact(period)
            .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
        // Actual vector capacity must fit the amount reserved by the buffer plan.
        if inverses.capacity() > period {
            return Err(ZkX509StarkErrorV1::ProofTooLarge);
        }
        for _ in 0..period {
            inverses.push(
                power
                    .sub(F::ONE)
                    .inv()
                    .ok_or(ZkX509StarkErrorV1::InternalInvariant)?,
            );
            power = power.mul(step);
        }
        Ok(Self {
            native_log2,
            rows: stripe.rows,
            inverses,
        })
    }

    /// Index only by a checked public row and the registration's native size.
    pub(super) fn at_v1(&self, native_log2: u8, row: usize) -> Result<F, ZkX509StarkErrorV1> {
        if native_log2 != self.native_log2 || row >= self.rows {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        Ok(self.inverses[row % self.inverses.len()])
    }
}

#[cfg(test)]
#[path = "main_quotient_denominator_tests.rs"]
mod tests;
