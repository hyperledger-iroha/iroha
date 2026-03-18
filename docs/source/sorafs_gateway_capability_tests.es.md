---
lang: es
direction: ltr
source: docs/source/sorafs_gateway_capability_tests.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: aa3811e4524172204563b055e09c87c16846d8e354489f8e4690af0b3a9afaaa
source_last_modified: "2025-11-02T15:27:48.918601+00:00"
translation_last_reviewed: "2026-01-30"
---

# Pruebas de rechazo por capacidades del gateway SoraFS

El suite de conformidad del gateway (`integration_tests/tests/sorafs_gateway_conformance.rs`) consume los fixtures canonicos publicados en `fixtures/sorafs_gateway/capability_refusal/`. Cada caso mapea a un codigo `error` estable, status HTTP y etiqueta de telemetria (`torii_sorafs_gateway_refusals_total{reason,profile,provider_id,scope}`) para que los SDKs y los operadores vean un comportamiento consistente.

## Matriz de pruebas (modulo del harness `capability_refusal`)

| ID | Escenario | Status | Codigo de error | Notas |
|----|-----------|--------|-----------------|-------|
| C1 | Chunker handle no soportado | 406 | `unsupported_chunker` | El gateway rechaza manifiestos cuyo `chunk_profile_handle` no esta habilitado. |
| C2 | Variante de manifiesto ausente (alias inexistente) | 412 | `manifest_variant_missing` | Requests de rango con alias `Sora-Name` no ligados al sobre de manifiesto se rechazan; `details.alias` refleja el alias solicitado. |
| C3 | Mismatch de sobre de admision | 412 | `admission_mismatch` | El digest del manifiesto no esta cubierto por el sobre de governance. |
| C4 | Capability TLV desconocida | 428 | `unsupported_capability` | Capability GREASE rechazada cuando `allow_unknown_capabilities=false`. |
| C5 | Request de rango sin header `dag-scope` | 428 | `missing_header` | Hace cumplir el contrato trustless `Accept: …; dag-scope=block`; `details.header` es `Sora-Dag-Scope`. |
| C6 | Request gzip cuando el perfil no permite compresion | 406 | `unsupported_encoding` | El harness envia `Accept-Encoding: gzip`; `details.encoding` registra el encoding no soportado. |
| C7 | Manipulacion de prueba (mismatch de digest de chunk) | 422 | `proof_mismatch` | El gateway rechaza pruebas PoR cuyo digest de chunk es reemplazado con el sentinel de tamper `0xff`; `details.section` identifica la muestra ofensora. |

Los fixtures viven bajo `fixtures/sorafs_gateway/capability_refusal/` y son referenciados por los integration tests. Cada escenario expone `request.json`, `response.json` y `gateway.json` junto al indice compartido `scenarios.json`, para que operadores y autores de SDK puedan reproducir el rechazo end-to-end usando el bundle canonico de fixtures en `fixtures/sorafs_gateway/1.0.0/`.

> **Contratos de headers:** `Sora-Name` lleva el alias solicitado, `Accept` debe incluir `dag-scope=block` cuando se piden rangos trustless CAR, y `Accept-Encoding` debe omitir `gzip` para perfiles que no anuncian soporte de compresion.

## Contrato de respuesta

Los gateways emiten cuerpos JSON con la siguiente forma:

```json
{
  "error": "unsupported_chunker",
  "reason": "profile sorafs.sf1@1.0.0 is not enabled on this gateway",
  "details": {
    "request_id": "uuid",
    "manifest_cid": "bafkrei…",
    "hint": "check alias binding"
  }
}
```

- `error` (string, requerido) — codigo de maquina, coincide con la etiqueta `reason` de telemetria.
- `reason` (string, requerido) — explicacion orientada al operador.
- `details` (object, opcional) — contexto adicional (`request_id`, `manifest_cid`, `provider_id`, `hint`, etc.). Las claves son opcionales y se omiten cuando no aplican.
- El harness de conformidad persiste el mismo payload bajo el objeto `refusal` de cada escenario dentro del JSON de atestacion (`suite.scenarios[].refusal`), incluyendo el status HTTP para checks de paridad.

**Requisitos del gateway**

- El status HTTP debe coincidir con la tabla anterior.
- La respuesta debe incluir `Content-Type: application/json`.
- El header `X-SoraFS-Request-Id` refleja `details.request_id` cuando este presente.

## Telemetria y logging

- **Counter:** `torii_sorafs_gateway_refusals_total{reason,profile,provider_id,scope}` incrementa una vez por request rechazado (provider id es string vacia cuando el caller no lo suministro).
- **Log estructurado:** emitir entradas con `error`, `reason`, `status_code`, `remote_addr`, `request_duration_ms` y cualquier `details.*`. No loguear stream tokens ni otros secretos.
- **Dashboards/Alertas:** graficar conteos de rechazo por `reason`; alertar sobre picos (p. ej., `unsupported_chunker` durante rollout) usando ventanas de 5 minutos o deteccion de anomalias.

## Expectativas para SDK

- Las librerias cliente (Rust, TypeScript, Go, Swift, etc.) deben parsear el JSON y exponer `error` + `reason` al caller.
- Tests unit/integration en cada SDK deben simular la matriz anterior usando el loader compartido (`integration_tests/src/sorafs_gateway_capability_refusal.rs`) para mantener paridad de fixtures con Torii.

## Relacion con la auto-certificacion

El handbook de auto-cert (`docs/source/sorafs_gateway_self_cert.md`) y la guia de despliegue referencian los IDs de rechazo al guiar a operadores en troubleshooting. El workflow `sorafs_fetch --gateway-provider …` puede usarse para reproducir escenarios individuales para documentacion o entrenamiento.
