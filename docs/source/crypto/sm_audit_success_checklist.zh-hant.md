---
lang: zh-hant
direction: ltr
source: docs/source/crypto/sm_audit_success_checklist.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 624ef9305dc14d477a616923c80445094c692bc6a38d69465f679b54ccd52e92
source_last_modified: "2025-12-29T18:16:35.940844+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% SM2/SM3/SM4 審核成功標準
% Iroha 加密工作組
% 2026-01-30

# 目的

該清單列出了成功所需的具體標準
完成SM2/SM3/SM4外部審核。應在期間進行審查
啟動，在每個狀態檢查點重新訪問，並用於確認退出
為生產驗證器啟用 SM 簽名之前的條件。

# 參與前準備

- [ ] 已簽署合同，包括範圍、可交付成果、保密性和
      修復支持語言。
- [ ] 審核團隊接收存儲庫鏡像訪問權限、CI 工件存儲桶以及
      `docs/source/crypto/sm_audit_brief.md` 中列出的文檔包。
- [ ] 通過每個角色的備份確認聯繫點
      （加密貨幣、IVM、平台操作、安全性、文檔）。
- [ ] 內部利益相關者就目標發布日期進行協調並凍結窗口。
- [ ] SBOM 導出（`cargo auditable` + CycloneDX）生成並共享。
- [ ] OpenSSL/Tongsuo 構建出處包已準備好
      （源 tarball 哈希、構建腳本、可重複性說明）。
- [ ] 捕獲的最新確定性測試輸出：
      `scripts/sm_openssl_smoke.sh`、`cargo test -p iroha_crypto sm` 和
      Norito 往返夾具。
- [ ] Torii `/v2/node/capabilities` 廣告（通過 `iroha runtime capabilities`）記錄，驗證 `crypto.sm` 清單字段和加速策略快照。

# 參與執行

- [ ] 啟動研討會在對目標的共同理解下完成，
      時間表和溝通節奏。
- [ ] 收到並分類的每週狀態報告；風險登記冊已更新。
- [ ] 在發現嚴重性時在一個工作日內通報調查結果
      為高或嚴重。
- [ ] 審計團隊驗證 ≥2 個 CPU 架構（x86_64、
      aarch64）具有匹配的輸出。
- [ ] 旁道審查包括恆定時間證明或經驗測試
      Rust 和 FFI 路徑的證據。
- [ ] 合規性和文件審查確認操作員指南相符
      監管義務。
- [ ] 針對參考實現的差異測試（RustCrypto、
      OpenSSL/Tongsuo）在審計員監督下執行。
- [ ] 毛絨線束評估；在存在差距的地方提供新的種子語料庫。

# 修復和退出

- [ ] 所有發現均按嚴重性、影響、可利用性和
      建議的修復步驟。
- [ ] 高/關鍵問題得到審計員批准的補丁或緩解措施
      驗證；記錄殘餘風險。
- [ ] 審核員提供重新測試驗證，證明已解決的問題（差異、測試
      運行，或簽名證明）。
- [ ] 交付的最終報告：執行摘要、詳細調查結果、方法、
      決定論判決、順從判決。
- [ ] 內部簽署會議總結後續步驟、發布調整、
      和文檔更新。
- [ ] `status.md` 更新了審核結果和未完成的補救措施
      後續行動。
- [ ] `docs/source/crypto/sm_program.md` 中捕獲的事後分析（教訓
      學習了，未來的強化任務）。