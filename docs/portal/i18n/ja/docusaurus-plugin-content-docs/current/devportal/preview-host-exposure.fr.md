---
lang: ja
direction: ltr
source: docs/portal/docs/devportal/preview-host-exposure.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# ホテルの展示会プレビューガイド

La feuille de Route DOCS-SORA exige que Chaque プレビュー パブリック アプリ シュール ミーム バンドルは、チェックサム クエリ テスト ロケールを検証します。採用決定後のランブック (招待状の承認チケット) をベータ版で利用してください。

## 前提条件

- トラッカー プレビューの承認と登録に関する曖昧な承認。
- `docs/portal/build/` とチェックサム検証 (`build/checksums.sha256`) を含むポータル ビルドの詳細。
- 識別子 SoraFS プレビュー (URL Torii、autorite、cle privee、epoch soumis) は、環境変数と構成 JSON 電話番号をストックします [`docs/examples/sorafs_preview_publish.json`](../../../examples/sorafs_preview_publish.json)。
- DNS の変更チケット (`docs-preview.sora.link`、`docs.iroha.tech` など) とオンコールの連絡先。

## Etape 1 - バンドルの構築と検証

```bash
cd docs/portal
export DOCS_RELEASE_TAG="preview-$(date -u +%Y%m%dT%H%M%SZ)"
npm ci
npm run build
./scripts/preview_verify.sh --build-dir build
```

検証拒否のスクリプトは継続的にチェックサム管理のマニフェストを変更し、プレビュー監査の監査結果を確認します。

## Etape 2 - パッケージャー レ アーティファクト SoraFS

サイト統計とCAR/マニフェストを変換して決定します。 `ARTIFACT_DIR` デフォルトの標準 `docs/portal/artifacts/`。

```bash
./scripts/sorafs-pin-release.sh       --alias docs-preview.sora       --alias-namespace docs       --alias-name preview       --pin-label docs-preview       --skip-submit

node scripts/generate-preview-descriptor.mjs       --manifest artifacts/checksums.sha256       --archive artifacts/sorafs/portal.tar.gz       --out artifacts/sorafs/preview-descriptor.json
```

Joignez `portal.car`、`portal.manifest.*`、曖昧なプレビューのチケットの説明とチェックサムのマニフェスト。

## Etape 3 - Publier l'alias プレビュー

Relancez le helper de pin **sans** `--skip-submit` lorsque vous etes pret a exuser l'hote. Fournissez soit le config JSON soit des flags CLI は以下を明示します。

```bash
./scripts/sorafs-pin-release.sh       --alias docs-preview.sora       --alias-namespace docs       --alias-name preview       --pin-label docs-preview       --config ~/secrets/sorafs_preview_publish.json
```

`portal.pin.report.json`、`portal.manifest.submit.summary.json`、および `portal.submit.response.json` の命令は、招待状の証拠の束に付随するものです。

## Etape 4 - バスキュール プランのジェネレータ DNS

```bash
node scripts/generate-dns-cutover-plan.mjs       --dns-hostname docs.iroha.tech       --dns-zone sora.link       --dns-change-ticket DOCS-SORA-Preview       --dns-cutover-window "2026-03-05 18:00Z"       --dns-ops-contact "pagerduty:sre-docs"       --manifest artifacts/sorafs/portal.manifest.to       --cache-purge-endpoint https://cache.api/purge       --cache-purge-auth-env CACHE_PURGE_TOKEN       --out artifacts/sorafs/portal.dns-cutover.json
```

JSON の結果として得られる avec Ops は、バスキュール DNS 参照ファイルのマニフェストの正確なダイジェストを取得します。 Lorsqu'un 記述の先例は、ソースとロールバックを再利用し、ajoutez `--previous-dns-plan path/to/previous.json` です。

## Etape 5 - ソンダージュ デ ロテ デプロイエ

```bash
npm run probe:portal --       --base-url=https://docs-preview.sora.link       --expect-release="$DOCS_RELEASE_TAG"
```

リリース サービスのタグを確認し、CSP ヘッダーと署名のメタドンを確認します。 Relancez la commande depuis deuxregion (ou joignez une sortie curl) は、キャッシュ エッジを監視するために使用されます。

## 証拠のバンドル

曖昧なプレビューおよび参照用のチケットおよび招待状の電子メールに関する資料が含まれます:

|アーティファクト |目的 |
|----------|----------|
| `build/checksums.sha256` | Prouve que le バンドルは au ビルド CI に対応します。 |
| `artifacts/sorafs/portal.tar.gz` + `portal.manifest.to` |ペイロード SoraFS 正規 + マニフェスト。 |
| `portal.pin.report.json`、`portal.manifest.submit.summary.json`、`portal.submit.response.json` |モントレ・ケ・ラ・ソウミッション・デュ・マニフェスト+ル・バインディング・ダリアス・オント・レウシ。 |
| `artifacts/sorafs/portal.dns-cutover.json` | Metadonnees DNS (チケット、フェネトレ、連絡先)、ルートのプロモーションの再開 (`Sora-Route-Binding`)、ポイント `route_plan` (プランの JSON + ヘッダーのテンプレート)、キャッシュのパージ情報とロールバック操作の指示。 |
| `artifacts/sorafs/preview-descriptor.json` |アーカイブ + チェックサムの記述。 |
|出撃 `probe` |リリースのタグの通知を確認してください。 |