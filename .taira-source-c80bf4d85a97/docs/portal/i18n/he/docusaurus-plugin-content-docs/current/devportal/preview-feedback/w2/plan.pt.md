---
lang: he
direction: rtl
source: docs/portal/docs/devportal/preview-feedback/w2/plan.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: preview-feedback-w2-plan
כותרת: Plano de intake comunitario W2
sidebar_label: Plano W2
תיאור: Intake, aprovacoes e checklist de evidencia para a coorte de preview comunitaria.
---

| פריט | פרטים |
| --- | --- |
| אונדה | W2 - מבקרים comunitarios |
| ג'נלה אלבו | Q3 2025 Semana 1 (tentativa) |
| Tag de artefato (planejado) | `preview-2025-06-15` |
| בעיה לעשות גשש | `DOCS-SORA-Preview-W2` |

## אובייקטיביות

1. הגדר קריטריונים של שילוב זרימת עבודה ובדיקה.
2. Obter aprovacao de governanca para o roster proposto e o addendum de uso aceitavel.
3. Atualizar o artefato de preview verificado por checksum e o bundle de telemetria para a nova janela.
4. הכנת פרוקסי נסה את לוחות המחוונים של מערכת ההפעלה לפני נסיעות.

## Desdobramento de tarefas

| תעודה מזהה | טארפה | שו"ת | פראזו | סטטוס | Notas |
| --- | --- | --- | --- | --- | --- |
| W2-P1 | רדיר את קריטריונים לכניסה (כשירות, משבצות מקסימליות, דרישות של CoC) וחוזר עבור הממשלה | Docs/DevRel lead | 2025-05-15 | קונקלוידו | A politica de intake foi mergeada em `DOCS-SORA-Preview-W2` e endossada na reuniao do conselho 2025-05-20. |
| W2-P2 | Atualizar template de solicitacao com perguntas comunitarias (motivacao, disponibilidade, necessidades de localizacao) | Docs-core-01 | 2025-05-18 | קונקלוידו | `docs/examples/docs_preview_request_template.md` אגורה כוללת קהילה שנייה, רפרנציה ללא נוסחאות צריכת. |
| W2-P3 | Garantir aprovacao de governanca para o plano de intake (voto em reuniao + atas registradas) | קשר ממשל | 22-05-2025 | קונקלוידו | Voto aprovado por unanimidade em 2025-05-20; atas e roll call linkados em `DOCS-SORA-Preview-W2`. |
| W2-P4 | תכנת בימוי לעשות פרוקסי נסה את זה + Captura de Telemetria Para a Janela W2 (`preview-2025-06-15`) | Docs/DevRel + Ops | 2025-06-05 | קונקלוידו | כרטיס לשינוי `OPS-TRYIT-188` aprovado e executado em 2025-06-09 02:00-04:00 UTC; צילומי מסך Grafana arquivados com o ticket. |
| W2-P5 | Construir/verificar novo tag de artefato de preview (`preview-2025-06-15`) e arquivar descriptor/checksum/probe logs | פורטל TL | 2025-06-07 | קונקלוידו | `scripts/preview_wave_preflight.sh --tag preview-2025-06-15 ...` rodou em 2025-06-10; פלט armazenados em `artifacts/docs_preview/W2/preview-2025-06-15/`. |
| W2-P6 | מונטאר רוסטר de convites comunitarios (<=25 סוקרים, lotes escalonados) com contatos aprovados por governanca | מנהל קהילה | 2025-06-10 | קונקלוידו | ראשי קבוצה של 8 סוקרים comunitarios aprovado; IDs de requisicao `DOCS-SORA-Preview-REQ-C01...C08` נרשמים ללא עוקב. |

## רשימת הוכחות- [x] Registro de aprovacao de governanca (notas de reuniao + link de voto) anexado a `DOCS-SORA-Preview-W2`.
- [x] Template de solicitacao atualizado commited sob `docs/examples/`.
- [x] מתאר `preview-2025-06-15`, יומן בדיקה, פלט בדיקה, דוח קישור ותמלול לעשות פרוקסי נסה את זה armazenados em `artifacts/docs_preview/W2/`.
- [x] צילומי מסך Grafana (`docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals`) צילומי מסך עבור Janela Preflight W2.
- [x] Tabela de roster de convites com IDs de reviewers, tickets de solicitacao e timestamps de aprovacao preenchidos antes do envio (ver secao W2 no tracker).

Mantenha este plano atualizado; o tracker o referencia para que o מפת הדרכים DOCS-SORA וזו אקסטאמנטה או que falta antes de enviar convites W2.