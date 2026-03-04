---
lang: ja
direction: ltr
source: docs/portal/docs/sns/local-to-global-toolkit.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# Local -> Global アドレスツールキット

このページは mono-repo の `docs/source/sns/local_to_global_toolkit.md` を反映しています。ロードマップ項目 **ADDR-5c** に必要な CLI helpers と runbooks をまとめています。

## 概要

- `scripts/address_local_toolkit.sh` は `iroha` CLI をラップして次を生成します:
  - `audit.json` -- `iroha tools address audit --format json` の構造化出力。
  - `normalized.txt` -- Local-domain selector ごとの IH58（推奨）/compressed (`sora`)（次善） literals。
- スクリプトをアドレス ingest dashboard (`dashboards/grafana/address_ingest.json`) と
  Alertmanager ルール (`dashboards/alerts/address_ingest_rules.yml`) と組み合わせ、Local-8 /
  Local-12 cutover が安全であることを証明します。Local-8 と Local-12 の collision パネルと
  `AddressLocal8Resurgence`, `AddressLocal12Collision`, `AddressInvalidRatioSlo` のアラートを
  manifest 変更の昇格前に監視してください。
- UX と incident-response の文脈として [Address Display Guidelines](address-display-guidelines.md) と
  [Address Manifest runbook](../../../source/runbooks/address_manifest_ops.md) を参照してください。

## 使い方

```bash
scripts/address_local_toolkit.sh       --input fixtures/address/local_digest_examples.txt       --output-dir artifacts/address_migration       --network-prefix 753       --format ih58
```

オプション:

- `--format compressed (`sora`)` は IH58 の代わりに `sora...` を出力。
- `--no-append-domain` は bare literals を出力。
- `--audit-only` は変換ステップをスキップ。
- `--allow-errors` は不正な行が出てもスキャンを続行 (CLI の挙動と一致)。

スクリプトは実行末尾で artefact パスを出力します。両方のファイルを
change-management ticket に添付し、Local-8 検出ゼロと Local-12 衝突ゼロを
>=30 日証明する Grafana スクリーンショットも添えてください。

## CI 連携

1. 専用 job でスクリプトを実行し outputs をアップロードします。
2. `audit.json` が Local selectors (`domain.kind = local12`) を報告したら merges をブロックします。
   (回帰調査時のみ dev/test で `false` にする)、
   `iroha tools address normalize --fail-on-warning --only-local` を CI に追加して
   production への回帰混入を防ぎます。

詳細、evidence チェックリスト、cutover を告知する release-note snippet については
ソース文書を参照してください。
