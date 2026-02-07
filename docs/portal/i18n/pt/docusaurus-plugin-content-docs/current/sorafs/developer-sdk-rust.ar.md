---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/developer-sdk-rust.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: desenvolvedor-sdk-rust
título: Usando Rust SDK
sidebar_label: Rust
description: Rust دنيا لاستهلاك تدفقات الأدلة والمانيفستات.
---

:::note المصدر المعتمد
Verifique o valor `docs/source/sorafs/developer/sdk/rust.md`. احرص على إبقاء النسختين متزامنتين إلى أن يتم إيقاف مجموعة Sphinx القديمة.
:::

Use Rust no site e na CLI para que você possa usá-lo e usá-lo.
Você pode usar o software para obter mais informações.

## مساعد تدفق الأدلة

Para obter mais informações sobre o HTTP:

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

Verifique o valor do arquivo (sem nome) em `docs/examples/sorafs_rust_proof_stream.rs`.
Use `ProofStreamSummary::to_json()` como JSON para o CLI, mas não seja necessário
منصات الرصد, أو فرضيات CI.

## تقييم الجلب متعدد المصادر

Use o método `sorafs_car::multi_fetch` para obter mais informações no CLI.
`sorafs_car::multi_fetch::ScorePolicy` e `FetchOptions::score_policy`
لضبط ترتيب المزوّدين. يوضح اختبار الوحدة
`multi_fetch::tests::score_policy_can_filter_providers` é um produto que não funciona.

Clique em CLI:

- `FetchOptions::per_chunk_retry_limit` é usado para `--retry-budget` para CI.
  تحد عدد المحاولات عمدًا.
- Use `FetchOptions::global_parallel_limit` e `--max-peers` para obter mais informações
  المتزامنين.
- يقوم `OrchestratorConfig::with_telemetry_region("region")` بوسم مقاييس
  `sorafs_orchestrator_*`, بينما يعكس `OrchestratorConfig::with_transport_policy`
  Use CLI `--transport-policy`. تُشحن `TransportPolicy::SoranetPreferred` كافتراضي عبر
  Usando CLI/SDK; Atualizar `TransportPolicy::DirectOnly` para fazer downgrade ou fazer downgrade
  توجيه امتثال, واحجز `SoranetStrict` لطيارين Somente PQ بموافقة صريحة.
- Use `SorafsGatewayFetchOptions::write_mode_hint =
  Some(WriteModeHint::UploadPqOnly)` é usado para PQ-only; سيعزز المساعد سياسات
  A máquina/ferramenta deve ser usada para remover o excesso de água.
- استخدم `SorafsGatewayFetchOptions::policy_override` لتثبيت طبقة نقل أو إخفاء هوية
  مؤقتة لطلب واحد؛ تمرير أي حقل يتجاوز تخفيض brownout ويفشل عندما يتعذر تلبية الطبقة
  المطلوبة.
- Desenvolvimento de Python (`sorafs_multi_fetch_local` / `sorafs_gateway_fetch`) e JavaScript
  (`sorafsMultiFetchLocal`).
  لاسترجاع الأوزان المحسوبة مع إيصالات الـ chunk.
- `SorafsGatewayScoreboardOptions::telemetry_source_label` é compatível com OTLP
  حزمة تبنٍ. Você pode usar o `region:<telemetry_region>` (ou `chain:<chain_id>`) para obter mais informações
  Certifique-se de que está tudo bem e sem problemas.

## Nome de usuário `iroha::Client`

Usando o Rust SDK para usar o Rust SDK مرر مانيفست مع أوصاف المزوّدين
(بما في ذلك رموز البث) ودع العميل يدير الجلب متعدد المصادر:

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

Use `transport_policy` ou `Some(TransportPolicy::SoranetStrict)` para obter mais informações
O código de barras do cartão e `Some(TransportPolicy::DirectOnly)` é um problema de segurança
SoraNet é gratuito. وجّه `scoreboard.persist_path` إلى دليل آرتيفاكتات الإصدار, واضبط
`scoreboard.now_unix_secs` e `scoreboard.metadata`
(Equipamentos, Torii, também) `cargo xtask sorafs-adoption-check` JSON
Você pode usar SDKs do SF-6c.
Use `Client::sorafs_fetch_via_gateway` para obter mais informações sobre o produto
O código CID do código postal e o `gateway_manifest_provided` estão disponíveis.
`GatewayFetchConfig` `GatewayFetchConfig` `GatewayFetchConfig` `GatewayFetchConfig`
O SF-6c é um dispositivo que não funciona.

## مساعدات المانيفست

A configuração do `ManifestBuilder` é a seguinte:```rust
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

ضمّن المنشئ حيثما احتاجت الخدمات إلى توليد المانيفستات ديناميكيًا؛ Use CLI no site
Isso é algo que você pode fazer.