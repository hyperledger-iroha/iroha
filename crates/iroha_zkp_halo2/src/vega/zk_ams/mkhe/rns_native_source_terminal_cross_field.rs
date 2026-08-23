//! Private authenticated-source to terminal/cross-field linkage prerequisite.
//!
//! This stage first replays the canonical packed `X` source record under its
//! exact 89-used-slot layout, so the authenticated live snapshot itself must
//! have zeroes in every governed `X` padding slot. It then replays the packed
//! `E16/rE/W8/rW` source records, maps their 1,536 rows and blindings to the
//! retained Hyrax terminal commitments, and checks one transcript-derived
//! random linear combination under the exact 1,025-point commitment basis.
//! The detached 7,640-point zero-padding token remains only a redundant,
//! non-authoritative compatibility input pending schema retirement. This stage
//! also exact-decodes the post-RLWE residual anchor that freezes all five point,
//! forty limb, and twenty-nine lookup-round identities of the cross-field
//! section.
//!
//! The cross-field proof body remains opaque here.  Consequently the output
//! is only a move-only construction prerequisite: it is not proof-validity,
//! readiness, or release authority, and the composite verifier remains
//! fail-closed at the cross-field/global-lookup stage.

use super::{
    manifest::{ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1, ZK_AMS_MKHE_RELEASE_SLOT_COUNT_V1},
    packing::{
        T256PackedPlaintextDecodeWorkspaceV1,
        visit_rehydrated_t256_coefficients_used_slots_with_workspace_v1,
        zk_ams_t256_packing_layout_v1,
    },
    rns_native_profile::{
        ZK_AMS_MKHE_RNS_NATIVE_CROSS_FIELD_POINT_COUNT_V1, ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1,
        ZK_AMS_MKHE_RNS_NATIVE_OPENING_COUNT_V1, ZK_AMS_MKHE_RNS_NATIVE_SUMCHECK_ROUNDS_V1,
        ZkAmsMkheRnsNativeFamilyV1,
    },
    rns_native_rlwe_source_statement::{
        RNS_NATIVE_RLWE_SOURCE_DOWNSTREAM_MAX_BYTES_V1, RnsNativeRlweSourceStatementStageV1,
    },
    rns_native_section_codec::ZkAmsMkheRnsNativeCrossFieldGlobalLookupSectionV1,
    rns_native_source::{
        ZK_AMS_MKHE_RNS_NATIVE_SOURCE_MAIN_BLOCKS_PER_OPENING_V1,
        ZK_AMS_MKHE_RNS_NATIVE_SOURCE_MAIN_PLAINTEXT_BYTES_V1, ZkAmsMkheRnsNativeSecretChunkV1,
        ZkAmsMkheRnsNativeSourceArenaV1, ZkAmsMkheRnsNativeSourceSnapshotV1,
    },
    rns_native_terminal_cross_basis::RnsNativeTerminalCrossBasisKernelPrerequisiteV1,
    rns_native_transcript::{
        ZkAmsMkheRnsNativeChallengeSeedsV1, ZkAmsMkheRnsNativeOpeningCommitmentV1,
    },
    rns_native_zero_padding_commitment::RnsNativeZeroPaddingCommitmentPrerequisiteV1,
};
use crate::{
    generalized_bulletproof::{SecretMultiexpBuilder, multiexp, try_exact_capacity_vec_v1},
    vega::{
        VegaT256PointV1 as Point, VegaT256ScalarV1 as Scalar,
        bulletproof_t256::{
            ZeroizingT256ScalarCopyV1, ZeroizingT256ScalarVecV1, ZkAmsT256BulletproofSuiteV1,
        },
        commitment::CommitmentKey,
        sponge::Keccak256,
    },
};

const LINK_VERSION_V1: u8 = 1;
const DIGEST_BYTES_V1: usize = 32;
const POINT_BYTES_V1: usize = 33;
const TERMINAL_ROWS_V1: usize = 1_536;
const ERROR_ROWS_V1: usize = 1_024;
const WITNESS_ROWS_V1: usize = 512;
const TERMINAL_COLUMNS_V1: usize = 1_024;
const COMMITMENT_BASIS_V1: usize = TERMINAL_COLUMNS_V1 + 1;
const ROWS_PER_FULL_RECORD_V1: usize = 64;
const CANONICAL_BLOCKS_PER_RECORD_V1: usize = 512;
const CANONICAL_COEFFICIENT_BYTES_V1: usize = 32;
const CANONICAL_COEFFICIENTS_PER_BLOCK_V1: usize =
    ZK_AMS_MKHE_RNS_NATIVE_SOURCE_MAIN_PLAINTEXT_BYTES_V1 as usize / CANONICAL_COEFFICIENT_BYTES_V1;
const X_RECORD_V1: usize = 0;
const X_USED_SLOTS_V1: u32 = 89;
const E_FIRST_RECORD_V1: usize = 17;
const E_RECORDS_V1: usize = 16;
const RE_RECORD_V1: usize = 33;
const W_FIRST_RECORD_V1: usize = 34;
const W_RECORDS_V1: usize = 8;
const RW_RECORD_V1: usize = 42;
const RE_USED_SLOTS_V1: u32 = 1_024;
const RW_USED_SLOTS_V1: u32 = 512;
const MAX_CHALLENGE_ATTEMPTS_V1: u16 = 128;

const ANCHOR_MAGIC_V1: [u8; 4] = *b"ZSTL";
const ANCHOR_FLAGS_V1: u8 = 0;
const ANCHOR_CORE_DIGESTS_V1: usize = 16;
const CORE_PRIOR_STATEMENT_V1: usize = 0;
const CORE_PRIOR_RECORD_MAPPING_V1: usize = 1;
const CORE_TERMINAL_MAPPING_FORMULA_V1: usize = 2;
const CORE_HYRAX_POINTS_V1: usize = 3;
const CORE_BP_POINTS_V1: usize = 4;
const CORE_OPENING_HYRAX_BUNDLE_V1: usize = 5;
const CORE_MAPPING_ROOT_V1: usize = 6;
const CORE_TERMINAL_HYRAX_ROOT_V1: usize = 7;
const CORE_CROSS_POINT_BUNDLE_V1: usize = 8;
const CORE_CROSS_LIMB_BUNDLE_V1: usize = 9;
const CORE_LOOKUP_ROUND_BUNDLE_V1: usize = 10;
const CORE_CROSS_PROOF_V1: usize = 11;
const CORE_CROSS_ROOT_V1: usize = 12;
const CORE_GLOBAL_ROOT_V1: usize = 13;
const CORE_CROSS_LINK_V1: usize = 14;
const CORE_DOWNSTREAM_V1: usize = 15;
const ANCHOR_HEADER_BYTES_V1: usize = 4 + 1 + 1 + 3 + 1 + 2 + 2 + 1 + 1 + 4;
const ANCHOR_FIXED_BYTES_V1: usize =
    ANCHOR_HEADER_BYTES_V1 + ANCHOR_CORE_DIGESTS_V1 * DIGEST_BYTES_V1;
const LINK_DOWNSTREAM_MAX_BYTES_V1: usize =
    RNS_NATIVE_RLWE_SOURCE_DOWNSTREAM_MAX_BYTES_V1 - ANCHOR_FIXED_BYTES_V1;
const MAX_BOUND_DIGESTS_V1: usize = 384;

const FORMULA_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.rns-native-source-terminal.formula";
const OPENING_DIGEST_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.rns-native-source-terminal.opening";
const OPENING_BUNDLE_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-source-terminal.opening-bundle";
const CHALLENGE_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.rns-native-source-terminal.row-challenge";
const MAPPING_ROOT_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-source-terminal.mapping-root";
const TERMINAL_ROOT_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-source-terminal.hyrax-root";
const POINT_BUNDLE_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-source-terminal.cross-points";
const LIMB_BUNDLE_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.rns-native-source-terminal.cross-limbs";
const ROUND_BUNDLE_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-source-terminal.lookup-rounds";
const ZERO_LIMB_BUNDLE_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-source-terminal.zero-padding-limbs";
const CROSS_PROOF_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.rns-native-source-terminal.cross-proof";
const CROSS_LINK_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.rns-native-source-terminal.cross-link";
const DOWNSTREAM_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.rns-native-source-terminal.downstream";
const ANCHOR_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.rns-native-source-terminal.anchor";
const AGGREGATE_POINT_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-source-terminal.aggregate-point";

const SOURCE_MAPPING_FORMULA_V1: &[u8] = b"E[i][s]->row=i*64+s/1024,col=s%1024;rE[s]->blind[row=s];W[i][s]->row=1024+i*64+s/1024,col=s%1024;rW[s]->blind[row=1024+s]";
const SOURCE_PACKING_FORMULA_V1: &[u8] = b"canonical-131072-coefficient-T256-packed-polynomial;inverse-quadratic-factor-NTT;used-slots-only";
const SOURCE_PADDING_FORMULA_V1: &[u8] =
    b"X:used-slots=89;all-slots-from-89-through-65535-must-be-zero-on-live-source-snapshot";
const BATCH_FORMULA_V1: &[u8] =
    b"sum_row eta^row*(sum_col value[row,col]*G[col]+blind[row]*H);eta!=0,1";
const OPENING_SLICE_FORMULA_V1: &[u8] =
    b"X/U:role-only;E:64-row-slice;rE:error-1024-row-set;W:64-row-slice;rW:witness-512-row-set";

const _: () = {
    assert!(ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1 == 131_072);
    assert!(ZK_AMS_MKHE_RELEASE_SLOT_COUNT_V1 == 65_536);
    assert!(X_RECORD_V1 == 0);
    assert!(X_USED_SLOTS_V1 == 89);
    assert!(CANONICAL_COEFFICIENTS_PER_BLOCK_V1 == 256);
    assert!(CANONICAL_BLOCKS_PER_RECORD_V1 == 512);
    assert!(TERMINAL_ROWS_V1 == ERROR_ROWS_V1 + WITNESS_ROWS_V1);
    assert!(E_RECORDS_V1 * ROWS_PER_FULL_RECORD_V1 == ERROR_ROWS_V1);
    assert!(W_RECORDS_V1 * ROWS_PER_FULL_RECORD_V1 == WITNESS_ROWS_V1);
    assert!(ZK_AMS_MKHE_RNS_NATIVE_OPENING_COUNT_V1 == 43);
    assert!(ZK_AMS_MKHE_RNS_NATIVE_CROSS_FIELD_POINT_COUNT_V1 == 5);
    assert!(ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1 == 40);
    assert!(ZK_AMS_MKHE_RNS_NATIVE_SUMCHECK_ROUNDS_V1 == 29);
    assert!(ANCHOR_HEADER_BYTES_V1 == 20);
    assert!(ANCHOR_FIXED_BYTES_V1 == 532);
    assert!(LINK_DOWNSTREAM_MAX_BYTES_V1 == 3_251);
};

/// Failure while constructing the source/terminal/cross-field prerequisite.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RnsNativeSourceTerminalCrossFieldErrorV1 {
    InvalidContext,
    InvalidGeometry,
    InvalidAnchor,
    AnchorCapExceeded,
    AliasedDigest,
    InvalidSource,
    InvalidPacking,
    InvalidPoint,
    InvalidScalar,
    InvalidMapping,
    InvalidCrossFieldBinding,
    Allocation,
}

impl core::fmt::Display for RnsNativeSourceTerminalCrossFieldErrorV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for RnsNativeSourceTerminalCrossFieldErrorV1 {}

/// Move-only non-authorizing construction state after exact source mapping.
#[allow(
    dead_code,
    missing_copy_implementations,
    reason = "the future concrete cross-field/global-lookup verifier will consume this prerequisite"
)]
pub(super) struct RnsNativeSourceTerminalCrossFieldPrerequisiteV1<
    'a,
    S: ZkAmsMkheRnsNativeSourceSnapshotV1,
> {
    source: RnsNativeRlweSourceStatementStageV1<'a, S>,
    terminal: RnsNativeTerminalCrossBasisKernelPrerequisiteV1,
    zero_padding: RnsNativeZeroPaddingCommitmentPrerequisiteV1,
    downstream: &'a [u8],
    formula_digest: [u8; DIGEST_BYTES_V1],
    opening_bundle_digest: [u8; DIGEST_BYTES_V1],
    aggregate_point_digest: [u8; DIGEST_BYTES_V1],
    point_bundle_digest: [u8; DIGEST_BYTES_V1],
    limb_bundle_digest: [u8; DIGEST_BYTES_V1],
    round_bundle_digest: [u8; DIGEST_BYTES_V1],
    zero_limb_bundle_digest: [u8; DIGEST_BYTES_V1],
    cross_proof_digest: [u8; DIGEST_BYTES_V1],
    cross_link_digest: [u8; DIGEST_BYTES_V1],
    anchor_digest: [u8; DIGEST_BYTES_V1],
}

#[allow(
    dead_code,
    reason = "retained bindings are consumed by the future private cross-field verifier"
)]
impl<'a, S: ZkAmsMkheRnsNativeSourceSnapshotV1>
    RnsNativeSourceTerminalCrossFieldPrerequisiteV1<'a, S>
{
    pub(super) const fn downstream(&self) -> &'a [u8] {
        self.downstream
    }

    pub(super) const fn formula_digest(&self) -> [u8; DIGEST_BYTES_V1] {
        self.formula_digest
    }

    pub(super) const fn opening_bundle_digest(&self) -> [u8; DIGEST_BYTES_V1] {
        self.opening_bundle_digest
    }

    pub(super) const fn aggregate_point_digest(&self) -> [u8; DIGEST_BYTES_V1] {
        self.aggregate_point_digest
    }

    pub(super) const fn point_bundle_digest(&self) -> [u8; DIGEST_BYTES_V1] {
        self.point_bundle_digest
    }

    pub(super) const fn limb_bundle_digest(&self) -> [u8; DIGEST_BYTES_V1] {
        self.limb_bundle_digest
    }

    pub(super) const fn round_bundle_digest(&self) -> [u8; DIGEST_BYTES_V1] {
        self.round_bundle_digest
    }

    pub(super) const fn zero_limb_bundle_digest(&self) -> [u8; DIGEST_BYTES_V1] {
        self.zero_limb_bundle_digest
    }

    pub(super) const fn cross_proof_digest(&self) -> [u8; DIGEST_BYTES_V1] {
        self.cross_proof_digest
    }

    pub(super) const fn cross_link_digest(&self) -> [u8; DIGEST_BYTES_V1] {
        self.cross_link_digest
    }

    pub(super) const fn anchor_digest(&self) -> [u8; DIGEST_BYTES_V1] {
        self.anchor_digest
    }

    pub(super) const fn source(&self) -> &RnsNativeRlweSourceStatementStageV1<'a, S> {
        &self.source
    }

    pub(super) const fn terminal(&self) -> &RnsNativeTerminalCrossBasisKernelPrerequisiteV1 {
        &self.terminal
    }

    pub(super) const fn zero_padding(&self) -> &RnsNativeZeroPaddingCommitmentPrerequisiteV1 {
        &self.zero_padding
    }

    /// Re-authenticate the exact cross-field section retained only by digest.
    ///
    /// The following proof-body codec calls this before borrowing any nested
    /// commitment bytes, preventing a same-transcript section substitution.
    pub(super) fn validate_cross_section_v1(
        &self,
        cross: ZkAmsMkheRnsNativeCrossFieldGlobalLookupSectionV1<'_>,
    ) -> Result<(), RnsNativeSourceTerminalCrossFieldErrorV1> {
        if indexed_digest_bundle_v1(POINT_BUNDLE_DOMAIN_V1, cross.point_evaluation_digests())?
            != self.point_bundle_digest
            || indexed_digest_bundle_v1(LIMB_BUNDLE_DOMAIN_V1, cross.limb_relation_digests())?
                != self.limb_bundle_digest
            || indexed_digest_bundle_v1(ROUND_BUNDLE_DOMAIN_V1, cross.sumcheck_round_digests())?
                != self.round_bundle_digest
            || cross_proof_digest_v1(cross.proof())? != self.cross_proof_digest
        {
            return Err(RnsNativeSourceTerminalCrossFieldErrorV1::InvalidCrossFieldBinding);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ResidualAnchorV1<'a> {
    core: [[u8; DIGEST_BYTES_V1]; ANCHOR_CORE_DIGESTS_V1],
    downstream: &'a [u8],
}

impl<'a> ResidualAnchorV1<'a> {
    fn from_canonical_bytes_exact_v1(
        bytes: &'a [u8],
    ) -> Result<Self, RnsNativeSourceTerminalCrossFieldErrorV1> {
        if bytes.len() > RNS_NATIVE_RLWE_SOURCE_DOWNSTREAM_MAX_BYTES_V1 {
            return Err(RnsNativeSourceTerminalCrossFieldErrorV1::AnchorCapExceeded);
        }
        if bytes.len() < ANCHOR_FIXED_BYTES_V1 + 1 {
            return Err(RnsNativeSourceTerminalCrossFieldErrorV1::InvalidAnchor);
        }
        let mut decoder = DecoderV1::new(bytes);
        if decoder.array::<4>()? != ANCHOR_MAGIC_V1
            || decoder.u8()? != LINK_VERSION_V1
            || decoder.u8()? != ANCHOR_FLAGS_V1
            || decoder.u8()? != ZK_AMS_MKHE_RNS_NATIVE_CROSS_FIELD_POINT_COUNT_V1
            || usize::from(decoder.u8()?) != ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1
            || decoder.u8()? != ZK_AMS_MKHE_RNS_NATIVE_SUMCHECK_ROUNDS_V1
            || usize::from(decoder.u8()?) != ANCHOR_CORE_DIGESTS_V1
            || usize::from(decoder.u16()?) != TERMINAL_ROWS_V1
            || usize::from(decoder.u16()?) != TERMINAL_COLUMNS_V1
            || decoder.u8()? != 4
            || decoder.u8()? != 0
        {
            return Err(RnsNativeSourceTerminalCrossFieldErrorV1::InvalidAnchor);
        }
        let downstream_len = usize::try_from(decoder.u32()?)
            .map_err(|_| RnsNativeSourceTerminalCrossFieldErrorV1::InvalidAnchor)?;
        if downstream_len == 0 || downstream_len > LINK_DOWNSTREAM_MAX_BYTES_V1 {
            return Err(RnsNativeSourceTerminalCrossFieldErrorV1::InvalidAnchor);
        }
        let mut core = [[0_u8; DIGEST_BYTES_V1]; ANCHOR_CORE_DIGESTS_V1];
        for digest in &mut core {
            *digest = decoder.array()?;
        }
        let downstream = decoder.take(downstream_len)?;
        decoder.finish()?;
        let mut registry = DigestRegistryV1::new();
        for digest in core {
            registry.insert(digest)?;
        }
        if core[CORE_DOWNSTREAM_V1] != downstream_digest_v1(downstream) {
            return Err(RnsNativeSourceTerminalCrossFieldErrorV1::InvalidAnchor);
        }
        Ok(Self { core, downstream })
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

    fn take(&mut self, count: usize) -> Result<&'a [u8], RnsNativeSourceTerminalCrossFieldErrorV1> {
        let end = self
            .cursor
            .checked_add(count)
            .ok_or(RnsNativeSourceTerminalCrossFieldErrorV1::InvalidAnchor)?;
        let value = self
            .bytes
            .get(self.cursor..end)
            .ok_or(RnsNativeSourceTerminalCrossFieldErrorV1::InvalidAnchor)?;
        self.cursor = end;
        Ok(value)
    }

    fn array<const N: usize>(
        &mut self,
    ) -> Result<[u8; N], RnsNativeSourceTerminalCrossFieldErrorV1> {
        self.take(N)?
            .try_into()
            .map_err(|_| RnsNativeSourceTerminalCrossFieldErrorV1::InvalidAnchor)
    }

    fn u8(&mut self) -> Result<u8, RnsNativeSourceTerminalCrossFieldErrorV1> {
        self.take(1)?
            .first()
            .copied()
            .ok_or(RnsNativeSourceTerminalCrossFieldErrorV1::InvalidAnchor)
    }

    fn u16(&mut self) -> Result<u16, RnsNativeSourceTerminalCrossFieldErrorV1> {
        Ok(u16::from_be_bytes(self.array()?))
    }

    fn u32(&mut self) -> Result<u32, RnsNativeSourceTerminalCrossFieldErrorV1> {
        Ok(u32::from_be_bytes(self.array()?))
    }

    fn finish(self) -> Result<(), RnsNativeSourceTerminalCrossFieldErrorV1> {
        if self.cursor == self.bytes.len() {
            Ok(())
        } else {
            Err(RnsNativeSourceTerminalCrossFieldErrorV1::InvalidAnchor)
        }
    }
}

struct DigestRegistryV1 {
    values: [[u8; DIGEST_BYTES_V1]; MAX_BOUND_DIGESTS_V1],
    len: usize,
}

impl DigestRegistryV1 {
    const fn new() -> Self {
        Self {
            values: [[0; DIGEST_BYTES_V1]; MAX_BOUND_DIGESTS_V1],
            len: 0,
        }
    }

    fn insert(
        &mut self,
        digest: [u8; DIGEST_BYTES_V1],
    ) -> Result<(), RnsNativeSourceTerminalCrossFieldErrorV1> {
        if digest == [0; DIGEST_BYTES_V1] || self.values[..self.len].contains(&digest) {
            return Err(RnsNativeSourceTerminalCrossFieldErrorV1::AliasedDigest);
        }
        *self
            .values
            .get_mut(self.len)
            .ok_or(RnsNativeSourceTerminalCrossFieldErrorV1::AliasedDigest)? = digest;
        self.len += 1;
        Ok(())
    }
}

struct ZeroizingCoefficientsV1(Vec<[u8; CANONICAL_COEFFICIENT_BYTES_V1]>);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TerminalCoordinateV1 {
    Value { row: usize, column: usize },
    Blinding { row: usize },
}

impl ZeroizingCoefficientsV1 {
    fn try_new() -> Result<Self, RnsNativeSourceTerminalCrossFieldErrorV1> {
        Ok(Self(
            try_exact_capacity_vec_v1(ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1)
                .map_err(|_| RnsNativeSourceTerminalCrossFieldErrorV1::Allocation)?,
        ))
    }

    fn into_vec(mut self) -> Vec<[u8; CANONICAL_COEFFICIENT_BYTES_V1]> {
        core::mem::take(&mut self.0)
    }
}

impl Drop for ZeroizingCoefficientsV1 {
    fn drop(&mut self) {
        for coefficient in &mut self.0 {
            coefficient.fill(0);
        }
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
    }
}

/// Consume all preceding private prerequisites and verify the exact source to
/// terminal mapping.  The returned token remains non-authorizing because the
/// cross-field/global-lookup proof is only identity-bound, not verified.
pub(super) fn link_rns_native_source_terminal_cross_field_v1<'a, S>(
    transcript: &ZkAmsMkheRnsNativeChallengeSeedsV1,
    mut source: RnsNativeRlweSourceStatementStageV1<'a, S>,
    terminal: RnsNativeTerminalCrossBasisKernelPrerequisiteV1,
    zero_padding: RnsNativeZeroPaddingCommitmentPrerequisiteV1,
    cross: ZkAmsMkheRnsNativeCrossFieldGlobalLookupSectionV1<'_>,
) -> Result<
    RnsNativeSourceTerminalCrossFieldPrerequisiteV1<'a, S>,
    RnsNativeSourceTerminalCrossFieldErrorV1,
>
where
    S: ZkAmsMkheRnsNativeSourceSnapshotV1,
{
    validate_context_v1(transcript, &source, &terminal, &zero_padding)?;
    let formula_digest = mapping_formula_digest_v1()?;
    let opening_bundle_digest =
        validate_opening_bindings_v1(transcript, &terminal, formula_digest)?;
    let challenge = derive_mapping_challenge_v1(
        transcript.mapping_challenge_seed(),
        formula_digest,
        opening_bundle_digest,
        terminal.hyrax_digest(),
    )?;
    let weights = row_weights_v1(challenge);
    let public_aggregate = public_terminal_aggregate_v1(terminal.hyrax_commitments(), &weights)?;
    let aggregate_point_digest = aggregate_point_digest_v1(&public_aggregate)?;
    validate_mapping_roots_v1(
        transcript,
        formula_digest,
        opening_bundle_digest,
        terminal.hyrax_digest(),
        challenge,
        aggregate_point_digest,
    )?;

    let point_bundle =
        indexed_digest_bundle_v1(POINT_BUNDLE_DOMAIN_V1, cross.point_evaluation_digests())?;
    let limb_bundle =
        indexed_digest_bundle_v1(LIMB_BUNDLE_DOMAIN_V1, cross.limb_relation_digests())?;
    let round_bundle =
        indexed_digest_bundle_v1(ROUND_BUNDLE_DOMAIN_V1, cross.sumcheck_round_digests())?;
    let zero_limb_bundle = indexed_digest_bundle_v1(
        ZERO_LIMB_BUNDLE_DOMAIN_V1,
        zero_padding.limb_padding_digests(),
    )?;
    let cross_proof_digest = cross_proof_digest_v1(cross.proof())?;
    let cross_link_digest = cross_link_digest_v1(
        transcript,
        &source,
        &terminal,
        &zero_padding,
        formula_digest,
        opening_bundle_digest,
        aggregate_point_digest,
        point_bundle,
        limb_bundle,
        round_bundle,
        zero_limb_bundle,
        cross_proof_digest,
    )?;
    validate_global_aliases_v1(
        transcript,
        &source,
        &terminal,
        &zero_padding,
        cross.point_evaluation_digests(),
        cross.limb_relation_digests(),
        cross.sumcheck_round_digests(),
        [
            formula_digest,
            opening_bundle_digest,
            aggregate_point_digest,
            point_bundle,
            limb_bundle,
            round_bundle,
            zero_limb_bundle,
            cross_proof_digest,
            cross_link_digest,
        ],
    )?;

    let anchor = ResidualAnchorV1::from_canonical_bytes_exact_v1(source.downstream())?;
    let expected_core = expected_anchor_core_v1(
        transcript,
        &source,
        &terminal,
        formula_digest,
        opening_bundle_digest,
        point_bundle,
        limb_bundle,
        round_bundle,
        cross_proof_digest,
        cross_link_digest,
        anchor.downstream,
    );
    validate_anchor_core_v1(anchor, expected_core)?;
    let anchor_digest = anchor_digest_v1(source.downstream())?;

    let secret_aggregate = replay_source_terminal_aggregate_v1(source.snapshot_mut(), &weights)?;
    verify_aggregate_commitment_v1(&secret_aggregate, &public_aggregate)?;

    Ok(RnsNativeSourceTerminalCrossFieldPrerequisiteV1 {
        source,
        terminal,
        zero_padding,
        downstream: anchor.downstream,
        formula_digest,
        opening_bundle_digest,
        aggregate_point_digest,
        point_bundle_digest: point_bundle,
        limb_bundle_digest: limb_bundle,
        round_bundle_digest: round_bundle,
        zero_limb_bundle_digest: zero_limb_bundle,
        cross_proof_digest,
        cross_link_digest,
        anchor_digest,
    })
}

fn validate_anchor_core_v1(
    anchor: ResidualAnchorV1<'_>,
    expected: [[u8; DIGEST_BYTES_V1]; ANCHOR_CORE_DIGESTS_V1],
) -> Result<(), RnsNativeSourceTerminalCrossFieldErrorV1> {
    if anchor.core == expected {
        Ok(())
    } else {
        Err(RnsNativeSourceTerminalCrossFieldErrorV1::InvalidAnchor)
    }
}

fn validate_context_v1<S: ZkAmsMkheRnsNativeSourceSnapshotV1>(
    transcript: &ZkAmsMkheRnsNativeChallengeSeedsV1,
    source: &RnsNativeRlweSourceStatementStageV1<'_, S>,
    terminal: &RnsNativeTerminalCrossBasisKernelPrerequisiteV1,
    zero_padding: &RnsNativeZeroPaddingCommitmentPrerequisiteV1,
) -> Result<(), RnsNativeSourceTerminalCrossFieldErrorV1> {
    let layout = source.snapshot().layout();
    if layout.profile_digest() != transcript.profile_digest()
        || layout.topology_digest() != transcript.topology_digest()
        || layout.release_candidate_digest() != transcript.release_candidate_digest()
        || layout.statement_digest() != transcript.statement_digest()
        || layout.operational_context_digest() != transcript.operational_context_digest()
        || layout.source_binding_digest() != transcript.source_binding_digest()
        || source.public_bundle_digest() != transcript.public_ciphertext_digest()
        || source.qpcs().transcript_digest() != transcript.transcript_digest()
        || source.qpcs().query_seed() != transcript.qpcs_query_challenge_seed()
        || terminal.hyrax_commitments().len() != TERMINAL_ROWS_V1
        || terminal.bp_commitments().len() != TERMINAL_ROWS_V1
    {
        return Err(RnsNativeSourceTerminalCrossFieldErrorV1::InvalidContext);
    }
    terminal
        .validate_context_v1(transcript)
        .map_err(|_| RnsNativeSourceTerminalCrossFieldErrorV1::InvalidContext)?;
    zero_padding
        .validate_context_v1(transcript)
        .map_err(|_| RnsNativeSourceTerminalCrossFieldErrorV1::InvalidContext)?;
    Ok(())
}

fn mapping_formula_digest_v1()
-> Result<[u8; DIGEST_BYTES_V1], RnsNativeSourceTerminalCrossFieldErrorV1> {
    let x = zk_ams_t256_packing_layout_v1(X_USED_SLOTS_V1)
        .map_err(|_| RnsNativeSourceTerminalCrossFieldErrorV1::InvalidPacking)?;
    let full = zk_ams_t256_packing_layout_v1(ZK_AMS_MKHE_RELEASE_SLOT_COUNT_V1 as u32)
        .map_err(|_| RnsNativeSourceTerminalCrossFieldErrorV1::InvalidPacking)?;
    let re = zk_ams_t256_packing_layout_v1(RE_USED_SLOTS_V1)
        .map_err(|_| RnsNativeSourceTerminalCrossFieldErrorV1::InvalidPacking)?;
    let rw = zk_ams_t256_packing_layout_v1(RW_USED_SLOTS_V1)
        .map_err(|_| RnsNativeSourceTerminalCrossFieldErrorV1::InvalidPacking)?;
    let mut hash = Keccak256::new();
    hash.update(FORMULA_DOMAIN_V1);
    hash.update(&[LINK_VERSION_V1]);
    hash.update(&(ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1 as u32).to_be_bytes());
    hash.update(&(TERMINAL_ROWS_V1 as u16).to_be_bytes());
    hash.update(&(TERMINAL_COLUMNS_V1 as u16).to_be_bytes());
    hash.update(&(X_RECORD_V1 as u8).to_be_bytes());
    hash.update(&(E_FIRST_RECORD_V1 as u8).to_be_bytes());
    hash.update(&(E_RECORDS_V1 as u8).to_be_bytes());
    hash.update(&(RE_RECORD_V1 as u8).to_be_bytes());
    hash.update(&(W_FIRST_RECORD_V1 as u8).to_be_bytes());
    hash.update(&(W_RECORDS_V1 as u8).to_be_bytes());
    hash.update(&(RW_RECORD_V1 as u8).to_be_bytes());
    hash.update(&x.digest);
    hash.update(&full.digest);
    hash.update(&re.digest);
    hash.update(&rw.digest);
    for formula in [
        SOURCE_MAPPING_FORMULA_V1,
        SOURCE_PACKING_FORMULA_V1,
        SOURCE_PADDING_FORMULA_V1,
        BATCH_FORMULA_V1,
        OPENING_SLICE_FORMULA_V1,
        super::super::COMMITMENT_KEY_LABEL_V1,
    ] {
        hash.update(&(formula.len() as u16).to_be_bytes());
        hash.update(formula);
    }
    Ok(hash.finalize())
}

fn validate_opening_bindings_v1(
    transcript: &ZkAmsMkheRnsNativeChallengeSeedsV1,
    terminal: &RnsNativeTerminalCrossBasisKernelPrerequisiteV1,
    formula_digest: [u8; DIGEST_BYTES_V1],
) -> Result<[u8; DIGEST_BYTES_V1], RnsNativeSourceTerminalCrossFieldErrorV1> {
    let mut bundle = Keccak256::new();
    bundle.update(OPENING_BUNDLE_DOMAIN_V1);
    bundle.update(&[LINK_VERSION_V1]);
    bundle.update(&formula_digest);
    bundle.update(&terminal.hyrax_digest());
    bundle.update(&[ZK_AMS_MKHE_RNS_NATIVE_OPENING_COUNT_V1]);
    for (ordinal, opening) in transcript.opening_commitments().iter().enumerate() {
        let expected = opening_hyrax_digest_v1(
            u8::try_from(ordinal)
                .map_err(|_| RnsNativeSourceTerminalCrossFieldErrorV1::InvalidGeometry)?,
            *opening,
            terminal.hyrax_digest(),
            terminal.hyrax_commitments(),
        )?;
        if expected != opening.hyrax_commitment_digest() {
            return Err(RnsNativeSourceTerminalCrossFieldErrorV1::InvalidMapping);
        }
        bundle.update(&[
            ordinal as u8,
            opening.family() as u8,
            opening.family_index(),
        ]);
        bundle.update(&opening.source_commitment_digest());
        bundle.update(&expected);
    }
    Ok(bundle.finalize())
}

fn opening_hyrax_digest_v1(
    ordinal: u8,
    opening: ZkAmsMkheRnsNativeOpeningCommitmentV1,
    point_set_digest: [u8; DIGEST_BYTES_V1],
    points: &[Point],
) -> Result<[u8; DIGEST_BYTES_V1], RnsNativeSourceTerminalCrossFieldErrorV1> {
    let (start, count, role) = match opening.family() {
        ZkAmsMkheRnsNativeFamilyV1::X | ZkAmsMkheRnsNativeFamilyV1::U => (0, 0, 0_u8),
        ZkAmsMkheRnsNativeFamilyV1::E => (
            usize::from(opening.family_index()) * ROWS_PER_FULL_RECORD_V1,
            ROWS_PER_FULL_RECORD_V1,
            1,
        ),
        ZkAmsMkheRnsNativeFamilyV1::RE => (0, ERROR_ROWS_V1, 2),
        ZkAmsMkheRnsNativeFamilyV1::W => (
            ERROR_ROWS_V1 + usize::from(opening.family_index()) * ROWS_PER_FULL_RECORD_V1,
            ROWS_PER_FULL_RECORD_V1,
            3,
        ),
        ZkAmsMkheRnsNativeFamilyV1::RW => (ERROR_ROWS_V1, WITNESS_ROWS_V1, 4),
    };
    let end = start
        .checked_add(count)
        .ok_or(RnsNativeSourceTerminalCrossFieldErrorV1::InvalidGeometry)?;
    let slice = points
        .get(start..end)
        .ok_or(RnsNativeSourceTerminalCrossFieldErrorV1::InvalidGeometry)?;
    let mut hash = Keccak256::new();
    hash.update(OPENING_DIGEST_DOMAIN_V1);
    hash.update(&[LINK_VERSION_V1, ordinal, opening.family() as u8]);
    hash.update(&[opening.family_index(), role]);
    hash.update(&opening.source_commitment_digest());
    hash.update(&point_set_digest);
    hash.update(&(start as u16).to_be_bytes());
    hash.update(&(count as u16).to_be_bytes());
    for point in slice {
        let mut encoded = [0_u8; POINT_BYTES_V1];
        point
            .write_non_identity_wire_bytes_ref(&mut encoded)
            .map_err(|_| RnsNativeSourceTerminalCrossFieldErrorV1::InvalidPoint)?;
        hash.update(&encoded);
    }
    Ok(hash.finalize())
}

fn derive_mapping_challenge_v1(
    seed: [u8; DIGEST_BYTES_V1],
    formula_digest: [u8; DIGEST_BYTES_V1],
    opening_bundle_digest: [u8; DIGEST_BYTES_V1],
    point_set_digest: [u8; DIGEST_BYTES_V1],
) -> Result<Scalar, RnsNativeSourceTerminalCrossFieldErrorV1> {
    for attempt in 0..MAX_CHALLENGE_ATTEMPTS_V1 {
        let mut wide = [0_u8; 64];
        for (half, output) in wide.chunks_exact_mut(DIGEST_BYTES_V1).enumerate() {
            let mut hash = Keccak256::new();
            hash.update(CHALLENGE_DOMAIN_V1);
            hash.update(&[LINK_VERSION_V1, half as u8]);
            hash.update(&seed);
            hash.update(&formula_digest);
            hash.update(&opening_bundle_digest);
            hash.update(&point_set_digest);
            hash.update(&attempt.to_be_bytes());
            output.copy_from_slice(&hash.finalize());
        }
        let challenge = Scalar::from_uniform_le_bytes(wide);
        if !challenge.is_zero() && challenge != Scalar::one() {
            return Ok(challenge);
        }
    }
    Err(RnsNativeSourceTerminalCrossFieldErrorV1::InvalidScalar)
}

fn row_weights_v1(challenge: Scalar) -> [Scalar; TERMINAL_ROWS_V1] {
    let mut next = Scalar::one();
    core::array::from_fn(|_| {
        let current = next;
        next *= challenge;
        current
    })
}

fn public_terminal_aggregate_v1(
    points: &[Point],
    weights: &[Scalar; TERMINAL_ROWS_V1],
) -> Result<Point, RnsNativeSourceTerminalCrossFieldErrorV1> {
    if points.len() != TERMINAL_ROWS_V1 || points.iter().any(|point| point.is_identity()) {
        return Err(RnsNativeSourceTerminalCrossFieldErrorV1::InvalidPoint);
    }
    let mut terms = try_exact_capacity_vec_v1(TERMINAL_ROWS_V1)
        .map_err(|_| RnsNativeSourceTerminalCrossFieldErrorV1::Allocation)?;
    for (weight, point) in weights.iter().zip(points) {
        terms.push((*weight, *point));
    }
    let aggregate = multiexp::<ZkAmsT256BulletproofSuiteV1>(&terms);
    if aggregate.is_identity() {
        return Err(RnsNativeSourceTerminalCrossFieldErrorV1::InvalidMapping);
    }
    Ok(aggregate)
}

fn aggregate_point_digest_v1(
    point: &Point,
) -> Result<[u8; DIGEST_BYTES_V1], RnsNativeSourceTerminalCrossFieldErrorV1> {
    let mut encoded = [0_u8; POINT_BYTES_V1];
    point
        .write_non_identity_wire_bytes_ref(&mut encoded)
        .map_err(|_| RnsNativeSourceTerminalCrossFieldErrorV1::InvalidPoint)?;
    let mut hash = Keccak256::new();
    hash.update(AGGREGATE_POINT_DOMAIN_V1);
    hash.update(&[LINK_VERSION_V1]);
    hash.update(&encoded);
    Ok(hash.finalize())
}

fn validate_mapping_roots_v1(
    transcript: &ZkAmsMkheRnsNativeChallengeSeedsV1,
    formula_digest: [u8; DIGEST_BYTES_V1],
    opening_bundle_digest: [u8; DIGEST_BYTES_V1],
    hyrax_digest: [u8; DIGEST_BYTES_V1],
    challenge: Scalar,
    aggregate_point_digest: [u8; DIGEST_BYTES_V1],
) -> Result<(), RnsNativeSourceTerminalCrossFieldErrorV1> {
    let mut mapping = Keccak256::new();
    mapping.update(MAPPING_ROOT_DOMAIN_V1);
    mapping.update(&[LINK_VERSION_V1]);
    for digest in [
        transcript.profile_digest(),
        transcript.topology_digest(),
        transcript.release_candidate_digest(),
        transcript.statement_digest(),
        transcript.operational_context_digest(),
        transcript.source_binding_digest(),
        transcript.main_snapshot_digest(),
        transcript.nonce_snapshot_digest(),
        transcript.source_receipt_digest(),
        transcript.mapping_challenge_seed(),
        formula_digest,
        opening_bundle_digest,
        hyrax_digest,
        aggregate_point_digest,
    ] {
        mapping.update(&digest);
    }
    mapping.update(&challenge.to_be_bytes());
    let mapping_root = mapping.finalize();
    if mapping_root != transcript.mapping_root() {
        return Err(RnsNativeSourceTerminalCrossFieldErrorV1::InvalidMapping);
    }
    let mut terminal = Keccak256::new();
    terminal.update(TERMINAL_ROOT_DOMAIN_V1);
    terminal.update(&[LINK_VERSION_V1]);
    for digest in [
        mapping_root,
        opening_bundle_digest,
        hyrax_digest,
        aggregate_point_digest,
    ] {
        terminal.update(&digest);
    }
    if terminal.finalize() != transcript.terminal_hyrax_root() {
        return Err(RnsNativeSourceTerminalCrossFieldErrorV1::InvalidMapping);
    }
    Ok(())
}

fn replay_source_terminal_aggregate_v1<S: ZkAmsMkheRnsNativeSourceSnapshotV1>(
    snapshot: &mut S,
    weights: &[Scalar; TERMINAL_ROWS_V1],
) -> Result<ZeroizingT256ScalarVecV1, RnsNativeSourceTerminalCrossFieldErrorV1> {
    let mut aggregate = ZeroizingT256ScalarVecV1::try_with_exact_capacity(COMMITMENT_BASIS_V1)
        .map_err(|_| RnsNativeSourceTerminalCrossFieldErrorV1::Allocation)?;
    for _ in 0..COMMITMENT_BASIS_V1 {
        aggregate.push(Scalar::zero());
    }
    if aggregate.len() != COMMITMENT_BASIS_V1 {
        return Err(RnsNativeSourceTerminalCrossFieldErrorV1::Allocation);
    }
    let mut workspace = T256PackedPlaintextDecodeWorkspaceV1::try_new_v1()
        .map_err(|_| RnsNativeSourceTerminalCrossFieldErrorV1::Allocation)?;
    let x_layout = zk_ams_t256_packing_layout_v1(X_USED_SLOTS_V1)
        .map_err(|_| RnsNativeSourceTerminalCrossFieldErrorV1::InvalidPacking)?;
    replay_record_v1(
        snapshot,
        X_RECORD_V1,
        x_layout,
        &mut workspace,
        |_slot, _value| Ok(()),
    )?;
    let full_layout = zk_ams_t256_packing_layout_v1(ZK_AMS_MKHE_RELEASE_SLOT_COUNT_V1 as u32)
        .map_err(|_| RnsNativeSourceTerminalCrossFieldErrorV1::InvalidPacking)?;
    for family_index in 0..E_RECORDS_V1 {
        let record = E_FIRST_RECORD_V1 + family_index;
        replay_record_v1(
            snapshot,
            record,
            full_layout,
            &mut workspace,
            |slot, value| {
                accumulate_mapped_source_slot_v1(&mut aggregate, record, slot, value, weights)
            },
        )?;
    }
    let re_layout = zk_ams_t256_packing_layout_v1(RE_USED_SLOTS_V1)
        .map_err(|_| RnsNativeSourceTerminalCrossFieldErrorV1::InvalidPacking)?;
    replay_record_v1(
        snapshot,
        RE_RECORD_V1,
        re_layout,
        &mut workspace,
        |slot, value| {
            accumulate_mapped_source_slot_v1(&mut aggregate, RE_RECORD_V1, slot, value, weights)
        },
    )?;
    for family_index in 0..W_RECORDS_V1 {
        let record = W_FIRST_RECORD_V1 + family_index;
        replay_record_v1(
            snapshot,
            record,
            full_layout,
            &mut workspace,
            |slot, value| {
                accumulate_mapped_source_slot_v1(&mut aggregate, record, slot, value, weights)
            },
        )?;
    }
    let rw_layout = zk_ams_t256_packing_layout_v1(RW_USED_SLOTS_V1)
        .map_err(|_| RnsNativeSourceTerminalCrossFieldErrorV1::InvalidPacking)?;
    replay_record_v1(
        snapshot,
        RW_RECORD_V1,
        rw_layout,
        &mut workspace,
        |slot, value| {
            accumulate_mapped_source_slot_v1(&mut aggregate, RW_RECORD_V1, slot, value, weights)
        },
    )?;
    Ok(aggregate)
}

fn terminal_coordinate_v1(
    record: usize,
    slot: usize,
) -> Result<TerminalCoordinateV1, RnsNativeSourceTerminalCrossFieldErrorV1> {
    if (E_FIRST_RECORD_V1..E_FIRST_RECORD_V1 + E_RECORDS_V1).contains(&record)
        && slot < ZK_AMS_MKHE_RELEASE_SLOT_COUNT_V1
    {
        return Ok(TerminalCoordinateV1::Value {
            row: (record - E_FIRST_RECORD_V1) * ROWS_PER_FULL_RECORD_V1
                + slot / TERMINAL_COLUMNS_V1,
            column: slot % TERMINAL_COLUMNS_V1,
        });
    }
    if record == RE_RECORD_V1 && slot < RE_USED_SLOTS_V1 as usize {
        return Ok(TerminalCoordinateV1::Blinding { row: slot });
    }
    if (W_FIRST_RECORD_V1..W_FIRST_RECORD_V1 + W_RECORDS_V1).contains(&record)
        && slot < ZK_AMS_MKHE_RELEASE_SLOT_COUNT_V1
    {
        return Ok(TerminalCoordinateV1::Value {
            row: ERROR_ROWS_V1
                + (record - W_FIRST_RECORD_V1) * ROWS_PER_FULL_RECORD_V1
                + slot / TERMINAL_COLUMNS_V1,
            column: slot % TERMINAL_COLUMNS_V1,
        });
    }
    if record == RW_RECORD_V1 && slot < RW_USED_SLOTS_V1 as usize {
        return Ok(TerminalCoordinateV1::Blinding {
            row: ERROR_ROWS_V1 + slot,
        });
    }
    Err(RnsNativeSourceTerminalCrossFieldErrorV1::InvalidGeometry)
}

fn accumulate_mapped_source_slot_v1(
    aggregate: &mut ZeroizingT256ScalarVecV1,
    record: usize,
    slot: usize,
    value: Scalar,
    weights: &[Scalar; TERMINAL_ROWS_V1],
) -> Result<(), RnsNativeSourceTerminalCrossFieldErrorV1> {
    let (index, row) = match terminal_coordinate_v1(record, slot)? {
        TerminalCoordinateV1::Value { row, column } => (column, row),
        TerminalCoordinateV1::Blinding { row } => (TERMINAL_COLUMNS_V1, row),
    };
    let weight = *weights
        .get(row)
        .ok_or(RnsNativeSourceTerminalCrossFieldErrorV1::InvalidGeometry)?;
    accumulate_secret_product_v1(aggregate, index, value, weight)
}

fn replay_record_v1<S: ZkAmsMkheRnsNativeSourceSnapshotV1>(
    snapshot: &mut S,
    record: usize,
    layout: super::packing::ZkAmsT256PackingLayoutV1,
    workspace: &mut T256PackedPlaintextDecodeWorkspaceV1,
    mut visit: impl FnMut(usize, Scalar) -> Result<(), RnsNativeSourceTerminalCrossFieldErrorV1>,
) -> Result<(), RnsNativeSourceTerminalCrossFieldErrorV1> {
    let mut coefficients = ZeroizingCoefficientsV1::try_new()?;
    let record_base = record
        .checked_mul(ZK_AMS_MKHE_RNS_NATIVE_SOURCE_MAIN_BLOCKS_PER_OPENING_V1 as usize)
        .ok_or(RnsNativeSourceTerminalCrossFieldErrorV1::InvalidGeometry)?;
    for block in 0..CANONICAL_BLOCKS_PER_RECORD_V1 {
        let slot = u64::try_from(record_base + block)
            .map_err(|_| RnsNativeSourceTerminalCrossFieldErrorV1::InvalidGeometry)?;
        let chunk = snapshot
            .read_slot(ZkAmsMkheRnsNativeSourceArenaV1::Main, slot)
            .map_err(|_| RnsNativeSourceTerminalCrossFieldErrorV1::InvalidSource)?;
        if chunk.arena() != ZkAmsMkheRnsNativeSourceArenaV1::Main
            || chunk.as_slice().len()
                != ZK_AMS_MKHE_RNS_NATIVE_SOURCE_MAIN_PLAINTEXT_BYTES_V1 as usize
        {
            return Err(RnsNativeSourceTerminalCrossFieldErrorV1::InvalidSource);
        }
        for coefficient in chunk
            .as_slice()
            .chunks_exact(CANONICAL_COEFFICIENT_BYTES_V1)
        {
            coefficients.0.push(
                coefficient
                    .try_into()
                    .map_err(|_| RnsNativeSourceTerminalCrossFieldErrorV1::InvalidSource)?,
            );
        }
    }
    if coefficients.0.len() != ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1 {
        return Err(RnsNativeSourceTerminalCrossFieldErrorV1::InvalidSource);
    }
    let mut visited = 0_usize;
    visit_rehydrated_t256_coefficients_used_slots_with_workspace_v1(
        layout,
        0,
        coefficients.into_vec(),
        workspace,
        |bytes| {
            let value = Scalar::from_be_bytes_exact(*bytes)
                .map_err(|_| super::ZkAmsMkheErrorV1::InvalidPolynomial)?;
            visit(visited, value).map_err(|_| super::ZkAmsMkheErrorV1::InvalidPolynomial)?;
            visited = visited
                .checked_add(1)
                .ok_or(super::ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
            Ok(())
        },
    )
    .map_err(|_| RnsNativeSourceTerminalCrossFieldErrorV1::InvalidPacking)?;
    if visited != layout.logical_value_count as usize {
        return Err(RnsNativeSourceTerminalCrossFieldErrorV1::InvalidPacking);
    }
    Ok(())
}

fn accumulate_secret_product_v1(
    aggregate: &mut ZeroizingT256ScalarVecV1,
    index: usize,
    value: Scalar,
    weight: Scalar,
) -> Result<(), RnsNativeSourceTerminalCrossFieldErrorV1> {
    let value = ZeroizingT256ScalarCopyV1::new(value);
    let destination = aggregate
        .as_mut_slice()
        .get_mut(index)
        .ok_or(RnsNativeSourceTerminalCrossFieldErrorV1::InvalidGeometry)?;
    *destination += value.get() * weight;
    Ok(())
}

fn verify_aggregate_commitment_v1(
    aggregate: &ZeroizingT256ScalarVecV1,
    public_aggregate: &Point,
) -> Result<(), RnsNativeSourceTerminalCrossFieldErrorV1> {
    if aggregate.len() != COMMITMENT_BASIS_V1 || public_aggregate.is_identity() {
        return Err(RnsNativeSourceTerminalCrossFieldErrorV1::InvalidGeometry);
    }
    let key = CommitmentKey::derive(super::super::COMMITMENT_KEY_LABEL_V1, TERMINAL_COLUMNS_V1)
        .map_err(|_| RnsNativeSourceTerminalCrossFieldErrorV1::InvalidMapping)?;
    if key.columns() != TERMINAL_COLUMNS_V1 {
        return Err(RnsNativeSourceTerminalCrossFieldErrorV1::InvalidGeometry);
    }
    verify_aggregate_commitment_for_key_v1(aggregate.as_slice(), public_aggregate, &key)
}

fn verify_aggregate_commitment_for_key_v1(
    aggregate: &[Scalar],
    public_aggregate: &Point,
    key: &CommitmentKey,
) -> Result<(), RnsNativeSourceTerminalCrossFieldErrorV1> {
    let basis = key
        .columns()
        .checked_add(1)
        .ok_or(RnsNativeSourceTerminalCrossFieldErrorV1::InvalidGeometry)?;
    if aggregate.len() != basis || public_aggregate.is_identity() {
        return Err(RnsNativeSourceTerminalCrossFieldErrorV1::InvalidGeometry);
    }
    let mut builder = SecretMultiexpBuilder::<ZkAmsT256BulletproofSuiteV1>::new(basis)
        .map_err(|_| RnsNativeSourceTerminalCrossFieldErrorV1::Allocation)?;
    for (value, generator) in aggregate.iter().take(key.columns()).zip(key.generators()) {
        builder
            .push(value, generator)
            .map_err(|_| RnsNativeSourceTerminalCrossFieldErrorV1::InvalidMapping)?;
    }
    builder
        .push(
            aggregate
                .get(key.columns())
                .ok_or(RnsNativeSourceTerminalCrossFieldErrorV1::InvalidGeometry)?,
            &key.hiding_generator(),
        )
        .map_err(|_| RnsNativeSourceTerminalCrossFieldErrorV1::InvalidMapping)?;
    let expected = builder
        .evaluate()
        .map_err(|_| RnsNativeSourceTerminalCrossFieldErrorV1::InvalidMapping)?;
    if expected.is_identity() || !expected.equals(public_aggregate) {
        return Err(RnsNativeSourceTerminalCrossFieldErrorV1::InvalidMapping);
    }
    Ok(())
}

fn indexed_digest_bundle_v1(
    domain: &[u8],
    digests: &[[u8; DIGEST_BYTES_V1]],
) -> Result<[u8; DIGEST_BYTES_V1], RnsNativeSourceTerminalCrossFieldErrorV1> {
    if digests.is_empty() || digests.len() > u8::MAX as usize {
        return Err(RnsNativeSourceTerminalCrossFieldErrorV1::InvalidCrossFieldBinding);
    }
    let mut registry = DigestRegistryV1::new();
    let mut hash = Keccak256::new();
    hash.update(domain);
    hash.update(&[LINK_VERSION_V1, digests.len() as u8]);
    for (index, digest) in digests.iter().enumerate() {
        registry.insert(*digest)?;
        hash.update(&[index as u8]);
        hash.update(digest);
    }
    Ok(hash.finalize())
}

fn cross_proof_digest_v1(
    proof: &[u8],
) -> Result<[u8; DIGEST_BYTES_V1], RnsNativeSourceTerminalCrossFieldErrorV1> {
    if proof.is_empty() || proof.len() > u32::MAX as usize {
        return Err(RnsNativeSourceTerminalCrossFieldErrorV1::InvalidCrossFieldBinding);
    }
    let mut hash = Keccak256::new();
    hash.update(CROSS_PROOF_DOMAIN_V1);
    hash.update(&[LINK_VERSION_V1]);
    hash.update(&(proof.len() as u32).to_be_bytes());
    hash.update(proof);
    Ok(hash.finalize())
}

#[allow(clippy::too_many_arguments)]
fn cross_link_digest_v1<S: ZkAmsMkheRnsNativeSourceSnapshotV1>(
    transcript: &ZkAmsMkheRnsNativeChallengeSeedsV1,
    source: &RnsNativeRlweSourceStatementStageV1<'_, S>,
    terminal: &RnsNativeTerminalCrossBasisKernelPrerequisiteV1,
    zero_padding: &RnsNativeZeroPaddingCommitmentPrerequisiteV1,
    formula: [u8; DIGEST_BYTES_V1],
    opening_bundle: [u8; DIGEST_BYTES_V1],
    aggregate_point: [u8; DIGEST_BYTES_V1],
    point_bundle: [u8; DIGEST_BYTES_V1],
    limb_bundle: [u8; DIGEST_BYTES_V1],
    round_bundle: [u8; DIGEST_BYTES_V1],
    zero_limb_bundle: [u8; DIGEST_BYTES_V1],
    cross_proof: [u8; DIGEST_BYTES_V1],
) -> Result<[u8; DIGEST_BYTES_V1], RnsNativeSourceTerminalCrossFieldErrorV1> {
    let mut hash = Keccak256::new();
    hash.update(CROSS_LINK_DOMAIN_V1);
    hash.update(&[LINK_VERSION_V1]);
    hash.update(&source.epoch().to_be_bytes());
    for digest in [
        transcript.transcript_digest(),
        transcript.mapping_root(),
        transcript.terminal_hyrax_root(),
        transcript.cross_basis_bridge_root(),
        transcript.cross_field_challenge_seed(),
        transcript.global_lookup_challenge_seed(),
        transcript.cross_field_root(),
        transcript.global_lookup_root(),
        source.statement_anchor_digest(),
        source.mapping_digest(),
        source.formula_digest(),
        source.aggregation_schedule_digest(),
        source.preflight_statement_digest(),
        source.public_key_digest(),
        source.public_bundle_digest(),
        source.qpcs().parameter_digest(),
        source.qpcs().query_seed(),
        source.qpcs().section_binding_digest(),
        source.qpcs().schedule_digest(),
        source.qpcs().evaluation_binding_digest(),
        source.qpcs().residual_digest(),
        terminal.binding_digest(),
        terminal.hyrax_digest(),
        terminal.bp_digest(),
        terminal.bridge_root(),
        zero_padding.binding_digest(),
        zero_padding.point_set_digest(),
        zero_padding.root(),
        zero_padding.proof_digest(),
        zero_limb_bundle,
        formula,
        opening_bundle,
        aggregate_point,
        point_bundle,
        limb_bundle,
        round_bundle,
        cross_proof,
    ] {
        hash.update(&digest);
    }
    let digest = hash.finalize();
    if digest == [0; DIGEST_BYTES_V1] {
        return Err(RnsNativeSourceTerminalCrossFieldErrorV1::InvalidCrossFieldBinding);
    }
    Ok(digest)
}

#[allow(clippy::too_many_arguments)]
fn expected_anchor_core_v1<S: ZkAmsMkheRnsNativeSourceSnapshotV1>(
    transcript: &ZkAmsMkheRnsNativeChallengeSeedsV1,
    source: &RnsNativeRlweSourceStatementStageV1<'_, S>,
    terminal: &RnsNativeTerminalCrossBasisKernelPrerequisiteV1,
    formula: [u8; DIGEST_BYTES_V1],
    opening_bundle: [u8; DIGEST_BYTES_V1],
    point_bundle: [u8; DIGEST_BYTES_V1],
    limb_bundle: [u8; DIGEST_BYTES_V1],
    round_bundle: [u8; DIGEST_BYTES_V1],
    cross_proof: [u8; DIGEST_BYTES_V1],
    cross_link: [u8; DIGEST_BYTES_V1],
    downstream: &[u8],
) -> [[u8; DIGEST_BYTES_V1]; ANCHOR_CORE_DIGESTS_V1] {
    let mut core = [[0_u8; DIGEST_BYTES_V1]; ANCHOR_CORE_DIGESTS_V1];
    core[CORE_PRIOR_STATEMENT_V1] = source.statement_anchor_digest();
    core[CORE_PRIOR_RECORD_MAPPING_V1] = source.mapping_digest();
    core[CORE_TERMINAL_MAPPING_FORMULA_V1] = formula;
    core[CORE_HYRAX_POINTS_V1] = terminal.hyrax_digest();
    core[CORE_BP_POINTS_V1] = terminal.bp_digest();
    core[CORE_OPENING_HYRAX_BUNDLE_V1] = opening_bundle;
    core[CORE_MAPPING_ROOT_V1] = transcript.mapping_root();
    core[CORE_TERMINAL_HYRAX_ROOT_V1] = transcript.terminal_hyrax_root();
    core[CORE_CROSS_POINT_BUNDLE_V1] = point_bundle;
    core[CORE_CROSS_LIMB_BUNDLE_V1] = limb_bundle;
    core[CORE_LOOKUP_ROUND_BUNDLE_V1] = round_bundle;
    core[CORE_CROSS_PROOF_V1] = cross_proof;
    core[CORE_CROSS_ROOT_V1] = transcript.cross_field_root();
    core[CORE_GLOBAL_ROOT_V1] = transcript.global_lookup_root();
    core[CORE_CROSS_LINK_V1] = cross_link;
    core[CORE_DOWNSTREAM_V1] = downstream_digest_v1(downstream);
    core
}

#[allow(
    clippy::too_many_arguments,
    reason = "alias validation keeps every authenticated digest family explicit"
)]
fn validate_global_aliases_v1<S: ZkAmsMkheRnsNativeSourceSnapshotV1>(
    transcript: &ZkAmsMkheRnsNativeChallengeSeedsV1,
    source: &RnsNativeRlweSourceStatementStageV1<'_, S>,
    terminal: &RnsNativeTerminalCrossBasisKernelPrerequisiteV1,
    zero_padding: &RnsNativeZeroPaddingCommitmentPrerequisiteV1,
    point_digests: &[[u8; DIGEST_BYTES_V1]],
    limb_digests: &[[u8; DIGEST_BYTES_V1]],
    round_digests: &[[u8; DIGEST_BYTES_V1]],
    derived: [[u8; DIGEST_BYTES_V1]; 9],
) -> Result<(), RnsNativeSourceTerminalCrossFieldErrorV1> {
    let mut registry = DigestRegistryV1::new();
    for digest in [
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
        transcript.mapping_root(),
        transcript.terminal_hyrax_root(),
        transcript.cross_basis_bridge_root(),
        transcript.cross_field_root(),
        transcript.global_lookup_root(),
        transcript.zero_padding_root(),
        source.statement_anchor_digest(),
        source.mapping_digest(),
        source.formula_digest(),
        source.aggregation_schedule_digest(),
        source.preflight_statement_digest(),
        source.public_key_digest(),
        source.qpcs().parameter_digest(),
        source.qpcs().section_binding_digest(),
        source.qpcs().schedule_digest(),
        source.qpcs().evaluation_binding_digest(),
        source.qpcs().residual_digest(),
        terminal.binding_digest(),
        terminal.hyrax_digest(),
        terminal.bp_digest(),
        zero_padding.binding_digest(),
        zero_padding.point_set_digest(),
        zero_padding.proof_digest(),
    ] {
        registry.insert(digest)?;
    }
    for seed in transcript.ordered_challenge_seeds() {
        registry.insert(seed)?;
    }
    for root in [
        transcript.qpcs_initial_root(),
        transcript.qpcs_quotient_root(),
    ] {
        registry.insert(root)?;
    }
    for root in transcript.qpcs_fri_roots() {
        registry.insert(root.root())?;
    }
    for opening in transcript.opening_commitments() {
        registry.insert(opening.source_commitment_digest())?;
        registry.insert(opening.hyrax_commitment_digest())?;
    }
    for digest in zero_padding.limb_padding_digests() {
        registry.insert(*digest)?;
    }
    for digest in point_digests
        .iter()
        .chain(limb_digests)
        .chain(round_digests)
        .copied()
        .chain(derived)
    {
        registry.insert(digest)?;
    }
    Ok(())
}

fn downstream_digest_v1(bytes: &[u8]) -> [u8; DIGEST_BYTES_V1] {
    let mut hash = Keccak256::new();
    hash.update(DOWNSTREAM_DOMAIN_V1);
    hash.update(&[LINK_VERSION_V1]);
    hash.update(&(bytes.len() as u32).to_be_bytes());
    hash.update(bytes);
    hash.finalize()
}

fn anchor_digest_v1(
    bytes: &[u8],
) -> Result<[u8; DIGEST_BYTES_V1], RnsNativeSourceTerminalCrossFieldErrorV1> {
    if bytes.is_empty() || bytes.len() > RNS_NATIVE_RLWE_SOURCE_DOWNSTREAM_MAX_BYTES_V1 {
        return Err(RnsNativeSourceTerminalCrossFieldErrorV1::InvalidAnchor);
    }
    let mut hash = Keccak256::new();
    hash.update(ANCHOR_DOMAIN_V1);
    hash.update(&[LINK_VERSION_V1]);
    hash.update(&(bytes.len() as u32).to_be_bytes());
    hash.update(bytes);
    Ok(hash.finalize())
}

#[cfg(test)]
#[path = "rns_native_source_terminal_cross_field_tests.rs"]
mod tests;
