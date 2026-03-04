---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w1/plan.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: preview-feedback-w1-plan
título: Plano de pré-voo partenaires W1
sidebar_label: Plano W1
descrição: Lista de verificação, responsáveis e lista de verificação para o grupo de parceiros de visualização.
---

| Elemento | Detalhes |
| --- | --- |
| Vago | W1 - Partes e integradores Torii |
| Fenetre cible | Semana 3 do 2º trimestre de 2025 |
| Tag d'artefact (planificação) | `preview-2025-04-12` |
| Rastreador de problemas | `DOCS-SORA-Preview-W1` |

## Objetivos

1. Obter as aprovações legais e de governança para os termos de pré-visualização dos parceiros.
2. Prepare o proxy Experimente e use os instantâneos de telemetria no pacote de convite.
3. Rafraichir o artefato de visualização de verificação por soma de verificação e os resultados das sondagens.
4. Finalize a lista de participantes e os modelos solicitados antes do envio dos convites.

## Decoupage des taches

| ID | Tache | Responsável | Echeance | Estatuto | Notas |
| --- | --- | --- | --- | --- | --- |
| W1-P1 | Obter a aprovação legal para o adendo aos termos de visualização | Líder do Docs/DevRel -> Jurídico | 05/04/2025 | Término | Bilhete legal `DOCS-SORA-Preview-W1-Legal` válido para 05/04/2025; PDF anexado ao rastreador. |
| W1-P2 | Capture a janela de teste do proxy Try it (2025-04-10) e valide a segurança do proxy | Documentos/DevRel + Operações | 06/04/2025 | Término | `npm run manage:tryit-proxy -- --stage preview-w1 --expires-in=21d --target https://tryit-preprod.sora` execute o arquivo 2025-04-06; arquivos de transcrição CLI + `.env.tryit-proxy.bak`. |
| W1-P3 | Construa o artefato de visualização (`preview-2025-04-12`), executor `scripts/preview_verify.sh` + `npm run probe:portal`, descritor de arquivador/somas de verificação | PortalTL | 08/04/2025 | Término | Artefato + registros de verificação de estoques em `artifacts/docs_preview/W1/preview-2025-04-12/`; sortie de sonda anexada ao rastreador. |
| W1-P4 | Revoir les formulaires d'intake partenaires (`DOCS-SORA-Preview-REQ-P01...P08`), contatos de confirmação + NDAs | Ligação para governação | 07/04/2025 | Término | Les huit demandes approuvees (les deux dernieres le 2025-04-11); aprovações mentiras no rastreador. |
| W1-P5 | Redija o texto do convite (com base em `docs/examples/docs_preview_invite_template.md`), defina `<preview_tag>` e `<request_ticket>` para cada partenaire | Líder do Documentos/DevRel | 08/04/2025 | Término | Brouillon d'invitation enviado para 2025-04-12 15:00 UTC com les liens d'artefact. |

## Pré-impressão da lista de verificação

> Astuce: lancez `scripts/preview_wave_preflight.sh --tag preview-2025-04-12 --base-url https://preview.staging.sora --descriptor artifacts/preview-2025-04-12/descriptor.json --archive artifacts/preview-2025-04-12/docs-portal-preview.tar.zst --tryit-target https://tryit-proxy.staging.sora --output-json artifacts/preview-2025-04-12/preflight-summary.json` para executar automaticamente as etapas 1-5 (construção, soma de verificação de verificação, sonda do portal, verificador de link e mise a jour du proxy Try it). O script registra um log JSON para se juntar ao rastreador.

1. `npm run build` (com `DOCS_RELEASE_TAG=preview-2025-04-12`) para regenerar `build/checksums.sha256` e `build/release.json`.
2.`docs/portal/scripts/preview_verify.sh --build-dir docs/portal/build --descriptor artifacts/<tag>/descriptor.json --archive artifacts/<tag>/docs-portal-preview.tar.zst`.
3.`PORTAL_BASE_URL=https://preview.staging.sora DOCS_RELEASE_TAG=preview-2025-04-12 npm run probe:portal -- --expect-release=preview-2025-04-12`.
4. `DOCS_RELEASE_TAG=preview-2025-04-12 npm run check:links` e arquivador `build/link-report.json` na parte inferior do descritor.
5. `npm run manage:tryit-proxy -- update --target https://tryit-proxy.staging.sora` (ou forneça o código apropriado via `--tryit-target`); comprometa o `.env.tryit-proxy` hoje e salve o `.bak` para reverter.
6. Execute recentemente o problema W1 com os arquivos de log (soma de verificação do descritor, sonda de triagem, alteração do proxy Try it e instantâneos Grafana).

## Checklist prévio

- [x] Aprovação legale signee (PDF ou lien du ticket) adido a `DOCS-SORA-Preview-W1`.
- [x] Capturas de tela Grafana para `docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals`.
- [x] Descritor e log de soma de verificação `preview-2025-04-12` armazenados sob `artifacts/docs_preview/W1/`.
- [x] Tabela de lista de convites com `invite_sent_at` renderizados (veja o log W1 do rastreador).
- [x] Artefatos de feedback publicados em [`preview-feedback/w1/log.md`](./log.md) com uma linha por parceiro (até hoje, 26/04/2025 com escalação/telemetria/edições).

Mettre a jour ce plan a mesure de l'avancement; O rastreador é uma referência para monitorar o roteiro auditável.

## Fluxo de feedback

1. Para cada revisor, duplique o modelo em
   [`docs/examples/docs_preview_feedback_form.md`](../../../../../examples/docs_preview_feedback_form.md),
   remplir les metadonnees et stocker la copie complete sous
   `artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/`.
2. Convites de currículo, pontos de verificação de telemetria e problemas abertos no registro vivo
   [`preview-feedback/w1/log.md`](./log.md) para que os revisores de governança possam regozijar-se com a vaga
   sem desistir do depósito.
3. Quando as exportações de verificação de conhecimento ou de sondagens chegam, as juntas no caminho do artefato são anotadas no registro
   e aqui está o problema do rastreador.