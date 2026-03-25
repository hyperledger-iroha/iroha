#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Torii contract manifest endpoints: bytecode deploy wraps ISIs and GET reads the derived on-chain manifest.

use std::time::{Duration, Instant};

use base64::Engine as _;
use eyre::{Result, eyre};
use integration_tests::sandbox;
use iroha::data_model::prelude::*;
use iroha_executor_data_model::permission::{
    governance::CanEnactGovernance, smart_contract::CanRegisterSmartContractCode,
};
use iroha_test_network::NetworkBuilder;
use reqwest::StatusCode;

fn minimal_contract_artifact() -> Vec<u8> {
    let meta = ivm::ProgramMetadata {
        version_major: 1,
        version_minor: 1,
        mode: 0,
        vector_length: 0,
        max_cycles: 1_000,
        abi_version: 1,
    };
    let interface = ivm::EmbeddedContractInterfaceV1 {
        compiler_fingerprint: "integration-tests".to_owned(),
        features_bitmap: 0,
        access_set_hints: None,
        kotoba: Vec::new(),
        entrypoints: vec![ivm::EmbeddedEntrypointDescriptor {
            name: "main".to_owned(),
            kind: iroha_data_model::smart_contract::manifest::EntryPointKind::Public,
            permission: None,
            read_keys: Vec::new(),
            write_keys: Vec::new(),
            access_hints_complete: Some(true),
            access_hints_skipped: Vec::new(),
            triggers: Vec::new(),
            entry_pc: 0,
        }],
    };
    let mut out = meta.encode();
    out.extend_from_slice(&interface.encode_section());
    out.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
    out
}

fn contract_state_probe_artifact() -> Vec<u8> {
    let src = r#"
seiyaku ContractStateProbe {
  meta { abi_version: 1; }

  state int Initialized;
  state int StoredValue;

  kotoage fn main() -> int {
    return 0;
  }

  kotoage fn init() permission(Admin) {
    Initialized = 1;
    StoredValue = 7;
  }

  kotoage fn verify() permission(Admin) {
    assert(Initialized == 1, "not initialized");
    state_set(name("probe_readback"), encode_int(StoredValue));
  }
}
"#;
    ivm::KotodamaCompiler::new()
        .compile_source(src)
        .expect("compile contract-state probe program")
}

async fn wait_for_approved_txs(
    client: &iroha::client::Client,
    baseline: u64,
    timeout: Duration,
    stage: &str,
) -> Result<()> {
    let deadline = Instant::now() + timeout;
    let mut last_status = None;
    let mut last_error = None;

    while Instant::now() < deadline {
        match tokio::task::spawn_blocking({
            let client = client.clone();
            move || client.get_status()
        })
        .await
        .expect("poll status")
        {
            Ok(status) => {
                if status.txs_approved > baseline {
                    return Ok(());
                }
                last_status = Some(status);
                last_error = None;
            }
            Err(err) => {
                last_error = Some(err.to_string());
            }
        }

        tokio::time::sleep(Duration::from_millis(250)).await;
    }

    Err(eyre!(
        "{stage}: timed out waiting for txs_approved to advance beyond {baseline}; last_status={last_status:?}; last_error={last_error:?}"
    ))
}

fn pipeline_status_kind(payload: &norito::json::Value) -> Option<&str> {
    let status = payload
        .get("content")
        .and_then(|content| content.get("status"))
        .or_else(|| payload.get("status"))?;
    match status {
        norito::json::Value::String(kind) => Some(kind.as_str()),
        norito::json::Value::Object(map) => map.get("kind").and_then(norito::json::Value::as_str),
        _ => None,
    }
}

async fn wait_for_tx_applied(
    http: &reqwest::Client,
    torii_url: &reqwest::Url,
    tx_hash_hex: &str,
    timeout: Duration,
    stage: &str,
) -> Result<()> {
    let mut status_url = torii_url.join("v1/pipeline/transactions/status")?;
    status_url
        .query_pairs_mut()
        .append_pair("hash", tx_hash_hex);
    let deadline = Instant::now() + timeout;
    let mut last_kind = String::from("unavailable");
    let mut last_payload = String::new();
    let mut last_error = String::new();

    loop {
        match http.get(status_url.clone()).send().await {
            Ok(response)
                if response.status() == reqwest::StatusCode::OK
                    || response.status() == reqwest::StatusCode::ACCEPTED =>
            {
                let status = response.status();
                let bytes = response.bytes().await?;
                if bytes.is_empty() {
                    last_kind = format!("http {status} with empty body");
                } else {
                    let payload: norito::json::Value = norito::json::from_slice(&bytes)?;
                    if let Some(kind) = pipeline_status_kind(&payload) {
                        last_kind = kind.to_string();
                        last_payload = format!("{payload:?}");
                        match kind {
                            "Applied" => return Ok(()),
                            "Rejected" => {
                                return Err(eyre!(
                                    "{stage}: tx `{tx_hash_hex}` rejected; payload={payload:?}"
                                ));
                            }
                            "Expired" => {
                                return Err(eyre!("{stage}: tx `{tx_hash_hex}` expired"));
                            }
                            _ => {}
                        }
                    } else {
                        last_kind = "missing status kind".to_string();
                        last_payload = format!("{payload:?}");
                    }
                }
            }
            Ok(response)
                if response.status() == reqwest::StatusCode::NO_CONTENT
                    || response.status() == reqwest::StatusCode::NOT_FOUND =>
            {
                last_kind = format!("http {}", response.status());
            }
            Ok(response) => {
                last_error = format!(
                    "http {} {}",
                    response.status(),
                    std::str::from_utf8(response.bytes().await?.as_ref()).unwrap_or("")
                );
            }
            Err(err) => {
                last_error = format!("{err}");
            }
        }

        if Instant::now() >= deadline {
            return Err(eyre!(
                "{stage}: timed out waiting for tx `{tx_hash_hex}` to reach Applied; last_kind={last_kind}, last_payload={last_payload}, last_error={last_error}"
            ));
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}

#[allow(clippy::too_many_lines)]
#[tokio::test]
async fn deploy_and_get_contract_manifest_via_torii() -> Result<()> {
    // Grant CanRegisterSmartContractCode to Alice in genesis so she can deploy contracts.
    let permission: Permission = CanRegisterSmartContractCode.into();
    let builder = NetworkBuilder::new()
        .with_min_peers(4)
        // Keep pipeline timings short to ensure the deploy transaction is flushed promptly.
        .with_pipeline_time(std::time::Duration::from_secs(4))
        .with_config_layer(|layer| {
            // Surface more detail if the pipeline stalls while registering the contract.
            layer.write(["logger", "level"], "TRACE").write(
                ["logger", "filter"],
                "iroha_core::sumeragi=trace,iroha_core::queue=trace,iroha_core::smartcontracts=trace,iroha_core::tx=trace",
            );
        })
        .with_genesis_instruction(Grant::account_permission(
            permission,
            iroha_test_samples::ALICE_ID.clone(),
        ));
    let Some(network) = sandbox::start_network_async_or_skip(
        builder,
        stringify!(deploy_and_get_contract_manifest_via_torii),
    )
    .await?
    else {
        return Ok(());
    };

    let client = network.client();

    // Wait for genesis to be committed before submitting additional transactions
    network.ensure_blocks(1).await?;

    let code_bytes = minimal_contract_artifact();
    let code_b64 = base64::engine::general_purpose::STANDARD.encode(&code_bytes);
    let pk = iroha_data_model::prelude::ExposedPrivateKey(
        iroha_test_samples::ALICE_KEYPAIR.private_key().clone(),
    );
    let authority_literal = iroha_test_samples::ALICE_ID.to_string();
    let body = norito::json::object([
        (
            "authority",
            norito::json::to_value(&authority_literal).expect("serialize authority"),
        ),
        (
            "private_key",
            norito::json::to_value(&format!("{pk}")).expect("serialize private key"),
        ),
        (
            "code_b64",
            norito::json::to_value(&code_b64).expect("serialize bytecode"),
        ),
    ])
    .expect("serialize contract deploy body");

    // POST /v1/contracts/deploy
    let post_url = client.torii_url.join("/v1/contracts/deploy").unwrap();
    let http = reqwest::Client::new();
    let resp = http
        .post(post_url)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .body(norito::json::to_json(&body).unwrap())
        .send()
        .await?;
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        panic!("POST /v1/contracts/deploy returned {status}: {body}");
    }
    let deploy_body: norito::json::Value = norito::json::from_str(&resp.text().await?)?;
    let code_hash_hex = deploy_body
        .get("code_hash_hex")
        .and_then(norito::json::Value::as_str)
        .ok_or_else(|| eyre!("deploy response missing code_hash_hex"))?
        .to_owned();

    // Poll status until we see the deploy transaction committed
    let deadline = Instant::now() + std::time::Duration::from_secs(120);
    let mut status = None;
    let mut last_status_error: Option<String> = None;
    while Instant::now() < deadline {
        match tokio::task::spawn_blocking({
            let client = client.clone();
            move || client.get_status()
        })
        .await
        .expect("poll status")
        {
            Ok(current) => {
                let non_empty = current.blocks_non_empty;
                status = Some(current);
                last_status_error = None;
                if non_empty >= 2 {
                    break;
                }
            }
            Err(err) => {
                last_status_error = Some(err.to_string());
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    let status = status.ok_or_else(|| {
        eyre!(
            "failed to fetch status before deadline{}",
            last_status_error
                .as_deref()
                .map(|err| format!("; last error: {err}"))
                .unwrap_or_default()
        )
    })?;
    if status.blocks_non_empty < 2 {
        return Err(eyre!(
            "expected blocks_non_empty>=2 after manifest registration, got {} (blocks {}, queue {}, approved {}, rejected {}; last status error: {})",
            status.blocks_non_empty,
            status.blocks,
            status.queue_size,
            status.txs_approved,
            status.txs_rejected,
            last_status_error.as_deref().unwrap_or("none")
        ));
    }

    // GET by code hash
    let get_url = client
        .torii_url
        .join(&format!("/v1/contracts/code/{code_hash_hex}"))
        .unwrap();
    let get_deadline = Instant::now() + std::time::Duration::from_secs(120);
    let mut got_txt = None;
    let mut last_get_error: Option<String> = None;
    while Instant::now() < get_deadline {
        let resp = http
            .get(get_url.clone())
            .header("Accept", "application/json")
            .send()
            .await?;
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        if status == StatusCode::NOT_FOUND {
            last_get_error = Some("manifest not found".to_owned());
        } else if !status.is_success() {
            return Err(eyre!(
                "GET /v1/contracts/code/{code_hash_hex} returned {status}: {body}"
            ));
        } else if body.trim().is_empty() {
            last_get_error = Some("empty response body".to_owned());
        } else {
            got_txt = Some(body);
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    let got_txt = got_txt.ok_or_else(|| {
        eyre!(
            "manifest GET did not return JSON before deadline{}",
            last_get_error
                .as_deref()
                .map(|err| format!("; last error: {err}"))
                .unwrap_or_default()
        )
    })?;
    let got: norito::json::Value = norito::json::from_str(&got_txt)?;

    // Validate manifest present and code_bytes absent
    let (got_manifest, got_bytes) = match &got {
        norito::json::Value::Object(m) => (
            m.get("manifest")
                .cloned()
                .unwrap_or(norito::json::Value::Null),
            m.get("code_bytes")
                .cloned()
                .unwrap_or(norito::json::Value::Null),
        ),
        _ => (norito::json::Value::Null, norito::json::Value::Null),
    };
    let got_code = match &got_manifest {
        norito::json::Value::Object(m) => m.get("code_hash").and_then(|v| v.as_str()),
        _ => None,
    };
    assert_eq!(got_code, Some(code_hash_hex.as_str()));
    assert!(got_bytes.is_null(), "code_bytes must be null/absent");

    Ok(())
}

#[tokio::test]
#[ignore = "reproduces live Sora-profile contract state regression"]
async fn contract_state_survives_across_calls_in_sora_profile_network() -> Result<()> {
    let register_permission: Permission = CanRegisterSmartContractCode.into();
    let enact_permission: Permission = CanEnactGovernance.into();
    let builder = NetworkBuilder::new()
        .with_peers(1)
        .with_pipeline_time(Duration::from_secs(4))
        .with_config_layer(|layer| {
            layer
                .write(["sumeragi", "consensus_mode"], "npos")
                .write(["nexus", "enabled"], true)
                .write(["nexus", "lane_count"], 1i64);
        })
        .with_genesis_instruction(Grant::account_permission(
            register_permission,
            iroha_test_samples::ALICE_ID.clone(),
        ))
        .with_genesis_instruction(Grant::account_permission(
            enact_permission,
            iroha_test_samples::ALICE_ID.clone(),
        ));
    let Some(network) = sandbox::start_network_async_or_skip(
        builder,
        stringify!(contract_state_survives_across_calls_in_sora_profile_network),
    )
    .await?
    else {
        return Ok(());
    };

    let client = network.client();
    let http = reqwest::Client::new();
    network.ensure_blocks(1).await?;

    let code_bytes = contract_state_probe_artifact();
    let code_b64 = base64::engine::general_purpose::STANDARD.encode(&code_bytes);
    let pk = iroha_data_model::prelude::ExposedPrivateKey(
        iroha_test_samples::ALICE_KEYPAIR.private_key().clone(),
    );
    let authority_literal = iroha_test_samples::ALICE_ID.to_string();

    let deploy_body = norito::json::object([
        (
            "authority",
            norito::json::to_value(&authority_literal).expect("serialize authority"),
        ),
        (
            "private_key",
            norito::json::to_value(&format!("{pk}")).expect("serialize private key"),
        ),
        (
            "code_b64",
            norito::json::to_value(&code_b64).expect("serialize bytecode"),
        ),
    ])
    .expect("serialize deploy body");

    let baseline = client.get_status()?.txs_approved;
    let deploy_url = client.torii_url.join("/v1/contracts/deploy").unwrap();
    let deploy_resp = http
        .post(deploy_url)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .body(norito::json::to_json(&deploy_body)?)
        .send()
        .await?;
    if !deploy_resp.status().is_success() {
        let status = deploy_resp.status();
        let body = deploy_resp.text().await.unwrap_or_default();
        return Err(eyre!("deploy returned {status}: {body}"));
    }
    let deploy_payload: norito::json::Value = norito::json::from_str(&deploy_resp.text().await?)?;
    let code_hash_hex = deploy_payload
        .get("code_hash_hex")
        .and_then(norito::json::Value::as_str)
        .ok_or_else(|| eyre!("deploy response missing code_hash_hex: {deploy_payload:?}"))?
        .to_owned();
    wait_for_approved_txs(&client, baseline, Duration::from_secs(30), "deploy").await?;

    let activate_body = norito::json::object([
        (
            "authority",
            norito::json::to_value(&authority_literal).expect("serialize authority"),
        ),
        (
            "private_key",
            norito::json::to_value(&format!("{pk}")).expect("serialize private key"),
        ),
        (
            "namespace",
            norito::json::to_value("apps").expect("serialize namespace"),
        ),
        (
            "contract_id",
            norito::json::to_value("contract_state_probe.v1").expect("serialize contract id"),
        ),
        (
            "code_hash",
            norito::json::to_value(&code_hash_hex).expect("serialize code hash"),
        ),
    ])
    .expect("serialize activate body");
    let activate_baseline = client.get_status()?.txs_approved;
    let activate_resp = http
        .post(client.torii_url.join("/v1/contracts/instance/activate")?)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .body(norito::json::to_json(&activate_body)?)
        .send()
        .await?;
    if !activate_resp.status().is_success() {
        let status = activate_resp.status();
        let body = activate_resp.text().await.unwrap_or_default();
        return Err(eyre!("activate returned {status}: {body}"));
    }
    let activate_payload: norito::json::Value =
        norito::json::from_str(&activate_resp.text().await?)?;
    if activate_payload
        .get("submitted")
        .and_then(norito::json::Value::as_bool)
        == Some(false)
    {
        return Err(eyre!(
            "activate produced unsigned scaffold instead of submitting: {activate_payload:?}"
        ));
    }
    if let Some(activate_tx_hash) = activate_payload
        .get("tx_hash_hex")
        .and_then(norito::json::Value::as_str)
    {
        wait_for_tx_applied(
            &http,
            &client.torii_url,
            activate_tx_hash,
            Duration::from_secs(30),
            "activate",
        )
        .await?;
    } else {
        wait_for_approved_txs(
            &client,
            activate_baseline,
            Duration::from_secs(30),
            "activate",
        )
        .await?;
    }

    let init_body = norito::json::object([
        (
            "authority",
            norito::json::to_value(&authority_literal).expect("serialize authority"),
        ),
        (
            "private_key",
            norito::json::to_value(&format!("{pk}")).expect("serialize private key"),
        ),
        (
            "namespace",
            norito::json::to_value("apps").expect("serialize namespace"),
        ),
        (
            "contract_id",
            norito::json::to_value("contract_state_probe.v1").expect("serialize contract id"),
        ),
        (
            "entrypoint",
            norito::json::to_value("init").expect("serialize entrypoint"),
        ),
        (
            "gas_limit",
            norito::json::to_value(&10_000u64).expect("serialize gas limit"),
        ),
    ])
    .expect("serialize init body");
    let init_baseline = client.get_status()?.txs_approved;
    let init_resp = http
        .post(client.torii_url.join("/v1/contracts/call")?)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .body(norito::json::to_json(&init_body)?)
        .send()
        .await?;
    if !init_resp.status().is_success() {
        let status = init_resp.status();
        let body = init_resp.text().await.unwrap_or_default();
        return Err(eyre!("init returned {status}: {body}"));
    }
    let init_payload: norito::json::Value = norito::json::from_str(&init_resp.text().await?)?;
    if init_payload
        .get("submitted")
        .and_then(norito::json::Value::as_bool)
        == Some(false)
    {
        return Err(eyre!(
            "init produced unsigned scaffold instead of submitting: {init_payload:?}"
        ));
    }
    if let Some(init_tx_hash) = init_payload
        .get("tx_hash_hex")
        .and_then(norito::json::Value::as_str)
    {
        wait_for_tx_applied(
            &http,
            &client.torii_url,
            init_tx_hash,
            Duration::from_secs(30),
            "init",
        )
        .await?;
    } else {
        wait_for_approved_txs(&client, init_baseline, Duration::from_secs(30), "init").await?;
    }

    let verify_body = norito::json::object([
        (
            "authority",
            norito::json::to_value(&authority_literal).expect("serialize authority"),
        ),
        (
            "private_key",
            norito::json::to_value(&format!("{pk}")).expect("serialize private key"),
        ),
        (
            "namespace",
            norito::json::to_value("apps").expect("serialize namespace"),
        ),
        (
            "contract_id",
            norito::json::to_value("contract_state_probe.v1").expect("serialize contract id"),
        ),
        (
            "entrypoint",
            norito::json::to_value("verify").expect("serialize entrypoint"),
        ),
        (
            "gas_limit",
            norito::json::to_value(&10_000u64).expect("serialize gas limit"),
        ),
    ])
    .expect("serialize verify body");
    let verify_baseline = client.get_status()?.txs_approved;
    let verify_resp = http
        .post(client.torii_url.join("/v1/contracts/call")?)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .body(norito::json::to_json(&verify_body)?)
        .send()
        .await?;
    if !verify_resp.status().is_success() {
        let status = verify_resp.status();
        let body = verify_resp.text().await.unwrap_or_default();
        return Err(eyre!("verify returned {status}: {body}"));
    }
    let verify_payload: norito::json::Value = norito::json::from_str(&verify_resp.text().await?)?;
    if verify_payload
        .get("submitted")
        .and_then(norito::json::Value::as_bool)
        == Some(false)
    {
        return Err(eyre!(
            "verify produced unsigned scaffold instead of submitting: {verify_payload:?}"
        ));
    }
    if let Some(verify_tx_hash) = verify_payload
        .get("tx_hash_hex")
        .and_then(norito::json::Value::as_str)
    {
        wait_for_tx_applied(
            &http,
            &client.torii_url,
            verify_tx_hash,
            Duration::from_secs(30),
            "verify",
        )
        .await?;
    } else {
        wait_for_approved_txs(&client, verify_baseline, Duration::from_secs(30), "verify").await?;
    }

    let mut state_url = client.torii_url.join("/v1/contracts/state")?;
    state_url
        .query_pairs_mut()
        .append_pair("path", "probe_readback");
    let state_resp = http
        .get(state_url)
        .header("Accept", "application/json")
        .send()
        .await?;
    if !state_resp.status().is_success() {
        let status = state_resp.status();
        let body = state_resp.text().await.unwrap_or_default();
        return Err(eyre!("state query returned {status}: {body}"));
    }
    let state_payload: norito::json::Value = norito::json::from_str(&state_resp.text().await?)?;
    let found = state_payload
        .get("entries")
        .and_then(norito::json::Value::as_array)
        .and_then(|entries| entries.first())
        .and_then(|entry| entry.get("found"))
        .and_then(norito::json::Value::as_bool)
        .unwrap_or(false);
    assert!(
        found,
        "probe_readback should be persisted: {state_payload:?}"
    );

    Ok(())
}
