---
lang: ja
direction: ltr
source: docs/portal/docs/sns/local-to-global-toolkit.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# ローカル -> グローバル

یہ صفحہ `docs/source/sns/local_to_global_toolkit.md` کا عکاس ہے۔ロードマップ **ADDR-5c** ٩ے لیے درکار CLI ヘルパー اور runbook الٹھے کرتا ہے۔

## やあ

- `scripts/address_local_toolkit.sh` `iroha` CLI はラップをラップします。
  - `audit.json` -- `iroha tools address audit --format json` 構造化出力
  - `normalized.txt` -- ローカル ドメイン セレクター IH58 (`sora`、2 番目に優れた) リテラル
- アドレス取り込みダッシュボード (`dashboards/grafana/address_ingest.json`)
  アラートマネージャー ルール (`dashboards/alerts/address_ingest_rules.yml`) のルール (`dashboards/alerts/address_ingest_rules.yml`)
  Local-8 / Local-12 のカットオーバーローカル-8 ローカル-12 衝突パネル
  `AddressLocal8Resurgence`、`AddressLocal12Collision`、`AddressInvalidRatioSlo` アラート
  マニフェスト تبدیلیاں プロモーション کرنے سے پہلے دیکھیں۔
- UX インシデント対応 [アドレス表示ガイドライン](address-display-guidelines.md)
  [アドレス マニフェスト ランブック](../../../source/runbooks/address_manifest_ops.md)

## ああ

```bash
scripts/address_local_toolkit.sh       --input fixtures/address/local_digest_examples.txt       --output-dir artifacts/address_migration       --network-prefix 753       --format ih58
```

回答:

- `--format compressed` IH58 の出力 `sora...` の出力
- `domainless output (default)` تاکہ 裸のリテラル نکلیں۔
- `--audit-only` 変換ステップ
- `--allow-errors` 不正な行行です (CLI 動作) (CLI 動作) (CLI 動作)

アーティファクト パスを作成するऔर देखें
変更管理チケット パスワード パスワード Grafana スクリーンショット パスワード パスワード
>=30 件 ローカル 8 件の検出 ローカル 12 件の衝突 件

## CI 認証

1. 専用ジョブの実行 出力の実行
2. `audit.json` ローカル セレクター (`domain.kind = local12`) はマージします。
   デフォルト `true` پر رکھیں (開発/テストの回帰 کی تشخیص کے وقت `false` کریں) ور
   `iroha tools address normalize` کو CI میں شامل کریں تاکہ
   回帰生産 تک پہنچنے سے پہلے فیل ہوں۔

証拠チェックリスト、リリース ノート スニペット、スニペット、リリース ノート スニペット、リリース ノート スニペット、リリース ノート スニペット、リリース ノート スニペット、リリース ノート スニペット、証拠チェックリスト
ビデオ カットオーバー ビデオ ジャック ビデオ ジャック ビデオ