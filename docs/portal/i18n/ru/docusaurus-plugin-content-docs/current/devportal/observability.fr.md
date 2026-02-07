---
lang: ru
direction: ltr
source: docs/portal/docs/devportal/observability.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Наблюдательность и аналитика портала

Дорожная карта DOCS-SORA описывает аналитику, синтетические зонды и автоматизацию залогов
кассы для предварительной сборки. Эта записка документирует книгу «Пломбери книги с ближайшим порталом»
что операторы могут разветвлять мониторинг без разоблачения посетителей.

## Маркировка выпуска

- Definir `DOCS_RELEASE_TAG=<identifier>` (резервный вариант для `GIT_COMMIT` или `dev`) для вас
  построй порталь. La valeur est injectee dans `<meta name="sora-release">`
  для различных датчиков и панелей мониторинга для различных развертываний.
- `npm run build` род `build/release.json` (код сертификата
  `scripts/write-checksums.mjs`), который определяет тег, метку времени и т. д.
  Опция `DOCS_RELEASE_SOURCE`. Le meme fichier est embarque dans les artefacts Preview et
  ссылка на раппорт du link checker.

## Уважаемые аналитики частной жизни

- Конфигуратор `DOCS_ANALYTICS_ENDPOINT=<https://collector.example/ingest>` для активации
  ле трекер легер. Компоненты полезной нагрузки `{ event, path, locale, release, ts }`
  без метаданных реферера или IP, и `navigator.sendBeacon` может использовать то, что возможно.
  для предотвращения блокировки навигации.
- Контроллер echantillonnage с `DOCS_ANALYTICS_SAMPLE_RATE` (0-1). Сохранение трекера
  le dernier path envoye et n'emmet jamais d'evenements dupliques для навигации по мемам.
- Реализация найдена в `src/components/AnalyticsTracker.jsx` и установлена
  Глобальное управление через `src/theme/Root.js`.

## Синтетические зонды

- `npm run probe:portal` emet des requetes GET по курантским маршрутам
  (`/`, `/norito/overview`, `/reference/torii-swagger` и т. д.) и проверьте, что
  метатег `sora-release` соответствует `--expect-release` (или `DOCS_RELEASE_TAG`).
  Пример:

```bash
PORTAL_BASE_URL="https://docs.staging.sora" \
DOCS_RELEASE_TAG="preview-42" \
npm run probe:portal -- --expect-release=preview-42
```

Les echecs sont rapportes par path, ce qui facilite le Gate CD Sur le Succes des Prosces.

## Автоматизация дел по залоговым обязательствам

- `npm run check:links` отсканируйте `build/sitemap.xml`, убедитесь, что вы войдете в карту версий
  локальное описание (проверка резервных копий `index.html`) и др.
  `build/link-report.json` содержит метаданные выпуска, все, файлы и т. д.
  l'empreinte SHA-256 от `checksums.sha256` (разоблачение под именем `manifest.id`) до тех пор, пока
  rapport puisse etre rattache au manife d'artefact.
- Скрипт возвращает ненулевой код и каждую страницу, чтобы блокировать CI-мощность.
  Релизы на маршрутах устарели или кассеты. Les rapports citent les chemins candidates
  Тентес, это то, что помогает отслеживать регрессии маршрутизации просто в архиве документов.

## Панель мониторинга Grafana и оповещения- `dashboards/grafana/docs_portal.json` опубликовать таблицу Grafana **Публикация на портале документации**.
  Содержание следующих панно:
  - *Отказы шлюза (5 м)* используют параметр объема `torii_sorafs_gateway_refusals_total`.
    `profile`/`reason` для того, чтобы SRE обнаруживал неправильные политические толчки или
    выбросы жетонов.
  - *Результаты обновления кэша псевдонима* и *Alias Proof Age p90* соответствующие
    `torii_sorafs_alias_cache_*` для доказательства того, что доказательства существуют до разрезания
    через DNS.
  - *Количество контактов в манифесте реестра* и статистика *Количество активных псевдонимов* отражаются
    невыполненная регистрация PIN-регистрации и общее количество псевдонимов для управления вашим аудитором
    выпуск чака.
  - *Срок действия TLS шлюза (часы)* достигнут до истечения срока действия сертификата TLS.
    шлюз публикации (по тревоге в 72 часа).
  - *Результаты SLA репликации* и *Журнал репликации* с наблюдением за телеметрией
    `torii_sorafs_replication_*` для уверенности в том, что все реплики уважительны.
    niveau GA после публикации.
- Используйте встроенные переменные шаблона (`profile`, `reason`) для вашего концентратора.
  Профиль публикации `docs.sora` или просмотрите фотографии на ансамбле шлюзов.
- Для маршрутизации PagerDuty используются предварительные панели панели управления: предупреждения.
  `DocsPortal/GatewayRefusals`, `DocsPortal/AliasCache` и `DocsPortal/TLSExpiry`
  se dellenchent lorsque la serie corante depasse son seuil. Liez le runbook de
  Оповещение об этой странице для вызова по вызову может быть повторено по запросу Prometheus.

## Ассемблер для всех

1. Кулон `npm run build`, определение переменных среды выпуска/аналитики и т. д.
   laisser l'etape после сборки emettre `checksums.sha256`, `release.json` и др.
   `link-report.json`.
2. Executer `npm run probe:portal` с предварительным просмотром
   `--expect-release` ветка тега мема. Сохранение стандарта для контрольного списка
   де издательство.
3. Исполнитель `npm run check:links` для ускорения вывода файлов карты сайта.
   и архивируйте формат JSON с предварительным просмотром артефактов. La CI свергнуть ле
   Более глубокие взаимоотношения в `artifacts/docs_portal/link-report.json` для управления
   puisse telecharger le Bundle de Preuves, направляющий загрузку журналов сборки.
4. Маршрутизатор для анализа конечных точек и сбора данных о частной жизни (правдоподобно,
   OTEL принимает автоматические запросы и т. д.) и гарантирует, что все документы будут сохранены
   после того, как панели мониторинга интерпретируют корректирующие объемы.
5. Проводите эти этапы с помощью CI с помощью предварительного просмотра/развертывания рабочих процессов.
   (`.github/workflows/docs-portal-preview.yml`,
   `.github/workflows/docs-portal-deploy.yml`), не проводите пробные прогоны в местах, где не должно быть места
   что такое особое поведение и секреты.