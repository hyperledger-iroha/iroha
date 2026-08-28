//! Canonical FASTPQ STARK parameter definitions.
/// Parameters that configure FRI round structure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FriParameters {
    /// Folding arity per round (V1 requires 8).
    pub arity: u32,
    /// Overall trace domain expansion (`blowup factor`).
    pub blowup_factor: u32,
    /// Maximum reduction rounds available before opening and degree-checking
    /// the complete terminal domain.
    pub max_reductions: u32,
    /// Number of queries sampled by the verifier.
    pub queries: u32,
}
/// Canonical parameter pack for a FASTPQ STARK instantiation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StarkParameterSet {
    /// Stable identifier (used in manifests and telemetry).
    pub name: &'static str,
    /// Log₂ size of the trace domain (`N_trace = 2^trace_log_size`).
    pub trace_log_size: u32,
    /// Primitive `2^trace_log_size` root of unity for the trace domain.
    ///
    /// This is exactly `lde_root^fri.blowup_factor`, so advancing one trace
    /// row advances by the blowup stride in the LDE evaluation ordering.
    pub trace_root: u64,
    /// Log₂ size of the low-degree extension domain (`N_eval = 2^lde_log_size`).
    pub lde_log_size: u32,
    /// Primitive `2^lde_log_size` root of unity for the evaluation domain.
    pub lde_root: u64,
    /// Coset offset applied to the evaluation domain; V1 requires it to lie
    /// outside the LDE subgroup.
    pub omega_coset: u64,
    /// FRI parameterisation.
    pub fri: FriParameters,
}
/// Canonical parameters targeting balanced prover throughput.
pub const FASTPQ_CANONICAL_BALANCED: StarkParameterSet = StarkParameterSet {
    name: "fastpq-lane-balanced",
    trace_log_size: 16,
    trace_root: 0x11a8_cf07_fa6a_f903,
    lde_log_size: 19,
    lde_root: 0x8584_a585_229f_b11b,
    omega_coset: 0xb6a3_8ed4_23da_ef71,
    fri: FriParameters {
        arity: 8,
        blowup_factor: 8,
        max_reductions: 8,
        queries: 46,
    },
};
/// Canonical FASTPQ parameter catalogue.
pub const CANONICAL_PARAMETER_SETS: [StarkParameterSet; 1] = [FASTPQ_CANONICAL_BALANCED];
/// Look up a canonical parameter set by name.
pub fn find_by_name(name: &str) -> Option<&'static StarkParameterSet> {
    CANONICAL_PARAMETER_SETS.iter().find(|set| set.name == name)
}
#[cfg(test)]
mod tests {
    use super::*;

    fn mul_mod(left: u64, right: u64) -> u64 {
        let modulus = u128::from(crate::poseidon::FIELD_MODULUS);
        u64::try_from((u128::from(left) * u128::from(right)) % modulus)
            .expect("reduced product fits u64")
    }

    fn pow_mod(mut base: u64, mut exponent: u64) -> u64 {
        let mut result = 1;
        while exponent != 0 {
            if exponent & 1 == 1 {
                result = mul_mod(result, base);
            }
            base = mul_mod(base, base);
            exponent >>= 1;
        }
        result
    }

    #[test]
    fn canonical_set_declares_v1_shape() {
        for set in CANONICAL_PARAMETER_SETS {
            assert_eq!(set.fri.arity, 8);
            assert_eq!(set.fri.blowup_factor, set.fri.arity);
            assert!(set.fri.max_reductions >= 6);
            assert!(set.fri.queries >= 30);
            assert_ne!(set.trace_root, 0);
            assert_ne!(set.lde_root, 0);
            assert_ne!(set.omega_coset, 0);
        }
    }
    #[test]
    fn lookup_finds_sets() {
        let balanced = find_by_name("fastpq-lane-balanced").expect("balanced params");
        assert_eq!(balanced.fri.arity, 8);
        assert!(find_by_name("unknown").is_none());
        assert_eq!(CANONICAL_PARAMETER_SETS, [FASTPQ_CANONICAL_BALANCED]);
    }

    #[test]
    fn regenerated_domain_roots_are_coherent_and_cosets_are_outside_lde_subgroups() {
        for params in CANONICAL_PARAMETER_SETS {
            for (root, log_size) in [
                (params.trace_root, params.trace_log_size),
                (params.lde_root, params.lde_log_size),
            ] {
                assert_eq!(pow_mod(root, 1_u64 << log_size), 1, "{}", params.name);
                assert_ne!(pow_mod(root, 1_u64 << (log_size - 1)), 1, "{}", params.name);
            }
            assert_eq!(
                pow_mod(params.lde_root, u64::from(params.fri.blowup_factor)),
                params.trace_root,
                "{} trace/LDE generators must use the advertised blowup stride",
                params.name
            );
            assert_ne!(
                pow_mod(params.omega_coset, 1_u64 << params.lde_log_size),
                1,
                "{} coset offset must be outside the LDE subgroup",
                params.name
            );
        }
    }
}
