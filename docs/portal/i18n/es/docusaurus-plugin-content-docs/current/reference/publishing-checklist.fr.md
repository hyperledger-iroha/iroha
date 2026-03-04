---
lang: es
direction: ltr
source: docs/portal/docs/reference/publishing-checklist.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Lista de verificación de publicación

Utilice esta lista de verificación cada vez que esté lista en el día del desarrollador. Esto garantiza que la compilación de CI, la implementación de GitHub Pages y los manuales de pruebas de humo se encuentran en cada sección antes de que se publique o se cierre la hoja de ruta.

## 1. Configuración regional de validación

- `npm run sync-openapi -- --version=current --latest` (añada uno de nuestros favoritos `--mirror=<label>` cuando Torii OpenAPI cambie para una imagen de instantánea).
- `npm run build` - confirme que el texto del héroe `Build on Iroha with confidence` aparece siempre en `build/index.html`.
- `./docs/portal/scripts/preview_verify.sh --build-dir build` - verifique el manifiesto de sumas de comprobación (ajoutez `--descriptor`/`--archive` lors des tests d'artefacts CI telecharges).
- `npm run serve` - lanza el asistente de vista previa protegido por suma de verificación que verifica el manifiesto antes de la llamada `docusaurus serve`, para que los revisores no encuentren una instantánea sin firmar (el alias `serve:verified` queda disponible para las aplicaciones explícitas).
- Haga una verificación puntual del descuento que debe modificar a través de `npm run start` y el servidor de recarga en vivo.

## 2. Verificaciones de pull request- Verifique que el trabajo `docs-portal-build` se reussi en `.github/workflows/check-docs.yml`.
- Confirme el torneo `ci/check_docs_portal.sh` (los registros CI muestran el control de humo del héroe).
- Asegúrese de que el flujo de trabajo de vista previa y carga de un manifiesto (`build/checksums.sha256`) y que el script de verificación de vista previa se reutilice (los registros muestran la salida `scripts/preview_verify.sh`).
- Agregue la URL de vista previa publicada después del entorno de GitHub Pages y la descripción de PR.

## 3. Sección de validación par| Sección | Propietario | Lista de verificación |
|---------|-------|-----------|
| Página de inicio | Desarrollol | Se muestra el héroe, las tarjetas de inicio rápido apuntando a las rutas válidas, los botones CTA resolutivos. |
| Norito | Norito GT | La descripción general de las guías y la introducción hacen referencia a las últimas banderas de CLI y la documentación del esquema Norito. |
| SoraFS | Equipo de almacenamiento | El inicio rápido se ejecuta justo después de la pelea, los campos de la relación de manifiesto están documentados y las instrucciones de simulación de recuperación están verificadas. |
| Guías SDK | Lidera el SDK | Las guías compilan Rust/Python/JS con ejemplos actuales y lo envían a repositorios en vivo. |
| Referencia | Documentos/DevRel | El índice enumera las especificaciones más recientes, la referencia del códec Norito corresponde a `norito.md`. |
| Artefacto de vista previa | Documentos/DevRel | El artefacto `docs-portal-preview` está adjunto a las relaciones públicas, los controles de humo pasaron, el gravamen está repartido con los revisores. |
| Seguridad y Pruébelo en la zona de pruebas | Seguridad de Documentos/DevRel | Configuración de inicio de sesión con código de dispositivo OAuth (`DOCS_OAUTH_*`), lista de verificación ejecutada `security-hardening.md`, verificación de CSP/tipos de confianza a través de `npm run build` o `npm run probe:portal`. |

Cochez chaque ligne lors de la revista du PR, ou note toute action de suivi pour que le suivi de statut reste exactitud.

## 4. Notas de lanzamiento- Incluez `https://docs.iroha.tech/` (o la URL del entorno del trabajo de implementación) en las notas de publicación y las actualizaciones del día de estatuto.
- Señale explícitamente todas las secciones nuevas o modificadas según los equipos instalados aguas abajo o los relés de las pruebas de humo propias.