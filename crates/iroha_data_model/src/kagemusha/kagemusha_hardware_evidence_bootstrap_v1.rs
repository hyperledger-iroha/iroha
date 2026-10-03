//! First-device Android evidence only. No account, FI, financial secret or money grant.
use super::{
    KagemushaAppKeySecurityLevelV1, KagemushaDevicePublicKeyV1, KagemushaPlayIntegrityPolicyV1,
    KagemushaReleaseAuthorityPolicyV1,
};
use iroha_crypto::{Algorithm, PublicKey, Signature};
use norito::{Decode, Encode, NoritoSchema};
use sha2::{Digest as _, Sha256};

/// Hard complete canonical archive bound for this hardware-only family.
pub const KAGEMUSHA_HARDWARE_BOOTSTRAP_MAX_ORIGINAL_V1: usize = 192 * 1024;
/// Hard reservation deadline span; no timer can create another operation.
pub const KAGEMUSHA_HARDWARE_BOOTSTRAP_MAX_ATTEMPT_MS_V1: u64 = 120_000;
/// Distinct hardware-only possession signing domain, including NUL.
pub const KAGEMUSHA_HARDWARE_EVIDENCE_POSSESSION_DOMAIN_V1: &[u8] =
    b"iroha:kagemusha:v1:hardware-evidence-possession\0";
/// Sole fixed E308 body width.
pub const KAGEMUSHA_HARDWARE_EVIDENCE_POSSESSION_BODY_BYTES_V1: usize = 308;

/// Public semantic compile input, independently admitted by the artifact producer and copied
/// into the measured JNI. It contains no JNI SHA or signed manifest, avoiding a self-hash cycle.
/// Decoding alone grants no Native authority. The shipping producer must retain this complete
/// original and its actual build recipe alongside the artifact signature.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, NoritoSchema, iroha_schema::IntoSchema)]
#[norito_schema(name = "iroha_data_model::kagemusha::HardwareEvidenceCompiledBindingOriginalV1")]
pub struct KagemushaHardwareEvidenceCompiledBindingOriginalV1 {
    /// Exactly one, first-device hardware evidence only.
    pub version: u16,
    /// Complete independently approved threshold release-authority policy original.
    pub authority_policy_original: Vec<u8>,
    /// Semantic source input, separately retained outside its compiled product.
    pub app_source_sha256: [u8; 32],
    /// Semantic source input, separately retained outside its compiled product.
    pub sdk_source_sha256: [u8; 32],
    /// Exact compiled Native ABI selected by the artifact producer.
    pub native_abi: u32,
}
impl KagemushaHardwareEvidenceCompiledBindingOriginalV1 {
    /// Require the sole complete canonical authority original and nonzero semantic bindings.
    /// This performs shape validation; it does not independently approve a build input.
    ///
    /// # Errors
    /// Rejects a version other than one, zero ABI or source bindings, or an empty, oversized,
    /// malformed, noncanonical or structurally invalid authority-policy original.
    pub fn validate(&self) -> Result<KagemushaReleaseAuthorityPolicyV1, String> {
        if self.version != 1
            || self.native_abi == 0
            || self.app_source_sha256 == [0; 32]
            || self.sdk_source_sha256 == [0; 32]
        {
            return Err("hardware compiled binding rejected".into());
        }
        KagemushaReleaseAuthorityPolicyV1::decode_canonical_exact(&self.authority_policy_original)
            .map_err(|_| "hardware compiled authority rejected".into())
    }
}

/// Independently approved artifact-phase selection. Its only purpose is hardware evidence.
/// Exact SDK/JNI/package measurements come from the trusted startup factory, never the UI.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, NoritoSchema, iroha_schema::IntoSchema)]
#[norito_schema(name = "iroha_data_model::kagemusha::HardwareEvidenceJniArtifactV1")]
pub struct KagemushaHardwareEvidenceJniArtifactV1 {
    /// Exact Android packaging architecture, selected by the compiled Native target.
    pub android_abi: String,
    /// SHA256 of the complete genuine JNI artifact for this architecture.
    pub sha256: [u8; 32],
}

/// Complete signed hardware-only release, shared by the application's ABI/config splits.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, NoritoSchema, iroha_schema::IntoSchema)]
#[norito_schema(name = "iroha_data_model::kagemusha::HardwareEvidenceBootstrapManifestV1")]
/// Public complete hardware-evidence data original; decoding it grants no Native owner or money authority.
pub struct KagemushaHardwareEvidenceBootstrapManifestV1 {
    /// Sole first-release format version.
    pub version: u16,
    /// Must equal 1 (first-device evidence only); no money purpose exists in this family.
    pub purpose: u8,
    /// Exact independently selected threshold authority-policy identity.
    pub authority_policy_digest: [u8; 32],
    /// Genesis-derived network identity; selected by the authenticated release.
    pub network_id: [u8; 32],
    /// Exact Android package identity authenticated by the release and issuer.
    pub app_package: String,
    /// Exact Android version code checked against package metadata, raw attestation and PI originals.
    pub app_version_code: u64,
    /// Exact issuer origin independently selected by the release; no route fallback.
    pub core_origin: String,
    /// SHA256 of the exact Android application signing certificate; no aggregate/OEM digest mapping.
    pub app_signing_identity_digest: [u8; 32],
    /// Governed app distribution policy identity, separate from measured code.
    pub app_distribution_digest: [u8; 32],
    /// Independently measured application source binding.
    pub app_source_sha256: [u8; 32],
    /// Exact ordered application DEX code digest measured from the installed APK; excludes
    /// the external signed manifest and APK signing block, preventing a self-hash cycle.
    pub app_code_sha256: [u8; 32],
    /// Independently measured SDK source binding.
    pub sdk_source_sha256: [u8; 32],
    /// Strictly ordered unique actual JNI artifact originals. Native chooses its own compiled
    /// architecture; the base AAB asset never accepts a managed architecture or digest selector.
    pub jni_artifacts: Vec<KagemushaHardwareEvidenceJniArtifactV1>,
    /// Exact supported Native ABI binding; does not grant monetary operations.
    pub native_abi: u32,
    /// Ed25519 issuer independently authorized for this evidence-only release.
    pub evidence_issuer: PublicKey,
    /// Identity of the complete independently installed chain/root/revocation/app verifier policy.
    pub raw_verifier_policy_digest: [u8; 32],
    /// Core verifies the original Google ID token against this exact issuer/audience.
    pub google_oauth_issuer: String,
    /// Exact independently admitted Google OAuth audience.
    pub google_oauth_client_id: String,
    /// Project selected by the independently approved Play Integrity policy.
    pub google_cloud_project_number: u64,
    /// Mandatory separately verified Play Integrity requirements; no client verdict is accepted.
    pub play_integrity_policy: KagemushaPlayIntegrityPolicyV1,
    /// Sorted permitted TEE/StrongBox levels; no software key class exists.
    pub allowed_android_security_levels: Vec<KagemushaAppKeySecurityLevelV1>,
    /// Exact independently installed signed Native clock selection.
    pub native_clock_selection_digest: [u8; 32],
    /// Exact four HTTPS validator base URLs in the same independently signed node order.
    /// A managed endpoint or offered clock reply cannot replace these transport selections.
    pub native_clock_base_urls: [String; 4],
    /// Nonzero approved bootstrap policy epoch.
    pub policy_epoch: u64,
    /// Lower admitted release validity bound, checked under actual Native time.
    pub not_before_ms: u64,
    /// Exclusive signed upper validity bound.
    pub expires_at_ms: u64,
    /// Maximum original reservation lifetime, at most 120 seconds.
    pub maximum_attempt_lifetime_ms: u64,
}
impl KagemushaHardwareEvidenceBootstrapManifestV1 {
    /// Reject incomplete, relabeled or substituted evidence-purpose fields.
    ///
    /// # Errors
    /// Rejects invalid purpose, identities, key, policy, origins, selectors or validity bounds.
    pub fn validate(&self) -> Result<(), String> {
        if self.version != 1
            || self.purpose != 1
            || self.native_abi == 0
            || self.app_version_code == 0
            || self.jni_artifacts.is_empty()
            || self.jni_artifacts.len() > 4
            || self.jni_artifacts.iter().any(|a| {
                a.sha256 == [0; 32]
                    || !matches!(
                        a.android_abi.as_str(),
                        "arm64-v8a" | "armeabi-v7a" | "x86" | "x86_64"
                    )
            })
            || !self
                .jni_artifacts
                .windows(2)
                .all(|a| a[0].android_abi < a[1].android_abi)
            || self.evidence_issuer.algorithm() != Algorithm::Ed25519
            || self.policy_epoch == 0
            || self.not_before_ms == 0
            || self.expires_at_ms <= self.not_before_ms
            || !(1..=KAGEMUSHA_HARDWARE_BOOTSTRAP_MAX_ATTEMPT_MS_V1)
                .contains(&self.maximum_attempt_lifetime_ms)
            || self.google_cloud_project_number == 0
            || self.google_cloud_project_number > i64::MAX as u64
            || !matches!(
                self.google_oauth_issuer.as_str(),
                "https://accounts.google.com" | "accounts.google.com"
            )
            || self.google_oauth_client_id.is_empty()
            || self.google_oauth_client_id.len() > 512
            || !self
                .google_oauth_client_id
                .bytes()
                .all(|b| b.is_ascii_graphic())
            || !valid_origin(&self.core_origin)
            || self
                .native_clock_base_urls
                .iter()
                .any(|base| !valid_clock_base_url(base))
            || self.app_package.is_empty()
            || self.app_package.len() > 128
            || !self.app_package.split('.').all(|part| {
                !part.is_empty() && part.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
            })
            || !self.app_package.contains('.')
            || self.allowed_android_security_levels.is_empty()
            || self.allowed_android_security_levels.len() > 2
            || self
                .allowed_android_security_levels
                .contains(&KagemushaAppKeySecurityLevelV1::AppleAppAttest)
            || !self
                .allowed_android_security_levels
                .windows(2)
                .all(|p| p[0] < p[1])
        {
            return Err("hardware bootstrap manifest rejected".into());
        }
        for d in [
            self.authority_policy_digest,
            self.network_id,
            self.app_signing_identity_digest,
            self.app_distribution_digest,
            self.app_source_sha256,
            self.app_code_sha256,
            self.sdk_source_sha256,
            self.raw_verifier_policy_digest,
            self.native_clock_selection_digest,
            self.play_integrity_policy.policy_digest,
        ] {
            nonzero(d)?;
        }
        let p = self.play_integrity_policy;
        if p.maximum_evidence_age_ms == 0
            || p.maximum_refresh_interval_ms == 0
            || !p.require_play_recognized
            || !p.require_licensed
            || !matches!(p.minimum_device_integrity, 1 | 2)
        {
            return Err("hardware bootstrap Integrity policy rejected".into());
        }
        Ok(())
    }
    /// Return the sole complete canonical purpose-bound original identity.
    ///
    /// # Errors
    /// Rejects a malformed manifest or an unencodable or oversized canonical original.
    pub fn digest(&self) -> Result<[u8; 32], String> {
        self.validate()?;
        digest(b"iroha:kagemusha:v1:hardware-bootstrap-manifest\0", self)
    }
    /// Return the sole complete purpose-specific issuer/platform signing message.
    ///
    /// # Errors
    /// Rejects a malformed manifest or an unencodable or oversized canonical original.
    pub fn signing_bytes(&self) -> Result<Vec<u8>, String> {
        self.validate()?;
        message(b"iroha:kagemusha:v1:hardware-bootstrap-release\0", self)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, NoritoSchema, iroha_schema::IntoSchema)]
#[norito_schema(name = "iroha_data_model::kagemusha::HardwareBootstrapReleaseApprovalV1")]
/// Public complete hardware-evidence data original; decoding it grants no Native owner or money authority.
pub struct KagemushaHardwareBootstrapReleaseApprovalV1 {
    /// Exact approved release authority signer.
    pub public_key: PublicKey,
    /// Unmodified signature over the sole purpose-specific message.
    pub signature: Signature,
}
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, NoritoSchema, iroha_schema::IntoSchema)]
#[norito_schema(name = "iroha_data_model::kagemusha::SignedHardwareBootstrapReleaseV1")]
/// Public complete hardware-evidence data original; decoding it grants no Native owner or money authority.
pub struct KagemushaSignedHardwareBootstrapReleaseV1 {
    /// Complete independently signed hardware-only release selection.
    pub manifest: KagemushaHardwareEvidenceBootstrapManifestV1,
    /// Sorted distinct authorized threshold approvals over the complete manifest.
    pub approvals: Vec<KagemushaHardwareBootstrapReleaseApprovalV1>,
}
impl KagemushaSignedHardwareBootstrapReleaseV1 {
    /// Authentication under independently selected release authorities, not installation.
    ///
    /// # Errors
    /// Rejects invalid authority or manifest data, policy substitution, threshold, signer or signature,
    /// or a canonical encoding or complete-frame bound failure.
    pub fn authenticate(
        &self,
        authority: &KagemushaReleaseAuthorityPolicyV1,
    ) -> Result<(), String> {
        authority
            .validate()
            .map_err(|_| "hardware bootstrap authority rejected")?;
        if self.manifest.authority_policy_digest
            != authority
                .canonical_digest()
                .map_err(|_| "authority digest rejected")?
            || self.approvals.len() > authority.authorized_signers.len()
            || self.approvals.len() < usize::from(authority.threshold)
            || !self
                .approvals
                .windows(2)
                .all(|p| p[0].public_key < p[1].public_key)
        {
            return Err("hardware bootstrap threshold rejected".into());
        }
        let m = self.manifest.signing_bytes()?;
        for a in &self.approvals {
            if a.public_key.algorithm() != Algorithm::Ed25519
                || authority
                    .authorized_signers
                    .binary_search(&a.public_key)
                    .is_err()
            {
                return Err("hardware bootstrap signer rejected".into());
            }
            a.signature
                .verify(&a.public_key, &m)
                .map_err(|_| "hardware bootstrap signature rejected")?;
        }
        Ok(())
    }
}

/// Public exact reservation, fsynced before the protected prepare invocation.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, NoritoSchema, iroha_schema::IntoSchema)]
#[norito_schema(name = "iroha_data_model::kagemusha::HardwareEvidenceReservationV1")]
/// Public complete hardware-evidence data original; decoding it grants no Native owner or money authority.
pub struct KagemushaHardwareEvidenceReservationV1 {
    /// Sole first-release format version.
    pub version: u16,
    /// Exact complete authenticated bootstrap manifest identity.
    pub manifest_digest: [u8; 32],
    /// Original durable Native operation identity; never a caller-selected replacement.
    pub operation_id: [u8; 32],
    /// Fresh Native nonce fsynced before any platform/network effect.
    pub client_nonce: [u8; 32],
    /// Sole original app-owned hardware key alias derived from manifest/operation/nonce.
    pub alias: String,
    /// Original Native/issuer issue time under the admitted bounded interval.
    pub issued_at_ms: u64,
    /// Authoritative Native reservation deadline; a local timer cannot change ownership.
    pub deadline_ms: u64,
}
impl KagemushaHardwareEvidenceReservationV1 {
    /// Derive only the exact original persistent alias; this data helper grants no key authority.
    pub fn alias_for(manifest: [u8; 32], operation: [u8; 32], nonce: [u8; 32]) -> String {
        let mut h = Sha256::new();
        h.update(b"iroha:kagemusha:v1:hardware-bootstrap-alias\0");
        h.update(manifest);
        h.update(operation);
        h.update(nonce);
        format!("kagemusha-hardware-v1-{}", hex::encode(h.finalize()))
    }
    /// Reject incomplete, relabeled or substituted evidence-purpose fields.
    ///
    /// # Errors
    /// Rejects a version other than one, zero selectors, an operation ID equal to the client nonce,
    /// an invalid interval or a substituted persistent alias.
    pub fn validate(&self) -> Result<(), String> {
        for d in [self.manifest_digest, self.operation_id, self.client_nonce] {
            nonzero(d)?;
        }
        if self.version != 1
            || self.operation_id == self.client_nonce
            || self.issued_at_ms == 0
            || self.deadline_ms <= self.issued_at_ms
            || self.deadline_ms - self.issued_at_ms > KAGEMUSHA_HARDWARE_BOOTSTRAP_MAX_ATTEMPT_MS_V1
            || self.alias
                != Self::alias_for(self.manifest_digest, self.operation_id, self.client_nonce)
        {
            return Err("hardware reservation rejected".into());
        }
        Ok(())
    }
    /// Return the sole complete canonical purpose-bound original identity.
    ///
    /// # Errors
    /// Rejects malformed reservation data or an unencodable or oversized canonical original.
    pub fn digest(&self) -> Result<[u8; 32], String> {
        self.validate()?;
        digest(b"iroha:kagemusha:v1:hardware-bootstrap-reservation\0", self)
    }
}

/// Core supplies a fresh C only after independent original Google OAuth verification.
/// This C contains no enrolled account, financial commitment, epoch or FI authority.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, NoritoSchema, iroha_schema::IntoSchema)]
#[norito_schema(name = "iroha_data_model::kagemusha::HardwareEvidenceChallengeV1")]
/// Public complete hardware-evidence data original; decoding it grants no Native owner or money authority.
pub struct KagemushaHardwareEvidenceChallengeV1 {
    /// Exact canonical original reservation identity.
    pub reservation_digest: [u8; 32],
    /// Exact complete authenticated bootstrap manifest identity.
    pub manifest_digest: [u8; 32],
    /// Original durable Native operation identity; never a caller-selected replacement.
    pub operation_id: [u8; 32],
    /// Fresh Native nonce fsynced before any platform/network effect.
    pub client_nonce: [u8; 32],
    /// Fresh independently signed Core nonce, distinct from the client nonce.
    pub server_nonce: [u8; 32],
    /// SHA256 of the exact retained Native alias UTF-8 bytes.
    pub alias_digest: [u8; 32],
    /// Purpose-bound verified Google issuer/audience/subject identity, selected only by Core.
    pub google_owner_binding: [u8; 32],
    /// SHA256 of the complete original Google ID token verified independently by Core.
    pub google_id_token_original_sha256: [u8; 32],
    /// Original Native/issuer issue time under the admitted bounded interval.
    pub issued_at_ms: u64,
    /// Exclusive signed upper validity bound.
    pub expires_at_ms: u64,
}
impl KagemushaHardwareEvidenceChallengeV1 {
    /// Return the sole complete purpose-specific issuer/platform signing message.
    ///
    /// # Errors
    /// Rejects zero selectors, equal client/server nonces, invalid validity bounds,
    /// or a canonical encoding or complete-frame bound failure.
    pub fn signing_bytes(&self) -> Result<Vec<u8>, String> {
        for d in [
            self.reservation_digest,
            self.manifest_digest,
            self.operation_id,
            self.client_nonce,
            self.server_nonce,
            self.alias_digest,
            self.google_owner_binding,
            self.google_id_token_original_sha256,
        ] {
            nonzero(d)?;
        }
        if self.client_nonce == self.server_nonce
            || self.issued_at_ms == 0
            || self.expires_at_ms <= self.issued_at_ms
            || self.expires_at_ms - self.issued_at_ms
                > KAGEMUSHA_HARDWARE_BOOTSTRAP_MAX_ATTEMPT_MS_V1
        {
            return Err("hardware C rejected".into());
        }
        message(b"iroha:kagemusha:v1:hardware-evidence-challenge\0", self)
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, NoritoSchema, iroha_schema::IntoSchema)]
#[norito_schema(name = "iroha_data_model::kagemusha::SignedHardwareEvidenceChallengeV1")]
/// Public complete hardware-evidence data original; decoding it grants no Native owner or money authority.
pub struct KagemushaSignedHardwareEvidenceChallengeV1 {
    /// Complete evidence-only signed challenge; financial C21 is excluded.
    pub challenge: KagemushaHardwareEvidenceChallengeV1,
    /// Unmodified signature over the sole purpose-specific message.
    pub signature: Signature,
}

impl KagemushaSignedHardwareEvidenceChallengeV1 {
    /// Sole attestation nonce and C-original join: SHA256 of the complete canonical signed C.
    /// No second digest of unsigned C or alternate encoder is accepted for this purpose.
    ///
    /// # Errors
    /// Rejects malformed challenge data or an unencodable or oversized canonical signed original.
    pub fn original_digest(&self) -> Result<[u8; 32], String> {
        self.challenge.signing_bytes()?;
        Ok(Sha256::digest(hardware_bootstrap_encode_v1(self)?).into())
    }
}

/// Independent raw verifier assertion. No client security-level or Google verdict is authoritative.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, NoritoSchema, iroha_schema::IntoSchema,
)]
#[norito_schema(name = "iroha_data_model::kagemusha::HardwareEvidenceRawAdmissionV1")]
/// Public complete hardware-evidence data original; decoding it grants no Native owner or money authority.
pub struct KagemushaHardwareEvidenceRawAdmissionV1 {
    /// SHA256 of the full canonical signed challenge original.
    pub challenge_digest: [u8; 32],
    /// SHA256 of the complete canonical platform attestation archive.
    pub raw_original_sha256: [u8; 32],
    /// SHA256 of the same canonical P256 SEC1 public point.
    pub attested_key_id: [u8; 32],
    /// Same app-owned attested P256 public point; no private key is carried.
    pub app_public_key: KagemushaDevicePublicKeyV1,
    /// Independent issuer verdict restricted to permitted TEE/StrongBox.
    pub security_level: KagemushaAppKeySecurityLevelV1,
    /// Identity of the complete independently installed chain/root/revocation/app verifier policy.
    pub raw_verifier_policy_digest: [u8; 32],
    /// Original independent raw-verifier check time, bounded by C and Native time.
    pub checked_at_ms: u64,
}
impl KagemushaHardwareEvidenceRawAdmissionV1 {
    /// Return the sole complete purpose-specific issuer/platform signing message.
    ///
    /// # Errors
    /// Rejects zero selectors, an invalid public key, zero check time, an Apple security level,
    /// or a canonical encoding or complete-frame bound failure.
    pub fn signing_bytes(&self) -> Result<Vec<u8>, String> {
        for d in [
            self.challenge_digest,
            self.raw_original_sha256,
            self.attested_key_id,
            self.raw_verifier_policy_digest,
        ] {
            nonzero(d)?;
        }
        self.app_public_key
            .validate()
            .map_err(|_| "hardware raw key rejected")?;
        if self.checked_at_ms == 0
            || self.security_level == KagemushaAppKeySecurityLevelV1::AppleAppAttest
        {
            return Err("hardware raw admission rejected".into());
        }
        message(
            b"iroha:kagemusha:v1:hardware-evidence-raw-admission\0",
            self,
        )
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, NoritoSchema, iroha_schema::IntoSchema)]
#[norito_schema(name = "iroha_data_model::kagemusha::SignedHardwareEvidenceRawAdmissionV1")]
/// Public complete hardware-evidence data original; decoding it grants no Native owner or money authority.
pub struct KagemushaSignedHardwareEvidenceRawAdmissionV1 {
    /// Complete independent raw-verifier assertion under the selected issuer.
    pub admission: KagemushaHardwareEvidenceRawAdmissionV1,
    /// Unmodified signature over the sole purpose-specific message.
    pub signature: Signature,
}

/// Fixed E308 is a distinct private signing purpose. Decoding E is data-only.
#[derive(Clone, Debug, PartialEq, Eq)]
/// Public complete hardware-evidence data original; decoding it grants no Native owner or money authority.
pub struct KagemushaHardwareEvidencePossessionV1 {
    /// Exact complete authenticated bootstrap manifest identity.
    pub manifest_digest: [u8; 32],
    /// Original durable Native operation identity; never a caller-selected replacement.
    pub operation_id: [u8; 32],
    /// SHA256 of the full canonical signed challenge original.
    pub challenge_digest: [u8; 32],
    /// SHA256 of the exact retained Native alias UTF-8 bytes.
    pub alias_digest: [u8; 32],
    /// SHA256 of the complete canonical platform attestation archive.
    pub raw_original_sha256: [u8; 32],
    /// SHA256 of the same canonical P256 SEC1 public point.
    pub attested_key_id: [u8; 32],
    /// Purpose-bound verified Google issuer/audience/subject identity, selected only by Core.
    pub google_owner_binding: [u8; 32],
    /// Same app-owned attested P256 public point; no private key is carried.
    pub app_public_key: [u8; 65],
    /// Original Native/issuer issue time under the admitted bounded interval.
    pub issued_at_ms: u64,
    /// Exclusive signed upper validity bound.
    pub expires_at_ms: u64,
}
impl KagemushaHardwareEvidencePossessionV1 {
    /// Return the sole complete purpose-specific issuer/platform signing message.
    ///
    /// # Errors
    /// Rejects an invalid public point, selector, key identity, interval or fixed possession-body width.
    pub fn signing_bytes(&self) -> Result<Vec<u8>, String> {
        KagemushaDevicePublicKeyV1::from_sec1_bytes(&self.app_public_key)
            .map_err(|_| "hardware E point rejected")?;
        let mut body = vec![1, 0, 1];
        for d in [
            self.manifest_digest,
            self.operation_id,
            self.challenge_digest,
            self.alias_digest,
            self.raw_original_sha256,
            self.attested_key_id,
            self.google_owner_binding,
        ] {
            nonzero(d)?;
            body.extend(d);
        }
        if self.app_public_key[0] != 4
            || self.attested_key_id != <[u8; 32]>::from(Sha256::digest(self.app_public_key))
            || self.issued_at_ms == 0
            || self.expires_at_ms <= self.issued_at_ms
            || self.expires_at_ms - self.issued_at_ms
                > KAGEMUSHA_HARDWARE_BOOTSTRAP_MAX_ATTEMPT_MS_V1
        {
            return Err("hardware E rejected".into());
        }
        body.extend(self.app_public_key);
        body.extend(self.issued_at_ms.to_le_bytes());
        body.extend(self.expires_at_ms.to_le_bytes());
        if body.len() != KAGEMUSHA_HARDWARE_EVIDENCE_POSSESSION_BODY_BYTES_V1 {
            return Err("hardware E width rejected".into());
        }
        let mut out = KAGEMUSHA_HARDWARE_EVIDENCE_POSSESSION_DOMAIN_V1.to_vec();
        out.extend((body.len() as u64).to_le_bytes());
        out.extend(body);
        Ok(out)
    }
}

/// Terminal hardware-only evidence receipt. It does not create an ordinary wallet credential.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, NoritoSchema, iroha_schema::IntoSchema)]
#[norito_schema(name = "iroha_data_model::kagemusha::HardwareEvidenceReceiptV1")]
/// Public complete hardware-evidence data original; decoding it grants no Native owner or money authority.
pub struct KagemushaHardwareEvidenceReceiptV1 {
    /// Exact complete authenticated bootstrap manifest identity.
    pub manifest_digest: [u8; 32],
    /// Exact canonical original reservation identity.
    pub reservation_digest: [u8; 32],
    /// SHA256 of the full canonical signed challenge original.
    pub challenge_digest: [u8; 32],
    /// SHA256 of the complete canonical platform attestation archive.
    pub raw_original_sha256: [u8; 32],
    /// SHA256 of the full canonical signed raw-admission original.
    pub raw_admission_original_sha256: [u8; 32],
    /// SHA256 of exact Native-selected E308 signing bytes.
    pub possession_message_sha256: [u8; 32],
    /// SHA256 of the unmodified original P256 possession DER.
    pub possession_der_sha256: [u8; 32],
    /// SHA256 of the entire separately verified opaque original Google Integrity token.
    pub integrity_token_original_sha256: [u8; 32],
    /// Sole C/raw/key/E-bound original Google request hash.
    pub integrity_request_hash: [u8; 32],
    /// Exact independently selected complete Play Integrity verifier policy identity.
    pub integrity_policy_digest: [u8; 32],
    /// Purpose-bound verified Google issuer/audience/subject identity, selected only by Core.
    pub google_owner_binding: [u8; 32],
    /// Original independent receipt verification time, bounded by C and Native time.
    pub verified_at_ms: u64,
    /// Exclusive signed upper validity bound.
    pub expires_at_ms: u64,
}
impl KagemushaHardwareEvidenceReceiptV1 {
    /// Return the sole complete purpose-specific issuer/platform signing message.
    ///
    /// # Errors
    /// Rejects zero selectors, an invalid receipt interval,
    /// or a canonical encoding or complete-frame bound failure.
    pub fn signing_bytes(&self) -> Result<Vec<u8>, String> {
        for d in [
            self.manifest_digest,
            self.reservation_digest,
            self.challenge_digest,
            self.raw_original_sha256,
            self.raw_admission_original_sha256,
            self.possession_message_sha256,
            self.possession_der_sha256,
            self.integrity_token_original_sha256,
            self.integrity_request_hash,
            self.integrity_policy_digest,
            self.google_owner_binding,
        ] {
            nonzero(d)?;
        }
        if self.verified_at_ms == 0 || self.expires_at_ms <= self.verified_at_ms {
            return Err("hardware receipt interval rejected".into());
        }
        message(b"iroha:kagemusha:v1:hardware-evidence-receipt\0", self)
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, NoritoSchema, iroha_schema::IntoSchema)]
#[norito_schema(name = "iroha_data_model::kagemusha::SignedHardwareEvidenceReceiptV1")]
/// Public complete hardware-evidence data original; decoding it grants no Native owner or money authority.
pub struct KagemushaSignedHardwareEvidenceReceiptV1 {
    /// Complete terminal hardware evidence original; it grants no wallet credential or funds.
    pub receipt: KagemushaHardwareEvidenceReceiptV1,
    /// Unmodified signature over the sole purpose-specific message.
    pub signature: Signature,
}
/// Sole PI request hash, joined to the complete signed C, key, raw and E originals.
pub fn kagemusha_hardware_evidence_integrity_request_hash_v1(
    c: &[u8],
    raw: &[u8],
    admission: &[u8],
    e: &[u8],
    der: &[u8],
) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(b"iroha:kagemusha:v1:hardware-evidence-play-integrity\0");
    for original in [c, raw, admission, e, der] {
        h.update((original.len() as u64).to_le_bytes());
        h.update(original);
    }
    h.finalize().into()
}
/// Encode or project the complete bounded hardware evidence original; this operation grants no authority.
///
/// # Errors
/// Rejects canonical encoding failure and an empty or oversized complete frame.
pub fn hardware_bootstrap_encode_v1<T: norito::NoritoSerialize>(v: &T) -> Result<Vec<u8>, String> {
    let b = norito::encode_canonical(v).map_err(|_| "hardware original encode rejected")?;
    if b.is_empty() || b.len() > KAGEMUSHA_HARDWARE_BOOTSTRAP_MAX_ORIGINAL_V1 {
        return Err("hardware original bound rejected".into());
    }
    Ok(b)
}
/// Encode or project the complete bounded hardware evidence original; this operation grants no authority.
///
/// # Errors
/// Rejects an empty, oversized, malformed or noncanonical complete frame,
/// or a canonical re-encoding or complete-frame bound failure.
pub fn hardware_bootstrap_decode_v1<
    T: for<'de> norito::NoritoDeserialize<'de> + norito::NoritoSerialize,
>(
    b: &[u8],
) -> Result<T, String> {
    if b.is_empty() || b.len() > KAGEMUSHA_HARDWARE_BOOTSTRAP_MAX_ORIGINAL_V1 {
        return Err("hardware original bound rejected".into());
    }
    let v = norito::decode_canonical_with_limits(b, norito::canonical_decode_limits(b.len()))
        .map_err(|_| "hardware original decode rejected")?;
    if hardware_bootstrap_encode_v1(&v)? != b {
        return Err("hardware original canonical shape rejected".into());
    }
    Ok(v)
}
fn nonzero(d: [u8; 32]) -> Result<(), String> {
    if d == [0; 32] {
        Err("hardware selector absent".into())
    } else {
        Ok(())
    }
}
fn message<T: norito::NoritoSerialize>(domain: &[u8], v: &T) -> Result<Vec<u8>, String> {
    let b = hardware_bootstrap_encode_v1(v)?;
    let mut out = domain.to_vec();
    out.extend((b.len() as u64).to_le_bytes());
    out.extend(b);
    Ok(out)
}
fn digest<T: norito::NoritoSerialize>(domain: &[u8], v: &T) -> Result<[u8; 32], String> {
    Ok(Sha256::digest(message(domain, v)?).into())
}

/// Core calls this only after real Google original verification and exact configured issuer/audience.
/// Public deterministic naming alone is not login/session/Native authority.
///
/// # Errors
/// Rejects an inert manifest digest or empty, oversized or control-bearing identity strings.
pub fn kagemusha_hardware_evidence_google_owner_binding_v1(
    manifest_digest: [u8; 32],
    verified_issuer: &str,
    verified_audience: &str,
    verified_sub: &str,
) -> Result<[u8; 32], String> {
    nonzero(manifest_digest)?;
    let mut h = Sha256::new();
    h.update(b"iroha:kagemusha:v1:hardware-evidence-google-owner\0");
    h.update(manifest_digest);
    for value in [verified_issuer, verified_audience, verified_sub] {
        if value.is_empty() || value.len() > 512 || value.chars().any(char::is_control) {
            return Err("hardware Google binding rejected".into());
        }
        h.update((value.len() as u64).to_le_bytes());
        h.update(value.as_bytes());
    }
    Ok(h.finalize().into())
}

fn valid_clock_base_url(value: &str) -> bool {
    let Some(remainder) = value.strip_prefix("https://") else {
        return false;
    };
    let Some((authority, path)) = remainder.split_once('/') else {
        return false;
    };
    value.len() <= 2048
        && valid_origin(&format!("https://{authority}"))
        && (path.is_empty()
            || (path.ends_with('/')
                && path[..path.len() - 1].split('/').all(|part| {
                    !part.is_empty()
                        && part != "."
                        && part != ".."
                        && part
                            .bytes()
                            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
                })))
}
fn valid_origin(origin: &str) -> bool {
    let Some(authority) = origin.strip_prefix("https://") else {
        return false;
    };
    let (host, port) = match authority.split_once(':') {
        Some((h, p)) => (h, Some(p)),
        None => (authority, None),
    };
    if host.len() > 253
        || !host.contains('.')
        || !host.split('.').all(|part| {
            !part.is_empty()
                && part.len() <= 63
                && !part.starts_with('-')
                && !part.ends_with('-')
                && part
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        })
    {
        return false;
    }
    port.is_none_or(|p| {
        !p.is_empty()
            && p.bytes().all(|b| b.is_ascii_digit())
            && p.parse::<u16>().is_ok_and(|n| n != 0)
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::{Algorithm, KeyPair};
    fn fixture() -> (
        KagemushaReleaseAuthorityPolicyV1,
        KagemushaSignedHardwareBootstrapReleaseV1,
    ) {
        // Known-public synthetic authorities; these tests install no release or runtime owner.
        let a = KeyPair::from_seed(vec![31; 32], Algorithm::Ed25519);
        let b = KeyPair::from_seed(vec![32; 32], Algorithm::Ed25519);
        let mut keys = vec![a.public_key().clone(), b.public_key().clone()];
        keys.sort();
        let policy = KagemushaReleaseAuthorityPolicyV1 {
            version: 1,
            authority_set_id: [2; 32],
            threshold: 2,
            authorized_signers: keys,
        };
        let m = KagemushaHardwareEvidenceBootstrapManifestV1 {
            version: 1,
            purpose: 1,
            authority_policy_digest: policy.canonical_digest().unwrap(),
            network_id: [3; 32],
            app_package: "test.known.public".into(),
            app_version_code: 1,
            core_origin: "https://test.known.example".into(),
            app_signing_identity_digest: [4; 32],
            app_distribution_digest: [5; 32],
            app_source_sha256: [6; 32],
            app_code_sha256: [12; 32],
            sdk_source_sha256: [7; 32],
            jni_artifacts: vec![
                KagemushaHardwareEvidenceJniArtifactV1 {
                    android_abi: "arm64-v8a".into(),
                    sha256: [8; 32],
                },
                KagemushaHardwareEvidenceJniArtifactV1 {
                    android_abi: "x86_64".into(),
                    sha256: [13; 32],
                },
            ],
            native_abi: 25,
            evidence_issuer: a.public_key().clone(),
            raw_verifier_policy_digest: [9; 32],
            google_oauth_issuer: "https://accounts.google.com".into(),
            google_oauth_client_id: "known-public-test-audience".into(),
            google_cloud_project_number: 1,
            play_integrity_policy: KagemushaPlayIntegrityPolicyV1 {
                policy_digest: [10; 32],
                maximum_evidence_age_ms: 120_000,
                maximum_refresh_interval_ms: 120_000,
                require_play_recognized: true,
                require_licensed: true,
                minimum_device_integrity: 1,
            },
            allowed_android_security_levels: vec![
                KagemushaAppKeySecurityLevelV1::TrustedExecutionEnvironment,
                KagemushaAppKeySecurityLevelV1::StrongBox,
            ],
            native_clock_selection_digest: [11; 32],
            native_clock_base_urls: std::array::from_fn(|i| {
                format!("https://public-clock-{i}.example/role/{i}/")
            }),
            policy_epoch: 1,
            not_before_ms: 1,
            expires_at_ms: 1_000_000,
            maximum_attempt_lifetime_ms: 120_000,
        };
        let message = m.signing_bytes().unwrap();
        let mut approvals = vec![
            KagemushaHardwareBootstrapReleaseApprovalV1 {
                public_key: a.public_key().clone(),
                signature: Signature::new(a.private_key(), &message),
            },
            KagemushaHardwareBootstrapReleaseApprovalV1 {
                public_key: b.public_key().clone(),
                signature: Signature::new(b.private_key(), &message),
            },
        ];
        approvals.sort_by(|a, b| a.public_key.cmp(&b.public_key));
        (
            policy,
            KagemushaSignedHardwareBootstrapReleaseV1 {
                manifest: m,
                approvals,
            },
        )
    }
    #[test]
    fn hardware_release_threshold_is_real_and_exact() {
        let (p, m) = fixture();
        m.authenticate(&p).unwrap();
        let raw = hardware_bootstrap_encode_v1(&m).unwrap();
        let decoded: KagemushaSignedHardwareBootstrapReleaseV1 =
            hardware_bootstrap_decode_v1(&raw).unwrap();
        assert_eq!(m, decoded);
        decoded.authenticate(&p).unwrap();
    }
    #[test]
    fn duplicate_missing_unknown_and_substituted_signers_reject() {
        let (p, m) = fixture();
        let mut missing = m.clone();
        missing.approvals.pop();
        assert!(missing.authenticate(&p).is_err());
        let mut duplicate = m.clone();
        duplicate.approvals[1] = duplicate.approvals[0].clone();
        assert!(duplicate.authenticate(&p).is_err());
        let mut other = p.clone();
        other.authority_set_id = [12; 32];
        assert!(m.authenticate(&other).is_err());
    }
    #[test]
    fn release_purpose_and_software_policy_cannot_be_promoted() {
        let (p, m) = fixture();
        let mut other = m.clone();
        other.manifest.purpose = 2;
        assert!(other.authenticate(&p).is_err());
        other = m.clone();
        other.manifest.allowed_android_security_levels.clear();
        assert!(other.authenticate(&p).is_err());
        other = m.clone();
        other
            .manifest
            .allowed_android_security_levels
            .push(KagemushaAppKeySecurityLevelV1::AppleAppAttest);
        assert!(other.authenticate(&p).is_err());
    }
    #[test]
    fn signed_package_source_clock_and_google_scope_cannot_be_substituted() {
        let (p, m) = fixture();
        for tag in 0..7 {
            let mut n = m.clone();
            match tag {
                0 => n.manifest.app_package = "other.package".into(),
                1 => n.manifest.sdk_source_sha256 = [21; 32],
                2 => n.manifest.jni_artifacts[0].sha256 = [22; 32],
                3 => n.manifest.native_clock_selection_digest = [23; 32],
                4 => n.manifest.google_oauth_client_id = "other-audience".into(),
                5 => n.manifest.core_origin = "https://other.example".into(),
                _ => n.manifest.network_id = [24; 32],
            }
            assert!(n.authenticate(&p).is_err());
        }
    }
    #[test]
    fn complete_jni_inventory_rejects_absence_duplicates_order_and_foreign_architectures() {
        let (_, m) = fixture();
        for mutation in 0..5 {
            let mut a = m.manifest.clone();
            match mutation {
                0 => a.jni_artifacts.clear(),
                1 => a.jni_artifacts[1] = a.jni_artifacts[0].clone(),
                2 => a.jni_artifacts.reverse(),
                3 => a.jni_artifacts[0].android_abi = "offered-ui-architecture".into(),
                _ => a.jni_artifacts[0].sha256 = [0; 32],
            }
            assert!(a.validate().is_err());
        }
    }
    #[test]
    fn complete_canonical_original_rejects_extra_bytes() {
        let (_, m) = fixture();
        let mut b = hardware_bootstrap_encode_v1(&m).unwrap();
        b.push(0);
        assert!(
            hardware_bootstrap_decode_v1::<KagemushaSignedHardwareBootstrapReleaseV1>(&b).is_err()
        );
    }
    #[test]
    fn reservation_alias_is_original_nonce_bound() {
        let a = KagemushaHardwareEvidenceReservationV1::alias_for([1; 32], [2; 32], [3; 32]);
        let mut r = KagemushaHardwareEvidenceReservationV1 {
            version: 1,
            manifest_digest: [1; 32],
            operation_id: [2; 32],
            client_nonce: [3; 32],
            alias: a,
            issued_at_ms: 1,
            deadline_ms: 120_001,
        };
        r.validate().unwrap();
        r.client_nonce = [4; 32];
        assert!(r.validate().is_err());
    }
    #[test]
    fn google_owner_binding_uses_length_delimited_exact_originals() {
        let a =
            kagemusha_hardware_evidence_google_owner_binding_v1([1; 32], "ab", "c", "d").unwrap();
        let b =
            kagemusha_hardware_evidence_google_owner_binding_v1([1; 32], "a", "bc", "d").unwrap();
        assert_ne!(a, b);
        assert!(
            kagemusha_hardware_evidence_google_owner_binding_v1([0; 32], "a", "b", "c").is_err()
        );
        assert!(
            kagemusha_hardware_evidence_google_owner_binding_v1([1; 32], "a", "b", "c\n").is_err()
        );
    }
    #[test]
    fn origin_never_admits_transport_redirect_selectors() {
        for s in [
            "http://test.example",
            "https://user@test.example",
            "https://test.example/path",
            "https://test.example?query",
            "https://test.example#fragment",
            "https://test.example:0",
            "https://test.example:65536",
            "https://::",
            "https://-test.example",
        ] {
            assert!(!valid_origin(s), "{s}");
        }
        assert!(valid_origin("https://test.example:443"));
    }
}
