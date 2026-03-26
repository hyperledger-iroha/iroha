---
lang: zh-hant
direction: ltr
source: docs/source/data_model_and_isi_spec.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2077d985b10b26b29b821646b435cc8850cbc6c842d372de6c9c4523ee95a5b7
source_last_modified: "2026-03-12T11:24:34.970622+00:00"
translation_last_reviewed: 2026-03-12
translator: machine-google-reviewed
---

# Iroha v2 資料模型與 ISI — 實作衍生規範

該規範是根據 `iroha_data_model` 和 `iroha_core` 的當前實現進行逆向工程的，以幫助設計審查。反引號中的路徑指向權威代碼。

## 範圍
- 定義規範實體（網域、帳戶、資產、NFT、角色、權限、對等點、觸發器）及其識別碼。
- 描述狀態變更指令 (ISI)：類型、參數、前提條件、狀態轉換、發出的事件和錯誤條件。
- 總結參數管理、事務和指令序列化。

確定性：所有指令語意都是純粹的狀態轉換，沒有依賴硬體的行為。序列化使用 Norito；VM 字節碼使用 IVM，並在鏈上執行之前經過主機端驗證。

---

## 實體和識別符
ID 具有穩定的字串形式，可進行 `Display`/`FromStr` 往返。名稱規則禁止空格和保留的 `@ # $` 字元。- `Name` — 經過驗證的文字識別碼。規則：`crates/iroha_data_model/src/name.rs`。
- `DomainId` — `name`。網域：`{ id, logo, metadata, owned_by }`。建造者：`NewDomain`。代碼：`crates/iroha_data_model/src/domain.rs`。
- `AccountId` — 規範位址透過 `AccountAddress`（I105 / 十六進位）生成，Torii 透過 `AccountAddress::parse_encoded` 標準化輸入。 I105是首選帳戶格式； I105 表格僅適用於 Sora UX。熟悉的 `alias`（已拒絕的舊格式）字串僅保留作為路由別名。帳號：`{ id, metadata }`。代碼：`crates/iroha_data_model/src/account.rs`。- 帳戶存取政策 — 網域透過在元資料金鑰 `iroha:account_admission_policy` 下儲存 Norito-JSON `AccountAdmissionPolicy` 來控制隱式帳戶建立。當金鑰不存在時，鏈級自訂參數`iroha:default_account_admission_policy`提供預設值；當它也不存在時，硬預設值是 `ImplicitReceive`（第一個版本）。策略標籤 `mode`（`ExplicitOnly` 或 `ImplicitReceive`）加上可選的每筆交易（預設 `16`）和每塊建立上限、可選的 `implicit_creation_fee`（銷毀或收取和可選的`default_role_on_create`（在 `AccountCreated` 之後授予，如果缺失，則以 `DefaultRoleError` 拒絕）。 Genesis 無法選擇加入；停用/無效原則拒絕針對 `InstructionExecutionError::AccountAdmission` 的未知帳戶的收據式指示。隱式帳戶在 `AccountCreated` 之前標記元資料 `iroha:created_via="implicit"`；預設角色發出後續 `AccountRoleGranted`，執行者所有者基準規則讓新帳戶無需額外角色即可使用自己的資產/NFT。代碼：`crates/iroha_data_model/src/account/admission.rs`、`crates/iroha_core/src/smartcontracts/isi/account_admission.rs`。
- `AssetDefinitionId` — 規格 `unprefixed Base58 address with versioning and checksum`（UUID-v4 位元組）。定義：`{ id, name, description?, alias?, spec: NumericSpec, mintable: Mintable, logo, metadata, owned_by, total_quantity }`。 `alias` 文字必須是 `<name>#<domain>.<dataspace>` 或 `<name>#<dataspace>`，其中 `<name>` 等於資產定義名稱。代碼：`crates/iroha_data_model/src/asset/definition.rs`。

  - Torii asset-definition responses may include `alias_binding { alias, status, lease_expiry_ms, grace_until_ms, bound_at_ms }`, where `status` is `permanent`, `leased_active`, `leased_grace`, or `expired_pending_cleanup`. Alias selectors resolve against the latest committed block creation time and stop resolving after grace even before sweep removes stale bindings.
- `AssetId`：規範編碼文字 `<asset-definition-id>#<i105-account-id>`（第一個版本不支援舊文字形式）。- `NftId` — `nft$domain`。 NFT：`{ id, content: Metadata, owned_by }`。代碼：`crates/iroha_data_model/src/nft.rs`。
- `RoleId` — `name`。角色：`{ id, permissions: BTreeSet<Permission> }` 和建構器 `NewRole { inner: Role, grant_to }`。代碼：`crates/iroha_data_model/src/role.rs`。
- `Permission` — `{ name: Ident, payload: Json }`。代碼：`crates/iroha_data_model/src/permission.rs`。
- `PeerId`/`Peer` — 對等身分（公鑰）和位址。代碼：`crates/iroha_data_model/src/peer.rs`。
- `TriggerId` — `name`。觸發器：`{ id, action }`。行動：`{ executable, repeats, authority, filter, metadata }`。代碼：`crates/iroha_data_model/src/trigger/`。
- `Metadata` — `BTreeMap<Name, Json>` 已檢查插入/刪除。代碼：`crates/iroha_data_model/src/metadata.rs`。
- 訂閱模式（應用層）：方案是 `subscription_plan` 元資料的 `AssetDefinition` 條目；訂閱是 `subscription` 元資料的 `Nft` 記錄；計費由引用 NFT 的時間觸發訂閱器執行。請參閱 `docs/source/subscriptions_api.md` 和 `crates/iroha_data_model/src/subscription.rs`。
- **加密原語**（功能 `sm`）：
  - `Sm2PublicKey` / `Sm2Signature` 鏡像 SM2 的規格 SEC1 點 + 固定寬度 `r∥s` 編碼。建構函數強制執行曲線成員資格和區分 ID 語意 (`DEFAULT_DISTID`)，而驗證則拒絕格式錯誤或高範圍標量。代碼：`crates/iroha_crypto/src/sm.rs` 和 `crates/iroha_data_model/src/crypto/mod.rs`。
  - `Sm3Hash` 將 GM/T 0004 摘要公開為 Norito 可序列化的 `[u8; 32]` 新類型，只要散列出現在清單或遙測中，就會使用該新類型。代碼：`crates/iroha_data_model/src/crypto/hash.rs`。- `Sm4Key` 表示 128 位元 SM4 金鑰，並在主機系統呼叫和資料模型裝置之間共用。代碼：`crates/iroha_data_model/src/crypto/symmetric.rs`。
  這些類型與現有的 Ed25519/BLS/ML-DSA 原語並存，一旦啟用 `sm` 功能，資料模型使用者（Torii、SDK、創世工具）就可以使用這些類型。
- 資料空間派生的關係儲存（`space_directory_manifests`、`uaid_dataspaces`、`axt_policies`、`axt_replay_ledger`、通道中繼緊急覆蓋註冊表）和資料空間目標`dataspace_catalog` 中消失時，`State::set_nexus(...)`，防止執行時間目錄更新後出現過時的資料空間參考。當通道退役或重新分配到不同的資料空間時，通道範圍的 DA/中繼快取（`lane_relays`、`da_commitments`、`da_confidential_compute`、`da_pin_intents`）也會被修剪，因此通道被洩漏在資料空間中空間目錄 ISI（`PublishSpaceDirectoryManifest`、`RevokeSpaceDirectoryManifest`、`ExpireSpaceDirectoryManifest`）也會根據活動目錄驗證 `dataspace`，並使用 `InvalidParameter` 拒絕未知 ID。

重要特徵：`Identifiable`、`Registered`/`Registrable`（建構器模式）、`HasMetadata`、`IntoKeyValue`。代碼：`crates/iroha_data_model/src/lib.rs`。

事件：每個實體都有在突變時發出的事件（建立/刪除/擁有者變更/元資料變更等）。代碼：`crates/iroha_data_model/src/events/`。

---## 參數（鏈配置）
- 系列：`SumeragiParameters { block_time_ms, commit_time_ms, min_finality_ms, pacing_factor_bps, max_clock_drift_ms, collectors_k, collectors_redundant_send_r }`、`BlockParameters { max_transactions }`、`TransactionParameters { max_signatures, max_instructions, ivm_bytecode_size, max_tx_bytes, max_decompressed_bytes }`、`SmartContractParameters { fuel, memory, execution_depth }` 以及 `custom: BTreeMap`。
- 差異的單一枚舉：`SumeragiParameter`、`BlockParameter`、`TransactionParameter`、`SmartContractParameter`。聚合器：`Parameters`。代碼：`crates/iroha_data_model/src/parameter/system.rs`。

設定參數（ISI）：`SetParameter(Parameter)` 更新對應欄位並發出 `ConfigurationEvent::Changed`。代碼：`crates/iroha_data_model/src/isi/transparent.rs`，執行者在`crates/iroha_core/src/smartcontracts/isi/world.rs`。

---

## 指令序列化和註冊
- 核心特徵：`Instruction: Send + Sync + 'static` 與 `dyn_encode()`、`as_any()`、穩定 `id()`（預設為特定類型名稱）。
- `InstructionBox`：`Box<dyn Instruction>` 包裝器。 Clone/Eq/Ord 在 `(type_id, encoded_bytes)` 上運行，因此相等是按值計算的。
- `InstructionBox` 的 Norito Serde 序列化為 `(String wire_id, Vec<u8> payload)`（如果沒有線 ID，則回退到 `type_name`）。反序列化使用全域 `InstructionRegistry` 將標識符對應到建構子。預設註冊表包括所有內建 ISI。代碼：`crates/iroha_data_model/src/isi/{mod.rs,registry.rs}`。

---

## ISI：類型、語意、錯誤
執行是透過 `iroha_core::smartcontracts::isi` 中的 `Execute for <Instruction>` 實現的。下面列出了公共效果、先決條件、發出的事件和錯誤。

### 註冊/取消註冊
類型：`Register<T: Registered>` 和 `Unregister<T: Identifiable>`，總和型 `RegisterBox`/`UnregisterBox` 涵蓋具體目標。- 註冊對等點：插入到世界對等點集中。
  - 前提條件：必須不存在。
  - 事件：`PeerEvent::Added`。
  - 錯誤：`Repetition(Register, PeerId)`（如果重複）；查找時為 `FindError`。代碼：`core/.../isi/world.rs`。

- 註冊域：從 `NewDomain` 和 `owned_by = authority` 建置。不允許：`genesis` 域。
  - 前提條件：網域不存在；不是 `genesis`。
  - 事件：`DomainEvent::Created`。
  - 錯誤：`Repetition(Register, DomainId)`、`InvariantViolation("Not allowed to register genesis domain")`。代碼：`core/.../isi/world.rs`。

- 註冊帳戶：從 `NewAccount` 構建，在 `genesis` 網域中不允許；`genesis` 帳戶無法註冊。
  - 前提條件：網域名稱必須存在；帳號不存在；不在創世域。
  - 事件：`DomainEvent::Account(AccountEvent::Created)`。
  - 錯誤：`Repetition(Register, AccountId)`、`InvariantViolation("Not allowed to register account in genesis domain")`。代碼：`core/.../isi/domain.rs`。

- 註冊AssetDefinition：從建構器建置；設定 `owned_by = authority`。
  - 前提條件：定義不存在；域存在； `name` 為必填項，修剪後必須為非空，且不得包含 `#`/`@`。
  - 事件：`DomainEvent::AssetDefinition(AssetDefinitionEvent::Created)`。
  - 錯誤：`Repetition(Register, AssetDefinitionId)`。代碼：`core/.../isi/domain.rs`。

- 註冊NFT：由建構者建構；設定 `owned_by = authority`。
  - 前提條件：NFT 不存在；域存在。
  - 事件：`DomainEvent::Nft(NftEvent::Created)`。
  - 錯誤：`Repetition(Register, NftId)`。代碼：`core/.../isi/nft.rs`。- 註冊角色：從 `NewRole { inner, grant_to }`（透過帳戶角色映射記錄的第一個所有者）構建，儲存 `inner: Role`。
  - 前提條件：角色不存在。
  - 事件：`RoleEvent::Created`。
  - 錯誤：`Repetition(Register, RoleId)`。代碼：`core/.../isi/world.rs`。

- 註冊觸發器：將觸發器儲存在按過濾器類型設定的適當觸發器中。
  - 前提條件：如果過濾器不可鑄造，則 `action.repeats` 必須是 `Exactly(1)`（否則為 `MathError::Overflow`）。禁止重复 ID。
  - 事件：`TriggerEvent::Created(TriggerId)`。
  - 錯誤：轉換/驗證失敗時出現 `Repetition(Register, TriggerId)`、`InvalidParameterError::SmartContract(..)`。代碼：`core/.../isi/triggers/mod.rs`。- 登出Peer/Domain/Account/AssetDefinition/NFT/Role/Trigger：刪除目標；發出刪除事件。額外的級聯刪除：- 取消註冊網域：刪除網域實體及其選擇器/認可策略狀態；刪除網域中的資產定義（以及由這些定義鍵入的機密 `zk_assets` 側面狀態）、這些定義的資產（以及每個資產元資料）、網域中的 NFT 以及網域範圍內的帳戶標籤/別名投影。它還會取消倖存帳戶與已刪除網域的鏈接，並修剪引用已刪除網域或隨其刪除的資源的帳戶/角色範圍權限條目（網域權限、已刪除定義的資產定義/資產權限以及已刪除 NFT ID 的 NFT 權限）。網域刪除不會刪除全域 `AccountId`、其 tx 序列/UAID 狀態、外部資產或 NFT 所有權、觸發權限或指向倖存帳戶的其他外部稽核/組態參考。護欄：當回購協議、結算分類帳、公共通道獎勵/索賠、離線津貼/轉帳、結算回購預設（`settlement.repo.eligible_collateral`、`settlement.repo.collateral_substitution_matrix`）、治理配置的投票/公民身份/議會資格/病毒獎勵資產定義、配置的預言機經濟資產仍然定義資產域中的任何資產/債券Nexus 費用/質押資產定義參考（`nexus.fees.fee_asset_id`、`nexus.staking.stake_asset_id`）。事件：`DomainEvent::Deleted`，加上每個項目的刪除關於已刪除的網域範圍資源的事件。錯誤：`FindError::Domain`（如果缺失）； `InvariantViolation` 保留資產定義引用衝突。代碼：`core/.../isi/world.rs`。- 登出帳戶：刪除帳戶的權限、角色、交易序列計數器、帳戶標籤映射和 UAID 綁定；刪除帳戶擁有的資產（以及每個資產的元資料）；刪除該帳戶擁有的 NFT；刪除權限為該帳戶的觸發器；修剪引用已刪除帳戶的帳戶/角色範圍觸發項目、已刪除擁有的 NFT ID 的帳戶/角色範圍 NFT 的帳戶/角色範圍。防護欄：如果帳戶仍然擁有網域、資產定義、SoraFS 提供者綁定、活躍公民記錄、公共通道質押/獎勵狀態（包括帳戶顯示為索賠人或獎勵資產所有者的獎勵領取密鑰）、活躍預言機狀態（包括預言機提要歷史記錄提供商條目、Twitter 綁定提供者記錄或預言機經濟學配置的獎勵/斜線帳戶費用/質押帳戶引用（`nexus.fees.fee_sink_account_id`、`nexus.staking.stake_escrow_account_id`、`nexus.staking.slash_sink_account_id`；解析為規範的無域帳戶標識符，並在無效文字時拒絕失敗關閉）、活動回購協議狀態、活動結算法轉帳本狀態、活動離線津貼或離線託管活動(`settlement.offline.escrow_accounts`)、活動治理狀態（提案/階段批准als/locks/slashes/council/parliament 名冊、提案議會快照、運行時升級提案者記錄、治理配置的託管/slash-receiver/viral-pool 帳戶引用、透過 `gov.sorafs_telemetry.submitters` / `gov.sorafs_telemetry.per_provider_submitters` 的治理 SoraFS，或治理配置SoraFS 提供者擁有者引用（透過 `gov.sorafs_provider_owners`）、配置的內容發布允許清單帳戶引用 (`content.publish_allow_accounts`)、活動中繼寄件者狀態、活動內容包建立者狀態、活動 DA pin 10070700700700043 緊急通道狀態、活動中 DA 1000X00070003700X3000470700363666666666F870000 意圖註冊表發行者/綁定者記錄（pin 清單、清單別名、複製順序）。事件：`AccountEvent::Deleted`，加上每個刪除的 NFT 的 `NftEvent::Deleted`。錯誤：`FindError::Account`（如果缺失）； `InvariantViolation` 所有權孤兒。代碼：`core/.../isi/domain.rs`。- 取消註冊 AssetDefinition：刪除該定義的所有資產及其每個資產的元數據，並刪除由該定義鍵入的機密 `zk_assets` 側面狀態；也會修剪匹配的 `settlement.offline.escrow_accounts` 條目和引用已刪除資產定義或其資產實例的帳戶/角色範圍條目條目。護欄：當回購協議、結算帳本、公共通道獎勵/索賠、離線津貼/轉移狀態、結算回購預設值（`settlement.repo.eligible_collateral`、`settlement.repo.collateral_substitution_matrix`）、治理配置的投票/公民身份/議會獎勵/病毒獎勵資產定義、資產配置的預言機經濟仍引用時拒絕費用/質押資產定義參考（`nexus.fees.fee_asset_id`、`nexus.staking.stake_asset_id`）。事件：每個資產 `AssetDefinitionEvent::Deleted` 和 `AssetEvent::Deleted`。錯誤：引用衝突時出現 `FindError::AssetDefinition`、`InvariantViolation`。代碼：`core/.../isi/domain.rs`。
  - 取消註冊 NFT：刪除 NFT 並刪除引用已刪除 NFT 的帳戶/角色範圍權限條目。事件：`NftEvent::Deleted`。錯誤：`FindError::Nft`。代碼：`core/.../isi/nft.rs`。
  - 登出角色：先從所有帳戶中登出該角色；然後刪除該角色。事件：`RoleEvent::Deleted`。錯誤：`FindError::Role`。代碼：`core/.../isi/world.rs`。- 取消註冊觸發器：刪除觸發器（如果存在）並刪除引用已刪除觸發器的帳戶/角色範圍權限條目；重複登出會產生 `Repetition(Unregister, TriggerId)`。事件：`TriggerEvent::Deleted`。代碼：`core/.../isi/triggers/mod.rs`。

### 薄荷/燃燒
型號：`Mint<O, D: Identifiable>` 和 `Burn<O, D: Identifiable>`，盒裝為 `MintBox`/`BurnBox`。

- 資產（數位）鑄造/銷毀：調整餘額和定義的 `total_quantity`。
  - 前提條件：`Numeric`值必須符合`AssetDefinition.spec()`；`mintable` 允許的薄荷：
    - `Infinitely`：始終允許。
    - `Once`：僅允許一次；第一個鑄幣廠將 `mintable` 翻轉為 `Not` 並發出 `AssetDefinitionEvent::MintabilityChanged`，以及用於可審計的詳細 `AssetDefinitionEvent::MintabilityChangedDetailed { asset_definition, minted_amount, authority }`。
    - `Limited(n)`：允許 `n` 額外的鑄造作業。每個成功的鑄幣廠都會減少計數器；當它達到零時，定義翻轉到 `Not` 並發出與上面相同的 `MintabilityChanged` 事件。
    - `Not`：錯誤 `MintabilityError::MintUnmintable`。
  - 狀態變更：如果鑄幣廠遺失則建立資產；如果銷毀時餘額變為零，則刪除資產條目。
  - 事件：`AssetEvent::Added`/`AssetEvent::Removed`、`AssetDefinitionEvent::MintabilityChanged`（當 `Once` 或 `Limited(n)` 耗盡其配額時）。
  - 錯誤：`TypeError::AssetNumericSpec(Mismatch)`、`MathError::Overflow`/`NotEnoughQuantity`。代碼：`core/.../isi/asset.rs`。- 觸發重複薄荷/燃燒：更改觸發的 `action.repeats` 計數。
  - 前提條件：在薄荷上，濾網必須是可薄荷的；算術不能溢出/下溢。
  - 事件：`TriggerEvent::Extended`/`TriggerEvent::Shortened`。
  - 錯誤：`MathError::Overflow` 無效鑄幣； `FindError::Trigger`（如果遺失）。代碼：`core/.../isi/triggers/mod.rs`。

### 轉移
型號：`Transfer<S: Identifiable, O, D: Identifiable>`，盒裝為 `TransferBox`。

- 資產（數字）：從來源 `AssetId` 中減去，新增至目標 `AssetId`（相同定義，不同帳戶）。刪除歸零的來源資產。
  - 前提條件：來源資產存在；值滿足 `spec`。
  - 事件：`AssetEvent::Removed`（來源）、`AssetEvent::Added`（目標）。
  - 錯誤：`FindError::Asset`、`TypeError::AssetNumericSpec`、`MathError::NotEnoughQuantity/Overflow`。代碼：`core/.../isi/asset.rs`。

- 網域所有權：將 `Domain.owned_by` 變更為目標帳戶。
  - 前提條件：兩個帳戶都存在；網域存在。
  - 事件：`DomainEvent::OwnerChanged`。
  - 錯誤：`FindError::Account/Domain`。代碼：`core/.../isi/domain.rs`。

- AssetDefinition 擁有權：將 `AssetDefinition.owned_by` 變更為目標帳戶。
  - 前提條件：兩個帳戶都存在；定義存在；來源目前必須擁有它；權限必須是來源帳戶、來源網域所有者或資產定義網域所有者。
  - 事件：`AssetDefinitionEvent::OwnerChanged`。
  - 錯誤：`FindError::Account/AssetDefinition`。代碼：`core/.../isi/account.rs`。- NFT 所有權：將 `Nft.owned_by` 變更為目標帳戶。
  - 前提條件：兩個帳戶都存在； NFT 存在；來源目前必須擁有它；權限必須是來源帳戶、來源網域擁有者、NFT 網域擁有者或持有該 NFT 的 `CanTransferNft`。
  - 事件：`NftEvent::OwnerChanged`。
  - 錯誤：`FindError::Account/Nft`、`InvariantViolation`（如果來源不擁有 NFT）。代碼：`core/.../isi/nft.rs`。

### 元資料：設定/刪除鍵值
型號：`SetKeyValue<T>` 和 `RemoveKeyValue<T>` 以及 `T ∈ { Domain, Account, AssetDefinition, Nft, Trigger }`。提供盒裝枚舉。

- 設定：插入或取代 `Metadata[key] = Json(value)`。
- 移除：移除鑰匙；如果遺失則出錯。
- 事件：`<Target>Event::MetadataInserted` / `MetadataRemoved` 以及舊/新值。
- 錯誤：如果目標不存在，則為 `FindError::<Target>`； `FindError::MetadataKey` 缺少用於移除的鑰匙。代碼：`crates/iroha_data_model/src/isi/transparent.rs` 和每個目標的執行器實作。

### 權限和角色：授予/撤銷
型號：`Grant<O, D>` 和 `Revoke<O, D>`，以及 `Permission`/`Role` 與 `Account` 之間的盒裝000320X 與 `Account` 之間的盒裝0013218NI20003035010303000032 的舉盒。- 授予帳戶權限：新增 `Permission`，除非已經固有。事件：`AccountEvent::PermissionAdded`。錯誤：`Repetition(Grant, Permission)`（如果重複）。代碼：`core/.../isi/account.rs`。
- 撤銷帳號的權限：如果存在則刪除。事件：`AccountEvent::PermissionRemoved`。錯誤：`FindError::Permission`（如果不存在）。代碼：`core/.../isi/account.rs`。
- 將角色授予帳戶：插入 `(account, role)` 映射（如果不存在）。事件：`AccountEvent::RoleGranted`。錯誤：`Repetition(Grant, RoleId)`。代碼：`core/.../isi/account.rs`。
- 從帳戶中撤銷角色：刪除對應（如果存在）。事件：`AccountEvent::RoleRevoked`。錯誤：`FindError::Role`（如果不存在）。代碼：`core/.../isi/account.rs`。
- 授予角色權限：重建角色並新增權限。事件：`RoleEvent::PermissionAdded`。錯誤：`Repetition(Grant, Permission)`。代碼：`core/.../isi/world.rs`。
- 撤銷角色的權限：在沒有該權限的情況下重建角色。事件：`RoleEvent::PermissionRemoved`。錯誤：`FindError::Permission`（如果不存在）。代碼：`core/.../isi/world.rs`。### 觸發器：執行
類型：`ExecuteTrigger { trigger: TriggerId, args: Json }`。
- 行為：為觸發子系統排隊 `ExecuteTriggerEvent { trigger_id, authority, args }`。僅允許呼叫觸發手動執行（`ExecuteTrigger` 過濾器）；過濾器必須匹配，且呼叫者必須是觸發操作權限或持有該權限的 `CanExecuteTrigger`。當使用者提供的執行器處於活動狀態時，觸發器執行由執行時間執行器驗證，並消耗交易的執行器燃料預算（基本 `executor.fuel` 加上可選元資料 `additional_fuel`）。
- 錯誤：如果未註冊，則為 `FindError::Trigger`； `InvariantViolation`（如果由非權威機構調用）。代碼：`core/.../isi/triggers/mod.rs`（並在 `core/.../smartcontracts/isi/mod.rs` 中進行測試）。

### 升級並登入
- `Upgrade { executor }`：使用提供的 `Executor` 字節碼遷移執行器，更新執行器及其資料模型，發出 `ExecutorEvent::Upgraded`。錯誤：遷移失敗時包裝為 `InvalidParameterError::SmartContract`。代碼：`core/.../isi/world.rs`。
- `Log { level, msg }`：發出給定等級的節點日誌；沒有狀態改變。代碼：`core/.../isi/world.rs`。

### 錯誤模型
通用信封：`InstructionExecutionError`，包含評估錯誤、查詢失敗、轉換、未找到實體、重複、可鑄造性、數學、無效參數和不變違規等變體。枚舉和幫助程序位於 `crates/iroha_data_model/src/isi/mod.rs` 下的 `pub mod error` 中。

---## 事務和可執行文件
- `Executable`：`Instructions(ConstVec<InstructionBox>)` 或 `Ivm(IvmBytecode)`；字節碼序列化為 base64。代碼：`crates/iroha_data_model/src/transaction/executable.rs`。
- `TransactionBuilder`/`SignedTransaction`：使用元資料、`chain_id`、`authority`、`creation_time_ms`、選用的 `creation_time_ms`、可包裝和 `creation_time_ms`、包裝可執行檔和編號代碼：`crates/iroha_data_model/src/transaction/`。
- 在運行時，`iroha_core` 透過 `Execute for InstructionBox` 執行 `InstructionBox` 批次，向下轉換為適當的 `*Box` 或具體指令。代碼：`crates/iroha_core/src/smartcontracts/isi/mod.rs`。
- 執行時間執行器驗證預算（使用者提供的執行器）：來自參數的基礎 `executor.fuel` 加上選購的交易元資料 `additional_fuel` (`u64`)，在交易內的指令/觸發器驗證之間共用。

---## 不變量和註釋（來自測試和防護）
- 創世保護：無法註冊 `genesis` 網域或 `genesis` 網域中的帳戶； `genesis` 帳戶無法註冊。代碼/測試：`core/.../isi/world.rs`、`core/.../smartcontracts/isi/mod.rs`。
- 數位資產在鑄造/轉移/銷毀時必須符合 `NumericSpec` 要求；規格不匹配產生 `TypeError::AssetNumericSpec`。
- 可鑄造性：`Once` 允許單次鑄造，然後翻轉至 `Not`； `Limited(n)` 在翻轉至 `Not` 之前正好允許 `n` 薄荷糖。嘗試禁止在 `Infinitely` 上鑄造會導致 `MintabilityError::ForbidMintOnMintable`，配置 `Limited(0)` 會產生 `MintabilityError::InvalidMintabilityTokens`。
- 元資料操作是關鍵精確的；刪除不存在的金鑰是一個錯誤。
- 觸發過濾器可以是不可鑄造的；那麼 `Register<Trigger>` 只允許 `Exactly(1)` 重複。
- 觸發元資料鍵 `__enabled` (bool) 閘執行；缺少預設啟用的觸發器，且停用的觸發器會在資料/時間/按呼叫路徑上跳過。
- 確定性：所有算術都使用檢查操作；下溢/溢出返回鍵入的數學錯誤；零餘額會刪除資產條目（無隱藏狀態）。

---## 實際例子
- 鑄造和轉讓：
  - `Mint::asset_numeric(10, asset_id)` → 若規格/可鑄造性允許，則增加 10；事件：`AssetEvent::Added`。
  - `Transfer::asset_numeric(asset_id, 5, to_account)` → 移動 5；刪除/新增事件。
- 元資料更新：
  - `SetKeyValue::account(account_id, "avatar".parse()?, json)` → 更新插入；透過 `RemoveKeyValue::account(...)` 刪除。
- 角色/權限管理：
  - `Grant::account_role(role_id, account)`、`Grant::role_permission(perm, role)` 及其 `Revoke` 對應項。
- 觸發器生命週期：
  - `Register::trigger(Trigger::new(id, Action::new(exec, repeats, authority, filter)))`，具有過濾器暗示的可鑄造性檢查； `ExecuteTrigger::new(id).with_args(&args)` 必須與配置的權限相符。
  - 可透過將元資料鍵 `__enabled` 設定為 `false` 來停用觸發器（缺少預設啟用）；透過 `SetKeyValue::trigger` 或 IVM `set_trigger_enabled` 系統呼叫系統進行切換。
  - 載入時修復觸發器儲存：刪除重複的 id、不符合的 id 以及引用遺失字節碼的觸發器；重新計算字節碼引用計數。
  - 如果觸發器的 IVM 字節碼在執行時遺失，則觸發器將被刪除，並且執行將被視為具有失敗結果的無操作。
  - 耗盡的觸發器立即被移除；如果在執行過程中遇到耗盡的條目，則會將其修剪並視為丟失。
- 參數更新：
  - `SetParameter(SumeragiParameter::BlockTimeMs(2500).into())` 更新並發出 `ConfigurationEvent::Changed`。CLI / Torii asset-definition id + 別名範例：
- 使用規範輔助+顯式名稱+長別名進行註冊：
  - `iroha ledger asset definition register --id 66owaQmAQMuHxPzxUN3bqZ6FJfDa --name pkr --alias pkr#ubl.sbp`
- 使用規範輔助+顯式名稱+短別名進行註冊：
  - `iroha ledger asset definition register --id 66owaQmAQMuHxPzxUN3bqZ6FJfDa --name pkr --alias pkr#sbp`
- 由別名 + 帳戶組成的 Mint：
  - `iroha ledger asset mint --definition-alias pkr#ubl.sbp --account <i105> --quantity 500`
- 將別名解析為規範援助：
  - `POST /v1/assets/aliases/resolve` 與 JSON `{ "alias": "pkr#ubl.sbp" }`

遷移注意事項：
- `name#domain` 在第一個版本中故意不支援文字資產定義 ID。
- 鑄造/銷毀/轉移邊界的資產 ID 維持規範 `<asset-definition-id>#<i105-account-id>`；將 `iroha tools encode asset-id` 與 `--definition <base58-asset-definition-id>` 或 `--alias ...` 加 `--account` 結合使用。

---

## 可追溯性（選定來源）
 - 資料模型核心：`crates/iroha_data_model/src/{account.rs,domain.rs,asset/**,nft.rs,role.rs,permission.rs,metadata.rs,trigger/**,parameter/**}`。
 - ISI 定義和註冊表：`crates/iroha_data_model/src/isi/{mod.rs,register.rs,transfer.rs,mint_burn.rs,transparent.rs,registry.rs}`。
 - ISI 執行：`crates/iroha_core/src/smartcontracts/isi/{mod.rs,world.rs,domain.rs,account.rs,asset.rs,nft.rs,triggers/**}`。
 - 事件：`crates/iroha_data_model/src/events/**`。
 - 交易：`crates/iroha_data_model/src/transaction/**`。

如果您希望將此規範擴展為呈現的 API/行為表或交叉連結到每個具體事件/錯誤，請說出這個詞，我將擴展它。