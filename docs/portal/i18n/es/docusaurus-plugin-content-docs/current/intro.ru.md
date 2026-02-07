---
lang: es
direction: ltr
source: docs/portal/docs/intro.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Добро пожаловать на портал разработчиков SORA Nexus

El portal de software SORA Nexus contiene documentos interactivos, SDK y materiales adicionales API de configuración para operadores Nexus y controladores Hyperledger Iroha. En la mayoría de los documentos disponibles, consulte el plan de estudios prácticos y especializados de forma genérica. напрямую из этого репозитория. El tipo de préstamo incluye temas temáticos como Norito/SoraFS, подписанные снимки OpenAPI y отдельный справочник Norito Streaming, чтобы контрибьюторы могли найти контракт контрольной плоскости стриминга, не пролистывая корневую спецификацию.

## Что можно сделать здесь

- **Изучить Norito** - Acceda a la ventana de inicio rápido y muestre el código de bytes del modelo de serie y de los instrumentos.
- **Instalar SDK** - Siga el inicio rápido de JavaScript y Rust en una sección; Los programas de Python, Swift y Android se pueden utilizar simplemente como receptor de migración.
- **API de actualización** - página Torii OpenAPI agrega las especificaciones actuales de REST y tablas Configuraciones de configuración de canales de Markdown.
- **Подготовить развертывания** - эксплуатационные runbook-и (telemetría, liquidación, superposiciones Nexus) переносятся из `docs/source/` и появятся здесь по мере миграции.

## Estado técnico- ✅ Préstamo temático Docusaurus v3 con tipografía novedosa, héroe/tarjetas en gradientes y recursos de archivos, включая сводку Norito Transmisión.
- ✅ La placa Torii OpenAPI se puede conectar a `npm run sync-openapi`, con protectores de pantalla y protectores CSP-guard, применяемыми `buildSecurityHeaders`.
- ✅ Vista previa y cobertura de la sonda disponibles en CI (`docs-portal-preview.yml` + `scripts/portal-probe.mjs`), control de documentos en streaming, inicios rápidos SoraFS y listas de verificación de referencia antes de la publicación artefactos.
- ✅ Inicios rápidos Norito, SoraFS y SDK más secciones utilizadas en paneles de bloques; Nuevas importaciones de `docs/source/` (streaming, orquestación, runbooks) se pueden encontrar en esta versión.

## Как принять участие

- См. `docs/portal/README.md` para los controladores de comandos locales (`npm install`, `npm run start`, `npm run build`).
- El contenido de cada migración está incluido en la hoja de ruta de puntos `DOCS-*`. Вклад приветствуется - Perenosite las secciones de `docs/source/` y coloque la página en el panel frontal.
- Если вы добавляете генерируемый артефакт (especificaciones, tablas de configuración), задокументируйте команду сборки, чтобы будущие участники могли легко его обновить.