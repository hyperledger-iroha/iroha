//! Exact private verifier configuration derived from selected Model policy originals.
//!
//! These are consistency-checked bytes, not operator approval or runtime custody. The node
//! must independently admit the policies, root originals, runtime and inherited descriptors.

use super::*;

const CONFIG_SCHEMA: &str = "iroha.kagemusha.wallet-e1-verifier-config.v1";
const MAX_CONFIG: usize = 128 * 1024;
const MAX_ROOT: usize = 16 * 1024;
const MAX_GOOGLE_POLICY: usize = 16 * 1024;

/// Independently selected public Google decoder original and its exact SHA-256 pin.
/// Construction alone grants no permission to use an OAuth credential or call Google.
pub struct GoogleDecoderOriginalV1<'a> {
    /// Exact governed public decoder JSON, never a mobile-supplied projection.
    pub original: &'a [u8],
    /// Pin supplied by the admitted operator selection, not inferred as approval.
    pub sha256: [u8; 32],
}

/// Public runtime locations already selected by the node's private process owner.
/// The worker is Linux-only. These strings do not open files or establish path custody.
pub struct VerifierRuntimeSelectionV1<'a> {
    /// Canonical absolute Linux path of the retained public OpenSSL executable.
    pub openssl_path: &'a str,
    /// Independently selected executable digest, also checked against inherited FD21.
    pub openssl_sha256: [u8; 32],
    /// Canonical absolute private directory path, also checked against inherited FD17.
    pub store_directory: &'a str,
}

/// Immutable private configuration bytes with their exact policy and digest bindings.
/// This object grants no enrollment, worker, signer, KYC or filesystem authority.
pub struct VerifierConfigurationV1 {
    original: Vec<u8>,
    digest: [u8; 32],
    app: KagemushaWalletAppPolicyV1,
    policy: KagemushaWalletEnrollmentPolicyV1,
}

fn linux_path(value: &str) -> Result<(), Error> {
    // Check spelling only. Actual descriptor/path identities and loaded dependencies belong
    // to the admitted process owner; no canonicalize-and-reopen operation is performed here.
    if value.len() > MAX_CONFIG
        || !value.starts_with('/')
        || value.as_bytes().contains(&0)
        || value[1..]
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return Err(Error("private runtime path spelling"));
    }
    Ok(())
}

impl VerifierConfigurationV1 {
    /// Derive the sole private worker projection from selected policy and public originals.
    /// Root and decoder pins are checked before encoding; the actual chain, Google decoder
    /// principal and runtime originals must still pass the private worker's verifiers.
    /// # Errors
    /// Rejects invalid/mismatched policies, roots, decoder pins, platform arms, path spelling,
    /// zero runtime pins or oversized configuration. It never reads secrets or a filesystem.
    pub fn from_selected(
        app: &KagemushaWalletAppPolicyV1,
        policy: &KagemushaWalletEnrollmentPolicyV1,
        root_original: &[u8],
        google: Option<GoogleDecoderOriginalV1<'_>>,
        runtime: VerifierRuntimeSelectionV1<'_>,
    ) -> Result<Self, Error> {
        policy
            .validate_for_app(app)
            .map_err(|_| Error("selected policy binding"))?;
        linux_path(runtime.openssl_path)?;
        linux_path(runtime.store_directory)?;
        if runtime.openssl_sha256 == [0; 32] {
            return Err(Error("empty runtime pin"));
        }
        let root_pin = match policy.platform {
            KagemushaWalletEnrollmentPlatformV1::Android {
                attestation_root_sha256,
                ..
            }
            | KagemushaWalletEnrollmentPlatformV1::Apple {
                attestation_root_sha256,
            } => attestation_root_sha256,
        };
        if root_original.is_empty()
            || root_original.len() > MAX_ROOT
            || <[u8; 32]>::from(Sha256::digest(root_original)) != root_pin
        {
            return Err(Error("selected root original"));
        }
        let regulator = norito::json!({
            "permitted_controls": (policy.regulatory_policy.permitted_controls),
            "blacklist_max_age_ms": (policy.regulatory_policy.blacklist_max_age_ms),
            "time_anchor_max_response_ms": (policy.regulatory_policy.time_anchor_max_response_ms),
        });
        let mut selected = norito::json!({
            "scheme_id_hex": (hex::encode(policy.scheme_id)),
            "asset_digest_hex": (hex::encode(policy.asset_digest)),
            "root_base64": (STANDARD.encode(root_original)),
            "root_sha256": (hex::encode(root_pin)),
            "regulatory_policy": (regulator),
            "challenge_lifetime_ms": (policy.challenge_lifetime_ms),
            "attestation_lease_lifetime_ms": (policy.attestation_lease_lifetime_ms),
        });
        let object = selected.as_object_mut().ok_or(Error("policy projection"))?;
        let platform = match (&app.identity, policy.platform, google) {
            (
                KagemushaWalletAppIdentityV1::Apple { app_id },
                KagemushaWalletEnrollmentPlatformV1::Apple { .. },
                None,
            ) => {
                object.insert("app_id".into(), Value::from(app_id.clone()));
                "apple"
            }
            (
                KagemushaWalletAppIdentityV1::Android {
                    package_name,
                    package_version,
                    app_signing_certificate_sha256,
                },
                KagemushaWalletEnrollmentPlatformV1::Android {
                    hardware,
                    patch_floor_yyyymm,
                    play_integrity_maximum_age_ms,
                    require_play_recognized,
                    require_licensed,
                    minimum_device_integrity,
                    ..
                },
                Some(google),
            ) => {
                if google.original.is_empty()
                    || google.original.len() > MAX_GOOGLE_POLICY
                    || google.sha256 == [0; 32]
                    || <[u8; 32]>::from(Sha256::digest(google.original)) != google.sha256
                {
                    return Err(Error("selected Google decoder original"));
                }
                let levels: Vec<u8> = match hardware {
                    KagemushaWalletAndroidHardwareV1::Tee => vec![1],
                    KagemushaWalletAndroidHardwareV1::StrongBox => vec![2],
                    KagemushaWalletAndroidHardwareV1::TeeOrStrongBox => vec![1, 2],
                };
                let minimum = match minimum_device_integrity {
                    KagemushaWalletPlayIntegrityLevelV1::Device => "MEETS_DEVICE_INTEGRITY",
                    KagemushaWalletPlayIntegrityLevelV1::Strong => "MEETS_STRONG_INTEGRITY",
                };
                let extra = norito::json!({
                    "package_name": (package_name), "package_version": (package_version),
                    "app_certificate_sha256": (hex::encode(app_signing_certificate_sha256)),
                    "security_levels": (levels), "patch_floor_yyyymm": (patch_floor_yyyymm),
                    "google_policy_base64": (STANDARD.encode(google.original)),
                    "google_policy_sha256": (hex::encode(google.sha256)),
                    "maximum_evidence_age_ms": (play_integrity_maximum_age_ms),
                    "require_play_recognized": (require_play_recognized),
                    "require_licensed": (require_licensed),
                    "minimum_device_integrity": (minimum),
                });
                for (name, value) in extra.as_object().ok_or(Error("policy projection"))? {
                    object.insert(name.clone(), value.clone());
                }
                "android"
            }
            _ => return Err(Error("selected verifier platform")),
        };
        let original = encode(
            &norito::json!({
                "schema": (CONFIG_SCHEMA), "version": (1_u16), "platform": (platform),
                "app_policy_hex": (hex::encode(app.policy_digest().map_err(|_| Error("app policy"))?)),
                "enrollment_policy_hex": (hex::encode(policy.policy_digest().map_err(|_| Error("enrollment policy"))?)),
                "openssl_path": (runtime.openssl_path),
                "openssl_sha256": (hex::encode(runtime.openssl_sha256)),
                "store_directory": (runtime.store_directory), "policy": (selected),
            }),
            MAX_CONFIG,
        )?;
        Ok(Self {
            digest: Sha256::digest(&original).into(),
            original,
            app: app.clone(),
            policy: *policy,
        })
    }

    /// Exact private configuration for retention and inherited FD20. Never regenerate on retry.
    pub fn original(&self) -> &[u8] {
        &self.original
    }

    /// SHA-256 of the exact retained configuration, not a proof of its admission.
    pub fn digest(&self) -> [u8; 32] {
        self.digest
    }

    /// Observe the configured private worker's journal with a fresh exchange identity.
    /// # Errors
    /// Empty exchange identity or packet bounds; this establishes no runtime authority.
    pub fn journal(&self, exchange: [u8; 32]) -> Result<VerifierExchangeV1, Error> {
        VerifierExchangeV1::journal(self.digest, exchange)
    }

    /// Bind a pre-key preparation to this exact selected app, enrollment policy and configuration.
    /// # Errors
    /// Foreign selection, invalid challenge/account originals or challenge time.
    pub fn preparation(
        &self,
        dispatch: &PreKeyDispatchV1,
        challenge: KagemushaWalletEnrollmentChallengeV1,
        created_at_ms: u64,
    ) -> Result<VerifierPreparationV1, Error> {
        if dispatch.app != self.app || dispatch.policy != self.policy {
            return Err(Error("approved configuration binding"));
        }
        VerifierPreparationV1::from_selected(dispatch, challenge, created_at_ms, self.digest)
    }

    /// Bind account-signed E5 to a previously selected preparation under this exact configuration.
    /// # Errors
    /// Changed preparation/configuration/policies, invalid E5 or captured challenge time.
    pub fn request(
        &self,
        request: RequestV1,
        preparation: &VerifierPreparationV1,
        verification_time_ms: u64,
    ) -> Result<VerifierRequestV1, Error> {
        if preparation.configuration() != self.digest
            || request.body.app != self.app
            || request.body.policy != self.policy
        {
            return Err(Error("approved configuration binding"));
        }
        VerifierRequestV1::from_prepared(request, preparation, verification_time_ms)
    }
}
