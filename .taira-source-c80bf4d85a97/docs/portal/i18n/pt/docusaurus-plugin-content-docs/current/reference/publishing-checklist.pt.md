---
lang: pt
direction: ltr
source: docs/portal/docs/reference/publishing-checklist.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Checklist de publicação

Use esta lista de verificação sempre que atualizar o portal de desenvolvedores. Ele garante que o build de CI, o deploy no GitHub Pages e os manuais de testes de fumaça cobrem todas as etapas antes de um lançamento ou marco do roadmap.

## 1. Validação local

- `npm run sync-openapi -- --version=current --latest` (adicione um ou mais flags `--mirror=<label>` quando o Torii OpenAPI mudar para um snapshot congelado).
- `npm run build` - confirme que o herói copy `Build on Iroha with confidence` ainda aparece em `build/index.html`.
- `./docs/portal/scripts/preview_verify.sh --build-dir build` - verifique o manifesto de checksums (adicione `--descriptor`/`--archive` ao testar artefatos de CI baixados).
- `npm run serve` - inicia o helper de preview com checksum gating que verifica o manifesto antes de chamar `docusaurus serve`, para que os revisores nunca naveguem um snapshot sem assinatura (o alias `serve:verified` permanece para chamadas explícitas).
- Faça um spot-check do markdown alterado via `npm run start` e o servidor de live reload.

## 2. Verifica a solicitação pull

- Verifique se o job `docs-portal-build` passou em `.github/workflows/check-docs.yml`.
- Confirme que `ci/check_docs_portal.sh` rodou (logs de CI mostram o hero smoke check).
- Garanta que o fluxo de trabalho de visualização enviou um manifesto (`build/checksums.sha256`) e que o script de verificação de visualização foi bem-sucedido (logs mostram a mensagem de `scripts/preview_verify.sh`).
- Adicione a URL da visualização publicada no ambiente GitHub Pages na descrição do PR.

## 3. Aprovação por temporada

| Secção | Proprietário | Lista de verificação |
|--------|-------|-----------|
| Página inicial | DevRel | Hero copy renderiza, quickstart cards linkam para rotas validas, botoes CTA resolvem. |
| Norito | Norito WG | Guias Overview e Getting Started referenciam os flags mais recentes do CLI e os documentos do esquema Norito. |
| SoraFS | Equipe de armazenamento | Quickstart roda até o fim, campos do relatório de manifesto documentados, instruções de simulação de busca verificadas. |
| Guias do SDK | Leads do SDK | Guias Rust/Python/JS compilam os exemplos atuais e linkam para repositórios live. |
| Referência | Documentos/DevRel | O índice lista as especificações mais recentes, a referência do codec Norito coincide com `norito.md`. |
| Artefato de visualização | Documentos/DevRel | Os artigos `docs-portal-preview` estão anexados ao PR, smoke checks passam, o link e compartilhado com revisores. |
| Segurança e experimente sandbox | Documentos/DevRel/Segurança | Login do código do dispositivo OAuth configurado (`DOCS_OAUTH_*`), checklist `security-hardening.md` realizado, cabeçalhos CSP/Trusted Types selecionados via `npm run build` ou `npm run probe:portal`. |

Marque cada linha como parte de sua revisão do PR, ou anote tarefas de acompanhamento para manter o rastreamento de status preciso.

## 4. Notas de lançamento

- Inclui `https://docs.iroha.tech/` (ou a URL do ambiente do trabalho de implantação) nas notas de lançamento e atualizações de status.
- Destaque quaisquer secas novas ou alteradas para que as equipes downstream saibam onde reexecutar seus próprios testes de fumaça.