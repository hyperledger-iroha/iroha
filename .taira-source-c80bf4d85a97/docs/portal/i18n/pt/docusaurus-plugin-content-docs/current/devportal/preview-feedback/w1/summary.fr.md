---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w1/summary.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: visualização-feedback-w1-resumo
título: Retomar feedback e surtida W1
sidebar_label: Currículo W1
descrição: Estatísticas, ações e testes de triagem para a vaga de visualização de parceiros/integradores Torii.
---

| Elemento | Detalhes |
| --- | --- |
| Vago | W1 - Partes e integradores Torii |
| Janela de convite | 12/04/2025 -> 26/04/2025 |
| Etiqueta do artefato | `preview-2025-04-12` |
| Rastreador de problemas | `DOCS-SORA-Preview-W1` |
| Participantes | sorafs-op-01...03, torii-int-01...02, sdk-partner-01...02, gateway-ops-01 |

## Pontos marinheiros

1. **Soma de verificação do fluxo de trabalho** - Todos os revisores verificarão o descritor/arquivo via `scripts/preview_verify.sh`; les logs ont ete stockes avec les accuses d'invitation.
2. **Telemetria** - Os painéis `docs.preview.integrity`, `TryItProxyErrors` e `DocsPortal/GatewayRefusals` são os restantes verdes pendentes de toda vaga; nenhum incidente na página de alerta.
3. **Documentos de feedback (`docs-preview/w1`)** - Dois nits mineurs ont ete signales:
   - `docs-preview/w1 #1`: esclarece a formulação de navegação na seção Try it (resolu).
   - `docs-preview/w1 #2`: mettre a jour la capture Experimente (resolu).
4. **Parite runbook** - Os operadores SoraFS confirmaram que os novos links cruzados entre `orchestrator-ops` e `multi-source-rollout` traçaram seus pontos W0.

## Ações

| ID | Descrição | Responsável | Estatuto |
| --- | --- | --- | --- |
| W1-A1 | Mettre a jour la formulation de navigation Try it selon `docs-preview/w1 #1`. | Documentos-núcleo-02 | Término (18/04/2025). |
| W1-A2 | Rafraichir la capture Try it selon `docs-preview/w1 #2`. | Documentos-núcleo-03 | Término (19/04/2025). |
| W1-A3 | Retome os dados constantes dos parceiros e a previsão de telemetria no roteiro/status. | Líder do Documentos/DevRel | Terminar (ver rastreador + status.md). |

## Currículo da surtida (2025-04-26)

- Os revisores da casa não confirmarão o final do horário de expediente, limparão os artefatos locais e seu acesso será revogado.
- La telemetrie est restee verte jusqu'a la sortie; snapshots finaux anexa um `DOCS-SORA-Preview-W1`.
- Le log d'invitations a ete mis a jour com as acusações de surtida; le tracker da marca W1 termina e adiciona pontos de verificação.
- Pacote de teste (descritor, log de soma de verificação, saída da sonda, transcrição do proxy Experimente, capturas de tela de telemetria, resumo de feedback) arquivado em `artifacts/docs_preview/W1/`.

## Étapes de Prochaines

- Preparador do plano de admissão comunitário W2 (governança de aprovação + ajustes do modelo de demanda).
- Rafraichir a etiqueta de visualização do artefato para o vago W2 e relançar o script de pré-voo uma vez nas datas finalizadas.
- Porter as constantes aplicáveis ​​do W1 no roteiro/status para que a vaga comunidade nas indicações posteriores.