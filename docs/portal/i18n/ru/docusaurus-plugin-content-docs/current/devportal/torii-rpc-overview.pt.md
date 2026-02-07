---
lang: ru
direction: ltr
source: docs/portal/docs/devportal/torii-rpc-overview.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Визовое письмо Norito-RPC

Norito-RPC и транспортировка двоичного файла для API Torii. Он повторно использует HTTP-запросы HTTP `/v1/pipeline`, большинство полезных данных которых задерживается для Norito, включая хэши схемы и контрольные суммы. Используйте, когда точные ответы детерминированы и валидированы или когда ответы JSON делают конвейер вирусным и гаргало.

## Что лучше?
- Детерминированное определение CRC64 и хэши схемы, уменьшающие ошибки декодирования.
- Помощники Norito, используемые в SDK, позволяют повторно использовать существующие типы данных для создания данных.
- Torii и марка сеансов Norito для телеметрии, которые позволяют операторам работать с панелями мониторинга.

## Fazendo uma requisicao

```bash
curl       -H 'Content-Type: application/x-norito'       -H 'Accept: application/x-norito'       -H "Authorization: Bearer ${TOKEN}"       --data-binary @signed_transaction.norito       https://torii.devnet.sora.example/v1/transactions/submit
```

1. Сериализуйте полезные данные с помощью кодека Norito (`iroha_client`, помощники для SDK или `norito::to_bytes`).
2. Завидуйте запросу `Content-Type: application/x-norito`.
3. Попросите ответ Norito, используя `Accept: application/x-norito`.
4. Расшифруйте ответ с помощью помощника корреспондента SDK.

Инструкция по SDK:
- **Rust**: `iroha_client::Client` negocia Norito автоматически при голосовом определении заголовка `Accept`.
- **Python**: используйте `NoritoRpcClient` от `iroha_python.norito_rpc`.
- **Android**: используйте `NoritoRpcClient` и `NoritoRpcRequestOptions` без SDK Android.
- **JavaScript/Swift**: наши помощники используются в `docs/source/torii/norito_rpc_tracker.md` и работают как часть NRPC-3.

## Пример консоли Попробуйте

Портал, предназначенный для разработки, включает в себя прокси-сервер. Попробуйте, чтобы можно было пересмотреть полезные нагрузки Norito, чтобы они могли хранить сценарии для medida.

1. [Начало прокси](./try-it.md#start-the-proxy-locally) и определение `TRYIT_PROXY_PUBLIC_URL` для того, чтобы виджеты были доступны для отправки или трафика.
2. Откройте карточку **Попробуйте**, откройте страницу или введите `/reference/torii-swagger` и выберите конечную точку в виде `POST /v1/pipeline/submit`.
3. Выберите **Content-Type** для `application/x-norito`, откройте редактор **Binary** и завидуйте `fixtures/norito_rpc/transfer_asset.norito` (или любой другой список полезной нагрузки в `fixtures/norito_rpc/transaction_fixtures.manifest.json`).
4. Используйте токен носителя с помощью виджета кода устройства OAuth или руководства пользователя (или прокси-сервера переопределяет `X-TryIt-Auth` при настройке с помощью `TRYIT_PROXY_ALLOW_CLIENT_AUTH=1`).
5. Запросите запрос и проверьте его на Torii или `schema_hash` в списке `fixtures/norito_rpc/schema_hashes.json`. Хэши подтверждают, что кабель Norito сохранен в виде навигатора/прокси.

Для подтверждения дорожной карты объедините снимок тела, сделанного с помощью Try It, с выполнением `scripts/run_norito_rpc_fixtures.sh --note "<ticket>"`. Сценарий включает `cargo xtask norito-rpc-verify`, записывает резюме в формате JSON в `artifacts/norito_rpc/<timestamp>/` и захватывает основные светильники, потребляемые на портале.

## Решение проблем| Синтома | Когда появляется | Причина проведания | Коррекао |
| --- | --- | --- | --- |
| `415 Unsupported Media Type` | Ответ на Torii | Заголовок `Content-Type` добавлен или не указан | Определено `Content-Type: application/x-norito` перед отправкой полезной нагрузки. |
| `X-Iroha-Error-Code: schema_mismatch` (HTTP 400) | Корпус/заголовки ответов на Torii | Хэш-схемы приборов различаются при сборке Torii | Перегенерировать светильники с помощью `cargo xtask norito-rpc-fixtures` и подтвердить или хешировать `fixtures/norito_rpc/schema_hashes.json`; используйте резервный JSON или конечную точку ainda nao habilitou Norito. |
| `{"error":"origin_forbidden"}` (HTTP 403) | Ответить через прокси Попробуйте | Требуется исходный список в списке `TRYIT_PROXY_ALLOWED_ORIGINS` | Добавьте исходный портал (например, `https://docs.devnet.sora.example`) с различными вариантами окружения и адресами или прокси. |
| `{"error":"rate_limited"}` (HTTP 429) | Ответить через прокси Попробуйте | Стоимость IP превышает бюджет `TRYIT_PROXY_RATE_LIMIT`/`TRYIT_PROXY_RATE_WINDOW_MS` | Увеличьте или ограничьте тесты внутреннего груза или дождитесь ответа на вопрос JSON (`retryAfterMs`). |
| `{"error":"upstream_timeout"}` (HTTP 504) или `{"error":"upstream_error"}` (HTTP 502) | Ответить через прокси Попробуйте | Torii истекает срок действия или прокси-сервер, который можно просмотреть или настроить серверную часть | Подтвердите, что `TRYIT_PROXY_TARGET` является доступным, подтвердите, что вы сделали Torii или что-то новое с `TRYIT_PROXY_TIMEOUT_MS` больше. |

Основная диагностика выполняется с помощью Try It и данных OAuth по адресу [`devportal/try-it.md`](./try-it.md#norito-rpc-samples).

## Дополнительные повторы
- Транспортный RFC: `docs/source/torii/norito_rpc.md`
- Исполнительное резюме: `docs/source/torii/norito_rpc_brief.md`.
- Трекер активности: `docs/source/torii/norito_rpc_tracker.md`.
- Инструкции по пробному прокси-серверу: `docs/portal/docs/devportal/try-it.md`