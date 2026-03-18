---
lang: zh-hans
direction: ltr
source: docs/source/examples/sorafs_manifest/cli_end_to_end.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a8209e602132efb6c29962bf09aea8cd74f972fa956ea8a7a1dbac08a7f6f00f
source_last_modified: "2026-01-05T09:28:12.006380+00:00"
translation_last_reviewed: 2026-02-07
title: "SoraFS Manifest CLI End-to-End Example"
translator: machine-google-reviewed
---

# SoraFS 清单 CLI 端到端示例

此示例演示如何使用以下命令将文档版本发布到 SoraFS
`sorafs_manifest_stub` CLI 与确定性分块装置一起
SoraFS 架构 RFC 中进行了描述。该流程涵盖了明显的生成，
期望检查、获取计划验证和检索证明演练
团队可以在 CI 中嵌入相同的步骤。

## 先决条件

- 克隆工作区并准备好工具链（`cargo`、`rustc`）。
- `fixtures/sorafs_chunker` 中的夹具可用，因此期望值可以
  派生（对于生产运行，从迁移分类帐条目中提取值
  与工件相关）。
- 要发布的示例有效负载目录（本示例使用 `docs/book`）。

## 步骤 1 — 生成清单、CAR、签名和获取计划

```bash
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- docs/book \
  --manifest-out target/sorafs/docs.manifest \
  --manifest-signatures-out target/sorafs/docs.manifest_signatures.json \
  --car-out target/sorafs/docs.car \
  --chunk-fetch-plan-out target/sorafs/docs.fetch_plan.json \
  --car-digest=13fa919c67e55a2e95a13ff8b0c6b40b2e51d6ef505568990f3bc7754e6cc482 \
  --car-size=429391872 \
  --root-cid=f40101d0cfa9be459f4a4ba4da51990b75aef262ef546270db0e42d37728755d \
  --dag-codec=0x71 \
  --chunker-profile=sorafs.sf1@1.0.0
```

命令：

- 通过 `ChunkProfile::DEFAULT` 传输有效负载。
- 发出 CARv2 存档以及块获取计划。
- 构建 `ManifestV1` 记录，验证清单签名（如果提供），以及
  信封上写道。
- 强制执行期望标志，因此如果字节漂移则运行失败。

## 步骤 2 — 使用块存储 + PoR 演练验证输出

```bash
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
  target/sorafs/docs.car \
  --manifest target/sorafs/docs.manifest \
  --report-out target/sorafs/docs.manifest_report.json \
  --por-json-out target/sorafs/docs.por.json
```

这通过确定性块存储重放 CAR，得出
可检索性证明采样树，并发出适合的清单报告
治理审查。

## 步骤 3 — 模拟多提供商检索

```bash
cargo run -p sorafs_car --bin sorafs_fetch -- \
  --plan=target/sorafs/docs.fetch_plan.json \
  --provider=primary=target/sorafs/docs.car \
  --chunk-receipts-out=target/sorafs/docs.chunk_receipts.json \
  --json-out=target/sorafs/docs.fetch_report.json
```

对于 CI 环境，为每个提供程序提供单独的有效负载路径（例如，安装
固定装置）来练习范围调度和故障处理。

## 步骤 4 — 记录账本条目

在 `docs/source/sorafs/migration_ledger.md` 中记录发布，捕获：

- 清单 CID、CAR 摘要和理事会签名哈希。
- 状态（`Draft`、`Staging`、`Pinned`）。
- CI 运行或治理票证的链接。

## 步骤 5 — 通过治理工具固定（当注册中心上线时）

部署 Pin 注册表后（迁移路线图中的 Milestone M2），
通过 CLI 提交清单：

```bash
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- docs/book \
  --plan=target/sorafs/docs.fetch_plan.json \
  --manifest-out target/sorafs/docs.manifest \
  --manifest-signatures-in target/sorafs/docs.manifest_signatures.json \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --council-signature-file <signer_hex>:path/to/signature.bin

cargo run -p sorafs_cli --bin sorafs_pin -- propose \
  --manifest target/sorafs/docs.manifest \
  --manifest-signatures target/sorafs/docs.manifest_signatures.json
```

提案标识符和后续批准交易哈希值应该是
在迁移分类帐条目中捕获以进行审计。

## 清理

`target/sorafs/` 下的工件可以存档或上传到暂存节点。
将清单、签名、CAR 和获取计划放在一起，以便下游
运营商和 SDK 团队可以确定性地验证部署。