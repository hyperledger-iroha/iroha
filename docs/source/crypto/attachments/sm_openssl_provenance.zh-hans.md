---
lang: zh-hans
direction: ltr
source: docs/source/crypto/attachments/sm_openssl_provenance.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 95a34657b6064f925995a7e9f20145d14fda681f4af1f182418b9f624047e576
source_last_modified: "2025-12-29T18:16:35.937817+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% SM OpenSSL/通所出处快照
生成百分比：2026-01-30

# 环境总结

- `pkg-config --modversion openssl`: `3.6.0`
- `openssl version -a`：报告 `LibreSSL 3.3.6`（macOS 上系统提供的 TLS 工具包）。
- `cargo tree -p iroha_crypto --features "sm sm-ffi-openssl"`：请参阅 `sm_iroha_crypto_tree.txt` 了解确切的 Rust 依赖堆栈（`openssl` crate v0.10.74、`openssl-sys` v0.9.x、通过 `openssl-src` crate 提供的供应商 OpenSSL 3.x 源；功能`vendored` 在 `crates/iroha_crypto/Cargo.toml` 中启用，用于确定性预览版本）。

# 注释

- 本地开发环境链接到 LibreSSL 标头/库；生产预览版本必须使用 OpenSSL >= 3.0.0 或 Tongsuo 8.x。生成最终工件包时，替换系统工具包或设置 `OPENSSL_DIR`/`PKG_CONFIG_PATH`。
- 在发布构建环境中重新生成此快照，以捕获准确的 OpenSSL/Tongsuo tarball 哈希（`openssl version -v`、`openssl version -b`、`openssl version -f`）并附加可重现的构建脚本/校验和。对于供应商版本，记录 Cargo 使用的 `openssl-src` 板条箱版本/提交（在 `target/debug/build/openssl-sys-*/output` 中可见）。
- Apple Silicon 主机在运行 OpenSSL Smoke Harness 时需要 `RUSTFLAGS=-Aunsafe-code`，以便编译 AArch64 SM3/SM4 加速存根（内在函数在 macOS 上不可用）。脚本 `scripts/sm_openssl_smoke.sh` 在调用 `cargo` 之前导出此标志，以保持 CI 和本地运行一致。
- 固定打包管道后，附加上游源出处（例如，`openssl-src-<ver>.tar.gz` SHA256）；在 CI 制品中使用相同的哈希值。