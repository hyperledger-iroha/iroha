---
lang: zh-hans
direction: ltr
source: docs/portal/docs/sorafs/manifest-pipeline.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e77b792e19fbfa8e1efeddd042adbe68a48287a582a1be76aa518af7830774e2
source_last_modified: "2026-01-05T09:28:11.879581+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SoraFS 分块 → 清单管道

快速入门的这个配套文件跟踪了将原始数据转化为数据的端到端管道
Norito 清单中的字节适合 SoraFS Pin 注册表。内容是
改编自[`docs/source/sorafs/manifest_pipeline.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/manifest_pipeline.md)；
请查阅该文档以获取规范规范和变更日志。

## 1. 确定性分块

SoraFS 使用 SF-1 (`sorafs.sf1@1.0.0`) 配置文件：受 FastCDC 启发的滚动
具有 64KiB 最小块大小、256KiB 目标、512KiB 最大大小和
`0x0000ffff` 破坏掩码。该个人资料注册于
`sorafs_manifest::chunker_registry`。

### Rust 助手

- `sorafs_car::CarBuildPlan::single_file` – 发出块偏移量、长度和
  BLAKE3 在准备 CAR 元数据时进行摘要。
- `sorafs_car::ChunkStore` – 流式传输有效负载，保留块元数据，以及
  派生 64KiB / 4KiB 可检索性证明 (PoR) 采样树。
- `sorafs_chunker::chunk_bytes_with_digests` – 两个 CLI 背后的库助手。

### CLI 工具

```bash
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- ./payload.bin \
  > chunk-plan.json
```

JSON 包含有序偏移量、长度和块摘要。坚持
在构建清单或协调器获取规范时进行规划。

### PoR 见证人

`ChunkStore` 暴露 `--por-proof=<chunk>:<segment>:<leaf>` 和
`--por-sample=<count>`，以便审核员可以请求确定性见证集。配对
这些带有 `--por-proof-out` 或 `--por-sample-out` 的标志来记录 JSON。

## 2. 包装清单

`ManifestBuilder` 将块元数据与治理附件相结合：

- 根 CID (dag-cbor) 和 CAR 承诺。
- 别名证明和提供商能力声明。
- 委员会签名和可选元数据（例如构建 ID）。

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub -- \
  ./payload.bin \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --manifest-out=payload.manifest \
  --manifest-signatures-out=payload.manifest_signatures.json \
  --json-out=payload.report.json
```

重要输出：

- `payload.manifest` – Norito 编码的清单字节。
- `payload.report.json` – 人类/自动化可读摘要，包括
  `chunk_fetch_specs`、`payload_digest_hex`、CAR 摘要和别名元数据。
- `payload.manifest_signatures.json` – 包含清单 BLAKE3 的信封
  摘要、块计划 SHA3 摘要和排序的 Ed25519 签名。

使用 `--manifest-signatures-in` 验证外部提供的信封
签署人之前将其写回，以及 `--chunker-profile-id` 或
`--chunker-profile=<handle>` 锁定注册表选择。

## 3. 发布并固定

1. **治理提交** – 提供清单摘要和签名
   将信封寄给理事会，以便允许使用密码。外部审计师应
   将块计划 SHA3 摘要与清单摘要一起存储。
2. **Pin 有效负载** – 上传引用的 CAR 存档（和可选的 CAR 索引）
   在 Pin 注册表的清单中。确保清单和 CAR 共享
   相同的根 CID。
3. **记录遥测** – 保留 JSON 报告、PoR 见证和任何获取
   发布工件中的指标。这些记录提供给操作员仪表板和
   帮助重现问题，而无需下载大量负载。

## 4. 多提供商获取模拟

`cargo run -p sorafs_car --bin sorafs_fetch -- --plan=payload.report.json \
  --provider=alpha=providers/alpha.bin --provider=beta=providers/beta.bin#4@3 \
  --output=payload.bin --json-out=fetch_report.json`

- `#<concurrency>` 增加了每个提供者的并行性（上面的 `#4`）。
- `@<weight>` 调整调度偏差；默认为 1。
- `--max-peers=<n>` 限制了计划运行的提供者数量
  发现产生的候选者数量超出了预期。
- `--expect-payload-digest` 和 `--expect-payload-len` 防范静音
  腐败。
- `--provider-advert=name=advert.to` 之前验证提供商的能力
  在模拟中使用它们。
- `--retry-budget=<n>` 覆盖每个块的重试计数（默认值：3），因此 CI
  在测试故障场景时可以更快地发现回归。

`fetch_report.json` 显示聚合指标 (`chunk_retry_total`,
`provider_failure_rate`等）适合CI断言和可观察性。

## 5. 注册表更新和治理

当提出新的分块配置文件时：

1. 在 `sorafs_manifest::chunker_registry_data` 中编写描述符。
2.更新`docs/source/sorafs/chunker_registry.md`及相关章程。
3. 重新生成装置 (`export_vectors`) 并捕获签名清单。
4. 提交带有治理签名的章程合规报告。

自动化应该更喜欢规范句柄（`namespace.name@semver`）并下降