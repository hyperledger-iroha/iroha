---
lang: zh-hant
direction: ltr
source: docs/source/jdg_attestations.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 459e8ed4612da7cfa68053e4e299b2f68e7620d4f3b98a8a721ebf8327829ea1
source_last_modified: "2026-01-08T21:57:18.412403+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# JDG 證明：保護、輪換和保留

本說明記錄了現在在 `iroha_core` 中提供的 v1 JDG 證明防護。

- **委員會清單：** Norito 編碼的 `JdgCommitteeManifest` 捆綁包攜帶每個數據空間輪換
  計劃（`committee_id`、有序成員、閾值、`activation_height`、`retire_height`）。
  清單加載了 `JdgCommitteeSchedule::from_path` 並強制嚴格遞增
  退休/激活之間具有可選寬限重疊 (`grace_blocks`) 的激活高度
  委員會。
- **證明保護：** `JdgAttestationGuard` 強制執行數據空間綁定、過期、陳舊邊界，
  委員會 ID/閾值匹配、簽名者成員資格、支持的簽名方案以及可選
  通過 `JdgSdnEnforcer` 進行 SDN 驗證。大小上限、最大延遲和允許的簽名方案是
  構造函數參數； `validate(attestation, dataspace, current_height)` 返回活動的
  委員會或結構性錯誤。
  - `scheme_id = 1` (`simple_threshold`)：每個簽名者簽名，可選簽名者位圖。
  - `scheme_id = 2` (`bls_normal_aggregate`)：單個預聚合的 BLS-正常簽名
    證明哈希；簽名者位圖可選，默認為證明中的所有簽名者。勞工統計局
    聚合驗證需要清單中每個委員會成員的有效 PoP；缺失或
    無效的 PoP 拒絕證明。
  通過 `governance.jdg_signature_schemes` 配置允許列表。
- **保留存儲：** `JdgAttestationStore` 使用可配置的跟踪每個數據空間的證明
  每個數據空間上限，在插入時修剪最舊的條目。致電 `for_dataspace` 或
  `for_dataspace_and_epoch` 用於檢索審核/重播包。
- **測試：** 單位覆蓋範圍現在執行有效的委員會選擇、未知簽名者拒絕、陳舊
  證明拒絕、不支持的方案 ID 和保留修剪。參見
  `crates/iroha_core/src/jurisdiction.rs`。

警衛拒絕配置的允許列表之外的方案。