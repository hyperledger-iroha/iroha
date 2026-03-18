---
lang: es
direction: ltr
source: docs/examples/android_device_lab_request.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8a8e6a4981a11faac56d9b04432773e94fd59f8e2524fa4c552be459291c7c39
source_last_modified: "2025-11-12T08:31:44.643013+00:00"
translation_last_reviewed: 2026-01-01
---

# Plantilla de solicitud de reserva del laboratorio de dispositivos Android

Copia esta plantilla en la cola de Jira `_android-device-lab` al reservar hardware. Adjunta
links a pipelines de Buildkite, artefactos de compliance y cualquier ticket de partner que
dependa del run.

```
Resumen: <Hito / carga> - <lane(s)> - <fecha/hora UTC>

Hito / Seguimiento:
- Item de roadmap: AND6 / AND7 / AND8 (elige)
- Ticket(s) relacionados: <link al issue ANDx>, <referencia partner-sla si aplica>

Solicitante / Contacto:
- Ingeniero principal:
- Ingeniero de respaldo:
- Canal de Slack / escalado de pager:

Detalles de reserva:
- Lanes requeridas: <pixel8pro-strongbox-a / pixel8a-ci-b / pixel7-fallback / firebase-burst / strongbox-external>
- Slot deseado: <YYYY-MM-DD HH:MM UTC> por <duracion>
- Tipo de carga: <CI smoke / attestation sweep / chaos rehearsal / partner demo>
- Tooling a ejecutar: <nombres de jobs/scripts en Buildkite>
- Artefactos producidos: <logs, bundles de attestation, dashboards>

Dependencias:
- Referencia de snapshot de capacidad: link a `android_strongbox_capture_status.md`
- Filas de matriz de readiness tocadas: link a `android_strongbox_device_matrix.md`
- Vinculo de compliance (si aplica): fila de checklist AND6, ID de log de evidencia

Plan de fallback:
- Si el slot primario no esta disponible, el slot alterno es:
- Necesita pool de fallback / Firebase? (si/no)
- Retainer externo de StrongBox requerido? (si/no - incluir lead time)

Aprobaciones:
- Lider del Hardware Lab:
- TL de Android Foundations (cuando impacta lanes de CI):
- Lider de programa (si se invoca StrongBox retainer):

Checklist post-run:
- Adjuntar URL(s) de Buildkite:
- Actualizar fila del log de evidencia: <ID/fecha>
- Anotar desviaciones/sobrecostos:
```
