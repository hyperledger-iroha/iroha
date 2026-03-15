---
lang: zh-hant
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

# FISC 安全控制清單 — Android SDK

|領域 |價值|
|--------|--------|
|版本 | 0.1 (2026-02-12) |
|範圍 |日本金融部署中使用的 Android SDK + 操作員工具 |
|業主 |合規與法律 (Daniel Park)，Android 項目負責人 |

## 控制矩陣

| FISC控制|實施細節|證據/參考文獻|狀態 |
|--------------|------------------------|------------------------|--------|
| **系統配置完整性** | `ClientConfig` 強制執行清單哈希、模式驗證和只讀運行時訪問。配置重新加載失敗會發出 Runbook 中記錄的 `android.telemetry.config.reload` 事件。 | `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/ClientConfig.java`； `docs/source/android_runbook.md` §1–2。 | ✅ 已實施 |
| **訪問控制和身份驗證** | SDK 遵循 Torii TLS 策略和 `/v2/pipeline` 簽名請求；操作員工作流程參考支持手冊 §4–5，通過簽名的 Norito 工件進行升級和覆蓋門控。 | `docs/source/android_support_playbook.md`； `docs/source/sdk/android/telemetry_redaction.md`（覆蓋工作流程）。 | ✅ 已實施 |
| **加密密鑰管理** | StrongBox 首選的提供商、證明驗證和設備矩陣覆蓋可確保 KMS 合規性。證明工具輸出在 `artifacts/android/attestation/` 下存檔並在準備矩陣中進行跟踪。 | `docs/source/sdk/android/key_management.md`； `docs/source/sdk/android/readiness/android_strongbox_device_matrix.md`； `scripts/android_strongbox_attestation_ci.sh`。 | ✅ 已實施 |
| **記錄、監控和保留** |遙測編輯策略對敏感數據進行哈希處理、對設備屬性進行存儲桶並強制保留（7/30/90/365 天窗口）。支持手冊 §8 描述了儀表板閾值；覆蓋 `telemetry_override_log.md` 中記錄的內容。 | `docs/source/sdk/android/telemetry_redaction.md`； `docs/source/android_support_playbook.md`； `docs/source/sdk/android/telemetry_override_log.md`。 | ✅ 已實施 |
| **運營和變革管理** | GA 切換程序（支持 Playbook §7.2）加上 `status.md` 更新跟踪發布準備情況。通過 `docs/source/compliance/android/eu/sbom_attestation.md` 鏈接的發布證據（SBOM、Sigstore 捆綁包）。 | `docs/source/android_support_playbook.md`； `status.md`； `docs/source/compliance/android/eu/sbom_attestation.md`。 | ✅ 已實施 |
| **事件響應和報告** | Playbook 定義了嚴重性矩陣、SLA 響應窗口和合規性通知步驟；遙測覆蓋+混亂排練確保飛行員面前的再現性。 | `docs/source/android_support_playbook.md` §§4–9； `docs/source/sdk/android/telemetry_chaos_checklist.md`。 | ✅ 已實施 |
| **數據駐留/本地化** |用於 JP 部署的遙測收集器在批准的東京地區運行； StrongBox 證明捆綁包存儲在區域內並從合作夥伴票證中引用。本地化計劃確保文檔在測試版 (AND5) 之前提供日語版本。 | `docs/source/android_support_playbook.md` §9； `docs/source/sdk/android/developer_experience_plan.md` §5； `docs/source/sdk/android/readiness/android_strongbox_device_matrix.md`。 | 🈺 進行中（本地化正在進行中）|

## 審稿人註釋

- 在受監管的合作夥伴加入之前驗證 Galaxy S23/S24 的設備矩陣條目（請參閱準備文檔行 `s23-strongbox-a`、`s24-strongbox-a`）。
- 確保 JP 部署中的遙測收集器強制執行 DPIA (`docs/source/compliance/android/eu/gdpr_dpia_summary.md`) 中定義的相同保留/覆蓋邏輯。
- 一旦銀行合作夥伴審查此清單，即可獲取外部審計師的確認。