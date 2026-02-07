---
lang: es
direction: ltr
source: docs/portal/docs/devportal/preview-invite-tracker.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Трекер приглашений vista previa

Esta función de seguimiento incluye todos los documentos del portal de vista previa, los archivos DOCS-SORA y los controladores de gobernanza de los editores. видеть активную когорту, кто утвердил приглашения и какие артефакты еще требуют внимания. Tenga en cuenta que, si lo desea, puede utilizar un dispositivo auditivo que esté instalado en los repositorios.

## Estado del volumen| Volna | Когорта | Problema трекера | Aprobadores | Estado | Целевое окно | Примечания |
| --- | --- | --- | --- | --- | --- | --- |
| **W0 - Mantenedores principales** | Mantenedores Docs + SDK, flujo de suma de comprobación de validación | `DOCS-SORA-Preview-W0` (GitHub/rastreador de operaciones) | Documentos principales/DevRel + Portal TL | Завершено | Q2 2025 números 1-2 | Приглашения отправлены 2025-03-25, телеметрия была зеленой, итоговый resumen опубликован 2025-04-08. |
| **W1 - Socios** | Operadores SoraFS, integradores Torii en NDA | `DOCS-SORA-Preview-W1` | Coordinador principal de documentos/DevRel + gobernanza | Завершено | Q2 2025 неделя 3 | Приглашения 2025-04-12 -> 2025-04-26, все восемь партнеров подтвердили; evidencia en [`preview-feedback/w1/log.md`](./preview-feedback/w1/log.md) y resumen выхода en [`preview-feedback/w1/summary.md`](./preview-feedback/w1/summary.md). |
| **W2 - Comunidad** | Lista de espera de la comunidad curadora ( 2025-06-29, telemeter зеленая весь период; evidencia + hallazgos в [`preview-feedback/w2/summary.md`](./preview-feedback/w2/summary.md). |
| **W3 - Cohortes Beta** | Beta de finanzas/observabilidad + socio de SDK + defensor del ecosistema | `DOCS-SORA-Preview-W3` | Coordinador principal de documentos/DevRel + gobernanza | Завершено | Q1 2026 неделя 8 | Fecha 2026-02-18 -> 2026-02-28; resumen + datos del portal через волну `preview-20260218` (см. [`preview-feedback/w3/summary.md`](./preview-feedback/w3/summary.md)). |> Примечание: связывайте каждую трекера соответствующими vista previa de solicitud de tickets y архивируйте их в проекте `docs-portal-preview`, чтобы aprobaciones оставались доступными.

## Активные задачи (W0)

- Artefactos de verificación previa disponibles (descarga GitHub Actions `docs-portal-preview` 2025-03-24, descriptor disponible en `scripts/preview_verify.sh` con el código `preview-2025-03-24`).
- Telemetros de referencia de Зафиксированы (`docs.preview.integrity`, instantánea `TryItProxyErrors` publicada en el número W0).
- Texto de divulgación зафиксирован через [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md) con etiqueta de vista previa `preview-2025-03-24`.
- Записаны ingesta запросы для первых пяти mantenedores (boletos `DOCS-SORA-Preview-REQ-01` ... `-05`).
- Первые пять приглашений отправлены 2025-03-25 10:00-10:20 UTC después de dos televisores de pantalla plana; agradecimientos сохранены в `DOCS-SORA-Preview-W0`.
- Monitoreo de televisores + horario de oficina para el personal (check-ins hasta el 31/03/2025; registro de puntos de control hasta el 31 de marzo de 2025).
- Comentarios/problemas del punto medio de Собраны y отмечены `docs-preview/w0` (см. [W0 digest](./preview-feedback/w0/summary.md)).
- Resumen público de volúmenes + información completa (salir del paquete 2025-04-08; см. [W0 digest](./preview-feedback/w0/summary.md)).
- W3 beta wave отслежена; будущие волны планируются после revisión de la gobernanza.

## Резюме волны socios W1- Aprobaciones legales y de gobernanza. Anexo socios подписан 2025-04-05; aprobaciones загружены в `DOCS-SORA-Preview-W1`.
- Telemetría + Pruébalo en escena. Boleto de cambio `OPS-TRYIT-147` publicado el 6 de abril de 2025 con instantáneas Grafana `docs.preview.integrity`, `TryItProxyErrors` y `DocsPortal/GatewayRefusals`.
- Preparación de artefacto + suma de comprobación. Paquete `preview-2025-04-12` проверен; descriptor de registros/suma de comprobación/sonda сохранены в `artifacts/docs_preview/W1/preview-2025-04-12/`.
- Invitar lista + despacho. Все восемь solicitudes de socios (`DOCS-SORA-Preview-REQ-P01...P08`) одобрены; приглашения отправлены 2025-04-12 15:00-15:21 UTC, agradecimientos зафиксированы по ревьюеру.
- Instrumentación de retroalimentación. Ежедневные horario de oficina + puntos de control telemétricos; см. [`preview-feedback/w1/summary.md`](./preview-feedback/w1/summary.md).
- Lista final/registro de salida. [`preview-feedback/w1/log.md`](./preview-feedback/w1/log.md) теперь фиксирует marcas de tiempo invitación/ack, evidencia de telemetría, exportaciones de cuestionarios y punteros артефактов на 2025-04-26, чтобы могла воспроизвести волну.

## Registro de invitaciones: mantenedores principales de W0| ID del revisor | Rol | Solicitar billete | Invitación enviada (UTC) | Salida prevista (UTC) | Estado | Notas |
| --- | --- | --- | --- | --- | --- | --- |
| documentos-core-01 | Mantenedor del portal | `DOCS-SORA-Preview-REQ-01` | 2025-03-25 10:05 | 2025-04-08 10:00 | Activo | Suma de verificación de verificación de Подтвердил; Fotos de revisión de navegación/barra lateral. |
| sdk-óxido-01 | Plomo del SDK de Rust | `DOCS-SORA-Preview-REQ-02` | 2025-03-25 10:08 | 2025-04-08 10:00 | Activo | Pruebe recetas del SDK + inicios rápidos de Norito. |
| sdk-js-01 | Mantenedor del SDK de JS | `DOCS-SORA-Preview-REQ-03` | 2025-03-25 10:12 | 2025-04-08 10:00 | Activo | Valido Pruébelo consola + flujos ISO. |
| sorafs-ops-01 | SoraFS enlace operador | `DOCS-SORA-Preview-REQ-04` | 2025-03-25 10:15 | 2025-04-08 10:00 | Activo | Audite los runbooks SoraFS + documentos de orquestación. |
| observabilidad-01 | Observabilidad TL | `DOCS-SORA-Preview-REQ-05` | 2025-03-25 10:18 | 2025-04-08 10:00 | Activo | Revisar los apéndices de telemetría/incidentes; отвечает за Cobertura de Alertmanager. |

Hay varias aplicaciones instaladas en Odín y en el artefacto `docs-portal-preview` (ejecutado el 24 de marzo de 2025, etiqueta `preview-2025-03-24`) y archivos de transcripción en `DOCS-SORA-Preview-W0`. Любые добавления/паузы нужно фиксировать and в таблице выше, and в issues трекера перед переходом к следующей волне.

## Registro de puntos de control - W0| Fecha (UTC) | Actividad | Notas |
| --- | --- | --- |
| 2025-03-26 | Revisión de telemetría inicial + horario de oficina | `docs.preview.integrity` + `TryItProxyErrors` оставались зелеными; horario de oficina подтвердили завершение verificación de suma de comprobación. |
| 2025-03-27 | Se publicó el resumen de comentarios del punto medio | Resumen сохранен в [`preview-feedback/w0/summary.md`](./preview-feedback/w0/summary.md); Две problemas menores de navegación отмечены `docs-preview/w0`, инцидентов нет. |
| 2025-03-31 | Verificación puntual de telemetría de la última semana | Последние horario de oficina перед выходом; ревьюеры подтвердили оставшиеся задачи, алертов не было. |
| 2025-04-08 | Resumen de salida + cierres de invitaciones | Подтверждены завершенные обзоры, временный доступ отозван, hallazgos архивированы в [`preview-feedback/w0/summary.md`](./preview-feedback/w0/summary.md#exit-summary-2025-04-08); rastreador обновлен перед W1. |

## Registro de invitaciones: socios de W1| ID del revisor | Rol | Solicitar billete | Invitación enviada (UTC) | Salida prevista (UTC) | Estado | Notas |
| --- | --- | --- | --- | --- | --- | --- |
| sorafs-op-01 | Operador SoraFS (UE) | `DOCS-SORA-Preview-REQ-P01` | 2025-04-12 15:00 | 2025-04-26 15:00 | Completado | Comentarios sobre operaciones del orquestador entregados el 20 de abril de 2025; salir de nuevo a las 15:05 UTC. |
| sorafs-op-02 | Operador SoraFS (JP) | `DOCS-SORA-Preview-REQ-P02` | 2025-04-12 15:03 | 2025-04-26 15:00 | Completado | Comentarios de implementación registrados en `docs-preview/w1`; salir de nuevo a las 15:10 UTC. |
| sorafs-op-03 | Operador SoraFS (EE. UU.) | `DOCS-SORA-Preview-REQ-P03` | 2025-04-12 15:06 | 2025-04-26 15:00 | Completado | Se presentaron ediciones de disputa/lista negra; salir de nuevo a las 15:12 UTC. |
| torii-int-01 | Integrador Torii | `DOCS-SORA-Preview-REQ-P04` | 2025-04-12 15:09 | 2025-04-26 15:00 | Completado | Pruébelo, se acepta el tutorial de autenticación; salir de nuevo a las 15:14 UTC. |
| torii-int-02 | Integrador Torii | `DOCS-SORA-Preview-REQ-P05` | 2025-04-12 15:12 | 2025-04-26 15:00 | Completado | Se registran los comentarios del documento RPC/OAuth; salir de nuevo a las 15:16 UTC. |
| socio-sdk-01 | Socio SDK (Swift) | `DOCS-SORA-Preview-REQ-P06` | 2025-04-12 15:15 | 2025-04-26 15:00 | Completado | Comentarios de integridad de vista previa fusionados; salir de nuevo a las 15:18 UTC. |
| socio-sdk-02 | Socio SDK (Android) | `DOCS-SORA-Preview-REQ-P07` | 2025-04-12 15:18 | 2025-04-26 15:00 | Completado | Revisión de telemetría/redacción realizada; salir de nuevo a las 15:22 UTC. || puerta de enlace-ops-01 | Operador de puerta de enlace | `DOCS-SORA-Preview-REQ-P08` | 2025-04-12 15:21 | 2025-04-26 15:00 | Completado | Se presentaron los comentarios del runbook DNS de la puerta de enlace; salir de nuevo a las 15:24 UTC. |

## Registro de puntos de control - W1

| Fecha (UTC) | Actividad | Notas |
| --- | --- | --- |
| 2025-04-12 | Envío de invitaciones + verificación de artefactos | Los ocho socios enviaron por correo electrónico el descriptor/archivo `preview-2025-04-12`; acuses de recibo almacenados en el rastreador. |
| 2025-04-13 | Revisión básica de telemetría | Tableros `docs.preview.integrity`, `TryItProxyErrors` e `DocsPortal/GatewayRefusals` verdes; horario de oficina confirmado verificación de suma de verificación completada. |
| 2025-04-18 | Horario de oficina de media ola | `docs.preview.integrity` permaneció en verde; dos documentos registrados en `docs-preview/w1` (texto de navegación + captura de pantalla Pruébelo). |
| 2025-04-22 | Verificación puntual final de telemetría | Proxy + paneles de control en buen estado; No se plantearon nuevos problemas, anotados en el rastreador antes de la salida. |
| 2025-04-26 | Resumen de salida + cierres de invitaciones | Todos los socios confirmaron la finalización de la revisión, las invitaciones revocadas y la evidencia archivada en [`preview-feedback/w1/summary.md`](./preview-feedback/w1/summary.md#exit-summary-2025-04-26). |

## Resumen de la cohorte beta del W3

- Invitaciones enviadas el 18/02/2026 con verificación de suma de verificación + acuses de recibo registrados el mismo día.
- Comentarios recopilados en `docs-preview/20260218` con el problema de gobernanza `DOCS-SORA-Preview-20260218`; resumen + resumen generado a través de `npm run --prefix docs/portal preview:wave -- --wave preview-20260218`.
- Acceso revocado el 28/02/2026 después de la verificación final de telemetría; Se actualizaron las tablas del rastreador + portal para mostrar W3 como completado.## Registro de invitaciones: comunidad W2| ID del revisor | Rol | Solicitar billete | Invitación enviada (UTC) | Salida prevista (UTC) | Estado | Notas |
| --- | --- | --- | --- | --- | --- | --- |
| comunicación-vol-01 | Revisor de la comunidad (SDK) | `DOCS-SORA-Preview-REQ-C01` | 2025-06-15 16:00 | 2025-06-29 16:00 | Completado | Confirmación a las 16:06 UTC; centrarse en los inicios rápidos del SDK; salida confirmada 2025-06-29. |
| comunicación-vol-02 | Revisor de la comunidad (Gobernanza) | `REQ-C02` | 2025-06-15 16:03 | 2025-06-29 16:00 | Completado | Revisión de gobernanza/SNS realizada; salida confirmada 2025-06-29. |
| comunicación-vol-03 | Revisor de la comunidad (Norito) | `REQ-C03` | 2025-06-15 16:06 | 2025-06-29 16:00 | Completado | Se registraron comentarios sobre la guía Norito; salir de nuevo 2025-06-29. |
| comunicación-vol-04 | Revisor de la comunidad (SoraFS) | `REQ-C04` | 2025-06-15 16:09 | 2025-06-29 16:00 | Completado | Revisión del runbook SoraFS realizada; salir de nuevo 2025-06-29. |
| comunicación-vol-05 | Revisor de la comunidad (Accesibilidad) | `REQ-C05` | 2025-06-15 16:12 | 2025-06-29 16:00 | Completado | Notas de accesibilidad/UX compartidas; salir de nuevo 2025-06-29. |
| comunicación-vol-06 | Revisor de la comunidad (Localización) | `REQ-C06` | 2025-06-15 16:15 | 2025-06-29 16:00 | Completado | Comentarios de localización registrados; salir de nuevo 2025-06-29. |
| comunicación-vol-07 | Revisor de la comunidad (móvil) | `REQ-C07` | 2025-06-15 16:18 | 2025-06-29 16:00 | Completado | Se entregan comprobaciones de documentos del SDK móvil; salir de nuevo 2025-06-29. || comunicación-vol-08 | Revisor de la comunidad (Observabilidad) | `REQ-C08` | 2025-06-15 16:21 | 2025-06-29 16:00 | Completado | Revisión del apéndice de observabilidad realizada; salir de nuevo 2025-06-29. |

## Registro de puntos de control - W2

| Fecha (UTC) | Actividad | Notas |
| --- | --- | --- |
| 2025-06-15 | Envío de invitaciones + verificación de artefactos | Descriptor/archivo `preview-2025-06-15` compartido con 8 revisores de la comunidad; acuses de recibo almacenados en el rastreador. |
| 2025-06-16 | Revisión básica de telemetría | `docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals` tableros verdes; Pruébelo, los registros de proxy muestran tokens comunitarios activos. |
| 2025-06-18 | Horario de oficina y clasificación de problemas | Se recopilaron dos sugerencias (texto de información sobre herramientas `docs-preview/w2 #1`, barra lateral de localización `#2`), ambas enviadas a Docs. |
| 2025-06-21 | Comprobación de telemetría + correcciones de documentos | Los documentos dirigidos a `docs-preview/w2 #1/#2`; Los cuadros de mando siguen en verde, sin incidencias. |
| 2025-06-24 | Horario de oficina de la última semana | Los revisores confirmaron los comentarios restantes; No hay fuego de alerta. |
| 2025-06-29 | Resumen de salida + cierres de invitaciones | Confirmaciones registradas, acceso a vista previa revocado, instantáneas de telemetría + artefactos archivados (consulte [`preview-feedback/w2/summary.md`](./preview-feedback/w2/summary.md#exit-summary-2025-06-29)). |
| 2025-04-15 | Horario de oficina y clasificación de problemas | Dos sugerencias de documentación registradas en `docs-preview/w1`; no se activaron incidentes ni alertas. |

## Ganchos de informes- Cada miércoles, actualice la tabla de seguimiento anterior más el problema de invitación activa con una breve nota de estado (invitaciones enviadas, revisores activos, incidentes).
- Cuando se cierra una ola, agregue la ruta del resumen de comentarios (por ejemplo, `docs/portal/docs/devportal/preview-feedback/w0/summary.md`) y vincúlela desde `status.md`.
- Si se activa algún criterio de pausa del [flujo de invitación de vista previa](./preview-invite-flow.md), agregue aquí los pasos de corrección antes de reanudar las invitaciones.