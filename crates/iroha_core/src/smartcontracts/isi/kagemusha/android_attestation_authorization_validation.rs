// Strict Android KeyMint AuthorizationList and application-identity validation.

fn validate_android_attestation_application_id_matches(
    application_id: &AndroidAttestationApplicationId,
    package_name: &str,
    signing_digest: &[u8; 32],
) -> Result<(), Error> {
    if application_id.packages.len() != 1 || application_id.packages[0].package_name != package_name
    {
        return Err(labeled_invariant(
            "invalid_attestation",
            "Android KeyMint attestation application id must bind exactly the registered package",
        )
        .into());
    }
    if application_id.signature_digests.len() != 1
        || application_id.signature_digests[0].as_slice() != signing_digest
    {
        return Err(labeled_invariant(
            "invalid_attestation",
            "Android KeyMint attestation application id must bind exactly the registered signing digest",
        )
        .into());
    }
    Ok(())
}

fn validate_der_set_element_order<'a>(
    previous: &mut Option<&'a [u8]>,
    current: &'a [u8],
    message: &str,
) -> Result<(), Error> {
    if previous.is_some_and(|previous| previous > current) {
        return Err(labeled_invariant("invalid_attestation", message.to_owned()).into());
    }
    *previous = Some(current);
    Ok(())
}

fn parse_android_attestation_application_id(
    input: &[u8],
) -> Result<AndroidAttestationApplicationId, Error> {
    let mut reader = DerReader::sequence(input)?;
    let package_set = reader.read_expected(0x31)?;
    let signature_set = reader.read_expected(0x31)?;
    if reader.has_remaining() {
        return Err(labeled_invariant(
            "invalid_attestation",
            "Android KeyMint attestation application id has trailing bytes",
        )
        .into());
    }
    let mut packages = Vec::new();
    let mut seen_packages = HashSet::new();
    let mut package_reader = DerReader::new(package_set);
    let mut previous_package_der = None;
    while package_reader.has_remaining() {
        let (tag, package_der, raw_package_der) = package_reader.read_tlv_full_with_raw()?;
        if tag.first_byte != 0x30 {
            return Err(labeled_invariant(
                "invalid_attestation",
                "attestation extension DER has an unexpected DER tag",
            )
            .into());
        }
        validate_der_set_element_order(
            &mut previous_package_der,
            raw_package_der,
            "Android KeyMint attestation package SET elements are not DER sorted",
        )?;
        let mut info_reader = DerReader::new(package_der);
        let package_name_bytes = info_reader.read_octet_string()?;
        let _version = info_reader.read_integer()?;
        if info_reader.has_remaining() {
            return Err(labeled_invariant(
                "invalid_attestation",
                "Android KeyMint attestation package info has trailing bytes",
            )
            .into());
        }
        let package_name = String::from_utf8(package_name_bytes).map_err(|_| {
            labeled_invariant(
                "invalid_attestation",
                "Android KeyMint attestation package name must be UTF-8",
            )
        })?;
        if package_name.trim().is_empty() {
            return Err(labeled_invariant(
                "invalid_attestation",
                "Android KeyMint attestation package name must be non-empty",
            )
            .into());
        }
        if !seen_packages.insert(package_name.clone()) {
            return Err(labeled_invariant(
                "invalid_attestation",
                "Android KeyMint attestation application id duplicates a package name",
            )
            .into());
        }
        packages.push(AndroidAttestationPackageInfo { package_name });
    }
    let mut signature_digests = Vec::new();
    let mut seen_signature_digests = HashSet::new();
    let mut signature_reader = DerReader::new(signature_set);
    let mut previous_signature_der = None;
    while signature_reader.has_remaining() {
        let (tag, digest, raw_signature_der) = signature_reader.read_tlv_full_with_raw()?;
        if tag.first_byte != 0x04 {
            return Err(labeled_invariant(
                "invalid_attestation",
                "attestation extension DER has an unexpected DER tag",
            )
            .into());
        }
        validate_der_set_element_order(
            &mut previous_signature_der,
            raw_signature_der,
            "Android KeyMint attestation signing-digest SET elements are not DER sorted",
        )?;
        if digest.len() != 32 {
            return Err(labeled_invariant(
                "invalid_attestation",
                "Android KeyMint attestation signing digest must be 32 bytes",
            )
            .into());
        }
        let mut digest_array = [0u8; 32];
        digest_array.copy_from_slice(digest);
        if !seen_signature_digests.insert(digest_array) {
            return Err(labeled_invariant(
                "invalid_attestation",
                "Android KeyMint attestation application id duplicates a signing digest",
            )
            .into());
        }
        signature_digests.push(digest.to_vec());
    }
    if packages.is_empty() || signature_digests.is_empty() {
        return Err(labeled_invariant(
            "invalid_attestation",
            "Android KeyMint attestation application id must include packages and signing digests",
        )
        .into());
    }
    Ok(AndroidAttestationApplicationId {
        packages,
        signature_digests,
    })
}

fn validate_android_root_of_trust(input: &[u8]) -> Result<(), Error> {
    let mut reader = DerReader::sequence(input)?;
    let verified_boot_key = reader.read_octet_string()?;
    let device_locked = match reader.read_expected(0x01)? {
        [0x00] => false,
        [0xFF] => true,
        _ => {
            return Err(labeled_invariant(
                "invalid_attestation",
                "Android KeyMint rootOfTrust deviceLocked must be a canonical DER boolean",
            )
            .into());
        }
    };
    let verified_boot_state = reader.read_enumerated()?;
    let verified_boot_hash = reader.read_octet_string()?;
    if reader.has_remaining() {
        return Err(labeled_invariant(
            "invalid_attestation",
            "Android KeyMint rootOfTrust has trailing fields",
        )
        .into());
    }
    if verified_boot_key.is_empty() {
        return Err(labeled_invariant(
            "invalid_attestation",
            "Android KeyMint rootOfTrust verifiedBootKey must be non-empty",
        )
        .into());
    }
    if !device_locked {
        return Err(labeled_invariant(
            "invalid_attestation",
            "Android KeyMint rootOfTrust requires deviceLocked=true",
        )
        .into());
    }
    if verified_boot_state != KAGEMUSHA_ATTESTATION_ANDROID_VERIFIED_BOOT_STATE_VERIFIED {
        return Err(labeled_invariant(
            "invalid_attestation",
            "Android KeyMint rootOfTrust requires verifiedBootState=Verified",
        )
        .into());
    }
    if verified_boot_hash.len() != 32 {
        return Err(labeled_invariant(
            "invalid_attestation",
            "Android KeyMint rootOfTrust verifiedBootHash must be 32 bytes",
        )
        .into());
    }
    Ok(())
}

fn parse_android_authorization_list(
    input: &[u8],
    hardware_enforced: bool,
) -> Result<
    (
        Option<i64>,
        bool,
        Option<AndroidAttestationApplicationId>,
        bool,
    ),
    Error,
> {
    let mut reader = DerReader::new(input);
    let mut usage_count_limit = None;
    let mut all_applications = false;
    let mut application_id = None;
    let mut root_of_trust = false;
    let mut seen_tags = HashSet::new();
    while reader.has_remaining() {
        let (tag, value) = reader.read_tlv_full()?;
        if tag.class_bits != 0x80 || !tag.constructed {
            return Err(labeled_invariant(
                "invalid_attestation",
                "Android KeyMint authorization list contains an invalid tag",
            )
            .into());
        }
        if !seen_tags.insert(tag.number) {
            return Err(labeled_invariant(
                "invalid_attestation",
                "Android KeyMint authorization list duplicates a context tag",
            )
            .into());
        }
        match tag.number {
            KAGEMUSHA_ATTESTATION_ANDROID_TAG_USAGE_COUNT_LIMIT => {
                usage_count_limit = Some(der_single_integer(value)?);
            }
            KAGEMUSHA_ATTESTATION_ANDROID_TAG_ALL_APPLICATIONS => {
                let mut null_reader = DerReader::new(value);
                null_reader.read_null()?;
                all_applications = true;
            }
            KAGEMUSHA_ATTESTATION_ANDROID_TAG_ROOT_OF_TRUST => {
                if !hardware_enforced {
                    return Err(labeled_invariant(
                        "invalid_attestation",
                        "Android KeyMint rootOfTrust must be hardwareEnforced",
                    )
                    .into());
                }
                validate_android_root_of_trust(value)?;
                root_of_trust = true;
            }
            KAGEMUSHA_ATTESTATION_ANDROID_TAG_ATTESTATION_APPLICATION_ID => {
                let app_id_der = der_single_octet_string(value)?;
                application_id = Some(parse_android_attestation_application_id(&app_id_der)?);
            }
            _ => {}
        }
    }
    Ok((
        usage_count_limit,
        all_applications,
        application_id,
        root_of_trust,
    ))
}
