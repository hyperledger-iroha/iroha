fn trusted_root_der_for_platform(
    policy: &OfflineDeviceAttestationPolicy,
    platform: &str,
    block_unix_timestamp_ms: u64,
) -> Result<Vec<Vec<u8>>, Error> {
    let roots: Vec<_> = policy
        .trusted_roots
        .iter()
        .filter(|root| {
            root.platform == platform && trusted_root_is_active(root, block_unix_timestamp_ms)
        })
        .map(|root| root.der.clone())
        .collect();
    if roots.is_empty() {
        return Err(labeled_invariant(
            "invalid_attestation_policy",
            "Offline device attestation policy has no active trusted root for platform",
        )
        .into());
    }
    Ok(roots)
}
fn policy_revoked_certificate_tbs_hashes(
    policy: &OfflineDeviceAttestationPolicy,
) -> Result<HashSet<[u8; 32]>, Error> {
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
    Ok(revoked)
}
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct X509EvaluationTime {
    unix_timestamp_seconds: i64,
    subsecond_millis: u64,
}
impl X509EvaluationTime {
    fn is_before(self, boundary: ASN1Time) -> bool {
        self.unix_timestamp_seconds < boundary.timestamp()
    }

    fn is_after(self, boundary: ASN1Time) -> bool {
        self.unix_timestamp_seconds > boundary.timestamp()
            || (self.unix_timestamp_seconds == boundary.timestamp() && self.subsecond_millis != 0)
    }
}
fn x509_evaluation_time(block_unix_timestamp_ms: u64) -> Result<X509EvaluationTime, Error> {
    #[cfg(test)]
    let block_unix_timestamp_ms = if block_unix_timestamp_ms == 0 {
        1_800_000_000_000
    } else {
        block_unix_timestamp_ms
    };
    let seconds = i64::try_from(block_unix_timestamp_ms / 1_000).map_err(|_| {
        invalid_attestation("offline device attestation block timestamp is out of range")
    })?;
    ASN1Time::from_timestamp(seconds).map_err(|_| {
        invalid_attestation(
            "offline device attestation block timestamp cannot be represented as ASN.1 time",
        )
    })?;
    Ok(X509EvaluationTime {
        unix_timestamp_seconds: seconds,
        subsecond_millis: block_unix_timestamp_ms % 1_000,
    })
}
fn parse_x509_certificate_der(certificate_der: &[u8]) -> Result<X509Certificate<'_>, Error> {
    reject_invalid_attestation!(
        certificate_der.is_empty()
            || certificate_der.len() > OFFLINE_ATTESTATION_MAX_X509_CERTIFICATE_BYTES,
        "attestation certificate DER size is outside protocol bounds",
    );
    let strict_tbs_der = strict_x509_tbs_certificate_der(certificate_der)?;
    let (remaining, certificate) = X509Certificate::from_der(certificate_der)
        .map_err(|_| invalid_attestation("attestation certificate DER is invalid"))?;
    reject_invalid_attestation!(
        !remaining.is_empty(),
        "attestation certificate DER has trailing bytes",
    );
    reject_invalid_attestation!(
        strict_tbs_der != certificate.tbs_certificate.as_ref(),
        "attestation certificate TBSCertificate parser did not consume the exact signed DER",
    );
    validate_x509_certificate_signature_algorithm(&certificate)?;
    Ok(certificate)
}
fn x509_certificate_signature_oid_is_weak(oid: &str) -> bool {
    matches!(
        oid,
        // PKCS #1 and legacy OIW/DEC RSA signature identifiers using MD2.
        "1.2.840.113549.1.1.2" | "1.3.14.7.2.3.1" | "1.3.12.2.1011.7.3.1"
        // PKCS #1 and legacy OIW RSA signature identifiers using MD5.
        | "1.2.840.113549.1.1.4" | "1.3.14.3.2.3" | "1.3.14.3.2.25"
        // RSA, DSA, and ECDSA signature identifiers using SHA-1.
        | "1.2.840.113549.1.1.5" | "1.3.14.3.2.29"
        | "1.2.840.10040.4.3" | "1.3.14.3.2.27"
        | "1.2.840.10045.4.1"
    )
}
fn x509_algorithm_parameters_are_absent_or_null(
    parameters: Option<&x509_parser::asn1_rs::Any<'_>>,
) -> bool {
    parameters.is_none_or(|parameters| {
        parameters.class() == x509_parser::asn1_rs::Class::Universal
            && parameters.tag() == x509_parser::asn1_rs::Tag::Null
            && !parameters.header.constructed()
            && parameters.data.is_empty()
    })
}
fn validate_x509_rsa_pss_signature_algorithm(
    algorithm: &x509_parser::x509::AlgorithmIdentifier<'_>,
) -> Result<(), Error> {
    let parameters = algorithm.parameters.as_ref().ok_or_else(|| {
        invalid_attestation("attestation certificate RSA-PSS parameters are missing")
    })?;
    reject_invalid_attestation!(
        parameters.tag() != x509_parser::asn1_rs::Tag::Sequence,
        "attestation certificate RSA-PSS parameters are not a DER sequence",
    );

    // RFC 4055 assigns SHA-1 defaults to hashAlgorithm and maskGenAlgorithm.
    // Require the exact explicit SHA-2 profile consumed by ring. Requiring the
    // canonical [0], [1], [2] field sequence also prevents the parser's ignored
    // trailing fields from changing the declared algorithm semantics.
    let mut remaining = parameters.data;
    for expected_tag in [0, 1, 2] {
        let (next, field) = x509_parser::asn1_rs::Any::from_der(remaining).map_err(|_| {
            invalid_attestation("attestation certificate RSA-PSS parameters are malformed")
        })?;
        reject_invalid_attestation!(
            field.class() != x509_parser::asn1_rs::Class::ContextSpecific
                || field.tag() != x509_parser::asn1_rs::Tag(expected_tag),
            "attestation certificate RSA-PSS parameters do not use the required SHA-2 profile",
        );
        remaining = next;
    }
    reject_invalid_attestation!(
        !remaining.is_empty(),
        "attestation certificate RSA-PSS parameters contain trailing fields",
    );

    let parameters = x509_parser::signature_algorithm::RsaSsaPssParams::try_from(parameters)
        .map_err(|_| {
            invalid_attestation("attestation certificate RSA-PSS parameters are malformed")
        })?;
    let hash_oid = parameters.hash_algorithm_oid().to_string();
    let expected_salt_length = match hash_oid.as_str() {
        "2.16.840.1.101.3.4.2.1" => 32,
        "2.16.840.1.101.3.4.2.2" => 48,
        "2.16.840.1.101.3.4.2.3" => 64,
        _ => {
            return Err(invalid_attestation(
                "attestation certificate RSA-PSS hash algorithm is not approved",
            )
            .into());
        }
    };
    let hash_algorithm = parameters.hash_algorithm().ok_or_else(|| {
        invalid_attestation("attestation certificate RSA-PSS hash algorithm is implicit SHA-1")
    })?;
    reject_invalid_attestation!(
        !x509_algorithm_parameters_are_absent_or_null(hash_algorithm.parameters.as_ref()),
        "attestation certificate RSA-PSS hash parameters are invalid",
    );

    let mask_algorithm = parameters.mask_gen_algorithm_raw().ok_or_else(|| {
        invalid_attestation("attestation certificate RSA-PSS mask algorithm is implicit SHA-1")
    })?;
    reject_invalid_attestation!(
        mask_algorithm.algorithm.to_string() != "1.2.840.113549.1.1.8",
        "attestation certificate RSA-PSS mask algorithm is not MGF1",
    );
    let mask_parameters = mask_algorithm.parameters.as_ref().ok_or_else(|| {
        invalid_attestation("attestation certificate RSA-PSS MGF1 parameters are missing")
    })?;
    reject_invalid_attestation!(
        mask_parameters.tag() != x509_parser::asn1_rs::Tag::Sequence,
        "attestation certificate RSA-PSS MGF1 parameters are malformed",
    );
    let (remaining, mask_hash_oid) = x509_parser::asn1_rs::Oid::from_der(mask_parameters.data)
        .map_err(|_| {
            invalid_attestation("attestation certificate RSA-PSS MGF1 hash is malformed")
        })?;
    let mask_hash_parameters = if remaining.is_empty() {
        None
    } else {
        let (remaining, parameters) =
            x509_parser::asn1_rs::Any::from_der(remaining).map_err(|_| {
                invalid_attestation("attestation certificate RSA-PSS MGF1 hash is malformed")
            })?;
        reject_invalid_attestation!(
            !remaining.is_empty(),
            "attestation certificate RSA-PSS MGF1 parameters contain trailing data",
        );
        Some(parameters)
    };
    reject_invalid_attestation!(
        mask_hash_oid.to_string() != hash_oid
            || !x509_algorithm_parameters_are_absent_or_null(mask_hash_parameters.as_ref()),
        "attestation certificate RSA-PSS MGF1 hash does not match the signature hash",
    );
    reject_invalid_attestation!(
        parameters.salt_length() != expected_salt_length,
        "attestation certificate RSA-PSS salt length does not match the signature hash",
    );
    reject_invalid_attestation!(
        parameters.trailer_field() != 1,
        "attestation certificate RSA-PSS trailer field is invalid",
    );
    Ok(())
}
fn validate_x509_certificate_signature_algorithm(
    certificate: &X509Certificate<'_>,
) -> Result<(), Error> {
    // RFC 5280 requires the signature AlgorithmIdentifier inside TBSCertificate
    // to match the outer Certificate.signatureAlgorithm. Compare the complete
    // parsed values, including parameters, before selecting a verifier.
    reject_invalid_attestation!(
        certificate.tbs_certificate.signature != certificate.signature_algorithm,
        "attestation certificate inner and outer signature algorithms do not match",
    );
    reject_invalid_attestation!(
        x509_certificate_signature_oid_is_weak(
            &certificate.signature_algorithm.algorithm.to_string()
        ),
        "attestation certificate uses a prohibited weak signature algorithm",
    );
    let signature_algorithm = &certificate.signature_algorithm;
    match signature_algorithm.algorithm.to_string().as_str() {
        "1.2.840.113549.1.1.10" => {
            validate_x509_rsa_pss_signature_algorithm(signature_algorithm)?;
        }
        "1.2.840.113549.1.1.11" | "1.2.840.113549.1.1.12" | "1.2.840.113549.1.1.13" => {
            reject_invalid_attestation!(
                !x509_algorithm_parameters_are_absent_or_null(
                    signature_algorithm.parameters.as_ref()
                ),
                "attestation certificate RSA signature parameters are invalid",
            );
        }
        "1.2.840.10045.4.3.2" | "1.2.840.10045.4.3.3" | "1.3.101.112" => {
            reject_invalid_attestation!(
                signature_algorithm.parameters.is_some(),
                "attestation certificate signature parameters must be absent",
            );
        }
        _ => {
            return Err(invalid_attestation(
                "attestation certificate signature algorithm is not approved",
            )
            .into());
        }
    }
    Ok(())
}
fn validate_x509_certificate_critical_extensions(
    certificate: &X509Certificate<'_>,
) -> Result<(), Error> {
    let mut seen_extension_oids = HashSet::new();
    for extension in certificate.extensions() {
        let extension_oid = extension.oid.to_string();
        if !seen_extension_oids.insert(extension_oid.clone()) {
            return Err(invalid_attestation(
                "attestation certificate contains duplicate extension OIDs",
            )
            .into());
        }

        if matches!(
            extension_oid.as_str(),
            "2.5.29.30" | "2.5.29.32" | "2.5.29.33" | "2.5.29.36" | "2.5.29.54"
        ) {
            return Err(invalid_attestation(
                "attestation certificate contains an unsupported path-processing extension",
            )
            .into());
        }

        if extension.critical
            && !matches!(
                extension.parsed_extension(),
                ParsedExtension::BasicConstraints(_) | ParsedExtension::KeyUsage(_)
            )
        {
            return Err(invalid_attestation(
                "attestation certificate contains an unsupported critical extension",
            )
            .into());
        }
    }
    Ok(())
}
fn x509_certificate_is_ca(certificate: &X509Certificate<'_>) -> Result<bool, Error> {
    let Some(basic_constraints) = certificate.basic_constraints().map_err(|_| {
        invalid_attestation("attestation certificate basic constraints are invalid")
    })?
    else {
        return Ok(false);
    };
    if !basic_constraints.critical || !basic_constraints.value.ca {
        return Ok(false);
    }
    let Some(key_usage) = certificate
        .key_usage()
        .map_err(|_| invalid_attestation("attestation certificate key usage is invalid"))?
    else {
        return Ok(false);
    };
    Ok(key_usage.critical && key_usage.value.key_cert_sign())
}
fn validate_x509_leaf_certificate_profile(certificate: &X509Certificate<'_>) -> Result<(), Error> {
    if certificate
        .basic_constraints()
        .map_err(|_| invalid_attestation("attestation certificate basic constraints are invalid"))?
        .is_some_and(|basic_constraints| {
            basic_constraints.value.ca || basic_constraints.value.path_len_constraint.is_some()
        })
    {
        return Err(invalid_attestation(
            "attestation leaf certificate must not assert CA basic constraints",
        )
        .into());
    }
    let Some(key_usage) = certificate
        .key_usage()
        .map_err(|_| invalid_attestation("attestation certificate key usage is invalid"))?
    else {
        return Err(invalid_attestation(
            "attestation leaf certificate must contain critical signing key usage",
        )
        .into());
    };
    if !key_usage.critical
        || !key_usage.value.digital_signature()
        || key_usage.value.key_cert_sign()
    {
        return Err(invalid_attestation(
            "attestation leaf certificate must be an end-entity signing certificate",
        )
        .into());
    }
    Ok(())
}
// Kagemusha augments RFC 5280 by applying pathLenConstraint from a governed
// trust-anchor certificate as application policy, whether the anchor is
// included in the submitted chain or supplied only by policy.
fn validate_x509_ca_path_len_constraint(
    certificate: &X509Certificate<'_>,
    subordinate_ca_count: usize,
) -> Result<(), Error> {
    let Some(basic_constraints) = certificate.basic_constraints().map_err(|_| {
        invalid_attestation("attestation certificate basic constraints are invalid")
    })?
    else {
        return Err(
            invalid_attestation("attestation certificate issuer has no basic constraints").into(),
        );
    };
    if basic_constraints
        .value
        .path_len_constraint
        .is_some_and(|maximum| subordinate_ca_count > maximum as usize)
    {
        return Err(invalid_attestation(
            "attestation certificate path length constraint is violated",
        )
        .into());
    }
    Ok(())
}
fn non_self_issued_subordinate_ca_count(certificates: &[X509Certificate<'_>]) -> usize {
    certificates
        .iter()
        .filter(|certificate| certificate.issuer() != certificate.subject())
        .count()
}
fn validate_x509_certificate_time(
    certificate: &X509Certificate<'_>,
    evaluation_time: X509EvaluationTime,
) -> Result<(), Error> {
    if !evaluation_time.is_before(certificate.validity().not_before)
        && !evaluation_time.is_after(certificate.validity().not_after)
    {
        Ok(())
    } else {
        Err(
            invalid_attestation("attestation certificate is not valid at the block timestamp")
                .into(),
        )
    }
}
fn verify_x509_certificate_signature(
    certificate: &X509Certificate<'_>,
    issuer: &X509Certificate<'_>,
) -> Result<(), Error> {
    certificate
        .verify_signature(Some(issuer.public_key()))
        .map_err(|_| {
            invalid_attestation("attestation certificate signature chain is invalid").into()
        })
}
fn validate_attestation_certificate_chain(
    certificate_chain: &[Vec<u8>],
    trusted_roots_der: &[Vec<u8>],
    revoked_certificate_tbs_sha256: &HashSet<[u8; 32]>,
    evaluation_time: X509EvaluationTime,
) -> Result<(), Error> {
    if certificate_chain.is_empty()
        || certificate_chain.len() > OFFLINE_ATTESTATION_MAX_X509_CHAIN_CERTIFICATES
        || trusted_roots_der.is_empty()
    {
        return Err(invalid_attestation(
            "attestation certificate chain size is outside protocol bounds",
        )
        .into());
    }
    let mut seen = HashSet::new();
    for certificate_der in certificate_chain {
        let certificate = parse_x509_certificate_der(certificate_der)?;
        let certificate_tbs_sha256 = sha256_bytes(certificate.tbs_certificate.as_ref());
        if revoked_certificate_tbs_sha256.contains(&certificate_tbs_sha256) {
            return Err(labeled_invariant(
                "revoked_attestation",
                "attestation certificate is revoked by Offline device attestation policy",
            )
            .into());
        }
        if !seen.insert(certificate_tbs_sha256) {
            return Err(invalid_attestation(
                "attestation certificate chain contains duplicate certificates",
            )
            .into());
        }
        validate_x509_certificate_critical_extensions(&certificate)?;
        validate_x509_certificate_time(&certificate, evaluation_time)?;
    }
    let parsed_chain = certificate_chain
        .iter()
        .map(|certificate_der| parse_x509_certificate_der(certificate_der))
        .collect::<Result<Vec<_>, _>>()?;
    let leaf = parsed_chain
        .first()
        .ok_or_else(|| invalid_attestation("attestation certificate chain is empty"))?;
    validate_x509_leaf_certificate_profile(leaf)?;
    for (issuer_offset, pair) in parsed_chain.windows(2).enumerate() {
        let certificate = &pair[0];
        let issuer = &pair[1];
        if certificate.issuer() != issuer.subject() || !x509_certificate_is_ca(issuer)? {
            return Err(
                invalid_attestation("attestation certificate issuer chain is invalid").into(),
            );
        }
        let issuer_index = issuer_offset + 1;
        let subordinate_ca_count =
            non_self_issued_subordinate_ca_count(&parsed_chain[1..issuer_index]);
        verify_x509_certificate_signature(certificate, issuer)?;
        validate_x509_ca_path_len_constraint(issuer, subordinate_ca_count)?;
    }
    let tail_der = certificate_chain.last().expect("chain is non-empty");
    let tail = parsed_chain.last().expect("chain is non-empty");
    // Exact DER equality is an out-of-band trust decision. Check exact pins before
    // issuer-name candidates so an older same-subject key cannot shadow a rollover
    // anchor, and do not confuse self-issued names with a valid self-signature.
    for root_der in trusted_roots_der {
        if tail_der != root_der {
            continue;
        }
        let Ok(root) = parse_x509_certificate_der(root_der) else {
            continue;
        };
        if revoked_certificate_tbs_sha256.contains(&sha256_bytes(root.tbs_certificate.as_ref())) {
            continue;
        }
        if validate_x509_certificate_critical_extensions(&root).is_err()
            || validate_x509_certificate_time(&root, evaluation_time).is_err()
            || !x509_certificate_is_ca(&root).unwrap_or(false)
        {
            continue;
        }
        return Ok(());
    }

    let subordinate_ca_count = non_self_issued_subordinate_ca_count(&parsed_chain[1..]);
    let mut signature_failure = None;
    let mut path_len_failure = None;
    for root_der in trusted_roots_der {
        let Ok(root) = parse_x509_certificate_der(root_der) else {
            continue;
        };
        if revoked_certificate_tbs_sha256.contains(&sha256_bytes(root.tbs_certificate.as_ref())) {
            continue;
        }
        if validate_x509_certificate_critical_extensions(&root).is_err()
            || validate_x509_certificate_time(&root, evaluation_time).is_err()
            || !x509_certificate_is_ca(&root).unwrap_or(false)
            || tail.issuer() != root.subject()
        {
            continue;
        }
        if let Err(error) = verify_x509_certificate_signature(tail, &root) {
            if signature_failure.is_none() {
                signature_failure = Some(error);
            }
            continue;
        }
        // RFC 5280 does not process the trust-anchor certificate as part of the
        // prospective path. Kagemusha intentionally augments that algorithm by
        // treating a governed anchor's pathLenConstraint as application policy.
        if let Err(error) = validate_x509_ca_path_len_constraint(&root, subordinate_ca_count) {
            if path_len_failure.is_none() {
                path_len_failure = Some(error);
            }
            continue;
        }
        return Ok(());
    }
    if let Some(error) = path_len_failure {
        return Err(error);
    }
    if let Some(error) = signature_failure {
        return Err(error);
    }
    #[cfg(test)]
    if tail.issuer() == tail.subject()
        && x509_certificate_is_ca(tail)?
        && x509_certificate_is_offline_attestation_test_root(tail)
    {
        verify_x509_certificate_signature(tail, tail)?;
        return Ok(());
    }
    Err(labeled_invariant(
        "invalid_attestation",
        "attestation certificate chain is not anchored in a trusted root",
    )
    .into())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AndroidKeyAttestationCertificateChainKind {
    Factory,
    RemoteKeyProvisioning,
}

fn classify_android_key_attestation_certificate_chain(
    root_nearest_non_anchor: &X509Certificate<'_>,
) -> Result<AndroidKeyAttestationCertificateChainKind, Error> {
    let subject = root_nearest_non_anchor.subject();
    let has_factory_serial_number = subject
        .iter_attributes()
        .any(|attribute| attribute.attr_type().to_string() == "2.5.4.5");

    let mut common_names = subject.iter_common_name();
    let has_exact_rkp_common_name = common_names
        .next()
        .is_some_and(|attribute| attribute.as_str().is_ok_and(|value| value == "Droid CA2"))
        && common_names.next().is_none();
    let mut organizations = subject.iter_organization();
    let has_exact_rkp_organization = organizations
        .next()
        .is_some_and(|attribute| attribute.as_str().is_ok_and(|value| value == "Google LLC"))
        && organizations.next().is_none();
    let has_rkp_identity = has_exact_rkp_common_name
        && has_exact_rkp_organization
        && subject.iter_attributes().count() == 2;

    match (has_factory_serial_number, has_rkp_identity) {
        (true, false) => Ok(AndroidKeyAttestationCertificateChainKind::Factory),
        (false, true) => Ok(AndroidKeyAttestationCertificateChainKind::RemoteKeyProvisioning),
        (true, true) => Err(invalid_attestation(
            "Android Key Attestation certificate chain classification is ambiguous",
        )
        .into()),
        (false, false) => Err(invalid_attestation(
            "Android Key Attestation certificate chain classification is unknown",
        )
        .into()),
    }
}

fn x509_certificate_canonical_serial_hex(certificate: &X509Certificate<'_>) -> String {
    // Android's status service keys entries by BigInteger.toString(16): lowercase
    // hexadecimal with no sign byte or other leading zero padding.
    certificate.tbs_certificate.serial.to_str_radix(16)
}

fn validate_android_key_attestation_certificate_chain_time_profile(
    parsed_chain: &[X509Certificate<'_>],
    anchor_is_submitted: bool,
    anchor: &X509Certificate<'_>,
    anchor_der: &[u8],
    evaluation_time: X509EvaluationTime,
    registration_expiry_time: X509EvaluationTime,
) -> Result<(), Error> {
    reject_invalid_attestation!(
        registration_expiry_time < evaluation_time,
        "Android Key Attestation registration expiry precedes the block timestamp",
    );
    let classifier_index = if anchor_is_submitted {
        parsed_chain.len().checked_sub(2)
    } else {
        parsed_chain.len().checked_sub(1)
    }
    .ok_or_else(|| {
        invalid_attestation(
            "Android Key Attestation certificate chain has no root-nearest non-anchor certificate",
        )
    })?;
    let chain_kind =
        classify_android_key_attestation_certificate_chain(&parsed_chain[classifier_index])?;
    let legacy_google_root = decode_trusted_root_der(ANDROID_KEY_ATTESTATION_ROOT_CA_DER_B64)?;
    let factory_may_ignore_expiration = chain_kind
        == AndroidKeyAttestationCertificateChainKind::Factory
        && anchor_der == legacy_google_root.as_slice();

    let validate_non_target = |certificate: &X509Certificate<'_>| -> Result<(), Error> {
        if evaluation_time.is_before(certificate.validity().not_before) {
            return Err(invalid_attestation(
                "Android Key Attestation non-target certificate is not yet valid at the block timestamp",
            )
            .into());
        }
        match chain_kind {
            AndroidKeyAttestationCertificateChainKind::Factory => {
                if !factory_may_ignore_expiration
                    && evaluation_time.is_after(certificate.validity().not_after)
                {
                    return Err(invalid_attestation(
                        "Android Key Attestation factory certificate is expired at the block timestamp",
                    )
                    .into());
                }
            }
            AndroidKeyAttestationCertificateChainKind::RemoteKeyProvisioning => {
                if validate_x509_certificate_time(certificate, evaluation_time).is_err() {
                    return Err(invalid_attestation(
                        "Android Key Attestation RKP certificate is not valid at the block timestamp",
                    )
                    .into());
                }
                if validate_x509_certificate_time(certificate, registration_expiry_time).is_err() {
                    return Err(invalid_attestation(
                        "Android Key Attestation RKP certificate is not valid through the registration lifetime",
                    )
                    .into());
                }
            }
        }
        Ok(())
    };
    for certificate in parsed_chain.iter().skip(1) {
        validate_non_target(certificate)?;
    }
    if !anchor_is_submitted {
        validate_non_target(anchor)?;
    }
    Ok(())
}

fn validate_android_key_attestation_certificate_chain(
    certificate_chain: &[Vec<u8>],
    trusted_roots_der: &[Vec<u8>],
    revoked_certificate_tbs_sha256: &HashSet<[u8; 32]>,
    non_valid_certificate_serials: &HashSet<String>,
    evaluation_time: X509EvaluationTime,
    registration_expiry_time: X509EvaluationTime,
) -> Result<(), Error> {
    if certificate_chain.is_empty()
        || certificate_chain.len() > OFFLINE_ATTESTATION_MAX_X509_CHAIN_CERTIFICATES
        || trusted_roots_der.is_empty()
    {
        return Err(invalid_attestation(
            "Android Key Attestation certificate chain size is outside protocol bounds",
        )
        .into());
    }
    let mut seen = HashSet::new();
    for certificate_der in certificate_chain {
        let certificate = parse_x509_certificate_der(certificate_der)?;
        let certificate_tbs_sha256 = sha256_bytes(certificate.tbs_certificate.as_ref());
        if revoked_certificate_tbs_sha256.contains(&certificate_tbs_sha256) {
            return Err(labeled_invariant(
                "revoked_attestation",
                "Android Key Attestation certificate is revoked by Offline device attestation policy",
            )
            .into());
        }
        if !seen.insert(certificate_tbs_sha256) {
            return Err(invalid_attestation(
                "Android Key Attestation certificate chain contains duplicate certificates",
            )
            .into());
        }
        validate_x509_certificate_critical_extensions(&certificate)?;
        if non_valid_certificate_serials
            .contains(&x509_certificate_canonical_serial_hex(&certificate))
        {
            return Err(labeled_invariant(
                "revoked_attestation",
                "Android Key Attestation certificate serial is non-valid in the governed status snapshot",
            )
            .into());
        }
    }
    let parsed_chain = certificate_chain
        .iter()
        .map(|certificate_der| parse_x509_certificate_der(certificate_der))
        .collect::<Result<Vec<_>, _>>()?;
    let leaf = parsed_chain
        .first()
        .ok_or_else(|| invalid_attestation("Android Key Attestation certificate chain is empty"))?;
    validate_x509_leaf_certificate_profile(leaf)?;
    for (issuer_offset, pair) in parsed_chain.windows(2).enumerate() {
        let certificate = &pair[0];
        let issuer = &pair[1];
        if certificate.issuer() != issuer.subject() || !x509_certificate_is_ca(issuer)? {
            return Err(invalid_attestation(
                "Android Key Attestation certificate issuer chain is invalid",
            )
            .into());
        }
        let issuer_index = issuer_offset + 1;
        let subordinate_ca_count =
            non_self_issued_subordinate_ca_count(&parsed_chain[1..issuer_index]);
        verify_x509_certificate_signature(certificate, issuer)?;
        validate_x509_ca_path_len_constraint(issuer, subordinate_ca_count)?;
    }
    let tail_der = certificate_chain.last().expect("chain is non-empty");
    let tail = parsed_chain.last().expect("chain is non-empty");
    let mut profile_failure = None;
    for root_der in trusted_roots_der {
        if tail_der != root_der {
            continue;
        }
        let Ok(root) = parse_x509_certificate_der(root_der) else {
            continue;
        };
        if revoked_certificate_tbs_sha256.contains(&sha256_bytes(root.tbs_certificate.as_ref()))
            || non_valid_certificate_serials.contains(&x509_certificate_canonical_serial_hex(&root))
        {
            continue;
        }
        if validate_x509_certificate_critical_extensions(&root).is_err()
            || !x509_certificate_is_ca(&root).unwrap_or(false)
        {
            continue;
        }
        match validate_android_key_attestation_certificate_chain_time_profile(
            &parsed_chain,
            true,
            &root,
            root_der,
            evaluation_time,
            registration_expiry_time,
        ) {
            Ok(()) => return Ok(()),
            Err(error) => {
                if profile_failure.is_none() {
                    profile_failure = Some(error);
                }
            }
        }
    }

    let subordinate_ca_count = non_self_issued_subordinate_ca_count(&parsed_chain[1..]);
    let mut signature_failure = None;
    let mut path_len_failure = None;
    for root_der in trusted_roots_der {
        let Ok(root) = parse_x509_certificate_der(root_der) else {
            continue;
        };
        if revoked_certificate_tbs_sha256.contains(&sha256_bytes(root.tbs_certificate.as_ref()))
            || non_valid_certificate_serials.contains(&x509_certificate_canonical_serial_hex(&root))
        {
            continue;
        }
        if validate_x509_certificate_critical_extensions(&root).is_err()
            || !x509_certificate_is_ca(&root).unwrap_or(false)
            || tail.issuer() != root.subject()
        {
            continue;
        }
        if let Err(error) = verify_x509_certificate_signature(tail, &root) {
            if signature_failure.is_none() {
                signature_failure = Some(error);
            }
            continue;
        }
        if let Err(error) = validate_x509_ca_path_len_constraint(&root, subordinate_ca_count) {
            if path_len_failure.is_none() {
                path_len_failure = Some(error);
            }
            continue;
        }
        match validate_android_key_attestation_certificate_chain_time_profile(
            &parsed_chain,
            false,
            &root,
            root_der,
            evaluation_time,
            registration_expiry_time,
        ) {
            Ok(()) => return Ok(()),
            Err(error) => {
                if profile_failure.is_none() {
                    profile_failure = Some(error);
                }
            }
        }
    }
    if let Some(error) = path_len_failure {
        return Err(error);
    }
    if let Some(error) = signature_failure {
        return Err(error);
    }
    if let Some(error) = profile_failure {
        return Err(error);
    }
    Err(labeled_invariant(
        "invalid_attestation",
        "Android Key Attestation certificate chain is not anchored in a trusted root",
    )
    .into())
}
#[cfg(test)]
fn x509_certificate_is_offline_attestation_test_root(certificate: &X509Certificate<'_>) -> bool {
    certificate.subject().iter_common_name().any(|name| {
        name.as_str()
            .is_ok_and(|value| value == "Iroha Offline Attestation Test Root")
    })
}
fn x509_unique_extension_value(
    certificate: &X509Certificate<'_>,
    oid: &str,
    duplicate_message: &'static str,
) -> Result<Option<Vec<u8>>, Error> {
    let mut matches = certificate
        .extensions()
        .iter()
        .filter(|extension| extension.oid.to_string() == oid);
    let first = matches.next().map(|extension| extension.value.to_vec());
    if matches.next().is_some() {
        return Err(invalid_attestation(duplicate_message).into());
    }
    Ok(first)
}
fn x509_root_nearest_unique_extension_value(
    certificate_chain: &[Vec<u8>],
    oid: &str,
    duplicate_message: &'static str,
) -> Result<Option<(usize, Vec<u8>)>, Error> {
    for (index, certificate_der) in certificate_chain.iter().enumerate().rev() {
        let certificate = parse_x509_certificate_der(certificate_der)?;
        if let Some(value) = x509_unique_extension_value(&certificate, oid, duplicate_message)? {
            return Ok(Some((index, value)));
        }
    }
    Ok(None)
}
fn android_keymint_leaf_attestation_extension(
    certificate_chain: &[Vec<u8>],
) -> Result<Vec<u8>, Error> {
    let (certificate_index, extension_value) = x509_root_nearest_unique_extension_value(
        certificate_chain,
        OFFLINE_ATTESTATION_ANDROID_KEY_OID,
        "Android KeyMint certificate contains duplicate attestation extensions",
    )?
    .ok_or_else(|| {
        labeled_invariant(
            "invalid_attestation",
            "Android KeyMint certificate chain is missing the attestation extension",
        )
    })?;
    if certificate_index != 0 {
        return Err(labeled_invariant(
            "invalid_attestation",
            "Android KeyMint root-nearest attestation extension does not directly attest the assertion leaf key",
        )
        .into());
    }
    Ok(extension_value)
}
fn x509_subject_public_key_bytes(certificate: &X509Certificate<'_>) -> Vec<u8> {
    certificate.public_key().subject_public_key.data.to_vec()
}

#[cfg(test)]
mod attestation_certificate_validation_tests {
    use super::*;
    use rcgen::{
        BasicConstraints, CertificateParams, CustomExtension, DnType, IsCa, Issuer, KeyPair,
        KeyUsagePurpose, PKCS_ECDSA_P256_SHA256, date_time_ymd,
    };

    struct CertificateChainFixture {
        leaf: Vec<u8>,
        intermediate: Vec<u8>,
        root: Vec<u8>,
    }

    fn certificate_chain_fixture(root_path_len: u8) -> CertificateChainFixture {
        let root_key = KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256).expect("generate root key");
        let mut root_params = CertificateParams::new(vec!["kagemusha-root.example".to_owned()])
            .expect("root certificate parameters");
        root_params
            .distinguished_name
            .push(DnType::CommonName, "Kagemusha Attestation Test Root");
        root_params.is_ca = IsCa::Ca(BasicConstraints::Constrained(root_path_len));
        root_params.key_usages = vec![KeyUsagePurpose::KeyCertSign];
        root_params.not_before = date_time_ymd(2020, 1, 1);
        root_params.not_after = date_time_ymd(2030, 1, 1);
        let root_certificate = root_params
            .self_signed(&root_key)
            .expect("self-sign root certificate");
        let root_issuer = Issuer::new(root_params, root_key);

        let intermediate_key =
            KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256).expect("generate intermediate key");
        let mut intermediate_params =
            CertificateParams::new(vec!["kagemusha-intermediate.example".to_owned()])
                .expect("intermediate certificate parameters");
        intermediate_params.distinguished_name.push(
            DnType::CommonName,
            "Kagemusha Attestation Test Intermediate",
        );
        intermediate_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        intermediate_params.key_usages = vec![KeyUsagePurpose::KeyCertSign];
        intermediate_params.not_before = date_time_ymd(2020, 1, 1);
        intermediate_params.not_after = date_time_ymd(2030, 1, 1);
        let intermediate_certificate = intermediate_params
            .signed_by(&intermediate_key, &root_issuer)
            .expect("sign intermediate certificate");
        let intermediate_issuer = Issuer::new(intermediate_params, intermediate_key);

        let leaf_key = KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256).expect("generate leaf key");
        let mut leaf_params = CertificateParams::new(vec!["kagemusha-leaf.example".to_owned()])
            .expect("leaf certificate parameters");
        leaf_params
            .distinguished_name
            .push(DnType::CommonName, "Kagemusha Attestation Test Leaf");
        leaf_params.is_ca = IsCa::NoCa;
        leaf_params.key_usages = vec![KeyUsagePurpose::DigitalSignature];
        leaf_params.not_before = date_time_ymd(2020, 1, 1);
        leaf_params.not_after = date_time_ymd(2030, 1, 1);
        let leaf_certificate = leaf_params
            .signed_by(&leaf_key, &intermediate_issuer)
            .expect("sign leaf certificate");

        CertificateChainFixture {
            leaf: leaf_certificate.der().to_vec(),
            intermediate: intermediate_certificate.der().to_vec(),
            root: root_certificate.der().to_vec(),
        }
    }

    fn evaluation_time() -> X509EvaluationTime {
        x509_evaluation_time(1_800_000_000_000).expect("fixed certificate evaluation time")
    }

    fn ca_certificate_params(common_name: &str) -> CertificateParams {
        let mut params =
            CertificateParams::new(Vec::<String>::new()).expect("CA certificate parameters");
        params
            .distinguished_name
            .push(DnType::CommonName, common_name);
        params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        params.key_usages = vec![KeyUsagePurpose::KeyCertSign];
        params.not_before = date_time_ymd(2020, 1, 1);
        params.not_after = date_time_ymd(2030, 1, 1);
        params
    }

    fn leaf_certificate_params(common_name: &str) -> CertificateParams {
        let mut params =
            CertificateParams::new(Vec::<String>::new()).expect("leaf certificate parameters");
        params
            .distinguished_name
            .push(DnType::CommonName, common_name);
        params.is_ca = IsCa::NoCa;
        params.key_usages = vec![KeyUsagePurpose::DigitalSignature];
        params.not_before = date_time_ymd(2020, 1, 1);
        params.not_after = date_time_ymd(2030, 1, 1);
        params
    }

    fn self_signed_leaf_certificate(
        custom_extensions: Vec<CustomExtension>,
        key_usages: Vec<KeyUsagePurpose>,
    ) -> Vec<u8> {
        let key = KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256).expect("generate leaf key");
        let mut params = leaf_certificate_params("Kagemusha Extension Test Leaf");
        params.custom_extensions = custom_extensions;
        params.key_usages = key_usages;
        params
            .self_signed(&key)
            .expect("self-sign extension test leaf")
            .der()
            .to_vec()
    }

    struct SameSubjectRootFixture {
        leaf: Vec<u8>,
        old_root: Vec<u8>,
        new_root: Vec<u8>,
    }

    fn same_subject_root_fixture() -> SameSubjectRootFixture {
        let old_root_key =
            KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256).expect("generate old root key");
        let old_root_params = ca_certificate_params("Kagemusha Rollover Root");
        let old_root = old_root_params
            .self_signed(&old_root_key)
            .expect("self-sign old root")
            .der()
            .to_vec();

        let new_root_key =
            KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256).expect("generate new root key");
        let new_root_params = ca_certificate_params("Kagemusha Rollover Root");
        let new_root = new_root_params
            .self_signed(&new_root_key)
            .expect("self-sign new root")
            .der()
            .to_vec();
        let new_root_issuer = Issuer::new(new_root_params, new_root_key);

        let leaf_key =
            KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256).expect("generate rollover leaf key");
        let leaf = leaf_certificate_params("Kagemusha Rollover Leaf")
            .signed_by(&leaf_key, &new_root_issuer)
            .expect("sign rollover leaf")
            .der()
            .to_vec();
        SameSubjectRootFixture {
            leaf,
            old_root,
            new_root,
        }
    }

    fn android_attestation_extension(value: &[u8]) -> CustomExtension {
        CustomExtension::from_oid_content(&[1, 3, 6, 1, 4, 1, 11_129, 2, 1, 17], value.to_vec())
    }

    fn android_extension_chain_fixture(
        ca_has_extension: bool,
        leaf_has_extension: bool,
    ) -> CertificateChainFixture {
        let root_key =
            KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256).expect("generate Android root key");
        let root_params = ca_certificate_params("Kagemusha Android Test Root");
        let root = root_params
            .self_signed(&root_key)
            .expect("self-sign Android root")
            .der()
            .to_vec();
        let root_issuer = Issuer::new(root_params, root_key);

        let intermediate_key = KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256)
            .expect("generate Android attestation CA key");
        let mut intermediate_params =
            ca_certificate_params("Kagemusha Android Attestation Test CA");
        if ca_has_extension {
            intermediate_params
                .custom_extensions
                .push(android_attestation_extension(&[0x30, 0x00]));
        }
        let intermediate = intermediate_params
            .signed_by(&intermediate_key, &root_issuer)
            .expect("sign Android attestation CA")
            .der()
            .to_vec();
        let intermediate_issuer = Issuer::new(intermediate_params, intermediate_key);

        let leaf_key = KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256)
            .expect("generate Android assertion leaf key");
        let mut leaf_params = leaf_certificate_params("Kagemusha Android Assertion Test Leaf");
        if leaf_has_extension {
            leaf_params
                .custom_extensions
                .push(android_attestation_extension(&[0x30, 0x00]));
        }
        let leaf = leaf_params
            .signed_by(&leaf_key, &intermediate_issuer)
            .expect("sign Android assertion leaf")
            .der()
            .to_vec();

        CertificateChainFixture {
            leaf,
            intermediate,
            root,
        }
    }

    #[derive(Clone, Copy)]
    enum AndroidCertificateChainTestKind {
        Factory,
        RemoteKeyProvisioning,
        Unknown,
        Ambiguous,
    }

    fn android_certificate_chain_profile_fixture(
        kind: AndroidCertificateChainTestKind,
        leaf_not_after_year: i32,
        intermediate_not_before_year: i32,
        intermediate_not_after_year: i32,
    ) -> CertificateChainFixture {
        let root_key =
            KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256).expect("generate Android root key");
        let mut root_params = ca_certificate_params("Kagemusha Android Profile Test Root");
        root_params.serial_number = Some(vec![0x00, 0x0c].into());
        root_params.not_after = date_time_ymd(2040, 1, 1);
        let root = root_params
            .self_signed(&root_key)
            .expect("self-sign Android profile root")
            .der()
            .to_vec();
        let root_issuer = Issuer::new(root_params, root_key);

        let intermediate_key = KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256)
            .expect("generate Android profile intermediate key");
        let mut intermediate_params = match kind {
            AndroidCertificateChainTestKind::RemoteKeyProvisioning
            | AndroidCertificateChainTestKind::Ambiguous => {
                let mut params = ca_certificate_params("Droid CA2");
                params
                    .distinguished_name
                    .push(DnType::OrganizationName, "Google LLC");
                params
            }
            AndroidCertificateChainTestKind::Factory | AndroidCertificateChainTestKind::Unknown => {
                ca_certificate_params("Kagemusha Android Profile Test Intermediate")
            }
        };
        if matches!(
            kind,
            AndroidCertificateChainTestKind::Factory | AndroidCertificateChainTestKind::Ambiguous
        ) {
            intermediate_params.distinguished_name.push(
                DnType::CustomDnType(vec![2, 5, 4, 5]),
                "factory-attestation-ca",
            );
        }
        intermediate_params.serial_number = Some(vec![0x00, 0x0b].into());
        intermediate_params.not_before = date_time_ymd(intermediate_not_before_year, 1, 1);
        intermediate_params.not_after = date_time_ymd(intermediate_not_after_year, 1, 1);
        let intermediate = intermediate_params
            .signed_by(&intermediate_key, &root_issuer)
            .expect("sign Android profile intermediate")
            .der()
            .to_vec();
        let intermediate_issuer = Issuer::new(intermediate_params, intermediate_key);

        let leaf_key = KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256)
            .expect("generate Android profile leaf key");
        let mut leaf_params = leaf_certificate_params("Kagemusha Android Profile Test Leaf");
        leaf_params.serial_number = Some(vec![0x00, 0x0a].into());
        leaf_params.not_after = date_time_ymd(leaf_not_after_year, 1, 1);
        let leaf = leaf_params
            .signed_by(&leaf_key, &intermediate_issuer)
            .expect("sign Android profile leaf")
            .der()
            .to_vec();

        CertificateChainFixture {
            leaf,
            intermediate,
            root,
        }
    }

    fn android_profile_evaluation_time() -> X509EvaluationTime {
        x509_evaluation_time(1_800_000_000_000).expect("Android profile evaluation time")
    }

    fn android_profile_registration_expiry_time() -> X509EvaluationTime {
        x509_evaluation_time(1_860_000_000_000).expect("Android profile registration expiry")
    }

    #[test]
    fn certificate_validity_preserves_exact_millisecond_boundaries() {
        let der = self_signed_leaf_certificate(Vec::new(), vec![KeyUsagePurpose::DigitalSignature]);
        let certificate =
            parse_x509_certificate_der(&der).expect("parse certificate validity fixture");
        let not_before_ms = u64::try_from(certificate.validity().not_before.timestamp())
            .expect("positive notBefore fixture")
            .checked_mul(1_000)
            .expect("notBefore milliseconds");
        let not_after_ms = u64::try_from(certificate.validity().not_after.timestamp())
            .expect("positive notAfter fixture")
            .checked_mul(1_000)
            .expect("notAfter milliseconds");

        validate_x509_certificate_time(
            &certificate,
            x509_evaluation_time(not_before_ms).expect("exact notBefore evaluation time"),
        )
        .expect("a certificate is valid at its exact notBefore instant");
        validate_x509_certificate_time(
            &certificate,
            x509_evaluation_time(not_after_ms).expect("exact notAfter evaluation time"),
        )
        .expect("a certificate is valid at its exact notAfter instant");

        let before_error = validate_x509_certificate_time(
            &certificate,
            x509_evaluation_time(not_before_ms - 1).expect("pre-notBefore evaluation time"),
        )
        .expect_err("the millisecond before notBefore must be rejected");
        assert!(before_error.to_string().contains("not valid"));
        let after_error = validate_x509_certificate_time(
            &certificate,
            x509_evaluation_time(not_after_ms + 1).expect("post-notAfter evaluation time"),
        )
        .expect_err("the millisecond after notAfter must be rejected");
        assert!(after_error.to_string().contains("not valid"));
    }

    #[test]
    fn certificate_signature_algorithm_identifiers_must_match_exactly() {
        let der = self_signed_leaf_certificate(Vec::new(), vec![KeyUsagePurpose::DigitalSignature]);
        let certificate =
            parse_x509_certificate_der(&der).expect("parse signature algorithm test certificate");

        let mut oid_mismatch = certificate.clone();
        oid_mismatch.signature_algorithm.algorithm =
            certificate.public_key().algorithm.algorithm.clone();
        let error = validate_x509_certificate_signature_algorithm(&oid_mismatch)
            .expect_err("different inner and outer signature OIDs must be rejected");
        assert!(error.to_string().contains("do not match"));

        let mut parameter_mismatch = certificate.clone();
        parameter_mismatch.signature_algorithm.parameters =
            certificate.public_key().algorithm.parameters.clone();
        assert!(parameter_mismatch.signature_algorithm.parameters.is_some());
        let error = validate_x509_certificate_signature_algorithm(&parameter_mismatch)
            .expect_err("different inner and outer signature parameters must be rejected");
        assert!(error.to_string().contains("do not match"));
    }

    #[test]
    fn weak_certificate_signature_algorithm_oids_are_rejected() {
        const WEAK_SIGNATURE_OIDS: &[(&str, &[u64])] = &[
            ("1.2.840.113549.1.1.2", &[1, 2, 840, 113549, 1, 1, 2]),
            ("1.3.14.7.2.3.1", &[1, 3, 14, 7, 2, 3, 1]),
            ("1.3.12.2.1011.7.3.1", &[1, 3, 12, 2, 1011, 7, 3, 1]),
            ("1.2.840.113549.1.1.4", &[1, 2, 840, 113549, 1, 1, 4]),
            ("1.3.14.3.2.3", &[1, 3, 14, 3, 2, 3]),
            ("1.3.14.3.2.25", &[1, 3, 14, 3, 2, 25]),
            ("1.2.840.113549.1.1.5", &[1, 2, 840, 113549, 1, 1, 5]),
            ("1.3.14.3.2.29", &[1, 3, 14, 3, 2, 29]),
            ("1.2.840.10040.4.3", &[1, 2, 840, 10040, 4, 3]),
            ("1.3.14.3.2.27", &[1, 3, 14, 3, 2, 27]),
            ("1.2.840.10045.4.1", &[1, 2, 840, 10045, 4, 1]),
        ];
        let der = self_signed_leaf_certificate(Vec::new(), vec![KeyUsagePurpose::DigitalSignature]);
        let certificate =
            parse_x509_certificate_der(&der).expect("parse weak algorithm test certificate");

        for &(oid, components) in WEAK_SIGNATURE_OIDS {
            let weak_oid = x509_parser::asn1_rs::Oid::from(components)
                .expect("construct weak signature algorithm OID");
            assert_eq!(weak_oid.to_string(), oid);
            let mut weak_certificate = certificate.clone();
            weak_certificate.tbs_certificate.signature.algorithm = weak_oid.clone();
            weak_certificate.signature_algorithm.algorithm = weak_oid;
            let error = validate_x509_certificate_signature_algorithm(&weak_certificate)
                .expect_err("weak signature algorithm must be rejected before verification");
            assert!(
                error
                    .to_string()
                    .contains("prohibited weak signature algorithm"),
                "weak signature OID {oid} was not rejected: {error}"
            );
        }

        for strong_oid in [
            "1.2.840.113549.1.1.10",
            "1.2.840.113549.1.1.11",
            "1.2.840.113549.1.1.12",
            "1.2.840.113549.1.1.13",
            "1.2.840.10045.4.3.2",
            "1.2.840.10045.4.3.3",
        ] {
            assert!(
                !x509_certificate_signature_oid_is_weak(strong_oid),
                "strong signature OID {strong_oid} must remain eligible"
            );
        }
    }

    #[test]
    fn rsa_pss_signature_parameters_must_match_verifier_profile() {
        const PSS_DEFAULT_SHA1: &[u8] = &[0x30, 0x00];
        const PSS_SHA256_IMPLICIT_MGF1_SHA1: &[u8] = &[
            0x30, 0x11, 0xa0, 0x0f, 0x30, 0x0d, 0x06, 0x09, 0x60, 0x86, 0x48, 0x01, 0x65, 0x03,
            0x04, 0x02, 0x01, 0x05, 0x00,
        ];
        const PSS_SHA256: &[u8] = &[
            0x30, 0x34, 0xa0, 0x0f, 0x30, 0x0d, 0x06, 0x09, 0x60, 0x86, 0x48, 0x01, 0x65, 0x03,
            0x04, 0x02, 0x01, 0x05, 0x00, 0xa1, 0x1c, 0x30, 0x1a, 0x06, 0x09, 0x2a, 0x86, 0x48,
            0x86, 0xf7, 0x0d, 0x01, 0x01, 0x08, 0x30, 0x0d, 0x06, 0x09, 0x60, 0x86, 0x48, 0x01,
            0x65, 0x03, 0x04, 0x02, 0x01, 0x05, 0x00, 0xa2, 0x03, 0x02, 0x01, 0x20,
        ];
        const PSS_SHA256_MGF1_SHA384: &[u8] = &[
            0x30, 0x34, 0xa0, 0x0f, 0x30, 0x0d, 0x06, 0x09, 0x60, 0x86, 0x48, 0x01, 0x65, 0x03,
            0x04, 0x02, 0x01, 0x05, 0x00, 0xa1, 0x1c, 0x30, 0x1a, 0x06, 0x09, 0x2a, 0x86, 0x48,
            0x86, 0xf7, 0x0d, 0x01, 0x01, 0x08, 0x30, 0x0d, 0x06, 0x09, 0x60, 0x86, 0x48, 0x01,
            0x65, 0x03, 0x04, 0x02, 0x02, 0x05, 0x00, 0xa2, 0x03, 0x02, 0x01, 0x20,
        ];
        const PSS_SHA256_SALT20: &[u8] = &[
            0x30, 0x34, 0xa0, 0x0f, 0x30, 0x0d, 0x06, 0x09, 0x60, 0x86, 0x48, 0x01, 0x65, 0x03,
            0x04, 0x02, 0x01, 0x05, 0x00, 0xa1, 0x1c, 0x30, 0x1a, 0x06, 0x09, 0x2a, 0x86, 0x48,
            0x86, 0xf7, 0x0d, 0x01, 0x01, 0x08, 0x30, 0x0d, 0x06, 0x09, 0x60, 0x86, 0x48, 0x01,
            0x65, 0x03, 0x04, 0x02, 0x01, 0x05, 0x00, 0xa2, 0x03, 0x02, 0x01, 0x14,
        ];
        const PSS_SHA256_TRAILER2: &[u8] = &[
            0x30, 0x39, 0xa0, 0x0f, 0x30, 0x0d, 0x06, 0x09, 0x60, 0x86, 0x48, 0x01, 0x65, 0x03,
            0x04, 0x02, 0x01, 0x05, 0x00, 0xa1, 0x1c, 0x30, 0x1a, 0x06, 0x09, 0x2a, 0x86, 0x48,
            0x86, 0xf7, 0x0d, 0x01, 0x01, 0x08, 0x30, 0x0d, 0x06, 0x09, 0x60, 0x86, 0x48, 0x01,
            0x65, 0x03, 0x04, 0x02, 0x01, 0x05, 0x00, 0xa2, 0x03, 0x02, 0x01, 0x20, 0xa3, 0x03,
            0x02, 0x01, 0x02,
        ];

        let der = self_signed_leaf_certificate(Vec::new(), vec![KeyUsagePurpose::DigitalSignature]);
        let certificate =
            parse_x509_certificate_der(&der).expect("parse RSA-PSS profile test certificate");
        let pss_oid = x509_parser::asn1_rs::Oid::from(&[1, 2, 840, 113549, 1, 1, 10])
            .expect("construct RSA-PSS OID");
        let validate_profile = |parameters_der: Option<&'static [u8]>| {
            let parameters = parameters_der.map(|parameters_der| {
                let (remaining, parameters) = x509_parser::asn1_rs::Any::from_der(parameters_der)
                    .expect("parse RSA-PSS parameter fixture");
                assert!(remaining.is_empty());
                parameters
            });
            let mut candidate = certificate.clone();
            candidate.tbs_certificate.signature.algorithm = pss_oid.clone();
            candidate.tbs_certificate.signature.parameters = parameters.clone();
            candidate.signature_algorithm.algorithm = pss_oid.clone();
            candidate.signature_algorithm.parameters = parameters;
            validate_x509_certificate_signature_algorithm(&candidate)
        };

        validate_profile(Some(PSS_SHA256)).expect("exact SHA-256 RSA-PSS profile must be eligible");
        for invalid_parameters in [
            None,
            Some(PSS_DEFAULT_SHA1),
            Some(PSS_SHA256_IMPLICIT_MGF1_SHA1),
            Some(PSS_SHA256_MGF1_SHA384),
            Some(PSS_SHA256_SALT20),
            Some(PSS_SHA256_TRAILER2),
        ] {
            let error = validate_profile(invalid_parameters)
                .expect_err("RSA-PSS parameters outside the verifier profile must be rejected");
            assert!(error.to_string().contains("RSA-PSS"), "{error}");
        }
    }

    #[test]
    fn same_subject_trust_anchor_rollover_is_order_independent() {
        let fixture = same_subject_root_fixture();
        for roots in [
            [fixture.old_root.clone(), fixture.new_root.clone()],
            [fixture.new_root.clone(), fixture.old_root.clone()],
        ] {
            validate_attestation_certificate_chain(
                std::slice::from_ref(&fixture.leaf),
                &roots,
                &HashSet::new(),
                evaluation_time(),
            )
            .expect("either same-subject root order must find the signing key");
        }

        validate_attestation_certificate_chain(
            &[fixture.leaf.clone(), fixture.new_root.clone()],
            &[fixture.old_root.clone(), fixture.new_root.clone()],
            &HashSet::new(),
            evaluation_time(),
        )
        .expect("an earlier same-subject root must not shadow an exact DER anchor");

        validate_attestation_certificate_chain(
            &[fixture.leaf],
            &[fixture.old_root],
            &HashSet::new(),
            evaluation_time(),
        )
        .expect_err("the wrong same-subject key alone must not anchor the leaf");
    }

    #[test]
    fn same_subject_path_len_candidate_failure_does_not_shadow_valid_anchor() {
        let root_key =
            KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256).expect("generate shared root key");
        let mut restrictive_params = ca_certificate_params("Kagemusha PathLen Rollover Root");
        restrictive_params.is_ca = IsCa::Ca(BasicConstraints::Constrained(0));
        let restrictive_root = restrictive_params
            .self_signed(&root_key)
            .expect("self-sign restrictive root")
            .der()
            .to_vec();

        let mut permissive_params = ca_certificate_params("Kagemusha PathLen Rollover Root");
        permissive_params.is_ca = IsCa::Ca(BasicConstraints::Constrained(1));
        let permissive_root = permissive_params
            .self_signed(&root_key)
            .expect("self-sign permissive root")
            .der()
            .to_vec();
        let root_issuer = Issuer::new(permissive_params, root_key);

        let intermediate_key = KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256)
            .expect("generate path length intermediate key");
        let intermediate_params = ca_certificate_params("Kagemusha PathLen Intermediate");
        let intermediate = intermediate_params
            .signed_by(&intermediate_key, &root_issuer)
            .expect("sign path length intermediate")
            .der()
            .to_vec();
        let intermediate_issuer = Issuer::new(intermediate_params, intermediate_key);
        let leaf_key =
            KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256).expect("generate path length leaf key");
        let leaf = leaf_certificate_params("Kagemusha PathLen Leaf")
            .signed_by(&leaf_key, &intermediate_issuer)
            .expect("sign path length leaf")
            .der()
            .to_vec();

        validate_attestation_certificate_chain(
            &[leaf, intermediate],
            &[restrictive_root, permissive_root],
            &HashSet::new(),
            evaluation_time(),
        )
        .expect("a restrictive same-subject candidate must not shadow a valid anchor");
    }

    #[test]
    fn exact_cross_signed_trust_anchor_does_not_require_self_signature() {
        let old_key =
            KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256).expect("generate old rollover key");
        let old_issuer = Issuer::new(
            ca_certificate_params("Kagemusha Cross-Signed Rollover Root"),
            old_key,
        );
        let new_key =
            KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256).expect("generate new rollover key");
        let new_params = ca_certificate_params("Kagemusha Cross-Signed Rollover Root");
        let cross_signed_anchor = new_params
            .signed_by(&new_key, &old_issuer)
            .expect("cross-sign rollover anchor")
            .der()
            .to_vec();
        let new_issuer = Issuer::new(new_params, new_key);
        let leaf_key =
            KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256).expect("generate rollover leaf key");
        let leaf = leaf_certificate_params("Kagemusha Cross-Signed Rollover Leaf")
            .signed_by(&leaf_key, &new_issuer)
            .expect("sign rollover leaf")
            .der()
            .to_vec();

        validate_attestation_certificate_chain(
            &[leaf, cross_signed_anchor.clone()],
            &[cross_signed_anchor],
            &HashSet::new(),
            evaluation_time(),
        )
        .expect("an exact out-of-band anchor pin need not be self-signed");
    }

    #[test]
    fn parsed_critical_and_path_processing_extensions_fail_closed() {
        let mut critical_eku = CustomExtension::from_oid_content(
            &[2, 5, 29, 37],
            vec![
                0x30, 0x0A, 0x06, 0x08, 0x2B, 0x06, 0x01, 0x05, 0x05, 0x07, 0x03, 0x03,
            ],
        );
        critical_eku.set_criticality(true);
        let critical_eku_der = self_signed_leaf_certificate(
            vec![critical_eku],
            vec![KeyUsagePurpose::DigitalSignature],
        );
        let critical_eku_certificate =
            parse_x509_certificate_der(&critical_eku_der).expect("parse critical EKU certificate");
        let error = validate_x509_certificate_critical_extensions(&critical_eku_certificate)
            .expect_err("a parsed but unsupported critical EKU must be rejected");
        assert!(error.to_string().contains("unsupported critical extension"));

        let noncritical_name_constraints = CustomExtension::from_oid_content(
            &[2, 5, 29, 30],
            vec![
                0x30, 0x0E, 0xA0, 0x0C, 0x30, 0x0A, 0x82, 0x08, b'.', b'e', b'x', b'a', b'm', b'p',
                b'l', b'e',
            ],
        );
        let name_constraints_der = self_signed_leaf_certificate(
            vec![noncritical_name_constraints],
            vec![KeyUsagePurpose::DigitalSignature],
        );
        let name_constraints_certificate = parse_x509_certificate_der(&name_constraints_der)
            .expect("parse noncritical name constraints certificate");
        let error = validate_x509_certificate_critical_extensions(&name_constraints_certificate)
            .expect_err("unprocessed noncritical name constraints must be rejected");
        assert!(error.to_string().contains("path-processing extension"));

        let certificate_policies = CustomExtension::from_oid_content(
            &[2, 5, 29, 32],
            vec![0x30, 0x08, 0x30, 0x06, 0x06, 0x04, 0x55, 0x1D, 0x20, 0x00],
        );
        let policies_der = self_signed_leaf_certificate(
            vec![certificate_policies],
            vec![KeyUsagePurpose::DigitalSignature],
        );
        let policies_certificate =
            parse_x509_certificate_der(&policies_der).expect("parse certificate policies");
        validate_x509_certificate_critical_extensions(&policies_certificate)
            .expect_err("unprocessed certificate policy semantics must be rejected");
    }

    #[test]
    fn duplicate_extension_oids_are_rejected() {
        let extension =
            CustomExtension::from_oid_content(&[1, 3, 6, 1, 4, 1, 55_555, 1], vec![0x05, 0x00]);
        let der = self_signed_leaf_certificate(
            vec![extension.clone(), extension],
            vec![KeyUsagePurpose::DigitalSignature],
        );
        let certificate = parse_x509_certificate_der(&der).expect("parse duplicate extensions");
        let error = validate_x509_certificate_critical_extensions(&certificate)
            .expect_err("duplicate extension OIDs must be rejected");
        assert!(error.to_string().contains("duplicate extension OIDs"));
    }

    #[test]
    fn leaf_ca_and_key_cert_sign_claims_are_rejected_explicitly() {
        let noncritical_ca =
            CustomExtension::from_oid_content(&[2, 5, 29, 19], vec![0x30, 0x03, 0x01, 0x01, 0xFF]);
        let ca_der = self_signed_leaf_certificate(
            vec![noncritical_ca],
            vec![KeyUsagePurpose::DigitalSignature],
        );
        let ca_certificate =
            parse_x509_certificate_der(&ca_der).expect("parse noncritical leaf CA claim");
        validate_x509_leaf_certificate_profile(&ca_certificate)
            .expect_err("a leaf must not bypass CA rejection with noncritical constraints");

        let path_len_without_ca =
            CustomExtension::from_oid_content(&[2, 5, 29, 19], vec![0x30, 0x03, 0x02, 0x01, 0x00]);
        let path_len_der = self_signed_leaf_certificate(
            vec![path_len_without_ca],
            vec![KeyUsagePurpose::DigitalSignature],
        );
        let path_len_certificate =
            parse_x509_certificate_der(&path_len_der).expect("parse leaf path length constraint");
        validate_x509_leaf_certificate_profile(&path_len_certificate)
            .expect_err("a leaf must not assert a path length constraint");

        let key_cert_sign_der = self_signed_leaf_certificate(
            Vec::new(),
            vec![
                KeyUsagePurpose::DigitalSignature,
                KeyUsagePurpose::KeyCertSign,
            ],
        );
        let key_cert_sign_certificate =
            parse_x509_certificate_der(&key_cert_sign_der).expect("parse leaf keyCertSign claim");
        validate_x509_leaf_certificate_profile(&key_cert_sign_certificate)
            .expect_err("a leaf must not assert keyCertSign");
    }

    #[test]
    fn android_keymint_uses_only_a_directly_attested_leaf_extension() {
        let direct = android_extension_chain_fixture(false, true);
        validate_attestation_certificate_chain(
            &[direct.leaf.clone(), direct.intermediate.clone()],
            std::slice::from_ref(&direct.root),
            &HashSet::new(),
            evaluation_time(),
        )
        .expect("directly attested Android leaf chain is valid");
        let extension =
            android_keymint_leaf_attestation_extension(&[direct.leaf, direct.intermediate])
                .expect("select direct Android leaf attestation extension");
        assert_eq!(extension.as_slice(), &[0x30, 0x00]);

        let ca_only = android_extension_chain_fixture(true, false);
        validate_attestation_certificate_chain(
            &[ca_only.leaf.clone(), ca_only.intermediate.clone()],
            std::slice::from_ref(&ca_only.root),
            &HashSet::new(),
            evaluation_time(),
        )
        .expect("attestation-CA-only chain is cryptographically valid");
        android_keymint_leaf_attestation_extension(&[ca_only.leaf, ca_only.intermediate])
            .expect_err("an attestation CA extension does not attest the assertion leaf key");

        let attacker_extended = android_extension_chain_fixture(true, true);
        validate_attestation_certificate_chain(
            &[
                attacker_extended.leaf.clone(),
                attacker_extended.intermediate.clone(),
            ],
            std::slice::from_ref(&attacker_extended.root),
            &HashSet::new(),
            evaluation_time(),
        )
        .expect("attacker-extended Android chain is cryptographically valid");
        let error = android_keymint_leaf_attestation_extension(&[
            attacker_extended.leaf,
            attacker_extended.intermediate,
        ])
        .expect_err("the root-nearest CA extension must shadow an attacker-added leaf extension");
        assert!(
            error
                .to_string()
                .contains("root-nearest attestation extension")
        );
    }

    #[test]
    fn raw_tbs_digest_is_the_revocation_identity_for_both_chain_profiles() {
        let fixture = android_certificate_chain_profile_fixture(
            AndroidCertificateChainTestKind::Factory,
            2035,
            2020,
            2035,
        );
        let intermediate = parse_x509_certificate_der(&fixture.intermediate)
            .expect("parse revocation-target intermediate");
        let revoked = HashSet::from([sha256_bytes(intermediate.tbs_certificate.as_ref())]);

        let generic_error = validate_attestation_certificate_chain(
            &[fixture.leaf.clone(), fixture.intermediate.clone()],
            std::slice::from_ref(&fixture.root),
            &revoked,
            android_profile_evaluation_time(),
        )
        .expect_err("raw TBS revocation must reject the generic chain");
        assert!(generic_error.to_string().contains("revoked"));

        let android_error = validate_android_key_attestation_certificate_chain(
            &[fixture.leaf, fixture.intermediate],
            std::slice::from_ref(&fixture.root),
            &revoked,
            &HashSet::new(),
            android_profile_evaluation_time(),
            android_profile_registration_expiry_time(),
        )
        .expect_err("raw TBS revocation must reject the Android chain");
        assert!(android_error.to_string().contains("revoked"));
    }

    #[test]
    fn android_status_projection_rejects_every_presented_certificate_serial() {
        let fixture = android_certificate_chain_profile_fixture(
            AndroidCertificateChainTestKind::Factory,
            2035,
            2020,
            2035,
        );
        let cases = [
            (
                "REVOKED",
                vec![fixture.leaf.clone(), fixture.intermediate.clone()],
                fixture.leaf.clone(),
            ),
            (
                "SUSPENDED",
                vec![fixture.leaf.clone(), fixture.intermediate.clone()],
                fixture.intermediate.clone(),
            ),
            (
                "REVOKED root",
                vec![
                    fixture.leaf.clone(),
                    fixture.intermediate.clone(),
                    fixture.root.clone(),
                ],
                fixture.root.clone(),
            ),
            (
                "SUSPENDED external root",
                vec![fixture.leaf.clone(), fixture.intermediate.clone()],
                fixture.root.clone(),
            ),
        ];
        for (projected_status, chain, denied_certificate) in cases {
            let denied_certificate = parse_x509_certificate_der(&denied_certificate)
                .expect("parse status-projected certificate");
            let serial = x509_certificate_canonical_serial_hex(&denied_certificate);
            assert!(matches!(serial.as_str(), "a" | "b" | "c"));
            let error = validate_android_key_attestation_certificate_chain(
                &chain,
                std::slice::from_ref(&fixture.root),
                &HashSet::new(),
                &HashSet::from([serial]),
                android_profile_evaluation_time(),
                android_profile_registration_expiry_time(),
            )
            .expect_err("every governed non-valid serial must fail closed");
            assert!(
                error.to_string().contains("non-valid"),
                "{projected_status} projection was not rejected: {error}"
            );
        }
    }

    #[test]
    fn android_factory_expiration_exception_requires_exact_legacy_google_root() {
        let fixture = android_certificate_chain_profile_fixture(
            AndroidCertificateChainTestKind::Factory,
            2035,
            2020,
            2025,
        );
        let parsed_chain = [&fixture.leaf, &fixture.intermediate]
            .into_iter()
            .map(|certificate_der| parse_x509_certificate_der(certificate_der))
            .collect::<Result<Vec<_>, _>>()
            .expect("parse factory chain");
        let legacy_root_der = decode_trusted_root_der(ANDROID_KEY_ATTESTATION_ROOT_CA_DER_B64)
            .expect("decode legacy Google root");
        let legacy_root =
            parse_x509_certificate_der(&legacy_root_der).expect("parse legacy Google root");
        validate_android_key_attestation_certificate_chain_time_profile(
            &parsed_chain,
            false,
            &legacy_root,
            &legacy_root_der,
            android_profile_evaluation_time(),
            android_profile_registration_expiry_time(),
        )
        .expect("legacy Google factory chains may retain expired non-target certificates");

        let nonlegacy_root =
            parse_x509_certificate_der(&fixture.root).expect("parse non-legacy test root");
        let error = validate_android_key_attestation_certificate_chain_time_profile(
            &parsed_chain,
            false,
            &nonlegacy_root,
            &fixture.root,
            android_profile_evaluation_time(),
            android_profile_registration_expiry_time(),
        )
        .expect_err("a non-legacy root must not receive the factory expiration exception");
        assert!(error.to_string().contains("expired"));

        let future_fixture = android_certificate_chain_profile_fixture(
            AndroidCertificateChainTestKind::Factory,
            2035,
            2030,
            2035,
        );
        let future_chain = [&future_fixture.leaf, &future_fixture.intermediate]
            .into_iter()
            .map(|certificate_der| parse_x509_certificate_der(certificate_der))
            .collect::<Result<Vec<_>, _>>()
            .expect("parse future factory chain");
        let error = validate_android_key_attestation_certificate_chain_time_profile(
            &future_chain,
            false,
            &legacy_root,
            &legacy_root_der,
            android_profile_evaluation_time(),
            android_profile_registration_expiry_time(),
        )
        .expect_err("the legacy factory exception must never admit a not-yet-valid issuer");
        assert!(error.to_string().contains("not yet valid"));
    }

    #[test]
    fn android_factory_validity_rejects_first_millisecond_after_not_after() {
        let fixture = android_certificate_chain_profile_fixture(
            AndroidCertificateChainTestKind::Factory,
            2035,
            2020,
            2028,
        );
        let parsed_chain = [&fixture.leaf, &fixture.intermediate]
            .into_iter()
            .map(|certificate_der| parse_x509_certificate_der(certificate_der))
            .collect::<Result<Vec<_>, _>>()
            .expect("parse factory millisecond-boundary chain");
        let root = parse_x509_certificate_der(&fixture.root)
            .expect("parse factory millisecond-boundary root");
        let intermediate_not_after_ms =
            u64::try_from(parsed_chain[1].validity().not_after.timestamp())
                .expect("positive intermediate notAfter fixture")
                .checked_mul(1_000)
                .expect("intermediate notAfter milliseconds");
        let exact_not_after = x509_evaluation_time(intermediate_not_after_ms)
            .expect("exact intermediate notAfter evaluation time");

        validate_android_key_attestation_certificate_chain_time_profile(
            &parsed_chain,
            false,
            &root,
            &fixture.root,
            exact_not_after,
            exact_not_after,
        )
        .expect("a factory issuer is valid at its exact notAfter instant");

        let first_expired_millisecond = x509_evaluation_time(intermediate_not_after_ms + 1)
            .expect("first expired evaluation time");
        let error = validate_android_key_attestation_certificate_chain_time_profile(
            &parsed_chain,
            false,
            &root,
            &fixture.root,
            first_expired_millisecond,
            first_expired_millisecond,
        )
        .expect_err("the first millisecond after a factory issuer notAfter must be rejected");
        assert!(error.to_string().contains("expired"));
    }

    #[test]
    fn android_rkp_chain_must_remain_valid_through_registration_expiry() {
        let fixture = android_certificate_chain_profile_fixture(
            AndroidCertificateChainTestKind::RemoteKeyProvisioning,
            2035,
            2020,
            2028,
        );
        let error = validate_android_key_attestation_certificate_chain(
            &[fixture.leaf, fixture.intermediate],
            std::slice::from_ref(&fixture.root),
            &HashSet::new(),
            &HashSet::new(),
            android_profile_evaluation_time(),
            android_profile_registration_expiry_time(),
        )
        .expect_err("RKP non-target certificates must cover the registration lifetime");
        assert!(error.to_string().contains("registration expiry"));
    }

    #[test]
    fn android_target_leaf_validity_is_not_a_chain_admission_clock() {
        let fixture = android_certificate_chain_profile_fixture(
            AndroidCertificateChainTestKind::RemoteKeyProvisioning,
            2025,
            2020,
            2035,
        );
        validate_android_key_attestation_certificate_chain(
            &[fixture.leaf, fixture.intermediate],
            std::slice::from_ref(&fixture.root),
            &HashSet::new(),
            &HashSet::new(),
            android_profile_evaluation_time(),
            android_profile_registration_expiry_time(),
        )
        .expect("Android target leaf validity must be skipped");
    }

    #[test]
    fn android_unknown_and_ambiguous_chain_classifiers_fail_closed() {
        for (kind, expected) in [
            (AndroidCertificateChainTestKind::Unknown, "unknown"),
            (AndroidCertificateChainTestKind::Ambiguous, "ambiguous"),
        ] {
            let fixture = android_certificate_chain_profile_fixture(kind, 2035, 2020, 2035);
            let error = validate_android_key_attestation_certificate_chain(
                &[fixture.leaf, fixture.intermediate],
                std::slice::from_ref(&fixture.root),
                &HashSet::new(),
                &HashSet::new(),
                android_profile_evaluation_time(),
                android_profile_registration_expiry_time(),
            )
            .expect_err("unrecognized Android chain provenance must fail closed");
            assert!(error.to_string().contains(expected), "{error}");
        }
    }

    #[test]
    fn certificate_chain_size_bounds_are_enforced_before_parsing() {
        let fixture = same_subject_root_fixture();
        let excessive_chain =
            vec![fixture.leaf.clone(); OFFLINE_ATTESTATION_MAX_X509_CHAIN_CERTIFICATES + 1];
        let error = validate_attestation_certificate_chain(
            &excessive_chain,
            std::slice::from_ref(&fixture.new_root),
            &HashSet::new(),
            evaluation_time(),
        )
        .expect_err("an excessive certificate count must be rejected before parsing");
        assert!(
            error
                .to_string()
                .contains("size is outside protocol bounds")
        );

        let oversized_certificate = vec![0u8; OFFLINE_ATTESTATION_MAX_X509_CERTIFICATE_BYTES + 1];
        let error = validate_attestation_certificate_chain(
            &[oversized_certificate],
            &[fixture.new_root],
            &HashSet::new(),
            evaluation_time(),
        )
        .expect_err("an oversized certificate must be rejected before DER parsing");
        assert!(
            error
                .to_string()
                .contains("DER size is outside protocol bounds")
        );
    }

    #[test]
    fn included_trust_anchor_enforces_path_len_constraint() {
        let permitted = certificate_chain_fixture(1);
        validate_attestation_certificate_chain(
            &[
                permitted.leaf,
                permitted.intermediate,
                permitted.root.clone(),
            ],
            &[permitted.root],
            &HashSet::new(),
            evaluation_time(),
        )
        .expect("pathLenConstraint=1 permits one subordinate CA");

        let denied = certificate_chain_fixture(0);
        let error = validate_attestation_certificate_chain(
            &[denied.leaf, denied.intermediate, denied.root.clone()],
            &[denied.root],
            &HashSet::new(),
            evaluation_time(),
        )
        .expect_err("pathLenConstraint=0 must reject one subordinate CA");
        assert!(error.to_string().contains("path length constraint"));
    }

    #[test]
    fn external_trust_anchor_enforces_path_len_constraint() {
        let permitted = certificate_chain_fixture(1);
        validate_attestation_certificate_chain(
            &[permitted.leaf, permitted.intermediate],
            &[permitted.root],
            &HashSet::new(),
            evaluation_time(),
        )
        .expect("external pathLenConstraint=1 permits one subordinate CA");

        let denied = certificate_chain_fixture(0);
        let error = validate_attestation_certificate_chain(
            &[denied.leaf, denied.intermediate],
            &[denied.root],
            &HashSet::new(),
            evaluation_time(),
        )
        .expect_err("external pathLenConstraint=0 must reject one subordinate CA");
        assert!(error.to_string().contains("path length constraint"));
    }
}
fn validate_attestation_protocol_string(
    subject: &'static str,
    field: &'static str,
    value: &str,
    error_label: &'static str,
) -> Result<(), InstructionExecutionError> {
    if value.trim().is_empty() {
        return Err(labeled_invariant(
            error_label,
            format!("{subject} {field} must be non-empty"),
        ));
    }
    if value.trim() != value {
        return Err(labeled_invariant(
            error_label,
            format!("{subject} {field} must not contain surrounding whitespace"),
        ));
    }
    Ok(())
}
fn is_kagemusha_transparent_backend(backend: &str) -> bool {
    backend == crate::zk::ZK_BACKEND_HALO2_IPA || crate::zk::is_stark_fri_v1_backend(backend)
}
fn ensure_kagemusha_transparent_backend(
    backend: &str,
    backend_tag: BackendTag,
) -> Result<(), Error> {
    if crate::zk::is_production_claim_backend_label(backend) {
        return Err(labeled_invariant(
            "verifier_key_invalid",
            "offline transparent proofs may not use readiness-claim proof backends",
        )
        .into());
    }
    if !is_kagemusha_transparent_backend(backend) {
        return Err(labeled_invariant(
            "verifier_key_invalid",
            "offline recursive proofs require a transparent halo2/ipa or stark/fri backend",
        )
        .into());
    }
    let expected_tag = crate::zk::verifier_backend_registry_tag_v1(backend).ok_or_else(|| {
        labeled_invariant(
            "verifier_key_invalid",
            "offline recursive proof backend is not admitted by the native verifier registry",
        )
    })?;
    if backend_tag != expected_tag {
        return Err(labeled_invariant(
            "verifier_key_invalid",
            "offline recursive verifier backend tag does not match the transparent backend",
        )
        .into());
    }
    Ok(())
}
fn ensure_kagemusha_transparent_attachment(attachment: &ProofAttachment) -> Result<(), Error> {
    if attachment.backend != attachment.proof.backend
        || attachment.backend != attachment.vk_ref.backend
    {
        return Err(labeled_invariant(
            "proof_binding",
            "Kagemusha proof backend, proof payload backend, and verifier key backend must match",
        )
        .into());
    }
    if attachment.vk_ref.name.trim().is_empty() {
        return Err(labeled_invariant(
            "verifier_key_invalid",
            "Kagemusha proof verifier key id name must be non-empty",
        )
        .into());
    }
    let backend = attachment.backend.as_str();
    let backend_tag = crate::zk::verifier_backend_registry_tag_v1(backend).ok_or_else(|| {
        labeled_invariant(
            "verifier_key_invalid",
            "Kagemusha proof backend is not a supported generic OpenVerify engine",
        )
    })?;
    ensure_kagemusha_transparent_backend(backend, backend_tag)
}
