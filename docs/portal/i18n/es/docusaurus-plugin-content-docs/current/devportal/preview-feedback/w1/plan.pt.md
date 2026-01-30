---
lang: es
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w1/plan.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: preview-feedback-w1-plan
title: Plano de preflight de parceiros W1
sidebar_label: Plano W1
description: Tarefas, responsaveis e checklist de evidencia para a coorte de preview de parceiros.
---

| Item | Detalhes |
| --- | --- |
| Onda | W1 - Parceiros e integradores Torii |
| Janela alvo | Q2 2025 semana 3 |
| Tag de artefato (planejado) | `preview-2025-04-12` |
| Issue do tracker | `DOCS-SORA-Preview-W1` |

## Objetivos

1. Garantir aprovacoes legais e de governanca para os termos de preview de parceiros.
2. Preparar o proxy Try it e snapshots de telemetria usados no bundle de convite.
3. Atualizar o artefato de preview verificado por checksum e os resultados de probes.
4. Finalizar o roster de parceiros e os templates de solicitacao antes do envio dos convites.

## Desdobramento de tarefas

| ID | Tarefa | Responsavel | Prazo | Status | Notas |
| --- | --- | --- | --- | --- | --- |
| W1-P1 | Obter aprovacao legal para o adendo dos termos de preview | Docs/DevRel lead -> Legal | 2025-04-05 | Concluido | Ticket legal `DOCS-SORA-Preview-W1-Legal` aprovado em 2025-04-05; PDF anexado ao tracker. |
| W1-P2 | Capturar janela de staging do proxy Try it (2025-04-10) e validar a saude do proxy | Docs/DevRel + Ops | 2025-04-06 | Concluido | `npm run manage:tryit-proxy -- --stage preview-w1 --expires-in=21d --target https://tryit-preprod.sora` executado em 2025-04-06; transcricao de CLI e `.env.tryit-proxy.bak` arquivados. |
| W1-P3 | Construir artefato de preview (`preview-2025-04-12`), rodar `scripts/preview_verify.sh` + `npm run probe:portal`, arquivar descriptor/checksums | Portal TL | 2025-04-08 | Concluido | Artefato e logs de verificacao armazenados em `artifacts/docs_preview/W1/preview-2025-04-12/`; saida de probe anexada ao tracker. |
| W1-P4 | Revisar formularios de intake de parceiros (`DOCS-SORA-Preview-REQ-P01...P08`), confirmar contatos e NDAs | Governance liaison | 2025-04-07 | Concluido | As oito solicitacoes aprovadas (as duas ultimas em 2025-04-11); aprovacoes linkadas no tracker. |
| W1-P5 | Redigir o convite (baseado em `docs/examples/docs_preview_invite_template.md`), definir `<preview_tag>` e `<request_ticket>` para cada parceiro | Docs/DevRel lead | 2025-04-08 | Concluido | Rascunho do convite enviado em 2025-04-12 15:00 UTC junto com links de artefato. |

## Checklist de preflight

> Dica: rode `scripts/preview_wave_preflight.sh --tag preview-2025-04-12 --base-url https://preview.staging.sora --descriptor artifacts/preview-2025-04-12/descriptor.json --archive artifacts/preview-2025-04-12/docs-portal-preview.tar.zst --tryit-target https://tryit-proxy.staging.sora --output-json artifacts/preview-2025-04-12/preflight-summary.json` para executar automaticamente os passos 1-5 (build, verificacao de checksum, probe do portal, link checker e atualizacao do proxy Try it). O script registra um log JSON que voce pode anexar ao issue do tracker.

1. `npm run build` (com `DOCS_RELEASE_TAG=preview-2025-04-12`) para regenerar `build/checksums.sha256` e `build/release.json`.
2. `docs/portal/scripts/preview_verify.sh --build-dir docs/portal/build --descriptor artifacts/<tag>/descriptor.json --archive artifacts/<tag>/docs-portal-preview.tar.zst`.
3. `PORTAL_BASE_URL=https://preview.staging.sora DOCS_RELEASE_TAG=preview-2025-04-12 npm run probe:portal -- --expect-release=preview-2025-04-12`.
4. `DOCS_RELEASE_TAG=preview-2025-04-12 npm run check:links` e arquivar `build/link-report.json` ao lado do descriptor.
5. `npm run manage:tryit-proxy -- update --target https://tryit-proxy.staging.sora` (ou passar o target adequado via `--tryit-target`); commit do `.env.tryit-proxy` atualizado e guardar a `.bak` para rollback.
6. Atualizar o issue W1 com caminhos de logs (checksum do descriptor, saida de probe, mudanca no proxy Try it e snapshots Grafana).

## Checklist de evidencia

- [x] Aprovacao legal assinada (PDF ou link do ticket) anexada ao `DOCS-SORA-Preview-W1`.
- [x] Screenshots de Grafana para `docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals`.
- [x] Descriptor e log de checksum de `preview-2025-04-12` armazenados em `artifacts/docs_preview/W1/`.
- [x] Tabela de roster de convites com `invite_sent_at` preenchido (ver log W1 no tracker).
- [x] Artefatos de feedback refletidos em [`preview-feedback/w1/log.md`](./log.md) com uma linha por parceiro (atualizado em 2025-04-26 com dados de roster/telemetria/issues).

Atualize este plano conforme as tarefas avancarem; o tracker o referencia para manter o roadmap auditavel.

## Fluxo de feedback

1. Para cada reviewer, duplicar o template em
   [`docs/examples/docs_preview_feedback_form.md`](../../../../../examples/docs_preview_feedback_form.md),
   preencher os metadados e armazenar a copia completa em
   `artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/`.
2. Resumir convites, checkpoints de telemetria e issues abertos dentro do log vivo em
   [`preview-feedback/w1/log.md`](./log.md) para que reviewers de governanca possam rever toda a onda
   sem sair do repositorio.
3. Quando os exports de knowledge-check ou surveys chegarem, anexar no caminho de artefatos indicado no log
   e linkar o issue do tracker.
