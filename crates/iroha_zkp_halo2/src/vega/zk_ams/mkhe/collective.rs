//! Exact compact collective RNS-BGV key and ciphertext core.
//!
//! This module owns the sole native two-polynomial collective ciphertext used
//! by encryption, evaluation, canonical wire conversion, and full-roster
//! decryption.  Secret RLWE coefficients never cross its public API boundary.

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
    active_exact_binding::{
        PersistentWitnessConsumerV1, VerifiedPersistentWitnessBindingV1,
        mint_collective_secret_binding_from_verified_cpk_v1,
    },
    checked_coefficient_work, checked_ring_multiplication_work,
    cpk_relation::{VerifiedZkAmsMkheCpkContributionV1, ZK_AMS_MKHE_CPK_PARTY_B_OBJECT_BYTES_V1},
    manifest::{
        ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1, release_profile_v1, zk_ams_mkhe_release_manifest_v1,
        zk_ams_mkhe_security_certificate_v1,
    },
    packing::{ZkAmsT256PackedPlaintextV1, ZkAmsT256PackingLayoutV1, packed_plaintext_to_rns_v1},
    persistent_membership_evidence::ZK_AMS_MKHE_PERSISTENT_MEMBERSHIP_CHUNKS_V1,
    wire::{
        ZkAmsMkheCollectiveCiphertextWireV1, ZkAmsMkheGovernedRosterWireV1,
        ZkAmsMkheRnsPolynomialWireV1, ZkAmsMkheWireBindingV1, governed_roster_digest,
    },
};
use crate::vega::{
    VegaT256PointV1 as Point,
    bulletproof_t256::{
        ZK_AMS_MEMBERSHIP_CHUNK_COEFFICIENTS_V1, ZkAmsT256MembershipBoundV1,
        commit_zk_ams_t256_membership_chunk_v1,
    },
    sponge::{Keccak256, keccak256},
};

const COLLECTIVE_CIPHERTEXT_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.compact-collective-ciphertext";
const COLLECTIVE_PARTY_SHARE_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.collective-public-key-share";
const COLLECTIVE_PUBLIC_KEY_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.collective-public-key";
const COLLECTIVE_ENCRYPTION_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.collective-encryption";
const COLLECTIVE_ADD_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.collective-add";
const COLLECTIVE_SUB_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.collective-sub";
const COLLECTIVE_PLAINTEXT_MUL_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.collective-plaintext-mul";
const COLLECTIVE_AUTOMORPHISM_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.collective-automorphism";
const COLLECTIVE_MULTIPLY_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.collective-multiply";
const COLLECTIVE_LEVEL_ONE_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.collective-level-one";

struct ZeroizingRns(RnsPolynomial);

impl Drop for ZeroizingRns {
    fn drop(&mut self) {
        self.0.coefficients.fill(0);
    }
}

struct ZeroizingSecretCoefficients(Vec<i64>);

impl Drop for ZeroizingSecretCoefficients {
    fn drop(&mut self) {
        self.0.fill(0);
    }
}

/// Short-lived canonical T256 view of an owned RLWE secret.
///
/// The state remains the only long-lived owner. This adapter exists solely to
/// bind the complete CPK relation to the exact state opening and erases its
/// narrowed copy on every ordinary, error, and unwind exit.
struct ZeroizingT256MembershipCoefficientsV1(Vec<i8>);

impl ZeroizingT256MembershipCoefficientsV1 {
    fn from_ternary_secret(secret: &SecretPolynomial) -> Result<Self, ZkAmsMkheErrorV1> {
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
        for coefficient in secret.coefficients.iter().copied() {
            let coefficient =
                i8::try_from(coefficient).map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?;
            if !(-1..=1).contains(&coefficient) {
                return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
            }
            coefficients.0.push(coefficient);
        }
        Ok(coefficients)
    }

    fn as_slice(&self) -> &[i8] {
        &self.0
    }
}

impl Drop for ZeroizingT256MembershipCoefficientsV1 {
    fn drop(&mut self) {
        let coefficients = core::hint::black_box(&mut self.0);
        coefficients.fill(0);
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
        let _ = core::hint::black_box(&mut *coefficients);
    }
}

fn commit_persistent_secret_opening_v1(
    coefficients: &[i8],
    blindings: &[Scalar; ZK_AMS_MKHE_PERSISTENT_MEMBERSHIP_CHUNKS_V1],
) -> Result<[Point; ZK_AMS_MKHE_PERSISTENT_MEMBERSHIP_CHUNKS_V1], ZkAmsMkheErrorV1> {
    let chunks = coefficients.chunks_exact(ZK_AMS_MEMBERSHIP_CHUNK_COEFFICIENTS_V1);
    if !chunks.remainder().is_empty() || chunks.len() != blindings.len() {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    let mut commitments = Vec::new();
    commitments
        .try_reserve_exact(blindings.len())
        .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    for (chunk, blinding) in chunks.zip(blindings.iter()) {
        commitments.push(
            commit_zk_ams_t256_membership_chunk_v1(
                ZkAmsT256MembershipBoundV1::One,
                chunk,
                blinding,
            )
            .map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?,
        );
    }
    commitments
        .try_into()
        .map_err(|_: Vec<Point>| ZkAmsMkheErrorV1::InvalidKeyMaterial)
}

fn ensure_state_owned_cpk_commitments_v1(
    verified: &[Point; ZK_AMS_MKHE_PERSISTENT_MEMBERSHIP_CHUNKS_V1],
    expected: &[Point; ZK_AMS_MKHE_PERSISTENT_MEMBERSHIP_CHUNKS_V1],
) -> Result<(), ZkAmsMkheErrorV1> {
    if verified != expected {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    Ok(())
}

struct ZeroizingCanonicalPlaintext(Vec<[u8; 32]>);

impl Drop for ZeroizingCanonicalPlaintext {
    fn drop(&mut self) {
        for coefficient in &mut self.0 {
            coefficient.fill(0);
        }
    }
}

struct ZeroizingEntropyProbe([u8; 32]);

impl Drop for ZeroizingEntropyProbe {
    fn drop(&mut self) {
        self.0.fill(0);
    }
}

struct ZeroizingRandomByte([u8; 1]);

impl Drop for ZeroizingRandomByte {
    fn drop(&mut self) {
        self.0.fill(0);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CollectiveEncryptionInputIdentityV1 {
    layout_digest: [u8; 32],
    plaintext_digest: [u8; 32],
    plaintext_chunk_index: u32,
    plaintext_used_slots: u32,
}

impl CollectiveEncryptionInputIdentityV1 {
    const fn from_packed(
        layout: ZkAmsT256PackingLayoutV1,
        plaintext: &ZkAmsT256PackedPlaintextV1,
    ) -> Self {
        Self {
            layout_digest: layout.digest,
            plaintext_digest: plaintext.digest,
            plaintext_chunk_index: plaintext.chunk_index,
            plaintext_used_slots: plaintext.used_slots,
        }
    }
}

/// Secret encryption witness retained only long enough for a sibling proof
/// adapter to consume it.
///
/// This value intentionally implements neither `Clone` nor serialization.
/// Its only sibling-visible witness boundary validates the complete public
/// context and both RLWE equations before lending ephemeral references to the
/// proof builder. Public encryption drops it immediately.
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
            .field("input_identity", &self.input_identity)
            .field("ciphertext_digest", &hex::encode(self.ciphertext_digest))
            .field("canonical_plaintext", &"[REDACTED]")
            .field("plaintext_lift", &"[REDACTED]")
            .field("ephemeral", &"[REDACTED]")
            .field("error_zero", &"[REDACTED]")
            .field("error_one", &"[REDACTED]")
            .finish()
    }
}

impl Drop for ZkAmsMkheCollectiveEncryptionOpeningV1 {
    fn drop(&mut self) {
        for coefficient in &mut self.canonical_plaintext.0 {
            coefficient.fill(0);
        }
        self.plaintext_lift.0.coefficients.fill(0);
        self.ephemeral.coefficients.fill(0);
        self.error_zero.coefficients.fill(0);
        self.error_one.coefficients.fill(0);

        #[cfg(test)]
        if let Some(audit) = &self.drop_audit {
            let zeroized = self
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
/// The scalar reduction necessarily receives one by-value array copy. Rust
/// cannot guarantee erasure of that compiler-managed temporary, but this
/// owner covers the complete caller-visible buffer across errors and unwinds.
struct PersistentSecretCommitmentBlindingEntropyV1([u8; PERSISTENT_BLINDING_ENTROPY_BYTES_V1]);

impl PersistentSecretCommitmentBlindingEntropyV1 {
    const fn zeroed() -> Self {
        Self([0; PERSISTENT_BLINDING_ENTROPY_BYTES_V1])
    }

    fn as_mut_slice(&mut self) -> &mut [u8] {
        &mut self.0
    }

    fn expose_copy(&self) -> [u8; PERSISTENT_BLINDING_ENTROPY_BYTES_V1] {
        self.0
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
pub(super) struct PersistentSecretCommitmentBlindingsV1(
    [Scalar; ZK_AMS_MKHE_PERSISTENT_MEMBERSHIP_CHUNKS_V1],
);

impl PersistentSecretCommitmentBlindingsV1 {
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
                *blinding = Scalar::from_uniform_le_bytes(entropy.expose_copy());
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

    /// Borrow the exact ordered blindings for the sibling native CPK prover.
    ///
    /// No by-value or mutable access is exposed: the party state remains the
    /// sole owner until the complete relation is proven or the state drops.
    #[allow(dead_code)]
    pub(super) const fn as_array(&self) -> &[Scalar; ZK_AMS_MKHE_PERSISTENT_MEMBERSHIP_CHUNKS_V1] {
        &self.0
    }
}

impl core::fmt::Debug for PersistentSecretCommitmentBlindingsV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("PersistentSecretCommitmentBlindingsV1([REDACTED])")
    }
}

impl Drop for PersistentSecretCommitmentBlindingsV1 {
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
    assert!(core::mem::size_of::<PersistentSecretCommitmentBlindingsV1>() == 256);
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
    profile_digest: [u8; 32],
    security_certificate_digest: [u8; 32],
    roster_digest: [u8; 32],
    key_material_digest: [u8; 32],
    epoch: u64,
    transcript_digest: [u8; 32],
    party_index: u8,
    party: ZkAmsMkhePartyIdV1,
    public_share_digest: [u8; 32],
    persistent_secret_binding: Option<VerifiedPersistentWitnessBindingV1>,
    persistent_secret_commitment_blindings: PersistentSecretCommitmentBlindingsV1,
    secret: SecretPolynomial,
    public_error: SecretPolynomial,
}

impl core::fmt::Debug for ZkAmsMkheCollectivePartyStateV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("ZkAmsMkheCollectivePartyStateV1")
            .field("profile_digest", &hex::encode(self.profile_digest))
            .field(
                "security_certificate_digest",
                &hex::encode(self.security_certificate_digest),
            )
            .field("roster_digest", &hex::encode(self.roster_digest))
            .field("epoch", &self.epoch)
            .field("transcript_digest", &hex::encode(self.transcript_digest))
            .field("party_index", &self.party_index)
            .field("party", &self.party)
            .field(
                "public_share_digest",
                &hex::encode(self.public_share_digest),
            )
            .field(
                "persistent_secret_binding_verified",
                &self.persistent_secret_binding.is_some(),
            )
            .field(
                "persistent_secret_commitment_blindings",
                &self.persistent_secret_commitment_blindings,
            )
            .field("secret", &"[REDACTED]")
            .field("public_error", &"[REDACTED]")
            .finish()
    }
}

impl ZkAmsMkheCollectivePartyStateV1 {
    pub(super) const fn profile_digest(&self) -> [u8; 32] {
        self.profile_digest
    }

    pub(super) const fn roster_digest(&self) -> [u8; 32] {
        self.roster_digest
    }

    /// Authentication-key-derived governed party identifier.
    #[must_use]
    pub const fn party(&self) -> ZkAmsMkhePartyIdV1 {
        self.party
    }

    /// Exact zero-based position in the governed eight-party roster.
    #[must_use]
    pub const fn party_index(&self) -> u8 {
        self.party_index
    }

    /// Digest of the matching verified public share.
    #[must_use]
    pub const fn public_share_digest(&self) -> [u8; 32] {
        self.public_share_digest
    }

    /// Governed secret/key epoch.
    #[must_use]
    pub const fn epoch(&self) -> u64 {
        self.epoch
    }

    /// Exact collective-key transcript.
    #[must_use]
    pub const fn transcript_digest(&self) -> [u8; 32] {
        self.transcript_digest
    }

    pub(super) const fn secret(&self) -> &SecretPolynomial {
        &self.secret
    }

    pub(super) const fn public_error(&self) -> &SecretPolynomial {
        &self.public_error
    }

    /// Borrow the retained blindings for the complete sibling CPK relation.
    /// Absence of a connected consumer remains a release blocker; it is never
    /// permission to resample or accept an independently supplied opening.
    #[allow(dead_code)]
    pub(super) const fn persistent_secret_commitment_blindings(
        &self,
    ) -> &[Scalar; ZK_AMS_MKHE_PERSISTENT_MEMBERSHIP_CHUNKS_V1] {
        self.persistent_secret_commitment_blindings.as_array()
    }

    pub(super) const fn profile_digest_internal(&self) -> [u8; 32] {
        self.profile_digest
    }

    pub(super) const fn security_certificate_digest_internal(&self) -> [u8; 32] {
        self.security_certificate_digest
    }

    pub(super) const fn roster_digest_internal(&self) -> [u8; 32] {
        self.roster_digest
    }

    pub(super) const fn key_material_digest_internal(&self) -> [u8; 32] {
        self.key_material_digest
    }

    /// Validate the complete state/share axes and lend the exact state-owned
    /// persistent commitment opening to one sibling CPK operation.
    ///
    /// The narrowed coefficient copy is always erased before this call
    /// returns. This boundary does not accept caller-supplied openings or
    /// provide any resampling path.
    #[allow(dead_code)]
    pub(super) fn with_validated_cpk_secret_opening_v1<T>(
        &self,
        roster: &ZkAmsMkheGovernedActiveRosterV1,
        share: &ZkAmsMkheCollectivePublicKeyShareV1,
        adapter: impl FnOnce(
            &[i8],
            &[Scalar; ZK_AMS_MKHE_PERSISTENT_MEMBERSHIP_CHUNKS_V1],
        ) -> Result<T, ZkAmsMkheErrorV1>,
    ) -> Result<T, ZkAmsMkheErrorV1> {
        roster.validate()?;
        let party_index = usize::from(self.party_index);
        if self.profile_digest != roster.profile_digest()
            || self.roster_digest != roster.roster_digest()
            || self.key_material_digest != roster.key_material_digest()
            || self.epoch != roster.epoch()
            || party_index >= ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1
            || self.party != roster.participants()[party_index].party()
            || self.transcript_digest == [0; 32]
            || self.public_share_digest == [0; 32]
            || self.persistent_secret_binding.is_some()
            || share.profile_digest != self.profile_digest
            || share.security_certificate_digest != self.security_certificate_digest
            || share.roster_digest != self.roster_digest
            || share.key_material_digest != self.key_material_digest
            || share.epoch != self.epoch
            || share.transcript_digest != self.transcript_digest
            || share.party_index != self.party_index
            || share.party != self.party
            || share.digest != self.public_share_digest
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        validate_collective_public_key_share(roster, self.transcript_digest, party_index, share)?;
        let coefficients =
            ZeroizingT256MembershipCoefficientsV1::from_ternary_secret(&self.secret)?;
        adapter(
            coefficients.as_slice(),
            self.persistent_secret_commitment_blindings.as_array(),
        )
    }

    /// Admit and retain the sole persistent commitment capability for this
    /// state-owned share by consuming a complete native CPK relation.
    ///
    /// The relation is rebound to the exact canonical `b_i` payload here.
    /// Membership-only receipts, raw commitment digests, and a capability for
    /// another state/share cannot enter the persistent runtime graph.
    #[allow(dead_code)]
    pub(super) fn admit_verified_cpk_contribution(
        &mut self,
        roster: &ZkAmsMkheGovernedActiveRosterV1,
        share: &ZkAmsMkheCollectivePublicKeyShareV1,
        contribution: VerifiedZkAmsMkheCpkContributionV1,
    ) -> Result<&VerifiedPersistentWitnessBindingV1, ZkAmsMkheErrorV1> {
        let party_index = usize::from(self.party_index);
        let expected_commitments = self.with_validated_cpk_secret_opening_v1(
            roster,
            share,
            commit_persistent_secret_opening_v1,
        )?;
        let party_b_payload_blake3 = cpk_party_b_payload_blake3_v1(&share.party_public_b)?;
        let source = contribution
            .into_collective_binding_source(
                roster,
                self.transcript_digest,
                party_index,
                party_b_payload_blake3,
            )
            .map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?;
        ensure_state_owned_cpk_commitments_v1(source.commitments(), &expected_commitments)?;
        let binding = mint_collective_secret_binding_from_verified_cpk_v1(
            roster,
            self.transcript_digest,
            party_index,
            self.public_share_digest,
            source,
        )?;
        binding.validate_for(
            roster,
            self.transcript_digest,
            party_index,
            self.public_share_digest,
            PersistentWitnessConsumerV1::CollectivePublicKey,
        )?;
        self.persistent_secret_binding = Some(binding);
        let binding = self
            .persistent_secret_binding
            .as_ref()
            .ok_or(ZkAmsMkheErrorV1::ReleaseUnavailable)?;
        binding.validate_for(
            roster,
            self.transcript_digest,
            usize::from(self.party_index),
            self.public_share_digest,
            PersistentWitnessConsumerV1::CollectivePublicKey,
        )?;
        Ok(binding)
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
            .persistent_secret_binding
            .as_ref()
            .ok_or(ZkAmsMkheErrorV1::ReleaseUnavailable)?;
        binding.validate_for(
            roster,
            self.transcript_digest,
            usize::from(self.party_index),
            self.public_share_digest,
            consumer,
        )?;
        Ok(binding)
    }
}

/// One proof-carrying share of the exact eight-party collective public key.
#[derive(Clone, Debug, PartialEq, Eq)]
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
    pub fn public_a_wire(&self) -> Result<ZkAmsMkheRnsPolynomialWireV1, ZkAmsMkheErrorV1> {
        self.validate(&release_profile_v1())?;
        ZkAmsMkheRnsPolynomialWireV1::new(self.public_a.coefficients.clone())
    }

    /// Canonical release wire form of aggregate `b = sum_i b_i`.
    pub fn collective_public_b_wire(
        &self,
    ) -> Result<ZkAmsMkheRnsPolynomialWireV1, ZkAmsMkheErrorV1> {
        self.validate(&release_profile_v1())?;
        ZkAmsMkheRnsPolynomialWireV1::new(self.collective_public_b.coefficients.clone())
    }

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
        if self.public_a == RnsPolynomial::zero(profile)
            || self.collective_public_b == RnsPolynomial::zero(profile)
            || self.digest == [0; 32]
            || self.digest != collective_public_key_digest(self, profile)?
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        Ok(())
    }
}

/// Generate opaque RLWE state and its proof-carrying public-key share for one
/// exact governed roster position.
///
/// The caller supplies only the governed authentication secret and a
/// cryptographic random source. Raw RLWE secret/error arrays are neither an
/// input nor an output of this boundary.
pub fn generate_zk_ams_mkhe_collective_party_state_v1<R: MaskedRelaxedRandomSourceV1>(
    roster: &ZkAmsMkheGovernedActiveRosterV1,
    transcript_digest: [u8; 32],
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
    if transcript_digest == [0; 32]
        || party_index >= ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1
        || roster.participants()[party_index].party() != party_secret.party()?
    {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    let profile = release_profile_v1();
    profile.validate()?;
    let security_certificate_digest = release_security_certificate_digest()?;
    let public_a = zk_ams_mkhe_active_collective_public_a_v1(roster, transcript_digest)?;
    let public_a_native = RnsPolynomial::from_flat(&profile, public_a.residues().to_vec())?;
    let secret = sample_nonzero_ternary(&profile, random)?;
    let public_error = SecretPolynomial::sample_error(&profile, random)?;
    let secret_rns = ZeroizingRns(secret.as_rns(&profile)?);
    let scaled_error = scaled_public_error(&profile, &public_error)?;
    let party_public_b_native = public_a_native
        .mul(&secret_rns.0, &profile)?
        .negate(&profile)?
        .add(&scaled_error.0, &profile)?;
    let party_public_b = ZkAmsMkheRnsPolynomialWireV1::new(party_public_b_native.coefficients)?;
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
    };
    share.digest = collective_public_key_share_digest(&share)?;
    validate_collective_public_key_share(roster, transcript_digest, party_index, &share)?;
    let persistent_secret_commitment_blindings =
        PersistentSecretCommitmentBlindingsV1::sample(random)?;
    let state = ZkAmsMkheCollectivePartyStateV1 {
        profile_digest: share.profile_digest,
        security_certificate_digest,
        roster_digest: share.roster_digest,
        key_material_digest: share.key_material_digest,
        epoch: share.epoch,
        transcript_digest,
        party_index: share.party_index,
        party: share.party,
        public_share_digest: share.digest,
        persistent_secret_binding: None,
        persistent_secret_commitment_blindings,
        secret,
        public_error,
    };
    Ok((state, share))
}

/// Verify and aggregate exactly all eight ordered collective-public-key shares.
///
/// Missing, duplicate, reordered, cross-roster, cross-epoch, cross-transcript,
/// and proof-spliced shares are rejected before the aggregate key is returned.
pub fn aggregate_zk_ams_mkhe_collective_public_key_v1(
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
    let expected_public_a = zk_ams_mkhe_active_collective_public_a_v1(roster, transcript_digest)?;
    if shares
        .iter()
        .any(|share| share.public_a != expected_public_a)
    {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    checked_coefficient_work(&profile, ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1)?;
    let mut aggregate_b = RnsPolynomial::zero(&profile);
    for share in shares {
        let party_b = RnsPolynomial::from_flat(&profile, share.party_public_b.residues().to_vec())?;
        aggregate_b = aggregate_b.add(&party_b, &profile)?;
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
        collective_public_b: aggregate_b,
        share_digests: shares.map(ZkAmsMkheCollectivePublicKeyShareV1::digest),
        digest: [0; 32],
    };
    key.digest = collective_public_key_digest(&key, &profile)?;
    key.validate(&profile)?;
    Ok(key)
}

fn validate_collective_public_key_share(
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
    let expected_public_a = zk_ams_mkhe_active_collective_public_a_v1(roster, transcript_digest)?;
    if share.public_a != expected_public_a {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
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
fn cpk_party_b_payload_blake3_v1(
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
/// The transcript digest is the artifact-lineage commitment.  Constructors
/// for encryption and evaluation derive it from the frozen security
/// certificate, collective-key digest, exact input identity, and operation;
/// callers never provide a replacement lineage digest to those APIs.
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

impl ZkAmsMkheCollectiveEncryptionOpeningV1 {
    /// Validate the exact release encryption context and lend the opening to
    /// one sibling proof adapter for the duration of a single call.
    ///
    /// No witness reference is returned or stored outside the adapter call.
    /// The canonical T256 coefficients are included so a proof layer can
    /// derive an exact digit decomposition without reconstructing it from CRT
    /// residues. The fourth witness argument is the zeroizing effective error
    /// `e0' = e0 - 1[M > (t-1)/2]` matching the canonical natural lift; the
    /// sampled centered-lift `e0` never crosses this sibling boundary.
    #[allow(dead_code, clippy::type_complexity)]
    pub(super) fn with_validated_proof_witness_v1<T>(
        &self,
        key: &ZkAmsMkheCollectivePublicKeyV1,
        layout: ZkAmsT256PackingLayoutV1,
        plaintext: &ZkAmsT256PackedPlaintextV1,
        ciphertext: &ZkAmsMkheCollectiveCiphertextV1,
        adapter: impl FnOnce(
            &[[u8; 32]],
            &RnsPolynomial,
            &SecretPolynomial,
            &SecretPolynomial,
            &SecretPolynomial,
        ) -> Result<T, ZkAmsMkheErrorV1>,
    ) -> Result<T, ZkAmsMkheErrorV1> {
        let profile = release_profile_v1();
        key.validate(&profile)?;
        if key.security_certificate_digest != release_security_certificate_digest()?
            || ciphertext.sample_index
                >= zk_ams_mkhe_release_manifest_v1()?.max_samples_per_secret_epoch
            || layout.profile_digest != key.profile_digest
            || plaintext.profile_digest != key.profile_digest
        {
            return Err(ZkAmsMkheErrorV1::InvalidCiphertext);
        }
        let expected_message = ZeroizingRns(packed_plaintext_to_rns_v1(layout, plaintext)?);
        let input_identity = CollectiveEncryptionInputIdentityV1::from_packed(layout, plaintext);
        let expected_transcript_digest = collective_lineage_digest(
            COLLECTIVE_ENCRYPTION_DOMAIN_V1,
            key,
            &[],
            &[
                layout.digest.as_slice(),
                plaintext.digest.as_slice(),
                &plaintext.chunk_index.to_be_bytes(),
                &ciphertext.sample_index.to_be_bytes(),
            ],
        );
        self.validate_against(
            &profile,
            key,
            &expected_message.0,
            &plaintext.coefficients,
            input_identity,
            expected_transcript_digest,
            ciphertext,
        )?;
        let effective_error_zero = derive_natural_lift_effective_error_zero(
            &profile,
            &self.canonical_plaintext.0,
            &self.error_zero,
        )?;
        adapter(
            &self.canonical_plaintext.0,
            &self.plaintext_lift.0,
            &self.ephemeral,
            &effective_error_zero,
            &self.error_one,
        )
    }

    #[allow(clippy::too_many_arguments, clippy::type_complexity)]
    #[cfg(test)]
    fn with_validated_native_proof_witness_v1<T>(
        &self,
        profile: &BgvProfile,
        key: &ZkAmsMkheCollectivePublicKeyV1,
        expected_message: &RnsPolynomial,
        expected_canonical_plaintext: &[[u8; 32]],
        input_identity: CollectiveEncryptionInputIdentityV1,
        expected_transcript_digest: [u8; 32],
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
            input_identity,
            expected_transcript_digest,
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
        input_identity: CollectiveEncryptionInputIdentityV1,
        expected_transcript_digest: [u8; 32],
        ciphertext: &ZkAmsMkheCollectiveCiphertextV1,
    ) -> Result<(), ZkAmsMkheErrorV1> {
        profile.validate()?;
        key.validate(profile)?;
        expected_message.validate(profile)?;
        validate_compact_for_key(ciphertext, key, profile)?;
        if expected_transcript_digest == [0; 32]
            || input_identity.layout_digest == [0; 32]
            || input_identity.plaintext_digest == [0; 32]
            || input_identity.plaintext_used_slots == 0
            || usize::try_from(input_identity.plaintext_used_slots)
                .map_or(true, |used_slots| used_slots > profile.ring_degree)
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
            || self.input_identity != input_identity
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
    let input_identity = CollectiveEncryptionInputIdentityV1::from_packed(layout, plaintext);
    let transcript_digest = collective_lineage_digest(
        COLLECTIVE_ENCRYPTION_DOMAIN_V1,
        key,
        &[],
        &[
            layout.digest.as_slice(),
            plaintext.digest.as_slice(),
            &plaintext.chunk_index.to_be_bytes(),
            &sample_index.to_be_bytes(),
        ],
    );
    encrypt_collective_native_with_opening(
        &profile,
        key,
        message,
        canonical_plaintext,
        input_identity,
        transcript_digest,
        sample_index,
        random,
    )
}

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

    #[allow(
        dead_code,
        reason = "used by the private fail-closed collective evaluated-key runtime"
    )]
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
fn encrypt_collective_native_with_opening<R: MaskedRelaxedRandomSourceV1>(
    profile: &BgvProfile,
    key: &ZkAmsMkheCollectivePublicKeyV1,
    message: ZeroizingRns,
    canonical_plaintext: ZeroizingCanonicalPlaintext,
    input_identity: CollectiveEncryptionInputIdentityV1,
    transcript_digest: [u8; 32],
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
    if transcript_digest == [0; 32]
        || input_identity.layout_digest == [0; 32]
        || input_identity.plaintext_digest == [0; 32]
        || input_identity.plaintext_used_slots == 0
        || usize::try_from(input_identity.plaintext_used_slots)
            .map_or(true, |used_slots| used_slots > profile.ring_degree)
        || canonical_plaintext.0.len() != profile.ring_degree
    {
        return Err(ZkAmsMkheErrorV1::InvalidCiphertext);
    }
    checked_ring_multiplication_work(profile, 2)?;
    validate_collective_encryption_entropy(random)?;
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
        input_identity,
        transcript_digest,
        &ciphertext,
    )?;
    Ok((ciphertext, opening))
}

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

fn scaled_public_error(
    profile: &BgvProfile,
    error: &SecretPolynomial,
) -> Result<ZeroizingRns, ZkAmsMkheErrorV1> {
    let raw = ZeroizingRns(error.as_rns(profile)?);
    Ok(ZeroizingRns(raw.0.scale_plaintext_modulus(profile)?))
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

fn validate_collective_encryption_entropy<R: MaskedRelaxedRandomSourceV1>(
    random: &mut R,
) -> Result<(), ZkAmsMkheErrorV1> {
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
    Ok(())
}

fn entropy_probe_has_short_period(probe: &[u8; 32]) -> bool {
    (1..=probe.len() / 2).any(|period| {
        probe.len().is_multiple_of(period)
            && probe
                .iter()
                .enumerate()
                .all(|(index, byte)| *byte == probe[index % period])
    })
}

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
mod tests {
    use super::*;
    use crate::vega::{MaskedRelaxedRandomErrorV1, sponge::shake256};

    const TEST_MODULI: [u64; 2] = [2_013_265_921, 1_811_939_329];
    const TEST_ROOTS: [u64; 2] = [1_400_279_418, 677_356_115];

    fn test_profile() -> BgvProfile {
        BgvProfile {
            profile_id: [0x61; 32],
            ring_degree: 8,
            moduli: &TEST_MODULI,
            negacyclic_roots: &TEST_ROOTS,
            plaintext_modulus: super::super::PlaintextModulus::Tiny(17),
            error_eta: 2,
            hybrid_rns_decomposition: false,
            gadget_base_log: 8,
            gadget_digits: 8,
            max_ciphertext_bytes: 1 << 20,
            max_evaluated_key_bytes: 16 << 20,
            max_round_bytes: 16 << 20,
            max_share_bytes: 4 << 20,
            max_workspace_bytes: 16 << 20,
            max_work_units: 1 << 20,
        }
    }

    struct KatRandom {
        state: [u8; 32],
        counter: u64,
    }

    impl KatRandom {
        fn new(label: &[u8]) -> Self {
            Self {
                state: keccak256(label),
                counter: 0,
            }
        }
    }

    impl MaskedRelaxedRandomSourceV1 for KatRandom {
        fn fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), MaskedRelaxedRandomErrorV1> {
            let mut written = 0;
            while written < destination.len() {
                let mut frame = Vec::with_capacity(40);
                frame.extend_from_slice(&self.state);
                frame.extend_from_slice(&self.counter.to_be_bytes());
                let block = shake256(&frame, 64);
                let take = (destination.len() - written).min(block.len());
                destination[written..written + take].copy_from_slice(&block[..take]);
                self.state = keccak256(&block);
                self.counter = self.counter.wrapping_add(1);
                written += take;
            }
            Ok(())
        }
    }

    struct ConstantRandom(u8);

    impl MaskedRelaxedRandomSourceV1 for ConstantRandom {
        fn fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), MaskedRelaxedRandomErrorV1> {
            destination.fill(self.0);
            Ok(())
        }
    }

    struct FailingRandom;

    impl MaskedRelaxedRandomSourceV1 for FailingRandom {
        fn fill_bytes(
            &mut self,
            _destination: &mut [u8],
        ) -> Result<(), MaskedRelaxedRandomErrorV1> {
            Err(MaskedRelaxedRandomErrorV1::Unavailable)
        }
    }

    fn persistent_blinding_uniform_block(value: u64) -> [u8; 64] {
        let mut block = [0; 64];
        block[..8].copy_from_slice(&value.to_le_bytes());
        block
    }

    struct ScriptedPersistentBlindingRandom {
        blocks: Vec<[u8; 64]>,
        next: usize,
        request_lengths: Vec<usize>,
    }

    impl ScriptedPersistentBlindingRandom {
        fn from_scalars(values: impl IntoIterator<Item = u64>) -> Self {
            Self {
                blocks: values
                    .into_iter()
                    .map(persistent_blinding_uniform_block)
                    .collect(),
                next: 0,
                request_lengths: Vec::new(),
            }
        }
    }

    impl MaskedRelaxedRandomSourceV1 for ScriptedPersistentBlindingRandom {
        fn fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), MaskedRelaxedRandomErrorV1> {
            self.request_lengths.push(destination.len());
            let Some(block) = self.blocks.get(self.next) else {
                return Err(MaskedRelaxedRandomErrorV1::Unavailable);
            };
            self.next += 1;
            if destination.len() != block.len() {
                return Err(MaskedRelaxedRandomErrorV1::Unavailable);
            }
            destination.copy_from_slice(block);
            Ok(())
        }
    }

    struct PartialFailurePersistentBlindingRandom {
        successful_requests: usize,
        calls: usize,
        partial_bytes: usize,
    }

    impl MaskedRelaxedRandomSourceV1 for PartialFailurePersistentBlindingRandom {
        fn fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), MaskedRelaxedRandomErrorV1> {
            assert_eq!(destination.len(), PERSISTENT_BLINDING_ENTROPY_BYTES_V1);
            let call = self.calls;
            self.calls += 1;
            if call < self.successful_requests {
                destination.copy_from_slice(&persistent_blinding_uniform_block(
                    u64::try_from(call + 1).expect("test request index fits u64"),
                ));
                return Ok(());
            }
            destination[..self.partial_bytes].fill(0xa5);
            Err(MaskedRelaxedRandomErrorV1::Unavailable)
        }
    }

    struct PartialPanicPersistentBlindingRandom {
        successful_requests: usize,
        calls: usize,
        partial_bytes: usize,
    }

    impl MaskedRelaxedRandomSourceV1 for PartialPanicPersistentBlindingRandom {
        fn fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), MaskedRelaxedRandomErrorV1> {
            assert_eq!(destination.len(), PERSISTENT_BLINDING_ENTROPY_BYTES_V1);
            let call = self.calls;
            self.calls += 1;
            if call < self.successful_requests {
                destination.copy_from_slice(&persistent_blinding_uniform_block(
                    u64::try_from(call + 1).expect("test request index fits u64"),
                ));
                return Ok(());
            }
            destination[..self.partial_bytes].fill(0x5a);
            panic!("injected persistent-blinding entropy panic");
        }
    }

    fn reset_persistent_blinding_drop_audits() {
        PERSISTENT_BLINDING_ENTROPY_ZEROIZED_DROPS_V1.with(|drops| drops.set(0));
        PERSISTENT_BLINDING_OWNER_ZEROIZED_DROPS_V1.with(|drops| drops.set(0));
    }

    fn persistent_blinding_drop_audits() -> (usize, usize) {
        let entropy = PERSISTENT_BLINDING_ENTROPY_ZEROIZED_DROPS_V1.with(std::cell::Cell::get);
        let owner = PERSISTENT_BLINDING_OWNER_ZEROIZED_DROPS_V1.with(std::cell::Cell::get);
        (entropy, owner)
    }

    fn test_persistent_secret_commitment_blindings() -> PersistentSecretCommitmentBlindingsV1 {
        PersistentSecretCommitmentBlindingsV1(core::array::from_fn(|index| {
            Scalar::from_u64(u64::try_from(index + 17).expect("test blinding index fits u64"))
        }))
    }

    struct RepeatedHealthyBlockRandom;

    impl MaskedRelaxedRandomSourceV1 for RepeatedHealthyBlockRandom {
        fn fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), MaskedRelaxedRandomErrorV1> {
            for (index, byte) in destination.iter_mut().enumerate() {
                *byte = u8::try_from(index).unwrap_or(0).wrapping_mul(29) ^ 0xa7;
            }
            Ok(())
        }
    }

    struct ProbeThenConstantRandom {
        calls: usize,
    }

    impl ProbeThenConstantRandom {
        const fn new() -> Self {
            Self { calls: 0 }
        }
    }

    impl MaskedRelaxedRandomSourceV1 for ProbeThenConstantRandom {
        fn fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), MaskedRelaxedRandomErrorV1> {
            match self.calls {
                0 | 1 => {
                    let domain = if self.calls == 0 { 0x39 } else { 0xd2 };
                    for (index, byte) in destination.iter_mut().enumerate() {
                        *byte = u8::try_from(index)
                            .unwrap_or(0)
                            .wrapping_mul(37)
                            .wrapping_add(domain)
                            ^ u8::try_from(index * index).unwrap_or(0);
                    }
                }
                _ => destination.fill(0x55),
            }
            self.calls += 1;
            Ok(())
        }
    }

    fn test_parties() -> super::super::PartySet {
        super::super::PartySet::new(
            (1_u8..=ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1 as u8)
                .map(|tag| {
                    let mut bytes = [0_u8; 32];
                    bytes[31] = tag;
                    ZkAmsMkhePartyIdV1::new(bytes).unwrap()
                })
                .collect(),
        )
        .unwrap()
    }

    fn test_key(label: u8) -> (ZkAmsMkheCollectivePublicKeyV1, SecretPolynomial) {
        let profile = test_profile();
        profile.validate().unwrap();
        let parties = test_parties();
        let aggregate_secret = SecretPolynomial {
            coefficients: vec![8, 0, 0, 0, 0, 0, 0, 0],
        };
        let public_a = RnsPolynomial::from_unsigned(&profile, &[1, 2, 3, 4, 5, 6, 7, 8]).unwrap();
        let collective_public_b = public_a
            .mul(&aggregate_secret.as_rns(&profile).unwrap(), &profile)
            .unwrap()
            .negate(&profile)
            .unwrap();
        let epoch = 19;
        let mut key = ZkAmsMkheCollectivePublicKeyV1 {
            version: MKHE_VERSION_V1,
            profile_digest: profile.digest().unwrap(),
            security_certificate_digest: [0x22; 32],
            roster_digest: governed_roster_digest(
                profile.digest().unwrap(),
                epoch,
                &parties.parties,
            ),
            key_material_digest: [label; 32],
            epoch,
            transcript_digest: [label.wrapping_add(1); 32],
            parties,
            public_a,
            collective_public_b,
            share_digests: core::array::from_fn(|index| [index as u8 + 1; 32]),
            digest: [0; 32],
        };
        key.digest = collective_public_key_digest(&key, &profile).unwrap();
        key.validate(&profile).unwrap();
        (key, aggregate_secret)
    }

    fn test_canonical_plaintext(values: &[u64; 8]) -> Vec<[u8; 32]> {
        values
            .iter()
            .map(|value| {
                let mut coefficient = [0; 32];
                coefficient[24..].copy_from_slice(&value.to_be_bytes());
                coefficient
            })
            .collect()
    }

    fn test_input_identity(
        profile: &BgvProfile,
        message: &RnsPolynomial,
        label: &[u8],
    ) -> CollectiveEncryptionInputIdentityV1 {
        let mut plaintext_hash = Keccak256::new();
        plaintext_hash.update(b"iroha.zk-ams.test.collective-opening-plaintext");
        update_rns_hash(&mut plaintext_hash, profile, message).unwrap();
        CollectiveEncryptionInputIdentityV1 {
            layout_digest: keccak256(&[b"layout".as_slice(), label].concat()),
            plaintext_digest: plaintext_hash.finalize(),
            plaintext_chunk_index: 0,
            plaintext_used_slots: u32::try_from(profile.ring_degree).unwrap(),
        }
    }

    fn encrypt_test_with_opening(
        profile: &BgvProfile,
        key: &ZkAmsMkheCollectivePublicKeyV1,
        values: &[u64; 8],
        sample_index: u64,
        label: &[u8],
    ) -> (
        ZkAmsMkheCollectiveCiphertextV1,
        ZkAmsMkheCollectiveEncryptionOpeningV1,
        RnsPolynomial,
        Vec<[u8; 32]>,
        CollectiveEncryptionInputIdentityV1,
        [u8; 32],
    ) {
        let message = RnsPolynomial::from_test_plaintext(profile, values).unwrap();
        let canonical_plaintext = test_canonical_plaintext(values);
        let input_identity = test_input_identity(profile, &message, label);
        let transcript_digest = keccak256(label);
        let (ciphertext, opening) = encrypt_collective_native_with_opening(
            profile,
            key,
            ZeroizingRns(message.clone()),
            ZeroizingCanonicalPlaintext(canonical_plaintext.clone()),
            input_identity,
            transcript_digest,
            sample_index,
            &mut KatRandom::new(label),
        )
        .unwrap();
        (
            ciphertext,
            opening,
            message,
            canonical_plaintext,
            input_identity,
            transcript_digest,
        )
    }

    fn try_encrypt_test_with_random<R: MaskedRelaxedRandomSourceV1>(
        profile: &BgvProfile,
        key: &ZkAmsMkheCollectivePublicKeyV1,
        values: &[u64; 8],
        sample_index: u64,
        label: &[u8],
        random: &mut R,
    ) -> Result<
        (
            ZkAmsMkheCollectiveCiphertextV1,
            ZkAmsMkheCollectiveEncryptionOpeningV1,
        ),
        ZkAmsMkheErrorV1,
    > {
        let message = RnsPolynomial::from_test_plaintext(profile, values).unwrap();
        let canonical_plaintext = test_canonical_plaintext(values);
        let input_identity = test_input_identity(profile, &message, label);
        encrypt_collective_native_with_opening(
            profile,
            key,
            ZeroizingRns(message),
            ZeroizingCanonicalPlaintext(canonical_plaintext),
            input_identity,
            keccak256(label),
            sample_index,
            random,
        )
    }

    fn encrypt_test(
        profile: &BgvProfile,
        key: &ZkAmsMkheCollectivePublicKeyV1,
        values: &[u64; 8],
        sample_index: u64,
        label: &[u8],
    ) -> ZkAmsMkheCollectiveCiphertextV1 {
        let (ciphertext, opening, ..) =
            encrypt_test_with_opening(profile, key, values, sample_index, label);
        drop(opening);
        ciphertext
    }

    fn decrypt_compact(
        profile: &BgvProfile,
        ciphertext: &ZkAmsMkheCollectiveCiphertextV1,
        secret: &SecretPolynomial,
    ) -> Vec<u64> {
        let value = ciphertext
            .constant
            .add(
                &ciphertext
                    .linear
                    .mul(&secret.as_rns(profile).unwrap(), profile)
                    .unwrap(),
                profile,
            )
            .unwrap();
        super::super::reduce_test_polynomial(profile, &value).unwrap()
    }

    fn decrypt_level_one(
        profile: &BgvProfile,
        ciphertext: &ZkAmsMkheCollectiveLevelOneV1,
        secret: &SecretPolynomial,
    ) -> Vec<u64> {
        let secret = secret.as_rns(profile).unwrap();
        let secret_square = secret.mul(&secret, profile).unwrap();
        let value = ciphertext
            .constant
            .add(&ciphertext.linear.mul(&secret, profile).unwrap(), profile)
            .unwrap()
            .add(
                &ciphertext.quadratic.mul(&secret_square, profile).unwrap(),
                profile,
            )
            .unwrap();
        super::super::reduce_test_polynomial(profile, &value).unwrap()
    }

    fn negacyclic_plaintext_product(left: &[u64; 8], right: &[u64; 8]) -> Vec<u64> {
        let mut output = [0_i128; 8];
        for (left_index, left_value) in left.iter().copied().enumerate() {
            for (right_index, right_value) in right.iter().copied().enumerate() {
                let product = i128::from(left_value) * i128::from(right_value);
                let index = left_index + right_index;
                if index < 8 {
                    output[index] += product;
                } else {
                    output[index - 8] -= product;
                }
            }
        }
        output
            .into_iter()
            .map(|value| value.rem_euclid(17) as u64)
            .collect()
    }

    #[test]
    fn tiny_collective_algebra_matches_plaintext_oracle() {
        let profile = test_profile();
        let (key, secret) = test_key(0x31);
        let left_values = [1, 2, 3, 4, 5, 6, 7, 8];
        let right_values = [8, 0, 2, 0, 4, 0, 6, 0];
        let left = encrypt_test(&profile, &key, &left_values, 11, b"collective-left");
        let right = encrypt_test(&profile, &key, &right_values, 17, b"collective-right");
        assert_eq!(decrypt_compact(&profile, &left, &secret), left_values);
        assert_eq!(decrypt_compact(&profile, &right, &secret), right_values);

        let sum = compact_binary_with_profile(
            &profile,
            &left,
            &key,
            &right,
            COLLECTIVE_ADD_DOMAIN_V1,
            RnsPolynomial::add,
        )
        .unwrap();
        assert_eq!(
            decrypt_compact(&profile, &sum, &secret),
            left_values
                .iter()
                .zip(right_values)
                .map(|(left, right)| (*left + right) % 17)
                .collect::<Vec<_>>()
        );
        assert_eq!(sum.sample_index(), 11);
        assert_ne!(sum.transcript_digest(), left.transcript_digest());

        let difference = compact_binary_with_profile(
            &profile,
            &left,
            &key,
            &right,
            COLLECTIVE_SUB_DOMAIN_V1,
            RnsPolynomial::sub,
        )
        .unwrap();
        assert_eq!(
            decrypt_compact(&profile, &difference, &secret),
            left_values
                .iter()
                .zip(right_values)
                .map(|(left, right)| (17 + *left - right) % 17)
                .collect::<Vec<_>>()
        );
        assert_ne!(sum.digest(), difference.digest());

        let expected_product = negacyclic_plaintext_product(&left_values, &right_values);
        let product = multiply_with_profile(&profile, &left, &key, &right).unwrap();
        assert_eq!(
            decrypt_level_one(&profile, &product, &secret),
            expected_product
        );
        assert_eq!(product.evaluation_key_digest(), key.digest());

        let plaintext_multiplier =
            RnsPolynomial::from_test_plaintext(&profile, &right_values).unwrap();
        let scaled = compact_plaintext_mul_with_profile(
            &profile,
            &left,
            &key,
            &plaintext_multiplier,
            &[b"canonical-test-plaintext"],
        )
        .unwrap();
        assert_eq!(
            decrypt_compact(&profile, &scaled, &secret),
            expected_product
        );

        let doubled_product = level_one_binary_with_profile(
            &profile,
            &product,
            &key,
            &product,
            COLLECTIVE_ADD_DOMAIN_V1,
            RnsPolynomial::add,
        )
        .unwrap();
        assert_eq!(
            decrypt_level_one(&profile, &doubled_product, &secret),
            expected_product
                .iter()
                .map(|value| (2 * value) % 17)
                .collect::<Vec<_>>()
        );
        let zero_product = level_one_binary_with_profile(
            &profile,
            &product,
            &key,
            &product,
            COLLECTIVE_SUB_DOMAIN_V1,
            RnsPolynomial::sub,
        )
        .unwrap();
        assert_eq!(
            decrypt_level_one(&profile, &zero_product, &secret),
            vec![0; 8]
        );

        let scaled_product = level_one_plaintext_mul_with_profile(
            &profile,
            &product,
            &key,
            &plaintext_multiplier,
            &[b"canonical-level-one-test-plaintext"],
        )
        .unwrap();
        let expected_product_array: [u64; 8] = expected_product.clone().try_into().unwrap();
        assert_eq!(
            decrypt_level_one(&profile, &scaled_product, &secret),
            negacyclic_plaintext_product(&expected_product_array, &right_values)
        );

        let transformed_product =
            level_one_automorphism_with_profile(&profile, &product, &key, 3, 3_u64.to_be_bytes())
                .unwrap();
        let transformed_secret = secret.automorphism(3, &profile).unwrap();
        let expected_transformed =
            RnsPolynomial::from_test_plaintext(&profile, &expected_product_array)
                .unwrap()
                .automorphism(3, &profile)
                .unwrap();
        assert_eq!(
            decrypt_level_one(&profile, &transformed_product, &transformed_secret),
            super::super::reduce_test_polynomial(&profile, &expected_transformed).unwrap()
        );
    }

    #[test]
    fn raw_automorphism_moves_to_exact_automorphed_key_domain() {
        let profile = test_profile();
        let (key, secret) = test_key(0x41);
        let values = [1, 2, 4, 8, 3, 6, 12, 7];
        let ciphertext = encrypt_test(&profile, &key, &values, 5, b"collective-auto");
        let exponent = 3;
        let transformed = compact_automorphism_with_profile(
            &profile,
            &ciphertext,
            &key,
            exponent,
            (exponent as u64).to_be_bytes(),
        )
        .unwrap();
        assert_ne!(transformed.evaluation_key_digest(), Some(key.digest()));
        assert_eq!(
            validate_compact_for_key(&transformed, &key, &profile),
            Err(ZkAmsMkheErrorV1::InvalidCiphertext)
        );
        let transformed_secret = secret.automorphism(exponent, &profile).unwrap();
        let expected = RnsPolynomial::from_test_plaintext(&profile, &values)
            .unwrap()
            .automorphism(exponent, &profile)
            .unwrap();
        assert_eq!(
            decrypt_compact(&profile, &transformed, &transformed_secret),
            super::super::reduce_test_polynomial(&profile, &expected).unwrap()
        );
        for invalid in [0, 2, 16, usize::MAX] {
            assert!(
                compact_automorphism_with_profile(
                    &profile,
                    &ciphertext,
                    &key,
                    invalid,
                    u64::try_from(invalid).unwrap_or(u64::MAX).to_be_bytes(),
                )
                .is_err()
            );
        }
    }

    #[test]
    fn cross_key_unbound_and_tampered_ciphertexts_fail_closed() {
        let profile = test_profile();
        let (key, _) = test_key(0x51);
        let (other_key, _) = test_key(0x52);
        let values = [1, 0, 0, 0, 0, 0, 0, 0];
        let ciphertext = encrypt_test(&profile, &key, &values, 3, b"collective-binding");
        assert_eq!(
            compact_binary_with_profile(
                &profile,
                &ciphertext,
                &other_key,
                &ciphertext,
                COLLECTIVE_ADD_DOMAIN_V1,
                RnsPolynomial::add,
            ),
            Err(ZkAmsMkheErrorV1::InvalidCiphertext)
        );

        let unbound = ZkAmsMkheCollectiveCiphertextV1::new(
            &profile,
            &key.parties,
            key.epoch,
            [0x71; 32],
            3,
            0,
            ciphertext.constant.clone(),
            ciphertext.linear.clone(),
        )
        .unwrap();
        assert_eq!(
            compact_binary_with_profile(
                &profile,
                &unbound,
                &key,
                &ciphertext,
                COLLECTIVE_ADD_DOMAIN_V1,
                RnsPolynomial::add,
            ),
            Err(ZkAmsMkheErrorV1::InvalidCiphertext)
        );

        for axis in 0..4 {
            let mut tampered = ciphertext.clone();
            match axis {
                0 => tampered.profile_digest[0] ^= 1,
                1 => tampered.roster_digest[0] ^= 1,
                2 => tampered.epoch ^= 1,
                _ => tampered.transcript_digest[0] ^= 1,
            }
            assert!(tampered.validate(&profile, &key.parties).is_err());
        }
        let mut tampered_component = ciphertext.clone();
        tampered_component.constant.coefficients[0] = TEST_MODULI[0];
        assert_eq!(
            tampered_component.validate(&profile, &key.parties),
            Err(ZkAmsMkheErrorV1::InvalidPolynomial)
        );
    }

    #[test]
    fn level_one_component_and_digest_tampering_is_rejected() {
        let profile = test_profile();
        let (key, _) = test_key(0x61);
        let values = [1, 2, 3, 4, 5, 6, 7, 8];
        let left = encrypt_test(&profile, &key, &values, 1, b"level-one-left");
        let right = encrypt_test(&profile, &key, &values, 2, b"level-one-right");
        let mut product = multiply_with_profile(&profile, &left, &key, &right).unwrap();
        product.quadratic.coefficients[0] ^= 1;
        assert_eq!(
            product.validate(&profile),
            Err(ZkAmsMkheErrorV1::InvalidCiphertext)
        );
        let mut product = multiply_with_profile(&profile, &left, &key, &right).unwrap();
        product.evaluation_key_digest[0] ^= 1;
        assert_eq!(
            product.validate(&profile),
            Err(ZkAmsMkheErrorV1::InvalidCiphertext)
        );
    }

    #[test]
    fn deterministic_zero_ternary_rng_exhausts_without_emitting_secret_or_ciphertext() {
        let profile = test_profile();
        let mut zero_ternary = ConstantRandom(0x55);
        assert!(matches!(
            sample_nonzero_ternary(&profile, &mut zero_ternary),
            Err(ZkAmsMkheErrorV1::RandomUnavailable)
        ));

        let (key, _) = test_key(0x71);
        // The two initial probes are distinct and non-periodic; all subsequent
        // ternary bytes encode zero, so bounded rejection must still stop.
        let mut zero_ternary = ProbeThenConstantRandom::new();
        assert!(matches!(
            try_encrypt_test_with_random(
                &profile,
                &key,
                &[0; 8],
                0,
                b"all-zero-r",
                &mut zero_ternary,
            ),
            Err(ZkAmsMkheErrorV1::RandomUnavailable)
        ));
    }

    #[test]
    fn collective_opening_adapter_recomputes_both_rlwe_equations_independently() {
        let profile = test_profile();
        let (key, _) = test_key(0x72);
        let values = [1, 16, 3, 14, 5, 12, 7, 10];
        let (ciphertext, opening, message, canonical, input_identity, transcript_digest) =
            encrypt_test_with_opening(&profile, &key, &values, 29, b"opening-equations");

        opening
            .with_validated_native_proof_witness_v1(
                &profile,
                &key,
                &message,
                &canonical,
                input_identity,
                transcript_digest,
                &ciphertext,
                |actual_canonical, actual_message, ephemeral, error_zero, error_one| {
                    assert_eq!(actual_canonical, canonical.as_slice());
                    assert_eq!(actual_message, &message);
                    assert!(
                        ephemeral
                            .coefficients
                            .iter()
                            .any(|coefficient| *coefficient != 0)
                    );
                    assert!(bounded_error_polynomial(&profile, error_zero));
                    assert!(bounded_error_polynomial(&profile, error_one));

                    // Recompute from the raw validated witnesses rather than
                    // relying on the opening's validation result.
                    let ephemeral_rns = ZeroizingRns(ephemeral.as_rns(&profile)?);
                    let error_zero_rns = ZeroizingRns(error_zero.as_rns(&profile)?);
                    let error_one_rns = ZeroizingRns(error_one.as_rns(&profile)?);
                    let scaled_error_zero =
                        ZeroizingRns(error_zero_rns.0.scale_plaintext_modulus(&profile)?);
                    let scaled_error_one =
                        ZeroizingRns(error_one_rns.0.scale_plaintext_modulus(&profile)?);
                    let public_b_product =
                        ZeroizingRns(key.collective_public_b.mul(&ephemeral_rns.0, &profile)?);
                    let constant_with_error =
                        ZeroizingRns(public_b_product.0.add(&scaled_error_zero.0, &profile)?);
                    let independently_recomputed_constant =
                        ZeroizingRns(constant_with_error.0.add(actual_message, &profile)?);
                    let public_a_product =
                        ZeroizingRns(key.public_a.mul(&ephemeral_rns.0, &profile)?);
                    let independently_recomputed_linear =
                        ZeroizingRns(public_a_product.0.add(&scaled_error_one.0, &profile)?);
                    assert_eq!(independently_recomputed_constant.0, *ciphertext.constant());
                    assert_eq!(independently_recomputed_linear.0, *ciphertext.linear());
                    Ok(())
                },
            )
            .unwrap();
    }

    #[test]
    fn collective_opening_rejects_key_ciphertext_message_and_context_splices() {
        let profile = test_profile();
        let (key, _) = test_key(0x73);
        let (other_key, _) = test_key(0x74);
        let values = [2, 4, 6, 8, 10, 12, 14, 16];
        let (ciphertext, opening, message, canonical, identity, transcript_digest) =
            encrypt_test_with_opening(&profile, &key, &values, 31, b"opening-splices");

        assert!(
            opening
                .validate_against(
                    &profile,
                    &other_key,
                    &message,
                    &canonical,
                    identity,
                    transcript_digest,
                    &ciphertext,
                )
                .is_err()
        );

        let other_ciphertext = encrypt_test(
            &profile,
            &key,
            &[1, 3, 5, 7, 9, 11, 13, 15],
            32,
            b"opening-other-ciphertext",
        );
        assert!(
            opening
                .validate_against(
                    &profile,
                    &key,
                    &message,
                    &canonical,
                    identity,
                    other_ciphertext.transcript_digest,
                    &other_ciphertext,
                )
                .is_err()
        );

        let wrong_message =
            RnsPolynomial::from_test_plaintext(&profile, &[3, 4, 6, 8, 10, 12, 14, 16]).unwrap();
        assert!(
            opening
                .validate_against(
                    &profile,
                    &key,
                    &wrong_message,
                    &canonical,
                    identity,
                    transcript_digest,
                    &ciphertext,
                )
                .is_err()
        );

        let mut wrong_canonical = canonical.clone();
        wrong_canonical[0][31] ^= 1;
        assert!(
            opening
                .validate_against(
                    &profile,
                    &key,
                    &message,
                    &wrong_canonical,
                    identity,
                    transcript_digest,
                    &ciphertext,
                )
                .is_err()
        );

        for axis in 0..4 {
            let mut wrong_identity = identity;
            match axis {
                0 => wrong_identity.layout_digest[0] ^= 1,
                1 => wrong_identity.plaintext_digest[0] ^= 1,
                2 => wrong_identity.plaintext_chunk_index ^= 1,
                _ => wrong_identity.plaintext_used_slots -= 1,
            }
            assert!(
                opening
                    .validate_against(
                        &profile,
                        &key,
                        &message,
                        &canonical,
                        wrong_identity,
                        transcript_digest,
                        &ciphertext,
                    )
                    .is_err(),
                "context splice axis {axis} was accepted"
            );
        }

        let mut wrong_transcript = transcript_digest;
        wrong_transcript[0] ^= 1;
        assert!(
            opening
                .validate_against(
                    &profile,
                    &key,
                    &message,
                    &canonical,
                    identity,
                    wrong_transcript,
                    &ciphertext,
                )
                .is_err()
        );
        let mut corrupted_ciphertext = ciphertext.clone();
        corrupted_ciphertext.digest[0] ^= 1;
        assert!(
            opening
                .validate_against(
                    &profile,
                    &key,
                    &message,
                    &canonical,
                    identity,
                    transcript_digest,
                    &corrupted_ciphertext,
                )
                .is_err()
        );
    }

    #[test]
    fn collective_opening_rejects_out_of_range_or_tampered_secret_witnesses() {
        let profile = test_profile();
        let (key, _) = test_key(0x75);
        let values = [1, 2, 3, 4, 5, 6, 7, 8];

        for axis in 0..4 {
            let (ciphertext, mut opening, message, canonical, identity, transcript_digest) =
                encrypt_test_with_opening(
                    &profile,
                    &key,
                    &values,
                    40 + axis,
                    &[b"opening-witness-tamper".as_slice(), &[axis as u8]].concat(),
                );
            match axis {
                0 => opening.ephemeral.coefficients.fill(0),
                1 => opening.ephemeral.coefficients[0] = 2,
                2 => {
                    opening.error_zero.coefficients[0] = i64::from(profile.error_eta) + 1;
                }
                _ => opening.error_one.coefficients[0] ^= 1,
            }
            assert!(
                opening
                    .validate_against(
                        &profile,
                        &key,
                        &message,
                        &canonical,
                        identity,
                        transcript_digest,
                        &ciphertext,
                    )
                    .is_err(),
                "secret witness tamper axis {axis} was accepted"
            );
        }
    }

    #[test]
    fn collective_encryption_rejects_zero_repeating_and_failing_entropy() {
        let mut healthy = KatRandom::new(b"entropy-healthy");
        validate_collective_encryption_entropy(&mut healthy).unwrap();

        let mut zero = ConstantRandom(0);
        assert_eq!(
            validate_collective_encryption_entropy(&mut zero),
            Err(ZkAmsMkheErrorV1::RandomUnavailable)
        );
        let mut constant = ConstantRandom(0xa5);
        assert_eq!(
            validate_collective_encryption_entropy(&mut constant),
            Err(ZkAmsMkheErrorV1::RandomUnavailable)
        );
        let mut repeating = RepeatedHealthyBlockRandom;
        assert_eq!(
            validate_collective_encryption_entropy(&mut repeating),
            Err(ZkAmsMkheErrorV1::RandomUnavailable)
        );
        let mut failing = FailingRandom;
        assert_eq!(
            validate_collective_encryption_entropy(&mut failing),
            Err(ZkAmsMkheErrorV1::RandomUnavailable)
        );

        let profile = test_profile();
        let (key, _) = test_key(0x76);
        let mut repeating = RepeatedHealthyBlockRandom;
        assert!(matches!(
            try_encrypt_test_with_random(
                &profile,
                &key,
                &[0; 8],
                0,
                b"repeating-entropy-encryption",
                &mut repeating,
            ),
            Err(ZkAmsMkheErrorV1::RandomUnavailable)
        ));
        let mut failing = FailingRandom;
        assert!(matches!(
            try_encrypt_test_with_random(
                &profile,
                &key,
                &[0; 8],
                0,
                b"failing-entropy-encryption",
                &mut failing,
            ),
            Err(ZkAmsMkheErrorV1::RandomUnavailable)
        ));
    }

    #[test]
    fn collective_opening_debug_is_redacted_and_drop_zeroizes_every_witness() {
        let profile = test_profile();
        let (key, _) = test_key(0x77);
        let (_, mut opening, ..) = encrypt_test_with_opening(
            &profile,
            &key,
            &[16, 15, 14, 13, 12, 11, 10, 9],
            47,
            b"opening-redaction-drop",
        );
        let debug = format!("{opening:?}");
        assert_eq!(debug.matches("[REDACTED]").count(), 5);
        assert!(!debug.contains("coefficients:"));

        let audit = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        opening.arm_drop_zeroization_audit(audit.clone());
        drop(opening);
        assert!(audit.load(std::sync::atomic::Ordering::SeqCst));
    }

    #[test]
    fn natural_lift_effective_error_uses_the_exact_centered_boundary() {
        let profile = release_profile_v1();
        let mut canonical = vec![[0; 32]; profile.ring_degree];
        canonical[0] = super::super::T256_CENTERED_MAX_BE_V1;
        canonical[1] = super::super::T256_CENTERED_MAX_BE_V1;
        for byte in canonical[1].iter_mut().rev() {
            let (incremented, carried) = byte.overflowing_add(1);
            *byte = incremented;
            if !carried {
                break;
            }
        }
        let mut sampled = SecretPolynomial {
            coefficients: vec![0; profile.ring_degree],
        };
        sampled.coefficients[0] = i64::from(profile.error_eta);
        sampled.coefficients[1] = -i64::from(profile.error_eta);
        let effective =
            derive_natural_lift_effective_error_zero(&profile, &canonical, &sampled).unwrap();
        assert_eq!(effective.coefficients[0], i64::from(profile.error_eta));
        assert_eq!(effective.coefficients[1], -i64::from(profile.error_eta) - 1);
        assert!(
            effective.coefficients[2..]
                .iter()
                .all(|coefficient| *coefficient == 0)
        );

        assert!(matches!(
            derive_natural_lift_effective_error_zero(&test_profile(), &canonical[..8], &sampled),
            Err(ZkAmsMkheErrorV1::InvalidCiphertext)
        ));
    }

    #[test]
    fn persistent_commitment_blindings_have_exact_shape_order_and_redaction() {
        reset_persistent_blinding_drop_audits();
        let mut random = ScriptedPersistentBlindingRandom::from_scalars(
            1..=u64::try_from(ZK_AMS_MKHE_PERSISTENT_MEMBERSHIP_CHUNKS_V1)
                .expect("release chunk count fits u64"),
        );
        let owner = PersistentSecretCommitmentBlindingsV1::sample(&mut random)
            .expect("eight nonzero scripted blindings");

        assert_eq!(
            random.request_lengths,
            vec![PERSISTENT_BLINDING_ENTROPY_BYTES_V1; ZK_AMS_MKHE_PERSISTENT_MEMBERSHIP_CHUNKS_V1]
        );
        for (index, scalar) in owner.as_array().iter().enumerate() {
            assert_eq!(
                *scalar,
                Scalar::from_u64(u64::try_from(index + 1).expect("test index fits u64"))
            );
            assert!(!scalar.is_zero());
        }
        let canonical_bytes = owner
            .as_array()
            .iter()
            .flat_map(|scalar| scalar.to_le_bytes())
            .collect::<Vec<_>>();
        assert_eq!(canonical_bytes.len(), PERSISTENT_BLINDING_STATE_BYTES_V1);
        assert_eq!(canonical_bytes.len(), 256);
        assert_eq!(
            core::mem::size_of::<PersistentSecretCommitmentBlindingsV1>(),
            256
        );
        assert_eq!(
            format!("{owner:?}"),
            "PersistentSecretCommitmentBlindingsV1([REDACTED])"
        );
        assert_eq!(persistent_blinding_drop_audits(), (8, 0));
        drop(owner);
        assert_eq!(persistent_blinding_drop_audits(), (8, 1));
    }

    #[test]
    fn persistent_secret_membership_view_rejects_wrong_shape_and_non_ternary_state() {
        let exact_len =
            ZK_AMS_MKHE_PERSISTENT_MEMBERSHIP_CHUNKS_V1 * ZK_AMS_MEMBERSHIP_CHUNK_COEFFICIENTS_V1;
        let mut valid = SecretPolynomial {
            coefficients: vec![0; exact_len],
        };
        valid.coefficients[0] = -1;
        valid.coefficients[exact_len - 1] = 1;
        let narrowed = ZeroizingT256MembershipCoefficientsV1::from_ternary_secret(&valid)
            .expect("exact release ternary secret narrows without changing order");
        assert_eq!(narrowed.as_slice().len(), exact_len);
        assert_eq!(narrowed.as_slice()[0], -1);
        assert_eq!(narrowed.as_slice()[exact_len - 1], 1);
        drop(narrowed);

        let short = SecretPolynomial {
            coefficients: vec![0; exact_len - 1],
        };
        assert!(matches!(
            ZeroizingT256MembershipCoefficientsV1::from_ternary_secret(&short),
            Err(ZkAmsMkheErrorV1::InvalidKeyMaterial)
        ));

        valid.coefficients[ZK_AMS_MEMBERSHIP_CHUNK_COEFFICIENTS_V1] = 2;
        assert!(matches!(
            ZeroizingT256MembershipCoefficientsV1::from_ternary_secret(&valid),
            Err(ZkAmsMkheErrorV1::InvalidKeyMaterial)
        ));
    }

    #[test]
    fn state_owned_cpk_commitments_reject_secret_blinding_order_and_splice_changes() {
        let exact_len =
            ZK_AMS_MKHE_PERSISTENT_MEMBERSHIP_CHUNKS_V1 * ZK_AMS_MEMBERSHIP_CHUNK_COEFFICIENTS_V1;
        let mut coefficients = vec![0_i8; exact_len];
        coefficients[0] = -1;
        coefficients[ZK_AMS_MEMBERSHIP_CHUNK_COEFFICIENTS_V1] = 1;
        let blindings = core::array::from_fn(|index| {
            Scalar::from_u64(u64::try_from(index + 17).expect("test index fits u64"))
        });
        let expected = commit_persistent_secret_opening_v1(&coefficients, &blindings)
            .expect("exact state-owned opening commits all eight chunks");
        ensure_state_owned_cpk_commitments_v1(&expected, &expected)
            .expect("the exact ordered set is accepted");

        let mut changed_coefficients =
            coefficients[..ZK_AMS_MEMBERSHIP_CHUNK_COEFFICIENTS_V1].to_vec();
        changed_coefficients[0] = 0;
        let changed_secret_point = commit_zk_ams_t256_membership_chunk_v1(
            ZkAmsT256MembershipBoundV1::One,
            &changed_coefficients,
            &blindings[0],
        )
        .expect("mutated ternary chunk remains a valid commitment opening");
        assert_ne!(changed_secret_point, expected[0]);

        let changed_blinding_point = commit_zk_ams_t256_membership_chunk_v1(
            ZkAmsT256MembershipBoundV1::One,
            &coefficients[..ZK_AMS_MEMBERSHIP_CHUNK_COEFFICIENTS_V1],
            &Scalar::from_u64(0x5a),
        )
        .expect("replacement nonzero blinding remains a valid opening");
        assert_ne!(changed_blinding_point, expected[0]);

        let mut reordered = expected;
        reordered.swap(0, 1);
        assert!(matches!(
            ensure_state_owned_cpk_commitments_v1(&reordered, &expected),
            Err(ZkAmsMkheErrorV1::InvalidKeyMaterial)
        ));

        let mut duplicated = expected;
        duplicated[1] = duplicated[0];
        assert!(matches!(
            ensure_state_owned_cpk_commitments_v1(&duplicated, &expected),
            Err(ZkAmsMkheErrorV1::InvalidKeyMaterial)
        ));

        let mut spliced = expected;
        spliced[0] = changed_secret_point;
        assert!(matches!(
            ensure_state_owned_cpk_commitments_v1(&spliced, &expected),
            Err(ZkAmsMkheErrorV1::InvalidKeyMaterial)
        ));

        let mut zero_blinding = blindings;
        zero_blinding[3] = Scalar::zero();
        assert!(matches!(
            commit_persistent_secret_opening_v1(&coefficients, &zero_blinding),
            Err(ZkAmsMkheErrorV1::InvalidKeyMaterial)
        ));
        assert!(matches!(
            commit_persistent_secret_opening_v1(&coefficients[..exact_len - 1], &blindings),
            Err(ZkAmsMkheErrorV1::InvalidKeyMaterial)
        ));
    }

    #[test]
    fn persistent_commitment_blindings_retry_zero_without_reordering() {
        reset_persistent_blinding_drop_audits();
        let mut random =
            ScriptedPersistentBlindingRandom::from_scalars(core::iter::once(0).chain(1..=8));
        let owner = PersistentSecretCommitmentBlindingsV1::sample(&mut random)
            .expect("zero is retried before the ordered nonzero values");

        assert_eq!(random.request_lengths, vec![64; 9]);
        assert_eq!(
            owner
                .as_array()
                .iter()
                .map(|scalar| scalar.to_le_bytes()[0])
                .collect::<Vec<_>>(),
            (1_u8..=8).collect::<Vec<_>>()
        );
        assert_eq!(persistent_blinding_drop_audits(), (9, 0));
        drop(owner);
        assert_eq!(persistent_blinding_drop_audits(), (9, 1));
    }

    #[test]
    fn persistent_commitment_blindings_stop_at_exact_zero_rejection_ceiling() {
        reset_persistent_blinding_drop_audits();
        let mut random = ScriptedPersistentBlindingRandom::from_scalars(core::iter::repeat_n(
            0,
            MAX_RANDOM_REJECTION_ATTEMPTS_V1,
        ));
        assert!(matches!(
            PersistentSecretCommitmentBlindingsV1::sample(&mut random),
            Err(ZkAmsMkheErrorV1::RandomUnavailable)
        ));
        assert_eq!(random.next, MAX_RANDOM_REJECTION_ATTEMPTS_V1);
        assert_eq!(
            random.request_lengths,
            vec![PERSISTENT_BLINDING_ENTROPY_BYTES_V1; MAX_RANDOM_REJECTION_ATTEMPTS_V1]
        );
        assert_eq!(
            persistent_blinding_drop_audits(),
            (MAX_RANDOM_REJECTION_ATTEMPTS_V1, 1)
        );
    }

    #[test]
    fn persistent_commitment_blindings_erase_partial_state_on_rng_failure() {
        reset_persistent_blinding_drop_audits();
        let mut random = PartialFailurePersistentBlindingRandom {
            successful_requests: 3,
            calls: 0,
            partial_bytes: 23,
        };
        assert!(matches!(
            PersistentSecretCommitmentBlindingsV1::sample(&mut random),
            Err(ZkAmsMkheErrorV1::RandomUnavailable)
        ));
        assert_eq!(random.calls, 4);
        assert_eq!(persistent_blinding_drop_audits(), (4, 1));
    }

    #[test]
    fn persistent_commitment_blindings_erase_partial_state_during_unwind() {
        reset_persistent_blinding_drop_audits();
        let mut random = PartialPanicPersistentBlindingRandom {
            successful_requests: 2,
            calls: 0,
            partial_bytes: 41,
        };
        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = PersistentSecretCommitmentBlindingsV1::sample(&mut random);
        }));
        assert!(panic.is_err());
        assert_eq!(random.calls, 3);
        assert_eq!(persistent_blinding_drop_audits(), (3, 1));
    }

    #[test]
    fn persistent_commitment_blindings_move_without_duplicate_drop() {
        fn move_once(
            owner: PersistentSecretCommitmentBlindingsV1,
        ) -> PersistentSecretCommitmentBlindingsV1 {
            owner
        }

        reset_persistent_blinding_drop_audits();
        let mut random = ScriptedPersistentBlindingRandom::from_scalars(1..=8);
        let owner = PersistentSecretCommitmentBlindingsV1::sample(&mut random)
            .expect("scripted nonzero blindings");
        let owner = move_once(owner);
        let owner = move_once(owner);
        assert_eq!(persistent_blinding_drop_audits(), (8, 0));
        drop(owner);
        assert_eq!(persistent_blinding_drop_audits(), (8, 1));
    }

    #[test]
    fn opaque_party_state_debug_and_api_do_not_expose_rlwe_coefficients() {
        let state = ZkAmsMkheCollectivePartyStateV1 {
            profile_digest: [1; 32],
            security_certificate_digest: [2; 32],
            roster_digest: [3; 32],
            key_material_digest: [4; 32],
            epoch: 1,
            transcript_digest: [5; 32],
            party_index: 0,
            party: test_parties().parties[0],
            public_share_digest: [6; 32],
            persistent_secret_binding: None,
            persistent_secret_commitment_blindings: test_persistent_secret_commitment_blindings(),
            secret: SecretPolynomial {
                coefficients: vec![1, -1, 0],
            },
            public_error: SecretPolynomial {
                coefficients: vec![2, -2, 0],
            },
        };
        let debug = format!("{state:?}");
        assert_eq!(debug.matches("[REDACTED]").count(), 3);
        assert!(!debug.contains("-1"));
        assert!(!debug.contains("-2"));
        assert!(!debug.contains("17"));
        assert_eq!(state.secret().coefficients.len(), 3);
        assert_eq!(state.public_error().coefficients.len(), 3);
        assert_eq!(
            state.persistent_secret_commitment_blindings().len(),
            ZK_AMS_MKHE_PERSISTENT_MEMBERSHIP_CHUNKS_V1
        );
        assert!(
            state
                .persistent_secret_commitment_blindings()
                .iter()
                .all(|blinding| !blinding.is_zero())
        );
    }
}
