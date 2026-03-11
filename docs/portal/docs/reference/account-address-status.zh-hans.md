---
lang: zh-hans
direction: ltr
source: docs/portal/docs/reference/account-address-status.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b92bdfc323a4bc031ca7f2237f238d5d515f7238791a6ec9c50b55e361c85560
source_last_modified: "2026-01-28T17:11:30.639071+00:00"
translation_last_reviewed: 2026-02-07
id: account-address-status
title: Account address compliance
description: Summary of the ADDR-2 fixture workflow and how SDK teams stay in sync.
translator: machine-google-reviewed
---

规范的 ADDR-2 捆绑包 (`fixtures/account/address_vectors.json`) 捕获
I105（首选）、压缩（`sora`，第二好；半角/全角）、多重签名和负固定装置。
每个 SDK + Torii 表面都依赖于相同的 JSON，因此我们可以检测任何编解码器
在投入生产之前就发生了漂移。此页面反映了内部状态简介
（根存储库中的`docs/source/account_address_status.md`）所以门户
读者可以参考工作流程，而无需深入研究 mono-repo。

## 重新生成或验证包

```bash
# Refresh the canonical fixture (writes fixtures/account/address_vectors.json)
cargo xtask address-vectors --out fixtures/account/address_vectors.json

# Fail fast if the committed file is stale
cargo xtask address-vectors --verify
```

标志：

- `--stdout` — 将 JSON 发送到标准输出以进行临时检查。
- `--out <path>` — 写入不同的路径（例如，当本地差异更改时）。
- `--verify` — 将工作副本与新生成的内容进行比较（不能
  与 `--stdout` 组合）。

CI 工作流程**地址向量漂移**运行 `cargo xtask address-vectors --verify`
每当装置、生成器或文档发生变化时，都会立即提醒审阅者。

## 谁消耗了灯具？

|表面|验证 |
|---------|------------|
| Rust 数据模型 | `crates/iroha_data_model/tests/account_address_vectors.rs` |
| Torii（服务器）| `crates/iroha_torii/tests/account_address_vectors.rs` |
| JavaScript SDK | `javascript/iroha_js/test/address.test.js` |
|斯威夫特 SDK | `IrohaSwift/Tests/IrohaSwiftTests/AccountAddressTests.swift` |
|安卓SDK | `java/iroha_android/src/test/java/org/hyperledger/iroha/android/address/AccountAddressTests.java` |

每个线束往返规范字节 + I105 + 压缩（`sora`，第二好的）编码和
检查 Norito 类型的错误代码是否与负面情况下的夹具一致。

## 需要自动化吗？

发布工具可以使用助手来编写夹具刷新脚本
`scripts/account_fixture_helper.py`，获取或验证规范
捆绑无需复制/粘贴步骤：

```bash
# Download to a custom path (defaults to fixtures/account/address_vectors.json)
python3 scripts/account_fixture_helper.py fetch --output path/to/sdk/address_vectors.json

# Verify that a local copy matches the canonical source (HTTPS or file://)
python3 scripts/account_fixture_helper.py check --target path/to/sdk/address_vectors.json --quiet

# Emit Prometheus textfile metrics for dashboards/alerts
python3 scripts/account_fixture_helper.py check \
  --target path/to/sdk/address_vectors.json \
  --metrics-out /var/lib/node_exporter/textfile_collector/address_fixture.prom \
  --metrics-label android
```

帮助程序接受 `--source` 覆盖或 `IROHA_ACCOUNT_FIXTURE_URL`
环境变量，以便 SDK CI 作业可以指向其首选镜像。
当提供 `--metrics-out` 时，帮助程序写入
`account_address_fixture_check_status{target=\"…\"}` 以及规范
SHA-256 摘要 (`account_address_fixture_remote_info`) 所以 Prometheus 文本文件
收集器和 Grafana 仪表板 `account_address_fixture_status` 可以证明
每个表面都保持同步。每当目标报告 `0` 时发出警报。对于
多表面自动化使用包装器 `ci/account_fixture_metrics.sh`
（接受重复的 `--target label=path[::source]`）以便待命团队可以发布
一个用于节点导出器文本文件收集器的合并 `.prom` 文件。

## 需要完整的简介吗？

完整的 ADDR-2 合规状态（所有者、监控计划、未决行动项目）
位于存储库中的 `docs/source/account_address_status.md` 中
与地址结构 RFC (`docs/account_structure.md`)。使用此页面作为
快捷的操作提醒；请参阅回购文档以获取深入指导。