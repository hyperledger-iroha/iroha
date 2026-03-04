---
lang: ja
direction: ltr
source: docs/portal/docs/devportal/preview-host-exposure.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Руководство экспозиции プレビュー-хоста

DOCS-SORA требует、чтобы каждый публичный プレビュー использовал тот же バンドル、проверенный checksum、который ревьюеры проверяют локально。 Runbook は、ベータ プレビュー ホストのテストを実行します。

## Предварительные требования

- プレビュー トラッカーの機能。
- `docs/portal/build/` とチェックサム (`build/checksums.sha256`) を確認します。
- SoraFS プレビュー (Torii URL、権限、秘密キー、エポック) と JSON の比較конфиге、например [`docs/examples/sorafs_preview_publish.json`](../../../examples/sorafs_preview_publish.json)。
- DNS とホスト名 (`docs-preview.sora.link`、`docs.iroha.tech` および т.д.) およびオンコール контактами。

## Шаг 1 - Собрать и проверить バンドル

```bash
cd docs/portal
export DOCS_RELEASE_TAG="preview-$(date -u +%Y%m%dT%H%M%SZ)"
npm ci
npm run build
./scripts/preview_verify.sh --build-dir build
```

チェックサム チェックサム チェックサム プレビュー プレビューああ。

## Шаг 2 - Упаковать артефакты SoraFS

CAR/マニフェストを確認してください。 `ARTIFACT_DIR` は `docs/portal/artifacts/` です。

```bash
./scripts/sorafs-pin-release.sh       --alias docs-preview.sora       --alias-namespace docs       --alias-name preview       --pin-label docs-preview       --skip-submit

node scripts/generate-preview-descriptor.mjs       --manifest artifacts/checksums.sha256       --archive artifacts/sorafs/portal.tar.gz       --out artifacts/sorafs/preview-descriptor.json
```

`portal.car`、`portal.manifest.*`、記述子、チェックサム、プレビュー波形。

## 手順 3 - プレビュー エイリアス

ピン ヘルパー **без** `--skip-submit` を使用して、ピン ヘルパーを呼び出します。 JSON と CLI の組み合わせ:

```bash
./scripts/sorafs-pin-release.sh       --alias docs-preview.sora       --alias-namespace docs       --alias-name preview       --pin-label docs-preview       --config ~/secrets/sorafs_preview_publish.json
```

`portal.pin.report.json`、`portal.manifest.submit.summary.json`、`portal.submit.response.json`、証拠バンドルを確認してください。

## 手順 4 - DNS カットオーバーの手順

```bash
node scripts/generate-dns-cutover-plan.mjs       --dns-hostname docs.iroha.tech       --dns-zone sora.link       --dns-change-ticket DOCS-SORA-Preview       --dns-cutover-window "2026-03-05 18:00Z"       --dns-ops-contact "pagerduty:sre-docs"       --manifest artifacts/sorafs/portal.manifest.to       --cache-purge-endpoint https://cache.api/purge       --cache-purge-auth-env CACHE_PURGE_TOKEN       --out artifacts/sorafs/portal.dns-cutover.json
```

JSON と Ops、DNS のダイジェスト マニフェストを確認します。 `--previous-dns-plan path/to/previous.json` を使用して、記述子をロールバックします。

## Шаг 5 - Проверить развернутый хост

```bash
npm run probe:portal --       --base-url=https://docs-preview.sora.link       --expect-release="$DOCS_RELEASE_TAG"
```

リリース タグ、CSP および метаданные подписи をプローブします。 Повторите команду из двух регионов (или приложите вывод カール)、чтобы аудиторы увидели、что エッジ キャッシュ прогрет。

## 証拠の束

プレビュー ウェーブを表示するには、次の手順を実行します。

| Артефакт | Назначение |
|----------|-----------|
| `build/checksums.sha256` | CI ビルドをバンドルします。 |
| `artifacts/sorafs/portal.tar.gz` + `portal.manifest.to` | Канонический SoraFS ペイロード + マニフェスト。 |
| `portal.pin.report.json`、`portal.manifest.submit.summary.json`、`portal.submit.response.json` | Показывает、что отправка マニフェストと привязка エイリアスが必要です。 |
| `artifacts/sorafs/portal.dns-cutover.json` | DNS メソッド (тикет, окно, контакты)、сводка продвижения маргрута (`Sora-Route-Binding`)、указатель `route_plan` (JSON план +ヘッダー)、キャッシュのパージ、ロールバック、操作。 |
| `artifacts/sorafs/preview-descriptor.json` | Подписанный 記述子、связывающий アーカイブ + チェックサム。 |
| Вывод `probe` | Подтверждает、что ライブホスト публикует ожидаемый リリース タグ。 |