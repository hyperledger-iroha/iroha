---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w1/plan.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: preview-feedback-w1-plan
título: Plano de pré-voo de parceiros W1
sidebar_label: Plano W1
descrição: Tarefas, responsabilidades e checklist de evidência para o grupo de visualização de parceiros.
---

| Artigo | Detalhes |
| --- | --- |
| Onda | W1 - Parceiros e integradores Torii |
| Janela alvo | 2º trimestre de 2025, semana 3 |
| Tag de arte (planejado) | `preview-2025-04-12` |
| Problema do rastreador | `DOCS-SORA-Preview-W1` |

## Objetivos

1. Garantir aprovações legais e de governança para os termos de visualização de parceiros.
2. Preparar o proxy Try it e snapshots de telemetria usados ​​no pacote de convite.
3. Atualizar os artistas de visualização selecionados por checksum e os resultados de probes.
4. Finalizar a lista de parceiros e os modelos de solicitação antes do envio dos convites.

## Desdobramento de tarefas

| ID | Tarefa | Responsável | Prazo | Estado | Notas |
| --- | --- | --- | --- | --- | --- |
| W1-P1 | Obter aprovação legal para o adendo dos termos de visualização | Líder do Docs/DevRel -> Jurídico | 05/04/2025 | Concluído | Bilhete legal `DOCS-SORA-Preview-W1-Legal` aprovado em 2025-04-05; PDF anexado ao tracker. |
| W1-P2 | Capturar janela de staging do proxy Try it (2025-04-10) e validar a saúde do proxy | Documentos/DevRel + Operações | 06/04/2025 | Concluído | `npm run manage:tryit-proxy -- --stage preview-w1 --expires-in=21d --target https://tryit-preprod.sora` concluído em 06/04/2025; transcrição de CLI e `.env.tryit-proxy.bak` arquivados. |
| W1-P3 | Construir artefactos de pré-visualização (`preview-2025-04-12`), rodar `scripts/preview_verify.sh` + `npm run probe:portal`, descritor de arquivo/checksums | PortalTL | 08/04/2025 | Concluído | Artefato e logs de verificação armazenados em `artifacts/docs_preview/W1/preview-2025-04-12/`; saida de sonda anexada ao rastreador. |
| W1-P4 | Revisar formulários de admissão de parceiros (`DOCS-SORA-Preview-REQ-P01...P08`), confirmar contatos e NDAs | Ligação para governação | 07/04/2025 | Concluído | As oito solicitações aprovadas (as duas ultimas em 2025-04-11); aprovações vinculadas no tracker. |
| W1-P5 | Redigir o convite (baseado em `docs/examples/docs_preview_invite_template.md`), definir `<preview_tag>` e `<request_ticket>` para cada parceiro | Líder do Documentos/DevRel | 08/04/2025 | Concluído | Rascunho do convite enviado em 12/04/2025 15:00 UTC junto com links de artistas. |

## Checklist de pré-voo

> Dica: rode `scripts/preview_wave_preflight.sh --tag preview-2025-04-12 --base-url https://preview.staging.sora --descriptor artifacts/preview-2025-04-12/descriptor.json --archive artifacts/preview-2025-04-12/docs-portal-preview.tar.zst --tryit-target https://tryit-proxy.staging.sora --output-json artifacts/preview-2025-04-12/preflight-summary.json` para executar automaticamente os passos 1-5 (build, verificação de checksum, probe do portal, link checker e atualização do proxy Try it). O script registra um log JSON que você pode anexar ao issue do tracker.

1. `npm run build` (com `DOCS_RELEASE_TAG=preview-2025-04-12`) para regenerar `build/checksums.sha256` e `build/release.json`.
2.`docs/portal/scripts/preview_verify.sh --build-dir docs/portal/build --descriptor artifacts/<tag>/descriptor.json --archive artifacts/<tag>/docs-portal-preview.tar.zst`.
3.`PORTAL_BASE_URL=https://preview.staging.sora DOCS_RELEASE_TAG=preview-2025-04-12 npm run probe:portal -- --expect-release=preview-2025-04-12`.
4. `DOCS_RELEASE_TAG=preview-2025-04-12 npm run check:links` e arquivar `build/link-report.json` ao lado do descritor.
5. `npm run manage:tryit-proxy -- update --target https://tryit-proxy.staging.sora` (ou passar o alvo adequado via `--tryit-target`); commit do `.env.tryit-proxy` atualizado e salve o `.bak` para rollback.
6. Atualizar o issue W1 com caminhos de logs (checksum do descritor, saida de probe, mudanca no proxy Try it e snapshots Grafana).

## Checklist de evidências

- [x] Aprovação legal assinada (PDF ou link do ticket) anexada ao `DOCS-SORA-Preview-W1`.
- [x] Capturas de tela de Grafana para `docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals`.
- [x] Descritor e log de checksum de `preview-2025-04-12` armazenado em `artifacts/docs_preview/W1/`.
- [x] Tabela de lista de convites com `invite_sent_at` necessária (ver log W1 no tracker).
- [x] Artefatos de feedback refletidos em [`preview-feedback/w1/log.md`](./log.md) com uma linha por parceiro (atualizado em 2025-04-26 com dados de roster/telemetria/issues).

Atualizar este plano conforme as tarefas avancarem; o rastreador o referencial para manter o roadmap auditável.

## Fluxo de feedback

1. Para cada revisor, duplicar o modelo em
   [`docs/examples/docs_preview_feedback_form.md`](../../../../../examples/docs_preview_feedback_form.md),
   preencha os metadados e armazene a cópia completa em
   `artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/`.
2. Resumir convites, pontos de telemetria e questões abertas dentro do log vivo em
   [`preview-feedback/w1/log.md`](./log.md) para que revisores de governança possam rever toda a onda
   sem sair do repositório.
3. Quando as exportações de conhecimento-verificação ou pesquisas chegam, fixam-se no caminho dos artefatos indicados no log
   e linkar o problema do rastreador.