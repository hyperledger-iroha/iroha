---
lang: pt
direction: ltr
source: docs/portal/docs/reference/README.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: Índice de referência
slug: /referência
---

Esta seção adicionou o material "leia como especificação" para Iroha. Essas páginas permanecem estáveis ​​mesmo quando guias e tutoriais evoluem.

##Disponivel hoje

- **Visão geral do codec Norito** - `reference/norito-codec.md` aponta diretamente para a especificação autoritativa `norito.md` enquanto a tabela do portal está sendo preenchida.
- **Torii OpenAPI** - `/reference/torii-openapi` renderiza uma especificação REST mais recente de Torii usando Redoc. Regenere uma especificação com `npm run sync-openapi -- --version=current --latest` (adicione `--mirror=<label>` para copiar o snapshot para versos históricos adicionais).
- **Tabelas de configuração** - O catálogo completo de parâmetros fica em `docs/source/references/configuration.md`. Até o portal oferecer auto-importação, consulte esse arquivo Markdown para padrões exatos e substituições de ambiente.
- **Versionamento de docs** - O menu suspenso de versão na barra de navegação expoe snapshots congelados criados com `npm run docs:version -- <label>`, facilitando a comparação de orientações entre lançamentos.