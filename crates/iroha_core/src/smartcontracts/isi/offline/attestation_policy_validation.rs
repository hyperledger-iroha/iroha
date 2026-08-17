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
    fn normalize_policy_ascii(value: &str, field: &str) -> Result<String, Error> {
        let trimmed = value.trim();
        if trimmed.is_empty() || !trimmed.is_ascii() || trimmed.chars().any(char::is_control) {
            return Err(labeled_invariant(
                "invalid_attestation_policy",
                format!(
                    "Offline device attestation policy {field} must be non-empty printable ASCII"
                ),
            )
            .into());
        }
        if trimmed != value {
            return Err(labeled_invariant(
                "invalid_attestation_policy",
                format!(
                    "Offline device attestation policy {field} must not contain surrounding whitespace"
                ),
            )
            .into());
        }
        Ok(value.to_owned())
    }
    fn normalize_sha256_digest(digest: &[u8], field: &str) -> Result<[u8; 32], Error> {
        digest.try_into().map_err(|_| {
            labeled_invariant(
                "invalid_attestation_policy",
                format!(
                    "Offline device attestation policy {field} must be a 32-byte SHA-256 digest"
                ),
            )
            .into()
        })
    }
    fn trusted_root_is_active(
        root: &OfflineDeviceAttestationTrustedRoot,
        block_unix_timestamp_ms: u64,
    ) -> bool {
        root.not_before_ms
            .is_none_or(|not_before_ms| block_unix_timestamp_ms >= not_before_ms)
            && root
                .not_after_ms
                .is_none_or(|not_after_ms| block_unix_timestamp_ms <= not_after_ms)
    }
    fn offline_attestation_policy_for_registration_lifetime(
        policy: &OfflineDeviceAttestationPolicy,
        platform: &str,
        admitted_at_ms: u64,
        expires_at_ms: u64,
    ) -> Result<OfflineDeviceAttestationPolicy, Error> {
        let mut lifetime_policy = policy.clone();
        lifetime_policy.trusted_roots.retain(|root| {
            root.platform == platform
                && trusted_root_is_active(root, admitted_at_ms)
                && trusted_root_is_active(root, expires_at_ms)
        });
        if lifetime_policy.trusted_roots.is_empty() {
            return Err(labeled_invariant(
                "invalid_attestation_policy",
                "Offline device attestation policy has no trusted platform root covering the full registration lifetime",
            )
            .into());
        }
        Ok(lifetime_policy)
    }
    fn validate_offline_attestation_policy(
        policy: &OfflineDeviceAttestationPolicy,
        block_unix_timestamp_ms: u64,
    ) -> Result<(), Error> {
        if policy.version != 1 {
            return Err(labeled_invariant(
                "invalid_attestation_policy",
                "Offline device attestation policy version is unsupported",
            )
            .into());
        }
        validate_offline_attestation_policy_bounds(policy)?;
        if policy.trusted_roots.is_empty() {
            return Err(labeled_invariant(
                "invalid_attestation_policy",
                "Offline device attestation policy must include at least one trusted root",
            )
            .into());
        }
        let evaluation_time = x509_evaluation_time(block_unix_timestamp_ms)?;
        let mut root_hashes = HashSet::new();
        for root in &policy.trusted_roots {
            match root.platform.as_str() {
                OFFLINE_ATTESTATION_PLATFORM_IOS_APP_ATTEST
                | OFFLINE_ATTESTATION_PLATFORM_ANDROID_KEYMINT => {}
                _ => {
                    return Err(labeled_invariant(
                        "invalid_attestation_policy",
                        "Offline device attestation policy trusted root platform is unsupported",
                    )
                    .into());
                }
            }
            if root.der.is_empty()
                || root
                    .not_before_ms
                    .zip(root.not_after_ms)
                    .is_some_and(|(not_before, not_after)| not_before > not_after)
            {
                return Err(labeled_invariant(
                    "invalid_attestation_policy",
                    "Offline device attestation policy trusted root lifetime is invalid",
                )
                .into());
            }
            let digest = sha256_bytes(&root.der);
            if !root_hashes.insert(digest) {
                return Err(labeled_invariant(
                    "invalid_attestation_policy",
                    "Offline device attestation policy contains a duplicate trusted root",
                )
                .into());
            }
            let certificate = parse_x509_certificate_der(&root.der)?;
            validate_x509_certificate_critical_extensions(&certificate)?;
            if trusted_root_is_active(root, block_unix_timestamp_ms) {
                validate_x509_certificate_time(&certificate, evaluation_time)?;
            }
            if !x509_certificate_is_ca(&certificate)? {
                return Err(labeled_invariant(
                    "invalid_attestation_policy",
                    "Offline device attestation policy trusted root must be a CA certificate",
                )
                .into());
            }
        }
        let mut revoked = HashSet::new();
        for digest in &policy.revoked_certificate_sha256 {
            let digest = normalize_sha256_digest(digest, "revoked certificate digest")?;
            if digest == [0u8; 32] || !revoked.insert(digest) {
                return Err(labeled_invariant(
                    "invalid_attestation_policy",
                    "Offline device attestation policy has an invalid revoked certificate digest",
                )
                .into());
            }
        }
        let mut ios_apps = HashSet::new();
        for app in &policy.ios_apps {
            let team_id = normalize_policy_ascii(&app.team_id, "iOS Team ID")?.to_ascii_uppercase();
            let bundle_id = normalize_policy_ascii(&app.bundle_id, "iOS bundle ID")?;
            let environment =
                normalize_policy_ascii(&app.environment, "iOS environment")?.to_ascii_lowercase();
            if environment != OFFLINE_ATTESTATION_IOS_ENV_PRODUCTION
                && environment != OFFLINE_ATTESTATION_IOS_ENV_DEVELOPMENT
            {
                return Err(labeled_invariant(
                    "invalid_attestation_policy",
                    "Offline device attestation policy iOS environment must be production or development",
                )
                .into());
            }
            if !ios_apps.insert((team_id, bundle_id, environment)) {
                return Err(labeled_invariant(
                    "invalid_attestation_policy",
                    "Offline device attestation policy contains a duplicate iOS app identity",
                )
                .into());
            }
            if app.allowed_validation_categories.is_empty()
                || app.allowed_bundle_versions.is_empty()
            {
                return Err(labeled_invariant(
                    "invalid_attestation_policy",
                    "Offline device attestation policy iOS app must configure non-empty extension category and bundle-version allowlists",
                )
                .into());
            }
            let mut validation_categories = HashSet::new();
            for category in &app.allowed_validation_categories {
                if !matches!(*category, 1..=6 | 10) || !validation_categories.insert(*category) {
                    return Err(labeled_invariant(
                        "invalid_attestation_policy",
                        "Offline device attestation policy iOS app has an invalid or duplicate validation category",
                    )
                    .into());
                }
            }
            let mut bundle_versions = HashSet::new();
            for bundle_version in &app.allowed_bundle_versions {
                let bundle_version =
                    normalize_policy_ascii(bundle_version, "iOS allowed bundle version")?;
                if bundle_version.len()
                    > OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_IOS_BUNDLE_VERSION_BYTES_V1
                    || bundle_version.chars().any(char::is_control)
                    || !bundle_versions.insert(bundle_version)
                {
                    return Err(labeled_invariant(
                        "invalid_attestation_policy",
                        "Offline device attestation policy iOS app has an invalid or duplicate bundle version",
                    )
                    .into());
                }
            }
        }
        let mut android_apps = HashSet::new();
        for app in &policy.android_apps {
            let package_name = normalize_policy_ascii(&app.package_name, "Android package name")?;
            if app.signing_certificate_sha256.is_empty() {
                return Err(labeled_invariant(
                    "invalid_attestation_policy",
                    "Offline device attestation policy Android app must include signing digests",
                )
                .into());
            }
            let mut signing_digests = Vec::with_capacity(app.signing_certificate_sha256.len());
            let mut seen_signers = HashSet::new();
            for digest in &app.signing_certificate_sha256 {
                let digest = normalize_sha256_digest(digest, "Android signing certificate digest")?;
                if digest == [0u8; 32] || !seen_signers.insert(digest) {
                    return Err(labeled_invariant(
                        "invalid_attestation_policy",
                        "Offline device attestation policy Android app has an invalid signing digest",
                    )
                    .into());
                }
                signing_digests.push(digest);
            }
            signing_digests.sort_unstable();
            if !android_apps.insert((package_name, signing_digests)) {
                return Err(labeled_invariant(
                    "invalid_attestation_policy",
                    "Offline device attestation policy contains a duplicate Android app identity",
                )
                .into());
            }
        }
        if policy.require_ios_app_policy && policy.ios_apps.is_empty() {
            return Err(labeled_invariant(
                "invalid_attestation_policy",
                "Offline device attestation policy requires iOS apps but none are configured",
            )
            .into());
        }
        if policy.require_android_app_policy && policy.android_apps.is_empty() {
            return Err(labeled_invariant(
                "invalid_attestation_policy",
                "Offline device attestation policy requires Android apps but none are configured",
            )
            .into());
        }
        Ok(())
    }
    /// Validate the complete consensus policy used by one Kagemusha release activation.
    ///
    /// This side-effect-free entry point is shared with operator tooling so a
    /// prepared activation cannot pass a weaker X.509 or application-policy
    /// check than the instruction will receive during consensus execution.
    ///
    /// # Errors
    ///
    /// Returns an instruction-execution error when the policy is non-canonical,
    /// exceeds a protocol bound, contains an invalid trusted CA certificate, is
    /// not valid at `block_unix_timestamp_ms`, or does not enable the required
    /// fail-closed production application policies.
    pub fn validate_offline_attestation_policy_for_release_activation(
        policy: &OfflineDeviceAttestationPolicy,
        block_unix_timestamp_ms: u64,
    ) -> Result<(), Error> {
        validate_offline_attestation_policy(policy, block_unix_timestamp_ms)?;
        if !policy.require_ios_app_policy
            || !policy.require_android_app_policy
            || policy.ios_apps.is_empty()
            || policy.android_apps.is_empty()
        {
            return Err(labeled_invariant(
                "invalid_attestation_policy",
                "Kagemusha release activation requires fail-closed iOS and Android app policies",
            )
            .into());
        }
        let platforms = policy
            .trusted_roots
            .iter()
            .map(|root| root.platform.as_str())
            .collect::<BTreeSet<_>>();
        if platforms
            != BTreeSet::from([
                OFFLINE_ATTESTATION_PLATFORM_ANDROID_KEYMINT,
                OFFLINE_ATTESTATION_PLATFORM_IOS_APP_ATTEST,
            ])
            || policy.trusted_roots.iter().any(|root| {
                root.der.len() > OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_TRUSTED_ROOT_DER_BYTES_V1
                    || root
                        .not_before_ms
                        .zip(root.not_after_ms)
                        .is_some_and(|(start, end)| start >= end)
            })
        {
            return Err(labeled_invariant(
                "invalid_attestation_policy",
                "Kagemusha release activation requires bounded trust roots for both production platforms",
            )
            .into());
        }
        for app in &policy.ios_apps {
            let sorted_categories = app
                .allowed_validation_categories
                .iter()
                .copied()
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>();
            let sorted_versions = app
                .allowed_bundle_versions
                .iter()
                .cloned()
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>();
            if app.environment != OFFLINE_ATTESTATION_IOS_ENV_PRODUCTION
                || app.allowed_validation_categories.is_empty()
                || app.allowed_bundle_versions.is_empty()
                || app.allowed_validation_categories != sorted_categories
                || app.allowed_bundle_versions != sorted_versions
            {
                return Err(labeled_invariant(
                    "invalid_attestation_policy",
                    "Kagemusha release activation requires canonical production iOS app policy",
                )
                .into());
            }
        }
        for app in &policy.android_apps {
            let sorted_signers = app
                .signing_certificate_sha256
                .iter()
                .cloned()
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>();
            if app.signing_certificate_sha256 != sorted_signers {
                return Err(labeled_invariant(
                    "invalid_attestation_policy",
                    "Kagemusha release activation requires canonical Android signing policy",
                )
                .into());
            }
        }
        Ok(())
    }
