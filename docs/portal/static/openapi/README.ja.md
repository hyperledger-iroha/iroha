---
lang: ja
direction: ltr
source: docs/portal/static/openapi/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8ad316fefd99c4c3b9ddbade7de59f12aa2dbe9ee256784f61ac87bb4341f04a
source_last_modified: "2025-11-16T17:11:03.605418+00:00"
translation_last_reviewed: 2026-01-30
---

OpenAPI の署名
---------------

- Torii の OpenAPI 仕様（`torii.json`）は署名が必須で、マニフェストは `cargo xtask openapi-verify` で検証されます。
- 許可された署名鍵は `allowed_signers.json` に保存されます。署名鍵が変更されたらこのファイルをローテーションし、`version` フィールドは `1` に保ってください。
- CI（`ci/check_openapi_spec.sh`）は latest と current の仕様に対して allowlist を既に適用しています。別のポータルやパイプラインが署名済み仕様を利用する場合は、同じ allowlist ファイルを検証ステップに指定して差異を防いでください。
- 鍵のローテーション後に再署名するには:
  1. `allowed_signers.json` を新しい公開鍵で更新する。
  2. 仕様を再生成して署名する: `NORITO_SKIP_BINDINGS_SYNC=1 cargo xtask openapi --output docs/portal/static/openapi/torii.json --sign <ed25519-key-hex-path>`.
  3. `ci/check_openapi_spec.sh`（または `cargo xtask openapi-verify` を手動で）を再実行し、マニフェストが allowlist と一致することを確認する。
