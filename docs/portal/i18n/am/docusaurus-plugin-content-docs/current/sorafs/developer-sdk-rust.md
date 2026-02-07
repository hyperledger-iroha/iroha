---
id: developer-sdk-rust
lang: am
direction: ltr
source: docs/portal/docs/sorafs/developer-sdk-rust.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Rust SDK Snippets
sidebar_label: Rust snippets
description: Minimal Rust examples for consuming proof streams and manifests.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

::: ማስታወሻ ቀኖናዊ ምንጭ
::

የዝገቱ ሳጥኖች በዚህ ማከማቻ ሃይል ውስጥ CLI ን ያሰራጫሉ እና በውስጡም ሊካተት ይችላል።
ብጁ ኦርኬስትራዎች ወይም አገልግሎቶች. ከታች ያሉት ቅንጥቦች ረዳቶቹን በጣም ያደምቃሉ
ገንቢዎች ይጠይቃሉ.

## የፍሰት ረዳት

የኤችቲቲፒ መለኪያዎችን ለማዋሃድ ያለውን የማረጋገጫ ዥረት ተንታኝ እንደገና ይጠቀሙ
ምላሽ፡-

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

ሙሉው ስሪት (ከሙከራዎች ጋር) በ`docs/examples/sorafs_rust_proof_stream.rs` ውስጥ ይኖራል።
`ProofStreamSummary::to_json()` JSON ከ CLI ጋር ተመሳሳይ መለኪያዎችን ያቀርባል፣
የታዛቢነት ጀርባዎችን ወይም የCI ማረጋገጫዎችን መመገብ ቀላል ነው።

## ባለ ብዙ ምንጭ ማምጣት

የ`sorafs_car::multi_fetch` ሞጁል ያልተመሳሰለውን ፈልጎ መርሐግብር ያጋልጣል
በ CLI ጥቅም ላይ የዋለ. `sorafs_car::multi_fetch::ScorePolicy`ን ይተግብሩ እና ይለፉ
በ`FetchOptions::score_policy` በኩል የአቅራቢውን ትዕዛዝ ለማስተካከል። የክፍል ሙከራ
`multi_fetch::tests::score_policy_can_filter_providers` እንዴት ማስፈጸም እንደሚቻል ያሳያል
ብጁ ምርጫዎች.

ሌሎች አንጓዎች የCLI ባንዲራዎችን ያንጸባርቃሉ፡

- `FetchOptions::per_chunk_retry_limit` ለ CI ባንዲራ ከ `--retry-budget` ጋር ይዛመዳል
  ሆን ብሎ ሙከራዎችን የሚገድብ ያስኬዳል።
- የ `FetchOptions::global_parallel_limit`ን ከ `--max-peers` ጋር ያዋህዱ
  በተመሳሳይ ጊዜ የአቅራቢዎች ብዛት።
- `OrchestratorConfig::with_telemetry_region("region")` መለያ ይሰጣል
  `sorafs_orchestrator_*` ሜትሪክስ፣ እያለ
  `OrchestratorConfig::with_transport_policy` CLI ን ያንፀባርቃል
  `--transport-policy` ባንዲራ። `TransportPolicy::SoranetPreferred` አሁን እንደ
  በ CLI/SDK ወለል ላይ ያለው ነባሪ; `TransportPolicy::DirectOnly` ብቻ ይጠቀሙ
  የማውረድ ደረጃን ሲያዘጋጁ ወይም የማክበር መመሪያን ሲከተሉ እና ያስያዙ
  `SoranetStrict` ለPQ-ብቻ አብራሪዎች ግልጽ ይሁንታ ያላቸው።
- `SorafsGatewayFetchOptions ::write_mode_hint = አዘጋጅ
  PQ-ብቻ ሰቀላዎችን ለማስገደድ አንዳንድ(WriteModeHint:: UploadPqOnly)`; ረዳቱ ያደርጋል
  በግልጽ ካልሆነ በስተቀር የትራንስፖርት/ስም-መታወቅ ፖሊሲዎችን በራስ-ሰር ያስተዋውቁ
  የተሻረ።
- ጊዜያዊ መጓጓዣን ለመሰካት `SorafsGatewayFetchOptions::policy_override` ይጠቀሙ
  ወይም ለአንድ ነጠላ ጥያቄ ማንነታቸው የማይታወቅ ደረጃ; የትኛውንም መስክ ማቅረቡ ይዘላል
  የተጠየቀው እርከን ሊረካ በማይችልበት ጊዜ ቡናማ ያልሆነ እና ያልተሳካለት።
- ፓይዘን (`sorafs_multi_fetch_local` / I18NI0000024X) እና
  ጃቫ ስክሪፕት (`sorafsMultiFetchLocal`) ማሰሪያዎች ተመሳሳዩን መርሐግብር እንደገና ይጠቀማሉ፣ ስለዚህ
  የተሰሉትን ክብደቶች ለማውጣት በእነዚያ ረዳቶች ውስጥ `return_scoreboard=true` አዘጋጅ
  ከተቆራረጡ ደረሰኞች ጋር.
- `SorafsGatewayScoreboardOptions::telemetry_source_label` OTLP ይመዘግባል
  የጉዲፈቻ ጥቅል ያመረተ ዥረት። ሲቀር ደንበኛው ያገኛል
  `region:<telemetry_region>` (ወይም `chain:<chain_id>`) በራስ ሰር ስለዚህ ሜታዳታ
  ሁልጊዜ ገላጭ መለያ ይይዛል።

## በI18NI0000030X በኩል አምጣ

የዝገቱ ኤስዲኬ የመግቢያ መንገዱን ረዳት ያጠቃልላል፤ አንጸባራቂ ፕላስ አቅራቢ ያቅርቡ
ገላጭ (የዥረት ቶከንን ጨምሮ) እና ደንበኛው ባለብዙ-ምንጩን እንዲነዳ ያድርጉት
አምጣ፡

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

በሚሰቀልበት ጊዜ `transport_policy` ወደ `Some(TransportPolicy::SoranetStrict)` ያቀናብሩ
ክላሲካል ቅብብሎሽ እምቢ ማለት አለበት፣ ወይም `Some(TransportPolicy::DirectOnly)` መቼ SoraNet
ሙሉ በሙሉ መታለፍ አለበት. በሚለቀቅበት ጊዜ `scoreboard.persist_path` ነጥብ
artefact directory፣ እንደ አማራጭ `scoreboard.now_unix_secs` አስተካክል እና ሙላ
`scoreboard.metadata` ከተቀረጸ አውድ ጋር (ቋሚ መለያዎች፣ Torii ዒላማ፣ ወዘተ)
ስለዚህ `cargo xtask sorafs-adoption-check` የሚወስነው JSON በመላው ኤስዲኬዎች ይበላል
ከፕሮቬንሽን ብሌብ ጋር SF-6c ይጠብቃል.
`Client::sorafs_fetch_via_gateway` አሁን ያንን ሜታዳታ ከማንፀባረቂያው ጋር ይጨምራል
መለያ፣ የአማራጭ አንጸባራቂ CID መጠበቅ እና የ
`gateway_manifest_provided` ባንዲራ የቀረበውን በመመርመር
`GatewayFetchConfig`፣ስለዚህ የተፈረመ አንጸባራቂ ኤንቨሎፕ ያካተቱ ቀረጻዎች ያረካሉ
የ SF-6c ማስረጃ መስፈርቶች እነዚያን መስኮች በእጅ ሳይባዙ።

## ረዳቶችን አሳይ

`ManifestBuilder` Norito ጭነትን ለመሰብሰብ ቀኖናዊ መንገድ ሆኖ ይቆያል
በፕሮግራም:

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

አግልግሎቶች በሚበሩበት ቦታ ሁሉ ገንቢውን ይክተቱ ፣ የ
CLI ለመወሰን የሚመከር መንገድ ሆኖ ይቆያል።