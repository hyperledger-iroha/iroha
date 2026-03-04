---
lang: zh-hans
direction: ltr
source: docs/automation/da/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8757f0bf8699b532ece29437af953353526b3201b4b129ebec7d6bf5d224f038
source_last_modified: "2025-12-29T18:16:35.061402+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# 数据可用性威胁模型自动化 (DA-1)

路线图项目 DA-1 和 `status.md` 要求确定性自动化循环
产生 Norito PDP/PoTR 威胁模型摘要
`docs/source/da/threat_model.md` 和 Docusaurus 镜像。这个目录
捕获以下所引用的文物：

- `cargo xtask da-threat-model-report [--out <path|->] [--seed <u64|0xhex>] [--config <path>]`
- `.github/workflows/da-threat-model-nightly.yml`
- `make docs-da-threat-model`（运行 `scripts/docs/render_da_threat_model_tables.py`）
- `cargo xtask da-commitment-reconcile --receipt <path> --block <path> [--json-out <path|->]`
- `cargo xtask da-privilege-audit --config <torii.toml> [--extra-path <path> ...] [--json-out <path|->]`

## 流程

1. **生成报告**
   ```bash
   cargo xtask da-threat-model-report \
     --config configs/da/threat_model.toml \
     --out artifacts/da/threat_model_report.json
   ```
   JSON摘要记录了模拟的复制失败率，chunker
   阈值以及 PDP/PoTR 工具检测到的任何策略违规
   `integration_tests/src/da/pdp_potr.rs`。
2. **渲染 Markdown 表格**
   ```bash
   make docs-da-threat-model
   ```
   这运行 `scripts/docs/render_da_threat_model_tables.py` 来重写
   `docs/source/da/threat_model.md` 和 `docs/portal/docs/da/threat-model.md`。
3. **通过将 JSON 报告（和可选的 CLI 日志）复制到
   `docs/automation/da/reports/<timestamp>-threat_model_report.json`。当
   治理决策依赖于特定的运行，包括 git 提交哈希和
   模拟器种子位于同级 `<timestamp>-metadata.md` 中。

## 证据预期

- JSON 文件应保持 <100 KiB，以便它们可以存在于 git 中。更大的执行力
  痕迹属于外部存储——在元数据中引用它们的签名哈希
  如果需要请注意。
- 每个存档文件必须列出种子、配置路径和模拟器版本，以便
  在审核 DA 发布门时可以准确地重现重播。
- 随时从 `status.md` 或路线图条目链接回存档文件
  DA-1 验收标准提前，确保评审员能够验证
  基线，无需重新运行线束。

## 承诺协调（排序器省略）

使用 `cargo xtask da-commitment-reconcile` 将 DA 摄取收据与
DA承诺记录，捕获定序器遗漏或篡改：

```bash
cargo xtask da-commitment-reconcile \
  --receipt artifacts/da/receipts/ \
  --block storage/blocks/ \
  --json-out artifacts/da/commitment_reconciliation.json
```

- 接受 Norito 或 JSON 形式的收据和承诺
  `SignedBlockWire`、`.norito` 或 JSON 捆绑包。
- 当块日志中缺少任何票据或散列出现分歧时失败；
  当您有意范围时，`--allow-unexpected` 会忽略仅块票证
  收据集。
- 将发出的 JSON 附加到治理数据包/Alertmanager 以进行省略
  警报；默认为 `artifacts/da/commitment_reconciliation.json`。

## 权限审核（季度访问审核）

使用 `cargo xtask da-privilege-audit` 扫描 DA 清单/重播目录
（加上可选的额外路径）用于丢失、非目录或全局可写
条目：

```bash
cargo xtask da-privilege-audit \
  --config configs/torii.dev.toml \
  --extra-path /var/lib/iroha/da-manifests \
  --json-out artifacts/da/privilege_audit.json
```

- 从提供的 Torii 配置中读取 DA 摄取路径并检查 Unix
  权限（如果有）。
- 标记缺失/非目录/全局可写路径并返回非零退出
  存在问题时的代码。
- 签署并附加 JSON 捆绑包 (`artifacts/da/privilege_audit.json` by
  默认）每季度访问审查数据包和仪表板。