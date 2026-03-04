---
lang: ja
direction: ltr
source: docs/portal/i18n/pt/docusaurus-plugin-content-docs/current/devportal/preview-invite-tracker.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 32b86b03b18f8a8fb7f45613663da4be3979c942c924a337adb6da2c82b728bc
source_last_modified: "2026-01-03T18:08:01+00:00"
translation_last_reviewed: 2026-01-30
---

---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/preview-invite-tracker.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# Tracker de convites do preview

Este tracker registra cada onda de preview do portal de docs para que owners de DOCS-SORA e revisores de governanca vejam qual coorte esta ativa, quem aprovou os convites e quais artefatos ainda precisam de atencao. Atualize-o sempre que convites forem enviados, revogados ou adiados para que a trilha de auditoria permaneca no repositorio.

## Status das ondas

| Onda | Coorte | Issue de acompanhamento | Aprovador(es) | Status | Janela alvo | Notas |
| --- | --- | --- | --- | --- | --- | --- |
| **W0 - Core maintainers** | Maintainers de Docs + SDK validando o fluxo de checksum | `DOCS-SORA-Preview-W0` (GitHub/ops tracker) | Lead Docs/DevRel + Portal TL | Concluido | Q2 2025 semanas 1-2 | Convites enviados 2025-03-25, telemetria ficou verde, resumo de saida publicado 2025-04-08. |
| **W1 - Partners** | Operadores SoraFS, integradores Torii sob NDA | `DOCS-SORA-Preview-W1` | Lead Docs/DevRel + liaison de governanca | Concluido | Q2 2025 semana 3 | Convites 2025-04-12 -> 2025-04-26 com os oito partners confirmados; evidencia capturada em [`preview-feedback/w1/log.md`](./preview-feedback/w1/log.md) e resumo de saida em [`preview-feedback/w1/summary.md`](./preview-feedback/w1/summary.md). |
| **W2 - Comunidade** | Waitlist comunitaria curada (<=25 por vez) | `DOCS-SORA-Preview-W2` | Lead Docs/DevRel + community manager | Concluido | Q3 2025 semana 1 (tentativo) | Convites 2025-06-15 -> 2025-06-29 com telemetria verde o periodo todo; evidencia + achados em [`preview-feedback/w2/summary.md`](./preview-feedback/w2/summary.md). |
| **W3 - Coortes beta** | Beta de financas/observabilidade + partner SDK + advocate do ecossistema | `DOCS-SORA-Preview-W3` | Lead Docs/DevRel + liaison de governanca | Concluido | Q1 2026 semana 8 | Convites 2026-02-18 -> 2026-02-28; resumo + dados do portal gerados via onda `preview-20260218` (ver [`preview-feedback/w3/summary.md`](./preview-feedback/w3/summary.md)). |

> Nota: vincule cada issue do tracker aos tickets de solicitacao de preview e arquive-os no projeto `docs-portal-preview` para que as aprovacoes continuem descobriveis.

## Tarefas ativas (W0)

- Artefatos de preflight atualizados (execucao GitHub Actions `docs-portal-preview` 2025-03-24, descriptor verificado via `scripts/preview_verify.sh` com tag `preview-2025-03-24`).
- Baselines de telemetria capturados (`docs.preview.integrity`, snapshot dos dashboards `TryItProxyErrors` salvo na issue W0).
- Texto de outreach travado usando [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md) com tag preview `preview-2025-03-24`.
- Solicitacoes de entrada registradas para os primeiros cinco maintainers (tickets `DOCS-SORA-Preview-REQ-01` ... `-05`).
- Cinco primeiros convites enviados 2025-03-25 10:00-10:20 UTC apos sete dias consecutivos de telemetria verde; acuses guardados em `DOCS-SORA-Preview-W0`.
- Monitoramento de telemetria + office hours do host (check-ins diarios ate 2025-03-31; log de checkpoints abaixo).
- Feedback de meio de onda / issues coletadas e tagueadas `docs-preview/w0` (ver [W0 digest](./preview-feedback/w0/summary.md)).
- Resumo da onda publicado + confirmacoes de saida (bundle de saida datado 2025-04-08; ver [W0 digest](./preview-feedback/w0/summary.md)).
- Onda beta W3 acompanhada; futuras ondas agendadas conforme revisao de governanca.

## Resumo da onda W1 partners

- Aprovacoes legais e de governanca. Addendum de partners assinado 2025-04-05; aprovacoes enviadas para `DOCS-SORA-Preview-W1`.
- Telemetria + Try it staging. Ticket de mudanca `OPS-TRYIT-147` executado 2025-04-06 com snapshots Grafana de `docs.preview.integrity`, `TryItProxyErrors`, e `DocsPortal/GatewayRefusals` arquivados.
- Preparacao de artefato + checksum. Bundle `preview-2025-04-12` verificado; logs de descriptor/checksum/probe salvos em `artifacts/docs_preview/W1/preview-2025-04-12/`.
- Roster de convites + envio. Oito solicitacoes de partners (`DOCS-SORA-Preview-REQ-P01...P08`) aprovadas; convites enviados 2025-04-12 15:00-15:21 UTC com acuses registrados por revisor.
- Instrumentacao de feedback. Office hours diarias + checkpoints de telemetria registrados; ver [`preview-feedback/w1/summary.md`](./preview-feedback/w1/summary.md) para o digest.
- Roster final / log de saida. [`preview-feedback/w1/log.md`](./preview-feedback/w1/log.md) registra timestamps de convite/ack, evidencia de telemetria, exports de quiz e ponteiros de artefatos em 2025-04-26 para que a governanca possa reproduzir a onda.

## Log de convites - W0 core maintainers

| ID de revisor | Papel | Ticket de solicitacao | Convite enviado (UTC) | Saida esperada (UTC) | Status | Notas |
| --- | --- | --- | --- | --- | --- | --- |
| docs-core-01 | Portal maintainer | `DOCS-SORA-Preview-REQ-01` | 2025-03-25 10:05 | 2025-04-08 10:00 | Ativo | Confirmou verificacao de checksum; foco em revisao de nav/sidebar. |
| sdk-rust-01 | Rust SDK lead | `DOCS-SORA-Preview-REQ-02` | 2025-03-25 10:08 | 2025-04-08 10:00 | Ativo | Testando receitas de SDK + quickstarts de Norito. |
| sdk-js-01 | JS SDK maintainer | `DOCS-SORA-Preview-REQ-03` | 2025-03-25 10:12 | 2025-04-08 10:00 | Ativo | Validando console Try it + fluxos ISO. |
| sorafs-ops-01 | SoraFS operator liaison | `DOCS-SORA-Preview-REQ-04` | 2025-03-25 10:15 | 2025-04-08 10:00 | Ativo | Auditando runbooks SoraFS + docs de orquestracao. |
| observability-01 | Observability TL | `DOCS-SORA-Preview-REQ-05` | 2025-03-25 10:18 | 2025-04-08 10:00 | Ativo | Revisando apendices de telemetria/incidentes; responsavel pela cobertura de Alertmanager. |

Todos os convites referenciam o mesmo artefato `docs-portal-preview` (execucao 2025-03-24, tag `preview-2025-03-24`) e o log de verificacao capturado em `DOCS-SORA-Preview-W0`. Qualquer adicao/pausa deve ser registrada tanto na tabela acima quanto na issue do tracker antes de passar para a proxima onda.

## Log de checkpoints - W0

| Data (UTC) | Atividade | Notas |
| --- | --- | --- |
| 2025-03-26 | Revisao de telemetria baseline + office hours | `docs.preview.integrity` + `TryItProxyErrors` ficaram verdes; office hours confirmaram verificacao de checksum concluida. |
| 2025-03-27 | Digest de feedback intermediario publicado | Resumo capturado em [`preview-feedback/w0/summary.md`](./preview-feedback/w0/summary.md); dois issues de nav menores tagueados como `docs-preview/w0`, sem incidentes. |
| 2025-03-31 | Checagem de telemetria da ultima semana | Ultimas office hours pre-exit; revisores confirmaram tarefas restantes em andamento, sem alertas. |
| 2025-04-08 | Resumo de saida + encerramento de convites | Reviews completas confirmadas, acesso temporario revogado, achados arquivados em [`preview-feedback/w0/summary.md`](./preview-feedback/w0/summary.md#exit-summary-2025-04-08); tracker atualizado antes de preparar W1. |

## Log de convites - W1 partners

| ID de revisor | Papel | Ticket de solicitacao | Convite enviado (UTC) | Saida esperada (UTC) | Status | Notas |
| --- | --- | --- | --- | --- | --- | --- |
| sorafs-op-01 | SoraFS operator (EU) | `DOCS-SORA-Preview-REQ-P01` | 2025-04-12 15:00 | 2025-04-26 15:00 | Concluido | Entregou feedback de ops do orquestrador 2025-04-20; ack de saida 15:05 UTC. |
| sorafs-op-02 | SoraFS operator (JP) | `DOCS-SORA-Preview-REQ-P02` | 2025-04-12 15:03 | 2025-04-26 15:00 | Concluido | Comentarios de rollout registrados em `docs-preview/w1`; ack 15:10 UTC. |
| sorafs-op-03 | SoraFS operator (US) | `DOCS-SORA-Preview-REQ-P03` | 2025-04-12 15:06 | 2025-04-26 15:00 | Concluido | Edicoes de dispute/blacklist registradas; ack 15:12 UTC. |
| torii-int-01 | Torii integrator | `DOCS-SORA-Preview-REQ-P04` | 2025-04-12 15:09 | 2025-04-26 15:00 | Concluido | Walkthrough de Try it auth aceito; ack 15:14 UTC. |
| torii-int-02 | Torii integrator | `DOCS-SORA-Preview-REQ-P05` | 2025-04-12 15:12 | 2025-04-26 15:00 | Concluido | Comentarios de RPC/OAuth registrados; ack 15:16 UTC. |
| sdk-partner-01 | SDK partner (Swift) | `DOCS-SORA-Preview-REQ-P06` | 2025-04-12 15:15 | 2025-04-26 15:00 | Concluido | Feedback de integridade do preview mergeado; ack 15:18 UTC. |
| sdk-partner-02 | SDK partner (Android) | `DOCS-SORA-Preview-REQ-P07` | 2025-04-12 15:18 | 2025-04-26 15:00 | Concluido | Revisao de telemetria/redaction feita; ack 15:22 UTC. |
| gateway-ops-01 | Gateway operator | `DOCS-SORA-Preview-REQ-P08` | 2025-04-12 15:21 | 2025-04-26 15:00 | Concluido | Comentarios do runbook de DNS gateway registrados; ack 15:24 UTC. |

## Log de checkpoints - W1

| Data (UTC) | Atividade | Notas |
| --- | --- | --- |
| 2025-04-12 | Envio de convites + verificacao de artefatos | Oito partners receberam email com descriptor/archive `preview-2025-04-12`; acuses registrados no tracker. |
| 2025-04-13 | Revisao de telemetria baseline | `docs.preview.integrity`, `TryItProxyErrors`, e `DocsPortal/GatewayRefusals` verdes; office hours confirmaram verificacao de checksum concluida. |
| 2025-04-18 | Office hours de meio de onda | `docs.preview.integrity` permaneceu verde; dois nits de docs tagueados `docs-preview/w1` (nav wording + screenshot Try it). |
| 2025-04-22 | Checagem final de telemetria | Proxy + dashboards saudaveis; nenhuma issue nova, registrado no tracker antes da saida. |
| 2025-04-26 | Resumo de saida + encerramento de convites | Todos os partners confirmaram review, convites revogados, evidencia arquivada em [`preview-feedback/w1/summary.md`](./preview-feedback/w1/summary.md#exit-summary-2025-04-26). |

## Recap da coorte beta W3

- Convites enviados 2026-02-18 com verificacao de checksum + acuses registrados no mesmo dia.
- Feedback coletado em `docs-preview/20260218` com issue de governanca `DOCS-SORA-Preview-20260218`; digest + resumo gerados via `npm run --prefix docs/portal preview:wave -- --wave preview-20260218`.
- Acesso revogado 2026-02-28 apos o check final de telemetria; tracker + tabelas do portal atualizadas para marcar W3 como concluido.

## Log de convites - W2 community

| ID de revisor | Papel | Ticket de solicitacao | Convite enviado (UTC) | Saida esperada (UTC) | Status | Notas |
| --- | --- | --- | --- | --- | --- | --- |
| comm-vol-01 | Community reviewer (SDK) | `DOCS-SORA-Preview-REQ-C01` | 2025-06-15 16:00 | 2025-06-29 16:00 | Concluido | Ack 16:06 UTC; foco em quickstarts de SDK; saida confirmada 2025-06-29. |
| comm-vol-02 | Community reviewer (Governance) | `REQ-C02` | 2025-06-15 16:03 | 2025-06-29 16:00 | Concluido | Revisao de governanca/SNS feita; saida confirmada 2025-06-29. |
| comm-vol-03 | Community reviewer (Norito) | `REQ-C03` | 2025-06-15 16:06 | 2025-06-29 16:00 | Concluido | Feedback do walkthrough Norito registrado; ack 2025-06-29. |
| comm-vol-04 | Community reviewer (SoraFS) | `REQ-C04` | 2025-06-15 16:09 | 2025-06-29 16:00 | Concluido | Revisao de runbooks SoraFS feita; ack 2025-06-29. |
| comm-vol-05 | Community reviewer (Accessibility) | `REQ-C05` | 2025-06-15 16:12 | 2025-06-29 16:00 | Concluido | Notas de acessibilidade/UX compartilhadas; ack 2025-06-29. |
| comm-vol-06 | Community reviewer (Localization) | `REQ-C06` | 2025-06-15 16:15 | 2025-06-29 16:00 | Concluido | Feedback de localizacao registrado; ack 2025-06-29. |
| comm-vol-07 | Community reviewer (Mobile) | `REQ-C07` | 2025-06-15 16:18 | 2025-06-29 16:00 | Concluido | Checks de docs de SDK mobile entregues; ack 2025-06-29. |
| comm-vol-08 | Community reviewer (Observability) | `REQ-C08` | 2025-06-15 16:21 | 2025-06-29 16:00 | Concluido | Revisao de apendice de observabilidade feita; ack 2025-06-29. |

## Log de checkpoints - W2

| Data (UTC) | Atividade | Notas |
| --- | --- | --- |
| 2025-06-15 | Envio de convites + verificacao de artefatos | Descriptor/archive `preview-2025-06-15` compartilhado com 8 revisores; acuses guardados no tracker. |
| 2025-06-16 | Revisao de telemetria baseline | `docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals` verdes; logs do proxy Try it mostram tokens comunitarios ativos. |
| 2025-06-18 | Office hours e triage de issues | Duas sugestoes (`docs-preview/w2 #1` wording de tooltip, `#2` sidebar de localizacao) - ambas atribuidas a Docs. |
| 2025-06-21 | Checagem de telemetria + fixes de docs | Docs resolveu `docs-preview/w2 #1/#2`; dashboards verdes, sem incidentes. |
| 2025-06-24 | Office hours da ultima semana | Revisores confirmaram envios restantes; nenhum alerta. |
| 2025-06-29 | Resumo de saida + encerramento de convites | Acks registrados, acesso de preview revogado, snapshots + artefatos arquivados (ver [`preview-feedback/w2/summary.md`](./preview-feedback/w2/summary.md#exit-summary-2025-06-29)). |
| 2025-04-15 | Office hours e triage de issues | Duas sugestoes de documentacao registradas em `docs-preview/w1`; sem incidentes nem alertas. |

## Hooks de reporting

- Toda quarta-feira, atualize a tabela acima e a issue de convites ativa com uma nota curta (convites enviados, revisores ativos, incidentes).
- Quando uma onda encerrar, adicione o caminho do resumo de feedback (por exemplo, `docs/portal/docs/devportal/preview-feedback/w0/summary.md`) e linke a partir de `status.md`.
- Se qualquer criterio de pausa do [preview invite flow](./preview-invite-flow.md) for acionado, adicione os passos de remediacao aqui antes de retomar os convites.
