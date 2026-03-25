<!-- Japanese translation of docs/source/data_model.md -->

---
lang: ja
direction: ltr
source: docs/source/data_model.md
status: complete
translator: machine-google-reviewed
source_hash: 683bfb31442f8f4ce7b1bf5038f9dba92fe092545e655f43b51195c21535d3c4
source_last_modified: "2026-03-12T11:24:23.059339+00:00"
translation_last_reviewed: 2026-03-12
---

# Iroha v2 データ モデル – 詳細

このドキュメントでは、`iroha_data_model` クレートに実装され、ワークスペース全体で使用される Iroha v2 データ モデルを形成する構造、識別子、特性、およびプロトコルについて説明します。これは、レビューして更新を提案できる正確なリファレンスとなることを目的としています。

## 範囲と基礎

- 目的: ドメイン オブジェクト (ドメイン、アカウント、資産、NFT、ロール、権限、ピア)、状態変更命令 (ISI)、クエリ、トリガー、トランザクション、ブロック、パラメーターの正規タイプを提供します。
- シリアル化: すべてのパブリック型は、Norito コーデック (`norito::codec::{Encode, Decode}`) およびスキーマ (`iroha_schema::IntoSchema`) を派生します。 JSON は、機能フラグの背後で選択的に (HTTP および `Json` ペイロードなどに) 使用されます。
- IVM 注: Iroha 仮想マシン (IVM) をターゲットにする場合、ホストはコントラクトを呼び出す前に検証を実行するため、特定の逆シリアル化時の検証は無効になります (`src/lib.rs` のクレート ドキュメントを参照)。
- FFI ゲート: FFI が必要ない場合のオーバーヘッドを回避するために、一部の型には `ffi_export`/`ffi_import` の背後にある `iroha_ffi` を介して FFI に対して条件付きのアノテーションが付けられます。

## コア特性とヘルパー- `Identifiable`: エンティティには安定した `Id` および `fn id(&self) -> &Self::Id` があります。マップ/セットをわかりやすくするために、`IdEqOrdHash` で派生する必要があります。
- `Registrable`/`Registered`: 多くのエンティティ (`Domain`、`AssetDefinition`、`Role` など) はビルダー パターンを使用します。 `Registered` は、ランタイム型を登録トランザクションに適した軽量ビルダー型 (`With`) に結び付けます。
- `HasMetadata`: キー/値 `Metadata` マップへの統合アクセス。
- `IntoKeyValue`: 重複を減らすために、`Key` (ID) と `Value` (データ) を別々に保存するストレージ分割ヘルパー。
- `Owned<T>`/`Ref<'world, K, V>`: 不要なコピーを避けるためにストレージとクエリ フィルターで使用される軽量ラッパー。

## 名前と識別子- `Name`: 有効なテキスト識別子。空白文字と予約文字 `@`、`#`、`$` (複合 ID で使用) は許可されません。検証付きの `FromStr` 経由で構築可能。名前は解析時に Unicode NFC に正規化されます (標準的に同等のスペルは同一として扱われ、合成されて保存されます)。特別な名前 `genesis` は予約されています (大文字と小文字を区別せずにチェックされます)。
- `IdBox`: サポートされている ID の合計タイプのエンベロープ (`DomainId`、`AccountId`、`AssetDefinitionId`、`AssetId`、`NftId`、`PeerId`、 `TriggerId`、`RoleId`、`Permission`、`CustomParameterId`)。単一タイプとしての汎用フローおよび Norito エンコードに役立ちます。
- `ChainId`: トランザクションのリプレイ保護に使用される不透明なチェーン識別子。文字列形式の ID (`Display`/`FromStr` とラウンドトリップ可能):
- `DomainId`: `name` (例: `wonderland`)。
- `AccountId`: `AccountAddress` を介して I105 のみとしてエンコードされた正規のドメインレス アカウント識別子。パーサー入力は正規の I105 である必要があります。ドメイン サフィックス (`@domain`)、正規 I105 リテラル、エイリアス リテラル、正規 16 進パーサー入力、レガシー `norito:` ペイロード、および `uaid:`/`opaque:` アカウント パーサー フォームは拒否されます。
- `AssetDefinitionId`: 正規の `unprefixed Base58 address with versioning and checksum` (UUID-v4 バイト)。
- `AssetId`: 正規エンコードされたリテラル `<asset-definition-id>#<account-id>` (従来のテキスト形式は最初のリリースではサポートされていません)。
- `NftId`: `nft$domain` (例: `rose$garden`)。
- `PeerId`: `public_key` (ピアの同等性は公開キーによって決まります)。

## エンティティ

### ドメイン
- `DomainId { name: Name }` – 一意の名前。
- `Domain { id, logo: Option<SorafsUri>, metadata: Metadata, owned_by: AccountId }`。
- ビルダー: `NewDomain` と `with_logo`、`with_metadata`、その後 `Registrable::build(authority)` は `owned_by` を設定します。### アカウント
- `AccountId` は、コントローラによってキー設定され、正規の I105 としてエンコードされた正規のドメインレス アカウント ID です。
- `ScopedAccountId { account: AccountId, domain: DomainId }` は、スコープ付きビューが必要な場合にのみ明示的なドメイン コンテキストを伝えます。
- `Account { id, metadata, label?, uaid? }` — `label` は、キー再生成レコードで使用されるオプションの安定したエイリアスです。`uaid` は、オプションの Nexus 全体の [ユニバーサル アカウント ID](./universal_accounts_guide.md) を保持します。
- ビルダー: `Account::new(id)` 経由の `NewAccount`。登録には明示的な `ScopedAccountId` ドメインが必要であり、デフォルトからは推測されません。

### 資産の定義と資産
- `AssetDefinitionId { aid_bytes: [u8; 16] }` はテキストで `unprefixed Base58 address` として公開されます。
- `AssetDefinition { id, name, description?, alias?, spec: NumericSpec, mintable: Mintable, logo: Option<SorafsUri>, metadata, owned_by: AccountId, total_quantity: Numeric }`。

  - Torii asset-definition responses may include `alias_binding { alias, status, lease_expiry_ms, grace_until_ms, bound_at_ms }`; alias selectors resolve against latest committed block time and stop resolving after grace, while direct reads may still show `expired_pending_cleanup` until sweep.
  - `name` は人に向けた表示テキストが必要であり、`#`/`@` を含めることはできません。
  - `alias` はオプションであり、次のいずれかである必要があります。
    - `<name>#<domain>.<dataspace>`
    - `<name>#<dataspace>`
    左側のセグメントは `AssetDefinition.name` と完全に一致します。
  - `Mintable`: `Infinitely` | `Once` | `Limited(u32)` | `Not`。
  - ビルダー: `AssetDefinition::new(id, spec)` またはコンビニエンス `numeric(id)`; `name` は必須であり、`.with_name(...)` 経由で設定する必要があります。
- `AssetId { account: AccountId, definition: AssetDefinitionId, scope: AssetBalanceScope }`。
- `Asset { id, value: Numeric }` とストレージに優しい `AssetEntry`/`AssetValue`。
- `AssetBalanceScope`: 無制限の残高の場合は `Global`、データスペース制限のある残高の場合は `Dataspace(DataSpaceId)`。
- `AssetTotalQuantityMap = BTreeMap<AssetDefinitionId, Numeric>` は概要 API 用に公開されました。### NFT
- `NftId { domain: DomainId, name: Name }`。
- `Nft { id, content: Metadata, owned_by: AccountId }` (コンテンツは任意のキー/値メタデータです)。
- ビルダー: `Nft::new(id, content)` 経由の `NewNft`。

### 役割と権限
- `RoleId { name: Name }`。
- `Role { id, permissions: BTreeSet<Permission> }` とビルダー `NewRole { inner: Role, grant_to: AccountId }`。
- `Permission { name: Ident, payload: Json }` – `name` およびペイロード スキーマは、アクティブな `ExecutorDataModel` と一致する必要があります (下記を参照)。

### ピア
- `PeerId { public_key: PublicKey }`。
- `Peer { address: SocketAddr, id: PeerId }` および解析可能な `public_key@address` 文字列形式。

### 暗号化プリミティブ (機能 `sm`)
- `Sm2PublicKey` および `Sm2Signature`: SM2 の SEC1 準拠ポイントおよび固定幅 `r∥s` 署名。コンストラクターは曲線のメンバーシップと識別 ID を検証します。 Norito エンコードは、`iroha_crypto` で使用される正規表現を反映しています。
- `Sm3Hash`: `[u8; 32]` GM/T 0004 ダイジェストを表すニュータイプ。マニフェスト、テレメトリ、およびシステムコール応答で使用されます。
- `Sm4Key`: ホスト システムコールとデータモデル フィクスチャ間で共有される 128 ビット対称キー ラッパー。
これらのタイプは既存の Ed25519/BLS/ML-DSA プリミティブと並んで配置され、ワー​​クスペースが `--features sm` で構築されるとパブリック スキーマの一部になります。### トリガーとイベント
- `TriggerId { name: Name }` および `Trigger { id, action: action::Action }`。
- `action::Action { executable: Executable, repeats: Repeats, authority: AccountId, filter: EventFilterBox, metadata }`。
  - `Repeats`: `Indefinitely` または `Exactly(u32)`;注文および枯渇ユーティリティが含まれています。
  - 安全性: `TriggerCompleted` はアクションのフィルターとして使用できません (シリアル化 (逆) 中に検証されます)。
- `EventBox`: パイプライン、パイプライン バッチ、データ、時間、トリガー実行、およびトリガー完了イベントの合計タイプ。 `EventFilterBox` は、サブスクリプションとトリガー フィルターのそれを反映しています。

## パラメータと設定

- システム パラメーター ファミリ (すべて `Default`ed、ゲッターを実行、個別の列挙型に変換):
- `SumeragiParameters { block_time_ms, commit_time_ms, min_finality_ms, pacing_factor_bps, max_clock_drift_ms, collectors_k, collectors_redundant_send_r }`。
  - `BlockParameters { max_transactions: NonZeroU64 }`。
  - `TransactionParameters { max_signatures, max_instructions, ivm_bytecode_size, max_tx_bytes, max_decompressed_bytes }`。
  - `SmartContractParameters { fuel, memory, execution_depth }`。
- `Parameters` はすべてのファミリーと `custom: BTreeMap<CustomParameterId, CustomParameter>` をグループ化します。
- 単一パラメータ列挙型: `SumeragiParameter`、`BlockParameter`、`TransactionParameter`、`SmartContractParameter` (差分形式の更新と反復用)。
- カスタム パラメータ: エグゼキュータによって定義され、`Json` として伝送され、`CustomParameterId` (`Name`) によって識別されます。

## ISI (Iroha 特別な命令)- コア特性: `dyn_encode`、`as_any`、および安定した型ごとの識別子 `id()` (デフォルトは具体的な型名) を備えた `Instruction`。すべての命令は `Send + Sync + 'static` です。
- `InstructionBox`: タイプ ID + エンコードされたバイトを介して実装された clone/eq/ord を持つ所有の `Box<dyn Instruction>` ラッパー。
- 組み込み命令ファミリーは次のように編成されています。
  - `mint_burn`、`transfer`、`register`、および `transparent` ヘルパーのバンドル。
  - メタ フローの列挙型: `InstructionType`、`SetKeyValueBox` のようなボックス化された合計 (domain/account/asset_def/nft/trigger)。
- エラー: `isi::error` に基づく豊富なエラー モデル (評価タイプのエラー、エラーの検索、ミント可能性、数学、無効なパラメーター、繰り返し、不変条件)。
- 命令レジストリ: `instruction_registry!{ ... }` マクロは、型名をキーとしたランタイム デコード レジストリを構築します。 `InstructionBox` クローンおよび Norito serde によって動的な (逆) シリアル化を実現するために使用されます。 `set_instruction_registry(...)` 経由でレジストリが明示的に設定されていない場合は、バイナリの堅牢性を保つために、すべてのコア ISI を含む組み込みのデフォルト レジストリが最初の使用時に遅延インストールされます。

## トランザクション- `Executable`: `Instructions(ConstVec<InstructionBox>)` または `Ivm(IvmBytecode)` のいずれか。 `IvmBytecode` は、base64 としてシリアル化されます (`Vec<u8>` に対する透過的な newtype)。
- `TransactionBuilder`: `chain`、`authority`、`creation_time_ms`、オプションの `time_to_live_ms` および `nonce`、`metadata`、および`Executable`。
  - ヘルパー: `with_instructions`、`with_bytecode`、`with_executable`、`with_metadata`、`set_nonce`、`set_ttl`、`set_creation_time`、`sign`。
- `SignedTransaction` (`iroha_version` でバージョン管理): `TransactionSignature` とペイロードを伝送します。ハッシュと署名の検証を提供します。
- エントリポイントと結果:
  - `TransactionEntrypoint`: `External(SignedTransaction)` | `Time(TimeTriggerEntrypoint)`。
  - `TransactionResult` = `Result<DataTriggerSequence, TransactionRejectionReason>` (ハッシュ ヘルパーあり)。
  - `ExecutionStep(ConstVec<InstructionBox>)`: トランザクション内の単一の命令バッチ。

## ブロック- `SignedBlock` (バージョン付き) は以下をカプセル化します。
  - `signatures: BTreeSet<BlockSignature>` (バリデーターから)、
  - `payload: BlockPayload { header: BlockHeader, transactions: Vec<SignedTransaction> }`、
  - `time_triggers`、エントリ/結果マークル ツリー、`transaction_results`、および `fastpq_transcripts: BTreeMap<Hash, Vec<TransferTranscript>>` を含む `result: BlockResult` (二次実行状態)。
- ユーティリティ: `presigned`、`set_transaction_results(...)`、`set_transaction_results_with_transcripts(...)`、`header()`、`signatures()`、`hash()`、`add_signature`、`replace_signatures`。
- マークル ルート: トランザクションのエントリポイントと結果はマークル ツリーを介してコミットされます。結果、マークル ルートがブロック ヘッダーに配置されます。
- ブロック包含証明 (`BlockProofs`) は、エントリ/結果マークル証明と `fastpq_transcripts` マップの両方を公開するため、オフチェーン証明者はトランザクション ハッシュに関連付けられた転送デルタをフェッチできます。
- `ExecWitness` メッセージ (Torii 経由でストリーミングされ、コンセンサス ゴシップに便乗) には、`fastpq_transcripts` と、`public_inputs` (dsid、スロット、ルート、perm_root、 tx_set_hash) なので、外部証明者はトランスクリプトを再エンコードせずに正規の FASTPQ 行を取り込むことができます。

## クエリ- 2 つのフレーバー:
  - 単数: `SingularQuery<Output>` を実装します (例: `FindParameters`、`FindExecutorDataModel`)。
  - 反復可能: `Query<Item>` (例: `FindAccounts`、`FindAssets`、`FindDomains` など) を実装します。
- タイプ消去されたフォーム:
  - `QueryBox<T>` は、グローバル レジストリによってバックアップされた Norito serde を備えた、ボックス化され消去された `Query<Item = T>` です。
  - `QueryWithFilter<T> { query, predicate, selector }` はクエリを DSL 述語/セレクターと組み合わせます。 `From` を介して、消去された反復可能なクエリに変換されます。
- レジストリとコーデック:
  - `query_registry!{ ... }` は、動的デコードのために型名によって具体的なクエリ型をコンストラクターにマッピングするグローバル レジストリを構築します。
  - `QueryRequest = Singular(SingularQueryBox) | Start(QueryWithParams) | Continue(ForwardCursor)` および `QueryResponse = Singular(..) | Iterable(QueryOutput)`。
  - `QueryOutputBatchBox` は、均一なベクトル (`Vec<Account>`、`Vec<Name>`、`Vec<AssetDefinition>`、`Vec<BlockHeader>`) の合計タイプであり、効率的なページネーションのためのタプルと拡張ヘルパーが追加されています。
- DSL: コンパイル時にチェックされる述語とセレクター用のプロジェクション特性 (`HasProjection<PredicateMarker>` / `SelectorMarker`) を備えた `query::dsl` に実装されました。 `fast_dsl` 機能は、必要に応じて軽量のバリアントを公開します。

## エグゼキューターと拡張性- `Executor { bytecode: IvmBytecode }`: バリデーターによって実行されたコード バンドル。
- `ExecutorDataModel { parameters: CustomParameters, instructions: BTreeSet<Ident>, permissions: BTreeSet<Ident>, schema: Json }` は、エグゼキューター定義のドメインを宣言します。
  - カスタム設定パラメータ、
  - カスタム命令識別子、
  - 許可トークン識別子、
  - クライアント ツールのカスタム タイプを記述する JSON スキーマ。
- カスタマイズ サンプルは `data_model/samples/executor_custom_data_model` の下にあり、以下を示しています。
  - `iroha_executor_data_model::permission::Permission` 派生によるカスタム許可トークン、
  - `CustomParameter`に変換可能な型として定義されたカスタムパラメータ、
  - 実行のために `CustomInstruction` にシリアル化されたカスタム命令。

### Customstruct (エグゼキュータ定義の ISI)- タイプ: `isi::CustomInstruction { payload: Json }` (安定したワイヤ ID `"iroha.custom"`)。
- 目的: パブリック データ モデルをフォークすることなく、プライベート/コンソーシアム ネットワークまたはプロトタイピング用の実行者固有の命令用のエンベロープ。
- デフォルトのエグゼキュータ動作: `iroha_core` の組み込みエグゼキュータは `CustomInstruction` を実行せず、遭遇するとパニックを起こします。カスタム エグゼキュータは、`InstructionBox` を `CustomInstruction` にダウンキャストし、すべてのバリデータのペイロードを確定的に解釈する必要があります。
- Norito: スキーマを含む `norito::codec::{Encode, Decode}` 経由でエンコード/デコードします。 `Json` ペイロードは決定的にシリアル化されます。ラウンドトリップは、命令レジストリに `CustomInstruction` (デフォルト レジストリの一部) が含まれている限り安定しています。
- IVM: Kotodama は IVM バイトコード (`.to`) にコンパイルされ、アプリケーション ロジックの推奨パスです。 `CustomInstruction` は、Kotodama でまだ表現できないエグゼキュータ レベルの拡張にのみ使用してください。決定性とピア全体で同一の実行バイナリを確保します。
- パブリック ネットワークには使用しないでください。異種の実行者がコンセンサス フォークのリスクを負うパブリック チェーンには使用しないでください。プラットフォーム機能が必要な場合は、新しい組み込み ISI アップストリームを提案することを優先します。

## メタデータ- `Metadata(BTreeMap<Name, Json>)`: 複数のエンティティにアタッチされたキー/値ストア (`Domain`、`Account`、`AssetDefinition`、`Nft`、トリガー、およびトランザクション)。
- API: `contains`、`iter`、`get`、`insert`、および (`transparent_api` を含む) `remove`。

## 機能と決定論

- 機能制御オプション API (`std`、`json`、`transparent_api`、`ffi_export`、`ffi_import`、`fast_dsl`、`http`、 `fault_injection`)。
- 決定性: すべてのシリアル化は、ハードウェア間で移植できるように Norito エンコーディングを使用します。 IVM バイトコードは不透明なバイト BLOB です。実行時に非決定的な削減を導入してはなりません。ホストはトランザクションを検証し、入力を IVM に決定的に供給します。

### 透過的 API (`transparent_api`)- 目的: Torii、実行プログラム、統合テストなどの内部コンポーネントの `#[model]` 構造体/列挙型への完全な変更可能なアクセスを公開します。これがないと、これらの項目は意図的に不透明になるため、外部 SDK は安全なコンストラクターとエンコードされたペイロードのみを認識します。
- メカニズム: `iroha_data_model_derive::model` マクロは、各パブリック フィールドを `#[cfg(feature = "transparent_api")] pub` で書き換え、デフォルト ビルドのプライベート コピーを保持します。この機能を有効にすると、これらの cfgs が反転されるため、`Account`、`Domain`、`Asset` などを定義モジュールの外で分割することが合法になります。
- 表面検出: クレートは `TRANSPARENT_API: bool` 定数をエクスポートします (`transparent_api.rs` または `non_transparent_api.rs` に生成されます)。ダウンストリーム コードはこのフラグをチェックし、不透明ヘルパーにフォールバックする必要がある場合に分岐できます。
- 有効化: `Cargo.toml` の依存関係に `features = ["transparent_api"]` を追加します。 JSON プロジェクションを必要とするワークスペース クレート (`iroha_torii` など) はフラグを自動的に転送しますが、サードパーティのコンシューマーは、デプロイメントを制御し、より広範な API サーフェスを受け入れる場合を除き、フラグをオフにしておく必要があります。

## 簡単な例

ドメインとアカウントを作成し、アセットを定義し、手順に従ってトランザクションを構築します。

```rust
use iroha_data_model::prelude::*;
use iroha_crypto::KeyPair;
use iroha_primitives::numeric::Numeric;

// Domain
let domain_id: DomainId = "wonderland".parse().unwrap();
let new_domain = Domain::new(domain_id.clone()).with_metadata(Metadata::default());

// Account
let kp = KeyPair::random();
let account_id = AccountId::new(kp.public_key().clone());
let new_account = Account::new(account_id.to_account_id(domain_id.clone()))
    .with_metadata(Metadata::default());

// Asset definition and an asset for the account
let asset_def_id: AssetDefinitionId = "66owaQmAQMuHxPzxUN3bqZ6FJfDa".parse().unwrap();
let new_asset_def = AssetDefinition::numeric(asset_def_id.clone())
    .with_name("USD Coin".to_owned())
    .with_metadata(Metadata::default());
let asset_id = AssetId::new(asset_def_id.clone(), account_id.clone());
let asset = Asset::new(asset_id.clone(), Numeric::from(100));

// Build a transaction with instructions (pseudo-ISI; exact ISI types live under `isi`)
let chain_id: ChainId = "dev-chain".parse().unwrap();
let tx = TransactionBuilder::new(chain_id, account_id.clone())
    .with_instructions(vec![ /* Register/ Mint/ Transfer instructions here */ ])
    .sign(kp.private_key());
```

DSL を使用してアカウントとアセットをクエリします。

```rust
use iroha_data_model::prelude::*;

let predicate = query::dsl::CompoundPredicate::build(|p| {
    p.equals("metadata.tier", 1_u32)
        .exists("metadata.display_name")
});
let selector = query::dsl::SelectorTuple::default();
let q: QueryBox<QueryOutputBatchBox> =
    QueryWithFilter::new(
        Box::new(query::account::FindAccounts),
        predicate,
        selector,
    ).into();
// Encode and send via Torii; decode on server using the query registry
```

IVM スマート コントラクト バイトコードを使用します。

```rust
use iroha_data_model::prelude::*;

let bytecode = IvmBytecode::from_compiled(include_bytes!("contract.to").to_vec());
let tx = TransactionBuilder::new("dev-chain".parse().unwrap(), account_id.clone())
    .with_bytecode(bytecode)
    .sign(kp.private_key());
```

asset-definition id / エイリアス クイック リファレンス (CLI + Torii):

```bash
# Register an asset definition with canonical Base58 id + explicit name + alias
iroha ledger asset definition register \
  --id 66owaQmAQMuHxPzxUN3bqZ6FJfDa \
  --name pkr \
  --alias pkr#ubl.sbp

# Short alias form (no owner segment): <name>#<dataspace>
iroha ledger asset definition register \
  --id 66owaQmAQMuHxPzxUN3bqZ6FJfDa \
  --name pkr \
  --alias pkr#sbp

# Mint using alias + account components (no manual norito hex copy/paste)
iroha ledger asset mint \
  --definition-alias pkr#ubl.sbp \
  --account sorauﾛ1P... \
  --quantity 500

# Resolve alias to canonical Base58 id via Torii
curl -sS http://127.0.0.1:8080/v1/assets/aliases/resolve \
  -H 'content-type: application/json' \
  -d '{"alias":"pkr#ubl.sbp"}'
```移行メモ:
- 古い `name#domain` 資産定義 ID は、v1 では受け入れられません。
- ミント/バーン/転送のアセット ID は正規の `<asset-definition-id>#<account-id>` のままです。以下を使用してそれらを構築します。
  - `iroha tools encode asset-id --definition <base58-asset-definition-id> --account <i105>`
  - または `--alias <name>#<domain>.<dataspace>` / `--alias <name>#<dataspace>` + `--account`。

## バージョン管理

- `SignedTransaction`、`SignedBlock`、および `SignedQuery` は、正規の Norito でエンコードされた構造体です。それぞれは `iroha_version::Version` を実装し、`EncodeVersioned` 経由でエンコードされるときにペイロードに現在の ABI バージョン (現在は `1`) をプレフィックスします。

## レビューノート/潜在的なアップデート

- クエリ DSL: 安定したユーザー向けサブセットと一般的なフィルター/セレクターの例を文書化することを検討してください。
- 命令ファミリー: `mint_burn`、`register`、`transfer` によって公開される組み込み ISI バリアントをリストするパブリック ドキュメントを展開します。

---
より詳細な部分が必要な場合 (例: 完全な ISI カタログ、完全なクエリ レジストリ リスト、またはブロック ヘッダー フィールド)、お知らせください。それに応じてそれらのセクションを拡張します。