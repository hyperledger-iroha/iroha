---
lang: es
direction: ltr
source: docs/portal/docs/intro.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Bienvenue sur le portail desarrollador SORA Nexus

El portal de desarrollo SORA Nexus reúne la documentación interactiva, los tutoriales SDK y las referencias de API para los operadores Nexus y los contribuyentes Hyperledger Iroha. Il complete le site principal de docs en mettant en avant des Guides Pratiques et des especificaciones generées directement depuis ce depot. La página de inicio propone desormar los puntos de entrada temáticos Norito/SoraFS, las instantáneas OpenAPI firmadas y una referencia destinada a Norito Streaming para que los contribuyentes puedan encontrar el contrato del plan de control del streaming sin falta de especificación racine.

## Ce que vous pouvez faire aquí- **Apprendre Norito** - Comience por la apertura y el inicio rápido para comprender el modelo de serialización y las herramientas de código de bytes.
- **Elaborar los SDK** - suivez los inicios rápidos de JavaScript y Rust desde el inicio; Las guías Python, Swift y Android se reúnen a medida que se migran las recetas.
- **Consulte las referencias API** - la página OpenAPI de Torii muestra la última especificación REST y las tablas de configuración que incluyen fuentes auxiliares Markdown canónicos.
- **Preparar las implementaciones** - Los runbooks operativos (telemetría, liquidación, superposiciones Nexus) están en curso de transporte después de `docs/source/` y llegan aquí para medir el avance de la migración.

## Estatuto actual- Landing Docusaurus v3 temático con tipografía rafraichie, guías de héroes/tarjetas par des degrades et tuiles de recursos que incluyen el currículum Norito Streaming.
- Complemento OpenAPI Torii cable en `npm run sync-openapi`, con verificaciones de instantáneas firmadas y protecciones CSP aplicadas por `buildSecurityHeaders`.
- La vista previa de cobertura y la prueba se ejecutan en CI (`docs-portal-preview.yml` + `scripts/portal-probe.mjs`), y bloquean la transmisión de documentos, los inicios rápidos SoraFS y las listas de verificación de referencia antes de la publicación de artefactos.
- Los inicios rápidos Norito, SoraFS y SDK además de las secciones de referencia están en la barra lateral; les nouvelles importations depuis `docs/source/` (streaming, orquestación, runbooks) llegan aquí en el hilo de su redacción.

## Participante

- Consulte `docs/portal/README.md` para los comandos de desarrollo local (`npm install`, `npm run start`, `npm run build`).
- Las tareas de migración de contenido se siguen con los elementos de la hoja de ruta `DOCS-*`. Les contribuciones sont bienvenues: portez dessections depuis `docs/source/` et ajoutez la page a la barre laterale.
- Si agrega algún tipo de artefacto (especificaciones, cuadros de configuración), documente el comando de compilación para que los futuros contribuyentes puedan regenerar fácilmente.