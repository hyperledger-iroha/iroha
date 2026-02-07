---
lang: kk
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

:::ескерту Канондық дереккөз
:::

Осы репозиторийдегі Rust жәшіктері CLI-ге қуат береді және оны ішіне енгізуге болады
реттелетін оркестрлер немесе қызметтер. Төмендегі үзінділер көмекшілерді көбірек көрсетеді
әзірлеушілер сұрайды.

## Дәлелдеу ағынының көмекшісі

HTTP деректерін біріктіру үшін бар дәлелдеу ағынының талдаушысын қайта пайдаланыңыз
жауап:

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

Толық нұсқасы (тесттермен) `docs/examples/sorafs_rust_proof_stream.rs` ішінде тұрады.
`ProofStreamSummary::to_json()` JSON көрсеткіштерін CLI сияқты бірдей етіп көрсетеді.
бақылау мүмкіндігін немесе CI бекітулерін беру оңай.

## Көп дереккөзді алу ұпайлары

`sorafs_car::multi_fetch` модулі асинхронды алу жоспарлаушысын көрсетеді
CLI пайдаланады. `sorafs_car::multi_fetch::ScorePolicy` іске қосыңыз және оны өткізіңіз
провайдердің тапсырысын реттеу үшін `FetchOptions::score_policy` арқылы. Бірлік сынағы
`multi_fetch::tests::score_policy_can_filter_providers` орындау жолын көрсетеді
теңшелетін теңшелімдер.

Басқа түймелер CLI жалауларын көрсетеді:

- `FetchOptions::per_chunk_retry_limit` CI үшін `--retry-budget` жалауына сәйкес келеді
  қайталау әрекеттерін әдейі шектейтін іске қосылады.
- жабу үшін `FetchOptions::global_parallel_limit` мен `--max-peers` біріктіріңіз.
  қатарлас провайдерлердің саны.
- `OrchestratorConfig::with_telemetry_region("region")` белгілейді
  `sorafs_orchestrator_*` метрикасы, ал
  `OrchestratorConfig::with_transport_policy` CLI көрсетеді
  `--transport-policy` жалауы. `TransportPolicy::SoranetPreferred` енді келесідей жеткізіледі
  CLI/SDK беттеріндегі әдепкі; тек `TransportPolicy::DirectOnly` пайдаланыңыз
  төмендетілгенде немесе сәйкестік директивасын орындағанда және резервте қалдырыңыз
  `SoranetStrict` нақты мақұлдауымен тек PQ ұшқыштарына арналған.
- `SorafsGatewayFetchOptions::write_mode_hint = орнату
  Кейбір(WriteModeHint::UploadPqOnly)` тек PQ жүктеп салуларды мәжбүрлеу үшін; көмекші болады
  егер анық болмаса, көлік/анонимділік саясатын автоматты түрде алға жылжытыңыз
  ауыстырылды.
- Уақытша тасымалдауды бекіту үшін `SorafsGatewayFetchOptions::policy_override` пайдаланыңыз
  немесе бір сұрау үшін анонимділік деңгейі; кез келген өрісті беру параметрін өткізіп жібереді
  төмендетеді және сұралған деңгей қанағаттандырылмаған кезде орындалмайды.
- Python (`sorafs_multi_fetch_local` / `sorafs_gateway_fetch`) және
  JavaScript (`sorafsMultiFetchLocal`) байланыстырулары бірдей жоспарлаушыны қайта пайдаланады, сондықтан
  есептелген салмақтарды алу үшін сол көмекшілерге `return_scoreboard=true` орнатыңыз
  кесінді түбіртектермен қатар.
- `SorafsGatewayScoreboardOptions::telemetry_source_label` OTLP жазады
  қабылдау бумасын шығаратын ағын. Өткізіп тастаған кезде, клиент шығарады
  `region:<telemetry_region>` (немесе `chain:<chain_id>`) автоматты түрде метадеректер
  әрқашан сипаттама белгісі бар.

## `iroha::Client` арқылы алу

Rust SDK шлюзді алу көмекшісін жинақтайды; манифест плюс провайдерін қамтамасыз етіңіз
дескрипторлар (ағынды таңбалауыштарды қоса) және клиентке көп дереккөзді басқаруға мүмкіндік беріңіз
алу:

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

Жүктеп салу кезінде `transport_policy` мәнін `Some(TransportPolicy::SoranetStrict)` етіп орнатыңыз
классикалық реледен бас тартуы керек немесе SoraNet болғанда `Some(TransportPolicy::DirectOnly)`
толығымен айналып өту керек. Шығарылымдағы `scoreboard.persist_path` нүктесі
артефакт каталогында, қосымша түрде `scoreboard.now_unix_secs` түзетіңіз және толтырыңыз
Түсіру мәтінмәні бар `scoreboard.metadata` (бекіту белгілері, Torii нысанасы, т.б.)
сондықтан `cargo xtask sorafs-adoption-check` SDK арқылы детерминирленген JSON пайдаланады
шығу тегін SF-6c күтеді.
`Client::sorafs_fetch_via_gateway` енді сол метадеректерді манифестпен толықтырады
идентификатор, қосымша манифест CID күту және
Берілгенді тексеру арқылы `gateway_manifest_provided` жалаушасын қойыңыз
`GatewayFetchConfig`, сондықтан қол қойылған манифест конверті бар түсірулер қанағаттандырады
сол өрістерді қолмен қайталамай, SF-6c дәлелдеу талабы.

## Манифест көмекшілері

`ManifestBuilder` Norito пайдалы жүктемелерін жинаудың канондық әдісі болып қала береді
бағдарламалық түрде:

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

Қызметтер манифесттерді жылдам жасау үшін қажет жерде құрастырушыны ендіріңіз; the
CLI детерминирленген конвейерлер үшін ұсынылған жол болып қалады.