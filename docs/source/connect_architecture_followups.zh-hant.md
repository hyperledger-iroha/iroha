---
lang: zh-hant
direction: ltr
source: docs/source/connect_architecture_followups.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 476331772efe169a7a073b561fa9935e314ff89d6bfff440f7246606c1c02669
source_last_modified: "2025-12-29T18:16:35.934525+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# 連接架構後續行動

本說明記錄了跨 SDK 出現的工程後續工作
連接架構審查。每行應映射到一個問題（Jira 票證或 PR）
一旦工作安排好。當所有者創建跟踪票時更新表。|項目 |描述 |所有者 |追踪 |狀態 |
|------|-------------|----------|---------|--------|
|共享退避常數|實現指數退避 + 抖動助手 (`connect_retry::policy`) 並將其公開給 Swift/Android/JS SDK。 | Swift SDK, Android Networking TL, JS Lead | [IOS-CONNECT-001](project_tracker/connect_architecture_followups_ios.md#ios-connect-001) |已完成 — `connect_retry::policy` 具有確定性 splitmix64 採樣； Swift (`ConnectRetryPolicy`)、Android 和 JS SDK 提供鏡像助手和黃金測試。 |
|乒乓球執法 |使用商定的 30 秒節奏和瀏覽器最小限制添加可配置的心跳強制執行；表面指標 (`connect.ping_miss_total`)。 | Swift SDK、Android 網絡 TL、JS 主管 | [IOS-CONNECT-002](project_tracker/connect_architecture_followups_ios.md#ios-connect-002) |已完成 — Torii 現在強制執行可配置的心跳間隔（`ping_interval_ms`、`ping_miss_tolerance`、`ping_min_interval_ms`），公開 `connect.ping_miss_total` 指標，並提供涵蓋心跳斷開處理的回歸測試。 SDK 功能快照為客戶展示了新的旋鈕。 |
|離線隊列持久化 |使用共享模式實現 Connect 隊列的 Norito `.to` 日誌寫入器/讀取器（Swift `FileManager`、Android 加密存儲、JS IndexedDB）。 | Swift SDK、Android 數據模型 TL、JS 主管 | [IOS-CONNECT-003](project_tracker/connect_architecture_followups_ios.md#ios-connect-003) |已完成 - Swift、Android 和 JS 現在提供共享的 `ConnectQueueJournal` + 診斷助手以及保留/溢出測試，因此證據包在整個過程中保持確定性SDKs.【IrohaSwift/Sources/IrohaSwift/ConnectQueueJournal.swift:1】【java/iroha_android/src/main/java/org/hyperledger/iroha/android/connect/ConnectQueueJournal.java:1】【javascript/iroha_js/src/connectQueueJournal.js:1】 |
| StrongBox 證明負載 |通過錢包批准線程 `{platform,evidence_b64,statement_hash}` 並向 dApp SDK 添加驗證。 | Android 加密 TL、JS 主管 | [IOS-CONNECT-004](project_tracker/connect_architecture_followups_ios.md#ios-connect-004) |待定 |
|旋轉控制架|實現 `Control::RotateKeys` + `RotateKeysAck` 並在所有 SDK 中公開 `cancelRequest(hash)` / 旋轉 API。 | Swift SDK、Android 網絡 TL、JS 主管 | [IOS-CONNECT-005](project_tracker/connect_architecture_followups_ios.md#ios-connect-005) |待定 |
|遙測出口商|將 `connect.queue_depth`、`connect.reconnects_total`、`connect.latency_ms` 和重播計數器發送到現有遙測管道 (OpenTelemetry)。 |遙測工作組、SDK 所有者 | [IOS-CONNECT-006](project_tracker/connect_architecture_followups_ios.md#ios-connect-006) |待定 |
| Swift CI 門控 |確保與 Connect 相關的管道調用 `make swift-ci`，以便夾具奇偶校驗、儀表板源和 Buildkite `ci/xcframework-smoke:<lane>:device_tag` 元數據在 SDK 之間保持一致。 | Swift SDK 主管，構建基礎設施 | [IOS-CONNECT-007](project_tracker/connect_architecture_followups_ios.md#ios-connect-007) |待定 |
|後備事件報告|將 XCFramework 煙霧安全事件（`xcframework_smoke_fallback`、`xcframework_smoke_strongbox_unavailable`）連接到 Connect 儀表板以實現共享可見性。 | Swift QA 主管，構建基礎設施 | [IOS-CONNECT-008](project_tracker/connect_architecture_followups_ios.md#ios-connect-008) |待定 ||合規附件傳遞|確保 SDK 接受並轉發批准負載中的可選 `attachments[]` + `compliance_manifest_id` 字段而不會丟失。 | Swift SDK、Android 數據模型 TL、JS 主管 | [IOS-CONNECT-009](project_tracker/connect_architecture_followups_ios.md#ios-connect-009) |待定 |
|錯誤分類對齊 |使用文檔/示例將共享枚舉（`Transport`、`Codec`、`Authorization`、`Timeout`、`QueueOverflow`、`Internal`）映射到特定於平台的錯誤。 | Swift SDK、Android 網絡 TL、JS 主管 | [IOS-CONNECT-010](project_tracker/connect_architecture_followups_ios.md#ios-connect-010) |已完成 - Swift、Android 和 JS SDK 提供共享的 `ConnectError` 包裝器 + 遙測助手，以及 README/TypeScript/Java 文檔和涵蓋 TLS/timeout/HTTP/codec/queue 的回歸測試案例。 【docs/source/connect_error_taxonomy.md:1】【IrohaSwift/Sources/IrohaSwift/ConnectError.swift:1】【java/iroha_android/src/test/java/org/hyperledger/iroha/android/connect/ConnectErrorTests.java:1】【javascript/iroha_js/test/connectError.test.js:1】 |
|車間決策日誌 |將總結已接受決定的帶註釋的甲板/註釋發佈到理事會檔案中。 | SDK 項目負責人 | [IOS-CONNECT-011](project_tracker/connect_architecture_followups_ios.md#ios-connect-011) |待定 |

> 業主開票時將填寫跟踪標識符；更新 `Status` 列以及問題進度。