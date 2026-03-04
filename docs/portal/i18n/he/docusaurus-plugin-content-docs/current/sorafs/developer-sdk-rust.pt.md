---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/developer-sdk-rust.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: developer-sdk-rust
כותרת: Snippets de SDK Rust
sidebar_label: Snippets de Rust
תיאור: דוגמאות Rust minimos para consumir הוכחה זרמים e manifests.
---

:::שים לב Fonte canonica
Esta pagina espelha `docs/source/sorafs/developer/sdk/rust.md`. Mantenha ambas as copias sincronizadas.
:::

Os ארגזים Rust neste repositorio alimentam o CLI e podem ser embutidos em
orquestradores ou servicos customizados. Os snippets abaixo destacam os helpers que
mais aparecem nas solicitacoes dos desenvolvedores.

## זרם עוזר הוכחה

השתמש מחדש ב-Perser de proof stream existente ל-Agregar Metricas de Uma Resposta HTTP:

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

A versao completa (com testes) vive em `docs/examples/sorafs_rust_proof_stream.rs`.
`ProofStreamSummary::to_json()` renderiza o mesmo JSON de metricas do CLI, facilitando
alimentar backends de observabilidade ou assertions de CI.

## ניקוד אחזר ריבוי מקורות

O modulo `sorafs_car::multi_fetch` אקספו או מתזמן להביא assincrono ארה"ב
CLI. יישם את `sorafs_car::multi_fetch::ScorePolicy` ועבור
`FetchOptions::score_policy` para ajustar a ordenacao de provedores. O teste unitario
`multi_fetch::tests::score_policy_can_filter_providers` כמו גם העדפות חשובות
התאמה אישית.

Outros מכוון את דגלי espelham ל-CLI:

- `FetchOptions::per_chunk_retry_limit` corresponde ao flag `--retry-budget` para runs
  de CI que restringem tentativas propositalmente.
- Combine `FetchOptions::global_parallel_limit` com `--max-peers` למספר מוגבל או מספריים
  de provedores concorrentes.
- `OrchestratorConfig::with_telemetry_region("region")` סמן כמדדים
  `sorafs_orchestrator_*`, enquanto `OrchestratorConfig::with_transport_policy` espelha
  o דגל CLI `--transport-policy`. `TransportPolicy::SoranetPreferred` או ברירת מחדל
  superficies CLI/SDK; השתמש ב-`TransportPolicy::DirectOnly` אפנים או הכנה לשדרוג לאחור
  או להגדיר אומה הנחיות לציות, e Reserve `SoranetStrict` עבור פיילוט PQ בלבד
  com aprovacao explicita.
- Defina `SorafsGatewayFetchOptions::write_mode_hint =
  Some(WriteModeHint::UploadPqOnly)` para forcar העלאות PQ-only; o עוזר לקדם
  automaticamente as politicas de transporte/anonymity a menos que sejam explicitamente
  sobrescritas.
- השתמש ב-`SorafsGatewayFetchOptions::policy_override` para fixar um tier temporario de
  transporte ou anonimato para um unico request; fornecer qualquer um dos campos pula
  o הורדה בראייה e falha quando o tier solicitado nao pode ser atendido.
- כריכות מערכת הפעלה Python (`sorafs_multi_fetch_local` / `sorafs_gateway_fetch`) e
  JavaScript (`sorafsMultiFetchLocal`) מחדש או מתזמן mesmo, entao defina
  `return_scoreboard=true` nesses helpers para recuperar os pesos calculados junto com
  חתיכות קבלות.
- `SorafsGatewayScoreboardOptions::telemetry_source_label` רישום או זרם OTLP que
  חבילת אימוץ. Quando omitido, o cliente deriva automaticamente
  `region:<telemetry_region>` (ou `chain:<chain_id>`) עבור מטא נתונים סמפר
  carregue um rotulo descritivo.

## אחזור דרך `iroha::Client`

O SDK Rust כולל עוזר לשער אחזור; forneca um manifest mais decritores de
הוכחות (כולל אסימוני זרם) או שירות לקוחות או אחזור מקורות מרובים:

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
```Defina `transport_policy` como `Some(TransportPolicy::SoranetStrict)` quando העלאות
precisarem recusar relays classicos, ou `Some(TransportPolicy::DirectOnly)` quando
SoraNet סרגל מעקף מוחלט. Aponte `scoreboard.persist_path` para o
מדריך לשחרור אמנות, תיקון אופציונלי `scoreboard.now_unix_secs` e preencha
`scoreboard.metadata` com contexto de captura (תוויות מתקנים, alvo Torii וכו')
para que `cargo xtask sorafs-adoption-check` צרכני SDK של JSON מוגדרים
com o blob de provenance que SF-6c espera.
`Client::sorafs_fetch_via_gateway` agora aumenta esse metadata com o identificador de
מניפסט, ציפייה אופציונלית למניפסט CID e o flag `gateway_manifest_provided`
inspecionando o `GatewayFetchConfig` fornecido, para que capturas que incluem um
מעטפת מניפסט assinado atendam ao requisito de evidencia SF-6c sem duplicar esses
campos manualmente.

## עוזרי המניפסט

`ManifestBuilder` ממשיך לשלוח פורמה קנוניקה דה מונטאר מטענים Norito דה פורמה
פרומטיקה:

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

Incorpore o Builder semper que servicos precisarem gerar manifests em tempo real; o
CLI segue sendo o caminho recomendado para pipelines deterministicos.