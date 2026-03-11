---
lang: zh-hant
direction: ltr
source: docs/source/data_model_and_isi_spec.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 55ac770cf80229c23d6067ef1ab312422c76fb928a08e8cad8c040bdab396016
source_last_modified: "2026-01-28T18:22:38.873410+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Iroha v2 數據模型和 ISI — 實現衍生規範

該規範是根據 `iroha_data_model` 和 `iroha_core` 的當前實現進行逆向工程的，以幫助設計審查。反引號中的路徑指向權威代碼。

## 範圍
- 定義規範實體（域、帳戶、資產、NFT、角色、權限、對等點、觸發器）及其標識符。
- 描述狀態更改指令 (ISI)：類型、參數、前提條件、狀態轉換、發出的事件和錯誤條件。
- 總結參數管理、事務和指令序列化。

確定性：所有指令語義都是純粹的狀態轉換，沒有依賴於硬件的行為。序列化使用Norito； VM 字節碼使用 IVM，並在鏈上執行之前經過主機端驗證。

---

## 實體和標識符
ID 具有穩定的字符串形式，可進行 `Display`/`FromStr` 往返。名稱規則禁止空格和保留的 `@ # $` 字符。- `Name` — 經過驗證的文本標識符。規則：`crates/iroha_data_model/src/name.rs`。
- `DomainId` — `name`。域名：`{ id, logo, metadata, owned_by }`。建造者：`NewDomain`。代碼：`crates/iroha_data_model/src/domain.rs`。
- `AccountId` — 規範地址通過 `AccountAddress`（I105 / `i105` 壓縮/十六進制）生成，Torii 通過 `AccountAddress::parse_encoded` 標準化輸入。 I105是首選賬戶格式； `i105` 形式對於僅 Sora 的 UX 來說是第二好的。熟悉的 `alias` (rejected legacy form) 字符串僅保留作為路由別名。帳號：`{ id, metadata }`。代碼：`crates/iroha_data_model/src/account.rs`。
- 帳戶准入策略 — 域通過在元數據密鑰 `iroha:account_admission_policy` 下存儲 Norito-JSON `AccountAdmissionPolicy` 來控制隱式帳戶創建。當密鑰不存在時，鏈級自定義參數`iroha:default_account_admission_policy`提供默認值；當它也不存在時，硬默認值是 `ImplicitReceive`（第一個版本）。策略標籤 `mode`（`ExplicitOnly` 或 `ImplicitReceive`）加上可選的每筆交易（默認 `16`）和每塊創建上限、可選的 `implicit_creation_fee`（銷毀或接收帳戶）、每個資產定義的 `min_initial_amounts` 和可選的`default_role_on_create`（在 `AccountCreated` 之後授予，如果缺失則以 `DefaultRoleError` 拒絕）。 Genesis 無法選擇加入；禁用/無效策略拒絕針對 `InstructionExecutionError::AccountAdmission` 的未知賬戶的收據式指令。隱式帳戶在 `AccountCreated` 之前標記元數據 `iroha:created_via="implicit"`；默認角色發出後續 `AccountRoleGranted`，執行者所有者基線規則讓新賬戶無需額外角色即可使用自己的資產/NFT。代碼：`crates/iroha_data_model/src/account/admission.rs`、`crates/iroha_core/src/smartcontracts/isi/account_admission.rs`。
- `AssetDefinitionId` — `asset#domain`。定義：`{ id, spec: NumericSpec, mintable: Mintable, logo, metadata, owned_by, total_quantity }`。代碼：`crates/iroha_data_model/src/asset/definition.rs`。
- `AssetId`: canonical encoded literal `norito:<hex>` (legacy textual forms are not supported in first release).
- `NftId` — `nft$domain`。 NFT：`{ id, content: Metadata, owned_by }`。代碼：`crates/iroha_data_model/src/nft.rs`。
- `RoleId` — `name`。角色：`{ id, permissions: BTreeSet<Permission> }` 和構建器 `NewRole { inner: Role, grant_to }`。代碼：`crates/iroha_data_model/src/role.rs`。
- `Permission` — `{ name: Ident, payload: Json }`。代碼：`crates/iroha_data_model/src/permission.rs`。
- `PeerId`/`Peer` — 對等身份（公鑰）和地址。代碼：`crates/iroha_data_model/src/peer.rs`。
- `TriggerId` — `name`。觸發器：`{ id, action }`。行動：`{ executable, repeats, authority, filter, metadata }`。代碼：`crates/iroha_data_model/src/trigger/`。
- `Metadata` — `BTreeMap<Name, Json>` 已檢查插入/刪除。代碼：`crates/iroha_data_model/src/metadata.rs`。
- 訂閱模式（應用層）：計劃是帶有 `subscription_plan` 元數據的 `AssetDefinition` 條目；訂閱是帶有 `subscription` 元數據的 `Nft` 記錄；計費由引用訂閱 NFT 的時間觸發器執行。請參閱 `docs/source/subscriptions_api.md` 和 `crates/iroha_data_model/src/subscription.rs`。
- **加密原語**（功能 `sm`）：- `Sm2PublicKey` / `Sm2Signature` 鏡像 SM2 的規範 SEC1 點 + 固定寬度 `r∥s` 編碼。構造函數強制執行曲線成員資格和區分 ID 語義 (`DEFAULT_DISTID`)，而驗證則拒絕格式錯誤或高範圍標量。代碼：`crates/iroha_crypto/src/sm.rs` 和 `crates/iroha_data_model/src/crypto/mod.rs`。
  - `Sm3Hash` 將 GM/T 0004 摘要公開為 Norito 可序列化的 `[u8; 32]` 新類型，無論哈希出現在清單或遙測中，都使用該新類型。代碼：`crates/iroha_data_model/src/crypto/hash.rs`。
  - `Sm4Key` 表示 128 位 SM4 密鑰，並在主機系統調用和數據模型裝置之間共享。代碼：`crates/iroha_data_model/src/crypto/symmetric.rs`。
  這些類型與現有的 Ed25519/BLS/ML-DSA 原語並存，一旦啟用 `sm` 功能，數據模型使用者（Torii、SDK、創世工具）就可以使用這些類型。

重要特徵：`Identifiable`、`Registered`/`Registrable`（構建器模式）、`HasMetadata`、`IntoKeyValue`。代碼：`crates/iroha_data_model/src/lib.rs`。

事件：每個實體都有在突變時發出的事件（創建/刪除/所有者更改/元數據更改等）。代碼：`crates/iroha_data_model/src/events/`。

---

## 參數（鏈配置）
- 系列：`SumeragiParameters { block_time_ms, commit_time_ms, min_finality_ms, pacing_factor_bps, max_clock_drift_ms, collectors_k, collectors_redundant_send_r }`、`BlockParameters { max_transactions }`、`TransactionParameters { max_signatures, max_instructions, ivm_bytecode_size, max_tx_bytes, max_decompressed_bytes }`、`SmartContractParameters { fuel, memory, execution_depth }` 以及 `custom: BTreeMap`。
- 差異的單個枚舉：`SumeragiParameter`、`BlockParameter`、`TransactionParameter`、`SmartContractParameter`。聚合器：`Parameters`。代碼：`crates/iroha_data_model/src/parameter/system.rs`。

設置參數（ISI）：`SetParameter(Parameter)` 更新相應字段並發出 `ConfigurationEvent::Changed`。代碼：`crates/iroha_data_model/src/isi/transparent.rs`，執行者在`crates/iroha_core/src/smartcontracts/isi/world.rs`。

---

## 指令序列化和註冊
- 核心特徵：`Instruction: Send + Sync + 'static` 與 `dyn_encode()`、`as_any()`、穩定 `id()`（默認為具體類型名稱）。
- `InstructionBox`：`Box<dyn Instruction>` 包裝器。 Clone/Eq/Ord 在 `(type_id, encoded_bytes)` 上運行，因此相等是按值計算的。
- `InstructionBox` 的 Norito Serde 序列化為 `(String wire_id, Vec<u8> payload)`（如果沒有線 ID，則回退到 `type_name`）。反序列化使用全局 `InstructionRegistry` 將標識符映射到構造函數。默認註冊表包括所有內置 ISI。代碼：`crates/iroha_data_model/src/isi/{mod.rs,registry.rs}`。

---

## ISI：類型、語義、錯誤
執行是通過 `iroha_core::smartcontracts::isi` 中的 `Execute for <Instruction>` 實現的。下面列出了公共效果、先決條件、發出的事件和錯誤。

### 註冊/取消註冊
類型：`Register<T: Registered>` 和 `Unregister<T: Identifiable>`，總和類型 `RegisterBox`/`UnregisterBox` 覆蓋具體目標。

- 註冊對等點：插入到世界對等點集中。
  - 前提條件：必須不存在。
  - 事件：`PeerEvent::Added`。
  - 錯誤：`Repetition(Register, PeerId)`（如果重複）；查找時為 `FindError`。代碼：`core/.../isi/world.rs`。

- 註冊域：從 `NewDomain` 和 `owned_by = authority` 構建。不允許：`genesis` 域。
  - 前提條件：域名不存在；不是 `genesis`。
  - 事件：`DomainEvent::Created`。
  - 錯誤：`Repetition(Register, DomainId)`、`InvariantViolation("Not allowed to register genesis domain")`。代碼：`core/.../isi/world.rs`。- 註冊帳戶：從 `NewAccount` 構建，在 `genesis` 域中不允許； `genesis` 賬戶無法註冊。
  - 前提條件：域名必須存在；賬戶不存在；不在創世域中。
  - 事件：`DomainEvent::Account(AccountEvent::Created)`。
  - 錯誤：`Repetition(Register, AccountId)`、`InvariantViolation("Not allowed to register account in genesis domain")`。代碼：`core/.../isi/domain.rs`。

- 註冊AssetDefinition：從構建器構建；設置 `owned_by = authority`。
  - 前提條件：定義不存在；域存在。
  - 事件：`DomainEvent::AssetDefinition(AssetDefinitionEvent::Created)`。
  - 錯誤：`Repetition(Register, AssetDefinitionId)`。代碼：`core/.../isi/domain.rs`。

- 註冊NFT：由構建者構建；設置 `owned_by = authority`。
  - 前提條件：NFT 不存在；域存在。
  - 事件：`DomainEvent::Nft(NftEvent::Created)`。
  - 錯誤：`Repetition(Register, NftId)`。代碼：`core/.../isi/nft.rs`。

- 註冊角色：從 `NewRole { inner, grant_to }`（通過帳戶角色映射記錄的第一個所有者）構建，存儲 `inner: Role`。
  - 前提條件：角色不存在。
  - 事件：`RoleEvent::Created`。
  - 錯誤：`Repetition(Register, RoleId)`。代碼：`core/.../isi/world.rs`。

- 註冊觸發器：將觸發器存儲在按過濾器類型設置的適當觸發器中。
  - 前提條件：如果過濾器不可鑄造，則 `action.repeats` 必須是 `Exactly(1)`（否則為 `MathError::Overflow`）。禁止重複 ID。
  - 事件：`TriggerEvent::Created(TriggerId)`。
  - 錯誤：轉換/驗證失敗時出現 `Repetition(Register, TriggerId)`、`InvalidParameterError::SmartContract(..)`。代碼：`core/.../isi/triggers/mod.rs`。

- 註銷Peer/Domain/Account/AssetDefinition/NFT/Role/Trigger：刪除目標；發出刪除事件。額外的級聯刪除：
  - 取消註冊域：刪除域中的所有帳戶及其角色、權限、發送序列計數器、帳戶標籤和 UAID 綁定；刪除其資產（以及每個資產的元數據）；刪除域中的所有資產定義；刪除域中的 NFT 以及已刪除帳戶擁有的任何 NFT；刪除權限域匹配的觸發器。事件：`DomainEvent::Deleted`，加上每個項目的刪除事件。錯誤：`FindError::Domain`（如果缺失）。代碼：`core/.../isi/world.rs`。
  - 註銷賬戶：刪除賬戶的權限、角色、交易序列計數器、賬戶標籤映射和 UAID 綁定；刪除帳戶擁有的資產（以及每個資產的元數據）；刪除該賬戶擁有的 NFT；刪除權限為該帳戶的觸發器。事件：`AccountEvent::Deleted`，加上每個刪除的 NFT 的 `NftEvent::Deleted`。錯誤：`FindError::Account`（如果缺失）。代碼：`core/.../isi/domain.rs`。
  - 取消註冊資產定義：刪除該定義的所有資產及其每個資產的元數據。事件：每個資產的 `AssetDefinitionEvent::Deleted` 和 `AssetEvent::Deleted`。錯誤：`FindError::AssetDefinition`。代碼：`core/.../isi/domain.rs`。
  - 註銷NFT：刪除NFT。事件：`NftEvent::Deleted`。錯誤：`FindError::Nft`。代碼：`core/.../isi/nft.rs`。
  - 註銷角色：先從所有賬戶中註銷該角色；然後刪除該角色。事件：`RoleEvent::Deleted`。錯誤：`FindError::Role`。代碼：`core/.../isi/world.rs`。
  - 取消註冊觸發器：刪除觸發器（如果存在）；重複註銷會產生 `Repetition(Unregister, TriggerId)`。事件：`TriggerEvent::Deleted`。代碼：`core/.../isi/triggers/mod.rs`。

### 薄荷/燃燒
類型：`Mint<O, D: Identifiable>` 和 `Burn<O, D: Identifiable>`，盒裝為 `MintBox`/`BurnBox`。- 資產（數字）鑄造/銷毀：調整餘額和定義的 `total_quantity`。
  - 前提條件：`Numeric`值必須滿足`AssetDefinition.spec()`； `mintable` 允許的薄荷：
    - `Infinitely`：始終允許。
    - `Once`：僅允許一次；第一個鑄幣廠將 `mintable` 翻轉為 `Not` 並發出 `AssetDefinitionEvent::MintabilityChanged`，以及用於可審計的詳細 `AssetDefinitionEvent::MintabilityChangedDetailed { asset_definition, minted_amount, authority }`。
    - `Limited(n)`：允許 `n` 額外的鑄造操作。每個成功的鑄幣廠都會減少計數器；當它達到零時，定義翻轉到 `Not` 並發出與上面相同的 `MintabilityChanged` 事件。
    - `Not`：錯誤 `MintabilityError::MintUnmintable`。
  - 狀態變化：如果鑄幣廠丟失則創建資產；如果銷毀時餘額變為零，則刪除資產條目。
  - 事件：`AssetEvent::Added`/`AssetEvent::Removed`、`AssetDefinitionEvent::MintabilityChanged`（當 `Once` 或 `Limited(n)` 耗盡其配額時）。
  - 錯誤：`TypeError::AssetNumericSpec(Mismatch)`、`MathError::Overflow`/`NotEnoughQuantity`。代碼：`core/.../isi/asset.rs`。

- 觸發重複薄荷/燃燒：更改觸發的 `action.repeats` 計數。
  - 前提條件：在薄荷上，過濾器必須是可薄荷的；算術不能溢出/下溢。
  - 事件：`TriggerEvent::Extended`/`TriggerEvent::Shortened`。
  - 錯誤：`MathError::Overflow` 無效鑄幣； `FindError::Trigger`（如果丟失）。代碼：`core/.../isi/triggers/mod.rs`。

### 轉移
類型：`Transfer<S: Identifiable, O, D: Identifiable>`，盒裝為 `TransferBox`。

- 資產（數字）：從源 `AssetId` 中減去，添加到目標 `AssetId`（相同定義，不同帳戶）。刪除歸零的源資產。
  - 前提條件：源資產存在；值滿足 `spec`。
  - 事件：`AssetEvent::Removed`（源）、`AssetEvent::Added`（目標）。
  - 錯誤：`FindError::Asset`、`TypeError::AssetNumericSpec`、`MathError::NotEnoughQuantity/Overflow`。代碼：`core/.../isi/asset.rs`。

- 域所有權：將 `Domain.owned_by` 更改為目標帳戶。
  - 前提條件：兩個賬戶都存在；域存在。
  - 事件：`DomainEvent::OwnerChanged`。
  - 錯誤：`FindError::Account/Domain`。代碼：`core/.../isi/domain.rs`。

- AssetDefinition 所有權：將 `AssetDefinition.owned_by` 更改為目標賬戶。
  - 前提條件：兩個賬戶都存在；定義存在；源當前必須擁有它。
  - 事件：`AssetDefinitionEvent::OwnerChanged`。
  - 錯誤：`FindError::Account/AssetDefinition`。代碼：`core/.../isi/account.rs`。

- NFT 所有權：將 `Nft.owned_by` 更改為目標賬戶。
  - 前提條件：兩個賬戶都存在； NFT 存在；源當前必須擁有它。
  - 事件：`NftEvent::OwnerChanged`。
  - 錯誤：`FindError::Account/Nft`、`InvariantViolation`（如果來源不擁有 NFT）。代碼：`core/.../isi/nft.rs`。

### 元數據：設置/刪除鍵值
類型：`SetKeyValue<T>` 和 `RemoveKeyValue<T>` 與 `T ∈ { Domain, Account, AssetDefinition, Nft, Trigger }`。提供盒裝枚舉。

- 設置：插入或替換 `Metadata[key] = Json(value)`。
- 移除：移除鑰匙；如果丟失則出錯。
- 事件：`<Target>Event::MetadataInserted` / `MetadataRemoved` 以及舊/新值。
- 錯誤：如果目標不存在，則為 `FindError::<Target>`； `FindError::MetadataKey` 缺少用於移除的鑰匙。代碼：`crates/iroha_data_model/src/isi/transparent.rs` 和每個目標的執行器實現。### 權限和角色：授予/撤銷
類型：`Grant<O, D>` 和 `Revoke<O, D>`，帶有 `Permission`/`Role` 與 `Account` 之間的盒裝枚舉，以及 `Permission` 與 `Role` 之間的盒裝枚舉。

- 授予帳戶權限：添加 `Permission`，除非已經固有。事件：`AccountEvent::PermissionAdded`。錯誤：`Repetition(Grant, Permission)`（如果重複）。代碼：`core/.../isi/account.rs`。
- 撤銷帳戶的權限：如果存在則刪除。事件：`AccountEvent::PermissionRemoved`。錯誤：`FindError::Permission`（如果不存在）。代碼：`core/.../isi/account.rs`。
- 將角色授予帳戶：插入 `(account, role)` 映射（如果不存在）。事件：`AccountEvent::RoleGranted`。錯誤：`Repetition(Grant, RoleId)`。代碼：`core/.../isi/account.rs`。
- 從帳戶中撤銷角色：刪除映射（如果存在）。事件：`AccountEvent::RoleRevoked`。錯誤：`FindError::Role`（如果不存在）。代碼：`core/.../isi/account.rs`。
- 授予角色權限：重建角色並添加權限。事件：`RoleEvent::PermissionAdded`。錯誤：`Repetition(Grant, Permission)`。代碼：`core/.../isi/world.rs`。
- 撤銷角色的權限：在沒有該權限的情況下重建角色。事件：`RoleEvent::PermissionRemoved`。錯誤：`FindError::Permission`（如果不存在）。代碼：`core/.../isi/world.rs`。

### 觸發器：執行
類型：`ExecuteTrigger { trigger: TriggerId, args: Json }`。
- 行為：將觸發子系統的 `ExecuteTriggerEvent { trigger_id, authority, args }` 排入隊列。僅允許調用觸發（`ExecuteTrigger` 過濾器）手動執行；過濾器必須匹配，並且調用者必須是觸發操作權限或持有該權限的 `CanExecuteTrigger`。當用戶提供的執行器處於活動狀態時，觸發器執行由運行時執行器驗證，並消耗事務的執行器燃料預算（基本 `executor.fuel` 加上可選元數據 `additional_fuel`）。
- 錯誤：如果未註冊，則為 `FindError::Trigger`； `InvariantViolation`（如果由非權威機構調用）。代碼：`core/.../isi/triggers/mod.rs`（並在 `core/.../smartcontracts/isi/mod.rs` 中進行測試）。

### 升級並登錄
- `Upgrade { executor }`：使用提供的 `Executor` 字節碼遷移執行器，更新執行器及其數據模型，發出 `ExecutorEvent::Upgraded`。錯誤：遷移失敗時包裝為 `InvalidParameterError::SmartContract`。代碼：`core/.../isi/world.rs`。
- `Log { level, msg }`：發出給定級別的節點日誌；沒有狀態改變。代碼：`core/.../isi/world.rs`。

### 錯誤模型
通用信封：`InstructionExecutionError`，具有評估錯誤、查詢失敗、轉換、未找到實體、重複、可鑄造性、數學、無效參數和不變違規等變體。枚舉和幫助程序位於 `crates/iroha_data_model/src/isi/mod.rs` 下的 `pub mod error` 中。

---## 事務和可執行文件
- `Executable`：`Instructions(ConstVec<InstructionBox>)` 或 `Ivm(IvmBytecode)`；字節碼序列化為 base64。代碼：`crates/iroha_data_model/src/transaction/executable.rs`。
- `TransactionBuilder`/`SignedTransaction`：使用元數據、`chain_id`、`authority`、`creation_time_ms`、可選的 `ttl_ms` 和 `nonce` 構造、簽署和打包可執行文件。代碼：`crates/iroha_data_model/src/transaction/`。
- 在運行時，`iroha_core` 通過 `Execute for InstructionBox` 執行 `InstructionBox` 批次，向下轉換為適當的 `*Box` 或具體指令。代碼：`crates/iroha_core/src/smartcontracts/isi/mod.rs`。
- 運行時執行器驗證預算（用戶提供的執行器）：來自參數的基礎 `executor.fuel` 加上可選的事務元數據 `additional_fuel` (`u64`)，在事務內的指令/觸發器驗證之間共享。

---

## 不變量和註釋（來自測試和防護）
- 創世保護：無法註冊 `genesis` 域或 `genesis` 域中的帳戶； `genesis` 賬戶無法註冊。代碼/測試：`core/.../isi/world.rs`、`core/.../smartcontracts/isi/mod.rs`。
- 數字資產在鑄造/轉移/銷毀時必須滿足 `NumericSpec` 要求；規格不匹配產生 `TypeError::AssetNumericSpec`。
- 可鑄造性：`Once` 允許單次鑄造，然後翻轉至 `Not`；在翻轉到 `Not` 之前，`Limited(n)` 正好允許 `n` 鑄幣。嘗試禁止在 `Infinitely` 上鑄造會導致 `MintabilityError::ForbidMintOnMintable`，配置 `Limited(0)` 會產生 `MintabilityError::InvalidMintabilityTokens`。
- 元數據操作是關鍵精確的；刪除不存在的密鑰是一個錯誤。
- 觸發過濾器可以是不可鑄造的；那麼 `Register<Trigger>` 只允許 `Exactly(1)` 重複。
- 觸發元數據鍵 `__enabled` (bool) 門執行；缺少默認啟用的觸發器，並且禁用的觸發器會在數據/時間/按調用路徑上跳過。
- 確定性：所有算術都使用檢查操作；下溢/溢出返回鍵入的數學錯誤；零餘額會刪除資產條目（無隱藏狀態）。

---## 實際例子
- 鑄造和轉讓：
  - `Mint::asset_numeric(10, asset_id)` → 如果規格/可鑄造性允許，則增加 10；事件：`AssetEvent::Added`。
  - `Transfer::asset_numeric(asset_id, 5, to_account)` → 移動 5；刪除/添加事件。
- 元數據更新：
  - `SetKeyValue::account(account_id, "avatar".parse()?, json)` → 更新插入；通過 `RemoveKeyValue::account(...)` 刪除。
- 角色/權限管理：
  - `Grant::account_role(role_id, account)`、`Grant::role_permission(perm, role)` 及其 `Revoke` 對應項。
- 觸發器生命週期：
  - `Register::trigger(Trigger::new(id, Action::new(exec, repeats, authority, filter)))`，帶有過濾器暗示的可鑄造性檢查； `ExecuteTrigger::new(id).with_args(&args)` 必須與配置的權限匹配。
  - 可以通過將元數據鍵 `__enabled` 設置為 `false` 來禁用觸發器（缺少默認啟用）；通過 `SetKeyValue::trigger` 或 IVM `set_trigger_enabled` 系統調用進行切換。
  - 加載時修復觸發器存儲：刪除重複的 id、不匹配的 id 以及引用丟失字節碼的觸發器；重新計算字節碼引用計數。
  - 如果觸發器的 IVM 字節碼在執行時丟失，則觸發器將被刪除，並且執行將被視為具有失敗結果的無操作。
  - 耗盡的觸發器立即被移除；如果在執行過程中遇到耗盡的條目，則會將其修剪並視為丟失。
- 參數更新：
  - `SetParameter(SumeragiParameter::BlockTimeMs(2500).into())` 更新並發出 `ConfigurationEvent::Changed`。

---

## 可追溯性（選定來源）
 - 數據模型核心：`crates/iroha_data_model/src/{account.rs,domain.rs,asset/**,nft.rs,role.rs,permission.rs,metadata.rs,trigger/**,parameter/**}`。
 - ISI 定義和註冊表：`crates/iroha_data_model/src/isi/{mod.rs,register.rs,transfer.rs,mint_burn.rs,transparent.rs,registry.rs}`。
 - ISI 執行：`crates/iroha_core/src/smartcontracts/isi/{mod.rs,world.rs,domain.rs,account.rs,asset.rs,nft.rs,triggers/**}`。
 - 事件：`crates/iroha_data_model/src/events/**`。
 - 交易：`crates/iroha_data_model/src/transaction/**`。

如果您希望將此規範擴展為呈現的 API/行為表或交叉鏈接到每個具體事件/錯誤，請說出這個詞，我將擴展它。