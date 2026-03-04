---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w1/summary.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: visualização-feedback-w1-resumo
título: Resumo de feedback e cierre W1
sidebar_label: Resumo W1
descrição: Hallazgos, ações e evidências de segurança para a visão de parceiros e integradores Torii.
---

| Artigo | Detalhes |
| --- | --- |
| Olá | W1 - Parceiros e integradores de Torii |
| Ventana de convite | 12/04/2025 -> 26/04/2025 |
| Etiqueta de artefato | `preview-2025-04-12` |
| Emissão do rastreador | `DOCS-SORA-Preview-W1` |
| Participantes | sorafs-op-01...03, torii-int-01...02, sdk-partner-01...02, gateway-ops-01 |

## Destacados

1. **Flujo de checksum** - Todos os revisores verificam o descritor/arquivo via `scripts/preview_verify.sh`; os registros são salvos junto com as acusações de convite.
2. **Telemetria** - Os painéis `docs.preview.integrity`, `TryItProxyErrors` e `DocsPortal/GatewayRefusals` são mantidos em verde durante toda a tarde; não há incidentes nem páginas de alerta.
3. **Feedback de docs (`docs-preview/w1`)** - Se registraron dos nits menores:
   - `docs-preview/w1 #1`: esclarecer o texto de navegação na seção Try it (resuelto).
   - `docs-preview/w1 #2`: atualizar captura de tela de Try it (resultado).
4. **Paridade de runbooks** - Operadores de SoraFS confirmam que os novos links cruzados entre `orchestrator-ops` e `multi-source-rollout` resolvem suas preocupações de W0.

## Ações

| ID | Descrição | Responsável | Estado |
| --- | --- | --- | --- |
| W1-A1 | Atualizar o texto de navegação de Try it após `docs-preview/w1 #1`. | Documentos-núcleo-02 | Concluído (18/04/2025). |
| W1-A2 | Atualizar captura de tela de Try it segundo `docs-preview/w1 #2`. | Documentos-núcleo-03 | Concluído (19/04/2025). |
| W1-A3 | Resumir hallazgos de parceiros e evidências de telemetria em roadmap/status. | Líder do Documentos/DevRel | Concluído (ver tracker + status.md). |

## Resumo de cierre (2025-04-26)

- Os revisores confirmarão a finalização durante o horário comercial final, limparão os locais dos artefatos e revogarão seu acesso.
- A telemetria se mantuvo em verde até o topo; snapshots finais anexados a `DOCS-SORA-Preview-W1`.
- O registro de convites é atualizado com acusações de saída; o rastreador marco W1 foi concluído e agregou os pontos de verificação.
- Pacote de evidências (descritor, log de soma de verificação, saída da sonda, transcrição do proxy Try it, capturas de tela de telemetria, resumo de feedback) arquivado abaixo de `artifacts/docs_preview/W1/`.

## Seguintes passos

- Preparar o plano de admissão comunitário W2 (aprovação de governo + ajustes de modelo de solicitação).
- Atualize a etiqueta do artefato de visualização para o W2 e reejecutar o script de comprovação quando o término for finalizado.
- Volcar hallazgos aplicáveis ​​de W1 no roteiro/status para que a ola comunitária tenha o guia mais recente.