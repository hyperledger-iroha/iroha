---
lang: pt
direction: ltr
source: docs/examples/docs_preview_feedback_digest.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4b1191d3475126df594d0f9f81d91f8ebbcd002c74a1f4d4176f2f42a59ca885
source_last_modified: "2025-11-19T07:56:11.635822+00:00"
translation_last_reviewed: 2026-01-01
---

# Digest de feedback de preview do portal de docs (Modelo)

Use este modelo ao resumir uma onda de preview para governance, revisoes de release ou
`status.md`. Copie o Markdown para o ticket de tracking, substitua os placeholders por
dados reais e anexe o resumo JSON exportado via
`npm run --prefix docs/portal preview:log -- --summary --summary-json`. O helper
`preview:digest` (`npm run --prefix docs/portal preview:digest -- --wave <label>`) gera
a secao de metricas abaixo para que voce so precise preencher as linhas de
highlights/actions/artefacts.

```markdown
## Digest de feedback da onda preview-<tag> (YYYY-MM-DD)
- Janela de convite: <start -> end>
- Revisores convidados: <count> (abertos: <count>)
- Envios de feedback: <count>
- Issues abertos: <count>
- Ultimo timestamp de evento: <ISO8601 from summary.json>

| Categoria | Detalhes | Owner / Follow-up |
| --- | --- | --- |
| Highlights | <ex., "ISO builder walkthrough landed well"> | <owner + data limite> |
| Achados bloqueantes | <lista de issue IDs ou links do tracker> | <owner> |
| Itens menores de polimento | <agrupar edicoes cosmeticas ou copy> | <owner> |
| Anomalias de telemetria | <link para snapshot de dashboard / log de probe> | <owner> |

## Acoes
1. <Acao + link + ETA>
2. <Segunda acao opcional>

## Artefatos
- Feedback log: `artifacts/docs_portal_preview/feedback_log.json` (`sha256:<digest>`)
- Resumo da onda: `artifacts/docs_portal_preview/preview-<tag>-summary.json`
- Snapshot de dashboard: `<link ou caminho>`

```

Mantenha cada digest junto ao ticket de tracking de convites para que revisores e
governance possam reconstruir o rastro de evidencia sem vasculhar logs de CI.
