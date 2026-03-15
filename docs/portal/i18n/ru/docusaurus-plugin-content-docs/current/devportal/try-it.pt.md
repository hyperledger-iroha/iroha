---
lang: ru
direction: ltr
source: docs/portal/docs/devportal/try-it.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Песочница, попробуйте

Портал разработчиков включает дополнительную консоль «Попробуйте», чтобы вы могли указать конечные точки Torii в документе. На консоли повторно передайте необходимые реквизиты с помощью прокси-сервера, чтобы навигаторы могли поддерживать ограничения CORS, а также ограничения скорости и аутентификации приложений.

## Предварительные требования

— Node.js 18.18 или более новая версия (сочетание всех требований для создания портала)
- Доступ к среде постановки Torii
- Токен на предъявителя, который может быть использован по мере ротации Torii, который вы говорите, притворяясь, что выполняете упражнение.

Далее необходимо настроить прокси и настройки для изменения окружения. В таблице ниже перечислены наиболее важные ручки:

| Переменная | Предложение | По умолчанию |
| --- | --- | --- |
| `TRYIT_PROXY_TARGET` | База URL-адресов Torii для обеспечения требуемых прокси-серверов | **Обязательно** |
| `TRYIT_PROXY_LISTEN` | Открытие экрана для локального использования (формат `host:port` или `[ipv6]:port`) | `127.0.0.1:8787` |
| `TRYIT_PROXY_ALLOWED_ORIGINS` | Отдельный список от происхождения, который может помочь или прокси | `http://localhost:3000` |
| `TRYIT_PROXY_CLIENT_ID` | Идентификатор номера `X-TryIt-Client` для всех необходимых восходящих потоков | `docs-portal` |
| `TRYIT_PROXY_BEARER` | Токен на предъявителя Padrao Encaminhado ao Torii | _пустой_ |
| `TRYIT_PROXY_ALLOW_CLIENT_AUTH` | Разрешите окончательным пользователям использовать собственный токен через `X-TryIt-Auth` | `0` |
| `TRYIT_PROXY_MAX_BODY` | Максимальное количество требуемых тел (байты) | `1048576` |
| `TRYIT_PROXY_TIMEOUT_MS` | Тайм-аут в восходящем направлении в милиссундосах | `10000` |
| `TRYIT_PROXY_RATE_LIMIT` | Реквизиты разрешений для налоговых деклараций для IP-адресов клиентов | `60` |
| `TRYIT_PROXY_RATE_WINDOW_MS` | Джанела очистила параметр ограничения скорости (мс) | `60000` |
| `TRYIT_PROXY_METRICS_LISTEN` | Дополнительная дополнительная информация для конечной точки метрик стиля Prometheus (`host:port` или `[ipv6]:port`) | _пусто (отключено)_ |
| `TRYIT_PROXY_METRICS_PATH` | HTTP-сервис Caminho, конечная точка метрик | `/metrics` |

При использовании прокси-сервера `GET /healthz` в журналах возвращаются ошибки JSON и токены на предъявителя туши.

Активный `TRYIT_PROXY_ALLOW_CLIENT_AUTH=1` является экспортом или прокси-сервером для пользователей документов, которые позволяют Swagger и RapiDoc передавать токены на предъявителя для использования пользователями. В качестве прокси-сервера используются ограничения по налогам, тушь для ресниц и регистрация требуемого товара с использованием токена или переопределения запроса. Настройте `TRYIT_PROXY_CLIENT_ID` как `X-TryIt-Client` или поверните его, чтобы услышать, как передается `X-TryIt-Client`.
(падрао `docs-portal`). В случае прокси-корта и валидных значений `X-TryIt-Client` для клиента необходимо указать значение по умолчанию для того, чтобы шлюзы промежуточного состояния аудита выполнялись с корреляционными метаданными для навигации.

## Начало или локальный прокси

Установите зависимости от того, как настроить портал:

```bash
cd docs/portal
npm install
```

Выберите прокси и ответ на свой экземпляр Torii:

```bash
export TRYIT_PROXY_TARGET="https://torii.devnet.sora.example"
export TRYIT_PROXY_ALLOWED_ORIGINS="http://localhost:3000"
# Optional: preset a bearer token for the Swagger / RapiDoc panels
export TRYIT_PROXY_BEARER="Bearer eyJhbGciOi..."
npm run tryit-proxy
```

Сценарий регистрации или завершения соединения и ввода реквизитов `/proxy/*` для первоначальной настройки Torii.Antes de fazer не привязывает сокет или не проверяет скрипт
`static/openapi/torii.json` соответствует зарегистрированному дайджесту
`static/openapi/manifest.json`. Если вы находитесь в растерянных архивах, или команда войдет в ошибку,
проинструктируйте исполнителя `npm run sync-openapi -- --latest`. Экспорт
`TRYIT_PROXY_ALLOW_STALE_SPEC=1` вызывается для аварийной ситуации; о регистрации через прокси, предупреждаю
И продолжайте говорить, что вы сможете восстановить силы во время ручного управления.

## Подключите виджеты к порталу

Когда вы создаете портал для разработчиков или обслуживаете его, определите URL-адрес для разработки виджетов.
использовать прокси:

```bash
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
export TRYIT_PROXY_DEFAULT_BEARER="Bearer eyJhbGciOi..." # Optional
npm run start
```

Наши компоненты имеют следующие значения `docusaurus.config.js`:

- **Swagger UI** — рендеринг в `/reference/torii-swagger`; предварительная авторизация или эскема
  предъявитель, когда токен, марка requisicoes com `X-TryIt-Client`,
  введите `X-TryIt-Auth` и загрузите прокси-сервер, когда
  `TRYIT_PROXY_PUBLIC_URL` настроен.
- **RapiDoc** — рендеринг в `/reference/torii-rapidoc`; Эспелья о Кампо де Токен,
  повторно использовать заголовки сообщений, используя Swagger и aponta для прокси
  автоматически, когда URL-адрес настроен.
- **Попробуйте консоль** - добавлена ​​страница с обзором API; позвольте завидовать
  персонализированные реквизиты, заголовки и проверка корпусов ответов.

Os dois Paineis Mostram um **селектор снимков**, который
`docs/portal/static/openapi/versions.json`. Preencha esse indice com
`npm run sync-openapi -- --version=<label> --mirror=current --latest` для рецензентов
альтернативные варианты между историческими спецификациями, версия дайджеста SHA-256, зарегистрированная и подтвержденная
снимок выпуска выпуска и манифест до использования интерактивных виджетов.

Мудар или токен в любом виджете, чтобы автоматически начать навигацию; o прокси-сервер nunca persiste
не регистрируйтесь или не регистрируйтесь.

## Токены OAuth для Curta Duracao

Чтобы отменить распространение токенов Torii в течение долгого времени для рецензентов, подключитесь к консоли. Попробуйте это или
свой сервер OAuth. Когда представлены варианты окружающей среды, или портал рендеринга
гм виджет для входа в систему с кодом устройства, дополнительные токены на предъявителя de curta duracao и os injeta
автоматически не используется формула консоли.

| Переменная | Предложение | По умолчанию |
| --- | --- | --- |
| `DOCS_OAUTH_DEVICE_CODE_URL` | Конечная точка авторизации устройства OAuth (`/oauth/device/code`) | _пусто (отключено)_ |
| `DOCS_OAUTH_TOKEN_URL` | Конечная точка токена, который соответствует `grant_type=urn:ietf:params:oauth:grant-type:device_code` | _пустой_ |
| `DOCS_OAUTH_CLIENT_ID` | Идентификатор клиента, зарегистрированный по OAuth, для предварительного просмотра документов | _пустой_ |
| `DOCS_OAUTH_SCOPE` | Разделенные области для запроса на поиск нет входа в систему | `openid profile offline_access` |
| `DOCS_OAUTH_AUDIENCE` | Дополнительная аудитория API для валюты или токена | _пустой_ |
| `DOCS_OAUTH_POLL_INTERVAL_MS` | Минимальный интервал опроса после подтверждения (мс) | `5000` (значения < 5000 мс при сохранении) |
| `DOCS_OAUTH_DEVICE_CODE_TTL_SECONDS` | Срок действия кода устройства (второй) | `600` (время от 300 до 900 с) |
| `DOCS_OAUTH_TOKEN_TTL_SECONDS` | Дюрасао делает токен доступа (вторые) | `900` (время от 300 до 900 с) |
| `DOCS_OAUTH_ALLOW_INSECURE` | Defina `1` для предварительного просмотра в местах, где требуется принудительное применение OAuth | _закат_ |

Пример конфигурации:

```bash
export DOCS_OAUTH_DEVICE_CODE_URL="https://auth.dev.sora.example/oauth/device/code"
export DOCS_OAUTH_TOKEN_URL="https://auth.dev.sora.example/oauth/token"
export DOCS_OAUTH_CLIENT_ID="docs-preview"
export DOCS_OAUTH_SCOPE="torii openid offline_access"
# Optional audience and polling tweaks
export DOCS_OAUTH_AUDIENCE="https://torii.devnet.sora.example"
export DOCS_OAUTH_POLL_INTERVAL_MS="6000"
```Когда вы произносите род `npm run start` или `npm run build`, на портале есть ценность.
`docusaurus.config.js`. Durante um предварительный просмотр локальной карты Попробуйте Mostra um botao
«Войти с кодом устройства». Пользователи, использующие цифры или коды, отправляются на свою страницу OAuth; Когда успешно выполняется работа с устройством или виджетом:

- ввод или токен на предъявителя Emitido no Campo Do Console Попробуйте,
- требования к маркам с существующими заголовками ОС `X-TryIt-Client` и `X-TryIt-Auth`,
- exibe o tempo de vida restante, e
- Автоматическая очистка токена по истечении срока действия.

Руководство по вводу носителя, продолжающее распространяться; опустить как вариант OAuth при запросе
для рецензентов, использующих временный токен для собственной собственности или экспорта
`DOCS_OAUTH_ALLOW_INSECURE=1` для предварительного просмотра изолированных мест при анонимном доступе и
aceitavel. Создает собственную конфигурацию OAuth перед быстрым запуском для входа или входа.
дорожная карта DOCS-1b.

Примечание. Пересмотрите [Контрольный список повышения безопасности и проверки проникновения] (./security-hardening.md)
перед экспортом или порталом для лаборатории; электронная документация или модель угроз,
o Пройдите CSP/Trusted Types и пройдите пен-тест, который ранее был заблокирован DOCS-1b.

## Амостра Norito-RPC

Реквизиты Norito-RPC, совместимые с собственным прокси-сервером и сантехническим протоколом OAuth, которые меняются в формате JSON;
eles apenas definem `Content-Type: application/x-norito` и enviam o payload Norito
предварительное описание конкретного NRPC
(`docs/source/torii/nrpc_spec.md`).
В репозитории содержатся полезные данные canonicos sob `fixtures/norito_rpc/` для авторов, которые это делают.
Портал, владельцы SDK и рецензенты могут воспроизвести существующие байты, которые есть в CI США.

### Enviar um payload Norito pelo console Попробуйте

1. Крепление Escolha как `fixtures/norito_rpc/transfer_asset.norito`. Эссес
   arquivos sao конверты Norito brutos; **нао** faca base64.
2. Используя Swagger или RapiDoc, локализуйте конечную точку NRPC (например,
   `POST /v2/pipeline/submit`) и измените селектор **Content-Type** для
   `application/x-norito`.
3. Работайте с редактором корпуса для **двоичного** файла (режим «Файл» для Swagger или
   выберите «Двоичный файл/Двоичный файл» в RapiDoc) и завидуйте архиву `.norito`. О виджет
   передавать байты через прокси-сервер с возможностью изменения.
4. Завидуйте реквизиции. Se o Torii retornar `X-Iroha-Error-Code: schema_mismatch`,
   проверьте, есть ли у вас конечная точка, которая подтверждает бинарную нагрузку и подтверждает
   что хэш схемы зарегистрирован в `fixtures/norito_rpc/schema_hashes.json`
   соответствует сборке Torii, которую вы использовали.

O console mantem или arquivo mais new memoria, чтобы вы могли повторно просмотреть или написать сообщение
полезная нагрузка, содержащая тесты различных токенов авторизации или хостов Torii. Дополнительный
`scripts/run_norito_rpc_fixtures.sh --note "<ticket>"` в качестве рабочего процесса продукта или пакета
 справочное доказательство без плана обработки NRPC-4 (журнал + резюме в формате JSON), которое объединяется
com захватывайте снимки экрана и отвечайте. Попробуйте во время обзоров.

### Пример интерфейса командной строки (curl)

Наши основные светильники могут быть воспроизведены на портале через `curl`, или это поможет
Когда вы подтвердите действие прокси-сервера или получите ответ на шлюз:

```bash
TORII="https://torii.devnet.sora.example"
TOKEN="Bearer $(cat ~/.config/torii/devnet.token)"
curl   -H "Content-Type: application/x-norito"   -H "Authorization: ${TOKEN}"   --data-binary @fixtures/norito_rpc/transfer_asset.norito   "${TORII}/v2/pipeline/submit"
```Тройка или приспособление для того, чтобы войти в список `transaction_fixtures.manifest.json`
или закодировать свою собственную полезную нагрузку с помощью `cargo xtask norito-rpc-fixtures`. Когда есть Torii
em modo canary voce pode pode o `curl` для прокси-сервера, попробуйте
(`https://docs.sora.example/proxy/v2/pipeline/submit`) для упражнений
Виджеты infraestrutura usada pelos делают портал.

## Наблюдение и работа

Cada requisicao e registrada uma vez com метод, путь, происхождение, статус восходящего потока и шрифт
аутентификации (`override`, `default` или `client`). Токены nunca sao Armazenados: tanto
Носитель заголовков ОС, сколько os valores `X-TryIt-Auth` sao redigidos ante do log,
entao voce pode encaminhar stdout для центрального коллекционера, который озабочен своими важными вопросами.

### Проверка безопасности и оповещения

Включенный зонд в течение всего времени развертывает расписание:

```bash
# Ensure the proxy responds to /healthz and forwards a sample request.
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" TRYIT_PROXY_SAMPLE_PATH="/v2/status" npm run probe:tryit-proxy
```

Ручки окружения:

- `TRYIT_PROXY_SAMPLE_PATH` - поменяйте Torii дополнительно (sem `/proxy`) для тренировок.
- `TRYIT_PROXY_SAMPLE_METHOD` - падрао `GET`; Определено `POST` для ротационного письма.
- `TRYIT_PROXY_PROBE_TOKEN` — временная ввод токена на предъявителя для обмена информацией.
- `TRYIT_PROXY_PROBE_TIMEOUT_MS` - зафиксировать тайм-аут на 5 с.
- `TRYIT_PROXY_PROBE_METRICS_FILE` — место назначения текста Prometheus, необязательно для `probe_success`/`probe_duration_seconds`.
- `TRYIT_PROXY_PROBE_LABELS` - разделяет `key=value`, разделенный на отдельные числа, как метрики (padrao `job=tryit-proxy` и `instance=<proxy URL>`).
- `TRYIT_PROXY_PROBE_METRICS_URL` — необязательный URL-адрес конечной точки метрик (например, `http://localhost:9798/metrics`), который должен ответить на успех, когда `TRYIT_PROXY_METRICS_LISTEN` является привычным.

Получение результатов в сборщике текстовых файлов, а затем в ходе проверки для получения гравия
(например, `/var/lib/node_exporter/textfile_collector/tryit.prom`) и дополнительные этикетки
настройки:

```bash
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" TRYIT_PROXY_PROBE_METRICS_FILE="/var/lib/node_exporter/textfile_collector/tryit.prom" TRYIT_PROXY_PROBE_LABELS="job=tryit-proxy,cluster=prod" npm run probe:tryit-proxy
```

Сценарий резервного копирования или архива метрик атомной формы для того, чтобы ваш сборщик всегда оставался в живых.
хм полезная нагрузка завершена.

Когда `TRYIT_PROXY_METRICS_LISTEN` установлена определенная конфигурация
`TRYIT_PROXY_PROBE_METRICS_URL` для конечной точки метрик, чтобы быстро проверить ошибочность
поверхностная очистка desaparecer (например, вход в неправильную конфигурацию или неправильная настройка брандмауэра).
Скорректируйте тип производства и
`TRYIT_PROXY_PROBE_METRICS_URL="http://127.0.0.1:9798/metrics"`.

Для оповещения на уровнях подключите зонд к своей стойке мониторинга. Пример Prometheus очередь
страница apos duas falhas consecutivas:

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

### Конечная точка метрик и информационных панелей

Defina `TRYIT_PROXY_METRICS_LISTEN=127.0.0.1:9798` (или любой хост/порт) до этого
Инициатор или прокси для экспорта конечной точки метрик без формата Prometheus. О каминьо
Padrao e `/metrics` больше подходит для записи через
`TRYIT_PROXY_METRICS_PATH=/custom`. Cada Scrape Retorna Contadores de Totalis por Methodo,
ограничения по скорости, ошибки/тайм-ауты в восходящем направлении, результаты прокси-сервера и результаты задержки:

```bash
export TRYIT_PROXY_METRICS_LISTEN="127.0.0.1:9798"
npm run tryit-proxy &
curl http://127.0.0.1:9798/metrics | head -n 5
# HELP tryit_proxy_requests_total Requests handled by method
tryit_proxy_requests_total{method="GET"} 12
tryit_proxy_rate_limited_total 1
```

Используйте свои сборщики Prometheus/OTLP для конечной точки метрик и повторного использования панелей ОС.
существует в `dashboards/grafana/docs_portal.json`, чтобы SRE наблюдал за латентностью хвоста
e picos de rejeicao, когда вы анализируете журналы. Публичный прокси автоматически `tryit_proxy_start_timestamp_ms`
для помощи в работе детектора.

### Автоматический откатИспользуйте помощник по созданию для настройки или восстановления URL-адреса Torii. О сценарий
Armazena a configuracao anterior em `.env.tryit-proxy.bak` для отката сейчас um
уникальный командо.

```bash
# Update TRYIT_PROXY_TARGET and back up the previous config.
npm run manage:tryit-proxy -- update --target https://torii.devnet.sora.example

# Roll back to the previously backed-up target.
npm run manage:tryit-proxy -- rollback
```

Сохранение или архивирование env com `--env` или `TRYIT_PROXY_ENV`, если вы используете имплантат
Вы можете настроить конфигурацию в конце.