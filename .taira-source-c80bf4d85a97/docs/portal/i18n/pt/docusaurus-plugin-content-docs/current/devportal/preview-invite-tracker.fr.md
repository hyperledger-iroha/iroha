---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/preview-invite-tracker.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Visualização do rastreador de convites

Este rastreador registra toda vaga visualização dos documentos do portal para que os proprietários DOCS-SORA e os relecteurs gouvernance voient quelle cohorte estejam ativos, que aprovam os convites e os artefatos restantes a trair. Mande-o um dia sempre que os convites forem enviados, revogados ou reportados para que a pista de auditoria fique no depósito.

## Estatuto das vagas

| Vago | Coorte | Emissão de suivi | Aprovador(es) | Estatuto | Fenetre cible | Notas |
| --- | --- | --- | --- | --- | --- | --- |
| **W0 - Mantenedores principais** | Documentos dos mantenedores + SDK validam a soma de verificação do fluxo | `DOCS-SORA-Preview-W0` (rastreador GitHub/ops) | Documentos principais/DevRel + Portal TL | Término | Semanas 1-2 do 2º trimestre de 2025 | Convites enviados 2025-03-25, telemetrie restee verte, resume de sortie publie 2025-04-08. |
| **W1 - Parceiros** | Operadores SoraFS, integradores Torii sob NDA | `DOCS-SORA-Preview-W1` | Documentos principais/DevRel + governança de ligação | Término | Semana 3 do 2º trimestre de 2025 | Convites 12/04/2025 -> 26/04/2025 com confirmação dos parceiros da casa; evidências capturadas em [`preview-feedback/w1/log.md`](./preview-feedback/w1/log.md) e retomadas de surtida em [`preview-feedback/w1/summary.md`](./preview-feedback/w1/summary.md). |
| **W2 - Comunauta** | Lista de espera communauute triee (<=25 a la fois) | `DOCS-SORA-Preview-W2` | Líder de Documentos/DevRel + gerente de comunidade | Término | Semana 1 do terceiro trimestre de 2025 (tentativa) | Convites 15/06/2025 -> 29/06/2025 com telemetria verte tout du long; evidência + constantes em [`preview-feedback/w2/summary.md`](./preview-feedback/w2/summary.md). |
| **W3 - Coortes beta** | Beta financiamento/observabilidade + parceiro SDK + defensor do ecossistema | `DOCS-SORA-Preview-W3` | Documentos principais/DevRel + governança de ligação | Término | Semana 8 do primeiro trimestre de 2026 | Convites 18/02/2026 -> 28/02/2026; currículo + donnees portail gerados via vago `preview-20260218` (voir [`preview-feedback/w3/summary.md`](./preview-feedback/w3/summary.md)). |

> Nota: leia todo o problema do rastreador de tickets de visualização e arquive-os no projeto `docs-portal-preview` para que as aprovações permaneçam disponíveis.

## Taches ativos (W0)

- Artefatos de preflight rafraichis (execução GitHub Actions `docs-portal-preview` 2025-03-24, descritor verificado via `scripts/preview_verify.sh` com a tag `preview-2025-03-24`).
- Capturas de telemetria de linhas de base (`docs.preview.integrity`, instantâneo dos painéis `TryItProxyErrors` salvo na edição W0).
- Texto de divulgação com [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md) e visualização da tag `preview-2025-03-24`.
- Exigências de entradas registradas para os mantenedores cinq premiers (ingressos `DOCS-SORA-Preview-REQ-01` ... `-05`).
- Cinq estreia convites enviados 25/03/2025 10:00-10:20 UTC apres sept jours consecutifs de telemetrie verte; accus stockes em `DOCS-SORA-Preview-W0`.
- Telemetria suivi + horário de expediente do anfitrião (check-ins quotidiens jusqu'au 2025-03-31; log des checkpoints ci-dessous).
- Feedback mi-vago / questões coletadas e tags `docs-preview/w0` (ver [W0 digest](./preview-feedback/w0/summary.md)).
- Currículo de publicação vaga + confirmações de surtida (pacote de surtida data 08/04/2025; ver [W0 digest](./preview-feedback/w0/summary.md)).
- Suivie beta W3 vago; futuros vagos planejados selon revue gouvernance.

## Currículo dos parceiros W1 vagos

- Aprovações legais e governamentais. Adendo de parceiros assinados em 05/04/2025; taxas de aprovação em `DOCS-SORA-Preview-W1`.
- Telemetria + Experimente a encenação. A alteração do ticket `OPS-TRYIT-147` é executada em 2025-04-06 com snapshots Grafana de `docs.preview.integrity`, `TryItProxyErrors` e arquivos `DocsPortal/GatewayRefusals`.
- Artefato de preparação + checksum. Verificação do pacote `preview-2025-04-12`; logs descritor/checksum/probe armazenados em `artifacts/docs_preview/W1/preview-2025-04-12/`.
- Lista de convites + envio. Huit exige parceiros (`DOCS-SORA-Preview-REQ-P01...P08`) aprovados; convites enviados 2025-04-12 15:00-15:21 UTC com accus par relecteur.
- Feedback de instrumentação. Horário de expediente quotidiennes + postos de controle telemetrie enregistres; veja [`preview-feedback/w1/summary.md`](./preview-feedback/w1/summary.md) para o resumo.
- Roster final / log de surtida. [`preview-feedback/w1/log.md`](./preview-feedback/w1/log.md) registra carimbos de data e hora de convite/ack, telemetria de evidências, questionário de exportação e indicadores de artefatos em 2025-04-26 para permitir o controle da seleção.

## Log des Invitations - Mantenedores principais do W0| Receptor de identificação | Função | Ticket de demanda | Enviado de convite (UTC) | Participação na surtida (UTC) | Estatuto | Notas |
| --- | --- | --- | --- | --- | --- | --- |
| docs-core-01 | Mantenedor do portal | `DOCS-SORA-Preview-REQ-01` | 25/03/2025 10:05 | 08/04/2025 10:00 | Ativo | Uma confirmação da soma de verificação de verificação; foco na navegação/barra lateral. |
| sdk-rust-01 | Líder do Rust SDK | `DOCS-SORA-Preview-REQ-02` | 25/03/2025 10:08 | 08/04/2025 10:00 | Ativo | Teste as receitas SDK + guias de início rápido Norito. |
| sdk-js-01 | Mantenedor JS SDK | `DOCS-SORA-Preview-REQ-03` | 25/03/2025 10:12 | 08/04/2025 10:00 | Ativo | Válido para o console Experimente + fluxos ISO. |
| sorafs-ops-01 | Contato com o operador SoraFS | `DOCS-SORA-Preview-REQ-04` | 25/03/2025 10:15 | 08/04/2025 10:00 | Ativo | Audite os runbooks SoraFS + orquestração de documentos. |
| observabilidade-01 | Observabilidade TL | `DOCS-SORA-Preview-REQ-05` | 25/03/2025 10:18 | 08/04/2025 10:00 | Ativo | Revoit os anexos de telemetria/incidentes; Alertmanager de cobertura responsável. |

Todos os convites referem-se ao artefato meme `docs-portal-preview` (execução 2025-03-24, tag `preview-2025-03-24`) e ao log de captura de verificação em `DOCS-SORA-Preview-W0`. Toute ajout/pause doit etre consigne dans le tableau ci-dessus et l'issue du tracker antes de passar para a vaga seguinte.

## Registro de pontos de verificação - W0

| Data (UTC) | Atividade | Notas |
| --- | --- | --- |
| 26/03/2025 | Revue linha de base de telemetria + horário de expediente | `docs.preview.integrity` + `TryItProxyErrors` são verdes restantes; o horário comercial não confirma o término da verificação da soma de verificação. |
| 27/03/2025 | Digest feedback intermediaire public | Retomar captura em [`preview-feedback/w0/summary.md`](./preview-feedback/w0/summary.md); dois problemas nav mineures taguees `docs-preview/w0`, incidente aucun. |
| 31/03/2025 | Verifique telemetria no final da semana | Pré-saída do horário comercial de Dernieres; Os refletores não confirmarão as tachas restantes, mesmo que estejam alertas. |
| 08/04/2025 | Resumo de surtida + convites para fechamento do DES | Revisões demissionais confirmados, acesso temporário revogado, arquivos de estatísticas em [`preview-feedback/w0/summary.md`](./preview-feedback/w0/summary.md#exit-summary-2025-04-08); rastreador mis um dia antes do W1. |

## Registro de convites - Parceiros W1

| Receptor de identificação | Função | Ticket de demanda | Enviado de convite (UTC) | Participação na surtida (UTC) | Estatuto | Notas |
| --- | --- | --- | --- | --- | --- | --- |
| sorafs-op-01 | Operador SoraFS (UE) | `DOCS-SORA-Preview-REQ-P01` | 12/04/2025 15:00 | 26/04/2025 15:00 | Término | Orquestrador de operações de feedback livre 2025-04-20; sortida de confirmação às 15:05 UTC. |
| sorafs-op-02 | Operador SoraFS (JP) | `DOCS-SORA-Preview-REQ-P02` | 12/04/2025 15:03 | 26/04/2025 15:00 | Término | Comenta logs de implementação em `docs-preview/w1`; retorno às 15:10 UTC. |
| sorafs-op-03 | Operador SoraFS (EUA) | `DOCS-SORA-Preview-REQ-P03` | 12/04/2025 15:06 | 26/04/2025 15:00 | Término | Edita registros de disputas/listas negras; retorno às 15:12 UTC. |
| torii-int-01 | Integrador Torii | `DOCS-SORA-Preview-REQ-P04` | 12/04/2025 15:09 | 26/04/2025 15:00 | Término | Passo a passo Experimente, auth aceite; retorno às 15h14 UTC. |
| torii-int-02 | Integrador Torii | `DOCS-SORA-Preview-REQ-P05` | 12/04/2025 15:12 | 26/04/2025 15:00 | Término | Comenta logs RPC/OAuth; retorno às 15h16 UTC. |
| sdk-parceiro-01 | Parceiro SDK (Swift) | `DOCS-SORA-Preview-REQ-P06` | 12/04/2025 15:15 | 26/04/2025 15:00 | Término | Mesclagem de visualização integral de feedback; retorno às 15h18 UTC. |
| sdk-parceiro-02 | Parceiro SDK (Android) | `DOCS-SORA-Preview-REQ-P07` | 12/04/2025 15:18 | 26/04/2025 15:00 | Término | Revue telemetria/redação faité; retorno às 15:22 UTC. |
| gateway-ops-01 | Operador de gateway | `DOCS-SORA-Preview-REQ-P08` | 12/04/2025 15:21 | 26/04/2025 15:00 | Término | Comenta logs de gateway DNS do runbook; retorno às 15h24 UTC. |

## Registro de pontos de verificação - W1

| Data (UTC) | Atividade | Notas |
| --- | --- | --- |
| 12/04/2025 | Convites Envoi + artefatos de verificação | E-mails de parceiros Huit com descritor/arquivo `preview-2025-04-12`; acusa ações no rastreador. |
| 13/04/2025 | Revisão da linha de base da telemetria | `docs.preview.integrity`, `TryItProxyErrors`, e `DocsPortal/GatewayRefusals` verts; o horário comercial não confirma o término da verificação da soma de verificação. |
| 18/04/2025 | Horário de expediente mi-vago | `docs.preview.integrity` resto verde; deux nits docs tags `docs-preview/w1` (texto de navegação + captura de tela Experimente). |
| 22/04/2025 | Verifique a telemetria final | Proxy + dashboards santos; nenhum novo problema, anotado no rastreador antes da surtida. |
| 2025/04/26 | Resumo de surtida + convites de fermeture | Todos os parceiros confirmaram a revisão, convites revogados, evidências arquivadas em [`preview-feedback/w1/summary.md`](./preview-feedback/w1/summary.md#exit-summary-2025-04-26). |

## Recapitulação da coorte beta W3- Convites enviados 18/02/2026 com soma de verificação de verificação + accus le meme jour.
- Feedback coletado sob `docs-preview/20260218` com controle de emissão `DOCS-SORA-Preview-20260218`; resumo + resumo gerado via `npm run --prefix docs/portal preview:wave -- --wave preview-20260218`.
- Acesso revogado em 28/02/2026 após a verificação da telemetria final; rastreador + tabelas portail mises a jour pour marquer W3 termine.

## Registro de convites - comunidade W2

| Receptor de identificação | Função | Ticket de demanda | Enviado de convite (UTC) | Participação na surtida (UTC) | Estatuto | Notas |
| --- | --- | --- | --- | --- | --- | --- |
| comm-vol-01 | Revisor da comunidade (SDK) | `DOCS-SORA-Preview-REQ-C01` | 15/06/2025 16:00 | 29/06/2025 16:00 | Término | Confirmação 16:06 UTC; foco nos inícios rápidos SDK; sortie confirmada 2025-06-29. |
| comm-vol-02 | Revisor comunitário (Governação) | `REQ-C02` | 15/06/2025 16:03 | 29/06/2025 16:00 | Término | Revue gouvernance/SNS demitido; sortie confirmada 2025-06-29. |
| comm-vol-03 | Revisor da comunidade (Norito) | `REQ-C03` | 15/06/2025 16:06 | 29/06/2025 16:00 | Término | Passo a passo de feedback Registro Norito; confirmação 29/06/2025. |
| comm-vol-04 | Revisor da comunidade (SoraFS) | `REQ-C04` | 15/06/2025 16:09 | 29/06/2025 16:00 | Término | Revue runbooks SoraFS finalizado; confirmação 29/06/2025. |
| comm-vol-05 | Revisor da comunidade (Acessibilidade) | `REQ-C05` | 15/06/2025 16:12 | 29/06/2025 16:00 | Término | Observa participantes de acessibilidade/UX; confirmação 29/06/2025. |
| comm-vol-06 | Revisor da comunidade (Localização) | `REQ-C06` | 15/06/2025 16:15 | 29/06/2025 16:00 | Término | Registro de localização de feedback; confirmação 29/06/2025. |
| comm-vol-07 | Revisor da comunidade (móvel) | `REQ-C07` | 15/06/2025 16:18 | 29/06/2025 16:00 | Término | Verifica documentos SDK móveis livres; confirmação 29/06/2025. |
| comm-vol-08 | Revisor comunitário (Observabilidade) | `REQ-C08` | 15/06/2025 16:21 | 29/06/2025 16:00 | Término | Revue anexo observabilite terminee; confirmação 29/06/2025. |

## Registro de pontos de verificação - W2

| Data (UTC) | Atividade | Notas |
| --- | --- | --- |
| 15/06/2025 | Convites Envoi + artefatos de verificação | Descritor/arquivo `preview-2025-06-15` parte com 8 refletores; acusa ações no rastreador. |
| 16/06/2025 | Revisão da linha de base da telemetria | `docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals` verts; logs proxy Experimente montrent tokens communaute actifs. |
| 18/06/2025 | Horário comercial e questões de triagem | Sugestões Deux (dica de ferramenta de texto `docs-preview/w2 #1`, localização da barra lateral `#2`) - todos os dois destinatários do Docs. |
| 21/06/2025 | Verifique a documentação de telemetria + correções | Documenta uma correção `docs-preview/w2 #1/#2`; painéis verts, nenhum incidente. |
| 24/06/2025 | Horário de atendimento no final da semana | Os relecteurs não confirmam os retornos restantes; nenhum alerta. |
| 29/06/2025 | Resumo de surtida + convites de fermeture | Registros de reconhecimento, revogação de visualização de acesso, arquivos de instantâneos e artefatos (veja [`preview-feedback/w2/summary.md`](./preview-feedback/w2/summary.md#exit-summary-2025-06-29)). |
| 15/04/2025 | Horário comercial e questões de triagem | Duas sugestões de documentação registradas sob `docs-preview/w1`; nenhum incidente e alerta. |

## Ganchos de relatórios

- Chaque mercredi, mettre a jour le tableau ci-dessus et l'issue convida active avec une note courte (convites enviados, relecteurs actifs, incidentes).
- Ao terminar uma vaga, adicione o caminho do feedback de currículo (ex. `docs/portal/docs/devportal/preview-feedback/w0/summary.md`) e o lier depois de `status.md`.
- Se um critério de pausa do [fluxo de convite de pré-visualização](./preview-invite-flow.md) for desativado, adicione as etapas de remediação aqui antes de repetir os convites.