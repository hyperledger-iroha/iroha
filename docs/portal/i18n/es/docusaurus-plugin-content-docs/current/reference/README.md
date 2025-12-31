<!-- Auto-generated stub for Spanish (es) translation. Replace this content with the full translation. -->

---
lang: es
direction: ltr
source: docs/portal/docs/reference/README.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Índice de referencia
slug: /reference
---

Esta sección reúne el material de “léelo como especificación” para Iroha. Estas páginas se mantienen estables incluso cuando evolucionan las guías y los tutoriales.

## Disponible hoy

- **Resumen del codec Norito** - `reference/norito-codec.md` enlaza directamente a la especificación autoritativa `norito.md` mientras se completa la tabla del portal.
- **Torii OpenAPI** - `/reference/torii-openapi` renderiza la especificación REST más reciente de Torii usando Redoc. Regenera la spec con `npm run sync-openapi -- --version=current --latest` (agrega `--mirror=<label>` para copiar el snapshot en versiones históricas adicionales).
- **Tablas de configuración** - El catálogo completo de parámetros se mantiene en `docs/source/references/configuration.md`. Hasta que el portal publique una auto-importación, consulta ese archivo Markdown para los valores por defecto exactos y las anulaciones de entorno.
- **Versionado de docs** - El desplegable de versión en la barra de navegación expone snapshots congelados creados con `npm run docs:version -- <label>`, lo que facilita comparar la guía entre releases.
