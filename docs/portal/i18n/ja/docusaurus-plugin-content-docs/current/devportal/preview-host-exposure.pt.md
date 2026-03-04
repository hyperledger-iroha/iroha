---
lang: ja
direction: ltr
source: docs/portal/docs/devportal/preview-host-exposure.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# プレビューをホストとして説明する

ロードマップ DOCS-SORA は、ローカルで実行されるチェックサム クエリのバンドル検証や OS 改訂の使用をプレビューするために公開されています。オンラインでのホスト ベータ版の完了には、ESTE​​ Runbook を使用してオンボーディングの改訂 (会議の承認チケット) を実行してください。

## 前提条件

- オンボーディングと改訂の承認およびプレビューのトラッカーなしの登録。
- Ultimo ビルドは、`docs/portal/build/` およびチェックサム検証 (`build/checksums.sha256`) をポータルに提示します。
- プレビュー SoraFS の認証情報 (URL Torii、オートリダード、チャット プライベート、エポック エンビアド) は、環境変数の設定 JSON コモ [`docs/examples/sorafs_preview_publish.json`](../../../examples/sorafs_preview_publish.json) を提供します。
- チケット デ ムダンカ DNS アベルト コム、ホスト名デセハド (`docs-preview.sora.link`、`docs.iroha.tech` など) はオンコールで送信されます。

## パッソ 1 - バンドルの構築と検証

```bash
cd docs/portal
export DOCS_RELEASE_TAG="preview-$(date -u +%Y%m%dT%H%M%SZ)"
npm ci
npm run build
./scripts/preview_verify.sh --build-dir build
```

スクリプトの検証は継続的に行われ、チェックサムのマニフェストは成人向けに行われ、プレビュー監査の精度は維持されます。

## Passo 2 - Empacotar os artefatos SoraFS

CAR/マニフェストの決定性を考慮したサイトの静的変換。 `ARTIFACT_DIR` パドラオ e `docs/portal/artifacts/`。

```bash
./scripts/sorafs-pin-release.sh       --alias docs-preview.sora       --alias-namespace docs       --alias-name preview       --pin-label docs-preview       --skip-submit

node scripts/generate-preview-descriptor.mjs       --manifest artifacts/checksums.sha256       --archive artifacts/sorafs/portal.tar.gz       --out artifacts/sorafs/preview-descriptor.json
```

Anexe `portal.car`、`portal.manifest.*`、記述子、マニフェスト、チェックサム、チケット、プレビュー。

## Passo 3 - プレビューの別名公開

ヘルパー デ ピン **sem** `--skip-submit` quando estiver pronto para expor o host を再実行します。 Forneca で JSON を設定し、CLI で明示的にフラグを設定します。

```bash
./scripts/sorafs-pin-release.sh       --alias docs-preview.sora       --alias-namespace docs       --alias-name preview       --pin-label docs-preview       --config ~/secrets/sorafs_preview_publish.json
```

どうか、`portal.pin.report.json`、`portal.manifest.submit.summary.json`、`portal.submit.response.json`、会議の証拠をまとめて開発してください。

## Passo 4 - ゲラル・オ・プラノ・デ・コルテDNS

```bash
node scripts/generate-dns-cutover-plan.mjs       --dns-hostname docs.iroha.tech       --dns-zone sora.link       --dns-change-ticket DOCS-SORA-Preview       --dns-cutover-window "2026-03-05 18:00Z"       --dns-ops-contact "pagerduty:sre-docs"       --manifest artifacts/sorafs/portal.manifest.to       --cache-purge-endpoint https://cache.api/purge       --cache-purge-auth-env CACHE_PURGE_TOKEN       --out artifacts/sorafs/portal.dns-cutover.json
```

JSON の結果を比較し、マニフェストのダイジェストを作成し、DNS 参照を処理します。ロールバックの前に記述子を再利用し、アディシオン `--previous-dns-plan path/to/previous.json`。

## Passo 5 - ホストインプラントのテスト

```bash
npm run probe:portal --       --base-url=https://docs-preview.sora.link       --expect-release="$DOCS_RELEASE_TAG"
```

プローブの確認、リリース サービドのタグ、CSP のヘッダー、およびアシナチュアのメタデータ。地域の一部のコマンド (カールの付属文書) を視聴して、エッジ キャッシュを確認してください。

## 証拠のバンドル

OS のセグインテス アートを含めるには、プレビューおよび参照用のチケットは必要ありません。会議の電子メールは必要ありません。

|アルテファト |プロポジト |
|----------|----------|
| `build/checksums.sha256` | CI のビルドに対応するバンドルを提供します。 |
| `artifacts/sorafs/portal.tar.gz` + `portal.manifest.to` |ペイロード canonico SoraFS + マニフェスト。 |
| `portal.pin.report.json`、`portal.manifest.submit.summary.json`、`portal.submit.response.json` |マニフェストとエイリアス バインディング フォームの結論を実行するためのほとんどの環境。 |
| `artifacts/sorafs/portal.dns-cutover.json` | Metadados DNS (チケット、ジャネラ、コンタトス)、プロモーション レポート (`Sora-Route-Binding`)、ポンテイロ `route_plan` (JSON の平面 + ヘッダーのテンプレート)、キャッシュのパージ情報と運用時のロールバック手順の情報。 |
| `artifacts/sorafs/preview-descriptor.json` |アーカイブ + チェックサムの記述子。 |
|サイダ ド `probe` |ホストを生体内で発表し、リリースエスペラードをタグ付けすることを確認します。 |