//! Constant-space geometry checks for verifier-known trace polynomials.
//!
//! Checking the whole LDE coset for overlap needs one exponentiation, not an
//! allocation or scan over its points. This helper validates the exact declared
//! roots before deriving the smaller subgroup used by a particular statement.

use super::{FriDomain, GOLDILOCKS_MODULUS, field_pow};
use crate::{Error, Result};
use fastpq_isi::StarkParameterSet;

/// Exact trace generator after constant-space validation of both domains.
#[derive(Clone, Copy, Debug)]
pub(super) struct FixedTraceDomain {
    pub(super) generator: u64,
}

impl FixedTraceDomain {
    /// Validate roots, declared orders, subgroup agreement and coset disjointness.
    pub(super) fn new(params: &StarkParameterSet, trace_rows: usize) -> Result<Self> {
        if !trace_rows.is_power_of_two()
            || params.trace_log_size > 32
            || params.lde_log_size > 32
            || trace_rows.ilog2() > params.trace_log_size
        {
            return Err(shape_error("fixed trace requires a supported subgroup"));
        }
        for (value, context) in [
            (params.trace_root, "fixed_domain_trace_root"),
            (params.lde_root, "fixed_domain_lde_root"),
            (params.omega_coset, "fixed_domain_coset"),
        ] {
            if value >= GOLDILOCKS_MODULUS {
                return Err(Error::NonCanonicalGoldilocksElement {
                    context,
                    indices: Vec::new(),
                });
            }
        }
        let trace_order = 1_u64 << params.trace_log_size;
        let lde_order = 1_u64 << params.lde_log_size;
        if !has_exact_order(params.trace_root, trace_order)
            || !has_exact_order(params.lde_root, lde_order)
        {
            return Err(shape_error(
                "fixed trace roots need their exact declared orders",
            ));
        }
        let blowup = usize::try_from(params.fri.blowup_factor)
            .map_err(|_| shape_error("fixed trace blowup exceeds this platform"))?;
        if !blowup.is_power_of_two() {
            return Err(shape_error("fixed trace blowup must be a power of two"));
        }
        let lde_rows = trace_rows
            .checked_mul(blowup)
            .ok_or(Error::TraceLengthOverflow { rows: trace_rows })?;
        if lde_rows.ilog2() > params.lde_log_size {
            return Err(shape_error("fixed trace LDE exceeds the supported domain"));
        }
        let domain = FriDomain::from_lde_parameters(
            params.lde_root,
            params.lde_log_size,
            lde_rows,
            params.omega_coset,
        )?;
        let generator = field_pow(domain.generator, blowup as u64);
        let trace_stride = 1_u64 << (params.trace_log_size - trace_rows.ilog2());
        if field_pow(params.trace_root, trace_stride) != generator {
            return Err(shape_error("fixed trace and LDE subgroup roots disagree"));
        }
        // H is a subgroup of the LDE subgroup L. A coset aL intersects H iff
        // a is in L. Exact L order was established above; no per-point scan is
        // needed even when the declared domain is as large as 2^32.
        if params.omega_coset == 0 || field_pow(params.omega_coset, lde_rows as u64) == 1 {
            return Err(shape_error("fixed trace requires a disjoint LDE coset"));
        }
        Ok(Self { generator })
    }
}

fn has_exact_order(root: u64, order: u64) -> bool {
    field_pow(root, order) == 1 && (order == 1 || field_pow(root, order / 2) != 1)
}

fn shape_error(details: &'static str) -> Error {
    Error::InvalidTraceShape {
        details: details.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::{AirQuotientDomain, field_inverse, mul_mod};
    use fastpq_isi::FASTPQ_FINAL_V1;

    #[test]
    fn every_supported_trace_order_matches_the_quotient_domain() {
        assert!(has_exact_order(1, 1));
        assert!(!has_exact_order(0, 1));
        assert!(has_exact_order(GOLDILOCKS_MODULUS - 1, 2));
        assert!(!has_exact_order(1, 2));
        assert!(!has_exact_order(GOLDILOCKS_MODULUS - 1, 4));
        for log in 0..=FASTPQ_FINAL_V1.trace_log_size {
            let rows = 1_usize << log;
            let fixed = FixedTraceDomain::new(&FASTPQ_FINAL_V1, rows).unwrap();
            assert!(has_exact_order(fixed.generator, rows as u64));
            assert_eq!(
                fixed.generator,
                field_pow(
                    FASTPQ_FINAL_V1.trace_root,
                    1_u64 << (FASTPQ_FINAL_V1.trace_log_size - log)
                )
            );
            assert!(
                AirQuotientDomain::new(
                    &FASTPQ_FINAL_V1,
                    rows * FASTPQ_FINAL_V1.fri.blowup_factor as usize
                )
                .is_ok()
            );
        }
        assert_eq!(core::mem::size_of::<FixedTraceDomain>(), 8);
    }

    #[test]
    fn full_goldilocks_domain_geometry_uses_only_constant_space() {
        let mut params = FASTPQ_FINAL_V1;
        params.trace_log_size = 31;
        params.lde_log_size = 32;
        params.lde_root = field_pow(7, (GOLDILOCKS_MODULUS - 1) >> 32);
        params.trace_root = mul_mod(params.lde_root, params.lde_root);
        params.fri.blowup_factor = 2;
        params.omega_coset = 7;
        let fixed = FixedTraceDomain::new(&params, 1_usize << 31).unwrap();
        assert_eq!(fixed.generator, params.trace_root);
    }

    #[test]
    fn malformed_geometry_and_all_root_coordinates_fail_before_allocation() {
        for rows in [0, 3, 1_usize << (FASTPQ_FINAL_V1.trace_log_size + 1)] {
            assert!(FixedTraceDomain::new(&FASTPQ_FINAL_V1, rows).is_err());
        }
        for bad in [GOLDILOCKS_MODULUS, u64::MAX] {
            for field in 0..3 {
                let mut params = FASTPQ_FINAL_V1;
                match field {
                    0 => params.trace_root = bad,
                    1 => params.lde_root = bad,
                    _ => params.omega_coset = bad,
                }
                assert!(matches!(
                    FixedTraceDomain::new(&params, 4),
                    Err(Error::NonCanonicalGoldilocksElement { .. })
                ));
            }
        }
        for field in 0..2 {
            for bad in [0, 1] {
                let mut params = FASTPQ_FINAL_V1;
                if field == 0 {
                    params.trace_root = bad;
                } else {
                    params.lde_root = bad;
                }
                assert!(FixedTraceDomain::new(&params, 4).is_err());
            }
        }
        for bad in [0, 3, u32::MAX] {
            let mut params = FASTPQ_FINAL_V1;
            params.fri.blowup_factor = bad;
            assert!(FixedTraceDomain::new(&params, 4).is_err());
        }
        for field in 0..2 {
            let mut params = FASTPQ_FINAL_V1;
            if field == 0 {
                params.trace_log_size = 33;
            } else {
                params.lde_log_size = 33;
            }
            assert!(FixedTraceDomain::new(&params, 4).is_err());
        }
    }

    #[test]
    fn exact_but_inconsistent_generators_and_every_small_overlapping_coset_fail() {
        let mut params = FASTPQ_FINAL_V1;
        params.trace_root = field_inverse(params.trace_root);
        assert!(has_exact_order(
            params.trace_root,
            1_u64 << params.trace_log_size
        ));
        assert!(FixedTraceDomain::new(&params, 4).is_err());
        let lde_rows = 4 * FASTPQ_FINAL_V1.fri.blowup_factor as usize;
        let generator = field_pow(
            FASTPQ_FINAL_V1.lde_root,
            1_u64 << (FASTPQ_FINAL_V1.lde_log_size - lde_rows.ilog2()),
        );
        let mut point = 1;
        for _ in 0..lde_rows {
            let mut params = FASTPQ_FINAL_V1;
            params.omega_coset = point;
            assert!(FixedTraceDomain::new(&params, 4).is_err());
            point = mul_mod(point, generator);
        }
        let mut params = FASTPQ_FINAL_V1;
        params.omega_coset = 0;
        assert!(FixedTraceDomain::new(&params, 4).is_err());
    }
}
