---
lang: ja
direction: ltr
source: docs/source/soranet_gateway_billing.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: fc115dfe974fd80c707df0fef676bf48dd6bf3f584c8be5b4f306ed470e9db57
source_last_modified: "2025-11-21T13:08:05.986629+00:00"
translation_last_reviewed: 2026-01-21
---

<!-- 日本語訳: docs/source/soranet_gateway_billing.md -->

# SoraGlobal ゲートウェイ課金 (SN15-M0-9)

このノートは、SoraGlobal Gateway CDN の M0 課金カタログを記述し、
使用量のレーティング、ガードレールの適用、台帳プロジェクション
の生成に必要なツールを提供する。特記のない限り、金額はすべて
**micro-XOR (1e-6 XOR)** で表記する。

## 正準アーティファクト
- メーターカタログ: `configs/soranet/gateway_m0/meter_catalog.json`
- ガードレール: `configs/soranet/gateway_m0/billing_guardrails.json`
- 使用量スナップショット例: `configs/soranet/gateway_m0/billing_usage_sample.json`
- 照合テンプレート: `docs/examples/soranet_gateway_billing/reconciliation_report_template.md`

## メーターカタログ
| メーター id | 単位 | ベース価格 (micro-XOR) | 地域倍率 (bps) | 割引階層 (bps @ しきい値) |
|-------------|------|------------------------|----------------|---------------------------|
| `http.request` | request | 5 | NA 10000, EU 9500, APAC 11000, LATAM 10250 | 500 @ 1,000,000; 900 @ 5,000,000 |
| `http.egress_gib` | gibibyte | 250000 | NA 10000, EU 9500, APAC 11000, LATAM 10250 | 300 @ 100; 1000 @ 300 |
| `dns.doh_query` | request | 3 | NA 10000, EU 9800, APAC 10500, LATAM 10000 | 1000 @ 2,000,000 |
| `waf.decision` | decision | 20 | GLOBAL 10000 | — |
| `car.verification_ms` | ms | 10 | GLOBAL 10000 | 1500 @ 500,000 |
| `storage.gib_month` | gib-month | 180000 | NA 10000, EU 9700, APAC 10300, LATAM 10000 | 500 @ 50; 1000 @ 200 |

注記:
- 地域倍率と割引は、単価ではなく行アイテム合計に適用する。高ボリュームでも
  決定論性を維持するためである。
- 地域が倍率マップに無い場合、デフォルトの `10000` (1.0x) を使用する。

## 請求ハーネス (`cargo xtask soranet-gateway-billing`)

使用量スナップショットに対してレーティングパイプラインを実行する:

```
cargo xtask soranet-gateway-billing \
  --usage configs/soranet/gateway_m0/billing_usage_sample.json \
  --catalog configs/soranet/gateway_m0/meter_catalog.json \
  --guardrails configs/soranet/gateway_m0/billing_guardrails.json \
  --payer ih58... \
  --treasury ih58... \
  --asset xor#wonderland \
  --out artifacts/soranet/gateway_billing
```

出力（すべて `--out` ディレクトリ配下）:
- `meter_catalog.json` — 実行で使用したカタログのコピー。
- `billing_usage_snapshot.json` — 正規化された使用量入力。
- `billing_invoice.json` — ガードレール判定を含む正準 Norito JSON 請求書。
- `billing_invoice.csv` — 行アイテムの表形式エクスポート。
- `billing_invoice.parquet` — BI 用の Parquet エクスポート（Arrow スキーマ）。
- `billing_ledger_projection.json` — XOR 台帳向けの `TransferAssetBatch`。
  payer/treasury の紐付けと合計請求額を含む。
- `billing_reconciliation_report.md` — テンプレートと実行メタデータから生成された
  事前入力済み照合/異議申し立てレポート。
- 請求書メタデータには `normalized_entries`（地域の大文字化と重複統合）と
  `merge_notes` が含まれ、照合ツールが使用量統合の差分を追える。

ガードレールの挙動:
- ソフト上限（デフォルト 140,000,000 micro-XOR）はアラートフラグを立てる。
  ハード上限（デフォルト 220,000,000 micro-XOR）は、呼び出し側が明示的に
  継続しない限り台帳プロジェクションをブロックする。
- 最低請求額（デフォルト 1,000,000 micro-XOR）は小額請求書の発行を防ぐ。
- 未知の地域は拒否される（カタログに地域倍率の列挙が必要）。
  メーター/地域の重複は地域名を大文字化して統合し、監査リプレイ用に
  請求書へ記録する。

## 異議・照合フロー
1. 使用量スナップショットをカタログ（メーター id、単位、地域）に対して検証する。
2. `billing_invoice.json` が CSV/Parquet エクスポートと一致することを確認する。
3. ガードレールのフラグを確認し、トリガーされた場合はアラート/override 証拠を添付する。
4. `billing_ledger_projection.json` の合計が請求書の `total_micros` と一致し、
   期待する payer/treasury アカウントおよび asset 定義を参照していることを確認する。
5. `billing_reconciliation_report.md` を記入する（または
   `docs/examples/soranet_gateway_billing/reconciliation_report_template.md` の
   テンプレートから開始し、証拠リンク、承認、要求調整を記載する）。

## レーティングと丸め規則
- 地域倍率と割引階層は basis points (bps) で表現し、行アイテム合計
  （数量 × ベース価格）に適用して micro-XOR 単位で四捨五入する。
- 割引階層は各行で適用可能なしきい値のうち最も高いものを選ぶ。
- 金額は micro-XOR で保存/出力され、台帳プロジェクションは最終合計を
  6 桁小数の `Numeric` に変換して transfer batch を生成する。
