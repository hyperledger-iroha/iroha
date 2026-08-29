//! Exact compact collective RNS-BGV key and ciphertext core.
//!
//! Production encryption and evaluation retain only bounded streaming
//! authorities and manifests. The native two- and three-polynomial ciphertext
//! owners remain `cfg(test)` reference implementations. Secret RLWE
//! coefficients never cross the public API boundary.
#[cfg(test)]
use super::active_exact_binding::mint_test_state_owned_collective_secret_binding_v1;
use super::{
    BgvProfile, MAX_RANDOM_REJECTION_ATTEMPTS_V1, MKHE_VERSION_V1, MaskedRelaxedRandomSourceV1,
    RnsPolynomial, Scalar, SecretPolynomial, ZkAmsMkheErrorV1, ZkAmsMkhePartyIdV1,
    active::{
        ZkAmsMkheActiveCollectivePublicKeyStatementV1, ZkAmsMkheActiveCollectivePublicKeyWitnessV1,
        ZkAmsMkheActivePartySecretV1, ZkAmsMkheActiveRkgProofV1, ZkAmsMkheGovernedActiveRosterV1,
        prove_zk_ams_mkhe_active_collective_public_key_v1,
        verify_zk_ams_mkhe_active_collective_public_key_v1,
        zk_ams_mkhe_active_collective_public_a_v1,
    },
    active_exact_binding::{PersistentWitnessConsumerV1, VerifiedPersistentWitnessBindingV1},
    checked_coefficient_work, checked_ring_multiplication_work,
    cpk_relation::{
        ZK_AMS_MKHE_CPK_PARTY_B_OBJECT_BYTES_V1, ZkAmsMkheCpkPartyBPointerV1,
        prepare_active_collective_public_a_v1,
    },
    manifest::{
        ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1, release_profile_v1, zk_ams_mkhe_release_manifest_v1,
        zk_ams_mkhe_security_certificate_v1,
    },
    packing::{ZkAmsT256PackedPlaintextV1, ZkAmsT256PackingLayoutV1},
    persistent_membership_evidence::{
        ZK_AMS_MKHE_PERSISTENT_MEMBERSHIP_CHUNKS_V1, ZkAmsMkhePersistentMembershipContextV1,
        ZkAmsMkhePersistentMembershipErrorV1, ZkAmsMkhePersistentMembershipEvidenceV1,
    },
    wire::{ZkAmsMkheRnsPolynomialWireV1, governed_roster_digest},
};
#[cfg(test)]
use super::{
    packing::packed_plaintext_to_rns_v1,
    wire::{
        ZkAmsMkheCollectiveCiphertextWireV1, ZkAmsMkheGovernedRosterWireV1, ZkAmsMkheWireBindingV1,
    },
};
#[cfg(test)]
use crate::vega::sponge::keccak256;
use crate::{
    generalized_bulletproof::ProofRandomSource,
    vega::{
        VegaT256PointV1 as Point,
        bulletproof_t256::{
            ZK_AMS_MEMBERSHIP_CHUNK_COEFFICIENTS_V1, ZkAmsT256MembershipBoundV1,
            commit_zk_ams_t256_membership_chunk_v1,
        },
        sponge::Keccak256,
    },
};
#[path = "collective/borrowed_product.rs"]
pub(super) mod borrowed_product;
#[path = "collective/incremental_source.rs"]
mod incremental_source;
#[path = "collective/party_local_rkg_ephemeral_v1.rs"]
mod party_local_rkg_ephemeral_v1;
#[path = "collective/persistent_direct_opening_v1.rs"]
mod persistent_direct_opening_v1;
#[path = "collective/prepared_public_a.rs"]
mod prepared_public_a;
pub(in crate::vega::zk_ams::mkhe) use incremental_source::{
    RnsNativeClaimedDirectNumericOriginV2, RnsNativeQpcsCompositeAuthorityV2,
};
#[expect(
    unused_imports,
    reason = "sealed sibling-only streaming capabilities share one narrow reexport seam"
)]
pub(super) use incremental_source::{
    ZkAmsMkheStreamingCollectiveAutomorphismOutputV1,
    ZkAmsMkheStreamingCollectiveCiphertextBindingV1, ZkAmsMkheStreamingCollectiveEvalAdmissionV1,
    ZkAmsMkheStreamingCollectiveEvalKeyBindingV1, ZkAmsMkheStreamingCollectiveKeyAdmissionV1,
    bind_zk_ams_mkhe_streaming_collective_eval_key_v1,
    fork_zk_ams_mkhe_staged_collective_key_admission_v1,
    mint_zk_ams_mkhe_streaming_collective_encryption_key_authority_v1,
    prepare_zk_ams_mkhe_streaming_collective_automorphism_output_v1,
};
pub use incremental_source::{
    ZkAmsMkheStreamingCollectiveCiphertextV1, ZkAmsMkheStreamingCollectiveEncryptionKeyAuthorityV1,
    encrypt_zk_ams_mkhe_collective_packed_streaming_v1,
};
pub(super) use party_local_rkg_ephemeral_v1::DirectRkgOneProverSessionV1;
use party_local_rkg_ephemeral_v1::PartyLocalRkgEphemeralOpeningV1;
pub(in crate::vega::zk_ams::mkhe) use party_local_rkg_ephemeral_v1::{
    DirectRkgOneProofDurabilityPermitV2, DirectRkgOnePublicationOwnerV1,
};
use persistent_direct_opening_v1::{PersistentDirectOpeningAxesV1, PersistentDirectOpeningOwnerV1};
pub use prepared_public_a::{
    ZkAmsMkhePreparedCollectivePublicAV1, prepare_zk_ams_mkhe_collective_public_a_v1,
};
pub(super) const COLLECTIVE_CIPHERTEXT_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.compact-collective-ciphertext";
const COLLECTIVE_PARTY_SHARE_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.collective-public-key-share";
const COLLECTIVE_PARTY_SHARE_ACTIVE_ADMISSION_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.collective-public-key-share.active-admission";
const COLLECTIVE_PARTY_SHARE_STAGED_ADMISSION_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.collective-public-key-share.staged-admission";
const COLLECTIVE_PUBLIC_KEY_STAGED_ADMISSION_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.collective-public-key.staged-admission";
const COLLECTIVE_PUBLIC_KEY_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.collective-public-key";
const CKS_RNS_NATIVE_DIGEST_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.rns-polynomial-digest";
const CKS_RNS_WIRE_DIGEST_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.rns-polynomial";
const COLLECTIVE_ENCRYPTION_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.collective-encryption";
const COLLECTIVE_ENCRYPTION_NONCE_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.collective-encryption-nonce";
#[cfg(test)]
const COLLECTIVE_ADD_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.collective-add";
#[cfg(test)]
const COLLECTIVE_SUB_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.collective-sub";
#[cfg(test)]
const COLLECTIVE_PLAINTEXT_MUL_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.collective-plaintext-mul";
#[cfg(test)]
const COLLECTIVE_AUTOMORPHISM_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.collective-automorphism";
#[cfg(test)]
const COLLECTIVE_MULTIPLY_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.collective-multiply";
#[cfg(test)]
const COLLECTIVE_LEVEL_ONE_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.collective-level-one";
fn clear_secret_bytes_v1(bytes: &mut [u8]) {
    let bytes = core::hint::black_box(bytes);
    bytes.fill(0);
    core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
    let _ = core::hint::black_box(&mut *bytes);
}
fn clear_secret_i8_slice_v1(values: &mut [i8]) {
    let values = core::hint::black_box(values);
    values.fill(0);
    core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
    let _ = core::hint::black_box(&mut *values);
}
fn clear_secret_i64_slice_v1(values: &mut [i64]) {
    let values = core::hint::black_box(values);
    values.fill(0);
    core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
    let _ = core::hint::black_box(&mut *values);
}
fn clear_secret_u64_slice_v1(values: &mut [u64]) {
    let values = core::hint::black_box(values);
    values.fill(0);
    core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
    let _ = core::hint::black_box(&mut *values);
}
#[cfg(test)]
fn clear_secret_canonical_plaintext_v1(values: &mut [[u8; 32]]) {
    let values = core::hint::black_box(values);
    for value in values.iter_mut() {
        clear_secret_bytes_v1(value);
    }
    core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
    let _ = core::hint::black_box(&mut *values);
}
pub(super) struct ZeroizingRns(RnsPolynomial);
impl ZeroizingRns {
    pub(super) fn from_canonical_flat_v1(
        profile: &BgvProfile,
        mut coefficients: Vec<u64>,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        profile.validate()?;
        let expected = profile
            .ring_degree
            .checked_mul(profile.moduli.len())
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        if coefficients.len() != expected
            || coefficients
                .chunks_exact(profile.ring_degree)
                .enumerate()
                .any(|(limb, values)| values.iter().any(|value| *value >= profile.moduli[limb]))
        {
            clear_secret_u64_slice_v1(&mut coefficients);
            return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
        }
        Ok(Self(RnsPolynomial { coefficients }))
    }
    pub(super) fn zero_exact_v1(profile: &BgvProfile) -> Result<Self, ZkAmsMkheErrorV1> {
        profile.validate()?;
        let coefficient_count = profile
            .ring_degree
            .checked_mul(profile.moduli.len())
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        let mut coefficients = Vec::new();
        coefficients
            .try_reserve_exact(coefficient_count)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        coefficients.resize(coefficient_count, 0);
        Ok(Self(RnsPolynomial { coefficients }))
    }
    pub(super) fn coefficients(&self) -> &[u64] {
        &self.0.coefficients
    }
    pub(super) fn coefficients_mut(&mut self) -> &mut [u64] {
        &mut self.0.coefficients
    }
    pub(super) fn into_public(mut self) -> RnsPolynomial {
        core::mem::replace(
            &mut self.0,
            RnsPolynomial {
                coefficients: Vec::new(),
            },
        )
    }
}
impl Drop for ZeroizingRns {
    fn drop(&mut self) {
        clear_secret_u64_slice_v1(&mut self.0.coefficients);
    }
}
#[cfg(test)]
struct ZeroizingSecretCoefficients(Vec<i64>);
#[cfg(test)]
impl Drop for ZeroizingSecretCoefficients {
    fn drop(&mut self) {
        clear_secret_i64_slice_v1(&mut self.0);
    }
}
/// Short-lived canonical T256 view of an owned RLWE secret.
///
/// The state remains the only long-lived owner. This adapter exists solely to
/// bind the complete CPK relation to the exact state opening and erases its
/// narrowed copy on every ordinary, error, and unwind exit.
pub(in crate::vega::zk_ams::mkhe) struct ZeroizingT256MembershipCoefficientsV1(Vec<i8>);
impl ZeroizingT256MembershipCoefficientsV1 {
    fn from_bounded(secret: &SecretPolynomial, bound: i8) -> Result<Self, ZkAmsMkheErrorV1> {
        let expected_coefficients = ZK_AMS_MKHE_PERSISTENT_MEMBERSHIP_CHUNKS_V1
            .checked_mul(ZK_AMS_MEMBERSHIP_CHUNK_COEFFICIENTS_V1)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        if secret.coefficients.len() != expected_coefficients {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        // Establish the erasing owner before any subsequent fallible
        // conversion so a malformed coefficient cannot leave a populated
        // ordinary `Vec` to be freed without clearing it.
        let mut coefficients = Self(Vec::new());
        coefficients
            .0
            .try_reserve_exact(expected_coefficients)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        if coefficients.0.capacity() != expected_coefficients {
            return Err(ZkAmsMkheErrorV1::ResourceCeilingExceeded);
        }
        let allocation = coefficients.0.as_ptr();
        for coefficient in secret.coefficients.iter().copied() {
            let coefficient =
                i8::try_from(coefficient).map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?;
            if bound < 1 || !(-bound..=bound).contains(&coefficient) {
                return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
            }
            coefficients.0.push(coefficient);
        }
        if coefficients.0.len() != expected_coefficients
            || coefficients.0.capacity() != expected_coefficients
            || coefficients.0.as_ptr() != allocation
        {
            return Err(ZkAmsMkheErrorV1::ResourceCeilingExceeded);
        }
        Ok(coefficients)
    }
    fn as_slice(&self) -> &[i8] {
        &self.0
    }
}
impl Drop for ZeroizingT256MembershipCoefficientsV1 {
    fn drop(&mut self) {
        clear_secret_i8_slice_v1(&mut self.0);
    }
}
const PERSISTENT_OPENING_POINT_WIRE_BYTES_V1: usize = 33;
/// Exact canonical payload retained for the eight state-owned public points.
pub(super) const ZK_AMS_MKHE_PERSISTENT_OPENING_RETAINED_POINT_BYTES_V1: usize =
    ZK_AMS_MKHE_PERSISTENT_MEMBERSHIP_CHUNKS_V1 * PERSISTENT_OPENING_POINT_WIRE_BYTES_V1;
type PersistentOpeningCommitmentWireV1 =
    [[u8; PERSISTENT_OPENING_POINT_WIRE_BYTES_V1]; ZK_AMS_MKHE_PERSISTENT_MEMBERSHIP_CHUNKS_V1];
fn encode_persistent_opening_commitments_v1(
    commitments: &[Point; ZK_AMS_MKHE_PERSISTENT_MEMBERSHIP_CHUNKS_V1],
) -> Result<PersistentOpeningCommitmentWireV1, ZkAmsMkheErrorV1> {
    let mut encoded = [[0_u8; PERSISTENT_OPENING_POINT_WIRE_BYTES_V1];
        ZK_AMS_MKHE_PERSISTENT_MEMBERSHIP_CHUNKS_V1];
    for (commitment, destination) in commitments.iter().zip(encoded.iter_mut()) {
        commitment
            .write_non_identity_wire_bytes_ref(destination)
            .map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?;
    }
    Ok(encoded)
}
fn commit_cpk_membership_opening_v1(
    coefficients: &[i8],
    blindings: &[Scalar; ZK_AMS_MKHE_PERSISTENT_MEMBERSHIP_CHUNKS_V1],
    bound: ZkAmsT256MembershipBoundV1,
) -> Result<[Point; ZK_AMS_MKHE_PERSISTENT_MEMBERSHIP_CHUNKS_V1], ZkAmsMkheErrorV1> {
    let chunks = coefficients.chunks_exact(ZK_AMS_MEMBERSHIP_CHUNK_COEFFICIENTS_V1);
    if !chunks.remainder().is_empty() || chunks.len() != blindings.len() {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    let mut commitments = Vec::new();
    commitments
        .try_reserve_exact(blindings.len())
        .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    if commitments.capacity() != blindings.len() {
        return Err(ZkAmsMkheErrorV1::ResourceCeilingExceeded);
    }
    let allocation = commitments.as_ptr();
    for (chunk, blinding) in chunks.zip(blindings.iter()) {
        commitments.push(
            commit_zk_ams_t256_membership_chunk_v1(bound, chunk, blinding)
                .map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?,
        );
    }
    if commitments.len() != blindings.len()
        || commitments.capacity() != blindings.len()
        || commitments.as_ptr() != allocation
    {
        return Err(ZkAmsMkheErrorV1::ResourceCeilingExceeded);
    }
    commitments
        .try_into()
        .map_err(|_: Vec<Point>| ZkAmsMkheErrorV1::InvalidKeyMaterial)
}
const _: () = {
    assert!(PERSISTENT_OPENING_POINT_WIRE_BYTES_V1 == 33);
    assert!(ZK_AMS_MKHE_PERSISTENT_OPENING_RETAINED_POINT_BYTES_V1 == 264);
    assert!(core::mem::size_of::<PersistentOpeningCommitmentWireV1>() == 264);
};
fn ensure_state_owned_cpk_commitments_v1(
    verified: &[Point; ZK_AMS_MKHE_PERSISTENT_MEMBERSHIP_CHUNKS_V1],
    expected: &[Point; ZK_AMS_MKHE_PERSISTENT_MEMBERSHIP_CHUNKS_V1],
) -> Result<(), ZkAmsMkheErrorV1> {
    if verified != expected {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    Ok(())
}
#[cfg(test)]
struct ZeroizingCanonicalPlaintext(Vec<[u8; 32]>);
#[cfg(test)]
impl Drop for ZeroizingCanonicalPlaintext {
    fn drop(&mut self) {
        clear_secret_canonical_plaintext_v1(&mut self.0);
    }
}
struct ZeroizingEntropyProbe([u8; 32]);
impl Drop for ZeroizingEntropyProbe {
    fn drop(&mut self) {
        clear_secret_bytes_v1(&mut self.0);
    }
}
/// Heap-stable owner for the opening-only fresh-encryption nonce.
///
/// The allocation is created while it is still all zero, then filled in
/// place. Moving this owner therefore moves only a pointer, never live nonce
/// bytes.
struct ZeroizingEncryptionNonce(Box<[u8; 32]>);
#[cfg(test)]
std::thread_local! {
    static ENCRYPTION_NONCE_ZEROIZED_DROPS_V1: std::cell::Cell<usize> = const {
        std::cell::Cell::new(0)
    };
}
impl ZeroizingEncryptionNonce {
    fn zeroed() -> Self {
        Self(Box::new([0; 32]))
    }
    fn as_bytes(&self) -> &[u8; 32] {
        self.0.as_ref()
    }
    fn as_mut_bytes(&mut self) -> &mut [u8; 32] {
        self.0.as_mut()
    }
    fn is_zero(&self) -> bool {
        self.as_bytes() == &[0; 32]
    }
}
impl Drop for ZeroizingEncryptionNonce {
    fn drop(&mut self) {
        clear_secret_bytes_v1(self.0.as_mut());
        #[cfg(test)]
        if self.is_zero() {
            let _ = ENCRYPTION_NONCE_ZEROIZED_DROPS_V1
                .try_with(|drops| drops.set(drops.get().saturating_add(1)));
        }
    }
}
struct ZeroizingRandomByte([u8; 1]);
impl Drop for ZeroizingRandomByte {
    fn drop(&mut self) {
        clear_secret_bytes_v1(&mut self.0);
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CollectiveEncryptionInputTopologyV1 {
    layout_digest: [u8; 32],
    plaintext_chunk_index: u32,
    plaintext_used_slots: u32,
}
impl CollectiveEncryptionInputTopologyV1 {
    const fn from_packed(
        layout: ZkAmsT256PackingLayoutV1,
        plaintext: &ZkAmsT256PackedPlaintextV1,
    ) -> Self {
        Self {
            layout_digest: layout.digest,
            plaintext_chunk_index: plaintext.chunk_index,
            plaintext_used_slots: plaintext.used_slots,
        }
    }
}
/// Move-only fresh-encryption identity. The opaque nonce never enters the
/// public ciphertext; only its domain-separated transcript digest does.
struct CollectiveEncryptionInputIdentityV1 {
    topology: CollectiveEncryptionInputTopologyV1,
    encryption_nonce: ZeroizingEncryptionNonce,
}
/// Test-only secret encryption witness retained by the native reference path.
///
/// This value intentionally implements neither `Clone` nor serialization.
/// It validates the complete public context and both RLWE equations before a
/// reference proof adapter may consume it; no witness reference crosses that
/// boundary.
#[cfg(test)]
pub(super) struct ZkAmsMkheCollectiveEncryptionOpeningV1 {
    profile_digest: [u8; 32],
    security_certificate_digest: [u8; 32],
    roster_digest: [u8; 32],
    key_material_digest: [u8; 32],
    key_digest: [u8; 32],
    epoch: u64,
    key_transcript_digest: [u8; 32],
    ciphertext_transcript_digest: [u8; 32],
    sample_index: u64,
    input_identity: CollectiveEncryptionInputIdentityV1,
    ciphertext_digest: [u8; 32],
    canonical_plaintext: ZeroizingCanonicalPlaintext,
    plaintext_lift: ZeroizingRns,
    ephemeral: SecretPolynomial,
    error_zero: SecretPolynomial,
    error_one: SecretPolynomial,
    #[cfg(test)]
    drop_audit: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
}
#[cfg(test)]
impl core::fmt::Debug for ZkAmsMkheCollectiveEncryptionOpeningV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("ZkAmsMkheCollectiveEncryptionOpeningV1")
            .field("profile_digest", &hex::encode(self.profile_digest))
            .field(
                "security_certificate_digest",
                &hex::encode(self.security_certificate_digest),
            )
            .field("roster_digest", &hex::encode(self.roster_digest))
            .field(
                "key_material_digest",
                &hex::encode(self.key_material_digest),
            )
            .field("key_digest", &hex::encode(self.key_digest))
            .field("epoch", &self.epoch)
            .field(
                "key_transcript_digest",
                &hex::encode(self.key_transcript_digest),
            )
            .field(
                "ciphertext_transcript_digest",
                &hex::encode(self.ciphertext_transcript_digest),
            )
            .field("sample_index", &self.sample_index)
            .field("input_topology", &self.input_identity.topology)
            .field("encryption_nonce", &"[REDACTED]")
            .field("ciphertext_digest", &hex::encode(self.ciphertext_digest))
            .field("canonical_plaintext", &"[REDACTED]")
            .field("plaintext_lift", &"[REDACTED]")
            .field("ephemeral", &"[REDACTED]")
            .field("error_zero", &"[REDACTED]")
            .field("error_one", &"[REDACTED]")
            .finish()
    }
}
#[cfg(test)]
impl Drop for ZkAmsMkheCollectiveEncryptionOpeningV1 {
    fn drop(&mut self) {
        clear_secret_bytes_v1(self.input_identity.encryption_nonce.as_mut_bytes());
        clear_secret_canonical_plaintext_v1(&mut self.canonical_plaintext.0);
        clear_secret_u64_slice_v1(&mut self.plaintext_lift.0.coefficients);
        clear_secret_i64_slice_v1(&mut self.ephemeral.coefficients);
        clear_secret_i64_slice_v1(&mut self.error_zero.coefficients);
        clear_secret_i64_slice_v1(&mut self.error_one.coefficients);
        #[cfg(test)]
        if let Some(audit) = &self.drop_audit {
            let zeroized = self.input_identity.encryption_nonce.is_zero()
                && self
                    .canonical_plaintext
                    .0
                    .iter()
                    .all(|coefficient| *coefficient == [0; 32])
                && self
                    .plaintext_lift
                    .0
                    .coefficients
                    .iter()
                    .all(|coefficient| *coefficient == 0)
                && self
                    .ephemeral
                    .coefficients
                    .iter()
                    .chain(&self.error_zero.coefficients)
                    .chain(&self.error_one.coefficients)
                    .all(|coefficient| *coefficient == 0);
            audit.store(zeroized, std::sync::atomic::Ordering::SeqCst);
        }
    }
}
const PERSISTENT_BLINDING_ENTROPY_BYTES_V1: usize = 64;
const PERSISTENT_BLINDING_CANONICAL_BYTES_V1: usize = 32;
const PERSISTENT_BLINDING_STATE_BYTES_V1: usize =
    ZK_AMS_MKHE_PERSISTENT_MEMBERSHIP_CHUNKS_V1 * PERSISTENT_BLINDING_CANONICAL_BYTES_V1;
/// One fallible entropy request whose named bytes are erased on every exit.
///
/// Scalar reduction borrows the fixed array, avoiding an unmanaged array copy;
/// this owner covers the complete caller-visible buffer across errors and
/// unwinds.
struct PersistentSecretCommitmentBlindingEntropyV1([u8; PERSISTENT_BLINDING_ENTROPY_BYTES_V1]);
impl PersistentSecretCommitmentBlindingEntropyV1 {
    const fn zeroed() -> Self {
        Self([0; PERSISTENT_BLINDING_ENTROPY_BYTES_V1])
    }
    fn as_mut_slice(&mut self) -> &mut [u8] {
        &mut self.0
    }
    fn as_array(&self) -> &[u8; PERSISTENT_BLINDING_ENTROPY_BYTES_V1] {
        &self.0
    }
}
impl Drop for PersistentSecretCommitmentBlindingEntropyV1 {
    fn drop(&mut self) {
        let bytes = core::hint::black_box(&mut self.0);
        bytes.fill(0);
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
        let _ = core::hint::black_box(&mut *bytes);
        #[cfg(test)]
        if bytes.iter().all(|byte| *byte == 0) {
            let _ = PERSISTENT_BLINDING_ENTROPY_ZEROIZED_DROPS_V1
                .try_with(|drops| drops.set(drops.get().saturating_add(1)));
        }
    }
}
/// Move-only owner of the eight persistent membership-commitment blindings.
///
/// This type deliberately implements neither `Clone`, `Copy`, `Default`, nor
/// serialization. Only an immutable array borrow crosses the sibling-module
/// proof boundary, and every named scalar is erased on drop.
struct ZeroizingCpkMembershipBlindingsV1([Scalar; ZK_AMS_MKHE_PERSISTENT_MEMBERSHIP_CHUNKS_V1]);
impl ZeroizingCpkMembershipBlindingsV1 {
    fn sample<R: MaskedRelaxedRandomSourceV1>(random: &mut R) -> Result<Self, ZkAmsMkheErrorV1> {
        // Own the complete zero-initialized array before making the first
        // fallible or panicking entropy request. This makes every partial
        // construction path subject to the owner's destructor.
        let mut owner = Self([Scalar::zero(); ZK_AMS_MKHE_PERSISTENT_MEMBERSHIP_CHUNKS_V1]);
        for blinding in &mut owner.0 {
            let mut accepted = false;
            for _ in 0..MAX_RANDOM_REJECTION_ATTEMPTS_V1 {
                let mut entropy = PersistentSecretCommitmentBlindingEntropyV1::zeroed();
                random
                    .fill_bytes(entropy.as_mut_slice())
                    .map_err(|_| ZkAmsMkheErrorV1::RandomUnavailable)?;
                *blinding = Scalar::from_uniform_le_bytes_ref(entropy.as_array());
                if !blinding.is_zero() {
                    accepted = true;
                    break;
                }
            }
            if !accepted {
                return Err(ZkAmsMkheErrorV1::RandomUnavailable);
            }
        }
        Ok(owner)
    }
    /// Borrow the exact ordered blindings inside the sealed owner/lease path.
    ///
    /// No by-value or mutable access is exposed: the party state remains the
    /// sole owner while the precursor is produced and until the state drops.
    const fn as_array(&self) -> &[Scalar; ZK_AMS_MKHE_PERSISTENT_MEMBERSHIP_CHUNKS_V1] {
        &self.0
    }
}
impl core::fmt::Debug for ZeroizingCpkMembershipBlindingsV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("PersistentSecretCommitmentBlindingsV1([REDACTED])")
    }
}
impl Drop for ZeroizingCpkMembershipBlindingsV1 {
    fn drop(&mut self) {
        let blindings = core::hint::black_box(&mut self.0);
        for blinding in blindings.iter_mut() {
            blinding.clear_secret();
        }
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
        let _ = core::hint::black_box(&mut *blindings);
        #[cfg(test)]
        if blindings.iter().all(|blinding| blinding.is_zero()) {
            let _ = PERSISTENT_BLINDING_OWNER_ZEROIZED_DROPS_V1
                .try_with(|drops| drops.set(drops.get().saturating_add(1)));
        }
    }
}
const _: () = {
    assert!(ZK_AMS_MKHE_PERSISTENT_MEMBERSHIP_CHUNKS_V1 == 8);
    assert!(PERSISTENT_BLINDING_STATE_BYTES_V1 == 256);
    assert!(core::mem::size_of::<ZeroizingCpkMembershipBlindingsV1>() == 256);
};
#[cfg(test)]
std::thread_local! {
    static PERSISTENT_BLINDING_ENTROPY_ZEROIZED_DROPS_V1: std::cell::Cell<usize> = const {
        std::cell::Cell::new(0)
    };
    static PERSISTENT_BLINDING_OWNER_ZEROIZED_DROPS_V1: std::cell::Cell<usize> = const {
        std::cell::Cell::new(0)
    };
}
/// Exclusive, lifetime-scoped access to one state-owned persistent opening.
///
/// The sibling CPK adapter can consume this value but cannot construct it or
/// obtain either the secret coefficients or the original blindings.
pub(super) struct PersistentDirectOpeningLeaseV1<'a> {
    owner: &'a mut PersistentDirectOpeningOwnerV1,
    coefficients: ZeroizingT256MembershipCoefficientsV1,
}
impl<'a> PersistentDirectOpeningLeaseV1<'a> {
    pub(super) const fn profile_digest(&self) -> [u8; 32] {
        self.owner.axes.profile_digest
    }
    pub(super) const fn security_certificate_digest(&self) -> [u8; 32] {
        self.owner.axes.security_certificate_digest
    }
    pub(super) const fn roster_digest(&self) -> [u8; 32] {
        self.owner.axes.roster_digest
    }
    pub(super) const fn key_material_digest(&self) -> [u8; 32] {
        self.owner.axes.key_material_digest
    }
    pub(super) const fn epoch(&self) -> u64 {
        self.owner.axes.epoch
    }
    pub(super) const fn cpk_transcript_digest(&self) -> [u8; 32] {
        self.owner.axes.cpk_transcript_digest
    }
    pub(super) const fn party_index(&self) -> usize {
        self.owner.axes.party_index as usize
    }
    pub(super) const fn party(&self) -> ZkAmsMkhePartyIdV1 {
        self.owner.axes.party
    }
    pub(super) const fn public_share_digest(&self) -> [u8; 32] {
        self.owner.axes.public_share_digest
    }
    pub(super) fn validate_secret_membership_v1(
        &self,
        evidence: &ZkAmsMkhePersistentMembershipEvidenceV1,
    ) -> Result<(), ZkAmsMkhePersistentMembershipErrorV1> {
        let encoded = encode_persistent_opening_commitments_v1(&evidence.commitments())
            .map_err(|_| ZkAmsMkhePersistentMembershipErrorV1::DigestMismatch)?;
        if encoded != self.owner.retained_commitment_wire {
            return Err(ZkAmsMkhePersistentMembershipErrorV1::DigestMismatch);
        }
        Ok(())
    }
    /// Consume the exclusive lease into public membership evidence.
    pub(super) fn prove<R: ProofRandomSource>(
        self,
        context: ZkAmsMkhePersistentMembershipContextV1,
        random: &mut R,
    ) -> Result<ZkAmsMkhePersistentMembershipEvidenceV1, ZkAmsMkhePersistentMembershipErrorV1> {
        let evidence = ZkAmsMkhePersistentMembershipEvidenceV1::prove(
            context,
            self.coefficients.as_slice(),
            self.owner.blindings.as_array(),
            random,
        )?;
        self.validate_secret_membership_v1(&evidence)?;
        Ok(evidence)
    }
    pub(super) fn into_reopened_v1<R: MaskedRelaxedRandomSourceV1>(
        self,
        error_coefficients: ZeroizingT256MembershipCoefficientsV1,
        random: &mut R,
    ) -> Result<ReopenedCpkDirectOpeningLeaseV1<'a>, ZkAmsMkheErrorV1> {
        Ok(ReopenedCpkDirectOpeningLeaseV1 {
            opening: self,
            error_coefficients,
            error_blindings: ZeroizingCpkMembershipBlindingsV1::sample(random)?,
        })
    }
}
pub(super) struct ReopenedCpkDirectOpeningLeaseV1<'a> {
    opening: PersistentDirectOpeningLeaseV1<'a>,
    error_coefficients: ZeroizingT256MembershipCoefficientsV1,
    error_blindings: ZeroizingCpkMembershipBlindingsV1,
}
impl ReopenedCpkDirectOpeningLeaseV1<'_> {
    pub(super) fn prove_error_membership_v1<R: ProofRandomSource>(
        &self,
        context: super::cpk_relation::ZkAmsMkheCpkErrorMembershipContextV1,
        random: &mut R,
    ) -> Result<
        super::cpk_relation::ZkAmsMkheCpkErrorMembershipEvidenceV1,
        super::cpk_relation::ZkAmsMkheCpkRelationErrorV1,
    > {
        super::cpk_relation::ZkAmsMkheCpkErrorMembershipEvidenceV1::prove(
            context,
            self.error_coefficients.as_slice(),
            self.error_blindings.as_array(),
            random,
        )
    }
    pub(super) fn consume_sealed_cpk_abort_session_v1<R: MaskedRelaxedRandomSourceV1>(
        self,
        session: super::cpk_relation::state_owned_secret_adapter_v1::StateOwnedCpkSealedAbortSessionV1,
        roster: &ZkAmsMkheGovernedActiveRosterV1,
        share: &ZkAmsMkheCollectivePublicKeyShareV1,
        random: &mut R,
    ) -> Result<
        super::cpk_relation::state_owned_secret_adapter_v1::StateOwnedCpkProvedPublicV1,
        super::cpk_relation::ZkAmsMkheCpkRelationErrorV1,
    > {
        session.prove_with_opening_v1(
            roster,
            share,
            self.opening.coefficients.as_slice(),
            self.error_coefficients.as_slice(),
            self.opening.owner.blindings.as_array(),
            self.error_blindings.as_array(),
            random,
        )
    }
}
/// Opaque RLWE state owned by one exact governed party and secret epoch.
///
/// The ternary secret, centered-binomial public-key error, and eight persistent
/// commitment blindings are generated internally, are redacted from debug
/// output, and are zeroized by their underlying secret containers on drop.
/// The state is move-only so the persistent secret material has one owner:
///
/// ```compile_fail,E0277
/// use iroha_zkp_halo2::vega::ZkAmsMkheCollectivePartyStateV1;
///
/// fn requires_clone<T: Clone>() {}
/// requires_clone::<ZkAmsMkheCollectivePartyStateV1>();
/// ```
pub struct ZkAmsMkheCollectivePartyStateV1 {
    persistent_direct_opening: PersistentDirectOpeningOwnerV1,
    public_error: SecretPolynomial,
    party_local_rkg_ephemeral_opening: Option<PartyLocalRkgEphemeralOpeningV1>,
    party_local_rkg_ephemeral_creation_mask: u64,
}
impl core::fmt::Debug for ZkAmsMkheCollectivePartyStateV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("ZkAmsMkheCollectivePartyStateV1")
            .field(
                "profile_digest",
                &hex::encode(self.persistent_direct_opening.axes.profile_digest),
            )
            .field(
                "security_certificate_digest",
                &hex::encode(
                    self.persistent_direct_opening
                        .axes
                        .security_certificate_digest,
                ),
            )
            .field(
                "roster_digest",
                &hex::encode(self.persistent_direct_opening.axes.roster_digest),
            )
            .field("epoch", &self.persistent_direct_opening.axes.epoch)
            .field(
                "transcript_digest",
                &hex::encode(self.persistent_direct_opening.axes.cpk_transcript_digest),
            )
            .field(
                "party_index",
                &self.persistent_direct_opening.axes.party_index,
            )
            .field("party", &self.persistent_direct_opening.axes.party)
            .field(
                "public_share_digest",
                &hex::encode(self.persistent_direct_opening.axes.public_share_digest),
            )
            .field("persistent_direct_opening", &self.persistent_direct_opening)
            .field("public_error", &"[REDACTED]")
            .finish()
    }
}
impl ZkAmsMkheCollectivePartyStateV1 {
    #[cfg(test)]
    pub(super) const fn profile_digest(&self) -> [u8; 32] {
        self.persistent_direct_opening.axes.profile_digest
    }
    #[cfg(test)]
    pub(super) const fn roster_digest(&self) -> [u8; 32] {
        self.persistent_direct_opening.axes.roster_digest
    }
    /// Authentication-key-derived governed party identifier.
    #[must_use]
    pub const fn party(&self) -> ZkAmsMkhePartyIdV1 {
        self.persistent_direct_opening.axes.party
    }
    /// Exact zero-based position in the governed eight-party roster.
    #[must_use]
    pub const fn party_index(&self) -> u8 {
        self.persistent_direct_opening.axes.party_index
    }
    /// Digest of the matching verified public share.
    #[must_use]
    pub const fn public_share_digest(&self) -> [u8; 32] {
        self.persistent_direct_opening.axes.public_share_digest
    }
    /// Governed secret/key epoch.
    #[must_use]
    pub const fn epoch(&self) -> u64 {
        self.persistent_direct_opening.axes.epoch
    }
    /// Exact collective-key transcript.
    #[must_use]
    pub const fn transcript_digest(&self) -> [u8; 32] {
        self.persistent_direct_opening.axes.cpk_transcript_digest
    }
    pub(super) const fn secret(&self) -> &SecretPolynomial {
        &self.persistent_direct_opening.secret
    }
    pub(super) const fn public_error(&self) -> &SecretPolynomial {
        &self.public_error
    }
    pub(super) const fn profile_digest_internal(&self) -> [u8; 32] {
        self.persistent_direct_opening.axes.profile_digest
    }
    pub(super) const fn security_certificate_digest_internal(&self) -> [u8; 32] {
        self.persistent_direct_opening
            .axes
            .security_certificate_digest
    }
    pub(super) const fn roster_digest_internal(&self) -> [u8; 32] {
        self.persistent_direct_opening.axes.roster_digest
    }
    pub(super) const fn key_material_digest_internal(&self) -> [u8; 32] {
        self.persistent_direct_opening.axes.key_material_digest
    }
    fn validate_state_owned_cpk_source_v1(
        &self,
        roster: &ZkAmsMkheGovernedActiveRosterV1,
        share: &ZkAmsMkheCollectivePublicKeyShareV1,
    ) -> Result<(), ZkAmsMkheErrorV1> {
        roster.validate()?;
        let axes = &self.persistent_direct_opening.axes;
        axes.validate()?;
        let party_index = usize::from(axes.party_index);
        if axes.profile_digest != roster.profile_digest()
            || axes.security_certificate_digest != release_security_certificate_digest()?
            || axes.roster_digest != roster.roster_digest()
            || axes.key_material_digest != roster.key_material_digest()
            || axes.epoch != roster.epoch()
            || party_index >= ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1
            || axes.party != roster.participants()[party_index].party()
            || self.persistent_direct_opening.verified_binding.is_some()
            || share.profile_digest != axes.profile_digest
            || share.security_certificate_digest != axes.security_certificate_digest
            || share.roster_digest != axes.roster_digest
            || share.key_material_digest != axes.key_material_digest
            || share.epoch != axes.epoch
            || share.transcript_digest != axes.cpk_transcript_digest
            || share.party_index != axes.party_index
            || share.party != axes.party
            || share.digest != axes.public_share_digest
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        validate_collective_public_key_share(
            roster,
            axes.cpk_transcript_digest,
            party_index,
            share,
        )?;
        Ok(())
    }
    fn recompute_persistent_direct_commitments_v1(
        &self,
    ) -> Result<[Point; ZK_AMS_MKHE_PERSISTENT_MEMBERSHIP_CHUNKS_V1], ZkAmsMkheErrorV1> {
        self.persistent_direct_opening.axes.validate()?;
        let coefficients = ZeroizingT256MembershipCoefficientsV1::from_bounded(
            &self.persistent_direct_opening.secret,
            1,
        )?;
        let commitments = commit_cpk_membership_opening_v1(
            coefficients.as_slice(),
            self.persistent_direct_opening.blindings.as_array(),
            ZkAmsT256MembershipBoundV1::One,
        )?;
        let encoded = encode_persistent_opening_commitments_v1(&commitments)?;
        if encoded != self.persistent_direct_opening.retained_commitment_wire {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        Ok(commitments)
    }
    fn validated_cpk_secret_commitments_v1(
        &self,
        roster: &ZkAmsMkheGovernedActiveRosterV1,
        share: &ZkAmsMkheCollectivePublicKeyShareV1,
    ) -> Result<[Point; ZK_AMS_MKHE_PERSISTENT_MEMBERSHIP_CHUNKS_V1], ZkAmsMkheErrorV1> {
        self.validate_state_owned_cpk_source_v1(roster, share)?;
        self.recompute_persistent_direct_commitments_v1()
    }
    fn persistent_direct_opening_lease_v1<'a>(
        &'a mut self,
        roster: &ZkAmsMkheGovernedActiveRosterV1,
        share: &ZkAmsMkheCollectivePublicKeyShareV1,
    ) -> Result<PersistentDirectOpeningLeaseV1<'a>, ZkAmsMkheErrorV1> {
        self.validate_state_owned_cpk_source_v1(roster, share)?;
        let owner = &mut self.persistent_direct_opening;
        let coefficients = ZeroizingT256MembershipCoefficientsV1::from_bounded(&owner.secret, 1)?;
        let commitments = commit_cpk_membership_opening_v1(
            coefficients.as_slice(),
            owner.blindings.as_array(),
            ZkAmsT256MembershipBoundV1::One,
        )?;
        let encoded = encode_persistent_opening_commitments_v1(&commitments)?;
        if encoded != owner.retained_commitment_wire {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        Ok(PersistentDirectOpeningLeaseV1 {
            owner,
            coefficients,
        })
    }
    /// Produce only public, membership-only CPK precursor material.
    ///
    /// This method cannot mint a persistent witness binding.  The complete
    /// CPK relation verifier remains the sole authority for that capability.
    #[allow(dead_code)]
    pub(super) fn prove_state_owned_cpk_secret_membership_v1<R: ProofRandomSource>(
        &mut self,
        roster: &ZkAmsMkheGovernedActiveRosterV1,
        share: &ZkAmsMkheCollectivePublicKeyShareV1,
        party_b_pointer: ZkAmsMkheCpkPartyBPointerV1,
        random: &mut R,
    ) -> Result<
        super::cpk_relation::state_owned_secret_adapter_v1::StateOwnedCpkSecretMembershipPrecursorV1,
        ZkAmsMkheErrorV1,
    >{
        self.validate_state_owned_cpk_source_v1(roster, share)?;
        let expected_party_b_payload_blake3 = cpk_party_b_payload_blake3_v1(&share.party_public_b)?;
        if party_b_pointer.payload_blake3() != expected_party_b_payload_blake3 {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        let lease = self.persistent_direct_opening_lease_v1(roster, share)?;
        super::cpk_relation::state_owned_secret_adapter_v1::prove_state_owned_cpk_secret_membership_v1(
            roster,
            party_b_pointer,
            expected_party_b_payload_blake3,
            lease,
            random,
        )
    }
    /// Rejoin a public precursor to a fresh exclusive lease without exposing its opening.
    pub(super) fn reopen_state_owned_cpk_relation_v1<'a, R>(
        &'a mut self,
        roster: &ZkAmsMkheGovernedActiveRosterV1,
        share: &ZkAmsMkheCollectivePublicKeyShareV1,
        party_b_pointer: ZkAmsMkheCpkPartyBPointerV1,
        precursor: super::cpk_relation::state_owned_secret_adapter_v1::StateOwnedCpkSecretMembershipPrecursorV1,
        random: &mut R,
    ) -> Result<super::cpk_relation::state_owned_secret_adapter_v1::ReopenedStateOwnedCpkRelationPrecursorV1<'a>, ZkAmsMkheErrorV1>
    where
        R: ProofRandomSource + MaskedRelaxedRandomSourceV1,
    {
        self.validate_state_owned_cpk_source_v1(roster, share)?;
        let payload = cpk_party_b_payload_blake3_v1(share.party_public_b())?;
        if party_b_pointer.payload_blake3() != payload {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        let statement = super::cpk_relation::ZkAmsMkheCpkShareStatementV1::from_governed_roster(
            roster,
            self.transcript_digest(),
            usize::from(self.party_index()),
            party_b_pointer,
        )
        .map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?;
        let error_coefficients =
            ZeroizingT256MembershipCoefficientsV1::from_bounded(&self.public_error, 2)?;
        let opening = self.persistent_direct_opening_lease_v1(roster, share)?;
        super::cpk_relation::state_owned_secret_adapter_v1::reopen_state_owned_cpk_relation_precursor_v1(
            precursor,
            statement,
            share.digest(),
            opening,
            error_coefficients,
            random,
        )
    }
    /// Admit the move-only party binding atomically emitted with the verified set.
    #[allow(dead_code)]
    pub(super) fn admit_verified_cpk_binding(
        &mut self,
        roster: &ZkAmsMkheGovernedActiveRosterV1,
        share: &ZkAmsMkheCollectivePublicKeyShareV1,
        binding: VerifiedPersistentWitnessBindingV1,
    ) -> Result<&VerifiedPersistentWitnessBindingV1, ZkAmsMkheErrorV1> {
        let party_index = usize::from(self.party_index());
        let expected_commitments = self.validated_cpk_secret_commitments_v1(roster, share)?;
        binding.validate_for(
            roster,
            self.transcript_digest(),
            party_index,
            self.public_share_digest(),
            PersistentWitnessConsumerV1::CollectivePublicKey,
        )?;
        ensure_state_owned_cpk_commitments_v1(binding.commitments(), &expected_commitments)?;
        self.persistent_direct_opening.verified_binding = Some(binding);
        let binding = self
            .persistent_direct_opening
            .verified_binding
            .as_ref()
            .ok_or(ZkAmsMkheErrorV1::ReleaseUnavailable)?;
        binding.validate_for(
            roster,
            self.transcript_digest(),
            usize::from(self.party_index()),
            self.public_share_digest(),
            PersistentWitnessConsumerV1::CollectivePublicKey,
        )?;
        Ok(binding)
    }
    /// Admit the state successor emitted by the bounded staged CPK ceremony.
    ///
    /// The full share has already been consumed and released. Its sealed
    /// staged admission supplies the exact share lineage, while this state
    /// recomputes its own commitments before the binding is installed. Every
    /// check precedes the sole mutation, so failure leaves the state unchanged.
    pub(super) fn admit_staged_verified_cpk_binding_v1(
        &mut self,
        roster: &ZkAmsMkheGovernedActiveRosterV1,
        admission: &VerifiedCollectivePublicKeyShareStagedAdmissionV1,
        binding: VerifiedPersistentWitnessBindingV1,
    ) -> Result<(), ZkAmsMkheErrorV1> {
        roster.validate()?;
        let axes = &self.persistent_direct_opening.axes;
        axes.validate()?;
        let party_index = usize::from(axes.party_index);
        admission.validate_for_v1(roster, axes.cpk_transcript_digest, party_index)?;
        if axes.profile_digest != roster.profile_digest()
            || axes.security_certificate_digest != release_security_certificate_digest()?
            || axes.roster_digest != roster.roster_digest()
            || axes.key_material_digest != roster.key_material_digest()
            || axes.epoch != roster.epoch()
            || party_index >= ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1
            || axes.party != roster.participants()[party_index].party()
            || axes.public_share_digest != admission.share_digest
            || self.persistent_direct_opening.verified_binding.is_some()
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        binding.validate_for(
            roster,
            axes.cpk_transcript_digest,
            party_index,
            axes.public_share_digest,
            PersistentWitnessConsumerV1::CollectivePublicKey,
        )?;
        let expected_commitments = self.recompute_persistent_direct_commitments_v1()?;
        ensure_state_owned_cpk_commitments_v1(binding.commitments(), &expected_commitments)?;
        self.persistent_direct_opening.verified_binding = Some(binding);
        Ok(())
    }
    /// Borrow the cached capability for a specific consumer.  Absence is a
    /// release blocker, never a request to accept a raw digest fallback.
    #[allow(dead_code)]
    pub(super) fn persistent_secret_binding_for(
        &self,
        roster: &ZkAmsMkheGovernedActiveRosterV1,
        consumer: PersistentWitnessConsumerV1,
    ) -> Result<&VerifiedPersistentWitnessBindingV1, ZkAmsMkheErrorV1> {
        let binding = self
            .persistent_direct_opening
            .verified_binding
            .as_ref()
            .ok_or(ZkAmsMkheErrorV1::ReleaseUnavailable)?;
        binding.validate_for(
            roster,
            self.transcript_digest(),
            usize::from(self.party_index()),
            self.public_share_digest(),
            consumer,
        )?;
        Ok(binding)
    }
    #[cfg(test)]
    pub(super) fn test_state_owned_cpk_bindings_v1(
        &self,
        roster: &ZkAmsMkheGovernedActiveRosterV1,
        share: &ZkAmsMkheCollectivePublicKeyShareV1,
    ) -> Result<
        (
            VerifiedPersistentWitnessBindingV1,
            VerifiedPersistentWitnessBindingV1,
        ),
        ZkAmsMkheErrorV1,
    > {
        let commitments = self.validated_cpk_secret_commitments_v1(roster, share)?;
        let binding = mint_test_state_owned_collective_secret_binding_v1(
            roster,
            self.security_certificate_digest_internal(),
            self.transcript_digest(),
            usize::from(self.party_index()),
            self.public_share_digest(),
            commitments,
        )?;
        Ok(binding.fork_for_state_and_verifier_v1())
    }
    #[cfg(test)]
    #[expect(
        dead_code,
        reason = "native state-owned CPK admission fixture retained for reference tests"
    )]
    pub(super) fn admit_test_state_owned_cpk_binding(
        &mut self,
        roster: &ZkAmsMkheGovernedActiveRosterV1,
        share: &ZkAmsMkheCollectivePublicKeyShareV1,
    ) -> Result<VerifiedPersistentWitnessBindingV1, ZkAmsMkheErrorV1> {
        let (state_binding, verifier_binding) =
            self.test_state_owned_cpk_bindings_v1(roster, share)?;
        self.admit_verified_cpk_binding(roster, share, state_binding)?;
        Ok(verifier_binding)
    }
}
/// Private, non-codec proof that this exact share and its complete active-proof
/// evidence passed the native verifier before leaving the generator.
///
/// This capability is intentionally neither publicly constructible nor
/// cloneable in production. The complete CPK verifier later supplies the
/// independent move-only relation authority; this admission prevents callers
/// from pairing that authority with substituted legacy proof bytes when the
/// native-equivalent share digest is derived by the compact path.
#[cfg_attr(test, derive(Clone))]
#[derive(Debug, PartialEq, Eq)]
struct VerifiedCollectivePublicKeyShareActiveAdmissionV1 {
    _seal: VerifiedCollectivePublicKeyShareActiveAdmissionSealV1,
    evidence_digest: [u8; 32],
}
#[cfg_attr(test, derive(Clone))]
#[derive(Debug, PartialEq, Eq)]
struct VerifiedCollectivePublicKeyShareActiveAdmissionSealV1;
/// Compact move-only proof that one complete generated share was admitted and
/// consumed by the ordered staging ceremony.
///
/// The raw share digest is deliberately insufficient: this capability also
/// binds the private native-proof admission while the proof bytes still exist,
/// plus the exact native/wire polynomial digests needed by later bounded
/// consumers. It has no decoder, public constructor, or production `Clone`.
struct VerifiedCollectivePublicKeyShareStagedAdmissionSealV1;
pub(super) struct VerifiedCollectivePublicKeyShareStagedAdmissionV1 {
    _seal: VerifiedCollectivePublicKeyShareStagedAdmissionSealV1,
    profile_digest: [u8; 32],
    security_certificate_digest: [u8; 32],
    roster_digest: [u8; 32],
    key_material_digest: [u8; 32],
    epoch: u64,
    transcript_digest: [u8; 32],
    party_index: u8,
    party: ZkAmsMkhePartyIdV1,
    share_digest: [u8; 32],
    active_evidence_digest: [u8; 32],
    public_a_native_digest: [u8; 32],
    public_a_wire_digest: [u8; 32],
    party_public_b_native_digest: [u8; 32],
    party_public_b_wire_digest: [u8; 32],
    admission_digest: [u8; 32],
}
impl core::fmt::Debug for VerifiedCollectivePublicKeyShareStagedAdmissionV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("VerifiedCollectivePublicKeyShareStagedAdmissionV1")
            .field("party_index", &self.party_index)
            .field("share_digest", &hex::encode(self.share_digest))
            .field("admission_digest", &hex::encode(self.admission_digest))
            .finish_non_exhaustive()
    }
}
impl VerifiedCollectivePublicKeyShareStagedAdmissionV1 {
    pub(super) const fn share_digest(&self) -> [u8; 32] {
        self.share_digest
    }
    pub(super) const fn public_a_digests(&self) -> ([u8; 32], [u8; 32]) {
        (self.public_a_native_digest, self.public_a_wire_digest)
    }
    pub(super) const fn party_public_b_digests(&self) -> ([u8; 32], [u8; 32]) {
        (
            self.party_public_b_native_digest,
            self.party_public_b_wire_digest,
        )
    }
    pub(super) const fn admission_digest(&self) -> [u8; 32] {
        self.admission_digest
    }
    pub(super) fn validate_for_v1(
        &self,
        roster: &ZkAmsMkheGovernedActiveRosterV1,
        transcript_digest: [u8; 32],
        party_index: usize,
    ) -> Result<(), ZkAmsMkheErrorV1> {
        let expected_party = roster
            .participants()
            .get(party_index)
            .ok_or(ZkAmsMkheErrorV1::InvalidPartySet)?
            .party();
        if self.profile_digest != roster.profile_digest()
            || self.security_certificate_digest != release_security_certificate_digest()?
            || self.roster_digest != roster.roster_digest()
            || self.key_material_digest != roster.key_material_digest()
            || self.epoch != roster.epoch()
            || self.transcript_digest != transcript_digest
            || usize::from(self.party_index) != party_index
            || self.party != expected_party
            || self.share_digest == [0; 32]
            || self.active_evidence_digest == [0; 32]
            || self.public_a_native_digest == [0; 32]
            || self.public_a_wire_digest == [0; 32]
            || self.party_public_b_native_digest == [0; 32]
            || self.party_public_b_wire_digest == [0; 32]
            || self.admission_digest == [0; 32]
            || self.admission_digest != collective_public_key_share_staged_admission_digest_v1(self)
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        Ok(())
    }
}
struct StagedCollectivePublicKeyAdmissionSealV1;
/// One-shot admission for a final key built from the sealed staged batch.
///
/// Runtime and ceremony consumers must consume this private capability beside
/// the materialized key; accepting the key digest alone would discard the
/// ordered share/proof lineage established before the large owners were freed.
pub(super) struct ZkAmsMkheStagedCollectivePublicKeyAdmissionV1 {
    _seal: StagedCollectivePublicKeyAdmissionSealV1,
    profile_digest: [u8; 32],
    roster_digest: [u8; 32],
    key_material_digest: [u8; 32],
    epoch: u64,
    transcript_digest: [u8; 32],
    collective_public_key_digest: [u8; 32],
    share_digests: [[u8; 32]; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
    admission_digest: [u8; 32],
}
impl core::fmt::Debug for ZkAmsMkheStagedCollectivePublicKeyAdmissionV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("ZkAmsMkheStagedCollectivePublicKeyAdmissionV1")
            .field(
                "collective_public_key_digest",
                &hex::encode(self.collective_public_key_digest),
            )
            .field("admission_digest", &hex::encode(self.admission_digest))
            .finish_non_exhaustive()
    }
}
/// One proof-carrying share of the exact eight-party collective public key.
///
/// The share is move-only in production because it contains the private active
/// admission capability used by bounded complete-CPK consumers. Unit tests may
/// clone malformed fixtures solely to exercise hostile validation paths.
#[cfg_attr(test, derive(Clone))]
#[derive(Debug, PartialEq, Eq)]
pub struct ZkAmsMkheCollectivePublicKeyShareV1 {
    version: u8,
    profile_digest: [u8; 32],
    security_certificate_digest: [u8; 32],
    roster_digest: [u8; 32],
    key_material_digest: [u8; 32],
    epoch: u64,
    transcript_digest: [u8; 32],
    party_index: u8,
    party: ZkAmsMkhePartyIdV1,
    public_a: ZkAmsMkheRnsPolynomialWireV1,
    party_public_b: ZkAmsMkheRnsPolynomialWireV1,
    proof: ZkAmsMkheActiveRkgProofV1,
    digest: [u8; 32],
    active_admission: Option<VerifiedCollectivePublicKeyShareActiveAdmissionV1>,
}
impl ZkAmsMkheCollectivePublicKeyShareV1 {
    /// Exact governed contributor.
    #[must_use]
    pub const fn party(&self) -> ZkAmsMkhePartyIdV1 {
        self.party
    }
    /// Exact governed roster position.
    #[must_use]
    pub const fn party_index(&self) -> u8 {
        self.party_index
    }
    /// Governed secret/key epoch bound to this share.
    #[must_use]
    pub const fn epoch(&self) -> u64 {
        self.epoch
    }
    /// Exact collective-key ceremony transcript bound to this share.
    #[must_use]
    pub const fn transcript_digest(&self) -> [u8; 32] {
        self.transcript_digest
    }
    /// Common deterministic public `a` polynomial.
    #[must_use]
    pub const fn public_a(&self) -> &ZkAmsMkheRnsPolynomialWireV1 {
        &self.public_a
    }
    /// This party's `b_i = -a*s_i + t*e_i` contribution.
    #[must_use]
    pub const fn party_public_b(&self) -> &ZkAmsMkheRnsPolynomialWireV1 {
        &self.party_public_b
    }
    /// Native bounded-relation and authentication proof.
    #[must_use]
    pub const fn proof(&self) -> &ZkAmsMkheActiveRkgProofV1 {
        &self.proof
    }
    /// Consensus digest of the complete share and proof.
    #[must_use]
    pub const fn digest(&self) -> [u8; 32] {
        self.digest
    }
    /// Build a structurally valid hostile fixture whose proof/authentication
    /// bytes came from another admitted share. The ordinary share digest is
    /// deliberately recomputed while the private admission is left untouched,
    /// so bounded consumers must reject specifically at that seal.
    #[cfg(test)]
    pub(super) fn splice_active_proof_for_test(
        &mut self,
        other: &Self,
    ) -> Result<(), ZkAmsMkheErrorV1> {
        self.proof = other.proof.clone();
        self.digest = collective_public_key_share_digest(self)?;
        Ok(())
    }
}
/// Verified aggregate of all eight collective-public-key shares.
pub struct ZkAmsMkheCollectivePublicKeyV1 {
    version: u8,
    profile_digest: [u8; 32],
    security_certificate_digest: [u8; 32],
    roster_digest: [u8; 32],
    key_material_digest: [u8; 32],
    epoch: u64,
    transcript_digest: [u8; 32],
    parties: super::PartySet,
    public_a: RnsPolynomial,
    collective_public_b: RnsPolynomial,
    share_digests: [[u8; 32]; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
    digest: [u8; 32],
}
impl core::fmt::Debug for ZkAmsMkheCollectivePublicKeyV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("ZkAmsMkheCollectivePublicKeyV1")
            .field("profile_digest", &hex::encode(self.profile_digest))
            .field("roster_digest", &hex::encode(self.roster_digest))
            .field("epoch", &self.epoch)
            .field("transcript_digest", &hex::encode(self.transcript_digest))
            .field("share_digests", &self.share_digests.map(hex::encode))
            .field("digest", &hex::encode(self.digest))
            .finish_non_exhaustive()
    }
}
impl ZkAmsMkheCollectivePublicKeyV1 {
    /// Frozen release profile digest.
    #[must_use]
    pub const fn profile_digest(&self) -> [u8; 32] {
        self.profile_digest
    }
    /// Frozen estimator certificate digest.
    #[cfg(test)]
    #[must_use]
    pub const fn security_certificate_digest(&self) -> [u8; 32] {
        self.security_certificate_digest
    }
    /// Exact ordered governed roster digest.
    #[must_use]
    pub const fn roster_digest(&self) -> [u8; 32] {
        self.roster_digest
    }
    /// Exact collective-key ceremony transcript.
    #[must_use]
    pub const fn transcript_digest(&self) -> [u8; 32] {
        self.transcript_digest
    }
    /// Governed secret/key epoch.
    #[must_use]
    pub const fn epoch(&self) -> u64 {
        self.epoch
    }
    /// Consensus identity of the aggregate public key and all eight shares.
    #[must_use]
    pub const fn digest(&self) -> [u8; 32] {
        self.digest
    }
    /// Canonical release wire form of common `a`.
    #[cfg(test)]
    pub fn public_a_wire(&self) -> Result<ZkAmsMkheRnsPolynomialWireV1, ZkAmsMkheErrorV1> {
        self.validate(&release_profile_v1())?;
        ZkAmsMkheRnsPolynomialWireV1::new(self.public_a.coefficients.clone())
    }
    /// Canonical release wire form of aggregate `b = sum_i b_i`.
    #[cfg(test)]
    pub fn collective_public_b_wire(
        &self,
    ) -> Result<ZkAmsMkheRnsPolynomialWireV1, ZkAmsMkheErrorV1> {
        self.validate(&release_profile_v1())?;
        ZkAmsMkheRnsPolynomialWireV1::new(self.collective_public_b.coefficients.clone())
    }
    #[cfg(test)]
    pub(super) const fn parties(&self) -> &super::PartySet {
        &self.parties
    }
    pub(super) const fn key_material_digest_internal(&self) -> [u8; 32] {
        self.key_material_digest
    }
    pub(super) const fn share_digests_internal(
        &self,
    ) -> &[[u8; 32]; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1] {
        &self.share_digests
    }
    pub(super) fn validate(&self, profile: &BgvProfile) -> Result<(), ZkAmsMkheErrorV1> {
        if self.version != MKHE_VERSION_V1
            || self.profile_digest != profile.digest()?
            || self.security_certificate_digest == [0; 32]
            || self.roster_digest
                != governed_roster_digest(self.profile_digest, self.epoch, &self.parties.parties)
            || self.key_material_digest == [0; 32]
            || self.epoch == 0
            || self.transcript_digest == [0; 32]
            || self.parties.parties.len() != ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1
            || self.share_digests.contains(&[0; 32])
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        self.public_a.validate(profile)?;
        self.collective_public_b.validate(profile)?;
        if self.public_a.is_zero()
            || self.collective_public_b.is_zero()
            || self.digest == [0; 32]
            || self.digest != collective_public_key_digest(self, profile)?
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        Ok(())
    }
}
/// Generate one party state from the ceremony-owned prepared common `a`.
///
/// Production code obtains `prepared` by borrowing the live move-only CPK
/// ceremony. Each returned share shares that one backing and is immediately
/// consumed by the ceremony's next-party transition.
pub fn generate_zk_ams_mkhe_collective_party_state_with_prepared_public_a_v1<
    R: MaskedRelaxedRandomSourceV1,
>(
    roster: &ZkAmsMkheGovernedActiveRosterV1,
    prepared: &ZkAmsMkhePreparedCollectivePublicAV1,
    party_index: usize,
    party_secret: &ZkAmsMkheActivePartySecretV1,
    random: &mut R,
) -> Result<
    (
        ZkAmsMkheCollectivePartyStateV1,
        ZkAmsMkheCollectivePublicKeyShareV1,
    ),
    ZkAmsMkheErrorV1,
> {
    // Complete all attacker-controlled scalar checks before allocating any
    // ring-sized secret or polynomial storage.
    roster.validate()?;
    prepared.validate_for(roster)?;
    if party_index >= ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1
        || roster.participants()[party_index].party() != party_secret.party()?
    {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    let transcript_digest = prepared.transcript_digest();
    let profile = release_profile_v1();
    profile.validate()?;
    let security_certificate_digest = release_security_certificate_digest()?;
    let public_a = prepared.shared_public_a();
    public_a.encoded_len()?;
    let secret = sample_nonzero_ternary(&profile, random)?;
    if secret.coefficients.capacity() != profile.ring_degree {
        return Err(ZkAmsMkheErrorV1::ResourceCeilingExceeded);
    }
    let public_error = SecretPolynomial::sample_error(&profile, random)?;
    if public_error.coefficients.capacity() != profile.ring_degree {
        return Err(ZkAmsMkheErrorV1::ResourceCeilingExceeded);
    }
    let mut party_public_b_product =
        borrowed_product::multiply_public_residues_by_secret_signed_v1(
            public_a.residues(),
            &secret.coefficients,
            &profile,
        )?;
    negate_and_add_scaled_error_in_place(&mut party_public_b_product.0, &public_error, &profile)?;
    let party_public_b_native = party_public_b_product.into_public();
    let party_public_b =
        ZkAmsMkheRnsPolynomialWireV1::new_exact_capacity_v1(party_public_b_native.coefficients)?;
    let statement = ZkAmsMkheActiveCollectivePublicKeyStatementV1::new(&public_a, &party_public_b)?;
    let witness = ZkAmsMkheActiveCollectivePublicKeyWitnessV1::new(
        &secret.coefficients,
        &public_error.coefficients,
    )?;
    let proof = prove_zk_ams_mkhe_active_collective_public_key_v1(
        roster,
        transcript_digest,
        party_index,
        statement,
        witness,
        party_secret,
        random,
    )?;
    let mut share = ZkAmsMkheCollectivePublicKeyShareV1 {
        version: MKHE_VERSION_V1,
        profile_digest: roster.profile_digest(),
        security_certificate_digest,
        roster_digest: roster.roster_digest(),
        key_material_digest: roster.key_material_digest(),
        epoch: roster.epoch(),
        transcript_digest,
        party_index: u8::try_from(party_index).map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?,
        party: party_secret.party()?,
        public_a,
        party_public_b,
        proof,
        digest: [0; 32],
        active_admission: None,
    };
    share.digest = collective_public_key_share_digest(&share)?;
    validate_collective_public_key_share_unsealed_v1(
        roster,
        transcript_digest,
        party_index,
        &share,
    )?;
    share.active_admission = Some(mint_collective_public_key_share_active_admission_v1(
        &share,
    )?);
    validate_collective_public_key_share_active_admission_v1(&share)?;
    let persistent_secret_commitment_blindings = ZeroizingCpkMembershipBlindingsV1::sample(random)?;
    let persistent_direct_opening = PersistentDirectOpeningOwnerV1::new_unverified(
        PersistentDirectOpeningAxesV1 {
            profile_digest: share.profile_digest,
            security_certificate_digest,
            roster_digest: share.roster_digest,
            key_material_digest: share.key_material_digest,
            epoch: share.epoch,
            cpk_transcript_digest: transcript_digest,
            party_index: share.party_index,
            party: share.party,
            public_share_digest: share.digest,
        },
        secret,
        persistent_secret_commitment_blindings,
    )?;
    let state = ZkAmsMkheCollectivePartyStateV1 {
        persistent_direct_opening,
        public_error,
        party_local_rkg_ephemeral_opening: None,
        party_local_rkg_ephemeral_creation_mask: 0,
    };
    Ok((state, share))
}
/// Test-only native reference aggregation of all eight ordered shares.
///
/// Missing, duplicate, reordered, cross-roster, cross-epoch, cross-transcript,
/// and proof-spliced shares are rejected before the aggregate key is returned.
/// Production construction must consume parties through the staged CPK
/// ceremony so eight `P`-sized party-`b_i` owners cannot coexist.
#[cfg(test)]
pub(super) fn aggregate_zk_ams_mkhe_collective_public_key_v1(
    roster: &ZkAmsMkheGovernedActiveRosterV1,
    transcript_digest: [u8; 32],
    shares: [&ZkAmsMkheCollectivePublicKeyShareV1; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
) -> Result<ZkAmsMkheCollectivePublicKeyV1, ZkAmsMkheErrorV1> {
    roster.validate()?;
    if transcript_digest == [0; 32] {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    let profile = release_profile_v1();
    profile.validate()?;
    // Verification is deliberately complete before allocating the aggregate
    // output polynomial, so malformed proof sets cannot trigger that work.
    for (party_index, share) in shares.iter().enumerate() {
        validate_collective_public_key_share(roster, transcript_digest, party_index, share)?;
    }
    // Every share has already passed the full native verifier above. Reuse
    // the first immutable common owner instead of deriving another complete
    // release polynomial solely for this equality pass.
    let expected_public_a = shares[0].public_a.clone();
    if shares
        .iter()
        .any(|share| share.public_a != expected_public_a)
    {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    checked_coefficient_work(&profile, ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1)?;
    let mut aggregate_b = ZeroizingRns::zero_exact_v1(&profile)?;
    for share in shares {
        add_canonical_residues_in_place_v1(
            aggregate_b.coefficients_mut(),
            share.party_public_b.residues(),
            &profile,
        )?;
    }
    let parties = super::PartySet::new(
        roster
            .participants()
            .iter()
            .map(|participant| participant.party())
            .collect(),
    )?;
    let mut key = ZkAmsMkheCollectivePublicKeyV1 {
        version: MKHE_VERSION_V1,
        profile_digest: roster.profile_digest(),
        security_certificate_digest: release_security_certificate_digest()?,
        roster_digest: roster.roster_digest(),
        key_material_digest: roster.key_material_digest(),
        epoch: roster.epoch(),
        transcript_digest,
        parties,
        public_a: RnsPolynomial::from_flat(&profile, expected_public_a.residues().to_vec())?,
        collective_public_b: aggregate_b.into_public(),
        share_digests: shares.map(ZkAmsMkheCollectivePublicKeyShareV1::digest),
        digest: [0; 32],
    };
    key.digest = collective_public_key_digest(&key, &profile)?;
    key.validate(&profile)?;
    Ok(key)
}
#[cfg(test)]
fn add_canonical_residues_in_place_v1(
    aggregate: &mut [u64],
    addend: &[u64],
    profile: &BgvProfile,
) -> Result<(), ZkAmsMkheErrorV1> {
    profile.validate()?;
    let coefficient_count = profile
        .ring_degree
        .checked_mul(profile.moduli.len())
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    if aggregate.len() != coefficient_count || addend.len() != coefficient_count {
        return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
    }
    // Validate the complete input before mutating the accumulator. This keeps
    // the operation retry-safe if a future provider supplies a malformed late
    // limb, without allocating a rollback copy of the release polynomial.
    for (limb, (aggregate_limb, addend_limb)) in aggregate
        .chunks_exact(profile.ring_degree)
        .zip(addend.chunks_exact(profile.ring_degree))
        .enumerate()
    {
        let modulus = profile.moduli[limb];
        if aggregate_limb.iter().any(|value| *value >= modulus)
            || addend_limb.iter().any(|value| *value >= modulus)
        {
            return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
        }
    }
    for (limb, (aggregate_limb, addend_limb)) in aggregate
        .chunks_exact_mut(profile.ring_degree)
        .zip(addend.chunks_exact(profile.ring_degree))
        .enumerate()
    {
        let modulus = profile.moduli[limb];
        for (aggregate, addend) in aggregate_limb.iter_mut().zip(addend_limb) {
            *aggregate = super::mod_add(*aggregate, *addend, modulus);
        }
    }
    Ok(())
}
fn staged_collective_public_key_admission_digest_v1(
    admission: &ZkAmsMkheStagedCollectivePublicKeyAdmissionV1,
) -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(COLLECTIVE_PUBLIC_KEY_STAGED_ADMISSION_DOMAIN_V1);
    hash.update(&[MKHE_VERSION_V1]);
    hash.update(&admission.profile_digest);
    hash.update(&admission.roster_digest);
    hash.update(&admission.key_material_digest);
    hash.update(&admission.epoch.to_be_bytes());
    hash.update(&admission.transcript_digest);
    hash.update(&admission.collective_public_key_digest);
    for digest in admission.share_digests {
        hash.update(&digest);
    }
    hash.finalize()
}
impl ZkAmsMkheStagedCollectivePublicKeyAdmissionV1 {
    fn validate_for_key_v1(
        &self,
        roster: &ZkAmsMkheGovernedActiveRosterV1,
        transcript_digest: [u8; 32],
        key: &ZkAmsMkheCollectivePublicKeyV1,
    ) -> Result<(), ZkAmsMkheErrorV1> {
        roster.validate()?;
        key.validate(&release_profile_v1())?;
        if self.profile_digest != roster.profile_digest()
            || self.roster_digest != roster.roster_digest()
            || self.key_material_digest != roster.key_material_digest()
            || self.epoch != roster.epoch()
            || self.transcript_digest != transcript_digest
            || self.collective_public_key_digest != key.digest()
            || self.share_digests != *key.share_digests_internal()
            || key.profile_digest() != roster.profile_digest()
            || key.roster_digest() != roster.roster_digest()
            || key.key_material_digest_internal() != roster.key_material_digest()
            || key.epoch() != roster.epoch()
            || key.transcript_digest() != transcript_digest
            || self.admission_digest == [0; 32]
            || self.admission_digest != staged_collective_public_key_admission_digest_v1(self)
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        Ok(())
    }
    /// Consume this one-shot admission beside its exact materialized key.
    pub(super) fn consume_for_key_v1(
        self,
        roster: &ZkAmsMkheGovernedActiveRosterV1,
        transcript_digest: [u8; 32],
        key: &ZkAmsMkheCollectivePublicKeyV1,
    ) -> Result<(), ZkAmsMkheErrorV1> {
        self.validate_for_key_v1(roster, transcript_digest, key)
    }
}
struct ZeroizingStagedCollectivePublicKeyConstructionV1 {
    key: Option<ZkAmsMkheCollectivePublicKeyV1>,
}
impl ZeroizingStagedCollectivePublicKeyConstructionV1 {
    fn into_public(mut self) -> ZkAmsMkheCollectivePublicKeyV1 {
        self.key
            .take()
            .expect("validated staged collective key must remain present")
    }
}
impl Drop for ZeroizingStagedCollectivePublicKeyConstructionV1 {
    fn drop(&mut self) {
        if let Some(key) = self.key.as_mut() {
            clear_secret_u64_slice_v1(&mut key.public_a.coefficients);
            clear_secret_u64_slice_v1(&mut key.collective_public_b.coefficients);
        }
    }
}
/// Construct the final key from the exact sealed stage without cloning either
/// release polynomial.
///
/// During direct aggregation the only ring-sized owners are common `a` and the
/// zeroizing aggregate. The final key takes both allocations by move after all
/// eight compact admissions and streamed per-party digests match. Any error or
/// unwind before the move-to-public boundary clears both buffers.
#[allow(clippy::too_many_arguments)]
pub(super) fn finalize_collective_public_key_from_staged_v1(
    roster: &ZkAmsMkheGovernedActiveRosterV1,
    transcript_digest: [u8; 32],
    public_a: ZeroizingRns,
    aggregate_b: ZeroizingRns,
    admissions: [VerifiedCollectivePublicKeyShareStagedAdmissionV1;
        ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
    observed_party_b_native_digests: [[u8; 32]; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
    observed_party_b_wire_digests: [[u8; 32]; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
) -> Result<
    (
        ZkAmsMkheCollectivePublicKeyV1,
        ZkAmsMkheStagedCollectivePublicKeyAdmissionV1,
    ),
    ZkAmsMkheErrorV1,
> {
    roster.validate()?;
    let profile = release_profile_v1();
    profile.validate()?;
    if transcript_digest == [0; 32]
        || observed_party_b_native_digests.contains(&[0; 32])
        || observed_party_b_wire_digests.contains(&[0; 32])
    {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    public_a.0.validate(&profile)?;
    aggregate_b.0.validate(&profile)?;
    if public_a.0.is_zero() || aggregate_b.0.is_zero() {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    let public_a_digests = cks_staged_residue_digests_v1(&profile, public_a.coefficients())?;
    let share_digests = core::array::from_fn(|party_index| admissions[party_index].share_digest());
    for party_index in 0..ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1 {
        let admission = &admissions[party_index];
        admission.validate_for_v1(roster, transcript_digest, party_index)?;
        if admission.public_a_digests() != public_a_digests
            || admission.party_public_b_digests()
                != (
                    observed_party_b_native_digests[party_index],
                    observed_party_b_wire_digests[party_index],
                )
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
    }
    let prepared_public_a =
        prepare_active_collective_public_a_v1(&profile, roster, transcript_digest)
            .map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?;
    let mut remaining_public_a_candidates = prepared_public_a
        .candidate_budget_for_limbs_v1(profile.moduli.len())
        .map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?;
    for limb in 0..profile.moduli.len() {
        let expected = prepared_public_a
            .derive_limb_budgeted_v1(limb, &mut remaining_public_a_candidates)
            .map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?;
        if public_a.0.limb(&profile, limb) != expected.as_slice() {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
    }
    checked_coefficient_work(&profile, ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1)?;
    let parties = super::PartySet::new(
        roster
            .participants()
            .iter()
            .map(|participant| participant.party())
            .collect(),
    )?;
    let mut construction = ZeroizingStagedCollectivePublicKeyConstructionV1 {
        key: Some(ZkAmsMkheCollectivePublicKeyV1 {
            version: MKHE_VERSION_V1,
            profile_digest: roster.profile_digest(),
            security_certificate_digest: release_security_certificate_digest()?,
            roster_digest: roster.roster_digest(),
            key_material_digest: roster.key_material_digest(),
            epoch: roster.epoch(),
            transcript_digest,
            parties,
            public_a: public_a.into_public(),
            collective_public_b: aggregate_b.into_public(),
            share_digests,
            digest: [0; 32],
        }),
    };
    let key = construction
        .key
        .as_mut()
        .ok_or(ZkAmsMkheErrorV1::InvalidKeyMaterial)?;
    key.digest = collective_public_key_digest(key, &profile)?;
    key.validate(&profile)?;
    let mut admission = ZkAmsMkheStagedCollectivePublicKeyAdmissionV1 {
        _seal: StagedCollectivePublicKeyAdmissionSealV1,
        profile_digest: roster.profile_digest(),
        roster_digest: roster.roster_digest(),
        key_material_digest: roster.key_material_digest(),
        epoch: roster.epoch(),
        transcript_digest,
        collective_public_key_digest: key.digest(),
        share_digests,
        admission_digest: [0; 32],
    };
    admission.admission_digest = staged_collective_public_key_admission_digest_v1(&admission);
    admission.validate_for_key_v1(roster, transcript_digest, key)?;
    Ok((construction.into_public(), admission))
}
fn validate_collective_public_key_share(
    roster: &ZkAmsMkheGovernedActiveRosterV1,
    transcript_digest: [u8; 32],
    party_index: usize,
    share: &ZkAmsMkheCollectivePublicKeyShareV1,
) -> Result<(), ZkAmsMkheErrorV1> {
    validate_collective_public_key_share_unsealed_v1(
        roster,
        transcript_digest,
        party_index,
        share,
    )?;
    validate_collective_public_key_share_active_admission_v1(share)
}
/// Replay the complete native active proof. This is the only minting gate for
/// [`VerifiedCollectivePublicKeyShareActiveAdmissionV1`].
fn validate_collective_public_key_share_unsealed_v1(
    roster: &ZkAmsMkheGovernedActiveRosterV1,
    transcript_digest: [u8; 32],
    party_index: usize,
    share: &ZkAmsMkheCollectivePublicKeyShareV1,
) -> Result<(), ZkAmsMkheErrorV1> {
    roster.validate()?;
    let security_certificate_digest = release_security_certificate_digest()?;
    let expected_party = roster
        .participants()
        .get(party_index)
        .ok_or(ZkAmsMkheErrorV1::InvalidKeyMaterial)?
        .party();
    if share.version != MKHE_VERSION_V1
        || share.profile_digest != roster.profile_digest()
        || share.security_certificate_digest != security_certificate_digest
        || share.roster_digest != roster.roster_digest()
        || share.key_material_digest != roster.key_material_digest()
        || share.epoch != roster.epoch()
        || share.transcript_digest != transcript_digest
        || usize::from(share.party_index) != party_index
        || share.party != expected_party
        || share.digest == [0; 32]
        || share.digest != collective_public_key_share_digest(share)?
    {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    // The native verifier below validates deterministic common `a` one limb
    // at a time. Do not derive a redundant full release polynomial here.
    let statement =
        ZkAmsMkheActiveCollectivePublicKeyStatementV1::new(&share.public_a, &share.party_public_b)?;
    verify_zk_ams_mkhe_active_collective_public_key_v1(
        roster,
        transcript_digest,
        party_index,
        statement,
        &share.proof,
    )
}
fn release_security_certificate_digest() -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    let certificate = zk_ams_mkhe_security_certificate_v1()?;
    let digest = certificate.certificate_digest();
    if digest == [0; 32] {
        return Err(ZkAmsMkheErrorV1::InvalidProfile);
    }
    Ok(digest)
}
fn collective_public_key_share_digest(
    share: &ZkAmsMkheCollectivePublicKeyShareV1,
) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    share.public_a.encoded_len()?;
    share.party_public_b.encoded_len()?;
    let mut hash = Keccak256::new();
    hash.update(COLLECTIVE_PARTY_SHARE_DOMAIN_V1);
    hash.update(&[share.version]);
    hash.update(&share.profile_digest);
    hash.update(&share.security_certificate_digest);
    hash.update(&share.roster_digest);
    hash.update(&share.key_material_digest);
    hash.update(&share.epoch.to_be_bytes());
    hash.update(&share.transcript_digest);
    hash.update(&[share.party_index]);
    hash.update(&share.party.to_bytes());
    update_wire_polynomial_hash(&mut hash, &share.public_a)?;
    update_wire_polynomial_hash(&mut hash, &share.party_public_b)?;
    hash.update(&share.proof.statement_digest());
    hash.update(&[share.proof.witness_polynomials()]);
    hash.update(&share.proof.contribution().digest()?);
    Ok(hash.finalize())
}
/// Digest every byte whose native admission is represented by the private
/// capability. The ordinary share digest binds the complete public statement;
/// the evidence stream additionally binds the raw proof and authentication
/// bytes which are deliberately not all present in that legacy digest.
fn collective_public_key_share_active_admission_digest_v1(
    share: &ZkAmsMkheCollectivePublicKeyShareV1,
) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    let recomputed_share_digest = collective_public_key_share_digest(share)?;
    if share.digest == [0; 32] || share.digest != recomputed_share_digest {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    let mut hash = Keccak256::new();
    hash.update(COLLECTIVE_PARTY_SHARE_ACTIVE_ADMISSION_DOMAIN_V1);
    hash.update(&share.digest);
    share.proof.write_evidence_chunks(|chunk| {
        hash.update(chunk);
        Ok(())
    })?;
    let digest = hash.finalize();
    if digest == [0; 32] {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    Ok(digest)
}
fn mint_collective_public_key_share_active_admission_v1(
    share: &ZkAmsMkheCollectivePublicKeyShareV1,
) -> Result<VerifiedCollectivePublicKeyShareActiveAdmissionV1, ZkAmsMkheErrorV1> {
    Ok(VerifiedCollectivePublicKeyShareActiveAdmissionV1 {
        _seal: VerifiedCollectivePublicKeyShareActiveAdmissionSealV1,
        evidence_digest: collective_public_key_share_active_admission_digest_v1(share)?,
    })
}
fn validate_collective_public_key_share_active_admission_v1(
    share: &ZkAmsMkheCollectivePublicKeyShareV1,
) -> Result<(), ZkAmsMkheErrorV1> {
    let admission = share
        .active_admission
        .as_ref()
        .ok_or(ZkAmsMkheErrorV1::InvalidKeyMaterial)?;
    if admission.evidence_digest != collective_public_key_share_active_admission_digest_v1(share)? {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    Ok(())
}
pub(super) fn cks_staged_residue_digests_v1(
    profile: &BgvProfile,
    residues: &[u64],
) -> Result<([u8; 32], [u8; 32]), ZkAmsMkheErrorV1> {
    profile.validate()?;
    let coefficient_count = profile
        .ring_degree
        .checked_mul(profile.moduli.len())
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    if residues.len() != coefficient_count {
        return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
    }
    for (limb, values) in residues.chunks_exact(profile.ring_degree).enumerate() {
        if values
            .iter()
            .any(|residue| *residue >= profile.moduli[limb])
        {
            return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
        }
    }
    let count =
        u32::try_from(coefficient_count).map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let mut native = Keccak256::new();
    native.update(CKS_RNS_NATIVE_DIGEST_DOMAIN_V1);
    native.update(&count.to_be_bytes());
    let mut wire = Keccak256::new();
    wire.update(CKS_RNS_WIRE_DIGEST_DOMAIN_V1);
    wire.update(&count.to_be_bytes());
    for residue in residues {
        let encoded = residue.to_be_bytes();
        native.update(&encoded);
        wire.update(&encoded);
    }
    let digests = (native.finalize(), wire.finalize());
    if digests.0 == [0; 32] || digests.1 == [0; 32] {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    Ok(digests)
}
fn collective_public_key_share_staged_admission_digest_v1(
    admission: &VerifiedCollectivePublicKeyShareStagedAdmissionV1,
) -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(COLLECTIVE_PARTY_SHARE_STAGED_ADMISSION_DOMAIN_V1);
    hash.update(&[MKHE_VERSION_V1]);
    hash.update(&admission.profile_digest);
    hash.update(&admission.security_certificate_digest);
    hash.update(&admission.roster_digest);
    hash.update(&admission.key_material_digest);
    hash.update(&admission.epoch.to_be_bytes());
    hash.update(&admission.transcript_digest);
    hash.update(&[admission.party_index]);
    hash.update(&admission.party.to_bytes());
    hash.update(&admission.share_digest);
    hash.update(&admission.active_evidence_digest);
    hash.update(&admission.public_a_native_digest);
    hash.update(&admission.public_a_wire_digest);
    hash.update(&admission.party_public_b_native_digest);
    hash.update(&admission.party_public_b_wire_digest);
    hash.finalize()
}
/// Consume one complete share into the compact capability retained by staging.
///
/// Callers must perform any required CAS publication before this transition:
/// after it returns neither the `P`-sized party `b_i` nor the proof bytes exist.
pub(super) fn consume_collective_public_key_share_for_staging_v1(
    roster: &ZkAmsMkheGovernedActiveRosterV1,
    transcript_digest: [u8; 32],
    party_index: usize,
    mut share: ZkAmsMkheCollectivePublicKeyShareV1,
) -> Result<VerifiedCollectivePublicKeyShareStagedAdmissionV1, ZkAmsMkheErrorV1> {
    let share_digest = validate_collective_public_key_share_for_verified_cpk_compact_v1(
        roster,
        transcript_digest,
        party_index,
        &share,
    )?;
    let profile = release_profile_v1();
    let (public_a_native_digest, public_a_wire_digest) =
        cks_staged_residue_digests_v1(&profile, share.public_a.residues())?;
    let (party_public_b_native_digest, party_public_b_wire_digest) =
        cks_staged_residue_digests_v1(&profile, share.party_public_b.residues())?;
    let active_admission = share
        .active_admission
        .take()
        .ok_or(ZkAmsMkheErrorV1::InvalidKeyMaterial)?;
    let mut admission = VerifiedCollectivePublicKeyShareStagedAdmissionV1 {
        _seal: VerifiedCollectivePublicKeyShareStagedAdmissionSealV1,
        profile_digest: share.profile_digest,
        security_certificate_digest: share.security_certificate_digest,
        roster_digest: share.roster_digest,
        key_material_digest: share.key_material_digest,
        epoch: share.epoch,
        transcript_digest: share.transcript_digest,
        party_index: share.party_index,
        party: share.party,
        share_digest,
        active_evidence_digest: active_admission.evidence_digest,
        public_a_native_digest,
        public_a_wire_digest,
        party_public_b_native_digest,
        party_public_b_wire_digest,
        admission_digest: [0; 32],
    };
    admission.admission_digest = collective_public_key_share_staged_admission_digest_v1(&admission);
    admission.validate_for_v1(roster, transcript_digest, party_index)?;
    Ok(admission)
}
/// Validate the compact public data paired with a complete verified CPK receipt.
///
/// The CPK receipt, rather than the legacy active-RKG proof, is the move-only
/// authority for the native `b_i = -a*s_i + t*e_i` relation. This helper still
/// requires the private admission minted only after that exact proof passed the
/// native verifier, and recomputes its digest over all raw proof/authentication
/// evidence. Thus substituted legacy proof bytes cannot alter a supposedly
/// native-equivalent share digest without replaying the allocation-heavy proof
/// verifier inside this bounded path.
#[allow(dead_code)]
pub(super) fn validate_collective_public_key_share_for_verified_cpk_compact_v1(
    roster: &ZkAmsMkheGovernedActiveRosterV1,
    transcript_digest: [u8; 32],
    party_index: usize,
    share: &ZkAmsMkheCollectivePublicKeyShareV1,
) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    roster.validate()?;
    let profile = release_profile_v1();
    profile.validate()?;
    let expected_party = roster
        .participants()
        .get(party_index)
        .ok_or(ZkAmsMkheErrorV1::InvalidPartySet)?
        .party();
    if transcript_digest == [0; 32]
        || share.version != MKHE_VERSION_V1
        || share.profile_digest != roster.profile_digest()
        || share.security_certificate_digest != release_security_certificate_digest()?
        || share.roster_digest != roster.roster_digest()
        || share.key_material_digest != roster.key_material_digest()
        || share.epoch != roster.epoch()
        || share.transcript_digest != transcript_digest
        || usize::from(share.party_index) != party_index
        || share.party != expected_party
    {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    share.public_a.encoded_len()?;
    share.party_public_b.encoded_len()?;
    share.proof.evidence_encoded_len()?;
    if share
        .public_a
        .residues()
        .iter()
        .all(|residue| *residue == 0)
        || share
            .party_public_b
            .residues()
            .iter()
            .all(|residue| *residue == 0)
    {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    let prepared_public_a =
        prepare_active_collective_public_a_v1(&profile, roster, transcript_digest)
            .map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?;
    let mut remaining_public_a_candidates = prepared_public_a
        .candidate_budget_for_limbs_v1(profile.moduli.len())
        .map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?;
    for limb in 0..profile.moduli.len() {
        let start = limb
            .checked_mul(profile.ring_degree)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        let end = start
            .checked_add(profile.ring_degree)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        let expected = prepared_public_a
            .derive_limb_budgeted_v1(limb, &mut remaining_public_a_candidates)
            .map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?;
        if share.public_a.residues().get(start..end) != Some(expected.as_slice()) {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
    }
    let digest = collective_public_key_share_digest(share)?;
    if digest == [0; 32] || share.digest != digest {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    validate_collective_public_key_share_active_admission_v1(share)?;
    Ok(digest)
}
/// Derive the native collective-key digest from one bounded aggregate buffer.
///
/// The output is exactly [`collective_public_key_digest`] for the same ordered
/// shares. Common `a` is derived one limb at a time and no aggregate-key object
/// or second complete RNS polynomial is constructed.
#[allow(dead_code)]
pub(super) fn collective_public_key_digest_from_bounded_cpk_v1(
    roster: &ZkAmsMkheGovernedActiveRosterV1,
    transcript_digest: [u8; 32],
    aggregate_b: &[u64],
    share_digests: [[u8; 32]; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    roster.validate()?;
    let profile = release_profile_v1();
    profile.validate()?;
    let coefficient_count = profile
        .ring_degree
        .checked_mul(profile.moduli.len())
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    if transcript_digest == [0; 32]
        || aggregate_b.len() != coefficient_count
        || aggregate_b.iter().all(|residue| *residue == 0)
        || share_digests.contains(&[0; 32])
    {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    for (limb, residues) in aggregate_b.chunks_exact(profile.ring_degree).enumerate() {
        if residues
            .iter()
            .any(|residue| *residue >= profile.moduli[limb])
        {
            return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
        }
    }
    let mut hash = Keccak256::new();
    hash.update(COLLECTIVE_PUBLIC_KEY_DOMAIN_V1);
    hash.update(&[MKHE_VERSION_V1]);
    hash.update(&roster.profile_digest());
    hash.update(&release_security_certificate_digest()?);
    hash.update(&roster.roster_digest());
    hash.update(&roster.key_material_digest());
    hash.update(&roster.epoch().to_be_bytes());
    hash.update(&transcript_digest);
    for participant in roster.participants() {
        hash.update(&participant.party().to_bytes());
    }
    hash.update(
        &u32::try_from(coefficient_count)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?
            .to_be_bytes(),
    );
    let prepared_public_a =
        prepare_active_collective_public_a_v1(&profile, roster, transcript_digest)
            .map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?;
    let mut remaining_public_a_candidates = prepared_public_a
        .candidate_budget_for_limbs_v1(profile.moduli.len())
        .map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?;
    for limb in 0..profile.moduli.len() {
        let public_a = prepared_public_a
            .derive_limb_budgeted_v1(limb, &mut remaining_public_a_candidates)
            .map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?;
        for residue in public_a {
            hash.update(&residue.to_be_bytes());
        }
    }
    hash.update(
        &u32::try_from(coefficient_count)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?
            .to_be_bytes(),
    );
    for residue in aggregate_b {
        hash.update(&residue.to_be_bytes());
    }
    for share_digest in share_digests {
        hash.update(&share_digest);
    }
    let digest = hash.finalize();
    if digest == [0; 32] {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    Ok(digest)
}
fn collective_public_key_digest(
    key: &ZkAmsMkheCollectivePublicKeyV1,
    profile: &BgvProfile,
) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    key.public_a.validate(profile)?;
    key.collective_public_b.validate(profile)?;
    let mut hash = Keccak256::new();
    hash.update(COLLECTIVE_PUBLIC_KEY_DOMAIN_V1);
    hash.update(&[key.version]);
    hash.update(&key.profile_digest);
    hash.update(&key.security_certificate_digest);
    hash.update(&key.roster_digest);
    hash.update(&key.key_material_digest);
    hash.update(&key.epoch.to_be_bytes());
    hash.update(&key.transcript_digest);
    for party in &key.parties.parties {
        hash.update(&party.to_bytes());
    }
    update_rns_hash(&mut hash, profile, &key.public_a)?;
    update_rns_hash(&mut hash, profile, &key.collective_public_b)?;
    for share_digest in &key.share_digests {
        hash.update(share_digest);
    }
    Ok(hash.finalize())
}
fn update_wire_polynomial_hash(
    hash: &mut Keccak256,
    polynomial: &ZkAmsMkheRnsPolynomialWireV1,
) -> Result<(), ZkAmsMkheErrorV1> {
    polynomial.encoded_len()?;
    hash.update(
        &u32::try_from(polynomial.residues().len())
            .map_err(|_| ZkAmsMkheErrorV1::InvalidPolynomial)?
            .to_be_bytes(),
    );
    for residue in polynomial.residues() {
        hash.update(&residue.to_be_bytes());
    }
    Ok(())
}
/// BLAKE3 content address of the exact direct-object party-`b` framing.
pub(super) fn cpk_party_b_payload_blake3_v1(
    polynomial: &ZkAmsMkheRnsPolynomialWireV1,
) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    polynomial.encoded_len()?;
    let coefficient_count = u32::try_from(polynomial.residues().len())
        .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let payload_bytes = polynomial
        .residues()
        .len()
        .checked_mul(core::mem::size_of::<u64>())
        .and_then(|bytes| bytes.checked_add(core::mem::size_of::<u32>()))
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    if payload_bytes != ZK_AMS_MKHE_CPK_PARTY_B_OBJECT_BYTES_V1 {
        return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
    }
    let mut hash = norito::streaming::Blake3Hasher::new();
    hash.update(&coefficient_count.to_be_bytes());
    for residue in polynomial.residues() {
        hash.update(&residue.to_be_bytes());
    }
    let digest = hash.finalize();
    if digest == [0; 32] {
        return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
    }
    Ok(digest)
}
/// Exact native collective ciphertext containing only `(c_0, c_1)`.
///
/// The transcript digest is the artifact-lineage commitment. Fresh encryption
/// binds public topology plus an opening-owned opaque nonce; evaluation binds
/// its separately documented public operands. The nonce itself is never part
/// of this public object, and callers cannot replace constructor lineage.
#[cfg(test)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ZkAmsMkheCollectiveCiphertextV1 {
    profile_digest: [u8; 32],
    roster_digest: [u8; 32],
    epoch: u64,
    transcript_digest: [u8; 32],
    sample_index: u64,
    level: u8,
    constant: RnsPolynomial,
    linear: RnsPolynomial,
    // Native evaluation capability. It is deliberately absent after decoding
    // an untrusted wire record; decryption remains possible, but homomorphic
    // evaluation requires an exact verified collective key.
    evaluation_key_digest: Option<[u8; 32]>,
    digest: [u8; 32],
}
#[cfg(test)]
impl ZkAmsMkheCollectiveCiphertextV1 {
    /// Decode the exact release wire representation under its governed roster.
    ///
    /// Wire dimensions and every binding axis are checked before residue
    /// storage is copied into the native representation.
    pub fn from_release_wire(
        roster: &ZkAmsMkheGovernedRosterWireV1,
        wire: &ZkAmsMkheCollectiveCiphertextWireV1,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        let profile = release_profile_v1();
        let binding = wire.binding();
        if binding.profile_digest() != roster.profile_digest()
            || binding.roster_digest() != roster.roster_digest()
            || binding.epoch() != roster.epoch()
        {
            return Err(ZkAmsMkheErrorV1::InvalidCiphertext);
        }
        // `encoded_len` performs the release preflight without allocating.
        wire.constant().encoded_len()?;
        wire.linear().encoded_len()?;
        let parties = super::PartySet::new(roster.parties().to_vec())?;
        Self::new(
            &profile,
            &parties,
            roster.epoch(),
            binding.transcript_digest(),
            wire.sample_index(),
            binding.level(),
            RnsPolynomial::from_flat(&profile, wire.constant().residues().to_vec())?,
            RnsPolynomial::from_flat(&profile, wire.linear().residues().to_vec())?,
        )
    }
    /// Convert to the sole canonical release wire representation.
    pub fn to_release_wire(
        &self,
        roster: &ZkAmsMkheGovernedRosterWireV1,
        record_index: u32,
    ) -> Result<ZkAmsMkheCollectiveCiphertextWireV1, ZkAmsMkheErrorV1> {
        let profile = release_profile_v1();
        if roster.profile_digest() != self.profile_digest
            || roster.roster_digest() != self.roster_digest
            || roster.epoch() != self.epoch
            || self.sample_index >= zk_ams_mkhe_release_manifest_v1()?.max_samples_per_secret_epoch
        {
            return Err(ZkAmsMkheErrorV1::InvalidCiphertext);
        }
        let parties = super::PartySet::new(roster.parties().to_vec())?;
        self.validate(&profile, &parties)?;
        let binding =
            ZkAmsMkheWireBindingV1::new(roster, self.transcript_digest, record_index, self.level)?;
        ZkAmsMkheCollectiveCiphertextWireV1::new(
            binding,
            self.sample_index,
            ZkAmsMkheRnsPolynomialWireV1::new(self.constant.coefficients.clone())?,
            ZkAmsMkheRnsPolynomialWireV1::new(self.linear.coefficients.clone())?,
        )
    }
    /// Frozen profile digest.
    #[must_use]
    pub const fn profile_digest(&self) -> [u8; 32] {
        self.profile_digest
    }
    /// Exact ordered governed-roster digest.
    #[must_use]
    pub const fn roster_digest(&self) -> [u8; 32] {
        self.roster_digest
    }
    /// Nonzero governed secret/key epoch.
    #[must_use]
    pub const fn epoch(&self) -> u64 {
        self.epoch
    }
    /// Exact security/key/input/operation lineage digest.
    #[must_use]
    pub const fn transcript_digest(&self) -> [u8; 32] {
        self.transcript_digest
    }
    /// Zero-based RLWE sample identity for fresh encryption, or the canonical
    /// minimum origin index for an evaluated result. The transcript commits
    /// the complete ordered operand lineage.
    #[must_use]
    pub const fn sample_index(&self) -> u64 {
        self.sample_index
    }
    /// BGV ciphertext level (`0` or `1`).
    #[must_use]
    pub const fn level(&self) -> u8 {
        self.level
    }
    /// Consensus digest of every native field and both exact polynomials.
    #[must_use]
    pub const fn digest(&self) -> [u8; 32] {
        self.digest
    }
    /// Verified native evaluation-key identity, absent for wire-only records.
    #[must_use]
    pub const fn evaluation_key_digest(&self) -> Option<[u8; 32]> {
        self.evaluation_key_digest
    }
    #[allow(
        clippy::too_many_arguments,
        reason = "the constructor binds every independently authenticated ciphertext field"
    )]
    pub(super) fn new(
        profile: &BgvProfile,
        parties: &super::PartySet,
        epoch: u64,
        transcript_digest: [u8; 32],
        sample_index: u64,
        level: u8,
        constant: RnsPolynomial,
        linear: RnsPolynomial,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        Self::new_with_key(
            profile,
            parties,
            epoch,
            transcript_digest,
            sample_index,
            level,
            constant,
            linear,
            None,
        )
    }
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new_with_key(
        profile: &BgvProfile,
        parties: &super::PartySet,
        epoch: u64,
        transcript_digest: [u8; 32],
        sample_index: u64,
        level: u8,
        constant: RnsPolynomial,
        linear: RnsPolynomial,
        evaluation_key_digest: Option<[u8; 32]>,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        profile.validate()?;
        if parties.parties.len() != ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1
            || epoch == 0
            || transcript_digest == [0; 32]
            || level > 1
            || evaluation_key_digest.is_some_and(|digest| digest == [0; 32])
        {
            return Err(ZkAmsMkheErrorV1::InvalidCiphertext);
        }
        constant.validate(profile)?;
        linear.validate(profile)?;
        let profile_digest = profile.digest()?;
        let roster_digest = governed_roster_digest(profile_digest, epoch, &parties.parties);
        let mut value = Self {
            profile_digest,
            roster_digest,
            epoch,
            transcript_digest,
            sample_index,
            level,
            constant,
            linear,
            evaluation_key_digest,
            digest: [0; 32],
        };
        value.digest = value.compute_digest(profile)?;
        value.validate(profile, parties)?;
        Ok(value)
    }
    pub(super) fn validate(
        &self,
        profile: &BgvProfile,
        parties: &super::PartySet,
    ) -> Result<(), ZkAmsMkheErrorV1> {
        if self.profile_digest != profile.digest()?
            || self.roster_digest
                != governed_roster_digest(self.profile_digest, self.epoch, &parties.parties)
            || self.epoch == 0
            || self.transcript_digest == [0; 32]
            || self.level > 1
            || self
                .evaluation_key_digest
                .is_some_and(|digest| digest == [0; 32])
            || parties.parties.len() != ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1
        {
            return Err(ZkAmsMkheErrorV1::InvalidCiphertext);
        }
        self.constant.validate(profile)?;
        self.linear.validate(profile)?;
        if self.digest == [0; 32] || self.digest != self.compute_digest(profile)? {
            return Err(ZkAmsMkheErrorV1::InvalidCiphertext);
        }
        Ok(())
    }
    #[allow(
        dead_code,
        reason = "used by the private fail-closed collective evaluated-key runtime"
    )]
    pub(super) const fn constant(&self) -> &RnsPolynomial {
        &self.constant
    }
    #[allow(
        dead_code,
        reason = "used by the private fail-closed collective evaluated-key runtime"
    )]
    pub(super) const fn linear(&self) -> &RnsPolynomial {
        &self.linear
    }
    fn compute_digest(&self, profile: &BgvProfile) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
        let mut hash = Keccak256::new();
        hash.update(COLLECTIVE_CIPHERTEXT_DOMAIN_V1);
        hash.update(&self.profile_digest);
        hash.update(&self.roster_digest);
        hash.update(&self.epoch.to_be_bytes());
        hash.update(&self.transcript_digest);
        hash.update(&self.sample_index.to_be_bytes());
        hash.update(&[self.level]);
        update_rns_hash(&mut hash, profile, &self.constant)?;
        update_rns_hash(&mut hash, profile, &self.linear)?;
        Ok(hash.finalize())
    }
}
#[cfg(test)]
impl ZkAmsMkheCollectiveEncryptionOpeningV1 {
    #[allow(clippy::too_many_arguments, clippy::type_complexity)]
    #[cfg(test)]
    fn with_validated_native_proof_witness_v1<T>(
        &self,
        profile: &BgvProfile,
        key: &ZkAmsMkheCollectivePublicKeyV1,
        expected_message: &RnsPolynomial,
        expected_canonical_plaintext: &[[u8; 32]],
        input_topology: CollectiveEncryptionInputTopologyV1,
        ciphertext: &ZkAmsMkheCollectiveCiphertextV1,
        adapter: impl FnOnce(
            &[[u8; 32]],
            &RnsPolynomial,
            &SecretPolynomial,
            &SecretPolynomial,
            &SecretPolynomial,
        ) -> Result<T, ZkAmsMkheErrorV1>,
    ) -> Result<T, ZkAmsMkheErrorV1> {
        self.validate_against(
            profile,
            key,
            expected_message,
            expected_canonical_plaintext,
            input_topology,
            ciphertext,
        )?;
        adapter(
            &self.canonical_plaintext.0,
            &self.plaintext_lift.0,
            &self.ephemeral,
            &self.error_zero,
            &self.error_one,
        )
    }
    #[allow(clippy::too_many_arguments)]
    fn validate_against(
        &self,
        profile: &BgvProfile,
        key: &ZkAmsMkheCollectivePublicKeyV1,
        expected_message: &RnsPolynomial,
        expected_canonical_plaintext: &[[u8; 32]],
        input_topology: CollectiveEncryptionInputTopologyV1,
        ciphertext: &ZkAmsMkheCollectiveCiphertextV1,
    ) -> Result<(), ZkAmsMkheErrorV1> {
        profile.validate()?;
        key.validate(profile)?;
        expected_message.validate(profile)?;
        validate_compact_for_key(ciphertext, key, profile)?;
        let expected_transcript_digest = collective_encryption_transcript_digest_v1(
            key,
            input_topology,
            self.sample_index,
            self.input_identity.encryption_nonce.as_bytes(),
        );
        if expected_transcript_digest == [0; 32]
            || input_topology.layout_digest == [0; 32]
            || input_topology.plaintext_used_slots == 0
            || usize::try_from(input_topology.plaintext_used_slots)
                .map_or(true, |used_slots| used_slots > profile.ring_degree)
            || self.input_identity.encryption_nonce.is_zero()
            || expected_canonical_plaintext.len() != profile.ring_degree
            || self.profile_digest != profile.digest()?
            || self.profile_digest != key.profile_digest
            || self.security_certificate_digest != key.security_certificate_digest
            || self.roster_digest != key.roster_digest
            || self.key_material_digest != key.key_material_digest
            || self.key_digest != key.digest
            || self.epoch != key.epoch
            || self.key_transcript_digest != key.transcript_digest
            || self.ciphertext_transcript_digest != expected_transcript_digest
            || self.ciphertext_transcript_digest != ciphertext.transcript_digest
            || self.sample_index != ciphertext.sample_index
            || self.input_identity.topology != input_topology
            || self.ciphertext_digest != ciphertext.digest
            || ciphertext.level != 0
            || self.canonical_plaintext.0.as_slice() != expected_canonical_plaintext
            || self.plaintext_lift.0 != *expected_message
            || self.ephemeral.coefficients.len() != profile.ring_degree
            || self
                .ephemeral
                .coefficients
                .iter()
                .any(|coefficient| !(-1..=1).contains(coefficient))
            || self
                .ephemeral
                .coefficients
                .iter()
                .all(|coefficient| *coefficient == 0)
            || !bounded_error_polynomial(profile, &self.error_zero)
            || !bounded_error_polynomial(profile, &self.error_one)
        {
            return Err(ZkAmsMkheErrorV1::InvalidCiphertext);
        }
        self.plaintext_lift.0.validate(profile)?;
        let ephemeral_rns = ZeroizingRns(self.ephemeral.as_rns(profile)?);
        let scaled_error_zero = scaled_public_error(profile, &self.error_zero)?;
        let scaled_error_one = scaled_public_error(profile, &self.error_one)?;
        let constant_product =
            ZeroizingRns(key.collective_public_b.mul(&ephemeral_rns.0, profile)?);
        let constant_with_error =
            ZeroizingRns(constant_product.0.add(&scaled_error_zero.0, profile)?);
        let expected_constant =
            ZeroizingRns(constant_with_error.0.add(&self.plaintext_lift.0, profile)?);
        let linear_product = ZeroizingRns(key.public_a.mul(&ephemeral_rns.0, profile)?);
        let expected_linear = ZeroizingRns(linear_product.0.add(&scaled_error_one.0, profile)?);
        if expected_constant.0 != ciphertext.constant || expected_linear.0 != ciphertext.linear {
            return Err(ZkAmsMkheErrorV1::InvalidCiphertext);
        }
        Ok(())
    }
    #[cfg(test)]
    fn arm_drop_zeroization_audit(&mut self, audit: std::sync::Arc<std::sync::atomic::AtomicBool>) {
        self.drop_audit = Some(audit);
    }
}
/// Encrypt one exact canonical T256 packed-plaintext chunk under the verified
/// all-eight collective public key.
#[cfg(test)]
#[expect(
    dead_code,
    reason = "native reference encryption remains available to same-module parity tests"
)]
pub fn encrypt_zk_ams_mkhe_collective_packed_v1<R: MaskedRelaxedRandomSourceV1>(
    key: &ZkAmsMkheCollectivePublicKeyV1,
    layout: ZkAmsT256PackingLayoutV1,
    plaintext: &ZkAmsT256PackedPlaintextV1,
    sample_index: u64,
    random: &mut R,
) -> Result<ZkAmsMkheCollectiveCiphertextV1, ZkAmsMkheErrorV1> {
    let (ciphertext, opening) = encrypt_zk_ams_mkhe_collective_packed_with_opening_v1(
        key,
        layout,
        plaintext,
        sample_index,
        random,
    )?;
    drop(opening);
    Ok(ciphertext)
}
/// Sibling-private encryption path retaining the exact zeroizing opening for
/// an immediately adjacent proof construction.
#[cfg(test)]
pub(super) fn encrypt_zk_ams_mkhe_collective_packed_with_opening_v1<
    R: MaskedRelaxedRandomSourceV1,
>(
    key: &ZkAmsMkheCollectivePublicKeyV1,
    layout: ZkAmsT256PackingLayoutV1,
    plaintext: &ZkAmsT256PackedPlaintextV1,
    sample_index: u64,
    random: &mut R,
) -> Result<
    (
        ZkAmsMkheCollectiveCiphertextV1,
        ZkAmsMkheCollectiveEncryptionOpeningV1,
    ),
    ZkAmsMkheErrorV1,
> {
    let profile = release_profile_v1();
    key.validate(&profile)?;
    let manifest = zk_ams_mkhe_release_manifest_v1()?;
    if sample_index >= manifest.max_samples_per_secret_epoch
        || key.security_certificate_digest != release_security_certificate_digest()?
        || layout.profile_digest != key.profile_digest
        || plaintext.profile_digest != key.profile_digest
    {
        return Err(ZkAmsMkheErrorV1::InvalidCiphertext);
    }
    // Canonical layout/digest/padding checks happen inside this conversion,
    // before secret randomness or ciphertext-sized output is allocated.
    let message = ZeroizingRns(packed_plaintext_to_rns_v1(layout, plaintext)?);
    let canonical_plaintext = ZeroizingCanonicalPlaintext(plaintext.coefficients.clone());
    let input_topology = CollectiveEncryptionInputTopologyV1::from_packed(layout, plaintext);
    encrypt_collective_native_with_opening(
        &profile,
        key,
        message,
        canonical_plaintext,
        input_topology,
        sample_index,
        random,
    )
}
#[cfg(test)]
impl ZkAmsMkheCollectiveCiphertextV1 {
    /// Add two same-key compact ciphertexts and derive exact combined lineage.
    pub fn add(
        &self,
        key: &ZkAmsMkheCollectivePublicKeyV1,
        rhs: &Self,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        self.binary(key, rhs, COLLECTIVE_ADD_DOMAIN_V1, RnsPolynomial::add)
    }
    /// Subtract two same-key compact ciphertexts and derive exact combined lineage.
    pub fn sub(
        &self,
        key: &ZkAmsMkheCollectivePublicKeyV1,
        rhs: &Self,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        self.binary(key, rhs, COLLECTIVE_SUB_DOMAIN_V1, RnsPolynomial::sub)
    }
    /// Multiply both components by one exact canonical packed plaintext.
    ///
    /// This evaluation operand is public and its digest remains in evaluated
    /// ciphertext lineage. It is separate from fresh-encryption nonce hiding.
    pub fn mul_plaintext(
        &self,
        key: &ZkAmsMkheCollectivePublicKeyV1,
        layout: ZkAmsT256PackingLayoutV1,
        plaintext: &ZkAmsT256PackedPlaintextV1,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        let profile = release_profile_v1();
        validate_compact_for_key(self, key, &profile)?;
        if layout.profile_digest != key.profile_digest
            || plaintext.profile_digest != key.profile_digest
        {
            return Err(ZkAmsMkheErrorV1::InvalidCiphertext);
        }
        let multiplier = packed_plaintext_to_rns_v1(layout, plaintext)?;
        compact_plaintext_mul_with_profile(
            &profile,
            self,
            key,
            &multiplier,
            &[layout.digest.as_slice(), plaintext.digest.as_slice()],
        )
    }
    /// Apply the exact raw Galois automorphism to both components.
    ///
    /// The result is deliberately bound to the automorphed secret-key domain;
    /// it cannot be evaluated as an original-key ciphertext until a verified
    /// Galois key switch restores that domain.
    pub fn automorphism(
        &self,
        key: &ZkAmsMkheCollectivePublicKeyV1,
        exponent: usize,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        let profile = release_profile_v1();
        validate_compact_for_key(self, key, &profile)?;
        let exponent_bytes = u64::try_from(exponent)
            .map_err(|_| ZkAmsMkheErrorV1::InvalidCiphertext)?
            .to_be_bytes();
        compact_automorphism_with_profile(&profile, self, key, exponent, exponent_bytes)
    }
    /// Multiply two level-zero ciphertexts into the exact unrelinearized
    /// `(d_0, d_1, d_2)` level-one form.
    pub fn multiply(
        &self,
        key: &ZkAmsMkheCollectivePublicKeyV1,
        rhs: &Self,
    ) -> Result<ZkAmsMkheCollectiveLevelOneV1, ZkAmsMkheErrorV1> {
        let profile = release_profile_v1();
        multiply_with_profile(&profile, self, key, rhs)
    }
    fn binary(
        &self,
        key: &ZkAmsMkheCollectivePublicKeyV1,
        rhs: &Self,
        domain: &[u8],
        operation: fn(
            &RnsPolynomial,
            &RnsPolynomial,
            &BgvProfile,
        ) -> Result<RnsPolynomial, ZkAmsMkheErrorV1>,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        let profile = release_profile_v1();
        compact_binary_with_profile(&profile, self, key, rhs, domain, operation)
    }
}
#[cfg(test)]
fn compact_binary_with_profile(
    profile: &BgvProfile,
    left: &ZkAmsMkheCollectiveCiphertextV1,
    key: &ZkAmsMkheCollectivePublicKeyV1,
    right: &ZkAmsMkheCollectiveCiphertextV1,
    domain: &[u8],
    operation: fn(
        &RnsPolynomial,
        &RnsPolynomial,
        &BgvProfile,
    ) -> Result<RnsPolynomial, ZkAmsMkheErrorV1>,
) -> Result<ZkAmsMkheCollectiveCiphertextV1, ZkAmsMkheErrorV1> {
    validate_compact_for_key(left, key, profile)?;
    validate_compact_for_key(right, key, profile)?;
    if left.level != right.level {
        return Err(ZkAmsMkheErrorV1::InvalidCiphertext);
    }
    checked_coefficient_work(profile, 2)?;
    let transcript_digest =
        collective_lineage_digest(domain, key, &[left.digest, right.digest], &[]);
    ZkAmsMkheCollectiveCiphertextV1::new_with_key(
        profile,
        &key.parties,
        left.epoch,
        transcript_digest,
        left.sample_index.min(right.sample_index),
        left.level,
        operation(&left.constant, &right.constant, profile)?,
        operation(&left.linear, &right.linear, profile)?,
        Some(key.digest),
    )
}
#[cfg(test)]
fn multiply_with_profile(
    profile: &BgvProfile,
    left: &ZkAmsMkheCollectiveCiphertextV1,
    key: &ZkAmsMkheCollectivePublicKeyV1,
    right: &ZkAmsMkheCollectiveCiphertextV1,
) -> Result<ZkAmsMkheCollectiveLevelOneV1, ZkAmsMkheErrorV1> {
    validate_compact_for_key(left, key, profile)?;
    validate_compact_for_key(right, key, profile)?;
    if left.level != 0 || right.level != 0 {
        return Err(ZkAmsMkheErrorV1::InvalidCiphertext);
    }
    checked_ring_multiplication_work(profile, 4)?;
    let transcript_digest = collective_lineage_digest(
        COLLECTIVE_MULTIPLY_DOMAIN_V1,
        key,
        &[left.digest, right.digest],
        &[],
    );
    let linear = left
        .constant
        .mul(&right.linear, profile)?
        .add(&left.linear.mul(&right.constant, profile)?, profile)?;
    ZkAmsMkheCollectiveLevelOneV1::new(
        profile,
        key,
        transcript_digest,
        left.sample_index.min(right.sample_index),
        left.constant.mul(&right.constant, profile)?,
        linear,
        left.linear.mul(&right.linear, profile)?,
        key.digest,
    )
}
#[cfg(test)]
fn compact_plaintext_mul_with_profile(
    profile: &BgvProfile,
    ciphertext: &ZkAmsMkheCollectiveCiphertextV1,
    key: &ZkAmsMkheCollectivePublicKeyV1,
    multiplier: &RnsPolynomial,
    input_identity: &[&[u8]],
) -> Result<ZkAmsMkheCollectiveCiphertextV1, ZkAmsMkheErrorV1> {
    validate_compact_for_key(ciphertext, key, profile)?;
    multiplier.validate(profile)?;
    checked_ring_multiplication_work(profile, 2)?;
    let transcript_digest = collective_lineage_digest(
        COLLECTIVE_PLAINTEXT_MUL_DOMAIN_V1,
        key,
        &[ciphertext.digest],
        input_identity,
    );
    ZkAmsMkheCollectiveCiphertextV1::new_with_key(
        profile,
        &key.parties,
        ciphertext.epoch,
        transcript_digest,
        ciphertext.sample_index,
        ciphertext.level,
        ciphertext.constant.mul(multiplier, profile)?,
        ciphertext.linear.mul(multiplier, profile)?,
        Some(key.digest),
    )
}
#[cfg(test)]
fn compact_automorphism_with_profile(
    profile: &BgvProfile,
    ciphertext: &ZkAmsMkheCollectiveCiphertextV1,
    key: &ZkAmsMkheCollectivePublicKeyV1,
    exponent: usize,
    exponent_bytes: [u8; 8],
) -> Result<ZkAmsMkheCollectiveCiphertextV1, ZkAmsMkheErrorV1> {
    validate_compact_for_key(ciphertext, key, profile)?;
    // Validate the exponent before deriving the new key-domain identity.
    let constant = ciphertext.constant.automorphism(exponent, profile)?;
    let linear = ciphertext.linear.automorphism(exponent, profile)?;
    let transcript_digest = collective_lineage_digest(
        COLLECTIVE_AUTOMORPHISM_DOMAIN_V1,
        key,
        &[ciphertext.digest],
        &[&exponent_bytes],
    );
    let transformed_key_digest = keccak256(
        &[
            COLLECTIVE_AUTOMORPHISM_DOMAIN_V1,
            key.digest.as_slice(),
            &exponent_bytes,
        ]
        .concat(),
    );
    ZkAmsMkheCollectiveCiphertextV1::new_with_key(
        profile,
        &key.parties,
        ciphertext.epoch,
        transcript_digest,
        ciphertext.sample_index,
        ciphertext.level,
        constant,
        linear,
        Some(transformed_key_digest),
    )
}
/// Exact unrelinearized three-polynomial level-one collective ciphertext.
#[cfg(test)]
pub struct ZkAmsMkheCollectiveLevelOneV1 {
    version: u8,
    profile_digest: [u8; 32],
    security_certificate_digest: [u8; 32],
    roster_digest: [u8; 32],
    epoch: u64,
    transcript_digest: [u8; 32],
    sample_index: u64,
    evaluation_key_digest: [u8; 32],
    constant: RnsPolynomial,
    linear: RnsPolynomial,
    quadratic: RnsPolynomial,
    digest: [u8; 32],
}
#[cfg(test)]
impl core::fmt::Debug for ZkAmsMkheCollectiveLevelOneV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("ZkAmsMkheCollectiveLevelOneV1")
            .field("profile_digest", &hex::encode(self.profile_digest))
            .field("roster_digest", &hex::encode(self.roster_digest))
            .field("epoch", &self.epoch)
            .field("transcript_digest", &hex::encode(self.transcript_digest))
            .field("sample_index", &self.sample_index)
            .field(
                "evaluation_key_digest",
                &hex::encode(self.evaluation_key_digest),
            )
            .field("digest", &hex::encode(self.digest))
            .finish_non_exhaustive()
    }
}
#[cfg(test)]
impl ZkAmsMkheCollectiveLevelOneV1 {
    /// Frozen profile digest.
    #[must_use]
    pub const fn profile_digest(&self) -> [u8; 32] {
        self.profile_digest
    }
    /// Exact ordered governed-roster digest.
    #[must_use]
    pub const fn roster_digest(&self) -> [u8; 32] {
        self.roster_digest
    }
    /// Governed secret/key epoch.
    #[must_use]
    pub const fn epoch(&self) -> u64 {
        self.epoch
    }
    /// Security/key/input/operation lineage digest.
    #[must_use]
    pub const fn transcript_digest(&self) -> [u8; 32] {
        self.transcript_digest
    }
    /// Canonical minimum origin-sample index; complete origin identity is in
    /// the transcript lineage.
    #[must_use]
    pub const fn sample_index(&self) -> u64 {
        self.sample_index
    }
    /// Exact evaluation-key domain required by this ciphertext.
    #[must_use]
    pub const fn evaluation_key_digest(&self) -> [u8; 32] {
        self.evaluation_key_digest
    }
    /// Consensus digest of all context fields and exactly three polynomials.
    #[must_use]
    pub const fn digest(&self) -> [u8; 32] {
        self.digest
    }
    /// Add two same-domain level-one ciphertexts.
    pub fn add(
        &self,
        key: &ZkAmsMkheCollectivePublicKeyV1,
        rhs: &Self,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        self.binary(key, rhs, COLLECTIVE_ADD_DOMAIN_V1, RnsPolynomial::add)
    }
    /// Subtract two same-domain level-one ciphertexts.
    pub fn sub(
        &self,
        key: &ZkAmsMkheCollectivePublicKeyV1,
        rhs: &Self,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        self.binary(key, rhs, COLLECTIVE_SUB_DOMAIN_V1, RnsPolynomial::sub)
    }
    /// Multiply all three level-one components by a canonical packed plaintext.
    ///
    /// This evaluation operand is public and its digest remains in evaluated
    /// ciphertext lineage. It is separate from fresh-encryption nonce hiding.
    pub fn mul_plaintext(
        &self,
        key: &ZkAmsMkheCollectivePublicKeyV1,
        layout: ZkAmsT256PackingLayoutV1,
        plaintext: &ZkAmsT256PackedPlaintextV1,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        let profile = release_profile_v1();
        self.validate_for_key(key, &profile)?;
        if layout.profile_digest != key.profile_digest
            || plaintext.profile_digest != key.profile_digest
        {
            return Err(ZkAmsMkheErrorV1::InvalidCiphertext);
        }
        let multiplier = packed_plaintext_to_rns_v1(layout, plaintext)?;
        level_one_plaintext_mul_with_profile(
            &profile,
            self,
            key,
            &multiplier,
            &[layout.digest.as_slice(), plaintext.digest.as_slice()],
        )
    }
    /// Apply the raw automorphism to all three components and move to the
    /// corresponding automorphed secret-key domain.
    pub fn automorphism(
        &self,
        key: &ZkAmsMkheCollectivePublicKeyV1,
        exponent: usize,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        let profile = release_profile_v1();
        self.validate_for_key(key, &profile)?;
        let exponent_bytes = u64::try_from(exponent)
            .map_err(|_| ZkAmsMkheErrorV1::InvalidCiphertext)?
            .to_be_bytes();
        level_one_automorphism_with_profile(&profile, self, key, exponent, exponent_bytes)
    }
    #[allow(clippy::too_many_arguments)]
    fn new(
        profile: &BgvProfile,
        key: &ZkAmsMkheCollectivePublicKeyV1,
        transcript_digest: [u8; 32],
        sample_index: u64,
        constant: RnsPolynomial,
        linear: RnsPolynomial,
        quadratic: RnsPolynomial,
        evaluation_key_digest: [u8; 32],
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        key.validate(profile)?;
        if transcript_digest == [0; 32] || evaluation_key_digest == [0; 32] {
            return Err(ZkAmsMkheErrorV1::InvalidCiphertext);
        }
        constant.validate(profile)?;
        linear.validate(profile)?;
        quadratic.validate(profile)?;
        let mut value = Self {
            version: MKHE_VERSION_V1,
            profile_digest: key.profile_digest,
            security_certificate_digest: key.security_certificate_digest,
            roster_digest: key.roster_digest,
            epoch: key.epoch,
            transcript_digest,
            sample_index,
            evaluation_key_digest,
            constant,
            linear,
            quadratic,
            digest: [0; 32],
        };
        value.digest = value.compute_digest(profile)?;
        value.validate(profile)?;
        Ok(value)
    }
    fn validate(&self, profile: &BgvProfile) -> Result<(), ZkAmsMkheErrorV1> {
        if self.version != MKHE_VERSION_V1
            || self.profile_digest != profile.digest()?
            || self.security_certificate_digest == [0; 32]
            || self.roster_digest == [0; 32]
            || self.epoch == 0
            || self.transcript_digest == [0; 32]
            || self.evaluation_key_digest == [0; 32]
        {
            return Err(ZkAmsMkheErrorV1::InvalidCiphertext);
        }
        self.constant.validate(profile)?;
        self.linear.validate(profile)?;
        self.quadratic.validate(profile)?;
        if self.digest == [0; 32] || self.digest != self.compute_digest(profile)? {
            return Err(ZkAmsMkheErrorV1::InvalidCiphertext);
        }
        Ok(())
    }
    pub(super) fn validate_for_key(
        &self,
        key: &ZkAmsMkheCollectivePublicKeyV1,
        profile: &BgvProfile,
    ) -> Result<(), ZkAmsMkheErrorV1> {
        key.validate(profile)?;
        self.validate(profile)?;
        if self.profile_digest != key.profile_digest
            || self.security_certificate_digest != key.security_certificate_digest
            || self.roster_digest != key.roster_digest
            || self.epoch != key.epoch
            || self.evaluation_key_digest != key.digest
        {
            return Err(ZkAmsMkheErrorV1::InvalidCiphertext);
        }
        Ok(())
    }
    pub(super) const fn constant(&self) -> &RnsPolynomial {
        &self.constant
    }
    pub(super) const fn linear(&self) -> &RnsPolynomial {
        &self.linear
    }
    pub(super) const fn quadratic(&self) -> &RnsPolynomial {
        &self.quadratic
    }
    fn binary(
        &self,
        key: &ZkAmsMkheCollectivePublicKeyV1,
        rhs: &Self,
        domain: &[u8],
        operation: fn(
            &RnsPolynomial,
            &RnsPolynomial,
            &BgvProfile,
        ) -> Result<RnsPolynomial, ZkAmsMkheErrorV1>,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        let profile = release_profile_v1();
        level_one_binary_with_profile(&profile, self, key, rhs, domain, operation)
    }
    fn compute_digest(&self, profile: &BgvProfile) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
        let mut hash = Keccak256::new();
        hash.update(COLLECTIVE_LEVEL_ONE_DOMAIN_V1);
        hash.update(&[self.version]);
        hash.update(&self.profile_digest);
        hash.update(&self.security_certificate_digest);
        hash.update(&self.roster_digest);
        hash.update(&self.epoch.to_be_bytes());
        hash.update(&self.transcript_digest);
        hash.update(&self.sample_index.to_be_bytes());
        hash.update(&self.evaluation_key_digest);
        hash.update(&[1]);
        update_rns_hash(&mut hash, profile, &self.constant)?;
        update_rns_hash(&mut hash, profile, &self.linear)?;
        update_rns_hash(&mut hash, profile, &self.quadratic)?;
        Ok(hash.finalize())
    }
}
#[cfg(test)]
fn level_one_binary_with_profile(
    profile: &BgvProfile,
    left: &ZkAmsMkheCollectiveLevelOneV1,
    key: &ZkAmsMkheCollectivePublicKeyV1,
    right: &ZkAmsMkheCollectiveLevelOneV1,
    domain: &[u8],
    operation: fn(
        &RnsPolynomial,
        &RnsPolynomial,
        &BgvProfile,
    ) -> Result<RnsPolynomial, ZkAmsMkheErrorV1>,
) -> Result<ZkAmsMkheCollectiveLevelOneV1, ZkAmsMkheErrorV1> {
    left.validate_for_key(key, profile)?;
    right.validate_for_key(key, profile)?;
    checked_coefficient_work(profile, 3)?;
    let transcript_digest =
        collective_lineage_digest(domain, key, &[left.digest, right.digest], &[]);
    ZkAmsMkheCollectiveLevelOneV1::new(
        profile,
        key,
        transcript_digest,
        left.sample_index.min(right.sample_index),
        operation(&left.constant, &right.constant, profile)?,
        operation(&left.linear, &right.linear, profile)?,
        operation(&left.quadratic, &right.quadratic, profile)?,
        key.digest,
    )
}
#[cfg(test)]
fn level_one_plaintext_mul_with_profile(
    profile: &BgvProfile,
    ciphertext: &ZkAmsMkheCollectiveLevelOneV1,
    key: &ZkAmsMkheCollectivePublicKeyV1,
    multiplier: &RnsPolynomial,
    input_identity: &[&[u8]],
) -> Result<ZkAmsMkheCollectiveLevelOneV1, ZkAmsMkheErrorV1> {
    ciphertext.validate_for_key(key, profile)?;
    multiplier.validate(profile)?;
    checked_ring_multiplication_work(profile, 3)?;
    let transcript_digest = collective_lineage_digest(
        COLLECTIVE_PLAINTEXT_MUL_DOMAIN_V1,
        key,
        &[ciphertext.digest],
        input_identity,
    );
    ZkAmsMkheCollectiveLevelOneV1::new(
        profile,
        key,
        transcript_digest,
        ciphertext.sample_index,
        ciphertext.constant.mul(multiplier, profile)?,
        ciphertext.linear.mul(multiplier, profile)?,
        ciphertext.quadratic.mul(multiplier, profile)?,
        key.digest,
    )
}
#[cfg(test)]
fn level_one_automorphism_with_profile(
    profile: &BgvProfile,
    ciphertext: &ZkAmsMkheCollectiveLevelOneV1,
    key: &ZkAmsMkheCollectivePublicKeyV1,
    exponent: usize,
    exponent_bytes: [u8; 8],
) -> Result<ZkAmsMkheCollectiveLevelOneV1, ZkAmsMkheErrorV1> {
    ciphertext.validate_for_key(key, profile)?;
    let constant = ciphertext.constant.automorphism(exponent, profile)?;
    let linear = ciphertext.linear.automorphism(exponent, profile)?;
    let quadratic = ciphertext.quadratic.automorphism(exponent, profile)?;
    let transcript_digest = collective_lineage_digest(
        COLLECTIVE_AUTOMORPHISM_DOMAIN_V1,
        key,
        &[ciphertext.digest],
        &[&exponent_bytes],
    );
    let transformed_key_digest = keccak256(
        &[
            COLLECTIVE_AUTOMORPHISM_DOMAIN_V1,
            key.digest.as_slice(),
            &exponent_bytes,
        ]
        .concat(),
    );
    ZkAmsMkheCollectiveLevelOneV1::new(
        profile,
        key,
        transcript_digest,
        ciphertext.sample_index,
        constant,
        linear,
        quadratic,
        transformed_key_digest,
    )
}
#[cfg(test)]
pub(super) fn validate_compact_for_key(
    ciphertext: &ZkAmsMkheCollectiveCiphertextV1,
    key: &ZkAmsMkheCollectivePublicKeyV1,
    profile: &BgvProfile,
) -> Result<(), ZkAmsMkheErrorV1> {
    key.validate(profile)?;
    ciphertext.validate(profile, &key.parties)?;
    if ciphertext.profile_digest != key.profile_digest
        || ciphertext.roster_digest != key.roster_digest
        || ciphertext.epoch != key.epoch
        || ciphertext.evaluation_key_digest != Some(key.digest)
    {
        return Err(ZkAmsMkheErrorV1::InvalidCiphertext);
    }
    Ok(())
}
#[allow(clippy::too_many_arguments)]
#[cfg(test)]
fn encrypt_collective_native_with_opening<R: MaskedRelaxedRandomSourceV1>(
    profile: &BgvProfile,
    key: &ZkAmsMkheCollectivePublicKeyV1,
    message: ZeroizingRns,
    canonical_plaintext: ZeroizingCanonicalPlaintext,
    input_topology: CollectiveEncryptionInputTopologyV1,
    sample_index: u64,
    random: &mut R,
) -> Result<
    (
        ZkAmsMkheCollectiveCiphertextV1,
        ZkAmsMkheCollectiveEncryptionOpeningV1,
    ),
    ZkAmsMkheErrorV1,
> {
    key.validate(profile)?;
    message.0.validate(profile)?;
    if input_topology.layout_digest == [0; 32]
        || input_topology.plaintext_used_slots == 0
        || usize::try_from(input_topology.plaintext_used_slots)
            .map_or(true, |used_slots| used_slots > profile.ring_degree)
        || canonical_plaintext.0.len() != profile.ring_degree
    {
        return Err(ZkAmsMkheErrorV1::InvalidCiphertext);
    }
    checked_ring_multiplication_work(profile, 2)?;
    let encryption_nonce = derive_collective_encryption_nonce_v1(random)?;
    let input_identity = CollectiveEncryptionInputIdentityV1 {
        topology: input_topology,
        encryption_nonce,
    };
    let transcript_digest = collective_encryption_transcript_digest_v1(
        key,
        input_topology,
        sample_index,
        input_identity.encryption_nonce.as_bytes(),
    );
    if transcript_digest == [0; 32] {
        return Err(ZkAmsMkheErrorV1::RandomUnavailable);
    }
    let ephemeral = sample_nonzero_ternary_zeroizing(profile, random)?;
    let error_zero = sample_bounded_error(profile, random)?;
    let error_one = sample_bounded_error(profile, random)?;
    let ephemeral_rns = ZeroizingRns(ephemeral.as_rns(profile)?);
    let scaled_error_zero = scaled_public_error(profile, &error_zero)?;
    let scaled_error_one = scaled_public_error(profile, &error_one)?;
    let constant_product = ZeroizingRns(key.collective_public_b.mul(&ephemeral_rns.0, profile)?);
    let constant_with_error = ZeroizingRns(constant_product.0.add(&scaled_error_zero.0, profile)?);
    let constant = ZeroizingRns(constant_with_error.0.add(&message.0, profile)?);
    let linear_product = ZeroizingRns(key.public_a.mul(&ephemeral_rns.0, profile)?);
    let linear = ZeroizingRns(linear_product.0.add(&scaled_error_one.0, profile)?);
    // The two ciphertext components are public outputs. Secret-derived
    // intermediates remain in zeroizing wrappers across every fallible step.
    let ciphertext = ZkAmsMkheCollectiveCiphertextV1::new_with_key(
        profile,
        &key.parties,
        key.epoch,
        transcript_digest,
        sample_index,
        0,
        constant.0.clone(),
        linear.0.clone(),
        Some(key.digest),
    )?;
    let opening = ZkAmsMkheCollectiveEncryptionOpeningV1 {
        profile_digest: profile.digest()?,
        security_certificate_digest: key.security_certificate_digest,
        roster_digest: key.roster_digest,
        key_material_digest: key.key_material_digest,
        key_digest: key.digest,
        epoch: key.epoch,
        key_transcript_digest: key.transcript_digest,
        ciphertext_transcript_digest: transcript_digest,
        sample_index,
        input_identity,
        ciphertext_digest: ciphertext.digest,
        canonical_plaintext,
        plaintext_lift: message,
        ephemeral,
        error_zero,
        error_one,
        #[cfg(test)]
        drop_audit: None,
    };
    opening.validate_against(
        profile,
        key,
        &opening.plaintext_lift.0,
        &opening.canonical_plaintext.0,
        input_topology,
        &ciphertext,
    )?;
    Ok((ciphertext, opening))
}
#[cfg(test)]
fn collective_lineage_digest(
    domain: &[u8],
    key: &ZkAmsMkheCollectivePublicKeyV1,
    operand_digests: &[[u8; 32]],
    supplemental: &[&[u8]],
) -> [u8; 32] {
    let mut frame = Vec::with_capacity(
        256 + operand_digests.len() * 32 + supplemental.iter().map(|v| v.len()).sum::<usize>(),
    );
    frame.extend_from_slice(domain);
    frame.push(MKHE_VERSION_V1);
    frame.extend_from_slice(&key.profile_digest);
    frame.extend_from_slice(&key.security_certificate_digest);
    frame.extend_from_slice(&key.roster_digest);
    frame.extend_from_slice(&key.key_material_digest);
    frame.extend_from_slice(&key.epoch.to_be_bytes());
    frame.extend_from_slice(&key.transcript_digest);
    frame.extend_from_slice(&key.digest);
    frame.extend_from_slice(&(operand_digests.len() as u32).to_be_bytes());
    for digest in operand_digests {
        frame.extend_from_slice(digest);
    }
    frame.extend_from_slice(&(supplemental.len() as u32).to_be_bytes());
    for value in supplemental {
        frame.extend_from_slice(&(value.len() as u32).to_be_bytes());
        frame.extend_from_slice(value);
    }
    keccak256(&frame)
}
#[cfg(test)]
fn collective_encryption_transcript_digest_v1(
    key: &ZkAmsMkheCollectivePublicKeyV1,
    topology: CollectiveEncryptionInputTopologyV1,
    sample_index: u64,
    encryption_nonce: &[u8; 32],
) -> [u8; 32] {
    // Stream the nonce directly into the sponge instead of copying it into the
    // heap frame used by generic public evaluation lineage.
    // Allocate the all-zero sponge before it absorbs the opening nonce. Every
    // later move therefore transports only the `Box` pointer.
    let chunk_index = topology.plaintext_chunk_index.to_be_bytes();
    let used_slots = topology.plaintext_used_slots.to_be_bytes();
    let sample_index = sample_index.to_be_bytes();
    let mut hash = Box::new(Keccak256::new());
    hash.update(COLLECTIVE_ENCRYPTION_DOMAIN_V1);
    hash.update(&[MKHE_VERSION_V1]);
    hash.update(&key.profile_digest);
    hash.update(&key.security_certificate_digest);
    hash.update(&key.roster_digest);
    hash.update(&key.key_material_digest);
    hash.update(&key.epoch.to_be_bytes());
    hash.update(&key.transcript_digest);
    hash.update(&key.digest);
    hash.update(&0_u32.to_be_bytes());
    hash.update(&5_u32.to_be_bytes());
    hash.update(&32_u32.to_be_bytes());
    hash.update(&topology.layout_digest);
    hash.update(&4_u32.to_be_bytes());
    hash.update(&chunk_index);
    hash.update(&4_u32.to_be_bytes());
    hash.update(&used_slots);
    hash.update(&8_u32.to_be_bytes());
    hash.update(&sample_index);
    hash.update(&32_u32.to_be_bytes());
    hash.update(encryption_nonce);
    let mut digest = [0_u8; 32];
    hash.finalize_into(&mut digest);
    drop(hash);
    digest
}
#[cfg(test)]
fn scaled_public_error(
    profile: &BgvProfile,
    error: &SecretPolynomial,
) -> Result<ZeroizingRns, ZkAmsMkheErrorV1> {
    let raw = ZeroizingRns(error.as_rns(profile)?);
    Ok(ZeroizingRns(raw.0.scale_plaintext_modulus(profile)?))
}
fn negate_and_add_scaled_error_in_place(
    product: &mut RnsPolynomial,
    error: &SecretPolynomial,
    profile: &BgvProfile,
) -> Result<(), ZkAmsMkheErrorV1> {
    product.validate(profile)?;
    if !bounded_error_polynomial(profile, error) {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    for (limb, values) in product
        .coefficients
        .chunks_exact_mut(profile.ring_degree)
        .enumerate()
    {
        let modulus = profile.moduli[limb];
        let plaintext_modulus = profile.plaintext_modulus.residue(modulus);
        for (value, error) in values.iter_mut().zip(&error.coefficients) {
            let negated = if *value == 0 { 0 } else { modulus - *value };
            let scaled_error = super::mod_mul(
                super::signed_mod(*error, modulus),
                plaintext_modulus,
                modulus,
            );
            *value = super::mod_add(negated, scaled_error, modulus);
        }
    }
    Ok(())
}
fn sample_nonzero_ternary<R: MaskedRelaxedRandomSourceV1>(
    profile: &BgvProfile,
    random: &mut R,
) -> Result<SecretPolynomial, ZkAmsMkheErrorV1> {
    for _ in 0..MAX_RANDOM_REJECTION_ATTEMPTS_V1 {
        let candidate = SecretPolynomial::sample_ternary(profile, random)?;
        if candidate
            .coefficients
            .iter()
            .any(|coefficient| *coefficient != 0)
        {
            return Ok(candidate);
        }
    }
    Err(ZkAmsMkheErrorV1::RandomUnavailable)
}
#[cfg(test)]
fn sample_nonzero_ternary_zeroizing<R: MaskedRelaxedRandomSourceV1>(
    profile: &BgvProfile,
    random: &mut R,
) -> Result<SecretPolynomial, ZkAmsMkheErrorV1> {
    for _ in 0..MAX_RANDOM_REJECTION_ATTEMPTS_V1 {
        let candidate = sample_ternary_zeroizing(profile, random)?;
        if candidate
            .coefficients
            .iter()
            .any(|coefficient| *coefficient != 0)
        {
            return Ok(candidate);
        }
    }
    Err(ZkAmsMkheErrorV1::RandomUnavailable)
}
#[cfg(test)]
fn derive_collective_encryption_nonce_v1<R: MaskedRelaxedRandomSourceV1>(
    random: &mut R,
) -> Result<ZeroizingEncryptionNonce, ZkAmsMkheErrorV1> {
    let mut first = ZeroizingEntropyProbe([0; 32]);
    let mut second = ZeroizingEntropyProbe([0; 32]);
    random
        .fill_bytes(&mut first.0)
        .map_err(|_| ZkAmsMkheErrorV1::RandomUnavailable)?;
    random
        .fill_bytes(&mut second.0)
        .map_err(|_| ZkAmsMkheErrorV1::RandomUnavailable)?;
    if first.0 == second.0
        || entropy_probe_has_short_period(&first.0)
        || entropy_probe_has_short_period(&second.0)
    {
        return Err(ZkAmsMkheErrorV1::RandomUnavailable);
    }
    // Both live nonce material and its absorbing sponge have stable heap
    // addresses from first secret write through optimizer-resistant drop.
    let mut hash = Box::new(Keccak256::new());
    hash.update(COLLECTIVE_ENCRYPTION_NONCE_DOMAIN_V1);
    hash.update(&[MKHE_VERSION_V1]);
    hash.update(&first.0);
    hash.update(&second.0);
    let mut nonce = ZeroizingEncryptionNonce::zeroed();
    hash.finalize_into(nonce.as_mut_bytes());
    drop(hash);
    if nonce.is_zero() {
        return Err(ZkAmsMkheErrorV1::RandomUnavailable);
    }
    Ok(nonce)
}
fn entropy_probe_has_short_period(probe: &[u8; 32]) -> bool {
    (1..=probe.len() / 2).any(|period| {
        probe[period..]
            .iter()
            .zip(&probe[..probe.len() - period])
            .all(|(current, prior)| current == prior)
    })
}
#[cfg(test)]
fn sample_ternary_zeroizing<R: MaskedRelaxedRandomSourceV1>(
    profile: &BgvProfile,
    random: &mut R,
) -> Result<SecretPolynomial, ZkAmsMkheErrorV1> {
    let mut coefficients = ZeroizingSecretCoefficients(Vec::with_capacity(profile.ring_degree));
    let max_bytes = profile
        .ring_degree
        .checked_mul(super::MAX_TERNARY_SAMPLE_BYTES_PER_COEFFICIENT_V1)
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    super::checked_rng_bytes(profile, max_bytes)?;
    for _ in 0..max_bytes {
        let mut byte = ZeroizingRandomByte([0]);
        random
            .fill_bytes(&mut byte.0)
            .map_err(|_| ZkAmsMkheErrorV1::RandomUnavailable)?;
        for shift in [0, 2, 4, 6] {
            match (byte.0[0] >> shift) & 0x03 {
                0 => coefficients.0.push(-1),
                1 => coefficients.0.push(0),
                2 => coefficients.0.push(1),
                _ => continue,
            }
            if coefficients.0.len() == profile.ring_degree {
                return Ok(SecretPolynomial {
                    coefficients: core::mem::take(&mut coefficients.0),
                });
            }
        }
    }
    Err(ZkAmsMkheErrorV1::RandomUnavailable)
}
#[cfg(test)]
fn sample_bounded_error<R: MaskedRelaxedRandomSourceV1>(
    profile: &BgvProfile,
    random: &mut R,
) -> Result<SecretPolynomial, ZkAmsMkheErrorV1> {
    let max_bytes = profile
        .ring_degree
        .checked_mul(usize::from(profile.error_eta))
        .and_then(|value| value.checked_mul(2))
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    super::checked_rng_bytes(profile, max_bytes)?;
    let mut coefficients = ZeroizingSecretCoefficients(Vec::with_capacity(profile.ring_degree));
    for _ in 0..profile.ring_degree {
        let mut positive = 0_i64;
        let mut negative = 0_i64;
        for _ in 0..profile.error_eta {
            let mut byte = ZeroizingRandomByte([0]);
            random
                .fill_bytes(&mut byte.0)
                .map_err(|_| ZkAmsMkheErrorV1::RandomUnavailable)?;
            positive += i64::from(byte.0[0] & 1);
            let mut byte = ZeroizingRandomByte([0]);
            random
                .fill_bytes(&mut byte.0)
                .map_err(|_| ZkAmsMkheErrorV1::RandomUnavailable)?;
            negative += i64::from(byte.0[0] & 1);
        }
        coefficients.0.push(positive - negative);
    }
    Ok(SecretPolynomial {
        coefficients: core::mem::take(&mut coefficients.0),
    })
}
fn bounded_error_polynomial(profile: &BgvProfile, error: &SecretPolynomial) -> bool {
    error.coefficients.len() == profile.ring_degree
        && error
            .coefficients
            .iter()
            .all(|coefficient| coefficient.unsigned_abs() <= u64::from(profile.error_eta))
}
#[cfg(test)]
fn derive_natural_lift_effective_error_zero(
    profile: &BgvProfile,
    canonical_plaintext: &[[u8; 32]],
    sampled_error_zero: &SecretPolynomial,
) -> Result<SecretPolynomial, ZkAmsMkheErrorV1> {
    if profile.plaintext_modulus != super::PlaintextModulus::T256
        || canonical_plaintext.len() != profile.ring_degree
        || !bounded_error_polynomial(profile, sampled_error_zero)
    {
        return Err(ZkAmsMkheErrorV1::InvalidCiphertext);
    }
    let mut coefficients = ZeroizingSecretCoefficients(Vec::with_capacity(profile.ring_degree));
    for (canonical, sampled_error) in canonical_plaintext
        .iter()
        .zip(&sampled_error_zero.coefficients)
    {
        let upper_half = i64::from(*canonical > super::T256_CENTERED_MAX_BE_V1);
        coefficients.0.push(*sampled_error - upper_half);
    }
    let minimum = -i64::from(profile.error_eta) - 1;
    let maximum = i64::from(profile.error_eta);
    if coefficients
        .0
        .iter()
        .any(|coefficient| *coefficient < minimum || *coefficient > maximum)
    {
        return Err(ZkAmsMkheErrorV1::InvalidCiphertext);
    }
    Ok(SecretPolynomial {
        coefficients: core::mem::take(&mut coefficients.0),
    })
}
fn update_rns_hash(
    hash: &mut Keccak256,
    profile: &BgvProfile,
    polynomial: &RnsPolynomial,
) -> Result<(), ZkAmsMkheErrorV1> {
    polynomial.validate(profile)?;
    hash.update(
        &u32::try_from(polynomial.coefficients.len())
            .map_err(|_| ZkAmsMkheErrorV1::InvalidPolynomial)?
            .to_be_bytes(),
    );
    for coefficient in &polynomial.coefficients {
        hash.update(&coefficient.to_be_bytes());
    }
    Ok(())
}
#[cfg(test)]
#[path = "collective/tests.rs"]
mod tests;
