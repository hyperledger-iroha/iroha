// Release-scoped device-policy authentication for Kagemusha V4 redemption.

/// Preflight the ledger-resolved hardware identity shared by top-up and redemption.
///
/// This read-only boundary is intended for Torii before it sponsors an Offline
/// command with its escrow-manager authority. It verifies the exact protected
/// registration, request identity, platform lifecycle, and hardware signature.
/// Consensus execution still performs the complete policy, replay, counter,
/// and proof checks against its transactional snapshot.
///
/// # Errors
///
/// Returns an error when the authorization cannot be authenticated against the
/// protected registration visible in `world` at `evaluated_at_ms`.
pub fn preflight_registered_kagemusha_v2_hardware_authorization(
    world: &impl WorldReadOnly,
    authorization: &KagemushaRequestAuthorizationV2,
    asset: &AssetDefinitionId,
    evaluated_at_ms: u64,
) -> Result<(), String> {
    if evaluated_at_ms == 0 || &authorization.asset_definition_id != asset {
        return Err("Kagemusha hardware authorization has an invalid snapshot or asset".to_owned());
    }
    let state_key = kagemusha_online_registration_state_key(&authorization.registration_hash)
        .map_err(|error| format!("Kagemusha registration key is invalid: {error}"))?;
    let archive = world
        .smart_contract_state()
        .get(&state_key)
        .ok_or_else(|| {
            "Kagemusha hardware authorization references an unknown registration".to_owned()
        })?;
    if archive.len() > KAGEMUSHA_ONLINE_REGISTRATION_STATE_MAX_CANONICAL_BYTES_V4 {
        return Err("protected Kagemusha registration exceeds the protocol byte limit".to_owned());
    }
    let state = decode_kagemusha_online_registration_state_v4(&state_key, archive)
        .map_err(|error| format!("protected Kagemusha registration is invalid: {error}"))?;
    let registration = &state.registration;
    if state.original_registration_hash != authorization.registration_hash
        || registration.account_id != authorization.authority
        || registration.device_id != authorization.device_id
        || registration.asset_definition_id.as_ref() != Some(asset)
        || authorization.expires_at_ms > registration.expires_at_ms
        || registration.expires_at_ms <= evaluated_at_ms
    {
        return Err(
            "Kagemusha authorization identity or expiry does not match its registration"
                .to_owned(),
        );
    }
    validate_offline_attestation_platform_profile(registration)
        .map_err(|error| error.to_string())?;
    validate_offline_attestation_optional_metadata(registration)
        .map_err(|error| error.to_string())?;
    match (&authorization.hardware_assertion, &state.lifecycle) {
        (
            KagemushaOnlineHardwareAssertionV1::AndroidKeyMint(_),
            KagemushaOnlineHardwareAssertionLifecycleV1::AndroidKeyMintUnused,
        ) if registration.platform == OFFLINE_ATTESTATION_PLATFORM_ANDROID_KEYMINT => {}
        (
            KagemushaOnlineHardwareAssertionV1::AndroidKeyMint(_),
            KagemushaOnlineHardwareAssertionLifecycleV1::AndroidKeyMintConsumed(_),
        ) if registration.platform == OFFLINE_ATTESTATION_PLATFORM_ANDROID_KEYMINT => {
            return Err(
                "Kagemusha Android hardware authorization has already been consumed".to_owned(),
            );
        }
        (
            KagemushaOnlineHardwareAssertionV1::IosAppAttest(assertion),
            KagemushaOnlineHardwareAssertionLifecycleV1::IosAppAttest {
                last_sign_count, ..
            },
        ) if registration.platform == OFFLINE_ATTESTATION_PLATFORM_IOS_APP_ATTEST => {
            let (team_id, bundle_id, _) =
                ios_attestation_metadata(registration).map_err(|error| error.to_string())?;
            let authenticator_data = parse_ios_app_attest_assertion_auth_data(
                &assertion.authenticator_data,
            )
            .map_err(|error| error.to_string())?;
            let expected_rp_id_hash = sha256_bytes(format!("{team_id}.{bundle_id}").as_bytes());
            validate_ios_app_attest_assertion_identity(&authenticator_data, expected_rp_id_hash)
                .map_err(|error| error.to_string())?;
            if authenticator_data.sign_count <= *last_sign_count {
                return Err(
                    "Kagemusha iOS hardware authorization counter does not advance".to_owned(),
                );
            }
        }
        _ => {
            return Err(
                "Kagemusha authorization platform does not match its registration lifecycle"
                    .to_owned(),
            );
        }
    }
    authorization
        .verify_hardware_signature(&registration.assertion_public_key)
        .map_err(|error| error.to_string())
}

fn ensure_redemption_registration_policy_compatibility(
    registration: &OfflineDeviceAttestationRegistration,
    release_policy: &OfflineDeviceAttestationPolicy,
    admission_policy: &OfflineDeviceAttestationPolicy,
) -> Result<(), Error> {
    let release_roots = release_policy
        .trusted_roots
        .iter()
        .filter(|root| root.platform == registration.platform)
        .collect::<BTreeSet<_>>();
    let admission_roots = admission_policy
        .trusted_roots
        .iter()
        .filter(|root| root.platform == registration.platform)
        .collect::<BTreeSet<_>>();
    let release_revocations = release_policy
        .revoked_certificate_tbs_sha256
        .iter()
        .collect::<BTreeSet<_>>();
    let admission_revocations = admission_policy
        .revoked_certificate_tbs_sha256
        .iter()
        .collect::<BTreeSet<_>>();
    if release_roots.is_empty()
        || release_roots != admission_roots
        || !release_revocations.is_subset(&admission_revocations)
    {
        return Err(labeled_invariant(
            "attestation_policy_changed",
            "the current registration was not admitted under a trust basis at least as strict as the release-scoped redemption policy",
        )
        .into());
    }
    if registration.platform == OFFLINE_ATTESTATION_PLATFORM_ANDROID_KEYMINT {
        let release_snapshot =
            release_policy
                .android_status_snapshot
                .as_ref()
                .ok_or_else(|| {
                    labeled_invariant(
                        "attestation_policy_changed",
                        "the release-scoped Android policy has no authenticated status snapshot",
                    )
                })?;
        let admission_snapshot = admission_policy
            .android_status_snapshot
            .as_ref()
            .ok_or_else(|| {
                labeled_invariant(
                    "attestation_policy_changed",
                    "the current Android registration policy has no authenticated status snapshot",
                )
            })?;
        validate_android_attestation_status_transition(
            Some(release_snapshot),
            Some(admission_snapshot),
        )?;
        let release_non_valid = release_snapshot
            .non_valid_serials
            .iter()
            .collect::<BTreeSet<_>>();
        let admission_non_valid = admission_snapshot
            .non_valid_serials
            .iter()
            .collect::<BTreeSet<_>>();
        if !release_non_valid.is_subset(&admission_non_valid) {
            return Err(labeled_invariant(
                "attestation_policy_changed",
                "the current Android status snapshot no longer rejects every release-scoped non-valid certificate serial",
            )
            .into());
        }
    }
    Ok(())
}

fn ensure_existing_release_registration_trust_is_unchanged(
    registration: &OfflineDeviceAttestationRegistration,
    release_policy: &OfflineDeviceAttestationPolicy,
    current_policy: &OfflineDeviceAttestationPolicy,
) -> Result<(), Error> {
    let release_revocations = release_policy
        .revoked_certificate_tbs_sha256
        .iter()
        .collect::<BTreeSet<_>>();
    let current_revocations = current_policy
        .revoked_certificate_tbs_sha256
        .iter()
        .collect::<BTreeSet<_>>();
    let android_status_changed = registration.platform
        == OFFLINE_ATTESTATION_PLATFORM_ANDROID_KEYMINT
        && release_policy.android_status_snapshot != current_policy.android_status_snapshot;
    if release_revocations != current_revocations || android_status_changed {
        return Err(labeled_invariant(
            "attestation_policy_changed",
            "the live trust or status policy changed after this release-scoped registration; the device must register again",
        )
        .into());
    }
    Ok(())
}

fn authenticate_registered_kagemusha_v2_device_against_policy(
    authorization: &KagemushaRequestAuthorizationV2,
    asset: &AssetDefinitionId,
    release_policy: &OfflineDeviceAttestationPolicy,
    state_transaction: &StateTransaction<'_, '_>,
) -> Result<KagemushaAuthenticatedDeviceV1, Error> {
    if &authorization.asset_definition_id != asset {
        return Err(labeled_invariant(
            "invalid_authorization",
            "Kagemusha hardware authorization asset does not match the operation asset",
        )
        .into());
    }
    let state_key = kagemusha_online_registration_state_key(&authorization.registration_hash)?;
    let previous_archive = state_transaction
        .world
        .smart_contract_state
        .get(&state_key)
        .ok_or_else(|| {
            labeled_invariant(
                "device_not_registered",
                "Kagemusha hardware authorization references an unknown registration hash",
            )
        })?;
    if previous_archive.len() > KAGEMUSHA_ONLINE_REGISTRATION_STATE_MAX_CANONICAL_BYTES_V4 {
        return Err(labeled_invariant(
            "invalid_attestation",
            "persisted Kagemusha registration exceeds the protocol byte limit",
        )
        .into());
    }
    let previous_archive = previous_archive.clone();
    let state = decode_kagemusha_online_registration_state_v4(&state_key, &previous_archive)
        .map_err(|err| {
            labeled_invariant(
                "invalid_attestation",
                format!("failed to validate persisted Kagemusha registration: {err}"),
            )
        })?;
    if state.original_registration_hash != authorization.registration_hash {
        return Err(labeled_invariant(
            "invalid_attestation",
            "persisted Kagemusha registration is non-canonical, corrupt, or keyed incorrectly",
        )
        .into());
    }
    let registration = &state.registration;
    if registration.account_id != authorization.authority
        || registration.device_id != authorization.device_id
        || registration.asset_definition_id.as_ref() != Some(asset)
        || authorization.expires_at_ms > registration.expires_at_ms
        || registration.expires_at_ms <= state_transaction.block_unix_timestamp_ms()
    {
        return Err(labeled_invariant(
            "invalid_attestation",
            "Kagemusha authorization account, device, asset, or expiry does not match its registration",
        )
        .into());
    }
    validate_offline_attestation_platform_profile(registration)?;
    validate_offline_attestation_optional_metadata(registration)?;
    let release_policy_hash = canonical_offline_device_attestation_policy_hash(release_policy)?;
    let current_policy = effective_offline_device_attestation_policy(state_transaction)?;
    validate_offline_attestation_policy(
        &current_policy,
        state_transaction.block_unix_timestamp_ms(),
    )?;
    let current_policy_hash = canonical_offline_device_attestation_policy_hash(&current_policy)?;
    if state.admission_policy_hash != release_policy_hash
        && state.admission_policy_hash != current_policy_hash
    {
        return Err(labeled_invariant(
            "attestation_policy_changed",
            "Offline device attestation policy changed after registration; the device must register again",
        )
        .into());
    }
    if current_policy_hash != release_policy_hash {
        let historical_validation_time_ms = release_policy
            .android_status_snapshot
            .as_ref()
            .map_or(state_transaction.block_unix_timestamp_ms(), |snapshot| {
                snapshot.response_date_ms
            });
        validate_offline_attestation_policy(release_policy, historical_validation_time_ms)?;
        ensure_redemption_registration_policy_compatibility(
            registration,
            release_policy,
            &current_policy,
        )?;
        if state.admission_policy_hash == release_policy_hash {
            ensure_existing_release_registration_trust_is_unchanged(
                registration,
                release_policy,
                &current_policy,
            )?;
        }
    }
    let assertion = match (&authorization.hardware_assertion, &state.lifecycle) {
        (
            KagemushaOnlineHardwareAssertionV1::AndroidKeyMint(_),
            KagemushaOnlineHardwareAssertionLifecycleV1::AndroidKeyMintUnused
            | KagemushaOnlineHardwareAssertionLifecycleV1::AndroidKeyMintConsumed(_),
        ) if registration.platform == OFFLINE_ATTESTATION_PLATFORM_ANDROID_KEYMINT => {
            let (package_name, signing_digest) = android_attestation_metadata(registration)?;
            ensure_android_app_allowed_by_policy(release_policy, &package_name, &signing_digest)?;
            authorization
                .verify_hardware_signature(&registration.assertion_public_key)
                .map_err(|err| labeled_invariant("invalid_authorization", err.to_string()))?;
            KagemushaAuthenticatedHardwareAssertionV1::AndroidKeyMint
        }
        (
            KagemushaOnlineHardwareAssertionV1::IosAppAttest(assertion),
            KagemushaOnlineHardwareAssertionLifecycleV1::IosAppAttest { .. },
        ) if registration.platform == OFFLINE_ATTESTATION_PLATFORM_IOS_APP_ATTEST => {
            let (team_id, bundle_id, environment) = ios_attestation_metadata(registration)?;
            let app_policy = ensure_ios_app_allowed_by_policy(
                release_policy,
                &team_id,
                &bundle_id,
                &environment,
            )?;
            let authenticator_data =
                parse_ios_app_attest_assertion_auth_data(&assertion.authenticator_data)?;
            validate_ios_app_attest_extensions_against_policy(
                app_policy,
                &authenticator_data.extensions,
            )?;
            let expected_rp_id_hash = sha256_bytes(format!("{team_id}.{bundle_id}").as_bytes());
            validate_ios_app_attest_assertion_identity(&authenticator_data, expected_rp_id_hash)?;
            authorization
                .verify_hardware_signature(&registration.assertion_public_key)
                .map_err(|err| labeled_invariant("invalid_authorization", err.to_string()))?;
            KagemushaAuthenticatedHardwareAssertionV1::IosAppAttest {
                sign_count: authenticator_data.sign_count,
            }
        }
        _ => {
            return Err(labeled_invariant(
                "invalid_attestation",
                "Kagemusha authorization platform does not match its persisted registration lifecycle",
            )
            .into());
        }
    };
    Ok(KagemushaAuthenticatedDeviceV1 {
        state_key,
        previous_archive,
        state,
        consumption: assertion_consumption(authorization)?,
        assertion,
    })
}

fn authenticate_registered_kagemusha_v2_device(
    authorization: &KagemushaRequestAuthorizationV2,
    asset: &AssetDefinitionId,
    state_transaction: &StateTransaction<'_, '_>,
) -> Result<KagemushaAuthenticatedDeviceV1, Error> {
    let policy = effective_offline_device_attestation_policy(state_transaction)?;
    authenticate_registered_kagemusha_v2_device_against_policy(
        authorization,
        asset,
        &policy,
        state_transaction,
    )
}
