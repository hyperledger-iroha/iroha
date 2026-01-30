---
lang: ru
direction: ltr
source: docs/portal/static/openapi/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8ad316fefd99c4c3b9ddbade7de59f12aa2dbe9ee256784f61ac87bb4341f04a
source_last_modified: "2025-11-16T17:11:03.605418+00:00"
translation_last_reviewed: 2026-01-30
---

Подпись OpenAPI
---------------

- Спецификация OpenAPI для Torii (`torii.json`) должна быть подписана, а манифест проверяется командой `cargo xtask openapi-verify`.
- Разрешённые ключи подписи хранятся в `allowed_signers.json`; обновляйте этот файл при смене ключа подписи. Поле `version` держите равным `1`.
- CI (`ci/check_openapi_spec.sh`) уже применяет allowlist для спецификаций latest и current. Если другой портал или пайплайн использует подписанную спецификацию, укажите для проверки тот же файл allowlist, чтобы избежать рассинхронизации.
- Чтобы переподписать после ротации ключей:
  1. Обновите `allowed_signers.json` новым публичным ключом.
  2. Пересоздайте и подпишите спецификацию: `NORITO_SKIP_BINDINGS_SYNC=1 cargo xtask openapi --output docs/portal/static/openapi/torii.json --sign <ed25519-key-hex-path>`.
  3. Повторно запустите `ci/check_openapi_spec.sh` (или `cargo xtask openapi-verify` вручную), чтобы подтвердить, что манифест соответствует allowlist.
