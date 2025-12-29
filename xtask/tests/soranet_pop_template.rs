use std::{fs, io::Read, path::Path};

use assert_cmd::cargo::cargo_bin_cmd;
use norito::json::{self, Value};
use sha2::{Digest, Sha256};
use tempfile::tempdir;

#[test]
fn soranet_pop_template_renders_fixture() {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask crate resides in workspace root");
    let descriptor = workspace_root
        .join("fixtures")
        .join("soranet_pop")
        .join("sjc01.json");
    assert!(
        descriptor.exists(),
        "fixture {} missing",
        descriptor.display()
    );

    let temp = tempdir().expect("tempdir");
    let output_path = temp.path().join("sjc01.conf");
    let golden = workspace_root
        .join("fixtures")
        .join("soranet_pop")
        .join("sjc01.frr.golden");
    assert!(golden.exists(), "golden {} missing", golden.display());

    let mut cmd = cargo_bin_cmd!("xtask");
    let result = cmd
        .args([
            "soranet-pop-template",
            "--input",
            descriptor.to_str().expect("descriptor utf8"),
            "--output",
            output_path.to_str().expect("output utf8"),
        ])
        .env("CARGO_NET_OFFLINE", "true")
        .output()
        .expect("run soranet-pop-template");

    assert!(
        result.status.success(),
        "command failed: status={:?}\nstdout={}\nstderr={}",
        result.status,
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );

    let rendered = fs::read_to_string(&output_path).expect("read generated config");
    assert!(
        rendered.contains("router bgp 65110"),
        "missing BGP stanza:\n{}",
        rendered
    );
    assert!(
        rendered.contains("neighbor 198.51.100.1 route-map RM_ix-west_IMPORT in"),
        "missing import route-map:\n{}",
        rendered
    );
    assert!(
        rendered.contains("match rpki invalid"),
        "missing RPKI guard:\n{}",
        rendered
    );
    let expected = fs::read_to_string(&golden).expect("read golden config");
    assert_eq!(
        rendered, expected,
        "rendered config diverged from golden fixture"
    );
}

#[test]
fn soranet_pop_template_writes_resolver_config() {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask crate resides in workspace root");
    let descriptor = workspace_root
        .join("fixtures")
        .join("soranet_pop")
        .join("sjc01.json");
    assert!(
        descriptor.exists(),
        "fixture {} missing",
        descriptor.display()
    );

    let temp = tempdir().expect("tempdir");
    let resolver_cfg = temp.path().join("resolver.toml");

    let mut cmd = cargo_bin_cmd!("xtask");
    let result = cmd
        .args([
            "soranet-pop-template",
            "--input",
            descriptor.to_str().expect("descriptor utf8"),
            "--resolver-config",
            resolver_cfg.to_str().expect("resolver utf8"),
        ])
        .env("CARGO_NET_OFFLINE", "true")
        .output()
        .expect("run soranet-pop-template with resolver");

    assert!(
        result.status.success(),
        "command failed: status={:?}\nstdout={}\nstderr={}",
        result.status,
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );

    let rendered = fs::read_to_string(&resolver_cfg).expect("read resolver config");
    assert!(
        rendered.contains("pop = \"sjc01\""),
        "resolver config missing pop name:\n{rendered}"
    );
    assert!(
        rendered.contains("protocols = [\"doh\",\"dot\",\"doq\",\"odoh-preview\"]"),
        "resolver config missing protocol map:\n{rendered}"
    );
    assert!(
        rendered.contains("managed_zones_catalog"),
        "resolver config missing managed_zones_catalog entry:\n{rendered}"
    );
}

#[test]
fn soranet_pop_validate_reports_metadata() {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask crate resides in workspace root");
    let descriptor = workspace_root
        .join("fixtures")
        .join("soranet_pop")
        .join("lab_pop.json");
    let roa_bundle = workspace_root
        .join("fixtures")
        .join("soranet_pop")
        .join("lab_roas.json");
    assert!(
        descriptor.exists(),
        "fixture {} missing",
        descriptor.display()
    );
    assert!(
        roa_bundle.exists(),
        "fixture {} missing",
        roa_bundle.display()
    );

    let mut cmd = cargo_bin_cmd!("xtask");
    let output = cmd
        .args([
            "soranet-pop-validate",
            "--input",
            descriptor.to_str().expect("descriptor utf8"),
            "--roa",
            roa_bundle.to_str().expect("roa utf8"),
            "--json-out",
            "-",
        ])
        .env("CARGO_NET_OFFLINE", "true")
        .output()
        .expect("run soranet-pop-validate");

    assert!(
        output.status.success(),
        "command failed: status={:?}\nstdout={}\nstderr={}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let report: json::Value =
        json::from_slice(&output.stdout).expect("parse validation report as JSON");
    assert_eq!(
        report["pop_name"],
        json::Value::String("lab-h3-sjc".to_string())
    );
    assert_eq!(
        report["communities"]["drain"],
        json::Value::String("65210:300".to_string())
    );
    assert_eq!(
        report["communities"]["fail_open"],
        json::Value::String("graceful-shutdown".to_string())
    );
    assert_eq!(
        report["communities"]["blackhole"],
        json::Value::String("65535:666".to_string())
    );
    assert_eq!(
        report["communities"]["using_defaults"],
        json::Value::Bool(false)
    );
}

#[test]
fn soranet_pop_policy_report_emits_monitoring_pack() {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask crate resides in workspace root");
    let descriptor = workspace_root.join("fixtures/soranet_pop/lab_pop.json");
    let roa_bundle = workspace_root.join("fixtures/soranet_pop/lab_roas.json");
    let temp = tempdir().expect("tempdir");
    let policy_dir = temp.path().join("policy");
    let report_path = policy_dir.join("policy_report.json");

    let mut cmd = cargo_bin_cmd!("xtask");
    let output = cmd
        .args([
            "soranet-pop-policy-report",
            "--input",
            descriptor.to_str().expect("descriptor utf8"),
            "--roa",
            roa_bundle.to_str().expect("roa utf8"),
            "--output-dir",
            policy_dir.to_str().expect("policy utf8"),
        ])
        .env("CARGO_NET_OFFLINE", "true")
        .output()
        .expect("run soranet-pop-policy-report");
    assert!(
        output.status.success(),
        "command failed: status={:?}\nstdout={}\nstderr={}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let alert_rules =
        fs::read_to_string(policy_dir.join("bgp_alert_rules.yml")).expect("alert rules rendered");
    assert!(
        alert_rules.contains("SoranetBGPNeighborDown"),
        "alert rules missing neighbor check:\n{alert_rules}"
    );
    assert!(
        alert_rules.contains("lab-h3-sjc"),
        "alert rules missing PoP name:\n{alert_rules}"
    );
    assert!(
        alert_rules.contains("SoranetBfdSessionsDown"),
        "alert rules missing BFD coverage:\n{alert_rules}"
    );

    let grafana = fs::read_to_string(policy_dir.join("grafana_soranet_bgp.json"))
        .expect("grafana dashboard rendered");
    assert!(
        grafana.contains("SoraNet BGP - lab-h3-sjc"),
        "grafana dashboard missing title:\n{grafana}"
    );
    assert!(
        grafana.contains("frr_bgp_prefix_bestpath_localpref"),
        "grafana dashboard missing local-pref panel:\n{grafana}"
    );
    assert!(
        grafana.contains("frr_bgp_prefix_bestpath_med"),
        "grafana dashboard missing MED panel:\n{grafana}"
    );

    let report_bytes = fs::read(&report_path).expect("policy report rendered");
    let report: Value = json::from_slice(&report_bytes).expect("report parses");
    assert_eq!(
        report["route_health"]["expected_neighbors"],
        Value::from(2u64),
        "report expected neighbor count not captured"
    );
    assert_eq!(
        report["route_health"]["expected_bfd_sessions"],
        Value::from(2u64),
        "report expected BFD session count not captured"
    );
    assert_eq!(
        report["route_health"]["expected_prefixes"],
        Value::from(2u64),
        "report expected prefix count not captured"
    );
    let lp_targets = report["route_health"]["local_pref_targets"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    assert!(
        lp_targets
            .iter()
            .any(|target| target["neighbor"] == Value::from("lab-edge-ix")
                && target["value"] == Value::from(300)),
        "local-pref targets missing lab-edge-ix: {lp_targets:?}"
    );
    let med_targets = report["route_health"]["med_targets"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    assert!(
        med_targets
            .iter()
            .any(|target| target["neighbor"] == Value::from("lab-edge-ix")
                && target["value"] == Value::from(20)),
        "MED targets missing lab-edge-ix: {med_targets:?}"
    );
}

#[test]
fn soranet_pop_bundle_embeds_route_health_probe() {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask crate resides in workspace root");
    let descriptor = workspace_root.join("fixtures/soranet_pop/lab_pop.json");
    let roa_bundle = workspace_root.join("fixtures/soranet_pop/lab_roas.json");
    let temp = tempdir().expect("tempdir");
    let bundle_dir = temp.path().join("bundle");

    let mut cmd = cargo_bin_cmd!("xtask");
    let output = cmd
        .args([
            "soranet-pop-bundle",
            "--input",
            descriptor.to_str().expect("descriptor utf8"),
            "--roa",
            roa_bundle.to_str().expect("roa utf8"),
            "--output-dir",
            bundle_dir.to_str().expect("bundle utf8"),
            "--skip-edns",
            "--skip-ds",
        ])
        .env("CARGO_NET_OFFLINE", "true")
        .output()
        .expect("run soranet-pop-bundle");
    assert!(
        output.status.success(),
        "command failed: status={:?}\nstdout={}\nstderr={}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let health_probe =
        fs::read_to_string(bundle_dir.join("health_probe.sh")).expect("health probe exists");
    assert!(
        health_probe.contains("expected_neighbors=2"),
        "health probe missing neighbor count:\n{health_probe}"
    );
    assert!(
        health_probe.contains("expected_rpki=1"),
        "health probe missing rpki expectation:\n{health_probe}"
    );
    assert!(
        health_probe.contains("expected_prefixes=2"),
        "health probe missing prefix expectation:\n{health_probe}"
    );
}

#[test]
fn soranet_pop_bundle_writes_manifest_and_assets() {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask crate resides in workspace root");
    let descriptor = workspace_root
        .join("fixtures")
        .join("soranet_pop")
        .join("lab_pop.json");
    let roa_bundle = workspace_root
        .join("fixtures")
        .join("soranet_pop")
        .join("lab_roas.json");
    assert!(
        descriptor.exists(),
        "fixture {} missing",
        descriptor.display()
    );

    let temp = tempdir().expect("tempdir");
    let output_dir = temp.path().join("bundle");
    let manifest = run_bundle_command(
        "soranet-pop-bundle",
        &descriptor,
        &roa_bundle,
        &output_dir,
        "lab-img-v1",
    );
    let manifest_path = output_dir.join("bundle_manifest.json");
    assert!(
        manifest_path.exists(),
        "manifest {} missing",
        manifest_path.display()
    );

    assert_eq!(
        manifest["pop_name"],
        json::Value::String("lab-h3-sjc".to_string())
    );
    assert_eq!(
        manifest["descriptor_sha256"],
        json::Value::String(sha256_hex(&descriptor))
    );
    assert_eq!(
        manifest["image_tag"],
        json::Value::String("lab-img-v1".to_string())
    );
    assert!(
        manifest["ds_validation"].is_null(),
        "ds_validation should be null when --skip-ds is set"
    );
    assert!(
        manifest["edns_matrix"].is_null(),
        "edns_matrix should be null when --skip-edns is set"
    );
    for required in [
        "frr_config",
        "resolver_config",
        "pxe_profile",
        "pop_env",
        "secrets_template",
        "bringup_checklist",
        "ci_job",
        "signoff_bundle",
        "health_probe",
    ] {
        assert!(
            manifest[required].is_object(),
            "manifest missing required artifact record for {required}"
        );
    }
    assert!(
        manifest["sigstore_provenance_path"].is_string(),
        "sigstore_provenance_path must be a string"
    );
    assert!(
        manifest["promotion_gates"]
            .as_array()
            .map(|arr| !arr.is_empty())
            .unwrap_or(false),
        "manifest missing promotion_gates entries"
    );
    assert!(output_dir.join("frr.conf").exists());
    assert!(output_dir.join("resolver.toml").exists());
    assert!(output_dir.join("pxe_profile.toml").exists());
    assert!(output_dir.join("health_probe.sh").exists());
    assert!(output_dir.join("pop.env").exists());
    assert!(
        output_dir
            .join("secrets")
            .join("secret_placeholders.toml")
            .exists()
    );
    assert!(
        output_dir
            .join("attestations")
            .join("soranet-lab-h3-sjc-provenance.intoto.jsonl")
            .exists()
    );
    assert!(output_dir.join("checklist.md").exists());
    assert!(output_dir.join("ci").join("pop_bringup.yml").exists());
    let signoff_path = output_dir.join("attestations").join("signoff.json");
    assert!(signoff_path.exists());
    let signoff: json::Value =
        json::from_slice(&fs::read(&signoff_path).expect("read signoff JSON"))
            .expect("parse signoff JSON");
    assert_eq!(
        signoff["pop_name"],
        json::Value::String("lab-h3-sjc".to_string())
    );
    assert_eq!(
        signoff["image_tag"],
        json::Value::String("lab-img-v1".to_string())
    );
    assert!(
        signoff["promotion_gates"]
            .as_array()
            .map(|arr| !arr.is_empty())
            .unwrap_or(false),
        "signoff bundle missing promotion gates"
    );
    let artifacts = signoff["artifacts"]
        .as_array()
        .expect("signoff artifacts array");
    assert!(
        artifacts.iter().any(|artifact| artifact["path"]
            .as_str()
            .unwrap_or("")
            .ends_with("frr.conf")),
        "signoff bundle missing FRR artifact"
    );
}

#[test]
fn soranet_popctl_aliases_pop_bundle() {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask crate resides in workspace root");
    let descriptor = workspace_root
        .join("fixtures")
        .join("soranet_pop")
        .join("lab_pop.json");
    let roa_bundle = workspace_root
        .join("fixtures")
        .join("soranet_pop")
        .join("lab_roas.json");
    let temp = tempdir().expect("tempdir");
    let output_dir = temp.path().join("popctl");

    let manifest = run_bundle_command(
        "soranet-popctl",
        &descriptor,
        &roa_bundle,
        &output_dir,
        "popctl-img-1",
    );

    assert_eq!(
        manifest["pop_name"],
        json::Value::String("lab-h3-sjc".to_string())
    );
    assert_eq!(
        manifest["image_tag"],
        json::Value::String("popctl-img-1".to_string())
    );
    assert!(
        output_dir
            .join("attestations")
            .join("signoff.json")
            .exists(),
        "signoff.json missing for popctl alias"
    );
}

fn run_bundle_command(
    command: &str,
    descriptor: &Path,
    roa_bundle: &Path,
    output_dir: &Path,
    image_tag: &str,
) -> json::Value {
    let mut cmd = cargo_bin_cmd!("xtask");
    let result = cmd
        .args([
            command,
            "--input",
            descriptor.to_str().expect("descriptor utf8"),
            "--roa",
            roa_bundle.to_str().expect("roa utf8"),
            "--output-dir",
            output_dir.to_str().expect("output utf8"),
            "--skip-edns",
            "--skip-ds",
            "--image-tag",
            image_tag,
        ])
        .env("CARGO_NET_OFFLINE", "true")
        .output()
        .expect("run soranet pop bundle variant");

    assert!(
        result.status.success(),
        "command failed: status={:?}\nstdout={}\nstderr={}",
        result.status,
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );

    let manifest_path = output_dir.join("bundle_manifest.json");
    assert!(
        manifest_path.exists(),
        "manifest {} missing",
        manifest_path.display()
    );
    json::from_slice(&fs::read(&manifest_path).expect("read manifest JSON"))
        .expect("parse manifest JSON")
}

fn sha256_hex(path: &Path) -> String {
    let mut hasher = Sha256::new();
    let mut file = fs::File::open(path).expect("open file for hashing");
    let mut buf = [0u8; 4096];
    loop {
        let read = file.read(&mut buf).expect("read file");
        if read == 0 {
            break;
        }
        hasher.update(&buf[..read]);
    }
    format!("{:x}", hasher.finalize())
}
