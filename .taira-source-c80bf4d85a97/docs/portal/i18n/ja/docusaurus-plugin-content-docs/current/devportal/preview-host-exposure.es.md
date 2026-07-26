---
lang: ja
direction: ltr
source: docs/portal/docs/devportal/preview-host-exposure.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# ホストのプレビューに関する説明

ロードマップ DOCS-SORA の証明書プレビュー公開では、ミスモ バンドルのチェックサム検証を使用して、ローカルの改訂を確認してください。米国のランブックは、完全なオンボーディングと改訂 (招待チケットと招待状) とプレビュー ベータ版のホストを作成します。

## 以前の要件

- プレビューのトラッカーの登録とオンボーディングの改訂を行います。
- `docs/portal/build/` およびチェックサム検証 (`build/checksums.sha256`) でポータルを提示するウルティモ ビルド。
- プレビュー SoraFS の認証情報 (Torii の URL、autoridad、llave privada、epoch enviado) は、変数と設定 JSON の組み合わせ [`docs/examples/sorafs_preview_publish.json`](../../../examples/sorafs_preview_publish.json) を参照します。
- オンコールの連絡先としてホスト名を指定して DNS チケット (`docs-preview.sora.link`、`docs.iroha.tech` など) を設定します。

## 手順 1 - バンドルの構築と検証

```bash
cd docs/portal
export DOCS_RELEASE_TAG="preview-$(date -u +%Y%m%dT%H%M%SZ)"
npm ci
npm run build
./scripts/preview_verify.sh --build-dir build
```

スクリプトの検証は継続的に行われ、チェックサム ファルタまたはフエのマニピュレートをマニフェストし、プレビューの監査結果を管理します。

## パソ 2 - エンパケタール ロス アーティファクトス SoraFS

CAR/マニフェスト決定者によるサイトの情報を確認します。 `ARTIFACT_DIR` は `docs/portal/artifacts/` に欠陥があります。

```bash
./scripts/sorafs-pin-release.sh       --alias docs-preview.sora       --alias-namespace docs       --alias-name preview       --pin-label docs-preview       --skip-submit

node scripts/generate-preview-descriptor.mjs       --manifest artifacts/checksums.sha256       --archive artifacts/sorafs/portal.tar.gz       --out artifacts/sorafs/preview-descriptor.json
```

付属品 `portal.car`、`portal.manifest.*`、記述子およびプレビューのチェックサム チケットをマニフェストします。

## Paso 3 - プレビューの別名公開

ヘルパー デ ピン **罪** `--skip-submit` cuando estes listo para exponer el host を繰り返します。 JSON 設定または CLI 明示的なフラグの設定:

```bash
./scripts/sorafs-pin-release.sh       --alias docs-preview.sora       --alias-namespace docs       --alias-name preview       --pin-label docs-preview       --config ~/secrets/sorafs_preview_publish.json
```

コマンドは `portal.pin.report.json`、`portal.manifest.submit.summary.json` および `portal.submit.response.json` を記述し、jar 経由で招待状の証拠バンドルを参照してください。

## パソ 4 - 一般的な DNS 計画

```bash
node scripts/generate-dns-cutover-plan.mjs       --dns-hostname docs.iroha.tech       --dns-zone sora.link       --dns-change-ticket DOCS-SORA-Preview       --dns-cutover-window "2026-03-05 18:00Z"       --dns-ops-contact "pagerduty:sre-docs"       --manifest artifacts/sorafs/portal.manifest.to       --cache-purge-endpoint https://cache.api/purge       --cache-purge-auth-env CACHE_PURGE_TOKEN       --out artifacts/sorafs/portal.dns-cutover.json
```

操作パラレルの JSON 結果を比較し、DNS 参照とダイジェストを正確にマニフェストします。 Cuando はロールバックの前の記述子を再利用し、agrega `--previous-dns-plan path/to/previous.json` を実行します。

## パソ 5 - プロバー エル ホスト デスプレガド

```bash
npm run probe:portal --       --base-url=https://docs-preview.sora.link       --expect-release="$DOCS_RELEASE_TAG"
```

リリース サービスのタグ、ヘッダー CSP、および企業のメタデータをプローブします。地域のコマンド (またはカールの補助) を繰り返し、エッジ キャッシュを確認してください。

## 証拠のバンドル

招待状のプレビューおよび参照用のチケットのシグエンテス アートファクトが含まれています:

|アーティファクト |プロポジト |
|----------|----------|
| `build/checksums.sha256` |バンドルの実装は、CI のビルドと一致します。 |
| `artifacts/sorafs/portal.tar.gz` + `portal.manifest.to` |ペイロード canonico SoraFS + マニフェスト。 |
| `portal.pin.report.json`、`portal.manifest.submit.summary.json`、`portal.submit.response.json` | Muestra que el envio del manifyto + el alias binding se completaron。 |
| `artifacts/sorafs/portal.dns-cutover.json` |メタデータ DNS (チケット、ベンタナ、コンタクト)、プロモーションの再開 (`Sora-Route-Binding`)、エルプンテロ `route_plan` (プラン JSON + ヘッダーのプラント)、キャッシュのパージ情報、および運用時のロールバック命令の情報。 |
| `artifacts/sorafs/preview-descriptor.json` |記述子 armado que enlaza el archive + チェックサム。 |
|サリダ デ `probe` |生体内でのホストのリリース タグを確認します。 |