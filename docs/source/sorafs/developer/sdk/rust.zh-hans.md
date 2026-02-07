---
lang: zh-hans
direction: ltr
source: docs/source/sorafs/developer/sdk/rust.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6a3d79242a05afc85bc1c13ddbabe5bb21f4b483430f3571a5b30434d05a9275
source_last_modified: "2026-01-22T14:35:37.734101+00:00"
translation_last_reviewed: 2026-02-07
title: Rust SDK Snippets
summary: Minimal Rust examples for consuming proof streams and manifests.
---

# Rust snippets

The Rust crates shipped in this repository power the CLI and can be embedded in
custom orchestrators or services. The snippet below shows how to reuse the proof
stream helpers to aggregate metrics from an HTTP response.

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

The full version (including a test harness) lives in
`docs/examples/sorafs_rust_proof_stream.rs` and is ready to drop into a binary
crate or integration test.

- `ProofStreamItem::from_ndjson` understands the same schema emitted by Torii
  gateways and the CLI.
- `ProofStreamSummary::to_json()` renders metrics identical to the CLI output,
  making it easy to feed observability backends or CI assertions.
- Extend the function with authenticated requests (stream tokens, bearer tokens)
  as shown in `sorafs_cli proof stream`.

### Multi-source fetch scoring

The `sorafs_car::multi_fetch` module exposes the asynchronous fetch scheduler
used by the CLI. You can plug in custom provider policies by implementing the
`sorafs_car::multi_fetch::ScorePolicy` trait and passing it via
`FetchOptions::score_policy`. Policies receive per-provider statistics and a
chunk spec, then return a `ProviderScoreDecision` that can adjust priority or
skip providers entirely. The unit test
`multi_fetch::tests::score_policy_can_filter_providers` demonstrates how to
prefer a specific provider without modifying the core scheduler.

Additional knobs mirror the CLI flags:

- `FetchOptions::per_chunk_retry_limit` matches the new `--retry-budget` flag
  so CI can clamp the retry budget when simulating failure scenarios.
- Apply `FetchOptions::global_parallel_limit` together with `--max-peers` when
  you need to cap the number of active providers (for example, when trimming a
  discovery set to the best N candidates).
- `OrchestratorConfig::with_telemetry_region("region")` tags the
  `sorafs_orchestrator_*` metrics emitted by the runtime so Grafana dashboards
  and OpenTelemetry exporters can break out activity by region or environment.
- `OrchestratorConfig::with_transport_policy` mirrors the CLI
  `--transport-policy` flag. `TransportPolicy::SoranetPreferred` is now the
  default across binaries; supply `TransportPolicy::DirectOnly` only when a
  downgrade or compliance directive forbids relay use, and reserve
  `TransportPolicy::SoranetStrict` for PQ-only pilots with explicit approval.
- Set `SorafsGatewayFetchOptions::write_mode_hint =
  Some(WriteModeHint::UploadPqOnly)` to force PQ-only uploads: the client will
  switch to `TransportPolicy::SoranetStrict` and `AnonymityPolicy::StrictPq`
  whenever the hint is present unless you explicitly override those fields.
- Use `SorafsGatewayFetchOptions::policy_override` to pin a temporary transport
  or anonymity stage for a single request. Supplying either field skips the
  usual brownout demotion—if the requested tier cannot be satisfied the fetch
  fails instead of silently downgrading. Pair this with a change log entry so
  governance can audit emergency overrides.
- The helper wrappers exposed to the Python
  (`sorafs_multi_fetch_local` / `sorafs_gateway_fetch`) and
  JavaScript (`sorafsMultiFetchLocal`) bindings reuse the logic from this
  module, so multi-language test harnesses can exercise the same scheduler with
  local fixtures. Set `return_scoreboard=true` in those helpers to retrieve the
  computed weights and eligibility decisions alongside chunk receipts.
- `SorafsGatewayScoreboardOptions::telemetry_source_label` records which OTLP
  stream produced the adoption evidence. When you omit it, the client derives
  `region:<telemetry_region>` (or falls back to `chain:<chain_id>`) so metadata
  always carries a descriptive label.

### Fetch via `iroha::Client`

The Rust SDK now bundles the orchestrator helper. Pass a gateway manifest and
provider descriptors (including stream tokens) and let the client drive the
multi-source fetch. Metrics will be tagged with the client's chain identifier.

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
        // Pin Stage C for this fetch; omit `policy_override` to apply staged defaults.
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
must refuse classical relays, or to `Some(TransportPolicy::DirectOnly)` whenever
a deployment must avoid SoraNet entirely; omitting the field keeps the default
SoraNet-first prioritisation.

When adoption evidence is required, point `scoreboard.persist_path` at the
release artefact directory, optionally fix `scoreboard.now_unix_secs`, and fill
`scoreboard.metadata` with structured context (fixture label, capture id, Torii
target, etc.) so `cargo xtask sorafs-adoption-check` and downstream SDKs can
attach the provenance blob that SF-6c demands.
`Client::sorafs_fetch_via_gateway` now augments that metadata with the manifest
identifier, optional manifest CID expectation, and the
`gateway_manifest_provided` flag by inspecting the supplied
`GatewayFetchConfig`, so any capture that includes a `--manifest-envelope`
automatically satisfies the SF-6c evidence requirement without duplicating the
manifest details manually.

### Manifest helpers

`ManifestBuilder` continues to serve as the canonical way to assemble Norito
payloads programmatically:

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

Use the builder inside services that need to generate manifests on the fly,
while the CLI remains the recommended path for deterministic pipelines.

### DA proof artefacts

The Rust client now ships the same proof emitters as the CLI, letting SDKs
produce PoR artefacts without shelling out to `iroha app da prove`. Provide the
paths (or labels) that should appear in the JSON so downstream governance
reports can trace every artefact to its source.

```rust
use eyre::Result;
use iroha::{
    Client,
    da::{DaProofArtifactMetadata, DaProofConfig},
};

async fn emit_proof(client: &Client, storage_ticket: &str) -> Result<()> {
    let bundle = client.get_da_manifest_bundle(storage_ticket)?;
    let plan = client.build_da_car_plan(&bundle)?;
    let payload_bytes = client
        .sorafs_fetch_via_gateway(&plan, gateway_config(), providers(), Default::default())
        .await?
        .assemble_payload();

    let metadata = DaProofArtifactMetadata::new(
        "artifacts/da/manifest.to",
        "artifacts/da/payload.car",
    );
    // Build the JSON structure (matches `iroha app da prove --json-out`):
    let summary = client.build_da_proof_artifact(
        &bundle,
        &payload_bytes,
        &DaProofConfig::default(),
        &metadata,
    )?;

    // Persist the same artefact with pretty JSON + trailing newline.
    client.write_da_proof_artifact(
        &bundle,
        &payload_bytes,
        &DaProofConfig::default(),
        &metadata,
        "artifacts/da/proof.json",
        true,
    )?;
    println!("emitted {} proofs", summary["proof_count"]);
    Ok(())
}
```

`DaProofArtifactMetadata` records the manifest/payload identifiers that should
appear in the artefact (typically the paths written to disk). Toggle the `pretty`
flag when calling `write_da_proof_artifact` if compact JSON is preferred. The
resulting files are identical to the CLI output, so `cargo xtask sorafs-adoption-check`
and release pipelines can ingest SDK-generated artefacts transparently.
