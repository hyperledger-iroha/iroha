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
        policy.revoked_certificate_tbs_sha256.len(),
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
    for digest in &policy.revoked_certificate_tbs_sha256 {
        ensure_offline_attestation_policy_limit(
            digest.len(),
            32,
            "revoked-certificate TBS digest bytes",
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
    if let Some(snapshot) = &policy.android_status_snapshot {
        ensure_offline_attestation_policy_limit(
            snapshot.non_valid_serials.len(),
            iroha_data_model::offline::OFFLINE_ANDROID_ATTESTATION_STATUS_MAX_NON_VALID_SERIALS_V1,
            "Android attestation status serial count",
        )?;
        for serial in &snapshot.non_valid_serials {
            ensure_offline_attestation_policy_limit(
                    serial.len(),
                    iroha_data_model::offline::OFFLINE_ANDROID_ATTESTATION_STATUS_MAX_SERIAL_HEX_BYTES_V1,
                    "Android attestation status serial bytes",
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
fn android_attestation_status_snapshot_fresh_until_ms(
    snapshot: &iroha_data_model::offline::OfflineAndroidAttestationStatusSnapshotV1,
) -> Result<u64, Error> {
    if snapshot.version
        != iroha_data_model::offline::OFFLINE_ANDROID_ATTESTATION_STATUS_SNAPSHOT_VERSION_V1
        || snapshot.payload_sha256 == [0; 32]
        || snapshot.response_date_ms == 0
        || !snapshot.response_date_ms.is_multiple_of(1_000)
        || snapshot.cache_max_age_seconds == 0
        || snapshot.cache_max_age_seconds
            > iroha_data_model::offline::OFFLINE_ANDROID_ATTESTATION_STATUS_MAX_CACHE_AGE_SECONDS_V1
        || snapshot.last_modified_ms.is_some_and(|last_modified_ms| {
            last_modified_ms == 0
                || !last_modified_ms.is_multiple_of(1_000)
                || last_modified_ms > snapshot.response_date_ms
        })
    {
        return Err(labeled_invariant(
            "invalid_attestation_policy",
            "Android attestation status snapshot metadata is invalid",
        )
        .into());
    }
    let mut previous_serial: Option<&str> = None;
    for serial in &snapshot.non_valid_serials {
        if serial.is_empty()
                || serial.len()
                    > iroha_data_model::offline::OFFLINE_ANDROID_ATTESTATION_STATUS_MAX_SERIAL_HEX_BYTES_V1
                || !serial
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
                || (serial.len() > 1 && serial.starts_with('0'))
                || previous_serial.is_some_and(|previous| previous >= serial.as_str())
            {
                return Err(labeled_invariant(
                    "invalid_attestation_policy",
                    "Android attestation status serials must be sorted unique canonical lowercase hexadecimal values",
                )
                .into());
            }
        previous_serial = Some(serial);
    }
    snapshot
        .response_date_ms
        .checked_add(u64::from(snapshot.cache_max_age_seconds) * 1_000)
        .ok_or_else(|| {
            labeled_invariant(
                "invalid_attestation_policy",
                "Android attestation status freshness deadline overflows",
            )
            .into()
        })
}
fn validate_android_attestation_status_snapshot_at(
    snapshot: &iroha_data_model::offline::OfflineAndroidAttestationStatusSnapshotV1,
    evaluation_time_ms: u64,
) -> Result<u64, Error> {
    let fresh_until_ms = android_attestation_status_snapshot_fresh_until_ms(snapshot)?;
    if evaluation_time_ms < snapshot.response_date_ms || evaluation_time_ms >= fresh_until_ms {
        return Err(labeled_invariant(
            "invalid_attestation_policy",
            "Android attestation status snapshot is not fresh at the evaluation time",
        )
        .into());
    }
    Ok(fresh_until_ms)
}
fn validate_android_attestation_status_transition(
    previous: Option<&iroha_data_model::offline::OfflineAndroidAttestationStatusSnapshotV1>,
    candidate: Option<&iroha_data_model::offline::OfflineAndroidAttestationStatusSnapshotV1>,
) -> Result<(), Error> {
    let (previous, candidate) = match (previous, candidate) {
        (None, _) => return Ok(()),
        (Some(_), None) => {
            return Err(labeled_invariant(
                "invalid_attestation_policy",
                "Android attestation status anti-rollback state cannot be removed",
            )
            .into());
        }
        (Some(previous), Some(candidate)) => (previous, candidate),
    };
    if previous == candidate {
        return Ok(());
    }
    android_attestation_status_snapshot_fresh_until_ms(previous)?;
    android_attestation_status_snapshot_fresh_until_ms(candidate)?;
    if candidate.response_date_ms <= previous.response_date_ms
        || previous
            .last_modified_ms
            .zip(candidate.last_modified_ms)
            .is_some_and(|(previous, candidate)| candidate < previous)
        || (previous.last_modified_ms.is_some() && candidate.last_modified_ms.is_none())
        || (candidate.last_modified_ms == previous.last_modified_ms
            && (candidate.payload_sha256 != previous.payload_sha256
                || candidate.non_valid_serials != previous.non_valid_serials))
    {
        return Err(labeled_invariant(
                "invalid_attestation_policy",
                "Android attestation status snapshot transition is stale or rolls back authenticated upstream state",
            )
            .into());
    }
    Ok(())
}
fn validate_offline_attestation_policy_transition(
    previous: &OfflineDeviceAttestationPolicy,
    candidate: &OfflineDeviceAttestationPolicy,
) -> Result<(), Error> {
    validate_android_attestation_status_transition(
        previous.android_status_snapshot.as_ref(),
        candidate.android_status_snapshot.as_ref(),
    )
}
pub(super) fn validate_offline_attestation_policy_transition_from_state(
    candidate: &OfflineDeviceAttestationPolicy,
    state_transaction: &StateTransaction<'_, '_>,
) -> Result<(), Error> {
    let Some(previous_bytes) = state_transaction
        .world
        .smart_contract_state
        .get(&*OFFLINE_DEVICE_ATTESTATION_POLICY_STATE_KEY)
    else {
        return Ok(());
    };
    let previous = norito::decode_canonical::<OfflineDeviceAttestationPolicy>(previous_bytes)
        .map_err(|error| {
            labeled_invariant(
                "invalid_attestation_policy",
                format!("existing governed Offline device attestation policy is corrupt: {error}"),
            )
        })?;
    validate_offline_attestation_policy_transition(&previous, candidate)
}
pub(crate) fn validate_offline_attestation_policy_status_coverage(
    policy: &OfflineDeviceAttestationPolicy,
    exclusive_end_ms: u64,
) -> Result<(), Error> {
    if !policy.require_android_app_policy {
        return Ok(());
    }
    let snapshot = policy.android_status_snapshot.as_ref().ok_or_else(|| {
        labeled_invariant(
            "invalid_attestation_policy",
            "Android attestation requires a governed status snapshot",
        )
    })?;
    let fresh_until_ms = android_attestation_status_snapshot_fresh_until_ms(snapshot)?;
    if exclusive_end_ms > fresh_until_ms {
        return Err(labeled_invariant(
            "invalid_attestation_policy",
            "Android attestation status snapshot does not cover the complete validity window",
        )
        .into());
    }
    Ok(())
}
fn normalize_policy_ascii(value: &str, field: &str) -> Result<String, Error> {
    let trimmed = value.trim();
    if trimmed.is_empty() || !trimmed.is_ascii() || trimmed.chars().any(char::is_control) {
        return Err(labeled_invariant(
            "invalid_attestation_policy",
            format!("Offline device attestation policy {field} must be non-empty printable ASCII"),
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
            format!("Offline device attestation policy {field} must be a 32-byte SHA-256 digest"),
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
    for digest in &policy.revoked_certificate_tbs_sha256 {
        let digest = normalize_sha256_digest(digest, "revoked certificate TBS digest")?;
        if digest == [0u8; 32] || !revoked.insert(digest) {
            return Err(labeled_invariant(
                "invalid_attestation_policy",
                "Offline device attestation policy has an invalid revoked certificate TBS digest",
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
        if app.allowed_validation_categories.is_empty() || app.allowed_bundle_versions.is_empty() {
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
    match &policy.android_status_snapshot {
        Some(snapshot) => {
            validate_android_attestation_status_snapshot_at(snapshot, block_unix_timestamp_ms)?;
        }
        None if policy.require_android_app_policy => {
            return Err(labeled_invariant(
                "invalid_attestation_policy",
                "Android attestation requires a governed status snapshot",
            )
            .into());
        }
        None => {}
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
    let ios_roots = policy
        .trusted_roots
        .iter()
        .filter(|root| root.platform == OFFLINE_ATTESTATION_PLATFORM_IOS_APP_ATTEST)
        .map(|root| root.der.clone())
        .collect::<Vec<_>>();
    let expected_ios_roots = vec![decode_trusted_root_der(
        APPLE_APP_ATTESTATION_ROOT_CA_DER_B64,
    )?];
    if ios_roots != expected_ios_roots {
        return Err(labeled_invariant(
            "invalid_attestation_policy",
            "Kagemusha release activation requires the exact current Apple App Attest root",
        )
        .into());
    }
    let mut android_roots = policy
        .trusted_roots
        .iter()
        .filter(|root| root.platform == OFFLINE_ATTESTATION_PLATFORM_ANDROID_KEYMINT)
        .map(|root| root.der.clone())
        .collect::<Vec<_>>();
    android_roots.sort_unstable();
    let mut expected_android_roots = vec![
        decode_trusted_root_der(ANDROID_KEY_ATTESTATION_ROOT_CA_DER_B64)?,
        decode_trusted_root_der(ANDROID_KEY_ATTESTATION_CA_DER_B64)?,
    ];
    expected_android_roots.sort_unstable();
    if android_roots != expected_android_roots {
        return Err(labeled_invariant(
                "invalid_attestation_policy",
                "Kagemusha release activation requires the exact current Google Android attestation roots",
            )
            .into());
    }
    if policy
        .trusted_roots
        .iter()
        .any(|root| !trusted_root_is_active(root, block_unix_timestamp_ms))
    {
        return Err(labeled_invariant(
                "invalid_attestation_policy",
                "Kagemusha release activation requires every exact production attestation root to be governance-active",
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

#[cfg(test)]
mod android_attestation_status_policy_tests {
    use super::*;

    const RESPONSE_DATE_MS: u64 = 1_800_000_000_000;

    fn snapshot() -> iroha_data_model::offline::OfflineAndroidAttestationStatusSnapshotV1 {
        iroha_data_model::offline::OfflineAndroidAttestationStatusSnapshotV1 {
            version:
                iroha_data_model::offline::OFFLINE_ANDROID_ATTESTATION_STATUS_SNAPSHOT_VERSION_V1,
            payload_sha256: [0x5a; 32],
            response_date_ms: RESPONSE_DATE_MS,
            last_modified_ms: Some(RESPONSE_DATE_MS - 1_000),
            cache_max_age_seconds: 60,
            non_valid_serials: vec!["1".to_owned(), "a0".to_owned()],
        }
    }

    #[test]
    fn android_status_freshness_boundaries_are_exact() {
        let snapshot = snapshot();
        let fresh_until_ms = RESPONSE_DATE_MS + 60_000;
        validate_android_attestation_status_snapshot_at(&snapshot, RESPONSE_DATE_MS)
            .expect("the upstream response instant is fresh");
        validate_android_attestation_status_snapshot_at(&snapshot, fresh_until_ms - 1)
            .expect("the final millisecond before expiry is fresh");
        assert!(
            validate_android_attestation_status_snapshot_at(&snapshot, RESPONSE_DATE_MS - 1)
                .is_err()
        );
        assert!(
            validate_android_attestation_status_snapshot_at(&snapshot, fresh_until_ms).is_err()
        );
    }

    #[test]
    fn android_status_snapshot_shape_is_canonical() {
        let baseline = snapshot();
        android_attestation_status_snapshot_fresh_until_ms(&baseline).expect("canonical snapshot");
        for serials in [
            vec!["01".to_owned()],
            vec!["A0".to_owned()],
            vec!["a0".to_owned(), "1".to_owned()],
            vec!["a0".to_owned(), "a0".to_owned()],
            vec!["g0".to_owned()],
        ] {
            let mut candidate = baseline.clone();
            candidate.non_valid_serials = serials;
            assert!(android_attestation_status_snapshot_fresh_until_ms(&candidate).is_err());
        }
        let mut candidate = baseline.clone();
        candidate.payload_sha256 = [0; 32];
        assert!(android_attestation_status_snapshot_fresh_until_ms(&candidate).is_err());
        let mut candidate = baseline.clone();
        candidate.response_date_ms += 1;
        assert!(android_attestation_status_snapshot_fresh_until_ms(&candidate).is_err());
        let mut candidate = baseline.clone();
        candidate.last_modified_ms = Some(RESPONSE_DATE_MS + 1_000);
        assert!(android_attestation_status_snapshot_fresh_until_ms(&candidate).is_err());
        let mut candidate = baseline;
        candidate.cache_max_age_seconds = 0;
        assert!(android_attestation_status_snapshot_fresh_until_ms(&candidate).is_err());
    }

    #[test]
    fn android_status_transition_prevents_rollback_but_allows_authenticated_shrink() {
        let previous = snapshot();
        validate_android_attestation_status_transition(Some(&previous), Some(&previous))
            .expect("an identical update is idempotent");

        let mut refreshed = previous.clone();
        refreshed.response_date_ms += 1_000;
        refreshed.last_modified_ms = Some(RESPONSE_DATE_MS);
        refreshed.payload_sha256 = [0x6b; 32];
        refreshed.non_valid_serials = vec!["a0".to_owned()];
        validate_android_attestation_status_transition(Some(&previous), Some(&refreshed))
            .expect("a newer authenticated list may shrink");
        assert!(
            validate_android_attestation_status_transition(Some(&previous), None).is_err(),
            "an update cannot erase the anti-rollback watermark before re-enabling Android",
        );

        let mut same_response = refreshed.clone();
        same_response.response_date_ms = previous.response_date_ms;
        assert!(
            validate_android_attestation_status_transition(Some(&previous), Some(&same_response))
                .is_err()
        );
        let mut removed_last_modified = refreshed.clone();
        removed_last_modified.last_modified_ms = None;
        assert!(
            validate_android_attestation_status_transition(
                Some(&previous),
                Some(&removed_last_modified)
            )
            .is_err()
        );
        let mut changed_under_same_last_modified = previous.clone();
        changed_under_same_last_modified.response_date_ms += 1_000;
        changed_under_same_last_modified.payload_sha256 = [0x7c; 32];
        assert!(
            validate_android_attestation_status_transition(
                Some(&previous),
                Some(&changed_under_same_last_modified)
            )
            .is_err()
        );
    }

    #[test]
    fn android_status_must_cover_the_exclusive_validity_window() {
        let mut policy =
            default_offline_device_attestation_policy().expect("built-in policy template");
        policy.require_android_app_policy = true;
        assert!(
            validate_offline_attestation_policy_status_coverage(&policy, RESPONSE_DATE_MS).is_err()
        );
        policy.android_status_snapshot = Some(snapshot());
        validate_offline_attestation_policy_status_coverage(&policy, RESPONSE_DATE_MS + 60_000)
            .expect("freshness through the exclusive endpoint is sufficient");
        assert!(
            validate_offline_attestation_policy_status_coverage(&policy, RESPONSE_DATE_MS + 60_001)
                .is_err()
        );
    }
}
