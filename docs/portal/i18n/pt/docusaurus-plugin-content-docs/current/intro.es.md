---
lang: pt
direction: ltr
source: docs/portal/docs/intro.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Bem-vindo ao portal de desenvolvimento de SORA Nexus

O portal de desenvolvedores de SORA Nexus agrupa documentação interativa, tutoriais de SDK e referências de API para operadores de Nexus e colaboradores de Hyperledger Iroha. Complementa o site principal de documentos para mostrar guias práticas e especificações geradas diretamente deste repositório. A página de início agora inclui pontos de entrada temáticos de Norito/SoraFS, snapshots OpenAPI firmados e uma referência exclusiva de Norito Streaming para que os contribuintes encontrem o contrato do plano de controle de streaming sem ter que buscar na especificação raiz.

## O que você pode fazer aqui

- **Aprender Norito** - começa com a descrição geral e o guia de início rápido para entender o modelo de serialização e as ferramentas de bytecode.
- **Ponha em marcha os SDKs** - siga os inícios rápidos de JavaScript e Rust hoje; as guias de Python, Swift e Android são resumidas conforme suas receitas.
- **Explorar referências de API** - a página OpenAPI de Torii renderiza a especificação REST mais recente, e as tabelas de configuração são inseridas nas fontes canônicas em Markdown.
- **Preparar despliegues** - os runbooks operacionais (telemetria, liquidação, sobreposições Nexus) estão migrando de `docs/source/` e são transferidos para este local conforme avança a migração.

## Estado atual

- Landing de Docusaurus v3 com tema, tipografia renovada, heróis/tarjetas com gradientes e blocos de recursos que incluem o resumo de Norito Streaming.
- Plugin OpenAPI de Torii conectado a `npm run sync-openapi`, com verificações de snapshots firmados e guardas CSP aplicados por `buildSecurityHeaders`.
- A cobertura de visualização e teste corre em CI (`docs-portal-preview.yml` + `scripts/portal-probe.mjs`), e agora bloqueie o documento de streaming, os inícios rápidos de SoraFS e as listas de verificação de referência antes de publicar artefatos.
- Os quickstarts de Norito, SoraFS e SDKs, juntamente com seções de referência, estão na barra lateral; novas importações de `docs/source/` (streaming, orquestração, runbooks) foram transferidas aqui conforme redigido.

## Como participar

- Consulte `docs/portal/README.md` para comandos de desenvolvimento local (`npm install`, `npm run start`, `npm run build`).
- As tarefas de migração de conteúdo são rastreadas junto com os itens `DOCS-*` do roteiro. Agradecemos contribuições: porta seções de `docs/source/` e adição da página à barra lateral.
- Se você adicionar um artefato gerado (especificações, tabelas de configuração), documente o comando de build para que futuros contribuidores possam atualizá-lo facilmente.