---
lang: ru
direction: ltr
source: docs/portal/docs/devportal/preview-host-exposure.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Инструкция по открытию хоста предварительного просмотра

Дорожная карта DOCS-SORA требует, чтобы каждая предварительная версия публично использовала пакет мисмо, проверенный для проверки контрольной суммы, которую необходимо выполнить локально. Используйте этот Runbook после завершения регистрации ревизоров (и билета на апробацию приглашений), чтобы попасть на хост предварительной бета-версии.

## Предыдущие реквизиты

- Опробуйте и зарегистрируйтесь на трекере предварительного просмотра.
- Последняя сборка портала представлена ​​в формате `docs/portal/build/` и проверена контрольная сумма (`build/checksums.sha256`).
- Учетные данные предварительного просмотра SoraFS (URL-адрес Torii, autoridad, llave privada, epoch enviado) добавлены в переменные или в конфигурацию JSON как [`docs/examples/sorafs_preview_publish.json`](../../../examples/sorafs_preview_publish.json).
- Билет на смену DNS с желаемым именем хоста (`docs-preview.sora.link`, `docs.iroha.tech` и т. д.) для контактов по вызову.

## Шаг 1 — Создание и проверка пакета

```bash
cd docs/portal
export DOCS_RELEASE_TAG="preview-$(date -u +%Y%m%dT%H%M%SZ)"
npm ci
npm run build
./scripts/preview_verify.sh --build-dir build
```

Сценарий проверки не будет продолжаться, пока не будет отображена неверная контрольная сумма или манипуляция с ней, будет проведена проверка каждого артефакта предварительного просмотра.

## Шаг 2 - Упаковать артефакты SoraFS

Преобразуйте статическое положение в детерминированный CAR/манифест. `ARTIFACT_DIR` из-за дефекта `docs/portal/artifacts/`.

```bash
./scripts/sorafs-pin-release.sh       --alias docs-preview.sora       --alias-namespace docs       --alias-name preview       --pin-label docs-preview       --skip-submit

node scripts/generate-preview-descriptor.mjs       --manifest artifacts/checksums.sha256       --archive artifacts/sorafs/portal.tar.gz       --out artifacts/sorafs/preview-descriptor.json
```

Дополнения `portal.car`, `portal.manifest.*`, дескриптор и манифест контрольной суммы для билета предварительного просмотра.

## Шаг 3 – Публикация псевдонима предварительного просмотра

Повторите действие помощника по выводу **sin** `--skip-submit`, когда этот список будет показан для отображения хоста. Пропорции конфигурации JSON или явных флагов CLI:

```bash
./scripts/sorafs-pin-release.sh       --alias docs-preview.sora       --alias-namespace docs       --alias-name preview       --pin-label docs-preview       --config ~/secrets/sorafs_preview_publish.json
```

Команда напишет `portal.pin.report.json`, `portal.manifest.submit.summary.json` и `portal.submit.response.json`, которые нужно будет использовать с пакетом приглашений.

## Шаг 4 — Создание плана кортежа DNS

```bash
node scripts/generate-dns-cutover-plan.mjs       --dns-hostname docs.iroha.tech       --dns-zone sora.link       --dns-change-ticket DOCS-SORA-Preview       --dns-cutover-window "2026-03-05 18:00Z"       --dns-ops-contact "pagerduty:sre-docs"       --manifest artifacts/sorafs/portal.manifest.to       --cache-purge-endpoint https://cache.api/purge       --cache-purge-auth-env CACHE_PURGE_TOKEN       --out artifacts/sorafs/portal.dns-cutover.json
```

Сравните результат JSON с операциями, чтобы получить точную DNS-ссылку на дайджест манифеста. Когда он повторно использует передний дескриптор в качестве функции отката, в совокупности `--previous-dns-plan path/to/previous.json`.

## Пасо 5 – проверить хозяина

```bash
npm run probe:portal --       --base-url=https://docs-preview.sora.link       --expect-release="$DOCS_RELEASE_TAG"
```

Проверка подтверждает тег выпуска сервера, заголовки CSP и метаданные фирмы. Повторите команду в двух регионах (или в дополнение к завитку), чтобы аудиторы могли убедиться, что край кэша остывает.

## Пакет доказательств

Добавьте важные артефакты в билет предварительного просмотра и рекомендации в электронное письмо с приглашением:| Артефакт | Предложение |
|----------|-----------|
| `build/checksums.sha256` | Убедитесь, что пакет совпадает со сборкой CI. |
| `artifacts/sorafs/portal.tar.gz` + `portal.manifest.to` | Полезная нагрузка canonico SoraFS + манифесто. |
| `portal.pin.report.json`, `portal.manifest.submit.summary.json`, `portal.submit.response.json` | Muestra que el envio del manifico + el aliasbinding se completaron. |
| `artifacts/sorafs/portal.dns-cutover.json` | Метаданные DNS (билет, вход, контакты), возобновление продвижения руты (`Sora-Route-Binding`), точка `route_plan` (план JSON + элементы заголовка), информация по очистке кеша и инструкции по откату для операций. |
| `artifacts/sorafs/preview-descriptor.json` | Дескриптор, загружающий архив + контрольная сумма. |
| Салида де `probe` | Подтвердите, что хост в естественных условиях немедленно объявил о выпуске. |