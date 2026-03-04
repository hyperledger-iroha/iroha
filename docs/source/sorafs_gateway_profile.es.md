---
lang: es
direction: ltr
source: docs/source/sorafs_gateway_profile.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1f02810b8d1cc2067c9a06f63c5924b48182fd8435813c4548ee10b259b534ca
source_last_modified: "2025-12-05T17:03:20.753001+00:00"
translation_last_reviewed: "2026-01-30"
---

# Perfil trustless del gateway SoraFS (Borrador)

Este documento especifica el **perfil de entrega trustless** requerido para
gateways y clientes SoraFS que participan en el rollout SF-5. Captura la matriz
de requests/responses HTTP, los formatos de prueba y las reglas de verificación
necesarias para streamear objetos CAR de forma determinista sobre transporte no
confiable. Iteraciones futuras del suite de conformidad, pruebas de carga y
tooling de auto-certificación seguirán las definiciones aquí.

> **Estado:** Borrador. El feedback debe dirigirse al Networking TL y al QA Guild.

## 1. Alcance

El perfil aplica a endpoints HTTPS HTTP/1.1 y HTTP/2 que emiten chunks SoraFS
codificados como CARv2. Los gateways PUEDEN ofrecer otras APIs, pero **cada**
endpoint cubierto aquí DEBE cumplir el comportamiento normativo para que los
clientes puedan intercalar múltiples proveedores sin confianza fuera de banda.

## 2. Matriz de requests

| Método | Patrón de ruta                | Descripción                               | Headers requeridos                                 |
|--------|--------------------------------|-------------------------------------------|----------------------------------------------------|
| `GET`  | `/car/{manifest_cid}`          | Recuperación de objeto completo           | `Accept: application/vnd.ipld.car; dag-scope=full` |
| `GET`  | `/car/{manifest_cid}`          | Recuperación por rango (byte ranges)      | `Accept: application/vnd.ipld.car; dag-scope=block`, `Range` |
| `HEAD` | `/car/{manifest_cid}`          | Probe de metadata                         | `Accept: application/vnd.ipld.car`                 |
| `GET`  | `/chunk/{chunk_digest}`        | Recuperación de chunk único               | `Accept: application/octet-stream`, `X-SoraFS-Chunk-Index` |
| `GET`  | `/proof/{manifest_cid}`        | Payload de prueba PoR (`PorProofV1`)      | `Accept: application/json`                         |

### 2.1. Headers requeridos

* `X-SoraFS-Version`: `sf1` para el rollout inicial; se incrementará en perfiles futuros.
* `X-SoraFS-Client`: identificador opaco (útil para telemetría). OPCIONAL pero
  **RECOMENDADO**.
* `X-SoraFS-Nonce`: nonce hex de 32 bytes, reflejado en la firma de respuesta para evitar replay.

Los clientes DEBEN adjuntar el sobre de manifiesto firmado (`manifest_signatures.json`)
usando `X-SoraFS-Manifest-Envelope` al solicitar un CAR completo para permitir que los gateways
realicen checks de política (GAR, PDP/PoR).

## 3. Requisitos de respuesta

### 3.1. Headers comunes

Todas las respuestas exitosas (2xx) DEBEN incluir:

* `Content-Type`: `application/vnd.ipld.car` para streams CAR, `application/json`
  para bundles de prueba, `application/octet-stream` para chunks crudos.
* `X-SoraFS-Nonce`: eco del nonce del request.
* `X-SoraFS-Chunker`: handle canónico de chunker (`sorafs.sf1@1.0.0`).
* `X-SoraFS-Proof-Digest`: digest hex de la prueba PoR acompañante
  (`PorProofV1.proof_digest`, BLAKE3-256 sobre los campos de prueba excluyendo la firma).
* `X-SoraFS-PoR-Root`: digest hex de la raíz Merkle PoR para el scope solicitado.

Los gateways DEBEN fallar con `428 Precondition Required` cuando el sobre de manifiesto
falta o no coincide con el registro de admisión cacheado localmente.

### 3.2. Soporte de rangos

* Implementar byte ranges (`Range: bytes=start-end`). La respuesta DEBE incluir
  `Content-Range` y alinear chunks a los límites de bloque CAR anunciados en el
  plan de chunks del manifiesto.
* Hacer cumplir la semántica determinista de `dag-scope`:
  * `full`: DAG completo con raíz en `manifest_cid`.
  * `block`: subconjunto que contiene el rango solicitado, más bloques padres
    requeridos para validación.
* Códigos de estado: respuestas CAR completas devuelven `200 OK`; rangos alineados
  devuelven `206 Partial Content` con `Content-Range`, mientras que rangos inválidos
  devuelven `416 Range Not Satisfiable`.

### 3.3. Requests condicionales

* Soportar `If-None-Match` con el digest del manifiesto.
* Responder `304 Not Modified` cuando el digest proporcionado coincide con el manifiesto actual
  para permitir que los clientes omitan descargas redundantes.

## 4. Formatos de prueba

### 4.1. Integridad de CAR

* Cada respuesta CAR DEBE contener un sobre de prueba Norito con:
  * `manifest_digest`: BLAKE3-256 de `ManifestV1`.
  * `chunk_plan_digest`: SHA3-256 de la metadata de chunks ordenada.
  * `range_proof`: lista opcional de índices de chunk cubiertos por la respuesta.

Los clientes DEBEN verificar:

1. El digest BLAKE3 de los bytes streameados coincide con `manifest_digest`.
2. La prueba BLAKE3 se descomprime al plan de chunks registrado en el manifiesto.
3. Las secciones CAR se alinean con los offsets del plan de chunks.

### 4.2. Proof-of-Retrievability (PoR)

* Los gateways deben servir `GET /proof/{manifest_cid}` retornando un payload Norito JSON `PorProofV1`:
  * `manifest_digest`, `provider_id`, `samples[]`, `auth_path`, `signature`, `submitted_at`.
  * `auth_path` es la lista ordenada de raíces de chunk del árbol PoR.
  * `signature` es Ed25519 sobre `proof_digest` (BLAKE3-256 de los campos de prueba sin la firma), y `signature.public_key`
    es la clave pública Ed25519 del gateway.
* La clave de firma se configura en `torii.sorafs.storage.stream_tokens.signing_key_path`
  y se comparte con la emisión de stream tokens.

Los clientes DEBEN verificar la firma y asegurar que la clave pública coincida con la clave
del gateway admitido antes de confiar en el header `X-SoraFS-PoR-Root`.

### 4.3. Recibos de streaming

Cuando el streaming de chunks se completa con éxito, los gateways DEBERÍAN emitir un recibo firmado:

```json
{
  "version": 1,
  "provider_id": "hex",
  "manifest_digest": "hex",
  "range_start": 0,
  "range_end": 1048575,
  "duration_ms": 1200,
  "bytes_sent": 1048576,
  "por_root": "hex",
  "nonce": "hex",
  "signature": "hex"
}
```

Los recibos habilitan pruebas de deadline PoTR-Lite (SF-14). El esquema exacto se estabilizará
en una iteración posterior.

## 5. Comportamiento negativo

Los gateways DEBEN devolver códigos de error deterministas para rutas de rechazo:

| Condición                                               | Estado | Body                                                     |
|---------------------------------------------------------|--------|----------------------------------------------------------|
| Perfil de chunker no soportado                          | 406    | `{ "error": "unsupported_chunker", "handle": "..." }`    |
| Sobre de manifiesto faltante/inválido                   | 428    | `{ "error": "manifest_envelope_required" }`              |
| Manifiesto no admitido / registro de admisión no disponible | 412 | `{ "error": "provider_not_admitted" \| "admission_unavailable" }` |
| Falta identificador de proveedor para admisión/capability | 428  | `{ "error": "provider_id_missing" }`                     |
| Fallo de verificación de prueba                         | 422    | `{ "error": "proof_verification_failed" }`               |
| Intento de downgrade (headers faltantes)                | 428    | `{ "error": "required_headers_missing" }`                |
| Violación de rate limit / política GAR                  | 429    | `{ "error": "rate_limited", "reason": "..." }`           |
| Denylist (provider/manifest/CID/familia perceptual)     | 451    | `{ "error": "denylisted", "kind": "..." }`               |
| Errores internos                                        | 500    | `{ "error": "internal", "request_id": "..." }`           |

Los clientes DEBEN tratar cualquier respuesta no-2xx como rechazo y excluir el gateway
del schedule multi-source hasta que ocurra una revisión del operador.

### 5.1. Superficie de matriz de políticas (fixtures)

Los fixtures de conformidad en `fixtures/sorafs_gateway/1.0.0/scenarios.json`
ejercitan la matriz canónica de estados:

- Rutas de éxito: `200` (CAR completo) y `206` (replay de rango alineado).
- Enforcement de headers/manifiesto: `428` por sobres faltantes o headers requeridos
  (`B2`), `412` por fallos de admisión (`B5`).
- Enforcement GAR/denylist: `451` por rechazos de gobernanza o compliance (`D1`).

Los fixtures de rechazo por capacidad (`fixtures/sorafs_gateway/capability_refusal`) reflejan
los mismos códigos y los usan suites de SDK y auto-certificación de gateways para evitar drift.

## 6. Expectativas de telemetría

Los gateways DEBERÍAN emitir las siguientes métricas Prometheus y headers HTTP:

| Métrica / Header                     | Descripción                                        |
|--------------------------------------|----------------------------------------------------|
| `sorafs_gateway_requests_total{result}` | Buckets de éxito/error                          |
| `sorafs_gateway_range_bytes_total`      | Bytes servidos por handle de chunker           |
| `sorafs_gateway_proof_failures_total`   | Conteo de validaciones PoR/manifiesto fallidas |
| `sorafs_gateway_latency_ms_bucket`      | Histograma de latencia de requests              |
| `X-SoraFS-Telemetry-Nonce`              | Nonce de 16 bytes correlacionado con telemetría |

La telemetría NO DEBE filtrar información identificable de usuarios. Agrega estadísticas
por cliente usando identificadores seudónimos derivados del nonce de request.

## 7. Objetivos de conformidad

El QA Guild publicará un harness de replay que:

1. Reproduce fixtures canónicos (CAR completos y con rango) y verifica BLAKE3/PoR.
2. Emite requests negativas (chunker no soportado, prueba corrupta) y espera códigos de rechazo.
3. Lanza ≥1.000 streams de rango concurrentes con aleatoriedad con semilla para confirmar que
   los gateways mantienen throughput determinista e integridad de pruebas bajo carga.

Los gateways DEBEN cumplir el harness antes de onboarding. Los operadores auto-certificarán con
una atestación firmada que contenga el hash de la corrida de conformidad, recibos de prueba y
metadata de build del gateway.

## 8. Preguntas abiertas

* Finalizar el esquema de firma de recibos (Ed25519 vs claves multi-sig del consejo).
* Determinar si los endpoints chunk-range requieren capacidades GREASE.
* Alinear defaults de muestreo PoR con upgrades de SF-13 PDP.
* Evaluar factibilidad de soporte HTTP/3 dentro del mismo perfil o publicar un
  suplemento QUIC dedicado.

Las contribuciones y el feedback se rastrean vía la tarea SF-5a en `roadmap.md`.
