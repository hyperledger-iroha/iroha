---
lang: es
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w2/plan.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: vista previa-comentarios-w2-plan
título: Plan de captación comunitario W2
sidebar_label: Plan W2
descripción: Admisión, aprobaciones y lista de verificación de evidencia para la cohorte de vista previa comunitaria.
---

| Artículo | Detalles |
| --- | --- |
| ola | W2 - Revisores comunitarios |
| Ventana objetivo | T3 2025 semana 1 (tentativa) |
| Etiqueta de artefacto (planeado) | `preview-2025-06-15` |
| Problema del rastreador | `DOCS-SORA-Preview-W2` |

## Objetivos

1. Definir criterios de ingesta comunitaria y flujo de vetting.
2. Obtener aprobación de gobernanza para el roster propuesto y el addendum de uso aceptable.
3. Refrescar el artefacto de vista previa verificado por checksum y el paquete de telemetría para la nueva ventana.
4. Preparar el proxy Pruébalo y los paneles antes del envío de invitaciones.

## Desglose de tareas| identificación | Tarea | Responsable | Fecha límite | Estado | Notas |
| --- | --- | --- | --- | --- | --- |
| G2-P1 | Redactar criterios de ingesta comunitaria (elegibilidad, max slots, requisitos de CoC) y circular a gobernanza | Líder de Docs/DevRel | 2025-05-15 | Completado | La política de ingesta se fusiono en `DOCS-SORA-Preview-W2` y se respalda en la reunión del consejo 2025-05-20. |
| W2-P2 | Actualizar plantilla de solicitud con preguntas especificas de comunidad (motivacion, disponibilidad, necesidades de localización) | Documentos-core-01 | 2025-05-18 | Completado | `docs/examples/docs_preview_request_template.md` ahora incluye la sección Comunidad, referenciada en el formulario de ingesta. |
| W2-P3 | Asegurar aprobación de gobernanza para el plan de admisión (voto en reunión + actas registradas) | Enlace de gobernanza | 2025-05-22 | Completado | Voto aprobado por unanimidad el 2025-05-20; actas y roll call enlazados en `DOCS-SORA-Preview-W2`. |
| W2-P4 | Programar staging del proxy Try it + captura de telemetria para la ventana W2 (`preview-2025-06-15`) | Documentos/DevRel + Operaciones | 2025-06-05 | Completado | Boleto de cambio `OPS-TRYIT-188` aprobado y ejecutado 2025-06-09 02:00-04:00 UTC; capturas de pantalla de Grafana archivadas con el ticket. || W2-P5 | Construir/verificar nueva etiqueta de artefacto de vista previa (`preview-2025-06-15`) y archivar descriptor/checksum/probe logs | Portal TL | 2025-06-07 | Completado | `scripts/preview_wave_preflight.sh --tag preview-2025-06-15 ...` se ejecutó el 2025-06-10; salidas guardadas bajo `artifacts/docs_preview/W2/preview-2025-06-15/`. |
| W2-P6 | Armar roster de invitaciones comunitarias (<=25 críticos, lotes escalonados) con información de contacto aprobada por gobernanza | Responsable de la comunidad | 2025-06-10 | Completado | Primer cohorte de 8 revisores comunitarios aprobados; ID de solicitud `DOCS-SORA-Preview-REQ-C01...C08` registrados en el rastreador. |

## Lista de verificación de evidencia

- [x] Registro de aprobación de gobernanza (notas de reunión + enlace de voto) adjunto a `DOCS-SORA-Preview-W2`.
- [x] Plantilla de solicitud actualizada comprometida bajo `docs/examples/`.
- [x] Descriptor `preview-2025-06-15`, log de checksum, salida de sonda, informe de enlace y transcripción del proxy Pruébalo guardado bajo `artifacts/docs_preview/W2/`.
- [x] Capturas de pantalla de Grafana (`docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals`) capturadas para la ventana de verificación previa W2.
- [x] Tabla de roster de invitaciones con ID de revisores, tickets de solicitud y timestamps de aprobación completadas antes del envío (ver sección W2 del tracker).

Mantener este plan actualizado; el tracker lo referencia para que el roadmap DOCS-SORA vea exactamente lo que resta antes de que salgan las invitaciones W2.