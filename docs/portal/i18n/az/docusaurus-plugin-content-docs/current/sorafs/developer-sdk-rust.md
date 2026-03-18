---
id: developer-sdk-rust
lang: az
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

:::Qeyd Kanonik Mənbə
:::

Bu anbardakı Rust qutuları CLI-yə güc verir və içəriyə daxil edilə bilər
xüsusi orkestrlər və ya xidmətlər. Aşağıdakı fraqmentlər köməkçiləri ən çox vurğulayır
tərtibatçılar xahiş edirlər.

## Sübut axını köməkçisi

HTTP-dən ölçüləri toplamaq üçün mövcud sübut axını təhlilçisindən yenidən istifadə edin
cavab:

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

Tam versiya (testlərlə) `docs/examples/sorafs_rust_proof_stream.rs`-də yaşayır.
`ProofStreamSummary::to_json()`, CLI ilə eyni ölçüləri JSON göstərir,
müşahidə oluna bilən arxa uçları və ya CI təsdiqlərini qidalandırmaq asandır.

## Çox mənbəli əldə etmə hesabı

`sorafs_car::multi_fetch` modulu asinxron gətirmə planlaşdırıcısını ifşa edir
CLI tərəfindən istifadə olunur. `sorafs_car::multi_fetch::ScorePolicy` tətbiq edin və keçin
provayder sifarişini tənzimləmək üçün `FetchOptions::score_policy` vasitəsilə. Vahid sınağı
`multi_fetch::tests::score_policy_can_filter_providers` necə tətbiq olunacağını göstərir
fərdi üstünlüklər.

Digər düymələr CLI bayraqlarını əks etdirir:

- `FetchOptions::per_chunk_retry_limit` CI üçün `--retry-budget` bayrağına uyğun gəlir
  təkrar cəhdləri qəsdən məhdudlaşdıran qaçışlar.
- `FetchOptions::global_parallel_limit` ilə `--max-peers` birləşdirin
  paralel provayderlərin sayı.
- `OrchestratorConfig::with_telemetry_region("region")` etiketləri
  `sorafs_orchestrator_*` ölçüləri isə
  `OrchestratorConfig::with_transport_policy` CLI-ni əks etdirir
  `--transport-policy` bayrağı. `TransportPolicy::SoranetPreferred` indi olaraq göndərilir
  CLI/SDK səthlərində standart; yalnız `TransportPolicy::DirectOnly` istifadə edin
  səviyyəni endirərkən və ya uyğunluq direktivinə əməl edərkən və ehtiyatda saxlayın
  `SoranetStrict`, açıq təsdiqi olan yalnız PQ pilotları üçün.
- `SorafsGatewayFetchOptions::write_mode_hint = seçin
  Bəziləri(WriteModeHint::UploadPqOnly)` yalnız PQ üçün yükləmələri məcbur etmək; köməkçi olacaq
  aydın olmadığı təqdirdə nəqliyyat/anonimlik siyasətini avtomatik olaraq təşviq edin
  ləğv edildi.
- Müvəqqəti nəqliyyatı bağlamaq üçün `SorafsGatewayFetchOptions::policy_override` istifadə edin
  və ya bir sorğu üçün anonimlik səviyyəsi; hər hansı bir sahəni təmin edən sahəni atlayır
  aşağı düşür və tələb olunan səviyyə təmin edilmədikdə uğursuz olur.
- Python (`sorafs_multi_fetch_local` / `sorafs_gateway_fetch`) və
  JavaScript (`sorafsMultiFetchLocal`) bağlamaları eyni planlaşdırıcıdan təkrar istifadə edir, buna görə də
  hesablanmış çəkiləri əldə etmək üçün həmin köməkçilərdə `return_scoreboard=true` təyin edin
  yığın qəbzləri ilə yanaşı.
- `SorafsGatewayScoreboardOptions::telemetry_source_label` OTLP-ni qeyd edir
  qəbul paketi yaradan axın. Buraxıldıqda, müştəri əldə edir
  `region:<telemetry_region>` (və ya `chain:<chain_id>`) avtomatik olaraq metadata
  həmişə təsviri etiket daşıyır.

## `iroha::Client` vasitəsilə gətirin

Rust SDK şlüz gətirmə köməkçisini birləşdirir; manifest plus provayderi təmin edin
deskriptorları (axın işarələri daxil olmaqla) və müştəriyə çox mənbəni idarə etməsinə icazə verin
gətirmək:

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

Yükləmə zamanı `transport_policy`-i `Some(TransportPolicy::SoranetStrict)` olaraq təyin edin
klassik relelərdən imtina etməlidir və ya SoraNet zaman `Some(TransportPolicy::DirectOnly)`
tamamilə yan keçməlidir. Buraxılış zamanı `scoreboard.persist_path` nöqtəsi
artefakt kataloqu, isteğe bağlı olaraq `scoreboard.now_unix_secs`-i düzəldin və doldurun
Çəkmə konteksti ilə `scoreboard.metadata` (fiksator etiketləri, Torii hədəfi və s.)
beləliklə, `cargo xtask sorafs-adoption-check` SDK-lar arasında deterministik JSON istehlak edir
SF-6c gözlənilən mənşəli blob ilə.
`Client::sorafs_fetch_via_gateway` indi həmin metadatanı manifestlə artırır
identifikator, isteğe bağlı manifest CID gözləntisi və
Verilənləri yoxlayaraq `gateway_manifest_provided` bayrağı
`GatewayFetchConfig`, buna görə də imzalanmış manifest zərfini ehtiva edən çəkilişlər təmin edir
bu sahələri əl ilə təkrarlamadan SF-6c sübut tələbi.

## Aydın köməkçilər

`ManifestBuilder`, Norito faydalı yükləri yığmaq üçün kanonik üsul olaraq qalır.
proqramlı olaraq:

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

Xidmətlərin tez bir zamanda manifestlər yaratması lazım olduğu yerdə qurucu yerləşdirin; the
CLI deterministik boru kəmərləri üçün tövsiyə olunan yol olaraq qalır.