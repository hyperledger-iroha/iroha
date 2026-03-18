---
lang: es
direction: ltr
source: docs/portal/static/openapi/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8ad316fefd99c4c3b9ddbade7de59f12aa2dbe9ee256784f61ac87bb4341f04a
source_last_modified: "2025-11-16T17:11:03.605418+00:00"
translation_last_reviewed: 2026-01-30
---

Firma de OpenAPI
----------------

- La especificación OpenAPI de Torii (`torii.json`) debe firmarse, y el manifiesto se verifica con `cargo xtask openapi-verify`.
- Las claves de firma permitidas se mantienen en `allowed_signers.json`; rota este archivo cada vez que cambie la clave de firma. Mantén el campo `version` en `1`.
- El CI (`ci/check_openapi_spec.sh`) ya aplica la allowlist para las especificaciones latest y current. Si otro portal o pipeline consume la especificación firmada, apunta su verificación al mismo archivo de allowlist para evitar divergencias.
- Para volver a firmar después de una rotación de claves:
  1. Actualiza `allowed_signers.json` con la nueva clave pública.
  2. Regenera y firma la spec: `NORITO_SKIP_BINDINGS_SYNC=1 cargo xtask openapi --output docs/portal/static/openapi/torii.json --sign <ed25519-key-hex-path>`.
  3. Ejecuta de nuevo `ci/check_openapi_spec.sh` (o `cargo xtask openapi-verify` manualmente) para confirmar que el manifiesto coincide con la allowlist.
