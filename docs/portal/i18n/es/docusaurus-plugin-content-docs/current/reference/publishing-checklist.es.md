---
lang: es
direction: ltr
source: docs/portal/docs/reference/publishing-checklist.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Lista de verificación de publicación

Usa esta lista cada vez que actualiza el portal de desarrolladores. Garantiza que la compilación de CI, el despliegue en GitHub Pages y las pruebas manuales de humo cubren cada sección antes de que llegue un lanzamiento o un hito del roadmap.

## 1. Validación local

- `npm run sync-openapi -- --version=current --latest` (agrega uno o más flags `--mirror=<label>` cuando Torii OpenAPI cambia para una instantánea congelada).
- `npm run build` - confirma que la copia del hero `Build on Iroha with confidence` sigue apareciendo en `build/index.html`.
- `./docs/portal/scripts/preview_verify.sh --build-dir build` - verifica el manifiesto de checksums (agrega `--descriptor`/`--archive` al probar artefactos descargados de CI).
- `npm run serve` - lanza el ayudante de vista previa con gating de checksum que verifica el manifiesto antes de llamar a `docusaurus serve`, para que los revisores no naveguen una instantánea sin firma (el alias `serve:verified` sigue disponible para llamadas explícitas).
- Revisa el markdown que tocaste vía `npm run start` y el servidor de live reload.

## 2. Chequeos de solicitud de extracción- Verifica que el trabajo `docs-portal-build` haya pasado en `.github/workflows/check-docs.yml`.
- Confirma que `ci/check_docs_portal.sh` se haya ejecutado (los logs de CI muestran el hero smoke check).
- Asegura que el flujo de trabajo de vista previa subio un manifiesto (`build/checksums.sha256`) y que el script de verificación de vista previa tuvo éxito (los logs muestran la salida de `scripts/preview_verify.sh`).
- Agrega la URL de vista previa publicada desde el entorno de GitHub Pages a la descripción del PR.

## 3. Aprobación por sección| Sección | Propietario | Lista de verificación |
|---------|-------|-----------|
| Página de inicio | Desarrollol | El héroe renderiza, las tarjetas de inicio rápido enlazan a rutas válidas, los botones CTA resuelven. |
| Norito | Norito GT | Las guías de descripción general y introducción hacen referencia a las banderas más recientes del CLI y la documentación del esquema Norito. |
| SoraFS | Equipo de almacenamiento | El inicio rápido se ejecuta hasta el final, los campos del informe de manifiesto están documentados, las instrucciones de simulación de fetch verificadas. |
| Guías SDK | Líderes SDK | Las guías de Rust/Python/JS compilan los ejemplos actuales y enlazan a repositorios vivos. |
| Referencia | Documentos/DevRel | El índice lista las especificaciones más recientes, la referencia del códec Norito coincide con `norito.md`. |
| Artefacto de vista previa | Documentos/DevRel | El artefacto `docs-portal-preview` está adjunto al PR, los controles de humo pasan, el enlace se comparte con revisores. |
| Seguridad y Pruébelo en la zona de pruebas | Seguridad de Documentos/DevRel | Inicio de sesión con código de dispositivo OAuth configurado (`DOCS_OAUTH_*`), lista de verificación `security-hardening.md` ejecutado, encabezados CSP/Trusted Types verificados vía `npm run build` o `npm run probe:portal`. |

Marca cada fila como parte de tu revisión de PR, o anota tareas pendientes para que el seguimiento de estado siga siendo preciso.

## 4. Notas de lanzamiento- Incluye `https://docs.iroha.tech/` (o la URL del entorno proveniente del trabajo de implementación) en las notas de lanzamiento y actualizaciones de estado.
- Destaca cualquier sección nueva o modificada para que los equipos downstream sepan donde volver a ejecutar sus propias pruebas de humo.