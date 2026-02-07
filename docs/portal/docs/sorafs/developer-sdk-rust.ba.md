---
lang: ba
direction: ltr
source: docs/portal/docs/sorafs/developer-sdk-rust.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4fb8761802761aad2b91202fbb11136734036d46c0245814616492ad90b12258
source_last_modified: "2026-01-05T09:28:11.869572+00:00"
translation_last_reviewed: 2026-02-07
id: developer-sdk-rust
title: Rust SDK Snippets
sidebar_label: Rust snippets
description: Minimal Rust examples for consuming proof streams and manifests.
---

:::note Canonical Source
:::

The Rust crates in this repository power the CLI and can be embedded inside
custom orchestrators or services. The snippets below highlight the helpers most
developers ask for.

## Proof stream helper

Reuse the existing proof stream parser to aggregate metrics from an HTTP
response:

```rust
use std::error::Error;
use std::io::{BufRead, BufReader};

use reqwest::blocking::Response;
use sorafs_car::proof_stream::{ProofStreamItem, ProofStreamMetrics, ProofStreamSummary};

/// Consume an NDJSON proof stream and return aggregated metrics.
pub fn collect_proof_metrics(response: Response) -> Result<ProofStreamSummary, Box<dyn Error>> {
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
```

The full version (with tests) lives in `docs/examples/sorafs_rust_proof_stream.rs`.
`ProofStreamSummary::to_json()` renders the same metrics JSON as the CLI, making
it easy to feed observability backends or CI assertions.

## Multi-source fetch scoring

The `sorafs_car::multi_fetch` module exposes the asynchronous fetch scheduler
used by the CLI. Implement `sorafs_car::multi_fetch::ScorePolicy` and pass it
via `FetchOptions::score_policy` to tune provider ordering. The unit test
`multi_fetch::tests::score_policy_can_filter_providers` shows how to enforce
custom preferences.

Other knobs mirror CLI flags:

- `FetchOptions::per_chunk_retry_limit` matches the `--retry-budget` flag for CI
  runs that intentionally constrain retries.
- Combine `FetchOptions::global_parallel_limit` with `--max-peers` to cap the
  number of concurrent providers.
- `OrchestratorConfig::with_telemetry_region("region")` tags the
  `sorafs_orchestrator_*` metrics, while
  `OrchestratorConfig::with_transport_policy` mirrors the CLI
  `--transport-policy` flag. `TransportPolicy::SoranetPreferred` now ships as
  the default across CLI/SDK surfaces; use `TransportPolicy::DirectOnly` only
  when staging a downgrade or following a compliance directive, and reserve
  `SoranetStrict` for PQ-only pilots with explicit approval.
- Set `SorafsGatewayFetchOptions::write_mode_hint =
  Some(WriteModeHint::UploadPqOnly)` to force PQ-only uploads; the helper will
  automatically promote the transport/anonymity policies unless explicitly
  overridden.
- Use `SorafsGatewayFetchOptions::policy_override` to pin a temporary transport
  or anonymity tier for a single request; supplying either field skips the
  brownout demotion and fails when the requested tier cannot be satisfied.
- The Python (`sorafs_multi_fetch_local` / `sorafs_gateway_fetch`) and
  JavaScript (`sorafsMultiFetchLocal`) bindings reuse the same scheduler, so
  set `return_scoreboard=true` in those helpers to retrieve the computed weights
  alongside chunk receipts.
- `SorafsGatewayScoreboardOptions::telemetry_source_label` records the OTLP
  stream that produced an adoption bundle. When omitted, the client derives
  `region:<telemetry_region>` (or `chain:<chain_id>`) automatically so metadata
  always carries a descriptive label.

## Fetch via `iroha::Client`

The Rust SDK bundles the gateway fetch helper; provide a manifest plus provider
descriptors (including stream tokens) and let the client drive the multi-source
fetch:

```rust
use eyre::Result;
use iroha::{
    Client,
    client::{SorafsGatewayFetchOptions, SorafsGatewayScoreboardOptions},
};
use sorafs_car::CarBuildPlan;
use sorafs_orchestrator::{
    AnonymityPolicy, PolicyOverride,
    prelude::{GatewayFetchConfig, GatewayProviderInput, TransportPolicy},
};
use std::path::PathBuf;

pub async fn fetch_payload(
    client: &Client,
    plan: &CarBuildPlan,
    gateway: GatewayFetchConfig,
    providers: Vec<GatewayProviderInput>,
) -> Result<Vec<u8>> {
    let options = SorafsGatewayFetchOptions {
        transport_policy: Some(TransportPolicy::SoranetPreferred),
        // Pin Stage C for this fetch; omit `policy_override` to apply staged defaults.
        policy_override: PolicyOverride::new(
            Some(TransportPolicy::SoranetStrict),
            Some(AnonymityPolicy::StrictPq),
        ),
        write_mode_hint: None,
        scoreboard: Some(SorafsGatewayScoreboardOptions {
            persist_path: Some(
                PathBuf::from("artifacts/sorafs_orchestrator/latest/scoreboard.json"),
            ),
            now_unix_secs: None,
            metadata: Some(norito::json!({
                "capture_id": "sdk-smoke-run",
                "fixture": "multi_peer_parity_v1"
            })),
            telemetry_source_label: Some("otel::staging".into()),
        }),
        ..SorafsGatewayFetchOptions::default()
    };
    let outcome = client
        .sorafs_fetch_via_gateway(plan, gateway, providers, options)
        .await?;
    Ok(outcome.assemble_payload())
}
```

Set `transport_policy` to `Some(TransportPolicy::SoranetStrict)` when uploads
must refuse classical relays, or `Some(TransportPolicy::DirectOnly)` when SoraNet
must be bypassed entirely. Point `scoreboard.persist_path` at the release
artefact directory, optionally fix `scoreboard.now_unix_secs`, and populate
`scoreboard.metadata` with capture context (fixture labels, Torii target, etc.)
so `cargo xtask sorafs-adoption-check` consumes deterministic JSON across SDKs
with the provenance blob SF-6c expects.
`Client::sorafs_fetch_via_gateway` now augments that metadata with the manifest
identifier, optional manifest CID expectation, and the
`gateway_manifest_provided` flag by inspecting the supplied
`GatewayFetchConfig`, so captures that include a signed manifest envelope satisfy
the SF-6c evidence requirement without duplicating those fields manually.

## Manifest helpers

`ManifestBuilder` remains the canonical way to assemble Norito payloads
programmatically:

```rust
use sorafs_manifest::{ManifestBuilder, ManifestV1, PinPolicy, StorageClass};

fn build_manifest(bytes: &[u8]) -> Result<ManifestV1, Box<dyn std::error::Error>> {
    let mut builder = ManifestBuilder::new();
    builder.pin_policy(PinPolicy {
        min_streams: 3,
        storage_class: StorageClass::Warm,
        retention_epoch: Some(48),
    });
    builder.payload(bytes)?;
    Ok(builder.build()?)
}
```

Embed the builder wherever services need to generate manifests on the fly; the
CLI remains the recommended path for deterministic pipelines.
