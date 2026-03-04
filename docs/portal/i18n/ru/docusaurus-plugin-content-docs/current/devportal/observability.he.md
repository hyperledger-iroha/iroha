---
lang: he
direction: rtl
source: docs/portal/i18n/ru/docusaurus-plugin-content-docs/current/devportal/observability.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: efd3fd00410791133e00b32db6466fd0b4e782d78d6842e47de9621282ad8dec
source_last_modified: "2026-01-03T18:08:01+00:00"
translation_last_reviewed: 2026-01-30
---

---
lang: ru
direction: ltr
source: docs/portal/docs/devportal/observability.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# Наблюдаемость и аналитика портала

Дорожная карта DOCS-SORA требует аналитики, синтетических probe и автоматизации
проверки битых ссылок для каждого preview build. В этой заметке описана инфраструктура,
которая поставляется вместе с порталом, чтобы операторы могли подключить мониторинг
без утечки данных посетителей.

## Тегирование релиза

- Установите `DOCS_RELEASE_TAG=<identifier>` (fallback на `GIT_COMMIT` или `dev`) при сборке портала.
  Значение внедряется в `<meta name="sora-release">`, чтобы probes и dashboards могли отличать
  развертывания.
- `npm run build` создает `build/release.json` (записывается `scripts/write-checksums.mjs`), где описаны
  тег, timestamp и опциональный `DOCS_RELEASE_SOURCE`. Этот файл упаковывается в preview-артефакты
  и упоминается в отчете link checker.

## Аналитика с сохранением приватности

- Настройте `DOCS_ANALYTICS_ENDPOINT=<https://collector.example/ingest>` для включения легкого трекера.
  Пейлоады содержат `{ event, path, locale, release, ts }` без referrer или IP метаданных, и при
  возможности используется `navigator.sendBeacon`, чтобы не блокировать навигации.
- Управляйте выборкой через `DOCS_ANALYTICS_SAMPLE_RATE` (0-1). Трекер хранит последний отправленный path
  и никогда не отправляет дубликаты событий для одной навигации.
- Реализация находится в `src/components/AnalyticsTracker.jsx` и глобально монтируется через `src/theme/Root.js`.

## Синтетические пробы

- `npm run probe:portal` делает GET-запросы на типичные маршруты (`/`, `/norito/overview`,
  `/reference/torii-swagger`, и т.д.) и проверяет, что мета тег `sora-release` соответствует
  `--expect-release` (или `DOCS_RELEASE_TAG`). Пример:

```bash
PORTAL_BASE_URL="https://docs.staging.sora" \
DOCS_RELEASE_TAG="preview-42" \
npm run probe:portal -- --expect-release=preview-42
```

Сбои показываются по path, что упрощает gate CD по успеху проб.

## Автоматизация битых ссылок

- `npm run check:links` сканирует `build/sitemap.xml`, убеждается, что каждая запись мапится на локальный файл
  (проверяя fallback `index.html`), и пишет `build/link-report.json`, содержащий метаданные релиза, итоги,
  ошибки и SHA-256 отпечаток `checksums.sha256` (выставлен как `manifest.id`), чтобы каждый отчет можно
  было связать с манифестом артефакта.
- Скрипт завершается с ненулевым кодом, когда страница отсутствует, поэтому CI может блокировать релизы
  при устаревших или сломанных маршрутах. Отчеты содержат кандидатные пути, которые пытались открыть, что
  помогает проследить регрессию маршрутизации до дерева docs.

## Дашборд Grafana и алерты

- `dashboards/grafana/docs_portal.json` публикует Grafana доску **Docs Portal Publishing**.
  Она включает следующие панели:
  - *Gateway Refusals (5m)* использует `torii_sorafs_gateway_refusals_total` в разрезе
    `profile`/`reason`, чтобы SRE могли обнаруживать плохие policy push или сбои токенов.
  - *Alias Cache Refresh Outcomes* и *Alias Proof Age p90* отслеживают `torii_sorafs_alias_cache_*`,
    чтобы подтвердить наличие свежих proof перед DNS cut over.
  - *Pin Registry Manifest Counts* и стат *Active Alias Count* отражают backlog pin-registry и общее
    число alias, чтобы governance могла аудировать каждый релиз.
  - *Gateway TLS Expiry (hours)* подсвечивает приближение истечения TLS cert publishing gateway
    (порог алерта 72 h).
  - *Replication SLA Outcomes* и *Replication Backlog* следят за телеметрией
    `torii_sorafs_replication_*`, чтобы убедиться, что все реплики соответствуют GA после публикации.
- Используйте встроенные template переменные (`profile`, `reason`), чтобы фокусироваться на
  publishing профиле `docs.sora` или исследовать всплески по всем шлюзам.
- Роутинг PagerDuty использует панели дашборда как доказательство: алерты
  `DocsPortal/GatewayRefusals`, `DocsPortal/AliasCache` и `DocsPortal/TLSExpiry` срабатывают,
  когда соответствующие серии выходят за порог. Привяжите runbook алерта к этой странице,
  чтобы on-call инженеры могли повторить точные Prometheus запросы.

## Сводим вместе

1. Во время `npm run build` установите переменные окружения release/analytics и дайте post-build шагу
   записать `checksums.sha256`, `release.json` и `link-report.json`.
2. Запустите `npm run probe:portal` против preview hostname с `--expect-release`, связанным с тем же тегом.
   Сохраните stdout для publishing чеклиста.
3. Запустите `npm run check:links`, чтобы быстро упасть на битых записях sitemap и архивировать
   сгенерированный JSON отчет вместе с preview артефактами. CI кладет последний отчет в
   `artifacts/docs_portal/link-report.json`, чтобы governance могла скачать evidence bundle прямо из логов build.
4. Прокиньте analytics endpoint в ваш privacy-preserving collector (Plausible, self-hosted OTEL ingest и т.д.)
   и убедитесь, что sample rate документируется для каждого релиза, чтобы dashboards корректно интерпретировали
   счетчики.
5. CI уже прокладывает эти шаги через preview/deploy workflows
   (`.github/workflows/docs-portal-preview.yml`,
   `.github/workflows/docs-portal-deploy.yml`), поэтому локальные dry runs должны покрывать
   только поведение, связанное с секретами.
