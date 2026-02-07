---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/preview-invite-tracker.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Antevisão do programa de rastreamento

Este трекер фиксирует каждую волну preview портала документации, чтобы владельцы DOCS-SORA e ревьюеры governança Você pode ver a atividade ativa, que fornece programas e quais artefatos causam vibração. Faça isso por meio de uma operação, verifique ou realize um serviço de auditoria que seja auditado no local repositório.

## Estado atual

| Volna | Corta | Edição трекера | Aprovador(es) | Status | Céu aberto | Nomeação |
| --- | --- | --- | --- | --- | --- | --- |
| **W0 - Mantenedores principais** | Mantenedores Docs + SDK, fluxo de checksum de validação | `DOCS-SORA-Preview-W0` (rastreador GitHub/ops) | Documentos principais/DevRel + Portal TL | Sobreviver | 2º trimestre de 2025 1-2 | Приглашения отправлены 2025-03-25, телеметрия была зеленой, итоговый resumo опубликован 2025-04-08. |
| **W1 - Parceiros** | Operador SoraFS, integrador Torii sob NDA | `DOCS-SORA-Preview-W1` | Liderança de documentos/DevRel + contato de governança | Sobreviver | 2º trimestre de 2025, 3º trimestre | Приглашения 2025-04-12 -> 2025-04-26, все восемь партнеров подтвердили; evidência em [`preview-feedback/w1/log.md`](./preview-feedback/w1/log.md) e resumo exibido em [`preview-feedback/w1/summary.md`](./preview-feedback/w1/summary.md). |
| **W2 - Comunidade** | Lista de espera da comunidade Кураторский (<=25 por semana) | `DOCS-SORA-Preview-W2` | Lead Docs/DevRel + Gerente de comunidade | Sobreviver | 3º trimestre de 2025, 1º trimestre (provisório) | Приглашения 2025-06-15 -> 2025-06-29, телеметрия зеленая весь период; evidências + descobertas em [`preview-feedback/w2/summary.md`](./preview-feedback/w2/summary.md). |
| **W3 – Coortes beta** | Beta de finanças/observabilidade + parceiro SDK + defensor do ecossistema | `DOCS-SORA-Preview-W3` | Liderança de documentos/DevRel + contato de governança | Sobreviver | 1º trimestre de 2026, 8 de agosto | Приглашения 2026-02-18 -> 2026-02-28; resumo + dados do portal estão no volume `preview-20260218` (см. [`preview-feedback/w3/summary.md`](./preview-feedback/w3/summary.md)). |

> Observação: связывайте каждую issue трекера соответствующими tickets de solicitação de visualização e архивируйте их их в проекте `docs-portal-preview`, чтобы aprovações оставались доступными.

## Atividades ativas (W0)

- Обновлены preflight артефакты (запуск GitHub Actions `docs-portal-preview` 2025-03-24, descritor проверен через `scripts/preview_verify.sh` с тегом `preview-2025-03-24`).
- Зафиксированы linha de base телеметрии (`docs.preview.integrity`, snapshot `TryItProxyErrors` сохранен в issue W0).
- O texto de divulgação está disponível [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md) com a tag de visualização `preview-2025-03-24`.
- Записаны entrada запросы для первых пяти mantenedores (ingressos `DOCS-SORA-Preview-REQ-01` ... `-05`).
- Первые пять приглашений отправлены 2025-03-25 10:00-10:20 UTC после семи дней зеленой телеметрии; agradecimentos fornecidos em `DOCS-SORA-Preview-W0`.
- Мониторинг телеметрии + horário de expediente para хоста (eжедневные check-ins até 2025-03-31; checkpoint log ниже).
- Resolver feedback/problemas de ponto médio e comentários `docs-preview/w0` (см. [W0 digest](./preview-feedback/w0/summary.md)).
- Resumo de Опубликован волны + подтверждения выхода (pacote de saída 2025-04-08; ver. [W0 digest](./preview-feedback/w0/summary.md)).
- W3 beta wave отслежена; будущие волны планируются após a revisão da governança.

## Резюме волны Parceiros W1

- Aprovações legais e de governança. Adendo aos parceiros подписан 2025-04-05; aprovações aprovadas em `DOCS-SORA-Preview-W1`.
- Telemetria + Experimente a preparação. Alterar ticket `OPS-TRYIT-147` foi lançado em 2025-04-06 com instantâneos Grafana `docs.preview.integrity`, `TryItProxyErrors`, e `DocsPortal/GatewayRefusals`.
- Preparação de artefato + soma de verificação. Pacote `preview-2025-04-12` testado; logs descritor/checksum/probe encontrados em `artifacts/docs_preview/W1/preview-2025-04-12/`.
- Lista de convites + envio. Suas solicitações de parceiro (`DOCS-SORA-Preview-REQ-P01...P08`) одобрены; приглашения отправлены 2025-04-12 15:00-15:21 UTC, agradecimentos зафиксированы по ревьюеру.
- Instrumentação de feedback. Ежедневные horário de expediente + postos de controle телеметрии; sim. [`preview-feedback/w1/summary.md`](./preview-feedback/w1/summary.md).
- Registro final de escalação/saída. [`preview-feedback/w1/log.md`](./preview-feedback/w1/log.md) теперь фиксирует carimbos de data/hora convite/ack, evidência de telemetria, exportações de quiz e ponteiros артефактов на 2025-04-26, чтобы governança могла воспроизвести волну.

## Registro de convites - mantenedores principais do W0| ID do revisor | Função | Solicitar ingresso | Convite enviado (UTC) | Saída prevista (UTC) | Estado | Notas |
| --- | --- | --- | --- | --- | --- | --- |
| docs-core-01 | Mantenedor do portal | `DOCS-SORA-Preview-REQ-01` | 25/03/2025 10:05 | 08/04/2025 10:00 | Ativo | Подтвердил soma de verificação de verificação; foco na revisão de navegação/barra lateral. |
| sdk-rust-01 | Líder do Rust SDK | `DOCS-SORA-Preview-REQ-02` | 25/03/2025 10:08 | 08/04/2025 10:00 | Ativo | Teste receitas do SDK + guias de início rápido Norito. |
| sdk-js-01 | Mantenedor JS SDK | `DOCS-SORA-Preview-REQ-03` | 25/03/2025 10:12 | 08/04/2025 10:00 | Ativo | Валидирует Experimente console + fluxos ISO. |
| sorafs-ops-01 | Contato com o operador SoraFS | `DOCS-SORA-Preview-REQ-04` | 25/03/2025 10:15 | 08/04/2025 10:00 | Ativo | Аудитит SoraFS runbooks + documentos de orquestração. |
| observabilidade-01 | Observabilidade TL | `DOCS-SORA-Preview-REQ-05` | 25/03/2025 10:18 | 08/04/2025 10:00 | Ativo | Apêndices de telemetria/incidentes; отвечает para a cobertura do Alertmanager. |

Você está procurando o artefato `docs-portal-preview` (executado em 24/03/2025, tag `preview-2025-03-24`) e a transcrição provada em `DOCS-SORA-Preview-W0`. Deixe o download/pacote de uma nova física e em uma tabela, e em um problema, verifique o período de execução no final bem.

## Registro do ponto de verificação - W0

| Data (UTC) | Atividade | Notas |
| --- | --- | --- |
| 26/03/2025 | Revisão de telemetria de linha de base + horário comercial | `docs.preview.integrity` + `TryItProxyErrors` оставались зелеными; horário de expediente подтвердили завершение verificação de soma de verificação. |
| 27/03/2025 | Resumo de feedback de ponto médio postado | Resumo enviado em [`preview-feedback/w0/summary.md`](./preview-feedback/w0/summary.md); Há pequenos problemas de navegação encontrados em `docs-preview/w0`, sem problemas. |
| 31/03/2025 | Verificação pontual de telemetria da semana final | Последние horário de expediente перед выходом; ревьюеры подтвердили оставшиеся задачи, алертов не было. |
| 08/04/2025 | Resumo de saída + fechamento de convites | Подтверждены завершенные обзоры, временный доступ отозван, resultados архивированы в [`preview-feedback/w0/summary.md`](./preview-feedback/w0/summary.md#exit-summary-2025-04-08); rastreador обновлен перед W1. |

## Registro de convites - parceiros W1

| ID do revisor | Função | Solicitar ingresso | Convite enviado (UTC) | Saída prevista (UTC) | Estado | Notas |
| --- | --- | --- | --- | --- | --- | --- |
| sorafs-op-01 | Operador SoraFS (UE) | `DOCS-SORA-Preview-REQ-P01` | 12/04/2025 15:00 | 26/04/2025 15:00 | Concluído | Feedback de operações do orquestrador entregue em 20/04/2025; saída confirmada às 15:05 UTC. |
| sorafs-op-02 | Operador SoraFS (JP) | `DOCS-SORA-Preview-REQ-P02` | 12/04/2025 15:03 | 26/04/2025 15:00 | Concluído | Comentários de implementação registrados em `docs-preview/w1`; saída confirmada às 15:10 UTC. |
| sorafs-op-03 | Operador SoraFS (EUA) | `DOCS-SORA-Preview-REQ-P03` | 12/04/2025 15:06 | 26/04/2025 15:00 | Concluído | Edições de disputa/lista negra arquivadas; saída confirmada às 15:12 UTC. |
| torii-int-01 | Integrador Torii | `DOCS-SORA-Preview-REQ-P04` | 12/04/2025 15:09 | 26/04/2025 15:00 | Concluído | Experimente o passo a passo de autenticação aceito; saída confirmada às 15:14 UTC. |
| torii-int-02 | Integrador Torii | `DOCS-SORA-Preview-REQ-P05` | 12/04/2025 15:12 | 26/04/2025 15:00 | Concluído | Comentários do documento RPC/OAuth registrados; saída confirmada às 15:16 UTC. |
| sdk-parceiro-01 | Parceiro SDK (Swift) | `DOCS-SORA-Preview-REQ-P06` | 12/04/2025 15:15 | 26/04/2025 15:00 | Concluído | Visualizar feedback de integridade mesclado; saída confirmada às 15:18 UTC. |
| sdk-parceiro-02 | Parceiro SDK (Android) | `DOCS-SORA-Preview-REQ-P07` | 12/04/2025 15:18 | 26/04/2025 15:00 | Concluído | Revisão de telemetria/edição feita; saída confirmada às 15:22 UTC. |
| gateway-ops-01 | Operador de gateway | `DOCS-SORA-Preview-REQ-P08` | 12/04/2025 15:21 | 26/04/2025 15:00 | Concluído | Comentários do runbook DNS do gateway arquivados; saída confirmada às 15:24 UTC. |

## Registro do ponto de verificação - W1

| Data (UTC) | Atividade | Notas |
| --- | --- | --- |
| 12/04/2025 | Envio de convite + verificação de artefato | Todos os oito parceiros enviaram por e-mail o descritor/arquivo `preview-2025-04-12`; confirmações armazenadas no rastreador. |
| 13/04/2025 | Revisão da linha de base da telemetria | Painéis `docs.preview.integrity`, `TryItProxyErrors` e `DocsPortal/GatewayRefusals` verdes; horário comercial confirmado verificação de soma de verificação concluída. |
| 18/04/2025 | Horário comercial da onda média | `docs.preview.integrity` permaneceu verde; dois documentos registrados em `docs-preview/w1` (texto de navegação + captura de tela de teste). |
| 22/04/2025 | Verificação final de telemetria | Proxy + dashboards saudáveis; nenhuma nova questão foi levantada, anotada no rastreador antes da saída. |
| 2025/04/26 | Resumo de saída + fechamento de convites | Todos os parceiros confirmaram a conclusão da revisão, convites revogados, evidências arquivadas em [`preview-feedback/w1/summary.md`](./preview-feedback/w1/summary.md#exit-summary-2025-04-26). |

## Recapitulação da coorte W3 beta- Convites enviados em 18/02/2026 com verificação de soma de verificação + confirmações registradas no mesmo dia.
- Feedback coletado sob `docs-preview/20260218` com questão de governança `DOCS-SORA-Preview-20260218`; resumo + resumo gerado via `npm run --prefix docs/portal preview:wave -- --wave preview-20260218`.
- Acesso revogado em 28/02/2026 após a verificação final de telemetria; tabelas do rastreador + portal atualizadas para mostrar o W3 como concluído.

## Registro de convites - comunidade W2

| ID do revisor | Função | Solicitar ingresso | Convite enviado (UTC) | Saída prevista (UTC) | Estado | Notas |
| --- | --- | --- | --- | --- | --- | --- |
| comm-vol-01 | Revisor da comunidade (SDK) | `DOCS-SORA-Preview-REQ-C01` | 15/06/2025 16:00 | 29/06/2025 16:00 | Concluído | Confirmação 16:06 UTC; focando nos inícios rápidos do SDK; saída confirmada em 29/06/2025. |
| comm-vol-02 | Revisor comunitário (Governação) | `REQ-C02` | 15/06/2025 16:03 | 29/06/2025 16:00 | Concluído | Revisão da governação/SNS realizada; saída confirmada em 29/06/2025. |
| comm-vol-03 | Revisor da comunidade (Norito) | `REQ-C03` | 15/06/2025 16:06 | 29/06/2025 16:00 | Concluído | Feedback do passo a passo Norito registrado; saída confirmada 2025-06-29. |
| comm-vol-04 | Revisor da comunidade (SoraFS) | `REQ-C04` | 15/06/2025 16:09 | 29/06/2025 16:00 | Concluído | Revisão do runbook SoraFS concluída; saída confirmada 2025-06-29. |
| comm-vol-05 | Revisor da comunidade (Acessibilidade) | `REQ-C05` | 15/06/2025 16:12 | 29/06/2025 16:00 | Concluído | Notas de acessibilidade/UX compartilhadas; saída confirmada 2025-06-29. |
| comm-vol-06 | Revisor da comunidade (Localização) | `REQ-C06` | 15/06/2025 16:15 | 29/06/2025 16:00 | Concluído | Feedback de localização registrado; saída confirmada 2025-06-29. |
| comm-vol-07 | Revisor da comunidade (móvel) | `REQ-C07` | 15/06/2025 16:18 | 29/06/2025 16:00 | Concluído | Verificações de documentos do Mobile SDK entregues; saída confirmada 2025-06-29. |
| comm-vol-08 | Revisor comunitário (Observabilidade) | `REQ-C08` | 15/06/2025 16:21 | 29/06/2025 16:00 | Concluído | Revisão do apêndice de observabilidade concluída; saída confirmada 2025-06-29. |

## Registro do ponto de verificação - W2

| Data (UTC) | Atividade | Notas |
| --- | --- | --- |
| 15/06/2025 | Envio de convite + verificação de artefato | Descritor/arquivo `preview-2025-06-15` compartilhado com 8 revisores da comunidade; confirmações armazenadas no rastreador. |
| 16/06/2025 | Revisão da linha de base da telemetria | Painéis `docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals` verdes; Experimente os logs de proxy mostrarem tokens da comunidade ativos. |
| 18/06/2025 | Horário comercial e triagem de problemas | Duas sugestões coletadas (texto da dica de ferramenta `docs-preview/w2 #1`, barra lateral de localização `#2`) - ambas roteadas para o Documentos. |
| 21/06/2025 | Verificação de telemetria + correções de documentos | Documentos endereçados a `docs-preview/w2 #1/#2`; painéis ainda verdes, sem incidentes. |
| 24/06/2025 | Horário de expediente da última semana | Os revisores confirmaram os envios de feedback restantes; nenhum fogo de alerta. |
| 29/06/2025 | Resumo de saída + fechamento de convites | Confirmações registradas, acesso de visualização revogado, instantâneos de telemetria + artefatos arquivados (consulte [`preview-feedback/w2/summary.md`](./preview-feedback/w2/summary.md#exit-summary-2025-06-29)). |
| 15/04/2025 | Horário comercial e triagem de problemas | Duas sugestões de documentação registradas em `docs-preview/w1`; nenhum incidente ou alerta foi acionado. |

## Ganchos de relatórios

- Todas as quartas-feiras, atualize a tabela do rastreador acima e o problema do convite ativo com uma breve nota de status (convites enviados, revisores ativos, incidentes).
- Quando uma onda for fechada, anexe o caminho de resumo de feedback (por exemplo, `docs/portal/docs/devportal/preview-feedback/w0/summary.md`) e vincule-o a `status.md`.
- Se algum critério de pausa do [fluxo de convite de visualização](./preview-invite-flow.md) for acionado, adicione as etapas de correção aqui antes de retomar os convites.