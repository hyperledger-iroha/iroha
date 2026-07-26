---
lang: zh-hans
direction: ltr
source: docs/source/compliance/android/jp/fisc_controls_checklist.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2d8b4c90c94dddd8118fcb9c55f07c25000c6dab1f8d239570402023ab89e844
source_last_modified: "2025-12-29T18:16:35.928660+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# FISC 安全控制清单 — Android SDK

|领域 |价值|
|--------|--------|
|版本 | 0.1 (2026-02-12) |
|范围 |日本金融部署中使用的 Android SDK + 操作员工具 |
|业主 |合规与法律 (Daniel Park)，Android 项目负责人 |

## 控制矩阵

| FISC控制|实施细节|证据/参考文献|状态 |
|--------------|------------------------|------------------------|--------|
| **系统配置完整性** | `ClientConfig` 强制执行清单哈希、模式验证和只读运行时访问。配置重新加载失败会发出 Runbook 中记录的 `android.telemetry.config.reload` 事件。 | `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/ClientConfig.java`； `docs/source/android_runbook.md` §1–2。 | ✅ 已实施 |
| **访问控制和身份验证** | SDK 遵循 Torii TLS 策略和 `/v1/pipeline` 签名请求；操作员工作流程参考支持手册 §4–5，通过签名的 Norito 工件进行升级和覆盖门控。 | `docs/source/android_support_playbook.md`； `docs/source/sdk/android/telemetry_redaction.md`（覆盖工作流程）。 | ✅ 已实施 |
| **加密密钥管理** | StrongBox 首选的提供商、证明验证和设备矩阵覆盖可确保 KMS 合规性。证明工具输出在 `artifacts/android/attestation/` 下存档并在准备矩阵中进行跟踪。 | `docs/source/sdk/android/key_management.md`； `docs/source/sdk/android/readiness/android_strongbox_device_matrix.md`； `scripts/android_strongbox_attestation_ci.sh`。 | ✅ 已实施 |
| **记录、监控和保留** |遥测编辑策略对敏感数据进行哈希处理、对设备属性进行存储桶并强制保留（7/30/90/365 天窗口）。支持手册 §8 描述了仪表板阈值；覆盖 `telemetry_override_log.md` 中记录的内容。 | `docs/source/sdk/android/telemetry_redaction.md`； `docs/source/android_support_playbook.md`； `docs/source/sdk/android/telemetry_override_log.md`。 | ✅ 已实施 |
| **运营和变革管理** | GA 切换程序（支持 Playbook §7.2）加上 `status.md` 更新跟踪发布准备情况。通过 `docs/source/compliance/android/eu/sbom_attestation.md` 链接的发布证据（SBOM、Sigstore 捆绑包）。 | `docs/source/android_support_playbook.md`； `status.md`； `docs/source/compliance/android/eu/sbom_attestation.md`。 | ✅ 已实施 |
| **事件响应和报告** | Playbook 定义了严重性矩阵、SLA 响应窗口和合规性通知步骤；遥测覆盖+混乱排练确保飞行员面前的再现性。 | `docs/source/android_support_playbook.md` §§4–9； `docs/source/sdk/android/telemetry_chaos_checklist.md`。 | ✅ 已实施 |
| **数据驻留/本地化** |用于 JP 部署的遥测收集器在批准的东京地区运行； StrongBox 证明捆绑包存储在区域内并从合作伙伴票证中引用。本地化计划确保文档在测试版 (AND5) 之前提供日语版本。 | `docs/source/android_support_playbook.md` §9； `docs/source/sdk/android/developer_experience_plan.md` §5； `docs/source/sdk/android/readiness/android_strongbox_device_matrix.md`。 | 🈺 进行中（本地化正在进行中）|

## 审稿人注释

- 在受监管的合作伙伴加入之前验证 Galaxy S23/S24 的设备矩阵条目（请参阅准备文档行 `s23-strongbox-a`、`s24-strongbox-a`）。
- 确保 JP 部署中的遥测收集器强制执行 DPIA (`docs/source/compliance/android/eu/gdpr_dpia_summary.md`) 中定义的相同保留/覆盖逻辑。
- 一旦银行合作伙伴审查此清单，即可获取外部审计师的确认。