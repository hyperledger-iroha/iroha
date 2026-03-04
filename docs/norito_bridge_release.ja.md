<!-- Japanese translation of docs/norito_bridge_release.md -->

---
lang: ja
direction: ltr
source: docs/norito_bridge_release.md
status: complete
translator: manual
---

# NoritoBridge リリースパッケージング手順

このガイドでは、`NoritoBridge` Swift バインディングを Swift Package Manager と CocoaPods から利用できる XCFramework として公開する手順を解説します。Iroha の Norito コーデックを提供する Rust クレートのリリースと歩調を合わせる形で Swift 成果物を更新するのがポイントです。アプリ側での統合手順（Xcode プロジェクト設定や ChaChaPoly、ConnectSession の利用方法など）は `docs/connect_swift_integration.md` を参照してください。

> **補足:** Apple ツールチェーンを備えた macOS CI ビルダーが整備され次第
> （Release Engineering の macOS ビルダーバックログで追跡中）、このフローは自動化される予定です。
> それまでは開発用 Mac 上で以下の手順を手動で実施してください。

## 前提条件

- 最新の安定版 Xcode コマンドラインツールをインストールした macOS ホスト
- ワークスペースの `rust-toolchain.toml` に一致する Rust ツールチェーン
- Swift 5.7 以降
- CocoaPods（中央リポジトリへ公開する場合は rubygems 経由でインストール）
- Swift 成果物にタグを付与するための Hyperledger Iroha リリース署名鍵へのアクセス

## バージョニングモデル

1. Norito コーデックの Rust クレート（`crates/norito/Cargo.toml`）で公開バージョンを確認する。
2. ワークスペースにリリースタグ（例: `v2.1.0`）を付与する。
3. Swift パッケージおよび CocoaPods の podspec でも同じセマンティックバージョンを使用する。
4. Rust 側のバージョンが上がった場合は、Swift 成果物も同バージョンで再発行する。テスト段階では `-alpha.1` のようなメタデータ付きバージョンも利用可能。

## ビルド手順

1. リポジトリルートでヘルパースクリプトを実行し、XCFramework を組み立てます:

   ```bash
   ./scripts/build_norito_xcframework.sh --workspace-root "$(pwd)" \
       --output "artifacts/NoritoBridge.xcframework" \
       --profile release
   ```

   このスクリプトは iOS / macOS 向けに Rust ブリッジライブラリをビルドし、一つの XCFramework ディレクトリにバンドルします。

2. 配布用に XCFramework を zip 化します:

   ```bash
   ditto -c -k --sequesterRsrc --keepParent \
     artifacts/NoritoBridge.xcframework \
     artifacts/NoritoBridge.xcframework.zip
   ```

3. Swift パッケージマニフェスト（`IrohaSwift/Package.swift`）のバイナリターゲットに新しいバージョンとチェックサムを反映します:

   ```bash
   swift package compute-checksum artifacts/NoritoBridge.xcframework.zip
   ```

   得られたチェックサムを `Package.swift` に記録してください。

4. `IrohaSwift/IrohaSwift.podspec` も同じバージョン・チェックサム・アーカイブ URL に更新します。

5. タグ付け前に Swift の検証スイートを実行します:

   ```bash
   swift test --package-path IrohaSwift
   make swift-ci
   ```

   1 行目は Swift パッケージ（`AccelerationSettings` 含む）が成功することを確認し、2 行目はフィクスチャパリティとダッシュボード検証を行います。CI では `ci/xcframework-smoke:<lane>:device_tag` 形式の Buildkite メタデータに依存するため、パイプラインやエージェントを変更した際はメタデータが出力されていることを確認してください。

6. 生成物をリリースブランチにコミットし、該当コミットへタグを付けます。

## 公開手順

### Swift Package Manager

- 公開リポジトリへタグをプッシュする。
- Apple 公式またはコミュニティのパッケージインデックスからタグが参照できることを確認する。
- 利用者は `.package(url: "https://github.com/hyperledger/iroha", from: "<version>")` で依存追加可能。

### CocoaPods

1. ローカルで podspec を検証:

   ```bash
   pod lib lint IrohaSwift.podspec --allow-warnings
   ```

2. podspec を公開:

   ```bash
   pod trunk push IrohaSwift.podspec
   ```

3. CocoaPods インデックスに新バージョンが反映されたことを確認する。

## CI での留意点

- macOS ジョブでパッケージングスクリプトを実行し、成果物をアーカイブしてチェックサムをワークフロー出力として公開する。
- Swift デモアプリが生成したフレームワークでビルドできることをリリースゲートに設定する。
- ビルドログを保存し、トラブルシューティングを容易にする。

## さらなる自動化アイデア

- 必要なターゲットが揃い次第、`xcodebuild -create-xcframework` を直接利用する。
- 開発マシン以外で配布する場合に備え、署名やノータライゼーションを組み込む。
- SPM 依存をリリースタグに固定することで、統合テストを常に最新のパッケージに合わせる。
