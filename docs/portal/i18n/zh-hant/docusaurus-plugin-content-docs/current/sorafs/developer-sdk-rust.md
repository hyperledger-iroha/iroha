---
id: developer-sdk-rust
lang: zh-hant
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

:::注意規範來源
:::

此存儲庫中的 Rust 箱為 CLI 提供支持，並且可以嵌入其中
自定義協調器或服務。下面的片段最突出顯示了幫助者
開發商要求。

## 證明流助手

重用現有的證明流解析器來聚合來自 HTTP 的指標
回應：

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

完整版本（帶測試）位於 `docs/examples/sorafs_rust_proof_stream.rs` 中。
`ProofStreamSummary::to_json()` 呈現與 CLI 相同的指標 JSON，使得
很容易提供可觀察性後端或 CI 斷言。

## 多源抓取評分

`sorafs_car::multi_fetch` 模塊公開異步獲取調度程序
由 CLI 使用。實現`sorafs_car::multi_fetch::ScorePolicy`並通過
通過 `FetchOptions::score_policy` 調整提供商排序。單元測試
`multi_fetch::tests::score_policy_can_filter_providers` 顯示如何強制執行
自定義偏好。

其他旋鈕鏡像 CLI 標誌：

- `FetchOptions::per_chunk_retry_limit` 與 CI 的 `--retry-budget` 標誌匹配
  有意限制重試的運行。
- 將 `FetchOptions::global_parallel_limit` 與 `--max-peers` 組合以限制
  並發提供者的數量。
- `OrchestratorConfig::with_telemetry_region("region")` 標記
  `sorafs_orchestrator_*` 指標，同時
  `OrchestratorConfig::with_transport_policy` 鏡像 CLI
  `--transport-policy` 標誌。 `TransportPolicy::SoranetPreferred` 現在發貨為
  CLI/SDK 界面的默認值；僅使用 `TransportPolicy::DirectOnly`
  當進行降級或遵循合規指令時，並保留
  `SoranetStrict` 適用於經過明確批准的僅 PQ 飛行員。
- 設置 `SorafsGatewayFetchOptions::write_mode_hint =
  Some(WriteModeHint::UploadPqOnly)` 強制僅 PQ 上傳；幫助者將
  自動促進傳輸/匿名政策，除非明確
  被覆蓋。
- 使用 `SorafsGatewayFetchOptions::policy_override` 固定臨時傳輸
  或單個請求的匿名層；提供任一字段都會跳過
  當無法滿足請求的等級時，掉電降級並失敗。
- Python (`sorafs_multi_fetch_local` / `sorafs_gateway_fetch`) 和
  JavaScript (`sorafsMultiFetchLocal`) 綁定重用相同的調度程序，因此
  在這些助手中設置 `return_scoreboard=true` 以檢索計算出的權重
  與大塊收據一起。
- `SorafsGatewayScoreboardOptions::telemetry_source_label` 記錄 OTLP
  產生採用包的流。當省略時，客戶端得出
  `region:<telemetry_region>`（或 `chain:<chain_id>`）自動生成元數據
  始終帶有描述性標籤。

## 通過 `iroha::Client` 獲取

Rust SDK 捆綁了網關獲取助手；提供清單加提供商
描述符（包括流令牌）並讓客戶端驅動多源
獲取：

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

上傳時將 `transport_policy` 設置為 `Some(TransportPolicy::SoranetStrict)`
SoraNet 時必須拒絕經典中繼，或者 `Some(TransportPolicy::DirectOnly)`
必須完全繞過。發佈時點 `scoreboard.persist_path`
artefact 目錄，可選擇修復 `scoreboard.now_unix_secs`，並填充
`scoreboard.metadata` 具有捕獲上下文（夾具標籤、Torii 目標等）
因此 `cargo xtask sorafs-adoption-check` 跨 SDK 使用確定性 JSON
與 SF-6c 所期望的出處斑點相同。
`Client::sorafs_fetch_via_gateway` 現在通過清單增強了元數據
標識符、可選的清單 CID 期望以及
通過檢查提供的 `gateway_manifest_provided` 標誌
`GatewayFetchConfig`，因此包含簽名清單信封的捕獲滿足
SF-6c 證據要求，無需手動複製這些字段。

## 清單助手

`ManifestBuilder` 仍然是組裝 Norito 有效負載的規範方法
以編程方式：

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

在服務需要動態生成清單的任何地方嵌入構建器；的
CLI 仍然是確定性管道的推薦路徑。