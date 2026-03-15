---
lang: es
direction: ltr
source: docs/portal/docs/devportal/torii-rpc-overview.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Objeto Norito-RPC

Norito-RPC: transporte binario para API Torii. Al implementar las rutas HTTP, entre `/v2/pipeline`, no se observa el código Norito con esquemas y sumas de comprobación. Utilice, por ejemplo, varios parámetros de configuración y otras fuentes o un código JSON de canalización de archivos permanentes.

## ¿Зачем переключаться?
- Utilice un controlador de temperatura CRC64 y algunas otras unidades de codificación.
- El asistente Norito ayuda a que el SDK pueda personalizar los modelos de estos modelos.
- Torii para conectar Norito durante una sesión en televisores, los operadores pueden evitar las restricciones доступные дашборды.

## Отправка запроса

```bash
curl       -H 'Content-Type: application/x-norito'       -H 'Accept: application/x-norito'       -H "Authorization: Bearer ${TOKEN}"       --data-binary @signed_transaction.norito       https://torii.devnet.sora.example/v2/transactions/submit
```

1. Configure la carga útil del códec Norito (`iroha_client`, SDK helper-ы или `norito::to_bytes`).
2. Haga clic en `Content-Type: application/x-norito`.
3. Conecte Norito a `Accept: application/x-norito`.
4. Configure el asistente auxiliar del SDK.

Recomendaciones para el SDK:
- **Rust**: `iroha_client::Client` автоматически договаривается о Norito, когда задан заголовок `Accept`.
- **Python**: utilice `NoritoRpcClient` y `iroha_python.norito_rpc`.
- **Android**: implemente `NoritoRpcClient` e `NoritoRpcRequestOptions` en el SDK de Android.
- **JavaScript/Swift**: ayudante-ы отслеживаются в `docs/source/torii/norito_rpc_tracker.md` y появятся в рамках NRPC-3.

## Primeras consolas PruébaloEl programa de actualización del portal Pruébelo, que puede utilizar la carga útil Norito sin usar la versión original. скриптов.

1. [Запустите прокси](./try-it.md#start-the-proxy-locally) and задайте `TRYIT_PROXY_PUBLIC_URL`, чтобы виджеты знали, куда отправлять трафик.
2. Abra la tarjeta **Pruébelo** en esta página o panel `/reference/torii-swagger` y acceda al punto final, con el nombre `POST /v2/pipeline/submit`.
3. Inserte **Tipo de contenido** en `application/x-norito`, agregue el editor **Binario** y descargue `fixtures/norito_rpc/transfer_asset.norito` (o la carga útil más grande en `fixtures/norito_rpc/transaction_fixtures.manifest.json`).
4. Utilice el token de portador en el widget de código de dispositivo OAuth o en su lugar (el código anula `X-TryIt-Auth`, mientras que `TRYIT_PROXY_ALLOW_CLIENT_AUTH=1`).
5. Mantenga presionado el botón Torii junto con `schema_hash`, conectado a `fixtures/norito_rpc/schema_hashes.json`. Совпадение хешей подтверждает, что заголовок Norito пережил прыжок браузер/прокси.

La hoja de ruta de la aplicación está escrita en Pruébelo con el teclado `scripts/run_norito_rpc_fixtures.sh --note "<ticket>"`. El script `cargo xtask norito-rpc-verify`, escribe el texto JSON en `artifacts/norito_rpc/<timestamp>/` y descarga estos accesorios, según el portal actual.

## Устранение неполадок| Síntoma | Где проявляется | Вероятная причина | Исправление |
| --- | --- | --- | --- |
| `415 Unsupported Media Type` | Salida Torii | Отсутствует o неверный заголовок `Content-Type` | Instale `Content-Type: application/x-norito` antes de la carga útil. |
| `X-Iroha-Error-Code: schema_mismatch` (HTTP 400) | Тело/заголовки ответа Torii | Estos accesorios no están compatibles con el sistema Torii | Coloque los accesorios en `cargo xtask norito-rpc-fixtures` y póngalos en `fixtures/norito_rpc/schema_hashes.json`; Utilice el respaldo JSON, ya que el punto final no incluye Norito. |
| `{"error":"origin_forbidden"}` (HTTP 403) | Ответ Pruébelo прокси | Запрос пришел с origin, которого нет в `TRYIT_PROXY_ALLOWED_ORIGINS` | Agregue el portal de origen (por ejemplo, `https://docs.devnet.sora.example`) en el proceso permanente de operación y operación. |
| `{"error":"rate_limited"}` (HTTP 429) | Ответ Pruébelo прокси | Código de IP anterior `TRYIT_PROXY_RATE_LIMIT`/`TRYIT_PROXY_RATE_WINDOW_MS` | Utilice un límite máximo para las pruebas de texto existentes o establezca una secuencia de comandos (compruebe `retryAfterMs` en formato JSON). |
| `{"error":"upstream_timeout"}` (HTTP 504) o `{"error":"upstream_error"}` (HTTP 502) | Ответ Pruébelo прокси | Torii El proceso y el proceso no están disponibles en el backend | Guarde el dispositivo `TRYIT_PROXY_TARGET`, conecte el Torii o conecte los bloques `TRYIT_PROXY_TIMEOUT_MS`. |

Muchos diagnósticos Pruébelo y aplicaciones de OAuth en [`devportal/try-it.md`](./try-it.md#norito-rpc-samples).## Recursos adicionales
- Transporte RFC: `docs/source/torii/norito_rpc.md`
- Respuesta incorrecta: `docs/source/torii/norito_rpc_brief.md`
- Трекер задач: `docs/source/torii/norito_rpc_tracker.md`
- Instrucciones Proceso Try-It: `docs/portal/docs/devportal/try-it.md`