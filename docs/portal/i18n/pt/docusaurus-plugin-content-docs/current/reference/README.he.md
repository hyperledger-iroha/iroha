---
lang: he
direction: rtl
source: docs/portal/i18n/pt/docusaurus-plugin-content-docs/current/reference/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 797f63cd882e3e413b5f84492cbce9abdae64a448e3f04bcce3a1ac4f791aa06
source_last_modified: "2025-11-14T04:43:20.931549+00:00"
translation_last_reviewed: 2026-01-30
---

Esta secao agrega o material "leia como especificacao" para Iroha. Essas paginas permanecem estaveis mesmo quando guias e tutoriais evoluem.

## Disponivel hoje

- **Visao geral do codec Norito** - `reference/norito-codec.md` aponta diretamente para a especificacao autoritativa `norito.md` enquanto a tabela do portal esta sendo preenchida.
- **Torii OpenAPI** - `/reference/torii-openapi` renderiza a especificacao REST mais recente de Torii usando Redoc. Regenere a spec com `npm run sync-openapi -- --version=current --latest` (adicione `--mirror=<label>` para copiar o snapshot para versoes historicas adicionais).
- **Tabelas de configuracao** - O catalogo completo de parametros fica em `docs/source/references/configuration.md`. Ate o portal oferecer auto-import, consulte esse arquivo Markdown para defaults exatos e overrides de ambiente.
- **Versionamento de docs** - O dropdown de versao na navbar expoe snapshots congelados criados com `npm run docs:version -- <label>`, facilitando comparar orientacoes entre releases.
