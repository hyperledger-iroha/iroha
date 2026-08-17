fn ensure_offline_attestation_policy_limit(
    actual: usize,
    maximum: usize,
    field: &str,
) -> Result<(), Error> {
    if actual > maximum {
        return Err(labeled_invariant(
            "invalid_attestation_policy",
            format!(
                "Offline device attestation policy {field} exceeds the first-release limit of {maximum}"
            ),
        )
        .into());
    }
    Ok(())
}

fn validate_offline_attestation_policy_bounds(
    policy: &OfflineDeviceAttestationPolicy,
) -> Result<(), Error> {
    ensure_offline_attestation_policy_limit(
        policy.trusted_roots.len(),
        OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_TRUSTED_ROOTS_V1,
        "trusted-root count",
    )?;
    ensure_offline_attestation_policy_limit(
        policy.revoked_certificate_sha256.len(),
        OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_REVOKED_CERTIFICATES_V1,
        "revoked-certificate count",
    )?;
    ensure_offline_attestation_policy_limit(
        policy.ios_apps.len(),
        OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_IOS_APPS_V1,
        "iOS app count",
    )?;
    ensure_offline_attestation_policy_limit(
        policy.android_apps.len(),
        OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_ANDROID_APPS_V1,
        "Android app count",
    )?;
    for platform in [
        OFFLINE_ATTESTATION_PLATFORM_IOS_APP_ATTEST,
        OFFLINE_ATTESTATION_PLATFORM_ANDROID_KEYMINT,
    ] {
        ensure_offline_attestation_policy_limit(
            policy
                .trusted_roots
                .iter()
                .filter(|root| root.platform == platform)
                .count(),
            OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_TRUSTED_ROOTS_PER_PLATFORM_V1,
            "trusted roots per platform",
        )?;
    }
    for root in &policy.trusted_roots {
        ensure_offline_attestation_policy_limit(
            root.platform.len(),
            OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_APP_IDENTIFIER_BYTES_V1,
            "trusted-root platform bytes",
        )?;
        ensure_offline_attestation_policy_limit(
            root.der.len(),
            OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_TRUSTED_ROOT_DER_BYTES_V1,
            "trusted-root DER bytes",
        )?;
    }
    for digest in &policy.revoked_certificate_sha256 {
        ensure_offline_attestation_policy_limit(
            digest.len(),
            32,
            "revoked-certificate digest bytes",
        )?;
    }
    for app in &policy.ios_apps {
        ensure_offline_attestation_policy_limit(
            app.team_id.len(),
            OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_TEAM_ID_BYTES_V1,
            "iOS Team ID bytes",
        )?;
        ensure_offline_attestation_policy_limit(
            app.bundle_id.len(),
            OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_APP_IDENTIFIER_BYTES_V1,
            "iOS bundle ID bytes",
        )?;
        ensure_offline_attestation_policy_limit(
            app.environment.len(),
            OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_APP_IDENTIFIER_BYTES_V1,
            "iOS environment bytes",
        )?;
        ensure_offline_attestation_policy_limit(
            app.allowed_validation_categories.len(),
            OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_IOS_VALIDATION_CATEGORIES_V1,
            "iOS validation-category count",
        )?;
        ensure_offline_attestation_policy_limit(
            app.allowed_bundle_versions.len(),
            OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_IOS_BUNDLE_VERSIONS_V1,
            "iOS bundle-version count",
        )?;
        for version in &app.allowed_bundle_versions {
            ensure_offline_attestation_policy_limit(
                version.len(),
                OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_IOS_BUNDLE_VERSION_BYTES_V1,
                "iOS bundle-version bytes",
            )?;
        }
    }
    for app in &policy.android_apps {
        ensure_offline_attestation_policy_limit(
            app.package_name.len(),
            OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_APP_IDENTIFIER_BYTES_V1,
            "Android package-name bytes",
        )?;
        ensure_offline_attestation_policy_limit(
            app.signing_certificate_sha256.len(),
            OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_ANDROID_SIGNING_CERTIFICATES_V1,
            "Android signing-certificate count",
        )?;
        for digest in &app.signing_certificate_sha256 {
            ensure_offline_attestation_policy_limit(
                digest.len(),
                32,
                "Android signing-certificate digest bytes",
            )?;
        }
    }
    let canonical = norito::encode_canonical(policy).map_err(|error| {
        labeled_invariant(
            "invalid_attestation_policy",
            format!("failed to encode Offline device attestation policy: {error}"),
        )
    })?;
    ensure_offline_attestation_policy_limit(
        canonical.len(),
        OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_CANONICAL_BYTES_V1,
        "canonical bytes",
    )
}
