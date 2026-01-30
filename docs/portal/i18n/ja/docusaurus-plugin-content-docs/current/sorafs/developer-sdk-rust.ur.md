---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/developer-sdk-rust.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: developer-sdk-rust
title: Rust SDK snippets
sidebar_label: Rust snippets
description: Proof streams اور manifests consume کرنے کے لیے minimal Rust examples۔
---

:::note مستند ماخذ
:::

اس repository کے Rust crates CLI کو power کرتے ہیں اور custom orchestrators یا services میں embed کیے جا سکتے ہیں۔
نیچے والے snippets ان helpers کو highlight کرتے ہیں جن کی طلب سب سے زیادہ ہوتی ہے۔

## Proof stream helper

HTTP response سے metrics aggregate کرنے کے لیے existing proof stream parser reuse کریں:

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

Full version (tests سمیت) `docs/examples/sorafs_rust_proof_stream.rs` میں ہے۔
`ProofStreamSummary::to_json()` وہی metrics JSON render کرتا ہے جو CLI دیتا ہے، جس سے
observability backends یا CI assertions feed کرنا آسان ہوتا ہے۔

## Multi-source fetch scoring

`sorafs_car::multi_fetch` module وہ async fetch scheduler expose کرتا ہے جو CLI استعمال کرتا ہے۔
`sorafs_car::multi_fetch::ScorePolicy` implement کریں اور `FetchOptions::score_policy` کے ذریعے pass کریں
تاکہ provider ordering tune ہو سکے۔ Unit test `multi_fetch::tests::score_policy_can_filter_providers`
custom preferences enforce کرنے کا طریقہ دکھاتا ہے۔

Other knobs CLI flags کو mirror کرتے ہیں:

- `FetchOptions::per_chunk_retry_limit` CI runs کے لیے `--retry-budget` flag سے match کرتا ہے
  جو retries کو جان بوجھ کر constrain کرتے ہیں۔
- `FetchOptions::global_parallel_limit` کو `--max-peers` کے ساتھ combine کریں تاکہ concurrent
  providers کی تعداد cap ہو۔
- `OrchestratorConfig::with_telemetry_region("region")` `sorafs_orchestrator_*` metrics کو tag کرتا ہے،
  جبکہ `OrchestratorConfig::with_transport_policy` CLI کے `--transport-policy` flag کو mirror کرتا ہے۔
  `TransportPolicy::SoranetPreferred` CLI/SDK surfaces پر default shipped ہے؛ `TransportPolicy::DirectOnly`
  صرف downgrade stage کرنے یا compliance directive follow کرنے پر استعمال کریں، اور `SoranetStrict` کو
  PQ-only pilots کے لیے explicit approval کے ساتھ reserve کریں۔
- `SorafsGatewayFetchOptions::write_mode_hint =
  Some(WriteModeHint::UploadPqOnly)` set کریں تاکہ PQ-only uploads force ہوں؛ helper transport/anonymity
  policies کو خودکار طور پر promote کرے گا جب تک واضح override نہ ہو۔
- `SorafsGatewayFetchOptions::policy_override` استعمال کریں تاکہ ایک request کے لیے temporary transport
  یا anonymity tier pin ہو جائے؛ کسی بھی field کے دینے سے brownout demotion skip ہوتا ہے اور اگر
  requested tier satisfy نہ ہو سکے تو failure آتا ہے۔
- Python (`sorafs_multi_fetch_local` / `sorafs_gateway_fetch`) اور JavaScript (`sorafsMultiFetchLocal`)
  bindings اسی scheduler کو reuse کرتے ہیں، اس لیے ان helpers میں `return_scoreboard=true` set کریں تاکہ
  computed weights chunk receipts کے ساتھ مل سکیں۔
- `SorafsGatewayScoreboardOptions::telemetry_source_label` OTLP stream کو record کرتا ہے جس نے adoption bundle
  بنایا تھا۔ اگر omit ہو تو client خودکار طور پر `region:<telemetry_region>` (یا `chain:<chain_id>`) derive کرتا ہے
  تاکہ metadata میں ہمیشہ descriptive label رہے۔

## Fetch via `iroha::Client`

Rust SDK میں gateway fetch helper شامل ہے؛ manifest کے ساتھ provider descriptors (stream tokens سمیت) فراہم کریں
اور client کو multi-source fetch drive کرنے دیں:

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

جب uploads کو classical relays سے انکار کرنا ہو تو `transport_policy` کو `Some(TransportPolicy::SoranetStrict)`
پر set کریں، یا جب SoraNet کو مکمل bypass کرنا ہو تو `Some(TransportPolicy::DirectOnly)` پر۔
`scoreboard.persist_path` کو release artifact directory پر point کریں، ضرورت ہو تو `scoreboard.now_unix_secs` fix کریں،
اور `scoreboard.metadata` میں capture context (fixture labels، Torii target، وغیرہ) شامل کریں تاکہ
`cargo xtask sorafs-adoption-check` SDKs کے درمیان deterministic JSON consume کرے اور SF-6c والا provenance blob
include رہے۔ `Client::sorafs_fetch_via_gateway` اب اس metadata کو manifest identifier، optional manifest CID expectation،
اور `gateway_manifest_provided` flag کے ساتھ augment کرتا ہے via supplied `GatewayFetchConfig`، تاکہ signed manifest envelope
شامل کرنے والی captures SF-6c evidence requirement پورا کریں بغیر ان fields کو دستی طور پر duplicate کیے۔

## Manifest helpers

`ManifestBuilder` اب بھی Norito payloads programmatically assemble کرنے کا canonical طریقہ ہے:

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

جہاں services کو on-the-fly manifests generate کرنے ہوں وہاں builder embed کریں؛ deterministic pipelines کے لیے CLI ابھی بھی recommended path ہے۔
