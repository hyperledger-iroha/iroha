---
lang: he
direction: rtl
source: docs/portal/i18n/ja/docusaurus-plugin-content-docs/current/devportal/preview-host-exposure.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3b851abfdfc78c43c207586ebac3fc0e9f52456954207b71d5f5bd2f7e0863bd
source_last_modified: "2025-11-14T04:43:19.994499+00:00"
translation_last_reviewed: 2026-01-30
---

# プレビュー ホスト公開ガイド

DOCS-SORA ロードマップでは、すべての公開プレビューが、レビュー担当者がローカルで検証するのと同じチェックサム検証済みバンドルに乗ることを求めています。レビュー担当者のオンボーディング（および招待承認チケット）が完了した後、この runbook を使ってベータ プレビュー ホストを公開してください。

## 前提条件

- レビュー担当者オンボーディングのウェーブが承認され、プレビュー トラッカーに記録済み。
- 最新のポータルビルドが `docs/portal/build/` に存在し、チェックサムが検証済み（`build/checksums.sha256`）。
- SoraFS プレビュー認証情報（Torii URL、authority、秘密鍵、提出済み epoch）が環境変数または JSON 設定（例: [`docs/examples/sorafs_preview_publish.json`](../../../examples/sorafs_preview_publish.json)）に保存されている。
- 望ましいホスト名（`docs-preview.sora.link`、`docs.iroha.tech` など）とオンコール連絡先を含む DNS 変更チケットがオープン。

## ステップ1 - バンドルのビルドと検証

```bash
cd docs/portal
export DOCS_RELEASE_TAG="preview-$(date -u +%Y%m%dT%H%M%SZ)"
npm ci
npm run build
./scripts/preview_verify.sh --build-dir build
```

検証スクリプトは、チェックサム マニフェストが欠落または改ざんされている場合に継続を拒否し、すべてのプレビュー成果物を監査可能に保ちます。

## ステップ2 - SoraFS 成果物のパッケージ化

静的サイトを決定的な CAR/manifest ペアに変換します。`ARTIFACT_DIR` のデフォルトは `docs/portal/artifacts/` です。

```bash
./scripts/sorafs-pin-release.sh       --alias docs-preview.sora       --alias-namespace docs       --alias-name preview       --pin-label docs-preview       --skip-submit

node scripts/generate-preview-descriptor.mjs       --manifest artifacts/checksums.sha256       --archive artifacts/sorafs/portal.tar.gz       --out artifacts/sorafs/preview-descriptor.json
```

生成された `portal.car`、`portal.manifest.*`、descriptor、checksum マニフェストをプレビュー ウェーブ チケットに添付します。

## ステップ3 - プレビュー alias の公開

ホストを公開する準備ができたら、`--skip-submit` **なし**で pin ヘルパーを再実行します。JSON 設定または明示的な CLI フラグを指定してください:

```bash
./scripts/sorafs-pin-release.sh       --alias docs-preview.sora       --alias-namespace docs       --alias-name preview       --pin-label docs-preview       --config ~/secrets/sorafs_preview_publish.json
```

このコマンドは `portal.pin.report.json`、`portal.manifest.submit.summary.json`、`portal.submit.response.json` を出力します。これらは招待証拠バンドルに含める必要があります。

## ステップ4 - DNS 切替計画の生成

```bash
node scripts/generate-dns-cutover-plan.mjs       --dns-hostname docs.iroha.tech       --dns-zone sora.link       --dns-change-ticket DOCS-SORA-Preview       --dns-cutover-window "2026-03-05 18:00Z"       --dns-ops-contact "pagerduty:sre-docs"       --manifest artifacts/sorafs/portal.manifest.to       --cache-purge-endpoint https://cache.api/purge       --cache-purge-auth-env CACHE_PURGE_TOKEN       --out artifacts/sorafs/portal.dns-cutover.json
```

生成された JSON を Ops と共有し、DNS 切替が正確な manifest digest を参照するようにします。以前の descriptor をロールバック元として再利用する場合は、`--previous-dns-plan path/to/previous.json` を追加してください。

## ステップ5 - 公開済みホストのプローブ

```bash
npm run probe:portal --       --base-url=https://docs-preview.sora.link       --expect-release="$DOCS_RELEASE_TAG"
```

プローブは配信中の release タグ、CSP ヘッダー、署名メタデータを確認します。2 つのリージョンからコマンドを再実行するか、curl 出力を添付して、エッジキャッシュが温まっていることを監査担当者に示してください。

## 証拠バンドル

プレビュー ウェーブ チケットに以下の成果物を含め、招待メールで参照してください:

| 成果物 | 目的 |
|--------|------|
| `build/checksums.sha256` | バンドルが CI ビルドと一致することを証明。 |
| `artifacts/sorafs/portal.tar.gz` + `portal.manifest.to` | 正規の SoraFS ペイロード + マニフェスト。 |
| `portal.pin.report.json`, `portal.manifest.submit.summary.json`, `portal.submit.response.json` | マニフェスト送信と alias バインディングが成功したことを示す。 |
| `artifacts/sorafs/portal.dns-cutover.json` | DNS メタデータ（チケット、ウィンドウ、連絡先）、経路昇格（`Sora-Route-Binding`）サマリ、`route_plan` ポインタ（JSON 計画 + ヘッダーテンプレート）、キャッシュパージ情報、Ops 向けロールバック手順。 |
| `artifacts/sorafs/preview-descriptor.json` | アーカイブ + チェックサムを結びつける署名付き descriptor。 |
| `probe` 出力 | ライブホストが期待する release タグを公開していることを確認。 |
