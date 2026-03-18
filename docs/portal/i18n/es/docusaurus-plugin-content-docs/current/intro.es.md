---
lang: es
direction: ltr
source: docs/portal/docs/intro.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Bienvenido al portal de desarrolladores de SORA Nexus

El portal de desarrolladores de SORA Nexus agrupa documentación interactiva, tutoriales de SDK y referencias de API para operadores de Nexus y contribuyentes de Hyperledger Iroha. Complementa el sitio principal de documentos al mostrar guías prácticas y especificaciones generadas directamente desde este repositorio. La pagina de inicio ahora incluye puntos de entrada tematicos de Norito/SoraFS, snapshots OpenAPI firmados y una referencia dedicada de Norito Streaming para que los contribuyentes encuentren el contrato del plano de control de streaming sin tener que buscar en la especificación raiz.

## Lo que puedes hacer aquí- **Aprender Norito** - comienza con la descripción general y la guía de inicio rápido para entender el modelo de serialización y las herramientas de bytecode.
- **Poner en marcha los SDK** - sigue los inicios rápidos de JavaScript y Rust hoy; Las guías de Python, Swift y Android se sumarán conforme se migran las recetas.
- **Explorar referencias de API** - la página OpenAPI de Torii renderiza la especificación REST más reciente, y las tablas de configuración enlazan a las fuentes canónicas en Markdown.
- **Preparar despliegues** - los runbooks operativos (telemetria, asentamiento, Nexus overlays) se están migrando desde `docs/source/` y llegaran a este sitio conforme avance la migración.

## Estado actual- Landing de Docusaurus v3 con tema, tipografía renovada, hero/tarjetas con gradientes y mosaicos de recursos que incluyen el resumen de Norito Streaming.
- Plugin OpenAPI de Torii conectado a `npm run sync-openapi`, con verificaciones de instantáneas firmadas y guardas CSP aplicadas por `buildSecurityHeaders`.
- La cobertura de vista previa y sonda corre en CI (`docs-portal-preview.yml` + `scripts/portal-probe.mjs`), y ahora bloquea el doc de streaming, los inicios rápidos de SoraFS y las listas de verificación de referencia antes de publicar artefactos.
- Los inicios rápidos de Norito, SoraFS y SDK, junto con secciones de referencia, ya están en la barra lateral; nuevas importaciones desde `docs/source/` (streaming, Orchestration, runbooks) llegan aquí conforme se redactan.

## Como participar

- Consulta `docs/portal/README.md` para comandos de desarrollo local (`npm install`, `npm run start`, `npm run build`).
- Las tareas de migración de contenido se rastrean junto a los items `DOCS-*` del roadmap. Se agradecen contribuciones: porta secciones desde `docs/source/` y agrega la página a la barra lateral.
- Si agregas un artefacto generado (especificaciones, tablas de configuración), documenta el comando de construcción para que futuros contribuyentes puedan refrescarlo fácilmente.