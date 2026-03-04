---
lang: es
direction: ltr
source: docs/source/sorafs_gateway_chunk_range.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7f22f6f23f1845733aa8145ac594cee529534dc76f74427cd455d84857cedc6d
source_last_modified: "2025-11-02T14:56:07.845819+00:00"
translation_last_reviewed: "2026-01-30"
---

# Integración de chunk-range y scheduler en el gateway SoraFS

## Objetivos

- Implementar endpoints HTTP de rango deterministas que respeten la semántica de `dag-scope`.
- Hacer cumplir tokens de stream por peer ligados a la política de admisión y a las declaraciones de capacidad.
- Emitir telemetría para solicitudes de rango (`Sora-Chunk-Range`) para alimentar al orquestador y a la observabilidad.

## Requisitos de API

| Método | Ruta | Encabezados requeridos | Notas |
|--------|------|-------------------------|-------|
| `GET`  | `/car/{manifest_id}` | `Range`, `dag-scope=block`, `X-SoraFS-Chunker`, `X-SoraFS-Nonce`, `X-SoraFS-Stream-Token`, `Sora-Name` (alias opcional) | Devuelve un slice CAR alineado; el alias se valida contra el sobre del manifiesto. El token de stream debe estar codificado en Norito base64. |
| `GET`  | `/chunk/{manifest_id}/{chunk_digest}` | `X-SoraFS-Nonce`, `X-SoraFS-Stream-Token` | Recuperación de un chunk único con encabezados deterministas. |
| `POST` | `/token` | `X-SoraFS-Client`, `X-SoraFS-Nonce`, sobre de manifiesto firmado | Emite un token de stream por peer con TTL y límites de tasa. |

Las respuestas CAR DEBEN incluir:
- `Content-Range: bytes start-end/total`
- `X-Sora-Chunk-Range: start={start};end={end};chunks={count}`
- `X-SoraFS-Chunker` (eco)
- `X-SoraFS-Stream-Token`/`X-SoraFS-Nonce` (eco)

Las respuestas de chunk DEBEN incluir:
- `Content-Type: application/octet-stream`
- `X-Sora-Chunk-Range: start={offset};end={offset+len-1};chunks=1`
- `X-SoraFS-Chunk-Digest`
- Nonce / token de stream en eco cuando se hayan enviado

## Cumplimiento del token de stream

- Metadatos del token:
  - ID del proveedor
  - Digest del manifiesto / handle del chunker
  - Máximo de streams concurrentes
  - Expiración (segundos de época)
  - Presupuesto de rate-limit (req/min, bytes/s)
- La ruta de verificación comprueba la firma del token + el sobre de admisión.
- En caso de violación (expirado, sobre presupuesto) responder `429` con motivo `stream_token_exhausted`.

## Telemetría

Métricas:
- `sorafs_gateway_chunk_range_requests_total{result,chunker}`
- `sorafs_gateway_stream_tokens_active`
- `sorafs_gateway_stream_token_denials_total{reason}`
- `sorafs_gateway_chunk_range_latency_ms_bucket`

Logs:
- Eventos estructurados cuando se emite/revoca un token
- ID de correlación que enlaza el token con la descarga del orquestador (futura integración SF-6b)

## Firma y rotación de tokens

- **Almacenamiento de claves.** Los gateways escriben el secreto de firma Ed25519 en
  `sorafs_gateway_secrets/token_signing_sk` (solo lectura para el proceso del gateway). Un registro de clave pública correspondiente se publica en el manifiesto de admisión bajo `gateway.token_signing_pk`.
- **Distribución.** Proveedores y orquestadores cargan la clave pública al iniciar y la almacenan en caché para verificar firmas; los manifiestos incluyen `token_pk_version` para coordinar cambios en caliente.
- **Cadencia de rotación.** Las claves rotan trimestralmente. La rotación se gestiona con el runbook `SF-6b-ROTATE-KEYS`:
  1. Generar un nuevo par de claves en el host del gateway (`sorafs-gateway key rotate --kind token-signing`).
  2. Actualizar el manifiesto de admisión y publicarlo en el Pin Registry.
  3. Notificar a los orquestadores vía el tópico de telemetría `sorafs.gateway.token_pk_update`.
  4. Mantener la clave anterior disponible por 24 h para respetar tokens en vuelo.
- **Rastro de auditoría.** Los gateways emiten `sorafs_gateway_token_key_rotation_total` y registran la huella de la nueva clave en `ops/sorafs_gateway_tokens.md`.

## Esquema canónico del token

- **Conjunto de campos.** Los tokens se serializan como Norito JSON con los siguientes campos:
  - `token_id` (cadena ULID)
  - `manifest_cid`
  - `provider_id`
  - `profile_handle`
  - `max_streams`
  - `ttl_epoch`
  - `rate_limit_bytes`
  - `issued_at`
  - `requests_per_minute`
  - `signature` (hex Ed25519)
- **Reglas de normalización.** Los campos se ordenan alfabéticamente antes de firmar para asegurar serialización canónica. Los valores numéricos se representan como enteros; los tiempos usan segundos Unix epoch.
- **Alineación con el scoreboard.** El scoreboard del orquestador ingiere estos campos directamente, mapeando `max_streams`, `ttl_epoch` y `rate_limit_bytes` a factores de disponibilidad y penalización. Señales adicionales (por ejemplo, salud del token) derivan de la telemetría de emisión usando `token_id`.
- **Helpers de validación.** El crate compartido en Rust `sorafs_token_schema` expondrá `Token::sign` / `Token::verify` y validación de esquema para minimizar duplicación entre los binarios del gateway y el orquestador.

## API segura de emisión de tokens

- **Autenticación.** `/token` requiere mTLS. Los clientes deben presentar certificados firmados por la CA de admisión; el gateway valida el sujeto del certificado contra el manifiesto de admisión.
- **Flujo de solicitud.**
  1. El cliente envía `POST /token` con los headers `X-SoraFS-Client`, `X-SoraFS-Nonce` y un sobre de manifiesto firmado en el cuerpo.
  2. El gateway autentica al cliente, valida el manifiesto y aplica límites de tasa por cliente usando `X-SoraFS-Client`.
  3. El gateway genera un token, lo firma con la clave Ed25519 y devuelve JSON:

     ```json
     {
       "token": { /* campos canónicos */ },
       "signature": "hex",
       "expires_at": 1738368000
     }
     ```

  4. Los headers de respuesta incluyen `X-SoraFS-Token-Id`, `X-SoraFS-Client-Quota-Remaining` y el `X-SoraFS-Nonce` en eco. `X-SoraFS-Client-Quota-Remaining` informa cuántas solicitudes de emisión quedan en la ventana actual de 60 s; cuando el cupo es ilimitado el header se fija a `unlimited`. Una vez agotado el presupuesto, el gateway devuelve `429` junto con `Retry-After` indicando cuándo se permite la siguiente solicitud de token.
- **Telemetría.** El gateway registra métricas de emisión:
  - `sorafs_gateway_token_issuance_total{client,result}`
  - `sorafs_gateway_token_issuance_latency_ms_bucket`
  - `sorafs_gateway_token_denials_total{reason}`
- **Protección contra abuso.** Los clientes que excedan su presupuesto `requests_per_minute` reciben `429 stream_token_rate_limited`. Los replays de nonce se rechazan con `409` y se registran en logs de auditoría.

## Documentación y despliegue

- **Documentación del protocolo.** Ampliar `docs/source/sorafs_node_client_protocol.md` con:
  - Ejemplos de request/response de `/token`.
  - Definiciones del esquema del token y pasos de verificación de firma.
  - Matriz de errores que describa los casos `401`, `403`, `409`, `429` y `5xx`.
- **Actualizaciones de SDK.** Coordinar con los equipos de SDK para añadir helpers:
  - Rust: `sorafs_sdk::TokenClient::request_token`.
  - TypeScript: `requestToken(manifestCid, profileHandle, options)`.
  - Go: `client.RequestToken(ctx, manifestCID, opts)`.
- **Gestión del cambio.** El despliegue inicial apunta al hito SF-5d:
  1. Implementar el controlador de tokens del gateway con el crate de esquema.
  2. Actualizar el orquestador para validar tokens usando el crate compartido.
  3. Publicar las actualizaciones de documentación y anunciar en notas de release (`RLS-105`).
  4. Habilitar dashboards de telemetría que sigan emisiones y denegaciones antes del GA.
