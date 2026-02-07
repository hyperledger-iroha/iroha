---
id: developer-sdk-rust
lang: hy
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

:::note Կանոնական աղբյուր
:::

Rust-ը այս պահոցում միացնում է CLI-ն և կարող է ներկառուցվել ներսում
պատվերով նվագարկիչներ կամ ծառայություններ: Ստորև բերված հատվածներն ամենաշատը ընդգծում են օգնականներին
մշակողները խնդրում են.

## Ապացուցողական հոսքի օգնական

Կրկին օգտագործեք գոյություն ունեցող ապացույց հոսքի վերլուծիչը՝ HTTP-ի չափումները համախմբելու համար
արձագանք.

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

Ամբողջական տարբերակը (թեստերով) ապրում է `docs/examples/sorafs_rust_proof_stream.rs`-ում։
`ProofStreamSummary::to_json()`-ը ներկայացնում է նույն չափումները JSON, ինչ CLI-ն՝ դարձնելով
հեշտ է կերակրել դիտելիության հետևանքները կամ CI պնդումները:

## Բազմաղբյուրի առբերման միավոր

`sorafs_car::multi_fetch` մոդուլը բացահայտում է ասինխրոն բեռնման ժամանակացույցը
օգտագործվում է CLI-ի կողմից: Իրականացրեք `sorafs_car::multi_fetch::ScorePolicy` և անցեք այն
`FetchOptions::score_policy`-ի միջոցով՝ մատակարարի պատվերը կարգավորելու համար: Միավորի թեստ
`multi_fetch::tests::score_policy_can_filter_providers`-ը ցույց է տալիս, թե ինչպես կիրառել
մաքսային նախապատվություններ:

Մյուս բռնակները արտացոլում են CLI դրոշները.

- `FetchOptions::per_chunk_retry_limit` համապատասխանում է `--retry-budget` դրոշին CI-ի համար
  գործարկումներ, որոնք միտումնավոր սահմանափակում են կրկնվող փորձերը:
- Միավորել `FetchOptions::global_parallel_limit`-ը `--max-peers`-ի հետ՝ փակելու համար
  միաժամանակյա մատակարարների թիվը:
- `OrchestratorConfig::with_telemetry_region("region")` նշում է
  `sorafs_orchestrator_*` չափումներ, մինչդեռ
  `OrchestratorConfig::with_transport_policy`-ը արտացոլում է CLI-ն
  `--transport-policy` դրոշ. `TransportPolicy::SoranetPreferred`-ն այժմ առաքվում է որպես
  լռելյայն CLI/SDK մակերեսների վրա; օգտագործել միայն `TransportPolicy::DirectOnly`
  երբ բեմադրում է վարկանիշի իջեցում կամ համապատասխանության հրահանգին հետևում, և պահում
  `SoranetStrict` միայն PQ օդաչուների համար՝ հստակ հաստատումով:
- Սահմանեք `SorafsGatewayFetchOptions::write_mode_hint =
  Որոշ (WriteModeHint::UploadPqOnly)` ստիպելու միայն PQ վերբեռնումները; օգնականը կամք
  ավտոմատ կերպով խրախուսել տրանսպորտի/անանունության քաղաքականությունը, բացառությամբ այն դեպքերի, երբ դրանք հստակ չեն
  գերագնահատված.
- Օգտագործեք `SorafsGatewayFetchOptions::policy_override`՝ ժամանակավոր տրանսպորտ ամրացնելու համար
  կամ մեկ հարցման համար անանունության մակարդակ; մատակարարելով կամ դաշտը բաց է թողնում
  ավարտված իջեցում և ձախողվում է, երբ պահանջվող մակարդակը չի կարող բավարարվել:
- Python (`sorafs_multi_fetch_local` / `sorafs_gateway_fetch`) և
  JavaScript (`sorafsMultiFetchLocal`) կապերը նորից օգտագործում են նույն ժամանակացույցը, ուստի
  տեղադրեք `return_scoreboard=true` այդ օգնականների մեջ՝ հաշվարկված կշիռները ստանալու համար
  անդորրագրերի կողքին:
- `SorafsGatewayScoreboardOptions::telemetry_source_label`-ը գրանցում է OTLP-ը
  հոսք, որը ստեղծեց որդեգրման փաթեթ: Երբ բաց թողնված է, հաճախորդը բխում է
  `region:<telemetry_region>` (կամ `chain:<chain_id>`) ավտոմատ կերպով մետատվյալներ
  միշտ կրում է նկարագրական պիտակ:

## Ստացեք `iroha::Client`-ի միջոցով

Rust SDK-ն միացնում է դարպասի բեռնման օգնականը. տրամադրել մանիֆեստ գումարած մատակարար
նկարագրիչներ (ներառյալ հոսքային նշաններ) և թույլ տվեք հաճախորդին վարել բազմակի աղբյուրը
բերել:

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

Վերբեռնելիս սահմանեք `transport_policy`-ը մինչև `Some(TransportPolicy::SoranetStrict)`
պետք է հրաժարվի դասական ռելեներից, կամ `Some(TransportPolicy::DirectOnly)` երբ SoraNet
պետք է ամբողջությամբ շրջանցել. Կետ `scoreboard.persist_path` թողարկման ժամանակ
artefact գրացուցակը, ընտրովի շտկեք `scoreboard.now_unix_secs` և լրացրեք
`scoreboard.metadata` գրավման համատեքստով (հարմարանքների պիտակներ, Torii թիրախ և այլն)
այնպես որ `cargo xtask sorafs-adoption-check`-ը սպառում է դետերմինիստական JSON SDK-ներում
հետ ծագման բլբի SF-6c ակնկալում.
`Client::sorafs_fetch_via_gateway`-ն այժմ ավելացնում է այդ մետատվյալները մանիֆեստի հետ
նույնացուցիչը, կամընտիր մանիֆեստի CID ակնկալիքը և
`gateway_manifest_provided` դրոշակը՝ ստուգելով մատակարարվածը
`GatewayFetchConfig`, այնպես որ նկարահանումները, որոնք ներառում են ստորագրված մանիֆեստի ծրար, բավարարում են
SF-6c ապացույցների պահանջը՝ առանց այդ դաշտերը ձեռքով կրկնօրինակելու:

## Մանիֆեստ օգնականներ

`ManifestBuilder`-ը մնում է Norito օգտակար բեռներ հավաքելու կանոնական միջոցը
ծրագրային առումով:

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

Ներդրեք շինարարը, որտեղ ծառայությունները պետք է հայտնվեն անմիջապես: որ
CLI-ն մնում է դետերմինիստական խողովակաշարերի համար առաջարկվող ուղին: