---
lang: zh-hant
direction: ltr
source: docs/source/governance_pipeline.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f9f765fbe3170f654a9c44c3cd1afc5d82a72ff49137f32b98cf9d310faf114e
source_last_modified: "2025-12-29T18:16:35.963528+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% 治理管道（Iroha 2 和 SORA 議會）

# 當前狀態 (v1)
- 治理提案的運行順序為：提案者→公投→計票→頒布。公投窗口和投票率/批准閾值按照 `gov.md` 中所述執行；鎖只能延長並在到期時解鎖。
- 議會選舉使用基於 VRF 的抽籤，具有確定性排序和任期限制；當不存在持久名冊時，Torii 使用 `gov.parliament_*` 配置派生回退。在 `gov_parliament_bodies` / `gov_pipeline_sla` 測試中執行理事會門控和仲裁檢查。
- 投票模式：ZK（默認，需要帶內聯字節的 `Active` VK）和 Plain（二次權重）。模式不匹配被拒絕；鎖創建/擴展在兩種模式下都是單調的，具有 ZK 和普通重新投票的回歸測試。
- 驗證者的不當行為通過證據管道（`/v1/sumeragi/evidence*`，CLI 助手）進行處理，並由 `NextMode` + `ModeActivationHeight` 強制執行聯合共識交接。
- 受保護的命名空間、運行時升級掛鉤和治理清單准入記錄在 `governance_api.md` 中，並由遙測覆蓋（`governance_manifest_*`、`governance_protected_namespace_total`）。

# 飛行中/積壓訂單
- 發布 VRF 抽籤工件（種子、證明、有序名單、替補）並製定缺席替換規則；為抽籤和替換添加黃金賽程。
- 議會機構的階段 SLA 執行（規則 → 議程 → 研究 → 審查 → 陪審團 → 頒布）需要明確的計時器、升級路徑和遙測計數器。
- 政策陪審團秘密/承諾披露投票和相關的反賄賂審計仍有待實施。
- 角色紐帶乘數、高風險機構的不當行為削減以及服務時段之間的冷卻需要配置管道和測試。
- 在 `gov.md`/`status.md` 中跟踪治理車道密封和全民投票窗口/道岔門；隨著剩餘的驗收測試的進行，保持路線圖條目的更新。