---
lang: ru
direction: ltr
source: docs/portal/docs/devportal/try-it.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Песочница «Попробуй это»

Портал desarrolladores включает дополнительную консоль «Попробуйте», чтобы можно было открыть конечные точки Torii без проверки документации. Консоль ретранслирует запросы и включает прокси-сервер, чтобы навигаторы могли отменить ограничения CORS, пока они не будут применены к ограничениям ввода и аутентификации.

## Предварительные условия

- Node.js 18.18 или новая версия (совпадает с требованиями для сборки портала)
- Красный доступ к промежуточному устройству Torii.
- Токен на предъявителя, который может быть использован для рутинных операций Torii que planeas ejercitar

Все настройки прокси-сервера реализуют промежуточные переменные. На следующей таблице перечислены наиболее важные ручки:

| Переменная | Предложение | По умолчанию |
| --- | --- | --- |
| `TRYIT_PROXY_TARGET` | База URL-адресов Torii, похожая на прокси-сервер, повторно отправленный заботами | **Обязательно** |
| `TRYIT_PROXY_LISTEN` | Направление получения локальной информации (формат `host:port` или `[ipv6]:port`) | `127.0.0.1:8787` |
| `TRYIT_PROXY_ALLOWED_ORIGINS` | Список отдельных источников происхождения, которые могут вызвать прокси | `http://localhost:3000` |
| `TRYIT_PROXY_CLIENT_ID` | Идентификатор цвета в `X-TryIt-Client` для каждого запроса вверх по течению | `docs-portal` |
| `TRYIT_PROXY_BEARER` | Токен на предъявителя из-за дефекта повторно отправлен на Torii | _пустой_ |
| `TRYIT_PROXY_ALLOW_CLIENT_AUTH` | Разрешите пользователям использовать собственный токен через `X-TryIt-Auth` | `0` |
| `TRYIT_PROXY_MAX_BODY` | Максимальное количество запросов (в байтах) | `1048576` |
| `TRYIT_PROXY_TIMEOUT_MS` | Тайм-аут восходящего потока в нескольких милях | `10000` |
| `TRYIT_PROXY_RATE_LIMIT` | Разрешение на въезд в офис для IP-клиента | `60` |
| `TRYIT_PROXY_RATE_WINDOW_MS` | Удаление ограничения скорости (мс) | `60000` |
| `TRYIT_PROXY_METRICS_LISTEN` | Дополнительное направление просмотра для конечной точки метрик стиля Prometheus (`host:port` или `[ipv6]:port`) | _пусто (отключено)_ |
| `TRYIT_PROXY_METRICS_PATH` | Передача HTTP-сервиса для конечной точки метрик | `/metrics` |

Прокси-сервер также экспонируется `GET /healthz`, устраняет ошибки структурированного JSON и скрывает токены-носители журналов.

Активируйте `TRYIT_PROXY_ALLOW_CLIENT_AUTH=1` и используйте прокси-сервер для пользователей документов, чтобы панели Swagger и RapiDoc могли повторно отправить токены на предъявителя, пропорциональные пользователю. Прокси и ограничения приложения, скрытые учетные данные и регистрация, если вы запросили использование токена для дефекта или аннулирования для запроса. Настройте `TRYIT_PROXY_CLIENT_ID` с этикетом, который хочет отправить как `X-TryIt-Client`
(по дефекту `docs-portal`). Прокси-запись и валидные значения `X-TryIt-Client`, переносимые для клиента, по умолчанию используются для того, чтобы промежуточные шлюзы могли проверять процесс без корреляционных метаданных навигатора.

## Начало локального прокси

Установите зависимости от того, как настраивает портал:

```bash
cd docs/portal
npm install
```

Вызов прокси-сервера и подключение к вашему экземпляру Torii:

```bash
export TRYIT_PROXY_TARGET="https://torii.devnet.sora.example"
export TRYIT_PROXY_ALLOWED_ORIGINS="http://localhost:3000"
# Optional: preset a bearer token for the Swagger / RapiDoc panels
export TRYIT_PROXY_BEARER="Bearer eyJhbGciOi..."
npm run tryit-proxy
```Сценарий регистрации направления и повторной отправки по запросу `/proxy/*` в исходном Torii сконфигурирован.

Прежде чем сканировать сокет, действителен сценарий
`static/openapi/torii.json` совпадают с зарегистрированным дайджестом
`static/openapi/manifest.json`. Если архивы будут десинхронизированы, команда закончится с
ошибка и указан индикатор `npm run sync-openapi -- --latest`. Экспорта
`TRYIT_PROXY_ALLOW_STALE_SPEC=1` только для отключения экстренной помощи; регистрация прокси-сервера
предупреждение и продолжение, чтобы можно было восстановить силы во время вентиляции.

## Подключите виджеты портала

Когда вам нужно создать портал для загрузки, определить URL-адрес виджетов
Необходимо использовать прокси-сервер:

```bash
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
export TRYIT_PROXY_DEFAULT_BEARER="Bearer eyJhbGciOi..." # Optional
npm run start
```

Следующие компоненты имеют эти значения для `docusaurus.config.js`:

- **Swagger UI** — рендеринг в `/reference/torii-swagger`; предварительное авторизация эль-эскемы
  предъявитель cuando hay un token, этикет заботы с `X-TryIt-Client`,
  inyecta `X-TryIt-Auth`, вы перепишите las lamadas a traves del proxy cuando
  `TRYIT_PROXY_PUBLIC_URL` уже определен.
- **RapiDoc** — рендеринг в `/reference/torii-rapidoc`; отражение эль кампо де жетон,
  повторно использовать заголовки Mismos, которые находятся на панели Swagger, и использовать прокси-сервер
  автоматически, когда URL-адрес настроен.
- **Попробуйте консоль** - вставьте на страницу обзора API; позвольте завидовать
  персонализированные заботы, заголовки и проверки служб реагирования.

Есть панели, которые нужно использовать для **селектора снимков**, которые вам нужны
`docs/portal/static/openapi/versions.json`. Узнайте об этом индексе
`npm run sync-openapi -- --version=<label> --mirror=current --latest` для рецензентов
можно просмотреть все исторические характеристики, проверить зарегистрированный дайджест SHA-256 и подтвердить его.
Снимок выпуска представляет собой манифест фирмы перед использованием интерактивных виджетов.

Выберите токен и другой виджет, который будет влиять на текущий сеанс навигации; Эль-прокси-нунка
Продолжайте регистрировать соответствующий токен.

## Токены OAuth Corta Vida

Чтобы избежать распространения токенов Torii в течение длительного времени для рецензентов, подключитесь к консоли. Попробуйте
ваш сервер OAuth. Когда переменные исчезнут, появится портал рендеринга
виджет входа в систему с кодом устройства, общими токенами на предъявителя карты жизни и автоматическим вводом
в формуле консоли.| Переменная | Предложение | По умолчанию |
| --- | --- | --- |
| `DOCS_OAUTH_DEVICE_CODE_URL` | Конечная точка авторизации устройства OAuth (`/oauth/device/code`) | _пусто (отключено)_ |
| `DOCS_OAUTH_TOKEN_URL` | Конечная точка токена, который принимает `grant_type=urn:ietf:params:oauth:grant-type:device_code` | _пустой_ |
| `DOCS_OAUTH_CLIENT_ID` | Идентификатор клиента, зарегистрированный по OAuth, для предварительного просмотра документов | _пустой_ |
| `DOCS_OAUTH_SCOPE` | Разделы областей, запрашиваемых во время входа в систему | `openid profile offline_access` |
| `DOCS_OAUTH_AUDIENCE` | Дополнительная аудитория API для оплаты токена | _пустой_ |
| `DOCS_OAUTH_POLL_INTERVAL_MS` | Минимальный интервал опроса в период ожидания ожидания (мс) | `5000` (значения < 5000 мс при повторе) |
| `DOCS_OAUTH_DEVICE_CODE_TTL_SECONDS` | Уведомление об истечении срока действия кода устройства (второе) | `600` (должно пройти от 300 до 900 с) |
| `DOCS_OAUTH_TOKEN_TTL_SECONDS` | Продолжительность токена доступа (секунды) | `900` (длительностью от 300 до 900 с) |
| `DOCS_OAUTH_ALLOW_INSECURE` | `1` для предварительного просмотра локалей, которые намеренно пропускают принудительное применение OAuth | _закат_ |

Пример конфигурации:

```bash
export DOCS_OAUTH_DEVICE_CODE_URL="https://auth.dev.sora.example/oauth/device/code"
export DOCS_OAUTH_TOKEN_URL="https://auth.dev.sora.example/oauth/token"
export DOCS_OAUTH_CLIENT_ID="docs-preview"
export DOCS_OAUTH_SCOPE="torii openid offline_access"
# Optional audience and polling tweaks
export DOCS_OAUTH_AUDIENCE="https://torii.devnet.sora.example"
export DOCS_OAUTH_POLL_INTERVAL_MS="6000"
```

Когда вы выбрасываете `npm run start` или `npm run build`, портал становится ценным.
ru `docusaurus.config.js`. Durante un Preview Local La Tarjeta Попробуйте это muestra un
кнопка «Войти по коду устройства». Пользователи вводят код, отправленный на страницу проверки OAuth; то, что происходит с потоком устройства и выходом из виджета:

- ввод токена на предъявителя в эль-кампо-де-ла-консола. Попробуйте,
- этикет заботы о существующих заголовках `X-TryIt-Client` и `X-TryIt-Auth`,
- Муэстра эль таймпо де vida restante, y
- Автоматическая загрузка токена после истечения срока действия.

La entrada manual Bearer sigue доступен; опустить переменные OAuth, если вы хотите
forzar a los рецензенты a pegar un token temporal ellos mismos, o Exporta
`DOCS_OAUTH_ALLOW_INSECURE=1` для предварительного просмотра языков, доступных для анонимного доступа
приемлемо. Не удалось настроить OAuth, чтобы быстро получить доступ к воротам.
дорожная карта DOCS-1b.

Примечание: Пересмотр [Контрольный список повышения безопасности и проверки на проникновение] (./security-hardening.md)
до открытия портала лаборатории; документация модели угроз,
Профиль CSP/Trusted Types, а также этапы пен-теста, которые сейчас блокируют DOCS-1b.

## Муэстрас Norito-RPC

Заботы Norito-RPC объединяют прокси-сервер и сантехнику OAuth, которые используются в формате JSON;
Простая настройка `Content-Type: application/x-norito` и передача полезной нагрузки Norito
предварительное описание в спецификации NRPC
(`docs/source/torii/nrpc_spec.md`).
Репозиторий включает полезные данные canonicos bajo `fixtures/norito_rpc/` для авторов портала,
Владельцы SDK и рецензенты могут воспроизводить точные байты, которые использует CI в США.

### Отправьте полезную нагрузку Norito из консоли. Попробуйте1. Используйте приспособление как `fixtures/norito_rpc/transfer_asset.norito`. Эстос
   архивные конверты сына Norito en bruto; **нет** кодировок в base64.
2. В Swagger или RapiDoc локализуйте конечную точку NRPC (например,
   `POST /v2/pipeline/submit`) и выберите селектор **Content-Type** a
   `application/x-norito`.
3. Откройте редактор запроса в **двоичном формате** (режим «Файл» Swagger или
   селектор «Бинарный/Файл» RapiDoc) и загрузите архив `.norito`. Эль виджет
   передать байты и пройти через прокси без изменения.
4. Пожелание по просьбе. Si Torii devuelve `X-Iroha-Error-Code: schema_mismatch`,
   проверка того, что есть конечная точка, которая принимает двоичные файлы полезной нагрузки и подтверждает
   что хэш схемы зарегистрирован в `fixtures/norito_rpc/schema_hashes.json`
   совпадают со сборкой Torii, которая используется.

La console mantiene el archive mas reciente en memoria, чтобы можно было снова просмотреть это послание
полезная нагрузка включает разные токены авторизации или хосты Torii. Агрегар
`scripts/run_norito_rpc_fixtures.sh --note "<ticket>"` в рабочем процессе создания пакета
справочные доказательства в плане внедрения NRPC-4 (журнал + резюме в формате JSON), локальная комбинация
лучше делать снимки экрана в ответ. Попробуйте во время внесения изменений.

### Пример CLI (завиток)

Приборы Los Mismos могут воспроизводиться с портала через `curl`, вот что используется.
Когда действителен прокси-сервер или проверяются ответы шлюза:

```bash
TORII="https://torii.devnet.sora.example"
TOKEN="Bearer $(cat ~/.config/torii/devnet.token)"
curl   -H "Content-Type: application/x-norito"   -H "Authorization: ${TOKEN}"   --data-binary @fixtures/norito_rpc/transfer_asset.norito   "${TORII}/v2/pipeline/submit"
```

Установите приспособление для входа в список `transaction_fixtures.manifest.json`
o кодификация собственной полезной нагрузки с `cargo xtask norito-rpc-fixtures`. Cuando Torii находится в
modo canary puedes apuntar `curl` al proxy попробуйте
(`https://docs.sora.example/proxy/v2/pipeline/submit`) для чтения ошибок
Инфраструктура, которая использует виджеты портала.

## Наблюдение и операции

Cada solicitud se registra una vez con metodo, path, origin, estado upstream y la fuente
аутентификации (`override`, `default` или `client`). Los tokens nunca se almacenan: tanto
носители заголовков как значения `X-TryIt-Auth`, измененные перед регистратором,
так как вы можете снова открыть центральный коллектор, не заботясь о фильтрации.

### Зонды спасения и оповещения

Включение зонда во время предварительного расписания:

```bash
# Ensure the proxy responds to /healthz and forwards a sample request.
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" TRYIT_PROXY_SAMPLE_PATH="/v2/status" npm run probe:tryit-proxy
```

Ручки входа:

- `TRYIT_PROXY_SAMPLE_PATH` - дополнительный режим Torii (без `/proxy`) для извлечения.
- `TRYIT_PROXY_SAMPLE_METHOD` - из-за дефекта `GET`; определите `POST` для рута написания.
- `TRYIT_PROXY_PROBE_TOKEN` - временный токен носителя для звонка по телефону.
- `TRYIT_PROXY_PROBE_TIMEOUT_MS` - пропишите таймаут из-за неисправности 5 с.
- `TRYIT_PROXY_PROBE_METRICS_FILE` — дополнительный текст Prometheus для `probe_success`/`probe_duration_seconds`.
- `TRYIT_PROXY_PROBE_LABELS` - разделяет `key=value` из-за присоединения к метрикам (из-за дефекта `job=tryit-proxy` и `instance=<proxy URL>`).
- `TRYIT_PROXY_PROBE_METRICS_URL` — необязательный URL-адрес конечной точки метрик (например, `http://localhost:9798/metrics`), который должен быть ответчиком при выходе из `TRYIT_PROXY_METRICS_LISTEN`, который является привычным.Получение результатов в сборщике текстовых файлов с помощью зонда, записываемого по правилам
(например, `/var/lib/node_exporter/textfile_collector/tryit.prom`) и общие этикеты
персонализации:

```bash
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" TRYIT_PROXY_PROBE_METRICS_FILE="/var/lib/node_exporter/textfile_collector/tryit.prom" TRYIT_PROXY_PROBE_LABELS="job=tryit-proxy,cluster=prod" npm run probe:tryit-proxy
```

Сценарий переписывает архив метрик атомной формы, чтобы ваш сборщик всегда мог его использовать.
полная полезная нагрузка.

Когда `TRYIT_PROXY_METRICS_LISTEN` настроен, определите
`TRYIT_PROXY_PROBE_METRICS_URL` аль конечная точка метрики, чтобы зонд упал быстро
неудовлетворительная очистка (например, вход в неправильную конфигурацию или нарушение правил брандмауэра).
Скорректированный тип производства
`TRYIT_PROXY_PROBE_METRICS_URL="http://127.0.0.1:9798/metrics"`.

Чтобы предупредить жителей Ливана, подключите зонд к стеку монитора. Пример Prometheus, который
страница после дос-фаллос последовательных:

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

### Конечная точка показателей и информационных панелей

Настройте `TRYIT_PROXY_METRICS_LISTEN=127.0.0.1:9798` (по запросу хоста/пуэрто) до
инициализирует прокси-сервер для отображения конечной точки метрик в формате Prometheus. Ла Рута
Из-за дефекта `/metrics` он может быть изменен с `TRYIT_PROXY_METRICS_PATH=/custom`. Када
очистка данных по общему методу, очистка лимита скорости, ошибки/тайм-ауты
восходящий поток, результаты прокси и возобновления задержки:

```bash
export TRYIT_PROXY_METRICS_LISTEN="127.0.0.1:9798"
npm run tryit-proxy &
curl http://127.0.0.1:9798/metrics | head -n 5
# HELP tryit_proxy_requests_total Requests handled by method
tryit_proxy_requests_total{method="GET"} 12
tryit_proxy_rate_limited_total 1
```

Используйте сборщики Prometheus/OTLP для конечных точек метрик и повторного использования существующих панелей
en `dashboards/grafana/docs_portal.json`, чтобы SRE наблюдал за задержкой колы и фотографиями
rechazo грех разбирать логи. Публичный прокси автоматически `tryit_proxy_start_timestamp_ms`
чтобы помочь операторам и детекторам.

### Автоматизация отката

Используйте помощника для актуализации или восстановления URL-адреса объекта Torii. Эль сценарий
Предварительная настройка конфигурации на `.env.tryit-proxy.bak` для откатов, Шон ООН
одиночный коммандос.

```bash
# Update TRYIT_PROXY_TARGET and back up the previous config.
npm run manage:tryit-proxy -- update --target https://torii.devnet.sora.example

# Roll back to the previously backed-up target.
npm run manage:tryit-proxy -- rollback
```

Напишите маршрут архивной среды с `--env` или `TRYIT_PROXY_ENV`, если вы хотите
Проверьте конфигурацию в другом месте.