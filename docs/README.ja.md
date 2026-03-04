# Iroha ドキュメント（日本語ガイド）

このリポジトリからは同じコードベースでビルドされる 2 つのリリースラインが提供されます。
**Iroha 2** はセルフホスト型の許可制／コンソーシアムネットワーク向け、
**Iroha 3（SORA Nexus）** はマルチレーン・データスペースによる単一のグローバル台帳です。
両者とも同一の Iroha Virtual Machine（IVM）と Kotodama ツールチェーンを共有しているため、
コントラクトやバイトコードは再コンパイルなしでどちらのネットワークでも動作します。
英語版の詳細は [`README.md`](./README.md) を参照してください。

## オンラインドキュメント

公式ドキュメントサイト <https://docs.iroha.tech/> では次の情報が提供されています。

- [クイックスタート](https://docs.iroha.tech/get-started/)
- Rust／Python／JavaScript／Java／Kotlin 向け [SDK チュートリアル](https://docs.iroha.tech/guide/tutorials/)
- [Torii API リファレンス](https://docs.iroha.tech/reference/torii-endpoints.html)

補助資料:

- [Iroha 2 ホワイトペーパー](./source/iroha_2_whitepaper.md) — セルフホスト型ネットワークの仕様
- [Iroha 3（SORA Nexus）ホワイトペーパー](./source/iroha_3_whitepaper.md) — Nexus のマルチレーン／データスペース設計
- [データモデルと ISI 仕様（実装準拠）](./source/data_model_and_isi_spec.md) — コードベースから逆算した挙動の記録
- [ZK エンベロープ (Norito)](./source/zk_envelopes.md) — Norito ベースの添付データと検証要件

## ツール関連ドキュメント

リポジトリには以下のツールドキュメントがあります。

- [Kagami](../crates/iroha_kagami/README.md)
- [`iroha_derive`](../crates/iroha_derive/) — 設定定義用マクロ（`config_base` 機能を参照）
- [プロファイルビルド手順](./profile_build.md) — `iroha_data_model` のボトルネック特定に役立つ設定

## Swift / iOS SDK 関連ドキュメント

- [Swift SDK 概要](./source/sdk/swift/index.md) — パイプラインヘルパーやハードウェアアクセラレーション、Connect/WebSocket API の指針。
- [Connect クイックスタート](./connect_swift_ios.ja.md) — SDK を使った手順と CryptoKit 参考実装をまとめたガイド。
- [Xcode 統合ガイド](./connect_swift_integration.ja.md) — NoritoBridgeKit／Connect をアプリに組み込む具体的な手順（ChaChaPoly やフレーム処理を含む）。
- [SwiftUI デモ貢献者ガイド](./norito_demo_contributor.ja.md) — ローカル Torii ノードと連携した iOS デモの実行方法や加速設定。
- Swift 関連の変更を公開する前に `make swift-ci` を実行し、フィクスチャ整合性・ダッシュボード検証・Buildkite の `ci/xcframework-smoke:<lane>:device_tag` メタデータが揃っていることを確認してください。

## Norito（シリアライゼーションコーデック）

Norito はワークスペースで使用するシリアライゼーションコーデックです。SCALE（`parity-scale-codec`）は使用しません。SCALE との比較はあくまで性能比較や参考情報に留まります。運用経路では常に Norito が利用されます。

`norito::codec::{Encode, Decode}` API はヘッダレス（いわゆる "bare"）形式の Norito ペイロードを提供し、ハッシュ計算や効率的なワイヤフォーマットに利用します。

最新の Norito 実装ポイント:

- 16 バイトのスキーマ ID や圧縮フラグなどを含む固定ヘッダで決定論的にエンコード／デコード
- CRC64-XZ チェックサムを採用し、PCLMULQDQ（x86_64）や PMULL（AArch64）によるハードウェアアクセラレーションを自動選択
- 64 KiB のストリーミングバッファと、デコード時の逐次 CRC 更新
- zstd 圧縮をサポート（GPU オフロードは機能フラグで有効化し、常に決定論的なフォールバックを保持）
- `norito::to_bytes_auto(&T)` がペイロードサイズとハードウェア能力に基づき、非圧縮／CPU zstd／GPU zstd を選択（ヘッダの `compression` バイトのみが変化し、意味論は不変）

詳細およびベンチマークは `crates/norito/README.md` を参照してください。

## ローカリゼーション

英語のソースドキュメントと同じ場所に `*.ja.*` および `*.he.*` 形式で翻訳スタブを配置しています。生成・更新手順や新しい言語の追加方法は [`docs/i18n/README.ja.md`](./i18n/README.ja.md) にまとめています。

## ステータスエンドポイントのフォーマット

- Torii `/status` エンドポイントはデフォルトで Norito（ヘッダレス）のレスポンスを返します。クライアントはまず Norito デコードを試行してください。
- `Content-Type: application/json` が設定されている場合、または JSON を要求した場合は JSON にフォールバックします。
- ワイヤフォーマットはあくまで Norito であり、SCALE ではありません。

## ドキュメントの進捗

IVM や ZK 周辺の文書の多くは翻訳・校正済みです。残りの更新中セクションについてはメイン英語版を参照し、差異を見つけた場合は Issue/PR で知らせてください。

英語版と内容が乖離している箇所を見つけた場合は Issue や Pull Request で報告してください。
