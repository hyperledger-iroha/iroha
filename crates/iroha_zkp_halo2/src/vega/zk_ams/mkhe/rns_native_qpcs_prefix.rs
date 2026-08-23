//! Authenticated qPCS continuation prefix for the replacement 40-limb proof.
//!
//! The initial-tree substage hands its borrowed codeword openings to this
//! verifier.  This module then authenticates the opening-quotient tree, checks
//! every queried one-point quotient and ten-row batching equation, authenticates
//! FRI layers zero and one, and checks exactly the first FRI fold.  The remaining
//! seventeen folds, terminal degree equation, and RLWE/source relations are not
//! implemented here.  Success is therefore deliberately non-authorizing and the
//! composite boundary still reports the complete RNS/qPCS stage unavailable.

use super::{
    manifest::ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1,
    rns_native_profile::{
        ZK_AMS_MKHE_RNS_NATIVE_FRI_ROUNDS_V1, ZK_AMS_MKHE_RNS_NATIVE_LDE_DOMAIN_LOG2_V1,
        ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1, ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1,
        ZK_AMS_MKHE_RNS_NATIVE_QPCS_MAX_BYTES_V1, ZK_AMS_MKHE_RNS_NATIVE_QUERY_COUNT_V1,
    },
    rns_native_qpcs_initial::{
        RnsNativeQpcsInitialStageV1, authenticate_rns_native_qpcs_initial_v1,
    },
    rns_native_transcript::{
        ZkAmsMkheRnsNativeChallengeSeedsV1, ZkAmsMkheRnsNativeQpcsBoundTranscriptV1,
        ZkAmsMkheRnsNativeQpcsRelationBindingV1, ZkAmsMkheRnsNativeQpcsRelationLineageV1,
    },
};
use crate::vega::sponge::Keccak256;

const PREFIX_MAGIC_V1: [u8; 4] = *b"ZQPX";
const PREFIX_VERSION_V1: u8 = 1;
const REPETITIONS_V1: usize = 5;
const ROWS_PER_REPETITION_V1: usize = 2;
pub(super) const ROWS_PER_LIMB_V1: usize = REPETITIONS_V1 * ROWS_PER_REPETITION_V1;
const RELATION_COUNT_V1: usize = ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1 * REPETITIONS_V1;
const EVALUATION_COUNT_V1: usize = RELATION_COUNT_V1 * ROWS_PER_REPETITION_V1;
const EVALUATION_BYTES_V1: usize = EVALUATION_COUNT_V1 * 8;
pub(super) const COORDINATE_COUNT_V1: usize = ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1 * ROWS_PER_LIMB_V1;
pub(super) const FQ2_BYTES_V1: usize = 16;
pub(super) const DIGEST_BYTES_V1: usize = 32;
pub(super) const LEAF_BYTES_V1: usize = COORDINATE_COUNT_V1 * FQ2_BYTES_V1;
pub(super) const QUERY_COUNT_V1: usize = ZK_AMS_MKHE_RNS_NATIVE_QUERY_COUNT_V1 as usize;
pub(super) const MAX_OPENED_LEAVES_V1: usize = 2 * QUERY_COUNT_V1;
const MAX_AUTHENTICATION_HASHES_V1: usize = 3_392;
const MAX_TREE_BYTES_V1: usize =
    MAX_OPENED_LEAVES_V1 * LEAF_BYTES_V1 + MAX_AUTHENTICATION_HASHES_V1 * DIGEST_BYTES_V1;
pub(super) const DOMAIN_SIZE_V1: usize = 1 << ZK_AMS_MKHE_RNS_NATIVE_LDE_DOMAIN_LOG2_V1;
pub(super) const FRI_ONE_SIZE_V1: usize = DOMAIN_SIZE_V1 / 2;
const MAX_CHALLENGE_ATTEMPTS_V1: u16 = 256;
const TREE_COUNT_V1: usize = 3;
const CHECKED_FOLD_COUNT_V1: u8 = 1;
const TREE_DESCRIPTOR_BYTES_V1: usize = 2 + 2 + 4 + 4;
const PREFIX_HEADER_BYTES_V1: usize =
    4 + 4 + 3 * 2 + 2 + 2 * 4 + TREE_COUNT_V1 * TREE_DESCRIPTOR_BYTES_V1 + 13 * DIGEST_BYTES_V1;

const SECTION_BINDING_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-qpcs.prefix.section-binding";
const RLWE_AGGREGATION_IDENTITY_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-qpcs.prefix.rlwe-aggregation-identity";
const EVALUATION_BINDING_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-qpcs.prefix.ordered-evaluations";
const RELATION_POINT_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-qpcs.prefix.relation-point";
const BATCH_CHALLENGE_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-qpcs.prefix.ten-row-batch";
const FOLD_CHALLENGE_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.rns-native-qpcs.prefix.fri-fold-0";
const TREE_LEAF_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.rns-native-qpcs.prefix.tree-leaf";
const TREE_NODE_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.rns-native-qpcs.prefix.tree-node";
const RESIDUAL_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-qpcs.prefix.unverified-residual";

const _: () = {
    assert!(ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1 == 1 << 17);
    assert!(ZK_AMS_MKHE_RNS_NATIVE_LDE_DOMAIN_LOG2_V1 == 19);
    assert!(ZK_AMS_MKHE_RNS_NATIVE_FRI_ROUNDS_V1 == 18);
    assert!(ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1 == 40);
    assert!(QUERY_COUNT_V1 == 160);
    assert!(RELATION_COUNT_V1 == 200);
    assert!(EVALUATION_COUNT_V1 == 400);
    assert!(EVALUATION_BYTES_V1 == 3_200);
    assert!(COORDINATE_COUNT_V1 == 400);
    assert!(LEAF_BYTES_V1 == 6_400);
    assert!(MAX_TREE_BYTES_V1 == 2_156_544);
    assert!(PREFIX_HEADER_BYTES_V1 == 476);
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RnsNativeQpcsPrefixErrorV1 {
    InvalidInitial,
    InvalidContext,
    ProofCapExceeded,
    Truncated,
    TrailingBytes,
    InvalidHeader,
    InvalidCount,
    InvalidOrder,
    NonCanonicalResidue,
    InvalidRelation,
    InvalidMerklePath,
    InvalidOpeningQuotient,
    InvalidBatchEquation,
    InvalidFriEquation,
    InvalidChallenge,
    ArithmeticOverflow,
}

impl core::fmt::Display for RnsNativeQpcsPrefixErrorV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for RnsNativeQpcsPrefixErrorV1 {}

#[derive(Clone, Copy)]
struct PrefixContextV1 {
    parameter_digest: [u8; DIGEST_BYTES_V1],
    transcript_digest: [u8; DIGEST_BYTES_V1],
    q_mask_s_root: [u8; DIGEST_BYTES_V1],
    qpcs_pre_relation_transcript_digest: [u8; DIGEST_BYTES_V1],
    rns_aggregation_seed: [u8; DIGEST_BYTES_V1],
    relation_seed: [u8; DIGEST_BYTES_V1],
    batching_seed: [u8; DIGEST_BYTES_V1],
    fold_zero_seed: [u8; DIGEST_BYTES_V1],
    query_seed: [u8; DIGEST_BYTES_V1],
    quotient_root: [u8; DIGEST_BYTES_V1],
    fri_zero_root: [u8; DIGEST_BYTES_V1],
    fri_one_root: [u8; DIGEST_BYTES_V1],
    section_binding_digest: [u8; DIGEST_BYTES_V1],
    equation_commitment_digests: [[u8; DIGEST_BYTES_V1]; 2],
    limb_commitment_digests: [[u8; DIGEST_BYTES_V1]; ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1],
}

impl PrefixContextV1 {
    fn from_transcript_v1(
        transcript: &ZkAmsMkheRnsNativeChallengeSeedsV1,
        parameter_digest: [u8; DIGEST_BYTES_V1],
        equation_commitment_digests: &[[u8; DIGEST_BYTES_V1]],
        limb_commitment_digests: &[[u8; DIGEST_BYTES_V1]],
        query_opening_digests: &[[u8; DIGEST_BYTES_V1]],
    ) -> Result<Self, RnsNativeQpcsPrefixErrorV1> {
        if equation_commitment_digests.len() != 2
            || limb_commitment_digests.len() != ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1
            || query_opening_digests.len() != QUERY_COUNT_V1
        {
            return Err(RnsNativeQpcsPrefixErrorV1::InvalidCount);
        }
        let fri_roots = transcript.qpcs_fri_roots();
        if usize::from(fri_roots[0].layer()) != 0 || usize::from(fri_roots[1].layer()) != 1 {
            return Err(RnsNativeQpcsPrefixErrorV1::InvalidOrder);
        }
        let transcript_digest = transcript.transcript_digest();
        let q_mask_s_root = transcript.q_mask_s_root();
        let qpcs_pre_relation_transcript_digest = transcript.qpcs_pre_relation_transcript_digest();
        let rns_aggregation_seed = transcript.rns_aggregation_challenge_seed();
        let relation_seed = transcript.qpcs_relation_challenge_seed();
        let batching_seed = transcript.qpcs_batching_challenge_seed();
        let fold_zero_seed = transcript.qpcs_fri_fold_challenge_seeds()[0];
        let query_seed = transcript.qpcs_query_challenge_seed();
        let quotient_root = transcript.qpcs_quotient_root();
        let fri_zero_root = fri_roots[0].root();
        let fri_one_root = fri_roots[1].root();
        if [
            parameter_digest,
            transcript_digest,
            q_mask_s_root,
            qpcs_pre_relation_transcript_digest,
            rns_aggregation_seed,
            relation_seed,
            batching_seed,
            fold_zero_seed,
            query_seed,
            quotient_root,
            fri_zero_root,
            fri_one_root,
        ]
        .contains(&[0; DIGEST_BYTES_V1])
        {
            return Err(RnsNativeQpcsPrefixErrorV1::InvalidContext);
        }
        let section_binding_digest = section_binding_digest_v1(
            transcript_digest,
            equation_commitment_digests,
            limb_commitment_digests,
            query_opening_digests,
        )?;
        let equation_commitment_digests = equation_commitment_digests
            .try_into()
            .map_err(|_| RnsNativeQpcsPrefixErrorV1::InvalidCount)?;
        let limb_commitment_digests = limb_commitment_digests
            .try_into()
            .map_err(|_| RnsNativeQpcsPrefixErrorV1::InvalidCount)?;
        Ok(Self {
            parameter_digest,
            transcript_digest,
            q_mask_s_root,
            qpcs_pre_relation_transcript_digest,
            rns_aggregation_seed,
            relation_seed,
            batching_seed,
            fold_zero_seed,
            query_seed,
            quotient_root,
            fri_zero_root,
            fri_one_root,
            section_binding_digest,
            equation_commitment_digests,
            limb_commitment_digests,
        })
    }
}

/// Move-only owner of the exact 200 qPCS relation points.
///
/// The prover owner consumes the transcript's sole relation binding after the
/// initial qPCS root and authenticated q-mask `S` root, but before the quotient
/// or any FRI root exists. Legacy verification may deterministically reconstruct
/// the same public points from final seeds, but that compatibility schedule has
/// no private lineage and cannot authorize the direct handoff. qPCS borrows its
/// points and returns the lineage-bearing owner through every later private
/// stage; cross-field code must consume it rather than derive a parallel
/// challenge schedule.
#[allow(
    missing_copy_implementations,
    reason = "the qPCS/cross-field relation schedule must be threaded exactly once"
)]
pub(super) struct RnsNativeQpcsRelationScheduleV1 {
    lineage: Option<ZkAmsMkheRnsNativeQpcsRelationLineageV1>,
    parameter_digest: [u8; DIGEST_BYTES_V1],
    q_mask_s_root: [u8; DIGEST_BYTES_V1],
    qpcs_pre_relation_transcript_digest: [u8; DIGEST_BYTES_V1],
    relation_seed: [u8; DIGEST_BYTES_V1],
    points: [u64; RELATION_COUNT_V1],
}

#[allow(
    dead_code,
    reason = "some accessors are reserved for the undeclared typed qPCS/cross-field adapter"
)]
impl RnsNativeQpcsRelationScheduleV1 {
    fn from_context_v1(context: PrefixContextV1) -> Result<Self, RnsNativeQpcsPrefixErrorV1> {
        Self::from_bound_parts_v1(
            None,
            context.parameter_digest,
            context.q_mask_s_root,
            context.qpcs_pre_relation_transcript_digest,
            context.relation_seed,
        )
    }

    fn from_bound_parts_v1(
        lineage: Option<ZkAmsMkheRnsNativeQpcsRelationLineageV1>,
        parameter_digest: [u8; DIGEST_BYTES_V1],
        q_mask_s_root: [u8; DIGEST_BYTES_V1],
        qpcs_pre_relation_transcript_digest: [u8; DIGEST_BYTES_V1],
        relation_seed: [u8; DIGEST_BYTES_V1],
    ) -> Result<Self, RnsNativeQpcsPrefixErrorV1> {
        let identities = [
            parameter_digest,
            q_mask_s_root,
            qpcs_pre_relation_transcript_digest,
            relation_seed,
        ];
        if identities.contains(&[0; DIGEST_BYTES_V1])
            || identities
                .iter()
                .enumerate()
                .any(|(index, digest)| identities[index + 1..].contains(digest))
        {
            return Err(RnsNativeQpcsPrefixErrorV1::InvalidContext);
        }
        Ok(Self {
            lineage,
            parameter_digest,
            q_mask_s_root,
            qpcs_pre_relation_transcript_digest,
            relation_seed,
            points: derive_relation_points_from_seed_v1(parameter_digest, relation_seed)?,
        })
    }

    fn from_transcript_v1(
        context: PrefixContextV1,
        transcript: &ZkAmsMkheRnsNativeChallengeSeedsV1,
    ) -> Result<Self, RnsNativeQpcsPrefixErrorV1> {
        if context.transcript_digest != transcript.transcript_digest()
            || context.q_mask_s_root != transcript.q_mask_s_root()
            || context.qpcs_pre_relation_transcript_digest
                != transcript.qpcs_pre_relation_transcript_digest()
            || context.relation_seed != transcript.qpcs_relation_challenge_seed()
        {
            return Err(RnsNativeQpcsPrefixErrorV1::InvalidContext);
        }
        Self::from_context_v1(context)
    }

    /// Consume the sole relation binding issued by the move-only pre-quotient
    /// transcript stage.
    #[allow(
        dead_code,
        reason = "the private qPCS prover adapter will mint this owner before quotient roots"
    )]
    pub(super) fn from_relation_binding_v1(
        parameter_digest: [u8; DIGEST_BYTES_V1],
        binding: ZkAmsMkheRnsNativeQpcsRelationBindingV1,
    ) -> Result<Self, RnsNativeQpcsPrefixErrorV1> {
        let q_mask_s_root = binding.q_mask_s_root();
        let qpcs_pre_relation_transcript_digest = binding.qpcs_pre_relation_transcript_digest();
        let relation_seed = binding.qpcs_relation_challenge_seed();
        let lineage = binding.into_lineage_v1();
        Self::from_bound_parts_v1(
            Some(lineage),
            parameter_digest,
            q_mask_s_root,
            qpcs_pre_relation_transcript_digest,
            relation_seed,
        )
    }

    /// Require the private one-shot relation lineage to match the supplied
    /// qPCS-bound transcript.  Schedules reconstructed from final public seeds
    /// intentionally fail this check.
    pub(super) fn validate_qpcs_bound_lineage_v1(
        &self,
        transcript: &ZkAmsMkheRnsNativeQpcsBoundTranscriptV1,
    ) -> Result<(), RnsNativeQpcsPrefixErrorV1> {
        let lineage = self
            .lineage
            .as_ref()
            .ok_or(RnsNativeQpcsPrefixErrorV1::InvalidOrder)?;
        if !transcript.matches_qpcs_relation_lineage_v1(lineage) {
            return Err(RnsNativeQpcsPrefixErrorV1::InvalidContext);
        }
        Ok(())
    }

    pub(super) const fn has_qpcs_relation_lineage_v1(&self) -> bool {
        self.lineage.is_some()
    }

    pub(super) fn validate_context_v1(
        &self,
        parameter_digest: [u8; DIGEST_BYTES_V1],
        q_mask_s_root: [u8; DIGEST_BYTES_V1],
        qpcs_pre_relation_transcript_digest: [u8; DIGEST_BYTES_V1],
        relation_seed: [u8; DIGEST_BYTES_V1],
    ) -> Result<(), RnsNativeQpcsPrefixErrorV1> {
        if self.parameter_digest != parameter_digest
            || self.q_mask_s_root != q_mask_s_root
            || self.qpcs_pre_relation_transcript_digest != qpcs_pre_relation_transcript_digest
            || self.relation_seed != relation_seed
            || self.points.contains(&0)
        {
            return Err(RnsNativeQpcsPrefixErrorV1::InvalidContext);
        }
        Ok(())
    }

    pub(super) const fn parameter_digest(&self) -> [u8; DIGEST_BYTES_V1] {
        self.parameter_digest
    }

    pub(super) const fn q_mask_s_root(&self) -> [u8; DIGEST_BYTES_V1] {
        self.q_mask_s_root
    }

    pub(super) const fn qpcs_pre_relation_transcript_digest(&self) -> [u8; DIGEST_BYTES_V1] {
        self.qpcs_pre_relation_transcript_digest
    }

    pub(super) const fn relation_seed(&self) -> [u8; DIGEST_BYTES_V1] {
        self.relation_seed
    }

    pub(super) const fn points(&self) -> &[u64; RELATION_COUNT_V1] {
        &self.points
    }

    pub(super) fn point(&self, limb: usize, repetition: usize) -> Option<u64> {
        (limb < ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1 && repetition < REPETITIONS_V1)
            .then(|| self.points[limb * REPETITIONS_V1 + repetition])
    }

    #[cfg(test)]
    pub(super) fn test_fixture_v1(parameter_digest: [u8; DIGEST_BYTES_V1]) -> Self {
        Self::test_fixture_with_binding_v1(
            parameter_digest,
            [0xc1; DIGEST_BYTES_V1],
            [0xc2; DIGEST_BYTES_V1],
            [0xc3; DIGEST_BYTES_V1],
            [1; RELATION_COUNT_V1],
        )
    }

    #[cfg(test)]
    pub(super) const fn test_fixture_with_binding_v1(
        parameter_digest: [u8; DIGEST_BYTES_V1],
        q_mask_s_root: [u8; DIGEST_BYTES_V1],
        qpcs_pre_relation_transcript_digest: [u8; DIGEST_BYTES_V1],
        relation_seed: [u8; DIGEST_BYTES_V1],
        points: [u64; RELATION_COUNT_V1],
    ) -> Self {
        Self {
            lineage: None,
            parameter_digest,
            q_mask_s_root,
            qpcs_pre_relation_transcript_digest,
            relation_seed,
            points,
        }
    }

    #[cfg(test)]
    pub(super) const fn test_fixture_with_lineage_v1(
        parameter_digest: [u8; DIGEST_BYTES_V1],
        q_mask_s_root: [u8; DIGEST_BYTES_V1],
        qpcs_pre_relation_transcript_digest: [u8; DIGEST_BYTES_V1],
        relation_seed: [u8; DIGEST_BYTES_V1],
        points: [u64; RELATION_COUNT_V1],
        lineage: ZkAmsMkheRnsNativeQpcsRelationLineageV1,
    ) -> Self {
        Self {
            lineage: Some(lineage),
            parameter_digest,
            q_mask_s_root,
            qpcs_pre_relation_transcript_digest,
            relation_seed,
            points,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub(super) enum TreeRoleV1 {
    Quotient = 1,
    Fri = 2,
}

#[derive(Clone, Copy)]
pub(super) struct TreeDescriptorV1 {
    pub(super) opened: usize,
    pub(super) authentication: usize,
    pub(super) values_bytes: usize,
    pub(super) authentication_bytes: usize,
}

#[derive(Clone, Copy)]
pub(super) struct TreeViewV1<'a> {
    pub(super) values: &'a [u8],
    pub(super) authentication: &'a [u8],
}

struct PrefixViewV1<'a> {
    evaluations: &'a [u8],
    quotient: TreeViewV1<'a>,
    fri_zero: TreeViewV1<'a>,
    fri_one: TreeViewV1<'a>,
    residual: &'a [u8],
}

/// Move-only internal output after quotient, batching, and FRI fold zero.
///
/// This is only a sequencing token for the remaining private FRI verifier. It
/// is not a proof receipt and grants no candidate, readiness, or release
/// authority.
#[allow(
    missing_copy_implementations,
    reason = "qPCS substages must be consumed once and cannot be rewound"
)]
pub(super) struct RnsNativeQpcsFoldZeroStageV1<'a> {
    relation_schedule: Option<RnsNativeQpcsRelationScheduleV1>,
    parameter_digest: [u8; DIGEST_BYTES_V1],
    transcript_digest: [u8; DIGEST_BYTES_V1],
    query_seed: [u8; DIGEST_BYTES_V1],
    section_binding_digest: [u8; DIGEST_BYTES_V1],
    fri_one_root: [u8; DIGEST_BYTES_V1],
    queries: [u32; QUERY_COUNT_V1],
    fri_one_indices: IndexSetV1,
    fri_one_values: &'a [u8],
    evaluations: &'a [u8],
    evaluation_binding_digest: [u8; DIGEST_BYTES_V1],
    residual: &'a [u8],
}

impl<'a> RnsNativeQpcsFoldZeroStageV1<'a> {
    pub(super) fn take_relation_schedule_v1(
        &mut self,
    ) -> Result<RnsNativeQpcsRelationScheduleV1, RnsNativeQpcsPrefixErrorV1> {
        self.relation_schedule
            .take()
            .ok_or(RnsNativeQpcsPrefixErrorV1::InvalidOrder)
    }

    pub(super) const fn parameter_digest(&self) -> [u8; DIGEST_BYTES_V1] {
        self.parameter_digest
    }

    pub(super) const fn transcript_digest(&self) -> [u8; DIGEST_BYTES_V1] {
        self.transcript_digest
    }

    pub(super) const fn query_seed(&self) -> [u8; DIGEST_BYTES_V1] {
        self.query_seed
    }

    pub(super) const fn section_binding_digest(&self) -> [u8; DIGEST_BYTES_V1] {
        self.section_binding_digest
    }

    pub(super) const fn fri_one_root(&self) -> [u8; DIGEST_BYTES_V1] {
        self.fri_one_root
    }

    pub(super) const fn queries(&self) -> &[u32; QUERY_COUNT_V1] {
        &self.queries
    }

    pub(super) const fn fri_one_indices(&self) -> IndexSetV1 {
        self.fri_one_indices
    }

    pub(super) const fn fri_one_values(&self) -> &'a [u8] {
        self.fri_one_values
    }

    pub(super) const fn evaluations(&self) -> &'a [u8] {
        self.evaluations
    }

    pub(super) const fn evaluation_binding_digest(&self) -> [u8; DIGEST_BYTES_V1] {
        self.evaluation_binding_digest
    }

    pub(super) const fn residual(&self) -> &'a [u8] {
        self.residual
    }
}

#[derive(Clone, Copy)]
pub(super) struct IndexSetV1 {
    pub(super) values: [u32; MAX_OPENED_LEAVES_V1],
    pub(super) len: usize,
}

#[derive(Clone, Copy)]
struct FrontierNodeV1 {
    index: u32,
    digest: [u8; DIGEST_BYTES_V1],
}

const EMPTY_FRONTIER_NODE_V1: FrontierNodeV1 = FrontierNodeV1 {
    index: 0,
    digest: [0; DIGEST_BYTES_V1],
};

struct DecoderV1<'a> {
    bytes: &'a [u8],
    cursor: usize,
}

impl<'a> DecoderV1<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, cursor: 0 }
    }

    fn take(&mut self, len: usize) -> Result<&'a [u8], RnsNativeQpcsPrefixErrorV1> {
        let end = self
            .cursor
            .checked_add(len)
            .ok_or(RnsNativeQpcsPrefixErrorV1::ArithmeticOverflow)?;
        let value = self
            .bytes
            .get(self.cursor..end)
            .ok_or(RnsNativeQpcsPrefixErrorV1::Truncated)?;
        self.cursor = end;
        Ok(value)
    }

    fn u8(&mut self) -> Result<u8, RnsNativeQpcsPrefixErrorV1> {
        Ok(self.take(1)?[0])
    }

    fn u16(&mut self) -> Result<u16, RnsNativeQpcsPrefixErrorV1> {
        Ok(u16::from_be_bytes(
            self.take(2)?
                .try_into()
                .map_err(|_| RnsNativeQpcsPrefixErrorV1::Truncated)?,
        ))
    }

    fn u32(&mut self) -> Result<u32, RnsNativeQpcsPrefixErrorV1> {
        Ok(u32::from_be_bytes(
            self.take(4)?
                .try_into()
                .map_err(|_| RnsNativeQpcsPrefixErrorV1::Truncated)?,
        ))
    }

    fn digest(&mut self) -> Result<[u8; DIGEST_BYTES_V1], RnsNativeQpcsPrefixErrorV1> {
        self.take(DIGEST_BYTES_V1)?
            .try_into()
            .map_err(|_| RnsNativeQpcsPrefixErrorV1::Truncated)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct Fq2V1 {
    pub(super) c0: u64,
    pub(super) c1: u64,
}

impl Fq2V1 {
    pub(super) const ZERO: Self = Self { c0: 0, c1: 0 };
    pub(super) const ONE: Self = Self { c0: 1, c1: 0 };

    const fn base(value: u64) -> Self {
        Self { c0: value, c1: 0 }
    }
}

#[derive(Clone, Copy)]
pub(super) struct Fq2ParametersV1 {
    pub(super) modulus: u64,
    nonresidue: u64,
    pub(super) domain_root: Fq2V1,
}

impl Fq2ParametersV1 {
    pub(super) fn derive(modulus: u64) -> Result<Self, RnsNativeQpcsPrefixErrorV1> {
        if modulus < 3
            || modulus.is_multiple_of(2)
            || (modulus - 1).trailing_zeros() + (modulus + 1).trailing_zeros()
                < u32::from(ZK_AMS_MKHE_RNS_NATIVE_LDE_DOMAIN_LOG2_V1)
        {
            return Err(RnsNativeQpcsPrefixErrorV1::InvalidContext);
        }
        let nonresidue = (2_u64..=64)
            .find(|&candidate| mod_pow_v1(candidate, (modulus - 1) / 2, modulus) == modulus - 1)
            .ok_or(RnsNativeQpcsPrefixErrorV1::InvalidContext)?;
        let mut parameters = Self {
            modulus,
            nonresidue,
            domain_root: Fq2V1::ZERO,
        };
        let group_order = u128::from(modulus)
            .checked_mul(u128::from(modulus))
            .and_then(|value| value.checked_sub(1))
            .ok_or(RnsNativeQpcsPrefixErrorV1::ArithmeticOverflow)?;
        let exponent = group_order >> ZK_AMS_MKHE_RNS_NATIVE_LDE_DOMAIN_LOG2_V1;
        'outer: for c0 in 1_u64..=32 {
            for c1 in 1_u64..=32 {
                let root = parameters.pow(Fq2V1 { c0, c1 }, exponent);
                if parameters.pow(root, DOMAIN_SIZE_V1 as u128) == Fq2V1::ONE
                    && parameters.pow(root, (DOMAIN_SIZE_V1 / 2) as u128) != Fq2V1::ONE
                {
                    parameters.domain_root = root;
                    break 'outer;
                }
            }
        }
        if parameters.domain_root == Fq2V1::ZERO {
            return Err(RnsNativeQpcsPrefixErrorV1::InvalidContext);
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

    pub(super) fn pow(self, mut base: Fq2V1, mut exponent: u128) -> Fq2V1 {
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

    fn inverse(self, value: Fq2V1) -> Result<Fq2V1, RnsNativeQpcsPrefixErrorV1> {
        if value == Fq2V1::ZERO {
            return Err(RnsNativeQpcsPrefixErrorV1::InvalidFriEquation);
        }
        let exponent = u128::from(self.modulus)
            .checked_mul(u128::from(self.modulus))
            .and_then(|value| value.checked_sub(2))
            .ok_or(RnsNativeQpcsPrefixErrorV1::ArithmeticOverflow)?;
        Ok(self.pow(value, exponent))
    }
}

/// Verify the authenticated quotient/batch/first-fold prefix.
///
/// A successful return is intentionally only an internal substage result.  It
/// never grants candidate, readiness, or release authority.
pub(super) fn authenticate_rns_native_qpcs_prefix_v1<'a>(
    transcript: &ZkAmsMkheRnsNativeChallengeSeedsV1,
    equation_commitment_digests: &[[u8; DIGEST_BYTES_V1]],
    limb_commitment_digests: &[[u8; DIGEST_BYTES_V1]],
    query_opening_digests: &[[u8; DIGEST_BYTES_V1]],
    proof: &'a [u8],
) -> Result<RnsNativeQpcsFoldZeroStageV1<'a>, RnsNativeQpcsPrefixErrorV1> {
    let initial = authenticate_rns_native_qpcs_initial_v1(transcript, query_opening_digests, proof)
        .map_err(|_| RnsNativeQpcsPrefixErrorV1::InvalidInitial)?;
    let context = PrefixContextV1::from_transcript_v1(
        transcript,
        initial.parameter_digest(),
        equation_commitment_digests,
        limb_commitment_digests,
        query_opening_digests,
    )?;
    let relation_schedule =
        RnsNativeQpcsRelationScheduleV1::from_transcript_v1(context, transcript)?;
    verify_prefix_with_initial_v1(context, relation_schedule, initial)
}

/// Verify the qPCS prefix with the exact schedule minted before terminal roots.
#[allow(
    dead_code,
    reason = "the typed qPCS/cross-field orchestration adapter is not declared yet"
)]
pub(super) fn authenticate_rns_native_qpcs_prefix_with_schedule_v1<'a>(
    transcript: &ZkAmsMkheRnsNativeChallengeSeedsV1,
    relation_schedule: RnsNativeQpcsRelationScheduleV1,
    equation_commitment_digests: &[[u8; DIGEST_BYTES_V1]],
    limb_commitment_digests: &[[u8; DIGEST_BYTES_V1]],
    query_opening_digests: &[[u8; DIGEST_BYTES_V1]],
    proof: &'a [u8],
) -> Result<RnsNativeQpcsFoldZeroStageV1<'a>, RnsNativeQpcsPrefixErrorV1> {
    let initial = authenticate_rns_native_qpcs_initial_v1(transcript, query_opening_digests, proof)
        .map_err(|_| RnsNativeQpcsPrefixErrorV1::InvalidInitial)?;
    let context = PrefixContextV1::from_transcript_v1(
        transcript,
        initial.parameter_digest(),
        equation_commitment_digests,
        limb_commitment_digests,
        query_opening_digests,
    )?;
    verify_prefix_with_initial_v1(context, relation_schedule, initial)
}

fn verify_prefix_with_initial_v1<'a>(
    context: PrefixContextV1,
    relation_schedule: RnsNativeQpcsRelationScheduleV1,
    initial: RnsNativeQpcsInitialStageV1<'a>,
) -> Result<RnsNativeQpcsFoldZeroStageV1<'a>, RnsNativeQpcsPrefixErrorV1> {
    verify_prefix_parts_with_schedule_v1(
        context,
        relation_schedule,
        initial.queries(),
        initial.indices(),
        initial.values(),
        initial.continuation(),
    )
}

#[cfg(test)]
fn verify_prefix_parts_v1<'a>(
    context: PrefixContextV1,
    queries: &[u32; QUERY_COUNT_V1],
    initial_indices: &[u32],
    initial_values: &[u8],
    continuation: &'a [u8],
) -> Result<RnsNativeQpcsFoldZeroStageV1<'a>, RnsNativeQpcsPrefixErrorV1> {
    let relation_schedule = RnsNativeQpcsRelationScheduleV1::from_context_v1(context)?;
    verify_prefix_parts_with_schedule_v1(
        context,
        relation_schedule,
        queries,
        initial_indices,
        initial_values,
        continuation,
    )
}

fn verify_prefix_parts_with_schedule_v1<'a>(
    context: PrefixContextV1,
    relation_schedule: RnsNativeQpcsRelationScheduleV1,
    queries: &[u32; QUERY_COUNT_V1],
    initial_indices: &[u32],
    initial_values: &[u8],
    continuation: &'a [u8],
) -> Result<RnsNativeQpcsFoldZeroStageV1<'a>, RnsNativeQpcsPrefixErrorV1> {
    relation_schedule.validate_context_v1(
        context.parameter_digest,
        context.q_mask_s_root,
        context.qpcs_pre_relation_transcript_digest,
        context.relation_seed,
    )?;
    preflight_prefix_v1(continuation)?;
    let quotient_indices = query_pair_indices_v1(queries, DOMAIN_SIZE_V1)?;
    if &quotient_indices.values[..quotient_indices.len] != initial_indices {
        return Err(RnsNativeQpcsPrefixErrorV1::InvalidOrder);
    }
    let fri_zero_indices = quotient_indices;
    let fri_one_indices = query_pair_indices_v1(queries, FRI_ONE_SIZE_V1)?;
    let descriptors = [
        descriptor_for_indices_v1(quotient_indices, DOMAIN_SIZE_V1)?,
        descriptor_for_indices_v1(fri_zero_indices, DOMAIN_SIZE_V1)?,
        descriptor_for_indices_v1(fri_one_indices, FRI_ONE_SIZE_V1)?,
    ];
    let view = decode_prefix_exact_v1(continuation, context, descriptors)?;
    validate_leaf_values_v1(view.quotient.values, quotient_indices.len)?;
    validate_leaf_values_v1(view.fri_zero.values, fri_zero_indices.len)?;
    validate_leaf_values_v1(view.fri_one.values, fri_one_indices.len)?;
    authenticate_tree_v1(
        view.quotient,
        quotient_indices,
        DOMAIN_SIZE_V1,
        TreeRoleV1::Quotient,
        0,
        context.parameter_digest,
        context.quotient_root,
    )?;
    authenticate_tree_v1(
        view.fri_zero,
        fri_zero_indices,
        DOMAIN_SIZE_V1,
        TreeRoleV1::Fri,
        0,
        context.parameter_digest,
        context.fri_zero_root,
    )?;
    authenticate_tree_v1(
        view.fri_one,
        fri_one_indices,
        FRI_ONE_SIZE_V1,
        TreeRoleV1::Fri,
        1,
        context.parameter_digest,
        context.fri_one_root,
    )?;
    let relation_openings = RelationOpeningsV1 {
        initial_indices,
        initial_values,
        quotient_indices,
        quotient_values: view.quotient.values,
        fri_zero_indices,
        fri_zero_values: view.fri_zero.values,
        evaluations: view.evaluations,
    };
    verify_relations_openings_and_batch_with_schedule_v1(
        context,
        &relation_schedule,
        relation_openings,
    )?;
    verify_first_fold_v1(
        context,
        queries,
        fri_zero_indices,
        view.fri_zero.values,
        fri_one_indices,
        view.fri_one.values,
    )?;
    if residual_digest_v1(context, view.residual)? == [0; DIGEST_BYTES_V1] {
        return Err(RnsNativeQpcsPrefixErrorV1::InvalidHeader);
    }
    Ok(RnsNativeQpcsFoldZeroStageV1 {
        relation_schedule: Some(relation_schedule),
        parameter_digest: context.parameter_digest,
        transcript_digest: context.transcript_digest,
        query_seed: context.query_seed,
        section_binding_digest: context.section_binding_digest,
        fri_one_root: context.fri_one_root,
        queries: *queries,
        fri_one_indices,
        fri_one_values: view.fri_one.values,
        evaluations: view.evaluations,
        evaluation_binding_digest: evaluation_binding_digest_v1(context, view.evaluations)?,
        residual: view.residual,
    })
}

fn preflight_prefix_v1(prefix: &[u8]) -> Result<(), RnsNativeQpcsPrefixErrorV1> {
    if prefix.len() > ZK_AMS_MKHE_RNS_NATIVE_QPCS_MAX_BYTES_V1 as usize {
        return Err(RnsNativeQpcsPrefixErrorV1::ProofCapExceeded);
    }
    if prefix.len() < PREFIX_HEADER_BYTES_V1 {
        return Err(RnsNativeQpcsPrefixErrorV1::Truncated);
    }
    Ok(())
}

pub(super) fn descriptor_for_indices_v1(
    indices: IndexSetV1,
    length: usize,
) -> Result<TreeDescriptorV1, RnsNativeQpcsPrefixErrorV1> {
    let authentication = exact_authentication_count_v1(indices, length)?;
    let values_bytes = indices
        .len
        .checked_mul(LEAF_BYTES_V1)
        .ok_or(RnsNativeQpcsPrefixErrorV1::ArithmeticOverflow)?;
    let authentication_bytes = authentication
        .checked_mul(DIGEST_BYTES_V1)
        .ok_or(RnsNativeQpcsPrefixErrorV1::ArithmeticOverflow)?;
    if values_bytes
        .checked_add(authentication_bytes)
        .ok_or(RnsNativeQpcsPrefixErrorV1::ArithmeticOverflow)?
        > MAX_TREE_BYTES_V1
    {
        return Err(RnsNativeQpcsPrefixErrorV1::InvalidCount);
    }
    Ok(TreeDescriptorV1 {
        opened: indices.len,
        authentication,
        values_bytes,
        authentication_bytes,
    })
}

fn decode_prefix_exact_v1<'a>(
    prefix: &'a [u8],
    context: PrefixContextV1,
    expected: [TreeDescriptorV1; TREE_COUNT_V1],
) -> Result<PrefixViewV1<'a>, RnsNativeQpcsPrefixErrorV1> {
    preflight_prefix_v1(prefix)?;
    let mut decoder = DecoderV1::new(prefix);
    if decoder.take(PREFIX_MAGIC_V1.len())? != PREFIX_MAGIC_V1.as_slice()
        || decoder.u8()? != PREFIX_VERSION_V1
        || decoder.u8()? != ZK_AMS_MKHE_RNS_NATIVE_LDE_DOMAIN_LOG2_V1
        || usize::from(decoder.u8()?) != ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1
        || usize::from(decoder.u8()?) != ROWS_PER_LIMB_V1
        || decoder.u16()? != ZK_AMS_MKHE_RNS_NATIVE_QUERY_COUNT_V1
        || usize::from(decoder.u16()?) != RELATION_COUNT_V1
        || usize::from(decoder.u16()?) != EVALUATION_COUNT_V1
        || usize::from(decoder.u8()?) != TREE_COUNT_V1
        || decoder.u8()? != CHECKED_FOLD_COUNT_V1
    {
        return Err(RnsNativeQpcsPrefixErrorV1::InvalidHeader);
    }
    let evaluation_bytes = usize::try_from(decoder.u32()?)
        .map_err(|_| RnsNativeQpcsPrefixErrorV1::ArithmeticOverflow)?;
    let residual_bytes = usize::try_from(decoder.u32()?)
        .map_err(|_| RnsNativeQpcsPrefixErrorV1::ArithmeticOverflow)?;
    let mut encoded = [TreeDescriptorV1 {
        opened: 0,
        authentication: 0,
        values_bytes: 0,
        authentication_bytes: 0,
    }; TREE_COUNT_V1];
    for descriptor in &mut encoded {
        descriptor.opened = usize::from(decoder.u16()?);
        descriptor.authentication = usize::from(decoder.u16()?);
        descriptor.values_bytes = usize::try_from(decoder.u32()?)
            .map_err(|_| RnsNativeQpcsPrefixErrorV1::ArithmeticOverflow)?;
        descriptor.authentication_bytes = usize::try_from(decoder.u32()?)
            .map_err(|_| RnsNativeQpcsPrefixErrorV1::ArithmeticOverflow)?;
    }
    let parameter_digest = decoder.digest()?;
    let transcript_digest = decoder.digest()?;
    let rns_aggregation_seed = decoder.digest()?;
    let relation_seed = decoder.digest()?;
    let batching_seed = decoder.digest()?;
    let fold_zero_seed = decoder.digest()?;
    let query_seed = decoder.digest()?;
    let quotient_root = decoder.digest()?;
    let fri_zero_root = decoder.digest()?;
    let fri_one_root = decoder.digest()?;
    let section_binding_digest = decoder.digest()?;
    let encoded_evaluation_binding_digest = decoder.digest()?;
    let encoded_residual_digest = decoder.digest()?;
    if decoder.cursor != PREFIX_HEADER_BYTES_V1
        || evaluation_bytes != EVALUATION_BYTES_V1
        || residual_bytes == 0
        || encoded.iter().zip(expected).any(|(encoded, expected)| {
            encoded.opened != expected.opened
                || encoded.authentication != expected.authentication
                || encoded.values_bytes != expected.values_bytes
                || encoded.authentication_bytes != expected.authentication_bytes
        })
        || parameter_digest != context.parameter_digest
        || transcript_digest != context.transcript_digest
        || rns_aggregation_seed != context.rns_aggregation_seed
        || relation_seed != context.relation_seed
        || batching_seed != context.batching_seed
        || fold_zero_seed != context.fold_zero_seed
        || query_seed != context.query_seed
        || quotient_root != context.quotient_root
        || fri_zero_root != context.fri_zero_root
        || fri_one_root != context.fri_one_root
        || section_binding_digest != context.section_binding_digest
    {
        return Err(RnsNativeQpcsPrefixErrorV1::InvalidHeader);
    }
    let payload_bytes = expected
        .iter()
        .try_fold(evaluation_bytes, |total, descriptor| {
            total
                .checked_add(descriptor.values_bytes)
                .and_then(|value| value.checked_add(descriptor.authentication_bytes))
                .ok_or(RnsNativeQpcsPrefixErrorV1::ArithmeticOverflow)
        })?;
    let expected_total = PREFIX_HEADER_BYTES_V1
        .checked_add(payload_bytes)
        .and_then(|value| value.checked_add(residual_bytes))
        .ok_or(RnsNativeQpcsPrefixErrorV1::ArithmeticOverflow)?;
    if expected_total > ZK_AMS_MKHE_RNS_NATIVE_QPCS_MAX_BYTES_V1 as usize {
        return Err(RnsNativeQpcsPrefixErrorV1::ProofCapExceeded);
    }
    if prefix.len() < expected_total {
        return Err(RnsNativeQpcsPrefixErrorV1::Truncated);
    }
    if prefix.len() != expected_total {
        return Err(RnsNativeQpcsPrefixErrorV1::TrailingBytes);
    }
    let evaluations = decoder.take(evaluation_bytes)?;
    let quotient = read_tree_view_v1(&mut decoder, expected[0])?;
    let fri_zero = read_tree_view_v1(&mut decoder, expected[1])?;
    let fri_one = read_tree_view_v1(&mut decoder, expected[2])?;
    let residual = decoder.take(residual_bytes)?;
    if decoder.cursor != prefix.len()
        || encoded_evaluation_binding_digest != evaluation_binding_digest_v1(context, evaluations)?
        || encoded_residual_digest != residual_digest_v1(context, residual)?
    {
        return Err(RnsNativeQpcsPrefixErrorV1::InvalidHeader);
    }
    Ok(PrefixViewV1 {
        evaluations,
        quotient,
        fri_zero,
        fri_one,
        residual,
    })
}

fn read_tree_view_v1<'a>(
    decoder: &mut DecoderV1<'a>,
    descriptor: TreeDescriptorV1,
) -> Result<TreeViewV1<'a>, RnsNativeQpcsPrefixErrorV1> {
    Ok(TreeViewV1 {
        values: decoder.take(descriptor.values_bytes)?,
        authentication: decoder.take(descriptor.authentication_bytes)?,
    })
}

pub(super) fn query_pair_indices_v1(
    queries: &[u32; QUERY_COUNT_V1],
    length: usize,
) -> Result<IndexSetV1, RnsNativeQpcsPrefixErrorV1> {
    if length < 4 || !length.is_power_of_two() || length > DOMAIN_SIZE_V1 {
        return Err(RnsNativeQpcsPrefixErrorV1::InvalidCount);
    }
    let half =
        u32::try_from(length / 2).map_err(|_| RnsNativeQpcsPrefixErrorV1::ArithmeticOverflow)?;
    let mut values = [0_u32; MAX_OPENED_LEAVES_V1];
    for (ordinal, query) in queries.iter().copied().enumerate() {
        let base = query % half;
        values[2 * ordinal] = base;
        values[2 * ordinal + 1] = base
            .checked_add(half)
            .ok_or(RnsNativeQpcsPrefixErrorV1::ArithmeticOverflow)?;
    }
    values.sort_unstable();
    let mut len = 0_usize;
    for position in 0..values.len() {
        if len == 0 || values[position] != values[len - 1] {
            values[len] = values[position];
            len += 1;
        }
    }
    if len == 0 || len > MAX_OPENED_LEAVES_V1 {
        return Err(RnsNativeQpcsPrefixErrorV1::InvalidCount);
    }
    Ok(IndexSetV1 { values, len })
}

fn exact_authentication_count_v1(
    indices: IndexSetV1,
    mut length: usize,
) -> Result<usize, RnsNativeQpcsPrefixErrorV1> {
    if !length.is_power_of_two()
        || !(2..=DOMAIN_SIZE_V1).contains(&length)
        || indices.len == 0
        || indices.values[..indices.len]
            .iter()
            .any(|&index| usize::try_from(index).map_or(true, |index| index >= length))
    {
        return Err(RnsNativeQpcsPrefixErrorV1::InvalidCount);
    }
    let mut current = indices.values;
    let mut current_len = indices.len;
    let mut authentication = 0_usize;
    while length > 1 {
        let mut parents = [0_u32; MAX_OPENED_LEAVES_V1];
        let mut parent_len = 0_usize;
        for position in 0..current_len {
            let index = current[position];
            if current[..current_len].binary_search(&(index ^ 1)).is_err() {
                authentication = authentication
                    .checked_add(1)
                    .ok_or(RnsNativeQpcsPrefixErrorV1::ArithmeticOverflow)?;
            }
            let parent = index / 2;
            if parent_len == 0 || parents[parent_len - 1] != parent {
                parents[parent_len] = parent;
                parent_len += 1;
            }
        }
        current = parents;
        current_len = parent_len;
        length /= 2;
    }
    if authentication > MAX_AUTHENTICATION_HASHES_V1 {
        return Err(RnsNativeQpcsPrefixErrorV1::InvalidCount);
    }
    Ok(authentication)
}

pub(super) fn validate_leaf_values_v1(
    values: &[u8],
    opened: usize,
) -> Result<(), RnsNativeQpcsPrefixErrorV1> {
    if values.len() != opened * LEAF_BYTES_V1 {
        return Err(RnsNativeQpcsPrefixErrorV1::InvalidCount);
    }
    for leaf in values.chunks_exact(LEAF_BYTES_V1) {
        for coordinate in 0..COORDINATE_COUNT_V1 {
            let offset = coordinate * FQ2_BYTES_V1;
            let modulus = ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1[coordinate / ROWS_PER_LIMB_V1];
            if read_u64_v1(leaf, offset)? >= modulus || read_u64_v1(leaf, offset + 8)? >= modulus {
                return Err(RnsNativeQpcsPrefixErrorV1::NonCanonicalResidue);
            }
        }
    }
    Ok(())
}

pub(super) fn authenticate_tree_v1(
    tree: TreeViewV1<'_>,
    indices: IndexSetV1,
    length: usize,
    role: TreeRoleV1,
    layer: u8,
    parameter_digest: [u8; DIGEST_BYTES_V1],
    expected_root: [u8; DIGEST_BYTES_V1],
) -> Result<(), RnsNativeQpcsPrefixErrorV1> {
    if !length.is_power_of_two()
        || indices.len == 0
        || tree.values.len() != indices.len * LEAF_BYTES_V1
        || !tree.authentication.len().is_multiple_of(DIGEST_BYTES_V1)
    {
        return Err(RnsNativeQpcsPrefixErrorV1::InvalidCount);
    }
    let mut current = [EMPTY_FRONTIER_NODE_V1; MAX_OPENED_LEAVES_V1];
    let mut next = [EMPTY_FRONTIER_NODE_V1; MAX_OPENED_LEAVES_V1];
    for (position, node) in current.iter_mut().enumerate().take(indices.len) {
        let start = position * LEAF_BYTES_V1;
        *node = FrontierNodeV1 {
            index: indices.values[position],
            digest: tree_leaf_hash_v1(
                parameter_digest,
                role,
                layer,
                length,
                &tree.values[start..start + LEAF_BYTES_V1],
            )?,
        };
    }
    let mut current_len = indices.len;
    let mut nodes_at_height = length;
    let mut height = 1_usize;
    let mut authentication_cursor = 0_usize;
    while nodes_at_height > 1 {
        let mut cursor = 0_usize;
        let mut next_len = 0_usize;
        while cursor < current_len {
            let node = current[cursor];
            let sibling_index = node.index ^ 1;
            let (left, right);
            if node.index.is_multiple_of(2)
                && cursor + 1 < current_len
                && current[cursor + 1].index == sibling_index
            {
                left = node.digest;
                right = current[cursor + 1].digest;
                cursor += 2;
            } else {
                let start = authentication_cursor
                    .checked_mul(DIGEST_BYTES_V1)
                    .ok_or(RnsNativeQpcsPrefixErrorV1::ArithmeticOverflow)?;
                let sibling = tree
                    .authentication
                    .get(start..start + DIGEST_BYTES_V1)
                    .ok_or(RnsNativeQpcsPrefixErrorV1::InvalidMerklePath)?
                    .try_into()
                    .map_err(|_| RnsNativeQpcsPrefixErrorV1::InvalidMerklePath)?;
                authentication_cursor += 1;
                if node.index.is_multiple_of(2) {
                    left = node.digest;
                    right = sibling;
                } else {
                    left = sibling;
                    right = node.digest;
                }
                cursor += 1;
            }
            next[next_len] = FrontierNodeV1 {
                index: node.index / 2,
                digest: tree_node_hash_v1(
                    parameter_digest,
                    role,
                    layer,
                    length,
                    height,
                    left,
                    right,
                )?,
            };
            next_len += 1;
        }
        current[..next_len].copy_from_slice(&next[..next_len]);
        current_len = next_len;
        nodes_at_height /= 2;
        height += 1;
    }
    if current_len != 1
        || current[0].index != 0
        || current[0].digest != expected_root
        || authentication_cursor * DIGEST_BYTES_V1 != tree.authentication.len()
    {
        return Err(RnsNativeQpcsPrefixErrorV1::InvalidMerklePath);
    }
    Ok(())
}

#[cfg(test)]
fn verify_relations_openings_and_batch_v1(
    context: PrefixContextV1,
    openings: RelationOpeningsV1<'_>,
) -> Result<(), RnsNativeQpcsPrefixErrorV1> {
    let relation_schedule = RnsNativeQpcsRelationScheduleV1::from_context_v1(context)?;
    verify_relations_openings_and_batch_with_schedule_v1(context, &relation_schedule, openings)
}

struct RelationOpeningsV1<'a> {
    initial_indices: &'a [u32],
    initial_values: &'a [u8],
    quotient_indices: IndexSetV1,
    quotient_values: &'a [u8],
    fri_zero_indices: IndexSetV1,
    fri_zero_values: &'a [u8],
    evaluations: &'a [u8],
}

fn verify_relations_openings_and_batch_with_schedule_v1(
    context: PrefixContextV1,
    relation_schedule: &RnsNativeQpcsRelationScheduleV1,
    openings: RelationOpeningsV1<'_>,
) -> Result<(), RnsNativeQpcsPrefixErrorV1> {
    let RelationOpeningsV1 {
        initial_indices,
        initial_values,
        quotient_indices,
        quotient_values,
        fri_zero_indices,
        fri_zero_values,
        evaluations,
    } = openings;
    if evaluations.len() != EVALUATION_BYTES_V1 {
        return Err(RnsNativeQpcsPrefixErrorV1::InvalidCount);
    }
    relation_schedule.validate_context_v1(
        context.parameter_digest,
        context.q_mask_s_root,
        context.qpcs_pre_relation_transcript_digest,
        context.relation_seed,
    )?;
    let points = relation_schedule.points();
    let fields = derive_fields_v1()?;
    for (limb, field) in fields.into_iter().enumerate() {
        let modulus = field.modulus;
        let mut batch = [[Fq2V1::ZERO; 2]; ROWS_PER_LIMB_V1];
        for (row, coefficients) in batch.iter_mut().enumerate() {
            coefficients[0] = derive_fq2_challenge_v1(
                BATCH_CHALLENGE_DOMAIN_V1,
                context.parameter_digest,
                context.batching_seed,
                limb,
                row,
                0,
                modulus,
            )?;
            coefficients[1] = derive_fq2_challenge_v1(
                BATCH_CHALLENGE_DOMAIN_V1,
                context.parameter_digest,
                context.batching_seed,
                limb,
                row,
                1,
                modulus,
            )?;
        }
        for repetition in 0..REPETITIONS_V1 {
            let relation = limb * REPETITIONS_V1 + repetition;
            let product = read_u64_v1(evaluations, relation * 16)?;
            let quotient = read_u64_v1(evaluations, relation * 16 + 8)?;
            if product >= modulus || quotient >= modulus {
                return Err(RnsNativeQpcsPrefixErrorV1::NonCanonicalResidue);
            }
            let factor = mod_add_v1(
                mod_pow_v1(
                    points[relation],
                    ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1 as u64,
                    modulus,
                ),
                1,
                modulus,
            );
            if product != mod_mul_v1(factor, quotient, modulus) {
                return Err(RnsNativeQpcsPrefixErrorV1::InvalidRelation);
            }
        }
        for &index in initial_indices {
            let x = field.pow(field.domain_root, u128::from(index));
            for (row, coefficients) in batch.iter().enumerate() {
                let relation = limb * REPETITIONS_V1 + row / ROWS_PER_REPETITION_V1;
                let role = row % ROWS_PER_REPETITION_V1;
                let evaluation = read_u64_v1(evaluations, relation * 16 + role * 8)?;
                let coordinate = limb * ROWS_PER_LIMB_V1 + row;
                let committed =
                    read_value_from_slice_v1(initial_indices, initial_values, index, coordinate)?;
                let quotient = read_value_v1(quotient_indices, quotient_values, index, coordinate)?;
                if field.sub(committed, Fq2V1::base(evaluation))
                    != field.mul(field.sub(x, Fq2V1::base(points[relation])), quotient)
                {
                    return Err(RnsNativeQpcsPrefixErrorV1::InvalidOpeningQuotient);
                }
                let expected_batch = batch_value_v1(
                    field,
                    x,
                    committed,
                    quotient,
                    coefficients[0],
                    coefficients[1],
                    row,
                );
                if read_value_v1(fri_zero_indices, fri_zero_values, index, coordinate)?
                    != expected_batch
                {
                    return Err(RnsNativeQpcsPrefixErrorV1::InvalidBatchEquation);
                }
            }
        }
    }
    Ok(())
}

fn verify_first_fold_v1(
    context: PrefixContextV1,
    queries: &[u32; QUERY_COUNT_V1],
    fri_zero_indices: IndexSetV1,
    fri_zero_values: &[u8],
    fri_one_indices: IndexSetV1,
    fri_one_values: &[u8],
) -> Result<(), RnsNativeQpcsPrefixErrorV1> {
    let fields = derive_fields_v1()?;
    let half = u32::try_from(DOMAIN_SIZE_V1 / 2)
        .map_err(|_| RnsNativeQpcsPrefixErrorV1::ArithmeticOverflow)?;
    for (limb, field) in fields.into_iter().enumerate() {
        let mut alphas = [Fq2V1::ZERO; ROWS_PER_LIMB_V1];
        for (row, alpha) in alphas.iter_mut().enumerate() {
            *alpha = derive_fq2_challenge_v1(
                FOLD_CHALLENGE_DOMAIN_V1,
                context.parameter_digest,
                context.fold_zero_seed,
                limb,
                row,
                0,
                field.modulus,
            )?;
        }
        for &query in queries {
            let base = query % half;
            let x = field.pow(field.domain_root, u128::from(base));
            for (row, &alpha) in alphas.iter().enumerate() {
                let coordinate = limb * ROWS_PER_LIMB_V1 + row;
                let positive = read_value_v1(fri_zero_indices, fri_zero_values, base, coordinate)?;
                let negative =
                    read_value_v1(fri_zero_indices, fri_zero_values, base + half, coordinate)?;
                // Folding the pair at q and q + |D|/2 lands at q in the
                // half-sized codeword. The next multiproof opens both
                // q mod |D|/4 members, so select the member equal to q rather
                // than unconditionally selecting the lower member.
                let next = read_value_v1(fri_one_indices, fri_one_values, base, coordinate)?;
                if next != fold_value_v1(field, x, positive, negative, alpha)? {
                    return Err(RnsNativeQpcsPrefixErrorV1::InvalidFriEquation);
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
fn derive_relation_points_v1(
    context: PrefixContextV1,
) -> Result<[u64; RELATION_COUNT_V1], RnsNativeQpcsPrefixErrorV1> {
    derive_relation_points_from_seed_v1(context.parameter_digest, context.relation_seed)
}

fn derive_relation_points_from_seed_v1(
    parameter_digest: [u8; DIGEST_BYTES_V1],
    relation_seed: [u8; DIGEST_BYTES_V1],
) -> Result<[u64; RELATION_COUNT_V1], RnsNativeQpcsPrefixErrorV1> {
    let mut points = [0_u64; RELATION_COUNT_V1];
    for (limb, &modulus) in ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1.iter().enumerate() {
        let zone = u64::MAX - u64::MAX % modulus;
        for repetition in 0..REPETITIONS_V1 {
            let coordinate = limb * REPETITIONS_V1 + repetition;
            let prior = &points[limb * REPETITIONS_V1..coordinate];
            let mut accepted = None;
            for attempt in 0..MAX_CHALLENGE_ATTEMPTS_V1 {
                let candidate = derive_candidate_v1(
                    RELATION_POINT_DOMAIN_V1,
                    parameter_digest,
                    relation_seed,
                    limb,
                    repetition,
                    0,
                    modulus,
                    attempt,
                    0,
                )?;
                if candidate < zone {
                    let point = candidate % modulus;
                    if point != 0
                        && !prior.contains(&point)
                        && mod_add_v1(
                            mod_pow_v1(point, ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1 as u64, modulus),
                            1,
                            modulus,
                        ) != 0
                        && mod_pow_v1(point, DOMAIN_SIZE_V1 as u64, modulus) != 1
                    {
                        accepted = Some(point);
                        break;
                    }
                }
            }
            points[coordinate] = accepted.ok_or(RnsNativeQpcsPrefixErrorV1::InvalidChallenge)?;
        }
    }
    Ok(points)
}

#[allow(
    clippy::too_many_arguments,
    reason = "every fixed challenge axis is explicitly domain separated"
)]
fn derive_candidate_v1(
    domain: &[u8],
    parameter_digest: [u8; DIGEST_BYTES_V1],
    seed: [u8; DIGEST_BYTES_V1],
    limb: usize,
    row: usize,
    component: usize,
    modulus: u64,
    attempt: u16,
    half: u8,
) -> Result<u64, RnsNativeQpcsPrefixErrorV1> {
    let mut hash = Keccak256::new();
    hash.update(domain);
    hash.update(&[PREFIX_VERSION_V1]);
    hash.update(&parameter_digest);
    hash.update(&seed);
    hash.update(&[
        u8::try_from(limb).map_err(|_| RnsNativeQpcsPrefixErrorV1::ArithmeticOverflow)?,
        u8::try_from(row).map_err(|_| RnsNativeQpcsPrefixErrorV1::ArithmeticOverflow)?,
        u8::try_from(component).map_err(|_| RnsNativeQpcsPrefixErrorV1::ArithmeticOverflow)?,
        half,
    ]);
    hash.update(&modulus.to_be_bytes());
    hash.update(&attempt.to_be_bytes());
    let digest = hash.finalize();
    Ok(u64::from_be_bytes(digest[..8].try_into().map_err(
        |_| RnsNativeQpcsPrefixErrorV1::InvalidChallenge,
    )?))
}

#[allow(
    clippy::too_many_arguments,
    reason = "every fixed challenge axis is explicitly domain separated"
)]
fn derive_fq2_challenge_v1(
    domain: &[u8],
    parameter_digest: [u8; DIGEST_BYTES_V1],
    seed: [u8; DIGEST_BYTES_V1],
    limb: usize,
    row: usize,
    component: usize,
    modulus: u64,
) -> Result<Fq2V1, RnsNativeQpcsPrefixErrorV1> {
    let zone = u64::MAX - u64::MAX % modulus;
    for attempt in 0..MAX_CHALLENGE_ATTEMPTS_V1 {
        let c0 = derive_candidate_v1(
            domain,
            parameter_digest,
            seed,
            limb,
            row,
            component,
            modulus,
            attempt,
            0,
        )?;
        let c1 = derive_candidate_v1(
            domain,
            parameter_digest,
            seed,
            limb,
            row,
            component,
            modulus,
            attempt,
            1,
        )?;
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
    Err(RnsNativeQpcsPrefixErrorV1::InvalidChallenge)
}

pub(super) fn derive_fields_v1()
-> Result<[Fq2ParametersV1; ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1], RnsNativeQpcsPrefixErrorV1> {
    let first = Fq2ParametersV1::derive(ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1[0])?;
    let mut fields = [first; ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1];
    for limb in 1..ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1 {
        fields[limb] = Fq2ParametersV1::derive(ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1[limb])?;
    }
    Ok(fields)
}

fn batch_value_v1(
    field: Fq2ParametersV1,
    x: Fq2V1,
    committed: Fq2V1,
    quotient: Fq2V1,
    a: Fq2V1,
    b: Fq2V1,
    row: usize,
) -> Fq2V1 {
    let (committed_power, quotient_power) = if row.is_multiple_of(2) {
        (Fq2V1::ONE, x)
    } else {
        let x_to_n = field.pow(x, ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1 as u128);
        (x_to_n, field.mul(x_to_n, x))
    };
    field.add(
        field.mul(a, field.mul(committed_power, committed)),
        field.mul(b, field.mul(quotient_power, quotient)),
    )
}

pub(super) fn fold_value_v1(
    field: Fq2ParametersV1,
    x: Fq2V1,
    positive: Fq2V1,
    negative: Fq2V1,
    alpha: Fq2V1,
) -> Result<Fq2V1, RnsNativeQpcsPrefixErrorV1> {
    let inverse_x = field.inverse(x)?;
    Ok(fold_value_with_inverse_x_v1(
        field, inverse_x, positive, negative, alpha,
    ))
}

/// Evaluate one binary FRI fold when the caller already has the nonzero
/// domain point's inverse.
///
/// Complete FRI verification derives both `x` and `x^-1` from the exact root
/// schedule, avoiding a field inversion for every row while retaining the
/// same equation as [`fold_value_v1`].
pub(super) fn fold_value_with_inverse_x_v1(
    field: Fq2ParametersV1,
    inverse_x: Fq2V1,
    positive: Fq2V1,
    negative: Fq2V1,
    alpha: Fq2V1,
) -> Fq2V1 {
    let inverse_two = field.modulus.div_ceil(2);
    let inverse_two_x = field.scale(inverse_x, inverse_two);
    let even = field.scale(field.add(positive, negative), inverse_two);
    let odd = field.mul(field.sub(positive, negative), inverse_two_x);
    field.add(even, field.mul(alpha, odd))
}

pub(super) fn read_value_v1(
    indices: IndexSetV1,
    values: &[u8],
    index: u32,
    coordinate: usize,
) -> Result<Fq2V1, RnsNativeQpcsPrefixErrorV1> {
    read_value_from_slice_v1(&indices.values[..indices.len], values, index, coordinate)
}

fn read_value_from_slice_v1(
    indices: &[u32],
    values: &[u8],
    index: u32,
    coordinate: usize,
) -> Result<Fq2V1, RnsNativeQpcsPrefixErrorV1> {
    if coordinate >= COORDINATE_COUNT_V1 || values.len() != indices.len() * LEAF_BYTES_V1 {
        return Err(RnsNativeQpcsPrefixErrorV1::InvalidCount);
    }
    let position = indices
        .binary_search(&index)
        .map_err(|_| RnsNativeQpcsPrefixErrorV1::InvalidOrder)?;
    let offset = position
        .checked_mul(LEAF_BYTES_V1)
        .and_then(|value| value.checked_add(coordinate * FQ2_BYTES_V1))
        .ok_or(RnsNativeQpcsPrefixErrorV1::ArithmeticOverflow)?;
    Ok(Fq2V1 {
        c0: read_u64_v1(values, offset)?,
        c1: read_u64_v1(values, offset + 8)?,
    })
}

pub(super) fn tree_leaf_hash_v1(
    parameter_digest: [u8; DIGEST_BYTES_V1],
    role: TreeRoleV1,
    layer: u8,
    length: usize,
    values: &[u8],
) -> Result<[u8; DIGEST_BYTES_V1], RnsNativeQpcsPrefixErrorV1> {
    if values.len() != LEAF_BYTES_V1 {
        return Err(RnsNativeQpcsPrefixErrorV1::InvalidCount);
    }
    let mut hash = Keccak256::new();
    hash.update(TREE_LEAF_DOMAIN_V1);
    hash.update(&[PREFIX_VERSION_V1, role as u8, layer]);
    hash.update(&parameter_digest);
    hash.update(
        &u32::try_from(length)
            .map_err(|_| RnsNativeQpcsPrefixErrorV1::ArithmeticOverflow)?
            .to_be_bytes(),
    );
    hash.update(
        &u16::try_from(COORDINATE_COUNT_V1)
            .map_err(|_| RnsNativeQpcsPrefixErrorV1::ArithmeticOverflow)?
            .to_be_bytes(),
    );
    hash.update(values);
    Ok(hash.finalize())
}

#[allow(
    clippy::too_many_arguments,
    reason = "the Merkle domain includes every fixed tree axis"
)]
pub(super) fn tree_node_hash_v1(
    parameter_digest: [u8; DIGEST_BYTES_V1],
    role: TreeRoleV1,
    layer: u8,
    length: usize,
    height: usize,
    left: [u8; DIGEST_BYTES_V1],
    right: [u8; DIGEST_BYTES_V1],
) -> Result<[u8; DIGEST_BYTES_V1], RnsNativeQpcsPrefixErrorV1> {
    let mut hash = Keccak256::new();
    hash.update(TREE_NODE_DOMAIN_V1);
    hash.update(&[PREFIX_VERSION_V1, role as u8, layer]);
    hash.update(&parameter_digest);
    hash.update(
        &u32::try_from(length)
            .map_err(|_| RnsNativeQpcsPrefixErrorV1::ArithmeticOverflow)?
            .to_be_bytes(),
    );
    hash.update(&[
        u8::try_from(height).map_err(|_| RnsNativeQpcsPrefixErrorV1::ArithmeticOverflow)?
    ]);
    hash.update(&left);
    hash.update(&right);
    Ok(hash.finalize())
}

fn section_binding_digest_v1(
    transcript_digest: [u8; DIGEST_BYTES_V1],
    equation_commitment_digests: &[[u8; DIGEST_BYTES_V1]],
    limb_commitment_digests: &[[u8; DIGEST_BYTES_V1]],
    query_opening_digests: &[[u8; DIGEST_BYTES_V1]],
) -> Result<[u8; DIGEST_BYTES_V1], RnsNativeQpcsPrefixErrorV1> {
    let mut hash = Keccak256::new();
    hash.update(SECTION_BINDING_DOMAIN_V1);
    hash.update(&[PREFIX_VERSION_V1]);
    hash.update(&transcript_digest);
    hash.update(&[
        u8::try_from(equation_commitment_digests.len())
            .map_err(|_| RnsNativeQpcsPrefixErrorV1::ArithmeticOverflow)?,
        u8::try_from(limb_commitment_digests.len())
            .map_err(|_| RnsNativeQpcsPrefixErrorV1::ArithmeticOverflow)?,
    ]);
    hash.update(
        &u16::try_from(query_opening_digests.len())
            .map_err(|_| RnsNativeQpcsPrefixErrorV1::ArithmeticOverflow)?
            .to_be_bytes(),
    );
    for digest in equation_commitment_digests
        .iter()
        .chain(limb_commitment_digests)
        .chain(query_opening_digests)
    {
        hash.update(digest);
    }
    let digest = hash.finalize();
    if digest == [0; DIGEST_BYTES_V1] {
        return Err(RnsNativeQpcsPrefixErrorV1::InvalidContext);
    }
    Ok(digest)
}

fn evaluation_binding_digest_v1(
    context: PrefixContextV1,
    evaluations: &[u8],
) -> Result<[u8; DIGEST_BYTES_V1], RnsNativeQpcsPrefixErrorV1> {
    if evaluations.len() != EVALUATION_BYTES_V1 {
        return Err(RnsNativeQpcsPrefixErrorV1::InvalidCount);
    }
    let mut hash = Keccak256::new();
    hash.update(EVALUATION_BINDING_DOMAIN_V1);
    hash.update(&[PREFIX_VERSION_V1]);
    hash.update(&context.parameter_digest);
    hash.update(&context.transcript_digest);
    hash.update(&context.relation_seed);
    hash.update(&context.section_binding_digest);
    let aggregation_identity = rlwe_aggregation_identity_v1(context)?;
    hash.update(
        &u16::try_from(EVALUATION_COUNT_V1)
            .map_err(|_| RnsNativeQpcsPrefixErrorV1::ArithmeticOverflow)?
            .to_be_bytes(),
    );
    for limb in 0..ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1 {
        for repetition in 0..REPETITIONS_V1 {
            for role in 0..ROWS_PER_REPETITION_V1 {
                let row = repetition * ROWS_PER_REPETITION_V1 + role;
                let relation = limb * REPETITIONS_V1 + repetition;
                let offset = relation * 16 + role * 8;
                hash.update(&[
                    u8::try_from(limb)
                        .map_err(|_| RnsNativeQpcsPrefixErrorV1::ArithmeticOverflow)?,
                    u8::try_from(row)
                        .map_err(|_| RnsNativeQpcsPrefixErrorV1::ArithmeticOverflow)?,
                    u8::try_from(role)
                        .map_err(|_| RnsNativeQpcsPrefixErrorV1::ArithmeticOverflow)?,
                ]);
                // Row roles are Product/OpeningQuotient, not RLWE equation
                // ordinals. Bind the ordered equation pair through one exact
                // transcript-derived aggregation identity in every record.
                hash.update(&aggregation_identity);
                hash.update(&context.limb_commitment_digests[limb]);
                hash.update(
                    evaluations
                        .get(offset..offset + 8)
                        .ok_or(RnsNativeQpcsPrefixErrorV1::Truncated)?,
                );
            }
        }
    }
    let digest = hash.finalize();
    if digest == [0; DIGEST_BYTES_V1] {
        return Err(RnsNativeQpcsPrefixErrorV1::InvalidContext);
    }
    Ok(digest)
}

fn rlwe_aggregation_identity_v1(
    context: PrefixContextV1,
) -> Result<[u8; DIGEST_BYTES_V1], RnsNativeQpcsPrefixErrorV1> {
    let mut hash = Keccak256::new();
    hash.update(RLWE_AGGREGATION_IDENTITY_DOMAIN_V1);
    hash.update(&[PREFIX_VERSION_V1, 2]);
    hash.update(&context.parameter_digest);
    hash.update(&context.transcript_digest);
    hash.update(&context.rns_aggregation_seed);
    for (ordinal, digest) in context.equation_commitment_digests.iter().enumerate() {
        hash.update(&[
            u8::try_from(ordinal).map_err(|_| RnsNativeQpcsPrefixErrorV1::ArithmeticOverflow)?
        ]);
        hash.update(digest);
    }
    let digest = hash.finalize();
    if digest == [0; DIGEST_BYTES_V1] {
        return Err(RnsNativeQpcsPrefixErrorV1::InvalidContext);
    }
    Ok(digest)
}

fn residual_digest_v1(
    context: PrefixContextV1,
    residual: &[u8],
) -> Result<[u8; DIGEST_BYTES_V1], RnsNativeQpcsPrefixErrorV1> {
    let mut hash = Keccak256::new();
    hash.update(RESIDUAL_DOMAIN_V1);
    hash.update(&[PREFIX_VERSION_V1]);
    for digest in [
        context.parameter_digest,
        context.transcript_digest,
        context.rns_aggregation_seed,
        context.relation_seed,
        context.batching_seed,
        context.fold_zero_seed,
        context.query_seed,
        context.quotient_root,
        context.fri_zero_root,
        context.fri_one_root,
        context.section_binding_digest,
    ] {
        hash.update(&digest);
    }
    hash.update(&rlwe_aggregation_identity_v1(context)?);
    hash.update(
        &u32::try_from(residual.len())
            .map_err(|_| RnsNativeQpcsPrefixErrorV1::ArithmeticOverflow)?
            .to_be_bytes(),
    );
    hash.update(residual);
    Ok(hash.finalize())
}

fn read_u64_v1(bytes: &[u8], offset: usize) -> Result<u64, RnsNativeQpcsPrefixErrorV1> {
    let end = offset
        .checked_add(8)
        .ok_or(RnsNativeQpcsPrefixErrorV1::ArithmeticOverflow)?;
    Ok(u64::from_be_bytes(
        bytes
            .get(offset..end)
            .ok_or(RnsNativeQpcsPrefixErrorV1::Truncated)?
            .try_into()
            .map_err(|_| RnsNativeQpcsPrefixErrorV1::Truncated)?,
    ))
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

#[cfg(test)]
#[path = "rns_native_qpcs_prefix_tests.rs"]
mod tests;
