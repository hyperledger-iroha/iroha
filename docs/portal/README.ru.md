---
lang: ru
direction: ltr
source: docs/portal/README.md
status: complete
translator: manual
source_hash: 4b0d6c295c7188355e2c03d7c8240271da147095ff557fae2152f42e27bd17fa
source_last_modified: "2025-11-14T04:43:03.939564+00:00"
translation_last_reviewed: 2025-11-14
---

<!-- Русский перевод docs/portal/README.md (SORA Nexus Developer Portal) -->

# Портал разработчика SORA Nexus

В этом каталоге находится workspace Docusaurus для интерактивного портала разработчика.
Портал агрегирует гайды по Norito, SDK‑quickstart’ы и OpenAPI‑справочник, сгенерированный
`cargo xtask openapi`, оформляя их в едином стиле SORA Nexus, используемом по всему
документационному стеку.

## Предпосылки

- Node.js 18.18 или новее (baseline Docusaurus v3).
- Yarn 1.x или npm ≥ 9 для управления пакетами.
- Rust‑toolchain (используется скриптом синхронизации OpenAPI).

## Bootstrap

```bash
cd docs/portal
npm install    # или yarn install
```

## Доступные скрипты

| Команда | Описание |
|--------|----------|
| `npm run start` / `yarn start` | Запуск локального dev‑сервера с live‑reload (по умолчанию `http://localhost:3000`). |
| `npm run build` / `yarn build` | Сборка production‑build’а в `build/`. |
| `npm run serve` / `yarn serve` | Обслуживание последнего build’а локально (полезно для smoke‑тестов). |
| `npm run docs:version -- <label>` | Снятие snapshot’а текущей документации в `versioned_docs/version-<label>` (обёртка над `docusaurus docs:version`). |
| `npm run sync-openapi` / `yarn sync-openapi` | Регенирация `static/openapi/torii.json` через `cargo xtask openapi` (передайте `--mirror=<label>`, чтобы скопировать spec в дополнительные snapshots). |
| `npm run tryit-proxy` | Запуск staging‑proxy’а, обслуживающего консоль «Try it» (см. конфигурацию ниже). |
| `npm run probe:tryit-proxy` | Прогон `/healthz` + пробного запроса против proxy (helper для CI/мониторинга). |
| `npm run manage:tryit-proxy -- <update|rollback>` | Обновление или откат целевого `.env` proxy с поддержкой backup’ов. |
| `npm run sync-i18n` | Проверка наличия переводческих stubs для японского, иврита, испанского, португальского, французского, русского, арабского и урду под `i18n/`. |
| `npm run sync-norito-snippets` | Регенирация отобранных Kotodama‑примеров + скачиваемых snippets (также вызывается автоматически плагином dev‑сервера). |
| `npm run test:tryit-proxy` | Прогон unit‑тестов proxy через Node‑runner (`node --test`). |

Скрипт синхронизации OpenAPI требует, чтобы `cargo xtask openapi` был доступен из корня
репозитория; он генерирует детерминированный JSON‑файл в `static/openapi/` и теперь
ожидает, что Torii‑router будет отдавать «живую» spec (используйте
`cargo xtask openapi --allow-stub` только для аварийных placeholder’ов).

## Версионирование docs и OpenAPI‑snapshots

- **Создание версии docs.** Запустите `npm run docs:version -- 2025-q3` (или любой
  согласованный label). Закоммитьте сгенерированные `versioned_docs/version-<label>`,
  `versioned_sidebars` и `versions.json`. Выпадающий список версий в navbar автоматически
  подхватит новый snapshot.
- **Синхронизация OpenAPI‑артефактов.** После создания версии обновите каноническую spec и
  manifest с помощью `cargo xtask openapi --sign <путь-к-ed25519‑ключу>`, затем
  сохраните согласованный snapshot через
  `npm run sync-openapi -- --version=2025-q3 --mirror=current --latest`. Скрипт запишет
  `static/openapi/versions/2025-q3/torii.json`, скопирует spec в
  `versions/current/torii.json`, обновит `versions.json`, перегенерирует
  `/openapi/torii.json` и клонирует подписанный `manifest.json` в каждый каталог версии,
  чтобы исторические specs имели одинаковые метаданные происхождения. Можно передать
  несколько флагов `--mirror=<label>`, чтобы скопировать spec в другие исторические
  snapshots.
- **Ожидания CI.** Коммиты, затрагивающие docs, должны включать при необходимости bump
  версии и обновлённые OpenAPI‑snapshots, чтобы панели Swagger/RapiDoc/Redoc могли
  переключаться между историческими specs без fetch‑ошибок.
- **Контроль manifest’а.** Скрипт `sync-openapi` копирует manifests только если `manifest.json`
  на диске совпадает со свежесгенерированной spec. Если копирование пропускается,
  перезапустите `cargo xtask openapi --sign <ключ>`, чтобы обновить канонический
  manifest, а затем повторите синхронизацию, чтобы versioned‑snapshots получили
  подписанные метаданные. Скрипт `ci/check_openapi_spec.sh` повторно запускает
  генератор и валидирует manifest перед разрешением merge’ей.

## Структура

```text
docs/portal/
├── docs/                 # Markdown/MDX‑контент портала
├── i18n/                 # Локальные overrides (ja/he), сгенерированные sync-i18n
├── src/                  # React‑страницы и компоненты (scaffolding)
├── static/               # Статические assets (включая OpenAPI‑JSON)
├── scripts/              # Вспомогательные скрипты (синхронизация OpenAPI)
├── docusaurus.config.js  # Основная конфигурация сайта
└── sidebars.js           # Модель навигации/sidebars
```

### Конфигурация Try It‑proxy

Sandbox «Try it» проксирует запросы через `scripts/tryit-proxy.mjs`. Перед запуском
настройте proxy через переменные окружения:

```bash
export TRYIT_PROXY_TARGET="https://torii.staging.sora"
export TRYIT_PROXY_ALLOWED_ORIGINS="http://localhost:3000"
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
```

В staging/production задавайте эти переменные в системе конфигурации вашей платформы
(секреты GitHub Actions, переменные окружения оркестратора контейнеров и т.п.).

### Preview‑URL’ы и release‑notes

- Публичный beta‑preview: `https://docs.iroha.tech/`
- GitHub также публикует build в окружении **github-pages** для каждого деплоя.
- Pull‑requests, затрагивающие контент портала, включают artefact’ы Actions
  (`docs-portal-preview`, `docs-portal-preview-metadata`), содержащие собранный сайт,
  manifest контрольных сумм, сжатый архив и descriptor; ревьюеры могут скачать bundle,
  локально открыть `index.html` и сверить checksums до расшаривания preview. Workflow
  добавляет сводный комментарий (hash’и manifest/архива + статус SoraFS) к каждому PR,
  давая быструю индикацию, что проверка прошла успешно.
- Используйте
  `./docs/portal/scripts/preview_verify.sh --build-dir <распакованный build> --descriptor <descriptor> --archive <архив>` после скачивания preview‑bundle, чтобы убедиться, что артефакты совпадают с теми, что были созданы CI, перед тем как делиться ссылкой наружу.
- При подготовке release‑notes или статусных апдейтов ссылайтесь на preview‑URL, чтобы
  внешние ревьюеры могли просматривать актуальный snapshot портала без клонирования
  репозитория.
- Согласовывайте волны preview через
  `docs/portal/docs/devportal/preview-invite-flow.md` совместно с
  `docs/portal/docs/devportal/reviewer-onboarding.md`, чтобы каждое приглашение, экспорт
  телеметрии и шаг offboarding опирались на один и тот же набор доказательств.

