use std::error::Error;
use std::io::{BufRead, BufReader};

use reqwest::blocking::Client;
use reqwest::header::CONTENT_TYPE;
use sorafs_car::proof_stream::{ProofStreamItem, ProofStreamMetrics, ProofStreamSummary};

/// Fetch an NDJSON proof stream and return the aggregated summary.
pub fn fetch_and_summarise(
    endpoint: &str,
    manifest_digest_hex: &str,
    provider_id: &str,
) -> Result<ProofStreamSummary, Box<dyn Error>> {
    let request_body = norito::json::json!({
        "manifest_digest_hex": manifest_digest_hex,
        "provider_id": provider_id,
        "proof_kind": "por",
        "sample_count": 32,
        "nonce_hex": hex::encode(rand::random::<[u8; 16]>()),
    });
    let request_bytes = norito::json::to_vec(&request_body)?;

    let response = Client::new()
        .post(endpoint)
        .header(CONTENT_TYPE, "application/json")
        .body(request_bytes)
        .send()?;
    if !response.status().is_success() {
        return Err(format!("gateway returned {}", response.status()).into());
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
        if item.status.is_failure() && failures.len() < 5 {
            failures.push(item.clone());
        }
        metrics.record(&item);
        line.clear();
    }

    Ok(ProofStreamSummary::new(metrics, failures))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aggregates_success_and_failure_items() {
        let ndjson = r#"{"verification_status":"success","latency_ms":40}
{"verification_status":"failure","failure_reason":"invalid_proof","latency_ms":95}
"#;

        let server = httpmock::MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(httpmock::Method::POST).path("/stream");
            then.status(200)
                .header("content-type", "application/x-ndjson")
                .body(ndjson);
        });

        let summary = fetch_and_summarise(
            &server.url("/stream"),
            "deadbeef",
            "provider::alpha",
        )
        .expect("summary");

        mock.assert();
        assert_eq!(summary.metrics.item_total, 2);
        assert_eq!(summary.metrics.success_total, 1);
        assert_eq!(summary.metrics.failure_total, 1);
        assert_eq!(summary.failures.len(), 1);
        assert_eq!(
            summary.failures[0]
                .failure_reason
                .as_deref(),
            Some("invalid_proof")
        );
    }
}
