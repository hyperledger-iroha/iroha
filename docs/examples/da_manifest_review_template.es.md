---
lang: es
direction: ltr
source: docs/examples/da_manifest_review_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c5c959bd6654d095d2b3785a02e9c2ec162e699ad985b342760b952e38766a66
source_last_modified: "2025-11-12T19:46:29.811940+00:00"
translation_last_reviewed: 2026-01-01
---

# Paquete de governance para manifiesto de disponibilidad de datos (Plantilla)

Usa esta plantilla cuando paneles del Parlamento revisen manifiestos de DA para subsidios,
takedowns o cambios de retencion (roadmap DA-10). Copia el Markdown en el ticket de
governance, completa los placeholders y adjunta el archivo completado junto con los
payloads Norito firmados y los artefactos de CI referenciados abajo.

```markdown
## Metadatos del manifiesto
- Nombre / version del manifiesto: <string>
- Clase de blob y tag de governance: <taikai_segment / da.taikai.live>
- Digest BLAKE3 (hex): `<digest>`
- Hash de payload Norito (opcional): `<digest>`
- Envelope de origen / URL: <https://.../manifest_signatures.json>
- ID de snapshot de politica Torii: `<unix timestamp or git sha>`

## Verificacion de firmas
- Fuente de descarga del manifiesto / ticket de storage: `<hex>`
- Comando/salida de verificacion: `cargo run -p sorafs_car --bin sorafs-manifest-stub -- manifest.json --manifest-signatures-in=manifest_signatures.json` (fragmento de log adjunto?)
- `manifest_blake3` reportado por la herramienta: `<digest>`
- `chunk_digest_sha3_256` reportado por la herramienta: `<digest>`
- Multihashes del firmante del consejo:
  - `<did:...>` / `<ed25519 multihash>`
- Marca de tiempo de verificacion (UTC): `<2026-02-20T11:04:33Z>`

## Verificacion de retencion
| Campo | Esperado (politica) | Observado (manifiesto) | Evidencia |
|-------|---------------------|------------------------|----------|
| Retencion hot (segundos) | <p. ej., 86400> | <valor> | `<torii.da_ingest.replication_policy dump | CI link>` |
| Retencion cold (segundos) | <p. ej., 1209600> | <valor> |  |
| Replicas requeridas | <valor> | <valor> |  |
| Clase de almacenamiento | <hot / warm / cold> | <valor> |  |
| Tag de governance | <da.taikai.live> | <valor> |  |

## Contexto
- Tipo de solicitud: <Subsidio | Takedown | Rotacion de manifiesto | Congelamiento de emergencia>
- Ticket de origen / referencia de compliance: <link o ID>
- Impacto en subsidio / renta: <cambio XOR esperado o "n/a">
- Link de apelacion de moderacion (si aplica): <case_id o link>

## Resumen de decision
- Panel: <Infraestructura | Moderacion | Tesoreria>
- Resultado de votacion: `<for>/<against>/<abstain>` (quorum `<threshold>` cumplido?)
- Altura o ventana de activacion / rollback: `<block/slot range>`
- Acciones de seguimiento:
  - [ ] Notificar a Tesoreria / ops de renta
  - [ ] Actualizar reporte de transparencia (`TransparencyReportV1`)
  - [ ] Programar auditoria de buffer

## Escalamiento y reporte
- Ruta de escalamiento: <Subsidio | Compliance | Congelamiento de emergencia>
- Link / ID del reporte de transparencia (si se actualizo): <`TransparencyReportV1` CID>
- Bundle de proof-token o referencia ComplianceUpdate: <ruta o ticket ID>
- Delta de ledger de renta / reserva (si aplica): <`ReserveSummaryV1` snapshot link>
- URL(s) de snapshot de telemetria: <Grafana permalink o artefact ID>
- Notas para las actas del Parlamento: <resumen de plazos / obligaciones>

## Adjuntos
- [ ] Manifiesto Norito firmado (`.to`)
- [ ] Resumen JSON / artefacto de CI que pruebe valores de retencion
- [ ] Proof token o paquete de compliance (para takedowns)
- [ ] Snapshot de telemetria de buffer (`iroha_settlement_buffer_xor`)
```

Archiva cada paquete completado bajo la entrada del DAG de Governance para la votacion para que
revisiones posteriores puedan referenciar el digest del manifiesto sin repetir la ceremonia completa.
