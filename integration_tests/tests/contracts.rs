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
        contract_name: "TestContract".to_owned(),
        compiler_fingerprint: "integration-tests".to_owned(),
        features_bitmap: 0,
        access_set_hints: None,
        kotoba: Vec::new(),
        entrypoints: vec![ivm::EmbeddedEntrypointDescriptor {
            name: "main".to_owned(),
            kind: iroha_data_model::smart_contract::manifest::EntryPointKind::Public,
            params: Vec::new(),
            return_type: None,
            permission: None,
            read_keys: Vec::new(),
            write_keys: Vec::new(),
            access_hints_complete: Some(true),
            access_hints_skipped: Vec::new(),
            triggers: Vec::new(),
            entry_pc: 0,
        }],
        error_codes: Vec::new(),
        states: Vec::new(),
    };
    let mut out = meta.encode();
    out.extend_from_slice(&interface.encode_section());
    out.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
    out
}

fn contract_state_probe_artifact() -> Vec<u8> {
    let src = r#"
seiyaku ContractStateProbe {
  error enum ProbeError {
    NotInitialized = 1
  }

  state Initialized: i64;
  state StoredValue: i64;
  state probe_readback: i64;

  kotoage fn main() -> i64 authorize("CanEnactGovernance") {
    return 0;
  }

  fn initialize_impl() {
    Initialized = 1;
    StoredValue = 7;
    probe_readback = 0;
  }

  hajimari() {
    initialize_impl();
  }

  kotoage fn initialize() authorize("CanEnactGovernance") {
    initialize_impl();
  }

  kotoage fn verify() authorize("CanEnactGovernance") {
    require(Initialized == 1, ProbeError::NotInitialized);
    probe_readback = StoredValue;
  }
}
"#;
    ivm::KotodamaCompiler::new()
        .compile_source(src)
        .expect("compile contract-state probe program")
}

fn dynamic_access_counter_artifact() -> Vec<u8> {
    let src = r#"
seiyaku DynamicAccessCounter {
  state Counters: StateMap<i64, i64>;

  fn bump_hidden(key: i64, delta: i64) {
    let current = Counters.get(key).unwrap_or(0);
    Counters[key] = current + delta;
  }

  kotoage fn bump_direct(key: i64, delta: i64) authorize("CanEnactGovernance") {
    let current = Counters.get(key).unwrap_or(0);
    Counters[key] = current + delta;
  }

  kotoage fn bump_via_helper(key: i64, delta: i64) authorize("CanEnactGovernance") {
    bump_hidden(key, delta);
  }
}
"#;
    let artifact = ivm::KotodamaCompiler::new()
        .compile_source(src)
        .expect("compile dynamic-access counter program");
    let parsed = ivm::ProgramMetadata::parse(&artifact).expect("parse dynamic-access metadata");
    let interface = parsed
        .contract_interface
        .expect("dynamic-access contract interface");
    for entrypoint_name in ["bump_direct", "bump_via_helper"] {
        let entrypoint = interface
            .entrypoints
            .iter()
            .find(|entrypoint| entrypoint.name == entrypoint_name)
            .unwrap_or_else(|| panic!("missing `{entrypoint_name}` entrypoint"));
        assert!(
            entrypoint
                .write_keys
                .iter()
                .any(|key| key == "state:Counters"),
            "`{entrypoint_name}` must transitively report the dynamic StateMap write: {entrypoint:?}"
        );
    }
    artifact
}

fn dynamic_counter_args(key: i64, delta: i64) -> norito::json::Value {
    norito::json::object([
        ("key", norito::json::Value::from(key)),
        ("delta", norito::json::Value::from(delta)),
    ])
    .expect("serialize dynamic counter arguments")
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

fn pipeline_status_block_height(payload: &norito::json::Value) -> Option<u64> {
    payload
        .get("content")
        .and_then(|content| content.get("status"))
        .or_else(|| payload.get("status"))?
        .get("block_height")
        .and_then(norito::json::Value::as_u64)
}

async fn wait_for_tx_applied(
    http: &reqwest::Client,
    torii_url: &reqwest::Url,
    tx_hash_hex: &str,
    timeout: Duration,
    stage: &str,
) -> Result<u64> {
    let mut status_url = torii_url.join("v1/pipeline/transactions/status")?;
    status_url
        .query_pairs_mut()
        .append_pair("hash", tx_hash_hex);
    let deadline = Instant::now() + timeout;
    let mut last_kind = String::from("unavailable");
    let mut last_payload = String::new();
    let mut last_error = String::new();

    loop {
        match http
            .get(status_url.clone())
            .header("Accept", "application/json")
            .send()
            .await
        {
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
                            "Applied" => {
                                if let Some(block_height) = pipeline_status_block_height(&payload) {
                                    return Ok(block_height);
                                }
                                last_kind = "Applied without block_height".to_owned();
                            }
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

async fn deploy_contract_artifact(
    client: &iroha::client::Client,
    http: &reqwest::Client,
    artifact: &[u8],
    alias_name: &str,
    stage: &str,
) -> Result<iroha_data_model::smart_contract::ContractAddress> {
    let baseline = client.get_status()?.txs_approved;
    let code_b64 = base64::engine::general_purpose::STANDARD.encode(artifact);
    let contract_alias = iroha_data_model::smart_contract::ContractAlias::from_components(
        alias_name,
        None,
        "universal",
    )
    .map_err(|error| eyre!("{stage}: invalid contract alias: {error}"))?;
    let response = tokio::task::spawn_blocking({
        let client = client.clone();
        move || {
            client.post_contract_deploy_json(
                &iroha_test_samples::ALICE_ID.clone(),
                iroha_test_samples::ALICE_KEYPAIR.private_key(),
                &code_b64,
                &contract_alias,
                None,
            )
        }
    })
    .await
    .expect("deploy contract task")?;

    if let Some(tx_hash_hex) = response
        .get("tx_hash_hex")
        .and_then(norito::json::Value::as_str)
    {
        wait_for_tx_applied(
            http,
            &client.torii_url,
            tx_hash_hex,
            Duration::from_secs(60),
            stage,
        )
        .await?;
    } else {
        wait_for_approved_txs(client, baseline, Duration::from_secs(60), stage).await?;
    }

    response
        .get("contract_address")
        .and_then(norito::json::Value::as_str)
        .ok_or_else(|| eyre!("{stage}: response missing contract_address: {response:?}"))?
        .parse()
        .map_err(|error| eyre!("{stage}: invalid contract_address: {error}"))
}

async fn contract_state_json_value(
    http: &reqwest::Client,
    torii_url: &reqwest::Url,
    contract_address: &iroha_data_model::smart_contract::ContractAddress,
    path: &str,
) -> Result<norito::json::Value> {
    let mut url = torii_url.join("v1/contracts/state")?;
    url.query_pairs_mut()
        .append_pair("contract_address", &contract_address.to_string())
        .append_pair("path", path)
        .append_pair("decode", "json");
    let response = http
        .get(url)
        .header("Accept", "application/json")
        .send()
        .await?;
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(eyre!("contract state `{path}` returned {status}: {body}"));
    }
    let payload: norito::json::Value = norito::json::from_str(&body)?;
    let entry = payload
        .get("entries")
        .and_then(norito::json::Value::as_array)
        .and_then(|entries| entries.first())
        .ok_or_else(|| eyre!("contract state `{path}` response missing entry: {payload:?}"))?;
    if entry.get("found").and_then(norito::json::Value::as_bool) != Some(true) {
        return Err(eyre!("contract state `{path}` was not found: {payload:?}"));
    }
    entry
        .get("value_json")
        .cloned()
        .ok_or_else(|| eyre!("contract state `{path}` was not decoded: {payload:?}"))
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
    let contract_alias = iroha_data_model::smart_contract::ContractAlias::from_components(
        "deploy_test",
        None,
        "universal",
    )
    .expect("contract alias");
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
        (
            "contract_alias",
            norito::json::to_value(&contract_alias).expect("serialize contract alias"),
        ),
    ])
    .expect("serialize contract deploy body");

    // POST /v1/contracts/deploy
    let post_url = client.torii_url.join("/v1/contracts/deploy").unwrap();
    let http = integration_tests::http::client();
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
        .get("contracts")
        .and_then(norito::json::Value::as_array)
        .and_then(|contracts| contracts.first())
        .and_then(|contract| contract.get("code_hash_hex"))
        .and_then(norito::json::Value::as_str)
        .ok_or_else(|| eyre!("deploy response missing contracts[0].code_hash_hex"))?
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
async fn dynamic_and_helper_hidden_contract_writes_serialize_on_four_peers() -> Result<()> {
    let register_permission: Permission = CanRegisterSmartContractCode.into();
    let alice_enact_permission: Permission = CanEnactGovernance.into();
    let bob_enact_permission: Permission = CanEnactGovernance.into();
    let builder = NetworkBuilder::new()
        .with_peers(4)
        .with_auto_populated_trusted_peers()
        .with_pipeline_time(Duration::from_secs(4))
        .with_config_layer(|layer| {
            layer
                .write(["pipeline", "dynamic_prepass"], true)
                .write(["pipeline", "parallel_overlay"], true)
                .write(["pipeline", "parallel_apply"], true)
                .write(["pipeline", "workers"], 2_i64);
        })
        .with_genesis_instruction(Grant::account_permission(
            register_permission,
            iroha_test_samples::ALICE_ID.clone(),
        ))
        .with_genesis_instruction(Grant::account_permission(
            alice_enact_permission,
            iroha_test_samples::ALICE_ID.clone(),
        ))
        .with_genesis_instruction(Grant::account_permission(
            bob_enact_permission,
            iroha_test_samples::BOB_ID.clone(),
        ));
    let Some(network) = sandbox::start_network_async_or_skip(
        builder,
        stringify!(dynamic_and_helper_hidden_contract_writes_serialize_on_four_peers),
    )
    .await?
    else {
        return Ok(());
    };
    assert_eq!(network.peers().len(), 4, "test requires four voting peers");

    network.ensure_blocks(1).await?;
    let alice_client = network.peers()[0].client();
    let bob_client = network.peers()[1].client();
    let http = integration_tests::http::client();
    let artifact = dynamic_access_counter_artifact();
    let contract_address = deploy_contract_artifact(
        &alice_client,
        &http,
        &artifact,
        "dynamic_access_counter",
        "deploy dynamic-access counter",
    )
    .await?;

    let deploy_height = alice_client.get_status()?.blocks;
    network.ensure_blocks(deploy_height).await?;

    let alice_submission = tokio::task::spawn_blocking({
        let client = alice_client.clone();
        let contract_address = contract_address.clone();
        let payload = dynamic_counter_args(7, 3);
        move || {
            client.post_contract_call_json(
                &iroha_test_samples::ALICE_ID.clone(),
                Some(iroha_test_samples::ALICE_KEYPAIR.private_key()),
                Some(&contract_address),
                None,
                "bump_direct",
                Some(&payload),
                None,
                None,
                None,
                100_000,
            )
        }
    });
    let bob_submission = tokio::task::spawn_blocking({
        let client = bob_client.clone();
        let contract_address = contract_address.clone();
        let payload = dynamic_counter_args(7, 5);
        move || {
            client.post_contract_call_json(
                &iroha_test_samples::BOB_ID.clone(),
                Some(iroha_test_samples::BOB_KEYPAIR.private_key()),
                Some(&contract_address),
                None,
                "bump_via_helper",
                Some(&payload),
                None,
                None,
                None,
                100_000,
            )
        }
    });
    let (alice_response, bob_response) = tokio::join!(alice_submission, bob_submission);
    let alice_response = alice_response.expect("submit direct bump task")?;
    let bob_response = bob_response.expect("submit helper bump task")?;
    let alice_tx_hash = alice_response
        .get("tx_hash_hex")
        .and_then(norito::json::Value::as_str)
        .ok_or_else(|| eyre!("direct bump response missing tx_hash_hex: {alice_response:?}"))?
        .to_owned();
    let bob_tx_hash = bob_response
        .get("tx_hash_hex")
        .and_then(norito::json::Value::as_str)
        .ok_or_else(|| eyre!("helper bump response missing tx_hash_hex: {bob_response:?}"))?
        .to_owned();

    let (alice_block_height, bob_block_height) = tokio::try_join!(
        wait_for_tx_applied(
            &http,
            &alice_client.torii_url,
            &alice_tx_hash,
            Duration::from_secs(60),
            "direct dynamic bump",
        ),
        wait_for_tx_applied(
            &http,
            &bob_client.torii_url,
            &bob_tx_hash,
            Duration::from_secs(60),
            "helper-hidden dynamic bump",
        ),
    )?;
    assert_eq!(
        alice_block_height, bob_block_height,
        "concurrent conflicting calls must be observed in the same committed block"
    );

    network.ensure_blocks(alice_block_height).await?;

    let mut peer_values = Vec::with_capacity(network.peers().len());
    for peer in network.peers() {
        let peer_client = peer.client();
        peer_values.push(
            contract_state_json_value(
                &http,
                &peer_client.torii_url,
                &contract_address,
                "Counters/7",
            )
            .await?,
        );
    }
    let expected = norito::json::Value::from("8");
    assert!(
        peer_values.iter().all(|value| value == &expected),
        "conflicting dynamic calls lost an update or peers diverged: {peer_values:?}"
    );
    assert!(
        peer_values.windows(2).all(|pair| pair[0] == pair[1]),
        "contract state differs across voting peers: {peer_values:?}"
    );

    Ok(())
}

#[tokio::test]
#[ignore = "reproduces live Sora-profile contract state regression"]
async fn contract_state_survives_across_calls_in_sora_profile_network() -> Result<()> {
    let register_permission: Permission = CanRegisterSmartContractCode.into();
    let enact_permission: Permission = CanEnactGovernance.into();
    let builder = NetworkBuilder::new()
        .with_min_peers(4)
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
    let http = integration_tests::http::client();
    network.ensure_blocks(1).await?;

    let code_bytes = contract_state_probe_artifact();
    let code_b64 = base64::engine::general_purpose::STANDARD.encode(&code_bytes);
    let contract_alias = iroha_data_model::smart_contract::ContractAlias::from_components(
        "contract_state_probe",
        None,
        "universal",
    )
    .expect("contract alias");
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
        (
            "contract_alias",
            norito::json::to_value(&contract_alias).expect("serialize contract alias"),
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
        .get("contracts")
        .and_then(norito::json::Value::as_array)
        .and_then(|contracts| contracts.first())
        .and_then(|contract| contract.get("code_hash_hex"))
        .and_then(norito::json::Value::as_str)
        .ok_or_else(|| {
            eyre!("deploy response missing contracts[0].code_hash_hex: {deploy_payload:?}")
        })?
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
            norito::json::to_value("initialize").expect("serialize entrypoint"),
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
