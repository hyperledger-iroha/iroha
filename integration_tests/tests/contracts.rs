//! Torii contract manifest endpoints: POST wraps ISI and GET reads on-chain manifest.

use std::time::Instant;

use eyre::{Result, eyre};
use integration_tests::sandbox;
use iroha::data_model::prelude::*;
use iroha_data_model::smart_contract::manifest::ContractManifest;
use iroha_executor_data_model::permission::smart_contract::CanRegisterSmartContractCode;
use iroha_test_network::NetworkBuilder;
use reqwest::StatusCode;

fn hex32(hash: &Hash) -> String {
    hex::encode(hash.as_ref())
}

#[allow(clippy::too_many_lines)]
#[tokio::test]
async fn post_and_get_contract_manifest_via_torii() -> Result<()> {
    // Grant CanRegisterSmartContractCode to Alice in genesis so she can register manifests.
    let permission: Permission = CanRegisterSmartContractCode.into();
    let builder = NetworkBuilder::new()
        .with_min_peers(4)
        // Keep pipeline timings short to ensure the manifest transaction is flushed promptly.
        .with_pipeline_time(std::time::Duration::from_secs(4))
        .with_config_layer(|layer| {
            // Surface more detail if the pipeline stalls while registering the manifest.
            layer.write(["logger", "level"], "TRACE").write(
                ["logger", "filter"],
                "iroha_core::sumeragi=trace,iroha_core::queue=trace",
            );
        })
        .with_genesis_instruction(Grant::account_permission(
            permission,
            iroha_test_samples::ALICE_ID.clone(),
        ));
    let Some(network) = sandbox::start_network_async_or_skip(
        builder,
        stringify!(post_and_get_contract_manifest_via_torii),
    )
    .await?
    else {
        return Ok(());
    };

    let client = network.client();

    // Wait for genesis to be committed before submitting additional transactions
    network.ensure_blocks(1).await?;

    // Build a deterministic code_hash for POST/GET
    let code_bytes = b"manifest_integration_dummy_code";
    let code_hash = Hash::new(code_bytes);

    // Prepare POST body
    let pk = iroha_data_model::prelude::ExposedPrivateKey(
        iroha_test_samples::ALICE_KEYPAIR.private_key().clone(),
    );
    let manifest = ContractManifest {
        code_hash: Some(code_hash),
        abi_hash: None,
        compiler_fingerprint: None,
        features_bitmap: None,
        access_set_hints: None,
        entrypoints: None,
        kotoba: None,
        provenance: None,
    }
    .signed(&iroha_test_samples::ALICE_KEYPAIR);
    let manifest_json =
        norito::json::to_value(&manifest).expect("serialize contract manifest to JSON");
    let body = norito::json::object([
        (
            "authority",
            norito::json::to_value(&format!("{}", *iroha_test_samples::ALICE_ID))
                .expect("serialize authority"),
        ),
        (
            "private_key",
            norito::json::to_value(&format!("{pk}")).expect("serialize private key"),
        ),
        ("manifest", manifest_json),
    ])
    .expect("serialize contract post body");

    // POST /v1/contracts/code
    let post_url = client.torii_url.join("/v1/contracts/code").unwrap();
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
        panic!("POST /v1/contracts/code returned {status}: {body}");
    }

    // Poll status until we see the manifest transaction committed
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
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
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
        .join(&format!("/v1/contracts/code/{}", hex32(&code_hash)))
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
                "GET /v1/contracts/code/{code_hash} returned {status}: {body}"
            ));
        } else if body.trim().is_empty() {
            last_get_error = Some("empty response body".to_owned());
        } else {
            got_txt = Some(body);
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
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
    assert_eq!(got_code, Some(hex32(&code_hash).as_str()));
    assert!(got_bytes.is_null(), "code_bytes must be null/absent");

    Ok(())
}
