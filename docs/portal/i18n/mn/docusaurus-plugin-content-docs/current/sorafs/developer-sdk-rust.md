---
id: developer-sdk-rust
lang: mn
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

::: Каноник эх сурвалжийг анхаарна уу
:::

Энэхүү агуулах дахь Rust хайрцаг нь CLI-г идэвхжүүлдэг бөгөөд дотор нь суулгаж болно
захиалгат найрал хөгжимчид эсвэл үйлчилгээ. Доорх хэсгүүдэд туслагчдыг хамгийн их онцолсон
хөгжүүлэгчид асууж байна.

## Баталгаажуулах дамжуулагч

HTTP-с хэмжигдэхүүнийг нэгтгэхийн тулд одоо байгаа баталгааны урсгал задлагчийг дахин ашиглана уу
хариу:

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

Бүрэн хувилбар (туршилтын хамт) нь `docs/examples/sorafs_rust_proof_stream.rs` дээр амьдардаг.
`ProofStreamSummary::to_json()` нь CLI-тай ижил хэмжигдэхүүн JSON-г гаргаж өгдөг.
Энэ нь ажиглалтын арын хэсэг эсвэл CI батламжийг өгөхөд хялбар байдаг.

## Олон эх сурвалжаас татах оноо

`sorafs_car::multi_fetch` модуль нь асинхрон татан авалтын хуваарийг харуулж байна.
CLI ашигладаг. `sorafs_car::multi_fetch::ScorePolicy`-г хэрэгжүүлээд дамжуулаарай
үйлчилгээ үзүүлэгчийн захиалгыг тааруулахын тулд `FetchOptions::score_policy`-ээр дамжуулан. Нэгжийн туршилт
`multi_fetch::tests::score_policy_can_filter_providers` хэрхэн хэрэгжүүлэхийг харуулж байна
захиалгат тохиргоо.

Бусад товчлуурууд нь CLI тугуудыг тусгадаг:

- `FetchOptions::per_chunk_retry_limit` нь CI-ийн `--retry-budget` тугтай таарч байна
  дахин оролдохыг зориудаар хязгаарладаг гүйлтүүд.
- `FetchOptions::global_parallel_limit`-ийг `--max-peers`-тэй нэгтгэхийн тулд
  зэрэгцээ үйлчилгээ үзүүлэгчдийн тоо.
- `OrchestratorConfig::with_telemetry_region("region")` хаягууд
  `sorafs_orchestrator_*` хэмжүүр, харин
  `OrchestratorConfig::with_transport_policy` нь CLI-г тусгадаг
  `--transport-policy` туг. `TransportPolicy::SoranetPreferred` одоо зарна
  CLI/SDK гадаргуу дээрх анхдагч; зөвхөн `TransportPolicy::DirectOnly` ашиглана уу
  зэрэглэлийг бууруулах эсвэл дагаж мөрдөх удирдамжийг дагаж мөрдөх үед нөөц
  `SoranetStrict` нь зөвхөн PQ-д зориулагдсан нисгэгчдэд зориулагдсан бөгөөд тодорхой зөвшөөрөлтэй.
- `SorafsGatewayFetchOptions::write_mode_hint = тохируулах
  Зарим(WriteModeHint::UploadPqOnly)` зөвхөн PQ-д байршуулахыг албадах; туслах болно
  тодорхой заагаагүй бол тээврийн/нэрээ нууцлах бодлогыг автоматаар сурталчлах
  дарагдсан.
- `SorafsGatewayFetchOptions::policy_override`-г ашиглан түр зуурын тээвэрлэлтийг тогтооно
  эсвэл нэг хүсэлтийн нэрээ нууцлах түвшин; нийлүүлэх аль нэг талбар алгасаж байна
  Хүссэн түвшнийг хангах боломжгүй үед амжилтгүй болно.
- Python (`sorafs_multi_fetch_local` / `sorafs_gateway_fetch`) ба
  JavaScript (`sorafsMultiFetchLocal`) холболтууд нь ижил хуваарилагчийг дахин ашигладаг тул
  Тооцоолсон жинг авахын тулд тэдгээр туслахуудад `return_scoreboard=true`-г тохируулна уу.
  хэсэгчилсэн баримтын хамт.
- `SorafsGatewayScoreboardOptions::telemetry_source_label` нь OTLP-г бичдэг
  үрчлэлтийн багц үүсгэсэн урсгал. Орхигдсон тохиолдолд үйлчлүүлэгч үүснэ
  `region:<telemetry_region>` (эсвэл `chain:<chain_id>`) автоматаар мета өгөгдөл
  үргэлж дүрсэлсэн шошготой байдаг.

## `iroha::Client`-ээр татаж авах

Rust SDK нь гарц татах туслахыг багцалсан; manifest plus үйлчилгээ үзүүлэгчээр хангах
тодорхойлогч (стримт токенуудыг оруулаад) ба үйлчлүүлэгчид олон эх сурвалжийг жолоодох боломжийг олгоно
авах:

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

Байршуулахдаа `transport_policy`-г `Some(TransportPolicy::SoranetStrict)` болгож тохируулна уу
сонгодог реле, эсвэл SoraNet үед `Some(TransportPolicy::DirectOnly)` татгалзах ёстой
бүрэн тойрч гарах ёстой. Гаргах үед `scoreboard.persist_path` цэг
олдворын лавлах, сонголтоор `scoreboard.now_unix_secs` засч, бөглөнө үү
Зураг авах контексттэй `scoreboard.metadata` (бэхэлгээний шошго, Torii зорилт гэх мэт)
тиймээс `cargo xtask sorafs-adoption-check` нь SDK дээр тодорхойлогч JSON ашигладаг
SF-6c-ийн хүлээгдэж буй гарал үүсэлтэй.
`Client::sorafs_fetch_via_gateway` одоо тэр мета өгөгдлийг манифестээр нэмэгдүүлнэ
танигч, нэмэлт манифест CID хүлээлт болон
Нийлүүлсэн барааг шалгаж `gateway_manifest_provided` дарцаглана
`GatewayFetchConfig`, тиймээс гарын үсэг зурсан манифест дугтуйг агуулсан зураг авалтад нийцнэ
Эдгээр талбаруудыг гараар хуулбарлахгүйгээр SF-6c нотлох баримтын шаардлага.

## Илэрхий туслагч

`ManifestBuilder` нь Norito ачааллыг цуглуулах канон арга хэвээр байна
программын хувьд:

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

Үйлчилгээнүүд шууд манифест үүсгэх шаардлагатай газарт бүтээгчийг оруулах; нь
CLI нь детерминистик дамжуулах хоолойн санал болгож буй зам хэвээр байна.