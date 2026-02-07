---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/developer-sdk-rust.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: 開発者-sdk-rust
タイトル: Rust の SDK
サイドバーラベル: Rust の機能
説明: Rust の証明ストリームとマニフェスト。
---

:::note Канонический источник
:::

Rust クレートは、CLI および могут быть встроены в кастомные で動作します。
オーケストレーター、または сервисы。 Сниппеты ниже выделяют ヘルパー、которые чаще всего
нужны разработчикам。

## ヘルパーの証明ストリーム

証明ストリーム パーサーを使用して、テストを実行します。
HTTP 通信:

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

Полная версия (с тестами) находится в `docs/examples/sorafs_rust_proof_stream.rs`.
`ProofStreamSummary::to_json()` は、JSON メソッド、CLI、что упрощает をサポートします。
可観測性バックエンドと CI アサーションの両方を備えています。

## マルチソースフェッチのスコアリング

Модуль `sorafs_car::multi_fetch` フェッチ スケジューラ、説明
CLI。 Реализуйте `sorafs_car::multi_fetch::ScorePolicy` または передайте его через
`FetchOptions::score_policy`、чтобы настроить порядок провайдеров. Юнит-тест
`multi_fetch::tests::score_policy_can_filter_providers` 、 как вводить
кастомные предпочтения。

CLI でのノブの操作:

- `FetchOptions::per_chunk_retry_limit` 日付 `--retry-budget` CI
  再試行します。
- Скомбинируйте `FetchOptions::global_parallel_limit` с `--max-peers`、чтобы ограничить
  количество одновременных провайдеров。
- `OrchestratorConfig::with_telemetry_region("region")` です。
  `sorafs_orchestrator_*`、`OrchestratorConfig::with_transport_policy` とは
  CLI `--transport-policy`。 `TransportPolicy::SoranetPreferred` は、 умолчанию
  CLI/SDK を参照してください。ステージング `TransportPolicy::DirectOnly` только при
  ダウングレード、準拠指令、`SoranetStrict` 日
  PQ のみの機能です。
- `SorafsGatewayFetchOptions::write_mode_hint = の例
  一部の(WriteModeHint::UploadPqOnly)` чтобы форсировать PQ のみのアップロード。ヘルパー автоматически
  トランスポート/匿名性があり、явно がオーバーライドされます。
- Используйте `SorafsGatewayFetchOptions::policy_override` чтобы закрепить временный
  輸送と匿名性の層。必要に応じて、必要な情報を入力してください。
  電圧低下による降格と、階級の低下。
- Python (`sorafs_multi_fetch_local` / `sorafs_gateway_fetch`) および JavaScript
  (`sorafsMultiFetchLocal`) バインディング используют тот же スケジューラ、поэтому задайте
  `return_scoreboard=true` этих ヘルパー、чтобы получить рассчитанные веса рядом с
  塊の領収書。
- `SorafsGatewayScoreboardOptions::telemetry_source_label` OTLP ストリーム、
  採用バンドル。 Если он не задан, клиент автоматически выводит
  `region:<telemetry_region>` (`chain:<chain_id>`) メタデータの詳細
  описательный レーベル。

## フェッチ `iroha::Client`

Rust SDK ヘルパーとゲートウェイフェッチ。マニフェストと дескрипторы провайдеров
(ストリーム トークン) マルチソース フェッチを実行します。

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

`transport_policy` と `Some(TransportPolicy::SoranetStrict)`、ビデオのアップロード
должны отвергать классические リレー、или в `Some(TransportPolicy::DirectOnly)`、когда
SoraNet нужно полностью обходить。 Укажите `scoreboard.persist_path` на директорию
релизных артефактов, опционально зафиксируйте `scoreboard.now_unix_secs` и заполните
`scoreboard.metadata` контекстом захвата (ラベルフィクスチャ、цель Torii и т.д.) так、
чтобы `cargo xtask sorafs-adoption-check` は、JSON メソッド SDK を提供します
ブロブの出所、SF-6c です。
`Client::sorafs_fetch_via_gateway` メタデータ マニフェスト、
опциональным ожиданием マニフェスト CID と флагом `gateway_manifest_provided` путем анализа
переданного `GatewayFetchConfig`、чтобы захваты с подписанным 封筒マニフェスト удовлетворяли
SF-6c が 10 月にリリースされました。

## ヘルパーのマニフェスト

`ManifestBuilder` остается каноничным способом программно собирать Norito ペイロード:

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

ビルダーは、マニフェストをマニフェストします。 CLI
パイプラインを維持します。