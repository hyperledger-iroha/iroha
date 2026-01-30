---
lang: ja
direction: ltr
source: docs/portal/i18n/pt/docusaurus-plugin-content-docs/current/devportal/preview-feedback/w2/summary.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9d2bab28ffdf5c2549c951acd3cd696b279a145bfc0d4955b4aa4db6e54a7b5f
source_last_modified: "2025-11-14T04:43:19.965603+00:00"
translation_last_reviewed: 2026-01-30
---

| Item | Detalhes |
| --- | --- |
| Onda | W2 - Reviewers comunitarios |
| Janela de convite | 2025-06-15 -> 2025-06-29 |
| Tag de artefato | `preview-2025-06-15` |
| Issue do tracker | `DOCS-SORA-Preview-W2` |
| Participantes | comm-vol-01...comm-vol-08 |

## Destaques

1. **Governanca e tooling** - A politica de intake comunitario foi aprovada por unanimidade em 2025-05-20; o template de solicitacao atualizado com campos de motivacao/fuso horario esta em `docs/examples/docs_preview_request_template.md`.
2. **Evidencia de preflight** - A mudanca do proxy Try it `OPS-TRYIT-188` rodou em 2025-06-09, dashboards do Grafana capturados, e os outputs de descriptor/checksum/probe de `preview-2025-06-15` arquivados em `artifacts/docs_preview/W2/`.
3. **Onda de convites** - Oito reviewers comunitarios convidados em 2025-06-15, com acknowledgements registrados na tabela de convites do tracker; todos completaram a verificacao de checksum antes de navegar.
4. **Feedback** - `docs-preview/w2 #1` (wording de tooltip) e `#2` (ordem do sidebar de localizacao) foram registrados em 2025-06-18 e resolvidos ate 2025-06-21 (Docs-core-04/05); nenhum incidente durante a onda.

## Itens de acao

| ID | Descricao | Responsavel | Status |
| --- | --- | --- | --- |
| W2-A1 | Tratar `docs-preview/w2 #1` (wording de tooltip). | Docs-core-04 | Concluido 2025-06-21 |
| W2-A2 | Tratar `docs-preview/w2 #2` (sidebar de localizacao). | Docs-core-05 | Concluido 2025-06-21 |
| W2-A3 | Arquivar evidencia de saida + atualizar roadmap/status. | Docs/DevRel lead | Concluido 2025-06-29 |

## Resumo de encerramento (2025-06-29)

- Todos os oito reviewers comunitarios confirmaram a conclusao e tiveram o acesso de preview revogado; acknowledgements registrados no log de convites do tracker.
- Os snapshots finais de telemetria (`docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals`) ficaram verdes; logs e transcripts do proxy Try it anexados a `DOCS-SORA-Preview-W2`.
- Bundle de evidencia (descriptor, checksum log, probe output, link report, screenshots do Grafana, acknowledgements de convite) arquivado em `artifacts/docs_preview/W2/preview-2025-06-15/`.
- O log de checkpoints W2 do tracker foi atualizado ate o encerramento, garantindo um registro auditavel antes do inicio do planejamento W3.
