---
lang: zh-hant
direction: ltr
source: docs/source/crypto/sm_audit_brief.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9cda4648f0af7f89022e9d9f4ea243bc22685d9356927bbf1417c77b2057d872
source_last_modified: "2025-12-29T18:16:35.940439+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% SM2/SM3/SM4 外部審計簡介
% Iroha 加密工作組
% 2026-01-30

# 概述

本簡介包含了所需的工程和合規環境
Iroha 的 SM2/SM3/SM4 支持的獨立審查。它針對的是審計團隊
具有 Rust 密碼學經驗並熟悉中國國家
密碼學標準。預期結果是一份書面報告，內容涵蓋
實施風險、一致性差距和優先修復指導
在 SM 推出之前從預覽轉向生產。

# 程序快照

- **發布範圍：** Iroha 2/3 共享代碼庫，確定性驗證
  跨節點和 SDK 的路徑，可在配置保護後面進行簽名。
- **當前階段：** SM-P3.2（OpenSSL/Tongsuo 後端集成）與 Rust
  實現已經交付用於驗證和對稱用例。
- **目標決策日期：** 2026-04-30（審核結果告知是否繼續
  在驗證器構建中啟用 SM 簽名）。
- **跟踪的關鍵風險：** 第三方依賴譜系、確定性
  混合硬件下的行為、運營商合規性準備情況。

# 代碼和夾具參考

- `crates/iroha_crypto/src/sm.rs` — Rust 實現和可選的 OpenSSL
  綁定（`sm-ffi-openssl` 功能）。
- `crates/ivm/tests/sm_syscalls.rs` — IVM 哈希系統調用覆蓋率，
  驗證和對稱模式。
- `crates/iroha_data_model/tests/sm_norito_roundtrip.rs` — Norito 有效負載
  SM文物往返。
- `docs/source/crypto/sm_program.md` — 程序歷史記錄、依賴性審計和
  推出護欄。
- `docs/source/crypto/sm_operator_rollout.md` — 面向操作員的支持和
  回滾程序。
- `docs/source/crypto/sm_compliance_brief.md` — 監管摘要和導出
  考慮因素。
- `scripts/sm_openssl_smoke.sh` / `crates/iroha_crypto/tests/sm_openssl_smoke.rs`
  — 用於 OpenSSL 支持的流的確定性煙霧控制。
- `fuzz/sm_*` 語料庫 — 基於 RustCrypto 的模糊種子，涵蓋 SM3/SM4 原語。

# 請求的審核範圍1. **規格符合性**
   - 驗證SM2簽名驗證、ZA計算和規範
     編碼行為。
   - 確認SM3/SM4原語遵循GM/T 0002-2012和GM/T 0007-2012，
     包括計數器模式不變量和 IV 處理。
2. **確定性和恆定時間保證**
   - 檢查分支、表查找和硬件調度以便節點執行
     跨 CPU 系列保持確定性。
   - 評估私鑰操作的恆定時間聲明並確認
     OpenSSL/Tongsuo 路徑保留恆定時間語義。
3. **側通道和故障分析**
   - 檢查 Rust 和 中的時序、緩存和電源側通道風險
     FFI 支持的代碼路徑。
   - 評估簽名驗證的故障處理和錯誤傳播
     驗證加密失敗。
4. **構建、依賴性和供應鏈審查**
   - 確認 OpenSSL/Tongsuo 工件的可重複構建和出處。
   - 審查依賴樹許可和審計覆蓋範圍。
5. **測試和驗證利用批評**
   - 評估確定性煙霧測試、模糊測試和 Norito 裝置。
   - 建議額外的覆蓋範圍（例如差異測試、基於屬性的測試）
     證明）如果仍然存在差距。
6. **合規性和操作員指南驗證**
   - 根據法律要求和預期交叉檢查裝運文件
     操作員控制。

# 交付成果和物流

- **開球：** 2026 年 2 月 24 日（虛擬，90 分鐘）。
- **採訪：** 加密貨幣工作組、IVM 維護者、平台操作員（根據需要）。
- **Artefact 訪問：** 只讀存儲庫鏡像、CI 管道日誌、固定裝置
  輸出和依賴 SBOM (CycloneDX)。
- **臨時更新：**每周書面狀態+風險標註。
- **最終交付成果（截至 2026 年 4 月 15 日）：**
  - 帶有風險評級的執行摘要。
  - 詳細的調查結果（每個問題：影響、可能性、代碼參考、
    補救指導）。
  - 重新測試/驗證計劃。
  - 關於決定論、恆定時間姿態和合規性調整的聲明。

## 參與狀態

|供應商|狀態 |開球 |現場窗口|筆記|
|--------|--------|----------|----------------|--------|
| Trail of Bits（中文練習）| 2026年2月21日執行的工作說明書 | 2026-02-24 | 2026-02-24–2026-03-22 |交貨截止日期為2026年4月15日；張輝作為工程同行領導與 Alexey M. 的合作。每週狀態通話週三 09:00 UTC。 |
| NCC 集團（亞太地區）|預留應急時段 |不適用（暫停）|暫定 2026-05-06–2026-05-31 |僅當高風險發現需要第二次時才激活； Priya N.（安全）和 NCC 集團參與部門於 2026 年 2 月 22 日確認了準備情況。 |

# 外展包中包含的附件- `docs/source/crypto/sm_program.md`
- `docs/source/crypto/sm_operator_rollout.md`
- `docs/source/crypto/sm_compliance_brief.md`
- `docs/source/crypto/sm_lock_refresh_plan.md`
- `docs/source/crypto/sm_rust_vector_check.md`
- `docs/source/crypto/attachments/sm_iroha_crypto_tree.txt` — `cargo tree -p iroha_crypto --no-default-features --features "sm sm-ffi-openssl"` 快照。
- `docs/source/crypto/attachments/sm_iroha_crypto_metadata.json` — `cargo metadata` 導出 `iroha_crypto` 箱（鎖定的依賴關係圖）。
- `docs/source/crypto/attachments/sm_openssl_smoke.log` — 最新的 `scripts/sm_openssl_smoke.sh` 運行（缺少提供商支持時跳過 SM2/SM4 路徑）。
- `docs/source/crypto/attachments/sm_openssl_provenance.md` — 本地工具包出處（pkg-config/OpenSSL 版本說明）。
- 模糊語料庫清單 (`fuzz/sm_corpus_manifest.json`)。

> **環境警告：** 當前的開發快照使用供應商的 OpenSSL 3.x 工具鏈（`openssl` crate `vendored` 功能），但 macOS 缺乏 SM3/SM4 CPU 內在函數，並且默認提供程序不公開 SM4-GCM，因此 OpenSSL 煙霧線束仍然跳過 SM4 覆蓋和附件示例 SM2 解析。工作區依賴循環 (`sorafs_manifest ↔ sorafs_car`) 還會強制幫助程序腳本在發出 `cargo check` 故障後跳過運行。在 Linux 發布構建環境（啟用了 SM4 且沒有循環的 OpenSSL/Tongsuo）中重新運行捆綁包，以在外部審計之前捕獲完整的奇偶校驗。

# 候選審核合作夥伴及範圍

|公司|相關經驗|典型範圍和可交付成果 |筆記|
|------|---------------------------------|--------------------------------------------|-----|
| Trail of Bits（CN密碼學實踐）| Rust 代碼審查（`ring`、zkVM）、移動支付堆棧的先前 GM/T 評估。 |規範一致性差異 (GM/T 0002/3/4)、Rust + OpenSSL 路徑的恆定時間審查、差異模糊測試、供應鏈審查、修復路線圖。 |已經訂婚了；在規劃未來的刷新周期時保留該表是為了完整性。 |
| NCC 集團亞太區 |硬件/SOC + Rust 密碼學紅隊，發表了 RustCrypto 原語和支付 HSM 橋的評論。 | Rust + JNI/FFI 綁定的整體評估、確定性策略驗證、性能/遙測門審查、操作手冊演練。 |保留作為應急措施；還可以為中國監管機構提供雙語報告。 |
| Kudelski Security（區塊鍊和加密貨幣團隊）|對 Halo2、Mina、zkSync、Rust 中實現的自定義簽名方案的審核。 |重點關注橢圓曲線正確性、轉錄完整性、硬件加速的威脅建模以及 CI/推出證據。 |對於有關硬件加速 (SM-5a) 和 FASTPQ 到 SM 交互的第二意見很有用。 |
|最小權限|基於 Rust 的區塊鏈（Filecoin、Polkadot）的加密協議審計，可重現的構建諮詢。 |確定性構建驗證、Norito 編解碼器驗證、合規證據交叉檢查、運營商通信審查。 |當監管機構要求代碼審查之外的獨立驗證時，非常適合提供透明度/審計報告。 |

所有約定都要求上面列舉的相同工件捆綁包以及以下可選附加組件（具體取決於公司）：- **規範一致性和確定性行為：** SM2 ZA 推導、SM3 填充、SM4 輪函數和 `sm_accel` 運行時調度門的逐行驗證，以確保加速永遠不會改變語義。
- **側通道和 FFI 審查：** 檢查恆定時間聲明、不安全代碼塊和 OpenSSL/Tongsuo 橋接層，包括針對 Rust 路徑的差異測試。
- **CI/供應鏈驗證：** `sm_interop_matrix`、`sm_openssl_smoke` 和 `sm_perf` 的複制與 SBOM/SLSA 證明一起使用，因此審計結果可以直接與發布證據聯繫起來。
- **面向運營商的抵押品：** 交叉檢查 `sm_operator_rollout.md`、合規性歸檔模板和遙測儀表板，以確認文檔中承諾的緩解措施在技術上是可執行的。

在確定未來審核的範圍時，請重用此表，以使供應商優勢與特定路線圖里程碑保持一致（例如，支持 Kudelski 進行硬件/性能重度發布，支持 Trail of Bits 來支持語言/運行時正確性，支持 Least Authority 來確保可重現的構建保證）。

# 聯絡點

- **技術所有者：** Crypto WG 負責人 (Alexey M., `alexey@iroha.tech`)
- **項目經理：** 平台運營協調員（Sarah K.，
  `sarah@iroha.tech`)
- **安全聯絡員：** 安全工程 (Priya N., `security@iroha.tech`)
- **文檔聯絡員：** Docs/DevRel 負責人（Jamila R.，
  `docs@iroha.tech`)