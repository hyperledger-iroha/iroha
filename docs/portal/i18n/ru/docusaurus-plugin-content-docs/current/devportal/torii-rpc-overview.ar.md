---
lang: ru
direction: ltr
source: docs/portal/docs/devportal/torii-rpc-overview.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Введите код Norito-RPC.

Norito-RPC в локальной сети Torii. Получение HTTP-запроса `/v1/pipeline` в режиме онлайн-загрузки. Norito хэширует данные и контрольные суммы. Он сказал, что в 1990-х годах он был рожден в 1980-х годах. Формат JSON для конвейера.

## لماذا ننتقل؟
- Вы можете использовать CRC64 и хэши для проверки и проверки.
- Добавление Norito для создания SDK для последующего использования. الحالية.
- Torii يوسم جلسات Norito بالفعل في القياس عن عدد, لذا يمكن للمشغلين Нажмите на ссылку, чтобы получить больше информации.

## تنفيذ طلب

```bash
curl       -H 'Content-Type: application/x-norito'       -H 'Accept: application/x-norito'       -H "Authorization: Bearer ${TOKEN}"       --data-binary @signed_transaction.norito       https://torii.devnet.sora.example/v1/transactions/submit
```

1. Используется встроенный кодек Norito (`iroha_client`, встроенный SDK или `norito::to_bytes`).
2. Установите флажок `Content-Type: application/x-norito`.
3. Установите Norito и установите `Accept: application/x-norito`.
4. Установите флажок для создания SDK-файла.

Доступ к SDK:
- **Rust**: `iroha_client::Client` или Norito для установки `Accept`.
- **Python**: `NoritoRpcClient` вместо `iroha_python.norito_rpc`.
- **Android**: используйте `NoritoRpcClient` и `NoritoRpcRequestOptions` в Android SDK.
- **JavaScript/Swift**: настройка выполняется в `docs/source/torii/norito_rpc_tracker.md` в соответствии с NRPC-3.

## مثال وحدة Попробуйте

В фильме «Попробуй это» в фильме «Попробуй это» в фильме «Старый мир» Norito был отправлен на сервер.

1. [Закрыть](./try-it.md#start-the-proxy-locally) и `TRYIT_PROXY_PUBLIC_URL`, чтобы установить его в исходное состояние. حركة.
2. Нажмите **Попробуйте** в разделе `/reference/torii-swagger` и установите флажок `POST /v1/pipeline/submit`.
3. Выберите **Content-Type** для `application/x-norito`, выберите **Binary** и `fixtures/norito_rpc/transfer_asset.norito` (если хотите, выберите **Binary**). في `fixtures/norito_rpc/transaction_fixtures.manifest.json`).
4. Для токена-носителя используется код устройства OAuth в соответствии с кодом устройства `X-TryIt-Auth`. ضبط `TRYIT_PROXY_ALLOW_CLIENT_AUTH=1`).
5. Установите флажок Torii для `schema_hash` для `fixtures/norito_rpc/schema_hashes.json`. Он был создан в راس Norito, когда был установлен/завершен.

Откройте приложение Try It на странице `scripts/run_norito_rpc_fixtures.sh --note "<ticket>"`. Встроенные файлы `cargo xtask norito-rpc-verify`, файлы JSON `artifacts/norito_rpc/<timestamp>/`, встроенные светильники التي استخدمها البوابة.

## استكشاف الاخطاء| عرض | مكان الظهور | السبب المحتمل | حل |
| --- | --- | --- | --- |
| `415 Unsupported Media Type` | Сообщение Torii | ترويسة `Content-Type` مفقودة او غير صحيحة | Это `Content-Type: application/x-norito` в случае необходимости. |
| `X-Iroha-Error-Code: schema_mismatch` (HTTP 400) | Информационный/информационный центр Torii | хеш-фиксаторы مخطط يختلف عن build Torii | Зарегистрированные светильники: `cargo xtask norito-rpc-fixtures` и хеш-код `fixtures/norito_rpc/schema_hashes.json`; Резервный JSON для создания резервного файла Norito. |
| `{"error":"origin_forbidden"}` (HTTP 403) | استجابة وكيل Попробуйте | جاء الطلب من اصل غير مدرج في `TRYIT_PROXY_ALLOWED_ORIGINS` | Он был установлен (номер `https://docs.devnet.sora.example`) и был установлен на место. |
| `{"error":"rate_limited"}` (HTTP 429) | استجابة وكيل Попробуйте | Доступ к IP-адресу `TRYIT_PROXY_RATE_LIMIT`/`TRYIT_PROXY_RATE_WINDOW_MS` | زد الحد لاختبارات الحمل الداخلية او انتظر اعادة ضبط النافذة (راجع `retryAfterMs` في) Формат JSON). |
| `{"error":"upstream_timeout"}` (HTTP 504) и `{"error":"upstream_error"}` (HTTP 502) | استجابة وكيل Попробуйте | Torii Добавление новых изображений в раздел "Ведомости" | Он был создан для `TRYIT_PROXY_TARGET`, а также для Torii, а также для `TRYIT_PROXY_TIMEOUT_MS`. اكبر. |

Попробуйте использовать OAuth для [`devportal/try-it.md`](./try-it.md#norito-rpc-samples).

## موارد اضافية
- RFC-код: `docs/source/torii/norito_rpc.md`.
- Дополнительная информация: `docs/source/torii/norito_rpc_brief.md`.
- Имя пользователя: `docs/source/torii/norito_rpc_tracker.md`.
- Попробуйте попробовать: `docs/portal/docs/devportal/try-it.md`