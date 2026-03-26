---
lang: zh-hant
direction: ltr
source: docs/source/data_model.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 683bfb31442f8f4ce7b1bf5038f9dba92fe092545e655f43b51195c21535d3c4
source_last_modified: "2026-03-12T11:24:23.059339+00:00"
translation_last_reviewed: 2026-03-12
translator: machine-google-reviewed
---

# Iroha v2 資料模型 – 深入探討

本文檔解釋了構成 Iroha v2 資料模型的結構、識別碼、特徵和協議，這些模型在 `iroha_data_model` 套件中實現並在整個工作區中使用。它旨在成為您可以查看並提出更新建議的精確參考。

## 範圍和基礎

- 目的：為網域物件（網域、帳戶、資產、NFT、角色、權限、對等點）、狀態變更指令（ISI）、查詢、觸發器、交易、區塊和參數提供規格類型。
- 序列化：所有公共類型派生 Norito 編解碼器 (`norito::codec::{Encode, Decode}`) 和架構 (`iroha_schema::IntoSchema`)。 JSON 在功能標誌後面選擇性地使用（例如，用於 HTTP 和 `Json` 有效負載）。
- IVM 注意：當以 Iroha 虛擬機器 (IVM) 為目標時，某些反序列化時驗證已停用，因為主機在呼叫合約之前執行驗證（請參閱 `src/lib.rs` 中的主機在呼叫合約之前執行驗證（請參閱 `src/lib.rs`）。
- FFI 閘：某些類型透過 `ffi_export`/`ffi_import` 後面的 `iroha_ffi` 有條件地註釋 FFI，以避免不需要 FFI 時的開銷。

## 核心特徵和助手- `Identifiable`：實體具有穩定的`Id`和`fn id(&self) -> &Self::Id`。應使用 `IdEqOrdHash` 導出，以實現地圖/設定的友善性。
- `Registrable`/`Registered`：許多實體（例如，`Domain`、`AssetDefinition`、`Role`）使用建構器模式。 `Registered` 將運行時類型與適合註冊事務的輕量級建構器類型 (`With`) 連結起來。
- `HasMetadata`：統一存取鍵/值 `Metadata` 對映。
- `IntoKeyValue`：儲存分割助手，分別儲存 `Key`（ID）和 `Value`（資料）以減少重複。
- `Owned<T>`/`Ref<'world, K, V>`：在儲存和查詢過濾器中使用輕量級包裝器以避免不必要的複製。

## 名稱和識別符- `Name`：有效的文字識別碼。不允許空格和保留字元 `@`、`#`、`$`（在複合 ID 中使用）。可透過 `FromStr` 進行建構並進行驗證。名稱在解析時標準化為 Unicode NFC（規範等效的拼字被視為相同並儲存組合）。特殊名稱 `genesis` 被保留（檢查不區分大小寫）。
- `IdBox`：任何支援的 ID 的求和型信封（`DomainId`、`AccountId`、`AssetDefinitionId`、`AssetId`、`AssetDefinitionId`、`AssetId`、Norito `TriggerId`、`RoleId`、`Permission`、`CustomParameterId`）。對於通用流和 Norito 編碼作為單一類型很有用。
- `ChainId`：用於交易中重播保護的不透明鏈標識符。ID 的字串形式（可與 `Display`/`FromStr` 往返）：
- `DomainId`：`name`（例如，`wonderland`）。
- `AccountId`：僅透過 `AccountAddress` 編碼為 I105 的規範無域帳戶識別碼。解析器輸入必須是規範的 I105；域後綴 (`@domain`)、規範 I105 文字、別名文字、規範十六進位解析器輸入、舊版 `norito:` 有效負載和 `uaid:`/I1800000000000000 帳戶形式將被拒絕。
- `AssetDefinitionId`：規格 `unprefixed Base58 address with versioning and checksum`（UUID-v4 位元組）。
- `AssetId`：規範編碼文字 `<base58-asset-id>#<katakana-i105-account-id>`（第一個版本不支援舊文字形式）。
- `NftId`：`nft$domain`（例如，`rose$garden`）。
- `PeerId`：`public_key`（對等平等由公鑰決定）。

## 實體

### 域名
- `DomainId { name: Name }` – 唯一名稱。
- `Domain { id, logo: Option<SorafsUri>, metadata: Metadata, owned_by: AccountId }`。
- 生成器：`NewDomain` 與 `with_logo`、`with_metadata`，然後 `Registrable::build(authority)` 設定 `owned_by`。### 帳戶
- `AccountId` 是由控制器鍵入並編碼為規範 I105 的規範無網域帳戶身分。
- `ScopedAccountId { account: AccountId, domain: DomainId }` 僅在需要範圍視圖時才攜帶明確域上下文。
- `Account { id, metadata, label?, uaid? }` — `label` 是密鑰更新記錄使用的可選穩定別名，`uaid` 攜帶可選的 Nexus 範圍 [通用帳戶 ID](./universal_accounts_guide.md)。
- 生成器：`NewAccount` 通過 `Account::new(id)`；註冊需要明確的 `ScopedAccountId` 域，並且不會從預設值推斷出域。

### 資產定義與資產
- `AssetDefinitionId { aid_bytes: [u8; 16] }` 在文字上顯示為 `unprefixed Base58 address`。
- `AssetDefinition { id, name, description?, alias?, spec: NumericSpec, mintable: Mintable, logo: Option<SorafsUri>, metadata, owned_by: AccountId, total_quantity: Numeric }`。

  - Torii asset-definition responses may include `alias_binding { alias, status, lease_expiry_ms, grace_until_ms, bound_at_ms }`; alias selectors resolve against latest committed block time and stop resolving after grace, while direct reads may still show `expired_pending_cleanup` until sweep.
  - `name` 是必需的人性化顯示文本，且不得包含 `#`/`@`。
  - `alias` 是可選的，並且必須是以下之一：
    - `<name>#<domain>.<dataspace>`
    - `<name>#<dataspace>`
    左側段與 `AssetDefinition.name` 完全匹配。
  - `Mintable`: `Infinitely` | `Once` | `Limited(u32)` | `Not`。
  - 建造者：`AssetDefinition::new(id, spec)` 或便利 `numeric(id)`； `name` 是必需的，並且必須透過 `.with_name(...)` 設定。
- `AssetId { account: AccountId, definition: AssetDefinitionId, scope: AssetBalanceScope }`。
- `Asset { id, value: Numeric }` 與儲存友善 `AssetEntry`/`AssetValue`。
- `AssetBalanceScope`：`Global` 用於無限制餘額，`Dataspace(DataSpaceId)` 用於資料空間受限餘額。
- `AssetTotalQuantityMap = BTreeMap<AssetDefinitionId, Numeric>` 公開用於摘要 API。### NFT
- `NftId { domain: DomainId, name: Name }`。
- `Nft { id, content: Metadata, owned_by: AccountId }`（內容是任意鍵/值元資料）。
- 生成器：`NewNft` 通過 `Nft::new(id, content)`。

### 角色和權限
- `RoleId { name: Name }`。
- `Role { id, permissions: BTreeSet<Permission> }` 與建構器 `NewRole { inner: Role, grant_to: AccountId }`。
- `Permission { name: Ident, payload: Json }` – `name` 和有效負載架構必須與活動的 `ExecutorDataModel` 一致（見下文）。

### 同行
- `PeerId { public_key: PublicKey }`。
- `Peer { address: SocketAddr, id: PeerId }` 和可解析的 `public_key@address` 字串形式。

### 加密原語（功能 `sm`）
- `Sm2PublicKey` 和 `Sm2Signature`：SM2 的 SEC1 相容點和固定寬度 `r∥s` 簽章。建構函數驗證曲線成員資格和區分 ID； Norito 編碼鏡像 `iroha_crypto` 所使用的規範表示。
- `Sm3Hash`：`[u8; 32]` 代表 GM/T 0004 摘要的新類型，用於清單、遙測和系統呼叫回應。
- `Sm4Key`：主機系統呼叫和資料模型裝置之間共用的 128 位元對稱金鑰包裝器。
這些類型與現有的 Ed25519/BLS/ML-DSA 原語並存，一旦使用 `--features sm` 建置工作區，它們就會成為公共模式的一部分。### 触发器和事件
- `TriggerId { name: Name }` 和 `Trigger { id, action: action::Action }`。
- `action::Action { executable: Executable, repeats: Repeats, authority: AccountId, filter: EventFilterBox, metadata }`。
  - `Repeats`：`Indefinitely` 或 `Exactly(u32)`；包括排序和消耗实用程序。
  - 安全性：`TriggerCompleted` 不能用作操作的过滤器（在（反）序列化期间验证）。
- `EventBox`：管道、管道批次、資料、時間、執行觸發和觸發完成事件的總和類型；`EventFilterBox` 鏡像訂閱和觸發過濾器。

## 参数及配置

- 系统参数系列（所有 `Default`ed，携带吸气剂，并转换为单独的枚举）：
- `SumeragiParameters { block_time_ms, commit_time_ms, min_finality_ms, pacing_factor_bps, max_clock_drift_ms, collectors_k, collectors_redundant_send_r }`。
  - `BlockParameters { max_transactions: NonZeroU64 }`。
  - `TransactionParameters { max_signatures, max_instructions, ivm_bytecode_size, max_tx_bytes, max_decompressed_bytes }`。
  - `SmartContractParameters { fuel, memory, execution_depth }`。
- `Parameters` 将所有系列和 `custom: BTreeMap<CustomParameterId, CustomParameter>` 分组。
- 单参数枚举：`SumeragiParameter`、`BlockParameter`、`TransactionParameter`、`SmartContractParameter` 用于类似 diff 的更新和迭代。
- 自定义参数：执行器定义，携带为`Json`，由`CustomParameterId`（a `Name`）标识。

## ISI（Iroha 特別說明）- 核心特徵：`Instruction` 與 `dyn_encode`、`as_any` 和穩定的每類型識別碼 `id()`（預設為具體類型名稱）。所有指令均為 `Send + Sync + 'static`。
- `InstructionBox`：擁有 `Box<dyn Instruction>` 包裝器，透過類型 ID + 編碼位元組實現克隆/eq/ord。
- 內建指令系列的組織方式如下：
  - `mint_burn`、`transfer`、`register` 和 `transparent` 幫助程式包。
  - 元流的類型枚舉：`InstructionType`，盒裝總和，如 `SetKeyValueBox`（網域/帳戶/asset_def/nft/trigger）。
- 錯誤：`isi::error` 下的豐富錯誤模型（評估類型錯誤、尋找錯誤、可鑄造性、數學、無效參數、重複、不變量）。
- 指令登錄：`instruction_registry!{ ... }` 巨集建立一個按類型名稱鍵入的執行時間解碼註冊表。由 `InstructionBox` 克隆和 Norito serde 使用來實現動態（反）序列化。如果沒有透過 `set_instruction_registry(...)` 明確設定註冊表，則在首次使用時會延遲安裝包含所有核心 ISI 的內建預設註冊表，以保持二進位檔案的穩健性。

## 交易- `Executable`：`Instructions(ConstVec<InstructionBox>)` 或 `Ivm(IvmBytecode)`。 `IvmBytecode` 序列化為 base64（`Vec<u8>` 上的透明新型別）。
- `TransactionBuilder`：使用 `chain`、`authority`、`creation_time_ms`、選購 `time_to_live_ms` 和 `nonce`、I100000209X 和 `nonce`、I100002101002100121002100210021002100210021002100210021002100210021210002120100021210002121000212100021212000021220100022120002。
  - 幫助程式：`with_instructions`、`with_bytecode`、`with_executable`、`with_metadata`、`set_nonce`、`set_ttl`I1080202020202019000000218XI10802020202020202020202002020202020202020202020202020202020202020202020202020202020202020202002。
- `SignedTransaction`（版本為`iroha_version`）：攜帶`TransactionSignature`和有效負載；提供雜湊和簽章驗證。
- 切入點和結果：
  - `TransactionEntrypoint`: `External(SignedTransaction)` | `Time(TimeTriggerEntrypoint)`。
  - `TransactionResult` = `Result<DataTriggerSequence, TransactionRejectionReason>` 附有雜湊助手。
  - `ExecutionStep(ConstVec<InstructionBox>)`：事務中的單一有序批次指令。

## 區塊- `SignedBlock`（版本化）封裝：
  - `signatures: BTreeSet<BlockSignature>`（來自驗證者），
  - `payload: BlockPayload { header: BlockHeader, transactions: Vec<SignedTransaction> }`，
  - `result: BlockResult`（輔助執行狀態），包含 `time_triggers`、條目/結果 Merkle 樹、`transaction_results` 和 `fastpq_transcripts: BTreeMap<Hash, Vec<TransferTranscript>>`。
- 實用程序：`presigned`、`set_transaction_results(...)`、`set_transaction_results_with_transcripts(...)`、`header()`、`signatures()`、`hash()`、`add_signature`、`replace_signatures`。
- Merkle 根：交易入口點和結果透過 Merkle 樹提交；結果 Merkle root 被放入區塊頭中。
- 區塊包含證明 (`BlockProofs`) 公開條目/結果 Merkle 證明和 `fastpq_transcripts` 映射，以便鏈下證明者可以獲得與交易哈希相關的傳輸增量。
- `ExecWitness` 訊息（透過 Torii 串流並搭載共識八卦）現在包括 `fastpq_transcripts` 和帶有嵌入式 `public_inputs`（dsid、slot、roots、perm_root、 tx_0000250X（dsid、slot、roots、perm_root、 tx_set000250X（dsid、slot、roots、perm_root、 tx_set_QPAST行，而無需重新編碼轉錄本。

## 查詢- 兩種口味：
  - 單數：實施 `SingularQuery<Output>`（例如，`FindParameters`、`FindExecutorDataModel`）。
  - 可迭代：實現 `Query<Item>`（例如，`FindAccounts`、`FindAssets`、`FindDomains` 等）。
- 類型擦除表格：
  - `QueryBox<T>` 是一個盒裝的、已擦除的 `Query<Item = T>`，帶有由全域註冊表支援的 Norito serde。
  - `QueryWithFilter<T> { query, predicate, selector }` 將查詢與 DSL 謂詞/選擇器配對；透過 `From` 轉換為擦除的可迭代查詢。
- 註冊表和編解碼器：
  - `query_registry!{ ... }` 建立一個全域註冊表，按類型名稱將特定查詢類型對應到建構函數以進行動態解碼。
  - `QueryRequest = Singular(SingularQueryBox) | Start(QueryWithParams) | Continue(ForwardCursor)` 和 `QueryResponse = Singular(..) | Iterable(QueryOutput)`。
  - `QueryOutputBatchBox` 是同質向量的求和類型（例如，`Vec<Account>`、`Vec<Name>`、`Vec<AssetDefinition>`、`Vec<BlockHeader>`），以及NI00000268X、`Vec<BlockHeader>`），以及用於擴展高效分頁的元組和擴展幫助程式。
- DSL：在 `query::dsl` 中實現，具有用於編譯時檢查謂詞和選擇器的投影特徵 (`HasProjection<PredicateMarker>` / `SelectorMarker`)。如果需要，`fast_dsl` 功能會公開更輕的變體。

## 執行器和可擴充性- `Executor { bytecode: IvmBytecode }`：驗證器執行的代碼包。
- `ExecutorDataModel { parameters: CustomParameters, instructions: BTreeSet<Ident>, permissions: BTreeSet<Ident>, schema: Json }` 聲明執行者定義的領域：
  - 自訂配置參數，
  - 自訂指令標識符，
  - 權限令牌標識符，
  - 描述客戶端工具自訂類型的 JSON 模式。
- `data_model/samples/executor_custom_data_model` 下存在客製化範例，演示：
  - 透過 `iroha_executor_data_model::permission::Permission` 派生自訂權限令牌，
  - 自訂參數定義為可轉換為 `CustomParameter` 的類型，
  - 自訂指令序列化到 `CustomInstruction` 中以供執行。

### CustomInstruction（執行者定義的ISI）- 類型：`isi::CustomInstruction { payload: Json }`，具有穩定的線 ID `"iroha.custom"`。
- 目的：在私有/聯盟網路中執行程式特定指令或用於原型設計的信封，無需分叉公共資料模型。
- 預設執行器行為：`iroha_core` 中的內建執行器不執行 `CustomInstruction`，如果遇到會出現恐慌。自訂執行器必須將 `InstructionBox` 向下轉換為 `CustomInstruction` 並確定性地解釋所有驗證器上的有效負載。
- Norito：透過 `norito::codec::{Encode, Decode}` 進行編碼/解碼，包含模式； `Json` 有效負載是確定性序列化的。只要指令註冊表包含 `CustomInstruction`（它是預設註冊表的一部分），往返就是穩定的。
- IVM：Kotodama 編譯為 IVM 字節碼 (`.to`)，是應用程式邏輯的建議路徑。僅將 `CustomInstruction` 用於尚無法以 Kotodama 表示的執行程式級擴充。確保同行之間的確定性和相同的執行器二進位。
- 不適用於公共網路：不要用於異質執行者有共識分叉風險的公共鏈。當您需要平台功能時，更願意建議新的內建 ISI 上游。

## 元數據- `Metadata(BTreeMap<Name, Json>)`：附加到多個實體的鍵/值儲存（`Domain`、`Account`、`AssetDefinition`、`Nft`、觸發器和交易）。
- API：`contains`、`iter`、`get`、`insert` 和（使用 `transparent_api`）`remove`。

## 特徵和確定性

- 功能控制選用 API（`std`、`json`、`transparent_api`、`ffi_export`、`ffi_import`、`fast_dsl`、I030010302X `fault_injection`）。
- 確定性：所有序列化都使用 Norito 編碼，以便跨硬體移植。 IVM 字節碼是不透明的位元組 blob；執行不得引入非確定性縮減。主機驗證事務並向 IVM 確定性地提供輸入。

### 透明 API (`transparent_api`)- 目的：公開對 `#[model]` 結構/枚舉的完整、可變訪問，以供內部組件（例如 Torii、執行器和集成測試）使用。如果沒有它，這些專案就會故意不透明，因此外部 SDK 只能看到安全的建構函式和編碼的有效負載。
- 機制：`iroha_data_model_derive::model` 巨集使用 `#[cfg(feature = "transparent_api")] pub` 重寫每個公用字段，並為預設建置保留一個私人副本。啟用該功能會翻轉這些 cfgs，因此解構 `Account`、`Domain`、`Asset` 等在其定義模組之外變得合法。
- 表面檢測：套件導出 `TRANSPARENT_API: bool` 常數（生成為 `transparent_api.rs` 或 `non_transparent_api.rs`）。下游代碼可以在需要回退到不清晰助手時檢查此標誌和分支。
- 啟用：將 `features = ["transparent_api"]` 新增至 `Cargo.toml` 的相依性。需要 JSON 投影的工作區 crate（例如 `iroha_torii`）會自動轉發該標誌，但第三方使用者應將其關閉，除非他們控制部署並接受更廣泛的 API 表面。

## 簡單範例

建立網域和帳戶、定義資產並按照指示建立交易：

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

透過DSL查詢帳戶和資產：

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

使用 IVM 智慧合約位元組碼：

```rust
use iroha_data_model::prelude::*;

let bytecode = IvmBytecode::from_compiled(include_bytes!("contract.to").to_vec());
let tx = TransactionBuilder::new("dev-chain".parse().unwrap(), account_id.clone())
    .with_bytecode(bytecode)
    .sign(kp.private_key());
```

asset-definition id / 別名快速參考 (CLI + Torii)：

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
```遷移注意事項：
- v1 中不接受舊的 `name#domain` 資產定義 ID。
- 用於鑄幣/銷毀/轉移的資產 ID 仍然是規範的 `<base58-asset-id>#<katakana-i105-account-id>`；建造它們：
  - `iroha tools encode asset-id --definition <base58-asset-definition-id> --account <i105>`
  - 或 `--alias <name>#<domain>.<dataspace>` / `--alias <name>#<dataspace>` + `--account`。

## 版本控制

- `SignedTransaction`、`SignedBlock` 和 `SignedQuery` 是規範的 Norito 編碼結構。當透過 `EncodeVersioned` 編碼時，每個都實作 `iroha_version::Version`，以目前 ABI 版本（目前為 `1`）為其有效負載添加前綴。

## 評論註釋/潛在更新

- 查詢 DSL：考慮記錄一個穩定的面向使用者的子集以及常見篩選器/選擇器的範例。
- 指令係列：展開公共文檔，列出 `mint_burn`、`register`、`transfer` 公開的內建 ISI 變體。

---
如果任何部分需要更多深度（例如，完整的 ISI 目錄、完整的查詢註冊表列表或區塊頭字段），請告訴我，我將相應地擴展這些部分。