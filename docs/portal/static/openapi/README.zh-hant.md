---
lang: zh-hant
direction: ltr
source: docs/portal/static/openapi/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8ad316fefd99c4c3b9ddbade7de59f12aa2dbe9ee256784f61ac87bb4341f04a
source_last_modified: "2025-12-29T18:16:35.902041+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

OpenAPI 簽名
----------------

- 必須對 Torii OpenAPI 規範 (`torii.json`) 進行簽名，並且清單由 `cargo xtask openapi-verify` 進行驗證。
- 允許的簽名者密鑰存在於 `allowed_signers.json` 中；每當簽名密鑰發生更改時，都會輪換此文件。將 `version` 字段保留為 `1`。
- CI (`ci/check_openapi_spec.sh`) 已強制執行最新和當前規範的許可名單。如果另一個門戶或管道使用已簽名的規範，請將其驗證步驟指向同一白名單文件以避免漂移。
- 密鑰輪換後重新簽名：
  1. 使用新公鑰更新 `allowed_signers.json`。
  2. 重新生成/簽署規範：`NORITO_SKIP_BINDINGS_SYNC=1 cargo xtask openapi --output docs/portal/static/openapi/torii.json --sign <ed25519-key-hex-path>`。
  3. 重新運行 `ci/check_openapi_spec.sh`（或手動運行 `cargo xtask openapi-verify`）以確認清單與允許列表匹配。