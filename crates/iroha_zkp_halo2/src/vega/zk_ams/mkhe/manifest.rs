//! Frozen ZK-AMS MKHE release parameters and fail-closed readiness gates.

use super::{
    BgvProfile, PlaintextModulus, ZkAmsMkheErrorV1, modulus_product_bit_len,
    noise::{ZkAmsMkheNoiseCertificateV1, derive_noise_certificate_v1},
    phase23::{
        zk_ams_phase23_equation_certificate_digest_v1, zk_ams_phase23_equation_certificate_v1,
    },
    resource::{ZkAmsMkheResourceCertificateV1, derive_resource_certificate_v1},
    security::{
        ZkAmsMkheSecurityCandidateV1, ZkAmsMkheSecurityCertificateV1, derive_security_candidate_v1,
        frozen_security_certificate_v1, security_candidate_input_digest_v1,
    },
};
use crate::vega::sponge::keccak256;

/// Frozen power-of-two cyclotomic degree of the collective-ingress candidate.
pub const ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1: usize = 131_072;
/// Exact number of T256 base-field values packed through conjugate `Fp2` slots.
pub const ZK_AMS_MKHE_RELEASE_SLOT_COUNT_V1: usize = 65_536;
/// Fixed governed roster size; this is not an on-the-fly MKHE key union.
pub const ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1: usize = 8;
/// Target classical security strength for activation.
pub const ZK_AMS_MKHE_TARGET_SECURITY_BITS_V1: u16 = 128;
/// Statistical hiding strength for CKS and final-share smudging.
pub const ZK_AMS_MKHE_STATISTICAL_SECURITY_BITS_V1: u16 = 128;
/// Exact bit length of the full 38-prime ciphertext modulus product.
pub const ZK_AMS_MKHE_CIPHERTEXT_MODULUS_BITS_V1: u16 = 2_280;
/// Conservative upper bound on the final centered decryption residual.
pub const ZK_AMS_MKHE_FINAL_DECRYPTION_BOUND_BITS_V1: u16 = 2_115;
/// Strict headroom between the symbolic final bound and `Q/2`.
pub const ZK_AMS_MKHE_CORRECTNESS_MARGIN_BITS_V1: u16 = 164;
/// Maximum possible nonzero count of one compiled sparse R1CS row.
pub const ZK_AMS_MKHE_SPARSE_MAP_FAN_IN_CEILING_V1: usize = 524_378;

const RELEASE_PROFILE_ID_V1: [u8; 32] = [
    0x26, 0x07, 0xf2, 0x03, 0x92, 0x5d, 0x98, 0xf4, 0xfb, 0xed, 0x1d, 0x27, 0xbb, 0xef, 0x1b, 0x09,
    0x56, 0xb2, 0x01, 0x67, 0xf3, 0x02, 0x16, 0x3b, 0x2b, 0x14, 0x31, 0x3f, 0x7d, 0x48, 0x9f, 0xd5,
];

pub(super) const RELEASE_MODULI_V1: [u64; 38] = [
    1_152_921_504_606_584_833,
    1_152_921_504_598_720_513,
    1_152_921_504_592_429_057,
    1_152_921_504_581_419_009,
    1_152_921_504_580_894_721,
    1_152_921_504_578_273_281,
    1_152_921_504_577_748_993,
    1_152_921_504_577_486_849,
    1_152_921_504_568_836_097,
    1_152_921_504_565_166_081,
    1_152_921_504_563_331_073,
    1_152_921_504_556_515_329,
    1_152_921_504_555_466_753,
    1_152_921_504_554_156_033,
    1_152_921_504_552_583_169,
    1_152_921_504_542_883_841,
    1_152_921_504_538_951_681,
    1_152_921_504_537_378_817,
    1_152_921_504_531_873_793,
    1_152_921_504_521_650_177,
    1_152_921_504_509_853_697,
    1_152_921_504_508_280_833,
    1_152_921_504_506_970_113,
    1_152_921_504_495_697_921,
    1_152_921_504_491_241_473,
    1_152_921_504_488_620_033,
    1_152_921_504_479_444_993,
    1_152_921_504_470_794_241,
    1_152_921_504_468_172_801,
    1_152_921_504_462_929_921,
    1_152_921_504_462_667_777,
    1_152_921_504_455_589_889,
    1_152_921_504_447_987_713,
    1_152_921_504_442_482_689,
    1_152_921_504_436_191_233,
    1_152_921_504_427_278_337,
    1_152_921_504_419_414_017,
    1_152_921_504_409_190_401,
];

pub(super) const RELEASE_NEGACYCLIC_ROOTS_V1: [u64; 38] = [
    720_645_352_895_426_071,
    282_755_386_997_791_573,
    1_129_868_644_045_593_393,
    853_812_227_483_389_373,
    313_941_090_484_177_697,
    430_486_680_513_317_260,
    143_942_864_930_673_074,
    807_173_726_984_510_404,
    191_722_530_547_666_486,
    467_567_141_367_137_610,
    941_895_608_111_266_529,
    164_841_987_874_738_392,
    662_956_088_516_163_749,
    418_880_473_612_227_419,
    392_461_511_604_930_516,
    764_249_630_711_722_482,
    864_013_988_376_557_277,
    705_763_476_696_323_117,
    1_036_023_418_809_922_092,
    1_093_496_573_364_979_026,
    465_626_502_647_312_456,
    108_719_633_419_962_724,
    1_009_384_194_290_538_050,
    926_844_163_581_853_650,
    935_039_477_417_276_816,
    950_668_019_576_080_971,
    551_479_639_661_014_597,
    612_386_825_931_585_809,
    452_213_060_731_776_498,
    215_387_729_362_370_611,
    506_439_537_974_696_847,
    1_138_741_943_693_016_536,
    378_985_449_492_583_188,
    143_344_989_960_478_445,
    879_283_036_444_379_690,
    150_226_471_703_910_190,
    1_049_010_867_608_938_030,
    533_899_346_966_036_544,
];

/// Frozen, consensus-digestible MKHE release manifest.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ZkAmsMkheReleaseManifestV1 {
    /// Manifest schema version.
    pub version: u8,
    /// Exact native parameter identifier.
    pub profile_id: [u8; 32],
    /// Power-of-two cyclotomic degree.
    pub ring_degree: u32,
    /// Number of conjugate-pair T256 slots.
    pub slot_count: u32,
    /// Fixed governed roster size.
    pub roster_size: u8,
    /// Digest of the explicit collective-ingress-hybrid semantics.
    pub construction_digest: [u8; 32],
    /// Number of frozen RNS primes.
    pub rns_limb_count: u8,
    /// Bit width of the product of all RNS primes.
    pub ciphertext_modulus_bits: u16,
    /// Centered bit ceiling for each streamed hybrid RNS digit.
    pub hybrid_digit_bits: u8,
    /// Exact number of streamed hybrid digits.
    pub hybrid_digit_count: u8,
    /// Centered-binomial error parameter.
    pub error_eta: u8,
    /// Claimed target classical security strength.
    pub target_security_bits: u16,
    /// Statistical CKS/share-hiding strength.
    pub statistical_security_bits: u16,
    /// Independently certified classical security strength; zero means absent.
    pub certified_security_bits: u16,
    /// Maximum RLWE samples admitted under one secret and governed epoch.
    pub max_samples_per_secret_epoch: u64,
    /// Symbolic final centered decryption residual in bits.
    pub final_decryption_bound_bits: u16,
    /// Strict remaining correctness headroom in bits.
    pub correctness_margin_bits: u16,
    /// Per-ciphertext canonical byte ceiling.
    pub max_ciphertext_bytes: u64,
    /// Per-evaluated-key canonical byte ceiling.
    pub max_evaluated_key_bytes: u64,
    /// Per-round canonical byte ceiling.
    pub max_round_bytes: u64,
    /// Per-decryption-share canonical byte ceiling.
    pub max_share_bytes: u64,
    /// Per-operation canonical workspace ceiling.
    pub max_workspace_bytes: u64,
    /// Per-operation abstract work-unit ceiling.
    pub max_work_units: u64,
    /// Reproducible estimator certificate digest; zero means absent.
    pub security_certificate_digest: [u8; 32],
    /// Digest of the exact estimator inputs; this is not a security result.
    pub security_candidate_input_digest: [u8; 32],
    /// Digest of exact static resource accounting and its open evidence bits.
    pub resource_certificate_digest: [u8; 32],
    /// Digest of Equations (6)--(11), finalization semantics, and closure bits.
    pub phase23_equation_certificate_digest: [u8; 32],
    /// Release-size execution KAT digest; zero means absent.
    pub release_kat_digest: [u8; 32],
}

/// Native readiness result. Every field is bound by the readiness digest.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ZkAmsMkheReadinessV1 {
    /// Frozen profile validates natively.
    pub parameter_gate: bool,
    /// A reproducible 128-bit RLWE estimator certificate is pinned and verified.
    pub security_gate: bool,
    /// The symbolic correctness/noise schedule validates with strict headroom.
    pub noise_gate: bool,
    /// Release-size byte/work/peak-memory evidence fits every Taira ceiling.
    pub resource_gate: bool,
    /// Canonical wire/pre-decode coverage is complete.
    pub wire_gate: bool,
    /// Governed roster and contribution well-formedness checks are complete.
    pub malicious_party_gate: bool,
    /// Full-set authenticated partial decryption and share PoK are complete.
    pub decryption_share_gate: bool,
    /// Exact T256 `Fp2` packing, padding, and rotations are complete.
    pub packing_gate: bool,
    /// Sparse A/B/C and Phase II/III orchestration are complete.
    pub phase23_gate: bool,
    /// A full release-parameter KAT completed and matches its governed digest.
    pub release_kat_gate: bool,
}

impl ZkAmsMkheReadinessV1 {
    /// Return true only when every release obligation is closed.
    #[must_use]
    pub const fn is_ready(self) -> bool {
        self.parameter_gate
            && self.security_gate
            && self.noise_gate
            && self.resource_gate
            && self.wire_gate
            && self.malicious_party_gate
            && self.decryption_share_gate
            && self.packing_gate
            && self.phase23_gate
            && self.release_kat_gate
    }
}

pub(super) fn release_profile_v1() -> BgvProfile {
    BgvProfile {
        profile_id: RELEASE_PROFILE_ID_V1,
        ring_degree: ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1,
        moduli: &RELEASE_MODULI_V1,
        negacyclic_roots: &RELEASE_NEGACYCLIC_ROOTS_V1,
        plaintext_modulus: PlaintextModulus::T256,
        error_eta: 2,
        hybrid_rns_decomposition: true,
        gadget_base_log: 60,
        gadget_digits: RELEASE_MODULI_V1.len(),
        max_ciphertext_bytes: 96 * 1024 * 1024,
        max_evaluated_key_bytes: 2 * 1024 * 1024 * 1024,
        max_round_bytes: 64 * 1024 * 1024,
        max_share_bytes: 64 * 1024 * 1024,
        max_workspace_bytes: 160 * 1024 * 1024,
        max_work_units: 100_000_000_000,
    }
}

const COLLECTIVE_INGRESS_CONSTRUCTION_V1: &[u8] = b"iroha.zk-ams.v1.collective-ingress-hybrid:not-cdks19:on-entry-independent-owner-key:fixed-roster-sum-secret:proof-bound-cks:collective-s2-rkg:all-roster-decryption:transparent-seeded-a";

/// Return the machine-checked conservative Phase-II/III noise certificate.
pub fn zk_ams_mkhe_noise_certificate_v1() -> Result<ZkAmsMkheNoiseCertificateV1, ZkAmsMkheErrorV1> {
    derive_noise_certificate_v1(
        ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1,
        ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1,
        RELEASE_MODULI_V1.len(),
        ZK_AMS_MKHE_SPARSE_MAP_FAN_IN_CEILING_V1,
        8,
        usize::from(ZK_AMS_MKHE_CIPHERTEXT_MODULUS_BITS_V1),
        ZK_AMS_MKHE_STATISTICAL_SECURITY_BITS_V1,
    )
}

/// Return exact static byte/work accounting and explicit open evidence bits.
pub fn zk_ams_mkhe_resource_certificate_v1()
-> Result<ZkAmsMkheResourceCertificateV1, ZkAmsMkheErrorV1> {
    derive_resource_certificate_v1(&release_profile_v1(), ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1)
}

/// Return the consensus digest of exact resource accounting and evidence state.
pub fn zk_ams_mkhe_resource_certificate_digest_v1() -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    let profile = release_profile_v1();
    let certificate = zk_ams_mkhe_resource_certificate_v1()?;
    let mut frame = Vec::with_capacity(192);
    frame.extend_from_slice(b"iroha.zk-ams.v1.mkhe.resource-certificate");
    frame.extend_from_slice(&profile.resource_policy_digest()?);
    frame.extend_from_slice(&certificate.governed_roster_wire_bytes.to_be_bytes());
    frame.extend_from_slice(&certificate.ring_degree.to_be_bytes());
    frame.push(certificate.rns_limb_count);
    frame.push(certificate.max_composed_rotation_key_switch_count);
    frame.push(certificate.collective_evaluated_key_count);
    for value in [
        certificate.rns_polynomial_wire_bytes,
        certificate.compact_collective_ciphertext_wire_bytes,
        certificate.multiplication_triple_wire_bytes,
        certificate.seeded_collective_relinearization_key_wire_bytes,
        certificate.total_collective_evaluated_key_artifact_bytes,
        certificate.streamed_contribution_base_wire_bytes,
        certificate.proof_envelope_header_wire_bytes,
        certificate.max_round_contribution_proof_bytes,
        certificate.max_decryption_share_proof_bytes,
        certificate.streamed_hybrid_workspace_bytes,
        certificate.ring_multiplication_work_units,
        certificate.hybrid_key_switch_work_units,
        certificate.max_composed_rotation_work_units,
    ] {
        frame.extend_from_slice(&value.to_be_bytes());
    }
    frame.extend_from_slice(&[
        certificate.ciphertext_ceiling_met.into(),
        certificate.per_evaluated_key_ceiling_met.into(),
        certificate.workspace_ceiling_met.into(),
        certificate.composed_rotation_work_ceiling_met.into(),
        certificate.contribution_proof_sizes_certified.into(),
        certificate
            .evaluated_key_artifact_transport_certified
            .into(),
        certificate.phase23_work_measured.into(),
        certificate.release_peak_memory_measured.into(),
    ]);
    Ok(keccak256(&frame))
}

/// Return the exact frozen RLWE estimator inputs.
pub fn zk_ams_mkhe_security_candidate_v1() -> Result<ZkAmsMkheSecurityCandidateV1, ZkAmsMkheErrorV1>
{
    derive_security_candidate_v1(&release_profile_v1(), ZK_AMS_MKHE_TARGET_SECURITY_BITS_V1)
}

/// Return the consensus digest of the exact estimator inputs.
///
/// The accepted estimator output is deliberately excluded. It belongs in
/// `security_certificate_digest`, so publishing a result cannot mutate the
/// parameter/input identity that the result certifies.
pub fn zk_ams_mkhe_security_candidate_input_digest_v1() -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    let candidate = zk_ams_mkhe_security_candidate_v1()?;
    Ok(security_candidate_input_digest_v1(candidate))
}

/// Return the immutable estimator result for the exact release profile.
///
/// The returned value has no public constructor or mutable result fields. Its
/// private validator checks the exact profile/input digest, estimator and Sage
/// provenance, ordered attack records, minimum cost, and certificate digest.
pub fn zk_ams_mkhe_security_certificate_v1()
-> Result<ZkAmsMkheSecurityCertificateV1, ZkAmsMkheErrorV1> {
    frozen_security_certificate_v1(zk_ams_mkhe_security_candidate_v1()?)
}

/// Return and natively validate the frozen candidate release manifest.
pub fn zk_ams_mkhe_release_manifest_v1() -> Result<ZkAmsMkheReleaseManifestV1, ZkAmsMkheErrorV1> {
    let profile = release_profile_v1();
    profile.validate()?;
    let modulus_bits = u16::try_from(modulus_product_bit_len(profile.moduli)?)
        .map_err(|_| ZkAmsMkheErrorV1::InvalidProfile)?;
    let noise = zk_ams_mkhe_noise_certificate_v1()?;
    let security = zk_ams_mkhe_security_candidate_v1()?;
    let security_certificate = zk_ams_mkhe_security_certificate_v1()?;
    if modulus_bits != ZK_AMS_MKHE_CIPHERTEXT_MODULUS_BITS_V1
        || noise.final_decryption_residual_bits != ZK_AMS_MKHE_FINAL_DECRYPTION_BOUND_BITS_V1
        || noise.correctness_margin_bits != ZK_AMS_MKHE_CORRECTNESS_MARGIN_BITS_V1
    {
        return Err(ZkAmsMkheErrorV1::InvalidProfile);
    }
    Ok(ZkAmsMkheReleaseManifestV1 {
        version: 1,
        profile_id: profile.profile_id,
        ring_degree: u32::try_from(profile.ring_degree)
            .map_err(|_| ZkAmsMkheErrorV1::InvalidProfile)?,
        slot_count: u32::try_from(ZK_AMS_MKHE_RELEASE_SLOT_COUNT_V1)
            .map_err(|_| ZkAmsMkheErrorV1::InvalidProfile)?,
        roster_size: u8::try_from(ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1)
            .map_err(|_| ZkAmsMkheErrorV1::InvalidProfile)?,
        construction_digest: keccak256(COLLECTIVE_INGRESS_CONSTRUCTION_V1),
        rns_limb_count: u8::try_from(profile.moduli.len())
            .map_err(|_| ZkAmsMkheErrorV1::InvalidProfile)?,
        ciphertext_modulus_bits: modulus_bits,
        hybrid_digit_bits: profile.gadget_base_log,
        hybrid_digit_count: u8::try_from(profile.gadget_digits)
            .map_err(|_| ZkAmsMkheErrorV1::InvalidProfile)?,
        error_eta: profile.error_eta,
        target_security_bits: ZK_AMS_MKHE_TARGET_SECURITY_BITS_V1,
        statistical_security_bits: ZK_AMS_MKHE_STATISTICAL_SECURITY_BITS_V1,
        certified_security_bits: security_certificate.minimum_security_bits(),
        max_samples_per_secret_epoch: security.max_samples_per_secret_epoch,
        final_decryption_bound_bits: noise.final_decryption_residual_bits,
        correctness_margin_bits: ZK_AMS_MKHE_CORRECTNESS_MARGIN_BITS_V1,
        max_ciphertext_bytes: profile.max_ciphertext_bytes as u64,
        max_evaluated_key_bytes: profile.max_evaluated_key_bytes as u64,
        max_round_bytes: profile.max_round_bytes as u64,
        max_share_bytes: profile.max_share_bytes as u64,
        max_workspace_bytes: profile.max_workspace_bytes as u64,
        max_work_units: profile.max_work_units,
        security_certificate_digest: security_certificate.certificate_digest(),
        security_candidate_input_digest: zk_ams_mkhe_security_candidate_input_digest_v1()?,
        resource_certificate_digest: zk_ams_mkhe_resource_certificate_digest_v1()?,
        phase23_equation_certificate_digest: zk_ams_phase23_equation_certificate_digest_v1(),
        release_kat_digest: [0; 32],
    })
}

/// Return the consensus digest of the exact release manifest and prime/root chain.
pub fn zk_ams_mkhe_manifest_digest_v1() -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    let manifest = zk_ams_mkhe_release_manifest_v1()?;
    let mut frame = Vec::with_capacity(512);
    frame.extend_from_slice(b"iroha.zk-ams.v1.mkhe.release-manifest");
    frame.push(manifest.version);
    frame.extend_from_slice(&manifest.profile_id);
    frame.extend_from_slice(&manifest.ring_degree.to_be_bytes());
    frame.extend_from_slice(&manifest.slot_count.to_be_bytes());
    frame.push(manifest.roster_size);
    frame.extend_from_slice(&manifest.construction_digest);
    frame.push(manifest.rns_limb_count);
    frame.extend_from_slice(&manifest.ciphertext_modulus_bits.to_be_bytes());
    frame.push(manifest.hybrid_digit_bits);
    frame.push(manifest.hybrid_digit_count);
    frame.push(manifest.error_eta);
    frame.extend_from_slice(&manifest.target_security_bits.to_be_bytes());
    frame.extend_from_slice(&manifest.statistical_security_bits.to_be_bytes());
    frame.extend_from_slice(&manifest.certified_security_bits.to_be_bytes());
    frame.extend_from_slice(&manifest.max_samples_per_secret_epoch.to_be_bytes());
    frame.extend_from_slice(&manifest.final_decryption_bound_bits.to_be_bytes());
    frame.extend_from_slice(&manifest.correctness_margin_bits.to_be_bytes());
    for value in [
        manifest.max_ciphertext_bytes,
        manifest.max_evaluated_key_bytes,
        manifest.max_round_bytes,
        manifest.max_share_bytes,
        manifest.max_workspace_bytes,
        manifest.max_work_units,
    ] {
        frame.extend_from_slice(&value.to_be_bytes());
    }
    for (&modulus, root) in RELEASE_MODULI_V1.iter().zip(RELEASE_NEGACYCLIC_ROOTS_V1) {
        frame.extend_from_slice(&modulus.to_be_bytes());
        frame.extend_from_slice(&root.to_be_bytes());
    }
    frame.extend_from_slice(&manifest.security_certificate_digest);
    frame.extend_from_slice(&manifest.security_candidate_input_digest);
    frame.extend_from_slice(&manifest.resource_certificate_digest);
    frame.extend_from_slice(&manifest.phase23_equation_certificate_digest);
    frame.extend_from_slice(&manifest.release_kat_digest);
    Ok(keccak256(&frame))
}

/// Evaluate every release-readiness gate without silently downgrading.
pub fn zk_ams_mkhe_readiness_v1() -> Result<ZkAmsMkheReadinessV1, ZkAmsMkheErrorV1> {
    let manifest = zk_ams_mkhe_release_manifest_v1()?;
    let noise = zk_ams_mkhe_noise_certificate_v1()?;
    let resource = zk_ams_mkhe_resource_certificate_v1()?;
    let phase23 = zk_ams_phase23_equation_certificate_v1();
    let security = zk_ams_mkhe_security_certificate_v1()?;
    Ok(ZkAmsMkheReadinessV1 {
        parameter_gate: true,
        security_gate: security.security_parameters_digest()
            == release_profile_v1().security_parameters_digest()?
            && security.candidate_input_digest() == manifest.security_candidate_input_digest
            && security.minimum_security_bits() == manifest.certified_security_bits
            && security.target_security_bits() == manifest.target_security_bits
            && security.certificate_digest() == manifest.security_certificate_digest,
        noise_gate: noise.final_decryption_residual_bits == manifest.final_decryption_bound_bits
            && noise.correctness_margin_bits == manifest.correctness_margin_bits
            && manifest.correctness_margin_bits >= 64,
        resource_gate: resource.is_release_ready()
            && manifest.resource_certificate_digest
                == zk_ams_mkhe_resource_certificate_digest_v1()?,
        // Static wire formulas and bounded toy-profile KATs are not a
        // substitute for a pinned release-parameter positive/negative KAT.
        wire_gate: false,
        malicious_party_gate: false,
        decryption_share_gate: false,
        packing_gate: false,
        phase23_gate: phase23.is_complete()
            && manifest.phase23_equation_certificate_digest
                == zk_ams_phase23_equation_certificate_digest_v1(),
        release_kat_gate: manifest.release_kat_digest != [0; 32],
    })
}

/// Return a consensus digest binding the manifest and every readiness bit.
pub fn zk_ams_mkhe_readiness_digest_v1() -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    let readiness = zk_ams_mkhe_readiness_v1()?;
    let mut frame = Vec::with_capacity(96);
    frame.extend_from_slice(b"iroha.zk-ams.v1.mkhe.release-readiness");
    frame.extend_from_slice(&zk_ams_mkhe_manifest_digest_v1()?);
    frame.extend_from_slice(&[
        readiness.parameter_gate.into(),
        readiness.security_gate.into(),
        readiness.noise_gate.into(),
        readiness.resource_gate.into(),
        readiness.wire_gate.into(),
        readiness.malicious_party_gate.into(),
        readiness.decryption_share_gate.into(),
        readiness.packing_gate.into(),
        readiness.phase23_gate.into(),
        readiness.release_kat_gate.into(),
    ]);
    Ok(keccak256(&frame))
}

pub(in crate::vega::zk_ams) fn require_release_ready_v1() -> Result<(), ZkAmsMkheErrorV1> {
    if !zk_ams_mkhe_readiness_v1()?.is_ready() {
        return Err(ZkAmsMkheErrorV1::ReleaseUnavailable);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn estimator_input_identity_binds_every_input_class() {
        let candidate = zk_ams_mkhe_security_candidate_v1().expect("security candidate");
        let expected = security_candidate_input_digest_v1(candidate);

        for changed in [
            ZkAmsMkheSecurityCandidateV1 {
                security_parameters_digest: [7; 32],
                ..candidate
            },
            ZkAmsMkheSecurityCandidateV1 {
                ciphertext_modulus_bits: candidate.ciphertext_modulus_bits + 1,
                ..candidate
            },
            ZkAmsMkheSecurityCandidateV1 {
                max_samples_per_secret_epoch: candidate.max_samples_per_secret_epoch + 1,
                ..candidate
            },
            ZkAmsMkheSecurityCandidateV1 {
                target_security_bits: candidate.target_security_bits + 1,
                ..candidate
            },
        ] {
            assert_ne!(security_candidate_input_digest_v1(changed), expected);
        }
    }

    #[test]
    fn exact_estimator_evidence_closes_only_the_security_gate() {
        let certificate = zk_ams_mkhe_security_certificate_v1().expect("security certificate");
        let manifest = zk_ams_mkhe_release_manifest_v1().expect("manifest");
        let readiness = zk_ams_mkhe_readiness_v1().expect("readiness");

        assert_eq!(manifest.certified_security_bits, 172);
        assert_eq!(manifest.target_security_bits, 128);
        assert_eq!(
            manifest.security_candidate_input_digest,
            certificate.candidate_input_digest()
        );
        assert_eq!(
            manifest.security_certificate_digest,
            certificate.certificate_digest()
        );
        assert!(readiness.security_gate);
        assert!(!readiness.is_ready());
        assert_eq!(
            require_release_ready_v1(),
            Err(ZkAmsMkheErrorV1::ReleaseUnavailable)
        );
    }
}
