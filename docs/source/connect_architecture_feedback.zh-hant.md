---
lang: zh-hant
direction: ltr
source: docs/source/connect_architecture_feedback.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 097ea58d49f48d059cda762cd719bc62f0b2d6f6ddecedef3f9bac030ae46aec
source_last_modified: "2025-12-29T18:16:35.934098+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

#！連接架構反饋清單

此清單捕獲了 Connect 會話架構中的未決問題
需要來自 Android 和 JavaScript 的輸入的稻草人
2026 年 2 月跨 SDK 研討會。用它來異步收集評論，跟踪
所有權，並暢通研討會議程。

> 狀態/備註欄捕獲了 Android 和 JS 線索的最終回复
> 2026 年 2 月研討會前同步；如果決策內聯新的後續問題
> 進化。

## 會話生命週期和傳輸

|主題 |安卓機主 | JS 所有者 |狀態/註釋|
|--------|-------------|----------|----------------|
| WebSocket 重新連接退避策略（指數與上限線性）| Android 網絡 TL | JS 主管 | ✅ 同意帶抖動的指數退避，上限為 60 秒； JS 鏡像了瀏覽器/節點奇偶校驗的相同常量。 |
|離線緩衝區容量默認值（當前稻草人：32 幀）| Android 網絡 TL | JS 主管 | ✅ 確認 32 幀默認值並覆蓋配置； Android 通過 `ConnectQueueConfig` 持久化，JS 尊重 `window.connectQueueMax`。 |
|推送式重新連接通知（FCM/APNS 與輪詢）| Android 網絡 TL | JS 主管 | ✅ Android 將為錢包應用程序公開可選的 FCM 掛鉤； JS 仍然基於輪詢，具有指數退避，並註意瀏覽器推送限制。 |
|適用於移動客戶端的乒乓球節奏護欄 | Android 網絡 TL | JS 主管 | ✅ 標準化 30 秒 ping，具有 3 倍失誤容忍度； Android平衡Doze影響，JS箝位到≥15s以避免瀏覽器限流。 |

## 加密和密鑰管理

|主題 |安卓機主 | JS 所有者 |狀態/註釋|
|--------|-------------|----------|----------------|
| X25519 關鍵存儲期望（StrongBox、WebCrypto 安全上下文）| Android 加密 TL | JS 主管 | ✅ Android 在可用時將 X25519 存儲在 StrongBox 中（回退到 TEE）； JS 要求 dApp 使用安全上下文 WebCrypto，回退到 Node.js 中的本機 `iroha_js_host` 橋。 |
| ChaCha20-Poly1305 跨 SDK 共享隨機數管理 | Android 加密 TL | JS 主管 | ✅ 採用共享 `sequence` 計數器 API，具有 64 位包裝防護和共享測試； JS 使用 BigInt 計數器來匹配 Rust 行為。 |
|硬件支持的證明負載架構 | Android 加密 TL | JS 主管 | ✅ 架構最終確定：`attestation { platform, evidence_b64, statement_hash }`； JS可選（瀏覽器），Node使用HSM插件鉤子。 |
|丟失錢包的恢復流程（密鑰輪換握手）| Android 加密 TL | JS 主管 | ✅ 接受錢包輪換握手：dApp 發出 `rotate` 控制權，錢包回復新的公鑰 + 簽名確認； JS 立即重新加密 WebCrypto 材料。 |

## 權限和證明包|主題 |安卓機主 | JS 所有者 |狀態/註釋|
|--------|-------------|----------|----------------|
| GA 的最低權限架構（方法/事件/資源）| Android 數據模型 TL | JS 主管 | ✅ GA基線：`methods`、`events`、`resources`、`constraints`； JS 將 TypeScript 類型與 Rust 清單保持一致。 |
|錢包拒絕負載（`reason_code`，本地化消息）| Android 網絡 TL | JS 主管 | ✅ 代碼最終確定（`user_declined`、`permissions_mismatch`、`compliance_failed`、`internal_error`）以及可選的 `localized_message`。 |
|證明包可選字段（合規性/KYC 附件）| Android 數據模型 TL | JS 主管 | ✅ 所有 SDK 均接受可選的 `attachments[]` (Norito `AttachmentRef`) 和 `compliance_manifest_id`；無需改變行為。 |
| Norito JSON 模式與橋生成的結構的對齊 | Android 數據模型 TL | JS 主管 | ✅ 決策：更喜歡橋生成的結構； JSON 路徑保留僅用於調試，JS 保留 `Value` 適配器。 |

## SDK 外觀和 API 形狀

|主題 |安卓機主 | JS 所有者 |狀態/註釋|
|--------|-------------|----------|----------------|
|高級異步接口（`Flow`，異步迭代器）奇偶校驗 | Android 網絡 TL | JS 主管 | ✅ Android 暴露 `Flow<ConnectEvent>`； JS使用`AsyncIterable<ConnectEvent>`；兩者都映射到共享 `ConnectEventKind`。 |
|錯誤分類映射（`ConnectError`，類型化子類）| Android 網絡 TL | JS 主管 | ✅ 採用共享枚舉 {`Transport`、`Codec`、`Authorization`、`Timeout`、`QueueOverflow`、`Internal`} 以及特定於平台的負載詳細信息。 |
|飛行中標誌請求的取消語義 | Android 網絡 TL | JS 主管 | ✅ 引入`cancelRequest(hash)`控件；這兩個 SDK 都提供了尊重錢包確認的可取消協程/承諾。 |
|共享遙測掛鉤（事件、指標命名）| Android 網絡 TL | JS 主管 | ✅ 對齊的指標名稱：`connect.queue_depth`、`connect.latency_ms`、`connect.reconnects_total`；記錄了樣本出口商。 |

## 離線持久化和日誌記錄

|主題 |安卓機主 | JS 所有者 |狀態/註釋|
|--------|-------------|----------|----------------|
|排隊幀的存儲格式（二進制 Norito 與 JSON）| Android 數據模型 TL | JS 主管 | ✅ 到處存儲二進制 Norito (`.to`)； JS 使用 IndexedDB `ArrayBuffer`。 |
|期刊保留政策和大小上限 | Android 網絡 TL | JS 主管 | ✅ 默認保留 24 小時，每次會話 1MiB；可通過 `ConnectQueueConfig` 進行配置。 |
|雙方重播幀時的衝突解決 | Android 網絡 TL | JS 主管 | ✅ 使用 `sequence` + `payload_hash`；忽略重複項，與遙測事件衝突觸發 `ConnectError.Internal`。 |
|隊列深度和重放成功的遙測 | Android 網絡 TL | JS 主管 | ✅ 發出 `connect.queue_depth` 儀表和 `connect.replay_success_total` 計數器；兩個 SDK 都連接到共享的 Norito 遙測模式。 |

## 實施峰值和參考- **Rust 橋接裝置：** `crates/connect_norito_bridge/src/lib.rs` 和相關測試涵蓋每個 SDK 使用的規範編碼/解碼路徑。
- **Swift 演示工具：** `examples/ios/NoritoDemoXcode/NoritoDemoXcodeTests/ConnectViewModelTests.swift` 練習將會話流與模擬傳輸連接起來。
- **Swift CI 門控：** 在更新 Connect 工件時運行 `make swift-ci`，以在與其他 SDK 共享之前驗證夾具奇偶校驗、儀表板源和 Buildkite `ci/xcframework-smoke:<lane>:device_tag` 元數據。
- **JavaScript SDK 集成測試：** `javascript/iroha_js/test/integrationTorii.test.js` 根據 Torii 驗證連接狀態/會話幫助程序。
- **Android 客戶端彈性註釋：** `java/iroha_android/README.md:150` 記錄了激發隊列/回退默認值的當前連接實驗。

## 研討會準備物品

- [x] Android：為上面的每個表格行分配重點人員。
- [x] JS：為上面的每個表格行指定負責人。
- [x] 收集現有實施峰值或實驗的鏈接。
- [x] 在 2026 年 2 月理事會之前安排工作前審查（與 Android TL、JS Lead、Swift Lead 於 2026 年 1 月 29 日 15:00 UTC 預訂）。
- [x] 使用已接受的答案更新 `docs/source/connect_architecture_strawman.md`。

## 預讀包

- ✅ 記錄在 `artifacts/connect/pre-read/20260129/` 下的捆綁包（刷新稻草人、SDK 指南和此清單後通過 `make docs-html` 生成）。
- 📄 總結 + 分發步驟位於 `docs/source/project_tracker/connect_architecture_pre_read.md` 中；將該鏈接包含在 2026 年 2 月研討會邀請和 `#sdk-council` 提醒中。
- 🔁 刷新捆綁包時，更新預讀註釋中的路徑和哈希，並將公告存檔在 IOS7/AND7 就緒日誌下的 `status.md` 中。