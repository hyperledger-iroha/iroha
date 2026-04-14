---
lang: zh-hant
direction: ltr
source: docs/source/kotodama_grammar.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ac9b1fa221c6de46c139ee3a3c280957adad4910b49015fbb746259a4af22659
source_last_modified: "2026-01-30T12:29:10.190473+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Kotodama 語言語法和語義

本文檔指定了 Kotodama 語言語法（詞法分析、語法）、鍵入規則、確定性語義，以及程序如何使用 Norito 指針 ABI 約定降低為 IVM 字節碼 (.to)。 Kotodama 源使用 .ko 擴展名。編譯器發出 IVM 字節碼 (.to)，並且可以選擇返回清單。

內容
- 概述和目標
- 詞彙結構
- 類型和文字
- 聲明和模塊
- 合約容器和元數據
- 功能及參數
- 聲明
- 表達式
- 內置函數和指針 ABI 構造函數
- 收藏品和地圖
- 確定性迭代和界限
- 錯誤和診斷
- Codegen 映射到 IVM
- ABI、標頭和清單
- 路線圖

## 概述和目標

- 確定性：程序必須在硬件上產生相同的結果；沒有浮點或不確定源。所有主機交互都是通過帶有 Norito 編碼參數的系統調用進行的。
- 可移植：目標為 Iroha 虛擬機 (IVM) 字節碼，而不是物理 ISA。存儲庫中可見的類似 RISC-V 的編碼是 IVM 解碼的實現細節，並且不得改變可觀察到的行為。
- 可審計：小而明確的語義；語法到 IVM 操作碼和主機系統調用的清晰映射。
- 有界性：無界數據上的循環必須帶有顯式邊界。地圖迭代有嚴格的規則來保證確定性。

## 詞彙結構

空白和註釋
- 空格分隔標記，否則無關緊要。
- 行註釋以 `//` 開始，一直到行尾。
- 塊註釋 `/* ... */` 不嵌套。

標識符
- 開始：`[A-Za-z_]`，然後繼續 `[A-Za-z0-9_]*`。
- 區分大小寫; `_` 是一個有效的標識符，但不鼓勵使用。

關鍵詞（保留）
- `seiyaku`、`hajimari`、`kotoage`、`kaizen`、`state`、`struct`、`fn`、`let`、 `const`、`return`、`if`、`else`、`while`、`for`、`in`、`break`、 `continue`、`true`、`false`、`permission`、`kotoba`。

運算符和標點符號
- 算術：`+ - * / %`
- 按位：`& | ^ ~`，移位 `<< >>`
- 比較：`== != < <= > >=`
- 邏輯：`&& || !`
- 分配：`= += -= *= /= %= &= |= ^= <<= >>=`
- 其他：`: , ; . :: ->`
- 支架：`() [] {}`文字
- 整數：十進制 (`123`)、十六進制 (`0x2A`)、二進制 (`0b1010`)。所有整數在運行時都是有符號的 64 位；不帶後綴的文字通過推理輸入或默認輸入為 `int`。
- 字符串：帶轉義的雙引號（`\n`、`\r`、`\t`、`\0`、`\xNN`、`\u{...}`、`\"`、 `\\`); UTF-8。原始字符串 `r"..."` 或 `r#"..."#` 禁用轉義並允許換行。
- 字節：帶轉義的 `b"..."`，或原始 `br"..."` / `rb"..."`；產生 `bytes` 文字。
- 布爾值：`true`、`false`。

## 類型和文字

標量類型
- `int`：64位二進制補碼；算術對 add/sub/mul 進行模 2^64 換行；除法在 IVM 中定義了有符號/無符號變體；編譯器根據語義選擇適當的操作。
- `fixed_u128`、`Amount`、`Balance`：由 Norito `Numeric` 支持的數字別名（有符號十進制，最多 512 位尾數和小數位數）。 Kotodama 將這些別名視為非負數；檢查算術，保留別名，並捕獲溢出或除以零。從 `int` 創建的值使用小數位 0；與 `int` 之間的轉換在運行時進行範圍檢查（非負、整數、適合 i64）。
- `bool`：邏輯真值；降低至 `0`/`1`。
- `string`：不可變的 UTF-8 字符串；傳遞給系統調用時表示為 Norito TLV；虛擬機內操作使用字節片和長度。
- `bytes`：原始 Norito 有效負載；為散列/加密/證明輸入和持久覆蓋的指針 ABI `Blob` 類型別名。

複合類型
- `struct Name { field: Type, ... }` 用戶定義的產品類型。構造函數在表達式中使用調用語法 `Name(a, b, ...)`。支持字段訪問 `obj.field` 並在內部降低為元組樣式位置字段。鏈上持久狀態 ABI 採用 Norito 編碼；編譯器會發出反映結構順序的覆蓋層，並且最近的測試（`crates/iroha_core/tests/kotodama_struct_overlay.rs`）使佈局在各個版本中保持鎖定。
- `Map<K, V>`：確定性關聯圖；語義限制迭代和迭代過程中的突變（見下文）。
- `Tuple (T1, T2, ...)`：帶有位置字段的匿名產品類型；用於多次返回。

特殊指針 ABI 類型（面向主機）
- `AccountId`、`AssetDefinitionId`、`Name`、`Json`、`NftId`、`Blob` 和類似的不是一流的運行時類型。它們是生成輸入區域（Norito TLV 信封）的類型化、不可變指針的構造函數，並且只能用作系統調用參數或在變量之間移動而無需突變。

類型推斷
- 本地 `let` 綁定從初始值設定項推斷類型。函數參數必須顯式鍵入。除非不明確，否則可以推斷返回類型。

## 聲明和模塊頂級項目
- 合約：`seiyaku Name { ... }` 包含函數、狀態、結構和元數據。
- 允許但不鼓勵每個文件有多個合同；一個主 `seiyaku` 用作清單中的默認條目。
- `struct` 聲明定義合約內的用戶類型。

能見度
- `kotoage fn` 表示公共入口點；可見性影響調度程序權限，而不是代碼生成。
- 可選訪問提示：`#[access(read=..., write=...)]` 可以在 `fn`/`kotoage fn` 之前提供清單讀/寫密鑰。編譯器還會自動發出諮詢提示；不透明的主機調用會回退到保守的通配符密鑰 (`*`) 並顯示診斷，除非提供顯式訪問提示，因此調度程序可以選擇動態預傳遞以獲得更細粒度的密鑰。

## 合約容器和元數據

語法
```
seiyaku Name {
  meta {
    abi_version: 1,
    vector_length: 0,
    max_cycles: 0,
    features: ["zk", "simd"],
  }

  state int counter;

  hajimari() { counter = 0; }

  kotoage fn inc() { counter = counter + 1; }
}
```

語義學
- `meta { ... }` 字段覆蓋已發出的 IVM 標頭的編譯器默認值：`abi_version`、`vector_length`（0 表示未設置）、`max_cycles`（0 表示編譯器默認值）、`features` 切換標頭功能位（ZK 跟踪、向量）宣布）。編譯器將 `max_cycles: 0` 視為“使用默認值”，並發出配置的非零默認值以滿足准入要求。不支持的功能將被忽略並發出警告。當省略 `meta {}` 時，編譯器將發出 `abi_version = 1` 並使用其餘標頭字段的選項默認值。
- `features: ["zk", "simd"]`（別名：`"vector"`）顯式請求相應的標頭位。未知的特徵字符串現在會產生解析器錯誤而不是被忽略。
- `state` 聲明持久合約變量。編譯器降低對 `STATE_GET/STATE_SET/STATE_DEL` 系統調用的訪問，主機將它們暫存在每個事務覆蓋中（檢查點/恢復回滾、提交時刷新到 WSV）。針對文字狀態路徑發出訪問提示；動態鍵回退到映射級衝突鍵。對於顯式主機支持的讀/寫，請使用 `state_get/state_set/state_del` 幫助程序和 `map.ensure(...)` 映射幫助程序；這些通過 Norito TLV 進行路由並保持名稱/字段順序穩定。
- 保留狀態標識符；參數中隱藏 `state` 名稱或 `let` 綁定被拒絕 (`E_STATE_SHADOWED`)。
- 狀態映射值不是一流的：直接使用狀態標識符進行映射操作和迭代。將狀態映射綁定或傳遞給用戶定義的函數被拒絕 (`E_STATE_MAP_ALIAS`)。
- 持久狀態映射當前僅支持 `int` 和指針 ABI 密鑰類型；其他鍵類型在編譯時被拒絕。
- 持久狀態字段必須是 `int`、`bool`、`Json`、`Blob`/`bytes` 或指針 ABI 類型（包括由這些字段組成的結構體/元組）； `string` 不支持持久狀態。

### 言葉本地化
語法
```
kotoba {
  "E_UNBOUNDED_ITERATION": { en: "Loop over map lacks a bound." }
}
```語義學
- `kotoba` 條目將轉換錶附加到合同清單（`kotoba` 字段）。
- 消息 ID 和語言標籤接受標識符或字符串文字；條目必須非空。
- 重複的 `msg_id` + 語言標記對在編譯時被拒絕。

## 觸發聲明

觸發器聲明將調度元數據附加到入口點清單並自動註冊
當合同實例被激活時（停用時被刪除）。它們在一個內部被解析
`seiyaku` 塊。

語法
```
register_trigger wake {
  call run;
  on time pre_commit;
  repeats 2;
  authority "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB";
  metadata { tag: "alpha"; count: 1; enabled: true; }
}
```

註釋
- `call` 必須引用同一合約中的公共 `kotoage fn` 入口點；一個可選的
  清單中記錄了 `namespace::entrypoint` 但跨合約回調被拒絕
  目前由運行時（僅限本地回調）。
- 支持的過濾器：`time pre_commit` 和 `time schedule(start_ms, period_ms?)`，以及
  `execute trigger <name>` 用於按調用觸發器、`data any` 和管道過濾器
  （`pipeline transaction`、`pipeline block`、`pipeline merge`、`pipeline witness`）。
- `authority` 可選擇覆蓋觸發權限（AccountId 字符串文字）。如果省略，
  運行時使用合約激活權限。
- 元數據值必須是 JSON 文本（`string`、`number`、`bool`、`null`）或 `json!(...)`。
- 運行時注入的觸發器元數據鍵：`contract_namespace`、`contract_id`、
  `contract_entrypoint`、`contract_code_hash`、`contract_trigger_id`。

## 函數和參數

語法
- 聲明：`fn name(param1: Type, param2: Type, ...) -> Ret { ... }`
- 公共：`kotoage fn name(...) { ... }`
- 初始化程序：`hajimari() { ... }`（在部署時由運行時調用，而不是由虛擬機本身調用）。
- 升級掛鉤：`kaizen(args...) permission(Role) { ... }`。

參數和返回值
- 根據 ABI，參數作為值或 INPUT 指針 (Norito TLV) 在寄存器 `r10..r22` 中傳遞；額外的參數溢出到堆棧。
- 函數返回零或一個標量或元組。標量的主要返回值位於 `r10` 中；按照慣例，元組在堆棧/輸出中具體化。

## 聲明- 變量綁定：`let x = expr;`、`let mut x = expr;`（可變性是編譯時檢查；僅允許本地變量運行時突變）。
- 賦值：`x = expr;` 和復合形式 `x += 1;` 等。目標必須是變量或映射索引；元組/結構字段是不可變的。
- 數字別名（`fixed_u128`、`Amount`、`Balance`）是不同的 `Numeric` 支持類型；算術保留別名，混合別名需要通過 `int` 綁定進行轉換。在運行時檢查與 `int` 之間的轉換（非負、整數、範圍限制）。
- 控制：`if (cond) { ... } else { ... }`、`while (cond) { ... }`、C 型 `for (init; cond; step) { ... }`。
  - `for` 初始化器和步驟必須是簡單的 `let name = expr` 或表達式語句；複雜的解構被拒絕（`E0005`、`E0006`）。
  - `for` 範圍：來自 init 子句的綁定在循環中及其之後可見；在主體或步驟中創建的綁定不會逃脫循環。
- `int`、`bool`、`string`、指針 ABI 標量（例如 `AccountId`、 `Name`、`Blob`/`bytes`、`Json`)；元組、結構體和映射不具有可比性。
- 地圖循環：`for (k, v) in map { ... }`（確定性；見下文）。
- 流量：`return expr;`、`break;`、`continue;`。
- 調用：`name(args...);` 或 `call name(args...);`（兩者都接受；編譯器規範化為調用語句）。
- 斷言：`assert(cond);`、`assert_eq(a, b);` 映射到非 ZK 構建中的 IVM `ASSERT*` 或 ZK 模式中的 ZK 約束。

## 表達式

優先級（高→低）
1. 會員/索引：`a.b`、`a[b]`
2. 一元：`! ~ -`
3.乘法：`* / %`
4.添加劑：`+ -`
5.班次：`<< >>`
6. 關係：`< <= > >=`
7. 平等：`== !=`
8. 按位與/異或/或：`& ^ |`
9. 邏輯與/或：`&& ||`
10.三元：`cond ? a : b`

調用和元組
- 調用使用位置參數：`f(a, b, c)`。
- 元組文字：`(a, b, c)` 和解構：`let (x, y) = pair;`。
- 元組解構需要具有匹配數量的元組/結構類型；不匹配會被拒絕。

字符串和字節
- 字符串為 UTF-8；源代碼中接受原始字符串和字節文字形式。
- 字節文字 (`b"..."`、`br"..."`、`rb"..."`) 低於 `bytes` (Blob) 指針；當系統調用需要 NoritoBytes TLV 有效負載時，用 `norito_bytes(...)` 包裝。

## 內置函數和指針 ABI 構造函數

指針構造函數（將 Norito TLV 發出到 INPUT 並返回類型化指針）
- `account_id(string) -> AccountId*`
- `asset_definition(string) -> AssetDefinitionId*`
- `asset_id(string) -> AssetId*`
- `domain(string) | domain_id(string) -> DomainId*`
- `name(string) -> Name*`
- `json(string) -> Json*`
- `nft_id(string) -> NftId*`
- `blob(bytes|string) -> Blob*`
- `norito_bytes(bytes|string) -> NoritoBytes*`
- `dataspace_id(string|0xhex) -> DataSpaceId*`
- `axt_descriptor(string|0xhex) -> AxtDescriptor*`
- `asset_handle(string|0xhex) -> AssetHandle*`
- `proof_blob(string|0xhex) -> ProofBlob*`Prelude 宏為這些構造函數提供更短的別名和內聯驗證：
- `account!("<i105-account-id>")`, `account_id!("<i105-account-id>")`
- `asset_definition!("62Fk4FPcMuLvW5QjDGNF2a4jAmjM")`、`asset_id!("62Fk4FPcMuLvW5QjDGNF2a4jAmjM")`
- `domain!("wonderland")`, `domain_id!("wonderland")`
- `name!("example")`
- `json!("{\"hello\":\"world\"}")` 或結構化文字，例如 `json!{ hello: "world" }`
- `nft_id!("dragon$demo")`、`blob!("bytes")`、`norito_bytes!("...")`

這些宏擴展為上面的構造函數，並在編譯時拒絕無效的文字。

實施情況
- 已實現：上面的構造函數接受字符串文字參數，下面的構造函數接受放置在 INPUT 區域中的類型化 Norito TLV 信封。它們返回可用作系統調用參數的不可變類型指針。非文字字符串表達式被拒絕；使用 `Blob`/`bytes` 進行動態輸入。 `blob`/`norito_bytes` 在運行時也接受 `bytes` 類型的值，無需宏填充程序。
- 擴展形式：
  - `json(Blob[NoritoBytes]) -> Json*` 通過 `JSON_DECODE` 系統調用。
  - `name(Blob[NoritoBytes]) -> Name*` 通過 `NAME_DECODE` 系統調用。
  - 來自 Blob/NoritoBytes 的指針解碼：任何指針構造函數（包括 AXT 類型）接受 `Blob`/`NoritoBytes` 有效負載，並降低為具有預期類型 id 的 `POINTER_FROM_NORITO`。
  - 指針形式的傳遞：`name(Name) -> Name*`、`blob(Blob) -> Blob*`、`norito_bytes(Blob) -> Blob*`。
  - 支持方法糖：`s.name()`、`s.json()`、`b.blob()`、`b.norito_bytes()`。

主機/系統調用內置函數（映射到 SCALL；精確數字在 ivm.md 中）
- `mint_asset(AccountId*, AssetDefinitionId*, numeric)`
- `burn_asset(AccountId*, AssetDefinitionId*, numeric)`
- `transfer_asset(AccountId*, AccountId*, AssetDefinitionId*, numeric)`
- `set_account_detail(AccountId*, Name*, Json*)`
- `execute_instruction(Blob[NoritoBytes])`
- `execute_query(Blob[NoritoBytes]) -> Blob`
- `subscription_bill()`
- `subscription_record_usage()`
- `nft_mint_asset(NftId*, AccountId*)`
- `nft_transfer_asset(AccountId*, NftId*, AccountId*)`
- `nft_set_metadata(NftId*, Json*)`
- `nft_burn_asset(NftId*)`
- `authority() -> AccountId*`
- `register_domain(DomainId*)`
- `unregister_domain(DomainId*)`
- `transfer_domain(AccountId*, DomainId*, AccountId*)`
- `vrf_verify(Blob, Blob, Blob, int variant) -> Blob`
- `vrf_verify_batch(Blob) -> Blob`
- `axt_begin(AxtDescriptor*)`
- `axt_touch(DataSpaceId*, Blob[NoritoBytes]? manifest)`
- `verify_ds_proof(DataSpaceId*, ProofBlob?)`
- `use_asset_handle(AssetHandle*, Blob[NoritoBytes], ProofBlob?)`
- `axt_commit()`
- `contains(Map<K,V>, K) -> bool`

內置實用程序
- `info(string|int)`：通過 OUTPUT 發出結構化事件/消息。
- `hash(blob) -> Blob*`：將 Norito 編碼的哈希值返回為 Blob。
- `build_submit_ballot_inline(election_id, ciphertext, nullifier32, backend, proof, vk) -> Blob*` 和 `build_unshield_inline(asset, to, amount, inputs32, backend, proof, vk) -> Blob*`：內聯 ISI 構建器；所有參數必須是編譯時文字（字符串文字或文字的指針構造函數）。 `nullifier32` 和 `inputs32` 必須恰好為 32 個字節（原始字符串或 `0x` 十六進制），並且 `amount` 必須為非負數。
- `schema_info(Name*) -> Json* { "id": "<hex>", "version": N }`
- `encode_schema(Name*, Json*) -> Blob`：使用主機架構註冊表對 JSON 進行編碼（除了訂單/交易示例之外，DefaultRegistry 還支持 `QueryRequest` 和 `QueryResponse`）。
- `decode_schema(Name*, Blob|bytes) -> Json*`：使用主機架構註冊表解碼 Norito 字節。
- `pointer_to_norito(ptr) -> NoritoBytes*`：將現有指針 ABI TLV 包裝為 NoritoBytes 以進行存儲或傳輸。
- `isqrt(int) -> int`：作為 IVM 操作碼實現的整數平方根 (`floor(sqrt(x))`)。
- `min(int, int) -> int`、`max(int, int) -> int`、`abs(int) -> int`、`div_ceil(int, int) -> int`、`gcd(int, int) -> int`、`mean(int, int) -> int` — 由本機 IVM 操作碼支持的融合算術助手（ceil 除法陷阱除以零）。註釋
- 內置是薄墊片；編譯器將它們降低為寄存器移動和 `SCALL`。
- 指針構造函數是純的：VM 確保 INPUT 中的 Norito TLV 在調用期間不可變。
 - 具有指針 ABI 字段的結構（例如，`DomainId`、`AccountId`）可用於按人體工程學對系統調用參數進行分組。編譯器將 `obj.field` 映射到正確的寄存器/值，無需額外分配。

## 收藏和地圖

類型：`Map<K, V>`
- 內存中映射（通過 `Map::new()` 堆分配或作為參數傳遞）存儲單個鍵/值對；鍵和值必須是字大小的類型：`int`、`bool`、`string`、`Blob`、`bytes`、`Json` 或指針類型（例如，`AccountId`、 `Name`）。
- 持久狀態映射 (`state Map<...>`) 使用 Norito 編碼的鍵/值。支持的按鍵：`int` 或指針類型。支持的值：`int`、`bool`、`Json`、`Blob`/`bytes` 或指針類型。
- `Map::new()` 分配並清零初始化單個內存條目（鍵/值 = 0）；對於非 `Map<int,int>` 映射，提供顯式類型註釋或返回類型。
- 狀態地圖不是一流的值：您無法重新分配它們（例如，`M = Map::new()`）；通過索引更新條目 (`M[key] = value`)。
- 操作：
  - 索引：`map[key]` 獲取/設置值（通過主機系統調用執行設置；請參閱運行時 API 映射）。
  - 存在：`contains(map, key) -> bool`（降低的助手；可能是內部系統調用）。
  - 迭代：`for (k, v) in map { ... }`，具有確定性順序和變異規則。

確定性迭代規則
- 迭代集是循環入口處鍵的快照。
- 順序是 Norito 編碼密鑰的嚴格字節字典順序升序。
- 循環期間對迭代映射的結構修改（插入/刪除/清除）會導致確定性 `E_ITER_MUTATION` 陷阱。
- 需要有界性：地圖上聲明的最大值 (`@max_len`)、顯式屬性 `#[bounded(n)]` 或使用 `.take(n)`/`.range(..)` 的顯式邊界；否則編譯器會發出 `E_UNBOUNDED_ITERATION`。

界限助手
- `#[bounded(n)]`：地圖表達式上的可選屬性，例如`for (k, v) in my_map #[bounded(2)] { ... }`。
- `.take(n)`：從頭開始迭代第一個 `n` 條目。
- `.range(start, end)`：迭代半開區間 `[start, end)` 中的條目。語義相當於 `start` 和 `n = end - start`。關於動態邊界的註釋
- 文字邊界：完全支持 `n`、`start` 和 `end` 作為整數文字，並編譯為固定的迭代次數。
- 非文字邊界：當在 `ivm` 包中啟用 `kotodama_dynamic_bounds` 功能時，編譯器接受動態 `n`、`start` 和 `end` 表達式，並插入運行時斷言以確保安全（非負、 `end >= start`）。降低會通過 `if (i < n)` 檢查發出最多 K 個受保護的迭代，以避免額外的主體執行（默認 K=2）。您可以通過 `CompilerOptions { dynamic_iter_cap, .. }` 以編程方式調整 K。
- 在編譯前運行 `koto_lint` 檢查 Kotodama lint 警告；主編譯器總是在解析和類型檢查後繼續進行降低。
- 錯誤代碼記錄在 [Kotodama 編譯器錯誤代碼](./kotodama_error_codes.md) 中；使用 `koto_compile --explain <code>` 進行快速解釋。

## 錯誤和診斷

編譯時診斷（示例）
- `E_UNBOUNDED_ITERATION`：地圖循環缺少界限。
- `E_MUT_DURING_ITER`：循環體中迭代映射的結構突變。
- `E_STATE_SHADOWED`：本地綁定不能隱藏 `state` 聲明。
- `E_BREAK_OUTSIDE_LOOP`：`break` 在循環外部使用。
- `E_CONTINUE_OUTSIDE_LOOP`：`continue` 在循環外部使用。
- `E0005`：for 循環初始值設定項比支持的更複雜。
- `E0006`：for 循環步驟子句比支持的更複雜。
- `E_BAD_POINTER_USE`：在需要第一類類型的情況下使用指針 ABI 構造函數結果。
- `E_UNRESOLVED_NAME`、`E_TYPE_MISMATCH`、`E_ARITY_MISMATCH`、`E_DUP_SYMBOL`。
- 工具：`koto_compile` 在發出字節碼之前運行 lint pass；使用 `--no-lint` 跳過或使用 `--deny-lint-warnings` 使 lint 輸出的構建失敗。

運行時 VM 錯誤（已選擇；完整列表位於 ivm.md 中）
- `E_NORITO_INVALID`、`E_OOB`、`E_UNALIGNED`、`E_SCALL_UNKNOWN`、`E_ASSERT`、`E_ASSERT_EQ`、`E_ITER_MUTATION`。

錯誤信息
- 診斷攜帶穩定的 `msg_id`，映射到 `kotoba {}` 轉換錶（如果可用）中的條目。

## Codegen 映射到 IVM

管道
1. Lexer/Parser 生成 AST。
2. 語義分析解析名稱、檢查類型並填充符號表。
3. IR 降低為簡單的類似 SSA 的形式。
4. IVM GPR 的寄存器分配（根據調用約定，args/ret 為 `r10+`）；溢出到堆棧。
5. 字節碼發射：允許混合 IVM 原生編碼和 RV 兼容編碼；使用 `abi_version`、特徵、向量長度和 `max_cycles` 發出的元數據標頭。映射亮點
- 算術和邏輯映射到 IVM ALU 操作。
- 分支和控制映射到條件分支和跳轉；編譯器在有利可圖的情況下使用壓縮形式。
- 本地內存溢出到VM堆棧；強制對齊。
- 內置寄存器移動和 `SCALL` 的 8 位數字。
- 指針構造函數將 Norito TLV 放入 INPUT 區域並生成它們的地址。
- 斷言映射到 `ASSERT`/`ASSERT_EQ`，它捕獲非 ZK 執行並在 ZK 構建中發出約束。

決定論約束
- 無FP；沒有不確定的系統調用。
- SIMD/GPU加速對字節碼不可見，並且必須位相同；編譯器不會發出特定於硬件的操作。

## ABI、標頭和清單

IVM 編譯器設置的頭字段
- `version`：IVM 字節碼格式版本（主要.次要）。
- `abi_version`：系統調用表和指針 ABI 架構版本。
- `feature_bits`：功能標誌（例如，`ZK`、`VECTOR`）。
- `vector_len`：邏輯向量長度（0 → 未設置）。
- `max_cycles`：准入限制和 ZK 填充提示。

清單（可選 sidecar）
- `code_hash`、`abi_hash`、`meta {}` 塊的元數據、編譯器版本以及可重現性的構建提示。

## 路線圖

- **KD-231（2026 年 4 月）：** 添加迭代邊界的編譯時範圍分析，以便循環向調度程序公開有界訪問集。
- **KD-235（2026 年 5 月）：** 引入與 `string` 不同的一流 `bytes` 標量，以實現指針構造函數和 ABI 清晰度。
- **KD-242（2026 年 6 月）：** 使用確定性回退擴展功能標誌後面的內置操作碼集（哈希/簽名驗證）。
- **KD-247 (Jun2026)：** 穩定錯誤 `msg_id` 並維護 `kotoba {}` 表中的映射以進行本地化診斷。
### 清單排放

- Kotodama 編譯器 API 可以通過 `ivm::kotodama::compiler::Compiler::compile_source_with_manifest` 返回 `ContractManifest` 以及已編譯的 `.to`。
- 領域：
  - `code_hash`：編譯器計算的用於綁定工件的代碼字節的哈希值（不包括 IVM 標頭和文字）。
  - `abi_hash`：程序的 `abi_version` 允許的系統調用表面的穩定摘要（請參閱 `ivm.md` 和 `ivm::syscalls::compute_abi_hash`）。
- 可選的 `compiler_fingerprint` 和 `features_bitmap` 保留用於工具鏈。
- `entrypoints`：導出入口點的有序列表（公共、`hajimari`、`kaizen`），包括其所需的 `permission(...)` 字符串和編譯器的盡力讀/寫關鍵提示，以便准入邏輯和調度程序可以推斷預期的 WSV 訪問。
- 清單用於入場時檢查和登記；有關生命週期，請參閱 `docs/source/new_pipeline.md`。