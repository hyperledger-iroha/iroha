---
lang: ru
direction: ltr
source: docs/portal/docs/devportal/preview-host-exposure.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Инструкция по открытию хоста предварительного просмотра

В дорожной карте DOCS-SORA указано, что все публичные предварительные просмотры можно использовать, или пакет, проверенный по контрольной сумме, который можно изменить, выполняя локальные действия. Используйте этот Runbook после регистрации ревизоров (или билета подтверждения согласия) и завершите его для размещения или бета-тестирования хоста в Интернете.

## Предварительные требования

- Проверка и регистрация обновлений без отслеживания предварительного просмотра.
- Последняя сборка портала представляет `docs/portal/build/` и проверенную контрольную сумму (`build/checksums.sha256`).
- Credenciais предварительного просмотра SoraFS (URL Torii, autoridade, chave privada, epoch enviado) вооружены различными вариантами окружения или конфигурацией JSON в виде [`docs/examples/sorafs_preview_publish.json`](../../../examples/sorafs_preview_publish.json).
- Запрос на изменение DNS с использованием требуемого имени хоста (`docs-preview.sora.link`, `docs.iroha.tech` и т. д.) при вызове.

## Шаг 1 — Создание и проверка пакета

```bash
cd docs/portal
export DOCS_RELEASE_TAG="preview-$(date -u +%Y%m%dT%H%M%SZ)"
npm ci
npm run build
./scripts/preview_verify.sh --build-dir build
```

Если сценарий проверки требует продолжения, когда манифест контрольной суммы утрачен или подделан, происходит каждый раз артефакт предварительного просмотра.

## Passo 2 - Empacotar os artefatos SoraFS

Преобразование статичного сайта в детерминированный CAR/манифест. `ARTIFACT_DIR` Padrao и `docs/portal/artifacts/`.

```bash
./scripts/sorafs-pin-release.sh       --alias docs-preview.sora       --alias-namespace docs       --alias-name preview       --pin-label docs-preview       --skip-submit

node scripts/generate-preview-descriptor.mjs       --manifest artifacts/checksums.sha256       --archive artifacts/sorafs/portal.tar.gz       --out artifacts/sorafs/preview-descriptor.json
```

Anexe `portal.car`, `portal.manifest.*`, дескриптор и манифест контрольной суммы или билет для просмотра.

## Шаг 3 — Публикация или псевдоним предварительного просмотра

Повторно выполните помощник по выводу **sem** `--skip-submit`, когда это произойдет быстро для экспорта на хост. Примеры конфигурации JSON или явные флаги CLI:

```bash
./scripts/sorafs-pin-release.sh       --alias docs-preview.sora       --alias-namespace docs       --alias-name preview       --pin-label docs-preview       --config ~/secrets/sorafs_preview_publish.json
```

О командах `portal.pin.report.json`, `portal.manifest.submit.summary.json` и `portal.submit.response.json`, которые мы разработаем вместе с пакетом доказательств осуждения.

## Passo 4 — Герар или план корте DNS

```bash
node scripts/generate-dns-cutover-plan.mjs       --dns-hostname docs.iroha.tech       --dns-zone sora.link       --dns-change-ticket DOCS-SORA-Preview       --dns-cutover-window "2026-03-05 18:00Z"       --dns-ops-contact "pagerduty:sre-docs"       --manifest artifacts/sorafs/portal.manifest.to       --cache-purge-endpoint https://cache.api/purge       --cache-purge-auth-env CACHE_PURGE_TOKEN       --out artifacts/sorafs/portal.dns-cutover.json
```

Сравнивайте результаты JSON с операциями, чтобы получить нужную DNS-ссылку или дайджест exato в манифесте. Чтобы повторно использовать передний дескриптор как исходный откат, добавьте `--previous-dns-plan path/to/previous.json`.

## Шаг 5 - Тест или имплантат хоста

```bash
npm run probe:portal --       --base-url=https://docs-preview.sora.link       --expect-release="$DOCS_RELEASE_TAG"
```

Проверка подтверждения или тега выпуска сервера, заголовков CSP и метаданных уничтожения. Повторите команду двух областей (или приложение к локну) для того, чтобы аудиторы увидели, что крайний кеш находится там.

## Пакет доказательств

Включите следующие артефаты без билета для предварительного просмотра и ссылок без электронного письма для приглашения:

| Артефато | Предложение |
|----------|-----------|
| `build/checksums.sha256` | Проверьте, какой пакет соответствует сборке CI. |
| `artifacts/sorafs/portal.tar.gz` + `portal.manifest.to` | Полезная нагрузка canonico SoraFS + манифест. |
| `portal.pin.report.json`, `portal.manifest.submit.summary.json`, `portal.submit.response.json` | Большинство из них отправят манифест + псевдоним, привязывающийся к заключению. |
| `artifacts/sorafs/portal.dns-cutover.json` | Метададо DNS (билет, сообщение, контакты), резюме ротационного обновления (`Sora-Route-Binding`), сообщение `route_plan` (план JSON + шаблоны заголовка), информация об очистке кеша и инструкции по откату для операций. |
| `artifacts/sorafs/preview-descriptor.json` | Дескриптор, присвоенный строке или архиву + контрольная сумма. |
| Саида до `probe` | Подтвердите, что хост опубликовал живое объявление или тег выпуска. |