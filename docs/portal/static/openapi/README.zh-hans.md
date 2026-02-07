---
lang: zh-hans
direction: ltr
source: docs/portal/static/openapi/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8ad316fefd99c4c3b9ddbade7de59f12aa2dbe9ee256784f61ac87bb4341f04a
source_last_modified: "2025-12-29T18:16:35.902041+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

OpenAPI 签名
----------------

- 必须对 Torii OpenAPI 规范 (`torii.json`) 进行签名，并且清单由 `cargo xtask openapi-verify` 进行验证。
- 允许的签名者密钥存在于 `allowed_signers.json` 中；每当签名密钥发生更改时，都会轮换此文件。将 `version` 字段保留为 `1`。
- CI (`ci/check_openapi_spec.sh`) 已强制执行最新和当前规范的许可名单。如果另一个门户或管道使用已签名的规范，请将其验证步骤指向同一白名单文件以避免漂移。
- 密钥轮换后重新签名：
  1. 使用新公钥更新 `allowed_signers.json`。
  2. 重新生成/签署规范：`NORITO_SKIP_BINDINGS_SYNC=1 cargo xtask openapi --output docs/portal/static/openapi/torii.json --sign <ed25519-key-hex-path>`。
  3. 重新运行 `ci/check_openapi_spec.sh`（或手动运行 `cargo xtask openapi-verify`）以确认清单与允许列表匹配。