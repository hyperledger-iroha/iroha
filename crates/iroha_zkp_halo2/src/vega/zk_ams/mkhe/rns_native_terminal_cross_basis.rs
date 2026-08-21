//! Private RNS-native terminal cross-basis kernel prerequisite.
//!
//! This module gives the existing T256 representation-equality kernel a
//! canonical, fixed-width transport and binds it to the exact pre-bridge-root
//! transcript context.  It proves only that 1,536 ordered Hyrax and
//! Bulletproof commitments have the same 1,025-coordinate row openings.  It
//! does not prove that those rows are the terminal materialization of the 43
//! source commitments, does not satisfy the source/packing seals, and cannot
//! mint proof-readiness or release authority.

use super::{
    rns_native_transcript::ZkAmsMkheRnsNativeChallengeSeedsV1,
    rns_native_wire::ZK_AMS_MKHE_RNS_NATIVE_TERMINAL_BRIDGE_SECTION_MAX_BYTES_V1,
    terminal_cross_basis_ipa::{
        BRIDGE_POINT_BYTES_V2, BRIDGE_RAW_PROOF_BYTES_V2, BRIDGE_ROWS_V2,
        verify_detached_kernel_prerequisite_v2,
    },
};
use crate::vega::{VegaT256PointV1 as Point, sponge::Keccak256};

const CODEC_TAG_V1: [u8; 4] = *b"ZTCB";
const CODEC_VERSION_V1: u8 = 1;
const CODEC_FLAGS_V1: u8 = 0;
const KERNEL_VERSION_V2: u8 = 2;
const VALUE_COLUMNS_V1: usize = 1_024;
const BASIS_VIEW_V1: usize = VALUE_COLUMNS_V1 + 1;
const CONTEXT_BINDING_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-terminal-cross-basis.context";
const POINT_SET_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.rns-native-terminal-cross-basis.points";
const PROOF_DIGEST_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.rns-native-terminal-cross-basis.proof";
const CODEC_DIGEST_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.rns-native-terminal-cross-basis.codec";
const HYRAX_POINT_ROLE_V1: u8 = 1;
const BP_POINT_ROLE_V1: u8 = 2;
const DIGEST_BYTES_V1: usize = 32;
const CODEC_DIGEST_COUNT_V1: usize = 5;
const HEADER_BYTES_V1: usize =
    4 + 1 + 1 + 2 + 2 + 2 + 1 + 4 + CODEC_DIGEST_COUNT_V1 * DIGEST_BYTES_V1;
const POINT_SET_BYTES_V1: usize = BRIDGE_ROWS_V2 * BRIDGE_POINT_BYTES_V2;
const HYRAX_POINTS_OFFSET_V1: usize = HEADER_BYTES_V1;
const BP_POINTS_OFFSET_V1: usize = HYRAX_POINTS_OFFSET_V1 + POINT_SET_BYTES_V1;
const RAW_PROOF_OFFSET_V1: usize = BP_POINTS_OFFSET_V1 + POINT_SET_BYTES_V1;
const CODEC_DIGEST_OFFSET_V1: usize = RAW_PROOF_OFFSET_V1 + BRIDGE_RAW_PROOF_BYTES_V2;
const EXACT_CODEC_BYTES_V1: usize = CODEC_DIGEST_OFFSET_V1 + DIGEST_BYTES_V1;
#[cfg(test)]
const BINDING_DIGEST_OFFSET_V1: usize = 17;
#[cfg(test)]
const HYRAX_DIGEST_OFFSET_V1: usize = BINDING_DIGEST_OFFSET_V1 + DIGEST_BYTES_V1;
#[cfg(test)]
const BP_DIGEST_OFFSET_V1: usize = HYRAX_DIGEST_OFFSET_V1 + DIGEST_BYTES_V1;
#[cfg(test)]
const EXPECTED_ROOT_OFFSET_V1: usize = BP_DIGEST_OFFSET_V1 + DIGEST_BYTES_V1;
#[cfg(test)]
const PROOF_DIGEST_OFFSET_V1: usize = EXPECTED_ROOT_OFFSET_V1 + DIGEST_BYTES_V1;
const MAX_BOUND_DIGESTS_V1: usize = 160;

const _: () = {
    assert!(BRIDGE_ROWS_V2 == 1_536);
    assert!(BRIDGE_POINT_BYTES_V2 == 33);
    assert!(BRIDGE_RAW_PROOF_BYTES_V2 == 32_866);
    assert!(HEADER_BYTES_V1 == 177);
    assert!(POINT_SET_BYTES_V1 == 50_688);
    assert!(EXACT_CODEC_BYTES_V1 == 134_451);
    assert!(
        EXACT_CODEC_BYTES_V1
            <= ZK_AMS_MKHE_RNS_NATIVE_TERMINAL_BRIDGE_SECTION_MAX_BYTES_V1 as usize
    );
};

/// Failure while decoding or checking the private cross-basis prerequisite.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RnsNativeTerminalCrossBasisErrorV1 {
    /// The nested proof exceeded the terminal section cap.
    CapExceeded,
    /// Fixed-width framing, counts, or trailing bytes were invalid.
    InvalidEncoding,
    /// A context field did not match the exact pre-root transcript.
    ContextMismatch,
    /// A nested or cross-layer semantic digest was zero or aliased.
    AliasedDigest,
    /// A point, proof, or codec digest did not bind its exact bytes.
    Integrity,
    /// A commitment point was identity, noncanonical, or otherwise invalid.
    InvalidPoint,
    /// The first-party representation-equality equation failed.
    InvalidProof,
    /// The verified kernel root did not equal the transcript bridge root.
    RootMismatch,
    /// Exact bounded allocation failed.
    Allocation,
}

impl core::fmt::Display for RnsNativeTerminalCrossBasisErrorV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(match self {
            Self::CapExceeded => "RNS-native terminal cross-basis proof exceeds its cap",
            Self::InvalidEncoding => "invalid RNS-native terminal cross-basis encoding",
            Self::ContextMismatch => "mismatched RNS-native terminal cross-basis context",
            Self::AliasedDigest => "aliased RNS-native terminal cross-basis digest",
            Self::Integrity => "invalid RNS-native terminal cross-basis integrity digest",
            Self::InvalidPoint => "invalid RNS-native terminal cross-basis point",
            Self::InvalidProof => "invalid RNS-native terminal cross-basis proof",
            Self::RootMismatch => "mismatched RNS-native terminal cross-basis root",
            Self::Allocation => "RNS-native terminal cross-basis allocation failed",
        })
    }
}

impl std::error::Error for RnsNativeTerminalCrossBasisErrorV1 {}

/// Move-only evidence that only the detached representation kernel passed.
///
/// The type deliberately exposes no fields or constructor.  It is not the
/// sealed terminal bridge capability and carries no source, packing, proof
/// readiness, or release authority.
#[allow(
    missing_copy_implementations,
    reason = "even a non-authorizing kernel prerequisite is consumed by its next private stage"
)]
pub(super) struct RnsNativeTerminalCrossBasisKernelPrerequisiteV1 {
    binding_digest: [u8; 32],
    hyrax_digest: [u8; 32],
    bp_digest: [u8; 32],
    bridge_root: [u8; 32],
    hyrax_commitments: Vec<Point>,
    bp_commitments: Vec<Point>,
}

impl RnsNativeTerminalCrossBasisKernelPrerequisiteV1 {
    pub(super) const fn binding_digest(&self) -> [u8; 32] {
        self.binding_digest
    }

    pub(super) const fn hyrax_digest(&self) -> [u8; 32] {
        self.hyrax_digest
    }

    pub(super) const fn bp_digest(&self) -> [u8; 32] {
        self.bp_digest
    }

    pub(super) const fn bridge_root(&self) -> [u8; 32] {
        self.bridge_root
    }

    pub(super) fn hyrax_commitments(&self) -> &[Point] {
        &self.hyrax_commitments
    }

    pub(super) fn bp_commitments(&self) -> &[Point] {
        &self.bp_commitments
    }

    pub(super) fn validate_context_v1(
        &self,
        transcript: &ZkAmsMkheRnsNativeChallengeSeedsV1,
    ) -> Result<(), RnsNativeTerminalCrossBasisErrorV1> {
        if self.bridge_root != transcript.cross_basis_bridge_root()
            || self.binding_digest != context_binding_digest_v1(transcript)?
        {
            return Err(RnsNativeTerminalCrossBasisErrorV1::ContextMismatch);
        }
        Ok(())
    }
}

struct DecodedKernelV1<'a> {
    binding_digest: [u8; 32],
    hyrax_digest: [u8; 32],
    bp_digest: [u8; 32],
    expected_root: [u8; 32],
    proof_digest: [u8; 32],
    hyrax_points: &'a [u8],
    bp_points: &'a [u8],
    proof: &'a [u8],
    codec_digest: [u8; 32],
}

struct DecoderV1<'a> {
    bytes: &'a [u8],
    cursor: usize,
}

impl<'a> DecoderV1<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, cursor: 0 }
    }

    fn take(&mut self, count: usize) -> Result<&'a [u8], RnsNativeTerminalCrossBasisErrorV1> {
        let end = self
            .cursor
            .checked_add(count)
            .ok_or(RnsNativeTerminalCrossBasisErrorV1::InvalidEncoding)?;
        let value = self
            .bytes
            .get(self.cursor..end)
            .ok_or(RnsNativeTerminalCrossBasisErrorV1::InvalidEncoding)?;
        self.cursor = end;
        Ok(value)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], RnsNativeTerminalCrossBasisErrorV1> {
        self.take(N)?
            .try_into()
            .map_err(|_| RnsNativeTerminalCrossBasisErrorV1::InvalidEncoding)
    }

    fn u8(&mut self) -> Result<u8, RnsNativeTerminalCrossBasisErrorV1> {
        self.take(1)?
            .first()
            .copied()
            .ok_or(RnsNativeTerminalCrossBasisErrorV1::InvalidEncoding)
    }

    fn u16(&mut self) -> Result<u16, RnsNativeTerminalCrossBasisErrorV1> {
        Ok(u16::from_be_bytes(self.array()?))
    }

    fn u32(&mut self) -> Result<u32, RnsNativeTerminalCrossBasisErrorV1> {
        Ok(u32::from_be_bytes(self.array()?))
    }

    fn finish(self) -> Result<(), RnsNativeTerminalCrossBasisErrorV1> {
        if self.cursor == self.bytes.len() {
            Ok(())
        } else {
            Err(RnsNativeTerminalCrossBasisErrorV1::InvalidEncoding)
        }
    }
}

struct DigestRegistryV1 {
    digests: [[u8; 32]; MAX_BOUND_DIGESTS_V1],
    len: usize,
}

impl DigestRegistryV1 {
    const fn new() -> Self {
        Self {
            digests: [[0; 32]; MAX_BOUND_DIGESTS_V1],
            len: 0,
        }
    }

    fn insert(&mut self, digest: [u8; 32]) -> Result<(), RnsNativeTerminalCrossBasisErrorV1> {
        if digest == [0; 32] || self.digests[..self.len].contains(&digest) {
            return Err(RnsNativeTerminalCrossBasisErrorV1::AliasedDigest);
        }
        let destination = self
            .digests
            .get_mut(self.len)
            .ok_or(RnsNativeTerminalCrossBasisErrorV1::AliasedDigest)?;
        *destination = digest;
        self.len += 1;
        Ok(())
    }
}

/// Decode and verify the exact terminal representation-equality prerequisite.
///
/// The outer cap and exact fixed width are checked before either point vector
/// is allocated.  The resulting move-only token proves only the detached
/// kernel; mapping, terminal materialization, source, and packing remain
/// unavailable stages.
pub(super) fn authenticate_rns_native_terminal_cross_basis_kernel_v1(
    transcript: &ZkAmsMkheRnsNativeChallengeSeedsV1,
    bytes: &[u8],
) -> Result<RnsNativeTerminalCrossBasisKernelPrerequisiteV1, RnsNativeTerminalCrossBasisErrorV1> {
    let decoded = decode_exact_v1(bytes)?;
    let expected_binding = context_binding_digest_v1(transcript)?;
    if decoded.binding_digest != expected_binding
        || decoded.expected_root != transcript.cross_basis_bridge_root()
    {
        return Err(RnsNativeTerminalCrossBasisErrorV1::ContextMismatch);
    }
    if decoded.hyrax_digest != point_set_digest_v1(HYRAX_POINT_ROLE_V1, decoded.hyrax_points)?
        || decoded.bp_digest != point_set_digest_v1(BP_POINT_ROLE_V1, decoded.bp_points)?
        || decoded.proof_digest != proof_digest_v1(decoded.proof)?
        || decoded.codec_digest != codec_digest_v1(&bytes[..CODEC_DIGEST_OFFSET_V1])
    {
        return Err(RnsNativeTerminalCrossBasisErrorV1::Integrity);
    }
    validate_global_digest_aliases_v1(transcript, &decoded)?;
    let hyrax = decode_points_v1(decoded.hyrax_points)?;
    let bp = decode_points_v1(decoded.bp_points)?;
    let root =
        verify_detached_kernel_prerequisite_v2(decoded.binding_digest, &hyrax, &bp, decoded.proof)
            .map_err(|_| RnsNativeTerminalCrossBasisErrorV1::InvalidProof)?;
    if root != decoded.expected_root {
        return Err(RnsNativeTerminalCrossBasisErrorV1::RootMismatch);
    }
    Ok(RnsNativeTerminalCrossBasisKernelPrerequisiteV1 {
        binding_digest: decoded.binding_digest,
        hyrax_digest: decoded.hyrax_digest,
        bp_digest: decoded.bp_digest,
        bridge_root: root,
        hyrax_commitments: hyrax,
        bp_commitments: bp,
    })
}

fn decode_exact_v1(
    bytes: &[u8],
) -> Result<DecodedKernelV1<'_>, RnsNativeTerminalCrossBasisErrorV1> {
    if bytes.len()
        > usize::try_from(ZK_AMS_MKHE_RNS_NATIVE_TERMINAL_BRIDGE_SECTION_MAX_BYTES_V1)
            .map_err(|_| RnsNativeTerminalCrossBasisErrorV1::CapExceeded)?
    {
        return Err(RnsNativeTerminalCrossBasisErrorV1::CapExceeded);
    }
    if bytes.len() != EXACT_CODEC_BYTES_V1 {
        return Err(RnsNativeTerminalCrossBasisErrorV1::InvalidEncoding);
    }
    let mut decoder = DecoderV1::new(bytes);
    if decoder.array::<4>()? != CODEC_TAG_V1
        || decoder.u8()? != CODEC_VERSION_V1
        || decoder.u8()? != CODEC_FLAGS_V1
        || usize::from(decoder.u16()?) != BRIDGE_ROWS_V2
        || usize::from(decoder.u16()?) != VALUE_COLUMNS_V1
        || usize::from(decoder.u16()?) != BASIS_VIEW_V1
        || usize::from(decoder.u8()?) != BRIDGE_POINT_BYTES_V2
        || usize::try_from(decoder.u32()?).ok() != Some(BRIDGE_RAW_PROOF_BYTES_V2)
    {
        return Err(RnsNativeTerminalCrossBasisErrorV1::InvalidEncoding);
    }
    let binding_digest = decoder.array()?;
    let hyrax_digest = decoder.array()?;
    let bp_digest = decoder.array()?;
    let expected_root = decoder.array()?;
    let proof_digest = decoder.array()?;
    let hyrax_points = decoder.take(POINT_SET_BYTES_V1)?;
    let bp_points = decoder.take(POINT_SET_BYTES_V1)?;
    let proof = decoder.take(BRIDGE_RAW_PROOF_BYTES_V2)?;
    let codec_digest = decoder.array()?;
    decoder.finish()?;
    Ok(DecodedKernelV1 {
        binding_digest,
        hyrax_digest,
        bp_digest,
        expected_root,
        proof_digest,
        hyrax_points,
        bp_points,
        proof,
        codec_digest,
    })
}

fn decode_points_v1(bytes: &[u8]) -> Result<Vec<Point>, RnsNativeTerminalCrossBasisErrorV1> {
    if bytes.len() != POINT_SET_BYTES_V1 {
        return Err(RnsNativeTerminalCrossBasisErrorV1::InvalidEncoding);
    }
    let mut points = Vec::new();
    points
        .try_reserve_exact(BRIDGE_ROWS_V2)
        .map_err(|_| RnsNativeTerminalCrossBasisErrorV1::Allocation)?;
    for encoded in bytes.chunks_exact(BRIDGE_POINT_BYTES_V2) {
        points.push(
            Point::from_non_identity_wire_bytes_exact(encoded)
                .map_err(|_| RnsNativeTerminalCrossBasisErrorV1::InvalidPoint)?,
        );
    }
    if points.len() != BRIDGE_ROWS_V2 {
        return Err(RnsNativeTerminalCrossBasisErrorV1::InvalidEncoding);
    }
    Ok(points)
}

fn context_binding_digest_v1(
    transcript: &ZkAmsMkheRnsNativeChallengeSeedsV1,
) -> Result<[u8; 32], RnsNativeTerminalCrossBasisErrorV1> {
    let mut hash = Keccak256::new();
    hash.update(CONTEXT_BINDING_DOMAIN_V1);
    hash.update(&CODEC_TAG_V1);
    hash.update(&[CODEC_VERSION_V1, KERNEL_VERSION_V2]);
    hash.update(
        &u16::try_from(BRIDGE_ROWS_V2)
            .map_err(|_| RnsNativeTerminalCrossBasisErrorV1::InvalidEncoding)?
            .to_be_bytes(),
    );
    hash.update(
        &u16::try_from(VALUE_COLUMNS_V1)
            .map_err(|_| RnsNativeTerminalCrossBasisErrorV1::InvalidEncoding)?
            .to_be_bytes(),
    );
    hash.update(
        &u16::try_from(BASIS_VIEW_V1)
            .map_err(|_| RnsNativeTerminalCrossBasisErrorV1::InvalidEncoding)?
            .to_be_bytes(),
    );
    hash.update(&[BRIDGE_POINT_BYTES_V2 as u8]);
    hash.update(
        &u32::try_from(BRIDGE_RAW_PROOF_BYTES_V2)
            .map_err(|_| RnsNativeTerminalCrossBasisErrorV1::InvalidEncoding)?
            .to_be_bytes(),
    );
    for digest in context_identities_v1(transcript) {
        hash.update(&digest);
    }
    hash.update(
        &u16::try_from(transcript.opening_commitments().len())
            .map_err(|_| RnsNativeTerminalCrossBasisErrorV1::InvalidEncoding)?
            .to_be_bytes(),
    );
    for (ordinal, opening) in transcript.opening_commitments().iter().copied().enumerate() {
        hash.update(
            &u16::try_from(ordinal)
                .map_err(|_| RnsNativeTerminalCrossBasisErrorV1::InvalidEncoding)?
                .to_be_bytes(),
        );
        hash.update(&[opening.family() as u8, opening.family_index()]);
        hash.update(&opening.source_commitment_digest());
        hash.update(&opening.hyrax_commitment_digest());
    }
    for digest in [
        transcript.mapping_challenge_seed(),
        transcript.cross_basis_challenge_seed(),
        transcript.mapping_root(),
        transcript.terminal_hyrax_root(),
    ] {
        hash.update(&digest);
    }
    let digest = hash.finalize();
    if digest == [0; 32] {
        return Err(RnsNativeTerminalCrossBasisErrorV1::ContextMismatch);
    }
    Ok(digest)
}

fn context_identities_v1(transcript: &ZkAmsMkheRnsNativeChallengeSeedsV1) -> [[u8; 32]; 12] {
    [
        transcript.profile_manifest_digest(),
        transcript.profile_digest(),
        transcript.topology_digest(),
        transcript.release_candidate_digest(),
        transcript.statement_digest(),
        transcript.operational_context_digest(),
        transcript.source_binding_digest(),
        transcript.main_snapshot_digest(),
        transcript.nonce_snapshot_digest(),
        transcript.source_receipt_digest(),
        transcript.governed_roster_digest(),
        transcript.public_ciphertext_digest(),
    ]
}

fn point_set_digest_v1(
    role: u8,
    bytes: &[u8],
) -> Result<[u8; 32], RnsNativeTerminalCrossBasisErrorV1> {
    if !matches!(role, HYRAX_POINT_ROLE_V1 | BP_POINT_ROLE_V1) || bytes.len() != POINT_SET_BYTES_V1
    {
        return Err(RnsNativeTerminalCrossBasisErrorV1::InvalidEncoding);
    }
    let mut hash = Keccak256::new();
    hash.update(POINT_SET_DOMAIN_V1);
    hash.update(&[CODEC_VERSION_V1, role]);
    hash.update(
        &u16::try_from(BRIDGE_ROWS_V2)
            .map_err(|_| RnsNativeTerminalCrossBasisErrorV1::InvalidEncoding)?
            .to_be_bytes(),
    );
    for (ordinal, point) in bytes.chunks_exact(BRIDGE_POINT_BYTES_V2).enumerate() {
        hash.update(
            &u16::try_from(ordinal)
                .map_err(|_| RnsNativeTerminalCrossBasisErrorV1::InvalidEncoding)?
                .to_be_bytes(),
        );
        hash.update(point);
    }
    Ok(hash.finalize())
}

fn proof_digest_v1(proof: &[u8]) -> Result<[u8; 32], RnsNativeTerminalCrossBasisErrorV1> {
    if proof.len() != BRIDGE_RAW_PROOF_BYTES_V2 {
        return Err(RnsNativeTerminalCrossBasisErrorV1::InvalidEncoding);
    }
    let mut hash = Keccak256::new();
    hash.update(PROOF_DIGEST_DOMAIN_V1);
    hash.update(&[CODEC_VERSION_V1, KERNEL_VERSION_V2]);
    hash.update(
        &u32::try_from(proof.len())
            .map_err(|_| RnsNativeTerminalCrossBasisErrorV1::InvalidEncoding)?
            .to_be_bytes(),
    );
    hash.update(proof);
    Ok(hash.finalize())
}

fn codec_digest_v1(bytes: &[u8]) -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(CODEC_DIGEST_DOMAIN_V1);
    hash.update(&[CODEC_VERSION_V1]);
    hash.update(bytes);
    hash.finalize()
}

fn validate_global_digest_aliases_v1(
    transcript: &ZkAmsMkheRnsNativeChallengeSeedsV1,
    decoded: &DecodedKernelV1<'_>,
) -> Result<(), RnsNativeTerminalCrossBasisErrorV1> {
    let mut registry = DigestRegistryV1::new();
    for digest in context_identities_v1(transcript) {
        registry.insert(digest)?;
    }
    for opening in transcript.opening_commitments().iter().copied() {
        registry.insert(opening.source_commitment_digest())?;
        registry.insert(opening.hyrax_commitment_digest())?;
    }
    for digest in [
        transcript.mapping_root(),
        transcript.terminal_hyrax_root(),
        transcript.cross_basis_bridge_root(),
        transcript.qpcs_initial_root(),
        transcript.qpcs_quotient_root(),
        transcript.cross_field_root(),
        transcript.global_lookup_root(),
        transcript.zero_padding_root(),
    ] {
        registry.insert(digest)?;
    }
    for root in transcript.qpcs_fri_roots() {
        registry.insert(root.root())?;
    }
    for seed in transcript.ordered_challenge_seeds() {
        registry.insert(seed)?;
    }
    registry.insert(transcript.transcript_digest())?;
    for digest in [
        decoded.binding_digest,
        decoded.hyrax_digest,
        decoded.bp_digest,
        decoded.proof_digest,
        decoded.codec_digest,
    ] {
        registry.insert(digest)?;
    }
    Ok(())
}

#[cfg(test)]
fn encode_kernel_v1(
    transcript: &ZkAmsMkheRnsNativeChallengeSeedsV1,
    hyrax_commitments: &[Point],
    bp_commitments: &[Point],
    proof: &[u8],
) -> Result<Vec<u8>, RnsNativeTerminalCrossBasisErrorV1> {
    if hyrax_commitments.len() != BRIDGE_ROWS_V2
        || bp_commitments.len() != BRIDGE_ROWS_V2
        || proof.len() != BRIDGE_RAW_PROOF_BYTES_V2
    {
        return Err(RnsNativeTerminalCrossBasisErrorV1::InvalidEncoding);
    }
    let mut hyrax_bytes = Vec::new();
    let mut bp_bytes = Vec::new();
    hyrax_bytes
        .try_reserve_exact(POINT_SET_BYTES_V1)
        .map_err(|_| RnsNativeTerminalCrossBasisErrorV1::Allocation)?;
    bp_bytes
        .try_reserve_exact(POINT_SET_BYTES_V1)
        .map_err(|_| RnsNativeTerminalCrossBasisErrorV1::Allocation)?;
    for point in hyrax_commitments {
        hyrax_bytes.extend_from_slice(
            &point
                .to_non_identity_wire_bytes()
                .map_err(|_| RnsNativeTerminalCrossBasisErrorV1::InvalidPoint)?,
        );
    }
    for point in bp_commitments {
        bp_bytes.extend_from_slice(
            &point
                .to_non_identity_wire_bytes()
                .map_err(|_| RnsNativeTerminalCrossBasisErrorV1::InvalidPoint)?,
        );
    }
    let binding_digest = context_binding_digest_v1(transcript)?;
    let expected_root = transcript.cross_basis_bridge_root();
    let verified_root = verify_detached_kernel_prerequisite_v2(
        binding_digest,
        hyrax_commitments,
        bp_commitments,
        proof,
    )
    .map_err(|_| RnsNativeTerminalCrossBasisErrorV1::InvalidProof)?;
    if verified_root != expected_root {
        return Err(RnsNativeTerminalCrossBasisErrorV1::RootMismatch);
    }
    let hyrax_digest = point_set_digest_v1(HYRAX_POINT_ROLE_V1, &hyrax_bytes)?;
    let bp_digest = point_set_digest_v1(BP_POINT_ROLE_V1, &bp_bytes)?;
    let proof_digest = proof_digest_v1(proof)?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(EXACT_CODEC_BYTES_V1)
        .map_err(|_| RnsNativeTerminalCrossBasisErrorV1::Allocation)?;
    bytes.extend_from_slice(&CODEC_TAG_V1);
    bytes.extend_from_slice(&[CODEC_VERSION_V1, CODEC_FLAGS_V1]);
    bytes.extend_from_slice(&(BRIDGE_ROWS_V2 as u16).to_be_bytes());
    bytes.extend_from_slice(&(VALUE_COLUMNS_V1 as u16).to_be_bytes());
    bytes.extend_from_slice(&(BASIS_VIEW_V1 as u16).to_be_bytes());
    bytes.push(BRIDGE_POINT_BYTES_V2 as u8);
    bytes.extend_from_slice(&(BRIDGE_RAW_PROOF_BYTES_V2 as u32).to_be_bytes());
    for digest in [
        binding_digest,
        hyrax_digest,
        bp_digest,
        expected_root,
        proof_digest,
    ] {
        bytes.extend_from_slice(&digest);
    }
    bytes.extend_from_slice(&hyrax_bytes);
    bytes.extend_from_slice(&bp_bytes);
    bytes.extend_from_slice(proof);
    if bytes.len() != CODEC_DIGEST_OFFSET_V1 {
        return Err(RnsNativeTerminalCrossBasisErrorV1::InvalidEncoding);
    }
    let codec_digest = codec_digest_v1(&bytes);
    bytes.extend_from_slice(&codec_digest);
    if bytes.len() != EXACT_CODEC_BYTES_V1 {
        return Err(RnsNativeTerminalCrossBasisErrorV1::InvalidEncoding);
    }
    authenticate_rns_native_terminal_cross_basis_kernel_v1(transcript, &bytes)?;
    Ok(bytes)
}

#[cfg(test)]
#[path = "rns_native_terminal_cross_basis_tests.rs"]
mod tests;
