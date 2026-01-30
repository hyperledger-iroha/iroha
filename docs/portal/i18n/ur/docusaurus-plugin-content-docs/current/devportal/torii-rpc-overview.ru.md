---
lang: ur
direction: rtl
source: docs/portal/docs/devportal/torii-rpc-overview.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# Обзор Norito-RPC

Norito-RPC - бинарный транспорт для API Torii. Он использует те же HTTP пути, что и `/v1/pipeline`, но обменивается Norito-фреймингом с хешами схемы и checksum. Используйте его, когда нужны детерминированные и проверенные ответы или когда JSON ответы pipeline становятся узким местом.

## Зачем переключаться?
- Детерминированное фреймирование с CRC64 и хешами схемы снижает ошибки декодирования.
- Общие Norito helper-ы между SDK позволяют переиспользовать существующие типы модели данных.
- Torii уже помечает Norito сессии в телеметрии, поэтому операторы могут отслеживать принятие через доступные дашборды.

## Отправка запроса

```bash
curl       -H 'Content-Type: application/x-norito'       -H 'Accept: application/x-norito'       -H "Authorization: Bearer ${TOKEN}"       --data-binary @signed_transaction.norito       https://torii.devnet.sora.example/v1/transactions/submit
```

1. Сериализуйте payload через Norito codec (`iroha_client`, SDK helper-ы или `norito::to_bytes`).
2. Отправьте запрос с `Content-Type: application/x-norito`.
3. Запросите ответ Norito с `Accept: application/x-norito`.
4. Декодируйте ответ соответствующим SDK helper-ом.

Рекомендации по SDK:
- **Rust**: `iroha_client::Client` автоматически договаривается о Norito, когда задан заголовок `Accept`.
- **Python**: используйте `NoritoRpcClient` из `iroha_python.norito_rpc`.
- **Android**: используйте `NoritoRpcClient` и `NoritoRpcRequestOptions` в Android SDK.
- **JavaScript/Swift**: helper-ы отслеживаются в `docs/source/torii/norito_rpc_tracker.md` и появятся в рамках NRPC-3.

## Пример консоли Try It

Портал разработчика поставляет Try It прокси, чтобы рецензенты могли воспроизводить Norito payload-ы без написания отдельных скриптов.

1. [Запустите прокси](./try-it.md#start-the-proxy-locally) и задайте `TRYIT_PROXY_PUBLIC_URL`, чтобы виджеты знали, куда отправлять трафик.
2. Откройте карточку **Try it** на этой странице или панель `/reference/torii-swagger` и выберите endpoint, например `POST /v1/pipeline/submit`.
3. Переключите **Content-Type** на `application/x-norito`, выберите редактор **Binary** и загрузите `fixtures/norito_rpc/transfer_asset.norito` (или любой payload из `fixtures/norito_rpc/transaction_fixtures.manifest.json`).
4. Укажите bearer token через OAuth device-code widget или вручную (прокси принимает overrides `X-TryIt-Auth`, когда задан `TRYIT_PROXY_ALLOW_CLIENT_AUTH=1`).
5. Отправьте запрос и убедитесь, что Torii возвращает `schema_hash`, указанный в `fixtures/norito_rpc/schema_hashes.json`. Совпадение хешей подтверждает, что заголовок Norito пережил прыжок браузер/прокси.

Для доказательств roadmap совместите скриншот Try It с запуском `scripts/run_norito_rpc_fixtures.sh --note "<ticket>"`. Скрипт оборачивает `cargo xtask norito-rpc-verify`, записывает JSON сводку в `artifacts/norito_rpc/<timestamp>/` и захватывает те же fixtures, которые использовал портал.

## Устранение неполадок

| Симптом | Где проявляется | Вероятная причина | Исправление |
| --- | --- | --- | --- |
| `415 Unsupported Media Type` | Ответ Torii | Отсутствует или неверный заголовок `Content-Type` | Установите `Content-Type: application/x-norito` перед отправкой payload. |
| `X-Iroha-Error-Code: schema_mismatch` (HTTP 400) | Тело/заголовки ответа Torii | Хеш схемы fixtures не совпадает со сборкой Torii | Пересоздайте fixtures через `cargo xtask norito-rpc-fixtures` и проверьте хеш в `fixtures/norito_rpc/schema_hashes.json`; используйте JSON fallback, если endpoint еще не включает Norito. |
| `{"error":"origin_forbidden"}` (HTTP 403) | Ответ Try It прокси | Запрос пришел с origin, которого нет в `TRYIT_PROXY_ALLOWED_ORIGINS` | Добавьте origin портала (например, `https://docs.devnet.sora.example`) в переменную окружения и перезапустите прокси. |
| `{"error":"rate_limited"}` (HTTP 429) | Ответ Try It прокси | Квота на IP превысила бюджет `TRYIT_PROXY_RATE_LIMIT`/`TRYIT_PROXY_RATE_WINDOW_MS` | Увеличьте лимит для внутренних нагрузочных тестов или дождитесь сброса окна (смотрите `retryAfterMs` в JSON ответе). |
| `{"error":"upstream_timeout"}` (HTTP 504) или `{"error":"upstream_error"}` (HTTP 502) | Ответ Try It прокси | Torii тайм-аут или прокси не смог достучаться до backend | Проверьте доступность `TRYIT_PROXY_TARGET`, состояние Torii или повторите с большим `TRYIT_PROXY_TIMEOUT_MS`. |

Больше диагностики Try It и советы по OAuth находятся в [`devportal/try-it.md`](./try-it.md#norito-rpc-samples).

## Дополнительные ресурсы
- RFC транспорта: `docs/source/torii/norito_rpc.md`
- Исполнительное резюме: `docs/source/torii/norito_rpc_brief.md`
- Трекер задач: `docs/source/torii/norito_rpc_tracker.md`
- Инструкции Try-It прокси: `docs/portal/docs/devportal/try-it.md`
