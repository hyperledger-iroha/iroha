<!-- Japanese translation of docs/source/crypto/dependency_audits.md -->

---
lang: ja
direction: ltr
source: docs/source/crypto/dependency_audits.md
status: complete
translator: manual
---

# 暗号依存関係の監査

## Streebog（`streebog` クレート）

- **ツリー内バージョン**: `0.11.0-rc.2` を `vendor/streebog` に同梱（`gost` フィーチャ有効時）。
- **利用箇所**: `crates/iroha_crypto::signature::gost`（HMAC-Streebog DRBG + メッセージハッシュ）。
- **ステータス**: リリース候補版のみ。必要な API を提供する正式版が無いため、監査容易性のためリポジトリ内にミラーし、安定版リリースを追跡中。
- **レビュー項目**:
  - `cargo test -p iroha_crypto --features gost`（`crates/iroha_crypto/tests/gost_wycheproof.rs`）で Wycheproof／TC26 フィクスチャに対するハッシュ出力を検証。
  - `cargo bench -p iroha_crypto --bench gost_sign --features gost` で Ed25519/Secp256k1 と各 TC26 曲線を測定。
  - `cargo run -p iroha_crypto --bin gost_perf_check --features gost` により最新測定値をベースラインと比較（CI は `--summary-only`、再ベースラインは `--write-baseline crates/iroha_crypto/benches/gost_perf_baseline.json`）。
  - `scripts/gost_bench.sh` が上記ベンチ＋検証をラップ。`--write-baseline` 指定で JSON 更新。ワークフロー詳細は `docs/source/crypto/gost_performance.md` を参照。
- **緩和策**: `streebog` はゼロ化を行う決定論的ラッパー経由でのみ呼び出され、署名器は OS エントロピによるノンスヘッジで RNG 障害を軽減。
- **次のアクション**: RustCrypto が streebog `0.11.x` 正式版をタグ付けしたら通常の依存関係更新として扱い、チェックサム検証・差分レビュー・プロビナンス記録を実施した上でベンダー内ミラーを削除する。
