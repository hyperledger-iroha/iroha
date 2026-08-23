//! Complete correlated FRI closure for the replacement 40-limb qPCS.
//!
//! This private verifier consumes the authenticated fold-zero substage,
//! authenticates FRI layers two through seventeen, checks folds one through
//! sixteen, and derives the terminal degree equation directly from all four
//! authenticated layer-seventeen leaves.  No uncommitted terminal value is
//! accepted from the prover.  Successful verification remains non-authorizing:
//! the RLWE/source linkage is retained as a nonempty digest-bound residual and
//! the composite verifier continues to fail closed.

use super::{
    rns_native_profile::{
        ZK_AMS_MKHE_RNS_NATIVE_CORRELATED_FRI_MAX_BYTES_V1, ZK_AMS_MKHE_RNS_NATIVE_FRI_ROUNDS_V1,
        ZK_AMS_MKHE_RNS_NATIVE_LDE_DOMAIN_LOG2_V1, ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1,
        ZK_AMS_MKHE_RNS_NATIVE_QPCS_MAX_BYTES_V1, ZK_AMS_MKHE_RNS_NATIVE_QUERY_COUNT_V1,
    },
    rns_native_qpcs_prefix::{
        DIGEST_BYTES_V1, DOMAIN_SIZE_V1, Fq2ParametersV1, Fq2V1, IndexSetV1, LEAF_BYTES_V1,
        MAX_OPENED_LEAVES_V1, QUERY_COUNT_V1, ROWS_PER_LIMB_V1, RnsNativeQpcsFoldZeroStageV1,
        RnsNativeQpcsRelationScheduleV1, TreeDescriptorV1, TreeRoleV1, TreeViewV1,
        authenticate_rns_native_qpcs_prefix_v1,
        authenticate_rns_native_qpcs_prefix_with_schedule_v1, authenticate_tree_v1,
        derive_fields_v1, descriptor_for_indices_v1, fold_value_with_inverse_x_v1,
        query_pair_indices_v1, read_value_v1, validate_leaf_values_v1,
    },
    rns_native_transcript::{
        ZkAmsMkheRnsNativeChallengeSeedsV1, ZkAmsMkheRnsNativeCrossFieldBoundTranscriptV1,
        ZkAmsMkheRnsNativeCrossFieldRootClaimV1,
        ZkAmsMkheRnsNativeCrossFieldRootEqualityObligationV1,
        ZkAmsMkheRnsNativeQpcsBoundTranscriptV1,
    },
};
use crate::vega::sponge::Keccak256;

const CLOSURE_MAGIC_V1: [u8; 4] = *b"ZQFC";
const CLOSURE_VERSION_V1: u8 = 1;
const FIRST_ENCODED_LAYER_V1: usize = 2;
const LAST_LAYER_V1: usize = ZK_AMS_MKHE_RNS_NATIVE_FRI_ROUNDS_V1 as usize - 1;
const ENCODED_LAYER_COUNT_V1: usize = LAST_LAYER_V1 - FIRST_ENCODED_LAYER_V1 + 1;
const FIRST_CHECKED_FOLD_V1: u8 = 1;
const LAST_CHECKED_FOLD_V1: u8 = ZK_AMS_MKHE_RNS_NATIVE_FRI_ROUNDS_V1 - 1;
const TERMINAL_DERIVED_V1: u8 = 1;
const TREE_DESCRIPTOR_BYTES_V1: usize = 2 + 2 + 4 + 4;
const CLOSURE_HEADER_BYTES_V1: usize = 4
    + 4
    + 2
    + 6
    + 2 * 2
    + 3 * 4
    + ENCODED_LAYER_COUNT_V1 * TREE_DESCRIPTOR_BYTES_V1
    + 6 * DIGEST_BYTES_V1;
const MAX_FRI_OPENED_LEAVES_V1: usize = 4_028;
const MAX_FRI_AUTHENTICATION_HASHES_V1: usize = 20_030;
const MAX_CHALLENGE_ATTEMPTS_V1: u16 = 256;

const SCHEDULE_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.rns-native-qpcs.fri-complete.schedule";
const FOLD_CHALLENGE_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.rns-native-qpcs.fri-complete.fold";
const RESIDUAL_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-qpcs.fri-complete.rlwe-source-residual";

const _: () = {
    assert!(ZK_AMS_MKHE_RNS_NATIVE_FRI_ROUNDS_V1 == 18);
    assert!(FIRST_ENCODED_LAYER_V1 == 2);
    assert!(LAST_LAYER_V1 == 17);
    assert!(ENCODED_LAYER_COUNT_V1 == 16);
    assert!(CLOSURE_HEADER_BYTES_V1 == 416);
    assert!(QUERY_COUNT_V1 == 160);
    assert!(MAX_OPENED_LEAVES_V1 == 320);
    assert!(LEAF_BYTES_V1 == 6_400);
    assert!(MAX_FRI_OPENED_LEAVES_V1 == 4_028);
    assert!(MAX_FRI_AUTHENTICATION_HASHES_V1 == 20_030);
    assert!(ZK_AMS_MKHE_RNS_NATIVE_CORRELATED_FRI_MAX_BYTES_V1 == 26_409_984);
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RnsNativeQpcsFriCompleteErrorV1 {
    InvalidPrefix,
    InvalidContext,
    ProofCapExceeded,
    Truncated,
    TrailingBytes,
    InvalidHeader,
    InvalidCount,
    InvalidOrder,
    NonCanonicalResidue,
    InvalidMerklePath,
    InvalidChallenge,
    InvalidFriEquation,
    InvalidTerminalCoverage,
    InvalidTerminalDegree,
    ArithmeticOverflow,
}

impl core::fmt::Display for RnsNativeQpcsFriCompleteErrorV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for RnsNativeQpcsFriCompleteErrorV1 {}

#[derive(Clone, Copy)]
struct FriClosureContextV1 {
    parameter_digest: [u8; DIGEST_BYTES_V1],
    transcript_digest: [u8; DIGEST_BYTES_V1],
    qpcs_bound_transcript_state: [u8; DIGEST_BYTES_V1],
    query_seed: [u8; DIGEST_BYTES_V1],
    section_binding_digest: [u8; DIGEST_BYTES_V1],
    roots: [[u8; DIGEST_BYTES_V1]; ZK_AMS_MKHE_RNS_NATIVE_FRI_ROUNDS_V1 as usize],
    fold_seeds: [[u8; DIGEST_BYTES_V1]; ZK_AMS_MKHE_RNS_NATIVE_FRI_ROUNDS_V1 as usize],
    schedule_digest: [u8; DIGEST_BYTES_V1],
}

impl FriClosureContextV1 {
    fn from_transcript_v1(
        transcript: &ZkAmsMkheRnsNativeChallengeSeedsV1,
        prefix: &RnsNativeQpcsFoldZeroStageV1<'_>,
    ) -> Result<Self, RnsNativeQpcsFriCompleteErrorV1> {
        let roots = core::array::from_fn(|layer| transcript.qpcs_fri_roots()[layer].root());
        for (layer, root) in transcript.qpcs_fri_roots().iter().enumerate() {
            if usize::from(root.layer()) != layer {
                return Err(RnsNativeQpcsFriCompleteErrorV1::InvalidOrder);
            }
        }
        let fold_seeds = *transcript.qpcs_fri_fold_challenge_seeds();
        let parameter_digest = prefix.parameter_digest();
        let transcript_digest = transcript.transcript_digest();
        let qpcs_bound_transcript_state = transcript.qpcs_bound_transcript_state_v1();
        let query_seed = transcript.qpcs_query_challenge_seed();
        let section_binding_digest = prefix.section_binding_digest();
        if transcript_digest != prefix.transcript_digest()
            || query_seed != prefix.query_seed()
            || roots[1] != prefix.fri_one_root()
            || [
                parameter_digest,
                transcript_digest,
                qpcs_bound_transcript_state,
                query_seed,
                section_binding_digest,
            ]
            .contains(&[0; DIGEST_BYTES_V1])
            || roots.contains(&[0; DIGEST_BYTES_V1])
            || fold_seeds.contains(&[0; DIGEST_BYTES_V1])
        {
            return Err(RnsNativeQpcsFriCompleteErrorV1::InvalidContext);
        }
        let mut context = Self {
            parameter_digest,
            transcript_digest,
            qpcs_bound_transcript_state,
            query_seed,
            section_binding_digest,
            roots,
            fold_seeds,
            schedule_digest: [0; DIGEST_BYTES_V1],
        };
        context.schedule_digest = schedule_digest_v1(context)?;
        Ok(context)
    }
}

#[derive(Clone, Copy)]
struct ClosureShapeV1 {
    indices: [IndexSetV1; ENCODED_LAYER_COUNT_V1],
    descriptors: [TreeDescriptorV1; ENCODED_LAYER_COUNT_V1],
    aggregate_opened: usize,
    aggregate_authentication: usize,
    aggregate_values_bytes: usize,
    aggregate_authentication_bytes: usize,
}

struct ClosureViewV1<'a> {
    layers: [TreeViewV1<'a>; ENCODED_LAYER_COUNT_V1],
    downstream_residual: &'a [u8],
}

struct DecoderV1<'a> {
    bytes: &'a [u8],
    cursor: usize,
}

impl<'a> DecoderV1<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, cursor: 0 }
    }

    fn take(&mut self, len: usize) -> Result<&'a [u8], RnsNativeQpcsFriCompleteErrorV1> {
        let end = self
            .cursor
            .checked_add(len)
            .ok_or(RnsNativeQpcsFriCompleteErrorV1::ArithmeticOverflow)?;
        let value = self
            .bytes
            .get(self.cursor..end)
            .ok_or(RnsNativeQpcsFriCompleteErrorV1::Truncated)?;
        self.cursor = end;
        Ok(value)
    }

    fn u8(&mut self) -> Result<u8, RnsNativeQpcsFriCompleteErrorV1> {
        Ok(self.take(1)?[0])
    }

    fn u16(&mut self) -> Result<u16, RnsNativeQpcsFriCompleteErrorV1> {
        Ok(u16::from_be_bytes(
            self.take(2)?
                .try_into()
                .map_err(|_| RnsNativeQpcsFriCompleteErrorV1::Truncated)?,
        ))
    }

    fn u32(&mut self) -> Result<u32, RnsNativeQpcsFriCompleteErrorV1> {
        Ok(u32::from_be_bytes(
            self.take(4)?
                .try_into()
                .map_err(|_| RnsNativeQpcsFriCompleteErrorV1::Truncated)?,
        ))
    }

    fn digest(&mut self) -> Result<[u8; DIGEST_BYTES_V1], RnsNativeQpcsFriCompleteErrorV1> {
        self.take(DIGEST_BYTES_V1)?
            .try_into()
            .map_err(|_| RnsNativeQpcsFriCompleteErrorV1::Truncated)
    }
}

/// Move-only internal output after the complete qPCS FRI argument.
///
/// The retained bytes still require the separate RLWE/source-linkage verifier;
/// this token is therefore deliberately non-authorizing.
#[allow(
    dead_code,
    missing_copy_implementations,
    reason = "the following private RLWE/source milestone will consume this stage once"
)]
pub(super) struct RnsNativeQpcsFriCompleteStageV1<'a> {
    relation_schedule: Option<RnsNativeQpcsRelationScheduleV1>,
    qpcs_bound_transcript_state: [u8; DIGEST_BYTES_V1],
    parameter_digest: [u8; DIGEST_BYTES_V1],
    transcript_digest: [u8; DIGEST_BYTES_V1],
    query_seed: [u8; DIGEST_BYTES_V1],
    section_binding_digest: [u8; DIGEST_BYTES_V1],
    schedule_digest: [u8; DIGEST_BYTES_V1],
    evaluations: &'a [u8],
    evaluation_binding_digest: [u8; DIGEST_BYTES_V1],
    residual_digest: [u8; DIGEST_BYTES_V1],
    rlwe_source_residual: &'a [u8],
}

/// Move-only joint owner proving that a completed qPCS relation schedule and
/// a qPCS-bound transcript descend from the same one-shot relation lineage and
/// have the exact post-FRI, pre-cross-field transcript state retained by this
/// completed closure.
///
/// Construction is private to this module and occurs only by consuming the
/// retained schedule from a successfully completed FRI stage.  In particular,
/// the legacy schedule reconstructed from final public challenge seeds cannot
/// construct this owner.
#[allow(
    dead_code,
    missing_copy_implementations,
    reason = "the completed qPCS lineage and its sole transcript must move together"
)]
pub(super) struct RnsNativeQpcsCompletedLineageV1 {
    relation_schedule: RnsNativeQpcsRelationScheduleV1,
    qpcs_transcript: Option<ZkAmsMkheRnsNativeQpcsBoundTranscriptV1>,
}

#[allow(
    dead_code,
    reason = "the undeclared direct adapter consumes this completed qPCS owner"
)]
impl RnsNativeQpcsCompletedLineageV1 {
    fn from_completed_fri_v1(
        relation_schedule: RnsNativeQpcsRelationScheduleV1,
        expected_qpcs_bound_transcript_state: [u8; DIGEST_BYTES_V1],
        qpcs_transcript: ZkAmsMkheRnsNativeQpcsBoundTranscriptV1,
    ) -> Result<Self, RnsNativeQpcsFriCompleteErrorV1> {
        relation_schedule
            .validate_qpcs_bound_lineage_v1(&qpcs_transcript)
            .map_err(|_| RnsNativeQpcsFriCompleteErrorV1::InvalidContext)?;
        if expected_qpcs_bound_transcript_state == [0; DIGEST_BYTES_V1]
            || qpcs_transcript.binding_digest() != expected_qpcs_bound_transcript_state
        {
            return Err(RnsNativeQpcsFriCompleteErrorV1::InvalidContext);
        }
        Ok(Self {
            relation_schedule,
            qpcs_transcript: Some(qpcs_transcript),
        })
    }

    pub(super) const fn relation_schedule_v1(&self) -> &RnsNativeQpcsRelationScheduleV1 {
        &self.relation_schedule
    }

    pub(super) const fn has_unconsumed_qpcs_transcript_v1(&self) -> bool {
        self.qpcs_transcript.is_some()
    }

    #[cfg(test)]
    pub(super) fn qpcs_transcript_binding_digest_v1(
        &self,
    ) -> Result<[u8; DIGEST_BYTES_V1], RnsNativeQpcsFriCompleteErrorV1> {
        self.qpcs_transcript
            .as_ref()
            .map(ZkAmsMkheRnsNativeQpcsBoundTranscriptV1::binding_digest)
            .ok_or(RnsNativeQpcsFriCompleteErrorV1::InvalidOrder)
    }

    pub(super) fn take_qpcs_transcript_v1(
        &mut self,
    ) -> Result<ZkAmsMkheRnsNativeQpcsBoundTranscriptV1, RnsNativeQpcsFriCompleteErrorV1> {
        self.qpcs_transcript
            .take()
            .ok_or(RnsNativeQpcsFriCompleteErrorV1::InvalidOrder)
    }

    /// Consume the sole qPCS transcript against an authenticated claimed
    /// cross-field root.  The returned transcript is provisional until the
    /// opaque equality obligation is discharged by the direct verifier.
    pub(super) fn bind_claimed_cross_field_root_v1(
        &mut self,
        claim: ZkAmsMkheRnsNativeCrossFieldRootClaimV1,
    ) -> Result<
        (
            ZkAmsMkheRnsNativeCrossFieldBoundTranscriptV1,
            ZkAmsMkheRnsNativeCrossFieldRootEqualityObligationV1,
        ),
        RnsNativeQpcsFriCompleteErrorV1,
    > {
        self.take_qpcs_transcript_v1()?
            .bind_claimed_cross_field_root_v1(claim)
            .map_err(|_| RnsNativeQpcsFriCompleteErrorV1::InvalidContext)
    }

    #[cfg(test)]
    pub(super) fn test_fixture_v1(
        relation_schedule: RnsNativeQpcsRelationScheduleV1,
        expected_qpcs_bound_transcript_state: [u8; DIGEST_BYTES_V1],
        qpcs_transcript: ZkAmsMkheRnsNativeQpcsBoundTranscriptV1,
    ) -> Result<Self, RnsNativeQpcsFriCompleteErrorV1> {
        Self::from_completed_fri_v1(
            relation_schedule,
            expected_qpcs_bound_transcript_state,
            qpcs_transcript,
        )
    }
}

#[allow(
    dead_code,
    reason = "the private RLWE/source verifier will consume these retained bindings; the current composite boundary must remain fail-closed"
)]
impl<'a> RnsNativeQpcsFriCompleteStageV1<'a> {
    pub(super) const fn has_relation_schedule_v1(&self) -> bool {
        self.relation_schedule.is_some()
    }

    pub(super) fn take_relation_schedule_v1(
        &mut self,
    ) -> Result<RnsNativeQpcsRelationScheduleV1, RnsNativeQpcsFriCompleteErrorV1> {
        self.relation_schedule
            .take()
            .ok_or(RnsNativeQpcsFriCompleteErrorV1::InvalidOrder)
    }

    /// Consume the retained one-shot schedule into a joint owner with the
    /// matching exact qPCS-bound transcript. This transition is the only
    /// production constructor for `RnsNativeQpcsCompletedLineageV1`.
    pub(super) fn take_completed_qpcs_lineage_v1(
        &mut self,
        qpcs_transcript: ZkAmsMkheRnsNativeQpcsBoundTranscriptV1,
    ) -> Result<RnsNativeQpcsCompletedLineageV1, RnsNativeQpcsFriCompleteErrorV1> {
        let expected_qpcs_bound_transcript_state = self.qpcs_bound_transcript_state;
        let relation_schedule = self.take_relation_schedule_v1()?;
        RnsNativeQpcsCompletedLineageV1::from_completed_fri_v1(
            relation_schedule,
            expected_qpcs_bound_transcript_state,
            qpcs_transcript,
        )
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

    pub(super) const fn schedule_digest(&self) -> [u8; DIGEST_BYTES_V1] {
        self.schedule_digest
    }

    pub(super) const fn evaluations(&self) -> &'a [u8] {
        self.evaluations
    }

    pub(super) const fn evaluation_binding_digest(&self) -> [u8; DIGEST_BYTES_V1] {
        self.evaluation_binding_digest
    }

    pub(super) const fn residual_digest(&self) -> [u8; DIGEST_BYTES_V1] {
        self.residual_digest
    }

    pub(super) const fn rlwe_source_residual(&self) -> &'a [u8] {
        self.rlwe_source_residual
    }
}

/// Consume the authenticated fold-zero stage and complete correlated FRI.
pub(super) fn authenticate_rns_native_qpcs_fri_complete_v1<'a>(
    transcript: &ZkAmsMkheRnsNativeChallengeSeedsV1,
    equation_commitment_digests: &[[u8; DIGEST_BYTES_V1]],
    limb_commitment_digests: &[[u8; DIGEST_BYTES_V1]],
    query_opening_digests: &[[u8; DIGEST_BYTES_V1]],
    proof: &'a [u8],
) -> Result<RnsNativeQpcsFriCompleteStageV1<'a>, RnsNativeQpcsFriCompleteErrorV1> {
    let prefix = authenticate_rns_native_qpcs_prefix_v1(
        transcript,
        equation_commitment_digests,
        limb_commitment_digests,
        query_opening_digests,
        proof,
    )
    .map_err(|_| RnsNativeQpcsFriCompleteErrorV1::InvalidPrefix)?;
    authenticate_fri_after_fold_zero_v1(transcript, prefix)
}

/// Complete qPCS while preserving the exact pre-terminal relation schedule.
#[allow(
    dead_code,
    reason = "the typed qPCS/cross-field orchestration adapter is not declared yet"
)]
pub(super) fn authenticate_rns_native_qpcs_fri_complete_with_schedule_v1<'a>(
    transcript: &ZkAmsMkheRnsNativeChallengeSeedsV1,
    relation_schedule: RnsNativeQpcsRelationScheduleV1,
    equation_commitment_digests: &[[u8; DIGEST_BYTES_V1]],
    limb_commitment_digests: &[[u8; DIGEST_BYTES_V1]],
    query_opening_digests: &[[u8; DIGEST_BYTES_V1]],
    proof: &'a [u8],
) -> Result<RnsNativeQpcsFriCompleteStageV1<'a>, RnsNativeQpcsFriCompleteErrorV1> {
    let prefix = authenticate_rns_native_qpcs_prefix_with_schedule_v1(
        transcript,
        relation_schedule,
        equation_commitment_digests,
        limb_commitment_digests,
        query_opening_digests,
        proof,
    )
    .map_err(|_| RnsNativeQpcsFriCompleteErrorV1::InvalidPrefix)?;
    authenticate_fri_after_fold_zero_v1(transcript, prefix)
}

fn authenticate_fri_after_fold_zero_v1<'a>(
    transcript: &ZkAmsMkheRnsNativeChallengeSeedsV1,
    mut prefix: RnsNativeQpcsFoldZeroStageV1<'a>,
) -> Result<RnsNativeQpcsFriCompleteStageV1<'a>, RnsNativeQpcsFriCompleteErrorV1> {
    let context = FriClosureContextV1::from_transcript_v1(transcript, &prefix)?;
    let relation_schedule = prefix
        .take_relation_schedule_v1()
        .map_err(|_| RnsNativeQpcsFriCompleteErrorV1::InvalidPrefix)?;
    verify_closure_parts_with_retained_evaluations_v1(
        context,
        relation_schedule,
        prefix.queries(),
        prefix.fri_one_indices(),
        prefix.fri_one_values(),
        prefix.evaluations(),
        prefix.evaluation_binding_digest(),
        prefix.residual(),
    )
}

#[allow(
    clippy::too_many_arguments,
    reason = "FRI closure verification keeps each authenticated proof section explicit"
)]
fn verify_closure_parts_with_retained_evaluations_v1<'a>(
    context: FriClosureContextV1,
    relation_schedule: RnsNativeQpcsRelationScheduleV1,
    queries: &[u32; QUERY_COUNT_V1],
    fri_one_indices: IndexSetV1,
    fri_one_values: &[u8],
    evaluations: &'a [u8],
    evaluation_binding_digest: [u8; DIGEST_BYTES_V1],
    closure: &'a [u8],
) -> Result<RnsNativeQpcsFriCompleteStageV1<'a>, RnsNativeQpcsFriCompleteErrorV1> {
    let shape = closure_shape_v1(queries)?;
    let expected_fri_one = query_pair_indices_v1(queries, DOMAIN_SIZE_V1 / 2)
        .map_err(|_| RnsNativeQpcsFriCompleteErrorV1::InvalidPrefix)?;
    if fri_one_indices.values[..fri_one_indices.len]
        != expected_fri_one.values[..expected_fri_one.len]
        || fri_one_values.len() != fri_one_indices.len * LEAF_BYTES_V1
        || evaluations.len() != ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1 * 5 * 2 * 8
        || evaluation_binding_digest == [0; DIGEST_BYTES_V1]
    {
        return Err(RnsNativeQpcsFriCompleteErrorV1::InvalidPrefix);
    }
    let view = decode_closure_exact_v1(closure, context, shape)?;
    for ordinal in 0..ENCODED_LAYER_COUNT_V1 {
        let layer = FIRST_ENCODED_LAYER_V1 + ordinal;
        let length = DOMAIN_SIZE_V1 >> layer;
        validate_leaf_values_v1(view.layers[ordinal].values, shape.indices[ordinal].len).map_err(
            |error| match error {
                super::rns_native_qpcs_prefix::RnsNativeQpcsPrefixErrorV1::NonCanonicalResidue => {
                    RnsNativeQpcsFriCompleteErrorV1::NonCanonicalResidue
                }
                _ => RnsNativeQpcsFriCompleteErrorV1::InvalidCount,
            },
        )?;
        authenticate_tree_v1(
            view.layers[ordinal],
            shape.indices[ordinal],
            length,
            TreeRoleV1::Fri,
            u8::try_from(layer).map_err(|_| RnsNativeQpcsFriCompleteErrorV1::ArithmeticOverflow)?,
            context.parameter_digest,
            context.roots[layer],
        )
        .map_err(|_| RnsNativeQpcsFriCompleteErrorV1::InvalidMerklePath)?;
    }
    let fields = derive_fields_v1().map_err(|_| RnsNativeQpcsFriCompleteErrorV1::InvalidContext)?;
    let mut current_indices = fri_one_indices;
    let mut current_values = fri_one_values;
    for layer in 1..LAST_LAYER_V1 {
        let next = layer + 1 - FIRST_ENCODED_LAYER_V1;
        verify_fold_v1(
            context,
            &fields,
            queries,
            layer,
            current_indices,
            current_values,
            shape.indices[next],
            view.layers[next].values,
        )?;
        current_indices = shape.indices[next];
        current_values = view.layers[next].values;
    }
    verify_terminal_degree_v1(context, &fields, current_indices, current_values)?;
    let residual_digest = residual_digest_v1(context, view.downstream_residual)?;
    Ok(RnsNativeQpcsFriCompleteStageV1 {
        relation_schedule: Some(relation_schedule),
        qpcs_bound_transcript_state: context.qpcs_bound_transcript_state,
        parameter_digest: context.parameter_digest,
        transcript_digest: context.transcript_digest,
        query_seed: context.query_seed,
        section_binding_digest: context.section_binding_digest,
        schedule_digest: context.schedule_digest,
        evaluations,
        evaluation_binding_digest,
        residual_digest,
        rlwe_source_residual: view.downstream_residual,
    })
}

#[cfg(test)]
fn verify_closure_parts_v1<'a>(
    context: FriClosureContextV1,
    queries: &[u32; QUERY_COUNT_V1],
    fri_one_indices: IndexSetV1,
    fri_one_values: &[u8],
    closure: &'a [u8],
) -> Result<RnsNativeQpcsFriCompleteStageV1<'a>, RnsNativeQpcsFriCompleteErrorV1> {
    static TEST_EVALUATIONS_V1: [u8; ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1 * 5 * 2 * 8] =
        [0; ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1 * 5 * 2 * 8];
    verify_closure_parts_with_retained_evaluations_v1(
        context,
        RnsNativeQpcsRelationScheduleV1::test_fixture_v1(context.parameter_digest),
        queries,
        fri_one_indices,
        fri_one_values,
        &TEST_EVALUATIONS_V1,
        [0x5e; DIGEST_BYTES_V1],
        closure,
    )
}

fn closure_shape_v1(
    queries: &[u32; QUERY_COUNT_V1],
) -> Result<ClosureShapeV1, RnsNativeQpcsFriCompleteErrorV1> {
    let empty_indices = IndexSetV1 {
        values: [0; MAX_OPENED_LEAVES_V1],
        len: 0,
    };
    let empty_descriptor = TreeDescriptorV1 {
        opened: 0,
        authentication: 0,
        values_bytes: 0,
        authentication_bytes: 0,
    };
    let mut indices = [empty_indices; ENCODED_LAYER_COUNT_V1];
    let mut descriptors = [empty_descriptor; ENCODED_LAYER_COUNT_V1];
    let mut aggregate_opened = 0_usize;
    let mut aggregate_authentication = 0_usize;
    let mut aggregate_values_bytes = 0_usize;
    let mut aggregate_authentication_bytes = 0_usize;
    for layer in 0..=LAST_LAYER_V1 {
        let length = DOMAIN_SIZE_V1 >> layer;
        let layer_indices = query_pair_indices_v1(queries, length)
            .map_err(|_| RnsNativeQpcsFriCompleteErrorV1::InvalidCount)?;
        let descriptor = descriptor_for_indices_v1(layer_indices, length)
            .map_err(|_| RnsNativeQpcsFriCompleteErrorV1::InvalidCount)?;
        aggregate_opened = aggregate_opened
            .checked_add(descriptor.opened)
            .ok_or(RnsNativeQpcsFriCompleteErrorV1::ArithmeticOverflow)?;
        aggregate_authentication = aggregate_authentication
            .checked_add(descriptor.authentication)
            .ok_or(RnsNativeQpcsFriCompleteErrorV1::ArithmeticOverflow)?;
        aggregate_values_bytes = aggregate_values_bytes
            .checked_add(descriptor.values_bytes)
            .ok_or(RnsNativeQpcsFriCompleteErrorV1::ArithmeticOverflow)?;
        aggregate_authentication_bytes = aggregate_authentication_bytes
            .checked_add(descriptor.authentication_bytes)
            .ok_or(RnsNativeQpcsFriCompleteErrorV1::ArithmeticOverflow)?;
        if layer >= FIRST_ENCODED_LAYER_V1 {
            let ordinal = layer - FIRST_ENCODED_LAYER_V1;
            indices[ordinal] = layer_indices;
            descriptors[ordinal] = descriptor;
        }
    }
    let aggregate_bytes = aggregate_values_bytes
        .checked_add(aggregate_authentication_bytes)
        .ok_or(RnsNativeQpcsFriCompleteErrorV1::ArithmeticOverflow)?;
    if aggregate_opened > MAX_FRI_OPENED_LEAVES_V1
        || aggregate_authentication > MAX_FRI_AUTHENTICATION_HASHES_V1
        || aggregate_bytes > ZK_AMS_MKHE_RNS_NATIVE_CORRELATED_FRI_MAX_BYTES_V1 as usize
    {
        return Err(RnsNativeQpcsFriCompleteErrorV1::InvalidCount);
    }
    Ok(ClosureShapeV1 {
        indices,
        descriptors,
        aggregate_opened,
        aggregate_authentication,
        aggregate_values_bytes,
        aggregate_authentication_bytes,
    })
}

fn decode_closure_exact_v1<'a>(
    closure: &'a [u8],
    context: FriClosureContextV1,
    shape: ClosureShapeV1,
) -> Result<ClosureViewV1<'a>, RnsNativeQpcsFriCompleteErrorV1> {
    if closure.len() > ZK_AMS_MKHE_RNS_NATIVE_QPCS_MAX_BYTES_V1 as usize {
        return Err(RnsNativeQpcsFriCompleteErrorV1::ProofCapExceeded);
    }
    if closure.len() < CLOSURE_HEADER_BYTES_V1 {
        return Err(RnsNativeQpcsFriCompleteErrorV1::Truncated);
    }
    let mut decoder = DecoderV1::new(closure);
    if decoder.take(CLOSURE_MAGIC_V1.len())? != CLOSURE_MAGIC_V1.as_slice()
        || decoder.u8()? != CLOSURE_VERSION_V1
        || decoder.u8()? != ZK_AMS_MKHE_RNS_NATIVE_LDE_DOMAIN_LOG2_V1
        || usize::from(decoder.u8()?) != ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1
        || usize::from(decoder.u8()?) != ROWS_PER_LIMB_V1
        || decoder.u16()? != ZK_AMS_MKHE_RNS_NATIVE_QUERY_COUNT_V1
        || usize::from(decoder.u8()?) != FIRST_ENCODED_LAYER_V1
        || usize::from(decoder.u8()?) != LAST_LAYER_V1
        || usize::from(decoder.u8()?) != ENCODED_LAYER_COUNT_V1
        || decoder.u8()? != FIRST_CHECKED_FOLD_V1
        || decoder.u8()? != LAST_CHECKED_FOLD_V1
        || decoder.u8()? != TERMINAL_DERIVED_V1
    {
        return Err(RnsNativeQpcsFriCompleteErrorV1::InvalidHeader);
    }
    let aggregate_opened = usize::from(decoder.u16()?);
    let aggregate_authentication = usize::from(decoder.u16()?);
    let aggregate_values_bytes = usize::try_from(decoder.u32()?)
        .map_err(|_| RnsNativeQpcsFriCompleteErrorV1::ArithmeticOverflow)?;
    let aggregate_authentication_bytes = usize::try_from(decoder.u32()?)
        .map_err(|_| RnsNativeQpcsFriCompleteErrorV1::ArithmeticOverflow)?;
    let downstream_bytes = usize::try_from(decoder.u32()?)
        .map_err(|_| RnsNativeQpcsFriCompleteErrorV1::ArithmeticOverflow)?;
    let empty_descriptor = TreeDescriptorV1 {
        opened: 0,
        authentication: 0,
        values_bytes: 0,
        authentication_bytes: 0,
    };
    let mut encoded = [empty_descriptor; ENCODED_LAYER_COUNT_V1];
    for descriptor in &mut encoded {
        descriptor.opened = usize::from(decoder.u16()?);
        descriptor.authentication = usize::from(decoder.u16()?);
        descriptor.values_bytes = usize::try_from(decoder.u32()?)
            .map_err(|_| RnsNativeQpcsFriCompleteErrorV1::ArithmeticOverflow)?;
        descriptor.authentication_bytes = usize::try_from(decoder.u32()?)
            .map_err(|_| RnsNativeQpcsFriCompleteErrorV1::ArithmeticOverflow)?;
    }
    let parameter_digest = decoder.digest()?;
    let transcript_digest = decoder.digest()?;
    let query_seed = decoder.digest()?;
    let section_binding_digest = decoder.digest()?;
    let schedule_digest = decoder.digest()?;
    let encoded_residual_digest = decoder.digest()?;
    if decoder.cursor != CLOSURE_HEADER_BYTES_V1
        || downstream_bytes == 0
        || aggregate_opened != shape.aggregate_opened
        || aggregate_authentication != shape.aggregate_authentication
        || aggregate_values_bytes != shape.aggregate_values_bytes
        || aggregate_authentication_bytes != shape.aggregate_authentication_bytes
        || encoded
            .iter()
            .zip(shape.descriptors)
            .any(|(encoded, expected)| {
                encoded.opened != expected.opened
                    || encoded.authentication != expected.authentication
                    || encoded.values_bytes != expected.values_bytes
                    || encoded.authentication_bytes != expected.authentication_bytes
            })
        || parameter_digest != context.parameter_digest
        || transcript_digest != context.transcript_digest
        || query_seed != context.query_seed
        || section_binding_digest != context.section_binding_digest
        || schedule_digest != context.schedule_digest
    {
        return Err(RnsNativeQpcsFriCompleteErrorV1::InvalidHeader);
    }
    let encoded_tree_bytes = shape
        .descriptors
        .iter()
        .try_fold(0_usize, |total, descriptor| {
            total
                .checked_add(descriptor.values_bytes)
                .and_then(|value| value.checked_add(descriptor.authentication_bytes))
                .ok_or(RnsNativeQpcsFriCompleteErrorV1::ArithmeticOverflow)
        })?;
    let expected_total = CLOSURE_HEADER_BYTES_V1
        .checked_add(encoded_tree_bytes)
        .and_then(|value| value.checked_add(downstream_bytes))
        .ok_or(RnsNativeQpcsFriCompleteErrorV1::ArithmeticOverflow)?;
    if expected_total > ZK_AMS_MKHE_RNS_NATIVE_QPCS_MAX_BYTES_V1 as usize {
        return Err(RnsNativeQpcsFriCompleteErrorV1::ProofCapExceeded);
    }
    if closure.len() < expected_total {
        return Err(RnsNativeQpcsFriCompleteErrorV1::Truncated);
    }
    if closure.len() != expected_total {
        return Err(RnsNativeQpcsFriCompleteErrorV1::TrailingBytes);
    }
    let empty = TreeViewV1 {
        values: &[],
        authentication: &[],
    };
    let mut layers = [empty; ENCODED_LAYER_COUNT_V1];
    for (layer, descriptor) in layers.iter_mut().zip(shape.descriptors) {
        layer.values = decoder.take(descriptor.values_bytes)?;
        layer.authentication = decoder.take(descriptor.authentication_bytes)?;
    }
    let downstream_residual = decoder.take(downstream_bytes)?;
    if decoder.cursor != closure.len()
        || encoded_residual_digest != residual_digest_v1(context, downstream_residual)?
    {
        return Err(RnsNativeQpcsFriCompleteErrorV1::InvalidHeader);
    }
    Ok(ClosureViewV1 {
        layers,
        downstream_residual,
    })
}

#[allow(
    clippy::too_many_arguments,
    reason = "the fixed FRI layer relation has explicit authenticated inputs"
)]
fn verify_fold_v1(
    context: FriClosureContextV1,
    fields: &[Fq2ParametersV1; ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1],
    queries: &[u32; QUERY_COUNT_V1],
    layer: usize,
    current_indices: IndexSetV1,
    current_values: &[u8],
    next_indices: IndexSetV1,
    next_values: &[u8],
) -> Result<(), RnsNativeQpcsFriCompleteErrorV1> {
    if !(1..LAST_LAYER_V1).contains(&layer) {
        return Err(RnsNativeQpcsFriCompleteErrorV1::InvalidOrder);
    }
    let length = DOMAIN_SIZE_V1 >> layer;
    let half = u32::try_from(length / 2)
        .map_err(|_| RnsNativeQpcsFriCompleteErrorV1::ArithmeticOverflow)?;
    for (limb, &field) in fields.iter().enumerate() {
        let layer_root = field.pow(field.domain_root, 1_u128 << layer);
        let mut alphas = [Fq2V1::ZERO; ROWS_PER_LIMB_V1];
        for (row, alpha) in alphas.iter_mut().enumerate() {
            *alpha = derive_fold_challenge_v1(context, layer, limb, row, field.modulus)?;
        }
        for &query in queries {
            let base = query % half;
            let inverse_exponent = if base == 0 {
                0
            } else {
                u128::try_from(length)
                    .map_err(|_| RnsNativeQpcsFriCompleteErrorV1::ArithmeticOverflow)?
                    - u128::from(base)
            };
            let inverse_x = field.pow(layer_root, inverse_exponent);
            for (row, &alpha) in alphas.iter().enumerate() {
                let coordinate = limb * ROWS_PER_LIMB_V1 + row;
                let positive = read_value_v1(current_indices, current_values, base, coordinate)
                    .map_err(|_| RnsNativeQpcsFriCompleteErrorV1::InvalidOrder)?;
                let negative =
                    read_value_v1(current_indices, current_values, base + half, coordinate)
                        .map_err(|_| RnsNativeQpcsFriCompleteErrorV1::InvalidOrder)?;
                // The fold output remains at `base` in the half-sized layer;
                // `next_indices` contains both lower/upper query-pair members.
                let next = read_value_v1(next_indices, next_values, base, coordinate)
                    .map_err(|_| RnsNativeQpcsFriCompleteErrorV1::InvalidOrder)?;
                if next != fold_value_with_inverse_x_v1(field, inverse_x, positive, negative, alpha)
                {
                    return Err(RnsNativeQpcsFriCompleteErrorV1::InvalidFriEquation);
                }
            }
        }
    }
    Ok(())
}

fn verify_terminal_degree_v1(
    context: FriClosureContextV1,
    fields: &[Fq2ParametersV1; ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1],
    final_indices: IndexSetV1,
    final_values: &[u8],
) -> Result<(), RnsNativeQpcsFriCompleteErrorV1> {
    if final_indices.len != 4 || final_indices.values[..4] != [0, 1, 2, 3] {
        return Err(RnsNativeQpcsFriCompleteErrorV1::InvalidTerminalCoverage);
    }
    for (limb, &field) in fields.iter().enumerate() {
        let layer_root = field.pow(field.domain_root, 1_u128 << LAST_LAYER_V1);
        let inverse_layer_root = field.pow(layer_root, 3);
        let mut alphas = [Fq2V1::ZERO; ROWS_PER_LIMB_V1];
        for (row, alpha) in alphas.iter_mut().enumerate() {
            *alpha = derive_fold_challenge_v1(context, LAST_LAYER_V1, limb, row, field.modulus)?;
        }
        for (row, &alpha) in alphas.iter().enumerate() {
            let coordinate = limb * ROWS_PER_LIMB_V1 + row;
            let value = |index| {
                read_value_v1(final_indices, final_values, index, coordinate)
                    .map_err(|_| RnsNativeQpcsFriCompleteErrorV1::InvalidTerminalCoverage)
            };
            let terminal_zero =
                fold_value_with_inverse_x_v1(field, Fq2V1::ONE, value(0)?, value(2)?, alpha);
            let terminal_one = fold_value_with_inverse_x_v1(
                field,
                inverse_layer_root,
                value(1)?,
                value(3)?,
                alpha,
            );
            if terminal_zero != terminal_one {
                return Err(RnsNativeQpcsFriCompleteErrorV1::InvalidTerminalDegree);
            }
        }
    }
    Ok(())
}

fn derive_fold_challenge_v1(
    context: FriClosureContextV1,
    layer: usize,
    limb: usize,
    row: usize,
    modulus: u64,
) -> Result<Fq2V1, RnsNativeQpcsFriCompleteErrorV1> {
    if layer >= context.fold_seeds.len()
        || limb >= ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1
        || row >= ROWS_PER_LIMB_V1
    {
        return Err(RnsNativeQpcsFriCompleteErrorV1::InvalidChallenge);
    }
    let zone = u64::MAX - u64::MAX % modulus;
    for attempt in 0..MAX_CHALLENGE_ATTEMPTS_V1 {
        let candidate = |half| {
            let mut hash = Keccak256::new();
            hash.update(FOLD_CHALLENGE_DOMAIN_V1);
            hash.update(&[CLOSURE_VERSION_V1]);
            hash.update(&context.parameter_digest);
            // The transcript seed was derived immediately after this layer's
            // root. Do not mix later roots or terminal commitments back into
            // the fold challenge and thereby permit post-challenge grinding.
            hash.update(&context.fold_seeds[layer]);
            hash.update(&[
                u8::try_from(layer)
                    .map_err(|_| RnsNativeQpcsFriCompleteErrorV1::ArithmeticOverflow)?,
                u8::try_from(limb)
                    .map_err(|_| RnsNativeQpcsFriCompleteErrorV1::ArithmeticOverflow)?,
                u8::try_from(row)
                    .map_err(|_| RnsNativeQpcsFriCompleteErrorV1::ArithmeticOverflow)?,
                half,
            ]);
            hash.update(&modulus.to_be_bytes());
            hash.update(&attempt.to_be_bytes());
            let digest = hash.finalize();
            Ok::<u64, RnsNativeQpcsFriCompleteErrorV1>(u64::from_be_bytes(
                digest[..8]
                    .try_into()
                    .map_err(|_| RnsNativeQpcsFriCompleteErrorV1::InvalidChallenge)?,
            ))
        };
        let c0 = candidate(0)?;
        let c1 = candidate(1)?;
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
    Err(RnsNativeQpcsFriCompleteErrorV1::InvalidChallenge)
}

fn schedule_digest_v1(
    context: FriClosureContextV1,
) -> Result<[u8; DIGEST_BYTES_V1], RnsNativeQpcsFriCompleteErrorV1> {
    let mut hash = Keccak256::new();
    hash.update(SCHEDULE_DOMAIN_V1);
    hash.update(&[CLOSURE_VERSION_V1, ZK_AMS_MKHE_RNS_NATIVE_FRI_ROUNDS_V1]);
    for digest in [
        context.parameter_digest,
        context.transcript_digest,
        context.qpcs_bound_transcript_state,
        context.query_seed,
        context.section_binding_digest,
    ] {
        hash.update(&digest);
    }
    for layer in 0..context.roots.len() {
        hash.update(&[
            u8::try_from(layer).map_err(|_| RnsNativeQpcsFriCompleteErrorV1::ArithmeticOverflow)?
        ]);
        hash.update(&context.roots[layer]);
        hash.update(&context.fold_seeds[layer]);
    }
    let digest = hash.finalize();
    if digest == [0; DIGEST_BYTES_V1] {
        return Err(RnsNativeQpcsFriCompleteErrorV1::InvalidContext);
    }
    Ok(digest)
}

fn residual_digest_v1(
    context: FriClosureContextV1,
    residual: &[u8],
) -> Result<[u8; DIGEST_BYTES_V1], RnsNativeQpcsFriCompleteErrorV1> {
    let mut hash = Keccak256::new();
    hash.update(RESIDUAL_DOMAIN_V1);
    hash.update(&[CLOSURE_VERSION_V1]);
    for digest in [
        context.parameter_digest,
        context.transcript_digest,
        context.query_seed,
        context.section_binding_digest,
        context.schedule_digest,
    ] {
        hash.update(&digest);
    }
    hash.update(
        &u32::try_from(residual.len())
            .map_err(|_| RnsNativeQpcsFriCompleteErrorV1::ArithmeticOverflow)?
            .to_be_bytes(),
    );
    hash.update(residual);
    let digest = hash.finalize();
    if digest == [0; DIGEST_BYTES_V1] {
        return Err(RnsNativeQpcsFriCompleteErrorV1::InvalidContext);
    }
    Ok(digest)
}

#[cfg(test)]
#[path = "rns_native_qpcs_fri_complete_tests.rs"]
mod tests;
