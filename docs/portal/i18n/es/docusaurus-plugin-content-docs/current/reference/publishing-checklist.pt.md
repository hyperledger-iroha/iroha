---
lang: es
direction: ltr
source: docs/portal/docs/reference/publishing-checklist.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Lista de verificación de publicación

Utilice esta lista de verificación siempre que actualice el portal de desarrolladores. Ele garantiza que la compilación de CI, la implementación de páginas de GitHub y las pruebas de humo manuales cubren todos los segundos antes de un lanzamiento o marco de la hoja de ruta.

## 1. Validacao local

- `npm run sync-openapi -- --version=current --latest` (añadir o más flags `--mirror=<label>` cuando o Torii OpenAPI mudar para una instantánea congelada).
- `npm run build` - confirme que o hero copy `Build on Iroha with confidence` todavía aparece en `build/index.html`.
- `./docs/portal/scripts/preview_verify.sh --build-dir build` - verifique o manifesto de checksums (adicione `--descriptor`/`--archive` ao testar artefatos de CI baixados).
- `npm run serve` - inicia el ayudante de vista previa con checksum gating que verifica el manifiesto antes de chamar `docusaurus serve`, para que los revisores nunca naveguen um snapshot sin assinatura (o alias `serve:verified` permanece para chamadas explícitas).
- Faca um spot-check do markdown alterado vía `npm run start` y el servidor de recarga en vivo.

## 2. Verificaciones de solicitud de extracción

- Verifique que el trabajo `docs-portal-build` pasó en `.github/workflows/check-docs.yml`.
- Confirme que `ci/check_docs_portal.sh` rodou (logs de CI mostram o hero smoke check).
- Garantía de que el flujo de trabajo de vista previa envió un manifiesto (`build/checksums.sha256`) y que el script de verificación de vista previa de su éxito (los registros se muestran en la tabla de `scripts/preview_verify.sh`).
- Agregue una URL de vista previa publicada en el ambiente de GitHub Pages en la descripción de relaciones públicas.## 3. Aprobación por secado

| Secao | Propietario | Lista de verificación |
|---------|-------|-----------|
| Página de inicio | Desarrollol | Representación de copia de héroe, enlace de tarjetas de inicio rápido para rotaciones válidas, resolución de botones de CTA. |
| Norito | Norito GT | Descripción general de las guías y referencias de introducción a las banderas más recientes de CLI y a los documentos del esquema Norito. |
| SoraFS | Equipo de almacenamiento | Quickstart roda ate o fim, campos de informe de manifiesto documentados, instrucciones de simulación de recuperación verificadas. |
| Guías SDK | Clientes potenciales del SDK | Las guías Rust/Python/JS compilan ejemplos actuales y enlaces para repositorios en vivo. |
| Referencia | Documentos/DevRel | En la lista de índice de especificaciones más recientes, la referencia del códec Norito coincide con `norito.md`. |
| Vista previa del artefacto | Documentos/DevRel | El artefato `docs-portal-preview` esta anexado ao PR, smoke checks passam, o link e compartilhado com reviewers. |
| Seguridad y Pruébelo en la zona de pruebas | Documentos/DevRel / Seguridad | Inicio de sesión con código de dispositivo OAuth configurado (`DOCS_OAUTH_*`), lista de verificación `security-hardening.md` ejecutada, encabezados CSP/Tipos de confianza verificados a través de `npm run build` o `npm run probe:portal`. |

Marque cada línea como parte de su revisión de relaciones públicas, o anote tareas de seguimiento para mantener o rastrear el estado preciso.

## 4. Notas de la versión- Incluye `https://docs.iroha.tech/` (o una URL del entorno del trabajo de implementación) y notas de la versión actualizadas del estado.
- Destaque quaisquer secoes novas ou alteradas para que as equipes downstream saibam onde reejecutar sus propias pruebas de humo.