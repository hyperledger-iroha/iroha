---
lang: he
direction: rtl
source: docs/portal/i18n/pt/docusaurus-plugin-content-docs/current/intro.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: cb0792481a7865648348a27bc0f21807601c1fd59efaa33c44dcb2fa915ecc98
source_last_modified: "2025-11-14T04:43:20.256306+00:00"
translation_last_reviewed: 2026-01-30
---

# Bem vindo ao Portal de Desenvolvedores SORA Nexus

O portal de desenvolvedores SORA Nexus reune documentacao interativa, tutoriais de SDK e referencias de API para operadores Nexus e contribuidores Hyperledger Iroha. Ele complementa o site principal de docs ao expor guias praticos e specs geradas diretamente deste repositorio. A landing page agora traz pontos de entrada tematicos de Norito/SoraFS, snapshots OpenAPI assinados e uma referencia dedicada ao Norito Streaming para que contribuidores encontrem o contrato do control-plane de streaming sem vasculhar a spec raiz.

## O que voce pode fazer aqui

- **Aprender Norito** - comece pelo overview e quickstart para entender o modelo de serializacao e o tooling de bytecode.
- **Inicializar SDKs** - siga os quickstarts de JavaScript e Rust hoje; guias de Python, Swift e Android serao adicionados conforme as receitas forem migradas.
- **Explorar referencias de API** - a pagina OpenAPI do Torii renderiza a especificacao REST mais recente, e as tabelas de configuracao apontam para as fontes canonicas em Markdown.
- **Preparar deploys** - runbooks operacionais (telemetry, settlement, Nexus overlays) estao sendo portados de `docs/source/` e chegarao a este site conforme a migracao avancar.

## Status atual

- OK Landing Docusaurus v3 tematizada com tipografia renovada, hero/cards guiados por gradiente e tiles de recursos que incluem o resumo de Norito Streaming.
- OK Plugin OpenAPI do Torii ligado a `npm run sync-openapi`, com checks de snapshots assinados e guards CSP aplicados por `buildSecurityHeaders`.
- OK Preview e probe coverage rodam no CI (`docs-portal-preview.yml` + `scripts/portal-probe.mjs`), agora gateando o doc de streaming, os quickstarts de SoraFS e as checklists de referencia antes da publicacao de artefatos.
- OK Quickstarts de Norito, SoraFS e SDKs mais secoes de referencia estao na sidebar; novas importacoes de `docs/source/` (streaming, orchestration, runbooks) chegam aqui conforme sao escritas.

## Como participar

- Veja `docs/portal/README.md` para comandos de desenvolvimento local (`npm install`, `npm run start`, `npm run build`).
- As tarefas de migracao de conteudo sao acompanhadas junto aos itens de roadmap `DOCS-*`. Contribuicoes sao bem-vindas - porte secoes de `docs/source/` e adicione a pagina na sidebar.
- Se voce adicionar um artefato gerado (specs, config tables), documente o comando de build para que futuros contribuidores possam atualiza-lo facilmente.
