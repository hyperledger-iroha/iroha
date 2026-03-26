---
lang: zh-hant
direction: ltr
source: docs/source/ivm_syscalls.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: bcf280df1e00065199d386e07b9fd67d8f94c4046d73cfa3b63d1eec18228cd8
source_last_modified: "2026-01-22T16:26:46.570453+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# IVM 系統調用 ABI

本文檔定義了 IVM 系統調用編號、指針 ABI 調用約定、保留編號範圍以及 Kotodama 降低所使用的面向合約的系統調用規範表。它補充了 `ivm.md`（體系結構）和 `kotodama_grammar.md`（語言）。

版本控制
- 可識別的系統調用集取決於字節碼標頭 `abi_version` 字段。第一個版本僅接受 `abi_version = 1`；其他值在入學時被拒絕。活動 `abi_version` 的未知編號確定性地捕獲 `E_SCALL_UNKNOWN`。
- 運行時升級保留 `abi_version = 1` 並且不擴展系統調用或指針 ABI 表面。
- 系統調用 Gas 成本是綁定到字節碼標頭版本的版本化 Gas Schedule 的一部分。請參閱 `ivm.md`（天然氣政策）。

編號範圍
- `0x00..=0x1F`：VM 核心/實用程序（調試/退出幫助程序在 `CoreHost` 下可用；其餘開發幫助程序僅是模擬主機）。
- `0x20..=0x5F`：Iroha 核心 ISI 橋（在 ABI v1 中穩定）。
- `0x60..=0x7F`：由協議功能門控的擴展 ISI（啟用後仍是 ABI v1 的一部分）。
- `0x80..=0xFF`：主機/加密助手和保留插槽；僅接受 ABI v1 允許列表中存在的號碼。

耐用幫手 (ABI v1)
- 持久狀態幫助程序系統調用（0x50–0x5A：STATE_{GET,SET,DEL}、ENCODE/DECODE_INT、BUILD_PATH_*、JSON/SCHEMA 編碼/解碼）是 V1 ABI 的一部分，並包含在 `abi_hash` 計算中。
- CoreHost 將 STATE_{GET,SET,DEL} 連接到 WSV 支持的持久智能合約狀態；開發/測試主機可以在本地保留，但必須保留相同的系統調用語義。

指針 ABI 調用約定（智能合約系統調用）
- 參數作為原始 `u64` 值或作為指向不可變 Norito TLV 包絡的 INPUT 區域的指針放置在寄存器 `r10+` 中（例如，`AccountId`、`AssetDefinitionId`、`Name`、 `Json`、`NftId`）。
- 標量返回值是從主機返回的 `u64`。主機將指針結果寫入`r10`。

規範系統調用表（子集）|十六進制 |名稱 |參數（在 `r10+` 中）|返回|氣體（基礎+變量）|筆記|
|------|----------------------------------------|----------------------------------------------------------------------------|------------------------|--------------------------------------------------------|------|
| 0x1A | 0x1A設置帳戶詳細信息 | `&AccountId`、`&Name`、`&Json` | `u64=0` | `G_set_detail + bytes(val)` |寫入帳戶詳細信息 |
| 0x22 | 0x22 MINT_資產 | `&AccountId`、`&AssetDefinitionId`、`&NoritoBytes(Numeric)` | `u64=0` | `G_mint` |向賬戶鑄造 `amount` 資產 |
| 0x23 | 0x23 BURN_ASSET | 燒毀資產`&AccountId`、`&AssetDefinitionId`、`&NoritoBytes(Numeric)` | `u64=0` | `G_burn` |從帳戶中燒毀 `amount` |
| 0x24 | 0x24轉讓_資產| `&AccountId(from)`、`&AccountId(to)`、`&AssetDefinitionId`、`&NoritoBytes(Numeric)` | `u64=0` | `G_transfer` |在賬戶之間轉賬 `amount` |
| 0x29 | 0x29 TRANSFER_V1_BATCH_BEGIN | 傳輸– | `u64=0` | `G_transfer` |開始 FASTPQ 傳輸批次範圍 |
| 0x2A | 0x2A TRANSFER_V1_BATCH_END | – | `u64=0` | `G_transfer` |刷新累積的 FASTPQ 傳輸批次 |
| 0x2B | 0x2B TRANSFER_V1_BATCH_APPLY | 轉移_V1_BATCH_APPLY `r10=&NoritoBytes(TransferAssetBatch)` | `u64=0` | `G_transfer` |在單個系統調用中應用 Norito 編碼批處理 |
| 0x25 | 0x25 NFT_MINT_資產 | `&NftId`，`&AccountId(owner)` | `u64=0` | `G_nft_mint_asset` |註冊新的 NFT |
| 0x26 | 0x26 NFT_TRANSFER_ASSET | NFT_TRANSFER_ASSET | `&AccountId(from)`、`&NftId`、`&AccountId(to)` | `u64=0` | `G_nft_transfer_asset` |轉讓 NFT 所有權 |
| 0x27 | 0x27 NFT_SET_METADATA | NFT_SET_METADATA | `&NftId`、`&Json` | `u64=0` | `G_nft_set_metadata` |更新 NFT 元數據 |
| 0x28 | 0x28 NFT_BURN_ASSET | NFT_BURN_ASSET | `&NftId` | `u64=0` | `G_nft_burn_asset` |燒毀（銷毀）NFT |
| 0xA1 | 0xA1 SMARTCONTRACT_EXECUTE_QUERY| `r10=&NoritoBytes(QueryRequest)` | `r10=ptr (&NoritoBytes(QueryResponse))` | `G_scq + per_item*items + per_byte*bytes(resp)` |可迭代查詢運行時間短暫； `QueryRequest::Continue` 被拒絕 |
| 0xA2 | 0xA2創建_NFTS_FOR_ALL_USERS | – | `u64=count` | `G_create_nfts_for_all` |幫手;功能門控 || 0xA3 | 0xA3 SET_SMARTCONTRACT_EXECUTION_DEPTH | SET_SMARTCONTRACT_EXECUTION_DEPTH | `depth:u64` | `u64=prev` | `G_set_depth` |行政;功能門控 |
| 0xA4 | 0xA4獲取權限 | –（主機寫入結果）| `&AccountId`| `G_get_auth` |主機將指向當前權限的指針寫入 `r10` |
| 0xF7 | 0xF7獲取_MERKLE_路徑 | `addr:u64`、`out_ptr:u64`、可選 `root_out:u64` | `u64=len` | `G_mpath + len` |寫入路徑（葉→根）和可選的根字節 |
| 0xFA |獲取_MERKLE_COMPACT | `addr:u64`、`out_ptr:u64`、可選 `depth_cap:u64`、可選 `root_out:u64` | `u64=depth` | `G_mpath + depth` | `[u8 depth][u32 dirs_le][u32 count][count*32 siblings]` |
| 0xFF | 0xFF GET_REGISTER_MERKLE_COMPACT| `reg_index:u64`、`out_ptr:u64`、可選 `depth_cap:u64`、可選 `root_out:u64` | `u64=depth` | `G_mpath + depth` |寄存器承諾的相同緊湊佈局|

天然氣執法
- CoreHost 使用本機 ISI 計劃對 ISI 系統調用收取額外的 Gas 費用； FASTPQ 批量傳輸按條目收費。
- ZK_VERIFY 系統調用重用機密驗證氣體計劃（基礎+證明大小）。
- SMARTCONTRACT_EXECUTE_QUERY 收費基本+每項+每字節；排序會使每個項目的成本成倍增加，而未排序的偏移量會增加每個項目的損失。

註釋
- 所有指針參數都引用 INPUT 區域中的 Norito TLV 信封，並在第一次取消引用時進行驗證（出錯時為 `E_NORITO_INVALID`）。
- 所有突變均通過 Iroha 的標準執行器（通過 `CoreHost`）應用，而不是直接由 VM 應用。
- 精確的氣體常數 (`G_*`) 由活動氣體表定義；參見 `ivm.md`。

錯誤
- `E_SCALL_UNKNOWN`：活動 `abi_version` 無法識別系統調用號。
- 輸入驗證錯誤作為 VM 陷阱傳播（例如，對於格式錯誤的 TLV 為 `E_NORITO_INVALID`）。

交叉引用
- 架構和VM語義：`ivm.md`
- 語言和內置映射：`docs/source/kotodama_grammar.md`

代記
- 可以通過以下方式從源生成系統調用常量的完整列表：
  - `make docs-syscalls` → 寫入 `docs/source/ivm_syscalls_generated.md`
  - `make check-docs` → 驗證生成的表是最新的（在 CI 中有用）
- 上面的子集仍然是面向合約的系統調用的精心策劃的穩定表。

## 管理員/角色 TLV 示例（模擬主機）

本節記錄了測試中使用的管理樣式系統調用的模擬 WSV 主機接受的 TLV 形狀和最小 JSON 有效負載。所有指針參數都遵循指針 ABI（放置在 INPUT 中的 Norito TLV 信封）。生產主機可以使用更豐富的模式；這些例子旨在闡明類型和基本形狀。- REGISTER_PEER / UNREGISTER_PEER
  - 參數：`r10=&Json`
  - JSON 示例：`{ "peer": "peer-id-or-info" }`
  - CoreHost 注意：`REGISTER_PEER` 需要一個 `RegisterPeerWithPop` JSON 對象，其中包含 `peer` + `pop` 字節（可選 `activation_at`、`expiry_at`、`hsm`）； `UNREGISTER_PEER` 接受對等 ID 字符串或 `{ "peer": "..." }`。

- 創建_觸發/刪除_觸發/設置_觸發_啟用
  - 創建_觸發：
    - 參數：`r10=&Json`
    - 最小 JSON：`{ "name": "t1" }`（模擬忽略的其他字段）
  - 刪除觸發：
    - 參數：`r10=&Name`（觸發器名稱）
  - SET_TRIGGER_ENABLED：
    - 參數：`r10=&Name`、`r11=enabled:u64`（0 = 禁用，非零 = 啟用）
  - CoreHost 注意：`CREATE_TRIGGER` 需要完整的觸發器規範（base64 Norito `Trigger` 字符串或
    `{ "id": "<trigger_id>", "action": ... }` 以 `action` 作為基數64 Norito `Action` 字符串或
    JSON 對象），並且 `SET_TRIGGER_ENABLED` 切換觸發器元數據鍵 `__enabled`（缺少
    默認啟用）。

- 角色：CREATE_ROLE / DELETE_ROLE / GRANT_ROLE / REVOKE_ROLE
  - 創建角色：
    - 參數：`r10=&Name`（角色名稱）、`r11=&Json`（權限集）
    - JSON 接受鍵 `"perms"` 或 `"permissions"`，每個鍵都是權限名稱的字符串數組。
    - 示例：
      - `{ "perms": [ "mint_asset:rose#wonder" ] }`
      - `{ "permissions": [ "read_assets:<i105-account-id>", "transfer_asset:rose#wonder" ] }`
    - 模擬中支持的權限名稱前綴：
      - `register_domain`、`register_account`、`register_asset_definition`
      - `read_assets:<account_id>`
      - `mint_asset:<asset_definition_id>`
      - `burn_asset:<asset_definition_id>`
      - `transfer_asset:<asset_definition_id>`
  - 刪除角色：
    - 參數：`r10=&Name`
    - 如果任何帳戶仍分配有此角色，則會失敗。
  - 授予角色/撤銷角色：
    - 參數：`r10=&AccountId`（主題）、`r11=&Name`（角色名稱）
  - CoreHost注意：權限JSON可以是完整的`Permission`對象（`{ "name": "...", "payload": ... }`）或字符串（有效負載默認為`null`）； `GRANT_PERMISSION`/`REVOKE_PERMISSION` 接受 `&Name` 或 `&Json(Permission)`。

- 取消註冊操作（域/帳戶/資產）：不變量（模擬）
  - 如果域中存在帳戶或資產定義，UNREGISTER_DOMAIN (`r10=&DomainId`) 將失敗。
  - 如果賬戶餘額非零或擁有 NFT，UNREGISTER_ACCOUNT (`r10=&AccountId`) 會失敗。
  - 如果資產存在任何餘額，UNREGISTER_ASSET (`r10=&AssetDefinitionId`) 將失敗。

註釋
- 這些示例反映了測試中使用的模擬 WSV 主機；真實的節點主機可能會公開更豐富的管理模式或需要額外的驗證。指針 ABI 規則仍然適用：TLV 必須位於 INPUT 中，版本 = 1，類型 ID 必須匹配，有效負載哈希必須驗證。