---
lang: es
direction: ltr
source: docs/portal/README.md
status: complete
translator: manual
source_hash: 4b0d6c295c7188355e2c03d7c8240271da147095ff557fae2152f42e27bd17fa
source_last_modified: "2025-11-14T04:43:03.939564+00:00"
translation_last_reviewed: 2025-11-14
---

<!-- Traducción al español de docs/portal/README.md (SORA Nexus Developer Portal) -->

# Portal de Desarrolladores de SORA Nexus

Este directorio aloja el workspace de Docusaurus para el portal interactivo de
desarrolladores. El portal agrega guías de Norito, quickstarts de SDK y la referencia
OpenAPI generada por `cargo xtask openapi`, envolviéndolos con el branding de SORA Nexus
utilizado en todo el programa de documentación.

## Prerrequisitos

- Node.js 18.18 o más reciente (baseline de Docusaurus v3).
- Yarn 1.x o npm ≥ 9 para la gestión de paquetes.
- Toolchain de Rust (usado por el script de sincronización de OpenAPI).

## Bootstrap

```bash
cd docs/portal
npm install    # o yarn install
```

## Scripts disponibles

| Comando | Descripción |
|---------|-------------|
| `npm run start` / `yarn start` | Inicia un servidor de desarrollo local con recarga en vivo (por defecto `http://localhost:3000`). |
| `npm run build` / `yarn build` | Genera un build de producción en `build/`. |
| `npm run serve` / `yarn serve` | Sirve el último build localmente (útil para smoke tests). |
| `npm run docs:version -- <label>` | Toma un snapshot de la documentación actual en `versioned_docs/version-<label>` (wrapper de `docusaurus docs:version`). |
| `npm run sync-openapi` / `yarn sync-openapi` | Regenera `static/openapi/torii.json` vía `cargo xtask openapi` (pasa `--mirror=<label>` para copiar la spec a snapshots de versión adicionales). |
| `npm run tryit-proxy` | Lanza el proxy de staging que alimenta la consola “Try it” (ver configuración más abajo). |
| `npm run probe:tryit-proxy` | Lanza un probe `/healthz` + una petición de ejemplo contra el proxy (helper para CI/monitorización). |
| `npm run manage:tryit-proxy -- <update|rollback>` | Actualiza o restaura el `.env` de destino del proxy con manejo de backups. |
| `npm run sync-i18n` | Garantiza que existan stubs de traducción para japonés, hebreo, español, portugués, francés, ruso, árabe y urdu bajo `i18n/`. |
| `npm run sync-norito-snippets` | Regenera documentos de ejemplos Kotodama curados + snippets descargables (también se invoca automáticamente desde el plugin del servidor de desarrollo). |
| `npm run test:tryit-proxy` | Ejecuta los tests unitarios del proxy usando el test runner de Node (`node --test`). |

El script de sincronización de OpenAPI requiere que `cargo xtask openapi` esté disponible
desde la raíz del repositorio; genera un archivo JSON determinista en `static/openapi/` y
ahora espera que el router de Torii exponga una spec en vivo (usa
`cargo xtask openapi --allow-stub` solo para salida temporal de emergencia).

## Versionado de docs y snapshots de OpenAPI

- **Crear una versión de docs:** ejecuta `npm run docs:version -- 2025-q3` (o cualquier
  etiqueta acordada). Haz commit de `versioned_docs/version-<label>`,
  `versioned_sidebars` y `versions.json`. El desplegable de versiones de la navbar
  mostrará automáticamente el nuevo snapshot.
- **Sincronizar artefactos OpenAPI:** después de cortar una versión, actualiza la spec
  canónica y el manifest con `cargo xtask openapi --sign <ruta-a-clave-ed25519>`, luego
  captura un snapshot coincidente con
  `npm run sync-openapi -- --version=2025-q3 --mirror=current --latest`. El script escribe
  `static/openapi/versions/2025-q3/torii.json`, copia la spec a
  `versions/current/torii.json`, actualiza `versions.json`, refresca
  `/openapi/torii.json` y clona el `manifest.json` firmado en cada directorio de versión
  para que las specs históricas compartan el mismo metadata de procedencia. Puede
  suministrar cualquier número de flags `--mirror=<label>` para copiar la spec recién
  generada en otros snapshots históricos.
- **Expectativas de CI:** los commits que toquen docs deben incluir el bump de versión (si
  aplica) y snapshots OpenAPI actualizados, de modo que los paneles Swagger, RapiDoc y
  Redoc puedan cambiar entre specs históricas sin errores de fetch.
- **Enforcement del manifest:** el script `sync-openapi` solo copia manifests cuando
  `manifest.json` en disco coincide con la spec recién generada. Si omite la copia, vuelve
  a ejecutar `cargo xtask openapi --sign <clave>` para actualizar el manifest canónico y
  repite la sincronización para que los snapshots versionados recojan el metadata
  firmado. El script `ci/check_openapi_spec.sh` reejecuta el generador y valida el
  manifest antes de permitir el merge de commits.

## Estructura

```text
docs/portal/
├── docs/                 # Contenido Markdown/MDX del portal
├── i18n/                 # Overrides de idiomas (ja/he) generados por sync-i18n
├── src/                  # Páginas/componentes React (scaffolding placeholder)
├── static/               # Recursos estáticos servidos tal cual (incluye JSON de OpenAPI)
├── scripts/              # Scripts de ayuda (sincronización de OpenAPI)
├── docusaurus.config.js  # Configuración principal del sitio
└── sidebars.js           # Modelo de navegación / sidebars
```

### Configuración del proxy Try It

El sandbox “Try it” enruta las peticiones a través de `scripts/tryit-proxy.mjs`. Configura
el proxy con variables de entorno antes de lanzarlo:

```bash
export TRYIT_PROXY_TARGET="https://torii.staging.sora"
export TRYIT_PROXY_ALLOWED_ORIGINS="http://localhost:3000"
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
```

En entornos de producción/staging, define estas variables en el sistema de configuración
de tu plataforma (por ejemplo, secretos de GitHub Actions, variables de entorno en el
orquestador de contenedores, etc.).

### URLs de preview y notas de release

- Preview pública en beta: `https://docs.iroha.tech/`
- GitHub también expone el build bajo el entorno **github-pages** para cada despliegue.
- Los pull requests que tocan contenido del portal incluyen artefactos de Actions
  (`docs-portal-preview`, `docs-portal-preview-metadata`) con el sitio compilado, manifest
  de checksums, archivo comprimido y descriptor; quienes revisan pueden descargar y abrir
  `index.html` localmente y verificar los checksums antes de compartir previews. El
  workflow deja un comentario de resumen (hashes de manifest/archivo más estado de SoraFS)
  en cada PR para proporcionar una señal rápida de que la verificación ha pasado.
- Usa `./docs/portal/scripts/preview_verify.sh --build-dir <build extraído> --descriptor <descriptor> --archive <archivo>` después de descargar un bundle de preview para confirmar que los artefactos coinciden con lo que produjo CI antes de compartir el enlace externamente.
- Al preparar notas de versión o actualizaciones de estado, referencia la URL de preview
  para que quienes revisan puedan navegar el snapshot más reciente del portal sin clonar
  el repositorio.
- Coordina las oleadas de preview mediante
  `docs/portal/docs/devportal/preview-invite-flow.md` y combínalo con
  `docs/portal/docs/devportal/reviewer-onboarding.md` para que cada invitación، export de
  telemetría y paso de offboarding reutilicen نفس أثر الأدلة نفسه.

