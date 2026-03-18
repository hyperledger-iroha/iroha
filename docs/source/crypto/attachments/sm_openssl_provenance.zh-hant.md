---
lang: zh-hant
direction: ltr
source: docs/source/crypto/attachments/sm_openssl_provenance.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 95a34657b6064f925995a7e9f20145d14fda681f4af1f182418b9f624047e576
source_last_modified: "2025-12-29T18:16:35.937817+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% SM OpenSSL/通所出處快照
生成百分比：2026-01-30

# 環境總結

- `pkg-config --modversion openssl`: `3.6.0`
- `openssl version -a`：報告 `LibreSSL 3.3.6`（macOS 上系統提供的 TLS 工具包）。
- `cargo tree -p iroha_crypto --features "sm sm-ffi-openssl"`：請參閱 `sm_iroha_crypto_tree.txt` 了解確切的 Rust 依賴堆棧（`openssl` crate v0.10.74、`openssl-sys` v0.9.x、通過 `openssl-src` crate 提供的供應商 OpenSSL 3.x 源；功能`vendored` 在 `crates/iroha_crypto/Cargo.toml` 中啟用，用於確定性預覽版本）。

# 註釋

- 本地開發環境鏈接到 LibreSSL 標頭/庫；生產預覽版本必須使用 OpenSSL >= 3.0.0 或 Tongsuo 8.x。生成最終工件包時，替換系統工具包或設置 `OPENSSL_DIR`/`PKG_CONFIG_PATH`。
- 在發布構建環境中重新生成此快照，以捕獲准確的 OpenSSL/Tongsuo tarball 哈希（`openssl version -v`、`openssl version -b`、`openssl version -f`）並附加可重現的構建腳本/校驗和。對於供應商版本，記錄 Cargo 使用的 `openssl-src` 板條箱版本/提交（在 `target/debug/build/openssl-sys-*/output` 中可見）。
- Apple Silicon 主機在運行 OpenSSL Smoke Harness 時需要 `RUSTFLAGS=-Aunsafe-code`，以便編譯 AArch64 SM3/SM4 加速存根（內在函數在 macOS 上不可用）。腳本 `scripts/sm_openssl_smoke.sh` 在調用 `cargo` 之前導出此標誌，以保持 CI 和本地運行一致。
- 固定打包管道後，附加上游源出處（例如，`openssl-src-<ver>.tar.gz` SHA256）；在 CI 製品中使用相同的哈希值。