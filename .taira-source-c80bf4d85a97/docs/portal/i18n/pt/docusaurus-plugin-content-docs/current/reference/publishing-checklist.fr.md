---
lang: pt
direction: ltr
source: docs/portal/docs/reference/publishing-checklist.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Checklist de publicação

Use esta lista de verificação sempre que você estiver desenvolvendo o portal no dia a dia. Ela garantiu que a construção do CI, a implantação do GitHub Pages e os manuais de testes de fumaça cobrem cada seção antes de um lançamento ou um atraso no roteiro ao chegar.

## 1. Local de validação

- `npm run sync-openapi -- --version=current --latest` (adicione um ou mais sinalizadores `--mirror=<label>` em vez de Torii OpenAPI alterar para uma figura de instantâneo).
- `npm run build` - confirme que o texto do herói `Build on Iroha with confidence` aparece sempre em `build/index.html`.
- `./docs/portal/scripts/preview_verify.sh --build-dir build` - verifica o manifesto das somas de verificação (adicione `--descriptor`/`--archive` durante os testes de artefatos CI telecharges).
- `npm run serve` - lança o ajudante de visualização protegido por soma de verificação que verifica o manifesto antes da chamada `docusaurus serve`, para que os revisores nunca parcuram um instantâneo não assinado (o alias `serve:verified` permanece disponível para chamadas explícitas).
- Faça uma verificação pontual da remarcação que você modificou via `npm run start` e o servidor de recarga ao vivo.

## 2. Verificações de pull request

- Verifique se o trabalho `docs-portal-build` foi reusado em `.github/workflows/check-docs.yml`.
- Confirme que `ci/check_docs_portal.sh` está ativado (os logs CI exibem a verificação de fumaça do herói).
- Certifique-se de que o fluxo de trabalho de visualização carregue um manifesto (`build/checksums.sha256`) e que o script de verificação visualize novamente (os logs exibem a classificação `scripts/preview_verify.sh`).
- Adicione o URL de visualização publicado no ambiente GitHub Pages à descrição do PR.

## 3. Seção par de validação

| Seção | Proprietário | Lista de verificação |
|--------|-------|-----------|
| Página inicial | DevRel | O herói aparece, os cartões de início rápido apontam para as rotas válidas, os botões CTA resolvem. |
| Norito | Norito WG | Os guias de visão geral e primeiros passos referem-se aos sinalizadores anteriores da CLI e à documentação do esquema Norito. |
| SoraFS | Equipe de armazenamento | O início rápido é executado apenas, os campos do relatório do manifesto são documentados, as instruções de simulação de busca são verificadas. |
| Guias SDK | Leads SDK | Os guias Rust/Python/JS compilam exemplos atuais e os enviam para repositórios ao vivo. |
| Referência | Documentos/DevRel | O índice lista as especificações mais recentes, a referência do codec Norito corresponde a `norito.md`. |
| Artefato de visualização | Documentos/DevRel | O artefato `docs-portal-preview` está anexado ao PR, as verificações de fumaça passam, a garantia é compartilhada com os revisores. |
| Segurança e experimente sandbox | Documentos/DevRel Segurança | Configuração de login do código do dispositivo OAuth (`DOCS_OAUTH_*`), lista de verificação `security-hardening.md` executada, en-tetes CSP/Trusted Types verifica via `npm run build` ou `npm run probe:portal`. |

Cochez chaque ligne lors de la revue du PR, ou note toda ação de suivi para que le suivi de status seja exato.

## 4. Notas de lançamento- Inclua `https://docs.iroha.tech/` (ou o URL do problema de ambiente do trabalho de implantação) nas notas de lançamento e nas mises de hoje.
- Sinalize explicitamente toda seção nova ou modificada para que as equipes a jusante ensaquem ou relancem seus próprios testes de fumaça.