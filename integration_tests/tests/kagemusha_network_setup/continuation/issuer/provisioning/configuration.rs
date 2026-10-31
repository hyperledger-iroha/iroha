//! Exact issuer TOML from authenticated originals and explicit runtime selections.
use super::*;

pub(super) struct Originals<'a> {
    pub scheme: &'a KagemushaWalletSchemeV1,
    pub app: &'a KagemushaWalletAppPolicyV1,
    pub enrollment: &'a KagemushaWalletEnrollmentPolicyTemplateV1,
    pub certificate: &'a KagemushaWalletSignerCertificateV1,
    pub template: &'a [u8],
    pub manifest: [u8; 32],
    pub scopes: &'a scopes::Scopes,
}

pub(super) fn encode(
    request: &Request,
    output_path: &Path,
    originals: Originals<'_>,
) -> Result<Vec<u8>> {
    let Originals {
        scheme,
        app,
        enrollment,
        certificate,
        template,
        manifest,
        scopes,
    } = originals;
    let mut provider = Table::new();
    for (name, value) in [
        ("eligibility_template_hex", hex::encode(template)),
        ("scheme_hex", hex::encode(scheme.to_canonical_bytes()?)),
        ("app_hex", hex::encode(app.encode_canonical()?)),
        (
            "enrollment_template_hex",
            hex::encode(enrollment.encode_canonical()?),
        ),
        (
            "certificate_hex",
            hex::encode(certificate.to_canonical_bytes()?),
        ),
        ("manifest_digest_hex", hex::encode(manifest)),
        ("release_digest_hex", hex::encode(scopes.release_digest)),
        (
            "service_origin_digest_hex",
            hex::encode(scopes.service_origin_digest),
        ),
        ("observation_endpoint", request.observation_endpoint.clone()),
        (
            "observation_credential",
            path_text(&request.observation_credential)?,
        ),
        (
            "signer_private_key",
            path_text(&output_path.join("enrollment-signer.key"))?,
        ),
    ] {
        text(&mut provider, name, value);
    }
    if let Some(root) = &request.observation_tls_root {
        text(
            &mut provider,
            "observation_tls_root",
            path_text(&root.path)?,
        );
    }
    let mut worker = Table::new();
    for (name, value) in [
        (
            "python_executable",
            path_text(&request.worker.python_executable)?,
        ),
        (
            "python_sha256_hex",
            request.worker.python_sha256_hex.clone(),
        ),
        (
            "verifier_archive",
            path_text(&request.worker.verifier_archive)?,
        ),
        (
            "verifier_sha256_hex",
            request.worker.verifier_sha256_hex.clone(),
        ),
        (
            "openssl_executable",
            path_text(&request.worker.openssl_executable)?,
        ),
        (
            "openssl_sha256_hex",
            request.worker.openssl_sha256_hex.clone(),
        ),
        (
            "store_directory",
            path_text(&request.worker.store_directory)?,
        ),
    ] {
        text(&mut worker, name, value);
    }
    number(
        &mut worker,
        "exchange_timeout_ms",
        request.worker.exchange_timeout_ms,
    )?;
    provider.insert("worker".into(), toml::Value::Table(worker));
    let mut issuer = Table::new();
    number(&mut issuer, "revision", 1)?;
    text(&mut issuer, "scope_hex", request.scope_hex.clone());
    text(
        &mut issuer,
        "journal_dir",
        path_text(&output_path.join("journal"))?,
    );
    number(
        &mut issuer,
        "request_timeout_ms",
        request.maximum_response_ms,
    )?;
    number(&mut issuer, "max_inflight", 1)?;
    issuer.insert(
        "providers".into(),
        toml::Value::Array(vec![toml::Value::Table(provider)]),
    );
    let torii = Table::from_iter([("kagemusha_enrollment".into(), toml::Value::Table(issuer))]);
    let layer = Table::from_iter([("torii".into(), toml::Value::Table(torii))]);
    let configuration = toml::to_string(&layer)?.into_bytes();
    ensure!(
        parse_layer(&configuration, scheme, manifest)? == layer,
        "issuer layer roundtrip"
    );
    Ok(configuration)
}
