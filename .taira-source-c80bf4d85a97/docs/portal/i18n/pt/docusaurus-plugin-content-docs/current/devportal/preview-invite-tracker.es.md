---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/preview-invite-tracker.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Rastreador de convites de visualização

Este rastreador registra cada página de visualização do portal de documentos para que os proprietários de DOCS-SORA e os revisores de governança que coorte estejam ativados, que aprovem os convites e que os artefatos sigam pendentes. Atualize cada vez que você enviar, revogar ou diferentes convites para o rastro de auditório que está dentro do repositório.

##Estado de Olas

| Olá | Coorte | Edição de acompanhamento | Aprovador(es) | Estado | Ventana objetivo | Notas |
| --- | --- | --- | --- | --- | --- | --- |
| **W0 - Mantenedores principais** | Mantenedores de Docs + SDK validando o fluxo de checksum | `DOCS-SORA-Preview-W0` (rastreador GitHub/ops) | Documentos principais/DevRel + Portal TL | Concluído | 2º trimestre de 2025 semanas 1-2 | Convites enviados 2025-03-25, telemetria se mantuvo verde, resumo de salida publicado 2025-04-08. |
| **W1 - Parceiros** | Operadores SoraFS, integradores Torii bajo NDA | `DOCS-SORA-Preview-W1` | Lead Docs/DevRel + link de governo | Concluído | 2º trimestre de 2025, semana 3 | Convites 2025-04-12 -> 2025-04-26 com os outros parceiros confirmados; evidência capturada em [`preview-feedback/w1/log.md`](./preview-feedback/w1/log.md) e o resumo da saída em [`preview-feedback/w1/summary.md`](./preview-feedback/w1/summary.md). |
| **W2 - Comunidade** | Lista de espera comunitária curada (<=25 por vez) | `DOCS-SORA-Preview-W2` | Líder de Documentos/DevRel + gerente de comunidade | Concluído | 3º trimestre de 2025, semana 1 (tentativa) | Convites 2025-06-15 -> 2025-06-29 con telemetria verde todo o período; evidência + hallazgos capturados em [`preview-feedback/w2/summary.md`](./preview-feedback/w2/summary.md). |
| **W3 - Coortes beta** | Beta finanças/observabilidade + parceiro SDK + defensor do ecossistema | `DOCS-SORA-Preview-W3` | Lead Docs/DevRel + link de governo | Concluído | 1º trimestre de 2026, semana 8 | Convites 18/02/2026 -> 28/02/2026; resumo + dados do portal gerados via ola `preview-20260218` (ver [`preview-feedback/w3/summary.md`](./preview-feedback/w3/summary.md)). |

> Nota: coloque cada edição do rastreador com os tickets de solicitação de visualização e arquive-os abaixo do projeto `docs-portal-preview` para que as aprovações sigam se forem descubríveis.

## Tareas ativas (W0)

- Artefactos de comprovação atualizados (execução GitHub Actions `docs-portal-preview` 2025-03-24, descritor verificado via `scripts/preview_verify.sh` usando tag `preview-2025-03-24`).
- Linhas de base de telemetria capturadas (`docs.preview.integrity`, instantâneo de painéis `TryItProxyErrors` salvo na edição W0).
- Texto de divulgação bloqueado usando [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md) com visualização da tag `preview-2025-03-24`.
- Solicitações de ingresso registradas para os primeiros cinco mantenedores (tickets `DOCS-SORA-Preview-REQ-01` ... `-05`).
- Primeiros cinco convites enviados 2025-03-25 10:00-10:20 UTC após os sete dias consecutivos de telemetria verde; acusa salvos em `DOCS-SORA-Preview-W0`.
- Monitoramento de telemetria + horário de expediente do anfitrião (check-ins diários até 31/03/2025; registro de pontos de controle abaixo).
- Feedback de metade de ola / questões recopiladas e etiquetadas `docs-preview/w0` (ver [W0 digest](./preview-feedback/w0/summary.md)).
- Resumen de ola publicado + confirmações de saída de convites (pacote de saída fechado 2025-04-08; ver [W0 digest](./preview-feedback/w0/summary.md)).
- Ola beta W3 em seguida; futuras olas programadas segunda revisão de gobernanza.

## Resumo dos parceiros W1

- Aprovações legais e de governo. Adendo de parceiros firmado em 05/04/2025; avaliações de subidas em `DOCS-SORA-Preview-W1`.
- Telemetria + Experimente encenação. Ticket de mudança `OPS-TRYIT-147` executado em 2025-04-06 com snapshots Grafana de `docs.preview.integrity`, `TryItProxyErrors`, e `DocsPortal/GatewayRefusals` arquivados.
- Preparação de artefato + checksum. Pacote `preview-2025-04-12` selecionado; logs do descritor/checksum/sonda salvos em `artifacts/docs_preview/W1/preview-2025-04-12/`.
- Lista de convites + envio. Ocho solicitações de parceiros (`DOCS-SORA-Preview-REQ-P01...P08`) aprovadas; convites enviados 2025-04-12 15:00-15:21 UTC com acusações registradas por revisor.
- Instrumentação de feedback. Diários de horário comercial + postos de telemetria registrados; ver [`preview-feedback/w1/summary.md`](./preview-feedback/w1/summary.md) para o resumo.
- Roster final / log de saída. [`preview-feedback/w1/log.md`](./preview-feedback/w1/log.md) agora registre carimbos de data/hora de convite/ack, evidência de telemetria, exportações de questionário e punteros de artefatos em 26/04/2025 para que o governo possa reproduzir a ola.

## Log de convites - Mantenedores principais do W0| ID do revisor | Rolo | Ticket de solicitação | Convite enviado (UTC) | Saída esperada (UTC) | Estado | Notas |
| --- | --- | --- | --- | --- | --- | --- |
| docs-core-01 | Mantenedor do portal | `DOCS-SORA-Preview-REQ-01` | 25/03/2025 10:05 | 08/04/2025 10:00 | Ativo | Confirmação de verificação de soma de verificação; enfocado na revisão de navegação/barra lateral. |
| sdk-rust-01 | Líder do Rust SDK | `DOCS-SORA-Preview-REQ-02` | 25/03/2025 10:08 | 08/04/2025 10:00 | Ativo | Testando receitas do SDK + guias de início rápido do Norito. |
| sdk-js-01 | Mantenedor JS SDK | `DOCS-SORA-Preview-REQ-03` | 25/03/2025 10:12 | 08/04/2025 10:00 | Ativo | Validando console Try it + flujos ISO. |
| sorafs-ops-01 | Contato com o operador SoraFS | `DOCS-SORA-Preview-REQ-04` | 25/03/2025 10:15 | 08/04/2025 10:00 | Ativo | Auditando runbooks de SoraFS + documentos de orquestração. |
| observabilidade-01 | Observabilidade TL | `DOCS-SORA-Preview-REQ-05` | 25/03/2025 10:18 | 08/04/2025 10:00 | Ativo | Revisando apêndices de telemetria/incidentes; responsável pela cobertura do Alertmanager. |

Todos os convites referem-se ao mesmo artefato `docs-portal-preview` (ejeção 2025-03-24, tag `preview-2025-03-24`) e ao registro de verificação capturado em `DOCS-SORA-Preview-W0`. Qualquer alta/pausa deve ser registrada tanto na tabela anterior como na edição do rastreador antes de prosseguir para a próxima etapa.

## Log de pontos de verificação - W0

| Data (UTC) | Atividade | Notas |
| --- | --- | --- |
| 26/03/2025 | Revisão da linha base de telemetria + horário de atendimento | `docs.preview.integrity` + `TryItProxyErrors` se mantuvieron verdes; horário comercial confirma que todos os revisores completam a verificação da soma de verificação. |
| 27/03/2025 | Resumo de feedback intermediário publicado | Resumo capturado em [`preview-feedback/w0/summary.md`](./preview-feedback/w0/summary.md); dos problemas menores de navegação registrados como `docs-preview/w0`, sem incidentes reportados. |
| 31/03/2025 | Cheque de telemetria da última semana | Pré-saída do horário de expediente do Ultimas; revisores confirmam as tarefas restantes em curso, sem alertas. |
| 08/04/2025 | Currículo de saída + cierres de convites | Reviews completadas confirmadas, acesso temporal revogado, hallazgos arquivados em [`preview-feedback/w0/summary.md`](./preview-feedback/w0/summary.md#exit-summary-2025-04-08); rastreador atualizado antes de preparar W1. |

## Registro de convites - Parceiros W1

| ID do revisor | Rolo | Ticket de solicitação | Convite enviado (UTC) | Saída esperada (UTC) | Estado | Notas |
| --- | --- | --- | --- | --- | --- | --- |
| sorafs-op-01 | Operador SoraFS (UE) | `DOCS-SORA-Preview-REQ-P01` | 12/04/2025 15:00 | 26/04/2025 15:00 | Concluído | Entrego feedback de operações do orquestrador 2025/04/20; confirmação de saída às 15:05 UTC. |
| sorafs-op-02 | Operador SoraFS (JP) | `DOCS-SORA-Preview-REQ-P02` | 12/04/2025 15:03 | 26/04/2025 15:00 | Concluído | Registro de comentários de implementação em `docs-preview/w1`; retorno às 15:10 UTC. |
| sorafs-op-03 | Operador SoraFS (EUA) | `DOCS-SORA-Preview-REQ-P03` | 12/04/2025 15:06 | 26/04/2025 15:00 | Concluído | Edições de disputa/lista negra de marcas registradas; retorno às 15:12 UTC. |
| torii-int-01 | Integrador Torii | `DOCS-SORA-Preview-REQ-P04` | 12/04/2025 15:09 | 26/04/2025 15:00 | Concluído | Passo a passo de Try it auth aceito; retorno às 15h14 UTC. |
| torii-int-02 | Integrador Torii | `DOCS-SORA-Preview-REQ-P05` | 12/04/2025 15:12 | 26/04/2025 15:00 | Concluído | Comentários de registradores RPC/OAuth; retorno às 15h16 UTC. |
| sdk-parceiro-01 | Parceiro SDK (Swift) | `DOCS-SORA-Preview-REQ-P06` | 12/04/2025 15:15 | 26/04/2025 15:00 | Concluído | Feedback de integridade da visualização fundida; retorno às 15h18 UTC. |
| sdk-parceiro-02 | Parceiro SDK (Android) | `DOCS-SORA-Preview-REQ-P07` | 12/04/2025 15:18 | 26/04/2025 15:00 | Concluído | Revisão de telemetria/redação hecha; retorno às 15:22 UTC. |
| gateway-ops-01 | Operador de gateway | `DOCS-SORA-Preview-REQ-P08` | 12/04/2025 15:21 | 26/04/2025 15:00 | Concluído | Comentários do runbook de gateway DNS registrados; retorno às 15h24 UTC. |

## Registro de pontos de verificação - W1| Data (UTC) | Atividade | Notas |
| --- | --- | --- |
| 12/04/2025 | Envio de convites + verificação de artefatos | Os outros parceiros receberam e-mail com descritor/arquivo `preview-2025-04-12`; acusa registrados no rastreador. |
| 13/04/2025 | Revisão da linha de base da telemetria | `docs.preview.integrity`, `TryItProxyErrors`, e `DocsPortal/GatewayRefusals` em verde; horário comercial confirmaron verificação de checksum completada. |
| 18/04/2025 | Horário de atendimento de metade de ola | `docs.preview.integrity` se mantuvo verde; dos nits de documentos registrados como `docs-preview/w1` (texto de navegação + captura de tela de Try it). |
| 22/04/2025 | Cheque final de telemetria | Proxy + dashboards saludáveis; pecado emite novas, anotadas no rastreador antes de saída. |
| 2025/04/26 | Currículo de saída + cierres de convites | Todos os parceiros confirmam revisão, convites revogados, evidências arquivadas em [`preview-feedback/w1/summary.md`](./preview-feedback/w1/summary.md#exit-summary-2025-04-26). |

## Recapitulação da coorte beta W3

- Convites enviados 2026-02-18 com verificação de checksum + acusações registradas no mesmo dia.
- Feedback recopilado abaixo `docs-preview/20260218` com emissão de governo `DOCS-SORA-Preview-20260218`; resumo + currículo gerado via `npm run --prefix docs/portal preview:wave -- --wave preview-20260218`.
- Acesso revogado 2026-02-28 após o cheque final de telemetria; tracker + tablas del portal atualizadas para marcar W3 como completado.

## Registro de convites - comunidade W2

| ID do revisor | Rolo | Ticket de solicitação | Convite enviado (UTC) | Saída esperada (UTC) | Estado | Notas |
| --- | --- | --- | --- | --- | --- | --- |
| comm-vol-01 | Revisor da comunidade (SDK) | `DOCS-SORA-Preview-REQ-C01` | 15/06/2025 16:00 | 29/06/2025 16:00 | Concluído | Confirmação 16:06 UTC; focado em guias de início rápido do SDK; saída confirmada em 2025-06-29. |
| comm-vol-02 | Revisor comunitário (Governação) | `REQ-C02` | 15/06/2025 16:03 | 29/06/2025 16:00 | Concluído | Revisão da governança/SNS hecha; saída confirmada em 2025-06-29. |
| comm-vol-03 | Revisor da comunidade (Norito) | `REQ-C03` | 15/06/2025 16:06 | 29/06/2025 16:00 | Concluído | Feedback do passo a passo Norito registrado; confirmação 29/06/2025. |
| comm-vol-04 | Revisor da comunidade (SoraFS) | `REQ-C04` | 15/06/2025 16:09 | 29/06/2025 16:00 | Concluído | Revisão dos runbooks SoraFS hecha; confirmação 29/06/2025. |
| comm-vol-05 | Revisor da comunidade (Acessibilidade) | `REQ-C05` | 15/06/2025 16:12 | 29/06/2025 16:00 | Concluído | Notas de acessibilidade/UX compartilhadas; confirmação 29/06/2025. |
| comm-vol-06 | Revisor da comunidade (Localização) | `REQ-C06` | 15/06/2025 16:15 | 29/06/2025 16:00 | Concluído | Feedback de localização registrado; confirmação 29/06/2025. |
| comm-vol-07 | Revisor da comunidade (móvel) | `REQ-C07` | 15/06/2025 16:18 | 29/06/2025 16:00 | Concluído | Verifica os documentos do SDK mobile entregues; confirmação 29/06/2025. |
| comm-vol-08 | Revisor comunitário (Observabilidade) | `REQ-C08` | 15/06/2025 16:21 | 29/06/2025 16:00 | Concluído | Revisão do apêndice de observabilidade hecha; confirmação 29/06/2025. |

## Registro de pontos de verificação - W2

| Data (UTC) | Atividade | Notas |
| --- | --- | --- |
| 15/06/2025 | Envio de convites + verificação de artefatos | Descritor/arquivo `preview-2025-06-15` compartilhado com 8 revisores; acusa salvos no rastreador. |
| 16/06/2025 | Revisão da linha de base da telemetria | `docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals` em verde; logs del proxy Experimente exibir tokens comunitários ativos. |
| 18/06/2025 | Horário de atendimento e triagem de problemas | Duas sugestões (`docs-preview/w2 #1` texto da dica de ferramenta, `#2` barra lateral de localização) - ambas atribuídas ao Docs. |
| 21/06/2025 | Cheque de telemetria + correções de documentos | Resolução de documentos `docs-preview/w2 #1/#2`; painéis verdes, sem incidentes. |
| 24/06/2025 | Horário de atendimento da última semana | Revisores confirmam envios pendentes; não se dispararão alertas. |
| 29/06/2025 | Currículo de saída + cierres de convites | Acks registrados, acesso de visualização revogado, snapshots + artefatos arquivados (ver [`preview-feedback/w2/summary.md`](./preview-feedback/w2/summary.md#exit-summary-2025-06-29)). |
| 15/04/2025 | Horário de atendimento e triagem de problemas | Das sugestões de documentação registrada abaixo `docs-preview/w1`; pecados incidentes e alertas. |

## Ganchos de relatório- Cada miercoles, atualiza a tabela do rastreador e a emissão de convites ativa com uma nota curta de estado (convites enviados, revisores ativos, incidentes).
- Ao iniciar uma linha, adicione a rota do currículo de feedback (por exemplo, `docs/portal/docs/devportal/preview-feedback/w0/summary.md`) e comece a partir de `status.md`.
- Se os critérios de pausa de [visualização do fluxo de convite] forem ativados (./preview-invite-flow.md), adicione os passos de correção aqui antes de reanudar convites.