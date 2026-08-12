//! Isolated five-repetition masking and accounting prerequisite for q-PCS.
//!
//! This child is deliberately private and non-authorizing. It borrows five
//! caller-provided `(P_j,H_j)` pairs, samples five full `degree <= N-2` masks
//! into zeroizing owners, and constructs the fixed row order
//! `(P~_0,H~_0),...,(P~_4,H~_4)`, where
//! `H~=H+S` and `P~=P+(X^N+1)S`. It requires both original and masked residuals
//! to be zero, but neither
//! constructs the five source-owned aggregate pairs nor binds them to the
//! Fiat-Shamir transcript. The existing PCS still mixes only two FRI rows and
//! its variable-degree input path is incompatible with fixed-width top zeros;
//! this child is isolated from that path and ten-row wiring remains false.
//!
//! `S` is private sampler entropy, never a Fiat-Shamir/public derivation. The
//! precommit domain binds the sealed source transcript, limb, repetition,
//! `gamma`, and `beta`, but deliberately cannot contain the later DEEP point
//! `r`: all ten rows must be masked and committed before `r` is derived. A
//! private type-state split enforces that local order. The descriptor only
//! separates sampler calls; it cannot prove that a caller-supplied sampler is
//! uniform or uncorrelated. Exact repeated masks are rejected by comparing
//! the retained zeroizing mask owners directly, without retaining a mask
//! digest. No mask, source polynomial, or secret-derived digest is returned.
//! All ten borrowed coefficient ranges must be disjoint; equal-valued slices
//! in independent allocations remain valid and no content provenance follows.
//!
//! The 74,662,064-byte bound below is isolated PCS-kernel accounting only. It
//! excludes the currently borrowed 43-ciphertext set (about 3.2 GiB), packed
//! owner (about 172 MiB), opening/plaintext-lift duplication (about 38 MiB),
//! source adapter, allocator overhead, OS page cache, and RSS evidence. It is
//! therefore not an end-to-end residency claim or release evidence. This
//! retired test prototype's ten-row spool and retained cross-limb aggregates
//! used an independently enumerated 3,785,356,320-byte minimum external peak;
//! it is not the concrete V2 spool layout. The classified work
//! total is only a lower accounting: it omits sampler/rejection, batch mixing,
//! quotient construction, and hash-byte work, and is blocked explicitly below.
//! Zeroization covers named heap coefficient owners on Rust drop paths only;
//! arithmetic/register/compiler temporaries and panic-abort are outside that
//! best-effort guarantee, so no confidentiality or release claim follows.

#![cfg(test)]

use core::fmt;

use crate::vega::sponge::keccak256;

use super::{
    BATCH_ROWS_V1, FQ2_WIRE_BYTES_V1, HASH_BYTES_V1, OPENING_REPETITIONS_V1, PROOF_CAP_BYTES_V1,
    QPcsChallengeTupleV1, QPcsErrorV1, RELEASE_DOMAIN_LOG_V1, RELEASE_DOMAIN_SIZE_V1,
    RELEASE_FRI_QUERY_COUNT_V1, RELEASE_FRI_ROUNDS_V1, RELEASE_LIMBS_V1, RELEASE_LOG_N_V1,
    RESIDENT_CAP_BYTES_V1, ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1, is_prime_u64, mod_add_v1,
    mod_pow_v1, mod_sub_v1, validate_challenge_tuples_v1,
};

const MASKING_VERSION_V1: u8 = 1;
const MASKED_ROWS_PER_REPETITION_V1: usize = 2;
const MASKED_ROW_COUNT_V1: usize = OPENING_REPETITIONS_V1 * MASKED_ROWS_PER_REPETITION_V1;
const MASK_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.phase23.rns-link.q-pcs.mask-domain";
const MASK_PRECOMMIT_ORDER_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.phase23.rns-link.q-pcs.mask-precommit-order";
const MASK_POSTCOMMIT_ORDER_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.phase23.rns-link.q-pcs.mask-postcommit-order";
const RELEASE_MASKED_CROSS_LIMB_LEAF_BYTES_V1: usize =
    RELEASE_LIMBS_V1 * MASKED_ROW_COUNT_V1 * FQ2_WIRE_BYTES_V1;
const RELEASE_FRI_CROSS_LIMB_LEAF_BYTES_V1: usize =
    RELEASE_LIMBS_V1 * BATCH_ROWS_V1 * FQ2_WIRE_BYTES_V1;
const RELEASE_EXTERNAL_IO_BUFFER_BYTES_V1: usize = 8 * 1024 * 1024;
const RELEASE_FIXED_ENVELOPE_HEADER_BYTES_V1: usize = 512;
const RELEASE_COUNT_HEADER_BYTES_V1: usize = 8;
const RELEASE_BASE_FIELD_WIRE_BYTES_V1: usize = 8;

const SOURCE_OWNED_AGGREGATE_PAIRS_LINKED_V1: bool = false;
const FIAT_SHAMIR_RELATION_BOUND_V1: bool = false;
const PRODUCTION_UNIFORM_INDEPENDENT_SAMPLER_INTEGRATED_V1: bool = false;
const SAMPLER_ENTROPY_AND_WORK_ACCOUNTED_V1: bool = false;
const TEN_ROW_PCS_PROOF_INTEGRATED_V1: bool = false;
const TEN_ROW_PCS_WIRING_IMPLEMENTED_V1: bool = false;
const ZERO_KNOWLEDGE_QUALIFIED_V1: bool = false;
const END_TO_END_SOURCE_RESIDENCY_ACCOUNTED_V1: bool = false;
const MASKED_ROW_ROOT_AUTHENTICITY_VERIFIED_V1: bool = false;
const POSTCOMMIT_OPENING_POINTS_PCS_BOUND_V1: bool = false;
const ONE_POINT_OPENING_QUOTIENTS_IMPLEMENTED_V1: bool = false;
const COMPLETE_WORK_BOUND_DERIVED_V1: bool = false;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum QPcsMaskingErrorV1 {
    InvalidGeometry,
    InvalidModulus,
    InvalidCoefficientCount,
    NonCanonicalResidue,
    InvalidPublicBinding,
    ReusedChallengeOrDomain,
    AliasedSourcePair,
    DeepPointIsNegacyclicRoot,
    ReusedMask,
    RandomUnavailable,
    MaskingIdentityMismatch,
    ResourceCeilingExceeded,
}

impl fmt::Display for QPcsMaskingErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PrototypeMaskingRepetitionBindingV1 {
    repetition: u8,
    gamma: u64,
    beta: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PrototypeMaskingPublicContextV1 {
    q_pcs_parameter_digest: [u8; 32],
    sealed_source_transcript_digest: [u8; 32],
    limb: u8,
    repetitions: [PrototypeMaskingRepetitionBindingV1; OPENING_REPETITIONS_V1],
}

#[derive(Clone, Copy)]
struct PrototypeBorrowedRelationPairV1<'a> {
    product: &'a [u64],
    quotient: &'a [u64],
}

/// Public-only domain separation metadata passed to the private sampler.
/// Possessing it does not reveal `S`, but neither does it enforce sampler
/// uniformity or independence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PrototypeMaskDomainV1 {
    q_pcs_parameter_digest: [u8; 32],
    sealed_source_transcript_digest: [u8; 32],
    modulus: u64,
    ring_degree: u32,
    limb: u8,
    repetition: u8,
    gamma: u64,
    beta: u64,
}

impl PrototypeMaskDomainV1 {
    fn digest(self) -> [u8; 32] {
        let mut frame = Vec::with_capacity(MASK_DOMAIN_V1.len() + 160);
        frame.extend_from_slice(MASK_DOMAIN_V1);
        frame.push(MASKING_VERSION_V1);
        frame.extend_from_slice(&self.q_pcs_parameter_digest);
        frame.extend_from_slice(&self.sealed_source_transcript_digest);
        frame.extend_from_slice(&self.modulus.to_be_bytes());
        frame.extend_from_slice(&self.ring_degree.to_be_bytes());
        frame.push(self.limb);
        frame.push(self.repetition);
        frame.extend_from_slice(&self.gamma.to_be_bytes());
        frame.extend_from_slice(&self.beta.to_be_bytes());
        keccak256(&frame)
    }
}

/// Prototype-only sampler contract. A production implementation must sample
/// each destination uniformly from `Fq^(N-1)` with independent secret entropy
/// for the supplied domain. This child validates canonical output and exact
/// reuse only; correlated or biased output remains a release blocker.
trait PrototypeUniformCanonicalMaskSamplerV1 {
    fn fill_mask_polynomial_v1(
        &mut self,
        domain: PrototypeMaskDomainV1,
        destination: &mut [u64],
    ) -> Result<(), QPcsMaskingErrorV1>;
}

#[cfg(test)]
std::thread_local! {
    static ZEROIZING_Q_POLYNOMIAL_DROPS_V1: std::cell::Cell<usize> = const {
        std::cell::Cell::new(0)
    };
}

/// Heap-stable owner allocated while all coefficients are zero and filled in
/// place. Deliberately neither cloneable nor debuggable and has no raw-Vec
/// extraction API.
struct ZeroizingQPolynomialV1 {
    coefficients: Vec<u64>,
}

impl ZeroizingQPolynomialV1 {
    fn zeroed(coefficient_count: usize) -> Result<Self, QPcsMaskingErrorV1> {
        if coefficient_count == 0 {
            return Err(QPcsMaskingErrorV1::InvalidCoefficientCount);
        }
        let mut coefficients = Vec::new();
        coefficients
            .try_reserve_exact(coefficient_count)
            .map_err(|_| QPcsMaskingErrorV1::ResourceCeilingExceeded)?;
        coefficients.resize(coefficient_count, 0);
        Ok(Self { coefficients })
    }

    fn as_slice(&self) -> &[u64] {
        &self.coefficients
    }

    fn as_mut_slice(&mut self) -> &mut [u64] {
        &mut self.coefficients
    }
}

impl Drop for ZeroizingQPolynomialV1 {
    fn drop(&mut self) {
        let coefficients = core::hint::black_box(&mut self.coefficients);
        coefficients.fill(0);
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
        #[cfg(test)]
        if coefficients.iter().all(|coefficient| *coefficient == 0) {
            let _ = ZEROIZING_Q_POLYNOMIAL_DROPS_V1
                .try_with(|drops| drops.set(drops.get().saturating_add(1)));
        }
        let _ = core::hint::black_box(&mut *coefficients);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MaskedRowRoleV1 {
    Product = 1,
    Quotient = 2,
}

struct ZeroizingMaskedRowV1 {
    repetition: u8,
    role: MaskedRowRoleV1,
    polynomial: ZeroizingQPolynomialV1,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PrototypePostcommitOpeningBindingV1 {
    repetition: u8,
    r: u64,
    opening_transcript_digest: [u8; 32],
}

/// Move-only precommit state. Construction has sampled and masked all ten
/// fixed-width rows, but no DEEP point has been accepted yet. It is not a
/// commitment, proof, receipt, or capability.
struct PendingMaskedRowsV1 {
    modulus: u64,
    ring_degree: usize,
    context: PrototypeMaskingPublicContextV1,
    precommit_order_digest: [u8; 32],
    rows: [ZeroizingMaskedRowV1; MASKED_ROW_COUNT_V1],
}

/// Intermediate state proving only that the caller supplied a nonzero root
/// after row construction. The root is not authenticated by a PCS here.
struct RootSealedMaskedRowsV1 {
    pending: PendingMaskedRowsV1,
    masked_rows_root: [u8; 32],
}

/// Private terminal prototype state after caller-supplied postcommit points
/// pass local checks. Root authenticity and PCS/Fiat-Shamir binding are false;
/// no production function consumes this state and it grants no authority.
struct BoundMaskedRowsV1 {
    sealed: RootSealedMaskedRowsV1,
    postcommit_order_digest: [u8; 32],
    masked_row_root_authenticity_verified: bool,
    postcommit_opening_points_pcs_bound: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct QPcsFiveRepetitionMaskingAccountingV1 {
    repetition_count: u8,
    masked_row_count: u8,
    current_fri_batch_rows: u8,
    masked_cross_limb_leaf_bytes: u32,
    fri_cross_limb_leaf_bytes: u16,
    initial_opened_leaves: u16,
    fri_opened_leaves: u32,
    authentication_hashes: u32,
    fixed_envelope_bytes: u32,
    maximum_encoded_proof_bytes: u64,
    remaining_global_proof_budget_bytes: u64,
    isolated_kernel_heap_bytes: u64,
    mask_construction_peak_bytes: u64,
    ten_row_lde_spool_bytes: u64,
    retained_masked_aggregate_bytes: u64,
    minimum_external_peak_bytes: u64,
    fri_current_and_next_external_bytes: u64,
    minimum_accepted_mask_bytes: u64,
    initial_leaf_hash_input_bytes: u64,
    fft_transform_count: u16,
    fft_butterflies: u64,
    merkle_hash_invocations: u64,
    fri_folded_row_values: u64,
    masking_field_updates: u64,
    coarse_classified_work_units: u64,
    source_owned_aggregate_pairs_linked: bool,
    fiat_shamir_relation_bound: bool,
    production_uniform_independent_sampler_integrated: bool,
    sampler_entropy_and_work_accounted: bool,
    ten_row_pcs_proof_integrated: bool,
    ten_row_pcs_wiring_implemented: bool,
    zero_knowledge_qualified: bool,
    end_to_end_source_residency_accounted: bool,
    masked_row_root_authenticity_verified: bool,
    postcommit_opening_points_pcs_bound: bool,
    one_point_opening_quotients_implemented: bool,
    complete_work_bound_derived: bool,
}

fn maximum_authentication_nodes_v1(tree_length: usize, opened_leaves: usize) -> usize {
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

fn complete_merkle_tree_hashes_v1(leaves: usize) -> Result<u64, QPcsMaskingErrorV1> {
    u64::try_from(
        leaves
            .checked_mul(2)
            .and_then(|value| value.checked_sub(1))
            .ok_or(QPcsMaskingErrorV1::ResourceCeilingExceeded)?,
    )
    .map_err(|_| QPcsMaskingErrorV1::ResourceCeilingExceeded)
}

fn q_pcs_five_repetition_masking_accounting_v1()
-> Result<QPcsFiveRepetitionMaskingAccountingV1, QPcsMaskingErrorV1> {
    let n = ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1;
    if n != 1 << RELEASE_LOG_N_V1
        || RELEASE_DOMAIN_SIZE_V1 != 4 * n
        || OPENING_REPETITIONS_V1 != 5
        || MASKED_ROW_COUNT_V1 != 10
        || BATCH_ROWS_V1 != 2
    {
        return Err(QPcsMaskingErrorV1::InvalidGeometry);
    }

    let initial_opened = 2_usize
        .checked_mul(RELEASE_FRI_QUERY_COUNT_V1)
        .ok_or(QPcsMaskingErrorV1::ResourceCeilingExceeded)?;
    let initial_auth = maximum_authentication_nodes_v1(RELEASE_DOMAIN_SIZE_V1, initial_opened);
    let mut fri_opened = 0_usize;
    let mut fri_auth = 0_usize;
    let mut fri_tree_hashes = 0_u64;
    for log in (2..=RELEASE_DOMAIN_LOG_V1).rev() {
        let length = 1_usize
            .checked_shl(
                u32::try_from(log).map_err(|_| QPcsMaskingErrorV1::ResourceCeilingExceeded)?,
            )
            .ok_or(QPcsMaskingErrorV1::ResourceCeilingExceeded)?;
        let opened = initial_opened.min(length);
        fri_opened = fri_opened
            .checked_add(opened)
            .ok_or(QPcsMaskingErrorV1::ResourceCeilingExceeded)?;
        fri_auth = fri_auth
            .checked_add(maximum_authentication_nodes_v1(length, opened))
            .ok_or(QPcsMaskingErrorV1::ResourceCeilingExceeded)?;
        fri_tree_hashes = fri_tree_hashes
            .checked_add(complete_merkle_tree_hashes_v1(length)?)
            .ok_or(QPcsMaskingErrorV1::ResourceCeilingExceeded)?;
    }

    let initial_value_bytes = 2_usize
        .checked_mul(initial_opened)
        .and_then(|count| count.checked_mul(RELEASE_MASKED_CROSS_LIMB_LEAF_BYTES_V1))
        .ok_or(QPcsMaskingErrorV1::ResourceCeilingExceeded)?;
    let fri_value_bytes = fri_opened
        .checked_mul(RELEASE_FRI_CROSS_LIMB_LEAF_BYTES_V1)
        .ok_or(QPcsMaskingErrorV1::ResourceCeilingExceeded)?;
    let authentication_hashes = 2_usize
        .checked_mul(initial_auth)
        .and_then(|count| count.checked_add(fri_auth))
        .ok_or(QPcsMaskingErrorV1::ResourceCeilingExceeded)?;
    let authentication_bytes = authentication_hashes
        .checked_mul(HASH_BYTES_V1)
        .ok_or(QPcsMaskingErrorV1::ResourceCeilingExceeded)?;

    let fixed_envelope_bytes = HASH_BYTES_V1
        + RELEASE_FRI_ROUNDS_V1 * HASH_BYTES_V1
        + BATCH_ROWS_V1 * RELEASE_FRI_CROSS_LIMB_LEAF_BYTES_V1
        + RELEASE_LIMBS_V1
            * OPENING_REPETITIONS_V1
            * MASKED_ROWS_PER_REPETITION_V1
            * RELEASE_BASE_FIELD_WIRE_BYTES_V1
        + (RELEASE_FRI_ROUNDS_V1 + 2) * RELEASE_COUNT_HEADER_BYTES_V1
        + RELEASE_FIXED_ENVELOPE_HEADER_BYTES_V1;
    let maximum_encoded_proof_bytes = initial_value_bytes
        .checked_add(fri_value_bytes)
        .and_then(|bytes| bytes.checked_add(authentication_bytes))
        .and_then(|bytes| bytes.checked_add(fixed_envelope_bytes))
        .ok_or(QPcsMaskingErrorV1::ResourceCeilingExceeded)?;

    // Each independently aggregated pair owns fixed-width P~(2N-1), H~(N-1),
    // one-point QP(2N-2), and one-point QH(N-2): exactly 6N-6 u64 words. Five
    // pairs cannot reuse the one-pair coefficient storage.
    let coefficient_words_per_pair = (2 * n - 1)
        .checked_add(n - 1)
        .and_then(|words| words.checked_add(2 * n - 2))
        .and_then(|words| words.checked_add(n - 2))
        .ok_or(QPcsMaskingErrorV1::ResourceCeilingExceeded)?;
    let coefficient_words = coefficient_words_per_pair
        .checked_mul(OPENING_REPETITIONS_V1)
        .ok_or(QPcsMaskingErrorV1::ResourceCeilingExceeded)?;
    let coefficient_heap_bytes = coefficient_words
        .checked_mul(core::mem::size_of::<u64>())
        .ok_or(QPcsMaskingErrorV1::ResourceCeilingExceeded)?;
    let mask_construction_peak_bytes = (20 * n - 15)
        .checked_mul(core::mem::size_of::<u64>())
        .ok_or(QPcsMaskingErrorV1::ResourceCeilingExceeded)?;
    let fri_current_and_next_heap_bytes = RELEASE_DOMAIN_SIZE_V1
        .checked_add(RELEASE_DOMAIN_SIZE_V1 / 2)
        .and_then(|values| values.checked_mul(BATCH_ROWS_V1 * FQ2_WIRE_BYTES_V1))
        .ok_or(QPcsMaskingErrorV1::ResourceCeilingExceeded)?;
    let merkle_frontier_bytes = (RELEASE_DOMAIN_LOG_V1 + 1)
        .checked_mul(HASH_BYTES_V1)
        .ok_or(QPcsMaskingErrorV1::ResourceCeilingExceeded)?;
    let isolated_kernel_heap_bytes = coefficient_heap_bytes
        .checked_add(fri_current_and_next_heap_bytes)
        .and_then(|bytes| bytes.checked_add(maximum_encoded_proof_bytes))
        .and_then(|bytes| bytes.checked_add(RELEASE_EXTERNAL_IO_BUFFER_BYTES_V1))
        .and_then(|bytes| bytes.checked_add(merkle_frontier_bytes))
        .and_then(|bytes| bytes.checked_add(RELEASE_MASKED_CROSS_LIMB_LEAF_BYTES_V1))
        .ok_or(QPcsMaskingErrorV1::ResourceCeilingExceeded)?;
    let ten_row_lde_spool_bytes = RELEASE_DOMAIN_SIZE_V1
        .checked_mul(RELEASE_MASKED_CROSS_LIMB_LEAF_BYTES_V1)
        .ok_or(QPcsMaskingErrorV1::ResourceCeilingExceeded)?;
    let retained_masked_aggregate_bytes = (3 * n - 2)
        .checked_mul(core::mem::size_of::<u64>())
        .and_then(|bytes| bytes.checked_mul(RELEASE_LIMBS_V1))
        .and_then(|bytes| bytes.checked_mul(OPENING_REPETITIONS_V1))
        .ok_or(QPcsMaskingErrorV1::ResourceCeilingExceeded)?;
    let minimum_external_peak_bytes = ten_row_lde_spool_bytes
        .checked_add(retained_masked_aggregate_bytes)
        .ok_or(QPcsMaskingErrorV1::ResourceCeilingExceeded)?;
    let fri_current_and_next_external_bytes = RELEASE_DOMAIN_SIZE_V1
        .checked_add(RELEASE_DOMAIN_SIZE_V1 / 2)
        .and_then(|values| values.checked_mul(RELEASE_FRI_CROSS_LIMB_LEAF_BYTES_V1))
        .ok_or(QPcsMaskingErrorV1::ResourceCeilingExceeded)?;
    let minimum_accepted_mask_bytes = (n - 1)
        .checked_mul(RELEASE_LIMBS_V1)
        .and_then(|words| words.checked_mul(OPENING_REPETITIONS_V1))
        .and_then(|words| words.checked_mul(core::mem::size_of::<u64>()))
        .ok_or(QPcsMaskingErrorV1::ResourceCeilingExceeded)?;
    let initial_leaf_hash_input_bytes = ten_row_lde_spool_bytes
        .checked_mul(4)
        .ok_or(QPcsMaskingErrorV1::ResourceCeilingExceeded)?;

    let butterflies_per_fft = u64::try_from(RELEASE_DOMAIN_SIZE_V1 / 2)
        .map_err(|_| QPcsMaskingErrorV1::ResourceCeilingExceeded)?
        .checked_mul(
            u64::try_from(RELEASE_DOMAIN_LOG_V1)
                .map_err(|_| QPcsMaskingErrorV1::ResourceCeilingExceeded)?,
        )
        .ok_or(QPcsMaskingErrorV1::ResourceCeilingExceeded)?;
    let fft_transform_count = 12_usize
        .checked_mul(RELEASE_LIMBS_V1)
        .and_then(|count| count.checked_mul(OPENING_REPETITIONS_V1))
        .ok_or(QPcsMaskingErrorV1::ResourceCeilingExceeded)?;
    let fft_butterflies = butterflies_per_fft
        .checked_mul(
            u64::try_from(fft_transform_count)
                .map_err(|_| QPcsMaskingErrorV1::ResourceCeilingExceeded)?,
        )
        .ok_or(QPcsMaskingErrorV1::ResourceCeilingExceeded)?;
    let merkle_hash_invocations = 4_u64
        .checked_mul(complete_merkle_tree_hashes_v1(RELEASE_DOMAIN_SIZE_V1)?)
        .and_then(|count| count.checked_add(2 * fri_tree_hashes))
        .ok_or(QPcsMaskingErrorV1::ResourceCeilingExceeded)?;
    let fri_folded_row_values = u64::try_from(RELEASE_DOMAIN_SIZE_V1 - 2)
        .map_err(|_| QPcsMaskingErrorV1::ResourceCeilingExceeded)?
        .checked_mul(
            u64::try_from(RELEASE_LIMBS_V1 * BATCH_ROWS_V1 * 2)
                .map_err(|_| QPcsMaskingErrorV1::ResourceCeilingExceeded)?,
        )
        .ok_or(QPcsMaskingErrorV1::ResourceCeilingExceeded)?;
    // Construction performs 3(N-1) modular additions. The explicit residual
    // check performs four modular subtractions at each of 2N-1 coefficients,
    // for exactly 11N-7 classified field updates per limb/repetition.
    let masking_field_updates =
        u64::try_from((11 * n - 7) * RELEASE_LIMBS_V1 * OPENING_REPETITIONS_V1)
            .map_err(|_| QPcsMaskingErrorV1::ResourceCeilingExceeded)?;
    // This is exact only for the listed coarse categories. It is a lower
    // accounting, not a total-work maximum: batch mixing, one-point quotient
    // construction, sampler/rejection work, and the separately enumerated
    // initial-leaf hash input bytes remain unclassified.
    let coarse_classified_work_units = fft_butterflies
        .checked_add(merkle_hash_invocations)
        .and_then(|count| count.checked_add(fri_folded_row_values))
        .and_then(|count| count.checked_add(masking_field_updates))
        .ok_or(QPcsMaskingErrorV1::ResourceCeilingExceeded)?;

    if mask_construction_peak_bytes > coefficient_heap_bytes
        || maximum_encoded_proof_bytes >= PROOF_CAP_BYTES_V1
        || isolated_kernel_heap_bytes >= RESIDENT_CAP_BYTES_V1
    {
        return Err(QPcsMaskingErrorV1::ResourceCeilingExceeded);
    }

    Ok(QPcsFiveRepetitionMaskingAccountingV1 {
        repetition_count: OPENING_REPETITIONS_V1 as u8,
        masked_row_count: MASKED_ROW_COUNT_V1 as u8,
        current_fri_batch_rows: BATCH_ROWS_V1 as u8,
        masked_cross_limb_leaf_bytes: RELEASE_MASKED_CROSS_LIMB_LEAF_BYTES_V1 as u32,
        fri_cross_limb_leaf_bytes: RELEASE_FRI_CROSS_LIMB_LEAF_BYTES_V1 as u16,
        initial_opened_leaves: initial_opened as u16,
        fri_opened_leaves: fri_opened as u32,
        authentication_hashes: authentication_hashes as u32,
        fixed_envelope_bytes: fixed_envelope_bytes as u32,
        maximum_encoded_proof_bytes: maximum_encoded_proof_bytes as u64,
        remaining_global_proof_budget_bytes: (PROOF_CAP_BYTES_V1 - maximum_encoded_proof_bytes)
            as u64,
        isolated_kernel_heap_bytes: isolated_kernel_heap_bytes as u64,
        mask_construction_peak_bytes: mask_construction_peak_bytes as u64,
        ten_row_lde_spool_bytes: ten_row_lde_spool_bytes as u64,
        retained_masked_aggregate_bytes: retained_masked_aggregate_bytes as u64,
        minimum_external_peak_bytes: minimum_external_peak_bytes as u64,
        fri_current_and_next_external_bytes: fri_current_and_next_external_bytes as u64,
        minimum_accepted_mask_bytes: minimum_accepted_mask_bytes as u64,
        initial_leaf_hash_input_bytes: initial_leaf_hash_input_bytes as u64,
        fft_transform_count: fft_transform_count as u16,
        fft_butterflies,
        merkle_hash_invocations,
        fri_folded_row_values,
        masking_field_updates,
        coarse_classified_work_units,
        source_owned_aggregate_pairs_linked: SOURCE_OWNED_AGGREGATE_PAIRS_LINKED_V1,
        fiat_shamir_relation_bound: FIAT_SHAMIR_RELATION_BOUND_V1,
        production_uniform_independent_sampler_integrated:
            PRODUCTION_UNIFORM_INDEPENDENT_SAMPLER_INTEGRATED_V1,
        sampler_entropy_and_work_accounted: SAMPLER_ENTROPY_AND_WORK_ACCOUNTED_V1,
        ten_row_pcs_proof_integrated: TEN_ROW_PCS_PROOF_INTEGRATED_V1,
        ten_row_pcs_wiring_implemented: TEN_ROW_PCS_WIRING_IMPLEMENTED_V1,
        zero_knowledge_qualified: ZERO_KNOWLEDGE_QUALIFIED_V1,
        end_to_end_source_residency_accounted: END_TO_END_SOURCE_RESIDENCY_ACCOUNTED_V1,
        masked_row_root_authenticity_verified: MASKED_ROW_ROOT_AUTHENTICITY_VERIFIED_V1,
        postcommit_opening_points_pcs_bound: POSTCOMMIT_OPENING_POINTS_PCS_BOUND_V1,
        one_point_opening_quotients_implemented: ONE_POINT_OPENING_QUOTIENTS_IMPLEMENTED_V1,
        complete_work_bound_derived: COMPLETE_WORK_BOUND_DERIVED_V1,
    })
}

fn validate_input_polynomial_v1(
    coefficients: &[u64],
    fixed_width: usize,
    modulus: u64,
) -> Result<(), QPcsMaskingErrorV1> {
    if coefficients.len() != fixed_width {
        return Err(QPcsMaskingErrorV1::InvalidCoefficientCount);
    }
    if coefficients
        .iter()
        .any(|coefficient| *coefficient >= modulus)
    {
        return Err(QPcsMaskingErrorV1::NonCanonicalResidue);
    }
    Ok(())
}

fn nonempty_u64_slices_overlap_v1(left: &[u64], right: &[u64]) -> Result<bool, QPcsMaskingErrorV1> {
    if left.is_empty() || right.is_empty() {
        return Err(QPcsMaskingErrorV1::InvalidCoefficientCount);
    }
    let left_start = left.as_ptr() as usize;
    let right_start = right.as_ptr() as usize;
    let left_end = left_start
        .checked_add(
            left.len()
                .checked_mul(core::mem::size_of::<u64>())
                .ok_or(QPcsMaskingErrorV1::ResourceCeilingExceeded)?,
        )
        .ok_or(QPcsMaskingErrorV1::ResourceCeilingExceeded)?;
    let right_end = right_start
        .checked_add(
            right
                .len()
                .checked_mul(core::mem::size_of::<u64>())
                .ok_or(QPcsMaskingErrorV1::ResourceCeilingExceeded)?,
        )
        .ok_or(QPcsMaskingErrorV1::ResourceCeilingExceeded)?;
    Ok(left_start < right_end && right_start < left_end)
}

fn mask_domain_v1(
    modulus: u64,
    ring_degree: usize,
    context: PrototypeMaskingPublicContextV1,
    binding: PrototypeMaskingRepetitionBindingV1,
) -> Result<PrototypeMaskDomainV1, QPcsMaskingErrorV1> {
    Ok(PrototypeMaskDomainV1 {
        q_pcs_parameter_digest: context.q_pcs_parameter_digest,
        sealed_source_transcript_digest: context.sealed_source_transcript_digest,
        modulus,
        ring_degree: u32::try_from(ring_degree)
            .map_err(|_| QPcsMaskingErrorV1::ResourceCeilingExceeded)?,
        limb: context.limb,
        repetition: binding.repetition,
        gamma: binding.gamma,
        beta: binding.beta,
    })
}

fn validate_precommit_context_and_domains_v1(
    modulus: u64,
    ring_degree: usize,
    context: PrototypeMaskingPublicContextV1,
) -> Result<[PrototypeMaskDomainV1; OPENING_REPETITIONS_V1], QPcsMaskingErrorV1> {
    if context.q_pcs_parameter_digest == [0; 32]
        || context.sealed_source_transcript_digest == [0; 32]
        || usize::from(context.limb) >= RELEASE_LIMBS_V1
    {
        return Err(QPcsMaskingErrorV1::InvalidPublicBinding);
    }
    let mut domains = [mask_domain_v1(modulus, ring_degree, context, context.repetitions[0])?;
        OPENING_REPETITIONS_V1];
    let mut domain_digests = [[0_u8; 32]; OPENING_REPETITIONS_V1];
    let mut prior_challenges = [0_u64; OPENING_REPETITIONS_V1 * 2];
    for (index, binding) in context.repetitions.iter().copied().enumerate() {
        if usize::from(binding.repetition) != index {
            return Err(QPcsMaskingErrorV1::InvalidPublicBinding);
        }
        if binding.gamma == 0
            || binding.beta == 0
            || binding.gamma >= modulus
            || binding.beta >= modulus
        {
            return Err(QPcsMaskingErrorV1::NonCanonicalResidue);
        }
        if binding.gamma == binding.beta
            || prior_challenges[..2 * index].contains(&binding.gamma)
            || prior_challenges[..2 * index].contains(&binding.beta)
        {
            return Err(QPcsMaskingErrorV1::ReusedChallengeOrDomain);
        }
        prior_challenges[2 * index] = binding.gamma;
        prior_challenges[2 * index + 1] = binding.beta;
        let domain = mask_domain_v1(modulus, ring_degree, context, binding)?;
        let digest = domain.digest();
        if digest == [0; 32] || domain_digests[..index].contains(&digest) {
            return Err(QPcsMaskingErrorV1::ReusedChallengeOrDomain);
        }
        domains[index] = domain;
        domain_digests[index] = digest;
    }
    Ok(domains)
}

fn coefficient_at_v1(coefficients: &[u64], index: usize) -> u64 {
    coefficients.get(index).copied().unwrap_or(0)
}

fn verify_preserved_residual_v1(
    product: &[u64],
    quotient: &[u64],
    masked_product: &[u64],
    masked_quotient: &[u64],
    ring_degree: usize,
    modulus: u64,
) -> Result<(), QPcsMaskingErrorV1> {
    let residual_coefficient_count = ring_degree
        .checked_mul(2)
        .and_then(|count| count.checked_sub(1))
        .ok_or(QPcsMaskingErrorV1::ResourceCeilingExceeded)?;
    for degree in 0..residual_coefficient_count {
        let quotient_low = coefficient_at_v1(quotient, degree);
        let quotient_high = degree
            .checked_sub(ring_degree)
            .map_or(0, |index| coefficient_at_v1(quotient, index));
        let original = mod_sub_v1(
            mod_sub_v1(coefficient_at_v1(product, degree), quotient_low, modulus),
            quotient_high,
            modulus,
        );
        let masked_quotient_low = coefficient_at_v1(masked_quotient, degree);
        let masked_quotient_high = degree
            .checked_sub(ring_degree)
            .map_or(0, |index| coefficient_at_v1(masked_quotient, index));
        let masked = mod_sub_v1(
            mod_sub_v1(
                coefficient_at_v1(masked_product, degree),
                masked_quotient_low,
                modulus,
            ),
            masked_quotient_high,
            modulus,
        );
        if original != 0 || masked != 0 {
            return Err(QPcsMaskingErrorV1::MaskingIdentityMismatch);
        }
    }
    Ok(())
}

fn precommit_order_digest_v1(
    context: PrototypeMaskingPublicContextV1,
    domains: &[PrototypeMaskDomainV1; OPENING_REPETITIONS_V1],
) -> Result<[u8; 32], QPcsMaskingErrorV1> {
    let mut frame = Vec::with_capacity(MASK_PRECOMMIT_ORDER_DOMAIN_V1.len() + 32 * 7 + 32);
    frame.extend_from_slice(MASK_PRECOMMIT_ORDER_DOMAIN_V1);
    frame.push(MASKING_VERSION_V1);
    frame.extend_from_slice(&context.q_pcs_parameter_digest);
    frame.extend_from_slice(&context.sealed_source_transcript_digest);
    frame.push(context.limb);
    frame.push(MASKED_ROW_COUNT_V1 as u8);
    for domain in domains {
        frame.extend_from_slice(&domain.digest());
        frame.push(domain.repetition);
        frame.push(MaskedRowRoleV1::Product as u8);
        frame.push(MaskedRowRoleV1::Quotient as u8);
    }
    let digest = keccak256(&frame);
    if digest == [0; 32] {
        return Err(QPcsMaskingErrorV1::InvalidPublicBinding);
    }
    Ok(digest)
}

fn mask_one_limb_five_repetitions_v1<S: PrototypeUniformCanonicalMaskSamplerV1>(
    modulus: u64,
    ring_degree: usize,
    pairs: [PrototypeBorrowedRelationPairV1<'_>; OPENING_REPETITIONS_V1],
    context: PrototypeMaskingPublicContextV1,
    sampler: &mut S,
) -> Result<PendingMaskedRowsV1, QPcsMaskingErrorV1> {
    if modulus < 3
        || modulus >= 1_u64 << 62
        || modulus.is_multiple_of(2)
        || !is_prime_u64(modulus)
        || ring_degree < 8
        || !ring_degree.is_power_of_two()
    {
        return Err(QPcsMaskingErrorV1::InvalidModulus);
    }
    let product_coefficient_count = ring_degree
        .checked_mul(2)
        .and_then(|count| count.checked_sub(1))
        .ok_or(QPcsMaskingErrorV1::ResourceCeilingExceeded)?;
    for (index, pair) in pairs.iter().enumerate() {
        validate_input_polynomial_v1(pair.product, product_coefficient_count, modulus)?;
        validate_input_polynomial_v1(pair.quotient, ring_degree - 1, modulus)?;
        if nonempty_u64_slices_overlap_v1(pair.product, pair.quotient)? {
            return Err(QPcsMaskingErrorV1::AliasedSourcePair);
        }
        for prior in &pairs[..index] {
            for current in [pair.product, pair.quotient] {
                for prior_slice in [prior.product, prior.quotient] {
                    if nonempty_u64_slices_overlap_v1(current, prior_slice)? {
                        return Err(QPcsMaskingErrorV1::AliasedSourcePair);
                    }
                }
            }
        }
    }
    let domains = validate_precommit_context_and_domains_v1(modulus, ring_degree, context)?;
    let precommit_order_digest = precommit_order_digest_v1(context, &domains)?;
    let mut masks = Vec::new();
    masks
        .try_reserve_exact(OPENING_REPETITIONS_V1)
        .map_err(|_| QPcsMaskingErrorV1::ResourceCeilingExceeded)?;
    let mut rows = Vec::new();
    rows.try_reserve_exact(MASKED_ROW_COUNT_V1)
        .map_err(|_| QPcsMaskingErrorV1::ResourceCeilingExceeded)?;

    for (repetition, domain) in domains.iter().copied().enumerate() {
        let mut mask = ZeroizingQPolynomialV1::zeroed(ring_degree - 1)?;
        sampler.fill_mask_polynomial_v1(domain, mask.as_mut_slice())?;
        if mask
            .as_slice()
            .iter()
            .any(|coefficient| *coefficient >= modulus)
        {
            return Err(QPcsMaskingErrorV1::NonCanonicalResidue);
        }
        if masks
            .iter()
            .any(|prior: &ZeroizingQPolynomialV1| prior.as_slice() == mask.as_slice())
        {
            return Err(QPcsMaskingErrorV1::ReusedMask);
        }
        masks.push(mask);
        let mask = masks.last().ok_or(QPcsMaskingErrorV1::InvalidGeometry)?;
        let pair = &pairs[repetition];

        let mut masked_product = ZeroizingQPolynomialV1::zeroed(product_coefficient_count)?;
        let mut masked_quotient = ZeroizingQPolynomialV1::zeroed(ring_degree - 1)?;
        for (index, destination) in masked_product.as_mut_slice().iter_mut().enumerate() {
            *destination = coefficient_at_v1(pair.product, index);
        }
        for (index, destination) in masked_quotient.as_mut_slice().iter_mut().enumerate() {
            *destination = coefficient_at_v1(pair.quotient, index);
        }
        for (index, mask_coefficient) in mask.as_slice().iter().copied().enumerate() {
            let quotient_coefficient = masked_quotient.as_slice()[index];
            masked_quotient.as_mut_slice()[index] =
                mod_add_v1(quotient_coefficient, mask_coefficient, modulus);
            let product_low = masked_product.as_slice()[index];
            masked_product.as_mut_slice()[index] =
                mod_add_v1(product_low, mask_coefficient, modulus);
            let high = index + ring_degree;
            let product_high = masked_product.as_slice()[high];
            masked_product.as_mut_slice()[high] =
                mod_add_v1(product_high, mask_coefficient, modulus);
        }
        verify_preserved_residual_v1(
            pair.product,
            pair.quotient,
            masked_product.as_slice(),
            masked_quotient.as_slice(),
            ring_degree,
            modulus,
        )?;

        let repetition =
            u8::try_from(repetition).map_err(|_| QPcsMaskingErrorV1::ResourceCeilingExceeded)?;
        rows.push(ZeroizingMaskedRowV1 {
            repetition,
            role: MaskedRowRoleV1::Product,
            polynomial: masked_product,
        });
        rows.push(ZeroizingMaskedRowV1 {
            repetition,
            role: MaskedRowRoleV1::Quotient,
            polynomial: masked_quotient,
        });
    }

    let rows = rows
        .try_into()
        .map_err(|_: Vec<ZeroizingMaskedRowV1>| QPcsMaskingErrorV1::InvalidGeometry)?;
    Ok(PendingMaskedRowsV1 {
        modulus,
        ring_degree,
        context,
        precommit_order_digest,
        rows,
    })
}

impl PendingMaskedRowsV1 {
    fn seal_masked_rows_root_v1(
        self,
        masked_rows_root: [u8; 32],
    ) -> Result<RootSealedMaskedRowsV1, QPcsMaskingErrorV1> {
        if masked_rows_root == [0; 32] {
            return Err(QPcsMaskingErrorV1::InvalidPublicBinding);
        }
        Ok(RootSealedMaskedRowsV1 {
            pending: self,
            masked_rows_root,
        })
    }
}

impl RootSealedMaskedRowsV1 {
    /// Accepts postcommit points only from a root-sealed type state. This
    /// prototype validates ordering and field constraints but cannot
    /// authenticate the root or derive `r` itself.
    fn bind_opening_points_v1(
        self,
        openings: [PrototypePostcommitOpeningBindingV1; OPENING_REPETITIONS_V1],
    ) -> Result<BoundMaskedRowsV1, QPcsMaskingErrorV1> {
        let mut tuples = [QPcsChallengeTupleV1 {
            r: 0,
            gamma: 0,
            beta: 0,
        }; OPENING_REPETITIONS_V1];
        for (index, opening) in openings.iter().copied().enumerate() {
            let precommit = self.pending.context.repetitions[index];
            if usize::from(opening.repetition) != index
                || opening.opening_transcript_digest == [0; 32]
                || openings[..index].iter().any(|prior| {
                    prior.opening_transcript_digest == opening.opening_transcript_digest
                })
            {
                return Err(QPcsMaskingErrorV1::InvalidPublicBinding);
            }
            tuples[index] = QPcsChallengeTupleV1 {
                r: opening.r,
                gamma: precommit.gamma,
                beta: precommit.beta,
            };
        }
        validate_challenge_tuples_v1(self.pending.modulus, &tuples).map_err(
            |error| match error {
                QPcsErrorV1::InvalidChallenge => QPcsMaskingErrorV1::NonCanonicalResidue,
                QPcsErrorV1::ReusedChallenge => QPcsMaskingErrorV1::ReusedChallengeOrDomain,
                _ => QPcsMaskingErrorV1::InvalidPublicBinding,
            },
        )?;
        let exponent = u64::try_from(self.pending.ring_degree)
            .map_err(|_| QPcsMaskingErrorV1::ResourceCeilingExceeded)?;
        for opening in openings {
            if mod_add_v1(
                mod_pow_v1(opening.r, exponent, self.pending.modulus),
                1,
                self.pending.modulus,
            ) == 0
            {
                return Err(QPcsMaskingErrorV1::DeepPointIsNegacyclicRoot);
            }
        }

        // Transcript order is explicit: the masked-row root is absorbed before
        // any postcommit r or opening-frame digest.
        let mut frame = Vec::with_capacity(MASK_POSTCOMMIT_ORDER_DOMAIN_V1.len() + 32 * 8 + 64);
        frame.extend_from_slice(MASK_POSTCOMMIT_ORDER_DOMAIN_V1);
        frame.push(MASKING_VERSION_V1);
        frame.extend_from_slice(&self.pending.precommit_order_digest);
        frame.extend_from_slice(&self.masked_rows_root);
        for (index, opening) in openings.iter().copied().enumerate() {
            frame.push(opening.repetition);
            frame.extend_from_slice(&opening.r.to_be_bytes());
            frame.extend_from_slice(&self.pending.context.repetitions[index].gamma.to_be_bytes());
            frame.extend_from_slice(&self.pending.context.repetitions[index].beta.to_be_bytes());
            frame.extend_from_slice(&opening.opening_transcript_digest);
        }
        let postcommit_order_digest = keccak256(&frame);
        if postcommit_order_digest == [0; 32] {
            return Err(QPcsMaskingErrorV1::InvalidPublicBinding);
        }
        Ok(BoundMaskedRowsV1 {
            sealed: self,
            postcommit_order_digest,
            masked_row_root_authenticity_verified: MASKED_ROW_ROOT_AUTHENTICITY_VERIFIED_V1,
            postcommit_opening_points_pcs_bound: POSTCOMMIT_OPENING_POINTS_PCS_BOUND_V1,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::panic::{AssertUnwindSafe, catch_unwind};

    use super::*;

    const TEST_MODULUS_V1: u64 = 97;
    const TEST_RING_DEGREE_V1: usize = 8;

    fn test_relation_owners_v1() -> ([Vec<u64>; 5], [Vec<u64>; 5]) {
        let quotients = core::array::from_fn(|repetition| {
            (0..TEST_RING_DEGREE_V1 - 1)
                .map(|index| ((17 * repetition + 5 * index + 1) as u64) % TEST_MODULUS_V1)
                .collect::<Vec<_>>()
        });
        let products = core::array::from_fn(|repetition| {
            let mut product = vec![0_u64; 2 * TEST_RING_DEGREE_V1 - 1];
            for (index, coefficient) in quotients[repetition].iter().copied().enumerate() {
                product[index] = coefficient;
                product[index + TEST_RING_DEGREE_V1] = coefficient;
            }
            product
        });
        (products, quotients)
    }

    fn borrowed_pairs_v1<'a>(
        products: &'a [Vec<u64>; 5],
        quotients: &'a [Vec<u64>; 5],
    ) -> [PrototypeBorrowedRelationPairV1<'a>; 5] {
        core::array::from_fn(|index| PrototypeBorrowedRelationPairV1 {
            product: &products[index],
            quotient: &quotients[index],
        })
    }

    fn test_context_v1() -> PrototypeMaskingPublicContextV1 {
        let gamma_beta = [(3, 4), (6, 7), (10, 11), (14, 15), (19, 20)];
        PrototypeMaskingPublicContextV1 {
            q_pcs_parameter_digest: [0x31; 32],
            sealed_source_transcript_digest: [0x52; 32],
            limb: 3,
            repetitions: core::array::from_fn(|index| PrototypeMaskingRepetitionBindingV1 {
                repetition: u8::try_from(index).unwrap(),
                gamma: gamma_beta[index].0,
                beta: gamma_beta[index].1,
            }),
        }
    }

    fn test_openings_v1() -> [PrototypePostcommitOpeningBindingV1; OPENING_REPETITIONS_V1] {
        let points = [2, 5, 9, 13, 17];
        core::array::from_fn(|index| PrototypePostcommitOpeningBindingV1 {
            repetition: u8::try_from(index).unwrap(),
            r: points[index],
            opening_transcript_digest: [u8::try_from(index + 21).unwrap(); 32],
        })
    }

    #[derive(Default)]
    struct DomainAwareSamplerV1 {
        domains: Vec<PrototypeMaskDomainV1>,
    }

    impl PrototypeUniformCanonicalMaskSamplerV1 for DomainAwareSamplerV1 {
        fn fill_mask_polynomial_v1(
            &mut self,
            domain: PrototypeMaskDomainV1,
            destination: &mut [u64],
        ) -> Result<(), QPcsMaskingErrorV1> {
            self.domains.push(domain);
            for (index, coefficient) in destination.iter_mut().enumerate() {
                *coefficient = (u64::from(domain.repetition + 1) * 23
                    + u64::try_from(index).unwrap() * 7)
                    % domain.modulus;
            }
            *destination.last_mut().unwrap() = 0;
            Ok(())
        }
    }

    struct SameMaskSamplerV1;

    impl PrototypeUniformCanonicalMaskSamplerV1 for SameMaskSamplerV1 {
        fn fill_mask_polynomial_v1(
            &mut self,
            _domain: PrototypeMaskDomainV1,
            destination: &mut [u64],
        ) -> Result<(), QPcsMaskingErrorV1> {
            destination.fill(7);
            Ok(())
        }
    }

    struct NonCanonicalSamplerV1;

    impl PrototypeUniformCanonicalMaskSamplerV1 for NonCanonicalSamplerV1 {
        fn fill_mask_polynomial_v1(
            &mut self,
            domain: PrototypeMaskDomainV1,
            destination: &mut [u64],
        ) -> Result<(), QPcsMaskingErrorV1> {
            destination.fill(1);
            destination[0] = domain.modulus;
            Ok(())
        }
    }

    struct FailOrPanicSamplerV1 {
        calls: usize,
        fail_at: usize,
        panic_instead: bool,
    }

    impl PrototypeUniformCanonicalMaskSamplerV1 for FailOrPanicSamplerV1 {
        fn fill_mask_polynomial_v1(
            &mut self,
            domain: PrototypeMaskDomainV1,
            destination: &mut [u64],
        ) -> Result<(), QPcsMaskingErrorV1> {
            let call = self.calls;
            self.calls += 1;
            if call == self.fail_at {
                destination.fill(0x2a);
                if self.panic_instead {
                    panic!("injected masking sampler unwind");
                }
                return Err(QPcsMaskingErrorV1::RandomUnavailable);
            }
            for (index, coefficient) in destination.iter_mut().enumerate() {
                *coefficient = (u64::from(domain.repetition + 1) * 29
                    + u64::try_from(index).unwrap() * 3)
                    % domain.modulus;
            }
            Ok(())
        }
    }

    fn reset_zeroizing_drop_count_v1() {
        let _ = ZEROIZING_Q_POLYNOMIAL_DROPS_V1.try_with(|drops| drops.set(0));
    }

    fn zeroizing_drop_count_v1() -> usize {
        ZEROIZING_Q_POLYNOMIAL_DROPS_V1
            .try_with(std::cell::Cell::get)
            .unwrap_or(usize::MAX)
    }

    #[test]
    fn five_masks_preserve_the_residual_and_encode_exact_ten_row_order() {
        let (products, quotients) = test_relation_owners_v1();
        let context = test_context_v1();
        let mut sampler = DomainAwareSamplerV1::default();
        let pending = mask_one_limb_five_repetitions_v1(
            TEST_MODULUS_V1,
            TEST_RING_DEGREE_V1,
            borrowed_pairs_v1(&products, &quotients),
            context,
            &mut sampler,
        )
        .unwrap();

        assert_eq!(pending.modulus, TEST_MODULUS_V1);
        assert_eq!(pending.ring_degree, TEST_RING_DEGREE_V1);
        assert_ne!(pending.precommit_order_digest, [0; 32]);
        assert_eq!(pending.rows.len(), 10);
        assert_eq!(sampler.domains.len(), 5);
        for repetition in 0..OPENING_REPETITIONS_V1 {
            let product = &products[repetition];
            let quotient = &quotients[repetition];
            let product_row = &pending.rows[2 * repetition];
            let quotient_row = &pending.rows[2 * repetition + 1];
            assert_eq!(usize::from(product_row.repetition), repetition);
            assert_eq!(usize::from(quotient_row.repetition), repetition);
            assert_eq!(product_row.role, MaskedRowRoleV1::Product);
            assert_eq!(quotient_row.role, MaskedRowRoleV1::Quotient);
            assert_eq!(product_row.polynomial.as_slice().len(), 15);
            assert_eq!(quotient_row.polynomial.as_slice().len(), 7);
            verify_preserved_residual_v1(
                product,
                quotient,
                product_row.polynomial.as_slice(),
                quotient_row.polynomial.as_slice(),
                TEST_RING_DEGREE_V1,
                TEST_MODULUS_V1,
            )
            .unwrap();

            for index in 0..TEST_RING_DEGREE_V1 - 1 {
                let mask = mod_sub_v1(
                    quotient_row.polynomial.as_slice()[index],
                    quotient[index],
                    TEST_MODULUS_V1,
                );
                assert_eq!(
                    mod_sub_v1(
                        product_row.polynomial.as_slice()[index],
                        product[index],
                        TEST_MODULUS_V1,
                    ),
                    mask
                );
                assert_eq!(
                    mod_sub_v1(
                        product_row.polynomial.as_slice()[index + TEST_RING_DEGREE_V1],
                        product[index + TEST_RING_DEGREE_V1],
                        TEST_MODULUS_V1,
                    ),
                    mask
                );
            }
        }
        for left in 0..OPENING_REPETITIONS_V1 {
            for right in left + 1..OPENING_REPETITIONS_V1 {
                assert_ne!(sampler.domains[left], sampler.domains[right]);
                assert_ne!(
                    sampler.domains[left].digest(),
                    sampler.domains[right].digest()
                );
                assert_ne!(
                    pending.rows[2 * left + 1].polynomial.as_slice(),
                    pending.rows[2 * right + 1].polynomial.as_slice()
                );
            }
        }
        let bound = pending
            .seal_masked_rows_root_v1([0x81; 32])
            .unwrap()
            .bind_opening_points_v1(test_openings_v1())
            .unwrap();
        assert_eq!(bound.sealed.masked_rows_root, [0x81; 32]);
        assert_ne!(bound.postcommit_order_digest, [0; 32]);
        assert_eq!(bound.sealed.pending.rows.len(), 10);
        assert!(!bound.masked_row_root_authenticity_verified);
        assert!(!bound.postcommit_opening_points_pcs_bound);
    }

    #[test]
    fn hostile_repetition_mask_reuse_and_slice_overlap_are_rejected() {
        let (products, quotients) = test_relation_owners_v1();

        let mut same_mask = SameMaskSamplerV1;
        assert!(matches!(
            mask_one_limb_five_repetitions_v1(
                TEST_MODULUS_V1,
                TEST_RING_DEGREE_V1,
                borrowed_pairs_v1(&products, &quotients),
                test_context_v1(),
                &mut same_mask,
            ),
            Err(QPcsMaskingErrorV1::ReusedMask)
        ));
        let mut aliased = borrowed_pairs_v1(&products, &quotients);
        aliased[1] = aliased[0];
        assert!(matches!(
            mask_one_limb_five_repetitions_v1(
                TEST_MODULUS_V1,
                TEST_RING_DEGREE_V1,
                aliased,
                test_context_v1(),
                &mut DomainAwareSamplerV1::default(),
            ),
            Err(QPcsMaskingErrorV1::AliasedSourcePair)
        ));
        let mut intra_pair_alias = borrowed_pairs_v1(&products, &quotients);
        intra_pair_alias[0] = PrototypeBorrowedRelationPairV1 {
            product: &products[0],
            quotient: &products[0][..TEST_RING_DEGREE_V1 - 1],
        };
        assert!(matches!(
            mask_one_limb_five_repetitions_v1(
                TEST_MODULUS_V1,
                TEST_RING_DEGREE_V1,
                intra_pair_alias,
                test_context_v1(),
                &mut DomainAwareSamplerV1::default(),
            ),
            Err(QPcsMaskingErrorV1::AliasedSourcePair)
        ));
        let mut offset_overlap = borrowed_pairs_v1(&products, &quotients);
        offset_overlap[0] = PrototypeBorrowedRelationPairV1 {
            product: &products[0],
            quotient: &products[0][1..TEST_RING_DEGREE_V1],
        };
        assert!(matches!(
            mask_one_limb_five_repetitions_v1(
                TEST_MODULUS_V1,
                TEST_RING_DEGREE_V1,
                offset_overlap,
                test_context_v1(),
                &mut DomainAwareSamplerV1::default(),
            ),
            Err(QPcsMaskingErrorV1::AliasedSourcePair)
        ));
        let (mut equal_products, mut equal_quotients) = test_relation_owners_v1();
        let equal_product = equal_products[0].clone();
        let equal_quotient = equal_quotients[0].clone();
        equal_products[1] = equal_product;
        equal_quotients[1] = equal_quotient;
        let equal_valued = mask_one_limb_five_repetitions_v1(
            TEST_MODULUS_V1,
            TEST_RING_DEGREE_V1,
            borrowed_pairs_v1(&equal_products, &equal_quotients),
            test_context_v1(),
            &mut DomainAwareSamplerV1::default(),
        )
        .expect("equal-valued independent allocations are not aliases");
        drop(equal_valued);

        let mut wrong_repetition = test_context_v1();
        wrong_repetition.repetitions[1].repetition = 0;
        assert_eq!(
            validate_precommit_context_and_domains_v1(
                TEST_MODULUS_V1,
                TEST_RING_DEGREE_V1,
                wrong_repetition,
            ),
            Err(QPcsMaskingErrorV1::InvalidPublicBinding)
        );
        let mut missing_source_transcript = test_context_v1();
        missing_source_transcript.sealed_source_transcript_digest = [0; 32];
        assert_eq!(
            validate_precommit_context_and_domains_v1(
                TEST_MODULUS_V1,
                TEST_RING_DEGREE_V1,
                missing_source_transcript,
            ),
            Err(QPcsMaskingErrorV1::InvalidPublicBinding)
        );
        let mut invalid_limb = test_context_v1();
        invalid_limb.limb = RELEASE_LIMBS_V1 as u8;
        assert_eq!(
            validate_precommit_context_and_domains_v1(
                TEST_MODULUS_V1,
                TEST_RING_DEGREE_V1,
                invalid_limb,
            ),
            Err(QPcsMaskingErrorV1::InvalidPublicBinding)
        );
        let mut reused_challenge = test_context_v1();
        reused_challenge.repetitions[1].gamma = reused_challenge.repetitions[0].beta;
        assert_eq!(
            validate_precommit_context_and_domains_v1(
                TEST_MODULUS_V1,
                TEST_RING_DEGREE_V1,
                reused_challenge,
            ),
            Err(QPcsMaskingErrorV1::ReusedChallengeOrDomain)
        );
        let mut plus_q = test_context_v1();
        plus_q.repetitions[0].gamma = TEST_MODULUS_V1;
        assert_eq!(
            validate_precommit_context_and_domains_v1(TEST_MODULUS_V1, TEST_RING_DEGREE_V1, plus_q,),
            Err(QPcsMaskingErrorV1::NonCanonicalResidue)
        );
    }

    #[test]
    fn fixed_width_canonical_rows_and_postcommit_deep_points_are_enforced() {
        let (mut products, quotients) = test_relation_owners_v1();
        products[0][0] = TEST_MODULUS_V1;
        assert!(matches!(
            mask_one_limb_five_repetitions_v1(
                TEST_MODULUS_V1,
                TEST_RING_DEGREE_V1,
                borrowed_pairs_v1(&products, &quotients),
                test_context_v1(),
                &mut DomainAwareSamplerV1::default(),
            ),
            Err(QPcsMaskingErrorV1::NonCanonicalResidue)
        ));

        let (mut products, quotients) = test_relation_owners_v1();
        products[0].push(0);
        assert!(matches!(
            mask_one_limb_five_repetitions_v1(
                TEST_MODULUS_V1,
                TEST_RING_DEGREE_V1,
                borrowed_pairs_v1(&products, &quotients),
                test_context_v1(),
                &mut DomainAwareSamplerV1::default(),
            ),
            Err(QPcsMaskingErrorV1::InvalidCoefficientCount)
        ));
        let (mut products, mut quotients) = test_relation_owners_v1();
        for repetition in 0..OPENING_REPETITIONS_V1 {
            *quotients[repetition].last_mut().unwrap() = 0;
            products[repetition][TEST_RING_DEGREE_V1 - 2] = 0;
            *products[repetition].last_mut().unwrap() = 0;
        }
        let top_zero_pending = mask_one_limb_five_repetitions_v1(
            TEST_MODULUS_V1,
            TEST_RING_DEGREE_V1,
            borrowed_pairs_v1(&products, &quotients),
            test_context_v1(),
            &mut DomainAwareSamplerV1::default(),
        )
        .expect("a fixed-width canonical row may have a zero top coefficient");
        assert!(
            top_zero_pending
                .rows
                .iter()
                .all(|row| row.polynomial.as_slice().last() == Some(&0))
        );
        drop(top_zero_pending);

        let (products, quotients) = test_relation_owners_v1();
        assert!(matches!(
            mask_one_limb_five_repetitions_v1(
                TEST_MODULUS_V1,
                TEST_RING_DEGREE_V1,
                borrowed_pairs_v1(&products, &quotients),
                test_context_v1(),
                &mut NonCanonicalSamplerV1,
            ),
            Err(QPcsMaskingErrorV1::NonCanonicalResidue)
        ));
        let pending = mask_one_limb_five_repetitions_v1(
            TEST_MODULUS_V1,
            TEST_RING_DEGREE_V1,
            borrowed_pairs_v1(&products, &quotients),
            test_context_v1(),
            &mut DomainAwareSamplerV1::default(),
        )
        .unwrap();
        let mut tampered_masked_product = pending.rows[0].polynomial.as_slice().to_vec();
        tampered_masked_product[0] = mod_add_v1(tampered_masked_product[0], 1, TEST_MODULUS_V1);
        assert_eq!(
            verify_preserved_residual_v1(
                &products[0],
                &quotients[0],
                &tampered_masked_product,
                pending.rows[1].polynomial.as_slice(),
                TEST_RING_DEGREE_V1,
                TEST_MODULUS_V1,
            ),
            Err(QPcsMaskingErrorV1::MaskingIdentityMismatch)
        );
        drop(pending);
        let (mut invalid_products, invalid_quotients) = test_relation_owners_v1();
        invalid_products[0][0] = mod_add_v1(invalid_products[0][0], 1, TEST_MODULUS_V1);
        assert!(matches!(
            mask_one_limb_five_repetitions_v1(
                TEST_MODULUS_V1,
                TEST_RING_DEGREE_V1,
                borrowed_pairs_v1(&invalid_products, &invalid_quotients),
                test_context_v1(),
                &mut DomainAwareSamplerV1::default(),
            ),
            Err(QPcsMaskingErrorV1::MaskingIdentityMismatch)
        ));

        let root = (1..TEST_MODULUS_V1)
            .find(|candidate| {
                mod_add_v1(
                    mod_pow_v1(*candidate, TEST_RING_DEGREE_V1 as u64, TEST_MODULUS_V1),
                    1,
                    TEST_MODULUS_V1,
                ) == 0
            })
            .expect("tiny field has a negacyclic root");
        let make_pending = || {
            let (products, quotients) = test_relation_owners_v1();
            mask_one_limb_five_repetitions_v1(
                TEST_MODULUS_V1,
                TEST_RING_DEGREE_V1,
                borrowed_pairs_v1(&products, &quotients),
                test_context_v1(),
                &mut DomainAwareSamplerV1::default(),
            )
            .unwrap()
        };
        let mut root_openings = test_openings_v1();
        root_openings[0].r = root;
        assert!(matches!(
            make_pending()
                .seal_masked_rows_root_v1([0x91; 32])
                .unwrap()
                .bind_opening_points_v1(root_openings),
            Err(QPcsMaskingErrorV1::DeepPointIsNegacyclicRoot)
        ));
        let mut plus_q = test_openings_v1();
        plus_q[0].r = TEST_MODULUS_V1;
        assert!(matches!(
            make_pending()
                .seal_masked_rows_root_v1([0x91; 32])
                .unwrap()
                .bind_opening_points_v1(plus_q),
            Err(QPcsMaskingErrorV1::NonCanonicalResidue)
        ));
        let mut reused = test_openings_v1();
        reused[1].r = reused[0].r;
        assert!(matches!(
            make_pending()
                .seal_masked_rows_root_v1([0x91; 32])
                .unwrap()
                .bind_opening_points_v1(reused),
            Err(QPcsMaskingErrorV1::ReusedChallengeOrDomain)
        ));
        let mut reordered = test_openings_v1();
        reordered[1].repetition = 0;
        assert!(matches!(
            make_pending()
                .seal_masked_rows_root_v1([0x91; 32])
                .unwrap()
                .bind_opening_points_v1(reordered),
            Err(QPcsMaskingErrorV1::InvalidPublicBinding)
        ));
        let mut duplicate_frame = test_openings_v1();
        duplicate_frame[1].opening_transcript_digest = duplicate_frame[0].opening_transcript_digest;
        assert!(matches!(
            make_pending()
                .seal_masked_rows_root_v1([0x91; 32])
                .unwrap()
                .bind_opening_points_v1(duplicate_frame),
            Err(QPcsMaskingErrorV1::InvalidPublicBinding)
        ));
        assert!(matches!(
            make_pending().seal_masked_rows_root_v1([0; 32]),
            Err(QPcsMaskingErrorV1::InvalidPublicBinding)
        ));
        let left = make_pending()
            .seal_masked_rows_root_v1([0xa1; 32])
            .unwrap()
            .bind_opening_points_v1(test_openings_v1())
            .unwrap();
        let right = make_pending()
            .seal_masked_rows_root_v1([0xa2; 32])
            .unwrap()
            .bind_opening_points_v1(test_openings_v1())
            .unwrap();
        assert_ne!(left.postcommit_order_digest, right.postcommit_order_digest);
    }

    #[test]
    fn zeroizing_polynomial_owners_cover_success_error_and_unwind() {
        let (products, quotients) = test_relation_owners_v1();

        reset_zeroizing_drop_count_v1();
        let masked = mask_one_limb_five_repetitions_v1(
            TEST_MODULUS_V1,
            TEST_RING_DEGREE_V1,
            borrowed_pairs_v1(&products, &quotients),
            test_context_v1(),
            &mut DomainAwareSamplerV1::default(),
        )
        .unwrap();
        assert_eq!(zeroizing_drop_count_v1(), 5);
        drop(masked);
        assert_eq!(zeroizing_drop_count_v1(), 15);

        reset_zeroizing_drop_count_v1();
        let pending = mask_one_limb_five_repetitions_v1(
            TEST_MODULUS_V1,
            TEST_RING_DEGREE_V1,
            borrowed_pairs_v1(&products, &quotients),
            test_context_v1(),
            &mut DomainAwareSamplerV1::default(),
        )
        .unwrap();
        assert_eq!(zeroizing_drop_count_v1(), 5);
        assert!(matches!(
            pending.seal_masked_rows_root_v1([0; 32]),
            Err(QPcsMaskingErrorV1::InvalidPublicBinding)
        ));
        assert_eq!(zeroizing_drop_count_v1(), 15);

        reset_zeroizing_drop_count_v1();
        let error = mask_one_limb_five_repetitions_v1(
            TEST_MODULUS_V1,
            TEST_RING_DEGREE_V1,
            borrowed_pairs_v1(&products, &quotients),
            test_context_v1(),
            &mut FailOrPanicSamplerV1 {
                calls: 0,
                fail_at: 2,
                panic_instead: false,
            },
        );
        assert!(matches!(error, Err(QPcsMaskingErrorV1::RandomUnavailable)));
        assert_eq!(zeroizing_drop_count_v1(), 7);

        reset_zeroizing_drop_count_v1();
        let unwind = catch_unwind(AssertUnwindSafe(|| {
            let _ = mask_one_limb_five_repetitions_v1(
                TEST_MODULUS_V1,
                TEST_RING_DEGREE_V1,
                borrowed_pairs_v1(&products, &quotients),
                test_context_v1(),
                &mut FailOrPanicSamplerV1 {
                    calls: 0,
                    fail_at: 2,
                    panic_instead: true,
                },
            );
        }));
        assert!(unwind.is_err());
        assert_eq!(zeroizing_drop_count_v1(), 7);
    }

    #[test]
    fn release_resource_accounting_is_exact_but_work_is_a_non_authorizing_lower_bound() {
        let plan = q_pcs_five_repetition_masking_accounting_v1().unwrap();
        assert_eq!(plan.repetition_count, 5);
        assert_eq!(plan.masked_row_count, 10);
        assert_eq!(plan.current_fri_batch_rows, 2);
        assert_eq!(plan.masked_cross_limb_leaf_bytes, 6_080);
        assert_eq!(plan.fri_cross_limb_leaf_bytes, 1_216);
        assert_eq!(plan.initial_opened_leaves, 320);
        assert_eq!(plan.fri_opened_leaves, 4_028);
        assert_eq!(plan.authentication_hashes, 26_496);
        assert_eq!(plan.fixed_envelope_bytes, 6_752);
        assert_eq!(plan.maximum_encoded_proof_bytes, 9_643_872);
        assert_eq!(plan.remaining_global_proof_budget_bytes, 23_910_560);
        assert_eq!(plan.isolated_kernel_heap_bytes, 74_662_064);
        assert_eq!(plan.mask_construction_peak_bytes, 20_971_400);
        assert_eq!(plan.ten_row_lde_spool_bytes, 3_187_671_040);
        assert_eq!(plan.retained_masked_aggregate_bytes, 597_685_280);
        assert_eq!(plan.minimum_external_peak_bytes, 3_785_356_320);
        assert_eq!(plan.fri_current_and_next_external_bytes, 956_301_312);
        assert_eq!(plan.minimum_accepted_mask_bytes, 199_227_920);
        assert_eq!(plan.initial_leaf_hash_input_bytes, 12_750_684_160);
        assert_eq!(plan.fft_transform_count, 2_280);
        assert_eq!(plan.fft_butterflies, 11_356_078_080);
        assert_eq!(plan.merkle_hash_invocations, 8_388_552);
        assert_eq!(plan.fri_folded_row_values, 79_691_472);
        assert_eq!(plan.masking_field_updates, 273_939_150);
        assert_eq!(plan.coarse_classified_work_units, 11_718_097_254);
        assert!(!plan.source_owned_aggregate_pairs_linked);
        assert!(!plan.fiat_shamir_relation_bound);
        assert!(!plan.production_uniform_independent_sampler_integrated);
        assert!(!plan.sampler_entropy_and_work_accounted);
        assert!(!plan.ten_row_pcs_proof_integrated);
        assert!(!plan.ten_row_pcs_wiring_implemented);
        assert!(!plan.zero_knowledge_qualified);
        assert!(!plan.end_to_end_source_residency_accounted);
        assert!(!plan.masked_row_root_authenticity_verified);
        assert!(!plan.postcommit_opening_points_pcs_bound);
        assert!(!plan.one_point_opening_quotients_implemented);
        assert!(!plan.complete_work_bound_derived);
    }

    #[test]
    fn source_guards_keep_masks_private_unwired_and_every_release_axis_false() {
        let source = include_str!("phase23_rns_link_q_pcs_masking.rs");
        let parent = include_str!("phase23_rns_link_q_pcs.rs");
        let production = source
            .split("#[cfg(test)]\nmod tests")
            .next()
            .expect("production source prefix");
        assert!(source.lines().count() <= 1_700);
        assert!(source.len() <= 70_000);
        assert!(!production.contains("pub(super)"));
        assert!(!production.contains("pub(crate)"));
        assert!(!production.contains("pub struct"));
        for forbidden in [
            "SecretPolynomial",
            "RnsPolynomial",
            "state_owned",
            "q_relation_adapter",
            "receipt_capability_audit",
            "wire::",
            "mask_digest",
            "source_relation_digest",
            "impl Clone for ZeroizingQPolynomialV1",
            "fn into_inner",
        ] {
            assert!(
                !production.contains(forbidden),
                "forbidden masking surface: {forbidden}"
            );
        }
        for required in [
            "S` is private sampler entropy, never a Fiat-Shamir/public derivation",
            "The existing PCS still mixes only two FRI rows",
            "SOURCE_OWNED_AGGREGATE_PAIRS_LINKED_V1: bool = false",
            "FIAT_SHAMIR_RELATION_BOUND_V1: bool = false",
            "PRODUCTION_UNIFORM_INDEPENDENT_SAMPLER_INTEGRATED_V1: bool = false",
            "SAMPLER_ENTROPY_AND_WORK_ACCOUNTED_V1: bool = false",
            "TEN_ROW_PCS_PROOF_INTEGRATED_V1: bool = false",
            "TEN_ROW_PCS_WIRING_IMPLEMENTED_V1: bool = false",
            "ZERO_KNOWLEDGE_QUALIFIED_V1: bool = false",
            "END_TO_END_SOURCE_RESIDENCY_ACCOUNTED_V1: bool = false",
            "MASKED_ROW_ROOT_AUTHENTICITY_VERIFIED_V1: bool = false",
            "POSTCOMMIT_OPENING_POINTS_PCS_BOUND_V1: bool = false",
            "ONE_POINT_OPENING_QUOTIENTS_IMPLEMENTED_V1: bool = false",
            "COMPLETE_WORK_BOUND_DERIVED_V1: bool = false",
            "prior.as_slice() == mask.as_slice()",
            "verify_preserved_residual_v1(",
            "original != 0 || masked != 0",
            "nonempty_u64_slices_overlap_v1(pair.product, pair.quotient)?",
            "for prior_slice in [prior.product, prior.quotient]",
            "All ten borrowed coefficient ranges must be disjoint",
            "struct RootSealedMaskedRowsV1",
            "fn seal_masked_rows_root_v1(",
            "impl RootSealedMaskedRowsV1",
            "all ten rows must be masked and committed before `r` is derived",
            "one-point QP(2N-2), and one-point QH(N-2)",
            "not a total-work maximum",
            "arithmetic/register/compiler temporaries and panic-abort are outside",
        ] {
            assert!(
                production.contains(required),
                "missing fail-closed pin: {required}"
            );
        }
        let mask_domain = production
            .split("struct PrototypeMaskDomainV1")
            .nth(1)
            .and_then(|suffix| suffix.split("impl PrototypeMaskDomainV1").next())
            .expect("mask-domain type body");
        assert!(!mask_domain.contains("\n    r: u64"));
        let postcommit = production
            .split("fn bind_opening_points_v1")
            .nth(1)
            .expect("postcommit transition");
        let postcommit_signature = postcommit.split('{').next().expect("postcommit signature");
        assert!(!postcommit_signature.contains("masked_rows_root"));
        let root_offset = postcommit
            .find("frame.extend_from_slice(&self.masked_rows_root)")
            .expect("root transcript absorption");
        let r_offset = postcommit
            .find("frame.extend_from_slice(&opening.r.to_be_bytes())")
            .expect("postcommit r absorption");
        assert!(root_offset < r_offset);
        assert!(parent.contains(
            "#[cfg(test)]\n#[path = \"phase23_rns_link_q_pcs_masking.rs\"]\nmod masking;"
        ));
        assert!(!parent.contains(concat!("pub use ", "masking")));
        for false_gate in [
            "fiat_shamir_relation_adapter_implemented: false",
            "zero_knowledge_masking_implemented: false",
            "deterministic_plaintext_lineage_hiding_implemented: false",
            "secret_packed_plaintext_owner_hardened: false",
            "release_qualified: false",
        ] {
            assert!(
                parent.contains(false_gate),
                "parent gate changed: {false_gate}"
            );
        }
    }
}
