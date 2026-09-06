use assert_cmd::cargo::cargo_bin_cmd;
use blake3::Hasher as Blake3;
use iroha_data_model::{
    account::AccountId,
    sorafs::gar::{GarEnforcementActionV1, GarEnforcementReceiptV1},
};
use norito::json::{self, Value};
use std::{fs, path::Path};
use tempfile::tempdir;
#[test]
fn soranet_gateway_m2_pipeline_emits_beta_and_ga() {
    let temp = tempdir().expect("tempdir");
    let out_dir = temp.path().join("gateway_m2");
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root");
    let receipts_dir = temp.path().join("receipts");
    fs::create_dir_all(&receipts_dir).expect("receipts dir");
    let receipt = GarEnforcementReceiptV1 {
        receipt_id: *b"beta-receipt-id!",
        gar_name: "docs.sora".to_string(),
        canonical_host: "docs.gateway.sora.net".to_string(),
        action: GarEnforcementActionV1::GeoFence,
        triggered_at_unix: 1_747_000_000,
        expires_at_unix: None,
        policy_version: Some("2027-beta".to_string()),
        policy_digest: Some([0xAA; 32]),
        operator: AccountId::new(
            "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03"
                .parse()
                .expect("public key"),
        ),
        reason: "SN15 compliance drill".to_string(),
        notes: Some("Beta rollout".to_string()),
        evidence_uris: vec!["sora://gar/receipts/docs/beta".to_string()],
        labels: vec!["snnet-15g".to_string()],
    };
    let receipt_path = receipts_dir.join("receipt.json");
    let receipt_file = fs::File::create(&receipt_path).expect("receipt file");
    norito::json::to_writer_pretty(receipt_file, &receipt).expect("write receipt");
    let acks_dir = temp.path().join("acks");
    fs::create_dir_all(&acks_dir).expect("acks dir");
    let ack = norito::json!({
        "receipt_id": "626574612d726563656970742d696421",
        "applied_version": "2027-beta",
        "pop": "soranet-sjc01",
        "acked_at_unix": 1_747_000_123u64
    });
    let ack_path = acks_dir.join("ack.json");
    let ack_file = fs::File::create(&ack_path).expect("ack file");
    norito::json::to_writer_pretty(ack_file, &ack).expect("write ack");
    let sbom_path = temp.path().join("sbom.json");
    fs::write(&sbom_path, r#"{"sbom":"ok"}"#).expect("sbom");
    let vuln_report = temp.path().join("vuln.txt");
    fs::write(&vuln_report, "no critical vulns").expect("vuln");
    let signing_policy = temp.path().join("signing-policy.txt");
    fs::write(&signing_policy, "external signing policy").expect("signing policy");
    let sandbox_profile = temp.path().join("sandbox.json");
    fs::write(&sandbox_profile, r#"{"profile":"cgroup"}"#).expect("sandbox");
    let descriptor = workspace_root
        .join("fixtures")
        .join("soranet_pop")
        .join("lab_pop.json");
    let trustless_config = workspace_root
        .join("configs")
        .join("soranet")
        .join("gateway_m0")
        .join("gateway_trustless_verifier.toml");
    let billing_usage = workspace_root
        .join("configs")
        .join("soranet")
        .join("gateway_m0")
        .join("billing_usage_sample.json");
    let billing_catalog = workspace_root
        .join("configs")
        .join("soranet")
        .join("gateway_m0")
        .join("meter_catalog.json");
    let billing_guardrails = workspace_root
        .join("configs")
        .join("soranet")
        .join("gateway_m0")
        .join("billing_guardrails.json");
    let config = norito::json!({
        "pops": [{
            "name": "soranet-sjc01",
            "descriptor": descriptor,
            "trustless_config": trustless_config,
            "doq_listen": ["0.0.0.0:8853"],
            "odoh_relay": "https://odoh.sora.net/relay"
        }],
        "billing": {
            "usage": billing_usage,
            "catalog": billing_catalog,
            "guardrails": billing_guardrails,
            "payer": "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D",
            "treasury": "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D",
            "asset": "4cuvDVPuLBKJyN6dPbRQhmLh68sU"
        },
        "compliance": {
            "receipts_dir": receipts_dir,
            "acks_dir": acks_dir
        },
        "hardening": {
            "sbom": sbom_path,
            "vuln_report": vuln_report,
            "signing_policy": signing_policy,
            "sandbox_profile": sandbox_profile,
            "data_retention_days": 14,
            "log_retention_days": 14
        }
    });
    let config_path = temp.path().join("gateway_m2_config.json");
    let config_file = fs::File::create(&config_path).expect("config file");
    norito::json::to_writer_pretty(config_file, &config).expect("write config");
    let mut cmd = cargo_bin_cmd!("xtask");
    let output = cmd
        .current_dir(workspace_root)
        .args([
            "soranet-gateway-m2",
            "--config",
            config_path.to_str().expect("utf8"),
            "--output-dir",
            out_dir.to_str().expect("utf8"),
        ])
        .env("CARGO_NET_OFFLINE", "true")
        .output()
        .expect("run soranet-gateway-m2");
    assert!(
        output.status.success(),
        "command failed: status={:?}\nstdout={}\nstderr={}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let summary_path = out_dir.join("gateway_m2_summary.json");
    let summary_bytes = fs::read(&summary_path).expect("summary exists");
    let summary: Value = json::from_slice(&summary_bytes).expect("summary parses");
    let pop = &summary["pops"][0];
    let beta_edge_config = pop["beta_edge_config"].as_str().expect("beta edge config");
    let beta_edge =
        fs::read_to_string(out_dir.join(beta_edge_config)).expect("read beta edge config");
    assert!(beta_edge.contains("name: sora-cache-version"));
    assert!(beta_edge.contains("prometheus_listener: 127.0.0.1:19092"));
    assert!(!beta_edge.contains("prometheus_listener: 0.0.0.0"));
    assert!(
        !beta_edge.contains("sora-denylist-version"),
        "retired local denylist header must not be required"
    );
    let hardening = pop["hardening_summary"]
        .as_str()
        .expect("hardening summary");
    assert!(pop.get("pq_summary").is_none());
    assert!(
        out_dir.join(hardening).is_file(),
        "hardening summary missing"
    );
    let compliance_path = summary["compliance_summary"]
        .as_str()
        .expect("compliance summary");
    assert!(
        out_dir.join(compliance_path).is_file(),
        "compliance summary missing"
    );
    let ga_dir = temp.path().join("gateway_m3");
    let autoscale_plan = temp.path().join("autoscale.json");
    fs::write(&autoscale_plan, r#"{"scale":"m3"}"#).expect("autoscale");
    let worker_pack = temp.path().join("worker.wasm");
    fs::write(&worker_pack, b"worker-bytes").expect("worker pack");
    let mut ga_cmd = cargo_bin_cmd!("xtask");
    let ga_output = ga_cmd
        .current_dir(workspace_root)
        .args([
            "soranet-gateway-m3",
            "--m2-summary",
            summary_path.to_str().expect("utf8"),
            "--autoscale-plan",
            autoscale_plan.to_str().expect("utf8"),
            "--worker-pack",
            worker_pack.to_str().expect("utf8"),
            "--out",
            ga_dir.to_str().expect("utf8"),
            "--sla-target",
            "99.95-regional",
        ])
        .env("CARGO_NET_OFFLINE", "true")
        .output()
        .expect("run soranet-gateway-m3");
    assert!(
        ga_output.status.success(),
        "m3 command failed: status={:?}\nstdout={}\nstderr={}",
        ga_output.status,
        String::from_utf8_lossy(&ga_output.stdout),
        String::from_utf8_lossy(&ga_output.stderr)
    );
    let ga_summary_path = ga_dir.join("gateway_m3_summary.json");
    let ga_bytes = fs::read(&ga_summary_path).expect("ga summary exists");
    let ga_summary: Value = json::from_slice(&ga_bytes).expect("ga summary parses");
    let autoscale_hex = ga_summary["autoscale_plan_blake3"]
        .as_str()
        .expect("autoscale digest");
    let worker_hex = ga_summary["worker_pack_blake3"]
        .as_str()
        .expect("worker digest");
    assert_eq!(autoscale_hex, blake3_hex(&autoscale_plan));
    assert_eq!(worker_hex, blake3_hex(&worker_pack));
}
fn blake3_hex(path: &Path) -> String {
    let mut hasher = Blake3::new();
    let mut file = fs::File::open(path).expect("open file");
    std::io::copy(&mut file, &mut hasher).expect("hash");
    hasher.finalize().to_hex().to_string()
}
