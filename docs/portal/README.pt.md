---
lang: pt
direction: ltr
source: docs/portal/README.md
status: complete
translator: manual
source_hash: 4b0d6c295c7188355e2c03d7c8240271da147095ff557fae2152f42e27bd17fa
source_last_modified: "2025-11-14T04:43:03.939564+00:00"
translation_last_reviewed: 2025-11-14
---

<!-- Tradução para português de docs/portal/README.md (SORA Nexus Developer Portal) -->

# Portal de Desenvolvedores SORA Nexus

Este diretório hospeda o workspace Docusaurus para o portal interativo de
desenvolvedores. O portal agrega guias Norito, quickstarts de SDK e a referência OpenAPI
gerada por `cargo xtask openapi`, aplicando a identidade visual SORA Nexus usada em todo
o programa de documentação.

## Pré‑requisitos

- Node.js 18.18 ou mais recente (baseline Docusaurus v3).
- Yarn 1.x ou npm ≥ 9 para gerenciamento de pacotes.
- Toolchain Rust (usado pelo script de sincronização do OpenAPI).

## Bootstrap

```bash
cd docs/portal
npm install    # ou yarn install
```

## Scripts disponíveis

| Comando | Descrição |
|---------|-----------|
| `npm run start` / `yarn start` | Inicia um servidor de desenvolvimento local com live reload (padrão `http://localhost:3000`). |
| `npm run build` / `yarn build` | Gera um build de produção em `build/`. |
| `npm run serve` / `yarn serve` | Serve o build mais recente localmente (útil para smoke tests). |
| `npm run docs:version -- <label>` | Tira um snapshot da documentação atual em `versioned_docs/version-<label>` (wrapper em volta de `docusaurus docs:version`). |
| `npm run sync-openapi` / `yarn sync-openapi` | Regenera `static/openapi/torii.json` via `cargo xtask openapi` (use `--mirror=<label>` para copiar a spec para snapshots adicionais). |
| `npm run tryit-proxy` | Sobe o proxy de staging que alimenta o console “Try it” (veja a configuração abaixo). |
| `npm run probe:tryit-proxy` | Envia um probe `/healthz` + requisição de exemplo contra o proxy (helper para CI/monitoramento). |
| `npm run manage:tryit-proxy -- <update|rollback>` | Atualiza ou restaura o target `.env` do proxy com suporte a backup. |
| `npm run sync-i18n` | Garante a existência de stubs de tradução para japonês, hebraico, espanhol, português, francês, russo, árabe e urdu sob `i18n/`. |
| `npm run sync-norito-snippets` | Regenera docs de exemplos Kotodama curados + snippets para download (também invocado automaticamente pelo plugin do dev server). |
| `npm run test:tryit-proxy` | Executa os testes unitários do proxy via test runner do Node (`node --test`). |

O script de sincronização OpenAPI exige que `cargo xtask openapi` esteja disponível a
partir da raiz do repositório; ele emite um JSON determinístico em `static/openapi/` e
agora espera que o router Torii exponha uma spec “ao vivo” (use
`cargo xtask openapi --allow-stub` apenas para saídas temporárias de emergência).

## Versionamento de docs e snapshots OpenAPI

- **Cortando uma versão de docs:** execute `npm run docs:version -- 2025-q3` (ou qualquer
  label acordada). Faça commit de `versioned_docs/version-<label>`,
  `versioned_sidebars` e `versions.json`. O dropdown de versões na navbar exibirá
  automaticamente o novo snapshot.
- **Sincronizando artefatos OpenAPI:** depois de cortar uma versão, atualize a spec
  canônica e o manifest com `cargo xtask openapi --sign <caminho-para-chave-ed25519>`, e
  então capture um snapshot correspondente com
  `npm run sync-openapi -- --version=2025-q3 --mirror=current --latest`. O script grava
  `static/openapi/versions/2025-q3/torii.json`, espelha a spec em
  `versions/current/torii.json`, atualiza `versions.json`, atualiza
  `/openapi/torii.json` e clona o `manifest.json` assinado em cada diretório de versão
  para que specs históricas compartilhem o mesmo metadata de proveniência. Forneça quantos
  `--mirror=<label>` forem necessários para copiar a spec recém‑gerada em snapshots
  históricos adicionais.
- **Expectativas de CI:** commits que toquem docs devem incluir o bump de versão (quando
  aplicável) e snapshots OpenAPI atualizados para que os painéis Swagger, RapiDoc e Redoc
  possam alternar entre specs históricas sem erros de fetch.
- **Enforcement do manifest:** o script `sync-openapi` só copia manifests quando o
  `manifest.json` em disco coincide com a spec recém‑gerada. Se a cópia for pulada,
  execute novamente `cargo xtask openapi --sign <chave>` para atualizar o manifest
  canônico e rode o sync novamente para que os snapshots versionados passem a carregar o
  metadata assinado. O script `ci/check_openapi_spec.sh` reproduz o gerador e valida o
  manifest antes de permitir merges.

## Estrutura

```text
docs/portal/
├── docs/                 # Conteúdo Markdown/MDX do portal
├── i18n/                 # Overrides de idioma (ja/he) gerados pelo sync-i18n
├── src/                  # Páginas/componentes React (scaffolding)
├── static/               # Assets estáticos servidos como estão (inclui JSON OpenAPI)
├── scripts/              # Scripts de ajuda (sincronização OpenAPI)
├── docusaurus.config.js  # Configuração principal do site
└── sidebars.js           # Modelo de navegação/sidebars
```

### Configuração do proxy Try It

O sandbox “Try it” encaminha requisições através de `scripts/tryit-proxy.mjs`. Configure
o proxy com variáveis de ambiente antes de iniciá‑lo:

```bash
export TRYIT_PROXY_TARGET="https://torii.staging.sora"
export TRYIT_PROXY_ALLOWED_ORIGINS="http://localhost:3000"
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
```

Em staging/produção, defina essas variáveis no sistema de configuração da sua plataforma
(por exemplo, secretos do GitHub Actions, variáveis de ambiente do orquestrador de
containers, etc.).

### URLs de preview e notas de release

- Preview público em beta: `https://docs.iroha.tech/`
- O GitHub também expõe o build no ambiente **github-pages** para cada deployment.
- Pull requests que alteram conteúdo do portal incluem artefatos de Actions
  (`docs-portal-preview`, `docs-portal-preview-metadata`) contendo o site compilado,
  manifest de checksums, arquivo comprimido e descriptor; revisores podem baixar e abrir
  `index.html` localmente e verificar checksums antes de compartilhar previews. O
  workflow adiciona um comentário de resumo (hashes do manifest/arquivo e status SoraFS)
  em cada PR, fornecendo um sinal rápido de que a verificação passou.
- Use `./docs/portal/scripts/preview_verify.sh --build-dir <build extraído> --descriptor <descriptor> --archive <arquivo>` depois de baixar um bundle de preview para confirmar que os artefatos correspondem ao que o CI produziu antes de compartilhar o link externamente.
- Ao preparar release notes ou atualizações de status, faça referência à URL de preview
  para que revisores externos possam navegar no snapshot mais recente do portal sem
  precisar clonar o repositório.
- Coordene ondas de preview usando
  `docs/portal/docs/devportal/preview-invite-flow.md` em conjunto com
  `docs/portal/docs/devportal/reviewer-onboarding.md` para garantir que cada convite،
  export de telemetria e etapa de offboarding reutilize a mesma trilha de evidência.

