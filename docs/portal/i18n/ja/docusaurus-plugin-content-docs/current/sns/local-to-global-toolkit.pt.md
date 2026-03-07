---
lang: ja
direction: ltr
source: docs/portal/docs/sns/local-to-global-toolkit.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Kit de enderecos ローカル -> グローバル

Esta pagina espelha `docs/source/sns/local_to_global_toolkit.md` はモノレポを行います。 CLI および Runbook のロードマップ **ADDR-5c** の OS ヘルパーを拡張します。

## ヴィサオ・ゲラル

- `scripts/address_local_toolkit.sh` の CLI `iroha` 製品のカプセル化:
  - `audit.json` -- `iroha tools address audit --format json` の説明。
  - `normalized.txt` -- literais IH58 (preferido) / 圧縮 (`sora`) (segunda melhor opcao) ローカルの変換セレクター。
- スクリプトとダッシュボードのエンジェストの結合 (`dashboards/grafana/address_ingest.json`)
  Local-8 / カットオーバーのプロバー キューとして Alertmanager (`dashboards/alerts/address_ingest_rules.yml`) を実行します。
  Local-12 e セグロ。 Local-8 および Local-12 の OS アラートを観察してください。
  `AddressLocal8Resurgence`、`AddressLocal12Collision`、`AddressInvalidRatioSlo` 事前
  推進者ムダンカス・デ・マニフェスト。
- [アドレス表示ガイドライン](address-display-guidelines.md) e o として参照してください。
  [アドレス マニフェスト ランブック](../../../source/runbooks/address_manifest_ops.md) インシデントの UX 応答に関するコンテキスト。

## うそ

```bash
scripts/address_local_toolkit.sh       --input fixtures/address/local_digest_examples.txt       --output-dir artifacts/address_migration       --network-prefix 753       --format ih58
```

オペコ:

- `--format compressed (`sora`)` は `sora...` と IH58 を比較しました。
- `domainless output (default)` パラエミミール・リタライス・セム・ドミニオ。
- `--audit-only` は、会話に関する質問です。
- `--allow-errors` は、引き続き不正な不正行為を実行します (CLI のイグアル アオ コンポルタメント)。

O スクリプトは、最終的な実行を実行するためのスクリプトを作成します。 Anexe os dois arquivos ao
ムダンカス ジュント コムのチケットを取得し、Grafana を実行してゼロを改善します
検出ローカル 8 e ゼロコリソローカル 12 por >=30 dias。

## インテグラカオ CI

1. スクリプトを実行して、その仕事に専念し、羨望の念を抱きます。
2. Bloqueie は、quando `audit.json` レポーター セレクター ローカル (`domain.kind = local12`) をマージします。
   勇気がありません `true` (つまり、`false` em クラスターの開発/テストの診断を変更します)
   regressoes) e アディシオーネ
   `iroha tools address normalize` CI パラケリグレッソ
   ファルヘム・アンテ・デ・シュガールは生産者です。

詳細な文書、証拠およびスニペットのチェックリストを確認する
リリース ノートは、クライアントの発表やカットオーバーの再利用に役立ちます。