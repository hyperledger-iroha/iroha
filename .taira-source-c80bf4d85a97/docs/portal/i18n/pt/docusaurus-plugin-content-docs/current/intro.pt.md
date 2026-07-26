---
lang: pt
direction: ltr
source: docs/portal/docs/intro.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Bem vindo ao Portal de Desenvolvedores SORA Nexus

O portal de desenvolvedores SORA Nexus reúne documentação interativa, tutoriais de SDK e referências de API para operadores Nexus e colaboradores Hyperledger Iroha. Ele complementa o site principal de documentos ao expor guias práticos e especificações geradas diretamente deste repositório. A landing page agora traz pontos de entrada temáticos de Norito/SoraFS, snapshots OpenAPI conhecidos e uma referência dedicada ao Norito Streaming para que contribuidores encontrem o contrato do control-plane de streaming sem vasculhar a especificação raiz.

## O que você pode fazer aqui

- **Aprender Norito** - comece pela visão geral e início rápido para entender o modelo de serialização e as ferramentas de bytecode.
- **Inicializar SDKs** - siga os quickstarts de JavaScript e Rust hoje; guias de Python, Swift e Android serão adicionados conforme as receitas antes migradas.
- **Explorar referências de API** - a página OpenAPI do Torii renderiza a especificação REST mais recente, e as tabelas de configuração apontam para as fontes canônicas em Markdown.
- **Preparar implantações** - runbooks operacionais (telemetria, liquidação, sobreposições Nexus) estão sendo portados de `docs/source/` e chegamao a este site conforme a migração avançada.

## Status atual

- OK Landing Docusaurus v3 tematizada com tipografia renovada, hero/cards guiados por gradiente e azulejos de recursos que incluem o resumo de Norito Streaming.
- OK Plugin OpenAPI do Torii ligado a `npm run sync-openapi`, com verificações de snapshots assinados e guardas CSP aplicados por `buildSecurityHeaders`.
- OK Preview e sonda de cobertura rodam no CI (`docs-portal-preview.yml` + `scripts/portal-probe.mjs`), agora gateando o documento de streaming, os quickstarts de SoraFS e as checklists de referência antes da publicação de artes.
- OK Quickstarts de Norito, SoraFS e SDKs mais seções de referência estão na barra lateral; novas importações de `docs/source/` (streaming, orquestração, runbooks) chegam aqui conforme são escritas.

## Como participar

- Veja `docs/portal/README.md` para comandos de desenvolvimento local (`npm install`, `npm run start`, `npm run build`).
- As tarefas de migração de conteúdo são acompanhadas junto aos itens do roadmap `DOCS-*`. Contribuições são bem-vindas - porte seco de `docs/source/` e adiciona a página na barra lateral.
- Se você adicionar um artefacto gerado (especificações, tabelas de configuração), documente o comando de build para que futuros contribuidores possam atualizá-lo facilmente.