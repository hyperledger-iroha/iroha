---
lang: he
direction: rtl
source: docs/portal/docs/devportal/preview-feedback/w2/summary.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: preview-feedback-w2-summary
כותרת: Resumo de feedback e status W2
sidebar_label: Resumo W2
תיאור: Resumo ao vivo para a onda de preview comunitaria (W2).
---

| פריט | פרטים |
| --- | --- |
| אונדה | W2 - מבקרים comunitarios |
| Janela de convite | 2025-06-15 -> 2025-06-29 |
| Tag de artefato | `preview-2025-06-15` |
| בעיה לעשות גשש | `DOCS-SORA-Preview-W2` |
| משתתפים | comm-vol-01...comm-vol-08 |

## Destaques

1. **Governanca e tooling** - A politica de intake comunitario foi aprovada por unanimidade em 2025-05-20; o template de solicitacao atualizado com campos de motivacao/fuso horario esta em `docs/examples/docs_preview_request_template.md`.
2. **Evidencia de preflight** - A mudanca do proxy נסה את זה `OPS-TRYIT-188` rodou em 2025-06-09, לוחות מחוונים עושים Grafana capturados, e os outputs descriptor/checksum/probe de I1800NI080X `artifacts/docs_preview/W2/`.
3. **Onda de convites** - Oito reviewers comunitarios convidados em 2025-06-15, com acknowledgements registrados na tablea de convites do tracker; todos completaram a verificacao de checksum antes de navegar.
4. **משוב** - `docs-preview/w2 #1` (ניסוח de tooltip) e `#2` (מסדר do sidebar de localizacao) foram registrados em 2025-06-18 e resolvidos ate 2025-06-5-2100; nenhum incidente durante a onda.

## Itens de acao

| תעודת זהות | תיאור | שו"ת | סטטוס |
| --- | --- | --- | --- |
| W2-A1 | Tratar `docs-preview/w2 #1` (ניסוח de tooltip). | Docs-core-04 | Concluido 2025-06-21 |
| W2-A2 | Tratar `docs-preview/w2 #2` (סרגל צד דה לוקליזאקאו). | Docs-core-05 | Concluido 2025-06-21 |
| W2-A3 | ארכיון הוכחות דה אמרה + מפת דרכים/סטטוס אטואליסר. | Docs/DevRel lead | Concluido 2025-06-29 |

## Resumo de encerramento (2025-06-29)

- Todos os oito מבקרים comunitarios confirmaram a conclusao e tiveram o acesso de preview revogado; אישורים registrados אין לוג de convites do tracker.
- תמונות מצב של OS finais de telemetria (`docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals`) ficaram verdes; יומנים ותמלילים לעשות פרוקסי נסה זאת ב-`DOCS-SORA-Preview-W2`.
- Bundle de evidencia (תיאור, יומן בדיקה, פלט בדיקה, דוח קישור, צילומי מסך ל-Grafana, אישורי אישור) arquivado em `artifacts/docs_preview/W2/preview-2025-06-15/`.
- O log de checkpoints W2 do tracker foi atualizado ate o encerramento, garantindo um registro auditavel antes do inicio do planejamento W3.