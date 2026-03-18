---
id: preview-feedback-w3-summary
lang: pt
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w3/summary.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

| Item | Detalhes |
| --- | --- |
| Onda | W3 - Cohortes beta (financas + ops + partner SDK + advocate de ecossistema) |
| Janela de convite | 2026-02-18 -> 2026-02-28 |
| Tag de artefato | `preview-20260218` |
| Issue do tracker | `DOCS-SORA-Preview-W3` |
| Participantes | finance-beta-01, observability-ops-02, partner-sdk-03, ecosystem-advocate-04 |

## Destaques

1. **Pipeline de evidencia end-to-end.** `npm run preview:wave -- --wave preview-20260218 --invite-start 2026-02-18 --invite-end 2026-02-28 --report-date 2026-03-01 --notes "Finance/observability beta wave"` gera o resumo por onda (`artifacts/docs_portal_preview/preview-20260218-summary.json`), o digest (`preview-20260218-digest.md`) e atualiza `docs/portal/src/data/previewFeedbackSummary.json` para que reviewers de governanca possam depender de um unico comando.
2. **Cobertura de telemetria + governanca.** Os quatro reviewers reconheceram acesso com checksum, enviaram feedback e foram revogados no prazo; o digest referencia os issues de feedback (`docs-preview/20260218` set + `DOCS-SORA-Preview-20260218`) junto com os runs do Grafana coletados durante a onda.
3. **Visibilidade no portal.** A tabela do portal atualizada agora mostra a onda W3 encerrada com metricas de latencia e taxa de resposta, e a nova pagina de log abaixo espelha a linha do tempo para auditores que nao baixam o log JSON bruto.

## Itens de acao

| ID | Descricao | Responsavel | Status |
| --- | --- | --- | --- |
| W3-A1 | Capturar o digest de preview e anexar ao tracker. | Docs/DevRel lead | Concluido 2026-02-28 |
| W3-A2 | Espelhar evidencia de convite/digest no portal + roadmap/status. | Docs/DevRel lead | Concluido 2026-02-28 |

## Resumo de encerramento (2026-02-28)

- Convites enviados em 2026-02-18 com acknowledgements registrados minutos depois; acesso de preview revogado em 2026-02-28 apos a checagem final de telemetria.
- Digest + resumo armazenados em `artifacts/docs_portal_preview/`, com o log bruto ancorado por `artifacts/docs_portal_preview/feedback_log.json` para reprodutibilidade.
- Follow-ups de issues registrados em `docs-preview/20260218` com o tracker de governanca `DOCS-SORA-Preview-20260218`; notas de CSP/Try it roteadas aos owners de observability/financas e linkadas a partir do digest.
- A linha do tracker foi atualizada para Completed e a tabela de feedback do portal reflete a onda encerrada, concluindo a tarefa beta restante do DOCS-SORA.
