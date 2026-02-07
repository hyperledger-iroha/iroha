---
lang: pt
direction: ltr
source: docs/portal/docs/reference/publishing-checklist.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Lista de verificação de publicação

Use esta lista sempre que atualizar o portal de desenvolvimento. Certifique-se de que a construção do CI, a aplicação nas páginas do GitHub e os manuais de humo revisem cada seção antes de lançar um lançamento ou um hit do roadmap.

## 1. Validação local

- `npm run sync-openapi -- --version=current --latest` (agregar um ou mais sinalizadores `--mirror=<label>` quando Torii OpenAPI mudar para um instantâneo congelado).
- `npm run build` - confirma que a cópia do herói `Build on Iroha with confidence` segue aparecendo em `build/index.html`.
- `./docs/portal/scripts/preview_verify.sh --build-dir build` - verifica a manifestação de checksums (agregar `--descriptor`/`--archive` ao teste de artefatos baixados de CI).
- `npm run serve` - lança o auxiliar de visualização com controle de soma de verificação que verifica o manifesto antes de ligar para `docusaurus serve`, para que os revisores não naveguem um instantâneo sem firma (o alias `serve:verified` fica disponível para chamadas explícitas).
- Revise o markdown que tocaste via `npm run start` e o servidor de recarregamento ao vivo.

## 2. Cheques de pull request

- Verifique se o trabalho `docs-portal-build` foi passado para `.github/workflows/check-docs.yml`.
- Confirme que `ci/check_docs_portal.sh` foi executado (os logs de CI mostram o hero smoke check).
- Certifique-se de que o fluxo de trabalho de visualização suba uma manifestação (`build/checksums.sha256`) e que o script de verificação de visualização seja encerrado (os logs exibem a saída de `scripts/preview_verify.sh`).
- Agregar o URL da visualização publicada no ambiente do GitHub Pages à descrição do PR.

## 3. Aprovação por seção

| Seção | Proprietário | Lista de verificação |
|--------|-------|-----------|
| Página inicial | DevRel | O herói renderiza, as tarjetas de início rápido enlaçam em rotas válidas, os botões CTA são resolvidos. |
| Norito | Norito WG | As guias de visão geral e introdução referenciam os sinalizadores mais recentes da CLI e a documentação do esquema Norito. |
| SoraFS | Equipe de armazenamento | O início rápido é executado até o final, os campos do relatório do manifesto estão documentados, as instruções de simulação de busca verificadas. |
| Guias SDK | Líderes SDK | As guias de Rust/Python/JS compilam os exemplos atuais e os colocam em repositórios vivos. |
| Referência | Documentos/DevRel | O índice lista as especificações mais recentes, a referência do codec Norito coincide com `norito.md`. |
| Artefato de visualização | Documentos/DevRel | O artefato `docs-portal-preview` está adjunto ao PR, as verificações de fumaça passam, o link é comparado com revisores. |
| Segurança e experimente sandbox | Documentos/DevRel Segurança | Login de código de dispositivo OAuth configurado (`DOCS_OAUTH_*`), checklist `security-hardening.md` executado, CSP/Trusted Types encabezados selecionados via `npm run build` ou `npm run probe:portal`. |

Marque cada fila como parte de sua revisão de PR, ou anota tarefas pendentes para que o acompanhamento de estado siga sendo preciso.

## 4. Notas de lançamento- Inclua `https://docs.iroha.tech/` (ou a URL do ambiente proveniente do trabalho de despliegue) nas notas de lançamento e atualizações de estado.
- Destaca qualquer seção nova ou modificada para que os equipamentos a jusante, onde voltarem a executar suas próprias experiências de humor.