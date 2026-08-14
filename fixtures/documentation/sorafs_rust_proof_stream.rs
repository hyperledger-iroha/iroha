use std::error::Error;
use std::io::BufReader;
use reqwest::blocking::Client;
use reqwest::header::{ACCEPT, ACCEPT_ENCODING, CONTENT_ENCODING, CONTENT_TYPE};
use sorafs_car::{
    proof_stream::{
        ProofKind, ProofStreamMetrics, ProofStreamSummary, ProofStreamVerificationContext,
        por_request_sample_seed_v1, proof_stream_request_digest_v1,
    },
    proof_stream_transport::ProofStreamNdjsonReader,
};
use sorafs_manifest::{ProofStreamHttpRequestV1, ProofStreamRequestV1};
const POR_SAMPLE_COUNT: u32 = 32;
fn canonical_nonzero_hex<const N: usize>(
    raw: &str,
    field: &str,
) -> Result<[u8; N], Box<dyn Error>> {
    if raw.len() != N * 2 {
        return Err(format!(
            "`{field}` must contain exactly {} lowercase hexadecimal characters",
            N * 2
        )
        .into());
    }
    let mut bytes = [0_u8; N];
    hex::decode_to_slice(raw, &mut bytes)?;
    if bytes.iter().all(|byte| *byte == 0) {
        return Err(format!("`{field}` must be non-zero").into());
    }
    if hex::encode(bytes) != raw {
        return Err(format!("`{field}` must use canonical lowercase hexadecimal").into());
    }
    Ok(bytes)
}
/// Fetch an NDJSON proof stream and return the aggregated summary.
///
/// `trusted_por_root_hex` and the finalized cursor must come from the same authenticated,
/// approved ledger projection. Neither value may be learned from the proof-stream response.
/// Load `bearer_token` from a runtime secret and use it for both that readback and this request.
pub fn fetch_and_summarise(
    endpoint: &str,
    manifest_digest_hex: &str,
    provider_id_hex: &str,
    trusted_por_root_hex: &str,
    expected_finalized_height: u64,
    expected_finalized_block_hash_hex: &str,
    bearer_token: &str,
) -> Result<ProofStreamSummary, Box<dyn Error>> {
    let mut nonce = rand::random::<[u8; 16]>();
    while nonce.iter().all(|byte| *byte == 0) {
        nonce = rand::random::<[u8; 16]>();
    }
    fetch_and_summarise_with_nonce(
        endpoint,
        manifest_digest_hex,
        provider_id_hex,
        trusted_por_root_hex,
        expected_finalized_height,
        expected_finalized_block_hash_hex,
        bearer_token,
        nonce,
    )
}
fn fetch_and_summarise_with_nonce(
    endpoint: &str,
    manifest_digest_hex: &str,
    provider_id_hex: &str,
    trusted_por_root_hex: &str,
    expected_finalized_height: u64,
    expected_finalized_block_hash_hex: &str,
    bearer_token: &str,
    nonce: [u8; 16],
) -> Result<ProofStreamSummary, Box<dyn Error>> {
    let manifest_digest = canonical_nonzero_hex::<32>(manifest_digest_hex, "manifest_digest_hex")?;
    let provider_id = canonical_nonzero_hex::<32>(provider_id_hex, "provider_id_hex")?;
    // Obtain this root from the authenticated, approved manifest/ledger record.
    // Never derive it from a proof-stream response.
    let trusted_por_root =
        canonical_nonzero_hex::<32>(trusted_por_root_hex, "trusted_por_root_hex")?;
    let expected_finalized_block_hash = canonical_nonzero_hex::<32>(
        expected_finalized_block_hash_hex,
        "expected_finalized_block_hash_hex",
    )?;
    let proof_request = ProofStreamRequestV1 {
        manifest_digest,
        provider_id,
        proof_kind: ProofKind::Por,
        challenge_id: None,
        sample_count: Some(POR_SAMPLE_COUNT),
        deadline_ms: None,
        sample_seed: None,
        expected_finalized_height: Some(expected_finalized_height),
        expected_finalized_block_hash: Some(expected_finalized_block_hash),
        nonce,
        orchestrator_job_id: None,
        tier: None,
    };
    let request_digest = proof_stream_request_digest_v1(&proof_request)?;
    let sample_seed = por_request_sample_seed_v1(&proof_request, &trusted_por_root)?;
    let verification_context =
        ProofStreamVerificationContext::new(proof_request, Some(trusted_por_root))?;
    if verification_context.request_digest() != &request_digest
        || verification_context.por_sample_seed() != Some(sample_seed)
    {
        return Err(
            "proof-stream request derivation disagrees with its verification context".into(),
        );
    }
    let request = ProofStreamHttpRequestV1::new(proof_request)?;
    let request_bytes = norito::json::to_vec(&request)?;
    let response = Client::new()
        .post(endpoint)
        .bearer_auth(bearer_token)
        .header(CONTENT_TYPE, "application/json")
        .header(ACCEPT, "application/x-ndjson")
        .header(ACCEPT_ENCODING, "identity")
        .body(request_bytes)
        .send()?;
    if !response.status().is_success() {
        return Err(format!("gateway returned {}", response.status()).into());
    }
    if response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        != Some("application/x-ndjson")
    {
        return Err("gateway returned a noncanonical proof-stream Content-Type".into());
    }
    if response
        .headers()
        .get(CONTENT_ENCODING)
        .is_some_and(|value| value != "identity")
    {
        return Err("gateway returned a compressed proof stream".into());
    }
    let mut metrics = ProofStreamMetrics::default();
    for item in ProofStreamNdjsonReader::new(BufReader::new(response), &verification_context) {
        let item = item?;
        if item.proof_kind() != ProofKind::Por
            || item.manifest_digest_hex() != manifest_digest_hex
            || item.provider_id_hex() != provider_id_hex
        {
            return Err("gateway returned a proof item outside the requested PoR scope".into());
        }
        metrics.record(&item);
    }
    if metrics.item_total == 0 {
        return Err("gateway returned an empty proof stream".into());
    }
    if metrics.failure_total != 0 {
        return Err("proof stream reported a failed item".into());
    }
    Ok(ProofStreamSummary::new(metrics, Vec::new()))
}
#[cfg(test)]
mod tests {
    use super::*;
    use norito::json::Value;
    use sorafs_car::{ChunkStore, POR_LEAF_SIZE, por_json::sample_to_map};
    #[test]
    fn aggregates_exact_por_sequence_and_rejects_truncation_and_reordering() {
        let manifest_digest_hex = "11".repeat(32);
        let provider_id_hex = "22".repeat(32);
        let trusted_cursor_height = 17;
        let trusted_cursor_hash = [0x33; 32];
        let trusted_cursor_hash_hex = hex::encode(trusted_cursor_hash);
        let nonce = [0x44; 16];
        let payload = (0..(POR_LEAF_SIZE * 4 + 17))
            .map(|value| u8::try_from(value % 251).expect("fixture byte"))
            .collect::<Vec<_>>();
        let mut store = ChunkStore::new();
        store
            .ingest_bytes(&payload)
            .expect("ingest canonical PoR fixture");
        let request = ProofStreamRequestV1 {
            manifest_digest: [0x11; 32],
            provider_id: [0x22; 32],
            proof_kind: ProofKind::Por,
            challenge_id: None,
            sample_count: Some(POR_SAMPLE_COUNT),
            deadline_ms: None,
            sample_seed: None,
            expected_finalized_height: Some(trusted_cursor_height),
            expected_finalized_block_hash: Some(trusted_cursor_hash),
            nonce,
            orchestrator_job_id: None,
            tier: None,
        };
        let request_digest =
            proof_stream_request_digest_v1(&request).expect("derive canonical request digest");
        let sample_seed = por_request_sample_seed_v1(&request, store.root())
            .expect("derive request-bound PoR sample seed");
        let items = store
            .sample_leaves(
                usize::try_from(POR_SAMPLE_COUNT).expect("sample count fits usize"),
                sample_seed,
                &payload,
            )
            .expect("sample canonical request-bound PoR fixture")
            .into_iter()
            .map(|(flat_index, proof)| {
                let mut item = sample_to_map(flat_index, &proof);
                item.insert(
                    "request_digest_hex".into(),
                    Value::from(hex::encode(request_digest)),
                );
                item.insert(
                    "manifest_digest_hex".into(),
                    Value::from(manifest_digest_hex.clone()),
                );
                item.insert(
                    "provider_id_hex".into(),
                    Value::from(provider_id_hex.clone()),
                );
                item.insert("proof_kind".into(), Value::from("por"));
                item.insert("result".into(), Value::from("success"));
                item.insert("latency_ms".into(), Value::from(40_u64));
                item.insert(
                    "finalized_block_height".into(),
                    Value::from(trusted_cursor_height),
                );
                item.insert(
                    "finalized_block_hash_hex".into(),
                    Value::from(trusted_cursor_hash_hex.clone()),
                );
                Value::Object(item)
            })
            .collect::<Vec<_>>();
        assert!(items.len() > 2, "fixture needs a non-trivial PoR schedule");
        let encode_ndjson = |items: &[Value]| {
            let mut ndjson = String::new();
            for item in items {
                ndjson.push_str(
                    &norito::json::to_string(item).expect("encode canonical PoR stream item"),
                );
                ndjson.push('\n');
            }
            ndjson
        };
        let canonical_ndjson = encode_ndjson(&items);
        let truncated_ndjson = encode_ndjson(&items[..items.len() - 1]);
        let mut reordered_items = items.clone();
        reordered_items.swap(0, 1);
        let reordered_ndjson = encode_ndjson(&reordered_items);
        let server = httpmock::MockServer::start();
        let canonical_mock = server.mock(|when, then| {
            when.method(httpmock::Method::POST).path("/stream");
            then.status(200)
                .header("content-type", "application/x-ndjson")
                .body(canonical_ndjson.clone());
        });
        let truncated_mock = server.mock(|when, then| {
            when.method(httpmock::Method::POST).path("/truncated");
            then.status(200)
                .header("content-type", "application/x-ndjson")
                .body(truncated_ndjson.clone());
        });
        let reordered_mock = server.mock(|when, then| {
            when.method(httpmock::Method::POST).path("/reordered");
            then.status(200)
                .header("content-type", "application/x-ndjson")
                .body(reordered_ndjson.clone());
        });
        let call = |path: &str| {
            fetch_and_summarise_with_nonce(
                &server.url(path),
                &manifest_digest_hex,
                &provider_id_hex,
                &hex::encode(store.root()),
                trusted_cursor_height,
                &trusted_cursor_hash_hex,
                "test-token",
                nonce,
            )
        };
        let summary = call("/stream").expect("canonical exact sequence must verify");
        let truncated_error = call("/truncated")
            .expect_err("request-bound verifier must reject a truncated response")
            .to_string();
        let reordered_error = call("/reordered")
            .expect_err("request-bound verifier must reject reordered samples")
            .to_string();
        canonical_mock.assert();
        truncated_mock.assert();
        reordered_mock.assert();
        assert!(
            truncated_error.contains("ended after"),
            "unexpected truncation error: {truncated_error}"
        );
        assert!(
            reordered_error.contains("has index"),
            "unexpected reorder error: {reordered_error}"
        );
        assert_eq!(
            summary.metrics.item_total,
            u64::try_from(items.len()).expect("fixture item count fits u64")
        );
        assert_eq!(summary.metrics.success_total, summary.metrics.item_total);
        assert_eq!(summary.metrics.failure_total, 0);
        assert!(summary.failure_samples.is_empty());
    }
}
