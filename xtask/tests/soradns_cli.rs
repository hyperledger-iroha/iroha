use std::{fs, path::PathBuf};

use assert_cmd::cargo::cargo_bin_cmd;
use blake3::hash as blake3_hash;
use data_encoding::BASE32_NOPAD;
use iroha_primitives::soradns::{GatewayHostBindings, derive_gateway_hosts};
use norito::json::{self as serde_json, Value};
use tempfile::TempDir;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}

#[test]
fn soradns_hosts_reports_expected_derivations() {
    let temp = TempDir::new().expect("temp dir");
    let output_path = temp.path().join("hosts.json");

    let mut cmd = cargo_bin_cmd!("xtask");
    cmd.current_dir(workspace_root());
    cmd.args([
        "soradns-hosts",
        "--name",
        "docs.sora",
        "--json-out",
        output_path.to_str().expect("utf8 path"),
    ]);
    cmd.assert().success();

    let raw = fs::read_to_string(&output_path).expect("host summary output");
    let parsed: Value = serde_json::from_str(&raw).expect("host summary json");
    let entries = parsed
        .as_array()
        .expect("soradns-hosts should render an array");
    assert_eq!(
        entries.len(),
        1,
        "single input name should yield a single entry"
    );
    let entry = entries[0]
        .as_object()
        .expect("each entry should be a JSON object");

    let bindings = derive_gateway_hosts("docs.sora").expect("derive gateway hosts");
    assert_eq!(entry["name"].as_str(), Some("docs.sora"));
    assert_eq!(
        entry["canonical_label"].as_str(),
        Some(bindings.canonical_label())
    );
    assert_eq!(
        entry["canonical_host"].as_str(),
        Some(bindings.canonical_host())
    );
    assert_eq!(entry["pretty_host"].as_str(), Some(bindings.pretty_host()));
    assert_eq!(
        entry["canonical_wildcard"].as_str(),
        Some(GatewayHostBindings::canonical_wildcard())
    );
}

#[test]
fn soradns_binding_template_writes_payload_and_headers() {
    let temp = TempDir::new().expect("temp dir");
    let manifest_path = temp.path().join("manifest.json");
    fs::write(&manifest_path, r#"{ "root_cid_hex": "0123456789abcdef" }"#).expect("write manifest");

    let json_out = temp.path().join("binding.json");
    let headers_out = temp.path().join("binding.headers");
    let timestamp = "2025-01-02T03:04:05Z";

    let mut cmd = cargo_bin_cmd!("xtask");
    cmd.current_dir(workspace_root());
    cmd.args([
        "soradns-binding-template",
        "--manifest",
        manifest_path.to_str().expect("utf8 manifest"),
        "--alias",
        "docs.sora",
        "--hostname",
        "docs.sora.gw.sora.name",
        "--route-label",
        "docs",
        "--proof-status",
        "ok",
        "--generated-at",
        timestamp,
        "--json-out",
        json_out.to_str().expect("utf8 json path"),
        "--headers-out",
        headers_out.to_str().expect("utf8 header path"),
    ]);
    cmd.assert().success();

    let payload_raw = fs::read_to_string(&json_out).expect("binding payload");
    let payload: Value = serde_json::from_str(&payload_raw).expect("binding json");
    assert_eq!(payload["alias"].as_str(), Some("docs.sora"));
    assert_eq!(payload["hostname"].as_str(), Some("docs.sora.gw.sora.name"));
    assert_eq!(payload["proofStatus"].as_str(), Some("ok"));
    assert_eq!(payload["generatedAt"].as_str(), Some(timestamp));

    let expected_cid = {
        let root = hex::decode("0123456789abcdef").expect("root hex");
        let encoded = BASE32_NOPAD.encode(&root).to_ascii_lowercase();
        format!("b{encoded}")
    };
    assert_eq!(payload["contentCid"].as_str(), Some(expected_cid.as_str()));

    let headers = payload["headers"]
        .as_object()
        .expect("headers block should be an object");
    assert_eq!(
        headers.get("Sora-Content-CID").and_then(Value::as_str),
        Some(expected_cid.as_str())
    );
    assert_eq!(
        headers.get("Sora-Name").and_then(Value::as_str),
        Some("docs.sora")
    );
    let route_binding = headers
        .get("Sora-Route-Binding")
        .and_then(Value::as_str)
        .expect("route binding header");
    assert!(
        route_binding.contains("host=docs.sora.gw.sora.name"),
        "route binding must record host"
    );
    assert!(
        route_binding.contains(&format!("cid={expected_cid}")),
        "route binding must reference CID"
    );
    assert!(
        route_binding.contains(&format!("generated_at={timestamp}")),
        "route binding must record timestamp"
    );
    assert!(
        route_binding.contains("label=docs"),
        "route binding must include the route label"
    );

    let headers_template =
        fs::read_to_string(&headers_out).expect("binding header template output");
    assert!(
        headers_template.contains("Sora-Content-CID"),
        "header template should include Sora-Content-CID"
    );
    assert!(
        headers_template.contains("Content-Security-Policy"),
        "header template should include CSP defaults"
    );
}

#[test]
fn soradns_gar_template_renders_payload() {
    let temp = TempDir::new().expect("temp dir");
    let output_path = temp.path().join("gar.json");
    let manifest_digest = "0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF";

    let mut cmd = cargo_bin_cmd!("xtask");
    cmd.current_dir(workspace_root());
    cmd.args([
        "soradns-gar-template",
        "--name",
        "docs.sora",
        "--manifest-cid",
        "bafyMockCid",
        "--manifest-digest",
        manifest_digest,
        "--valid-from",
        "1700000000",
        "--valid-until",
        "1750000000",
        "--telemetry-label",
        "docs-gateway",
        "--json-out",
        output_path.to_str().expect("utf8 output path"),
    ]);
    cmd.assert().success();

    let payload_raw = fs::read_to_string(&output_path).expect("gar payload");
    let payload: Value = serde_json::from_str(&payload_raw).expect("gar json");

    assert_eq!(payload["version"].as_u64(), Some(2));
    let bindings = derive_gateway_hosts("docs.sora").expect("derive gateway hosts");
    assert_eq!(payload["name"].as_str(), Some(bindings.normalized_name()));
    assert_eq!(payload["manifest_cid"].as_str(), Some("bafyMockCid"));
    let normalized_digest = manifest_digest.to_ascii_lowercase();
    assert_eq!(
        payload["manifest_digest"].as_str(),
        Some(normalized_digest.as_str())
    );
    assert_eq!(payload["valid_from_epoch"].as_u64(), Some(1_700_000_000u64));
    assert_eq!(
        payload["valid_until_epoch"].as_u64(),
        Some(1_750_000_000u64)
    );

    let host_patterns = payload["host_patterns"]
        .as_array()
        .expect("host_patterns array");
    let patterns: Vec<&str> = host_patterns
        .iter()
        .map(|value| value.as_str().expect("host pattern"))
        .collect();
    assert!(
        patterns.contains(&bindings.canonical_host()),
        "GAR must authorise the canonical host"
    );
    assert!(
        patterns.contains(&bindings.pretty_host()),
        "GAR must authorise the pretty host"
    );
    assert!(
        patterns.contains(&GatewayHostBindings::canonical_wildcard()),
        "GAR must include wildcard host pattern"
    );
    assert_eq!(
        payload["permissions_template"].as_str(),
        Some(concat!(
            "accelerometer=(), ambient-light-sensor=(), autoplay=(), camera=(), ",
            "clipboard-read=(self), clipboard-write=(self), encrypted-media=(), fullscreen=(self), ",
            "geolocation=(), gyroscope=(), hid=(), magnetometer=(), microphone=(), midi=(), ",
            "payment=(), picture-in-picture=(), speaker-selection=(), usb=(), xr-spatial-tracking=()"
        )),
        "GAR should include the default Permissions-Policy template"
    );

    let telemetry_labels = payload["telemetry_labels"]
        .as_array()
        .expect("telemetry label array");
    assert_eq!(telemetry_labels.len(), 1);
    assert_eq!(
        telemetry_labels[0].as_str(),
        Some("docs-gateway"),
        "GAR payload should inherit CLI telemetry labels"
    );
}

#[test]
fn soradns_gar_template_derives_manifest_metadata() {
    let temp = TempDir::new().expect("temp dir");
    let manifest_path = temp.path().join("manifest.json");
    fs::write(&manifest_path, r#"{ "root_cid_hex": "0123456789abcdef" }"#).expect("write manifest");

    let output_path = temp.path().join("gar.json");
    let mut cmd = cargo_bin_cmd!("xtask");
    cmd.current_dir(workspace_root());
    cmd.args([
        "soradns-gar-template",
        "--name",
        "docs.sora",
        "--manifest",
        manifest_path.to_str().expect("utf8 manifest path"),
        "--telemetry-label",
        "dg-3",
        "--json-out",
        output_path.to_str().expect("utf8 output path"),
    ]);
    cmd.assert().success();

    let payload_raw = fs::read_to_string(&output_path).expect("gar payload");
    let payload: Value = serde_json::from_str(&payload_raw).expect("gar json");
    let expected_root = hex::decode("0123456789abcdef").expect("root hex");
    let expected_cid = format!(
        "b{}",
        BASE32_NOPAD.encode(&expected_root).to_ascii_lowercase()
    );
    assert_eq!(
        payload["manifest_cid"].as_str(),
        Some(expected_cid.as_str())
    );
    let expected_digest = blake3_hash(&fs::read(&manifest_path).expect("read manifest"))
        .to_hex()
        .to_string();
    assert_eq!(
        payload["manifest_digest"].as_str(),
        Some(expected_digest.as_str())
    );
}

#[test]
fn soradns_cache_plan_renders_targets() {
    let temp = TempDir::new().expect("temp dir");
    let output_path = temp.path().join("cache_plan.json");

    let mut cmd = cargo_bin_cmd!("xtask");
    cmd.current_dir(workspace_root());
    cmd.args([
        "soradns-cache-plan",
        "--name",
        "docs.sora",
        "--path",
        "/",
        "--path",
        "/health",
        "--auth-header",
        "Authorization",
        "--auth-env",
        "CACHE_PURGE_TOKEN",
        "--json-out",
        output_path.to_str().expect("utf8 path"),
    ]);
    cmd.assert().success();

    let raw = fs::read_to_string(&output_path).expect("cache plan output");
    let plan: Value = serde_json::from_str(&raw).expect("cache plan json");

    assert_eq!(plan["http_method"].as_str(), Some("PURGE"));
    assert_eq!(plan["auth_header"].as_str(), Some("Authorization"));
    assert_eq!(plan["auth_env"].as_str(), Some("CACHE_PURGE_TOKEN"));

    let entries = plan["entries"]
        .as_array()
        .expect("plan entries should be an array");
    assert_eq!(entries.len(), 1, "single alias should yield a single entry");
    let entry = entries[0].as_object().expect("entry must be a JSON object");
    assert_eq!(entry["name"].as_str(), Some("docs.sora"));

    let canonical_host = entry["canonical_host"]
        .as_str()
        .expect("canonical host string");
    let pretty_host = entry["pretty_host"].as_str().expect("pretty host string");
    let targets = entry["purge_targets"]
        .as_array()
        .expect("purge_targets should be an array");
    assert_eq!(
        targets.len(),
        2,
        "canonical and pretty hosts should be included"
    );
    for target in targets {
        let host = target["host"].as_str().expect("target host string");
        assert!(
            host == canonical_host || host == pretty_host,
            "purge targets should match canonical/pretty hosts"
        );
        let paths = target["paths"].as_array().expect("paths array");
        let rendered_paths: Vec<&str> = paths
            .iter()
            .map(|value| value.as_str().expect("path entry"))
            .collect();
        assert_eq!(
            rendered_paths,
            vec!["/", "/health"],
            "paths should mirror CLI arguments"
        );
    }
}

#[test]
fn soradns_acme_plan_covers_canonical_and_pretty_hosts() {
    let temp = TempDir::new().expect("temp dir");
    let output_path = temp.path().join("acme_plan.json");

    let mut cmd = cargo_bin_cmd!("xtask");
    cmd.current_dir(workspace_root());
    cmd.args([
        "soradns-acme-plan",
        "--name",
        "docs.sora",
        "--directory-url",
        "https://acme.example/soranet",
        "--generated-at",
        "2025-01-02T03:04:05Z",
        "--json-out",
        output_path.to_str().expect("utf8 path"),
    ]);
    cmd.assert().success();

    let raw = fs::read_to_string(&output_path).expect("acme plan output");
    let plan: Value = serde_json::from_str(&raw).expect("acme plan json");

    assert_eq!(
        plan["directory_url"].as_str(),
        Some("https://acme.example/soranet"),
        "ACME plan should mirror custom directory URL"
    );
    assert_eq!(
        plan["generated_at"].as_str(),
        Some("2025-01-02T03:04:05Z"),
        "ACME plan should record the supplied timestamp"
    );

    let hosts = plan["hosts"].as_array().expect("hosts array should exist");
    assert_eq!(hosts.len(), 1, "single alias should render a single entry");
    let entry = hosts[0]
        .as_object()
        .expect("ACME plan host entry must be an object");
    assert_eq!(entry["name"].as_str(), Some("docs.sora"));
    let bindings = derive_gateway_hosts("docs.sora").expect("derive gateway hosts");
    assert_eq!(
        entry["canonical_host"].as_str(),
        Some(bindings.canonical_host())
    );
    let canonical_wildcard = format!("*.{}", bindings.canonical_host());
    assert_eq!(
        entry["canonical_wildcard"].as_str(),
        Some(canonical_wildcard.as_str())
    );
    assert_eq!(entry["pretty_host"].as_str(), Some(bindings.pretty_host()));

    let certificates = entry["certificates"]
        .as_array()
        .expect("certificate plans must be an array");
    assert_eq!(
        certificates.len(),
        2,
        "plan should cover canonical wildcard and pretty host"
    );

    let canonical = certificates[0]
        .as_object()
        .expect("canonical certificate entry");
    assert_eq!(canonical["kind"].as_str(), Some("canonical_wildcard"));
    let san = canonical["san"].as_array().expect("canonical SAN array");
    let san_values: Vec<&str> = san
        .iter()
        .map(|value| value.as_str().expect("SAN entries are strings"))
        .collect();
    assert_eq!(
        san_values,
        vec![canonical_wildcard.as_str(), bindings.canonical_host()]
    );
    let challenges = canonical["recommended_challenges"]
        .as_array()
        .expect("challenge list");
    let challenge_values: Vec<&str> = challenges
        .iter()
        .map(|value| value.as_str().expect("challenge entries are strings"))
        .collect();
    assert_eq!(
        challenge_values,
        vec!["dns-01"],
        "wildcard certs should prefer dns-01"
    );
    let labels = canonical["dns_challenge_labels"]
        .as_array()
        .expect("dns label list");
    let label_values: Vec<&str> = labels
        .iter()
        .map(|value| value.as_str().expect("dns label entry"))
        .collect();
    assert_eq!(
        label_values,
        vec![format!("_acme-challenge.{}", bindings.canonical_host())]
    );

    let pretty = certificates[1].as_object().expect("pretty certificate");
    assert_eq!(pretty["kind"].as_str(), Some("pretty_host"));
    let pretty_san = pretty["san"].as_array().expect("pretty SAN array");
    let pretty_san_values: Vec<&str> = pretty_san
        .iter()
        .map(|value| value.as_str().expect("SAN entries are strings"))
        .collect();
    assert_eq!(pretty_san_values, vec![bindings.pretty_host()]);
    let pretty_challenges = pretty["recommended_challenges"]
        .as_array()
        .expect("pretty challenge list");
    let pretty_challenge_values: Vec<&str> = pretty_challenges
        .iter()
        .map(|value| value.as_str().expect("challenge entry"))
        .collect();
    assert_eq!(
        pretty_challenge_values,
        vec!["tls-alpn-01", "http-01"],
        "pretty host certs should support tls-alpn-01 and http-01"
    );
    assert!(
        pretty["dns_challenge_labels"]
            .as_array()
            .expect("dns label array for pretty entry")
            .is_empty(),
        "pretty host plans should omit dns-01 labels"
    );
}
