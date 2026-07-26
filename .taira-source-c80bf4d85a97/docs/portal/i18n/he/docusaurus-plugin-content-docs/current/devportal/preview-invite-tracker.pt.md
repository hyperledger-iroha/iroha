---
lang: he
direction: rtl
source: docs/portal/docs/devportal/preview-invite-tracker.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Tracker de convites עושה תצוגה מקדימה

Este tracker registra cada onda de preview do portal de docs para que owners de DOCS-SORA e revisores de governanca vejam qual coorte esta ativa, quem aprovou os convites e quais artefatos ainda precisam de atencao. Atualize-o semper que convites forem enviados, revogados ou adiados para que a trilha de auditoria permaneca no repositorio.

## Status das ondas

| אונדה | קורטה | גיליון דה acompanhamento | אפרודור(ים) | סטטוס | ג'נלה אלבו | Notas |
| --- | --- | --- | --- | --- | --- | --- |
| **W0 - מתחזקי ליבה** | מנהלי ה-Docs + SDK validando או fluxo de checksum | `DOCS-SORA-Preview-W0` (GitHub/ops tracker) | Lead Docs/DevRel + Portal TL | קונקלוידו | Q2 2025 סמנים 1-2 | Convites enviados 2025-03-25, telemetria ficou verde, resumo de saida publicado 2025-04-08. |
| **W1 - שותפים** | Operadores SoraFS, אינטגרדורים Torii מתייפח NDA | `DOCS-SORA-Preview-W1` | Lead Docs/DevRel + liaison de governanca | קונקלוידו | Q2 2025 Semana 3 | מזמין 2025-04-12 -> 2025-04-26 com os oito partners confirmados; evidencia capturada em [`preview-feedback/w1/log.md`](./preview-feedback/w1/log.md) e resumo de saida em [`preview-feedback/w1/summary.md`](./preview-feedback/w1/summary.md). |
| **W2 - קומונידה** | רשימת המתנה comunitaria curada (<=25 por vez) | `DOCS-SORA-Preview-W2` | Lead Docs/DevRel + מנהל קהילה | קונקלוידו | Q3 2025 Semana 1 (tentativo) | מזמין 2025-06-15 -> 2025-06-29 com telemetria verde o periodo todo; evidencia + achados em [`preview-feedback/w2/summary.md`](./preview-feedback/w2/summary.md). |
| **W3 - Coortes בטא** | Beta de financas/observabilidade + שותף SDK + advocate do ecossistema | `DOCS-SORA-Preview-W3` | Lead Docs/DevRel + liaison de governanca | קונקלוידו | Q1 2026 סמנה 8 | מזמין 2026-02-18 -> 2026-02-28; resumo + dados do portal gerados via onda `preview-20260218` (ver [`preview-feedback/w3/summary.md`](./preview-feedback/w3/summary.md)). |

> הערה: סוגיית vincule cada do tracker aos tickets de solicitacao de preview e arquive-os no projeto `docs-portal-preview` para que as aprovacoes continuem descobriveis.

## Tarefas ativas (W0)- Artefatos de preflight atualizados (execucao GitHub Actions `docs-portal-preview` 2025-03-24, descriptor verificado via `scripts/preview_verify.sh` com tag `preview-2025-03-24`).
- קווי בסיס של telemetria capturados (`docs.preview.integrity`, לוחות מחוונים של תמונת מצב `TryItProxyErrors` בבעיה W0).
- Texto de outreach travado usando [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md) תצוגה מקדימה של תג com `preview-2025-03-24`.
- Solicitacoes de entrada registradas para os primeiros cinco מתחזקות (כרטיסים `DOCS-SORA-Preview-REQ-01` ... `-05`).
- Cinco primeiros convites enviados 2025-03-25 10:00-10:20 UTC apos sete dias consecutivos de telemetria verde; acuses guardados em `DOCS-SORA-Preview-W0`.
- Monitoramento de telemetria + שעות המשרד מארחים (יומני צ'ק-אין אכלו 2025-03-31; log de checkpoints abaixo).
- משוב de meio de onda / issues coletadas e tagueadas `docs-preview/w0` (ver [W0 digest](./preview-feedback/w0/summary.md)).
- Resumo da onda publicado + confirmacoes de saida (bundle de saida datado 2025-04-08; ver [W0 digest](./preview-feedback/w0/summary.md)).
- Onda Beta W3 acompanhada; Futuras Ondas Agendas Conforme Revisao de Governanca.

## שותפי Resumo da onda W1

- Aprovacoes legais e de governanca. תוספת de partners assinado 2025-04-05; aprovacoes enviadas para `DOCS-SORA-Preview-W1`.
- Telemetria + נסה את זה בימוי. Ticket de mudanca `OPS-TRYIT-147` executado 2025-04-06 com צילומי מצב Grafana de `docs.preview.integrity`, `TryItProxyErrors`, e `DocsPortal/GatewayRefusals` arquidos.
- Preparacao de artefato + checksum. חבילה `preview-2025-04-12` מאומת; תיאור יומני/צ'ק-סיום/בדיקה מבצעים את `artifacts/docs_preview/W1/preview-2025-04-12/`.
- Roster de convites + envio. Oito solicitacoes de partners (`DOCS-SORA-Preview-REQ-P01...P08`) aprovadas; מזמין enviados 2025-04-12 15:00-15:21 UTC com אcuses registrados por revisor.
- משוב Instrumentacao de. יומני שעות המשרד + נקודות ביקורת של טלמטריה registrados; ver [`preview-feedback/w1/summary.md`](./preview-feedback/w1/summary.md) para o digest.
- סגל סגל / log de saida. [`preview-feedback/w1/log.md`](./preview-feedback/w1/log.md) חותמות זמן של registra de convite/ack, evidencia de telemetria, exports de quiz e ponteiros de artefatos em 2025-04-26 para que a governanca a possa reproduzir.

## Log de convites - מתחזקי הליבה של W0| ID de revisor | פאפל | Ticket de solicitacao | Convite enviado (UTC) | Saida esperada (UTC) | סטטוס | Notas |
| --- | --- | --- | --- | --- | --- | --- |
| docs-core-01 | מתחזק פורטל | `DOCS-SORA-Preview-REQ-01` | 25-03-2025 10:05 | 2025-04-08 10:00 | אטיו | Confirmou verificacao de checksum; foco em revisao de nav/sidebar. |
| sdk-rust-01 | עופרת SDK חלודה | `DOCS-SORA-Preview-REQ-02` | 25-03-2025 10:08 | 2025-04-08 10:00 | אטיו | Teststando receitas de SDK + התחלה מהירה של Norito. |
| sdk-js-01 | מתחזק JS SDK | `DOCS-SORA-Preview-REQ-03` | 25-03-2025 10:12 | 2025-04-08 10:00 | אטיו | קונסולת Validando נסה זאת + Fluxos ISO. |
| sorafs-ops-01 | SoraFS קשר מפעיל | `DOCS-SORA-Preview-REQ-04` | 25-03-2025 10:15 | 2025-04-08 10:00 | אטיו | Auditando runbooks SoraFS + docs de orquestracao. |
| observability-01 | צפיות TL | `DOCS-SORA-Preview-REQ-05` | 25-03-2025 10:18 | 2025-04-08 10:00 | אטיו | Revisando apendices de telemetria/incidentes; responsavel pela cobertura de Alertmanager. |

Todos os convites referenciam o mesmo artefato `docs-portal-preview` (execucao 2025-03-24, tag `preview-2025-03-24`) e o log de verificacao capturado em `DOCS-SORA-Preview-W0`. Qualquer adicao/pausa deve ser registrada tanto na tabela acima quanto na issue do tracker antes de Passer para a proxima onda.

## יומן מחסומים - W0

| נתונים (UTC) | אטיבידידה | Notas |
| --- | --- | --- |
| 26-03-2025 | Revisao de telemetria baseline + שעות משרד | `docs.preview.integrity` + `TryItProxyErrors` ficaram verdes; שעות המשרד confirmaram verificacao de checksum concluida. |
| 27-03-2025 | Digest de feedback intermediario publicado | Resumo capturado em [`preview-feedback/w0/summary.md`](./preview-feedback/w0/summary.md); dois issues de nav menores tagueados como `docs-preview/w0`, sem incidentes. |
| 31-03-2025 | Checagem de telemetria da ultima semana | שעות המשרד של Ultimas לפני יציאה; revisores confirmaram tarefas restantes em andamento, sem alertas. |
| 2025-04-08 | Resumo de saida + encerramento de convites | ביקורות קומפלטס אישור, acesso temporario revogado, achados arquivados em [`preview-feedback/w0/summary.md`](./preview-feedback/w0/summary.md#exit-summary-2025-04-08); tracker atualizado antes de preparar W1. |

## Log de convites - שותפים W1| ID de revisor | פאפל | Ticket de solicitacao | Convite enviado (UTC) | Saida esperada (UTC) | סטטוס | Notas |
| --- | --- | --- | --- | --- | --- | --- |
| soraps-op-01 | מפעיל SoraFS (EU) | `DOCS-SORA-Preview-REQ-P01` | 2025-04-12 15:00 | 26/04/2025 15:00 | קונקלוידו | Entregou feedback de ops do orquestrador 2025-04-20; ack de saida 15:05 UTC. |
| soraps-op-02 | מפעיל SoraFS (JP) | `DOCS-SORA-Preview-REQ-P02` | 12-04-2025 15:03 | 26/04/2025 15:00 | קונקלוידו | הערות לרישום השקה ב-`docs-preview/w1`; ack 15:10 UTC. |
| soraps-op-03 | מפעיל SoraFS (ארה"ב) | `DOCS-SORA-Preview-REQ-P03` | 2025-04-12 15:06 | 26/04/2025 15:00 | קונקלוידו | רישומי מחלוקת/רשימה שחורה; ack 15:12 UTC. |
| torii-int-01 | אינטגרטור Torii | `DOCS-SORA-Preview-REQ-P04` | 12-04-2025 15:09 | 26/04/2025 15:00 | קונקלוידו | Walkthrough de Try it auth aceito; ack 15:14 UTC. |
| torii-int-02 | אינטגרטור Torii | `DOCS-SORA-Preview-REQ-P05` | 12-04-2025 15:12 | 26/04/2025 15:00 | קונקלוידו | הערות לרישום RPC/OAuth; ack 15:16 UTC. |
| sdk-partner-01 | שותף SDK (Swift) | `DOCS-SORA-Preview-REQ-P06` | 12-04-2025 15:15 | 26/04/2025 15:00 | קונקלוידו | Feedback de integridade do preview mergeado; ack 15:18 UTC. |
| sdk-partner-02 | שותף SDK (אנדרואיד) | `DOCS-SORA-Preview-REQ-P07` | 2025-04-12 15:18 | 26/04/2025 15:00 | קונקלוידו | Revisao de telemetria/redaction feita; ack 15:22 UTC. |
| gateway-ops-01 | מפעיל שער | `DOCS-SORA-Preview-REQ-P08` | 2025-04-12 15:21 | 26/04/2025 15:00 | קונקלוידו | הערות לרישום רישום שער DNS; ack 15:24 UTC. |

## יומן מחסומים - W1

| נתונים (UTC) | אטיבידידה | Notas |
| --- | --- | --- |
| 2025-04-12 | Envio de convites + verificacao de artefatos | Oito partners receberam email com descriptor/archive `preview-2025-04-12`; מאשים registrados ללא מעקב. |
| 2025-04-13 | Revisao de telemetria baseline | `docs.preview.integrity`, `TryItProxyErrors`, ו-`DocsPortal/GatewayRefusals` verdes; שעות המשרד confirmaram verificacao de checksum concluida. |
| 2025-04-18 | שעות המשרד de meio de onda | `docs.preview.integrity` permaneceu verde; dois nits de docs tagueados `docs-preview/w1` (ניסוח ניווט + צילום מסך נסה זאת). |
| 22-04-2025 | Checagem final de telemetria | פרוקסי + לוחות מחוונים saudaveis; nenhuma issue nova, registrado no tracker antes da saida. |
| 26-04-2025 | Resumo de saida + encerramento de convites | Todos os partners confirmaram review, convites revogados, evidencia arquivada em [`preview-feedback/w1/summary.md`](./preview-feedback/w1/summary.md#exit-summary-2025-04-26). |

## Recap da coorte beta W3

- Convites enviados 2026-02-18 com verificacao de checksum + acuses registrados no mesmo dia.
- משוב coletado em `docs-preview/20260218` com issue de governanca `DOCS-SORA-Preview-20260218`; digest + resumo gerados דרך `npm run --prefix docs/portal preview:wave -- --wave preview-20260218`.
- Acesso revogado 2026-02-28 apos o check final de telemetria; גשש + טבלאות לעשות את הפורטל atualizadas para marcar W3 como concluido.

## Log de convites - קהילת W2| ID de revisor | פאפל | Ticket de solicitacao | Convite enviado (UTC) | Saida esperada (UTC) | סטטוס | Notas |
| --- | --- | --- | --- | --- | --- | --- |
| comm-vol-01 | מבקר קהילה (SDK) | `DOCS-SORA-Preview-REQ-C01` | 15-06-2025 16:00 | 29/06/2025 16:00 | קונקלוידו | אק 16:06 UTC; foco em quickstarts de SDK; saida confirmada 2025-06-29. |
| comm-vol-02 | מבקר קהילה (ממשל) | `REQ-C02` | 15/06/2025 16:03 | 29/06/2025 16:00 | קונקלוידו | Revisao de governanca/SNS feita; saida confirmada 2025-06-29. |
| comm-vol-03 | מבקר קהילה (Norito) | `REQ-C03` | 15/06/2025 16:06 | 29/06/2025 16:00 | קונקלוידו | משוב יש לעבור דרך Norito רישום; ack 2025-06-29. |
| comm-vol-04 | מבקר קהילה (SoraFS) | `REQ-C04` | 15/06/2025 16:09 | 29/06/2025 16:00 | קונקלוידו | Revisao de runbooks SoraFS feita; ack 2025-06-29. |
| comm-vol-05 | מבקר קהילה (נגישות) | `REQ-C05` | 15/06/2025 16:12 | 29/06/2025 16:00 | קונקלוידו | Notas de acessibilidade/UX compartilhadas; ack 2025-06-29. |
| comm-vol-06 | מבקר קהילה (לוקליזציה) | `REQ-C06` | 15/06/2025 16:15 | 29/06/2025 16:00 | קונקלוידו | משוב de localizacao registrado; ack 2025-06-29. |
| comm-vol-07 | מבקר קהילה (נייד) | `REQ-C07` | 15/06/2025 16:18 | 29/06/2025 16:00 | קונקלוידו | בודק את מסמכי ה-SDK הניידים; ack 2025-06-29. |
| comm-vol-08 | מבקר קהילה (צפיות) | `REQ-C08` | 15/06/2025 16:21 | 29/06/2025 16:00 | קונקלוידו | Revisao de apendice de observabilidade feita; ack 2025-06-29. |

## יומן מחסומים - W2

| נתונים (UTC) | אטיבידידה | Notas |
| --- | --- | --- |
| 2025-06-15 | Envio de convites + verificacao de artefatos | תיאור/ארכיון `preview-2025-06-15` compartilhado com 8 revisores; מאשים guardados ללא גשש. |
| 2025-06-16 | Revisao de telemetria baseline | `docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals` verdes; יומנים עושים פרוקסי נסה את זה אסימוני Mostram comunitarios ativos. |
| 2025-06-18 | שעות משרד e triage de issues | Duas sugestoes (`docs-preview/w2 #1` formule de tooltip, `#2` sidebar de localizacao) - ambas atribuidas a Docs. |
| 21-06-2025 | Checagem de telemetria + תיקוני מסמכים | Docs resolveu `docs-preview/w2 #1/#2`; לוחות מחוונים verdes, sem תקריות. |
| 24-06-2025 | שעות המשרד da Ultima Semana | מחזירה לאישורים; nenhum alerta. |
| 29-06-2025 | Resumo de saida + encerramento de convites | Acks registrados, acesso de preview revogado, תמונות מצב + artefatos arquivados (ver [`preview-feedback/w2/summary.md`](./preview-feedback/w2/summary.md#exit-summary-2025-06-29)). |
| 2025-04-15 | שעות משרד e triage de issues | Duas sugestoes de documentacao registradas em `docs-preview/w1`; sem incidentes nem alertas. |

## Hooks de reporting- Toda quarta-feira, atualize a tabela acima e a issue de convites ativa com uma nota curta (convites enviados, revisores ativos, incidentes).
- Quando uma onda encerrar, adicione o caminho do resumo de feedback (לדוגמה, `docs/portal/docs/devportal/preview-feedback/w0/summary.md`) e linke a partir de `status.md`.
- Se qualquer criterio de pausa do [תצוגה מקדימה של הזמנה](./preview-invite-flow.md) עבור acionado, adicione os passos de remediacao aqui antes de retomar os convites.