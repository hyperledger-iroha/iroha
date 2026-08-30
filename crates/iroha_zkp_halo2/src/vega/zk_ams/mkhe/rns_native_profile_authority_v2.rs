//! Versioned, non-authorizing resolver for the corrected 40-limb profile.
//!
//! The active ZK-AMS release manifest is still the legacy 38-limb V1 shape.
//! This module gives the corrected 40-limb candidate a distinct outer schema
//! and rejects requests for the legacy generation.  It deliberately installs
//! no estimator, KAT, resource-review, readiness, or runtime-routing authority.

use super::{
    ZkAmsMkheErrorV1,
    manifest::{release_profile_v1, zk_ams_mkhe_manifest_digest_v1},
    rns_native_profile::{
        ZK_AMS_MKHE_RNS_NATIVE_CENTERED_CAPACITY_BITS_V1, ZK_AMS_MKHE_RNS_NATIVE_HEADROOM_BITS_V1,
        ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1, ZK_AMS_MKHE_RNS_NATIVE_MODULUS_BITS_V1,
        ZK_AMS_MKHE_RNS_NATIVE_RESIDUAL_BITS_V1, zk_ams_mkhe_rns_native_profile_manifest_v1,
        zk_ams_mkhe_rns_native_profile_v1, zk_ams_mkhe_rns_native_release_candidate_digest_v1,
    },
};
use crate::vega::sponge::Keccak256;

const PROFILE_AUTHORITY_DOMAIN_V2: &[u8] =
    b"iroha.zk-ams.v2.mkhe.corrected-40-limb-profile-authority";

/// Outer resolver schema for the corrected profile candidate.
pub const ZK_AMS_MKHE_RNS_NATIVE_PROFILE_AUTHORITY_VERSION_V2: u8 = 2;

/// Closed profile generation accepted by the V2 resolver.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ZkAmsMkheRnsNativeProfileGenerationV2 {
    /// The frozen 38-limb V1 release profile; always rejected by this resolver.
    Legacy38LimbV1 = 1,
    /// The corrected, deliberately non-authorizing 40-limb candidate.
    Corrected40LimbV2 = 2,
}

/// Canonical identity of the corrected candidate under a V2 outer schema.
///
/// Every field is private and reconstructed during validation.  The retained
/// legacy identities are negative-domain separators: they make accidental V1
/// routing or profile substitution observable, not accepted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ZkAmsMkheRnsNativeProfileAuthorityV2 {
    version: u8,
    generation: ZkAmsMkheRnsNativeProfileGenerationV2,
    profile_digest: [u8; 32],
    topology_digest: [u8; 32],
    candidate_manifest_digest: [u8; 32],
    release_candidate_digest: [u8; 32],
    legacy_profile_digest: [u8; 32],
    legacy_manifest_digest: [u8; 32],
    authority_digest: [u8; 32],
    release_available: bool,
}

impl ZkAmsMkheRnsNativeProfileAuthorityV2 {
    /// Rebuild and validate every identity and the permanently closed gate.
    pub fn validate(self) -> Result<(), ZkAmsMkheErrorV1> {
        let expected = build_corrected_profile_authority_v2()?;
        if self != expected || self.release_available {
            return Err(ZkAmsMkheErrorV1::InvalidProfile);
        }
        Ok(())
    }

    /// Corrected 40-limb BGV profile digest.
    #[must_use]
    pub const fn profile_digest(self) -> [u8; 32] {
        self.profile_digest
    }

    /// Corrected composite-proof topology digest.
    #[must_use]
    pub const fn topology_digest(self) -> [u8; 32] {
        self.topology_digest
    }

    /// Domain-separated V2 authority identity.
    #[must_use]
    pub const fn authority_digest(self) -> [u8; 32] {
        self.authority_digest
    }

    /// Whether this phase-0 record can authorize release.
    ///
    /// This remains false until a later schema installs independently reviewed
    /// evidence and is explicitly connected to runtime governance.
    #[must_use]
    pub const fn release_available(self) -> bool {
        self.release_available
    }
}

/// Resolve only the corrected 40-limb generation under the V2 outer schema.
///
/// # Errors
///
/// Rejects the legacy V1 generation, any invalid corrected parameter, any
/// nonzero evidence pin in the phase-0 manifest, or an identity collision.
pub fn resolve_zk_ams_mkhe_rns_native_profile_authority_v2(
    generation: ZkAmsMkheRnsNativeProfileGenerationV2,
) -> Result<ZkAmsMkheRnsNativeProfileAuthorityV2, ZkAmsMkheErrorV1> {
    if generation != ZkAmsMkheRnsNativeProfileGenerationV2::Corrected40LimbV2 {
        return Err(ZkAmsMkheErrorV1::InvalidProfile);
    }
    let authority = build_corrected_profile_authority_v2()?;
    authority.validate()?;
    Ok(authority)
}

fn build_corrected_profile_authority_v2()
-> Result<ZkAmsMkheRnsNativeProfileAuthorityV2, ZkAmsMkheErrorV1> {
    let profile = zk_ams_mkhe_rns_native_profile_v1()?;
    let candidate_manifest = zk_ams_mkhe_rns_native_profile_manifest_v1()?;
    let release_candidate_digest = zk_ams_mkhe_rns_native_release_candidate_digest_v1()?;
    let legacy_profile_digest = release_profile_v1().digest()?;
    let legacy_manifest_digest = zk_ams_mkhe_manifest_digest_v1()?;
    if profile.rns_limb_count as usize != ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1
        || profile.ciphertext_modulus_bits != ZK_AMS_MKHE_RNS_NATIVE_MODULUS_BITS_V1
        || profile.residual_bits != ZK_AMS_MKHE_RNS_NATIVE_RESIDUAL_BITS_V1
        || profile.centered_capacity_bits != ZK_AMS_MKHE_RNS_NATIVE_CENTERED_CAPACITY_BITS_V1
        || profile.headroom_bits != ZK_AMS_MKHE_RNS_NATIVE_HEADROOM_BITS_V1
        || profile.evidence_complete()
        || candidate_manifest.authorizes_release()
        || candidate_manifest.security_certificate_digest != [0; 32]
        || candidate_manifest.release_kat_digest != [0; 32]
        || candidate_manifest.resource_review_digest != [0; 32]
        || legacy_profile_digest == profile.profile_digest
        || legacy_manifest_digest == candidate_manifest.manifest_digest
        || release_candidate_digest == legacy_manifest_digest
    {
        return Err(ZkAmsMkheErrorV1::InvalidProfile);
    }
    let mut authority = ZkAmsMkheRnsNativeProfileAuthorityV2 {
        version: ZK_AMS_MKHE_RNS_NATIVE_PROFILE_AUTHORITY_VERSION_V2,
        generation: ZkAmsMkheRnsNativeProfileGenerationV2::Corrected40LimbV2,
        profile_digest: profile.profile_digest,
        topology_digest: profile.proof_topology_digest,
        candidate_manifest_digest: candidate_manifest.manifest_digest,
        release_candidate_digest,
        legacy_profile_digest,
        legacy_manifest_digest,
        authority_digest: [0; 32],
        release_available: false,
    };
    authority.authority_digest = profile_authority_digest_v2(authority);
    if authority.authority_digest == [0; 32]
        || [
            authority.profile_digest,
            authority.topology_digest,
            authority.candidate_manifest_digest,
            authority.release_candidate_digest,
            authority.legacy_profile_digest,
            authority.legacy_manifest_digest,
        ]
        .contains(&authority.authority_digest)
    {
        return Err(ZkAmsMkheErrorV1::InvalidProfile);
    }
    Ok(authority)
}

fn profile_authority_digest_v2(authority: ZkAmsMkheRnsNativeProfileAuthorityV2) -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(PROFILE_AUTHORITY_DOMAIN_V2);
    hash.update(&[authority.version, authority.generation as u8]);
    hash.update(&authority.profile_digest);
    hash.update(&authority.topology_digest);
    hash.update(&authority.candidate_manifest_digest);
    hash.update(&authority.release_candidate_digest);
    hash.update(&authority.legacy_profile_digest);
    hash.update(&authority.legacy_manifest_digest);
    hash.update(&[authority.release_available.into()]);
    hash.finalize()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolver_rejects_legacy_without_mixing_v1_release_identity() {
        assert_eq!(
            resolve_zk_ams_mkhe_rns_native_profile_authority_v2(
                ZkAmsMkheRnsNativeProfileGenerationV2::Legacy38LimbV1,
            ),
            Err(ZkAmsMkheErrorV1::InvalidProfile)
        );
        let authority = resolve_zk_ams_mkhe_rns_native_profile_authority_v2(
            ZkAmsMkheRnsNativeProfileGenerationV2::Corrected40LimbV2,
        )
        .expect("corrected phase-0 authority");
        authority.validate().expect("canonical authority");
        let corrected = zk_ams_mkhe_rns_native_profile_v1().expect("corrected candidate");
        let legacy = release_profile_v1()
            .digest()
            .expect("legacy release digest");
        assert_eq!(corrected.rns_limb_count, 40);
        assert_eq!(authority.profile_digest(), corrected.profile_digest);
        assert_eq!(authority.topology_digest(), corrected.proof_topology_digest);
        assert_ne!(authority.profile_digest(), legacy);
        assert!(!authority.release_available());
        assert_ne!(authority.authority_digest(), [0; 32]);
    }
}
