---
lang: pt
direction: ltr
source: docs/portal/docs/reference/publishing-checklist.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# Lista de verificacion de publicacion

Usa esta lista cada vez que actualices el portal de desarrolladores. Garantiza que el build de CI, el despliegue en GitHub Pages y las pruebas manuales de humo cubran cada seccion antes de que llegue un release o un hito del roadmap.

## 1. Validacion local

- `npm run sync-openapi -- --version=current --latest` (agrega uno o mas flags `--mirror=<label>` cuando Torii OpenAPI cambie para un snapshot congelado).
- `npm run build` - confirma que el copy del hero `Build on Iroha with confidence` sigue apareciendo en `build/index.html`.
- `./docs/portal/scripts/preview_verify.sh --build-dir build` - verifica el manifiesto de checksums (agrega `--descriptor`/`--archive` al probar artefactos descargados de CI).
- `npm run serve` - lanza el helper de preview con gating de checksum que verifica el manifiesto antes de llamar a `docusaurus serve`, para que los reviewers no naveguen un snapshot sin firma (el alias `serve:verified` sigue disponible para llamadas explicitas).
- Revisa el markdown que tocaste via `npm run start` y el servidor de live reload.

## 2. Chequeos de pull request

- Verifica que el job `docs-portal-build` haya pasado en `.github/workflows/check-docs.yml`.
- Confirma que `ci/check_docs_portal.sh` se haya ejecutado (los logs de CI muestran el hero smoke check).
- Asegura que el workflow de preview subio un manifiesto (`build/checksums.sha256`) y que el script de verificacion de preview tuvo exito (los logs muestran la salida de `scripts/preview_verify.sh`).
- Agrega la URL de preview publicada desde el entorno de GitHub Pages a la descripcion del PR.

## 3. Aprobacion por seccion

| Seccion | Owner | Checklist |
|---------|-------|-----------|
| Homepage | DevRel | El hero renderiza, las tarjetas de quickstart enlazan a rutas validas, los botones CTA resuelven. |
| Norito | Norito WG | Las guias de overview y getting-started referencian los flags mas recientes del CLI y la documentacion del esquema Norito. |
| SoraFS | Storage Team | El quickstart se ejecuta hasta el final, los campos del reporte de manifest estan documentados, las instrucciones de simulacion de fetch verificadas. |
| Guias SDK | Lideres SDK | Las guias de Rust/Python/JS compilan los ejemplos actuales y enlazan a repos vivos. |
| Referencia | Docs/DevRel | El indice lista las specs mas recientes, la referencia del codec Norito coincide con `norito.md`. |
| Artefacto de preview | Docs/DevRel | El artefacto `docs-portal-preview` esta adjunto al PR, los smoke checks pasan, el enlace se comparte con reviewers. |
| Security & Try it sandbox | Docs/DevRel  Security | OAuth device-code login configurado (`DOCS_OAUTH_*`), checklist `security-hardening.md` ejecutado, encabezados CSP/Trusted Types verificados via `npm run build` o `npm run probe:portal`. |

Marca cada fila como parte de tu revision de PR, o anota tareas pendientes para que el seguimiento de estado siga siendo preciso.

## 4. Notas de lanzamiento

- Incluye `https://docs.iroha.tech/` (o la URL del entorno proveniente del job de despliegue) en las notas de release y actualizaciones de estado.
- Destaca cualquier seccion nueva o modificada para que los equipos downstream sepan donde volver a ejecutar sus propias pruebas de humo.
