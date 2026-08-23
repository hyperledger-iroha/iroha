//! Exact private transport for the existing radix commitment owners.
//!
//! The first cross-field inventory authenticates the comparator, signed-small,
//! and q-mask planes, but it deliberately has no owner for the original radix
//! digits.  This stage consumes the move-only statement-10/11 prerequisite and
//! exact-decodes the missing 344 `(D, S)` groups.  Every group contains the 17
//! low radix commitments of `D` followed by the 17 low radix commitments of
//! `S`, for exactly 11,696 canonical non-identity T256 points.
//!
//! The two top commitments `bD` and `bS` are not repeated on this wire.  They
//! are returned only by aliasing the already-authenticated 688-point prefix of
//! the original cross-field inventory.  This prevents a second, potentially
//! inconsistent owner for either top vector.
//!
//! The role/group/column root is intentionally computed from only the new
//! candidate commitments and fixed public geometry.  It is suitable for the
//! future sole global-lookup `z` preimage when combined with separately audited
//! pre-z axes.  The full predecessor axes, which transitively bind post-z
//! inverse commitments, authenticate this transport but are excluded from that
//! candidate root.  Predecessor residual and binding digests occur only in the
//! post-decode private token binding, avoiding both Fiat--Shamir fixed points
//! and accidental pre-z absorption of post-z commitments.
//!
//! This stage proves no radix equation, range, inverse, product, lookup,
//! readiness, release, or authorization claim.

use super::{
    rns_native_cross_field_rlwe_direct::{
        RnsNativeCrossFieldRlweAtomicVerifiedV2, RnsNativeCrossFieldRlweDirectErrorV1,
        RnsNativeCrossFieldRlweSafeCoreProjectionV1,
    },
    rns_native_q_mask_linear_relations::{
        RNS_NATIVE_Q_MASK_LINEAR_RESIDUAL_MAX_BYTES_V1, RnsNativeQMaskLinearRelationsPrerequisiteV1,
    },
    rns_native_source::ZkAmsMkheRnsNativeSourceSnapshotV1,
};
use crate::vega::{
    VEGA_T256_SCALAR_MODULUS_BE_V1, VegaT256PointV1 as Point,
    bulletproof_t256::ZK_AMS_T256_BP_GENERATOR_BASIS_DIGEST_V1, sponge::Keccak256,
};

const VERSION_V1: u8 = 1;
const FLAGS_V1: u8 = 0;
const MAGIC_V1: [u8; 4] = *b"ZER1";
const DIGEST_BYTES_V1: usize = 32;
const POINT_BYTES_V1: usize = 33;
const GROUPS_V1: usize = 344;
const ROLES_V1: usize = 2;
const LOW_DIGITS_V1: usize = 17;
const POINTS_PER_GROUP_V1: usize = ROLES_V1 * LOW_DIGITS_V1;
const INVENTORY_POINTS_V1: usize = GROUPS_V1 * POINTS_PER_GROUP_V1;
const INVENTORY_BYTES_V1: usize = INVENTORY_POINTS_V1 * POINT_BYTES_V1;
const DIRECT_ALIAS_COPIED_DIGEST_BYTES_V1: usize = 2 * DIGEST_BYTES_V1;
const UPSTREAM_DIGESTS_V1: usize = 10;

// Fixed geometry prefix (26 bytes), ten transport-only predecessor axes, the
// candidate-only pre-z root, residual digest, and residual length.
const HEADER_BYTES_V1: usize = 26 + UPSTREAM_DIGESTS_V1 * DIGEST_BYTES_V1 + 2 * DIGEST_BYTES_V1 + 4;
const CODEC_DIGEST_BYTES_V1: usize = DIGEST_BYTES_V1;
const MIN_WIRE_BYTES_V1: usize = HEADER_BYTES_V1 + INVENTORY_BYTES_V1 + 1 + CODEC_DIGEST_BYTES_V1;
pub(super) const RNS_NATIVE_EXISTING_RADIX_RESIDUAL_MAX_BYTES_V1: usize =
    RNS_NATIVE_Q_MASK_LINEAR_RESIDUAL_MAX_BYTES_V1
        - HEADER_BYTES_V1
        - INVENTORY_BYTES_V1
        - CODEC_DIGEST_BYTES_V1;

const ROLE_DIFFERENCE_LOW_V1: u8 = 1;
const ROLE_SLACK_LOW_V1: u8 = 2;
const PRE_Z_MANIFEST_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-existing-radix.pre-z-manifest";
const PRE_Z_CANDIDATE_ROOT_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-existing-radix.pre-z-candidate-root";
const RESIDUAL_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.rns-native-existing-radix.residual";
const CODEC_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.rns-native-existing-radix.codec";
const PREREQUISITE_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.rns-native-existing-radix.prerequisite";
const POINT_ORDER_LANGUAGE_V1: &[u8] =
    b"ordinal=((group*2+role-index)*17+column);group=0..343;role-index=(0:D-low/tag1,1:S-low/tag2);column=0..16;top-commitments-are-aliased-from-original-inventory-and-never-encoded-here";
const SOLE_Z_SEPARATION_LANGUAGE_V1: &[u8] =
    b"pre-z-candidate-root=fixed-manifest||role-group-column-points-only;exclude-full-added-inventory-root,S3/S5/S8/S10-11-roots,residuals,bindings,codec,and-all-inverse-roots;transport-header-binds-predecessors;post-verification-token-binds-predecessor-residual-and-binding";

const EXISTING_RADIX_LOW_COMMITMENT_VIEW_AUTHENTICATED_V1: bool = true;
const EXISTING_RADIX_INVERSES_POST_Z_VERIFIED_V1: bool = false;
const RADIX_RECONSTRUCTION_VERIFIED_V1: bool = false;
const CENTERING_SUBTRACTION_VERIFIED_V1: bool = false;
const GLOBAL_LOOKUP_PRE_Z_READY_V1: bool = false;
const GLOBAL_LOOKUP_RELATIONS_VERIFIED_V1: bool = false;
const CROSS_FIELD_GLOBAL_LOOKUP_VERIFIED_V1: bool = false;
const RELEASE_READY_V1: bool = false;

const _: () = {
    assert!(GROUPS_V1 == 43 * 8);
    assert!(ROLES_V1 == 2);
    assert!(LOW_DIGITS_V1 == 17);
    assert!(POINTS_PER_GROUP_V1 == 34);
    assert!(INVENTORY_POINTS_V1 == 11_696);
    assert!(INVENTORY_BYTES_V1 == 385_968);
    assert!(DIRECT_ALIAS_COPIED_DIGEST_BYTES_V1 == 64);
    assert!(UPSTREAM_DIGESTS_V1 == 10);
    assert!(HEADER_BYTES_V1 == 414);
    assert!(MIN_WIRE_BYTES_V1 == 386_415);
    assert!(MIN_WIRE_BYTES_V1 <= 386_513);
    assert!(MIN_WIRE_BYTES_V1 <= RNS_NATIVE_Q_MASK_LINEAR_RESIDUAL_MAX_BYTES_V1);
    assert!(RNS_NATIVE_EXISTING_RADIX_RESIDUAL_MAX_BYTES_V1 == 1_817_839);
    assert!(EXISTING_RADIX_LOW_COMMITMENT_VIEW_AUTHENTICATED_V1);
    assert!(!EXISTING_RADIX_INVERSES_POST_Z_VERIFIED_V1);
    assert!(!RADIX_RECONSTRUCTION_VERIFIED_V1);
    assert!(!CENTERING_SUBTRACTION_VERIFIED_V1);
    assert!(!GLOBAL_LOOKUP_PRE_Z_READY_V1);
    assert!(!GLOBAL_LOOKUP_RELATIONS_VERIFIED_V1);
    assert!(!CROSS_FIELD_GLOBAL_LOOKUP_VERIFIED_V1);
    assert!(!RELEASE_READY_V1);
};

/// Failure while authenticating the exact existing-radix commitment view.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RnsNativeExistingRadixCommitmentViewErrorV1 {
    ProofCapExceeded,
    InvalidHeader,
    InvalidGeometry,
    InvalidPoint,
    InvalidIntegrity,
    InvalidContext,
    ArithmeticOverflow,
}

impl core::fmt::Display for RnsNativeExistingRadixCommitmentViewErrorV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for RnsNativeExistingRadixCommitmentViewErrorV1 {}

#[derive(Clone, Copy)]
struct UpstreamBindingV1 {
    prior_context_digest: [u8; DIGEST_BYTES_V1],
    added_inventory_root: [u8; DIGEST_BYTES_V1],
    statement3_proof_set_root: [u8; DIGEST_BYTES_V1],
    statement3_verified_transcript_root: [u8; DIGEST_BYTES_V1],
    statement5_proof_set_root: [u8; DIGEST_BYTES_V1],
    statement5_verified_transcript_root: [u8; DIGEST_BYTES_V1],
    statement8_proof_set_root: [u8; DIGEST_BYTES_V1],
    statement8_verified_transcript_root: [u8; DIGEST_BYTES_V1],
    q_mask_proof_set_root: [u8; DIGEST_BYTES_V1],
    q_mask_verified_transcript_root: [u8; DIGEST_BYTES_V1],
}

impl UpstreamBindingV1 {
    fn from_prerequisite_v1<S: ZkAmsMkheRnsNativeSourceSnapshotV1>(
        previous: &RnsNativeQMaskLinearRelationsPrerequisiteV1<'_, '_, S>,
    ) -> Self {
        let statement8 = previous.previous();
        let statement5 = statement8.previous();
        let statement3 = statement5.previous();
        let inventory = statement3.inventory();
        Self {
            prior_context_digest: inventory.prior_context_digest(),
            added_inventory_root: inventory.inventory_root(),
            statement3_proof_set_root: statement3.proof_set_root(),
            statement3_verified_transcript_root: statement3.verified_transcript_root(),
            statement5_proof_set_root: statement5.proof_set_root(),
            statement5_verified_transcript_root: statement5.verified_transcript_root(),
            statement8_proof_set_root: statement8.proof_set_root(),
            statement8_verified_transcript_root: statement8.verified_transcript_root(),
            q_mask_proof_set_root: previous.proof_set_root(),
            q_mask_verified_transcript_root: previous.verified_transcript_root(),
        }
    }

    fn digests_v1(self) -> [[u8; DIGEST_BYTES_V1]; UPSTREAM_DIGESTS_V1] {
        [
            self.prior_context_digest,
            self.added_inventory_root,
            self.statement3_proof_set_root,
            self.statement3_verified_transcript_root,
            self.statement5_proof_set_root,
            self.statement5_verified_transcript_root,
            self.statement8_proof_set_root,
            self.statement8_verified_transcript_root,
            self.q_mask_proof_set_root,
            self.q_mask_verified_transcript_root,
        ]
    }

    fn is_valid_v1(self) -> bool {
        unique_nonzero_digests_v1(&self.digests_v1())
    }
}

fn unique_nonzero_digests_v1(digests: &[[u8; DIGEST_BYTES_V1]]) -> bool {
    for (ordinal, digest) in digests.iter().enumerate() {
        if *digest == [0; DIGEST_BYTES_V1] || digests[..ordinal].contains(digest) {
            return false;
        }
    }
    true
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ExistingRadixCoordinateV1 {
    ordinal: usize,
    group: usize,
    role: u8,
    column: usize,
}

fn coordinate_v1(
    ordinal: usize,
) -> Result<ExistingRadixCoordinateV1, RnsNativeExistingRadixCommitmentViewErrorV1> {
    if ordinal >= INVENTORY_POINTS_V1 {
        return Err(RnsNativeExistingRadixCommitmentViewErrorV1::InvalidGeometry);
    }
    let group = ordinal / POINTS_PER_GROUP_V1;
    let local = ordinal % POINTS_PER_GROUP_V1;
    let (role, column) = if local < LOW_DIGITS_V1 {
        (ROLE_DIFFERENCE_LOW_V1, local)
    } else {
        (ROLE_SLACK_LOW_V1, local - LOW_DIGITS_V1)
    };
    Ok(ExistingRadixCoordinateV1 {
        ordinal,
        group,
        role,
        column,
    })
}

fn pre_z_manifest_digest_v1() -> [u8; DIGEST_BYTES_V1] {
    let mut hash = Keccak256::new();
    hash.update(PRE_Z_MANIFEST_DOMAIN_V1);
    hash.update(&[VERSION_V1]);
    for value in [
        GROUPS_V1 as u32,
        ROLES_V1 as u32,
        LOW_DIGITS_V1 as u32,
        POINTS_PER_GROUP_V1 as u32,
        INVENTORY_POINTS_V1 as u32,
        INVENTORY_BYTES_V1 as u32,
    ] {
        hash.update(&value.to_be_bytes());
    }
    hash.update(&VEGA_T256_SCALAR_MODULUS_BE_V1);
    hash.update(&ZK_AMS_T256_BP_GENERATOR_BASIS_DIGEST_V1);
    for language in [POINT_ORDER_LANGUAGE_V1, SOLE_Z_SEPARATION_LANGUAGE_V1] {
        hash.update(&(language.len() as u16).to_be_bytes());
        hash.update(language);
    }
    hash.finalize()
}

fn canonical_pre_z_candidate_root_v1(
    inventory: &[u8],
) -> Result<[u8; DIGEST_BYTES_V1], RnsNativeExistingRadixCommitmentViewErrorV1> {
    if inventory.len() != INVENTORY_BYTES_V1 {
        return Err(RnsNativeExistingRadixCommitmentViewErrorV1::InvalidGeometry);
    }
    let mut hash = Keccak256::new();
    hash.update(PRE_Z_CANDIDATE_ROOT_DOMAIN_V1);
    hash.update(&[VERSION_V1]);
    hash.update(&pre_z_manifest_digest_v1());
    for ordinal in 0..INVENTORY_POINTS_V1 {
        let coordinate = coordinate_v1(ordinal)?;
        let offset = ordinal
            .checked_mul(POINT_BYTES_V1)
            .ok_or(RnsNativeExistingRadixCommitmentViewErrorV1::ArithmeticOverflow)?;
        let end = offset
            .checked_add(POINT_BYTES_V1)
            .ok_or(RnsNativeExistingRadixCommitmentViewErrorV1::ArithmeticOverflow)?;
        let encoded = inventory
            .get(offset..end)
            .ok_or(RnsNativeExistingRadixCommitmentViewErrorV1::InvalidGeometry)?;
        Point::from_non_identity_wire_bytes_exact(encoded)
            .map_err(|_| RnsNativeExistingRadixCommitmentViewErrorV1::InvalidPoint)?;
        hash.update(
            &u32::try_from(coordinate.ordinal)
                .map_err(|_| RnsNativeExistingRadixCommitmentViewErrorV1::ArithmeticOverflow)?
                .to_be_bytes(),
        );
        hash.update(
            &u16::try_from(coordinate.group)
                .map_err(|_| RnsNativeExistingRadixCommitmentViewErrorV1::ArithmeticOverflow)?
                .to_be_bytes(),
        );
        hash.update(&[coordinate.role, coordinate.column as u8]);
        hash.update(encoded);
    }
    let digest = hash.finalize();
    if digest == [0; DIGEST_BYTES_V1] {
        return Err(RnsNativeExistingRadixCommitmentViewErrorV1::InvalidIntegrity);
    }
    Ok(digest)
}

fn absorb_upstream_v1(hash: &mut Keccak256, upstream: UpstreamBindingV1) {
    for digest in upstream.digests_v1() {
        hash.update(&digest);
    }
}

fn canonical_residual_digest_v1(
    upstream: UpstreamBindingV1,
    pre_z_candidate_root: [u8; DIGEST_BYTES_V1],
    residual: &[u8],
) -> Result<[u8; DIGEST_BYTES_V1], RnsNativeExistingRadixCommitmentViewErrorV1> {
    if !upstream.is_valid_v1()
        || pre_z_candidate_root == [0; DIGEST_BYTES_V1]
        || residual.is_empty()
    {
        return Err(RnsNativeExistingRadixCommitmentViewErrorV1::InvalidContext);
    }
    let mut hash = Keccak256::new();
    hash.update(RESIDUAL_DOMAIN_V1);
    hash.update(&[VERSION_V1]);
    absorb_upstream_v1(&mut hash, upstream);
    hash.update(&pre_z_candidate_root);
    hash.update(
        &u32::try_from(residual.len())
            .map_err(|_| RnsNativeExistingRadixCommitmentViewErrorV1::ArithmeticOverflow)?
            .to_be_bytes(),
    );
    hash.update(residual);
    let digest = hash.finalize();
    if digest == [0; DIGEST_BYTES_V1] {
        return Err(RnsNativeExistingRadixCommitmentViewErrorV1::InvalidIntegrity);
    }
    Ok(digest)
}

fn codec_digest_v1(bytes: &[u8]) -> [u8; DIGEST_BYTES_V1] {
    let mut hash = Keccak256::new();
    hash.update(CODEC_DOMAIN_V1);
    hash.update(&[VERSION_V1]);
    hash.update(bytes);
    hash.finalize()
}

struct DecoderV1<'a> {
    bytes: &'a [u8],
    cursor: usize,
}

impl<'a> DecoderV1<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, cursor: 0 }
    }

    fn take(
        &mut self,
        count: usize,
    ) -> Result<&'a [u8], RnsNativeExistingRadixCommitmentViewErrorV1> {
        let end = self
            .cursor
            .checked_add(count)
            .ok_or(RnsNativeExistingRadixCommitmentViewErrorV1::ArithmeticOverflow)?;
        let value = self
            .bytes
            .get(self.cursor..end)
            .ok_or(RnsNativeExistingRadixCommitmentViewErrorV1::InvalidHeader)?;
        self.cursor = end;
        Ok(value)
    }

    fn array<const N: usize>(
        &mut self,
    ) -> Result<[u8; N], RnsNativeExistingRadixCommitmentViewErrorV1> {
        self.take(N)?
            .try_into()
            .map_err(|_| RnsNativeExistingRadixCommitmentViewErrorV1::InvalidHeader)
    }

    fn u8(&mut self) -> Result<u8, RnsNativeExistingRadixCommitmentViewErrorV1> {
        self.take(1)?
            .first()
            .copied()
            .ok_or(RnsNativeExistingRadixCommitmentViewErrorV1::InvalidHeader)
    }

    fn u16(&mut self) -> Result<u16, RnsNativeExistingRadixCommitmentViewErrorV1> {
        Ok(u16::from_be_bytes(self.array()?))
    }

    fn u32(&mut self) -> Result<u32, RnsNativeExistingRadixCommitmentViewErrorV1> {
        Ok(u32::from_be_bytes(self.array()?))
    }
}

#[derive(Clone, Copy)]
struct ExistingRadixProofViewV1<'a> {
    inventory: &'a [u8],
    residual: &'a [u8],
    pre_z_candidate_root: [u8; DIGEST_BYTES_V1],
    residual_digest: [u8; DIGEST_BYTES_V1],
    codec_digest: [u8; DIGEST_BYTES_V1],
}

impl<'a> ExistingRadixProofViewV1<'a> {
    #[cfg(test)]
    fn from_components_v1(
        bytes: &'a [u8],
        expected: UpstreamBindingV1,
    ) -> Result<Self, RnsNativeExistingRadixCommitmentViewErrorV1> {
        let (view, upstream) = Self::from_self_consistent_components_v1(bytes)?;
        if upstream.digests_v1() != expected.digests_v1() {
            return Err(RnsNativeExistingRadixCommitmentViewErrorV1::InvalidHeader);
        }
        Ok(view)
    }

    /// Exact-decode and authenticate the candidate transport without granting
    /// authority to its header-carried predecessor digests.  The returned
    /// upstream binding must later be consumed against the verified statement
    /// 3/5/8/10-11 chain before this view can become an ordinary prerequisite.
    fn from_self_consistent_components_v1(
        bytes: &'a [u8],
    ) -> Result<(Self, UpstreamBindingV1), RnsNativeExistingRadixCommitmentViewErrorV1> {
        if bytes.len() > RNS_NATIVE_Q_MASK_LINEAR_RESIDUAL_MAX_BYTES_V1 {
            return Err(RnsNativeExistingRadixCommitmentViewErrorV1::ProofCapExceeded);
        }
        if bytes.len() < MIN_WIRE_BYTES_V1 {
            return Err(RnsNativeExistingRadixCommitmentViewErrorV1::InvalidHeader);
        }
        let mut decoder = DecoderV1::new(bytes);
        if decoder.array::<4>()? != MAGIC_V1
            || decoder.u8()? != VERSION_V1
            || decoder.u8()? != FLAGS_V1
            || usize::from(decoder.u16()?) != HEADER_BYTES_V1
            || usize::try_from(decoder.u32()?)
                .map_err(|_| RnsNativeExistingRadixCommitmentViewErrorV1::ArithmeticOverflow)?
                != bytes.len()
            || usize::from(decoder.u16()?) != GROUPS_V1
            || usize::from(decoder.u8()?) != ROLES_V1
            || usize::from(decoder.u8()?) != LOW_DIGITS_V1
            || usize::from(decoder.u8()?) != POINTS_PER_GROUP_V1
            || usize::from(decoder.u8()?) != POINT_BYTES_V1
            || usize::try_from(decoder.u32()?)
                .map_err(|_| RnsNativeExistingRadixCommitmentViewErrorV1::ArithmeticOverflow)?
                != INVENTORY_POINTS_V1
            || usize::try_from(decoder.u32()?)
                .map_err(|_| RnsNativeExistingRadixCommitmentViewErrorV1::ArithmeticOverflow)?
                != INVENTORY_BYTES_V1
        {
            return Err(RnsNativeExistingRadixCommitmentViewErrorV1::InvalidGeometry);
        }
        let upstream = UpstreamBindingV1 {
            prior_context_digest: decoder.array()?,
            added_inventory_root: decoder.array()?,
            statement3_proof_set_root: decoder.array()?,
            statement3_verified_transcript_root: decoder.array()?,
            statement5_proof_set_root: decoder.array()?,
            statement5_verified_transcript_root: decoder.array()?,
            statement8_proof_set_root: decoder.array()?,
            statement8_verified_transcript_root: decoder.array()?,
            q_mask_proof_set_root: decoder.array()?,
            q_mask_verified_transcript_root: decoder.array()?,
        };
        let pre_z_candidate_root = decoder.array()?;
        let residual_digest = decoder.array()?;
        let residual_len = usize::try_from(decoder.u32()?)
            .map_err(|_| RnsNativeExistingRadixCommitmentViewErrorV1::ArithmeticOverflow)?;
        let expected_total = HEADER_BYTES_V1
            .checked_add(INVENTORY_BYTES_V1)
            .and_then(|value| value.checked_add(residual_len))
            .and_then(|value| value.checked_add(CODEC_DIGEST_BYTES_V1))
            .ok_or(RnsNativeExistingRadixCommitmentViewErrorV1::ArithmeticOverflow)?;
        let mut bound_digests = upstream.digests_v1().to_vec();
        bound_digests.extend([pre_z_candidate_root, residual_digest]);
        if decoder.cursor != HEADER_BYTES_V1
            || residual_len == 0
            || residual_len > RNS_NATIVE_EXISTING_RADIX_RESIDUAL_MAX_BYTES_V1
            || expected_total != bytes.len()
            || !upstream.is_valid_v1()
            || !unique_nonzero_digests_v1(&bound_digests)
        {
            return Err(RnsNativeExistingRadixCommitmentViewErrorV1::InvalidHeader);
        }
        let inventory = decoder.take(INVENTORY_BYTES_V1)?;
        let residual = decoder.take(residual_len)?;
        let codec_offset = decoder.cursor;
        let codec_digest = decoder.array()?;
        bound_digests.push(codec_digest);
        if decoder.cursor != bytes.len()
            || canonical_pre_z_candidate_root_v1(inventory)? != pre_z_candidate_root
            || canonical_residual_digest_v1(upstream, pre_z_candidate_root, residual)?
                != residual_digest
            || !unique_nonzero_digests_v1(&bound_digests)
            || codec_digest_v1(&bytes[..codec_offset]) != codec_digest
        {
            return Err(RnsNativeExistingRadixCommitmentViewErrorV1::InvalidIntegrity);
        }
        Ok((
            Self {
                inventory,
                residual,
                pre_z_candidate_root,
                residual_digest,
                codec_digest,
            },
            upstream,
        ))
    }

    fn point_v1(
        self,
        group: usize,
        role: u8,
        column: usize,
    ) -> Result<Point, RnsNativeExistingRadixCommitmentViewErrorV1> {
        if group >= GROUPS_V1
            || column >= LOW_DIGITS_V1
            || ![ROLE_DIFFERENCE_LOW_V1, ROLE_SLACK_LOW_V1].contains(&role)
        {
            return Err(RnsNativeExistingRadixCommitmentViewErrorV1::InvalidGeometry);
        }
        let role_offset = usize::from(role - 1)
            .checked_mul(LOW_DIGITS_V1)
            .ok_or(RnsNativeExistingRadixCommitmentViewErrorV1::ArithmeticOverflow)?;
        let ordinal = group
            .checked_mul(POINTS_PER_GROUP_V1)
            .and_then(|value| value.checked_add(role_offset))
            .and_then(|value| value.checked_add(column))
            .ok_or(RnsNativeExistingRadixCommitmentViewErrorV1::ArithmeticOverflow)?;
        let offset = ordinal
            .checked_mul(POINT_BYTES_V1)
            .ok_or(RnsNativeExistingRadixCommitmentViewErrorV1::ArithmeticOverflow)?;
        Point::from_non_identity_wire_bytes_exact(
            self.inventory
                .get(offset..offset + POINT_BYTES_V1)
                .ok_or(RnsNativeExistingRadixCommitmentViewErrorV1::InvalidGeometry)?,
        )
        .map_err(|_| RnsNativeExistingRadixCommitmentViewErrorV1::InvalidPoint)
    }
}

/// Exact original-radix commitments for one 16,384-coordinate group.
///
/// The low vectors come from this stage.  `difference_top` and `slack_top`
/// alias the original cross-field inventory and have no second wire owner.
#[derive(Clone, Copy)]
pub(super) struct ExistingRadixCommitmentsV1 {
    pub(super) difference_low: [Point; LOW_DIGITS_V1],
    pub(super) slack_low: [Point; LOW_DIGITS_V1],
    pub(super) difference_top: Point,
    pub(super) slack_top: Point,
}

fn exact_subslice_range_v1(base: &[u8], child: &[u8]) -> Option<(usize, usize)> {
    let base_start = base.as_ptr() as usize;
    let base_end = base_start.checked_add(base.len())?;
    let child_start = child.as_ptr() as usize;
    let child_end = child_start.checked_add(child.len())?;
    if child_start < base_start || child_end > base_end {
        return None;
    }
    Some((child_start.checked_sub(base_start)?, child.len()))
}

fn exact_slice_at_v1(base: &[u8], offset: usize, len: usize) -> Option<&[u8]> {
    let end = offset.checked_add(len)?;
    base.get(offset..end)
}

fn same_slice_identity_v1(left: &[u8], right: &[u8]) -> bool {
    left.len() == right.len() && core::ptr::eq(left.as_ptr(), right.as_ptr())
}

#[allow(
    missing_copy_implementations,
    dead_code,
    reason = "the candidate axis must move with the sole direct schedule"
)]
pub(super) struct RnsNativeExistingRadixCandidateAxisV1 {
    origin: RnsNativeExistingRadixCandidateAxisOriginV1,
}

enum RnsNativeExistingRadixCandidateAxisOriginV1 {
    Preflight {
        root: [u8; DIGEST_BYTES_V1],
        base_address: usize,
        base_len: usize,
        existing_wire_offset: usize,
        existing_wire_len: usize,
        inventory_offset: usize,
        inventory_len: usize,
    },
    #[cfg(test)]
    RawFixture { root: [u8; DIGEST_BYTES_V1] },
}

impl RnsNativeExistingRadixCandidateAxisV1 {
    pub(super) fn is_valid_with_fixed_axes_v1(&self, other_axes: &[[u8; DIGEST_BYTES_V1]]) -> bool {
        let root = match &self.origin {
            RnsNativeExistingRadixCandidateAxisOriginV1::Preflight {
                root,
                base_address,
                base_len,
                existing_wire_offset,
                existing_wire_len,
                inventory_offset,
                inventory_len,
            } => {
                if *base_address == 0
                    || *base_len == 0
                    || *existing_wire_len == 0
                    || *inventory_len != INVENTORY_BYTES_V1
                    || existing_wire_offset
                        .checked_add(*existing_wire_len)
                        .is_none_or(|end| end > *base_len)
                    || inventory_offset
                        .checked_add(*inventory_len)
                        .is_none_or(|end| end > *base_len)
                {
                    return false;
                }
                root
            }
            #[cfg(test)]
            RnsNativeExistingRadixCandidateAxisOriginV1::RawFixture { root } => root,
        };
        *root != [0; DIGEST_BYTES_V1] && !other_axes.contains(root)
    }

    pub(super) fn absorb_fixed_axis_v1(&self, hash: &mut Keccak256) {
        match &self.origin {
            RnsNativeExistingRadixCandidateAxisOriginV1::Preflight { root, .. } => {
                hash.update(root);
            }
            #[cfg(test)]
            RnsNativeExistingRadixCandidateAxisOriginV1::RawFixture { root } => {
                hash.update(root);
            }
        }
    }

    /// Complete only the direct verifier's purpose-bound safe-core projection.
    /// The candidate root never escapes this opaque axis as a detached value.
    pub(super) fn complete_verified_safe_core_projection_v1(
        &self,
        mut projection: RnsNativeCrossFieldRlweSafeCoreProjectionV1,
    ) -> Result<
        RnsNativeCrossFieldRlweSafeCoreProjectionV1,
        RnsNativeExistingRadixCommitmentViewErrorV1,
    > {
        if projection.existing_radix_candidate_root != [0; DIGEST_BYTES_V1] {
            return Err(RnsNativeExistingRadixCommitmentViewErrorV1::InvalidContext);
        }
        projection.existing_radix_candidate_root = match &self.origin {
            RnsNativeExistingRadixCandidateAxisOriginV1::Preflight { root, .. } => *root,
            #[cfg(test)]
            RnsNativeExistingRadixCandidateAxisOriginV1::RawFixture { root } => *root,
        };
        Ok(projection)
    }

    pub(super) fn permits_full_direct_bind_v1(&self) -> bool {
        matches!(
            &self.origin,
            RnsNativeExistingRadixCandidateAxisOriginV1::Preflight { .. }
        ) || cfg!(test)
    }

    pub(super) fn matches_authenticated_alias_v1(
        &self,
        base: &[u8],
        alias: &RnsNativeExistingRadixDirectAliasV1<'_>,
    ) -> bool {
        match (&self.origin, &alias.origin) {
            (
                RnsNativeExistingRadixCandidateAxisOriginV1::Preflight {
                    root,
                    base_address,
                    base_len,
                    existing_wire_offset,
                    existing_wire_len,
                    inventory_offset,
                    inventory_len,
                },
                RnsNativeExistingRadixAuthenticatedOriginV1 {
                    root: authenticated_root,
                    base_address: authenticated_base_address,
                    base_len: authenticated_base_len,
                    existing_wire_offset: authenticated_wire_offset,
                    existing_wire_len: authenticated_wire_len,
                    inventory_offset: authenticated_inventory_offset,
                    inventory_len: authenticated_inventory_len,
                },
            ) => {
                base.as_ptr() as usize == *base_address
                    && base.len() == *base_len
                    && base_address == authenticated_base_address
                    && base_len == authenticated_base_len
                    && existing_wire_offset == authenticated_wire_offset
                    && existing_wire_len == authenticated_wire_len
                    && inventory_offset == authenticated_inventory_offset
                    && inventory_len == authenticated_inventory_len
                    && root == authenticated_root
                    && exact_slice_at_v1(base, *inventory_offset, *inventory_len)
                        .is_some_and(|inventory| same_slice_identity_v1(inventory, alias.inventory))
            }
            #[cfg(test)]
            (
                RnsNativeExistingRadixCandidateAxisOriginV1::RawFixture { root },
                RnsNativeExistingRadixAuthenticatedOriginV1 {
                    root: authenticated_root,
                    ..
                },
            ) => root == authenticated_root,
            #[cfg(test)]
            _ => false,
        }
    }

    #[cfg(test)]
    pub(super) fn test_fixture_v1(
        root: [u8; DIGEST_BYTES_V1],
    ) -> Result<Self, RnsNativeExistingRadixCommitmentViewErrorV1> {
        let value = Self {
            origin: RnsNativeExistingRadixCandidateAxisOriginV1::RawFixture { root },
        };
        if !value.is_valid_with_fixed_axes_v1(&[]) {
            return Err(RnsNativeExistingRadixCommitmentViewErrorV1::InvalidContext);
        }
        Ok(value)
    }
}

#[allow(
    missing_copy_implementations,
    dead_code,
    reason = "the exact allocation permit must be consumed by ordinary authentication once"
)]
pub(super) struct RnsNativeExistingRadixValidationPermitV1 {
    base_address: usize,
    base_len: usize,
    existing_wire_offset: usize,
    existing_wire_len: usize,
    inventory_offset: usize,
    inventory_len: usize,
    residual_offset: usize,
    residual_len: usize,
    upstream: UpstreamBindingV1,
    pre_z_candidate_root: [u8; DIGEST_BYTES_V1],
    residual_digest: [u8; DIGEST_BYTES_V1],
    codec_digest: [u8; DIGEST_BYTES_V1],
}

/// Opaque one-shot pair minted by the sole schedule-free recursive preflight.
/// The direct module may split it only while retaining both children beside
/// the exact inventory owner.
#[allow(
    missing_copy_implementations,
    dead_code,
    reason = "axis and validation permit must separate only inside the atomic direct bind"
)]
pub(super) struct RnsNativeExistingRadixPreDirectSplitV1 {
    pub(super) axis: RnsNativeExistingRadixCandidateAxisV1,
    pub(super) permit: RnsNativeExistingRadixValidationPermitV1,
}

struct RnsNativeExistingRadixAuthenticatedOriginV1 {
    root: [u8; DIGEST_BYTES_V1],
    base_address: usize,
    base_len: usize,
    existing_wire_offset: usize,
    existing_wire_len: usize,
    inventory_offset: usize,
    inventory_len: usize,
}

/// Perform the sole 11,696-point validation/root pass over the exact existing-
/// radix child of an already exact-decoded cross-inventory continuation.
pub(super) fn preflight_rns_native_existing_radix_candidate_v1(
    base: &[u8],
    existing_wire: &[u8],
) -> Result<RnsNativeExistingRadixPreDirectSplitV1, RnsNativeExistingRadixCommitmentViewErrorV1> {
    let (existing_wire_offset, existing_wire_len) = exact_subslice_range_v1(base, existing_wire)
        .ok_or(RnsNativeExistingRadixCommitmentViewErrorV1::InvalidContext)?;
    let (view, upstream) =
        ExistingRadixProofViewV1::from_self_consistent_components_v1(existing_wire)?;
    let (inventory_offset, inventory_len) = exact_subslice_range_v1(base, view.inventory)
        .ok_or(RnsNativeExistingRadixCommitmentViewErrorV1::InvalidContext)?;
    let (residual_offset, residual_len) = exact_subslice_range_v1(base, view.residual)
        .ok_or(RnsNativeExistingRadixCommitmentViewErrorV1::InvalidContext)?;
    let base_address = base.as_ptr() as usize;
    if base_address == 0
        || inventory_len != INVENTORY_BYTES_V1
        || exact_slice_at_v1(base, existing_wire_offset, existing_wire_len)
            .is_none_or(|wire| !same_slice_identity_v1(wire, existing_wire))
        || exact_slice_at_v1(base, inventory_offset, inventory_len)
            .is_none_or(|inventory| !same_slice_identity_v1(inventory, view.inventory))
        || exact_slice_at_v1(base, residual_offset, residual_len)
            .is_none_or(|residual| !same_slice_identity_v1(residual, view.residual))
    {
        return Err(RnsNativeExistingRadixCommitmentViewErrorV1::InvalidContext);
    }
    Ok(RnsNativeExistingRadixPreDirectSplitV1 {
        axis: RnsNativeExistingRadixCandidateAxisV1 {
            origin: RnsNativeExistingRadixCandidateAxisOriginV1::Preflight {
                root: view.pre_z_candidate_root,
                base_address,
                base_len: base.len(),
                existing_wire_offset,
                existing_wire_len,
                inventory_offset,
                inventory_len,
            },
        },
        permit: RnsNativeExistingRadixValidationPermitV1 {
            base_address,
            base_len: base.len(),
            existing_wire_offset,
            existing_wire_len,
            inventory_offset,
            inventory_len,
            residual_offset,
            residual_len,
            upstream,
            pre_z_candidate_root: view.pre_z_candidate_root,
            residual_digest: view.residual_digest,
            codec_digest: view.codec_digest,
        },
    })
}

impl RnsNativeExistingRadixValidationPermitV1 {
    fn bind_verified_predecessor_v1<'proof, S>(
        self,
        previous: &RnsNativeQMaskLinearRelationsPrerequisiteV1<'_, 'proof, S>,
    ) -> Result<
        (
            ExistingRadixProofViewV1<'proof>,
            RnsNativeExistingRadixAuthenticatedOriginV1,
        ),
        RnsNativeExistingRadixCommitmentViewErrorV1,
    >
    where
        S: ZkAmsMkheRnsNativeSourceSnapshotV1,
    {
        let base = previous
            .previous()
            .previous()
            .previous()
            .inventory()
            .continuation();
        let wire = exact_slice_at_v1(base, self.existing_wire_offset, self.existing_wire_len)
            .ok_or(RnsNativeExistingRadixCommitmentViewErrorV1::InvalidContext)?;
        let inventory = exact_slice_at_v1(base, self.inventory_offset, self.inventory_len)
            .ok_or(RnsNativeExistingRadixCommitmentViewErrorV1::InvalidContext)?;
        let residual = exact_slice_at_v1(base, self.residual_offset, self.residual_len)
            .ok_or(RnsNativeExistingRadixCommitmentViewErrorV1::InvalidContext)?;
        let expected_upstream = UpstreamBindingV1::from_prerequisite_v1(previous);
        if base.as_ptr() as usize != self.base_address
            || base.len() != self.base_len
            || !same_slice_identity_v1(wire, previous.residual())
            || self.inventory_len != INVENTORY_BYTES_V1
            || self.inventory_offset
                != self
                    .existing_wire_offset
                    .checked_add(HEADER_BYTES_V1)
                    .ok_or(RnsNativeExistingRadixCommitmentViewErrorV1::ArithmeticOverflow)?
            || self.residual_offset
                != self
                    .inventory_offset
                    .checked_add(INVENTORY_BYTES_V1)
                    .ok_or(RnsNativeExistingRadixCommitmentViewErrorV1::ArithmeticOverflow)?
            || self
                .residual_offset
                .checked_add(self.residual_len)
                .and_then(|end| end.checked_add(CODEC_DIGEST_BYTES_V1))
                != self
                    .existing_wire_offset
                    .checked_add(self.existing_wire_len)
            || self.upstream.digests_v1() != expected_upstream.digests_v1()
            || [
                self.pre_z_candidate_root,
                self.residual_digest,
                self.codec_digest,
            ]
            .contains(&[0; DIGEST_BYTES_V1])
        {
            return Err(RnsNativeExistingRadixCommitmentViewErrorV1::InvalidContext);
        }
        Ok((
            ExistingRadixProofViewV1 {
                inventory,
                residual,
                pre_z_candidate_root: self.pre_z_candidate_root,
                residual_digest: self.residual_digest,
                codec_digest: self.codec_digest,
            },
            RnsNativeExistingRadixAuthenticatedOriginV1 {
                root: self.pre_z_candidate_root,
                base_address: self.base_address,
                base_len: self.base_len,
                existing_wire_offset: self.existing_wire_offset,
                existing_wire_len: self.existing_wire_len,
                inventory_offset: self.inventory_offset,
                inventory_len: self.inventory_len,
            },
        ))
    }
}

/// Move-only verifier alias to the exact authenticated `D`-low point bytes.
///
/// Construction is available only by consuming the authenticated
/// existing-radix prerequisite below. The alias borrows the already-owned
/// 385,968-byte point inventory, copies only its candidate root and binding
/// digest, exposes no raw byte slice, and cannot be built from detached
/// points. `D`-top remains under the original cross-field inventory owner.
#[allow(
    missing_copy_implementations,
    dead_code,
    reason = "the verifier-only direct handoff consumes this exact alias once"
)]
pub(super) struct RnsNativeExistingRadixDirectAliasV1<'proof> {
    inventory: &'proof [u8],
    pre_z_candidate_root: [u8; DIGEST_BYTES_V1],
    binding_digest: [u8; DIGEST_BYTES_V1],
    origin: RnsNativeExistingRadixAuthenticatedOriginV1,
}

#[allow(
    dead_code,
    reason = "the constructor-less verifier point adapter is preparatory and non-authorizing"
)]
impl RnsNativeExistingRadixDirectAliasV1<'_> {
    /// Decode only the authenticated `D`-low role. Slack points and raw bytes
    /// are intentionally unavailable through this direct-verifier alias.
    pub(super) fn difference_low_commitment_v1(&self, group: usize, digit: usize) -> Option<Point> {
        ExistingRadixProofViewV1 {
            inventory: self.inventory,
            residual: &[],
            pre_z_candidate_root: self.pre_z_candidate_root,
            residual_digest: [0; DIGEST_BYTES_V1],
            codec_digest: [0; DIGEST_BYTES_V1],
        }
        .point_v1(group, ROLE_DIFFERENCE_LOW_V1, digit)
        .ok()
    }
}

fn existing_radix_commitments_v1<F>(
    view: ExistingRadixProofViewV1<'_>,
    group: usize,
    mut top_at: F,
) -> Option<ExistingRadixCommitmentsV1>
where
    F: FnMut(usize) -> Option<(Point, Point)>,
{
    if group >= GROUPS_V1 {
        return None;
    }
    let first_difference = view.point_v1(group, ROLE_DIFFERENCE_LOW_V1, 0).ok()?;
    let first_slack = view.point_v1(group, ROLE_SLACK_LOW_V1, 0).ok()?;
    let mut difference_low = [first_difference; LOW_DIGITS_V1];
    let mut slack_low = [first_slack; LOW_DIGITS_V1];
    for column in 1..LOW_DIGITS_V1 {
        difference_low[column] = view.point_v1(group, ROLE_DIFFERENCE_LOW_V1, column).ok()?;
        slack_low[column] = view.point_v1(group, ROLE_SLACK_LOW_V1, column).ok()?;
    }
    let (difference_top, slack_top) = top_at(group)?;
    if difference_top.is_identity() || slack_top.is_identity() {
        return None;
    }
    Some(ExistingRadixCommitmentsV1 {
        difference_low,
        slack_low,
        difference_top,
        slack_top,
    })
}

fn prerequisite_binding_digest_v1<S: ZkAmsMkheRnsNativeSourceSnapshotV1>(
    previous: &RnsNativeQMaskLinearRelationsPrerequisiteV1<'_, '_, S>,
    view: ExistingRadixProofViewV1<'_>,
) -> Result<[u8; DIGEST_BYTES_V1], RnsNativeExistingRadixCommitmentViewErrorV1> {
    let upstream = UpstreamBindingV1::from_prerequisite_v1(previous);
    let mut hash = Keccak256::new();
    hash.update(PREREQUISITE_DOMAIN_V1);
    hash.update(&[VERSION_V1]);
    absorb_upstream_v1(&mut hash, upstream);
    // These two q-mask digests contain this complete wire and therefore enter
    // only after decoding.  They are never inputs to the pre-z candidate root.
    for digest in [
        previous.residual_digest(),
        previous.binding_digest(),
        view.pre_z_candidate_root,
        view.residual_digest,
        view.codec_digest,
        pre_z_manifest_digest_v1(),
        ZK_AMS_T256_BP_GENERATOR_BASIS_DIGEST_V1,
    ] {
        hash.update(&digest);
    }
    let digest = hash.finalize();
    if digest == [0; DIGEST_BYTES_V1] {
        return Err(RnsNativeExistingRadixCommitmentViewErrorV1::InvalidIntegrity);
    }
    Ok(digest)
}

/// Move-only, private evidence that the exact existing-radix commitment view
/// has been authenticated after statements 3, 5, 8, 10, and 11.
///
/// This is not range, inverse, product, lookup, readiness, release, or
/// authorization evidence.
#[allow(
    missing_copy_implementations,
    reason = "the q-mask owner and unverified downstream residual must advance exactly once"
)]
pub(super) struct RnsNativeExistingRadixCommitmentPrerequisiteV1<
    'source,
    'proof,
    S: ZkAmsMkheRnsNativeSourceSnapshotV1,
> {
    previous: RnsNativeQMaskLinearRelationsPrerequisiteV1<'source, 'proof, S>,
    inventory: &'proof [u8],
    residual: &'proof [u8],
    pre_z_candidate_root: [u8; DIGEST_BYTES_V1],
    residual_digest: [u8; DIGEST_BYTES_V1],
    binding_digest: [u8; DIGEST_BYTES_V1],
    authenticated_origin: RnsNativeExistingRadixAuthenticatedOriginV1,
}

impl<'source, 'proof, S: ZkAmsMkheRnsNativeSourceSnapshotV1>
    RnsNativeExistingRadixCommitmentPrerequisiteV1<'source, 'proof, S>
{
    pub(super) const fn previous(
        &self,
    ) -> &RnsNativeQMaskLinearRelationsPrerequisiteV1<'source, 'proof, S> {
        &self.previous
    }

    pub(super) const fn residual(&self) -> &'proof [u8] {
        self.residual
    }

    /// Candidate-only root for the future sole-z transcript.
    ///
    /// Callers must combine it only with separately audited pre-z axes; neither
    /// `binding_digest` nor any predecessor proof/transcript root is safe there.
    pub(super) const fn pre_z_candidate_root(&self) -> [u8; DIGEST_BYTES_V1] {
        self.pre_z_candidate_root
    }

    pub(super) const fn residual_digest(&self) -> [u8; DIGEST_BYTES_V1] {
        self.residual_digest
    }

    pub(super) const fn binding_digest(&self) -> [u8; DIGEST_BYTES_V1] {
        self.binding_digest
    }

    /// Consume the authenticated existing-radix view and recover its exact
    /// q-mask predecessor.
    pub(super) fn into_previous_v1(
        self,
    ) -> RnsNativeQMaskLinearRelationsPrerequisiteV1<'source, 'proof, S> {
        self.previous
    }

    /// Consume the authenticated stage and its recursively owned claimed
    /// direct predecessor through one purpose-bound no-copy alias.
    ///
    /// The intervening q-mask, small-sign, range-carry, and comparator owners
    /// are consumed here only after their later descendants have verified.
    /// No alias/predecessor tuple or claimed-successor parts can escape.
    #[allow(
        dead_code,
        reason = "the one-argument membership handoff consumes this exact transition"
    )]
    pub(super) fn verify_claimed_direct_v2(
        self,
    ) -> Result<
        RnsNativeCrossFieldRlweAtomicVerifiedV2<'source, 'proof, S>,
        RnsNativeCrossFieldRlweDirectErrorV1,
    > {
        let alias = RnsNativeExistingRadixDirectAliasV1 {
            inventory: self.inventory,
            pre_z_candidate_root: self.pre_z_candidate_root,
            binding_digest: self.binding_digest,
            origin: self.authenticated_origin,
        };
        let small_sign = self.previous.into_previous_v1();
        let range_carry = small_sign.into_previous_v1();
        let comparator = range_carry.into_previous_v1();
        let claimed_successor = comparator.into_previous_v1();
        claimed_successor.verify_claimed_direct_with_alias_v2(alias)
    }

    pub(super) fn existing_radix_commitments(
        &self,
        group: usize,
    ) -> Option<ExistingRadixCommitmentsV1> {
        let view = ExistingRadixProofViewV1 {
            inventory: self.inventory,
            residual: self.residual,
            pre_z_candidate_root: self.pre_z_candidate_root,
            residual_digest: self.residual_digest,
            codec_digest: [0; DIGEST_BYTES_V1],
        };
        let inventory = self.previous.previous().previous().previous().inventory();
        existing_radix_commitments_v1(view, group, |owner| {
            inventory.comparator_top_commitments(owner)
        })
    }
}

/// Consume the statement-10/11 owner into the exact existing-radix view.
#[allow(
    dead_code,
    reason = "the private transport entry awaits its radix algebra consumer"
)]
pub(super) fn authenticate_rns_native_existing_radix_commitment_view_v1<'source, 'proof, S>(
    mut previous: RnsNativeQMaskLinearRelationsPrerequisiteV1<'source, 'proof, S>,
) -> Result<
    RnsNativeExistingRadixCommitmentPrerequisiteV1<'source, 'proof, S>,
    RnsNativeExistingRadixCommitmentViewErrorV1,
>
where
    S: ZkAmsMkheRnsNativeSourceSnapshotV1,
{
    let permit = previous
        .take_existing_radix_validation_permit_v1()
        .ok_or(RnsNativeExistingRadixCommitmentViewErrorV1::InvalidContext)?;
    let (view, authenticated_origin) = permit.bind_verified_predecessor_v1(&previous)?;
    let binding_digest = prerequisite_binding_digest_v1(&previous, view)?;
    Ok(RnsNativeExistingRadixCommitmentPrerequisiteV1 {
        previous,
        inventory: view.inventory,
        residual: view.residual,
        pre_z_candidate_root: view.pre_z_candidate_root,
        residual_digest: view.residual_digest,
        binding_digest,
        authenticated_origin,
    })
}

#[cfg(test)]
#[path = "rns_native_existing_radix_commitment_view_tests.rs"]
mod tests;
