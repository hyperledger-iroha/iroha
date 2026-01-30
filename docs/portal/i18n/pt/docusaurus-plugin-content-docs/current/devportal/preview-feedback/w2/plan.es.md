---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w2/plan.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: preview-feedback-w2-plan
title: Plan de intake comunitario W2
sidebar_label: Plan W2
description: Intake, aprobaciones y checklist de evidencia para la cohorte de preview comunitaria.
---

| Item | Detalles |
| --- | --- |
| Ola | W2 - Reviewers comunitarios |
| Ventana objetivo | Q3 2025 semana 1 (tentativa) |
| Tag de artefacto (planeado) | `preview-2025-06-15` |
| Issue del tracker | `DOCS-SORA-Preview-W2` |

## Objetivos

1. Definir criterios de intake comunitario y flujo de vetting.
2. Obtener aprobacion de gobernanza para el roster propuesto y el addendum de uso aceptable.
3. Refrescar el artefacto de preview verificado por checksum y el bundle de telemetria para la nueva ventana.
4. Preparar el proxy Try it y los dashboards antes del envio de invitaciones.

## Desglose de tareas

| ID | Tarea | Responsable | Fecha limite | Estado | Notas |
| --- | --- | --- | --- | --- | --- |
| W2-P1 | Redactar criterios de intake comunitario (elegibilidad, max slots, requisitos de CoC) y circular a gobernanza | Docs/DevRel lead | 2025-05-15 | Completado | La politica de intake se fusiono en `DOCS-SORA-Preview-W2` y se respaldo en la reunion del consejo 2025-05-20. |
| W2-P2 | Actualizar template de solicitud con preguntas especificas de comunidad (motivacion, disponibilidad, necesidades de localizacion) | Docs-core-01 | 2025-05-18 | Completado | `docs/examples/docs_preview_request_template.md` ahora incluye la seccion Community, referenciada en el formulario de intake. |
| W2-P3 | Asegurar aprobacion de gobernanza para el plan de intake (voto en reunion + actas registradas) | Governance liaison | 2025-05-22 | Completado | Voto aprobado por unanimidad el 2025-05-20; actas y roll call enlazados en `DOCS-SORA-Preview-W2`. |
| W2-P4 | Programar staging del proxy Try it + captura de telemetria para la ventana W2 (`preview-2025-06-15`) | Docs/DevRel + Ops | 2025-06-05 | Completado | Ticket de cambio `OPS-TRYIT-188` aprobado y ejecutado 2025-06-09 02:00-04:00 UTC; screenshots de Grafana archivados con el ticket. |
| W2-P5 | Construir/verificar nuevo tag de artefacto de preview (`preview-2025-06-15`) y archivar descriptor/checksum/probe logs | Portal TL | 2025-06-07 | Completado | `scripts/preview_wave_preflight.sh --tag preview-2025-06-15 ...` se ejecuto 2025-06-10; outputs guardados bajo `artifacts/docs_preview/W2/preview-2025-06-15/`. |
| W2-P6 | Armar roster de invitaciones comunitarias (<=25 reviewers, lotes escalonados) con info de contacto aprobada por gobernanza | Community manager | 2025-06-10 | Completado | Primer cohorte de 8 reviewers comunitarios aprobado; IDs de solicitud `DOCS-SORA-Preview-REQ-C01...C08` registrados en el tracker. |

## Checklist de evidencia

- [x] Registro de aprobacion de gobernanza (notas de reunion + link de voto) adjunto a `DOCS-SORA-Preview-W2`.
- [x] Template de solicitud actualizado commiteado bajo `docs/examples/`.
- [x] Descriptor `preview-2025-06-15`, log de checksum, probe output, link report y transcript del proxy Try it guardados bajo `artifacts/docs_preview/W2/`.
- [x] Screenshots de Grafana (`docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals`) capturados para la ventana de preflight W2.
- [x] Tabla de roster de invitaciones con IDs de reviewers, tickets de solicitud y timestamps de aprobacion completados antes del envio (ver seccion W2 del tracker).

Mantener este plan actualizado; el tracker lo referencia para que el roadmap DOCS-SORA vea exactamente lo que resta antes de que salgan las invitaciones W2.
