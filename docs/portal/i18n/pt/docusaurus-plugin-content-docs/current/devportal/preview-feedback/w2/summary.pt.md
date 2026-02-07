---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w2/summary.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: visualização-feedback-w2-resumo
título: Resumo de feedback e status W2
sidebar_label: Resumo W2
descrição: Resumo ao vivo para a onda de visualização comunitária (W2).
---

| Artigo | Detalhes |
| --- | --- |
| Onda | W2 - Revisores comunitários |
| Janela de convite | 15/06/2025 -> 29/06/2025 |
| Etiqueta de arte | `preview-2025-06-15` |
| Problema do rastreador | `DOCS-SORA-Preview-W2` |
| Participantes | comm-vol-01...comm-vol-08 |

##Destaques

1. **Governanca e ferramentas** - Uma política de admissão comunitária foi aprovada por unanimidade em 20/05/2025; o modelo de solicitação atualizado com campos de motivação/fuso horário está em `docs/examples/docs_preview_request_template.md`.
2. **Evidencia de preflight** - A mudança do proxy Try it `OPS-TRYIT-188` rodou em 2025-06-09, dashboards do Grafana capturados, e as saídas do descritor/checksum/probe de `preview-2025-06-15` arquivados em `artifacts/docs_preview/W2/`.
3. **Onda de convites** - Oito revisores comunitários convidados em 15/06/2025, com agradecimentos registrados na tabela de convites do tracker; todos completaram a verificação do checksum antes de navegar.
4. **Feedback** - `docs-preview/w2 #1` (redação da dica de ferramenta) e `#2` (ordem da barra lateral de localização) foram registrados em 18/06/2025 e resolvidos em 21/06/2025 (Docs-core-04/05); nenhum incidente durante a onda.

## Itens de ação

| ID | Descrição | Responsável | Estado |
| --- | --- | --- | --- |
| W2-A1 | Tratar `docs-preview/w2 #1` (redação da dica de ferramenta). | Documentos-núcleo-04 | Concluído 2025-06-21 |
| W2-A2 | Tratar `docs-preview/w2 #2` (barra lateral de localização). | Documentos-núcleo-05 | Concluído 2025-06-21 |
| W2-A3 | Arquivar evidência de saida + atualizar roadmap/status. | Líder do Documentos/DevRel | Concluído 2025-06-29 |

## Resumo de encerramento (2025-06-29)

- Todos os oito revisores comunitários confirmaram a conclusão e tiveram o acesso de visualização revogado; agradecimentos registrados no log de convites do tracker.
- Os snapshots finais de telemetria (`docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals`) ficaram verdes; logs e transcrições do proxy Experimente anexados a `DOCS-SORA-Preview-W2`.
- Pacote de evidências (descritor, log de checksum, saída de sonda, relatório de link, capturas de tela do Grafana, agradecimentos de convite) arquivado em `artifacts/docs_preview/W2/preview-2025-06-15/`.
- O log de checkpoints W2 do tracker foi atualizado até o encerramento, garantindo um registro auditável antes do início do planejamento W3.