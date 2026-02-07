---
lang: pt
direction: ltr
source: docs/portal/docs/intro.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# SORA Nexus Portal do desenvolvedor میں خوش آمدید

SORA Nexus portal do desenvolvedor Nexus operadores اور Hyperledger Iroha contribuidores کے لئے documentação interativa, tutoriais SDK, e referências API کو یکجا کرتا ہے۔ یہ site de documentos principais کو اس repositório سے براہ راست especificações geradas اور guias práticos سامنے لا کر مکمل کرتا ہے۔ página de destino اب pontos de entrada temáticos Norito/SoraFS, instantâneos OpenAPI assinados, اور ایک Norito dedicado Referência de streaming فراہم کرتا ہے تاکہ especificação raiz dos contribuidores کھنگالے بغیر contrato de plano de controle de streaming تک پہنچ سکیں۔

## آپ یہاں کیا کر سکتے ہیں

- **Norito سیکھیں** - visão geral اور quickstart سے آغاز کریں تاکہ modelo de serialização اور ferramentas de bytecode سمجھ سکیں۔
- **SDKs bootstrap کریں** - JavaScript اور Rust کے quickstarts آج فالو کریں؛ Python, Swift, Android orienta receitas migratórias ہونے کے ساتھ شامل ہوں گے۔
- **Referências de API دیکھیں** - Torii OpenAPI página تازہ ترین Especificação REST renderização کرتا ہے، اور tabelas de configuração fontes canônicas de Markdown کی طرف link کرتے ہیں۔
- **Implantações تیار کریں** - runbooks operacionais (telemetria, liquidação, sobreposições Nexus) `docs/source/` سے porta ہو رہے ہیں اور migração کے ساتھ اس سائٹ پر آئیں گے۔

## موجودہ حالت

- ✅ pouso temático Docusaurus v3 جس میں tipografia atualizada, heróis / cartões baseados em gradiente, blocos de recursos شامل ہیں جو Norito Resumo de streaming رکھتے ہیں۔
- ✅ Torii OpenAPI plugin کو `npm run sync-openapi` سے com fio کیا گیا ہے، verificações de instantâneos assinados اور protetores CSP `buildSecurityHeaders` کے ذریعے نافذ ہیں۔
- ✅ Visualizar CI de cobertura de sonda (`docs-portal-preview.yml` + `scripts/portal-probe.mjs`) میں چلتی ہے, اور اب streaming doc, SoraFS quickstarts, اور listas de verificação de referência کو publicação de artefatos ہونے سے پہلے portão کرتی ہے۔
- ✅ Norito, SoraFS, e os guias de início rápido do SDK کے ساتھ barra lateral de seções de referência میں live ہیں؛ `docs/source/` سے نئی importações (streaming, orquestração, runbooks)

## شمولیت کیسے کریں

- لوکل comandos de desenvolvimento کے لئے `docs/portal/README.md` دیکھیں (`npm install`, `npm run start`, `npm run build`)۔
- Tarefas de migração de conteúdo `DOCS-*` itens de roteiro کے ساتھ rastrear کی جاتی ہیں۔ Contribuições خوش آمدید ہیں - `docs/source/` سے seções porta کریں اور página کو barra lateral میں شامل کریں۔
- اگر آپ کوئی artefato gerado (especificações, tabelas de configuração) شامل کریں تو comando de construção دستاویز کریں تاکہ آئندہ contribuidores اسے آسانی سے atualizar کر سکیں۔