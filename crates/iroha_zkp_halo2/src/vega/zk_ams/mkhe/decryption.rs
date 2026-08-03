//! Authenticated full-roster partial decryption for the ZK-AMS MKHE profile.
//!
//! The native relation implemented here is
//!
//! ```text
//! b_i     = -a * s_i + t * e_i       in R_Q,
//! share_i = c_1 * s_i + t * z_i      in R_Q.
//! ```
//!
//! `s_i` is ternary, `e_i` is bounded by the profile's centered-binomial
//! parameter, and `z_i` is an exact signed, fixed-width smudging quotient.
//! The proof is a Fiat--Shamir-with-aborts lattice proof over both equations;
//! the verifier reconstructs both masked RNS commitments from the responses.
//! Consequently neither a signature nor a digest is accepted as a substitute
//! for the native polynomial relations.
//!
//! The sole first-release transport is a small authenticated `ZDSM` manifest
//! followed by two ordered, content-addressed objects: the existing canonical
//! count-prefixed limb-major polynomial bytes and the standalone native `ZADP`
//! proof bytes. Both objects fit their unchanged 64 MiB and 32 MiB ceilings.
//! Reconstruction authenticates the manifest and every statement axis before
//! hashing or decoding either large object. A private tiny-profile codec exists
//! only for exhaustive algebra tests and is not part of the public transport.

use core::{cmp::Ordering, mem::size_of};
use std::sync::Arc;

use super::{
    ArtifactAuthentication, BgvProfile, MAX_RANDOM_REJECTION_ATTEMPTS_V1, MKHE_VERSION_V1,
    MaskedRelaxedRandomSourceV1, PlaintextModulus, RnsPolynomial, SecretPolynomial, WideUint,
    ZkAmsMkheErrorV1, ZkAmsMkhePartyIdV1,
    active::ZkAmsMkheActivePartySecretV1,
    checked_coefficient_work, checked_ring_multiplication_work, checked_rns_polynomial_bytes,
    collective::{
        ZkAmsMkheCollectiveCiphertextV1, ZkAmsMkheCollectivePartyStateV1,
        ZkAmsMkheCollectivePublicKeyShareV1, ZkAmsMkheCollectivePublicKeyV1,
    },
    manifest::{
        ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1, ZK_AMS_MKHE_STATISTICAL_SECURITY_BITS_V1,
        release_profile_v1, zk_ams_mkhe_noise_certificate_v1,
    },
    modulus_product, sample_below,
    wire::{
        ZK_AMS_MKHE_MAX_PROOF_BYTES_V1, ZkAmsMkheAuthenticationWireV1,
        ZkAmsMkheCollectiveCiphertextWireV1, ZkAmsMkheGovernedRosterWireV1,
        ZkAmsMkheRnsPolynomialWireV1, derive_wire_length_certificate_v1, governed_roster_digest,
    },
    zk_ams_mkhe_security_certificate_v1,
};
use crate::vega::sponge::{Keccak256, keccak256, shake256};

#[cfg(test)]
use super::{
    AuthenticationSecret,
    persistent_membership_evidence::ZK_AMS_MKHE_PERSISTENT_MEMBERSHIP_CHUNKS_V1,
    wire::ZkAmsMkheWireBindingV1,
};

const DECRYPTION_PROOF_TAG_V1: [u8; 4] = *b"ZADP";
const DECRYPTION_SPLIT_MANIFEST_TAG_V1: [u8; 4] = *b"ZDSM";
// This private codec exists solely to exercise the complete tiny-profile path
// without allowing its variable dimensions to be confused with the production
// split decryption-share transport.
#[cfg(test)]
const TEST_DECRYPTION_SHARE_TAG_V1: [u8; 4] = *b"ZADT";
const DECRYPTION_PROOF_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.decryption-proof";
const DECRYPTION_CHALLENGE_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.decryption-proof-fiat-shamir";
const DECRYPTION_SPARSE_CHALLENGE_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.decryption-proof-sparse-challenge";
const DECRYPTION_SHARE_AUTH_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.authenticated-decryption-share";
const DECRYPTION_SPLIT_MANIFEST_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.decryption-split-manifest";
const DECRYPTION_SPLIT_MANIFEST_AUTH_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.authenticated-decryption-split-manifest";
const DECRYPTION_SET_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.decryption-share-set";
const DECRYPTION_RESOURCE_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.decryption-resource-evidence";
const DECRYPTION_KEY_CONTEXT_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.decryption-key-context";

const DECRYPTION_RELEASE_CHALLENGE_WEIGHT_V1: usize = 20;
pub(super) const WIDE_RELATION_MASK_SLACK_LOG2_V1: u32 = 24;
const DECRYPTION_SIGNED_SMALL_BYTES_V1: usize = size_of::<i64>();
const DECRYPTION_MAX_WIDE_LIMBS_V1: usize = 32;
const DECRYPTION_MAX_WIDE_BITS_V1: usize = DECRYPTION_MAX_WIDE_LIMBS_V1 * u64::BITS as usize;
const DECRYPTION_PROOF_HEADER_BYTES_V1: usize = 4 + 1 + 2 + 4 + 32 + 4 + 4 + 4;
const DECRYPTION_SPLIT_COMPONENT_COUNT_V1: usize = 2;
const DECRYPTION_SPLIT_POINTER_BYTES_V1: usize = 1 + 1 + 8 + 32;
/// Exact fixed width of the authenticated canonical split-transport manifest.
pub const ZK_AMS_MKHE_DECRYPTION_SPLIT_MANIFEST_BYTES_V1: usize = 4
    + 1
    + 32
    + 32
    + 8
    + 32
    + 32
    + 32
    + 32
    + 4
    + 8
    + 1
    + 32
    + 1
    + 1
    + DECRYPTION_SPLIT_COMPONENT_COUNT_V1 * DECRYPTION_SPLIT_POINTER_BYTES_V1
    + 32
    + 32
    + 33
    + 65;
/// Digest pinned by a genuine release-size split prove/reconstruct/verify KAT.
pub const ZK_AMS_MKHE_DECRYPTION_SPLIT_RELEASE_KAT_DIGEST_V1: [u8; 32] = [
    0xe0, 0x3a, 0x07, 0xa2, 0xea, 0xc5, 0xc5, 0xbe, 0x90, 0x7d, 0x6c, 0x46, 0x82, 0xc8, 0x07, 0xb9,
    0xfb, 0xf0, 0x00, 0xb5, 0x4f, 0x5e, 0x1d, 0xe9, 0x72, 0x5b, 0xc8, 0x58, 0xc2, 0xdb, 0xdc, 0xbd,
];
// tag, version, profile, roster, epoch, transcript, ciphertext, key context,
// exact public statement binding, sample index, party index, party, level, and
// proof length. The polynomial byte count below already includes its canonical
// residue-count word.
#[cfg(test)]
const TEST_DECRYPTION_SHARE_HEADER_BYTES_V1: usize =
    4 + 1 + 32 + 32 + 8 + 32 + 32 + 32 + 32 + 4 + 8 + 1 + 32 + 1 + 4;
#[cfg(test)]
const TEST_DECRYPTION_AUTHENTICATION_BYTES_V1: usize = 1 + 32 + 33 + 65;

/// Exact byte accounting for the sound transparent split decryption transport.
///
/// The hypothetical combined-inline figures document why the release uses two
/// separately governed content objects; they do not describe a public codec.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ZkAmsMkheDecryptionResourceEvidenceV1 {
    /// Frozen release ring degree.
    pub ring_degree: u32,
    /// Number of release RNS limbs.
    pub rns_limb_count: u8,
    /// Required fixed roster size.
    pub roster_size: u8,
    /// Exact smudging-quotient magnitude bound in bits.
    pub smudge_quotient_bits: u16,
    /// Sparse ring-challenge weight.
    pub challenge_weight: u8,
    /// Conservative lower bound on the signed sparse challenge space.
    pub challenge_space_lower_bound_bits: u16,
    /// Statistical hiding target used to derive the smudging width.
    pub statistical_security_bits: u16,
    /// Common-box mask slack used by Fiat--Shamir with aborts.
    pub mask_slack_log2: u8,
    /// Fixed bytes for one canonical wide response coefficient.
    pub wide_response_coefficient_bytes: u16,
    /// Exact canonical share-polynomial bytes.
    pub share_polynomial_bytes: u64,
    /// Exact bounded ternary-secret response bytes.
    pub secret_response_bytes: u64,
    /// Exact bounded public-key-error response bytes.
    pub public_key_error_response_bytes: u64,
    /// Exact statistically masked wide-smudge response bytes.
    pub smudge_response_bytes: u64,
    /// Exact proof headers and Fiat--Shamir challenge bytes.
    pub proof_header_bytes: u64,
    /// Exact standalone native `ZADP` proof bytes.
    pub proof_payload_bytes: u64,
    /// Independent absolute ceiling for the opaque proof-system payload.
    pub governed_proof_payload_ceiling_bytes: u64,
    /// Exact remaining room under the independent proof-payload ceiling.
    pub proof_payload_headroom_bytes: u64,
    /// True only if the proof-system payload fits its independent ceiling.
    pub proof_payload_ceiling_met: bool,
    /// Exact canonical bytes of the separately addressed share polynomial object.
    pub split_polynomial_object_bytes: u64,
    /// Exact canonical bytes of the separately addressed native proof envelope.
    pub split_proof_envelope_bytes: u64,
    /// Exact canonical bytes of the small authenticated split manifest.
    pub split_manifest_bytes: u64,
    /// Exact room remaining for the polynomial object below the unchanged 64 MiB ceiling.
    pub split_polynomial_headroom_bytes: u64,
    /// Exact room remaining for the proof envelope below the unchanged 32 MiB ceiling.
    pub split_proof_headroom_bytes: u64,
    /// Both separately addressed release objects fit their unchanged ceilings.
    pub split_component_ceilings_met: bool,
    /// Pinned authenticated manifest digest from the exact release-size split KAT.
    pub split_release_kat_digest: [u8; 32],
    /// The exact release-size native prove/verify path and canonical split
    /// reconstruction have closed the transport-implementation gate.
    ///
    /// This is not a knowledge-soundness claim for the sparse proof and does
    /// not close the global decryption admission/readiness gate.
    pub split_transport_ready: bool,
    /// Hypothetical inline-record overhead, retained only for resource evidence.
    pub record_overhead_bytes: u64,
    /// Hypothetical combined inline-record bytes; no public codec exposes it.
    pub total_share_record_bytes: u64,
    /// Current governed per-share ceiling.
    pub governed_share_ceiling_bytes: u64,
    /// Minimum governed ceiling required by this transparent proof.
    pub minimum_sound_share_ceiling_bytes: u64,
    /// Exact number of bytes by which the current ceiling is short.
    pub ceiling_shortfall_bytes: u64,
    /// True only if the hypothetical combined inline record fits one ceiling.
    pub share_ceiling_met: bool,
    /// Digest of every field and proof-domain parameter above.
    pub evidence_digest: [u8; 32],
}

impl ZkAmsMkheDecryptionResourceEvidenceV1 {
    /// Recompute and compare every byte, bound, and domain identity.
    pub fn validate(self) -> Result<(), ZkAmsMkheErrorV1> {
        let expected = derive_decryption_resource_evidence(&release_profile_v1())?;
        if self != expected
            || self.evidence_digest == [0; 32]
            || self.minimum_sound_share_ceiling_bytes != self.total_share_record_bytes
            || self.challenge_space_lower_bound_bits < 256
            || self.statistical_security_bits < 128
            || self.mask_slack_log2 != WIDE_RELATION_MASK_SLACK_LOG2_V1 as u8
            || self.proof_payload_ceiling_met
                != (self.proof_payload_bytes <= self.governed_proof_payload_ceiling_bytes)
            || self.proof_payload_headroom_bytes
                != self
                    .governed_proof_payload_ceiling_bytes
                    .saturating_sub(self.proof_payload_bytes)
            || self.split_polynomial_object_bytes != self.share_polynomial_bytes
            || self.split_proof_envelope_bytes != self.proof_payload_bytes
            || self.split_manifest_bytes
                != u64::try_from(ZK_AMS_MKHE_DECRYPTION_SPLIT_MANIFEST_BYTES_V1)
                    .map_err(|_| ZkAmsMkheErrorV1::InvalidProfile)?
            || self.split_polynomial_headroom_bytes
                != self
                    .governed_share_ceiling_bytes
                    .saturating_sub(self.split_polynomial_object_bytes)
            || self.split_proof_headroom_bytes
                != self
                    .governed_proof_payload_ceiling_bytes
                    .saturating_sub(self.split_proof_envelope_bytes)
            || self.split_component_ceilings_met
                != (self.split_polynomial_object_bytes <= self.governed_share_ceiling_bytes
                    && self.split_proof_envelope_bytes <= self.governed_proof_payload_ceiling_bytes)
            || self.split_release_kat_digest != ZK_AMS_MKHE_DECRYPTION_SPLIT_RELEASE_KAT_DIGEST_V1
            || self.split_transport_ready
                != (self.split_component_ceilings_met && self.split_release_kat_digest != [0; 32])
            || self.share_ceiling_met
                != (self.total_share_record_bytes <= self.governed_share_ceiling_bytes)
            || self.ceiling_shortfall_bytes
                != self
                    .total_share_record_bytes
                    .saturating_sub(self.governed_share_ceiling_bytes)
        {
            return Err(ZkAmsMkheErrorV1::InvalidProfile);
        }
        Ok(())
    }
}

/// Return machine-checked exact release-size share/proof accounting.
pub fn zk_ams_mkhe_decryption_resource_evidence_v1()
-> Result<ZkAmsMkheDecryptionResourceEvidenceV1, ZkAmsMkheErrorV1> {
    let evidence = derive_decryption_resource_evidence(&release_profile_v1())?;
    evidence.validate()?;
    Ok(evidence)
}

fn derive_decryption_resource_evidence(
    profile: &BgvProfile,
) -> Result<ZkAmsMkheDecryptionResourceEvidenceV1, ZkAmsMkheErrorV1> {
    profile.validate()?;
    let noise = zk_ams_mkhe_noise_certificate_v1()?;
    let smudge_bits = usize::from(noise.decryption_smudge_quotient_bits);
    let challenge_weight = wide_relation_challenge_weight(profile.ring_degree)?;
    let challenge_space_lower_bound_bits = u16::try_from(
        (profile.ring_degree / challenge_weight)
            .ilog2()
            .checked_add(1)
            .and_then(|bits| bits.checked_mul(challenge_weight as u32))
            .ok_or(ZkAmsMkheErrorV1::InvalidProfile)?,
    )
    .map_err(|_| ZkAmsMkheErrorV1::InvalidProfile)?;
    let (_, _, response_bytes) = wide_response_parameters(smudge_bits, challenge_weight)?;
    let ring_degree = u64::try_from(profile.ring_degree)
        .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let share_polynomial_bytes = u64::try_from(checked_rns_polynomial_bytes(profile)?)
        .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let secret_response_bytes = ring_degree
        .checked_mul(DECRYPTION_SIGNED_SMALL_BYTES_V1 as u64)
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let public_key_error_response_bytes = secret_response_bytes;
    let smudge_response_bytes = ring_degree
        .checked_mul(
            u64::try_from(response_bytes).map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
        )
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let proof_header_bytes = DECRYPTION_PROOF_HEADER_BYTES_V1 as u64;
    let proof_payload_bytes = secret_response_bytes
        .checked_add(public_key_error_response_bytes)
        .and_then(|value| value.checked_add(smudge_response_bytes))
        .and_then(|value| value.checked_add(proof_header_bytes))
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let governed_proof_payload_ceiling_bytes = u64::try_from(ZK_AMS_MKHE_MAX_PROOF_BYTES_V1)
        .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let wire_lengths = derive_wire_length_certificate_v1(profile)?;
    let public_share_base_bytes = u64::try_from(wire_lengths.streamed_contribution_base_wire_bytes)
        .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let proof_envelope_header_bytes = u64::try_from(wire_lengths.proof_envelope_header_wire_bytes)
        .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let record_overhead_bytes = public_share_base_bytes
        .checked_sub(share_polynomial_bytes)
        .and_then(|value| value.checked_add(proof_envelope_header_bytes))
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let total_share_record_bytes = share_polynomial_bytes
        .checked_add(proof_payload_bytes)
        .and_then(|value| value.checked_add(record_overhead_bytes))
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let (certified_proof_bytes, certified_record_bytes) =
        production_decryption_share_record_bytes(profile, smudge_bits)?;
    if u64::try_from(certified_proof_bytes).ok() != Some(proof_payload_bytes)
        || u64::try_from(certified_record_bytes).ok() != Some(total_share_record_bytes)
    {
        return Err(ZkAmsMkheErrorV1::InvalidProfile);
    }
    let governed_share_ceiling_bytes = u64::try_from(profile.max_share_bytes)
        .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let mut evidence = ZkAmsMkheDecryptionResourceEvidenceV1 {
        ring_degree: u32::try_from(profile.ring_degree)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
        rns_limb_count: u8::try_from(profile.moduli.len())
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
        roster_size: u8::try_from(ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
        smudge_quotient_bits: noise.decryption_smudge_quotient_bits,
        challenge_weight: u8::try_from(challenge_weight)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
        challenge_space_lower_bound_bits,
        statistical_security_bits: ZK_AMS_MKHE_STATISTICAL_SECURITY_BITS_V1,
        mask_slack_log2: u8::try_from(WIDE_RELATION_MASK_SLACK_LOG2_V1)
            .map_err(|_| ZkAmsMkheErrorV1::InvalidProfile)?,
        wide_response_coefficient_bytes: u16::try_from(response_bytes)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
        share_polynomial_bytes,
        secret_response_bytes,
        public_key_error_response_bytes,
        smudge_response_bytes,
        proof_header_bytes,
        proof_payload_bytes,
        governed_proof_payload_ceiling_bytes,
        proof_payload_headroom_bytes: governed_proof_payload_ceiling_bytes
            .saturating_sub(proof_payload_bytes),
        proof_payload_ceiling_met: proof_payload_bytes <= governed_proof_payload_ceiling_bytes,
        split_polynomial_object_bytes: share_polynomial_bytes,
        split_proof_envelope_bytes: proof_payload_bytes,
        split_manifest_bytes: u64::try_from(ZK_AMS_MKHE_DECRYPTION_SPLIT_MANIFEST_BYTES_V1)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
        split_polynomial_headroom_bytes: governed_share_ceiling_bytes
            .saturating_sub(share_polynomial_bytes),
        split_proof_headroom_bytes: governed_proof_payload_ceiling_bytes
            .saturating_sub(proof_payload_bytes),
        split_component_ceilings_met: share_polynomial_bytes <= governed_share_ceiling_bytes
            && proof_payload_bytes <= governed_proof_payload_ceiling_bytes,
        split_release_kat_digest: ZK_AMS_MKHE_DECRYPTION_SPLIT_RELEASE_KAT_DIGEST_V1,
        split_transport_ready: share_polynomial_bytes <= governed_share_ceiling_bytes
            && proof_payload_bytes <= governed_proof_payload_ceiling_bytes
            && ZK_AMS_MKHE_DECRYPTION_SPLIT_RELEASE_KAT_DIGEST_V1 != [0; 32],
        record_overhead_bytes,
        total_share_record_bytes,
        governed_share_ceiling_bytes,
        minimum_sound_share_ceiling_bytes: total_share_record_bytes,
        ceiling_shortfall_bytes: total_share_record_bytes
            .saturating_sub(governed_share_ceiling_bytes),
        share_ceiling_met: total_share_record_bytes <= governed_share_ceiling_bytes,
        evidence_digest: [0; 32],
    };
    evidence.evidence_digest = decryption_resource_evidence_digest(evidence);
    Ok(evidence)
}

fn decryption_resource_evidence_digest(
    evidence: ZkAmsMkheDecryptionResourceEvidenceV1,
) -> [u8; 32] {
    let mut frame = Vec::with_capacity(384);
    frame.extend_from_slice(DECRYPTION_RESOURCE_DOMAIN_V1);
    for domain in [
        DECRYPTION_PROOF_DOMAIN_V1,
        DECRYPTION_CHALLENGE_DOMAIN_V1,
        DECRYPTION_SPARSE_CHALLENGE_DOMAIN_V1,
        DECRYPTION_SHARE_AUTH_DOMAIN_V1,
        DECRYPTION_SPLIT_MANIFEST_DOMAIN_V1,
        DECRYPTION_SPLIT_MANIFEST_AUTH_DOMAIN_V1,
        DECRYPTION_SET_DOMAIN_V1,
    ] {
        frame.extend_from_slice(&(domain.len() as u32).to_be_bytes());
        frame.extend_from_slice(domain);
    }
    frame.extend_from_slice(&evidence.ring_degree.to_be_bytes());
    frame.push(evidence.rns_limb_count);
    frame.push(evidence.roster_size);
    frame.extend_from_slice(&evidence.smudge_quotient_bits.to_be_bytes());
    frame.push(evidence.challenge_weight);
    frame.extend_from_slice(&evidence.challenge_space_lower_bound_bits.to_be_bytes());
    frame.extend_from_slice(&evidence.statistical_security_bits.to_be_bytes());
    frame.push(evidence.mask_slack_log2);
    frame.extend_from_slice(&evidence.wide_response_coefficient_bytes.to_be_bytes());
    for value in [
        evidence.share_polynomial_bytes,
        evidence.secret_response_bytes,
        evidence.public_key_error_response_bytes,
        evidence.smudge_response_bytes,
        evidence.proof_header_bytes,
        evidence.proof_payload_bytes,
        evidence.governed_proof_payload_ceiling_bytes,
        evidence.proof_payload_headroom_bytes,
        evidence.split_polynomial_object_bytes,
        evidence.split_proof_envelope_bytes,
        evidence.split_manifest_bytes,
        evidence.split_polynomial_headroom_bytes,
        evidence.split_proof_headroom_bytes,
        evidence.record_overhead_bytes,
        evidence.total_share_record_bytes,
        evidence.governed_share_ceiling_bytes,
        evidence.minimum_sound_share_ceiling_bytes,
        evidence.ceiling_shortfall_bytes,
    ] {
        frame.extend_from_slice(&value.to_be_bytes());
    }
    frame.push(evidence.proof_payload_ceiling_met.into());
    frame.push(evidence.split_component_ceilings_met.into());
    frame.extend_from_slice(&evidence.split_release_kat_digest);
    frame.push(evidence.split_transport_ready.into());
    frame.push(evidence.share_ceiling_met.into());
    frame.extend_from_slice(&WIDE_RELATION_MASK_SLACK_LOG2_V1.to_be_bytes());
    frame.extend_from_slice(&(DECRYPTION_MAX_WIDE_BITS_V1 as u32).to_be_bytes());
    keccak256(&frame)
}

#[derive(Clone, PartialEq, Eq)]
pub(super) struct WideMagnitudeV1 {
    pub(super) limbs: [u64; DECRYPTION_MAX_WIDE_LIMBS_V1],
}

impl WideMagnitudeV1 {
    pub(super) const fn zero() -> Self {
        Self {
            limbs: [0; DECRYPTION_MAX_WIDE_LIMBS_V1],
        }
    }

    pub(super) fn max_for_bits(bits: usize) -> Result<Self, ZkAmsMkheErrorV1> {
        if bits == 0 || bits > DECRYPTION_MAX_WIDE_BITS_V1 {
            return Err(ZkAmsMkheErrorV1::InvalidProfile);
        }
        let mut value = Self::zero();
        let full = bits / 64;
        let tail = bits % 64;
        for limb in value.limbs.iter_mut().take(full) {
            *limb = u64::MAX;
        }
        if tail != 0 {
            value.limbs[full] = (1_u64 << tail) - 1;
        }
        Ok(value)
    }

    pub(super) fn is_zero(&self) -> bool {
        self.limbs.iter().all(|limb| *limb == 0)
    }

    pub(super) fn bit_len(&self) -> usize {
        self.limbs
            .iter()
            .rposition(|limb| *limb != 0)
            .map_or(0, |index| {
                index * 64 + (64 - self.limbs[index].leading_zeros() as usize)
            })
    }

    pub(super) fn checked_add(&self, rhs: &Self) -> Option<Self> {
        let mut output = Self::zero();
        let mut carry = 0_u128;
        for index in 0..DECRYPTION_MAX_WIDE_LIMBS_V1 {
            let sum = u128::from(self.limbs[index]) + u128::from(rhs.limbs[index]) + carry;
            output.limbs[index] = sum as u64;
            carry = sum >> 64;
        }
        (carry == 0).then_some(output)
    }

    pub(super) fn checked_sub(&self, rhs: &Self) -> Option<Self> {
        let mut output = Self::zero();
        let mut borrow = false;
        for index in 0..DECRYPTION_MAX_WIDE_LIMBS_V1 {
            let (first, first_borrow) = self.limbs[index].overflowing_sub(rhs.limbs[index]);
            let (second, second_borrow) = first.overflowing_sub(u64::from(borrow));
            output.limbs[index] = second;
            borrow = first_borrow || second_borrow;
        }
        (!borrow).then_some(output)
    }

    pub(super) fn checked_mul_u64(&self, rhs: u64) -> Option<Self> {
        let mut output = Self::zero();
        let mut carry = 0_u128;
        for (destination, value) in output.limbs.iter_mut().zip(self.limbs) {
            let product = u128::from(value) * u128::from(rhs) + carry;
            *destination = product as u64;
            carry = product >> 64;
        }
        (carry == 0).then_some(output)
    }

    pub(super) fn mod_u64(&self, modulus: u64) -> u64 {
        self.limbs.iter().rev().fold(0_u64, |remainder, limb| {
            ((u128::from(remainder) << 64 | u128::from(*limb)) % u128::from(modulus)) as u64
        })
    }

    pub(super) fn from_fixed_be(bytes: &[u8]) -> Result<Self, ZkAmsMkheErrorV1> {
        if bytes.is_empty() || bytes.len() > DECRYPTION_MAX_WIDE_LIMBS_V1 * 8 {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        let mut value = Self::zero();
        for (index, byte) in bytes.iter().rev().enumerate() {
            let limb = index / 8;
            let shift = (index % 8) * 8;
            value.limbs[limb] |= u64::from(*byte) << shift;
        }
        Ok(value)
    }
}

impl PartialOrd for WideMagnitudeV1 {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for WideMagnitudeV1 {
    fn cmp(&self, other: &Self) -> Ordering {
        for index in (0..DECRYPTION_MAX_WIDE_LIMBS_V1).rev() {
            match self.limbs[index].cmp(&other.limbs[index]) {
                Ordering::Equal => {}
                ordering => return ordering,
            }
        }
        Ordering::Equal
    }
}

impl core::fmt::Debug for WideMagnitudeV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("WideMagnitudeV1")
            .field("bit_len", &self.bit_len())
            .finish_non_exhaustive()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub(super) struct SignedWideV1 {
    pub(super) negative: bool,
    pub(super) magnitude: WideMagnitudeV1,
}

impl SignedWideV1 {
    pub(super) const fn zero() -> Self {
        Self {
            negative: false,
            magnitude: WideMagnitudeV1::zero(),
        }
    }

    #[cfg(test)]
    pub(super) fn from_i64(value: i64) -> Self {
        let mut magnitude = WideMagnitudeV1::zero();
        magnitude.limbs[0] = value.unsigned_abs();
        Self {
            negative: value < 0,
            magnitude,
        }
    }

    pub(super) fn new(
        negative: bool,
        magnitude: WideMagnitudeV1,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        if negative && magnitude.is_zero() {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        Ok(Self {
            negative,
            magnitude,
        })
    }

    pub(super) fn checked_add(&self, rhs: &Self) -> Option<Self> {
        if self.negative == rhs.negative {
            let magnitude = self.magnitude.checked_add(&rhs.magnitude)?;
            Some(Self {
                negative: self.negative && !magnitude.is_zero(),
                magnitude,
            })
        } else {
            match self.magnitude.cmp(&rhs.magnitude) {
                Ordering::Greater => Some(Self {
                    negative: self.negative,
                    magnitude: self.magnitude.checked_sub(&rhs.magnitude)?,
                }),
                Ordering::Less => Some(Self {
                    negative: rhs.negative,
                    magnitude: rhs.magnitude.checked_sub(&self.magnitude)?,
                }),
                Ordering::Equal => Some(Self::zero()),
            }
        }
    }

    pub(super) fn negated(&self) -> Self {
        if self.magnitude.is_zero() {
            Self::zero()
        } else {
            Self {
                negative: !self.negative,
                magnitude: self.magnitude.clone(),
            }
        }
    }

    pub(super) fn mod_u64(&self, modulus: u64) -> u64 {
        let residue = self.magnitude.mod_u64(modulus);
        if self.negative && residue != 0 {
            modulus - residue
        } else {
            residue
        }
    }

    #[cfg(test)]
    pub(super) fn encode_fixed(&self, byte_len: usize) -> Result<Vec<u8>, ZkAmsMkheErrorV1> {
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(byte_len)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        self.encode_fixed_into(&mut bytes, byte_len)?;
        Ok(bytes)
    }

    pub(super) fn encode_fixed_into(
        &self,
        output: &mut Vec<u8>,
        byte_len: usize,
    ) -> Result<(), ZkAmsMkheErrorV1> {
        if byte_len == 0 || self.magnitude.bit_len() > byte_len * 8 - 1 {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        if self.negative && self.magnitude.is_zero() {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        let start = output.len();
        output.resize(
            start
                .checked_add(byte_len)
                .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
            0,
        );
        let bytes = &mut output[start..];
        for (index, destination) in bytes.iter_mut().rev().enumerate() {
            let limb = index / 8;
            let shift = (index % 8) * 8;
            *destination = (self.magnitude.limbs[limb] >> shift) as u8;
        }
        if bytes[0] & 0x80 != 0 {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        if self.negative {
            bytes[0] |= 0x80;
        }
        Ok(())
    }

    pub(super) fn decode_fixed(bytes: &[u8]) -> Result<Self, ZkAmsMkheErrorV1> {
        if bytes.is_empty() || bytes.len() > DECRYPTION_MAX_WIDE_LIMBS_V1 * 8 {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        let negative = bytes[0] & 0x80 != 0;
        let mut magnitude = WideMagnitudeV1::zero();
        for (index, byte) in bytes.iter().rev().enumerate() {
            let limb = index / 8;
            let shift = (index % 8) * 8;
            let value = if index + 1 == bytes.len() {
                *byte & 0x7f
            } else {
                *byte
            };
            magnitude.limbs[limb] |= u64::from(value) << shift;
        }
        Self::new(negative, magnitude)
    }
}

impl Drop for SignedWideV1 {
    fn drop(&mut self) {
        self.negative = false;
        self.magnitude.limbs.fill(0);
    }
}

impl core::fmt::Debug for SignedWideV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("SignedWideV1([REDACTED])")
    }
}

pub(super) fn wide_response_parameters(
    witness_bits: usize,
    challenge_weight: usize,
) -> Result<(WideMagnitudeV1, WideMagnitudeV1, usize), ZkAmsMkheErrorV1> {
    let witness_bound = WideMagnitudeV1::max_for_bits(witness_bits)?;
    let challenge_slack = witness_bound
        .checked_mul_u64(
            u64::try_from(challenge_weight)
                .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
        )
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let mask_bound = challenge_slack
        .checked_mul_u64(1_u64 << WIDE_RELATION_MASK_SLACK_LOG2_V1)
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let response_limit = mask_bound
        .checked_sub(&challenge_slack)
        .ok_or(ZkAmsMkheErrorV1::InvalidProfile)?;
    let response_bytes = mask_bound
        .bit_len()
        .checked_add(1)
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?
        .div_ceil(8);
    if response_bytes == 0 || response_bytes > DECRYPTION_MAX_WIDE_LIMBS_V1 * 8 {
        return Err(ZkAmsMkheErrorV1::ResourceCeilingExceeded);
    }
    Ok((mask_bound, response_limit, response_bytes))
}

pub(super) fn small_response_parameters(
    witness_bound: i64,
    challenge_weight: usize,
    profile: &BgvProfile,
) -> Result<(i64, i64), ZkAmsMkheErrorV1> {
    if witness_bound <= 0 {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    let challenge_slack = witness_bound
        .checked_mul(
            i64::try_from(challenge_weight)
                .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
        )
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let mask_bound = challenge_slack
        .checked_mul(1_i64 << WIDE_RELATION_MASK_SLACK_LOG2_V1)
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let response_limit = mask_bound
        .checked_sub(challenge_slack)
        .ok_or(ZkAmsMkheErrorV1::InvalidProfile)?;
    let minimum_modulus = profile
        .moduli
        .iter()
        .copied()
        .min()
        .and_then(|value| i64::try_from(value).ok())
        .ok_or(ZkAmsMkheErrorV1::InvalidProfile)?;
    if response_limit <= 0 || mask_bound >= (minimum_modulus - 1) / 2 {
        return Err(ZkAmsMkheErrorV1::InvalidProfile);
    }
    Ok((mask_bound, response_limit))
}

pub(super) fn wide_relation_challenge_weight(
    ring_degree: usize,
) -> Result<usize, ZkAmsMkheErrorV1> {
    if ring_degree < 2 || !ring_degree.is_power_of_two() {
        return Err(ZkAmsMkheErrorV1::InvalidProfile);
    }
    Ok(DECRYPTION_RELEASE_CHALLENGE_WEIGHT_V1.min((ring_degree / 2).max(1)))
}

pub(super) fn sample_signed_small<R: MaskedRelaxedRandomSourceV1>(
    bound: i64,
    random: &mut R,
) -> Result<i64, ZkAmsMkheErrorV1> {
    let width = u64::try_from(bound)
        .ok()
        .and_then(|bound| bound.checked_mul(2))
        .and_then(|bound| bound.checked_add(1))
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    i64::try_from(sample_below(width, random)?)
        .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?
        .checked_sub(bound)
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)
}

pub(super) fn validate_wide_relation_random_health<R: MaskedRelaxedRandomSourceV1>(
    random: &mut R,
) -> Result<(), ZkAmsMkheErrorV1> {
    let mut first = [0_u8; 32];
    random
        .fill_bytes(&mut first)
        .map_err(|_| ZkAmsMkheErrorV1::RandomUnavailable)?;
    if first == [0; 32] {
        return Err(ZkAmsMkheErrorV1::RandomUnavailable);
    }
    for _ in 0..MAX_RANDOM_REJECTION_ATTEMPTS_V1 {
        let mut next = [0_u8; 32];
        random
            .fill_bytes(&mut next)
            .map_err(|_| ZkAmsMkheErrorV1::RandomUnavailable)?;
        if next != [0; 32] && next != first {
            first.fill(0);
            next.fill(0);
            return Ok(());
        }
        next.fill(0);
    }
    first.fill(0);
    Err(ZkAmsMkheErrorV1::RandomUnavailable)
}

fn sample_wide_magnitude_below_or_equal<R: MaskedRelaxedRandomSourceV1>(
    bound: &WideMagnitudeV1,
    random: &mut R,
) -> Result<WideMagnitudeV1, ZkAmsMkheErrorV1> {
    if bound.is_zero() {
        return Ok(WideMagnitudeV1::zero());
    }
    let bits = bound.bit_len();
    let byte_len = bits.div_ceil(8);
    let unused = byte_len * 8 - bits;
    for _ in 0..MAX_RANDOM_REJECTION_ATTEMPTS_V1 {
        let mut bytes = vec![0_u8; byte_len];
        random
            .fill_bytes(&mut bytes)
            .map_err(|_| ZkAmsMkheErrorV1::RandomUnavailable)?;
        if unused != 0 {
            bytes[0] &= u8::MAX >> unused;
        }
        let candidate = WideMagnitudeV1::from_fixed_be(&bytes)?;
        bytes.fill(0);
        if candidate <= *bound {
            return Ok(candidate);
        }
    }
    Err(ZkAmsMkheErrorV1::RandomUnavailable)
}

pub(super) fn sample_signed_wide<R: MaskedRelaxedRandomSourceV1>(
    magnitude_bound: &WideMagnitudeV1,
    random: &mut R,
) -> Result<SignedWideV1, ZkAmsMkheErrorV1> {
    // Sample exactly from the `2B + 1` signed integers in `[-B, B]`.
    let twice_bound = magnitude_bound
        .checked_mul_u64(2)
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let sample = sample_wide_magnitude_below_or_equal(&twice_bound, random)?;
    if sample <= *magnitude_bound {
        SignedWideV1::new(false, sample)
    } else {
        let magnitude = sample
            .checked_sub(magnitude_bound)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        SignedWideV1::new(true, magnitude)
    }
}

pub(super) fn wide_vector_as_rns(
    profile: &BgvProfile,
    values: &[SignedWideV1],
) -> Result<RnsPolynomial, ZkAmsMkheErrorV1> {
    if values.len() != profile.ring_degree {
        return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
    }
    checked_coefficient_work(profile, profile.moduli.len())?;
    let mut coefficients = Vec::with_capacity(
        profile
            .ring_degree
            .checked_mul(profile.moduli.len())
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
    );
    for modulus in profile.moduli {
        coefficients.extend(values.iter().map(|value| value.mod_u64(*modulus)));
    }
    RnsPolynomial::from_flat(profile, coefficients)
}

pub(super) fn sparse_negacyclic_mul_small(
    challenge: &[i8],
    witness: &[i64],
) -> Result<Vec<i64>, ZkAmsMkheErrorV1> {
    if challenge.len() != witness.len()
        || challenge.is_empty()
        || !challenge.len().is_power_of_two()
    {
        return Err(ZkAmsMkheErrorV1::InvalidShareProof);
    }
    let degree = challenge.len();
    let mut output = vec![0_i64; degree];
    for (shift, sign) in challenge.iter().copied().enumerate() {
        if sign == 0 {
            continue;
        }
        if ![-1, 1].contains(&sign) {
            return Err(ZkAmsMkheErrorV1::InvalidShareProof);
        }
        for (index, coefficient) in witness.iter().copied().enumerate() {
            let destination = index + shift;
            let (destination, wrap_sign) = if destination >= degree {
                (destination - degree, -1_i64)
            } else {
                (destination, 1_i64)
            };
            let term = coefficient
                .checked_mul(i64::from(sign))
                .and_then(|value| value.checked_mul(wrap_sign))
                .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
            output[destination] = output[destination]
                .checked_add(term)
                .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        }
    }
    Ok(output)
}

pub(super) fn sparse_negacyclic_mul_wide(
    challenge: &[i8],
    witness: &[SignedWideV1],
) -> Result<Vec<SignedWideV1>, ZkAmsMkheErrorV1> {
    if challenge.len() != witness.len()
        || challenge.is_empty()
        || !challenge.len().is_power_of_two()
    {
        return Err(ZkAmsMkheErrorV1::InvalidShareProof);
    }
    let degree = challenge.len();
    let mut output = vec![SignedWideV1::zero(); degree];
    for (shift, sign) in challenge.iter().copied().enumerate() {
        if sign == 0 {
            continue;
        }
        if ![-1, 1].contains(&sign) {
            return Err(ZkAmsMkheErrorV1::InvalidShareProof);
        }
        for (index, coefficient) in witness.iter().enumerate() {
            let destination = index + shift;
            let (destination, wrap_sign) = if destination >= degree {
                (destination - degree, -1_i8)
            } else {
                (destination, 1_i8)
            };
            let term = if sign * wrap_sign < 0 {
                coefficient.negated()
            } else {
                coefficient.clone()
            };
            output[destination] = output[destination]
                .checked_add(&term)
                .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        }
    }
    Ok(output)
}

fn derive_sparse_challenge(
    ring_degree: usize,
    challenge_seed: [u8; 32],
) -> Result<Vec<i8>, ZkAmsMkheErrorV1> {
    if challenge_seed == [0; 32] {
        return Err(ZkAmsMkheErrorV1::InvalidShareProof);
    }
    let weight = wide_relation_challenge_weight(ring_degree)?;
    let mut frame = Vec::with_capacity(96);
    frame.extend_from_slice(DECRYPTION_SPARSE_CHALLENGE_DOMAIN_V1);
    frame.extend_from_slice(&challenge_seed);
    frame.extend_from_slice(
        &u32::try_from(ring_degree)
            .map_err(|_| ZkAmsMkheErrorV1::InvalidProfile)?
            .to_be_bytes(),
    );
    frame.extend_from_slice(
        &u32::try_from(weight)
            .map_err(|_| ZkAmsMkheErrorV1::InvalidProfile)?
            .to_be_bytes(),
    );
    let stream = shake256(
        &frame,
        weight
            .checked_mul(MAX_RANDOM_REJECTION_ATTEMPTS_V1)
            .and_then(|value| value.checked_mul(8))
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
    );
    let mask = u64::try_from(ring_degree - 1).map_err(|_| ZkAmsMkheErrorV1::InvalidProfile)?;
    let mut challenge = vec![0_i8; ring_degree];
    let mut selected = 0;
    for chunk in stream.chunks_exact(8) {
        let word = u64::from_le_bytes(
            chunk
                .try_into()
                .map_err(|_| ZkAmsMkheErrorV1::InvalidShareProof)?,
        );
        let position =
            usize::try_from(word & mask).map_err(|_| ZkAmsMkheErrorV1::InvalidProfile)?;
        if challenge[position] != 0 {
            continue;
        }
        challenge[position] = if word >> 63 == 0 { -1 } else { 1 };
        selected += 1;
        if selected == weight {
            return Ok(challenge);
        }
    }
    Err(ZkAmsMkheErrorV1::InvalidShareProof)
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct DecryptionBindingV1 {
    profile_digest: [u8; 32],
    roster_digest: [u8; 32],
    epoch: u64,
    transcript_digest: [u8; 32],
    ciphertext_digest: [u8; 32],
    key_context_digest: [u8; 32],
    statement_binding_digest: [u8; 32],
    ciphertext_record_index: u32,
    sample_index: u64,
    party_index: u8,
    party: ZkAmsMkhePartyIdV1,
    level: u8,
}

impl DecryptionBindingV1 {
    fn validate(
        &self,
        profile: &BgvProfile,
        parties: &super::PartySet,
    ) -> Result<(), ZkAmsMkheErrorV1> {
        if self.profile_digest != profile.digest()?
            || self.roster_digest
                != governed_roster_digest(self.profile_digest, self.epoch, &parties.parties)
            || self.epoch == 0
            || self.transcript_digest == [0; 32]
            || self.ciphertext_digest == [0; 32]
            || self.key_context_digest == [0; 32]
            || self.statement_binding_digest == [0; 32]
            || self.level > 1
            || parties.parties.len() != ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1
            || usize::from(self.party_index) >= parties.parties.len()
            || parties.parties[usize::from(self.party_index)] != self.party
        {
            return Err(ZkAmsMkheErrorV1::InvalidShareSet);
        }
        Ok(())
    }

    fn update_hash(&self, hash: &mut Keccak256) {
        hash.update(&self.profile_digest);
        hash.update(&self.roster_digest);
        hash.update(&self.epoch.to_be_bytes());
        hash.update(&self.transcript_digest);
        hash.update(&self.ciphertext_digest);
        hash.update(&self.key_context_digest);
        hash.update(&self.statement_binding_digest);
        hash.update(&self.ciphertext_record_index.to_be_bytes());
        hash.update(&self.sample_index.to_be_bytes());
        hash.update(&[self.party_index]);
        hash.update(&self.party.to_bytes());
        hash.update(&[self.level]);
    }
}

/// Ordered content kind carried by the canonical split decryption transport.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ZkAmsMkheDecryptionTransportComponentKindV1 {
    /// Exact canonical limb-major partial-decryption polynomial object.
    SharePolynomial = 1,
    /// Exact canonical native `ZADP` proof envelope.
    ProofEnvelope = 2,
}

impl TryFrom<u8> for ZkAmsMkheDecryptionTransportComponentKindV1 {
    type Error = ZkAmsMkheErrorV1;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::SharePolynomial),
            2 => Ok(Self::ProofEnvelope),
            _ => Err(ZkAmsMkheErrorV1::InvalidWireEncoding),
        }
    }
}

/// Exact BLAKE3 content address of one ordered decryption transport object.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ZkAmsMkheDecryptionTransportPointerV1 {
    kind: ZkAmsMkheDecryptionTransportComponentKindV1,
    payload_bytes: u64,
    payload_blake3: [u8; 32],
}

impl ZkAmsMkheDecryptionTransportPointerV1 {
    fn from_payload(
        kind: ZkAmsMkheDecryptionTransportComponentKindV1,
        payload: &[u8],
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        let value = Self {
            kind,
            payload_bytes: u64::try_from(payload.len())
                .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
            payload_blake3: norito::streaming::blake3_hash(payload),
        };
        value.validate_shape()?;
        Ok(value)
    }

    fn validate_shape(self) -> Result<(), ZkAmsMkheErrorV1> {
        let evidence = derive_decryption_resource_evidence(&release_profile_v1())?;
        let expected = match self.kind {
            ZkAmsMkheDecryptionTransportComponentKindV1::SharePolynomial => {
                evidence.split_polynomial_object_bytes
            }
            ZkAmsMkheDecryptionTransportComponentKindV1::ProofEnvelope => {
                evidence.split_proof_envelope_bytes
            }
        };
        if self.payload_bytes != expected || self.payload_blake3 == [0; 32] {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        Ok(())
    }

    fn validate_payload(self, payload: &[u8]) -> Result<(), ZkAmsMkheErrorV1> {
        self.validate_shape()?;
        if u64::try_from(payload.len()).ok() != Some(self.payload_bytes)
            || norito::streaming::blake3_hash(payload) != self.payload_blake3
        {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        Ok(())
    }

    /// Ordered component kind committed by the authenticated manifest.
    #[must_use]
    pub const fn kind(self) -> ZkAmsMkheDecryptionTransportComponentKindV1 {
        self.kind
    }

    /// Exact complete component byte length.
    #[must_use]
    pub const fn payload_bytes(self) -> u64 {
        self.payload_bytes
    }

    /// BLAKE3 digest of the exact complete component bytes.
    #[must_use]
    pub const fn payload_blake3(self) -> [u8; 32] {
        self.payload_blake3
    }
}

/// Small authenticated header for the two-object canonical decryption transport.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ZkAmsMkheDecryptionTransportManifestV1 {
    binding: DecryptionBindingV1,
    polynomial: ZkAmsMkheDecryptionTransportPointerV1,
    proof: ZkAmsMkheDecryptionTransportPointerV1,
    manifest_digest: [u8; 32],
    authentication: ArtifactAuthentication,
}

impl ZkAmsMkheDecryptionTransportManifestV1 {
    fn new(
        binding: DecryptionBindingV1,
        polynomial: ZkAmsMkheDecryptionTransportPointerV1,
        proof: ZkAmsMkheDecryptionTransportPointerV1,
        authentication: ArtifactAuthentication,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        let manifest_digest = decryption_split_manifest_digest(&binding, polynomial, proof)?;
        let value = Self {
            binding,
            polynomial,
            proof,
            manifest_digest,
            authentication,
        };
        value.validate_structural()?;
        Ok(value)
    }

    fn validate_structural(&self) -> Result<(), ZkAmsMkheErrorV1> {
        if self.polynomial.kind != ZkAmsMkheDecryptionTransportComponentKindV1::SharePolynomial
            || self.proof.kind != ZkAmsMkheDecryptionTransportComponentKindV1::ProofEnvelope
            || self.manifest_digest == [0; 32]
            || self.manifest_digest
                != decryption_split_manifest_digest(&self.binding, self.polynomial, self.proof)?
            || self.authentication.party != self.binding.party
        {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        self.polynomial.validate_shape()?;
        self.proof.validate_shape()?;
        self.authentication.verify(
            DECRYPTION_SPLIT_MANIFEST_AUTH_DOMAIN_V1,
            self.manifest_digest,
        )
    }

    fn validate_for_statement(
        &self,
        statement: ZkAmsMkheDecryptionStatementV1<'_>,
    ) -> Result<(), ZkAmsMkheErrorV1> {
        self.validate_structural()?;
        let expected =
            decryption_binding_from_statement(statement, usize::from(self.binding.party_index))?;
        if self.binding != expected {
            return Err(ZkAmsMkheErrorV1::InvalidShareSet);
        }
        Ok(())
    }

    /// Encode the sole fixed-width canonical split manifest.
    pub fn encode(&self) -> Result<Vec<u8>, ZkAmsMkheErrorV1> {
        self.validate_structural()?;
        let mut bytes = Vec::with_capacity(ZK_AMS_MKHE_DECRYPTION_SPLIT_MANIFEST_BYTES_V1);
        bytes.extend_from_slice(&DECRYPTION_SPLIT_MANIFEST_TAG_V1);
        bytes.push(MKHE_VERSION_V1);
        bytes.extend_from_slice(&self.binding.profile_digest);
        bytes.extend_from_slice(&self.binding.roster_digest);
        bytes.extend_from_slice(&self.binding.epoch.to_be_bytes());
        bytes.extend_from_slice(&self.binding.transcript_digest);
        bytes.extend_from_slice(&self.binding.ciphertext_digest);
        bytes.extend_from_slice(&self.binding.key_context_digest);
        bytes.extend_from_slice(&self.binding.statement_binding_digest);
        bytes.extend_from_slice(&self.binding.ciphertext_record_index.to_be_bytes());
        bytes.extend_from_slice(&self.binding.sample_index.to_be_bytes());
        bytes.push(self.binding.party_index);
        bytes.extend_from_slice(&self.binding.party.to_bytes());
        bytes.push(self.binding.level);
        bytes.push(DECRYPTION_SPLIT_COMPONENT_COUNT_V1 as u8);
        write_decryption_transport_pointer(&mut bytes, 0, self.polynomial);
        write_decryption_transport_pointer(&mut bytes, 1, self.proof);
        bytes.extend_from_slice(&self.manifest_digest);
        bytes.extend_from_slice(&self.authentication.party.to_bytes());
        bytes.extend_from_slice(&self.authentication.public_key);
        bytes.extend_from_slice(&self.authentication.signature);
        if bytes.len() != ZK_AMS_MKHE_DECRYPTION_SPLIT_MANIFEST_BYTES_V1 {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        Ok(bytes)
    }

    /// Decode, authenticate, and bind one exact manifest before any component allocation.
    pub fn decode_exact(
        statement: ZkAmsMkheDecryptionStatementV1<'_>,
        bytes: &[u8],
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        if bytes.len() != ZK_AMS_MKHE_DECRYPTION_SPLIT_MANIFEST_BYTES_V1 {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        let mut cursor = 0;
        expect_bytes(bytes, &mut cursor, &DECRYPTION_SPLIT_MANIFEST_TAG_V1)?;
        expect_u8(bytes, &mut cursor, MKHE_VERSION_V1)?;
        let binding = DecryptionBindingV1 {
            profile_digest: read_array::<32>(bytes, &mut cursor)?,
            roster_digest: read_array::<32>(bytes, &mut cursor)?,
            epoch: read_u64(bytes, &mut cursor)?,
            transcript_digest: read_array::<32>(bytes, &mut cursor)?,
            ciphertext_digest: read_array::<32>(bytes, &mut cursor)?,
            key_context_digest: read_array::<32>(bytes, &mut cursor)?,
            statement_binding_digest: read_array::<32>(bytes, &mut cursor)?,
            ciphertext_record_index: read_u32(bytes, &mut cursor)?,
            sample_index: read_u64(bytes, &mut cursor)?,
            party_index: read_u8(bytes, &mut cursor)?,
            party: ZkAmsMkhePartyIdV1::new(read_array::<32>(bytes, &mut cursor)?)?,
            level: read_u8(bytes, &mut cursor)?,
        };
        expect_u8(
            bytes,
            &mut cursor,
            u8::try_from(DECRYPTION_SPLIT_COMPONENT_COUNT_V1)
                .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?,
        )?;
        let polynomial = read_decryption_transport_pointer(
            bytes,
            &mut cursor,
            0,
            ZkAmsMkheDecryptionTransportComponentKindV1::SharePolynomial,
        )?;
        let proof = read_decryption_transport_pointer(
            bytes,
            &mut cursor,
            1,
            ZkAmsMkheDecryptionTransportComponentKindV1::ProofEnvelope,
        )?;
        let manifest_digest = read_array::<32>(bytes, &mut cursor)?;
        let authentication = ArtifactAuthentication {
            version: MKHE_VERSION_V1,
            party: ZkAmsMkhePartyIdV1::new(read_array::<32>(bytes, &mut cursor)?)?,
            public_key: read_array::<33>(bytes, &mut cursor)?,
            signature: read_array::<65>(bytes, &mut cursor)?,
        };
        if cursor != bytes.len() {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        let value = Self {
            binding,
            polynomial,
            proof,
            manifest_digest,
            authentication,
        };
        value.validate_for_statement(statement)?;
        Ok(value)
    }

    /// Exact share-polynomial content address.
    #[must_use]
    pub const fn polynomial(&self) -> ZkAmsMkheDecryptionTransportPointerV1 {
        self.polynomial
    }

    /// Exact native-proof content address.
    #[must_use]
    pub const fn proof(&self) -> ZkAmsMkheDecryptionTransportPointerV1 {
        self.proof
    }

    /// Consensus digest signed by the governed party.
    #[must_use]
    pub const fn manifest_digest(&self) -> [u8; 32] {
        self.manifest_digest
    }

    /// Exact governed roster slot.
    #[must_use]
    pub const fn party_index(&self) -> u8 {
        self.binding.party_index
    }

    /// Authentication-key-derived governed party.
    #[must_use]
    pub const fn party(&self) -> ZkAmsMkhePartyIdV1 {
        self.binding.party
    }

    /// Canonical public authentication material over the complete manifest.
    pub fn authentication(&self) -> Result<ZkAmsMkheAuthenticationWireV1, ZkAmsMkheErrorV1> {
        ZkAmsMkheAuthenticationWireV1::new(
            self.authentication.party,
            self.authentication.public_key,
            self.authentication.signature,
        )
    }
}

fn write_decryption_transport_pointer(
    bytes: &mut Vec<u8>,
    ordinal: u8,
    pointer: ZkAmsMkheDecryptionTransportPointerV1,
) {
    bytes.push(ordinal);
    bytes.push(pointer.kind as u8);
    bytes.extend_from_slice(&pointer.payload_bytes.to_be_bytes());
    bytes.extend_from_slice(&pointer.payload_blake3);
}

fn read_decryption_transport_pointer(
    bytes: &[u8],
    cursor: &mut usize,
    expected_ordinal: u8,
    expected_kind: ZkAmsMkheDecryptionTransportComponentKindV1,
) -> Result<ZkAmsMkheDecryptionTransportPointerV1, ZkAmsMkheErrorV1> {
    expect_u8(bytes, cursor, expected_ordinal)?;
    if ZkAmsMkheDecryptionTransportComponentKindV1::try_from(read_u8(bytes, cursor)?)?
        != expected_kind
    {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    let value = ZkAmsMkheDecryptionTransportPointerV1 {
        kind: expected_kind,
        payload_bytes: read_u64(bytes, cursor)?,
        payload_blake3: read_array::<32>(bytes, cursor)?,
    };
    value.validate_shape()?;
    Ok(value)
}

fn decryption_split_manifest_digest(
    binding: &DecryptionBindingV1,
    polynomial: ZkAmsMkheDecryptionTransportPointerV1,
    proof: ZkAmsMkheDecryptionTransportPointerV1,
) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    polynomial.validate_shape()?;
    proof.validate_shape()?;
    if polynomial.kind != ZkAmsMkheDecryptionTransportComponentKindV1::SharePolynomial
        || proof.kind != ZkAmsMkheDecryptionTransportComponentKindV1::ProofEnvelope
    {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    let mut hash = Keccak256::new();
    hash.update(DECRYPTION_SPLIT_MANIFEST_DOMAIN_V1);
    hash.update(&DECRYPTION_SPLIT_MANIFEST_TAG_V1);
    hash.update(&[MKHE_VERSION_V1]);
    binding.update_hash(&mut hash);
    for (ordinal, pointer) in [(0_u8, polynomial), (1_u8, proof)] {
        hash.update(&[ordinal, pointer.kind as u8]);
        hash.update(&pointer.payload_bytes.to_be_bytes());
        hash.update(&pointer.payload_blake3);
    }
    Ok(hash.finalize())
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct DecryptionPublicRelationV1 {
    binding: DecryptionBindingV1,
    common_a: Arc<RnsPolynomial>,
    party_b: RnsPolynomial,
}

impl DecryptionPublicRelationV1 {
    fn validate(
        &self,
        profile: &BgvProfile,
        parties: &super::PartySet,
    ) -> Result<(), ZkAmsMkheErrorV1> {
        self.binding.validate(profile, parties)?;
        self.common_a.validate(profile)?;
        self.party_b.validate(profile)?;
        if *self.common_a == RnsPolynomial::zero(profile)
            || self.party_b == RnsPolynomial::zero(profile)
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        Ok(())
    }
}

struct DecryptionPartyWitnessV1<'a> {
    binding: DecryptionBindingV1,
    secret: &'a SecretPolynomial,
    public_key_error: &'a SecretPolynomial,
}

impl core::fmt::Debug for DecryptionPartyWitnessV1<'_> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("DecryptionPartyWitnessV1")
            .field("binding", &self.binding)
            .field("secret", &"[REDACTED]")
            .field("public_key_error", &"[REDACTED]")
            .finish()
    }
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

fn update_wire_rns_hash(
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

/// Borrowed release-only statement for exact all-eight governed P24-H decryption.
///
/// The verified aggregate key and its exact eight proof-carrying source shares
/// are retained by reference. The constructor derives `a`, every `b_i`, the
/// authentication-key material, and all share digests from those immutable
/// objects, so callers cannot assert an unrelated raw polynomial context.
#[derive(Clone, Copy)]
pub struct ZkAmsMkheDecryptionStatementV1<'a> {
    roster: &'a ZkAmsMkheGovernedRosterWireV1,
    ciphertext: &'a ZkAmsMkheCollectiveCiphertextWireV1,
    collective_public_key: &'a ZkAmsMkheCollectivePublicKeyV1,
    public_key_shares:
        &'a [&'a ZkAmsMkheCollectivePublicKeyShareV1; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
    key_context_digest: [u8; 32],
}

impl core::fmt::Debug for ZkAmsMkheDecryptionStatementV1<'_> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("ZkAmsMkheDecryptionStatementV1")
            .field("roster_digest", &hex::encode(self.roster.roster_digest()))
            .field("epoch", &self.roster.epoch())
            .field("ciphertext_binding", &self.ciphertext.binding())
            .field("key_context_digest", &hex::encode(self.key_context_digest))
            .field("party_relations", &ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1)
            .finish_non_exhaustive()
    }
}

impl<'a> ZkAmsMkheDecryptionStatementV1<'a> {
    /// Construct the exact release statement from one verified collective key
    /// and the same ordered eight source shares used to aggregate it.
    pub fn new(
        roster: &'a ZkAmsMkheGovernedRosterWireV1,
        ciphertext: &'a ZkAmsMkheCollectiveCiphertextWireV1,
        collective_public_key: &'a ZkAmsMkheCollectivePublicKeyV1,
        public_key_shares: &'a [&'a ZkAmsMkheCollectivePublicKeyShareV1;
                ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        let mut value = Self {
            roster,
            ciphertext,
            collective_public_key,
            public_key_shares,
            key_context_digest: [0; 32],
        };
        value.key_context_digest = value.recompute_key_context_digest()?;
        value.validate()?;
        Ok(value)
    }

    /// Exact governed release roster.
    #[must_use]
    pub const fn roster(&self) -> &'a ZkAmsMkheGovernedRosterWireV1 {
        self.roster
    }

    /// Exact compact collective ciphertext.
    #[must_use]
    pub const fn ciphertext(&self) -> &'a ZkAmsMkheCollectiveCiphertextWireV1 {
        self.ciphertext
    }

    /// Exact common public-key polynomial `a` derived from the verified shares.
    #[must_use]
    pub const fn common_a(&self) -> &'a ZkAmsMkheRnsPolynomialWireV1 {
        self.public_key_shares[0].public_a()
    }

    /// One ordered party public polynomial `b_i`.
    #[must_use]
    pub fn party_public_b(&self, party_index: usize) -> Option<&'a ZkAmsMkheRnsPolynomialWireV1> {
        self.public_key_shares
            .get(party_index)
            .map(|share| share.party_public_b())
    }

    /// Verified all-eight collective public key governing this statement.
    #[must_use]
    pub const fn collective_public_key(&self) -> &'a ZkAmsMkheCollectivePublicKeyV1 {
        self.collective_public_key
    }

    /// Exact ordered proof-carrying public-key shares.
    #[must_use]
    pub const fn public_key_shares(
        &self,
    ) -> &'a [&'a ZkAmsMkheCollectivePublicKeyShareV1; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1] {
        self.public_key_shares
    }

    /// Digest of the verified collective key and every exact ordered public share.
    #[must_use]
    pub const fn key_context_digest(&self) -> [u8; 32] {
        self.key_context_digest
    }

    /// Compact digest of every non-polynomial statement binding axis.
    #[must_use]
    pub fn binding_digest(&self) -> [u8; 32] {
        let binding = self.ciphertext.binding();
        let mut frame = Vec::with_capacity(192);
        frame.extend_from_slice(b"iroha.zk-ams.v1.mkhe.decryption-statement-binding");
        frame.extend_from_slice(&self.roster.profile_digest());
        frame.extend_from_slice(&self.roster.roster_digest());
        frame.extend_from_slice(&self.roster.epoch().to_be_bytes());
        frame.extend_from_slice(&binding.transcript_digest());
        frame.extend_from_slice(&binding.record_index().to_be_bytes());
        frame.extend_from_slice(&self.ciphertext.sample_index().to_be_bytes());
        frame.push(binding.level());
        frame.extend_from_slice(&self.key_context_digest);
        keccak256(&frame)
    }

    /// Native ciphertext digest bound into every party proof and share.
    pub fn ciphertext_digest(&self) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
        let (_, _, ciphertext) = self.internal_ciphertext()?;
        Ok(ciphertext.digest())
    }

    fn validate(&self) -> Result<(), ZkAmsMkheErrorV1> {
        let profile = release_profile_v1();
        let binding = self.ciphertext.binding();
        self.collective_public_key.validate(&profile)?;
        if self.roster.profile_digest() != profile.digest()?
            || binding.profile_digest() != self.roster.profile_digest()
            || binding.roster_digest() != self.roster.roster_digest()
            || binding.epoch() != self.roster.epoch()
            || self.collective_public_key.profile_digest() != self.roster.profile_digest()
            || self.collective_public_key.roster_digest() != self.roster.roster_digest()
            || self.collective_public_key.epoch() != self.roster.epoch()
            || self.collective_public_key.parties().parties != *self.roster.parties()
            || self.collective_public_key.security_certificate_digest()
                != zk_ams_mkhe_security_certificate_v1()?.certificate_digest()
            || self.key_context_digest == [0; 32]
            || self.key_context_digest != self.recompute_key_context_digest()?
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        let common_a = self.common_a();
        common_a.encoded_len()?;
        if common_a.residues().iter().all(|value| *value == 0) {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        for (index, share) in self.public_key_shares.iter().enumerate() {
            if usize::from(share.party_index()) != index
                || share.party() != self.roster.parties()[index]
                || share.digest() != self.collective_public_key.share_digests_internal()[index]
                || share.public_a() != common_a
                || share
                    .party_public_b()
                    .residues()
                    .iter()
                    .all(|value| *value == 0)
            {
                return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
            }
            share.party_public_b().encoded_len()?;
        }
        self.ciphertext.constant().encoded_len()?;
        self.ciphertext.linear().encoded_len()?;
        Ok(())
    }

    fn recompute_key_context_digest(&self) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
        let mut hash = Keccak256::new();
        hash.update(DECRYPTION_KEY_CONTEXT_DOMAIN_V1);
        hash.update(&self.roster.profile_digest());
        hash.update(&self.roster.roster_digest());
        hash.update(&self.roster.epoch().to_be_bytes());
        hash.update(&self.collective_public_key.security_certificate_digest());
        hash.update(&self.collective_public_key.key_material_digest_internal());
        hash.update(&self.collective_public_key.digest());
        update_wire_rns_hash(&mut hash, self.common_a())?;
        for (party, share) in self.roster.parties().iter().zip(self.public_key_shares) {
            hash.update(&party.to_bytes());
            hash.update(&share.digest());
            update_wire_rns_hash(&mut hash, share.party_public_b())?;
        }
        Ok(hash.finalize())
    }

    fn internal_for_party(
        &self,
        party_index: usize,
    ) -> Result<
        (
            BgvProfile,
            super::PartySet,
            ZkAmsMkheCollectiveCiphertextV1,
            RnsPolynomial,
            RnsPolynomial,
        ),
        ZkAmsMkheErrorV1,
    > {
        self.validate()?;
        if party_index >= ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1 {
            return Err(ZkAmsMkheErrorV1::InvalidPartySet);
        }
        let (profile, parties, ciphertext) = self.internal_ciphertext()?;
        let common_a = RnsPolynomial::from_flat(&profile, self.common_a().residues().to_vec())?;
        let party_b = RnsPolynomial::from_flat(
            &profile,
            self.public_key_shares[party_index]
                .party_public_b()
                .residues()
                .to_vec(),
        )?;
        Ok((profile, parties, ciphertext, common_a, party_b))
    }

    fn internal_ciphertext(
        &self,
    ) -> Result<(BgvProfile, super::PartySet, ZkAmsMkheCollectiveCiphertextV1), ZkAmsMkheErrorV1>
    {
        self.validate()?;
        let profile = release_profile_v1();
        let parties = super::PartySet::new(self.roster.parties().to_vec())?;
        let ciphertext =
            ZkAmsMkheCollectiveCiphertextV1::from_release_wire(self.roster, self.ciphertext)?;
        ciphertext.validate(&profile, &parties)?;
        if ciphertext.profile_digest() != self.roster.profile_digest()
            || ciphertext.roster_digest() != self.roster.roster_digest()
            || ciphertext.epoch() != self.roster.epoch()
        {
            return Err(ZkAmsMkheErrorV1::InvalidCiphertext);
        }
        Ok((profile, parties, ciphertext))
    }
}

fn decryption_binding_from_statement(
    statement: ZkAmsMkheDecryptionStatementV1<'_>,
    party_index: usize,
) -> Result<DecryptionBindingV1, ZkAmsMkheErrorV1> {
    statement.validate()?;
    if party_index >= ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1 {
        return Err(ZkAmsMkheErrorV1::InvalidPartySet);
    }
    Ok(DecryptionBindingV1 {
        profile_digest: statement.roster.profile_digest(),
        roster_digest: statement.roster.roster_digest(),
        epoch: statement.roster.epoch(),
        transcript_digest: statement.ciphertext.binding().transcript_digest(),
        ciphertext_digest: statement.ciphertext_digest()?,
        key_context_digest: statement.key_context_digest,
        statement_binding_digest: statement.binding_digest(),
        ciphertext_record_index: statement.ciphertext.binding().record_index(),
        sample_index: statement.ciphertext.sample_index(),
        party_index: u8::try_from(party_index).map_err(|_| ZkAmsMkheErrorV1::InvalidPartySet)?,
        party: statement.roster.parties()[party_index],
        level: statement.ciphertext.binding().level(),
    })
}

/// Canonical native Fiat--Shamir-with-aborts proof of the public-key and share relations.
#[derive(Clone, PartialEq, Eq)]
pub struct ZkAmsMkheDecryptionProofV1 {
    wide_response_bytes: u16,
    challenge_seed: [u8; 32],
    secret_response: Vec<i64>,
    public_key_error_response: Vec<i64>,
    smudge_response: Vec<SignedWideV1>,
}

type DecryptionRelationProofV1 = ZkAmsMkheDecryptionProofV1;

impl core::fmt::Debug for ZkAmsMkheDecryptionProofV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("ZkAmsMkheDecryptionProofV1")
            .field("wide_response_bytes", &self.wide_response_bytes)
            .field("challenge_seed", &hex::encode(self.challenge_seed))
            .field("coefficient_count", &self.secret_response.len())
            .finish_non_exhaustive()
    }
}

impl ZkAmsMkheDecryptionProofV1 {
    /// Exact byte length of the canonical native lattice-proof encoding.
    pub fn encoded_len(&self) -> Result<usize, ZkAmsMkheErrorV1> {
        if self.challenge_seed == [0; 32]
            || self.secret_response.is_empty()
            || self.secret_response.len() != self.public_key_error_response.len()
            || self.secret_response.len() != self.smudge_response.len()
        {
            return Err(ZkAmsMkheErrorV1::InvalidShareProof);
        }
        decryption_proof_bytes(
            self.secret_response.len(),
            usize::from(self.wide_response_bytes),
        )
    }

    /// Encode the proof as the sole canonical `ZADP` byte string.
    pub fn encode(&self) -> Result<Vec<u8>, ZkAmsMkheErrorV1> {
        let length = self.encoded_len()?;
        let degree = self.secret_response.len();
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(length)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        bytes.extend_from_slice(&DECRYPTION_PROOF_TAG_V1);
        bytes.push(MKHE_VERSION_V1);
        bytes.extend_from_slice(&self.wide_response_bytes.to_be_bytes());
        bytes.extend_from_slice(
            &u32::try_from(degree)
                .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?
                .to_be_bytes(),
        );
        bytes.extend_from_slice(&self.challenge_seed);
        for count in [
            self.secret_response.len(),
            self.public_key_error_response.len(),
            self.smudge_response.len(),
        ] {
            bytes.extend_from_slice(
                &u32::try_from(count)
                    .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?
                    .to_be_bytes(),
            );
        }
        for response in &self.secret_response {
            bytes.extend_from_slice(&response.to_be_bytes());
        }
        for response in &self.public_key_error_response {
            bytes.extend_from_slice(&response.to_be_bytes());
        }
        for response in &self.smudge_response {
            response.encode_fixed_into(&mut bytes, usize::from(self.wide_response_bytes))?;
        }
        if bytes.len() != length {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        Ok(bytes)
    }

    fn decode_exact(
        bytes: &[u8],
        expected_degree: usize,
        expected_wide_response_bytes: usize,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        let expected_len = decryption_proof_bytes(expected_degree, expected_wide_response_bytes)?;
        if bytes.len() != expected_len || expected_degree == 0 {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        let mut cursor = 0;
        expect_bytes(bytes, &mut cursor, &DECRYPTION_PROOF_TAG_V1)?;
        expect_u8(bytes, &mut cursor, MKHE_VERSION_V1)?;
        let wide_response_bytes = read_u16(bytes, &mut cursor)?;
        if usize::from(wide_response_bytes) != expected_wide_response_bytes {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        let degree = read_u32(bytes, &mut cursor)?;
        if usize::try_from(degree).ok() != Some(expected_degree) {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        let challenge_seed = read_array::<32>(bytes, &mut cursor)?;
        if challenge_seed == [0; 32] {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        for _ in 0..3 {
            if usize::try_from(read_u32(bytes, &mut cursor)?).ok() != Some(expected_degree) {
                return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
            }
        }
        // Every attacker-controlled count and the complete length have been
        // checked before allocating any response vector.
        let mut secret_response = Vec::new();
        secret_response
            .try_reserve_exact(expected_degree)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        for _ in 0..expected_degree {
            secret_response.push(i64::from_be_bytes(read_array::<8>(bytes, &mut cursor)?));
        }
        let mut public_key_error_response = Vec::new();
        public_key_error_response
            .try_reserve_exact(expected_degree)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        for _ in 0..expected_degree {
            public_key_error_response
                .push(i64::from_be_bytes(read_array::<8>(bytes, &mut cursor)?));
        }
        let mut smudge_response = Vec::new();
        smudge_response
            .try_reserve_exact(expected_degree)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        for _ in 0..expected_degree {
            let end = cursor
                .checked_add(expected_wide_response_bytes)
                .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?;
            smudge_response.push(SignedWideV1::decode_fixed(
                bytes
                    .get(cursor..end)
                    .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?,
            )?);
            cursor = end;
        }
        if cursor != bytes.len() {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        Ok(Self {
            wide_response_bytes,
            challenge_seed,
            secret_response,
            public_key_error_response,
            smudge_response,
        })
    }

    /// Decode the sole release-profile proof shape with all counts preflighted.
    ///
    /// This only establishes canonical syntax and bounds. A decoded proof is
    /// authenticated to context only by [`verify_zk_ams_mkhe_decryption_share_v1`]
    /// or [`reconstruct_zk_ams_mkhe_decryption_share_v1`].
    pub fn decode_release_exact(bytes: &[u8]) -> Result<Self, ZkAmsMkheErrorV1> {
        let profile = release_profile_v1();
        let evidence = derive_decryption_resource_evidence(&profile)?;
        Self::decode_exact(
            bytes,
            profile.ring_degree,
            usize::from(evidence.wide_response_coefficient_bytes),
        )
    }
}

fn decryption_proof_bytes(
    ring_degree: usize,
    wide_response_bytes: usize,
) -> Result<usize, ZkAmsMkheErrorV1> {
    if ring_degree == 0 || wide_response_bytes == 0 {
        return Err(ZkAmsMkheErrorV1::InvalidProfile);
    }
    ring_degree
        .checked_mul(DECRYPTION_SIGNED_SMALL_BYTES_V1)
        .and_then(|bytes| {
            ring_degree
                .checked_mul(DECRYPTION_SIGNED_SMALL_BYTES_V1)
                .and_then(|rhs| bytes.checked_add(rhs))
        })
        .and_then(|bytes| {
            ring_degree
                .checked_mul(wide_response_bytes)
                .and_then(|rhs| bytes.checked_add(rhs))
        })
        .and_then(|bytes| bytes.checked_add(DECRYPTION_PROOF_HEADER_BYTES_V1))
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)
}

fn production_decryption_share_record_bytes(
    profile: &BgvProfile,
    smudge_bits: usize,
) -> Result<(usize, usize), ZkAmsMkheErrorV1> {
    let weight = wide_relation_challenge_weight(profile.ring_degree)?;
    let (_, _, wide_response_bytes) = wide_response_parameters(smudge_bits, weight)?;
    let proof_bytes = decryption_proof_bytes(profile.ring_degree, wide_response_bytes)?;
    let wire_lengths = derive_wire_length_certificate_v1(profile)?;
    let total = wire_lengths
        .streamed_contribution_base_wire_bytes
        .checked_add(wire_lengths.proof_envelope_header_wire_bytes)
        .and_then(|bytes| bytes.checked_add(proof_bytes))
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    Ok((proof_bytes, total))
}

#[cfg(test)]
fn test_decryption_share_record_bytes(
    profile: &BgvProfile,
    smudge_bits: usize,
) -> Result<(usize, usize), ZkAmsMkheErrorV1> {
    let weight = wide_relation_challenge_weight(profile.ring_degree)?;
    let (_, _, wide_response_bytes) = wide_response_parameters(smudge_bits, weight)?;
    let proof_bytes = decryption_proof_bytes(profile.ring_degree, wide_response_bytes)?;
    let total = TEST_DECRYPTION_SHARE_HEADER_BYTES_V1
        .checked_add(checked_rns_polynomial_bytes(profile)?)
        .and_then(|bytes| bytes.checked_add(proof_bytes))
        .and_then(|bytes| bytes.checked_add(TEST_DECRYPTION_AUTHENTICATION_BYTES_V1))
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    Ok((proof_bytes, total))
}

/// One native proof-bound and T256-authenticated governed decryption share.
#[derive(Clone, PartialEq, Eq)]
pub struct ZkAmsMkheAuthenticatedDecryptionShareV1 {
    binding: DecryptionBindingV1,
    share: RnsPolynomial,
    proof: DecryptionRelationProofV1,
    authentication: ArtifactAuthentication,
}

type AuthenticatedDecryptionShareV1 = ZkAmsMkheAuthenticatedDecryptionShareV1;

impl core::fmt::Debug for ZkAmsMkheAuthenticatedDecryptionShareV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("ZkAmsMkheAuthenticatedDecryptionShareV1")
            .field("party_index", &self.binding.party_index)
            .field("party", &self.binding.party)
            .field("share_residue_count", &self.share.coefficients.len())
            .field("proof", &self.proof)
            .finish_non_exhaustive()
    }
}

impl ZkAmsMkheAuthenticatedDecryptionShareV1 {
    /// Governed roster slot authenticated by this share.
    #[must_use]
    pub const fn party_index(&self) -> u8 {
        self.binding.party_index
    }

    /// Authentication-key-derived governed party.
    #[must_use]
    pub const fn party(&self) -> ZkAmsMkhePartyIdV1 {
        self.binding.party
    }

    /// Native proof tied to both the party public-key relation and ciphertext share.
    #[must_use]
    pub const fn proof(&self) -> &ZkAmsMkheDecryptionProofV1 {
        &self.proof
    }

    /// Canonical limb-major partial-decryption residues.
    #[must_use]
    pub fn share_residues(&self) -> &[u64] {
        &self.share.coefficients
    }

    /// Exact standalone native `ZADP` proof bytes used by the split transport.
    pub fn canonical_proof_bytes(&self) -> Result<Vec<u8>, ZkAmsMkheErrorV1> {
        self.proof.encode()
    }

    #[cfg(test)]
    fn record_digest(&self, profile: &BgvProfile) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
        self.share.validate(profile)?;
        let proof = self.proof.encode()?;
        let mut hash = Keccak256::new();
        hash.update(DECRYPTION_SHARE_AUTH_DOMAIN_V1);
        self.binding.update_hash(&mut hash);
        update_rns_hash(&mut hash, profile, &self.share)?;
        hash.update(
            &u32::try_from(proof.len())
                .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?
                .to_be_bytes(),
        );
        hash.update(&proof);
        Ok(hash.finalize())
    }

    #[cfg(test)]
    fn encode(
        &self,
        profile: &BgvProfile,
        smudge_bits: usize,
    ) -> Result<Vec<u8>, ZkAmsMkheErrorV1> {
        let (expected_proof_bytes, expected_total) =
            test_decryption_share_record_bytes(profile, smudge_bits)?;
        if expected_total > profile.max_share_bytes {
            return Err(ZkAmsMkheErrorV1::ResourceCeilingExceeded);
        }
        let proof = self.proof.encode()?;
        if proof.len() != expected_proof_bytes {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        self.share.validate(profile)?;
        self.authentication.verify(
            DECRYPTION_SHARE_AUTH_DOMAIN_V1,
            self.record_digest(profile)?,
        )?;
        if self.authentication.party != self.binding.party {
            return Err(ZkAmsMkheErrorV1::InvalidAuthentication);
        }
        let mut bytes = Vec::with_capacity(expected_total);
        bytes.extend_from_slice(&TEST_DECRYPTION_SHARE_TAG_V1);
        bytes.push(MKHE_VERSION_V1);
        bytes.extend_from_slice(&self.binding.profile_digest);
        bytes.extend_from_slice(&self.binding.roster_digest);
        bytes.extend_from_slice(&self.binding.epoch.to_be_bytes());
        bytes.extend_from_slice(&self.binding.transcript_digest);
        bytes.extend_from_slice(&self.binding.ciphertext_digest);
        bytes.extend_from_slice(&self.binding.key_context_digest);
        bytes.extend_from_slice(&self.binding.statement_binding_digest);
        bytes.extend_from_slice(&self.binding.ciphertext_record_index.to_be_bytes());
        bytes.extend_from_slice(&self.binding.sample_index.to_be_bytes());
        bytes.push(self.binding.party_index);
        bytes.extend_from_slice(&self.binding.party.to_bytes());
        bytes.push(self.binding.level);
        bytes.extend_from_slice(
            &u32::try_from(self.share.coefficients.len())
                .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?
                .to_be_bytes(),
        );
        bytes.extend_from_slice(
            &u32::try_from(proof.len())
                .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?
                .to_be_bytes(),
        );
        for residue in &self.share.coefficients {
            bytes.extend_from_slice(&residue.to_be_bytes());
        }
        bytes.extend_from_slice(&proof);
        bytes.push(self.authentication.version);
        bytes.extend_from_slice(&self.authentication.party.to_bytes());
        bytes.extend_from_slice(&self.authentication.public_key);
        bytes.extend_from_slice(&self.authentication.signature);
        if bytes.len() != expected_total {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        Ok(bytes)
    }

    #[cfg(test)]
    fn decode_exact(
        bytes: &[u8],
        profile: &BgvProfile,
        parties: &super::PartySet,
        expected_binding: &DecryptionBindingV1,
        smudge_bits: usize,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        expected_binding.validate(profile, parties)?;
        let (proof_len, total_len) = test_decryption_share_record_bytes(profile, smudge_bits)?;
        if total_len > profile.max_share_bytes {
            return Err(ZkAmsMkheErrorV1::ResourceCeilingExceeded);
        }
        if bytes.len() != total_len {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        let coefficient_count = profile
            .ring_degree
            .checked_mul(profile.moduli.len())
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        let (_, _, wide_response_bytes) = wide_response_parameters(
            smudge_bits,
            wide_relation_challenge_weight(profile.ring_degree)?,
        )?;
        let mut cursor = 0;
        expect_bytes(bytes, &mut cursor, &TEST_DECRYPTION_SHARE_TAG_V1)?;
        expect_u8(bytes, &mut cursor, MKHE_VERSION_V1)?;
        if read_array::<32>(bytes, &mut cursor)? != expected_binding.profile_digest
            || read_array::<32>(bytes, &mut cursor)? != expected_binding.roster_digest
            || read_u64(bytes, &mut cursor)? != expected_binding.epoch
            || read_array::<32>(bytes, &mut cursor)? != expected_binding.transcript_digest
            || read_array::<32>(bytes, &mut cursor)? != expected_binding.ciphertext_digest
            || read_array::<32>(bytes, &mut cursor)? != expected_binding.key_context_digest
            || read_array::<32>(bytes, &mut cursor)? != expected_binding.statement_binding_digest
            || read_u32(bytes, &mut cursor)? != expected_binding.ciphertext_record_index
            || read_u64(bytes, &mut cursor)? != expected_binding.sample_index
            || read_u8(bytes, &mut cursor)? != expected_binding.party_index
            || read_array::<32>(bytes, &mut cursor)? != expected_binding.party.to_bytes()
            || read_u8(bytes, &mut cursor)? != expected_binding.level
            || usize::try_from(read_u32(bytes, &mut cursor)?).ok() != Some(coefficient_count)
            || usize::try_from(read_u32(bytes, &mut cursor)?).ok() != Some(proof_len)
        {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        // Counts and total length are now frozen; only then allocate the RNS
        // vector and proof responses.
        let mut residues = Vec::new();
        residues
            .try_reserve_exact(coefficient_count)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        for _ in 0..coefficient_count {
            residues.push(read_u64(bytes, &mut cursor)?);
        }
        let share = RnsPolynomial::from_flat(profile, residues)?;
        let proof_end = cursor
            .checked_add(proof_len)
            .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?;
        let proof = DecryptionRelationProofV1::decode_exact(
            bytes
                .get(cursor..proof_end)
                .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?,
            profile.ring_degree,
            wide_response_bytes,
        )?;
        cursor = proof_end;
        let version = read_u8(bytes, &mut cursor)?;
        let party = ZkAmsMkhePartyIdV1::new(read_array::<32>(bytes, &mut cursor)?)?;
        let public_key = read_array::<33>(bytes, &mut cursor)?;
        let signature = read_array::<65>(bytes, &mut cursor)?;
        if cursor != bytes.len() || version != MKHE_VERSION_V1 || party != expected_binding.party {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        let value = Self {
            binding: expected_binding.clone(),
            share,
            proof,
            authentication: ArtifactAuthentication {
                version,
                party,
                public_key,
                signature,
            },
        };
        value.authentication.verify(
            DECRYPTION_SHARE_AUTH_DOMAIN_V1,
            value.record_digest(profile)?,
        )?;
        Ok(value)
    }
}

/// Canonical first-release decryption-share transport.
///
/// The two large objects remain independently reusable and content addressed;
/// only the small authenticated manifest is consensus/header material.
pub struct ZkAmsMkheDecryptionSplitTransportV1 {
    manifest: ZkAmsMkheDecryptionTransportManifestV1,
    polynomial_object: Vec<u8>,
    proof_envelope: Vec<u8>,
}

impl core::fmt::Debug for ZkAmsMkheDecryptionSplitTransportV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("ZkAmsMkheDecryptionSplitTransportV1")
            .field(
                "manifest_digest",
                &hex::encode(self.manifest.manifest_digest),
            )
            .field("polynomial_object_bytes", &self.polynomial_object.len())
            .field("proof_envelope_bytes", &self.proof_envelope.len())
            .finish()
    }
}

impl ZkAmsMkheDecryptionSplitTransportV1 {
    /// Authenticated fixed-width manifest.
    #[must_use]
    pub const fn manifest(&self) -> &ZkAmsMkheDecryptionTransportManifestV1 {
        &self.manifest
    }

    /// Encode the exact 498-byte manifest.
    pub fn manifest_bytes(&self) -> Result<Vec<u8>, ZkAmsMkheErrorV1> {
        self.manifest.encode()
    }

    /// Exact 39,845,892-byte count-prefixed limb-major polynomial object.
    #[must_use]
    pub fn polynomial_object(&self) -> &[u8] {
        &self.polynomial_object
    }

    /// Exact 33,030,199-byte standalone native `ZADP` proof envelope.
    #[must_use]
    pub fn proof_envelope(&self) -> &[u8] {
        &self.proof_envelope
    }

    /// Sole canonical ordered component list: polynomial first, proof second.
    #[must_use]
    pub fn ordered_components(&self) -> [&[u8]; DECRYPTION_SPLIT_COMPONENT_COUNT_V1] {
        [&self.polynomial_object, &self.proof_envelope]
    }
}

fn encode_decryption_polynomial_object(
    profile: &BgvProfile,
    polynomial: &RnsPolynomial,
) -> Result<Vec<u8>, ZkAmsMkheErrorV1> {
    polynomial.validate(profile)?;
    let length = checked_rns_polynomial_bytes(profile)?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(length)
        .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    bytes.extend_from_slice(
        &u32::try_from(polynomial.coefficients.len())
            .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?
            .to_be_bytes(),
    );
    for residue in &polynomial.coefficients {
        bytes.extend_from_slice(&residue.to_be_bytes());
    }
    if bytes.len() != length {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    Ok(bytes)
}

fn decode_decryption_polynomial_object(bytes: &[u8]) -> Result<RnsPolynomial, ZkAmsMkheErrorV1> {
    let profile = release_profile_v1();
    let expected_length = checked_rns_polynomial_bytes(&profile)?;
    if bytes.len() != expected_length || bytes.len() > profile.max_share_bytes {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    let coefficient_count = profile
        .ring_degree
        .checked_mul(profile.moduli.len())
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let mut cursor = 0;
    if usize::try_from(read_u32(bytes, &mut cursor)?).ok() != Some(coefficient_count) {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    // Exact total length and count are frozen before the only large allocation.
    let mut residues = Vec::new();
    residues
        .try_reserve_exact(coefficient_count)
        .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    for _ in 0..coefficient_count {
        residues.push(read_u64(bytes, &mut cursor)?);
    }
    if cursor != bytes.len() {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    RnsPolynomial::from_flat(&profile, residues)
}

fn release_decryption_split_components(
    share: &ZkAmsMkheAuthenticatedDecryptionShareV1,
) -> Result<
    (
        Vec<u8>,
        Vec<u8>,
        ZkAmsMkheDecryptionTransportPointerV1,
        ZkAmsMkheDecryptionTransportPointerV1,
    ),
    ZkAmsMkheErrorV1,
> {
    let profile = release_profile_v1();
    let evidence = derive_decryption_resource_evidence(&profile)?;
    let polynomial_object = encode_decryption_polynomial_object(&profile, &share.share)?;
    let proof_envelope = share.proof.encode()?;
    if u64::try_from(polynomial_object.len()).ok() != Some(evidence.split_polynomial_object_bytes)
        || u64::try_from(proof_envelope.len()).ok() != Some(evidence.split_proof_envelope_bytes)
        || polynomial_object.len() > profile.max_share_bytes
        || proof_envelope.len() > ZK_AMS_MKHE_MAX_PROOF_BYTES_V1
    {
        return Err(ZkAmsMkheErrorV1::WireTooLarge);
    }
    let polynomial_pointer = ZkAmsMkheDecryptionTransportPointerV1::from_payload(
        ZkAmsMkheDecryptionTransportComponentKindV1::SharePolynomial,
        &polynomial_object,
    )?;
    let proof_pointer = ZkAmsMkheDecryptionTransportPointerV1::from_payload(
        ZkAmsMkheDecryptionTransportComponentKindV1::ProofEnvelope,
        &proof_envelope,
    )?;
    Ok((
        polynomial_object,
        proof_envelope,
        polynomial_pointer,
        proof_pointer,
    ))
}

fn release_decryption_split_authentication_digest(
    share: &ZkAmsMkheAuthenticatedDecryptionShareV1,
) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    let (_, _, polynomial, proof) = release_decryption_split_components(share)?;
    decryption_split_manifest_digest(&share.binding, polynomial, proof)
}

fn authenticated_decryption_share_identity(
    profile: &BgvProfile,
    share: &ZkAmsMkheAuthenticatedDecryptionShareV1,
) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    if profile.digest()? == release_profile_v1().digest()? {
        return release_decryption_split_authentication_digest(share);
    }
    #[cfg(test)]
    {
        return share.record_digest(profile);
    }
    #[cfg(not(test))]
    Err(ZkAmsMkheErrorV1::InvalidProfile)
}

/// Materialize the sole canonical first-release split transport from a verified native share.
pub fn split_zk_ams_mkhe_decryption_share_v1(
    statement: ZkAmsMkheDecryptionStatementV1<'_>,
    share: &ZkAmsMkheAuthenticatedDecryptionShareV1,
) -> Result<ZkAmsMkheDecryptionSplitTransportV1, ZkAmsMkheErrorV1> {
    verify_zk_ams_mkhe_decryption_share_v1(statement, share)?;
    let (polynomial_object, proof_envelope, polynomial, proof) =
        release_decryption_split_components(share)?;
    let manifest = ZkAmsMkheDecryptionTransportManifestV1::new(
        share.binding.clone(),
        polynomial,
        proof,
        share.authentication.clone(),
    )?;
    manifest.validate_for_statement(statement)?;
    Ok(ZkAmsMkheDecryptionSplitTransportV1 {
        manifest,
        polynomial_object,
        proof_envelope,
    })
}

/// Authenticate addresses, preflight exact lengths, reconstruct, and verify one native share.
///
/// `components` must contain exactly the polynomial object followed by the
/// proof envelope. The manifest signature is verified before either large
/// component is hashed or decoded.
pub fn reconstruct_zk_ams_mkhe_decryption_share_v1(
    statement: ZkAmsMkheDecryptionStatementV1<'_>,
    manifest_bytes: &[u8],
    components: &[&[u8]],
) -> Result<ZkAmsMkheAuthenticatedDecryptionShareV1, ZkAmsMkheErrorV1> {
    let manifest = ZkAmsMkheDecryptionTransportManifestV1::decode_exact(statement, manifest_bytes)?;
    if components.len() != DECRYPTION_SPLIT_COMPONENT_COUNT_V1 {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    manifest.polynomial.validate_payload(components[0])?;
    manifest.proof.validate_payload(components[1])?;
    let share = decode_decryption_polynomial_object(components[0])?;
    let proof = ZkAmsMkheDecryptionProofV1::decode_release_exact(components[1])?;
    let native = ZkAmsMkheAuthenticatedDecryptionShareV1 {
        binding: manifest.binding,
        share,
        proof,
        authentication: manifest.authentication,
    };
    verify_zk_ams_mkhe_decryption_share_v1(statement, &native)?;
    Ok(native)
}

fn expect_bytes(bytes: &[u8], cursor: &mut usize, expected: &[u8]) -> Result<(), ZkAmsMkheErrorV1> {
    let end = cursor
        .checked_add(expected.len())
        .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?;
    if bytes.get(*cursor..end) != Some(expected) {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    *cursor = end;
    Ok(())
}

fn read_array<const N: usize>(
    bytes: &[u8],
    cursor: &mut usize,
) -> Result<[u8; N], ZkAmsMkheErrorV1> {
    let end = cursor
        .checked_add(N)
        .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?;
    let value = bytes
        .get(*cursor..end)
        .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?
        .try_into()
        .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?;
    *cursor = end;
    Ok(value)
}

fn read_u8(bytes: &[u8], cursor: &mut usize) -> Result<u8, ZkAmsMkheErrorV1> {
    Ok(read_array::<1>(bytes, cursor)?[0])
}

fn expect_u8(bytes: &[u8], cursor: &mut usize, expected: u8) -> Result<(), ZkAmsMkheErrorV1> {
    if read_u8(bytes, cursor)? != expected {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    Ok(())
}

fn read_u16(bytes: &[u8], cursor: &mut usize) -> Result<u16, ZkAmsMkheErrorV1> {
    Ok(u16::from_be_bytes(read_array::<2>(bytes, cursor)?))
}

fn read_u32(bytes: &[u8], cursor: &mut usize) -> Result<u32, ZkAmsMkheErrorV1> {
    Ok(u32::from_be_bytes(read_array::<4>(bytes, cursor)?))
}

fn read_u64(bytes: &[u8], cursor: &mut usize) -> Result<u64, ZkAmsMkheErrorV1> {
    Ok(u64::from_be_bytes(read_array::<8>(bytes, cursor)?))
}

fn validate_party_witness(
    profile: &BgvProfile,
    parties: &super::PartySet,
    relation: &DecryptionPublicRelationV1,
    witness: &DecryptionPartyWitnessV1,
) -> Result<(), ZkAmsMkheErrorV1> {
    relation.validate(profile, parties)?;
    witness.binding.validate(profile, parties)?;
    if witness.binding != relation.binding
        || witness.secret.coefficients.len() != profile.ring_degree
        || witness.public_key_error.coefficients.len() != profile.ring_degree
        || witness
            .secret
            .coefficients
            .iter()
            .any(|coefficient| coefficient.unsigned_abs() > 1)
        || witness
            .public_key_error
            .coefficients
            .iter()
            .any(|coefficient| coefficient.unsigned_abs() > u64::from(profile.error_eta))
    {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    let expected_b = relation
        .common_a
        .mul(&witness.secret.as_rns(profile)?, profile)?
        .negate(profile)?
        .add(
            &witness
                .public_key_error
                .as_rns(profile)?
                .scale_plaintext_modulus(profile)?,
            profile,
        )?;
    if expected_b != relation.party_b {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    Ok(())
}

fn decryption_challenge_seed(
    profile: &BgvProfile,
    smudge_bits: usize,
    relation: &DecryptionPublicRelationV1,
    ciphertext: &ZkAmsMkheCollectiveCiphertextV1,
    share: &RnsPolynomial,
    public_key_commitment: &RnsPolynomial,
    share_commitment: &RnsPolynomial,
) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    share.validate(profile)?;
    public_key_commitment.validate(profile)?;
    share_commitment.validate(profile)?;
    let mut hash = Keccak256::new();
    hash.update(DECRYPTION_CHALLENGE_DOMAIN_V1);
    hash.update(DECRYPTION_PROOF_DOMAIN_V1);
    hash.update(
        &u16::try_from(smudge_bits)
            .map_err(|_| ZkAmsMkheErrorV1::InvalidProfile)?
            .to_be_bytes(),
    );
    hash.update(
        &u16::try_from(wide_relation_challenge_weight(profile.ring_degree)?)
            .map_err(|_| ZkAmsMkheErrorV1::InvalidProfile)?
            .to_be_bytes(),
    );
    hash.update(&WIDE_RELATION_MASK_SLACK_LOG2_V1.to_be_bytes());
    relation.binding.update_hash(&mut hash);
    update_rns_hash(&mut hash, profile, &relation.common_a)?;
    update_rns_hash(&mut hash, profile, &relation.party_b)?;
    update_rns_hash(&mut hash, profile, ciphertext.constant())?;
    update_rns_hash(&mut hash, profile, ciphertext.linear())?;
    update_rns_hash(&mut hash, profile, share)?;
    update_rns_hash(&mut hash, profile, public_key_commitment)?;
    update_rns_hash(&mut hash, profile, share_commitment)?;
    Ok(hash.finalize())
}

#[allow(
    clippy::too_many_arguments,
    reason = "the proof boundary keeps every public relation and private witness input explicit"
)]
fn prove_decryption_relation<R: MaskedRelaxedRandomSourceV1>(
    profile: &BgvProfile,
    parties: &super::PartySet,
    relation: &DecryptionPublicRelationV1,
    witness: &DecryptionPartyWitnessV1,
    ciphertext: &ZkAmsMkheCollectiveCiphertextV1,
    share: &RnsPolynomial,
    smudge: &[SignedWideV1],
    smudge_bits: usize,
    random: &mut R,
) -> Result<DecryptionRelationProofV1, ZkAmsMkheErrorV1> {
    validate_party_witness(profile, parties, relation, witness)?;
    ciphertext.validate(profile, parties)?;
    share.validate(profile)?;
    if ciphertext.digest() != relation.binding.ciphertext_digest
        || ciphertext.sample_index() != relation.binding.sample_index
        || ciphertext.level() != relation.binding.level
        || smudge.len() != profile.ring_degree
    {
        return Err(ZkAmsMkheErrorV1::InvalidShareSet);
    }
    let smudge_bound = WideMagnitudeV1::max_for_bits(smudge_bits)?;
    if smudge.iter().any(|value| value.magnitude > smudge_bound) {
        return Err(ZkAmsMkheErrorV1::InvalidShareProof);
    }
    let expected_share = ciphertext
        .linear()
        .mul(&witness.secret.as_rns(profile)?, profile)?
        .add(
            &wide_vector_as_rns(profile, smudge)?.scale_plaintext_modulus(profile)?,
            profile,
        )?;
    if expected_share != *share {
        return Err(ZkAmsMkheErrorV1::InvalidShareProof);
    }

    let challenge_weight = wide_relation_challenge_weight(profile.ring_degree)?;
    let (secret_mask_bound, secret_response_limit) =
        small_response_parameters(1, challenge_weight, profile)?;
    let (error_mask_bound, error_response_limit) =
        small_response_parameters(i64::from(profile.error_eta), challenge_weight, profile)?;
    let (wide_mask_bound, wide_response_limit, wide_response_bytes) =
        wide_response_parameters(smudge_bits, challenge_weight)?;
    let (expected_proof_bytes, _) = production_decryption_share_record_bytes(profile, smudge_bits)?;
    if expected_proof_bytes > ZK_AMS_MKHE_MAX_PROOF_BYTES_V1 {
        return Err(ZkAmsMkheErrorV1::WireTooLarge);
    }
    if expected_proof_bytes != decryption_proof_bytes(profile.ring_degree, wide_response_bytes)? {
        return Err(ZkAmsMkheErrorV1::InvalidProfile);
    }
    checked_ring_multiplication_work(profile, 8)?;
    validate_wide_relation_random_health(random)?;

    for _ in 0..MAX_RANDOM_REJECTION_ATTEMPTS_V1 {
        let mut secret_mask = (0..profile.ring_degree)
            .map(|_| sample_signed_small(secret_mask_bound, random))
            .collect::<Result<Vec<_>, _>>()?;
        let mut error_mask = (0..profile.ring_degree)
            .map(|_| sample_signed_small(error_mask_bound, random))
            .collect::<Result<Vec<_>, _>>()?;
        let mut smudge_mask = (0..profile.ring_degree)
            .map(|_| sample_signed_wide(&wide_mask_bound, random))
            .collect::<Result<Vec<_>, _>>()?;

        let secret_mask_rns = RnsPolynomial::from_signed(profile, &secret_mask)?;
        let error_mask_rns = RnsPolynomial::from_signed(profile, &error_mask)?;
        let smudge_mask_rns = wide_vector_as_rns(profile, &smudge_mask)?;
        let public_key_commitment = relation
            .common_a
            .mul(&secret_mask_rns, profile)?
            .negate(profile)?
            .add(&error_mask_rns.scale_plaintext_modulus(profile)?, profile)?;
        let share_commitment = ciphertext
            .linear()
            .mul(&secret_mask_rns, profile)?
            .add(&smudge_mask_rns.scale_plaintext_modulus(profile)?, profile)?;
        let challenge_seed = decryption_challenge_seed(
            profile,
            smudge_bits,
            relation,
            ciphertext,
            share,
            &public_key_commitment,
            &share_commitment,
        )?;
        if challenge_seed == [0; 32] {
            secret_mask.fill(0);
            error_mask.fill(0);
            smudge_mask.clear();
            continue;
        }
        let challenge = derive_sparse_challenge(profile.ring_degree, challenge_seed)?;
        let folded_secret = sparse_negacyclic_mul_small(&challenge, &witness.secret.coefficients)?;
        let folded_error =
            sparse_negacyclic_mul_small(&challenge, &witness.public_key_error.coefficients)?;
        let folded_smudge = sparse_negacyclic_mul_wide(&challenge, smudge)?;
        let mut accepted = true;
        let mut secret_response = Vec::with_capacity(profile.ring_degree);
        let mut public_key_error_response = Vec::with_capacity(profile.ring_degree);
        let mut smudge_response = Vec::with_capacity(profile.ring_degree);
        for (mask, folded) in secret_mask.iter().copied().zip(folded_secret) {
            let response = mask
                .checked_add(folded)
                .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
            accepted &= response.unsigned_abs() <= secret_response_limit as u64;
            secret_response.push(response);
        }
        for (mask, folded) in error_mask.iter().copied().zip(folded_error) {
            let response = mask
                .checked_add(folded)
                .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
            accepted &= response.unsigned_abs() <= error_response_limit as u64;
            public_key_error_response.push(response);
        }
        for (mask, folded) in smudge_mask.iter().zip(folded_smudge) {
            let response = mask
                .checked_add(&folded)
                .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
            accepted &= response.magnitude <= wide_response_limit;
            smudge_response.push(response);
        }
        secret_mask.fill(0);
        error_mask.fill(0);
        smudge_mask.clear();
        if !accepted {
            continue;
        }
        let proof = DecryptionRelationProofV1 {
            wide_response_bytes: u16::try_from(wide_response_bytes)
                .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
            challenge_seed,
            secret_response,
            public_key_error_response,
            smudge_response,
        };
        verify_decryption_relation(
            profile,
            parties,
            relation,
            ciphertext,
            share,
            smudge_bits,
            &proof,
        )?;
        return Ok(proof);
    }
    Err(ZkAmsMkheErrorV1::RandomUnavailable)
}

fn verify_decryption_relation(
    profile: &BgvProfile,
    parties: &super::PartySet,
    relation: &DecryptionPublicRelationV1,
    ciphertext: &ZkAmsMkheCollectiveCiphertextV1,
    share: &RnsPolynomial,
    smudge_bits: usize,
    proof: &DecryptionRelationProofV1,
) -> Result<(), ZkAmsMkheErrorV1> {
    relation.validate(profile, parties)?;
    ciphertext.validate(profile, parties)?;
    share.validate(profile)?;
    if ciphertext.digest() != relation.binding.ciphertext_digest
        || proof.challenge_seed == [0; 32]
        || proof.secret_response.len() != profile.ring_degree
        || proof.public_key_error_response.len() != profile.ring_degree
        || proof.smudge_response.len() != profile.ring_degree
    {
        return Err(ZkAmsMkheErrorV1::InvalidShareProof);
    }
    let challenge_weight = wide_relation_challenge_weight(profile.ring_degree)?;
    let (_, secret_response_limit) = small_response_parameters(1, challenge_weight, profile)?;
    let (_, error_response_limit) =
        small_response_parameters(i64::from(profile.error_eta), challenge_weight, profile)?;
    let (_, wide_response_limit, wide_response_bytes) =
        wide_response_parameters(smudge_bits, challenge_weight)?;
    if usize::from(proof.wide_response_bytes) != wide_response_bytes
        || proof
            .secret_response
            .iter()
            .any(|value| value.unsigned_abs() > secret_response_limit as u64)
        || proof
            .public_key_error_response
            .iter()
            .any(|value| value.unsigned_abs() > error_response_limit as u64)
        || proof
            .smudge_response
            .iter()
            .any(|value| value.magnitude > wide_response_limit)
    {
        return Err(ZkAmsMkheErrorV1::InvalidShareProof);
    }
    checked_ring_multiplication_work(profile, 8)?;
    let secret_response = RnsPolynomial::from_signed(profile, &proof.secret_response)?;
    let error_response = RnsPolynomial::from_signed(profile, &proof.public_key_error_response)?;
    let smudge_response = wide_vector_as_rns(profile, &proof.smudge_response)?;
    let challenge = derive_sparse_challenge(profile.ring_degree, proof.challenge_seed)?;
    let challenge_i64 = challenge
        .iter()
        .map(|value| i64::from(*value))
        .collect::<Vec<_>>();
    let challenge_rns = RnsPolynomial::from_signed(profile, &challenge_i64)?;

    let public_key_commitment = relation
        .common_a
        .mul(&secret_response, profile)?
        .negate(profile)?
        .add(&error_response.scale_plaintext_modulus(profile)?, profile)?
        .sub(&relation.party_b.mul(&challenge_rns, profile)?, profile)?;
    let share_commitment = ciphertext
        .linear()
        .mul(&secret_response, profile)?
        .add(&smudge_response.scale_plaintext_modulus(profile)?, profile)?
        .sub(&share.mul(&challenge_rns, profile)?, profile)?;
    let expected = decryption_challenge_seed(
        profile,
        smudge_bits,
        relation,
        ciphertext,
        share,
        &public_key_commitment,
        &share_commitment,
    )?;
    if expected != proof.challenge_seed {
        return Err(ZkAmsMkheErrorV1::InvalidShareProof);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn create_decryption_share<R: MaskedRelaxedRandomSourceV1>(
    profile: &BgvProfile,
    parties: &super::PartySet,
    relation: &DecryptionPublicRelationV1,
    witness: &DecryptionPartyWitnessV1,
    ciphertext: &ZkAmsMkheCollectiveCiphertextV1,
    smudge_bits: usize,
    random: &mut R,
) -> Result<AuthenticatedDecryptionShareV1, ZkAmsMkheErrorV1> {
    validate_party_witness(profile, parties, relation, witness)?;
    ciphertext.validate(profile, parties)?;
    let (proof_bytes, _) = production_decryption_share_record_bytes(profile, smudge_bits)?;
    if proof_bytes > ZK_AMS_MKHE_MAX_PROOF_BYTES_V1 {
        return Err(ZkAmsMkheErrorV1::WireTooLarge);
    }
    validate_wide_relation_random_health(random)?;
    let smudge_bound = WideMagnitudeV1::max_for_bits(smudge_bits)?;
    let mut smudge = (0..profile.ring_degree)
        .map(|_| sample_signed_wide(&smudge_bound, random))
        .collect::<Result<Vec<_>, _>>()?;
    let share = ciphertext
        .linear()
        .mul(&witness.secret.as_rns(profile)?, profile)?
        .add(
            &wide_vector_as_rns(profile, &smudge)?.scale_plaintext_modulus(profile)?,
            profile,
        )?;
    let proof = prove_decryption_relation(
        profile,
        parties,
        relation,
        witness,
        ciphertext,
        &share,
        &smudge,
        smudge_bits,
        random,
    )?;
    smudge.clear();
    let placeholder = ArtifactAuthentication {
        version: MKHE_VERSION_V1,
        party: relation.binding.party,
        public_key: [0; 33],
        signature: [0; 65],
    };
    Ok(AuthenticatedDecryptionShareV1 {
        binding: relation.binding.clone(),
        share,
        proof,
        authentication: placeholder,
    })
}

#[cfg(test)]
#[allow(clippy::too_many_arguments)]
fn create_authenticated_decryption_share<R: MaskedRelaxedRandomSourceV1>(
    profile: &BgvProfile,
    parties: &super::PartySet,
    relation: &DecryptionPublicRelationV1,
    witness: &DecryptionPartyWitnessV1,
    ciphertext: &ZkAmsMkheCollectiveCiphertextV1,
    authentication_secret: &AuthenticationSecret,
    smudge_bits: usize,
    random: &mut R,
) -> Result<AuthenticatedDecryptionShareV1, ZkAmsMkheErrorV1> {
    if authentication_secret.party_id()? != relation.binding.party {
        return Err(ZkAmsMkheErrorV1::InvalidAuthentication);
    }
    let mut record = create_decryption_share(
        profile,
        parties,
        relation,
        witness,
        ciphertext,
        smudge_bits,
        random,
    )?;
    record.authentication = ArtifactAuthentication::sign(
        DECRYPTION_SHARE_AUTH_DOMAIN_V1,
        record.record_digest(profile)?,
        authentication_secret,
        random,
    )?;
    Ok(record)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct OpaquePartyStateContextV1 {
    profile_digest: [u8; 32],
    security_certificate_digest: [u8; 32],
    roster_digest: [u8; 32],
    key_material_digest: [u8; 32],
    epoch: u64,
    transcript_digest: [u8; 32],
    party_index: u8,
    party: ZkAmsMkhePartyIdV1,
    public_share_digest: [u8; 32],
}

fn expected_opaque_party_state_context(
    statement: ZkAmsMkheDecryptionStatementV1<'_>,
    party_index: usize,
) -> Result<OpaquePartyStateContextV1, ZkAmsMkheErrorV1> {
    if party_index >= ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1 {
        return Err(ZkAmsMkheErrorV1::InvalidPartySet);
    }
    Ok(OpaquePartyStateContextV1 {
        profile_digest: statement.roster.profile_digest(),
        security_certificate_digest: statement
            .collective_public_key
            .security_certificate_digest(),
        roster_digest: statement.roster.roster_digest(),
        key_material_digest: statement
            .collective_public_key
            .key_material_digest_internal(),
        epoch: statement.roster.epoch(),
        transcript_digest: statement.collective_public_key.transcript_digest(),
        party_index: u8::try_from(party_index).map_err(|_| ZkAmsMkheErrorV1::InvalidPartySet)?,
        party: statement.roster.parties()[party_index],
        public_share_digest: statement.public_key_shares[party_index].digest(),
    })
}

fn validate_opaque_party_state_context(
    party_state: &ZkAmsMkheCollectivePartyStateV1,
    expected: OpaquePartyStateContextV1,
) -> Result<(), ZkAmsMkheErrorV1> {
    if party_state.profile_digest_internal() != expected.profile_digest
        || party_state.security_certificate_digest_internal()
            != expected.security_certificate_digest
        || party_state.roster_digest_internal() != expected.roster_digest
        || party_state.key_material_digest_internal() != expected.key_material_digest
        || party_state.epoch() != expected.epoch
        || party_state.transcript_digest() != expected.transcript_digest
        || party_state.party_index() != expected.party_index
        || party_state.party() != expected.party
        || party_state.public_share_digest() != expected.public_share_digest
    {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    Ok(())
}

/// Create one authenticated native P24-H partial-decryption share.
///
/// Materialize its sole canonical public representation with
/// [`split_zk_ams_mkhe_decryption_share_v1`]. The polynomial and proof are
/// admitted independently under their unchanged governed ceilings.
pub fn prove_zk_ams_mkhe_decryption_share_v1<R: MaskedRelaxedRandomSourceV1>(
    statement: ZkAmsMkheDecryptionStatementV1<'_>,
    party_index: usize,
    party_state: &ZkAmsMkheCollectivePartyStateV1,
    party_secret: &ZkAmsMkheActivePartySecretV1,
    random: &mut R,
) -> Result<ZkAmsMkheAuthenticatedDecryptionShareV1, ZkAmsMkheErrorV1> {
    statement.validate()?;
    if party_index >= ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1 {
        return Err(ZkAmsMkheErrorV1::InvalidPartySet);
    }
    let expected_context = expected_opaque_party_state_context(statement, party_index)?;
    let expected_party = expected_context.party;
    if party_secret.party()? != expected_party {
        return Err(ZkAmsMkheErrorV1::InvalidAuthentication);
    }
    validate_opaque_party_state_context(party_state, expected_context)?;
    let (profile, parties, ciphertext, common_a, party_b) =
        statement.internal_for_party(party_index)?;
    let ciphertext_digest = ciphertext.digest();
    let binding = DecryptionBindingV1 {
        profile_digest: statement.roster.profile_digest(),
        roster_digest: statement.roster.roster_digest(),
        epoch: statement.roster.epoch(),
        transcript_digest: statement.ciphertext.binding().transcript_digest(),
        ciphertext_digest,
        key_context_digest: statement.key_context_digest,
        statement_binding_digest: statement.binding_digest(),
        ciphertext_record_index: statement.ciphertext.binding().record_index(),
        sample_index: statement.ciphertext.sample_index(),
        party_index: u8::try_from(party_index).map_err(|_| ZkAmsMkheErrorV1::InvalidPartySet)?,
        party: parties.parties[party_index],
        level: statement.ciphertext.binding().level(),
    };
    let relation = DecryptionPublicRelationV1 {
        binding: binding.clone(),
        common_a: Arc::new(common_a),
        party_b,
    };
    let native_witness = DecryptionPartyWitnessV1 {
        binding,
        secret: party_state.secret(),
        public_key_error: party_state.public_error(),
    };
    let smudge_bits =
        usize::from(zk_ams_mkhe_noise_certificate_v1()?.decryption_smudge_quotient_bits);
    let mut record = create_decryption_share(
        &profile,
        &parties,
        &relation,
        &native_witness,
        &ciphertext,
        smudge_bits,
        random,
    )?;
    let manifest_digest = release_decryption_split_authentication_digest(&record)?;
    record.authentication = party_secret.authenticate_artifact(
        DECRYPTION_SPLIT_MANIFEST_AUTH_DOMAIN_V1,
        manifest_digest,
        random,
    )?;
    Ok(record)
}

/// Verify one authenticated native share against its exact governed statement.
pub fn verify_zk_ams_mkhe_decryption_share_v1(
    statement: ZkAmsMkheDecryptionStatementV1<'_>,
    share: &ZkAmsMkheAuthenticatedDecryptionShareV1,
) -> Result<(), ZkAmsMkheErrorV1> {
    let party_index = usize::from(share.binding.party_index);
    if party_index >= ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1 {
        return Err(ZkAmsMkheErrorV1::InvalidPartySet);
    }
    statement.validate()?;
    let public_binding = statement.ciphertext.binding();
    let ciphertext_digest = statement.ciphertext_digest()?;
    if share.binding.profile_digest != statement.roster.profile_digest()
        || share.binding.roster_digest != statement.roster.roster_digest()
        || share.binding.epoch != statement.roster.epoch()
        || share.binding.transcript_digest != public_binding.transcript_digest()
        || share.binding.ciphertext_digest != ciphertext_digest
        || share.binding.key_context_digest != statement.key_context_digest
        || share.binding.statement_binding_digest != statement.binding_digest()
        || share.binding.ciphertext_record_index != public_binding.record_index()
        || share.binding.sample_index != statement.ciphertext.sample_index()
        || share.binding.party != statement.roster.parties()[party_index]
        || share.binding.level != public_binding.level()
    {
        return Err(ZkAmsMkheErrorV1::InvalidShareSet);
    }
    let (profile, parties, ciphertext, common_a, party_b) =
        statement.internal_for_party(party_index)?;
    if ciphertext.digest() != ciphertext_digest {
        return Err(ZkAmsMkheErrorV1::InvalidCiphertext);
    }
    let expected_binding = DecryptionBindingV1 {
        profile_digest: statement.roster.profile_digest(),
        roster_digest: statement.roster.roster_digest(),
        epoch: statement.roster.epoch(),
        transcript_digest: statement.ciphertext.binding().transcript_digest(),
        ciphertext_digest,
        key_context_digest: statement.key_context_digest,
        statement_binding_digest: statement.binding_digest(),
        ciphertext_record_index: statement.ciphertext.binding().record_index(),
        sample_index: statement.ciphertext.sample_index(),
        party_index: share.binding.party_index,
        party: parties.parties[party_index],
        level: statement.ciphertext.binding().level(),
    };
    let relation = DecryptionPublicRelationV1 {
        binding: expected_binding.clone(),
        common_a: Arc::new(common_a),
        party_b,
    };
    verify_authenticated_share(
        &profile,
        &parties,
        &relation,
        &ciphertext,
        &expected_binding,
        share,
        usize::from(zk_ams_mkhe_noise_certificate_v1()?.decryption_smudge_quotient_bits),
    )
}

/// Verify an exact ordered all-eight share set, aggregate in `R_Q`, center by
/// CRT, enforce the certified residual bound, and recover canonical T256 values.
pub fn verify_combine_decode_zk_ams_mkhe_decryption_v1(
    statement: ZkAmsMkheDecryptionStatementV1<'_>,
    shares: &[ZkAmsMkheAuthenticatedDecryptionShareV1],
) -> Result<ZkAmsMkheFullRosterDecryptionResultV1, ZkAmsMkheIdentifiableDecryptionAbortV1> {
    let profile = release_profile_v1();
    let parties = super::PartySet::new(statement.roster.parties().to_vec()).map_err(|_| {
        // The public roster constructor already guarantees a nonempty exact
        // roster; this branch only guards against an internal invariant break.
        let fallback = super::PartySet {
            parties: statement.roster.parties().to_vec(),
            digest: statement.roster.roster_digest(),
        };
        identifiable_abort(
            &fallback,
            0,
            DecryptionAbortReasonV1::BindingMismatch,
            [0; 32],
        )
    })?;
    let binding_failure = |index: usize, digest: [u8; 32]| {
        identifiable_abort(
            &parties,
            index,
            DecryptionAbortReasonV1::BindingMismatch,
            digest,
        )
    };
    if shares.len() != ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1 {
        return Err(identifiable_abort(
            &parties,
            shares.len(),
            if shares.len() < ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1 {
                DecryptionAbortReasonV1::MissingShare
            } else {
                DecryptionAbortReasonV1::ExcessShare
            },
            statement.binding_digest(),
        ));
    }
    statement
        .validate()
        .map_err(|_| binding_failure(0, [0; 32]))?;
    let binding = statement.ciphertext.binding();
    let ciphertext =
        ZkAmsMkheCollectiveCiphertextV1::from_release_wire(statement.roster, statement.ciphertext)
            .map_err(|_| binding_failure(0, [0; 32]))?;
    ciphertext
        .validate(&profile, &parties)
        .map_err(|_| binding_failure(0, [0; 32]))?;
    let ciphertext_digest = ciphertext.digest();
    for (index, share) in shares.iter().enumerate() {
        if usize::from(share.binding.party_index) != index
            || share.binding.party != parties.parties[index]
        {
            return Err(identifiable_abort(
                &parties,
                index,
                DecryptionAbortReasonV1::ReorderedOrDuplicateShare,
                ciphertext_digest,
            ));
        }
        if share.binding.profile_digest != statement.roster.profile_digest()
            || share.binding.roster_digest != statement.roster.roster_digest()
            || share.binding.epoch != statement.roster.epoch()
            || share.binding.transcript_digest != binding.transcript_digest()
            || share.binding.ciphertext_digest != ciphertext_digest
            || share.binding.key_context_digest != statement.key_context_digest
            || share.binding.statement_binding_digest != statement.binding_digest()
            || share.binding.ciphertext_record_index != binding.record_index()
            || share.binding.sample_index != statement.ciphertext.sample_index()
            || share.binding.level != binding.level()
        {
            return Err(binding_failure(index, ciphertext_digest));
        }
    }
    let common_a = Arc::new(
        RnsPolynomial::from_flat(&profile, statement.common_a().residues().to_vec())
            .map_err(|_| binding_failure(0, ciphertext_digest))?,
    );
    let mut relations = Vec::with_capacity(ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1);
    for index in 0..ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1 {
        let party_b = RnsPolynomial::from_flat(
            &profile,
            statement.public_key_shares[index]
                .party_public_b()
                .residues()
                .to_vec(),
        )
        .map_err(|_| binding_failure(index, ciphertext_digest))?;
        relations.push(DecryptionPublicRelationV1 {
            binding: DecryptionBindingV1 {
                profile_digest: statement.roster.profile_digest(),
                roster_digest: statement.roster.roster_digest(),
                epoch: statement.roster.epoch(),
                transcript_digest: binding.transcript_digest(),
                ciphertext_digest,
                key_context_digest: statement.key_context_digest,
                statement_binding_digest: statement.binding_digest(),
                ciphertext_record_index: binding.record_index(),
                sample_index: statement.ciphertext.sample_index(),
                party_index: u8::try_from(index).unwrap_or(u8::MAX),
                party: parties.parties[index],
                level: binding.level(),
            },
            common_a: Arc::clone(&common_a),
            party_b,
        });
    }
    let noise =
        zk_ams_mkhe_noise_certificate_v1().map_err(|_| binding_failure(0, ciphertext_digest))?;
    aggregate_and_decrypt_full_roster(
        &profile,
        &parties,
        &relations,
        &ciphertext,
        shares,
        usize::from(noise.decryption_smudge_quotient_bits),
        usize::from(noise.final_decryption_residual_bits),
    )
}

/// Deterministic reason attached to the first rejected governed roster slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ZkAmsMkheDecryptionAbortReasonV1 {
    /// A required governed party share was absent.
    MissingShare = 1,
    /// More than the exact governed eight shares were supplied.
    ExcessShare = 2,
    /// The first offending share was duplicated or appeared out of order.
    ReorderedOrDuplicateShare = 3,
    /// A profile, roster, epoch, transcript, ciphertext, index, or level differed.
    BindingMismatch = 4,
    /// The party authentication failed.
    AuthenticationFailure = 5,
    /// The native RNS public-key/share relation proof failed.
    ProofFailure = 6,
    /// The centered aggregate exceeded the certified correctness bound.
    CorrectnessBoundExceeded = 7,
}

type DecryptionAbortReasonV1 = ZkAmsMkheDecryptionAbortReasonV1;

/// Auditable first-offender evidence returned instead of a threshold fallback.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ZkAmsMkheIdentifiableDecryptionAbortV1 {
    /// Canonical governed roster slot of the first offender.
    pub party_index: u8,
    /// Authentication-key-derived party at that slot.
    pub party: ZkAmsMkhePartyIdV1,
    /// Deterministic rejection category.
    pub reason: ZkAmsMkheDecryptionAbortReasonV1,
    /// Domain-separated evidence digest for audit logging.
    pub evidence_digest: [u8; 32],
}

type IdentifiableDecryptionAbortV1 = ZkAmsMkheIdentifiableDecryptionAbortV1;

fn identifiable_abort(
    parties: &super::PartySet,
    party_index: usize,
    reason: DecryptionAbortReasonV1,
    binding_digest: [u8; 32],
) -> IdentifiableDecryptionAbortV1 {
    let bounded_index = party_index.min(parties.parties.len().saturating_sub(1));
    let party = parties.parties[bounded_index];
    let mut frame = Vec::with_capacity(160);
    frame.extend_from_slice(b"iroha.zk-ams.v1.mkhe.identifiable-decryption-abort");
    frame.extend_from_slice(&parties.digest);
    frame.extend_from_slice(&(party_index as u64).to_be_bytes());
    frame.extend_from_slice(&party.to_bytes());
    frame.push(reason as u8);
    frame.extend_from_slice(&binding_digest);
    IdentifiableDecryptionAbortV1 {
        party_index: u8::try_from(bounded_index).unwrap_or(u8::MAX),
        party,
        reason,
        evidence_digest: keccak256(&frame),
    }
}

/// Canonical plaintext recovered after CRT centering and final modulus reduction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ZkAmsMkheDecryptedPlaintextV1 {
    /// Tiny-profile residues used only by exhaustive native arithmetic tests.
    #[cfg(test)]
    Tiny(Vec<u64>),
    /// Canonical T256 scalar bytes, one per ring coefficient.
    T256(Vec<[u8; 32]>),
}

type DecryptedPlaintextV1 = ZkAmsMkheDecryptedPlaintextV1;

/// Result of verifying and combining the sole exact ordered all-eight share set.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ZkAmsMkheFullRosterDecryptionResultV1 {
    /// Canonically recovered plaintext coefficients.
    pub plaintext: ZkAmsMkheDecryptedPlaintextV1,
    /// Largest centered residual bit length observed before T256 reduction.
    pub maximum_residual_bits: u16,
    /// Digest of the exact verified ordered eight-share set.
    pub ordered_share_set_digest: [u8; 32],
}

type DecryptionResultV1 = ZkAmsMkheFullRosterDecryptionResultV1;

fn verify_authenticated_share(
    profile: &BgvProfile,
    parties: &super::PartySet,
    relation: &DecryptionPublicRelationV1,
    ciphertext: &ZkAmsMkheCollectiveCiphertextV1,
    expected_binding: &DecryptionBindingV1,
    share: &AuthenticatedDecryptionShareV1,
    smudge_bits: usize,
) -> Result<(), ZkAmsMkheErrorV1> {
    expected_binding.validate(profile, parties)?;
    relation.validate(profile, parties)?;
    ciphertext.validate(profile, parties)?;
    if share.binding != *expected_binding
        || relation.binding != *expected_binding
        || share.authentication.party != expected_binding.party
        || ciphertext.digest() != expected_binding.ciphertext_digest
    {
        return Err(ZkAmsMkheErrorV1::InvalidShareSet);
    }
    let release = profile.digest()? == release_profile_v1().digest()?;
    let identity = authenticated_decryption_share_identity(profile, share)?;
    if release {
        share
            .authentication
            .verify(DECRYPTION_SPLIT_MANIFEST_AUTH_DOMAIN_V1, identity)?;
    } else {
        share
            .authentication
            .verify(DECRYPTION_SHARE_AUTH_DOMAIN_V1, identity)?;
    }
    verify_decryption_relation(
        profile,
        parties,
        relation,
        ciphertext,
        &share.share,
        smudge_bits,
        &share.proof,
    )
}

fn aggregate_and_decrypt_full_roster(
    profile: &BgvProfile,
    parties: &super::PartySet,
    relations: &[DecryptionPublicRelationV1],
    ciphertext: &ZkAmsMkheCollectiveCiphertextV1,
    shares: &[AuthenticatedDecryptionShareV1],
    smudge_bits: usize,
    final_residual_bits: usize,
) -> Result<DecryptionResultV1, IdentifiableDecryptionAbortV1> {
    ciphertext.validate(profile, parties).map_err(|_| {
        identifiable_abort(
            parties,
            0,
            DecryptionAbortReasonV1::BindingMismatch,
            [0; 32],
        )
    })?;
    let ciphertext_digest = ciphertext.digest();
    if relations.len() != ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1 {
        return Err(identifiable_abort(
            parties,
            relations.len(),
            if relations.len() < ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1 {
                DecryptionAbortReasonV1::MissingShare
            } else {
                DecryptionAbortReasonV1::ExcessShare
            },
            ciphertext_digest,
        ));
    }
    if shares.len() != ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1 {
        return Err(identifiable_abort(
            parties,
            shares.len(),
            if shares.len() < ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1 {
                DecryptionAbortReasonV1::MissingShare
            } else {
                DecryptionAbortReasonV1::ExcessShare
            },
            ciphertext_digest,
        ));
    }
    let mut aggregate = ciphertext.constant().clone();
    let mut set_hash = Keccak256::new();
    set_hash.update(DECRYPTION_SET_DOMAIN_V1);
    set_hash.update(&ciphertext.roster_digest());
    set_hash.update(&ciphertext_digest);
    for index in 0..ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1 {
        let expected_binding = DecryptionBindingV1 {
            profile_digest: profile.digest().map_err(|_| {
                identifiable_abort(
                    parties,
                    index,
                    DecryptionAbortReasonV1::BindingMismatch,
                    ciphertext_digest,
                )
            })?,
            roster_digest: ciphertext.roster_digest(),
            epoch: ciphertext.epoch(),
            transcript_digest: ciphertext.transcript_digest(),
            ciphertext_digest,
            key_context_digest: relations[index].binding.key_context_digest,
            statement_binding_digest: relations[index].binding.statement_binding_digest,
            ciphertext_record_index: relations[index].binding.ciphertext_record_index,
            sample_index: ciphertext.sample_index(),
            party_index: u8::try_from(index).map_err(|_| {
                identifiable_abort(
                    parties,
                    index,
                    DecryptionAbortReasonV1::BindingMismatch,
                    ciphertext_digest,
                )
            })?,
            party: parties.parties[index],
            level: ciphertext.level(),
        };
        if shares[index].binding.party_index != expected_binding.party_index
            || shares[index].binding.party != expected_binding.party
            || relations[index].binding.party_index != expected_binding.party_index
            || relations[index].binding.party != expected_binding.party
        {
            return Err(identifiable_abort(
                parties,
                index,
                DecryptionAbortReasonV1::ReorderedOrDuplicateShare,
                ciphertext_digest,
            ));
        }
        match verify_authenticated_share(
            profile,
            parties,
            &relations[index],
            ciphertext,
            &expected_binding,
            &shares[index],
            smudge_bits,
        ) {
            Ok(()) => {}
            Err(ZkAmsMkheErrorV1::InvalidAuthentication) => {
                return Err(identifiable_abort(
                    parties,
                    index,
                    DecryptionAbortReasonV1::AuthenticationFailure,
                    ciphertext_digest,
                ));
            }
            Err(ZkAmsMkheErrorV1::InvalidShareProof) => {
                return Err(identifiable_abort(
                    parties,
                    index,
                    DecryptionAbortReasonV1::ProofFailure,
                    ciphertext_digest,
                ));
            }
            Err(_) => {
                return Err(identifiable_abort(
                    parties,
                    index,
                    DecryptionAbortReasonV1::BindingMismatch,
                    ciphertext_digest,
                ));
            }
        }
        aggregate = aggregate.add(&shares[index].share, profile).map_err(|_| {
            identifiable_abort(
                parties,
                index,
                DecryptionAbortReasonV1::CorrectnessBoundExceeded,
                ciphertext_digest,
            )
        })?;
        set_hash.update(&[u8::try_from(index).unwrap_or(u8::MAX)]);
        set_hash.update(
            &authenticated_decryption_share_identity(profile, &shares[index]).map_err(|_| {
                identifiable_abort(
                    parties,
                    index,
                    DecryptionAbortReasonV1::ProofFailure,
                    ciphertext_digest,
                )
            })?,
        );
    }
    let (plaintext, maximum_residual_bits) =
        decode_centered_plaintext(profile, &aggregate, final_residual_bits).map_err(|_| {
            identifiable_abort(
                parties,
                0,
                DecryptionAbortReasonV1::CorrectnessBoundExceeded,
                ciphertext_digest,
            )
        })?;
    Ok(DecryptionResultV1 {
        plaintext,
        maximum_residual_bits,
        ordered_share_set_digest: set_hash.finalize(),
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SignedCrtV1 {
    negative: bool,
    magnitude: WideUint,
}

impl SignedCrtV1 {
    fn normalized(negative: bool, magnitude: WideUint) -> Self {
        Self {
            negative: negative && magnitude != WideUint::zero(),
            magnitude,
        }
    }

    fn subtract(self, rhs: Self) -> Result<Self, ZkAmsMkheErrorV1> {
        if self.negative == rhs.negative {
            match self.magnitude.cmp(&rhs.magnitude) {
                Ordering::Greater | Ordering::Equal => Ok(Self::normalized(
                    self.negative,
                    self.magnitude
                        .checked_sub(rhs.magnitude)
                        .ok_or(ZkAmsMkheErrorV1::DecryptionBoundExceeded)?,
                )),
                Ordering::Less => Ok(Self::normalized(
                    !self.negative,
                    rhs.magnitude
                        .checked_sub(self.magnitude)
                        .ok_or(ZkAmsMkheErrorV1::DecryptionBoundExceeded)?,
                )),
            }
        } else {
            let magnitude = wide_checked_add(self.magnitude, rhs.magnitude)?;
            Ok(Self::normalized(self.negative, magnitude))
        }
    }
}

fn wide_checked_add(left: WideUint, right: WideUint) -> Result<WideUint, ZkAmsMkheErrorV1> {
    let mut output = WideUint::zero();
    let mut carry = 0_u128;
    for index in 0..super::WIDE_LIMBS {
        let sum = u128::from(left.limbs[index]) + u128::from(right.limbs[index]) + carry;
        output.limbs[index] = sum as u64;
        carry = sum >> 64;
    }
    if carry != 0 {
        return Err(ZkAmsMkheErrorV1::ResourceCeilingExceeded);
    }
    Ok(output)
}

fn centered_crt(
    residues: &[u64],
    moduli: &[u64],
    modulus_product: WideUint,
) -> Result<SignedCrtV1, ZkAmsMkheErrorV1> {
    let reconstructed = WideUint::crt(residues, moduli)?;
    if reconstructed > modulus_product.shr_one() {
        Ok(SignedCrtV1::normalized(
            true,
            modulus_product
                .checked_sub(reconstructed)
                .ok_or(ZkAmsMkheErrorV1::DecryptionBoundExceeded)?,
        ))
    } else {
        Ok(SignedCrtV1::normalized(false, reconstructed))
    }
}

fn wide_from_be(bytes: &[u8]) -> Result<WideUint, ZkAmsMkheErrorV1> {
    if bytes.len() > super::WIDE_LIMBS * 8 {
        return Err(ZkAmsMkheErrorV1::ResourceCeilingExceeded);
    }
    let mut value = WideUint::zero();
    for (index, byte) in bytes.iter().rev().enumerate() {
        value.limbs[index / 8] |= u64::from(*byte) << ((index % 8) * 8);
    }
    Ok(value)
}

#[cfg(test)]
fn wide_from_u64(value: u64) -> WideUint {
    let mut output = WideUint::zero();
    output.limbs[0] = value;
    output
}

fn reduce_wide_mod_t256(value: WideUint) -> [u8; 32] {
    let mut modulus = [0_u64; 4];
    for (index, chunk) in super::VEGA_T256_SCALAR_MODULUS_BE_V1
        .rchunks_exact(8)
        .enumerate()
    {
        modulus[index] = u64::from_be_bytes(chunk.try_into().expect("eight-byte chunk"));
    }
    let mut remainder = [0_u64; 4];
    for bit in (0..value.bit_len()).rev() {
        let incoming = (value.limbs[bit / 64] >> (bit % 64)) & 1;
        let mut carry = incoming;
        for limb in &mut remainder {
            let next = *limb >> 63;
            *limb = (*limb << 1) | carry;
            carry = next;
        }
        if carry != 0 {
            // `remainder < p` before the shift, hence the 257-bit shifted
            // value is below `2p`. Subtracting `p` exactly once must borrow
            // from the captured high bit and leave a canonical 256-bit value.
            let borrowed = subtract_u256(&mut remainder, &modulus);
            debug_assert!(borrowed);
        } else if compare_u256(&remainder, &modulus) != Ordering::Less {
            let borrowed = subtract_u256(&mut remainder, &modulus);
            debug_assert!(!borrowed);
        }
    }
    let mut output = [0_u8; 32];
    for (index, limb) in remainder.iter().enumerate() {
        output[(3 - index) * 8..(4 - index) * 8].copy_from_slice(&limb.to_be_bytes());
    }
    output
}

fn compare_u256(left: &[u64; 4], right: &[u64; 4]) -> Ordering {
    for index in (0..4).rev() {
        match left[index].cmp(&right[index]) {
            Ordering::Equal => {}
            ordering => return ordering,
        }
    }
    Ordering::Equal
}

fn subtract_u256(left: &mut [u64; 4], right: &[u64; 4]) -> bool {
    let mut borrow = false;
    for index in 0..4 {
        let (first, first_borrow) = left[index].overflowing_sub(right[index]);
        let (second, second_borrow) = first.overflowing_sub(u64::from(borrow));
        left[index] = second;
        borrow = first_borrow || second_borrow;
    }
    borrow
}

fn t256_subtract_modulus(value: [u8; 32]) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    let mut output = [0_u8; 32];
    let mut borrow = false;
    for index in (0..32).rev() {
        let (first, first_borrow) =
            super::VEGA_T256_SCALAR_MODULUS_BE_V1[index].overflowing_sub(value[index]);
        let (second, second_borrow) = first.overflowing_sub(u8::from(borrow));
        output[index] = second;
        borrow = first_borrow || second_borrow;
    }
    if borrow {
        return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
    }
    Ok(output)
}

fn decode_centered_plaintext(
    profile: &BgvProfile,
    polynomial: &RnsPolynomial,
    final_residual_bits: usize,
) -> Result<(DecryptedPlaintextV1, u16), ZkAmsMkheErrorV1> {
    polynomial.validate(profile)?;
    if final_residual_bits == 0 || final_residual_bits >= modulus_product(profile.moduli)?.bit_len()
    {
        return Err(ZkAmsMkheErrorV1::InvalidProfile);
    }
    let ciphertext_modulus = modulus_product(profile.moduli)?;
    let mut maximum = 0_usize;
    match profile.plaintext_modulus {
        #[cfg(test)]
        PlaintextModulus::Tiny(plaintext_modulus) => {
            let mut plaintext = Vec::with_capacity(profile.ring_degree);
            for coefficient in 0..profile.ring_degree {
                let residues = (0..profile.moduli.len())
                    .map(|limb| polynomial.limb(profile, limb)[coefficient])
                    .collect::<Vec<_>>();
                let centered = centered_crt(&residues, profile.moduli, ciphertext_modulus)?;
                let magnitude_mod = centered.magnitude.mod_u64(plaintext_modulus);
                let canonical = if centered.negative && magnitude_mod != 0 {
                    plaintext_modulus - magnitude_mod
                } else {
                    magnitude_mod
                };
                let centered_plaintext = if canonical <= (plaintext_modulus - 1) / 2 {
                    SignedCrtV1::normalized(false, wide_from_u64(canonical))
                } else {
                    SignedCrtV1::normalized(true, wide_from_u64(plaintext_modulus - canonical))
                };
                let residual = centered.subtract(centered_plaintext)?;
                if residual.magnitude.mod_u64(plaintext_modulus) != 0
                    || residual.magnitude.bit_len() > final_residual_bits
                {
                    return Err(ZkAmsMkheErrorV1::DecryptionBoundExceeded);
                }
                maximum = maximum.max(residual.magnitude.bit_len());
                plaintext.push(canonical);
            }
            Ok((
                DecryptedPlaintextV1::Tiny(plaintext),
                u16::try_from(maximum).map_err(|_| ZkAmsMkheErrorV1::DecryptionBoundExceeded)?,
            ))
        }
        PlaintextModulus::T256 => {
            let modulus = wide_from_be(&super::VEGA_T256_SCALAR_MODULUS_BE_V1)?;
            let mut plaintext = Vec::with_capacity(profile.ring_degree);
            for coefficient in 0..profile.ring_degree {
                let residues = (0..profile.moduli.len())
                    .map(|limb| polynomial.limb(profile, limb)[coefficient])
                    .collect::<Vec<_>>();
                let centered = centered_crt(&residues, profile.moduli, ciphertext_modulus)?;
                let mut canonical = reduce_wide_mod_t256(centered.magnitude);
                if centered.negative && canonical != [0; 32] {
                    canonical = t256_subtract_modulus(canonical)?;
                }
                let centered_plaintext = if canonical <= super::T256_CENTERED_MAX_BE_V1 {
                    SignedCrtV1::normalized(false, wide_from_be(&canonical)?)
                } else {
                    SignedCrtV1::normalized(
                        true,
                        modulus
                            .checked_sub(wide_from_be(&canonical)?)
                            .ok_or(ZkAmsMkheErrorV1::InvalidPolynomial)?,
                    )
                };
                let residual = centered.subtract(centered_plaintext)?;
                if reduce_wide_mod_t256(residual.magnitude) != [0; 32]
                    || residual.magnitude.bit_len() > final_residual_bits
                {
                    return Err(ZkAmsMkheErrorV1::DecryptionBoundExceeded);
                }
                maximum = maximum.max(residual.magnitude.bit_len());
                plaintext.push(canonical);
            }
            Ok((
                DecryptedPlaintextV1::T256(plaintext),
                u16::try_from(maximum).map_err(|_| ZkAmsMkheErrorV1::DecryptionBoundExceeded)?,
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::MaskedRelaxedRandomErrorV1;
    use super::super::collective::{
        aggregate_zk_ams_mkhe_collective_public_key_v1,
        generate_zk_ams_mkhe_collective_party_state_v1,
    };
    use super::super::sample_uniform_rns;
    use super::*;

    const TEST_MODULI: [u64; 2] = [2_013_265_921, 1_811_939_329];
    const TEST_ROOTS: [u64; 2] = [1_400_279_418, 677_356_115];
    const TEST_SMUDGE_BITS: usize = 8;
    const TEST_FINAL_RESIDUAL_BITS: usize = 24;

    fn test_profile() -> BgvProfile {
        BgvProfile {
            profile_id: [0xd8; 32],
            ring_degree: 8,
            moduli: &TEST_MODULI,
            negacyclic_roots: &TEST_ROOTS,
            plaintext_modulus: PlaintextModulus::Tiny(17),
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
            let mut cursor = 0;
            while cursor < destination.len() {
                let mut frame = Vec::with_capacity(40);
                frame.extend_from_slice(&self.state);
                frame.extend_from_slice(&self.counter.to_be_bytes());
                let block = shake256(&frame, 64);
                let take = (destination.len() - cursor).min(block.len());
                destination[cursor..cursor + take].copy_from_slice(&block[..take]);
                cursor += take;
                self.counter = self.counter.wrapping_add(1);
            }
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

    struct ConstantRandom(u8);

    impl MaskedRelaxedRandomSourceV1 for ConstantRandom {
        fn fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), MaskedRelaxedRandomErrorV1> {
            destination.fill(self.0);
            Ok(())
        }
    }

    struct FastDeterministicRandom(u64);

    impl FastDeterministicRandom {
        fn new(label: &[u8]) -> Self {
            let digest = keccak256(label);
            Self(u64::from_be_bytes(digest[..8].try_into().unwrap()))
        }

        fn next_u64(&mut self) -> u64 {
            self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
            let mut value = self.0;
            value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
            value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
            value ^ (value >> 31)
        }
    }

    impl MaskedRelaxedRandomSourceV1 for FastDeterministicRandom {
        fn fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), MaskedRelaxedRandomErrorV1> {
            for chunk in destination.chunks_mut(8) {
                let block = self.next_u64().to_be_bytes();
                chunk.copy_from_slice(&block[..chunk.len()]);
            }
            Ok(())
        }
    }

    struct Fixture {
        profile: BgvProfile,
        parties: super::super::PartySet,
        authentication: Vec<AuthenticationSecret>,
        relations: Vec<DecryptionPublicRelationV1>,
        secrets: Vec<SecretPolynomial>,
        errors: Vec<SecretPolynomial>,
        ciphertext: ZkAmsMkheCollectiveCiphertextV1,
        message: Vec<u64>,
    }

    fn fixture(label: &[u8]) -> Fixture {
        let profile = test_profile();
        profile.validate().unwrap();
        let mut random = KatRandom::new(label);
        let mut authentication = (0..ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1)
            .map(|_| AuthenticationSecret::generate(&mut random).unwrap())
            .collect::<Vec<_>>();
        authentication.sort_by_key(|secret| secret.party_id().unwrap());
        let parties = super::super::PartySet::new(
            authentication
                .iter()
                .map(|secret| secret.party_id().unwrap())
                .collect(),
        )
        .unwrap();
        let common_a = sample_uniform_rns(&profile, &mut random).unwrap();
        let mut secrets = Vec::with_capacity(ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1);
        let mut errors = Vec::with_capacity(ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1);
        let mut party_bs = Vec::with_capacity(ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1);
        let mut collective_b = RnsPolynomial::zero(&profile);
        for _ in 0..ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1 {
            let secret = SecretPolynomial::sample_ternary(&profile, &mut random).unwrap();
            let error = SecretPolynomial::sample_error(&profile, &mut random).unwrap();
            let party_b = common_a
                .mul(&secret.as_rns(&profile).unwrap(), &profile)
                .unwrap()
                .negate(&profile)
                .unwrap()
                .add(
                    &error
                        .as_rns(&profile)
                        .unwrap()
                        .scale_plaintext_modulus(&profile)
                        .unwrap(),
                    &profile,
                )
                .unwrap();
            collective_b = collective_b.add(&party_b, &profile).unwrap();
            secrets.push(secret);
            errors.push(error);
            party_bs.push(party_b);
        }
        let message = vec![0, 1, 2, 3, 8, 9, 15, 16];
        let message_rns = RnsPolynomial::from_test_plaintext(&profile, &message).unwrap();
        let ephemeral = SecretPolynomial::sample_ternary(&profile, &mut random).unwrap();
        let error_zero = SecretPolynomial::sample_error(&profile, &mut random).unwrap();
        let error_one = SecretPolynomial::sample_error(&profile, &mut random).unwrap();
        let ephemeral_rns = ephemeral.as_rns(&profile).unwrap();
        let constant = collective_b
            .mul(&ephemeral_rns, &profile)
            .unwrap()
            .add(
                &error_zero
                    .as_rns(&profile)
                    .unwrap()
                    .scale_plaintext_modulus(&profile)
                    .unwrap(),
                &profile,
            )
            .unwrap()
            .add(&message_rns, &profile)
            .unwrap();
        let linear = common_a
            .mul(&ephemeral_rns, &profile)
            .unwrap()
            .add(
                &error_one
                    .as_rns(&profile)
                    .unwrap()
                    .scale_plaintext_modulus(&profile)
                    .unwrap(),
                &profile,
            )
            .unwrap();
        let transcript_digest = keccak256(&[label, b".transcript"].concat());
        let epoch = 41;
        let ciphertext = ZkAmsMkheCollectiveCiphertextV1::new(
            &profile,
            &parties,
            epoch,
            transcript_digest,
            73,
            1,
            constant,
            linear,
        )
        .unwrap();
        let roster_digest = ciphertext.roster_digest();
        // Make the digest available to every exact per-party binding.
        let ciphertext_digest = ciphertext.digest();
        let key_context_digest = keccak256(&[label, b".key-context"].concat());
        let statement_binding_digest = keccak256(&[label, b".statement-binding"].concat());
        let mut relations = Vec::with_capacity(ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1);
        for (index, party_b) in party_bs.into_iter().enumerate() {
            let binding = DecryptionBindingV1 {
                profile_digest: profile.digest().unwrap(),
                roster_digest,
                epoch: ciphertext.epoch(),
                transcript_digest,
                ciphertext_digest,
                key_context_digest,
                statement_binding_digest,
                ciphertext_record_index: 0,
                sample_index: ciphertext.sample_index(),
                party_index: u8::try_from(index).unwrap(),
                party: parties.parties[index],
                level: ciphertext.level(),
            };
            relations.push(DecryptionPublicRelationV1 {
                binding: binding.clone(),
                common_a: Arc::new(common_a.clone()),
                party_b,
            });
        }
        ciphertext.validate(&profile, &parties).unwrap();
        Fixture {
            profile,
            parties,
            authentication,
            relations,
            secrets,
            errors,
            ciphertext,
            message,
        }
    }

    fn make_shares(fixture: &Fixture, label: &[u8]) -> Vec<AuthenticatedDecryptionShareV1> {
        let mut random = KatRandom::new(label);
        (0..ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1)
            .map(|index| {
                let witness = DecryptionPartyWitnessV1 {
                    binding: fixture.relations[index].binding.clone(),
                    secret: &fixture.secrets[index],
                    public_key_error: &fixture.errors[index],
                };
                create_authenticated_decryption_share(
                    &fixture.profile,
                    &fixture.parties,
                    &fixture.relations[index],
                    &witness,
                    &fixture.ciphertext,
                    &fixture.authentication[index],
                    TEST_SMUDGE_BITS,
                    &mut random,
                )
                .unwrap()
            })
            .collect()
    }

    struct PublicReleaseProvingFixture {
        party_secrets: Vec<ZkAmsMkheActivePartySecretV1>,
        party_states: Vec<ZkAmsMkheCollectivePartyStateV1>,
        public_key_shares: Vec<ZkAmsMkheCollectivePublicKeyShareV1>,
        collective_public_key: ZkAmsMkheCollectivePublicKeyV1,
        roster: ZkAmsMkheGovernedRosterWireV1,
        ciphertext: ZkAmsMkheCollectiveCiphertextWireV1,
    }

    fn public_release_proving_fixture() -> &'static PublicReleaseProvingFixture {
        static FIXTURE: std::sync::OnceLock<PublicReleaseProvingFixture> =
            std::sync::OnceLock::new();
        FIXTURE.get_or_init(|| {
            let mut random = FastDeterministicRandom::new(b"decryption-public-reachability-setup");
            let mut party_secrets = (0..ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1)
                .map(|_| ZkAmsMkheActivePartySecretV1::generate(&mut random).unwrap())
                .collect::<Vec<_>>();
            party_secrets.sort_by_key(|secret| secret.party().unwrap());
            let ordered_secrets: [&ZkAmsMkheActivePartySecretV1;
                ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1] =
                std::array::from_fn(|index| &party_secrets[index]);
            let governed_roster = super::super::active::ZkAmsMkheGovernedActiveRosterV1::new(
                0xdec0_de01,
                ordered_secrets,
                &mut random,
            )
            .unwrap();
            let roster = governed_roster.to_wire_roster().unwrap();
            let transcript_digest = keccak256(b"decryption-public-reachability.transcript");
            // Each contribution has a disjoint witness and deterministic test
            // stream. Build all eight concurrently so the release-size fixture
            // does not serialize eight otherwise independent native proofs.
            let generated = std::thread::scope(|scope| {
                let handles = (0..ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1)
                    .map(|party_index| {
                        let governed_roster = &governed_roster;
                        let party_secret = &party_secrets[party_index];
                        scope.spawn(move || {
                            let seed = [
                                b"decryption-public-reachability-party".as_slice(),
                                &[u8::try_from(party_index).unwrap()],
                            ]
                            .concat();
                            let mut party_random = FastDeterministicRandom::new(&seed);
                            generate_zk_ams_mkhe_collective_party_state_v1(
                                governed_roster,
                                transcript_digest,
                                party_index,
                                party_secret,
                                &mut party_random,
                            )
                        })
                    })
                    .collect::<Vec<_>>();
                handles
                    .into_iter()
                    .map(|handle| handle.join().unwrap().unwrap())
                    .collect::<Vec<_>>()
            });
            let (party_states, public_key_shares): (Vec<_>, Vec<_>) = generated.into_iter().unzip();
            assert!(
                party_states.len() == ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1
                    && party_states.iter().all(|state| {
                        state.persistent_secret_commitment_blindings().len()
                            == ZK_AMS_MKHE_PERSISTENT_MEMBERSHIP_CHUNKS_V1
                            && state
                                .persistent_secret_commitment_blindings()
                                .iter()
                                .all(|blinding| !blinding.is_zero())
                    }),
                "the production constructor must retain eight nonzero persistent commitment blindings per party",
            );
            let share_references: [&ZkAmsMkheCollectivePublicKeyShareV1;
                ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1] =
                std::array::from_fn(|index| &public_key_shares[index]);
            let collective_public_key = aggregate_zk_ams_mkhe_collective_public_key_v1(
                &governed_roster,
                transcript_digest,
                share_references,
            )
            .unwrap();
            let binding = ZkAmsMkheWireBindingV1::new(
                &roster,
                keccak256(b"decryption-public-reachability.ciphertext-lineage"),
                0,
                1,
            )
            .unwrap();
            let common_a = public_key_shares[0].public_a().clone();
            let ciphertext = ZkAmsMkheCollectiveCiphertextWireV1::new(
                binding,
                0x0051_a2e0,
                common_a.clone(),
                common_a,
            )
            .unwrap();
            PublicReleaseProvingFixture {
                party_secrets,
                party_states,
                public_key_shares,
                collective_public_key,
                roster,
                ciphertext,
            }
        })
    }

    fn public_release_statement<'a>(
        fixture: &'a PublicReleaseProvingFixture,
        public_key_shares: &'a [&'a ZkAmsMkheCollectivePublicKeyShareV1;
                ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
    ) -> ZkAmsMkheDecryptionStatementV1<'a> {
        ZkAmsMkheDecryptionStatementV1::new(
            &fixture.roster,
            &fixture.ciphertext,
            &fixture.collective_public_key,
            public_key_shares,
        )
        .unwrap()
    }

    #[test]
    fn exact_wide_sign_magnitude_boundaries_are_canonical_and_no_wrap() {
        let maximum = WideMagnitudeV1::max_for_bits(1_855).unwrap();
        assert_eq!(maximum.bit_len(), 1_855);
        for value in [
            SignedWideV1::zero(),
            SignedWideV1::new(false, maximum.clone()).unwrap(),
            SignedWideV1::new(true, maximum.clone()).unwrap(),
        ] {
            let encoded = value.encode_fixed(232).unwrap();
            assert_eq!(encoded.len(), 232);
            assert_eq!(SignedWideV1::decode_fixed(&encoded).unwrap(), value);
            for modulus in TEST_MODULI {
                assert!(value.mod_u64(modulus) < modulus);
            }
        }
        let mut negative_zero = vec![0_u8; 232];
        negative_zero[0] = 0x80;
        assert_eq!(
            SignedWideV1::decode_fixed(&negative_zero),
            Err(ZkAmsMkheErrorV1::InvalidWireEncoding)
        );
        let overflow =
            SignedWideV1::new(false, WideMagnitudeV1::max_for_bits(1_856).unwrap()).unwrap();
        assert_eq!(
            overflow.encode_fixed(232),
            Err(ZkAmsMkheErrorV1::InvalidWireEncoding)
        );
    }

    #[test]
    fn t256_wide_reduction_and_centering_boundaries_are_exact() {
        let modulus = wide_from_be(&super::super::VEGA_T256_SCALAR_MODULUS_BE_V1).unwrap();
        let one = wide_from_u64(1);
        let modulus_minus_one = modulus.checked_sub(one).unwrap();
        let modulus_plus_one = wide_checked_add(modulus, one).unwrap();
        let twice_modulus = wide_checked_add(modulus, modulus).unwrap();
        assert_eq!(reduce_wide_mod_t256(WideUint::zero()), [0; 32]);
        let mut one_be = [0_u8; 32];
        one_be[31] = 1;
        assert_eq!(reduce_wide_mod_t256(one), one_be);
        assert_eq!(
            reduce_wide_mod_t256(modulus_minus_one),
            t256_subtract_modulus(one_be).unwrap()
        );
        assert_eq!(reduce_wide_mod_t256(modulus), [0; 32]);
        assert_eq!(reduce_wide_mod_t256(modulus_plus_one), one_be);
        assert_eq!(reduce_wide_mod_t256(twice_modulus), [0; 32]);

        let negative_one = SignedCrtV1::normalized(true, one);
        let canonical = t256_subtract_modulus(one_be).unwrap();
        let reduced = reduce_wide_mod_t256(negative_one.magnitude);
        assert_eq!(t256_subtract_modulus(reduced).unwrap(), canonical);
        assert!(canonical > super::super::T256_CENTERED_MAX_BE_V1);
    }

    #[test]
    fn unavailable_zero_and_constant_random_sources_fail_closed() {
        assert_eq!(
            validate_wide_relation_random_health(&mut FailingRandom),
            Err(ZkAmsMkheErrorV1::RandomUnavailable)
        );
        assert_eq!(
            validate_wide_relation_random_health(&mut ConstantRandom(0)),
            Err(ZkAmsMkheErrorV1::RandomUnavailable)
        );
        assert_eq!(
            validate_wide_relation_random_health(&mut ConstantRandom(0xa5)),
            Err(ZkAmsMkheErrorV1::RandomUnavailable)
        );
        let bound = WideMagnitudeV1::max_for_bits(TEST_SMUDGE_BITS).unwrap();
        assert_eq!(
            sample_signed_wide(&bound, &mut FailingRandom),
            Err(ZkAmsMkheErrorV1::RandomUnavailable)
        );
    }

    #[test]
    fn public_generated_opaque_state_reaches_native_prove_and_verify_end_to_end() {
        let fixture = public_release_proving_fixture();
        let public_key_shares: [&ZkAmsMkheCollectivePublicKeyShareV1;
            ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1] =
            std::array::from_fn(|index| &fixture.public_key_shares[index]);
        let statement = public_release_statement(fixture, &public_key_shares);
        let mut random = FastDeterministicRandom::new(b"decryption-public-reachability-proof");
        let share = prove_zk_ams_mkhe_decryption_share_v1(
            statement,
            0,
            &fixture.party_states[0],
            &fixture.party_secrets[0],
            &mut random,
        )
        .unwrap();
        assert_eq!(share.party_index(), 0);
        assert_eq!(share.party(), fixture.roster.parties()[0]);
        verify_zk_ams_mkhe_decryption_share_v1(statement, &share).unwrap();

        let transport = split_zk_ams_mkhe_decryption_share_v1(statement, &share).unwrap();
        let manifest = transport.manifest_bytes().unwrap();
        println!(
            "ZK_AMS_MKHE_DECRYPTION_SPLIT_RELEASE_KAT_DIGEST_V1={}",
            hex::encode(transport.manifest().manifest_digest())
        );
        assert_eq!(
            transport.manifest().manifest_digest(),
            ZK_AMS_MKHE_DECRYPTION_SPLIT_RELEASE_KAT_DIGEST_V1
        );
        assert_eq!(
            manifest.len(),
            ZK_AMS_MKHE_DECRYPTION_SPLIT_MANIFEST_BYTES_V1
        );
        assert_eq!(transport.polynomial_object().len(), 39_845_892);
        assert_eq!(transport.proof_envelope().len(), 33_030_199);
        assert!(transport.polynomial_object().len() <= 64 * 1024 * 1024);
        assert!(transport.proof_envelope().len() <= 32 * 1024 * 1024);
        let components = transport.ordered_components();
        let reconstructed =
            reconstruct_zk_ams_mkhe_decryption_share_v1(statement, &manifest, &components).unwrap();
        assert_eq!(reconstructed, share);
        verify_zk_ams_mkhe_decryption_share_v1(statement, &reconstructed).unwrap();

        // Every manifest statement/binding/authentication axis is covered by
        // the signed manifest digest. These offsets are fixed by the 498-byte
        // `ZDSM` layout and deliberately exercise each field independently.
        for offset in [
            0_usize, // tag
            4_usize, // version
            5,       // profile
            37,      // roster
            69,      // epoch
            77,      // transcript
            109,     // ciphertext digest
            141,     // collective-key context
            173,     // exact public statement binding
            205,     // ciphertext record index
            209,     // sample index
            217,     // party index
            218,     // party id
            250,     // level
            251,     // component count
            254,     // polynomial exact byte length
            262,     // polynomial BLAKE3 digest
            296,     // proof exact byte length
            304,     // proof BLAKE3 digest
            336,     // stored manifest digest
            368,     // authenticated party
            400,     // authentication public key
            433,     // authentication signature
        ] {
            let mut changed = manifest.clone();
            changed[offset] ^= 1;
            assert!(
                reconstruct_zk_ams_mkhe_decryption_share_v1(statement, &changed, &components,)
                    .is_err(),
                "manifest mutation offset {offset} must reject"
            );
        }

        // Kind and ordinal swaps are rejected during the small-header
        // preflight, before authentication hashing or component processing.
        for offset in [252_usize, 253, 294, 295] {
            let mut changed = manifest.clone();
            changed[offset] ^= 1;
            assert!(
                ZkAmsMkheDecryptionTransportManifestV1::decode_exact(statement, &changed).is_err()
            );
        }
        let mut reordered_pointers = manifest.clone();
        let first_pointer = reordered_pointers[252..294].to_vec();
        let second_pointer = reordered_pointers[294..336].to_vec();
        reordered_pointers[252..294].copy_from_slice(&second_pointer);
        reordered_pointers[294..336].copy_from_slice(&first_pointer);
        assert!(
            ZkAmsMkheDecryptionTransportManifestV1::decode_exact(statement, &reordered_pointers,)
                .is_err()
        );
        let mut duplicate_pointer = manifest.clone();
        duplicate_pointer[294..336].copy_from_slice(&first_pointer);
        assert!(
            ZkAmsMkheDecryptionTransportManifestV1::decode_exact(statement, &duplicate_pointer,)
                .is_err()
        );

        // Manifest authentication is resolved before even the component-list
        // cardinality is inspected, hence before either large object can be
        // hashed or decoded.
        let mut forged_authentication = manifest.clone();
        *forged_authentication.last_mut().unwrap() ^= 1;
        assert_eq!(
            reconstruct_zk_ams_mkhe_decryption_share_v1(statement, &forged_authentication, &[],),
            Err(ZkAmsMkheErrorV1::InvalidAuthentication)
        );

        assert!(reconstruct_zk_ams_mkhe_decryption_share_v1(statement, &manifest, &[]).is_err());
        assert!(
            reconstruct_zk_ams_mkhe_decryption_share_v1(
                statement,
                &manifest,
                &[transport.polynomial_object()],
            )
            .is_err()
        );
        assert!(
            reconstruct_zk_ams_mkhe_decryption_share_v1(
                statement,
                &manifest,
                &[transport.polynomial_object(), transport.polynomial_object(),],
            )
            .is_err()
        );
        assert!(
            reconstruct_zk_ams_mkhe_decryption_share_v1(
                statement,
                &manifest,
                &[transport.proof_envelope(), transport.polynomial_object(),],
            )
            .is_err()
        );
        assert!(
            reconstruct_zk_ams_mkhe_decryption_share_v1(
                statement,
                &manifest,
                &[
                    transport.polynomial_object(),
                    transport.proof_envelope(),
                    transport.proof_envelope(),
                ],
            )
            .is_err()
        );

        for (object_index, object) in components.into_iter().enumerate() {
            let truncated_components: [&[u8]; 2] = if object_index == 0 {
                [&object[..object.len() - 1], transport.proof_envelope()]
            } else {
                [transport.polynomial_object(), &object[..object.len() - 1]]
            };
            assert!(
                reconstruct_zk_ams_mkhe_decryption_share_v1(
                    statement,
                    &manifest,
                    &truncated_components,
                )
                .is_err()
            );
            let mut extended = object.to_vec();
            extended.push(0);
            let extended_components: [&[u8]; 2] = if object_index == 0 {
                [&extended, transport.proof_envelope()]
            } else {
                [transport.polynomial_object(), &extended]
            };
            assert!(
                reconstruct_zk_ams_mkhe_decryption_share_v1(
                    statement,
                    &manifest,
                    &extended_components,
                )
                .is_err()
            );
            let mut mutated = object.to_vec();
            *mutated.last_mut().unwrap() ^= 1;
            let mutated_components: [&[u8]; 2] = if object_index == 0 {
                [&mutated, transport.proof_envelope()]
            } else {
                [transport.polynomial_object(), &mutated]
            };
            assert!(
                reconstruct_zk_ams_mkhe_decryption_share_v1(
                    statement,
                    &manifest,
                    &mutated_components,
                )
                .is_err()
            );
        }
        assert!(
            reconstruct_zk_ams_mkhe_decryption_share_v1(
                statement,
                &manifest[..manifest.len() - 1],
                &transport.ordered_components(),
            )
            .is_err()
        );
        let mut extended_manifest = manifest.clone();
        extended_manifest.push(0);
        assert!(
            reconstruct_zk_ams_mkhe_decryption_share_v1(
                statement,
                &extended_manifest,
                &transport.ordered_components(),
            )
            .is_err()
        );

        let wrong_binding = ZkAmsMkheWireBindingV1::new(
            &fixture.roster,
            keccak256(b"decryption-public-reachability.wrong-operation-transcript"),
            fixture.ciphertext.binding().record_index() + 1,
            fixture.ciphertext.binding().level(),
        )
        .unwrap();
        let wrong_ciphertext = ZkAmsMkheCollectiveCiphertextWireV1::new(
            wrong_binding,
            fixture.ciphertext.sample_index(),
            fixture.ciphertext.constant().clone(),
            fixture.ciphertext.linear().clone(),
        )
        .unwrap();
        let wrong_statement = ZkAmsMkheDecryptionStatementV1::new(
            &fixture.roster,
            &wrong_ciphertext,
            &fixture.collective_public_key,
            &public_key_shares,
        )
        .unwrap();
        assert!(
            reconstruct_zk_ams_mkhe_decryption_share_v1(
                wrong_statement,
                &manifest,
                &transport.ordered_components(),
            )
            .is_err()
        );

        let wrong_sample_ciphertext = ZkAmsMkheCollectiveCiphertextWireV1::new(
            fixture.ciphertext.binding(),
            fixture.ciphertext.sample_index() + 1,
            fixture.ciphertext.constant().clone(),
            fixture.ciphertext.linear().clone(),
        )
        .unwrap();
        let wrong_sample_statement = ZkAmsMkheDecryptionStatementV1::new(
            &fixture.roster,
            &wrong_sample_ciphertext,
            &fixture.collective_public_key,
            &public_key_shares,
        )
        .unwrap();
        assert!(
            reconstruct_zk_ams_mkhe_decryption_share_v1(
                wrong_sample_statement,
                &manifest,
                &transport.ordered_components(),
            )
            .is_err()
        );
    }

    #[test]
    fn public_prover_rejects_wrong_opaque_active_secret_and_roster_slot_before_rng() {
        let fixture = public_release_proving_fixture();
        let public_key_shares: [&ZkAmsMkheCollectivePublicKeyShareV1;
            ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1] =
            std::array::from_fn(|index| &fixture.public_key_shares[index]);
        let statement = public_release_statement(fixture, &public_key_shares);
        assert_eq!(
            prove_zk_ams_mkhe_decryption_share_v1(
                statement,
                0,
                &fixture.party_states[0],
                &fixture.party_secrets[1],
                &mut FailingRandom,
            ),
            Err(ZkAmsMkheErrorV1::InvalidAuthentication)
        );
        assert_eq!(
            prove_zk_ams_mkhe_decryption_share_v1(
                statement,
                ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1,
                &fixture.party_states[0],
                &fixture.party_secrets[0],
                &mut FailingRandom,
            ),
            Err(ZkAmsMkheErrorV1::InvalidPartySet)
        );
        assert_eq!(
            prove_zk_ams_mkhe_decryption_share_v1(
                statement,
                1,
                &fixture.party_states[0],
                &fixture.party_secrets[1],
                &mut FailingRandom,
            ),
            Err(ZkAmsMkheErrorV1::InvalidKeyMaterial)
        );
        assert_eq!(
            prove_zk_ams_mkhe_decryption_share_v1(
                statement,
                0,
                &fixture.party_states[0],
                &fixture.party_secrets[0],
                &mut ConstantRandom(0),
            ),
            Err(ZkAmsMkheErrorV1::RandomUnavailable)
        );
    }

    #[test]
    fn opaque_state_and_verified_key_context_reject_every_splice_before_rng() {
        let fixture = public_release_proving_fixture();
        let public_key_shares: [&ZkAmsMkheCollectivePublicKeyShareV1;
            ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1] =
            std::array::from_fn(|index| &fixture.public_key_shares[index]);
        let statement = public_release_statement(fixture, &public_key_shares);
        let expected = expected_opaque_party_state_context(statement, 0).unwrap();
        validate_opaque_party_state_context(&fixture.party_states[0], expected).unwrap();

        for axis in 0..9 {
            let mut changed = expected;
            match axis {
                0 => changed.profile_digest[0] ^= 1,
                1 => changed.security_certificate_digest[0] ^= 1,
                2 => changed.roster_digest[0] ^= 1,
                3 => changed.key_material_digest[0] ^= 1,
                4 => changed.epoch += 1,
                5 => changed.transcript_digest[0] ^= 1,
                6 => changed.party_index = 1,
                7 => changed.party = fixture.roster.parties()[1],
                8 => changed.public_share_digest[0] ^= 1,
                _ => unreachable!(),
            }
            assert_eq!(
                validate_opaque_party_state_context(&fixture.party_states[0], changed),
                Err(ZkAmsMkheErrorV1::InvalidKeyMaterial),
                "opaque state context axis {axis} must reject"
            );
        }

        let mut swapped_public_key_shares = public_key_shares;
        swapped_public_key_shares.swap(0, 1);
        assert!(matches!(
            ZkAmsMkheDecryptionStatementV1::new(
                &fixture.roster,
                &fixture.ciphertext,
                &fixture.collective_public_key,
                &swapped_public_key_shares,
            ),
            Err(ZkAmsMkheErrorV1::InvalidKeyMaterial)
        ));

        let (profile, parties, ciphertext, common_a, party_b) =
            statement.internal_for_party(0).unwrap();
        let binding = DecryptionBindingV1 {
            profile_digest: statement.roster.profile_digest(),
            roster_digest: statement.roster.roster_digest(),
            epoch: statement.roster.epoch(),
            transcript_digest: statement.ciphertext.binding().transcript_digest(),
            ciphertext_digest: ciphertext.digest(),
            key_context_digest: statement.key_context_digest,
            statement_binding_digest: statement.binding_digest(),
            ciphertext_record_index: statement.ciphertext.binding().record_index(),
            sample_index: statement.ciphertext.sample_index(),
            party_index: 0,
            party: statement.roster.parties()[0],
            level: statement.ciphertext.binding().level(),
        };
        let witness = DecryptionPartyWitnessV1 {
            binding: binding.clone(),
            secret: fixture.party_states[0].secret(),
            public_key_error: fixture.party_states[0].public_error(),
        };
        let relation = DecryptionPublicRelationV1 {
            binding: binding.clone(),
            common_a: Arc::new(common_a.clone()),
            party_b: party_b.clone(),
        };
        validate_party_witness(&profile, &parties, &relation, &witness).unwrap();

        let mut wrong_a = common_a;
        wrong_a.coefficients[0] = (wrong_a.coefficients[0] + 1) % profile.moduli[0];
        let wrong_a_relation = DecryptionPublicRelationV1 {
            binding: binding.clone(),
            common_a: Arc::new(wrong_a),
            party_b: party_b.clone(),
        };
        assert_eq!(
            create_decryption_share(
                &profile,
                &parties,
                &wrong_a_relation,
                &witness,
                &ciphertext,
                usize::from(
                    zk_ams_mkhe_noise_certificate_v1()
                        .unwrap()
                        .decryption_smudge_quotient_bits,
                ),
                &mut FailingRandom,
            ),
            Err(ZkAmsMkheErrorV1::InvalidKeyMaterial)
        );

        let mut wrong_b = party_b;
        wrong_b.coefficients[0] = (wrong_b.coefficients[0] + 1) % profile.moduli[0];
        let wrong_b_relation = DecryptionPublicRelationV1 {
            binding,
            common_a: relation.common_a,
            party_b: wrong_b,
        };
        assert_eq!(
            create_decryption_share(
                &profile,
                &parties,
                &wrong_b_relation,
                &witness,
                &ciphertext,
                usize::from(
                    zk_ams_mkhe_noise_certificate_v1()
                        .unwrap()
                        .decryption_smudge_quotient_bits,
                ),
                &mut FailingRandom,
            ),
            Err(ZkAmsMkheErrorV1::InvalidKeyMaterial)
        );
    }

    #[test]
    fn complete_eight_party_native_decryption_kat_recovers_plaintext() {
        let fixture = fixture(b"decryption-positive-kat");
        let shares = make_shares(&fixture, b"decryption-positive-shares");
        let result = aggregate_and_decrypt_full_roster(
            &fixture.profile,
            &fixture.parties,
            &fixture.relations,
            &fixture.ciphertext,
            &shares,
            TEST_SMUDGE_BITS,
            TEST_FINAL_RESIDUAL_BITS,
        )
        .unwrap();
        assert_eq!(
            result.plaintext,
            DecryptedPlaintextV1::Tiny(fixture.message)
        );
        assert_eq!(
            result.ordered_share_set_digest,
            [
                0x29, 0xa8, 0x11, 0x71, 0x08, 0x12, 0x26, 0x8b, 0x3d, 0x6f, 0x34, 0xc5, 0x39, 0x5f,
                0x2d, 0xf2, 0xc3, 0x28, 0x17, 0xef, 0x42, 0xaa, 0x2f, 0x70, 0xce, 0x1e, 0xdf, 0x1b,
                0x1b, 0xfb, 0x2c, 0x28,
            ]
        );
        assert!(result.maximum_residual_bits <= TEST_FINAL_RESIDUAL_BITS as u16);
        assert_ne!(result.ordered_share_set_digest, [0; 32]);
    }

    #[test]
    fn canonical_share_wire_roundtrip_rehashes_and_reverifies_every_relation() {
        let fixture = fixture(b"decryption-wire-kat");
        let shares = make_shares(&fixture, b"decryption-wire-shares");
        for (index, share) in shares.iter().enumerate() {
            let encoded = share.encode(&fixture.profile, TEST_SMUDGE_BITS).unwrap();
            if index == 0 {
                assert_eq!(
                    keccak256(&encoded),
                    [
                        0x53, 0x7a, 0xd4, 0x71, 0xe1, 0xff, 0xed, 0xc2, 0x8c, 0x0a, 0x91, 0xf7,
                        0xce, 0x31, 0x99, 0xce, 0x26, 0x61, 0x90, 0x07, 0xff, 0xde, 0xef, 0x8b,
                        0x19, 0x8f, 0x05, 0xfa, 0x0a, 0xa1, 0xac, 0xea,
                    ]
                );
            }
            let decoded = AuthenticatedDecryptionShareV1::decode_exact(
                &encoded,
                &fixture.profile,
                &fixture.parties,
                &fixture.relations[index].binding,
                TEST_SMUDGE_BITS,
            )
            .unwrap();
            assert_eq!(decoded, *share);
            assert_eq!(
                decoded.encode(&fixture.profile, TEST_SMUDGE_BITS).unwrap(),
                encoded
            );
            verify_authenticated_share(
                &fixture.profile,
                &fixture.parties,
                &fixture.relations[index],
                &fixture.ciphertext,
                &fixture.relations[index].binding,
                &decoded,
                TEST_SMUDGE_BITS,
            )
            .unwrap();
        }
    }

    #[test]
    fn release_resource_evidence_is_exact_for_split_transport() {
        let evidence = zk_ams_mkhe_decryption_resource_evidence_v1().unwrap();
        evidence.validate().unwrap();
        assert_eq!(evidence.ring_degree, 131_072);
        assert_eq!(evidence.rns_limb_count, 38);
        assert_eq!(evidence.roster_size, 8);
        assert_eq!(evidence.smudge_quotient_bits, 1_855);
        assert_eq!(evidence.challenge_weight, 20);
        assert_eq!(evidence.challenge_space_lower_bound_bits, 260);
        assert_eq!(evidence.statistical_security_bits, 128);
        assert_eq!(evidence.mask_slack_log2, 24);
        assert_eq!(evidence.wide_response_coefficient_bytes, 236);
        assert_eq!(evidence.share_polynomial_bytes, 39_845_892);
        assert_eq!(
            evidence.secret_response_bytes,
            131_072 * DECRYPTION_SIGNED_SMALL_BYTES_V1 as u64
        );
        assert_eq!(
            evidence.public_key_error_response_bytes,
            evidence.secret_response_bytes
        );
        assert_eq!(evidence.smudge_response_bytes, 30_932_992);
        assert_eq!(evidence.proof_header_bytes, 55);
        assert_eq!(evidence.proof_payload_bytes, 33_030_199);
        assert_eq!(
            evidence.governed_proof_payload_ceiling_bytes,
            32 * 1024 * 1024
        );
        assert_eq!(evidence.proof_payload_headroom_bytes, 524_233);
        assert!(evidence.proof_payload_ceiling_met);
        assert_eq!(evidence.split_polynomial_object_bytes, 39_845_892);
        assert_eq!(evidence.split_proof_envelope_bytes, 33_030_199);
        assert_eq!(evidence.split_manifest_bytes, 498);
        assert_eq!(evidence.split_polynomial_headroom_bytes, 27_262_972);
        assert_eq!(evidence.split_proof_headroom_bytes, 524_233);
        assert!(evidence.split_component_ceilings_met);
        assert_eq!(
            evidence.split_release_kat_digest,
            ZK_AMS_MKHE_DECRYPTION_SPLIT_RELEASE_KAT_DIGEST_V1
        );
        assert_ne!(evidence.split_release_kat_digest, [0; 32]);
        assert!(evidence.split_transport_ready);
        assert_eq!(evidence.record_overhead_bytes, 432);
        assert_eq!(evidence.total_share_record_bytes, 72_876_523);
        assert_eq!(evidence.governed_share_ceiling_bytes, 64 * 1024 * 1024);
        assert!(!evidence.share_ceiling_met);
        assert_eq!(evidence.ceiling_shortfall_bytes, 5_767_659);
        assert_eq!(
            evidence.minimum_sound_share_ceiling_bytes,
            evidence.total_share_record_bytes
        );
        assert_eq!(
            evidence.evidence_digest,
            [
                0x1a, 0x67, 0x05, 0xd1, 0xad, 0x09, 0xd0, 0x0c, 0x7f, 0x90, 0x2d, 0xf1, 0x9b, 0xff,
                0xe8, 0xc2, 0x37, 0x35, 0x93, 0x5d, 0x5b, 0x0c, 0x4f, 0x2d, 0xea, 0xd4, 0xb5, 0xf0,
                0x25, 0xd6, 0xc3, 0x67,
            ]
        );
    }

    #[test]
    fn release_split_objects_have_exact_canonical_sizes_and_preflight() {
        let profile = release_profile_v1();
        let evidence = zk_ams_mkhe_decryption_resource_evidence_v1().unwrap();
        let polynomial = RnsPolynomial::zero(&profile);
        let polynomial_object = encode_decryption_polynomial_object(&profile, &polynomial).unwrap();
        assert_eq!(polynomial_object.len(), 39_845_892);
        assert!(polynomial_object.len() <= profile.max_share_bytes);
        assert_eq!(
            decode_decryption_polynomial_object(&polynomial_object).unwrap(),
            polynomial
        );
        assert!(
            decode_decryption_polynomial_object(&polynomial_object[..polynomial_object.len() - 1])
                .is_err()
        );
        let mut extended_polynomial = polynomial_object.clone();
        extended_polynomial.push(0);
        assert!(decode_decryption_polynomial_object(&extended_polynomial).is_err());
        drop(extended_polynomial);
        let mut wrong_polynomial_count = polynomial_object.clone();
        wrong_polynomial_count[..4].copy_from_slice(&u32::MAX.to_be_bytes());
        assert!(decode_decryption_polynomial_object(&wrong_polynomial_count).is_err());
        drop(wrong_polynomial_count);
        let mut noncanonical_polynomial = polynomial_object.clone();
        noncanonical_polynomial[4..12].copy_from_slice(&profile.moduli[0].to_be_bytes());
        assert!(decode_decryption_polynomial_object(&noncanonical_polynomial).is_err());
        drop(noncanonical_polynomial);

        let proof = DecryptionRelationProofV1 {
            wide_response_bytes: evidence.wide_response_coefficient_bytes,
            challenge_seed: [0x5a; 32],
            secret_response: vec![0; profile.ring_degree],
            public_key_error_response: vec![0; profile.ring_degree],
            smudge_response: vec![SignedWideV1::zero(); profile.ring_degree],
        };
        let proof_envelope = proof.encode().unwrap();
        assert_eq!(proof_envelope.len(), 33_030_199);
        assert!(proof_envelope.len() <= ZK_AMS_MKHE_MAX_PROOF_BYTES_V1);
        let decoded = ZkAmsMkheDecryptionProofV1::decode_release_exact(&proof_envelope).unwrap();
        assert_eq!(decoded, proof);
        assert!(matches!(
            ZkAmsMkheDecryptionProofV1::decode_release_exact(
                &proof_envelope[..proof_envelope.len() - 1]
            ),
            Err(ZkAmsMkheErrorV1::InvalidWireEncoding)
        ));
        let mut extended_proof = proof_envelope.clone();
        extended_proof.push(0);
        assert!(ZkAmsMkheDecryptionProofV1::decode_release_exact(&extended_proof).is_err());
        drop(extended_proof);
        for offset in [
            0_usize, // tag
            4,       // version
            5,       // fixed wide-response width
            7,       // ring degree
            43,      // secret-response count
            47,      // public-error-response count
            51,      // smudge-response count
        ] {
            let mut malformed = proof_envelope.clone();
            malformed[offset] ^= 1;
            assert!(
                ZkAmsMkheDecryptionProofV1::decode_release_exact(&malformed).is_err(),
                "proof header mutation offset {offset} must reject"
            );
        }
        let mut negative_zero = proof_envelope.clone();
        let first_wide_response = DECRYPTION_PROOF_HEADER_BYTES_V1
            + profile.ring_degree * 2 * DECRYPTION_SIGNED_SMALL_BYTES_V1;
        negative_zero
            [first_wide_response..first_wide_response + usize::from(proof.wide_response_bytes)]
            .fill(0);
        negative_zero[first_wide_response] = 0x80;
        assert!(ZkAmsMkheDecryptionProofV1::decode_release_exact(&negative_zero).is_err());
        let polynomial_pointer = ZkAmsMkheDecryptionTransportPointerV1::from_payload(
            ZkAmsMkheDecryptionTransportComponentKindV1::SharePolynomial,
            &polynomial_object,
        )
        .unwrap();
        let proof_pointer = ZkAmsMkheDecryptionTransportPointerV1::from_payload(
            ZkAmsMkheDecryptionTransportComponentKindV1::ProofEnvelope,
            &proof_envelope,
        )
        .unwrap();
        assert_eq!(polynomial_pointer.payload_bytes(), 39_845_892);
        assert_eq!(proof_pointer.payload_bytes(), 33_030_199);
        assert_ne!(polynomial_pointer.payload_blake3(), [0; 32]);
        assert_ne!(proof_pointer.payload_blake3(), [0; 32]);
    }

    #[test]
    fn sound_share_ceiling_boundary_is_exact_to_one_byte() {
        let baseline = zk_ams_mkhe_decryption_resource_evidence_v1().unwrap();
        let mut one_short = release_profile_v1();
        one_short.max_share_bytes = usize::try_from(
            baseline
                .minimum_sound_share_ceiling_bytes
                .checked_sub(1)
                .unwrap(),
        )
        .unwrap();
        let short = derive_decryption_resource_evidence(&one_short).unwrap();
        assert!(!short.share_ceiling_met);
        assert_eq!(short.ceiling_shortfall_bytes, 1);

        let mut exact = release_profile_v1();
        exact.max_share_bytes =
            usize::try_from(baseline.minimum_sound_share_ceiling_bytes).unwrap();
        let exact = derive_decryption_resource_evidence(&exact).unwrap();
        assert!(exact.share_ceiling_met);
        assert_eq!(exact.ceiling_shortfall_bytes, 0);
        assert_eq!(
            production_decryption_share_record_bytes(
                &release_profile_v1(),
                usize::from(baseline.smudge_quotient_bits),
            )
            .unwrap()
            .1,
            usize::try_from(baseline.minimum_sound_share_ceiling_bytes).unwrap()
        );
    }

    #[test]
    fn missing_excess_duplicate_and_reordered_sets_identify_first_slot() {
        let fixture = fixture(b"decryption-set-negative");
        let shares = make_shares(&fixture, b"decryption-set-negative-shares");
        let missing = aggregate_and_decrypt_full_roster(
            &fixture.profile,
            &fixture.parties,
            &fixture.relations,
            &fixture.ciphertext,
            &shares[..7],
            TEST_SMUDGE_BITS,
            TEST_FINAL_RESIDUAL_BITS,
        )
        .unwrap_err();
        assert_eq!(missing.reason, DecryptionAbortReasonV1::MissingShare);
        assert_eq!(missing.party_index, 7);

        let mut excess = shares.clone();
        excess.push(shares[0].clone());
        assert_eq!(
            aggregate_and_decrypt_full_roster(
                &fixture.profile,
                &fixture.parties,
                &fixture.relations,
                &fixture.ciphertext,
                &excess,
                TEST_SMUDGE_BITS,
                TEST_FINAL_RESIDUAL_BITS,
            )
            .unwrap_err()
            .reason,
            DecryptionAbortReasonV1::ExcessShare
        );

        for mutation in [
            {
                let mut values = shares.clone();
                values[1] = values[0].clone();
                values
            },
            {
                let mut values = shares.clone();
                values.swap(2, 3);
                values
            },
        ] {
            let abort = aggregate_and_decrypt_full_roster(
                &fixture.profile,
                &fixture.parties,
                &fixture.relations,
                &fixture.ciphertext,
                &mutation,
                TEST_SMUDGE_BITS,
                TEST_FINAL_RESIDUAL_BITS,
            )
            .unwrap_err();
            assert_eq!(
                abort.reason,
                DecryptionAbortReasonV1::ReorderedOrDuplicateShare
            );
            assert_ne!(abort.evidence_digest, [0; 32]);
        }
    }

    #[test]
    fn every_binding_axis_and_replay_axis_fails_closed() {
        let fixture = fixture(b"decryption-binding-negative");
        let shares = make_shares(&fixture, b"decryption-binding-negative-shares");
        for axis in 0..12 {
            let mut mutation = shares[3].clone();
            match axis {
                0 => mutation.binding.profile_digest[0] ^= 1,
                1 => mutation.binding.roster_digest[0] ^= 1,
                2 => mutation.binding.epoch += 1,
                3 => mutation.binding.transcript_digest[0] ^= 1,
                4 => mutation.binding.ciphertext_digest[0] ^= 1,
                5 => mutation.binding.key_context_digest[0] ^= 1,
                6 => mutation.binding.statement_binding_digest[0] ^= 1,
                7 => mutation.binding.ciphertext_record_index += 1,
                8 => mutation.binding.sample_index += 1,
                9 => mutation.binding.party_index = 4,
                10 => mutation.binding.party = fixture.parties.parties[4],
                11 => mutation.binding.level = 0,
                _ => unreachable!(),
            }
            assert!(
                verify_authenticated_share(
                    &fixture.profile,
                    &fixture.parties,
                    &fixture.relations[3],
                    &fixture.ciphertext,
                    &fixture.relations[3].binding,
                    &mutation,
                    TEST_SMUDGE_BITS,
                )
                .is_err(),
                "binding axis {axis} must reject"
            );
        }
        let other_fixture = self::fixture(b"decryption-binding-other-session");
        assert!(
            verify_authenticated_share(
                &other_fixture.profile,
                &other_fixture.parties,
                &other_fixture.relations[3],
                &other_fixture.ciphertext,
                &other_fixture.relations[3].binding,
                &shares[3],
                TEST_SMUDGE_BITS,
            )
            .is_err()
        );
    }

    #[test]
    fn polynomial_public_key_proof_and_authentication_mutations_are_rejected() {
        let fixture = fixture(b"decryption-proof-negative");
        let shares = make_shares(&fixture, b"decryption-proof-negative-shares");

        let mut share_poly = shares[2].clone();
        share_poly.share.coefficients[0] =
            (share_poly.share.coefficients[0] + 1) % fixture.profile.moduli[0];
        assert_eq!(
            verify_authenticated_share(
                &fixture.profile,
                &fixture.parties,
                &fixture.relations[2],
                &fixture.ciphertext,
                &fixture.relations[2].binding,
                &share_poly,
                TEST_SMUDGE_BITS,
            ),
            Err(ZkAmsMkheErrorV1::InvalidAuthentication)
        );

        let mut bad_relation = fixture.relations[2].clone();
        bad_relation.party_b.coefficients[0] =
            (bad_relation.party_b.coefficients[0] + 1) % fixture.profile.moduli[0];
        assert!(
            verify_authenticated_share(
                &fixture.profile,
                &fixture.parties,
                &bad_relation,
                &fixture.ciphertext,
                &fixture.relations[2].binding,
                &shares[2],
                TEST_SMUDGE_BITS,
            )
            .is_err()
        );

        for mutation in 0..4 {
            let mut proof = shares[2].clone();
            match mutation {
                0 => proof.proof.challenge_seed[0] ^= 1,
                1 => proof.proof.secret_response[0] += 1,
                2 => proof.proof.public_key_error_response[0] += 1,
                3 => {
                    proof.proof.smudge_response[0] = proof.proof.smudge_response[0]
                        .checked_add(&SignedWideV1::from_i64(1))
                        .unwrap();
                }
                _ => unreachable!(),
            }
            assert!(
                verify_decryption_relation(
                    &fixture.profile,
                    &fixture.parties,
                    &fixture.relations[2],
                    &fixture.ciphertext,
                    &proof.share,
                    TEST_SMUDGE_BITS,
                    &proof.proof,
                )
                .is_err(),
                "proof mutation {mutation} must reject"
            );
        }
        assert!(
            verify_decryption_relation(
                &fixture.profile,
                &fixture.parties,
                &fixture.relations[2],
                &fixture.ciphertext,
                &shares[2].share,
                TEST_SMUDGE_BITS + 1,
                &shares[2].proof,
            )
            .is_err(),
            "a proof must not replay under another smudging bound"
        );

        let mut signature = shares[2].clone();
        signature.authentication.signature[64] ^= 1;
        assert_eq!(
            verify_authenticated_share(
                &fixture.profile,
                &fixture.parties,
                &fixture.relations[2],
                &fixture.ciphertext,
                &fixture.relations[2].binding,
                &signature,
                TEST_SMUDGE_BITS,
            ),
            Err(ZkAmsMkheErrorV1::InvalidAuthentication)
        );
    }

    #[test]
    fn cross_party_relation_proof_authentication_and_ciphertext_splices_fail() {
        let fixture = fixture(b"decryption-splice-negative");
        let shares = make_shares(&fixture, b"decryption-splice-negative-shares");
        let index = 4;

        let mut proof_splice = shares[index].clone();
        proof_splice.proof = shares[index + 1].proof.clone();
        assert!(
            verify_decryption_relation(
                &fixture.profile,
                &fixture.parties,
                &fixture.relations[index],
                &fixture.ciphertext,
                &proof_splice.share,
                TEST_SMUDGE_BITS,
                &proof_splice.proof,
            )
            .is_err()
        );

        let mut authentication_splice = shares[index].clone();
        authentication_splice.authentication = shares[index + 1].authentication.clone();
        assert!(
            verify_authenticated_share(
                &fixture.profile,
                &fixture.parties,
                &fixture.relations[index],
                &fixture.ciphertext,
                &fixture.relations[index].binding,
                &authentication_splice,
                TEST_SMUDGE_BITS,
            )
            .is_err()
        );

        assert!(
            verify_authenticated_share(
                &fixture.profile,
                &fixture.parties,
                &fixture.relations[index + 1],
                &fixture.ciphertext,
                &fixture.relations[index].binding,
                &shares[index],
                TEST_SMUDGE_BITS,
            )
            .is_err()
        );

        for mutate_constant in [false, true] {
            let mut constant = fixture.ciphertext.constant().clone();
            let mut linear = fixture.ciphertext.linear().clone();
            let polynomial = if mutate_constant {
                &mut constant
            } else {
                &mut linear
            };
            polynomial.coefficients[0] =
                (polynomial.coefficients[0] + 1) % fixture.profile.moduli[0];
            let ciphertext = ZkAmsMkheCollectiveCiphertextV1::new(
                &fixture.profile,
                &fixture.parties,
                fixture.ciphertext.epoch(),
                fixture.ciphertext.transcript_digest(),
                fixture.ciphertext.sample_index(),
                fixture.ciphertext.level(),
                constant,
                linear,
            )
            .unwrap();
            assert!(
                verify_authenticated_share(
                    &fixture.profile,
                    &fixture.parties,
                    &fixture.relations[index],
                    &ciphertext,
                    &fixture.relations[index].binding,
                    &shares[index],
                    TEST_SMUDGE_BITS,
                )
                .is_err()
            );
        }

        let mut wrong_profile = fixture.profile.clone();
        wrong_profile.profile_id[0] ^= 1;
        assert!(
            verify_authenticated_share(
                &wrong_profile,
                &fixture.parties,
                &fixture.relations[index],
                &fixture.ciphertext,
                &fixture.relations[index].binding,
                &shares[index],
                TEST_SMUDGE_BITS,
            )
            .is_err()
        );
    }

    #[test]
    fn response_bounds_reject_one_step_over_every_family() {
        let fixture = fixture(b"decryption-response-bound-negative");
        let share = make_shares(&fixture, b"decryption-response-bound-shares").remove(0);
        let weight = wide_relation_challenge_weight(fixture.profile.ring_degree).unwrap();
        let (_, secret_limit) = small_response_parameters(1, weight, &fixture.profile).unwrap();
        let (_, error_limit) = small_response_parameters(
            i64::from(fixture.profile.error_eta),
            weight,
            &fixture.profile,
        )
        .unwrap();
        let (_, wide_limit, _) = wide_response_parameters(TEST_SMUDGE_BITS, weight).unwrap();

        let mut secret = share.proof.clone();
        secret.secret_response[0] = secret_limit + 1;
        let mut error = share.proof.clone();
        error.public_key_error_response[0] = error_limit + 1;
        let mut wide = share.proof.clone();
        wide.smudge_response[0] = SignedWideV1::new(
            false,
            wide_limit
                .checked_add(&WideMagnitudeV1 {
                    limbs: {
                        let mut limbs = [0_u64; DECRYPTION_MAX_WIDE_LIMBS_V1];
                        limbs[0] = 1;
                        limbs
                    },
                })
                .unwrap(),
        )
        .unwrap();
        for proof in [secret, error, wide] {
            assert_eq!(
                verify_decryption_relation(
                    &fixture.profile,
                    &fixture.parties,
                    &fixture.relations[0],
                    &fixture.ciphertext,
                    &share.share,
                    TEST_SMUDGE_BITS,
                    &proof,
                ),
                Err(ZkAmsMkheErrorV1::InvalidShareProof)
            );
        }
    }

    #[test]
    fn decoder_preflights_truncation_extension_counts_residues_and_negative_zero() {
        let fixture = fixture(b"decryption-wire-negative");
        let share = make_shares(&fixture, b"decryption-wire-negative-shares").remove(0);
        let encoded = share.encode(&fixture.profile, TEST_SMUDGE_BITS).unwrap();
        for malformed in [
            encoded[..encoded.len() - 1].to_vec(),
            {
                let mut value = encoded.clone();
                value.push(0);
                value
            },
            {
                let mut value = encoded.clone();
                // Residue count begins immediately before the proof length.
                let offset = TEST_DECRYPTION_SHARE_HEADER_BYTES_V1 - 4;
                value[offset..offset + 4].copy_from_slice(&u32::MAX.to_be_bytes());
                value
            },
        ] {
            assert!(
                AuthenticatedDecryptionShareV1::decode_exact(
                    &malformed,
                    &fixture.profile,
                    &fixture.parties,
                    &fixture.relations[0].binding,
                    TEST_SMUDGE_BITS,
                )
                .is_err()
            );
        }

        let polynomial_offset = TEST_DECRYPTION_SHARE_HEADER_BYTES_V1 + 4;
        let mut noncanonical = encoded.clone();
        noncanonical[polynomial_offset..polynomial_offset + 8]
            .copy_from_slice(&fixture.profile.moduli[0].to_be_bytes());
        assert!(
            AuthenticatedDecryptionShareV1::decode_exact(
                &noncanonical,
                &fixture.profile,
                &fixture.parties,
                &fixture.relations[0].binding,
                TEST_SMUDGE_BITS,
            )
            .is_err()
        );

        let proof_offset = TEST_DECRYPTION_SHARE_HEADER_BYTES_V1
            + checked_rns_polynomial_bytes(&fixture.profile).unwrap();
        let wide_bytes = usize::from(share.proof.wide_response_bytes);
        let wide_offset =
            proof_offset + DECRYPTION_PROOF_HEADER_BYTES_V1 + fixture.profile.ring_degree * 16;
        let mut negative_zero = encoded.clone();
        negative_zero[wide_offset..wide_offset + wide_bytes].fill(0);
        negative_zero[wide_offset] = 0x80;
        assert!(
            AuthenticatedDecryptionShareV1::decode_exact(
                &negative_zero,
                &fixture.profile,
                &fixture.parties,
                &fixture.relations[0].binding,
                TEST_SMUDGE_BITS,
            )
            .is_err()
        );
    }

    #[test]
    fn final_centered_correctness_bound_is_enforced_after_all_proofs() {
        let fixture = fixture(b"decryption-bound-negative");
        let shares = make_shares(&fixture, b"decryption-bound-negative-shares");
        let abort = aggregate_and_decrypt_full_roster(
            &fixture.profile,
            &fixture.parties,
            &fixture.relations,
            &fixture.ciphertext,
            &shares,
            TEST_SMUDGE_BITS,
            1,
        )
        .unwrap_err();
        assert_eq!(
            abort.reason,
            DecryptionAbortReasonV1::CorrectnessBoundExceeded
        );
    }
}
