---
lang: ja
direction: ltr
source: CHANGELOG.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 26f5115a14476de15fbc8f26c5a9807954df6884763a818b2bc98ec6cfe1a4cc
source_last_modified: "2026-01-04T13:46:50.705991+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# 変更ログ

[Unreleased]: https://github.com/hyperledger-iroha/iroha/compare/v2.0.0-rc.2.0...HEAD
[2.0.0-rc.2.0]: https://github.com/hyperledger-iroha/iroha/releases/tag/v2.0.0-rc.2.0

このプロジェクトに対するすべての重要な変更は、このファイルに文書化されます。

## [未公開]- SCALE シムを取り外します。 `norito::codec` はネイティブ Norito シリアル化で実装されるようになりました。
- クレート全体で `parity_scale_codec` の使用を `norito::codec` に置き換えます。
- ツールをネイティブ Norito シリアル化に移行し始めます。
- ネイティブ Norito シリアル化を優先して、残りの `parity-scale-codec` 依存関係をワークスペースから削除します。
- 残りの SCALE 特性派生をネイティブ Norito 実装に置き換え、バージョン管理されたコーデック モジュールの名前を変更します。
- 機能ゲート マクロを使用して、`iroha_config_base_derive` と `iroha_futures_derive` を `iroha_derive` にマージします。
- *(マルチシグ)* 安定したエラー コード/理由でマルチシグ機関からの直接署名を拒否し、ネストされたリレーラー全体でマルチシグ TTL キャップを強制し、送信前に CLI で TTL キャップを表示します (SDK パリティ保留中)。
- FFI プロシージャ マクロを `iroha_ffi` に移動し、`iroha_ffi_derive` クレートを削除します。
- *(schema_g​​en)* 不要な `transparent_api` 機能を `iroha_data_model` 依存関係から削除します。
- *(data_model)* `Name` 解析用の ICU NFC ノーマライザーをキャッシュして、繰り返しの初期化オーバーヘッドを削減します。
- 📚 Torii クライアントの JS クイックスタート、構成リゾルバー、公開ワークフロー、および構成対応レシピを文書化します。
- *(IrohaSwift)* 最小展開ターゲットを iOS 15 / macOS 12 に引き上げ、Torii クライアント API 全体で Swift 同時実行性を採用し、パブリック モデルを `Sendable` としてマークします。
- *(IrohaSwift)* `ToriiDaProofSummaryArtifact` と `DaProofSummaryArtifactEmitter.emit` を追加したため、Swift アプリは CLI にシェルアウトせずに CLI 互換の DA 証明バンドルを構築/発行でき、メモリ内とディスク上の両方をカバーするドキュメントと回帰テストが完了します。 workflows.【F:IrohaSwift/Sources/IrohaSwift/ToriiDaProofsummaryArtifact.swift:1】【F:IrohaSwift/Tests/IrohaSwiftTests/ToriiDaProofsummaryArtifactTests.swift:1】【F:docs/source/sdk/swift/index.md:260】
- *(data_model/js_host)* `KaigiParticipantCommitment` からアーカイブ再利用フラグを削除して Kaigi オプションのシリアル化を修正し、ネイティブ ラウンドトリップ テストを追加し、JS デコード フォールバックを削除して、Kaigi 命令が前に Norito ラウンドトリップするようになりました。 【F:crates/iroha_data_model/src/kaigi.rs:128】【F:crates/iroha_js_host/src/lib.rs:1379】【F:javascript/iroha_js/test/instructionBuilders.test.js:30】
- *(javascript)* `ToriiClient` 呼び出し元がデフォルトのヘッダーを削除できるようにします (`null` を渡すことで)。これにより、`getMetrics` は JSON と Prometheus テキストをきれいに切り替えます。 headers.【F:javascript/iroha_js/src/toriiClient.js:488】【F:javascript/iroha_js/src/toriiClient.js:761】
- *(javascript)* NFT、アカウントごとの資産残高、資産定義ホルダー (TypeScript の定義、ドキュメント、テストを含む) 用の反復可能なヘルパーを追加したため、Torii ページネーションが残りのアプリをカバーするようになりました。エンドポイント.【F:javascript/iroha_js/src/toriiClient.js:105】【F:javascript/iroha_js/index.d.ts:80】【F:javascript/iroha_js/test/toriiClient.test.js:365】【F:javascript/iroha_js/README.md:470】
- *(javascript)* ガバナンス命令/トランザクション ビルダーとガバナンス レシピを追加したため、JS クライアントは段階的に提案、投票、制定、議会の永続化を段階的に展開できるようになります。 【F:javascript/iroha_js/src/instructionBuilders.js:1012】【F:javascript/iroha_js/src/transaction.js:1082】【F:javascript/iroha_js/recipes/governance.mjs:1】
- *(javascript)* ISO 20022 pacs.008 送信/ステータス ヘルパーと一致するレシピを追加し、JS 呼び出し元が特注の HTTP なしで Torii ISO ブリッジを実行できるようにします。配管.【F:javascript/iroha_js/src/toriiClient.js:888】【F:javascript/iroha_js/index.d.ts:706】【F:javascript/iroha_js/recipes/iso_bridge.mjs:1】- *(javascript)* pacs.008/pacs.009 ビルダー ヘルパーと構成主導のレシピを追加したため、JS 呼び出し元は、検証された BIC/IBAN メタデータを使用して ISO 20022 ペイロードを合成してから、 Bridge.【F:javascript/iroha_js/src/isoBridge.js:1】【F:javascript/iroha_js/test/isoBridge.test.js:1】【F:javascript/iroha_js/recipes/iso_bridge_builder.mjs:1】【F:javascript/iroha_js/index.d.ts:1】
- *(javascript)* DA の取り込み/フェッチ/証明ループが完了しました: `ToriiClient.fetchDaPayloadViaGateway` はチャンカー ハンドルを (新しい `deriveDaChunkerHandle` バインディング経由で) 自動導出するようになり、オプションの証明サマリーはネイティブ `generateDaProofSummary` を再利用し、SDK 呼び出し元がミラーリングできるように README/型指定/テストが更新されました。 `iroha da get-blob/prove-availability` ビスポークなし配管.【F:javascript/iroha_js/src/toriiClient.js:1123】【F:javascript/iroha_js/src/dataAvailability.js:1】【F:javascrip】 t/iroha_js/test/toriiClient.test.js:1454]【F:javascript/iroha_js/index.d.ts:3275】【F:javascript/iroha_js/README.md:760】
- *(javascript/js_host)* `sorafsGatewayFetch` スコアボード メタデータは、ゲートウェイ プロバイダーが使用されるたびにゲートウェイ マニフェスト ID/CID を記録するようになりました。これにより、導入アーティファクトは CLI キャプチャと一致します。【F:crates/iroha_js_host/src/lib.rs:3017】【F:docs/source/sorafs_orchestrator_rollout.md:23】
- *(torii/cli)* ISO クロスウォークの強制: Torii は不明なエージェント BIC を持つ `pacs.008` 送信を拒否し、DvP CLI プレビューは経由で `--delivery-instrument-id` を検証します。 `--iso-reference-crosswalk`.【F:crates/iroha_torii/src/iso20022_bridge.rs:704】【F:crates/iroha_cli/src/main.rs:3892】
- *(torii)* `POST /v1/iso20022/pacs009` 経由の PvP キャッシュ インジェストを追加し、転送を構築する前に `Purp=SECU` と BIC 参照データ チェックを強制します。【F:crates/iroha_torii/src/iso20022_bridge.rs:1070】【F:crates/iroha_torii/src/lib.rs:4759】
- *(ツール)* リポジトリ フィクスチャと一緒に ISIN/CUSIP、BIC↔LEI、および MIC スナップショットを検証するために `cargo xtask iso-bridge-lint` (および `ci/check_iso_reference_data.sh`) を追加しました。【F:xtask/src/main.rs:146】【F:ci/check_iso_reference_data.sh:1】
- *(javascript)* リポジトリ メタデータ、明示的なファイル許可リスト、来歴対応 `publishConfig`、`prepublishOnly` 変更ログ/テスト ガード、およびノード 18/20 を実行する GitHub Actions ワークフローを宣言することにより、強化された npm 公開CI【F:javascript/iroha_js/package.json:1】【F:javascript/iroha_js/scripts/check-changelog.mjs:1】【F:docs/source/sdk/js/publishing.md:1】【F:.github/workflows/javascript-sdk.yml:1】
- *(ivm/cuda)* BN254 フィールドの add/sub/mul は、`bn254_launch_kernel` を介したホスト側のバッチ処理を使用して新しい CUDA カーネル上で実行されるようになり、確定性を維持しながら Poseidon および ZK ガジェットのハードウェア アクセラレーションが可能になります。フォールバック。【F:crates/ivm/cuda/bn254.cu:1】【F:crates/ivm/src/cuda.rs:66】【F:crates/ivm/src/cuda.rs:1244】

## [2.0.0-rc.2.0] - 2025-05-08

### 🚀 機能

- *(cli)* `iroha transaction get` およびその他の重要なコマンドを追加 (#5289)
- [**破壊的**] 代替可能な資産と代替不可能な資産を分離する (#5308)
- [**breaking**] 空でないブロックの後に空のブロックを許可することで、空でないブロックを終了します (#5320)
- スキーマとクライアントでテレメトリ タイプを公開する (#5387)
- *(iroha_torii)* 機能ゲート型エンドポイントのスタブ (#5385)
- コミット時間メトリクスを追加 (#5380)

### 🐛 バグ修正

- NonZeros の改訂 (#5278)
- ドキュメント ファイルのタイプミス (#5309)
- *(crypto)* `Signature::payload` ゲッターを公開する (#5302) (#5310)
- *(core)* ロールを付与する前にロールの存在のチェックを追加します (#5300)
- *(core)* 切断されたピアを再接続します (#5325)
- ストアアセットとNFTに関連するpytestを修正(#5341)
- *(CI)* 詩 v2 の Python 静的分析ワークフローを修正 (#5374)
- コミット後に期限切れトランザクションイベントが表示される (#5396)

### 💼その他- `rust-toolchain.toml` (#5376) を含める
- `deny` ではなく、`unused` で警告する (#5377)

### 🚜 リファクタリング

- アンブレラ Iroha CLI (#5282)
- *(iroha_test_network)* ログにきれいな形式を使用する (#5331)
- [**重大**] `genesis.json` での `NumericSpec` のシリアル化を簡素化 (#5340)
- 失敗した P2P 接続のログを改善しました (#5379)
- `logger.level` を元に戻し、`logger.filter` を追加し、設定ルートを拡張します (#5384)

### 📚 ドキュメント

- `network.public_address` を `peer.template.toml` に追加 (#5321)

### ⚡ パフォーマンス

- *(蔵)* ディスクへの重複ブロック書き込みを防止 (#5373)
- トランザクション ハッシュ用のカスタム ストレージを実装しました (#5405)

### ⚙️ その他のタスク

- 詩の使用を修正 (#5285)
- `iroha_torii_const` から冗長な const を削除 (#5322)
- 未使用の `AssetEvent::Metadata*` (#5339) を削除します。
- バンプソナーキューブアクションバージョン (#5337)
- 未使用の権限を削除する (#5346)
- unzip パッケージを ci-image に追加 (#5347)
- いくつかのコメントを修正 (#5397)
- 統合テストを `iroha` クレートから移動 (#5393)
-defectdojo ジョブを無効にする (#5406)
- 欠落したコミットに対する DCO サインオフを追加
- ワークフローの再編成 (2 回目) (#5399)
- メインへのプッシュ時にプルリクエスト CI を実行しない (#5415)

<!-- generated by git-cliff -->

## [2.0.0-rc.1.3] - 2025-03-07

### 追加

- 空ではないブロックの後に空のブロックを許可することで、空ではないブロックを終了します (#5320)

## [2.0.0-rc.1.2] - 2025-02-25

### 修正済み

- 再登録されたピアがピアリストに正しく反映されるようになりました (#5327)

## [2.0.0-rc.1.1] - 2025-02-12

### 追加

- `iroha transaction get` およびその他の重要なコマンドを追加 (#5289)

## [2.0.0-rc.1.0] - 2024-12-06

### 追加

- クエリプロジェクションを実装する (#5242)
- 永続的なエグゼキューターを使用する (#5082)
- iroha cli にリッスンタイムアウトを追加 (#5241)
- /peers API エンドポイントをtorii に追加 (#5235)
- アドレスに依存しない p2p (#5176)
- マルチシグユーティリティと使いやすさを改善 (#5027)
- `BasicAuth::password` が印刷されないように保護する (#5195)
- `FindTransactions` クエリでの降順ソート (#5190)
- すべてのスマート コントラクト実行コンテキストにブロック ヘッダーを導入する (#5151)
- ビュー変更インデックスに基づく動的コミット時間 (#4957)
- デフォルトの権限セットを定義する (#5075)
- `Option<Box<R>>` の Niche の実装を追加 (#5094)
- トランザクション述語とブロック述語 (#5025)
- クエリ内の残りアイテムの量をレポートする (#5016)
- 有界離散時間 (#4928)
- 不足している数学演算を `Numeric` に追加 (#4976)
- ブロック同期メッセージを検証する (#4965)
- クエリフィルター (#4833)

### 変更されました

- ピア ID の解析を簡素化 (#5228)
- トランザクション エラーをブロック ペイロードから移動 (#5118)
- JsonString の名前を Json に変更します (#5154)
- クライアントエンティティをスマートコントラクトに追加 (#5073)
- トランザクション注文サービスとしてのリーダー (#4967)
- 蔵がメモリから古いブロックを削除するようにします (#5103)
- `Executable` の命令には `ConstVec` を使用します (#5096)
- ゴシップ TXS は最大 1 回まで (#5079)
- `CommittedTransaction` のメモリ使用量を削減 (#5089)
- クエリカーソルエラーをより具体的にする (#5086)
- 木箱を再編成する (#4970)
- `FindTriggers` クエリを導入し、`FindTriggerById` を削除 (#5040)
- アップデートの署名に依存しない (#5039)
- Genesis.json のパラメータ形式を変更 (#5020)
- 現在および以前のビュー変更証明のみを送信する (#4929)
- ビジーループを防ぐため、準備ができていないときのメッセージ送信を無効にする (#5032)
- 資産の合計数量を資産定義に移動 (#5029)
- ペイロード全体ではなく、ブロックのヘッダーのみに署名します (#5000)
- ブロック ハッシュのタイプとして `HashOf<BlockHeader>` を使用します (#4998)
- `/health` および `/api_version` を簡略化します (#4960)
- `configs` の名前を `defaults` に変更し、`swarm` を削除 (#4862)

### 修正済み- JSON の内部ロールをフラット化 (#5198)
- `cargo audit` 警告を修正 (#5183)
- 署名インデックスに範囲チェックを追加 (#5157)
- ドキュメントのモデル マクロの例を修正 (#5149)
- ブロック/イベントストリームでWSを適切に閉じる (#5101)
- 壊れた信頼できるピアチェック (#5121)
- 次のブロックの高さが +1 であることを確認します (#5111)
- ジェネシスブロックのタイムスタンプを修正 (#5098)
- `transparent_api` 機能を使用しない `iroha_genesis` コンパイルを修正 (#5056)
- `replace_top_block` を正しく処理する (#4870)
- エグゼキューターのクローン作成を修正 (#4955)
- エラーの詳細を表示 (#4973)
- ブロックストリームに `GET` を使用 (#4990)
- キューのトランザクション処理を改善しました (#4947)
- 冗長な blocksync ブロック メッセージを防止 (#4909)
- 大きなメッセージの同時送信時のデッドロックを防止 (#4948)
- 期限切れのトランザクションをキャッシュから削除 (#4922)
- パス付きの鳥居の URL を修正 (#4903)

### 削除されました

- クライアントからモジュールベースの API を削除 (#5184)
- `riffle_iter` (#5181) を削除
- 未使用の依存関係を削除 (#5173)
- `blocks_in_memory` から `max` プレフィックスを削除 (#5145)
- コンセンサス推定を削除 (#5116)
- ブロックから `event_recommendations` を削除 (#4932)

### セキュリティ

## [2.0.0-pre-rc.22.1] - 2024-07-30

### 修正済み

- `jq` を Docker イメージに追加しました

## [2.0.0-pre-rc.22.0] - 2024-07-25

### 追加

- ジェネシスでオンチェーンパラメータを明示的に指定 (#4812)
- 複数の `Instruction` を持つ Turbofish を許可 (#4805)
- マルチシグネチャトランザクションを再実装する (#4788)
- 組み込みパラメータとカスタムオンチェーンパラメータを実装 (#4731)
- カスタム命令の使用を改善 (#4778)
- JsonString を実装することでメタデータを動的にします (#4732)
- 複数のピアがジェネシスブロックを送信できるようにする (#4775)
- `SignedTransaction` の代わりに `SignedBlock` をピアに供給します (#4739)
- エグゼキューターのカスタム命令 (#4645)
- クライアント CLI を拡張して JSON クエリをリクエストする (#4684)
 - `norito_decoder` の検出サポートを追加 (#4680)
- パーミッションスキーマをエグゼキューターデータモデルに一般化 (#4658)
- デフォルトのエグゼキュータに登録トリガー権限を追加しました (#4616)
 - `norito_cli` で JSON をサポート
- P2P アイドル タイムアウトの導入

### 変更されました

- `lol_alloc` を `dlmalloc` に置き換えます (#4857)
- スキーマ内の `type_` の名前を `type` に変更 (#4855)
- スキーマ内の `Duration` を `u64` に置き換えます (#4841)
- ロギングに `RUST_LOG` のような EnvFilter を使用する (#4837)
- 可能な場合は投票ブロックを維持する (#4828)
- ワープからアクサムに移行 (#4718)
- 分割エグゼキューター データ モデル (#4791)
- 浅いデータモデル (#4734) (#4792)
- 署名付きの公開キーを送信しない (#4518)
- `--outfile` の名前を `--out-file` に変更 (#4679)
- irohaサーバーとクライアントの名前を変更 (#4662)
- `PermissionToken` の名前を `Permission` に変更 (#4635)
- `BlockMessages` を積極的に拒否する (#4606)
- `SignedBlock` を不変にする (#4620)
- TransactionValue の名前を CommittedTransaction に変更 (#4610)
- ID による個人アカウントの認証 (#4411)
- 秘密鍵にマルチハッシュ形式を使用する (#4541)
 - `parity_scale_decoder` の名前を `norito_cli` に変更します
- ブロックをセット B バリデーターに送信します
- `Role` を透明にする (#4886)
- ヘッダーからブロックハッシュを導出する (#4890)

### 修正済み- 権限が移管するドメインを所有していることを確認する (#4807)
- ロガーの二重初期化を削除 (#4800)
- アセットと権限の命名規則を修正 (#4741)
- Genesis ブロック内の別のトランザクションでエグゼキュータをアップグレードする (#4757)
- `JsonString` の正しいデフォルト値 (#4692)
- 逆シリアル化エラー メッセージを改善しました (#4659)
- 渡された Ed25519Sha512 公開キーの長さが無効でもパニックにならないでください (#4650)
- init ブロックのロード時に適切なビュー変更インデックスを使用する (#4612)
- `start` タイムスタンプより前にタイムトリガーを時期尚早に実行しないでください (#4333)
- `torii_url` (#4601) (#4617) の `https` をサポート
- SetKeyValue/RemoveKeyValue から serde( flatten) を削除 (#4547)
- トリガーセットが正しくシリアル化されている
- `Upgrade<Executor>` で削除された `PermissionToken` を取り消します (#4503)
- 現在のラウンドの正しいビュー変更インデックスをレポートします
- `Unregister<Domain>` の対応するトリガーを削除 (#4461)
- ジェネシスラウンドでジェネシス公開キーを確認する
- 生成ドメインまたはアカウントの登録を防止します
- エンティティの登録解除時にロールから権限を削除します
- トリガーのメタデータはスマート コントラクトでアクセス可能
- 一貫性のない状態ビューを防ぐために rw ロックを使用します (#4867)
- スナップショットでソフト フォークを処理する (#4868)
- ChaCha20Poly1305のMinSizeを修正
- メモリ使用量の増加を防ぐために LiveQueryStore に制限を追加 (#4893)

### 削除されました

- ed25519 秘密鍵から公開鍵を削除 (#4856)
- Kura.lock を削除 (#4849)
- 設定内の `_ms` および `_bytes` サフィックスを元に戻す (#4667)
- Genesis フィールドから `_id` および `_file` サフィックスを削除 (#4724)
- AssetDefinitionId による AssetsMap のインデックス アセットの削除 (#4701)
- トリガー ID からドメインを削除 (#4640)
- Iroha からジェネシス署名を削除 (#4673)
- `Validate` からバインドされた `Visit` を削除 (#4642)
- `TriggeringEventFilterBox` (#4866) を削除
- p2p ハンドシェイクで `garbage` を削除 (#4889)
- ブロックから `committed_topology` を削除 (#4880)

### セキュリティ

- 機密漏洩を防ぐ

## [2.0.0-pre-rc.21] - 2024-04-19

### 追加

- トリガーのエントリポイントにトリガー ID を含める (#4391)
- スキーマ内のビットフィールドとして設定されたイベントを公開 (#4381)
- きめ細かなアクセスを備えた新しい `wsv` を導入 (#2664)
- `PermissionTokenSchemaUpdate`、`Configuration`、および `Executor` イベントのイベント フィルターを追加します。
- スナップショット「モード」の導入 (#4365)
- ロールの権限の付与/取り消しを許可 (#4244)
- アセットに任意精度の数値型を導入 (他のすべての数値型を削除) (#3660)
- Executor の異なる燃料制限 (#3354)
- pprof プロファイラーを統合 (#4250)
- クライアント CLI にアセット サブコマンドを追加 (#4200)
- `Register<AssetDefinition>` 権限 (#4049)
- リプレイ攻撃を防ぐために `chain_id` を追加 (#4185)
- クライアント CLI でドメイン メタデータを編集するサブコマンドを追加 (#4175)
- クライアント CLI でストアの設定、削除、取得の操作を実装 (#4163)
- トリガーの同一のスマート コントラクトをカウントする (#4133)
- ドメインを転送するためのサブコマンドをクライアント CLI に追加 (#3974)
- FFI でボックス化されたスライスをサポート (#4062)
- git SHA をクライアント CLI にコミット (#4042)
- デフォルトのバリデータボイラープレートの proc マクロ (#3856)
- クライアント API にクエリ リクエスト ビルダーを導入しました (#3124)
- スマート コントラクト内の遅延クエリ (#3929)
- `fetch_size` クエリパラメータ (#3900)
- アセットストア転送命令 (#4258)
- 機密漏洩を防ぐ (#3240)
- 同じソースコードでトリガーの重複を排除する (#4419)

### 変更されました- Rust ツールチェーンを夜間にバンプします-2024-04-18
- ブロックをセット B バリデーターに送信 (#4387)
- パイプライン イベントをブロック イベントとトランザクション イベントに分割する (#4366)
- `[telemetry.dev]` 設定セクションの名前を `[dev_telemetry]` に変更 (#4377)
- `Action` および `Filter` を非ジェネリック型にする (#4375)
- ビルダー パターンを使用してイベント フィルタリング API を改善しました (#3068)
- さまざまなイベントフィルター API を統合し、流暢なビルダー API を導入
- `FilterBox` の名前を `EventFilterBox` に変更します
- `TriggeringFilterBox` の名前を `TriggeringEventFilterBox` に変更します
- フィルターの命名を改善します。例: `AccountFilter` -> `AccountEventFilter`
- 構成 RFC (#4239) に従って構成を書き換えます
- バージョン管理された構造体の内部構造をパブリック API から非表示にする (#3887)
- ビューの変更があまりにも多く失敗した後、予測可能な順序を一時的に導入します (#4263)
- `iroha_crypto` で具象キー タイプを使用する (#4181)
- 通常のメッセージからの分割ビューの変更 (#4115)
- `SignedTransaction` を不変にする (#4162)
- `iroha_config` から `iroha_client` までエクスポート (#4147)
- `iroha_crypto` から `iroha_client` までエクスポート (#4149)
- `data_model` から `iroha_client` までエクスポート (#4081)
- `openssl-sys` の依存関係を `iroha_crypto` から削除し、構成可能な TLS バックエンドを `iroha_client` に導入します (#3422)
- メンテナンスされていない EOF `hyperledger/ursa` を社内ソリューション `iroha_crypto` に置き換えます (#3422)
- エグゼキュータのパフォーマンスを最適化する (#4013)
- トポロジーピアの更新 (#3995)

### 修正済み

- `Unregister<Domain>` の対応するトリガーを削除 (#4461)
- エンティティの登録解除時にロールから権限を削除 (#4242)
- ジェネシストランザクションがジェネシス公開鍵によって署名されていることをアサートします (#4253)
- p2p で応答しないピアのタイムアウトを導入 (#4267)
- 生成ドメインまたはアカウントの登録を防止する (#4226)
- `MinSize`、`ChaCha20Poly1305` (#4395)
- `tokio-console` が有効なときにコンソールを起動する (#4377)
- 各項目を `\n` で区切って、`dev-telemetry` ファイル ログの親ディレクトリを再帰的に作成します
- 署名なしのアカウント登録を防止 (#4212)
- キーペアの生成が確実になりました (#4283)
- `X25519` キーを `Ed25519` としてエンコードするのを停止 (#4174)
- `no_std` で署名検証を行う (#4270)
- 非同期コンテキスト内でのブロッキング メソッドの呼び出し (#4211)
- エンティティの登録解除時に関連付けられたトークンを取り消す (#3962)
- Sumeragi 起動時の非同期ブロックのバグ
- `(get|set)_config` 401 HTTP を修正 (#4177)
- Docker 内の `musl` アーカイバー名 (#4193)
- スマート コントラクトのデバッグ印刷 (#4178)
- 再起動時のトポロジ更新 (#4164)
- 新しいピアの登録 (#4142)
- オンチェーンで予測可能な反復順序 (#4130)
- ロガーと動的構成を再構築 (#4100)
- トリガーの原子性 (#4106)
- クエリ ストア メッセージの順序付けの問題 (#4057)
- Norito を使用して応答するエンドポイントには `Content-Type: application/x-norito` を設定します

### 削除されました

- `logger.tokio_console_address` 設定パラメータ (#4377)
- `NotificationEvent` (#4377)
- `Value` 列挙型 (#4305)
- iroha からの MST アグリゲーション (#4229)
- ISI のクローン作成とスマート コントラクトでのクエリ実行 (#4182)
- `bridge` および `dex` 機能 (#4152)
- フラット化されたイベント (#3068)
- 表現 (#4089)
- 自動生成された構成リファレンス
- `warp` ログのノイズ (#4097)

### セキュリティ

- P2P での公開キーのスプーフィングを防止 (#4065)
- OpenSSL から送信される `secp256k1` 署名が正規化されていることを確認します (#4155)

## [2.0.0-pre-rc.20] - 2023-10-17

### 追加- `Domain` の所有権を譲渡
- `Domain` 所有者の権限
- `owned_by` フィールドを `Domain` に追加
- `iroha_client_cli` の JSON5 としてフィルターを解析 (#3923)
- Serde の部分的にタグ付けされた列挙型での Self 型の使用のサポートを追加
- ブロックAPIの標準化 (#3884)
- `Fast` 蔵初期化モードを実装
- iroha_swarm 免責事項ヘッダーを追加
- WSV スナップショットの初期サポート

### 修正済み

- update_configs.sh でのエグゼキュータのダウンロードを修正 (#3990)
- devShellの適切なrustc
- 書き込み `Trigger` の再発行を修正
- 転送 `AssetDefinition` を修正
- `Domain` の `RemoveKeyValue` を修正
- `Span::join` の使用法を修正
- トポロジの不一致バグを修正 (#3903)
- `apply_blocks` および `validate_blocks` ベンチマークを修正
- `mkdir -r` (ロック パスではなくストア パスあり) (#3908)
- test_env.pyにdirが存在しても失敗しない
- 認証/認可のドキュメント文字列を修正 (#3876)
- クエリ検索エラーのエラー メッセージの改善
- Genesis アカウントの公開キーを開発 Docker Compose に追加
- パーミッション トークン ペイロードを JSON として比較 (#3855)
- `#[model]` マクロの `irrefutable_let_patterns` を修正
- Genesis による ISI の実行を許可 (#3850)
- ジェネシスの検証を修正 (#3844)
- 3 つ以下のピアのトポロジを修正
- tx_amounts ヒストグラムの計算方法を修正します。
- `genesis_transactions_are_validated()` 薄片性テスト
- デフォルトのバリデータの生成
- irohaの正常なシャットダウンを修正

### リファクタリング- 未使用の依存関係を削除 (#3992)
- 依存関係をバンプする (#3981)
- バリデーターの名前をエグゼキューターに変更 (#3976)
- `IsAssetDefinitionOwner` (#3979) を削除
- スマート コントラクト コードをワークスペースに含めます (#3944)
- API とテレメトリのエンドポイントを 1 つのサーバーに統合します
- 式 len をパブリック API からコアに移動 (#3949)
- ロールルックアップでのクローンの回避
- ロールの範囲クエリ
- アカウントの役割を `WSV` に移動
- ISI の名前を *Box から *Expr に変更 (#3930)
- バージョン管理されたコンテナから「バージョン管理」プレフィックスを削除 (#3913)
- `commit_topology` をブロック ペイロードに移動 (#3916)
- `telemetry_future` マクロを syn 2.0 に移行
- ISI 境界内で識別可能に登録 (#3925)
- 基本ジェネリックのサポートを `derive(HasOrigin)` に追加
- エミッター API ドキュメントをクリーンアップして、Clippy を満足させる
- 派生(HasOrigin)マクロのテストを追加し、派生(IdEqOrdHash)の繰り返しを減らし、安定版でのエラー報告を修正
- 名前付けを改善し、繰り返される .filter_map を簡素化し、derive(Filter) 内の不要な .以外を削除しました。
- PartiallyTaggedSerialize/Deserialize に darling を使用させる
- derling(IdEqOrdHash)にdarlingを使用させ、テストを追加します
- derling(Filter)にdarlingを使用させる
- syn 2.0を使用するようにiroha_data_model_deriveを更新
- 署名チェック条件単体テストを追加
- 固定された署名検証条件セットのみを許可します
- ConstBytes を任意の const シーケンスを保持する ConstVec に一般化します。
- 変化しないバイト値に対してより効率的な表現を使用する
- 完成したwsvをスナップショットに保存
- `SnapshotMaker` アクターを追加
- proc マクロでの派生解析のドキュメント制限
- コメントをクリーンアップする
- 属性を解析するための共通テスト ユーティリティを lib.rs に抽出します。
- parse_display を使用し、属性を更新します -> 属性の名前付け
- ffi 関数の引数でパターン マッチングの使用を許可します
- getset attrs 解析の繰り返しを減らす
- Emitter::into_token_stream の名前を Emitter::finish_token_stream に変更します
- parse_display を使用して getset トークンを解析する
- タイプミスを修正し、エラーメッセージを改善しました
- iroha_ffi_derive: darling を使用して属性を解析し、syn 2.0 を使用します。
- iroha_ffi_derive: proc-macro-error を manyhow に置き換えます
- クラロックファイルのコードを簡素化
- すべての数値を文字列リテラルとしてシリアル化します。
- Kagami を分割 (#3841)
- `scripts/test-env.sh` を書き換えます
- スマートコントラクトとトリガーエントリーポイントを区別する
- `.cloned()` を `data_model/src/block.rs` に省略します
- syn 2.0 を使用するように `iroha_schema_derive` を更新します

## [2.0.0-rc.19以前] - 2023-08-14

### 追加- hyperledger#3309 バンプ IVM ランタイムの改善
- hyperledger#3383 コンパイル時にソケット アドレスを解析するマクロを実装します。
- hyperledger#2398 クエリ フィルターの統合テストを追加
- 実際のエラー メッセージを `InternalError` に含めます。
- デフォルトのツールチェーンとしての `nightly-2023-06-25` の使用
- hyperledger#3692 バリデーターの移行
- [DSL インターンシップ] hyperledger#3688: 基本的な算術演算を proc マクロとして実装する
- hyperledger#3371 バリデータ `entrypoint` を分割して、バリデータがスマート コントラクトとして認識されないようにする
- hyperledger#3651 WSV スナップショット。クラッシュ後に Iroha ノードを迅速に起動できます。
- hyperledger#3752 `MockValidator` を、すべてのトランザクションを受け入れる `Initial` バリデーターに置き換えます。
- hyperledger#3276 指定された文字列を Iroha ノードのメイン ログに記録する `Log` という一時的な命令を追加します。
- hyperledger#3641 許可トークンのペイロードを人間が判読できるようにする
- hyperledger#3324 `iroha_client_cli` 関連の `burn` チェックとリファクタリングを追加
- hyperledger#3781 ジェネシス トランザクションを検証する
- hyperledger#2885 トリガーに使用できるイベントと使用できないイベントを区別する
- hyperledger#2245 `Nix` ベースの iroha ノード バイナリのビルド (`AppImage`)

### 修正済み

- hyperledger#3613 誤って署名されたトランザクションが受け入れられる可能性がある回帰
- 間違った構成トポロジを早期に拒否します
- hyperledger#3445 回帰を修正し、`/configuration` エンドポイント上の `POST` が再び動作するようにしました。
- hyperledger#3654 `iroha2` `glibc` ベースの `Dockerfiles` が展開されるように修正
- hyperledger#3451 Apple Silicon Mac 上の `docker` ビルドを修正
- hyperledger#3741 `kagami validator` の `tempfile` エラーを修正
- hyperledger#3758 個々のクレートを構築できなかったが、ワークスペースの一部として構築できたリグレッションを修正
- hyperledger#3777 ロール登録のパッチの抜け穴が検証されない
- hyperledger#3805 `SIGTERM` を受信した後に Iroha がシャットダウンしない問題を修正

### その他

- hyperledger#3648 CI プロセスに `docker-compose.*.yml` チェックを含める
- 命令 `len()` を `iroha_data_model` から `iroha_core` に移動します。
- hyperledger#3672 派生マクロで `HashMap` を `FxHashMap` に置き換えます。
- hyperledger#3374 エラーのドキュメントコメントと `fmt::Display` 実装を統合
- hyperledger#3289 プロジェクト全体で Rust 1.70 ワークスペースの継承を使用する
- hyperledger#3654 `Dockerfiles` を追加して `GNU libc <https://www.gnu.org/software/libc/>`_ に iroha2 をビルドします
- proc-マクロ用に `syn` 2.0、`manyhow`、および `darling` を導入
- hyperledger#3802 Unicode `kagami crypto` シード

## [2.0.0-rc.18以前]

### 追加

- hyperledger#3468: サーバー側カーソル。これにより、遅延評価された再入可能なページネーションが可能になり、クエリのレイテンシーに対してパフォーマンスに大きなプラスの影響を与えるはずです。
- hyperledger#3624: 汎用アクセス許可トークン。具体的には
  - アクセス許可トークンは任意の構造を持つことができます
  - トークン構造は `iroha_schema` で自己記述され、JSON 文字列としてシリアル化されます。
  - トークン値は `Norito` でエンコードされています
  - この変更の結果、パーミッション トークンの命名規則は `snake_case` から `UpeerCamelCase` に移動されました。
- hyperledger#3615 検証後に wsv を保存する

### 修正済み- hyperledger#3627 `WorlStateView` のクローン作成によってトランザクションのアトミック性が適用されるようになりました
- hyperledger#3195 拒否されたジェネシストランザクションを受信したときのパニック動作を拡張
- hyperledger#3042 不正なリクエスト メッセージを修正
- hyperledger#3352 制御フローとデータ メッセージを別々のチャネルに分割する
- hyperledger#3543 メトリクスの精度を向上させます

## 2.0.0-pre-rc.17

### 追加

- hyperledger#3330 `NumericValue` デシリアライゼーションの拡張
- hyperledger#2622 FFI での `u128`/`i128` のサポート
- hyperledger#3088 DoS を防ぐためにキュー スロットリングを導入します
- hyperledger#2373 `docker-compose` ファイルを生成するための `kagami swarm file` および `kagami swarm dir` コマンド バリアント
- hyperledger#3597 パーミッショントークン分析 (Iroha 側)
- hyperledger#3353 エラー条件を列挙し、厳密に型指定されたエラーを使用して、`eyre` を `block.rs` から削除します。
- hyperledger#3318 ブロック内の拒否されたトランザクションと承認されたトランザクションをインターリーブして、トランザクションの処理順序を保持します

### 修正済み

- hyperledger#3075 `genesis.json` の無効なトランザクションでパニックを起こし、無効なトランザクションが処理されないようにする
- hyperledger#3461 デフォルト設定のデフォルト値の適切な処理
- hyperledger#3548 `IntoSchema` 透明属性を修正
- hyperledger#3552 バリデーター パス スキーマ表現を修正
- hyperledger#3546 時間トリガーがスタックする問題を修正
- hyperledger#3162 ブロック ストリーミング リクエストでの高さ 0 の禁止
- 設定マクロの初期テスト
- hyperledger#3592 `release` で更新される構成ファイルを修正
- hyperledger#3246 `fault <https://en.wikipedia.org/wiki/Byzantine_fault>`_ なしで `Set B validators <https://github.com/hyperledger-iroha/iroha/blob/main/docs/source/iroha_2_whitepaper.md#2-system-architecture>`_ を関与させないでください
- hyperledger#3570 クライアント側の文字列クエリ エラーを正しく表示する
- hyperledger#3596 `iroha_client_cli` はブロック/イベントを示します
- hyperledger#3473 `kagami validator` を iroha リポジトリのルート ディレクトリの外部から動作させる

### その他

- hyperledger#3063 トランザクション `hash` を `wsv` のブロック高さにマップします
- `Value` 内の厳密に型指定された `HashOf<T>`

## [2.0.0-rc.16以前]

### 追加

- hyperledger#2373 `docker-compose.yml` を生成するための `kagami swarm` サブコマンド
- hyperledger#3525 トランザクション API の標準化
- hyperledger#3376 Iroha クライアント CLI `pytest <https://docs.pytest.org/en/7.4.x/>`_ 自動化フレームワークを追加
- hyperledger#3516 `LoadedExecutable` に元の BLOB ハッシュを保持する

### 修正済み

- hyperledger#3462 `burn` 資産コマンドを `client_cli` に追加
- hyperledger#3233 エラー タイプのリファクタリング
- hyperledger#3330 `partially-tagged <https://serde.rs/enum-representations.html>`_ `enums` に対して `serde::de::Deserialize` を手動で実装することで回帰を修正
- hyperledger#3487 欠落している型をスキーマに戻す
- hyperledger#3444 判別式をスキーマに戻す
- hyperledger#3496 `SocketAddr` フィールドの解析を修正
- hyperledger#3498 ソフトフォーク検出を修正
- hyperledger#3396 ブロックコミットイベントを発行する前に `kura` にブロックを保存

### その他

- hyperledger#2817 `WorldStateView` から内部可変性を削除
- hyperledger#3363 Genesis API リファクタリング
- 既存のリファクタリングを行い、トポロジーの新しいテストを追加します。
- テスト カバレッジを `Codecov <https://about.codecov.io/>`_ から `Coveralls <https://coveralls.io/>`_ に切り替える
- hyperledger#3533 スキーマ内の `Bool` の名前を `bool` に変更します

## [2.0.0-rc.15以前]

### 追加- hyperledger#3231 モノリシックバリデーター
- hyperledger#3015 FFI でのニッチ最適化のサポート
- hyperledger#2547 `AssetDefinition` にロゴを追加
- hyperledger#3274 サンプルを生成するサブコマンドを `kagami` に追加します (LTS にバックポート)
- hyperledger#3415 `Nix <https://nixos.wiki/wiki/Flakes>`_ フレーク
- hyperledger#3412 トランザクションのゴシップを別のアクターに移動する
- hyperledger#3435 `Expression` 訪問者を紹介します
- hyperledger#3168 ジェネシスバリデータを別のファイルとして提供する
- hyperledger#3454 ほとんどの Docker 操作およびドキュメントのデフォルトを LTS にする
- hyperledger#3090 オンチェーン パラメーターをブロックチェーンから `sumeragi` に伝播する

### 修正済み

- hyperledger#3330 `u128` リーフによるタグなし列挙型の逆シリアル化を修正 (RC14 にバックポート)
- hyperledger#2581 はログのノイズを削減しました
- hyperledger#3360 `tx/s` ベンチマークを修正
- hyperledger#3393 `actors` の通信デッドロック ループを解除する
- hyperledger#3402 `nightly` ビルドを修正
- hyperledger#3411 ピアの同時接続を適切に処理する
- hyperledger#3440 転送中の資産変換を非推奨にし、代わりにスマートコントラクトによって処理します
- hyperledger#3408: `public_keys_cannot_be_burned_to_nothing` テストを修正

### その他

- hyperledger#3362 `tokio` アクターへの移行
- hyperledger#3349 スマート コントラクトから `EvaluateOnHost` を削除
- hyperledger#1786 ソケット アドレスに `iroha` ネイティブ タイプを追加
- IVM キャッシュを無効にする
- IVM キャッシュを再度有効にする
- パーミッションバリデーターの名前をバリデーターに変更します
- hyperledger#3388 `model!` をモジュールレベルの属性マクロにする
- hyperledger#3370 `hash` を 16 進文字列としてシリアル化します
- `maximum_transactions_in_block` を `queue` から `sumeragi` 構成に移動
- `AssetDefinitionEntry` タイプを非推奨にして削除
- `configs/client_cli` の名前を `configs/client` に変更します
- アップデート `MAINTAINERS.md`

## [2.0.0-rc.14以前]

### 追加

- hyperledger#3127 データ モデル `structs` はデフォルトで不透明
- hyperledger#3122 は、ダイジェスト関数の保存に `Algorithm` を使用します (コミュニティ貢献者)
- hyperledger#3153 `iroha_client_cli` 出力は機械可読です
- hyperledger#3105 `AssetDefinition` に対して `Transfer` を実装
- hyperledger#3010 `Transaction` パイプライン期限切れイベントが追加されました

### 修正済み

- hyperledger#3113 不安定なネットワーク テストのリビジョン
- hyperledger#3129 `Parameter` のシリアル化解除を修正
- hyperledger#3141 `Hash` に対して `IntoSchema` を手動で実装する
- hyperledger#3155 テストのパニックフックを修正し、デッドロックを防止
- hyperledger#3166 アイドル状態では変更を表示しないため、パフォーマンスが向上します
- hyperledger#2123 マルチハッシュから PublicKey の非シリアル化に戻る
- hyperledger#3132 NewParameter バリデーターを追加
- hyperledger#3249 ブロック ハッシュを部分バージョンと完全バージョンに分割する
- hyperledger#3031 欠落している構成パラメータの UI/UX を修正
- hyperledger#3247 `sumeragi` からフォールト挿入を削除しました。

### その他

- 偽の障害を修正するために不足している `#[cfg(debug_assertions)]` を追加
- hyperledger#2133 ホワイトペーパーに近くなるようにトポロジを書き換えます
- `iroha_core` に対する `iroha_client` の依存関係を削除
- hyperledger#2943 `HasOrigin` を導出
- hyperledger#3232 ワークスペースのメタデータを共有する
- hyperledger#3254 `commit_block()` および `replace_top_block()` をリファクタリング
- 安定したデフォルトのアロケータ ハンドラを使用する
- hyperledger#3183 `docker-compose.yml` ファイルの名前を変更します
- `Multihash`の表示形式を改善しました
- hyperledger#3268 グローバルに一意の項目識別子
- 新しい PR テンプレート

## [2.0.0-rc.13 より前]

### 追加- hyperledger#2399 パラメーターを ISI として構成します。
- hyperledger#3119 `dropped_messages` メトリックを追加します。
- hyperledger#3094 `n` ピアを使用してネットワークを生成します。
- hyperledger#3082 `Created` イベントで完全なデータを提供します。
- hyperledger#3021 不透明なポインターのインポート。
- hyperledger#2794 FFI で明示的な判別式を持つフィールドレス列挙型を拒否します。
- hyperledger#2922 `Grant<Role>` をデフォルトのジェネシスに追加します。
- hyperledger#2922 `NewRole` json 逆シリアル化の `inner` フィールドを省略します。
- hyperledger#2922 json 逆シリアル化で `object(_id)` を省略します。
- hyperledger#2922 json 逆シリアル化で `Id` を省略します。
- hyperledger#2922 json 逆シリアル化で `Identifiable` を省略します。
- hyperledger#2963 `queue_size` をメトリクスに追加します。
- hyperledger#3027 Kura のロックファイルを実装します。
- hyperledger#2813 Kagami はデフォルトのピア構成を生成します。
- hyperledger#3019 JSON5 をサポートします。
- hyperledger#2231 FFI ラッパー API を生成します。
- hyperledger#2999 ブロック署名を蓄積します。
- hyperledger#2995 ソフトフォーク検出。
- hyperledger#2905 `NumericValue` をサポートするために算術演算を拡張します
- hyperledger#2868 iroha バージョンを出力し、ログにハッシュをコミットします。
- hyperledger#2096 資産の合計額をクエリします。
- hyperledger#2899 複数命令サブコマンドを「client_cli」に追加
- hyperledger#2247 WebSocket の通信ノイズを除去します。
- hyperledger#2889 `iroha_client` にブロック ストリーミング サポートを追加
- hyperledger#2280 ロールの付与/取り消し時に権限イベントを生成します。
- hyperledger#2797 イベントを充実させます。
- hyperledger#2725 `submit_transaction_blocking` にタイムアウトを再導入する
- hyperledger#2712 構成プロップテスト。
- hyperledger#2491 FFi での列挙型のサポート。
- hyperledger#2775 合成ジェネシスで異なるキーを生成します。
- hyperledger#2627 構成のファイナライゼーション、プロキシ エントリポイント、kagami docgen。
- hyperledger#2765 `kagami` で合成ジェネシスを生成する
- hyperledger#2698 `iroha_client` の不明確なエラー メッセージを修正
- hyperledger#2689 権限トークン定義パラメーターを追加します。
- hyperledger#2502 ビルドの GIT ハッシュを保存します。
- hyperledger#2672 `ipv4Addr`、`ipv6Addr` バリアントと述語を追加します。
- hyperledger#2626 `Combine` 派生、分割 `config` マクロを実装します。
- プロキシ構造体の hyperledger#2586 `Builder` および `LoadFromEnv`。
- hyperledger#2611 汎用の不透明構造体に対して `TryFromReprC` および `IntoFfi` を導出します。
- hyperledger#2587 `Configurable` を 2 つの特性に分割します。 #2587: `Configurable` を 2 つの特性に分割する
- hyperledger#2488 `ffi_export` に特性 impls のサポートを追加
- hyperledger#2553 資産クエリに並べ替えを追加します。
- hyperledger#2407 パラメータトリガー。
- hyperledger#2536 FFI クライアント用に `ffi_import` を導入します。
- hyperledger#2338 `cargo-all-features` インストルメンテーションを追加します。
- hyperledger#2564 Kagami ツール アルゴリズム オプション。
- hyperledger#2490 自立関数用に ffi_export を実装します。
- hyperledger#1891 トリガーの実行を検証します。
- hyperledger#1988 Identifiable、Eq、Hash、Ord のマクロを派生します。
- hyperledger#2434 FFI バインドジェン ライブラリ。
- hyperledger#2073 ブロックチェーンの型には String よりも ConstString を優先します。
- hyperledger#1889 ドメイン スコープのトリガーを追加します。
- hyperledger#2098 ヘッダー クエリをブロックします。 #2098: ブロックヘッダークエリを追加
- hyperledger#2467 iroha_client_cli にアカウント許可サブコマンドを追加します。
- hyperledger#2301 クエリ時にトランザクションのブロック ハッシュを追加します。
 - hyperledger#2454 Norito デコーダ ツールにビルド スクリプトを追加します。
- hyperledger#2061 フィルターの派生マクロ。- hyperledger#2228 スマートコントラクトのクエリ エラーに未承認のバリアントを追加します。
- hyperledger#2395 ジェネシスを適用できない場合のパニックを追加。
- hyperledger#2000 空の名前を禁止します。 #2000: 空の名前を禁止する
 - hyperledger#2127 Norito コーデックによってデコードされたすべてのデータが消費されていることを確認するための健全性チェックを追加しました。
- hyperledger#2360 `genesis.json` を再びオプションにします。
- hyperledger#2053 プライベート ブロックチェーンの残りのすべてのクエリにテストを追加します。
- hyperledger#2381 `Role` 登録を統合します。
- hyperledger#2053 プライベート ブロックチェーンの資産関連のクエリにテストを追加します。
- hyperledger#2053 「private_blockchain」にテストを追加
- hyperledger#2302 「FindTriggersByDomainId」スタブクエリを追加します。
- hyperledger#1998 クエリにフィルターを追加します。
- hyperledger#2276 現在のブロック ハッシュを BlockHeaderValue に含めます。
- hyperledger#2161 ハンドル ID と共有 FFI fns。
- ハンドル ID を追加し、共有特性 (Clone、Eq、Ord) に相当する FFI を実装します。
- hyperledger#1638 `configuration` ドキュメントのサブツリーを返します。
- hyperledger#2132 `endpointN` proc マクロを追加します。
- hyperledger#2257 Revoke<Role> は、RoleRevoked イベントを発行します。
- hyperledger#2125 FindAssetDefinitionById クエリを追加します。
- hyperledger#1926 シグナル処理と正常なシャットダウンを追加します。
- hyperledger#2161 `data_model` の FFI 関数を生成
- hyperledger#1149 ブロック ファイル数はディレクトリごとに 1000000 を超えません。
- hyperledger#1413 API バージョンのエンドポイントを追加します。
- hyperledger#2103 は、ブロックとトランザクションのクエリをサポートします。 `FindAllTransactions` クエリを追加
- hyperledger#2186 `BigQuantity` および `Fixed` の転送 ISI を追加します。
- hyperledger#2056 `AssetValueType` `enum` の派生手続きマクロ クレートを追加します。
- hyperledger#2100 資産を持つすべてのアカウントを検索するクエリを追加します。
- hyperledger#2179 トリガーの実行を最適化します。
- hyperledger#1883 埋め込まれた構成ファイルを削除します。
- hyperledger#2105 クライアントでのクエリ エラーを処理します。
- hyperledger#2050 ロール関連のクエリを追加します。
- hyperledger#1572 特殊な権限トークン。
- hyperledger#2121 構築時にキーペアが有効であることを確認します。
 - hyperledger#2003 Norito デコーダ ツールを導入します。
- hyperledger#1952 最適化の標準として TPS ベンチマークを追加します。
- hyperledger#2040 トランザクション実行制限を伴う統合テストを追加します。
- hyperledger#1890 Orillion のユースケースに基づいた統合テストを導入します。
- hyperledger#2048 ツールチェーン ファイルを追加します。
- hyperledger#2100 資産を持つすべてのアカウントを検索するクエリを追加します。
- hyperledger#2179 トリガーの実行を最適化します。
- hyperledger#1883 埋め込まれた構成ファイルを削除します。
- hyperledger#2004 `isize` および `usize` が `IntoSchema` になることを禁止します。
- hyperledger#2105 クライアントでのクエリ エラーを処理します。
- hyperledger#2050 ロール関連のクエリを追加します。
- hyperledger#1572 特殊な権限トークン。
- hyperledger#2121 構築時にキーペアが有効であることを確認します。
 - hyperledger#2003 Norito デコーダ ツールを導入します。
- hyperledger#1952 最適化の標準として TPS ベンチマークを追加します。
- hyperledger#2040 トランザクション実行制限を伴う統合テストを追加します。
- hyperledger#1890 Orillion のユースケースに基づいた統合テストを導入します。
- hyperledger#2048 ツールチェーン ファイルを追加します。
- hyperledger#2037 コミット前トリガーを導入します。
- hyperledger#1621 コールトリガーによる導入。
- hyperledger#1970 オプションのスキーマ エンドポイントを追加します。
- hyperledger#1620 時間ベースのトリガーを導入します。
- hyperledger#1918 `client` の基本認証を実装する
- hyperledger#1726 リリース PR ワークフローを実装します。
- hyperledger#1815 クエリ応答をより型構造化します。- hyperledger#1928 `gitchangelog` を使用して変更ログ生成を実装
- hyperledger#1902 ベアメタル 4 ピア セットアップ スクリプト。

  docker-compose を必要とせず、Iroha のデバッグ ビルドを使用するバージョンの setup_test_env.sh を追加しました。
- hyperledger#1619 イベントベースのトリガーを導入します。
- hyperledger#1195 WebSocket 接続を正常に閉じます。
- hyperledger#1606 ドメイン構造のドメイン ロゴに ipfs リンクを追加します。
- hyperledger#1754 Kura インスペクター CLI を追加します。
- hyperledger#1790 スタックベースのベクトルを使用してパフォーマンスを向上させます。
- hyperledger#1805 パニック エラー用のオプションの端子色。
- ハイパーレジャー#1749 `no_std` (`data_model`)
- hyperledger#1179 権限またはロールの取り消し命令を追加します。
- hyperledger#1782 iroha_crypto no_std と互換性を持たせます。
- hyperledger#1172 命令イベントを実装します。
- hyperledger#1734 `Name` を検証して空白を除外します。
- hyperledger#1144 メタデータのネストを追加します。
- #1210 ストリーミングをブロックします (サーバー側)。
- hyperledger#1331 より多くの `Prometheus` メトリクスを実装します。
- hyperledger#1689 機能の依存関係を修正しました。 #1261: 貨物の膨張を追加します。
- hyperledger#1675 バージョン管理されたアイテムのラッパー構造体の代わりに型を使用します。
- hyperledger#1643 ピアがテストでジェネシスをコミットするのを待ちます。
- ハイパーレジャー#1678 `try_allocate`
- hyperledger#1216 Prometheus エンドポイントを追加します。 #1216: メトリクスエンドポイントの初期実装。
- hyperledger#1238 実行時のログレベルの更新。基本的な `connection` エントリポイントベースのリロードを作成しました。
- hyperledger#1652 PR タイトルの書式設定。
- 接続ピア数を `Status` に追加します。

  ・「接続ピア数に関するものを削除」を元に戻す

  これにより、コミット b228b41dab3c035ce9973b6aa3b35d443c082544 が元に戻ります。
  - `Peer` がハンドシェイク後にのみ真の公開キーを持っていることを明確にする
  - テストなしの `DisconnectPeer`
  - 登録解除ピアの実行を実装する
  - ピア登録サブコマンドを `client_cli` に追加 (解除)
  - 未登録のピアからの再接続をアドレスによって拒否します

  ピアが別のピアの登録を解除して切断した後、
  ネットワークはピアからの再接続要求を受信します。
  最初に分かるのはポート番号が任意のアドレスだけです。
  そのため、未登録のピアをポート番号以外の部分で覚えておいてください。
  そこからの再接続を拒否します
- `/status` エンドポイントを特定のポートに追加します。

### 修正- hyperledger#3129 `Parameter` のシリアル化解除を修正しました。
- hyperledger#3109 役割に依存しないメッセージの後に `sumeragi` がスリープしないようにします。
- hyperledger#3046 Iroha が空の状態で正常に開始できることを確認する
  `./storage`
- hyperledger#2599 苗床の糸くずを除去します。
- hyperledger#3087 ビューの変更後にセット B バリデーターから投票を収集します。
- hyperledger#3056 `tps-dev` ベンチマークのハングを修正。
- hyperledger#1170 クローン作成 -wsv スタイルのソフトフォーク処理を実装します。
- hyperledger#2456 ジェネシスブロックを無制限にします。
- hyperledger#3038 マルチシグを再度有効にします。
- hyperledger#2894 `LOG_FILE_PATH` 環境変数の逆シリアル化を修正しました。
- hyperledger#2803 署名エラーに対して正しいステータス コードを返します。
- hyperledger#2963 `Queue` トランザクションを正しく削除します。
- hyperledger#0000 Vergen の CI 破壊。
- hyperledger#2165 ツールチェーン フィジェットを削除します。
- hyperledger#2506 ブロック検証を修正しました。
- hyperledger#3013 検証ツールを適切にチェーン書き込みします。
- hyperledger#2998 使用されていないチェーンコードを削除します。
- hyperledger#2816 ブロックへのアクセス責任を蔵に移します。
- hyperledger#2384 デコードを decode_all に置き換えます。
- hyperledger#1967 ValueName を名前に置き換えます。
- hyperledger#2980 ブロック値 ffi タイプを修正しました。
- hyperledger#2858 std の代わりに parking_lot::Mutex を導入します。
- hyperledger#2850 `Fixed` の逆シリアル化/デコードを修正
- hyperledger#2923 `AssetDefinition` が返さない場合に `FindError` を返す
  存在します。
- hyperledger#0000 修正 `panic_on_invalid_genesis.sh`
- hyperledger#2880 WebSocket 接続を適切に閉じます。
- hyperledger#2880 ブロック ストリーミングを修正しました。
- hyperledger#2804 `iroha_client_cli` 送信トランザクションのブロック。
- hyperledger#2819 重要でないメンバーを WSV から移動します。
- 式のシリアル化再帰バグを修正しました。
- hyperledger#2834 短縮構文を改善しました。
- hyperledger#2379 新しい Kura ブロックを blocks.txt にダンプする機能を追加します。
- hyperledger#2758 ソート構造をスキーマに追加します。
-CI.
- hyperledger#2548 大きな Genesis ファイルについて警告します。
- hyperledger#2638 `whitepaper` を更新し、変更を反映します。
- hyperledger#2678 ステージング ブランチのテストを修正しました。
- hyperledger#2678 Kura 強制シャットダウン時にテストが中止される問題を修正しました。
- hyperledger#2607 よりシンプルにするために Sumeragi コードをリファクタリングし、
  堅牢性を修正。
- hyperledger#2561 コンセンサスへのビュー変更を再導入します。
- hyperledger#2560 block_sync とピアの切断を再度追加します。
- hyperledger#2559 sumeragi スレッドのシャットダウンを追加。
- hyperledger#2558 蔵から wsv を更新する前に、genesis を検証します。
- hyperledger#2465 sumeragi ノードをシングルスレッド状態として再実装します
  機械。
- hyperledger#2449 Sumeragi 再構築の初期実装。
- hyperledger#2802 構成の環境読み込みを修正しました。
- hyperledger#2787 パニック時にシャットダウンするようにすべてのリスナーに通知します。
- hyperledger#2764 最大メッセージ サイズの制限を削除します。
- #2571: Kura Inspector UX の改善。
- hyperledger#2703 Orillion 開発環境のバグを修正。
- スキーマ/ソースのドキュメント コメントのタイプミスを修正しました。
- hyperledger#2716 稼働時間の期間を公開します。
- hyperledger#2700 Docker イメージで `KURA_BLOCK_STORE_PATH` をエクスポートします。
- hyperledger#0 ビルダーから `/iroha/rust-toolchain.toml` を削除します
  画像。
- hyperledger#0 修正 `docker-compose-single.yml`
- hyperledger#2554 `secp256k1` シードが 32 より短い場合にエラーが発生する
  バイト。
- hyperledger#0 各ピアにストレージを割り当てるように `test_env.sh` を変更します。
- hyperledger#2457 テストで蔵を強制的にシャットダウンします。
- hyperledger#2623 VariantCount の doctest を修正しました。
- ui_fail テストで予期されるエラーを更新します。
- 権限バリデーターの誤ったドキュメント コメントを修正。- hyperledger#2422 構成エンドポイントの応答で秘密キーを非表示にします。
- hyperledger#2492: イベントに一致する実行中のトリガーの一部を修正しました。
- hyperledger#2504 失敗する TPS ベンチマークを修正。
- hyperledger#2477 ロールからの権限がカウントされなかったときのバグを修正。
- hyperledger#2416 macOS アーム上の糸くずを修正。
- hyperledger#2457 パニック時のシャットダウンに関連するテストの不安定さを修正しました。
  #2457: パニック構成でシャットダウンを追加
- hyperledger#2473 RUSTUP_TOOLCHAIN の代わりに Rustc --version を解析します。
- hyperledger#1480 パニック時にシャットダウンします。 #1480: パニック時にプログラムを終了するパニックフックを追加
- hyperledger#2376 簡略化された Kura、非同期なし、2 つのファイル。
- hyperledger#0000 Docker ビルド失敗。
- hyperledger#1649 `do_send` から `spawn` を削除します
- hyperledger#2128 `MerkleTree` の構築と反復を修正しました。
- hyperledger#2137 マルチプロセス コンテキストのテストを準備します。
- hyperledger#2227 資産の登録と登録解除を実装します。
- hyperledger#2081 ロール付与のバグを修正。
- hyperledger#2358 デバッグ プロファイルを含むリリースを追加します。
- hyperledger#2294 フレームグラフ生成を oneshot.rs に追加します。
- hyperledger#2202 クエリ応答の合計フィールドを修正しました。
- hyperledger#2081 ロールを付与するためのテスト ケースを修正しました。
- hyperledger#2017 ロールの登録解除を修正しました。
- hyperledger#2303 docker-compose ピアが正常にシャットダウンされない問題を修正しました。
- hyperledger#2295 トリガーの登録解除のバグを修正。
- hyperledger#2282 getset 実装から派生した FFI を改善します。
- hyperledger#1149 nocheckin コードを削除します。
- hyperledger#2232 Genesis の isi が多すぎる場合に、Iroha に意味のあるメッセージを出力させるようにしました。
- hyperledger#2170 M1 マシン上の Docker コンテナのビルドを修正しました。
- hyperledger#2215 `cargo build` の nightly-2022-04-20 をオプションにします
- hyperledger#1990 config.json がない場合に、環境変数を介したピアの起動を有効にします。
- hyperledger#2081 ロールの登録を修正しました。
- hyperledger#1640 config.json とgenesis.json を生成します。
- hyperledger#1716 f=0 のケースでのコンセンサス失敗を修正しました。
- hyperledger#1845 鋳造不可能な資産は 1 回だけ鋳造できます。
- hyperledger#2005 `Client::listen_for_events()` が WebSocket ストリームを閉じない問題を修正しました。
- hyperledger#1623 RawGenesisBlockBuilder を作成します。
- hyperledger#1917 easy_from_str_impl マクロを追加します。
- hyperledger#1990 config.json がない場合に、環境変数を介したピアの起動を有効にします。
- hyperledger#2081 ロールの登録を修正しました。
- hyperledger#1640 config.json とgenesis.json を生成します。
- hyperledger#1716 f=0 のケースでのコンセンサス失敗を修正しました。
- hyperledger#1845 鋳造不可能な資産は 1 回だけ鋳造できます。
- hyperledger#2005 `Client::listen_for_events()` が WebSocket ストリームを閉じない問題を修正しました。
- hyperledger#1623 RawGenesisBlockBuilder を作成します。
- hyperledger#1917 easy_from_str_impl マクロを追加します。
- hyperledger#1922 crypto_cli をツールに移動します。
- hyperledger#1969 `roles` 機能をデフォルトの機能セットの一部にします。
- hyperledger#2013 Hotfix CLI 引数。
- hyperledger#1897 シリアル化から usesize/isize を削除します。
- hyperledger#1955 `web_login` 内で `:` を渡す可能性を修正
- hyperledger#1943 クエリ エラーをスキーマに追加します。
- hyperledger#1939 `iroha_config_derive` の適切な機能。
- hyperledger#1908 テレメトリ分析スクリプトのゼロ値処理を修正しました。
- hyperledger#0000 暗黙的に無視される doc-test を明示的に無視します。
- hyperledger#1848 公開キーが焼き付けられて何もなくなるのを防ぎます。
- hyperledger#1811 は、信頼できるピア キーを重複除去するためのテストとチェックを追加しました。
- hyperledger#1821 MerkleTree および VersionedValidBlock の IntoSchema を追加し、HashOf および SignatureOf スキーマを修正しました。- hyperledger#1819 検証時のエラー レポートからトレースバックを削除します。
- hyperledger#1774 検証失敗の正確な理由をログに記録します。
- hyperledger#1714 PeerId をキーのみで比較します。
- hyperledger#1788 `Value` のメモリ フットプリントを削減します。
- hyperledger#1804 HashOf、SignatureOf のスキーマ生成を修正し、スキーマが欠落していないことを確認するテストを追加しました。
- hyperledger#1802 ログの読みやすさが向上しました。
  - イベント ログがトレース レベルに移動されました
  - ctx がログ キャプチャから削除されました
  - 端子の色はオプションになりました (ファイルへのログ出力を改善するため)
- hyperledger#1783 鳥居のベンチマークを修正しました。
- hyperledger#1772 #1764 以降を修正。
- hyperledger#1755 #1743、#1725 のマイナー修正。
  - #1743 `Domain` 構造体の変更に従って JSON を修正
- hyperledger#1751 コンセンサスを修正。 #1715: 高負荷を処理するためのコンセンサス修正 (#1746)
  - 変更処理の修正を表示
  - 特定のトランザクション ハッシュに依存せずに作成された変更証明を表示する
  - メッセージパッシングの削減
  - メッセージをすぐに送信するのではなく、ビュー変更投票を収集します (ネットワークの復元力を向上させます)
  - Sumeragi の Actor フレームワークを完全に使用します (タスク生成の代わりに自分自身へのメッセージをスケジュールします)
  - Sumeragi を使用したテストのフォールト挿入を改善しました。
  - テストコードを実稼働コードに近づけます
  - 過度に複雑なラッパーを削除します
  - Sumeragi がテスト コードでアクター コンテキストを使用できるようにします
- hyperledger#1734 新しいドメイン検証に合わせてジェネシスを更新します。
- hyperledger#1742 `core` 命令で具体的なエラーが返されました。
- hyperledger#1404 修正されたことを確認します。
- hyperledger#1636 `trusted_peers.json` および `structopt` を削除します
  #1636: `trusted_peers.json` を削除します。
- hyperledger#1706 トポロジ更新で `max_faults` を更新します。
- hyperledger#1698 公開キー、ドキュメント、エラー メッセージを修正しました。
- 鋳造号 (1593 年および 1405 年) 号 1405

### リファクタリング- sumeragi メインループから関数を抽出します。
- `ProofChain` を newtype にリファクタリングします。
- `Metrics` から `Mutex` を削除
- adt_const_generics 夜間機能を削除します。
- hyperledger#3039 マルチシグ用の待機バッファを導入します。
- スメラギを簡略化します。
- hyperledger#3053 クリッピーな糸くずを修正します。
- hyperledger#2506 ブロック検証にテストを追加します。
- Kura の `BlockStoreTrait` を削除します。
- `nightly-2022-12-22` の lint を更新
- hyperledger#3022 `transaction_cache` の `Option` を削除
- hyperledger#3008 `Hash` にニッチな値を追加します
- lint を 1.65 に更新します。
- 小さなテストを追加してカバレッジを高めます。
- `FaultInjection` からデッドコードを削除
- sumeragi から p2p を呼び出す頻度を減らします。
- hyperledger#2675 Vec を割り当てずに項目名/ID を検証します。
- hyperledger#2974 完全な再検証を行わずにブロックのなりすましを防止します。
- コンビネータでの `NonEmpty` の効率が向上しました。
- hyperledger#2955 BlockSigned メッセージからブロックを削除します。
- hyperledger#1868 検証されたトランザクションが送信されないようにする
  仲間の間で。
- hyperledger#2458 汎用コンビネータ API を実装します。
- gitignore にストレージフォルダーを追加します。
- hyperledger#2909 次のポートのハードコード。
- hyperledger#2747 `LoadFromEnv` API を変更します。
- 設定失敗時のエラー メッセージを改善しました。
- `genesis.json` に追加の例を追加
- `rc9` リリース前に未使用の依存関係を削除します。
- 新しい Sumeragi でリンティングを完了します。
- メインループ内のサブプロシージャを抽出します。
- hyperledger#2774 `kagami` ジェネシス生成モードをフラグからに変更します。
  サブコマンド。
- hyperledger#2478 `SignedTransaction` を追加
- hyperledger#2649 `Kura` から `byteorder` クレートを削除します
- `DEFAULT_BLOCK_STORE_PATH` の名前を `./blocks` から `./storage` に変更します
- hyperledger#2650 irohaサブモジュールをシャットダウンするための`ThreadHandler`を追加。
- hyperledger#2482 `Account` 権限トークンを `Wsv` に保存する
- 新しい lint を 1.62 に追加します。
- `p2p` エラー メッセージを改善しました。
- hyperledger#2001 `EvaluatesTo` 静的型チェック。
- hyperledger#2052 権限トークンを定義付きで登録可能にします。
  #2052: PermissionTokenDefinition の実装
- すべての機能の組み合わせが機能することを確認します。
- hyperledger#2468 権限バリデーターからデバッグ スーパートレイトを削除します。
- hyperledger#2419 明示的な `drop` を削除します。
- hyperledger#2253 `Registrable` 特性を `data_model` に追加
- データ イベントに対して `Identifiable` の代わりに `Origin` を実装します。
- hyperledger#2369 権限バリデータをリファクタリングします。
- hyperledger#2307 `WorldStateView` の `events_sender` をオプションではありません。
- hyperledger#1985 `Name` 構造体のサイズを削減します。
- `const fn` を追加します。
- 統合テストに `default_permissions()` を使用するようにする
- private_blockchain に許可トークン ラッパーを追加します。
- hyperledger#2292 `WorldTrait` を削除し、`IsAllowedBoxed` からジェネリックを削除します
- hyperledger#2204 資産関連の操作を汎用化します。
- hyperledger#2233 `Display` および `Debug` を、`impl` を `derive` に置き換えます。
- 識別可能な構造の改善。
- hyperledger#2323 kura init エラー メッセージを強化しました。
- hyperledger#2238 テスト用のピア ビルダーを追加します。
- hyperledger#2011 より説明的な設定パラメータ。
- hyperledger#1896 `produce_event` 実装を簡素化します。
- `QueryError` を中心にリファクタリングします。
- `TriggerSet` を `data_model` に移動します。
- hyperledger#2145 クライアントの `WebSocket` 側をリファクタリングし、純粋なデータ ロジックを抽出します。
- `ValueMarker` 特性を削除します。
- hyperledger#2149 `prelude` で `Mintable` と `MintabilityError` を公開する
- hyperledger#2144 クライアントの http ワークフローを再設計し、内部 API を公開します。- `clap` に移動します。
- `iroha_gen` バイナリを作成し、ドキュメント schema_bin を統合します。
- hyperledger#2109 `integration::events::pipeline` テストを安定させます。
- hyperledger#1982 は、`iroha_crypto` 構造へのアクセスをカプセル化します。
- `AssetDefinition` ビルダーを追加します。
- 不要な `&mut` を API から削除します。
- データ モデル構造へのアクセスをカプセル化します。
- hyperledger#2144 クライアントの http ワークフローを再設計し、内部 API を公開します。
- `clap` に移動します。
- `iroha_gen` バイナリを作成し、ドキュメント schema_bin を統合します。
- hyperledger#2109 `integration::events::pipeline` テストを安定させます。
- hyperledger#1982 は、`iroha_crypto` 構造へのアクセスをカプセル化します。
- `AssetDefinition` ビルダーを追加します。
- 不要な `&mut` を API から削除します。
- データ モデル構造へのアクセスをカプセル化します。
- コア、`sumeragi`、インスタンス関数、`torii`
- hyperledger#1903 イベントの発行を `modify_*` メソッドに移動します。
- `data_model` lib.rs ファイルを分割します。
- wsv 参照をキューに追加します。
- hyperledger#1210 イベント ストリームを分割します。
  - トランザクション関連の機能を data_model/transaction モジュールに移動
- hyperledger#1725 Torii のグローバル状態を削除します。
  - `add_state macro_rules` を実装し、`ToriiState` を削除
- リンターエラーを修正。
- hyperledger#1661 `Cargo.toml` クリーンアップ。
  - 貨物の依存関係を整理する
- hyperledger#1650 整理整頓 `data_model`
  - World を wsv に移動し、ロール機能を修正し、CommittedBlock の IntoSchema を派生します。
- `json` ファイルと Readme の構成。テンプレートに準拠するように Readme を更新します。
- 1529: 構造化ログ。
  - ログメッセージのリファクタリング
- `iroha_p2p`
  - P2P 民営化を追加します。

### ドキュメント

- Iroha クライアント CLI の Readme を更新します。
- チュートリアルのスニペットを更新します。
- 「sort_by_metadata_key」を API 仕様に追加します。
- ドキュメントへのリンクを更新します。
- アセット関連のドキュメントを追加してチュートリアルを拡張します。
- 古いドキュメント ファイルを削除します。
- 句読点を確認します。
- いくつかのドキュメントをチュートリアル リポジトリに移動します。
- ステージングブランチの不安定性レポート。
- rc.7 より前の変更ログを生成します。
- 7月30日の薄片性レポート。
- バンプバージョン。
- テストの不安定性を更新します。
- hyperledger#2499 client_cli エラー メッセージを修正しました。
- hyperledger#2344 2.0.0-pre-rc.5-lts の CHANGELOG を生成します。
- チュートリアルへのリンクを追加します。
- gitフックに関する情報を更新しました。
- 不安定性テストの書き込み。
- hyperledger#2193 Iroha クライアントのドキュメントを更新します。
- hyperledger#2193 Iroha CLI ドキュメントを更新します。
- hyperledger#2193 マクロ クレートの README を更新。
 - hyperledger#2193 Norito デコーダー ツールのドキュメントを更新します。
- hyperledger#2193 Kagami ドキュメントを更新します。
- hyperledger#2193 ベンチマーク ドキュメントを更新します。
- hyperledger#2192 貢献ガイドラインを確認してください。
- 壊れたコード内参照を修正します。
- hyperledger#1280 ドキュメント Iroha メトリクス。
- hyperledger#2119 Docker コンテナーに Iroha をホット リロードする方法に関するガイダンスを追加しました。
- hyperledger#2181 README を確認してください。
- hyperledger#2113 Cargo.toml ファイル内の文書機能。
- hyperledger#2177 gitchangelog 出力をクリーンアップします。
- hyperledger#1991 クラインスペクタにreadmeを追加。
- hyperledger#2119 Docker コンテナーに Iroha をホット リロードする方法に関するガイダンスを追加しました。
- hyperledger#2181 README を確認してください。
- hyperledger#2113 Cargo.toml ファイル内の文書機能。
- hyperledger#2177 gitchangelog 出力をクリーンアップします。
- hyperledger#1991 クラインスペクタにreadmeを追加。
- 最新の変更ログを生成します。
- 変更ログを生成します。
- 古い README ファイルを更新します。
- 欠落していたドキュメントを `api_spec.md` に追加しました。

### CI/CD の変更- さらに 5 つの自己ホスト ランナーを追加します。
- ソラミツレジストリに通常のイメージタグを追加します。
- libgit2-sys 0.5.0 の回避策。 0.4.4に戻します。
- アーチベースのイメージを使用しようとします。
- 新しい夜間専用コンテナで動作するようにワークフローを更新します。
- バイナリ エントリポイントをカバレッジから削除します。
- 開発テストを Equinix の自己ホスト型ランナーに切り替えます。
- hyperledger#2865 `scripts/check.sh` から tmp ファイルの使用を削除
- hyperledger#2781 カバレッジ オフセットを追加します。
- 遅い統合テストを無効にします。
- 基本イメージを Docker キャッシュに置き換えます。
- hyperledger#2781 codecov コミット親機能を追加します。
- ジョブを github ランナーに移動します。
- hyperledger#2778 クライアント構成チェック。
- hyperledger#2732 iroha2ベースのイメージを更新する条件を追加して追加
  PRラベル。
- 夜間のイメージビルドを修正しました。
- `docker/build-push-action` による `buildx` エラーを修正
- 機能しない場合の応急処置 `tj-actions/changed-files`
- #2662 以降のイメージの順次公開を有効にします。
- 港湾登録簿を追加します。
- 自動ラベル付け `api-changes` および `config-changes`
- イメージ内のハッシュをコミット、ツールチェーン ファイルを再度コミット、UI を分離、
  スキーマの追跡。
- 公開ワークフローを順次化し、#2427 を補完します。
- hyperledger#2309: CI でのドキュメント テストを再度有効にします。
- hyperledger#2165 codecov インストールを削除します。
- 現在のユーザーとの競合を防ぐために、新しいコンテナに移動します。
 - hyperledger#2158 `parity_scale_codec` およびその他の依存関係をアップグレードします。 (Norito コーデック)
- ビルドを修正。
- hyperledger#2461 iroha2 CIを改善。
- `syn` を更新します。
- カバレッジを新しいワークフローに移動します。
- リバースドッカーログイン版
- `archlinux:base-devel` のバージョン指定を削除
- Dockerfile と Codecov レポートの再利用と同時実行を更新します。
- 変更ログを生成します。
- `cargo deny` ファイルを追加します。
- `iroha2` からコピーされたワークフローを含む `iroha2-lts` ブランチを追加します
- hyperledger#2393 Docker 基本イメージのバージョンをバンプします。
- hyperledger#1658 ドキュメントチェックを追加。
- クレートのバージョンアップを行い、未使用の依存関係を削除します。
- 不要なカバレッジレポートを削除します。
- hyperledger#2222 カバレッジが含まれるかどうかによってテストを分割します。
- hyperledger#2153 #2154 を修正。
- すべてのクレートのバージョンをバンプします。
- デプロイパイプラインを修正しました。
- hyperledger#2153 カバレッジを修正。
- ジェネシスチェックを追加し、ドキュメントを更新します。
- サビ、カビ、夜間をそれぞれ 1.60、1.2.0、1.62 にバンプします。
- ロード RS トリガー。
- hyperledger#2153 #2154 を修正。
- すべてのクレートのバージョンをバンプします。
- デプロイパイプラインを修正しました。
- hyperledger#2153 カバレッジを修正。
- ジェネシスチェックを追加し、ドキュメントを更新します。
- サビ、カビ、夜間をそれぞれ 1.60、1.2.0、1.62 にバンプします。
- ロード RS トリガー。
-load-rs:ワークフロートリガーを解放します。
- プッシュワークフローを修正しました。
- デフォルト機能にテレメトリを追加します。
- メインにワークフローをプッシュするための適切なタグを追加します。
- 失敗したテストを修正します。
- hyperledger#1657 イメージを Rust 1.57 に更新します。 #1630: 自己ホスト型ランナーに戻ります。
- CIの改善。
- `lld` を使用するようにカバレッジを切り替えました。
- CI 依存関係の修正。
- CI セグメンテーションの改善。
- CI で修正された Rust バージョンを使用します。
- Docker パブリッシュおよび iroha2-dev プッシュ CI を修正しました。カバレッジとベンチを PR に移動
- CI Docker テストで不要な完全な Iroha ビルドを削除します。

  Iroha ビルドは Docker イメージ自体で実行されるようになったため、役に立たなくなりました。したがって、CI はテストで使用されるクライアント cli のみを構築します。
- CI パイプラインに iroha2 ブランチのサポートを追加しました。
  - 長いテストはiroha2へのPRでのみ実行されました
  - iroha2からのみDockerイメージを公開します
- 追加の CI キャッシュ。

### Web アセンブリ


### バージョンアップ- rc.13 より前のバージョン。
- rc.11 より前のバージョン。
- RC.9 へのバージョン。
- RC.8 へのバージョン。
- バージョンを RC7 に更新します。
- リリース前の準備。
- 金型 1.0 を更新します。
- 依存関係をバンプします。
- api_spec.md を更新: リクエスト/レスポンスボディを修正。
- Rustのバージョンを1.56.0にアップデート。
- 貢献ガイドを更新。
- 新しい API と URL 形式に合わせて README.md と `iroha/config.json` を更新します。
- docker パブリッシュ ターゲットを hyperledger/iroha2 #1453 に更新します。
- メインと一致するようにワークフローを更新します。
- API 仕様を更新し、ヘルス エンドポイントを修正します。
- Rust を 1.54 にアップデート。
- ドキュメント(iroha_crypto): `Signature` ドキュメントを更新し、`verify` の引数を調整します。
- Ursa のバージョンが 0.3.5 から 0.3.6 に変更されました。
- ワークフローを新しいランナーに更新します。
- キャッシュと CI ビルドの高速化のために dockerfile を更新します。
- libssl バージョンを更新します。
- dockerfile と async-std を更新します。
- 更新されたクリッピーを修正しました。
- アセット構造を更新します。
  - アセット内のキーと値の命令のサポート
  - 列挙型としてのアセットタイプ
  - アセット ISI のオーバーフロー脆弱性を修正
- 貢献ガイドを更新しました。
- 古いライブラリを更新します。
- ホワイトペーパーを更新し、リンティングの問題を修正します。
- cucumber_rust ライブラリを更新します。
- キー生成に関する README の更新。
- Github Actions ワークフローを更新します。
- Github Actions ワークフローを更新します。
- 要件.txt を更新します。
- common.yaml を更新します。
- Sara からのドキュメントの更新。
- 命令ロジックを更新します。
- ホワイトペーパーを更新します。
- ネットワーク機能の説明を更新。
- コメントに基づいてホワイトペーパーを更新します。
- WSV アップデートと Scale への移行の分離。
- gitignore を更新します。
- WPの蔵の説明を少し更新。
- ホワイトペーパーの蔵に関する説明を更新。

### スキーマ

- hyperledger#2114 スキーマでのソートされたコレクションのサポート。
- hyperledger#2108 ページネーションを追加します。
- hyperledger#2114 スキーマでのソートされたコレクションのサポート。
- hyperledger#2108 ページネーションを追加します。
- スキーマ、バージョン、マクロ no_std に互換性を持たせます。
- スキーマ内の署名を修正しました。
- スキーマ内の `FixedPoint` の表現が変更されました。
- スキーマのイントロスペクションに `RawGenesisBlock` を追加しました。
- スキーマ IR-115 を作成するためにオブジェクト モデルを変更しました。

### テスト

- hyperledger#2544 チュートリアルの doctest。
- hyperledger#2272 「FindAssetDefinitionById」クエリのテストを追加します。
- `roles` 統合テストを追加。
- ui テスト形式を標準化し、ui テストの派生をクレートの派生に移動します。
- 模擬テスト (先物の順序付けされていないバグ) を修正しました。
- DSL クレートを削除し、テストを `data_model` に移動しました
- 有効なコードについて不安定なネットワーク テストに合格することを確認します。
- iroha_p2p にテストを追加しました。
- テストが失敗しない限り、テスト中のログをキャプチャします。
- テストにポーリングを追加し、まれに壊れるテストを修正します。
- 並列セットアップをテストします。
- iroha init および iroha_client テストから root を削除します。
- テストのクリップ的な警告を修正し、ci にチェックを追加します。
- ベンチマーク テスト中の `tx` 検証エラーを修正しました。
- hyperledger#860: Iroha クエリとテスト。
- Iroha カスタム ISI ガイドと Cucumber テスト。
- no-std クライアントのテストを追加します。
- ブリッジ登録の変更とテスト。
- ネットワークモックを使用したコンセンサステスト。
- テスト実行のための一時ディレクトリの使用。
- ベンチは陽性者を検査します。
- テスト付きの初期マークル ツリー機能。
- テストと World State View の初期化を修正しました。

＃＃＃ 他の- パラメータ化を特性に移動し、FFI IR タイプを削除します。
- 共用体のサポートを追加し、`non_robust_ref_mut` を導入 * conststring FFI 変換を実装します。
- IdOrdEqHashを改善しました。
- (逆) シリアル化から FilterOpt::BySome を削除します。
- 透明にしない。
- ContextValue を透明にします。
- Expression::Raw タグをオプションにします。
- 一部の指示に透明性を追加します。
- RoleId の (非) シリアル化を改善しました。
- validator::Id の (逆) シリアル化を改善しました。
- PermissionTokenId の (逆) シリアル化を改善します。
- TriggerId の (逆) シリアル化を改善しました。
- Asset(-Definition) ID の (逆) シリアル化を改善します。
- AccountId の (逆) シリアル化を改善しました。
- Ipfs と DomainId の (逆) シリアル化を改善します。
- ロガー構成をクライアント構成から削除します。
- FFI での透過構造体のサポートを追加します。
- &Option<T> を Option<&T> にリファクタリングします
- 途切れ途切れの警告を修正しました。
- `Find` エラーの説明に詳細を追加。
- `PartialOrd` および `Ord` の実装を修正しました。
- `cargo fmt` の代わりに `rustfmt` を使用します
- `roles` 機能を削除します。
- `cargo fmt` の代わりに `rustfmt` を使用します
- workdir をボリュームとして開発 Docker インスタンスと共有します。
- 実行時の Diff 関連タイプを削除します。
- 複数値の戻り値の代わりにカスタム エンコードを使用します。
- iroha_crypto 依存関係として serde_json を削除します。
- バージョン属性では既知のフィールドのみを許可します。
- エンドポイントの異なるポートを明確にします。
- `Io` 派生を削除します。
- key_pairs の初期ドキュメント。
- 自己ホスト型ランナーに戻ります。
- コード内の新しいクリッピーリントを修正します。
- i1i1 をメンテナから削除します。
- アクターのドキュメントを追加し、マイナーな修正を加えました。
- 最新のブロックをプッシュする代わりにポーリングします。
- 7 つのピアごとにテストされたトランザクション ステータス イベント。
- `join_all` の代わりに `FuturesUnordered`
- GitHub ランナーに切り替えます。
- /query エンドポイントには、VersionedQueryResult と QueryResult を使用します。
- テレメトリを再接続します。
- dependabot の設定を修正。
- commit-msg git フックを追加してサインオフを含めます。
- プッシュパイプラインを修正。
- 依存ボットをアップグレードします。
- キュープッシュ時に将来のタイムスタンプを検出します。
- hyperledger#1197: Kura はエラーを処理します。
- ピアの登録解除命令を追加しました。
- トランザクションを区別するためにオプションのナンスを追加します。 #1493を閉じます。
- 不要な `sudo` を削除しました。
- ドメインのメタデータ。
- `create-docker` ワークフローのランダム バウンスを修正しました。
- 失敗したパイプラインによって示唆された `buildx` を追加しました。
- hyperledger#1454: 特定のステータス コードとヒントを含むクエリ エラー応答を修正しました。
- hyperledger#1533: ハッシュによってトランザクションを検索します。
- `configure` エンドポイントを修正しました。
- ブール値ベースのアセットの鋳造可能性チェックを追加します。
- 型付き暗号プリミティブの追加とタイプセーフ暗号化への移行。
- ロギングの改善。
- hyperledger#1458: アクター チャネル サイズを `mailbox` として構成に追加します。
- hyperledger#1451: `faulty_peers = 0` および `trusted peers count > 1` の場合の構成ミスに関する警告を追加
- 特定のブロック ハッシュを取得するためのハンドラーを追加します。
- 新しいクエリ FindTransactionByHash を追加しました。
- hyperledger#1185: クレートの名前とパスを変更します。
- ログを修正し、全般的な改善を行いました。
- hyperledger#1150: 1000 ブロックを各ファイルにグループ化します
- キューストレステスト。
- ログレベルの修正。
- クライアントライブラリにヘッダ仕様を追加。
- キューパニック障害の修正。
- 修正キュー。
- dockerfile リリース ビルドを修正。
- HTTPS クライアントの修正。
- スピードアップCI。
- 1. iroha_crypto を除くすべての ursa 依存関係を削除しました。
- 継続時間を減算するときのオーバーフローを修正しました。
- クライアントでフィールドを公開します。
- Iroha2 を毎晩 Dockerhub にプッシュします。
- httpステータスコードを修正。
- iroha_error を thiserror、eyre、color-eyre に置き換えます。
- キューをクロスビーム 1 に置き換えます。- 無駄な糸くずの許容範囲をいくつか削除します。
- アセット定義のメタデータを導入します。
- test_network クレートからの引数の削除。
- 不要な依存関係を削除します。
- iroha_client_cli::events を修正。
- hyperledger#1382: 古いネットワーク実装を削除します。
- hyperledger#1169: 資産の精度を追加しました。
- ピアの起動の改善:
  - env からのみ Genesis 公開キーをロードできるようにします
  - config、genesis、trusted_peers パスを cli params で指定できるようになりました
- hyperledger#1134: Iroha P2P の統合。
- クエリエンドポイントを GET ではなく POST に変更します。
- アクター内の on_start を同期的に実行します。
- ワープに移行します。
- ブローカーのバグを修正してコミットをやり直す。
- 「複数のブローカーの修正を導入」コミットを元に戻す(9c148c33826067585b5868d297dcdd17c0efe246)
- 複数のブローカー修正を導入:
  - アクター停止時にブローカーからサブスクライブを解除する
  - 同じアクター タイプ (以前は TODO) からの複数のサブスクリプションをサポートします。
  - ブローカーが常に自分自身をアクター ID として置くバグを修正。
- ブローカーのバグ (テスト ショーケース)。
- データ モデルの派生を追加します。
- 鳥居から rwlock を削除します。
- OOB クエリ権限チェック。
- hyperledger#1272: ピア数の実装、
- 命令内のクエリ権限の再帰チェック。
- 停止アクターをスケジュールします。
- hyperledger#1165: ピア数の実装。
- torii エンドポイントのアカウントごとにクエリ権限を確認します。
- システム メトリクスでの CPU とメモリの使用量の公開を削除しました。
 - WS メッセージの JSON を Norito に置き換えます。
- ビューの変更のプルーフを保存します。
- hyperledger#1168: トランザクションが署名チェック条件に合格しなかった場合のログを追加しました。
- 小さな問題を修正し、接続リッスン コードを追加しました。
- ネットワーク トポロジ ビルダーを導入します。
- Iroha の P2P ネットワークを実装します。
- ブロックサイズメトリックを追加します。
- PermissionValidator 特性の名前が IsAllowed に変更されました。およびそれに対応するその他の名前の変更
- API 仕様の Web ソケットの修正。
- Docker イメージから不要な依存関係を削除します。
- Fmt は Crate import_granularity を使用します。
- Generic Permission Validator の導入。
- アクターフレームワークに移行します。
- ブローカーのデザインを変更し、アクターにいくつかの機能を追加します。
- codecov ステータス チェックを設定します。
- grcov によるソースベースのカバレッジを使用します。
- 複数のビルド引数形式を修正し、中間ビルド コンテナーの ARG を再宣言しました。
- SubscriptionAccepted メッセージを導入しました。
- 操作後にゼロ価値資産をアカウントから削除します。
- Docker ビルド引数の形式を修正しました。
- 子ブロックが見つからない場合のエラー メッセージを修正しました。
- ビルドにベンダーの OpenSSL を追加し、pkg-config の依存関係を修正しました。
- dockerhub のリポジトリ名とカバレッジの差分を修正しました。
- TrustedPeers をロードできなかった場合の明確なエラー テキストとファイル名を追加しました。
- ドキュメント内のテキスト エンティティをリンクに変更しました。
- Docker パブリッシュの間違ったユーザー名シークレットを修正。
- ホワイトペーパーの小さなタイプミスを修正。
- ファイル構造を改善するために mod.rs の使用を許可します。
- main.rs を別のクレートに移動し、パブリック ブロックチェーンのアクセス許可を作成します。
- クライアント CLI 内にクエリを追加します。
- clap から cli の structopts に移行します。
- テレメトリを不安定なネットワーク テストに制限します。
- 特性をスマートコントラクト モジュールに移動します。
- Sed -i "s/world_state_view/wsv/g"
- スマート コントラクトを別のモジュールに移動します。
- Iroha ネットワーク コンテンツの長さのバグ修正。
- アクター ID 用のタスク ローカル ストレージを追加します。デッドロックの検出に役立ちます。
- デッドロック検出テストをCIに追加
- イントロスペクトマクロを追加します。
- ワークフロー名の曖昧さを解消し、書式設定も修正します
- クエリAPIの変更。
- async-std から tokio への移行。
- ci にテレメトリの分析を追加します。- irohaの先物テレメトリを追加します。
- すべての非同期関数にiroha futuresを追加します。
- 投票数を観測できるようにiroha先物を追加しました。
- 手動のデプロイと構成が README に追加されました。
- レポーターの修正。
- メッセージ派生マクロを追加します。
- シンプルなアクターフレームワークを追加します。
- 依存ボット構成を追加します。
- 優れたパニックおよびエラー レポーターを追加します。
- Rust バージョン 1.52.1 への移行と対応する修正。
- ブロッキング CPU 集中型タスクを別のスレッドで生成します。
- crates.io の unique_port と Cargo-lints を使用します。
- ロックフリー WSV を修正:
  - 不要なダッシュマップと API のロックを削除します
  - 過剰な数のブロックが作成されるバグを修正 (拒否されたトランザクションは記録されなかった)
  - エラーの完全なエラー原因を表示します
- テレメトリ加入者を追加します。
- 役割と権限のクエリ。
- ブロックを蔵からwsvに移動します。
- wsv 内のロックフリーのデータ構造に変更します。
- ネットワークタイムアウトの修正。
- ヘルスエンドポイントを修正します。
- 役割の紹介。
- dev ブランチからプッシュ Docker イメージを追加します。
- 積極的なリンティングを追加し、コードからパニックを削除します。
- 命令の実行特性を作り直しました。
- iroha_config から古いコードを削除します。
- IR-1060 既存のすべての権限に対する付与チェックを追加します。
- iroha_network の ulimit と timeout を修正しました。
- Ci タイムアウト テストの修正。
- 定義が削除された場合は、すべてのアセットを削除します。
- アセット追加時の wsv パニックを修正。
- チャンネルの Arc と Rwlock を削除します。
- Iroha ネットワークの修正。
- 権限バリデータはチェックで参照を使用します。
- 補助金の指示。
- NewAccount、Domain、AssetDefinition IR-1036 の文字列の長さ制限と ID の検証の設定を追加しました。
- ログをトレース ライブラリに置き換えます。
- ドキュメントの ci チェックを追加し、dbg マクロを拒否します。
- 付与可能な権限を導入します。
- iroha_config クレートを追加します。
- @alerdenisov をコード所有者として追加して、受信したすべてのマージ リクエストを承認します。
- コンセンサス中のトランザクション サイズ チェックを修正しました。
- async-std のアップグレードを元に戻します。
- 一部の定数を 2 のべき乗 IR-1035 に置き換えます。
- トランザクション履歴 IR-1024 を取得するクエリを追加します。
- ストアの権限の検証を追加し、権限バリデーターを再構築します。
- アカウント登録用に NewAccount を追加します。
- アセット定義のタイプを追加します。
- 構成可能なメタデータ制限を導入します。
- トランザクション メタデータの導入。
- クエリ内に式を追加します。
- lints.toml を追加し、警告を修正します。
- Trusted_peers を config.json から分離します。
- Telegram の Iroha 2 コミュニティへの URL のタイプミスを修正。
- 途切れ途切れの警告を修正しました。
- アカウントのキーと値のメタデータのサポートを導入します。
- ブロックのバージョン管理を追加します。
- ci lint の繰り返しを修正しました。
- mul、div、mod、raise_to 式を追加します。
- バージョン管理のために into_v* を追加します。
- Error::msg をエラー マクロに置き換えます。
- iroha_http_server を書き換えて、torii のエラーを修正します。
 - Norito バージョンを 2 にアップグレードします。
- ホワイトペーパーのバージョン管理の説明。
- 確実なページネーション。エラーによりページネーションが不要になり、代わりに空のコレクションが返されないケースを修正します。
- 列挙型のderive(Error)を追加。
- 夜間バージョンを修正。
- iroha_error クレートを追加。
- バージョン管理されたメッセージ。
- コンテナーのバージョン管理プリミティブを導入します。
- ベンチマークを修正します。
- ページネーションを追加します。
- バリアントエンコードデコードを追加します。
- クエリのタイムスタンプを u128 に変更します。
- パイプライン イベントの RejectionReason 列挙型を追加します。
- Genesis ファイルから古い行を削除します。宛先は以前のコミットでレジスタ ISI から削除されました。
- ISI の登録と登録解除を簡素化します。
- 4 ピア ネットワークで送信されないコミット タイムアウトを修正しました。
- ビュー変更時のトポロジシャッフル。- FromVariant 派生マクロ用の他のコンテナを追加します。
- クライアント CLI の MST サポートを追加します。
- FromVariant マクロを追加し、コードベースをクリーンアップします。
- i1i1 をコード所有者に追加します。
- ゴシップ取引。
- 命令と式の長さを追加します。
- ブロック時間パラメータとコミット時間パラメータにドキュメントを追加します。
- Verify および Accept 特性を TryFrom に置き換えました。
- 最小数のピアのみを待機するようにします。
- iroha2-java で API をテストするための github アクションを追加します。
- docker-compose-single.yml のジェネシスを追加。
- アカウントのデフォルトの署名チェック条件。
- 複数の署名者を持つアカウントのテストを追加します。
- MST のクライアント API サポートを追加します。
- ドッカーをビルドします。
- docker compose に Genesis を追加。
- 条件付き MST を導入します。
- wait_for_active_peers 実装を追加します。
- iroha_http_server に isahc クライアントのテストを追加します。
- クライアント API 仕様。
- 式でのクエリの実行。
- 式と ISI を統合します。
- ISI の式。
- アカウント構成ベンチマークを修正しました。
- クライアントのアカウント構成を追加します。
- `submit_blocking` を修正。
- パイプライン イベントが送信されます。
- Iroha クライアントの Web ソケット接続。
- パイプライン イベントとデータ イベントのイベント分離。
- 権限の統合テスト。
- burn と mint の権限チェックを追加します。
- ISI 権限の登録を解除します。
- ワールド構造体 PR のベンチマークを修正。
- World 構造体を導入します。
- ジェネシスブロックロードコンポーネントを実装します。
- ジェネシスアカウントを導入します。
- 権限検証ビルダーを導入します。
- Github Actions を使用して Iroha2 PR にラベルを追加します。
- 権限フレームワークを導入します。
- キューの TX TX 数の制限と Iroha の初期化を修正しました。
- 構造体でハッシュをラップします。
- ログレベルの改善:
  - 情報レベルのログをコンセンサスに追加します。
  - ネットワーク通信ログをトレース レベルとしてマークします。
  - ブロック ベクターは重複しており、ログにすべてのブロックチェーンが表示されているため、WSV からブロック ベクターを削除します。
  - 情報ログレベルをデフォルトとして設定します。
- 検証のために変更可能な WSV 参照を削除します。
- ハイムのバージョンが増加します。
- デフォルトの信頼できるピアを構成に追加します。
- クライアント API の http への移行。
- CLI に転送 isi を追加します。
- Iroha ピア関連の命令の構成。
- 欠落している ISI 実行メソッドとテストの実装。
- URL クエリパラメータの解析
- `HttpResponse::ok()`、`HttpResponse::upgrade_required(..)` を追加
- 古い命令およびクエリ モデルを Iroha DSL アプローチに置き換えます。
- BLS 署名のサポートを追加します。
- http サーバー クレートを導入します。
- symlink を使用して libssl.so.1.0.0 にパッチを適用しました。
- トランザクションのアカウント署名を検証します。
- トランザクション ステージのリファクタリング。
- 初期ドメインの改善。
- DSL プロトタイプを実装します。
- Torii ベンチマークの改善: ベンチマークでのログ記録を無効にし、成功率アサートを追加しました。
- テスト カバレッジ パイプラインの改善: `tarpaulin` を `grcov` に置き換え、テスト カバレッジ レポートを `codecov.io` に発行します。
- RTD テーマを修正しました。
- iroha サブプロジェクトの配信成果物。
- `SignedQueryRequest` を導入します。
- 署名検証に関するバグを修正しました。
- ロールバック トランザクションのサポート。
- 生成されたキーペアを json として出力します。
- `Secp256k1` キーペアをサポートします。
- さまざまな暗号化アルゴリズムの初期サポート。
- DEX機能。
- ハードコードされた設定パスを cli パラメータに置き換えます。
- ベンチマスターワークフローの修正。
- Docker イベント接続テスト。
- Iroha モニター ガイドおよび CLI。
- イベント cli の改善。
- イベントフィルター。
- イベント接続。
- マスターワークフローを修正しました。
- iroha2のRTD。
- ブロックトランザクションのマークルツリールートハッシュ。
- Docker Hubへの公開。
- メンテナンス接続用の CLI 機能。
- メンテナンス接続用の CLI 機能。
- Eprintln でマクロをログに記録します。- ログの改善。
- IR-802 ステータスの変更をブロックするサブスクリプション。
- トランザクションとブロックのイベント送信。
- Sumeragi メッセージ処理をメッセージ実装に移動します。
- 一般的な接続メカニズム。
- no-std クライアントの Iroha ドメイン エンティティを抽出します。
- トランザクション TTL。
- ブロック構成ごとの最大トランザクション数。
- 無効化されたブロックのハッシュを保存します。
- ブロックをバッチで同期します。
- 接続機能の構成。
- Iroha 機能に接続します。
- ブロック検証の修正。
- ブロック同期: 図。
- Iroha 機能に接続します。
- ブリッジ: クライアントを削除します。
- ブロック同期。
- ピア ISI の追加。
- コマンドから命令への名前変更。
- シンプルなメトリクスエンドポイント。
- ブリッジ: 登録されたブリッジと外部アセットを取得します。
- Docker パイプラインでテストを作成します。
- 投票数が足りません Sumeragi テスト。
- ブロックチェーン。
- ブリッジ: 手動の外部転送処理。
- シンプルなメンテナンスエンドポイント。
- serde-json への移行。
- デミントISI。
- ブリッジ クライアント、AddSignatory ISI、および CanAddSignatory 権限を追加します。
- Sumeragi: セット b のピア関連の TODO 修正。
- Sumeragi にサインインする前にブロックを検証します。
- 外部資産の橋渡し。
- Sumeragi メッセージの署名検証。
- バイナリ アセット ストア。
- PublicKey エイリアスをタイプに置き換えます。
- 公開用のクレートを準備します。
- NetworkTopology 内の最小投票ロジック。
- TransactionReceipt 検証のリファクタリング。
- OnWorldStateViewChange トリガーの変更: 命令の代わりに IrohaQuery。
- NetworkTopology での構築と初期化を分離します。
- Iroha イベントに関連する Iroha 特別な命令を追加。
- ブロック作成タイムアウトの処理。
- 用語集と Iroha モジュールの追加方法に関するドキュメント。
- ハードコードされたブリッジ モデルを元の Iroha モデルに置き換えます。
- NetworkTopology 構造体を導入します。
- 命令からの変換を伴う許可エンティティを追加します。
- Sumeragi メッセージ モジュール内のメッセージ。
- Kuraのジェネシスブロック機能。
- Iroha クレートの README ファイルを追加。
- ブリッジおよび RegisterBridge ISI。
- Iroha を使用した最初の作業では、リスナーが変更されます。
- OOB ISI への許可チェックの挿入。
- Docker 複数のピアを修正しました。
- ピアツーピア Docker の例。
- 取引の受領処理。
- Iroha 権限。
- Dex用のモジュールとBridge用の木箱。
- 複数のピアによるアセット作成による統合テストを修正しました。
- EC-S- への資産モデルの再実装。
- タイムアウト処理をコミットします。
- ブロックヘッダー。
- ドメイン エンティティの ISI 関連メソッド。
- Kura Mode の列挙と Trusted Peers の構成。
- ドキュメントのリンティング ルール。
- CommittedBlock を追加します。
- 蔵を `sumeragi` から切り離します。
- ブロック作成前にトランザクションが空でないことを確認してください。
- Iroha 特別命令を再実装します。
- トランザクションのベンチマークとブロック遷移。
- トランザクションのライフサイクルと状態が再加工されました。
- ライフサイクルと状態をブロックします。
- 検証のバグ、`sumeragi` ループ サイクルが block_build_time_ms 構成パラメーターと同期される問題を修正しました。
- `sumeragi` モジュール内の Sumeragi アルゴリズムのカプセル化。
- チャネル経由で実装された Iroha ネットワーク クレートのモッキング モジュール。
- async-std API への移行。
- ネットワークモック機能。
- 非同期関連のコードのクリーンアップ。
- トランザクション処理ループのパフォーマンスの最適化。
- キーペアの生成は、Iroha 開始から抽出されました。
- Iroha 実行可能ファイルの Docker パッケージ化。- Sumeragi 基本シナリオを紹介します。
- Iroha CLI クライアント。
- ベンチグループ実行後のirohaのドロップ。
- `sumeragi` を統合します。
- `sort_peers` 実装を、前のブロック ハッシュでシードされたランド シャッフルに変更します。
- ピア モジュールのメッセージ ラッパーを削除します。
- `torii::uri` および `iroha_network` 内にネットワーク関連の情報をカプセル化します。
- ハードコード処理の代わりに実装されたピア命令を追加します。
- 信頼できるピア リストを介したピア通信。
- Torii 内で処理されるネットワーク リクエストのカプセル化。
- 暗号モジュール内の暗号ロジックのカプセル化。
- ペイロードとしてタイムスタンプと以前のブロック ハッシュを使用したブロック署名。
- 暗号化関数はモジュールの最上位に配置され、署名にカプセル化された ursa 署名者と連携します。
- Sumeragi 初期値。
- ストアにコミットする前に、ワールド ステート ビュー クローンでのトランザクション命令を検証します。
- 取引受諾時の署名を確認します。
- リクエストのデシリアライゼーションのバグを修正。
- Iroha 署名の実装。
- コードベースをクリーンアップするためにブロックチェーン エンティティが削除されました。
- トランザクション API の変更: リクエストの作成と操作が改善されました。
- トランザクションの空のベクトルを持つブロックを作成するバグを修正
- 保留中のトランザクションを転送します。
 - u128 Norito でエンコードされた TCP パケットの欠落バイトに関するバグを修正しました。
- メソッドトレース用の属性マクロ。
- P2pモジュール。
- 鳥居とクライアントでのiroha_networkの使用。
- 新しい ISI 情報を追加します。
- ネットワーク状態の特定のタイプのエイリアス。
- Box<dyn Error> が文字列に置き換えられました。
- ネットワークリッスンステートフル。
- トランザクションの初期検証ロジック。
- Iroha_network クレート。
- Io、IntoContract、および IntoQuery 特性の派生マクロ。
- Iroha クライアントの実装をクエリします。
- コマンドの ISI コントラクトへの変換。
- 条件付きマルチシグの提案された設計を追加します。
- Cargo ワークスペースへの移行。
- モジュールの移行。
- 環境変数による外部構成。
- Torii の Get および Put リクエストの処理。
- Github ci の修正。
- Cargo-make はテスト後にブロックをクリーンアップします。
- ディレクトリをブロックでクリーンアップする機能を備えた `test_helper_fns` モジュールを導入します。
- マークル ツリーによる検証を実装します。
- 未使用の派生を削除します。
- async/await を伝播し、待機されていない `wsv::put` を修正します。
- `futures` クレートからの結合を使用します。
- 並列ストア実行の実装: ディスクへの書き込みと WSV の更新が並列して行われます。
- (逆) シリアル化には、所有権の代わりに参照を使用します。
- ファイルからのコードの排出。
- ursa::blake2 を使用します。
- 投稿ガイドの mod.rs に関するルール。
- ハッシュ 32 バイト。
- Blake2 ハッシュ。
- ディスクはブロックへの参照を受け入れます。
- コマンドモジュールと初期マークルツリーのリファクタリング。
- リファクタリングされたモジュール構造。
- 正しい書式設定。
- read_all にドキュメント コメントを追加します。
- `read_all` を実装し、ストレージ テストを再編成し、非同期関数を使用したテストを非同期テストに変換します。
- 不要な可変キャプチャを削除します。
- 問題を確認し、クリッピーを修正します。
- ダッシュを削除します。
- フォーマットチェックを追加しました。
- トークンを追加します。
- Githubアクション用のrust.ymlを作成します。
- ディスクストレージのプロトタイプを紹介します。
- 資産のテストと機能を転送します。
- デフォルトの初期化子を構造体に追加します。
- MSTCache 構造体の名前を変更します。
- 借り忘れを追加します。
- iroha2 コードの初期概要。
- 初期の Kura API。
- いくつかの基本ファイルを追加し、iroha v2 のビジョンを概説するホワイトペーパーの初稿もリリースします。
- 基本的なiroha v2ブランチ。

## [1.5.0] - 2022-04-08

### CI/CD の変更
- Jenkinsfile と JenkinsCI を削除します。

＃＃＃ 追加した- Burrow 用の RocksDB ストレージ実装を追加。
- ブルームフィルターによるトラフィック最適化の導入
- `MST` モジュール ネットワークが `batches_cache` の `OS` モジュールに配置されるように更新します。
- トラフィックの最適化を提案します。

### ドキュメント

- ビルドを修正。 DB の違い、移行の実践、ヘルスチェック エンドポイント、iroha-swarm ツールに関する情報を追加しました。

### その他

- ドキュメントビルドの要件を修正。
- 残りの重要なフォローアップ項目に焦点を当てるために、リリース ドキュメントをトリミングします。
- 「docker イメージが存在するかどうかを確認する」/build all stop_testing を修正しました。
- /build all スキップ_テスト。
- /build スキップ_テスト;そしてその他のドキュメント。
- `.github/_README.md` を追加します。
- `.packer` を削除します。
- テストパラメータの変更を削除します。
- 新しいパラメータを使用してテスト段階をスキップします。
- ワークフローに追加します。
- リポジトリのディスパッチを削除します。
- リポジトリのディスパッチを追加します。
- テスター用のパラメーターを追加します。
- `proposal_delay` タイムアウトを削除します。

## [1.4.0] - 2022-01-31

### 追加

- 同期ノード状態を追加
- RocksDB のメトリクスを追加
- http およびメトリクス経由のヘルスチェック インターフェイスを追加します。

### 修正

- Iroha v1.4-rc.2 の列ファミリーを修正
- Iroha v1.4-rc.1 に 10 ビット ブルーム フィルターを追加

### ドキュメント

- zip と pkg-config をビルド配備のリストに追加します。
- Readme の更新: ビルド ステータス、ビルド ガイドなどへの壊れたリンクを修正します。
- 構成と Docker メトリクスを修正しました。

### その他

- GHA docker タグを更新します。
- g++11 でコンパイルするときの Iroha 1 コンパイル エラーを修正しました。
- `max_rounds_delay` を `proposal_creation_timeout` に置き換えます。
- サンプル構成ファイルを更新して、古い DB 接続パラメータを削除します。