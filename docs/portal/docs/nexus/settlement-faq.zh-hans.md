---
lang: zh-hans
direction: ltr
source: docs/portal/docs/nexus/settlement-faq.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e992cea8d0c835b30bd9e91860f6b6f87bed79a2c25bd6d0544639685834f80c
source_last_modified: "2025-12-29T18:16:35.146583+00:00"
translation_last_reviewed: 2026-02-07
id: nexus-settlement-faq
title: Settlement FAQ
description: Operator-facing answers covering settlement routing, XOR conversion, telemetry, and audit evidence.
translator: machine-google-reviewed
---

此页面镜像内部结算常见问题解答 (`docs/source/nexus_settlement_faq.md`)
因此门户读者可以查看相同的指南，而无需深入研究
单一仓库。它解释了结算路由器如何处理支出、哪些指标
来监控，以及 SDK 应如何集成 Norito 有效负载。

## 亮点

1. **通道映射**——每个数据空间声明一个`settlement_handle`
   （`xor_global`、`xor_lane_weighted`、`xor_hosted_custody` 或
   `xor_dual_fund`）。请参阅下面的最新车道目录
   `docs/source/project_tracker/nexus_config_deltas/`。
2. **确定性转换** — 路由器通过 XOR 将所有结算转换为 XOR
   治理批准的流动性来源。专用通道为 XOR 缓冲区预先提供资金；
   仅当缓冲区偏离政策时才适用折扣。
3. **遥测** — 手表 `nexus_settlement_latency_seconds`、转换计数器、
   和理发仪。仪表板位于 `dashboards/grafana/nexus_settlement.json`
   以及 `dashboards/alerts/nexus_audit_rules.yml` 中的警报。
4. **证据** — 存档配置、路由器日志、遥测导出和
   审计的调节报告。
5. **SDK 责任** — 每个 SDK 必须公开结算助手、车道 ID、
   和 Norito 有效负载编码器，以与路由器保持奇偶校验。

## 流程示例

|车道类型|需要捕捉的证据|它证明了什么 |
|----------|--------------------------------|----------------|
|私人 `xor_hosted_custody` |路由器日志 + `nexus_settlement_latency_seconds{lane}` + `settlement_router_haircut_total{lane}` | CBDC 缓冲借方确定性 XOR 和削减保持在政策范围内。 |
|公共 `xor_global` |路由器日志 + DEX/TWAP 参考 + 延迟/转换指标 |共享流动性路径以公布的 TWAP 定价，且折价为零。 |
|混合 `xor_dual_fund` |路由器日志显示公共与屏蔽分离 + 遥测计数器 |屏蔽/公共混合尊重治理比率并记录应用于每条腿的理发。 |

## 需要更多细节吗？

- 完整常见问题解答：`docs/source/nexus_settlement_faq.md`
- 结算路由器规格：`docs/source/settlement_router.md`
- CBDC 政策手册：`docs/source/cbdc_lane_playbook.md`
- 操作手册：[Nexus 操作](./nexus-operations)