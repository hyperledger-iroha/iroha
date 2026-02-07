---
lang: zh-hant
direction: ltr
source: docs/source/crypto/sm_audit_vendor_landscape.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 0f39199767280be0fdd582301cdc3e8929497cf372a96f9f300e718f827000a7
source_last_modified: "2025-12-29T18:16:35.941305+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% SM 審核供應商情況
% Iroha 加密工作組
% 2026-02-12

# 概述

加密貨幣工作組需要一個由獨立審查員組成的常設委員會，他們
了解 Rust 密碼學和中國 GM/T (SM2/SM3/SM4) 標準。
本說明列出了具有相關參考文獻的公司並總結了審計
我們通常要求的範圍，以便提案請求 (RFP) 週期保持快速且
一致。

# 候選公司

## Trail of Bits（CN 密碼學實踐）

- 記錄的約定：2023 年螞蟻集團通所安全審查
  （支持 SM 的 OpenSSL 發行版）和基於 Rust 的重複審核
  Diem/Libra、Sui 和 Aptos 等區塊鏈。
- 優點：專門的 Rust 密碼學團隊、自動化恆定時間
  分析工具、驗證確定性執行和硬件的經驗
  派遣政策。
- 適用於Iroha：可以擴展當前的SM審核SOW或執行獨立
  重新測試；使用 Norito 夾具和 IVM 系統調用輕鬆操作
  表面。

## NCC 集團（亞太地區密碼服務）

- 記錄的約定：區域支付的 gm/T (SM) 代碼檢查
  網絡和 HSM 供應商；之前對 Parity Substrate、Polkadot 的 Rust 評論
  和 Libra 組件。
- 優勢：亞太地區大型工作台，雙語報告，整合能力
  通過深入的代碼審查進行合規性流程檢查。
- 適合 Iroha：非常適合第二意見評估或治理驅動
  與 Trail of Bits 發現一起進行驗證。

## 安比實驗室（北京）

- 記錄的參與：開源 `libsm` Rust 箱的維護者
  由 Nervos CKB 和 CITA 使用；審核了果米對 Nervos、Muta 和
  FISCO BCOS Rust 組件具有雙語交付內容。
- 優點：積極使用 Rust 交付 SM 原語的工程師，強
  財產測試能力，對國內合規性有深入的了解
  要求。
- 適合 Iroha：當我們需要能夠提供比較的審閱者時很有價值
  測試向量和實施指南以及結果。

## 慢霧安全（成都）

- 記錄的參與：Substrate/Polkadot Rust 安全審查，包括
  果米為中國運營商分叉； SM2/SM3/SM4錢包的例行評估
  以及交易所使用的橋接代碼。
- 優勢：以區塊鍊為中心的審計實踐、綜合事件響應、
  涵蓋核心協議代碼和操作員工具的指南。
- 適用於 Iroha：有助於驗證 SDK 奇偶性和操作接觸點
  除了核心板條箱之外。

## 長亭科技（QAX 404 安全實驗室）- 記錄參與：GmSSL/Tongsuo 強化和 SM2/SM3/ 的貢獻者
  境內金融機構SM4實施指南；成立
  Rust 審計實踐涵蓋 TLS 堆棧和加密庫。
- 優勢：深厚的密碼分析背景、配對形式驗證的能力
  人工審查的文物，長期的監管關係。
- 適用於 Iroha：適用於監管簽字或正式證明文物
  需要附有標準代碼審查報告。

# 典型的審計範圍和可交付成果

- **規範一致性：** 驗證 SM2 ZA 計算、簽名
  規範化、SM3 填充/壓縮以及 SM4 密鑰調度和 IV 處理
  對照GM/T 0003-2012、GM/T 0004-2012、GM/T 0002-2012。
- **確定性和恆定時間行為：** 檢查分支、查找
  表和硬件功能門（例如 NEON、SM4 指令）以確保
  Rust 和 FFI 調度在支持的硬件上保持確定性。
- **FFI 和提供商集成：** 審查 OpenSSL/Tongsuo 綁定，
  PKCS#11/HSM 適配器和用於共識安全的錯誤傳播路徑。
- **測試和夾具覆蓋範圍：** 評估模糊線束，Norito 往返，
  確定性煙霧測試，並建議差異測試
  出現。
- **依賴性和供應鏈審查：**確認構建來源、供應商
  補丁策略、SBOM 準確性和可重複的構建指令。
- **文檔和操作：** 驗證操作員操作手冊、合規性
  簡介、默認配置和回滾過程。
- **報告預期：** 帶有風險評級的執行摘要，詳細
  包含代碼參考和補救指南、重新測試計劃的調查結果，以及
  涵蓋決定論保證的證明。

# 後續步驟

- 在詢價週期中使用此供應商名冊；將上面的範圍清單調整為
  在發出 RFP 之前匹配有效的 SM 里程碑。
- 在 `docs/source/crypto/sm_audit_brief.md` 中記錄參與結果以及
  合同執行後，`status.md` 中的表面狀態會更新。