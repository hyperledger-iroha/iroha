---
lang: zh-hans
direction: ltr
source: docs/portal/docs/sorafs/quickstart.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 79a048e6061f7054e14a471004cf7da0dddd3f9bf627d9f1d20ff63803cb0979
source_last_modified: "2026-01-05T09:28:11.908615+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SoraFS 快速入门

本实践指南介绍了确定性 SF-1 分块器配置文件，
支持 SoraFS 的清单签名和多提供商获取流程
存储管道。将其与[清单管道深入研究](manifest-pipeline.md)配对
获取设计说明和 CLI 标志参考材料。

## 先决条件

- Rust 工具链 (`rustup update`)，本地克隆的工作区。
- 可选：[OpenSSL 生成的 Ed25519 密钥对](https://github.com/hyperledger-iroha/iroha/tree/master/defaults/dev-keys#readme)
  用于签署清单。
- 可选：如果您计划预览 Docusaurus 门户，则 Node.js ≥ 18。

设置 `export RUST_LOG=info`，同时尝试显示有用的 CLI 消息。

## 1. 刷新确定性装置

重新生成规范的 SF-1 分块向量。该命令还发出有符号的
提供 `--signing-key` 时的舱单信封；使用 `--allow-unsigned`
仅在本地开发期间。

```bash
cargo run -p sorafs_chunker --bin export_vectors -- --allow-unsigned
```

输出：

- `fixtures/sorafs_chunker/sf1_profile_v1.{json,rs,ts,go}`
- `fixtures/sorafs_chunker/manifest_blake3.json`
- `fixtures/sorafs_chunker/manifest_signatures.json`（如果签名）
- `fuzz/sorafs_chunker/sf1_profile_v1_{input,backpressure}.json`

## 2. 对有效负载进行分块并检查计划

使用 `sorafs_chunker` 对任意文件或存档进行分块：

```bash
echo "SoraFS deterministic chunking" > /tmp/docs.txt
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- /tmp/docs.txt \
  > /tmp/docs.chunk-plan.json
```

关键领域：

- `profile` / `break_mask` – 确认 `sorafs.sf1@1.0.0` 参数。
- `chunks[]` – 有序偏移、长度和块 BLAKE3 摘要。

对于较大的装置，运行 proptest 支持的回归以确保流式传输和
批量分块保持同步：

```bash
cargo test -p sorafs_chunker streaming_backpressure_fuzz_matches_batch
```

## 3. 构建并签署清单

使用以下命令将块计划、别名和治理签名包装到清单中
`sorafs-manifest-stub`。下面的命令展示了一个单文件有效负载；通过
打包树的目录路径（CLI 按字典顺序遍历）。

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub -- \
  /tmp/docs.txt \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --manifest-out=/tmp/docs.manifest \
  --manifest-signatures-out=/tmp/docs.manifest_signatures.json \
  --json-out=/tmp/docs.report.json \
  --allow-unsigned
```

查看 `/tmp/docs.report.json`：

- `chunking.chunk_digest_sha3_256` – 偏移量/长度的 SHA3 摘要，与
  分块装置。
- `manifest.manifest_blake3` – 在清单信封中签名的 BLAKE3 摘要。
- `chunk_fetch_specs[]` – 协调器的有序获取指令。

准备好提供真实签名时，添加 `--signing-key` 和 `--signer`
论据。该命令在写入之前验证每个 Ed25519 签名
信封。

## 4. 模拟多提供商检索

使用开发人员获取 CLI 针对一个或多个重放块计划
提供商。这非常适合 CI 冒烟测试和协调器原型设计。

```bash
cargo run -p sorafs_car --bin sorafs_fetch -- \
  --plan=/tmp/docs.report.json \
  --provider=primary=/tmp/docs.txt \
  --output=/tmp/docs.reassembled \
  --json-out=/tmp/docs.fetch-report.json
```

断言：

- `payload_digest_hex` 必须与舱单报告匹配。
- `provider_reports[]` 显示每个提供商的成功/失败计数。
- 非零 `chunk_retry_total` 突出显示背压调整。
- 通过 `--max-peers=<n>` 来限制计划运行的提供者数量
  并使 CI 模拟集中于主要候选人。
- `--retry-budget=<n>` 覆盖默认的每块重试计数 (3)，因此您
  在注入故障时可以更快地显示协调器回归。

添加 `--expect-payload-digest=<hex>` 和 `--expect-payload-len=<bytes>` 失败
当重建的有效负载偏离清单时速度很快。

## 5. 后续步骤

- **治理集成** – 通过管道传输清单摘要和
  `manifest_signatures.json` 进入理事会工作流程，以便 Pin 注册表可以
  广告可用性。
- **注册协商** – 咨询 [`sorafs/chunker_registry.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/chunker_registry.md)
  在注册新的配置文件之前。自动化应该更喜欢规范句柄
  (`namespace.name@semver`) 通过数字 ID。
- **CI 自动化** – 添加上面的命令来发布管道，所以文档，
  固定装置和工件在签名的同时发布确定性清单
  元数据。