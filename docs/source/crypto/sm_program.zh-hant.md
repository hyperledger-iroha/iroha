---
lang: zh-hant
direction: ltr
source: docs/source/crypto/sm_program.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 08e2e1e4a54390d9142d6788aad2385e93282a33423b9fc7f3418e3633f3f86a
source_last_modified: "2026-01-23T23:46:10.134857+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//！ Hyperledger Iroha v2 的 SM2/SM3/SM4 支持架構簡介。

# SM 程序架構簡介

## 目的
定義在 Iroha v2 堆棧中引入中國國家密碼 (SM2/SM3/SM4) 的技術計劃、供應鏈態勢和風險邊界，同時保持確定性執行和可審計性。

## 範圍
- **共識關鍵路徑：** `iroha_crypto`、`iroha`、`irohad`、IVM 主機、Kotodama 內在函數。
- **客戶端 SDK 和工具：** Rust CLI、Kagami、Python/JS/Swift SDK、創世實用程序。
- **配置和序列化：** `iroha_config` 旋鈕、Norito 數據模型標籤、清單處理、多編解碼器更新。
- **測試與合規性：** 單元/屬性/互操作套件、Wycheproof 安全帶、性能分析、出口/監管指南。 *（狀態：RustCrypto 支持的 SM 堆棧合併；可選的 `sm_proptest` 模糊套件和 OpenSSL 奇偶校驗工具可用於擴展 CI。）*

超出範圍：PQ 算法、共識路徑中的非確定性主機加速； wasm/`no_std` 版本已退役。

## 算法輸入和可交付成果
|神器|業主|到期|筆記|
|----------|--------|-----|--------|
| SM算法特徵設計(`SM-P0`) |加密工作組 | 2025年02月 |功能門控、依賴性審計、風險登記。 |
|核心 Rust 集成 (`SM-P1`) |加密工作組/數據模型 | 2025 年 3 月 |基於 RustCrypto 的驗證/哈希/AEAD 幫助程序、Norito 擴展、固定裝置。 |
|簽名 + VM 系統調用 (`SM-P2`) | IVM 核心/SDK 程序 | 2025 年 4 月 |確定性簽名包裝器、系統調用、Kotodama 覆蓋範圍。 |
|可選的提供商和運營支持 (`SM-P3`) |平台運營/性能工作組 | 2025 年 6 月 | OpenSSL/Tongsuo 後端、ARM 內在函數、遙測、文檔。 |

## 選定的庫
- **主要：** RustCrypto 箱（`sm2`、`sm3`、`sm4`），啟用了 `rfc6979` 功能並且 SM3 綁定到確定性隨機數。
- **可選 FFI：** OpenSSL 3.x 提供商 API 或 Tongsuo，用於需要經過認證的堆棧或硬件引擎的部署；在共識二進製文件中默認情況下具有功能門控和禁用功能。### 核心庫集成狀態
- `iroha_crypto::sm` 在統一的 `sm` 功能下公開 SM3 哈希、SM2 驗證和 SM4 GCM/CCM 幫助程序，並通過 SDK 提供確定性 RFC6979 簽名路徑`Sm2PrivateKey`.【crates/iroha_crypto/src/sm.rs:1049】【crates/iroha_crypto/src/sm.rs:1128】【crates/iroha_crypto/src/sm.rs:1236】
- Norito/Norito-JSON 標籤和多編解碼器幫助程序涵蓋 SM2 公鑰/簽名和 SM3/SM4 有效負載，因此指令可以確定性地序列化主機.【crates/iroha_data_model/src/isi/registry.rs:407】【crates/iroha_data_model/tests/sm_norito_roundtrip.rs:12】
- 已知答案套件驗證 RustCrypto 集成（`sm3_sm4_vectors.rs`、`sm2_negative_vectors.rs`）並作為 CI 的 `sm` 功能作業的一部分運行，在節點繼續簽名時保持驗證確定性Ed25519.【板條箱/iroha_crypto/tests/sm3_sm4_vectors.rs:15】【板條箱/iroha_crypto/tests/sm2_male_vectors.rs:1】
- 可選的 `sm` 功能構建驗證：`cargo check -p iroha_crypto --features sm --locked`（冷 7.9 秒/暖 0.23 秒）和 `cargo check -p iroha_crypto --no-default-features --features "std sm" --locked`（1.0 秒）均成功；啟用該功能會添加 11 個箱子（`base64ct`、`ghash`、`opaque-debug`、`pem-rfc7468`、`pkcs8`、`polyval`、`primeorder`、 `sm2`、`sm3`、`sm4`、`sm4-gcm`）。調查結果記錄在 `docs/source/crypto/sm_rustcrypto_spike.md` 中。 【docs/source/crypto/sm_rustcrypto_spike.md:1】
- BouncyCastle/GmSSL 否定驗證裝置位於 `crates/iroha_crypto/tests/fixtures/sm/sm2_negative_vectors.json` 下，確保規範故障案例（r=0、s=0、區分 ID 不匹配、公鑰被篡改）與廣泛部署保持一致提供者.【crates/iroha_crypto/tests/sm2_male_vectors.rs:1】【crates/iroha_crypto/tests/fixtures/sm/sm2_male_vectors.json:1】
- `sm-ffi-openssl` 現在編譯供應商的 OpenSSL 3.x 工具鏈（`openssl` crate `vendored` 功能），因此即使系統 LibreSSL/OpenSSL 缺乏 SM 算法，預覽構建和測試也始終以支持現代 SM 的提供程序為目標。 【crates/iroha_crypto/Cargo.toml:59】
- `sm_accel` 現在在運行時檢測 AArch64 NEON，並通過 x86_64/RISC-V 調度將 SM3/SM4 掛鉤線程化，同時遵循配置旋鈕 `crypto.sm_intrinsics` (`auto`/`force-enable`/`force-disable`)。當向量後端不存在時，調度程序仍然通過標量 RustCrypto 路徑進行路由，因此工作台和策略切換在主機之間的行為一致。 【crates/iroha_crypto/src/sm.rs:702】【crates/iroha_crypto/src/sm.rs:733】

### Norito 架構和數據模型表面| Norito 類型/消費者 |代表|限制和注意事項|
|------------------------------------|----------------|---------------------|
| `Sm3Digest` (`iroha_crypto::Sm3Digest`) |裸露：32 字節 blob · JSON：大寫十六進製字符串 (`"4F4D..."`) |規範 Norito 元組包裝 `[u8; 32]`。 JSON/Bare 解碼拒絕長度 ≠32。 `sm_norito_roundtrip::sm3_digest_norito_roundtrip` 涵蓋的往返行程。 |
| `Sm2PublicKey` / `Sm2Signature` |多編解碼器前綴 blob（`0x1306` 臨時）|公鑰對未壓縮的 SEC1 點進行編碼；簽名為 `(r∥s)`（每個 32 字節），帶有 DER 解析防護。 |
| `Sm4Key` |裸露：16 字節 blob |歸零暴露於 Kotodama/CLI 的包裝器。故意省略了 JSON 序列化；操作員應通過 blob（合約）或 CLI 十六進制 (`--key-hex`) 傳遞密鑰。 |
| `sm4_gcm_seal/open` 操作數 | 4 個 blob 元組：`(key, nonce, aad, payload)` |密鑰 = 16 字節；隨機數 = 12 字節；標籤長度固定為 16 字節。返回 `(ciphertext, tag)`； Kotodama/CLI 發出十六進制和 Base64 幫助程序。 【crates/ivm/tests/sm_syscalls.rs:728】 |
| `sm4_ccm_seal/open` 操作數 | `r14` 中 4 個 blob 的元組（密鑰、隨機數、aad、有效負載）+ 標籤長度 |隨機數 7–13 字節；標籤長度 ∈ {4,6,8,10,12,14,16}。 `sm` 功能暴露了 `sm-ccm` 標誌後面的 CCM。 |
| Kotodama 內在函數（`sm::hash`、`sm::seal_gcm`、`sm::open_gcm`，...）映射到上面的 SCALL |輸入驗證反映主機規則；格式錯誤的大小會引發 `ExecutionError::Type`。 |

使用快速參考：
- **合約/測試中的 SM3 哈希：** `Sm3Digest::hash(b"...")` (Rust) 或 Kotodama `sm::hash(input_blob)`。 JSON 需要 64 個十六進製字符。
- **SM4 AEAD 通過 CLI:** `iroha tools crypto sm4 gcm-seal --key-hex <32 hex> --nonce-hex <24 hex> --plaintext-hex …` 生成十六進制/base64 密文+標籤對。使用匹配的 `gcm-open` 進行解密。
- **多編解碼器字符串：** SM2 公鑰/簽名從 `PublicKey::from_str`/`Signature::from_bytes` 接受的多基字符串解析，使 Norito 清單和帳戶 ID 能夠攜帶 SM 簽名者。

數據模型使用者應將 SM4 鍵和標籤視為瞬態 blob；切勿將原始密鑰保留在鏈上。當需要審計時，合約應僅存儲密文/標籤輸出或派生摘要（例如密鑰的 SM3）。

### 供應鍊和許可
|組件|許可證|緩解措施 |
|------------|---------|------------|
| `sm2`、`sm3`、`sm4` | Apache-2.0 / 麻省理工學院 |跟踪上游提交、供應商（如果需要同步發布）、在驗證者簽署 GA 之前安排第三方審核。 |
| `rfc6979` | Apache-2.0 / 麻省理工學院 |已經在其他算法中使用；確認 `k` 與 SM3 摘要的確定性結合。 |
|可選OpenSSL/通鎖 | Apache-2.0 / BSD 風格 |保留 `sm-ffi-openssl` 功能，需要明確的操作員選擇加入和打包清單。 |### 功能標誌和所有權
|表面|默認|維護者 |筆記|
|--------|---------|------------|--------|
| `iroha_crypto/sm-core`、`sm-ccm`、`sm` |關閉 |加密工作組 |啟用 RustCrypto SM 原語； `sm` 為需要經過身份驗證的加密的客戶端捆綁了 CCM 幫助程序。 |
| `ivm/sm` |關閉 | IVM 核心團隊 |構建 SM 系統調用（`sm3_hash`、`sm2_verify`、`sm4_gcm_*`、`sm4_ccm_*`）。主機門控源自 `crypto.allowed_signing`（存在 `sm2`）。 |
| `iroha_crypto/sm_proptest` |關閉 |質量保證/加密工作組 |屬性測試工具涵蓋格式錯誤的簽名/標籤。僅在擴展 CI 中啟用。 |
| `crypto.allowed_signing` + `default_hash` | `["ed25519"]`，`blake2b-256` |配置工作組/操作員工作組 | `sm2` 加上 `sm3-256` 散列的存在可啟用 SM 系統調用/簽名；刪除 `sm2` 將返回僅驗證模式。 |
|可選 `sm-ffi-openssl`（預覽）|關閉 |平台運營 | OpenSSL/Tongsuo 提供商集成的佔位符功能；在認證和包裝 SOP 落地之前保持禁用狀態。 |

網絡策略現在公開 `network.require_sm_handshake_match` 和
`network.require_sm_openssl_preview_match`（均默認為 `true`）。清除任一標誌允許
混合部署，其中僅 Ed25519 的觀察者連接到支持 SM 的驗證器；不匹配的是
記錄在`WARN`，但共識節點應保持默認啟用，以防止意外
SM 感知和 SM 禁用對等點之間的差異。
CLI 通過“iroha_cli app sorafs 握手更新”顯示這些切換
--allow-sm-handshake-mismatch` and `--allow-sm-openssl-preview-mismatch`, or the matching `--require-*`
旗幟恢復嚴格執行。#### OpenSSL/通所預覽(`sm-ffi-openssl`)
- **範圍。 ** 構建僅預覽的提供程序墊片 (`OpenSslProvider`)，用於驗證 OpenSSL 運行時可用性並公開 OpenSSL 支持的 SM3 哈希、SM2 驗證和 SM4-GCM 加密/解密，同時保持選擇加入。共識二進製文件必須繼續使用 RustCrypto 路徑； FFI 後端嚴格選擇加入邊緣驗證/簽名試點。
- **構建先決條件。 ** 使用 `cargo build -p iroha_crypto --features "sm sm-ffi-openssl"` 進行編譯，並確保工具鍊鍊接到 OpenSSL/Tongsuo 3.0+（支持 SM2/SM3/SM4 的 `libcrypto`）。不鼓勵靜態鏈接；更喜歡由操作員管理的動態庫。
- **開發人員冒煙測試。 ** 運行 `scripts/sm_openssl_smoke.sh` 來執行 `cargo check -p iroha_crypto --features "sm sm-ffi-openssl"`，然後執行 `cargo test -p iroha_crypto --features "sm sm-ffi-openssl" --test sm_openssl_smoke -- --nocapture`；當 OpenSSL ≥3 開發標頭不可用（或缺少 `pkg-config`）時，助手會自動跳過並顯示煙霧輸出，以便開發人員可以查看 SM2 驗證是運行還是退回到 Rust 實現。
- **Rust 腳手架。 ** `openssl_sm` 模塊現在通過 OpenSSL 路由 SM3 哈希、SM2 驗證（ZA 預哈希 + SM2 ECDSA）和 SM4 GCM 加密/解密，並產生涵蓋預覽切換和無效密鑰/隨機數/標籤長度的結構化錯誤； SM4 CCM 保持純 Rust 狀態，直到額外的 FFI 墊片落地。
- **跳過行為。 ** 當 OpenSSL ≥3.0 標頭或庫不存在時，煙霧測試會打印一個跳過橫幅（通過 `-- --nocapture`），但仍然成功退出，因此 CI 可以區分環境差距和真正的回歸。
- **運行時護欄。 ** 默認情況下禁用 OpenSSL 預覽；在嘗試使用 FFI 路徑之前，通過配置 (`crypto.enable_sm_openssl_preview` / `OpenSslProvider::set_preview_enabled(true)`) 啟用它。將生產集群保持在僅驗證模式（從 `allowed_signing` 中省略 `sm2`），直到提供商畢業，依賴確定性 RustCrypto 回退，並將簽名試點限制在隔離環境中。
- **打包清單。 ** 在部署清單中記錄提供程序版本、安裝路徑和完整性哈希值。運營商必須提供安裝腳本來安裝已批准的 OpenSSL/Tongsuo 版本，將其註冊到操作系統信任存儲（如果需要），並在維護時段後進行固定升級。
- **後續步驟。 ** 未來的里程碑添加確定性 SM4 CCM FFI 綁定、CI 煙霧作業（請參閱 `ci/check_sm_openssl_stub.sh`）和遙測。跟踪 `roadmap.md` 中 SM-P3.1.x 下的進度。

#### 代碼所有權快照
- **加密工作組：** `iroha_crypto`、SM 裝置、合規性文檔。
- **IVM 核心：** 系統調用實現、Kotodama 內在函數、主機門控。
- **配置工作組：** `crypto.allowed_signing`/`default_hash`，最新版本，最新版本。
- **SD​​​​K 程序：** 跨 CLI/Kagami/SDK 的 SM 感知工具、共享夾具。
- **平台運營和性能工作組：**加速掛鉤、遙測、操作員支持。

## 配置遷移手冊從僅使用 Ed25519 的網絡轉向支持 SM 的部署的運營商應該
遵循分階段流程
[`sm_config_migration.md`](sm_config_migration.md)。該指南涵蓋構建
驗證、`iroha_config` 分層（`defaults` → `user` → `actual`）、創世
通過 `kagami` 覆蓋（例如 `kagami genesis generate --allowed-signing sm2 --default-hash sm3-256`）、飛行前驗證和回滾進行重新生成
進行規劃，以便配置快照和清單在整個系統中保持一致
艦隊。

## 確定性策略
- 對 SDK 和可選主機簽名中的所有 SM2 簽名路徑強制執行 RFC6979 派生的隨機數；驗證者僅接受規範的 r∥s 編碼。
- 控制平面通信（流）仍然是 Ed25519； SM2 僅限於數據平面簽名，除非治理批准擴展。
- 內在函數 (ARM SM3/SM4) 僅限於具有運行時功能檢測和軟件回退的確定性驗證/哈希操作。

## Norito & 編碼計劃
1. 使用 `Sm2PublicKey`、`Sm2Signature`、`Sm3Digest`、`Sm4Key` 擴展 `iroha_data_model` 中的算法枚舉。
2、將SM2簽名序列化為大端定寬`r∥s`數組（32+32字節），以避免DER歧義；適配器中處理的轉換。 *（完成：在 `Sm2Signature` 幫助程序中實現；Norito/JSON 往返到位。）*
3. 如果使用多格式，請註冊多編解碼器標識符（`sm3-256`、`sm2-pub`、`sm4-key`）、更新裝置和文檔。 *（進展：`sm2-pub` 臨時代碼 `0x1306` 現已使用派生密鑰進行驗證；SM3/SM4 代碼等待最終分配，通過 `sm_known_answers.toml` 進行跟踪。）*
4. 更新 Norito 黃金測試，涵蓋往返和拒絕格式錯誤的編碼（短/長 r 或 s、無效曲線參數）。## 主機和虛擬機集成計劃 (SM-2)
1. 實現主機端 `sm3_hash` 系統調用鏡像現有的 GOST 哈希 shim；重用 `Sm3Digest::hash` 並公開確定性錯誤路徑。 *（已著陸：主機返回 Blob TLV；請參閱 `DefaultHost` 實現和 `sm_syscalls.rs` 回歸。）*
2. 使用 `sm2_verify` 擴展 VM 系統調用表，該表接受規範的 r∥s 簽名、驗證區分 ID 並將故障映射到確定性返回代碼。 *（完成：主機 + Kotodama 內在函數返回 `1/0`；回歸套件現在涵蓋截斷的簽名、格式錯誤的公鑰、非 blob TLV 和 UTF-8/空/不匹配的 `distid` 有效負載。）*
3. 為 `sm4_gcm_seal`/`sm4_gcm_open`（以及可選的 CCM）系統調用提供顯式隨機數/標籤大小調整 (RFC 8998)。 *（完成：GCM 使用固定的 12 字節隨機數 + 16 字節標籤；CCM 支持 7-13 字節隨機數，標籤長度為 {4,6,8,10,12,14,16}，通過 `r14` 控制；Kotodama 將這些公開為 `sm::seal_gcm/open_gcm` 和`sm::seal_ccm/open_ccm`。）在開發人員手冊中記錄隨機數重用策略。 *
4. 連線 Kotodama 煙霧合約和 IVM 集成測試，涵蓋正面和負面案例（標籤更改、簽名格式錯誤、算法不受支持）。 *（通過 SM3/SM2/SM4 的 `crates/ivm/tests/kotodama_sm_syscalls.rs` 鏡像主機回歸完成。）*
5. 更新系統調用允許列表、策略和 ABI 文檔 (`crates/ivm/docs/syscalls.md`)，並在添加新條目後刷新哈希清單。

### 主機和虛擬機集成狀態
- DefaultHost、CoreHost 和 WsvHost 公開 SM3/SM2/SM4 系統調用並在 `sm_enabled` 上對它們進行門控，當運行時標誌為時返回 `PermissionDenied` false.【crates/ivm/src/host.rs:915】【crates/ivm/src/core_host.rs:833】【crates/ivm/src/mock_wsv.rs:2307】
- `crypto.allowed_signing` 門控通過管道/執行器/狀態進行線程化，因此生產節點通過配置確定性地選擇加入；添加 `sm2` 切換 SM 助手可用性。 `【crates/iroha_core/src/smartcontracts/ivm/host.rs:170】【crates/iroha_core/src/state.rs:7673】【crates/iroha_core/src/executor.rs:683】
- 回歸覆蓋測試 SM3 哈希、SM2 驗證和 SM4 GCM/CCM 密封/開放的啟用和禁用路徑（DefaultHost/CoreHost/WsvHost） 【crates/ivm/tests/sm_syscalls.rs:129】【crates/ivm/tests/sm_syscalls.rs:733】【crates/ivm/tests/sm_syscalls.rs:1036】

## 配置線程
- 將 `crypto.allowed_signing`、`crypto.default_hash`、`crypto.sm2_distid_default` 和可選的 `crypto.enable_sm_openssl_preview` 添加到 `iroha_config`。確保數據模型功能管道鏡像加密箱（`iroha_data_model` 公開 `sm` → `iroha_crypto/sm`）。
- 將配置連接到准入策略，以便清單/創世文件定義允許的算法；控制平面默認保留 Ed25519。### CLI 和 SDK 工作 (SM-3)
1. **Torii CLI** (`crates/iroha_cli`)：添加 SM2 keygen/導入/導出（distid 感知）、SM3 哈希幫助程序和 SM4 AEAD 加密/解密命令。更新交互式提示和文檔。
2. **Genesis 工具**（`xtask`、`scripts/`）：允許清單聲明允許的簽名算法和默認哈希值，如果在沒有相應配置旋鈕的情況下啟用 SM，則會快速失敗。 *（完成：`RawGenesisTransaction`現在帶有一個`crypto`塊，其中包含`default_hash`/`allowed_signing`/`sm2_distid_default`；`ManifestCrypto::validate`和`kagami genesis validate`拒絕不一致的SM設置和默認/起源廣告快照。）*
3. **SDK 界面**：
   - Rust (`iroha_client`)：公開具有確定性默認值的 SM2 簽名/驗證助手、SM3 哈希、SM4 AEAD 包裝器。
   - Python/JS/Swift：鏡像 Rust API；重用 `sm_known_answers.toml` 中的分段裝置進行跨語言測試。
4. 記錄在 CLI/SDK 快速入門中啟用 SM 的操作員工作流程，並確保 JSON/YAML 配置接受新的算法標籤。

#### CLI 進度
- `cargo run -p iroha_cli --features sm -- crypto sm2 keygen --distid CN12345678901234` 現在發出描述 SM2 密鑰對的 JSON 有效負載以及 `client.toml` 片段（`public_key_config`、`private_key_hex`、`distid`）。該命令接受 `--seed-hex` 進行確定性生成，並鏡像主機使用的 RFC 6979 派生。
- `cargo xtask sm-operator-snippet --distid CN12345678901234` 包裝了註冊機/導出流程，一步寫入相同的 `sm2-key.json`/`client-sm2.toml` 輸出。使用 `--json-out <path|->` / `--snippet-out <path|->` 重定向文件或將其流式傳輸到標準輸出，從而刪除 `jq` 的自動化依賴性。
- `iroha_cli tools crypto sm2 import --private-key-hex <hex> [--distid ...]` 從現有材料中導出相同的元數據，以便操作員可以在准入之前驗證區分 ID。
- `iroha_cli tools crypto sm2 export --private-key-hex <hex> --emit-json` 打印配置片段（包括 `allowed_signing`/`sm2_distid_default` 指南），並可選擇重新發出 JSON 密鑰清單以進行腳本編寫。
- `iroha_cli tools crypto sm3 hash --data <string>` 散列任意有效負載； `--data-hex` / `--file` 涵蓋二進制輸入，並且該命令報告清單工具的十六進制和 Base64 摘要。
- `iroha_cli tools crypto sm4 gcm-seal --key-hex <KEY> --nonce-hex <NONCE> --plaintext-hex <PT>`（和 `gcm-open`）包裝主機 SM4-GCM 幫助程序和表面 `ciphertext_hex`/`tag_hex` 或明文有效負載。 `sm4 ccm-seal` / `sm4 ccm-open` 為 CCM 提供相同的 UX，內置隨機數長度（7-13 字節）和標籤長度（4,6,8,10,12,14,16）驗證；這兩個命令都可以選擇將原始字節發送到磁盤。## 測試策略
### 單元/已知答案測試
- SM3 的 GM/T 0004 和 GB/T 32905 矢量（例如 `"abc"`）。
- SM4 的 GM/T 0002 和 RFC 8998 矢量（塊 + GCM/CCM）。
- GM/T 0003/GB/T 32918 SM2 示例（Z 值、簽名驗證），包括 ID 為 `ALICE123@YAHOO.COM` 的附件示例 1。
- 臨時夾具暫存文件：`crates/iroha_crypto/tests/fixtures/sm_known_answers.toml`。
- Wycheproof 衍生的 SM2 回歸套件 (`crates/iroha_crypto/tests/sm2_wycheproof.rs`) 現在帶有 52 個案例的語料庫，該語料庫將確定性裝置（附件 D、SDK 種子）與位翻轉、消息篡改和截斷簽名負數分層。清理後的 JSON 存在於 `crates/iroha_crypto/tests/fixtures/wycheproof_sm2.json` 中，`sm2_fuzz.rs` 直接使用它，因此快樂路徑和篡改場景在模糊/屬性運行中保持一致。附件 附件 附件 附件 附件 附件 附件 附件請注意以下事項。
- `cargo xtask sm-wycheproof-sync --input <wycheproof-sm2.json>`（或 `--input-url <https://…>`）確定性地修剪任何上游丟棄（生成器標籤可選）並重寫 `crates/iroha_crypto/tests/fixtures/wycheproof_sm2.json`。在C2SP發布官方語料庫之前，手動下載fork並通過helper餵牠們；它標準化鍵、計數和標誌，以便審閱者可以對差異進行推理。
- SM2/SM3 Norito 往返在 `crates/iroha_data_model/tests/sm_norito_roundtrip.rs` 中驗證。
- `crates/ivm/tests/sm_syscalls.rs` 中的 SM3 主機系統調用回歸（SM 功能）。
- SM2 驗證 `crates/ivm/tests/sm_syscalls.rs` 中的系統調用回歸（成功 + 失敗案例）。

### 屬性和回歸測試
- SM2 的 Proptest 拒絕無效曲線、非規範 r/s 和隨機數重用。 *（在 `crates/iroha_crypto/tests/sm2_fuzz.rs` 中可用，在 `sm_proptest` 後面門控；通過 `cargo test -p iroha_crypto --features "sm sm_proptest"` 啟用。）*
- Wycheproof SM4 矢量（塊/AES 模式）適用於各種模式；跟踪上游 SM2 添加。 `sm3_sm4_vectors.rs` 現在針對 GCM 和 CCM 執行標籤位翻轉、截斷標籤和密文篡改。

### 互操作和性能
- 用於 SM2 簽名/驗證、SM3 摘要和 SM4 ECB/GCM 的 RustCrypto ↔ OpenSSL/Tongsuo 奇偶校驗套件位於 `crates/iroha_crypto/tests/sm_cli_matrix.rs` 中；使用 `scripts/sm_interop_matrix.sh` 調用它。 CCM 奇偶校驗向量現在在 `sm3_sm4_vectors.rs` 中運行；一旦上游 CLI 公開 CCM 幫助程序，CLI 矩陣支持就會隨之而來。
- SM3 NEON 幫助程序現在通過 `sm_accel::is_sm3_enabled` 運行時端到端運行 Armv8 壓縮/填充路徑（功能 + env 覆蓋在 SM3/SM4 上鏡像）。黃金摘要（零/`"abc"`/長塊 + 隨機長度）和強制禁用測試與標量 RustCrypto 後端保持同等，並且 Criterion 微基準 (`crates/sm3_neon/benches/digest.rs`) 捕獲 AArch64 主機上的標量與 NEON 吞吐量。
- Perf 線束鏡像 `scripts/gost_bench.sh`，用於比較 Ed25519/SHA-2 與 SM2/SM3/SM4 並驗證容差閾值。#### Arm64 基準（本地 Apple 芯片；標準 `sm_perf`，2025 年 12 月 5 日更新）
- `scripts/sm_perf.sh` 現在運行 Criterion 基準，並針對 `crates/iroha_crypto/benches/sm_perf_baseline.json` 強制執行中位數（在 aarch64 macOS 上記錄；默認情況下容差 25%，基線元數據捕獲主機三重）。新的 `--mode` 標誌使工程師可以捕獲標量、NEON 與 `sm-neon-force` 數據點，而無需編輯腳本；當前捕獲包（原始 JSON + 聚合摘要）位於 `artifacts/sm_perf/2026-03-lab/m3pro_native/` 下，並使用 `cpu_label="m3-pro-native"` 標記每個有效負載。
- 加速模式現在自動選擇標量基線作為比較目標。 `scripts/sm_perf.sh` 通過 `sm_perf_check` 線程 `--compare-baseline/--compare-tolerance/--compare-label`，針對標量參考發出每個基準增量，並在減速超過配置的閾值時失敗。基線的每個基準公差驅動比較保護（SM3 在 Apple 標量基線上的上限為 12%，而 SM3 比較增量現在允許與標量參考相比高達 70%，以避免波動）； Linux 基線重複使用相同的比較圖，因為它們是從 `neoverse-proxy-macos` 捕獲中導出的，如果中位數不同，我們將在裸機 Neoverse 運行後收緊它們。當捕獲更嚴格的邊界（例如，`--compare-tolerance 0.20`）時顯式傳遞 `--compare-tolerance` 並使用 `--compare-label` 來註釋替代參考主機。
- CI 參考機上記錄的基線現在位於 `crates/iroha_crypto/benches/sm_perf_baseline_aarch64_macos_scalar.json`、`sm_perf_baseline_aarch64_macos_auto.json` 和 `sm_perf_baseline_aarch64_macos_neon_force.json` 中。使用 `scripts/sm_perf.sh --mode scalar --write-baseline`、`--mode auto --write-baseline` 或 `--mode neon-force --write-baseline`（在捕獲之前設置 `SM_PERF_CPU_LABEL`）刷新它們，並將生成的 JSON 與運行日誌一起存檔。將聚合的幫助器輸出 (`artifacts/.../aggregated.json`) 與 PR 一起保留，以便審核者可以審核每個樣本。 Linux/Neoverse 基線現在以 `sm_perf_baseline_aarch64_unknown_linux_gnu_{mode}.json` 發布，從 `artifacts/sm_perf/2026-03-lab/neoverse-proxy-macos/aggregated.json` 升級（CPU 標籤 `neoverse-proxy-macos`，aarch64 macOS/Linux 的 SM3 比較容差 0.70）；當可以收緊容差時，在裸機 Neoverse 主機上重新運行。
- 基線 JSON 文件現在可以攜帶可選的 `tolerances` 對象，以加強每個基準的護欄。示例：
  ```json
  {
    "benchmarks": { "...": 12.34 },
    "tolerances": {
      "sm4_vs_chacha20poly1305_encrypt/sm4_gcm_encrypt": 0.08,
      "sm3_vs_sha256_hash/sm3_hash": 0.12
    }
  }
  ```
  `sm_perf_check` 應用這些分數限制（示例中為 8% 和 12%），同時對未列出的任何基準使用全局 CLI 容差。
- 比較防護還可以在比較基線中遵守 `compare_tolerances`。使用此選項可以允許針對標量參考的更寬鬆的增量（例如，標量基線中的 `\"sm3_vs_sha256_hash/sm3_hash\": 0.70`），同時保持主 `tolerances` 嚴格以進行直接基線檢查。- 簽入的 Apple Silicon 基線現在配備了混凝土護欄：SM2/SM4 操作允許根據方差有 12-20% 的漂移，而 SM3/ChaCha 比較則為 8-12%。標量基線的 `sm3` 容差現已收緊至 0.12； `unknown_linux_gnu` 文件鏡像 `neoverse-proxy-macos` 導出，具有相同的容差圖（SM3 比較 0.70）和元數據註釋，表明它們是為 Linux gateway 提供的，直到可以重新運行裸機 Neoverse。
- SM2 簽名：每次操作 298μs（Ed25519：32μs）⇒ 慢約 9.2 倍；驗證：267μs（Ed25519：41μs）⇒〜6.5×慢。
- SM3 散列（4KiB 有效負載）：11.2μs，與 11.3μs 的 SHA-256 有效奇偶校驗（約 356MiB/s 與 353MiB/s）。
- SM4-GCM 密封/打開（1KiB 有效負載，12 字節隨機數）：15.5μs 與 ChaCha20-Poly1305 相比，1.78μs（約 64MiB/s 與 525MiB/s）。
- 為再現性而捕獲的基準工件 (`target/criterion/sm_perf*`)； Linux 基線源自 `artifacts/sm_perf/2026-03-lab/neoverse-proxy-macos/`（CPU 標籤 `neoverse-proxy-macos`，SM3 比較容差 0.70），並且可以在實驗室時間開放後在裸機 Neoverse 主機 (`SM-4c.1`) 上刷新以收緊容差。

#### 跨架構捕獲清單
- 在目標機器**（x86_64 工作站、Neoverse ARM 服務器等）上運行 `scripts/sm_perf_capture_helper.sh`。通過 `--cpu-label <host>` 來標記捕獲並（在矩陣模式下運行時）預填充生成的計劃/命令以進行實驗室安排。幫助程序打印特定於模式的命令：
  1. 使用正確的功能集執行 Criterion 套件，並且
  2. 將中位數寫入 `crates/iroha_crypto/benches/sm_perf_baseline_${arch}_${os}_${mode}.json`。
- 首先捕獲標量基線，然後重新運行 `auto`（以及 AArch64 平台上的 `neon-force`）的幫助程序。使用有意義的 `SM_PERF_CPU_LABEL`，以便審閱者可以跟踪 JSON 元數據中的主機詳細信息。
- 每次運行後，存檔原始 `target/criterion/sm_perf*` 目錄並將其與生成的基線一起包含在 PR 中。一旦連續兩次運行穩定，就收緊每個基準的容差（有關參考格式，請參閱 `sm_perf_baseline_aarch64_macos_*.json`）。
- 記錄本節中的中位數 + 公差，並在涵蓋新架構時更新 `status.md`/`roadmap.md`。 Linux 基線現在從 `neoverse-proxy-macos` 捕獲中籤入（元數據記錄了導出到 aarch64-unknown-linux-gnu 門）；當這些實驗室插槽可用時，在裸機 Neoverse/x86_64 主機上重新運行作為後續操作。

#### ARMv8 SM3/SM4 內在函數與標量路徑
`sm_accel`（請參閱 `crates/iroha_crypto/src/sm.rs:739`）為 NEON 支持的 SM3/SM4 幫助器提供運行時調度層。該功能受到三個級別的保護：|層 |控制|筆記|
|--------|---------|--------|
|編譯時間| `--features sm`（現在在 `aarch64` 上自動引入 `sm-neon`）或 `sm-neon-force`（測試/基準測試）|構建 NEON 模塊並鏈接 `sm3-neon`/`sm4-neon`。 |
|運行時自動檢測 | `sm4_neon::is_supported()` |僅適用於公開 AES/PMULL 等效項的 CPU（例如 Apple M 系列、Neoverse V1/N2）。屏蔽 NEON 或 FEAT_SM4 的虛擬機會回退到標量代碼。 |
|操作員覆蓋 | `crypto.sm_intrinsics` (`auto`/`force-enable`/`force-disable`) |啟動時應用配置驅動的調度；僅將 `force-enable` 用於受信任環境中的分析，並在驗證標量回退時首選 `force-disable`。 |

**性能範圍（Apple M3 Pro；中值記錄在 `sm_perf_baseline_aarch64_macos_{mode}.json` 中）：**

|模式| SM3 摘要 (4KiB) | SM4-GCM 密封 (1KiB) |筆記|
|------|--------------------|----------------------|--------|
|標量 | 11.6μs | 11.6μs 15.9μs | 15.9μs確定性 RustCrypto 路徑；在編譯 `sm` 功能的任何地方都使用它，但 NEON 不可用。 |
|霓虹燈汽車 |比標量快約 2.7 倍 |比標量快約 2.3 倍 |當前的 NEON 內核 (SM-5a.2c) 將調度一次加寬四個字並使用雙隊列扇出；確切的中位數因主機而異，因此請參閱基準 JSON 元數據。 |
|霓虹力量|鏡像 NEON auto 但完全禁用回退 |與 NEON 汽車相同 |通過 `scripts/sm_perf.sh --mode neon-force` 行使；即使在默認為標量模式的主機上也能保持 CI 誠實。 |

**確定性和部署指南**
- 內在函數永遠不會改變可觀察的結果 - 當加速路徑不可用時，`sm_accel` 返回 `None`，因此標量助手運行。因此，只要標量實現正確，共識代碼路徑就保持確定性。
- **不要**對是否使用 NEON 路徑進行業務邏輯門控。將加速度純粹視為性能提示，並僅通過遙測技術公開狀態（例如，`sm_intrinsics_enabled` 儀表）。
- 接觸 SM 代碼後始終運行 `ci/check_sm_perf.sh`（或 `make check-sm-perf`），以便 Criterion 工具使用每個基線 JSON 中嵌入的容差來驗證標量路徑和加速路徑。
- 在進行基準測試或調試時，更喜歡配置旋鈕 `crypto.sm_intrinsics` 而不是編譯時標誌；使用 `sm-neon-force` 重新編譯會完全禁用標量回退，而 `force-enable` 只是輕推運行時檢測。
- 在發行說明中記錄所選策略：生產版本應將策略保留在 `Auto` 中，讓每個驗證器獨立發現硬件功能，同時仍共享相同的二進制工件。
- 避免交付混合靜態鏈接的供應商內在函數（例如第三方 SM4 庫）的二進製文件，除非它們遵循相同的調度和測試流程，否則我們的基線工具將無法捕獲性能回歸。#### x86_64 Rosetta 基線（Apple M3 Pro；捕獲於 2025 年 12 月 1 日）
- 基線位於 `crates/iroha_crypto/benches/sm_perf_baseline_x86_64_macos_{scalar,auto,neon_force}.json` (cpu_label=`m3-pro-rosetta`) 中，原始 + 聚合捕獲位於 `artifacts/sm_perf/2026-03-lab/m3pro_rosetta/` 下。
- x86_64 上的每個基準容差對於 SM2 設置為 20%，對於 Ed25519/SHA-256 設置為 15%，對於 SM4/ChaCha 設置為 12%。 `scripts/sm_perf.sh` 現在在非 AArch64 主機上將加速比較容差默認為 25%，因此標量與自動保持緊密，同時在 AArch64 上為共享 `m3-pro-native` 基線留下 5.25 的餘量，直到 Neoverse 重新運行。

|基準|標量 |汽車 |霓虹力量 |自動與標量 |霓虹燈與標量 |霓虹燈與汽車|
|------------|--------|------|------------|----------------|----------------------------|------------|
| sm2_vs_ed25519_sign/ed25519_sign | sm2_vs_ed25519_sign/ed25519_sign |    57.43 | 57.43  57.12 | 57.12      55.77 | 55.77          -0.53% |         -2.88% |        -2.36% |
| sm2_vs_ed25519_sign/sm2_sign | sm2_vs_ed25519_sign/sm2_sign |   572.76 | 572.76 568.71 | 568.71     557.83 | 557.83          -0.71% |         -2.61% |        -1.91% |
| sm2_vs_ed25519_verify/驗證/ed25519 |    69.03 |  68.42 |      66.28 |          -0.88% |         -3.97% |        -3.12% |
| sm2_vs_ed25519_verify/驗證/sm2 |   521.73 | 521.73 514.50 | 514.50     502.17 | 502.17          -1.38% |         -3.75% |        -2.40% |
| sm3_vs_sha256_hash/sha256_hash | sm3_vs_sha256_hash/sha256_hash |    16.78 | 16.78  16.58 | 16.58      16.16 | 16.16          -1.19% |         -3.69% |        -2.52% |
| sm3_vs_sha256_hash/sm3_hash | sm3_vs_sha256_hash/sm3_hash |    15.78 | 15.78  15.51 | 15.51      15.04 | 15.04          -1.71% |         -4.69% |        -3.03% |
| sm4_vs_chacha20poly1305_decrypt/chacha20poly1305_decrypt | sm4_vs_chacha20poly1305_decrypt     1.96 | 1.96   1.97 | 1.97       1.97 | 1.97           0.39% |          0.16% |        -0.23% |
| sm4_vs_chacha20poly1305_decrypt/sm4_gcm_decrypt | sm4_vs_chacha20poly1305_decrypt/sm4_gcm_decrypt |    16.26 | 16.26  16.38 | 16.38      16.26 | 16.26           0.72% |         -0.01% |        -0.72% |
| sm4_vs_chacha20poly1305_加密/chacha20poly1305_加密 |     1.96 | 1.96   2.00 | 2.00       1.93 | 1.93           2.23% |         -1.14% |        -3.30% |
| sm4_vs_chacha20poly1305_加密/sm4_gcm_加密 |    16.60 | 16.60  16.58 | 16.58      16.15 | 16.15          -0.10% |         -2.66% |        -2.57% |

#### x86_64 /其他非aarch64目標
- 當前版本仍然僅在 x86_64 上提供確定性 RustCrypto 標量路徑；保持 `sm` 啟用，但在 SM-4c.1b 登陸之前**不**注入外部 AVX2/VAES 內核。運行時策略鏡像 ARM：默認為 `Auto`，遵循 `crypto.sm_intrinsics`，並顯示相同的遙測儀表。
- Linux/x86_64 捕獲仍有待記錄；重用該硬件上的幫助程序，並將中位數與上面的 Rosetta 基線和容差圖一起放入 `sm_perf_baseline_x86_64_unknown_linux_gnu_{mode}.json` 中。**常見陷阱**
1. **虛擬化 ARM 實例：** 許多雲公開 NEON，但隱藏 `sm4_neon::is_supported()` 檢查的 SM4/AES 擴展。預期這些環境中的標量路徑並相應地捕獲性能基線。
2. **部分覆蓋：** 在運行之間混合保留的 `crypto.sm_intrinsics` 值會導致性能讀數不一致。在實驗票證中記錄預期的覆蓋，並在捕獲新基線之前重置配置。
3. **CI 奇偶校驗：** 一些 macOS 運行程序在 NEON 處於活動狀態時不允許基於計數器的性能採樣。將 `scripts/sm_perf_capture_helper.sh` 輸出附加到 PR，以便審核者可以確認加速路徑已被執行，即使跑步者隱藏了這些計數器。
4. **未來 ISA 變體 (SVE/SVE2)：** 當前內核採用 NEON 通道形狀。在移植到 SVE/SVE2 之前，使用專用變體擴展 `sm_accel::NeonPolicy`，以便我們可以保持 CI、遙測和操作旋鈕保持一致。

SM-5a/SM-4c.1 下跟踪的行動項目確保 CI 捕獲每個新架構的奇偶校驗證明，並且路線圖保持在 🈺 直到 Neoverse/x86 基線和 NEON 與標量容差收斂。

## 合規性和監管說明

### 標準和規範性參考資料
- **GM/T 0002-2012** (SM4)、**GM/T 0003-2012** + **GB/T 32918 系列** (SM2)、**GM/T 0004-2012** + **GB/T 32905/32907** (SM3) 和 **RFC 8998** 管理我們的算法定義、測試向量和 KDF 綁定燈具消耗。 【docs/source/crypto/sm_vectors.md#L79】
- `docs/source/crypto/sm_compliance_brief.md` 中的合規簡介將這些標準與工程、SRE 和法律團隊的歸檔/導出責任交叉鏈接；每當 GM/T 目錄修訂時，請及時更新該摘要。

### 中國大陸監管工作流程
1. **產品備案（開發備案）：** 在從中國大陸運送支持 SM 的二進製文件之前，請將工件清單、確定性構建步驟和依賴項列表提交給省級密碼管理部門。歸檔模板和合規性檢查表位於 `docs/source/crypto/sm_compliance_brief.md` 和附件目錄（`sm_product_filing_template.md`、`sm_sales_usage_filing_template.md`、`sm_export_statement_template.md`）中。
2. **銷售/使用備案（銷售/使用備案）：** 在岸上運行支持 SM 的節點的運營商必須註冊其部署範圍、密鑰管理態勢和遙測計劃。歸檔時附上已簽名的清單以及 `iroha_sm_*` 指標快照。
3. **認可的測試：** 關鍵基礎設施運營商可能需要經過認證的實驗室報告。提供可重現的構建腳本、SBOM 導出和 Wycheproof/互操​​​​作工件（見下文），以便下游審核員可以在不更改代碼的情況下重現向量。
4. **狀態跟踪：** 在放行單和 `status.md` 中記錄已完成的備案；缺少備案會阻止從僅驗證試點升級到簽名試點。### 出口和分銷態勢
- 根據**美國 EAR 類別 5 第 2 部分**和**歐盟法規 2021/821 附件 1 (5D002)**，將支持 SM 的二進製文件視為受控項目。源代碼的發布仍然符合開源/ENC 豁免的資格，但重新分發到禁運目的地仍需要法律審查。
- 發布清單必須捆綁引用 ENC/TSU 基礎的導出語句，並列出 OpenSSL/Tongsuo 構建標識符（如果打包了 FFI 預覽版）。
- 當運營商需要陸上配送以避免跨境轉移問題時，優先選擇區域本地包裝（例如大陸鏡像）。

### 操作員文檔和證據
- 將該架構簡介與 `docs/source/crypto/sm_operator_rollout.md` 中的部署清單以及 `docs/source/crypto/sm_compliance_brief.md` 中的合規性歸檔指南配對。
- 使創世/操作員快速入門在 `docs/genesis.md`、`docs/genesis.he.md` 和 `docs/genesis.ja.md` 之間保持同步； SM2/SM3 CLI 工作流程是面向操作員的用於播種 `crypto` 清單的事實來源。
- 將 OpenSSL/Tongsuo 出處、`scripts/sm_openssl_smoke.sh` 輸出和 `scripts/sm_interop_matrix.sh` 奇偶校驗日誌與每個版本捆綁包一起存檔，以便合規性和審計合作夥伴擁有確定性的工件。
- 每當合規範圍發生變化（新的司法管轄區、備案完成或出口決定）時更新 `status.md`，以保持程序狀態可發現。
- 遵循 `docs/source/release_dual_track_runbook.md` 中捕獲的分階段準備情況審核 (`SM-RR1`–`SM-RR3`)；僅驗證階段、試點階段和 GA 簽名階段之間的升級需要此處列舉的工件。

## 互操作食譜

### RustCrypto ↔ OpenSSL/通索矩陣
1. 確保 OpenSSL/Tongsuo CLI 可用（`IROHA_SM_CLI="openssl /opt/tongsuo/bin/openssl"` 允許顯式工具選擇）。
2、運行`scripts/sm_interop_matrix.sh`；它調用 `cargo test -p iroha_crypto --test sm_cli_matrix --features sm` 並對每個提供商執行 SM2 簽名/驗證、SM3 摘要和 SM4 ECB/GCM 流程，跳過任何不存在的 CLI。 【scripts/sm_interop_matrix.sh#L1】
3. 將生成的 `target/debug/deps/sm_cli_matrix*.log` 文件與發布工件存檔。

### OpenSSL 預覽煙霧（打包門）
1. 安裝 OpenSSL ≥3.0 開發標頭並確保 `pkg-config` 可以找到它們。
2、執行`scripts/sm_openssl_smoke.sh`；助手運行 `cargo check`/`cargo test --test sm_openssl_smoke`，通過 FFI 後端執行 SM3 哈希、SM2 驗證和 SM4-GCM 往返（測試工具顯式啟用預覽）。 【scripts/sm_openssl_smoke.sh#L1】
3. 將任何不可跳過的失敗視為釋放阻塞；捕獲控制台輸出以獲取審計證據。

### 確定性夾具刷新
- 在每次合規性歸檔之前重新生成 SM 裝置（`sm_vectors.md`、`fixtures/sm/…`），然後重新運行奇偶校驗矩陣和煙霧控制裝置，以便審核員在歸檔的同時收到新的確定性記錄。## 外部審計準備
- `docs/source/crypto/sm_audit_brief.md` 打包了外部審查的背景、範圍、時間表和聯繫人。
- 審計工件位於 `docs/source/crypto/attachments/`（OpenSSL 煙霧日誌、貨物樹快照、貨物元數據導出、工具包出處）和 `fuzz/sm_corpus_manifest.json`（源自現有回歸向量的確定性 SM 模糊種子）下。在 macOS 上，煙霧日誌當前記錄了跳過的運行，因為工作區依賴循環阻止了 `cargo check`；沒有循環的 Linux 構建將充分利用預覽後端。
- 於 2026 年 1 月 30 日分發給加密工作組、平台運營、安全和 Docs/DevRel 領導，以便在 RFQ 發送之前進行協調。

### 審計參與狀態

- **Trail of Bits（CN 密碼學實踐）** — 工作說明書於 **2026-02-21** 執行，啟動 **2026-02-24**，現場工作窗口 **2026-02-24–2026-03-22**，最終報告到期 **2026-04-15**。每週三 09:00UTC 與加密貨幣工作組領導和安全工程聯絡員進行每週狀態檢查。有關聯繫人、可交付成果和證據附件，請參閱 [`sm_audit_brief.md`](sm_audit_brief.md#engagement-status)。
- **NCC Group APAC（應急時段）** — 保留 2026 年 5 月的窗口期，作為後續/並行審查，以防其他調查結果或監管機構要求需要第二意見。參與細節和升級掛鉤與 `sm_audit_brief.md` 中的 Trail of Bits 條目一起記錄。

## 風險與緩解措施

完整寄存器：詳見[`sm_risk_register.md`](sm_risk_register.md)
概率/影響評分、監控觸發因素和簽核歷史記錄。的
下面的摘要跟踪了發布工程中出現的標題項目。
|風險|嚴重性 |業主|緩解措施 |
|------|----------|--------|------------|
| RustCrypto SM 板條箱缺乏外部審計 |高|加密工作組 | Bits/NCC 集團的合約追踪，在審計報告被接受之前保持僅驗證狀態。 |
|跨 SDK 的確定性隨機數回歸 |高| SDK 項目負責人 |跨 SDK CI 共享固定裝置；強制執行規範的 r∥s 編碼；添加跨 SDK 集成測試（在 SM-3c 中跟踪）。 |
|內在函數中特定於 ISA 的錯誤 |中等|績效工作組 |功能門內在函數，需要 ARM 上的 CI 覆蓋，維護軟件回退。硬件驗證矩陣維護在 `sm_perf.md` 中。 |
|合規性模糊性阻礙了採用 |中等|文件和法律聯絡 |在 GA 之前發布合規簡介和操作員清單 (SM-6a/SM-6b)；收集法律意見。 `sm_compliance_brief.md` 中提供的歸檔清單。 |
| FFI 後端隨提供商更新而變化 |中等|平台運營|固定提供商版本、添加奇偶校驗測試、保持 FFI 後端選擇加入直至封裝穩定 (SM-P3)。 |## 未決問題/後續行動
1. 選擇具有 Rust SM 算法經驗的獨立審計合作夥伴。
   - **答复（2026-02-24）：** Trail of Bits 的 CN 密碼學實踐簽署了初步審計 SOW（於 2026 年 2 月 24 日啟動，於 2026 年 4 月 15 日交付），並且 NCC Group APAC 擁有 5 月份的應急時段，以便監管機構可以在不重新開放採購的情況下請求第二次審查。參與範圍、聯繫人和清單位於 [`sm_audit_brief.md`](sm_audit_brief.md#engagement-status) 中，並鏡像在 `sm_audit_vendor_landscape.md` 中。
2. 繼續向上游追踪官方 Wycheproof SM2 數據集；該工作區目前提供了一個精心策劃的 52 個案例套件（確定性夾具 + 合成篡改案例）並將其輸入 `sm2_wycheproof.rs`/`sm2_fuzz.rs`。上游 JSON 落地後，通過 `cargo xtask sm-wycheproof-sync` 更新語料庫。
   - 跟踪 Bouncy Castle 和 GmSSL 負矢量套件；許可清除後導入 `sm2_fuzz.rs` 以補充現有語料庫。
3. 定義 SM 採用監控的基線遙測（指標、日誌記錄）。
4. 決定 Kotodama/VM 曝光的 SM4 AEAD 默認為 GCM 還是 CCM。
5. 跟踪附件示例 1 (ID `ALICE123@YAHOO.COM`) 的 RustCrypto/OpenSSL 奇偶校驗：確認庫支持已發布的公鑰和 `(r, s)`，以便裝置可以升級到回歸測試。

## 行動項目
- [x] 在安全跟踪器中完成依賴性審計和捕獲。
- [x] 確認 RustCrypto SM 箱的審計合作夥伴參與（SM-P0 後續）。 Trail of Bits（CN 密碼學實踐）擁有主要審查權，啟動/交付日期記錄在 `sm_audit_brief.md` 中，NCC Group APAC 保留了 2026 年 5 月的應急時段，以滿足監管機構或治理的後續要求。
- [x] 擴大 SM4 CCM 防篡改案例 (SM-4a) 的 Wycheproof 覆蓋範圍。
- [x] 在 SDK 中引入規範的 SM2 簽名裝置並連接到 CI (SM-3c/SM-1b.1)；由 `scripts/check_sm2_sdk_fixtures.py` 保護（參見 `ci/check_sm2_sdk_fixtures.sh`）。

## 合規性附錄（國家商業密碼）

- **分類：** SM2/SM3/SM4 遵循中國*國家商業密碼*制度（《中華人民共和國密碼法》第 3 條）。在 Iroha 軟件中傳輸這些算法並不**將項目置於核心/公共（國家機密）層，但在 PRC 部署中使用它們的運營商必須遵守商業加密備案和 MLPS 義務。 【docs/source/crypto/sm_chinese_crypto_law_brief.md:14】
- **標準沿襲：** 將公共文檔與 GM/T 規範的官方 GB/T 轉換保持一致：

|算法| GB/T參考| GM/T原點|筆記|
|------------|----------------|-------------|--------|
| SM2 | GB/T32918（所有部分） | GM/T0003 | ECC數字簽名+密鑰交換； Iroha 向 SDK 公開核心節點中的驗證和確定性簽名。 |
| SM3 | GB/T32905| GM/T0004 | 256 位哈希；跨標量和 ARMv8 加速路徑的確定性哈希。 |
| SM4 | GB/T32907| GM/T0002 | 128位分組密碼； Iroha 提供 GCM/CCM 幫助程序並確保跨實現的大端奇偶校驗。 |- **功能清單：** Torii `/v1/node/capabilities` 端點通告以下 JSON 形狀，以便操作員和工具可以以編程方式使用 SM 清單：

```json
{
  "abi_version": 1,
  "data_model_version": 1,
  "crypto": {
    "sm": {
      "enabled": true,
      "default_hash": "sm3-256",
      "allowed_signing": ["ed25519"],
      "sm2_distid_default": "1234567812345678",
      "openssl_preview": false,
      "acceleration": {
        "scalar": true,
        "neon_sm3": false,
        "neon_sm4": false,
        "policy": "auto"
      }
    }
  }
}
```

CLI 子命令 `iroha runtime capabilities` 在本地顯示相同的負載，打印一行摘要以及 JSON 廣告以收集合規性證據。

- **文檔交付成果：**發布識別上述算法/標準的發行說明和 SBOM，並將完整的合規簡介 (`sm_chinese_crypto_law_brief.md`) 與發布工件捆綁在一起，以便運營商可以將其附加到省級備案中。 【docs/source/crypto/sm_chinese_crypto_law_brief.md:59】
- **操作員交接：**提醒部署者MLPS2.0/GB/T39786-2021需要加密應用評估、SM密鑰管理SOP和≥6年的證據保留；讓他們查看合規簡報中的運營商清單。 【docs/source/crypto/sm_chinese_crypto_law_brief.md:43】【docs/source/crypto/sm_chinese_crypto_law_brief.md:74】

## 溝通計劃
- **觀眾：** Crypto WG 核心成員、發布工程、安全審查委員會、SDK 項目負責人。
- **工件：** `sm_program.md`、`sm_lock_refresh_plan.md`、`sm_vectors.md`、`sm_wg_sync_template.md`、路線圖摘錄（SM-0 .. SM-7a）。
- **渠道：** 每週加密工作組同步議程 + 後續電子郵件總結行動項目並請求批准鎖定刷新和依賴項獲取（草案於 2025 年 1 月 19 日分發）。
- **所有者：** 加密貨幣工作組領導（可接受代表）。