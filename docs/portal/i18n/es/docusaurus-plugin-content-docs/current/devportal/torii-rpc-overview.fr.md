---
lang: es
direction: ltr
source: docs/portal/docs/devportal/torii-rpc-overview.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Apercu de Norito-RPC

Norito-RPC es el binario de transporte para API Torii. Se reutilizan los memes chemins HTTP que `/v2/pipeline` y se intercambian las cargas encadrees por Norito que incluyen hashes de esquema y sumas de verificación. Utilice cada vez que tenga respuestas determinadas y válidas o cuando las respuestas JSON de la tubería devengan un punto de estrangulamiento.

## ¿Cambiador de Pourquoi?
- Un cuadro determinado con CRC64 y los hashes de esquema reducen los errores de decodificación.
- Los ayudantes Norito comparten entre SDK y le permiten reutilizar los tipos existentes del modelo de donante.
- Torii deja las sesiones Norito en la telemetría, los operadores pueden adoptar fácilmente los paneles de control.

## Faire une requete

```bash
curl       -H 'Content-Type: application/x-norito'       -H 'Accept: application/x-norito'       -H "Authorization: Bearer ${TOKEN}"       --data-binary @signed_transaction.norito       https://torii.devnet.sora.example/v2/transactions/submit
```

1. Serialice su carga útil con el códec Norito (`iroha_client`, SDK de ayuda o `norito::to_bytes`).
2. Envoyez la requete avec `Content-Type: application/x-norito`.
3. Solicite una respuesta Norito a través de `Accept: application/x-norito`.
4. Decodifica la respuesta con el asistente SDK correspondiente.Consejos del SDK:
- **Rust**: `iroha_client::Client` negocia Norito automáticamente cuando defines el en-tete `Accept`.
- **Python**: utiliza `NoritoRpcClient` de `iroha_python.norito_rpc`.
- **Android**: utilice `NoritoRpcClient` e `NoritoRpcRequestOptions` en el SDK de Android.
- **JavaScript/Swift**: los ayudantes se encuentran en `docs/source/torii/norito_rpc_tracker.md` y llegan en NRPC-3.

## Ejemplo de consola Pruébalo

El portal de desarrollo dispone de un proxy Pruébelo después de que los selectores puedan recargar las cargas útiles Norito sin escribir scripts a medida.

1. [Demarrez le proxy](./try-it.md#start-the-proxy-locally) y defina `TRYIT_PROXY_PUBLIC_URL` para que los widgets compren o envíen el tráfico.
2. Abra la carta **Pruébelo** en esta página o en el panel `/reference/torii-swagger` y seleccione un punto final como `POST /v2/pipeline/submit`.
3. Pase **Content-Type** a `application/x-norito`, seleccione el editor **Binary** y cargue `fixtures/norito_rpc/transfer_asset.norito` (o toda la lista de carga útil en `fixtures/norito_rpc/transaction_fixtures.manifest.json`).
4. Introduzca un token de portador a través del código de dispositivo OAuth del widget o el manual de usuario (el proxy acepta las anulaciones `X-TryIt-Auth` cuando se configura con `TRYIT_PROXY_ALLOW_CLIENT_AUTH=1`).
5. Envíe la solicitud y verifique que Torii envíe la lista `schema_hash` a `fixtures/norito_rpc/schema_hashes.json`. Los hashes idénticos confirman que el dispositivo Norito se encuentra en el navegador/proxy de salto.Para la hoja de ruta de evidencia, asocie la captura de pantalla Pruébelo con una ejecución de `scripts/run_norito_rpc_fixtures.sh --note "<ticket>"`. El script encapsula `cargo xtask norito-rpc-verify`, escribe el resumen JSON en `artifacts/norito_rpc/<timestamp>/` y captura los memes accesorios que le portan a consommes.

## Depannage| Síntoma | Nuestro aparato | Causa probable | Correctivo |
| --- | --- | --- | --- |
| `415 Unsupported Media Type` | Respuesta Torii | En-tete `Content-Type` incorrecto o incorrecto | Defina `Content-Type: application/x-norito` antes de enviar la carga útil. |
| `X-Iroha-Error-Code: schema_mismatch` (HTTP 400) | Cuerpo/en-tetes de respuesta Torii | El hash del esquema de los accesorios difiere de la construcción Torii | Regenere los accesorios con `cargo xtask norito-rpc-fixtures` y confirme el hash en `fixtures/norito_rpc/schema_hashes.json`; Vuelva a pasar JSON si el punto final no está activo Norito. |
| `{"error":"origin_forbidden"}` (HTTP 403) | Respuesta del proxy Pruébelo | La solicitud procedente de un origen no registrado en `TRYIT_PROXY_ALLOWED_ORIGINS` | Ajoutez l'origine du portail (par ex. `https://docs.devnet.sora.example`) a la variable d'environnement et redemarrez le proxy. |
| `{"error":"rate_limited"}` (HTTP 429) | Respuesta del proxy Pruébelo | La cuota por IP para eliminar el presupuesto `TRYIT_PROXY_RATE_LIMIT`/`TRYIT_PROXY_RATE_WINDOW_MS` | Aumente el límite para las pruebas de carga internas o asista a la reinicialización de la ventana (ver `retryAfterMs` en la respuesta JSON). |
| `{"error":"upstream_timeout"}` (HTTP 504) o `{"error":"upstream_error"}` (HTTP 502) | Respuesta del proxy Pruébelo | Torii ha caducado o el proxy no puede eliminar la configuración del backend | Verifique que `TRYIT_PROXY_TARGET` esté accesible, controle la salud de Torii o vuelva a probar con un `TRYIT_PROXY_TIMEOUT_MS` más once. |Además de los diagnósticos Pruébelo y los consejos de OAuth se encuentran en [`devportal/try-it.md`](./try-it.md#norito-rpc-samples).

## Recursos complementarios
- Transporte RFC: `docs/source/torii/norito_rpc.md`
- Currículum ejecutivo: `docs/source/torii/norito_rpc_brief.md`
- Rastreador de acciones: `docs/source/torii/norito_rpc_tracker.md`
- Instrucciones del proxy Try-It: `docs/portal/docs/devportal/try-it.md`