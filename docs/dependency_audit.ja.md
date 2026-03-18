<!-- Japanese translation of docs/dependency_audit.md -->

---
lang: ja
direction: ltr
source: docs/dependency_audit.md
status: complete
translator: manual
---

//! 依存関係監査サマリー

日付: 2025-09-01

対象範囲: Cargo.toml に記載され、Cargo.lock で解決されているワークスペース内のすべてのクレート。RustSec アドバイザリ DB に対する cargo-audit に加え、クレートの正当性およびアルゴリズムごとの「標準クレート」選定を手動で確認した。

使用したツール／コマンド:
- `cargo tree -d --workspace --locked --offline` — 重複バージョンの確認
- `cargo audit` — Cargo.lock を既知の脆弱性と yanked クレートについてスキャン

検出されたセキュリティ勧告（現状: 脆弱性 0 件、警告 2 件）:
- crossbeam-channel — RUSTSEC-2025-0024
  - 対応: `crates/ivm/Cargo.toml` でバージョンを `0.5.15` に更新。

  - 対応: `crates/iroha_torii/Cargo.toml` で `pprof` を `prost-codec` に切り替え。

- ring — RUSTSEC-2025-0009
  - 対応: QUIC/TLS スタック（`quinn 0.11`、`rustls 0.23`、`tokio-rustls 0.26`）を更新し、WS スタックも `tungstenite/tokio-tungstenite 0.24` に更新。`cargo update -p ring --precise 0.17.12` で `ring 0.17.12` をロック。

残存する勧告: なし。残存する警告: `backoff`（メンテナンス停止）、`derivative`（メンテナンス停止）。

正当性および「標準クレート」評価（主な結果）:
- ハッシュ: `sha2`（RustCrypto）、`blake2`（RustCrypto）、`tiny-keccak`（広く利用）— いずれも標準的。
- AEAD／共通鍵暗号: `aes-gcm`、`chacha20poly1305`、`aead` トレイト（RustCrypto）— 標準的。
- 署名／ECC: `ed25519-dalek`、`x25519-dalek`（dalek プロジェクト）、`k256`（RustCrypto）、`secp256k1`（libsecp バインディング）— いずれも正当。実装の重複を避けるため、可能なら `k256` か `secp256k1` のどちらかに揃える。
- BLS12-381／ZK: `blstrs`、`halo2_*` — ZK エコシステムで広く実績あり。
- PQ: `pqcrypto-dilithium`、`pqcrypto-traits` — リファレンスクレートとして信頼できる。
- TLS: `rustls`、`tokio-rustls`、`hyper-rustls` — 現代的な Rust TLS スタックの標準。
- Noise: `snow` — 標準的な実装。
- シリアライゼーション: `parity-scale-codec` は SCALE の標準。Serde は本番依存から削除済みで、Norito の derive／writer がすべてのランタイム経路をカバー。残存する Serde 参照は履歴ドキュメント、ガードレールスクリプト、テスト専用の許可リストのみ。
- FFI／ネイティブライブラリ: `libsodium-sys-stable`、`openssl` — 正当だが、本番経路では OpenSSL より Rustls を優先（現行コードは既にその方針）。
- `pprof` 0.13.0（crates.io）— 上流修正済み。公式リリースに `prost-codec` と frame-pointer を有効化した構成で利用し、旧コーデックは無効化済み。

推奨事項:
- 警告への対応:
  - `backoff` を `retry`／`futures-retry` または自前の指数バックオフ実装に置き換えることを検討。
  - `derivative` の derive を手書き実装か `derive_more` へ置き換える。
- 中優先: 可能な限り `k256` か `secp256k1` のどちらかに統一し、重複する実装を減らす（必要な場合のみ両方を残す）。
- 中優先: ZK 用に利用している `poseidon-primitives 0.2.0` の出自を再確認し、可能であれば Arkworks／Halo2 ネイティブの Poseidon 実装に合わせて並列エコシステムを縮小する。

備考:
- `cargo tree -d` では期待通りのメジャーバージョン重複（`bitflags` 1/2、複数の `ring`）のみが確認された。直接のセキュリティリスクではないがビルド表面は広がる。
- タイポスクワットと思われるクレートは検出されず、名称とソースはいずれも既知のエコシステムクレートまたはワークスペース内の内部メンバーに解決された。
- 実験的対応: `iroha_crypto` に `bls-backend-blstrs` フィーチャを追加し、BLS のバックエンドを blstrs のみに移行し始めた（このフィーチャを有効にすると Arkworks への依存を排除）。デフォルトは引き続き `w3f-bls` で、挙動／エンコーディングの変更を避ける。整合計画:
  - `Scalar::to_bytes_le` を用いた 32 バイト little-endian 形式にプライベートキーのシリアライズを正規化し、旧来の混在エンディアン補助関数を廃止する。
  - `blstrs::G1Affine::to_compressed` を再利用する公開鍵圧縮ラッパーを導入し、w3f エンコーディングとの比較チェックを追加してワイヤフォーマットの一致を保証する。
  - `crates/iroha_crypto/tests/bls_backend_compat.rs` に鍵生成・公開鍵・署名集約のラウンドトリップ fixture を追加し、`w3f-bls` と `blstrs` の結果が一致することをテストで確認する。
  - CI では `bls-backend-blstrs` フィーチャを明示的に有効化するジョブを用意しつつ、既定バックエンドでも互換テストを常時実行して回 regresion を早期検知する。

フォローアップ（提案される作業項目）:
- CI 内の Serde ガードレール（`scripts/check_no_direct_serde.sh`、`scripts/deny_serde_json.sh`）を維持し、新たな本番用途が入り込まないようにする。

本監査で実施したテスト:
- 最新のアドバイザリ DB で `cargo audit` を実行し、4 件の勧告と依存関係ツリーを確認。
- 該当クレートの直接依存宣言を検索し、修正箇所を特定。
