//! Canonical FASTPQ STARK parameter definitions.

/// Description of the scalar field used by the proof system.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FieldDescriptor {
    /// Human-readable name of the field.
    pub name: &'static str,
    /// Prime modulus written in decimal form for ease of reference.
    pub modulus_decimal: &'static str,
    /// Extension degree used for FRI (1 = base field, 2 = quadratic extension).
    pub extension_degree: u32,
}

/// Description of the hash functions used by the STARK.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HashDescriptor {
    /// Hash used for Merkle commitments.
    pub trace_commitment: &'static str,
    /// Hash used for the Fiat-Shamir transcript.
    pub transcript: &'static str,
}

/// Parameters that configure FRI round structure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FriParameters {
    /// Folding arity per round (supported values: 8 or 16).
    pub arity: u32,
    /// Overall trace domain expansion (`blowup factor`).
    pub blowup_factor: u32,
    /// Maximum number of reduction rounds.
    pub max_reductions: u32,
    /// Number of queries sampled by the verifier.
    pub queries: u32,
}

/// Canonical parameter pack for a FASTPQ STARK instantiation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StarkParameterSet {
    /// Stable identifier (used in manifests and telemetry).
    pub name: &'static str,
    /// Target security level in bits.
    pub target_security_bits: u32,
    /// Bit grinding applied before FRI challenges are sampled.
    pub grinding_bits: u32,
    /// Log₂ size of the trace domain (`N_trace = 2^trace_log_size`).
    pub trace_log_size: u32,
    /// Primitive `2^trace_log_size` root of unity for the trace domain.
    pub trace_root: u64,
    /// Log₂ size of the low-degree extension domain (`N_eval = 2^lde_log_size`).
    pub lde_log_size: u32,
    /// Primitive `2^lde_log_size` root of unity for the evaluation domain.
    pub lde_root: u64,
    /// Width of the permutation product domain (rows participating in the lookup grand product).
    pub permutation_size: u32,
    /// Optional log₂ size of the lookup evaluation domain if it differs from the trace domain.
    pub lookup_log_size: Option<u32>,
    /// Coset offset applied to the evaluation domain (1 for the base subgroup).
    pub omega_coset: u64,
    /// Field descriptor (base + extension degree).
    pub field: FieldDescriptor,
    /// Hash descriptor for commitments + transcript.
    pub hash: HashDescriptor,
    /// FRI parameterisation.
    pub fri: FriParameters,
}

/// Goldilocks prime (2^64 - 2^32 + 1) with quadratic extension for DEEP-FRI.
pub const GOLDILOCKS_FP2: FieldDescriptor = FieldDescriptor {
    name: "Goldilocks",
    modulus_decimal: "18446744069414584321",
    extension_degree: 2,
};

/// Poseidon2 commitment hash with SHA3-256 transcript permutations.
pub const POSEIDON2_SHA3: HashDescriptor = HashDescriptor {
    trace_commitment: "Poseidon2(Goldilocks)",
    transcript: "SHA3-256",
};

/// Canonical parameters targeting balanced prover throughput.
pub const FASTPQ_CANONICAL_BALANCED: StarkParameterSet = StarkParameterSet {
    name: "fastpq-lane-balanced",
    target_security_bits: 128,
    grinding_bits: 23,
    trace_log_size: 16,
    trace_root: 0x002a_247f_81c6_f850,
    lde_log_size: 19,
    lde_root: 0x6026_3388_dbbf_9b2a,
    permutation_size: 65_536,
    lookup_log_size: Some(19),
    omega_coset: 0x6af3_25e8_25ad_5c18,
    field: GOLDILOCKS_FP2,
    hash: POSEIDON2_SHA3,
    fri: FriParameters {
        arity: 8,
        blowup_factor: 8,
        max_reductions: 8,
        queries: 46,
    },
};

/// Canonical parameters optimised for latency-sensitive lanes.
pub const FASTPQ_CANONICAL_LATENCY: StarkParameterSet = StarkParameterSet {
    name: "fastpq-lane-latency",
    target_security_bits: 128,
    grinding_bits: 21,
    trace_log_size: 16,
    trace_root: 0x6a9f_4eb3_8fb9_b892,
    lde_log_size: 20,
    lde_root: 0x9c9c_3a57_1b6f_89ac,
    permutation_size: 65_536,
    lookup_log_size: Some(20),
    omega_coset: 0x3a5f_d417_1e3c_3a4d,
    field: GOLDILOCKS_FP2,
    hash: POSEIDON2_SHA3,
    fri: FriParameters {
        arity: 16,
        blowup_factor: 16,
        max_reductions: 6,
        queries: 34,
    },
};

/// Ordered slice of canonical parameter sets (stable order: balanced, latency).
pub const CANONICAL_PARAMETER_SETS: [StarkParameterSet; 2] =
    [FASTPQ_CANONICAL_BALANCED, FASTPQ_CANONICAL_LATENCY];

/// Look up a canonical parameter set by name.
pub fn find_by_name(name: &str) -> Option<&'static StarkParameterSet> {
    CANONICAL_PARAMETER_SETS.iter().find(|set| set.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_sets_meet_security_target() {
        for set in CANONICAL_PARAMETER_SETS {
            assert!(set.target_security_bits >= 128);
            assert!((set.fri.arity == 8) || (set.fri.arity == 16));
            assert_eq!(set.fri.blowup_factor, set.fri.arity);
            assert!(set.fri.max_reductions >= 6);
            assert!(set.fri.queries >= 30);
            assert_eq!(u64::from(set.permutation_size), 1u64 << set.trace_log_size);
            if let Some(lookup_log) = set.lookup_log_size {
                assert!(lookup_log <= set.lde_log_size);
            }
            assert_ne!(set.trace_root, 0);
            assert_ne!(set.lde_root, 0);
            assert_ne!(set.omega_coset, 0);
        }
    }

    #[test]
    fn lookup_finds_sets() {
        let balanced = find_by_name("fastpq-lane-balanced").expect("balanced params");
        assert_eq!(balanced.fri.arity, 8);
        assert!(find_by_name("fastpq-lane-latency").is_some());
        assert!(find_by_name("unknown").is_none());
        assert_eq!(balanced.permutation_size, 1 << balanced.trace_log_size);
    }
}
