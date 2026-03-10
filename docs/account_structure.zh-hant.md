---
lang: zh-hant
direction: ltr
source: docs/account_structure.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 01561366c9698c10d29ff3f49ad4c14b22b1796b5c7701cf98d200a140af1caf
source_last_modified: "2026-01-28T17:11:30.635172+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# 賬戶結構 RFC

**狀態：** 已接受 (ADDR-1)  
**受眾：** 數據模型、Torii、Nexus、錢包、治理團隊  
**相關問題：**待定

## 總結

本文檔描述了在
`AccountAddress` (`crates/iroha_data_model/src/account/address.rs`) 和
配套工具。它提供：

- 一個校驗和、面向人的 **Iroha Base58 地址 (IH58)** 由
  `AccountAddress::to_ih58` 將鏈判別式綁定到帳戶
  控制器並提供確定性的互操作友好的文本形式。
- 隱式默認域和本地摘要的域選擇器，帶有
  為未來 Nexus 支持的路由保留全局註冊表選擇器標記（
  註冊表查找**尚未發貨**）。

## 動機

如今，錢包和鏈下工具依賴於原始 `alias@domain` (rejected legacy form) 路由別名。這個
有兩個主要缺點：

1. **無網絡綁定。 ** 該字符串沒有校驗和或鏈前綴，因此用戶
   可以粘貼來自錯誤網絡的地址而不立即反饋。的
   交易最終將被拒絕（鏈不匹配），或者更糟糕的是，成功
   如果目的地存在於本地，則針對非預期帳戶。
2. **域衝突。 ** 域僅限命名空間，並且可以在每個域上重複使用
   鏈。服務聯盟（託管人、橋樑、跨鏈工作流程）
   變得脆弱，因為鏈 A 上的 `finance` 與鏈 A 上的 `finance` 無關
   鏈B。

我們需要一種人性化的地址格式來防止複制/粘貼錯誤
以及從域名到權威鏈的確定性映射。

## 目標

- 描述數據模型中實現的 IH58 Base58 信封以及
  `AccountId` 和 `AccountAddress` 遵循規範解析/別名規則。
- 將配置的鏈判別式直接編碼到每個地址中並
  定義其治理/註冊流程。
- 描述如何在不中斷當前的情況下引入全局域註冊機構
  部署並指定規範化/反欺騙規則。

## 非目標

- 實現跨鏈資產轉移。路由層只返回
  目標鏈。
- 最終確定全球域名發行的治理。此 RFC 重點關注數據
  模型和傳輸原語。

## 背景

### 當前路由別名

```
AccountId {
    domain: DomainId,   // wrapper over Name (ASCII-ish string)
    controller: AccountController // single PublicKey or multisig policy
}

Display: canonical IH58 literal (no `@domain` suffix)
Parse accepts:
- Encoded account identifiers only: IH58 (preferred) and `sora` compressed.
- Runtime parsers reject canonical hex (`0x...`), any `@<domain>` suffix, and alias literals such as `label@domain`.

Multihash hex is canonical: varint bytes are lowercase hex, payload bytes are uppercase hex,
and `0x` prefixes are not accepted.

This text form is now treated as an **account alias**: a routing convenience
that points to the canonical [`AccountAddress`](#2-canonical-address-codecs).
It remains useful for human readability and domain-scoped governance, but it is
no longer considered the authoritative account identifier on-chain.
```

`ChainId` 位於 `AccountId` 之外。節點檢查交易的`ChainId`
針對准入期間的配置 (`AcceptTransactionFail::ChainIdMismatch`)
並拒絕國外交易，但賬戶字符串本身不攜帶
網絡提示。

### 域標識符

`DomainId` 包裝 `Name`（規範化字符串），並且作用域為本地鏈。
每個鏈可以獨立註冊`wonderland`、`finance`等。

### Nexus 上下文

Nexus 負責跨組件協調（通道/數據空間）。它
目前還沒有跨鏈域路由的概念。

## 設計方案

### 1.確定性鏈判別式

`iroha_config::parameters::actual::Common` 現在公開：

```rust
pub struct Common {
    pub chain: ChainId,
    pub chain_discriminant: u16, // globally coordinated
    // ... existing fields
}
```

- **限制：**
  - 每個活動網絡都是唯一的；通過簽署的公共註冊表進行管理
    顯式保留範圍（例如，`0x0000–0x0FFF` test/dev、`0x1000–0x7FFF`
    社區分配，`0x8000–0xFFEF` 治理批准，`0xFFF0–0xFFFF`
    保留）。
  - 對於正在運行的鏈來說是不可變的。改變它需要硬分叉和
    註冊表更新。
- **治理和註冊（計劃）：** 多重簽名治理集將
  維護一個簽名的 JSON 註冊表，將判別式映射到人類別名，並且
  CAIP-2 標識符。該註冊表還不是附帶運行時的一部分。
- **用法：** 通過狀態准入、Torii、SDK 和錢包 API 進行線程化，以便
  每個組件都可以嵌入或驗證它。 CAIP-2 曝光仍是未來
  互操作任務。

### 2. 規範地址編解碼器

Rust 數據模型公開了單個規範的有效負載表示
(`AccountAddress`) 可以以多種面向人的格式發出。 IH58 是
用於共享和規範輸出的首選帳戶格式；壓縮的
`sora` 形式是第二好的、僅限 Sora 的 UX 選項，其中假名字母
增加價值。規範十六進制仍然是一種調試輔助工具。

- **IH58 (Iroha Base58)** – 嵌入鏈條的 Base58 信封
  判別性的。解碼器在將有效負載提升到之前驗證前綴
  規範形式。
- **Sora 壓縮視圖** – 由 **105 個符號**構建的僅限 Sora 的字母表
  58字後追加半角イロハ詩（包括ヰ、ヱ）
  IH58 套裝。字符串以哨兵 `sora` 開頭，嵌入 Bech32m 派生的
  校驗和，並省略網絡前綴（Sora Nexus 由哨兵暗示）。

  ```
  IH58  : 123456789ABCDEFGHJKMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz
  Iroha : ｲﾛﾊﾆﾎﾍﾄﾁﾘﾇﾙｦﾜｶﾖﾀﾚｿﾂﾈﾅﾗﾑｳヰﾉｵｸﾔﾏｹﾌｺｴﾃｱｻｷﾕﾒﾐｼヱﾋﾓｾｽ
  ```
- **規範十六進制** – 規範字節的易於調試的 `0x…` 編碼
  信封。

`AccountAddress::parse_encoded` 自動檢測 IH58（首選）、壓縮（`sora`，第二好）或規範十六進制
（僅限 `0x...`；裸十六進制被拒絕）輸入並返回解碼的有效負載和檢測到的負載
`AccountAddressFormat`。 Torii 現在調用 `parse_encoded` 作為 ISO 20022 補充
尋址並存儲規範的十六進制形式，因此元數據保持確定性
無論原始表示如何。

#### 2.1 標頭字節佈局（ADDR-1a）

每個規範有效負載都排列為 `header · controller`。的
`header` 是一個單字節，用於傳達哪些解析器規則適用於以下字節：
遵循：

```
bit index:   7        5 4      3 2      1 0
             ┌─────────┬────────┬────────┬────┐
payload bit: │version  │ class  │  norm  │ext │
             └─────────┴────────┴────────┴────┘
```

因此，第一個字節打包下游解碼器的模式元數據：

|比特|領域|允許值 |違規錯誤 |
|------|------|----------------|--------------------|
| 7-5 | 7-5 `addr_version` | `0`（v1）。值 `1-7` 保留供將來修訂。 | `0-7` 之外的值觸發 `AccountAddressError::InvalidHeaderVersion`；目前，實現必須將非零版本視為不受支持。 |
| 4-3 | `addr_class` | `0` = 單密鑰，`1` = 多重簽名。 |其他值提高 `AccountAddressError::UnknownAddressClass`。 |
| 2-1 | 2-1 `norm_version` | `1`（規範 v1）。值 `0`、`2`、`3` 被保留。 | `0-3` 之外的值會提高 `AccountAddressError::InvalidNormVersion`。 |
| 0 | `ext_flag` |必須是 `0`。 |設置位升高 `AccountAddressError::UnexpectedExtensionFlag`。 |

Rust 編碼器為單鍵控制器寫入 `0x02`（版本 0、類別 0、
標準 v1，擴展標誌已清除）和多重簽名控制器的 `0x0A`（版本 0，
1 類，規範 v1，擴展標誌已清除）。

#### 2.2 Domainless payload semantics

Canonical payload bytes are domainless: the wire layout is `header · controller`
with no selector segment, no implicit default-domain reconstruction, and no
public decode fallback for legacy scoped-account literals.

Explicit domain context is modeled separately as `ScopedAccountId { account,
domain }` or separate API fields; it is not encoded into `AccountId` payload
bytes.

| Tag | Meaning | Payload | Notes |
|-----|---------|---------|-------|
| `0x00` | Domainless canonical scope | none | Canonical account payloads are domainless; explicit domain context lives outside the address payload. |
| `0x01` | Local domain digest | 12 bytes | Digest = `blake2s_mac(key = "SORA-LOCAL-K:v1", canonical_label)[0..12]`. |
| `0x02` | Global registry entry | 4 bytes | Big-endian `registry_id`; reserved until the global registry ships. |

Domain labels are canonicalised (UTS-46 + STD3 + NFC) before hashing. Unknown tags raise `AccountAddressError::UnknownDomainTag`. When validating an address against a domain, mismatched selectors raise `AccountAddressError::DomainMismatch`.

```
legacy selector segment
┌──────────┬──────────────────────────────────────────────┐
│ tag (u8) │ payload (depends on selector kind, see table)│
└──────────┴──────────────────────────────────────────────┘
```

When present, the selector is immediately adjacent to the controller payload, so
a decoder can walk the wire format in order: read the tag byte, read the
tag-specific payload, then move on to the controller bytes.

**Legacy selector examples**

- *Implicit default* (`tag = 0x00`). No payload. Example canonical hex for the default
  domain using the deterministic test key:
  `0x020001203b6a27bcceb6a42d62a3a8d02a6f0d73653215771de243a63ac048a18b59da29`.
- *Local digest* (`tag = 0x01`). Payload is the 12-byte digest. Example (`treasury` seed
  `0x01`): `0x0201b18fe9c1abbac45b3e38fc5d0001208a88e3dd7409f195fd52db2d3cba5d72ca6709bf1d94121bf3748801b40f6f5c`.
- *Global registry* (`tag = 0x02`). Payload is a big-endian `registry_id:u32`. The bytes
  that follow the payload are identical to the implicit-default case; the selector simply
  replaces the normalised domain string with a registry pointer. Example using
  `registry_id = 0x0000_002A` (decimal 42) and the deterministic default controller:
  `0x02020000002a000120641297079357229f295938a4b5a333de35069bf47b9d0704e45805713d13c201`.

#### 2.3 控制器有效負載編碼 (ADDR-1a)

控制器有效負載是附加在域選擇器之後的另一個標記聯合：|標籤 |控制器|佈局|筆記|
|-----|------------|--------|--------|
| `0x00` |單鍵| `curve_id:u8` · `key_len:u8` · `key_bytes` | `curve_id=0x01` 今天映射到 Ed25519。 `key_len` 綁定到 `u8`；較大的值會引發 `AccountAddressError::KeyPayloadTooLong`（因此大於 255 字節的單密鑰 ML-DSA 公鑰無法進行編碼，必須使用多重簽名）。 |
| `0x01` |多重簽名 | `version:u8` · `threshold:u16` · `member_count:u8` · (`curve_id:u8` · `weight:u16` · `key_len:u16` · `key_bytes`)\* |最多支持 255 個成員 (`CONTROLLER_MULTISIG_MEMBER_MAX`)。未知曲線引發 `AccountAddressError::UnknownCurve`；畸形的政策以 `AccountAddressError::InvalidMultisigPolicy` 的形式出現。 |

多重簽名策略還公開了 CTAP2 風格的 CBOR 地圖和規範摘要，以便
主機和 SDK 可以確定性地驗證控制器。參見
`docs/source/references/multisig_policy_schema.md` (ADDR-1c) 用於架構，
驗證規則、散列程序和黃金裝置。

所有密鑰字節均按照 `PublicKey::to_bytes` 返回的方式進行編碼；如果字節與聲明的曲線不匹配，解碼器會重建 `PublicKey` 實例並引發 `AccountAddressError::InvalidPublicKey`。

> **Ed25519 規範執行 (ADDR-3a)：** 曲線 `0x01` 密鑰必須解碼為簽名者發出的確切字節字符串，並且不得位於小階子組中。節點現在拒絕非規範編碼（例如，以 `2^255-19` 為模減少的值）和身份元素等弱點，因此 SDK 應在提交地址之前顯示匹配驗證錯誤。

##### 2.3.1 曲線標識符註冊表（ADDR-1d）

| ID (`curve_id`) |算法|特色門|筆記|
|----------------|----------|--------------|-----|
| `0x00` |保留 | — |不得發射；解碼器表面 `ERR_UNKNOWN_CURVE`。 |
| `0x01` |埃德25519 | — | Canonical v1 算法 (`Algorithm::Ed25519`)；在默認配置中啟用。 |
| `0x02` | ML-DSA (Dilithium3) | — |使用 Dilithium3 公鑰字節（1952 字節）。單密鑰地址無法編碼 ML-DSA，因為 `key_len` 是 `u8`；多重簽名使用 `u16` 長度。 |
| `0x03` | BLS12-381（正常）| `bls` | G1 中的公鑰（48 字節），G2 中的簽名（96 字節）。 |
| `0x04` | secp256k1 | — |基於 SHA-256 的確定性 ECDSA；公鑰使用 33 字節 SEC1 壓縮形式，簽名使用規範的 64 字節 `r∥s` 佈局。 |
| `0x05` | BLS12-381（小）| `bls` | G2 中的公鑰（96 字節），G1 中的簽名（48 字節）。 |
| `0x0A` | GOST R 34.10-2012（256，A 組）| `gost` |僅當啟用 `gost` 功能時可用。 |
| `0x0B` | GOST R 34.10-2012（256，B 組）| `gost` |僅當啟用 `gost` 功能時可用。 |
| `0x0C` | GOST R 34.10-2012（256，C 組）| `gost` |僅當啟用 `gost` 功能時可用。 |
| `0x0D` | GOST R 34.10-2012（512，A 組）| `gost` |僅當啟用 `gost` 功能時可用。 |
| `0x0E` | GOST R 34.10-2012（512，B 組）| `gost` |僅當啟用 `gost` 功能時可用。 |
| `0x0F` | SM2 | `sm` | DistID 長度 (u16 BE) + DistID 字節 + 65 字節 SEC1 未壓縮 SM2 密鑰；僅當 `sm` 啟用時才可用。 |

插槽 `0x06–0x09` 仍未分配其他曲線；介紹一個新的
算法需要路線圖更新和匹配的 SDK/主機覆蓋範圍。編碼器
必須使用 `ERR_UNSUPPORTED_ALGORITHM` 拒絕任何不支持的算法，並且
解碼器必須在未知 ID 上快速失敗，並保留 `ERR_UNKNOWN_CURVE`
失敗關閉行為。

規範註冊表（包括機器可讀的 JSON 導出）位於
[`docs/source/references/address_curve_registry.md`](source/references/address_curve_registry.md)。
工具應該直接使用該數據集，以便保留曲線標識符
跨 SDK 和操作員工作流程保持一致。

- **SD​​​​K 門控：** SDK 默認僅進行 Ed25519 驗證/編碼。斯威夫特暴露
  編譯時標誌（`IROHASWIFT_ENABLE_MLDSA`、`IROHASWIFT_ENABLE_GOST`、
  `IROHASWIFT_ENABLE_SM`); Java/Android SDK 需要
  `AccountAddress.configureCurveSupport(...)`； JavaScript SDK 使用
  `configureCurveSupport({ allowMlDsa: true, allowGost: true, allowSm2: true })`。
  secp256k1 支持可用，但在 JS/Android 中默認未啟用
  軟件開發工具包；調用者在發出非 Ed25519 控制器時必須明確選擇加入。
- **主機門控：** `Register<Account>` 拒絕簽名者使用算法的控制器
  節點的 `crypto.allowed_signing` 列表中缺少 **或** 曲線標識符不存在
  `crypto.curves.allowed_curve_ids`，因此集群必須通告支持（配置+
  genesis），然後才能註冊 ML‑DSA/GOST/SM 控制器。 BLS控制器
  編譯時始終允許算法（共識密鑰依賴於它們），
  默認配置啟用 Ed25519 + secp256k1。 【crates/iroha_core/src/smartcontracts/isi/domain.rs:32】

##### 2.3.2 多重簽名控制器指南

`AccountController::Multisig` 通過序列化策略
`crates/iroha_data_model/src/account/controller.rs` 並強制實施架構
記錄在 [`docs/source/references/multisig_policy_schema.md`](source/references/multisig_policy_schema.md) 中。
關鍵實施細節：

- 政策之前經過 `MultisigPolicy::validate()` 標準化和驗證
  被嵌入。閾值必須≥1且≤Σ權重；重複的成員是
  按 `(algorithm || 0x00 || key_bytes)` 排序後確定性刪除。
- 二進制控制器有效負載 (`ControllerPayload::Multisig`) 編碼
  `version:u8`、`threshold:u16`、`member_count:u8`，然後是每個成員的
  `(curve_id, weight:u16, key_len:u16, key_bytes)`。這正是
  `AccountAddress::canonical_bytes()` 寫入 IH58（首選）/sora（第二佳）有效負載。
- 散列 (`MultisigPolicy::digest_blake2b256()`) 使用 Blake2b-256 和
  `iroha-ms-policy` 個性化字符串，以便治理清單可以綁定到
  與 IH58 中嵌入的控制器字節匹配的確定性策略 ID。
- 燈具覆蓋範圍位於 `fixtures/account/address_vectors.json`（案例
  `addr-multisig-*`）。錢包和 SDK 應斷言規範的 IH58 字符串
  下面確認它們的編碼器與 Rust 實現匹配。

|案例編號 |門檻/會員 | IH58 文字（前綴 `0x02F1`）| Sora 壓縮 (`sora`) 文字 |筆記|
|--------|---------------------------------|--------------------------------|----------------------------------------|--------|
| `addr-multisig-council-threshold3` | `≥3` 重量，成員 `(2,1,1)` | `SRfSHsrH3tEmYaaAYyD248F3vfT1oQ3WEGS22MaD8W9bLefF7rsoKLYGcpbcM9EcSus5ZhCAZU7ztn2BCsyeCAdfRncAVmVsipd4ibk6CBLF3Nrzcw8P7VKJg6mtFgEhWVTjfDkUMoc63oeEmaWyV6cyiphwk8ZgKAJUe4TyVtmKm1WWcg7qZ6i` | `sora3vﾑ2zkaoUwﾋﾅGﾘﾚyﾂe3ﾖfﾙヰｶﾘﾉwｷnoWﾛYicaUr3ﾔｲﾖ2Ado3TﾘYQﾉJqﾜﾇｳﾑﾐd8dDjRGｦ3Vﾃ9HcﾀMヰR8ﾎﾖgEqGｵEｾDyc5ﾁ1ﾔﾉ31sUﾑﾀﾖaｸxﾘ3ｲｷMEuFｺｿﾉBQSVQnxﾈeJzrXLヰhｿｹ5SEEﾅPﾂﾗｸdヰﾋ1bUGHｲVXBWNNJ6K` |理事會域治理法定人數。 |
| `addr-multisig-wonderland-threshold2` | `≥2`，成員 `(1,2)` | `3xsmkps1KPBn9dtpE5qHRhHEZCpiAe8d9j6H9A42TV6kc1TpaqdwnSksKgQrsSEHznqvWKBMc1os69BELzkLjsR7EV2gjV14d9JMzo97KEmYoKtxCrFeKFAcy7ffQdboV1uRt` | `sora2ﾖZﾘeｴAdx3ﾂﾉﾔXhnｹﾀ2ﾉｱﾋxﾅﾄﾌヱwﾐmﾊvEﾐCﾏﾎｦ1ﾑHﾋso2GKﾔﾕﾁwﾂﾃP6ﾁｼﾙﾖｺ9ｻｦbﾈ4wFdﾑFヰ3HaﾘｼMｷﾌHWtｷﾋLﾙﾖQ4D3XﾊﾜXmpktﾚｻ5ﾅﾅﾇ1gkﾏsCFQGH9` |雙簽名仙境示例（權重 1 + 2）。 |
| `addr-multisig-default-quorum3` | `≥3`，成員 `(1,1,1,1)` | `nA2bDNhMqXz7ERkHNoEWbvJGyR1aDRsw32LaUWLgbK3vcpzohmdFCLvdotxUWWDY3aZeX4ptLk4Z6TjF5ossnJm8VrNo6daxmGTkqUyP4MxJxiNyPFxsEE5DLnsoLWUcxaWNpZ76tmkbiGS31Gv8tejKpuiHUMaQ1s5ohWyZvDnpycNkBK8AEfGJqn5yc9zAzfWbVhpDwkPj8ScnzvH1Echr5` | `soraﾐ38ﾅｴｸﾜ8ﾃzwBrqﾘｺ4yﾄv6kqJp1ｳｱﾛｿrzﾄﾃﾘﾒRﾗtV9ｼﾔPｽcヱEﾌVVVｼﾘｲZAｦﾓﾅｦeﾒN76vﾈcuｶuﾛL54rzﾙﾏX2zMﾌRLﾃﾋpﾚpｲcHﾑﾅﾃﾔzｵｲVfAﾃﾚﾎﾚCヰﾔｲｽｦw9ﾔﾕ8bGGkﾁ6sNｼaｻRﾖﾜYﾕﾚU18ﾅHヰﾌuMeﾊtﾂrｿj95Ft8ﾜ3fﾄkNiｴuﾈrCﾐQt8ヱｸｸmﾙﾒgUbﾑEKTTCM` |用於基礎治理的隱式默認域仲裁。

#### 2.4 失敗規則（ADDR-1a）

- 短於所需標頭 + 選擇器或具有剩餘字節的有效負載會發出 `AccountAddressError::InvalidLength` 或 `AccountAddressError::UnexpectedTrailingBytes`。
- 設置保留 `ext_flag` 或通告不受支持的版本/類的標頭必須使用 `UnexpectedExtensionFlag`、`InvalidHeaderVersion` 或 `UnknownAddressClass` 拒絕。
- 未知的選擇器/控制器標籤引發 `UnknownDomainTag` 或 `UnknownControllerTag`。
- 過大或畸形的密鑰材料會引發 `KeyPayloadTooLong` 或 `InvalidPublicKey`。
- 超過 255 個成員的多重簽名控制器籌集了 `MultisigMemberOverflow`。
- IME/NFKC 轉換：半角 Sora 假名可以標準化為其全角形式，而不會破壞解碼，但 ASCII `sora` 哨兵和 IH58 數字/字母必須保持 ASCII。全角或大小寫折疊標記表面為 `ERR_MISSING_COMPRESSED_SENTINEL`，全角 ASCII 有效負載引發 `ERR_INVALID_COMPRESSED_CHAR`，校驗和不匹配冒泡為 `ERR_CHECKSUM_MISMATCH`。 `crates/iroha_data_model/src/account/address.rs` 中的屬性測試涵蓋了這些路徑，因此 SDK 和錢包可以依賴確定性故障。
- 當 IH58（首選）/sora（第二佳）輸入在別名回退之前失敗（例如，校驗和不匹配、域摘要不匹配）時，Torii 和 `address@domain` (rejected legacy form) 別名的 SDK 解析現在會發出相同的 `ERR_*` 代碼，因此客戶端可以中繼結構化原因，而無需從散文字符串中猜測。
- 本地選擇器有效負載短於 12 字節表面 `ERR_LOCAL8_DEPRECATED`，保留傳統 Local-8 摘要的硬切換。
- Domainless canonical IH58 literals decode directly to a domainless `AccountId`. Use `ScopedAccountId` only when an interface requires explicit domain context.

#### 2.5 規範二元向量

- **隱式默認域（`default`，種子字節 `0x00`）**  
  規範十六進制：`0x020001203b6a27bcceb6a42d62a3a8d02a6f0d73653215771de243a63ac048a18b59da29`。  
  細分：`0x02` 標頭、`0x00` 選擇器（隱式默認值）、`0x00` 控制器標籤、`0x01` 曲線 id (Ed25519)、`0x20` 密鑰長度，後跟 32 字節密鑰負載。
- **本地域摘要（`treasury`，種子字節 `0x01`）**  
  規範十六進制：`0x0201b18fe9c1abbac45b3e38fc5d0001208a88e3dd7409f195fd52db2d3cba5d72ca6709bf1d94121bf3748801b40f6f5c`。  
  細分：`0x02` 標頭、選擇器標籤 `0x01` 加上摘要 `b1 8f e9 c1 ab ba c4 5b 3e 38 fc 5d`，後跟單密鑰有效負載（`0x00` 標籤、`0x01` 曲線 id、`0x20` 長度、32 字節 Ed25519鍵）。單元測試 (`account::address::tests::parse_encoded_accepts_all_formats`) 通過 `AccountAddress::parse_encoded` 斷言下面的 V1 向量，保證工具可以依賴十六進制、IH58（首選）和壓縮（`sora`，第二佳）形式的規範有效負載。使用 `cargo run -p iroha_data_model --example address_vectors` 重新生成擴展夾具組。

|域名 |種子字節 |規範六角 |壓縮 (`sora`) |
|------------------------|----------|--------------------------------------------------------------------------------------------------------|------------|
|默認 | `0x00` | `0x020001203b6a27bcceb6a42d62a3a8d02a6f0d73653215771de243a63ac048a18b59da29` | `sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE` |
|國庫| `0x01` | `0x0201b18fe9c1abbac45b3e38fc5d0001208a88e3dd7409f195fd52db2d3cba5d72ca6709bf1d94121bf3748801b40f6f5c` | `sora5ｻu6rﾀCヰTGwﾏ1ﾅヱﾌQｲﾖﾇqCｦヰﾓZQCZRDSSﾅMｱﾙヱｹﾁｸ8ｾeﾄﾛ6C8bZuwﾗｹCZｦRSLQFU` |
|仙境| `0x02` | `0x0201b8ae571b79c5a80f5834da2b0001208139770ea87d175f56a35466c34c7ecccb8d8a91b4ee37a25df60f5b8fc9b394` | `sora5ｻwﾓyRｿqﾏnMﾀﾙヰKoﾒﾇﾓQｺﾛyｼ3ｸFHB2F5LyPﾐTMZkｹｼw67ﾋVﾕｻr8ﾉGﾇeEnｻVRNKCS` |
|伊呂波 | `0x03` | `0x0201de8b36819700c807083608e2000120ed4928c628d1c2c6eae90338905995612959273a5c63f93636c14614ac8737d1` | `sora5ｻﾜxﾀ7Vｱ7QFeｷMﾂLﾉﾃﾏﾓﾀTﾚgSav3Wnｱｵ4ｱCKｷﾛMﾘzヰHiﾐｱ6ﾃﾉﾁﾐZmﾇ2fiﾎX21P4L` |
|阿爾法 | `0x04` | `0x020146be2154ae86826a3fef0ec0000120ca93ac1705187071d67b83c7ff0efe8108e8ec4530575d7726879333dbdabe7c` | `sora5ｻ9JヱﾈｿuwU6ｴpﾔﾂﾈRqRTds1HﾃﾐｶLVﾍｳ9ﾔhｾNｵVｷyucEﾒGﾈﾏﾍ9sKeﾉDzrｷﾆ742WG1` |
|歐米茄| `0x05` | `0x0201390d946885bc8416b3d30c9d0001206e7a1cdd29b0b78fd13af4c5598feff4ef2a97166e3ca6f2e4fbfccd80505bf1` | `sora5ｻ3zrﾌuﾚﾄJﾑXQhｸTyN8pzwRkWxmjVﾗbﾚﾕヰﾈoｽｦｶtEEﾊﾐ6GPｿﾓﾊｾEhvPｾｻ3XAJ73F` |
|治理| `0x06` | `0x0201989eb45a80940d187e2c908f0001208a875fff1eb38451577acd5afee405456568dd7c89e090863a0557bc7af49f17` | `sora5ｻiｵﾁyVﾕｽbFpDHHuﾇﾉdﾗｲﾓﾄRﾋAW3frUCｾ5ｷﾘTwdﾚnｽtQiLﾏｼｶﾅXgｾZmﾒヱH58H4KP` |
|驗證者 | `0x07` | `0x0201e4ffa58704c69afaeb7cc2d7000120ea4a6c63e29c520abef5507b132ec5f9954776aebebe7b92421eea691446d22c` | `sora5ｻﾀLDH6VYﾑNAｾgﾉVﾜtxﾊRXLｹﾍﾔﾌLd93GﾔGeｴﾄYrs1ﾂHｸkYxｹwｿyZﾗxyﾎZoXT1S4N` |
|探險家| `0x08` | `0x02013b35422c65c2a83c99c523ad0001201398f62c6d1a457c51ba6a4b5f3dbd2f69fca93216218dc8997e416bd17d93ca` | `sora5ｻ4nmｻaﾚﾚPvNLgｿｱv6MHDeEyﾀovﾉJcpvrﾖ6ﾈCQcCNﾇﾜhﾚﾖyFdTwｸｶHEｱ9rWU8FMB` |
|索拉內特| `0x09` | `0x0201047d9ea7f5d5dbec3f7bfc58000120fd1724385aa0c75b64fb78cd602fa1d991fdebf76b13c58ed702eac835e9f618` | `sora5ｱｸヱVQﾂcﾁヱRﾓcApｲﾁﾅﾒvﾌﾏfｾNnﾛRJsｿDhﾙuHaﾚｺｦﾌﾍﾈeﾆﾎｺN1UUDｶ6ﾎﾄﾛoRH8JUL` |
|狐狸 | `0x0A` | `0x0201e91933de397fd7723dc9a76c00012043a72e714401762df66b68c26dfbdf2682aaec9f2474eca4613e424a0fbafd3c` | `sora5ｻﾚｺヱkfFJfSﾁｼJwﾉLvbpSｷﾔMWFMrbｳｸｲｲyヰKGJﾉｻ4ｹﾕrｽhｺｽzSDヰXAN62AD7RGNS` |
|達| `0x0B` | `0x02016838cf5bb0ce0f3d4f380e1c00012066be7e332c7a453332bd9d0a7f7db055f5c5ef1a06ada66d98b39fb6810c473a` | `sora5ｻNﾒ5SﾐRﾉﾐﾃ62ｿ1ｶｷWFKyF1BcAﾔvｼﾐHqﾙﾐPﾏｴヰ5tｲﾕvnﾙT6ﾀW7mﾔ7ﾇﾗﾂｳ25CXS93` |

審核者：數據模型工作組、密碼學工作組 — ADDR-1a 範圍已批准。

##### Sora Nexus 參考別名

Sora Nexus 網絡默認為 `chain_discriminant = 0x02F1`
（`iroha_config::parameters::defaults::common::CHAIN_DISCRIMINANT`）。的
因此，`AccountAddress::to_ih58` 和 `to_compressed_sora` 助手會發出
每個規範有效負載的一致文本形式。精選賽程來自
`fixtures/account/address_vectors.json`（通過生成
`cargo xtask address-vectors`）如下所示以供快速參考：

|帳戶/選擇器 | IH58 文字（前綴 `0x02F1`）| Sora 壓縮 (`sora`) 文字 |
|--------------------------------|--------------------------------|-------------------------|
| `default` 域（隱式選擇器，種子 `0x00`）| `6cmzPVPX5jDQFNfiz6KgmVfm1fhoAqjPhoPFn4nx9mBWaFMyUCwq4cw` | `sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE`（提供顯式路由提示時可選 `@default` 後綴）|
| `treasury`（本地摘要選擇器，種子 `0x01`）| `34mSYnCXkCzHXm31UDHh7SJfGvC4QPEhwim8z7sys2iHqXpCwCQkjL8KHvkFLSs1vZdJcb37r` | `sora5ｻu6rﾀCヰTGwﾏ1ﾅヱﾌQｲﾖﾇqCｦヰﾓZQCZRDSSﾅMｱﾙヱｹﾁｸ8ｾeﾄﾛ6C8bZuwﾗｹCZｦRSLQFU` |
|全局註冊表指針（`registry_id = 0x0000_002A`，相當於 `treasury`）| `3oE9sLeRGP49Cu7mQ1nF4wtKAm29BG4TGLiRsaXe7mhbMP5WZ113nNW1N6RbqF` | `sorakXｹ6NｻﾍﾀﾖSﾜﾖｱ3ﾚ5WﾘﾋQﾅｷｦxgﾛｸcﾁｵﾋkﾋvﾏ8SPﾓﾀｹdｴｴｲW9iCM6AEP` |

這些字符串與 CLI (`iroha tools address convert`)、Torii 發出的字符串匹配
響應 (`address_format=ih58|compressed`) 和 SDK 幫助程序，因此 UX 複製/粘貼
流量可以逐字依賴它們。僅當您需要顯式路由提示時才附加 `<address>@<domain>` (rejected legacy form)；後綴不是規範輸出的一部分。

#### 2.6 用於互操作性的文本別名（計劃）

- **鏈別名樣式：** `ih:<chain-alias>:<alias@domain>` 用於日誌和人類
  條目。錢包必須解析前綴，驗證嵌入的鏈，並阻止
  不匹配。
- **CAIP-10 形式：** `iroha:<caip-2-id>:<ih58-addr>` 與鏈無關
  集成。此映射在發貨中**尚未實現**
  工具鏈。
- **機器助手：** 發布 Rust、TypeScript/JavaScript、Python 的編解碼器
  和 Kotlin 涵蓋 IH58 和壓縮格式（`AccountAddress::to_ih58`，
  `AccountAddress::parse_encoded` 及其 SDK 等效項）。 CAIP-10 助手是
  未來的工作。

#### 2.7 確定性 IH58 別名

- **前綴映射：** 重新使用 `chain_discriminant` 作為 IH58 網絡前綴。
  `encode_ih58_prefix()`（參見 `crates/iroha_data_model/src/account/address.rs`）
  為值 `<64` 發出 6 位前綴（單字節）和 14 位兩字節
  更大網絡的形式。權威作業在
  [`address_prefix_registry.md`](source/references/address_prefix_registry.md);
  SDK 必須保持匹配的 JSON 註冊表同步以避免衝突。
- **帳戶材料：** IH58 編碼由以下構建的規範有效負載
  `AccountAddress::canonical_bytes()`—標頭字節、域選擇器和
  控制器有效負載。沒有額外的哈希步驟； IH58 嵌入
  Rust 生成的二進制控制器有效負載（單密鑰或多簽名）
  編碼器，而不是用於多重簽名策略摘要的 CTAP2 映射。
- **編碼：** `encode_ih58()` 將前綴字節與規範連接起來
  有效負載並附加從 Blake2b-512 派生的 16 位校驗和，其中固定
  前綴 `IH58PRE` (`b"IH58PRE" || prefix || payload`)。結果通過 `bs58` 進行 Base58 編碼。
  CLI/SDK 幫助程序公開相同的過程，並且 `AccountAddress::parse_encoded`
  通過 `decode_ih58` 反轉它。

#### 2.8 規範文本測試向量

`fixtures/account/address_vectors.json` 包含完整的 IH58（首選）和壓縮的（`sora`，第二好的）
每個規範有效負載的文字。亮點：

- **`addr-single-default-ed25519`（Sora Nexus，前綴 `0x02F1`）。 **  
  IH58 `6cmzPVPX5jDQFNfiz6KgmVfm1fhoAqjPhoPFn4nx9mBWaFMyUCwq4cw`，壓縮 (`sora`)
  `sora2QG…U4N5E5`。 Torii 從 `AccountId` 發出這些確切的字符串
  `Display` 實現（規範 IH58）和 `AccountAddress::to_compressed_sora`。
- **`addr-global-registry-002a`（註冊表選擇器→財務）。 **  
  IH58 `3oE9sLeRGP49Cu7mQ1nF4wtKAm29BG4TGLiRsaXe7mhbMP5WZ113nNW1N6RbqF`，壓縮 (`sora`)
  `sorakX…CM6AEP`。證明註冊表選擇器仍然解碼為
  與相應的本地摘要相同的規範有效負載。
- **失敗案例（`ih58-prefix-mismatch`）。 **  
  在節點上解析使用前綴 `NETWORK_PREFIX + 1` 編碼的 IH58 文字
  期望默認前綴產生
  `AccountAddressError::UnexpectedNetworkPrefix { expected: 753, found: 754 }`
  在嘗試域路由之前。 `ih58-checksum-mismatch` 夾具
  對 Blake2b 校驗和進行篡改檢測。

#### 2.9 合規裝置

ADDR-2 提供了一個可重玩的夾具包，涵蓋正面和負面
跨規範十六進制、IH58（首選）、壓縮（`sora`、半角/全角）、隱式的場景
默認選擇器、全局註冊表別名和多重簽名控制器。的
規範的 JSON 存在於 `fixtures/account/address_vectors.json` 中，並且可以
重新生成：

```
cargo xtask address-vectors --out fixtures/account/address_vectors.json
# verify without writing:
cargo xtask address-vectors --verify
```

對於臨時實驗（不同的路徑/格式），示例二進製文件仍然是
可用：

```
cargo run -p iroha_data_model --example account_address_vectors > fixtures/account/address_vectors.json
```

`crates/iroha_data_model/tests/account_address_vectors.rs` 中的 Rust 單元測試
和 `crates/iroha_torii/tests/account_address_vectors.rs` 以及 JS，
Swift 和 Android 線束 (`javascript/iroha_js/test/address.test.js`,
`IrohaSwift/Tests/IrohaSwiftTests/AccountAddressTests.swift`，
`java/iroha_android/src/test/java/org/hyperledger/iroha/android/address/AccountAddressTests.java`),
使用相同的固定裝置來保證 SDK 和 Torii 准入之間的編解碼器奇偶性。

### 3. 全球唯一的域名和標準化

另請參閱：[`docs/source/references/address_norm_v1.md`](source/references/address_norm_v1.md)
適用於 Torii、數據模型和 SDK 中使用的規範 Norm v1 管道。

將 `DomainId` 重新定義為標記元組：

```
DomainId {
    name: Name,
    authority: GlobalDomainAuthority, // new enum
}

enum GlobalDomainAuthority {
    LocalChain,                  // default for the local chain
    External { chain_discriminant: u16 },
}
```

`LocalChain` 包裝當前鏈管理的域的現有名稱。
當通過全局註冊表註冊域時，我們會保留所有權
鏈的判別式。顯示/解析目前保持不變，但是
擴展結構允許路由決策。

#### 3.1 標準化和欺騙防禦

Norm v1 定義了每個組件在域之前必須使用的規範管道
名稱被保留或嵌入到 `AccountAddress` 中。完整的演練
居住在 [`docs/source/references/address_norm_v1.md`](source/references/address_norm_v1.md)；
下面的摘要記錄了錢包、Torii、SDK 和治理的步驟
工具必須實施。

1. **輸入驗證。 ** 拒絕空字符串、空格和保留
   分隔符 `@`、`#`、`$`。這與強制執行的不變量相匹配
   `Name::validate_str`。
2. **Unicode NFC 組成。 ** 規範地應用 ICU 支持的 NFC 標準化
   等效序列確定性地崩潰（例如，`e\u{0301}` → `é`）。
3. **UTS-46 標準化。 ** 通過 UTS-46 運行 NFC 輸出
   `use_std3_ascii_rules = true`、`transitional_processing = false` 和
   已啟用 DNS 長度強制。結果是小寫的 A 標籤序列；
   違反 STD3 規則的輸入在此失敗。
4. **長度限制。 ** 強制執行 DNS 樣式限制：每個標籤必須為 1–63
   字節，並且在步驟 3 之後完整域不得超過 255 字節。
5. **可選的易混淆策略。 ** UTS-39 腳本檢查被跟踪
   規範 v2；操作員可以提前啟用它們，但如果檢查失敗則必須中止
   處理。

如果每個階段都成功，則小寫 A 標籤字符串將被緩存並用於
地址編碼、配置、清單和註冊表查找。本地文摘
選擇器將其 12 字節值派生為 `blake2s_mac(key = "SORA-LOCAL-K:v1",
canonical_label)[0..12]` 使用步驟 3 的輸出。所有其他嘗試（混合
大小寫、大寫、原始 Unicode 輸入）會被結構化拒絕
`ParseError`s 位於提供名稱的邊界處。

展示這些規則的規範裝置 - 包括 punycode 往返
和無效的 STD3 序列 — 列於
`docs/source/references/address_norm_v1.md` 並鏡像在 SDK CI 中
在 ADDR-2 下跟踪的向量套件。

### 4. Nexus 域名註冊和路由- **註冊表架構：** Nexus 維護簽名映射 `DomainName -> ChainRecord`
  其中 `ChainRecord` 包括鏈判別式、可選元數據（RPC
  端點）和權威證明（例如，治理多重簽名）。
- **同步機制：**
  - 鏈向 Nexus 提交簽名的域名聲明（在創世期間或通過
    治理指令）。
  - Nexus 發布定期清單（簽名的 JSON 加上可選的 Merkle 根）
    通過 HTTPS 和內容尋址存儲（例如 IPFS）。客戶固定
    最新清單並驗證簽名。
- **查找流程：**
  - Torii 接收引用 `DomainId` 的交易。
  - 如果本地未知域，則 Torii 查詢緩存的 Nexus 清單。
  - 如果清單指示外部鏈，則交易將被拒絕
    確定性 `ForeignDomain` 錯誤和遠程鏈信息。
  - 如果 Nexus 中缺少域，則 Torii 返回 `UnknownDomain`。
- **信任錨和輪換：** 治理關鍵標誌清單；旋轉或
  撤銷作為新的清單條目發布。客戶執行清單
  TTL（例如 24 小時）並拒絕查閱超出該窗口的陳舊數據。
- **失敗模式：** 如果清單檢索失敗，Torii 會回退到緩存
  TTL內的數據；過了 TTL，它會發出 `RegistryUnavailable` 並拒絕
  跨域路由以避免狀態不一致。

### 4.1 註冊表不變性、別名和邏輯刪除 (ADDR-7c)

Nexus 發布 **僅附加清單**，以便每個域或別名分配
可以審核和重播。運營商必須處理中描述的捆綁包
[地址清單 Runbook](source/runbooks/address_manifest_ops.md) 作為
唯一事實來源：如果清單丟失或驗證失敗，Torii 必須
拒絕解析受影響的域。

自動化支持：`cargo xtask address-manifest verify --bundle <current_dir> --previous <previous_dir>`
重放校驗和、模式和先前摘要檢查
運行手冊。將命令輸出包含在變更單中以顯示 `sequence`
在發布捆綁包之前驗證了 `previous_digest` 鏈接。

#### 清單標頭和簽名合同

|領域|要求 |
|--------|-------------|
| `version` |目前為 `1`。僅通過匹配的規格更新進行碰撞。 |
| `sequence` |每個出版物**精確**增加一。 Torii 緩存拒絕有間隙或回歸的修訂。 |
| `generated_ms` + `ttl_hours` |建立緩存新鮮度（默認 24 小時）。如果 TTL 在下一次發布之前到期，則 Torii 會翻轉為 `RegistryUnavailable`。 |
| `previous_digest` |先前清單正文的 BLAKE3 摘要（十六進制）。驗證者使用 `b3sum` 重新計算它以證明不變性。 |
| `signatures` |清單通過 Sigstore (`cosign sign-blob`) 進行簽名。運營人員必須運行 `cosign verify-blob --bundle manifest.sigstore manifest.json` 並在推出之前強制執行治理身份/發行者約束。 |

發布自動化發出 `manifest.sigstore` 和 `checksums.sha256`
與 JSON 主體一起。鏡像到 SoraFS 或時將文件放在一起
HTTP 端點，以便審核員可以逐字重播驗證步驟。

#### 條目類型

|類型 |目的|必填字段 |
|------|---------|-----------------|
| `global_domain` |聲明域已全局註冊，並且應映射到鏈判別式和 IH58 前綴。 | `{ "domain": "<label>", "chain": "sora:nexus:global", "ih58_prefix": 753, "selector": "global" }` |
| `tombstone` |永久停用別名/選擇器。擦除 Local-8 摘要或刪除域時需要。 | `{ "selector": {…}, "reason_code": "LOCAL8_RETIREMENT" \| …, "ticket": "<governance id>", "replaces_sequence": <number> }` |

`global_domain` 條目可以選擇包含 `manifest_url` 或 `sorafs_cid`
將錢包指向簽名的鏈元數據，但規范元組仍然存在
`{domain, chain, discriminant/ih58_prefix}`。 `tombstone` 記錄**必須**引用
已退休的選擇器和授權的票證/治理工件
進行更改，以便可以離線重建審計跟踪。

#### 別名/邏輯刪除工作流程和遙測

1. **檢測漂移。 ** 使用 `torii_address_local8_total{endpoint}`，
   `torii_address_local8_domain_total{endpoint,domain}`，
   `torii_address_collision_total{endpoint,kind="local12_digest"}`，
   `torii_address_collision_domain_total{endpoint,domain}`，
   `torii_address_domain_total{endpoint,domain_kind}`，和
   `torii_address_invalid_total{endpoint,reason}`（呈現於
   `dashboards/grafana/address_ingest.json`）以確認本地提交和
   在提出墓碑之前，Local-12 碰撞保持為零。的
   每個域計數器讓所有者證明只有開發/測試域發出 Local-8
   流量（並且 Local-12 衝突映射到已知的暫存域），同時
   包括 **Domain Kind Mix (5m)** 面板，以便 SRE 可以繪製出有多少
   `domain_kind="local12"` 流量保持不變，並且 `AddressLocal12Traffic`
   每當生產環境仍然看到 Local-12 選擇器時，就會觸發警報，儘管
   退休門。
2. **導出規範摘要。 ** 運行
   `iroha tools address convert <address> --format json --expect-prefix 753`
   （或通過消耗 `fixtures/account/address_vectors.json`
   `scripts/account_fixture_helper.py`) 捕獲准確的 `digest_hex`。
   CLI 接受 IH58、`sora…` 和規範 `0x…` 文字；追加
   `@<domain>` 僅當您需要保留清單標籤時。
   JSON 摘要通過 `input_domain` 字段顯示該域，並且
   `legacy  suffix` 將轉換後的編碼重播為 `<address>@<domain>` (rejected legacy form)
   清單差異（此後綴是元數據，而不是規範帳戶 ID）。
   對於面向換行的導出使用
   `iroha tools address normalize --input <file> legacy-selector input mode` 批量轉換本地
   選擇器轉換成規範的 IH58（首選）、壓縮的（`sora`，第二好的）、十六進製或 JSON 形式，同時跳過
   非本地行。當審計員需要電子表格友好的證據時，運行
   `iroha tools address audit --input <file> --format csv` 發出 CSV 摘要
   (`input,status,format,domain_kind,…`) 突出顯示本地選擇器，
   規範編碼和同一文件中的解析失敗。
3. **附加清單條目。 ** 起草 `tombstone` 記錄（以及後續
   `global_domain` 遷移到全局註冊表時記錄）並驗證
   在請求籤名之前使用 `cargo xtask address-vectors` 的清單。
4. **驗證並發布。 ** 遵循運行手冊清單（哈希值、Sigstore、
   序列單調性），然後將包鏡像到 SoraFS。現在 Torii
   捆綁包落地後立即規範化 IH58（首選）/sora（第二佳）文字。
5. **監控和回滾。 ** 將 Local-8 和 Local-12 碰撞面板保持在
   30天歸零；如果出現回歸，請重新發布之前的清單
   僅在受影響的非生產環境中，直到遙測穩定為止。

上述所有步驟都是 ADDR-7c 的強制性證據：清單中沒有
`cosign` 簽名包或沒有匹配的 `previous_digest` 值必須
自動拒絕，操作員必須附上驗證日誌
他們的改簽機票。

### 5.錢包和API人體工程學

- **顯示默認值：** 錢包顯示 IH58 地址（短，校驗和）
  加上從註冊表中獲取的解析域作為標籤。域名是
  明確標記為可能更改的描述性元數據，而 IH58 是
  穩定的地址。
- **輸入規範化：** Torii 和 SDK 接受 IH58（首選）/sora（次佳）/0x
  地址加上 `alias@domain` (rejected legacy form)、`uaid:…` 和
  `opaque:…` 形式，然後規範化為 IH58 進行輸出。沒有
  嚴格模式切換；原始電話/電子郵件標識符必須保留在賬本之外
  通過 UAID/不透明映射。
- **錯誤預防：** 錢包解析 IH58 前綴並強制鏈判別
  期望。鏈不匹配會通過可操作的診斷觸發硬故障。
- **編解碼器庫：** 官方 Rust、TypeScript/JavaScript、Python 和 Kotlin
  庫提供 IH58 編碼/解碼以及壓縮 (`sora`) 支持
  避免碎片化的實施。 CAIP-10 轉換尚未發貨。

#### 輔助功能和安全共享指南- 實時跟踪產品表面的實施指南
  `docs/portal/docs/reference/address-safety.md`；參考該清單時
  使這些要求適應錢包或瀏覽器用戶體驗。
- **安全共享流程：** 複製或顯示地址的表面默認為 IH58 形式，並公開相鄰的“共享”操作，該操作顯示完整的字符串和從同一負載派生的 QR 碼，以便用戶可以通過視覺或掃描來驗證校驗和。當截斷不可避免時（例如，小屏幕），請保留字符串的開頭和結尾，添加清晰的省略號，並保持可通過複製到剪貼板訪問完整地址，以防止意外剪輯。
- **IME 保護措施：** 地址輸入必須拒絕來自 IME/IME 風格鍵盤的組合偽影。強制僅輸入 ASCII，在檢測到全角或假名字符時顯示內聯警告，並提供純文本粘貼區域，在驗證之前剝離組合標記，以便日語和中文用戶可以禁用其 IME，而不會丟失進度。
- **屏幕閱讀器支持：** 提供可視化隱藏標籤 (`aria-label`/`aria-describedby`)，用於描述前導 Base58 前綴數字並將 IH58 有效負載分為 4 或 8 個字符組，因此輔助技術讀取分組字符而不是連續字符串。通過禮貌的實時區域宣布複製/共享成功，並確保二維碼預覽包含描述性替代文本（“鏈 0x02F1 上 <alias> 的 IH58 地址”）。
- **僅 Sora 壓縮使用：** 始終將 `sora…` 壓縮視圖標記為“僅 Sora”，並在復制之前將其置於顯式確認後面。當鏈判別式不是 Sora Nexus 值時，SDK 和錢包必須拒絕顯示壓縮輸出，並應引導用戶返回 IH58 進行網絡間轉賬，以避免資金錯誤路由。

## 實施清單

- **IH58 信封：** 前綴使用緊湊型編碼 `chain_discriminant`
  來自 `encode_ih58_prefix()` 的 6 位/14 位方案，主體是規範字節
  (`AccountAddress::canonical_bytes()`)，校驗和是前兩個字節
  Blake2b-512（`b"IH58PRE"` || 前綴 || 主體）。完整的有效負載是Base58-
  通過 `bs58` 編碼。
- **註冊合約：** 簽名 JSON（和可選的 Merkle 根）發布
  `{discriminant, ih58_prefix, chain_alias, endpoints}` 帶 24 小時 TTL 和
  旋轉鍵。
- **域名政策：** ASCII `Name` 今天；如果啟用 i18n，請申請 UTS-46
  標準化和 UTS-39 用於易混淆的檢查。強制執行最大標籤 (63) 和
  總長度 (255)。
- **文本助手：** 在 Rust 中提供 IH58 ↔ 壓縮 (`sora…`) 編解碼器，
  具有共享測試向量的 TypeScript/JavaScript、Python 和 Kotlin (CAIP-10
  映射仍然是未來的工作）。
- **CLI 工具：** 通過 `iroha tools address convert` 提供確定性操作員工作流程
  （參見 `crates/iroha_cli/src/address.rs`），它接受 IH58/`sora…`/`0x…` 文字和
  可選的 `<address>@<domain>` (rejected legacy form) 標籤，默認為使用 Sora Nexus 前綴 (`753`) 的 IH58 輸出，
  並且僅在操作員明確請求時才發出僅 Sora 的壓縮字母表
  `--format compressed` 或 JSON 摘要模式。該命令強制執行前綴期望
  解析，記錄提供的域（JSON 中的 `input_domain`）和 `legacy  suffix` 標誌
  將轉換後的編碼重播為 `<address>@<domain>` (rejected legacy form)，因此明顯的差異仍然符合人體工程學。
- **錢包/瀏覽器用戶體驗：**遵循[地址顯示準則](source/sns/address_display_guidelines.md)
  附帶 ADDR-6 — 提供雙複製按鈕，將 IH58 保留為 QR 有效負載，並發出警告
  用戶認為壓縮的 `sora…` 形式僅適用於 Sora，並且容易受到 IME 重寫。
- **Torii 集成：** 緩存 Nexus 體現尊重 TTL，發出
  `ForeignDomain`/`UnknownDomain`/`RegistryUnavailable` 確定性地，並且
  keep account-literal parsing encoded-only (`IH58` preferred, `sora…`
  compressed accepted) with canonical IH58 output.

### Torii 響應格式

- `GET /v1/accounts` 接受可選的 `address_format` 查詢參數並且
  `POST /v1/accounts/query` 接受 JSON 信封內的相同字段。
  支持的值為：
  - `ih58`（默認）——響應發出規範的 IH58 Base58 有效負載（例如，
    `6cmzPVPX5jDQFNfiz6KgmVfm1fhoAqjPhoPFn4nx9mBWaFMyUCwq4cw`）。
  - `compressed` — 響應發出僅 Sora 的 `sora…` 壓縮視圖，同時
    保持過濾器/路徑參數規範。
- 無效值返回 `400` (`QueryExecutionFail::Conversion`)。這允許
  錢包和瀏覽器請求壓縮字符串以獲得僅限 Sora 的 UX，同時
  將 IH58 保留為可互操作的默認值。
- 資產持有者列表 (`GET /v1/assets/{definition_id}/holders`) 及其 JSON
  對應的信封 (`POST …/holders/query`) 也兌現 `address_format`。
  每當 `items[*].account_id` 字段發出壓縮文字
  參數/信封字段設置為 `compressed`，鏡像帳戶
  端點，以便瀏覽器可以跨目錄呈現一致的輸出。
- **測試：** 添加編碼器/解碼器往返、錯誤鏈的單元測試
  失敗和明顯的查找；在 Torii 和 SDK 中添加集成覆蓋範圍
  對於 IH58 流端到端。

## 錯誤代碼註冊表

地址編碼器和解碼器通過以下方式暴露故障
`AccountAddressError::code_str()`。下表提供了穩定的代碼
SDK、錢包和 Torii 表面應與人類可讀的表面一起顯示
消息以及建議的修復指南。

### 規範構造

|代碼|失敗|建議的補救措施|
|------|---------|------------------------|
| `ERR_UNSUPPORTED_ALGORITHM` |編碼器收到註冊表或構建功能不支持的簽名算法。 |將帳戶構建限制為註冊表和配置中啟用的曲線。 |
| `ERR_KEY_PAYLOAD_TOO_LONG` |簽名密鑰有效負載長度超出支持的限制。 |單鍵控制器僅限於 `u8` 長度；對大型公鑰使用多重簽名（例如 ML-DSA）。 |
| `ERR_INVALID_HEADER_VERSION` |地址標頭版本超出支持範圍。 |為 V1 地址發出標頭版本 `0`；在採用新版本之前升級編碼器。 |
| `ERR_INVALID_NORM_VERSION` |無法識別標準化版本標誌。 |使用規範化版本 `1` 並避免切換保留位。 |
| `ERR_INVALID_IH58_PREFIX` |無法對請求的 IH58 網絡前綴進行編碼。 |在鏈註冊表中發布的包含 `0..=16383` 範圍內選擇一個前綴。 |
| `ERR_CANONICAL_HASH_FAILURE` |規範有效負載哈希失敗。 |重試操作；如果錯誤仍然存​​在，請將其視為哈希堆棧中的內部錯誤。 |

### 格式解碼和自動檢測

|代碼|失敗|建議的補救措施|
|------|---------|------------------------|
| `ERR_INVALID_IH58_ENCODING` | IH58 字符串包含字母表之外的字符。 |確保地址使用已發布的 IH58 字母表，並且在復制/粘貼過程中未被截斷。 |
| `ERR_INVALID_LENGTH` |有效負載長度與選擇器/控制器的預期規範大小不匹配。 |為選定的域選擇器和控制器佈局提供完整的規範有效負載。 |
| `ERR_CHECKSUM_MISMATCH` | IH58（首選）或壓縮（`sora`，第二好的）校驗和驗證失敗。 |從可信來源重新生成地址；這通常表示複製/粘貼錯誤。 |
| `ERR_INVALID_IH58_PREFIX_ENCODING` | IH58 前綴字節格式錯誤。 |使用兼容的編碼器重新編碼地址；不要手動更改前導 Base58 字節。 |
| `ERR_INVALID_HEX_ADDRESS` |規範的十六進制形式無法解碼。 |提供由官方編碼器生成的以 `0x` 為前綴、偶數長度的十六進製字符串。 |
| `ERR_MISSING_COMPRESSED_SENTINEL` |壓縮形式不以 `sora` 開頭。 |在將壓縮的 Sora 地址交給解碼器之前，使用所需的哨兵作為前綴。 |
| `ERR_COMPRESSED_TOO_SHORT` |壓縮字符串缺少足夠的有效負載和校驗和數字。 |使用編碼器發出的完整壓縮字符串而不是截斷的片段。 |
| `ERR_INVALID_COMPRESSED_CHAR` |遇到壓縮字母表之外的字符。 |將字符替換為已發布的半角/全角表中的有效 Base-105 字形。 |
| `ERR_INVALID_COMPRESSED_BASE` |編碼器嘗試使用不受支持的基數。 |針對編碼器提交錯誤； V1 中壓縮字母表固定為基數 105。 |
| `ERR_INVALID_COMPRESSED_DIGIT` |數字值超過壓縮字母大小。 |確保每個數字都在 `0..105)` 範圍內，如有必要，請重新生成地址。 |
| `ERR_UNSUPPORTED_ADDRESS_FORMAT` |自動檢測無法識別輸入格式。 |調用解析器時提供 IH58（首選）、壓縮的 (`sora`) 或規範的 `0x` 十六進製字符串。 |

### 域和網絡驗證|代碼|失敗|建議的補救措施|
|------|---------|------------------------|
| `ERR_DOMAIN_MISMATCH` |域選擇器與預期域不匹配。 |使用為目標域頒發的地址或更新期望。 |
| `ERR_INVALID_DOMAIN_LABEL` |域標籤未通過規範化檢查。 |編碼前使用 UTS-46 非過渡處理規範化域。 |
| `ERR_UNEXPECTED_NETWORK_PREFIX` |解碼後的 IH58 網絡前綴與配置的值不同。 |切換到目標鏈中的地址或調整預期的判別式/前綴。 |
| `ERR_UNKNOWN_ADDRESS_CLASS` |地址類別位無法識別。 |將解碼器升級到可以理解新類的版本，或避免篡改標頭位。 |
| `ERR_UNKNOWN_DOMAIN_TAG` |域選擇器標籤未知。 |更新到支持新選擇器類型的版本，或避免在 V1 節點上使用實驗性負載。 |
| `ERR_UNEXPECTED_EXTENSION_FLAG` |保留擴展位已設置。 |清除保留位；它們將保持封閉狀態，直到未來的 ABI 引入它們。 |
| `ERR_UNKNOWN_CONTROLLER_TAG` |無法識別控制器有效負載標籤。 |升級解碼器以在解析新控制器類型之前識別它們。 |
| `ERR_UNEXPECTED_TRAILING_BYTES` |規範有效負載包含解碼後的尾隨字節。 |重新生成規範有效負載；僅應顯示記錄的長度。 |

### 控制器有效負載驗證

|代碼|失敗|建議的補救措施|
|------|---------|------------------------|
| `ERR_INVALID_PUBLIC_KEY` |關鍵字節與聲明的曲線不匹配。 |確保關鍵字節完全按照所選曲線的要求進行編碼（例如，32 字節 Ed25519）。 |
| `ERR_UNKNOWN_CURVE` |曲線標識符未註冊。 |使用曲線 ID `1` (Ed25519)，直到其他曲線獲得批准並在註冊表中發布。 |
| `ERR_MULTISIG_MEMBER_OVERFLOW` |多重簽名控制器聲明的成員數量超出了支持的數量。 |在編碼之前將多重簽名成員資格減少到記錄的限制。 |
| `ERR_INVALID_MULTISIG_POLICY` |多重簽名策略有效負載驗證失敗（閾值/權重/架構）。 |重建策略，使其滿足 CTAP2 架構、權重邊界和閾值約束。 |

## 考慮的替代方案

- **純 Base58Check（比特幣風格）。 ** 校驗和更簡單，但錯誤檢測更弱
  比 Blake2b 派生的 IH58 校驗和（`encode_ih58` 截斷 512 位哈希值）
  並且缺乏 16 位判別式的顯式前綴語義。
- **在域字符串中嵌入鏈名稱（例如，`finance@chain`）。 ** 中斷
- **僅依靠 Nexus 路由而不更改地址。 **用戶仍然會
  複製/粘貼不明確的字符串；我們希望地址本身攜帶上下文。
- **Bech32m 信封。 ** QR 友好並提供人類可讀的前綴，但是
  將偏離運輸 IH58 實施 (`AccountAddress::to_ih58`)
  並需要重新創建所有裝置/SDK。目前的路線圖保持 IH58+
  壓縮（`sora`）支持，同時繼續研究未來
  Bech32m/QR 層（CAIP-10 映射被推遲）。

## 開放問題

- 確認`u16`判別式加上預留範圍覆蓋長期需求；
  否則使用 varint 編碼評估 `u32`。
- 完成註冊表更新的多重簽名治理流程以及如何實施
  處理撤銷/過期分配。
- 定義確切的清單簽名方案（例如，Ed25519 多重簽名）以及
  Nexus 分發的傳輸安全（HTTPS 固定、IPFS 哈希格式）。
- 確定是否支持遷移的域別名/重定向以及如何支持
  在不打破決定論的情況下將它們浮現出來。
- 指定 Kotodama/IVM 合約如何訪問 IH58 助手（`to_address()`、
  `parse_address()`）以及鏈上存儲是否應該公開 CAIP-10
  映射（現在 IH58 是規範的）。
- 探索在外部註冊表中註冊 Iroha 鏈（例如 IH58 註冊表、
  CAIP 命名空間目錄）以實現更廣泛的生態系統協調。

## 後續步驟

1、IH58編碼登陸`iroha_data_model`（`AccountAddress::to_ih58`，
   `parse_encoded`);繼續將固定裝置/測試移植到每個 SDK 並清除任何
   Bech32m 佔位符。
2. 使用 `chain_discriminant` 擴展配置模式並導出合理的
  現有測試/開發設置的默認值。 **（完成：`common.chain_discriminant`
  現在以 `iroha_config` 形式提供，每個網絡默認為 `0x02F1`
  覆蓋。）**
3. 起草 Nexus 註冊表架構和概念驗證清單發布者。
4. 收集錢包提供商和託管人對人為因素的反饋
   （HRP 命名、顯示格式）。
5. 更新文檔（`docs/source/data_model.md`、Torii API 文檔）
   實施路徑已承諾。
6. 發布官方編解碼庫（Rust/TS/Python/Kotlin）並進行規範測試
   涵蓋成功和失敗案例的向量。
