---
lang: es
direction: ltr
source: docs/portal/docs/reference/README.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: Índice de referencia
babosa: /referencia
---

Esta sección reagrupa el material en letras como una especificación para Iroha. Estas páginas descansan estables meme lorsque les guías y tutoriales evolucionados.

## Disponible aujourd'hui

- **Apercu du codec Norito** - `reference/norito-codec.md` enviado directamente a la especificación autorizada `norito.md` colgante que la tabla del portail está en curso de remplissage.
- **Torii OpenAPI** - `/reference/torii-openapi` muestra la última especificación REST de Torii con Redoc. Regenere la especificación a través de `npm run sync-openapi -- --version=current --latest` (conecte `--mirror=<label>` para copiar la instantánea en las versiones históricas complementarias).
- **Tablas de configuración** - El catálogo completo de parámetros se encuentra en `docs/source/references/configuration.md`. Por lo tanto, el portal no propone la importación automática, consulte este archivo Markdown para obtener los valores exactos predeterminados y los recargos ambientales.
- **Versión de documentos** - El menú de versión en la barra de navegación expone las instantáneas de las figuras creadas con `npm run docs:version -- <label>`, lo que facilita la comparación de recomendaciones entre versiones.