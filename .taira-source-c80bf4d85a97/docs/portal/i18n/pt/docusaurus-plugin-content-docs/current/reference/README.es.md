---
lang: pt
direction: ltr
source: docs/portal/docs/reference/README.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: Índice de referência
slug: /referência
---

Esta seção reúne o material de "lelo conforme especificação" para Iroha. Estas páginas são mantidas estáveis, mesmo quando os guias e os tutoriais são desenvolvidos.

## Disponível, cara

- **Resumo do codec Norito** - `reference/norito-codec.md` colocado diretamente na especificação autorizada `norito.md` enquanto a tabela do portal é completada.
- **Torii OpenAPI** - `/reference/torii-openapi` renderiza a especificação REST mais recente de Torii usando Redoc. Regenera a especificação com `npm run sync-openapi -- --version=current --latest` (agrega `--mirror=<label>` para copiar o snapshot em versões históricas adicionais).
- **Tablas de configuração** - O catálogo completo de parâmetros é mantido em `docs/source/references/configuration.md`. Até que o portal publique uma importação automática, consulte este arquivo Markdown para os valores por defeito exato e as anulações de ambiente.
- **Versionado de docs** - A versão desinstalada na barra de navegação expõe snapshots congelados criados com `npm run docs:version -- <label>`, para facilitar a comparação do guia entre as versões.