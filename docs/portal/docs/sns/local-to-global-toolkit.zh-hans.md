---
lang: zh-hans
direction: ltr
source: docs/portal/docs/sns/local-to-global-toolkit.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d4493e69ce57c4f691f368fb13c1bbe96e2c73991dfb39045753b5652d2f10a9
source_last_modified: "2026-01-28T17:11:30.702818+00:00"
translation_last_reviewed: 2026-02-07
title: Local → Global Address Toolkit
translator: machine-google-reviewed
---

此页面镜像 [`docs/source/sns/local_to_global_toolkit.md`](../../../source/sns/local_to_global_toolkit.md)
来自单一仓库。它打包了路线图项 **ADDR-5c** 所需的 CLI 帮助程序和 Runbook。

## 概述

- `scripts/address_local_toolkit.sh` 包装 `iroha` CLI 以生成：
  - `audit.json` — `iroha tools address audit --format json` 的结构化输出。
  - `normalized.txt` — 每个本地域选择器的已转换首选 i105/第二佳压缩 (`sora`) 文字。
- 将脚本与地址提取仪表板配对 (`dashboards/grafana/address_ingest.json`)
  和Alertmanager规则（`dashboards/alerts/address_ingest_rules.yml`）来证明Local-8 /
  Local-12 切换是安全的。观看 Local-8 和 Local-12 碰撞面板以及
  `AddressLocal8Resurgence`、`AddressLocal12Collision` 和 `AddressInvalidRatioSlo` 之前的警报
  促进明显的变化。
- 参考[地址显示指南](address-display-guidelines.md) 和
  [地址清单操作手册](../../../source/runbooks/address_manifest_ops.md)，用于用户体验和事件响应上下文。

## 用法

```bash
scripts/address_local_toolkit.sh \
  --input fixtures/address/local_digest_examples.txt \
  --output-dir artifacts/address_migration \
  --network-prefix 753 \
  --format i105
```

选项：

- `--format i105` 用于 `i105` 输出而不是 i105。
- `domainless output (default)` 发出裸文字。
- `--audit-only` 跳过转换步骤。
- `--allow-errors` 在出现格式错误的行时继续扫描（与 CLI 行为匹配）。

该脚本在运行结束时写入工件路径。将两个文件附加到
您的变更管理票以及证明为零的 Grafana 屏幕截图
≥30 天的 Local-8 检测和零 Local-12 冲突。

## CI 集成

1. 在专用作业中运行脚本并上传其输出。
2. 当 `audit.json` 报告本地选择器 (`domain.kind = local12`) 时阻止合并。
   以其默认的 `true` 值（仅在开发/测试集群上覆盖 `false` 时）
   诊断回归）并添加
   `iroha tools address normalize` 到 CI 所以回归
   在投入生产之前尝试失败。

请参阅源文档以了解更多详细信息、示例证据清单以及在向客户宣布切换时可以重复使用的发行说明片段。