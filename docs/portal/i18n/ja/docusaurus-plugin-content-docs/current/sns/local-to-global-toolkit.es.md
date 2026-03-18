---
lang: ja
direction: ltr
source: docs/portal/docs/sns/local-to-global-toolkit.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# キットの指示 ローカル -> グローバル

モノレポのページ `docs/source/sns/local_to_global_toolkit.md` を参照してください。 Empaqueta は、ロードマップ **ADDR-5c** の項目に関する CLI ランブックのヘルパーを必要としています。

## 履歴書

- `scripts/address_local_toolkit.sh` envuelve la CLI `iroha` パラ プロデューサー:
  - `audit.json` -- `iroha tools address audit --format json` の構造。
  - `normalized.txt` -- リテラル I105 (preferido) / 圧縮 (`sora`) (重要なオプション) ローカルの変換セレクター。
- ダッシュボードと指示の取り込みとスクリプトの組み合わせ (`dashboards/grafana/address_ingest.json`)
  Local-8 のカットオーバーに関する Alertmanager (`dashboards/alerts/address_ingest_rules.yml`) の規則
  Local-12 エスセグロ。ローカル-8 y ローカル-12 y の衝突を観察してください
  `AddressLocal8Resurgence`、`AddressLocal12Collision`、y `AddressInvalidRatioSlo` 前
  プロモーターのカンビオス・デ・マニフェスト。
- 参照先 [アドレス表示ガイドライン](address-display-guidelines.md) y el
  [アドレス マニフェスト ランブック](../../../source/runbooks/address_manifest_ops.md) インシデントの UX 応答に関するコンテキスト。

## うそ

```bash
scripts/address_local_toolkit.sh       --input fixtures/address/local_digest_examples.txt       --output-dir artifacts/address_migration       --network-prefix 753       --format i105
```

オプション:

- I105 の `--format I105` パラサリダ `sora...`。
- `domainless output (default)` パラエミミール リテラル シン ドミニオ。
- `--audit-only` 変換パラメータを省略します。
- `--allow-errors` エスカネアンド クアンド アパレスカン フィラス マルフォルマダを確認してください (CLI のコンポルタミエントと一致)。

このスクリプトは、最終的な射出の成果物を記述します。アジュンタ・アンボス・アルキヴォス
チケットのカンビオス ジュント コンエル Grafana que pruebe cero のスクリーンショット
検出ローカル 8 y cero 衝突ローカル 12、直径 30 以上。

## 統合 CI

1. エジェクタ エル スクリプト en un un job dedicado y sube sus salidas.
2. Bloquea は、cuando `audit.json` レポート セレクター ローカル (`domain.kind = local12`) をマージします。
   欠陥のある `true` の有効性 (クラスターの開発/テストで `false` を単独でオーバーライドする)
   診断的回帰) と集合体
   `iroha tools address normalize` CI パラケインテント
   回帰は、生産上の危険にさらされています。

詳細な文書、証拠およびスニペットのチェックリストを参照してください。
リリース ノートは、クライアントのカットオーバーに関するお知らせとして再利用されます。