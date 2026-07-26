---
lang: ru
direction: ltr
source: docs/portal/docs/devportal/torii-rpc-overview.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Norito-RPC.

Norito-RPC Torii API-интерфейсы `/v1/pipeline` и HTTP-интерфейс, а также Norito и полезные нагрузки. Использование хешей схемы и контрольных сумм. Если вы хотите, чтобы это произошло, вы можете сделать это в любое время. Использование конвейера для JSON-файлов и файлов для чтения

## کیوں تبدیل کریں؟
- CRC64 хеш-схемы для создания фреймов или для создания фреймов
- SDK позволяет использовать помощники Norito для создания модели данных и создания модели данных. دیتی ہیں۔
- Torii پہلے ہی ٹیلی میٹری میں Norito سیشنز کو ٹَیگ کرتا ہے، اس لئے Используйте информационные панели, чтобы получить доступ к информационным панелям

## درخواست بھیجنا

```bash
curl       -H 'Content-Type: application/x-norito'       -H 'Accept: application/x-norito'       -H "Authorization: Bearer ${TOKEN}"       --data-binary @signed_transaction.norito       https://torii.devnet.sora.example/v1/transactions/submit
```

1. Полезная нагрузка: кодек Norito (`iroha_client`, SDK helpers, `norito::to_bytes`).
2. درخواست `Content-Type: application/x-norito` کے ساتھ بھیجیں۔
3. `Accept: application/x-norito` کے ذریعے Norito ответ مانگیں۔
4. Помощник SDK для ответа на запрос

Возможности SDK:
- **Rust**: `iroha_client::Client` جب آپ `Accept` header سیٹ کرتے ہیں تو Norito خودکار طور پر negotiation. کرتا ہے۔
- **Python**: `iroha_python.norito_rpc` سے `NoritoRpcClient` استعمال کریں۔
- **Android**: Android SDK в формате `NoritoRpcClient` или `NoritoRpcRequestOptions`.
- **JavaScript/Swift**: помощники, например, `docs/source/torii/norito_rpc_tracker.md`, могут быть использованы для работы с NRPC-3, а также для работы с ними. آئیں گے۔

## Попробуй, пожалуйста

Попробуйте, пожалуйста, попробуйте это. Полезные нагрузки Norito, которые можно использовать

1. [Для создания виджетов](./try-it.md#start-the-proxy-locally) Для `TRYIT_PROXY_PUBLIC_URL` выберите нужные виджеты. ٹریفک کہاں بھیجنی ہے۔
2. Нажмите **Попробуйте** Установите `/reference/torii-swagger` и выберите `POST /v1/pipeline/submit` для конечной точки. کریں۔
3. **Content-Type** – `application/x-norito` или **Binary** – значение `fixtures/norito_rpc/transfer_asset.norito`. Установите флажок (یا `fixtures/norito_rpc/transaction_fixtures.manifest.json` для получения полезной нагрузки)۔
4. Виджет кода устройства OAuth с возможностью использования токена-носителя (`TRYIT_PROXY_ALLOW_CLIENT_AUTH=1`). تو پراکسی `X-TryIt-Auth` переопределяет قبول کرتا ہے)۔
5. Установите флажок Torii `fixtures/norito_rpc/schema_hashes.json` или установите `schema_hash`. واپس دیتا ہے۔ Получение хэшей и данных заголовка Norito براؤزر/پراکسی hop سے محفوظ رہا۔

Если вы хотите попробовать это, попробуйте `scripts/run_norito_rpc_fixtures.sh --note "<ticket>"`, чтобы узнать, как это сделать. Загрузите файл `cargo xtask norito-rpc-verify` для создания файла JSON `artifacts/norito_rpc/<timestamp>/` для создания файла JSON. светильники

## مسئلہ حل کرنا| علامت | کہاں نظر آتا ہے | ممکنہ وجہ | حل |
| --- | --- | --- | --- |
| `415 Unsupported Media Type` | Torii ответ | `Content-Type` заголовок غائب یا غلط | полезная нагрузка بھیجنے سے پہلے `Content-Type: application/x-norito` سیٹ کریں۔ |
| `X-Iroha-Error-Code: schema_mismatch` (HTTP 400) | Torii Тело/заголовки ответа | светильники کا хэш схемы Torii build سے مختلف ہے | `cargo xtask norito-rpc-fixtures` - светильники, необходимые для `fixtures/norito_rpc/schema_hashes.json` - хэш-функции. Конечная точка имеет Norito и резервный вариант JSON. |
| `{"error":"origin_forbidden"}` (HTTP 403) | Попробуйте ответ прокси | درخواست ایسے origin سے آئی جو `TRYIT_PROXY_ALLOWED_ORIGINS` میں درج نہیں | Источник происхождения (`https://docs.devnet.sora.example`) env var, который может быть использован в качестве источника данных. |
| `{"error":"rate_limited"}` (HTTP 429) | Попробуйте ответ прокси | IP-адрес `TRYIT_PROXY_RATE_LIMIT`/`TRYIT_PROXY_RATE_WINDOW_MS` Если вы хотите загрузить файл, выберите ответ в формате JSON (ответ JSON). میں `retryAfterMs` دیکھیں)۔ |
| `{"error":"upstream_timeout"}` (HTTP 504) или `{"error":"upstream_error"}` (HTTP 502) | Попробуйте ответ прокси | Torii ٹائم آؤٹ ہوا یا پراکسی настроенный серверный интерфейс, который позволяет настроить | `TRYIT_PROXY_TARGET`, как установить Torii, как `TRYIT_PROXY_TIMEOUT_MS`. رکھ کر دوبارہ کوشش کریں۔ |

Попробуйте диагностику Try It и советы по OAuth [`devportal/try-it.md`](./try-it.md#norito-rpc-samples).

## اضافی وسائل
- Транспортный RFC: `docs/source/torii/norito_rpc.md`
- Краткое описание: `docs/source/torii/norito_rpc_brief.md`
- Трекер действий: `docs/source/torii/norito_rpc_tracker.md`
- Инструкции по прокси-серверу Try-It: `docs/portal/docs/devportal/try-it.md`