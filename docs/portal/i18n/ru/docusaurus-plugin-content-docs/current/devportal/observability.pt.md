---
lang: ru
direction: ltr
source: docs/portal/docs/devportal/observability.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Портал наблюдения и электронной аналитики

Дорожная карта DOCS-SORA требует аналитики, синтетических зондов и автоматического автоматического создания ссылок
для каждой сборки предварительного просмотра. Это примечание к инфраструктуре, которая находится в компании или на портале.
для того, чтобы операторы подключались к монитору, чтобы их могли видеть посетители.

## Маркировка выпуска

- Defina `DOCS_RELEASE_TAG=<identifier>` (резервный вариант `GIT_COMMIT` или `dev`) или
  построить портал. О доблести и вкладе в `<meta name="sora-release">`
  для проверки и создания информационных панелей для различения развертываний.
- `npm run build` испускает `build/release.json` (скрипт для
  `scripts/write-checksums.mjs`) указан тег, метка времени e o
  `DOCS_RELEASE_SOURCE` опционально. O mesmo arquivo e empacotado nos artefatos de Preview e
  Referenciado Pelo relatorio делает проверку ссылок.

## Аналитика с сохранением конфиденциальности

- Настройте параметр `DOCS_ANALYTICS_ENDPOINT=<https://collector.example/ingest>`.
  навык или уровень трекера. Полезные нагрузки рассматриваются `{ event, path, locale, release, ts }`
  Сэм метаданные реферера или IP-адреса, а также `navigator.sendBeacon` и всегда используются, что возможно.
  чтобы избежать блокировки навигаторов.
- Управление выборкой через `DOCS_ANALYTICS_SAMPLE_RATE` (0-1). О трекер Армазена
  o последний путь отправлен и нет дубликатов событий, чтобы добраться до одного места.
- Реализация в `src/components/AnalyticsTracker.jsx` e e montada
  глобально через `src/theme/Root.js`.

## Синтетические зонды

- `npm run probe:portal` dispara запрашивает GET contra rotas comuns
  (`/`, `/norito/overview`, `/reference/torii-swagger` и т. д.) и проверьте наличие
  метатег `sora-release` соответствует `--expect-release` (или `DOCS_RELEASE_TAG`).
  Пример:

```bash
PORTAL_BASE_URL="https://docs.staging.sora" \
DOCS_RELEASE_TAG="preview-42" \
npm run probe:portal -- --expect-release=preview-42
```

Если вы получили отчеты о пути, вам будет легко получить доступ к компакт-диску или успешно выполнить зондирование.

## Автоматическое восстановление ссылок

- `npm run check:links` отличается от `build/sitemap.xml`, гарантируя, что каждый раз будет введена карта для
  локальный архив (резервные резервные копии `index.html`), и сохраните
  `build/link-report.json` содержит метаданные выпуска, полные, измененные и отпечатки.
  SHA-256 от `checksums.sha256` (объявлен как `manifest.id`) для того, чтобы можно было установить связь
  ser ligado ao manifeto do artefato.
- Если завершение сценария с кодом nao-zero, когда произошла ошибка на странице, можно заблокировать CI.
  выпускает вращающиеся противогазы или кебрады. Os relatorios citam os caminhos candidatos tenados,
  o que ajuda ajuda Restrear reressoes de roteamento de volta for a avore de docs.

## Dashboard Grafana и оповещения- `dashboards/grafana/docs_portal.json` публикуется на форуме Grafana **Публикация на портале документации**.
  Включены следующие моменты:
  - *Отказы от шлюза (5 м)* США `torii_sorafs_gateway_refusals_total` com escopo
    `profile`/`reason` для того, чтобы SRE обнаруживали руины политики или фальсификацию токенов.
  - *Результаты обновления кэша псевдонимов* e *Alias Proof Age p90* acompanham
    `torii_sorafs_alias_cache_*`, чтобы доказать, что последние существуют до того, как вырезать
    через DNS.
  - *Подсчет манифеста PIN-кода* и статистика *Количество активных псевдонимов* espelham o
    отставание в регистрации контактов и общее количество псевдонимов для управления, которое можно проверить
    релиз Када.
  - *Срок действия TLS шлюза (часы)* выдается, когда сертификат TLS шлюза публикации
    будет примерно в 72 часа.
  - *Результаты SLA репликации* и *Журнал репликации* в сочетании с телеметрией
    `torii_sorafs_replication_*` для гарантии того, что все копии будут в наличии или
    Патамар Г.А. в связи с публичной публикацией.
- Используйте в качестве вариантов вставки шаблона (`profile`, `reason`), чтобы не было необходимости.
  Публикация `docs.sora` или исследование фотографий всех шлюзов.
- Маршрутизация PagerDuty USA или Paineis на панели управления осуществляется как доказательство: alertas
  `DocsPortal/GatewayRefusals`, `DocsPortal/AliasCache` и `DocsPortal/TLSExpiry`
  disparam quando a serie correente ultrapassa seus limiares. Лига или ранбук
  Сделайте предупреждение на этой странице, чтобы повторять запросы по вызову по мере выполнения запросов Prometheus.

## Юнтандо тудо

1. Durante `npm run build`, определенный как варианты окружения выпуска/аналитики и
   deixe o pos-build emitir `checksums.sha256`, `release.json` e
   `link-report.json`.
2. Rode `npm run probe:portal` напротив имени хоста предварительного просмотра com
   `--expect-release` подключается к основному тегу. Используйте стандартный вывод для контрольного списка публикации.
3. Используйте `npm run check:links`, чтобы быстро получить доступ к карте сайта и архиву.
   Отношения JSON объединены с объектами предварительного просмотра. Депозит CI o
   последняя связь с `artifacts/docs_portal/link-report.json` для управления
   baixe или комплект доказательств непосредственно для журналов сборки.
4. Включение конечной точки аналитики для сохранения конфиденциальности (правдоподобно,
   ОТЕЛЬ принимает самостоятельное размещение и т. д.) и гарантирует, что налоги на недвижимость будут документированы для
   Release для того, чтобы панели мониторинга интерпретировали соответствующие тома.
5. CI и подключение не требуют рабочих процессов предварительного просмотра/развертывания.
   (`.github/workflows/docs-portal-preview.yml`,
   `.github/workflows/docs-portal-deploy.yml`), entao os сухие прогоны на месте, поэтому точны
   Соблюдайте особые правила разделения.