//! Finalized KAGEMUSHA verifier release authority and lifecycle.
//!
//! Local recursive proving keys are operational files. This record retains the
//! independently governed identities against which a node checks those files.

use super::{
    KAGEMUSHA_WIRE_VERSION_V1, KagemushaAuthenticatedReleaseV1,
    KagemushaInternalValidationReceiptV1, KagemushaReleaseAttestationV1,
    KagemushaReleaseAuthorityPolicyV1, KagemushaReleaseManifestV1,
};
use crate::{DeriveJsonDeserialize, DeriveJsonSerialize};
use iroha_schema::IntoSchema;
use norito::{Decode, Encode, NoritoSchema};

/// Maximum releases retained by the first-release verifier authority cell.
pub const KAGEMUSHA_VERIFIER_RELEASE_LIMIT_V1: usize = 64;

/// Active release accepting new issuance.
pub const KAGEMUSHA_RELEASE_ACTIVE_V1: u8 = 1;
/// Authenticated release awaiting activation.
pub const KAGEMUSHA_RELEASE_STANDBY_V1: u8 = 2;
/// Former active release retained for already issued monetary objects.
pub const KAGEMUSHA_RELEASE_VERIFICATION_ONLY_V1: u8 = 3;

/// Exact path-free identity of one governed verifier release.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Decode,
    Encode,
    IntoSchema,
    NoritoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha:kagemusha:governed-verifier-release:v1")]
pub struct KagemushaGovernedVerifierReleaseV1 {
    /// Authenticated manifest-derived release identity.
    #[norito(json = "crate::json_helpers::fixed_bytes")]
    pub release_id: [u8; 32],
    /// One of active, standby, or verification-only.
    pub status: u8,
    /// Digest of the exact recursive profile.
    #[norito(json = "crate::json_helpers::fixed_bytes")]
    pub profile_digest: [u8; 32],
    /// Digest of the complete release artifact manifest.
    #[norito(json = "crate::json_helpers::fixed_bytes")]
    pub artifact_manifest_digest: [u8; 32],
    /// Digest of the internal qualification receipt.
    #[norito(json = "crate::json_helpers::fixed_bytes")]
    pub receipt_digest: [u8; 32],
    /// Digest of the threshold-signed release attestation.
    #[norito(json = "crate::json_helpers::fixed_bytes")]
    pub attestation_digest: [u8; 32],
    /// Digest of the governed threshold authority policy.
    #[norito(json = "crate::json_helpers::fixed_bytes")]
    pub authority_policy_digest: [u8; 32],
    /// Digest of the exact hardware profile policy.
    #[norito(json = "crate::json_helpers::fixed_bytes")]
    pub hardware_policy_digest: [u8; 32],
    /// Digest of the native proof layout.
    #[norito(json = "crate::json_helpers::fixed_bytes")]
    pub native_profile_digest: [u8; 32],
    /// Authenticated provider credential policy root.
    #[norito(json = "crate::json_helpers::fixed_bytes")]
    pub provider_policy_root: [u8; 32],
    /// Exact proof suite.
    #[norito(json = "crate::json_helpers::fixed_bytes")]
    pub suite_id: [u8; 32],
    /// Digest of the exact verifying-key set.
    #[norito(json = "crate::json_helpers::fixed_bytes")]
    pub vk_set_digest: [u8; 32],
}

impl KagemushaGovernedVerifierReleaseV1 {
    /// Project a fully threshold-authenticated release without retaining file paths.
    #[must_use]
    pub fn from_authenticated(release: &KagemushaAuthenticatedReleaseV1, status: u8) -> Self {
        let profile = &release.enabled_profiles()[0];
        Self {
            release_id: release.release_id(),
            status,
            profile_digest: release.profile_digest(),
            artifact_manifest_digest: release.manifest_digest(),
            receipt_digest: release.receipt_digest(),
            attestation_digest: release.attestation_digest(),
            authority_policy_digest: release.authority_policy_digest(),
            hardware_policy_digest: release.hardware_policy_digest(),
            native_profile_digest: release.native_profile_digest(),
            provider_policy_root: release.provider_policy_root(),
            suite_id: profile.suite_id,
            vk_set_digest: release.vk_set_digest(),
        }
    }
}

/// Finalized authority against which every local verifier is checked.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Decode,
    Encode,
    IntoSchema,
    NoritoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha:kagemusha:governed-verifier-registry:v1")]
pub struct KagemushaGovernedVerifierRegistryV1 {
    /// Sole first-release layout version.
    pub version: u16,
    /// Trusted signer threshold chosen by finalized governance.
    pub authority_policy: Option<KagemushaReleaseAuthorityPolicyV1>,
    /// Unique active release, if installed.
    pub active_release_id: Option<[u8; 32]>,
    /// Strictly sorted, unique release authorities.
    pub releases: Vec<KagemushaGovernedVerifierReleaseV1>,
}

impl Default for KagemushaGovernedVerifierRegistryV1 {
    fn default() -> Self {
        Self {
            version: KAGEMUSHA_WIRE_VERSION_V1,
            authority_policy: None,
            active_release_id: None,
            releases: Vec::new(),
        }
    }
}

impl KagemushaGovernedVerifierRegistryV1 {
    /// Install the initial finalized release signer policy exactly once.
    ///
    /// The caller must separately enforce governance authorization before this mutation.
    /// # Errors
    /// Returns an error for an invalid or already installed policy.
    pub fn initialize_authority_policy(
        &mut self,
        policy: KagemushaReleaseAuthorityPolicyV1,
    ) -> Result<(), &'static str> {
        self.validate()?;
        if self.authority_policy.is_some() {
            return Err("KAGEMUSHA verifier authority policy is already installed");
        }
        policy
            .validate()
            .map_err(|_| "invalid governed KAGEMUSHA signer policy")?;
        self.authority_policy = Some(policy);
        Ok(())
    }

    /// Authenticate and add one signed release as standby without opening issuance.
    ///
    /// The caller must separately enforce governance authorization before this mutation.
    /// # Errors
    /// Returns an error for an invalid release, duplicate, or exhausted registry.
    pub fn install_authenticated_release(
        &mut self,
        manifest: &KagemushaReleaseManifestV1,
        receipt: &KagemushaInternalValidationReceiptV1,
        attestation: &KagemushaReleaseAttestationV1,
    ) -> Result<(), &'static str> {
        self.validate()?;
        let policy = self
            .authority_policy
            .as_ref()
            .ok_or("KAGEMUSHA verifier has no governed signer policy")?;
        if self.releases.len() >= KAGEMUSHA_VERIFIER_RELEASE_LIMIT_V1 {
            return Err("KAGEMUSHA verifier release registry is full");
        }
        let authenticated = manifest
            .authenticate(receipt, policy, attestation)
            .map_err(|_| "KAGEMUSHA verifier release authentication failed")?;
        let release_id = authenticated.release_id();
        let position = match self
            .releases
            .binary_search_by_key(&release_id, |row| row.release_id)
        {
            Ok(_) => return Err("KAGEMUSHA verifier release is already installed"),
            Err(position) => position,
        };
        let row = KagemushaGovernedVerifierReleaseV1::from_authenticated(
            &authenticated,
            KAGEMUSHA_RELEASE_STANDBY_V1,
        );
        self.releases.insert(position, row);
        self.validate()
    }

    /// Activate an exact standby successor and retain the old verifier for historical objects.
    ///
    /// The caller must separately enforce governance authorization before this mutation.
    /// # Errors
    /// Returns an error for a stale active pointer or a non-standby successor.
    pub fn activate_standby(
        &mut self,
        expected_active_release_id: Option<[u8; 32]>,
        successor_release_id: [u8; 32],
    ) -> Result<(), &'static str> {
        self.validate()?;
        if self.active_release_id != expected_active_release_id {
            return Err("KAGEMUSHA active release changed before activation");
        }
        let next = self
            .releases
            .binary_search_by_key(&successor_release_id, |row| row.release_id)
            .map_err(|_| "KAGEMUSHA successor release is missing")?;
        if self.releases[next].status != KAGEMUSHA_RELEASE_STANDBY_V1 {
            return Err("KAGEMUSHA successor is not a standby release");
        }
        if let Some(previous) = expected_active_release_id {
            let old = self
                .releases
                .binary_search_by_key(&previous, |row| row.release_id)
                .map_err(|_| "KAGEMUSHA active release is missing")?;
            self.releases[old].status = KAGEMUSHA_RELEASE_VERIFICATION_ONLY_V1;
        }
        self.releases[next].status = KAGEMUSHA_RELEASE_ACTIVE_V1;
        self.active_release_id = Some(successor_release_id);
        self.validate()
    }

    /// Retire only an unused standby release; historically active releases are retained.
    ///
    /// The caller must separately enforce governance authorization before this mutation.
    /// # Errors
    /// Returns an error if the release is absent or has ever been active.
    pub fn retire_standby(&mut self, release_id: [u8; 32]) -> Result<(), &'static str> {
        self.validate()?;
        let position = self
            .releases
            .binary_search_by_key(&release_id, |row| row.release_id)
            .map_err(|_| "KAGEMUSHA verifier release is missing")?;
        if self.releases[position].status != KAGEMUSHA_RELEASE_STANDBY_V1 {
            return Err("KAGEMUSHA active or historical release cannot be retired");
        }
        self.releases.remove(position);
        self.validate()
    }

    /// Validate the complete current or predecessor authority snapshot.
    ///
    /// # Errors
    /// Returns an error for malformed status, signer policy, row identity, or active pointer.
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.version != KAGEMUSHA_WIRE_VERSION_V1
            || self.releases.len() > KAGEMUSHA_VERIFIER_RELEASE_LIMIT_V1
        {
            return Err("invalid KAGEMUSHA verifier registry version or size");
        }
        let Some(policy) = self.authority_policy.as_ref() else {
            return if self.releases.is_empty() && self.active_release_id.is_none() {
                Ok(())
            } else {
                Err("KAGEMUSHA verifier releases have no governed signer policy")
            };
        };
        policy
            .validate()
            .map_err(|_| "invalid governed KAGEMUSHA signer policy")?;
        let policy_digest = policy
            .canonical_digest()
            .map_err(|_| "cannot digest governed KAGEMUSHA signer policy")?;
        if self
            .releases
            .windows(2)
            .any(|pair| pair[0].release_id >= pair[1].release_id)
        {
            return Err("KAGEMUSHA verifier releases are not strictly ordered");
        }
        let mut active_count = 0;
        for row in &self.releases {
            if row.release_id == [0; 32]
                || row.authority_policy_digest != policy_digest
                || [
                    row.profile_digest,
                    row.artifact_manifest_digest,
                    row.receipt_digest,
                    row.attestation_digest,
                    row.hardware_policy_digest,
                    row.native_profile_digest,
                    row.provider_policy_root,
                    row.suite_id,
                    row.vk_set_digest,
                ]
                .contains(&[0; 32])
            {
                return Err("invalid KAGEMUSHA verifier release identity");
            }
            match row.status {
                KAGEMUSHA_RELEASE_ACTIVE_V1 => {
                    active_count += 1;
                    if self.active_release_id != Some(row.release_id) {
                        return Err("KAGEMUSHA active release pointer differs");
                    }
                }
                KAGEMUSHA_RELEASE_STANDBY_V1 | KAGEMUSHA_RELEASE_VERIFICATION_ONLY_V1 => {}
                _ => return Err("invalid KAGEMUSHA verifier release status"),
            }
        }
        if active_count != usize::from(self.active_release_id.is_some()) {
            return Err("KAGEMUSHA verifier registry requires exactly one active release");
        }
        if self.active_release_id.is_none()
            && self
                .releases
                .iter()
                .any(|row| row.status != KAGEMUSHA_RELEASE_STANDBY_V1)
        {
            return Err("KAGEMUSHA inactive verifier registry contains a non-standby release");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::{Algorithm, KeyPair};

    fn policy() -> KagemushaReleaseAuthorityPolicyV1 {
        let signer =
            KeyPair::try_from_seed(vec![7; 32], Algorithm::Ed25519).expect("deterministic signer");
        KagemushaReleaseAuthorityPolicyV1 {
            version: KAGEMUSHA_WIRE_VERSION_V1,
            authority_set_id: [1; 32],
            threshold: 1,
            authorized_signers: vec![signer.public_key().clone()],
        }
    }

    fn row(
        release_id: u8,
        status: u8,
        policy_digest: [u8; 32],
    ) -> KagemushaGovernedVerifierReleaseV1 {
        KagemushaGovernedVerifierReleaseV1 {
            release_id: [release_id; 32],
            status,
            profile_digest: [2; 32],
            artifact_manifest_digest: [3; 32],
            receipt_digest: [4; 32],
            attestation_digest: [5; 32],
            authority_policy_digest: policy_digest,
            hardware_policy_digest: [6; 32],
            native_profile_digest: [7; 32],
            provider_policy_root: [8; 32],
            suite_id: [9; 32],
            vk_set_digest: [10; 32],
        }
    }

    #[test]
    fn default_and_one_time_policy_are_canonical() {
        let mut registry = KagemushaGovernedVerifierRegistryV1::default();
        registry.validate().unwrap();
        registry.initialize_authority_policy(policy()).unwrap();
        assert!(registry.initialize_authority_policy(policy()).is_err());
        let bytes = norito::encode_canonical(&registry).unwrap();
        let decoded: KagemushaGovernedVerifierRegistryV1 =
            norito::decode_canonical(&bytes).unwrap();
        assert_eq!(decoded, registry);
    }

    #[test]
    fn activation_preserves_historical_release_and_retirement_is_standby_only() {
        let policy = policy();
        let digest = policy.canonical_digest().unwrap();
        let mut registry = KagemushaGovernedVerifierRegistryV1 {
            version: KAGEMUSHA_WIRE_VERSION_V1,
            authority_policy: Some(policy),
            active_release_id: Some([1; 32]),
            releases: vec![
                row(1, KAGEMUSHA_RELEASE_ACTIVE_V1, digest),
                row(2, KAGEMUSHA_RELEASE_STANDBY_V1, digest),
                row(3, KAGEMUSHA_RELEASE_STANDBY_V1, digest),
            ],
        };
        registry.validate().unwrap();
        assert!(registry.activate_standby(Some([3; 32]), [2; 32]).is_err());
        registry.activate_standby(Some([1; 32]), [2; 32]).unwrap();
        assert_eq!(
            registry.releases[0].status,
            KAGEMUSHA_RELEASE_VERIFICATION_ONLY_V1
        );
        assert_eq!(registry.active_release_id, Some([2; 32]));
        assert!(registry.retire_standby([1; 32]).is_err());
        registry.retire_standby([3; 32]).unwrap();
        assert_eq!(registry.releases.len(), 2);
        registry.validate().unwrap();
    }

    #[test]
    fn first_activation_requires_exact_standby_and_cannot_be_replayed() {
        let policy = policy();
        let digest = policy.canonical_digest().unwrap();
        let mut registry = KagemushaGovernedVerifierRegistryV1 {
            version: KAGEMUSHA_WIRE_VERSION_V1,
            authority_policy: Some(policy),
            active_release_id: None,
            releases: vec![row(1, KAGEMUSHA_RELEASE_STANDBY_V1, digest)],
        };
        assert!(registry.activate_standby(Some([2; 32]), [1; 32]).is_err());
        assert!(registry.activate_standby(None, [2; 32]).is_err());
        registry.activate_standby(None, [1; 32]).unwrap();
        assert_eq!(registry.active_release_id, Some([1; 32]));
        assert_eq!(registry.releases[0].status, KAGEMUSHA_RELEASE_ACTIVE_V1);
        assert!(registry.activate_standby(None, [1; 32]).is_err());
        assert!(registry.activate_standby(Some([1; 32]), [1; 32]).is_err());
    }

    #[test]
    fn malformed_policy_order_identity_and_status_are_rejected() {
        let policy = policy();
        let digest = policy.canonical_digest().unwrap();
        let mut registry = KagemushaGovernedVerifierRegistryV1 {
            version: KAGEMUSHA_WIRE_VERSION_V1,
            authority_policy: Some(policy),
            active_release_id: Some([1; 32]),
            releases: vec![row(1, KAGEMUSHA_RELEASE_ACTIVE_V1, digest)],
        };
        registry.validate().unwrap();
        registry.releases[0].status = 0;
        assert!(registry.validate().is_err());
        registry.releases[0].status = KAGEMUSHA_RELEASE_ACTIVE_V1;
        registry.releases[0].profile_digest = [0; 32];
        assert!(registry.validate().is_err());
        registry.releases[0].profile_digest = [2; 32];
        registry.releases.push(registry.releases[0].clone());
        assert!(registry.validate().is_err());
        registry.releases.pop();
        registry.active_release_id = Some([9; 32]);
        assert!(registry.validate().is_err());
    }
}
