<!-- Japanese translation of docs/source/data_model_and_isi_spec.md -->

---
lang: ja
direction: ltr
source: docs/source/data_model_and_isi_spec.md
status: complete
translator: manual
---

# Iroha v2 データモデルと ISI — 実装由来の仕様

この仕様は、設計レビューを補助するために `iroha_data_model` と `iroha_core` の現行実装からリバースエンジニアリングした内容です。バッククォートで囲まれたパスは正本コードを指します。

## 範囲
- 正準なエンティティ（ドメイン、アカウント、アセット、NFT、ロール、パーミッション、ピア、トリガ）とそれらの識別子を定義します。
- 状態を変更する命令（ISI）の型、引数、事前条件、状態遷移、発行されるイベント、エラー条件を記述します。
- パラメータ管理、トランザクション、命令シリアライゼーションを概説します。

決定性: すべての命令セマンティクスはハードウェアに依存しない純粋な状態遷移です。シリアライゼーションには Norito を用い、VM バイトコードは IVM を使用します。オンチェーン実行前にホスト側で検証されます。

---

## エンティティと識別子
ID は安定した文字列表現を持ち、`Display`／`FromStr` の往復が可能です。名前規則は空白および予約文字 `@ # $` を禁止します。

- `Name` — 検証済みテキスト識別子。ルール: `crates/iroha_data_model/src/name.rs`。
- `DomainId` — `name`。ドメイン: `{ id, logo, metadata, owned_by }`。ビルダ: `NewDomain`。コード: `crates/iroha_data_model/src/domain.rs`。
- `AccountId` — 正規アドレスは `AccountAddress` が出力する IH58／Sora 圧縮 (`snx1…`)／hex を利用する（現在は単一署名者）。IH58 を推奨し、`snx1…` は Sora 専用の次善策とする。`alias@domain` 形式はルーティング用エイリアスとして扱う。Torii は `AccountAddress::parse_any` でどの表現でも正規バイト列に正規化する。アカウント: `{ id, metadata }`。コード: `crates/iroha_data_model/src/account.rs`。
- `AssetDefinitionId` — `asset#domain`。定義: `{ id, spec: NumericSpec, mintable: Mintable, logo, metadata, owned_by, total_quantity }`。コード: `crates/iroha_data_model/src/asset/definition.rs`。
- `AssetId` — `asset#domain#account@domain` またはドメインが同じ場合は `asset##account@domain`。アセット: `{ id, value: Numeric }`。コード: `crates/iroha_data_model/src/asset/{id.rs,value.rs}`。
- `NftId` — `nft$domain`。NFT: `{ id, content: Metadata, owned_by }`。コード: `crates/iroha_data_model/src/nft.rs`。
- `RoleId` — `name`。ロール: `{ id, permissions: BTreeSet<Permission> }`。ビルダ: `NewRole { inner: Role, grant_to }`。コード: `crates/iroha_data_model/src/role.rs`。
- `Permission` — `{ name: Ident, payload: Json }`。コード: `crates/iroha_data_model/src/permission.rs`。
- `PeerId` / `Peer` — ピアの公開鍵とアドレス。コード: `crates/iroha_data_model/src/peer.rs`。
- `TriggerId` — `name`。トリガ: `{ id, action }`。アクション: `{ executable, repeats, authority, filter, metadata }`。コード: `crates/iroha_data_model/src/trigger/`。
- `Metadata` — `BTreeMap<Name, Json>`（挿入／削除時に検証あり）。コード: `crates/iroha_data_model/src/metadata.rs`。

重要トレイト: `Identifiable`、`Registered`／`Registrable`（ビルダパターン）、`HasMetadata`、`IntoKeyValue`。コード: `crates/iroha_data_model/src/lib.rs`。

イベント: すべてのエンティティは変更時にイベント（作成／削除／オーナー変更／メタデータ変更など）を発行します。コード: `crates/iroha_data_model/src/events/`。

---

## パラメータ（チェーン設定）
- ファミリー: `SumeragiParameters { block_time_ms, commit_time_ms, max_clock_drift_ms, collectors_k, collectors_redundant_send_r }`、`BlockParameters { max_transactions }`、`TransactionParameters { max_signatures, max_instructions, ivm_bytecode_size, max_tx_bytes, max_decompressed_bytes }`、`SmartContractParameters { fuel, memory, execution_depth }`、および `custom: BTreeMap`。
- 差分用の単一 enum: `SumeragiParameter`、`BlockParameter`、`TransactionParameter`、`SmartContractParameter`。集約: `Parameters`。コード: `crates/iroha_data_model/src/parameter/system.rs`。

パラメータ設定（ISI）: `SetParameter(Parameter)` が該当フィールドを更新し、`ConfigurationEvent::Changed` を発行します。コード: `crates/iroha_data_model/src/isi/transparent.rs`、実行コード: `crates/iroha_core/src/smartcontracts/isi/world.rs`。

---

## 命令のシリアライゼーションとレジストリ
- 中核トレイト: `Instruction: Send + Sync + 'static`。`dyn_encode()` と `as_any()`、安定した `id()`（デフォルトは具象型名）を提供。
- `InstructionBox`: `Box<dyn Instruction>` のラッパー。`Clone`／`Eq`／`Ord` は `(type_id, encoded_bytes)` を比較するため値ベースで同値判定します。
- 安定したワイヤ ID: レジストリは型ごとの「ワイヤ ID」をサポートし、Rust の `type_name` から独立した識別子でシリアライズできます。デフォルトレジストリは主要命令に明示 ID を設定（例: `Log` → `iroha.log`、`SetParameter` → `iroha.set_parameter`、`ExecuteTrigger` → `iroha.execute_trigger`）。デコード時は後方互換性のためワイヤ ID と `type_name` の両方を受け付けます。
- Norito による `InstructionBox` シリアライズは `(String wire_id, Vec<u8> payload)` 形式（ワイヤ ID が無い場合は `type_name`）。デシリアライズではグローバルな `InstructionRegistry` が識別子をコンストラクタへマッピングします。デフォルトレジストリには全ビルトイン ISI が登録済み。コード: `crates/iroha_data_model/src/isi/{mod.rs,registry.rs}`。

---

## ISI: 型・セマンティクス・エラー
実行は `iroha_core::smartcontracts::isi` にある `Execute for <Instruction>` を通じて実装されています。以下では、公開されている効果、事前条件、発行イベント、エラーを列挙します。

### 登録／登録解除
型: `Register<T>` と `Unregister<T>`（`T ∈ { Domain, Account, AssetDefinition, Asset, Nft, Role, Permission, Peer, Trigger }`）。ボックス化された列挙型（`RegisterBox` など）が提供されています。

- ドメイン登録: `DomainId` が存在しないことを確認したうえで新しいドメインを格納します。
  - 事前条件: 同じ `DomainId` が未登録であること。
  - イベント: `DomainEvent::Created`。`owned_by` にはリクエスト発行者が記録されます。
  - エラー: すでに存在する場合は `Repetition(Register, DomainId)`。コード: `core/.../isi/world.rs`。

- アカウント登録: `AccountId` をキーにアカウントを保存します。
  - 事前条件: ドメインが存在すること、アカウントが未登録であること。
  - イベント: `AccountEvent::Created`。
  - エラー: `FindError::Domain`（ドメインなし）、`Repetition(Register, AccountId)`（重複）。コード: `core/.../isi/domain.rs`。

- アセット定義登録: `AssetDefinition` を保管し、`mintable=Once` の場合は初回ミント時に `Not` へ遷移するための準備を行います。
  - 事前条件: 定義が未登録であること。
  - イベント: `AssetDefinitionEvent::Created`。
  - エラー: `Repetition(Register, AssetDefinitionId)`。コード: `core/.../isi/domain.rs`。

- NFT 定義登録: `Nft` を格納します。
  - 事前条件: ドメインが存在すること。重複 ID を禁止。
  - イベント: `NftEvent::Created`。
  - エラー: `Repetition(Register, NftId)`。コード: `core/.../isi/nft.rs`。

- ロール登録: `NewRole { inner, grant_to }` からロールを構築し、`inner: Role` を保存します（`grant_to` はアカウントへの事前付与に使用されます）。
  - 事前条件: ロール ID が未登録であること。
  - イベント: `RoleEvent::Created`。
  - エラー: `Repetition(Register, RoleId)`。コード: `core/.../isi/world.rs`。

- トリガ登録: フィルタ種別ごとのトリガ集合にトリガを登録します。
  - 事前条件: フィルタが非ミント可能な場合、`action.repeats` は `Exactly(1)` でなければならず、違反すると `MathError::Overflow`。ID の重複は禁止。
  - イベント: `TriggerEvent::Created(TriggerId)`。
  - エラー: `Repetition(Register, TriggerId)`、変換や検証失敗時は `InvalidParameterError::SmartContract(..)`。コード: `core/.../isi/triggers/mod.rs`。

- ピア／ドメイン／アカウント／アセット定義／NFT／ロール／トリガの登録解除: 対象を削除し、関連イベントを発行します。追加の連鎖削除:
  - ドメイン削除: ドメイン配下のアカウント、ロール、パーミッション、tx シーケンス、アカウントラベル、UAID バインディングを削除し、各アカウントの保有アセット（アセットメタデータを含む）を削除します。ドメイン内のアセット定義を削除し、ドメイン内の NFT と削除対象アカウントが所有する NFT も削除します。権限ドメインが一致するトリガを削除します。イベント: `DomainEvent::Deleted` および各削除イベント。エラー: `FindError::Domain`。コード: `core/.../isi/world.rs`。
  - アカウント削除: アカウントのパーミッション／ロール、tx シーケンス、アカウントラベル、UAID バインディングを削除し、保有アセット（アセットメタデータを含む）と所有 NFT を削除します。アカウントを権限とするトリガを削除します。イベント: `AccountEvent::Deleted` と、削除された NFT ごとの `NftEvent::Deleted`。エラー: `FindError::Account`。コード: `core/.../isi/domain.rs`。
  - アセット定義削除: 当該定義のアセットを全削除し、アセットメタデータも削除します。イベント: `AssetDefinitionEvent::Deleted`、各アセットについて `AssetEvent::Deleted`。エラー: `FindError::AssetDefinition`。コード: `core/.../isi/domain.rs`。
  - NFT 削除: 対象 NFT を削除します。イベント: `NftEvent::Deleted`。エラー: `FindError::Nft`。コード: `core/.../isi/nft.rs`。
  - ロール削除: すべてのアカウントからロールを剥奪した後、ロールを削除します。イベント: `RoleEvent::Deleted`。エラー: `FindError::Role`。コード: `core/.../isi/world.rs`。
  - トリガ削除: 存在する場合のみ削除し、重複削除は `Repetition(Unregister, TriggerId)`。イベント: `TriggerEvent::Deleted`。コード: `core/.../isi/triggers/mod.rs`。

### ミント／バーン
型: `Mint<O, D: Identifiable>` および `Burn<O, D: Identifiable>`（`MintBox`／`BurnBox` として提供）。

- アセット（数値型）のミント／バーン: 残高と定義の `total_quantity` を調整します。
  - 事前条件: `Numeric` 値が `AssetDefinition.spec()` を満たすこと。ミント可能性は `mintable` によって制御:
    - `Infinitely`: 常に許可。
    - `Once`: 最初の 1 回のみ許可。初回ミント時に `mintable` が `Not` へ変わり、`AssetDefinitionEvent::MintabilityChanged` と監査向け `AssetDefinitionEvent::MintabilityChangedDetailed { asset_definition, minted_amount, authority }` を発行。
    - `Limited(n)`: 残り `n` 回までミントを許可。ミントのたびにカウンタを 1 減算し、0 になると `Not` へ切り替わり同じ `MintabilityChanged` 系イベントを発行。
    - `Not`: `MintabilityError::MintUnmintable`。
  - 状態変化: ミントで未作成のアセットを生成。バーン後に残高がゼロなら削除。
  - イベント: `AssetEvent::Added`／`AssetEvent::Removed`、`Once` または `Limited(n)` で残数を使い切った場合に `AssetDefinitionEvent::MintabilityChanged`。
  - エラー: `TypeError::AssetNumericSpec(Mismatch)`、`MathError::Overflow`／`NotEnoughQuantity`。コード: `core/.../isi/asset.rs`。

- トリガの繰り返し回数ミント／バーン: トリガの `action.repeats` を増減します。
  - 事前条件: ミント時にフィルタがミント可能であること。演算がオーバーフロー／アンダーフローしないこと。
  - イベント: `TriggerEvent::Extended`／`TriggerEvent::Shortened`。
  - エラー: 不正なミントで `MathError::Overflow`、未登録なら `FindError::Trigger`。コード: `core/.../isi/triggers/mod.rs`。

### 移転
型: `Transfer<S: Identifiable, O, D: Identifiable>`（`TransferBox` として提供）。

- アセット（数値型）: ソース `AssetId` から差し引き、同一アセット定義の宛先アカウントへ加算。ソース残高がゼロになれば削除。
  - 事前条件: ソースアセットが存在し、値が `spec` を満たすこと。
  - イベント: ソース側 `AssetEvent::Removed`、宛先側 `AssetEvent::Added`。
  - エラー: `FindError::Asset`、`TypeError::AssetNumericSpec`、`MathError::NotEnoughQuantity/Overflow`。コード: `core/.../isi/asset.rs`。

- ドメイン所有権: `Domain.owned_by` を宛先アカウントに更新。
  - 事前条件: 双方のアカウントとドメインが存在。
  - イベント: `DomainEvent::OwnerChanged`。
  - エラー: `FindError::Account`／`FindError::Domain`。コード: `core/.../isi/domain.rs`。

- アセット定義所有権: `AssetDefinition.owned_by` を宛先アカウントに更新。
  - 事前条件: 双方のアカウントと定義が存在し、ソースが所有者であること。
  - イベント: `AssetDefinitionEvent::OwnerChanged`。
  - エラー: `FindError::Account`／`FindError::AssetDefinition`。コード: `core/.../isi/account.rs`。

- NFT 所有権: `Nft.owned_by` を宛先アカウントに更新。
  - 事前条件: 双方のアカウントと NFT が存在し、ソースが所有者であること。
  - イベント: `NftEvent::OwnerChanged`。
  - エラー: `FindError::Account`／`FindError::Nft`、所有者不一致で `InvariantViolation`。コード: `core/.../isi/nft.rs`。

### メタデータ: キー／値の設定・削除
型: `SetKeyValue<T>`／`RemoveKeyValue<T>`（`T ∈ { Domain, Account, AssetDefinition, Nft, Trigger }`）。ボックス化された列挙を利用します。

- 設定: `Metadata[key] = Json(value)` を挿入または置換。
- 削除: キーを削除。存在しない場合はエラー。
- イベント: `<Target>Event::MetadataInserted`／`MetadataRemoved`（旧値／新値を含む）。
- エラー: 対象が存在しない場合は `FindError::<Target>`、削除対象のキーが無い場合は `FindError::MetadataKey`。コード: `crates/iroha_data_model/src/isi/transparent.rs` および対象ごとの実行コード。

### パーミッションとロール: 付与／剥奪
型: `Grant<O, D>`／`Revoke<O, D>`。`Permission`／`Role` と `Account`、`Permission` と `Role` の組み合わせに対応する列挙が提供されています。

- パーミッション付与（アカウント）: 既に暗黙的でない限り `Permission` を追加。
  - イベント: `AccountEvent::PermissionAdded`。
  - エラー: 重複で `Repetition(Grant, Permission)`。コード: `core/.../isi/account.rs`。

- パーミッション剥奪（アカウント）: 存在すれば削除。
  - イベント: `AccountEvent::PermissionRemoved`。
  - エラー: 不在で `FindError::Permission`。コード: `core/.../isi/account.rs`。

- ロール付与（アカウント）: `(account, role)` のマッピングを追加。
  - イベント: `AccountEvent::RoleGranted`。
  - エラー: `Repetition(Grant, RoleId)`。コード: `core/.../isi/account.rs`。

- ロール剥奪（アカウント）: マッピングを削除。
  - イベント: `AccountEvent::RoleRevoked`。
  - エラー: 不在で `FindError::Role`。コード: `core/.../isi/account.rs`。

- パーミッション付与（ロール）: パーミッションを追加したロールを再構築。
  - イベント: `RoleEvent::PermissionAdded`。
  - エラー: `Repetition(Grant, Permission)`。コード: `core/.../isi/world.rs`。

- パーミッション剥奪（ロール）: 該当パーミッションを除いたロールを再構築。
  - イベント: `RoleEvent::PermissionRemoved`。
  - エラー: `FindError::Permission`。コード: `core/.../isi/world.rs`。

### トリガ: 実行
型: `ExecuteTrigger { trigger: TriggerId, args: Json }`。
- 動作: `ExecuteTriggerEvent { trigger_id, authority, args }` をキューへ投入し、トリガサブシステムが消費します。手動実行は `ExecuteTrigger` フィルタ（by-call）のトリガのみ許可され、フィルタ一致と呼び出し者がトリガのアクション権限者または `CanExecuteTrigger` 権限を持つことが必要です。ユーザ提供エグゼキュータが有効な場合、トリガ実行はランタイムエグゼキュータ検証を通り、トランザクションの executor fuel 予算（`executor.fuel` 基本値 + メタデータ `additional_fuel`）を消費します。
- エラー: 未登録で `FindError::Trigger`、権限不一致で `InvariantViolation`。コード: `core/.../isi/triggers/mod.rs`（テストは `core/.../smartcontracts/isi/mod.rs`）。

### アップグレードとログ
- `Upgrade { executor }`: 提供された `Executor` バイトコードを使ってエグゼキュータを移行し、エグゼキュータとデータモデルを更新、`ExecutorEvent::Upgraded` を発行。失敗時は `InvalidParameterError::SmartContract` でラップ。コード: `core/.../isi/world.rs`。
- `Log { level, msg }`: 指定レベルのノードログを出力（状態変化なし）。コード: `core/.../isi/world.rs`。

### エラーモデル
共通のラッパーは `InstructionExecutionError` で、評価エラー、クエリエラー、変換エラー、エンティティ未検出、重複、ミント可否、数値演算エラー、無効パラメータ、インバリアント違反などのバリアントがあります。列挙とヘルパーは `crates/iroha_data_model/src/isi/mod.rs` の `pub mod error` に定義されています。

---

## トランザクションと実行体
- `Executable`: `Instructions(ConstVec<InstructionBox>)` もしくは `Ivm(IvmBytecode)` のいずれか。バイトコードは base64 でシリアライズ。コード: `crates/iroha_data_model/src/transaction/executable.rs`。
- `TransactionBuilder`／`SignedTransaction`: メタデータ、`chain_id`、`authority`、`creation_time_ms`、任意の `ttl_ms` と `nonce` を付与して実行体を組み立て署名。コード: `crates/iroha_data_model/src/transaction/`。
- 実行時、`iroha_core` は `InstructionBox` のバッチを `Execute for InstructionBox` で処理し、適切な `*Box` または具象命令へダウンキャストします。コード: `crates/iroha_core/src/smartcontracts/isi/mod.rs`。
- ランタイムエグゼキュータ検証の予算（ユーザ提供エグゼキュータ時）: パラメータの `executor.fuel` にトランザクションメタデータ `additional_fuel`（`u64`）を加算し、同一トランザクション内の命令/トリガ検証で共有します。

---

## インバリアントと注意点（テスト・ガードより）
- ジェネシス保護: `genesis` ドメインおよび `genesis` ドメイン内アカウントは登録できず、`genesis` アカウント自体も登録不可。コード／テスト: `core/.../isi/world.rs`, `core/.../smartcontracts/isi/mod.rs`。
- 数値アセットはミント／トランスファ／バーン時に `NumericSpec` を満たす必要があり、違反すると `TypeError::AssetNumericSpec`。
- ミント可否: `Once` は一度だけミント可能で、その後 `Not` へ切り替わります。`Limited(n)` は残り `n` 回のミントを許可し、使い切ると `Not` に遷移します。`Infinitely` を禁止しようとすると `MintabilityError::ForbidMintOnMintable`、`Limited(0)` は `MintabilityError::InvalidMintabilityTokens` を返します。ガードはアカウント ISI ヘルパーに存在します。
- メタデータ操作はキー一致で動作し、存在しないキーの削除はエラー。
- トリガフィルタが非ミント可能な場合、`Register<Trigger>` は `Exactly(1)` の繰り返しのみ許容します。
- 決定性: すべての算術はチェック付き演算を使い、オーバーフロー／アンダーフローは型付き数値エラーを返します。残高ゼロのアセットは削除され、隠し状態は残りません。

---

## 実用例
- ミントとトランスファ:
  - `Mint::asset_numeric(10, asset_id)` → スペックとミント可否が許容すれば 10 を加算。イベント: `AssetEvent::Added`。
  - `Transfer::asset_numeric(asset_id, 5, to_account)` → 5 を移転。差し引きと加算のイベントを発行。
- メタデータ更新:
  - `SetKeyValue::account(account_id, "avatar".parse()?, json)` → upsert。削除は `RemoveKeyValue::account(...)`。
- ロール／パーミッション管理:
  - `Grant::account_role(role_id, account)`、`Grant::role_permission(perm, role)` とその `Revoke` 版。
- トリガライフサイクル:
  - `Register::trigger(Trigger::new(id, Action::new(exec, repeats, authority, filter)))`（フィルタによりミント可否を暗示）。`ExecuteTrigger::new(id).with_args(&args)` は設定された権限と一致する必要があります。
- パラメータ更新:
  - `SetParameter(SumeragiParameter::BlockTimeMs(2500).into())` → パラメータを更新し `ConfigurationEvent::Changed` を発行。

---

## トレーサビリティ（主要ソース）
- データモデル中核: `crates/iroha_data_model/src/{account.rs,domain.rs,asset/**,nft.rs,role.rs,permission.rs,metadata.rs,trigger/**,parameter/**}`。
- ISI 定義とレジストリ: `crates/iroha_data_model/src/isi/{mod.rs,register.rs,transfer.rs,mint_burn.rs,transparent.rs,registry.rs}`。
- ISI 実行: `crates/iroha_core/src/smartcontracts/isi/{mod.rs,world.rs,domain.rs,account.rs,asset.rs,nft.rs,triggers/**}`。
- イベント: `crates/iroha_data_model/src/events/**`。
- トランザクション: `crates/iroha_data_model/src/transaction/**`。

この仕様を API／挙動テーブルとしてレンダリングしたり、各イベント／エラーへのクロスリンクを付与したりする必要があれば、お知らせください。
