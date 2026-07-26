---
lang: es
direction: ltr
source: docs/portal/docs/devportal/torii-rpc-overview.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Norito-RPC کا جائزہ

Norito-RPC Torii API کے لئے بائنری ٹرانسپورٹ ہے۔ یہ `/v1/pipeline` y HTTP راستے استعمال کرتا ہے لیکن Norito فریم y الے payloads کا تبادلہ کرتا ہے جن میں esquema hashes اور checksums شامل ہوتے ہیں۔ اسے وہاں استعمال کریں جہاں آپ کو متعین اور تصدیق شدہ جوابات درکار ہوں، یا جب pipeline کے JSON جوابات بوتل نیک بن جائیں۔

## کیوں تبدیل کریں؟
- CRC64 اور hashes de esquema کے ساتھ متعین framing ڈیکوڈنگ غلطیوں کو کم کرتی ہے۔
- SDKs کے درمیان مشترکہ Norito helpers موجودہ data-model اقسام کو دوبارہ استعمال کرنے دیتی ہیں۔
- Torii پہلے ہی ٹیلی میٹری میں Norito سیشنز کو ٹَیگ کرتا ہے، اس لئے آپریٹرز فراہم کردہ paneles de control کے ذریعے اپنانے کی نگرانی کر سکتے ہیں۔

## درخواست بھیجنا

```bash
curl       -H 'Content-Type: application/x-norito'       -H 'Accept: application/x-norito'       -H "Authorization: Bearer ${TOKEN}"       --data-binary @signed_transaction.norito       https://torii.devnet.sora.example/v1/transactions/submit
```

1. Carga útil del códec Norito (`iroha_client`, ayudantes del SDK, o `norito::to_bytes`) y del códec
2. درخواست `Content-Type: application/x-norito` کے ساتھ بھیجیں۔
3. `Accept: application/x-norito` کے ذریعے Norito respuesta مانگیں۔
4. متعلقہ SDK helper کے ساتھ respuesta ڈی کوڈ کریں۔SDK کے لحاظ سے رہنمائی:
- **Rust**: `iroha_client::Client` en el encabezado `Accept` en el encabezado y en el Norito en el negociador
- **Python**: `iroha_python.norito_rpc` سے `NoritoRpcClient` استعمال کریں۔
- **Android**: SDK de Android con software `NoritoRpcClient` y `NoritoRpcRequestOptions`.
- **JavaScript/Swift**: ayudantes کو `docs/source/torii/norito_rpc_tracker.md` میں ٹریک کیا جاتا ہے اور یہ NRPC-3 کے حصے کے طور پر آئیں گے۔

## Pruébalo کنسول کی مثال

ڈویلپر پورٹل ایک Pruébelo پراکسی فراہم کرتا ہے تاکہ ریویورز بغیر کسی مخصوص اسکرپٹ کے Cargas útiles Norito کو دوبارہ چلا سکیں۔

1. [پراکسی شروع کریں](./try-it.md#start-the-proxy-locally) اور `TRYIT_PROXY_PUBLIC_URL` سیٹ کریں تاکہ widgets جان سکیں کہ ٹریفک کہاں بھیجنی ہے۔
2. Haga clic en **Pruébelo** کارڈ یا `/reference/torii-swagger` پینل کھولیں اور `POST /v1/pipeline/submit` جیسا منتخب کریں۔
3. **Tipo de contenido** کو `application/x-norito` پر سوئچ کریں، **Binario** ایڈیٹر منتخب کریں، اور `fixtures/norito_rpc/transfer_asset.norito` اپ لوڈ کریں (یا `fixtures/norito_rpc/transaction_fixtures.manifest.json` میں درج کوئی بھی carga útil)۔
4. Widget de código de dispositivo OAuth یا دستی ٹوکن فیلڈ کے ذریعے token de portador فراہم کریں (جب `TRYIT_PROXY_ALLOW_CLIENT_AUTH=1` کنفیگر ہو تو پراکسی `X-TryIt-Auth` anula قبول کرتا ہے)۔
5. درخواست بھیجیں اور تصدیق کریں کہ Torii `fixtures/norito_rpc/schema_hashes.json` میں درج `schema_hash` واپس دیتا ہے۔ مطابقت پذیر hashes تصدیق کرتے ہیں کہ Norito header براؤزر/پراکسی hop سے محفوظ رہا۔روڈ میپ ثبوت کے لئے Pruébelo کا اسکرین شاٹ `scripts/run_norito_rpc_fixtures.sh --note "<ticket>"` کی رَن کے ساتھ جوڑیں۔ یہ اسکرپٹ `cargo xtask norito-rpc-verify` کو ریپ کرتا ہے، JSON خلاصہ `artifacts/norito_rpc/<timestamp>/` میں لکھتا ہے، اور وہی accesorios حاصل کرتا ہے جو پورٹل نے استعمال کئے تھے۔

## مسئلہ حل کرنا| علامت | کہاں نظر آتا ہے | ممکنہ وجہ | حل |
| --- | --- | --- | --- |
| `415 Unsupported Media Type` | Respuesta Torii | `Content-Type` encabezado غائب یا غلط | carga útil بھیجنے سے پہلے `Content-Type: application/x-norito` سیٹ کریں۔ |
| `X-Iroha-Error-Code: schema_mismatch` (HTTP 400) | Torii cuerpo/encabezados de respuesta | accesorios کا esquema hash Torii compilación سے مختلف ہے | `cargo xtask norito-rpc-fixtures` سے accesorios دوبارہ بنائیں اور `fixtures/norito_rpc/schema_hashes.json` میں hash تصدیق کریں؛ Punto final ابھی Norito فعال نہیں کرتا تو JSON fallback استعمال کریں۔ |
| `{"error":"origin_forbidden"}` (HTTP 403) | Pruébelo respuesta proxy | درخواست ایسے origen سے آئی جو `TRYIT_PROXY_ALLOWED_ORIGINS` میں درج نہیں | پورٹل کا origen (مثلا `https://docs.devnet.sora.example`) env var میں شامل کریں اور پراکسی ری اسٹارٹ کریں۔ |
| `{"error":"rate_limited"}` (HTTP 429) | Pruébelo respuesta proxy | فی IP کوٹا `TRYIT_PROXY_RATE_LIMIT`/`TRYIT_PROXY_RATE_WINDOW_MS` بجٹ سے تجاوز کر گیا | اندرونی load ٹیسٹ کے لئے حد بڑھائیں یا ونڈو ری سیٹ ہونے تک انتظار کریں (respuesta JSON میں `retryAfterMs` دیکھیں)۔ |
| `{"error":"upstream_timeout"}` (HTTP 504) o `{"error":"upstream_error"}` (HTTP 502) | Pruébelo respuesta proxy | Torii ٹائم آؤٹ ہوا یا پراکسی backend configurado تک نہیں پہنچ سکا | `TRYIT_PROXY_TARGET` Tarjeta de crédito Torii Tarjeta de crédito `TRYIT_PROXY_TIMEOUT_MS` Tarjeta de crédito کر دوبارہ کوشش کریں۔ |

Más Diagnóstico Pruébelo اور Consejos de OAuth [`devportal/try-it.md`](./try-it.md#norito-rpc-samples) میں موجود ہیں۔## اضافی وسائل
- Transporte RFC: `docs/source/torii/norito_rpc.md`
- Resumen ejecutivo: `docs/source/torii/norito_rpc_brief.md`
- Rastreador de acciones: `docs/source/torii/norito_rpc_tracker.md`
- Instrucciones de prueba de proxy: `docs/portal/docs/devportal/try-it.md`