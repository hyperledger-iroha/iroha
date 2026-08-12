//! Frozen first-release ZK-AMS MKHE manifest and fail-closed readiness facade.
//!
//! The native MKHE implementation is unavailable in the first release. These
//! snapshots preserve its candidate identity while every runtime entry fails closed.

use core::fmt;

/// Failure at the first-release ZK-AMS MKHE boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ZkAmsMkheErrorV1 {
    /// The complete governed release gates are not closed.
    ReleaseUnavailable,
}

impl fmt::Display for ZkAmsMkheErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ZK-AMS MKHE Phase II/III release profile is unavailable")
    }
}

impl std::error::Error for ZkAmsMkheErrorV1 {}

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
    /// Centered-binomial parameter used only by actual RLWE/key sampling.
    pub error_eta: u8,
    /// Exact minimum coefficient sampled by actual RLWE/key errors.
    pub sampled_rlwe_error_min: i16,
    /// Exact maximum coefficient sampled by actual RLWE/key errors.
    pub sampled_rlwe_error_max: i16,
    /// Maximum absolute coefficient sampled by actual RLWE/key errors.
    pub sampled_rlwe_error_max_abs: u8,
    /// Strict power-of-two magnitude bound for sampled RLWE/key errors.
    pub sampled_rlwe_error_bound_bits: u8,
    /// Exact asymmetric minimum of canonical-natural-lift proof witness `e0'`.
    pub natural_lift_effective_error_min: i16,
    /// Exact asymmetric maximum of canonical-natural-lift proof witness `e0'`.
    pub natural_lift_effective_error_max: i16,
    /// Symmetric verifier maximum absolute value admitted for `e0'`.
    pub natural_lift_effective_error_verifier_max_abs: u8,
    /// Strict power-of-two magnitude bound for verifier-admitted `e0'`.
    pub natural_lift_effective_error_bound_bits: u8,
    /// Minimum natural-to-centered upper-half correction bit.
    pub natural_lift_upper_half_correction_min: u8,
    /// Maximum natural-to-centered upper-half correction bit.
    pub natural_lift_upper_half_correction_max: u8,
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
    /// Digest of the exact release-degree positive and adversarial packing KAT.
    pub packing_certificate_digest: [u8; 32],
    /// Digest of Equations (6)--(11), finalization semantics, and closure bits.
    pub phase23_equation_certificate_digest: [u8; 32],
    /// Digest of the exact small-witness active-binding audit and blocker set.
    pub active_exact_binding_audit_digest: [u8; 32],
    /// Digest of the exact split-decryption resource and transport evidence.
    pub decryption_resource_evidence_digest: [u8; 32],
    /// Digest of the exact verified-receipt capability graph and open handoffs.
    pub receipt_capability_audit_digest: [u8; 32],
    /// Exact open verified-receipt handoff bit set.
    pub receipt_capability_blocker_mask: u16,
    /// True only when every mandatory verified-receipt handoff closes together.
    pub receipt_capability_release_available: bool,
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
    /// Every operational path consumes the required opaque verified receipts.
    pub receipt_capability_gate: bool,
    /// Exact open verified-receipt handoff bit set; readiness requires zero.
    pub receipt_capability_blocker_mask: u16,
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
            && self.receipt_capability_gate
            && self.receipt_capability_blocker_mask == 0
            && self.release_kat_gate
    }
}

const RELEASE_MANIFEST_V1: ZkAmsMkheReleaseManifestV1 = ZkAmsMkheReleaseManifestV1 {
    version: 1,
    profile_id: [
        0x26, 0x07, 0xf2, 0x03, 0x92, 0x5d, 0x98, 0xf4, 0xfb, 0xed, 0x1d, 0x27, 0xbb, 0xef, 0x1b,
        0x09, 0x56, 0xb2, 0x01, 0x67, 0xf3, 0x02, 0x16, 0x3b, 0x2b, 0x14, 0x31, 0x3f, 0x7d, 0x48,
        0x9f, 0xd5,
    ],
    ring_degree: 131_072,
    slot_count: 65_536,
    roster_size: 8,
    construction_digest: [
        0xe7, 0x45, 0xca, 0x7e, 0xe4, 0x5a, 0xc3, 0x88, 0x76, 0x78, 0x98, 0x11, 0x5a, 0xfc, 0xa5,
        0x79, 0x5c, 0x78, 0x7d, 0x02, 0x1d, 0x62, 0xbb, 0xf9, 0x46, 0x14, 0xb8, 0x8c, 0xcd, 0xa3,
        0x8f, 0x3c,
    ],
    rns_limb_count: 38,
    ciphertext_modulus_bits: 2_280,
    hybrid_digit_bits: 60,
    hybrid_digit_count: 38,
    error_eta: 2,
    sampled_rlwe_error_min: -2,
    sampled_rlwe_error_max: 2,
    sampled_rlwe_error_max_abs: 2,
    sampled_rlwe_error_bound_bits: 2,
    natural_lift_effective_error_min: -3,
    natural_lift_effective_error_max: 2,
    natural_lift_effective_error_verifier_max_abs: 3,
    natural_lift_effective_error_bound_bits: 2,
    natural_lift_upper_half_correction_min: 0,
    natural_lift_upper_half_correction_max: 1,
    target_security_bits: 128,
    statistical_security_bits: 128,
    certified_security_bits: 172,
    max_samples_per_secret_epoch: 67_108_864,
    final_decryption_bound_bits: 2_115,
    correctness_margin_bits: 164,
    max_ciphertext_bytes: 100_663_296,
    max_evaluated_key_bytes: 2_147_483_648,
    max_round_bytes: 67_108_864,
    max_share_bytes: 67_108_864,
    max_workspace_bytes: 167_772_160,
    max_work_units: 100_000_000_000,
    security_certificate_digest: [
        0xc4, 0xee, 0x05, 0xce, 0xd7, 0x38, 0xf4, 0x41, 0xa2, 0x5c, 0xd6, 0x6b, 0x5d, 0x87, 0x0d,
        0x25, 0xe1, 0x05, 0x75, 0x7e, 0x3e, 0xd9, 0x87, 0x1d, 0x8b, 0x76, 0x96, 0xba, 0x80, 0x18,
        0x1d, 0x72,
    ],
    security_candidate_input_digest: [
        0x64, 0x4f, 0xeb, 0x34, 0x47, 0xa9, 0x90, 0x66, 0x62, 0x46, 0x62, 0x19, 0x36, 0x4e, 0x2e,
        0x07, 0x9e, 0x71, 0x74, 0x71, 0x09, 0x75, 0x9b, 0x3f, 0x8d, 0x8b, 0x3d, 0x44, 0xb9, 0xc1,
        0xa3, 0x60,
    ],
    resource_certificate_digest: [
        0xfe, 0x8f, 0x59, 0xa3, 0x03, 0xda, 0x5f, 0x18, 0x87, 0x17, 0xa4, 0x78, 0x4f, 0x6e, 0xb0,
        0x4e, 0xf6, 0x5d, 0x56, 0x00, 0x8a, 0x89, 0xa5, 0x9a, 0x0a, 0x74, 0x42, 0x68, 0x11, 0x80,
        0xe1, 0xb2,
    ],
    packing_certificate_digest: [
        0xb8, 0x9e, 0xaa, 0xd1, 0x47, 0xc0, 0xd1, 0x6b, 0x2d, 0xcf, 0xcd, 0xcf, 0x6b, 0x4e, 0xf1,
        0xe2, 0xcf, 0xf9, 0xb0, 0x9d, 0x7e, 0xd6, 0x4e, 0xb7, 0x20, 0x8f, 0xd6, 0x37, 0x3d, 0x61,
        0x8f, 0x1c,
    ],
    phase23_equation_certificate_digest: [
        0x9c, 0x17, 0x85, 0x33, 0x04, 0x20, 0x8b, 0x7f, 0x78, 0x82, 0xd4, 0xa8, 0x5b, 0x8c, 0x4c,
        0xa2, 0x20, 0xea, 0xbc, 0x44, 0x56, 0x5c, 0xf6, 0xc9, 0x75, 0xf2, 0x54, 0x3c, 0x9f, 0xc1,
        0x5f, 0x24,
    ],
    active_exact_binding_audit_digest: [
        0xa2, 0x66, 0x4a, 0xec, 0x46, 0xc6, 0x7d, 0x38, 0xd7, 0x76, 0x10, 0x9e, 0xb4, 0x08, 0x18,
        0x9e, 0x93, 0xe4, 0x0b, 0x5d, 0x54, 0x49, 0xac, 0xb5, 0xf8, 0xe5, 0xed, 0x90, 0x1c, 0x23,
        0xe9, 0x96,
    ],
    decryption_resource_evidence_digest: [
        0x0d, 0x2f, 0x17, 0xf6, 0x3b, 0xfd, 0xe6, 0x5b, 0xfb, 0x41, 0xab, 0x8d, 0x0c, 0xd2, 0x38,
        0x51, 0xed, 0xd2, 0x3d, 0xee, 0xda, 0xda, 0x66, 0x11, 0x87, 0xab, 0x1f, 0x5b, 0xd3, 0xa0,
        0xbd, 0x56,
    ],
    receipt_capability_audit_digest: [
        0x3d, 0x79, 0xb9, 0x1f, 0x97, 0xc5, 0x4b, 0x71, 0xd1, 0xe9, 0x63, 0x88, 0xe1, 0xfa, 0x19,
        0x0d, 0x15, 0x26, 0x60, 0x60, 0x22, 0x20, 0xe6, 0xb7, 0xc6, 0x29, 0xaf, 0xe1, 0x6a, 0xf1,
        0xf1, 0xf1,
    ],
    receipt_capability_blocker_mask: 0xff,
    receipt_capability_release_available: false,
    release_kat_digest: [0; 32],
};

const RELEASE_MANIFEST_DIGEST_V1: [u8; 32] = [
    0xae, 0x19, 0xa0, 0x19, 0xf7, 0xee, 0x29, 0x30, 0x03, 0x5e, 0x41, 0xef, 0x00, 0x4a, 0x42, 0x29,
    0x6a, 0xb5, 0x4c, 0xe0, 0x4e, 0x5b, 0x62, 0xa4, 0xbb, 0xcf, 0x3c, 0x16, 0xf3, 0x5c, 0x2c, 0xfa,
];
const RELEASE_READINESS_DIGEST_V1: [u8; 32] = [
    0xfb, 0x4c, 0xd0, 0x7a, 0x98, 0xc0, 0xe2, 0x31, 0x85, 0xf1, 0xd4, 0x04, 0x9c, 0xd6, 0x67, 0x39,
    0x7d, 0x26, 0x78, 0x8f, 0xc5, 0x73, 0xe1, 0x0c, 0x5a, 0xc0, 0xc4, 0xfa, 0x3f, 0x7d, 0x20, 0x59,
];
const RELEASE_READINESS_V1: ZkAmsMkheReadinessV1 = ZkAmsMkheReadinessV1 {
    parameter_gate: true,
    security_gate: true,
    noise_gate: true,
    resource_gate: false,
    wire_gate: false,
    malicious_party_gate: false,
    decryption_share_gate: false,
    packing_gate: true,
    phase23_gate: false,
    receipt_capability_gate: false,
    receipt_capability_blocker_mask: 0xff,
    release_kat_gate: false,
};

/// Return the exact frozen first-release candidate manifest.
pub const fn zk_ams_mkhe_release_manifest_v1()
-> Result<ZkAmsMkheReleaseManifestV1, ZkAmsMkheErrorV1> {
    Ok(RELEASE_MANIFEST_V1)
}

/// Return the frozen consensus digest of the candidate manifest.
pub const fn zk_ams_mkhe_manifest_digest_v1() -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    Ok(RELEASE_MANIFEST_DIGEST_V1)
}

/// Return the exact fail-closed first-release readiness state.
pub const fn zk_ams_mkhe_readiness_v1() -> Result<ZkAmsMkheReadinessV1, ZkAmsMkheErrorV1> {
    Ok(RELEASE_READINESS_V1)
}

/// Return the frozen consensus digest of the candidate readiness state.
pub const fn zk_ams_mkhe_readiness_digest_v1() -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    Ok(RELEASE_READINESS_DIGEST_V1)
}

pub(in crate::vega::zk_ams) const fn require_release_ready_v1() -> Result<(), ZkAmsMkheErrorV1> {
    Err(ZkAmsMkheErrorV1::ReleaseUnavailable)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frozen_candidate_outputs_are_exact() {
        assert_eq!(zk_ams_mkhe_release_manifest_v1(), Ok(RELEASE_MANIFEST_V1));
        assert_eq!(
            zk_ams_mkhe_manifest_digest_v1(),
            Ok([
                0xae, 0x19, 0xa0, 0x19, 0xf7, 0xee, 0x29, 0x30, 0x03, 0x5e, 0x41, 0xef, 0x00, 0x4a,
                0x42, 0x29, 0x6a, 0xb5, 0x4c, 0xe0, 0x4e, 0x5b, 0x62, 0xa4, 0xbb, 0xcf, 0x3c, 0x16,
                0xf3, 0x5c, 0x2c, 0xfa
            ])
        );
        assert_eq!(zk_ams_mkhe_readiness_v1(), Ok(RELEASE_READINESS_V1));
        assert_eq!(
            zk_ams_mkhe_readiness_digest_v1(),
            Ok([
                0xfb, 0x4c, 0xd0, 0x7a, 0x98, 0xc0, 0xe2, 0x31, 0x85, 0xf1, 0xd4, 0x04, 0x9c, 0xd6,
                0x67, 0x39, 0x7d, 0x26, 0x78, 0x8f, 0xc5, 0x73, 0xe1, 0x0c, 0x5a, 0xc0, 0xc4, 0xfa,
                0x3f, 0x7d, 0x20, 0x59
            ])
        );
    }

    #[test]
    fn first_release_remains_deterministically_unavailable() {
        let readiness = zk_ams_mkhe_readiness_v1().expect("frozen readiness");
        assert!(!readiness.is_ready());
        assert_eq!(
            require_release_ready_v1(),
            Err(ZkAmsMkheErrorV1::ReleaseUnavailable)
        );
    }
}
