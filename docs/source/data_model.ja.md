<!-- Japanese translation of docs/source/data_model.md -->

---
lang: ja
direction: ltr
source: docs/source/data_model.md
status: complete
translator: manual
---

# Iroha v2 データモデル徹底解説

本ドキュメントは `iroha_data_model` クレートで実装され、ワークスペース全体で利用される構造体、識別子、トレイト、プロトコルを解説します。精確なリファレンスとして活用し、必要に応じて更新提案に利用してください。

## スコープと基礎

- **目的**: ドメイン／アカウント／アセット／NFT／ロール／権限／ピアといったドメインオブジェクトや、状態変更命令（ISI）、クエリ、トリガー、トランザクション、ブロック、パラメータの正規型を提供します。
- **シリアライズ**: すべての公開型は Norito コーデック（`Encode`, `Decode`）とスキーマ（`IntoSchema`）を実装。HTTP や `Json` ペイロードでは JSON（フィーチャ制御）を使用します。
- **IVM 注意**: IVM ターゲットでは一部のデシリアライズ検証が無効化され、ホストが事前検証します。
- **FFI**: `iroha_ffi` 属性は `ffi_export`/`ffi_import` フィーチャで切り替え、不要なオーバーヘッドを避けます。

## 主要トレイトとヘルパー

- `Identifiable`: 安定した `Id` を返す。`IdEqOrdHash` を derive し、セット／マップで扱いやすくします。
- `Registrable` / `Registered`: ビルダーパターンと登録済み型を結ぶ。`Registered` は軽量な `With` ビルダー型と実体を関連付けます。
- `HasMetadata`: キー／バリュー型の `Metadata` に統一アクセス。
- `IntoKeyValue`: ストレージで `Key` と `Value` を分離保存するヘルパー。
- `Owned<T>` / `Ref<'world, K, V>`: 余計なコピーを避ける薄いラッパー。

## 名前と ID

- `Name`: 有効な識別子。`@`, `#`, `$` を禁止し Unicode NFC に正規化。`genesis` は予約語。
- `IdBox`: 任意の ID（`DomainId`, `AccountId`, …）を包む和型。
- `ChainId`: トランザクションのリプレイ防止用。

**文字列表現 (Display/FromStr 往復可)**
- `DomainId`: `wonderland`
- `AccountId`: 正規のアドレスは `AccountAddress` が提供する IH58 / Sora 圧縮（`snx1…`）/ 16 進表現（`canonical_hex`）コードックを利用する。IH58 を推奨し、`snx1…` は Sora 専用の次善策とする。`alias@domain` 形式はルーティング用エイリアスとして維持される。Torii は `AccountAddress::parse_any` で入力を正規バイト列に揃える。
- `AssetDefinitionId`: `asset#domain`
- `AssetId`: `asset#domain#account@domain`（同一ドメインなら `asset##account@domain`）
- `NftId`: `nft$domain`
- `PeerId`: 公開鍵文字列

## 主なエンティティ

- **Domain**: `Domain { id: DomainId, logo: Option<IpfsPath>, metadata, owned_by }`
- **Account**: `Account { id: AccountId, metadata, label?, uaid? }` — `label` はリキー用の安定ラベル、`uaid` は Nexus のユニバーサルアカウント ID（任意）です。
- **AssetDefinition**: `AssetDefinition { id, spec, mintable, logo, metadata, owned_by, total_quantity }`
- **Asset**: `Asset { id: AssetId, value: Numeric }`
- **NFT**: `Nft { id: NftId, content: Metadata, owned_by }`
- **Role**: `Role { id: RoleId, permissions }`
- **Permission**: `Permission { name: Ident, payload: Json }`
- **Peer**: `Peer { id: PeerId, address: SocketAddr }`
- **Trigger**: `Trigger { id: TriggerId, action: action::Action }`

## メタデータ

- `Metadata(BTreeMap<Name, Json>)`: 多くのエンティティが保持。`contains`, `get`, `insert` 等を提供します。

## 命令 (Instruction)

- `InstructionBox`: すべての命令をラップ。`Register`, `Mint`, `Transfer`, `GrantRole` など。
- `CustomInstruction`: エグゼキュータ固有の拡張に用いる Json ラッパー（公開ネットワークでは推奨されません）。

## クエリ

- `QueryBox`: `FindTransactions`, `FindDomainById` 等。
- `QueryRequest` は `Singular` / `Start` / `Continue`、`QueryResponse` は `Singular` / `Iterable`。カーソル処理向けに `ForwardCursor` を使用。
- DSL (`query::dsl`) はコンパイル時に投影・選択を検証。`fast_dsl` フィーチャで軽量化可能。

## トランザクションとブロック

- `Transaction` / `VersionedTransaction`: ノンスや TTL、署名を保持。
- `SignedTransaction`: `ChainId` を含みリプレイ耐性を提供。
- `BlockHeader` / `Block`: ブロックメタデータとトランザクション集合。
- `SignedBlock`: `signatures: BTreeSet<BlockSignature)`、`BlockPayload { header, transactions }`、そして `BlockResult` を保持する。`BlockResult` には `time_triggers`、エントリ／結果の Merkle 木、`transaction_results` に加えて `fastpq_transcripts: BTreeMap<Hash, Vec<TransferTranscript>>` が含まれ、FASTPQ 転送トランスクリプトがブロック結果として永続化される。
- Torii から配信される `ExecWitness` には `fastpq_transcripts` に加えて、`public_inputs`（dsid / slot / old/new ルート / perm_root / tx_set_hash）を内包した `fastpq_batches: Vec<FastpqTransitionBatch>` も含まれ、外部プローバーはホスト側で再エンコードすることなく正規化済みの FASTPQ 行を取得できる。
- ユーティリティ: `presigned`, `set_transaction_results(...)`, `set_transaction_results_with_transcripts(...)`, `header()`, `signatures()`, `hash()`, `add_signature()`, `replace_signatures()`。

## イベント

- `DataEvent`（例: `AccountEvent`, `AssetEvent`）と `PipelineEvent`（パイプライン／ランタイムイベント）。

## トリガー

- `Trigger` は `action::Action` を内包し、スケジュール／条件に応じて実行。

## パラメータ

- `Parameter` / `CustomParameter` によりチェーン構成を管理。`CustomParameters` はエグゼキュータ拡張向け。

## エグゼキュータと拡張性

- `Executor { bytecode }`: バリデータが実行するバイトコード。
- `ExecutorDataModel { parameters, instructions, permissions, schema }`: カスタムパラメータ・命令・権限・スキーマを宣言。
- `CustomInstruction` はエグゼキュータ固有の ISI を包む用途。公開ネットワークでは新規 ISI の追加を優先すべきです。

## フィーチャと決定論

- 主なフィーチャ: `std`, `json`, `transparent_api`, `ffi_export/import`, `fast_dsl`, `http`, `fault_injection`。
- Norito によるシリアライズでハードウェアに依存しない決定論を保証。IVM バイトコードも決定論的に実行されるよう設計します。

### Transparent API (`transparent_api`)

- 目的: Torii やエグゼキュータ、結合テストなど内部コンポーネント向けに `#[model]` 構造体/列挙体のフィールドを完全公開します。無効時は意図的に不透明化され、外部 SDK は安全なコンストラクタやエンコード済みペイロードを介してのみ扱います。
- 仕組み: `iroha_data_model_derive::model` マクロが各公開フィールドに `#[cfg(feature = "transparent_api")] pub` を差し込み、デフォルトビルドでは private 版を保持します。フィーチャを有効化すると cfg が切り替わり、`Account` や `Domain`、`Asset` などをモジュール外でも分解できます。
- 検出: クレートは `TRANSPARENT_API: bool` 定数をエクスポートします（`transparent_api.rs` / `non_transparent_api.rs` で生成）。下流コードはこのフラグを確認して、不透明 API へのフォールバックが必要かを判断できます。
- 有効化: 依存先の `Cargo.toml` に `features = ["transparent_api"]` を追加します。JSON 射影が必要なワークスペース内クレート（例: `iroha_torii`）ではすでに転送されていますが、第三者クライアントは API 表面が広がることを理解したうえでのみ有効化してください。

## サンプル

```rust
use iroha_data_model::prelude::*;
use iroha_crypto::KeyPair;
use iroha_primitives::numeric::Numeric;

let domain_id: DomainId = "wonderland".parse().unwrap();
let new_domain = Domain::new(domain_id.clone()).with_metadata(Metadata::default());

let kp = KeyPair::random();
let account_id = AccountId::new(domain_id.clone(), kp.public_key().clone());
let new_account = Account::new(account_id.clone()).with_metadata(Metadata::default());

let asset_def_id: AssetDefinitionId = "xor#wonderland".parse().unwrap();
let new_asset_def = AssetDefinition::numeric(asset_def_id.clone())
    .with_metadata(Metadata::default());
let asset_id = AssetId::new(asset_def_id.clone(), account_id.clone());
let asset = Asset::new(asset_id.clone(), Numeric::from(100));

let chain_id: ChainId = "dev-chain".parse().unwrap();
let tx = TransactionBuilder::new(chain_id, account_id.clone())
    .with_instructions(vec![ /* Register / Mint / Transfer */ ])
    .sign(kp.private_key());
```

クエリ DSL 例:

```rust
use iroha_data_model::prelude::*;
// `FindAccounts` や `FindAssets` に DSL を適用
```
