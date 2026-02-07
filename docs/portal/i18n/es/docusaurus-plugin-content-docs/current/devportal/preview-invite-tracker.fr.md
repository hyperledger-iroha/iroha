---
lang: es
direction: ltr
source: docs/portal/docs/devportal/preview-invite-tracker.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Vista previa del rastreador de invitaciones

Este rastreador registra cada vaga vista previa de los documentos del portal relacionados con los propietarios DOCS-SORA y los selectores de gobierno de todas las cohortes que están activas, que aprueban las invitaciones y los artefactos que permanecen en el traidor. Mettez-le a jour cada vez que las invitaciones sont enviados, revocados o reportados para que la pista de auditoría reste en le depósito.

## Estatuto de vagos| Vago | cohorte | Problema de seguimiento | Aprobador(es) | Estatuto | Ventana cible | Notas |
| --- | --- | --- | --- | --- | --- | --- |
| **W0 - Mantenedores principales** | Mantenedores Docs + SDK válido para suma de comprobación de flujo | `DOCS-SORA-Preview-W0` (rastreador GitHub/operaciones) | Documentos principales/DevRel + Portal TL | Terminar | Segundo trimestre de 2025, semanas 1-2 | Invitaciones enviadas 2025-03-25, telemetrie restee verte, resume de sortie publie 2025-04-08. |
| **W1 - Socios** | Operadores SoraFS, integradores Torii bajo NDA | `DOCS-SORA-Preview-W1` | Lead Docs/DevRel + gobernanza de enlace | Terminar | Q2 2025 semana 3 | Invitaciones 2025-04-12 -> 2025-04-26 avec les huit partners confirma; evidencia capturada en [`preview-feedback/w1/log.md`](./preview-feedback/w1/log.md) y currículum de salida en [`preview-feedback/w1/summary.md`](./preview-feedback/w1/summary.md). |
| **W2 - Comuna** | Lista de espera para triee comunales ( 2025-06-29 con telemetrie verte tout du long; evidencia + constantes en [`preview-feedback/w2/summary.md`](./preview-feedback/w2/summary.md). |
| **S3 - Cohortes beta** | Finanzas beta/observabilidad + SDK de socio + ecosistema defensor | `DOCS-SORA-Preview-W3` | Lead Docs/DevRel + gobernanza de enlace | Terminar | Primer trimestre de 2026, semana 8 | Invitaciones 2026-02-18 -> 2026-02-28; resume + donnees portail generees via la vague `preview-20260218` (voir [`preview-feedback/w3/summary.md`](./preview-feedback/w3/summary.md)). |> Nota: cada vez que emita el rastreador de tickets de demanda de vista previa y archivez-les dans le projet `docs-portal-preview` para que les aprobaciones restantes se decouvrables.

## Tachas activas (W0)

- Artefactos de verificación previa de rafraichis (ejecución de GitHub Actions `docs-portal-preview` 2025-03-24, verificación del descriptor a través de `scripts/preview_verify.sh` con la etiqueta `preview-2025-03-24`).
- Capturas de telemetría de líneas base (`docs.preview.integrity`, instantánea de los paneles de control `TryItProxyErrors` guardadas en el número W0).
- Texto de divulgación figura con [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md) y vista previa de etiqueta `preview-2025-03-24`.
- Demandes d'entree enregistrees pour les cinq premiers mantenedores (tickets `DOCS-SORA-Preview-REQ-01` ... `-05`).
- Cinq estrena invitaciones enviadas 2025-03-25 10:00-10:20 UTC después de los días consecutivos de telemetría verde; Accus stockes en `DOCS-SORA-Preview-W0`.
- Suivi telemetrie + horario de oficina del anfitrión (check-ins quotidiens jusqu'au 2025-03-31; log des checkpoints ci-dessous).
- Comentarios mi-vagos / issues recopilados y etiquetados `docs-preview/w0` (voir [W0 digest](./preview-feedback/w0/summary.md)).
- Resume de vague publie + confirmaciones de salida (fecha de salida del paquete 2025-04-08; ver [W0 digest](./preview-feedback/w0/summary.md)).
- Suivie vaga beta W3; futuros vagues planifiees selon revue gouvernance.

## Currículum vitae de los socios vagos W1- Aprobaciones legales y de gobierno. Los socios del anexo firman el 5 de abril de 2025; encargados de aprobaciones en `DOCS-SORA-Preview-W1`.
- Telemetría + Pruébalo en escena. El cambio de ticket `OPS-TRYIT-147` se ejecutó el 6 de abril de 2025 con las instantáneas Grafana de los archivos `docs.preview.integrity`, `TryItProxyErrors` y `DocsPortal/GatewayRefusals`.
- Artefacto de preparación + suma de comprobación. Paquete `preview-2025-04-12` verificar; descriptor de registros/suma de verificación/existencias de sonda en `artifacts/docs_preview/W1/preview-2025-04-12/`.
- Lista de invitaciones + envío. Huit exige socios (`DOCS-SORA-Preview-REQ-P01...P08`) aprobados; invitaciones enviadas 2025-04-12 15:00-15:21 UTC avec accus par lecteur.
- Retroalimentación de instrumentación. Horario de oficina cotidiano + puntos de control telemetría registros; Consulte [`preview-feedback/w1/summary.md`](./preview-feedback/w1/summary.md) para digerirlo.
- Lista final / registro de salida. [`preview-feedback/w1/log.md`](./preview-feedback/w1/log.md) Registre marcas de tiempo de mantenimiento de invitación/ack, telemetría de evidencia, cuestionario de exportación y punteros de artefactos el 26 de abril de 2025 para permitir la gobernanza de la relectura.

## Registro de invitaciones: mantenedores principales de W0| lector de identificación | Rol | Boleto de demanda | Enviado de invitación (UTC) | Asistente de salida (UTC) | Estatuto | Notas |
| --- | --- | --- | --- | --- | --- | --- |
| documentos-core-01 | Mantenedor del portal | `DOCS-SORA-Preview-REQ-01` | 2025-03-25 10:05 | 2025-04-08 10:00 | Activo | Una confirmación de la suma de verificación de verificación; foco de navegación/barra lateral. |
| sdk-óxido-01 | Plomo del SDK de Rust | `DOCS-SORA-Preview-REQ-02` | 2025-03-25 10:08 | 2025-04-08 10:00 | Activo | Pruebe las recetas SDK + inicios rápidos Norito. |
| sdk-js-01 | Mantenedor del SDK de JS | `DOCS-SORA-Preview-REQ-03` | 2025-03-25 10:12 | 2025-04-08 10:00 | Activo | Valide la consola Pruébalo + flujos ISO. |
| sorafs-ops-01 | SoraFS enlace operador | `DOCS-SORA-Preview-REQ-04` | 2025-03-25 10:15 | 2025-04-08 10:00 | Activo | Audite los runbooks SoraFS + orquestación de documentos. |
| observabilidad-01 | Observabilidad TL | `DOCS-SORA-Preview-REQ-05` | 2025-03-25 10:18 | 2025-04-08 10:00 | Activo | Revoit les anexos telemetrie/incidentes; Alertmanager de cobertura responsable. |

Todas las invitaciones hacen referencia al meme artefacto `docs-portal-preview` (ejecución 2025-03-24, etiqueta `preview-2025-03-24`) y al registro de verificación capturado en `DOCS-SORA-Preview-W0`. Todos los ajustes/pausa deben guardarse en la tabla ci-dessus y el problema del rastreador antes de pasar a la vaga siguiente.

## Registro de puntos de control - W0| Fecha (UTC) | Actividad | Notas |
| --- | --- | --- |
| 2025-03-26 | Revue telemetría base + horario de oficina | `docs.preview.integrity` + `TryItProxyErrors` están en reposo verts; Horario de oficina para confirmar la suma de verificación terminada. |
| 2025-03-27 | Resumen de retroalimentación intermediaria pública | Reanudar captura en [`preview-feedback/w0/summary.md`](./preview-feedback/w0/summary.md); deux issues nav mineures taguees `docs-preview/w0`, incidente aucun. |
| 2025-03-31 | Comprobar telemetría fin de semana | Dernieres horas de oficina previas a la salida; Los selectores ont confirme les taches restantes, aucune alerte. |
| 2025-04-08 | Currículum vitae de salida + cierre de invitaciones | Reseñas terminadas confirmadas, acceso temporal revocado, archivos constantes en [`preview-feedback/w0/summary.md`](./preview-feedback/w0/summary.md#exit-summary-2025-04-08); rastreador mis a jour avant W1. |

## Registro de invitaciones: socios de W1| lector de identificación | Rol | Boleto de demanda | Enviado de invitación (UTC) | Asistente de salida (UTC) | Estatuto | Notas |
| --- | --- | --- | --- | --- | --- | --- |
| sorafs-op-01 | Operador SoraFS (UE) | `DOCS-SORA-Preview-REQ-P01` | 2025-04-12 15:00 | 2025-04-26 15:00 | Terminar | Libro del orquestador de operaciones de retroalimentación 2025-04-20; salida de regreso a las 15:05 UTC. |
| sorafs-op-02 | Operador SoraFS (JP) | `DOCS-SORA-Preview-REQ-P02` | 2025-04-12 15:03 | 2025-04-26 15:00 | Terminar | Registros de implementación de comentarios en `docs-preview/w1`; respuesta a las 15:10 UTC. |
| sorafs-op-03 | Operador SoraFS (EE. UU.) | `DOCS-SORA-Preview-REQ-P03` | 2025-04-12 15:06 | 2025-04-26 15:00 | Terminar | Edita registros de disputas/listas negras; respuesta a las 15:12 UTC. |
| torii-int-01 | Integrador Torii | `DOCS-SORA-Preview-REQ-P04` | 2025-04-12 15:09 | 2025-04-26 15:00 | Terminar | Tutorial Pruébelo auth Accepte; respuesta a las 15:14 UTC. |
| torii-int-02 | Integrador Torii | `DOCS-SORA-Preview-REQ-P05` | 2025-04-12 15:12 | 2025-04-26 15:00 | Terminar | Comentarios Registros RPC/OAuth; respuesta a las 15:16 UTC. |
| socio-sdk-01 | Socio SDK (Swift) | `DOCS-SORA-Preview-REQ-P06` | 2025-04-12 15:15 | 2025-04-26 15:00 | Terminar | Combinación de vista previa integrada de comentarios; respuesta a las 15:18 UTC. |
| socio-sdk-02 | Socio SDK (Android) | `DOCS-SORA-Preview-REQ-P07` | 2025-04-12 15:18 | 2025-04-26 15:00 | Terminar | Revue telemetrie/redacción faite; respuesta a las 15:22 UTC. || puerta de enlace-ops-01 | Operador de puerta de enlace | `DOCS-SORA-Preview-REQ-P08` | 2025-04-12 15:21 | 2025-04-26 15:00 | Terminar | Comentarios registros de puerta de enlace DNS del runbook; respuesta a las 15:24 UTC. |

## Registro de puntos de control - W1

| Fecha (UTC) | Actividad | Notas |
| --- | --- | --- |
| 2025-04-12 | Invitaciones enviadas + artefactos de verificación | Correos electrónicos de Huit Partners con descriptor/archivo `preview-2025-04-12`; Accus stockes dans le tracker. |
| 2025-04-13 | Línea base de telemetría de revisión | `docs.preview.integrity`, `TryItProxyErrors` y `DocsPortal/GatewayRefusals` verticales; Horario de oficina para confirmar la suma de verificación terminada. |
| 2025-04-18 | Horas de oficina vagas | `docs.preview.integrity` resto vert; deux nits docs tagges `docs-preview/w1` (redacción de navegación + captura de pantalla Pruébelo). |
| 2025-04-22 | Consultar telemetría final | Proxy + paneles de control sains; Aucune nouvelle issues, notee dans le tracker avant sortie. |
| 2025-04-26 | Currículum vitae de salida + invitaciones de cierre | Todos los socios confirmaron la revisión, invitaciones revocadas, evidencia archivada en [`preview-feedback/w1/summary.md`](./preview-feedback/w1/summary.md#exit-summary-2025-04-26). |

## Resumen cohorte beta W3

- Invitaciones enviadas 2026-02-18 con suma de verificación de verificación + accus le meme jour.
- Comentarios recopilados bajo `docs-preview/20260218` con gobernanza de problemas `DOCS-SORA-Preview-20260218`; digerir + reanudar genere a través de `npm run --prefix docs/portal preview:wave -- --wave preview-20260218`.
- Acceso revoque 2026-02-28 después de comprobar la telemetría final; tracker + tablas portail mises a jour pour marquer W3 termine.## Registro de invitaciones - Comunidad W2| lector de identificación | Rol | Boleto de demanda | Enviado de invitación (UTC) | Asistente de salida (UTC) | Estatuto | Notas |
| --- | --- | --- | --- | --- | --- | --- |
| comunicación-vol-01 | Revisor de la comunidad (SDK) | `DOCS-SORA-Preview-REQ-C01` | 2025-06-15 16:00 | 2025-06-29 16:00 | Terminar | Confirmación a las 16:06 UTC; SDK de inicio rápido de Focus; Confirmación de salida 2025-06-29. |
| comunicación-vol-02 | Revisor de la comunidad (Gobernanza) | `REQ-C02` | 2025-06-15 16:03 | 2025-06-29 16:00 | Terminar | Gobernanza de la revista/titular del SNS; Confirmación de salida 2025-06-29. |
| comunicación-vol-03 | Revisor de la comunidad (Norito) | `REQ-C03` | 2025-06-15 16:06 | 2025-06-29 16:00 | Terminar | Tutorial de comentarios Registro Norito; confirmar 2025-06-29. |
| comunicación-vol-04 | Revisor de la comunidad (SoraFS) | `REQ-C04` | 2025-06-15 16:09 | 2025-06-29 16:00 | Terminar | Runbooks de revisión SoraFS finalista; confirmar 2025-06-29. |
| comunicación-vol-05 | Revisor de la comunidad (Accesibilidad) | `REQ-C05` | 2025-06-15 16:12 | 2025-06-29 16:00 | Terminar | Participes de notas de accesibilidad/UX; confirmar 2025-06-29. |
| comunicación-vol-06 | Revisor de la comunidad (Localización) | `REQ-C06` | 2025-06-15 16:15 | 2025-06-29 16:00 | Terminar | Registro de localización de comentarios; confirmar 2025-06-29. |
| comunicación-vol-07 | Revisor de la comunidad (móvil) | `REQ-C07` | 2025-06-15 16:18 | 2025-06-29 16:00 | Terminar | Comprueba las libras móviles del SDK de documentos; confirmar 2025-06-29. || comunicación-vol-08 | Revisor de la comunidad (Observabilidad) | `REQ-C08` | 2025-06-15 16:21 | 2025-06-29 16:00 | Terminar | Revista anexa observabilité terminae; confirmar 2025-06-29. |

## Registro de puntos de control - W2

| Fecha (UTC) | Actividad | Notas |
| --- | --- | --- |
| 2025-06-15 | Invitaciones enviadas + artefactos de verificación | Descriptor/archivo `preview-2025-06-15` partage avec 8 lecteurs; Acusar acciones en el rastreador. |
| 2025-06-16 | Línea base de telemetría de revisión | `docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals` verticales; logs proxy Pruébelo montrent tokens commmunaute actifs. |
| 2025-06-18 | Horarios de oficina y cuestiones de clasificación | Sugerencias de dos (información sobre herramientas de texto `docs-preview/w2 #1`, localización de la barra lateral `#2`): todos los asignados a dos documentos. |
| 2025-06-21 | Verificar telemetría + corregir documentos | Documentos de corrección `docs-preview/w2 #1/#2`; Tableros verts, incidente aucun. |
| 2025-06-24 | Horario de oficina fin de semana | Los seleccionadores confirman los retornos restantes; aucune alertate. |
| 2025-06-29 | Currículum vitae de salida + invitaciones de cierre | Registros de reconocimientos, revocación de vista previa de acceso, instantáneas + archivos de artefactos (ver [`preview-feedback/w2/summary.md`](./preview-feedback/w2/summary.md#exit-summary-2025-06-29)). |
| 2025-04-15 | Horarios de oficina y cuestiones de clasificación | Dos registros de documentación de sugerencias bajo `docs-preview/w1`; aucun incidente ni alerta. |

## Ganchos para informar- Chaque mercredi, mettre a jour le tableau ci-dessus et l'issue invite active avec une note courte (invitaciones enviadas, relecteurs actifs, incidentes).
- Quand une vague se termine, ajouter le chemin du resume feedback (ej. `docs/portal/docs/devportal/preview-feedback/w0/summary.md`) et le lier depuis `status.md`.
- Si un criterio de pausa del [flujo de invitación de vista previa](./preview-invite-flow.md) está desactivado, agregue las etapas de remediación aquí antes de responder las invitaciones.