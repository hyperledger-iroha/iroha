---
lang: ja
direction: ltr
source: docs/source/crypto/sm_rustcrypto_spike.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1f133d9489c4bcfae2212e6c5dc098f39c3dea3e5cd42855ba76e8c9b73b4d03
source_last_modified: "2026-01-03T18:07:57.103009+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! RustCrypto SM 統合のスパイクに関するメモ。

# RustCrypto SM スパイクノート

## 目的
RustCrypto の `sm2`、`sm3`、および `sm4` クレート (および `rfc6979`、`ccm`、`gcm`) をオプションの依存関係として導入すると、 `iroha_crypto` クレートを作成し、機能フラグをより広いワークスペースに配線する前に、許容可能なビルド時間を生成します。

## 提案された依存関係マップ

|木箱 |推奨バージョン |特長 |メモ |
|----------|--------|----------|----------|
| `sm2` | `0.13` (RustCrypto/署名) | `std` | `elliptic-curve` に依存します。 MSRV がワークスペースと一致することを確認します。 |
| `sm3` | `0.5.0-rc.1` (RustCrypto/ハッシュ) |デフォルト | API は `sha2` と同等であり、既存の `digest` 特性と統合されています。 |
| `sm4` | `0.5.1` (RustCrypto/ブロック暗号) |デフォルト |暗号特性を使用して動作します。 AEAD ラッパーは後のスパイクに延期されました。 |
| `rfc6979` | `0.4` |デフォルト |決定論的なナンス導出のために再利用します。 |

*バージョンは 2024 年 12 月時点の現在のリリースを反映しています。着陸前に `cargo search` で確認してください。*

## マニフェストの変更 (草案)

```toml
[features]
sm = ["dep:sm2", "dep:sm3", "dep:sm4", "dep:rfc6979"]

[dependencies]
sm2 = { version = "0.13", optional = true, default-features = false, features = ["std"] }
sm3 = { version = "0.5.0-rc.1", optional = true }
sm4 = { version = "0.5.1", optional = true }
rfc6979 = { version = "0.4", optional = true, default-features = false }
```

フォローアップ: `iroha_crypto` (現在は `0.13.8`) にあるバージョンと一致するように `elliptic-curve` をピン留めします。

## スパイクチェックリスト
- [x] オプションの依存関係と機能を `crates/iroha_crypto/Cargo.toml` に追加します。
- [x] 配線を確認するために、プレースホルダー構造体を使用して `cfg(feature = "sm")` の後ろに `signature::sm` モジュールを作成します。
- [x] `cargo check -p iroha_crypto --features sm` を実行してコンパイルを確認します。ビルド時間と新しい依存関係の数を記録します (`cargo tree --features sm`)。
- [x] `cargo check -p iroha_crypto --features sm --locked` で std-only ポスチャを確認します。 `no_std` ビルドはサポートされなくなりました。
- [x] `docs/source/crypto/sm_program.md` のファイル結果 (タイミング、依存関係ツリー デルタ)。

## キャプチャする観測値
- ベースラインと比較して追加のコンパイル時間。
- `cargo builtinsize` によるバイナリ サイズの影響 (測定可能な場合)。
- MSRV または機能の競合 (`elliptic-curve` マイナー バージョンなど)。
- 上流のパッチが必要となる可能性がある警告 (安全でないコード、const-fn ゲート) が生成されます。

## 保留中のアイテム
- ワークスペースの依存関係グラフを拡張する前に、Crypto WG の承認を待ちます。
- レビューのためにクレートをベンダーに提供するか、crates.io に依存するかを確認します (ミラーが必要な場合があります)。
- チェックリストに完了のマークを付ける前に、`sm_lock_refresh_plan.md` ごとに `Cargo.lock` の更新を調整します。
- 承認が得られたら、`scripts/sm_lock_refresh.sh` を使用してロックファイルと依存関係ツリーを再生成します。

## 2025-01-19 スパイクログ
- オプションの依存関係 (`sm2 0.13`、`sm3 0.5.0-rc.1`、`sm4 0.5.1`、`rfc6979 0.4`) および `sm` 機能フラグを `iroha_crypto` に追加しました。
- コンパイル中にハッシュ/ブロック暗号 API を実行するためにスタブ化された `signature::sm` モジュール。
- `cargo check -p iroha_crypto --features sm --locked` は依存関係グラフを解決するようになりましたが、`Cargo.lock` 更新要件により中止されます。リポジトリ ポリシーではロックファイルの編集が禁止されているため、許可されたロックの更新を調整するまでコンパイルの実行は保留されたままになります。## 2026-02-12 スパイクログ
- 以前のロックファイル ブロッカーを解決しました (依存関係はすでにキャプチャされています)。そのため、`cargo check -p iroha_crypto --features sm --locked` は成功します (開発 Mac でのコールド ビルド 7.9 秒、増分再実行 0.23 秒)。
- `cargo check -p iroha_crypto --no-default-features --features "std sm" --locked` は 1.0 秒でパスし、オプション機能が `std` のみの構成でコンパイルされることを確認します (`no_std` パスは残りません)。
- `sm` 機能が有効になっている依存関係デルタでは、11 個のクレートが導入されます: `base64ct`、`ghash`、`opaque-debug`、`pem-rfc7468`、`pkcs8`、`polyval`、 `primeorder`、`sm2`、`sm3`、`sm4`、および `sm4-gcm`。 (`rfc6979` はすでにベースライン グラフの一部でした。)
- 未使用の NEON ポリシー ヘルパーに対してビルド警告が継続します。メータリング スムージング ランタイムがこれらのコード パスを再度有効にするまで、そのままにしておきます。