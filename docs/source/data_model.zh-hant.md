---
lang: zh-hant
direction: ltr
source: docs/source/data_model.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8b6388355a41797eb7d0b7f47cfa8fcac4e136c5a2e5eb0a264384ecdba930b8
source_last_modified: "2026-02-01T13:51:49.945202+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Iroha v2 數據模型 – 深入探討

本文檔解釋了構成 Iroha v2 數據模型的結構、標識符、特徵和協議，這些模型在 `iroha_data_model` 包中實現並在整個工作區中使用。它旨在成為您可以查看並提出更新建議的精確參考。

## 範圍和基礎

- 目的：為域對象（域、賬戶、資產、NFT、角色、權限、對等點）、狀態更改指令（ISI）、查詢、觸發器、交易、區塊和參數提供規範類型。
- 序列化：所有公共類型派生 Norito 編解碼器 (`norito::codec::{Encode, Decode}`) 和架構 (`iroha_schema::IntoSchema`)。 JSON 在功能標誌後面有選擇地使用（例如，用於 HTTP 和 `Json` 有效負載）。
- IVM 注意：當以 Iroha 虛擬機 (IVM) 為目標時，某些反序列化時驗證被禁用，因為主機在調用合約之前執行驗證（請參閱 `src/lib.rs` 中的 crate 文檔）。
- FFI 門：某些類型通過 `ffi_export`/`ffi_import` 後面的 `iroha_ffi` 有條件地註釋 FFI，以避免不需要 FFI 時的開銷。

## 核心特徵和助手

- `Identifiable`：實體具有穩定的`Id`和`fn id(&self) -> &Self::Id`。應使用 `IdEqOrdHash` 導出，以實現地圖/設置的友好性。
- `Registrable`/`Registered`：許多實體（例如，`Domain`、`AssetDefinition`、`Role`）使用構建器模式。 `Registered` 將運行時類型與適合註冊事務的輕量級構建器類型 (`With`) 聯繫起來。
- `HasMetadata`：統一訪問鍵/值 `Metadata` 映射。
- `IntoKeyValue`：存儲分割助手，分別存儲 `Key`（ID）和 `Value`（數據）以減少重複。
- `Owned<T>`/`Ref<'world, K, V>`：在存儲和查詢過濾器中使用輕量級包裝器以避免不必要的複制。

## 名稱和標識符

- `Name`：有效的文本標識符。不允許空格和保留字符 `@`、`#`、`$`（在復合 ID 中使用）。可通過 `FromStr` 進行構造並進行驗證。名稱在解析時標準化為 Unicode NFC（規範等效的拼寫被視為相同並存儲組合）。特殊名稱 `genesis` 被保留（檢查不區分大小寫）。
- `IdBox`：任何支持的 ID 的求和型信封（`DomainId`、`AccountId`、`AssetDefinitionId`、`AssetId`、`NftId`、`PeerId`、 `TriggerId`、`RoleId`、`Permission`、`CustomParameterId`）。對於通用流和 Norito 編碼作為單一類型很有用。
- `ChainId`：用於交易中重放保護的不透明鏈標識符。ID 的字符串形式（可與 `Display`/`FromStr` 進行往返）：
- `DomainId`：`name`（例如，`wonderland`）。
- `AccountId`：通過 `AccountAddress` 編碼的規範標識符，公開 IH58、Sora 壓縮 (`sora…`) 和規範十六進制編解碼器 (`AccountAddress::to_ih58`、`to_compressed_sora`、`canonical_hex`、 `parse_encoded`）。 IH58是首選賬戶格式； `sora…` 形式對於僅 Sora 的 UX 來說是第二好的。人性化的路由別名 `alias@domain` 保留用於 UX，但不再被視為權威標識符。 Torii 通過 `AccountAddress::parse_encoded` 規範傳入的字符串。帳戶 ID 支持單密鑰和多重簽名控制器。
- `AssetDefinitionId`：`asset#domain`（例如，`xor#soramitsu`）。
- `AssetId`: canonical encoded literal `norito:<hex>` (legacy textual forms are not supported in first release).
- `NftId`：`nft$domain`（例如，`rose$garden`）。
- `PeerId`：`public_key`（對等平等由公鑰決定）。

## 實體

### 域名
- `DomainId { name: Name }` – 唯一名稱。
- `Domain { id, logo: Option<IpfsPath>, metadata: Metadata, owned_by: AccountId }`。
- 生成器：`NewDomain` 與 `with_logo`、`with_metadata`，然後 `Registrable::build(authority)` 設置 `owned_by`。

### 賬戶
- `AccountId { domain: DomainId, controller: AccountController }`（控制器=單密鑰或多簽名策略）。
- `Account { id, metadata, label?, uaid? }` — `label` 是密鑰更新記錄使用的可選穩定別名，`uaid` 攜帶可選的 Nexus 範圍 [通用帳戶 ID](./universal_accounts_guide.md)。
- 生成器：`NewAccount` 通過 `Account::new(id)`； `HasMetadata` 對於構建者和實體。

### 資產定義和資產
- `AssetDefinitionId { domain: DomainId, name: Name }`。
- `AssetDefinition { id, spec: NumericSpec, mintable: Mintable, logo: Option<IpfsPath>, metadata, owned_by: AccountId, total_quantity: Numeric }`。
  - `Mintable`: `Infinitely` | `Once` | `Limited(u32)` | `Not`。
  - 建造者：`AssetDefinition::new(id, spec)` 或便利 `numeric(id)`； `metadata`、`mintable`、`owned_by` 的設定器。
- `AssetId { account: AccountId, definition: AssetDefinitionId }`。
- `Asset { id, value: Numeric }` 與存儲友好型 `AssetEntry`/`AssetValue`。
- `AssetTotalQuantityMap = BTreeMap<AssetDefinitionId, Numeric>` 公開用於摘要 API。

### NFT
- `NftId { domain: DomainId, name: Name }`。
- `Nft { id, content: Metadata, owned_by: AccountId }`（內容是任意鍵/值元數據）。
- 生成器：`NewNft` 通過 `Nft::new(id, content)`。

### 角色和權限
- `RoleId { name: Name }`。
- `Role { id, permissions: BTreeSet<Permission> }` 與構建器 `NewRole { inner: Role, grant_to: AccountId }`。
- `Permission { name: Ident, payload: Json }` – `name` 和有效負載模式必須與活動的 `ExecutorDataModel` 一致（見下文）。

### 同行
- `PeerId { public_key: PublicKey }`。
- `Peer { address: SocketAddr, id: PeerId }` 和可解析的 `public_key@address` 字符串形式。### 加密原語（功能 `sm`）
- `Sm2PublicKey` 和 `Sm2Signature`：SM2 的 SEC1 兼容點和固定寬度 `r∥s` 簽名。構造函數驗證曲線成員資格和區分 ID； Norito 編碼鏡像 `iroha_crypto` 使用的規範表示。
- `Sm3Hash`：`[u8; 32]` 代表 GM/T 0004 摘要的新類型，用於清單、遙測和系統調用響應。
- `Sm4Key`：主機系統調用和數據模型裝置之間共享的 128 位對稱密鑰包裝器。
這些類型與現有的 Ed25519/BLS/ML-DSA 原語並存，一旦使用 `--features sm` 構建工作區，它們就會成為公共模式的一部分。

### 觸發器和事件
- `TriggerId { name: Name }` 和 `Trigger { id, action: action::Action }`。
- `action::Action { executable: Executable, repeats: Repeats, authority: AccountId, filter: EventFilterBox, metadata }`。
  - `Repeats`：`Indefinitely` 或 `Exactly(u32)`；包括排序和消耗實用程序。
  - 安全：`TriggerCompleted` 不能用作操作的過濾器（在（反）序列化期間驗證）。
- `EventBox`：管道、管道批、數據、時間、執行觸發和触發完成事件的總和類型； `EventFilterBox` 鏡像訂閱和触發過濾器。

## 參數及配置

- 系統參數係列（所有 `Default`ed，攜帶吸氣劑，並轉換為單獨的枚舉）：
- `SumeragiParameters { block_time_ms, commit_time_ms, min_finality_ms, pacing_factor_bps, max_clock_drift_ms, collectors_k, collectors_redundant_send_r }`。
  - `BlockParameters { max_transactions: NonZeroU64 }`。
  - `TransactionParameters { max_signatures, max_instructions, ivm_bytecode_size, max_tx_bytes, max_decompressed_bytes }`。
  - `SmartContractParameters { fuel, memory, execution_depth }`。
- `Parameters` 將所有系列和 `custom: BTreeMap<CustomParameterId, CustomParameter>` 分組。
- 單參數枚舉：`SumeragiParameter`、`BlockParameter`、`TransactionParameter`、`SmartContractParameter` 用於類似 diff 的更新和迭代。
- 自定義參數：執行器定義，攜帶為`Json`，由`CustomParameterId`（a `Name`）標識。

## ISI（Iroha 特別說明）

- 核心特徵：`Instruction` 與 `dyn_encode`、`as_any` 和穩定的每類型標識符 `id()`（默認為具體類型名稱）。所有指令均為 `Send + Sync + 'static`。
- `InstructionBox`：擁有 `Box<dyn Instruction>` 包裝器，通過類型 ID + 編碼字節實現克隆/eq/ord。
- 內置指令系列的組織方式如下：
  - `mint_burn`、`transfer`、`register` 和 `transparent` 幫助程序包。
  - 元流的類型枚舉：`InstructionType`，盒裝總和，如 `SetKeyValueBox`（域/帳戶/asset_def/nft/trigger）。
- 錯誤：`isi::error` 下的豐富錯誤模型（評估類型錯誤、查找錯誤、可鑄造性、數學、無效參數、重複、不變量）。
- 指令註冊表：`instruction_registry!{ ... }` 宏構建一個按類型名稱鍵入的運行時解碼註冊表。由 `InstructionBox` 克隆和 Norito serde 使用來實現動態（反）序列化。如果沒有通過 `set_instruction_registry(...)` 顯式設置註冊表，則在首次使用時會延遲安裝包含所有核心 ISI 的內置默認註冊表，以保持二進製文件的穩健性。

## 交易- `Executable`：`Instructions(ConstVec<InstructionBox>)` 或 `Ivm(IvmBytecode)`。 `IvmBytecode` 序列化為 base64（`Vec<u8>` 上的透明新類型）。
- `TransactionBuilder`：使用 `chain`、`authority`、`creation_time_ms`、可選 `time_to_live_ms` 和 `nonce`、`metadata` 以及`Executable`。
  - 幫助程序：`with_instructions`、`with_bytecode`、`with_executable`、`with_metadata`、`set_nonce`、`set_ttl`、`set_creation_time`、`sign`。
- `SignedTransaction`（版本為`iroha_version`）：攜帶`TransactionSignature`和有效負載；提供哈希和簽名驗證。
- 切入點和結果：
  - `TransactionEntrypoint`: `External(SignedTransaction)` | `Time(TimeTriggerEntrypoint)`。
  - `TransactionResult` = `Result<DataTriggerSequence, TransactionRejectionReason>` 帶有哈希助手。
  - `ExecutionStep(ConstVec<InstructionBox>)`：事務中的單個有序批次指令。

## 塊

- `SignedBlock`（版本化）封裝：
  - `signatures: BTreeSet<BlockSignature>`（來自驗證器），
  - `payload: BlockPayload { header: BlockHeader, transactions: Vec<SignedTransaction> }`，
  - `result: BlockResult`（輔助執行狀態），包含 `time_triggers`、條目/結果 Merkle 樹、`transaction_results` 和 `fastpq_transcripts: BTreeMap<Hash, Vec<TransferTranscript>>`。
- 實用程序：`presigned`、`set_transaction_results(...)`、`set_transaction_results_with_transcripts(...)`、`header()`、`signatures()`、`hash()`、`add_signature`、`replace_signatures`。
- Merkle 根：交易入口點和結果通過 Merkle 樹提交；結果 Merkle root 被放入區塊頭中。
- 區塊包含證明 (`BlockProofs`) 公開條目/結果 Merkle 證明和 `fastpq_transcripts` 映射，以便鏈下證明者可以獲取與交易哈希相關的傳輸增量。
- `ExecWitness` 消息（通過 Torii 流式傳輸並搭載共識八卦）現在包括 `fastpq_transcripts` 和帶有嵌入式 `public_inputs`（dsid、slot、roots、perm_root、 tx_set_hash），因此外部證明者可以攝取規範的 FASTPQ 行，而無需重新編碼轉錄本。

## 查詢

- 兩種口味：
  - 單數：實施 `SingularQuery<Output>`（例如，`FindParameters`、`FindExecutorDataModel`）。
  - 可迭代：實現 `Query<Item>`（例如，`FindAccounts`、`FindAssets`、`FindDomains` 等）。
- 類型擦除表格：
  - `QueryBox<T>` 是一個盒裝的、已擦除的 `Query<Item = T>`，帶有由全局註冊表支持的 Norito serde。
  - `QueryWithFilter<T> { query, predicate, selector }` 將查詢與 DSL 謂詞/選擇器配對；通過 `From` 轉換為已擦除的可迭代查詢。
- 註冊表和編解碼器：
  - `query_registry!{ ... }` 構建一個全局註冊表，按類型名稱將具體查詢類型映射到構造函數以進行動態解碼。
  - `QueryRequest = Singular(SingularQueryBox) | Start(QueryWithParams) | Continue(ForwardCursor)` 和 `QueryResponse = Singular(..) | Iterable(QueryOutput)`。
  - `QueryOutputBatchBox` 是同質向量的求和類型（例如，`Vec<Account>`、`Vec<Name>`、`Vec<AssetDefinition>`、`Vec<BlockHeader>`），以及用於高效分頁的元組和擴展幫助程序。
- DSL：在 `query::dsl` 中實現，具有用於編譯時檢查謂詞和選擇器的投影特徵 (`HasProjection<PredicateMarker>` / `SelectorMarker`)。如果需要，`fast_dsl` 功能會公開更輕的變體。

## 執行器和可擴展性- `Executor { bytecode: IvmBytecode }`：驗證器執行的代碼包。
- `ExecutorDataModel { parameters: CustomParameters, instructions: BTreeSet<Ident>, permissions: BTreeSet<Ident>, schema: Json }` 聲明執行器定義的域：
  - 自定義配置參數，
  - 自定義指令標識符，
  - 權限令牌標識符，
  - 描述客戶端工具自定義類型的 JSON 模式。
- `data_model/samples/executor_custom_data_model` 下存在定制示例，演示：
  - 通過 `iroha_executor_data_model::permission::Permission` 派生自定義權限令牌，
  - 自定義參數定義為可轉換為 `CustomParameter` 的類型，
  - 自定義指令序列化到 `CustomInstruction` 中以供執行。

### CustomInstruction（執行者定義的ISI）

- 類型：`isi::CustomInstruction { payload: Json }`，具有穩定的線 ID `"iroha.custom"`。
- 目的：在私有/聯盟網絡中執行程序特定指令或用於原型設計的信封，無需分叉公共數據模型。
- 默認執行器行為：`iroha_core` 中的內置執行器不執行 `CustomInstruction`，如果遇到會出現恐慌。自定義執行器必須將 `InstructionBox` 向下轉換為 `CustomInstruction` 並確定性地解釋所有驗證器上的有效負載。
- Norito：通過包含模式的 `norito::codec::{Encode, Decode}` 進行編碼/解碼； `Json` 有效負載是確定性序列化的。只要指令註冊表包含 `CustomInstruction`（它是默認註冊表的一部分），往返就是穩定的。
- IVM：Kotodama 編譯為 IVM 字節碼 (`.to`)，是應用程序邏輯的推薦路徑。僅將 `CustomInstruction` 用於尚無法用 Kotodama 表示的執行程序級擴展。確保同行之間的確定性和相同的執行器二進製文件。
- 不適用於公共網絡：不要用於異構執行者存在共識分叉風險的公共鏈。當您需要平台功能時，更願意建議新的內置 ISI 上游。

## 元數據

- `Metadata(BTreeMap<Name, Json>)`：附加到多個實體的鍵/值存儲（`Domain`、`Account`、`AssetDefinition`、`Nft`、觸發器和事務）。
- API：`contains`、`iter`、`get`、`insert` 和（與 `transparent_api`）`remove`。

## 特徵和確定性

- 功能控制可選 API（`std`、`json`、`transparent_api`、`ffi_export`、`ffi_import`、`fast_dsl`、`http`、 `fault_injection`）。
- 確定性：所有序列化都使用 Norito 編碼，以便跨硬件移植。 IVM 字節碼是不透明的字節 blob；執行不得引入非確定性縮減。主機驗證事務並向 IVM 確定性地提供輸入。

### 透明 API (`transparent_api`)- 目的：公開對 `#[model]` 結構/枚舉的完整、可變訪問，以供內部組件（例如 Torii、執行器和集成測試）使用。如果沒有它，這些項目就會故意不透明，因此外部 SDK 只能看到安全的構造函數和編碼的有效負載。
- 機制：`iroha_data_model_derive::model` 宏使用 `#[cfg(feature = "transparent_api")] pub` 重寫每個公共字段，並為默認構建保留一個私有副本。啟用該功能會翻轉這些 cfgs，因此解構 `Account`、`Domain`、`Asset` 等在其定義模塊之外變得合法。
- 表面檢測：包導出 `TRANSPARENT_API: bool` 常量（生成為 `transparent_api.rs` 或 `non_transparent_api.rs`）。下游代碼可以在需要回退到不透​​​​明助手時檢查此標誌和分支。
- 啟用：將 `features = ["transparent_api"]` 添加到 `Cargo.toml` 的依賴項中。需要 JSON 投影的工作區 crate（例如 `iroha_torii`）會自動轉發該標誌，但第三方使用者應將其關閉，除非他們控制部署並接受更廣泛的 API 表面。

## 簡單示例

創建域和帳戶、定義資產並按照說明構建交易：

```rust
use iroha_data_model::prelude::*;
use iroha_crypto::KeyPair;
use iroha_primitives::numeric::Numeric;

// Domain
let domain_id: DomainId = "wonderland".parse().unwrap();
let new_domain = Domain::new(domain_id.clone()).with_metadata(Metadata::default());

// Account
let kp = KeyPair::random();
let account_id = AccountId::new(domain_id.clone(), kp.public_key().clone());
let new_account = Account::new(account_id.clone()).with_metadata(Metadata::default());

// Asset definition and an asset for the account
let asset_def_id: AssetDefinitionId = "xor#wonderland".parse().unwrap();
let new_asset_def = AssetDefinition::numeric(asset_def_id.clone())
    .with_metadata(Metadata::default());
let asset_id = AssetId::new(asset_def_id.clone(), account_id.clone());
let asset = Asset::new(asset_id.clone(), Numeric::from(100));

// Build a transaction with instructions (pseudo-ISI; exact ISI types live under `isi`)
let chain_id: ChainId = "dev-chain".parse().unwrap();
let tx = TransactionBuilder::new(chain_id, account_id.clone())
    .with_instructions(vec![ /* Register/ Mint/ Transfer instructions here */ ])
    .sign(kp.private_key());
```

通過DSL查詢賬戶和資產：

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

使用 IVM 智能合約字節碼：

```rust
use iroha_data_model::prelude::*;

let bytecode = IvmBytecode::from_compiled(include_bytes!("contract.to").to_vec());
let tx = TransactionBuilder::new("dev-chain".parse().unwrap(), account_id.clone())
    .with_bytecode(bytecode)
    .sign(kp.private_key());
```

## 版本控制

- `SignedTransaction`、`SignedBlock` 和 `SignedQuery` 是規範的 Norito 編碼結構。當通過 `EncodeVersioned` 編碼時，每個都實現 `iroha_version::Version`，以當前 ABI 版本（當前為 `1`）作為其有效負載的前綴。

## 評論註釋/潛在更新

- 查詢 DSL：考慮記錄一個穩定的面向用戶的子集以及常見過濾器/選擇器的示例。
- 指令系列：展開公共文檔，列出 `mint_burn`、`register`、`transfer` 公開的內置 ISI 變體。

---
如果任何部分需要更多深度（例如，完整的 ISI 目錄、完整的查詢註冊表列表或塊頭字段），請告訴我，我將相應地擴展這些部分。