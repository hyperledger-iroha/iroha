---
lang: ja
direction: ltr
source: docs/portal/docs/sns/suffix-catalog.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d23c9d6a77942b5918b933631b890addbc0ecdfef51e9ff427a4069d2cc37902
source_last_modified: "2025-11-15T16:27:31.089720+00:00"
translation_last_reviewed: 2026-01-01
---

# Sora Name Service サフィックス・カタログ

SNS のロードマップは承認済みサフィックス (SN-1/SN-2) をすべて追跡します。
このページは正本カタログを反映しており、registrar、DNS ゲートウェイ、
ウォレットツールを運用するオペレーターが、ステータス文書をスクレイピング
せずに同一パラメータを読み込めます。

- **Snapshot:** [`docs/examples/sns/suffix_catalog_v1.json`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/examples/sns/suffix_catalog_v1.json)
- **Consumers:** `iroha sns policy`、SNS オンボーディングキット、KPI ダッシュボード、
  DNS/Gateway リリーススクリプトはいずれも同じ JSON バンドルを読みます。
- **Statuses:** `active` (登録可能)、`paused` (一時的にゲート)、`revoked` (告知済みだが現在は利用不可)。

## カタログスキーマ

| フィールド | 型 | 説明 |
|-----------|----|------|
| `suffix` | string | 先頭にドットが付いた人間可読のサフィックス。 |
| `suffix_id` | `u16` | `SuffixPolicyV1::suffix_id` に保存される台帳上の識別子。 |
| `status` | enum | `active`, `paused`, `revoked` で起動準備状況を表す。 |
| `steward_account` | string | stewardship を担当するアカウント (registrar ポリシーフックに一致)。 |
| `fund_splitter_account` | string | `fee_split` に従ってルーティングする前に支払いを受け取るアカウント。 |
| `payment_asset_id` | string | 決済に使うアセット (初期コホートは `61CtjvNd9T3THAR65GsMVHr82Bjc`)。 |
| `min_term_years` / `max_term_years` | integer | ポリシーからの購入期間の上限/下限。 |
| `grace_period_days` / `redemption_period_days` | integer | Torii が適用する更新安全ウィンドウ。 |
| `referral_cap_bps` | integer | ガバナンスで許可される referral carve-out の上限 (basis points)。 |
| `reserved_labels` | array | ガバナンス保護ラベルオブジェクト `{label, assigned_to, release_at_ms, note}`。 |
| `pricing` | array | `label_regex`, `base_price`, `auction_kind` と期間上限/下限を持つ tier オブジェクト。 |
| `fee_split` | object | `{treasury_bps, steward_bps, referral_max_bps, escrow_bps}` の basis point 分割。 |
| `policy_version` | integer | ガバナンスがポリシーを編集するたびに増える単調カウンタ。 |

## 現行カタログ

| サフィックス | ID (`hex`) | Steward | Fund splitter | 状態 | 支払いアセット | Referral 上限 (bps) | 期間 (min - max 年) | Grace / Redemption (日) | 価格 tier (regex -> 基本価格 / オークション) | 予約ラベル | Fee split (T/S/R/E bps) | ポリシーバージョン |
|--------------|------------|---------|---------------|------|---------------|---------------------|----------------------|--------------------------|---------------------------------------------|-----------|-------------------------|--------------------|
| `.sora` | `0x0001` | `<katakana-i105-account-id>` | `<katakana-i105-account-id>` | 稼働中 | `61CtjvNd9T3THAR65GsMVHr82Bjc` | 500 | 1-5 | 30 / 60 | `T0: ^[a-z0-9]{3,}$ -> 120 XOR (Vickrey)` | `treasury -> <katakana-i105-account-id>` | `7000 / 3000 / 1000 / 0` | 1 |
| `.nexus` | `0x0002` | `<katakana-i105-account-id>` | `<katakana-i105-account-id>` | 一時停止 | `61CtjvNd9T3THAR65GsMVHr82Bjc` | 300 | 1-3 | 15 / 30 | `T0: ^[a-z0-9]{4,}$ -> 480 XOR (Vickrey)`<br>`T1: ^[a-z]{2}$ -> 4000 XOR (Dutch floor 500)` | `treasury -> <katakana-i105-account-id>`, `guardian -> <katakana-i105-account-id>` | `6500 / 2500 / 800 / 200` | 2 |
| `.dao` | `0x0003` | `<katakana-i105-account-id>` | `<katakana-i105-account-id>` | 廃止 | `61CtjvNd9T3THAR65GsMVHr82Bjc` | 0 | 1-2 | 30 / 30 | `T0: ^[a-z0-9]{3,}$ -> 60 XOR (Vickrey)` | `dao (held for future release)` | `9000 / 1000 / 0 / 0` | 0 |

## JSON 抜粋

```json
{
  "version": 1,
  "generated_at": "2026-05-01T00:00:00Z",
  "suffixes": [
    {
      "suffix": ".sora",
      "suffix_id": 1,
      "status": "active",
      "fund_splitter_account": "<katakana-i105-account-id>",
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

## 自動化メモ

1. JSON snapshot を読み込み、hash/署名してからオペレーターに配布します。
2. registrar tooling は `/v1/sns/*` にリクエストが来たら、`suffix_id`、期間制限、
   価格をカタログから提示するべきです。
3. DNS/Gateway ヘルパーは GAR テンプレート生成時に予約ラベルのメタデータを読み、
   DNS 応答がガバナンス制御と整合するようにします。
4. KPI annex ジョブはダッシュボードのエクスポートにサフィックスのメタデータを
   付与し、アラートがここで記録されたローンチ状態と一致するようにします。
