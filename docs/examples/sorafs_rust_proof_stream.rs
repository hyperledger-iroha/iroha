use std::error::Error;
use std::io::{BufRead, BufReader};

use reqwest::blocking::Client;
use reqwest::header::{ACCEPT, ACCEPT_ENCODING, CONTENT_ENCODING, CONTENT_TYPE};
use sorafs_car::proof_stream::{
    ProofKind, ProofStreamItem, ProofStreamMetrics, ProofStreamSummary,
};
use sorafs_manifest::{ProofStreamHttpRequestV1, ProofStreamRequestV1};

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
pub fn fetch_and_summarise(
    endpoint: &str,
    manifest_digest_hex: &str,
    provider_id_hex: &str,
) -> Result<ProofStreamSummary, Box<dyn Error>> {
    let manifest_digest = canonical_nonzero_hex::<32>(manifest_digest_hex, "manifest_digest_hex")?;
    let provider_id = canonical_nonzero_hex::<32>(provider_id_hex, "provider_id_hex")?;
    let mut nonce = rand::random::<[u8; 16]>();
    while nonce.iter().all(|byte| *byte == 0) {
        nonce = rand::random::<[u8; 16]>();
    }
    let request = ProofStreamHttpRequestV1::new(ProofStreamRequestV1 {
        manifest_digest,
        provider_id,
        proof_kind: ProofKind::Por,
        challenge_id: None,
        sample_count: Some(32),
        deadline_ms: None,
        sample_seed: None,
        nonce,
        orchestrator_job_id: None,
        tier: None,
    })?;
    let request_bytes = norito::json::to_vec(&request)?;

    let response = Client::new()
        .post(endpoint)
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

    let mut reader = BufReader::new(response);
    let mut line = String::new();
    let mut metrics = ProofStreamMetrics::default();
    let mut failures = Vec::new();

    while reader.read_line(&mut line)? != 0 {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            line.clear();
            continue;
        }
        let item = ProofStreamItem::from_ndjson(trimmed.as_bytes())?;
        if item.proof_kind() != ProofKind::Por
            || item.manifest_digest_hex() != manifest_digest_hex
            || item.provider_id_hex() != provider_id_hex
        {
            return Err("gateway returned a proof item outside the requested PoR scope".into());
        }
        if item.status().is_failure() && failures.len() < 5 {
            failures.push(item.clone());
        }
        metrics.record(&item);
        line.clear();
    }
    if metrics.item_total == 0 {
        return Err("gateway returned an empty proof stream".into());
    }

    Ok(ProofStreamSummary::new(metrics, failures))
}

#[cfg(test)]
mod tests {
    use super::*;
    use norito::json::Value;
    use sorafs_car::{ChunkStore, por_json::sample_to_map};

    #[test]
    fn aggregates_a_canonical_por_item() {
        let manifest_digest_hex = "11".repeat(32);
        let provider_id_hex = "22".repeat(32);
        let payload = (0_u16..512)
            .map(|value| u8::try_from(value % 251).expect("fixture byte"))
            .collect::<Vec<_>>();
        let mut store = ChunkStore::new();
        store
            .ingest_bytes(&payload)
            .expect("ingest canonical PoR fixture");
        let (flat_index, proof) = store
            .sample_leaves(1, 7, &payload)
            .expect("sample canonical PoR fixture")
            .into_iter()
            .next()
            .expect("one PoR sample");
        let mut item = sample_to_map(flat_index, &proof);
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
        let ndjson = format!(
            "{}\n",
            norito::json::to_string(&Value::Object(item)).expect("encode PoR item")
        );

        let server = httpmock::MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(httpmock::Method::POST).path("/stream");
            then.status(200)
                .header("content-type", "application/x-ndjson")
                .body(ndjson.clone());
        });

        let summary = fetch_and_summarise(
            &server.url("/stream"),
            &manifest_digest_hex,
            &provider_id_hex,
        )
        .expect("summary");

        mock.assert();
        assert_eq!(summary.metrics.item_total, 1);
        assert_eq!(summary.metrics.success_total, 1);
        assert_eq!(summary.metrics.failure_total, 0);
        assert!(summary.failure_samples.is_empty());
    }
}
