<!-- Japanese translation of docs/source/data_model_and_isi_spec.md -->

---
lang: ja
direction: ltr
source: docs/source/data_model_and_isi_spec.md
status: complete
translator: machine-google-reviewed
source_hash: 2077d985b10b26b29b821646b435cc8850cbc6c842d372de6c9c4523ee95a5b7
source_last_modified: "2026-03-12T11:24:34.970622+00:00"
translation_last_reviewed: 2026-03-12
---

# Iroha v2 データ モデルと ISI — 実装から派生した仕様

この仕様は、設計レビューを支援するために、`iroha_data_model` および `iroha_core` の現在の実装からリバース エンジニアリングされたものです。バッククォート内のパスは、権限のあるコードを指します。

## 範囲
- 正規エンティティ (ドメイン、アカウント、アセット、NFT、ロール、権限、ピア、トリガー) とその識別子を定義します。
- 状態変更命令 (ISI) について説明します: タイプ、パラメーター、前提条件、状態遷移、発行されたイベント、およびエラー条件。
- パラメータ管理、トランザクション、および命令のシリアル化を要約します。

決定論: すべての命令セマンティクスは、ハードウェアに依存する動作のない純粋な状態遷移です。シリアル化では Norito を使用します。 VM バイトコードは IVM を使用し、オンチェーン実行前にホスト側で検証されます。

---

## エンティティと識別子
ID は、`Display`/`FromStr` 往復の安定した文字列形式を持ちます。名前規則では、空白文字と予約された `@ # $` 文字が禁止されています。- `Name` — 検証されたテキスト識別子。ルール: `crates/iroha_data_model/src/name.rs`。
- `DomainId` — `name`。ドメイン: `{ id, logo, metadata, owned_by }`。ビルダー: `NewDomain`。コード: `crates/iroha_data_model/src/domain.rs`。
- `AccountId` — 正規アドレスは `AccountAddress` (I105 / 16 進数) を介して生成され、Torii は `AccountAddress::parse_encoded` を介して入力を正規化します。 I105 が推奨されるアカウント形式です。 I105 フォームは Sora のみの UX 用です。おなじみの `alias` (拒否されたレガシー フォーム) 文字列は、ルーティング エイリアスとしてのみ保持されます。アカウント: `{ id, metadata }`。コード: `crates/iroha_data_model/src/account.rs`。- アカウント アドミッション ポリシー - ドメインは、メタデータ キー `iroha:account_admission_policy` の下に Norito-JSON `AccountAdmissionPolicy` を保存することで、暗黙的なアカウント作成を制御します。キーが存在しない場合、チェーンレベルのカスタム パラメータ `iroha:default_account_admission_policy` がデフォルトを提供します。これも存在しない場合、ハードデフォルトは `ImplicitReceive` (最初のリリース) です。ポリシー タグ `mode` (`ExplicitOnly` または `ImplicitReceive`) に加え、オプションのトランザクションごと (デフォルト `16`) およびブロックごとの作成上限、オプションの `implicit_creation_fee` (バーン アカウントまたはシンク アカウント)、資産定義ごとの `min_initial_amounts`、オプションの `default_role_on_create` (`AccountCreated` の後に付与され、欠落している場合は `DefaultRoleError` で拒否されます)。 Genesis はオプトインできません。無効/無効なポリシーは、`InstructionExecutionError::AccountAdmission` の不明なアカウントに対するレシート形式の指示を拒否します。暗黙的なアカウント スタンプ メタデータ `iroha:created_via="implicit"` の前に `AccountCreated`。デフォルトのロールはフォローアップ `AccountRoleGranted` を発行し、実行者の所有者ベースライン ルールにより、新しいアカウントは追加のロールなしで独自の資産/NFT を使用できます。コード: `crates/iroha_data_model/src/account/admission.rs`、`crates/iroha_core/src/smartcontracts/isi/account_admission.rs`。
- `AssetDefinitionId` — 正規の `aid:<32-lower-hex-no-dash>` (UUID-v4 バイト)。定義: `{ id, name, description?, alias?, spec: NumericSpec, mintable: Mintable, logo, metadata, owned_by, total_quantity }`。 `alias` リテラルは、`<name>#<domain>@<dataspace>` または `<name>#<dataspace>` である必要があります。`<name>` はアセット定義名と同じです。コード: `crates/iroha_data_model/src/asset/definition.rs`。
- `AssetId`: 正規エンコードされたリテラル `norito:<hex>` (従来のテキスト形式は最初のリリースではサポートされていません)。- `NftId` — `nft$domain`。 NFT: `{ id, content: Metadata, owned_by }`。コード: `crates/iroha_data_model/src/nft.rs`。
- `RoleId` — `name`。役割: `{ id, permissions: BTreeSet<Permission> }` ビルダー `NewRole { inner: Role, grant_to }`。コード: `crates/iroha_data_model/src/role.rs`。
- `Permission` — `{ name: Ident, payload: Json }`。コード: `crates/iroha_data_model/src/permission.rs`。
- `PeerId`/`Peer` — ピア ID (公開キー) とアドレス。コード: `crates/iroha_data_model/src/peer.rs`。
- `TriggerId` — `name`。トリガー: `{ id, action }`。アクション: `{ executable, repeats, authority, filter, metadata }`。コード: `crates/iroha_data_model/src/trigger/`。
- `Metadata` — 挿入/削除がチェックされた `BTreeMap<Name, Json>`。コード: `crates/iroha_data_model/src/metadata.rs`。
- サブスクリプション パターン (アプリケーション層): プランは、`subscription_plan` メタデータを含む `AssetDefinition` エントリです。サブスクリプションは、`subscription` メタデータを持つ `Nft` レコードです。課金はサブスクリプションNFTを参照するタイムトリガーによって実行されます。 `docs/source/subscriptions_api.md` および `crates/iroha_data_model/src/subscription.rs` を参照してください。
- **暗号プリミティブ** (機能 `sm`):
  - `Sm2PublicKey` / `Sm2Signature` は、SM2 の正規 SEC1 ポイント + 固定幅 `r∥s` エンコーディングをミラーリングします。コンストラクターは曲線メンバーシップと識別 ID セマンティクス (`DEFAULT_DISTID`) を強制しますが、検証では不正な形式または高範囲のスカラーが拒否されます。コード: `crates/iroha_crypto/src/sm.rs` および `crates/iroha_data_model/src/crypto/mod.rs`。
  - `Sm3Hash` は、マニフェストまたはテレメトリにハッシュが表示される場所で使用される Norito シリアル化可能な `[u8; 32]` ニュータイプとして GM/T 0004 ダイジェストを公開します。コード: `crates/iroha_data_model/src/crypto/hash.rs`。- `Sm4Key` は 128 ビット SM4 キーを表し、ホスト システムコールとデータモデル フィクスチャ間で共有されます。コード: `crates/iroha_data_model/src/crypto/symmetric.rs`。
  これらのタイプは既存の Ed25519/BLS/ML-DSA プリミティブと並んでおり、`sm` 機能が有効になると、データ モデル コンシューマー (Torii、SDK、ジェネシス ツール) で使用できるようになります。
- データスペース派生リレーション ストア (`space_directory_manifests`、`uaid_dataspaces`、`axt_policies`、`axt_replay_ledger`、レーン リレー緊急オーバーライド レジストリ) およびデータスペース ターゲット権限 (アカウント/ロール権限ストアの `CanPublishSpaceDirectoryManifest{dataspace: ...}`) はプルーニングされます。 `State::set_nexus(...)` は、アクティブな `dataspace_catalog` からデータスペースが消失し、ランタイム カタログの更新後に古いデータスペース参照が発生するのを防ぎます。レーン スコープの DA/リレー キャッシュ (`lane_relays`、`da_commitments`、`da_confidential_compute`、`da_pin_intents`) も、レーンが廃止されるか、別のデータスペースに再割り当てされるときにプルーニングされるため、データスペースの移行間でレーン ローカルの状態が漏洩することはありません。スペース ディレクトリ ISI (`PublishSpaceDirectoryManifest`、`RevokeSpaceDirectoryManifest`、`ExpireSpaceDirectoryManifest`) も、アクティブなカタログに対して `dataspace` を検証し、不明な ID を `InvalidParameter` で拒否します。

重要な特性: `Identifiable`、`Registered`/`Registrable` (ビルダー パターン)、`HasMetadata`、`IntoKeyValue`。コード: `crates/iroha_data_model/src/lib.rs`。

イベント: すべてのエンティティには、ミューテーション (作成/削除/所有者の変更/メタデータの変更など) で発行されるイベントがあります。コード: `crates/iroha_data_model/src/events/`。

---## パラメータ (チェーン構成)
- ファミリ: `SumeragiParameters { block_time_ms, commit_time_ms, min_finality_ms, pacing_factor_bps, max_clock_drift_ms, collectors_k, collectors_redundant_send_r }`、`BlockParameters { max_transactions }`、`TransactionParameters { max_signatures, max_instructions, ivm_bytecode_size, max_tx_bytes, max_decompressed_bytes }`、`SmartContractParameters { fuel, memory, execution_depth }`、および `custom: BTreeMap`。
- 差分の単一列挙型: `SumeragiParameter`、`BlockParameter`、`TransactionParameter`、`SmartContractParameter`。アグリゲータ: `Parameters`。コード: `crates/iroha_data_model/src/parameter/system.rs`。

パラメータの設定 (ISI): `SetParameter(Parameter)` は対応するフィールドを更新し、`ConfigurationEvent::Changed` を出力します。コード: `crates/iroha_data_model/src/isi/transparent.rs`、`crates/iroha_core/src/smartcontracts/isi/world.rs` の実行者。

---

## 命令のシリアル化とレジストリ
- コア特性: `Instruction: Send + Sync + 'static` と `dyn_encode()`、`as_any()`、安定した `id()` (デフォルトは具体的な型名)。
- `InstructionBox`: `Box<dyn Instruction>` ラッパー。 Clone/Eq/Ord は `(type_id, encoded_bytes)` で動作するため、等価性は値によって決まります。
- `InstructionBox` の Norito serde は、`(String wire_id, Vec<u8> payload)` としてシリアル化されます (ワイヤ ID がない場合は `type_name` にフォールバックします)。逆シリアル化では、グローバル `InstructionRegistry` マッピング識別子をコンストラクターに使用します。デフォルトのレジストリには、すべての組み込み ISI が含まれています。コード: `crates/iroha_data_model/src/isi/{mod.rs,registry.rs}`。

---

## ISI: タイプ、セマンティクス、エラー
実行は、`iroha_core::smartcontracts::isi` の `Execute for <Instruction>` を介して実装されます。以下に、パブリック効果、前提条件、発行されたイベント、およびエラーをリストします。

### 登録/登録解除
タイプ: `Register<T: Registered>` および `Unregister<T: Identifiable>`、具体的なターゲットをカバーする合計タイプ `RegisterBox`/`UnregisterBox`。- ピアの登録: ワールド ピア セットに挿入します。
  - 前提条件: すでに存在していてはなりません。
  - イベント: `PeerEvent::Added`。
  - エラー: 重複の場合は `Repetition(Register, PeerId)`。ルックアップでは `FindError`。コード: `core/.../isi/world.rs`。

- ドメインの登録: `NewDomain` から `owned_by = authority` をビルドします。禁止: `genesis` ドメイン。
  - 前提条件: ドメインが存在しないこと。 `genesis`ではありません。
  - イベント: `DomainEvent::Created`。
  - エラー: `Repetition(Register, DomainId)`、`InvariantViolation("Not allowed to register genesis domain")`。コード: `core/.../isi/world.rs`。

- アカウントの登録: `NewAccount` からビルドします。`genesis` ドメインでは許可されていません。 `genesis` アカウントは登録できません。
  - 前提条件: ドメインが存在する必要があります。アカウントが存在しない。ジェネシスドメインにはありません。
  - イベント: `DomainEvent::Account(AccountEvent::Created)`。
  - エラー: `Repetition(Register, AccountId)`、`InvariantViolation("Not allowed to register account in genesis domain")`。コード: `core/.../isi/domain.rs`。

- AssetDefinition の登録: ビルダーからビルドします。 `owned_by = authority`を設定します。
  - 前提条件: 定義が存在しないこと。ドメインが存在します。 `name` は必須であり、トリム後に空であってはならず、`#`/`@` を含めることはできません。
  - イベント: `DomainEvent::AssetDefinition(AssetDefinitionEvent::Created)`。
  - エラー: `Repetition(Register, AssetDefinitionId)`。コード: `core/.../isi/domain.rs`。

- NFT の登録: ビルダーからビルドします。 `owned_by = authority` を設定します。
  - 前提条件: NFT が存在しない。ドメインが存在します。
  - イベント: `DomainEvent::Nft(NftEvent::Created)`。
  - エラー: `Repetition(Register, NftId)`。コード: `core/.../isi/nft.rs`。- ロールの登録: `NewRole { inner, grant_to }` (アカウントとロールのマッピングを介して記録された最初の所有者) から構築され、`inner: Role` が保存されます。
  - 前提条件: ロールが存在しない。
  - イベント: `RoleEvent::Created`。
  - エラー: `Repetition(Register, RoleId)`。コード: `core/.../isi/world.rs`。

- トリガーの登録: フィルターの種類によって設定された適切なトリガーにトリガーを保存します。
  - 前提条件: フィルターが mintable でない場合、`action.repeats` は `Exactly(1)` でなければなりません (それ以外の場合は `MathError::Overflow`)。 IDの重複は禁止です。
  - イベント: `TriggerEvent::Created(TriggerId)`。
  - エラー: 変換/検証失敗時の `Repetition(Register, TriggerId)`、`InvalidParameterError::SmartContract(..)`。コード: `core/.../isi/triggers/mod.rs`。- ピア/ドメイン/アカウント/資産定義/NFT/ロール/トリガーの登録解除: ターゲットを削除します。削除イベントを発行します。追加のカスケード削除:- ドメインの登録解除: ドメイン エンティティとそのセレクター/承認ポリシーの状態を削除します。ドメイン内の資産定義 (およびそれらの定義によってキー化された機密 `zk_assets` サイドステート)、それらの定義の資産 (および資産ごとのメタデータ)、ドメイン内の NFT、およびドメイン スコープのアカウント ラベル/エイリアス プロジェクションを削除します。また、削除されたドメインから生き残ったアカウントのリンクが解除され、削除されたドメインまたは一緒に削除されたリソースを参照するアカウント/ロールスコープの権限エントリ (ドメイン権限、削除された定義の資産定義/資産権限、および削除された NFT ID の NFT 権限) がプルーニングされます。ドメインを削除しても、グローバル `AccountId`、その tx シーケンス/UAID 状態、外部資産または NFT の所有権、トリガー権限、または存続するアカウントを指すその他の外部監査/構成参照は削除されません。ガード レール: ドメイン内の資産定義がリポ契約、決済元帳、パブリック レーン報酬/請求、オフライン手当/移転、決済リポジトリのデフォルト (`settlement.repo.eligible_collateral`、`settlement.repo.collateral_substitution_matrix`)、ガバナンスで構成された投票/市民権/議会資格/バイラル報酬の資産定義参照によって参照されている場合に拒否されます。 oracle-economics で構成された報酬/スラッシュ/紛争保証金資産定義参照、または Nexus 手数料/ステーキング資産定義参照 (`nexus.fees.fee_asset_id`、`nexus.staking.stake_asset_id`)。イベント: `DomainEvent::Deleted`、および項目ごとの削除削除されたドメイン スコープのリソースのイベント。エラー: 見つからない場合は `FindError::Domain`。保持された資産定義参照の競合に関する `InvariantViolation`。コード: `core/.../isi/world.rs`。- アカウントの登録解除: アカウントの権限、役割、tx シーケンス カウンタ、アカウント ラベル マッピング、および UAID バインディングを削除します。アカウントが所有するアセット (およびアセットごとのメタデータ) を削除します。アカウントが所有するNFTを削除します。そのアカウントの権限を持つトリガーを削除します。削除されたアカウントを参照するアカウント/ロール スコープのアクセス許可エントリ、削除された所有 NFT ID のアカウント/ロール スコープの NFT ターゲット アクセス許可、および削除されたトリガーのアカウント/ロール スコープのトリガー ターゲット アクセス許可をプルーニングします。ガードレール: アカウントがまだドメインを所有している場合は拒否、資産定義、SoraFSプロバイダー・バインディング、アクティブな市民権記録、パブリック・レーン・ステーキング/報酬状態(アカウントが請求者または報酬資産所有者として表示される場合の報酬請求キーを含む)、アクティブなオラクル状態(オラクル・フィード履歴プロバイダー・エントリ、twitterバインディング・プロバイダー・レコード、またはオラクル・エコノミクスで構成された報酬/スラッシュ・アカウント参照を含む)、アクティブNexus 手数料/ステーキング アカウント参照 (`nexus.fees.fee_sink_account_id`、`nexus.staking.stake_escrow_account_id`、`nexus.staking.slash_sink_account_id`、正規のドメインレス アカウント識別子として解析され、無効なリテラルで拒否されたフェールクローズ)、アクティブなレポ契約状態、アクティブな決済台帳状態、アクティブなオフライン アローアンス/転送またはオフライン評決取り消し状態、アクティブな資産定義のアクティブなオフライン エスクロー アカウント構成参照 (`settlement.offline.escrow_accounts`)、アクティブなガバナンス状態 (提案/段階承認)als/locks/slashes/council/parliament 名簿、提案議会スナップショット、ランタイム アップグレード提案者レコード、ガバナンス構成エスクロー/スラッシュ レシーバー/バイラル プール アカウント参照、`gov.sorafs_telemetry.submitters` / `gov.sorafs_telemetry.per_provider_submitters` 経由のガバナンス SoraFS テレメトリ提出者参照、またはガバナンス構成済み SoraFS プロバイダー所有者参照 (`gov.sorafs_provider_owners` 経由)、構成済みコンテンツ公開許可リストのアカウント参照 (`content.publish_allow_accounts`)、アクティブなソーシャル エスクロー送信者状態、アクティブなコンテンツ バンドル作成者状態、アクティブな DA ピンインテント所有者状態、アクティブなレーン リレー緊急バリデーター オーバーライド状態、またはアクティブSoraFS PIN レジストリ発行者/バインダー レコード (PIN マニフェスト、マニフェスト エイリアス、レプリケーション順序)。イベント: `AccountEvent::Deleted`、および削除された NFT ごとの `NftEvent::Deleted`。エラー: 見つからない場合は `FindError::Account`。所有権の孤立に関する `InvariantViolation`。コード: `core/.../isi/domain.rs`。- AssetDefinition の登録解除: その定義のすべてのアセットとそのアセットごとのメタデータを削除し、その定義によってキー設定された機密 `zk_assets` サイドステートを削除します。また、削除されたアセット定義またはそのアセット インスタンスを参照する、一致する `settlement.offline.escrow_accounts` エントリとアカウント/ロール スコープのアクセス許可エントリもプルーニングされます。ガードレール: 定義がリポ契約、決済元帳、パブリックレーン報酬/請求、オフライン許可/転送状態、決済リポジトリのデフォルト (`settlement.repo.eligible_collateral`、`settlement.repo.collateral_substitution_matrix`)、ガバナンスで構成された投票/市民権/議会資格/バイラル報酬資産定義参照、構成されたオラクル経済によって参照されている場合は拒否されます。リワード/スラッシュ/紛争債券資産定義リファレンス、または Nexus 手数料/ステーキング資産定義リファレンス (`nexus.fees.fee_asset_id`、`nexus.staking.stake_asset_id`)。イベント: アセットごとの `AssetDefinitionEvent::Deleted` および `AssetEvent::Deleted`。エラー: 参照の競合に関する `FindError::AssetDefinition`、`InvariantViolation`。コード: `core/.../isi/domain.rs`。
  - NFT の登録解除: NFT を削除し、削除された NFT を参照するアカウント/ロール スコープの権限エントリを整理します。イベント: `NftEvent::Deleted`。エラー: `FindError::Nft`。コード: `core/.../isi/nft.rs`。
  - ロールの登録解除: 最初にすべてのアカウントからロールを取り消します。その後、役割を削除します。イベント: `RoleEvent::Deleted`。エラー: `FindError::Role`。コード: `core/.../isi/world.rs`。- トリガーの登録解除: トリガーが存在する場合は削除し、削除されたトリガーを参照するアカウント/ロール スコープの権限エントリを削除します。重複して登録解除すると `Repetition(Unregister, TriggerId)` が発生します。イベント: `TriggerEvent::Deleted`。コード: `core/.../isi/triggers/mod.rs`。

### ミント / バーン
タイプ: `Mint<O, D: Identifiable>` および `Burn<O, D: Identifiable>`、`MintBox`/`BurnBox` としてボックス化されます。

- アセット (数値) ミント/バーン: バランスと定義の `total_quantity` を調整します。
  - 前提条件: `Numeric` 値は `AssetDefinition.spec()` を満たす必要があります。 `mintable` で許可されているミント:
    - `Infinitely`: 常に許可されます。
    - `Once`: 1 回だけ許可されます。最初のミントは `mintable` を `Not` に反転し、`AssetDefinitionEvent::MintabilityChanged` に加えて、可聴性のための詳細な `AssetDefinitionEvent::MintabilityChangedDetailed { asset_definition, minted_amount, authority }` を発行します。
    - `Limited(n)`: `n` の追加のミント操作を許可します。ミントが成功するたびにカウンターがデクリメントされます。ゼロに達すると、定義は `Not` に切り替わり、上記と同じ `MintabilityChanged` イベントを発行します。
    - `Not`: エラー `MintabilityError::MintUnmintable`。
  - 状態の変更: ミントに欠落している場合はアセットを作成します。書き込み時に残高がゼロになると、資産エントリが削除されます。
  - イベント: `AssetEvent::Added`/`AssetEvent::Removed`、`AssetDefinitionEvent::MintabilityChanged` (`Once` または `Limited(n)` がその許容量を使い果たしたとき)。
  - エラー: `TypeError::AssetNumericSpec(Mismatch)`、`MathError::Overflow`/`NotEnoughQuantity`。コード: `core/.../isi/asset.rs`。- トリガーの繰り返しのミント/バーン: トリガーの `action.repeats` カウントを変更します。
  - 前提条件: ミントでは、フィルターはミント可能である必要があります。算術演算はオーバーフロー/アンダーフローがあってはなりません。
  - イベント: `TriggerEvent::Extended`/`TriggerEvent::Shortened`。
  - エラー: 無効なミントでは `MathError::Overflow`。欠落している場合は `FindError::Trigger`。コード: `core/.../isi/triggers/mod.rs`。

### 転送
タイプ: `Transfer<S: Identifiable, O, D: Identifiable>`、`TransferBox` としてボックス化されます。

- 資産 (数値): ソース `AssetId` から減算し、宛先 `AssetId` に追加します (同じ定義、異なるアカウント)。ゼロ化されたソースアセットを削除します。
  - 前提条件: ソースアセットが存在します。値は `spec` を満たします。
  - イベント: `AssetEvent::Removed` (ソース)、`AssetEvent::Added` (宛先)。
  - エラー: `FindError::Asset`、`TypeError::AssetNumericSpec`、`MathError::NotEnoughQuantity/Overflow`。コード: `core/.../isi/asset.rs`。

- ドメイン所有権: `Domain.owned_by` を宛先アカウントに変更します。
  - 前提条件: 両方のアカウントが存在します。ドメインが存在します。
  - イベント: `DomainEvent::OwnerChanged`。
  - エラー: `FindError::Account/Domain`。コード: `core/.../isi/domain.rs`。

- AssetDefinition の所有権: `AssetDefinition.owned_by` を宛先アカウントに変更します。
  - 前提条件: 両方のアカウントが存在します。定義が存在します。ソースは現在それを所有している必要があります。権限は、ソースアカウント、ソースドメイン所有者、または資産定義ドメイン所有者である必要があります。
  - イベント: `AssetDefinitionEvent::OwnerChanged`。
  - エラー: `FindError::Account/AssetDefinition`。コード: `core/.../isi/account.rs`。- NFT 所有権: `Nft.owned_by` を宛先アカウントに変更します。
  - 前提条件: 両方のアカウントが存在します。 NFTは存在します。ソースは現在それを所有している必要があります。権限は、ソースアカウント、ソースドメイン所有者、NFTドメイン所有者、またはそのNFTの`CanTransferNft`を保持している必要があります。
  - イベント: `NftEvent::OwnerChanged`。
  - エラー: `FindError::Account/Nft`、`InvariantViolation` (ソースが NFT を所有していない場合)。コード: `core/.../isi/nft.rs`。

### メタデータ: Key-Value の設定/削除
タイプ: `SetKeyValue<T>` および `RemoveKeyValue<T>`、`T ∈ { Domain, Account, AssetDefinition, Nft, Trigger }`。ボックス化された列挙型が提供されます。

- 設定: `Metadata[key] = Json(value)` を挿入または置換します。
- 削除: キーを削除します。欠落している場合はエラー。
- イベント: `<Target>Event::MetadataInserted` / `MetadataRemoved` (古い/新しい値)。
- エラー: ターゲットが存在しない場合は `FindError::<Target>`。削除用のキーが見つからない場合は `FindError::MetadataKey`。コード: `crates/iroha_data_model/src/isi/transparent.rs` およびターゲットごとのエグゼキューターの暗示。

### 権限と役割: 付与/取り消し
型: `Grant<O, D>` および `Revoke<O, D>`。`Permission`/`Role` から `Account` へ/`Permission` から `Role` へ/`Role` のボックス化された列挙型。- アカウントに権限を付与: すでに固有でない限り、`Permission` を追加します。イベント: `AccountEvent::PermissionAdded`。エラー: 重複している場合は `Repetition(Grant, Permission)`。コード: `core/.../isi/account.rs`。
- アカウントからの権限の取り消し: 存在する場合は削除します。イベント: `AccountEvent::PermissionRemoved`。エラー: 存在しない場合は `FindError::Permission`。コード: `core/.../isi/account.rs`。
- アカウントに役割を付与: 存在しない場合は、`(account, role)` マッピングを挿入します。イベント: `AccountEvent::RoleGranted`。エラー: `Repetition(Grant, RoleId)`。コード: `core/.../isi/account.rs`。
- アカウントからロールを取り消す: マッピングが存在する場合は削除します。イベント: `AccountEvent::RoleRevoked`。エラー: 存在しない場合は `FindError::Role`。コード: `core/.../isi/account.rs`。
- ロールに権限を付与: 権限を追加してロールを再構築します。イベント: `RoleEvent::PermissionAdded`。エラー: `Repetition(Grant, Permission)`。コード: `core/.../isi/world.rs`。
- ロールからの権限を取り消す: その権限なしでロールを再構築します。イベント: `RoleEvent::PermissionRemoved`。エラー: 存在しない場合は `FindError::Permission`。コード: `core/.../isi/world.rs`。### トリガー: 実行
タイプ: `ExecuteTrigger { trigger: TriggerId, args: Json }`。
- 動作: トリガー サブシステムの `ExecuteTriggerEvent { trigger_id, authority, args }` をキューに入れます。手動実行は、呼び出しトリガー (`ExecuteTrigger` フィルター) に対してのみ許可されます。フィルタは一致する必要があり、呼び出し元はトリガー アクション権限であるか、その権限に対して `CanExecuteTrigger` を保持している必要があります。ユーザー指定のエグゼキュータがアクティブな場合、トリガーの実行はランタイム エグゼキュータによって検証され、トランザクションのエグゼキュータの燃料バジェット (ベース `executor.fuel` とオプションのメタデータ `additional_fuel`) が消費されます。
- エラー: `FindError::Trigger` (登録されていない場合)。権限のない者から呼び出された場合は `InvariantViolation`。コード: `core/.../isi/triggers/mod.rs` (および `core/.../smartcontracts/isi/mod.rs` でのテスト)。

### アップグレードとログ
- `Upgrade { executor }`: 提供された `Executor` バイトコードを使用してエグゼキュータを移行し、エグゼキュータとそのデータ モデルを更新し、`ExecutorEvent::Upgraded` を発行します。エラー: 移行失敗時に `InvalidParameterError::SmartContract` としてラップされます。コード: `core/.../isi/world.rs`。
- `Log { level, msg }`: 指定されたレベルのノード ログを出力します。状態は変化しません。コード: `core/.../isi/world.rs`。

### エラーモデル
共通エンベロープ: 評価エラー、クエリの失敗、変換、エンティティが見つからない、繰り返し、ミント可能性、数学、無効なパラメーター、および不変条件違反のバリアントを含む `InstructionExecutionError`。列挙とヘルパーは、`pub mod error` の下の `crates/iroha_data_model/src/isi/mod.rs` にあります。

---## トランザクションと実行可能ファイル
- `Executable`: `Instructions(ConstVec<InstructionBox>)` または `Ivm(IvmBytecode)` のいずれか。バイトコードはbase64としてシリアル化されます。コード: `crates/iroha_data_model/src/transaction/executable.rs`。
- `TransactionBuilder`/`SignedTransaction`: メタデータ `chain_id`、`authority`、`creation_time_ms`、オプションの `ttl_ms`、および `nonce` を含む実行可能ファイルを構築、署名、およびパッケージ化します。コード: `crates/iroha_data_model/src/transaction/`。
- 実行時、`iroha_core` は、`Execute for InstructionBox` を介して `InstructionBox` バッチを実行し、適切な `*Box` または具体的な命令にダウンキャストします。コード: `crates/iroha_core/src/smartcontracts/isi/mod.rs`。
- ランタイム エグゼキュータ検証バジェット (ユーザー提供のエグゼキュータ): パラメータからのベース `executor.fuel` とオプションのトランザクション メタデータ `additional_fuel` (`u64`)。トランザクション内の命令/トリガー検証全体で共有されます。

---## 不変条件とメモ (テストとガードから)
- Genesis 保護: `genesis` ドメインまたは `genesis` ドメインのアカウントを登録できません。 `genesis` アカウントは登録できません。コード/テスト: `core/.../isi/world.rs`、`core/.../smartcontracts/isi/mod.rs`。
- 数値資産は、ミント/転送/書き込みに関して `NumericSpec` を満たす必要があります。仕様の不一致により `TypeError::AssetNumericSpec` が生成されます。
- ミント可能性: `Once` は 1 回のミントを許可し、その後 `Not` に切り替わります。 `Limited(n)` では、`Not` に切り替える前に、正確に `n` のミントが許可されます。 `Infinitely` でミントを禁止しようとすると `MintabilityError::ForbidMintOnMintable` が発生し、`Limited(0)` を構成すると `MintabilityError::InvalidMintabilityTokens` が発生します。
- メタデータ操作はキーが正確です。存在しないキーを削除するとエラーになります。
- トリガーフィルターは作成不可能にすることができます。その場合、`Register<Trigger>` は `Exactly(1)` の繰り返しのみを許可します。
- トリガーメタデータキー `__enabled` (ブール値) ゲート実行。デフォルトが有効になっていない場合は、無効なトリガーはデータ/時間/呼び出しごとのパス全体でスキップされます。
- 決定論: すべての算術演算はチェックされた演算を使用します。アンダー/オーバーフローは型指定された数学エラーを返します。残高がゼロの場合、資産エントリが削除されます (非表示状態はありません)。

---## 実用的な例
- 鋳造と転送:
  - `Mint::asset_numeric(10, asset_id)` → 仕様/ミント可能性で許可されている場合は 10 を追加します。イベント: `AssetEvent::Added`。
  - `Transfer::asset_numeric(asset_id, 5, to_account)` → 5 移動します。削除/追加のイベント。
- メタデータの更新:
  - `SetKeyValue::account(account_id, "avatar".parse()?, json)` → アップサート; `RemoveKeyValue::account(...)` 経由で削除します。
- 役割/権限の管理:
  - `Grant::account_role(role_id, account)`、`Grant::role_permission(perm, role)`、およびそれらに相当する `Revoke`。
- トリガーのライフサイクル:
  - フィルタによって暗黙的にミント可能性チェックが行われる `Register::trigger(Trigger::new(id, Action::new(exec, repeats, authority, filter)))`。 `ExecuteTrigger::new(id).with_args(&args)` は設定された権限と一致する必要があります。
  - メタデータ キー `__enabled` を `false` に設定することでトリガーを無効にできます (デフォルトが有効になっていない)。 `SetKeyValue::trigger` または IVM `set_trigger_enabled` システムコールを介して切り替えます。
  - トリガー ストレージはロード時に修復されます。重複 ID、不一致 ID、欠落バイトコードを参照するトリガーは削除されます。バイトコード参照カウントが再計算されます。
  - 実行時にトリガーの IVM バイトコードが欠落している場合、トリガーは削除され、実行は失敗結果を伴う no-op として扱われます。
  - 枯渇したトリガーはすぐに削除されます。実行中に枯渇したエントリが見つかった場合、そのエントリは取り除かれ、欠落しているものとして扱われます。
- パラメータの更新:
  - `SetParameter(SumeragiParameter::BlockTimeMs(2500).into())` は `ConfigurationEvent::Changed` を更新および発行します。CLI / Torii `aid` + エイリアスの例:
- 正規の補助 + 明示的な名前 + 長いエイリアスを使用して登録します。
  - `iroha ledger asset definition register --id aid:2f17c72466f84a4bb8a8e24884fdcd2f --name pkr --alias pkr#ubl@sbp`
- 正規の補助 + 明示的な名前 + 短いエイリアスを使用して登録します。
  - `iroha ledger asset definition register --id aid:550e8400e29b41d4a7164466554400dd --name pkr --alias pkr#sbp`
- エイリアス + アカウント コンポーネントによるミント:
  - `iroha ledger asset mint --definition-alias pkr#ubl@sbp --account <i105> --quantity 500`
- エイリアスを正規エイドに解決します。
  - JSON を使用した `POST /v2/assets/aliases/resolve` `{ "alias": "pkr#ubl@sbp" }`

移行メモ:
- `name#domain` テキストのアセット定義 ID は、最初のリリースでは意図的にサポートされていません。
- ミント/バーン/トランスファーの境界におけるアセット ID は正規の `norito:<hex>` のままです。 `iroha tools encode asset-id` と `--definition aid:...`、または `--alias ...` と `--account` を使用します。

---

## トレーサビリティ (選択されたソース)
 - データ モデル コア: `crates/iroha_data_model/src/{account.rs,domain.rs,asset/**,nft.rs,role.rs,permission.rs,metadata.rs,trigger/**,parameter/**}`。
 - ISI 定義とレジストリ: `crates/iroha_data_model/src/isi/{mod.rs,register.rs,transfer.rs,mint_burn.rs,transparent.rs,registry.rs}`。
 - ISI 実行: `crates/iroha_core/src/smartcontracts/isi/{mod.rs,world.rs,domain.rs,account.rs,asset.rs,nft.rs,triggers/**}`。
 - イベント: `crates/iroha_data_model/src/events/**`。
 - トランザクション: `crates/iroha_data_model/src/transaction/**`。

この仕様をレンダリングされた API/動作テーブルに拡張したい場合、またはすべての具体的なイベント/エラーにクロスリンクしたい場合は、その言葉を言ってください。私がそれを拡張します。