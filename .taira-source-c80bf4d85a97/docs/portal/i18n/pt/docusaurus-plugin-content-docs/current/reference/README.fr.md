---
lang: pt
direction: ltr
source: docs/portal/docs/reference/README.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: Índice de referência
slug: /referência
---

Esta seção reagrupou o material para lire com uma especificação para Iroha. Estas páginas restent stables meme quando os guias e tutoriais evoluem.

## Disponible aujourd'hui

- **Apercu du codec Norito** - `reference/norito-codec.md` enviado diretamente para a especificação oficial `norito.md`, desde que a tabela do portal esteja em processo de remplissage.
- **Torii OpenAPI** - `/reference/torii-openapi` produz a especificação REST anterior de Torii com Redoc. Regenere a especificação via `npm run sync-openapi -- --version=current --latest` (adicione `--mirror=<label>` para copiar o instantâneo nas versões históricas suplementares).
- **Tabelas de configuração** - O catálogo completo de parâmetros é encontrado em `docs/source/references/configuration.md`. Se o portal não oferecer importação automática, consulte este arquivo Markdown para os valores padrão exatos e as sobretaxas ambientais.
- **Versão dos documentos** - O menu de versão na barra de navegação expõe os instantâneos dos valores com `npm run docs:version -- <label>`, o que facilita a comparação das recomendações entre os lançamentos.