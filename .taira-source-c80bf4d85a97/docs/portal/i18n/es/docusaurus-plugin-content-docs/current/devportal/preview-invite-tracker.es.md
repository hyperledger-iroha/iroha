---
lang: es
direction: ltr
source: docs/portal/docs/devportal/preview-invite-tracker.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Rastreador de invitaciones de vista previa

Este rastreador registra cada ola de vista previa del portal de documentos para que los propietarios de DOCS-SORA y los revisores de gobernanza vean que cohorte esta activa, quien aprobo las invitaciones y que artefactos siguen pendientes. Actualizalo cada vez que se envien, revoquen o difieran invitaciones para que el rastro de auditoria quede dentro del repositorio.

## Estado de olas| ola | cohorte | Problema de seguimiento | Aprobador(es) | Estado | Ventana objetivo | Notas |
| --- | --- | --- | --- | --- | --- | --- |
| **W0 - Mantenedores principales** | Mantenedores de Docs + SDK validando el flujo de suma de comprobación | `DOCS-SORA-Preview-W0` (GitHub/rastreador de operaciones) | Documentos principales/DevRel + Portal TL | Completado | Q2 2025 semanas 1-2 | Invitaciones enviadas 2025-03-25, telemetria se mantuvo verde, resumen de salida publicado 2025-04-08. |
| **W1 - Socios** | Operadores SoraFS, integradores Torii bajo NDA | `DOCS-SORA-Preview-W1` | Lead Docs/DevRel + enlace de gobernanza | Completado | Q2 2025 semana 3 | Invitaciones 2025-04-12 -> 2025-04-26 con los ocho socios confirmados; capturada en evidencia [`preview-feedback/w1/log.md`](./preview-feedback/w1/log.md) y el resumen de salida en [`preview-feedback/w1/summary.md`](./preview-feedback/w1/summary.md). |
| **W2 - Comunidad** | Lista de espera comunitaria curada ( 2025-06-29 con telemetria verde todo el periodo; evidencia + hallazgos capturados en [`preview-feedback/w2/summary.md`](./preview-feedback/w2/summary.md). || **S3 - Cohortes beta** | Beta finanzas/observabilidad + socio SDK + defensor del ecosistema | `DOCS-SORA-Preview-W3` | Lead Docs/DevRel + enlace de gobernanza | Completado | T1 2026 semana 8 | Invitaciones 2026-02-18 -> 2026-02-28; resumen + datos del portal generados vía ola `preview-20260218` (ver [`preview-feedback/w3/summary.md`](./preview-feedback/w3/summary.md)). |

> Nota: enlaza cada emisión del tracker con los tickets de solicitud de vista previa y archivalas bajo el proyecto `docs-portal-preview` para que las aprobaciones sigan siendo descubribles.

## Tareas activas (W0)- Artefactos de verificación previa actualizados (ejecución GitHub Actions `docs-portal-preview` 2025-03-24, descriptor verificado vía `scripts/preview_verify.sh` usando la etiqueta `preview-2025-03-24`).
- Líneas base de telemetría capturadas (`docs.preview.integrity`, instantánea de paneles `TryItProxyErrors` guardada en la edición W0).
- Texto de alcance bloqueado usando [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md) con vista previa de etiqueta `preview-2025-03-24`.
- Solicitudes de ingreso registrados para los primeros cinco mantenedores (tickets `DOCS-SORA-Preview-REQ-01` ... `-05`).
- Primeras cinco invitaciones enviadas 2025-03-25 10:00-10:20 UTC después de siete días consecutivos de telemetria verde; acuses guardados en `DOCS-SORA-Preview-W0`.
- Monitoreo de telemetria + horario de oficina del host (check-ins diarios hasta 2025-03-31; log de checkpoints abajo).
- Feedback de mitad de ola / issues recopilados y etiquetados `docs-preview/w0` (ver [W0 digest](./preview-feedback/w0/summary.md)).
- Resumen de ola publicado + confirmaciones de salida de invitaciones (bundle de salida fechado 2025-04-08; ver [W0 digest](./preview-feedback/w0/summary.md)).
- Ola beta W3 seguida; futuras olas programadas según revisión de gobernanza.

## Resumen de socios de ola W1- Aprobaciones legales y de gobernanza. Addendum de socios firmado el 05-04-2025; aprobaciones subidas a `DOCS-SORA-Preview-W1`.
- Telemetria + Pruébalo en escena. Ticket de cambio `OPS-TRYIT-147` ejecutado 2025-04-06 con snapshots Grafana de `docs.preview.integrity`, `TryItProxyErrors`, y `DocsPortal/GatewayRefusals` archivados.
- Preparación de artefacto + suma de comprobación. Paquete `preview-2025-04-12` verificado; logs de descriptor/checksum/probe guardados en `artifacts/docs_preview/W1/preview-2025-04-12/`.
- Lista de invitaciones + envío. Ocho solicitudes de socios (`DOCS-SORA-Preview-REQ-P01...P08`) aprobadas; invitaciones enviadas 2025-04-12 15:00-15:21 UTC con acuses registrados por revisor.
- Instrumentación de retroalimentación. Horario de oficina diario + puntos de control de telemetria registrados; ver [`preview-feedback/w1/summary.md`](./preview-feedback/w1/summary.md) para el resumen.
- Roster final / registro de salida. [`preview-feedback/w1/log.md`](./preview-feedback/w1/log.md) ahora registra timestamps de invitacion/ack, evidencia de telemetria, exports de quiz y punteros de artefactos al 2025-04-26 para que gobernanza pueda reproducir la ola.

## Registro de invitaciones - Mantenedores principales de W0| ID de revisor | papel | Boleto de solicitud | Invitación enviada (UTC) | Salida esperada (UTC) | Estado | Notas |
| --- | --- | --- | --- | --- | --- | --- |
| documentos-core-01 | Mantenedor del portal | `DOCS-SORA-Preview-REQ-01` | 2025-03-25 10:05 | 2025-04-08 10:00 | Activo | Confirmo verificación de suma de comprobación; enfocado en revisión de nav/sidebar. |
| sdk-óxido-01 | Plomo del SDK de Rust | `DOCS-SORA-Preview-REQ-02` | 2025-03-25 10:08 | 2025-04-08 10:00 | Activo | Probando recetas de SDK + inicios rápidos de Norito. |
| sdk-js-01 | Mantenedor del SDK de JS | `DOCS-SORA-Preview-REQ-03` | 2025-03-25 10:12 | 2025-04-08 10:00 | Activo | Validando consola Pruébalo + flujos ISO. |
| sorafs-ops-01 | SoraFS enlace operador | `DOCS-SORA-Preview-REQ-04` | 2025-03-25 10:15 | 2025-04-08 10:00 | Activo | Auditando runbooks de SoraFS + docs de orquestación. |
| observabilidad-01 | Observabilidad TL | `DOCS-SORA-Preview-REQ-05` | 2025-03-25 10:18 | 2025-04-08 10:00 | Activo | Revisando apéndices de telemetria/incidentes; responsable de la cobertura de Alertmanager. |

Todas las invitaciones referencian el mismo artefacto `docs-portal-preview` (ejecucion 2025-03-24, tag `preview-2025-03-24`) y el registro de verificación capturado en `DOCS-SORA-Preview-W0`. Cualquier alta/pausa debe registrarse tanto en la tabla anterior como en la edición del tracker antes de proceder a la siguiente ola.

## Registro de puntos de control - W0| Fecha (UTC) | Actividad | Notas |
| --- | --- | --- |
| 2025-03-26 | Revisión de línea base de telemetría + horario de atención | `docs.preview.integrity` + `TryItProxyErrors` se mantuvieron verdes; office hours confirmaron que todos los revisores completaron la verificación de checksum. |
| 2025-03-27 | Resumen de comentarios intermedio publicado | Resumen capturado en [`preview-feedback/w0/summary.md`](./preview-feedback/w0/summary.md); dos issues menores de nav registradas como `docs-preview/w0`, sin incidentes reportados. |
| 2025-03-31 | Chequeo de telemetria de la ultima semana | Ultimas horas de oficina previas a la salida; Los revisores confirmaron las tareas restantes en curso, sin alertas. |
| 2025-04-08 | Resumen de salida + cierres de invitaciones | Reseñas completadas confirmadas, acceso temporal revocado, hallazgos archivados en [`preview-feedback/w0/summary.md`](./preview-feedback/w0/summary.md#exit-summary-2025-04-08); rastreador actualizado antes de preparar W1. |

## Registro de invitaciones - Socios W1| ID de revisor | papel | Boleto de solicitud | Invitación enviada (UTC) | Salida esperada (UTC) | Estado | Notas |
| --- | --- | --- | --- | --- | --- | --- |
| sorafs-op-01 | Operador SoraFS (UE) | `DOCS-SORA-Preview-REQ-P01` | 2025-04-12 15:00 | 2025-04-26 15:00 | Completado | Entrego feedback de operaciones del orquestador 2025-04-20; ack de salida 15:05 UTC. |
| sorafs-op-02 | Operador SoraFS (JP) | `DOCS-SORA-Preview-REQ-P02` | 2025-04-12 15:03 | 2025-04-26 15:00 | Completado | Registro comentarios de implementación en `docs-preview/w1`; respuesta a las 15:10 UTC. |
| sorafs-op-03 | Operador SoraFS (EE. UU.) | `DOCS-SORA-Preview-REQ-P03` | 2025-04-12 15:06 | 2025-04-26 15:00 | Completado | Ediciones de disputa/lista negra registradas; respuesta a las 15:12 UTC. |
| torii-int-01 | Integrador Torii | `DOCS-SORA-Preview-REQ-P04` | 2025-04-12 15:09 | 2025-04-26 15:00 | Completado | Tutorial de Pruébelo con autenticación aceptada; respuesta a las 15:14 UTC. |
| torii-int-02 | Integrador Torii | `DOCS-SORA-Preview-REQ-P05` | 2025-04-12 15:12 | 2025-04-26 15:00 | Completado | Comentarios de RPC/OAuth registrados; respuesta a las 15:16 UTC. |
| socio-sdk-01 | Socio SDK (Swift) | `DOCS-SORA-Preview-REQ-P06` | 2025-04-12 15:15 | 2025-04-26 15:00 | Completado | Comentarios de integridad de vista previa fusionado; respuesta a las 15:18 UTC. |
| socio-sdk-02 | Socio SDK (Android) | `DOCS-SORA-Preview-REQ-P07` | 2025-04-12 15:18 | 2025-04-26 15:00 | Completado | Revisión de telemetria/redacción hecha; respuesta a las 15:22 UTC. || puerta de enlace-ops-01 | Operador de puerta de enlace | `DOCS-SORA-Preview-REQ-P08` | 2025-04-12 15:21 | 2025-04-26 15:00 | Completado | Comentarios del runbook de DNS gateway registrados; respuesta a las 15:24 UTC. |

## Registro de puntos de control - W1

| Fecha (UTC) | Actividad | Notas |
| --- | --- | --- |
| 2025-04-12 | Envío de invitaciones + verificación de artefactos | Los ocho socios recibieron correo electrónico con descriptor/archivo `preview-2025-04-12`; acusa registrados en el tracker. |
| 2025-04-13 | Revisión de línea base de telemetría | `docs.preview.integrity`, `TryItProxyErrors`, y `DocsPortal/GatewayRefusals` en verde; Horario de oficina confirmaron la verificación de suma de verificación completada. |
| 2025-04-18 | Horario de oficina de mitad de ola | `docs.preview.integrity` se mantuvo verde; dos nits de docs registrados como `docs-preview/w1` (texto de navegación + captura de pantalla de Pruébalo). |
| 2025-04-22 | Chequeo final de telemetria | Proxy + paneles de control saludables; sin issues nuevas, anotado en el tracker antes de salida. |
| 2025-04-26 | Resumen de salida + cierres de invitaciones | Todos los socios confirmaron revisión, invitaciones revocadas, evidencia archivada en [`preview-feedback/w1/summary.md`](./preview-feedback/w1/summary.md#exit-summary-2025-04-26). |

## Resumen de la cohorte beta W3- Invitaciones enviadas 2026-02-18 con verificacion de checksum + acuses registrados el mismo dia.
- Comentarios recopilados bajo `docs-preview/20260218` con issues de gobernanza `DOCS-SORA-Preview-20260218`; digest + resumen generados vía `npm run --prefix docs/portal preview:wave -- --wave preview-20260218`.
- Acceso revocado 2026-02-28 despues del chequeo final de telemetria; tracker + tablas del portal actualizadas para marcar W3 como completado.

## Registro de invitaciones - Comunidad W2| ID de revisor | papel | Boleto de solicitud | Invitación enviada (UTC) | Salida esperada (UTC) | Estado | Notas |
| --- | --- | --- | --- | --- | --- | --- |
| comunicación-vol-01 | Revisor de la comunidad (SDK) | `DOCS-SORA-Preview-REQ-C01` | 2025-06-15 16:00 | 2025-06-29 16:00 | Completado | Confirmación a las 16:06 UTC; enfocado en inicios rápidos de SDK; salida confirmada 2025-06-29. |
| comunicación-vol-02 | Revisor de la comunidad (Gobernanza) | `REQ-C02` | 2025-06-15 16:03 | 2025-06-29 16:00 | Completado | Revisión de gobernanza/SNS hecha; salida confirmada 2025-06-29. |
| comunicación-vol-03 | Revisor de la comunidad (Norito) | `REQ-C03` | 2025-06-15 16:06 | 2025-06-29 16:00 | Completado | Comentarios del tutorial Norito registrado; confirmar 2025-06-29. |
| comunicación-vol-04 | Revisor de la comunidad (SoraFS) | `REQ-C04` | 2025-06-15 16:09 | 2025-06-29 16:00 | Completado | Revisión de runbooks SoraFS hecha; confirmar 2025-06-29. |
| comunicación-vol-05 | Revisor de la comunidad (Accesibilidad) | `REQ-C05` | 2025-06-15 16:12 | 2025-06-29 16:00 | Completado | Notas de accesibilidad/UX compartidas; confirmar 2025-06-29. |
| comunicación-vol-06 | Revisor de la comunidad (Localización) | `REQ-C06` | 2025-06-15 16:15 | 2025-06-29 16:00 | Completado | Comentarios de localización registrada; confirmar 2025-06-29. || comunicación-vol-07 | Revisor de la comunidad (móvil) | `REQ-C07` | 2025-06-15 16:18 | 2025-06-29 16:00 | Completado | Verificaciones de documentos de SDK móviles entregados; confirmar 2025-06-29. |
| comunicación-vol-08 | Revisor de la comunidad (Observabilidad) | `REQ-C08` | 2025-06-15 16:21 | 2025-06-29 16:00 | Completado | Revisión de apéndice de observabilidad hecho; confirmar 2025-06-29. |

## Registro de puntos de control - W2| Fecha (UTC) | Actividad | Notas |
| --- | --- | --- |
| 2025-06-15 | Envío de invitaciones + verificación de artefactos | Descriptor/archivo `preview-2025-06-15` compartido con 8 revisores; acusa guardados en tracker. |
| 2025-06-16 | Revisión de línea base de telemetría | `docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals` en verde; logs del proxy Pruébalo muestran tokens comunitarios activos. |
| 2025-06-18 | Horarios de atención y triaje de incidencias | Dos sugerencias (`docs-preview/w2 #1` texto de información sobre herramientas, `#2` barra lateral de localización): ambas asignadas a Docs. |
| 2025-06-21 | Chequeo de telemetria + correcciones de documentos | Documentos resueltos `docs-preview/w2 #1/#2`; Cuadros de mandos verdes, sin incidentes. |
| 2025-06-24 | Horario de oficina de la última semana | Los revisores confirmaron envíos pendientes; no se dispararon alertas. |
| 2025-06-29 | Resumen de salida + cierres de invitaciones | Acks registrados, acceso de vista previa revocado, snapshots + artefactos archivados (ver [`preview-feedback/w2/summary.md`](./preview-feedback/w2/summary.md#exit-summary-2025-06-29)). |
| 2025-04-15 | Horarios de atención y triaje de incidencias | Dos sugerencias de documentación registrada bajo `docs-preview/w1`; sin incidentes ni alertas. |

## Ganchos de informe- Cada miercoles, actualiza la tabla del tracker y la emisión de invitaciones activa con una nota corta de estado (invitaciones enviadas, revisores activos, incidentes).
- Cuando una ola cierre, agregue la ruta del resumen de feedback (por ejemplo, `docs/portal/docs/devportal/preview-feedback/w0/summary.md`) y enlacela desde `status.md`.
- Si se activan criterios de pausa de [preview invite flow](./preview-invite-flow.md), agregue los pasos de remediacion aqui antes de reanudar invitaciones.