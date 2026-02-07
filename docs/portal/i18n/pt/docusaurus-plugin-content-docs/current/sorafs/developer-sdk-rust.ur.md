---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/developer-sdk-rust.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: desenvolvedor-sdk-rust
título: trechos do Rust SDK
sidebar_label: trechos de ferrugem
description: Fluxos de prova e manifestos consomem کرنے کے لیے exemplos mínimos de Rust۔
---

:::nota مستند ماخذ
:::

اس repositório کے Rust crates CLI کو power کرتے ہیں اور orquestradores personalizados یا serviços میں incorporar کیے جا سکتے ہیں۔
نیچے والے snippets ان helpers کو destaque کرتے ہیں جن کی طلب سب سے زیادہ ہوتی ہے۔

## Auxiliar de fluxo de prova

Resposta HTTP سے métricas agregadas کرنے کے لیے reutilização do analisador de fluxo de prova existente کریں:

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

Versão completa (testes سمیت) `docs/examples/sorafs_rust_proof_stream.rs` میں ہے۔
`ProofStreamSummary::to_json()` e métricas JSON render کرتا ہے جو CLI دیتا ہے، جس سے
backends de observabilidade e feed de asserções de CI

## Pontuação de busca de várias fontes

Módulo `sorafs_car::multi_fetch` e exposição do agendador de busca assíncrona کرتا ہے جو CLI استعمال کرتا ہے۔
`sorafs_car::multi_fetch::ScorePolicy` implementar کریں اور `FetchOptions::score_policy` کے ذریعے passar کریں
تاکہ melodia de pedido do provedor ہو سکے۔ Teste de unidade `multi_fetch::tests::score_policy_can_filter_providers`
preferências personalizadas impõem کرنے کا طریقہ دکھاتا ہے۔

Outros botões sinalizadores CLI کو espelho کرتے ہیں:

- `FetchOptions::per_chunk_retry_limit` CI executa کے لیے `--retry-budget` flag سے match کرتا ہے
  جو novas tentativas کو جان بوجھ کر restringir کرتے ہیں۔
- `FetchOptions::global_parallel_limit` کو `--max-peers` کے ساتھ combinar کریں تاکہ simultâneo
  provedores کی تعداد limite ہو۔
- `OrchestratorConfig::with_telemetry_region("region")` `sorafs_orchestrator_*` métricas کو tag کرتا ہے،
  جبکہ `OrchestratorConfig::with_transport_policy` CLI کے `--transport-policy` flag کو espelho کرتا ہے۔
  `TransportPolicy::SoranetPreferred` Superfícies CLI/SDK پر enviado por padrão ہے؛ `TransportPolicy::DirectOnly`
  صرف estágio de downgrade کرنے یا diretiva de conformidade siga کرنے پر استعمال کریں, اور `SoranetStrict` کو
  Pilotos somente PQ کے لیے aprovação explícita کے ساتھ reserva کریں۔
- `SorafsGatewayFetchOptions::write_mode_hint =
  Some(WriteModeHint::UploadPqOnly)` set کریں تاکہ Força de upload somente PQ ہوں؛ transporte de ajudante/anonimato
  políticas کو خودکار طور پر promover کرے گا جب تک واضح substituir نہ ہو۔
- `SorafsGatewayFetchOptions::policy_override` استعمال کریں تاکہ ایک solicitar کے لیے transporte temporário
  یا pin de nível de anonimato ہو جائے؛ کسی بھی campo کے دینے سے brownout rebaixamento pular ہوتا ہے اور اگر
  nível solicitado satisfeito نہ ہو سکے تو falha آتا ہے۔
- Python (`sorafs_multi_fetch_local` / `sorafs_gateway_fetch`) e JavaScript (`sorafsMultiFetchLocal`)
  ligações اسی agendador کو reutilização کرتے ہیں, اس لیے ان ajudantes میں `return_scoreboard=true` set کریں تاکہ
  recibos de pedaços de pesos computados کے ساتھ مل سکیں۔
- Fluxo OTLP `SorafsGatewayScoreboardOptions::telemetry_source_label` کو registro کرتا ہے جس نے pacote de adoção
  بنایا تھا۔ اگر omitir ہو تو cliente خودکار طور پر `region:<telemetry_region>` (یا `chain:<chain_id>`) derivar کرتا ہے
  تاکہ metadados میں ہمیشہ rótulo descritivo رہے۔

## Buscar via `iroha::Client`

Rust SDK میں gateway fetch helper شامل ہے؛ manifesto کے ساتھ descritores de provedor (stream tokens سمیت) فراہم کریں
O cliente e a unidade de busca de múltiplas fontes são os seguintes:

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
```جب uploads کو relés clássicos سے انکار کرنا ہو تو `transport_policy` کو `Some(TransportPolicy::SoranetStrict)`
پر set کریں, یا جب SoraNet کو مکمل bypass کرنا ہو تو `Some(TransportPolicy::DirectOnly)` پر۔
`scoreboard.persist_path` é o diretório de artefato de liberação پر ponto کریں, ضرورت ہو تو `scoreboard.now_unix_secs` correção کریں،
اور `scoreboard.metadata` میں contexto de captura (rótulos de luminária, alvo Torii, وغیرہ) شامل کریں تاکہ
`cargo xtask sorafs-adoption-check` SDKs são determinísticos JSON consumir e SF-6c e blob de proveniência
incluir رہے۔ `Client::sorafs_fetch_via_gateway` contém metadados e identificador de manifesto, expectativa de CID de manifesto opcional,
O sinalizador `gateway_manifest_provided` pode ser aumentado por meio do `GatewayFetchConfig` fornecido, envelope de manifesto assinado
شامل کرنے والی captura o requisito de evidência SF-6c پورا کریں بغیر ان campos کو دستی طور پر duplicado کیے۔

## Ajudantes de manifesto

`ManifestBuilder` é responsável por cargas úteis Norito montadas programaticamente کرنے کا canônico طریقہ ہے:

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

جہاں serviços کو manifestos on-the-fly geram کرنے ہوں وہاں construtor incorporar کریں؛ pipelines determinísticos کے لیے CLI ابھی بھی caminho recomendado ہے۔