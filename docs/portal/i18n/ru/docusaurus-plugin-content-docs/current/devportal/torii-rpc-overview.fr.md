---
lang: ru
direction: ltr
source: docs/portal/docs/devportal/torii-rpc-overview.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Сообщение Norito-RPC

Norito-RPC представляет собой двоичный транспортный файл для API Torii. Я повторно использую мемы, полученные по HTTP, которые `/v2/pipeline` могут быть заменены кодами зарядов по Norito, которые включают хэши схемы и контрольные суммы. Используйте поиск ответов, детерминированных и достоверных ответов, или вызов ответов JSON для конвейера, созданного для запутывания.

## Ченджер Пуркуа?
- Определенное размещение с CRC64 и хэшами схемы устраняет ошибки декодирования.
- Помощники Norito частично используются в SDK и позволяют повторно использовать типы существующих моделей.
- Torii отмечает сеансы Norito в телеметрии, позволяя операторам использовать новые панели мониторинга.

## Добро пожаловать по запросу

```bash
curl       -H 'Content-Type: application/x-norito'       -H 'Accept: application/x-norito'       -H "Authorization: Bearer ${TOKEN}"       --data-binary @signed_transaction.norito       https://torii.devnet.sora.example/v2/transactions/submit
```

1. Сериализация вашей полезной нагрузки с кодеком Norito (`iroha_client`, вспомогательный SDK или `norito::to_bytes`).
2. Отправьте запрос с `Content-Type: application/x-norito`.
3. Потребуйте ответ Norito через `Accept: application/x-norito`.
4. Раскодируйте ответ с помощью помощника-корреспондента SDK.

Советы по SDK:
- **Rust**: `iroha_client::Client` negocie Norito автоматизация, когда вы определитесь с глазу на глаз `Accept`.
- **Python**: используйте `NoritoRpcClient` от `iroha_python.norito_rpc`.
- **Android**: используйте `NoritoRpcClient` и `NoritoRpcRequestOptions` в SDK Android.
- **JavaScript/Swift**: помощники работают в `docs/source/torii/norito_rpc_tracker.md` и приходят в NRPC-3.

## Пример консоли Попробуйте

Портал разработки работает с прокси-сервером. Попробуйте, чтобы отражатели могли повторно использовать полезные нагрузки Norito без написания сценариев.

1. [Удалить прокси](./try-it.md#start-the-proxy-locally) и определить `TRYIT_PROXY_PUBLIC_URL` для того, чтобы виджеты были сохранены или отправляли трафик.
2. Откройте меню **Попробуйте** на этой странице или на панели `/reference/torii-swagger` и выберите конечную точку, соответствующую `POST /v2/pipeline/submit`.
3. Введите **Content-Type** в `application/x-norito`, выберите редактор **Binary** и зарядите `fixtures/norito_rpc/transfer_asset.norito` (или просмотрите список полезной нагрузки в `fixtures/norito_rpc/transaction_fixtures.manifest.json`).
4. Подключите токен носителя через код устройства OAuth виджета или вручную (прокси-сервер принимает переопределения `X-TryIt-Auth`, чтобы настроить его с помощью `TRYIT_PROXY_ALLOW_CLIENT_AUTH=1`).
5. Отправьте запрос и проверьте, что Torii отправил `schema_hash` в список `fixtures/norito_rpc/schema_hashes.json`. Идентификаторы хэшей подтверждают, что Norito находится наедине с системой наблюдения или прокси-сервером.

Для получения дорожной карты доказательств попробуйте захват экрана. Попробуйте выполнить `scripts/run_norito_rpc_fixtures.sh --note "<ticket>"`. Скрипт инкапсулирует `cargo xtask norito-rpc-verify`, записывает резюме в формате JSON в `artifacts/norito_rpc/<timestamp>/` и захватывает мемы-фиксаторы, которые переносятся в завершение.

## Депаннаж| Симптом | Ваш аппарат | Причина вероятная | Корректиф |
| --- | --- | --- | --- |
| `415 Unsupported Media Type` | Ответ Torii | En-tete `Content-Type` ошибка или ошибка | Definissez `Content-Type: application/x-norito` перед отправкой полезной нагрузки. |
| `X-Iroha-Error-Code: schema_mismatch` (HTTP 400) | Корпус/ответный ответ Torii | Хэш схемы приборов, отличающихся от сборки Torii | Перегенерировать светильники с `cargo xtask norito-rpc-fixtures` и подтвердить хэш в `fixtures/norito_rpc/schema_hashes.json`; Повторите проход в JSON, если конечная точка не активна на бис Norito. |
| `{"error":"origin_forbidden"}` (HTTP 403) | Ответ прокси-сервера Попробуйте | Просьба предоставить источник, не включенный в список в `TRYIT_PROXY_ALLOWED_ORIGINS` | Добавьте источник порта (например, `https://docs.devnet.sora.example`) в переменную среды и восстановите прокси. |
| `{"error":"rate_limited"}` (HTTP 429) | Ответ прокси-сервера Попробуйте | Квота по IP-адресу в обход бюджета `TRYIT_PROXY_RATE_LIMIT`/`TRYIT_PROXY_RATE_WINDOW_MS` | Увеличение лимита для тестов внутренней зарядки или участия в повторной инициализации окна (вид `retryAfterMs` в ответе JSON). |
| `{"error":"upstream_timeout"}` (HTTP 504) или `{"error":"upstream_error"}` (HTTP 502) | Ответ прокси-сервера Попробуйте | Torii истекает срок действия или прокси-сервер, и вы не можете выполнить настройку серверной части | Убедитесь, что `TRYIT_PROXY_TARGET` доступен, управляйте входом Torii или проверьте `TRYIT_PROXY_TIMEOUT_MS` плюс одиннадцать. |

Плюс диагностика Try It и советы по OAuth в [`devportal/try-it.md`](./try-it.md#norito-rpc-samples).

## Дополнительные ресурсы
- Транспорт RFC: `docs/source/torii/norito_rpc.md`
- Возобновить выполнение: `docs/source/torii/norito_rpc_brief.md`
- Трекер действий: `docs/source/torii/norito_rpc_tracker.md`
- Инструкции по пробному использованию прокси: `docs/portal/docs/devportal/try-it.md`