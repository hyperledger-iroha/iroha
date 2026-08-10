//! Private q-native polynomial-commitment prerequisite for RNS-Link clean-v1.
//!
//! This module is intentionally below every wire, receipt, readiness, and
//! release boundary.  It does two things only:
//!
//! 1. implements the arithmetic and authenticated-query kernel of a genuine
//!    Merkle + FRI low-degree proof over `Fq2`; and
//! 2. freezes an exact, fail-closed release-shape plan for all 38 RNS primes.
//!
//! Twenty-five release primes have only a `2^18` base-field root.  A
//! degree-`2N-2` polynomial therefore cannot receive a non-trivial base-field
//! FRI blow-up at `N=2^17`.  Every release prime does, however, have at least a
//! `2^19` root in `Fq2`, because
//! `v2(q^2-1) = v2(q-1) + v2(q+1) >= 19`.  The frozen plan uses a `2^19`
//! evaluation domain, two independently mixed FRI rows, and 160 common query
//! positions.  Merkle leaves are cross-limb vectors; algebra and fold
//! challenges remain field-specific and bind `(limb, modulus, row, layer)`.
//!
//! The plan is not release evidence.  In particular, the global layer is
//! 637,534,208 bytes and must live in a seekable external column store.  The
//! enumerated heap bound excludes OS page-cache residency and has not been
//! measured.  The exact FRI theorem has not been instantiated, no release KAT
//! exists, and no production wire consumes this prototype.  All qualification
//! booleans below consequently remain false.

use core::fmt;

use crate::vega::sponge::{keccak256, shake256};

use super::super::is_prime_u64;
use super::super::manifest::{
    RELEASE_MODULI_V1, ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1, release_profile_v1,
};

const PCS_VERSION_V1: u8 = 1;
const OPENING_REPETITIONS_V1: usize = 5;
const BATCH_ROWS_V1: usize = 2;
const RELEASE_LIMBS_V1: usize = 38;
const RELEASE_LOG_N_V1: usize = 17;
const RELEASE_DOMAIN_LOG_V1: usize = 19;
const RELEASE_DOMAIN_SIZE_V1: usize = 1 << RELEASE_DOMAIN_LOG_V1;
const RELEASE_MAX_DEGREE_V1: usize = 2 * ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1 - 2;
const RELEASE_QUOTIENT_MAX_DEGREE_V1: usize = ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1 - 2;
const RELEASE_FRI_ROUNDS_V1: usize = RELEASE_LOG_N_V1 + 1;
const RELEASE_FRI_QUERY_COUNT_V1: usize = 160;
const FQ2_WIRE_BYTES_V1: usize = 16;
const CROSS_LIMB_LEAF_BYTES_V1: usize = RELEASE_LIMBS_V1 * BATCH_ROWS_V1 * FQ2_WIRE_BYTES_V1;
const HASH_BYTES_V1: usize = 32;
const PROOF_CAP_BYTES_V1: usize = 32 * 1024 * 1024;
const RESIDENT_CAP_BYTES_V1: usize = 160 * 1024 * 1024;

const MERKLE_LEAF_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.phase23.rns-link.q-pcs.merkle-leaf";
const MERKLE_NODE_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.phase23.rns-link.q-pcs.merkle-node";
const PARAMETER_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.phase23.rns-link.q-pcs.parameters";
const CHALLENGE_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.phase23.rns-link.q-pcs.challenge";
const BATCH_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.phase23.rns-link.q-pcs.batch";
const FOLD_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.phase23.rns-link.q-pcs.fold";
const QUERY_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.phase23.rns-link.q-pcs.query";

// Exact maximum for the frozen cross-limb envelope.  The derivation is
// executable in `release_accounting_is_exact_and_fail_closed` below.
const RELEASE_INITIAL_MULTIPROOF_AUTH_HASHES_V1: usize = 3_392;
const RELEASE_FRI_MULTIPROOF_AUTH_HASHES_V1: usize = 19_712;
const RELEASE_FRI_OPENED_LEAVES_V1: usize = 4_028;
const RELEASE_MULTIPROOF_VALUE_BYTES_V1: usize = 5_676_288;
const RELEASE_MULTIPROOF_AUTH_BYTES_V1: usize = 847_872;
// 32-byte opening-quotient root + 18 FRI roots + two 1,216-byte
// terminal leaves + 38*5*2 canonical q evaluations + twenty 8-byte
// multiproof count headers + a 512-byte fixed envelope header.
const RELEASE_PROOF_FIXED_BYTES_V1: usize = 6_752;
const RELEASE_MAX_ENCODED_PROOF_BYTES_V1: usize = 6_530_912;
const RELEASE_REMAINING_GLOBAL_PROOF_BUDGET_BYTES_V1: usize =
    PROOF_CAP_BYTES_V1 - RELEASE_MAX_ENCODED_PROOF_BYTES_V1;

const RELEASE_COEFFICIENT_HEAP_BYTES_V1: usize = 6_291_344;
const RELEASE_FRI_CURRENT_AND_NEXT_HEAP_BYTES_V1: usize = 25_165_824;
const RELEASE_EXTERNAL_IO_BUFFER_BYTES_V1: usize = 8 * 1024 * 1024;
const RELEASE_MERKLE_FRONTIER_BYTES_V1: usize = 640;
const RELEASE_LEAF_BUFFER_BYTES_V1: usize = CROSS_LIMB_LEAF_BYTES_V1;
const RELEASE_ENUMERATED_HEAP_BYTES_V1: usize = RELEASE_COEFFICIENT_HEAP_BYTES_V1
    + RELEASE_FRI_CURRENT_AND_NEXT_HEAP_BYTES_V1
    + RELEASE_MAX_ENCODED_PROOF_BYTES_V1
    + RELEASE_EXTERNAL_IO_BUFFER_BYTES_V1
    + RELEASE_MERKLE_FRONTIER_BYTES_V1
    + RELEASE_LEAF_BUFFER_BYTES_V1;
const RELEASE_EXTERNAL_SCRATCH_BYTES_V1: usize = 956_301_312;

const RELEASE_FFT_BUTTERFLIES_PER_TRANSFORM_V1: u64 = 4_980_736;
// Twelve transforms per limb: two for commitment, two for the opening-
// quotient root pass, four for the batch/root pass, and four for proof-path
// extraction by recomputation.  12*38 = 456.
const RELEASE_FFT_TRANSFORM_COUNT_V1: u64 = 456;
const RELEASE_FFT_BUTTERFLIES_V1: u64 =
    RELEASE_FFT_BUTTERFLIES_PER_TRANSFORM_V1 * RELEASE_FFT_TRANSFORM_COUNT_V1;
// Two public trees, two opening-quotient trees, and two copies of the complete
// M..4 FRI tree sequence in the two-pass external-store schedule.
const RELEASE_MERKLE_HASH_INVOCATIONS_V1: u64 = 8_388_552;
// `(M-2) * 38 limbs * 2 rows * 2 passes` folded output coordinates.
const RELEASE_FRI_FOLDED_ROW_VALUES_V1: u64 = 79_691_472;
const RELEASE_CLASSIFIED_WORK_UNITS_V1: u64 = RELEASE_FFT_BUTTERFLIES_V1
    + RELEASE_MERKLE_HASH_INVOCATIONS_V1
    + RELEASE_FRI_FOLDED_ROW_VALUES_V1;
const RELEASE_WORK_CEILING_V1: u64 = 100_000_000_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum QPcsErrorV1 {
    InvalidGeometry,
    InvalidLimb,
    InvalidModulus,
    InvalidCoefficientCount,
    NonCanonicalResidue,
    NonCanonicalDegree,
    InvalidChunkLength,
    TrailingChunk,
    InvalidChallenge,
    ReusedChallenge,
    CommitmentMismatch,
    OpeningMismatch,
    InvalidMerkleProof,
    InvalidFriProof,
    ResourceCeilingExceeded,
    ExternalStoreRequired,
}

impl fmt::Display for QPcsErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

/// Exact release-shape feasibility result.  It is information, not authority.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ZkAmsPhase23RnsLinkQPcsReleasePlanV1 {
    release_parameter_digest: [u8; 32],
    limb_count: u8,
    minimum_base_two_adicity: u8,
    minimum_extension_two_adicity: u8,
    domain_log: u8,
    domain_size: u32,
    maximum_polynomial_degree: u32,
    maximum_relation_quotient_degree: u32,
    opening_repetitions: u8,
    batch_rows: u8,
    common_query_count: u16,
    fri_rounds: u8,
    cross_limb_leaf_bytes: u16,
    initial_multiproof_auth_hashes: u32,
    fri_multiproof_auth_hashes: u32,
    fri_opened_leaves: u32,
    maximum_encoded_proof_bytes: u64,
    remaining_global_proof_budget_bytes: u64,
    enumerated_heap_bytes: u64,
    external_scratch_bytes: u64,
    fft_transform_count: u16,
    fft_butterflies: u64,
    merkle_hash_invocations: u64,
    fri_folded_row_values: u64,
    classified_work_units: u64,
    fri_query_union_bound_bits_floor: u8,
    maximum_work_units: u64,
    cross_limb_vector_merkle_implemented: bool,
    seekable_external_store_implemented: bool,
    exact_fri_theorem_instantiated: bool,
    fiat_shamir_relation_adapter_implemented: bool,
    release_kat_matches: bool,
    measured_resident_set_within_cap: bool,
    production_wire_integrated: bool,
    non_pcs_sections_measured_within_remaining_budget: bool,
    release_qualified: bool,
}

fn zk_ams_phase23_rns_link_q_pcs_release_plan_v1()
-> Result<ZkAmsPhase23RnsLinkQPcsReleasePlanV1, QPcsErrorV1> {
    release_profile_v1()
        .validate()
        .map_err(|_| QPcsErrorV1::InvalidModulus)?;
    if RELEASE_MODULI_V1.len() != RELEASE_LIMBS_V1
        || ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1 != 1 << RELEASE_LOG_N_V1
        || RELEASE_DOMAIN_SIZE_V1 != 4 * ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1
        || RELEASE_MAX_DEGREE_V1 + 1 >= RELEASE_DOMAIN_SIZE_V1
    {
        return Err(QPcsErrorV1::InvalidGeometry);
    }

    let mut minimum_base_two_adicity = u32::MAX;
    let mut minimum_extension_two_adicity = u32::MAX;
    for (limb, &modulus) in RELEASE_MODULI_V1.iter().enumerate() {
        if modulus < 3
            || modulus >= 1_u64 << 62
            || modulus.is_multiple_of(2)
            || !is_prime_u64(modulus)
            || RELEASE_MODULI_V1[..limb].contains(&modulus)
        {
            return Err(QPcsErrorV1::InvalidModulus);
        }
        let base = (modulus - 1).trailing_zeros();
        let extension = base
            .checked_add((modulus + 1).trailing_zeros())
            .ok_or(QPcsErrorV1::ResourceCeilingExceeded)?;
        minimum_base_two_adicity = minimum_base_two_adicity.min(base);
        minimum_extension_two_adicity = minimum_extension_two_adicity.min(extension);
    }
    if minimum_base_two_adicity != 18
        || minimum_extension_two_adicity < RELEASE_DOMAIN_LOG_V1 as u32
    {
        return Err(QPcsErrorV1::InvalidGeometry);
    }
    let release_parameter_digest = q_pcs_parameter_digest_v1(
        QPcsGeometryV1 {
            ring_degree: ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1,
            domain_log: RELEASE_DOMAIN_LOG_V1,
            query_count: RELEASE_FRI_QUERY_COUNT_V1,
        },
        &RELEASE_MODULI_V1,
    )?;
    if release_parameter_digest == [0; 32] {
        return Err(QPcsErrorV1::InvalidGeometry);
    }

    let plan = ZkAmsPhase23RnsLinkQPcsReleasePlanV1 {
        release_parameter_digest,
        limb_count: RELEASE_LIMBS_V1 as u8,
        minimum_base_two_adicity: minimum_base_two_adicity as u8,
        minimum_extension_two_adicity: minimum_extension_two_adicity as u8,
        domain_log: RELEASE_DOMAIN_LOG_V1 as u8,
        domain_size: RELEASE_DOMAIN_SIZE_V1 as u32,
        maximum_polynomial_degree: RELEASE_MAX_DEGREE_V1 as u32,
        maximum_relation_quotient_degree: RELEASE_QUOTIENT_MAX_DEGREE_V1 as u32,
        opening_repetitions: OPENING_REPETITIONS_V1 as u8,
        batch_rows: BATCH_ROWS_V1 as u8,
        common_query_count: RELEASE_FRI_QUERY_COUNT_V1 as u16,
        fri_rounds: RELEASE_FRI_ROUNDS_V1 as u8,
        cross_limb_leaf_bytes: CROSS_LIMB_LEAF_BYTES_V1 as u16,
        initial_multiproof_auth_hashes: RELEASE_INITIAL_MULTIPROOF_AUTH_HASHES_V1 as u32,
        fri_multiproof_auth_hashes: RELEASE_FRI_MULTIPROOF_AUTH_HASHES_V1 as u32,
        fri_opened_leaves: RELEASE_FRI_OPENED_LEAVES_V1 as u32,
        maximum_encoded_proof_bytes: RELEASE_MAX_ENCODED_PROOF_BYTES_V1 as u64,
        remaining_global_proof_budget_bytes: RELEASE_REMAINING_GLOBAL_PROOF_BUDGET_BYTES_V1 as u64,
        enumerated_heap_bytes: RELEASE_ENUMERATED_HEAP_BYTES_V1 as u64,
        external_scratch_bytes: RELEASE_EXTERNAL_SCRATCH_BYTES_V1 as u64,
        fft_transform_count: RELEASE_FFT_TRANSFORM_COUNT_V1 as u16,
        fft_butterflies: RELEASE_FFT_BUTTERFLIES_V1,
        merkle_hash_invocations: RELEASE_MERKLE_HASH_INVOCATIONS_V1,
        fri_folded_row_values: RELEASE_FRI_FOLDED_ROW_VALUES_V1,
        classified_work_units: RELEASE_CLASSIFIED_WORK_UNITS_V1,
        fri_query_union_bound_bits_floor: 154,
        maximum_work_units: RELEASE_WORK_CEILING_V1,
        cross_limb_vector_merkle_implemented: false,
        seekable_external_store_implemented: false,
        exact_fri_theorem_instantiated: false,
        fiat_shamir_relation_adapter_implemented: false,
        release_kat_matches: false,
        measured_resident_set_within_cap: false,
        production_wire_integrated: false,
        non_pcs_sections_measured_within_remaining_budget: false,
        release_qualified: false,
    };

    // These inequalities show only that the frozen PCS envelope has room.
    // They deliberately do not turn any of the qualification bits above on.
    if plan.maximum_encoded_proof_bytes >= PROOF_CAP_BYTES_V1 as u64
        || plan.enumerated_heap_bytes >= RESIDENT_CAP_BYTES_V1 as u64
        || plan.classified_work_units >= plan.maximum_work_units
        || plan.remaining_global_proof_budget_bytes
            != (PROOF_CAP_BYTES_V1 - RELEASE_MAX_ENCODED_PROOF_BYTES_V1) as u64
    {
        return Err(QPcsErrorV1::ResourceCeilingExceeded);
    }
    Ok(plan)
}

/// Release proving cannot begin until the global external-store implementation
/// and the independent qualification evidence exist.
fn require_zk_ams_phase23_rns_link_q_pcs_release_prover_v1() -> Result<(), QPcsErrorV1> {
    let plan = zk_ams_phase23_rns_link_q_pcs_release_plan_v1()?;
    if !plan.cross_limb_vector_merkle_implemented || !plan.seekable_external_store_implemented {
        return Err(QPcsErrorV1::ExternalStoreRequired);
    }
    if !plan.exact_fri_theorem_instantiated
        || !plan.fiat_shamir_relation_adapter_implemented
        || !plan.release_kat_matches
        || !plan.measured_resident_set_within_cap
        || !plan.production_wire_integrated
        || !plan.non_pcs_sections_measured_within_remaining_budget
        || !plan.release_qualified
    {
        return Err(QPcsErrorV1::InvalidFriProof);
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Fq2V1 {
    c0: u64,
    c1: u64,
}

impl Fq2V1 {
    const ZERO: Self = Self { c0: 0, c1: 0 };
    const ONE: Self = Self { c0: 1, c1: 0 };

    const fn base(value: u64) -> Self {
        Self { c0: value, c1: 0 }
    }

    fn encode(self, modulus: u64) -> Result<[u8; FQ2_WIRE_BYTES_V1], QPcsErrorV1> {
        if self.c0 >= modulus || self.c1 >= modulus {
            return Err(QPcsErrorV1::NonCanonicalResidue);
        }
        let mut bytes = [0_u8; FQ2_WIRE_BYTES_V1];
        bytes[..8].copy_from_slice(&self.c0.to_be_bytes());
        bytes[8..].copy_from_slice(&self.c1.to_be_bytes());
        Ok(bytes)
    }

    fn decode(bytes: [u8; FQ2_WIRE_BYTES_V1], modulus: u64) -> Result<Self, QPcsErrorV1> {
        let c0 = u64::from_be_bytes(bytes[..8].try_into().expect("fixed first coordinate"));
        let c1 = u64::from_be_bytes(bytes[8..].try_into().expect("fixed second coordinate"));
        if c0 >= modulus || c1 >= modulus {
            return Err(QPcsErrorV1::NonCanonicalResidue);
        }
        Ok(Self { c0, c1 })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Fq2ParametersV1 {
    modulus: u64,
    nonresidue: u64,
    domain_root: Fq2V1,
    domain_log: u8,
}

impl Fq2ParametersV1 {
    fn derive(modulus: u64, domain_log: usize) -> Result<Self, QPcsErrorV1> {
        if modulus < 3
            || modulus >= 1_u64 << 62
            || modulus.is_multiple_of(2)
            || !is_prime_u64(modulus)
            || domain_log == 0
            || domain_log >= 63
            || (modulus - 1).trailing_zeros() + (modulus + 1).trailing_zeros() < domain_log as u32
        {
            return Err(QPcsErrorV1::InvalidModulus);
        }
        let nonresidue = (2_u64..=64)
            .find(|&candidate| mod_pow_v1(candidate, (modulus - 1) / 2, modulus) == modulus - 1)
            .ok_or(QPcsErrorV1::InvalidModulus)?;
        let mut parameters = Self {
            modulus,
            nonresidue,
            domain_root: Fq2V1::ZERO,
            domain_log: domain_log as u8,
        };
        let group_order = u128::from(modulus)
            .checked_mul(u128::from(modulus))
            .and_then(|value| value.checked_sub(1))
            .ok_or(QPcsErrorV1::ResourceCeilingExceeded)?;
        let exponent = group_order >> domain_log;
        'outer: for c0 in 1_u64..=32 {
            for c1 in 1_u64..=32 {
                let candidate = Fq2V1 {
                    c0: c0 % modulus,
                    c1: c1 % modulus,
                };
                let root = parameters.pow(candidate, exponent);
                if parameters.pow(root, 1_u128 << domain_log) == Fq2V1::ONE
                    && parameters.pow(root, 1_u128 << (domain_log - 1)) != Fq2V1::ONE
                {
                    parameters.domain_root = root;
                    break 'outer;
                }
            }
        }
        if parameters.domain_root == Fq2V1::ZERO {
            return Err(QPcsErrorV1::InvalidGeometry);
        }
        Ok(parameters)
    }

    fn add(self, left: Fq2V1, right: Fq2V1) -> Fq2V1 {
        Fq2V1 {
            c0: mod_add_v1(left.c0, right.c0, self.modulus),
            c1: mod_add_v1(left.c1, right.c1, self.modulus),
        }
    }

    fn sub(self, left: Fq2V1, right: Fq2V1) -> Fq2V1 {
        Fq2V1 {
            c0: mod_sub_v1(left.c0, right.c0, self.modulus),
            c1: mod_sub_v1(left.c1, right.c1, self.modulus),
        }
    }

    fn mul(self, left: Fq2V1, right: Fq2V1) -> Fq2V1 {
        let ac = mod_mul_v1(left.c0, right.c0, self.modulus);
        let bd = mod_mul_v1(left.c1, right.c1, self.modulus);
        let cross = mod_add_v1(
            mod_mul_v1(left.c0, right.c1, self.modulus),
            mod_mul_v1(left.c1, right.c0, self.modulus),
            self.modulus,
        );
        Fq2V1 {
            c0: mod_add_v1(
                ac,
                mod_mul_v1(bd, self.nonresidue, self.modulus),
                self.modulus,
            ),
            c1: cross,
        }
    }

    fn scale(self, value: Fq2V1, scalar: u64) -> Fq2V1 {
        Fq2V1 {
            c0: mod_mul_v1(value.c0, scalar, self.modulus),
            c1: mod_mul_v1(value.c1, scalar, self.modulus),
        }
    }

    fn pow(self, mut base: Fq2V1, mut exponent: u128) -> Fq2V1 {
        let mut result = Fq2V1::ONE;
        while exponent != 0 {
            if exponent & 1 == 1 {
                result = self.mul(result, base);
            }
            base = self.mul(base, base);
            exponent >>= 1;
        }
        result
    }

    fn inverse(self, value: Fq2V1) -> Result<Fq2V1, QPcsErrorV1> {
        if value == Fq2V1::ZERO {
            return Err(QPcsErrorV1::InvalidFriProof);
        }
        let exponent = u128::from(self.modulus)
            .checked_mul(u128::from(self.modulus))
            .and_then(|value| value.checked_sub(2))
            .ok_or(QPcsErrorV1::ResourceCeilingExceeded)?;
        Ok(self.pow(value, exponent))
    }
}

const fn mod_add_v1(left: u64, right: u64, modulus: u64) -> u64 {
    let sum = left + right;
    let (reduced, borrow) = sum.overflowing_sub(modulus);
    let mask = 0_u64.wrapping_sub(borrow as u64);
    (reduced & !mask) | (sum & mask)
}

const fn mod_sub_v1(left: u64, right: u64, modulus: u64) -> u64 {
    let (difference, borrow) = left.overflowing_sub(right);
    difference.wrapping_add(modulus & 0_u64.wrapping_sub(borrow as u64))
}

fn mod_mul_v1(left: u64, right: u64, modulus: u64) -> u64 {
    ((u128::from(left) * u128::from(right)) % u128::from(modulus)) as u64
}

fn mod_pow_v1(mut base: u64, mut exponent: u64, modulus: u64) -> u64 {
    let mut result = 1_u64;
    while exponent != 0 {
        if exponent & 1 == 1 {
            result = mod_mul_v1(result, base, modulus);
        }
        base = mod_mul_v1(base, base, modulus);
        exponent >>= 1;
    }
    result
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RelationPolynomialRoleV1 {
    Product,
    NegacyclicQuotient,
}

impl RelationPolynomialRoleV1 {
    const fn tag(self) -> u8 {
        match self {
            Self::Product => 1,
            Self::NegacyclicQuotient => 2,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct QPcsGeometryV1 {
    ring_degree: usize,
    domain_log: usize,
    query_count: usize,
}

impl QPcsGeometryV1 {
    fn validate(self) -> Result<(), QPcsErrorV1> {
        let domain_size = self.domain_size()?;
        let expected_domain_size = self
            .ring_degree
            .checked_mul(4)
            .ok_or(QPcsErrorV1::ResourceCeilingExceeded)?;
        if self.ring_degree < 8
            || !self.ring_degree.is_power_of_two()
            || self.domain_log != self.ring_degree.ilog2() as usize + 2
            || domain_size != expected_domain_size
            || self.query_count == 0
            || self.query_count > domain_size / 2
            || u32::try_from(self.ring_degree).is_err()
            || u8::try_from(self.domain_log).is_err()
            || u16::try_from(self.query_count).is_err()
            || u32::try_from(domain_size).is_err()
            || self
                .ring_degree
                .checked_add(OPENING_REPETITIONS_V1)
                .and_then(|value| u32::try_from(value).ok())
                .is_none()
        {
            return Err(QPcsErrorV1::InvalidGeometry);
        }
        Ok(())
    }

    fn domain_size(self) -> Result<usize, QPcsErrorV1> {
        let shift = u32::try_from(self.domain_log).map_err(|_| QPcsErrorV1::InvalidGeometry)?;
        1_usize
            .checked_shl(shift)
            .ok_or(QPcsErrorV1::ResourceCeilingExceeded)
    }

    fn fri_rounds(self) -> usize {
        self.ring_degree.ilog2() as usize + 1
    }

    fn degree_bound(self, role: RelationPolynomialRoleV1) -> usize {
        match role {
            RelationPolynomialRoleV1::Product => 2 * self.ring_degree - 2,
            RelationPolynomialRoleV1::NegacyclicQuotient => self.ring_degree - 2,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum InMemoryReferenceOperationV1 {
    Commit,
    Open,
}

fn in_memory_reference_residency_bytes_v1(
    geometry: QPcsGeometryV1,
    limb_count: usize,
    operation: InMemoryReferenceOperationV1,
) -> Result<usize, QPcsErrorV1> {
    geometry.validate()?;
    if limb_count == 0 {
        return Err(QPcsErrorV1::InvalidGeometry);
    }
    let domain_size = geometry.domain_size()?;
    let coordinate_count = limb_count
        .checked_mul(BATCH_ROWS_V1)
        .ok_or(QPcsErrorV1::ResourceCeilingExceeded)?;
    let layer_bytes = domain_size
        .checked_mul(coordinate_count)
        .and_then(|value| value.checked_mul(FQ2_WIRE_BYTES_V1))
        .ok_or(QPcsErrorV1::ResourceCeilingExceeded)?;
    let tree_bytes = domain_size
        .checked_mul(2)
        .and_then(|value| value.checked_mul(HASH_BYTES_V1))
        .ok_or(QPcsErrorV1::ResourceCeilingExceeded)?;
    if operation == InMemoryReferenceOperationV1::Commit {
        return layer_bytes
            .checked_add(tree_bytes)
            .ok_or(QPcsErrorV1::ResourceCeilingExceeded);
    }

    // The reference opening helper retains public and opening-quotient layers
    // and trees while its in-memory FRI helper retains a geometric sequence of
    // batch layers.  Four full layers plus three full trees conservatively
    // cover that cumulative live set; this is deliberately not the release
    // external-store topology.
    let retained_layers_and_trees = layer_bytes
        .checked_mul(4)
        .and_then(|value| value.checked_add(tree_bytes.checked_mul(3)?))
        .ok_or(QPcsErrorV1::ResourceCeilingExceeded)?;
    let opening_coefficients = geometry
        .ring_degree
        .checked_mul(3)
        .and_then(|value| value.checked_sub(12))
        .and_then(|value| value.checked_mul(limb_count))
        .and_then(|value| value.checked_mul(core::mem::size_of::<u64>()))
        .ok_or(QPcsErrorV1::ResourceCeilingExceeded)?;
    let multiproof_count = geometry
        .fri_rounds()
        .checked_add(2)
        .ok_or(QPcsErrorV1::ResourceCeilingExceeded)?;
    let opened_leaves = geometry
        .query_count
        .checked_mul(2)
        .ok_or(QPcsErrorV1::ResourceCeilingExceeded)?;
    let proof_values = multiproof_count
        .checked_mul(opened_leaves)
        .and_then(|value| value.checked_mul(coordinate_count))
        .and_then(|value| value.checked_mul(FQ2_WIRE_BYTES_V1))
        .ok_or(QPcsErrorV1::ResourceCeilingExceeded)?;
    let proof_authentication = multiproof_count
        .checked_mul(opened_leaves)
        .and_then(|value| value.checked_mul(geometry.domain_log))
        .and_then(|value| value.checked_mul(HASH_BYTES_V1))
        .ok_or(QPcsErrorV1::ResourceCeilingExceeded)?;
    retained_layers_and_trees
        .checked_add(opening_coefficients)
        .and_then(|value| value.checked_add(proof_values))
        .and_then(|value| value.checked_add(proof_authentication))
        .ok_or(QPcsErrorV1::ResourceCeilingExceeded)
}

fn preflight_in_memory_reference_v1(
    geometry: QPcsGeometryV1,
    limb_count: usize,
    operation: InMemoryReferenceOperationV1,
) -> Result<(), QPcsErrorV1> {
    if in_memory_reference_residency_bytes_v1(geometry, limb_count, operation)?
        >= RESIDENT_CAP_BYTES_V1
    {
        return Err(QPcsErrorV1::ExternalStoreRequired);
    }
    Ok(())
}

#[cfg(test)]
std::thread_local! {
    static IN_MEMORY_MATERIALIZATION_ATTEMPTS_V1: std::cell::Cell<usize> = const {
        std::cell::Cell::new(0)
    };
}

#[cfg(test)]
fn reset_in_memory_materialization_attempts_v1() {
    let _ = IN_MEMORY_MATERIALIZATION_ATTEMPTS_V1.try_with(|attempts| attempts.set(0));
}

#[cfg(test)]
fn in_memory_materialization_attempts_v1() -> usize {
    IN_MEMORY_MATERIALIZATION_ATTEMPTS_V1
        .try_with(std::cell::Cell::get)
        .unwrap_or(usize::MAX)
}

#[cfg(test)]
fn record_in_memory_materialization_attempt_v1() {
    let _ = IN_MEMORY_MATERIALIZATION_ATTEMPTS_V1
        .try_with(|attempts| attempts.set(attempts.get().saturating_add(1)));
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Caller-supplied challenge tuples exist only for the private reference
/// prover and hostile tests.  A release caller must not choose them: the
/// missing relation adapter must derive `(r, gamma, beta)` from the sealed
/// clean-v1 Fiat--Shamir transcript for every limb and repetition.
struct QPcsChallengeTupleV1 {
    r: u64,
    gamma: u64,
    beta: u64,
}

fn validate_challenge_tuples_v1(
    modulus: u64,
    challenges: &[QPcsChallengeTupleV1; OPENING_REPETITIONS_V1],
) -> Result<(), QPcsErrorV1> {
    let mut seen = [0_u64; OPENING_REPETITIONS_V1 * 3];
    let mut seen_len = 0_usize;
    for challenge in challenges {
        for value in [challenge.r, challenge.gamma, challenge.beta] {
            if value == 0 || value >= modulus {
                return Err(QPcsErrorV1::InvalidChallenge);
            }
            if seen[..seen_len].contains(&value) {
                return Err(QPcsErrorV1::ReusedChallenge);
            }
            seen[seen_len] = value;
            seen_len += 1;
        }
    }
    Ok(())
}

fn validate_cross_limb_challenges_v1(
    moduli: &[u64],
    challenges: &[[QPcsChallengeTupleV1; OPENING_REPETITIONS_V1]],
) -> Result<(), QPcsErrorV1> {
    if moduli.len() != challenges.len() {
        return Err(QPcsErrorV1::InvalidGeometry);
    }
    let mut prior_tuples = Vec::with_capacity(moduli.len() * OPENING_REPETITIONS_V1);
    for (limb, limb_challenges) in challenges.iter().enumerate() {
        validate_challenge_tuples_v1(moduli[limb], limb_challenges)?;
        for challenge in limb_challenges {
            if prior_tuples.contains(challenge) {
                return Err(QPcsErrorV1::ReusedChallenge);
            }
            prior_tuples.push(*challenge);
        }
    }
    Ok(())
}

trait CanonicalQPolynomialChunkSourceV1 {
    fn coefficient_count(&self) -> usize;

    /// Read the exact next coefficient chunk.  Full chunks contain 1,024
    /// big-endian `u64` residues; the final chunk has the exact remainder.
    /// Returning a different length, or data after the declared final chunk,
    /// is a canonical-transport failure.
    fn read_chunk(
        &mut self,
        chunk_index: usize,
        destination: &mut [u8; 8 * 1_024],
    ) -> Result<usize, QPcsErrorV1>;
}

fn read_canonical_polynomial_v1<S: CanonicalQPolynomialChunkSourceV1>(
    source: &mut S,
    modulus: u64,
    role: RelationPolynomialRoleV1,
    geometry: QPcsGeometryV1,
) -> Result<Vec<u64>, QPcsErrorV1> {
    geometry.validate()?;
    let coefficient_count = source.coefficient_count();
    let maximum_count = geometry
        .degree_bound(role)
        .checked_add(1)
        .ok_or(QPcsErrorV1::ResourceCeilingExceeded)?;
    if coefficient_count == 0 || coefficient_count > maximum_count {
        return Err(QPcsErrorV1::InvalidCoefficientCount);
    }
    let chunk_count = coefficient_count.div_ceil(1_024);
    let mut coefficients = Vec::new();
    coefficients
        .try_reserve_exact(coefficient_count)
        .map_err(|_| QPcsErrorV1::ResourceCeilingExceeded)?;
    let mut chunk = [0_u8; 8 * 1_024];
    for chunk_index in 0..chunk_count {
        chunk.fill(0);
        let remaining = coefficient_count - chunk_index * 1_024;
        let expected_coefficients = remaining.min(1_024);
        let expected_bytes = expected_coefficients * 8;
        let actual_bytes = source.read_chunk(chunk_index, &mut chunk)?;
        if actual_bytes != expected_bytes {
            return Err(QPcsErrorV1::InvalidChunkLength);
        }
        for encoded in chunk[..actual_bytes].chunks_exact(8) {
            let value = u64::from_be_bytes(encoded.try_into().expect("eight-byte residue"));
            if value >= modulus {
                return Err(QPcsErrorV1::NonCanonicalResidue);
            }
            coefficients.push(value);
        }
    }
    chunk.fill(0);
    if source.read_chunk(chunk_count, &mut chunk)? != 0 {
        return Err(QPcsErrorV1::TrailingChunk);
    }
    if coefficients.len() != coefficient_count
        || (coefficient_count > 1 && coefficients.last() == Some(&0))
    {
        return Err(QPcsErrorV1::NonCanonicalDegree);
    }
    Ok(coefficients)
}

fn evaluate_base_polynomial_v1(coefficients: &[u64], point: u64, modulus: u64) -> u64 {
    coefficients.iter().rev().fold(0_u64, |accumulator, value| {
        mod_add_v1(mod_mul_v1(accumulator, point, modulus), *value, modulus)
    })
}

fn opening_vanishing_polynomial_v1(
    modulus: u64,
    challenges: &[QPcsChallengeTupleV1; OPENING_REPETITIONS_V1],
) -> Vec<u64> {
    let mut polynomial = vec![1_u64];
    for challenge in challenges {
        let mut next = vec![0_u64; polynomial.len() + 1];
        for (index, &coefficient) in polynomial.iter().enumerate() {
            next[index] = mod_sub_v1(
                next[index],
                mod_mul_v1(coefficient, challenge.r, modulus),
                modulus,
            );
            next[index + 1] = mod_add_v1(next[index + 1], coefficient, modulus);
        }
        polynomial = next;
    }
    polynomial
}

fn interpolate_openings_v1(
    modulus: u64,
    challenges: &[QPcsChallengeTupleV1; OPENING_REPETITIONS_V1],
    evaluations: &[u64; OPENING_REPETITIONS_V1],
) -> Result<Vec<u64>, QPcsErrorV1> {
    validate_challenge_tuples_v1(modulus, challenges)?;
    let mut interpolation = vec![0_u64; OPENING_REPETITIONS_V1];
    for (index, challenge) in challenges.iter().enumerate() {
        let mut numerator = vec![1_u64];
        let mut denominator = 1_u64;
        for (other_index, other) in challenges.iter().enumerate() {
            if other_index == index {
                continue;
            }
            denominator = mod_mul_v1(
                denominator,
                mod_sub_v1(challenge.r, other.r, modulus),
                modulus,
            );
            let mut next = vec![0_u64; numerator.len() + 1];
            for (coefficient_index, &coefficient) in numerator.iter().enumerate() {
                next[coefficient_index] = mod_sub_v1(
                    next[coefficient_index],
                    mod_mul_v1(coefficient, other.r, modulus),
                    modulus,
                );
                next[coefficient_index + 1] =
                    mod_add_v1(next[coefficient_index + 1], coefficient, modulus);
            }
            numerator = next;
        }
        if denominator == 0 {
            return Err(QPcsErrorV1::ReusedChallenge);
        }
        let scale = mod_mul_v1(
            evaluations[index],
            mod_pow_v1(denominator, modulus - 2, modulus),
            modulus,
        );
        for (coefficient_index, coefficient) in numerator.into_iter().enumerate() {
            interpolation[coefficient_index] = mod_add_v1(
                interpolation[coefficient_index],
                mod_mul_v1(coefficient, scale, modulus),
                modulus,
            );
        }
    }
    Ok(interpolation)
}

fn five_point_opening_quotient_v1(
    coefficients: &[u64],
    modulus: u64,
    challenges: &[QPcsChallengeTupleV1; OPENING_REPETITIONS_V1],
    evaluations: &[u64; OPENING_REPETITIONS_V1],
) -> Result<Vec<u64>, QPcsErrorV1> {
    let interpolation = interpolate_openings_v1(modulus, challenges, evaluations)?;
    let divisor = opening_vanishing_polynomial_v1(modulus, challenges);
    let mut remainder = coefficients.to_vec();
    if remainder.len() < interpolation.len() {
        remainder.resize(interpolation.len(), 0);
    }
    for (index, value) in interpolation.into_iter().enumerate() {
        remainder[index] = mod_sub_v1(remainder[index], value, modulus);
    }
    if remainder.len() <= OPENING_REPETITIONS_V1 {
        if remainder.iter().any(|value| *value != 0) {
            return Err(QPcsErrorV1::OpeningMismatch);
        }
        return Ok(vec![0]);
    }
    let quotient_len = remainder.len() - OPENING_REPETITIONS_V1;
    let mut quotient = vec![0_u64; quotient_len];
    for degree in (OPENING_REPETITIONS_V1..remainder.len()).rev() {
        let leading = remainder[degree];
        let quotient_index = degree - OPENING_REPETITIONS_V1;
        quotient[quotient_index] = leading;
        for (divisor_index, divisor_coefficient) in divisor.iter().copied().enumerate() {
            let remainder_index = quotient_index + divisor_index;
            remainder[remainder_index] = mod_sub_v1(
                remainder[remainder_index],
                mod_mul_v1(leading, divisor_coefficient, modulus),
                modulus,
            );
        }
    }
    if remainder[..OPENING_REPETITIONS_V1]
        .iter()
        .any(|value| *value != 0)
    {
        return Err(QPcsErrorV1::OpeningMismatch);
    }
    while quotient.len() > 1 && quotient.last() == Some(&0) {
        quotient.pop();
    }
    Ok(quotient)
}

fn fft_forward_v1(
    coefficients: &[u64],
    parameters: Fq2ParametersV1,
) -> Result<Vec<Fq2V1>, QPcsErrorV1> {
    let domain_size = 1_usize
        .checked_shl(u32::from(parameters.domain_log))
        .ok_or(QPcsErrorV1::ResourceCeilingExceeded)?;
    if coefficients.len() > domain_size
        || coefficients
            .iter()
            .any(|value| *value >= parameters.modulus)
    {
        return Err(QPcsErrorV1::NonCanonicalResidue);
    }
    let mut values = vec![Fq2V1::ZERO; domain_size];
    for (destination, &coefficient) in values.iter_mut().zip(coefficients) {
        *destination = Fq2V1::base(coefficient);
    }
    for index in 1..domain_size {
        let reversed = index.reverse_bits() >> (usize::BITS - u32::from(parameters.domain_log));
        if index < reversed {
            values.swap(index, reversed);
        }
    }
    let mut length = 2_usize;
    while length <= domain_size {
        let twiddle_step = parameters.pow(parameters.domain_root, (domain_size / length) as u128);
        for chunk_start in (0..domain_size).step_by(length) {
            let mut twiddle = Fq2V1::ONE;
            for offset in 0..length / 2 {
                let even = values[chunk_start + offset];
                let odd = parameters.mul(values[chunk_start + offset + length / 2], twiddle);
                values[chunk_start + offset] = parameters.add(even, odd);
                values[chunk_start + offset + length / 2] = parameters.sub(even, odd);
                twiddle = parameters.mul(twiddle, twiddle_step);
            }
        }
        length = length
            .checked_mul(2)
            .ok_or(QPcsErrorV1::ResourceCeilingExceeded)?;
    }
    Ok(values)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MerkleLayerKindV1 {
    Public = 1,
    OpeningQuotient = 2,
    FriBatch = 3,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct CrossLimbLayerV1 {
    moduli: Vec<u64>,
    rows_per_limb: usize,
    columns: Vec<Vec<Fq2V1>>,
}

impl CrossLimbLayerV1 {
    fn validate(&self) -> Result<usize, QPcsErrorV1> {
        if self.moduli.is_empty()
            || self.rows_per_limb == 0
            || self.columns.len() != self.moduli.len() * self.rows_per_limb
        {
            return Err(QPcsErrorV1::InvalidGeometry);
        }
        let length = self
            .columns
            .first()
            .map(Vec::len)
            .ok_or(QPcsErrorV1::InvalidGeometry)?;
        if length < 2
            || !length.is_power_of_two()
            || self.columns.iter().any(|column| column.len() != length)
        {
            return Err(QPcsErrorV1::InvalidGeometry);
        }
        for (coordinate, column) in self.columns.iter().enumerate() {
            let modulus = self.moduli[coordinate / self.rows_per_limb];
            if column
                .iter()
                .any(|value| value.c0 >= modulus || value.c1 >= modulus)
            {
                return Err(QPcsErrorV1::NonCanonicalResidue);
            }
        }
        Ok(length)
    }

    fn coordinate_count(&self) -> usize {
        self.columns.len()
    }

    fn leaf_values(&self, index: usize) -> Result<Vec<Fq2V1>, QPcsErrorV1> {
        let length = self.validate()?;
        if index >= length {
            return Err(QPcsErrorV1::InvalidMerkleProof);
        }
        Ok(self.columns.iter().map(|column| column[index]).collect())
    }
}

fn q_pcs_parameter_digest_v1(
    geometry: QPcsGeometryV1,
    moduli: &[u64],
) -> Result<[u8; 32], QPcsErrorV1> {
    geometry.validate()?;
    if moduli.is_empty() || moduli.len() > u8::MAX as usize {
        return Err(QPcsErrorV1::InvalidGeometry);
    }
    for (limb, &modulus) in moduli.iter().enumerate() {
        if !is_prime_u64(modulus) || moduli[..limb].contains(&modulus) {
            return Err(QPcsErrorV1::InvalidModulus);
        }
    }
    let ring_degree =
        u32::try_from(geometry.ring_degree).map_err(|_| QPcsErrorV1::InvalidGeometry)?;
    let domain_log = u8::try_from(geometry.domain_log).map_err(|_| QPcsErrorV1::InvalidGeometry)?;
    let query_count =
        u16::try_from(geometry.query_count).map_err(|_| QPcsErrorV1::InvalidGeometry)?;
    let opening_repetitions =
        u32::try_from(OPENING_REPETITIONS_V1).map_err(|_| QPcsErrorV1::InvalidGeometry)?;
    let opening_repetitions_u8 =
        u8::try_from(OPENING_REPETITIONS_V1).map_err(|_| QPcsErrorV1::InvalidGeometry)?;
    let batch_rows = u8::try_from(BATCH_ROWS_V1).map_err(|_| QPcsErrorV1::InvalidGeometry)?;
    let quotient_shift = u32::try_from(
        geometry
            .ring_degree
            .checked_add(OPENING_REPETITIONS_V1)
            .ok_or(QPcsErrorV1::ResourceCeilingExceeded)?,
    )
    .map_err(|_| QPcsErrorV1::InvalidGeometry)?;
    let limb_count = u8::try_from(moduli.len()).map_err(|_| QPcsErrorV1::InvalidGeometry)?;
    let mut frame = Vec::with_capacity(PARAMETER_DOMAIN_V1.len() + 32 + moduli.len() * 8);
    frame.extend_from_slice(PARAMETER_DOMAIN_V1);
    frame.push(PCS_VERSION_V1);
    frame.extend_from_slice(&ring_degree.to_be_bytes());
    frame.push(domain_log);
    frame.extend_from_slice(&query_count.to_be_bytes());
    frame.push(opening_repetitions_u8);
    frame.push(batch_rows);
    frame.push(RelationPolynomialRoleV1::Product.tag());
    frame.extend_from_slice(&0_u32.to_be_bytes());
    frame.push(RelationPolynomialRoleV1::NegacyclicQuotient.tag());
    frame.extend_from_slice(&ring_degree.to_be_bytes());
    frame.push(3); // five-point quotient of P
    frame.extend_from_slice(&opening_repetitions.to_be_bytes());
    frame.push(4); // five-point quotient of H
    frame.extend_from_slice(&quotient_shift.to_be_bytes());
    frame.push(limb_count);
    for (limb, modulus) in moduli.iter().copied().enumerate() {
        frame.push(u8::try_from(limb).map_err(|_| QPcsErrorV1::InvalidGeometry)?);
        frame.extend_from_slice(&modulus.to_be_bytes());
    }
    Ok(keccak256(&frame))
}

fn merkle_leaf_hash_v1(
    parameter_digest: [u8; 32],
    kind: MerkleLayerKindV1,
    layer: u8,
    index: usize,
    length: usize,
    values: &[Fq2V1],
    moduli: &[u64],
    rows_per_limb: usize,
) -> Result<[u8; 32], QPcsErrorV1> {
    let coordinate_count = moduli
        .len()
        .checked_mul(rows_per_limb)
        .ok_or(QPcsErrorV1::ResourceCeilingExceeded)?;
    if values.len() != coordinate_count {
        return Err(QPcsErrorV1::InvalidMerkleProof);
    }
    let index = u32::try_from(index).map_err(|_| QPcsErrorV1::InvalidGeometry)?;
    let length = u32::try_from(length).map_err(|_| QPcsErrorV1::InvalidGeometry)?;
    let value_count = u16::try_from(values.len()).map_err(|_| QPcsErrorV1::InvalidGeometry)?;
    let mut frame = Vec::with_capacity(MERKLE_LEAF_DOMAIN_V1.len() + 56 + values.len() * 16);
    frame.extend_from_slice(MERKLE_LEAF_DOMAIN_V1);
    frame.push(PCS_VERSION_V1);
    frame.extend_from_slice(&parameter_digest);
    frame.push(kind as u8);
    frame.push(layer);
    frame.extend_from_slice(&index.to_be_bytes());
    frame.extend_from_slice(&length.to_be_bytes());
    frame.extend_from_slice(&value_count.to_be_bytes());
    for (coordinate, value) in values.iter().copied().enumerate() {
        frame.extend_from_slice(&value.encode(moduli[coordinate / rows_per_limb])?);
    }
    Ok(keccak256(&frame))
}

fn merkle_node_hash_v1(
    parameter_digest: [u8; 32],
    kind: MerkleLayerKindV1,
    layer: u8,
    height: u8,
    parent_index: usize,
    left: [u8; 32],
    right: [u8; 32],
) -> Result<[u8; 32], QPcsErrorV1> {
    let parent_index = u32::try_from(parent_index).map_err(|_| QPcsErrorV1::InvalidGeometry)?;
    let mut frame = Vec::with_capacity(MERKLE_NODE_DOMAIN_V1.len() + 104);
    frame.extend_from_slice(MERKLE_NODE_DOMAIN_V1);
    frame.push(PCS_VERSION_V1);
    frame.extend_from_slice(&parameter_digest);
    frame.push(kind as u8);
    frame.push(layer);
    frame.push(height);
    frame.extend_from_slice(&parent_index.to_be_bytes());
    frame.extend_from_slice(&left);
    frame.extend_from_slice(&right);
    Ok(keccak256(&frame))
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct MerkleMultiProofV1 {
    /// Values are leaf-major and then coordinate-major, in the verifier's
    /// sorted, deduplicated index order.
    values: Vec<Fq2V1>,
    authentication_nodes: Vec<[u8; 32]>,
}

impl MerkleMultiProofV1 {
    fn encoded_len(&self) -> Result<usize, QPcsErrorV1> {
        8_usize
            .checked_add(
                self.values
                    .len()
                    .checked_mul(FQ2_WIRE_BYTES_V1)
                    .ok_or(QPcsErrorV1::ResourceCeilingExceeded)?,
            )
            .and_then(|value| {
                value.checked_add(self.authentication_nodes.len().checked_mul(HASH_BYTES_V1)?)
            })
            .ok_or(QPcsErrorV1::ResourceCeilingExceeded)
    }
}

struct MerkleTreeV1 {
    parameter_digest: [u8; 32],
    kind: MerkleLayerKindV1,
    layer: u8,
    length: usize,
    rows_per_limb: usize,
    moduli: Vec<u64>,
    hashes: Vec<[u8; 32]>,
}

impl MerkleTreeV1 {
    fn build(
        parameter_digest: [u8; 32],
        kind: MerkleLayerKindV1,
        layer: u8,
        values: &CrossLimbLayerV1,
    ) -> Result<Self, QPcsErrorV1> {
        let length = values.validate()?;
        let allocation = length
            .checked_mul(2)
            .ok_or(QPcsErrorV1::ResourceCeilingExceeded)?;
        let mut hashes = Vec::new();
        hashes
            .try_reserve_exact(allocation)
            .map_err(|_| QPcsErrorV1::ResourceCeilingExceeded)?;
        hashes.resize(allocation, [0; 32]);
        for index in 0..length {
            let leaf_values = values.leaf_values(index)?;
            hashes[length + index] = merkle_leaf_hash_v1(
                parameter_digest,
                kind,
                layer,
                index,
                length,
                &leaf_values,
                &values.moduli,
                values.rows_per_limb,
            )?;
        }
        let mut nodes_at_height = length;
        let mut height = 1_u8;
        while nodes_at_height > 1 {
            let parents = nodes_at_height / 2;
            let parent_base = parents;
            let child_base = nodes_at_height;
            for parent in 0..parents {
                hashes[parent_base + parent] = merkle_node_hash_v1(
                    parameter_digest,
                    kind,
                    layer,
                    height,
                    parent,
                    hashes[child_base + 2 * parent],
                    hashes[child_base + 2 * parent + 1],
                )?;
            }
            nodes_at_height = parents;
            height = height
                .checked_add(1)
                .ok_or(QPcsErrorV1::ResourceCeilingExceeded)?;
        }
        Ok(Self {
            parameter_digest,
            kind,
            layer,
            length,
            rows_per_limb: values.rows_per_limb,
            moduli: values.moduli.clone(),
            hashes,
        })
    }

    fn root(&self) -> [u8; 32] {
        self.hashes[1]
    }

    fn open(
        &self,
        values: &CrossLimbLayerV1,
        indices: &[usize],
    ) -> Result<MerkleMultiProofV1, QPcsErrorV1> {
        if values.validate()? != self.length
            || values.rows_per_limb != self.rows_per_limb
            || values.moduli != self.moduli
        {
            return Err(QPcsErrorV1::InvalidMerkleProof);
        }
        let indices = canonical_indices_v1(indices, self.length)?;
        let mut opened_values = Vec::new();
        opened_values
            .try_reserve_exact(indices.len() * values.coordinate_count())
            .map_err(|_| QPcsErrorV1::ResourceCeilingExceeded)?;
        for &index in &indices {
            opened_values.extend(values.leaf_values(index)?);
        }

        let mut current: Vec<usize> = indices.iter().map(|index| self.length + index).collect();
        let mut authentication_nodes = Vec::new();
        while current.first().copied() != Some(1) || current.len() != 1 {
            let mut parents = Vec::with_capacity(current.len());
            let mut cursor = 0_usize;
            while cursor < current.len() {
                let node = current[cursor];
                let sibling = node ^ 1;
                if node.is_multiple_of(2) && current.get(cursor + 1).copied() == Some(sibling) {
                    cursor += 2;
                } else {
                    authentication_nodes.push(self.hashes[sibling]);
                    cursor += 1;
                }
                parents.push(node / 2);
            }
            parents.sort_unstable();
            parents.dedup();
            current = parents;
        }
        Ok(MerkleMultiProofV1 {
            values: opened_values,
            authentication_nodes,
        })
    }
}

fn canonical_indices_v1(indices: &[usize], length: usize) -> Result<Vec<usize>, QPcsErrorV1> {
    if indices.is_empty() || length < 2 || !length.is_power_of_two() {
        return Err(QPcsErrorV1::InvalidMerkleProof);
    }
    let mut canonical = indices.to_vec();
    canonical.sort_unstable();
    canonical.dedup();
    if canonical.last().copied().unwrap_or(length) >= length {
        return Err(QPcsErrorV1::InvalidMerkleProof);
    }
    Ok(canonical)
}

fn verify_merkle_multi_proof_v1(
    expected_root: [u8; 32],
    parameter_digest: [u8; 32],
    kind: MerkleLayerKindV1,
    layer: u8,
    length: usize,
    rows_per_limb: usize,
    moduli: &[u64],
    indices: &[usize],
    proof: &MerkleMultiProofV1,
) -> Result<(), QPcsErrorV1> {
    let indices = canonical_indices_v1(indices, length)?;
    let coordinate_count = moduli
        .len()
        .checked_mul(rows_per_limb)
        .ok_or(QPcsErrorV1::ResourceCeilingExceeded)?;
    if proof.values.len() != indices.len() * coordinate_count {
        return Err(QPcsErrorV1::InvalidMerkleProof);
    }
    let mut current = Vec::with_capacity(indices.len());
    for (leaf_number, &index) in indices.iter().enumerate() {
        let start = leaf_number * coordinate_count;
        let leaf_hash = merkle_leaf_hash_v1(
            parameter_digest,
            kind,
            layer,
            index,
            length,
            &proof.values[start..start + coordinate_count],
            moduli,
            rows_per_limb,
        )?;
        current.push((length + index, leaf_hash));
    }
    let mut authentication_cursor = 0_usize;
    let mut height = 1_u8;
    while current.first().map(|entry| entry.0) != Some(1) || current.len() != 1 {
        let mut parents = Vec::with_capacity(current.len());
        let mut cursor = 0_usize;
        while cursor < current.len() {
            let (node, node_hash) = current[cursor];
            let sibling = node ^ 1;
            let (left, right);
            if node.is_multiple_of(2)
                && current.get(cursor + 1).map(|entry| entry.0) == Some(sibling)
            {
                left = node_hash;
                right = current[cursor + 1].1;
                cursor += 2;
            } else {
                let authentication = proof
                    .authentication_nodes
                    .get(authentication_cursor)
                    .copied()
                    .ok_or(QPcsErrorV1::InvalidMerkleProof)?;
                authentication_cursor += 1;
                if node.is_multiple_of(2) {
                    left = node_hash;
                    right = authentication;
                } else {
                    left = authentication;
                    right = node_hash;
                }
                cursor += 1;
            }
            parents.push((
                node / 2,
                merkle_node_hash_v1(
                    parameter_digest,
                    kind,
                    layer,
                    height,
                    node / 2 - (length >> height),
                    left,
                    right,
                )?,
            ));
        }
        parents.sort_unstable_by_key(|entry| entry.0);
        parents.dedup_by_key(|entry| entry.0);
        current = parents;
        height = height
            .checked_add(1)
            .ok_or(QPcsErrorV1::ResourceCeilingExceeded)?;
    }
    if authentication_cursor != proof.authentication_nodes.len() || current[0].1 != expected_root {
        return Err(QPcsErrorV1::InvalidMerkleProof);
    }
    Ok(())
}

fn opened_leaf_v1<'a>(
    proof: &'a MerkleMultiProofV1,
    indices: &[usize],
    coordinate_count: usize,
    index: usize,
) -> Result<&'a [Fq2V1], QPcsErrorV1> {
    let position = indices
        .binary_search(&index)
        .map_err(|_| QPcsErrorV1::InvalidMerkleProof)?;
    let start = position
        .checked_mul(coordinate_count)
        .ok_or(QPcsErrorV1::ResourceCeilingExceeded)?;
    proof
        .values
        .get(start..start + coordinate_count)
        .ok_or(QPcsErrorV1::InvalidMerkleProof)
}

fn query_pair_indices_v1(query_positions: &[usize], length: usize) -> Vec<usize> {
    let half = length / 2;
    let mut indices = Vec::with_capacity(query_positions.len() * 2);
    for position in query_positions {
        let base = *position % half;
        indices.push(base);
        indices.push(base + half);
    }
    indices.sort_unstable();
    indices.dedup();
    indices
}

fn derive_field_challenge_v1(
    domain: &[u8],
    transcript_digest: [u8; 32],
    limb: usize,
    modulus: u64,
    row: usize,
    layer: usize,
) -> Result<Fq2V1, QPcsErrorV1> {
    for attempt in 0_u32..128 {
        let mut frame = Vec::with_capacity(domain.len() + 64);
        frame.extend_from_slice(domain);
        frame.push(PCS_VERSION_V1);
        frame.extend_from_slice(&transcript_digest);
        frame.push(limb as u8);
        frame.extend_from_slice(&modulus.to_be_bytes());
        frame.push(row as u8);
        frame.push(layer as u8);
        frame.extend_from_slice(&attempt.to_be_bytes());
        let uniform = shake256(&frame, 16);
        let c0 = u64::from_be_bytes(uniform[..8].try_into().expect("first challenge limb"));
        let c1 = u64::from_be_bytes(uniform[8..].try_into().expect("second challenge limb"));
        let zone = u64::MAX - u64::MAX % modulus;
        if c0 < zone && c1 < zone {
            let value = Fq2V1 {
                c0: c0 % modulus,
                c1: c1 % modulus,
            };
            if value != Fq2V1::ZERO {
                return Ok(value);
            }
        }
    }
    Err(QPcsErrorV1::InvalidChallenge)
}

fn fold_cross_limb_layer_v1(
    current: &CrossLimbLayerV1,
    parameters: &[Fq2ParametersV1],
    alphas: &[[Fq2V1; BATCH_ROWS_V1]],
) -> Result<CrossLimbLayerV1, QPcsErrorV1> {
    let length = current.validate()?;
    if current.rows_per_limb != BATCH_ROWS_V1
        || parameters.len() != current.moduli.len()
        || alphas.len() != current.moduli.len()
        || length < 4
    {
        return Err(QPcsErrorV1::InvalidFriProof);
    }
    let half = length / 2;
    let two_inverse: Vec<u64> = current
        .moduli
        .iter()
        .map(|modulus| mod_pow_v1(2, modulus - 2, *modulus))
        .collect();
    let mut columns = vec![vec![Fq2V1::ZERO; half]; current.columns.len()];
    for limb in 0..current.moduli.len() {
        let field = parameters[limb];
        let layer_log = length.ilog2();
        let root = field.pow(
            field.domain_root,
            1_u128 << (u32::from(field.domain_log) - layer_log),
        );
        let mut x = Fq2V1::ONE;
        for index in 0..half {
            let inverse_two_x = field.scale(field.inverse(x)?, two_inverse[limb]);
            for row in 0..BATCH_ROWS_V1 {
                let coordinate = limb * BATCH_ROWS_V1 + row;
                let positive = current.columns[coordinate][index];
                let negative = current.columns[coordinate][index + half];
                let even = field.scale(field.add(positive, negative), two_inverse[limb]);
                let odd = field.mul(field.sub(positive, negative), inverse_two_x);
                columns[coordinate][index] = field.add(even, field.mul(alphas[limb][row], odd));
            }
            x = field.mul(x, root);
        }
    }
    Ok(CrossLimbLayerV1 {
        moduli: current.moduli.clone(),
        rows_per_limb: BATCH_ROWS_V1,
        columns,
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct CrossLimbFriProofV1 {
    /// Roots for lengths `M, M/2, ..., 4`; the length-two terminal is opened.
    layer_roots: Vec<[u8; 32]>,
    terminal_values: Vec<Fq2V1>,
    layer_openings: Vec<MerkleMultiProofV1>,
}

impl CrossLimbFriProofV1 {
    fn encoded_len(&self) -> Result<usize, QPcsErrorV1> {
        let roots = self
            .layer_roots
            .len()
            .checked_mul(HASH_BYTES_V1)
            .ok_or(QPcsErrorV1::ResourceCeilingExceeded)?;
        let terminal = self
            .terminal_values
            .len()
            .checked_mul(FQ2_WIRE_BYTES_V1)
            .ok_or(QPcsErrorV1::ResourceCeilingExceeded)?;
        self.layer_openings.iter().try_fold(
            8_usize
                .checked_add(roots)
                .and_then(|value| value.checked_add(terminal))
                .ok_or(QPcsErrorV1::ResourceCeilingExceeded)?,
            |total, opening| {
                total
                    .checked_add(opening.encoded_len()?)
                    .ok_or(QPcsErrorV1::ResourceCeilingExceeded)
            },
        )
    }
}

fn absorb_fri_root_v1(transcript_digest: [u8; 32], layer: usize, root: [u8; 32]) -> [u8; 32] {
    let mut frame = Vec::with_capacity(FOLD_DOMAIN_V1.len() + 70);
    frame.extend_from_slice(FOLD_DOMAIN_V1);
    frame.push(PCS_VERSION_V1);
    frame.extend_from_slice(&transcript_digest);
    frame.push(layer as u8);
    frame.extend_from_slice(&root);
    keccak256(&frame)
}

fn absorb_fri_terminal_v1(
    transcript_digest: [u8; 32],
    terminal_values: &[Fq2V1],
    moduli: &[u64],
) -> Result<[u8; 32], QPcsErrorV1> {
    if terminal_values.len() != moduli.len() * BATCH_ROWS_V1 * 2 {
        return Err(QPcsErrorV1::InvalidFriProof);
    }
    let mut frame = Vec::with_capacity(FOLD_DOMAIN_V1.len() + 40 + terminal_values.len() * 16);
    frame.extend_from_slice(FOLD_DOMAIN_V1);
    frame.push(PCS_VERSION_V1);
    frame.extend_from_slice(&transcript_digest);
    frame.push(0xff);
    let coordinates = moduli.len() * BATCH_ROWS_V1;
    for (index, value) in terminal_values.iter().copied().enumerate() {
        frame.extend_from_slice(&value.encode(moduli[(index % coordinates) / BATCH_ROWS_V1])?);
    }
    Ok(keccak256(&frame))
}

fn derive_common_query_positions_v1(
    transcript_digest: [u8; 32],
    query_count: usize,
    initial_length: usize,
) -> Result<Vec<usize>, QPcsErrorV1> {
    if query_count == 0 || query_count > initial_length / 2 || u16::try_from(query_count).is_err() {
        return Err(QPcsErrorV1::InvalidGeometry);
    }
    let bound = initial_length / 2;
    let zone = u64::MAX - u64::MAX % bound as u64;
    let mut positions = Vec::new();
    positions
        .try_reserve_exact(query_count)
        .map_err(|_| QPcsErrorV1::ResourceCeilingExceeded)?;
    for ordinal in 0..query_count {
        let mut accepted = None;
        for attempt in 0_u32..256 {
            let mut frame = Vec::with_capacity(QUERY_DOMAIN_V1.len() + 48);
            frame.extend_from_slice(QUERY_DOMAIN_V1);
            frame.push(PCS_VERSION_V1);
            frame.extend_from_slice(&transcript_digest);
            frame.extend_from_slice(
                &u16::try_from(ordinal)
                    .map_err(|_| QPcsErrorV1::InvalidGeometry)?
                    .to_be_bytes(),
            );
            frame.extend_from_slice(&attempt.to_be_bytes());
            let uniform: [u8; 8] = shake256(&frame, 8)
                .try_into()
                .expect("fixed query challenge length");
            let candidate = u64::from_be_bytes(uniform);
            if candidate < zone {
                let position = (candidate % bound as u64) as usize;
                if !positions.contains(&position) {
                    accepted = Some(position);
                    break;
                }
            }
        }
        positions.push(accepted.ok_or(QPcsErrorV1::InvalidChallenge)?);
    }
    Ok(positions)
}

fn fri_alphas_v1(
    transcript_digest: [u8; 32],
    moduli: &[u64],
    layer: usize,
) -> Result<Vec<[Fq2V1; BATCH_ROWS_V1]>, QPcsErrorV1> {
    let mut alphas = Vec::new();
    alphas
        .try_reserve_exact(moduli.len())
        .map_err(|_| QPcsErrorV1::ResourceCeilingExceeded)?;
    for (limb, modulus) in moduli.iter().copied().enumerate() {
        alphas.push([
            derive_field_challenge_v1(FOLD_DOMAIN_V1, transcript_digest, limb, modulus, 0, layer)?,
            derive_field_challenge_v1(FOLD_DOMAIN_V1, transcript_digest, limb, modulus, 1, layer)?,
        ]);
    }
    Ok(alphas)
}

fn prove_cross_limb_fri_in_memory_v1(
    parameter_digest: [u8; 32],
    seed_digest: [u8; 32],
    initial: CrossLimbLayerV1,
    parameters: &[Fq2ParametersV1],
    query_count: usize,
) -> Result<CrossLimbFriProofV1, QPcsErrorV1> {
    let initial_length = initial.validate()?;
    let initial_value_bytes = initial_length
        .checked_mul(initial.coordinate_count())
        .and_then(|value| value.checked_mul(FQ2_WIRE_BYTES_V1))
        .ok_or(QPcsErrorV1::ResourceCeilingExceeded)?;
    let retained_layer_bytes = initial_value_bytes
        .checked_mul(2)
        .ok_or(QPcsErrorV1::ResourceCeilingExceeded)?;
    let maximum_tree_bytes = initial_length
        .checked_mul(2 * HASH_BYTES_V1)
        .ok_or(QPcsErrorV1::ResourceCeilingExceeded)?;
    let conservative_opening_bytes = query_count
        .checked_mul(2)
        .and_then(|value| value.checked_mul(initial.coordinate_count() * FQ2_WIRE_BYTES_V1))
        .and_then(|value| value.checked_mul(initial_length.ilog2() as usize))
        .ok_or(QPcsErrorV1::ResourceCeilingExceeded)?;
    let tiny_profile_heap_bound = retained_layer_bytes
        .checked_add(maximum_tree_bytes)
        .and_then(|value| value.checked_add(conservative_opening_bytes))
        .ok_or(QPcsErrorV1::ResourceCeilingExceeded)?;
    // This reference prover intentionally retains every folded layer so tests
    // can exercise all equations.  It is never the release topology.
    if initial.rows_per_limb != BATCH_ROWS_V1
        || parameters.len() != initial.moduli.len()
        || tiny_profile_heap_bound >= RESIDENT_CAP_BYTES_V1 / 2
    {
        return Err(QPcsErrorV1::ExternalStoreRequired);
    }
    let rounds = initial_length.ilog2() as usize - 1;
    let mut layers = Vec::new();
    let mut roots = Vec::new();
    let mut transcript = seed_digest;
    let mut current = initial;
    for layer in 0..rounds {
        let tree = MerkleTreeV1::build(
            parameter_digest,
            MerkleLayerKindV1::FriBatch,
            layer as u8,
            &current,
        )?;
        let root = tree.root();
        roots.push(root);
        transcript = absorb_fri_root_v1(transcript, layer, root);
        let alphas = fri_alphas_v1(transcript, &current.moduli, layer)?;
        layers.push(current);
        let next = fold_cross_limb_layer_v1(
            layers.last().expect("current layer was just retained"),
            parameters,
            &alphas,
        )?;
        if next.validate()? == 2 {
            current = next;
            break;
        }
        current = next;
    }
    if roots.len() != rounds || current.validate()? != 2 {
        return Err(QPcsErrorV1::InvalidFriProof);
    }
    let terminal_values = [current.leaf_values(0)?, current.leaf_values(1)?].concat();
    let coordinates = current.coordinate_count();
    if terminal_values[..coordinates] != terminal_values[coordinates..] {
        return Err(QPcsErrorV1::InvalidFriProof);
    }
    transcript = absorb_fri_terminal_v1(transcript, &terminal_values, &current.moduli)?;
    let mut query_positions =
        derive_common_query_positions_v1(transcript, query_count, initial_length)?;
    let mut layer_openings = Vec::new();
    for (layer, values) in layers.iter().enumerate() {
        let length = values.validate()?;
        let indices = query_pair_indices_v1(&query_positions, length);
        let tree = MerkleTreeV1::build(
            parameter_digest,
            MerkleLayerKindV1::FriBatch,
            layer as u8,
            values,
        )?;
        if tree.root() != roots[layer] {
            return Err(QPcsErrorV1::CommitmentMismatch);
        }
        layer_openings.push(tree.open(values, &indices)?);
        query_positions = query_positions
            .iter()
            .map(|position| position % (length / 2))
            .collect();
    }
    Ok(CrossLimbFriProofV1 {
        layer_roots: roots,
        terminal_values,
        layer_openings,
    })
}

fn verify_cross_limb_fri_v1(
    parameter_digest: [u8; 32],
    seed_digest: [u8; 32],
    moduli: &[u64],
    parameters: &[Fq2ParametersV1],
    initial_length: usize,
    query_count: usize,
    proof: &CrossLimbFriProofV1,
) -> Result<Vec<usize>, QPcsErrorV1> {
    if moduli.is_empty()
        || parameters.len() != moduli.len()
        || proof.layer_roots.len() != initial_length.ilog2() as usize - 1
        || proof.layer_openings.len() != proof.layer_roots.len()
        || proof.terminal_values.len() != moduli.len() * BATCH_ROWS_V1 * 2
    {
        return Err(QPcsErrorV1::InvalidFriProof);
    }
    let coordinates = moduli.len() * BATCH_ROWS_V1;
    if proof.terminal_values[..coordinates] != proof.terminal_values[coordinates..] {
        return Err(QPcsErrorV1::InvalidFriProof);
    }
    let mut transcript = seed_digest;
    let mut all_alphas = Vec::with_capacity(proof.layer_roots.len());
    for (layer, root) in proof.layer_roots.iter().copied().enumerate() {
        transcript = absorb_fri_root_v1(transcript, layer, root);
        all_alphas.push(fri_alphas_v1(transcript, moduli, layer)?);
    }
    transcript = absorb_fri_terminal_v1(transcript, &proof.terminal_values, moduli)?;
    let initial_queries =
        derive_common_query_positions_v1(transcript, query_count, initial_length)?;
    let mut query_positions = initial_queries.clone();
    let mut length = initial_length;
    for layer in 0..proof.layer_roots.len() {
        let indices = query_pair_indices_v1(&query_positions, length);
        verify_merkle_multi_proof_v1(
            proof.layer_roots[layer],
            parameter_digest,
            MerkleLayerKindV1::FriBatch,
            layer as u8,
            length,
            BATCH_ROWS_V1,
            moduli,
            &indices,
            &proof.layer_openings[layer],
        )?;
        let half = length / 2;
        for &position in &query_positions {
            let base = position % half;
            let positive =
                opened_leaf_v1(&proof.layer_openings[layer], &indices, coordinates, base)?;
            let negative = opened_leaf_v1(
                &proof.layer_openings[layer],
                &indices,
                coordinates,
                base + half,
            )?;
            let next_values = if length == 4 {
                let start = base * coordinates;
                proof
                    .terminal_values
                    .get(start..start + coordinates)
                    .ok_or(QPcsErrorV1::InvalidFriProof)?
            } else {
                let next_length = half;
                let next_positions: Vec<usize> =
                    query_positions.iter().map(|value| value % half).collect();
                let next_indices = query_pair_indices_v1(&next_positions, next_length);
                opened_leaf_v1(
                    &proof.layer_openings[layer + 1],
                    &next_indices,
                    coordinates,
                    base,
                )?
            };
            for limb in 0..moduli.len() {
                let field = parameters[limb];
                let root = field.pow(
                    field.domain_root,
                    1_u128 << (u32::from(field.domain_log) - length.ilog2()),
                );
                let x = field.pow(root, base as u128);
                let inverse_two = mod_pow_v1(2, moduli[limb] - 2, moduli[limb]);
                let inverse_two_x = field.scale(field.inverse(x)?, inverse_two);
                for row in 0..BATCH_ROWS_V1 {
                    let coordinate = limb * BATCH_ROWS_V1 + row;
                    let even = field.scale(
                        field.add(positive[coordinate], negative[coordinate]),
                        inverse_two,
                    );
                    let odd = field.mul(
                        field.sub(positive[coordinate], negative[coordinate]),
                        inverse_two_x,
                    );
                    let expected = field.add(even, field.mul(all_alphas[layer][limb][row], odd));
                    if next_values[coordinate] != expected {
                        return Err(QPcsErrorV1::InvalidFriProof);
                    }
                }
            }
        }
        query_positions = query_positions
            .iter()
            .map(|position| position % half)
            .collect();
        length = half;
    }
    Ok(initial_queries)
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct CanonicalLimbPolynomialPairV1 {
    product: Vec<u64>,
    quotient: Vec<u64>,
}

fn validate_polynomial_coefficients_v1(
    coefficients: &[u64],
    modulus: u64,
    role: RelationPolynomialRoleV1,
    geometry: QPcsGeometryV1,
) -> Result<(), QPcsErrorV1> {
    geometry.validate()?;
    if coefficients.is_empty()
        || coefficients.len() > geometry.degree_bound(role) + 1
        || coefficients.iter().any(|value| *value >= modulus)
    {
        return Err(if coefficients.iter().any(|value| *value >= modulus) {
            QPcsErrorV1::NonCanonicalResidue
        } else {
            QPcsErrorV1::InvalidCoefficientCount
        });
    }
    if coefficients.len() > 1 && coefficients.last() == Some(&0) {
        return Err(QPcsErrorV1::NonCanonicalDegree);
    }
    Ok(())
}

fn build_public_layer_v1(
    geometry: QPcsGeometryV1,
    moduli: &[u64],
    parameters: &[Fq2ParametersV1],
    polynomials: &[CanonicalLimbPolynomialPairV1],
) -> Result<CrossLimbLayerV1, QPcsErrorV1> {
    #[cfg(test)]
    record_in_memory_materialization_attempt_v1();
    if moduli.len() != parameters.len() || moduli.len() != polynomials.len() {
        return Err(QPcsErrorV1::InvalidGeometry);
    }
    let mut columns = Vec::new();
    columns
        .try_reserve_exact(moduli.len() * 2)
        .map_err(|_| QPcsErrorV1::ResourceCeilingExceeded)?;
    for limb in 0..moduli.len() {
        validate_polynomial_coefficients_v1(
            &polynomials[limb].product,
            moduli[limb],
            RelationPolynomialRoleV1::Product,
            geometry,
        )?;
        validate_polynomial_coefficients_v1(
            &polynomials[limb].quotient,
            moduli[limb],
            RelationPolynomialRoleV1::NegacyclicQuotient,
            geometry,
        )?;
        columns.push(fft_forward_v1(
            &polynomials[limb].product,
            parameters[limb],
        )?);
        columns.push(fft_forward_v1(
            &polynomials[limb].quotient,
            parameters[limb],
        )?);
    }
    Ok(CrossLimbLayerV1 {
        moduli: moduli.to_vec(),
        rows_per_limb: 2,
        columns,
    })
}

fn validate_opening_quotient_degree_bounds_v1(
    geometry: QPcsGeometryV1,
    polynomials: &[CanonicalLimbPolynomialPairV1],
) -> Result<(), QPcsErrorV1> {
    let product_bound = geometry
        .degree_bound(RelationPolynomialRoleV1::Product)
        .checked_sub(OPENING_REPETITIONS_V1)
        .ok_or(QPcsErrorV1::InvalidGeometry)?;
    let quotient_bound = geometry
        .degree_bound(RelationPolynomialRoleV1::NegacyclicQuotient)
        .checked_sub(OPENING_REPETITIONS_V1)
        .ok_or(QPcsErrorV1::InvalidGeometry)?;
    let maximum_degree = geometry.degree_bound(RelationPolynomialRoleV1::Product);
    // Exact degree alignment for both random FRI rows:
    // P + X^N H + X^5 QP + X^(N+5) QH has degree at most 2N-2.
    if geometry
        .degree_bound(RelationPolynomialRoleV1::NegacyclicQuotient)
        .checked_add(geometry.ring_degree)
        != Some(maximum_degree)
        || product_bound.checked_add(OPENING_REPETITIONS_V1) != Some(maximum_degree)
        || quotient_bound.checked_add(geometry.ring_degree + OPENING_REPETITIONS_V1)
            != Some(maximum_degree)
    {
        return Err(QPcsErrorV1::InvalidGeometry);
    }
    if polynomials.iter().any(|pair| {
        pair.product.len() > product_bound + 1 || pair.quotient.len() > quotient_bound + 1
    }) {
        return Err(QPcsErrorV1::NonCanonicalDegree);
    }
    Ok(())
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct CrossLimbQPcsCommitmentV1 {
    parameter_digest: [u8; 32],
    ordered_moduli: Vec<u64>,
    public_root: [u8; 32],
}

fn commit_cross_limb_q_polynomials_in_memory_v1(
    geometry: QPcsGeometryV1,
    moduli: &[u64],
    polynomials: &[CanonicalLimbPolynomialPairV1],
) -> Result<CrossLimbQPcsCommitmentV1, QPcsErrorV1> {
    geometry.validate()?;
    preflight_in_memory_reference_v1(geometry, moduli.len(), InMemoryReferenceOperationV1::Commit)?;
    let parameters: Vec<Fq2ParametersV1> = moduli
        .iter()
        .map(|modulus| Fq2ParametersV1::derive(*modulus, geometry.domain_log))
        .collect::<Result<_, _>>()?;
    let parameter_digest = q_pcs_parameter_digest_v1(geometry, moduli)?;
    let public = build_public_layer_v1(geometry, moduli, &parameters, polynomials)?;
    let tree = MerkleTreeV1::build(parameter_digest, MerkleLayerKindV1::Public, 0, &public)?;
    Ok(CrossLimbQPcsCommitmentV1 {
        parameter_digest,
        ordered_moduli: moduli.to_vec(),
        public_root: tree.root(),
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct CrossLimbQPcsOpeningProofV1 {
    evaluations: Vec<[[u64; 2]; OPENING_REPETITIONS_V1]>,
    opening_quotient_root: [u8; 32],
    public_opening: MerkleMultiProofV1,
    opening_quotient_opening: MerkleMultiProofV1,
    fri: CrossLimbFriProofV1,
}

impl CrossLimbQPcsOpeningProofV1 {
    fn encoded_len(&self) -> Result<usize, QPcsErrorV1> {
        let evaluations = self
            .evaluations
            .len()
            .checked_mul(OPENING_REPETITIONS_V1 * 2 * 8)
            .ok_or(QPcsErrorV1::ResourceCeilingExceeded)?;
        48_usize
            .checked_add(evaluations)
            .and_then(|value| value.checked_add(self.public_opening.encoded_len().ok()?))
            .and_then(|value| value.checked_add(self.opening_quotient_opening.encoded_len().ok()?))
            .and_then(|value| value.checked_add(self.fri.encoded_len().ok()?))
            .ok_or(QPcsErrorV1::ResourceCeilingExceeded)
    }
}

fn validate_proof_evaluations_v1(
    moduli: &[u64],
    evaluations: &[[[u64; 2]; OPENING_REPETITIONS_V1]],
) -> Result<(), QPcsErrorV1> {
    if evaluations.len() != moduli.len() {
        return Err(QPcsErrorV1::InvalidGeometry);
    }
    for (limb, limb_evaluations) in evaluations.iter().enumerate() {
        if limb_evaluations
            .iter()
            .flatten()
            .any(|evaluation| *evaluation >= moduli[limb])
        {
            return Err(QPcsErrorV1::NonCanonicalResidue);
        }
    }
    Ok(())
}

fn opening_seed_digest_v1(
    commitment: &CrossLimbQPcsCommitmentV1,
    opening_quotient_root: [u8; 32],
    challenges: &[[QPcsChallengeTupleV1; OPENING_REPETITIONS_V1]],
    evaluations: &[[[u64; 2]; OPENING_REPETITIONS_V1]],
) -> Result<[u8; 32], QPcsErrorV1> {
    if challenges.len() != commitment.ordered_moduli.len()
        || evaluations.len() != commitment.ordered_moduli.len()
    {
        return Err(QPcsErrorV1::InvalidGeometry);
    }
    validate_proof_evaluations_v1(&commitment.ordered_moduli, evaluations)?;
    validate_cross_limb_challenges_v1(&commitment.ordered_moduli, challenges)?;
    let mut frame =
        Vec::with_capacity(CHALLENGE_DOMAIN_V1.len() + 100 + commitment.ordered_moduli.len() * 160);
    frame.extend_from_slice(CHALLENGE_DOMAIN_V1);
    frame.push(PCS_VERSION_V1);
    frame.extend_from_slice(&commitment.parameter_digest);
    frame.extend_from_slice(&commitment.public_root);
    frame.extend_from_slice(&opening_quotient_root);
    for limb in 0..commitment.ordered_moduli.len() {
        let modulus = commitment.ordered_moduli[limb];
        validate_challenge_tuples_v1(modulus, &challenges[limb])?;
        frame.push(u8::try_from(limb).map_err(|_| QPcsErrorV1::InvalidGeometry)?);
        frame.extend_from_slice(&modulus.to_be_bytes());
        for repetition in 0..OPENING_REPETITIONS_V1 {
            let challenge = challenges[limb][repetition];
            frame.push(u8::try_from(repetition).map_err(|_| QPcsErrorV1::InvalidGeometry)?);
            frame.extend_from_slice(&challenge.r.to_be_bytes());
            frame.extend_from_slice(&challenge.gamma.to_be_bytes());
            frame.extend_from_slice(&challenge.beta.to_be_bytes());
            frame.extend_from_slice(&evaluations[limb][repetition][0].to_be_bytes());
            frame.extend_from_slice(&evaluations[limb][repetition][1].to_be_bytes());
        }
    }
    Ok(keccak256(&frame))
}

fn derive_batch_coefficients_v1(
    seed_digest: [u8; 32],
    moduli: &[u64],
) -> Result<Vec<[[Fq2V1; 4]; BATCH_ROWS_V1]>, QPcsErrorV1> {
    let mut coefficients = Vec::new();
    coefficients
        .try_reserve_exact(moduli.len())
        .map_err(|_| QPcsErrorV1::ResourceCeilingExceeded)?;
    for (limb, modulus) in moduli.iter().copied().enumerate() {
        let mut rows = [[Fq2V1::ZERO; 4]; BATCH_ROWS_V1];
        for (row, row_coefficients) in rows.iter_mut().enumerate() {
            for (component, coefficient) in row_coefficients.iter_mut().enumerate() {
                *coefficient = derive_field_challenge_v1(
                    BATCH_DOMAIN_V1,
                    seed_digest,
                    limb,
                    modulus,
                    row,
                    component,
                )?;
            }
        }
        coefficients.push(rows);
    }
    Ok(coefficients)
}

fn build_batch_layer_v1(
    geometry: QPcsGeometryV1,
    parameters: &[Fq2ParametersV1],
    public: &CrossLimbLayerV1,
    opening_quotients: &CrossLimbLayerV1,
    batch_coefficients: &[[[Fq2V1; 4]; BATCH_ROWS_V1]],
) -> Result<CrossLimbLayerV1, QPcsErrorV1> {
    let length = public.validate()?;
    if opening_quotients.validate()? != length
        || public.rows_per_limb != 2
        || opening_quotients.rows_per_limb != 2
        || public.moduli != opening_quotients.moduli
        || parameters.len() != public.moduli.len()
        || batch_coefficients.len() != public.moduli.len()
    {
        return Err(QPcsErrorV1::InvalidGeometry);
    }
    let mut columns = vec![vec![Fq2V1::ZERO; length]; public.moduli.len() * BATCH_ROWS_V1];
    for limb in 0..public.moduli.len() {
        let field = parameters[limb];
        let mut x = Fq2V1::ONE;
        for index in 0..length {
            let x_to_n = field.pow(x, geometry.ring_degree as u128);
            let x_to_five = field.pow(x, OPENING_REPETITIONS_V1 as u128);
            let x_to_n_plus_five = field.mul(x_to_n, x_to_five);
            let components = [
                public.columns[2 * limb][index],
                field.mul(x_to_n, public.columns[2 * limb + 1][index]),
                field.mul(x_to_five, opening_quotients.columns[2 * limb][index]),
                field.mul(
                    x_to_n_plus_five,
                    opening_quotients.columns[2 * limb + 1][index],
                ),
            ];
            for row in 0..BATCH_ROWS_V1 {
                let mut combined = Fq2V1::ZERO;
                for component in 0..4 {
                    combined = field.add(
                        combined,
                        field.mul(
                            batch_coefficients[limb][row][component],
                            components[component],
                        ),
                    );
                }
                columns[limb * BATCH_ROWS_V1 + row][index] = combined;
            }
            x = field.mul(x, field.domain_root);
        }
    }
    Ok(CrossLimbLayerV1 {
        moduli: public.moduli.clone(),
        rows_per_limb: BATCH_ROWS_V1,
        columns,
    })
}

fn prove_cross_limb_q_pcs_openings_in_memory_v1(
    geometry: QPcsGeometryV1,
    commitment: &CrossLimbQPcsCommitmentV1,
    polynomials: &[CanonicalLimbPolynomialPairV1],
    challenges: &[[QPcsChallengeTupleV1; OPENING_REPETITIONS_V1]],
) -> Result<CrossLimbQPcsOpeningProofV1, QPcsErrorV1> {
    geometry.validate()?;
    preflight_in_memory_reference_v1(
        geometry,
        commitment.ordered_moduli.len(),
        InMemoryReferenceOperationV1::Open,
    )?;
    if commitment.parameter_digest
        != q_pcs_parameter_digest_v1(geometry, &commitment.ordered_moduli)?
        || polynomials.len() != commitment.ordered_moduli.len()
        || challenges.len() != commitment.ordered_moduli.len()
    {
        return Err(QPcsErrorV1::CommitmentMismatch);
    }
    let parameters: Vec<Fq2ParametersV1> = commitment
        .ordered_moduli
        .iter()
        .map(|modulus| Fq2ParametersV1::derive(*modulus, geometry.domain_log))
        .collect::<Result<_, _>>()?;
    let public = build_public_layer_v1(
        geometry,
        &commitment.ordered_moduli,
        &parameters,
        polynomials,
    )?;
    let public_tree = MerkleTreeV1::build(
        commitment.parameter_digest,
        MerkleLayerKindV1::Public,
        0,
        &public,
    )?;
    if public_tree.root() != commitment.public_root {
        return Err(QPcsErrorV1::CommitmentMismatch);
    }

    let mut evaluations = Vec::new();
    let mut opening_pairs = Vec::new();
    for limb in 0..commitment.ordered_moduli.len() {
        let modulus = commitment.ordered_moduli[limb];
        validate_challenge_tuples_v1(modulus, &challenges[limb])?;
        let mut limb_evaluations = [[0_u64; 2]; OPENING_REPETITIONS_V1];
        for repetition in 0..OPENING_REPETITIONS_V1 {
            limb_evaluations[repetition][0] = evaluate_base_polynomial_v1(
                &polynomials[limb].product,
                challenges[limb][repetition].r,
                modulus,
            );
            limb_evaluations[repetition][1] = evaluate_base_polynomial_v1(
                &polynomials[limb].quotient,
                challenges[limb][repetition].r,
                modulus,
            );
        }
        let product_values = limb_evaluations.map(|values| values[0]);
        let quotient_values = limb_evaluations.map(|values| values[1]);
        opening_pairs.push(CanonicalLimbPolynomialPairV1 {
            product: five_point_opening_quotient_v1(
                &polynomials[limb].product,
                modulus,
                &challenges[limb],
                &product_values,
            )?,
            quotient: five_point_opening_quotient_v1(
                &polynomials[limb].quotient,
                modulus,
                &challenges[limb],
                &quotient_values,
            )?,
        });
        evaluations.push(limb_evaluations);
    }
    validate_opening_quotient_degree_bounds_v1(geometry, &opening_pairs)?;
    let opening_quotients = build_public_layer_v1(
        geometry,
        &commitment.ordered_moduli,
        &parameters,
        &opening_pairs,
    )?;
    let opening_quotient_tree = MerkleTreeV1::build(
        commitment.parameter_digest,
        MerkleLayerKindV1::OpeningQuotient,
        0,
        &opening_quotients,
    )?;
    let opening_quotient_root = opening_quotient_tree.root();
    let seed = opening_seed_digest_v1(commitment, opening_quotient_root, challenges, &evaluations)?;
    let batch_coefficients = derive_batch_coefficients_v1(seed, &commitment.ordered_moduli)?;
    let batch = build_batch_layer_v1(
        geometry,
        &parameters,
        &public,
        &opening_quotients,
        &batch_coefficients,
    )?;
    let fri = prove_cross_limb_fri_in_memory_v1(
        commitment.parameter_digest,
        seed,
        batch,
        &parameters,
        geometry.query_count,
    )?;
    let queries = verify_cross_limb_fri_v1(
        commitment.parameter_digest,
        seed,
        &commitment.ordered_moduli,
        &parameters,
        geometry.domain_size()?,
        geometry.query_count,
        &fri,
    )?;
    let initial_indices = query_pair_indices_v1(&queries, geometry.domain_size()?);
    Ok(CrossLimbQPcsOpeningProofV1 {
        evaluations,
        opening_quotient_root,
        public_opening: public_tree.open(&public, &initial_indices)?,
        opening_quotient_opening: opening_quotient_tree
            .open(&opening_quotients, &initial_indices)?,
        fri,
    })
}

fn evaluate_base_coefficients_in_fq2_v1(
    coefficients: &[u64],
    point: Fq2V1,
    field: Fq2ParametersV1,
) -> Fq2V1 {
    coefficients
        .iter()
        .rev()
        .fold(Fq2V1::ZERO, |accumulator, coefficient| {
            field.add(field.mul(accumulator, point), Fq2V1::base(*coefficient))
        })
}

fn verify_cross_limb_q_pcs_openings_v1(
    geometry: QPcsGeometryV1,
    commitment: &CrossLimbQPcsCommitmentV1,
    challenges: &[[QPcsChallengeTupleV1; OPENING_REPETITIONS_V1]],
    proof: &CrossLimbQPcsOpeningProofV1,
) -> Result<(), QPcsErrorV1> {
    geometry.validate()?;
    if commitment.parameter_digest
        != q_pcs_parameter_digest_v1(geometry, &commitment.ordered_moduli)?
        || proof.evaluations.len() != commitment.ordered_moduli.len()
        || challenges.len() != commitment.ordered_moduli.len()
    {
        return Err(QPcsErrorV1::CommitmentMismatch);
    }
    let parameters: Vec<Fq2ParametersV1> = commitment
        .ordered_moduli
        .iter()
        .map(|modulus| Fq2ParametersV1::derive(*modulus, geometry.domain_log))
        .collect::<Result<_, _>>()?;
    let seed = opening_seed_digest_v1(
        commitment,
        proof.opening_quotient_root,
        challenges,
        &proof.evaluations,
    )?;
    let queries = verify_cross_limb_fri_v1(
        commitment.parameter_digest,
        seed,
        &commitment.ordered_moduli,
        &parameters,
        geometry.domain_size()?,
        geometry.query_count,
        &proof.fri,
    )?;
    let initial_indices = query_pair_indices_v1(&queries, geometry.domain_size()?);
    verify_merkle_multi_proof_v1(
        commitment.public_root,
        commitment.parameter_digest,
        MerkleLayerKindV1::Public,
        0,
        geometry.domain_size()?,
        2,
        &commitment.ordered_moduli,
        &initial_indices,
        &proof.public_opening,
    )?;
    verify_merkle_multi_proof_v1(
        proof.opening_quotient_root,
        commitment.parameter_digest,
        MerkleLayerKindV1::OpeningQuotient,
        0,
        geometry.domain_size()?,
        2,
        &commitment.ordered_moduli,
        &initial_indices,
        &proof.opening_quotient_opening,
    )?;
    let batch_coefficients = derive_batch_coefficients_v1(seed, &commitment.ordered_moduli)?;
    let batch_opening = proof
        .fri
        .layer_openings
        .first()
        .ok_or(QPcsErrorV1::InvalidFriProof)?;
    let batch_indices = initial_indices.clone();
    let coordinate_count = commitment.ordered_moduli.len() * 2;
    for &index in &initial_indices {
        let public = opened_leaf_v1(
            &proof.public_opening,
            &initial_indices,
            coordinate_count,
            index,
        )?;
        let opening_quotients = opened_leaf_v1(
            &proof.opening_quotient_opening,
            &initial_indices,
            coordinate_count,
            index,
        )?;
        let batch = opened_leaf_v1(batch_opening, &batch_indices, coordinate_count, index)?;
        for limb in 0..commitment.ordered_moduli.len() {
            let field = parameters[limb];
            let x = field.pow(field.domain_root, index as u128);
            let product_values = proof.evaluations[limb].map(|values| values[0]);
            let quotient_values = proof.evaluations[limb].map(|values| values[1]);
            let product_interpolation = interpolate_openings_v1(
                commitment.ordered_moduli[limb],
                &challenges[limb],
                &product_values,
            )?;
            let quotient_interpolation = interpolate_openings_v1(
                commitment.ordered_moduli[limb],
                &challenges[limb],
                &quotient_values,
            )?;
            let vanishing =
                opening_vanishing_polynomial_v1(commitment.ordered_moduli[limb], &challenges[limb]);
            let z_at_x = evaluate_base_coefficients_in_fq2_v1(&vanishing, x, field);
            let product_i_at_x =
                evaluate_base_coefficients_in_fq2_v1(&product_interpolation, x, field);
            let quotient_i_at_x =
                evaluate_base_coefficients_in_fq2_v1(&quotient_interpolation, x, field);
            if field.sub(public[2 * limb], product_i_at_x)
                != field.mul(z_at_x, opening_quotients[2 * limb])
                || field.sub(public[2 * limb + 1], quotient_i_at_x)
                    != field.mul(z_at_x, opening_quotients[2 * limb + 1])
            {
                return Err(QPcsErrorV1::OpeningMismatch);
            }
            let x_to_n = field.pow(x, geometry.ring_degree as u128);
            let x_to_five = field.pow(x, OPENING_REPETITIONS_V1 as u128);
            let components = [
                public[2 * limb],
                field.mul(x_to_n, public[2 * limb + 1]),
                field.mul(x_to_five, opening_quotients[2 * limb]),
                field.mul(
                    field.mul(x_to_n, x_to_five),
                    opening_quotients[2 * limb + 1],
                ),
            ];
            for row in 0..BATCH_ROWS_V1 {
                let mut expected = Fq2V1::ZERO;
                for component in 0..4 {
                    expected = field.add(
                        expected,
                        field.mul(
                            batch_coefficients[limb][row][component],
                            components[component],
                        ),
                    );
                }
                if batch[limb * BATCH_ROWS_V1 + row] != expected {
                    return Err(QPcsErrorV1::InvalidFriProof);
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_MODULI: [u64; 2] = [97, 193];

    struct ChunkSourceV1 {
        declared_coefficients: usize,
        chunks: Vec<Vec<u8>>,
    }

    impl ChunkSourceV1 {
        fn from_coefficients(coefficients: &[u64]) -> Self {
            let encoded: Vec<u8> = coefficients
                .iter()
                .flat_map(|value| value.to_be_bytes())
                .collect();
            Self {
                declared_coefficients: coefficients.len(),
                chunks: encoded.chunks(8 * 1_024).map(<[u8]>::to_vec).collect(),
            }
        }
    }

    impl CanonicalQPolynomialChunkSourceV1 for ChunkSourceV1 {
        fn coefficient_count(&self) -> usize {
            self.declared_coefficients
        }

        fn read_chunk(
            &mut self,
            chunk_index: usize,
            destination: &mut [u8; 8 * 1_024],
        ) -> Result<usize, QPcsErrorV1> {
            let Some(chunk) = self.chunks.get(chunk_index) else {
                return Ok(0);
            };
            destination[..chunk.len()].copy_from_slice(chunk);
            Ok(chunk.len())
        }
    }

    fn test_geometry() -> QPcsGeometryV1 {
        QPcsGeometryV1 {
            ring_degree: 8,
            domain_log: 5,
            query_count: 4,
        }
    }

    fn test_polynomials() -> Vec<CanonicalLimbPolynomialPairV1> {
        TEST_MODULI
            .iter()
            .enumerate()
            .map(|(limb, modulus)| CanonicalLimbPolynomialPairV1 {
                product: (0..15)
                    .map(|index| (3 * index as u64 + 1 + limb as u64) % modulus)
                    .collect(),
                quotient: (0..7)
                    .map(|index| (5 * index as u64 + 2 + limb as u64) % modulus)
                    .collect(),
            })
            .collect()
    }

    fn test_challenges() -> Vec<[QPcsChallengeTupleV1; OPENING_REPETITIONS_V1]> {
        vec![
            [
                QPcsChallengeTupleV1 {
                    r: 1,
                    gamma: 2,
                    beta: 3,
                },
                QPcsChallengeTupleV1 {
                    r: 4,
                    gamma: 5,
                    beta: 6,
                },
                QPcsChallengeTupleV1 {
                    r: 7,
                    gamma: 8,
                    beta: 9,
                },
                QPcsChallengeTupleV1 {
                    r: 10,
                    gamma: 11,
                    beta: 12,
                },
                QPcsChallengeTupleV1 {
                    r: 13,
                    gamma: 14,
                    beta: 15,
                },
            ],
            [
                QPcsChallengeTupleV1 {
                    r: 16,
                    gamma: 17,
                    beta: 18,
                },
                QPcsChallengeTupleV1 {
                    r: 19,
                    gamma: 20,
                    beta: 21,
                },
                QPcsChallengeTupleV1 {
                    r: 22,
                    gamma: 23,
                    beta: 24,
                },
                QPcsChallengeTupleV1 {
                    r: 25,
                    gamma: 26,
                    beta: 27,
                },
                QPcsChallengeTupleV1 {
                    r: 28,
                    gamma: 29,
                    beta: 30,
                },
            ],
        ]
    }

    fn maximum_authentication_nodes(tree_length: usize, opened_leaves: usize) -> usize {
        let mut length = tree_length;
        let mut occupied = opened_leaves;
        let mut authentication = 0_usize;
        while length > 1 {
            let parents = occupied.min(length / 2);
            authentication += 2 * parents - occupied;
            occupied = parents;
            length /= 2;
        }
        authentication
    }

    #[test]
    fn release_accounting_is_exact_and_fail_closed() {
        let plan = zk_ams_phase23_rns_link_q_pcs_release_plan_v1().unwrap();
        assert_eq!(plan.minimum_base_two_adicity, 18);
        assert_eq!(plan.minimum_extension_two_adicity, 19);
        assert_eq!(
            plan.release_parameter_digest,
            q_pcs_parameter_digest_v1(
                QPcsGeometryV1 {
                    ring_degree: ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1,
                    domain_log: RELEASE_DOMAIN_LOG_V1,
                    query_count: RELEASE_FRI_QUERY_COUNT_V1,
                },
                &RELEASE_MODULI_V1,
            )
            .unwrap()
        );
        assert_eq!(plan.domain_size, 1 << 19);
        assert_eq!(plan.maximum_polynomial_degree, 2 * (1 << 17) - 2);
        assert_eq!(plan.maximum_relation_quotient_degree, (1 << 17) - 2);
        assert_eq!(plan.common_query_count, 160);
        assert_eq!(plan.cross_limb_leaf_bytes, 1_216);

        let initial_opened = 2 * RELEASE_FRI_QUERY_COUNT_V1;
        let initial_auth = maximum_authentication_nodes(RELEASE_DOMAIN_SIZE_V1, initial_opened);
        assert_eq!(initial_auth, RELEASE_INITIAL_MULTIPROOF_AUTH_HASHES_V1);
        let mut fri_auth = 0_usize;
        let mut fri_values = 0_usize;
        for log in (2..=RELEASE_DOMAIN_LOG_V1).rev() {
            let length = 1 << log;
            let opened = initial_opened.min(length);
            fri_auth += maximum_authentication_nodes(length, opened);
            fri_values += opened;
        }
        assert_eq!(fri_auth, RELEASE_FRI_MULTIPROOF_AUTH_HASHES_V1);
        assert_eq!(fri_values, RELEASE_FRI_OPENED_LEAVES_V1);
        assert_eq!(
            (2 * initial_opened + fri_values) * CROSS_LIMB_LEAF_BYTES_V1,
            RELEASE_MULTIPROOF_VALUE_BYTES_V1
        );
        assert_eq!(
            (2 * initial_auth + fri_auth) * HASH_BYTES_V1,
            RELEASE_MULTIPROOF_AUTH_BYTES_V1
        );
        assert_eq!(
            RELEASE_MULTIPROOF_VALUE_BYTES_V1
                + RELEASE_MULTIPROOF_AUTH_BYTES_V1
                + RELEASE_PROOF_FIXED_BYTES_V1,
            RELEASE_MAX_ENCODED_PROOF_BYTES_V1
        );
        assert_eq!(plan.maximum_encoded_proof_bytes, 6_530_912);
        assert_eq!(plan.remaining_global_proof_budget_bytes, 27_023_520);
        assert_eq!(plan.fft_butterflies, 2_271_215_616);
        assert_eq!(plan.merkle_hash_invocations, 8_388_552);
        assert_eq!(plan.fri_folded_row_values, 79_691_472);
        assert_eq!(plan.classified_work_units, 2_359_295_640);
        assert_eq!(plan.fri_query_union_bound_bits_floor, 154);
        assert_eq!(plan.enumerated_heap_bytes, 46_378_544);
        assert!(plan.enumerated_heap_bytes < RESIDENT_CAP_BYTES_V1 as u64);
        assert_eq!(plan.external_scratch_bytes, 956_301_312);
        assert!(!plan.cross_limb_vector_merkle_implemented);
        assert!(!plan.seekable_external_store_implemented);
        assert!(!plan.exact_fri_theorem_instantiated);
        assert!(!plan.fiat_shamir_relation_adapter_implemented);
        assert!(!plan.release_kat_matches);
        assert!(!plan.measured_resident_set_within_cap);
        assert!(!plan.production_wire_integrated);
        assert!(!plan.non_pcs_sections_measured_within_remaining_budget);
        assert!(!plan.release_qualified);
        assert_eq!(
            require_zk_ams_phase23_rns_link_q_pcs_release_prover_v1(),
            Err(QPcsErrorV1::ExternalStoreRequired)
        );
    }

    #[test]
    fn every_release_prime_has_the_required_extension_root() {
        assert_eq!(
            Fq2ParametersV1::derive(1_649, 5),
            Err(QPcsErrorV1::InvalidModulus)
        );
        for modulus in RELEASE_MODULI_V1 {
            let field = Fq2ParametersV1::derive(modulus, RELEASE_DOMAIN_LOG_V1).unwrap();
            assert_eq!(
                field.pow(field.domain_root, 1 << RELEASE_DOMAIN_LOG_V1),
                Fq2V1::ONE
            );
            assert_ne!(
                field.pow(field.domain_root, 1 << (RELEASE_DOMAIN_LOG_V1 - 1)),
                Fq2V1::ONE
            );
        }
    }

    #[test]
    fn transcript_dimensions_and_modulus_order_have_no_aliases() {
        let geometry = QPcsGeometryV1 {
            ring_degree: 65_536,
            domain_log: 18,
            query_count: RELEASE_FRI_QUERY_COUNT_V1,
        };
        assert!(q_pcs_parameter_digest_v1(geometry, &[RELEASE_MODULI_V1[0]]).is_ok());
        let aliased_query_geometry = QPcsGeometryV1 {
            query_count: RELEASE_FRI_QUERY_COUNT_V1 + (1 << 16),
            ..geometry
        };
        assert_eq!(
            q_pcs_parameter_digest_v1(aliased_query_geometry, &[RELEASE_MODULI_V1[0]],),
            Err(QPcsErrorV1::InvalidGeometry)
        );
        assert_eq!(
            derive_common_query_positions_v1(
                [0; 32],
                RELEASE_FRI_QUERY_COUNT_V1 + (1 << 16),
                geometry.domain_size().unwrap(),
            ),
            Err(QPcsErrorV1::InvalidGeometry)
        );
        assert_eq!(
            q_pcs_parameter_digest_v1(test_geometry(), &[TEST_MODULI[0], TEST_MODULI[0]]),
            Err(QPcsErrorV1::InvalidModulus)
        );
        if usize::BITS > 32 {
            assert_eq!(
                QPcsGeometryV1 {
                    ring_degree: 1_usize.checked_shl(32).expect("64-bit branch"),
                    domain_log: 34,
                    query_count: RELEASE_FRI_QUERY_COUNT_V1,
                }
                .validate(),
                Err(QPcsErrorV1::InvalidGeometry)
            );
        }
    }

    #[test]
    fn cumulative_residency_fails_before_materialization() {
        let geometry = QPcsGeometryV1 {
            ring_degree: 65_536,
            domain_log: 18,
            query_count: RELEASE_FRI_QUERY_COUNT_V1,
        };
        let limb_count = 4;
        let single_layer_bytes =
            geometry.domain_size().unwrap() * limb_count * BATCH_ROWS_V1 * FQ2_WIRE_BYTES_V1;
        assert!(single_layer_bytes < RESIDENT_CAP_BYTES_V1);
        assert!(
            in_memory_reference_residency_bytes_v1(
                geometry,
                limb_count,
                InMemoryReferenceOperationV1::Commit,
            )
            .unwrap()
                < RESIDENT_CAP_BYTES_V1
        );
        assert!(
            in_memory_reference_residency_bytes_v1(
                geometry,
                limb_count,
                InMemoryReferenceOperationV1::Open,
            )
            .unwrap()
                >= RESIDENT_CAP_BYTES_V1
        );

        let commitment = CrossLimbQPcsCommitmentV1 {
            parameter_digest: [0; 32],
            ordered_moduli: vec![TEST_MODULI[0]; limb_count],
            public_root: [0; 32],
        };
        let polynomials: Vec<CanonicalLimbPolynomialPairV1> = Vec::new();
        let challenges: Vec<[QPcsChallengeTupleV1; OPENING_REPETITIONS_V1]> = Vec::new();
        reset_in_memory_materialization_attempts_v1();
        assert_eq!(
            prove_cross_limb_q_pcs_openings_in_memory_v1(
                geometry,
                &commitment,
                &polynomials,
                &challenges,
            ),
            Err(QPcsErrorV1::ExternalStoreRequired)
        );
        assert_eq!(in_memory_materialization_attempts_v1(), 0);
    }

    #[test]
    fn genuine_cross_limb_fri_opening_round_trip_rejects_tampering() {
        let geometry = test_geometry();
        let polynomials = test_polynomials();
        let challenges = test_challenges();
        let commitment =
            commit_cross_limb_q_polynomials_in_memory_v1(geometry, &TEST_MODULI, &polynomials)
                .unwrap();
        let proof = prove_cross_limb_q_pcs_openings_in_memory_v1(
            geometry,
            &commitment,
            &polynomials,
            &challenges,
        )
        .unwrap();
        verify_cross_limb_q_pcs_openings_v1(geometry, &commitment, &challenges, &proof).unwrap();
        assert!(proof.encoded_len().unwrap() < PROOF_CAP_BYTES_V1);
        let opening_seed = opening_seed_digest_v1(
            &commitment,
            proof.opening_quotient_root,
            &challenges,
            &proof.evaluations,
        )
        .unwrap();
        let two_rows = derive_batch_coefficients_v1(opening_seed, &TEST_MODULI).unwrap();
        assert_eq!(BATCH_ROWS_V1, 2);
        assert_ne!(two_rows[0][0], two_rows[0][1]);
        let field = Fq2ParametersV1::derive(TEST_MODULI[0], geometry.domain_log).unwrap();
        let first_row = two_rows[0][0];
        let second_row = two_rows[0][1];
        let mut cancellation_witness = None;
        for left in 0..4 {
            for right in left + 1..4 {
                // These are coefficients of one out-of-bound degree across
                // two aligned component polynomials.  They cancel in row zero.
                let left_error = first_row[right];
                let right_error = field.sub(Fq2V1::ZERO, first_row[left]);
                let row_zero = field.add(
                    field.mul(first_row[left], left_error),
                    field.mul(first_row[right], right_error),
                );
                let row_one = field.add(
                    field.mul(second_row[left], left_error),
                    field.mul(second_row[right], right_error),
                );
                assert_eq!(row_zero, Fq2V1::ZERO);
                if row_one != Fq2V1::ZERO {
                    cancellation_witness = Some((left, right, row_one));
                    break;
                }
            }
            if cancellation_witness.is_some() {
                break;
            }
        }
        assert!(
            cancellation_witness.is_some(),
            "one FRI row admits a high-degree cancellation that the independent second row must reject"
        );

        let mut changed_value = proof.clone();
        changed_value.evaluations[0][0][0] =
            (changed_value.evaluations[0][0][0] + 1) % TEST_MODULI[0];
        assert!(
            verify_cross_limb_q_pcs_openings_v1(geometry, &commitment, &challenges, &changed_value)
                .is_err()
        );

        let mut plus_q_evaluation = proof.clone();
        plus_q_evaluation.evaluations[0][0][0] += TEST_MODULI[0];
        assert_eq!(
            verify_cross_limb_q_pcs_openings_v1(
                geometry,
                &commitment,
                &challenges,
                &plus_q_evaluation,
            ),
            Err(QPcsErrorV1::NonCanonicalResidue)
        );

        let mut changed_authentication = proof.clone();
        changed_authentication.fri.layer_openings[0].authentication_nodes[0][0] ^= 1;
        assert!(
            verify_cross_limb_q_pcs_openings_v1(
                geometry,
                &commitment,
                &challenges,
                &changed_authentication
            )
            .is_err()
        );

        let mut changed_public_value = proof.clone();
        changed_public_value.public_opening.values[0].c0 =
            (changed_public_value.public_opening.values[0].c0 + 1) % TEST_MODULI[0];
        assert!(
            verify_cross_limb_q_pcs_openings_v1(
                geometry,
                &commitment,
                &challenges,
                &changed_public_value
            )
            .is_err()
        );

        let mut changed_commitment_root = commitment.clone();
        changed_commitment_root.public_root[0] ^= 1;
        assert!(
            verify_cross_limb_q_pcs_openings_v1(
                geometry,
                &changed_commitment_root,
                &challenges,
                &proof
            )
            .is_err()
        );

        let mut changed_fri_root = proof.clone();
        changed_fri_root.fri.layer_roots[1][0] ^= 1;
        assert!(
            verify_cross_limb_q_pcs_openings_v1(
                geometry,
                &commitment,
                &challenges,
                &changed_fri_root
            )
            .is_err()
        );

        let mut changed_fold_value = proof.clone();
        changed_fold_value.fri.layer_openings[1].values[0].c0 =
            (changed_fold_value.fri.layer_openings[1].values[0].c0 + 1) % TEST_MODULI[0];
        assert!(
            verify_cross_limb_q_pcs_openings_v1(
                geometry,
                &commitment,
                &challenges,
                &changed_fold_value
            )
            .is_err()
        );

        let mut changed_terminal = proof.clone();
        changed_terminal.fri.terminal_values[0].c0 =
            (changed_terminal.fri.terminal_values[0].c0 + 1) % TEST_MODULI[0];
        assert!(
            verify_cross_limb_q_pcs_openings_v1(
                geometry,
                &commitment,
                &challenges,
                &changed_terminal
            )
            .is_err()
        );

        // Coordinate one is the independently mixed second row for limb zero.
        let mut changed_second_row = proof.clone();
        changed_second_row.fri.layer_openings[0].values[1].c0 =
            (changed_second_row.fri.layer_openings[0].values[1].c0 + 1) % TEST_MODULI[0];
        assert!(
            verify_cross_limb_q_pcs_openings_v1(
                geometry,
                &commitment,
                &challenges,
                &changed_second_row
            )
            .is_err()
        );

        // Gamma is transcript-bound before common query derivation.
        let mut changed_query_seed = challenges.clone();
        changed_query_seed[0][0].gamma = 31;
        assert!(
            verify_cross_limb_q_pcs_openings_v1(geometry, &commitment, &changed_query_seed, &proof)
                .is_err()
        );

        let mut limb_splice = polynomials.clone();
        limb_splice.swap(0, 1);
        let spliced_commitment =
            commit_cross_limb_q_polynomials_in_memory_v1(geometry, &TEST_MODULI, &limb_splice)
                .unwrap();
        assert_ne!(spliced_commitment.public_root, commitment.public_root);
        assert!(
            verify_cross_limb_q_pcs_openings_v1(geometry, &spliced_commitment, &challenges, &proof)
                .is_err()
        );

        let reordered_moduli = [TEST_MODULI[1], TEST_MODULI[0]];
        let mut reordered_polynomials = polynomials.clone();
        reordered_polynomials.swap(0, 1);
        let reordered_commitment = commit_cross_limb_q_polynomials_in_memory_v1(
            geometry,
            &reordered_moduli,
            &reordered_polynomials,
        )
        .unwrap();
        let mut reordered_challenges = challenges.clone();
        reordered_challenges.swap(0, 1);
        assert_ne!(
            reordered_commitment.parameter_digest,
            commitment.parameter_digest
        );
        assert!(
            verify_cross_limb_q_pcs_openings_v1(
                geometry,
                &reordered_commitment,
                &reordered_challenges,
                &proof
            )
            .is_err()
        );

        let substituted_moduli = [TEST_MODULI[0], 257];
        let substituted_commitment = commit_cross_limb_q_polynomials_in_memory_v1(
            geometry,
            &substituted_moduli,
            &polynomials,
        )
        .unwrap();
        assert_ne!(
            substituted_commitment.parameter_digest,
            commitment.parameter_digest
        );
        assert!(
            verify_cross_limb_q_pcs_openings_v1(
                geometry,
                &substituted_commitment,
                &challenges,
                &proof
            )
            .is_err()
        );
    }

    #[test]
    fn canonical_source_rejects_plus_q_noncanonical_and_chunk_splice() {
        let geometry = QPcsGeometryV1 {
            ring_degree: 2_048,
            domain_log: 13,
            query_count: 4,
        };
        let mut noncanonical = ChunkSourceV1::from_coefficients(&[97]);
        assert_eq!(
            read_canonical_polynomial_v1(
                &mut noncanonical,
                97,
                RelationPolynomialRoleV1::Product,
                geometry
            ),
            Err(QPcsErrorV1::NonCanonicalResidue)
        );
        let mut plus_q_alias = ChunkSourceV1::from_coefficients(&[98]);
        assert_eq!(
            read_canonical_polynomial_v1(
                &mut plus_q_alias,
                97,
                RelationPolynomialRoleV1::Product,
                geometry
            ),
            Err(QPcsErrorV1::NonCanonicalResidue)
        );
        assert_eq!(
            Fq2V1::decode(
                [97_u64.to_be_bytes(), 0_u64.to_be_bytes()]
                    .concat()
                    .try_into()
                    .unwrap(),
                97
            ),
            Err(QPcsErrorV1::NonCanonicalResidue)
        );
        let mut short = ChunkSourceV1::from_coefficients(&[1, 2]);
        short.chunks[0].pop();
        assert_eq!(
            read_canonical_polynomial_v1(
                &mut short,
                97,
                RelationPolynomialRoleV1::Product,
                geometry
            ),
            Err(QPcsErrorV1::InvalidChunkLength)
        );
        let mut trailing = ChunkSourceV1::from_coefficients(&[1]);
        trailing.chunks.push(2_u64.to_be_bytes().to_vec());
        assert_eq!(
            read_canonical_polynomial_v1(
                &mut trailing,
                97,
                RelationPolynomialRoleV1::Product,
                geometry
            ),
            Err(QPcsErrorV1::TrailingChunk)
        );

        let coefficients: Vec<u64> = (0..2_048).map(|index| index as u64 % 97).collect();
        let chunk_modulus = RELEASE_MODULI_V1[0];
        let mut canonical = ChunkSourceV1::from_coefficients(&coefficients);
        let baseline = read_canonical_polynomial_v1(
            &mut canonical,
            chunk_modulus,
            RelationPolynomialRoleV1::Product,
            geometry,
        )
        .unwrap();
        let mut spliced = ChunkSourceV1::from_coefficients(&coefficients);
        spliced.chunks.swap(0, 1);
        let changed = read_canonical_polynomial_v1(
            &mut spliced,
            chunk_modulus,
            RelationPolynomialRoleV1::Product,
            geometry,
        )
        .unwrap();
        assert_ne!(baseline, changed);
        let baseline_commitment = commit_cross_limb_q_polynomials_in_memory_v1(
            geometry,
            &[chunk_modulus],
            &[CanonicalLimbPolynomialPairV1 {
                product: baseline,
                quotient: vec![1],
            }],
        )
        .unwrap();
        let changed_commitment = commit_cross_limb_q_polynomials_in_memory_v1(
            geometry,
            &[chunk_modulus],
            &[CanonicalLimbPolynomialPairV1 {
                product: changed,
                quotient: vec![1],
            }],
        )
        .unwrap();
        assert_ne!(
            baseline_commitment.public_root,
            changed_commitment.public_root
        );
    }

    #[test]
    fn degree_n_minus_one_quotient_and_reused_challenges_are_rejected() {
        let geometry = test_geometry();
        let invalid_product = vec![1_u64; 2 * geometry.ring_degree];
        assert_eq!(
            validate_polynomial_coefficients_v1(
                &invalid_product,
                TEST_MODULI[0],
                RelationPolynomialRoleV1::Product,
                geometry,
            ),
            Err(QPcsErrorV1::InvalidCoefficientCount)
        );
        let invalid_quotient = vec![1_u64; geometry.ring_degree];
        assert_eq!(
            validate_polynomial_coefficients_v1(
                &invalid_quotient,
                TEST_MODULI[0],
                RelationPolynomialRoleV1::NegacyclicQuotient,
                geometry,
            ),
            Err(QPcsErrorV1::InvalidCoefficientCount)
        );

        let product_opening_bound =
            geometry.degree_bound(RelationPolynomialRoleV1::Product) - OPENING_REPETITIONS_V1;
        let quotient_opening_bound = geometry
            .degree_bound(RelationPolynomialRoleV1::NegacyclicQuotient)
            - OPENING_REPETITIONS_V1;
        assert_eq!(
            validate_opening_quotient_degree_bounds_v1(
                geometry,
                &[CanonicalLimbPolynomialPairV1 {
                    product: vec![1; product_opening_bound + 2],
                    quotient: vec![1],
                }],
            ),
            Err(QPcsErrorV1::NonCanonicalDegree)
        );
        assert_eq!(
            validate_opening_quotient_degree_bounds_v1(
                geometry,
                &[CanonicalLimbPolynomialPairV1 {
                    product: vec![1],
                    quotient: vec![1; quotient_opening_bound + 2],
                }],
            ),
            Err(QPcsErrorV1::NonCanonicalDegree)
        );

        let mut reused = test_challenges();
        reused[0][1].r = reused[0][0].r;
        assert_eq!(
            validate_cross_limb_challenges_v1(&TEST_MODULI, &reused),
            Err(QPcsErrorV1::ReusedChallenge)
        );
        let mut reused_gamma = test_challenges();
        reused_gamma[0][1].gamma = reused_gamma[0][0].gamma;
        assert_eq!(
            validate_cross_limb_challenges_v1(&TEST_MODULI, &reused_gamma),
            Err(QPcsErrorV1::ReusedChallenge)
        );
        let mut reused_beta = test_challenges();
        reused_beta[0][1].beta = reused_beta[0][0].beta;
        assert_eq!(
            validate_cross_limb_challenges_v1(&TEST_MODULI, &reused_beta),
            Err(QPcsErrorV1::ReusedChallenge)
        );
        let mut cross_limb_reuse = test_challenges();
        cross_limb_reuse[1][0] = cross_limb_reuse[0][0];
        assert_eq!(
            validate_cross_limb_challenges_v1(&TEST_MODULI, &cross_limb_reuse),
            Err(QPcsErrorV1::ReusedChallenge)
        );
    }

    #[test]
    fn release_source_guards_keep_the_prototype_private_and_fail_closed() {
        let source = include_str!("phase23_rns_link_q_pcs.rs");
        let parent = include_str!("phase23_rns_link.rs");
        let audit = include_str!("receipt_capability_audit.rs");
        let manifest = include_str!("manifest.rs");
        assert!(source.contains("const RELEASE_FRI_QUERY_COUNT_V1: usize = 160;"));
        assert!(source.contains("const RELEASE_MAX_ENCODED_PROOF_BYTES_V1: usize = 6_530_912;"));
        assert!(source.contains("const RELEASE_EXTERNAL_SCRATCH_BYTES_V1: usize = 956_301_312;"));
        assert!(source.contains("release_qualified: false"));
        assert!(source.contains("seekable_external_store_implemented: false"));
        assert!(source.contains("fiat_shamir_relation_adapter_implemented: false"));
        assert!(source.contains("validate_proof_evaluations_v1"));
        assert!(source.contains("preflight_in_memory_reference_v1("));
        assert!(parent.contains("#[path = \"phase23_rns_link_q_pcs.rs\"]\nmod q_pcs;"));
        assert!(!parent.contains("pub use q_pcs"));
        assert!(!audit.contains("phase23_rns_link_q_pcs"));
        assert!(!manifest.contains("phase23_rns_link_q_pcs"));
    }
}
