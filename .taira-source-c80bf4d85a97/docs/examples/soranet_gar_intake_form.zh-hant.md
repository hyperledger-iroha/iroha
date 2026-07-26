---
lang: zh-hant
direction: ltr
source: docs/examples/soranet_gar_intake_form.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6cd4da7e590d581719ed2607994d7d9eb16d153fbd06f85655d0da37c727853a
source_last_modified: "2025-12-29T18:16:35.085419+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# SoraNet GAR 攝入模板

請求 GAR 操作（清除、ttl 覆蓋、速率
上限、審核指令、地理圍欄或合法保留）。提交的表格
應與 `gar_controller` 輸出一起固定，以便審核日誌和
收據引用相同的證據 URI。

|領域|價值|筆記|
|--------|--------|--------|
|請求 ID |  |監護人/行動票證 ID。 |
|請求者 |  |帳號+聯繫方式。 |
|日期/時間 (UTC) |  |行動應該開始的時間。 |
| GAR 名稱 |  |例如，`docs.sora`。 |
|規範主機|  |例如，`docs.gw.sora.net`。 |
|行動|  | `ttl_override` / `rate_limit_override` / `purge_static_zone` / `geo_fence` / `legal_hold` / `moderation`。 |
| TTL 覆蓋（秒）|  |僅 `ttl_override` 需要。 |
|費率上限 (RPS) |  |僅 `rate_limit_override` 需要。 |
|允許的地區 |  |請求 `geo_fence` 時的 ISO 區域列表。 |
|被拒絕的地區 |  |請求 `geo_fence` 時的 ISO 區域列表。 |
|節制蛞蝓 |  |匹配 GAR 審核指令。 |
|清除標籤 |  |服務前必須清除的標籤。 |
|標籤|  |機器標籤（事件 ID、演習名稱、POP 範圍）。 |
|證據 URI |  |支持請求的日誌/儀表板/規格。 |
|審核 URI |  |如果與默認值不同，則按彈出審核 URI。 |
|請求到期 |  | Unix時間戳或RFC3339；默認留空。 |
|原因 |  |面向用戶的解釋；出現在收據和儀表板中。 |
|審批人 |  |監護人/委員會批准請求。 |

### 提交步驟

1. 填寫表格並將其附加到治理票證上。
2. 使用匹配的配置更新 GAR 控制器配置 (`policies`/`pops`)
   `labels`/`evidence_uris`/`expires_at_unix`。
3. 運行 `cargo xtask soranet-gar-controller ...` 以發出事件/收據。
4. 刪除 `gar_controller_summary.json`、`gar_reconciliation_report.json`、
   `gar_metrics.prom` 和 `gar_audit_log.jsonl` 放入同一張票中。的
   審批者在發送前確認收據計數與 PoP 列表匹配。