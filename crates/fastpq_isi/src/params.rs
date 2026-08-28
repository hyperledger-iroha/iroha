//! Canonical FASTPQ STARK parameter definitions.

use core::cmp::Ordering;

/// Sole FASTPQ parameter identifier accepted by the first-release prover and verifier.
pub const FASTPQ_FINAL_V1_ID: &str = "fastpq-state-transition-stark-v1";
/// Exact privacy-catalog identity bound into every FASTPQ native-STARK digest.
pub const FASTPQ_CATALOG_V1: &str = "iroha-privacy-exact12-v1";
/// Required aggregate qROM security for the first release.
pub const FASTPQ_REQUIRED_SECURITY_BITS_V1: u32 = 128;
/// Number of independently generated Goldilocks digest lanes.
pub const FASTPQ_DIGEST_LANES_V1: u32 = 6;
/// Bits in each canonical Goldilocks digest lane.
pub const FASTPQ_DIGEST_LANE_BITS_V1: u32 = 64;
/// Portable release proof-artifact count included in multi-target accounting.
pub const FASTPQ_AGGREGATE_TARGETS_V1: u64 = 54;
/// Explicit upper bound on quantum random-oracle queries used by the bound calculator.
pub const FASTPQ_QUANTUM_ORACLE_QUERY_LOG2_BOUND_V1: u32 = 32;
/// Minimum verifier query count admitted by the release policy.
pub const FASTPQ_MIN_QUERY_COUNT_V1: u32 = 64;
/// Query-count granularity required by the release policy.
pub const FASTPQ_QUERY_COUNT_GRANULARITY_V1: u32 = 8;
/// Search ceiling used by the deterministic query-count selector.
pub const FASTPQ_MAX_QUERY_COUNT_V1: u32 = 512;

/// Description of the scalar and challenge field used by the proof system.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FieldDescriptor {
    /// Human-readable name of the base field.
    pub name: &'static str,
    /// Prime modulus written in decimal form for ease of reference.
    pub modulus_decimal: &'static str,
    /// Extension degree used for FRI challenges and folds.
    pub extension_degree: u32,
    /// Irreducible polynomial defining the extension field.
    pub extension_polynomial: &'static str,
}

/// Description of the typed digest used by the STARK.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HashDescriptor {
    /// Hash used for every native-STARK commitment.
    pub trace_commitment: &'static str,
    /// Hash used for the Fiat-Shamir transcript.
    pub transcript: &'static str,
    /// Canonical encoded digest width.
    pub digest_bytes: u32,
}

/// Parameters that configure the binary FRI round structure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FriParameters {
    /// Folding arity per round. V1 admits only binary folding.
    pub arity: u32,
    /// Overall trace-domain expansion (`blowup factor`).
    pub blowup_factor: u32,
    /// Maximum binary reduction rounds before the complete terminal opening.
    pub max_reductions: u32,
    /// Number of deduplicated verifier queries.
    pub queries: u32,
}

/// Canonical parameter pack for the sole FASTPQ V1 STARK instantiation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StarkParameterSet {
    /// Stable identifier used in wires, manifests, and telemetry.
    pub name: &'static str,
    /// Required aggregate security target; this is not itself a qualification claim.
    pub required_security_bits: u32,
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
    /// Width of the permutation-product domain.
    pub permutation_size: u32,
    /// Log₂ size of the lookup evaluation domain.
    pub lookup_log_size: u32,
    /// Coset offset applied to the evaluation domain.
    pub omega_coset: u64,
    /// Base and extension-field descriptor.
    pub field: FieldDescriptor,
    /// Native-STARK digest descriptor.
    pub hash: HashDescriptor,
    /// Binary FRI parameters.
    pub fri: FriParameters,
}

/// Goldilocks base field with the degree-four challenge extension `X^4 - 7`.
pub const GOLDILOCKS_FP4_V1: FieldDescriptor = FieldDescriptor {
    name: "Goldilocks",
    modulus_decimal: "18446744069414584321",
    extension_degree: 4,
    extension_polynomial: "X^4 - 7",
};

/// Six independent Poseidon-x7 Goldilocks lanes for commitments and transcript.
pub const POSEIDON_X7_GOLDILOCKS_DIGEST384_V1: HashDescriptor = HashDescriptor {
    trace_commitment: "Poseidon-x7-Goldilocks-6x64-v1",
    transcript: "Poseidon-x7-Goldilocks-6x64-v1",
    digest_bytes: 48,
};

/// Fail-closed reasons preventing a parameter set from claiming production qualification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum FastpqProductionQualificationBlockerV1 {
    /// The repository lacks an independently reviewed, protocol-specific qROM reduction linking
    /// the exact arithmetic accounting to the complete FASTPQ adversary.
    MissingProtocolSpecificQromReduction,
    /// The six-lane multi-target collision term needs independent review tied to final artifacts.
    MissingDigestMultiTargetReview,
}

/// Typed production status derived from evidence rather than caller assertion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FastpqProductionQualificationV1 {
    /// Every required reduction and review is registered for the final artifact digests.
    ProductionQualified,
    /// One mandatory qualification input is missing.
    Unavailable(FastpqProductionQualificationBlockerV1),
}

/// Exact inputs to the deterministic aggregate qROM arithmetic calculator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FastpqQromBoundInputsV1 {
    /// LDE blowup factor; V1 requires a power of two.
    pub blowup_factor: u32,
    /// Total proof targets covered by the release union bound.
    pub aggregate_targets: u64,
    /// Log₂ upper bound on quantum random-oracle queries.
    pub quantum_oracle_query_log2_bound: u32,
    /// Combined bit width of all independent digest lanes.
    pub digest_bits: u32,
    /// Required aggregate security bits.
    pub required_security_bits: u32,
}

/// Exact dyadic upper bound `numerator / 2^denominator_log2`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExactDyadicBoundV1 {
    /// Integer numerator.
    pub numerator: u128,
    /// Power-of-two denominator exponent.
    pub denominator_log2: u32,
}

/// Exact calculator output for one query count.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FastpqQromBoundReportV1 {
    /// Query count evaluated by the calculator.
    pub queries: u32,
    /// Square-root/qROM sampling term, including oracle-query and target factors.
    pub sampling_term: ExactDyadicBoundV1,
    /// Six-lane quantum collision term, including every ordered target pair.
    pub collision_term: ExactDyadicBoundV1,
    /// Whether the exact sum of the two terms is strictly below the target bound.
    pub arithmetic_target_met: bool,
    /// Evidence-derived production status. Arithmetic alone cannot qualify the protocol.
    pub production_qualification: FastpqProductionQualificationV1,
}

/// Default arithmetic inputs frozen for the first-release manifest.
pub const FASTPQ_QROM_BOUND_INPUTS_V1: FastpqQromBoundInputsV1 = FastpqQromBoundInputsV1 {
    blowup_factor: 8,
    aggregate_targets: FASTPQ_AGGREGATE_TARGETS_V1,
    quantum_oracle_query_log2_bound: FASTPQ_QUANTUM_ORACLE_QUERY_LOG2_BOUND_V1,
    digest_bits: FASTPQ_DIGEST_LANES_V1 * FASTPQ_DIGEST_LANE_BITS_V1,
    required_security_bits: FASTPQ_REQUIRED_SECURITY_BITS_V1,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct WideUint512V1([u64; 8]);

impl WideUint512V1 {
    fn from_u128_shifted(value: u128, shift: u32) -> Option<Self> {
        if value == 0 {
            return Some(Self::default());
        }
        let highest = 127_u32.saturating_sub(value.leading_zeros());
        if highest.checked_add(shift)? >= 512 {
            return None;
        }
        let mut output = Self::default();
        let word_shift = usize::try_from(shift / 64).ok()?;
        let bit_shift = shift % 64;
        let limbs = [value as u64, (value >> 64) as u64];
        for (source_index, limb) in limbs.into_iter().enumerate() {
            if limb == 0 {
                continue;
            }
            let target = word_shift.checked_add(source_index)?;
            output.0[target] |= limb << bit_shift;
            if bit_shift != 0 {
                let carry = limb >> (64 - bit_shift);
                if carry != 0 {
                    output.0[target.checked_add(1)?] |= carry;
                }
            }
        }
        Some(output)
    }

    fn checked_add(self, other: Self) -> Option<Self> {
        let mut output = Self::default();
        let mut carry = false;
        for index in 0..self.0.len() {
            let (partial, first_carry) = self.0[index].overflowing_add(other.0[index]);
            let (sum, second_carry) = partial.overflowing_add(u64::from(carry));
            output.0[index] = sum;
            carry = first_carry || second_carry;
        }
        (!carry).then_some(output)
    }

    fn power_of_two(exponent: u32) -> Option<Self> {
        if exponent >= 512 {
            return None;
        }
        let mut output = Self::default();
        output.0[usize::try_from(exponent / 64).ok()?] = 1_u64 << (exponent % 64);
        Some(output)
    }

    fn cmp_words(&self, other: &Self) -> Ordering {
        self.0.iter().rev().cmp(other.0.iter().rev())
    }
}

fn checked_shift(value: u128, shift: u32) -> Option<u128> {
    value.checked_shl(shift)
}

fn qrom_bound_terms(
    inputs: FastpqQromBoundInputsV1,
    queries: u32,
) -> Option<(ExactDyadicBoundV1, ExactDyadicBoundV1)> {
    if !inputs.blowup_factor.is_power_of_two() || queries % 2 != 0 {
        return None;
    }
    let blowup_log2 = inputs.blowup_factor.ilog2();
    // The square-root loss is represented exactly by halving the classical proximity exponent.
    let sampling_denominator_log2 = queries.checked_mul(blowup_log2)?.checked_div(2)?;
    let oracle_square_shift = inputs.quantum_oracle_query_log2_bound.checked_mul(2)?;
    let sampling_numerator =
        checked_shift(u128::from(inputs.aggregate_targets), oracle_square_shift)?;

    // Generic quantum collision accounting uses Q^3 / 2^n. Every ordered target pair is
    // included; this intentionally over-counts self-pairs rather than understating the bound.
    let target_pairs =
        u128::from(inputs.aggregate_targets).checked_mul(u128::from(inputs.aggregate_targets))?;
    let oracle_cube_shift = inputs.quantum_oracle_query_log2_bound.checked_mul(3)?;
    let collision_numerator = checked_shift(target_pairs, oracle_cube_shift)?;
    Some((
        ExactDyadicBoundV1 {
            numerator: sampling_numerator,
            denominator_log2: sampling_denominator_log2,
        },
        ExactDyadicBoundV1 {
            numerator: collision_numerator,
            denominator_log2: inputs.digest_bits,
        },
    ))
}

fn exact_sum_below_target(
    left: ExactDyadicBoundV1,
    right: ExactDyadicBoundV1,
    target_bits: u32,
) -> bool {
    let common_denominator = left.denominator_log2.max(right.denominator_log2);
    let Some(left_scaled) = WideUint512V1::from_u128_shifted(
        left.numerator,
        common_denominator - left.denominator_log2,
    ) else {
        return false;
    };
    let Some(right_scaled) = WideUint512V1::from_u128_shifted(
        right.numerator,
        common_denominator - right.denominator_log2,
    ) else {
        return false;
    };
    let Some(sum) = left_scaled.checked_add(right_scaled) else {
        return false;
    };
    let Some(threshold_exponent) = common_denominator.checked_sub(target_bits) else {
        return false;
    };
    let Some(threshold) = WideUint512V1::power_of_two(threshold_exponent) else {
        return false;
    };
    sum.cmp_words(&threshold) == Ordering::Less
}

/// Evaluate the exact dyadic aggregate bound for one candidate query count.
#[must_use]
pub fn calculate_fastpq_qrom_bound_v1(
    inputs: FastpqQromBoundInputsV1,
    queries: u32,
) -> Option<FastpqQromBoundReportV1> {
    if queries < FASTPQ_MIN_QUERY_COUNT_V1 || queries % FASTPQ_QUERY_COUNT_GRANULARITY_V1 != 0 {
        return None;
    }
    let (sampling_term, collision_term) = qrom_bound_terms(inputs, queries)?;
    Some(FastpqQromBoundReportV1 {
        queries,
        sampling_term,
        collision_term,
        arithmetic_target_met: exact_sum_below_target(
            sampling_term,
            collision_term,
            inputs.required_security_bits,
        ),
        // Arithmetic does not substitute for the missing protocol-specific reduction.
        production_qualification: FastpqProductionQualificationV1::Unavailable(
            FastpqProductionQualificationBlockerV1::MissingProtocolSpecificQromReduction,
        ),
    })
}

/// Select the least multiple of eight, at least 64, whose exact aggregate arithmetic bound passes.
#[must_use]
pub fn select_fastpq_query_count_v1(inputs: FastpqQromBoundInputsV1) -> Option<u32> {
    (FASTPQ_MIN_QUERY_COUNT_V1..=FASTPQ_MAX_QUERY_COUNT_V1)
        .step_by(
            usize::try_from(FASTPQ_QUERY_COUNT_GRANULARITY_V1).expect("granularity fits usize"),
        )
        .find(|queries| {
            calculate_fastpq_qrom_bound_v1(inputs, *queries)
                .is_some_and(|report| report.arithmetic_target_met)
        })
}

/// Query count selected by [`select_fastpq_query_count_v1`] for the frozen V1 inputs.
pub const FASTPQ_QUERY_COUNT_V1: u32 = 136;

/// Sole canonical FASTPQ first-release parameter set.
pub const FASTPQ_FINAL_V1: StarkParameterSet = StarkParameterSet {
    name: FASTPQ_FINAL_V1_ID,
    required_security_bits: FASTPQ_REQUIRED_SECURITY_BITS_V1,
    grinding_bits: 0,
    trace_log_size: 16,
    trace_root: 0xbe5b_4f4b_47ee_4647,
    lde_log_size: 19,
    lde_root: 0xa9c4_68a3_57df_6e13,
    permutation_size: 65_536,
    lookup_log_size: 19,
    omega_coset: 0xfd0e_69f9_a98e_e946,
    field: GOLDILOCKS_FP4_V1,
    hash: POSEIDON_X7_GOLDILOCKS_DIGEST384_V1,
    fri: FriParameters {
        arity: 2,
        blowup_factor: 8,
        max_reductions: 18,
        queries: FASTPQ_QUERY_COUNT_V1,
    },
};

/// Ordered singleton slice of canonical parameter sets.
pub const CANONICAL_PARAMETER_SETS: [StarkParameterSet; 1] = [FASTPQ_FINAL_V1];

/// Look up the sole canonical parameter set by its exact V1 identifier.
#[must_use]
pub fn find_by_name(name: &str) -> Option<&'static StarkParameterSet> {
    (name == FASTPQ_FINAL_V1_ID).then_some(&FASTPQ_FINAL_V1)
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
    fn sole_profile_has_final_binary_fp4_shape() {
        let set = FASTPQ_FINAL_V1;
        assert_eq!(set.name, FASTPQ_FINAL_V1_ID);
        assert_eq!(set.required_security_bits, 128);
        assert_eq!(set.field, GOLDILOCKS_FP4_V1);
        assert_eq!(set.field.extension_degree, 4);
        assert_eq!(set.fri.arity, 2);
        assert_eq!(set.fri.blowup_factor, 8);
        assert_eq!(set.fri.queries, FASTPQ_QUERY_COUNT_V1);
        assert_eq!(set.fri.queries % 8, 0);
        assert!(set.fri.queries >= 64);
        assert_eq!(CANONICAL_PARAMETER_SETS, [set]);
    }

    #[test]
    fn old_profile_names_fail_closed() {
        let old_balanced = ["fastpq", "lane", "balanced"].join("-");
        let old_latency = ["fastpq", "lane", "latency"].join("-");
        assert!(find_by_name(&old_balanced).is_none());
        assert!(find_by_name(&old_latency).is_none());
        assert_eq!(find_by_name(FASTPQ_FINAL_V1_ID), Some(&FASTPQ_FINAL_V1));
    }

    #[test]
    fn exact_calculator_selects_the_frozen_query_count() {
        assert_eq!(
            select_fastpq_query_count_v1(FASTPQ_QROM_BOUND_INPUTS_V1),
            Some(FASTPQ_QUERY_COUNT_V1)
        );
        let previous = calculate_fastpq_qrom_bound_v1(
            FASTPQ_QROM_BOUND_INPUTS_V1,
            FASTPQ_QUERY_COUNT_V1 - FASTPQ_QUERY_COUNT_GRANULARITY_V1,
        )
        .expect("previous multiple of eight is admissible input");
        let selected =
            calculate_fastpq_qrom_bound_v1(FASTPQ_QROM_BOUND_INPUTS_V1, FASTPQ_QUERY_COUNT_V1)
                .expect("selected count is admissible input");
        assert!(!previous.arithmetic_target_met);
        assert!(selected.arithmetic_target_met);
        assert_eq!(
            selected.production_qualification,
            FastpqProductionQualificationV1::Unavailable(
                FastpqProductionQualificationBlockerV1::MissingProtocolSpecificQromReduction
            )
        );
    }

    #[test]
    fn malformed_query_counts_and_non_dyadic_blowups_are_rejected() {
        assert!(calculate_fastpq_qrom_bound_v1(FASTPQ_QROM_BOUND_INPUTS_V1, 63).is_none());
        assert!(calculate_fastpq_qrom_bound_v1(FASTPQ_QROM_BOUND_INPUTS_V1, 65).is_none());
        let mut invalid = FASTPQ_QROM_BOUND_INPUTS_V1;
        invalid.blowup_factor = 12;
        assert!(calculate_fastpq_qrom_bound_v1(invalid, FASTPQ_QUERY_COUNT_V1).is_none());
    }

    #[test]
    fn domain_roots_are_coherent_and_coset_is_outside_lde_subgroup() {
        let params = FASTPQ_FINAL_V1;
        for (root, log_size) in [
            (params.trace_root, params.trace_log_size),
            (params.lde_root, params.lde_log_size),
        ] {
            assert_eq!(pow_mod(root, 1_u64 << log_size), 1);
            assert_ne!(pow_mod(root, 1_u64 << (log_size - 1)), 1);
        }
        assert_eq!(
            pow_mod(params.lde_root, u64::from(params.fri.blowup_factor)),
            params.trace_root
        );
        assert_ne!(pow_mod(params.omega_coset, 1_u64 << params.lde_log_size), 1);
    }
}
