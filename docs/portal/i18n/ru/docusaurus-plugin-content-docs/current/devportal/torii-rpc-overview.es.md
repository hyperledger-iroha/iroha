---
lang: ru
direction: ltr
source: docs/portal/docs/devportal/torii-rpc-overview.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Резюме Norito-RPC

Norito-RPC — это бинарный транспортный файл для API Torii. Повторно используйте неверные руты HTTP, которые `/v2/pipeline`, но вставляются в пакеты Norito, которые включают хэши задач и контрольные суммы. Если вам необходимы детерминированные и проверенные ответы или когда ответы JSON из конвейера попадают в буфер обмена.

## Что можно сделать?
- Определенная маркировка с CRC64 и хеш-значения уменьшают количество ошибок декодирования.
- Помощники Norito, связанные с SDK, позволяют повторно использовать существующие типы данных.
- Torii соответствует этикету сеансов Norito в телеметрии, так как операторы могут отслеживать принятие решений с помощью панелей provistos.

## Как просили

```bash
curl       -H 'Content-Type: application/x-norito'       -H 'Accept: application/x-norito'       -H "Authorization: Bearer ${TOKEN}"       --data-binary @signed_transaction.norito       https://torii.devnet.sora.example/v2/transactions/submit
```

1. Сериализация полезных данных с кодеком Norito (`iroha_client`, помощники SDK или `norito::to_bytes`).
2. Отправка запроса с `Content-Type: application/x-norito`.
3. Попросите ответ Norito, используя `Accept: application/x-norito`.
4. Расшифруйте ответ с помощником корреспондента SDK.

Инструкция по SDK:
- **Rust**: `iroha_client::Client` negocia Norito автоматически, когда устанавливается заголовок `Accept`.
- **Python**: США `NoritoRpcClient` от `iroha_python.norito_rpc`.
- **Android**: США `NoritoRpcClient` и `NoritoRpcRequestOptions` в SDK для Android.
- **JavaScript/Swift**: помощники будут загружены в `docs/source/torii/norito_rpc_tracker.md` и станут частью NRPC-3.

## Консольный пример Попробуйте

На портале desarrolladores есть прокси-сервер Try It, который позволяет воспроизводить полезные нагрузки Norito без написания сценариев на медиуме.

1. [Начало прокси](./try-it.md#start-the-proxy-locally) и определите `TRYIT_PROXY_PUBLIC_URL`, чтобы виджеты были разделены и отправлены по трафику.
2. Нажмите кнопку **Попробовать** на этой странице или панели `/reference/torii-swagger` и выберите конечную точку как `POST /v2/pipeline/submit`.
3. Установите **Content-Type** в `application/x-norito`, выберите редактор **Binary** и субте `fixtures/norito_rpc/transfer_asset.norito` (или более широкий список полезной нагрузки в `fixtures/norito_rpc/transaction_fixtures.manifest.json`).
4. Пропорция токена-носителя с помощью кода устройства OAuth виджета или руководства по использованию токена (принятие прокси-сервера отменяет `X-TryIt-Auth`, если он настроен с помощью `TRYIT_PROXY_ALLOW_CLIENT_AUTH=1`).
5. Отправьте запрос и проверьте, что Torii отражает список `schema_hash` в `fixtures/norito_rpc/schema_hashes.json`. Совпадающие хэши подтверждают, что инкапсуляция Norito позволяет сохранить навигатор/прокси.

Для подтверждения дорожной карты объедините захват панели Try It с выбросом `scripts/run_norito_rpc_fixtures.sh --note "<ticket>"`. Сценарий окружает `cargo xtask norito-rpc-verify`, записывает резюме в формате JSON в `artifacts/norito_rpc/<timestamp>/` и захватывает разные приспособления, которые используются на портале.

## Решение проблем| Синтома | Донде апаресе | Причина вероятна | Решение |
| --- | --- | --- | --- |
| `415 Unsupported Media Type` | Ответ Torii | Ошибка или неправильный заголовок `Content-Type` | Определите `Content-Type: application/x-norito` перед отправкой полезной нагрузки. |
| `X-Iroha-Error-Code: schema_mismatch` (HTTP 400) | Сообщение/заголовки ответа Torii | Хэш таблицы приборов, отличающихся от сборки Torii | Регенерация приборов с `cargo xtask norito-rpc-fixtures` и подтверждением хэша в `fixtures/norito_rpc/schema_hashes.json`; Резервный JSON для США, если конечная точка не имеет навыков Norito. |
| `{"error":"origin_forbidden"}` (HTTP 403) | Ответ на прокси-сервер Попробуйте | Запрос о происхождении, который не указан в списке `TRYIT_PROXY_ALLOWED_ORIGINS` | Объединение исходного портала (например, `https://docs.devnet.sora.example`) с переменной отправителя и адресом прокси. |
| `{"error":"rate_limited"}` (HTTP 429) | Ответ на прокси-сервер Попробуйте | Значение IP превышает предположение `TRYIT_PROXY_RATE_LIMIT`/`TRYIT_PROXY_RATE_WINDOW_MS` | Увеличьте лимит, чтобы увеличить внутренние расходы на груз или дождаться, пока вы выйдете из строя (см. `retryAfterMs` в ответе JSON). |
| `{"error":"upstream_timeout"}` (HTTP 504) или `{"error":"upstream_error"}` (HTTP 502) | Ответ на прокси-сервер Попробуйте | Torii назад на время или на прокси-сервер, не могу выбрать конфигурацию серверной части | Подтвердите, что `TRYIT_PROXY_TARGET` доступен к морю, проверьте привет Torii или повторите попытку с мэром `TRYIT_PROXY_TIMEOUT_MS`. |

Диагностика Try It и советы по OAuth, действующие в [`devportal/try-it.md`](./try-it.md#norito-rpc-samples).

## Дополнительные повторы
- Транспортный RFC: `docs/source/torii/norito_rpc.md`
- Электронное резюме: `docs/source/torii/norito_rpc_brief.md`.
- Трекер действий: `docs/source/torii/norito_rpc_tracker.md`
- Инструкции по пробному прокси-серверу: `docs/portal/docs/devportal/try-it.md`