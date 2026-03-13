---
lang: es
direction: ltr
source: docs/portal/docs/devportal/torii-rpc-overview.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Visao general de Norito-RPC

Norito-RPC y el transporte binario para como API hacen Torii. Se reutilizan varios caminos HTTP de `/v2/pipeline` con más trocas de cargas útiles moldeadas por Norito que incluyen hashes de esquema y sumas de comprobación. Utilice cuando necesite respuestas determinísticas y validadas o cuando necesite respuestas JSON para que la tubería se convierta en un gargalo.

## ¿Por qué mudarse?
- Encuadre determinístico con CRC64 y hashes de esquema para reducir errores de decodificación.
- Helpers Norito compartilhados entre SDKs permite reutilizar tipos existentes en el modelo de dados.
- Torii ja marca sessoes Norito na telemetria, estos operadores pueden acompañar a adocao com os tableros fornecidos.

## Fazendo uma requisicao

```bash
curl       -H 'Content-Type: application/x-norito'       -H 'Accept: application/x-norito'       -H "Authorization: Bearer ${TOKEN}"       --data-binary @signed_transaction.norito       https://torii.devnet.sora.example/v2/transactions/submit
```

1. Serialice su carga útil con el códec Norito (`iroha_client`, los ayudantes del SDK o `norito::to_bytes`).
2. Envie a requisicao com `Content-Type: application/x-norito`.
3. Solicite una respuesta Norito usando `Accept: application/x-norito`.
4. Descifre una respuesta con el ayudante del SDK correspondiente.

Guía del SDK:
- **Rust**: `iroha_client::Client` negocia Norito automáticamente cuando se define el encabezado `Accept`.
- **Python**: use `NoritoRpcClient` de `iroha_python.norito_rpc`.
- **Android**: utilice `NoritoRpcClient` e `NoritoRpcRequestOptions` sin SDK de Android.
- **JavaScript/Swift**: los ayudantes están rastreados en `docs/source/torii/norito_rpc_tracker.md` y verificados como parte de NRPC-3.## Ejemplo de consola Pruébalo

El portal del desarrollador incluye un proxy Try It para que los revisores puedan reproducir las cargas útiles Norito sin grabar scripts a medida.

1. [Inicie o proxy](./try-it.md#start-the-proxy-locally) y defina `TRYIT_PROXY_PUBLIC_URL` para que los widgets saibam para onde enviar o trafego.
2. Abra la tarjeta **Pruébelo** en la página o en el panel `/reference/torii-swagger` y seleccione un punto final como `POST /v2/pipeline/submit`.
3. Mude **Content-Type** para `application/x-norito`, seleccione el editor **Binary** y envíe `fixtures/norito_rpc/transfer_asset.norito` (o cualquier lista de carga útil en `fixtures/norito_rpc/transaction_fixtures.manifest.json`).
4. Fornee un token de portador a través del código de dispositivo OAuth del widget o el campo manual (el proxy aceita anula `X-TryIt-Auth` cuando se configura con `TRYIT_PROXY_ALLOW_CLIENT_AUTH=1`).
5. Envie a requisicao y verifique se o Torii ecoa o `schema_hash` listado en `fixtures/norito_rpc/schema_hashes.json`. Los hashes confirman que el cabecalho Norito sobrevivió al salto navegador/proxy.

Para evidencia de la hoja de ruta, combine una captura de pantalla de Try It con una ejecución de `scripts/run_norito_rpc_fixtures.sh --note "<ticket>"`. El script involucra `cargo xtask norito-rpc-verify`, guarda el resumen JSON en `artifacts/norito_rpc/<timestamp>/` y captura todos los accesorios consumidos en el portal.

## Solución de problemas| Síntoma | Onde aparece | Causa prueba | Correçao |
| --- | --- | --- | --- |
| `415 Unsupported Media Type` | Respuesta a Torii | Encabezado `Content-Type` ausente o incorrecto | Defina `Content-Type: application/x-norito` antes de enviar o carga útil. |
| `X-Iroha-Error-Code: schema_mismatch` (HTTP 400) | Cuerpo/encabezados de respuesta de Torii | Hash de esquema de dos accesorios diferentes de compilación de Torii | Regenere los accesorios con `cargo xtask norito-rpc-fixtures` y confirme el hash en `fixtures/norito_rpc/schema_hashes.json`; utilice JSON alternativo para el punto final y no esté habilitado Norito. |
| `{"error":"origin_forbidden"}` (HTTP 403) | Respuesta al proxy Pruébalo | A requisicao veio de uma origem nao listada em `TRYIT_PROXY_ALLOWED_ORIGINS` | Agregue el origen del portal (por ejemplo, `https://docs.devnet.sora.example`) a diferentes ambientes y reinicie el proxy. |
| `{"error":"rate_limited"}` (HTTP 429) | Respuesta al proxy Pruébalo | Una cota por IP excedeu o presupuesto `TRYIT_PROXY_RATE_LIMIT`/`TRYIT_PROXY_RATE_WINDOW_MS` | Aumente el límite para las pruebas internas de carga o espere a que janela reinicie (veja `retryAfterMs` en la respuesta JSON). |
| `{"error":"upstream_timeout"}` (HTTP 504) o `{"error":"upstream_error"}` (HTTP 502) | Respuesta al proxy Pruébalo | Torii caducará o proxy y luego alcancar el backend configurado | Verifique si `TRYIT_PROXY_TARGET` está disponible, confire a saude do Torii o tente novamente con un `TRYIT_PROXY_TIMEOUT_MS` mayor. |

Más diagnósticos, Pruébelo y dicas de OAuth ficam en [`devportal/try-it.md`](./try-it.md#norito-rpc-samples).## Recursos adicionales
- RFC de transporte: `docs/source/torii/norito_rpc.md`
- Resumen ejecutivo: `docs/source/torii/norito_rpc_brief.md`
- Rastreador de acoes: `docs/source/torii/norito_rpc_tracker.md`
- Instrucciones para proxy Try-It: `docs/portal/docs/devportal/try-it.md`