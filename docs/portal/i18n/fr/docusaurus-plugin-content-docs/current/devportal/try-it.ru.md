---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/try-it.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# Песочница Try It

Портал разработчика включает опциональную консоль "Try it", чтобы вы могли вызывать endpoints Torii, не покидая документацию. Консоль проксирует запросы через встроенный proxy, чтобы браузеры обходили ограничения CORS, одновременно соблюдая rate limits и аутентификацию.

## Предварительные условия

- Node.js 18.18 или новее (соответствует требованиям build портала)
- Сетевой доступ к staging окружению Torii
- bearer token, который может вызывать нужные маршруты Torii

Вся настройка proxy выполняется через переменные окружения. В таблице ниже перечислены основные knobs:

| Variable | Назначение | Default |
| --- | --- | --- |
| `TRYIT_PROXY_TARGET` | Базовый URL Torii, куда proxy пересылает запросы | **Required** |
| `TRYIT_PROXY_LISTEN` | Адрес прослушивания для локальной разработки (формат `host:port` или `[ipv6]:port`) | `127.0.0.1:8787` |
| `TRYIT_PROXY_ALLOWED_ORIGINS` | Список origin-ов, которым разрешено обращаться к proxy (через запятую) | `http://localhost:3000` |
| `TRYIT_PROXY_CLIENT_ID` | Идентификатор, добавляемый в `X-TryIt-Client` для каждого upstream запроса | `docs-portal` |
| `TRYIT_PROXY_BEARER` | Bearer token по умолчанию, пересылаемый в Torii | _empty_ |
| `TRYIT_PROXY_ALLOW_CLIENT_AUTH` | Разрешить пользователям передавать собственный token через `X-TryIt-Auth` | `0` |
| `TRYIT_PROXY_MAX_BODY` | Максимальный размер тела запроса (bytes) | `1048576` |
| `TRYIT_PROXY_TIMEOUT_MS` | Upstream timeout в миллисекундах | `10000` |
| `TRYIT_PROXY_RATE_LIMIT` | Запросов в окне rate limiting на IP клиента | `60` |
| `TRYIT_PROXY_RATE_WINDOW_MS` | Скользящее окно rate limiting (ms) | `60000` |
| `TRYIT_PROXY_METRICS_LISTEN` | Адрес прослушивания для метрик Prometheus (`host:port` или `[ipv6]:port`) | _empty (disabled)_ |
| `TRYIT_PROXY_METRICS_PATH` | HTTP путь для endpoint метрик | `/metrics` |

Proxy также exposes `GET /healthz`, возвращает структурированные JSON ошибки и редактирует bearer tokens в логах.

Включите `TRYIT_PROXY_ALLOW_CLIENT_AUTH=1`, если proxy доступен пользователям docs, чтобы панели Swagger и RapiDoc могли пересылать bearer tokens, введенные пользователем. Proxy продолжает применять rate limits, редактирует креденшелы и отмечает, использовала ли заявка token по умолчанию или override на запрос. Установите `TRYIT_PROXY_CLIENT_ID` с меткой, которую хотите отправлять как `X-TryIt-Client`
(по умолчанию `docs-portal`). Proxy обрезает и валидирует `X-TryIt-Client` от клиента и возвращается к этому default, чтобы staging gateways могли аудитировать provenance без корреляции с метаданными браузера.

## Запуск proxy локально

Установите зависимости при первой настройке портала:

```bash
cd docs/portal
npm install
```

Запустите proxy и укажите ваш Torii instance:

```bash
export TRYIT_PROXY_TARGET="https://torii.devnet.sora.example"
export TRYIT_PROXY_ALLOWED_ORIGINS="http://localhost:3000"
# Optional: preset a bearer token for the Swagger / RapiDoc panels
export TRYIT_PROXY_BEARER="Bearer eyJhbGciOi..."
npm run tryit-proxy
```

Скрипт логирует адрес binding и проксирует запросы с `/proxy/*` на настроенный Torii origin.

Перед binding сокета скрипт проверяет, что
`static/openapi/torii.json` соответствует digest, записанному в
`static/openapi/manifest.json`. Если файлы разошлись, команда завершается ошибкой и предлагает выполнить `npm run sync-openapi -- --latest`. Экспортируйте
`TRYIT_PROXY_ALLOW_STALE_SPEC=1` только для аварийных обходов; proxy запишет предупреждение и продолжит работу, чтобы вы могли восстановиться во время maintenance window.

## Подключение виджетов портала

При build или serve портала разработчика задайте URL, который виджеты должны использовать для proxy:

```bash
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
export TRYIT_PROXY_DEFAULT_BEARER="Bearer eyJhbGciOi..." # Optional
npm run start
```

Следующие компоненты читают значения из `docusaurus.config.js`:

- **Swagger UI** - отображается на `/reference/torii-swagger`; pre-authorises bearer scheme при наличии token, помечает запросы `X-TryIt-Client`, инжектит `X-TryIt-Auth` и переписывает вызовы через proxy, когда `TRYIT_PROXY_PUBLIC_URL` задан.
- **RapiDoc** - отображается на `/reference/torii-rapidoc`; отражает поле token, переиспользует headers Swagger и автоматически направляет через proxy при заданном URL.
- **Try it console** - встроен в страницу обзора API; позволяет отправлять кастомные запросы, просматривать headers и инспектировать тела ответов.

Обе панели показывают **snapshot selector**, который читает
`docs/portal/static/openapi/versions.json`. Заполните индекс командой
`npm run sync-openapi -- --version=<label> --mirror=current --latest`, чтобы reviewers могли переключаться между историческими specs, видеть записанный SHA-256 digest и подтверждать, что snapshot release имеет signed manifest перед использованием интерактивных виджетов.

Смена token в любом виджете влияет только на текущую сессию браузера; proxy никогда не сохраняет и не логирует переданный token.

## Короткоживущие OAuth tokens

Чтобы не распространять долгоживущие Torii tokens среди reviewers, подключите Try it консоль к OAuth серверу. Когда переменные окружения ниже присутствуют, портал отображает device-code widget, выпускает короткоживущие bearer tokens и автоматически подставляет их в форму консоли.

| Variable | Назначение | Default |
| --- | --- | --- |
| `DOCS_OAUTH_DEVICE_CODE_URL` | OAuth Device Authorization endpoint (`/oauth/device/code`) | _empty (disabled)_ |
| `DOCS_OAUTH_TOKEN_URL` | Token endpoint, принимающий `grant_type=urn:ietf:params:oauth:grant-type:device_code` | _empty_ |
| `DOCS_OAUTH_CLIENT_ID` | OAuth client ID, зарегистрированный для docs preview | _empty_ |
| `DOCS_OAUTH_SCOPE` | Scopes, разделенные пробелами, запрашиваемые при входе | `openid profile offline_access` |
| `DOCS_OAUTH_AUDIENCE` | Опциональная API audience для привязки token | _empty_ |
| `DOCS_OAUTH_POLL_INTERVAL_MS` | Минимальный polling interval при ожидании подтверждения (ms) | `5000` (значения < 5000 ms отклоняются) |
| `DOCS_OAUTH_DEVICE_CODE_TTL_SECONDS` | Фоллбек TTL device-code (seconds) | `600` (должно быть между 300 s и 900 s) |
| `DOCS_OAUTH_TOKEN_TTL_SECONDS` | Фоллбек TTL access-token (seconds) | `900` (должно быть между 300 s и 900 s) |
| `DOCS_OAUTH_ALLOW_INSECURE` | Установите `1` для локальных preview, которые намеренно обходят OAuth | _unset_ |

Example configuration:

```bash
export DOCS_OAUTH_DEVICE_CODE_URL="https://auth.dev.sora.example/oauth/device/code"
export DOCS_OAUTH_TOKEN_URL="https://auth.dev.sora.example/oauth/token"
export DOCS_OAUTH_CLIENT_ID="docs-preview"
export DOCS_OAUTH_SCOPE="torii openid offline_access"
# Optional audience and polling tweaks
export DOCS_OAUTH_AUDIENCE="https://torii.devnet.sora.example"
export DOCS_OAUTH_POLL_INTERVAL_MS="6000"
```

Когда вы запускаете `npm run start` или `npm run build`, портал встраивает эти значения в `docusaurus.config.js`. Во время локального preview карточка Try it показывает кнопку "Sign in with device code". Пользователи вводят показанный код на вашей OAuth verification странице; после успешного device flow виджет:

- вставляет выданный bearer token в поле консоли Try it,
- добавляет headers `X-TryIt-Client` и `X-TryIt-Auth`,
- отображает оставшееся время жизни,
- автоматически очищает token при истечении срока.

Ручной ввод Bearer остается доступным - уберите OAuth переменные, если хотите заставить reviewers вставлять временный token вручную, или экспортируйте `DOCS_OAUTH_ALLOW_INSECURE=1` для изолированных локальных preview, где анонимный доступ приемлем. Builds без настроенного OAuth теперь быстро падают, чтобы удовлетворить gate DOCS-1b.

Note: Перед публикацией портала за пределами лаборатории проверьте [Security hardening & pen-test checklist](./security-hardening.md); в нем описаны threat model, профиль CSP/Trusted Types и pen-test шаги, которые теперь gate DOCS-1b.

## Примеры Norito-RPC

Norito-RPC запросы используют тот же proxy и OAuth plumbing, что и JSON routes; они просто ставят `Content-Type: application/x-norito` и отправляют заранее закодированный Norito payload, описанный в спецификации NRPC (`docs/source/torii/nrpc_spec.md`). Репозиторий содержит канонические payloads под `fixtures/norito_rpc/`, чтобы авторы портала, SDK owners и reviewers могли воспроизвести точные bytes, используемые CI.

### Отправка Norito payload из Try It console

1. Выберите fixture, например `fixtures/norito_rpc/transfer_asset.norito`. Эти файлы являются сырыми Norito envelopes; **не** кодируйте их в base64.
2. В Swagger или RapiDoc найдите NRPC endpoint (например, `POST /v1/pipeline/submit`) и переключите селектор **Content-Type** на `application/x-norito`.
3. Переключите редактор тела запроса в **binary** (режим "File" в Swagger или "Binary/File" в RapiDoc) и загрузите `.norito` файл. Виджет отправит bytes через proxy без изменений.
4. Отправьте запрос. Если Torii возвращает `X-Iroha-Error-Code: schema_mismatch`, убедитесь, что вызываете endpoint, принимающий бинарные payloads, и подтвердите, что schema hash в `fixtures/norito_rpc/schema_hashes.json` совпадает с build Torii.

Консоль хранит последний файл в памяти, чтобы можно было повторно отправлять тот же payload с разными authorization tokens или Torii hosts. Добавление `scripts/run_norito_rpc_fixtures.sh --note "<ticket>"` в workflow создает evidence bundle, упомянутый в плане внедрения NRPC-4 (log + JSON summary), который хорошо сочетается со скриншотом ответа Try It во время reviews.

### CLI пример (curl)

Те же fixtures можно проигрывать вне портала через `curl`, что полезно для проверки proxy или отладки ответов gateway:

```bash
TORII="https://torii.devnet.sora.example"
TOKEN="Bearer $(cat ~/.config/torii/devnet.token)"
curl   -H "Content-Type: application/x-norito"   -H "Authorization: ${TOKEN}"   --data-binary @fixtures/norito_rpc/transfer_asset.norito   "${TORII}/v1/pipeline/submit"
```

Замените fixture на любой entry из `transaction_fixtures.manifest.json` или закодируйте свой payload командой `cargo xtask norito-rpc-fixtures`. Когда Torii работает в canary режиме, можно направить `curl` на try-it proxy (`https://docs.sora.example/proxy/v1/pipeline/submit`), чтобы проверить ту же инфраструктуру, что и виджеты портала.

## Observability и operations

Каждый запрос логируется один раз с method, path, origin, upstream status и источником аутентификации (`override`, `default` или `client`). Tokens никогда не сохраняются - bearer headers и значения `X-TryIt-Auth` редактируются перед логированием, поэтому можно пересылать stdout в центральный collector без риска утечки секретов.

### Health probes и alerting

Запускайте встроенный probe при деплойментах или по расписанию:

```bash
# Ensure the proxy responds to /healthz and forwards a sample request.
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" TRYIT_PROXY_SAMPLE_PATH="/v1/status" npm run probe:tryit-proxy
```

Knobs окружения:

- `TRYIT_PROXY_SAMPLE_PATH` - опциональный Torii маршрут (без `/proxy`) для проверки.
- `TRYIT_PROXY_SAMPLE_METHOD` - по умолчанию `GET`; задайте `POST` для write маршрутов.
- `TRYIT_PROXY_PROBE_TOKEN` - добавляет временный bearer token к sample вызову.
- `TRYIT_PROXY_PROBE_TIMEOUT_MS` - переопределяет timeout по умолчанию 5 s.
- `TRYIT_PROXY_PROBE_METRICS_FILE` - опциональный Prometheus textfile для `probe_success`/`probe_duration_seconds`.
- `TRYIT_PROXY_PROBE_LABELS` - пары `key=value`, добавляемые к метрикам (по умолчанию `job=tryit-proxy` и `instance=<proxy URL>`).
- `TRYIT_PROXY_PROBE_METRICS_URL` - опциональный URL metrics endpoint (например `http://localhost:9798/metrics`), который должен отвечать успешно при включенном `TRYIT_PROXY_METRICS_LISTEN`.

Передавайте результаты в textfile collector, направив probe на writable path (например, `/var/lib/node_exporter/textfile_collector/tryit.prom`) и добавив кастомные labels:

```bash
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" TRYIT_PROXY_PROBE_METRICS_FILE="/var/lib/node_exporter/textfile_collector/tryit.prom" TRYIT_PROXY_PROBE_LABELS="job=tryit-proxy,cluster=prod" npm run probe:tryit-proxy
```

Скрипт атомарно переписывает файл метрик, чтобы collector всегда читал полный payload.

Если `TRYIT_PROXY_METRICS_LISTEN` настроен, задайте
`TRYIT_PROXY_PROBE_METRICS_URL` на metrics endpoint, чтобы probe быстро падал при пропаже поверхности scrape (например, неверный ingress или отсутствующие firewall rules). Типичная production настройка:
`TRYIT_PROXY_PROBE_METRICS_URL="http://127.0.0.1:9798/metrics"`.

Для легкого alerting подключите probe к вашему monitoring stack. Пример Prometheus, который пейджит после двух последовательных падений:

```yaml
groups:
  - name: tryit-proxy
    rules:
      - alert: TryItProxyUnhealthy
        expr: probe_success{job="tryit-proxy"} == 0
        for: 2m
        labels:
          severity: page
        annotations:
          summary: Try It proxy is failing health checks
          description: |
            The try-it proxy at {{ $labels.instance }} is not responding to probe requests.
```

### Metrics endpoint и dashboards

Установите `TRYIT_PROXY_METRICS_LISTEN=127.0.0.1:9798` (или любой host/port) перед запуском proxy, чтобы открыть Prometheus-formatted metrics endpoint. Путь по умолчанию `/metrics`, но его можно переопределить через `TRYIT_PROXY_METRICS_PATH=/custom`. Каждый scrape возвращает счетчики по методам, rate-limit rejects, upstream errors/timeouts, proxy outcomes и summary latency:

```bash
export TRYIT_PROXY_METRICS_LISTEN="127.0.0.1:9798"
npm run tryit-proxy &
curl http://127.0.0.1:9798/metrics | head -n 5
# HELP tryit_proxy_requests_total Requests handled by method
tryit_proxy_requests_total{method="GET"} 12
tryit_proxy_rate_limited_total 1
```

Направьте Prometheus/OTLP collectors на metrics endpoint и переиспользуйте панели из `dashboards/grafana/docs_portal.json`, чтобы SRE могли наблюдать tail latency и всплески отказов без парсинга логов. Proxy автоматически публикует `tryit_proxy_start_timestamp_ms`, помогая операторам обнаруживать рестарты.

### Автоматизация rollback

Используйте management helper для обновления или восстановления целевого URL Torii. Скрипт сохраняет предыдущую конфигурацию в `.env.tryit-proxy.bak`, так что rollback - это одна команда.

```bash
# Update TRYIT_PROXY_TARGET and back up the previous config.
npm run manage:tryit-proxy -- update --target https://torii.devnet.sora.example

# Roll back to the previously backed-up target.
npm run manage:tryit-proxy -- rollback
```

Переопределите путь env файла через `--env` или `TRYIT_PROXY_ENV`, если ваше развертывание хранит конфигурацию в другом месте.
