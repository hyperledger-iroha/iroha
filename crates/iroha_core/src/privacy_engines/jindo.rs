//! Native clean-room Jindo polynomial-commitment engine.
//!
//! The published Jindo construction is a univariate lattice PCS over a
//! Jindo-friendly coefficient field.  This module deliberately implements one
//! closed transparent profile; it does not expose the unpublished
//! "multilinear/flexible regime" surface that used to be represented by shape
//! checks alone.
//!
//! The implementation is built from the public algorithms in Figures 1--5 of
//! ePrint 2026/044.  Consensus registration stays fail-closed until the fixed
//! ring parameters, proof wire, prover, verifier, and adversarial vectors in
//! this module are all complete.

#[path = "jindo/crs.rs"]
mod crs;
#[path = "jindo/encoding.rs"]
mod encoding;
#[path = "jindo/field.rs"]
mod field;
#[path = "jindo/parameters.rs"]
mod parameters;
#[path = "jindo/ring.rs"]
mod ring;
#[path = "jindo/sampling.rs"]
mod sampling;

pub(crate) use field::JindoFieldElementV1;
pub(crate) use parameters::JINDO_PARAMETERS_V1;
pub(crate) use ring::{JINDO_INNER_MODULI_V1, JINDO_OUTER_MODULI_V1, JindoRnsPolynomialV1};

/// Exact coefficient-field byte width in the first native Jindo profile.
pub const JINDO_FIELD_ELEMENT_BYTES_V1: usize = 32;

/// CELPC/Jindo coefficient-encoding base `b`.
pub const JINDO_ENCODING_BASE_V1: u64 = 60_272;

/// CELPC/Jindo coefficient-encoding exponent `gamma`.
pub const JINDO_ENCODING_EXPONENT_V1: usize = 16;

/// Cyclotomic application-ring degree `d`.
pub const JINDO_RING_DEGREE_V1: usize = 256;

/// Number of coefficient-field slots encoded in one application-ring element.
pub const JINDO_ENCODING_SLOTS_V1: usize = JINDO_RING_DEGREE_V1 / JINDO_ENCODING_EXPONENT_V1;

/// Maximum polynomial coefficient count in the fixed testnet profile.
pub const JINDO_MAX_COEFFICIENTS_V1: usize = 256;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_profile_dimensions_are_self_consistent() {
        assert_eq!(JINDO_ENCODING_SLOTS_V1, 16);
        assert_eq!(JINDO_RING_DEGREE_V1 % JINDO_ENCODING_EXPONENT_V1, 0);
        assert_eq!(JINDO_FIELD_ELEMENT_BYTES_V1, 32);
        assert!(JINDO_MAX_COEFFICIENTS_V1.is_power_of_two());
    }
}
