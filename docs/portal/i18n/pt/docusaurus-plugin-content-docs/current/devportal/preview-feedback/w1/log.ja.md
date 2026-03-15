---
lang: ja
direction: ltr
source: docs/portal/i18n/pt/docusaurus-plugin-content-docs/current/devportal/preview-feedback/w1/log.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 22bbfd6fbcb208a632fcd24ad1478a68a0c679f5ec0c9415994ca95196a46300
source_last_modified: "2026-01-03T18:08:02+00:00"
translation_last_reviewed: 2026-01-30
---

---
id: preview-feedback-w1-log
lang: pt
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w1/log.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

Este log mantem o roster de convites, checkpoints de telemetria e feedback de reviewers para o
**preview de parceiros W1** que acompanha as tarefas de aceitacao em
[`preview-feedback/w1/plan.md`](./plan.md) e a entrada do tracker da onda em
[`../../preview-invite-tracker.md`](../../preview-invite-tracker.md). Atualize quando um convite for enviado,
um snapshot de telemetria for registrado ou um item de feedback for triado para que reviewers de governanca possam
reproduzir as evidencias sem perseguir tickets externos.

## Roster da coorte

| Partner ID | Ticket de solicitacao | NDA recebido | Convite enviado (UTC) | Ack/primeiro login (UTC) | Status | Notas |
| --- | --- | --- | --- | --- | --- | --- |
| partner-w1-01 | `DOCS-SORA-Preview-REQ-P01` | OK 2025-04-03 | 2025-04-12 15:00 | 2025-04-12 15:11 | Concluido 2025-04-26 | sorafs-op-01; focado em evidencia de paridade dos docs do orchestrator. |
| partner-w1-02 | `DOCS-SORA-Preview-REQ-P02` | OK 2025-04-03 | 2025-04-12 15:03 | 2025-04-12 15:15 | Concluido 2025-04-26 | sorafs-op-02; validou cross-links Norito/telemetria. |
| partner-w1-03 | `DOCS-SORA-Preview-REQ-P03` | OK 2025-04-04 | 2025-04-12 15:06 | 2025-04-12 15:18 | Concluido 2025-04-26 | sorafs-op-03; executou drills de failover multi-source. |
| partner-w1-04 | `DOCS-SORA-Preview-REQ-P04` | OK 2025-04-04 | 2025-04-12 15:09 | 2025-04-12 15:21 | Concluido 2025-04-26 | torii-int-01; revisao do cookbook Torii `/v1/pipeline` + Try it. |
| partner-w1-05 | `DOCS-SORA-Preview-REQ-P05` | OK 2025-04-05 | 2025-04-12 15:12 | 2025-04-12 15:23 | Concluido 2025-04-26 | torii-int-02; acompanhou a atualizacao do screenshot Try it (docs-preview/w1 #2). |
| partner-w1-06 | `DOCS-SORA-Preview-REQ-P06` | OK 2025-04-05 | 2025-04-12 15:15 | 2025-04-12 15:26 | Concluido 2025-04-26 | sdk-partner-01; feedback de cookbooks JS/Swift + sanity checks do ISO bridge. |
| partner-w1-07 | `DOCS-SORA-Preview-REQ-P07` | OK 2025-04-11 | 2025-04-12 15:18 | 2025-04-12 15:29 | Concluido 2025-04-26 | sdk-partner-02; compliance aprovado 2025-04-11, focado em notas de Connect/telemetria. |
| partner-w1-08 | `DOCS-SORA-Preview-REQ-P08` | OK 2025-04-11 | 2025-04-12 15:21 | 2025-04-12 15:33 | Concluido 2025-04-26 | gateway-ops-01; auditou o guia ops do gateway + fluxo anonimo do proxy Try it. |

Preencha os timestamps de **Convite enviado** e **Ack** assim que o email de saida for emitido.
Ancore os horarios no cronograma UTC definido no plano W1.

## Checkpoints de telemetria

| Timestamp (UTC) | Dashboards / probes | Responsavel | Resultado | Artefato |
| --- | --- | --- | --- | --- |
| 2025-04-06 18:05 | `docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals` | Docs/DevRel + Ops | Tudo verde | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250406` |
| 2025-04-06 18:20 | Transcript de `npm run manage:tryit-proxy -- --stage preview-w1` | Ops | Staged | `artifacts/docs_preview/W1/preview-2025-04-12/tryit/OPS-TRYIT-147.log` |
| 2025-04-12 14:45 | Dashboards acima + `probe:portal` | Docs/DevRel + Ops | Snapshot pre-invite, sem regressao | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250412` |
| 2025-04-19 17:55 | Dashboards acima + diff de latencia do proxy Try it | Docs/DevRel lead | Checkpoint de meio aprovado (0 alertas; latencia Try it p95=410 ms) | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250419` |
| 2025-04-26 16:25 | Dashboards acima + probe de saida | Docs/DevRel + Governance liaison | Snapshot de saida, zero alertas pendentes | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250426` |

As amostras diarias de office hours (2025-04-13 -> 2025-04-25) estao agrupadas como exports NDJSON + PNG em
`artifacts/docs_preview/W1/preview-2025-04-12/grafana/daily/` com nomes de arquivo
`docs-preview-integrity-<date>.json` e screenshots correspondentes.

## Log de feedback e issues

Use esta tabela para resumir achados enviados por reviewers. Vincule cada entrada ao ticket GitHub/discuss
mais o formulario estruturado capturado via
[`docs/examples/docs_preview_feedback_form.md`](../../../../../examples/docs_preview_feedback_form.md).

| Referencia | Severidade | Responsavel | Status | Notas |
| --- | --- | --- | --- | --- |
| `docs-preview/w1 #1` | Low | Docs-core-02 | Resolvido 2025-04-18 | Esclareceu o wording de nav do Try it + ancora de sidebar (`docs/source/sorafs/tryit.md` atualizado com novo label). |
| `docs-preview/w1 #2` | Low | Docs-core-03 | Resolvido 2025-04-19 | Screenshot do Try it + legenda atualizados conforme pedido; artefato `artifacts/docs_preview/W1/preview-2025-04-12/feedback/partner-w1-05/screenshot-diff.png`. |
| - | Info | Docs/DevRel lead | Fechado | Comentarios restantes foram apenas Q&A; capturados no formulario de feedback de cada parceiro sob `artifacts/docs_preview/W1/preview-2025-04-12/feedback/`. |

## Acompanhamento de knowledge check e surveys

1. Registre as notas do quiz (meta >=90%) para cada reviewer; anexe o CSV exportado ao lado dos artefatos de convite.
2. Colete as respostas qualitativas do survey capturadas no template de feedback e espelhe em
   `artifacts/docs_preview/W1/preview-2025-04-12/surveys/`.
3. Agende chamadas de remediation para quem estiver abaixo do limite e registre aqui.

Todos os oito reviewers marcaram >=94% no knowledge check (CSV:
`artifacts/docs_preview/W1/preview-2025-04-12/feedback/w1-quiz-scores.csv`). Nenhuma chamada de remediation
foi necessaria; exports de survey para cada parceiro vivem em
`artifacts/docs_preview/W1/preview-2025-04-12/surveys/<partner-id>/summary.json`.

## Inventario de artefatos

- Bundle preview descriptor/checksum: `artifacts/docs_preview/W1/preview-2025-04-12/descriptor.json`
- Resumo de probe + link-check: `artifacts/docs_preview/W1/preview-2025-04-12/preflight-summary.json`
- Log de mudanca do proxy Try it: `artifacts/docs_preview/W1/preview-2025-04-12/tryit/OPS-TRYIT-147.log`
- Exports de telemetria: `artifacts/docs_preview/W1/preview-2025-04-12/grafana/<date>/`
- Bundle diario de telemetria de office hours: `artifacts/docs_preview/W1/preview-2025-04-12/grafana/daily/`
- Exports de feedback + survey: colocar pastas por reviewer em
  `artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/`
- CSV e resumo do knowledge check: `artifacts/docs_preview/W1/preview-2025-04-12/feedback/w1-quiz-scores.csv`

Mantenha o inventario sincronizado com o issue do tracker. Anexe hashes ao copiar artefatos para o ticket de governanca
para que auditores verifiquem os arquivos sem acesso de shell.
