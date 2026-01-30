---
lang: ja
direction: ltr
source: docs/portal/i18n/pt/docusaurus-plugin-content-docs/current/devportal/preview-feedback/w1/summary.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6bde7ad9f93475b62716a08daa20905447dcdc545d300e9a427e766e960bca6e
source_last_modified: "2025-11-14T04:43:19.907727+00:00"
translation_last_reviewed: 2026-01-30
---

| Item | Detalhes |
| --- | --- |
| Onda | W1 - Parceiros e integradores Torii |
| Janela de convite | 2025-04-12 -> 2025-04-26 |
| Tag de artefato | `preview-2025-04-12` |
| Issue do tracker | `DOCS-SORA-Preview-W1` |
| Participantes | sorafs-op-01...03, torii-int-01...02, sdk-partner-01...02, gateway-ops-01 |

## Destaques

1. **Fluxo de checksum** - Todos os reviewers validaram descriptor/archive via `scripts/preview_verify.sh`; logs armazenados junto aos acknowledgements de convite.
2. **Telemetria** - Dashboards `docs.preview.integrity`, `TryItProxyErrors` e `DocsPortal/GatewayRefusals` ficaram verdes por toda a onda; nenhum incidente ou pagina de alerta.
3. **Feedback docs (`docs-preview/w1`)** - Dois nits menores registrados:
   - `docs-preview/w1 #1`: esclarecer wording de navegacao na secao Try it (resolvido).
   - `docs-preview/w1 #2`: atualizar screenshot de Try it (resolvido).
4. **Paridade de runbooks** - Operadores de SoraFS confirmaram que os novos cross-links entre `orchestrator-ops` e `multi-source-rollout` resolveram as preocupacoes de W0.

## Itens de acao

| ID | Descricao | Responsavel | Status |
| --- | --- | --- | --- |
| W1-A1 | Atualizar wording de navegacao do Try it conforme `docs-preview/w1 #1`. | Docs-core-02 | Concluido (2025-04-18). |
| W1-A2 | Atualizar screenshot de Try it conforme `docs-preview/w1 #2`. | Docs-core-03 | Concluido (2025-04-19). |
| W1-A3 | Resumir achados de parceiros e evidencia de telemetria em roadmap/status. | Docs/DevRel lead | Concluido (ver tracker + status.md). |

## Resumo de encerramento (2025-04-26)

- Todos os oito reviewers confirmaram a conclusao durante as office hours finais, limparam artefatos locais e tiveram o acesso revogado.
- A telemetria ficou verde ate a saida; snapshots finais anexados a `DOCS-SORA-Preview-W1`.
- O log de convites foi atualizado com acknowledgements de saida; o tracker marcou W1 como concluido e adicionou os checkpoints.
- Bundle de evidencia (descriptor, checksum log, probe output, transcript do proxy Try it, screenshots de telemetria, feedback digest) arquivado em `artifacts/docs_preview/W1/`.

## Proximos passos

- Preparar o plano de intake comunitario W2 (aprovacao de governanca + ajustes no template de solicitacao).
- Atualizar o tag de artefato de preview para a onda W2 e reexecutar o script de preflight quando as datas estiverem finalizadas.
- Levar achados aplicaveis de W1 para roadmap/status para que a onda comunitaria tenha a orientacao mais recente.
