//! SNNet-15PQ gateway readiness helper.
//!
//! Generates a post-quantum readiness summary for SoraGlobal gateway PoPs by
//! validating the supplied SRCv2 bundle, TLS/ECH artefacts, and trustless
//! verifier configuration, then emitting JSON/Markdown evidence for runbooks.

use std::{
    fs,
    path::{Path, PathBuf},
};

use blake3::Hasher as Blake3;
use ed25519_dalek::VerifyingKey;
use eyre::{Result, WrapErr, eyre};
use iroha_crypto::soranet::{
    certificate::{CertificateValidationPhase, RelayCertificateBundleV2},
    handshake::HandshakeSuite,
};
use norito::json::{self, Map, Number, Value};
use sorafs_car::trustless::TrustlessVerifierConfig;

/// Execution options for the gateway PQ readiness helper.
#[derive(Debug)]
pub struct GatewayPqOptions {
    /// Output directory for generated artefacts.
    pub output_dir: PathBuf,
    /// Human-readable PoP label used in the summaries.
    pub pop: String,
    /// Path to the SRCv2 bundle (CBOR).
    pub srcv2_bundle: PathBuf,
    /// Directory containing the TLS/ECH bundle (fullchain.pem, privkey.pem, ech.json).
    pub tls_bundle_dir: PathBuf,
    /// Trustless verifier config (TOML).
    pub trustless_config: PathBuf,
    /// Optional canary hosts to exercise PQ handshakes; defaults derived from the PoP.
    pub canary_hosts: Vec<String>,
    /// Certificate validation phase; defaults to Phase3 (dual signatures required).
    pub validation_phase: CertificateValidationPhase,
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

    let (src_state, src_summary) =
        load_srcv2_status(&options.srcv2_bundle, options.validation_phase)?;
    summary_root.insert("srcv2".into(), src_summary);
    overall = overall.elevate(src_state);

    let (tls_state, tls_summary) = load_tls_status(&options.tls_bundle_dir)?;
    summary_root.insert("tls".into(), tls_summary);
    overall = overall.elevate(tls_state);

    let (trustless_state, trustless_summary) = load_trustless_status(&options.trustless_config)?;
    summary_root.insert("trustless".into(), trustless_summary);
    overall = overall.elevate(trustless_state);

    let canary_hosts = if options.canary_hosts.is_empty() {
        default_canaries(&options.pop)
    } else {
        options.canary_hosts
    };
    summary_root.insert(
        "canary_hosts".into(),
        Value::Array(canary_hosts.into_iter().map(Value::String).collect()),
    );

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

    Ok(GatewayPqOutcome {
        summary_json,
        summary_markdown,
    })
}

fn load_srcv2_status(
    path: &Path,
    phase: CertificateValidationPhase,
) -> Result<(ComponentState, Value)> {
    let bytes = fs::read(path)
        .wrap_err_with(|| format!("failed to read SRCv2 bundle from `{}`", path.display()))?;
    let bundle = RelayCertificateBundleV2::from_cbor(&bytes)
        .wrap_err_with(|| format!("failed to parse SRCv2 bundle from `{}`", path.display()))?;
    let certificate = bundle.certificate.clone();

    let ed_pub = VerifyingKey::from_bytes(&certificate.identity_ed25519)
        .map_err(|err| eyre!("invalid Ed25519 identity key in SRCv2: {err}"))?;
    let mldsa_key = certificate.identity_mldsa65.clone();

    let mut state = ComponentState::Ok;
    let mut details = Map::new();
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
    details.insert(
        "kem_policy_mode".into(),
        Value::String(format!("{:?}", certificate.kem_policy.mode)),
    );
    details.insert(
        "kem_preferred_suite".into(),
        Value::Number(Number::from(u64::from(
            certificate.kem_policy.preferred_suite,
        ))),
    );

    let has_pq_suite = certificate.handshake_suites.iter().any(|suite| {
        matches!(
            suite,
            HandshakeSuite::Nk2Hybrid | HandshakeSuite::Nk3PqForwardSecure
        )
    });
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
    details.insert(
        "has_pq_kem_public".into(),
        Value::Bool(!certificate.pq_kem_public.is_empty()),
    );

    let dual_signature = bundle.signatures.mldsa65.is_some();
    details.insert("dual_signature".into(), Value::Bool(dual_signature));
    if !dual_signature {
        state = ComponentState::Error;
    }

    // Validate signatures when possible.
    match bundle.verify(&ed_pub, &mldsa_key, phase) {
        Ok(()) => {
            details.insert("signature_valid".into(), Value::Bool(true));
        }
        Err(err) => {
            details.insert(
                "signature_valid".into(),
                Value::String(format!("invalid: {err}")),
            );
            state = ComponentState::Error;
        }
    }

    details.insert("state".into(), Value::String(state.as_str().to_string()));

    Ok((state, Value::Object(details)))
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
        state = ComponentState::Warn;
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

fn default_canaries(pop: &str) -> Vec<String> {
    vec![
        format!("canary1.{pop}.gw.sora.id"),
        format!("canary2.{pop}.gw.sora.id"),
    ]
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
    use ed25519_dalek::SigningKey;
    use iroha_crypto::soranet::certificate::{RelayCapabilityFlagsV1, RelayCertificateV2};
    use rand_core_06::OsRng;
    use soranet_pq::{MlDsaSuite, generate_mldsa_keypair};
    use tempfile::TempDir;

    use super::*;

    fn sample_certificate() -> RelayCertificateV2 {
        RelayCertificateV2 {
            relay_id: [0x11; 32],
            identity_ed25519: [0x22; 32],
            identity_mldsa65: vec![0x33; 1952],
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
                url: "soranet://relay.example:443".to_string(),
                priority: 1,
                tags: vec!["norito-stream".into()],
            }],
            capability_flags: RelayCapabilityFlagsV1::new(
                iroha_crypto::soranet::certificate::CapabilityToggle::Enabled,
                iroha_crypto::soranet::certificate::CapabilityToggle::Enabled,
                iroha_crypto::soranet::certificate::CapabilityToggle::Enabled,
                iroha_crypto::soranet::certificate::CapabilityToggle::Enabled,
            ),
            kem_policy: iroha_crypto::soranet::certificate::KemRotationPolicyV1 {
                mode: iroha_crypto::soranet::certificate::KemRotationModeV1::Static,
                preferred_suite: 1,
                fallback_suite: Some(0),
                rotation_interval_hours: 0,
                grace_period_hours: 0,
            },
            handshake_suites: vec![
                HandshakeSuite::Nk3PqForwardSecure,
                HandshakeSuite::Nk2Hybrid,
            ],
            published_at: 1_734_000_000,
            valid_after: 1_734_000_000,
            valid_until: 1_734_086_400,
            directory_hash: [0x55; 32],
            issuer_fingerprint: [0x66; 32],
            pq_kem_public: vec![0x77; 1184],
        }
    }

    #[test]
    fn generates_readiness_summary() {
        let temp = TempDir::new().expect("tempdir");
        let out_dir = temp.path().join("out");
        let tls_dir = temp.path().join("tls");
        fs::create_dir_all(&tls_dir).expect("tls dir");
        fs::write(tls_dir.join("fullchain.pem"), "CERT").expect("fullchain");
        fs::write(tls_dir.join("privkey.pem"), "KEY").expect("privkey");
        fs::write(
            tls_dir.join("ech.json"),
            r#"{"ech_config_b64":"ZmFrZS1jb25maWc="}"#,
        )
        .expect("ech");

        let mut rng = OsRng;
        let ed_signing = SigningKey::generate(&mut rng);
        let ed_public = ed_signing.verifying_key();
        let ml_keypair = generate_mldsa_keypair(MlDsaSuite::MlDsa65).expect("ml keypair");
        let ml_public = ml_keypair.public_key.clone();
        let ml_secret = ml_keypair.secret_key;

        let mut certificate = sample_certificate();
        certificate.identity_ed25519 = ed_public.to_bytes();
        certificate.identity_mldsa65 = ml_public.clone();
        let bundle = certificate
            .issue(&ed_signing, &ml_secret)
            .expect("issue certificate");

        let src_path = temp.path().join("srcv2.cbor");
        fs::write(&src_path, bundle.to_cbor()).expect("write srcv2");

        let trustless_path = temp.path().join("trustless.toml");
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

        let outcome = run_gateway_pq_readiness(GatewayPqOptions {
            output_dir: out_dir.clone(),
            pop: "sjc-01".to_string(),
            srcv2_bundle: src_path,
            tls_bundle_dir: tls_dir,
            trustless_config: trustless_path,
            canary_hosts: Vec::new(),
            validation_phase: CertificateValidationPhase::Phase3RequireDual,
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
        assert_eq!(src.get("dual_signature"), Some(&Value::Bool(true)));
        assert_eq!(src.get("signature_valid"), Some(&Value::Bool(true)));

        let tls = summary
            .get("tls")
            .and_then(Value::as_object)
            .expect("tls summary");
        assert_eq!(tls.get("fullchain_present"), Some(&Value::Bool(true)));
        assert_eq!(tls.get("ech_present"), Some(&Value::Bool(true)));

        let canaries = summary
            .get("canary_hosts")
            .and_then(Value::as_array)
            .expect("canaries");
        assert!(!canaries.is_empty());
    }
}
