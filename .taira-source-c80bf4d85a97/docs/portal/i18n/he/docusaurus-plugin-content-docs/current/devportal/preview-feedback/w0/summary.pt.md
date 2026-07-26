---
lang: he
direction: rtl
source: docs/portal/docs/devportal/preview-feedback/w0/summary.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: preview-feedback-w0-summary
כותרת: Resumo de feedback do meio do W0
sidebar_label: משוב W0 (meio)
תיאור: מחסומים, achados e acoes de meio de onda para a onda de preview de mantenedores core.
---

| פריט | פרטים |
| --- | --- |
| אונדה | W0 - ליבת Mantenedores |
| נתונים לחידוש | 27-03-2025 |
| Janela de revisao | 2025-03-25 -> 2025-04-08 |
| משתתפים | docs-core-01, sdk-rust-01, sdk-js-01, sorafs-ops-01, observability-01 |
| Tag de artefato | `preview-2025-03-24` |

## Destaques

1. **Fluxo de checksum** - Todos os reviewers confirmaram que `scripts/preview_verify.sh`
   teve sucesso contra o par descriptor/comartilhado. Nenhum עוקף הודעה ידנית
   הכרחי.
2. **משוב de navegacao** - Dois problemas menores de ordenacao do sidebar foram
   registrados (`docs-preview/w0 #1-#2`). Ambos foram encaminhados para Docs/DevRel e nao
   בלוקייאם א-אונדה.
3. **Paridade de runbooks SoraFS** - sorafs-ops-01 pediu links cruzados mais claros entre
   `sorafs/orchestrator-ops` e `sorafs/multi-source-rollout`. Issue de acompanhamento aberta;
   tratar antes de W1.
4. **Revisao de telemetria** - observability-01 confirmou que `docs.preview.integrity`,
   `TryItProxyErrors` e os logs do proxy Try-it ficaram verdes; nenhum alerta disparou.

## Itens de acao

| תעודת זהות | תיאור | שו"ת | סטטוס |
| --- | --- | --- | --- |
| W0-A1 | Reordenar entradas do sidebar do devportal para destacar docs focados em reviewers (`preview-invite-*` agrupados). | Docs-core-01 | Concluido - או רשימה של סרגל צד או מסמכים של סוקרים לפורמה קונטיגוואה (`docs/portal/sidebars.js`). |
| W0-A2 | קישור נוסף cruzado explicito entre `sorafs/orchestrator-ops` e `sorafs/multi-source-rollout`. | Sorafs-ops-01 | Concluido - cada runbook agora aponta para o outro para que operadores veiam ambos os guias durante rollouts. |
| W0-A3 | שיתוף תמונות מצב של טלמטריה + חבילת שאילתות com o tracker de governanca. | צפיות-01 | Concluido - Bundle anexado ao `DOCS-SORA-Preview-W0`. |

## Resumo de encerramento (2025-04-08)

- סוקרי Todos os cinco confirmaram a conclusao, limparam בונה לוקאיס e sairam da janela
  de preview; כמו revogacoes de acesso ficaram registradas em `DOCS-SORA-Preview-W0`.
- Nenhum incidente ou alerta ocorreu durante a onda; לוחות המחוונים של מערכת ההפעלה של Telemetria Ficaram
  verdes durante todo o periodo.
- As acoes de navegacao + קישורים cruzados (W0-A1/A2) estao implementadas e refletidas nos docs
  acima; a evidencia de telemetria (W0-A3) esta anexada ao tracker.
- Bundle de evidencia arquivado: צילומי מסך של telemetria, confirmacoes de convite e este
  resumo estao linkados אין בעיה לעשות מעקב.

## Proximos passos

- Implementar os itens de acao do W0 antes de abrir W1.
- Obter aprovacao legal e um slot de staging para o proxy, depois seguir os passos de preflight
  da onda de parceiros descritos no [זרימת הזמנת תצוגה מקדימה](../../preview-invite-flow.md).

_Este resumo esta linkado a partir do [תצוגה מקדימה של הזמנה מעקב](../../preview-invite-tracker.md) para
manter o מפת דרכים DOCS-SORA rastreavel._