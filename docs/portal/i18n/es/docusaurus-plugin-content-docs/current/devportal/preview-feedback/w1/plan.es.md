---
lang: es
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w1/plan.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: preview-feedback-w1-plan
title: Plan de preflight de partners W1
sidebar_label: Plan W1
description: Tareas, responsables y checklist de evidencia para la cohorte de preview de partners.
---

| Item | Detalles |
| --- | --- |
| Ola | W1 - Partners y integradores de Torii |
| Ventana objetivo | Q2 2025 semana 3 |
| Tag de artefacto (planeado) | `preview-2025-04-12` |
| Issue del tracker | `DOCS-SORA-Preview-W1` |

## Objetivos

1. Asegurar aprobaciones legales y de gobernanza para los terminos de preview de partners.
2. Preparar el proxy Try it y snapshots de telemetria usados en el paquete de invitacion.
3. Refrescar el artefacto de preview verificado por checksum y los resultados de probes.
4. Finalizar el roster de partners y las plantillas de solicitud antes de enviar invitaciones.

## Desglose de tareas

| ID | Tarea | Responsable | Fecha limite | Estado | Notas |
| --- | --- | --- | --- | --- | --- |
| W1-P1 | Obtener aprobacion legal para el anexo de terminos de preview | Docs/DevRel lead -> Legal | 2025-04-05 | Completado | Ticket legal `DOCS-SORA-Preview-W1-Legal` aprobado el 2025-04-05; PDF adjunto al tracker. |
| W1-P2 | Capturar ventana de staging del proxy Try it (2025-04-10) y validar salud del proxy | Docs/DevRel + Ops | 2025-04-06 | Completado | Se ejecuto `npm run manage:tryit-proxy -- --stage preview-w1 --expires-in=21d --target https://tryit-preprod.sora` el 2025-04-06; transcripcion de CLI y `.env.tryit-proxy.bak` archivados. |
| W1-P3 | Construir artefacto de preview (`preview-2025-04-12`), correr `scripts/preview_verify.sh` + `npm run probe:portal`, archivar descriptor/checksums | Portal TL | 2025-04-08 | Completado | Artefacto y logs de verificacion guardados en `artifacts/docs_preview/W1/preview-2025-04-12/`; salida de probe adjunta al tracker. |
| W1-P4 | Revisar formularios de intake de partners (`DOCS-SORA-Preview-REQ-P01...P08`), confirmar contactos y NDAs | Governance liaison | 2025-04-07 | Completado | Las ocho solicitudes aprobadas (las ultimas dos el 2025-04-11); aprobaciones enlazadas en el tracker. |
| W1-P5 | Redactar copy de invitacion (basado en `docs/examples/docs_preview_invite_template.md`), fijar `<preview_tag>` y `<request_ticket>` para cada partner | Docs/DevRel lead | 2025-04-08 | Completado | Borrador de invitacion enviado el 2025-04-12 15:00 UTC junto con enlaces de artefacto. |

## Checklist de preflight

> Consejo: ejecuta `scripts/preview_wave_preflight.sh --tag preview-2025-04-12 --base-url https://preview.staging.sora --descriptor artifacts/preview-2025-04-12/descriptor.json --archive artifacts/preview-2025-04-12/docs-portal-preview.tar.zst --tryit-target https://tryit-proxy.staging.sora --output-json artifacts/preview-2025-04-12/preflight-summary.json` para ejecutar los pasos 1-5 automaticamente (build, verificacion de checksum, probe del portal, link checker y actualizacion del proxy Try it). El script registra un log JSON que puedes adjuntar al issue del tracker.

1. `npm run build` (con `DOCS_RELEASE_TAG=preview-2025-04-12`) para regenerar `build/checksums.sha256` y `build/release.json`.
2. `docs/portal/scripts/preview_verify.sh --build-dir docs/portal/build --descriptor artifacts/<tag>/descriptor.json --archive artifacts/<tag>/docs-portal-preview.tar.zst`.
3. `PORTAL_BASE_URL=https://preview.staging.sora DOCS_RELEASE_TAG=preview-2025-04-12 npm run probe:portal -- --expect-release=preview-2025-04-12`.
4. `DOCS_RELEASE_TAG=preview-2025-04-12 npm run check:links` y archivar `build/link-report.json` junto al descriptor.
5. `npm run manage:tryit-proxy -- update --target https://tryit-proxy.staging.sora` (o pasar el target adecuado via `--tryit-target`); commitea el `.env.tryit-proxy` actualizado y conserva la `.bak` para rollback.
6. Actualiza el issue W1 con rutas de logs (checksum del descriptor, salida de probe, cambio del proxy Try it y snapshots Grafana).

## Checklist de evidencia

- [x] Aprobacion legal firmada (PDF o enlace al ticket) adjunta a `DOCS-SORA-Preview-W1`.
- [x] Screenshots de Grafana para `docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals`.
- [x] Descriptor y log de checksum de `preview-2025-04-12` guardados bajo `artifacts/docs_preview/W1/`.
- [x] Tabla de roster de invitaciones con timestamps `invite_sent_at` completos (ver log W1 del tracker).
- [x] Artefactos de feedback reflejados en [`preview-feedback/w1/log.md`](./log.md) con una fila por partner (actualizado 2025-04-26 con datos de roster/telemetria/issues).

Actualiza este plan a medida que avancen las tareas; el tracker lo referencia para mantener el roadmap auditable.

## Flujo de feedback

1. Para cada reviewer, duplica la plantilla en
   [`docs/examples/docs_preview_feedback_form.md`](../../../../../examples/docs_preview_feedback_form.md),
   completa los metadatos y guarda la copia terminada bajo
   `artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/`.
2. Resume invitaciones, checkpoints de telemetria y issues abiertos dentro del log vivo en
   [`preview-feedback/w1/log.md`](./log.md) para que los reviewers de gobernanza puedan revisar toda la ola
   sin salir del repositorio.
3. Cuando lleguen exports de knowledge-check o encuestas, adjuntalos en la ruta de artefactos indicada en el log
   y enlaza el issue del tracker.
