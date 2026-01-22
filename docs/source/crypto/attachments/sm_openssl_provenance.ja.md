---
lang: ja
direction: ltr
source: docs/source/crypto/attachments/sm_openssl_provenance.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: cb88618a92dc54bfc2091c10ab45a4b90d69c4b767e5d48c4a3ba6812a13467f
source_last_modified: "2026-01-30T09:44:29.106026+00:00"
translation_last_reviewed: 2026-01-21
---

<!-- 日本語訳: docs/source/crypto/attachments/sm_openssl_provenance.md -->

% SM OpenSSL/Tongsuo Provenance Snapshot
% Generated: 2026-01-30

# 環境サマリ

- `pkg-config --modversion openssl`: `3.6.0`
- `openssl version -a`: `LibreSSL 3.3.6` を報告（macOS のシステム提供 TLS ツールキット）。
- `cargo tree -p iroha_crypto --features "sm sm-ffi-openssl"`: 正確な Rust 依存スタックは
  `sm_iroha_crypto_tree.txt` を参照（`openssl` crate v0.10.74、`openssl-sys` v0.9.x、
  `openssl-src` crate 経由で OpenSSL 3.x ソースが vendored として利用可能。
  `crates/iroha_crypto/Cargo.toml` では deterministic preview build のため `vendored` を有効化）。

# 注記

- ローカル開発環境は LibreSSL のヘッダ/ライブラリへリンクする。
  本番 preview ビルドでは OpenSSL >= 3.0.0 または Tongsuo 8.x を使用すること。
  最終アーティファクト生成時はシステムツールキットを置換するか
  `OPENSSL_DIR`/`PKG_CONFIG_PATH` を設定する。
- リリースビルド環境でこのスナップショットを再生成し、OpenSSL/Tongsuo の tarball
  ハッシュ（`openssl version -v`, `openssl version -b`, `openssl version -f`）を取得し、
  再現ビルドのスクリプト/チェックサムを添付する。vendored ビルドの場合は、Cargo が
  使用した `openssl-src` crate のバージョン/コミットを記録する
  （`target/debug/build/openssl-sys-*/output` に表示）。
- Apple Silicon では OpenSSL smoke harness 実行時に `RUSTFLAGS=-Aunsafe-code` が必要。
  AArch64 の SM3/SM4 アクセラレーション stub が macOS で利用できず、
  `scripts/sm_openssl_smoke.sh` が `cargo` 実行前にこのフラグをエクスポートして
  CI とローカル実行の一貫性を保つ。
- パッケージングパイプラインが固定されたら upstream の provenance（例:
  `openssl-src-<ver>.tar.gz` の SHA256）を添付し、CI アーティファクトでも同じハッシュを使用する。
