---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w1/summary.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: visualização-feedback-w1-resumo
título: Resumo de feedback e encerramento W1
sidebar_label: Resumo W1
descrição: Achados, ações e evidências de encerramento para a onda de visualização de parceiros/integradores Torii.
---

| Artigo | Detalhes |
| --- | --- |
| Onda | W1 - Parceiros e integradores Torii |
| Janela de convite | 12/04/2025 -> 26/04/2025 |
| Etiqueta de arte | `preview-2025-04-12` |
| Problema do rastreador | `DOCS-SORA-Preview-W1` |
| Participantes | sorafs-op-01...03, torii-int-01...02, sdk-partner-01...02, gateway-ops-01 |

##Destaques

1. **Fluxo de checksum** - Todos os revisores validaram o descritor/arquivo via `scripts/preview_verify.sh`; logs armazenados junto aos agradecimentos de convite.
2. **Telemetria** - Dashboards `docs.preview.integrity`, `TryItProxyErrors` e `DocsPortal/GatewayRefusals` ficam verdes por toda a onda; nenhum incidente ou página de alerta.
3. **Documentos de feedback (`docs-preview/w1`)** - Duas lêndeas menores registradas:
   - `docs-preview/w1 #1`: redação clara de navegação na seção Experimente (resolvido).
   - `docs-preview/w1 #2`: atualizar captura de tela de Try it (resolvido).
4. **Paridade de runbooks** - Operadores de SoraFS confirmaram que os novos cross-links entre `orchestrator-ops` e `multi-source-rollout` resolveram as preocupações de W0.

## Itens de ação

| ID | Descrição | Responsável | Estado |
| --- | --- | --- | --- |
| W1-A1 | Atualizar o texto de navegação do Try it conforme `docs-preview/w1 #1`. | Documentos-núcleo-02 | Concluído (2025-04-18). |
| W1-A2 | Atualizar captura de tela de Try it conforme `docs-preview/w1 #2`. | Documentos-núcleo-03 | Concluído (2025-04-19). |
| W1-A3 | Resumir achados de parceiros e evidências de telemetria em roadmap/status. | Líder do Documentos/DevRel | Concluído (ver tracker + status.md). |

## Resumo de encerramento (2025-04-26)

- Todos os oito revisores confirmaram a conclusão durante o horário comercial final, limparam os artistas locais e tiveram o acesso revogado.
- A telemetria ficou verde até a saida; snapshots finais anexados a `DOCS-SORA-Preview-W1`.
- O log de convites foi atualizado com agradecimentos de saida; o tracker marcou W1 como concluído e adicionado os checkpoints.
- Pacote de evidências (descritor, log de checksum, saída da sonda, transcrição do proxy Try it, capturas de tela de telemetria, resumo de feedback) arquivado em `artifacts/docs_preview/W1/`.

## Próximos passos

- Preparar o plano de admissão comunitário W2 (aprovação de governança + ajustes no modelo de solicitação).
- Atualizar a tag de artistas de preview para a onda W2 e reexecutar o script de preflight quando os dados estiverem finalizados.
- Levar achados aplicaveis de W1 para roadmap/status para que a onda comunitária tenha uma orientação mais recente.