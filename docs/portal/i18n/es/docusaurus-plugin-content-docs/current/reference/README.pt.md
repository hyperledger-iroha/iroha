---
lang: es
direction: ltr
source: docs/portal/docs/reference/README.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
title: Indice de referencia
slug: /reference
---

Esta secao agrega o material "leia como especificacao" para Iroha. Essas paginas permanecem estaveis mesmo quando guias e tutoriais evoluem.

## Disponivel hoje

- **Visao geral do codec Norito** - `reference/norito-codec.md` aponta diretamente para a especificacao autoritativa `norito.md` enquanto a tabela do portal esta sendo preenchida.
- **Torii OpenAPI** - `/reference/torii-openapi` renderiza a especificacao REST mais recente de Torii usando Redoc. Regenere a spec com `npm run sync-openapi -- --version=current --latest` (adicione `--mirror=<label>` para copiar o snapshot para versoes historicas adicionais).
- **Tabelas de configuracao** - O catalogo completo de parametros fica em `docs/source/references/configuration.md`. Ate o portal oferecer auto-import, consulte esse arquivo Markdown para defaults exatos e overrides de ambiente.
- **Versionamento de docs** - O dropdown de versao na navbar expoe snapshots congelados criados com `npm run docs:version -- <label>`, facilitando comparar orientacoes entre releases.
