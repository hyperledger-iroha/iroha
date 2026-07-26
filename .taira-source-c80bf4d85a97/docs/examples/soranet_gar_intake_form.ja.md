---
lang: ja
direction: ltr
source: docs/examples/soranet_gar_intake_form.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6cd4da7e590d581719ed2607994d7d9eb16d153fbd06f85655d0da37c727853a
source_last_modified: "2025-11-21T15:16:55.013762+00:00"
translation_last_reviewed: 2026-01-01
---

# SoraNet GAR 受付テンプレート

GAR アクション (purge, ttl override, rate ceiling, moderation directive, geofence, または legal hold) を依頼する際にこの受付フォームを使用する。
提出したフォームは `gar_controller` の出力と並べてピン留めし、監査ログとレシートが同じ証跡 URI を参照できるようにする。

| 項目 | 値 | 備考 |
|------|----|------|
| リクエスト ID |  | Guardian/ops のチケット ID. |
| 申請者 |  | アカウント + 連絡先. |
| 日時 (UTC) |  | 開始時刻. |
| GAR 名 |  | 例: `docs.sora`. |
| 正規ホスト |  | 例: `docs.gw.sora.net`. |
| アクション |  | `ttl_override` / `rate_limit_override` / `purge_static_zone` / `geo_fence` / `legal_hold` / `moderation`. |
| TTL override (秒) |  | `ttl_override` の場合のみ必須. |
| レート上限 (RPS) |  | `rate_limit_override` の場合のみ必須. |
| 許可リージョン |  | `geo_fence` を依頼する場合の ISO リージョン一覧. |
| 拒否リージョン |  | `geo_fence` を依頼する場合の ISO リージョン一覧. |
| モデレーション slug |  | GAR のモデレーション指示に一致させる. |
| パージタグ |  | 配信前にパージすべきタグ. |
| ラベル |  | マシンラベル (incident id, drill name, pop scope). |
| 証跡 URI |  | 申請を裏付けるログ/ダッシュボード/仕様. |
| 監査 URI |  | デフォルトと異なる場合の pop 別監査 URI. |
| 有効期限 |  | Unix タイムスタンプまたは RFC3339; デフォルトは空欄. |
| 理由 |  | ユーザー向け説明; レシートとダッシュボードに表示. |
| 承認者 |  | Guardian/委員会の承認者. |

### 提出手順

1. 表を埋め、ガバナンスチケットに添付する.
2. GAR controller の設定 (`policies`/`pops`) を更新し、`labels`/`evidence_uris`/`expires_at_unix` を一致させる.
3. `cargo xtask soranet-gar-controller ...` を実行してイベント/レシートを出力する.
4. `gar_controller_summary.json`, `gar_reconciliation_report.json`, `gar_metrics.prom`, `gar_audit_log.jsonl` を同じチケットに追加する。承認者は受領数が PoP リストと一致することを確認してから実行する.
