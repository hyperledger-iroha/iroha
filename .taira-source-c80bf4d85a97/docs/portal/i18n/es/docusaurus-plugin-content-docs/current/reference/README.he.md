---
lang: he
direction: rtl
source: docs/portal/i18n/es/docusaurus-plugin-content-docs/current/reference/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: da08a0666d27f68f76f8b0cbf1c866cbcb17880aed7b8f96b90b70af96bd4dd7
source_last_modified: "2026-01-03T18:07:57+00:00"
translation_last_reviewed: 2026-01-30
---

---
lang: es
direction: ltr
source: docs/portal/docs/reference/README.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Indice de referencia
slug: /reference
---

Esta seccion reune el material de "leelo como especificacion" para Iroha. Estas paginas se mantienen estables incluso cuando evolucionan las guias y los tutoriales.

## Disponible hoy

- **Resumen del codec Norito** - `reference/norito-codec.md` enlaza directamente a la especificacion autoritativa `norito.md` mientras se completa la tabla del portal.
- **Torii OpenAPI** - `/reference/torii-openapi` renderiza la especificacion REST mas reciente de Torii usando Redoc. Regenera la spec con `npm run sync-openapi -- --version=current --latest` (agrega `--mirror=<label>` para copiar el snapshot en versiones historicas adicionales).
- **Tablas de configuracion** - El catalogo completo de parametros se mantiene en `docs/source/references/configuration.md`. Hasta que el portal publique una auto-importacion, consulta ese archivo Markdown para los valores por defecto exactos y las anulaciones de entorno.
- **Versionado de docs** - El desplegable de version en la barra de navegacion expone snapshots congelados creados con `npm run docs:version -- <label>`, lo que facilita comparar la guia entre releases.
