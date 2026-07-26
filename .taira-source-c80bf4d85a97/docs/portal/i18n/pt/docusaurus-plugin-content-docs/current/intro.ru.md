---
lang: pt
direction: ltr
source: docs/portal/docs/intro.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Добро пожаловать на портал разработчиков SORA Nexus

O portal SORA Nexus fornece documentos interativos, materiais SDK e API de expansão para o operador Nexus e o contador Hyperledger Iroha. Ao obter toda a documentação, você precisará de um plano de prática de acordo com os padrões e especificações, генерируемые напрямую из этого репозитория. O modelo de empréstimo do tema de transferência é Norito/SoraFS, usando o ícone OpenAPI e отдельный справочник Norito Streaming, чтобы контрибьюторы могли найти контракт контрольной плоскости стриминга, не пролистывая корневую спецификацию.

## O que posso fazer com você

- **Изучить Norito** - начните собзора e quickstart, чтобы понять модель сериализации и инструменты bytecode.
- **Iniciar SDK** - Guia de início rápido para JavaScript e Rust está disponível; As soluções para Python, Swift e Android fornecem apenas muitos protocolos de migração.
- **Просмотреть справочники API** - página Torii OpenAPI отображает актуальную спецификацию REST, а As tabelas de configuração são configuradas para a história do Markdown canônico.
- **Подготовить развертывания** - эксплуатационные runbook-и (telemetria, liquidação, sobreposições Nexus) переносятся из `docs/source/` и появятся здесь по мере миграции.

## Status da tecnologia

- ✅ Tema de empréstimo Docusaurus v3 com tipo padrão, hero/cards на градиентах и тайлами ресурсов, включая сводку Norito Streaming.
- ✅ Плагин Torii OpenAPI подключен к `npm run sync-openapi`, с проверками подписанных снимков e CSP-guard защитами, exemplo `buildSecurityHeaders`.
- ✅ Pré-visualização e cobertura de sonda são obtidas no CI (`docs-portal-preview.yml` + `scripts/portal-probe.mjs`), arquivo on-line para obter documentos para streaming, inícios rápidos SoraFS e listas de verificação de referência acima artefactos públicos.
- ✅ Guias de início rápido Norito, SoraFS e SDK mais seções de segurança que você encontra no painel da caixa; novas importações de `docs/source/` (streaming, orquestração, runbooks) podem ser feitas por mais tempo.

## Как принять участие

- Sim. `docs/portal/README.md` para comando de controle local (`npm install`, `npm run start`, `npm run build`).
- O conteúdo da migração está disponível no roteiro `DOCS-*`. Вклад приветствуется - verifique a seção de `docs/source/` e instale a página no painel traseiro.
- Você vai precisar de um artefato genérico (especificações, tabelas de configuração), um comando de segurança, um botão участники могли легко его обновить.