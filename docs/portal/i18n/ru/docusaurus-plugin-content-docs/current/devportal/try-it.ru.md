---
lang: ru
direction: ltr
source: docs/portal/docs/devportal/try-it.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

#Песочница Попробуй

Портал разработчика включает опциональную консоль «Try it», чтобы вы могли использовать конечные точки Torii, не выходя из документации. Консоль проксирует запросы через встроенный прокси-сервер, чтобы браузеры обходили ограничения CORS, одновременно соблюдая ограничения скорости и аутентификацию.

## Предварительные условия

- Node.js 18.18 или новее (соответствует требованиям сборки портала)
- Сетевой доступ к промежуточному окружению Torii
- токен на предъявителя, который может указывать нужные маршруты Torii

Вся настройка прокси-сервера выполняется через переменные окружения. В таблице ниже перечислены основные ручки:

| Переменная | Назначение | По умолчанию |
| --- | --- | --- |
| `TRYIT_PROXY_TARGET` | Базовый URL Torii, куда прокси пересылает запрос | **Обязательно** |
| `TRYIT_PROXY_LISTEN` | Адрес прослушивания для локальной разработки (формат `host:port` или `[ipv6]:port`) | `127.0.0.1:8787` |
| `TRYIT_PROXY_ALLOWED_ORIGINS` | Список origin-ов, которым разрешено обращаться к прокси (через запятую) | `http://localhost:3000` |
| `TRYIT_PROXY_CLIENT_ID` | Идентификатор, дополненный в `X-TryIt-Client` для каждого восходящего запроса | `docs-portal` |
| `TRYIT_PROXY_BEARER` | Токен на предъявителя по умолчанию, пересылаемый в Torii | _пустой_ |
| `TRYIT_PROXY_ALLOW_CLIENT_AUTH` | Разрешить пользователям наш собственный токен через `X-TryIt-Auth` | `0` |
| `TRYIT_PROXY_MAX_BODY` | Максимальный размер запроса тела (байты) | `1048576` |
| `TRYIT_PROXY_TIMEOUT_MS` | Тайм-аут восходящего потока в миллисекундах | `10000` |
| `TRYIT_PROXY_RATE_LIMIT` | Запросы в окне ограничения скорости на IP клиента | `60` |
| `TRYIT_PROXY_RATE_WINDOW_MS` | Скользящее окно, ограничение скорости (мс) | `60000` |
| `TRYIT_PROXY_METRICS_LISTEN` | Адрес прослушивания для метрики Prometheus (`host:port` или `[ipv6]:port`) | _пусто (отключено)_ |
| `TRYIT_PROXY_METRICS_PATH` | HTTP-путь для метрики конечной точки | `/metrics` |

Прокси также предоставляет `GET /healthz`, возвращает структурированные ошибки JSON и редактирует токены-носители в логах.

Включите `TRYIT_PROXY_ALLOW_CLIENT_AUTH=1`, если прокси доступна пользователям документации, на панели Swagger и RapiDoc можно пересылать токены на предъявителя, зарегистрированные пользователем. Прокси-сервер продолжает применять ограничения скорости, редактирует учетные данные и отмечает, использовал токен заявки по умолчанию или переопределял запрос. Установите `TRYIT_PROXY_CLIENT_ID` с меткой, которую хотите отправить как `X-TryIt-Client`.
(по умолчанию `docs-portal`). Прокси обрезает и валидирует `X-TryIt-Client` от клиента и возвращается к этому значению по умолчанию, чтобы промежуточные шлюзы могли проверять происхождение без сопоставления с метаданными браузером.

## Запуск прокси локально

Укажите в зависимости от первой настройки портала:

```bash
cd docs/portal
npm install
```

Запустите прокси и укажите ваш экземпляр Torii:

```bash
export TRYIT_PROXY_TARGET="https://torii.devnet.sora.example"
export TRYIT_PROXY_ALLOWED_ORIGINS="http://localhost:3000"
# Optional: preset a bearer token for the Swagger / RapiDoc panels
export TRYIT_PROXY_BEARER="Bearer eyJhbGciOi..."
npm run tryit-proxy
```

Скрипт регистрирует привязку адреса и проксирует запросы с `/proxy/*` по настроенному источнику Torii.Перед связыванием сокета скрипта, что
`static/openapi/torii.json` соответствует дайджесту, приведенному в
`static/openapi/manifest.json`. Если файлы разошлись, команда завершает ошибку и предлагает выполнить баланс `npm run sync-openapi -- --latest`. Экспортируйте
`TRYIT_PROXY_ALLOW_STALE_SPEC=1` только для аварийных обходов; Прокси запишет предупреждение и продолжит работу, чтобы вы могли восстановиться во время окна обслуживания.

## Подключение виджетов портала

При создании или обслуживании портала разработчик задайте URL-адрес, который виджеты следует использовать для прокси:

```bash
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
export TRYIT_PROXY_DEFAULT_BEARER="Bearer eyJhbGciOi..." # Optional
npm run start
```

Следующие компоненты, читающие значения `docusaurus.config.js`:

- **Swagger UI** - отображается на `/reference/torii-swagger`; предварительно авторизует схему носителя при наличии токена, помечает запрос `X-TryIt-Client`, инжектит `X-TryIt-Auth` и переписывает вызовы через прокси, когда задан `TRYIT_PROXY_PUBLIC_URL`.
- **RapiDoc** - отображается на `/reference/torii-rapidoc`; Отражает токен поля, переиспользует заголовки Swagger и автоматически направляет через прокси-сервер по заданному URL-адресу.
- **Попробуйте консоль** — встроен в API обзора страницы; Позволяет отправлять кастомные запросы, просматривать заголовки и проверять ответы ответов.

Обе панели отображают **селектор снимков**, который читает
`docs/portal/static/openapi/versions.json`. Заполните индекс один
`npm run sync-openapi -- --version=<label> --mirror=current --latest`, чтобы рецензенты могли переключаться между историческими спецификациями, видеть приведенный дайджест SHA-256 и подтверждать, что выпуск моментальных снимков имеет подписанный манифест перед использованием интерактивных виджетов.

Смена токена в любом виджете влияет только на текущую сессию браузера; прокси никогда не сохраняет и не регистрирует переданный токен.

## Короткоживущие токены OAuth

Чтобы не распространять долгоживущие токены Torii среди рецензентов, подключите попробуйте консоль к серверу OAuth. Когда переменные окружения ниже представлены, портал отображает виджет кода устройства, выпускает короткоживущие токены на предъявителя и автоматически подставляет их в форму консоли.

| Переменная | Назначение | По умолчанию |
| --- | --- | --- |
| `DOCS_OAUTH_DEVICE_CODE_URL` | Конечная точка авторизации устройства OAuth (`/oauth/device/code`) | _пусто (отключено)_ |
| `DOCS_OAUTH_TOKEN_URL` | Конечная точка токена, принимающий `grant_type=urn:ietf:params:oauth:grant-type:device_code` | _пустой_ |
| `DOCS_OAUTH_CLIENT_ID` | Идентификатор клиента OAuth, зарегистрированный для предварительного просмотра документов | _пустой_ |
| `DOCS_OAUTH_SCOPE` | Области, разделенные пробелами, запрашиваемые при входе | `openid profile offline_access` |
| `DOCS_OAUTH_AUDIENCE` | Опциональная аудитория API для привязки токена | _пустой_ |
| `DOCS_OAUTH_POLL_INTERVAL_MS` | Минимальный интервал опроса при ожидании подтверждения (мс) | `5000` (значения < 5000 мс отклоняются) |
| `DOCS_OAUTH_DEVICE_CODE_TTL_SECONDS` | Фоллбек TTL-код устройства (секунды) | `600` (должно быть между 300 с и 900 с) |
| `DOCS_OAUTH_TOKEN_TTL_SECONDS` | Фоллбек TTL-токен доступа (секунды) | `900` (должно быть между 300 с и 900 с) |
| `DOCS_OAUTH_ALLOW_INSECURE` | Установите `1` для локального предварительного просмотра, который намеренно обходит OAuth | _закат_ |

Пример конфигурации:

```bash
export DOCS_OAUTH_DEVICE_CODE_URL="https://auth.dev.sora.example/oauth/device/code"
export DOCS_OAUTH_TOKEN_URL="https://auth.dev.sora.example/oauth/token"
export DOCS_OAUTH_CLIENT_ID="docs-preview"
export DOCS_OAUTH_SCOPE="torii openid offline_access"
# Optional audience and polling tweaks
export DOCS_OAUTH_AUDIENCE="https://torii.devnet.sora.example"
export DOCS_OAUTH_POLL_INTERVAL_MS="6000"
```Когда вы запускаете `npm run start` или `npm run build`, портал устанавливает эти значения в `docusaurus.config.js`. Во время локального предварительного просмотра карточка Попробуйте показать кнопку «Войти с помощью кода устройства». Пользователи вводят показанный код на вашей странице проверки OAuth; после получения виджет потока устройства:

- вставляет выданный жетон на предъявителя в поле консоли. Попробуйте,
- добавить заголовки `X-TryIt-Client` и `X-TryIt-Auth`,
- отображает оставшееся время жизни,
- автоматически очищает токен в течение срока.

Ручной ввод Носитель остается доступным — выберите переменные OAuth, если хотите, чтобы рецензенты создавали временный токен вручную, или экспортируйте `DOCS_OAUTH_ALLOW_INSECURE=1` для изолированных локальных предварительных просмотров, где приемлем анонимный доступ. Сборки без настроенного OAuth теперь быстро падают, чтобы поддержать шлюз DOCS-1b.

Примечание. Перед публикацией портала за хозяйственную лабораторию проверьте [Контрольный список повышения безопасности и проверки на проникновение] (./security-hardening.md); в нем описаны модель угроз, профиль CSP/доверенные типы и этапы проверки на проникновение, которые теперь являются шлюзом DOCS-1b.

## Примеры Norito-RPC

Norito-RPC использует тот же прокси и каналы OAuth, что и маршруты JSON; они просто устанавливают `Content-Type: application/x-norito` и отправляют заранее закодированную полезную нагрузку Norito, описанную в характеристиках NRPC (`docs/source/torii/nrpc_spec.md`). Репозиторий содержит стандартные полезные нагрузки под `fixtures/norito_rpc/`, которые авторы портала, владельцы SDK и рецензенты могут воспроизводить точные байты, используя CI.

### Отправка Norito payload из консоли Try It

1. Выберите приспособление, например `fixtures/norito_rpc/transfer_asset.norito`. Эти файлы представляют собой сырые конверты Norito; **не** кодируйте их в base64.
2. В Swagger или RapiDoc перейдите к конечной точке NRPC (например, `POST /v2/pipeline/submit`) и переключите селектор **Content-Type** на `application/x-norito`.
3. Переключите редактор запроса тела в **binary** (режим «Файл» в Swagger или «Binary/File» в RapiDoc) и загрузите файл `.norito`. Виджет отправляет байты через прокси без изменений.
4. Отправьте запрос. Если Torii возвращает `X-Iroha-Error-Code: schema_mismatch`, убедитесь, что возникает конечная точка, принимающая двоичные полезные данные, и подтвердите, что хеш схемы в `fixtures/norito_rpc/schema_hashes.json` соответствует сборке Torii.

Консоль сохраняет последний файл в памяти, чтобы можно было повторно отправить тот же самый полезный груз с других токенов авторизации или хостов Torii. Добавление `scripts/run_norito_rpc_fixtures.sh --note "<ticket>"` в рабочий процесс представляет пакет доказательств, упомянутый в системе здравоохранения NRPC-4 (журнал + сводка JSON), который хорошо сочетается со скриншотом ответа. Попробуйте во время обзоров.

### пример CLI (curl)

Те же самые устройства можно проигрывать вне портала через `curl`, что полезно для проверки прокси или отладки ответов шлюза:

```bash
TORII="https://torii.devnet.sora.example"
TOKEN="Bearer $(cat ~/.config/torii/devnet.token)"
curl   -H "Content-Type: application/x-norito"   -H "Authorization: ${TOKEN}"   --data-binary @fixtures/norito_rpc/transfer_asset.norito   "${TORII}/v2/pipeline/submit"
```

Замените приспособление на любую запись из `transaction_fixtures.manifest.json` или закодируйте свою полезную нагрузку командой `cargo xtask norito-rpc-fixtures`. Когда Torii работает в канареечном режиме, можно направить `curl` на пробный прокси (`https://docs.sora.example/proxy/v2/pipeline/submit`), чтобы проверить ту же инфраструктуру, что и виджеты портала.## Наблюдаемость и операции

Каждый запрос регистрируется один раз с методом, путем, источником, статусом восходящего потока и доказательством аутентификации (`override`, `default` или `client`). Токены не определяют — заголовки носителей всегда и значения `X-TryIt-Auth` редактируются перед регистрацией, поэтому можно пересылать стандартный вывод в центральный сборщик без риска утечки секретов.

### Проверка работоспособности и оповещение

Запускайте встроенный зонд при развертывании или по расписанию:

```bash
# Ensure the proxy responds to /healthz and forwards a sample request.
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" TRYIT_PROXY_SAMPLE_PATH="/v2/status" npm run probe:tryit-proxy
```

Ручки окружения:

- `TRYIT_PROXY_SAMPLE_PATH` - дополнительный маршрут Torii (без `/proxy`) для проверки.
- `TRYIT_PROXY_SAMPLE_METHOD` - по умолчанию `GET`; задайте `POST` для написания маршрутов.
- `TRYIT_PROXY_PROBE_TOKEN` - добавлен временный токен на предъявителя к образцу вызова.
- `TRYIT_PROXY_PROBE_TIMEOUT_MS` - переопределяет таймаут по умолчанию 5 с.
- `TRYIT_PROXY_PROBE_METRICS_FILE` - опциональный текстовый файл Prometheus для `probe_success`/`probe_duration_seconds`.
- `TRYIT_PROXY_PROBE_LABELS` - пары `key=value`, дополненные к метрикам (по умолчанию `job=tryit-proxy` и `instance=<proxy URL>`).
- `TRYIT_PROXY_PROBE_METRICS_URL` - опциональная конечная точка URL-метрики (например, `http://localhost:9798/metrics`), которая должна появиться успешно при включенном `TRYIT_PROXY_METRICS_LISTEN`.

Передайте результаты в сборщик текстовых файлов, направив пробник на записываемый путь (например, `/var/lib/node_exporter/textfile_collector/tryit.prom`) и добавив кастомные метки:

```bash
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" TRYIT_PROXY_PROBE_METRICS_FILE="/var/lib/node_exporter/textfile_collector/tryit.prom" TRYIT_PROXY_PROBE_LABELS="job=tryit-proxy,cluster=prod" npm run probe:tryit-proxy
```

Скрипт атомарно переписывает метрику файла, чтобы сборщик всегда читал полную полезную нагрузку.

Если `TRYIT_PROXY_METRICS_LISTEN` настроение, задайте
`TRYIT_PROXY_PROBE_METRICS_URL` на конечной точке метрики, чтобы проверять быстро падал при пропаже очистки поверхности (например, неверный вход или отсутствующие правила брандмауэра). Типичная производственная настройка:
`TRYIT_PROXY_PROBE_METRICS_URL="http://127.0.0.1:9798/metrics"`.

Для простого оповещения подключите датчик к стеку мониторинга. Пример Prometheus, который пейджит после двух последовательных падений:

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

### Конечная точка метрик и информационные панели

Установите `TRYIT_PROXY_METRICS_LISTEN=127.0.0.1:9798` (или любой хост/порт) перед запуском прокси-сервера, чтобы открыть конечную точку метрик в формате Prometheus. Путь по умолчанию `/metrics`, но его можно переопределить через `TRYIT_PROXY_METRICS_PATH=/custom`. Каждая очистка возвращает счетчики по методам, отклонения от ограничения скорости, ошибки/таймауты восходящего потока, результаты прокси и суммарную задержку:

```bash
export TRYIT_PROXY_METRICS_LISTEN="127.0.0.1:9798"
npm run tryit-proxy &
curl http://127.0.0.1:9798/metrics | head -n 5
# HELP tryit_proxy_requests_total Requests handled by method
tryit_proxy_requests_total{method="GET"} 12
tryit_proxy_rate_limited_total 1
```

Отправьте сборщики Prometheus/OTLP на конечную точку метрик и переиспользуйте панель из `dashboards/grafana/docs_portal.json`, чтобы SRE могла наблюдать задержку хвоста и всплески отказов без анализа журналов. Прокси-сервер автоматически публикует `tryit_proxy_start_timestamp_ms`, помогая операторам найти перезапуск.

### Автоматизация отката

Используйте помощник управления для обновления или восстановления целевого URL Torii. Скрипт сохраняет предыдущую конфигурацию в `.env.tryit-proxy.bak`, так что откат - это одна команда.

```bash
# Update TRYIT_PROXY_TARGET and back up the previous config.
npm run manage:tryit-proxy -- update --target https://torii.devnet.sora.example

# Roll back to the previously backed-up target.
npm run manage:tryit-proxy -- rollback
```

Переопределите путь файла env через `--env` или `TRYIT_PROXY_ENV`, если ваше развертывание хранит конфигурацию в другом месте.