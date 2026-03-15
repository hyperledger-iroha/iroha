---
lang: es
direction: ltr
source: docs/portal/docs/devportal/torii-rpc-overview.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Resumen de Norito-RPC

Norito-RPC es el transporte binario para las API de Torii. Reutiliza las mismas rutas HTTP que `/v1/pipeline` pero intercambia cargas marcadas por Norito que incluyen hashes de esquema y sumas de comprobación. Usalo cuando necesites respuestas deterministas y validadas o cuando las respuestas JSON del pipeline se vuelvan un cuello de botella.

## ¿Por qué cambiar?
- El enmarcado determinista con CRC64 y hashes de esquema reducen los errores de decodificación.
- Los helpers Norito compartidos entre SDK te permiten reutilizar tipos existentes del modelo de datos.
- Torii ya etiqueta las sesiones Norito en telemetría, así que los operadores pueden monitorear la adopción con los paneles provistos.

## Como hacer una solicitud

```bash
curl       -H 'Content-Type: application/x-norito'       -H 'Accept: application/x-norito'       -H "Authorization: Bearer ${TOKEN}"       --data-binary @signed_transaction.norito       https://torii.devnet.sora.example/v1/transactions/submit
```

1. Serializa tu carga útil con el códec Norito (`iroha_client`, ayudantes del SDK o `norito::to_bytes`).
2. Envia la solicitud con `Content-Type: application/x-norito`.
3. Solicita una respuesta Norito usando `Accept: application/x-norito`.
4. Decodifica la respuesta con el ayudante de SDK correspondiente.Guía del SDK:
- **Rust**: `iroha_client::Client` negocia Norito automáticamente cuando estableces el header `Accept`.
- **Python**: usa `NoritoRpcClient` de `iroha_python.norito_rpc`.
- **Android**: usa `NoritoRpcClient` y `NoritoRpcRequestOptions` en el SDK de Android.
- **JavaScript/Swift**: los helpers se rastrearon en `docs/source/torii/norito_rpc_tracker.md` y llegaron como parte de NRPC-3.

## Ejemplo de consola Pruébalo

El portal de desarrolladores incluye un proxy Try It para que los revisores puedan reproducir cargas útiles Norito sin escribir scripts a medida.

1. [Inicia el proxy](./try-it.md#start-the-proxy-locally) y define `TRYIT_PROXY_PUBLIC_URL` para que los widgets sepan a donde enviar el tráfico.
2. Abra la tarjeta **Pruébelo** en esta página o el panel `/reference/torii-swagger` y seleccione un punto final como `POST /v1/pipeline/submit`.
3. Cambie el **Content-Type** a `application/x-norito`, elija el editor **Binary** y sube `fixtures/norito_rpc/transfer_asset.norito` (o cualquier lista de carga útil en `fixtures/norito_rpc/transaction_fixtures.manifest.json`).
4. Proporciona un token de portador a través del widget OAuth device-code o el campo de token manual (el proxy acepta overrides `X-TryIt-Auth` cuando se configura con `TRYIT_PROXY_ALLOW_CLIENT_AUTH=1`).
5. Envia la solicitud y verifica que Torii refleja el `schema_hash` listado en `fixtures/norito_rpc/schema_hashes.json`. Los hashes coinciden confirman que el encabezado Norito sobrevivió al salto navegador/proxy.Para evidencia del roadmap, combine la captura de pantalla de Try It con una ejecución de `scripts/run_norito_rpc_fixtures.sh --note "<ticket>"`. El script envuelve `cargo xtask norito-rpc-verify`, escribe el resumen JSON en `artifacts/norito_rpc/<timestamp>/` y captura los mismos dispositivos que consume el portal.

## Resolución de problemas| Síntoma | Donde aparece | Causa probable | Solución |
| --- | --- | --- | --- |
| `415 Unsupported Media Type` | Respuesta de Torii | Falta o es incorrecto el encabezado `Content-Type` | Defina `Content-Type: application/x-norito` antes de enviar la carga útil. |
| `X-Iroha-Error-Code: schema_mismatch` (HTTP 400) | Cuerpo/encabezados de respuesta de Torii | El hash del esquema de accesorios difiere del build de Torii | Regenera accesorios con `cargo xtask norito-rpc-fixtures` y confirma el hash en `fixtures/norito_rpc/schema_hashes.json`; USA Fallback JSON si el punto final aún no está habilitado Norito. |
| `{"error":"origin_forbidden"}` (HTTP 403) | Respuesta del proxy Pruébalo | La solicitud provino de un origen que no esta listado en `TRYIT_PROXY_ALLOWED_ORIGINS` | Agrega el origen del portal (por ejemplo, `https://docs.devnet.sora.example`) a la variable de entorno y reinicia el proxy. |
| `{"error":"rate_limited"}` (HTTP 429) | Respuesta del proxy Pruébalo | La cuota por IP excede el presupuesto de `TRYIT_PROXY_RATE_LIMIT`/`TRYIT_PROXY_RATE_WINDOW_MS` | Incrementa el límite para pruebas internas de carga o espera hasta que la ventana se reinicie (consulta `retryAfterMs` en la respuesta JSON). |
| `{"error":"upstream_timeout"}` (HTTP 504) o `{"error":"upstream_error"}` (HTTP 502) | Respuesta del proxy Pruébalo | Torii agoto el tiempo o el proxy no pudo alcanzar el backend configurado | Verifica que `TRYIT_PROXY_TARGET` sea accesible, revisa la salud de Torii o reintenta con un `TRYIT_PROXY_TIMEOUT_MS` mayor. |Mas diagnósticos de Try It y consejos de OAuth viven en [`devportal/try-it.md`](./try-it.md#norito-rpc-samples).

## Recursos adicionales
- RFC de transporte: `docs/source/torii/norito_rpc.md`
- Resumen ejecutivo: `docs/source/torii/norito_rpc_brief.md`
- Rastreador de acciones: `docs/source/torii/norito_rpc_tracker.md`
- Instrucciones del proxy Try-It: `docs/portal/docs/devportal/try-it.md`