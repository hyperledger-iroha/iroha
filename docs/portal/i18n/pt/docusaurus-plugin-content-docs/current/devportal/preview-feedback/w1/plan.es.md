---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w1/plan.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: preview-feedback-w1-plan
título: Plano de pré-voo dos parceiros W1
sidebar_label: Plano W1
descrição: Tareas, responsáveis e lista de verificação de evidências para o grupo de visualização de parceiros.
---

| Artigo | Detalhes |
| --- | --- |
| Olá | W1 - Parceiros e integradores de Torii |
| Ventana objetivo | 2º trimestre de 2025, semana 3 |
| Etiqueta de artefato (planeado) | `preview-2025-04-12` |
| Emissão do rastreador | `DOCS-SORA-Preview-W1` |

## Objetivos

1. Garantir aprovações legais e de governança para os termos de pré-visualização dos parceiros.
2. Prepare o proxy Try it e os instantâneos de telemetria usados ​​no pacote de convite.
3. Atualize o artefato de visualização selecionado por checksum e os resultados das sondas.
4. Finalize a lista de parceiros e as plantas de solicitação antes de enviar os convites.

## Desglose de tareias

| ID | Tara | Responsável | Data limite | Estado | Notas |
| --- | --- | --- | --- | --- | --- |
| W1-P1 | Obter aprovação legal para o anexo dos termos de visualização | Líder do Docs/DevRel -> Jurídico | 05/04/2025 | Concluído | Bilhete legal `DOCS-SORA-Preview-W1-Legal` aprovado em 2025-04-05; PDF anexado ao rastreador. |
| W1-P2 | Capturar janela de teste do proxy Try it (2025-04-10) e validar a saúde do proxy | Documentos/DevRel + Operações | 06/04/2025 | Concluído | Se executado `npm run manage:tryit-proxy -- --stage preview-w1 --expires-in=21d --target https://tryit-preprod.sora` em 2025-04-06; transcrição de CLI e `.env.tryit-proxy.bak` arquivados. |
| W1-P3 | Construir artefato de visualização (`preview-2025-04-12`), executar `scripts/preview_verify.sh` + `npm run probe:portal`, descritor de arquivo/checksums | PortalTL | 08/04/2025 | Concluído | Artefato e logs de verificação salvos em `artifacts/docs_preview/W1/preview-2025-04-12/`; saída de sonda adjunta ao rastreador. |
| W1-P4 | Revisar formulários de admissão de parceiros (`DOCS-SORA-Preview-REQ-P01...P08`), confirmar contatos e NDAs | Ligação para governação | 07/04/2025 | Concluído | Las ocho solicitudes aprovadas (las ultimas dos el 2025-04-11); aprovações enlazadas no rastreador. |
| W1-P5 | Redigir cópia do convite (baseado em `docs/examples/docs_preview_invite_template.md`), fixar `<preview_tag>` e `<request_ticket>` para cada parceiro | Líder do Documentos/DevRel | 08/04/2025 | Concluído | Borrador de convite enviado em 12/04/2025 às 15:00 UTC junto com links de artefato. |

## Checklist de pré-voo

> Conselho: execute `scripts/preview_wave_preflight.sh --tag preview-2025-04-12 --base-url https://preview.staging.sora --descriptor artifacts/preview-2025-04-12/descriptor.json --archive artifacts/preview-2025-04-12/docs-portal-preview.tar.zst --tryit-target https://tryit-proxy.staging.sora --output-json artifacts/preview-2025-04-12/preflight-summary.json` para executar as etapas 1-5 automaticamente (build, verificação de checksum, sonda do portal, verificador de link e atualização do proxy Experimente). O script registra um log JSON que pode ser adicionado ao problema do rastreador.

1. `npm run build` (com `DOCS_RELEASE_TAG=preview-2025-04-12`) para regenerar `build/checksums.sha256` e `build/release.json`.
2.`docs/portal/scripts/preview_verify.sh --build-dir docs/portal/build --descriptor artifacts/<tag>/descriptor.json --archive artifacts/<tag>/docs-portal-preview.tar.zst`.
3.`PORTAL_BASE_URL=https://preview.staging.sora DOCS_RELEASE_TAG=preview-2025-04-12 npm run probe:portal -- --expect-release=preview-2025-04-12`.
4. `DOCS_RELEASE_TAG=preview-2025-04-12 npm run check:links` e arquivar `build/link-report.json` junto com o descritor.
5. `npm run manage:tryit-proxy -- update --target https://tryit-proxy.staging.sora` (ou passar o alvo adequado via `--tryit-target`); comprometa o `.env.tryit-proxy` atualizado e mantenha o `.bak` para reversão.
6. Atualize o problema W1 com rotas de logs (checksum do descritor, saída do probe, mudança do proxy Try it e snapshots Grafana).

## Checklist de evidências

- [x] Aprovação legal firmada (PDF ou link para ticket) adjunta a `DOCS-SORA-Preview-W1`.
- [x] Capturas de tela de Grafana para `docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals`.
- [x] Descritor e log de checksum de `preview-2025-04-12` salvos abaixo de `artifacts/docs_preview/W1/`.
- [x] Tabela de lista de convites com carimbos de data e hora `invite_sent_at` completos (ver log W1 do tracker).
- [x] Artefatos de feedback reflejados em [`preview-feedback/w1/log.md`](./log.md) com um arquivo por parceiro (atualizado em 2025-04-26 com dados de escalação/telemetria/edições).

Atualiza este plano na medida em que avança as tarefas; o rastreador é a referência para manter o roteiro auditável.

## Fluxo de feedback

1. Para cada revisor, duplique la plantilla en
   [`docs/examples/docs_preview_feedback_form.md`](../../../../../examples/docs_preview_feedback_form.md),
   complete os metadados e guarde a cópia terminada abaixo
   `artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/`.
2. Retomar convites, pontos de controle de telemetria e questões abertas dentro do log vivo em
   [`preview-feedback/w1/log.md`](./log.md) para que os revisores de governo possam revisar toda a ola
   sem sair do repositório.
3. Ao iniciar a exportação de verificação de conhecimento ou consultas, adicione-os à rota de artefatos indicados no registro
   e feche o problema do rastreador.