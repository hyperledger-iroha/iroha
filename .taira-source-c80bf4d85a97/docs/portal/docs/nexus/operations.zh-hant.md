---
lang: zh-hant
direction: ltr
source: docs/portal/docs/nexus/operations.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: dea5098afdf15e92c78aba363b38f3ec8ce2018672a3d34bb1b505e2ee2f5869
source_last_modified: "2025-12-29T18:16:35.144858+00:00"
translation_last_reviewed: 2026-02-07
id: nexus-operations
title: Nexus operations runbook
description: Field-ready summary of the Nexus operator workflow, mirroring `docs/source/nexus_operations.md`.
translator: machine-google-reviewed
---

使用此頁面作為快速參考同級頁面
`docs/source/nexus_operations.md`。它提煉出操作清單、更改
Nexus 操作員必須執行的管理掛鉤和遙測覆蓋要求
跟隨。

## 生命週期清單

|舞台|行動|證據|
|--------|--------|----------|
|飛行前 |驗證發布哈希/簽名，確認 `profile = "iroha3"`，並準備配置模板。 | `scripts/select_release_profile.py` 輸出、校驗和日誌、簽名清單包。 |
|目錄對齊 |根據理事會發布的清單更新 `[nexus]` 目錄、路由策略和 DA 閾值，然後捕獲 `--trace-config`。 | `irohad --sora --config … --trace-config` 輸出與登機單一起存儲。 |
|煙霧和切換 |運行 `irohad --sora --config … --trace-config`，執行 CLI Smoke (`FindNetworkStatus`)，驗證遙測導出並請求准入。 |冒煙測試日誌 + Alertmanager 確認。 |
|穩態|監控儀表板/警報，根據治理節奏輪換密鑰，並在清單發生變化時同步配置/運行手冊。 |季度審核記錄、儀表板屏幕截圖、輪換票 ID。 |

詳細的入職培訓（密鑰更換、路由模板、發布配置文件步驟）
保留在 `docs/source/sora_nexus_operator_onboarding.md` 中。

## 變更管理

1. **發布更新** – 跟踪 `status.md`/`roadmap.md` 中的公告；附加
   每個版本 PR 的入門清單。
2. **通道清單更改** – 驗證來自空間目錄的簽名包並
   將它們存檔在 `docs/source/project_tracker/nexus_config_deltas/` 下。
3. **配置增量** – 每個 `config/config.toml` 更改都需要票證
   引用車道/數據空間。存儲有效配置的編輯副本
   每當節點加入或升級時。
4. **回滾演習** – 每季度排練停止/恢復/煙霧程序；日誌
   `docs/source/project_tracker/nexus_config_deltas/<date>-rollback.md` 下的結果。
5. **合規性批准** – 私有/CBDC通道必須確保合規性簽字
   在修改 DA 策略或遙測編輯旋鈕之前（請參閱
   `docs/source/cbdc_lane_playbook.md`）。

## 遙測和 SLO

- 儀表板：`dashboards/grafana/nexus_lanes.json`、`nexus_settlement.json`，以及
  SDK 特定視圖（例如，`android_operator_console.json`）。
- 警報：`dashboards/alerts/nexus_audit_rules.yml` 和 Torii/Norito 傳輸
  規則（`dashboards/alerts/torii_norito_rpc_rules.yml`）。
- 值得關注的指標：
  - `nexus_lane_height{lane_id}` – 三個插槽的進度為零時發出警報。
  - `nexus_da_backlog_chunks{lane_id}` – 高於車道特定閾值的警報
    （默認 64 個公共/8 個私有）。
  - `nexus_settlement_latency_seconds{lane_id}` – 當 P99 超過 900ms 時發出警報
    （公共）或 1200 毫秒（私人）。
  - `torii_request_failures_total{scheme="norito_rpc"}` – 如果出現 5 分鐘錯誤則發出警報
    比例>2%。
  - `telemetry_redaction_override_total` – 立即 Sev2；確保覆蓋
    有合規票。
- 運行遙測修復清單
  [Nexus 遙測修復計劃](./nexus-telemetry-remediation) 至少
  每季度一次，並將填寫好的表格附加到運營審查記錄中。

## 事件矩陣

|嚴重性 |定義 |回應 |
|----------|------------|----------|
|嚴重程度1 |數據空間隔離違規、結算暫停>15分鐘或治理投票腐敗。 | Nexus 頁 主要 + 發布工程 + 合規性、凍結准入、收集文物、發布通信 ≤60 分鐘，RCA ≤5 個工作日。 |
|嚴重程度2 |車道積壓 SLA 違規、遙測盲點 >30 分鐘、清單推出失敗。 |頁 Nexus 主要 + SRE，緩解 ≤4 小時，2 個工作日內歸檔跟進。 |
|嚴重程度3 |非阻塞漂移（文檔、警報）。 |登錄跟踪器，安排衝刺內的修復。 |

事件單必須記錄受影響的車道/數據空間 ID、清單哈希值、
時間表、支持指標/日誌以及後續任務/所有者。

## 證據存檔

- 將捆綁包/清單/遙測導出存儲在 `artifacts/nexus/<lane>/<date>/` 下。
- 保留每個版本的編輯配置+ `--trace-config` 輸出。
- 當配置或清單更改土地時，附上理事會會議記錄+簽署的決定。
- 將與 Nexus 指標相關的每週 Prometheus 快照保留 12 個月。
- 在 `docs/source/project_tracker/nexus_config_deltas/README.md` 中記錄操作手冊編輯
  這樣審計人員就知道職責何時發生變化。

## 相關材料

- 概述：[Nexus 概述](./nexus-overview)
- 規格：[Nexus 規格](./nexus-spec)
- 車道幾何形狀：[Nexus 車道模型](./nexus-lane-model)
- 轉換和佈線墊片：[Nexus 轉換說明](./nexus-transition-notes)
- 操作員入門：[Sora Nexus 操作員入門](./nexus-operator-onboarding)
- 遙測修復：[Nexus 遙測修復計劃](./nexus-telemetry-remediation)