---
lang: zh-hans
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

:::注意规范来源
:::

此存储库中的 Rust 箱为 CLI 提供支持，并且可以嵌入其中
自定义协调器或服务。下面的片段最突出显示了帮助者
开发商要求。

## 证明流助手

重用现有的证明流解析器来聚合来自 HTTP 的指标
回应：

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

完整版本（带测试）位于 `docs/examples/sorafs_rust_proof_stream.rs` 中。
`ProofStreamSummary::to_json()` 呈现与 CLI 相同的指标 JSON，使得
很容易提供可观察性后端或 CI 断言。

## 多源抓取评分

`sorafs_car::multi_fetch` 模块公开异步获取调度程序
由 CLI 使用。实现`sorafs_car::multi_fetch::ScorePolicy`并通过
通过 `FetchOptions::score_policy` 调整提供商排序。单元测试
`multi_fetch::tests::score_policy_can_filter_providers` 显示如何强制执行
自定义偏好。

其他旋钮镜像 CLI 标志：

- `FetchOptions::per_chunk_retry_limit` 与 CI 的 `--retry-budget` 标志匹配
  有意限制重试的运行。
- 将 `FetchOptions::global_parallel_limit` 与 `--max-peers` 组合以限制
  并发提供者的数量。
- `OrchestratorConfig::with_telemetry_region("region")` 标记
  `sorafs_orchestrator_*` 指标，同时
  `OrchestratorConfig::with_transport_policy` 镜像 CLI
  `--transport-policy` 标志。 `TransportPolicy::SoranetPreferred` 现在发货为
  CLI/SDK 界面的默认值；仅使用 `TransportPolicy::DirectOnly`
  当进行降级或遵循合规指令时，并保留
  `SoranetStrict` 适用于经过明确批准的仅 PQ 飞行员。
- 设置 `SorafsGatewayFetchOptions::write_mode_hint =
  Some(WriteModeHint::UploadPqOnly)` 强制仅 PQ 上传；帮助者将
  自动促进传输/匿名政策，除非明确
  被覆盖。
- 使用 `SorafsGatewayFetchOptions::policy_override` 固定临时传输
  或单个请求的匿名层；提供任一字段都会跳过
  当无法满足请求的等级时，掉电降级并失败。
- Python (`sorafs_multi_fetch_local` / `sorafs_gateway_fetch`) 和
  JavaScript (`sorafsMultiFetchLocal`) 绑定重用相同的调度程序，因此
  在这些助手中设置 `return_scoreboard=true` 以检索计算出的权重
  与大块收据一起。
- `SorafsGatewayScoreboardOptions::telemetry_source_label` 记录 OTLP
  产生采用包的流。当省略时，客户端得出
  `region:<telemetry_region>`（或 `chain:<chain_id>`）自动生成元数据
  始终带有描述性标签。

## 通过 `iroha::Client` 获取

Rust SDK 捆绑了网关获取助手；提供清单加提供商
描述符（包括流令牌）并让客户端驱动多源
获取：

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

上传时将 `transport_policy` 设置为 `Some(TransportPolicy::SoranetStrict)`
SoraNet 时必须拒绝经典中继，或者 `Some(TransportPolicy::DirectOnly)`
必须完全绕过。发布时点 `scoreboard.persist_path`
artefact 目录，可选择修复 `scoreboard.now_unix_secs`，并填充
`scoreboard.metadata` 具有捕获上下文（夹具标签、Torii 目标等）
因此 `cargo xtask sorafs-adoption-check` 跨 SDK 使用确定性 JSON
与 SF-6c 所期望的出处斑点相同。
`Client::sorafs_fetch_via_gateway` 现在通过清单增强了元数据
标识符、可选的清单 CID 期望以及
通过检查提供的 `gateway_manifest_provided` 标志
`GatewayFetchConfig`，因此包含签名清单信封的捕获满足
SF-6c 证据要求，无需手动复制这些字段。

## 清单助手

`ManifestBuilder` 仍然是组装 Norito 有效负载的规范方法
以编程方式：

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

在服务需要动态生成清单的任何地方嵌入构建器；的
CLI 仍然是确定性管道的推荐路径。