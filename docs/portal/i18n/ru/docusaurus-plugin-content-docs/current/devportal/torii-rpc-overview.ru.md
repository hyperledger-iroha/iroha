---
lang: ru
direction: ltr
source: docs/portal/docs/devportal/torii-rpc-overview.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Обзор Norito-RPC

Norito-RPC - бинарный транспорт для API Torii. Он использует тот же путь HTTP, что и `/v1/pipeline`, но возникает Norito-фреймингом с хешами-схемой и контрольной суммой. Используйте его, когда нужны определенные и проверенные ответы или когда конвейер ответов JSON становится узким представлением.

## Зачем переключаться?
- Определенное кадрирование с CRC64 и хеш-схемами имеет низкую ошибку декодирования.
- Общие Norito helper-ы между SDK позволяют переиспользовать различные типы моделей данных.
- Torii уже помечает Norito сессию в телеметрии, поэтому операторы могут вести учет через доступные дашборды.

## Отправка запроса

```bash
curl       -H 'Content-Type: application/x-norito'       -H 'Accept: application/x-norito'       -H "Authorization: Bearer ${TOKEN}"       --data-binary @signed_transaction.norito       https://torii.devnet.sora.example/v1/transactions/submit
```

1. Сериализуйте полезную нагрузку через кодек Norito (`iroha_client`, SDK helper-ы или `norito::to_bytes`).
2. Отправьте запрос с `Content-Type: application/x-norito`.
3. Запросите ответ Norito с `Accept: application/x-norito`.
4. Декодируйте ответ соответствующим SDK helper-ом.

Рекомендации по SDK:
- **Rust**: `iroha_client::Client` автоматически договаривается о Norito, когда задан заголовок `Accept`.
- **Python**: воспользуйтесь `NoritoRpcClient` из `iroha_python.norito_rpc`.
- **Android**: используйте `NoritoRpcClient` и `NoritoRpcRequestOptions` в Android SDK.
- **JavaScript/Swift**: helper-ы отслеживаются в `docs/source/torii/norito_rpc_tracker.md` и появляются в рамках NRPC-3.

## Пример консоли Попробуйте

Портал разработчика предоставляет прокси Try It, чтобы рецензенты могли воспроизвести полезные нагрузки Norito без написания отдельных скриптов.

1. [Запустите прокси](./try-it.md#start-the-proxy-locally) и задайте `TRYIT_PROXY_PUBLIC_URL`, чтобы виджеты знали, передать куда трафик.
2. Откройте карточку **Попробуйте** на этой странице или панели `/reference/torii-swagger` и выберите конечную точку, например `POST /v1/pipeline/submit`.
3. Переключите **Content-Type** на `application/x-norito`, выберите редактор **Binary** и загрузите `fixtures/norito_rpc/transfer_asset.norito` (или любую полезную нагрузку из `fixtures/norito_rpc/transaction_fixtures.manifest.json`).
4. Укажите токен носителя через виджет кода устройства OAuth или вручную (прокси принимает переопределение `X-TryIt-Auth`, если задан `TRYIT_PROXY_ALLOW_CLIENT_AUTH=1`).
5. Отправьте запрос и убедитесь, что Torii возвращает `schema_hash`, указанный в `fixtures/norito_rpc/schema_hashes.json`. Совпадение хешей подтвердило, что заголовок Norito пережил прыжок браузера/прокси.

Для доказательства дорожной карты примените скриншот Попробуйте это с запуском `scripts/run_norito_rpc_fixtures.sh --note "<ticket>"`. Скрипт оборачивает `cargo xtask norito-rpc-verify`, записывает JSON-сводку в `artifacts/norito_rpc/<timestamp>/` и захватывает те же самые приборы, которые использовал портал.

## Устранение неполадок| Симптом | Где обнаружено | Вероятная причина | Исправление |
| --- | --- | --- | --- |
| `415 Unsupported Media Type` | Ответ Torii | Отсутствует или неверный заголовок `Content-Type` | Установите `Content-Type: application/x-norito` перед отправкой полезной нагрузки. |
| `X-Iroha-Error-Code: schema_mismatch` (HTTP 400) | Тело/заголовки ответа Torii | Хеш схемы светильников не соответствуют сборке Torii | Пересоздайте светильники через `cargo xtask norito-rpc-fixtures` и проверьте хеш в `fixtures/norito_rpc/schema_hashes.json`; Резервный JSON, если конечная точка еще не включает Norito. |
| `{"error":"origin_forbidden"}` (HTTP 403) | Ответ Попробуйте прокси | Запрос пришел с Origin, которого нет в `TRYIT_PROXY_ALLOWED_ORIGINS` | Добавьте исходный портал (например, `https://docs.devnet.sora.example`) в переменное окружение и перезапустите прокси. |
| `{"error":"rate_limited"}` (HTTP 429) | Ответ Попробуйте прокси | Квота на IP превысила бюджет `TRYIT_PROXY_RATE_LIMIT`/`TRYIT_PROXY_RATE_WINDOW_MS` | Увеличьте лимит для внутренних нагрузочных тестов или сбросьте окно (просмотрите `retryAfterMs` в ответе JSON). |
| `{"error":"upstream_timeout"}` (HTTP 504) или `{"error":"upstream_error"}` (HTTP 502) | Ответ Попробуйте прокси | Torii тайм-аут или прокси не смог достучаться до бэкенда | Проверьте доступность `TRYIT_PROXY_TARGET`, состояние Torii или повторите с большим `TRYIT_PROXY_TIMEOUT_MS`. |

Больше диагностики Попробуйте и советы по настройке OAuth в [`devportal/try-it.md`](./try-it.md#norito-rpc-samples).

## Дополнительные ресурсы
- Транспорт RFC: `docs/source/torii/norito_rpc.md`
- Исполнительное резюме: `docs/source/torii/norito_rpc_brief.md`
- Трекер задачи: `docs/source/torii/norito_rpc_tracker.md`
- Инструкции Try-It прокси: `docs/portal/docs/devportal/try-it.md`