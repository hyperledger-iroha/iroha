---
lang: pt
direction: ltr
source: docs/portal/docs/intro.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Bem-vindo ao portal de desenvolvimento SORA Nexus

O portal de desenvolvimento SORA Nexus agrupa a documentação interativa, os tutoriais SDK e as referências da API para os operadores Nexus e os colaboradores Hyperledger Iroha. Preencha o site principal dos documentos antes dos guias práticos e das especificações geradas diretamente deste depósito. A página inicial propõe os pontos de entrada temáticos Norito/SoraFS, os instantâneos OpenAPI assinados e uma referência dedicada a Norito Streaming para que os colaboradores possam encontrar o contrato do plano de controle do streaming sem falta de especificação racine.

## O que você pode fazer aqui

- **Aprenda Norito** - comece pela abertura e pelo início rápido para compreender o modelo de serialização e as ferramentas de bytecode.
- **Desmarque os SDKs** - siga os inícios rápidos do JavaScript e Rust do início ; os guias Python, Swift e Android são reunidos em uma medida de migração de receitas.
- **Parcourir as referências API** - a página OpenAPI de Torii produz a especificação REST anterior, e os quadros de configuração enviados para fontes Markdown canônicas.
- **Preparar as implantações** - As operações de runbooks (telemetria, liquidação, sobreposições Nexus) estão no curso de portagem a partir de `docs/source/` e chegam aqui na medida em que a migração avança.

## Estatuto atual

- Tema Landing Docusaurus v3 com tipografia rafraichie, guias de heróis / cartas por degradação e tutoriais de recursos, incluindo o currículo Norito Streaming.
- Plugin OpenAPI Torii cabo em `npm run sync-openapi`, com verificações de instantâneos, sinais e proteções CSP aplicadas por `buildSecurityHeaders`.
- A visualização da cobertura e a sonda são executadas em CI (`docs-portal-preview.yml` + `scripts/portal-probe.mjs`), e o bloco desorma o streaming de documentos, os inícios rápidos SoraFS e as listas de verificação de referência antes da publicação dos artefatos.
- Os inícios rápidos Norito, SoraFS e SDK também que as seções de referência estão na barra lateral; as novas importações de `docs/source/` (streaming, orquestração, runbooks) chegam aqui ao arquivo de sua redação.

## Participante

- Veja `docs/portal/README.md` para comandos de desenvolvimento local (`npm install`, `npm run start`, `npm run build`).
- As tabelas de migração de conteúdo são sucessivas com os itens do roteiro `DOCS-*`. As contribuições são bem-vindas: porta as seções a partir de `docs/source/` e adiciona a página à barra lateral.
- Se você adicionar um gênero de artefato (especificações, tabelas de configuração), documente o comando de construção para que os futuros contribuidores possam regenerar facilmente.