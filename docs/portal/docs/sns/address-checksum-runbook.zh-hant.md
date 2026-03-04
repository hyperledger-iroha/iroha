---
lang: zh-hant
direction: ltr
source: docs/portal/docs/sns/address-checksum-runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: fcd909a7013c5147e4f0c89c67de856ff56797b99281b954c7708ad83ab5cdc8
source_last_modified: "2026-01-28T17:11:30.699790+00:00"
translation_last_reviewed: 2026-02-07
id: address-checksum-runbook
title: Account Address Checksum Incident Runbook
sidebar_label: Checksum incidents
description: Operational response for IH58 (preferred) / compressed (`sora`, second-best) checksum failures (ADDR-7).
translator: machine-google-reviewed
---

:::注意規範來源
此頁面鏡像 `docs/source/sns/address_checksum_failure_runbook.md`。更新
首先源文件，然後同步此副本。
:::

校驗和失敗表現為 `ERR_CHECKSUM_MISMATCH` (`ChecksumMismatch`)
Torii、SDK 和錢包/瀏覽器客戶端。現在的 ADDR-6/ADDR-7 路線圖項目
每當出現校驗和警報或支持時，要求操作員遵循此操作手冊
票火了。

## 何時運行該劇

- **警報：** `AddressInvalidRatioSlo`（定義於
  `dashboards/alerts/address_ingest_rules.yml`) 行程及註釋列表
  `reason="ERR_CHECKSUM_MISMATCH"`。
- **夾具漂移：** `account_address_fixture_status` Prometheus 文本文件或
  Grafana 儀表板報告任何 SDK 副本的校驗和不匹配。
- **支持升級：** 錢包/瀏覽器/SDK 團隊引用校驗和錯誤、IME
  損壞，或剪貼板掃描不再解碼。
- **手動觀察：** Torii 日誌顯示重複的 `address_parse_error=checksum_mismatch`
  對於生產端點。

如果事件專門與 Local-8/Local-12 衝突有關，請遵循
`AddressLocal8Resurgence` 或 `AddressLocal12Collision` 劇本。

## 證據清單

|證據|命令/位置|筆記|
|----------|--------------------|--------|
| Grafana 快照 | `dashboards/grafana/address_ingest.json` |捕獲無效原因故障和受影響的端點。 |
|警報有效負載 | PagerDuty/Slack + `dashboards/alerts/address_ingest_rules.yml` |包括上下文標籤和時間戳。 |
|夾具健康| `artifacts/account_fixture/address_fixture.prom` + Grafana |證明 SDK 副本是否偏離 `fixtures/account/address_vectors.json`。 |
| PromQL 查詢 | `sum by (context) (increase(torii_address_invalid_total{reason="ERR_CHECKSUM_MISMATCH"}[5m]))` |導出事件文檔的 CSV。 |
|日誌 | `journalctl -u iroha_torii --since -30m | rg 'checksum_mismatch'`（或日誌聚合）|共享前清理 PII。 |
|夾具驗證| `cargo xtask address-vectors --verify` |確認規範生成器和提交的 JSON 一致。 |
| SDK奇偶校驗| `python3 scripts/account_fixture_helper.py check --target <path> --metrics-out artifacts/account_fixture/<label>.prom --metrics-label <label>` |針對警報/票證中報告的每個 SDK 運行。 |
|剪貼板/輸入法理智 | `iroha tools address inspect <literal>` |檢測隱藏字符或 IME 重寫；引用 `address_display_guidelines.md`。 |

## 立即響應

1. 確認警報，鏈接事件中的 Grafana 快照 + PromQL 輸出
   線程，並註意受影響的 Torii 上下文。
2. 凍結涉及地址解析的清單促銷/SDK 發布。
3. 將儀表板快照和生成的 Prometheus 文本文件工件保存在
   事件文件夾 (`docs/source/sns/incidents/YYYY-MM/<ticket>/`)。
4. 提取顯示 `checksum_mismatch` 有效負載的日誌樣本。
5. 通知 SDK 所有者 (`#sdk-parity`) 示例有效負載，以便他們進行分類。

## 根本原因隔離

### 夾具或發電機漂移

- 重新運行`cargo xtask address-vectors --verify`；如果失敗則重新生成。
- 執行`ci/account_fixture_metrics.sh`（或個別
  `scripts/account_fixture_helper.py check`) 每個 SDK 確認捆綁
  固定裝置與規範的 JSON 相匹配。

### 客戶端編碼器/IME 回歸

- 通過 `iroha tools address inspect` 檢查用戶提供的文字以查找零寬度
  連接、假名轉換或截斷的有效負載。
- 交叉檢查錢包/瀏覽器流量
  `docs/source/sns/address_display_guidelines.md`（雙複製目標、警告、
  QR 助手）以確保他們遵循批准的用戶體驗。

### 清單或註冊表問題

- 按照 `address_manifest_ops.md` 重新驗證最新的清單包並
  確保沒有 Local-8 選擇器重新出現。
  出現在有效負載中。

### 惡意或格式錯誤的流量

- 通過 Torii 日誌和 `torii_http_requests_total` 分解違規 IP/應用程序 ID。
- 保留至少 24 小時的日誌以供安全/治理跟進。

## 緩解和恢復

|場景 |行動|
|----------|---------|
|夾具漂移|重新生成 `fixtures/account/address_vectors.json`、重新運行 `cargo xtask address-vectors --verify`、更新 SDK 捆綁包並將 `address_fixture.prom` 快照附加到票證。 |
| SDK/客戶端回歸|引用規範夾具 + `iroha tools address inspect` 輸出的文件問題，以及 SDK 奇偶校驗 CI 後面的門釋放（例如，`ci/check_address_normalize.sh`）。 |
|惡意提交 |如果需要邏輯刪除選擇器，則對違規主體進行速率限製或阻止，升級至治理。 |

緩解措施落實後，重新運行上面的 PromQL 查詢以確認
`ERR_CHECKSUM_MISMATCH` 保持為零（不包括 `/tests/*`）至少
事件降級前30分鐘。

## 關閉

1. 存檔 Grafana 快照、PromQL CSV、日誌摘錄和 `address_fixture.prom`。
2. 更新 `status.md`（ADDR 部分）以及路線圖行（如果工具/文檔）
   改變了。
3. 新課程時，將事件後記錄歸檔在 `docs/source/sns/incidents/` 下
   出現。
4. 確保 SDK 發行說明提及適用的校驗和修復。
5. 確認警報保持綠色 24 小時，並且燈具檢查之前保持綠色
   解決。