//! Zerofier weights for all-row and non-wrapping transition constraints.
//!
//! Trace columns interpolate the subgroup of size N. Constraints that apply to
//! every row divide by X^N - 1; adjacent-row constraints exclude its last point
//! and divide by (X^N - 1)/(X - last). Evaluation takes place on a disjoint coset.

use super::{
    Error, FriDomain, GOLDILOCKS_MODULUS, Result, field_inverse, field_pow, mul_mod, sub_mod,
};
use fastpq_isi::StarkParameterSet;

/// Inverse zerofiers at one authenticated LDE point.
#[derive(Clone, Copy, Debug)]
pub(crate) struct AirQuotientWeights {
    pub(super) all_rows: u64,
    pub(super) transitions: u64,
}

/// Validated domain geometry and the small periodic all-row denominator table.
#[derive(Clone, Debug)]
pub(crate) struct AirQuotientDomain {
    domain: FriDomain,
    lde_size: usize,
    last_trace_point: u64,
    inverse_zerofiers: Vec<u64>,
}

impl AirQuotientDomain {
    /// Construct a disjoint coset for the exact trace/LDE geometry.
    pub(crate) fn new(params: &StarkParameterSet, lde_size: usize) -> Result<Self> {
        let blowup = usize::try_from(params.fri.blowup_factor).map_err(|_| shape_error())?;
        if !blowup.is_power_of_two()
            || !lde_size.is_power_of_two()
            || lde_size < blowup
            || params.omega_coset == 0
            || params.omega_coset >= GOLDILOCKS_MODULUS
        {
            return Err(shape_error());
        }
        let domain = FriDomain::from_lde_parameters(
            params.lde_root,
            params.lde_log_size,
            lde_size,
            params.omega_coset,
        )?;
        let lde_order = u64::try_from(lde_size).map_err(|_| shape_error())?;
        if field_pow(domain.generator, lde_order) != 1
            || (lde_size > 1 && field_pow(domain.generator, lde_order / 2) == 1)
        {
            return Err(shape_error());
        }
        let trace_size = lde_size / blowup;
        let trace_order = u64::try_from(trace_size).map_err(|_| shape_error())?;
        let last_exponent = u64::try_from(lde_size - blowup).map_err(|_| shape_error())?;
        let last_trace_point = field_pow(domain.generator, last_exponent);
        let mut inverse_zerofiers = Vec::with_capacity(blowup);
        // (offset * generator^i)^N has period blowup, so the prover needs only
        // blowup inversions for the whole LDE, not one inversion for every row.
        for index in 0..blowup {
            let denominator = sub_mod(field_pow(domain.point(index), trace_order), 1);
            if denominator == 0 {
                return Err(shape_error());
            }
            inverse_zerofiers.push(field_inverse(denominator));
        }
        Ok(Self {
            domain,
            lde_size,
            last_trace_point,
            inverse_zerofiers,
        })
    }

    /// Evaluate both inverse zerofiers at one bounded opening index.
    pub(crate) fn weights_at(&self, index: usize) -> Result<AirQuotientWeights> {
        if index >= self.lde_size {
            return Err(Error::QueryIndexOutOfRange {
                index,
                len: self.lde_size,
            });
        }
        Ok(self.weights_for_point(index, self.domain.point(index)))
    }

    /// Iterate weights using one multiplication per consecutive domain point.
    pub(super) fn weights(&self) -> impl Iterator<Item = AirQuotientWeights> + '_ {
        (0..self.lde_size).scan(self.domain.offset, |point, index| {
            let weights = self.weights_for_point(index, *point);
            *point = mul_mod(*point, self.domain.generator);
            Some(weights)
        })
    }

    fn weights_for_point(&self, index: usize, point: u64) -> AirQuotientWeights {
        let all_rows = self.inverse_zerofiers[index % self.inverse_zerofiers.len()];
        AirQuotientWeights {
            all_rows,
            transitions: mul_mod(sub_mod(point, self.last_trace_point), all_rows),
        }
    }
}

fn shape_error() -> Error {
    Error::InvalidTraceShape {
        details: "AIR quotient requires an exact trace subgroup and disjoint LDE coset".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fastpq_isi::FASTPQ_FINAL_V1;

    #[test]
    fn weights_match_explicit_zerofier_products_and_random_access() {
        for trace_size in [1usize, 2, 4, 16, 64] {
            let blowup = usize::try_from(FASTPQ_FINAL_V1.fri.blowup_factor).unwrap();
            let domain = AirQuotientDomain::new(&FASTPQ_FINAL_V1, trace_size * blowup).unwrap();
            let trace_generator = field_pow(domain.domain.generator, blowup as u64);
            for (index, weights) in domain.weights().enumerate() {
                let point = domain.domain.point(index);
                let mut all_rows = 1;
                let mut transitions = 1;
                let mut trace_point = 1;
                for row in 0..trace_size {
                    let factor = sub_mod(point, trace_point);
                    all_rows = mul_mod(all_rows, factor);
                    if row + 1 < trace_size {
                        transitions = mul_mod(transitions, factor);
                    }
                    trace_point = mul_mod(trace_point, trace_generator);
                }
                assert_eq!(mul_mod(all_rows, weights.all_rows), 1);
                assert_eq!(mul_mod(transitions, weights.transitions), 1);
                let sampled = domain.weights_at(index).unwrap();
                assert_eq!(sampled.all_rows, weights.all_rows);
                assert_eq!(sampled.transitions, weights.transitions);
            }
            assert!(domain.weights_at(trace_size * blowup).is_err());
        }
    }

    #[test]
    fn domain_rejects_overlap_and_invalid_geometry() {
        for offset in [0, 1, GOLDILOCKS_MODULUS] {
            let mut params = FASTPQ_FINAL_V1;
            params.omega_coset = offset;
            assert!(AirQuotientDomain::new(&params, 32).is_err());
        }
        for length in [0, 1, 2, 3, 7, 9, 31] {
            assert!(AirQuotientDomain::new(&FASTPQ_FINAL_V1, length).is_err());
        }
        let mut params = FASTPQ_FINAL_V1;
        params.lde_root = 1;
        assert!(AirQuotientDomain::new(&params, 32).is_err());
        params = FASTPQ_FINAL_V1;
        params.fri.blowup_factor = 3;
        assert!(AirQuotientDomain::new(&params, 32).is_err());
    }

    #[test]
    fn unsatisfied_constant_constraint_is_not_a_low_degree_quotient() {
        let trace_size = 4usize;
        let domain = AirQuotientDomain::new(&FASTPQ_FINAL_V1, 32).unwrap();
        // C(X)=X^N-1 has quotient one. Adding one makes C nonzero on every
        // trace row. Independently interpolate the resulting rational samples.
        let valid: Vec<_> = domain
            .weights()
            .enumerate()
            .map(|(index, weights)| {
                let residue = sub_mod(field_pow(domain.domain.point(index), trace_size as u64), 1);
                crate::GoldilocksFp4V1::from_base(mul_mod(residue, weights.all_rows)).unwrap()
            })
            .collect();
        assert!(
            domain
                .domain
                .evaluations_have_degree_below(&valid, 1)
                .unwrap()
        );
        let invalid: Vec<_> = domain
            .weights()
            .enumerate()
            .map(|(index, weights)| {
                let residue = field_pow(domain.domain.point(index), trace_size as u64);
                crate::GoldilocksFp4V1::from_base(mul_mod(residue, weights.all_rows)).unwrap()
            })
            .collect();
        assert!(
            !domain
                .domain
                .evaluations_have_degree_below(&invalid, 2 * trace_size)
                .unwrap()
        );
    }
}
