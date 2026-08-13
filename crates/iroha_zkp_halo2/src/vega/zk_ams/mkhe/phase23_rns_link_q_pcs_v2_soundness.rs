//! Fixed-ten-row qPCS V2 transcript and relation-soundness prerequisite.
//!
//! This bounded, non-authorizing child shares one exact challenge schedule
//! between prover and verifier, checks canonical relations and multiproofs,
//! and remains production-uninhabited behind uninhabited replay authorities.
use super::{
    Fq2ParametersV1 as BatchFieldV2, Fq2V1 as BatchValueV2, RELEASE_MODULI_V1, mod_add_v1,
    mod_mul_v1, mod_pow_v1,
};
use crate::vega::sponge::{Shake256Reader, keccak256};
use core::{convert::Infallible, fmt};
const VERSION_V2: u8 = 2;
const LIMBS_V2: usize = 38;
const REPETITIONS_V2: usize = 5;
const ROWS_PER_REPETITION_V2: usize = 2;
const ROWS_PER_LIMB_V2: usize = 10;
const RELATION_COUNT_V2: usize = LIMBS_V2 * REPETITIONS_V2;
const COORDINATE_COUNT_V2: usize = LIMBS_V2 * ROWS_PER_LIMB_V2;
const BATCH_CHALLENGE_COUNT_V2: usize = COORDINATE_COUNT_V2 * 2;
const LOG_N_V2: u8 = 17;
const N_V2: u64 = 1 << LOG_N_V2;
const DOMAIN_LOG_V2: u8 = 19;
const DOMAIN_SIZE_V2: usize = 1 << DOMAIN_LOG_V2;
const QUERY_COUNT_V2: usize = 160;
const FRI_ROUNDS_V2: usize = 18;
const FQ2_BYTES_V2: usize = 16;
const LEAF_BYTES_V2: usize = COORDINATE_COUNT_V2 * FQ2_BYTES_V2;
const HEADER_BYTES_V2: usize = 512;
const EVALUATION_BYTES_V2: usize = RELATION_COUNT_V2 * 2 * 8;
const QUOTIENT_ROOT_BYTES_V2: usize = 32;
const FRI_ROOT_BYTES_V2: usize = FRI_ROUNDS_V2 * 32;
const TERMINAL_BYTES_V2: usize = 2 * LEAF_BYTES_V2;
const SECTION_COUNT_V2: usize = FRI_ROUNDS_V2 + 2;
const SECTION_HEADER_BYTES_V2: usize = 8;
const FIXED_BEFORE_SECTIONS_V2: usize = HEADER_BYTES_V2
    + EVALUATION_BYTES_V2
    + QUOTIENT_ROOT_BYTES_V2
    + FRI_ROOT_BYTES_V2
    + TERMINAL_BYTES_V2;
const MAX_INITIAL_OPENED_LEAVES_V2: usize = 320;
const MAX_INITIAL_AUTH_HASHES_PER_TREE_V2: usize = 3_392;
const MAX_FRI_OPENED_LEAVES_V2: usize = 4_028;
const MAX_FRI_AUTH_HASHES_V2: usize = 20_030;
const MAX_MULTIPROOF_VALUE_BYTES_V2: usize =
    (2 * MAX_INITIAL_OPENED_LEAVES_V2 + MAX_FRI_OPENED_LEAVES_V2) * LEAF_BYTES_V2;
const MAX_MULTIPROOF_AUTH_BYTES_V2: usize =
    (2 * MAX_INITIAL_AUTH_HASHES_PER_TREE_V2 + MAX_FRI_AUTH_HASHES_V2) * 32;
const MAX_INITIAL_MULTIPROOF_BYTES_V2: usize =
    2 * (MAX_INITIAL_OPENED_LEAVES_V2 * LEAF_BYTES_V2 + MAX_INITIAL_AUTH_HASHES_PER_TREE_V2 * 32);
// This is the correlated FRI value-plus-authentication maximum; the two
// standalone maxima above are not simultaneously attainable.
const MAX_FRI_MULTIPROOF_BYTES_V2: usize = 25_121_024;
const MAX_MULTIPROOF_SECTION_BYTES_V2: usize =
    MAX_INITIAL_MULTIPROOF_BYTES_V2 + MAX_FRI_MULTIPROOF_BYTES_V2;
const FIXED_ENVELOPE_BYTES_V2: usize =
    FIXED_BEFORE_SECTIONS_V2 + SECTION_COUNT_V2 * SECTION_HEADER_BYTES_V2;
const MAX_PROOF_BYTES_V2: usize = 29_245_792;
const GLOBAL_PROOF_CAP_BYTES_V2: usize = 32 * 1024 * 1024;
const MAGIC_V2: [u8; 16] = *b"IROHA-QPCSV2\0\0\0\0";
const PARAMETER_DOMAIN_V2: &[u8] = b"iroha.zk-ams.v2.q-pcs.soundness.parameters\0";
const INITIAL_DOMAIN_V2: &[u8] = b"iroha.zk-ams.v2.q-pcs.soundness.initial-root\0";
const POINT_DOMAIN_V2: &[u8] = b"iroha.zk-ams.v2.q-pcs.soundness.relation-point\0";
const EVALUATION_DOMAIN_V2: &[u8] = b"iroha.zk-ams.v2.q-pcs.soundness.evaluations\0";
const QUOTIENT_DOMAIN_V2: &[u8] = b"iroha.zk-ams.v2.q-pcs.soundness.quotient-root\0";
const BATCH_DOMAIN_V2: &[u8] = b"iroha.zk-ams.v2.q-pcs.soundness.ten-row-batch\0";
const FRI_ROOT_DOMAIN_V2: &[u8] = b"iroha.zk-ams.v2.q-pcs.soundness.fri-root\0";
const FOLD_DOMAIN_V2: &[u8] = b"iroha.zk-ams.v2.q-pcs.soundness.ten-row-fold\0";
const TERMINAL_DOMAIN_V2: &[u8] = b"iroha.zk-ams.v2.q-pcs.soundness.fri-terminal\0";
const QUERY_DOMAIN_V2: &[u8] = b"iroha.zk-ams.v2.q-pcs.soundness.query\0";
const SCHEDULE_DOMAIN_V2: &[u8] = b"iroha.zk-ams.v2.q-pcs.soundness.schedule\0";
const FIXED_WIDTH_TAG_V2: &[u8] = b"P:2N/c[2N-1]=0;H:N/c[N-1]=0";
const ROW_ORDER_TAG_V2: &[u8] = b"column=limb*10+repetition*2+role;P:0;H:1";
const BATCH_FORMULA_TAG_V2: &[u8] = b"Bp=aP+bXUP;Bh=aX^NH+bX^(N+1)UH";
const SOURCE_AGGREGATION_LINKED_V2: bool = false;
const CROSS_SET_ALGEBRA_VERIFIED_V2: bool = false;
const HYRAX_LINKED_V2: bool = false;
const PRODUCTION_SAMPLER_QUALIFIED_V2: bool = false;
const ZERO_KNOWLEDGE_THEOREM_INSTANTIATED_V2: bool = false;
const AUTHENTICATED_MULTIPASS_REPLAY_INTEGRATED_V2: bool = false;
const COEFFICIENT_TOP_ZERO_REPLAY_VERIFIED_V2: bool = false;
const TEN_ROW_MERKLE_PATHS_VERIFIED_V2: bool = true;
const OPENING_QUOTIENT_EQUATIONS_VERIFIED_V2: bool = true;
const TEN_ROW_BATCHING_EQUATIONS_VERIFIED_V2: bool = true;
const TEN_ROW_FRI_EQUATIONS_VERIFIED_V2: bool = true;
const COMPLETE_WORK_BOUND_DERIVED_V2: bool = false;
const MEASURED_RSS_WITHIN_CAP_V2: bool = false;
const OPERATIONAL_RECEIPT_ACCEPTED_V2: bool = false;
const RELEASE_READY_V2: bool = false;
const _: () = {
    assert!(RELEASE_MODULI_V1.len() == LIMBS_V2);
    assert!(ROWS_PER_LIMB_V2 == REPETITIONS_V2 * ROWS_PER_REPETITION_V2);
    assert!(BATCH_CHALLENGE_COUNT_V2 == 760);
    assert!(LEAF_BYTES_V2 == 6_080);
    assert!(EVALUATION_BYTES_V2 == 3_040);
    assert!(FIXED_BEFORE_SECTIONS_V2 == 16_320);
    assert!(MAX_MULTIPROOF_VALUE_BYTES_V2 == 28_381_440);
    assert!(MAX_MULTIPROOF_AUTH_BYTES_V2 == 858_048);
    assert!(MAX_INITIAL_MULTIPROOF_BYTES_V2 == 4_108_288);
    assert!(MAX_FRI_MULTIPROOF_BYTES_V2 == 25_121_024);
    assert!(MAX_MULTIPROOF_SECTION_BYTES_V2 == 29_229_312);
    assert!(FIXED_ENVELOPE_BYTES_V2 == 16_480);
    assert!(MAX_PROOF_BYTES_V2 == MAX_MULTIPROOF_SECTION_BYTES_V2 + FIXED_ENVELOPE_BYTES_V2);
    assert!(GLOBAL_PROOF_CAP_BYTES_V2 - MAX_PROOF_BYTES_V2 == 4_308_640);
    assert!(MAX_PROOF_BYTES_V2 < GLOBAL_PROOF_CAP_BYTES_V2);
};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SoundnessErrorV2 {
    Poisoned,
    InvalidHeader,
    InvalidPublicContext,
    InvalidParameterDigest,
    InvalidRoot,
    InvalidChallenge,
    NonCanonicalResidue,
    RelationMismatch,
    InvalidMerklePath,
    InvalidOpeningQuotient,
    InvalidBatchEquation,
    InvalidFriEquation,
    InvalidTerminal,
    InvalidSectionCount,
    Truncated,
    TrailingBytes,
    ProofCapExceeded,
    ArithmeticOverflow,
}
impl fmt::Display for SoundnessErrorV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}
struct FrameV2<const N: usize> {
    bytes: [u8; N],
    len: usize,
}
impl<const N: usize> FrameV2<N> {
    const fn new() -> Self {
        Self {
            bytes: [0; N],
            len: 0,
        }
    }
    fn push(&mut self, bytes: &[u8]) -> Result<(), SoundnessErrorV2> {
        let end = self
            .len
            .checked_add(bytes.len())
            .ok_or(SoundnessErrorV2::ArithmeticOverflow)?;
        let destination = self
            .bytes
            .get_mut(self.len..end)
            .ok_or(SoundnessErrorV2::ArithmeticOverflow)?;
        destination.copy_from_slice(bytes);
        self.len = end;
        Ok(())
    }
    fn bytes(&self) -> &[u8] {
        &self.bytes[..self.len]
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
struct Fq2V2 {
    c0: u64,
    c1: u64,
}
impl Fq2V2 {
    const ZERO: Self = Self { c0: 0, c1: 0 };
    fn encode(self) -> [u8; FQ2_BYTES_V2] {
        let mut encoded = [0_u8; FQ2_BYTES_V2];
        encoded[..8].copy_from_slice(&self.c0.to_be_bytes());
        encoded[8..].copy_from_slice(&self.c1.to_be_bytes());
        encoded
    }
}
#[derive(Clone, Copy)]
struct ExpectedPublicContextV2 {
    sealed_source_transcript_digest: [u8; 32],
    source_algebra_binding_digest: [u8; 32],
}
enum SourceReplaySealV2 {
    Production {
        source_and_algebra: Infallible,
        authenticated_multipass_replay: Infallible,
    },
    #[cfg(test)]
    TestOnly,
}
#[derive(Clone, Copy)]
struct HeaderV2 {
    parameter_digest: [u8; 32],
    sealed_source_transcript_digest: [u8; 32],
    source_algebra_binding_digest: [u8; 32],
    initial_root: [u8; 32],
}
struct ProverPostRootLiveV2 {
    header: HeaderV2,
    transcript: [u8; 32],
    evaluation_transcript: [u8; 32],
    relation_points: [u64; RELATION_COUNT_V2],
    quotient_root: [u8; 32],
    batch_schedule_digest: [u8; 32],
    batch_challenges: [Fq2V2; BATCH_CHALLENGE_COUNT_V2],
}
pub(super) struct ProverPostRootPointsV2 {
    live: Option<ProverPostRootLiveV2>,
}
pub(super) struct ProverEvaluationsBoundV2 {
    live: Option<ProverPostRootLiveV2>,
}
pub(super) struct ProverQuotientRootBoundV2 {
    live: Option<ProverPostRootLiveV2>,
}
pub(super) struct ProverBatchChallengesV2 {
    live: Option<ProverPostRootLiveV2>,
    fields: [BatchFieldV2; LIMBS_V2],
    next_block: u64,
    next_column: u16,
}
pub(super) struct ProverBatchRowsCompleteV2 {
    transcript: [u8; 32],
    batch_schedule_digest: [u8; 32],
}
struct ProverFriLayer0LiveV2 {
    pre_layer_transcript: [u8; 32],
    transcript: [u8; 32],
    batch_schedule_digest: [u8; 32],
    fold_schedule_digest: [u8; 32],
    layer0_root: [u8; 32],
    alphas: [BatchValueV2; COORDINATE_COUNT_V2],
}
pub(super) struct ProverFriLayer0ChallengesV2 {
    live: Option<ProverFriLayer0LiveV2>,
    fields: [BatchFieldV2; LIMBS_V2],
    inverse_domain_roots: [BatchValueV2; LIMBS_V2],
    next_pair_block: u64,
    next_column: u16,
}
pub(super) struct ProverFriLayer0FoldCompleteV2 {
    pre_layer_transcript: [u8; 32],
    transcript: [u8; 32],
    batch_schedule_digest: [u8; 32],
    fold_schedule_digest: [u8; 32],
    layer0_root: [u8; 32],
}
struct LiveProtocolV2<'a> {
    wire: &'a [u8],
    header: HeaderV2,
    source_replay_seal: SourceReplaySealV2,
    offset: usize,
    transcript: [u8; 32],
    relation_points: [u64; RELATION_COUNT_V2],
    batch_schedule_digest: [u8; 32],
    fold_schedule_digest: [u8; 32],
    queries: [u32; QUERY_COUNT_V2],
}
struct HeaderParsedV2<'a> {
    live: Option<LiveProtocolV2<'a>>,
}
struct PointsDerivedV2<'a> {
    live: Option<LiveProtocolV2<'a>>,
}
struct RelationsCheckedV2<'a> {
    live: Option<LiveProtocolV2<'a>>,
}
struct QuotientRootBoundV2<'a> {
    live: Option<LiveProtocolV2<'a>>,
}
struct FriTranscriptBoundV2<'a> {
    live: Option<LiveProtocolV2<'a>>,
}
struct StructurallyParsedV2<'a> {
    live: Option<LiveProtocolV2<'a>>,
}
fn shake256_fixed_v2<const N: usize>(input: &[u8]) -> [u8; N] {
    let mut output = [0_u8; N];
    Shake256Reader::new(input).read(&mut output);
    output
}
fn parameter_digest_v2() -> Result<[u8; 32], SoundnessErrorV2> {
    let mut frame = FrameV2::<640>::new();
    frame.push(PARAMETER_DOMAIN_V2)?;
    frame.push(&[VERSION_V2, LOG_N_V2, DOMAIN_LOG_V2])?;
    frame.push(&(N_V2 as u32).to_be_bytes())?;
    frame.push(&(DOMAIN_SIZE_V2 as u32).to_be_bytes())?;
    frame.push(&(QUERY_COUNT_V2 as u16).to_be_bytes())?;
    frame.push(&[
        LIMBS_V2 as u8,
        REPETITIONS_V2 as u8,
        ROWS_PER_LIMB_V2 as u8,
        FRI_ROUNDS_V2 as u8,
    ])?;
    frame.push(FIXED_WIDTH_TAG_V2)?;
    frame.push(ROW_ORDER_TAG_V2)?;
    frame.push(BATCH_FORMULA_TAG_V2)?;
    for (limb, modulus) in RELEASE_MODULI_V1.iter().copied().enumerate() {
        frame.push(&[limb as u8])?;
        frame.push(&modulus.to_be_bytes())?;
    }
    Ok(keccak256(frame.bytes()))
}
#[cfg(test)]
pub(super) fn parameter_digest_for_spool_parity_v2() -> [u8; 32] {
    parameter_digest_v2().expect("fixed release qPCS V2 parameter digest")
}
#[cfg(test)]
pub(super) fn initial_leaf_hash_for_prover_parity_v2(
    parameter_digest: [u8; 32],
    length: usize,
    values: &[u8],
) -> [u8; 32] {
    verifier::initial_leaf_hash_for_prover_parity_v2(parameter_digest, length, values)
        .expect("valid initial C0 Merkle leaf parity frame")
}
#[cfg(test)]
pub(super) fn initial_node_hash_for_prover_parity_v2(
    parameter_digest: [u8; 32],
    height: usize,
    left: [u8; 32],
    right: [u8; 32],
) -> [u8; 32] {
    verifier::initial_node_hash_for_prover_parity_v2(parameter_digest, height, left, right)
        .expect("valid initial C0 Merkle node parity frame")
}
#[cfg(test)]
pub(super) fn quotient_leaf_hash_for_prover_parity_v2(
    parameter_digest: [u8; 32],
    length: usize,
    values: &[u8],
) -> [u8; 32] {
    verifier::quotient_leaf_hash_for_prover_parity_v2(parameter_digest, length, values)
        .expect("valid opening-quotient Merkle leaf parity frame")
}
#[cfg(test)]
pub(super) fn quotient_node_hash_for_prover_parity_v2(
    parameter_digest: [u8; 32],
    height: usize,
    left: [u8; 32],
    right: [u8; 32],
) -> [u8; 32] {
    verifier::quotient_node_hash_for_prover_parity_v2(parameter_digest, height, left, right)
        .expect("valid opening-quotient Merkle node parity frame")
}
fn read_u16_v2(bytes: &[u8], offset: usize) -> Result<u16, SoundnessErrorV2> {
    Ok(u16::from_be_bytes(
        bytes
            .get(offset..offset + 2)
            .ok_or(SoundnessErrorV2::Truncated)?
            .try_into()
            .map_err(|_| SoundnessErrorV2::Truncated)?,
    ))
}
fn read_u32_v2(bytes: &[u8], offset: usize) -> Result<u32, SoundnessErrorV2> {
    Ok(u32::from_be_bytes(
        bytes
            .get(offset..offset + 4)
            .ok_or(SoundnessErrorV2::Truncated)?
            .try_into()
            .map_err(|_| SoundnessErrorV2::Truncated)?,
    ))
}
fn read_u64_v2(bytes: &[u8], offset: usize) -> Result<u64, SoundnessErrorV2> {
    Ok(u64::from_be_bytes(
        bytes
            .get(offset..offset + 8)
            .ok_or(SoundnessErrorV2::Truncated)?
            .try_into()
            .map_err(|_| SoundnessErrorV2::Truncated)?,
    ))
}
fn read_digest_v2(bytes: &[u8], offset: usize) -> Result<[u8; 32], SoundnessErrorV2> {
    bytes
        .get(offset..offset + 32)
        .ok_or(SoundnessErrorV2::Truncated)?
        .try_into()
        .map_err(|_| SoundnessErrorV2::Truncated)
}
fn parse_header_v2(
    wire: &[u8],
    expected: ExpectedPublicContextV2,
) -> Result<HeaderV2, SoundnessErrorV2> {
    if wire.len() > MAX_PROOF_BYTES_V2 || wire.len() > GLOBAL_PROOF_CAP_BYTES_V2 {
        return Err(SoundnessErrorV2::ProofCapExceeded);
    }
    let header = wire
        .get(..HEADER_BYTES_V2)
        .ok_or(SoundnessErrorV2::Truncated)?;
    if header[..16] != MAGIC_V2
        || header[16] != VERSION_V2
        || header[17] != LOG_N_V2
        || header[18] != DOMAIN_LOG_V2
        || header[19] as usize != LIMBS_V2
        || header[20] as usize != REPETITIONS_V2
        || header[21] as usize != ROWS_PER_LIMB_V2
        || header[22] as usize != FRI_ROUNDS_V2
        || header[23] != 2
        || read_u32_v2(header, 24)? != N_V2 as u32
        || read_u32_v2(header, 28)? != DOMAIN_SIZE_V2 as u32
        || read_u16_v2(header, 32)? as usize != QUERY_COUNT_V2
        || read_u16_v2(header, 34)? as usize != MAX_INITIAL_OPENED_LEAVES_V2
        || read_u32_v2(header, 36)? as usize != MAX_FRI_OPENED_LEAVES_V2
        || read_u32_v2(header, 40)? as usize != MAX_INITIAL_AUTH_HASHES_PER_TREE_V2
        || read_u32_v2(header, 44)? as usize != MAX_FRI_AUTH_HASHES_V2
        || read_u32_v2(header, 48)? as usize != LEAF_BYTES_V2
        || read_u32_v2(header, 52)? as usize != FQ2_BYTES_V2
        || read_u64_v2(header, 56)? != MAX_PROOF_BYTES_V2 as u64
        || header[192..].iter().any(|byte| *byte != 0)
    {
        return Err(SoundnessErrorV2::InvalidHeader);
    }
    let parameter_digest = read_digest_v2(header, 64)?;
    if parameter_digest != parameter_digest_v2()? {
        return Err(SoundnessErrorV2::InvalidParameterDigest);
    }
    let sealed_source_transcript_digest = read_digest_v2(header, 96)?;
    let source_algebra_binding_digest = read_digest_v2(header, 128)?;
    if sealed_source_transcript_digest == [0; 32]
        || source_algebra_binding_digest == [0; 32]
        || sealed_source_transcript_digest != expected.sealed_source_transcript_digest
        || source_algebra_binding_digest != expected.source_algebra_binding_digest
    {
        return Err(SoundnessErrorV2::InvalidPublicContext);
    }
    let initial_root = read_digest_v2(header, 160)?;
    if initial_root == [0; 32] {
        return Err(SoundnessErrorV2::InvalidRoot);
    }
    Ok(HeaderV2 {
        parameter_digest,
        sealed_source_transcript_digest,
        source_algebra_binding_digest,
        initial_root,
    })
}
fn begin_v2<'a>(
    wire: &'a [u8],
    expected: ExpectedPublicContextV2,
    source_replay_seal: SourceReplaySealV2,
) -> Result<HeaderParsedV2<'a>, SoundnessErrorV2> {
    let header = parse_header_v2(wire, expected)?;
    Ok(HeaderParsedV2 {
        live: Some(LiveProtocolV2 {
            wire,
            header,
            source_replay_seal,
            offset: HEADER_BYTES_V2,
            transcript: [0; 32],
            relation_points: [0; RELATION_COUNT_V2],
            batch_schedule_digest: [0; 32],
            fold_schedule_digest: [0; 32],
            queries: [0; QUERY_COUNT_V2],
        }),
    })
}
fn initial_transcript_v2(header: HeaderV2) -> Result<[u8; 32], SoundnessErrorV2> {
    let mut frame = FrameV2::<256>::new();
    frame.push(INITIAL_DOMAIN_V2)?;
    frame.push(&[VERSION_V2])?;
    frame.push(&header.parameter_digest)?;
    frame.push(&header.sealed_source_transcript_digest)?;
    frame.push(&header.source_algebra_binding_digest)?;
    frame.push(&header.initial_root)?;
    Ok(keccak256(frame.bytes()))
}
fn derive_relation_point_v2(
    transcript: [u8; 32],
    limb: usize,
    repetition: usize,
    prior: &[u64],
) -> Result<u64, SoundnessErrorV2> {
    let modulus = RELEASE_MODULI_V1[limb];
    let zone = u64::MAX - u64::MAX % modulus;
    for attempt in 0_u32..256 {
        let mut frame = FrameV2::<160>::new();
        frame.push(POINT_DOMAIN_V2)?;
        frame.push(&[VERSION_V2])?;
        frame.push(&transcript)?;
        frame.push(&[limb as u8, repetition as u8])?;
        frame.push(&modulus.to_be_bytes())?;
        frame.push(&attempt.to_be_bytes())?;
        let bytes = shake256_fixed_v2::<8>(frame.bytes());
        let candidate = u64::from_be_bytes(bytes);
        if candidate < zone {
            let point = candidate % modulus;
            if point != 0
                && !prior.contains(&point)
                && mod_add_v1(mod_pow_v1(point, N_V2, modulus), 1, modulus) != 0
                && mod_pow_v1(point, DOMAIN_SIZE_V2 as u64, modulus) != 1
            {
                return Ok(point);
            }
        }
    }
    Err(SoundnessErrorV2::InvalidChallenge)
}
fn derive_all_relation_points_v2(
    header: HeaderV2,
) -> Result<([u8; 32], [u64; RELATION_COUNT_V2]), SoundnessErrorV2> {
    let transcript = initial_transcript_v2(header)?;
    let mut relation_points = [0_u64; RELATION_COUNT_V2];
    for limb in 0..LIMBS_V2 {
        for repetition in 0..REPETITIONS_V2 {
            let coordinate = limb * REPETITIONS_V2 + repetition;
            let prior = &relation_points[limb * REPETITIONS_V2..coordinate];
            relation_points[coordinate] =
                derive_relation_point_v2(transcript, limb, repetition, prior)?;
        }
    }
    Ok((transcript, relation_points))
}
impl<'a> HeaderParsedV2<'a> {
    fn derive_points_v2(&mut self) -> Result<PointsDerivedV2<'a>, SoundnessErrorV2> {
        let mut live = self.live.take().ok_or(SoundnessErrorV2::Poisoned)?;
        (live.transcript, live.relation_points) = derive_all_relation_points_v2(live.header)?;
        Ok(PointsDerivedV2 { live: Some(live) })
    }
}
fn absorb_evaluations_v2(
    transcript: [u8; 32],
    encoded: &[u8],
) -> Result<[u8; 32], SoundnessErrorV2> {
    let mut frame = FrameV2::<3_200>::new();
    frame.push(EVALUATION_DOMAIN_V2)?;
    frame.push(&[VERSION_V2])?;
    frame.push(&transcript)?;
    frame.push(&(RELATION_COUNT_V2 as u16).to_be_bytes())?;
    frame.push(encoded)?;
    Ok(keccak256(frame.bytes()))
}
fn validate_relations_v2(
    relation_points: &[u64; RELATION_COUNT_V2],
    encoded: &[u8],
) -> Result<(), SoundnessErrorV2> {
    if encoded.len() != EVALUATION_BYTES_V2 {
        return Err(SoundnessErrorV2::Truncated);
    }
    for (limb, &modulus) in RELEASE_MODULI_V1.iter().enumerate() {
        for repetition in 0..REPETITIONS_V2 {
            let relation = limb * REPETITIONS_V2 + repetition;
            let offset = relation * 16;
            let product = read_u64_v2(encoded, offset)?;
            let quotient = read_u64_v2(encoded, offset + 8)?;
            if product >= modulus || quotient >= modulus {
                return Err(SoundnessErrorV2::NonCanonicalResidue);
            }
            let point = relation_points[relation];
            let factor = mod_add_v1(mod_pow_v1(point, N_V2, modulus), 1, modulus);
            if product != mod_mul_v1(factor, quotient, modulus) {
                return Err(SoundnessErrorV2::RelationMismatch);
            }
        }
    }
    Ok(())
}
impl<'a> PointsDerivedV2<'a> {
    fn check_relations_v2(&mut self) -> Result<RelationsCheckedV2<'a>, SoundnessErrorV2> {
        let mut live = self.live.take().ok_or(SoundnessErrorV2::Poisoned)?;
        let end = live
            .offset
            .checked_add(EVALUATION_BYTES_V2)
            .ok_or(SoundnessErrorV2::ArithmeticOverflow)?;
        let encoded = live
            .wire
            .get(live.offset..end)
            .ok_or(SoundnessErrorV2::Truncated)?;
        validate_relations_v2(&live.relation_points, encoded)?;
        live.transcript = absorb_evaluations_v2(live.transcript, encoded)?;
        live.offset = end;
        Ok(RelationsCheckedV2 { live: Some(live) })
    }
}
impl ProverPostRootPointsV2 {
    pub(super) fn derive_v2(
        parameter_digest: [u8; 32],
        sealed_source_transcript_digest: [u8; 32],
        source_algebra_binding_digest: [u8; 32],
        initial_root: [u8; 32],
    ) -> Result<Self, SoundnessErrorV2> {
        if parameter_digest != parameter_digest_v2()?
            || sealed_source_transcript_digest == [0; 32]
            || source_algebra_binding_digest == [0; 32]
            || initial_root == [0; 32]
        {
            return Err(SoundnessErrorV2::InvalidPublicContext);
        }
        let header = HeaderV2 {
            parameter_digest,
            sealed_source_transcript_digest,
            source_algebra_binding_digest,
            initial_root,
        };
        let (transcript, relation_points) = derive_all_relation_points_v2(header)?;
        Ok(Self {
            live: Some(ProverPostRootLiveV2 {
                header,
                transcript,
                evaluation_transcript: [0; 32],
                relation_points,
                quotient_root: [0; 32],
                batch_schedule_digest: [0; 32],
                batch_challenges: [Fq2V2::ZERO; BATCH_CHALLENGE_COUNT_V2],
            }),
        })
    }
    pub(super) fn point_v2(&self, limb: usize, repetition: usize) -> Result<u64, SoundnessErrorV2> {
        if limb >= LIMBS_V2 || repetition >= REPETITIONS_V2 {
            return Err(SoundnessErrorV2::InvalidChallenge);
        }
        self.live
            .as_ref()
            .ok_or(SoundnessErrorV2::Poisoned)?
            .relation_points
            .get(limb * REPETITIONS_V2 + repetition)
            .copied()
            .ok_or(SoundnessErrorV2::InvalidChallenge)
    }
    pub(super) fn bind_evaluations_v2(
        mut self,
        encoded: &[u8],
    ) -> Result<ProverEvaluationsBoundV2, SoundnessErrorV2> {
        let mut live = self.live.take().ok_or(SoundnessErrorV2::Poisoned)?;
        validate_relations_v2(&live.relation_points, encoded)?;
        live.transcript = absorb_evaluations_v2(live.transcript, encoded)?;
        Ok(ProverEvaluationsBoundV2 { live: Some(live) })
    }
}
impl ProverEvaluationsBoundV2 {
    pub(super) fn point_v2(&self, limb: usize, repetition: usize) -> Result<u64, SoundnessErrorV2> {
        if limb >= LIMBS_V2 || repetition >= REPETITIONS_V2 {
            return Err(SoundnessErrorV2::InvalidChallenge);
        }
        self.live
            .as_ref()
            .ok_or(SoundnessErrorV2::Poisoned)?
            .relation_points
            .get(limb * REPETITIONS_V2 + repetition)
            .copied()
            .ok_or(SoundnessErrorV2::InvalidChallenge)
    }
    pub(super) fn transcript_v2(&self) -> Result<[u8; 32], SoundnessErrorV2> {
        Ok(self
            .live
            .as_ref()
            .ok_or(SoundnessErrorV2::Poisoned)?
            .transcript)
    }
    pub(super) fn bind_quotient_root_v2(
        mut self,
        root: [u8; 32],
    ) -> Result<ProverQuotientRootBoundV2, SoundnessErrorV2> {
        let mut live = self.live.take().ok_or(SoundnessErrorV2::Poisoned)?;
        if root == [0; 32] {
            return Err(SoundnessErrorV2::InvalidRoot);
        }
        live.evaluation_transcript = live.transcript;
        live.quotient_root = root;
        live.transcript = absorb_root_v2(QUOTIENT_DOMAIN_V2, live.transcript, 0, root)?;
        let mut challenges = [Fq2V2::ZERO; BATCH_CHALLENGE_COUNT_V2];
        live.batch_schedule_digest =
            derive_batch_schedule_with_v2(live.transcript, |index, value| {
                challenges[index] = value;
            })?;
        live.batch_challenges = challenges;
        Ok(ProverQuotientRootBoundV2 { live: Some(live) })
    }
}
impl ProverQuotientRootBoundV2 {
    pub(super) fn point_v2(&self, limb: usize, repetition: usize) -> Result<u64, SoundnessErrorV2> {
        if limb >= LIMBS_V2 || repetition >= REPETITIONS_V2 {
            return Err(SoundnessErrorV2::InvalidChallenge);
        }
        self.live
            .as_ref()
            .ok_or(SoundnessErrorV2::Poisoned)?
            .relation_points
            .get(limb * REPETITIONS_V2 + repetition)
            .copied()
            .ok_or(SoundnessErrorV2::InvalidChallenge)
    }
    pub(super) fn transcript_v2(&self) -> Result<[u8; 32], SoundnessErrorV2> {
        Ok(self
            .live
            .as_ref()
            .ok_or(SoundnessErrorV2::Poisoned)?
            .transcript)
    }
    pub(super) fn quotient_root_v2(&self) -> Result<[u8; 32], SoundnessErrorV2> {
        Ok(self
            .live
            .as_ref()
            .ok_or(SoundnessErrorV2::Poisoned)?
            .quotient_root)
    }
    pub(super) fn begin_batch_challenges_v2(
        mut self,
    ) -> Result<ProverBatchChallengesV2, SoundnessErrorV2> {
        let live = self.live.take().ok_or(SoundnessErrorV2::Poisoned)?;
        let first = BatchFieldV2::derive(RELEASE_MODULI_V1[0], DOMAIN_LOG_V2 as usize)
            .map_err(|_| SoundnessErrorV2::InvalidChallenge)?;
        let mut fields = [first; LIMBS_V2];
        for limb in 1..LIMBS_V2 {
            fields[limb] = BatchFieldV2::derive(RELEASE_MODULI_V1[limb], DOMAIN_LOG_V2 as usize)
                .map_err(|_| SoundnessErrorV2::InvalidChallenge)?;
        }
        Ok(ProverBatchChallengesV2 {
            live: Some(live),
            fields,
            next_block: 0,
            next_column: 0,
        })
    }
}
impl ProverBatchChallengesV2 {
    #[allow(
        clippy::type_complexity,
        reason = "fixed transcript-state tuple preserves reviewed batch-stage ordering"
    )]
    pub(super) fn context_v2(&self) -> Result<([u8; 32], [u8; 32], [u8; 32]), SoundnessErrorV2> {
        let live = self.live.as_ref().ok_or(SoundnessErrorV2::Poisoned)?;
        Ok((
            live.evaluation_transcript,
            live.transcript,
            live.batch_schedule_digest,
        ))
    }
    pub(super) fn mix_next_block_v2(
        &mut self,
        block: u64,
        column: u16,
        committed: &[u8],
        quotient: &[u8],
        output: &mut [u8],
    ) -> Result<(), SoundnessErrorV2> {
        let live = self.live.take().ok_or(SoundnessErrorV2::Poisoned)?;
        if block != self.next_block
            || column != self.next_column
            || block >= 512
            || column >= COORDINATE_COUNT_V2 as u16
            || committed.len() != 1_024 * FQ2_BYTES_V2
            || quotient.len() != committed.len()
            || output.len() != committed.len()
        {
            return Err(SoundnessErrorV2::InvalidBatchEquation);
        }
        let limb = usize::from(column) / ROWS_PER_LIMB_V2;
        let row = usize::from(column) % ROWS_PER_LIMB_V2;
        let field = self.fields[limb];
        let a = live.batch_challenges[usize::from(column) * 2];
        let b = live.batch_challenges[usize::from(column) * 2 + 1];
        let mut x = field.pow(field.domain_root, u128::from(block) * 1_024);
        for ((committed, quotient), output) in committed
            .chunks_exact(16)
            .zip(quotient.chunks_exact(16))
            .zip(output.chunks_exact_mut(16))
        {
            let decode = |value: &[u8]| -> Result<BatchValueV2, SoundnessErrorV2> {
                let c0 = read_u64_v2(value, 0)?;
                let c1 = read_u64_v2(value, 8)?;
                if c0 >= field.modulus || c1 >= field.modulus {
                    return Err(SoundnessErrorV2::NonCanonicalResidue);
                }
                Ok(BatchValueV2 { c0, c1 })
            };
            let c = decode(committed)?;
            let q = decode(quotient)?;
            let x_n = if row.is_multiple_of(2) {
                BatchValueV2::ONE
            } else {
                field.pow(x, N_V2 as u128)
            };
            let value = field.add(
                field.mul(BatchValueV2 { c0: a.c0, c1: a.c1 }, field.mul(x_n, c)),
                field.mul(
                    BatchValueV2 { c0: b.c0, c1: b.c1 },
                    field.mul(field.mul(x_n, x), q),
                ),
            );
            output[..8].copy_from_slice(&value.c0.to_be_bytes());
            output[8..].copy_from_slice(&value.c1.to_be_bytes());
            x = field.mul(x, field.domain_root);
        }
        self.next_column += 1;
        if self.next_column == COORDINATE_COUNT_V2 as u16 {
            self.next_column = 0;
            self.next_block += 1;
        }
        self.live = Some(live);
        Ok(())
    }
    pub(super) fn complete_v2(mut self) -> Result<ProverBatchRowsCompleteV2, SoundnessErrorV2> {
        let mut live = self.live.take().ok_or(SoundnessErrorV2::Poisoned)?;
        if self.next_block != 512 || self.next_column != 0 {
            return Err(SoundnessErrorV2::InvalidBatchEquation);
        }
        live.batch_challenges.fill(Fq2V2::ZERO);
        Ok(ProverBatchRowsCompleteV2 {
            transcript: live.transcript,
            batch_schedule_digest: live.batch_schedule_digest,
        })
    }
}
impl ProverBatchRowsCompleteV2 {
    pub(super) fn bind_fri_layer0_root_v2(
        self,
        root: [u8; 32],
    ) -> Result<ProverFriLayer0ChallengesV2, SoundnessErrorV2> {
        if root == [0; 32] {
            return Err(SoundnessErrorV2::InvalidRoot);
        }
        let transcript = absorb_root_v2(FRI_ROOT_DOMAIN_V2, self.transcript, 0, root)?;
        let mut fold_schedule_digest = self.batch_schedule_digest;
        let mut alphas = [BatchValueV2::ZERO; COORDINATE_COUNT_V2];
        for limb in 0..LIMBS_V2 {
            for row in 0..ROWS_PER_LIMB_V2 {
                let alpha = derive_fq2_challenge_v2(FOLD_DOMAIN_V2, transcript, limb, row, 0, 0)?;
                alphas[limb * ROWS_PER_LIMB_V2 + row] = BatchValueV2 {
                    c0: alpha.c0,
                    c1: alpha.c1,
                };
                fold_schedule_digest =
                    absorb_schedule_value_v2(fold_schedule_digest, 1, limb, row, 0, 0, alpha)?;
            }
        }
        let first = BatchFieldV2::derive(RELEASE_MODULI_V1[0], DOMAIN_LOG_V2 as usize)
            .map_err(|_| SoundnessErrorV2::InvalidChallenge)?;
        let first_inverse_root = first
            .inverse(first.domain_root)
            .map_err(|_| SoundnessErrorV2::InvalidFriEquation)?;
        let mut fields = [first; LIMBS_V2];
        let mut inverse_domain_roots = [first_inverse_root; LIMBS_V2];
        for limb in 1..LIMBS_V2 {
            let field = BatchFieldV2::derive(RELEASE_MODULI_V1[limb], DOMAIN_LOG_V2 as usize)
                .map_err(|_| SoundnessErrorV2::InvalidChallenge)?;
            fields[limb] = field;
            inverse_domain_roots[limb] = field
                .inverse(field.domain_root)
                .map_err(|_| SoundnessErrorV2::InvalidFriEquation)?;
        }
        Ok(ProverFriLayer0ChallengesV2 {
            live: Some(ProverFriLayer0LiveV2 {
                pre_layer_transcript: self.transcript,
                transcript,
                batch_schedule_digest: self.batch_schedule_digest,
                fold_schedule_digest,
                layer0_root: root,
                alphas,
            }),
            fields,
            inverse_domain_roots,
            next_pair_block: 0,
            next_column: 0,
        })
    }
}
impl ProverFriLayer0ChallengesV2 {
    #[allow(
        clippy::type_complexity,
        reason = "fixed transcript-state tuple preserves reviewed FRI-stage ordering"
    )]
    pub(super) fn context_v2(
        &self,
    ) -> Result<([u8; 32], [u8; 32], [u8; 32], [u8; 32], [u8; 32]), SoundnessErrorV2> {
        let live = self.live.as_ref().ok_or(SoundnessErrorV2::Poisoned)?;
        Ok((
            live.pre_layer_transcript,
            live.transcript,
            live.batch_schedule_digest,
            live.fold_schedule_digest,
            live.layer0_root,
        ))
    }
    pub(super) fn fold_next_pair_v2(
        &mut self,
        pair_block: u64,
        column: u16,
        lower: &[u8],
        upper: &[u8],
        output: &mut [u8],
    ) -> Result<(), SoundnessErrorV2> {
        let live = self.live.take().ok_or(SoundnessErrorV2::Poisoned)?;
        if pair_block != self.next_pair_block
            || column != self.next_column
            || pair_block >= 256
            || column >= COORDINATE_COUNT_V2 as u16
            || lower.len() != 1_024 * FQ2_BYTES_V2
            || upper.len() != lower.len()
            || output.len() != lower.len()
        {
            return Err(SoundnessErrorV2::InvalidFriEquation);
        }
        let coordinate = usize::from(column);
        let limb = coordinate / ROWS_PER_LIMB_V2;
        let field = self.fields[limb];
        let alpha = live.alphas[coordinate];
        let inverse_two = mod_pow_v1(2, field.modulus - 2, field.modulus);
        let exponent = u128::from(pair_block) * 1_024;
        let x = field.pow(field.domain_root, exponent);
        let mut inverse_x = field
            .inverse(x)
            .map_err(|_| SoundnessErrorV2::InvalidFriEquation)?;
        let inverse_root = self.inverse_domain_roots[limb];
        for ((positive, negative), next) in lower
            .chunks_exact(FQ2_BYTES_V2)
            .zip(upper.chunks_exact(FQ2_BYTES_V2))
            .zip(output.chunks_exact_mut(FQ2_BYTES_V2))
        {
            let decode = |value: &[u8]| -> Result<BatchValueV2, SoundnessErrorV2> {
                let c0 = read_u64_v2(value, 0)?;
                let c1 = read_u64_v2(value, 8)?;
                if c0 >= field.modulus || c1 >= field.modulus {
                    return Err(SoundnessErrorV2::NonCanonicalResidue);
                }
                Ok(BatchValueV2 { c0, c1 })
            };
            let positive = decode(positive)?;
            let negative = decode(negative)?;
            let even = field.scale(field.add(positive, negative), inverse_two);
            let inverse_two_x = field.scale(inverse_x, inverse_two);
            let odd = field.mul(field.sub(positive, negative), inverse_two_x);
            let value = field.add(even, field.mul(alpha, odd));
            next[..8].copy_from_slice(&value.c0.to_be_bytes());
            next[8..].copy_from_slice(&value.c1.to_be_bytes());
            inverse_x = field.mul(inverse_x, inverse_root);
        }
        self.next_column += 1;
        if self.next_column == COORDINATE_COUNT_V2 as u16 {
            self.next_column = 0;
            self.next_pair_block += 1;
        }
        self.live = Some(live);
        Ok(())
    }
    pub(super) fn complete_v2(mut self) -> Result<ProverFriLayer0FoldCompleteV2, SoundnessErrorV2> {
        let mut live = self.live.take().ok_or(SoundnessErrorV2::Poisoned)?;
        if self.next_pair_block != 256 || self.next_column != 0 {
            return Err(SoundnessErrorV2::InvalidFriEquation);
        }
        live.alphas.fill(BatchValueV2::ZERO);
        Ok(ProverFriLayer0FoldCompleteV2 {
            pre_layer_transcript: live.pre_layer_transcript,
            transcript: live.transcript,
            batch_schedule_digest: live.batch_schedule_digest,
            fold_schedule_digest: live.fold_schedule_digest,
            layer0_root: live.layer0_root,
        })
    }
}
impl ProverFriLayer0FoldCompleteV2 {
    #[allow(
        clippy::type_complexity,
        reason = "fixed transcript-state tuple preserves reviewed fold-complete ordering"
    )]
    pub(super) const fn context_v2(&self) -> ([u8; 32], [u8; 32], [u8; 32], [u8; 32], [u8; 32]) {
        (
            self.pre_layer_transcript,
            self.transcript,
            self.batch_schedule_digest,
            self.fold_schedule_digest,
            self.layer0_root,
        )
    }
}
fn absorb_root_v2(
    domain: &[u8],
    transcript: [u8; 32],
    ordinal: u8,
    root: [u8; 32],
) -> Result<[u8; 32], SoundnessErrorV2> {
    let mut frame = FrameV2::<160>::new();
    frame.push(domain)?;
    frame.push(&[VERSION_V2])?;
    frame.push(&transcript)?;
    frame.push(&[ordinal])?;
    frame.push(&root)?;
    Ok(keccak256(frame.bytes()))
}
fn derive_fq2_challenge_v2(
    domain: &[u8],
    transcript: [u8; 32],
    limb: usize,
    row: usize,
    component: usize,
    layer: usize,
) -> Result<Fq2V2, SoundnessErrorV2> {
    let modulus = RELEASE_MODULI_V1[limb];
    let zone = u64::MAX - u64::MAX % modulus;
    for attempt in 0_u32..256 {
        let mut frame = FrameV2::<176>::new();
        frame.push(domain)?;
        frame.push(&[VERSION_V2])?;
        frame.push(&transcript)?;
        frame.push(&[limb as u8, row as u8, component as u8, layer as u8])?;
        frame.push(&modulus.to_be_bytes())?;
        frame.push(&attempt.to_be_bytes())?;
        let bytes = shake256_fixed_v2::<FQ2_BYTES_V2>(frame.bytes());
        let c0 = u64::from_be_bytes(
            bytes[..8]
                .try_into()
                .map_err(|_| SoundnessErrorV2::InvalidChallenge)?,
        );
        let c1 = u64::from_be_bytes(
            bytes[8..]
                .try_into()
                .map_err(|_| SoundnessErrorV2::InvalidChallenge)?,
        );
        if c0 < zone && c1 < zone {
            let value = Fq2V2 {
                c0: c0 % modulus,
                c1: c1 % modulus,
            };
            if value != Fq2V2::ZERO {
                return Ok(value);
            }
        }
    }
    Err(SoundnessErrorV2::InvalidChallenge)
}
fn absorb_schedule_value_v2(
    digest: [u8; 32],
    kind: u8,
    limb: usize,
    row: usize,
    component: usize,
    layer: usize,
    value: Fq2V2,
) -> Result<[u8; 32], SoundnessErrorV2> {
    let mut frame = FrameV2::<160>::new();
    frame.push(SCHEDULE_DOMAIN_V2)?;
    frame.push(&[VERSION_V2, kind])?;
    frame.push(&digest)?;
    frame.push(&[limb as u8, row as u8, component as u8, layer as u8])?;
    frame.push(&value.encode())?;
    Ok(keccak256(frame.bytes()))
}
fn derive_batch_schedule_with_v2(
    transcript: [u8; 32],
    mut retain: impl FnMut(usize, Fq2V2),
) -> Result<[u8; 32], SoundnessErrorV2> {
    let mut digest = transcript;
    for limb in 0..LIMBS_V2 {
        for row in 0..ROWS_PER_LIMB_V2 {
            let (committed_power, quotient_power) = if row.is_multiple_of(2) {
                (0_u32, 1_u32)
            } else {
                (N_V2 as u32, N_V2 as u32 + 1)
            };
            let mut formula = FrameV2::<128>::new();
            formula.push(SCHEDULE_DOMAIN_V2)?;
            formula.push(&[VERSION_V2, 2, limb as u8, row as u8])?;
            formula.push(&digest)?;
            formula.push(&committed_power.to_be_bytes())?;
            formula.push(&quotient_power.to_be_bytes())?;
            digest = keccak256(formula.bytes());
            for component in 0..2 {
                let value =
                    derive_fq2_challenge_v2(BATCH_DOMAIN_V2, transcript, limb, row, component, 0)?;
                retain((limb * ROWS_PER_LIMB_V2 + row) * 2 + component, value);
                digest = absorb_schedule_value_v2(digest, 0, limb, row, component, 0, value)?;
            }
        }
    }
    Ok(digest)
}
fn derive_batch_schedule_v2(transcript: [u8; 32]) -> Result<[u8; 32], SoundnessErrorV2> {
    derive_batch_schedule_with_v2(transcript, |_, _| {})
}
impl<'a> RelationsCheckedV2<'a> {
    fn bind_quotient_root_v2(&mut self) -> Result<QuotientRootBoundV2<'a>, SoundnessErrorV2> {
        let mut live = self.live.take().ok_or(SoundnessErrorV2::Poisoned)?;
        let root = read_digest_v2(live.wire, live.offset)?;
        if root == [0; 32] {
            return Err(SoundnessErrorV2::InvalidRoot);
        }
        live.offset = live
            .offset
            .checked_add(QUOTIENT_ROOT_BYTES_V2)
            .ok_or(SoundnessErrorV2::ArithmeticOverflow)?;
        live.transcript = absorb_root_v2(QUOTIENT_DOMAIN_V2, live.transcript, 0, root)?;
        live.batch_schedule_digest = derive_batch_schedule_v2(live.transcript)?;
        Ok(QuotientRootBoundV2 { live: Some(live) })
    }
}
fn absorb_terminal_v2(transcript: [u8; 32], terminal: &[u8]) -> Result<[u8; 32], SoundnessErrorV2> {
    let mut frame = FrameV2::<12_320>::new();
    frame.push(TERMINAL_DOMAIN_V2)?;
    frame.push(&[VERSION_V2])?;
    frame.push(&transcript)?;
    frame.push(&(COORDINATE_COUNT_V2 as u16).to_be_bytes())?;
    frame.push(terminal)?;
    Ok(keccak256(frame.bytes()))
}
fn validate_leaf_values_v2(values: &[u8]) -> Result<(), SoundnessErrorV2> {
    if !values.len().is_multiple_of(LEAF_BYTES_V2) {
        return Err(SoundnessErrorV2::InvalidSectionCount);
    }
    for leaf in values.chunks_exact(LEAF_BYTES_V2) {
        for coordinate in 0..COORDINATE_COUNT_V2 {
            let modulus = RELEASE_MODULI_V1[coordinate / ROWS_PER_LIMB_V2];
            let offset = coordinate * FQ2_BYTES_V2;
            if read_u64_v2(leaf, offset)? >= modulus || read_u64_v2(leaf, offset + 8)? >= modulus {
                return Err(SoundnessErrorV2::NonCanonicalResidue);
            }
        }
    }
    Ok(())
}
fn derive_queries_v2(transcript: [u8; 32]) -> Result<[u32; QUERY_COUNT_V2], SoundnessErrorV2> {
    let bound = (DOMAIN_SIZE_V2 / 2) as u64;
    let zone = u64::MAX - u64::MAX % bound;
    let mut queries = [0_u32; QUERY_COUNT_V2];
    for ordinal in 0..QUERY_COUNT_V2 {
        let mut accepted = None;
        for attempt in 0_u32..256 {
            let mut frame = FrameV2::<144>::new();
            frame.push(QUERY_DOMAIN_V2)?;
            frame.push(&[VERSION_V2])?;
            frame.push(&transcript)?;
            frame.push(&(ordinal as u16).to_be_bytes())?;
            frame.push(&attempt.to_be_bytes())?;
            let bytes = shake256_fixed_v2::<8>(frame.bytes());
            let candidate = u64::from_be_bytes(bytes);
            if candidate < zone {
                let query = (candidate % bound) as u32;
                if !queries[..ordinal].contains(&query) {
                    accepted = Some(query);
                    break;
                }
            }
        }
        queries[ordinal] = accepted.ok_or(SoundnessErrorV2::InvalidChallenge)?;
    }
    Ok(queries)
}
impl<'a> QuotientRootBoundV2<'a> {
    fn bind_fri_transcript_v2(&mut self) -> Result<FriTranscriptBoundV2<'a>, SoundnessErrorV2> {
        let mut live = self.live.take().ok_or(SoundnessErrorV2::Poisoned)?;
        let mut schedule = live.batch_schedule_digest;
        for layer in 0..FRI_ROUNDS_V2 {
            let root = read_digest_v2(live.wire, live.offset)?;
            if root == [0; 32] {
                return Err(SoundnessErrorV2::InvalidRoot);
            }
            live.offset = live
                .offset
                .checked_add(32)
                .ok_or(SoundnessErrorV2::ArithmeticOverflow)?;
            live.transcript =
                absorb_root_v2(FRI_ROOT_DOMAIN_V2, live.transcript, layer as u8, root)?;
            for limb in 0..LIMBS_V2 {
                for row in 0..ROWS_PER_LIMB_V2 {
                    let alpha = derive_fq2_challenge_v2(
                        FOLD_DOMAIN_V2,
                        live.transcript,
                        limb,
                        row,
                        0,
                        layer,
                    )?;
                    schedule = absorb_schedule_value_v2(schedule, 1, limb, row, 0, layer, alpha)?;
                }
            }
        }
        live.fold_schedule_digest = schedule;
        let terminal_end = live
            .offset
            .checked_add(TERMINAL_BYTES_V2)
            .ok_or(SoundnessErrorV2::ArithmeticOverflow)?;
        let terminal = live
            .wire
            .get(live.offset..terminal_end)
            .ok_or(SoundnessErrorV2::Truncated)?;
        validate_equal_terminal_v2(terminal)?;
        live.transcript = absorb_terminal_v2(live.transcript, terminal)?;
        live.queries = derive_queries_v2(live.transcript)?;
        live.offset = terminal_end;
        Ok(FriTranscriptBoundV2 { live: Some(live) })
    }
}
struct IndexSetV2 {
    values: [u32; 2 * QUERY_COUNT_V2],
    len: usize,
}
fn query_pair_indices_v2(queries: &[u32; QUERY_COUNT_V2], length: usize) -> IndexSetV2 {
    let half = (length / 2) as u32;
    let mut result = IndexSetV2 {
        values: [0; 2 * QUERY_COUNT_V2],
        len: 2 * QUERY_COUNT_V2,
    };
    for (ordinal, query) in queries.iter().copied().enumerate() {
        let base = query % half;
        result.values[2 * ordinal] = base;
        result.values[2 * ordinal + 1] = base + half;
    }
    result.values[..result.len].sort_unstable();
    let mut unique = 0_usize;
    for index in 0..result.len {
        if unique == 0 || result.values[index] != result.values[unique - 1] {
            result.values[unique] = result.values[index];
            unique += 1;
        }
    }
    result.len = unique;
    result
}
fn exact_authentication_count_v2(
    indices: &IndexSetV2,
    mut length: usize,
) -> Result<usize, SoundnessErrorV2> {
    let mut current = indices.values;
    let mut current_len = indices.len;
    let mut authentication = 0_usize;
    while length > 1 {
        let mut parents = [0_u32; 2 * QUERY_COUNT_V2];
        let mut parent_len = 0_usize;
        for position in 0..current_len {
            let index = current[position];
            let sibling = index ^ 1;
            if current[..current_len].binary_search(&sibling).is_err() {
                authentication = authentication
                    .checked_add(1)
                    .ok_or(SoundnessErrorV2::ArithmeticOverflow)?;
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
    Ok(authentication)
}
fn checked_fri_multiproof_bytes_v2(
    opened: usize,
    authentication: usize,
) -> Result<usize, SoundnessErrorV2> {
    if opened > MAX_FRI_OPENED_LEAVES_V2 || authentication > MAX_FRI_AUTH_HASHES_V2 {
        return Err(SoundnessErrorV2::InvalidSectionCount);
    }
    let value_bytes = opened
        .checked_mul(LEAF_BYTES_V2)
        .ok_or(SoundnessErrorV2::ArithmeticOverflow)?;
    let authentication_bytes = authentication
        .checked_mul(32)
        .ok_or(SoundnessErrorV2::ArithmeticOverflow)?;
    let bytes = value_bytes
        .checked_add(authentication_bytes)
        .ok_or(SoundnessErrorV2::ArithmeticOverflow)?;
    if bytes > MAX_FRI_MULTIPROOF_BYTES_V2 {
        return Err(SoundnessErrorV2::InvalidSectionCount);
    }
    Ok(bytes)
}
fn parse_section_v2(
    live: &mut LiveProtocolV2<'_>,
    indices: &IndexSetV2,
    length: usize,
) -> Result<(usize, usize), SoundnessErrorV2> {
    let opened = read_u32_v2(live.wire, live.offset)? as usize;
    let authentication = read_u32_v2(live.wire, live.offset + 4)? as usize;
    let expected_authentication = exact_authentication_count_v2(indices, length)?;
    if opened != indices.len || authentication != expected_authentication {
        return Err(SoundnessErrorV2::InvalidSectionCount);
    }
    live.offset = live
        .offset
        .checked_add(SECTION_HEADER_BYTES_V2)
        .ok_or(SoundnessErrorV2::ArithmeticOverflow)?;
    let value_bytes = opened
        .checked_mul(LEAF_BYTES_V2)
        .ok_or(SoundnessErrorV2::ArithmeticOverflow)?;
    let value_end = live
        .offset
        .checked_add(value_bytes)
        .ok_or(SoundnessErrorV2::ArithmeticOverflow)?;
    validate_leaf_values_v2(
        live.wire
            .get(live.offset..value_end)
            .ok_or(SoundnessErrorV2::Truncated)?,
    )?;
    let authentication_bytes = authentication
        .checked_mul(32)
        .ok_or(SoundnessErrorV2::ArithmeticOverflow)?;
    live.offset = value_end
        .checked_add(authentication_bytes)
        .ok_or(SoundnessErrorV2::ArithmeticOverflow)?;
    if live.offset > live.wire.len() {
        return Err(SoundnessErrorV2::Truncated);
    }
    Ok((opened, authentication))
}
impl<'a> FriTranscriptBoundV2<'a> {
    fn parse_exact_sections_v2(&mut self) -> Result<StructurallyParsedV2<'a>, SoundnessErrorV2> {
        let mut live = self.live.take().ok_or(SoundnessErrorV2::Poisoned)?;
        if live.offset != FIXED_BEFORE_SECTIONS_V2 {
            return Err(SoundnessErrorV2::InvalidSectionCount);
        }
        let initial_indices = query_pair_indices_v2(&live.queries, DOMAIN_SIZE_V2);
        let (_, c0_auth) = parse_section_v2(&mut live, &initial_indices, DOMAIN_SIZE_V2)?;
        let (_, cq_auth) = parse_section_v2(&mut live, &initial_indices, DOMAIN_SIZE_V2)?;
        if c0_auth > MAX_INITIAL_AUTH_HASHES_PER_TREE_V2
            || cq_auth > MAX_INITIAL_AUTH_HASHES_PER_TREE_V2
        {
            return Err(SoundnessErrorV2::InvalidSectionCount);
        }
        let mut fri_opened = 0_usize;
        let mut fri_authentication = 0_usize;
        let mut layer_queries = live.queries;
        let mut length = DOMAIN_SIZE_V2;
        for _layer in 0..FRI_ROUNDS_V2 {
            let indices = query_pair_indices_v2(&layer_queries, length);
            let (opened, authentication) = parse_section_v2(&mut live, &indices, length)?;
            fri_opened = fri_opened
                .checked_add(opened)
                .ok_or(SoundnessErrorV2::ArithmeticOverflow)?;
            fri_authentication = fri_authentication
                .checked_add(authentication)
                .ok_or(SoundnessErrorV2::ArithmeticOverflow)?;
            let half = (length / 2) as u32;
            for query in &mut layer_queries {
                *query %= half;
            }
            length /= 2;
        }
        checked_fri_multiproof_bytes_v2(fri_opened, fri_authentication)?;
        if length != 2 {
            return Err(SoundnessErrorV2::InvalidSectionCount);
        }
        if live.offset != live.wire.len() {
            return Err(SoundnessErrorV2::TrailingBytes);
        }
        Ok(StructurallyParsedV2 { live: Some(live) })
    }
}
#[path = "phase23_rns_link_q_pcs_v2_soundness/prover_fri_rounds_v2.rs"]
mod prover_fri_rounds_v2;
pub(super) use prover_fri_rounds_v2::*;
#[path = "phase23_rns_link_q_pcs_v2_soundness/prover_canonical_proof_v2.rs"]
mod prover_canonical_proof_v2;
#[cfg(test)]
#[allow(
    unused_imports,
    reason = "canonical-proof helpers are retained while their spool-backed test owner is parked"
)]
pub(super) use prover_canonical_proof_v2::*;
#[cfg(test)]
#[path = "phase23_rns_link_q_pcs_v2_soundness_tests.rs"]
mod tests;
#[path = "phase23_rns_link_q_pcs_v2_verifier.rs"]
mod verifier;
