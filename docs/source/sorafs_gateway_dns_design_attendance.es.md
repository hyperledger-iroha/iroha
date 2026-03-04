---
lang: es
direction: ltr
source: docs/source/sorafs_gateway_dns_design_attendance.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 20e4dbd95067574ead4e1e9afc8875739e83813b2ff6b7b1e4850655906021c5
source_last_modified: "2025-11-20T07:16:03.636926+00:00"
translation_last_reviewed: "2026-01-30"
---

# Registro de asistencia

Reunión: **SoraFS Gateway & DNS Design Kickoff** (2025-03-03 @ 16:00 UTC)

## Resumen de estado

- Invitaciones enviadas el 2025-02-21 usando `docs/source/sorafs_gateway_dns_design_invite.txt`.
- Respuestas solicitadas antes del **2025-02-26**; el estado se actualizará conforme lleguen confirmaciones.
- Todas las confirmaciones se recibieron antes del **2025-02-26** y se registran aquí para que el workstream entre al kickoff con RSVP cerrado.

## Registro de respuestas

| Rol | Contacto | Estado | Notas / Seguimiento |
|------|---------|--------|---------------------|
| Networking TL (facilitador) | `networking.tl@soranet` | ✅ Confirmado 2025-02-21 | Dueño de la agenda, abrirá el bridge 10 min antes. |
| Ops Lead | `ops.lead@soranet` | ✅ Confirmado 2025-02-23 | Cubrirá el runbook de despliegue; pidió el preread deck. |
| Storage Team Rep | `storage.rep@sorafs` | ✅ Confirmado 2025-02-21 | Traerá el estado actualizado de fixtures de chunker. |
| Tooling WG Rep | `tooling.wg@sorafs` | ✅ Confirmado 2025-02-21 | Preparando actualizaciones del harness de conformidad. |
| Governance Liaison | `governance@sora` | ✅ Confirmado (delegado) 2025-02-24 | Delegado `governance.alt@sora`; el titular está OOO. |
| QA Guild Lead | `qa.guild@sorafs` | ✅ Confirmado 2025-02-21 | Necesita snapshot de telemetría previo a la reunión. |
| Docs/DevRel Observer | `docs.devrel@sora` | ✅ Confirmado 2025-02-21 | Redactando doc de notas compartidas. |
| Torii Platform Rep | `torii.platform@soranet` | ✅ Confirmado 2025-02-21 | Recolectando export de métricas GAR. |
| Security Engineering Observer | `security@soranet` | ✅ Confirmado 2025-02-25 | Slide deck compartido 2025-02-24; asistirá en remoto. |

## Responsables de automatización DNS

El ítem del roadmap **Decentralized DNS & Gateway** exige nombrar owners de automatización antes del kickoff. La tabla abajo registra alcance, lead responsable y backup para que las tareas de seguimiento mapeen directo a código/activos en el repositorio.

| Alcance | Owner primario | Backup | Responsabilidades |
|--------|----------------|--------|------------------|
| Automatización de zonefile SoraDNS y pinning GAR | Ops Lead (`ops.lead@soranet`) | Networking TL (`networking.tl@soranet`) | Mantener la automatización en `tools/soradns-resolver/`, publicar los esqueletos de zonefile firmados documentados en `docs/source/sns/governance_playbook.md` y rotar datos SPKI/GAR antes de cada cutover. |
| Alias de gateway y metadatos de cutover SoraFS | Tooling WG Rep (`tooling.wg@sorafs`) | Docs/DevRel (`docs.devrel@sora`) | Operar `docs/portal/scripts/sorafs-pin-release.sh` + `docs/portal/scripts/generate-dns-cutover-plan.mjs` (y sus tests en `docs/portal/scripts/__tests__/dns-cutover-plan.test.mjs`), adjuntar los manifiestos generados a `docs/portal/docs/devportal/deploy-guide.md`, y difundir cambios de alias al registro de pines. |
| Snapshots de telemetría y automatización de rollback | QA Guild Lead (`qa.guild@sorafs`) | Security Engineering (`security@soranet`) | Recolectar métricas GAR (`docs/source/sorafs_gateway_dns_design_gar_telemetry.md`, `docs/source/sorafs_gateway_dns_design_metrics_*.prom`), mantener alertas conectadas al dashboard de kickoff, y ensayar el flujo de rollback junto al observador de seguridad antes de GA. |

## Acciones de seguimiento

1. **Distribución del deck** — enviar el slide deck final a todos los asistentes antes del 2025-02-27 (Networking TL).
2. **Brief de delegados** — proveer a la delegación de governance el apéndice de política DNS antes de la sesión (Docs/DevRel).
3. **Logística de grabación** — Docs/DevRel preparará doc de notas compartidas y checklist de grabación.
4. **Sync de owners de automatización** — los owners anteriores confirmarán checkpoints del runbook (publicación de zonefile, promoción de alias, muestreo de telemetría) antes del 2025-03-04 para cerrar el kickoff con accountability clara.
5. **Owner runbook** — el SOP compartido vive en `docs/source/sorafs_gateway_dns_owner_runbook.md`; circularlo con el deck y referenciarlo al seguir el estado de ensayos.
6. **Confirmación de cierre** — todas las acciones anteriores se completaron y se registraron en `docs/source/sorafs_gateway_dns_design_minutes.md` bajo la entrada de cierre 2025-03-04.

## Calendario y artefactos

- Invitación de calendario enviada el 2025-02-25 vía Google Calendar. Enlace del evento:
  `https://calendar.google.com/calendar/event?eid=c29yYWZzLWdhdGV3YXktZG5zLTIwMjUwMzAz` (interno SoraNet).
- Doc de notas compartidas: `docs/source/sorafs_gateway_dns_design_agenda.md` (agregar registro de acciones después de la sesión).
