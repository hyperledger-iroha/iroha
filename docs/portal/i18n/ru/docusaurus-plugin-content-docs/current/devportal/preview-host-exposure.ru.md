---
lang: ru
direction: ltr
source: docs/portal/docs/devportal/preview-host-exposure.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Руководство по экспозиции предварительного просмотра-хоста

Дорожная карта DOCS-SORA требует, чтобы каждый общедоступный предварительный просмотр использовал один и тот же пакет, проверенную контрольную сумму, которую ревьюеры проверяют локально. Используйте этот Runbook после завершения онбординга ревьюеров (одобрений приглашений), чтобы вывести хост предварительной бета-версии в сеть.

## Предварительные требования

- Волна онбординга ревьюеров одобрена и зафиксирована в трекере предварительного просмотра.
- Последний билд портала находится в `docs/portal/build/` и контрольная сумма проверена (`build/checksums.sha256`).
- Предварительный просмотр учётных данных SoraFS (URL-адрес Torii, полномочия, закрытый ключ, эпоха доставки) сохраняется в окружении окружения или конфигурации JSON, например [`docs/examples/sorafs_preview_publish.json`](../../../examples/sorafs_preview_publish.json).
- Открыт тикет на изменение DNS с желаемым именем хоста (`docs-preview.sora.link`, `docs.iroha.tech` и т.д.) и дежурными контактами.

## Шаг 1 — Собрать и проверить комплект

```bash
cd docs/portal
export DOCS_RELEASE_TAG="preview-$(date -u +%Y%m%dT%H%M%SZ)"
npm ci
npm run build
./scripts/preview_verify.sh --build-dir build
```

Скрипт проверки показывает продолжение, если контрольная сумма манифеста отсутствует или подделана, что сохраняет аудит всех документов предварительного просмотра.

## Шаг 2 — Упаковать архивы SoraFS

Преобразуйте статический сайт в детерминированную пару CAR/manifest. `ARTIFACT_DIR` по умолчанию `docs/portal/artifacts/`.

```bash
./scripts/sorafs-pin-release.sh       --alias docs-preview.sora       --alias-namespace docs       --alias-name preview       --pin-label docs-preview       --skip-submit

node scripts/generate-preview-descriptor.mjs       --manifest artifacts/checksums.sha256       --archive artifacts/sorafs/portal.tar.gz       --out artifacts/sorafs/preview-descriptor.json
```

Прикрепите `portal.car`, `portal.manifest.*`, дескриптор и контрольную сумму манифеста к волне предварительного просмотра тикета.

## Шаг 3 - Опубликовать псевдоним предварительного просмотра

Запустите помощник по выводу **без** `--skip-submit`, когда будете готовы открыть хост. Передайте конфигурацию JSON или явные флаги CLI:

```bash
./scripts/sorafs-pin-release.sh       --alias docs-preview.sora       --alias-namespace docs       --alias-name preview       --pin-label docs-preview       --config ~/secrets/sorafs_preview_publish.json
```

Команда пишет `portal.pin.report.json`, `portal.manifest.submit.summary.json` и `portal.submit.response.json`, которые должны быть включены в пакет доказательств приглашений.

## Шаг 4 — Сгенерировать план переключения DNS

```bash
node scripts/generate-dns-cutover-plan.mjs       --dns-hostname docs.iroha.tech       --dns-zone sora.link       --dns-change-ticket DOCS-SORA-Preview       --dns-cutover-window "2026-03-05 18:00Z"       --dns-ops-contact "pagerduty:sre-docs"       --manifest artifacts/sorafs/portal.manifest.to       --cache-purge-endpoint https://cache.api/purge       --cache-purge-auth-env CACHE_PURGE_TOKEN       --out artifacts/sorafs/portal.dns-cutover.json
```

Поделитесь полученным JSON с Ops, чтобы DNS-переключение ссылалось на манифест дайджеста Google. При повторном использовании используйте дескриптор в качестве источника отката строки `--previous-dns-plan path/to/previous.json`.

## Шаг 5 — Проверить быстрый хостинг

```bash
npm run probe:portal --       --base-url=https://docs-preview.sora.link       --expect-release="$DOCS_RELEASE_TAG"
```

Проверка подтверждения отдаваемого тега выпуска, заголовки CSP и метаданные подключения. Повторите команду из двух регионов (или предложите вывести Curl), чтобы аудиторы увидели, что пограничный кэш прогрет.

## Пакет доказательств

Включите следующие реквизиты в волну предварительного просмотра заявки и укажите их в письменном заявлении:

| Артефакт | Назначение |
|----------|------------|
| `build/checksums.sha256` | Доказывает, что бандл соответствует сборке CI. |
| `artifacts/sorafs/portal.tar.gz` + `portal.manifest.to` | Канонический SoraFS полезная нагрузка + манифест. |
| `portal.pin.report.json`, `portal.manifest.submit.summary.json`, `portal.submit.response.json` | Показывает, что отправка манифеста и привязка псевдонима успешны. |
| `artifacts/sorafs/portal.dns-cutover.json` | метаданные DNS (тикет, окно, контакты), сокращение маршрута продвижения (`Sora-Route-Binding`), указатель `route_plan` (JSON-план + заголовок шаблона), сведения об очистке кэша и инструкции по откату для Ops. |
| `artifacts/sorafs/preview-descriptor.json` | Подписанный дескриптор, связывающий архив + контрольная сумма. |
| Вывод `probe` | Подтверждает, что ведущий Live публикует ожидаемый тег релиза. |