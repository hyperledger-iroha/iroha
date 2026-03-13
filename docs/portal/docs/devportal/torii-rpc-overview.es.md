---
lang: es
direction: ltr
source: docs/portal/docs/devportal/torii-rpc-overview.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1017858988f6bbc1c58029ca0476e2eee7b011c3c65ba5b33a80c049165600ca
source_last_modified: "2025-11-11T10:26:38.026921+00:00"
translation_last_reviewed: 2026-01-01
---

# Resumen de Norito-RPC

Norito-RPC es el transporte binario para las APIs de Torii. Reutiliza las mismas rutas HTTP que `/v2/pipeline` pero intercambia cargas enmarcadas por Norito que incluyen hashes de esquema y checksums. Usalo cuando necesites respuestas deterministas y validadas o cuando las respuestas JSON del pipeline se vuelvan un cuello de botella.

## Por que cambiar?
- El enmarcado determinista con CRC64 y hashes de esquema reduce errores de decodificacion.
- Los helpers Norito compartidos entre SDKs te permiten reutilizar tipos existentes del modelo de datos.
- Torii ya etiqueta las sesiones Norito en telemetria, asi que los operadores pueden monitorear la adopcion con los paneles provistos.

## Como hacer una solicitud

```bash
curl       -H 'Content-Type: application/x-norito'       -H 'Accept: application/x-norito'       -H "Authorization: Bearer ${TOKEN}"       --data-binary @signed_transaction.norito       https://torii.devnet.sora.example/v2/transactions/submit
```

1. Serializa tu payload con el codec Norito (`iroha_client`, helpers del SDK o `norito::to_bytes`).
2. Envia la solicitud con `Content-Type: application/x-norito`.
3. Solicita una respuesta Norito usando `Accept: application/x-norito`.
4. Decodifica la respuesta con el helper de SDK correspondiente.

Guia por SDK:
- **Rust**: `iroha_client::Client` negocia Norito automaticamente cuando estableces el header `Accept`.
- **Python**: usa `NoritoRpcClient` de `iroha_python.norito_rpc`.
- **Android**: usa `NoritoRpcClient` y `NoritoRpcRequestOptions` en el SDK de Android.
- **JavaScript/Swift**: los helpers se rastrean en `docs/source/torii/norito_rpc_tracker.md` y llegaran como parte de NRPC-3.

## Ejemplo de consola Try It

El portal de desarrolladores incluye un proxy Try It para que los revisores puedan reproducir payloads Norito sin escribir scripts a medida.

1. [Inicia el proxy](./try-it.md#start-the-proxy-locally) y define `TRYIT_PROXY_PUBLIC_URL` para que los widgets sepan a donde enviar el trafico.
2. Abre la tarjeta **Try it** en esta pagina o el panel `/reference/torii-swagger` y selecciona un endpoint como `POST /v2/pipeline/submit`.
   For MCP/agent flows, use `/reference/torii-mcp`.
3. Cambia el **Content-Type** a `application/x-norito`, elige el editor **Binary** y sube `fixtures/norito_rpc/transfer_asset.norito` (o cualquier payload listado en `fixtures/norito_rpc/transaction_fixtures.manifest.json`).
4. Proporciona un bearer token via el widget OAuth device-code o el campo de token manual (el proxy acepta overrides `X-TryIt-Auth` cuando se configura con `TRYIT_PROXY_ALLOW_CLIENT_AUTH=1`).
5. Envia la solicitud y verifica que Torii refleje el `schema_hash` listado en `fixtures/norito_rpc/schema_hashes.json`. Los hashes coincidentes confirman que el encabezado Norito sobrevivio el salto navegador/proxy.

Para evidencia del roadmap, combina la captura de pantalla de Try It con una ejecucion de `scripts/run_norito_rpc_fixtures.sh --note "<ticket>"`. El script envuelve `cargo xtask norito-rpc-verify`, escribe el resumen JSON en `artifacts/norito_rpc/<timestamp>/` y captura los mismos fixtures que consumio el portal.

## Resolucion de problemas

| Sintoma | Donde aparece | Causa probable | Solucion |
| --- | --- | --- | --- |
| `415 Unsupported Media Type` | Respuesta de Torii | Falta o es incorrecto el header `Content-Type` | Define `Content-Type: application/x-norito` antes de enviar el payload. |
| `X-Iroha-Error-Code: schema_mismatch` (HTTP 400) | Cuerpo/headers de respuesta de Torii | El hash del esquema de fixtures difiere del build de Torii | Regenera fixtures con `cargo xtask norito-rpc-fixtures` y confirma el hash en `fixtures/norito_rpc/schema_hashes.json`; usa fallback JSON si el endpoint aun no habilito Norito. |
| `{"error":"origin_forbidden"}` (HTTP 403) | Respuesta del proxy Try It | La solicitud provino de un origen que no esta listado en `TRYIT_PROXY_ALLOWED_ORIGINS` | Agrega el origen del portal (por ejemplo, `https://docs.devnet.sora.example`) a la variable de entorno y reinicia el proxy. |
| `{"error":"rate_limited"}` (HTTP 429) | Respuesta del proxy Try It | La cuota por IP excedio el presupuesto de `TRYIT_PROXY_RATE_LIMIT`/`TRYIT_PROXY_RATE_WINDOW_MS` | Incrementa el limite para pruebas internas de carga o espera hasta que la ventana se reinicie (consulta `retryAfterMs` en la respuesta JSON). |
| `{"error":"upstream_timeout"}` (HTTP 504) o `{"error":"upstream_error"}` (HTTP 502) | Respuesta del proxy Try It | Torii agoto el tiempo o el proxy no pudo alcanzar el backend configurado | Verifica que `TRYIT_PROXY_TARGET` sea accesible, revisa la salud de Torii o reintenta con un `TRYIT_PROXY_TIMEOUT_MS` mayor. |

Mas diagnosticos de Try It y consejos de OAuth viven en [`devportal/try-it.md`](./try-it.md#norito-rpc-samples).

## Recursos adicionales
- RFC de transporte: `docs/source/torii/norito_rpc.md`
- Resumen ejecutivo: `docs/source/torii/norito_rpc_brief.md`
- Tracker de acciones: `docs/source/torii/norito_rpc_tracker.md`
- Instrucciones del proxy Try-It: `docs/portal/docs/devportal/try-it.md`
