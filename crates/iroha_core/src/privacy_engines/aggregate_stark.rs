//! Shared aggregate SHA-256/Goldilocks STARK commitment and opening core.
//!
//! Relation modules supply their exact transcript suite, profile digest,
//! public-input digest, trace columns, constraint-composition values, and an
//! [`AggregateOpenedRowEvaluatorV1`]. This module owns the canonical ordered
//! trace-group layout, exact proof codec, SHA-256 vector-row commitments,
//! minimal batched Merkle multiproofs, shared binary FRI, and opened-query
//! verification. It deliberately contains no X.509, private-note, or PQ-MASP
//! policy.

use std::collections::{BTreeMap, BTreeSet};
#[cfg(test)]
use std::io::{Read as _, Seek as _, Write as _};
#[cfg(all(test, target_os = "linux"))]
use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};

#[cfg(test)]
use chacha20poly1305::{
    XChaCha20Poly1305,
    aead::{Aead as _, KeyInit as _, Payload},
};
use rand::TryRngCore;
#[cfg(any(test, feature = "privacy-release-evidence"))]
use rayon::prelude::*;
#[cfg(all(test, target_os = "linux"))]
use rustix::fs::{MemfdFlags, SealFlags, fcntl_get_seals, memfd_create};
#[cfg(any(test, feature = "privacy-release-evidence"))]
use sha2::{Digest as _, Sha256};
use thiserror::Error;
#[cfg(test)]
use zeroize::Zeroizing;

use super::transparent_stark::{
    ExactProofReaderV1, GOLDILOCKS_GENERATOR_V1, GoldilocksFieldV1 as F, GoldilocksFp4V1 as E,
    Sha256MerkleTreeV1, TransparentStarkErrorV1, TransparentTranscriptV1, append_u16_v1,
    append_u32_v1, append_u64_v1, derive_unique_query_indices_v1,
    ensure_fri_terminal_degree_fp4_v1, fri_fold_pair_fp4_v1, fri_fold_pair_with_inverse_x_fp4_v1,
    goldilocks_fp4_evaluate_coset_v1, goldilocks_fp4_ifft_v1, goldilocks_primitive_root_v1,
    random_goldilocks_fp4_v1, sha256_frame_v1, sha256_merkle_node_v1,
};
#[cfg(test)]
use super::transparent_stark::{
    ReplayableTraceMaskV1, goldilocks_batch_invert_v1, masked_trace_lde_column_with_mask_v1,
};
#[cfg(any(test, feature = "privacy-release-evidence"))]
use super::transparent_stark::{
    TRANSCRIPT_FRAME_DOMAIN_V1, goldilocks_ifft_v1, masked_trace_coefficients_on_coset_v1,
    masked_trace_coefficients_with_mask_v1, sample_trace_mask_v1,
};

const FRI_MASK_LEAF_DOMAIN_V1: &[u8] = b"iroha:privacy:aggregate-stark:fri-mask-oracle-leaf:v1";
const FRI_MASK_NODE_DOMAIN_V1: &[u8] = b"iroha:privacy:aggregate-stark:fri-mask-oracle-node:v1";
const FRI_MASK_ROOT_LABEL_V1: &[u8] = b"iroha:privacy:aggregate-stark:fri-mask-oracle-root:v1";
const DEEP_POINT_LABEL_V1: &[u8] = b"iroha:privacy:aggregate-stark:deep-point:v1";
const DEEP_OPENINGS_LABEL_V1: &[u8] = b"iroha:privacy:aggregate-stark:deep-openings:v1";
/// Exact maximum number of independent masked-trace LDE columns retained
/// concurrently by one deterministic parallel batch.
pub(crate) const MASKED_TRACE_LDE_COLUMN_BATCH_V1: usize = 8;
/// Shared aggregate-STARK failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub(crate) enum AggregateStarkErrorV1 {
    /// A verifier-derived protocol parameter or ordered layout is invalid.
    #[error("aggregate STARK layout is invalid")]
    InvalidLayout,
    /// A proof has the wrong exact statement-derived shape.
    #[error("aggregate STARK proof shape is invalid")]
    InvalidProofShape,
    /// Proof bytes are empty, truncated, trailing, or otherwise malformed.
    #[error("aggregate STARK proof wire is malformed")]
    MalformedProof,
    /// Proof bytes exceed the engine-fixed ceiling.
    #[error("aggregate STARK proof exceeds its byte ceiling")]
    ProofTooLarge,
    /// A proof field is not a canonical Goldilocks residue.
    #[error("aggregate STARK proof contains a non-canonical field")]
    NonCanonicalField,
    /// A committed trace opening is invalid.
    #[error("aggregate STARK trace opening is invalid")]
    TraceOpening,
    /// A composition opening or relation callback result is invalid.
    #[error("aggregate STARK composition opening is invalid")]
    ConstraintOpening,
    /// An out-of-domain point or DEEP opening/quotient identity is invalid.
    #[error("aggregate STARK DEEP opening is invalid")]
    DeepOpening,
    /// A FRI opening or fold is invalid.
    #[error("aggregate STARK FRI opening is invalid")]
    FriOpening,
    /// A FRI terminal violates its degree bound.
    #[error("aggregate STARK FRI terminal degree is invalid")]
    FriDegree,
    /// Transcript order or derived query positions do not match.
    #[error("aggregate STARK transcript is invalid")]
    TranscriptMismatch,
    /// A bounded allocation failed.
    #[error("aggregate STARK bounded allocation failed")]
    AllocationFailure,
    /// Required prover randomness was unavailable or failed its health checks.
    #[error("aggregate STARK prover randomness is unavailable")]
    RandomnessUnavailable,
    /// A checked implementation invariant failed.
    #[error("aggregate STARK internal invariant failed")]
    InternalInvariant,
}

fn map_transparent_error_v1(error: TransparentStarkErrorV1) -> AggregateStarkErrorV1 {
    match error {
        TransparentStarkErrorV1::RandomnessUnavailable => {
            AggregateStarkErrorV1::RandomnessUnavailable
        }
        TransparentStarkErrorV1::AllocationFailure => AggregateStarkErrorV1::AllocationFailure,
        TransparentStarkErrorV1::NonCanonicalField => AggregateStarkErrorV1::NonCanonicalField,
        TransparentStarkErrorV1::FriDegree => AggregateStarkErrorV1::FriDegree,
        TransparentStarkErrorV1::MalformedProof => AggregateStarkErrorV1::MalformedProof,
        TransparentStarkErrorV1::InvalidMerkleShape => AggregateStarkErrorV1::TraceOpening,
        TransparentStarkErrorV1::ChallengeSamplingExhausted
        | TransparentStarkErrorV1::QuerySamplingExhausted
        | TransparentStarkErrorV1::InvalidGrinding => AggregateStarkErrorV1::TranscriptMismatch,
        _ => AggregateStarkErrorV1::InternalInvariant,
    }
}

/// Immutable proof-system dimensions used by the generic aggregate core.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct AggregateStarkParametersV1 {
    /// Four-byte exact wire magic.
    pub(crate) proof_magic: [u8; 4],
    /// Exact proof version.
    pub(crate) proof_version: u16,
    /// Number of independent composition/FRI lanes.
    pub(crate) security_lanes: usize,
    /// Number of unique post-grinding queries.
    pub(crate) query_count: usize,
    /// Binary logarithm of the trace-to-LDE blow-up.
    pub(crate) blowup_log2: u8,
    /// Binary logarithm of the terminal FRI vector length.
    pub(crate) terminal_log2: u8,
    /// Maximum degree of the terminal polynomial.
    pub(crate) terminal_degree_bound: usize,
    /// Number of low-degree coefficient chunks per composition lane.
    pub(crate) composition_degree_chunks: usize,
    /// Minimum native trace logarithm.
    pub(crate) minimum_trace_log2: u8,
    /// Maximum native trace logarithm.
    pub(crate) maximum_trace_log2: u8,
    /// Maximum ordered trace-group count.
    pub(crate) maximum_trace_groups: usize,
    /// Maximum total segment instances.
    pub(crate) maximum_segment_instances: usize,
    /// Maximum base columns per segment instance.
    pub(crate) maximum_base_columns_per_instance: usize,
    /// Maximum auxiliary columns per segment instance.
    pub(crate) maximum_aux_columns_per_instance: usize,
    /// Exact proof-byte ceiling.
    pub(crate) maximum_proof_bytes: usize,
}

impl AggregateStarkParametersV1 {
    /// Terminal vector length.
    pub(crate) fn terminal_size(self) -> Result<usize, AggregateStarkErrorV1> {
        1_usize
            .checked_shl(u32::from(self.terminal_log2))
            .ok_or(AggregateStarkErrorV1::InvalidLayout)
    }

    /// Exact trace-hiding dimension for all current/next query openings.
    ///
    /// Every query can expose two distinct positions in each trace group, so
    /// the mask polynomial needs two independent coefficients per query.
    pub(crate) fn trace_hiding_coefficients(self) -> Result<usize, AggregateStarkErrorV1> {
        self.query_count
            .checked_mul(2)
            .ok_or(AggregateStarkErrorV1::InvalidLayout)
    }

    /// Validate every closed proof-system dimension.
    pub(crate) fn validate(self) -> Result<(), AggregateStarkErrorV1> {
        let terminal_size = self.terminal_size()?;
        let maximum_lde_log2 = self
            .maximum_trace_log2
            .checked_add(self.blowup_log2)
            .ok_or(AggregateStarkErrorV1::InvalidLayout)?;
        if self.proof_magic == [0; 4]
            || self.proof_version == 0
            || self.security_lanes == 0
            || self.query_count == 0
            || self.blowup_log2 == 0
            || self.terminal_degree_bound >= terminal_size
            || self.composition_degree_chunks == 0
            || self.composition_degree_chunks > usize::from(u16::MAX)
            || maximum_lde_log2 > u32::BITS as u8
            || maximum_lde_log2 >= usize::BITS as u8
            || self.minimum_trace_log2 > self.maximum_trace_log2
            || self.security_lanes > usize::from(u16::MAX)
            || self.maximum_trace_groups == 0
            || self.maximum_trace_groups > usize::from(u16::MAX)
            || self.maximum_segment_instances == 0
            || self.maximum_base_columns_per_instance == 0
            || self.maximum_aux_columns_per_instance == 0
            || self.maximum_proof_bytes == 0
        {
            return Err(AggregateStarkErrorV1::InvalidLayout);
        }
        Ok(())
    }
}

/// SHA-256 and transcript domains supplied by one relation profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct AggregateStarkDomainsV1 {
    /// Base-row leaf domain.
    pub(crate) base_leaf: &'static [u8],
    /// Base-tree node domain.
    pub(crate) base_node: &'static [u8],
    /// Auxiliary-row leaf domain.
    pub(crate) aux_leaf: &'static [u8],
    /// Auxiliary-tree node domain.
    pub(crate) aux_node: &'static [u8],
    /// Composition leaf domain.
    pub(crate) composition_leaf: &'static [u8],
    /// Composition-tree node domain.
    pub(crate) composition_node: &'static [u8],
    /// FRI leaf domain.
    pub(crate) fri_leaf: &'static [u8],
    /// FRI-tree node domain.
    pub(crate) fri_node: &'static [u8],
    /// Transcript label for the ordered layout.
    pub(crate) layout_label: &'static [u8],
    /// Transcript label for base roots.
    pub(crate) base_root_label: &'static [u8],
    /// Transcript label for auxiliary roots.
    pub(crate) aux_root_label: &'static [u8],
    /// Transcript label for composition roots.
    pub(crate) composition_root_label: &'static [u8],
    /// Transcript label for FRI roots.
    pub(crate) fri_root_label: &'static [u8],
    /// Transcript challenge label for FRI folds.
    pub(crate) fri_beta_label: &'static [u8],
    /// Query-seed frame domain.
    pub(crate) query_seed: &'static [u8],
}

impl AggregateStarkDomainsV1 {
    /// Reject missing or duplicate cryptographic domains and labels.
    pub(crate) fn validate(self) -> Result<(), AggregateStarkErrorV1> {
        let values = [
            DEEP_POINT_LABEL_V1,
            DEEP_OPENINGS_LABEL_V1,
            self.base_leaf,
            self.base_node,
            self.aux_leaf,
            self.aux_node,
            self.composition_leaf,
            self.composition_node,
            self.fri_leaf,
            self.fri_node,
            self.layout_label,
            self.base_root_label,
            self.aux_root_label,
            self.composition_root_label,
            self.fri_root_label,
            self.fri_beta_label,
            self.query_seed,
        ];
        if values.iter().any(|value| value.is_empty()) {
            return Err(AggregateStarkErrorV1::InvalidLayout);
        }
        for (index, value) in values.iter().enumerate() {
            if values[..index].contains(value) {
                return Err(AggregateStarkErrorV1::InvalidLayout);
            }
        }
        Ok(())
    }
}

/// One canonical group of equal-native-stride trace segments.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct AggregateTraceGroupLayoutV1 {
    /// Native trace-size logarithm shared by every segment in this group.
    pub(crate) native_trace_log2: u8,
    /// Number of segment instances concatenated into each vector row.
    pub(crate) segment_instances: usize,
    /// Total committed base width across the group.
    pub(crate) base_width: usize,
    /// Total committed auxiliary width across the group.
    pub(crate) aux_width: usize,
}

impl AggregateTraceGroupLayoutV1 {
    /// Derive the sole legal next-row stride on a common LDE domain.
    pub(crate) fn next_stride(self, common_lde_log2: u8) -> Result<usize, AggregateStarkErrorV1> {
        let shift = common_lde_log2
            .checked_sub(self.native_trace_log2)
            .ok_or(AggregateStarkErrorV1::InvalidLayout)?;
        1_usize
            .checked_shl(u32::from(shift))
            .ok_or(AggregateStarkErrorV1::InvalidLayout)
    }
}

/// Verifier-derived ordered aggregate commitment layout.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AggregateProofLayoutV1 {
    common_lde_log2: u8,
    trace_groups: Vec<AggregateTraceGroupLayoutV1>,
}

impl AggregateProofLayoutV1 {
    /// Construct from canonically ordered, non-empty group descriptors.
    ///
    /// Native trace logarithms are nondecreasing. Multiple independently
    /// committed adapters may use the same native domain; their verifier-fixed
    /// order is preserved in the transcript and proof vectors.
    pub(crate) fn new(
        parameters: AggregateStarkParametersV1,
        trace_groups: Vec<AggregateTraceGroupLayoutV1>,
    ) -> Result<Self, AggregateStarkErrorV1> {
        parameters.validate()?;
        let maximum_native_log2 = trace_groups
            .last()
            .ok_or(AggregateStarkErrorV1::InvalidLayout)?
            .native_trace_log2;
        let common_lde_log2 = maximum_native_log2
            .checked_add(parameters.blowup_log2)
            .ok_or(AggregateStarkErrorV1::InvalidLayout)?;
        let layout = Self {
            common_lde_log2,
            trace_groups,
        };
        layout.validate(parameters)?;
        Ok(layout)
    }

    /// Common LDE logarithm.
    pub(crate) const fn common_lde_log2(&self) -> u8 {
        self.common_lde_log2
    }

    /// Common LDE row count.
    pub(crate) fn common_lde_size(&self) -> usize {
        1_usize
            .checked_shl(u32::from(self.common_lde_log2))
            .unwrap_or(0)
    }

    /// Ordered trace-group descriptors.
    pub(crate) fn trace_groups(&self) -> &[AggregateTraceGroupLayoutV1] {
        &self.trace_groups
    }

    /// Number of binary FRI rounds.
    pub(crate) fn fri_rounds(
        &self,
        parameters: AggregateStarkParametersV1,
    ) -> Result<usize, AggregateStarkErrorV1> {
        self.common_lde_log2
            .checked_sub(parameters.terminal_log2)
            .map(usize::from)
            .ok_or(AggregateStarkErrorV1::InvalidLayout)
    }

    /// Maximum polynomial degree accepted by the terminal FRI check.
    pub(crate) fn maximum_fri_input_degree(
        &self,
        parameters: AggregateStarkParametersV1,
    ) -> Result<usize, AggregateStarkErrorV1> {
        let fold_factor = 1_usize
            .checked_shl(
                u32::try_from(self.fri_rounds(parameters)?)
                    .map_err(|_| AggregateStarkErrorV1::InvalidLayout)?,
            )
            .ok_or(AggregateStarkErrorV1::InvalidLayout)?;
        parameters
            .terminal_degree_bound
            .checked_add(1)
            .and_then(|coefficients| coefficients.checked_mul(fold_factor))
            .and_then(|coefficients| coefficients.checked_sub(1))
            .ok_or(AggregateStarkErrorV1::InvalidLayout)
    }

    /// Exclusive degree cap shared by traces, composition chunks, and FRI masks.
    pub(crate) fn fri_degree_cap(
        &self,
        parameters: AggregateStarkParametersV1,
    ) -> Result<usize, AggregateStarkErrorV1> {
        self.maximum_fri_input_degree(parameters)?
            .checked_add(1)
            .ok_or(AggregateStarkErrorV1::InvalidLayout)
    }

    /// Maximum unsplit composition degree represented by all coefficient chunks.
    pub(crate) fn maximum_composition_degree(
        &self,
        parameters: AggregateStarkParametersV1,
    ) -> Result<usize, AggregateStarkErrorV1> {
        self.fri_degree_cap(parameters)?
            .checked_mul(parameters.composition_degree_chunks)
            .and_then(|coefficients| coefficients.checked_sub(1))
            .ok_or(AggregateStarkErrorV1::InvalidLayout)
    }

    /// Exact coefficient count of each independent Protocol-2 FRI mask.
    ///
    /// The paper's minimum hiding space is
    /// `F[X]^{< |H| + h - 1}`. This implementation samples from the larger
    /// normalized space `F[X]^{< D - 1}`, where `D` is the one exclusive
    /// degree cap enforced by the shared FRI. One top-degree slot remains
    /// unused, matching the protocol's strict inequality and preventing an
    /// off-by-one reinterpretation of the terminal bound.
    pub(crate) fn fri_mask_coefficient_count(
        &self,
        parameters: AggregateStarkParametersV1,
    ) -> Result<usize, AggregateStarkErrorV1> {
        self.maximum_fri_input_degree(parameters)
    }

    fn minimum_protocol_fri_mask_coefficients(
        &self,
        parameters: AggregateStarkParametersV1,
    ) -> Result<usize, AggregateStarkErrorV1> {
        let largest_native_rows = self
            .common_lde_size()
            .checked_shr(u32::from(parameters.blowup_log2))
            .ok_or(AggregateStarkErrorV1::InvalidLayout)?;
        largest_native_rows
            .checked_add(parameters.trace_hiding_coefficients()?)
            .and_then(|count| count.checked_sub(1))
            .ok_or(AggregateStarkErrorV1::InvalidLayout)
    }

    /// Validate ordering, widths, instance count, common domain, and strides.
    pub(crate) fn validate(
        &self,
        parameters: AggregateStarkParametersV1,
    ) -> Result<(), AggregateStarkErrorV1> {
        parameters.validate()?;
        let fri_mask_coefficients = self.fri_mask_coefficient_count(parameters)?;
        let minimum_fri_mask_coefficients =
            self.minimum_protocol_fri_mask_coefficients(parameters)?;
        let maximum_fri_input_degree = self.maximum_fri_input_degree(parameters)?;
        if self.trace_groups.is_empty()
            || self.trace_groups.len() > parameters.maximum_trace_groups
            || self.trace_groups.len() > usize::from(u16::MAX)
            || self.common_lde_size() < parameters.query_count
            || (self.common_lde_size() >> self.fri_rounds(parameters)?)
                != parameters.terminal_size()?
            || fri_mask_coefficients == 0
            || fri_mask_coefficients < minimum_fri_mask_coefficients
            || fri_mask_coefficients
                .checked_sub(1)
                .is_none_or(|degree| degree > maximum_fri_input_degree)
        {
            return Err(AggregateStarkErrorV1::InvalidLayout);
        }
        let mut previous_log2 = None;
        let mut total_instances = 0_usize;
        for group in &self.trace_groups {
            let maximum_base = group
                .segment_instances
                .checked_mul(parameters.maximum_base_columns_per_instance)
                .ok_or(AggregateStarkErrorV1::InvalidLayout)?;
            let maximum_aux = group
                .segment_instances
                .checked_mul(parameters.maximum_aux_columns_per_instance)
                .ok_or(AggregateStarkErrorV1::InvalidLayout)?;
            if group.native_trace_log2 < parameters.minimum_trace_log2
                || group.native_trace_log2 > parameters.maximum_trace_log2
                || previous_log2.is_some_and(|previous| previous > group.native_trace_log2)
                || group.segment_instances == 0
                || group.segment_instances > usize::from(u16::MAX)
                || group.base_width == 0
                || group.aux_width == 0
                || group.base_width > usize::from(u16::MAX)
                || group.aux_width > usize::from(u16::MAX)
                || group.base_width > maximum_base
                || group.aux_width > maximum_aux
                || group.next_stride(self.common_lde_log2)? == 0
            {
                return Err(AggregateStarkErrorV1::InvalidLayout);
            }
            total_instances = total_instances
                .checked_add(group.segment_instances)
                .ok_or(AggregateStarkErrorV1::InvalidLayout)?;
            previous_log2 = Some(group.native_trace_log2);
        }
        let expected_common = self
            .trace_groups
            .last()
            .ok_or(AggregateStarkErrorV1::InvalidLayout)?
            .native_trace_log2
            .checked_add(parameters.blowup_log2)
            .ok_or(AggregateStarkErrorV1::InvalidLayout)?;
        if total_instances > parameters.maximum_segment_instances
            || self.common_lde_log2 != expected_common
        {
            return Err(AggregateStarkErrorV1::InvalidLayout);
        }
        Ok(())
    }
}

/// Exact release certificate for the affine-batched binary-FRI theorem.
///
/// `l_minus_one_*` represents the theorem's rational `L - 1`. `rho_*`
/// represents the exact code rate, and `affine_arities` is the complete list
/// whose sum appears in the commitment-error term. The remaining fields bind
/// the smooth domain, Fp4 field-size lower bound, fold/terminal geometry, and
/// the implementation's distinct-query schedule.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct AggregateFriTheorem2CertificateV1 {
    /// Numerator of `L - 1`.
    pub(crate) l_minus_one_numerator: u8,
    /// Denominator of `L - 1`.
    pub(crate) l_minus_one_denominator: u8,
    /// Affine batching parameter `m`.
    pub(crate) batching_parameter_m: u8,
    /// Exact rate numerator.
    pub(crate) rho_numerator: u8,
    /// Exact rate denominator.
    pub(crate) rho_denominator: u8,
    /// Affine arities whose sum occurs in the theorem.
    pub(crate) affine_arities: [u8; 3],
    /// Binary logarithm of the evaluation domain `|D|`.
    pub(crate) domain_log2: u8,
    /// Proven lower-bound exponent for `|F_{p^4}|`.
    pub(crate) extension_field_lower_bound_bits: u16,
    /// Binary two-adicity available to the base-field FFT domain.
    pub(crate) base_field_two_adicity: u8,
    /// Every native trace domain is a power-of-two multiplicative subgroup.
    pub(crate) trace_domains_are_smooth_subgroups: bool,
    /// The evaluation domain is the fixed primitive-generator coset of a
    /// power-of-two subgroup.
    pub(crate) evaluation_domain_is_smooth_generator_coset: bool,
    /// The evaluation coset is disjoint from every native trace subgroup.
    pub(crate) evaluation_domain_is_disjoint_from_trace_domains: bool,
    /// Exact binary fold count.
    pub(crate) fold_count: u8,
    /// Exact terminal-domain logarithm.
    pub(crate) terminal_log2: u8,
    /// Exact terminal polynomial degree bound.
    pub(crate) terminal_degree_bound: u16,
    /// Exact number of query invocations.
    pub(crate) query_count: u8,
    /// Query sampler draws distinct indices without replacement.
    pub(crate) distinct_queries_without_replacement: bool,
    /// Rejection sampling is uniform over every as-yet-unselected index.
    pub(crate) uniform_rejection_sampling: bool,
    /// Claimed floor for the query-error exponent.
    pub(crate) claimed_query_error_bits: u16,
}

/// Machine-checked error exponents for one FRI subproof.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct AggregateFriTheorem2BoundV1 {
    /// Query error is strictly below `2^-query_error_bits`.
    pub(crate) query_error_bits: u16,
    /// Sum of both commitment-error terms is below
    /// `2^-commitment_error_bits`.
    pub(crate) commitment_error_bits: u16,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct U256V1([u64; 4]);

impl U256V1 {
    fn one() -> Self {
        Self([1, 0, 0, 0])
    }

    fn checked_mul_small(mut self, multiplier: u64) -> Option<Self> {
        let mut carry = 0_u128;
        for limb in &mut self.0 {
            let product = u128::from(*limb)
                .checked_mul(u128::from(multiplier))?
                .checked_add(carry)?;
            *limb = product as u64;
            carry = product >> 64;
        }
        (carry == 0).then_some(self)
    }

    fn checked_pow_small(base: u64, exponent: u8) -> Option<Self> {
        (0..exponent).try_fold(Self::one(), |value, _| value.checked_mul_small(base))
    }

    fn checked_shl(self, shift: u16) -> Option<Self> {
        if shift >= 256 {
            return None;
        }
        let word_shift = usize::from(shift / 64);
        let bit_shift = u32::from(shift % 64);
        let mut shifted = [0_u64; 4];
        for (source, limb) in self.0.into_iter().enumerate() {
            if limb == 0 {
                continue;
            }
            let target = source.checked_add(word_shift)?;
            if target >= shifted.len() {
                return None;
            }
            shifted[target] |= limb.checked_shl(bit_shift).unwrap_or(0);
            if bit_shift != 0 {
                let high = limb >> (64 - bit_shift);
                if high != 0 {
                    let high_target = target.checked_add(1)?;
                    if high_target >= shifted.len() {
                        return None;
                    }
                    shifted[high_target] |= high;
                }
            }
        }
        Some(Self(shifted))
    }

    fn strictly_less_than(self, rhs: Self) -> bool {
        self.0
            .iter()
            .rev()
            .zip(rhs.0.iter().rev())
            .find_map(|(left, right)| (left != right).then_some(left < right))
            .unwrap_or(false)
    }
}

/// Validate every precondition and both error terms of the affine-batched
/// binary-FRI theorem used by the release.
///
/// For distinct queries, a bad set of size `b` in a domain of size `N` is hit
/// in every query with probability `(b)_q/(N)_q`. Every factor satisfies
/// `(b-i)/(N-i) <= b/N`, so sampling without replacement is no worse than the
/// theorem's with-replacement power bound. Rejection sampling therefore
/// changes neither the bound nor its exponent, provided all indices are
/// unique and `q <= N`; both are verifier-enforced.
pub(crate) fn validate_affine_batched_fri_theorem2_v1(
    parameters: AggregateStarkParametersV1,
    layout: &AggregateProofLayoutV1,
    certificate: AggregateFriTheorem2CertificateV1,
) -> Result<AggregateFriTheorem2BoundV1, AggregateStarkErrorV1> {
    layout.validate(parameters)?;
    let domain_size = layout.common_lde_size();
    let terminal_size = parameters.terminal_size()?;
    let fold_count = layout.fri_rounds(parameters)?;
    let degree_coefficients = layout.fri_degree_cap(parameters)?;
    let affine_arity_sum = certificate
        .affine_arities
        .iter()
        .try_fold(0_u16, |sum, arity| sum.checked_add(u16::from(*arity)))
        .ok_or(AggregateStarkErrorV1::InvalidLayout)?;
    if parameters.security_lanes != 1
        || certificate.l_minus_one_numerator != 3
        || certificate.l_minus_one_denominator != 2
        || certificate.batching_parameter_m != 3
        || certificate.rho_numerator != 1
        || certificate.rho_denominator != 32
        || certificate.affine_arities != [2, 2, 2]
        || affine_arity_sum != 6
        || certificate.domain_log2 != layout.common_lde_log2
        || certificate.extension_field_lower_bound_bits != 252
        || certificate.base_field_two_adicity != 32
        || layout.common_lde_log2 > certificate.base_field_two_adicity
        || !certificate.trace_domains_are_smooth_subgroups
        || !certificate.evaluation_domain_is_smooth_generator_coset
        || !certificate.evaluation_domain_is_disjoint_from_trace_domains
        || usize::from(certificate.fold_count) != fold_count
        || certificate.terminal_log2 != parameters.terminal_log2
        || usize::from(certificate.terminal_degree_bound) != parameters.terminal_degree_bound
        || usize::from(certificate.query_count) != parameters.query_count
        || !certificate.distinct_queries_without_replacement
        || !certificate.uniform_rejection_sampling
        || parameters.query_count > domain_size
        || certificate.claimed_query_error_bits == 0
        || terminal_size
            .checked_shl(u32::from(certificate.fold_count))
            .filter(|size| *size == domain_size)
            .is_none()
        || (parameters.terminal_degree_bound + 1)
            .checked_mul(usize::from(certificate.rho_denominator))
            != terminal_size.checked_mul(usize::from(certificate.rho_numerator))
        || degree_coefficients.checked_mul(usize::from(certificate.rho_denominator))
            != domain_size.checked_mul(usize::from(certificate.rho_numerator))
    {
        return Err(AggregateStarkErrorV1::InvalidLayout);
    }

    // The eta/delta regime requires
    // `sqrt(rho) * (1 + 1/(2m)) < 1`. Squaring and clearing
    // denominators gives this exact integer inequality.
    let twice_m = u32::from(certificate.batching_parameter_m) * 2;
    let eta_numerator = u32::from(certificate.rho_numerator)
        .checked_mul(
            twice_m
                .checked_add(1)
                .ok_or(AggregateStarkErrorV1::InvalidLayout)?
                .pow(2),
        )
        .ok_or(AggregateStarkErrorV1::InvalidLayout)?;
    let eta_denominator = u32::from(certificate.rho_denominator)
        .checked_mul(twice_m.pow(2))
        .ok_or(AggregateStarkErrorV1::InvalidLayout)?;
    if eta_numerator >= eta_denominator {
        return Err(AggregateStarkErrorV1::InvalidLayout);
    }

    // With rho=1/32 and m=3, an even q gives the exact rational query term
    // `(49/1152)^(q/2)`. Compare it to the claimed power of two after
    // cancelling `1152 = 9 * 2^7`; the remaining integers fit in 256 bits for
    // the release geometry.
    if certificate.query_count % 2 != 0 {
        return Err(AggregateStarkErrorV1::InvalidLayout);
    }
    let half_queries = certificate.query_count / 2;
    let denominator_shift = u16::from(half_queries)
        .checked_mul(7)
        .and_then(|bits| bits.checked_sub(certificate.claimed_query_error_bits))
        .ok_or(AggregateStarkErrorV1::InvalidLayout)?;
    let query_numerator =
        U256V1::checked_pow_small(49, half_queries).ok_or(AggregateStarkErrorV1::InvalidLayout)?;
    let query_denominator = U256V1::checked_pow_small(9, half_queries)
        .and_then(|value| value.checked_shl(denominator_shift))
        .ok_or(AggregateStarkErrorV1::InvalidLayout)?;
    if !query_numerator.strictly_less_than(query_denominator) {
        return Err(AggregateStarkErrorV1::InvalidLayout);
    }

    // Exact canonical constants conservatively bound the first commitment
    // term by `7^7 * |D|^2 / 2^252 < 2^-(252-2log|D|-20)`.
    // The second is below
    // `2^9 * 2^(log|D|+1) / 2^252`.
    let first_commitment_bits = certificate
        .extension_field_lower_bound_bits
        .checked_sub(u16::from(certificate.domain_log2) * 2)
        .and_then(|bits| bits.checked_sub(20))
        .ok_or(AggregateStarkErrorV1::InvalidLayout)?;
    let second_commitment_bits = certificate
        .extension_field_lower_bound_bits
        .checked_sub(u16::from(certificate.domain_log2))
        .and_then(|bits| bits.checked_sub(10))
        .ok_or(AggregateStarkErrorV1::InvalidLayout)?;
    let commitment_error_bits = first_commitment_bits
        .min(second_commitment_bits)
        .checked_sub(1)
        .ok_or(AggregateStarkErrorV1::InvalidLayout)?;
    Ok(AggregateFriTheorem2BoundV1 {
        query_error_bits: certificate.claimed_query_error_bits,
        commitment_error_bits,
    })
}

/// One group commitment and its two canonical multiproof frontiers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AggregateTraceGroupProofV1 {
    /// Base vector-row Merkle root.
    pub(crate) base_root: [u8; 32],
    /// Auxiliary vector-row Merkle root.
    pub(crate) aux_root: [u8; 32],
    /// Minimal base-tree multiproof frontier.
    pub(crate) base_frontier: Vec<[u8; 32]>,
    /// Minimal auxiliary-tree multiproof frontier.
    pub(crate) aux_frontier: Vec<[u8; 32]>,
}

/// One FRI round's low/high opening pair.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct AggregateFriRoundOpeningV1 {
    /// Canonical low-half quartic-extension value.
    pub(crate) low: [u64; 4],
    /// Canonical high-half quartic-extension value.
    pub(crate) high: [u64; 4],
}

/// All FRI round openings for one lane/query.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AggregateFriLaneQueryV1 {
    /// Round openings in folding order.
    pub(crate) rounds: Vec<AggregateFriRoundOpeningV1>,
}

/// Current and next vector rows for one trace group/query.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AggregateTraceGroupQueryV1 {
    /// Current base row.
    pub(crate) base_current: Vec<u64>,
    /// Next base row at the verifier-derived group stride.
    pub(crate) base_next: Vec<u64>,
    /// Current auxiliary row.
    pub(crate) aux_current: Vec<u64>,
    /// Next auxiliary row at the verifier-derived group stride.
    pub(crate) aux_next: Vec<u64>,
}

/// All opened values at one shared common-domain query index.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AggregateQueryProofV1 {
    /// Common-domain index.
    pub(crate) index: u32,
    /// Ordered trace-group openings.
    pub(crate) trace_groups: Vec<AggregateTraceGroupQueryV1>,
    /// Low-degree composition-chunk values in lane-major order.
    pub(crate) composition_values: Vec<Vec<[u64; 4]>>,
    /// Authenticated independent FRI-mask-oracle value per lane.
    pub(crate) fri_mask_values: Vec<[u64; 4]>,
    /// Shared FRI openings per lane.
    pub(crate) fri_lanes: Vec<AggregateFriLaneQueryV1>,
}

/// One complete shared FRI lane.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AggregateFriLaneProofV1 {
    /// Layer roots, including the terminal layer.
    pub(crate) roots: Vec<[u8; 32]>,
    /// Exact terminal evaluations.
    pub(crate) terminal_values: Vec<[u64; 4]>,
    /// Minimal multiproof frontier for each non-terminal layer.
    pub(crate) round_frontiers: Vec<Vec<[u8; 32]>>,
}

/// Exact aggregate proof object before/after canonical encoding.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AggregateStarkProofV1 {
    /// Exact proof version.
    pub(crate) version: u16,
    /// Ordered trace-group roots/frontiers.
    pub(crate) trace_groups: Vec<AggregateTraceGroupProofV1>,
    /// Aggregate composition roots.
    pub(crate) composition_roots: Vec<[u8; 32]>,
    /// Aggregate composition multiproof frontiers.
    pub(crate) composition_frontiers: Vec<Vec<[u8; 32]>>,
    /// Independent low-degree FRI-mask-oracle roots in lane order.
    pub(crate) fri_mask_roots: Vec<[u8; 32]>,
    /// Canonical FRI-mask-oracle multiproof frontiers in lane order.
    pub(crate) fri_mask_frontiers: Vec<Vec<[u8; 32]>>,
    /// Shared FRI lanes.
    pub(crate) fri_lanes: Vec<AggregateFriLaneProofV1>,
    /// Shared post-grinding queries.
    pub(crate) queries: Vec<AggregateQueryProofV1>,
    /// Relation-verified grinding nonce.
    pub(crate) grinding_nonce: u64,
}

/// Current and next out-of-domain evaluations for one trace group.
///
/// Each base trace polynomial is evaluated in Fp4 even though its committed
/// codeword lies in the base field. `*_next` is the same polynomial evaluated
/// at `z * omega_H`, where `omega_H` is that group's native trace generator.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AggregateDeepTraceGroupOpeningV1 {
    /// Base-column evaluations at `z`.
    pub(crate) base_current: Vec<[u64; 4]>,
    /// Base-column evaluations at `z * omega_H`.
    pub(crate) base_next: Vec<[u64; 4]>,
    /// Auxiliary-column evaluations at `z`.
    pub(crate) aux_current: Vec<[u64; 4]>,
    /// Auxiliary-column evaluations at `z * omega_H`.
    pub(crate) aux_next: Vec<[u64; 4]>,
}

/// Exact out-of-domain opening payload carried by a DEEP-enabled aggregate.
///
/// The point itself is not encoded: prover and verifier derive it uniformly
/// from the transcript after all trace/composition/mask roots are bound.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AggregateDeepProofV1 {
    /// Ordered current/next trace openings.
    pub(crate) trace_groups: Vec<AggregateDeepTraceGroupOpeningV1>,
    /// Composition-chunk evaluations in lane-major order.
    pub(crate) composition_values: Vec<Vec<[u64; 4]>>,
}

/// Canonically decoded DEEP trace openings supplied to relation verifiers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AggregateOpenedDeepTraceGroupV1 {
    /// Base-column evaluations at `z`.
    pub(crate) base_current: Vec<E>,
    /// Base-column evaluations at `z * omega_H`.
    pub(crate) base_next: Vec<E>,
    /// Auxiliary-column evaluations at `z`.
    pub(crate) aux_current: Vec<E>,
    /// Auxiliary-column evaluations at `z * omega_H`.
    pub(crate) aux_next: Vec<E>,
}

/// DEEP batching coefficients for one ordered trace group.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AggregateDeepTraceGroupMixV1 {
    /// Coefficients for `(base(x) - base(z)) / (x - z)`.
    pub(crate) base_current: Vec<E>,
    /// Coefficients for `(base(x) - base(omega_H z)) / (x - omega_H z)`.
    pub(crate) base_next: Vec<E>,
    /// Coefficients for `(aux(x) - aux(z)) / (x - z)`.
    pub(crate) aux_current: Vec<E>,
    /// Coefficients for `(aux(x) - aux(omega_H z)) / (x - omega_H z)`.
    pub(crate) aux_next: Vec<E>,
}

/// Complete DEEP batching coefficients for one FRI lane.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AggregateDeepLaneMixV1 {
    /// Ordered trace-group mixes.
    pub(crate) trace_groups: Vec<AggregateDeepTraceGroupMixV1>,
    /// Composition-chunk mixes.
    pub(crate) composition: Vec<E>,
}

/// Canonical opened fields passed to a relation evaluator.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AggregateOpenedTraceGroupV1 {
    /// Current base row.
    pub(crate) base_current: Vec<F>,
    /// Next base row.
    pub(crate) base_next: Vec<F>,
    /// Current auxiliary row.
    pub(crate) aux_current: Vec<F>,
    /// Next auxiliary row.
    pub(crate) aux_next: Vec<F>,
}

/// Relation-evaluated values that the shared verifier binds to composition/FRI.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct AggregateExpectedOpeningV1 {
    /// Expected aggregate composition value.
    pub(crate) composition: E,
    /// Expected base value for the selected FRI lane.
    pub(crate) fri_base: E,
}

/// Relation-specific opened-row algebra called by the shared verifier.
pub(crate) trait AggregateOpenedRowEvaluatorV1 {
    /// Evaluate one query/lane after canonical field decoding.
    fn evaluate_opened_row_v1(
        &mut self,
        query_index: usize,
        lane: usize,
        trace_groups: &[AggregateOpenedTraceGroupV1],
        composition_chunks: &[E],
    ) -> Result<AggregateExpectedOpeningV1, AggregateStarkErrorV1>;
}

/// Decode an exact vector of canonical Goldilocks residues.
pub(crate) fn canonical_fields_v1(
    values: &[u64],
    expected: usize,
) -> Result<Vec<F>, AggregateStarkErrorV1> {
    if values.len() != expected {
        return Err(AggregateStarkErrorV1::InvalidProofShape);
    }
    values
        .iter()
        .copied()
        .map(|value| F::canonical(value).ok_or(AggregateStarkErrorV1::NonCanonicalField))
        .collect()
}

/// Decode an exact vector of canonical Goldilocks-quartic residues.
pub(crate) fn canonical_fp4_fields_v1(
    values: &[[u64; 4]],
    expected: usize,
) -> Result<Vec<E>, AggregateStarkErrorV1> {
    if values.len() != expected {
        return Err(AggregateStarkErrorV1::InvalidProofShape);
    }
    values
        .iter()
        .copied()
        .map(|value| E::canonical(value).ok_or(AggregateStarkErrorV1::NonCanonicalField))
        .collect()
}

fn fp4_is_in_trace_subgroup_v1(point: E, trace_size: usize) -> bool {
    point.pow(trace_size as u128) == E::ONE
}

fn fp4_is_in_evaluation_coset_v1(point: E, evaluation_size: usize) -> bool {
    let shift_power = F(GOLDILOCKS_GENERATOR_V1).pow(evaluation_size as u128);
    point.pow(evaluation_size as u128) == E::from_base(shift_power)
}

/// Check the complete public exclusion predicate for a shared DEEP point.
pub(crate) fn deep_point_is_admissible_v1(
    point: E,
    parameters: AggregateStarkParametersV1,
    layout: &AggregateProofLayoutV1,
) -> Result<bool, AggregateStarkErrorV1> {
    layout.validate(parameters)?;
    let evaluation_size = layout.common_lde_size();
    for group in &layout.trace_groups {
        let next_root = goldilocks_primitive_root_v1(group.native_trace_log2)
            .map_err(map_transparent_error_v1)?;
        for candidate in [point, point.mul_base(next_root)] {
            if fp4_is_in_evaluation_coset_v1(candidate, evaluation_size) {
                return Ok(false);
            }
            for trace_group in &layout.trace_groups {
                if fp4_is_in_trace_subgroup_v1(
                    candidate,
                    checked_domain_size_v1(trace_group.native_trace_log2)?,
                ) {
                    return Ok(false);
                }
            }
        }
    }
    Ok(true)
}

/// Derive the sole uniform DEEP point outside every trace, evaluation, and
/// query domain used by an aggregate proof.
///
/// Zero remains admissible: it is outside all multiplicative domains and all
/// required denominators remain nonzero. For every trace group the predicate
/// also excludes `z * omega_H`, because the next-row opening is evaluated at
/// that point. Query positions are elements of the common evaluation coset, so
/// excluding the complete coset also excludes every possible query point
/// before grinding fixes the concrete index set.
pub(crate) fn derive_deep_point_v1(
    transcript: &mut TransparentTranscriptV1,
    parameters: AggregateStarkParametersV1,
    layout: &AggregateProofLayoutV1,
) -> Result<E, AggregateStarkErrorV1> {
    layout.validate(parameters)?;
    transcript
        .challenge_fp4_where(DEEP_POINT_LABEL_V1, |point| {
            deep_point_is_admissible_v1(point, parameters, layout).unwrap_or(false)
        })
        .map_err(map_transparent_error_v1)
}

/// Validate the exact statement-derived shape and canonicality of a DEEP
/// opening payload.
pub(crate) fn validate_deep_proof_shape_v1(
    deep: &AggregateDeepProofV1,
    parameters: AggregateStarkParametersV1,
    layout: &AggregateProofLayoutV1,
) -> Result<(), AggregateStarkErrorV1> {
    layout.validate(parameters)?;
    if deep.trace_groups.len() != layout.trace_groups.len()
        || deep.composition_values.len() != parameters.security_lanes
        || deep
            .composition_values
            .iter()
            .any(|values| values.len() != parameters.composition_degree_chunks)
    {
        return Err(AggregateStarkErrorV1::InvalidProofShape);
    }
    for (opening, group) in deep.trace_groups.iter().zip(&layout.trace_groups) {
        if opening.base_current.len() != group.base_width
            || opening.base_next.len() != group.base_width
            || opening.aux_current.len() != group.aux_width
            || opening.aux_next.len() != group.aux_width
        {
            return Err(AggregateStarkErrorV1::InvalidProofShape);
        }
        ensure_canonical_fp4_fields_v1(&opening.base_current)?;
        ensure_canonical_fp4_fields_v1(&opening.base_next)?;
        ensure_canonical_fp4_fields_v1(&opening.aux_current)?;
        ensure_canonical_fp4_fields_v1(&opening.aux_next)?;
    }
    for values in &deep.composition_values {
        ensure_canonical_fp4_fields_v1(values)?;
    }
    Ok(())
}

/// Canonically decode all DEEP trace openings for relation-specific checks.
pub(crate) fn canonical_deep_trace_groups_v1(
    deep: &AggregateDeepProofV1,
    parameters: AggregateStarkParametersV1,
    layout: &AggregateProofLayoutV1,
) -> Result<Vec<AggregateOpenedDeepTraceGroupV1>, AggregateStarkErrorV1> {
    validate_deep_proof_shape_v1(deep, parameters, layout)?;
    deep.trace_groups
        .iter()
        .zip(&layout.trace_groups)
        .map(|(opening, group)| {
            Ok(AggregateOpenedDeepTraceGroupV1 {
                base_current: canonical_fp4_fields_v1(&opening.base_current, group.base_width)?,
                base_next: canonical_fp4_fields_v1(&opening.base_next, group.base_width)?,
                aux_current: canonical_fp4_fields_v1(&opening.aux_current, group.aux_width)?,
                aux_next: canonical_fp4_fields_v1(&opening.aux_next, group.aux_width)?,
            })
        })
        .collect()
}

/// Absorb all DEEP values in their sole group/lane/column order.
///
/// This must run after the point is sampled and before any FRI batching or fold
/// challenge is derived.
pub(crate) fn absorb_deep_openings_v1(
    transcript: &mut TransparentTranscriptV1,
    deep: &AggregateDeepProofV1,
    parameters: AggregateStarkParametersV1,
    layout: &AggregateProofLayoutV1,
) -> Result<(), AggregateStarkErrorV1> {
    validate_deep_proof_shape_v1(deep, parameters, layout)?;
    let mut encoding = Vec::new();
    encoding
        .try_reserve_exact(exact_deep_opening_bytes_v1(parameters, layout)?)
        .map_err(|_| AggregateStarkErrorV1::AllocationFailure)?;
    for group in &deep.trace_groups {
        append_fp4_fields_v1(&mut encoding, &group.base_current);
        append_fp4_fields_v1(&mut encoding, &group.base_next);
        append_fp4_fields_v1(&mut encoding, &group.aux_current);
        append_fp4_fields_v1(&mut encoding, &group.aux_next);
    }
    for values in &deep.composition_values {
        append_fp4_fields_v1(&mut encoding, values);
    }
    transcript
        .absorb(DEEP_OPENINGS_LABEL_V1, &[&encoding])
        .map_err(map_transparent_error_v1)
}

fn ensure_canonical_base_fields_v1(values: &[u64]) -> Result<(), AggregateStarkErrorV1> {
    if values
        .iter()
        .copied()
        .any(|value| F::canonical(value).is_none())
    {
        return Err(AggregateStarkErrorV1::NonCanonicalField);
    }
    Ok(())
}

fn ensure_canonical_fp4_fields_v1(values: &[[u64; 4]]) -> Result<(), AggregateStarkErrorV1> {
    if values
        .iter()
        .copied()
        .any(|value| E::canonical(value).is_none())
    {
        return Err(AggregateStarkErrorV1::NonCanonicalField);
    }
    Ok(())
}

fn row_at_v1(columns: &[Vec<F>], index: usize) -> Result<Vec<F>, AggregateStarkErrorV1> {
    columns
        .iter()
        .map(|column| {
            column
                .get(index)
                .copied()
                .ok_or(AggregateStarkErrorV1::InternalInvariant)
        })
        .collect()
}

/// Hash one vector-row leaf with its ordered group index and exact width.
pub(crate) fn row_leaf_hash_v1(
    domain: &[u8],
    group: usize,
    values: &[F],
) -> Result<[u8; 32], AggregateStarkErrorV1> {
    let group = u16::try_from(group)
        .map_err(|_| AggregateStarkErrorV1::InvalidLayout)?
        .to_be_bytes();
    let width = u16::try_from(values.len())
        .map_err(|_| AggregateStarkErrorV1::InvalidLayout)?
        .to_be_bytes();
    let mut fields = Vec::new();
    fields
        .try_reserve_exact(
            values
                .len()
                .checked_mul(core::mem::size_of::<u64>())
                .ok_or(AggregateStarkErrorV1::InvalidLayout)?,
        )
        .map_err(|_| AggregateStarkErrorV1::AllocationFailure)?;
    for value in values {
        fields.extend_from_slice(&value.0.to_be_bytes());
    }
    sha256_frame_v1(domain, &[&group, &width, &fields]).map_err(map_transparent_error_v1)
}

fn composition_leaf_hash_unchecked_v1(
    domains: AggregateStarkDomainsV1,
    lane: usize,
    values: &[E],
) -> Result<[u8; 32], AggregateStarkErrorV1> {
    if values.is_empty() {
        return Err(AggregateStarkErrorV1::InvalidLayout);
    }
    let lane = u16::try_from(lane)
        .map_err(|_| AggregateStarkErrorV1::InvalidLayout)?
        .to_be_bytes();
    let width = u16::try_from(values.len())
        .map_err(|_| AggregateStarkErrorV1::InvalidLayout)?
        .to_be_bytes();
    let mut fields = Vec::new();
    fields
        .try_reserve_exact(
            values
                .len()
                .checked_mul(core::mem::size_of::<[u64; 4]>())
                .ok_or(AggregateStarkErrorV1::InvalidLayout)?,
        )
        .map_err(|_| AggregateStarkErrorV1::AllocationFailure)?;
    for value in values {
        fields.extend_from_slice(&value.to_be_bytes());
    }
    sha256_frame_v1(domains.composition_leaf, &[&lane, &width, &fields])
        .map_err(map_transparent_error_v1)
}

/// Prover-only material for one independently sampled FRI mask oracle.
///
/// The polynomial evaluations and Merkle tree are retained until the
/// transcript fixes all query positions. They are never serialized wholesale.
pub(crate) struct AggregateFriMaskOracleMaterialV1 {
    /// Coset evaluations on the common LDE domain.
    pub(crate) evaluations: Vec<E>,
    /// Authenticated oracle commitment.
    pub(crate) tree: Sha256MerkleTreeV1,
}

fn fri_mask_leaf_hash_v1(lane: usize, value: E) -> Result<[u8; 32], AggregateStarkErrorV1> {
    let lane = u16::try_from(lane)
        .map_err(|_| AggregateStarkErrorV1::InvalidLayout)?
        .to_be_bytes();
    sha256_frame_v1(FRI_MASK_LEAF_DOMAIN_V1, &[&lane, &value.to_be_bytes()])
        .map_err(map_transparent_error_v1)
}

fn fri_mask_tree_v1(
    lane: usize,
    evaluations: &[E],
) -> Result<Sha256MerkleTreeV1, AggregateStarkErrorV1> {
    if evaluations.is_empty() || !evaluations.len().is_power_of_two() {
        return Err(AggregateStarkErrorV1::InvalidLayout);
    }
    let leaves = evaluations
        .iter()
        .copied()
        .map(|value| fri_mask_leaf_hash_v1(lane, value))
        .collect::<Result<Vec<_>, _>>()?;
    Sha256MerkleTreeV1::from_leaves(leaves, FRI_MASK_NODE_DOMAIN_V1)
        .map_err(map_transparent_error_v1)
}

/// Sample and commit all independent Protocol-2 FRI mask polynomials.
///
/// Each lane samples the normalized `D - 1` coefficients, which contains the
/// required `|H| + h - 1`-coefficient hiding space and stays strictly inside
/// the one FRI-enforced exclusive cap `D`. The resulting root must be
/// transcript-bound before any challenge that batches the lane's trace and
/// composition polynomials.
pub(crate) fn build_fri_mask_oracles_v1<R: TryRngCore>(
    parameters: AggregateStarkParametersV1,
    layout: &AggregateProofLayoutV1,
    rng: &mut R,
) -> Result<Vec<AggregateFriMaskOracleMaterialV1>, AggregateStarkErrorV1> {
    layout.validate(parameters)?;
    let coefficient_count = layout.fri_mask_coefficient_count(parameters)?;
    let lde_size = layout.common_lde_size();
    let lde_root =
        goldilocks_primitive_root_v1(layout.common_lde_log2).map_err(map_transparent_error_v1)?;
    let mut oracles = Vec::new();
    oracles
        .try_reserve_exact(parameters.security_lanes)
        .map_err(|_| AggregateStarkErrorV1::AllocationFailure)?;
    for lane in 0..parameters.security_lanes {
        let mut coefficients = ZeroizingExtensionFieldColumnV1(Vec::new());
        coefficients
            .0
            .try_reserve_exact(lde_size)
            .map_err(|_| AggregateStarkErrorV1::AllocationFailure)?;
        for _ in 0..coefficient_count {
            coefficients
                .0
                .push(random_goldilocks_fp4_v1(rng).map_err(map_transparent_error_v1)?);
        }
        coefficients.0.resize(lde_size, E::ZERO);
        let evaluations = goldilocks_fp4_evaluate_coset_v1(
            &coefficients,
            lde_size,
            lde_root,
            F(GOLDILOCKS_GENERATOR_V1),
        )
        .map_err(map_transparent_error_v1)?;
        let tree = fri_mask_tree_v1(lane, &evaluations)?;
        oracles.push(AggregateFriMaskOracleMaterialV1 { evaluations, tree });
    }
    Ok(oracles)
}

/// Add the independently committed mask oracle to one batched FRI lane.
pub(crate) fn add_fri_mask_oracle_v1(
    base_values: &mut [E],
    mask: &AggregateFriMaskOracleMaterialV1,
) -> Result<(), AggregateStarkErrorV1> {
    if base_values.len() != mask.evaluations.len() {
        return Err(AggregateStarkErrorV1::InvalidLayout);
    }
    for (base, randomizer) in base_values.iter_mut().zip(&mask.evaluations) {
        *base = base.add(*randomizer);
    }
    Ok(())
}

fn fri_leaf_hash_unchecked_v1(
    domains: AggregateStarkDomainsV1,
    lane: usize,
    round: usize,
    value: E,
) -> Result<[u8; 32], AggregateStarkErrorV1> {
    let lane = u16::try_from(lane)
        .map_err(|_| AggregateStarkErrorV1::InvalidLayout)?
        .to_be_bytes();
    let round = u16::try_from(round)
        .map_err(|_| AggregateStarkErrorV1::InvalidLayout)?
        .to_be_bytes();
    sha256_frame_v1(domains.fri_leaf, &[&lane, &round, &value.to_be_bytes()])
        .map_err(map_transparent_error_v1)
}

/// Commit vector-row columns on a common power-of-two domain.
pub(crate) fn row_tree_v1(
    leaf_domain: &[u8],
    node_domain: &'static [u8],
    group: usize,
    columns: &[Vec<F>],
    rows: usize,
) -> Result<Sha256MerkleTreeV1, AggregateStarkErrorV1> {
    if rows == 0
        || !rows.is_power_of_two()
        || columns.is_empty()
        || columns.iter().any(|column| column.len() != rows)
    {
        return Err(AggregateStarkErrorV1::InvalidLayout);
    }
    let leaves = (0..rows)
        .map(|index| row_leaf_hash_v1(leaf_domain, group, &row_at_v1(columns, index)?))
        .collect::<Result<Vec<_>, _>>()?;
    Sha256MerkleTreeV1::from_leaves(leaves, node_domain).map_err(map_transparent_error_v1)
}

/// Root and canonical minimal frontier produced without retaining a Merkle tree.
#[cfg(any(test, feature = "privacy-release-evidence"))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct StreamingMerkleCommitmentV1 {
    /// Root of the complete power-of-two leaf stream.
    pub(crate) root: [u8; 32],
    /// Canonically ordered minimal frontier for the requested leaves.
    pub(crate) frontier: Vec<[u8; 32]>,
}

/// Incremental binary Merkle accumulator with a query-aware frontier plan.
///
/// Leaves must be appended in ascending index order. The accumulator retains
/// one pending subtree per level and only those sibling hashes required by the
/// requested canonical multiproof. Its memory is therefore
/// `O(log(leaf_count) + frontier_len)` rather than `O(leaf_count)`.
#[cfg(any(test, feature = "privacy-release-evidence"))]
pub(crate) struct StreamingMerkleAccumulatorV1 {
    node_domain: &'static [u8],
    leaf_count: usize,
    next_leaf: usize,
    pending: Vec<Option<[u8; 32]>>,
    frontier_positions: BTreeMap<(usize, usize), usize>,
    frontier: Vec<Option<[u8; 32]>>,
}

#[cfg(any(test, feature = "privacy-release-evidence"))]
impl StreamingMerkleAccumulatorV1 {
    /// Create an accumulator for an exact leaf count and sorted unique opening set.
    ///
    /// An empty opening set is permitted for the transcript's root-only
    /// commitment pass.
    pub(crate) fn new(
        node_domain: &'static [u8],
        leaf_count: usize,
        opening_indices: &[usize],
    ) -> Result<Self, AggregateStarkErrorV1> {
        if node_domain.is_empty() || leaf_count == 0 || !leaf_count.is_power_of_two() {
            return Err(AggregateStarkErrorV1::InvalidLayout);
        }
        if !opening_indices.is_empty() {
            validate_canonical_index_set_v1(leaf_count, opening_indices)?;
        }
        let height = usize::try_from(leaf_count.ilog2())
            .map_err(|_| AggregateStarkErrorV1::InvalidLayout)?;
        let mut frontier_positions = BTreeMap::new();
        let mut current = opening_indices.iter().copied().collect::<BTreeSet<_>>();
        let mut level_size = leaf_count;
        let mut level = 0_usize;
        let mut frontier_len = 0_usize;
        while level_size > 1 && !current.is_empty() {
            for &index in &current {
                if !current.contains(&(index ^ 1)) {
                    if frontier_positions
                        .insert((level, index ^ 1), frontier_len)
                        .is_some()
                    {
                        return Err(AggregateStarkErrorV1::InternalInvariant);
                    }
                    frontier_len = frontier_len
                        .checked_add(1)
                        .ok_or(AggregateStarkErrorV1::InvalidLayout)?;
                }
            }
            current = current.into_iter().map(|index| index >> 1).collect();
            level_size >>= 1;
            level += 1;
        }
        if !opening_indices.is_empty()
            && (current.len() != 1
                || !current.contains(&0)
                || frontier_len != multiproof_frontier_len_v1(leaf_count, opening_indices)?)
        {
            return Err(AggregateStarkErrorV1::InternalInvariant);
        }
        let mut pending = Vec::new();
        pending
            .try_reserve_exact(
                height
                    .checked_add(1)
                    .ok_or(AggregateStarkErrorV1::InvalidLayout)?,
            )
            .map_err(|_| AggregateStarkErrorV1::AllocationFailure)?;
        pending.resize(height + 1, None);
        let mut frontier = Vec::new();
        frontier
            .try_reserve_exact(frontier_len)
            .map_err(|_| AggregateStarkErrorV1::AllocationFailure)?;
        frontier.resize(frontier_len, None);
        Ok(Self {
            node_domain,
            leaf_count,
            next_leaf: 0,
            pending,
            frontier_positions,
            frontier,
        })
    }

    fn capture(
        &mut self,
        level: usize,
        index: usize,
        node: [u8; 32],
    ) -> Result<(), AggregateStarkErrorV1> {
        let Some(position) = self.frontier_positions.get(&(level, index)).copied() else {
            return Ok(());
        };
        let slot = self
            .frontier
            .get_mut(position)
            .ok_or(AggregateStarkErrorV1::InternalInvariant)?;
        if slot.replace(node).is_some() {
            return Err(AggregateStarkErrorV1::InternalInvariant);
        }
        Ok(())
    }

    /// Append the next leaf digest.
    pub(crate) fn append_leaf(&mut self, mut node: [u8; 32]) -> Result<(), AggregateStarkErrorV1> {
        if self.next_leaf >= self.leaf_count {
            return Err(AggregateStarkErrorV1::InvalidProofShape);
        }
        let mut index = self.next_leaf;
        let mut level = 0_usize;
        self.capture(level, index, node)?;
        loop {
            let slot = self
                .pending
                .get_mut(level)
                .ok_or(AggregateStarkErrorV1::InternalInvariant)?;
            if index & 1 == 0 {
                if slot.replace(node).is_some() {
                    return Err(AggregateStarkErrorV1::InternalInvariant);
                }
                break;
            }
            let left = slot
                .take()
                .ok_or(AggregateStarkErrorV1::InternalInvariant)?;
            node = sha256_merkle_node_v1(self.node_domain, &left, &node);
            index >>= 1;
            level += 1;
            self.capture(level, index, node)?;
        }
        self.next_leaf = self
            .next_leaf
            .checked_add(1)
            .ok_or(AggregateStarkErrorV1::InvalidLayout)?;
        Ok(())
    }

    /// Finish the exact stream and return its root and requested frontier.
    pub(crate) fn finish(mut self) -> Result<StreamingMerkleCommitmentV1, AggregateStarkErrorV1> {
        if self.next_leaf != self.leaf_count {
            return Err(AggregateStarkErrorV1::InvalidProofShape);
        }
        let height = usize::try_from(self.leaf_count.ilog2())
            .map_err(|_| AggregateStarkErrorV1::InvalidLayout)?;
        if self.pending[..height].iter().any(Option::is_some) {
            return Err(AggregateStarkErrorV1::InternalInvariant);
        }
        let root = self.pending[height]
            .take()
            .ok_or(AggregateStarkErrorV1::InternalInvariant)?;
        let frontier = self
            .frontier
            .into_iter()
            .map(|node| node.ok_or(AggregateStarkErrorV1::InternalInvariant))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(StreamingMerkleCommitmentV1 { root, frontier })
    }
}

/// Commit an exact leaf iterator with logarithmic tree memory.
#[cfg(any(test, feature = "privacy-release-evidence"))]
pub(crate) fn streaming_merkle_commitment_v1<I>(
    node_domain: &'static [u8],
    leaf_count: usize,
    opening_indices: &[usize],
    leaves: I,
) -> Result<StreamingMerkleCommitmentV1, AggregateStarkErrorV1>
where
    I: IntoIterator<Item = Result<[u8; 32], AggregateStarkErrorV1>>,
{
    let mut accumulator =
        StreamingMerkleAccumulatorV1::new(node_domain, leaf_count, opening_indices)?;
    for leaf in leaves {
        accumulator.append_leaf(leaf?)?;
    }
    accumulator.finish()
}

/// Result of one column-streamed vector-row commitment pass.
#[cfg(any(test, feature = "privacy-release-evidence"))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct StreamingRowCommitmentResultV1 {
    /// Root and canonical frontier of the committed vector rows.
    pub(crate) commitment: StreamingMerkleCommitmentV1,
    /// Exact requested vector rows keyed by ascending LDE index.
    pub(crate) opened_rows: BTreeMap<usize, Vec<F>>,
}

/// Column-at-a-time vector-row commitment builder.
///
/// This is the bounded-memory replacement for retaining every LDE column.
/// Each row owns one incremental SHA-256 state while columns are supplied in
/// canonical order. The final leaf digests are immediately consumed by
/// [`StreamingMerkleAccumulatorV1`], so neither leaves nor tree levels are
/// retained. A second deterministic pass after Fiat–Shamir query derivation
/// supplies the canonical frontier and the small set of opened rows.
#[cfg(any(test, feature = "privacy-release-evidence"))]
pub(crate) struct StreamingRowCommitmentV1 {
    rows: usize,
    width: usize,
    received_columns: usize,
    node_domain: &'static [u8],
    hashers: Vec<Sha256>,
    opening_indices: Vec<usize>,
    opened_rows: BTreeMap<usize, Vec<F>>,
}

#[cfg(any(test, feature = "privacy-release-evidence"))]
impl StreamingRowCommitmentV1 {
    /// Start an exact row commitment pass.
    pub(crate) fn new(
        leaf_domain: &[u8],
        node_domain: &'static [u8],
        group: usize,
        rows: usize,
        width: usize,
        opening_indices: &[usize],
    ) -> Result<Self, AggregateStarkErrorV1> {
        if leaf_domain.is_empty()
            || node_domain.is_empty()
            || rows == 0
            || !rows.is_power_of_two()
            || width == 0
        {
            return Err(AggregateStarkErrorV1::InvalidLayout);
        }
        if !opening_indices.is_empty() {
            validate_canonical_index_set_v1(rows, opening_indices)
                .map_err(|_| AggregateStarkErrorV1::InvalidLayout)?;
        }
        let group = u16::try_from(group)
            .map_err(|_| AggregateStarkErrorV1::InvalidLayout)?
            .to_be_bytes();
        let width_u16 = u16::try_from(width)
            .map_err(|_| AggregateStarkErrorV1::InvalidLayout)?
            .to_be_bytes();
        let domain_len = u16::try_from(leaf_domain.len())
            .map_err(|_| AggregateStarkErrorV1::InvalidLayout)?
            .to_be_bytes();
        let value_bytes = width
            .checked_mul(core::mem::size_of::<u64>())
            .and_then(|bytes| u64::try_from(bytes).ok())
            .ok_or(AggregateStarkErrorV1::InvalidLayout)?
            .to_be_bytes();

        // Exact prefix of `sha256_frame_v1(leaf_domain,
        // &[group, width, concatenated_values])`, stopping immediately before
        // the first value byte.
        let mut prefix = Sha256::new();
        prefix.update(TRANSCRIPT_FRAME_DOMAIN_V1);
        prefix.update(domain_len);
        prefix.update(leaf_domain);
        prefix.update(3_u16.to_be_bytes());
        prefix.update(2_u64.to_be_bytes());
        prefix.update(group);
        prefix.update(2_u64.to_be_bytes());
        prefix.update(width_u16);
        prefix.update(value_bytes);

        let mut hashers = Vec::new();
        hashers
            .try_reserve_exact(rows)
            .map_err(|_| AggregateStarkErrorV1::AllocationFailure)?;
        for _ in 0..rows {
            hashers.push(prefix.clone());
        }
        let mut opened_rows = BTreeMap::new();
        for &index in opening_indices {
            let mut values = Vec::new();
            values
                .try_reserve_exact(width)
                .map_err(|_| AggregateStarkErrorV1::AllocationFailure)?;
            if opened_rows.insert(index, values).is_some() {
                return Err(AggregateStarkErrorV1::InvalidLayout);
            }
        }
        Ok(Self {
            rows,
            width,
            received_columns: 0,
            node_domain,
            hashers,
            opening_indices: opening_indices.to_vec(),
            opened_rows,
        })
    }

    /// Absorb the next complete LDE column in canonical column order.
    pub(crate) fn absorb_column(&mut self, column: &[F]) -> Result<(), AggregateStarkErrorV1> {
        if self.received_columns >= self.width || column.len() != self.rows {
            return Err(AggregateStarkErrorV1::InvalidLayout);
        }
        for (hasher, value) in self.hashers.iter_mut().zip(column) {
            hasher.update(value.0.to_be_bytes());
        }
        for &index in &self.opening_indices {
            self.opened_rows
                .get_mut(&index)
                .ok_or(AggregateStarkErrorV1::InternalInvariant)?
                .push(column[index]);
        }
        self.received_columns += 1;
        Ok(())
    }

    /// Finalize the exact-width vector rows into a streaming Merkle commitment.
    pub(crate) fn finish(self) -> Result<StreamingRowCommitmentResultV1, AggregateStarkErrorV1> {
        if self.received_columns != self.width
            || self
                .opened_rows
                .values()
                .any(|values| values.len() != self.width)
        {
            return Err(AggregateStarkErrorV1::InvalidLayout);
        }
        let leaves = self.hashers.into_iter().map(|hasher| {
            let digest: [u8; 32] = hasher.finalize().into();
            Ok(digest)
        });
        let commitment = streaming_merkle_commitment_v1(
            self.node_domain,
            self.rows,
            &self.opening_indices,
            leaves,
        )?;
        Ok(StreamingRowCommitmentResultV1 {
            commitment,
            opened_rows: self.opened_rows,
        })
    }
}

/// Secret replay material for one exact ordered set of streamed trace columns.
///
/// This type deliberately implements neither `Clone` nor `Debug`. Dropping it
/// recursively overwrites every mask coefficient through
/// [`ReplayableTraceMaskV1`].
#[cfg(test)]
pub(crate) struct StreamingTraceMaskSetV1 {
    native_trace_log2: u8,
    lde_log2: u8,
    masks: Vec<ReplayableTraceMaskV1>,
}

/// Owner of one secret-bearing field column that overwrites every cell on
/// every return path.
#[cfg(any(test, feature = "privacy-release-evidence"))]
pub(crate) struct ZeroizingFieldColumnV1(Vec<F>);

#[cfg(any(test, feature = "privacy-release-evidence"))]
impl ZeroizingFieldColumnV1 {
    /// Transfer ownership to another zeroizing container without duplicating
    /// the secret-bearing allocation.
    pub(super) fn into_vec_v1(mut self) -> Vec<F> {
        core::mem::take(&mut self.0)
    }
}

#[cfg(any(test, feature = "privacy-release-evidence"))]
impl core::ops::Deref for ZeroizingFieldColumnV1 {
    type Target = [F];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[cfg(any(test, feature = "privacy-release-evidence"))]
impl Drop for ZeroizingFieldColumnV1 {
    fn drop(&mut self) {
        zeroize_field_column_v1(&mut self.0);
    }
}

#[cfg(any(test, feature = "privacy-release-evidence"))]
fn zeroize_field_column_v1(values: &mut [F]) {
    for value in values {
        value.0 = 0;
    }
}

/// Owner of one secret-bearing quartic-extension column that overwrites every
/// base-field coefficient on every return path.
struct ZeroizingExtensionFieldColumnV1(Vec<E>);

impl core::ops::Deref for ZeroizingExtensionFieldColumnV1 {
    type Target = [E];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Drop for ZeroizingExtensionFieldColumnV1 {
    fn drop(&mut self) {
        zeroize_extension_field_column_v1(&mut self.0);
    }
}

fn zeroize_extension_field_column_v1(values: &mut [E]) {
    for value in values {
        *value = E::ZERO;
    }
}

#[cfg(test)]
impl StreamingTraceMaskSetV1 {
    /// Number of committed columns.
    pub(crate) fn width(&self) -> usize {
        self.masks.len()
    }
}

/// Secret retained masked polynomials for one exact streamed trace commitment.
///
/// Each column stores ascending coefficients of
/// `T(X) + r(X) * (X^n - 1)`, rather than a common-domain LDE. This lets a
/// prover evaluate the same committed polynomial on its large commitment
/// domain, a smaller quotient domain, and transcript-derived DEEP points
/// without anonymous matrix scratch. The type implements neither `Clone` nor
/// `Debug`; every coefficient is overwritten recursively on drop.
#[cfg(any(test, feature = "privacy-release-evidence"))]
pub(crate) struct MaskedTracePolynomialSetV1 {
    native_trace_log2: u8,
    commitment_lde_log2: u8,
    columns: Vec<ZeroizingFieldColumnV1>,
}

#[cfg(any(test, feature = "privacy-release-evidence"))]
impl MaskedTracePolynomialSetV1 {
    fn validate_v1(&self) -> Result<(usize, usize), AggregateStarkErrorV1> {
        let native_rows = checked_domain_size_v1(self.native_trace_log2)?;
        let commitment_rows = checked_domain_size_v1(self.commitment_lde_log2)?;
        let coefficient_count = self
            .columns
            .first()
            .map(|column| column.len())
            .filter(|count| *count > native_rows && *count <= commitment_rows)
            .ok_or(AggregateStarkErrorV1::InvalidLayout)?;
        if self.native_trace_log2 >= self.commitment_lde_log2
            || self.columns.iter().any(|column| {
                column.len() != coefficient_count
                    || column.iter().any(|value| F::canonical(value.0).is_none())
            })
        {
            return Err(AggregateStarkErrorV1::InvalidLayout);
        }
        Ok((native_rows, commitment_rows))
    }

    /// Number of committed columns.
    pub(crate) fn width(&self) -> usize {
        self.columns.len()
    }

    /// Native trace-domain logarithm shared by every retained polynomial.
    pub(crate) const fn native_trace_log2(&self) -> u8 {
        self.native_trace_log2
    }

    /// Logarithm of the domain used by the transcript-bound commitment.
    pub(crate) const fn commitment_lde_log2(&self) -> u8 {
        self.commitment_lde_log2
    }

    /// Exact ascending coefficients of one retained masked polynomial.
    pub(crate) fn column_coefficients_v1(
        &self,
        column: usize,
    ) -> Result<&[F], AggregateStarkErrorV1> {
        self.validate_v1()?;
        self.columns
            .get(column)
            .map(|values| values.0.as_slice())
            .ok_or(AggregateStarkErrorV1::InvalidLayout)
    }

    /// Evaluate one retained column on a canonical generator coset.
    pub(crate) fn evaluate_column_on_coset_v1(
        &self,
        column: usize,
        evaluation_log2: u8,
    ) -> Result<ZeroizingFieldColumnV1, AggregateStarkErrorV1> {
        let coefficients = self.column_coefficients_v1(column)?;
        Ok(ZeroizingFieldColumnV1(
            masked_trace_coefficients_on_coset_v1(
                coefficients,
                self.native_trace_log2,
                evaluation_log2,
            )
            .map_err(map_transparent_error_v1)?,
        ))
    }

    /// Evaluate every retained column on one verifier-derived coset.
    #[cfg(test)]
    pub(crate) fn evaluate_columns_on_coset_v1(
        &self,
        evaluation_log2: u8,
    ) -> Result<Vec<ZeroizingFieldColumnV1>, AggregateStarkErrorV1> {
        self.validate_v1()?;
        let mut columns = Vec::new();
        columns
            .try_reserve_exact(self.width())
            .map_err(|_| AggregateStarkErrorV1::AllocationFailure)?;
        for column in 0..self.width() {
            columns.push(self.evaluate_column_on_coset_v1(column, evaluation_log2)?);
        }
        Ok(columns)
    }
}

fn checked_domain_size_v1(log2: u8) -> Result<usize, AggregateStarkErrorV1> {
    1_usize
        .checked_shl(u32::from(log2))
        .ok_or(AggregateStarkErrorV1::InvalidLayout)
}

#[cfg(any(test, feature = "privacy-release-evidence"))]
fn validate_masked_trace_commitment_shape_v1(
    leaf_domain: &[u8],
    node_domain: &[u8],
    group: usize,
    native_trace_log2: u8,
    lde_log2: u8,
    width: usize,
    mask_degree: usize,
    opening_indices: &[usize],
) -> Result<(usize, usize), AggregateStarkErrorV1> {
    let native_rows = checked_domain_size_v1(native_trace_log2)?;
    let lde_rows = checked_domain_size_v1(lde_log2)?;
    if leaf_domain.is_empty()
        || node_domain.is_empty()
        || native_trace_log2 >= lde_log2
        || width == 0
        || u16::try_from(group).is_err()
        || u16::try_from(width).is_err()
        || u16::try_from(leaf_domain.len()).is_err()
        || native_rows
            .checked_add(mask_degree)
            .is_none_or(|highest| highest >= lde_rows)
    {
        return Err(AggregateStarkErrorV1::InvalidLayout);
    }
    if !opening_indices.is_empty() {
        validate_canonical_index_set_v1(lde_rows, opening_indices)
            .map_err(|_| AggregateStarkErrorV1::InvalidLayout)?;
    }
    Ok((native_rows, lde_rows))
}

/// Sample replayable masks and commit columns generated one at a time.
#[cfg(test)]
pub(crate) fn commit_masked_trace_columns_v1<R, S>(
    leaf_domain: &[u8],
    node_domain: &'static [u8],
    group: usize,
    native_trace_log2: u8,
    lde_log2: u8,
    width: usize,
    mask_degree: usize,
    opening_indices: &[usize],
    rng: &mut R,
    mut source: S,
) -> Result<(StreamingRowCommitmentResultV1, StreamingTraceMaskSetV1), AggregateStarkErrorV1>
where
    R: TryRngCore,
    S: FnMut(usize) -> Result<Vec<F>, AggregateStarkErrorV1>,
{
    let (native_rows, lde_rows) = validate_masked_trace_commitment_shape_v1(
        leaf_domain,
        node_domain,
        group,
        native_trace_log2,
        lde_log2,
        width,
        mask_degree,
        opening_indices,
    )?;
    let mut commitment = StreamingRowCommitmentV1::new(
        leaf_domain,
        node_domain,
        group,
        lde_rows,
        width,
        opening_indices,
    )?;
    let mut masks = Vec::new();
    masks
        .try_reserve_exact(width)
        .map_err(|_| AggregateStarkErrorV1::AllocationFailure)?;
    for column_index in 0..width {
        let native = ZeroizingFieldColumnV1(source(column_index)?);
        if native.len() != native_rows {
            return Err(AggregateStarkErrorV1::InvalidLayout);
        }
        let mask = sample_trace_mask_v1(mask_degree, rng).map_err(map_transparent_error_v1)?;
        let lde = ZeroizingFieldColumnV1(
            masked_trace_lde_column_with_mask_v1(
                &native,
                native_trace_log2,
                lde_log2,
                mask.coefficients(),
            )
            .map_err(map_transparent_error_v1)?,
        );
        commitment.absorb_column(&lde)?;
        masks.push(mask);
    }
    Ok((
        commitment.finish()?,
        StreamingTraceMaskSetV1 {
            native_trace_log2,
            lde_log2,
            masks,
        },
    ))
}

/// Deterministically replay one streamed trace commitment with the original
/// secret masks after Fiat–Shamir queries are fixed.
#[cfg(test)]
pub(crate) fn replay_masked_trace_columns_v1<S>(
    leaf_domain: &[u8],
    node_domain: &'static [u8],
    group: usize,
    masks: &StreamingTraceMaskSetV1,
    opening_indices: &[usize],
    mut source: S,
) -> Result<StreamingRowCommitmentResultV1, AggregateStarkErrorV1>
where
    S: FnMut(usize) -> Result<Vec<F>, AggregateStarkErrorV1>,
{
    let native_rows = checked_domain_size_v1(masks.native_trace_log2)?;
    let lde_rows = checked_domain_size_v1(masks.lde_log2)?;
    let mut commitment = StreamingRowCommitmentV1::new(
        leaf_domain,
        node_domain,
        group,
        lde_rows,
        masks.width(),
        opening_indices,
    )?;
    for (column_index, mask) in masks.masks.iter().enumerate() {
        let native = ZeroizingFieldColumnV1(source(column_index)?);
        if native.len() != native_rows {
            return Err(AggregateStarkErrorV1::InvalidLayout);
        }
        let lde = ZeroizingFieldColumnV1(
            masked_trace_lde_column_with_mask_v1(
                &native,
                masks.native_trace_log2,
                masks.lde_log2,
                mask.coefficients(),
            )
            .map_err(map_transparent_error_v1)?,
        );
        commitment.absorb_column(&lde)?;
    }
    commitment.finish()
}

/// Sample masks, commit their LDEs, and retain only the masked coefficients.
///
/// All verifier-derived shape checks and commitment allocations finish before
/// the witness source is called. The retained coefficient set is sufficient
/// to replay the exact commitment, evaluate smaller quotient cosets, and
/// construct DEEP openings without materializing anonymous common-domain
/// scratch.
#[cfg(any(test, feature = "privacy-release-evidence"))]
pub(crate) fn commit_masked_trace_polynomial_columns_v1<R, S>(
    leaf_domain: &[u8],
    node_domain: &'static [u8],
    group: usize,
    native_trace_log2: u8,
    commitment_lde_log2: u8,
    width: usize,
    mask_degree: usize,
    opening_indices: &[usize],
    rng: &mut R,
    mut source: S,
) -> Result<(StreamingRowCommitmentResultV1, MaskedTracePolynomialSetV1), AggregateStarkErrorV1>
where
    R: TryRngCore,
    S: FnMut(usize) -> Result<Vec<F>, AggregateStarkErrorV1>,
{
    let (native_rows, commitment_rows) = validate_masked_trace_commitment_shape_v1(
        leaf_domain,
        node_domain,
        group,
        native_trace_log2,
        commitment_lde_log2,
        width,
        mask_degree,
        opening_indices,
    )?;
    let mut commitment = StreamingRowCommitmentV1::new(
        leaf_domain,
        node_domain,
        group,
        commitment_rows,
        width,
        opening_indices,
    )?;
    let mut columns = Vec::new();
    columns
        .try_reserve_exact(width)
        .map_err(|_| AggregateStarkErrorV1::AllocationFailure)?;
    for column_index in 0..width {
        let native = ZeroizingFieldColumnV1(source(column_index)?);
        if native.len() != native_rows {
            return Err(AggregateStarkErrorV1::InvalidLayout);
        }
        let mask = sample_trace_mask_v1(mask_degree, rng).map_err(map_transparent_error_v1)?;
        let coefficients = ZeroizingFieldColumnV1(
            masked_trace_coefficients_with_mask_v1(&native, native_trace_log2, mask.coefficients())
                .map_err(map_transparent_error_v1)?,
        );
        columns.push(coefficients);
    }
    // The profile's eight-column batch is a memory ceiling as well as a
    // throughput choice. Coefficients and masks are sampled serially above,
    // preserving the byte-exact transcript, while independent LDEs within
    // each bounded batch use the release runner's fixed Rayon pool. Roots are
    // still absorbed in canonical column order.
    for batch in columns.chunks(MASKED_TRACE_LDE_COLUMN_BATCH_V1) {
        let evaluations = batch
            .par_iter()
            .map(|coefficients| {
                masked_trace_coefficients_on_coset_v1(
                    coefficients,
                    native_trace_log2,
                    commitment_lde_log2,
                )
                .map(ZeroizingFieldColumnV1)
                .map_err(map_transparent_error_v1)
            })
            .collect::<Result<Vec<_>, _>>()?;
        for evaluation in &evaluations {
            commitment.absorb_column(evaluation)?;
        }
    }
    let polynomials = MaskedTracePolynomialSetV1 {
        native_trace_log2,
        commitment_lde_log2,
        columns,
    };
    polynomials.validate_v1()?;
    Ok((commitment.finish()?, polynomials))
}

/// Replay a retained masked-polynomial commitment and requested row frontier.
///
/// No native witness source or mask RNG is needed: the exact committed
/// polynomials, including canonical trailing zero coefficients, are owned by
/// `polynomials`.
#[cfg(any(test, feature = "privacy-release-evidence"))]
pub(crate) fn replay_masked_trace_polynomial_columns_v1(
    leaf_domain: &[u8],
    node_domain: &'static [u8],
    group: usize,
    polynomials: &MaskedTracePolynomialSetV1,
    opening_indices: &[usize],
) -> Result<StreamingRowCommitmentResultV1, AggregateStarkErrorV1> {
    let (_, commitment_rows) = polynomials.validate_v1()?;
    let mut commitment = StreamingRowCommitmentV1::new(
        leaf_domain,
        node_domain,
        group,
        commitment_rows,
        polynomials.width(),
        opening_indices,
    )?;
    for batch_start in (0..polynomials.width()).step_by(MASKED_TRACE_LDE_COLUMN_BATCH_V1) {
        let batch_end = batch_start
            .checked_add(MASKED_TRACE_LDE_COLUMN_BATCH_V1)
            .map_or(polynomials.width(), |end| end.min(polynomials.width()));
        let evaluations = (batch_start..batch_end)
            .into_par_iter()
            .map(|column| {
                polynomials.evaluate_column_on_coset_v1(column, polynomials.commitment_lde_log2)
            })
            .collect::<Result<Vec<_>, _>>()?;
        for evaluation in &evaluations {
            commitment.absorb_column(evaluation)?;
        }
    }
    commitment.finish()
}

#[cfg(test)]
const ENCRYPTED_FIELD_SCRATCH_AAD_DOMAIN_V1: &[u8] =
    b"iroha:privacy:aggregate-stark:encrypted-field-scratch-record:v1";
#[cfg(test)]
const XCHACHA20_POLY1305_TAG_BYTES_V1: usize = 16;
#[cfg(test)]
const XCHACHA20_NONCE_PREFIX_BYTES_V1: usize = 16;
#[cfg(test)]
const XCHACHA20_NONCE_BYTES_V1: usize = 24;

/// Default number of common-domain rows authenticated in one scratch record.
///
/// At eight bytes per field this keeps each plaintext record at 32 KiB. The
/// value is a power of two and therefore divides every admitted aggregate LDE
/// domain at or above this size.
pub(crate) const DEFAULT_ENCRYPTED_TRACE_SCRATCH_CHUNK_ROWS_V1: usize = 1 << 12;

#[cfg(test)]
fn encrypted_field_scratch_record_aad_v1(
    rows: usize,
    width: usize,
    chunk_rows: usize,
    record_index: u64,
) -> Result<[u8; 32], AggregateStarkErrorV1> {
    let rows = u64::try_from(rows)
        .map_err(|_| AggregateStarkErrorV1::InvalidLayout)?
        .to_be_bytes();
    let width = u64::try_from(width)
        .map_err(|_| AggregateStarkErrorV1::InvalidLayout)?
        .to_be_bytes();
    let chunk_rows = u64::try_from(chunk_rows)
        .map_err(|_| AggregateStarkErrorV1::InvalidLayout)?
        .to_be_bytes();
    let record_index = record_index.to_be_bytes();
    sha256_frame_v1(
        ENCRYPTED_FIELD_SCRATCH_AAD_DOMAIN_V1,
        &[&rows, &width, &chunk_rows, &record_index],
    )
    .map_err(map_transparent_error_v1)
}

#[cfg(test)]
fn encrypted_field_scratch_nonce_v1(
    nonce_prefix: &[u8; XCHACHA20_NONCE_PREFIX_BYTES_V1],
    record_index: u64,
) -> chacha20poly1305::XNonce {
    let mut bytes = [0_u8; XCHACHA20_NONCE_BYTES_V1];
    bytes[..XCHACHA20_NONCE_PREFIX_BYTES_V1].copy_from_slice(nonce_prefix);
    bytes[XCHACHA20_NONCE_PREFIX_BYTES_V1..].copy_from_slice(&record_index.to_be_bytes());
    bytes.into()
}

#[cfg(test)]
fn encrypted_field_scratch_shape_v1(
    rows: usize,
    width: usize,
    chunk_rows: usize,
) -> Result<(usize, usize, usize, u64), AggregateStarkErrorV1> {
    if rows == 0
        || !rows.is_power_of_two()
        || width == 0
        || chunk_rows == 0
        || !chunk_rows.is_power_of_two()
        || chunk_rows > rows
        || rows % chunk_rows != 0
    {
        return Err(AggregateStarkErrorV1::InvalidLayout);
    }
    let chunks_per_column = rows / chunk_rows;
    let plaintext_chunk_bytes = chunk_rows
        .checked_mul(core::mem::size_of::<u64>())
        .ok_or(AggregateStarkErrorV1::InvalidLayout)?;
    let ciphertext_chunk_bytes = plaintext_chunk_bytes
        .checked_add(XCHACHA20_POLY1305_TAG_BYTES_V1)
        .ok_or(AggregateStarkErrorV1::InvalidLayout)?;
    let record_count = width
        .checked_mul(chunks_per_column)
        .ok_or(AggregateStarkErrorV1::InvalidLayout)?;
    let expected_file_bytes = record_count
        .checked_mul(ciphertext_chunk_bytes)
        .and_then(|bytes| u64::try_from(bytes).ok())
        .ok_or(AggregateStarkErrorV1::InvalidLayout)?;
    if expected_file_bytes == 0 {
        return Err(AggregateStarkErrorV1::InvalidLayout);
    }
    Ok((
        chunks_per_column,
        plaintext_chunk_bytes,
        ciphertext_chunk_bytes,
        expected_file_bytes,
    ))
}

/// Sequential writer for one authenticated, anonymous field-matrix scratch.
///
/// Columns must be appended in their canonical commitment order. The file is
/// anonymous from creation, every fixed-size record is independently
/// XChaCha20-Poly1305 authenticated, and record coordinates and matrix
/// dimensions are included in AAD. Consequently truncation, extension,
/// substitution, reordering, and cross-matrix record reuse all fail closed.
/// The ephemeral key is drawn independently from operating-system entropy so
/// scratch encryption never perturbs deterministic proof-mask KATs.
#[cfg(test)]
pub(crate) struct EncryptedFieldMatrixScratchWriterV1 {
    file: std::fs::File,
    rows: usize,
    width: usize,
    chunk_rows: usize,
    chunks_per_column: usize,
    plaintext_chunk_bytes: usize,
    ciphertext_chunk_bytes: usize,
    expected_file_bytes: u64,
    next_column: usize,
    key: Zeroizing<[u8; 32]>,
    nonce_prefix: Zeroizing<[u8; XCHACHA20_NONCE_PREFIX_BYTES_V1]>,
}

#[cfg(test)]
fn encrypted_field_scratch_entropy_is_healthy_v1(
    key: &[u8; 32],
    nonce_prefix: &[u8; XCHACHA20_NONCE_PREFIX_BYTES_V1],
) -> bool {
    let key_constant = key.iter().all(|byte| *byte == key[0]);
    let nonce_constant = nonce_prefix.iter().all(|byte| *byte == nonce_prefix[0]);
    let repeated_prefix = key[..XCHACHA20_NONCE_PREFIX_BYTES_V1] == nonce_prefix[..];
    !key_constant && !nonce_constant && !repeated_prefix
}

#[cfg(test)]
std::thread_local! {
    static ENCRYPTED_SCRATCH_FILE_CREATION_ATTEMPTS_V1: std::cell::Cell<usize> =
        const { std::cell::Cell::new(0) };
}

#[cfg(test)]
fn create_anonymous_scratch_file_v1() -> Result<std::fs::File, AggregateStarkErrorV1> {
    #[cfg(test)]
    ENCRYPTED_SCRATCH_FILE_CREATION_ATTEMPTS_V1.with(|attempts| {
        attempts.set(attempts.get().saturating_add(1));
    });
    #[cfg(target_os = "linux")]
    {
        let descriptor = memfd_create(
            "iroha-aggregate-stark-scratch-v1",
            MemfdFlags::CLOEXEC | MemfdFlags::NOEXEC_SEAL,
        )
        .map_err(|_| AggregateStarkErrorV1::AllocationFailure)?;
        let file = std::fs::File::from(descriptor);
        file.set_permissions(std::fs::Permissions::from_mode(0o600))
            .map_err(|_| AggregateStarkErrorV1::AllocationFailure)?;
        let metadata = file
            .metadata()
            .map_err(|_| AggregateStarkErrorV1::AllocationFailure)?;
        let seals = fcntl_get_seals(&file).map_err(|_| AggregateStarkErrorV1::AllocationFailure)?;
        if !metadata.file_type().is_file()
            || metadata.nlink() != 0
            || metadata.mode() & 0o777 != 0o600
            || !seals.contains(SealFlags::EXEC)
        {
            return Err(AggregateStarkErrorV1::AllocationFailure);
        }
        Ok(file)
    }
    #[cfg(not(target_os = "linux"))]
    {
        tempfile::tempfile().map_err(|_| AggregateStarkErrorV1::AllocationFailure)
    }
}

#[cfg(test)]
fn encrypted_scratch_file_creation_attempts_v1() -> usize {
    ENCRYPTED_SCRATCH_FILE_CREATION_ATTEMPTS_V1.with(std::cell::Cell::get)
}

#[cfg(test)]
impl EncryptedFieldMatrixScratchWriterV1 {
    /// Create an owner-private anonymous scratch with an independent ephemeral
    /// encryption key.
    pub(crate) fn new(
        rows: usize,
        width: usize,
        chunk_rows: usize,
    ) -> Result<Self, AggregateStarkErrorV1> {
        Self::new_with_rng(rows, width, chunk_rows, &mut rand::rngs::OsRng)
    }

    /// Create a scratch writer with injected entropy.
    ///
    /// Shape validation and the complete key/nonce-prefix health check happen
    /// before the anonymous backing file is created. Both secret buffers are
    /// recursively zeroized on every early return and when the sealed scratch
    /// is dropped.
    pub(crate) fn new_with_rng<R: TryRngCore>(
        rows: usize,
        width: usize,
        chunk_rows: usize,
        rng: &mut R,
    ) -> Result<Self, AggregateStarkErrorV1> {
        let (chunks_per_column, plaintext_chunk_bytes, ciphertext_chunk_bytes, expected_file_bytes) =
            encrypted_field_scratch_shape_v1(rows, width, chunk_rows)?;
        let mut key = Zeroizing::new([0_u8; 32]);
        let mut nonce_prefix = Zeroizing::new([0_u8; XCHACHA20_NONCE_PREFIX_BYTES_V1]);
        rng.try_fill_bytes(key.as_mut())
            .map_err(|_| AggregateStarkErrorV1::RandomnessUnavailable)?;
        rng.try_fill_bytes(nonce_prefix.as_mut())
            .map_err(|_| AggregateStarkErrorV1::RandomnessUnavailable)?;
        if !encrypted_field_scratch_entropy_is_healthy_v1(&key, &nonce_prefix) {
            return Err(AggregateStarkErrorV1::RandomnessUnavailable);
        }
        let file = create_anonymous_scratch_file_v1()?;
        Ok(Self {
            file,
            rows,
            width,
            chunk_rows,
            chunks_per_column,
            plaintext_chunk_bytes,
            ciphertext_chunk_bytes,
            expected_file_bytes,
            next_column: 0,
            key,
            nonce_prefix,
        })
    }

    fn record_index(&self, column: usize, chunk: usize) -> Result<u64, AggregateStarkErrorV1> {
        if column >= self.width || chunk >= self.chunks_per_column {
            return Err(AggregateStarkErrorV1::InvalidLayout);
        }
        column
            .checked_mul(self.chunks_per_column)
            .and_then(|index| index.checked_add(chunk))
            .and_then(|index| u64::try_from(index).ok())
            .ok_or(AggregateStarkErrorV1::InvalidLayout)
    }

    /// Encrypt and append the next exact column.
    pub(crate) fn append_column(&mut self, column: &[F]) -> Result<(), AggregateStarkErrorV1> {
        if self.next_column >= self.width || column.len() != self.rows {
            return Err(AggregateStarkErrorV1::InvalidProofShape);
        }
        let cipher = XChaCha20Poly1305::new_from_slice(self.key.as_ref())
            .map_err(|_| AggregateStarkErrorV1::InternalInvariant)?;
        for chunk in 0..self.chunks_per_column {
            let start = chunk
                .checked_mul(self.chunk_rows)
                .ok_or(AggregateStarkErrorV1::InvalidLayout)?;
            let end = start
                .checked_add(self.chunk_rows)
                .ok_or(AggregateStarkErrorV1::InvalidLayout)?;
            let mut plaintext = Zeroizing::new(Vec::new());
            plaintext
                .try_reserve_exact(self.plaintext_chunk_bytes)
                .map_err(|_| AggregateStarkErrorV1::AllocationFailure)?;
            for value in column
                .get(start..end)
                .ok_or(AggregateStarkErrorV1::InvalidProofShape)?
            {
                plaintext.extend_from_slice(&value.0.to_be_bytes());
            }
            if plaintext.len() != self.plaintext_chunk_bytes {
                return Err(AggregateStarkErrorV1::InternalInvariant);
            }
            let record_index = self.record_index(self.next_column, chunk)?;
            let nonce = encrypted_field_scratch_nonce_v1(&self.nonce_prefix, record_index);
            let aad = encrypted_field_scratch_record_aad_v1(
                self.rows,
                self.width,
                self.chunk_rows,
                record_index,
            )?;
            let ciphertext = cipher
                .encrypt(
                    &nonce,
                    Payload {
                        msg: plaintext.as_slice(),
                        aad: &aad,
                    },
                )
                .map_err(|_| AggregateStarkErrorV1::InternalInvariant)?;
            if ciphertext.len() != self.ciphertext_chunk_bytes {
                return Err(AggregateStarkErrorV1::InternalInvariant);
            }
            self.file
                .write_all(&ciphertext)
                .map_err(|_| AggregateStarkErrorV1::AllocationFailure)?;
        }
        self.next_column += 1;
        Ok(())
    }

    /// Seal the exact matrix after every declared column was appended.
    pub(crate) fn finish(mut self) -> Result<EncryptedFieldMatrixScratchV1, AggregateStarkErrorV1> {
        if self.next_column != self.width {
            return Err(AggregateStarkErrorV1::InvalidProofShape);
        }
        self.file
            .flush()
            .map_err(|_| AggregateStarkErrorV1::AllocationFailure)?;
        let position = self
            .file
            .stream_position()
            .map_err(|_| AggregateStarkErrorV1::AllocationFailure)?;
        let actual = self
            .file
            .metadata()
            .map_err(|_| AggregateStarkErrorV1::AllocationFailure)?
            .len();
        if position != self.expected_file_bytes || actual != self.expected_file_bytes {
            return Err(AggregateStarkErrorV1::InternalInvariant);
        }
        Ok(EncryptedFieldMatrixScratchV1 {
            file: self.file,
            rows: self.rows,
            width: self.width,
            chunk_rows: self.chunk_rows,
            chunks_per_column: self.chunks_per_column,
            plaintext_chunk_bytes: self.plaintext_chunk_bytes,
            ciphertext_chunk_bytes: self.ciphertext_chunk_bytes,
            expected_file_bytes: self.expected_file_bytes,
            key: self.key,
            nonce_prefix: self.nonce_prefix,
        })
    }
}

/// Decrypted row-major view of one authenticated scratch chunk.
///
/// This type deliberately implements neither `Clone` nor `Debug`; all
/// decrypted field cells are overwritten when it leaves scope.
#[cfg(test)]
pub(crate) struct EncryptedFieldMatrixBlockV1 {
    row_start: usize,
    row_count: usize,
    width: usize,
    values: Vec<F>,
}

#[cfg(test)]
impl EncryptedFieldMatrixBlockV1 {
    /// First common-domain row represented by this block.
    pub(crate) fn row_start(&self) -> usize {
        self.row_start
    }

    /// Number of rows represented by this block.
    pub(crate) fn row_count(&self) -> usize {
        self.row_count
    }

    /// Return one exact row by its global common-domain index.
    pub(crate) fn row(&self, index: usize) -> Result<&[F], AggregateStarkErrorV1> {
        let local = index
            .checked_sub(self.row_start)
            .filter(|local| *local < self.row_count)
            .ok_or(AggregateStarkErrorV1::InvalidLayout)?;
        let start = local
            .checked_mul(self.width)
            .ok_or(AggregateStarkErrorV1::InvalidLayout)?;
        let end = start
            .checked_add(self.width)
            .ok_or(AggregateStarkErrorV1::InvalidLayout)?;
        self.values
            .get(start..end)
            .ok_or(AggregateStarkErrorV1::InternalInvariant)
    }
}

#[cfg(test)]
impl Drop for EncryptedFieldMatrixBlockV1 {
    fn drop(&mut self) {
        for value in &mut self.values {
            value.0 = 0;
        }
    }
}

/// Sealed authenticated anonymous storage for one masked LDE matrix.
///
/// Only masked polynomial evaluations are ever written. The anonymous file is
/// closed and unlinked on drop, and its ephemeral key is recursively zeroized.
/// At most one fixed-size row block is decrypted by [`Self::read_chunk`].
#[cfg(test)]
pub(crate) struct EncryptedFieldMatrixScratchV1 {
    file: std::fs::File,
    rows: usize,
    width: usize,
    chunk_rows: usize,
    chunks_per_column: usize,
    plaintext_chunk_bytes: usize,
    ciphertext_chunk_bytes: usize,
    expected_file_bytes: u64,
    key: Zeroizing<[u8; 32]>,
    nonce_prefix: Zeroizing<[u8; XCHACHA20_NONCE_PREFIX_BYTES_V1]>,
}

#[cfg(test)]
impl EncryptedFieldMatrixScratchV1 {
    /// Number of common-domain rows in the stored matrix.
    pub(crate) fn rows(&self) -> usize {
        self.rows
    }

    /// Number of columns in the stored matrix.
    pub(crate) fn width(&self) -> usize {
        self.width
    }

    /// Number of rows in every independently authenticated record.
    pub(crate) fn chunk_rows(&self) -> usize {
        self.chunk_rows
    }

    /// Number of row chunks in every column.
    pub(crate) fn chunk_count(&self) -> usize {
        self.chunks_per_column
    }

    /// Exact ciphertext bytes occupied by the anonymous backing file.
    pub(crate) fn ciphertext_bytes(&self) -> u64 {
        self.expected_file_bytes
    }

    fn record_index(&self, column: usize, chunk: usize) -> Result<u64, AggregateStarkErrorV1> {
        if column >= self.width || chunk >= self.chunks_per_column {
            return Err(AggregateStarkErrorV1::InvalidLayout);
        }
        column
            .checked_mul(self.chunks_per_column)
            .and_then(|index| index.checked_add(chunk))
            .and_then(|index| u64::try_from(index).ok())
            .ok_or(AggregateStarkErrorV1::InvalidLayout)
    }

    fn validate_backing_file(&self) -> Result<(), AggregateStarkErrorV1> {
        let actual = self
            .file
            .metadata()
            .map_err(|_| AggregateStarkErrorV1::InternalInvariant)?
            .len();
        if actual != self.expected_file_bytes {
            return Err(AggregateStarkErrorV1::InternalInvariant);
        }
        Ok(())
    }

    /// Authenticate, decrypt, and transpose one row chunk.
    pub(crate) fn read_chunk(
        &mut self,
        chunk: usize,
    ) -> Result<EncryptedFieldMatrixBlockV1, AggregateStarkErrorV1> {
        if chunk >= self.chunks_per_column {
            return Err(AggregateStarkErrorV1::InvalidLayout);
        }
        self.validate_backing_file()?;
        let value_count = self
            .chunk_rows
            .checked_mul(self.width)
            .ok_or(AggregateStarkErrorV1::InvalidLayout)?;
        let mut values = Vec::new();
        values
            .try_reserve_exact(value_count)
            .map_err(|_| AggregateStarkErrorV1::AllocationFailure)?;
        values.resize(value_count, F::ZERO);
        let cipher = XChaCha20Poly1305::new_from_slice(self.key.as_ref())
            .map_err(|_| AggregateStarkErrorV1::InternalInvariant)?;
        let mut ciphertext = Vec::new();
        ciphertext
            .try_reserve_exact(self.ciphertext_chunk_bytes)
            .map_err(|_| AggregateStarkErrorV1::AllocationFailure)?;
        ciphertext.resize(self.ciphertext_chunk_bytes, 0);
        for column in 0..self.width {
            let record_index = self.record_index(column, chunk)?;
            let offset = record_index
                .checked_mul(
                    u64::try_from(self.ciphertext_chunk_bytes)
                        .map_err(|_| AggregateStarkErrorV1::InvalidLayout)?,
                )
                .ok_or(AggregateStarkErrorV1::InvalidLayout)?;
            self.file
                .seek(std::io::SeekFrom::Start(offset))
                .map_err(|_| AggregateStarkErrorV1::InternalInvariant)?;
            self.file
                .read_exact(&mut ciphertext)
                .map_err(|_| AggregateStarkErrorV1::InternalInvariant)?;
            let nonce = encrypted_field_scratch_nonce_v1(&self.nonce_prefix, record_index);
            let aad = encrypted_field_scratch_record_aad_v1(
                self.rows,
                self.width,
                self.chunk_rows,
                record_index,
            )?;
            let plaintext = Zeroizing::new(
                cipher
                    .decrypt(
                        &nonce,
                        Payload {
                            msg: &ciphertext,
                            aad: &aad,
                        },
                    )
                    .map_err(|_| AggregateStarkErrorV1::InternalInvariant)?,
            );
            if plaintext.len() != self.plaintext_chunk_bytes {
                return Err(AggregateStarkErrorV1::InternalInvariant);
            }
            for row in 0..self.chunk_rows {
                let byte_start = row
                    .checked_mul(core::mem::size_of::<u64>())
                    .ok_or(AggregateStarkErrorV1::InvalidLayout)?;
                let bytes: [u8; 8] = plaintext
                    .get(byte_start..byte_start + core::mem::size_of::<u64>())
                    .ok_or(AggregateStarkErrorV1::InternalInvariant)?
                    .try_into()
                    .map_err(|_| AggregateStarkErrorV1::InternalInvariant)?;
                let value = F::canonical(u64::from_be_bytes(bytes))
                    .ok_or(AggregateStarkErrorV1::InternalInvariant)?;
                let destination = row
                    .checked_mul(self.width)
                    .and_then(|index| index.checked_add(column))
                    .ok_or(AggregateStarkErrorV1::InvalidLayout)?;
                values[destination] = value;
            }
        }
        let row_start = chunk
            .checked_mul(self.chunk_rows)
            .ok_or(AggregateStarkErrorV1::InvalidLayout)?;
        Ok(EncryptedFieldMatrixBlockV1 {
            row_start,
            row_count: self.chunk_rows,
            width: self.width,
            values,
        })
    }
}

/// Commit the exact row framing of a sealed encrypted scratch while retaining
/// only one decrypted block, logarithmic Merkle state, and requested rows.
#[cfg(test)]
pub(crate) fn commit_encrypted_field_scratch_rows_v1(
    leaf_domain: &[u8],
    node_domain: &'static [u8],
    group: usize,
    opening_indices: &[usize],
    scratch: &mut EncryptedFieldMatrixScratchV1,
) -> Result<StreamingRowCommitmentResultV1, AggregateStarkErrorV1> {
    if leaf_domain.is_empty() || node_domain.is_empty() {
        return Err(AggregateStarkErrorV1::InvalidLayout);
    }
    if !opening_indices.is_empty() {
        validate_canonical_index_set_v1(scratch.rows, opening_indices)?;
    }
    let mut merkle = StreamingMerkleAccumulatorV1::new(node_domain, scratch.rows, opening_indices)?;
    let mut opened_rows = BTreeMap::new();
    for &index in opening_indices {
        let mut row = Vec::new();
        row.try_reserve_exact(scratch.width)
            .map_err(|_| AggregateStarkErrorV1::AllocationFailure)?;
        if opened_rows.insert(index, row).is_some() {
            return Err(AggregateStarkErrorV1::InvalidProofShape);
        }
    }
    for chunk in 0..scratch.chunks_per_column {
        let block = scratch.read_chunk(chunk)?;
        let end = block
            .row_start()
            .checked_add(block.row_count())
            .ok_or(AggregateStarkErrorV1::InvalidLayout)?;
        for index in block.row_start()..end {
            let row = block.row(index)?;
            if let Some(opening) = opened_rows.get_mut(&index) {
                opening.extend_from_slice(row);
            }
            merkle.append_leaf(row_leaf_hash_v1(leaf_domain, group, row)?)?;
        }
    }
    if opened_rows
        .values()
        .any(|opening| opening.len() != scratch.width)
    {
        return Err(AggregateStarkErrorV1::InternalInvariant);
    }
    Ok(StreamingRowCommitmentResultV1 {
        commitment: merkle.finish()?,
        opened_rows,
    })
}

/// Replay a masked trace into authenticated anonymous scratch storage without
/// retaining more than one native column and one LDE column in memory.
#[cfg(test)]
pub(crate) fn spill_replayed_masked_trace_columns_v1<S>(
    masks: &StreamingTraceMaskSetV1,
    mut source: S,
) -> Result<EncryptedFieldMatrixScratchV1, AggregateStarkErrorV1>
where
    S: FnMut(usize) -> Result<Vec<F>, AggregateStarkErrorV1>,
{
    let native_rows = checked_domain_size_v1(masks.native_trace_log2)?;
    let lde_rows = checked_domain_size_v1(masks.lde_log2)?;
    let chunk_rows = DEFAULT_ENCRYPTED_TRACE_SCRATCH_CHUNK_ROWS_V1.min(lde_rows);
    let mut writer = EncryptedFieldMatrixScratchWriterV1::new(lde_rows, masks.width(), chunk_rows)?;
    for (column_index, mask) in masks.masks.iter().enumerate() {
        let native = ZeroizingFieldColumnV1(source(column_index)?);
        if native.len() != native_rows {
            return Err(AggregateStarkErrorV1::InvalidLayout);
        }
        let lde = ZeroizingFieldColumnV1(
            masked_trace_lde_column_with_mask_v1(
                &native,
                masks.native_trace_log2,
                masks.lde_log2,
                mask.coefficients(),
            )
            .map_err(map_transparent_error_v1)?,
        );
        writer.append_column(&lde)?;
    }
    writer.finish()
}

/// Sample masks, commit, and retain the exact encrypted masked-LDE matrix with
/// an explicitly bounded authenticated scratch-record height.
///
/// This is the canonical path for provers that need the committed trace again
/// for composition or post-query openings. Returning the sealed scratch
/// prevents a second interpolation/FFT pass while retaining only one native
/// column, one LDE column, and one decrypted row block in resident memory.
#[cfg(test)]
pub(crate) fn commit_masked_trace_columns_retaining_encrypted_scratch_with_chunk_rows_v1<R, S>(
    leaf_domain: &[u8],
    node_domain: &'static [u8],
    group: usize,
    native_trace_log2: u8,
    lde_log2: u8,
    width: usize,
    mask_degree: usize,
    scratch_chunk_rows: usize,
    opening_indices: &[usize],
    rng: &mut R,
    mut source: S,
) -> Result<
    (
        StreamingRowCommitmentResultV1,
        StreamingTraceMaskSetV1,
        EncryptedFieldMatrixScratchV1,
    ),
    AggregateStarkErrorV1,
>
where
    R: TryRngCore,
    S: FnMut(usize) -> Result<Vec<F>, AggregateStarkErrorV1>,
{
    let (native_rows, lde_rows) = validate_masked_trace_commitment_shape_v1(
        leaf_domain,
        node_domain,
        group,
        native_trace_log2,
        lde_log2,
        width,
        mask_degree,
        opening_indices,
    )?;
    if scratch_chunk_rows == 0
        || !scratch_chunk_rows.is_power_of_two()
        || scratch_chunk_rows > lde_rows
        || lde_rows % scratch_chunk_rows != 0
    {
        return Err(AggregateStarkErrorV1::InvalidLayout);
    }
    let mut writer = EncryptedFieldMatrixScratchWriterV1::new(lde_rows, width, scratch_chunk_rows)?;
    let mut masks = Vec::new();
    masks
        .try_reserve_exact(width)
        .map_err(|_| AggregateStarkErrorV1::AllocationFailure)?;
    for column_index in 0..width {
        let native = ZeroizingFieldColumnV1(source(column_index)?);
        if native.len() != native_rows {
            return Err(AggregateStarkErrorV1::InvalidLayout);
        }
        let mask = sample_trace_mask_v1(mask_degree, rng).map_err(map_transparent_error_v1)?;
        let lde = ZeroizingFieldColumnV1(
            masked_trace_lde_column_with_mask_v1(
                &native,
                native_trace_log2,
                lde_log2,
                mask.coefficients(),
            )
            .map_err(map_transparent_error_v1)?,
        );
        writer.append_column(&lde)?;
        masks.push(mask);
    }
    let mut scratch = writer.finish()?;
    let commitment = commit_encrypted_field_scratch_rows_v1(
        leaf_domain,
        node_domain,
        group,
        opening_indices,
        &mut scratch,
    )?;
    Ok((
        commitment,
        StreamingTraceMaskSetV1 {
            native_trace_log2,
            lde_log2,
            masks,
        },
        scratch,
    ))
}

/// Sample masks, commit, and retain the encrypted masked-LDE matrix using the
/// generic bounded scratch-record height.
#[cfg(test)]
pub(crate) fn commit_masked_trace_columns_retaining_encrypted_scratch_v1<R, S>(
    leaf_domain: &[u8],
    node_domain: &'static [u8],
    group: usize,
    native_trace_log2: u8,
    lde_log2: u8,
    width: usize,
    mask_degree: usize,
    opening_indices: &[usize],
    rng: &mut R,
    source: S,
) -> Result<
    (
        StreamingRowCommitmentResultV1,
        StreamingTraceMaskSetV1,
        EncryptedFieldMatrixScratchV1,
    ),
    AggregateStarkErrorV1,
>
where
    R: TryRngCore,
    S: FnMut(usize) -> Result<Vec<F>, AggregateStarkErrorV1>,
{
    let lde_rows = checked_domain_size_v1(lde_log2)?;
    commit_masked_trace_columns_retaining_encrypted_scratch_with_chunk_rows_v1(
        leaf_domain,
        node_domain,
        group,
        native_trace_log2,
        lde_log2,
        width,
        mask_degree,
        DEFAULT_ENCRYPTED_TRACE_SCRATCH_CHUNK_ROWS_V1.min(lde_rows),
        opening_indices,
        rng,
        source,
    )
}

/// Low-resident-memory root-only alternative to
/// [`commit_masked_trace_columns_v1`].
///
/// Callers that need composition or later openings must use
/// [`commit_masked_trace_columns_retaining_encrypted_scratch_v1`] so the
/// already-computed masked LDE is not discarded and recomputed.
#[cfg(test)]
pub(crate) fn commit_masked_trace_columns_via_encrypted_scratch_v1<R, S>(
    leaf_domain: &[u8],
    node_domain: &'static [u8],
    group: usize,
    native_trace_log2: u8,
    lde_log2: u8,
    width: usize,
    mask_degree: usize,
    opening_indices: &[usize],
    rng: &mut R,
    source: S,
) -> Result<(StreamingRowCommitmentResultV1, StreamingTraceMaskSetV1), AggregateStarkErrorV1>
where
    R: TryRngCore,
    S: FnMut(usize) -> Result<Vec<F>, AggregateStarkErrorV1>,
{
    let (commitment, masks, scratch) = commit_masked_trace_columns_retaining_encrypted_scratch_v1(
        leaf_domain,
        node_domain,
        group,
        native_trace_log2,
        lde_log2,
        width,
        mask_degree,
        opening_indices,
        rng,
        source,
    )?;
    drop(scratch);
    Ok((commitment, masks))
}

/// Low-resident-memory replay counterpart for an already sampled mask set.
#[cfg(test)]
pub(crate) fn replay_masked_trace_columns_via_encrypted_scratch_v1<S>(
    leaf_domain: &[u8],
    node_domain: &'static [u8],
    group: usize,
    masks: &StreamingTraceMaskSetV1,
    opening_indices: &[usize],
    source: S,
) -> Result<StreamingRowCommitmentResultV1, AggregateStarkErrorV1>
where
    S: FnMut(usize) -> Result<Vec<F>, AggregateStarkErrorV1>,
{
    let mut scratch = spill_replayed_masked_trace_columns_v1(masks, source)?;
    commit_encrypted_field_scratch_rows_v1(
        leaf_domain,
        node_domain,
        group,
        opening_indices,
        &mut scratch,
    )
}

/// Commit one aggregate composition lane.
pub(crate) fn composition_tree_v1(
    domains: AggregateStarkDomainsV1,
    lane: usize,
    chunks: &[Vec<E>],
) -> Result<Sha256MerkleTreeV1, AggregateStarkErrorV1> {
    domains.validate()?;
    let rows = chunks
        .first()
        .map(Vec::len)
        .filter(|rows| *rows != 0 && rows.is_power_of_two())
        .ok_or(AggregateStarkErrorV1::InvalidLayout)?;
    if chunks.iter().any(|chunk| chunk.len() != rows) {
        return Err(AggregateStarkErrorV1::InvalidLayout);
    }
    let leaves = (0..rows)
        .map(|index| {
            let values = chunks.iter().map(|chunk| chunk[index]).collect::<Vec<_>>();
            composition_leaf_hash_unchecked_v1(domains, lane, &values)
        })
        .collect::<Result<Vec<_>, _>>()?;
    Sha256MerkleTreeV1::from_leaves(leaves, domains.composition_node)
        .map_err(map_transparent_error_v1)
}

/// Commit one aggregate composition lane without retaining a Merkle tree.
#[cfg(any(test, feature = "privacy-release-evidence"))]
pub(crate) fn streaming_composition_commitment_v1(
    domains: AggregateStarkDomainsV1,
    lane: usize,
    chunks: &[Vec<E>],
    opening_indices: &[usize],
) -> Result<StreamingMerkleCommitmentV1, AggregateStarkErrorV1> {
    domains.validate()?;
    let rows = chunks
        .first()
        .map(Vec::len)
        .filter(|rows| *rows != 0 && rows.is_power_of_two())
        .ok_or(AggregateStarkErrorV1::InvalidLayout)?;
    if chunks.iter().any(|chunk| chunk.len() != rows) {
        return Err(AggregateStarkErrorV1::InvalidLayout);
    }
    streaming_merkle_commitment_v1(
        domains.composition_node,
        rows,
        opening_indices,
        (0..rows).map(|index| {
            let values = chunks.iter().map(|chunk| chunk[index]).collect::<Vec<_>>();
            composition_leaf_hash_unchecked_v1(domains, lane, &values)
        }),
    )
}

fn composition_chunks_from_coefficients_v1(
    coefficients: &[E],
    parameters: AggregateStarkParametersV1,
    layout: &AggregateProofLayoutV1,
) -> Result<Vec<Vec<E>>, AggregateStarkErrorV1> {
    layout.validate(parameters)?;
    let lde_size = layout.common_lde_size();
    if coefficients
        .iter()
        .any(|coefficient| !coefficient.is_canonical())
    {
        return Err(AggregateStarkErrorV1::NonCanonicalField);
    }
    let root =
        goldilocks_primitive_root_v1(layout.common_lde_log2).map_err(map_transparent_error_v1)?;
    let chunk_size = layout.fri_degree_cap(parameters)?;
    let mut chunks = Vec::new();
    chunks
        .try_reserve_exact(parameters.composition_degree_chunks)
        .map_err(|_| AggregateStarkErrorV1::AllocationFailure)?;
    for chunk in 0..parameters.composition_degree_chunks {
        let start = chunk
            .checked_mul(chunk_size)
            .ok_or(AggregateStarkErrorV1::InvalidLayout)?;
        let coefficient_chunk = if start >= coefficients.len() {
            &[]
        } else {
            let end = start
                .checked_add(chunk_size)
                .map(|end| end.min(coefficients.len()))
                .ok_or(AggregateStarkErrorV1::InvalidLayout)?;
            coefficients
                .get(start..end)
                .ok_or(AggregateStarkErrorV1::InternalInvariant)?
        };
        chunks.push(
            goldilocks_fp4_evaluate_coset_v1(
                coefficient_chunk,
                lde_size,
                root,
                F(GOLDILOCKS_GENERATOR_V1),
            )
            .map_err(map_transparent_error_v1)?,
        );
    }
    Ok(chunks)
}

/// Split one common-domain quotient codeword into canonical FRI chunks.
pub(crate) fn split_composition_evaluations_v1(
    evaluations: &[E],
    parameters: AggregateStarkParametersV1,
    layout: &AggregateProofLayoutV1,
) -> Result<Vec<Vec<E>>, AggregateStarkErrorV1> {
    layout.validate(parameters)?;
    let lde_size = layout.common_lde_size();
    if evaluations.len() != lde_size {
        return Err(AggregateStarkErrorV1::InvalidLayout);
    }
    let coefficients = fp4_coset_coefficients_v1(evaluations, layout.common_lde_log2)?;
    let covered_coefficients = layout
        .fri_degree_cap(parameters)?
        .checked_mul(parameters.composition_degree_chunks)
        .ok_or(AggregateStarkErrorV1::InvalidLayout)?;
    if coefficients
        .0
        .get(covered_coefficients..)
        .is_some_and(|high| high.iter().any(|coefficient| *coefficient != E::ZERO))
    {
        return Err(AggregateStarkErrorV1::FriDegree);
    }
    composition_chunks_from_coefficients_v1(&coefficients, parameters, layout)
}

/// Divide extension coefficients exactly by the trace vanishing polynomial.
///
/// Synthetic division is performed by the monic polynomial `X^n - 1`. The
/// complete remainder is checked to be zero before the quotient is returned;
/// a numerator that is only pointwise divisible on some evaluation set is
/// therefore rejected.
#[cfg(test)]
pub(crate) fn divide_extension_polynomial_by_trace_vanishing_v1(
    numerator_coefficients: &[E],
    trace_log2: u8,
) -> Result<Vec<E>, AggregateStarkErrorV1> {
    let trace_size = checked_domain_size_v1(trace_log2)?;
    if numerator_coefficients.len() <= trace_size {
        return Err(AggregateStarkErrorV1::InvalidLayout);
    }
    if numerator_coefficients
        .iter()
        .any(|coefficient| !coefficient.is_canonical())
    {
        return Err(AggregateStarkErrorV1::NonCanonicalField);
    }
    let quotient_len = numerator_coefficients
        .len()
        .checked_sub(trace_size)
        .ok_or(AggregateStarkErrorV1::InvalidLayout)?;
    let mut work = ZeroizingExtensionFieldColumnV1(Vec::new());
    work.0
        .try_reserve_exact(numerator_coefficients.len())
        .map_err(|_| AggregateStarkErrorV1::AllocationFailure)?;
    work.0.extend_from_slice(numerator_coefficients);
    let mut quotient = ZeroizingExtensionFieldColumnV1(Vec::new());
    quotient
        .0
        .try_reserve_exact(quotient_len)
        .map_err(|_| AggregateStarkErrorV1::AllocationFailure)?;
    quotient.0.resize(quotient_len, E::ZERO);
    for degree in (trace_size..work.len()).rev() {
        let factor = work[degree];
        let quotient_degree = degree
            .checked_sub(trace_size)
            .ok_or(AggregateStarkErrorV1::InternalInvariant)?;
        quotient.0[quotient_degree] = factor;
        work.0[degree] = E::ZERO;
        let remainder_coefficient = work[quotient_degree].add(factor);
        work.0[quotient_degree] = remainder_coefficient;
    }
    if work.iter().any(|coefficient| *coefficient != E::ZERO) {
        return Err(AggregateStarkErrorV1::ConstraintOpening);
    }
    Ok(core::mem::take(&mut quotient.0))
}

/// Divide a constraint-numerator codeword by `X^n - 1` on a quotient coset.
///
/// The generator shift is checked to be disjoint from both the native trace
/// subgroup and the quotient evaluation subgroup. Only `Q / n` denominators
/// are materialized and batch-inverted because the vanishing values repeat
/// with that exact period. Every pointwise division is multiplied back as an
/// implementation invariant.
#[cfg(test)]
pub(crate) fn quotient_evaluations_from_constraint_coset_v1(
    numerator_evaluations: &[E],
    trace_log2: u8,
    quotient_coset_log2: u8,
) -> Result<Vec<E>, AggregateStarkErrorV1> {
    let trace_size = checked_domain_size_v1(trace_log2)?;
    let quotient_size = checked_domain_size_v1(quotient_coset_log2)?;
    if trace_size >= quotient_size || numerator_evaluations.len() != quotient_size {
        return Err(AggregateStarkErrorV1::InvalidLayout);
    }
    if numerator_evaluations
        .iter()
        .any(|evaluation| !evaluation.is_canonical())
    {
        return Err(AggregateStarkErrorV1::NonCanonicalField);
    }
    let quotient_root = goldilocks_primitive_root_v1(quotient_coset_log2)
        .map_err(|_| AggregateStarkErrorV1::InvalidLayout)?;
    let shift = F(GOLDILOCKS_GENERATOR_V1);
    if shift.pow(trace_size as u128) == F::ONE || shift.pow(quotient_size as u128) == F::ONE {
        return Err(AggregateStarkErrorV1::InvalidLayout);
    }
    let denominator_period = quotient_size
        .checked_div(trace_size)
        .filter(|period| *period != 0)
        .ok_or(AggregateStarkErrorV1::InvalidLayout)?;
    let step = quotient_root.pow(trace_size as u128);
    let mut denominators = Vec::new();
    denominators
        .try_reserve_exact(denominator_period)
        .map_err(|_| AggregateStarkErrorV1::AllocationFailure)?;
    let mut point_to_trace_size = shift.pow(trace_size as u128);
    for _ in 0..denominator_period {
        denominators.push(point_to_trace_size.sub(F::ONE));
        point_to_trace_size = point_to_trace_size.mul(step);
    }
    let mut original_denominators = Vec::new();
    original_denominators
        .try_reserve_exact(denominator_period)
        .map_err(|_| AggregateStarkErrorV1::AllocationFailure)?;
    original_denominators.extend_from_slice(&denominators);
    goldilocks_batch_invert_v1(&mut denominators).map_err(map_transparent_error_v1)?;
    let mut quotient = Vec::new();
    quotient
        .try_reserve_exact(quotient_size)
        .map_err(|_| AggregateStarkErrorV1::AllocationFailure)?;
    for (index, numerator) in numerator_evaluations.iter().copied().enumerate() {
        let period_index = index % denominator_period;
        let value = numerator.mul_base(denominators[period_index]);
        if value.mul_base(original_denominators[period_index]) != numerator {
            return Err(AggregateStarkErrorV1::InternalInvariant);
        }
        quotient.push(value);
    }
    Ok(quotient)
}

/// Convert a minimal quotient-coset codeword into common-domain FRI chunks.
///
/// `maximum_quotient_degree` is the relation's exact inclusive `q_max`, not
/// the looser aggregate layout capacity. After interpolation, every
/// coefficient above that bound must be exactly zero. The canonical
/// coefficient chunks are then evaluated on the common commitment coset, so
/// the resulting proof wire remains independent of the prover's smaller
/// quotient domain.
#[cfg(test)]
pub(crate) fn composition_chunks_from_quotient_coset_v1(
    quotient_evaluations: &[E],
    quotient_coset_log2: u8,
    maximum_quotient_degree: usize,
    parameters: AggregateStarkParametersV1,
    layout: &AggregateProofLayoutV1,
) -> Result<Vec<Vec<E>>, AggregateStarkErrorV1> {
    layout.validate(parameters)?;
    let quotient_size = checked_domain_size_v1(quotient_coset_log2)?;
    if quotient_evaluations.len() != quotient_size
        || maximum_quotient_degree >= quotient_size
        || maximum_quotient_degree > layout.maximum_composition_degree(parameters)?
    {
        return Err(AggregateStarkErrorV1::InvalidLayout);
    }
    if quotient_evaluations
        .iter()
        .any(|evaluation| !evaluation.is_canonical())
    {
        return Err(AggregateStarkErrorV1::NonCanonicalField);
    }
    let coefficients = fp4_coset_coefficients_v1(quotient_evaluations, quotient_coset_log2)?;
    let first_forbidden = maximum_quotient_degree
        .checked_add(1)
        .ok_or(AggregateStarkErrorV1::InvalidLayout)?;
    if coefficients
        .get(first_forbidden..)
        .is_some_and(|tail| tail.iter().any(|coefficient| *coefficient != E::ZERO))
    {
        return Err(AggregateStarkErrorV1::FriDegree);
    }
    composition_chunks_from_coefficients_v1(&coefficients, parameters, layout)
}

/// Divide one constraint coset and canonically chunk the exact quotient.
#[cfg(test)]
pub(crate) fn composition_chunks_from_constraint_coset_v1(
    numerator_evaluations: &[E],
    trace_log2: u8,
    quotient_coset_log2: u8,
    maximum_quotient_degree: usize,
    parameters: AggregateStarkParametersV1,
    layout: &AggregateProofLayoutV1,
) -> Result<Vec<Vec<E>>, AggregateStarkErrorV1> {
    let quotient = ZeroizingExtensionFieldColumnV1(quotient_evaluations_from_constraint_coset_v1(
        numerator_evaluations,
        trace_log2,
        quotient_coset_log2,
    )?);
    composition_chunks_from_quotient_coset_v1(
        &quotient,
        quotient_coset_log2,
        maximum_quotient_degree,
        parameters,
        layout,
    )
}

/// Reconstruct one unsplit quotient value from its authenticated chunks.
pub(crate) fn recompose_composition_value_v1(
    chunks: &[E],
    x: F,
    parameters: AggregateStarkParametersV1,
    layout: &AggregateProofLayoutV1,
) -> Result<E, AggregateStarkErrorV1> {
    if chunks.len() != parameters.composition_degree_chunks {
        return Err(AggregateStarkErrorV1::InvalidProofShape);
    }
    let chunk_power = x.pow(layout.fri_degree_cap(parameters)? as u128);
    let mut power = F::ONE;
    let mut value = E::ZERO;
    for chunk in chunks {
        value = value.add(chunk.mul_base(power));
        power = power.mul(chunk_power);
    }
    Ok(value)
}

#[cfg(any(test, feature = "privacy-release-evidence"))]
fn evaluate_base_coefficients_at_fp4_v1(coefficients: &[F], point: E) -> E {
    coefficients
        .iter()
        .rev()
        .copied()
        .fold(E::ZERO, |value, coefficient| {
            value.mul(point).add(E::from_base(coefficient))
        })
}

fn evaluate_fp4_coefficients_at_fp4_v1(coefficients: &[E], point: E) -> E {
    coefficients
        .iter()
        .rev()
        .copied()
        .fold(E::ZERO, |value, coefficient| {
            value.mul(point).add(coefficient)
        })
}

#[cfg(any(test, feature = "privacy-release-evidence"))]
fn base_coset_coefficients_v1(
    evaluations: &[F],
    lde_log2: u8,
) -> Result<ZeroizingFieldColumnV1, AggregateStarkErrorV1> {
    if evaluations.len() != checked_domain_size_v1(lde_log2)? {
        return Err(AggregateStarkErrorV1::InvalidLayout);
    }
    let root = goldilocks_primitive_root_v1(lde_log2).map_err(map_transparent_error_v1)?;
    let inverse_shift = F(GOLDILOCKS_GENERATOR_V1)
        .inv()
        .ok_or(AggregateStarkErrorV1::InternalInvariant)?;
    let mut coefficients = ZeroizingFieldColumnV1(evaluations.to_vec());
    goldilocks_ifft_v1(&mut coefficients.0, root).map_err(map_transparent_error_v1)?;
    let mut inverse_shift_power = F::ONE;
    for coefficient in &mut coefficients.0 {
        *coefficient = coefficient.mul(inverse_shift_power);
        inverse_shift_power = inverse_shift_power.mul(inverse_shift);
    }
    Ok(coefficients)
}

fn fp4_coset_coefficients_v1(
    evaluations: &[E],
    lde_log2: u8,
) -> Result<ZeroizingExtensionFieldColumnV1, AggregateStarkErrorV1> {
    if evaluations.len() != checked_domain_size_v1(lde_log2)? {
        return Err(AggregateStarkErrorV1::InvalidLayout);
    }
    let root = goldilocks_primitive_root_v1(lde_log2).map_err(map_transparent_error_v1)?;
    let inverse_shift = F(GOLDILOCKS_GENERATOR_V1)
        .inv()
        .ok_or(AggregateStarkErrorV1::InternalInvariant)?;
    let mut coefficients = ZeroizingExtensionFieldColumnV1(evaluations.to_vec());
    goldilocks_fp4_ifft_v1(&mut coefficients.0, root).map_err(map_transparent_error_v1)?;
    let mut inverse_shift_power = F::ONE;
    for coefficient in &mut coefficients.0 {
        *coefficient = coefficient.mul_base(inverse_shift_power);
        inverse_shift_power = inverse_shift_power.mul(inverse_shift);
    }
    Ok(coefficients)
}

/// Evaluate one committed base-field coset codeword at arbitrary Fp4 points.
#[cfg(any(test, feature = "privacy-release-evidence"))]
pub(crate) fn evaluate_base_coset_polynomial_at_fp4_points_v1(
    evaluations: &[F],
    lde_log2: u8,
    points: &[E],
) -> Result<Vec<E>, AggregateStarkErrorV1> {
    if points.is_empty() {
        return Err(AggregateStarkErrorV1::InvalidLayout);
    }
    let coefficients = base_coset_coefficients_v1(evaluations, lde_log2)?;
    Ok(points
        .iter()
        .copied()
        .map(|point| evaluate_base_coefficients_at_fp4_v1(&coefficients, point))
        .collect())
}

/// Evaluate one committed Fp4 coset codeword at an arbitrary Fp4 point.
pub(crate) fn evaluate_fp4_coset_polynomial_at_point_v1(
    evaluations: &[E],
    lde_log2: u8,
    point: E,
) -> Result<E, AggregateStarkErrorV1> {
    let coefficients = fp4_coset_coefficients_v1(evaluations, lde_log2)?;
    Ok(evaluate_fp4_coefficients_at_fp4_v1(&coefficients, point))
}

#[cfg(test)]
fn evaluate_masked_native_column_at_points_v1(
    native: &[F],
    native_trace_log2: u8,
    mask: &ReplayableTraceMaskV1,
    points: &[E],
) -> Result<Vec<E>, AggregateStarkErrorV1> {
    if native.len() != checked_domain_size_v1(native_trace_log2)? || points.is_empty() {
        return Err(AggregateStarkErrorV1::InvalidLayout);
    }
    let root = goldilocks_primitive_root_v1(native_trace_log2).map_err(map_transparent_error_v1)?;
    let mut coefficients = ZeroizingFieldColumnV1(native.to_vec());
    goldilocks_ifft_v1(&mut coefficients.0, root).map_err(map_transparent_error_v1)?;
    let trace_size = native.len();
    Ok(points
        .iter()
        .copied()
        .map(|point| {
            let trace = evaluate_base_coefficients_at_fp4_v1(&coefficients, point);
            let randomizer = evaluate_base_coefficients_at_fp4_v1(mask.coefficients(), point);
            trace.add(point.pow(trace_size as u128).sub(E::ONE).mul(randomizer))
        })
        .collect())
}

/// Evaluate all replayable masked-native columns at `z` and
/// `z * omega_H` without retaining their common-domain LDEs.
#[cfg(test)]
pub(crate) fn evaluate_masked_native_columns_at_deep_v1<S>(
    masks: &StreamingTraceMaskSetV1,
    point: E,
    mut source: S,
) -> Result<(Vec<E>, Vec<E>), AggregateStarkErrorV1>
where
    S: FnMut(usize) -> Result<Vec<F>, AggregateStarkErrorV1>,
{
    if masks.width() == 0 {
        return Err(AggregateStarkErrorV1::InvalidLayout);
    }
    let next_root =
        goldilocks_primitive_root_v1(masks.native_trace_log2).map_err(map_transparent_error_v1)?;
    let next_point = point.mul_base(next_root);
    let mut current = Vec::new();
    let mut next = Vec::new();
    current
        .try_reserve_exact(masks.width())
        .map_err(|_| AggregateStarkErrorV1::AllocationFailure)?;
    next.try_reserve_exact(masks.width())
        .map_err(|_| AggregateStarkErrorV1::AllocationFailure)?;
    for (column, mask) in masks.masks.iter().enumerate() {
        let native = ZeroizingFieldColumnV1(source(column)?);
        let values = evaluate_masked_native_column_at_points_v1(
            &native,
            masks.native_trace_log2,
            mask,
            &[point, next_point],
        )?;
        current.push(values[0]);
        next.push(values[1]);
    }
    Ok((current, next))
}

/// Evaluate every retained masked polynomial at `z` and `z * omega_H`.
///
/// The DEEP point must be canonical and outside the native trace subgroup.
/// Evaluation is direct from the retained coefficients, so neither the native
/// witness columns nor a commitment-domain codeword are reconstructed.
#[cfg(any(test, feature = "privacy-release-evidence"))]
pub(crate) fn evaluate_masked_trace_polynomial_columns_at_deep_v1(
    polynomials: &MaskedTracePolynomialSetV1,
    point: E,
) -> Result<(Vec<E>, Vec<E>), AggregateStarkErrorV1> {
    let (native_rows, _) = polynomials.validate_v1()?;
    if !point.is_canonical() || fp4_is_in_trace_subgroup_v1(point, native_rows) {
        return Err(AggregateStarkErrorV1::DeepOpening);
    }
    let next_root = goldilocks_primitive_root_v1(polynomials.native_trace_log2)
        .map_err(map_transparent_error_v1)?;
    let next_point = point.mul_base(next_root);
    let mut current = Vec::new();
    let mut next = Vec::new();
    current
        .try_reserve_exact(polynomials.width())
        .map_err(|_| AggregateStarkErrorV1::AllocationFailure)?;
    next.try_reserve_exact(polynomials.width())
        .map_err(|_| AggregateStarkErrorV1::AllocationFailure)?;
    for column in &polynomials.columns {
        current.push(evaluate_base_coefficients_at_fp4_v1(column, point));
        next.push(evaluate_base_coefficients_at_fp4_v1(column, next_point));
    }
    Ok((current, next))
}

/// Evaluate all composition chunks at the transcript-derived DEEP point.
pub(crate) fn evaluate_composition_chunks_at_deep_v1(
    compositions: &[Vec<Vec<E>>],
    parameters: AggregateStarkParametersV1,
    layout: &AggregateProofLayoutV1,
    point: E,
) -> Result<Vec<Vec<E>>, AggregateStarkErrorV1> {
    layout.validate(parameters)?;
    if compositions.len() != parameters.security_lanes
        || compositions.iter().any(|lane| {
            lane.len() != parameters.composition_degree_chunks
                || lane
                    .iter()
                    .any(|chunk| chunk.len() != layout.common_lde_size())
        })
    {
        return Err(AggregateStarkErrorV1::InvalidLayout);
    }
    compositions
        .iter()
        .map(|lane| {
            lane.iter()
                .map(|chunk| {
                    evaluate_fp4_coset_polynomial_at_point_v1(chunk, layout.common_lde_log2, point)
                })
                .collect()
        })
        .collect()
}

/// Build a DEEP payload from retained materialized common-domain codewords.
#[cfg(any(test, feature = "privacy-release-evidence"))]
pub(crate) fn build_materialized_deep_proof_v1(
    trace_groups: &[AggregateTraceGroupMaterialV1],
    compositions: &[Vec<Vec<E>>],
    parameters: AggregateStarkParametersV1,
    layout: &AggregateProofLayoutV1,
    point: E,
) -> Result<AggregateDeepProofV1, AggregateStarkErrorV1> {
    layout.validate(parameters)?;
    if trace_groups.len() != layout.trace_groups.len() {
        return Err(AggregateStarkErrorV1::InvalidLayout);
    }
    let mut deep_groups = Vec::new();
    deep_groups
        .try_reserve_exact(trace_groups.len())
        .map_err(|_| AggregateStarkErrorV1::AllocationFailure)?;
    for (material, descriptor) in trace_groups.iter().zip(&layout.trace_groups) {
        if material.base_lde.len() != descriptor.base_width
            || material.aux_lde.len() != descriptor.aux_width
        {
            return Err(AggregateStarkErrorV1::InvalidLayout);
        }
        let root = goldilocks_primitive_root_v1(descriptor.native_trace_log2)
            .map_err(map_transparent_error_v1)?;
        let next_point = point.mul_base(root);
        let mut base_current = Vec::with_capacity(descriptor.base_width);
        let mut base_next = Vec::with_capacity(descriptor.base_width);
        let mut aux_current = Vec::with_capacity(descriptor.aux_width);
        let mut aux_next = Vec::with_capacity(descriptor.aux_width);
        for column in &material.base_lde {
            let values = evaluate_base_coset_polynomial_at_fp4_points_v1(
                column,
                layout.common_lde_log2,
                &[point, next_point],
            )?;
            base_current.push(values[0].coefficients().map(F::value));
            base_next.push(values[1].coefficients().map(F::value));
        }
        for column in &material.aux_lde {
            let values = evaluate_base_coset_polynomial_at_fp4_points_v1(
                column,
                layout.common_lde_log2,
                &[point, next_point],
            )?;
            aux_current.push(values[0].coefficients().map(F::value));
            aux_next.push(values[1].coefficients().map(F::value));
        }
        deep_groups.push(AggregateDeepTraceGroupOpeningV1 {
            base_current,
            base_next,
            aux_current,
            aux_next,
        });
    }
    let composition_values =
        evaluate_composition_chunks_at_deep_v1(compositions, parameters, layout, point)?
            .into_iter()
            .map(|lane| {
                lane.into_iter()
                    .map(|value| value.coefficients().map(F::value))
                    .collect()
            })
            .collect();
    let deep = AggregateDeepProofV1 {
        trace_groups: deep_groups,
        composition_values,
    };
    validate_deep_proof_shape_v1(&deep, parameters, layout)?;
    Ok(deep)
}

/// Batch-invert canonical nonzero Fp4 values using one extension-field
/// inversion.
///
/// DEEP code calls this on bounded row chunks so constructing quotient
/// codewords does not perform one expensive inversion per domain element or
/// allocate an evaluation-domain-sized prefix buffer.
pub(crate) fn batch_invert_fp4_nonzero_v1(values: &mut [E]) -> Result<(), AggregateStarkErrorV1> {
    if values.is_empty() {
        return Err(AggregateStarkErrorV1::DeepOpening);
    }
    let mut prefixes = Vec::new();
    prefixes
        .try_reserve_exact(values.len())
        .map_err(|_| AggregateStarkErrorV1::AllocationFailure)?;
    let mut product = E::ONE;
    for value in values.iter().copied() {
        if !value.is_canonical() || value == E::ZERO {
            return Err(AggregateStarkErrorV1::DeepOpening);
        }
        prefixes.push(product);
        product = product.mul(value);
    }
    let mut inverse = product.inv().ok_or(AggregateStarkErrorV1::DeepOpening)?;
    for index in (0..values.len()).rev() {
        let value = values[index];
        values[index] = inverse.mul(prefixes[index]);
        inverse = inverse.mul(value);
    }
    Ok(())
}

/// Validate verifier-derived DEEP batching dimensions.
pub(crate) fn validate_deep_lane_mixes_v1(
    mixes: &[AggregateDeepLaneMixV1],
    parameters: AggregateStarkParametersV1,
    layout: &AggregateProofLayoutV1,
) -> Result<(), AggregateStarkErrorV1> {
    layout.validate(parameters)?;
    if mixes.len() != parameters.security_lanes {
        return Err(AggregateStarkErrorV1::InvalidLayout);
    }
    for mix in mixes {
        if mix.trace_groups.len() != layout.trace_groups.len()
            || mix.composition.len() != parameters.composition_degree_chunks
        {
            return Err(AggregateStarkErrorV1::InvalidLayout);
        }
        for (group_mix, group) in mix.trace_groups.iter().zip(&layout.trace_groups) {
            if group_mix.base_current.len() != group.base_width
                || group_mix.base_next.len() != group.base_width
                || group_mix.aux_current.len() != group.aux_width
                || group_mix.aux_next.len() != group.aux_width
            {
                return Err(AggregateStarkErrorV1::InvalidLayout);
            }
        }
    }
    Ok(())
}

fn accumulate_fp4_deep_quotients_v1(
    mut accumulator: E,
    query: &[E],
    deep: &[E],
    coefficients: &[E],
    inverse_denominator: E,
) -> Result<E, AggregateStarkErrorV1> {
    if query.len() != deep.len() || query.len() != coefficients.len() {
        return Err(AggregateStarkErrorV1::DeepOpening);
    }
    for ((query, deep), coefficient) in query.iter().zip(deep).zip(coefficients) {
        accumulator = accumulator.add(query.sub(*deep).mul(inverse_denominator).mul(*coefficient));
    }
    Ok(accumulator)
}

fn accumulate_base_deep_quotients_v1(
    mut accumulator: E,
    query: &[F],
    deep: &[E],
    coefficients: &[E],
    inverse_denominator: E,
) -> Result<E, AggregateStarkErrorV1> {
    if query.len() != deep.len() || query.len() != coefficients.len() {
        return Err(AggregateStarkErrorV1::DeepOpening);
    }
    for ((query, deep), coefficient) in query.iter().zip(deep).zip(coefficients) {
        accumulator = accumulator.add(
            E::from_base(*query)
                .sub(*deep)
                .mul(inverse_denominator)
                .mul(*coefficient),
        );
    }
    Ok(accumulator)
}

/// Evaluate the complete batched DEEP-ALI quotient at one authenticated query
/// row.
pub(crate) fn deep_ali_mixed_opening_v1(
    query_point: E,
    deep_point: E,
    layout: &AggregateProofLayoutV1,
    trace_groups: &[AggregateOpenedTraceGroupV1],
    deep_trace_groups: &[AggregateOpenedDeepTraceGroupV1],
    composition_chunks: &[E],
    deep_composition_chunks: &[E],
    mix: &AggregateDeepLaneMixV1,
) -> Result<E, AggregateStarkErrorV1> {
    if trace_groups.len() != deep_trace_groups.len()
        || trace_groups.len() != mix.trace_groups.len()
        || composition_chunks.len() != deep_composition_chunks.len()
        || composition_chunks.len() != mix.composition.len()
    {
        return Err(AggregateStarkErrorV1::DeepOpening);
    }
    let current_inverse = query_point
        .sub(deep_point)
        .inv()
        .ok_or(AggregateStarkErrorV1::DeepOpening)?;
    let mut quotient = E::ZERO;
    for (group_index, ((query, deep), coefficients)) in trace_groups
        .iter()
        .zip(deep_trace_groups)
        .zip(&mix.trace_groups)
        .enumerate()
    {
        let group = layout
            .trace_groups
            .get(group_index)
            .ok_or(AggregateStarkErrorV1::DeepOpening)?;
        let next_root = goldilocks_primitive_root_v1(group.native_trace_log2)
            .map_err(map_transparent_error_v1)?;
        let next_inverse = query_point
            .sub(deep_point.mul_base(next_root))
            .inv()
            .ok_or(AggregateStarkErrorV1::DeepOpening)?;
        quotient = accumulate_base_deep_quotients_v1(
            quotient,
            &query.base_current,
            &deep.base_current,
            &coefficients.base_current,
            current_inverse,
        )?;
        quotient = accumulate_base_deep_quotients_v1(
            quotient,
            &query.base_current,
            &deep.base_next,
            &coefficients.base_next,
            next_inverse,
        )?;
        quotient = accumulate_base_deep_quotients_v1(
            quotient,
            &query.aux_current,
            &deep.aux_current,
            &coefficients.aux_current,
            current_inverse,
        )?;
        quotient = accumulate_base_deep_quotients_v1(
            quotient,
            &query.aux_current,
            &deep.aux_next,
            &coefficients.aux_next,
            next_inverse,
        )?;
    }
    quotient = accumulate_fp4_deep_quotients_v1(
        quotient,
        composition_chunks,
        deep_composition_chunks,
        &mix.composition,
        current_inverse,
    )?;
    Ok(quotient)
}

/// Commit one shared FRI layer.
pub(crate) fn fri_tree_v1(
    domains: AggregateStarkDomainsV1,
    lane: usize,
    round: usize,
    values: &[E],
) -> Result<Sha256MerkleTreeV1, AggregateStarkErrorV1> {
    domains.validate()?;
    let leaves = values
        .iter()
        .copied()
        .map(|value| fri_leaf_hash_unchecked_v1(domains, lane, round, value))
        .collect::<Result<Vec<_>, _>>()?;
    Sha256MerkleTreeV1::from_leaves(leaves, domains.fri_node).map_err(map_transparent_error_v1)
}

/// Commit one FRI layer without retaining a Merkle tree.
#[cfg(any(test, feature = "privacy-release-evidence"))]
pub(crate) fn streaming_fri_commitment_v1(
    domains: AggregateStarkDomainsV1,
    lane: usize,
    round: usize,
    values: &[E],
    opening_indices: &[usize],
) -> Result<StreamingMerkleCommitmentV1, AggregateStarkErrorV1> {
    domains.validate()?;
    streaming_merkle_commitment_v1(
        domains.fri_node,
        values.len(),
        opening_indices,
        values
            .iter()
            .copied()
            .map(|value| fri_leaf_hash_unchecked_v1(domains, lane, round, value)),
    )
}

/// Absorb the complete relation domain and ordered group layout before roots.
pub(crate) fn absorb_layout_v1(
    transcript: &mut TransparentTranscriptV1,
    parameters: AggregateStarkParametersV1,
    domains: AggregateStarkDomainsV1,
    relation_layout_domain: &[u8],
    layout: &AggregateProofLayoutV1,
) -> Result<(), AggregateStarkErrorV1> {
    layout.validate(parameters)?;
    domains.validate()?;
    if relation_layout_domain.is_empty() {
        return Err(AggregateStarkErrorV1::InvalidLayout);
    }
    let mut encoding = Vec::new();
    encoding.extend_from_slice(relation_layout_domain);
    encoding.push(layout.common_lde_log2);
    append_u16_v1(
        &mut encoding,
        u16::try_from(parameters.composition_degree_chunks)
            .map_err(|_| AggregateStarkErrorV1::InvalidLayout)?,
    );
    append_u16_v1(
        &mut encoding,
        u16::try_from(layout.trace_groups.len())
            .map_err(|_| AggregateStarkErrorV1::InvalidLayout)?,
    );
    for group in &layout.trace_groups {
        encoding.push(group.native_trace_log2);
        append_u16_v1(
            &mut encoding,
            u16::try_from(group.segment_instances)
                .map_err(|_| AggregateStarkErrorV1::InvalidLayout)?,
        );
        append_u32_v1(
            &mut encoding,
            u32::try_from(group.base_width).map_err(|_| AggregateStarkErrorV1::InvalidLayout)?,
        );
        append_u32_v1(
            &mut encoding,
            u32::try_from(group.aux_width).map_err(|_| AggregateStarkErrorV1::InvalidLayout)?,
        );
        append_u32_v1(
            &mut encoding,
            u32::try_from(group.next_stride(layout.common_lde_log2)?)
                .map_err(|_| AggregateStarkErrorV1::InvalidLayout)?,
        );
    }
    transcript
        .absorb(domains.layout_label, &[&encoding])
        .map_err(map_transparent_error_v1)
}

/// Absorb all ordered base roots before any relation auxiliary challenge.
pub(crate) fn absorb_base_roots_v1(
    transcript: &mut TransparentTranscriptV1,
    domains: AggregateStarkDomainsV1,
    groups: &[AggregateTraceGroupProofV1],
) -> Result<(), AggregateStarkErrorV1> {
    domains.validate()?;
    absorb_group_roots_v1(transcript, domains.base_root_label, groups, true)
}

/// Absorb all ordered auxiliary roots before constraint-composition challenges.
pub(crate) fn absorb_aux_roots_v1(
    transcript: &mut TransparentTranscriptV1,
    domains: AggregateStarkDomainsV1,
    groups: &[AggregateTraceGroupProofV1],
) -> Result<(), AggregateStarkErrorV1> {
    domains.validate()?;
    absorb_group_roots_v1(transcript, domains.aux_root_label, groups, false)
}

fn absorb_group_roots_v1(
    transcript: &mut TransparentTranscriptV1,
    label: &[u8],
    groups: &[AggregateTraceGroupProofV1],
    base: bool,
) -> Result<(), AggregateStarkErrorV1> {
    if groups.is_empty() || label.is_empty() {
        return Err(AggregateStarkErrorV1::InvalidLayout);
    }
    for (group, proof) in groups.iter().enumerate() {
        let group = u16::try_from(group)
            .map_err(|_| AggregateStarkErrorV1::InvalidLayout)?
            .to_be_bytes();
        let root = if base {
            &proof.base_root
        } else {
            &proof.aux_root
        };
        transcript
            .absorb(label, &[&group, root])
            .map_err(map_transparent_error_v1)?;
    }
    Ok(())
}

/// Absorb all aggregate composition roots in fixed lane order.
pub(crate) fn absorb_composition_roots_v1(
    transcript: &mut TransparentTranscriptV1,
    parameters: AggregateStarkParametersV1,
    domains: AggregateStarkDomainsV1,
    roots: &[[u8; 32]],
) -> Result<(), AggregateStarkErrorV1> {
    parameters.validate()?;
    domains.validate()?;
    if roots.len() != parameters.security_lanes {
        return Err(AggregateStarkErrorV1::InvalidProofShape);
    }
    for (lane, root) in roots.iter().enumerate() {
        let lane = u16::try_from(lane)
            .map_err(|_| AggregateStarkErrorV1::InvalidLayout)?
            .to_be_bytes();
        transcript
            .absorb(domains.composition_root_label, &[&lane, root])
            .map_err(map_transparent_error_v1)?;
    }
    Ok(())
}

/// Bind all independently committed FRI-mask roots before batching challenges.
pub(crate) fn absorb_fri_mask_roots_v1(
    transcript: &mut TransparentTranscriptV1,
    parameters: AggregateStarkParametersV1,
    roots: &[[u8; 32]],
) -> Result<(), AggregateStarkErrorV1> {
    parameters.validate()?;
    if roots.len() != parameters.security_lanes {
        return Err(AggregateStarkErrorV1::InvalidProofShape);
    }
    for (lane, root) in roots.iter().enumerate() {
        let lane = u16::try_from(lane)
            .map_err(|_| AggregateStarkErrorV1::InvalidLayout)?
            .to_be_bytes();
        transcript
            .absorb(FRI_MASK_ROOT_LABEL_V1, &[&lane, root])
            .map_err(map_transparent_error_v1)?;
    }
    Ok(())
}

/// Absorb one FRI root in fixed lane/round order.
pub(crate) fn absorb_fri_root_v1(
    transcript: &mut TransparentTranscriptV1,
    domains: AggregateStarkDomainsV1,
    lane: usize,
    round: usize,
    root: &[u8; 32],
) -> Result<(), AggregateStarkErrorV1> {
    domains.validate()?;
    let lane = u16::try_from(lane)
        .map_err(|_| AggregateStarkErrorV1::InvalidLayout)?
        .to_be_bytes();
    let round = u16::try_from(round)
        .map_err(|_| AggregateStarkErrorV1::InvalidLayout)?
        .to_be_bytes();
    transcript
        .absorb(domains.fri_root_label, &[&lane, &round, root])
        .map_err(map_transparent_error_v1)
}

/// Derive the shared unique query positions from the post-grinding transcript.
pub(crate) fn query_indices_v1(
    transcript: &TransparentTranscriptV1,
    parameters: AggregateStarkParametersV1,
    domains: AggregateStarkDomainsV1,
    layout: &AggregateProofLayoutV1,
) -> Result<Vec<usize>, AggregateStarkErrorV1> {
    layout.validate(parameters)?;
    domains.validate()?;
    let seed = sha256_frame_v1(
        domains.query_seed,
        &[&transcript.state(), &[layout.common_lde_log2]],
    )
    .map_err(map_transparent_error_v1)?;
    derive_unique_query_indices_v1(&seed, layout.common_lde_size(), parameters.query_count)
        .map_err(map_transparent_error_v1)
}

fn validate_canonical_index_set_v1(
    leaf_count: usize,
    indices: &[usize],
) -> Result<(), AggregateStarkErrorV1> {
    if leaf_count == 0
        || !leaf_count.is_power_of_two()
        || indices.is_empty()
        || indices.iter().any(|index| *index >= leaf_count)
        || indices.windows(2).any(|pair| pair[0] >= pair[1])
    {
        return Err(AggregateStarkErrorV1::InvalidProofShape);
    }
    Ok(())
}

/// Derive the sorted current/next leaf set for one trace group.
pub(crate) fn trace_group_opening_indices_v1(
    queries: &[AggregateQueryProofV1],
    layout: &AggregateProofLayoutV1,
    group: usize,
) -> Result<Vec<usize>, AggregateStarkErrorV1> {
    let group = *layout
        .trace_groups
        .get(group)
        .ok_or(AggregateStarkErrorV1::InvalidLayout)?;
    let next_stride = group.next_stride(layout.common_lde_log2)?;
    let mut indices = BTreeSet::new();
    for query in queries {
        let current =
            usize::try_from(query.index).map_err(|_| AggregateStarkErrorV1::InvalidProofShape)?;
        if current >= layout.common_lde_size() {
            return Err(AggregateStarkErrorV1::InvalidProofShape);
        }
        indices.insert(current);
        indices.insert((current + next_stride) % layout.common_lde_size());
    }
    Ok(indices.into_iter().collect())
}

/// Derive the sorted unique aggregate-composition leaf set.
pub(crate) fn composition_opening_indices_v1(
    queries: &[AggregateQueryProofV1],
    layout: &AggregateProofLayoutV1,
) -> Result<Vec<usize>, AggregateStarkErrorV1> {
    let mut indices = BTreeSet::new();
    for query in queries {
        let index =
            usize::try_from(query.index).map_err(|_| AggregateStarkErrorV1::InvalidProofShape)?;
        if index >= layout.common_lde_size() || !indices.insert(index) {
            return Err(AggregateStarkErrorV1::InvalidProofShape);
        }
    }
    Ok(indices.into_iter().collect())
}

/// Derive the sorted low/high leaf set for one shared FRI round.
pub(crate) fn fri_opening_indices_v1(
    queries: &[AggregateQueryProofV1],
    parameters: AggregateStarkParametersV1,
    layout: &AggregateProofLayoutV1,
    round: usize,
) -> Result<Vec<usize>, AggregateStarkErrorV1> {
    if round >= layout.fri_rounds(parameters)? {
        return Err(AggregateStarkErrorV1::InvalidProofShape);
    }
    let layer_size = layout.common_lde_size() >> round;
    let half = layer_size / 2;
    let mut indices = BTreeSet::new();
    for query in queries {
        let query_index =
            usize::try_from(query.index).map_err(|_| AggregateStarkErrorV1::InvalidProofShape)?;
        if query_index >= layout.common_lde_size() {
            return Err(AggregateStarkErrorV1::InvalidProofShape);
        }
        let low = query_index % half;
        indices.insert(low);
        indices.insert(low + half);
    }
    Ok(indices.into_iter().collect())
}

/// Exact number of hashes in the unique canonical minimal frontier.
pub(crate) fn multiproof_frontier_len_v1(
    leaf_count: usize,
    indices: &[usize],
) -> Result<usize, AggregateStarkErrorV1> {
    validate_canonical_index_set_v1(leaf_count, indices)?;
    let mut current = indices.iter().copied().collect::<BTreeSet<_>>();
    let mut frontier_len = 0_usize;
    let mut level_size = leaf_count;
    while level_size > 1 {
        for index in &current {
            if !current.contains(&(index ^ 1)) {
                frontier_len = frontier_len
                    .checked_add(1)
                    .ok_or(AggregateStarkErrorV1::InvalidLayout)?;
            }
        }
        current = current.into_iter().map(|index| index >> 1).collect();
        level_size >>= 1;
    }
    Ok(frontier_len)
}

/// Construct the unique sorted minimal batched Merkle frontier.
pub(crate) fn canonical_multiproof_frontier_v1(
    tree: &Sha256MerkleTreeV1,
    leaf_count: usize,
    indices: &[usize],
) -> Result<Vec<[u8; 32]>, AggregateStarkErrorV1> {
    validate_canonical_index_set_v1(leaf_count, indices)?;
    let expected_len = multiproof_frontier_len_v1(leaf_count, indices)?;
    let mut frontier = Vec::new();
    frontier
        .try_reserve_exact(expected_len)
        .map_err(|_| AggregateStarkErrorV1::AllocationFailure)?;
    let mut current = indices
        .iter()
        .copied()
        .map(|index| (index, index))
        .collect::<BTreeMap<_, _>>();
    let mut level_size = leaf_count;
    let mut level = 0_usize;
    while level_size > 1 {
        let mut parents = BTreeMap::new();
        for (&index, &representative_leaf) in &current {
            if !current.contains_key(&(index ^ 1)) {
                let path = tree
                    .path(representative_leaf)
                    .map_err(map_transparent_error_v1)?;
                frontier.push(
                    *path
                        .get(level)
                        .ok_or(AggregateStarkErrorV1::InternalInvariant)?,
                );
            }
            parents.entry(index >> 1).or_insert(representative_leaf);
        }
        current = parents;
        level_size >>= 1;
        level += 1;
    }
    if frontier.len() != expected_len || current.len() != 1 || !current.contains_key(&0) {
        return Err(AggregateStarkErrorV1::InternalInvariant);
    }
    Ok(frontier)
}

/// Verify one exact canonical minimal batched Merkle multiproof.
pub(crate) fn verify_canonical_multiproof_v1(
    node_domain: &[u8],
    root: &[u8; 32],
    leaf_count: usize,
    leaves: &BTreeMap<usize, [u8; 32]>,
    frontier: &[[u8; 32]],
) -> Result<(), AggregateStarkErrorV1> {
    let indices = leaves.keys().copied().collect::<Vec<_>>();
    validate_canonical_index_set_v1(leaf_count, &indices)?;
    if multiproof_frontier_len_v1(leaf_count, &indices)? != frontier.len() {
        return Err(AggregateStarkErrorV1::TraceOpening);
    }
    let mut current = leaves.clone();
    let mut cursor = 0_usize;
    let mut level_size = leaf_count;
    while level_size > 1 {
        let mut parents = BTreeMap::new();
        let mut consumed = BTreeSet::new();
        for (&index, &node) in &current {
            if consumed.contains(&index) {
                continue;
            }
            let sibling_index = index ^ 1;
            let sibling = if let Some(sibling) = current.get(&sibling_index) {
                consumed.insert(sibling_index);
                *sibling
            } else {
                let sibling = *frontier
                    .get(cursor)
                    .ok_or(AggregateStarkErrorV1::TraceOpening)?;
                cursor += 1;
                sibling
            };
            consumed.insert(index);
            let parent = if index & 1 == 0 {
                sha256_merkle_node_v1(node_domain, &node, &sibling)
            } else {
                sha256_merkle_node_v1(node_domain, &sibling, &node)
            };
            if parents.insert(index >> 1, parent).is_some() {
                return Err(AggregateStarkErrorV1::TraceOpening);
            }
        }
        current = parents;
        level_size >>= 1;
    }
    if cursor != frontier.len() || current.len() != 1 || current.get(&0).copied() != Some(*root) {
        return Err(AggregateStarkErrorV1::TraceOpening);
    }
    Ok(())
}

fn insert_opened_leaf_v1(
    leaves: &mut BTreeMap<usize, [u8; 32]>,
    index: usize,
    leaf: [u8; 32],
) -> Result<(), AggregateStarkErrorV1> {
    if leaves
        .insert(index, leaf)
        .is_some_and(|previous| previous != leaf)
    {
        return Err(AggregateStarkErrorV1::TraceOpening);
    }
    Ok(())
}

/// Exact worst-case minimal-frontier length for at most `maximum_opened_leaves`.
pub(crate) fn maximum_multiproof_frontier_len_v1(
    leaf_count: usize,
    maximum_opened_leaves: usize,
) -> Result<usize, AggregateStarkErrorV1> {
    if leaf_count == 0
        || !leaf_count.is_power_of_two()
        || maximum_opened_leaves == 0
        || maximum_opened_leaves > leaf_count
    {
        return Err(AggregateStarkErrorV1::InvalidLayout);
    }
    let height =
        usize::try_from(leaf_count.ilog2()).map_err(|_| AggregateStarkErrorV1::InvalidLayout)?;
    let mut previous = vec![None; maximum_opened_leaves + 1];
    previous[1] = Some(0_usize);
    let mut maximum = 0_usize;
    for current_height in 1..=height {
        let half_capacity = 1_usize
            .checked_shl(
                u32::try_from(current_height - 1)
                    .map_err(|_| AggregateStarkErrorV1::InvalidLayout)?,
            )
            .ok_or(AggregateStarkErrorV1::InvalidLayout)?;
        let capacity = half_capacity
            .checked_mul(2)
            .ok_or(AggregateStarkErrorV1::InvalidLayout)?;
        let row_limit = maximum_opened_leaves.min(capacity);
        let mut current = vec![None; maximum_opened_leaves + 1];
        let mut row_maximum = 0_usize;
        for opened in 1..=row_limit {
            let minimum_left = opened.saturating_sub(half_capacity);
            let maximum_left = opened.min(half_capacity);
            let mut best = None;
            for left in minimum_left..=maximum_left {
                let right = opened - left;
                let candidate = match (left, right) {
                    (0, right) => previous
                        .get(right)
                        .and_then(|value| *value)
                        .and_then(|value| value.checked_add(1)),
                    (left, 0) => previous
                        .get(left)
                        .and_then(|value| *value)
                        .and_then(|value| value.checked_add(1)),
                    (left, right) => previous
                        .get(left)
                        .and_then(|value| *value)
                        .zip(previous.get(right).and_then(|value| *value))
                        .and_then(|(left, right)| left.checked_add(right)),
                };
                if let Some(candidate) = candidate {
                    best = Some(best.map_or(candidate, |prior: usize| prior.max(candidate)));
                }
            }
            current[opened] = best;
            row_maximum = row_maximum.max(best.ok_or(AggregateStarkErrorV1::InternalInvariant)?);
        }
        previous = current;
        maximum = row_maximum;
    }
    Ok(maximum)
}

/// Validate the entire statement-derived proof shape and canonical frontier sizes.
pub(crate) fn validate_proof_shape_v1(
    proof: &AggregateStarkProofV1,
    parameters: AggregateStarkParametersV1,
    layout: &AggregateProofLayoutV1,
) -> Result<(), AggregateStarkErrorV1> {
    layout.validate(parameters)?;
    if proof.version != parameters.proof_version
        || proof.trace_groups.len() != layout.trace_groups.len()
        || proof.composition_roots.len() != parameters.security_lanes
        || proof.composition_frontiers.len() != parameters.security_lanes
        || proof.fri_mask_roots.len() != parameters.security_lanes
        || proof.fri_mask_frontiers.len() != parameters.security_lanes
        || proof.fri_lanes.len() != parameters.security_lanes
        || proof.queries.len() != parameters.query_count
    {
        return Err(AggregateStarkErrorV1::InvalidProofShape);
    }
    let fri_rounds = layout.fri_rounds(parameters)?;
    for query in &proof.queries {
        if query.trace_groups.len() != layout.trace_groups.len()
            || query.composition_values.len() != parameters.security_lanes
            || query
                .composition_values
                .iter()
                .any(|chunks| chunks.len() != parameters.composition_degree_chunks)
            || query.fri_mask_values.len() != parameters.security_lanes
            || query.fri_lanes.len() != parameters.security_lanes
            || query
                .fri_lanes
                .iter()
                .any(|lane| lane.rounds.len() != fri_rounds)
        {
            return Err(AggregateStarkErrorV1::InvalidProofShape);
        }
        for (opening, group) in query.trace_groups.iter().zip(&layout.trace_groups) {
            if opening.base_current.len() != group.base_width
                || opening.base_next.len() != group.base_width
                || opening.aux_current.len() != group.aux_width
                || opening.aux_next.len() != group.aux_width
            {
                return Err(AggregateStarkErrorV1::InvalidProofShape);
            }
            ensure_canonical_base_fields_v1(&opening.base_current)?;
            ensure_canonical_base_fields_v1(&opening.base_next)?;
            ensure_canonical_base_fields_v1(&opening.aux_current)?;
            ensure_canonical_base_fields_v1(&opening.aux_next)?;
        }
        for values in &query.composition_values {
            ensure_canonical_fp4_fields_v1(values)?;
        }
        ensure_canonical_fp4_fields_v1(&query.fri_mask_values)?;
        for lane in &query.fri_lanes {
            for opening in &lane.rounds {
                ensure_canonical_fp4_fields_v1(core::slice::from_ref(&opening.low))?;
                ensure_canonical_fp4_fields_v1(core::slice::from_ref(&opening.high))?;
            }
        }
    }
    for (group_index, group_proof) in proof.trace_groups.iter().enumerate() {
        let trace_indices = trace_group_opening_indices_v1(&proof.queries, layout, group_index)?;
        let expected = multiproof_frontier_len_v1(layout.common_lde_size(), &trace_indices)?;
        if group_proof.base_frontier.len() != expected || group_proof.aux_frontier.len() != expected
        {
            return Err(AggregateStarkErrorV1::InvalidProofShape);
        }
    }
    let composition_indices = composition_opening_indices_v1(&proof.queries, layout)?;
    let composition_expected =
        multiproof_frontier_len_v1(layout.common_lde_size(), &composition_indices)?;
    if proof
        .composition_frontiers
        .iter()
        .any(|frontier| frontier.len() != composition_expected)
        || proof
            .fri_mask_frontiers
            .iter()
            .any(|frontier| frontier.len() != composition_expected)
    {
        return Err(AggregateStarkErrorV1::InvalidProofShape);
    }
    for lane in &proof.fri_lanes {
        if lane.roots.len() != fri_rounds + 1
            || lane.terminal_values.len() != parameters.terminal_size()?
            || lane.round_frontiers.len() != fri_rounds
        {
            return Err(AggregateStarkErrorV1::InvalidProofShape);
        }
        ensure_canonical_fp4_fields_v1(&lane.terminal_values)?;
        for (round, frontier) in lane.round_frontiers.iter().enumerate() {
            let indices = fri_opening_indices_v1(&proof.queries, parameters, layout, round)?;
            let expected = multiproof_frontier_len_v1(layout.common_lde_size() >> round, &indices)?;
            if frontier.len() != expected {
                return Err(AggregateStarkErrorV1::InvalidProofShape);
            }
        }
    }
    Ok(())
}

fn append_hash_v1(bytes: &mut Vec<u8>, hash: &[u8; 32]) {
    bytes.extend_from_slice(hash);
}

fn append_hashes_v1(bytes: &mut Vec<u8>, hashes: &[[u8; 32]]) {
    for hash in hashes {
        append_hash_v1(bytes, hash);
    }
}

fn append_base_fields_v1(bytes: &mut Vec<u8>, fields: &[u64]) {
    for field in fields {
        append_u64_v1(bytes, *field);
    }
}

fn append_fp4_fields_v1(bytes: &mut Vec<u8>, fields: &[[u64; 4]]) {
    for field in fields {
        for coefficient in field {
            append_u64_v1(bytes, *coefficient);
        }
    }
}

/// Exact byte count of the statement-shaped DEEP opening payload.
pub(crate) fn exact_deep_opening_bytes_v1(
    parameters: AggregateStarkParametersV1,
    layout: &AggregateProofLayoutV1,
) -> Result<usize, AggregateStarkErrorV1> {
    layout.validate(parameters)?;
    let trace_fields = layout
        .trace_groups
        .iter()
        .try_fold(0_usize, |total, group| {
            group
                .base_width
                .checked_add(group.aux_width)
                .and_then(|width| width.checked_mul(2))
                .and_then(|fields| total.checked_add(fields))
                .ok_or(AggregateStarkErrorV1::InvalidLayout)
        })?;
    let composition_fields = parameters
        .security_lanes
        .checked_mul(parameters.composition_degree_chunks)
        .ok_or(AggregateStarkErrorV1::InvalidLayout)?;
    trace_fields
        .checked_add(composition_fields)
        .and_then(|fields| fields.checked_mul(core::mem::size_of::<[u64; 4]>()))
        .ok_or(AggregateStarkErrorV1::InvalidLayout)
}

fn deep_insertion_offset_v1(
    parameters: AggregateStarkParametersV1,
    layout: &AggregateProofLayoutV1,
) -> Result<usize, AggregateStarkErrorV1> {
    layout.validate(parameters)?;
    let header_bytes = parameters
        .proof_magic
        .len()
        .checked_add(core::mem::size_of::<u16>() * 2)
        .ok_or(AggregateStarkErrorV1::InvalidLayout)?;
    let roots = layout
        .trace_groups
        .len()
        .checked_mul(2)
        .and_then(|roots| roots.checked_add(parameters.security_lanes))
        .and_then(|roots| roots.checked_add(parameters.security_lanes))
        .ok_or(AggregateStarkErrorV1::InvalidLayout)?;
    header_bytes
        .checked_add(
            roots
                .checked_mul(core::mem::size_of::<[u8; 32]>())
                .ok_or(AggregateStarkErrorV1::InvalidLayout)?,
        )
        .ok_or(AggregateStarkErrorV1::InvalidLayout)
}

fn encode_deep_openings_raw_v1(
    deep: &AggregateDeepProofV1,
    parameters: AggregateStarkParametersV1,
    layout: &AggregateProofLayoutV1,
) -> Result<Vec<u8>, AggregateStarkErrorV1> {
    validate_deep_proof_shape_v1(deep, parameters, layout)?;
    let expected = exact_deep_opening_bytes_v1(parameters, layout)?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(expected)
        .map_err(|_| AggregateStarkErrorV1::AllocationFailure)?;
    for group in &deep.trace_groups {
        append_fp4_fields_v1(&mut bytes, &group.base_current);
        append_fp4_fields_v1(&mut bytes, &group.base_next);
        append_fp4_fields_v1(&mut bytes, &group.aux_current);
        append_fp4_fields_v1(&mut bytes, &group.aux_next);
    }
    for values in &deep.composition_values {
        append_fp4_fields_v1(&mut bytes, values);
    }
    if bytes.len() != expected {
        return Err(AggregateStarkErrorV1::InternalInvariant);
    }
    Ok(bytes)
}

fn decode_deep_openings_raw_v1(
    bytes: &[u8],
    parameters: AggregateStarkParametersV1,
    layout: &AggregateProofLayoutV1,
) -> Result<AggregateDeepProofV1, AggregateStarkErrorV1> {
    if bytes.len() != exact_deep_opening_bytes_v1(parameters, layout)? {
        return Err(AggregateStarkErrorV1::MalformedProof);
    }
    let mut reader = ExactProofReaderV1::new(bytes);
    let trace_groups = layout
        .trace_groups
        .iter()
        .map(|group| {
            Ok(AggregateDeepTraceGroupOpeningV1 {
                base_current: take_fp4_fields_v1(&mut reader, group.base_width)?,
                base_next: take_fp4_fields_v1(&mut reader, group.base_width)?,
                aux_current: take_fp4_fields_v1(&mut reader, group.aux_width)?,
                aux_next: take_fp4_fields_v1(&mut reader, group.aux_width)?,
            })
        })
        .collect::<Result<Vec<_>, AggregateStarkErrorV1>>()?;
    let composition_values = (0..parameters.security_lanes)
        .map(|_| take_fp4_fields_v1(&mut reader, parameters.composition_degree_chunks))
        .collect::<Result<Vec<_>, _>>()?;
    reader.finish().map_err(reader_error_v1)?;
    let deep = AggregateDeepProofV1 {
        trace_groups,
        composition_values,
    };
    validate_deep_proof_shape_v1(&deep, parameters, layout)?;
    Ok(deep)
}

fn encoded_non_frontier_bytes_v1(
    parameters: AggregateStarkParametersV1,
    layout: &AggregateProofLayoutV1,
) -> Result<usize, AggregateStarkErrorV1> {
    layout.validate(parameters)?;
    let terminal_size = parameters.terminal_size()?;
    let hash_bytes = core::mem::size_of::<[u8; 32]>();
    let base_field_bytes = core::mem::size_of::<u64>();
    let extension_field_bytes = core::mem::size_of::<[u64; 4]>();
    let mut bytes = parameters
        .proof_magic
        .len()
        .checked_add(core::mem::size_of::<u16>() * 2)
        .ok_or(AggregateStarkErrorV1::InvalidLayout)?;
    let trace_roots = layout
        .trace_groups
        .len()
        .checked_mul(2)
        .ok_or(AggregateStarkErrorV1::InvalidLayout)?;
    let fri_roots = parameters
        .security_lanes
        .checked_mul(
            layout
                .fri_rounds(parameters)?
                .checked_add(1)
                .ok_or(AggregateStarkErrorV1::InvalidLayout)?,
        )
        .ok_or(AggregateStarkErrorV1::InvalidLayout)?;
    let root_count = trace_roots
        .checked_add(parameters.security_lanes)
        .and_then(|value| value.checked_add(parameters.security_lanes))
        .and_then(|value| value.checked_add(fri_roots))
        .ok_or(AggregateStarkErrorV1::InvalidLayout)?;
    bytes = bytes
        .checked_add(
            root_count
                .checked_mul(hash_bytes)
                .ok_or(AggregateStarkErrorV1::InvalidLayout)?,
        )
        .and_then(|value| {
            value.checked_add(
                parameters
                    .security_lanes
                    .checked_mul(terminal_size)?
                    .checked_mul(extension_field_bytes)?,
            )
        })
        .and_then(|value| value.checked_add(core::mem::size_of::<u64>()))
        .ok_or(AggregateStarkErrorV1::InvalidLayout)?;
    let total_width = layout
        .trace_groups
        .iter()
        .try_fold(0_usize, |total, group| {
            total
                .checked_add(
                    group
                        .base_width
                        .checked_add(group.aux_width)
                        .ok_or(AggregateStarkErrorV1::InvalidLayout)?,
                )
                .ok_or(AggregateStarkErrorV1::InvalidLayout)
        })?;
    let trace_fields = total_width
        .checked_mul(2)
        .ok_or(AggregateStarkErrorV1::InvalidLayout)?;
    let fri_fields = parameters
        .security_lanes
        .checked_mul(layout.fri_rounds(parameters)?)
        .and_then(|value| value.checked_mul(2))
        .ok_or(AggregateStarkErrorV1::InvalidLayout)?;
    let composition_fields = parameters
        .security_lanes
        .checked_mul(parameters.composition_degree_chunks)
        .ok_or(AggregateStarkErrorV1::InvalidLayout)?;
    let trace_bytes = trace_fields
        .checked_mul(base_field_bytes)
        .ok_or(AggregateStarkErrorV1::InvalidLayout)?;
    let extension_fields_per_query = composition_fields
        .checked_add(parameters.security_lanes)
        .and_then(|value| value.checked_add(fri_fields))
        .ok_or(AggregateStarkErrorV1::InvalidLayout)?;
    let extension_bytes = extension_fields_per_query
        .checked_mul(extension_field_bytes)
        .ok_or(AggregateStarkErrorV1::InvalidLayout)?;
    let fields_bytes_per_query = trace_bytes
        .checked_add(extension_bytes)
        .ok_or(AggregateStarkErrorV1::InvalidLayout)?;
    let query_bytes = core::mem::size_of::<u32>()
        .checked_add(fields_bytes_per_query)
        .and_then(|value| value.checked_mul(parameters.query_count))
        .ok_or(AggregateStarkErrorV1::InvalidLayout)?;
    bytes
        .checked_add(query_bytes)
        .ok_or(AggregateStarkErrorV1::InvalidLayout)
}

/// Exact encoded byte length for one validated proof object.
pub(crate) fn exact_encoded_proof_bytes_v1(
    proof: &AggregateStarkProofV1,
    parameters: AggregateStarkParametersV1,
    layout: &AggregateProofLayoutV1,
) -> Result<usize, AggregateStarkErrorV1> {
    validate_proof_shape_v1(proof, parameters, layout)?;
    let trace_hashes = proof
        .trace_groups
        .iter()
        .try_fold(0_usize, |total, group| {
            total
                .checked_add(group.base_frontier.len())
                .and_then(|value| value.checked_add(group.aux_frontier.len()))
                .ok_or(AggregateStarkErrorV1::InvalidLayout)
        })?;
    let composition_hashes =
        proof
            .composition_frontiers
            .iter()
            .try_fold(0_usize, |total, frontier| {
                total
                    .checked_add(frontier.len())
                    .ok_or(AggregateStarkErrorV1::InvalidLayout)
            })?;
    let fri_mask_hashes =
        proof
            .fri_mask_frontiers
            .iter()
            .try_fold(0_usize, |total, frontier| {
                total
                    .checked_add(frontier.len())
                    .ok_or(AggregateStarkErrorV1::InvalidLayout)
            })?;
    let fri_hashes = proof
        .fri_lanes
        .iter()
        .flat_map(|lane| &lane.round_frontiers)
        .try_fold(0_usize, |total, frontier| {
            total
                .checked_add(frontier.len())
                .ok_or(AggregateStarkErrorV1::InvalidLayout)
        })?;
    let frontier_hashes = trace_hashes
        .checked_add(composition_hashes)
        .and_then(|value| value.checked_add(fri_mask_hashes))
        .and_then(|value| value.checked_add(fri_hashes))
        .ok_or(AggregateStarkErrorV1::InvalidLayout)?;
    encoded_non_frontier_bytes_v1(parameters, layout)?
        .checked_add(
            frontier_hashes
                .checked_mul(core::mem::size_of::<[u8; 32]>())
                .ok_or(AggregateStarkErrorV1::InvalidLayout)?,
        )
        .ok_or(AggregateStarkErrorV1::InvalidLayout)
}

/// Exact encoded byte length of one validated DEEP-enabled proof.
pub(crate) fn exact_encoded_proof_with_deep_bytes_v1(
    proof: &AggregateStarkProofV1,
    deep: &AggregateDeepProofV1,
    parameters: AggregateStarkParametersV1,
    layout: &AggregateProofLayoutV1,
) -> Result<usize, AggregateStarkErrorV1> {
    validate_deep_proof_shape_v1(deep, parameters, layout)?;
    exact_encoded_proof_bytes_v1(proof, parameters, layout)?
        .checked_add(exact_deep_opening_bytes_v1(parameters, layout)?)
        .ok_or(AggregateStarkErrorV1::InvalidLayout)
}

/// Hard maximum encoded length over all legal query/frontier arrangements.
pub(crate) fn maximum_encoded_proof_bytes_v1(
    parameters: AggregateStarkParametersV1,
    layout: &AggregateProofLayoutV1,
) -> Result<usize, AggregateStarkErrorV1> {
    let non_frontier = encoded_non_frontier_bytes_v1(parameters, layout)?;
    let trace_opened = parameters
        .query_count
        .checked_mul(2)
        .ok_or(AggregateStarkErrorV1::InvalidLayout)?
        .min(layout.common_lde_size());
    let trace_frontier =
        maximum_multiproof_frontier_len_v1(layout.common_lde_size(), trace_opened)?;
    let composition_frontier = maximum_multiproof_frontier_len_v1(
        layout.common_lde_size(),
        parameters.query_count.min(layout.common_lde_size()),
    )?;
    let mut fri_frontiers = 0_usize;
    for round in 0..layout.fri_rounds(parameters)? {
        let layer_size = layout.common_lde_size() >> round;
        fri_frontiers = fri_frontiers
            .checked_add(maximum_multiproof_frontier_len_v1(
                layer_size,
                trace_opened.min(layer_size),
            )?)
            .ok_or(AggregateStarkErrorV1::InvalidLayout)?;
    }
    let frontier_hashes = layout
        .trace_groups
        .len()
        .checked_mul(2)
        .and_then(|groups| groups.checked_mul(trace_frontier))
        .and_then(|value| {
            parameters
                .security_lanes
                .checked_mul(composition_frontier)
                .and_then(|composition| value.checked_add(composition))
        })
        .and_then(|value| {
            parameters
                .security_lanes
                .checked_mul(composition_frontier)
                .and_then(|fri_mask| value.checked_add(fri_mask))
        })
        .and_then(|value| {
            parameters
                .security_lanes
                .checked_mul(fri_frontiers)
                .and_then(|fri| value.checked_add(fri))
        })
        .ok_or(AggregateStarkErrorV1::InvalidLayout)?;
    non_frontier
        .checked_add(
            frontier_hashes
                .checked_mul(core::mem::size_of::<[u8; 32]>())
                .ok_or(AggregateStarkErrorV1::InvalidLayout)?,
        )
        .ok_or(AggregateStarkErrorV1::InvalidLayout)
}

/// Hard maximum encoded length of a DEEP-enabled proof.
pub(crate) fn maximum_encoded_proof_with_deep_bytes_v1(
    parameters: AggregateStarkParametersV1,
    layout: &AggregateProofLayoutV1,
) -> Result<usize, AggregateStarkErrorV1> {
    maximum_encoded_proof_bytes_v1(parameters, layout)?
        .checked_add(exact_deep_opening_bytes_v1(parameters, layout)?)
        .ok_or(AggregateStarkErrorV1::InvalidLayout)
}

/// Encode the sole canonical exact aggregate proof wire.
pub(crate) fn encode_proof_v1(
    proof: &AggregateStarkProofV1,
    parameters: AggregateStarkParametersV1,
    layout: &AggregateProofLayoutV1,
) -> Result<Vec<u8>, AggregateStarkErrorV1> {
    validate_proof_shape_v1(proof, parameters, layout)?;
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&parameters.proof_magic);
    append_u16_v1(&mut bytes, proof.version);
    append_u16_v1(
        &mut bytes,
        u16::try_from(proof.trace_groups.len())
            .map_err(|_| AggregateStarkErrorV1::InvalidProofShape)?,
    );
    for group in &proof.trace_groups {
        append_hash_v1(&mut bytes, &group.base_root);
    }
    for group in &proof.trace_groups {
        append_hash_v1(&mut bytes, &group.aux_root);
    }
    append_hashes_v1(&mut bytes, &proof.composition_roots);
    append_hashes_v1(&mut bytes, &proof.fri_mask_roots);
    for lane in &proof.fri_lanes {
        append_hashes_v1(&mut bytes, &lane.roots);
        append_fp4_fields_v1(&mut bytes, &lane.terminal_values);
    }
    append_u64_v1(&mut bytes, proof.grinding_nonce);
    for query in &proof.queries {
        append_u32_v1(&mut bytes, query.index);
        for group in &query.trace_groups {
            append_base_fields_v1(&mut bytes, &group.base_current);
            append_base_fields_v1(&mut bytes, &group.base_next);
            append_base_fields_v1(&mut bytes, &group.aux_current);
            append_base_fields_v1(&mut bytes, &group.aux_next);
        }
        for lane in &query.composition_values {
            append_fp4_fields_v1(&mut bytes, lane);
        }
        append_fp4_fields_v1(&mut bytes, &query.fri_mask_values);
        for lane in &query.fri_lanes {
            for opening in &lane.rounds {
                append_fp4_fields_v1(&mut bytes, core::slice::from_ref(&opening.low));
                append_fp4_fields_v1(&mut bytes, core::slice::from_ref(&opening.high));
            }
        }
    }
    for group in &proof.trace_groups {
        append_hashes_v1(&mut bytes, &group.base_frontier);
        append_hashes_v1(&mut bytes, &group.aux_frontier);
    }
    for frontier in &proof.composition_frontiers {
        append_hashes_v1(&mut bytes, frontier);
    }
    for frontier in &proof.fri_mask_frontiers {
        append_hashes_v1(&mut bytes, frontier);
    }
    for lane in &proof.fri_lanes {
        for frontier in &lane.round_frontiers {
            append_hashes_v1(&mut bytes, frontier);
        }
    }
    if bytes.len() != exact_encoded_proof_bytes_v1(proof, parameters, layout)? {
        return Err(AggregateStarkErrorV1::InternalInvariant);
    }
    if bytes.len() > parameters.maximum_proof_bytes {
        return Err(AggregateStarkErrorV1::ProofTooLarge);
    }
    Ok(bytes)
}

/// Encode the sole canonical DEEP-enabled aggregate wire.
///
/// DEEP values are inserted after every trace/composition/mask root and before
/// the first FRI root. This fixed position mirrors transcript order and leaves
/// no tag, count, or alternate representation for a decoder to reinterpret.
pub(crate) fn encode_proof_with_deep_v1(
    proof: &AggregateStarkProofV1,
    deep: &AggregateDeepProofV1,
    parameters: AggregateStarkParametersV1,
    layout: &AggregateProofLayoutV1,
) -> Result<Vec<u8>, AggregateStarkErrorV1> {
    let mut bytes = encode_proof_v1(proof, parameters, layout)?;
    let deep_bytes = encode_deep_openings_raw_v1(deep, parameters, layout)?;
    let insertion = deep_insertion_offset_v1(parameters, layout)?;
    if insertion > bytes.len() {
        return Err(AggregateStarkErrorV1::InternalInvariant);
    }
    bytes
        .try_reserve_exact(deep_bytes.len())
        .map_err(|_| AggregateStarkErrorV1::AllocationFailure)?;
    bytes.splice(insertion..insertion, deep_bytes);
    if bytes.len() != exact_encoded_proof_with_deep_bytes_v1(proof, deep, parameters, layout)? {
        return Err(AggregateStarkErrorV1::InternalInvariant);
    }
    if bytes.len() > parameters.maximum_proof_bytes {
        return Err(AggregateStarkErrorV1::ProofTooLarge);
    }
    Ok(bytes)
}

fn reader_error_v1(error: TransparentStarkErrorV1) -> AggregateStarkErrorV1 {
    match error {
        TransparentStarkErrorV1::NonCanonicalField => AggregateStarkErrorV1::NonCanonicalField,
        _ => AggregateStarkErrorV1::MalformedProof,
    }
}

fn take_hash_v1(reader: &mut ExactProofReaderV1<'_>) -> Result<[u8; 32], AggregateStarkErrorV1> {
    reader.take().map_err(reader_error_v1)
}

fn take_hashes_v1(
    reader: &mut ExactProofReaderV1<'_>,
    count: usize,
) -> Result<Vec<[u8; 32]>, AggregateStarkErrorV1> {
    (0..count).map(|_| take_hash_v1(reader)).collect()
}

fn take_base_fields_v1(
    reader: &mut ExactProofReaderV1<'_>,
    count: usize,
) -> Result<Vec<u64>, AggregateStarkErrorV1> {
    (0..count)
        .map(|_| reader.field().map(|field| field.0).map_err(reader_error_v1))
        .collect()
}

fn take_fp4_fields_v1(
    reader: &mut ExactProofReaderV1<'_>,
    count: usize,
) -> Result<Vec<[u64; 4]>, AggregateStarkErrorV1> {
    (0..count)
        .map(|_| {
            reader
                .fp4()
                .map(|field| field.coefficients().map(F::value))
                .map_err(reader_error_v1)
        })
        .collect()
}

/// Decode one exact statement-shaped proof and reject every suffix.
pub(crate) fn decode_proof_v1(
    bytes: &[u8],
    parameters: AggregateStarkParametersV1,
    layout: &AggregateProofLayoutV1,
) -> Result<AggregateStarkProofV1, AggregateStarkErrorV1> {
    layout.validate(parameters)?;
    if bytes.is_empty() {
        return Err(AggregateStarkErrorV1::MalformedProof);
    }
    if bytes.len() > parameters.maximum_proof_bytes {
        return Err(AggregateStarkErrorV1::ProofTooLarge);
    }
    let mut reader = ExactProofReaderV1::new(bytes);
    if reader.take::<4>().map_err(reader_error_v1)? != parameters.proof_magic {
        return Err(AggregateStarkErrorV1::MalformedProof);
    }
    let version = reader.u16().map_err(reader_error_v1)?;
    let group_count = usize::from(reader.u16().map_err(reader_error_v1)?);
    if group_count != layout.trace_groups.len() {
        return Err(AggregateStarkErrorV1::InvalidProofShape);
    }
    let base_roots = take_hashes_v1(&mut reader, group_count)?;
    let aux_roots = take_hashes_v1(&mut reader, group_count)?;
    let mut trace_groups = base_roots
        .into_iter()
        .zip(aux_roots)
        .map(|(base_root, aux_root)| AggregateTraceGroupProofV1 {
            base_root,
            aux_root,
            base_frontier: Vec::new(),
            aux_frontier: Vec::new(),
        })
        .collect::<Vec<_>>();
    let composition_roots = take_hashes_v1(&mut reader, parameters.security_lanes)?;
    let fri_mask_roots = take_hashes_v1(&mut reader, parameters.security_lanes)?;
    let fri_rounds = layout.fri_rounds(parameters)?;
    let mut fri_lanes = Vec::with_capacity(parameters.security_lanes);
    for _ in 0..parameters.security_lanes {
        fri_lanes.push(AggregateFriLaneProofV1 {
            roots: take_hashes_v1(&mut reader, fri_rounds + 1)?,
            terminal_values: take_fp4_fields_v1(&mut reader, parameters.terminal_size()?)?,
            round_frontiers: Vec::new(),
        });
    }
    let grinding_nonce = reader.u64().map_err(reader_error_v1)?;
    let queries = (0..parameters.query_count)
        .map(|_| {
            let index = reader.u32().map_err(reader_error_v1)?;
            let trace_groups = layout
                .trace_groups
                .iter()
                .map(|group| {
                    Ok(AggregateTraceGroupQueryV1 {
                        base_current: take_base_fields_v1(&mut reader, group.base_width)?,
                        base_next: take_base_fields_v1(&mut reader, group.base_width)?,
                        aux_current: take_base_fields_v1(&mut reader, group.aux_width)?,
                        aux_next: take_base_fields_v1(&mut reader, group.aux_width)?,
                    })
                })
                .collect::<Result<Vec<_>, AggregateStarkErrorV1>>()?;
            let composition_values = (0..parameters.security_lanes)
                .map(|_| take_fp4_fields_v1(&mut reader, parameters.composition_degree_chunks))
                .collect::<Result<Vec<_>, _>>()?;
            let fri_mask_values = take_fp4_fields_v1(&mut reader, parameters.security_lanes)?;
            let fri_lanes = (0..parameters.security_lanes)
                .map(|_| {
                    let rounds = (0..fri_rounds)
                        .map(|_| {
                            Ok(AggregateFriRoundOpeningV1 {
                                low: reader
                                    .fp4()
                                    .map_err(reader_error_v1)?
                                    .coefficients()
                                    .map(F::value),
                                high: reader
                                    .fp4()
                                    .map_err(reader_error_v1)?
                                    .coefficients()
                                    .map(F::value),
                            })
                        })
                        .collect::<Result<Vec<_>, AggregateStarkErrorV1>>()?;
                    Ok(AggregateFriLaneQueryV1 { rounds })
                })
                .collect::<Result<Vec<_>, AggregateStarkErrorV1>>()?;
            Ok(AggregateQueryProofV1 {
                index,
                trace_groups,
                composition_values,
                fri_mask_values,
                fri_lanes,
            })
        })
        .collect::<Result<Vec<_>, AggregateStarkErrorV1>>()?;
    for (group_index, group) in trace_groups.iter_mut().enumerate() {
        let indices = trace_group_opening_indices_v1(&queries, layout, group_index)?;
        let count = multiproof_frontier_len_v1(layout.common_lde_size(), &indices)?;
        group.base_frontier = take_hashes_v1(&mut reader, count)?;
        group.aux_frontier = take_hashes_v1(&mut reader, count)?;
    }
    let composition_indices = composition_opening_indices_v1(&queries, layout)?;
    let composition_count =
        multiproof_frontier_len_v1(layout.common_lde_size(), &composition_indices)?;
    let composition_frontiers = (0..parameters.security_lanes)
        .map(|_| take_hashes_v1(&mut reader, composition_count))
        .collect::<Result<Vec<_>, _>>()?;
    let fri_mask_frontiers = (0..parameters.security_lanes)
        .map(|_| take_hashes_v1(&mut reader, composition_count))
        .collect::<Result<Vec<_>, _>>()?;
    for lane in &mut fri_lanes {
        lane.round_frontiers = (0..fri_rounds)
            .map(|round| {
                let indices = fri_opening_indices_v1(&queries, parameters, layout, round)?;
                let count =
                    multiproof_frontier_len_v1(layout.common_lde_size() >> round, &indices)?;
                take_hashes_v1(&mut reader, count)
            })
            .collect::<Result<Vec<_>, _>>()?;
    }
    reader.finish().map_err(reader_error_v1)?;
    let proof = AggregateStarkProofV1 {
        version,
        trace_groups,
        composition_roots,
        composition_frontiers,
        fri_mask_roots,
        fri_mask_frontiers,
        fri_lanes,
        queries,
        grinding_nonce,
    };
    validate_proof_shape_v1(&proof, parameters, layout)?;
    if exact_encoded_proof_bytes_v1(&proof, parameters, layout)? != bytes.len() {
        return Err(AggregateStarkErrorV1::MalformedProof);
    }
    Ok(proof)
}

/// Decode the sole canonical DEEP-enabled aggregate wire.
pub(crate) fn decode_proof_with_deep_v1(
    bytes: &[u8],
    parameters: AggregateStarkParametersV1,
    layout: &AggregateProofLayoutV1,
) -> Result<(AggregateStarkProofV1, AggregateDeepProofV1), AggregateStarkErrorV1> {
    layout.validate(parameters)?;
    if bytes.is_empty() {
        return Err(AggregateStarkErrorV1::MalformedProof);
    }
    if bytes.len() > parameters.maximum_proof_bytes {
        return Err(AggregateStarkErrorV1::ProofTooLarge);
    }
    let insertion = deep_insertion_offset_v1(parameters, layout)?;
    let deep_len = exact_deep_opening_bytes_v1(parameters, layout)?;
    let deep_end = insertion
        .checked_add(deep_len)
        .ok_or(AggregateStarkErrorV1::MalformedProof)?;
    if deep_end > bytes.len() {
        return Err(AggregateStarkErrorV1::MalformedProof);
    }
    let mut base_bytes = Vec::new();
    base_bytes
        .try_reserve_exact(
            bytes
                .len()
                .checked_sub(deep_len)
                .ok_or(AggregateStarkErrorV1::MalformedProof)?,
        )
        .map_err(|_| AggregateStarkErrorV1::AllocationFailure)?;
    base_bytes.extend_from_slice(&bytes[..insertion]);
    base_bytes.extend_from_slice(&bytes[deep_end..]);
    let proof = decode_proof_v1(&base_bytes, parameters, layout)?;
    let deep = decode_deep_openings_raw_v1(&bytes[insertion..deep_end], parameters, layout)?;
    if exact_encoded_proof_with_deep_bytes_v1(&proof, &deep, parameters, layout)? != bytes.len() {
        return Err(AggregateStarkErrorV1::MalformedProof);
    }
    Ok((proof, deep))
}

/// Prover material for one ordered trace group on the common LDE domain.
#[derive(Clone)]
pub(crate) struct AggregateTraceGroupMaterialV1 {
    /// Masked base LDE columns.
    pub(crate) base_lde: Vec<Vec<F>>,
    /// Masked auxiliary LDE columns.
    pub(crate) aux_lde: Vec<Vec<F>>,
    /// Base vector-row Merkle tree.
    pub(crate) base_tree: Sha256MerkleTreeV1,
    /// Auxiliary vector-row Merkle tree.
    pub(crate) aux_tree: Sha256MerkleTreeV1,
}

/// Prover material for one shared FRI lane.
#[derive(Clone)]
pub(crate) struct AggregateFriLaneMaterialV1 {
    /// Every FRI layer including the terminal vector.
    pub(crate) layers: Vec<Vec<E>>,
    /// Merkle tree for every layer including the terminal vector.
    pub(crate) trees: Vec<Sha256MerkleTreeV1>,
    /// Root for every layer including the terminal vector.
    pub(crate) roots: Vec<[u8; 32]>,
    /// Exact terminal evaluations.
    pub(crate) terminal_values: Vec<E>,
}

fn fold_fri_layer_v1(
    current: &[E],
    beta: E,
    domain_shift: F,
    domain_root: F,
) -> Result<Vec<E>, AggregateStarkErrorV1> {
    if current.len() < 2 || !current.len().is_power_of_two() {
        return Err(AggregateStarkErrorV1::InvalidLayout);
    }
    let half = current.len() / 2;
    let inverse_root = domain_root
        .inv()
        .ok_or(AggregateStarkErrorV1::InternalInvariant)?;
    let mut inverse_x = domain_shift
        .inv()
        .ok_or(AggregateStarkErrorV1::InternalInvariant)?;
    let mut next = Vec::new();
    next.try_reserve_exact(half)
        .map_err(|_| AggregateStarkErrorV1::AllocationFailure)?;
    for index in 0..half {
        next.push(
            fri_fold_pair_with_inverse_x_fp4_v1(
                current[index],
                current[index + half],
                beta,
                inverse_x,
            )
            .map_err(map_transparent_error_v1)?,
        );
        inverse_x = inverse_x.mul(inverse_root);
    }
    Ok(next)
}

/// Build and transcript-bind one complete shared binary-FRI lane.
pub(crate) fn build_fri_lane_v1(
    parameters: AggregateStarkParametersV1,
    domains: AggregateStarkDomainsV1,
    layout: &AggregateProofLayoutV1,
    lane: usize,
    base_values: Vec<E>,
    transcript: &mut TransparentTranscriptV1,
) -> Result<AggregateFriLaneMaterialV1, AggregateStarkErrorV1> {
    layout.validate(parameters)?;
    domains.validate()?;
    if lane >= parameters.security_lanes || base_values.len() != layout.common_lde_size() {
        return Err(AggregateStarkErrorV1::InvalidLayout);
    }
    let fri_rounds = layout.fri_rounds(parameters)?;
    let mut layers = vec![base_values];
    let mut trees = Vec::with_capacity(fri_rounds + 1);
    let mut roots = Vec::with_capacity(fri_rounds + 1);
    let mut domain_shift = F(GOLDILOCKS_GENERATOR_V1);
    let mut domain_root =
        goldilocks_primitive_root_v1(layout.common_lde_log2).map_err(map_transparent_error_v1)?;
    for round in 0..fri_rounds {
        let current = layers
            .last()
            .ok_or(AggregateStarkErrorV1::InternalInvariant)?;
        let tree = fri_tree_v1(domains, lane, round, current)?;
        let root = tree.root();
        absorb_fri_root_v1(transcript, domains, lane, round, &root)?;
        let beta = transcript
            .challenge_fp4(domains.fri_beta_label)
            .map_err(map_transparent_error_v1)?;
        let next = fold_fri_layer_v1(current, beta, domain_shift, domain_root)?;
        trees.push(tree);
        roots.push(root);
        layers.push(next);
        domain_shift = domain_shift.mul(domain_shift);
        domain_root = domain_root.mul(domain_root);
    }
    let terminal_values = layers
        .last()
        .ok_or(AggregateStarkErrorV1::InternalInvariant)?
        .clone();
    if terminal_values.len() != parameters.terminal_size()? {
        return Err(AggregateStarkErrorV1::InternalInvariant);
    }
    ensure_fri_terminal_degree_fp4_v1(
        &terminal_values,
        parameters.terminal_log2,
        parameters.terminal_degree_bound,
    )
    .map_err(map_transparent_error_v1)?;
    let terminal_tree = fri_tree_v1(domains, lane, fri_rounds, &terminal_values)?;
    let terminal_root = terminal_tree.root();
    absorb_fri_root_v1(transcript, domains, lane, fri_rounds, &terminal_root)?;
    roots.push(terminal_root);
    trees.push(terminal_tree);
    Ok(AggregateFriLaneMaterialV1 {
        layers,
        trees,
        roots,
        terminal_values,
    })
}

/// Bounded-memory transcript material for one FRI lane.
#[cfg(any(test, feature = "privacy-release-evidence"))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AggregateStreamingFriLaneMaterialV1 {
    /// Layer roots, including the terminal layer.
    pub(crate) roots: Vec<[u8; 32]>,
    /// Fiat–Shamir folding challenge for each non-terminal layer.
    pub(crate) betas: Vec<E>,
    /// Exact terminal evaluations.
    pub(crate) terminal_values: Vec<E>,
}

/// Post-query openings and frontiers for one streamed FRI lane.
#[cfg(any(test, feature = "privacy-release-evidence"))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AggregateStreamingFriLaneOpeningsV1 {
    /// Openings in the caller's canonical transcript-query order.
    pub(crate) queries: Vec<AggregateFriLaneQueryV1>,
    /// Canonical minimal frontier for each non-terminal layer.
    pub(crate) round_frontiers: Vec<Vec<[u8; 32]>>,
}

/// Commit and transcript-bind a complete FRI lane while retaining only one
/// current layer and one half-sized successor layer.
#[cfg(any(test, feature = "privacy-release-evidence"))]
pub(crate) fn build_streaming_fri_lane_v1(
    parameters: AggregateStarkParametersV1,
    domains: AggregateStarkDomainsV1,
    layout: &AggregateProofLayoutV1,
    lane: usize,
    base_values: Vec<E>,
    transcript: &mut TransparentTranscriptV1,
) -> Result<AggregateStreamingFriLaneMaterialV1, AggregateStarkErrorV1> {
    layout.validate(parameters)?;
    domains.validate()?;
    if lane >= parameters.security_lanes || base_values.len() != layout.common_lde_size() {
        return Err(AggregateStarkErrorV1::InvalidLayout);
    }
    let fri_rounds = layout.fri_rounds(parameters)?;
    let mut roots = Vec::new();
    roots
        .try_reserve_exact(
            fri_rounds
                .checked_add(1)
                .ok_or(AggregateStarkErrorV1::InvalidLayout)?,
        )
        .map_err(|_| AggregateStarkErrorV1::AllocationFailure)?;
    let mut betas = Vec::new();
    betas
        .try_reserve_exact(fri_rounds)
        .map_err(|_| AggregateStarkErrorV1::AllocationFailure)?;
    let mut current = base_values;
    let mut domain_shift = F(GOLDILOCKS_GENERATOR_V1);
    let mut domain_root =
        goldilocks_primitive_root_v1(layout.common_lde_log2).map_err(map_transparent_error_v1)?;
    for round in 0..fri_rounds {
        let commitment = streaming_fri_commitment_v1(domains, lane, round, &current, &[])?;
        absorb_fri_root_v1(transcript, domains, lane, round, &commitment.root)?;
        let beta = transcript
            .challenge_fp4(domains.fri_beta_label)
            .map_err(map_transparent_error_v1)?;
        let next = fold_fri_layer_v1(&current, beta, domain_shift, domain_root)?;
        roots.push(commitment.root);
        betas.push(beta);
        current = next;
        domain_shift = domain_shift.mul(domain_shift);
        domain_root = domain_root.mul(domain_root);
    }
    if current.len() != parameters.terminal_size()? {
        return Err(AggregateStarkErrorV1::InternalInvariant);
    }
    ensure_fri_terminal_degree_fp4_v1(
        &current,
        parameters.terminal_log2,
        parameters.terminal_degree_bound,
    )
    .map_err(map_transparent_error_v1)?;
    let terminal = streaming_fri_commitment_v1(domains, lane, fri_rounds, &current, &[])?;
    absorb_fri_root_v1(transcript, domains, lane, fri_rounds, &terminal.root)?;
    roots.push(terminal.root);
    Ok(AggregateStreamingFriLaneMaterialV1 {
        roots,
        betas,
        terminal_values: current,
    })
}

/// Replay a committed FRI lane after transcript queries are fixed, retaining
/// only the exact opened pairs and canonical minimal frontiers.
#[cfg(any(test, feature = "privacy-release-evidence"))]
pub(crate) fn open_streaming_fri_lane_v1(
    parameters: AggregateStarkParametersV1,
    domains: AggregateStarkDomainsV1,
    layout: &AggregateProofLayoutV1,
    lane: usize,
    base_values: Vec<E>,
    material: &AggregateStreamingFriLaneMaterialV1,
    query_indices: &[usize],
) -> Result<AggregateStreamingFriLaneOpeningsV1, AggregateStarkErrorV1> {
    layout.validate(parameters)?;
    domains.validate()?;
    let fri_rounds = layout.fri_rounds(parameters)?;
    if lane >= parameters.security_lanes
        || base_values.len() != layout.common_lde_size()
        || material.roots.len() != fri_rounds + 1
        || material.betas.len() != fri_rounds
        || material.terminal_values.len() != parameters.terminal_size()?
        || query_indices.len() != parameters.query_count
        || query_indices
            .iter()
            .any(|index| *index >= layout.common_lde_size())
        || query_indices.iter().copied().collect::<BTreeSet<_>>().len() != query_indices.len()
    {
        return Err(AggregateStarkErrorV1::InvalidProofShape);
    }
    let mut queries = Vec::new();
    queries
        .try_reserve_exact(query_indices.len())
        .map_err(|_| AggregateStarkErrorV1::AllocationFailure)?;
    for _ in query_indices {
        let mut rounds = Vec::new();
        rounds
            .try_reserve_exact(fri_rounds)
            .map_err(|_| AggregateStarkErrorV1::AllocationFailure)?;
        queries.push(AggregateFriLaneQueryV1 { rounds });
    }
    let mut round_frontiers = Vec::new();
    round_frontiers
        .try_reserve_exact(fri_rounds)
        .map_err(|_| AggregateStarkErrorV1::AllocationFailure)?;
    let mut layer_indices = query_indices.to_vec();
    let mut current = base_values;
    let mut domain_shift = F(GOLDILOCKS_GENERATOR_V1);
    let mut domain_root =
        goldilocks_primitive_root_v1(layout.common_lde_log2).map_err(map_transparent_error_v1)?;
    for round in 0..fri_rounds {
        let half = current.len() / 2;
        let opening_indices = layer_indices
            .iter()
            .flat_map(|index| {
                let low = *index % half;
                [low, low + half]
            })
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let commitment =
            streaming_fri_commitment_v1(domains, lane, round, &current, &opening_indices)?;
        if commitment.root != material.roots[round] {
            return Err(AggregateStarkErrorV1::InternalInvariant);
        }
        for (query, index) in queries.iter_mut().zip(&layer_indices) {
            let low = *index % half;
            query.rounds.push(AggregateFriRoundOpeningV1 {
                low: current[low].coefficients().map(F::value),
                high: current[low + half].coefficients().map(F::value),
            });
        }
        let next = fold_fri_layer_v1(&current, material.betas[round], domain_shift, domain_root)?;
        for index in &mut layer_indices {
            *index %= half;
        }
        round_frontiers.push(commitment.frontier);
        current = next;
        domain_shift = domain_shift.mul(domain_shift);
        domain_root = domain_root.mul(domain_root);
    }
    if current != material.terminal_values {
        return Err(AggregateStarkErrorV1::InternalInvariant);
    }
    let terminal = streaming_fri_commitment_v1(domains, lane, fri_rounds, &current, &[])?;
    if terminal.root != material.roots[fri_rounds] {
        return Err(AggregateStarkErrorV1::InternalInvariant);
    }
    Ok(AggregateStreamingFriLaneOpeningsV1 {
        queries,
        round_frontiers,
    })
}

/// Construct all exact opened values for one shared query index.
pub(crate) fn build_query_v1(
    parameters: AggregateStarkParametersV1,
    layout: &AggregateProofLayoutV1,
    index: usize,
    trace_groups: &[AggregateTraceGroupMaterialV1],
    compositions: &[Vec<Vec<E>>],
    fri_masks: &[AggregateFriMaskOracleMaterialV1],
    fri_lanes: &[AggregateFriLaneMaterialV1],
) -> Result<AggregateQueryProofV1, AggregateStarkErrorV1> {
    layout.validate(parameters)?;
    if index >= layout.common_lde_size()
        || trace_groups.len() != layout.trace_groups.len()
        || compositions.len() != parameters.security_lanes
        || fri_masks.len() != parameters.security_lanes
        || fri_lanes.len() != parameters.security_lanes
        || compositions.iter().any(|lane| {
            lane.len() != parameters.composition_degree_chunks
                || lane
                    .iter()
                    .any(|chunk| chunk.len() != layout.common_lde_size())
        })
        || fri_masks
            .iter()
            .any(|mask| mask.evaluations.len() != layout.common_lde_size())
    {
        return Err(AggregateStarkErrorV1::InvalidLayout);
    }
    let mut group_queries = Vec::with_capacity(trace_groups.len());
    for (material, descriptor) in trace_groups.iter().zip(&layout.trace_groups) {
        if material.base_lde.len() != descriptor.base_width
            || material.aux_lde.len() != descriptor.aux_width
            || material
                .base_lde
                .iter()
                .chain(&material.aux_lde)
                .any(|column| column.len() != layout.common_lde_size())
        {
            return Err(AggregateStarkErrorV1::InvalidLayout);
        }
        let next =
            (index + descriptor.next_stride(layout.common_lde_log2)?) % layout.common_lde_size();
        group_queries.push(AggregateTraceGroupQueryV1 {
            base_current: row_at_v1(&material.base_lde, index)?
                .into_iter()
                .map(|value| value.0)
                .collect(),
            base_next: row_at_v1(&material.base_lde, next)?
                .into_iter()
                .map(|value| value.0)
                .collect(),
            aux_current: row_at_v1(&material.aux_lde, index)?
                .into_iter()
                .map(|value| value.0)
                .collect(),
            aux_next: row_at_v1(&material.aux_lde, next)?
                .into_iter()
                .map(|value| value.0)
                .collect(),
        });
    }
    let composition_values = compositions
        .iter()
        .map(|lane| {
            lane.iter()
                .map(|chunk| chunk[index].coefficients().map(F::value))
                .collect()
        })
        .collect();
    let fri_mask_values = fri_masks
        .iter()
        .map(|mask| mask.evaluations[index].coefficients().map(F::value))
        .collect();
    let mut fri_queries = Vec::with_capacity(parameters.security_lanes);
    let fri_rounds = layout.fri_rounds(parameters)?;
    for fri in fri_lanes {
        if fri.layers.len() != fri_rounds + 1 {
            return Err(AggregateStarkErrorV1::InvalidLayout);
        }
        let mut layer_index = index;
        let mut rounds = Vec::with_capacity(fri_rounds);
        for round in 0..fri_rounds {
            let layer = &fri.layers[round];
            let half = layer.len() / 2;
            let low_index = layer_index % half;
            rounds.push(AggregateFriRoundOpeningV1 {
                low: layer[low_index].coefficients().map(F::value),
                high: layer[low_index + half].coefficients().map(F::value),
            });
            layer_index = low_index;
        }
        fri_queries.push(AggregateFriLaneQueryV1 { rounds });
    }
    Ok(AggregateQueryProofV1 {
        index: u32::try_from(index).map_err(|_| AggregateStarkErrorV1::InvalidLayout)?,
        trace_groups: group_queries,
        composition_values,
        fri_mask_values,
        fri_lanes: fri_queries,
    })
}

/// Build every canonical trace/composition/FRI frontier after queries are fixed.
pub(crate) fn build_all_frontiers_v1(
    parameters: AggregateStarkParametersV1,
    layout: &AggregateProofLayoutV1,
    queries: &[AggregateQueryProofV1],
    trace_groups: &[AggregateTraceGroupMaterialV1],
    composition_trees: &[Sha256MerkleTreeV1],
    fri_masks: &[AggregateFriMaskOracleMaterialV1],
    fri_lanes: &[AggregateFriLaneMaterialV1],
) -> Result<
    (
        Vec<(Vec<[u8; 32]>, Vec<[u8; 32]>)>,
        Vec<Vec<[u8; 32]>>,
        Vec<Vec<[u8; 32]>>,
        Vec<Vec<Vec<[u8; 32]>>>,
    ),
    AggregateStarkErrorV1,
> {
    layout.validate(parameters)?;
    if queries.len() != parameters.query_count
        || trace_groups.len() != layout.trace_groups.len()
        || composition_trees.len() != parameters.security_lanes
        || fri_masks.len() != parameters.security_lanes
        || fri_lanes.len() != parameters.security_lanes
    {
        return Err(AggregateStarkErrorV1::InvalidProofShape);
    }
    let trace_frontiers = trace_groups
        .iter()
        .enumerate()
        .map(|(group, material)| {
            let indices = trace_group_opening_indices_v1(queries, layout, group)?;
            Ok((
                canonical_multiproof_frontier_v1(
                    &material.base_tree,
                    layout.common_lde_size(),
                    &indices,
                )?,
                canonical_multiproof_frontier_v1(
                    &material.aux_tree,
                    layout.common_lde_size(),
                    &indices,
                )?,
            ))
        })
        .collect::<Result<Vec<_>, AggregateStarkErrorV1>>()?;
    let composition_indices = composition_opening_indices_v1(queries, layout)?;
    let composition_frontiers = composition_trees
        .iter()
        .map(|tree| {
            canonical_multiproof_frontier_v1(tree, layout.common_lde_size(), &composition_indices)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let fri_mask_frontiers = fri_masks
        .iter()
        .map(|mask| {
            canonical_multiproof_frontier_v1(
                &mask.tree,
                layout.common_lde_size(),
                &composition_indices,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let fri_frontiers = fri_lanes
        .iter()
        .map(|lane| {
            (0..layout.fri_rounds(parameters)?)
                .map(|round| {
                    let indices = fri_opening_indices_v1(queries, parameters, layout, round)?;
                    canonical_multiproof_frontier_v1(
                        &lane.trees[round],
                        layout.common_lde_size() >> round,
                        &indices,
                    )
                })
                .collect::<Result<Vec<_>, AggregateStarkErrorV1>>()
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok((
        trace_frontiers,
        composition_frontiers,
        fri_mask_frontiers,
        fri_frontiers,
    ))
}

/// Verify all base, auxiliary, composition, and non-terminal FRI multiproofs.
pub(crate) fn verify_all_merkle_openings_v1(
    proof: &AggregateStarkProofV1,
    parameters: AggregateStarkParametersV1,
    domains: AggregateStarkDomainsV1,
    layout: &AggregateProofLayoutV1,
    expected_indices: &[usize],
) -> Result<(), AggregateStarkErrorV1> {
    validate_proof_shape_v1(proof, parameters, layout)?;
    domains.validate()?;
    if expected_indices.len() != parameters.query_count {
        return Err(AggregateStarkErrorV1::TranscriptMismatch);
    }
    let mut base_leaves = (0..layout.trace_groups.len())
        .map(|_| BTreeMap::new())
        .collect::<Vec<_>>();
    let mut aux_leaves = (0..layout.trace_groups.len())
        .map(|_| BTreeMap::new())
        .collect::<Vec<_>>();
    let mut composition_leaves = (0..parameters.security_lanes)
        .map(|_| BTreeMap::new())
        .collect::<Vec<_>>();
    let mut fri_mask_leaves = (0..parameters.security_lanes)
        .map(|_| BTreeMap::new())
        .collect::<Vec<_>>();
    let fri_rounds = layout.fri_rounds(parameters)?;
    let mut fri_leaves = (0..parameters.security_lanes)
        .map(|_| (0..fri_rounds).map(|_| BTreeMap::new()).collect::<Vec<_>>())
        .collect::<Vec<_>>();
    for (position, query) in proof.queries.iter().enumerate() {
        let index =
            usize::try_from(query.index).map_err(|_| AggregateStarkErrorV1::TranscriptMismatch)?;
        if expected_indices.get(position).copied() != Some(index)
            || index >= layout.common_lde_size()
        {
            return Err(AggregateStarkErrorV1::TranscriptMismatch);
        }
        for (group_index, (opening, descriptor)) in query
            .trace_groups
            .iter()
            .zip(&layout.trace_groups)
            .enumerate()
        {
            let next = (index + descriptor.next_stride(layout.common_lde_log2)?)
                % layout.common_lde_size();
            let base_current = canonical_fields_v1(&opening.base_current, descriptor.base_width)?;
            let base_next = canonical_fields_v1(&opening.base_next, descriptor.base_width)?;
            let aux_current = canonical_fields_v1(&opening.aux_current, descriptor.aux_width)?;
            let aux_next = canonical_fields_v1(&opening.aux_next, descriptor.aux_width)?;
            insert_opened_leaf_v1(
                &mut base_leaves[group_index],
                index,
                row_leaf_hash_v1(domains.base_leaf, group_index, &base_current)?,
            )?;
            insert_opened_leaf_v1(
                &mut base_leaves[group_index],
                next,
                row_leaf_hash_v1(domains.base_leaf, group_index, &base_next)?,
            )?;
            insert_opened_leaf_v1(
                &mut aux_leaves[group_index],
                index,
                row_leaf_hash_v1(domains.aux_leaf, group_index, &aux_current)?,
            )?;
            insert_opened_leaf_v1(
                &mut aux_leaves[group_index],
                next,
                row_leaf_hash_v1(domains.aux_leaf, group_index, &aux_next)?,
            )?;
        }
        for lane in 0..parameters.security_lanes {
            let composition = canonical_fp4_fields_v1(
                &query.composition_values[lane],
                parameters.composition_degree_chunks,
            )?;
            insert_opened_leaf_v1(
                &mut composition_leaves[lane],
                index,
                composition_leaf_hash_unchecked_v1(domains, lane, &composition)?,
            )
            .map_err(|_| AggregateStarkErrorV1::ConstraintOpening)?;
            let fri_mask = E::canonical(query.fri_mask_values[lane])
                .ok_or(AggregateStarkErrorV1::NonCanonicalField)?;
            insert_opened_leaf_v1(
                &mut fri_mask_leaves[lane],
                index,
                fri_mask_leaf_hash_v1(lane, fri_mask)?,
            )
            .map_err(|_| AggregateStarkErrorV1::FriOpening)?;
            let mut layer_index = index;
            let mut layer_size = layout.common_lde_size();
            for round in 0..fri_rounds {
                let opening = query.fri_lanes[lane].rounds[round];
                let low =
                    E::canonical(opening.low).ok_or(AggregateStarkErrorV1::NonCanonicalField)?;
                let high =
                    E::canonical(opening.high).ok_or(AggregateStarkErrorV1::NonCanonicalField)?;
                let half = layer_size / 2;
                let low_index = layer_index % half;
                insert_opened_leaf_v1(
                    &mut fri_leaves[lane][round],
                    low_index,
                    fri_leaf_hash_unchecked_v1(domains, lane, round, low)?,
                )
                .map_err(|_| AggregateStarkErrorV1::FriOpening)?;
                insert_opened_leaf_v1(
                    &mut fri_leaves[lane][round],
                    low_index + half,
                    fri_leaf_hash_unchecked_v1(domains, lane, round, high)?,
                )
                .map_err(|_| AggregateStarkErrorV1::FriOpening)?;
                layer_index = low_index;
                layer_size = half;
            }
        }
    }
    for group in 0..layout.trace_groups.len() {
        verify_canonical_multiproof_v1(
            domains.base_node,
            &proof.trace_groups[group].base_root,
            layout.common_lde_size(),
            &base_leaves[group],
            &proof.trace_groups[group].base_frontier,
        )?;
        verify_canonical_multiproof_v1(
            domains.aux_node,
            &proof.trace_groups[group].aux_root,
            layout.common_lde_size(),
            &aux_leaves[group],
            &proof.trace_groups[group].aux_frontier,
        )?;
    }
    for lane in 0..parameters.security_lanes {
        verify_canonical_multiproof_v1(
            domains.composition_node,
            &proof.composition_roots[lane],
            layout.common_lde_size(),
            &composition_leaves[lane],
            &proof.composition_frontiers[lane],
        )
        .map_err(|_| AggregateStarkErrorV1::ConstraintOpening)?;
        verify_canonical_multiproof_v1(
            FRI_MASK_NODE_DOMAIN_V1,
            &proof.fri_mask_roots[lane],
            layout.common_lde_size(),
            &fri_mask_leaves[lane],
            &proof.fri_mask_frontiers[lane],
        )
        .map_err(|_| AggregateStarkErrorV1::FriOpening)?;
        for round in 0..fri_rounds {
            verify_canonical_multiproof_v1(
                domains.fri_node,
                &proof.fri_lanes[lane].roots[round],
                layout.common_lde_size() >> round,
                &fri_leaves[lane][round],
                &proof.fri_lanes[lane].round_frontiers[round],
            )
            .map_err(|_| AggregateStarkErrorV1::FriOpening)?;
        }
    }
    Ok(())
}

/// Verify and transcript-bind terminal FRI vectors, returning fold challenges.
pub(crate) fn verify_fri_commitments_v1(
    proof: &AggregateStarkProofV1,
    parameters: AggregateStarkParametersV1,
    domains: AggregateStarkDomainsV1,
    layout: &AggregateProofLayoutV1,
    transcript: &mut TransparentTranscriptV1,
) -> Result<(Vec<Vec<E>>, Vec<Vec<E>>), AggregateStarkErrorV1> {
    validate_proof_shape_v1(proof, parameters, layout)?;
    let fri_rounds = layout.fri_rounds(parameters)?;
    let mut all_betas = Vec::with_capacity(parameters.security_lanes);
    let mut all_terminals = Vec::with_capacity(parameters.security_lanes);
    for lane in 0..parameters.security_lanes {
        let lane_proof = &proof.fri_lanes[lane];
        let terminal =
            canonical_fp4_fields_v1(&lane_proof.terminal_values, parameters.terminal_size()?)?;
        let terminal_tree = fri_tree_v1(domains, lane, fri_rounds, &terminal)?;
        if terminal_tree.root() != lane_proof.roots[fri_rounds] {
            return Err(AggregateStarkErrorV1::FriOpening);
        }
        ensure_fri_terminal_degree_fp4_v1(
            &terminal,
            parameters.terminal_log2,
            parameters.terminal_degree_bound,
        )
        .map_err(map_transparent_error_v1)?;
        let mut betas = Vec::with_capacity(fri_rounds);
        for round in 0..fri_rounds {
            absorb_fri_root_v1(transcript, domains, lane, round, &lane_proof.roots[round])?;
            betas.push(
                transcript
                    .challenge_fp4(domains.fri_beta_label)
                    .map_err(map_transparent_error_v1)?,
            );
        }
        absorb_fri_root_v1(
            transcript,
            domains,
            lane,
            fri_rounds,
            &lane_proof.roots[fri_rounds],
        )?;
        all_betas.push(betas);
        all_terminals.push(terminal);
    }
    Ok((all_betas, all_terminals))
}

fn verify_fri_query_v1(
    query_index: usize,
    expected_base: E,
    parameters: AggregateStarkParametersV1,
    layout: &AggregateProofLayoutV1,
    query: &AggregateFriLaneQueryV1,
    betas: &[E],
    terminal: &[E],
) -> Result<(), AggregateStarkErrorV1> {
    let fri_rounds = layout.fri_rounds(parameters)?;
    if betas.len() != fri_rounds
        || query.rounds.len() != fri_rounds
        || terminal.len() != parameters.terminal_size()?
    {
        return Err(AggregateStarkErrorV1::InvalidProofShape);
    }
    let mut layer_index = query_index;
    let mut layer_size = layout.common_lde_size();
    let mut domain_shift = F(GOLDILOCKS_GENERATOR_V1);
    let mut domain_root =
        goldilocks_primitive_root_v1(layout.common_lde_log2).map_err(map_transparent_error_v1)?;
    let mut expected = expected_base;
    for round in 0..fri_rounds {
        let opening = query.rounds[round];
        let low = E::canonical(opening.low).ok_or(AggregateStarkErrorV1::NonCanonicalField)?;
        let high = E::canonical(opening.high).ok_or(AggregateStarkErrorV1::NonCanonicalField)?;
        let half = layer_size / 2;
        let low_index = layer_index % half;
        let selected = if layer_index < half { low } else { high };
        if selected != expected {
            return Err(AggregateStarkErrorV1::FriOpening);
        }
        let x = domain_shift.mul(domain_root.pow(low_index as u128));
        expected = fri_fold_pair_fp4_v1(low, high, betas[round], x)
            .map_err(|_| AggregateStarkErrorV1::FriOpening)?;
        layer_index = low_index;
        layer_size = half;
        domain_shift = domain_shift.mul(domain_shift);
        domain_root = domain_root.mul(domain_root);
    }
    if layer_size != parameters.terminal_size()?
        || terminal.get(layer_index).copied() != Some(expected)
    {
        return Err(AggregateStarkErrorV1::FriOpening);
    }
    Ok(())
}

/// Invoke the relation callback for every opened row and bind its results to FRI.
#[cfg(test)]
pub(crate) fn verify_opened_query_relations_v1<Evaluator: AggregateOpenedRowEvaluatorV1>(
    proof: &AggregateStarkProofV1,
    parameters: AggregateStarkParametersV1,
    layout: &AggregateProofLayoutV1,
    expected_indices: &[usize],
    fri_betas: &[Vec<E>],
    terminals: &[Vec<E>],
    evaluator: &mut Evaluator,
) -> Result<(), AggregateStarkErrorV1> {
    validate_proof_shape_v1(proof, parameters, layout)?;
    if expected_indices.len() != parameters.query_count
        || fri_betas.len() != parameters.security_lanes
        || terminals.len() != parameters.security_lanes
    {
        return Err(AggregateStarkErrorV1::InvalidProofShape);
    }
    for (position, query) in proof.queries.iter().enumerate() {
        let index =
            usize::try_from(query.index).map_err(|_| AggregateStarkErrorV1::TranscriptMismatch)?;
        if expected_indices.get(position).copied() != Some(index)
            || index >= layout.common_lde_size()
        {
            return Err(AggregateStarkErrorV1::TranscriptMismatch);
        }
        let opened_groups = query
            .trace_groups
            .iter()
            .zip(&layout.trace_groups)
            .map(|(opening, descriptor)| {
                Ok(AggregateOpenedTraceGroupV1 {
                    base_current: canonical_fields_v1(
                        &opening.base_current,
                        descriptor.base_width,
                    )?,
                    base_next: canonical_fields_v1(&opening.base_next, descriptor.base_width)?,
                    aux_current: canonical_fields_v1(&opening.aux_current, descriptor.aux_width)?,
                    aux_next: canonical_fields_v1(&opening.aux_next, descriptor.aux_width)?,
                })
            })
            .collect::<Result<Vec<_>, AggregateStarkErrorV1>>()?;
        for lane in 0..parameters.security_lanes {
            let composition_chunks = canonical_fp4_fields_v1(
                &query.composition_values[lane],
                parameters.composition_degree_chunks,
            )?;
            let lde_root = goldilocks_primitive_root_v1(layout.common_lde_log2)
                .map_err(map_transparent_error_v1)?;
            let x = F(GOLDILOCKS_GENERATOR_V1).mul(lde_root.pow(index as u128));
            let composition =
                recompose_composition_value_v1(&composition_chunks, x, parameters, layout)?;
            let expected = evaluator.evaluate_opened_row_v1(
                index,
                lane,
                &opened_groups,
                &composition_chunks,
            )?;
            if composition != expected.composition {
                return Err(AggregateStarkErrorV1::ConstraintOpening);
            }
            let fri_mask = E::canonical(query.fri_mask_values[lane])
                .ok_or(AggregateStarkErrorV1::NonCanonicalField)?;
            verify_fri_query_v1(
                index,
                expected.fri_base.add(fri_mask),
                parameters,
                layout,
                &query.fri_lanes[lane],
                &fri_betas[lane],
                &terminals[lane],
            )?;
        }
    }
    Ok(())
}

/// Verify opened AIR constraints and the complete DEEP-ALI quotient before
/// binding every query to FRI.
///
/// The relation callback still recomputes the constraint quotient from the
/// authenticated current/next trace rows. The FRI base is no longer that raw
/// row mix: it is the verifier-computed random linear combination of all
/// current, next, and composition differences divided by `x - z`. Consequently
/// every encoded out-of-domain value is tied to a committed low-degree
/// polynomial, rather than being an unauthenticated transcript decoration.
#[allow(clippy::too_many_arguments)]
pub(crate) fn verify_opened_query_relations_with_deep_v1<
    Evaluator: AggregateOpenedRowEvaluatorV1,
>(
    proof: &AggregateStarkProofV1,
    deep: &AggregateDeepProofV1,
    deep_point: E,
    deep_mixes: &[AggregateDeepLaneMixV1],
    parameters: AggregateStarkParametersV1,
    layout: &AggregateProofLayoutV1,
    expected_indices: &[usize],
    fri_betas: &[Vec<E>],
    terminals: &[Vec<E>],
    evaluator: &mut Evaluator,
) -> Result<(), AggregateStarkErrorV1> {
    validate_proof_shape_v1(proof, parameters, layout)?;
    validate_deep_proof_shape_v1(deep, parameters, layout)?;
    validate_deep_lane_mixes_v1(deep_mixes, parameters, layout)?;
    let deep_trace_groups = canonical_deep_trace_groups_v1(deep, parameters, layout)?;
    let deep_compositions = deep
        .composition_values
        .iter()
        .map(|values| canonical_fp4_fields_v1(values, parameters.composition_degree_chunks))
        .collect::<Result<Vec<_>, _>>()?;
    if expected_indices.len() != parameters.query_count
        || fri_betas.len() != parameters.security_lanes
        || terminals.len() != parameters.security_lanes
    {
        return Err(AggregateStarkErrorV1::InvalidProofShape);
    }
    let lde_root =
        goldilocks_primitive_root_v1(layout.common_lde_log2).map_err(map_transparent_error_v1)?;
    for (position, query) in proof.queries.iter().enumerate() {
        let index =
            usize::try_from(query.index).map_err(|_| AggregateStarkErrorV1::TranscriptMismatch)?;
        if expected_indices.get(position).copied() != Some(index)
            || index >= layout.common_lde_size()
        {
            return Err(AggregateStarkErrorV1::TranscriptMismatch);
        }
        let opened_groups = query
            .trace_groups
            .iter()
            .zip(&layout.trace_groups)
            .map(|(opening, descriptor)| {
                Ok(AggregateOpenedTraceGroupV1 {
                    base_current: canonical_fields_v1(
                        &opening.base_current,
                        descriptor.base_width,
                    )?,
                    base_next: canonical_fields_v1(&opening.base_next, descriptor.base_width)?,
                    aux_current: canonical_fields_v1(&opening.aux_current, descriptor.aux_width)?,
                    aux_next: canonical_fields_v1(&opening.aux_next, descriptor.aux_width)?,
                })
            })
            .collect::<Result<Vec<_>, AggregateStarkErrorV1>>()?;
        let x = F(GOLDILOCKS_GENERATOR_V1).mul(lde_root.pow(index as u128));
        let query_point = E::from_base(x);
        for lane in 0..parameters.security_lanes {
            let composition_chunks = canonical_fp4_fields_v1(
                &query.composition_values[lane],
                parameters.composition_degree_chunks,
            )?;
            let composition =
                recompose_composition_value_v1(&composition_chunks, x, parameters, layout)?;
            let expected = evaluator.evaluate_opened_row_v1(
                index,
                lane,
                &opened_groups,
                &composition_chunks,
            )?;
            if composition != expected.composition {
                return Err(AggregateStarkErrorV1::ConstraintOpening);
            }
            let fri_base = deep_ali_mixed_opening_v1(
                query_point,
                deep_point,
                layout,
                &opened_groups,
                &deep_trace_groups,
                &composition_chunks,
                &deep_compositions[lane],
                &deep_mixes[lane],
            )?;
            let fri_mask = E::canonical(query.fri_mask_values[lane])
                .ok_or(AggregateStarkErrorV1::NonCanonicalField)?;
            verify_fri_query_v1(
                index,
                fri_base.add(fri_mask),
                parameters,
                layout,
                &query.fri_lanes[lane],
                &fri_betas[lane],
                &terminals[lane],
            )?;
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "aggregate_stark/retained_polynomial_tests.rs"]
mod retained_polynomial_tests;

#[cfg(test)]
mod tests {
    use rand::{SeedableRng as _, rngs::StdRng};

    use super::*;

    const PARAMETERS: AggregateStarkParametersV1 = AggregateStarkParametersV1 {
        proof_magic: *b"AGG1",
        proof_version: 1,
        security_lanes: 2,
        query_count: 2,
        blowup_log2: 3,
        terminal_log2: 3,
        terminal_degree_bound: 3,
        composition_degree_chunks: 3,
        minimum_trace_log2: 3,
        maximum_trace_log2: 6,
        maximum_trace_groups: 4,
        maximum_segment_instances: 4,
        maximum_base_columns_per_instance: 4,
        maximum_aux_columns_per_instance: 4,
        maximum_proof_bytes: 1 << 20,
    };

    const DOMAINS: AggregateStarkDomainsV1 = AggregateStarkDomainsV1 {
        base_leaf: b"aggregate-test-base-leaf",
        base_node: b"aggregate-test-base-node",
        aux_leaf: b"aggregate-test-aux-leaf",
        aux_node: b"aggregate-test-aux-node",
        composition_leaf: b"aggregate-test-composition-leaf",
        composition_node: b"aggregate-test-composition-node",
        fri_leaf: b"aggregate-test-fri-leaf",
        fri_node: b"aggregate-test-fri-node",
        layout_label: b"aggregate-test-layout-label",
        base_root_label: b"aggregate-test-base-root-label",
        aux_root_label: b"aggregate-test-aux-root-label",
        composition_root_label: b"aggregate-test-composition-root-label",
        fri_root_label: b"aggregate-test-fri-root-label",
        fri_beta_label: b"aggregate-test-fri-beta-label",
        query_seed: b"aggregate-test-query-seed",
    };

    fn transcript() -> TransparentTranscriptV1 {
        TransparentTranscriptV1::new(b"aggregate-test-suite", &[7; 32], &[9; 32])
            .expect("transcript")
    }

    #[test]
    fn aggregate_domains_cannot_alias_fixed_deep_transcript_labels() {
        assert!(DOMAINS.validate().is_ok());
        for reserved in [DEEP_POINT_LABEL_V1, DEEP_OPENINGS_LABEL_V1] {
            assert_eq!(
                AggregateStarkDomainsV1 {
                    query_seed: reserved,
                    ..DOMAINS
                }
                .validate(),
                Err(AggregateStarkErrorV1::InvalidLayout)
            );
        }
    }

    fn layout() -> AggregateProofLayoutV1 {
        AggregateProofLayoutV1::new(
            PARAMETERS,
            vec![AggregateTraceGroupLayoutV1 {
                native_trace_log2: 3,
                segment_instances: 1,
                base_width: 1,
                aux_width: 1,
            }],
        )
        .expect("layout")
    }

    fn release_fri_parameters_v1() -> AggregateStarkParametersV1 {
        AggregateStarkParametersV1 {
            proof_magic: *b"FRI2",
            proof_version: 1,
            security_lanes: 1,
            query_count: 58,
            blowup_log2: 6,
            terminal_log2: 10,
            terminal_degree_bound: 31,
            composition_degree_chunks: 4,
            minimum_trace_log2: 19,
            maximum_trace_log2: 19,
            maximum_trace_groups: 1,
            maximum_segment_instances: 1,
            maximum_base_columns_per_instance: 1,
            maximum_aux_columns_per_instance: 1,
            maximum_proof_bytes: 8 << 20,
        }
    }

    fn release_fri_layout_v1(parameters: AggregateStarkParametersV1) -> AggregateProofLayoutV1 {
        AggregateProofLayoutV1::new(
            parameters,
            vec![AggregateTraceGroupLayoutV1 {
                native_trace_log2: 19,
                segment_instances: 1,
                base_width: 1,
                aux_width: 1,
            }],
        )
        .expect("release FRI layout")
    }

    fn release_fri_certificate_v1() -> AggregateFriTheorem2CertificateV1 {
        AggregateFriTheorem2CertificateV1 {
            l_minus_one_numerator: 3,
            l_minus_one_denominator: 2,
            batching_parameter_m: 3,
            rho_numerator: 1,
            rho_denominator: 32,
            affine_arities: [2, 2, 2],
            domain_log2: 25,
            extension_field_lower_bound_bits: 252,
            base_field_two_adicity: 32,
            trace_domains_are_smooth_subgroups: true,
            evaluation_domain_is_smooth_generator_coset: true,
            evaluation_domain_is_disjoint_from_trace_domains: true,
            fold_count: 15,
            terminal_log2: 10,
            terminal_degree_bound: 31,
            query_count: 58,
            distinct_queries_without_replacement: true,
            uniform_rejection_sampling: true,
            claimed_query_error_bits: 132,
        }
    }

    #[test]
    fn affine_batched_fri_theorem_certificate_checks_every_precondition() {
        let parameters = release_fri_parameters_v1();
        let layout = release_fri_layout_v1(parameters);
        let certificate = release_fri_certificate_v1();
        assert_eq!(
            validate_affine_batched_fri_theorem2_v1(parameters, &layout, certificate)
                .expect("canonical theorem certificate"),
            AggregateFriTheorem2BoundV1 {
                query_error_bits: 132,
                commitment_error_bits: 181,
            }
        );

        let mutations = [
            AggregateFriTheorem2CertificateV1 {
                l_minus_one_numerator: 2,
                ..certificate
            },
            AggregateFriTheorem2CertificateV1 {
                batching_parameter_m: 2,
                ..certificate
            },
            AggregateFriTheorem2CertificateV1 {
                rho_denominator: 31,
                ..certificate
            },
            AggregateFriTheorem2CertificateV1 {
                affine_arities: [2, 2, 1],
                ..certificate
            },
            AggregateFriTheorem2CertificateV1 {
                domain_log2: 24,
                ..certificate
            },
            AggregateFriTheorem2CertificateV1 {
                extension_field_lower_bound_bits: 251,
                ..certificate
            },
            AggregateFriTheorem2CertificateV1 {
                base_field_two_adicity: 24,
                ..certificate
            },
            AggregateFriTheorem2CertificateV1 {
                trace_domains_are_smooth_subgroups: false,
                ..certificate
            },
            AggregateFriTheorem2CertificateV1 {
                evaluation_domain_is_smooth_generator_coset: false,
                ..certificate
            },
            AggregateFriTheorem2CertificateV1 {
                evaluation_domain_is_disjoint_from_trace_domains: false,
                ..certificate
            },
            AggregateFriTheorem2CertificateV1 {
                fold_count: 14,
                ..certificate
            },
            AggregateFriTheorem2CertificateV1 {
                terminal_log2: 9,
                ..certificate
            },
            AggregateFriTheorem2CertificateV1 {
                terminal_degree_bound: 30,
                ..certificate
            },
            AggregateFriTheorem2CertificateV1 {
                query_count: 57,
                ..certificate
            },
            AggregateFriTheorem2CertificateV1 {
                distinct_queries_without_replacement: false,
                ..certificate
            },
            AggregateFriTheorem2CertificateV1 {
                uniform_rejection_sampling: false,
                ..certificate
            },
        ];
        for (index, mutation) in mutations.into_iter().enumerate() {
            assert!(
                validate_affine_batched_fri_theorem2_v1(parameters, &layout, mutation).is_err(),
                "theorem-precondition mutation {index} must fail closed"
            );
        }

        let mut weak_parameters = parameters;
        weak_parameters.query_count = 56;
        let weak_layout = release_fri_layout_v1(weak_parameters);
        let weak_certificate = AggregateFriTheorem2CertificateV1 {
            query_count: 56,
            ..certificate
        };
        assert!(
            validate_affine_batched_fri_theorem2_v1(
                weak_parameters,
                &weak_layout,
                weak_certificate,
            )
            .is_err(),
            "q=56 cannot substantiate the claimed 132-bit query term"
        );
    }

    fn fixture() -> (
        AggregateProofLayoutV1,
        AggregateStarkProofV1,
        Vec<Vec<Vec<E>>>,
        Vec<Vec<E>>,
    ) {
        let layout = layout();
        let rows = layout.common_lde_size();
        let base_lde = vec![(0..rows).map(|index| F(index as u64)).collect()];
        let aux_lde = vec![
            (0..rows)
                .map(|index| F((index as u64).wrapping_mul(3)))
                .collect(),
        ];
        let base_tree = row_tree_v1(DOMAINS.base_leaf, DOMAINS.base_node, 0, &base_lde, rows)
            .expect("base tree");
        let aux_tree =
            row_tree_v1(DOMAINS.aux_leaf, DOMAINS.aux_node, 0, &aux_lde, rows).expect("aux tree");
        let mut group_proofs = vec![AggregateTraceGroupProofV1 {
            base_root: base_tree.root(),
            aux_root: aux_tree.root(),
            base_frontier: Vec::new(),
            aux_frontier: Vec::new(),
        }];
        let compositions = vec![
            vec![vec![E::ZERO; rows]; PARAMETERS.composition_degree_chunks];
            PARAMETERS.security_lanes
        ];
        let composition_trees = (0..PARAMETERS.security_lanes)
            .map(|lane| composition_tree_v1(DOMAINS, lane, &compositions[lane]))
            .collect::<Result<Vec<_>, _>>()
            .expect("composition trees");
        let composition_roots = composition_trees
            .iter()
            .map(Sha256MerkleTreeV1::root)
            .collect::<Vec<_>>();
        let mut fri_mask_rng = StdRng::seed_from_u64(0x4652_494d_4153_4b31);
        let fri_masks = build_fri_mask_oracles_v1(PARAMETERS, &layout, &mut fri_mask_rng)
            .expect("FRI mask oracles");
        let fri_mask_roots = fri_masks
            .iter()
            .map(|mask| mask.tree.root())
            .collect::<Vec<_>>();
        let mut prover_transcript = transcript();
        absorb_layout_v1(
            &mut prover_transcript,
            PARAMETERS,
            DOMAINS,
            b"aggregate-test-relation-layout",
            &layout,
        )
        .expect("layout absorption");
        absorb_base_roots_v1(&mut prover_transcript, DOMAINS, &group_proofs)
            .expect("base absorption");
        absorb_aux_roots_v1(&mut prover_transcript, DOMAINS, &group_proofs)
            .expect("aux absorption");
        absorb_composition_roots_v1(
            &mut prover_transcript,
            PARAMETERS,
            DOMAINS,
            &composition_roots,
        )
        .expect("composition absorption");
        absorb_fri_mask_roots_v1(&mut prover_transcript, PARAMETERS, &fri_mask_roots)
            .expect("FRI mask absorption");
        let fri_lanes = (0..PARAMETERS.security_lanes)
            .map(|lane| {
                let mut fri_base = vec![E::ZERO; rows];
                add_fri_mask_oracle_v1(&mut fri_base, &fri_masks[lane]).expect("add FRI mask");
                build_fri_lane_v1(
                    PARAMETERS,
                    DOMAINS,
                    &layout,
                    lane,
                    fri_base,
                    &mut prover_transcript,
                )
            })
            .collect::<Result<Vec<_>, _>>()
            .expect("FRI");
        let trace_material = vec![AggregateTraceGroupMaterialV1 {
            base_lde,
            aux_lde,
            base_tree,
            aux_tree,
        }];
        let queries = [1_usize, 7]
            .into_iter()
            .map(|index| {
                build_query_v1(
                    PARAMETERS,
                    &layout,
                    index,
                    &trace_material,
                    &compositions,
                    &fri_masks,
                    &fri_lanes,
                )
            })
            .collect::<Result<Vec<_>, _>>()
            .expect("queries");
        let (trace_frontiers, composition_frontiers, fri_mask_frontiers, fri_frontiers) =
            build_all_frontiers_v1(
                PARAMETERS,
                &layout,
                &queries,
                &trace_material,
                &composition_trees,
                &fri_masks,
                &fri_lanes,
            )
            .expect("frontiers");
        for (proof, (base, aux)) in group_proofs.iter_mut().zip(trace_frontiers) {
            proof.base_frontier = base;
            proof.aux_frontier = aux;
        }
        let proof = AggregateStarkProofV1 {
            version: PARAMETERS.proof_version,
            trace_groups: group_proofs,
            composition_roots,
            composition_frontiers,
            fri_mask_roots,
            fri_mask_frontiers,
            fri_lanes: fri_lanes
                .into_iter()
                .zip(fri_frontiers)
                .map(|(lane, round_frontiers)| AggregateFriLaneProofV1 {
                    roots: lane.roots,
                    terminal_values: lane
                        .terminal_values
                        .into_iter()
                        .map(|value| value.coefficients().map(F::value))
                        .collect(),
                    round_frontiers,
                })
                .collect(),
            queries,
            grinding_nonce: 0,
        };
        (layout, proof, compositions, vec![vec![E::ZERO; rows]; 2])
    }

    fn deep_fixture(layout: &AggregateProofLayoutV1) -> AggregateDeepProofV1 {
        let mut next_value = 1_u64;
        let mut values = |count: usize| {
            (0..count)
                .map(|_| {
                    let value = [
                        next_value,
                        next_value + 10_000,
                        next_value + 20_000,
                        next_value + 30_000,
                    ];
                    next_value += 1;
                    value
                })
                .collect::<Vec<_>>()
        };
        let trace_groups = layout
            .trace_groups
            .iter()
            .map(|group| AggregateDeepTraceGroupOpeningV1 {
                base_current: values(group.base_width),
                base_next: values(group.base_width),
                aux_current: values(group.aux_width),
                aux_next: values(group.aux_width),
            })
            .collect();
        let composition_values = (0..PARAMETERS.security_lanes)
            .map(|_| values(PARAMETERS.composition_degree_chunks))
            .collect();
        AggregateDeepProofV1 {
            trace_groups,
            composition_values,
        }
    }

    #[test]
    fn deep_point_exclusion_covers_trace_evaluation_query_and_next_domains() {
        let layout = layout();
        assert!(
            !deep_point_is_admissible_v1(E::ONE, PARAMETERS, &layout)
                .expect("trace-domain predicate")
        );
        let evaluation_shift = F(GOLDILOCKS_GENERATOR_V1);
        assert!(
            !deep_point_is_admissible_v1(E::from_base(evaluation_shift), PARAMETERS, &layout)
                .expect("evaluation-domain predicate")
        );
        let native_root = goldilocks_primitive_root_v1(layout.trace_groups[0].native_trace_log2)
            .expect("native root");
        let next_collision = evaluation_shift.mul(native_root.inv().expect("root inverse"));
        assert!(
            !deep_point_is_admissible_v1(E::from_base(next_collision), PARAMETERS, &layout)
                .expect("next-point predicate")
        );
        assert!(
            deep_point_is_admissible_v1(E::ZERO, PARAMETERS, &layout)
                .expect("zero is outside multiplicative domains")
        );
        let mut transcript = transcript();
        let derived =
            derive_deep_point_v1(&mut transcript, PARAMETERS, &layout).expect("derived DEEP point");
        assert!(
            deep_point_is_admissible_v1(derived, PARAMETERS, &layout)
                .expect("derived-point predicate")
        );
    }

    #[test]
    fn deep_codec_is_exact_and_every_value_is_transcript_bound() {
        let (layout, proof, _, _) = fixture();
        let deep = deep_fixture(&layout);
        let bytes =
            encode_proof_with_deep_v1(&proof, &deep, PARAMETERS, &layout).expect("DEEP encoding");
        let (decoded_proof, decoded_deep) =
            decode_proof_with_deep_v1(&bytes, PARAMETERS, &layout).expect("DEEP decoding");
        assert_eq!(decoded_proof, proof);
        assert_eq!(decoded_deep, deep);
        assert_eq!(
            bytes.len(),
            exact_encoded_proof_with_deep_bytes_v1(&proof, &deep, PARAMETERS, &layout)
                .expect("exact DEEP wire length")
        );
        assert_eq!(
            bytes.len(),
            encode_proof_v1(&proof, PARAMETERS, &layout)
                .expect("base encoding")
                .len()
                + exact_deep_opening_bytes_v1(PARAMETERS, &layout)
                    .expect("exact DEEP payload length")
        );

        let insertion = deep_insertion_offset_v1(PARAMETERS, &layout).expect("DEEP offset");
        let deep_len = exact_deep_opening_bytes_v1(PARAMETERS, &layout).expect("DEEP bytes");
        let field_count = deep_len / core::mem::size_of::<[u64; 4]>();
        let mut canonical_transcript = transcript();
        let canonical_point = derive_deep_point_v1(&mut canonical_transcript, PARAMETERS, &layout)
            .expect("canonical point");
        absorb_deep_openings_v1(&mut canonical_transcript, &deep, PARAMETERS, &layout)
            .expect("canonical DEEP absorption");
        let canonical_state = canonical_transcript.state();
        for field in 0..field_count {
            let mut changed_bytes = bytes.clone();
            let coefficient_offset = insertion + field * core::mem::size_of::<[u64; 4]>();
            let coefficient = u64::from_be_bytes(
                changed_bytes[coefficient_offset..coefficient_offset + 8]
                    .try_into()
                    .expect("coefficient bytes"),
            );
            changed_bytes[coefficient_offset..coefficient_offset + 8]
                .copy_from_slice(&(coefficient + 1).to_be_bytes());
            let (_, changed_deep) = decode_proof_with_deep_v1(&changed_bytes, PARAMETERS, &layout)
                .expect("canonical changed DEEP field");
            let mut changed_transcript = transcript();
            assert_eq!(
                derive_deep_point_v1(&mut changed_transcript, PARAMETERS, &layout)
                    .expect("changed point"),
                canonical_point
            );
            absorb_deep_openings_v1(&mut changed_transcript, &changed_deep, PARAMETERS, &layout)
                .expect("changed DEEP absorption");
            assert_ne!(
                changed_transcript.state(),
                canonical_state,
                "DEEP field {field} must affect the transcript"
            );
        }

        let mut reordered = bytes.clone();
        let first = reordered[insertion..insertion + 32].to_vec();
        let second = reordered[insertion + 32..insertion + 64].to_vec();
        reordered[insertion..insertion + 32].copy_from_slice(&second);
        reordered[insertion + 32..insertion + 64].copy_from_slice(&first);
        let (_, reordered_deep) = decode_proof_with_deep_v1(&reordered, PARAMETERS, &layout)
            .expect("shape-preserving reorder decodes");
        let mut reordered_transcript = transcript();
        derive_deep_point_v1(&mut reordered_transcript, PARAMETERS, &layout)
            .expect("reordered point");
        absorb_deep_openings_v1(
            &mut reordered_transcript,
            &reordered_deep,
            PARAMETERS,
            &layout,
        )
        .expect("reordered absorption");
        assert_ne!(reordered_transcript.state(), canonical_state);

        let mut omitted = bytes.clone();
        omitted.drain(insertion..insertion + 32);
        assert!(decode_proof_with_deep_v1(&omitted, PARAMETERS, &layout).is_err());
        let mut duplicated = bytes.clone();
        duplicated.splice(
            insertion..insertion,
            bytes[insertion..insertion + 32].iter().copied(),
        );
        assert!(decode_proof_with_deep_v1(&duplicated, PARAMETERS, &layout).is_err());
        let mut noncanonical = bytes;
        noncanonical[insertion..insertion + 8].copy_from_slice(
            &crate::privacy_engines::transparent_stark::GOLDILOCKS_MODULUS_V1.to_be_bytes(),
        );
        assert_eq!(
            decode_proof_with_deep_v1(&noncanonical, PARAMETERS, &layout),
            Err(AggregateStarkErrorV1::NonCanonicalField)
        );
    }

    #[test]
    fn fp4_batch_inversion_is_exact_and_rejects_zero_or_empty_inputs() {
        let original = [
            E::canonical([3, 5, 7, 11]).expect("field"),
            E::canonical([13, 17, 19, 23]).expect("field"),
            E::canonical([29, 31, 37, 41]).expect("field"),
        ];
        let mut inverses = original;
        batch_invert_fp4_nonzero_v1(&mut inverses).expect("batch inversion");
        for (value, inverse) in original.into_iter().zip(inverses) {
            assert_eq!(value.mul(inverse), E::ONE);
        }
        assert_eq!(
            batch_invert_fp4_nonzero_v1(&mut []),
            Err(AggregateStarkErrorV1::DeepOpening)
        );
        let mut contains_zero = [E::ONE, E::ZERO];
        assert_eq!(
            batch_invert_fp4_nonzero_v1(&mut contains_zero),
            Err(AggregateStarkErrorV1::DeepOpening)
        );
    }

    struct ZeroEvaluator;

    impl AggregateOpenedRowEvaluatorV1 for ZeroEvaluator {
        fn evaluate_opened_row_v1(
            &mut self,
            _query_index: usize,
            _lane: usize,
            _trace_groups: &[AggregateOpenedTraceGroupV1],
            _composition_chunks: &[E],
        ) -> Result<AggregateExpectedOpeningV1, AggregateStarkErrorV1> {
            Ok(AggregateExpectedOpeningV1 {
                composition: E::ZERO,
                fri_base: E::ZERO,
            })
        }
    }

    #[test]
    fn streaming_merkle_matches_materialized_tree_and_canonical_frontier() {
        let leaves = (0_u64..64)
            .map(|index| {
                sha256_frame_v1(b"aggregate-streaming-leaf", &[&index.to_be_bytes()]).expect("leaf")
            })
            .collect::<Vec<_>>();
        let tree = Sha256MerkleTreeV1::from_leaves(leaves.clone(), b"aggregate-streaming-node")
            .expect("tree");
        for indices in [
            Vec::new(),
            vec![0],
            vec![63],
            vec![0, 1],
            vec![1, 2, 7, 19, 32, 62],
            (0..64).step_by(3).collect::<Vec<_>>(),
            (0..64).collect::<Vec<_>>(),
        ] {
            let streamed = streaming_merkle_commitment_v1(
                b"aggregate-streaming-node",
                leaves.len(),
                &indices,
                leaves.iter().copied().map(Ok),
            )
            .expect("streamed commitment");
            assert_eq!(streamed.root, tree.root());
            if indices.is_empty() {
                assert!(streamed.frontier.is_empty());
                continue;
            }
            assert_eq!(
                streamed.frontier,
                canonical_multiproof_frontier_v1(&tree, leaves.len(), &indices)
                    .expect("materialized frontier")
            );
            let opened = indices
                .iter()
                .copied()
                .map(|index| (index, leaves[index]))
                .collect::<BTreeMap<_, _>>();
            verify_canonical_multiproof_v1(
                b"aggregate-streaming-node",
                &streamed.root,
                leaves.len(),
                &opened,
                &streamed.frontier,
            )
            .expect("streaming frontier verifies");
        }
    }

    #[test]
    fn streaming_merkle_rejects_noncanonical_or_inexact_streams() {
        let leaves = (0_u64..8)
            .map(|index| {
                sha256_frame_v1(b"aggregate-streaming-hostile-leaf", &[&index.to_be_bytes()])
                    .expect("leaf")
            })
            .collect::<Vec<_>>();
        for indices in [vec![1, 1], vec![2, 1], vec![8]] {
            assert!(
                StreamingMerkleAccumulatorV1::new(
                    b"aggregate-streaming-hostile-node",
                    leaves.len(),
                    &indices,
                )
                .is_err()
            );
        }
        assert!(StreamingMerkleAccumulatorV1::new(b"", leaves.len(), &[]).is_err());
        assert!(
            StreamingMerkleAccumulatorV1::new(
                b"aggregate-streaming-hostile-node",
                leaves.len() - 1,
                &[],
            )
            .is_err()
        );

        let missing = streaming_merkle_commitment_v1(
            b"aggregate-streaming-hostile-node",
            leaves.len(),
            &[1],
            leaves[..leaves.len() - 1].iter().copied().map(Ok),
        );
        assert_eq!(missing, Err(AggregateStarkErrorV1::InvalidProofShape));
        let trailing = streaming_merkle_commitment_v1(
            b"aggregate-streaming-hostile-node",
            leaves.len(),
            &[1],
            leaves
                .iter()
                .copied()
                .chain(core::iter::once(leaves[0]))
                .map(Ok),
        );
        assert_eq!(trailing, Err(AggregateStarkErrorV1::InvalidProofShape));
    }

    #[test]
    fn column_streamed_vector_rows_match_exact_leaf_framing_and_openings() {
        let rows = 64;
        let columns = (0_u64..7)
            .map(|column| {
                (0_u64..u64::try_from(rows).expect("rows"))
                    .map(|row| F::reduce(u128::from(column + 3) * u128::from(row + 11)))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let indices = vec![0, 1, 7, 31, 63];
        let tree = row_tree_v1(
            b"aggregate-streaming-row-leaf",
            b"aggregate-streaming-row-node",
            2,
            &columns,
            rows,
        )
        .expect("materialized row tree");
        let mut streamed = StreamingRowCommitmentV1::new(
            b"aggregate-streaming-row-leaf",
            b"aggregate-streaming-row-node",
            2,
            rows,
            columns.len(),
            &indices,
        )
        .expect("streaming row builder");
        for column in &columns {
            streamed.absorb_column(column).expect("column");
        }
        let result = streamed.finish().expect("streaming rows");
        assert_eq!(result.commitment.root, tree.root());
        assert_eq!(
            result.commitment.frontier,
            canonical_multiproof_frontier_v1(&tree, rows, &indices).expect("materialized frontier")
        );
        for index in indices {
            let expected = columns
                .iter()
                .map(|column| column[index])
                .collect::<Vec<_>>();
            assert_eq!(result.opened_rows.get(&index), Some(&expected));
            assert_eq!(
                row_leaf_hash_v1(
                    b"aggregate-streaming-row-leaf",
                    2,
                    result.opened_rows.get(&index).expect("opened row"),
                )
                .expect("leaf"),
                row_leaf_hash_v1(b"aggregate-streaming-row-leaf", 2, &expected)
                    .expect("reference leaf")
            );
        }
    }

    #[test]
    fn column_streamed_vector_rows_reject_shape_and_order_abuse() {
        assert!(
            StreamingRowCommitmentV1::new(
                b"aggregate-streaming-row-leaf",
                b"aggregate-streaming-row-node",
                0,
                8,
                2,
                &[2, 1],
            )
            .is_err()
        );
        let mut incomplete = StreamingRowCommitmentV1::new(
            b"aggregate-streaming-row-leaf",
            b"aggregate-streaming-row-node",
            0,
            8,
            2,
            &[1],
        )
        .expect("builder");
        assert_eq!(
            incomplete.absorb_column(&[F::ZERO; 7]),
            Err(AggregateStarkErrorV1::InvalidLayout)
        );
        incomplete
            .absorb_column(&[F::ZERO; 8])
            .expect("first column");
        assert_eq!(
            incomplete.finish(),
            Err(AggregateStarkErrorV1::InvalidLayout)
        );

        let mut overfull = StreamingRowCommitmentV1::new(
            b"aggregate-streaming-row-leaf",
            b"aggregate-streaming-row-node",
            0,
            8,
            1,
            &[],
        )
        .expect("builder");
        overfull.absorb_column(&[F::ZERO; 8]).expect("sole column");
        assert_eq!(
            overfull.absorb_column(&[F::ZERO; 8]),
            Err(AggregateStarkErrorV1::InvalidLayout)
        );
    }

    #[test]
    fn replayable_masked_column_commitment_matches_materialized_lde() {
        let native_columns = (0_u64..5)
            .map(|column| {
                (0_u64..8)
                    .map(|row| F::reduce(u128::from(column + 7) * u128::from(row + 13)))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let mut rng = StdRng::from_seed([0x42; 32]);
        let (root_only, masks) = commit_masked_trace_columns_v1(
            b"aggregate-streaming-masked-leaf",
            b"aggregate-streaming-masked-node",
            3,
            3,
            6,
            native_columns.len(),
            7,
            &[],
            &mut rng,
            |column| Ok(native_columns[column].clone()),
        )
        .expect("initial masked commitment");
        let indices = [0, 1, 7, 31, 63];
        let replay = replay_masked_trace_columns_v1(
            b"aggregate-streaming-masked-leaf",
            b"aggregate-streaming-masked-node",
            3,
            &masks,
            &indices,
            |column| Ok(native_columns[column].clone()),
        )
        .expect("masked replay");
        assert_eq!(replay.commitment.root, root_only.commitment.root);

        let materialized = native_columns
            .iter()
            .zip(&masks.masks)
            .map(|(column, mask)| {
                masked_trace_lde_column_with_mask_v1(column, 3, 6, mask.coefficients())
                    .expect("materialized masked LDE")
            })
            .collect::<Vec<_>>();
        let tree = row_tree_v1(
            b"aggregate-streaming-masked-leaf",
            b"aggregate-streaming-masked-node",
            3,
            &materialized,
            64,
        )
        .expect("materialized tree");
        assert_eq!(replay.commitment.root, tree.root());
        assert_eq!(
            replay.commitment.frontier,
            canonical_multiproof_frontier_v1(&tree, 64, &indices).expect("materialized frontier")
        );
        for index in indices {
            assert_eq!(
                replay.opened_rows.get(&index),
                Some(
                    &materialized
                        .iter()
                        .map(|column| column[index])
                        .collect::<Vec<_>>()
                )
            );
        }
    }

    #[test]
    fn encrypted_scratch_commitment_strategy_is_byte_exact_with_fast_path() {
        let columns = (0_u64..4)
            .map(|column| {
                (0_u64..8)
                    .map(|row| F::reduce(u128::from(column + 3) * u128::from(row + 19)))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let indices = [0, 5, 17, 63];
        let mut fast_rng = StdRng::from_seed([0x93; 32]);
        let (fast, fast_masks) = commit_masked_trace_columns_v1(
            b"aggregate-strategy-leaf",
            b"aggregate-strategy-node",
            2,
            3,
            6,
            columns.len(),
            7,
            &indices,
            &mut fast_rng,
            |column| Ok(columns[column].clone()),
        )
        .expect("fast commitment");
        let mut retained_rng = StdRng::from_seed([0x93; 32]);
        let (retained, retained_masks, mut retained_scratch) =
            commit_masked_trace_columns_retaining_encrypted_scratch_v1(
                b"aggregate-strategy-leaf",
                b"aggregate-strategy-node",
                2,
                3,
                6,
                columns.len(),
                7,
                &indices,
                &mut retained_rng,
                |column| Ok(columns[column].clone()),
            )
            .expect("retained scratch commitment");
        let mut scratch_rng = StdRng::from_seed([0x93; 32]);
        let (scratch, scratch_masks) = commit_masked_trace_columns_via_encrypted_scratch_v1(
            b"aggregate-strategy-leaf",
            b"aggregate-strategy-node",
            2,
            3,
            6,
            columns.len(),
            7,
            &indices,
            &mut scratch_rng,
            |column| Ok(columns[column].clone()),
        )
        .expect("scratch commitment");
        assert_eq!(retained, fast);
        assert_eq!(scratch, fast);
        assert_eq!(retained_scratch.rows(), 64);
        assert_eq!(retained_scratch.width(), columns.len());
        assert_eq!(retained_scratch.chunk_rows(), 64);
        assert_eq!(retained_scratch.chunk_count(), 1);
        assert!(retained_scratch.ciphertext_bytes() > 64 * columns.len() as u64 * 8);
        assert_eq!(retained_masks.width(), fast_masks.width());
        assert_eq!(scratch_masks.width(), fast_masks.width());
        for ((retained_mask, scratch_mask), fast_mask) in retained_masks
            .masks
            .iter()
            .zip(&scratch_masks.masks)
            .zip(&fast_masks.masks)
        {
            assert_eq!(retained_mask.coefficients(), fast_mask.coefficients());
            assert_eq!(scratch_mask.coefficients(), fast_mask.coefficients());
        }

        let replay_indices = [1, 7, 31, 62];
        let fast_replay = replay_masked_trace_columns_v1(
            b"aggregate-strategy-leaf",
            b"aggregate-strategy-node",
            2,
            &fast_masks,
            &replay_indices,
            |column| Ok(columns[column].clone()),
        )
        .expect("fast replay");
        let retained_replay = commit_encrypted_field_scratch_rows_v1(
            b"aggregate-strategy-leaf",
            b"aggregate-strategy-node",
            2,
            &replay_indices,
            &mut retained_scratch,
        )
        .expect("retained scratch replay");
        let scratch_replay = replay_masked_trace_columns_via_encrypted_scratch_v1(
            b"aggregate-strategy-leaf",
            b"aggregate-strategy-node",
            2,
            &scratch_masks,
            &replay_indices,
            |column| Ok(columns[column].clone()),
        )
        .expect("scratch replay");
        assert_eq!(retained_replay, fast_replay);
        assert_eq!(scratch_replay, fast_replay);
    }

    #[test]
    fn explicit_scratch_chunk_height_is_exact_and_rejects_invalid_geometry_before_source() {
        let columns = (0_u64..2)
            .map(|column| {
                (0_u64..8)
                    .map(|row| F::reduce(u128::from(column + 5) * u128::from(row + 23)))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let indices = [0, 7, 31, 63];
        let mut expected_rng = StdRng::from_seed([0xB6; 32]);
        let (expected, expected_masks) = commit_masked_trace_columns_v1(
            b"aggregate-explicit-chunk-leaf",
            b"aggregate-explicit-chunk-node",
            1,
            3,
            6,
            columns.len(),
            7,
            &indices,
            &mut expected_rng,
            |column| Ok(columns[column].clone()),
        )
        .expect("materialized commitment");
        let mut chunked_rng = StdRng::from_seed([0xB6; 32]);
        let (chunked, chunked_masks, scratch) =
            commit_masked_trace_columns_retaining_encrypted_scratch_with_chunk_rows_v1(
                b"aggregate-explicit-chunk-leaf",
                b"aggregate-explicit-chunk-node",
                1,
                3,
                6,
                columns.len(),
                7,
                8,
                &indices,
                &mut chunked_rng,
                |column| Ok(columns[column].clone()),
            )
            .expect("eight-row scratch chunks");
        assert_eq!(chunked, expected);
        assert_eq!(scratch.chunk_rows(), 8);
        assert_eq!(scratch.chunk_count(), 8);
        for (actual, expected) in chunked_masks.masks.iter().zip(expected_masks.masks) {
            assert_eq!(actual.coefficients(), expected.coefficients());
        }

        for hostile_chunk_rows in [0, 3, 128] {
            let calls = std::cell::Cell::new(0_usize);
            let mut rng = StdRng::from_seed([0xB7; 32]);
            assert_eq!(
                commit_masked_trace_columns_retaining_encrypted_scratch_with_chunk_rows_v1(
                    b"aggregate-explicit-chunk-leaf",
                    b"aggregate-explicit-chunk-node",
                    1,
                    3,
                    6,
                    columns.len(),
                    7,
                    hostile_chunk_rows,
                    &indices,
                    &mut rng,
                    |column| {
                        calls.set(calls.get() + 1);
                        Ok(columns[column].clone())
                    },
                )
                .map(|_| ()),
                Err(AggregateStarkErrorV1::InvalidLayout)
            );
            assert_eq!(calls.get(), 0);
        }
    }

    #[test]
    fn retaining_scratch_commitment_rejects_every_shape_before_calling_source() {
        fn reject_before_source(
            leaf_domain: &[u8],
            node_domain: &'static [u8],
            native_log2: u8,
            lde_log2: u8,
            width: usize,
            mask_degree: usize,
            indices: &[usize],
        ) {
            let calls = std::cell::Cell::new(0_usize);
            let mut rng = StdRng::from_seed([0xA4; 32]);
            assert!(
                commit_masked_trace_columns_retaining_encrypted_scratch_v1(
                    leaf_domain,
                    node_domain,
                    0,
                    native_log2,
                    lde_log2,
                    width,
                    mask_degree,
                    indices,
                    &mut rng,
                    |_| {
                        calls.set(calls.get() + 1);
                        Ok(vec![F::ZERO; 8])
                    },
                )
                .is_err()
            );
            assert_eq!(
                calls.get(),
                0,
                "invalid retained-scratch shape must reject before source work"
            );
        }

        reject_before_source(b"", b"node", 3, 6, 2, 7, &[]);
        reject_before_source(b"leaf", b"", 3, 6, 2, 7, &[]);
        reject_before_source(b"leaf", b"node", 3, 3, 2, 7, &[]);
        reject_before_source(b"leaf", b"node", 3, 6, 0, 7, &[]);
        reject_before_source(b"leaf", b"node", 3, 6, 2, 56, &[]);
        reject_before_source(b"leaf", b"node", 3, 6, 2, 7, &[1, 1]);
        reject_before_source(b"leaf", b"node", 3, 6, 2, 7, &[2, 1]);
        reject_before_source(b"leaf", b"node", 3, 6, 2, 7, &[64]);

        let calls = std::cell::Cell::new(0_usize);
        let mut rng = StdRng::from_seed([0xA5; 32]);
        assert!(
            commit_masked_trace_columns_retaining_encrypted_scratch_v1(
                b"leaf",
                b"node",
                0,
                3,
                6,
                2,
                7,
                &[],
                &mut rng,
                |_| {
                    calls.set(calls.get() + 1);
                    Ok(vec![F::ZERO; 7])
                },
            )
            .is_err()
        );
        assert_eq!(calls.get(), 1, "first malformed native column must stop");
    }

    #[test]
    fn replayable_masked_column_commitment_rejects_shape_substitution() {
        let columns = [vec![F::ZERO; 8], vec![F::ONE; 8]];
        let mut rng = StdRng::from_seed([0x24; 32]);
        assert!(
            commit_masked_trace_columns_v1(
                b"aggregate-streaming-masked-leaf",
                b"aggregate-streaming-masked-node",
                0,
                3,
                6,
                columns.len(),
                7,
                &[],
                &mut rng,
                |column| Ok(columns[column][..7].to_vec()),
            )
            .is_err()
        );
        let (_, masks) = commit_masked_trace_columns_v1(
            b"aggregate-streaming-masked-leaf",
            b"aggregate-streaming-masked-node",
            0,
            3,
            6,
            columns.len(),
            7,
            &[],
            &mut rng,
            |column| Ok(columns[column].clone()),
        )
        .expect("commitment");
        assert_eq!(
            replay_masked_trace_columns_v1(
                b"aggregate-streaming-masked-leaf",
                b"aggregate-streaming-masked-node",
                0,
                &masks,
                &[2, 1],
                |column| Ok(columns[column].clone()),
            ),
            Err(AggregateStarkErrorV1::InvalidLayout)
        );
        assert!(
            replay_masked_trace_columns_v1(
                b"aggregate-streaming-masked-leaf",
                b"aggregate-streaming-masked-node",
                0,
                &masks,
                &[1],
                |_column| Ok(vec![F::ZERO; 7]),
            )
            .is_err()
        );
    }

    #[test]
    fn retained_masked_polynomials_are_exact_commitment_coset_and_deep_replays() {
        let native_columns = (0_u64..3)
            .map(|column| {
                (0_u64..8)
                    .map(|row| F::reduce(u128::from(column + 11) * u128::from(row + 17) + 5))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let commitment_indices = [1, 9, 31];
        let mut replayable_rng = StdRng::from_seed([0xD1; 32]);
        let (replayable_commitment, masks) = commit_masked_trace_columns_v1(
            b"aggregate-retained-polynomial-leaf",
            b"aggregate-retained-polynomial-node",
            2,
            3,
            6,
            native_columns.len(),
            3,
            &commitment_indices,
            &mut replayable_rng,
            |column| Ok(native_columns[column].clone()),
        )
        .expect("replayable-mask commitment");
        let mut retained_rng = StdRng::from_seed([0xD1; 32]);
        let (retained_commitment, polynomials) = commit_masked_trace_polynomial_columns_v1(
            b"aggregate-retained-polynomial-leaf",
            b"aggregate-retained-polynomial-node",
            2,
            3,
            6,
            native_columns.len(),
            3,
            &commitment_indices,
            &mut retained_rng,
            |column| Ok(native_columns[column].clone()),
        )
        .expect("retained-polynomial commitment");
        assert_eq!(retained_commitment, replayable_commitment);
        assert_eq!(polynomials.width(), native_columns.len());
        assert_eq!(polynomials.native_trace_log2(), 3);
        assert_eq!(polynomials.commitment_lde_log2(), 6);

        for (column, mask) in masks.masks.iter().enumerate() {
            let expected = masked_trace_coefficients_with_mask_v1(
                &native_columns[column],
                3,
                mask.coefficients(),
            )
            .expect("reference coefficients");
            assert_eq!(
                polynomials
                    .column_coefficients_v1(column)
                    .expect("retained coefficients"),
                expected
            );
        }

        let replay_indices = [0, 7, 17, 63];
        let replayable = replay_masked_trace_columns_v1(
            b"aggregate-retained-polynomial-leaf",
            b"aggregate-retained-polynomial-node",
            2,
            &masks,
            &replay_indices,
            |column| Ok(native_columns[column].clone()),
        )
        .expect("mask replay");
        let retained = replay_masked_trace_polynomial_columns_v1(
            b"aggregate-retained-polynomial-leaf",
            b"aggregate-retained-polynomial-node",
            2,
            &polynomials,
            &replay_indices,
        )
        .expect("coefficient replay");
        assert_eq!(retained, replayable);
        assert_eq!(
            replay_masked_trace_polynomial_columns_v1(
                b"aggregate-retained-polynomial-leaf",
                b"aggregate-retained-polynomial-node",
                2,
                &polynomials,
                &[2, 1],
            ),
            Err(AggregateStarkErrorV1::InvalidLayout)
        );

        let reduced_coset = polynomials
            .evaluate_columns_on_coset_v1(4)
            .expect("reduced quotient coset");
        for (column, mask) in masks.masks.iter().enumerate() {
            let expected = masked_trace_lde_column_with_mask_v1(
                &native_columns[column],
                3,
                4,
                mask.coefficients(),
            )
            .expect("reference reduced coset");
            assert_eq!(&*reduced_coset[column], &expected);
        }

        let point = E::canonical([7, 1, 2, 3]).expect("canonical extension point");
        let replayable_deep = evaluate_masked_native_columns_at_deep_v1(&masks, point, |column| {
            Ok(native_columns[column].clone())
        })
        .expect("mask DEEP values");
        let retained_deep =
            evaluate_masked_trace_polynomial_columns_at_deep_v1(&polynomials, point)
                .expect("retained DEEP values");
        assert_eq!(retained_deep, replayable_deep);
        assert_eq!(
            evaluate_masked_trace_polynomial_columns_at_deep_v1(&polynomials, E::ONE),
            Err(AggregateStarkErrorV1::DeepOpening)
        );
    }

    #[test]
    fn retained_polynomial_shape_and_secret_zeroizers_reject_mutation() {
        let columns = [vec![F::ONE; 8], vec![F(2); 8]];
        let mut rng = StdRng::from_seed([0xD4; 32]);
        let (_, mut polynomials) = commit_masked_trace_polynomial_columns_v1(
            b"aggregate-retained-mutation-leaf",
            b"aggregate-retained-mutation-node",
            0,
            3,
            6,
            columns.len(),
            3,
            &[],
            &mut rng,
            |column| Ok(columns[column].clone()),
        )
        .expect("retained polynomial set");
        polynomials.columns[1].0.pop();
        assert_eq!(
            replay_masked_trace_polynomial_columns_v1(
                b"aggregate-retained-mutation-leaf",
                b"aggregate-retained-mutation-node",
                0,
                &polynomials,
                &[],
            ),
            Err(AggregateStarkErrorV1::InvalidLayout)
        );

        let mut base_values = vec![F(9), F(17), F(33)];
        zeroize_field_column_v1(&mut base_values);
        assert!(base_values.iter().all(|value| *value == F::ZERO));
        let mut extension_values = vec![
            E::canonical([1, 2, 3, 4]).expect("extension"),
            E::canonical([5, 6, 7, 8]).expect("extension"),
        ];
        zeroize_extension_field_column_v1(&mut extension_values);
        assert!(extension_values.iter().all(|value| *value == E::ZERO));
    }

    #[test]
    fn reduced_quotient_coset_chunks_match_common_domain_and_exact_division() {
        let layout = layout();
        let quotient_log2 = 5;
        let quotient_size = 1_usize << quotient_log2;
        let quotient_root = goldilocks_primitive_root_v1(quotient_log2).expect("quotient root");
        let common_root =
            goldilocks_primitive_root_v1(layout.common_lde_log2).expect("common root");
        let quotient_coefficients = (0_u64..20)
            .map(|index| {
                E::canonical([index + 1, index * 3 + 2, index * 5 + 3, index * 7 + 4])
                    .expect("canonical quotient coefficient")
            })
            .collect::<Vec<_>>();
        let quotient_evaluations = goldilocks_fp4_evaluate_coset_v1(
            &quotient_coefficients,
            quotient_size,
            quotient_root,
            F(GOLDILOCKS_GENERATOR_V1),
        )
        .expect("minimal quotient coset");
        let common_evaluations = goldilocks_fp4_evaluate_coset_v1(
            &quotient_coefficients,
            layout.common_lde_size(),
            common_root,
            F(GOLDILOCKS_GENERATOR_V1),
        )
        .expect("common quotient coset");
        let expected = split_composition_evaluations_v1(&common_evaluations, PARAMETERS, &layout)
            .expect("legacy common-domain chunks");
        let reduced = composition_chunks_from_quotient_coset_v1(
            &quotient_evaluations,
            quotient_log2,
            quotient_coefficients.len() - 1,
            PARAMETERS,
            &layout,
        )
        .expect("reduced quotient chunks");
        assert_eq!(reduced, expected);

        let trace_log2 = 3;
        let trace_size = 1_usize << trace_log2;
        let mut numerator_coefficients = vec![E::ZERO; quotient_coefficients.len() + trace_size];
        for (degree, coefficient) in quotient_coefficients.iter().copied().enumerate() {
            numerator_coefficients[degree] = numerator_coefficients[degree].sub(coefficient);
            numerator_coefficients[degree + trace_size] =
                numerator_coefficients[degree + trace_size].add(coefficient);
        }
        assert_eq!(
            divide_extension_polynomial_by_trace_vanishing_v1(&numerator_coefficients, trace_log2,)
                .expect("exact polynomial division"),
            quotient_coefficients
        );
        let numerator_evaluations = goldilocks_fp4_evaluate_coset_v1(
            &numerator_coefficients,
            quotient_size,
            quotient_root,
            F(GOLDILOCKS_GENERATOR_V1),
        )
        .expect("constraint numerator coset");
        assert_eq!(
            quotient_evaluations_from_constraint_coset_v1(
                &numerator_evaluations,
                trace_log2,
                quotient_log2,
            )
            .expect("pointwise quotient division"),
            quotient_evaluations
        );
        assert_eq!(
            composition_chunks_from_constraint_coset_v1(
                &numerator_evaluations,
                trace_log2,
                quotient_log2,
                quotient_coefficients.len() - 1,
                PARAMETERS,
                &layout,
            )
            .expect("constraint-to-composition chunks"),
            expected
        );
    }

    #[test]
    fn quotient_coset_rejects_tail_remainder_shape_and_adversarial_mutations() {
        let layout = layout();
        let quotient_log2 = 5;
        let quotient_size = 1_usize << quotient_log2;
        let quotient_root = goldilocks_primitive_root_v1(quotient_log2).expect("quotient root");
        let mut coefficients = (0_u64..20)
            .map(|index| E::from_base(F(index + 1)))
            .collect::<Vec<_>>();
        let evaluations = goldilocks_fp4_evaluate_coset_v1(
            &coefficients,
            quotient_size,
            quotient_root,
            F(GOLDILOCKS_GENERATOR_V1),
        )
        .expect("quotient evaluations");

        coefficients.resize(quotient_size, E::ZERO);
        coefficients[20] = E::ONE;
        let forbidden_tail = goldilocks_fp4_evaluate_coset_v1(
            &coefficients,
            quotient_size,
            quotient_root,
            F(GOLDILOCKS_GENERATOR_V1),
        )
        .expect("forbidden-tail evaluations");
        assert_eq!(
            composition_chunks_from_quotient_coset_v1(
                &forbidden_tail,
                quotient_log2,
                19,
                PARAMETERS,
                &layout,
            ),
            Err(AggregateStarkErrorV1::FriDegree)
        );

        let mut single_evaluation_mutation = evaluations.clone();
        single_evaluation_mutation[7] = single_evaluation_mutation[7].add(E::ONE);
        assert_eq!(
            composition_chunks_from_quotient_coset_v1(
                &single_evaluation_mutation,
                quotient_log2,
                19,
                PARAMETERS,
                &layout,
            ),
            Err(AggregateStarkErrorV1::FriDegree)
        );
        assert_eq!(
            composition_chunks_from_quotient_coset_v1(
                &evaluations[..quotient_size - 1],
                quotient_log2,
                19,
                PARAMETERS,
                &layout,
            ),
            Err(AggregateStarkErrorV1::InvalidLayout)
        );
        assert_eq!(
            composition_chunks_from_quotient_coset_v1(
                &evaluations,
                quotient_log2,
                quotient_size,
                PARAMETERS,
                &layout,
            ),
            Err(AggregateStarkErrorV1::InvalidLayout)
        );
        assert_eq!(
            composition_chunks_from_quotient_coset_v1(
                &vec![E::ZERO; 128],
                7,
                layout.maximum_composition_degree(PARAMETERS).expect("cap") + 1,
                PARAMETERS,
                &layout,
            ),
            Err(AggregateStarkErrorV1::InvalidLayout)
        );
        assert_eq!(
            quotient_evaluations_from_constraint_coset_v1(
                &evaluations,
                quotient_log2,
                quotient_log2,
            ),
            Err(AggregateStarkErrorV1::InvalidLayout)
        );
        assert_eq!(
            quotient_evaluations_from_constraint_coset_v1(
                &evaluations[..quotient_size - 1],
                3,
                quotient_log2,
            ),
            Err(AggregateStarkErrorV1::InvalidLayout)
        );

        let trace_size = 8;
        let quotient_coefficients = coefficients[..20].to_vec();
        let mut numerator = vec![E::ZERO; quotient_coefficients.len() + trace_size];
        for (degree, coefficient) in quotient_coefficients.iter().copied().enumerate() {
            numerator[degree] = numerator[degree].sub(coefficient);
            numerator[degree + trace_size] = numerator[degree + trace_size].add(coefficient);
        }
        numerator[0] = numerator[0].add(E::ONE);
        assert_eq!(
            divide_extension_polynomial_by_trace_vanishing_v1(&numerator, 3),
            Err(AggregateStarkErrorV1::ConstraintOpening)
        );
        assert_eq!(
            divide_extension_polynomial_by_trace_vanishing_v1(&[E::ZERO; 8], 3),
            Err(AggregateStarkErrorV1::InvalidLayout)
        );
    }

    #[test]
    fn streaming_composition_and_fri_match_materialized_commitments_and_openings() {
        let layout = layout();
        let rows = layout.common_lde_size();
        let root =
            goldilocks_primitive_root_v1(layout.common_lde_log2).expect("common-domain root");
        let mut point = F(GOLDILOCKS_GENERATOR_V1);
        let values = (0..rows)
            .map(|_| {
                let value = F(5).add(F(3).mul(point)).add(F(2).mul(point.mul(point)));
                point = point.mul(root);
                E::from_base(value)
            })
            .collect::<Vec<_>>();
        let indices = vec![1, 7];

        let composition_chunks = vec![values.clone()];
        let composition_tree =
            composition_tree_v1(DOMAINS, 0, &composition_chunks).expect("composition tree");
        let streamed_composition =
            streaming_composition_commitment_v1(DOMAINS, 0, &composition_chunks, &indices)
                .expect("streaming composition");
        assert_eq!(streamed_composition.root, composition_tree.root());
        assert_eq!(
            streamed_composition.frontier,
            canonical_multiproof_frontier_v1(&composition_tree, rows, &indices)
                .expect("composition frontier")
        );

        let mut materialized_transcript = transcript();
        let materialized = build_fri_lane_v1(
            PARAMETERS,
            DOMAINS,
            &layout,
            0,
            values.clone(),
            &mut materialized_transcript,
        )
        .expect("materialized FRI");
        let mut streaming_transcript = transcript();
        let streamed = build_streaming_fri_lane_v1(
            PARAMETERS,
            DOMAINS,
            &layout,
            0,
            values.clone(),
            &mut streaming_transcript,
        )
        .expect("streaming FRI");
        assert_eq!(streamed.roots, materialized.roots);
        assert_eq!(streamed.terminal_values, materialized.terminal_values);
        assert_eq!(
            streaming_transcript.state(),
            materialized_transcript.state()
        );

        let openings = open_streaming_fri_lane_v1(
            PARAMETERS, DOMAINS, &layout, 0, values, &streamed, &indices,
        )
        .expect("streaming FRI openings");
        let rounds = layout.fri_rounds(PARAMETERS).expect("FRI rounds");
        let mut layer_indices = indices.clone();
        for round in 0..rounds {
            let half = materialized.layers[round].len() / 2;
            let opening_indices = layer_indices
                .iter()
                .flat_map(|index| {
                    let low = *index % half;
                    [low, low + half]
                })
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>();
            assert_eq!(
                openings.round_frontiers[round],
                canonical_multiproof_frontier_v1(
                    &materialized.trees[round],
                    materialized.layers[round].len(),
                    &opening_indices,
                )
                .expect("materialized FRI frontier")
            );
            for (position, index) in layer_indices.iter_mut().enumerate() {
                let low = *index % half;
                assert_eq!(
                    openings.queries[position].rounds[round],
                    AggregateFriRoundOpeningV1 {
                        low: materialized.layers[round][low].coefficients().map(F::value),
                        high: materialized.layers[round][low + half]
                            .coefficients()
                            .map(F::value),
                    }
                );
                *index = low;
            }
        }
    }

    #[test]
    fn streaming_fri_replay_rejects_material_and_query_substitution() {
        let layout = layout();
        let root =
            goldilocks_primitive_root_v1(layout.common_lde_log2).expect("common-domain root");
        let mut point = F(GOLDILOCKS_GENERATOR_V1);
        let values = (0..layout.common_lde_size())
            .map(|_| {
                let value = F(5).add(F(3).mul(point)).add(F(2).mul(point.mul(point)));
                point = point.mul(root);
                E::from_base(value)
            })
            .collect::<Vec<_>>();
        let mut prover_transcript = transcript();
        let material = build_streaming_fri_lane_v1(
            PARAMETERS,
            DOMAINS,
            &layout,
            0,
            values.clone(),
            &mut prover_transcript,
        )
        .expect("streaming FRI");
        let indices = [1, 7];

        let mut changed = material.clone();
        changed.roots[0][0] ^= 1;
        assert!(
            open_streaming_fri_lane_v1(
                PARAMETERS,
                DOMAINS,
                &layout,
                0,
                values.clone(),
                &changed,
                &indices,
            )
            .is_err()
        );
        changed = material.clone();
        changed.betas[0] = changed.betas[0].add(E::ONE);
        assert!(
            open_streaming_fri_lane_v1(
                PARAMETERS,
                DOMAINS,
                &layout,
                0,
                values.clone(),
                &changed,
                &indices,
            )
            .is_err()
        );
        changed = material.clone();
        changed.terminal_values[0] = E::ONE;
        assert!(
            open_streaming_fri_lane_v1(
                PARAMETERS,
                DOMAINS,
                &layout,
                0,
                values.clone(),
                &changed,
                &indices,
            )
            .is_err()
        );
        for hostile in [vec![1], vec![1, 1], vec![1, layout.common_lde_size()]] {
            assert!(
                open_streaming_fri_lane_v1(
                    PARAMETERS,
                    DOMAINS,
                    &layout,
                    0,
                    values.clone(),
                    &material,
                    &hostile,
                )
                .is_err()
            );
        }
    }

    #[test]
    fn exact_codec_multiproofs_fri_and_callback_roundtrip() {
        let (layout, proof, _, _) = fixture();
        let encoded = encode_proof_v1(&proof, PARAMETERS, &layout).expect("encode");
        assert_eq!(
            encoded.len(),
            exact_encoded_proof_bytes_v1(&proof, PARAMETERS, &layout).expect("exact size")
        );
        assert!(
            encoded.len()
                <= maximum_encoded_proof_bytes_v1(PARAMETERS, &layout).expect("maximum size")
        );
        let decoded = decode_proof_v1(&encoded, PARAMETERS, &layout).expect("decode");
        assert_eq!(decoded, proof);
        verify_all_merkle_openings_v1(&decoded, PARAMETERS, DOMAINS, &layout, &[1, 7])
            .expect("multiproofs");

        let mut verifier_transcript = transcript();
        absorb_layout_v1(
            &mut verifier_transcript,
            PARAMETERS,
            DOMAINS,
            b"aggregate-test-relation-layout",
            &layout,
        )
        .expect("layout");
        absorb_base_roots_v1(&mut verifier_transcript, DOMAINS, &decoded.trace_groups)
            .expect("base");
        absorb_aux_roots_v1(&mut verifier_transcript, DOMAINS, &decoded.trace_groups).expect("aux");
        absorb_composition_roots_v1(
            &mut verifier_transcript,
            PARAMETERS,
            DOMAINS,
            &decoded.composition_roots,
        )
        .expect("composition");
        absorb_fri_mask_roots_v1(
            &mut verifier_transcript,
            PARAMETERS,
            &decoded.fri_mask_roots,
        )
        .expect("FRI masks");
        let (betas, terminals) = verify_fri_commitments_v1(
            &decoded,
            PARAMETERS,
            DOMAINS,
            &layout,
            &mut verifier_transcript,
        )
        .expect("FRI commitments");
        verify_opened_query_relations_v1(
            &decoded,
            PARAMETERS,
            &layout,
            &[1, 7],
            &betas,
            &terminals,
            &mut ZeroEvaluator,
        )
        .expect("callback");

        for length in [0, 1, encoded.len() / 2, encoded.len() - 1] {
            assert!(decode_proof_v1(&encoded[..length], PARAMETERS, &layout).is_err());
        }
        let mut trailing = encoded;
        trailing.push(0);
        assert!(decode_proof_v1(&trailing, PARAMETERS, &layout).is_err());
    }

    #[test]
    fn exact_codec_rejects_noncanonical_base_and_every_extension_opening_class() {
        let (layout, proof, _, _) = fixture();
        let noncanonical = crate::privacy_engines::transparent_stark::GOLDILOCKS_MODULUS_V1;

        let mut changed = proof.clone();
        changed.queries[0].trace_groups[0].base_current[0] = noncanonical;
        assert_eq!(
            encode_proof_v1(&changed, PARAMETERS, &layout),
            Err(AggregateStarkErrorV1::NonCanonicalField)
        );

        changed = proof.clone();
        changed.queries[0].composition_values[0][0][1] = noncanonical;
        assert_eq!(
            encode_proof_v1(&changed, PARAMETERS, &layout),
            Err(AggregateStarkErrorV1::NonCanonicalField)
        );

        changed = proof.clone();
        changed.queries[0].fri_mask_values[0][2] = noncanonical;
        assert_eq!(
            encode_proof_v1(&changed, PARAMETERS, &layout),
            Err(AggregateStarkErrorV1::NonCanonicalField)
        );

        changed = proof.clone();
        changed.queries[0].fri_lanes[0].rounds[0].low[3] = noncanonical;
        assert_eq!(
            encode_proof_v1(&changed, PARAMETERS, &layout),
            Err(AggregateStarkErrorV1::NonCanonicalField)
        );

        changed = proof;
        changed.fri_lanes[0].terminal_values[0][0] = noncanonical;
        assert_eq!(
            encode_proof_v1(&changed, PARAMETERS, &layout),
            Err(AggregateStarkErrorV1::NonCanonicalField)
        );
    }

    #[test]
    fn layout_domains_and_frontiers_are_fail_closed() {
        let mut groups = vec![
            AggregateTraceGroupLayoutV1 {
                native_trace_log2: 3,
                segment_instances: 1,
                base_width: 1,
                aux_width: 1,
            },
            AggregateTraceGroupLayoutV1 {
                native_trace_log2: 4,
                segment_instances: 1,
                base_width: 1,
                aux_width: 1,
            },
        ];
        AggregateProofLayoutV1::new(PARAMETERS, groups.clone()).expect("ordered");
        groups.swap(0, 1);
        assert!(AggregateProofLayoutV1::new(PARAMETERS, groups.clone()).is_err());
        groups[1].native_trace_log2 = groups[0].native_trace_log2;
        AggregateProofLayoutV1::new(PARAMETERS, groups)
            .expect("independent equal-domain groups remain distinct");
        assert!(AggregateProofLayoutV1::new(PARAMETERS, Vec::new()).is_err());

        let mut domains = DOMAINS;
        domains.fri_node = domains.fri_leaf;
        assert!(domains.validate().is_err());

        let (layout, proof, _, _) = fixture();
        let mut changed = proof.clone();
        changed.trace_groups[0].base_frontier.pop();
        assert!(encode_proof_v1(&changed, PARAMETERS, &layout).is_err());
        changed = proof.clone();
        changed.trace_groups[0].base_frontier.swap(0, 1);
        let encoded = encode_proof_v1(&changed, PARAMETERS, &layout).expect("shape-valid");
        let decoded = decode_proof_v1(&encoded, PARAMETERS, &layout).expect("decode");
        assert!(
            verify_all_merkle_openings_v1(&decoded, PARAMETERS, DOMAINS, &layout, &[1, 7]).is_err()
        );
    }

    fn encrypted_scratch_fixture() -> EncryptedFieldMatrixScratchV1 {
        let rows = 16;
        let width = 3;
        let chunk_rows = 4;
        let mut writer =
            EncryptedFieldMatrixScratchWriterV1::new(rows, width, chunk_rows).expect("scratch");
        for column in 0..width {
            let values = (0..rows)
                .map(|row| F(u64::try_from(column * 100 + row + 1).expect("small")))
                .collect::<Vec<_>>();
            writer.append_column(&values).expect("append column");
        }
        writer.finish().expect("finish scratch")
    }

    #[derive(Clone, Copy)]
    enum ScratchEntropyModeV1 {
        FailAt(u8),
        Constant(u8),
        RepeatedPrefix,
        Healthy,
    }

    #[derive(Debug)]
    struct ScratchEntropyErrorV1;

    impl core::fmt::Display for ScratchEntropyErrorV1 {
        fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            formatter.write_str("injected encrypted-scratch entropy failure")
        }
    }

    struct ScratchEntropyRngV1 {
        mode: ScratchEntropyModeV1,
        fills: usize,
    }

    impl ScratchEntropyRngV1 {
        fn new(mode: ScratchEntropyModeV1) -> Self {
            Self { mode, fills: 0 }
        }
    }

    impl TryRngCore for ScratchEntropyRngV1 {
        type Error = ScratchEntropyErrorV1;

        fn try_next_u32(&mut self) -> Result<u32, Self::Error> {
            let mut bytes = [0_u8; 4];
            self.try_fill_bytes(&mut bytes)?;
            Ok(u32::from_le_bytes(bytes))
        }

        fn try_next_u64(&mut self) -> Result<u64, Self::Error> {
            let mut bytes = [0_u8; 8];
            self.try_fill_bytes(&mut bytes)?;
            Ok(u64::from_le_bytes(bytes))
        }

        fn try_fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), Self::Error> {
            let fill = self.fills;
            self.fills = self.fills.saturating_add(1);
            match self.mode {
                ScratchEntropyModeV1::FailAt(target) if fill == usize::from(target) => {
                    let partial = destination.len() / 2;
                    destination
                        .iter_mut()
                        .take(partial)
                        .enumerate()
                        .for_each(|(index, byte)| *byte = index as u8);
                    Err(ScratchEntropyErrorV1)
                }
                ScratchEntropyModeV1::FailAt(_) | ScratchEntropyModeV1::Healthy => {
                    for (index, byte) in destination.iter_mut().enumerate() {
                        *byte = (fill as u8)
                            .wrapping_mul(97)
                            .wrapping_add(index as u8)
                            .wrapping_add(1);
                    }
                    Ok(())
                }
                ScratchEntropyModeV1::Constant(byte) => {
                    destination.fill(byte);
                    Ok(())
                }
                ScratchEntropyModeV1::RepeatedPrefix => {
                    for (index, byte) in destination.iter_mut().enumerate() {
                        *byte = index as u8 + 1;
                    }
                    Ok(())
                }
            }
        }
    }

    #[test]
    fn encrypted_field_scratch_entropy_is_injected_healthy_and_precedes_file_creation() {
        for mode in [
            ScratchEntropyModeV1::FailAt(0),
            ScratchEntropyModeV1::FailAt(1),
            ScratchEntropyModeV1::Constant(0),
            ScratchEntropyModeV1::Constant(0xa5),
            ScratchEntropyModeV1::RepeatedPrefix,
        ] {
            let attempts = encrypted_scratch_file_creation_attempts_v1();
            let mut rng = ScratchEntropyRngV1::new(mode);
            assert!(matches!(
                EncryptedFieldMatrixScratchWriterV1::new_with_rng(16, 2, 4, &mut rng),
                Err(AggregateStarkErrorV1::RandomnessUnavailable)
            ));
            assert_eq!(
                encrypted_scratch_file_creation_attempts_v1(),
                attempts,
                "no anonymous file may be created before entropy validation"
            );
        }

        let attempts = encrypted_scratch_file_creation_attempts_v1();
        let mut healthy = ScratchEntropyRngV1::new(ScratchEntropyModeV1::Healthy);
        let writer = EncryptedFieldMatrixScratchWriterV1::new_with_rng(16, 2, 4, &mut healthy)
            .expect("healthy injected entropy");
        assert_eq!(encrypted_scratch_file_creation_attempts_v1(), attempts + 1);
        drop(writer);

        let attempts = encrypted_scratch_file_creation_attempts_v1();
        let mut healthy = ScratchEntropyRngV1::new(ScratchEntropyModeV1::Healthy);
        assert!(matches!(
            EncryptedFieldMatrixScratchWriterV1::new_with_rng(12, 2, 4, &mut healthy),
            Err(AggregateStarkErrorV1::InvalidLayout)
        ));
        assert_eq!(healthy.fills, 0, "invalid shape must not consume entropy");
        assert_eq!(encrypted_scratch_file_creation_attempts_v1(), attempts);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn encrypted_field_scratch_memfd_is_anonymous_owner_private_and_exec_sealed() {
        use rustix::fs::{Mode, fchmod};

        let file = create_anonymous_scratch_file_v1().expect("Linux 6.3+ sealed scratch memfd");
        let metadata = file.metadata().expect("scratch metadata");
        assert!(metadata.file_type().is_file());
        assert_eq!(metadata.nlink(), 0);
        assert_eq!(metadata.mode() & 0o777, 0o600);
        assert_eq!(
            fcntl_get_seals(&file).expect("scratch seals") & SealFlags::EXEC,
            SealFlags::EXEC
        );
        assert_eq!(
            fchmod(&file, Mode::RWXU).expect_err("F_SEAL_EXEC must reject execute permission"),
            rustix::io::Errno::PERM
        );
        assert_eq!(
            file.metadata()
                .expect("scratch metadata after rejection")
                .mode()
                & 0o777,
            0o600
        );
    }

    #[test]
    fn encrypted_field_scratch_roundtrips_exact_row_chunks_without_plaintext_storage() {
        let mut scratch = encrypted_scratch_fixture();
        assert_eq!(scratch.rows(), 16);
        assert_eq!(scratch.width(), 3);
        assert_eq!(scratch.chunk_rows(), 4);
        assert_eq!(scratch.chunk_count(), 4);
        assert_eq!(
            scratch.ciphertext_bytes(),
            u64::try_from(3 * 4 * (4 * 8 + XCHACHA20_POLY1305_TAG_BYTES_V1)).expect("small")
        );

        scratch
            .file
            .seek(std::io::SeekFrom::Start(0))
            .expect("seek ciphertext");
        let mut ciphertext = Vec::new();
        scratch
            .file
            .read_to_end(&mut ciphertext)
            .expect("read ciphertext");
        let first_plaintext_chunk = (1_u64..=4).flat_map(u64::to_be_bytes).collect::<Vec<_>>();
        assert!(
            !ciphertext
                .windows(first_plaintext_chunk.len())
                .any(|window| window == first_plaintext_chunk)
        );

        for chunk in 0..scratch.chunk_count() {
            let block = scratch.read_chunk(chunk).expect("authenticated block");
            assert_eq!(block.row_start(), chunk * 4);
            assert_eq!(block.row_count(), 4);
            for row in block.row_start()..block.row_start() + block.row_count() {
                assert_eq!(
                    block.row(row).expect("row"),
                    &[
                        F(u64::try_from(row + 1).expect("small")),
                        F(u64::try_from(100 + row + 1).expect("small")),
                        F(u64::try_from(200 + row + 1).expect("small")),
                    ]
                );
            }
            assert!(block.row(block.row_start() + block.row_count()).is_err());
        }
        assert!(scratch.read_chunk(scratch.chunk_count()).is_err());
    }

    #[test]
    fn encrypted_field_scratch_rejects_shape_and_incomplete_writes() {
        assert!(EncryptedFieldMatrixScratchWriterV1::new(0, 1, 1).is_err());
        assert!(EncryptedFieldMatrixScratchWriterV1::new(12, 1, 4).is_err());
        assert!(EncryptedFieldMatrixScratchWriterV1::new(16, 0, 4).is_err());
        assert!(EncryptedFieldMatrixScratchWriterV1::new(16, 1, 3).is_err());
        assert!(EncryptedFieldMatrixScratchWriterV1::new(16, 1, 32).is_err());

        let mut writer = EncryptedFieldMatrixScratchWriterV1::new(16, 2, 4).expect("valid writer");
        assert!(writer.append_column(&[F::ZERO; 15]).is_err());
        writer
            .append_column(&[F::ONE; 16])
            .expect("first exact column");
        assert!(writer.finish().is_err());

        let mut writer = EncryptedFieldMatrixScratchWriterV1::new(16, 1, 4).expect("valid writer");
        writer
            .append_column(&[F::ONE; 16])
            .expect("only exact column");
        assert!(writer.append_column(&[F::ONE; 16]).is_err());
        writer.finish().expect("complete writer");
    }

    #[test]
    fn encrypted_field_scratch_authenticates_bytes_key_nonce_order_and_length() {
        let mut mutated = encrypted_scratch_fixture();
        mutated
            .file
            .seek(std::io::SeekFrom::Start(0))
            .expect("seek");
        let mut byte = [0_u8; 1];
        mutated.file.read_exact(&mut byte).expect("read");
        byte[0] ^= 1;
        mutated
            .file
            .seek(std::io::SeekFrom::Start(0))
            .expect("seek");
        mutated.file.write_all(&byte).expect("mutate");
        assert!(mutated.read_chunk(0).is_err());

        let mut wrong_key = encrypted_scratch_fixture();
        wrong_key.key[0] ^= 1;
        assert!(wrong_key.read_chunk(0).is_err());

        let mut wrong_nonce = encrypted_scratch_fixture();
        wrong_nonce.nonce_prefix[0] ^= 1;
        assert!(wrong_nonce.read_chunk(0).is_err());

        let mut reordered = encrypted_scratch_fixture();
        let record_bytes = reordered.ciphertext_chunk_bytes;
        let mut first = vec![0_u8; record_bytes];
        let mut second = vec![0_u8; record_bytes];
        reordered
            .file
            .seek(std::io::SeekFrom::Start(0))
            .expect("seek");
        reordered.file.read_exact(&mut first).expect("first");
        reordered.file.read_exact(&mut second).expect("second");
        reordered
            .file
            .seek(std::io::SeekFrom::Start(0))
            .expect("seek");
        reordered.file.write_all(&second).expect("swap first");
        reordered.file.write_all(&first).expect("swap second");
        assert!(reordered.read_chunk(0).is_err());

        let mut duplicated = encrypted_scratch_fixture();
        let mut first = vec![0_u8; duplicated.ciphertext_chunk_bytes];
        duplicated
            .file
            .seek(std::io::SeekFrom::Start(0))
            .expect("seek");
        duplicated.file.read_exact(&mut first).expect("first");
        duplicated
            .file
            .seek(std::io::SeekFrom::Start(
                u64::try_from(duplicated.ciphertext_chunk_bytes).expect("small"),
            ))
            .expect("seek second");
        duplicated.file.write_all(&first).expect("duplicate");
        assert!(duplicated.read_chunk(1).is_err());

        let mut truncated = encrypted_scratch_fixture();
        truncated
            .file
            .set_len(truncated.expected_file_bytes - 1)
            .expect("truncate");
        assert!(truncated.read_chunk(0).is_err());

        let mut extended = encrypted_scratch_fixture();
        extended
            .file
            .set_len(extended.expected_file_bytes + 1)
            .expect("extend");
        assert!(extended.read_chunk(0).is_err());
    }

    #[test]
    fn encrypted_field_scratch_rejects_authenticated_noncanonical_fields() {
        let mut scratch = encrypted_scratch_fixture();
        let record_index = 0_u64;
        let nonce = encrypted_field_scratch_nonce_v1(&scratch.nonce_prefix, record_index);
        let aad = encrypted_field_scratch_record_aad_v1(
            scratch.rows,
            scratch.width,
            scratch.chunk_rows,
            record_index,
        )
        .expect("aad");
        let mut plaintext = Zeroizing::new(Vec::new());
        for _ in 0..scratch.chunk_rows {
            plaintext.extend_from_slice(&u64::MAX.to_be_bytes());
        }
        let cipher =
            XChaCha20Poly1305::new_from_slice(scratch.key.as_ref()).expect("fixed key length");
        let ciphertext = cipher
            .encrypt(
                &nonce,
                Payload {
                    msg: plaintext.as_slice(),
                    aad: &aad,
                },
            )
            .expect("test encryption");
        assert_eq!(ciphertext.len(), scratch.ciphertext_chunk_bytes);
        scratch
            .file
            .seek(std::io::SeekFrom::Start(0))
            .expect("seek");
        scratch.file.write_all(&ciphertext).expect("replace record");
        assert!(scratch.read_chunk(0).is_err());
    }

    #[test]
    fn replayed_masked_trace_spill_matches_exact_masked_lde_rows() {
        let native_log2 = 3;
        let lde_log2 = 6;
        let native_columns = [
            (0..1_usize << native_log2)
                .map(|index| F(u64::try_from(index + 1).expect("small")))
                .collect::<Vec<_>>(),
            (0..1_usize << native_log2)
                .map(|index| F(u64::try_from(index * 7 + 3).expect("small")))
                .collect::<Vec<_>>(),
        ];
        let mut rng = StdRng::seed_from_u64(0x5C12_A7C4);
        let (_, masks) = commit_masked_trace_columns_v1(
            DOMAINS.base_leaf,
            DOMAINS.base_node,
            0,
            native_log2,
            lde_log2,
            native_columns.len(),
            7,
            &[],
            &mut rng,
            |column| Ok(native_columns[column].clone()),
        )
        .expect("masked commitment");
        let expected = masks
            .masks
            .iter()
            .zip(&native_columns)
            .map(|(mask, native)| {
                masked_trace_lde_column_with_mask_v1(
                    native,
                    native_log2,
                    lde_log2,
                    mask.coefficients(),
                )
                .expect("masked LDE")
            })
            .collect::<Vec<_>>();
        let mut scratch = spill_replayed_masked_trace_columns_v1(&masks, |column| {
            Ok(native_columns[column].clone())
        })
        .expect("spill");
        assert_eq!(scratch.chunk_count(), 1);
        let block = scratch.read_chunk(0).expect("block");
        for row in 0..1_usize << lde_log2 {
            assert_eq!(
                block.row(row).expect("row"),
                &[expected[0][row], expected[1][row]]
            );
        }
    }
}
