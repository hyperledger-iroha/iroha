---
lang: es
direction: ltr
source: docs/portal/docs/devportal/preview-invite-tracker.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Tracker de invitaciones para hacer vista previa

Este rastreador registra cada onda de vista previa del portal de documentos para que los propietarios de DOCS-SORA y los revisores de gobierno vejam qual coorte esta activa, quem aprovou os convites e quais artefatos ainda precisam de atencao. Atualize-o siempre que convites forem enviados, revogados ou adiados para que a trilha de auditoria permaneca no repositorio.

## Estado de las ondas| Onda | Corte | Edición de acompañamiento | Aprobador(es) | Estado | Janela alvo | Notas |
| --- | --- | --- | --- | --- | --- | --- |
| **W0 - Mantenedores principales** | Mantenedores de Docs + SDK validando el flujo de suma de comprobación | `DOCS-SORA-Preview-W0` (GitHub/rastreador de operaciones) | Documentos principales/DevRel + Portal TL | Concluido | Q2 2025 semanas 1-2 | Convites enviados 2025-03-25, telemetria ficou verde, resumen de Saida publicado 2025-04-08. |
| **W1 - Socios** | Operadores SoraFS, integradores Torii sollozo NDA | `DOCS-SORA-Preview-W1` | Lead Docs/DevRel + enlace de gobierno | Concluido | Q2 2025 semana 3 | Convites 2025-04-12 -> 2025-04-26 com oito partners confirmados; evidenciada capturada en [`preview-feedback/w1/log.md`](./preview-feedback/w1/log.md) y resumen de dicha en [`preview-feedback/w1/summary.md`](./preview-feedback/w1/summary.md). |
| **W2 - Comunidade** | Lista de espera comunitaria curada ( 2025-06-29 com telemetria verde o periodo todo; evidencia + achados em [`preview-feedback/w2/summary.md`](./preview-feedback/w2/summary.md). || **W3 - Coórtés beta** | Beta de finanzas/observabilidade + socio SDK + defensor del ecosistema | `DOCS-SORA-Preview-W3` | Lead Docs/DevRel + enlace de gobierno | Concluido | T1 2026 semana 8 | Convita 2026-02-18 -> 2026-02-28; resumen + datos del portal generados vía onda `preview-20260218` (ver [`preview-feedback/w3/summary.md`](./preview-feedback/w3/summary.md)). |

> Nota: vincule cada emisión del rastreador y entradas de solicitud de vista previa y archivo en el proyecto `docs-portal-preview` para que las aprovacoes continúen descubriendo.

## Tarefas ativas (W0)- Artefatos de verificación previa atualizados (ejecutado GitHub Actions `docs-portal-preview` 2025-03-24, descriptor verificado vía `scripts/preview_verify.sh` con etiqueta `preview-2025-03-24`).
- Líneas base de telemetría capturadas (`docs.preview.integrity`, instantánea de los paneles `TryItProxyErrors` salvo en el problema W0).
- Texto de trabajo de divulgación usando [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md) con vista previa de etiqueta `preview-2025-03-24`.
- Solicitacoes de entradas registradas para os primeiros cinco mantenedores (tickets `DOCS-SORA-Preview-REQ-01` ... `-05`).
- Cinco primeiros convites enviados 2025-03-25 10:00-10:20 UTC apos sete dias consecutivos de telemetria verde; acuses guardados en `DOCS-SORA-Preview-W0`.
- Monitoramento de telemetria + horario de oficina del host (check-ins diarios ate 2025-03-31; log de checkpoints abaixo).
- Feedback de meio de onda / issues coletadas e tagueadas `docs-preview/w0` (ver [W0 digest](./preview-feedback/w0/summary.md)).
- Resumo da onda publicado + confirmacoes de Saida (bundle de Saida fechado 2025-04-08; ver [W0 digest](./preview-feedback/w0/summary.md)).
- Onda beta W3 acompañada; futuras ondas agendadas conforme revisao de gobernancia.

## Resumen de la onda W1 socios- Aprovacoes legais e degobernanza. Addendum de socios assinados 2025-04-05; aprovacoes enviadas para `DOCS-SORA-Preview-W1`.
- Telemetria + Pruébalo en escena. Ticket de mudanca `OPS-TRYIT-147` ejecutado 2025-04-06 com snapshots Grafana de `docs.preview.integrity`, `TryItProxyErrors`, e `DocsPortal/GatewayRefusals` arquivados.
- Preparacao de artefato + suma de comprobación. Paquete `preview-2025-04-12` verificado; registros de descriptor/suma de comprobación/salvos de sonda en `artifacts/docs_preview/W1/preview-2025-04-12/`.
- Lista de invitaciones + envío. Oito solicitacoes de partners (`DOCS-SORA-Preview-REQ-P01...P08`) aprobados; convites enviados 2025-04-12 15:00-15:21 UTC com acuses registrados por revisor.
- Instrumentación de retroalimentación. Horario de oficina diario + puntos de control de telemetria registrados; ver [`preview-feedback/w1/summary.md`](./preview-feedback/w1/summary.md) para resumir.
- Lista final / log de saya. [`preview-feedback/w1/log.md`](./preview-feedback/w1/log.md) registra timestamps de convite/ack, evidencia de telemetria, exports de quiz e ponteiros de artefatos em 2025-04-26 para que agobernanca possa reproduzir a onda.

## Registro de invitaciones: mantenedores principales de W0| ID de revisor | Papel | Boleto de solicitud | Convite enviado (UTC) | Saida esperada (UTC) | Estado | Notas |
| --- | --- | --- | --- | --- | --- | --- |
| documentos-core-01 | Mantenedor del portal | `DOCS-SORA-Preview-REQ-01` | 2025-03-25 10:05 | 2025-04-08 10:00 | activo | Confirme la verificación de la suma de comprobación; foco em revisión de nav/sidebar. |
| sdk-óxido-01 | Plomo del SDK de Rust | `DOCS-SORA-Preview-REQ-02` | 2025-03-25 10:08 | 2025-04-08 10:00 | activo | Probando recetas de SDK + inicios rápidos de Norito. |
| sdk-js-01 | Mantenedor del SDK de JS | `DOCS-SORA-Preview-REQ-03` | 2025-03-25 10:12 | 2025-04-08 10:00 | activo | Validando consola Pruébalo + fluxos ISO. |
| sorafs-ops-01 | SoraFS enlace operador | `DOCS-SORA-Preview-REQ-04` | 2025-03-25 10:15 | 2025-04-08 10:00 | activo | Auditando runbooks SoraFS + documentos de orquestación. |
| observabilidad-01 | Observabilidad TL | `DOCS-SORA-Preview-REQ-05` | 2025-03-25 10:18 | 2025-04-08 10:00 | activo | Revisando apéndices de telemetria/incidentes; Responsavel pela cobertura de Alertmanager. |

Todos los convites hacen referencia al mesmo artefato `docs-portal-preview` (ejecutado el 2025-03-24, etiqueta `preview-2025-03-24`) y al registro de verificación capturado en `DOCS-SORA-Preview-W0`. Qualquer adicao/pausa debe ser registrado tanto en la tabla acima cuanto en la emisión del rastreador antes de pasar para a próxima onda.

## Registro de puntos de control - W0| Datos (UTC) | Atividade | Notas |
| --- | --- | --- |
| 2025-03-26 | Revisao de telemetria baseline + horario de oficina | `docs.preview.integrity` + `TryItProxyErrors` ficaram verdes; Horario de oficina confirmaram verificacao de checksum concluida. |
| 2025-03-27 | Resumen de comentarios intermediario publicado | Resumen capturado en [`preview-feedback/w0/summary.md`](./preview-feedback/w0/summary.md); dois issues de nav menores tagueados como `docs-preview/w0`, sin incidentes. |
| 2025-03-31 | Checagem de telemetria da ultima semana | Ultimas horas de oficina previas a la salida; revisores confirmaram tarefas restantes em andamento, sem alertas. |
| 2025-04-08 | Resumen de dicha + encerramento de convites | Reviews completas confirmadas, acesso temporario revogado, achados arquivados em [`preview-feedback/w0/summary.md`](./preview-feedback/w0/summary.md#exit-summary-2025-04-08); tracker actualizado antes de preparar W1. |

## Registro de invitaciones - socios W1| ID de revisor | Papel | Boleto de solicitud | Convite enviado (UTC) | Saida esperada (UTC) | Estado | Notas |
| --- | --- | --- | --- | --- | --- | --- |
| sorafs-op-01 | Operador SoraFS (UE) | `DOCS-SORA-Preview-REQ-P01` | 2025-04-12 15:00 | 2025-04-26 15:00 | Concluido | Entregou feedback de ops do orquestador 2025-04-20; Responder a las 15:05 UTC. |
| sorafs-op-02 | Operador SoraFS (JP) | `DOCS-SORA-Preview-REQ-P02` | 2025-04-12 15:03 | 2025-04-26 15:00 | Concluido | Comentarios de implementación registrados en `docs-preview/w1`; respuesta a las 15:10 UTC. |
| sorafs-op-03 | Operador SoraFS (EE. UU.) | `DOCS-SORA-Preview-REQ-P03` | 2025-04-12 15:06 | 2025-04-26 15:00 | Concluido | Edicoes de disputa/lista negra registradas; respuesta a las 15:12 UTC. |
| torii-int-01 | Integrador Torii | `DOCS-SORA-Preview-REQ-P04` | 2025-04-12 15:09 | 2025-04-26 15:00 | Concluido | Tutorial de Pruébalo auth aceito; respuesta a las 15:14 UTC. |
| torii-int-02 | Integrador Torii | `DOCS-SORA-Preview-REQ-P05` | 2025-04-12 15:12 | 2025-04-26 15:00 | Concluido | Comentarios de RPC/OAuth registrados; respuesta a las 15:16 UTC. |
| socio-sdk-01 | Socio SDK (Swift) | `DOCS-SORA-Preview-REQ-P06` | 2025-04-12 15:15 | 2025-04-26 15:00 | Concluido | Comentarios de integridad de la vista previa combinada; respuesta a las 15:18 UTC. |
| socio-sdk-02 | Socio SDK (Android) | `DOCS-SORA-Preview-REQ-P07` | 2025-04-12 15:18 | 2025-04-26 15:00 | Concluido | Revisao de telemetria/redaction feita; respuesta a las 15:22 UTC. || puerta de enlace-ops-01 | Operador de puerta de enlace | `DOCS-SORA-Preview-REQ-P08` | 2025-04-12 15:21 | 2025-04-26 15:00 | Concluido | Comentarios del runbook de registros de puertas de enlace DNS; respuesta a las 15:24 UTC. |

## Registro de puntos de control - W1

| Datos (UTC) | Atividade | Notas |
| --- | --- | --- |
| 2025-04-12 | Envío de invitaciones + verificacao de artefatos | Los socios de Oito reciben el descriptor/archivo de correo electrónico `preview-2025-04-12`; acusa registrados no tracker. |
| 2025-04-13 | Revisión de línea base de telemetría | `docs.preview.integrity`, `TryItProxyErrors`, e `DocsPortal/GatewayRefusals` verdes; Horario de oficina confirmaram verificacao de checksum concluida. |
| 2025-04-18 | Horario de oficina de meio de onda | `docs.preview.integrity` permanece verde; dois nits de docs tagueados `docs-preview/w1` (redacción de navegación + captura de pantalla Pruébelo). |
| 2025-04-22 | Checagem final de telemetria | Proxy + paneles saudaveis; nenhuma issues nova, registrado no tracker antes de dicha. |
| 2025-04-26 | Resumen de dicha + encerramento de convites | Todos os partners confirmaram review, convites revogados, evidencia arquivada em [`preview-feedback/w1/summary.md`](./preview-feedback/w1/summary.md#exit-summary-2025-04-26). |

## Resumen de la corte beta W3- Convites enviados 2026-02-18 com verificacao de checksum + acuses registrados no mesmo dia.
- Comentarios recopilados en `docs-preview/20260218` con problema de gobierno `DOCS-SORA-Preview-20260218`; resumen + resumen gerados vía `npm run --prefix docs/portal preview:wave -- --wave preview-20260218`.
- Acceso revogado 2026-02-28 apos o check final de telemetria; tracker + tablas del portal actualizadas para marcar W3 como concluido.

## Registro de invitaciones - Comunidad W2| ID de revisor | Papel | Boleto de solicitud | Convite enviado (UTC) | Saida esperada (UTC) | Estado | Notas |
| --- | --- | --- | --- | --- | --- | --- |
| comunicación-vol-01 | Revisor de la comunidad (SDK) | `DOCS-SORA-Preview-REQ-C01` | 2025-06-15 16:00 | 2025-06-29 16:00 | Concluido | Confirmación a las 16:06 UTC; foco en inicios rápidos de SDK; dijo confirmada 2025-06-29. |
| comunicación-vol-02 | Revisor de la comunidad (Gobernanza) | `REQ-C02` | 2025-06-15 16:03 | 2025-06-29 16:00 | Concluido | Revisao de Gobernanza/SNS Feita; dijo confirmada 2025-06-29. |
| comunicación-vol-03 | Revisor de la comunidad (Norito) | `REQ-C03` | 2025-06-15 16:06 | 2025-06-29 16:00 | Concluido | Comentarios sobre el tutorial Norito registrado; confirmar 2025-06-29. |
| comunicación-vol-04 | Revisor de la comunidad (SoraFS) | `REQ-C04` | 2025-06-15 16:09 | 2025-06-29 16:00 | Concluido | Revisao de runbooks SoraFS feita; confirmar 2025-06-29. |
| comunicación-vol-05 | Revisor de la comunidad (Accesibilidad) | `REQ-C05` | 2025-06-15 16:12 | 2025-06-29 16:00 | Concluido | Notas de accesibilidad/UX compartiladas; confirmar 2025-06-29. |
| comunicación-vol-06 | Revisor de la comunidad (Localización) | `REQ-C06` | 2025-06-15 16:15 | 2025-06-29 16:00 | Concluido | Comentarios de localización registrado; confirmar 2025-06-29. || comunicación-vol-07 | Revisor de la comunidad (móvil) | `REQ-C07` | 2025-06-15 16:18 | 2025-06-29 16:00 | Concluido | Comprueba los documentos de entrada del móvil SDK; confirmar 2025-06-29. |
| comunicación-vol-08 | Revisor de la comunidad (Observabilidad) | `REQ-C08` | 2025-06-15 16:21 | 2025-06-29 16:00 | Concluido | Revisao de apendice de observabilidade feita; confirmar 2025-06-29. |

## Registro de puntos de control - W2| Datos (UTC) | Atividade | Notas |
| --- | --- | --- |
| 2025-06-15 | Envío de invitaciones + verificacao de artefatos | Descriptor/archivo `preview-2025-06-15` compartilhado com 8 revisores; acusa guardados sin tracker. |
| 2025-06-16 | Revisión de línea base de telemetría | `docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals` verdes; logs do proxy Pruébalo mostram tokens comunitarios ativos. |
| 2025-06-18 | Horarios de oficina y clasificación de problemas | Duas sugestoes (`docs-preview/w2 #1` texto de información sobre herramientas, `#2` barra lateral de localización): ambas atribuidas a Docs. |
| 2025-06-21 | Consulta de telemetria + correcciones de documentos | Documentos resueltos `docs-preview/w2 #1/#2`; Cuadros de mando verdes, sin incidentes. |
| 2025-06-24 | Horario de oficina de la última semana | Los revisores confirmarán los envíos restantes; nenhum alerta. |
| 2025-06-29 | Resumen de dicha + encerramento de convites | Acks registrados, acceso de vista previa revogado, snapshots + artefatos archivados (ver [`preview-feedback/w2/summary.md`](./preview-feedback/w2/summary.md#exit-summary-2025-06-29)). |
| 2025-04-15 | Horarios de oficina y clasificación de problemas | Duas sugerencias de documentacao registradas em `docs-preview/w1`; sem incidentes nem alertas. |

## Ganchos para informar- Toda quarta-feira, atualize a tabela acima e a issues de convites ativa com uma nota curta (convites enviados, revisores ativos, incidentes).
- Cuando una onda encerrar, agregue o caminho do resumo de feedback (por ejemplo, `docs/portal/docs/devportal/preview-feedback/w0/summary.md`) y enlace desde `status.md`.
- Se qualquer criterio de pausa do [preview invite flow](./preview-invite-flow.md) for acionado, adicione os passos de remediacao aqui antes de retomar os convites.