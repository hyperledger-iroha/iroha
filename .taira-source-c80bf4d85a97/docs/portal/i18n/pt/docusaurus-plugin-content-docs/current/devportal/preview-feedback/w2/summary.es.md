---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w2/summary.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: visualização-feedback-w2-resumo
título: Resumo de feedback e estado W2
sidebar_label: Resumo W2
descrição: Resumen en vivo para la ola de preview comunitaria (W2).
---

| Artigo | Detalhes |
| --- | --- |
| Olá | W2 - Revisores comunitários |
| Ventana de convite | 15/06/2025 -> 29/06/2025 |
| Etiqueta de artefato | `preview-2025-06-15` |
| Emissão do rastreador | `DOCS-SORA-Preview-W2` |
| Participantes | comm-vol-01 ... comm-vol-08 |

## Destacados

1. **Governança e ferramentas** - A política de admissão comunitária foi aprovada por unanimidade em 20/05/2025; o modelo de solicitação atualizado com campos de motivação/zona horária viva em `docs/examples/docs_preview_request_template.md`.
2. **Evidência de comprovação** - A mudança do proxy Try it `OPS-TRYIT-188` foi executada em 2025-06-09, os painéis de Grafana capturados e as saídas do descritor/checksum/probe de `preview-2025-06-15` arquivadas abaixo `artifacts/docs_preview/W2/`.
3. **Ola de convites** - Outros revisores comunitários convidados em 15/06/2025, com agradecimentos registrados na tabela de convites do tracker; todos completam a verificação da soma de verificação antes de navegar.
4. **Feedback** - `docs-preview/w2 #1` (texto da dica de ferramenta) e `#2` (ordem da barra lateral de localização) foram registrados em 18/06/2025 e foram resolvidos para 21/06/2025 (Docs-core-04/05); não há incidentes hubo durante la ola.

## Ações

| ID | Descrição | Responsável | Estado |
| --- | --- | --- | --- |
| W2-A1 | Atender `docs-preview/w2 #1` (redação da dica de ferramenta). | Documentos-núcleo-04 | Concluído 2025-06-21 |
| W2-A2 | Atender `docs-preview/w2 #2` (barra lateral de localização). | Documentos-núcleo-05 | Concluído 2025/06/21 |
| W2-A3 | Arquivar evidências de saída + atualizar roteiro/status. | Líder do Documentos/DevRel | Concluído 2025/06/29 |

## Resumo da saída (2025-06-29)

- Os outros revisores comunitários confirmam a finalização e se revogam o acesso à visualização; agradecimentos registrados no log de convites do rastreador.
- Os snapshots finais de telemetria (`docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals`) são mantidos verdes; logs e transcrições do proxy Experimente junto com `DOCS-SORA-Preview-W2`.
- Pacote de evidências (descritor, log de soma de verificação, saída da sonda, relatório de link, capturas de tela de Grafana, reconhecimentos de convite) arquivado abaixo de `artifacts/docs_preview/W2/preview-2025-06-15/`.
- O log de checkpoints W2 do rastreador é atualizado até o cierre, garantindo que o roadmap mantenha um registro auditável antes de iniciar o planejamento do W3.