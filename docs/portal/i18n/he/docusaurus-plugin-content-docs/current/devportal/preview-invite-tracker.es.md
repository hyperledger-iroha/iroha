---
lang: he
direction: rtl
source: docs/portal/docs/devportal/preview-invite-tracker.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# Tracker de invitaciones de preview

Este tracker registra cada ola de preview del portal de docs para que los owners de DOCS-SORA y los revisores de gobernanza vean que cohorte esta activa, quien aprobo las invitaciones y que artefactos siguen pendientes. Actualizalo cada vez que se envien, revoquen o difieran invitaciones para que el rastro de auditoria quede dentro del repositorio.

## Estado de olas

| Ola | Cohorte | Issue de seguimiento | Aprobador(es) | Estado | Ventana objetivo | Notas |
| --- | --- | --- | --- | --- | --- | --- |
| **W0 - Core maintainers** | Maintainers de Docs + SDK validando el flujo de checksum | `DOCS-SORA-Preview-W0` (GitHub/ops tracker) | Lead Docs/DevRel + Portal TL | Completado | Q2 2025 semanas 1-2 | Invitaciones enviadas 2025-03-25, telemetria se mantuvo verde, resumen de salida publicado 2025-04-08. |
| **W1 - Partners** | Operadores SoraFS, integradores Torii bajo NDA | `DOCS-SORA-Preview-W1` | Lead Docs/DevRel + enlace de gobernanza | Completado | Q2 2025 semana 3 | Invitaciones 2025-04-12 -> 2025-04-26 con los ocho partners confirmados; evidencia capturada en [`preview-feedback/w1/log.md`](./preview-feedback/w1/log.md) y el resumen de salida en [`preview-feedback/w1/summary.md`](./preview-feedback/w1/summary.md). |
| **W2 - Comunidad** | Lista de espera comunitaria curada (<=25 a la vez) | `DOCS-SORA-Preview-W2` | Lead Docs/DevRel + community manager | Completado | Q3 2025 semana 1 (tentativo) | Invitaciones 2025-06-15 -> 2025-06-29 con telemetria verde todo el periodo; evidencia + hallazgos capturados en [`preview-feedback/w2/summary.md`](./preview-feedback/w2/summary.md). |
| **W3 - Cohortes beta** | Beta finanzas/observabilidad + partner SDK + defensor del ecosistema | `DOCS-SORA-Preview-W3` | Lead Docs/DevRel + enlace de gobernanza | Completado | Q1 2026 semana 8 | Invitaciones 2026-02-18 -> 2026-02-28; resumen + datos del portal generados via ola `preview-20260218` (ver [`preview-feedback/w3/summary.md`](./preview-feedback/w3/summary.md)). |

> Nota: enlaza cada issue del tracker con los tickets de solicitud de preview y archivalas bajo el proyecto `docs-portal-preview` para que las aprobaciones sigan siendo descubribles.

## Tareas activas (W0)

- Artefactos de preflight actualizados (ejecucion GitHub Actions `docs-portal-preview` 2025-03-24, descriptor verificado via `scripts/preview_verify.sh` usando tag `preview-2025-03-24`).
- Baselines de telemetria capturados (`docs.preview.integrity`, snapshot de dashboards `TryItProxyErrors` guardado en la issue W0).
- Texto de outreach bloqueado usando [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md) con tag preview `preview-2025-03-24`.
- Solicitudes de ingreso registradas para los primeros cinco maintainers (tickets `DOCS-SORA-Preview-REQ-01` ... `-05`).
- Primeras cinco invitaciones enviadas 2025-03-25 10:00-10:20 UTC despues de siete dias consecutivos de telemetria verde; acuses guardados en `DOCS-SORA-Preview-W0`.
- Monitoreo de telemetria + office hours del host (check-ins diarios hasta 2025-03-31; log de checkpoints abajo).
- Feedback de mitad de ola / issues recopilados y etiquetados `docs-preview/w0` (ver [W0 digest](./preview-feedback/w0/summary.md)).
- Resumen de ola publicado + confirmaciones de salida de invitaciones (bundle de salida fechado 2025-04-08; ver [W0 digest](./preview-feedback/w0/summary.md)).
- Ola beta W3 seguida; futuras olas programadas segun revision de gobernanza.

## Resumen de ola W1 partners

- Aprobaciones legales y de gobernanza. Addendum de partners firmado 2025-04-05; aprobaciones subidas a `DOCS-SORA-Preview-W1`.
- Telemetria + Try it staging. Ticket de cambio `OPS-TRYIT-147` ejecutado 2025-04-06 con snapshots Grafana de `docs.preview.integrity`, `TryItProxyErrors`, y `DocsPortal/GatewayRefusals` archivados.
- Preparacion de artefacto + checksum. Bundle `preview-2025-04-12` verificado; logs de descriptor/checksum/probe guardados en `artifacts/docs_preview/W1/preview-2025-04-12/`.
- Roster de invitaciones + envio. Ocho solicitudes de partners (`DOCS-SORA-Preview-REQ-P01...P08`) aprobadas; invitaciones enviadas 2025-04-12 15:00-15:21 UTC con acuses registrados por revisor.
- Instrumentacion de feedback. Office hours diarias + checkpoints de telemetria registrados; ver [`preview-feedback/w1/summary.md`](./preview-feedback/w1/summary.md) para el digest.
- Roster final / log de salida. [`preview-feedback/w1/log.md`](./preview-feedback/w1/log.md) ahora registra timestamps de invitacion/ack, evidencia de telemetria, exports de quiz y punteros de artefactos al 2025-04-26 para que gobernanza pueda reproducir la ola.

## Log de invitaciones - W0 core maintainers

| ID de revisor | Rol | Ticket de solicitud | Invitacion enviada (UTC) | Salida esperada (UTC) | Estado | Notas |
| --- | --- | --- | --- | --- | --- | --- |
| docs-core-01 | Portal maintainer | `DOCS-SORA-Preview-REQ-01` | 2025-03-25 10:05 | 2025-04-08 10:00 | Activo | Confirmo verificacion de checksum; enfocado en revision de nav/sidebar. |
| sdk-rust-01 | Rust SDK lead | `DOCS-SORA-Preview-REQ-02` | 2025-03-25 10:08 | 2025-04-08 10:00 | Activo | Probando recetas de SDK + quickstarts de Norito. |
| sdk-js-01 | JS SDK maintainer | `DOCS-SORA-Preview-REQ-03` | 2025-03-25 10:12 | 2025-04-08 10:00 | Activo | Validando consola Try it + flujos ISO. |
| sorafs-ops-01 | SoraFS operator liaison | `DOCS-SORA-Preview-REQ-04` | 2025-03-25 10:15 | 2025-04-08 10:00 | Activo | Auditando runbooks de SoraFS + docs de orquestacion. |
| observability-01 | Observability TL | `DOCS-SORA-Preview-REQ-05` | 2025-03-25 10:18 | 2025-04-08 10:00 | Activo | Revisando apendices de telemetria/incidentes; responsable de cobertura de Alertmanager. |

Todas las invitaciones referencian el mismo artefacto `docs-portal-preview` (ejecucion 2025-03-24, tag `preview-2025-03-24`) y el registro de verificacion capturado en `DOCS-SORA-Preview-W0`. Cualquier alta/pausa debe registrarse tanto en la tabla anterior como en la issue del tracker antes de proceder a la siguiente ola.

## Log de checkpoints - W0

| Fecha (UTC) | Actividad | Notas |
| --- | --- | --- |
| 2025-03-26 | Revision de telemetria baseline + office hours | `docs.preview.integrity` + `TryItProxyErrors` se mantuvieron verdes; office hours confirmaron que todos los revisores completaron la verificacion de checksum. |
| 2025-03-27 | Digest de feedback intermedio publicado | Resumen capturado en [`preview-feedback/w0/summary.md`](./preview-feedback/w0/summary.md); dos issues menores de nav registradas como `docs-preview/w0`, sin incidentes reportados. |
| 2025-03-31 | Chequeo de telemetria de la ultima semana | Ultimas office hours pre-exit; revisores confirmaron tareas restantes en curso, sin alertas. |
| 2025-04-08 | Resumen de salida + cierres de invitaciones | Reviews completadas confirmadas, acceso temporal revocado, hallazgos archivados en [`preview-feedback/w0/summary.md`](./preview-feedback/w0/summary.md#exit-summary-2025-04-08); tracker actualizado antes de preparar W1. |

## Log de invitaciones - W1 partners

| ID de revisor | Rol | Ticket de solicitud | Invitacion enviada (UTC) | Salida esperada (UTC) | Estado | Notas |
| --- | --- | --- | --- | --- | --- | --- |
| sorafs-op-01 | SoraFS operator (EU) | `DOCS-SORA-Preview-REQ-P01` | 2025-04-12 15:00 | 2025-04-26 15:00 | Completado | Entrego feedback de ops del orquestador 2025-04-20; ack de salida 15:05 UTC. |
| sorafs-op-02 | SoraFS operator (JP) | `DOCS-SORA-Preview-REQ-P02` | 2025-04-12 15:03 | 2025-04-26 15:00 | Completado | Registro comentarios de rollout en `docs-preview/w1`; ack 15:10 UTC. |
| sorafs-op-03 | SoraFS operator (US) | `DOCS-SORA-Preview-REQ-P03` | 2025-04-12 15:06 | 2025-04-26 15:00 | Completado | Ediciones de dispute/blacklist registradas; ack 15:12 UTC. |
| torii-int-01 | Torii integrator | `DOCS-SORA-Preview-REQ-P04` | 2025-04-12 15:09 | 2025-04-26 15:00 | Completado | Walkthrough de Try it auth aceptado; ack 15:14 UTC. |
| torii-int-02 | Torii integrator | `DOCS-SORA-Preview-REQ-P05` | 2025-04-12 15:12 | 2025-04-26 15:00 | Completado | Comentarios de RPC/OAuth registrados; ack 15:16 UTC. |
| sdk-partner-01 | SDK partner (Swift) | `DOCS-SORA-Preview-REQ-P06` | 2025-04-12 15:15 | 2025-04-26 15:00 | Completado | Feedback de integridad de preview fusionado; ack 15:18 UTC. |
| sdk-partner-02 | SDK partner (Android) | `DOCS-SORA-Preview-REQ-P07` | 2025-04-12 15:18 | 2025-04-26 15:00 | Completado | Revision de telemetria/redaction hecha; ack 15:22 UTC. |
| gateway-ops-01 | Gateway operator | `DOCS-SORA-Preview-REQ-P08` | 2025-04-12 15:21 | 2025-04-26 15:00 | Completado | Comentarios del runbook de DNS gateway registrados; ack 15:24 UTC. |

## Log de checkpoints - W1

| Fecha (UTC) | Actividad | Notas |
| --- | --- | --- |
| 2025-04-12 | Envio de invitaciones + verificacion de artefactos | Los ocho partners recibieron email con descriptor/archive `preview-2025-04-12`; acuses registrados en el tracker. |
| 2025-04-13 | Revision de telemetria baseline | `docs.preview.integrity`, `TryItProxyErrors`, y `DocsPortal/GatewayRefusals` en verde; office hours confirmaron verificacion de checksum completada. |
| 2025-04-18 | Office hours de mitad de ola | `docs.preview.integrity` se mantuvo verde; dos nits de docs registrados como `docs-preview/w1` (nav wording + screenshot de Try it). |
| 2025-04-22 | Chequeo final de telemetria | Proxy + dashboards saludables; sin issues nuevas, anotado en el tracker antes de salida. |
| 2025-04-26 | Resumen de salida + cierres de invitaciones | Todos los partners confirmaron revision, invitaciones revocadas, evidencia archivada en [`preview-feedback/w1/summary.md`](./preview-feedback/w1/summary.md#exit-summary-2025-04-26). |

## Recap de cohorte beta W3

- Invitaciones enviadas 2026-02-18 con verificacion de checksum + acuses registrados el mismo dia.
- Feedback recopilado bajo `docs-preview/20260218` con issue de gobernanza `DOCS-SORA-Preview-20260218`; digest + resumen generados via `npm run --prefix docs/portal preview:wave -- --wave preview-20260218`.
- Acceso revocado 2026-02-28 despues del chequeo final de telemetria; tracker + tablas del portal actualizadas para marcar W3 como completado.

## Log de invitaciones - W2 community

| ID de revisor | Rol | Ticket de solicitud | Invitacion enviada (UTC) | Salida esperada (UTC) | Estado | Notas |
| --- | --- | --- | --- | --- | --- | --- |
| comm-vol-01 | Community reviewer (SDK) | `DOCS-SORA-Preview-REQ-C01` | 2025-06-15 16:00 | 2025-06-29 16:00 | Completado | Ack 16:06 UTC; enfocado en quickstarts de SDK; salida confirmada 2025-06-29. |
| comm-vol-02 | Community reviewer (Governance) | `REQ-C02` | 2025-06-15 16:03 | 2025-06-29 16:00 | Completado | Revision de gobernanza/SNS hecha; salida confirmada 2025-06-29. |
| comm-vol-03 | Community reviewer (Norito) | `REQ-C03` | 2025-06-15 16:06 | 2025-06-29 16:00 | Completado | Feedback del walkthrough Norito registrado; ack 2025-06-29. |
| comm-vol-04 | Community reviewer (SoraFS) | `REQ-C04` | 2025-06-15 16:09 | 2025-06-29 16:00 | Completado | Revision de runbooks SoraFS hecha; ack 2025-06-29. |
| comm-vol-05 | Community reviewer (Accessibility) | `REQ-C05` | 2025-06-15 16:12 | 2025-06-29 16:00 | Completado | Notas de accesibilidad/UX compartidas; ack 2025-06-29. |
| comm-vol-06 | Community reviewer (Localization) | `REQ-C06` | 2025-06-15 16:15 | 2025-06-29 16:00 | Completado | Feedback de localizacion registrado; ack 2025-06-29. |
| comm-vol-07 | Community reviewer (Mobile) | `REQ-C07` | 2025-06-15 16:18 | 2025-06-29 16:00 | Completado | Checks de docs de SDK mobile entregados; ack 2025-06-29. |
| comm-vol-08 | Community reviewer (Observability) | `REQ-C08` | 2025-06-15 16:21 | 2025-06-29 16:00 | Completado | Revision de apendice de observabilidad hecha; ack 2025-06-29. |

## Log de checkpoints - W2

| Fecha (UTC) | Actividad | Notas |
| --- | --- | --- |
| 2025-06-15 | Envio de invitaciones + verificacion de artefactos | Descriptor/archive `preview-2025-06-15` compartido con 8 revisores; acuses guardados en tracker. |
| 2025-06-16 | Revision de telemetria baseline | `docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals` en verde; logs del proxy Try it muestran tokens comunitarios activos. |
| 2025-06-18 | Office hours y triage de issues | Dos sugerencias (`docs-preview/w2 #1` wording de tooltip, `#2` sidebar de localizacion) - ambas asignadas a Docs. |
| 2025-06-21 | Chequeo de telemetria + fixes de docs | Docs resolvio `docs-preview/w2 #1/#2`; dashboards verdes, sin incidentes. |
| 2025-06-24 | Office hours de la ultima semana | Revisores confirmaron envios pendientes; no se dispararon alertas. |
| 2025-06-29 | Resumen de salida + cierres de invitaciones | Acks registrados, acceso de preview revocado, snapshots + artefactos archivados (ver [`preview-feedback/w2/summary.md`](./preview-feedback/w2/summary.md#exit-summary-2025-06-29)). |
| 2025-04-15 | Office hours y triage de issues | Dos sugerencias de documentacion registradas bajo `docs-preview/w1`; sin incidentes ni alertas. |

## Hooks de reporte

- Cada miercoles, actualiza la tabla del tracker y la issue de invitaciones activa con una nota corta de estado (invitaciones enviadas, revisores activos, incidentes).
- Cuando una ola cierre, agrega la ruta del resumen de feedback (por ejemplo, `docs/portal/docs/devportal/preview-feedback/w0/summary.md`) y enlazala desde `status.md`.
- Si se activan criterios de pausa de [preview invite flow](./preview-invite-flow.md), agrega los pasos de remediacion aqui antes de reanudar invitaciones.
