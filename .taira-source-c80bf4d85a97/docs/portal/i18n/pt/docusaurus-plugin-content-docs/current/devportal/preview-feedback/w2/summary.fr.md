---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w2/summary.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: visualização-feedback-w2-resumo
título: Retomar feedback e status W2
sidebar_label: Retomar W2
descrição: Resumo direto para a vaga de visualização comunitária (W2).
---

| Elemento | Detalhes |
| --- | --- |
| Vago | W2 - Revisores comunitários |
| Janela de convite | 15/06/2025 -> 29/06/2025 |
| Etiqueta do artefato | `preview-2025-06-15` |
| Rastreador de problemas | `DOCS-SORA-Preview-W2` |
| Participantes | comm-vol-01...comm-vol-08 |

## Pontos marinheiros

1. **Governança e ferramentas** - La politique d'intake communautaire approuvee a l'unanimite le 2025-05-20; O modelo de demanda para o dia com Champs Motivation / Fuseau Horaire está em `docs/examples/docs_preview_request_template.md`.
2. **Preflight et preuves** - A alteração do proxy Try it `OPS-TRYIT-188` executa o 2025-06-09, os painéis Grafana capturam e os descritores de saída/checksum/sonda dos arquivos `preview-2025-06-15` sob `artifacts/docs_preview/W2/`.
3. **Convites vagos** - Huit revisores comunitários convida le 2025-06-15, com acusações registradas na tabela de convites do rastreador; Você deve encerrar a soma de verificação de verificação antes da navegação.
4. **Feedback** - `docs-preview/w2 #1` (texto da dica de ferramenta) e `#2` (ordem da barra lateral de localização) ont ete saisis le 2025-06-18 et resolus d'ici 2025-06-21 (Docs-core-04/05); nenhum incidente pendente la vago.

## Ações

| ID | Descrição | Responsável | Estatuto |
| --- | --- | --- | --- |
| W2-A1 | Traiter `docs-preview/w2 #1` (redação da dica de ferramenta). | Documentos-núcleo-04 | Término 21/06/2025 |
| W2-A2 | Traiter `docs-preview/w2 #2` (barra lateral de localização). | Documentos-núcleo-05 | Término 21/06/2025 |
| W2-A3 | Arquivar as previsões de saída + fornecer o roteiro/status do dia. | Líder do Documentos/DevRel | Término 29/06/2025 |

## Currículo da surtida (29/06/2025)

- Les huit reviewers communautaires ont confirme la fin et l'access preview a ete revoque; acusa inscritos no log de convite do rastreador.
- Os snapshots finais de telemetria (`docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals`) são restantes; logs e transcrições do proxy Tente anexar um `DOCS-SORA-Preview-W2`.
- Pacote de testes (descritor, log de soma de verificação, saída de sonda, relatório de link, capturas de tela Grafana, acusações de convite) arquivo sob `artifacts/docs_preview/W2/preview-2025-06-15/`.
- O registro dos pontos de verificação W2 do rastreador até o momento apenas na saída, garante um registro auditável antes do encerramento do planejamento W3.