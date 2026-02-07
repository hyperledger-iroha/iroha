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
translator: machine-google-reviewed
---

:::иҫкәртергә канонлы сығанаҡ
::: 1990 й.

Был һаҡлағыс ҡөҙрәтендә Rust йәшниктәр CLI һәм эсендә һеңдерергә мөмкин
ҡулланыусылар өсөн оркестр йәки хеҙмәттәр. Түбәндәге өҙөктәр ярҙамсыларҙы иң айырып күрһәтә
төҙөүселәр һорай.

## Дәлилдәр ағымы ярҙамсыһы

Ҡабаттан ҡулланыу ғәмәлдәге иҫбатлау ағымы анализлаусы агрегация метрикаһы HTTP .
яуап:

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

Тулы версия (тестар менән) йәшәй I18NI000000005X.
I18NI000000006X шул уҡ метрика JSON CLI кеүек күрһәтә, етештереү
был еңел туҡландырыу күҙәтеүсәнлеге бекэндтар йәки CI раҫлауҙары.

## Күп сығанаҡлы фетч балл

I18NI000000007X модуле асинхрон фетч планлаштырыусы фашлай
CLI ҡулланған. I18NI000000008X тормошҡа ашырыу һәм уны тапшырырға
аша `FetchOptions::score_policy` көйләү өсөн провайдер заказ. Блок һынауы
I18NI000000010X нисек күрһәтеүен күрһәтә
ҡулланыусылар өҫтөнлөктәре.

Башҡа ручкалар CLI флагтарын көҙгөләй:

- `FetchOptions::per_chunk_retry_limit` матчтар I18NI0000000012X өсөн CI .
  йүгерә, тип аңлы рәүештә сикләү ретт.
- `FetchOptions::global_parallel_limit` менән I18NI0000000014X менән берләшергә кәрәк.
  һаны бер үк ваҡытта провайдерҙар.
- `OrchestratorConfig::with_telemetry_region("region")` тегтар
  `sorafs_orchestrator_*` метрикаһы, шул уҡ ваҡытта
  I18NI000000017X CLI көҙгөләй
  `--transport-policy` флагы. I18NI000000019X хәҙер суднолар тип атала.
  CLI/SDK өҫтө буйынса ғәҙәттәгесә; ҡулланыу I18NI0000000020X ғына
  ҡасан сәхнәләштереү түбән йәки үтәү директиваһын үтәү, һәм запас
  `SoranetStrict` өсөн PQ-тик осоусылар өсөн асыҡ раҫлау менән.
- SorafsGateawayFetchOptions ҡуйырға::яҙа_mode_hint = .
  Ҡайһы бер(Яҙма ModeHint::UploadPqOnly)` PQ-тик тейәүҙәрҙе көсләү өсөн; ярҙамсы буласаҡ
  автоматик рәүештә транспорт/анонимлыҡ сәйәсәтен пропагандалау, әгәр асыҡтан-асыҡ
  өҫтөнлөк иткән.
- I18NI000000022X ҡулланыу, ваҡытлыса транспортты нығытыу өсөн
  йәки бер үтенес өсөн анонимлыҡ ярус; тәьмин итеү йәки ялан үткәрә
  браунут демократ һәм уңышһыҙлыҡҡа осрай, ҡасан һораған ярус ҡәнәғәтләндерергә мөмкин түгел.
- Питон (`sorafs_multi_fetch_local` / I18NI000000024X) һәм
  JavaScript (I18NI000000025X) бәйләүҙәре шул уҡ планлаштырыусыны ҡабаттан ҡуллана, шулай уҡ
  18NI000000026X был ярҙамсыларҙа иҫәпләнгән ауырлыҡтарҙы алыу өсөн ҡуйылған
  өлөшө менән бер рәттән квитанциялар.
- I18NI000000027X OTLP-ны теркәй.
  ағымы, тип етештерә ҡабул итеү өйөмө. Ҡасан төшөрөп ҡалдырылған, клиент ала
  I18NI000000028X (йәки I18NI000000029X X) автоматик рәүештә шулай метамағлүмәт
  һәр ваҡыт тасуири ярлыҡ йөрөтә.

## I18NI000000030X аша Фетч

Rust SDK шлюз ярҙамсыһы өйөмдәре; манифест плюс тәьмин итеүсе тәьмин итеү
дескрипторҙар (шул иҫәптән ағым токендары) һәм клиентҡа күп сығанаҡлы драйвер рөхсәт итә.
алып килтерергә:

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

I18NI000000031X комплекты I18NI0000000032X-ҡа тиклем тейәгәндә
классик релеларҙан баш тартырға тейеш, йәки I18NI0000000333X ҡасан SoraNet
тулыһынса урап үтергә тейеш. Поинт I18NI0000000034X релизда
артефакт каталогы, теләк буйынса төҙәтеү I18NI00000000035X, һәм халыҡ
I18NI000000036X менән тотоу контексы (фикстура лейблдары, I18NT000000001X маҡсатлы һ.б.)
тимәк, I18NI000000037X SDKs буйынса детерминистик JSON ҡуллана
провенанс менән блоб SF-6c көтә.
I18NI0000000038X хәҙер манифест менән метамағлүмәттәрҙе арттыра
идентификатор, өҫтәмә манифест CID көтөү, һәм был
I18NI0000000039X флагы менән тәьмин ителгән
I18NI0000000040X, шуға күрә ҡул ҡуйылған манифест конверт ҡәнәғәтләндереүҙе үҙ эсенә ала
SF-6c дәлилдәр талабы был яландарҙы ҡул менән ҡабатламай.

## Манифест ярҙамсылары

I18NI000000041X канонлы ысул булып ҡала йыйыу I18NT00000000000000 .
программалы рәүештә:

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

Төҙөүсегә ҡайҙа ғына хеҙмәттәр генерациялау кәрәк, осоуҙа манифест; был
CLI детерминистик торбалар өсөн тәҡдим ителгән юл булып ҡала.