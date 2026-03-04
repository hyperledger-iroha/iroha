---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/preview-invite-tracker.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Tracker de convites do preview

Este rastreador registra cada onda de visualização do portal de documentos para que proprietários de DOCS-SORA e revisores de governança vejam qual coorte está ativa, quem aprovou os convites e quais artistas ainda precisam de atenção. Atualize-o sempre que os convites forem enviados, revogados ou adiados para que a trilha de auditorias permaneça no repositório.

## Status das ondas

| Onda | Coorte | Emissão de envio | Aprovador(es) | Estado | Janela alvo | Notas |
| --- | --- | --- | --- | --- | --- | --- |
| **W0 - Mantenedores principais** | Mantenedores de Docs + SDK validando o fluxo de checksum | `DOCS-SORA-Preview-W0` (rastreador GitHub/ops) | Documentos principais/DevRel + Portal TL | Concluído | 2º trimestre de 2025 semanas 1-2 | Convites enviados 2025-03-25, telemetria ficou verde, resumo de saida publicado 2025-04-08. |
| **W1 - Parceiros** | Operadores SoraFS, integradores Torii sob NDA | `DOCS-SORA-Preview-W1` | Lead Docs/DevRel + contato de governança | Concluído | 2º trimestre de 2025, semana 3 | Convites 2025-04-12 -> 2025-04-26 com os oito parceiros confirmados; evidência capturada em [`preview-feedback/w1/log.md`](./preview-feedback/w1/log.md) e resumo de saida em [`preview-feedback/w1/summary.md`](./preview-feedback/w1/summary.md). |
| **W2 - Comunidade** | Lista de espera comunitária curada (<=25 por vez) | `DOCS-SORA-Preview-W2` | Líder de Documentos/DevRel + gerente de comunidade | Concluído | 3º trimestre de 2025, semana 1 (tentativa) | Convites 2025-06-15 -> 2025-06-29 com telemetria verde o período todo; evidência + achados em [`preview-feedback/w2/summary.md`](./preview-feedback/w2/summary.md). |
| **W3 - Coortes beta** | Beta de finanças/observabilidade + parceiro SDK + defensor do ecossistema | `DOCS-SORA-Preview-W3` | Lead Docs/DevRel + contato de governança | Concluído | 1º trimestre de 2026, semana 8 | Convites 2026-02-18 -> 2026-02-28; resumo + dados do portal gerados via onda `preview-20260218` (ver [`preview-feedback/w3/summary.md`](./preview-feedback/w3/summary.md)). |

> Nota: vincule cada issue do tracker aos tickets de solicitação de preview e arquive-os no projeto `docs-portal-preview` para que as aprovações continuem descobertas.

## Tarefas ativas (W0)

- Artefatos de comprovação atualizados (execução GitHub Actions `docs-portal-preview` 2025-03-24, descritor verificado via `scripts/preview_verify.sh` com tag `preview-2025-03-24`).
- Baselines de telemetria capturada (`docs.preview.integrity`, snapshot dos dashboards `TryItProxyErrors` salvo na issue W0).
- Texto de divulgação trabalhado usando [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md) com tag preview `preview-2025-03-24`.
- Solicitações de entrada registradas para os primeiros cinco mantenedores (tickets `DOCS-SORA-Preview-REQ-01` ... `-05`).
- Cinco primeiros convites enviados 2025-03-25 10:00-10:20 UTC após sete dias consecutivos de telemetria verde; acusa guardados em `DOCS-SORA-Preview-W0`.
- Monitoramento de telemetria + horário de atendimento do anfitrião (check-ins diários até 31/03/2025; log de checkpoints abaixo).
- Feedback de meio de onda / issue coletadas e tagueadas `docs-preview/w0` (ver [W0 digest](./preview-feedback/w0/summary.md)).
- Resumo da onda publicado + confirmações de saida (bundle de saida datado 2025-04-08; ver [W0 digest](./preview-feedback/w0/summary.md)).
- Onda beta W3 acompanhada; futuras ondas agendadas conforme revisão de governança.

## Resumo da onda W1 parceiros

- Aprovações legais e de governança. Adendo de parceiros assinado 2025-04-05; aprovações enviadas para `DOCS-SORA-Preview-W1`.
- Telemetria + Experimente encenação. Ticket de mudança `OPS-TRYIT-147` executado 2025-04-06 com snapshots Grafana de `docs.preview.integrity`, `TryItProxyErrors`, e `DocsPortal/GatewayRefusals` arquivados.
- Preparação de artistas + checksum. Pacote `preview-2025-04-12` selecionado; registra os salvos do descritor/checksum/sonda em `artifacts/docs_preview/W1/preview-2025-04-12/`.
- Lista de convites + envio. Oito solicitações de parceiros (`DOCS-SORA-Preview-REQ-P01...P08`) aprovados; convites enviados 2025-04-12 15:00-15:21 UTC com acuses registrados por revisor.
- Instrumentação de feedback. Diários de horário comercial + postos de telemetria registrados; ver [`preview-feedback/w1/summary.md`](./preview-feedback/w1/summary.md) para o resumo.
- Lista final / log de saida. [`preview-feedback/w1/log.md`](./preview-feedback/w1/log.md) registra timestamps de convite/ack, evidência de telemetria, exportações de quiz e ponteiros de artefatos em 26/04/2025 para que a governança possa reproduzir a onda.

## Log de convites - mantenedores principais do W0| ID do revisor | Papel | Ticket de solicitação | Convite enviado (UTC) | Saida esperada (UTC) | Estado | Notas |
| --- | --- | --- | --- | --- | --- | --- |
| docs-core-01 | Mantenedor do portal | `DOCS-SORA-Preview-REQ-01` | 25/03/2025 10:05 | 08/04/2025 10:00 | Ativo | Confirmou verificação de checksum; foco em revisão de navegação/barra lateral. |
| sdk-rust-01 | Líder do Rust SDK | `DOCS-SORA-Preview-REQ-02` | 25/03/2025 10:08 | 08/04/2025 10:00 | Ativo | Testando receitas de SDK + quickstarts de Norito. |
| sdk-js-01 | Mantenedor JS SDK | `DOCS-SORA-Preview-REQ-03` | 25/03/2025 10:12 | 08/04/2025 10:00 | Ativo | Validando console Experimente + fluxos ISO. |
| sorafs-ops-01 | Contato com o operador SoraFS | `DOCS-SORA-Preview-REQ-04` | 25/03/2025 10:15 | 08/04/2025 10:00 | Ativo | Auditando runbooks SoraFS + docs de orquestracao. |
| observabilidade-01 | Observabilidade TL | `DOCS-SORA-Preview-REQ-05` | 25/03/2025 10:18 | 08/04/2025 10:00 | Ativo | Revisando apêndices de telemetria/incidentes; responsavel pela cobertura do Alertmanager. |

Todos os convites referenciaram os mesmos artefatos `docs-portal-preview` (execução 2025-03-24, tag `preview-2025-03-24`) e o log de verificação capturado em `DOCS-SORA-Preview-W0`. Qualquer indicação/pausa deve ser registrada tanto na tabela acima quanto na emissão do tracker antes de passar para a próxima onda.

## Log de pontos de verificação - W0

| Dados (UTC) | Atividade | Notas |
| --- | --- | --- |
| 26/03/2025 | Revisão de base de telemetria + horário de atendimento | `docs.preview.integrity` + `TryItProxyErrors` ficam verdes; horário comercial confirmaram verificação de checksum concluída. |
| 27/03/2025 | Digest de feedback intermediário publicado | Resumo capturado em [`preview-feedback/w0/summary.md`](./preview-feedback/w0/summary.md); dois problemas de navegação menores marcados como `docs-preview/w0`, sem incidentes. |
| 31/03/2025 | Checagem de telemetria da última semana | Pré-saída do horário de expediente do Ultimas; revisores confirmaram tarefas restantes em andamento, sem alertas. |
| 08/04/2025 | Resumo de saida + encerramento de convites | Resenhas completas confirmadas, acesso temporário revogado, achados arquivados em [`preview-feedback/w0/summary.md`](./preview-feedback/w0/summary.md#exit-summary-2025-04-08); tracker atualizado antes de preparar W1. |

## Log de convites - Parceiros W1

| ID do revisor | Papel | Ticket de solicitação | Convite enviado (UTC) | Saida esperada (UTC) | Estado | Notas |
| --- | --- | --- | --- | --- | --- | --- |
| sorafs-op-01 | Operador SoraFS (UE) | `DOCS-SORA-Preview-REQ-P01` | 12/04/2025 15:00 | 26/04/2025 15:00 | Concluído | Entregou feedback de operações do orquestrador 2025/04/20; ack de saida 15:05 UTC. |
| sorafs-op-02 | Operador SoraFS (JP) | `DOCS-SORA-Preview-REQ-P02` | 12/04/2025 15:03 | 26/04/2025 15:00 | Concluído | Comentários de lançamento registrados em `docs-preview/w1`; retorno às 15:10 UTC. |
| sorafs-op-03 | Operador SoraFS (EUA) | `DOCS-SORA-Preview-REQ-P03` | 12/04/2025 15:06 | 26/04/2025 15:00 | Concluído | Edições de disputa/lista negra de marcas registradas; retorno às 15:12 UTC. |
| torii-int-01 | Integrador Torii | `DOCS-SORA-Preview-REQ-P04` | 12/04/2025 15:09 | 26/04/2025 15:00 | Concluído | Passo a passo de Try it auth aceito; retorno às 15h14 UTC. |
| torii-int-02 | Integrador Torii | `DOCS-SORA-Preview-REQ-P05` | 12/04/2025 15:12 | 26/04/2025 15:00 | Concluído | Comentários de registradores RPC/OAuth; retorno às 15h16 UTC. |
| sdk-parceiro-01 | Parceiro SDK (Swift) | `DOCS-SORA-Preview-REQ-P06` | 12/04/2025 15:15 | 26/04/2025 15:00 | Concluído | Feedback de integridade da visualização mesclada; retorno às 15h18 UTC. |
| sdk-parceiro-02 | Parceiro SDK (Android) | `DOCS-SORA-Preview-REQ-P07` | 12/04/2025 15:18 | 26/04/2025 15:00 | Concluído | Revisão de telemetria/redação feita; retorno às 15:22 UTC. |
| gateway-ops-01 | Operador de gateway | `DOCS-SORA-Preview-REQ-P08` | 12/04/2025 15:21 | 26/04/2025 15:00 | Concluído | Comentários do runbook de gateway DNS registrados; retorno às 15h24 UTC. |

## Registro de pontos de verificação - W1| Dados (UTC) | Atividade | Notas |
| --- | --- | --- |
| 12/04/2025 | Envio de convites + verificação de artefactos | Parceiros Oito receberam email com descritor/arquivo `preview-2025-04-12`; acusa registrados de não ter rastreador. |
| 13/04/2025 | Revisão da linha de base da telemetria | `docs.preview.integrity`, `TryItProxyErrors`, e `DocsPortal/GatewayRefusals` verdes; horário comercial confirmaram verificação de checksum concluída. |
| 18/04/2025 | Horário de atendimento de meio de onda | `docs.preview.integrity` quartos verdes; dois nits de docs tagueados `docs-preview/w1` (texto de navegação + captura de tela Experimente). |
| 22/04/2025 | Checagem final de telemetria | Proxy + dashboards saudáveis; nenhuma issue nova, registrada no tracker antes da saida. |
| 2025/04/26 | Resumo de saida + encerramento de convites | Todos os parceiros confirmaram revisão, convites revogados, evidências arquivadas em [`preview-feedback/w1/summary.md`](./preview-feedback/w1/summary.md#exit-summary-2025-04-26). |

## Recapitulação da coorte beta W3

- Convites enviados 2026-02-18 com verificação de checksum + acusações registradas no mesmo dia.
- Feedback coletado em `docs-preview/20260218` com questão de governança `DOCS-SORA-Preview-20260218`; resumo + resumo gerado via `npm run --prefix docs/portal preview:wave -- --wave preview-20260218`.
- Acesso revogado 2026-02-28 após o check final de telemetria; tracker + tabelas do portal atualizadas para marcar W3 como concluído.

## Log de convites - comunidade W2

| ID do revisor | Papel | Ticket de solicitação | Convite enviado (UTC) | Saida esperada (UTC) | Estado | Notas |
| --- | --- | --- | --- | --- | --- | --- |
| comm-vol-01 | Revisor da comunidade (SDK) | `DOCS-SORA-Preview-REQ-C01` | 15/06/2025 16:00 | 29/06/2025 16:00 | Concluído | Confirmação 16:06 UTC; foco em guias de início rápido do SDK; disse confirmada em 2025-06-29. |
| comm-vol-02 | Revisor comunitário (Governação) | `REQ-C02` | 15/06/2025 16:03 | 29/06/2025 16:00 | Concluído | Revisão de governança/SNS feita; disse confirmada em 2025-06-29. |
| comm-vol-03 | Revisor da comunidade (Norito) | `REQ-C03` | 15/06/2025 16:06 | 29/06/2025 16:00 | Concluído | Feedback do passo a passo Norito registrado; confirmação 29/06/2025. |
| comm-vol-04 | Revisor da comunidade (SoraFS) | `REQ-C04` | 15/06/2025 16:09 | 29/06/2025 16:00 | Concluído | Revisão de runbooks SoraFS feita; confirmação 29/06/2025. |
| comm-vol-05 | Revisor da comunidade (Acessibilidade) | `REQ-C05` | 15/06/2025 16:12 | 29/06/2025 16:00 | Concluído | Notas de acessibilidade/UX compartilhadas; confirmação 29/06/2025. |
| comm-vol-06 | Revisor da comunidade (Localização) | `REQ-C06` | 15/06/2025 16:15 | 29/06/2025 16:00 | Concluído | Feedback de localização registrada; confirmação 29/06/2025. |
| comm-vol-07 | Revisor da comunidade (móvel) | `REQ-C07` | 15/06/2025 16:18 | 29/06/2025 16:00 | Concluído | Verificação de documentos de SDK mobile entregues; confirmação 29/06/2025. |
| comm-vol-08 | Revisor comunitário (Observabilidade) | `REQ-C08` | 15/06/2025 16:21 | 29/06/2025 16:00 | Concluído | Revisão de apêndice de observabilidade feita; confirmação 29/06/2025. |

## Registro de pontos de verificação - W2

| Dados (UTC) | Atividade | Notas |
| --- | --- | --- |
| 15/06/2025 | Envio de convites + verificação de artefactos | Descritor/arquivo `preview-2025-06-15` compartilhado com 8 revisores; acusa guardados no rastreador. |
| 16/06/2025 | Revisão da linha de base da telemetria | `docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals` verdes; logs do proxy Experimente mostrar tokens comunitários ativos. |
| 18/06/2025 | Horário de atendimento e triagem de problemas | Duas sugestões (`docs-preview/w2 #1` texto da dica de ferramenta, `#2` barra lateral de localização) - ambas atribuídas aos Docs. |
| 21/06/2025 | Checagem de telemetria + correções de documentos | Documentos resolvidos `docs-preview/w2 #1/#2`; dashboards verdes, sem incidentes. |
| 24/06/2025 | Horário de atendimento da última semana | Os revisores confirmaram os envios restantes; nenhum alerta. |
| 29/06/2025 | Resumo de saida + encerramento de convites | Acks registrados, acesso de preview revogado, snapshots + arquitetos arquivados (ver [`preview-feedback/w2/summary.md`](./preview-feedback/w2/summary.md#exit-summary-2025-06-29)). |
| 15/04/2025 | Horário de atendimento e triagem de problemas | Duas sugestões de documentação registrada em `docs-preview/w1`; sem incidentes nem alertas. |

## Ganchos de relatórios

- Hoje quarta-feira, atualizar a tabela acima e a edição de convites ativos com uma nota curta (convites enviados, revisores ativos, incidentes).
- Quando uma onda encerrar, adicione o caminho do resumo de feedback (por exemplo, `docs/portal/docs/devportal/preview-feedback/w0/summary.md`) e linke a partir de `status.md`.
- Se qualquer critério de pausa do [fluxo de pré-visualização do convite](./preview-invite-flow.md) para o acionado, adicione os passos de remediação aqui antes de retomar os convites.