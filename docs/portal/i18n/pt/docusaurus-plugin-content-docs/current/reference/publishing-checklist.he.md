---
lang: he
direction: rtl
source: docs/portal/i18n/pt/docusaurus-plugin-content-docs/current/reference/publishing-checklist.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4346923ffe28a953408c80ec3d7f1ed3cefdd4a1e495b961a8467b4e52c32998
source_last_modified: "2026-01-03T18:08:01+00:00"
translation_last_reviewed: 2026-01-30
---

---
lang: pt
direction: ltr
source: docs/portal/docs/reference/publishing-checklist.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# Checklist de publicacao

Use este checklist sempre que atualizar o portal de desenvolvedores. Ele garante que o build de CI, o deploy no GitHub Pages e os smoke tests manuais cobrem todas as secoes antes de um release ou marco do roadmap.

## 1. Validacao local

- `npm run sync-openapi -- --version=current --latest` (adicione um ou mais flags `--mirror=<label>` quando o Torii OpenAPI mudar para um snapshot congelado).
- `npm run build` - confirme que o hero copy `Build on Iroha with confidence` ainda aparece em `build/index.html`.
- `./docs/portal/scripts/preview_verify.sh --build-dir build` - verifique o manifesto de checksums (adicione `--descriptor`/`--archive` ao testar artefatos de CI baixados).
- `npm run serve` - inicia o helper de preview com checksum gating que verifica o manifesto antes de chamar `docusaurus serve`, para que reviewers nunca naveguem um snapshot sem assinatura (o alias `serve:verified` permanece para chamadas explicitas).
- Faca um spot-check do markdown alterado via `npm run start` e o servidor de live reload.

## 2. Checks de pull request

- Verifique que o job `docs-portal-build` passou em `.github/workflows/check-docs.yml`.
- Confirme que `ci/check_docs_portal.sh` rodou (logs de CI mostram o hero smoke check).
- Garanta que o workflow de preview enviou um manifesto (`build/checksums.sha256`) e que o script de verificacao de preview foi bem-sucedido (logs mostram a saida de `scripts/preview_verify.sh`).
- Adicione a URL de preview publicada do ambiente GitHub Pages na descricao do PR.

## 3. Aprovacao por secao

| Secao | Owner | Checklist |
|---------|-------|-----------|
| Homepage | DevRel | Hero copy renderiza, quickstart cards linkam para rotas validas, botoes CTA resolvem. |
| Norito | Norito WG | Guias overview e getting-started referenciam os flags mais recentes do CLI e os docs do schema Norito. |
| SoraFS | Storage Team | Quickstart roda ate o fim, campos do report de manifest documentados, instrucoes de simulacao de fetch verificadas. |
| SDK guides | SDK leads | Guias Rust/Python/JS compilam os exemplos atuais e linkam para repos live. |
| Reference | Docs/DevRel | O index lista as specs mais recentes, a referencia do codec Norito coincide com `norito.md`. |
| Preview artifact | Docs/DevRel | O artefato `docs-portal-preview` esta anexado ao PR, smoke checks passam, o link e compartilhado com reviewers. |
| Security & Try it sandbox | Docs/DevRel / Security | OAuth device-code login configurado (`DOCS_OAUTH_*`), checklist `security-hardening.md` executada, headers CSP/Trusted Types verificados via `npm run build` ou `npm run probe:portal`. |

Marque cada linha como parte do seu review do PR, ou anote tarefas de follow-up para manter o tracking de status preciso.

## 4. Release notes

- Inclua `https://docs.iroha.tech/` (ou a URL do ambiente do job de deployment) nas release notes e atualizacoes de status.
- Destaque quaisquer secoes novas ou alteradas para que as equipes downstream saibam onde reexecutar seus proprios smoke tests.
