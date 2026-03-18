---
lang: ru
direction: ltr
source: docs/portal/docs/devportal/observability.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Наблюдение и аналитика портала

Дорожная карта DOCS-SORA требует аналитического анализа, синтетических зондов и автоматизации ротационного сплетения
для каждой сборки предварительного просмотра. Это нота документа о том, что сейчас происходит с порталом.
Чтобы операторы могли подключить монитор без фильтра данных посетителей.

## Этикет выпуска

- Определен `DOCS_RELEASE_TAG=<identifier>` (возможен резервный вариант `GIT_COMMIT` или `dev`) и др.
  построить портал. Доблесть появилась в `<meta name="sora-release">`
  для различных датчиков и приборных панелей.
- `npm run build` испускает `build/release.json` (скрипт для
  `scripts/write-checksums.mjs`) описывает тег, метку времени и
  `DOCS_RELEASE_SOURCE` опционально. Архив мисмо упакован в артефакты предварительного просмотра и
  Вы можете найти ссылку в отчете о проверке ссылок.

## Аналитика с сохранением конфиденциальности

- Настройка `DOCS_ANALYTICS_ENDPOINT=<https://collector.example/ingest>` для пункта
  Хабилитар эль трекер Ливиано. Полезные данные, содержащиеся `{ event, path, locale,
  выпуск, ts }` sin metadata de referrer o IP, y se usa `navigator.sendBeacon`
  Если море возможно, чтобы заблокировать навигацию.
- Управляйте устройством с `DOCS_ANALYTICS_SAMPLE_RATE` (0-1). Эль трекер гвардия
  последний путь отправлен и не вызывает дубликатов событий для неправильной навигации.
- Реализация вживую в `src/components/AnalyticsTracker.jsx` и в Монте
  globalmente a traves de `src/theme/Root.js`.

## Синтетические зонды

- `npm run probe:portal` выдает запросы GET contra rutas comunes
  (`/`, `/norito/overview`, `/reference/torii-swagger` и т. д.) и проверьте, что это
  метатег `sora-release` совпадает с `--expect-release` (o
  `DOCS_RELEASE_TAG`). Пример:

```bash
PORTAL_BASE_URL="https://docs.staging.sora" \
DOCS_RELEASE_TAG="preview-42" \
npm run probe:portal -- --expect-release=preview-42
```

Когда вы сообщаете о пути, вы можете легко получить компакт-диск с результатом исследования.

## Автоматизация вращения

- `npm run check:links` escanea `build/sitemap.xml`, убедитесь, что каждый раз вы вводите карту в один
  локальный архив (резервные резервные копии `index.html`), опишите
  `build/link-report.json` с метаданными выпуска, общими данными, отпечатками пальцев и отпечатками пальцев
  SHA-256 от `checksums.sha256` (используется как `manifest.id`) для каждого отчета
  se pueda vincular al manifico del artefacto.
- Продажа сценария с особым кодом, когда открывается одна страница, и ее можно использовать.
  блокировать выпуски в устаревшем или ротационном режиме. Лос-репортаж citan las rutas candidatas
  Если вы намерены, то это поможет вам избежать ошибок в маршрутизации, связанных с документацией.

## Панель мониторинга и оповещения Grafana- `dashboards/grafana/docs_portal.json` публикует таблицу Grafana **Публикация на портале документации**.
  Включены важные панели:
  - *Отказы от шлюза (5 м)* США `torii_sorafs_gateway_refusals_total` область применения
    `profile`/`reason` для того, чтобы SRE обнаруживал мало политических действий или падения токенов.
  - *Результаты обновления кэша псевдонимов* и *Alias Proof Age p90* siguen
    `torii_sorafs_alias_cache_*`, чтобы продемонстрировать существующие доказательства фресок до того, как их вырезали.
    через DNS.
  - *Количество закрепленных манифестов реестра* y la estadistica *Количество активных псевдонимов* reflejan el
    отставание в реестре контактов и общее количество псевдонимов для того, чтобы можно было управлять
    релиз Када.
  - *Срок действия TLS шлюза (часы)* истекает, когда сертификат TLS шлюза публикации
    se acerca al vencimiento (затмение тревоги в течение 72 часов).
  - *Результаты SLA репликации* и *Журнал репликации* vigilan la telemetria
    `torii_sorafs_replication_*`, чтобы гарантировать, что все реплики собраны
    уровень GA после публикации.
- Используйте встроенные переменные растений (`profile`, `reason`), чтобы просмотреть их на карте.
  Откройте публикацию `docs.sora` или изучите изображения и все шлюзы.
- Маршрут PagerDuty использует панели приборной панели в качестве доказательств: las alertas
  `DocsPortal/GatewayRefusals`, `DocsPortal/AliasCache` и `DocsPortal/TLSExpiry`
  disparan cuando la serie cordiente cruza su umbral. Enlaza el runbook de la alerta
  Эта страница позволяет дежурному оператору воспроизводить запросы точно по Prometheus.

## Поньендоло в соединении

1. Durante `npm run build`, определите переменные для выпуска/аналитики и
   deja que el post-build emita `checksums.sha256`, `release.json` y
   `link-report.json`.
2. Выведите `npm run probe:portal` с именем хоста предварительного просмотра.
   `--expect-release` подключён к тегу Mismo. Следите за стандартным списком публикаций.
3. Извлеките `npm run check:links` для быстрого поворота карты сайта и архива.
   отчет в формате JSON, созданный совместно с артефактами предварительного просмотра. CI deja el ultimo reporte ru
   `artifacts/docs_portal/link-report.json`, чтобы вы могли получить пакет доказательств
   направляйте журналы сборки.
4. Enruta el endpoint de analitica hacia tu collector con Preservacion de Privacidad (Правдоподобно,
   ОТЕЛЬ принимает самостоятельное размещение и т. д.) и подтверждает, что все необходимые документы указаны в документации.
   Release для того, чтобы панели мониторинга корректно интерпретировали информацию.
5. CI и подключение к рабочим процессам предварительного просмотра/развертывания
   (`.github/workflows/docs-portal-preview.yml`,
   `.github/workflows/docs-portal-deploy.yml`), поскольку необходимы только местные пробные прогоны
   cubrir comportamiento especialo de secretos.