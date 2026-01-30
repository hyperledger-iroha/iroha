---
lang: fr
direction: ltr
source: docs/portal/static/openapi/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8ad316fefd99c4c3b9ddbade7de59f12aa2dbe9ee256784f61ac87bb4341f04a
source_last_modified: "2025-11-16T17:11:03.605418+00:00"
translation_last_reviewed: 2026-01-30
---

Signature OpenAPI
-----------------

- La spécification OpenAPI de Torii (`torii.json`) doit être signée, et le manifeste est vérifié par `cargo xtask openapi-verify`.
- Les clés de signature autorisées se trouvent dans `allowed_signers.json` ; faites tourner ce fichier à chaque changement de clé de signature. Gardez le champ `version` à `1`.
- La CI (`ci/check_openapi_spec.sh`) applique déjà l’allowlist pour les specs latest et current. Si un autre portail ou pipeline consomme la spécification signée, pointez son étape de vérification vers le même fichier allowlist pour éviter les dérives.
- Pour signer à nouveau après une rotation de clé :
  1. Mettez à jour `allowed_signers.json` avec la nouvelle clé publique.
  2. Régénérez et signez la spec : `NORITO_SKIP_BINDINGS_SYNC=1 cargo xtask openapi --output docs/portal/static/openapi/torii.json --sign <ed25519-key-hex-path>`.
  3. Relancez `ci/check_openapi_spec.sh` (ou `cargo xtask openapi-verify` manuellement) pour confirmer que le manifeste correspond à l’allowlist.
