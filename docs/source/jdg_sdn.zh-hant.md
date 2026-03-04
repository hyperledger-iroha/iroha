---
lang: zh-hant
direction: ltr
source: docs/source/jdg_sdn.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1ee87ee60e2e8c9d9636b282231b33de3cf1fd7240c8d31d0a0a1673651dcef1
source_last_modified: "2025-12-29T18:16:35.972838+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% JDG-SDN 證明和輪換

本說明捕獲了秘密數據節點 (SDN) 證明的執行模型
由司法管轄區數據監護人 (JDG) 流程使用。

## 承諾格式
- `JdgSdnCommitment` 綁定範圍（`JdgAttestationScope`），加密
  有效負載哈希和 SDN 公鑰。印章是打字簽名
  (`SignatureOf<JdgSdnCommitmentSignable>`) 通過域標記的有效負載
  `iroha:jurisdiction:sdn:commitment:v1\x00 || norito(signable)`。
- 結構驗證 (`validate_basic`) 強制執行：
  - `version == JDG_SDN_COMMITMENT_VERSION_V1`
  - 有效的塊範圍
  - 非空密封件
  - 通過運行時與證明的範圍相等
    `JdgAttestation::validate_with_sdn`/`validate_with_sdn_registry`
- 重複數據刪除由證明驗證器處理（簽名者+有效負載哈希
  唯一性）以防止隱瞞/重複承諾。

## 註冊和輪換政策
- SDN 密鑰位於 `JdgSdnRegistry` 中，由 `(Algorithm, public_key_bytes)` 加密。
- `JdgSdnKeyRecord`記錄激活高度，可選退役高度，
  和可選的父密鑰。
- 旋轉由 `JdgSdnRotationPolicy` 控制（當前：`dual_publish_blocks`
  重疊窗口）。註冊子密鑰會將父密鑰更新為
  `child.activation + dual_publish_blocks`，帶護欄：
  - 失踪的父母被拒絕
  - 激活必須嚴格增加
  - 超過寬限窗口的重疊被拒絕
- 註冊表助手顯示已安裝記錄（`record`、`keys`）以了解狀態
  和 API 暴露。

## 驗證流程
- `JdgAttestation::validate_with_sdn_registry` 包含結構
  認證檢查和 SDN 實施。 `JdgSdnPolicy` 線程：
  - `require_commitments`：強制存在 PII/秘密有效負載
  - `rotation`：更新父退休時使用的寬限期
- 每項承諾均經過檢查：
  - 結構有效性+證明範圍匹配
  - 註冊關鍵存在
  - 活動窗口覆蓋已證明的區塊範圍（退休範圍已經
    包括雙重發布寬限期）
  - 域名標記承諾主體上的有效印章
- 穩定誤差顯示操作員證據索引：
  `MissingSdnCommitments`、`UnknownSdnKey`、`InactiveSdnKey`、`InvalidSeal`、
  或結構性 `Commitment`/`ScopeMismatch` 故障。

## 操作手冊
- **規定：** 在以下時間或之前使用 `activated_at` 註冊第一個 SDN 密鑰
  第一個秘密區塊高度。將密鑰指紋發布給 JDG 運營商。
- **旋轉：**生成後繼密鑰，用 `rotation_parent` 註冊
  指向當前鍵，並確認父退休等於
  `child_activation + dual_publish_blocks`。重新密封有效負載承諾
  重疊窗口期間的活動鍵。
- **審核：**通過 Torii/status 公開註冊表快照（`record`、`keys`）
  以便審計員可以確認活動密鑰和報廢窗口。警報
  如果證明的範圍落在活動窗口之外。
- **恢復：** `UnknownSdnKey` → 確保註冊表包含密封密鑰；
  `InactiveSdnKey` → 旋轉或調整激活高度； `InvalidSeal` →
  重新密封有效負載並刷新證明。## 運行時助手
- `JdgSdnEnforcer` (`crates/iroha_core/src/jurisdiction.rs`) 套餐保單 +
  通過 `validate_with_sdn_registry` 註冊並驗證證明。
- 可以從 Norito 編碼的 `JdgSdnKeyRecord` 捆綁包加載註冊表（請參閱
  `JdgSdnEnforcer::from_reader`/`from_path`) 或組裝
  `from_records`，在註冊過程中應用旋轉護欄。
- 運營商可以保留 Norito 捆綁包作為 Torii/狀態的證據
  浮出水面，同時相同的有效負載為准入和使用的執行器提供信息
  共識守護者。單個全局執行器可以在啟動時通過以下方式初始化
  `init_enforcer_from_path` 和 `enforcer()`/`registry_snapshot()`/`sdn_registry_status()`
  公開狀態/Torii 表面的實時策略+關鍵記錄。

## 測試
- `crates/iroha_data_model/src/jurisdiction.rs` 中的回歸覆蓋率：
  `sdn_registry_accepts_active_commitment`，`sdn_registry_rejects_unknown_key`，
  `sdn_registry_rejects_inactive_key`、`sdn_registry_rejects_bad_signature`、
  `sdn_registry_sets_parent_retirement_window`，
  `sdn_registry_rejects_overlap_beyond_policy`，以及現有的
  結構證明/SDN 驗證測試。