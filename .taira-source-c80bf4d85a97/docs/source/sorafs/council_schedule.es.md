---
lang: es
direction: ltr
source: docs/source/sorafs/council_schedule.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b60a5b12176a4fe7c213ae98f38732a103cde8bb656dba3be033d3d6e7748799
source_last_modified: "2025-11-18T19:11:17.016918+00:00"
translation_last_reviewed: "2026-01-30"
---

# Calendario y minutas del Council de Registro SoraFS

Las barandillas del roadmap para SF-2/SF-6 requieren que el council del registro
mantenga visibles publicamente su cadencia de reuniones, agendas y rastro de
 evidencia. Este documento es la referencia canonica de calendario para el
programa Storage/Governance y enlaza directamente a los archivos de minutas
(`docs/source/sorafs/council_minutes_*.md`) mas los buckets de artefactos que
respaldan cada decision.

## Cadencia y contactos

- **Frecuencia:** Primer y tercer miercoles de cada mes a las **15:00 UTC**.
  Cuando una sesion coincide con un feriado de governance, la Secretaria publica
  el horario ajustado aqui a mas tardar en la reunion previa.
- **Ubicacion:** Puente de governance Sora Nexus (video) con log IRC simultaneo,
  ambos capturados en la carpeta correspondiente `artifacts/council_minutes/<date>/`.
- **Chair y Secretaria:** Chair rotativo del council; la Secretaria de Governance
  maneja la circulacion de agenda, recoleccion de evidencia y publicacion de
  minutas dentro de 48 horas.
- **Lista de distribucion:** `council@governance.dataspace.foundation` (decision makers),
  `storage-wg@hyperledger.org` (observadores del Storage Team),
  `sdk-program@hyperledger.org` (notificaciones de paridad SDK).

## Sesiones proximas (Q2 2026)

| Fecha (UTC) | Enfoque y entregables | Pre-read / Inputs | Owner de evidencia | Notas |
|-------------|-----------------------|-------------------|--------------------|-------|
| 2026-04-08 15:00 | Promover perfil de chunker `0x29` y reseal de entradas del DAG de governance de manifiestos. Decisiones incluyen aprobar el sobre de perfil firmado y gatear el rollout de `ChunkerProfile::Digest`. | `docs/source/sorafs/chunker_registry.md`<br>`docs/source/sorafs/manifest_pipeline.md`<br>`fixtures/sorafs_chunker/manifest_signatures.json` | Storage Team · Governance Secretariat | La Secretaria debe pinnear el DAG resealado bajo `artifacts/sorafs/chunker_registry/2026-04-08/` antes del voto de ratificacion. |
| 2026-04-22 15:00 | Apelaciones trimestrales de admision de providers + refresh de baseline de telemetria GAR. Entregables: voto del slate de apelaciones, changelog de runbook GAR y follow-ups de disputas. | `docs/source/sorafs/provider_admission_policy.md`<br>`docs/source/sorafs/dispute_revocation_runbook.md`<br>`dashboards/grafana/sorafs_gateway_dns.json` snapshot | Governance Secretariat · Observability | Tarball de evidencia de apelaciones guardado en `artifacts/council_minutes/2026-04-22/appeals_bundle.zip` con digest blake3 registrado en las minutas. |
| 2026-05-06 15:00 | Guardrails del marketplace de capacidad + auditoria de contadores PoR. Entregables: ACK de guia de operadores, update del anexo KPI del marketplace y notas del playbook de respuesta a desviaciones PoR. | `docs/source/sorafs/storage_capacity_marketplace.md`<br>`docs/source/sorafs/reports/capacity_marketplace_validation.md`<br>`docs/source/sorafs/dispute_revocation_runbook.md` | Storage WG · Treasury Guild liaison | Se espera workshop de follow-up para telemetria PoR; capturar roster de asistentes + preguntas en el addendum de minutas. |

La Secretaria actualiza esta tabla inmediatamente despues de cada voto para que
los equipos downstream se suscriban a los folders de artefactos correctos y a
los bundles de pre-read.

## Archivo de minutas

| Fecha | Minutas | Highlights de decisiones | Evidence bucket |
|------|---------|--------------------------|-----------------|
| 2025-10-29 | [`council_minutes_2025-10-29.md`](council_minutes_2025-10-29.md) | Ratifico el RFC de arquitectura SF-1, confirmo el plumbing de governance de manifiestos/chunker y publico evidencia de enforcement de fixtures en CI. | `artifacts/council_minutes/2025-10-29/` |
| 2025-03-05 | [`council_minutes_2025-03-05.md`](council_minutes_2025-03-05.md) | Registro el bundle de ACK del priority snapshot, el digest del ensayo Gateway/DNS y el action tracker para SF-3/SF-6b/SF-9. | `artifacts/council_minutes/2025-03-05/` |

Para cada nueva reunion:

1. Copiar la plantilla de abajo en `docs/source/sorafs/council_minutes_<YYYY-MM-DD>.md`
   y localizar segun se requiera (`.ar.md`, `.es.md`, etc.).
2. Exportar un PDF de las minutas llenas y guardarlo junto al bundle de evidencia.
3. Actualizar la tabla de archivo y referencias en roadmap/status con el nuevo link.

## Snippet de plantilla de minutas

````markdown
---
title: SoraFS Registry Council Minutes — <YYYY-MM-DD>
summary: Topics, evidence, and decisions for the <focus> session.
---

## Attendees
- Chair:
- Storage WG:
- Governance Secretariat:
- Observers:

## Agenda
1. <item>
2. <item>

## Evidence Digest
- Bundle path: `artifacts/council_minutes/<YYYY-MM-DD>/<bundle>.tar.zst`
- BLAKE3-256: `<hex>`
- Uploaded by: `<name>` at `<timestamp>`

## Decisions
1. _Decision text_

## Action Items
| Item | Owner | Due | Notes |
|------|-------|-----|-------|
| Example | Governance Secretariat | <date> | Publish minutes PDF + artefact hash |
````

## Workflow de publicacion

1. **Circular agenda:** al menos 72 horas antes de la sesion, enviar la agenda
   por e-mail (incluir links arriba) y actualizar la tabla de Sesiones proximas.
2. **Recolectar artefactos:** guardar logs de CLI, exports de Grafana y votos del
   council en `artifacts/council_minutes/<date>/` con digests BLAKE3 registrados.
   Referenciar `docs/source/sorafs/dispute_revocation_runbook.md` §2 para pasos
   de archivado.
3. **Durante la sesion:** editar en vivo el archivo de minutas en este
   directorio y capturar asistencia (nombres + roles). Usar la tabla de evidencia
   de la plantilla para registrar hashes del bundle conforme se leen.
4. **Post-sesion:** dentro de 48 horas, mergear el archivo de minutas, agregar
   copias localizadas si aplica, subir el PDF y actualizar la tabla de archivo.
5. **Updates de status/roadmap:** enlazar las nuevas minutas desde `status.md`
   (Latest Updates) y anotar los entries relevantes del roadmap para que los
   auditores puedan rastrear la decision a una reunion especifica.
