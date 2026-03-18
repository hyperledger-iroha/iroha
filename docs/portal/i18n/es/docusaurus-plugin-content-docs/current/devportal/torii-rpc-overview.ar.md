---
lang: es
direction: ltr
source: docs/portal/docs/devportal/torii-rpc-overview.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# نظرة عامة على Norito-RPC

Norito-RPC y el sistema operativo Torii. Utilización de protocolos HTTP con `/v1/pipeline` y configuración de hashes con Norito. للمخطط y sumas de verificación. Establece archivos de código fuente y archivos de datos JSON en la tubería.

## لماذا ننتقل؟
- Utilice CRC64 y hashes para crear archivos.
- Utilice el software Norito para utilizar los SDK que deseen utilizar.
- Torii يوسم جلسات Norito بالفعل في القياس عن بعد، لذا يمكن للمشغلين متابعة الاعتماد عبر لوحات المراقبة المتاحة.

## تنفيذ طلب

```bash
curl       -H 'Content-Type: application/x-norito'       -H 'Accept: application/x-norito'       -H "Authorization: Bearer ${TOKEN}"       --data-binary @signed_transaction.norito       https://torii.devnet.sora.example/v1/transactions/submit
```

1. Utilice el códec Norito (`iroha_client`, SDK de software y `norito::to_bytes`).
2. ارسل الطلب مع `Content-Type: application/x-norito`.
3. Utilice el conector Norito y el `Accept: application/x-norito`.
4. افك ترميز الاستجابة باستخدام مساعد SDK المطابق.

Requisitos del SDK:
- **Óxido**: `iroha_client::Client` يتفاوض على Norito تلقائيا عندما تضبط ترويسة `Accept`.
- **Python**: Utiliza `NoritoRpcClient` desde `iroha_python.norito_rpc`.
- **Android**: Utilice `NoritoRpcClient` e `NoritoRpcRequestOptions` en el SDK de Android.
- **JavaScript/Swift**: Configuración de `docs/source/torii/norito_rpc_tracker.md` y NRPC-3.

## مثال وحدة Pruébalo

يوفر بوابة المطورين وكيل Pruébalo كي يتمكن المراجعون من اعادة تشغيل حمولات Norito دون كتابة نصوص مخصصة.1. [ابدأ الوكيل](./try-it.md#start-the-proxy-locally) واضبط `TRYIT_PROXY_PUBLIC_URL` حتى تعرف الادوات المصغرة اين ترسل الحركة.
2. افتح بطاقة **Pruébalo** في هذه الصفحة او لوحة `/reference/torii-swagger` واختر مسارا مثل `POST /v1/pipeline/submit`.
3. Seleccione **Tipo de contenido** en `application/x-norito`, haga clic en **Binario** y en `fixtures/norito_rpc/transfer_asset.norito` (es decir, haga clic en `fixtures/norito_rpc/transaction_fixtures.manifest.json`).
4. Utilice el token de portador y el código de dispositivo OAuth y el código de dispositivo OAuth (الوكيل يقبل تجاوزات `X-TryIt-Auth` y `TRYIT_PROXY_ALLOW_CLIENT_AUTH=1`).
5. Conecte el conector Torii al conector `schema_hash` y el `fixtures/norito_rpc/schema_hashes.json`. Utilice el cable Norito para conectar el cable de alimentación/controlador.

Para obtener más información, consulte Pruébelo en `scripts/run_norito_rpc_fixtures.sh --note "<ticket>"`. يقوم السكربت بتغليف `cargo xtask norito-rpc-verify`, y ملخص JSON الى `artifacts/norito_rpc/<timestamp>/`, y يلتقط نفس accesorios التي استخدمها البوابة.

## استكشاف الاخطاء| العرض | مكان الظهور | السبب المحتمل | الحل |
| --- | --- | --- | --- |
| `415 Unsupported Media Type` | Soporte Torii | ترويسة `Content-Type` مفقودة او غير صحيحة | Este es el caso `Content-Type: application/x-norito`. |
| `X-Iroha-Error-Code: schema_mismatch` (HTTP 400) | Accesorios para el hogar Torii | hash مخطط accesorios يختلف عن build Torii | اعادة توليد accesorios عبر `cargo xtask norito-rpc-fixtures` y تأكيد hash في `fixtures/norito_rpc/schema_hashes.json`؛ El JSON de respaldo es un archivo Norito. |
| `{"error":"origin_forbidden"}` (HTTP 403) | استجابة وكيل Pruébalo | جاء الطلب من اصل غير مدرج في `TRYIT_PROXY_ALLOWED_ORIGINS` | Asegúrese de que el dispositivo esté conectado (modelo `https://docs.devnet.sora.example`). |
| `{"error":"rate_limited"}` (HTTP 429) | استجابة وكيل Pruébalo | Impresoras IP IP `TRYIT_PROXY_RATE_LIMIT`/`TRYIT_PROXY_RATE_WINDOW_MS` | Haga clic en el enlace de abajo para obtener información sobre el archivo de configuración (escrito por `retryAfterMs` en formato JSON). |
| `{"error":"upstream_timeout"}` (HTTP 504) y `{"error":"upstream_error"}` (HTTP 502) | استجابة وكيل Pruébalo | Torii Otros productos y accesorios para el hogar | Coloque el cable `TRYIT_PROXY_TARGET` y el cable Torii y el cable `TRYIT_PROXY_TIMEOUT_MS`. |

Pruébelo y utilice OAuth en [`devportal/try-it.md`](./try-it.md#norito-rpc-samples).

## موارد اضافية
- RFC: `docs/source/torii/norito_rpc.md`
- Idioma: `docs/source/torii/norito_rpc_brief.md`
- Nombre del usuario: `docs/source/torii/norito_rpc_tracker.md`
- Aplicación y prueba Pruébelo: `docs/portal/docs/devportal/try-it.md`