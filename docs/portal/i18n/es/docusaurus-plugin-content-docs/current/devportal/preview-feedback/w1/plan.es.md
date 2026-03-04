---
lang: es
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w1/plan.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: vista previa-comentarios-w1-plan
título: Plan de verificación previa de socios W1
sidebar_label: Plan W1
descripción: Tareas, responsables y checklist de evidencia para la cohorte de previa de socios.
---

| Artículo | Detalles |
| --- | --- |
| ola | W1 - Socios e integradores de Torii |
| Ventana objetivo | Q2 2025 semana 3 |
| Etiqueta de artefacto (planeado) | `preview-2025-04-12` |
| Problema del rastreador | `DOCS-SORA-Preview-W1` |

## Objetivos

1. Asegurar aprobaciones legales y de gobernanza para los términos de previa de socios.
2. Preparar el proxy Pruébalo y instantáneas de telemetría usadas en el paquete de invitación.
3. Refrescar el artefacto de vista previa verificado por suma de comprobación y los resultados de sondas.
4. Finalizar el roster de socios y las plantillas de solicitud antes de enviar invitaciones.

## Desglose de tareas| identificación | Tarea | Responsable | Fecha límite | Estado | Notas |
| --- | --- | --- | --- | --- | --- |
| W1-P1 | Obtener aprobación legal para el anexo de términos de vista previa | Líder de Docs/DevRel -> Legal | 2025-04-05 | Completado | Boleto legal `DOCS-SORA-Preview-W1-Legal` aprobado el 2025-04-05; PDF adjunto al rastreador. |
| W1-P2 | Capturar ventana de staging del proxy Pruébalo (2025-04-10) y validar salud del proxy | Documentos/DevRel + Operaciones | 2025-04-06 | Completado | Se ejecutó `npm run manage:tryit-proxy -- --stage preview-w1 --expires-in=21d --target https://tryit-preprod.sora` el 2025-04-06; transcripción de CLI y `.env.tryit-proxy.bak` archivados. |
| W1-P3 | Construir artefacto de vista previa (`preview-2025-04-12`), correr `scripts/preview_verify.sh` + `npm run probe:portal`, archivar descriptor/checksums | Portal TL | 2025-04-08 | Completado | Artefacto y registros de verificación guardados en `artifacts/docs_preview/W1/preview-2025-04-12/`; salida de sonda adjunta al rastreador. |
| W1-P4 | Revisar formularios de admisión de socios (`DOCS-SORA-Preview-REQ-P01...P08`), confirmar contactos y NDAs | Enlace de gobernanza | 2025-04-07 | Completado | Las ocho solicitudes aprobadas (las últimas dos el 2025-04-11); aprobaciones enlazadas en el tracker. |
| W1-P5 | Redactar copia de invitación (basado en `docs/examples/docs_preview_invite_template.md`), fijar `<preview_tag>` y `<request_ticket>` para cada socio | Líder de Docs/DevRel | 2025-04-08 | Completado | Borrador de invitacion enviado el 2025-04-12 15:00 UTC junto con enlaces de artefacto. |

## Lista de verificación de verificación previa> Consejo: ejecuta `scripts/preview_wave_preflight.sh --tag preview-2025-04-12 --base-url https://preview.staging.sora --descriptor artifacts/preview-2025-04-12/descriptor.json --archive artifacts/preview-2025-04-12/docs-portal-preview.tar.zst --tryit-target https://tryit-proxy.staging.sora --output-json artifacts/preview-2025-04-12/preflight-summary.json` para ejecutar los pasos 1-5 automáticamente (build, verificación de checksum, probe del portal, link checker y actualización del proxy Try it). El script registra un log JSON que puedes adjuntar al problema del tracker.

1. `npm run build` (con `DOCS_RELEASE_TAG=preview-2025-04-12`) para regenerar `build/checksums.sha256` y `build/release.json`.
2. `docs/portal/scripts/preview_verify.sh --build-dir docs/portal/build --descriptor artifacts/<tag>/descriptor.json --archive artifacts/<tag>/docs-portal-preview.tar.zst`.
3. `PORTAL_BASE_URL=https://preview.staging.sora DOCS_RELEASE_TAG=preview-2025-04-12 npm run probe:portal -- --expect-release=preview-2025-04-12`.
4. `DOCS_RELEASE_TAG=preview-2025-04-12 npm run check:links` y archivar `build/link-report.json` junto al descriptor.
5. `npm run manage:tryit-proxy -- update --target https://tryit-proxy.staging.sora` (o pasar el objetivo adecuado vía `--tryit-target`); Comprueba el `.env.tryit-proxy` actualizado y conserva el `.bak` para revertir.
6. Actualiza el problema W1 con rutas de registros (checksum del descriptor, salida de sonda, cambio del proxy Try it y snapshots Grafana).

## Lista de verificación de evidencia

- [x] Aprobación legal firmada (PDF o enlace al ticket) adjunta a `DOCS-SORA-Preview-W1`.
- [x] Capturas de pantalla de Grafana para `docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals`.
- [x] Descriptor y registro de suma de comprobación de `preview-2025-04-12` guardados bajo `artifacts/docs_preview/W1/`.
- [x] Tabla de roster de invitaciones con timestamps `invite_sent_at` completos (ver log W1 del tracker).
- [x] Artefactos de feedback reflejados en [`preview-feedback/w1/log.md`](./log.md) con una fila por socio (actualizado 2025-04-26 con datos de roster/telemetria/issues).

Actualiza este plan a medida que avancen las tareas; el tracker lo referencia para mantener el roadmap auditable.## Flujo de comentarios

1. Para cada revisor, duplica la plantilla en
   [`docs/examples/docs_preview_feedback_form.md`](../../../../../examples/docs_preview_feedback_form.md),
   completa los metadatos y guarda la copia terminada bajo
   `artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/`.
2. Reanudar invitaciones, checkpoints de telemetria y issues abiertos dentro del log vivo en
   [`preview-feedback/w1/log.md`](./log.md) para que los revisores de gobernanza puedan revisar toda la ola
   sin salir del repositorio.
3. Cuando lleguen exports de know-check o encuestas, adjuntelos en la ruta de artefactos indicada en el log
   y enlaza el problema del rastreador.