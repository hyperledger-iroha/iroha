---
lang: zh-hant
direction: ltr
source: docs/source/crypto/dependency_audits.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 04e4cf26ed0ce9f9782be8aae9d16425a7a87fdbd1986cbcbca68a27ba0a3afe
source_last_modified: "2025-12-29T18:16:35.939138+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# 加密依賴審計

## Streebog（`streebog` 箱子）

- **樹中的版本：** `0.11.0-rc.2` 在 `vendor/streebog` 下銷售（啟用 `gost` 功能時使用）。
- **消費者：** `crates/iroha_crypto::signature::gost`（HMAC-Streebog DRBG + 消息哈希）。
- **狀態：** 僅限發布候選版本。目前沒有非 RC 板條箱提供所需的 API 表面，
  因此，我們在樹內鏡像板條箱以實現可審計性，同時跟踪上游的最終版本。
- **審查檢查點：**
  - 通過 Wycheproof 套件和 TC26 裝置驗證哈希輸出
    `cargo test -p iroha_crypto --features gost`（參見 `crates/iroha_crypto/tests/gost_wycheproof.rs`）。
  - `cargo bench -p iroha_crypto --bench gost_sign --features gost`
    與電流相關性的每條 TC26 曲線一起練習 Ed25519/Secp256k1。
  - `cargo run -p iroha_crypto --bin gost_perf_check --features gost`
    將較新的測量值與簽入的中位數進行比較（在 CI 中使用 `--summary-only`，添加
    重新設定基線時為 `--write-baseline crates/iroha_crypto/benches/gost_perf_baseline.json`）。
  - `scripts/gost_bench.sh`包裹工作台+檢查流程；傳遞 `--write-baseline` 來更新 JSON。
    有關端到端工作流程，請參閱 `docs/source/crypto/gost_performance.md`。
- **緩解措施：** `streebog` 僅通過將密鑰歸零的確定性包裝器調用；
  簽名者用操作系統熵對沖隨機數，以避免災難性的 RNG 失敗。
- **下一步行動：** 關注 RustCrypto 的 streebog `0.11.x` 版本；一旦標籤落地，請處理
  升級為標準依賴項碰撞（驗證校驗和、檢查差異、記錄出處以及
  放下出售的鏡子）。