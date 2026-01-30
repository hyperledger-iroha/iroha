---
lang: pt
direction: ltr
source: docs/portal/static/openapi/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8ad316fefd99c4c3b9ddbade7de59f12aa2dbe9ee256784f61ac87bb4341f04a
source_last_modified: "2025-11-16T17:11:03.605418+00:00"
translation_last_reviewed: 2026-01-30
---

Assinatura de OpenAPI
---------------------

- A especificação OpenAPI do Torii (`torii.json`) deve ser assinada, e o manifesto é verificado por `cargo xtask openapi-verify`.
- As chaves de assinatura permitidas ficam em `allowed_signers.json`; rotacione esse arquivo sempre que a chave de assinatura mudar. Mantenha o campo `version` em `1`.
- O CI (`ci/check_openapi_spec.sh`) já aplica a allowlist tanto para as specs latest quanto current. Se outro portal ou pipeline consumir a especificação assinada, aponte a verificação para o mesmo arquivo de allowlist para evitar divergências.
- Para assinar novamente após uma rotação de chave:
  1. Atualize `allowed_signers.json` com a nova chave pública.
  2. Regenere e assine a spec: `NORITO_SKIP_BINDINGS_SYNC=1 cargo xtask openapi --output docs/portal/static/openapi/torii.json --sign <ed25519-key-hex-path>`.
  3. Execute novamente `ci/check_openapi_spec.sh` (ou `cargo xtask openapi-verify` manualmente) para confirmar que o manifesto corresponde à allowlist.
