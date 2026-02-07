---
lang: zh-hant
direction: ltr
source: docs/source/confidential_assets_rotation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: fd1e43316c492cc96ed107f6318841ad8db160735d4698c4f05562ff6127fda9
source_last_modified: "2026-01-22T14:35:37.492932+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//！ `roadmap.md:M3` 引用的機密資產輪換手冊。

# 機密資產輪換操作手冊

本手冊解釋了操作員如何安排和執行機密資產
輪換（參數集、驗證密鑰和策略轉換），同時
確保錢包、Torii 客戶端和內存池守衛保持確定性。

## 生命週期和狀態

機密參數集（`PoseidonParams`、`PedersenParams`、驗證密鑰）
用於導出給定高度的有效狀態的格子和助手住在
`crates/iroha_core/src/state.rs:7540`–`7561`。運行時助手清除待處理的
一旦達到目標高度就進行轉換並記錄稍後的失敗
重播 (`crates/iroha_core/src/state.rs:6725`–`6765`)。

資產政策嵌入
`pending_transition { transition_id, new_mode, effective_height, conversion_window }`
因此治理可以通過以下方式安排升級
`ScheduleConfidentialPolicyTransition` 並根據需要取消它們。參見
`crates/iroha_data_model/src/asset/definition.rs:320` 和 Torii DTO 鏡像
（`crates/iroha_torii/src/routing.rs:1539`–`1580`）。

## 輪換工作流程

1. **發布新的參數包。 ** 運營商提交
   `PublishPedersenParams`/`PublishPoseidonParams` 指令（CLI
   `iroha app zk params publish ...`）使用元數據上演新的發電機組，
   激活/棄用窗口和狀態標記。執行人拒絕
   重複的 ID、不增加的版本或每個錯誤的狀態轉換
   `crates/iroha_core/src/smartcontracts/isi/world.rs:2499`–`2635`，以及
   註冊表測試涵蓋故障模式 (`crates/iroha_core/tests/confidential_params_registry.rs:93`–`226`)。
2. **註冊/驗證密鑰更新。 ** `RegisterVerifyingKey` 強制後端，
   密鑰可以進入之前的承諾和電路/版本約束
   註冊表（`crates/iroha_core/src/smartcontracts/isi/world.rs:2067`–`2137`）。
   更新密鑰會自動棄用舊條目並擦除內聯字節，
   由 `crates/iroha_core/tests/zk_vk_deprecate_marks_status.rs:1` 行使。
3. **安排資產政策轉換。 ** 新參數 ID 生效後，
   治理調用 `ScheduleConfidentialPolicyTransition` 並提供所需的信息
   模式、轉換窗口和審核哈希。執行人拒絕衝突
   具有出色的透明供應的過渡或資產。測試如
   `crates/iroha_core/tests/confidential_policy_gates.rs:300`–`384` 驗證
   中止轉換清除 `pending_transition`，同時
   `confidential_policy_transition_reaches_shielded_only_on_schedule` 在
   第 385-433 行確認預定的升級翻轉到 `ShieldedOnly` 恰好在
   有效高度。
4. **策略應用和mempool守衛。 ** 區塊執行器清除所有待處理的區塊
   在每個塊的開始處轉換（`apply_policy_if_due`）並發出
   如果轉換失敗，則進行遙測，以便操作員可以重新安排。入院期間
   內存池拒絕有效政策會在區塊中改變的交易，
   確保整個過渡窗口的確定性包含
   （`docs/source/confidential_assets.md:60`）。

## 錢包和 SDK 要求- Swift 和其他移動 SDK 公開 Torii 幫助程序來獲取活動策略
  加上任何待處理的轉換，因此錢包可以在簽名之前警告用戶。參見
  `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:309` (DTO) 和相關的
  在 `IrohaSwift/Tests/IrohaSwiftTests/ToriiClientTests.swift:591` 進行測試。
- CLI 通過 `iroha ledger assets data-policy get`（幫助程序）鏡像相同的元數據
  `crates/iroha_cli/src/main.rs:1497`–`1670`)，使操作員能夠審核
  策略/參數 ID 連接到資產定義中，無需深入探究
  塊存儲。

## 測試和遙測覆蓋範圍

- `crates/iroha_core/tests/zk_ledger_scaffold.rs:288`–`345` 驗證該策略
  轉換傳播到元數據快照並在應用後清除。
- `crates/iroha_core/tests/zk_dedup.rs:1`證明`Preverify`緩存
  拒絕雙花/雙證明，包括輪換場景
  承諾不同。
- `crates/iroha_core/tests/zk_confidential_events.rs` 和
  `zk_shield_transfer_audit.rs` 覆蓋端到端屏蔽→傳輸→取消屏蔽
  流，確保審計跟踪在參數輪換中得以保留。
- `dashboards/grafana/confidential_assets.json` 和
  `docs/source/confidential_assets.md:401` 記錄承諾樹 &
  伴隨每次校準/旋轉運行的驗證器緩存儀表。

## 運行手冊所有權

- **DevRel / Wallet SDK Leads：** 維護 SDK 片段 + 顯示的快速入門
  如何顯示待處理的過渡並重放薄荷 → 傳輸 → 顯示
  本地測試（在 `docs/source/project_tracker/confidential_assets_phase_c.md:M3.2` 下跟踪）。
- **計劃管理/機密資產 TL：** 批准過渡請求，保留
  `status.md` 更新了即將到來的輪換，並確保豁免（如果有）
  與校準分類賬一起記錄。