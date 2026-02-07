---
slug: /sdks/android-telemetry
lang: zh-hant
direction: ltr
source: docs/portal/docs/sdks/android-telemetry-redaction.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Android Telemetry Redaction Plan
sidebar_label: Android Telemetry
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::注意規範來源
:::

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Android 遙測修訂計劃 (AND7)

## 範圍

本文檔涵蓋了擬議的遙測編輯政策和啟用
按照路線圖項目 **AND7** 的要求，提供 Android SDK 的工件。它對齊
具有 Rust 節點基線的移動儀器，同時考慮
特定於設備的隱私保證。輸出作為預讀
2026 年 2 月 SRE 治理審查。

目標：

- 對每個 Android 發出的達到共享可觀測性的信號進行編目
  後端（OpenTelemetry 跟踪、Norito 編碼日誌、指標導出）。
- 對與 Rust 基線和文檔密文不同的字段進行分類或
  保留控制。
- 概述啟用和測試工作，以便支持團隊可以做出響應
  確定性地處理與編輯相關的警報。

## 信號清單（草案）

計劃的儀器按通道分組。所有字段名稱均遵循 Android
SDK 遙測架構 (`org.hyperledger.iroha.android.telemetry.*`)。可選
字段標有 `?`。

|信號ID |頻道|關鍵領域 | PII/PHI 分類 |編輯/保留|筆記|
|------------|---------|------------|------------------------|------------------------|--------|
| `android.torii.http.request` |跟踪跨度| `authority_hash`、`route`、`status_code`、`latency_ms` |權威是公開的；路線沒有秘密|導出前發出散列權限 (`blake2b_256`)；保留7天|鏡子銹 `torii.http.request`；散列可確保移動別名隱私。 |
| `android.torii.http.retry` |活動 | `route`、`retry_count`、`error_code`、`backoff_ms` |無 |沒有編輯；保留30天|用於確定性重試審核；與 Rust 字段相同。 |
| `android.pending_queue.depth` |規格公制| `queue_type`，`depth` |無 |沒有編輯；保留 90 天 |匹配 Rust `pipeline.pending_queue_depth`。 |
| `android.keystore.attestation.result` |活動 | `alias_label`、`security_level`、`attestation_digest`、`device_brand_bucket` |別名（派生）、設備元數據 |用確定性標籤替換別名，將品牌編輯為枚舉存儲桶 | AND2 證明準備所需； Rust 節點不發出設備元數據。 |
| `android.keystore.attestation.failure` |專櫃| `alias_label`、`failure_reason` |別名編輯後無 |沒有編輯；保留 90 天 |支持混沌演練； alias_label 源自散列別名。 |
| `android.telemetry.redaction.override` |活動 | `override_id`、`actor_role_masked`、`reason`、`expires_at` |演員角色符合可操作 PII |字段導出蒙面角色類別；保留 365 天審核日誌 | Rust 中不存在；操作員必須通過支持提交覆蓋文件。 |
| `android.telemetry.export.status` |專櫃| `backend`，`status` |無 |沒有編輯；保留30天|與 Rust 導出器狀態計數器相同。 |
| `android.telemetry.redaction.failure` |專櫃| `signal_id`，`reason` |無 |沒有編輯；保留30天|需要鏡像 Rust `streaming_privacy_redaction_fail_total`。 |
| `android.telemetry.device_profile` |儀表| `profile_id`、`sdk_level`、`hardware_tier` |設備元數據 |發出粗桶（SDK 主要，硬件層）；保留30天|啟用奇偶校驗儀表板而不暴露 OEM 細節。 |
| `android.telemetry.network_context` |活動 | `network_type`，`roaming` |運營商可能是 PII |完全刪除 `carrier_name`；保留其他字段 7 天 | `ClientConfig.networkContextProvider` 提供經過清理的快照，以便應用程序可以發出網絡類型+漫遊，而不會暴露訂戶數據； Parity 儀表板將信號視為 Rust `peer_host` 的移動模擬。 |
| `android.telemetry.config.reload` |活動 | `source`、`result`、`duration_ms` |無 |沒有編輯；保留30天|鏡像 Rust 配置重新加載範圍。 |
| `android.telemetry.chaos.scenario` |活動 | `scenario_id`、`outcome`、`duration_ms`、`device_profile` |設備配置文件已存儲 |與 `device_profile` 相同；保留30天|在 AND7 準備就緒所需的混亂排練期間記錄。 |
| `android.telemetry.redaction.salt_version` |儀表| `salt_epoch`，`rotation_id` |無 |沒有編輯；保留 365 天 |跟踪 Blake2b 鹽旋轉；當 Android 哈希紀元與 Rust 節點不同時發出奇偶校驗警報。 |
| `android.crash.report.capture` |活動 | `crash_id`、`signal`、`process_state`、`has_native_trace`、`anr_watchdog_bucket` |崩潰指紋+進程元數據 |哈希 `crash_id` 與共享編輯鹽、存儲桶看門狗狀態、導出前丟棄堆棧幀；保留30天|調用`ClientConfig.Builder.enableCrashTelemetryHandler()`時自動啟用；提供奇偶校驗儀表板，而不暴露設備識別痕跡。 |
| `android.crash.report.upload` |專櫃| `crash_id`、`backend`、`status`、`retry_count` |崩潰指紋|重用散列 `crash_id`，僅發出狀態；保留30天|通過 `ClientConfig.crashTelemetryReporter()` 或 `CrashTelemetryHandler.recordUpload` 發出，因此上傳與其他遙測共享相同的 Sigstore/OLTP 保證。 |

### 實施掛鉤

- `ClientConfig` 現在通過線程清單導出的遙測數據
  `setTelemetryOptions(...)`/`setTelemetrySink(...)`，自動註冊
  `TelemetryObserver` 因此，散列權威和鹽指標流無需定制觀察者。
  參見 `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/ClientConfig.java`
  以及下面的同伴類
  `java/iroha_android/src/main/java/org/hyperledger/iroha/android/telemetry/`。
- 應用程序可以調用
  `ClientConfig.Builder.enableAndroidNetworkContext(android.content.Context)` 註冊
  基於反射的 `AndroidNetworkContextProvider`，在運行時查詢 `ConnectivityManager`
  並發出 `android.telemetry.network_context` 事件，而不引入編譯時 Android
  依賴關係。
- 單元測試 `TelemetryOptionsTests` 和 `TelemetryObserverTests`
  (`java/iroha_android/src/test/java/org/hyperledger/iroha/android/telemetry/`) 保護哈希
  幫助程序加上 ClientConfig 集成掛鉤，因此明顯的回歸會立即浮現出來。
- 支持套件/實驗室現在引用具體的 API 而不是偽代碼，保留此文檔並
  Runbook 與發布的 SDK 保持一致。

> **操作注意：** 所有者/狀態工作表位於
> `docs/source/sdk/android/readiness/signal_inventory_worksheet.md` 並且必須是
> 在每個 AND7 檢查點期間與此表一起更新。

## 奇偶校驗允許列表和架構差異工作流程

治理需要雙重許可名單，以便 Android 導出不會洩漏標識符
Rust 服務有意浮出水面。此部分反映了 Runbook 條目
（`docs/source/android_runbook.md` §2.3）但保留 AND7 修訂計劃
獨立的。

|類別 | Android 出口商 |防銹服務|驗證鉤子 |
|----------|--------------------|------------------------|------|
|權限/路線上下文|通過 Blake2b-256 對 `authority`/`alias` 進行哈希處理，並在導出之前刪除原始 Torii 主機名；發出 `android.telemetry.redaction.salt_version` 以證明鹽旋轉。 |發出完整的 Torii 主機名和對等 ID 以進行關聯。 |比較 `docs/source/sdk/android/readiness/schema_diffs/` 下最新架構差異中的 `android.torii.http.request` 與 `torii.http.request` 條目，然後運行 ​​`scripts/telemetry/check_redaction_status.py` 來確認鹽紀元。 |
|設備和簽名者身份 |存儲桶 `hardware_tier`/`device_profile`、哈希控制器別名，並且從不導出序列號。 |逐字發出驗證器 `peer_id`、控制器 `public_key` 和隊列哈希。 |與 `docs/source/sdk/mobile_device_profile_alignment.md` 保持一致，在 `java/iroha_android/run_tests.sh` 內進行別名哈希測試，並在實驗室期間存檔隊列檢查器輸出。 |
|網絡元數據|僅導出 `network_type` + `roaming`；刪除 `carrier_name`。 |保留對等主機名/TLS 端點元數據。 |將每個架構差異存儲在 `readiness/schema_diffs/` 中，並在 Grafana 的“網絡上下文”小部件顯示運營商字符串時發出警報。 |
|覆蓋/混亂證據|發出帶有蒙面演員角色的 `android.telemetry.redaction.override`/`android.telemetry.chaos.scenario`。 |發出未屏蔽的覆蓋批准；沒有特定於混沌的跨度。 |演習後交叉檢查 `docs/source/sdk/android/readiness/and7_operator_enablement.md`，以確保覆蓋令牌 + 混亂文物與未掩蓋的 Rust 事件並存。 |

工作流程：

1. 每次清單/導出器更改後，運行
   `scripts/telemetry/run_schema_diff.sh --android-config <android.json> --rust-config <rust.json>` 並將 JSON 放置在 `docs/source/sdk/android/readiness/schema_diffs/` 下。
2. 根據上表檢查差異。如果 Android 發出 Rust-only 字段
   （反之亦然），提交 AND7 就緒性錯誤並更新此計劃和
   運行手冊。
3. 在每週運營審查期間，執行
   `scripts/telemetry/check_redaction_status.py --status-url https://android-telemetry-stg/api/redaction/status`
   並在準備工作表中記錄鹽紀元和模式差異時間戳。
4. 在 `docs/source/sdk/android/readiness/signal_inventory_worksheet.md` 中記錄任何偏差
   因此治理數據包捕獲奇偶校驗決策。

> **架構參考：** 規範字段標識符源自
> `android_telemetry_redaction.proto`（在 Android SDK 構建期間實現）
> 與 Norito 描述符並列）。該架構公開了 `authority_hash`，
> `alias_label`、`attestation_digest`、`device_brand_bucket` 和
> `actor_role_masked` 字段現在在 SDK 和遙測導出器中使用。

`authority_hash`是記錄的Torii權限值的固定32字節摘要
在原型中。 `attestation_digest` 捕獲規範證明聲明
指紋，而 `device_brand_bucket` 將原始 Android 品牌字符串映射到
批准的枚舉（`generic`、`oem`、`enterprise`）。 `actor_role_masked`攜帶
密文覆蓋參與者類別（`support`、`sre`、`audit`）而不是
原始用戶標識符。

### 崩潰遙測導出對齊崩潰遙測現在共享相同的 OpenTelemetry 導出器和出處
管道作為Torii聯網信號，關閉治理後續約
重複的出口商。崩潰處理程序為 `android.crash.report.capture` 提供數據
具有哈希 `crash_id` 的事件（Blake2b-256 已使用修訂鹽
由 `android.telemetry.redaction.salt_version` 跟踪），進程狀態桶，
並清理了 ANR 看門狗元數據。堆棧跟踪保留在設備上並且僅
之前總結為 `has_native_trace` 和 `anr_watchdog_bucket` 字段
導出，以便沒有 PII 或 OEM 字符串離開設備。

上傳崩潰會創建 `android.crash.report.upload` 計數器條目，
允許 SRE 審核後端可靠性，而無需了解任何相關信息
用戶或堆棧跟踪。由於兩個信號都重用了 Torii 導出器，因此它們繼承了
已定義相同的 Sigstore 簽名、保留策略和警報掛鉤
對於AND7。因此，支持運行手冊可以將哈希崩潰標識符關聯起來
Android 和 Rust 證據包之間沒有定制的崩潰管道。

通過 `ClientConfig.Builder.enableCrashTelemetryHandler()` 啟用一次處理程序
配置遙測選項和接收器；崩潰上傳橋可以重用
`ClientConfig.crashTelemetryReporter()`（或 `CrashTelemetryHandler.recordUpload`）
在同一簽名管道中發出後端結果。

## 策略增量與 Rust 基線

Android 和 Rust 遙測策略之間的差異以及緩解步驟。

|類別 | Rust 基線 |安卓政策 |緩解/驗證|
|----------|--------------|----------------|------------------------|
|權威/同行標識符|簡單的權限字符串 | `authority_hash`（Blake2b-256，旋轉鹽）|通過 `iroha_config.telemetry.redaction_salt` 發布的共享鹽；奇偶校驗測試確保支持人員的可逆映射。 |
|主機/網絡元數據 |導出的節點主機名/IP |網絡類型+僅限漫遊 |網絡運行狀況儀表板更新為使用可用性類別而不是主機名。 |
|設備特點|不適用（服務器端）|分桶配置文件（SDK 21/23/29+，層 `emulator`/`consumer`/`enterprise`）|混沌演練驗證桶映射；當需要更詳細的細節時，支持運行手冊文檔升級路徑。 |
|密文覆蓋 |不支持 |手動覆蓋令牌存儲在 Norito 分類賬中（`actor_role_masked`、`reason`）|覆蓋需要簽名請求；審核日誌保留1年。 |
|鑑證痕跡|僅通過 SRE 進行服務器認證 | SDK 發出經過淨化的證明摘要 |針對 Rust 證明驗證器交叉檢查證明哈希值；散列別名可防止洩漏。 |

驗證清單：

- 每個信號的編輯單元測試之前驗證散列/屏蔽字段
  出口商提交。
- 架構差異工具（與 Rust 節點共享）每晚運行以確認字段奇偶校驗。
- 混沌排練腳本練習覆蓋工作流程並確認審核日誌記錄。

## 實施任務（SRE 之前的治理）

1. **庫存確認** — 與實際 Android SDK 交叉驗證上表
   檢測掛鉤和 Norito 架構定義。所有者： 安卓
   可觀察性 TL，法學碩士。
2. **遙測模式差異** - 針對 Rust 指標運行共享差異工具以
   為 SRE 審查生成奇偶校驗工件。所有者：SRE 隱私主管。
3. **運行手冊草案（2026年2月3日完成）** — `docs/source/android_runbook.md`
   現在記錄了端到端覆蓋工作流程（第 3 節）和擴展的
   升級矩陣加上角色職責（第 3.1 節），綁定 CLI
   幫助者、事件證據和混亂腳本返回到治理策略。
   所有者：法學碩士，具有文檔/支持編輯功能。
4. **啟用內容** — 準備簡報幻燈片、實驗室說明和
   2026 年 2 月課程的知識檢查問題。所有者：文檔/支持
   SRE 支持團隊經理。

## 支持工作流程和 Runbook 掛鉤

### 1.本地+CI煙霧覆蓋

- `scripts/android_sample_env.sh --telemetry --telemetry-duration=5m --telemetry-cluster=<host>` 啟動 Torii 沙箱，重放規範的多源 SoraFS 固定裝置（委託給 `ci/check_sorafs_orchestrator_adoption.sh`），並播種合成 Android 遙測。
  - 流量生成由 `scripts/telemetry/generate_android_load.py` 處理，它在 `artifacts/android/telemetry/load-generator.log` 下記錄請求/響應轉錄，並支持標頭、路徑覆蓋或試運行模式。
  - 幫助程序將 SoraFS 記分板/摘要復製到 `${WORKDIR}/sorafs/` 中，以便 AND7 排練可以在接觸移動客戶端之前證明多源奇偶校驗。
- CI 重用相同的工具：`ci/check_android_dashboard_parity.sh` 針對 `dashboards/grafana/android_telemetry_overview.json`、Rust 參考儀表板和 `dashboards/data/android_rust_dashboard_allowances.json` 上的限額文件運行 `scripts/telemetry/compare_dashboards.py`，發出簽名的差異快照 `docs/source/sdk/android/readiness/dashboard_parity/android_vs_rust-latest.json`。
- 混亂排練遵循 `docs/source/sdk/android/telemetry_chaos_checklist.md`； Sample-env 腳本加上儀表板奇偶校驗形成“就緒”證據包，為 AND7 老化審計提供支持。

### 2. 覆蓋發行和審計跟踪

- `scripts/android_override_tool.py` 是用於發布和撤銷修訂覆蓋的規範 CLI。 `apply` 攝取簽名請求，發出清單包（默認情況下為 `telemetry_redaction_override.to`），並將哈希令牌行附加到 `docs/source/sdk/android/telemetry_override_log.md` 中。 `revoke` 針對同一行標記撤銷時間戳，`digest` 寫入治理所需的經過清理的 JSON 快照。
- CLI 拒絕修改審核日誌，除非存在 Markdown 表頭，符合 `docs/source/android_support_playbook.md` 中捕獲的合規性要求。 `scripts/tests/test_android_override_tool_cli.py` 中的單元覆蓋範圍保護表解析器、清單發射器和錯誤處理。
- 每當執行覆蓋時，操作員都會在 `docs/source/sdk/android/readiness/override_logs/` 下附加生成的清單、更新的日誌摘錄、**和** 摘要 JSON；根據本計劃中的治理決策，日誌保留 365 天的歷史記錄。

### 3. 證據捕獲和保留

- 每次排練或事件都會產生 `artifacts/android/telemetry/` 下的結構化捆綁包，其中包含：
  - 來自 `generate_android_load.py` 的負載生成器轉錄和聚合計數器。
  - 儀表板奇偶校驗差異 (`android_vs_rust-<stamp>.json`) 和 `ci/check_android_dashboard_parity.sh` 發出的允許哈希值。
  - 覆蓋日誌增量（如果授予覆蓋）、相應的清單和刷新的摘要 JSON。
- SRE 老化報告引用了這些工件以及 `android_sample_env.sh` 複製的 SoraFS 記分板，為 AND7 準備情況審查提供了從遙測哈希 → 儀表板 → 覆蓋狀態的確定性鏈。

## 跨 SDK 設備配置文件對齊

儀表板將 Android 的 `hardware_tier` 轉換為規範
`mobile_profile_class` 定義於
`docs/source/sdk/mobile_device_profile_alignment.md` 所以 AND7 和 IOS7 遙測
比較相同的隊列：

- `lab` — 發出為 `hardware_tier = emulator`，與 Swift 匹配
  `device_profile_bucket = simulator`。
- `consumer` — 發出為 `hardware_tier = consumer`（帶有 SDK 主後綴）
  並與 Swift 的 `iphone_small`/`iphone_large`/`ipad` 存儲桶分組。
- `enterprise` — 發出為 `hardware_tier = enterprise`，與 Swift 一致
  `mac_catalyst` 存儲桶和未來的託管/iOS 桌面運行時。

任何新層都必須添加到對齊文檔和架構差異工件中
在儀表板消耗它之前。

## 治理與分配

- **預讀包** — 本文檔加上附錄工件（架構差異、
  Runbook diff、準備平台大綱）將分發給 SRE 治理
  郵件列表不晚於 **2026-02-05**。
- **反饋循環** — 在治理期間收集的意見將反饋到
  `AND7` JIRA 史詩；攔截器出現在 `status.md` 和 Android 周刊中
  站立筆記。
- **發布** — 一旦獲得批准，政策摘要將鏈接至
  `docs/source/android_support_playbook.md` 並由共享引用
  `docs/source/telemetry.md` 中的遙測常見問題解答。

## 審計與合規說明

- 政策通過刪除移動用戶數據來遵守 GDPR/CCPA 要求
  出口前；哈希授權鹽每季度輪換一次並存儲在
  共享秘密庫。
- 啟用工件和運行手冊更新記錄在合規性註冊表中。
- 季度審查確認覆蓋仍然是閉環的（沒有過時的訪問）。

## 治理成果 (2026-02-12)

**2026-02-12** 的 SRE 治理會議批准了 Android 修訂
政策無需修改。關鍵決策（參見
`docs/source/sdk/android/telemetry_redaction_minutes_20260212.md`）：

- **策略接受。 ** 哈希授權、設備配置文件存儲以及
  批准省略承運人名稱。鹽旋轉跟踪通過
  `android.telemetry.redaction.salt_version` 成為季度審核項目。
- **驗證計劃。 ** 單元/集成覆蓋率、夜間架構差異運行以及
  每季度的混亂排練得到了認可。操作項：發布儀表板
  每次排練後的平價報告。
- **覆蓋治理。 ** Norito 記錄的覆蓋代幣已獲得批准
  365 天的保留窗口。支持工程將擁有覆蓋日誌
  每月操作同步期間摘要審查。

## 後續狀態

1. **設備配置文件對齊（截至 2026 年 3 月 1 日）。 ** ✅ 已完成 — 共享
   `docs/source/sdk/mobile_device_profile_alignment.md` 中的映射定義瞭如何
   Android `hardware_tier` 值映射到規範 `mobile_profile_class`
   由奇偶校驗儀表板和架構差異工具使用。

## 即將發布的 SRE 治理簡介（2026 年第 2 季度）路線圖項目 **AND7** 要求下一個 SRE 治理會議收到
簡潔的 Android 遙測修訂預讀。使用此部分作為生活
簡短；在每次理事會會議之前保持更新。

### 準備清單

1. **證據包** — 導出最新的架構差異、儀表板屏幕截圖、
   並覆蓋日誌摘要（參見下面的矩陣）並將它們放在日期下
   文件夾（例如
   `docs/source/sdk/android/readiness/and7_sre_brief/2026-02-07/`) 之前
   發送邀請。
2. **演練總結** — 附上最近的混亂演練日誌以及
   `android.telemetry.redaction.failure` 指標快照；確保警報管理器
   註釋引用相同的時間戳。
3. **覆蓋審核** — 確認所有活動覆蓋均記錄在 Norito 中
   登記並在會議甲板上進行總結。包括有效期和
   相應的事件 ID。
4. **議程說明** — 在會議前 48 小時通知 SRE 主席
   簡短的鏈接，突出顯示所需的任何決策（新信號、保留
   更改或覆蓋策略更新）。

### 證據矩陣

|文物|地點 |業主|筆記|
|----------|----------|--------|--------|
|架構差異與 Rust | `docs/source/sdk/android/readiness/schema_diffs/<latest>.json` |遙測工具 DRI |必須在會議前 72 小時內生成。 |
|儀表板差異截圖| `docs/source/sdk/android/readiness/dashboards/<date>/` |可觀察性 TL |包括 `sorafs.fetch.*`、`android.telemetry.*` 和 Alertmanager 快照。 |
|覆蓋摘要 | `docs/source/sdk/android/readiness/override_logs/<date>.json` |支持工程|針對最新的 `telemetry_override_log.md` 運行 `scripts/android_override_tool.sh digest`（請參閱該目錄中的 README）；令牌在共享之前保持散列狀態。 |
|混沌排練日誌| `artifacts/android/telemetry/chaos/<date>/log.ndjson` |質量保證自動化 |附上 KPI 摘要（停頓次數、重試率、覆蓋使用情況）。 |

### 向理事會提出的開放性問題

- 既然如此，我們是否需要將覆蓋保留窗口從 365 天縮短
  摘要是自動化的嗎？
- `android.telemetry.device_profile`是否應該採用新的共享
  `mobile_profile_class` 標籤在下一個版本中，或者等待 Swift/JS
  SDK 是否會帶來相同的更改？
- Torii 後，區域數據駐留是否需要額外指導
  Norito-RPC事件登陸Android（NRPC-3後續）？

### 遙測模式差異過程

每個候選版本至少運行一次 schema diff 工具（並且每當 Android
儀器發生變化），因此 SRE 理事會會收到新的平價工件以及
儀表板差異：

1. 導出要比較的 Android 和 Rust 遙測模式。對於 CI，配置是實時的
   在 `configs/android_telemetry.json` 和 `configs/rust_telemetry.json` 下。
2. 執行`scripts/telemetry/run_schema_diff.sh --android-config configs/android_telemetry.json --rust-config configs/rust_telemetry.json --out docs/source/sdk/android/readiness/schema_diffs/<date>-android_vs_rust.json`。
   - 或者將提交 (`scripts/telemetry/run_schema_diff.sh android-main rust-main`) 傳遞給
     直接從 git 拉取配置；該腳本將哈希值固定在工件內。
3. 將生成的 JSON 附加到就緒包並從 `status.md` + 鏈接它
   `docs/source/telemetry.md`。差異突出顯示添加/刪除的字段和保留增量，以便
   審計人員無需重放該工具即可確認編輯奇偶校驗。
4. 當差異顯示允許的差異時（例如，僅限 Android 的覆蓋信號），請更新
   `ci/check_android_dashboard_parity.sh` 引用的津貼文件並註意其中的基本原理
   schema-diff 目錄 README。

> **存檔規則：** 將最近的五個差異保留在
> `docs/source/sdk/android/readiness/schema_diffs/` 並將舊快照移至
> `artifacts/android/telemetry/schema_diffs/`，以便治理審核者始終看到最新數據。