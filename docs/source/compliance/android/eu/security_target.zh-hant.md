---
lang: zh-hant
direction: ltr
source: docs/source/compliance/android/eu/security_target.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 385d17a55579d2b0b365e21090ee081ded79e44655690b2abfbf54068c9b55b0
source_last_modified: "2025-12-29T18:16:35.927510+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Android SDK 安全目標 — ETSI EN 319 401 對齊

|領域|價值|
|--------|--------|
|文檔版本 | 0.1 (2026-02-12) |
|範圍 | Android SDK（`java/iroha_android/` 下的客戶端庫以及支持腳本/文檔）|
|業主|合規與法律（索菲亞·馬丁斯）|
|審稿人| Android 項目主管、發布工程、SRE 治理 |

## 1. TOE 描述

評估目標 (TOE) 包括 Android SDK 庫代碼 (`java/iroha_android/src/main/java`)、其配置界面（`ClientConfig` + Norito 攝取）以及 `roadmap.md` 中針對里程碑 AND2/AND6/AND7 引用的操作工具。

主要組成部分：

1. **配置攝取** — `ClientConfig` 線程 Torii 端點、TLS 策略、重試和來自生成的 `iroha_config` 清單的遙測掛鉤，並強制執行初始化後的不變性 (`java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/ClientConfig.java`)。
2. **密鑰管理/StrongBox** — 硬件支持的簽名通過 `SystemAndroidKeystoreBackend` 和 `AttestationVerifier` 實施，策略記錄在 `docs/source/sdk/android/key_management.md` 中。證明捕獲/驗證使用 `scripts/android_keystore_attestation.sh` 和 CI 幫助程序 `scripts/android_strongbox_attestation_ci.sh`。
3. **遙測和編輯** - 通過 `docs/source/sdk/android/telemetry_redaction.md` 中描述的共享模式進行檢測，導出散列權限、分桶設備配置文件，並覆蓋支持手冊強制執行的審核掛鉤。
4. **操作手冊** — `docs/source/android_runbook.md`（操作員響應）和 `docs/source/android_support_playbook.md`（SLA + 升級）通過確定性覆蓋、混沌演練和證據捕獲強化 TOE 的操作足跡。
5. **發布來源** — 基於 Gradle 的構建使用 CycloneDX 插件以及 `docs/source/sdk/android/developer_experience_plan.md` 和 AND6 合規性檢查表中捕獲的可重現構建標誌。發布工件在 `docs/source/release/provenance/android/` 中進行簽名和交叉引用。

## 2. 資產和假設

|資產|描述 |安全目標|
|--------|-------------|--------------------|
|配置清單 | Norito 派生的 `ClientConfig` 快照隨應用程序分發。 |靜態時的真實性、完整性和機密性。 |
|簽名密鑰 |通過 StrongBox/TEE 提供商生成或導入的密鑰。 | StrongBox 首選項、證明日誌記錄、無密鑰導出。 |
|遙測流 |從 SDK 工具導出的 OTLP 跟踪/日誌/指標。 |假名化（哈希授權）、最小化 PII、覆蓋審計。 |
|賬本交互 | Norito 有效負載、准入元數據、Torii 網絡流量。 |相互身份驗證、抗重放請求、確定性重試。 |

假設：

- 移動操作系統提供標準沙箱+SELinux； StrongBox 設備實現了 Google 的 keymaster 界面。
- 運營商為 Torii 端點提供由理事會信任的 CA 簽署的 TLS 證書。
- 在發佈到 Maven 之前，構建基礎設施遵循可重複構建的要求。

## 3. 威脅與控制|威脅|控制|證據|
|--------|---------|----------|
|被篡改的配置清單| `ClientConfig` 在應用之前驗證清單（哈希+架構），並通過 `android.telemetry.config.reload` 記錄拒絕的重新加載。 | `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/ClientConfig.java`； `docs/source/android_runbook.md` §1–2。 |
|簽名密鑰的洩露 | StrongBox 所需的策略、證明工具和設備矩陣審核可識別偏差；覆蓋每個事件記錄的內容。 | `docs/source/sdk/android/key_management.md`； `docs/source/sdk/android/readiness/android_strongbox_device_matrix.md`； `scripts/android_strongbox_attestation_ci.sh`。 |
|遙測中的 PII 洩露 | Blake2b 散列權限、分桶設備配置文件、運營商遺漏、覆蓋日誌記錄。 | `docs/source/sdk/android/telemetry_redaction.md`；支持 Playbook §8。 |
| Torii RPC 上的重放或降級 | `/v2/pipeline` 請求構建器使用散列權限上下文強制執行 TLS 固定、噪聲通道策略和重試預算。 | `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/ToriiRequestBuilder.java`； `docs/source/sdk/android/networking.md`（計劃）。 |
|未簽名或不可複制的版本 | CycloneDX SBOM + Sigstore 由 AND6 檢查表門控的證明；發布 RFC 需要 `docs/source/release/provenance/android/` 中的證據。 | `docs/source/sdk/android/developer_experience_plan.md`； `docs/source/compliance/android/eu/sbom_attestation.md`。 |
|不完整的事件處理| Runbook + playbook 定義覆蓋、混亂演練和升級樹；遙測覆蓋需要簽名的 Norito 請求。 | `docs/source/android_runbook.md`； `docs/source/android_support_playbook.md`。 |

## 4. 評估活動

1. **設計審查** — 合規性 + SRE 驗證配置、密鑰管理、遙測和發布控制是否映射到 ETSI 安全目標。
2. **實施檢查** — 自動化測試：
   - `scripts/android_strongbox_attestation_ci.sh` 驗證矩陣中列出的每個 StrongBox 設備捕獲的捆綁包。
   - `scripts/check_android_samples.sh` 和託管設備 CI 確保示例應用程序遵守 `ClientConfig`/遙測合同。
3. **操作驗證** — 根據 `docs/source/sdk/android/telemetry_chaos_checklist.md` 每季度進行混亂演習（修訂 + 覆蓋練習）。
4. **證據保留** — 存儲在 `docs/source/compliance/android/`（此文件夾）下並從 `status.md` 引用的文物。

## 5. ETSI EN 319 401 映射| EN 319 401 條款 | SDK控制|
|--------------------|-------------|
| 7.1 安全政策|記錄在此安全目標 + 支持手冊中。 |
| 7.2 組織安全|支持手冊 §2 中的 RACI + 隨叫隨到所有權。 |
| 7.3 資產管理 |上面第 §2 節中定義的配置、密鑰和遙測資產目標。 |
| 7.4 訪問控制| StrongBox 策略 + 覆蓋需要簽名 Norito 工件的工作流程。 |
| 7.5 加密控制 | AND2（密鑰管理指南）的密鑰生成、存儲和證明要求。 |
| 7.6 操作安全 |遙測哈希、混亂排練、事件響應和發布證據門控。 |
| 7.7 通信安全| `/v2/pipeline` TLS 策略 + 哈希授權（遙測編輯文檔）。 |
| 7.8 系統獲取/開發| AND5/AND6 計劃中可重現的 Gradle 構建、SBOM 和來源門。 |
| 7.9 供應商關係| Buildkite + Sigstore 證明與第三方依賴項 SBOM 一起記錄。 |
| 7.10 事件管理| Runbook/Playbook 升級、覆蓋日誌記錄、遙測失敗計數器。 |

## 6. 維護

- 每當 SDK 引入新的加密算法、遙測類別或發布自動化更改時，請更新此文檔。
- 將 `docs/source/compliance/android/evidence_log.csv` 中的簽名副本與 SHA-256 摘要和審閱者簽名鏈接起來。