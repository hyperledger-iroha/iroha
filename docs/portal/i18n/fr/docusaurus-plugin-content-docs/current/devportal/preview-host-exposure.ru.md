---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/preview-host-exposure.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# Руководство по экспозиции preview-хоста

Дорожная карта DOCS-SORA требует, чтобы каждый публичный preview использовал тот же bundle, проверенный checksum, который ревьюеры проверяют локально. Используйте этот runbook после завершения онбординга ревьюеров (и одобрения приглашений), чтобы вывести beta preview host в сеть.

## Предварительные требования

- Волна онбординга ревьюеров одобрена и зафиксирована в preview tracker.
- Последний билд портала находится в `docs/portal/build/` и checksum проверен (`build/checksums.sha256`).
- Учётные данные SoraFS preview (Torii URL, authority, private key, отправленный epoch) сохранены в переменных окружения или JSON конфиге, например [`docs/examples/sorafs_preview_publish.json`](../../../examples/sorafs_preview_publish.json).
- Открыт тикет на изменение DNS с желаемым hostname (`docs-preview.sora.link`, `docs.iroha.tech` и т.д.) и on-call контактами.

## Шаг 1 - Собрать и проверить bundle

```bash
cd docs/portal
export DOCS_RELEASE_TAG="preview-$(date -u +%Y%m%dT%H%M%SZ)"
npm ci
npm run build
./scripts/preview_verify.sh --build-dir build
```

Скрипт проверки откажется продолжать, если манифест checksum отсутствует или подделан, что сохраняет аудит всех preview артефактов.

## Шаг 2 - Упаковать артефакты SoraFS

Преобразуйте статический сайт в детерминированную пару CAR/manifest. `ARTIFACT_DIR` по умолчанию `docs/portal/artifacts/`.

```bash
./scripts/sorafs-pin-release.sh       --alias docs-preview.sora       --alias-namespace docs       --alias-name preview       --pin-label docs-preview       --skip-submit

node scripts/generate-preview-descriptor.mjs       --manifest artifacts/checksums.sha256       --archive artifacts/sorafs/portal.tar.gz       --out artifacts/sorafs/preview-descriptor.json
```

Прикрепите `portal.car`, `portal.manifest.*`, descriptor и манифест checksum к тикету preview wave.

## Шаг 3 - Опубликовать preview alias

Запустите pin helper **без** `--skip-submit`, когда будете готовы открыть хост. Передайте JSON конфиг или явные CLI флаги:

```bash
./scripts/sorafs-pin-release.sh       --alias docs-preview.sora       --alias-namespace docs       --alias-name preview       --pin-label docs-preview       --config ~/secrets/sorafs_preview_publish.json
```

Команда пишет `portal.pin.report.json`, `portal.manifest.submit.summary.json` и `portal.submit.response.json`, которые должны быть включены в evidence bundle приглашений.

## Шаг 4 - Сгенерировать план DNS cutover

```bash
node scripts/generate-dns-cutover-plan.mjs       --dns-hostname docs.iroha.tech       --dns-zone sora.link       --dns-change-ticket DOCS-SORA-Preview       --dns-cutover-window "2026-03-05 18:00Z"       --dns-ops-contact "pagerduty:sre-docs"       --manifest artifacts/sorafs/portal.manifest.to       --cache-purge-endpoint https://cache.api/purge       --cache-purge-auth-env CACHE_PURGE_TOKEN       --out artifacts/sorafs/portal.dns-cutover.json
```

Поделитесь полученным JSON с Ops, чтобы DNS переключение ссылалось на точный digest manifest. При повторном использовании предыдущего descriptor как источника rollback добавьте `--previous-dns-plan path/to/previous.json`.

## Шаг 5 - Проверить развернутый хост

```bash
npm run probe:portal --       --base-url=https://docs-preview.sora.link       --expect-release="$DOCS_RELEASE_TAG"
```

Probe подтверждает отдаваемый release tag, CSP заголовки и метаданные подписи. Повторите команду из двух регионов (или приложите вывод curl), чтобы аудиторы увидели, что edge cache прогрет.

## Evidence bundle

Включите следующие артефакты в тикет preview wave и укажите их в письме-приглашении:

| Артефакт | Назначение |
|----------|------------|
| `build/checksums.sha256` | Доказывает, что bundle соответствует CI build. |
| `artifacts/sorafs/portal.tar.gz` + `portal.manifest.to` | Канонический SoraFS payload + manifest. |
| `portal.pin.report.json`, `portal.manifest.submit.summary.json`, `portal.submit.response.json` | Показывает, что отправка manifest и привязка alias успешны. |
| `artifacts/sorafs/portal.dns-cutover.json` | DNS метаданные (тикет, окно, контакты), сводка продвижения маршрута (`Sora-Route-Binding`), указатель `route_plan` (JSON план + шаблоны header), сведения о cache purge и инструкции rollback для Ops. |
| `artifacts/sorafs/preview-descriptor.json` | Подписанный descriptor, связывающий archive + checksum. |
| Вывод `probe` | Подтверждает, что live host публикует ожидаемый release tag. |
