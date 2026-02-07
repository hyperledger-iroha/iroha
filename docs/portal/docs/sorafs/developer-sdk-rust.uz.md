---
lang: uz
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

::: Eslatma Kanonik manba
:::

Ushbu ombordagi Rust qutilari CLI-ni quvvatlaydi va ichiga o'rnatilishi mumkin
maxsus orkestrlar yoki xizmatlar. Quyidagi parchalar ko'proq yordamchilarni ta'kidlaydi
ishlab chiquvchilar so'rashadi.

## Isbot oqimi yordamchisi

HTTP ko'rsatkichlarini jamlash uchun mavjud isbot oqimi tahlilchisidan qayta foydalaning
javob:

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

To'liq versiya (testlar bilan) `docs/examples/sorafs_rust_proof_stream.rs` da yashaydi.
`ProofStreamSummary::to_json()` JSON ko'rsatkichlarini CLI bilan bir xil qiladi, bu
kuzatuvchanlik orqa tomonlarini yoki CI tasdiqlarini berish oson.

## Ko'p manbadan olingan ball

`sorafs_car::multi_fetch` moduli asinxron qabul qilish rejalashtiruvchisini ochib beradi.
CLI tomonidan qo'llaniladi. `sorafs_car::multi_fetch::ScorePolicy` ni amalga oshiring va uni o'tkazing
provayder buyurtmasini sozlash uchun `FetchOptions::score_policy` orqali. Birlik testi
`multi_fetch::tests::score_policy_can_filter_providers` qanday majburlash kerakligini ko'rsatadi
shaxsiy imtiyozlar.

Boshqa tugmalar CLI bayroqlarini aks ettiradi:

- `FetchOptions::per_chunk_retry_limit` CI uchun `--retry-budget` bayrog'iga mos keladi
  qayta urinishlarni qasddan cheklaydigan ishlaydi.
- `FetchOptions::global_parallel_limit` ni `--max-peers` bilan birlashtiring
  bir vaqtning o'zida provayderlar soni.
- `OrchestratorConfig::with_telemetry_region("region")` teglar
  `sorafs_orchestrator_*` ko'rsatkichlari, esa
  `OrchestratorConfig::with_transport_policy` CLI ni aks ettiradi
  `--transport-policy` bayrog'i. `TransportPolicy::SoranetPreferred` endi sifatida yuboriladi
  CLI/SDK sirtlarida standart; faqat `TransportPolicy::DirectOnly` dan foydalaning
  pasaytirish bosqichida yoki muvofiqlik direktivasiga amal qilganda, va zaxira
  `SoranetStrict` faqat PQ-uchuvchilar uchun aniq ma'qullangan.
- `SorafsGatewayFetchOptions::write_mode_hint = o'rnating
  Ba'zi(WriteModeHint::UploadPqOnly)` faqat PQ uchun yuklashni majburlash uchun; yordamchi bo'ladi
  aniq aytilmasa, transport/anonimlik siyosatini avtomatik ravishda targ'ib qilish
  bekor qilingan.
- Vaqtinchalik transportni mahkamlash uchun `SorafsGatewayFetchOptions::policy_override` dan foydalaning
  yoki bitta so'rov uchun anonimlik darajasi; har bir maydonni etkazib berish o'tkazib yuboradi
  pasaytirish va so'ralgan darajani qondirib bo'lmaganda muvaffaqiyatsiz tugadi.
- Python (`sorafs_multi_fetch_local` / `sorafs_gateway_fetch`) va
  JavaScript (`sorafsMultiFetchLocal`) ulanishlari bir xil rejalashtiruvchini qayta ishlatadi, shuning uchun
  Hisoblangan og'irliklarni olish uchun ushbu yordamchilarda `return_scoreboard=true` ni o'rnating
  parcha kvitansiyalari bilan bir qatorda.
- `SorafsGatewayScoreboardOptions::telemetry_source_label` OTLPni qayd qiladi
  qabul qilish paketini yaratgan oqim. O'tkazib yuborilganda, mijoz olinadi
  `region:<telemetry_region>` (yoki `chain:<chain_id>`) avtomatik ravishda metadata
  har doim tavsiflovchi belgini olib yuradi.

## `iroha::Client` orqali oling

Rust SDK shlyuzni olish yordamchisini birlashtiradi; manifest plyus provayderini taqdim eting
deskriptorlar (shu jumladan oqim tokenlari) va mijozga ko'p manbani boshqarishga ruxsat bering
olish:

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

Yuklanganda `transport_policy` ni `Some(TransportPolicy::SoranetStrict)` qilib belgilang
klassik o'rni rad qilish kerak, yoki SoraNet qachon `Some(TransportPolicy::DirectOnly)`
butunlay chetlab o'tish kerak. Chiqarish paytida `scoreboard.persist_path` nuqtasi
artefakt katalogi, ixtiyoriy ravishda `scoreboard.now_unix_secs`ni tuzating va to'ldiring
Yozib olish kontekstiga ega `scoreboard.metadata` (fiksator yorliqlari, Torii nishoni va boshqalar)
shuning uchun `cargo xtask sorafs-adoption-check` SDKlar bo'ylab deterministik JSONni iste'mol qiladi
SF-6c kutayotgan kelib chiqishi blob bilan.
`Client::sorafs_fetch_via_gateway` endi bu metamaʼlumotlarni manifest bilan kengaytiradi
identifikator, ixtiyoriy manifest CID kutilishi va
Ta'minlanganlarni tekshirish orqali `gateway_manifest_provided` bayrog'i
`GatewayFetchConfig`, shuning uchun imzolangan manifest konvertini o'z ichiga olgan suratlar qoniqarli
SF-6c dalil talabi, bu maydonlarni qo'lda takrorlamasdan.

## Aniq yordamchilar

`ManifestBuilder` Norito foydali yuklarni yig'ishning kanonik usuli bo'lib qolmoqda
dasturiy jihatdan:

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

Xizmatlar tez orada manifestlarni yaratishi kerak bo'lgan joyda quruvchini joylashtiring; the
CLI deterministik quvurlar uchun tavsiya etilgan yo'l bo'lib qolmoqda.