---
lang: zh-hant
direction: ltr
source: docs/source/compliance/android/eu/gdpr_dpia_summary.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8ef338a20104dc5d15094e28a1332a604b68bdcfef1ff82fea784d43fdbd10b5
source_last_modified: "2025-12-29T18:16:35.925474+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# GDPR DPIA 摘要 — Android SDK 遙測 (AND7)

|領域|價值|
|--------|--------|
|評估日期| 2026-02-12 |
|處理活動| Android SDK 遙測導出到共享 OTLP 後端 |
|控制器/處理器| SORA Nexus Ops（控制器）、合作夥伴操作員（聯合控制器）、Hyperledger Iroha 貢獻者（處理器）|
|參考文檔 | `docs/source/sdk/android/telemetry_redaction.md`、`docs/source/android_support_playbook.md`、`docs/source/android_runbook.md` |

## 1. 處理說明

- **目的：** 提供支持 AND7 可觀察性（延遲、重試、證明運行狀況）所需的操作遙測，同時鏡像 Rust 節點模式（`telemetry_redaction.md` 的第 2 節）。
- **系統：** Android SDK 工具 -> OTLP 導出器 -> 由 SRE 管理的共享遙測收集器（請參閱支持手冊第 8 節）。
- **數據主體：** 使用基於 Android SDK 的應用程序的運營商工作人員；下游 Torii 端點（權限字符串根據遙測策略進行哈希處理）。

## 2. 數據清單和緩解措施

|頻道|領域 | PII 風險 |緩解措施 |保留|
|--------|--------|----------|------------|------------|
|跡線（`android.torii.http.request`、`android.torii.http.retry`）|路由、狀態、延遲 |低（無 PII）|使用 Blake2b + 旋轉鹽進行哈希處理的權限；沒有導出有效負載主體。 | 7-30 天（每個遙測文檔）。 |
|事件 (`android.keystore.attestation.result`) |別名標籤、安全級別、認證摘要 |中等（運營數據）|別名散列 (`alias_label`)、`actor_role_masked` 記錄用於使用 Norito 審核令牌進行覆蓋。 | 90 天成功，365 天覆蓋/失敗。 |
|指標（`android.pending_queue.depth`、`android.telemetry.export.status`）|隊列計數、導出器狀態 |低|僅匯總計數。 | 90 天。 |
|設備輪廓儀表 (`android.telemetry.device_profile`) | SDK專業，硬件層|中等|分桶（模擬器/消費者/企業），無 OEM 或序列號。 | 30 天。 |
|網絡上下文事件 (`android.telemetry.network_context`) |網絡類型、漫游標志|中等|運營商名稱完全被刪除；支持合規性要求以避免訂閱者數據。 | 7天。 |

## 3. 合法依據和權利

- **法律依據：** 合法利益（第 6(1)(f) 條）——確保受監管賬本客戶的可靠運行。
- **必要性測試：** 僅限於運行狀況的指標（無用戶內容）；散列權限通過僅授權支持人員可用的可逆映射（通過覆蓋工作流程）確保與 Rust 節點的奇偶性。
- **平衡測試：** 遙測的範圍是操作員控制的設備，而不是最終用戶數據。覆蓋需要由支持 + 合規部門（支持手冊 §3 + §9）審核的已簽名 Norito 工件。
- **數據主體權利：** 操作員聯繫支持工程（劇本§2）請求遙測數據導出/刪除。密文覆蓋和日誌（遙測文檔§信號清單）可在 30 天內實現。

## 4. 風險評估|風險|可能性|影響 |殘留緩解|
|------|------------|--------|----------|
|通過哈希權威重新識別 |低|中等|通過`android.telemetry.redaction.salt_version`記錄鹽的旋轉；儲存在安全保險庫中的鹽；覆蓋季度審計。 |
|通過配置文件存儲桶進行設備指紋識別 |低|中等|僅導出tier+SDK專業；支持 Playbook 禁止 OEM/串行數據的升級請求。 |
|覆蓋濫用 PII 洩露 |極低 |高|已記錄 Norito 覆蓋請求，24 小時內過期，需要 SRE 批准（`docs/source/android_runbook.md` §3）。 |
|歐盟以外的遙測存儲 |中等|中等|收集器部署在歐盟+日本地區；通過 OTLP 後端配置強制執行保留策略（記錄在支持手冊第 8 節中）。 |

鑑於上述控制和持續監控，剩餘風險被認為是可以接受的。

## 5. 行動和後續行動

1. **季度審查：** 驗證遙測模式、鹽輪換和覆蓋日誌；文檔位於 `docs/source/sdk/android/telemetry_redaction_minutes_YYYYMMDD.md`。
2. **跨 SDK 對齊：** 與 Swift/JS 維護者協調，以維護一致的哈希/分桶規則（在路線圖 AND7 中跟踪）。
3. **合作夥伴通信：** 在合作夥伴入門套件中包含 DPIA 摘要（支持手冊第 9 節）並從 `status.md` 鏈接到此文檔。