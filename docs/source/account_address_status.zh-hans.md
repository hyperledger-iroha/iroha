---
lang: zh-hans
direction: ltr
source: docs/source/account_address_status.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7c2cbb5e965350648a30607bbd0f1588212ee0021b412ec55654993c18cc198e
source_last_modified: "2026-01-28T17:11:30.739162+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

## 账户地址合规状态 (ADDR-2)

状态：已接受 2026-03-30  
所有者：数据模型团队/QA Guild  
路线图参考：ADDR-2 — 双格式合规套件

### 1. 概述

- 夹具：`fixtures/account/address_vectors.json`（IH58（首选）+压缩（`sora`，第二好）+多重签名正/负案例）。
- 范围：确定性 V1 有效负载，涵盖隐式默认、Local-12、全局注册表和具有完整错误分类的多重签名控制器。
- 分发：在 Rust 数据模型、Torii、JS/TS、Swift 和 Android SDK 之间共享；如果任何消费者偏离，CI就会失败。
- 事实来源：生成器位于 `crates/iroha_data_model/src/account/address/compliance_vectors.rs` 中，并通过 `cargo xtask address-vectors` 公开。
### 2. 再生与验证

```bash
# Write/update the canonical fixture
cargo xtask address-vectors --out fixtures/account/address_vectors.json

# Verify the committed fixture matches the generator
cargo xtask address-vectors --verify
```

标志：

- `--out <path>` — 生成临时捆绑包时可选覆盖（默认为 `fixtures/account/address_vectors.json`）。
- `--stdout` — 将 JSON 发送到标准输出而不是写入磁盘。
- `--verify` — 将当前文件与新生成的内容进行比较（漂移时快速失败；不能与 `--stdout` 一起使用）。

### 3. 人工制品矩阵

|表面|执法|笔记|
|--------|-------------|--------|
| Rust 数据模型 | `crates/iroha_data_model/tests/account_address_vectors.rs` |解析 JSON，重建规范有效负载，并检查 IH58（首选）/压缩（`sora`，第二好）/规范转换 + 结构化错误。 |
| Torii | `crates/iroha_torii/tests/account_address_vectors.rs` |验证服务器端编解码器，以便 Torii 确定性地拒绝格式错误的 IH58（首选）/压缩（`sora`，第二好的）有效负载。 |
| JavaScript SDK | `javascript/iroha_js/test/address.test.js` |镜像 V1 灯具（IH58 首选/压缩 (`sora`) 第二佳/全角）并为每个负面情况断言 Norito 样式错误代码。 |
|斯威夫特 SDK | `IrohaSwift/Tests/IrohaSwiftTests/AccountAddressTests.swift` |练习 Apple 平台上的 IH58（首选）/压缩（`sora`，第二佳）解码、多重签名有效负载和错误显示。 |
|安卓SDK | `java/iroha_android/src/test/java/org/hyperledger/iroha/android/address/AccountAddressTests.java` |确保 Kotlin/Java 绑定与规范固定装置保持一致。 |

### 4. 监控和杰出工作- 状态报告：此文档与 `status.md` 和路线图链接，因此每周审查可以验证灯具的运行状况。
- 开发者门户摘要：请参阅文档门户 (`docs/portal/docs/reference/account-address-status.md`) 中的**参考 → 帐户地址合规性**，了解面向外部的概要。
- Prometheus 和仪表板：每当您验证 SDK 副本时，请使用 `--metrics-out`（以及可选的 `--metrics-label`）运行帮助程序，以便 Prometheus 文本文件收集器可以摄取 `account_address_fixture_check_status{target=…}`。 Grafana 仪表板 **帐户地址夹具状态** (`dashboards/grafana/account_address_fixture_status.json`) 呈现每个表面的通过/失败计数，并显示规范的 SHA-256 摘要以获取审计证据。当任何目标报告 `0` 时发出警报。
- Torii 指标：`torii_address_domain_total{endpoint,domain_kind}` 现在为每个成功解析的帐户文字发出，镜像 `torii_address_invalid_total`/`torii_address_local8_total`。对生产中的任何 `domain_kind="local12"` 流量发出警报，并将计数器镜像到 SRE `address_ingest` 仪表板，以便 Local-12 退休门拥有可审核的证据。
- 夹具助手：`scripts/account_fixture_helper.py` 下载或验证规范 JSON，以便 SDK 发布自动化可以获取/检查捆绑包，而无需手动复制/粘贴，同时可选择写入 Prometheus 指标。示例：

  ```bash
  # Write the latest fixture to a custom path (defaults to fixtures/account/address_vectors.json)
  python3 scripts/account_fixture_helper.py fetch --output path/to/sdk/address_vectors.json

  # Fail if an SDK copy drifts from the canonical remote (accepts file:// or HTTPS sources)
  python3 scripts/account_fixture_helper.py check --target path/to/sdk/address_vectors.json --quiet

  # Emit Prometheus textfile metrics for dashboards/alerts (writes remote/local digests as labels)
  python3 scripts/account_fixture_helper.py check \\
    --target path/to/sdk/address_vectors.json \\
    --metrics-out /var/lib/node_exporter/textfile_collector/address_fixture.prom \\
    --metrics-label android
  ```

  当目标匹配时，帮助程序会写入 `account_address_fixture_check_status{target="android"} 1`，以及公开 SHA-256 摘要的 `account_address_fixture_remote_info` / `account_address_fixture_local_info` 计量器。丢失文件报告 `account_address_fixture_local_missing`。
  自动化包装器：从 cron/CI 调用 `ci/account_fixture_metrics.sh` 以发出合并的文本文件（默认 `artifacts/account_fixture/address_fixture.prom`）。传递重复的 `--target label=path` 条目（可以选择为每个目标附加 `::https://mirror/...` 以覆盖源），以便 Prometheus 抓取覆盖每个 SDK/CLI 副本的一个文件。 GitHub 工作流程 `address-vectors-verify.yml` 已针对规范固定装置运行此帮助程序，并上传 `account-address-fixture-metrics` 工件以供 SRE 摄取。