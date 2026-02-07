---
lang: zh-hans
direction: ltr
source: docs/portal/docs/soranet/gar-operator-onboarding.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 565d4e8bf0a043b2c83a03ec87a8c71a30da34f56d94a28cad03677963b3e69a
source_last_modified: "2025-12-29T18:16:35.206864+00:00"
translation_last_reviewed: 2026-02-07
title: GAR Operator Onboarding
sidebar_label: GAR Operator Onboarding
description: Checklist to activate SNNet-9 compliance policies with attestation digests and evidence capture.
translator: machine-google-reviewed
---

使用此简介来推出具有可重复的 SNNet-9 合规性配置，
审计友好的流程。将其与管辖权审查配对，以便每个运营商
使用相同的摘要和证据布局。

## 步骤

1. **组装配置**
   - 导入 `governance/compliance/soranet_opt_outs.json`。
   - 将您的 `operator_jurisdictions` 与已发布的证明摘要合并
     在[管辖权审查](gar-jurisdictional-review)中。
2. **验证**
   - `cargo test -p sorafs_orchestrator -- compliance_policy_parses_from_json`
   - `cargo test -p sorafs_orchestrator -- compliance_example_config_parses`
   - 可选：`cargo xtask soranet-privacy-report --max-suppression-ratio 0.2 --ndjson <privacy-log.ndjson>`
3. **获取证据**
   - 存储在 `artifacts/soranet/compliance/<YYYYMMDD>/` 下：
     - `config.json`（最终合规块）
     - `attestations.json`（URI + 摘要）
     - 验证日志
     - 签名 PDF/Norito 信封的参考
4. **激活**
   - 标记推出 (`gar-opt-out-<date>`)，重新部署 Orchestrator/SDK 配置，
     并确认 `compliance_*` 事件在日志中按预期发出。
5. **平仓**
   - 向管理委员会提交证据包。
   - 在 GAR 日志中记录激活窗口 + 批准者。
   - 从管辖权审查表中安排下次审查日期。

## 快速清单

- [ ] `jurisdiction_opt_outs` 与规范目录匹配。
- [ ] 准确复制证明摘要。
- [ ] 验证命令运行并存档。
- [ ] 证据包存储在 `artifacts/soranet/compliance/<date>/` 中。
- [ ] 推出标签 + GAR 日志已更新。
- [ ] 下次回顾提醒设置。

## 另请参阅

- [GAR 管辖权审查](gar-jurisdictional-review)
- [GAR 合规手册（来源）](../../../source/soranet/gar_compliance_playbook.md)