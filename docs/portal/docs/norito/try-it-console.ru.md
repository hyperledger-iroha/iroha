---
lang: ru
direction: ltr
source: docs/portal/docs/norito/try-it-console.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d38bbaedc2d75fc67e3f027c46834fb5a94d5fccd845014f829f3defcbc782b5
source_last_modified: "2025-11-20T15:21:55.938588+00:00"
translation_last_reviewed: 2025-12-30
---

---
title: Консоль Try-It для Norito
description: Используйте прокси портала разработчиков и виджеты Swagger и RapiDoc, чтобы отправлять реальные запросы Torii / Norito-RPC напрямую с сайта документации.
---

Портал объединяет три интерактивные поверхности, которые пересылают трафик в Torii:

- **Swagger UI** на `/reference/torii-swagger` отображает подписанную спецификацию OpenAPI и автоматически переписывает запросы через прокси, когда задан `TRYIT_PROXY_PUBLIC_URL`.
- **RapiDoc** на `/reference/torii-rapidoc` публикует ту же схему с загрузкой файлов и селекторами типа контента, которые хорошо подходят для `application/x-norito`.
- **Try it sandbox** на странице обзора Norito предоставляет легкую форму для ad hoc REST-запросов и OAuth-входа через устройство.

Все три виджета отправляют запросы в локальный **Try-It прокси** (`docs/portal/scripts/tryit-proxy.mjs`). Прокси проверяет, что `static/openapi/torii.json` соответствует подписанному дайджесту в `static/openapi/manifest.json`, применяет ограничитель скорости, вырезает заголовки `X-TryIt-Auth` из логов и помечает каждый upstream-вызов `X-TryIt-Client`, чтобы операторы Torii могли аудитировать источники трафика.

## Запустите прокси

```bash
cd docs/portal
npm install
export TRYIT_PROXY_TARGET="https://torii.devnet.sora.example"
export TRYIT_PROXY_ALLOWED_ORIGINS="http://localhost:3000"
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
# Optional, use short-lived tokens only:
export TRYIT_PROXY_BEARER="Bearer eyJhbGciOi..."
export TRYIT_PROXY_CLIENT_ID="docs-portal"
export DOCS_TRYIT_ALLOW_DEFAULT_BEARER=1
npm run tryit-proxy
```

- `TRYIT_PROXY_TARGET` — базовый URL Torii, который вы хотите проверить.
- `TRYIT_PROXY_ALLOWED_ORIGINS` должен включать все источники портала (локальный сервер, продакшн-хостнейм, URL предпросмотра), которые должны встраивать консоль.
- `TRYIT_PROXY_PUBLIC_URL` используется в `docusaurus.config.js` и внедряется в виджеты через `customFields.tryIt`.
- `TRYIT_PROXY_BEARER` загружается только при `DOCS_TRYIT_ALLOW_DEFAULT_BEARER=1`; иначе пользователи должны предоставить свой токен через консоль или OAuth device flow.
- `TRYIT_PROXY_CLIENT_ID` задает тег `X-TryIt-Client`, который передается в каждом запросе.
  Передавать `X-TryIt-Client` из браузера разрешено, но значения обрезаются
  и отклоняются, если содержат управляющие символы.

При запуске прокси выполняет `verifySpecDigest` и завершает работу с подсказкой по исправлению, если манифест устарел. Запустите `npm run sync-openapi -- --latest`, чтобы скачать самую новую спецификацию Torii, или передайте `TRYIT_PROXY_ALLOW_STALE_SPEC=1` для экстренных обходов.

Чтобы обновить или откатить цель прокси без ручного редактирования файлов окружения, используйте помощник:

```bash
npm run manage:tryit-proxy -- update --target https://new.torii.example
npm run manage:tryit-proxy -- rollback
```

## Подключите виджеты

Запускайте портал после того, как прокси начал слушать:

```bash
cd docs/portal
TRYIT_PROXY_PUBLIC_URL="http://localhost:8787" npm run start
```

`docusaurus.config.js` предоставляет следующие настройки:

| Переменная | Назначение |
| --- | --- |
| `TRYIT_PROXY_PUBLIC_URL` | URL, внедряемый в Swagger, RapiDoc и Try it sandbox. Оставьте пустым, чтобы скрыть виджеты в несанкционированных превью. |
| `TRYIT_PROXY_DEFAULT_BEARER` | Опциональный токен по умолчанию, хранящийся в памяти. Требует `DOCS_TRYIT_ALLOW_DEFAULT_BEARER=1` и HTTPS-only CSP guard (DOCS-1b), если только вы не передадите `DOCS_SECURITY_ALLOW_INSECURE=1` локально. |
| `DOCS_OAUTH_*` | Включает OAuth device flow (`OAuthDeviceLogin` component), чтобы ревьюеры могли выпускать краткоживущие токены не покидая портал. |

Когда переменные OAuth присутствуют, sandbox отображает кнопку **Sign in with device code**, которая проходит через настроенный Auth сервер (см. `config/security-helpers.js` для точного формата). Токены, выданные через device flow, кэшируются только в сессии браузера.

## Отправка пейлоадов Norito-RPC

1. Соберите `.norito` пейлоад с помощью CLI или сниппетов из [Norito quickstart](./quickstart.md). Прокси пересылает тела `application/x-norito` без изменений, поэтому можно использовать тот же артефакт, который вы отправили бы с `curl`.
2. Откройте `/reference/torii-rapidoc` (предпочтительно для бинарных пейлоадов) или `/reference/torii-swagger`.
3. Выберите нужный snapshot Torii из выпадающего списка. Снимки подписаны; панель показывает дайджест манифеста, записанный в `static/openapi/manifest.json`.
4. Выберите тип контента `application/x-norito` в панели "Try it", нажмите **Choose File** и выберите свой пейлоад. Прокси переписывает запрос на `/proxy/v1/pipeline/submit` и помечает его `X-TryIt-Client=docs-portal-rapidoc`.
5. Чтобы скачать ответы Norito, установите `Accept: application/x-norito`. Swagger/RapiDoc показывают селектор заголовка в той же панели и стримят бинарные данные обратно через прокси.

Для маршрутов только с JSON встроенный Try it sandbox часто быстрее: введите путь (например, `/v1/accounts/i105.../assets`), выберите HTTP-метод, вставьте JSON-тело при необходимости и нажмите **Send request**, чтобы сразу увидеть заголовки, длительность и пейлоады.

## Устранение неполадок

| Симптом | Возможная причина | Решение |
| --- | --- | --- |
| В консоли браузера появляются ошибки CORS или sandbox предупреждает, что URL прокси отсутствует. | Прокси не запущен или источник не в белом списке. | Запустите прокси, убедитесь, что `TRYIT_PROXY_ALLOWED_ORIGINS` покрывает хост портала, и перезапустите `npm run start`. |
| `npm run tryit-proxy` завершился с “digest mismatch”. | OpenAPI bundle Torii изменился на стороне источника. | Запустите `npm run sync-openapi -- --latest` (или `--version=<tag>`) и повторите попытку. |
| Виджеты возвращают `401` или `403`. | Токен отсутствует, истек или имеет недостаточные права. | Используйте OAuth device flow или вставьте валидный bearer токен в sandbox. Для статических токенов нужно экспортировать `DOCS_TRYIT_ALLOW_DEFAULT_BEARER=1`. |
| `429 Too Many Requests` от прокси. | Превышен лимит запросов на IP. | Увеличьте `TRYIT_PROXY_RATE_LIMIT`/`TRYIT_PROXY_RATE_WINDOW_MS` для доверенных окружений или ограничьте тестовые скрипты. Все отказы по rate limit увеличивают `tryit_proxy_rate_limited_total`. |

## Наблюдаемость

- `npm run probe:tryit-proxy` (обертка над `scripts/tryit-proxy-probe.mjs`) вызывает `/healthz`, опционально проверяет примерный маршрут и выводит текстовые файлы Prometheus для `probe_success` / `probe_duration_seconds`. Настройте `TRYIT_PROXY_PROBE_METRICS_FILE` для интеграции с node_exporter.
- Установите `TRYIT_PROXY_METRICS_LISTEN=127.0.0.1:9798`, чтобы открыть счетчики (`tryit_proxy_requests_total`, `tryit_proxy_rate_limited_total`, `tryit_proxy_upstream_failures_total`) и гистограммы задержек. Дашборд `dashboards/grafana/docs_portal.json` читает эти метрики, чтобы обеспечивать SLOs DOCS-SORA.
- Логи рантайма выводятся в stdout. Каждая запись включает id запроса, upstream-статус, источник аутентификации (`default`, `override` или `client`) и длительность; секреты редактируются до вывода.

Если нужно проверить, что пейлоады `application/x-norito` доходят до Torii без изменений, запустите набор Jest (`npm test -- tryit-proxy`) или проверьте fixtures в `docs/portal/scripts/__tests__/tryit-proxy.test.mjs`. Регрессионные тесты покрывают сжатые Norito бинарники, подписанные OpenAPI манифесты и пути понижения прокси, чтобы разворачивания NRPC сохраняли постоянный след доказательств.
