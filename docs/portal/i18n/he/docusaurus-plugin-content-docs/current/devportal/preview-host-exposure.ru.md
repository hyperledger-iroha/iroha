---
lang: he
direction: rtl
source: docs/portal/docs/devportal/preview-host-exposure.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Руководство по экспозиции preview-хоста

Дорожная карта DOCS-SORA требует, чтобы каждый публичный תצוגה מקדימה использовал тот же חבילה, проверыкорный проверяют локально. Используйте этот runbook после завершения онбординга ревьюеров (и одобрения приглашений), чтобы всетьв בטא תצוגה מקדימה.

## Предварительные требования

- Волна онбординга ревьюеров одобрена и зафиксирована в מעקב תצוגה מקדימה.
- Последний билд портала находится в `docs/portal/build/` и checksum проверен (`build/checksums.sha256`).
- Учётные данные SoraFS (Torii כתובת אתר, סמכות, מפתח פרטי, עידן отправленный) сохранены в переменных окружо например [`docs/examples/sorafs_preview_publish.json`](../../../examples/sorafs_preview_publish.json).
- כתובות ל-DNS עם שם מארח של желаемым (`docs-preview.sora.link`, `docs.iroha.tech` ועוד) ושיחות כוננות.

## Шаг 1 - חבילת Собрать и проверить

```bash
cd docs/portal
export DOCS_RELEASE_TAG="preview-$(date -u +%Y%m%dT%H%M%SZ)"
npm ci
npm run build
./scripts/preview_verify.sh --build-dir build
```

הצג את הבדיקה או את התוכנית, הצג את התצוגה המקדימה של התוכנית.

## Шаг 2 - Упаковать артефакты SoraFS

Преобразуйте статический сайт в детерминированную пару CAR/מניפסט. `ARTIFACT_DIR` по умолчанию `docs/portal/artifacts/`.

```bash
./scripts/sorafs-pin-release.sh       --alias docs-preview.sora       --alias-namespace docs       --alias-name preview       --pin-label docs-preview       --skip-submit

node scripts/generate-preview-descriptor.mjs       --manifest artifacts/checksums.sha256       --archive artifacts/sorafs/portal.tar.gz       --out artifacts/sorafs/preview-descriptor.json
```

Прикрепите `portal.car`, `portal.manifest.*`, מתאר и манифест checksum к тикету תצוגה מקדימה גל.

## Шаг 3 - Опубликовать כינוי תצוגה מקדימה

Запустите pin helper **חינם** `--skip-submit`, когда будете готовы открыть хост. הצג את JSON קונפיג או מפלגות CLI:

```bash
./scripts/sorafs-pin-release.sh       --alias docs-preview.sora       --alias-namespace docs       --alias-name preview       --pin-label docs-preview       --config ~/secrets/sorafs_preview_publish.json
```

Команда пишет `portal.pin.report.json`, `portal.manifest.submit.summary.json` ו-`portal.submit.response.json`, которые должны быть включены в חבילת ראיות прига.

## Шаг 4 - Сгенерировать план חיתוך DNS

```bash
node scripts/generate-dns-cutover-plan.mjs       --dns-hostname docs.iroha.tech       --dns-zone sora.link       --dns-change-ticket DOCS-SORA-Preview       --dns-cutover-window "2026-03-05 18:00Z"       --dns-ops-contact "pagerduty:sre-docs"       --manifest artifacts/sorafs/portal.manifest.to       --cache-purge-endpoint https://cache.api/purge       --cache-purge-auth-env CACHE_PURGE_TOKEN       --out artifacts/sorafs/portal.dns-cutover.json
```

Поделитесь полученным JSON с Ops, чтобы DNS переключение ссылалось на точный digest manifest. При повторном использовании предыдущего descriptor как источника rollback добавьте `--previous-dns-plan path/to/previous.json`.

## Шаг 5 - Проверить развернутый хост

```bash
npm run probe:portal --       --base-url=https://docs-preview.sora.link       --expect-release="$DOCS_RELEASE_TAG"
```

Probe подтверждает отдаваемый תג שחרור, CSP заголовки ו метаданные подписи. Повторите команду из двух регионов (или приложите вывод curl), чтобы аудиторы увидели, что edge cache прогрет.

## חבילת ראיות

Включите следующие артефакты в тикет תצוגה מקדימה wave и укажите их в письме-приглашении:

| Артефакт | Назначение |
|--------|----------------|
| `build/checksums.sha256` | Доказывает, что חבילה соответствует CI build. |
| `artifacts/sorafs/portal.tar.gz` + `portal.manifest.to` | Канонический SoraFS מטען + מניפסט. |
| `portal.pin.report.json`, `portal.manifest.submit.summary.json`, `portal.submit.response.json` | Показывает, что отправка מניפסט и привязка כינוי успешны. |
| `artifacts/sorafs/portal.dns-cutover.json` | DNS метаданные (тикет, окно, контакты), сводка продвижения маршрута (`Sora-Route-Binding`), указатель SoraFS (+J18NI00000000SON) header), сведения о ניקוי מטמון и инструкции rollback ל-Ops. |
| `artifacts/sorafs/preview-descriptor.json` | מתאר Подписанный, связывающий ארכיון + בדיקת סכום. |
| Вывод `probe` | Подтверждает, что מארח חי публикует ожидаемый תג שחרור. |