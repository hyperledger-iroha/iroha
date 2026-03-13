---
lang: es
direction: ltr
source: docs/source/sorafs_gateway_refusal_guidance.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9ac09d0d75d38ac86591c4773a3a7f1980ed94f3d297b17b2a3190512ce874c1
source_last_modified: "2025-11-02T04:40:39.847639+00:00"
translation_last_reviewed: "2026-01-30"
---

# Guia de rechazos de capacidades del gateway SoraFS

Este documento cumple el entregable del item SF-5c del roadmap “Publicar guia para operadores/devs” documentando como los gateways SoraFS, SDKs y tooling downstream deben detectar, exponer y remediar rutas de rechazo que protegen la entrega trustless. La guia complementa la matriz de pruebas negativas en `docs/source/sorafs_gateway_capability_tests.md` y el borrador del kit de auto-certificacion en `docs/source/sorafs_gateway_self_cert.md`.

## Audiencia y alcance

- **Operadores** que ejecutan gateways u orquestadores y deben mantener el comportamiento de rechazo determinista, observable y auditado.
- **Desarrolladores** que mantienen SDKs/CLIs o integran endpoints SoraFS en aplicaciones downstream.
- **QA/DevRel** que cablean suites de conformidad y workflows de onboarding que ejercitan rutas de rechazo antes del despliegue.

El catalogo siguiente enumera los motivos de rechazo que DEBEN permanecer estables en la superficie API. Nuevos motivos de rechazo requieren un update del roadmap y fixtures correspondientes antes de llegar a produccion.

## Catalogo de rechazos

El gateway DEBE codificar rechazos como cuerpos JSON que cumplen el esquema descrito en `docs/source/sorafs_gateway_capability_tests.md` y reutilizar el campo `error` como etiqueta de telemetria. Las filas de la tabla tambien representan la expectativa minima de compliance.

| ID | HTTP | codigo `error` | Etiqueta(s) de telemetria | Remediacion primaria | Referencia cruzada |
|----|------|----------------|---------------------------|----------------------|--------------------|
| C1 | 406 | `unsupported_chunker` | `unsupported_chunker` | Alinear handle de chunker con la lista de perfiles SF-1; refrescar alias del manifiesto. | Docs del registro de chunker, entregables SF-1. |
| C2 | 412 | `manifest_variant_missing` | `manifest_variant_missing`, `alias_stale` | Publicar manifiesto sucesor o fijar alias al perfil solicitado; verificar snapshot del registro. | Plan de pin registry, politica de alias. |
| C3 | 412 | `admission_envelope_mismatch` | `governance_violation`, `alias_stale` | Regenerar sobre de admision via Torii; confirmar firmas de governance. | `docs/source/sorafs/pin_registry_validation_plan.md`. |
| C4 | 428 | `capability_grease_unsupported` | `capability_refusal` | Rechazar TLVs desconocidos salvo permiso explicito; asegurar que el cliente nego capacidades soportadas. | Draft del RFC Capability GREASE. |
| C5 | 428 | `missing_dag_scope` | `capability_refusal` | Agregar header `Sora-Dag-Scope` o actualizar cliente; configurar defaults del orquestador. | Spec de chunk-range (SF-5d). |
| C6 | 406 | `unsupported_encoding` | `unsupported_encoding` | Eliminar requests `Accept-Encoding` o habilitar compresion en la politica del manifiesto. | Docs de politica de manifiestos. |
| C7 | 422 | `proof_validation_failed` | `proof_validation_failed` | Verificar pruebas PoR/PoTR; inspeccionar output del replay harness para chunks corruptos. | Guia del replay harness. |

Los gateways PUEDEN extender la tabla con etiquetas de telemetria adicionales (p. ej., `profile=sf1`, `provider_id=aa12`) pero DEBEN conservar las etiquetas base listadas para que dashboards y alertas sigan siendo portables entre despliegues.

## Playbooks de troubleshooting

### Chunker no soportado (`C1`)

1. Inspeccionar la entrada de log del request para el hint `chunker_handle` emitido por Torii.
2. Comparar con la lista de perfiles del manifiesto servida por el Pin Registry (`/v2/sorafs/pin/<cid>`). El manifiesto DEBE incluir un chunker que coincida con el request.
3. Si el manifiesto falta, publicar un manifiesto sucesor via `sorafs-manifest submit` y esperar aprobacion de governance.
4. Confirmar que la cache del gateway refresco dentro de `alias_refresh_window` (ver `docs/source/sorafs_alias_policy.md`). Si la cache excedio `alias_hard_expiry`, evacuar localmente y re-solicitar.

### Variante de manifiesto ausente (`C2`)

1. Usar `sorafs-manifest inspect` para confirmar que el perfil solicitado existe.
2. Si el alias apunta a un manifiesto sin el perfil, enviar un manifiesto sucesor y disparar ordenes de replicacion.
3. Verificar que los registros emiten eventos SSE `alias-proof-updated` y que el gateway se suscribio correctamente. Eventos faltantes suelen indicar drift de TLS o auth.

### Mismatch de sobre de admision (`C3`)

1. Ejecutar `sorafs-manifest attest --cid <cid>` para regenerar un sobre de admision.
2. Revisar que el digest del payload Norito coincida con el CID del manifiesto; mismatches indican logs de governance obsoletos.
3. Confirmar cada firma del council con `sorafs-manifest verify`, asegurando que las llaves publicas coinciden con la ultima rotacion.
4. Si el sobre pertenece a una epoch de governance revocada, los operadores deben obtener un snapshot mas nuevo antes de que el gateway acepte el manifiesto.

### Capability TLV GREASE / Missing Dag Scope (`C4`, `C5`)

1. Capturar el request HTTP para asegurar que el cliente anuncia una lista de capacidades.
2. Si aparecen TLVs GREASE, confirmar que el cliente habilito el preview flag listado en el RFC GREASE; de lo contrario, indicar al cliente que alinee con la tabla de capacidades del manifiesto publicada.
3. Para `Sora-Dag-Scope` faltante, actualizar la configuracion del orquestador o SDK para inyectar el header automaticamente; fallar aqui indica regresion del SDK.

### Encoding no soportado (`C6`)

1. Inspeccionar el `StreamEncodingPolicy` del manifiesto en el snapshot del Pin Registry.
2. Si la compresion esta deshabilitada, considerar habilitar la capacidad `compression` y publicar un manifiesto sucesor. De lo contrario, eliminar `Accept-Encoding` de los requests del cliente o negociar deflate/gzip explicitamente.

### Validacion de prueba fallida (`C7`)

1. Ejecutar el replay harness (`cargo test -p integration_tests sorafs_gateway_conformance`) contra el gateway; asegurar que el fixture negativo reproduce la falla.
2. Inspeccionar el transcript PoR en `fixtures/sorafs_gateway/conformance/proofs/*.norito` para detectar tampering.
3. Si la corrupcion se origino upstream, poner en cuarentena al provider y rotar el alias del manifiesto hacia un replica sana. Registrar IDs de incidente en los logs de governance.

## Responsabilidades de desarrollo

- Los SDKs DEBEN exponer las respuestas de rechazo verbatim, incluyendo `error`, `reason` y `details`, y evitar convertir fallas en errores de transporte.
- Proveer enums de errores tipados que reflejen el catalogo anterior y asegurar que los retries traten `4xx` como rechazos deterministas (sin exponential backoff).
- Cuando los SDKs exponen fallback automatico, solo reintentar contra un provider diferente si el motivo es `unsupported_chunker` y existe un perfil compatible.
- Las librerias cliente DEBEN loguear eventos de rechazo con las mismas etiquetas de telemetria para que los operadores correlacionen tendencias de gateway y cliente.
- Los tests de integracion DEBERIAN importar los fixtures canonicos referenciados en el kit de self-cert para evitar drift entre comportamiento del SDK y del gateway.

## Compliance y monitoreo

- Los gateways DEBEN exponer `sorafs_gateway_refusals_total` con al menos la etiqueta `reason` y DEBERIAN incluir `profile`, `provider_id` y `scope`.
- Alertas:
  - Disparar alerta `critical` si las tasas de `unsupported_chunker` superan `threshold=25/minute` durante cinco minutos.
  - Disparar alerta `warning` si los rechazos totales exceden la media movil de una hora por mas de dos desviaciones estandar.
- Los logs DEBEN redactar bearer tokens y secretos de sesion. Retener logs de rechazo por al menos 30 dias para auditorias de governance.
- Los operadores DEBEN documentar rechazos en runbooks de incidentes cuando un tipo de rechazo persiste mas de 15 minutos o impacta mas del 5% del trafico.
- Asegurar que los pipelines de CI ejecuten la suite de rechazos del replay harness antes del despliegue; las fallas bloquean promociones.

## Checklist de integracion

Antes de hacer onboarding de un gateway o release de SDK, confirmar:

1. Los escenarios negativos del replay harness pasan con status codes deterministas.
2. El reporte de `self-cert` incluye conteos de rechazo con etiquetas de telemetria coincidentes.
3. Los dashboards grafican contadores de rechazo con baselines y alertas configuradas.
4. Los runbooks de operadores referencian este documento para pasos de remediacion.
5. Los developers actualizaron las notas de release del SDK resumiendo cualquier manejo nuevo de rechazos.

## Gestion de cambios

- Cualquier modificacion a codigos de rechazo o estados HTTP requiere:
  1. Actualizacion de esta guia y la matriz de pruebas.
  2. Regeneracion de fixtures y cobertura del replay harness.
  3. Aprobacion del council de governance con manifiesto firmado.
- Mantener actualizado el item del roadmap SF-5c cuando nuevos rechazos pasan de draft a produccion para evitar drift entre docs, tests y telemetria.
