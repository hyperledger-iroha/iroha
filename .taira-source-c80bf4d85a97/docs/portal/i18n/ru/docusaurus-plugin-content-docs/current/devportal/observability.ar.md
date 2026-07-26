---
lang: ru
direction: ltr
source: docs/portal/docs/devportal/observability.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# مراقبة البوابة والتحليلات

Установите DOCS-SORA для проверки и проверки зондов. Создайте новую сборку.
Он сказал, что в 2017 году он был главой государства-члена Совета Безопасности ООН. Он выступил в роли Дэниэла Пьера Бэнгера.

## وسم الاصدار

- `DOCS_RELEASE_TAG=<identifier>` (от `GIT_COMMIT` до `dev`)
  بناء البوابة. Создан для `<meta name="sora-release">`
  Он исследует информационные панели в Нью-Йорке.
- `npm run build` ينتج `build/release.json` (يكتبه
  `scripts/write-checksums.mjs`) ويصف الختياري.
  Он был показан в фильме "Старый мир" в фильме "Тридцатье" в Нью-Йорке.

## تحليلات تحافظ على الخصوصية

- اضبط `DOCS_ANALYTICS_ENDPOINT=<https://collector.example/ingest>` для дальнейшего использования.
  Используйте `{ event, path, locale, release, ts }` для метаданных, IP и IP-адресов.
  `navigator.sendBeacon` находится в центре внимания.
- Было выполнено в режиме `DOCS_ANALYTICS_SAMPLE_RATE` (0-1). يخزن المتتبع اخر path مرسل ولا يرسل
  احداثا مكررة لنفس التنقل.
- Создан для `src/components/AnalyticsTracker.jsx` и установлен в `src/theme/Root.js`.

## зонды

- `npm run probe:portal` يرسل طلبات GET الى المسارات الشائعة
  (`/`, `/norito/overview`, `/reference/torii-swagger`, وغيرها)
  `sora-release` или `--expect-release` (также `DOCS_RELEASE_TAG`). Название:

```bash
PORTAL_BASE_URL="https://docs.staging.sora" \
DOCS_RELEASE_TAG="preview-42" \
npm run probe:portal -- --expect-release=preview-42
```

Он находится в разделе «Путь», а компакт-диск с датчиками.

## اتتمة الروابط المكسورة

- `npm run check:links` на `build/sitemap.xml`, который находится в центре города.
  (для резервных копий `index.html`) и `build/link-report.json` для резервного копирования.
  Метаданные хранятся в формате SHA-256 с именем `checksums.sha256`.
  (написано на `manifest.id`) Зарегистрируйтесь в манифесте манифеста.
- Он был назначен президентом США Джоном Сингхом, сотрудником CI в штате Калифорния. عند وجود مسارات قديمة او
  مكسورة. Вы можете получить информацию о том, как это сделать, если вы хотите, чтобы это произошло. تراجعات التوجيه
  Воспользуйтесь документами.

## Grafana والتنبيهات- `dashboards/grafana/docs_portal.json` — ссылка Grafana **Публикация на портале документации**.
  Важная информация:
  - *Отказы шлюза (5 м)* يستخدم `torii_sorafs_gateway_refusals_total` بنطاق
    `profile`/`reason` был установлен в SRE в штате Калифорния.
  - *Результаты обновления кэша псевдонимов* и *Alias Proof Age p90* تتبع
    `torii_sorafs_alias_cache_*` заблокировал доказательства того, что DNS обрезан.
  - *Подсчет манифеста реестра контактов* واحصائية *Счетчик активных псевдонимов* Зарегистрируйте невыполненный реестр контактов
    Он псевдоним Тэтчер Тэхен الحوكمة, которого зовут Тэрри в اصدار.
  - *Срок действия TLS шлюза (часы)* Вы можете получить сертификат TLS в любое время.
    (Движение 72 ч).
  - *Результаты соглашения об уровне обслуживания репликации* и *Журнал репликации* Телеметрия
    `torii_sorafs_replication_*` был создан в рамках проекта GA.
- Установите флажок для проверки (`profile`, `reason`). `docs.sora`
  В 2007 году он был отправлен в Нью-Йорк.
- Приложение PagerDuty в Лос-Анджелесе. Панель управления: التنبيهات
  `DocsPortal/GatewayRefusals`, `DocsPortal/AliasCache`, و`DocsPortal/TLSExpiry`
  Он сказал, что хочет сделать это. اربط runbook التنبيه بهذه الصفحة حتى يتمكن
  Дежурный оператор по вызову Prometheus.

## جمع الخطوات

1. Установите `npm run build`, чтобы получить доступ к выпуску/аналитике, который будет доступен для просмотра.
   `checksums.sha256`, `release.json`, و`link-report.json`.
2. Введите `npm run probe:portal` для имени хоста.
   `--expect-release` отключен от сети. Это стандартное сообщение.
3. Создайте `npm run check:links` для просмотра карты сайта и создания файла JSON.
   Это не так. Выполнил команду CI для `artifacts/docs_portal/link-report.json`.
   Он был убит Стивом Дэвисом в 2007 году.
4. Получение конечной точки в режиме ожидания (Plausible, прием данных от OTEL). да)
   Он был показан в фильме "Лейт-Стрит" в Лос-Анджелесе. بدقة.
5. CI позволяет управлять рабочими процессами/процессами.
   (`.github/workflows/docs-portal-preview.yml`,
   `.github/workflows/docs-portal-deploy.yml`) и нажмите на него в разделе «Обзор».
   Сыграл он.