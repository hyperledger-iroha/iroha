---
lang: es
direction: ltr
source: docs/portal/docs/reference/README.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: Índice de referencia
babosa: /referencia
---

Esta seco agrega o material "leia como especificacao" para Iroha. Esas páginas permanecen en este lugar cuando las guías y los tutoriales evolucionan.

## Disponivel hoy

- **Visao general del codec Norito** - `reference/norito-codec.md` se conecta directamente a la especificacao autoritativa `norito.md` mientras a la tabla del portal esta sendo preenchida.
- **Torii OpenAPI** - `/reference/torii-openapi` renderiza a especificacao REST más reciente de Torii usando Redoc. Regenere una especificación con `npm run sync-openapi -- --version=current --latest` (adicione `--mirror=<label>` para copiar o instantáneas para versiones históricas adicionales).
- **Tablas de configuración** - El catálogo completo de parámetros fica en `docs/source/references/configuration.md`. En el portal que ofrece importación automática, consulte este archivo Markdown para valores predeterminados exactos y anulaciones de ambiente.
- **Versión de documentos** - El menú desplegable de versao na navbar expoe instantáneas congeladas criados con `npm run docs:version -- <label>`, facilitando la comparación de orientaciones entre versiones.