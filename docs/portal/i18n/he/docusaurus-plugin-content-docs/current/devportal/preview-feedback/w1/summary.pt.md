---
lang: he
direction: rtl
source: docs/portal/docs/devportal/preview-feedback/w1/summary.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: preview-feedback-w1-summary
כותרת: Resumo de feedback e encerramento W1
sidebar_label: Resumo W1
תיאור: Achados, acoes e evidencia de encerramento para a onda de preview de parceiros/integradores Torii.
---

| פריט | פרטים |
| --- | --- |
| אונדה | W1 - Parceiros e integradores Torii |
| Janela de convite | 2025-04-12 -> 2025-04-26 |
| Tag de artefato | `preview-2025-04-12` |
| בעיה לעשות גשש | `DOCS-SORA-Preview-W1` |
| משתתפים | sorafs-op-01...03, torii-int-01...02, sdk-partner-01...02, gateway-ops-01 |

## Destaques

1. **Fluxo de checksum** - Todos os reviewers validaram descriptor/archive via `scripts/preview_verify.sh`; logs armazenados junto aos acknowledgements de convite.
2. **Telemetria** - לוחות מחוונים `docs.preview.integrity`, `TryItProxyErrors` e `DocsPortal/GatewayRefusals` ficaram verdes por toda a onda; nenhum incidente ou page de alerta.
3. **מסמכי משוב (`docs-preview/w1`)** - Dois nits menores registrados:
   - `docs-preview/w1 #1`: ניסוח esclarecer de navegacao na secao נסה את זה (resolvido).
   - `docs-preview/w1 #2`: צילום מסך של נסה את זה (resolvido).
4. **Paridade de runbooks** - Operadores de SoraFS confirmaram que os novos crosslinks entre `orchestrator-ops` e `multi-source-rollout` resolveram as preocupacoes de W0.

## Itens de acao

| תעודת זהות | תיאור | שו"ת | סטטוס |
| --- | --- | --- | --- |
| W1-A1 | Atualizar ניסוח de navegacao do נסה את זה תואם `docs-preview/w1 #1`. | Docs-core-02 | קונקלוידו (2025-04-18). |
| W1-A2 | Atualizar צילום מסך של Try it conforme `docs-preview/w1 #2`. | Docs-core-03 | קונקלוידו (2025-04-19). |
| W1-A3 | רזומה achados de parceiros e evidencia de telemetria em מפת דרכים/סטטוס. | Docs/DevRel lead | Concluido (ver tracker + status.md). |

## Resumo de encerramento (2025-04-26)

- סוקרי הצהרת הוראה מאשרים את הסיכום לשעות הפעילות של המשרד, הלימפארם ארטפטוס לוקאיס e tiveram o acesso revogado.
- טלמטריה ficou verde אכלה סאדה; צילומי מצב פינאיס אנקסאדוס a `DOCS-SORA-Preview-W1`.
- O log de convites foi atualizado com acnowledgements de saida; o tracker marcou W1 como concluido e adicionou os מחסומים.
- Bundle de evidencia (מתאר, יומן בדיקה, פלט בדיקה, תמלול לעשות פרוקסי נסה את זה, צילומי מסך של טלמטריה, תקציר משוב) arquivado em `artifacts/docs_preview/W1/`.

## Proximos passos

- הכנת קומיוניטי כניסת W2 (aprovacao de governanca + adjustes no template de solicitacao).
- אטואליסר או תגית אמנות תצוגה מקדימה ל-Onda W2 וביצוע מחדש או סקריפט של טיסה מוקדמת כמו נתונים סופית.
- Levar achados aplicaveis de W1 para map/status para que a onda comunitaria tenha a orientacao mais recente.