---
lang: es
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w2/plan.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: vista previa-comentarios-w2-plan
título: Plan de admisión comunitaria W2
sidebar_label: Plan W2
descripción: Admisión, aprobaciones y lista de verificación de preuve pour la cohorte previa comunitaria.
---

| Elemento | Detalles |
| --- | --- |
| Vago | W2 - Revisores comunitarios |
| Ventana cible | Tercer trimestre de 2025, semana 1 (tentativa) |
| Etiqueta de artefacto (planificar) | `preview-2025-06-15` |
| Rastreador de problemas | `DOCS-SORA-Preview-W2` |

## Objetivos

1. Definir los criterios de admisión comunitaria y el flujo de trabajo de verificación.
2. Obtener la gobernanza de aprobación para la lista propuesta y el complemento de uso aceptable.
3. Rafraichir la vista previa del artefacto verifica la suma de comprobación y el paquete de telemetría para la nueva ventana.
4. Prepare el proxy Pruébelo y los paneles de control antes del envío de invitaciones.

## Decoupage de taches| identificación | Taché | Responsable | Echeance | Estatuto | Notas |
| --- | --- | --- | --- | --- | --- |
| G2-P1 | Redigerar los criterios de admisión comunitaria (elegibilidad, espacios máximos, exigencias CoC) y difundir la gobernanza | Líder de Docs/DevRel | 2025-05-15 | Terminar | La politique d'intake a ete mergee dans `DOCS-SORA-Preview-W2` et endossee lors de la reunion du conseil 2025-05-20. |
| W2-P2 | Mettre a jour le template de demande avec des questions communautaires (motivación, disponibilidad, necesidades de localización) | Documentos-core-01 | 2025-05-18 | Terminar | `docs/examples/docs_preview_request_template.md` incluye mantenimiento de la sección Comunidad, referencia en el formulario de admisión. |
| W2-P3 | Obtener la aprobación de la gobernanza para el plan de admisión (votación en reunión + actas registradas) | Enlace de gobernanza | 2025-05-22 | Terminar | Voto adoptado por unanimidad el 2025-05-20; minutos + pasar lista se encuentra en `DOCS-SORA-Preview-W2`. |
| W2-P4 | Planificador de puesta en escena del proxy Pruébelo + captura de telemetría para la ventana W2 (`preview-2025-06-15`) | Documentos/DevRel + Operaciones | 2025-06-05 | Terminar | Cambio de billete `OPS-TRYIT-188` aprobar y ejecutar 2025-06-09 02:00-04:00 UTC; capturas de pantalla Grafana archivos con ticket. || W2-P5 | Construir/verificar la nueva etiqueta de vista previa de artefacto (`preview-2025-06-15`) y archivar descriptores/suma de comprobación/registros de sonda | Portal TL | 2025-06-07 | Terminar | `scripts/preview_wave_preflight.sh --tag preview-2025-06-15 ...` ejecutar 2025-06-10; salidas existencias sous `artifacts/docs_preview/W2/preview-2025-06-15/`. |
| W2-P6 | Reúne la lista de invitaciones comunitarias (<=25 revisores, muchos niveles) con los contactos aprobados por la gobernanza | Responsable de la comunidad | 2025-06-10 | Terminar | Estreno cohorte de 8 críticos comunitarios aprobados; Los ID de solicitud `DOCS-SORA-Preview-REQ-C01...C08` se registran en el rastreador. |

## Lista de verificación anterior

- [x] Registro de aprobación de gobierno (notas de reunión + gravamen de voto) adjunto a `DOCS-SORA-Preview-W2`.
- [x] Plantilla de demanda del día del comité bajo `docs/examples/`.
- [x] Descriptor `preview-2025-06-15`, suma de verificación de registro, salida de sonda, informe de enlace y proxy de transcripción Pruébelo almacenado en `artifacts/docs_preview/W2/`.
- [x] Capturas de pantalla Grafana (`docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals`) capturas para la verificación previa de la ventana W2.
- [x] Lista de invitaciones de Tableau con ID de revisores, tickets de demanda y marcas de tiempo de aprobación enviadas antes del envío (consulte la sección W2 del rastreador).

Garder ce planea un día; El rastreador de referencia para la hoja de ruta DOCS-SORA es exactamente lo que queda antes del envío de invitaciones W2.