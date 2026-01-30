---
lang: es
direction: ltr
source: docs/portal/docs/intro.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# Bienvenido al portal de desarrolladores de SORA Nexus

El portal de desarrolladores de SORA Nexus agrupa documentacion interactiva, tutoriales de SDK y referencias de API para operadores de Nexus y contribuyentes de Hyperledger Iroha. Complementa el sitio principal de docs al mostrar guias practicas y especificaciones generadas directamente desde este repositorio. La pagina de inicio ahora incluye puntos de entrada tematicos de Norito/SoraFS, snapshots OpenAPI firmados y una referencia dedicada de Norito Streaming para que los contribuyentes encuentren el contrato del plano de control de streaming sin tener que buscar en la especificacion raiz.

## Lo que puedes hacer aqui

- **Aprender Norito** - comienza con la descripcion general y la guia de inicio rapido para entender el modelo de serializacion y las herramientas de bytecode.
- **Poner en marcha los SDKs** - sigue los quickstarts de JavaScript y Rust hoy; las guias de Python, Swift y Android se sumaran conforme se migren las recetas.
- **Explorar referencias de API** - la pagina OpenAPI de Torii renderiza la especificacion REST mas reciente, y las tablas de configuracion enlazan a las fuentes canonicas en Markdown.
- **Preparar despliegues** - los runbooks operativos (telemetria, settlement, Nexus overlays) se estan migrando desde `docs/source/` y llegaran a este sitio conforme avance la migracion.

## Estado actual

-  Landing de Docusaurus v3 con tema, tipografia renovada, hero/tarjetas con gradientes y tiles de recursos que incluyen el resumen de Norito Streaming.
-  Plugin OpenAPI de Torii conectado a `npm run sync-openapi`, con verificaciones de snapshots firmados y guardas CSP aplicadas por `buildSecurityHeaders`.
-  La cobertura de preview y probe corre en CI (`docs-portal-preview.yml` + `scripts/portal-probe.mjs`), y ahora bloquea el doc de streaming, los quickstarts de SoraFS y las checklists de referencia antes de publicar artefactos.
-  Los quickstarts de Norito, SoraFS y SDKs, junto con secciones de referencia, ya estan en la barra lateral; nuevas importaciones desde `docs/source/` (streaming, orchestration, runbooks) llegan aqui conforme se redactan.

## Como participar

- Consulta `docs/portal/README.md` para comandos de desarrollo local (`npm install`, `npm run start`, `npm run build`).
- Las tareas de migracion de contenido se rastrean junto a los items `DOCS-*` del roadmap. Se agradecen contribuciones: porta secciones desde `docs/source/` y agrega la pagina a la barra lateral.
- Si agregas un artefacto generado (specs, tablas de configuracion), documenta el comando de build para que futuros contribuyentes puedan refrescarlo facilmente.
