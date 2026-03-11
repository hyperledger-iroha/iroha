---
lang: pt
direction: ltr
source: docs/portal/docs/reference/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b3b2becfdbab1446f8f230ace905de306e1e89147f5a5e578d784be97445d74d
source_last_modified: "2025-11-08T06:08:33.073497+00:00"
translation_last_reviewed: 2025-12-30
---

---
title: Indice de referencia
slug: /reference
---

Esta secao agrega o material "leia como especificacao" para Iroha. Essas paginas permanecem estaveis mesmo quando guias e tutoriais evoluem.

## Disponivel hoje

- **Visao geral do codec Norito** - `reference/norito-codec.md` aponta diretamente para a especificacao autoritativa `norito.md` enquanto a tabela do portal esta sendo preenchida.
- **Torii OpenAPI** - `/reference/torii-openapi` renderiza a especificacao REST mais recente de Torii usando Redoc. Regenere a spec com `npm run sync-openapi -- --version=current --latest` (adicione `--mirror=<label>` para copiar o snapshot para versoes historicas adicionais).
- **Torii MCP API** - `/reference/torii-mcp` documents MCP JSON-RPC usage (`initialize`, `tools/list`, `tools/call`) and async job polling for `/v1/mcp`.
- **Tabelas de configuracao** - O catalogo completo de parametros fica em `docs/source/references/configuration.md`. Ate o portal oferecer auto-import, consulte esse arquivo Markdown para defaults exatos e overrides de ambiente.
- **Versionamento de docs** - O dropdown de versao na navbar expoe snapshots congelados criados com `npm run docs:version -- <label>`, facilitando comparar orientacoes entre releases.
