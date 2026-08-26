//! SNNet-15PQ gateway readiness helper.
//!
//! Generates a post-quantum readiness summary for SoraGlobal gateway PoPs by
//! validating the supplied SRCv2 bundle, TLS/ECH artefacts, and trustless
//! verifier configuration, then emitting JSON/Markdown evidence for runbooks.
use blake3::Hasher as Blake3;
use ed25519_dalek::VerifyingKey;
use eyre::{Result, WrapErr, eyre};
use iroha_crypto::soranet::{
    certificate::RelayCertificateBundleV2, directory::compute_issuer_fingerprint,
};
use norito::json::{self, Map, Value};
use sorafs_car::trustless::TrustlessVerifierConfig;
use soranet_pq::MlDsaSuite;
use std::{
    fs,
    path::{Path, PathBuf},
};
/// Execution options for the gateway PQ readiness helper.
#[derive(Debug)]
pub struct GatewayPqOptions {
    /// Output directory for generated artefacts.
    pub output_dir: PathBuf,
    /// Human-readable PoP label used in the summaries.
    pub pop: String,
    /// Path to the SRCv2 bundle (CBOR).
    pub srcv2_bundle: PathBuf,
    /// Canonical lowercase hex for the independently trusted issuer Ed25519 public key.
    pub issuer_ed25519_hex: String,
    /// Canonical lowercase hex for the independently trusted issuer ML-DSA-65 public key.
    pub issuer_mldsa65_hex: String,
    /// Directory containing the TLS/ECH bundle (fullchain.pem, privkey.pem, ech.json).
    pub tls_bundle_dir: PathBuf,
    /// Trustless verifier config (TOML).
    pub trustless_config: PathBuf,
    /// Explicit Unix second at which the SRCv2 certificate must be valid.
    pub at_unix: i64,
}
/// Paths to generated outputs.
#[derive(Debug)]
pub struct GatewayPqOutcome {
    /// JSON summary path.
    pub summary_json: PathBuf,
    /// Markdown summary path.
    pub summary_markdown: PathBuf,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ComponentState {
    Ok,
    Warn,
    Error,
}
impl ComponentState {
    fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Warn => "warn",
            Self::Error => "error",
        }
    }
    fn elevate(self, other: ComponentState) -> ComponentState {
        match (self, other) {
            (ComponentState::Error, _) | (_, ComponentState::Error) => ComponentState::Error,
            (ComponentState::Warn, _) | (_, ComponentState::Warn) => ComponentState::Warn,
            _ => ComponentState::Ok,
        }
    }
}
/// Generate the SNNet-15PQ readiness bundle.
pub fn run_gateway_pq_readiness(options: GatewayPqOptions) -> Result<GatewayPqOutcome> {
    fs::create_dir_all(&options.output_dir).wrap_err_with(|| {
        format!(
            "failed to create PQ readiness output directory `{}`",
            options.output_dir.display()
        )
    })?;
    let mut overall = ComponentState::Ok;
    let mut summary_root = Map::new();
    summary_root.insert("pop".into(), Value::String(options.pop.clone()));
    let (src_state, src_summary) = readiness_component(load_srcv2_status(
        &options.srcv2_bundle,
        &options.issuer_ed25519_hex,
        &options.issuer_mldsa65_hex,
        options.at_unix,
    ));
    summary_root.insert("srcv2".into(), src_summary);
    overall = overall.elevate(src_state);
    let (tls_state, tls_summary) = readiness_component(load_tls_status(&options.tls_bundle_dir));
    summary_root.insert("tls".into(), tls_summary);
    overall = overall.elevate(tls_state);
    let (trustless_state, trustless_summary) =
        readiness_component(load_trustless_status(&options.trustless_config));
    summary_root.insert("trustless".into(), trustless_summary);
    overall = overall.elevate(trustless_state);
    summary_root.insert("evaluated_at_unix".into(), Value::from(options.at_unix));
    // Dashboards/operators follow the SNNet-16 telemetry artefacts.
    summary_root.insert(
        "dashboards".into(),
        norito::json!({
            "handshake": "dashboards/grafana/soranet_sn16_handshake.json",
            "alerts": "dashboards/alerts/soranet_handshake_rules.yml",
        }),
    );
    summary_root.insert(
        "overall_status".into(),
        Value::String(overall.as_str().to_string()),
    );
    let summary_json = options.output_dir.join("gateway_pq_summary.json");
    let summary_markdown = options.output_dir.join("gateway_pq_summary.md");
    if let Some(parent) = summary_json.parent() {
        fs::create_dir_all(parent)?;
    }
    let json_payload = json::to_string_pretty(&Value::Object(summary_root.clone()))
        .wrap_err("failed to encode PQ readiness JSON")?;
    fs::write(&summary_json, format!("{json_payload}\n")).wrap_err_with(|| {
        format!(
            "failed to write PQ readiness JSON to {}",
            summary_json.display()
        )
    })?;
    let markdown = render_markdown(&summary_root);
    fs::write(&summary_markdown, markdown).wrap_err_with(|| {
        format!(
            "failed to write PQ readiness Markdown to {}",
            summary_markdown.display()
        )
    })?;
    let outcome = GatewayPqOutcome {
        summary_json,
        summary_markdown,
    };
    if overall != ComponentState::Ok {
        return Err(eyre!(
            "SoraNet gateway PQ readiness is {}; evidence written to `{}` and `{}`",
            overall.as_str(),
            outcome.summary_json.display(),
            outcome.summary_markdown.display()
        ));
    }
    Ok(outcome)
}
fn readiness_component(result: Result<(ComponentState, Value)>) -> (ComponentState, Value) {
    result.unwrap_or_else(|error| {
        let error = error.to_string();
        (
            ComponentState::Error,
            norito::json!({
                "state": "error",
                "error": error,
            }),
        )
    })
}
fn load_srcv2_status(
    path: &Path,
    issuer_ed25519_hex: &str,
    issuer_mldsa65_hex: &str,
    at_unix: i64,
) -> Result<(ComponentState, Value)> {
    let bytes = fs::read(path)
        .wrap_err_with(|| format!("failed to read SRCv2 bundle from `{}`", path.display()))?;
    let bundle = RelayCertificateBundleV2::from_cbor(&bytes)
        .wrap_err_with(|| format!("failed to parse SRCv2 bundle from `{}`", path.display()))?;
    let certificate = bundle.certificate.clone();
    let trusted_issuer = parse_trusted_issuer_keys(issuer_ed25519_hex, issuer_mldsa65_hex)?;
    if trusted_issuer.ed25519_bytes == certificate.identity_ed25519
        || trusted_issuer.mldsa65 == certificate.identity_mldsa65
    {
        return Err(eyre!(
            "SRCv2 issuer keys must be operationally distinct from the relay identity keys"
        ));
    }
    if trusted_issuer.fingerprint != certificate.issuer_fingerprint {
        return Err(eyre!(
            "trusted SRCv2 issuer fingerprint {} does not match certificate issuer_fingerprint {}",
            hex::encode(trusted_issuer.fingerprint),
            hex::encode(certificate.issuer_fingerprint)
        ));
    }
    let mut state = ComponentState::Ok;
    let mut details = Map::new();
    details.insert(
        "issuer_fingerprint_hex".into(),
        Value::String(hex::encode(trusted_issuer.fingerprint)),
    );
    details.insert(
        "handshake_suites".into(),
        Value::Array(
            certificate
                .handshake_suites
                .iter()
                .map(|suite| Value::String(format!("{suite:?}")))
                .collect(),
        ),
    );
    let has_pq_suite = certificate.supports_pq_handshake();
    details.insert(
        "pq_handshake_suite_present".into(),
        Value::Bool(has_pq_suite),
    );
    if !has_pq_suite {
        state = ComponentState::Error;
        details.insert(
            "error".into(),
            Value::String("SRCv2 handshake suites missing PQ entry".into()),
        );
    }
    let mut capability_flags = Map::new();
    capability_flags.insert(
        "blinded_cid".into(),
        Value::Bool(certificate.capability_flags.supports_blinded_cid()),
    );
    capability_flags.insert(
        "pow_ticket".into(),
        Value::Bool(certificate.capability_flags.requires_pow_ticket()),
    );
    capability_flags.insert(
        "norito_stream".into(),
        Value::Bool(certificate.capability_flags.supports_norito_stream()),
    );
    capability_flags.insert(
        "kaigi_bridge".into(),
        Value::Bool(certificate.capability_flags.supports_kaigi_bridge()),
    );
    details.insert("capability_flags".into(), Value::Object(capability_flags));
    details.insert("verified_at_unix".into(), Value::from(at_unix));
    // First-release readiness requires both signatures and an in-window certificate.
    match bundle.verify_at(&trusted_issuer.ed25519, &trusted_issuer.mldsa65, at_unix) {
        Ok(()) => {
            details.insert(
                "certificate_and_signatures_valid_at_unix".into(),
                Value::Bool(true),
            );
        }
        Err(err) => {
            details.insert(
                "certificate_and_signatures_valid_at_unix".into(),
                Value::Bool(false),
            );
            details.insert("verification_error".into(), Value::String(err.to_string()));
            state = ComponentState::Error;
        }
    }
    details.insert("state".into(), Value::String(state.as_str().to_string()));
    Ok((state, Value::Object(details)))
}
#[derive(Debug)]
struct TrustedIssuerKeys {
    ed25519_bytes: [u8; ed25519_dalek::PUBLIC_KEY_LENGTH],
    ed25519: VerifyingKey,
    mldsa65: Vec<u8>,
    fingerprint: [u8; 32],
}
fn parse_trusted_issuer_keys(
    issuer_ed25519_hex: &str,
    issuer_mldsa65_hex: &str,
) -> Result<TrustedIssuerKeys> {
    let ed25519_bytes = decode_exact_lowercase_hex(
        issuer_ed25519_hex,
        ed25519_dalek::PUBLIC_KEY_LENGTH,
        "trusted SRCv2 issuer Ed25519 public key",
    )?;
    let ed25519_bytes: [u8; ed25519_dalek::PUBLIC_KEY_LENGTH] = ed25519_bytes
        .try_into()
        .expect("exact Ed25519 public-key length checked above");
    let parsed = iroha_crypto::ed25519_parse_public_key(&ed25519_bytes)
        .map_err(|err| eyre!("invalid trusted SRCv2 issuer Ed25519 public key: {err}"))?;
    let ed25519 = VerifyingKey::from_bytes(parsed.as_bytes())
        .map_err(|err| eyre!("invalid trusted SRCv2 issuer Ed25519 public key: {err}"))?;
    let mldsa65 = decode_exact_lowercase_hex(
        issuer_mldsa65_hex,
        MlDsaSuite::MlDsa65.public_key_len(),
        "trusted SRCv2 issuer ML-DSA-65 public key",
    )?;
    let fingerprint = compute_issuer_fingerprint(&ed25519_bytes, &mldsa65)
        .map_err(|err| eyre!("invalid trusted SRCv2 issuer key pair: {err}"))?;
    Ok(TrustedIssuerKeys {
        ed25519_bytes,
        ed25519,
        mldsa65,
        fingerprint,
    })
}
fn decode_exact_lowercase_hex(value: &str, expected_bytes: usize, label: &str) -> Result<Vec<u8>> {
    let expected_hex = expected_bytes
        .checked_mul(2)
        .ok_or_else(|| eyre!("{label} length overflow"))?;
    if value.len() != expected_hex
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(eyre!(
            "{label} must contain exactly {expected_hex} lowercase hexadecimal characters"
        ));
    }
    let decoded = hex::decode(value).map_err(|err| eyre!("failed to decode {label}: {err}"))?;
    debug_assert_eq!(decoded.len(), expected_bytes);
    Ok(decoded)
}
fn load_tls_status(dir: &Path) -> Result<(ComponentState, Value)> {
    let mut state = ComponentState::Ok;
    let fullchain = dir.join("fullchain.pem");
    let privkey = dir.join("privkey.pem");
    let ech = dir.join("ech.json");
    let fullchain_exists = fullchain.exists();
    let privkey_exists = privkey.exists();
    let ech_exists = ech.exists();
    if !(fullchain_exists && privkey_exists && ech_exists) {
        state = ComponentState::Error;
    }
    let mut details = Map::new();
    details.insert("fullchain_present".into(), Value::Bool(fullchain_exists));
    details.insert("privkey_present".into(), Value::Bool(privkey_exists));
    details.insert("ech_present".into(), Value::Bool(ech_exists));
    if fullchain_exists {
        let digest = file_blake3_hex(&fullchain)?;
        details.insert("fullchain_blake3_hex".into(), Value::String(digest));
    }
    if ech_exists {
        let contents = fs::read_to_string(&ech)
            .wrap_err_with(|| format!("failed to read ECH config from `{}`", ech.display()))?;
        match json::from_str::<Value>(&contents) {
            Ok(value) => {
                let config = value
                    .get("ech_config_b64")
                    .and_then(Value::as_str)
                    .map(|s| s.to_owned());
                details.insert(
                    "ech_config_b64".into(),
                    config.map(Value::String).unwrap_or(Value::Null),
                );
            }
            Err(err) => {
                details.insert(
                    "ech_error".into(),
                    Value::String(format!("failed to parse ech.json: {err}")),
                );
                state = ComponentState::Error;
            }
        }
    }
    details.insert("state".into(), Value::String(state.as_str().to_string()));
    Ok((state, Value::Object(details)))
}
fn load_trustless_status(path: &Path) -> Result<(ComponentState, Value)> {
    let config = TrustlessVerifierConfig::from_file(path)?;
    let mut details = Map::new();
    details.insert(
        "kzg_trusted_setup".into(),
        Value::String(config.kzg_trusted_setup.clone()),
    );
    details.insert(
        "sdr_receipt_dir".into(),
        Value::String(config.sdr_receipt_dir.clone()),
    );
    details.insert(
        "pipeline_reject_stale_cache_versions".into(),
        Value::Bool(config.pipeline_reject_stale_cache_versions),
    );
    details.insert(
        "pipeline_verify_cache_binding_header".into(),
        Value::Bool(config.pipeline_verify_cache_binding_header),
    );
    details.insert(
        "allow_hybrid_manifest".into(),
        Value::Bool(config.pipeline_allow_hybrid_manifest),
    );
    let mut state = ComponentState::Ok;
    if !config.pipeline_reject_stale_cache_versions || !config.pipeline_verify_cache_binding_header
    {
        state = ComponentState::Error;
    }
    if config.sdr_receipt_dir.trim().is_empty() || config.kzg_trusted_setup.trim().is_empty() {
        state = state.elevate(ComponentState::Warn);
    }
    details.insert("state".into(), Value::String(state.as_str().to_string()));
    Ok((state, Value::Object(details)))
}
fn render_markdown(summary: &Map) -> String {
    let pop = summary
        .get("pop")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let overall = summary
        .get("overall_status")
        .and_then(Value::as_str)
        .unwrap_or("error");
    let src_state = component_state(summary, "srcv2");
    let tls_state = component_state(summary, "tls");
    let trustless_state = component_state(summary, "trustless");
    let dashboards = summary.get("dashboards").cloned().unwrap_or(Value::Null);
    let dashboards_text = dashboards_label(&dashboards);
    format!(
        "# SNNet-15PQ Gateway Readiness — {pop}\n\
\n\
- Overall status: **{overall}**\n\
- SRCv2 bundle: {src_state}\n\
- TLS/ECH bundle: {tls_state}\n\
- Trustless verifier: {trustless_state}\n\
- Dashboards: {dashboards_text}\n",
    )
}
fn file_blake3_hex(path: &Path) -> Result<String> {
    let mut hasher = Blake3::new();
    let bytes = fs::read(path)
        .wrap_err_with(|| format!("failed to read `{}` for hashing", path.display()))?;
    hasher.update(&bytes);
    Ok(hasher.finalize().to_hex().to_string())
}
fn component_state(summary: &Map, key: &str) -> String {
    summary
        .get(key)
        .and_then(Value::as_object)
        .and_then(|object| object.get("state"))
        .and_then(Value::as_str)
        .unwrap_or("error")
        .to_string()
}
fn dashboards_label(dashboards: &Value) -> String {
    dashboards
        .as_object()
        .map(|object| {
            let mut entries: Vec<String> = Vec::new();
            if let Some(handshake) = object.get("handshake").and_then(Value::as_str) {
                entries.push(format!("handshake: {handshake}"));
            }
            if let Some(alerts) = object.get("alerts").and_then(Value::as_str) {
                entries.push(format!("alerts: {alerts}"));
            }
            if entries.is_empty() {
                "no dashboard references".to_string()
            } else {
                entries.join(" | ")
            }
        })
        .unwrap_or_else(|| "no dashboard references".to_string())
}
#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::SigningKey;
    use iroha_crypto::soranet::{
        certificate::{RelayCapabilityFlagsV1, RelayCertificateV2},
        handshake::HandshakeSuite,
    };
    use soranet_pq::{HedgedRngSeed, generate_mldsa_keypair_from_seed};
    use tempfile::TempDir;
    const NONCANONICAL_ED25519_IDENTITY: [u8; ed25519_dalek::PUBLIC_KEY_LENGTH] = [
        0xee, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff, 0x7f,
    ];
    const NONCANONICAL_NON_SMALL_ORDER_ED25519_POINT: [u8; ed25519_dalek::PUBLIC_KEY_LENGTH] = [
        0xf0, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff, 0x7f,
    ];
    fn sample_certificate(
        identity_ed25519: [u8; 32],
        identity_mldsa65: Vec<u8>,
        issuer_fingerprint: [u8; 32],
    ) -> RelayCertificateV2 {
        RelayCertificateV2 {
            relay_id: identity_ed25519,
            identity_ed25519,
            identity_mldsa65,
            descriptor_commit: [0x44; 32],
            roles: iroha_crypto::soranet::certificate::RelayRolesV2 {
                entry: true,
                middle: true,
                exit: true,
            },
            guard_weight: 100,
            bandwidth_bytes_per_sec: 500_000,
            reputation_weight: 50,
            endpoints: vec![iroha_crypto::soranet::certificate::RelayEndpointV2 {
                quic_multiaddr: "/dns/relay.example/udp/443/quic".to_string(),
                tls_server_name: "relay.example".to_string(),
                tls_spki_sha256: [0xA5; 32],
                priority: 1,
                tags: vec!["norito-stream".into()],
            }],
            capability_flags: RelayCapabilityFlagsV1::new(
                iroha_crypto::soranet::certificate::CapabilityToggle::Enabled,
                iroha_crypto::soranet::certificate::CapabilityToggle::Enabled,
                iroha_crypto::soranet::certificate::CapabilityToggle::Enabled,
                iroha_crypto::soranet::certificate::CapabilityToggle::Enabled,
            ),
            handshake_suites: vec![
                HandshakeSuite::Nk3PqForwardSecure,
                HandshakeSuite::Nk2Hybrid,
            ],
            published_at: 1_734_000_000,
            valid_after: 1_734_000_000,
            valid_until: 1_734_086_400,
            directory_hash: [0x55; 32],
            issuer_fingerprint,
        }
    }
    struct IssuedBundleFixture {
        bundle: RelayCertificateBundleV2,
        issuer_ed25519_hex: String,
        issuer_mldsa65_hex: String,
    }
    fn issued_bundle_fixture() -> IssuedBundleFixture {
        let relay_ed25519 = SigningKey::from_bytes(&[0x21; 32]).verifying_key();
        let relay_mldsa65 = generate_mldsa_keypair_from_seed(
            MlDsaSuite::MlDsa65,
            HedgedRngSeed::from_entropy([0x31; 32]),
            b"xtask:soranet-gateway-pq:relay-identity",
        )
        .expect("relay ML-DSA keypair");
        let issuer_ed25519 = SigningKey::from_bytes(&[0x41; 32]);
        let issuer_mldsa65 = generate_mldsa_keypair_from_seed(
            MlDsaSuite::MlDsa65,
            HedgedRngSeed::from_entropy([0x51; 32]),
            b"xtask:soranet-gateway-pq:issuer",
        )
        .expect("issuer ML-DSA keypair");
        let issuer_ed25519_bytes = issuer_ed25519.verifying_key().to_bytes();
        let issuer_fingerprint =
            compute_issuer_fingerprint(&issuer_ed25519_bytes, issuer_mldsa65.public_key())
                .expect("issuer fingerprint");
        let certificate = sample_certificate(
            relay_ed25519.to_bytes(),
            relay_mldsa65.public_key().to_vec(),
            issuer_fingerprint,
        );
        let bundle = certificate
            .issue(&issuer_ed25519, issuer_mldsa65.secret_key())
            .expect("issue certificate");
        IssuedBundleFixture {
            bundle,
            issuer_ed25519_hex: hex::encode(issuer_ed25519_bytes),
            issuer_mldsa65_hex: hex::encode(issuer_mldsa65.public_key()),
        }
    }
    fn write_ready_dependencies(root: &Path) -> (PathBuf, PathBuf) {
        let tls_dir = root.join("tls");
        fs::create_dir_all(&tls_dir).expect("tls dir");
        fs::write(tls_dir.join("fullchain.pem"), "CERT").expect("fullchain");
        fs::write(tls_dir.join("privkey.pem"), "KEY").expect("privkey");
        fs::write(
            tls_dir.join("ech.json"),
            r#"{"ech_config_b64":"ZmFrZS1jb25maWc="}"#,
        )
        .expect("ech");
        let trustless_path = root.join("trustless.toml");
        fs::write(
            &trustless_path,
            r#"
version = 1

[merkle]
chunk_window = 16
max_parallel_streams = 4

[kzg]
trusted_setup = "/tmp/kzg.params"
proof_cache = "/tmp/cache"
max_gap_ms = 100

[sdr]
receipt_dir = "/tmp/sdr"
max_lag_seconds = 8

[pipeline]
allow_hybrid_manifest = false
reject_stale_cache_versions = true
verify_cache_binding_header = true

[logging]
level = "info"
emit_metrics = true
"#,
        )
        .expect("trustless config");
        (tls_dir, trustless_path)
    }
    #[test]
    fn generates_readiness_summary() {
        let temp = TempDir::new().expect("tempdir");
        let out_dir = temp.path().join("out");
        let (tls_dir, trustless_path) = write_ready_dependencies(temp.path());
        let issued = issued_bundle_fixture();
        let src_path = temp.path().join("srcv2.cbor");
        fs::write(
            &src_path,
            issued.bundle.try_to_cbor().expect("encode srcv2"),
        )
        .expect("write srcv2");
        let outcome = run_gateway_pq_readiness(GatewayPqOptions {
            output_dir: out_dir.clone(),
            pop: "sjc-01".to_string(),
            srcv2_bundle: src_path,
            issuer_ed25519_hex: issued.issuer_ed25519_hex,
            issuer_mldsa65_hex: issued.issuer_mldsa65_hex,
            tls_bundle_dir: tls_dir,
            trustless_config: trustless_path,
            at_unix: 1_734_000_001,
        })
        .expect("runs readiness");
        let raw = fs::read_to_string(outcome.summary_json).expect("read summary");
        let summary: Value = json::from_str(&raw).expect("parse summary");
        let overall = summary
            .get("overall_status")
            .and_then(Value::as_str)
            .unwrap_or("error");
        assert_eq!(overall, "ok");
        let src = summary
            .get("srcv2")
            .and_then(Value::as_object)
            .expect("srcv2 summary");
        assert_eq!(
            src.get("certificate_and_signatures_valid_at_unix"),
            Some(&Value::Bool(true))
        );
        let tls = summary
            .get("tls")
            .and_then(Value::as_object)
            .expect("tls summary");
        assert_eq!(tls.get("fullchain_present"), Some(&Value::Bool(true)));
        assert_eq!(tls.get("ech_present"), Some(&Value::Bool(true)));
        assert_eq!(
            summary.get("evaluated_at_unix").and_then(Value::as_i64),
            Some(1_734_000_001)
        );
        assert!(summary.get("canary_hosts").is_none());
    }
    #[test]
    fn expired_certificate_fails_after_writing_error_evidence() {
        let temp = TempDir::new().expect("tempdir");
        let out_dir = temp.path().join("out");
        let (tls_dir, trustless_path) = write_ready_dependencies(temp.path());
        let issued = issued_bundle_fixture();
        let src_path = temp.path().join("srcv2.cbor");
        fs::write(
            &src_path,
            issued.bundle.try_to_cbor().expect("encode srcv2"),
        )
        .expect("write srcv2");
        let error = run_gateway_pq_readiness(GatewayPqOptions {
            output_dir: out_dir.clone(),
            pop: "sjc-01".to_owned(),
            srcv2_bundle: src_path,
            issuer_ed25519_hex: issued.issuer_ed25519_hex,
            issuer_mldsa65_hex: issued.issuer_mldsa65_hex,
            tls_bundle_dir: tls_dir,
            trustless_config: trustless_path,
            at_unix: 1_734_086_400,
        })
        .expect_err("a certificate at its exclusive validity end must fail readiness");
        assert!(error.to_string().contains("readiness is error"));
        let summary_json = out_dir.join("gateway_pq_summary.json");
        let summary_markdown = out_dir.join("gateway_pq_summary.md");
        assert!(summary_json.is_file());
        assert!(summary_markdown.is_file());
        let summary: Value = json::from_str(
            &fs::read_to_string(summary_json).expect("read error readiness evidence"),
        )
        .expect("parse error readiness evidence");
        assert_eq!(
            summary.get("overall_status").and_then(Value::as_str),
            Some("error")
        );
        assert_eq!(
            summary
                .get("srcv2")
                .and_then(Value::as_object)
                .and_then(|srcv2| srcv2.get("certificate_and_signatures_valid_at_unix")),
            Some(&Value::Bool(false))
        );
    }
    #[test]
    fn invalid_in_window_signature_fails_after_writing_error_evidence() {
        let temp = TempDir::new().expect("tempdir");
        let out_dir = temp.path().join("out");
        let (tls_dir, trustless_path) = write_ready_dependencies(temp.path());
        let mut issued = issued_bundle_fixture();
        issued.bundle.signatures.ed25519[0] ^= 0x80;
        let src_path = temp.path().join("srcv2.cbor");
        fs::write(
            &src_path,
            issued.bundle.try_to_cbor().expect("encode srcv2"),
        )
        .expect("write srcv2");
        let error = run_gateway_pq_readiness(GatewayPqOptions {
            output_dir: out_dir.clone(),
            pop: "sjc-01".to_owned(),
            srcv2_bundle: src_path,
            issuer_ed25519_hex: issued.issuer_ed25519_hex,
            issuer_mldsa65_hex: issued.issuer_mldsa65_hex,
            tls_bundle_dir: tls_dir,
            trustless_config: trustless_path,
            at_unix: 1_734_000_001,
        })
        .expect_err("an invalid in-window signature must fail readiness");
        assert!(error.to_string().contains("readiness is error"));
        let summary: Value = json::from_str(
            &fs::read_to_string(out_dir.join("gateway_pq_summary.json"))
                .expect("read signature-error evidence"),
        )
        .expect("parse signature-error evidence");
        assert_eq!(
            summary
                .get("srcv2")
                .and_then(Value::as_object)
                .and_then(|srcv2| srcv2.get("certificate_and_signatures_valid_at_unix")),
            Some(&Value::Bool(false))
        );
        assert_eq!(
            summary.get("overall_status").and_then(Value::as_str),
            Some("error")
        );
    }
    #[test]
    fn unreadable_component_fails_after_writing_error_evidence() {
        let temp = TempDir::new().expect("tempdir");
        let out_dir = temp.path().join("out");
        let (tls_dir, trustless_path) = write_ready_dependencies(temp.path());
        let issued = issued_bundle_fixture();
        let error = run_gateway_pq_readiness(GatewayPqOptions {
            output_dir: out_dir.clone(),
            pop: "sjc-01".to_owned(),
            srcv2_bundle: temp.path().join("missing.srcv2.cbor"),
            issuer_ed25519_hex: issued.issuer_ed25519_hex,
            issuer_mldsa65_hex: issued.issuer_mldsa65_hex,
            tls_bundle_dir: tls_dir,
            trustless_config: trustless_path,
            at_unix: 1_734_000_001,
        })
        .expect_err("an unreadable SRCv2 component must fail readiness");
        assert!(error.to_string().contains("readiness is error"));
        let summary: Value = json::from_str(
            &fs::read_to_string(out_dir.join("gateway_pq_summary.json"))
                .expect("read loader-error evidence"),
        )
        .expect("parse loader-error evidence");
        assert_eq!(
            summary
                .get("srcv2")
                .and_then(Value::as_object)
                .and_then(|srcv2| srcv2.get("state"))
                .and_then(Value::as_str),
            Some("error")
        );
    }
    #[test]
    fn missing_trustless_paths_never_demote_policy_error() {
        let temp = TempDir::new().expect("tempdir");
        let trustless_path = temp.path().join("trustless.toml");
        fs::write(
            &trustless_path,
            r#"
version = 1

[merkle]
chunk_window = 16
max_parallel_streams = 4

[kzg]
trusted_setup = ""
proof_cache = "/tmp/cache"
max_gap_ms = 100

[sdr]
receipt_dir = ""
max_lag_seconds = 8

[pipeline]
allow_hybrid_manifest = false
reject_stale_cache_versions = false
verify_cache_binding_header = false

[logging]
level = "info"
emit_metrics = true
"#,
        )
        .expect("write invalid trustless config");
        let (state, summary) =
            load_trustless_status(&trustless_path).expect("parse invalid readiness policy");
        assert_eq!(state, ComponentState::Error);
        assert_eq!(
            summary
                .as_object()
                .and_then(|object| object.get("state"))
                .and_then(Value::as_str),
            Some("error")
        );
    }
    #[test]
    fn rejects_noncanonical_trusted_issuer_ed25519_key() {
        let issued = issued_bundle_fixture();
        for public_key in [
            NONCANONICAL_ED25519_IDENTITY,
            NONCANONICAL_NON_SMALL_ORDER_ED25519_POINT,
        ] {
            let err =
                parse_trusted_issuer_keys(&hex::encode(public_key), &issued.issuer_mldsa65_hex)
                    .expect_err("noncanonical trusted issuer key must fail readiness");
            let message = format!("{err:?}");
            assert!(
                message.contains("invalid trusted SRCv2 issuer Ed25519 public key")
                    && message.contains("non-canonical"),
                "unexpected error: {message}"
            );
        }
    }
    #[test]
    fn trusted_issuer_hex_is_exact_and_canonical() {
        let issued = issued_bundle_fixture();
        let shortened = &issued.issuer_ed25519_hex[..issued.issuer_ed25519_hex.len() - 2];
        assert!(
            parse_trusted_issuer_keys(shortened, &issued.issuer_mldsa65_hex).is_err(),
            "short issuer key must fail"
        );
        assert!(
            parse_trusted_issuer_keys(
                &issued.issuer_ed25519_hex.to_ascii_uppercase(),
                &issued.issuer_mldsa65_hex,
            )
            .is_err(),
            "uppercase issuer key must fail"
        );
        assert!(
            parse_trusted_issuer_keys(
                &issued.issuer_ed25519_hex,
                &issued.issuer_mldsa65_hex[..issued.issuer_mldsa65_hex.len() - 2],
            )
            .is_err(),
            "short ML-DSA issuer key must fail"
        );
    }
    #[test]
    fn readiness_rejects_self_signed_relay_certificate() {
        let temp = TempDir::new().expect("tempdir");
        let relay_ed25519 = SigningKey::from_bytes(&[0x61; 32]);
        let relay_mldsa65 = generate_mldsa_keypair_from_seed(
            MlDsaSuite::MlDsa65,
            HedgedRngSeed::from_entropy([0x71; 32]),
            b"xtask:soranet-gateway-pq:self-signed-relay",
        )
        .expect("relay ML-DSA keypair");
        let relay_ed25519_bytes = relay_ed25519.verifying_key().to_bytes();
        let fingerprint =
            compute_issuer_fingerprint(&relay_ed25519_bytes, relay_mldsa65.public_key())
                .expect("self-signed fingerprint");
        let certificate = sample_certificate(
            relay_ed25519_bytes,
            relay_mldsa65.public_key().to_vec(),
            fingerprint,
        );
        let bundle = certificate
            .issue(&relay_ed25519, relay_mldsa65.secret_key())
            .expect("self-sign relay certificate");
        let src_path = temp.path().join("srcv2.cbor");
        fs::write(&src_path, bundle.try_to_cbor().expect("encode srcv2")).expect("write srcv2");
        let error = load_srcv2_status(
            &src_path,
            &hex::encode(relay_ed25519_bytes),
            &hex::encode(relay_mldsa65.public_key()),
            1_734_000_001,
        )
        .expect_err("self-signed relay identity must never establish issuer trust");
        assert!(
            error.to_string().contains("operationally distinct"),
            "unexpected error: {error:?}"
        );
    }
    #[test]
    fn readiness_rejects_trusted_keys_that_do_not_bind_issuer_fingerprint() {
        let temp = TempDir::new().expect("tempdir");
        let issued = issued_bundle_fixture();
        let src_path = temp.path().join("srcv2.cbor");
        fs::write(
            &src_path,
            issued.bundle.try_to_cbor().expect("encode srcv2"),
        )
        .expect("write srcv2");
        let wrong_ed25519 = SigningKey::from_bytes(&[0x72; 32]);
        let wrong_mldsa65 = generate_mldsa_keypair_from_seed(
            MlDsaSuite::MlDsa65,
            HedgedRngSeed::from_entropy([0x73; 32]),
            b"xtask:soranet-gateway-pq:wrong-issuer",
        )
        .expect("wrong issuer ML-DSA keypair");
        let error = load_srcv2_status(
            &src_path,
            &hex::encode(wrong_ed25519.verifying_key().to_bytes()),
            &hex::encode(wrong_mldsa65.public_key()),
            1_734_000_001,
        )
        .expect_err("unbound trusted issuer keys must fail readiness");
        assert!(
            error.to_string().contains("issuer fingerprint"),
            "unexpected error: {error:?}"
        );
    }
}
