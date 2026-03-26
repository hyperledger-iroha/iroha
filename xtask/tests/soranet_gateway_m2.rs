use std::{fs, path::Path};

use assert_cmd::cargo::cargo_bin_cmd;
use blake3::Hasher as Blake3;
use ed25519_dalek::SigningKey;
use iroha_crypto::soranet::{
    certificate::{
        CapabilityToggle, KemRotationModeV1, KemRotationPolicyV1, RelayCapabilityFlagsV1,
        RelayCertificateV2, RelayEndpointV2, RelayRolesV2,
    },
    handshake::HandshakeSuite,
};
use iroha_data_model::{
    account::AccountId,
    sorafs::gar::{GarEnforcementActionV1, GarEnforcementReceiptV1},
};
use norito::json::{self, Value};
use soranet_pq::{MlDsaSuite, generate_mldsa_keypair};
use tempfile::tempdir;

#[test]
fn soranet_gateway_m2_pipeline_emits_beta_and_ga() {
    let temp = tempdir().expect("tempdir");
    let out_dir = temp.path().join("gateway_m2");
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root");

    let pq_bundle = temp.path().join("sample_srcv2.cbor");
    write_sample_srcv2(&pq_bundle);

    let tls_dir = temp.path().join("tls");
    fs::create_dir_all(&tls_dir).expect("tls dir");
    fs::write(tls_dir.join("fullchain.pem"), "CERT").expect("fullchain");
    fs::write(tls_dir.join("privkey.pem"), "KEY").expect("privkey");
    fs::write(
        tls_dir.join("ech.json"),
        r#"{"ech_config_b64":"ZWNobWFyay1zYW1wbGU="}"#,
    )
    .expect("ech");

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
    let hsm_policy = temp.path().join("hsm.txt");
    fs::write(&hsm_policy, "YubiHSM policy").expect("hsm");
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
            "pq_bundle": pq_bundle,
            "tls_bundle_dir": tls_dir,
            "doq_listen": ["0.0.0.0:8853"],
            "odoh_relay": "https://odoh.sora.net/relay"
        }],
        "billing": {
            "usage": billing_usage,
            "catalog": billing_catalog,
            "guardrails": billing_guardrails,
            "payer": "sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D",
            "treasury": "ed0120BDF918243253B1E731FA096194C8928DA37C4D3226F97EEBD18CF5523D758D6C",
            "asset": "4cuvDVPuLBKJyN6dPbRQhmLh68sU"
        },
        "compliance": {
            "receipts_dir": receipts_dir,
            "acks_dir": acks_dir
        },
        "hardening": {
            "sbom": sbom_path,
            "vuln_report": vuln_report,
            "hsm_policy": hsm_policy,
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
    let pq_summary = pop["pq_summary"].as_str().expect("pq summary");
    let hardening = pop["hardening_summary"]
        .as_str()
        .expect("hardening summary");
    assert!(out_dir.join(pq_summary).is_file(), "pq summary missing");
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

fn write_sample_srcv2(path: &Path) {
    let signing_key = SigningKey::from_bytes(&[0x11; 32]);
    let mldsa_keys = generate_mldsa_keypair(MlDsaSuite::MlDsa65).expect("mldsa keypair");

    let certificate = RelayCertificateV2 {
        relay_id: [0x11; 32],
        identity_ed25519: signing_key.verifying_key().to_bytes(),
        identity_mldsa65: mldsa_keys.public_key.clone(),
        descriptor_commit: [0x22; 32],
        roles: RelayRolesV2 {
            entry: true,
            middle: true,
            exit: true,
        },
        guard_weight: 120,
        bandwidth_bytes_per_sec: 5_000_000,
        reputation_weight: 90,
        endpoints: vec![RelayEndpointV2 {
            url: "soranet://relay.example:443".to_string(),
            priority: 1,
            tags: vec!["norito-stream".into(), "doq".into()],
        }],
        capability_flags: RelayCapabilityFlagsV1::new(
            CapabilityToggle::Enabled,
            CapabilityToggle::Enabled,
            CapabilityToggle::Enabled,
            CapabilityToggle::Disabled,
        ),
        kem_policy: KemRotationPolicyV1 {
            mode: KemRotationModeV1::Static,
            preferred_suite: 1,
            fallback_suite: None,
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
        directory_hash: [0x44; 32],
        issuer_fingerprint: [0x55; 32],
        pq_kem_public: vec![0x77; 1184],
    };
    let bundle = certificate
        .issue(&signing_key, mldsa_keys.secret_key())
        .expect("issue bundle");
    let bytes = bundle.to_cbor();
    fs::write(path, bytes).expect("write cbor");
}

fn blake3_hex(path: &Path) -> String {
    let mut hasher = Blake3::new();
    let mut file = fs::File::open(path).expect("open file");
    std::io::copy(&mut file, &mut hasher).expect("hash");
    hasher.finalize().to_hex().to_string()
}
