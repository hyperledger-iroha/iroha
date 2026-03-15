---
lang: ru
direction: ltr
source: docs/portal/docs/devportal/try-it.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Бак соболя Попробуй

Портал разработки включает консольную опцию «Попробуй» для вызова конечных точек Torii без выхода из документации. Консоль передает запросы через прокси-сервер для того, чтобы навигаторы контурировали ограничения CORS, чтобы применить ограничение скорости и аутентификацию.

## Предварительное условие

- Node.js 18.18 или более поздняя версия (соответствует требованиям сборки порта)
- Доступ к промежуточной среде Torii.
- Токен на предъявителя, способный обращаться к маршрутам Torii, который вы хотите протестировать

Вся конфигурация прокси-сервера соответствует переменным среды. Le tableau ci-dessous список ручек и самое важное:

| Переменная | Объектиф | По умолчанию |
| --- | --- | --- |
| `TRYIT_PROXY_TARGET` | URL Torii базовой версии прокси-сервера, передающего запросы | **Обязательно** |
| `TRYIT_PROXY_LISTEN` | Адрес электронной почты для локальной разработки (формат `host:port` или `[ipv6]:port`) | `127.0.0.1:8787` |
| `TRYIT_PROXY_ALLOWED_ORIGINS` | Отдельный список по virgules des origines, авторизованный при обращении к прокси | `http://localhost:3000` |
| `TRYIT_PROXY_CLIENT_ID` | Идентификатор места в `X-TryIt-Client` для запроса вверх по течению | `docs-portal` |
| `TRYIT_PROXY_BEARER` | Токен носителя по умолчанию реле версии Torii | _пустой_ |
| `TRYIT_PROXY_ALLOW_CLIENT_AUTH` | Разрешение пользователям получить собственный токен через `X-TryIt-Auth` | `0` |
| `TRYIT_PROXY_MAX_BODY` | Максимальный размер запроса (в байтах) | `1048576` |
| `TRYIT_PROXY_TIMEOUT_MS` | Тайм-аут восходящего потока в миллисекундах | `10000` |
| `TRYIT_PROXY_RATE_LIMIT` | Авторизованный запрос на доступ ко всем IP-клиентам | `60` |
| `TRYIT_PROXY_RATE_WINDOW_MS` | Ограничение скорости заливки Fenetre glissante (мс) | `60000` |
| `TRYIT_PROXY_METRICS_LISTEN` | Дополнительный адрес адреса для конечной точки измерений в стиле Prometheus (`host:port` или `[ipv6]:port`) | _пусто (отключено)_ |
| `TRYIT_PROXY_METRICS_PATH` | Использование HTTP-сервиса для конечной точки метрик | `/metrics` |

Прокси-сервер предоставляет доступ к `GET /healthz`, отсылает ошибки в структурах JSON и маскирует токены-носители в журналах.

Activez `TRYIT_PROXY_ALLOW_CLIENT_AUTH=1` предоставляет вам доступ к прокси-серверам пользователей, которые используют Swagger и RapiDoc, мощный ретранслятор токенов на предъявителя для пользователя. Приложение прокси-сервера изменяет все ограничения, маскирует учетные данные и регистрирует, если вы запросите использование токена по умолчанию или дополнительную плату по запросу. Настройте `TRYIT_PROXY_CLIENT_ID` с клеветой, которую вы хотите отправить, как `X-TryIt-Client`
(по умолчанию `docs-portal`). Le proxy tronque et valide les valeurs `X-TryIt-Client` Fournies par l'appelant, puis retombe Sur ce Defaut Afin que les Gates de Staging puissent Auditer la Provenance sans Correler les Metadonnees du Navigationur.

## Удаление локального прокси

Установите зависимости от первой конфигурации порта:

```bash
cd docs/portal
npm install
```

Откройте прокси-сервер и укажите свой экземпляр Torii:

```bash
export TRYIT_PROXY_TARGET="https://torii.devnet.sora.example"
export TRYIT_PROXY_ALLOWED_ORIGINS="http://localhost:3000"
# Optional: preset a bearer token for the Swagger / RapiDoc panels
export TRYIT_PROXY_BEARER="Bearer eyJhbGciOi..."
npm run tryit-proxy
```Сценарий записывает адрес лжи и передает запросы от `/proxy/*` против исходного кода Torii configuree.

Avant de Binder le Socket le script, который действителен
`static/openapi/torii.json` соответствует дайджесту и зарегистрируйтесь в нем
`static/openapi/manifest.json`. Если фичи расходятся, команда эхом с тобой
Ошибка и вы потребовали исполнителя `npm run sync-openapi -- --latest`. Экспорт
`TRYIT_PROXY_ALLOW_STALE_SPEC=1` Уникальный идентификатор для отмены срочности; прокси-лог
Предупредите и продолжайте, чтобы вы могли получить возможность рекуперации в местах технического обслуживания.

## Подключение виджетов порта

Когда вы создаете или обслуживаете разработчик портала, определите URL-адрес, который используют виджеты.
Утилизатор для прокси:

```bash
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
export TRYIT_PROXY_DEFAULT_BEARER="Bearer eyJhbGciOi..." # Optional
npm run start
```

Les composants suivants lisent ces valeurs depuis `docusaurus.config.js`:

- **Swagger UI** - откройте `/reference/torii-swagger`; предварительная авторизация носителя схемы
  Когда присутствует токен, тегируйте запросы с `X-TryIt-Client`, вводите
  `X-TryIt-Auth` и повторно запишите обращения через прокси-сервер.
  `TRYIT_PROXY_PUBLIC_URL` определен.
- **RapiDoc** - получить `/reference/torii-rapidoc`; жетон Reflete Le Champ,
  повторно использовать заголовки мемов, которые используются в Panneau Swagger, и автоматически
  URL-адрес прокси-сервера настроен.
- **Попробуйте консоль** - интегрируйте API-интерфейс обзора страницы; разрешение посланника
  запрашивает персонал, прослушивает заголовки и инспектора сил реагирования.

Две панели с возможностью выбора **снимков**, которые горят
`docs/portal/static/openapi/versions.json`. Ремплиссы с этим индексом
`npm run sync-openapi -- --version=<label> --mirror=current --latest` еще есть рецензенты
Мощный проход между историческими спецификациями, регистрация дайджеста SHA-256 и подтверждение
Если снимок выпуска выпущен, он помещается в манифест перед использованием интерактивных виджетов.

Измените токен в виджете, не касаясь текущей навигации по сеансу; ле прокси не
упорствуйте и не допускайте ошибок в знаках фурни.

## Токены OAuth при необходимости

Для прекращения распространения токенов Torii в течение длительного времени для рецензентов, освободите консоль. Попробуйте
Ваш сервер OAuth. Lorsque les переменные d'environnement ci-dessous sont Presentes, le portail
извлеките виджет кода устройства входа в систему, в течение длительного времени извлекайте токены на предъявителя и вводите их.
автоматизация в формуле консоли.| Переменная | Объектиф | По умолчанию |
| --- | --- | --- |
| `DOCS_OAUTH_DEVICE_CODE_URL` | Устройство авторизации конечной точки OAuth (`/oauth/device/code`) | _пусто (отключено)_ |
| `DOCS_OAUTH_TOKEN_URL` | Токен конечной точки, который принимается `grant_type=urn:ietf:params:oauth:grant-type:device_code` | _пустой_ |
| `DOCS_OAUTH_CLIENT_ID` | Регистрация клиента-идентификатора OAuth для предварительного просмотра документов | _пустой_ |
| `DOCS_OAUTH_SCOPE` | Области разграничения пространства, требуемого для входа в систему | `openid profile offline_access` |
| `DOCS_OAUTH_AUDIENCE` | Опции Audience API для токена | _пустой_ |
| `DOCS_OAUTH_POLL_INTERVAL_MS` | Минимальный интервал опроса l'attente d'approbation (мс) | `5000` (значения < 5000 мс, возвращаемые значения) |
| `DOCS_OAUTH_DEVICE_CODE_TTL_SECONDS` | Код устройства Fenetre d'expiration (секунды) | `600` (пауза между 300 с и 900 с) |
| `DOCS_OAUTH_TOKEN_TTL_SECONDS` | Токен доступа к продолжительности жизни (секунды) | `900` (пауза между 300 с и 900 с) |
| `DOCS_OAUTH_ALLOW_INSECURE` | Метр `1` для предварительного просмотра языков, обеспечивающих принудительное соблюдение OAuth | _закат_ |

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

Когда вы нажмете `npm run start` или `npm run build`, интегрированный порт будет иметь эти значения
в `docusaurus.config.js`. Предварительный просмотр по меню. Попробуйте приложение с бутоном.
«Войти с кодом устройства». Пользователи прикрепят код к вашей странице OAuth; Вот что нужно сделать, чтобы поток устройства повторно использовал виджет:

- внедрить токен носителя emis в консоль le champ. Попробуйте,
- запрашиваемые файлы с существующими заголовками `X-TryIt-Client` и `X-TryIt-Auth`,
- афиш la duree restante и др.
- Автоматическое удаление токена по истечении срока его действия.

L'entree manuelle Bearer reste disponible; ометез переменных OAuth, чтобы вы могли выбрать
заставить рецензентов собирать временные токены eux-memes или экспортировать их
`DOCS_OAUTH_ALLOW_INSECURE=1` для предварительного просмотра изолированных локалей или анонимного доступа
приемлемо. Сборка без OAuth, настройка быстрого обслуживания эхо, для удовлетворения
для удовлетворения требований дорожной карты DOCS-1b.

Примечание. Проконсультируйтесь с [Контрольный список повышения безопасности и проверки проникновения] (./security-hardening.md)
avant d'exposer le portail hors du labo; Elle Documente Le Threat Model,
профиль CSP/Trusted Types и этапы пен-тестирования, которые обеспечивают поддержку DOCS-1b.

## Примеры Norito-RPC

Запрашивает Norito-RPC, участвуя в прокси-сервере мема и сантехнике OAuth, которая соответствует маршрутам JSON;
все простое представление `Content-Type: application/x-norito` и передача полезной нагрузки Norito
предварительно закодировать декрет в соответствии со спецификацией NRPC
(`docs/source/torii/nrpc_spec.md`).
Le depot fournit des payloads canoniques sous `fixtures/norito_rpc/` для авторов, которые вы
Портал, владельцы SDK и рецензенты могут быть уверены в том, что байты точно используют номинал CI.

### Envoyer un payload Norito от консоли Попробуйте1. Выберите приспособление типа `fixtures/norito_rpc/transfer_asset.norito`. Цес
   fichiers sont des конверты Norito брют; **ne** не используется кодировка base64.
2. С помощью Swagger или RapiDoc локализация конечной точки NRPC (например,
   `POST /v2/pipeline/submit`) и выберите **Content-Type** на
   `application/x-norito`.
3. Откройте редактор тела в **двоичном формате** (режим «Файл» Swagger или
   выберите RapiDoc «Двоичный файл/файл» и зарядите файл `.norito`. Виджет
   передавать байты файлов через прокси-сервер без изменений.
4. Совершите запрос. Si Torii отправка `X-Iroha-Error-Code: schema_mismatch`,
   проверьте, что вы вызываете конечную точку, которая принимает бинарную полезную нагрузку и подтверждает
   que хэш схемы зарегистрируется в `fixtures/norito_rpc/schema_hashes.json`
   соответствует кабелю сборки Torii.

Консоль сохраняет записи плюс недавние воспоминания для того, чтобы вы могли быть отправлены
Полезная нагрузка мема рекламируется и тестируются различные токены авторизации или хосты Torii. Аджутер
`scripts/run_norito_rpc_fixtures.sh --note "<ticket>"` пакет продуктов для вашего рабочего процесса
предварительная ссылка на план внедрения NRPC-4 (журнал + резюме в формате JSON), это то, что вам нужно
с захватом экрана в ответ Попробуйте это по отзывам.

### Пример интерфейса командной строки (curl)

Les memes fixes peuvent etre rejoues hors du portail via `curl`, ce qui est utile
lorsque vous validez le proxy или разблокировать шлюз ответов:

```bash
TORII="https://torii.devnet.sora.example"
TOKEN="Bearer $(cat ~/.config/torii/devnet.token)"
curl   -H "Content-Type: application/x-norito"   -H "Authorization: ${TOKEN}"   --data-binary @fixtures/norito_rpc/transfer_asset.norito   "${TORII}/v2/pipeline/submit"
```

Замените приспособление, которое нужно импортировать, чтобы войти в список в `transaction_fixtures.manifest.json`
или закодируйте свою полезную нагрузку с помощью `cargo xtask norito-rpc-fixtures`. Quand Torii есть в
режим canary vous pouvez указатель `curl` vers le proxy try-it
(`https://docs.sora.example/proxy/v2/pipeline/submit`) для тренировки инфраструктуры мемов
какие виджеты порта используются.

## Наблюдаемость и операции

Chaque requete est loguee une fois avec Methode, path, origine, statut upstream et la source
аутентификация (`override`, `default` или `client`). Les tokens ne sont jamais stockes: les
Заголовки Bearer et les valeurs `X-TryIt-Auth` sont rediges avant log, afin que vous puissiez
Ретранслятор stdout и центральный сборщик без кранов.

### Проверка здоровья и оповещений

Включите пробник, включающий подвеску развертываний или расписание:

```bash
# Ensure the proxy responds to /healthz and forwards a sample request.
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" TRYIT_PROXY_SAMPLE_PATH="/v2/status" npm run probe:tryit-proxy
```

Ручки окружающей среды:

- `TRYIT_PROXY_SAMPLE_PATH` - маршрут Torii optionnelle (без `/proxy`) тренажер.
- `TRYIT_PROXY_SAMPLE_METHOD` - по умолчанию `GET`; определите `POST` для маршрутов написания.
- `TRYIT_PROXY_PROBE_TOKEN` — внедрить временный токен на предъявителя для примера.
- `TRYIT_PROXY_PROBE_TIMEOUT_MS` - сократить время ожидания по умолчанию на 5 с.
- `TRYIT_PROXY_PROBE_METRICS_FILE` — текст назначения Prometheus, опция для `probe_success`/`probe_duration_seconds`.
- `TRYIT_PROXY_PROBE_LABELS` - пары `key=value`, разделенные по virgules ajoutees aux meteriques (по умолчанию `job=tryit-proxy` и `instance=<proxy URL>`).
- `TRYIT_PROXY_PROBE_METRICS_URL` - URL-адрес конечной точки метрик (например, `http://localhost:9798/metrics`), который должен ответить на успешный `TRYIT_PROXY_METRICS_LISTEN`, который активен.Внедрение результатов в сборщик текстовых файлов и указатель на зонд для версии, доступной для записи
(например, `/var/lib/node_exporter/textfile_collector/tryit.prom`) и другие этикетки
персонализирует:

```bash
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" TRYIT_PROXY_PROBE_METRICS_FILE="/var/lib/node_exporter/textfile_collector/tryit.prom" TRYIT_PROXY_PROBE_LABELS="job=tryit-proxy,cluster=prod" npm run probe:tryit-proxy
```

Le script переписывает метрику атомного фабрики для вашего коллекционера Lise
toujours un полезная нагрузка завершена.

Quand `TRYIT_PROXY_METRICS_LISTEN` настроен, определен
`TRYIT_PROXY_PROBE_METRICS_URL` на конечной точке измерений для ускорения эхо-сигнала зонда
поверхность очистки нарушена (например, неправильная настройка входа или неправильное управление брандмауэром).
Типичный тип производства
`TRYIT_PROXY_PROBE_METRICS_URL="http://127.0.0.1:9798/metrics"`.

Для получения предупреждений подключите зонд к вашему стеку мониторинга. Пример Prometheus
Эта страница после двух последующих событий:

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

Definissez `TRYIT_PROXY_METRICS_LISTEN=127.0.0.1:9798` (или вся пара хост/порт) до
Lancer прокси-сервер для раскрытия конечной точки метрик в формате Prometheus. Ле Шемен
по умолчанию используется `/metrics`, но можно заменить параметр `TRYIT_PROXY_METRICS_PATH=/custom`. Чак
очистка отосланных счетчиков по всем методам, ограничение скорости повторных запросов, ошибки/таймауты
восходящий поток, прокси-сервер результатов и задержки резюме:

```bash
export TRYIT_PROXY_METRICS_LISTEN="127.0.0.1:9798"
npm run tryit-proxy &
curl http://127.0.0.1:9798/metrics | head -n 5
# HELP tryit_proxy_requests_total Requests handled by method
tryit_proxy_requests_total{method="GET"} 12
tryit_proxy_rate_limited_total 1
```

Pointez vos Collecteurs Prometheus/OTLP для конечных точек счетчиков и повторного использования панелей
существующие в `dashboards/grafana/docs_portal.json` до тех пор, пока SRE наблюдает за задержкой очереди
et les pics de rejet sans parser les logs. Автоматическая публикация прокси-сервера
`tryit_proxy_start_timestamp_ms` поможет операторам обнаружить красные пятна.

### Автоматизация отката

Используйте помощника для работы в течение дня или ресторатора по URL-адресу Torii. Ле сценарий
Предыдущая конфигурация в `.env.tryit-proxy.bak` будет зависеть от откатов
одна единственная команда.

```bash
# Update TRYIT_PROXY_TARGET and back up the previous config.
npm run manage:tryit-proxy -- update --target https://torii.devnet.sora.example

# Roll back to the previously backed-up target.
npm run manage:tryit-proxy -- rollback
```

Дополнительная плата за сбор документации с `--env` или `TRYIT_PROXY_ENV` при вашем развертывании
запас конфигурации.