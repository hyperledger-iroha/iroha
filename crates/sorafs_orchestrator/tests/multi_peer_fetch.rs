//! Integration tests for the multi-source orchestrator using fixture providers.

use std::{
    collections::{HashMap, HashSet},
    num::NonZeroUsize,
    sync::{Arc, Mutex},
    time::Duration,
};

use blake3::hash as blake3_hash;
use sorafs_car::{
    fixtures::MultiPeerFixture,
    multi_fetch::{ChunkResponse, FetchRequest},
};
use sorafs_orchestrator::{Orchestrator, OrchestratorConfig, PolicyStatus};
use thiserror::Error;

#[derive(Debug, Error)]
#[error("{0}")]
struct FetchError(&'static str);

#[tokio::test(flavor = "multi_thread")]
async fn orchestrator_recovers_from_provider_failures() {
    let fixture = MultiPeerFixture::with_providers(3);

    let mut config = OrchestratorConfig::default();
    config.scoreboard.now_unix_secs = fixture.now_unix_secs();
    config.scoreboard.telemetry_grace_period = Duration::from_hours(1);
    config.scoreboard.latency_cap_ms = 2_500;
    config.fetch.per_chunk_retry_limit = Some(5);
    config.fetch.provider_failure_threshold = 2;
    config.fetch.global_parallel_limit = Some(3);
    config.max_providers = NonZeroUsize::new(3);
    config.telemetry_region = Some("fixture".into());

    let orchestrator = Orchestrator::new(config);
    let scoreboard = orchestrator
        .build_scoreboard(fixture.plan(), fixture.providers(), fixture.telemetry())
        .expect("scoreboard build");

    let provider_payloads = fixture
        .providers()
        .iter()
        .zip(fixture.provider_payloads())
        .filter_map(|(metadata, payload)| metadata.provider_id.as_ref().map(|id| (id, payload)))
        .map(|(id, payload)| (id.clone(), Arc::new(payload.clone())))
        .collect::<HashMap<String, Arc<Vec<u8>>>>();
    let provider_payloads = Arc::new(provider_payloads);

    let transient_failures = Arc::new(Mutex::new(HashSet::new()));
    let corrupt_once = Arc::new(Mutex::new(HashSet::new()));

    let fetcher = {
        let provider_payloads = Arc::clone(&provider_payloads);
        let transient_failures = Arc::clone(&transient_failures);
        let corrupt_once = Arc::clone(&corrupt_once);
        move |request: FetchRequest| {
            let provider_payloads = Arc::clone(&provider_payloads);
            let transient_failures = Arc::clone(&transient_failures);
            let corrupt_once = Arc::clone(&corrupt_once);
            async move {
                let provider_id = request.provider.id().as_str().to_owned();
                let chunk_index = request.spec.chunk_index;

                if provider_id.ends_with('1') {
                    let mut guard = transient_failures.lock().expect("lock transient failures");
                    if guard.insert((provider_id.clone(), chunk_index)) {
                        return Err(FetchError("simulated transport error"));
                    }
                }

                let payload = provider_payloads
                    .get(&provider_id)
                    .expect("fixture provider present");
                let start = request.spec.offset as usize;
                let end = start + request.spec.length as usize;
                let mut bytes = payload[start..end].to_vec();

                if provider_id.ends_with('2') {
                    let mut guard = corrupt_once.lock().expect("lock corruption flag");
                    if guard.insert((provider_id.clone(), chunk_index)) {
                        bytes[0] ^= 0xFF;
                    }
                }

                Ok::<ChunkResponse, FetchError>(ChunkResponse::new(bytes))
            }
        }
    };

    let session = orchestrator
        .fetch_with_scoreboard(fixture.plan(), &scoreboard, fetcher)
        .await
        .expect("fetch outcome");

    let outcome = &session.outcome;

    let expected_chunk_count = fixture.plan().chunks.len();
    assert_eq!(
        outcome.chunks.len(),
        expected_chunk_count,
        "chunk count must match plan"
    );
    assert_eq!(
        outcome.chunk_receipts.len(),
        expected_chunk_count,
        "receipts must cover every chunk"
    );

    let assembled = outcome.assemble_payload();
    assert_eq!(
        assembled.as_slice(),
        fixture.payload(),
        "assembled payload must match fixture input"
    );

    for (idx, chunk_bytes) in outcome.chunks.iter().enumerate() {
        let plan_chunk = &fixture.plan().chunks[idx];
        assert_eq!(
            chunk_bytes.len(),
            plan_chunk.length as usize,
            "chunk length mismatch for index {idx}"
        );
        let digest = blake3_hash(chunk_bytes);
        assert_eq!(
            digest.as_bytes(),
            &plan_chunk.digest,
            "digest mismatch for chunk {idx}"
        );
    }

    let mut seen_indices = outcome
        .chunk_receipts
        .iter()
        .map(|receipt| receipt.chunk_index)
        .collect::<Vec<_>>();
    let mut expected_indices = (0..expected_chunk_count).collect::<Vec<_>>();
    seen_indices.sort_unstable();
    expected_indices.sort_unstable();
    assert_eq!(
        seen_indices, expected_indices,
        "receipts must cover every chunk index exactly once"
    );

    assert_eq!(
        outcome.provider_reports.len(),
        3,
        "three providers should be tracked"
    );

    for report in &outcome.provider_reports {
        assert!(
            report.successes > 0,
            "provider {} delivered no chunks",
            report.provider.id()
        );
    }

    let provider_one = outcome
        .provider_reports
        .iter()
        .find(|report| report.provider.id().as_str() == "fixture-provider-1")
        .expect("provider 1 present");
    assert!(
        provider_one.failures >= 1,
        "provider-1 should record at least one failure"
    );

    let provider_two = outcome
        .provider_reports
        .iter()
        .find(|report| report.provider.id().as_str() == "fixture-provider-2")
        .expect("provider 2 present");
    assert!(
        provider_two.failures >= 1,
        "provider-2 should record at least one failure"
    );

    assert_eq!(session.policy_report.status, PolicyStatus::NotApplicable);
}
