//! Initial qPCS Merkle authentication for the replacement 40-limb profile.
//!
//! This private verifier authenticates the 160 canonical query pairs against
//! the initial ten-row qPCS codeword root.  It is deliberately only the first
//! cryptographic substage: opening-quotient, batching, FRI, and RNS-relation
//! equations remain unavailable.  A successful return therefore never mints
//! a receipt and cannot grant proof, readiness, or release authority.

#[cfg(test)]
use super::rns_native_qpcs_leaf::RnsNativeLeafPayloadV1;
use super::rns_native_qpcs_leaf::{RnsNativeLeafCacheV1, RnsNativeOracleV1, oracle_node_hash_v1};
use super::{
    rns_native_profile::{
        ZK_AMS_MKHE_RNS_NATIVE_INITIAL_MULTIPROOF_MAX_BYTES_V1,
        ZK_AMS_MKHE_RNS_NATIVE_LDE_DOMAIN_LOG2_V1, ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1,
        ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1, ZK_AMS_MKHE_RNS_NATIVE_QPCS_MAX_BYTES_V1,
        ZK_AMS_MKHE_RNS_NATIVE_QUERY_COUNT_V1, zk_ams_mkhe_rns_native_profile_manifest_v1,
        zk_ams_mkhe_rns_native_profile_v1, zk_ams_mkhe_rns_native_release_candidate_digest_v1,
        zk_ams_mkhe_rns_native_topology_v1,
    },
    rns_native_transcript::ZkAmsMkheRnsNativeChallengeSeedsV1,
};
// Keccak is retained only for the public canonical parameter identity.
use super::{
    rns_native_proof_hash::{
        RnsNativeProofDigestV1 as ProofDigestV1, RnsNativeProofHashContextV1,
        RnsNativeProofHashPhaseV1, RnsNativeProofHashPositionV1, RnsNativeProofHashRoleV1,
        decode_proof_digest_v1,
    },
    rns_native_proof_sampling::derive_query_indices_v1,
    rns_native_qpcs_field_wire::{RNS_NATIVE_QPCS_FQ2_BYTES_V1, decode_fq2_v1},
};
use crate::vega::sponge::Keccak256;

const QPCS_BODY_MAGIC_V1: [u8; 4] = *b"ZQPB";
const QPCS_BODY_VERSION_V1: u8 = 1;
const ROWS_PER_LIMB_V1: usize = 10;
const FQ2_BYTES_V1: usize = RNS_NATIVE_QPCS_FQ2_BYTES_V1;
const DIGEST_BYTES_V1: usize = fastpq_isi::GOLDILOCKS_DIGEST384_BYTES_V1;
const QUERY_COUNT_V1: usize = ZK_AMS_MKHE_RNS_NATIVE_QUERY_COUNT_V1 as usize;
const OPENED_LEAF_COUNT_V1: usize = 2 * QUERY_COUNT_V1;
const COORDINATE_COUNT_V1: usize = ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1 * ROWS_PER_LIMB_V1;
const LEAF_BYTES_V1: usize = COORDINATE_COUNT_V1 * FQ2_BYTES_V1;
const DOMAIN_SIZE_V1: usize = 1 << ZK_AMS_MKHE_RNS_NATIVE_LDE_DOMAIN_LOG2_V1;
const MAX_INITIAL_AUTHENTICATION_HASHES_V1: usize = 3_392;
const MAX_INITIAL_TREE_BYTES_V1: usize =
    OPENED_LEAF_COUNT_V1 * LEAF_BYTES_V1 + MAX_INITIAL_AUTHENTICATION_HASHES_V1 * DIGEST_BYTES_V1;
pub(super) const MAX_INITIAL_AND_QUOTIENT_BYTES_V1: usize = 2 * MAX_INITIAL_TREE_BYTES_V1;
pub(super) const QPCS_BODY_HEADER_BYTES_V1: usize =
    4 + 4 + 3 * 2 + 3 * 4 + 32 + 4 * DIGEST_BYTES_V1;

const PARAMETER_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.rns-native-qpcs.parameters";
const QUERY_OPENING_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-qpcs.initial-query-opening";
const CONTINUATION_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-qpcs.unverified-continuation";

const _: () = {
    assert!(ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1 == 40);
    assert!(ZK_AMS_MKHE_RNS_NATIVE_LDE_DOMAIN_LOG2_V1 == 19);
    assert!(QUERY_COUNT_V1 == 160);
    assert!(ROWS_PER_LIMB_V1 == 10);
    assert!(COORDINATE_COUNT_V1 == 400);
    assert!(LEAF_BYTES_V1 == 6_000);
    assert!(OPENED_LEAF_COUNT_V1 == 320);
    assert!(MAX_INITIAL_TREE_BYTES_V1 == 2_082_816);
    assert!(2 * MAX_INITIAL_TREE_BYTES_V1 == 4_165_632);
    assert!(
        2 * MAX_INITIAL_TREE_BYTES_V1
            <= ZK_AMS_MKHE_RNS_NATIVE_INITIAL_MULTIPROOF_MAX_BYTES_V1 as usize
    );
    assert!(QPCS_BODY_HEADER_BYTES_V1 == 250);
    assert!(MAX_INITIAL_TREE_BYTES_V1 < ZK_AMS_MKHE_RNS_NATIVE_QPCS_MAX_BYTES_V1 as usize);
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Failure while decoding or authenticating the initial replacement qPCS tree.
pub(super) enum RnsNativeQpcsInitialErrorV1 {
    /// Canonical profile or transcript identities did not match.
    InvalidContext,
    /// The proof exceeded the complete qPCS body cap.
    ProofCapExceeded,
    /// The proof ended before its declared exact body length.
    Truncated,
    /// Bytes remained after the declared exact body.
    TrailingBytes,
    /// Fixed tags, dimensions, lengths, or context digests were invalid.
    InvalidHeader,
    /// An exact fixed-width count was invalid.
    InvalidCount,
    /// Unique unbiased query derivation failed.
    InvalidQuerySchedule,
    /// An Fq2 coordinate was outside its limb modulus.
    NonCanonicalResidue,
    /// The ordered multiproof did not authenticate the transcript root.
    InvalidMerklePath,
    /// A section query digest did not bind its exact ordered leaf pair.
    InvalidQueryOpening,
    /// Checked index or byte arithmetic overflowed.
    ArithmeticOverflow,
}

impl core::fmt::Display for RnsNativeQpcsInitialErrorV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for RnsNativeQpcsInitialErrorV1 {}

#[derive(Clone, Copy)]
struct InitialContextV1 {
    parameter_digest: [u8; 32],
    transcript_digest: ProofDigestV1,
    query_seed: ProofDigestV1,
    initial_root: ProofDigestV1,
}

impl InitialContextV1 {
    fn from_transcript_v1(
        transcript: &ZkAmsMkheRnsNativeChallengeSeedsV1,
    ) -> Result<Self, RnsNativeQpcsInitialErrorV1> {
        let manifest = zk_ams_mkhe_rns_native_profile_manifest_v1()
            .map_err(|_| RnsNativeQpcsInitialErrorV1::InvalidContext)?;
        manifest
            .validate()
            .map_err(|_| RnsNativeQpcsInitialErrorV1::InvalidContext)?;
        let profile = zk_ams_mkhe_rns_native_profile_v1()
            .map_err(|_| RnsNativeQpcsInitialErrorV1::InvalidContext)?;
        let topology = zk_ams_mkhe_rns_native_topology_v1()
            .map_err(|_| RnsNativeQpcsInitialErrorV1::InvalidContext)?;
        topology
            .validate()
            .map_err(|_| RnsNativeQpcsInitialErrorV1::InvalidContext)?;
        let release_candidate = zk_ams_mkhe_rns_native_release_candidate_digest_v1()
            .map_err(|_| RnsNativeQpcsInitialErrorV1::InvalidContext)?;
        let transcript_digest = transcript.transcript_digest();
        let query_seed = transcript.qpcs_query_challenge_seed();
        let initial_root = transcript.qpcs_initial_root();
        if transcript.profile_manifest_digest() != manifest.manifest_digest
            || transcript.profile_digest() != profile.profile_digest
            || transcript.topology_digest() != topology.topology_digest
            || transcript.release_candidate_digest() != release_candidate
            || transcript_digest == ProofDigestV1::ZERO
            || query_seed == ProofDigestV1::ZERO
            || initial_root == ProofDigestV1::ZERO
        {
            return Err(RnsNativeQpcsInitialErrorV1::InvalidContext);
        }
        Ok(Self {
            parameter_digest: canonical_parameter_digest_v1()?,
            transcript_digest,
            query_seed,
            initial_root,
        })
    }
}

#[derive(Clone, Copy)]
struct IndexSetV1 {
    values: [u32; OPENED_LEAF_COUNT_V1],
    len: usize,
}

#[derive(Clone, Copy)]
struct FrontierNodeV1 {
    index: u32,
    digest: ProofDigestV1,
}

const EMPTY_FRONTIER_NODE_V1: FrontierNodeV1 = FrontierNodeV1 {
    index: 0,
    digest: ProofDigestV1::ZERO,
};

#[derive(Clone, Copy)]
struct InitialProofViewV1<'a> {
    values: &'a [u8],
    authentication: &'a [u8],
    continuation: &'a [u8],
}

/// Borrowed, move-only result of the initial-tree substage.
///
/// This is an internal data-flow token only.  It is not a proof receipt and
/// carries no readiness or release authority; the sole consumer immediately
/// checks the next qPCS prefix and the composite verifier still fails closed.
#[allow(
    missing_copy_implementations,
    reason = "qPCS substages must not be copied or replayed out of order"
)]
pub(super) struct RnsNativeQpcsInitialStageV1<'a> {
    context: InitialContextV1,
    queries: [u32; QUERY_COUNT_V1],
    indices: IndexSetV1,
    values: &'a [u8],
    continuation: &'a [u8],
}

impl<'a> RnsNativeQpcsInitialStageV1<'a> {
    pub(super) const fn parameter_digest(&self) -> [u8; 32] {
        self.context.parameter_digest
    }

    pub(super) const fn queries(&self) -> &[u32; QUERY_COUNT_V1] {
        &self.queries
    }

    pub(super) fn indices(&self) -> &[u32] {
        &self.indices.values[..self.indices.len]
    }

    pub(super) const fn values(&self) -> &'a [u8] {
        self.values
    }

    pub(super) const fn continuation(&self) -> &'a [u8] {
        self.continuation
    }
}

struct DecoderV1<'a> {
    bytes: &'a [u8],
    cursor: usize,
}

impl<'a> DecoderV1<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, cursor: 0 }
    }

    fn take(&mut self, len: usize) -> Result<&'a [u8], RnsNativeQpcsInitialErrorV1> {
        let end = self
            .cursor
            .checked_add(len)
            .ok_or(RnsNativeQpcsInitialErrorV1::ArithmeticOverflow)?;
        let value = self
            .bytes
            .get(self.cursor..end)
            .ok_or(RnsNativeQpcsInitialErrorV1::Truncated)?;
        self.cursor = end;
        Ok(value)
    }

    fn u8(&mut self) -> Result<u8, RnsNativeQpcsInitialErrorV1> {
        Ok(self.take(1)?[0])
    }

    fn u16(&mut self) -> Result<u16, RnsNativeQpcsInitialErrorV1> {
        let bytes: [u8; 2] = self
            .take(2)?
            .try_into()
            .map_err(|_| RnsNativeQpcsInitialErrorV1::Truncated)?;
        Ok(u16::from_be_bytes(bytes))
    }

    fn u32(&mut self) -> Result<u32, RnsNativeQpcsInitialErrorV1> {
        let bytes: [u8; 4] = self
            .take(4)?
            .try_into()
            .map_err(|_| RnsNativeQpcsInitialErrorV1::Truncated)?;
        Ok(u32::from_be_bytes(bytes))
    }

    fn public_digest(&mut self) -> Result<[u8; 32], RnsNativeQpcsInitialErrorV1> {
        self.take(32)?
            .try_into()
            .map_err(|_| RnsNativeQpcsInitialErrorV1::Truncated)
    }

    fn digest(&mut self) -> Result<ProofDigestV1, RnsNativeQpcsInitialErrorV1> {
        decode_proof_digest_v1(self.take(DIGEST_BYTES_V1)?)
            .map_err(|_| RnsNativeQpcsInitialErrorV1::InvalidHeader)
    }
}

/// Authenticate the initial 40-limb qPCS codeword at all canonical queries.
///
/// This verifies only Merkle membership and ordered query-digest binding.  It
/// returns no capability; its sole production caller subsequently reports the
/// still-unavailable complete RNS/qPCS stage.
#[cfg(test)]
pub(super) fn verify_rns_native_qpcs_initial_v1(
    transcript: &ZkAmsMkheRnsNativeChallengeSeedsV1,
    expected_query_opening_digests: &[ProofDigestV1],
    proof: &[u8],
) -> Result<(), RnsNativeQpcsInitialErrorV1> {
    authenticate_rns_native_qpcs_initial_v1(transcript, expected_query_opening_digests, proof)
        .map(drop)
}

/// Authenticate the initial tree and consume its exact borrowed output in the
/// next private qPCS substage.
pub(super) fn authenticate_rns_native_qpcs_initial_v1<'a>(
    transcript: &ZkAmsMkheRnsNativeChallengeSeedsV1,
    expected_query_opening_digests: &[ProofDigestV1],
    proof: &'a [u8],
) -> Result<RnsNativeQpcsInitialStageV1<'a>, RnsNativeQpcsInitialErrorV1> {
    preflight_proof_v1(proof)?;
    let context = InitialContextV1::from_transcript_v1(transcript)?;
    authenticate_initial_with_context_v1(context, expected_query_opening_digests, proof)
}

#[cfg(test)]
fn verify_initial_with_context_v1(
    context: InitialContextV1,
    expected_query_opening_digests: &[ProofDigestV1],
    proof: &[u8],
) -> Result<(), RnsNativeQpcsInitialErrorV1> {
    authenticate_initial_with_context_v1(context, expected_query_opening_digests, proof).map(drop)
}

fn authenticate_initial_with_context_v1<'a>(
    context: InitialContextV1,
    expected_query_opening_digests: &[ProofDigestV1],
    proof: &'a [u8],
) -> Result<RnsNativeQpcsInitialStageV1<'a>, RnsNativeQpcsInitialErrorV1> {
    preflight_proof_v1(proof)?;
    if expected_query_opening_digests.len() != QUERY_COUNT_V1 {
        return Err(RnsNativeQpcsInitialErrorV1::InvalidCount);
    }
    let queries = derive_queries_v1(context.parameter_digest, context.query_seed)?;
    let indices = query_pair_indices_v1(&queries)?;
    let expected_authentication = exact_authentication_count_v1(indices)?;
    let view = decode_initial_proof_exact_v1(proof, context, expected_authentication)?;
    validate_leaf_values_v1(view.values)?;
    authenticate_initial_tree_v1(
        view.values,
        view.authentication,
        indices,
        context.parameter_digest,
        context.initial_root,
    )?;
    bind_query_openings_v1(
        context,
        &queries,
        indices,
        view.values,
        expected_query_opening_digests,
    )?;
    let continuation_digest = continuation_digest_v1(context, view.continuation)?;
    if continuation_digest == ProofDigestV1::ZERO {
        return Err(RnsNativeQpcsInitialErrorV1::InvalidHeader);
    }
    Ok(RnsNativeQpcsInitialStageV1 {
        context,
        queries,
        indices,
        values: view.values,
        continuation: view.continuation,
    })
}

fn preflight_proof_v1(proof: &[u8]) -> Result<(), RnsNativeQpcsInitialErrorV1> {
    if proof.len() > ZK_AMS_MKHE_RNS_NATIVE_QPCS_MAX_BYTES_V1 as usize {
        return Err(RnsNativeQpcsInitialErrorV1::ProofCapExceeded);
    }
    if proof.len() < QPCS_BODY_HEADER_BYTES_V1 {
        return Err(RnsNativeQpcsInitialErrorV1::Truncated);
    }
    Ok(())
}

pub(super) fn canonical_parameter_digest_v1() -> Result<[u8; 32], RnsNativeQpcsInitialErrorV1> {
    let manifest = zk_ams_mkhe_rns_native_profile_manifest_v1()
        .map_err(|_| RnsNativeQpcsInitialErrorV1::InvalidContext)?;
    manifest
        .validate()
        .map_err(|_| RnsNativeQpcsInitialErrorV1::InvalidContext)?;
    let profile = zk_ams_mkhe_rns_native_profile_v1()
        .map_err(|_| RnsNativeQpcsInitialErrorV1::InvalidContext)?;
    let topology = zk_ams_mkhe_rns_native_topology_v1()
        .map_err(|_| RnsNativeQpcsInitialErrorV1::InvalidContext)?;
    topology
        .validate()
        .map_err(|_| RnsNativeQpcsInitialErrorV1::InvalidContext)?;
    let release_candidate = zk_ams_mkhe_rns_native_release_candidate_digest_v1()
        .map_err(|_| RnsNativeQpcsInitialErrorV1::InvalidContext)?;
    let mut hash = Keccak256::new();
    hash.update(PARAMETER_DOMAIN_V1);
    hash.update(&[QPCS_BODY_VERSION_V1]);
    hash.update(&manifest.manifest_digest);
    hash.update(&profile.profile_digest);
    hash.update(&topology.topology_digest);
    hash.update(&release_candidate);
    hash.update(&[
        ZK_AMS_MKHE_RNS_NATIVE_LDE_DOMAIN_LOG2_V1,
        u8::try_from(ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1)
            .map_err(|_| RnsNativeQpcsInitialErrorV1::ArithmeticOverflow)?,
        u8::try_from(ROWS_PER_LIMB_V1)
            .map_err(|_| RnsNativeQpcsInitialErrorV1::ArithmeticOverflow)?,
    ]);
    hash.update(&ZK_AMS_MKHE_RNS_NATIVE_QUERY_COUNT_V1.to_be_bytes());
    hash.update(
        &u16::try_from(COORDINATE_COUNT_V1)
            .map_err(|_| RnsNativeQpcsInitialErrorV1::ArithmeticOverflow)?
            .to_be_bytes(),
    );
    for modulus in ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1 {
        hash.update(&modulus.to_be_bytes());
    }
    let digest = hash.finalize();
    if digest == [0; 32] {
        return Err(RnsNativeQpcsInitialErrorV1::InvalidContext);
    }
    Ok(digest)
}

fn derive_queries_v1(
    parameter_digest: [u8; 32],
    query_seed: ProofDigestV1,
) -> Result<[u32; QUERY_COUNT_V1], RnsNativeQpcsInitialErrorV1> {
    let context = hash_context_v1(parameter_digest)?;
    derive_query_indices_v1(&context, query_seed)
        .map_err(|_| RnsNativeQpcsInitialErrorV1::InvalidQuerySchedule)
}

fn query_pair_indices_v1(
    queries: &[u32; QUERY_COUNT_V1],
) -> Result<IndexSetV1, RnsNativeQpcsInitialErrorV1> {
    let half = u32::try_from(DOMAIN_SIZE_V1 / 2)
        .map_err(|_| RnsNativeQpcsInitialErrorV1::ArithmeticOverflow)?;
    let mut indices = IndexSetV1 {
        values: [0; OPENED_LEAF_COUNT_V1],
        len: OPENED_LEAF_COUNT_V1,
    };
    for (ordinal, query) in queries.iter().copied().enumerate() {
        if query >= half {
            return Err(RnsNativeQpcsInitialErrorV1::InvalidQuerySchedule);
        }
        indices.values[2 * ordinal] = query;
        indices.values[2 * ordinal + 1] = query
            .checked_add(half)
            .ok_or(RnsNativeQpcsInitialErrorV1::ArithmeticOverflow)?;
    }
    indices.values.sort_unstable();
    for pair in indices.values.windows(2) {
        if pair[0] == pair[1] {
            return Err(RnsNativeQpcsInitialErrorV1::InvalidQuerySchedule);
        }
    }
    Ok(indices)
}

fn exact_authentication_count_v1(
    indices: IndexSetV1,
) -> Result<usize, RnsNativeQpcsInitialErrorV1> {
    let mut current = indices.values;
    let mut current_len = indices.len;
    let mut length = DOMAIN_SIZE_V1;
    let mut authentication = 0_usize;
    while length > 1 {
        let mut parents = [0_u32; OPENED_LEAF_COUNT_V1];
        let mut parent_len = 0_usize;
        for position in 0..current_len {
            let index = current[position];
            if current[..current_len].binary_search(&(index ^ 1)).is_err() {
                authentication = authentication
                    .checked_add(1)
                    .ok_or(RnsNativeQpcsInitialErrorV1::ArithmeticOverflow)?;
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
    if authentication > MAX_INITIAL_AUTHENTICATION_HASHES_V1 {
        return Err(RnsNativeQpcsInitialErrorV1::InvalidCount);
    }
    Ok(authentication)
}

fn decode_initial_proof_exact_v1<'a>(
    proof: &'a [u8],
    context: InitialContextV1,
    expected_authentication: usize,
) -> Result<InitialProofViewV1<'a>, RnsNativeQpcsInitialErrorV1> {
    preflight_proof_v1(proof)?;
    let mut decoder = DecoderV1::new(proof);
    if decoder.take(QPCS_BODY_MAGIC_V1.len())? != QPCS_BODY_MAGIC_V1.as_slice()
        || decoder.u8()? != QPCS_BODY_VERSION_V1
        || decoder.u8()? != ZK_AMS_MKHE_RNS_NATIVE_LDE_DOMAIN_LOG2_V1
        || usize::from(decoder.u8()?) != ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1
        || usize::from(decoder.u8()?) != ROWS_PER_LIMB_V1
        || decoder.u16()? != ZK_AMS_MKHE_RNS_NATIVE_QUERY_COUNT_V1
        || usize::from(decoder.u16()?) != OPENED_LEAF_COUNT_V1
        || usize::from(decoder.u16()?) != expected_authentication
    {
        return Err(RnsNativeQpcsInitialErrorV1::InvalidHeader);
    }
    let values_bytes = usize::try_from(decoder.u32()?)
        .map_err(|_| RnsNativeQpcsInitialErrorV1::ArithmeticOverflow)?;
    let authentication_bytes = usize::try_from(decoder.u32()?)
        .map_err(|_| RnsNativeQpcsInitialErrorV1::ArithmeticOverflow)?;
    let continuation_bytes = usize::try_from(decoder.u32()?)
        .map_err(|_| RnsNativeQpcsInitialErrorV1::ArithmeticOverflow)?;
    let parameter_digest = decoder.public_digest()?;
    let transcript_digest = decoder.digest()?;
    let query_seed = decoder.digest()?;
    let initial_root = decoder.digest()?;
    let encoded_continuation_digest = decoder.digest()?;
    let expected_values_bytes = OPENED_LEAF_COUNT_V1
        .checked_mul(LEAF_BYTES_V1)
        .ok_or(RnsNativeQpcsInitialErrorV1::ArithmeticOverflow)?;
    let expected_authentication_bytes = expected_authentication
        .checked_mul(DIGEST_BYTES_V1)
        .ok_or(RnsNativeQpcsInitialErrorV1::ArithmeticOverflow)?;
    if values_bytes != expected_values_bytes
        || authentication_bytes != expected_authentication_bytes
        || values_bytes
            .checked_add(authentication_bytes)
            .ok_or(RnsNativeQpcsInitialErrorV1::ArithmeticOverflow)?
            > MAX_INITIAL_TREE_BYTES_V1
        || continuation_bytes == 0
        || parameter_digest != context.parameter_digest
        || transcript_digest != context.transcript_digest
        || query_seed != context.query_seed
        || initial_root != context.initial_root
    {
        return Err(RnsNativeQpcsInitialErrorV1::InvalidHeader);
    }
    let expected_total = QPCS_BODY_HEADER_BYTES_V1
        .checked_add(values_bytes)
        .and_then(|total| total.checked_add(authentication_bytes))
        .and_then(|total| total.checked_add(continuation_bytes))
        .ok_or(RnsNativeQpcsInitialErrorV1::ArithmeticOverflow)?;
    if expected_total > ZK_AMS_MKHE_RNS_NATIVE_QPCS_MAX_BYTES_V1 as usize {
        return Err(RnsNativeQpcsInitialErrorV1::ProofCapExceeded);
    }
    if proof.len() < expected_total {
        return Err(RnsNativeQpcsInitialErrorV1::Truncated);
    }
    if proof.len() != expected_total {
        return Err(RnsNativeQpcsInitialErrorV1::TrailingBytes);
    }
    let values = decoder.take(values_bytes)?;
    let authentication = decoder.take(authentication_bytes)?;
    let continuation = decoder.take(continuation_bytes)?;
    if decoder.cursor != proof.len()
        || encoded_continuation_digest != continuation_digest_v1(context, continuation)?
    {
        return Err(RnsNativeQpcsInitialErrorV1::InvalidHeader);
    }
    Ok(InitialProofViewV1 {
        values,
        authentication,
        continuation,
    })
}

fn validate_leaf_values_v1(values: &[u8]) -> Result<(), RnsNativeQpcsInitialErrorV1> {
    if values.len() != OPENED_LEAF_COUNT_V1 * LEAF_BYTES_V1 {
        return Err(RnsNativeQpcsInitialErrorV1::InvalidCount);
    }
    for leaf in values.chunks_exact(LEAF_BYTES_V1) {
        for (coordinate, pair) in leaf.chunks_exact(FQ2_BYTES_V1).enumerate() {
            decode_fq2_v1(coordinate / ROWS_PER_LIMB_V1, pair)
                .map_err(|_| RnsNativeQpcsInitialErrorV1::NonCanonicalResidue)?;
        }
    }
    Ok(())
}

fn hash_context_v1(
    parameter_digest: [u8; 32],
) -> Result<RnsNativeProofHashContextV1, RnsNativeQpcsInitialErrorV1> {
    let context = RnsNativeProofHashContextV1::canonical()
        .map_err(|_| RnsNativeQpcsInitialErrorV1::InvalidContext)?;
    if context.parameter_digest() != parameter_digest {
        return Err(RnsNativeQpcsInitialErrorV1::InvalidContext);
    }
    Ok(context)
}

#[cfg(test)]
fn leaf_hash_v1(
    parameter_digest: [u8; 32],
    index: u32,
    values: &[u8],
) -> Result<ProofDigestV1, RnsNativeQpcsInitialErrorV1> {
    RnsNativeLeafPayloadV1::from_canonical_values(
        parameter_digest,
        RnsNativeOracleV1::Initial,
        values,
    )
    .and_then(|payload| payload.at_index(index))
    .map_err(|_| RnsNativeQpcsInitialErrorV1::InvalidContext)
}

fn node_hash_v1(
    parameter_digest: [u8; 32],
    height: usize,
    index: u32,
    left: ProofDigestV1,
    right: ProofDigestV1,
) -> Result<ProofDigestV1, RnsNativeQpcsInitialErrorV1> {
    if height == 0
        || height > usize::from(ZK_AMS_MKHE_RNS_NATIVE_LDE_DOMAIN_LOG2_V1)
        || index as usize >= DOMAIN_SIZE_V1 >> height
    {
        return Err(RnsNativeQpcsInitialErrorV1::InvalidCount);
    }
    oracle_node_hash_v1(
        parameter_digest,
        RnsNativeOracleV1::Initial,
        height,
        index,
        [left, right],
    )
    .map_err(|_| RnsNativeQpcsInitialErrorV1::InvalidContext)
}

fn authenticate_initial_tree_v1(
    values: &[u8],
    authentication: &[u8],
    indices: IndexSetV1,
    parameter_digest: [u8; 32],
    expected_root: ProofDigestV1,
) -> Result<(), RnsNativeQpcsInitialErrorV1> {
    if indices.len != OPENED_LEAF_COUNT_V1
        || values.len() != indices.len * LEAF_BYTES_V1
        || !authentication.len().is_multiple_of(DIGEST_BYTES_V1)
    {
        return Err(RnsNativeQpcsInitialErrorV1::InvalidCount);
    }
    let mut leaves = RnsNativeLeafCacheV1::new(parameter_digest, RnsNativeOracleV1::Initial)
        .map_err(|_| RnsNativeQpcsInitialErrorV1::InvalidContext)?;
    let mut current = [EMPTY_FRONTIER_NODE_V1; OPENED_LEAF_COUNT_V1];
    let mut next = [EMPTY_FRONTIER_NODE_V1; OPENED_LEAF_COUNT_V1];
    for (position, node) in current.iter_mut().enumerate().take(indices.len) {
        let start = position
            .checked_mul(LEAF_BYTES_V1)
            .ok_or(RnsNativeQpcsInitialErrorV1::ArithmeticOverflow)?;
        *node = FrontierNodeV1 {
            index: indices.values[position],
            digest: leaves
                .leaf(
                    indices.values[position],
                    &values[start..start + LEAF_BYTES_V1],
                )
                .map_err(|_| RnsNativeQpcsInitialErrorV1::InvalidMerklePath)?,
        };
    }
    let mut current_len = indices.len;
    let mut nodes_at_height = DOMAIN_SIZE_V1;
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
                    .ok_or(RnsNativeQpcsInitialErrorV1::ArithmeticOverflow)?;
                let sibling = decode_proof_digest_v1(
                    authentication
                        .get(start..start + DIGEST_BYTES_V1)
                        .ok_or(RnsNativeQpcsInitialErrorV1::InvalidMerklePath)?,
                )
                .map_err(|_| RnsNativeQpcsInitialErrorV1::InvalidMerklePath)?;
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
                digest: node_hash_v1(parameter_digest, height, node.index / 2, left, right)?,
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
        || authentication_cursor * DIGEST_BYTES_V1 != authentication.len()
    {
        return Err(RnsNativeQpcsInitialErrorV1::InvalidMerklePath);
    }
    Ok(())
}

fn bind_query_openings_v1(
    context: InitialContextV1,
    queries: &[u32; QUERY_COUNT_V1],
    indices: IndexSetV1,
    values: &[u8],
    expected: &[ProofDigestV1],
) -> Result<(), RnsNativeQpcsInitialErrorV1> {
    let half = u32::try_from(DOMAIN_SIZE_V1 / 2)
        .map_err(|_| RnsNativeQpcsInitialErrorV1::ArithmeticOverflow)?;
    let mut leaves =
        RnsNativeLeafCacheV1::new(context.parameter_digest, RnsNativeOracleV1::Initial)
            .map_err(|_| RnsNativeQpcsInitialErrorV1::InvalidContext)?;
    for (ordinal, base) in queries.iter().copied().enumerate() {
        let paired = base
            .checked_add(half)
            .ok_or(RnsNativeQpcsInitialErrorV1::ArithmeticOverflow)?;
        let first = leaf_hash_at_index_v1(&mut leaves, indices, values, base)?;
        let second = leaf_hash_at_index_v1(&mut leaves, indices, values, paired)?;
        if expected.get(ordinal).copied()
            != Some(query_opening_digest_v1(
                context, ordinal, base, paired, first, second,
            )?)
        {
            return Err(RnsNativeQpcsInitialErrorV1::InvalidQueryOpening);
        }
    }
    Ok(())
}

fn query_opening_digest_v1(
    context: InitialContextV1,
    ordinal: usize,
    base: u32,
    paired: u32,
    first: ProofDigestV1,
    second: ProofDigestV1,
) -> Result<ProofDigestV1, RnsNativeQpcsInitialErrorV1> {
    if ordinal >= QUERY_COUNT_V1
        || base as usize >= DOMAIN_SIZE_V1 / 2
        || paired as usize != base as usize + DOMAIN_SIZE_V1 / 2
    {
        return Err(RnsNativeQpcsInitialErrorV1::InvalidQuerySchedule);
    }
    hash_context_v1(context.parameter_digest)?
        .hash(
            RnsNativeProofHashRoleV1::Initial,
            RnsNativeProofHashPhaseV1::Opening,
            RnsNativeProofHashPositionV1 {
                level: 0,
                index: ordinal as u64,
                counter: 0,
            },
            &[
                QUERY_OPENING_DOMAIN_V1,
                &[QPCS_BODY_VERSION_V1],
                context.transcript_digest.as_bytes(),
                context.query_seed.as_bytes(),
                context.initial_root.as_bytes(),
                &base.to_be_bytes(),
                &paired.to_be_bytes(),
                first.as_bytes(),
                second.as_bytes(),
            ],
        )
        .map_err(|_| RnsNativeQpcsInitialErrorV1::InvalidContext)
}

fn leaf_hash_at_index_v1<'a>(
    leaves: &mut RnsNativeLeafCacheV1<'a>,
    indices: IndexSetV1,
    values: &'a [u8],
    index: u32,
) -> Result<ProofDigestV1, RnsNativeQpcsInitialErrorV1> {
    let position = indices.values[..indices.len]
        .binary_search(&index)
        .map_err(|_| RnsNativeQpcsInitialErrorV1::InvalidQuerySchedule)?;
    let start = position
        .checked_mul(LEAF_BYTES_V1)
        .ok_or(RnsNativeQpcsInitialErrorV1::ArithmeticOverflow)?;
    let leaf = values
        .get(start..start + LEAF_BYTES_V1)
        .ok_or(RnsNativeQpcsInitialErrorV1::Truncated)?;
    leaves
        .leaf(index, leaf)
        .map_err(|_| RnsNativeQpcsInitialErrorV1::InvalidContext)
}

fn continuation_digest_v1(
    context: InitialContextV1,
    continuation: &[u8],
) -> Result<ProofDigestV1, RnsNativeQpcsInitialErrorV1> {
    let length = u32::try_from(continuation.len())
        .map_err(|_| RnsNativeQpcsInitialErrorV1::ArithmeticOverflow)?;
    hash_context_v1(context.parameter_digest)?
        .hash(
            RnsNativeProofHashRoleV1::Initial,
            RnsNativeProofHashPhaseV1::Binding,
            RnsNativeProofHashPositionV1::default(),
            &[
                CONTINUATION_DOMAIN_V1,
                &[QPCS_BODY_VERSION_V1],
                context.transcript_digest.as_bytes(),
                context.query_seed.as_bytes(),
                context.initial_root.as_bytes(),
                &length.to_be_bytes(),
                continuation,
            ],
        )
        .map_err(|_| RnsNativeQpcsInitialErrorV1::InvalidContext)
}

#[cfg(test)]
#[path = "rns_native_qpcs_initial_tests.rs"]
mod tests;
