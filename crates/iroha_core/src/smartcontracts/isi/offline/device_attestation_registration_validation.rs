fn validate_offline_attestation_registration_identifiers(
    registration: &OfflineDeviceAttestationRegistration,
) -> Result<(), Error> {
    for (field, value) in [
        ("platform", registration.platform.as_str()),
        ("key_id", registration.key_id.as_str()),
        ("device_id", registration.device_id.as_str()),
        ("assertion_scheme", registration.assertion_scheme.as_str()),
        (
            "assertion_key_algorithm",
            registration.assertion_key_algorithm.as_str(),
        ),
    ] {
        validate_attestation_protocol_string(
            "offline device attestation",
            field,
            value,
            "invalid_attestation",
        )
        .map_err(Error::from)?;
    }
    if registration.key_id.len() > OFFLINE_DEVICE_ATTESTATION_KEY_ID_MAX_BYTES_V1 {
        return Err(labeled_invariant(
            "invalid_attestation",
            format!(
                "offline device attestation key_id exceeds the first-release limit of {} bytes",
                OFFLINE_DEVICE_ATTESTATION_KEY_ID_MAX_BYTES_V1
            ),
        )
        .into());
    }
    if registration.device_id.len() > OFFLINE_DEVICE_ATTESTATION_DEVICE_ID_MAX_BYTES_V1
        || registration.device_id.chars().any(char::is_control)
    {
        return Err(labeled_invariant(
            "invalid_attestation",
            format!(
                "offline device attestation device_id must contain at most {} bytes and no control characters",
                OFFLINE_DEVICE_ATTESTATION_DEVICE_ID_MAX_BYTES_V1
            ),
        )
        .into());
    }
    Ok(())
}

fn validate_offline_device_attestation_registration(
    registration: &OfflineDeviceAttestationRegistration,
    authority: &AccountId,
    state_transaction: &StateTransaction<'_, '_>,
) -> Result<(Hash, [u8; 32]), Error> {
    ensure_can_submit_kagemusha_for_account(
        &registration.account_id,
        authority,
        state_transaction,
    )?;
    if registration.version != 2 {
        return Err(labeled_invariant(
            "invalid_attestation",
            "offline device attestation registration version is unsupported",
        )
        .into());
    }
    validate_offline_attestation_registration_identifiers(registration)?;
    if registration.assertion_public_key.is_empty() {
        return Err(labeled_invariant(
            "invalid_attestation",
            "offline device attestation assertion public key must be non-empty",
        )
        .into());
    }
    if is_zero_hash(&registration.challenge_hash)
        || is_zero_hash(&registration.attestation_report_hash)
        || is_zero_hash(&registration.evidence_hash)
        || is_zero_hash(&registration.recent_block_hash)
    {
        return Err(labeled_invariant(
            "invalid_attestation",
            "offline device attestation hashes must be non-zero",
        )
        .into());
    }
    validate_offline_attestation_platform_profile(registration)?;
    validate_offline_attestation_optional_metadata(registration)?;
    validate_offline_attestation_evidence_bytes(registration)?;
    let expected_challenge_hash = registration.canonical_challenge_hash().map_err(|err| {
        labeled_invariant(
            "invalid_attestation",
            format!("failed to encode Offline attestation challenge preimage: {err}"),
        )
    })?;
    if registration.challenge_hash != expected_challenge_hash {
        return Err(labeled_invariant(
            "invalid_attestation",
            "offline device attestation challenge hash does not match the canonical preimage",
        )
        .into());
    }
    if registration.expires_at_ms <= state_transaction.block_unix_timestamp_ms() {
        return Err(labeled_invariant(
            "expired_attestation",
            "offline device attestation registration is expired",
        )
        .into());
    }
    let policy = effective_offline_device_attestation_policy(state_transaction)?;
    let admitted_at_ms = state_transaction.block_unix_timestamp_ms();
    validate_offline_attestation_policy(&policy, admitted_at_ms)?;
    let lifetime_policy = offline_attestation_policy_for_registration_lifetime(
        &policy,
        &registration.platform,
        admitted_at_ms,
        registration.expires_at_ms,
    )?;
    validate_offline_attestation_policy(&lifetime_policy, admitted_at_ms)?;
    validate_offline_attestation_policy_status_coverage(
        &lifetime_policy,
        registration.expires_at_ms,
    )?;
    validate_offline_attestation_recent_block(registration, state_transaction)?;
    validate_offline_attestation_report(registration, &lifetime_policy, admitted_at_ms)?;
    // Admission must cover the registration's entire lifetime. Certificate
    // validity and governed root activation are continuous time ranges, so
    // validating both endpoints prevents a registration from surviving
    // beyond either bound without repeating X.509 verification on every use.
    let last_valid_ms = registration.expires_at_ms.saturating_sub(1);
    validate_offline_attestation_policy(&lifetime_policy, last_valid_ms)?;
    validate_offline_attestation_report(registration, &lifetime_policy, last_valid_ms)?;
    let bytes = norito::encode_canonical(registration).map_err(|err| {
        labeled_invariant(
            "invalid_attestation",
            format!("failed to encode Kagemusha device registration: {err}"),
        )
    })?;
    Ok((
        Hash::new(bytes),
        canonical_offline_device_attestation_policy_hash(&policy)?,
    ))
}
