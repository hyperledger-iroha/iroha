---
lang: ru
direction: ltr
source: docs/portal/docs/devportal/try-it.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Try It سینڈ باکس

Если вам нужна помощь с надписью «Попробуй», вы можете выбрать вариант «Попробуй». Проверьте конечные точки Torii. Как получить доступ к CORS حدود بائی پاس کر سکیں جبکہ ریٹ لمٹس اور آتھنٹیکیشن نافذ رہیں۔

## شرائط

— Версия Node.js 18.18 (доступна только при сборке)
- Torii staging ماحول تک نیٹ ورک رسائی
- Маркер на предъявителя может быть связан с Torii маршрутами, которые могут быть использованы для маршрутизации.

Как изменить переменные среды? Что касается ручек, например:

| Переменная | مقصد | По умолчанию |
| --- | --- | --- |
| `TRYIT_PROXY_TARGET` | Torii Базовый URL-адрес URL-адреса базы данных | **Обязательно** |
| `TRYIT_PROXY_LISTEN` | لوکل ڈیولپمنٹ کے لئے прослушать адрес (`host:port` یا `[ipv6]:port`) | `127.0.0.1:8787` |
| `TRYIT_PROXY_ALLOWED_ORIGINS` | происхождение کی через запятую فہرست جو پروکسی کو کال کر سکتے ہیں | `http://localhost:3000` |
| `TRYIT_PROXY_CLIENT_ID` | Восходящий запрос `X-TryIt-Client` позволяет получить идентификатор | `docs-portal` |
| `TRYIT_PROXY_BEARER` | Токен на предъявителя: Torii. _пустой_ |
| `TRYIT_PROXY_ALLOW_CLIENT_AUTH` | صارفین کو `X-TryIt-Auth` کے ذریعے اپنا token دینے کی ازت | `0` |
| `TRYIT_PROXY_MAX_BODY` | тело запроса Размер файла (байты) | `1048576` |
| `TRYIT_PROXY_TIMEOUT_MS` | тайм-аут восходящего потока (миллисекунды) | `10000` |
| `TRYIT_PROXY_RATE_LIMIT` | IP-адрес клиента в оконных запросах | `60` |
| `TRYIT_PROXY_RATE_WINDOW_MS` | ограничение скорости کے لئے скользящее окно (мс) | `60000` |
| `TRYIT_PROXY_METRICS_LISTEN` | Prometheus Зарегистрируйте конечную точку метрик для получения адреса прослушивания (`host:port` или `[ipv6]:port`) | _пусто (отключено)_ |
| `TRYIT_PROXY_METRICS_PATH` | конечная точка метрик и HTTP-путь | `/metrics` |

Воспользуйтесь `GET /healthz`, чтобы просмотреть структурированные ошибки JSON и получить токены на предъявителя, а также вывод журнала и редактирование. کرتا ہے۔

`TRYIT_PROXY_ALLOW_CLIENT_AUTH=1` позволяет получить доступ к документам, которые пользователи могут предоставить пользователям Swagger и панелям RapiDoc, предоставленным пользователем маркерам носителя. فارورڈ کر سکیں۔ Ограничения скорости и учетные данные, редактирование и редактирование, а также возможность запроса и токена по умолчанию. Переопределение для каждого запроса. `TRYIT_PROXY_CLIENT_ID` Отметьте ярлык, который можно использовать для `X-TryIt-Client`.
(ڈیفالٹ `docs-portal`)۔ Значения `X-TryIt-Client`, предоставленные вызывающим абонентом, и обрезка, и проверка достоверности, или значения по умолчанию, или промежуточные шлюзы, метаданные браузера коррелировать کئے بغیر аудит происхождения کر سکیں۔

## لوکل طور پر پروکسی چلائیں

Ниже приведены примеры зависимостей, которые можно использовать:

```bash
cd docs/portal
npm install
```

Для примера можно использовать экземпляр Torii, который может быть использован:

```bash
export TRYIT_PROXY_TARGET="https://torii.devnet.sora.example"
export TRYIT_PROXY_ALLOWED_ORIGINS="http://localhost:3000"
# Optional: preset a bearer token for the Swagger / RapiDoc panels
export TRYIT_PROXY_BEARER="Bearer eyJhbGciOi..."
npm run tryit-proxy
```

Связанный адрес может быть связан с `/proxy/*`, запросы настроены, Torii origin, перенаправление,ساکٹ привязать کرنے سے پہلے اسکرپٹ проверить کرتا ہے کہ
`static/openapi/torii.json` کا дайджест `static/openapi/manifest.json` میں ریکارڈ شدہ سے میچ کرے۔ Если вы дрифтуете, вы можете столкнуться с ошибкой или выйти из системы, когда `npm run sync-openapi -- --latest` будет отключен. ہدایت دیتا ہے۔ `TRYIT_PROXY_ALLOW_STALE_SPEC=1` صرف ہنگامی override کے لئے استعمال کریں؛ Предупреждение о том, что вам нужно установить окна обслуживания.

## Дополнительные виджеты

Портал разработчиков позволяет создавать и обслуживать веб-сайты с URL-адресами и виджетами. Варианты:

```bash
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
export TRYIT_PROXY_DEFAULT_BEARER="Bearer eyJhbGciOi..." # Optional
npm run start
```

Компоненты `docusaurus.config.js` и значения, необходимые для:

- **Swagger UI** - `/reference/torii-swagger` для рендеринга ہوتا ہے؛ Токен используется для схемы носителя и предварительной авторизации запросов для `X-TryIt-Client` для тега `X-TryIt-Auth` для инъекции ہے، اور `TRYIT_PROXY_PUBLIC_URL` سیٹ ہونے پر вызывает کو پروکسی کے ذریعے переписать کرتا ہے۔
- **RapiDoc** - `/reference/torii-rapidoc` для рендеринга ہوتا ہے؛ токен и зеркало Панель Swagger и заголовки повторное использование URL-адрес и настройка прокси-сервера и цель کرتا ہے۔
- **Попробуйте консоль** - Страница обзора API встроена ہے؛ пользовательские запросы بھیجنے، заголовки دیکھنے، اور органы реагирования проверяют کرنے دیتا ہے۔

Панели выбора **селектор снимков** دکھاتے ہیں جو
`docs/portal/static/openapi/versions.json` پڑھتا ہے۔ Индекс اس کو
`npm run sync-openapi -- --version=<label> --mirror=current --latest` سے بھریں تاکہ рецензенты исторические характеристики میں جا سکیں، ریکارڈ شدہ SHA-256 дайджест دیکھ سکیں، اور интерактивный Виджеты استعمال کرنے سے پہلے تصدیق کر سکیں کہ выпуск снапшота подписанный манифест لے کر آیا ہے۔

Доступ к виджету и токену Создание сеанса браузера и доступ к нему Использование прокси-сервера для токена и сохранения журнала регистрации

## Получение токенов OAuth

Используйте Torii для рецензентов токенов или проверьте, работает ли консоль. Попробуйте использовать консоль или сервер OAuth. Доступны переменные среды и виджет входа в код устройства, а также возможность использования токенов-носителей. Создайте консольную форму и введите команду inject.

| Переменная | مقصد | По умолчанию |
| --- | --- | --- |
| `DOCS_OAUTH_DEVICE_CODE_URL` | Конечная точка авторизации устройства OAuth (`/oauth/device/code`) | _пусто (отключено)_ |
| `DOCS_OAUTH_TOKEN_URL` | Конечная точка токена جو `grant_type=urn:ietf:params:oauth:grant-type:device_code` قبول کرتا ہے | _пустой_ |
| `DOCS_OAUTH_CLIENT_ID` | Идентификатор клиента OAuth Предварительный просмотр документации کے لئے رجسٹر ہے | _пустой_ |
| `DOCS_OAUTH_SCOPE` | вход в систему Доступ к областям, разделенным пробелами | `openid profile offline_access` |
| `DOCS_OAUTH_AUDIENCE` | токен کو باندھنے کے لئے اختیاری API аудитории | _пустой_ |
| `DOCS_OAUTH_POLL_INTERVAL_MS` | Введите интервал опроса (мс) | `5000` (время < 5000 мс в режиме ожидания) |
| `DOCS_OAUTH_DEVICE_CODE_TTL_SECONDS` | окно срока действия резервного кода устройства (секунды) | `600` (от 300 до 900 с в зависимости от времени) |
| `DOCS_OAUTH_TOKEN_TTL_SECONDS` | время жизни токена доступа (в секундах) | `900` (300 с или 900 с в зависимости от времени) |
| `DOCS_OAUTH_ALLOW_INSECURE` | Предварительный просмотр کے لئے `1` и принудительное применение OAuth для проверки безопасности | _закат_ |

Пример конфигурации:

```bash
export DOCS_OAUTH_DEVICE_CODE_URL="https://auth.dev.sora.example/oauth/device/code"
export DOCS_OAUTH_TOKEN_URL="https://auth.dev.sora.example/oauth/token"
export DOCS_OAUTH_CLIENT_ID="docs-preview"
export DOCS_OAUTH_SCOPE="torii openid offline_access"
# Optional audience and polling tweaks
export DOCS_OAUTH_AUDIENCE="https://torii.devnet.sora.example"
export DOCS_OAUTH_POLL_INTERVAL_MS="6000"
```جب آپ `npm run start` یا `npm run build` چلاتے ہیں تو پورٹل یہvalues ​​`docusaurus.config.js` میں embed کرتا ہے۔ Предварительный просмотр کے دوران Попробуйте карточка «Войти с помощью кода устройства» بٹن دکھاتا ہے۔ Введите код и страницу проверки OAuth. Виджет потока устройства:

- Используйте токен на предъявителя. Попробуйте консоль. Используйте инъекцию.
- Заголовки `X-TryIt-Client` и `X-TryIt-Auth` могут содержать запросы на тег کرتا ہے،
- باقی ماندہ مدت دکھاتا ہے، اور
- токен ختم ہونے پر خودکار طور پر صاف کر دیتا ہے۔

Руководство по вводу носителя для ввода данных - Переменные OAuth могут быть полезны для рецензентов, а также для вставки токена и вставки. Доступ к `DOCS_OAUTH_ALLOW_INSECURE=1` экспорт изолированные предварительные просмотры и анонимный доступ OAuth создает шлюз дорожной карты DOCS-1b, который может привести к сбою или сбою в работе OAuth.

Примечание. Чтобы открыть доступ к [Контрольный список повышения безопасности и проверки на проникновение] (./security-hardening.md) دیکھیں؛ Модель угроз, CSP/Trusted Types, этапы пен-теста, а также шлюзы DOCS-1b и шлюзы.

## Norito-RPC نمونے

Norito-RPC запрашивает прокси-сервер и канал OAuth, а также маршруты JSON; 2. `Content-Type: application/x-norito`. Информация о спецификации NRPC. Информация о предварительно закодированной полезной нагрузке Norito. ہیں
(`docs/source/torii/nrpc_spec.md`)۔ Используйте `fixtures/norito_rpc/` для канонических полезных нагрузок и для авторов портала, владельцев SDK и рецензентов, а также для воспроизведения байтов. جو CI استعمال کرتا ہے۔

### Try It console سے Norito payload بھیجیں

1. `fixtures/norito_rpc/transfer_asset.norito` Крепежное приспособление для установки. یہ فائلیں raw Norito конверты ہیں؛ Кодирование **base64 или кодирование**۔
2. Swagger в RapiDoc и в конечной точке NRPC (например, `POST /v1/pipeline/submit`) или селектор **Content-Type** в `application/x-norito`. کریں۔
3. Редактор тела запроса может быть **двоичным** или селектором (Swagger — «Файл» или RapiDoc — селектор «Двоичный/Файл») — `.norito`, например, `.norito`. کریں۔ Виджет байтов и прокси-сервера для потоковой передачи
4. запросить بھیجیں۔ Если Torii `X-Iroha-Error-Code: schema_mismatch` не используется, вы можете выбрать конечную точку, которая будет использоваться. Для двоичных полезных данных используется `fixtures/norito_rpc/schema_hashes.json`, используется хеш-схема, встроенная сборка Torii. کرتا ہے۔

Чтобы получить доступ к токенам авторизации, Torii хосты Информация о полезной нагрузке `scripts/run_norito_rpc_fixtures.sh --note "<ticket>"` Рабочий процесс для создания плана внедрения NRPC-4 и создания пакета доказательств (журнал + сводка в формате JSON). جو обзоры کے دوران Try It ответ کے скриншот کے ساتھ اچھی طرح جاتا ہے۔

### Кнопка CLI (завиток)

Встроенные светильники `curl` могут быть использованы для повторного воспроизведения или проверки прокси-сервера. Ответы шлюза на отладку или отладку:

```bash
TORII="https://torii.devnet.sora.example"
TOKEN="Bearer $(cat ~/.config/torii/devnet.token)"
curl   -H "Content-Type: application/x-norito"   -H "Authorization: ${TOKEN}"   --data-binary @fixtures/norito_rpc/transfer_asset.norito   "${TORII}/v1/pipeline/submit"
````transaction_fixtures.manifest.json` - это входная информация о приспособлении, используемая `cargo xtask norito-rpc-fixtures`, возможность кодирования полезной нагрузки. Torii canary mode можно использовать для `curl`, попробуйте прокси-сервер (`https://docs.sora.example/proxy/v1/pipeline/submit`) и установите его. Тестирование инфраструктуры и виджеты портала

## Операции наблюдения

Запрос, метод, путь, источник, статус восходящего потока, источник аутентификации (`override`, `default`, `client`) и журнал регистрации. ہے۔ хранение токенов и заголовки носителей اور `X-TryIt-Auth`, регистрация значений, редактирование и обработка стандартного вывода کو центральный коллектор میں вперед کر سکتے ہیں بغیر утечка секретов ہونے کے خدشے کے۔

### Датчики состояния здоровья اور alerting

График развертываний и встроенный зонд:

```bash
# Ensure the proxy responds to /healthz and forwards a sample request.
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" TRYIT_PROXY_SAMPLE_PATH="/v1/status" npm run probe:tryit-proxy
```

Ручки окружения:

- `TRYIT_PROXY_SAMPLE_PATH` - Маршрут Torii (بغیر `/proxy`)
- `TRYIT_PROXY_SAMPLE_METHOD` - ڈیفالٹ `GET`; запись маршрутов کے لئے `POST` سیٹ کریں۔
- `TRYIT_PROXY_PROBE_TOKEN` - пример вызова для ввода токена на предъявителя
- `TRYIT_PROXY_PROBE_TIMEOUT_MS` - Тайм-аут 5 с для переопределения значения
- `TRYIT_PROXY_PROBE_METRICS_FILE` - `probe_success`/`probe_duration_seconds` Указание места назначения Prometheus текстового файла.
- `TRYIT_PROXY_PROBE_LABELS` - `key=value` جوڑے метрики, разделенные запятыми, میں شامل ہوتے ہیں (ڈیفالٹ `job=tryit-proxy` اور `instance=<proxy URL>`).
- `TRYIT_PROXY_PROBE_METRICS_URL` - URL-адрес конечной точки метрики (например, `http://localhost:9798/metrics`) и `TRYIT_PROXY_METRICS_LISTEN` для получения дополнительной информации. جواب دے۔

Сборщик текстовых файлов, сборщик текстовых файлов, зонд, записываемый путь, а также путь к записи.
(Для `/var/lib/node_exporter/textfile_collector/tryit.prom`) Дополнительные этикетки:

```bash
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" TRYIT_PROXY_PROBE_METRICS_FILE="/var/lib/node_exporter/textfile_collector/tryit.prom" TRYIT_PROXY_PROBE_LABELS="job=tryit-proxy,cluster=prod" npm run probe:tryit-proxy
```

Используйте метрики и атомарную перезапись, а также сборщик данных и полезную нагрузку.

جب `TRYIT_PROXY_METRICS_LISTEN` configure ہو تو `TRYIT_PROXY_PROBE_METRICS_URL` کو metrics endpoint پر سیٹ کریں تاکہ probe تیزی سے fail ہو اگر scrape surface غائب ہو جائے (неправильно настроен вход یا, отсутствуют правила брандмауэра)۔ ایک типичная производственная установка ہے
`TRYIT_PROXY_PROBE_METRICS_URL="http://127.0.0.1:9798/metrics"`۔

Доступ к оповещениям и зондированию и использованию стека мониторинга Prometheus при возникновении сбоев на странице:

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

### Конечная точка показателей на информационных панелях

`TRYIT_PROXY_METRICS_LISTEN=127.0.0.1:9798` (хост/порт) Доступ к запуску прокси-сервера Prometheus-форматирование конечной точки метрик. путь `/metrics` ہے لیکن `TRYIT_PROXY_METRICS_PATH=/custom` سے بدل سکتے ہیں۔ Сбор итоговых данных по методам, отклонения по ограничению скорости, ошибки/таймауты восходящего потока, результаты прокси-сервера, сводные данные о задержках и многое другое:

```bash
export TRYIT_PROXY_METRICS_LISTEN="127.0.0.1:9798"
npm run tryit-proxy &
curl http://127.0.0.1:9798/metrics | head -n 5
# HELP tryit_proxy_requests_total Requests handled by method
tryit_proxy_requests_total{method="GET"} 12
tryit_proxy_rate_limited_total 1
```

Сборщики Prometheus/OTLP для конечной точки метрик и возможности использования `dashboards/grafana/docs_portal.json` для существующих панелей для повторного использования и хвоста SRE задержки и резкие скачки отклонений, анализ журналов и т. д. прокси-сервер خودکار طور پر `tryit_proxy_start_timestamp_ms` опубликовать کرتا ہے تاکہ операторы перезапускает обнаружение کر سکیں۔

### Автоматизация отката

помощник по управлению Torii целевой URL-адрес обновить или восстановить Для конфигурации `.env.tryit-proxy.bak` необходимо выполнить откат или откат.```bash
# Update TRYIT_PROXY_TARGET and back up the previous config.
npm run manage:tryit-proxy -- update --target https://torii.devnet.sora.example

# Roll back to the previously backed-up target.
npm run manage:tryit-proxy -- rollback
```

Для конфигурации развертывания выберите `--env` или `TRYIT_PROXY_ENV` для env. راستہ переопределить کریں۔