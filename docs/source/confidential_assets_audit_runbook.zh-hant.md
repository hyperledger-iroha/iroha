---
lang: zh-hant
direction: ltr
source: docs/source/confidential_assets_audit_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8691a94d23e589f46d8e8cf2359d6d9a31f7c38c5b7bf0def69c88d2dd081765
source_last_modified: "2026-01-22T14:35:37.510319+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//！ `roadmap.md:M4` 引用的機密資產審計和運營手冊。

# 機密資產審計和運營手冊

本指南整合了審核員和操作員所依賴的證據表面
驗證機密資產流時。它補充了輪換策略
(`docs/source/confidential_assets_rotation.md`) 和校準分類帳
（`docs/source/confidential_assets_calibration.md`）。

## 1. 選擇性披露和事件源

- 每條機密指令都會發出結構化的 `ConfidentialEvent` 有效負載
  （`Shielded`、`Transferred`、`Unshielded`）捕獲於
  `crates/iroha_data_model/src/events/data/events.rs:198` 並由
  執行者 (`crates/iroha_core/src/smartcontracts/isi/world.rs:3699`–`4021`)。
  回歸套件測試了具體的有效負載，以便審計人員可以信賴
  確定性 JSON 佈局 (`crates/iroha_core/tests/zk_confidential_events.rs:19`–`299`)。
- Torii 通過標準 SSE/WebSocket 管道公開這些事件；審計員
  使用 `ConfidentialEventFilter` (`crates/iroha_data_model/src/events/data/filters.rs:82`) 訂閱，
  可以選擇將範圍限定為單個資產定義。 CLI 示例：

  ```bash
  iroha ledger events data watch --filter '{ "confidential": { "asset_definition_id": "62Fk4FPcMuLvW5QjDGNF2a4jAmjM" } }'
  ```

- 政策元數據和待定轉換可通過
  `GET /v1/confidential/assets/{definition_id}/transitions`
  (`crates/iroha_torii/src/routing.rs:15205`)，由 Swift SDK 鏡像
  (`IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:3245`) 並記錄在
  機密資產設計和 SDK 指南
  （`docs/source/confidential_assets.md:70`、`docs/source/sdk/swift/index.md:334`）。

## 2. 遙測、儀表板和校准證據

- 運行時指標表面樹深度、承諾/前沿歷史、根驅逐
  計數器和驗證者緩存命中率
  （`crates/iroha_telemetry/src/metrics.rs:5760`–`5815`）。 Grafana 儀表板
  `dashboards/grafana/confidential_assets.json` 運送相關面板和
  警報，工作流程記錄在 `docs/source/confidential_assets.md:401` 中。
- 帶有簽名日誌的校準運行（NS/op、gas/op、ns/gas）
  `docs/source/confidential_assets_calibration.md`。最新的蘋果芯片
  NEON 運行存檔於
  `docs/source/confidential_assets_calibration_neon_20260428.log`，同樣
  ledger 記錄 SIMD 中性和 AVX2 配置文件的臨時豁免，直到
  x86 主機上線。

## 3. 事件響應和操作員任務

- 輪換/升級程序位於
  `docs/source/confidential_assets_rotation.md`，涵蓋如何上演新內容
  參數包、安排策略升級並通知錢包/審計員。的
  跟踪器 (`docs/source/project_tracker/confidential_assets_phase_c.md`) 列表
  操作手冊所有者和排練期望。
- 對於生產排練或緊急窗口，操作員將證據附在
  `status.md` 條目（例如，多車道排練日誌）並包括：
  `curl` 策略轉換證明、Grafana 快照以及相關事件
  摘要，以便審計員可以重建鑄幣→轉移→披露時間表。

## 4. 外部審核節奏

- 安全審查範圍：機密電路、參數註冊表、策略
  轉換和遙測。本文件加上校準分類帳表格
  發送給供應商的證據包；審核安排通過以下方式跟踪
  `docs/source/project_tracker/confidential_assets_phase_c.md` 中的 M4。
- 運營商必須隨時更新 `status.md` 的任何供應商調查結果或後續行動
  行動項目。在外部審核完成之前，本運行手冊將作為
  運營基線審計員可以進行測試。