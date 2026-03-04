---
lang: ru
direction: ltr
source: docs/devportal/try-it.md
status: complete
translator: manual
source_hash: 791d88296d9e52d3272ce3ac324e498fa3c622c323edc8c988302efe5092f0b4
source_last_modified: "2026-02-03T00:00:00Z"
translation_last_reviewed: 2026-02-03
---

<!-- Русский перевод docs/devportal/try-it.md (Try It Sandbox Guide) -->

---
title: Гайд по sandbox’у «Try It»
summary: Как запустить proxy для Torii staging и sandbox портала разработчика.
---

В портале разработчика есть консоль «Try it» для REST‑API Torii. В этом гайде описано,
как запустить вспомогательный proxy и подключить консоль к staging‑gateway’ю, не
раскрывая креденшалы.

## Предпосылки

- Клон репозитория Iroha (корень workspace’а).
- Node.js 18.18+ (соответствует baseline’у портала).
- Endpoint Torii, доступный с вашей рабочей станции (staging или локальный).

## 1. Генерация snapshot’а OpenAPI (опционально)

Консоль переиспользует тот же OpenAPI‑payload, что и reference‑страницы портала. Если вы
изменяли маршруты Torii, перегенерируйте snapshot:

```bash
cargo xtask openapi
```

Задача запишет `docs/portal/static/openapi/torii.json`.

## 2. Запуск Try It‑proxy

Из корня репозитория:

```bash
cd docs/portal

export TRYIT_PROXY_TARGET="https://torii.staging.sora"
export TRYIT_PROXY_ALLOWED_ORIGINS="http://localhost:3000"
# Необязательные значения по умолчанию
export TRYIT_PROXY_BEARER="sora-dev-token"
export TRYIT_PROXY_LISTEN="127.0.0.1:8787"

npm run tryit-proxy
```

### Переменные окружения

| Переменная | Описание |
|-----------|----------|
| `TRYIT_PROXY_TARGET` | Базовый URL Torii (обязательно). |
| `TRYIT_PROXY_ALLOWED_ORIGINS` | Список origin’ов (через запятую), которым разрешено пользоваться proxy (по умолчанию `http://localhost:3000`). |
| `TRYIT_PROXY_BEARER` | Необязательный bearer‑token по умолчанию, добавляемый ко всем proxied‑запросам. |
| `TRYIT_PROXY_ALLOW_CLIENT_AUTH` | Установите `1`, чтобы пробрасывать заголовок `Authorization` клиента без изменений. |
| `TRYIT_PROXY_RATE_LIMIT` / `TRYIT_PROXY_RATE_WINDOW_MS` | Настройки in‑memory‑rate‑limiter’а (по умолчанию 60 запросов за 60 с). |
| `TRYIT_PROXY_MAX_BODY` | Максимальный размер тела запроса (в байтах, по умолчанию 1 MiB). |
| `TRYIT_PROXY_TIMEOUT_MS` | Таймаут upstream‑запросов к Torii (по умолчанию 10 000 мс). |

Proxy экспонирует:

- `GET /healthz` — проверка готовности (readiness).
- `/proxy/*` — proxied‑запросы с сохранением path и query‑строки.

## 3. Запуск портала

В отдельном терминале:

```bash
cd docs/portal
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
npm run start
```

Откройте `http://localhost:3000/api/overview` и используйте консоль Try It. Те же
переменные окружения настраивают встраиваемые Swagger UI и RapiDoc.

## 4. Запуск unit‑тестов

В proxy есть быстрая suite unit‑тестов на Node:

```bash
npm run test:tryit-proxy
```

Тесты покрывают разбор адресов, обработку origin’ов, rate‑limiting и подстановку
bearer‑токенов.

## 5. Пробы и метрики

Используйте встроенный probe, чтобы проверять `/healthz` и тестовый endpoint:

```bash
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" \
TRYIT_PROXY_SAMPLE_PATH="/v1/status" \
npm run probe:tryit-proxy
```

Настройки:

- `TRYIT_PROXY_SAMPLE_PATH` — опциональный Torii‑маршрут (без `/proxy`), который надо дернуть.
- `TRYIT_PROXY_SAMPLE_METHOD` — по умолчанию `GET`; переключите на `POST` для write‑маршрутов.
- `TRYIT_PROXY_PROBE_TOKEN` — временный bearer‑token для примерного запроса.
- `TRYIT_PROXY_PROBE_TIMEOUT_MS` — переопределяет дефолтный таймаут 5 с.
- `TRYIT_PROXY_PROBE_METRICS_FILE` — файл в формате Prometheus textfile с `probe_success`/`probe_duration_seconds`.
- `TRYIT_PROXY_PROBE_LABELS` — список `ключ=значение` через запятую, дополняющий labels (по умолчанию `job=tryit-proxy` и `instance=<URL proxy>`).

Если указать `TRYIT_PROXY_PROBE_METRICS_FILE`, скрипт будет атомарно перезаписывать файл,
поэтому node_exporter/textfile‑collector всегда увидит целый payload. Пример:

```bash
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" \
TRYIT_PROXY_PROBE_METRICS_FILE="/var/lib/node_exporter/textfile_collector/tryit.prom" \
TRYIT_PROXY_PROBE_LABELS="job=tryit-proxy,cluster=staging" \
npm run probe:tryit-proxy
```

Далее отправьте метрики в Prometheus и подключите rule, которая будет алертить,
когда `probe_success` станет `0`.

## 6. Чек‑лист по ужесточению (hardening) для продакшена

Перед тем как использовать proxy вне локальной разработки:

- Терминируйте TLS перед proxy (reverse‑proxy или управляемый gateway).
- Настройте структурированное логирование и отправку логов в pipeline’ы наблюдаемости.
- Регулярно ротируйте bearer‑токены и храните их в менеджере секретов.
- Мониторьте endpoint `/healthz` proxy и агрегируйте latency‑метрики.
- Синхронизируйте rate‑лимиты с staging‑квотами Torii; при необходимости настройте поведение
  `Retry-After`, чтобы корректно сигнализировать throttling клиентам.
