---
lang: zh-hant
direction: ltr
source: docs/portal/docs/sns/suffix-catalog.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ffd062b69b97f11e5baa0ae82256c87cb76600982d599e1953573c1944112f51
source_last_modified: "2026-01-22T16:26:46.520176+00:00"
translation_last_reviewed: 2026-02-07
title: Sora Name Service Suffix Catalog
sidebar_label: Suffix catalog
description: Canonical allowlist of SNS suffixes, stewards, and pricing knobs for `.sora`, `.nexus`, and `.dao`.
translator: machine-google-reviewed
---

# Sora 名稱服務後綴目錄

SNS 路線圖跟踪每個批准的後綴 (SN-1/SN-2)。此頁面反映了
真實來源目錄，以便運營商運行註冊商、DNS 網關或錢包
工具可以加載相同的參數，而無需抓取狀態文檔。

- **快照：** [`docs/examples/sns/suffix_catalog_v1.json`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/examples/sns/suffix_catalog_v1.json)
- **消費者：** `iroha sns policy`、SNS 入門套件、KPI 儀表板和
  DNS/網關發布腳本都讀取相同的 JSON 包。
- **狀態：** `active`（允許註冊）、`paused`（暫時關閉）、
  `revoked`（已發布，但當前不可用）。

## 目錄架構

|領域|類型 |描述 |
|--------|------|-------------|
| `suffix` |字符串|帶前導點的人類可讀後綴。 |
| `suffix_id` | `u16` |標識符存儲在賬本上的 `SuffixPolicyV1::suffix_id` 中。 |
| `status` |枚舉 | `active`、`paused` 或 `revoked` 描述啟動準備情況。 |
| `steward_account` |字符串|負責管理的帳戶（匹配註冊商政策掛鉤）。 |
| `fund_splitter_account` |字符串|根據 `fee_split` 在路由之前接收付款的帳戶。 |
| `payment_asset_id` |字符串|用於結算的資產（初始隊列為 `61CtjvNd9T3THAR65GsMVHr82Bjc`）。 |
| `min_term_years` / `max_term_years` |整數 |購買保單的期限限制。 |
| `grace_period_days` / `redemption_period_days` |整數 |由 Torii 強制執行的更新安全窗口。 |
| `referral_cap_bps` |整數 |治理允許的最大推薦剝離（基點）。 |
| `reserved_labels` |數組|受治理保護的標籤對象 `{label, assigned_to, release_at_ms, note}`。 |
| `pricing` |數組|具有 `label_regex`、`base_price`、`auction_kind` 和持續時間界限的層對象。 |
| `fee_split` |對象| `{treasury_bps, steward_bps, referral_max_bps, escrow_bps}` 基點分割。 |
| `policy_version` |整數 |每當治理編輯策略時，單調計數器就會遞增。 |

## 當前目錄

|後綴 | ID (`hex`) |管家|資金分割 |狀態 |支付資產|推薦上限 (bps) |期限（最短 – 最長年）|恩典/救贖（天）|定價等級（正則表達式 → 基本價格/拍賣）|保留標籤|費用分割（T/S/R/E bps）|政策版本|
|--------|------------|---------|-------------|--------|----------------|--------------------|--------------------------|----------------------------|------------------------------------------------------------|-----------------|----------------------------------------|----------------|
| `.sora` | `0x0001` | `i105...` | `i105...` |活躍| `61CtjvNd9T3THAR65GsMVHr82Bjc` | 500 | 1 – 5 | 30 / 60 | `T0: ^[a-z0-9]{3,}$ → 120 XOR (Vickrey)` | `treasury → i105...` | `7000 / 3000 / 1000 / 0` | 1 |
| `.nexus` | `0x0002` | `i105...` | `i105...` |暫停| `61CtjvNd9T3THAR65GsMVHr82Bjc` | 300 | 300 1 – 3 | 15 / 30 | `T0: ^[a-z0-9]{4,}$ → 480 XOR (Vickrey)`<br>`T1: ^[a-z]{2}$ → 4000 XOR (Dutch floor 500)` | `treasury → i105...`、`guardian → i105...` | `6500 / 2500 / 800 / 200` | 2 |
| `.dao` | `0x0003` | `i105...` | `i105...` |撤銷| `61CtjvNd9T3THAR65GsMVHr82Bjc` | 0 | 1 – 2 | 30 / 30 | `T0: ^[a-z0-9]{3,}$ → 60 XOR (Vickrey)` | `dao (held for future release)` | `9000 / 1000 / 0 / 0` | 0 |

## JSON 摘錄

```json
{
  "version": 1,
  "generated_at": "2026-05-01T00:00:00Z",
  "suffixes": [
    {
      "suffix": ".sora",
      "suffix_id": 1,
      "status": "active",
      "fund_splitter_account": "i105...",
      "payment_asset_id": "61CtjvNd9T3THAR65GsMVHr82Bjc",
      "referral_cap_bps": 500,
      "pricing": [
        {
          "tier_id": 0,
          "label_regex": "^[a-z0-9]{3,}$",
          "base_price": {"asset_id": "61CtjvNd9T3THAR65GsMVHr82Bjc", "amount": 120},
          "auction_kind": "vickrey_commit_reveal",
          "min_duration_years": 1,
          "max_duration_years": 5
        }
      ],
      "...": "see docs/examples/sns/suffix_catalog_v1.json for the full record"
    }
  ]
}
```

## 自動化筆記

1. 加載 JSON 快照並對其進行哈希/簽名，然後再分發給操作員。
2. 註冊商工具應顯示 `suffix_id`、期限限制和定價
   每當請求到達 `/v1/sns/*` 時，就會從目錄中獲取。
3. DNS/網關助手在生成 GAR 時讀取保留的標籤元數據
   模板，以便 DNS 響應與治理控制保持一致。
4. KPI 附件作業標記儀表板導出並帶有後綴元數據，以便警報與
   此處記錄啟動狀態。