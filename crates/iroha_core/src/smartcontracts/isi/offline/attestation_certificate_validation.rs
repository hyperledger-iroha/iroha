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
fn policy_revoked_certificate_hashes(
    policy: &OfflineDeviceAttestationPolicy,
) -> Result<HashSet<[u8; 32]>, Error> {
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
    Ok(revoked)
}
fn x509_evaluation_time(block_unix_timestamp_ms: u64) -> Result<ASN1Time, Error> {
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
        .into()
    })
}
fn parse_x509_certificate_der(certificate_der: &[u8]) -> Result<X509Certificate<'_>, Error> {
    let (remaining, certificate) = X509Certificate::from_der(certificate_der)
        .map_err(|_| invalid_attestation("attestation certificate DER is invalid"))?;
    reject_invalid_attestation!(
        !remaining.is_empty(),
        "attestation certificate DER has trailing bytes",
    );
    Ok(certificate)
}
fn validate_x509_certificate_critical_extensions(
    certificate: &X509Certificate<'_>,
) -> Result<(), Error> {
    for extension in certificate.extensions() {
        if !extension.critical {
            continue;
        }
        match extension.parsed_extension() {
            ParsedExtension::UnsupportedExtension { .. }
            | ParsedExtension::ParseError { .. }
            | ParsedExtension::Unparsed => {
                return Err(invalid_attestation(
                    "attestation certificate contains an unsupported critical extension",
                )
                .into());
            }
            _ => {}
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
fn x509_leaf_allows_digital_signature(
    certificate: &X509Certificate<'_>,
) -> Result<bool, Error> {
    let Some(key_usage) = certificate
        .key_usage()
        .map_err(|_| invalid_attestation("attestation certificate key usage is invalid"))?
    else {
        return Ok(false);
    };
    Ok(key_usage.critical && key_usage.value.digital_signature())
}
fn validate_x509_certificate_time(
    certificate: &X509Certificate<'_>,
    evaluation_time: ASN1Time,
) -> Result<(), Error> {
    if certificate.validity().is_valid_at(evaluation_time) {
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
    revoked_certificate_sha256: &HashSet<[u8; 32]>,
    evaluation_time: ASN1Time,
) -> Result<(), Error> {
    if certificate_chain.is_empty() || trusted_roots_der.is_empty() {
        return Err(invalid_attestation("attestation certificate chain is empty").into());
    }
    let mut seen = HashSet::new();
    for certificate_der in certificate_chain {
        let certificate_sha256 = sha256_bytes(certificate_der);
        if revoked_certificate_sha256.contains(&certificate_sha256) {
            return Err(labeled_invariant(
                "revoked_attestation",
                "attestation certificate is revoked by Offline device attestation policy",
            )
            .into());
        }
        if !seen.insert(certificate_sha256) {
            return Err(invalid_attestation(
                "attestation certificate chain contains duplicate certificates",
            )
            .into());
        }
        let certificate = parse_x509_certificate_der(certificate_der)?;
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
    if x509_certificate_is_ca(leaf)? || !x509_leaf_allows_digital_signature(leaf)? {
        return Err(invalid_attestation(
            "attestation leaf certificate must be an end-entity signing certificate",
        )
        .into());
    }
    for pair in parsed_chain.windows(2) {
        let certificate = &pair[0];
        let issuer = &pair[1];
        if certificate.issuer() != issuer.subject() || !x509_certificate_is_ca(issuer)? {
            return Err(
                invalid_attestation("attestation certificate issuer chain is invalid").into(),
            );
        }
        verify_x509_certificate_signature(certificate, issuer)?;
    }
    let tail_der = certificate_chain.last().expect("chain is non-empty");
    let tail = parsed_chain.last().expect("chain is non-empty");
    for root_der in trusted_roots_der {
        if revoked_certificate_sha256.contains(&sha256_bytes(root_der)) {
            continue;
        }
        let root = parse_x509_certificate_der(root_der)?;
        validate_x509_certificate_critical_extensions(&root)?;
        validate_x509_certificate_time(&root, evaluation_time)?;
        if !x509_certificate_is_ca(&root)? {
            continue;
        }
        if tail_der == root_der {
            if tail.issuer() == tail.subject() {
                verify_x509_certificate_signature(tail, tail)?;
            }
            return Ok(());
        }
        if tail.issuer() == root.subject() {
            verify_x509_certificate_signature(tail, &root)?;
            return Ok(());
        }
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
#[cfg(test)]
fn x509_certificate_is_offline_attestation_test_root(
    certificate: &X509Certificate<'_>,
) -> bool {
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
fn x509_subject_public_key_bytes(certificate: &X509Certificate<'_>) -> Vec<u8> {
    certificate.public_key().subject_public_key.data.to_vec()
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
    if crate::zk::is_verifier_readiness_claim_label(backend) {
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
    let backend_tag =
        crate::zk::verifier_backend_registry_tag_v1(backend).ok_or_else(|| {
            labeled_invariant(
                "verifier_key_invalid",
                "Kagemusha proof backend is not a supported generic OpenVerify engine",
            )
        })?;
    ensure_kagemusha_transparent_backend(backend, backend_tag)
}

