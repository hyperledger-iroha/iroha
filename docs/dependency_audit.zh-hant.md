---
lang: zh-hant
direction: ltr
source: docs/dependency_audit.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: cb0a770fac1086462d949dbf17dd5a05f133169e57d50b0d90ddb48ae05f2853
source_last_modified: "2026-01-05T09:28:11.822642+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//！依賴性審計摘要

日期：2025-09-01

範圍：工作區範圍內對 Cargo.toml 文件中聲明並在 Cargo.lock 中解決的所有 crate 進行審查。針對 RustSec 諮詢數據庫進行貨物審計，並手動審查包的合法性和算法的“主包”選擇。

工具/命令運行：
- `cargo tree -d --workspace --locked --offline` – 檢查重複版本
- `cargo audit` – 掃描 Cargo.lock 是否存在已知漏洞和拉扯的板條箱

發現安全公告（現在 0 個漏洞；2 個警告）：
- 橫梁通道 — RUSTSEC-2025-0024
  - 修正：在 `crates/ivm/Cargo.toml` 中撞到 `0.5.15`。

  - 修正：在 `crates/iroha_torii/Cargo.toml` 中將 `pprof` 翻轉為 `prost-codec`。

- 環 — RUSTSEC-2025-0009
  - 修復：增加了 QUIC/TLS 堆棧（`quinn 0.11`、`rustls 0.23`、`tokio-rustls 0.26`）並將 WS 堆棧更新為 `tungstenite/tokio-tungstenite 0.24`。通過 `cargo update -p ring --precise 0.17.12` 強制鎖定 `ring 0.17.12`。

其余建議：無。剩餘警告：`backoff`（未維護）、`derivative`（未維護）。

合法性和“主要箱子”評估（焦點）：
- 哈希：`sha2` (RustCrypto)、`blake2` (RustCrypto)、`tiny-keccak`（廣泛使用）——規範選擇。
- AEAD/對稱：`aes-gcm`、`chacha20poly1305`、`aead` 特徵 (RustCrypto) — 規範。
- 簽名/ECC：`ed25519-dalek`、`x25519-dalek`（dalek 項目）、`k256`（RustCrypto）、`secp256k1`（libsecp 綁定）——全部合法；更喜歡單個 secp256k1 堆棧（`k256` 對於純 Rust 或 `secp256k1` 對於 libsecp）以減少表面積。
- BLS12-381/ZK：`blstrs`、`halo2_*` — 廣泛應用於生產 ZK 生態系統；合法的。
- PQ：`pqcrypto-dilithium`、`pqcrypto-traits` — 合法參考包。
- TLS：`rustls`、`tokio-rustls`、`hyper-rustls` — 規範的現代 Rust TLS 堆棧。
- 噪聲：`snow` — 規範實現。
- 序列化：`parity-scale-codec` 是 SCALE 的規範。 Serde 已從整個工作區的生產依賴關係中刪除； Norito 派生/編寫器涵蓋每個運行時路徑。任何殘留的 Serde 引用都存在於歷史文檔、護欄腳本或僅限測試的白名單中。
- FFI/庫：`libsodium-sys-stable`、`openssl` — 合法；在生產路徑中更喜歡 Rustls 而不是 OpenSSL（當前代碼已經這樣做了）。

建議：
-解決警告：
  - 考慮用 `retry`/`futures-retry` 或本地指數退避助手替換 `backoff`。
  - 將派生的 `derivative` 替換為手動實現或 `derive_more`（如果適用）。
- 中：盡可能統一 `k256` 或 `secp256k1`，以減少重複實現（僅在真正需要時才保留兩者）。
- Medium：查看 `poseidon-primitives 0.2.0` 的出處以了解 ZK 的使用情況；如果可行，請考慮與 Arkworks/Halo2 原生 Poseidon 實現保持一致，以盡量減少並行生態系統。

注意事項：
- `cargo tree -d` 顯示預期的重複主要版本（`bitflags` 1/2、多個 `ring`），其本身不會帶來安全風險，但會增加構建表面。
- 沒有觀察到類似拼寫錯誤的板條箱；所有名稱和來源都解析為知名的生態系統板條​​箱或內部工作區成員。
- 實驗性：添加了 `iroha_crypto` 功能 `bls-backend-blstrs`，以開始將 BLS 遷移到僅 blstrs 的後端（啟用後消除對 arkworks 的依賴）。默認保留 `w3f-bls` 以避免行為/編碼更改。對齊計劃：
  - 在 `crates/iroha_crypto/tests/bls_backend_compat.rs` 中添加往返固定裝置，一次導出密鑰並在兩個後端之間斷言相等，涵蓋 `SecretKey`、`PublicKey` 和簽名聚合。

後續行動（建議的工作項目）：
- 將 Serde 護欄保留在 CI 中（`scripts/check_no_direct_serde.sh`、`scripts/deny_serde_json.sh`），以便無法引入新的生產用途。

為此次審核進行的測試：
- 使用最新的諮詢數據庫運行 `cargo audit`；驗證了四個建議及其依賴關係樹。
- 搜索受影響板條箱的直接依賴聲明以查明修復位置。