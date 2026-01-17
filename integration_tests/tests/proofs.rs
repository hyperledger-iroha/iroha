//! Integration smoke test: submit a proof attachment and query its record via Torii.

use std::{convert::TryFrom as _, str::FromStr as _};

use eyre::{Report, Result};
use integration_tests::sandbox;
use iroha_data_model::proof::{ProofId, ProofStatus};
use iroha_test_network::NetworkBuilder;
use reqwest::Client as HttpClient;
use sha2::{Digest as ShaDigest, Sha256};

fn compute_proof_hash(backend: &str, bytes: &[u8]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(backend.as_bytes());
    h.update(bytes);
    h.finalize().into()
}

fn parse_hex32(input: &str) -> Option<[u8; 32]> {
    let hex = input.strip_prefix("0x").unwrap_or(input);
    if hex.len() != 64 {
        return None;
    }
    let mut out = [0u8; 32];
    let bytes = hex.as_bytes();
    for (i, slot) in out.iter_mut().enumerate() {
        let hi = hex_char(*bytes.get(2 * i)?)?;
        let lo = hex_char(*bytes.get(2 * i + 1)?)?;
        *slot = (hi << 4) | lo;
    }
    Some(out)
}

fn hex_char(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(10 + (b - b'a')),
        b'A'..=b'F' => Some(10 + (b - b'A')),
        _ => None,
    }
}

fn parse_proof_id_from_json(value: &norito::json::Value) -> Option<ProofId> {
    match value {
        norito::json::Value::String(s) => ProofId::from_str(s).ok(),
        norito::json::Value::Object(map) => {
            let backend = map.get("backend")?.as_str()?;
            let proof_hash = map
                .get("proof_hash")
                .and_then(|v| match v {
                    norito::json::Value::String(s) => parse_hex32(s),
                    norito::json::Value::Array(arr) if arr.len() == 32 => {
                        let mut out = [0u8; 32];
                        for (i, item) in arr.iter().enumerate() {
                            let n = u8::try_from(item.as_u64()?).ok()?;
                            out[i] = n;
                        }
                        Some(out)
                    }
                    _ => None,
                })
                .unwrap_or([0; 32]);
            Some(ProofId {
                backend: backend.into(),
                proof_hash,
            })
        }
        _ => None,
    }
}

async fn fetch_proof_snapshot(url: reqwest::Url) -> Result<(String, [u8; 32], ProofStatus)> {
    let response = HttpClient::new()
        .get(url)
        .header("Accept", "application/x-norito, application/json")
        .send()
        .await
        .map_err(Report::new)?
        .error_for_status()
        .map_err(Report::new)?;
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned)
        .unwrap_or_default();
    let bytes = response.bytes().await.map_err(Report::new)?;

    if bytes.len() >= norito::core::Header::SIZE && bytes.starts_with(&norito::core::MAGIC) {
        let record: iroha_data_model::proof::ProofRecord =
            norito::decode_from_bytes(&bytes).map_err(Report::new)?;
        return Ok((
            record.id.backend.clone(),
            record.id.proof_hash,
            record.status,
        ));
    }

    if content_type.starts_with("application/json") || bytes.starts_with(b"{") {
        let value: norito::json::Value = norito::json::from_slice(&bytes).map_err(Report::new)?;
        let status_str = value
            .get("status")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Report::msg("missing status field in proof response"))?;
        let status = match status_str {
            "Submitted" => ProofStatus::Submitted,
            "Verified" => ProofStatus::Verified,
            "Rejected" => ProofStatus::Rejected,
            other => {
                return Err(Report::msg(format!(
                    "unexpected proof status value: {other}"
                )));
            }
        };
        let proof_id = value
            .get("id")
            .and_then(parse_proof_id_from_json)
            .ok_or_else(|| Report::msg("missing id field in proof response"))?;
        return Ok((proof_id.backend.clone(), proof_id.proof_hash, status));
    }

    Err(Report::msg("unexpected proof response payload"))
}

#[tokio::test]
async fn submit_proof_and_query_record() -> Result<()> {
    // Start a minimal network
    let Some(network) = sandbox::start_network_async_or_skip(
        NetworkBuilder::new(),
        stringify!(submit_proof_and_query_record),
    )
    .await?
    else {
        return Ok(());
    };
    let client = network.client();

    // Prepare a dummy proof under a permissive backend (debug/* returns true)
    let backend = "debug/ok";
    let proof_bytes = b"integration_proof_bytes".to_vec();
    let proof = iroha_data_model::proof::ProofBox::new(backend.into(), proof_bytes.clone());
    let attachment = iroha_data_model::proof::ProofAttachment {
        backend: backend.into(),
        proof: proof.clone(),
        vk_ref: None,
        vk_inline: None,
        vk_commitment: None,
        envelope_hash: None,
        lane_privacy: None,
    };
    let isi = iroha_data_model::isi::zk::VerifyProof::new(attachment);

    // Submit as a transaction with a single instruction
    let submit = client.submit_blocking(isi);
    let Some(_hash) = sandbox::handle_result(submit, "submit_proof_and_query_record::submit")?
    else {
        return Ok(());
    };

    // Compute the expected ProofId string for GET /v1/proofs/:id
    let proof_hash = compute_proof_hash(backend, &proof_bytes);
    let pid = iroha_data_model::proof::ProofId {
        backend: backend.into(),
        proof_hash,
    };
    let pid_str = format!("{pid}");

    // Query Torii
    let url = client
        .torii_url
        .join(&format!("/v1/proofs/{pid_str}"))
        .unwrap();
    let snapshot = fetch_proof_snapshot(url).await;
    let Some((backend_got, proof_hash_got, status_got)) =
        sandbox::handle_result(snapshot, "submit_proof_and_query_record::fetch_record")?
    else {
        return Ok(());
    };

    // Validate record fields
    assert_eq!(backend_got, backend);
    assert_eq!(proof_hash_got, proof_hash);
    assert_eq!(status_got, iroha_data_model::proof::ProofStatus::Verified);

    Ok(())
}
