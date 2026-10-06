//! NEW first-release E1 policy preimages. These unsigned identities select verifier inputs;
//! they never authenticate operator policy, delegate a signer or admit an enrollment.

use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

use super::{
    WalletResult, WalletVersionsV1, decode_frame_v1,
    digest::{KagemushaWalletDigestRoleV1 as Role, WalletTranscriptV1, kagemusha_wallet_digest_v1},
    encode_frame_v1,
    identity::{
        KAGEMUSHA_WALLET_CONTROL_ATTESTATION_LEASE_V1, KagemushaWalletEnrollmentChallengeV1,
        KagemushaWalletRegulatoryPolicyV1,
    },
    invalid_v1, overflow_v1, require_nonzero_v1, require_scheme_v1, require_version_v1,
};

#[cfg(test)]
#[path = "enrollment_policy_tests.rs"]
pub(super) mod enrollment_policy_tests;

/// Complete canonical Norito frame cap for either NEW policy; checked before decoding.
pub const KAGEMUSHA_WALLET_ENROLLMENT_POLICY_MAX_BYTES_V1: usize = 1024;
// NEW frames contain no u128. Keep zero type padding under the current native frame contract;
// these shipping-library assertions qualify layout only, not actual native/device operation.
const _: () = {
    assert!(
        norito::core::Header::SIZE
            % norito::core::archived_payload_align::<KagemushaWalletAppPolicyV1>()
            == 0
    );
    assert!(
        norito::core::Header::SIZE
            % norito::core::archived_payload_align::<KagemushaWalletEnrollmentPolicyV1>()
            == 0
    );
};

/// Current Android package and Apple App ID UTF-8 bound, without normalization.
pub const KAGEMUSHA_WALLET_APP_IDENTIFIER_MAX_BYTES_V1: usize = 255;

/// Exact app identity selected at initial enrollment. It stays historical across renewal.
// On 32-bit targets the inline Android certificate pin outweighs the smaller
// String-only Apple identity. Keep the canonical fields and encoding unchanged.
#[cfg_attr(target_pointer_width = "32", allow(variant_size_differences))]
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema, norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_data_model::kagemusha::kagemusha_wallet_v1::KagemushaWalletAppIdentityV1"
)]
pub enum KagemushaWalletAppIdentityV1 {
    /// Transcript tag 1: package, exact version and SHA-256 of the app-signing certificate.
    #[codec(index = 1)]
    Android {
        /// Dotted ASCII package, using the current Google decoder grammar.
        package_name: String,
        /// Exact package version; zero is representable.
        package_version: u64,
        /// Nonzero SHA-256 of the selected certificate DER.
        app_signing_certificate_sha256: [u8; 32],
    },
    /// Transcript tag 2: exact UTF-8 App ID used by the production App Attest verifier.
    #[codec(index = 2)]
    Apple {
        /// Nonempty App ID of at most 255 UTF-8 bytes.
        app_id: String,
    },
}

/// NEW `H("app-policy", transcript)` preimage. No approval or signature is implied.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema, norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_data_model::kagemusha::kagemusha_wallet_v1::KagemushaWalletAppPolicyV1"
)]
pub struct KagemushaWalletAppPolicyV1 {
    /// Exactly version 1.
    pub version: u16,
    /// Nonzero scheme identity.
    pub scheme_id: [u8; 32],
    /// Exactly one platform app identity; no disabled/development variant.
    pub identity: KagemushaWalletAppIdentityV1,
}

fn package_valid(value: &str) -> bool {
    value.len() <= KAGEMUSHA_WALLET_APP_IDENTIFIER_MAX_BYTES_V1
        && value.split('.').count() >= 2
        && value.split('.').all(|part| {
            let mut bytes = part.bytes();
            bytes
                .next()
                .is_some_and(|first| first.is_ascii_alphabetic() || first == b'_')
                && bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        })
}

impl KagemushaWalletAppPolicyV1 {
    /// Validate version, scheme and the actual current verifier's exact app grammar.
    ///
    /// # Errors
    /// Rejects invalid identifiers or a zero Android app-signing certificate pin.
    pub fn validate(&self) -> WalletResult<()> {
        self.require_versions()?;
        require_nonzero_v1("app_policy.scheme_id", &self.scheme_id)?;
        match &self.identity {
            KagemushaWalletAppIdentityV1::Android {
                package_name,
                app_signing_certificate_sha256,
                ..
            } => {
                if !package_valid(package_name) {
                    return Err(invalid_v1("app_policy.package_name"));
                }
                require_nonzero_v1(
                    "app_policy.app_signing_certificate_sha256",
                    app_signing_certificate_sha256,
                )?;
            }
            KagemushaWalletAppIdentityV1::Apple { app_id } => {
                if app_id.is_empty() || app_id.len() > KAGEMUSHA_WALLET_APP_IDENTIFIER_MAX_BYTES_V1
                {
                    return Err(invalid_v1("app_policy.app_id"));
                }
            }
        }
        Ok(())
    }

    /// Exact NEW transcript; text is LE32 byte length followed by unchanged UTF-8.
    ///
    /// # Errors
    /// Rejects an invalid policy before writing any transcript.
    pub fn transcript(&self) -> WalletResult<Vec<u8>> {
        self.validate()?;
        let base = WalletTranscriptV1::with_capacity(334)
            .u16(self.version)
            .digest(&self.scheme_id);
        Ok(match &self.identity {
            KagemushaWalletAppIdentityV1::Android {
                package_name,
                package_version,
                app_signing_certificate_sha256,
            } => base
                .u8(1)
                .u32(u32::try_from(package_name.len()).expect("validated length"))
                .bytes(package_name.as_bytes())
                .u64(*package_version)
                .digest(app_signing_certificate_sha256)
                .finish(),
            KagemushaWalletAppIdentityV1::Apple { app_id } => base
                .u8(2)
                .u32(u32::try_from(app_id.len()).expect("validated length"))
                .bytes(app_id.as_bytes())
                .finish(),
        })
    }

    /// NEW app-policy identity, distinct from every existing digest role.
    ///
    /// # Errors
    /// Rejects an invalid policy; this digest does not approve it.
    pub fn policy_digest(&self) -> WalletResult<[u8; 32]> {
        Ok(kagemusha_wallet_digest_v1(
            Role::AppPolicy,
            &self.transcript()?,
        ))
    }

    /// Encode a validated canonical Norito frame under the complete-frame cap.
    ///
    /// # Errors
    /// Rejects invalid fields or an oversized frame.
    pub fn encode_canonical(&self) -> WalletResult<Vec<u8>> {
        self.validate()?;
        encode_frame_v1(self, KAGEMUSHA_WALLET_ENROLLMENT_POLICY_MAX_BYTES_V1)
    }

    /// Decode under the byte cap, then version, expected scheme, and structural rules.
    ///
    /// # Errors
    /// Rejects noncanonical bytes, unsupported versions, wrong scheme or invalid fields.
    pub fn decode_canonical(bytes: &[u8], expected_scheme: &[u8; 32]) -> WalletResult<Self> {
        let value: Self = decode_frame_v1(bytes, KAGEMUSHA_WALLET_ENROLLMENT_POLICY_MAX_BYTES_V1)?;
        value.require_versions()?;
        require_scheme_v1("app_policy.scheme_id", &value.scheme_id, expected_scheme)?;
        value.validate()?;
        Ok(value)
    }
}

impl WalletVersionsV1 for KagemushaWalletAppPolicyV1 {
    fn require_versions(&self) -> WalletResult<()> {
        require_version_v1("app_policy.version", self.version)
    }
}

/// Current KeyMint hardware levels; no software or disabled selector exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema, norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_data_model::kagemusha::kagemusha_wallet_v1::KagemushaWalletAndroidHardwareV1"
)]
pub enum KagemushaWalletAndroidHardwareV1 {
    /// Transcript tag 1: only hardware TEE level 1.
    #[codec(index = 1)]
    Tee,
    /// Transcript tag 2: only StrongBox level 2.
    #[codec(index = 2)]
    StrongBox,
    /// Transcript tag 3: either of the two hardware levels.
    #[codec(index = 3)]
    TeeOrStrongBox,
}

/// Finite current Google minimum-device verdict selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema, norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_data_model::kagemusha::kagemusha_wallet_v1::KagemushaWalletPlayIntegrityLevelV1"
)]
pub enum KagemushaWalletPlayIntegrityLevelV1 {
    /// Transcript tag 1: `MEETS_DEVICE_INTEGRITY` required.
    #[codec(index = 1)]
    Device,
    /// Transcript tag 2: `MEETS_STRONG_INTEGRITY` required.
    #[codec(index = 2)]
    Strong,
}

/// Mandatory platform verification inputs. Root DER originals remain private issuer inputs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema, norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_data_model::kagemusha::kagemusha_wallet_v1::KagemushaWalletEnrollmentPlatformV1"
)]
pub enum KagemushaWalletEnrollmentPlatformV1 {
    /// Transcript tag 1: KeyMint, current Google revocation and server Google decode all run.
    #[codec(index = 1)]
    Android {
        /// Nonzero SHA-256 pin of the genuine selected Google root DER.
        attestation_root_sha256: [u8; 32],
        /// Exact allowed hardware levels.
        hardware: KagemushaWalletAndroidHardwareV1,
        /// Exact YYYYMM; failure records an unset patch-policy fact and does not reject E1.
        patch_floor_yyyymm: u32,
        /// Positive Google timestamp age bound, no greater than this E1 challenge lifetime.
        play_integrity_maximum_age_ms: u64,
        /// Whether Google's recognized-app verdict is required; decoder verification always runs.
        require_play_recognized: bool,
        /// Whether Google's licensed verdict is required; decoder verification always runs.
        require_licensed: bool,
        /// One of the current physical-device minimum verdicts.
        minimum_device_integrity: KagemushaWalletPlayIntegrityLevelV1,
    },
    /// Transcript tag 2: production App Attest plus a durable fresh key-binding assertion.
    #[codec(index = 2)]
    Apple {
        /// Nonzero SHA-256 pin of the genuine selected Apple root DER.
        attestation_root_sha256: [u8; 32],
    },
}

/// NEW `H("enrollment-policy", transcript)` preimage; not a signer or policy approval.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema, norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_data_model::kagemusha::kagemusha_wallet_v1::KagemushaWalletEnrollmentPolicyV1"
)]
pub struct KagemushaWalletEnrollmentPolicyV1 {
    /// Exactly version 1.
    pub version: u16,
    /// Nonzero scheme identity.
    pub scheme_id: [u8; 32],
    /// Nonzero asset scope; regulatory and lease decisions cannot cross asset scopes.
    pub asset_digest: [u8; 32],
    /// Exact NEW app-policy digest.
    pub app_policy: [u8; 32],
    /// Exactly one platform verifier input set.
    pub platform: KagemushaWalletEnrollmentPlatformV1,
    /// Existing inline credential regulatory policy, unchanged by these NEW identities.
    pub regulatory_policy: KagemushaWalletRegulatoryPolicyV1,
    /// Positive server-owned single-use E1 challenge lifetime. No implicit value.
    pub challenge_lifetime_ms: u64,
    /// Zero iff the regulator does not permit a lease; otherwise positive. No PI lease.
    pub attestation_lease_lifetime_ms: u64,
}

impl KagemushaWalletEnrollmentPolicyV1 {
    /// Validate all self-contained fields; policy selection still needs genuine owner approval.
    ///
    /// # Errors
    /// Rejects zero bindings, invalid patch/Google selectors, inconsistent regulators/lease.
    pub fn validate(&self) -> WalletResult<()> {
        self.require_versions()?;
        require_nonzero_v1("enrollment_policy.scheme_id", &self.scheme_id)?;
        require_nonzero_v1("enrollment_policy.asset_digest", &self.asset_digest)?;
        require_nonzero_v1("enrollment_policy.app_policy", &self.app_policy)?;
        self.regulatory_policy.validate()?;
        if self.challenge_lifetime_ms == 0 {
            return Err(invalid_v1("enrollment_policy.challenge_lifetime_ms"));
        }
        if (self.attestation_lease_lifetime_ms > 0)
            != self
                .regulatory_policy
                .permits(KAGEMUSHA_WALLET_CONTROL_ATTESTATION_LEASE_V1)
        {
            return Err(invalid_v1(
                "enrollment_policy.attestation_lease_lifetime_ms",
            ));
        }
        match self.platform {
            KagemushaWalletEnrollmentPlatformV1::Android {
                attestation_root_sha256,
                patch_floor_yyyymm,
                play_integrity_maximum_age_ms,
                ..
            } => {
                require_nonzero_v1(
                    "enrollment_policy.attestation_root_sha256",
                    &attestation_root_sha256,
                )?;
                let year = patch_floor_yyyymm / 100;
                let month = patch_floor_yyyymm % 100;
                if !(1900..=9999).contains(&year) || !(1..=12).contains(&month) {
                    return Err(invalid_v1("enrollment_policy.patch_floor_yyyymm"));
                }
                if play_integrity_maximum_age_ms == 0
                    || play_integrity_maximum_age_ms > self.challenge_lifetime_ms
                {
                    return Err(invalid_v1(
                        "enrollment_policy.play_integrity_maximum_age_ms",
                    ));
                }
            }
            KagemushaWalletEnrollmentPlatformV1::Apple {
                attestation_root_sha256,
            } => require_nonzero_v1(
                "enrollment_policy.attestation_root_sha256",
                &attestation_root_sha256,
            )?,
        }
        Ok(())
    }

    /// Validate the exact app/scheme/platform pairing, without authenticating its approval.
    ///
    /// # Errors
    /// Rejects invalid policies or app/scheme/platform substitutions.
    pub fn validate_for_app(&self, app: &KagemushaWalletAppPolicyV1) -> WalletResult<()> {
        self.validate()?;
        app.validate()?;
        require_scheme_v1(
            "enrollment_policy.scheme_id",
            &self.scheme_id,
            &app.scheme_id,
        )?;
        if self.app_policy != app.policy_digest()? {
            return Err(invalid_v1("enrollment_policy.app_policy"));
        }
        if !matches!(
            (&app.identity, self.platform),
            (
                KagemushaWalletAppIdentityV1::Android { .. },
                KagemushaWalletEnrollmentPlatformV1::Android { .. }
            ) | (
                KagemushaWalletAppIdentityV1::Apple { .. },
                KagemushaWalletEnrollmentPlatformV1::Apple { .. }
            )
        ) {
            return Err(invalid_v1("enrollment_policy.platform"));
        }
        Ok(())
    }

    /// Exact NEW transcript; regulator bytes and durations follow the selected platform.
    ///
    /// # Errors
    /// Rejects invalid fields before writing the transcript.
    pub fn transcript(&self) -> WalletResult<Vec<u8>> {
        self.validate()?;
        let base = WalletTranscriptV1::with_capacity(183)
            .u16(self.version)
            .digest(&self.scheme_id)
            .digest(&self.asset_digest)
            .digest(&self.app_policy);
        let selected = match self.platform {
            KagemushaWalletEnrollmentPlatformV1::Android {
                attestation_root_sha256,
                hardware,
                patch_floor_yyyymm,
                play_integrity_maximum_age_ms,
                require_play_recognized,
                require_licensed,
                minimum_device_integrity,
            } => {
                let hardware_tag = match hardware {
                    KagemushaWalletAndroidHardwareV1::Tee => 1,
                    KagemushaWalletAndroidHardwareV1::StrongBox => 2,
                    KagemushaWalletAndroidHardwareV1::TeeOrStrongBox => 3,
                };
                let level_tag = match minimum_device_integrity {
                    KagemushaWalletPlayIntegrityLevelV1::Device => 1,
                    KagemushaWalletPlayIntegrityLevelV1::Strong => 2,
                };
                base.u8(1)
                    .digest(&attestation_root_sha256)
                    .u8(hardware_tag)
                    .u32(patch_floor_yyyymm)
                    .u64(play_integrity_maximum_age_ms)
                    .u8(u8::from(require_play_recognized))
                    .u8(u8::from(require_licensed))
                    .u8(level_tag)
            }
            KagemushaWalletEnrollmentPlatformV1::Apple {
                attestation_root_sha256,
            } => base.u8(2).digest(&attestation_root_sha256),
        };
        Ok(selected
            .bytes(&self.regulatory_policy.transcript())
            .u64(self.challenge_lifetime_ms)
            .u64(self.attestation_lease_lifetime_ms)
            .finish())
    }

    /// NEW policy identity; an existing opaque digest has no implied mapping to it.
    ///
    /// # Errors
    /// Rejects invalid fields; this digest grants no authority.
    pub fn policy_digest(&self) -> WalletResult<[u8; 32]> {
        Ok(kagemusha_wallet_digest_v1(
            Role::EnrollmentPolicy,
            &self.transcript()?,
        ))
    }

    /// Check the exact policy identities and selected scheme/asset in an E1 challenge.
    ///
    /// # Errors
    /// Rejects any substitution. Account authorization, freshness and use require the issuer.
    pub fn verify_challenge(
        &self,
        app: &KagemushaWalletAppPolicyV1,
        challenge: &KagemushaWalletEnrollmentChallengeV1,
    ) -> WalletResult<()> {
        self.validate_for_app(app)?;
        challenge.validate()?;
        require_scheme_v1(
            "enrollment_challenge.scheme_id",
            &challenge.scheme_id,
            &self.scheme_id,
        )?;
        if challenge.asset_digest != self.asset_digest
            || challenge.app_policy != self.app_policy
            || challenge.enrollment_policy != self.policy_digest()?
        {
            return Err(invalid_v1("enrollment_challenge.policy"));
        }
        Ok(())
    }

    /// Enforce NEW `[created, created + lifetime)` using genuine server-owned timestamps.
    ///
    /// # Errors
    /// Rejects absent/future creation, expiry and overflow. This does not consume the challenge.
    pub fn require_live_challenge(&self, created_at_ms: u64, now_ms: u64) -> WalletResult<()> {
        self.validate()?;
        let expires = created_at_ms
            .checked_add(self.challenge_lifetime_ms)
            .ok_or_else(|| overflow_v1("enrollment_policy.challenge_expiry"))?;
        if created_at_ms == 0 || now_ms < created_at_ms || now_ms >= expires {
            return Err(invalid_v1("enrollment_policy.challenge_time"));
        }
        Ok(())
    }

    /// Produce the exact NEW issuance lease value from the permitted duration; no PI lease.
    ///
    /// # Errors
    /// Rejects absent issuance or overflow. Only the Enrollment-role owner may sign the result.
    pub fn lease_expires_at(&self, issued_at_ms: u64) -> WalletResult<u64> {
        self.validate()?;
        if issued_at_ms == 0 {
            return Err(invalid_v1("enrollment_policy.issued_at_ms"));
        }
        if self.attestation_lease_lifetime_ms == 0 {
            return Ok(0);
        }
        issued_at_ms
            .checked_add(self.attestation_lease_lifetime_ms)
            .ok_or_else(|| overflow_v1("enrollment_policy.lease_expiry"))
    }

    /// Encode a validated canonical Norito frame under its complete-frame cap.
    ///
    /// # Errors
    /// Rejects invalid fields or an oversized frame.
    pub fn encode_canonical(&self) -> WalletResult<Vec<u8>> {
        self.validate()?;
        encode_frame_v1(self, KAGEMUSHA_WALLET_ENROLLMENT_POLICY_MAX_BYTES_V1)
    }

    /// Decode under the cap, then version, expected scheme and structural checks.
    ///
    /// # Errors
    /// Rejects noncanonical bytes, unsupported versions, wrong scheme or invalid fields.
    pub fn decode_canonical(bytes: &[u8], expected_scheme: &[u8; 32]) -> WalletResult<Self> {
        let value: Self = decode_frame_v1(bytes, KAGEMUSHA_WALLET_ENROLLMENT_POLICY_MAX_BYTES_V1)?;
        value.require_versions()?;
        require_scheme_v1(
            "enrollment_policy.scheme_id",
            &value.scheme_id,
            expected_scheme,
        )?;
        value.validate()?;
        Ok(value)
    }
}

impl WalletVersionsV1 for KagemushaWalletEnrollmentPolicyV1 {
    fn require_versions(&self) -> WalletResult<()> {
        require_version_v1("enrollment_policy.version", self.version)
    }
}
