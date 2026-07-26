---
lang: zh-hant
direction: ltr
source: docs/source/crypto/attachments/sm_product_filing_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e7116d28e32d8bd77434edd6767427cc3d2ae0624f4de132b1d0cec3c7d44b86
source_last_modified: "2025-12-29T18:16:35.938246+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% SM2/SM3/SM4 產品備案 (開發備案) 模板
% Hyperledger Iroha 合規工作組
% 2026-05-06

# 說明

向省級提交“產品開發備案”時使用此模板
分發前或市國家密碼管理局 (SCA) 辦公室
支持 SM 的二進製文件或來自中國大陸的源文物。更換
包含項目特定詳細信息的佔位符，如果滿足以下條件，請將完成的表格導出為 PDF：
要求，並附上清單中引用的文物。

# 1. 申請人和產品摘要

|領域|價值|
|--------|--------|
|組織名稱| {{ 組織 }} |
|註冊地址 | {{ 地址}} |
|法定代表人 | {{ LEGAL_REP }} |
|主要聯繫人（姓名/職務/電子郵件/電話）| {{ 聯繫 }} |
|產品名稱 | Hyperledger Iroha {{ RELEASE_NAME }} |
|產品版本/構建 ID | {{ 版本 }} |
|歸檔類型 |產品開發(開發備案) |
|申請日期 | {{ 年-月-日 }} |

# 2. 密碼學使用概述

- 支持的算法：`SM2`、`SM3`、`SM4`（在下面提供使用矩陣）。
- 使用上下文：
  |算法|組件|目的|確定性保障措施 |
  |----------|----------|---------|----------------------------|
  | SM2 | {{ 組件 }} | {{ 目的}} | RFC6979 + 規範 r∥ 執行 |
  | SM3 | {{ 組件 }} | {{ 目的}} |通過 `Sm3Digest` 進行確定性哈希 |
  | SM4 | {{ 組件 }} | {{ 目的}} |具有強制隨機數策略的 AEAD (GCM/CCM) |
- 構建中的非 SM 算法：{{ OTHER_ALGORITHMS }}（為了完整性）。

# 3. 開發和供應鏈控制

- 源代碼存儲庫：{{ REPOSITORY_URL }}
- 確定性構建說明：
  1.`git clone {{ REPOSITORY_URL }} && git checkout {{ COMMIT_SHA }}`
  2. `cargo build --workspace --locked --release --features "sm sm-ffi-openssl"`（根據需要調整）。
  3. 通過 `cargo auditable` / CycloneDX (`{{ SBOM_PATH }}`) 生成 SBOM。
- 持續集成環境總結：
  |項目 |價值|
  |------|--------|
  |構建操作系統/版本 | {{ BUILD_OS }} |
  |編譯器工具鏈 | {{ 工具鏈 }} |
  | OpenSSL/通索源碼| {{ OPENSSL_SOURCE }} |
  |再現性校驗和| {{ 校驗和 }} |

# 4. 密鑰管理和安全

- 默認啟用的 SM 功能：{{ DEFAULTS }}（例如，僅驗證）。
- 簽名所需的配置標誌：{{ CONFIG_FLAGS }}。
- 鑰匙保管方式：
  |項目 |詳情 |
  |------|---------|
  |密鑰生成工具| {{ KEY_TOOL }} |
  |存儲介質 | {{ STORAGE_MEDIUM }} |
  |備份策略| {{ BACKUP_POLICY }} |
  |訪問控制| {{ ACCESS_CONTROLS }} |
- 事件響應聯繫人 (24/7)：
  |角色 |名稱 |電話 |電子郵件 |
  |------|------|--------|--------|
  |加密貨幣領先 | {{ 姓名 }} | {{ 電話 }} | {{ 電子郵件 }} |
  |平台運營| {{ 姓名 }} | {{ 電話 }} | {{ 電子郵件 }} |
  |法律聯絡 | {{ 姓名 }} | {{ 電話 }} | {{ 電子郵件 }} |

# 5. 附件清單- [ ] 源代碼快照 (`{{ SOURCE_ARCHIVE }}`) 和哈希值。
- [ ] 確定性構建腳本/再現性註釋。
- [ ] SBOM (`{{ SBOM_PATH }}`) 和依賴項清單 (`Cargo.lock` 指紋)。
- [ ] 確定性測試成績單（`scripts/sm_openssl_smoke.sh`、`cargo test -p iroha_crypto sm`）。
- [ ] 遙測儀表板導出展示 SM 可觀察性。
- [ ] 出口控制聲明（參見單獨的模板）。
- [ ] 審計報告或第三方評估（如果已完成）。

# 6. 申請人聲明

> 我確認以上信息準確無誤，所披露的
> 加密功能符合適用的中國法律和法規，
> 並且該組織將保留提交的文物至少
> 三年。

- 簽名（法定代表人）：________________________
- 日期：___________________