---
id: payment-settlement-plan
lang: ja
direction: ltr
source: docs/portal/docs/sns/payment-settlement-plan.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# SNS 支払い・決済プラン

> 正規ソース: [`docs/source/sns/payment_settlement_plan.md`](../../../source/sns/payment_settlement_plan.md).

ロードマップタスク **SN-5 -- Payment & Settlement Service** は、Sora Name
Service のための決定的な支払いレイヤーを導入します。すべての登録、
更新、返金は構造化された Norito payload を発行し、トレジャリー、
steward、ガバナンスがスプレッドシートなしで金融フローを再生できる
ようにする必要があります。このページはポータル向けに仕様を要約
しています。

## 収益モデル

- 基本料金 (`gross_fee`) は registrar の価格マトリクスから導出されます。
- トレジャリーは `gross_fee x 0.70` を受け取り、steward は残額から
  referral ボーナス (上限 10 %) を差し引いた分を受け取ります。
- オプションの holdbacks により、ガバナンスは紛争時に steward への
  支払いを一時停止できます。
- settlement バンドルは `ledger_projection` ブロックと具体的な `Transfer`
  ISI を公開し、オートメーションが XOR の移動を Torii に直接投稿
  できるようにします。

## サービスとオートメーション

| コンポーネント | 目的 | 証拠 |
|----------------|------|------|
| `sns_settlementd` | ポリシー適用、バンドル署名、`/v2/sns/settlements` の公開。 | JSON バンドル + hash. |
| Settlement queue & writer | idempotent キュー + `iroha_cli app sns settlement ledger` による ledger 提出。 | bundle hash <-> tx hash manifest. |
| Reconciliation job | 日次 diff + 月次明細 (`docs/source/sns/reports/` 配下)。 | Markdown + JSON digest. |
| Refund desk | `/settlements/{id}/refund` 経由のガバナンス承認返金。 | `RefundRecordV1` + ticket. |

CI ヘルパーはこれらのフローを反映します:

```bash
# Quote & ledger projection
iroha_cli app sns settlement quote --selector makoto.sora --term-years 1 --pricing hot-tier-a

# Emit transfers for automation/pipeline
iroha_cli app sns settlement ledger --bundle artifacts/sns/settlements/2026-05/makoto.sora.json

# Produce a reconciliation statement
iroha_cli app sns settlement reconcile --period 2026-05 --out docs/source/sns/reports/settlement_202605.md
```

## 観測とレポート

- ダッシュボード: `dashboards/grafana/sns_payment_settlement.json` は
  トレジャリー vs steward の合計、referral 支払い、キュー深度、返金
  レイテンシを可視化します。
- アラート: `dashboards/alerts/sns_payment_settlement_rules.yml` は pending
  年齢、照合失敗、ledger ドリフトを監視します。
- 明細: 日次 digest (`settlement_YYYYMMDD.{json,md}`) が月次レポート
  (`settlement_YYYYMM.md`) に集約され、Git とガバナンスのオブジェクト
  ストア (`s3://sora-governance/sns/settlements/<period>/`) にアップロード
  されます。
- ガバナンスパケットはダッシュボード、CLI ログ、承認をまとめて、
  council のサインオフ前に提出します。

## ロールアウトチェックリスト

1. quote + ledger ヘルパーをプロトタイプ化し、staging バンドルを取得。
2. queue + writer を備えた `sns_settlementd` を起動し、ダッシュボードを
   配線してアラートテスト (`promtool test rules ...`) を実行。
3. 返金ヘルパーと月次明細テンプレートを提供し、アーティファクトを
   `docs/portal/docs/sns/reports/` にミラーする。
4. パートナーリハーサル (1 か月分の settlements) を実施し、SN-5 完了
   を示すガバナンス投票を記録。

正確なスキーマ定義、未解決の質問、将来の改訂についてはソース文書を
参照してください。
