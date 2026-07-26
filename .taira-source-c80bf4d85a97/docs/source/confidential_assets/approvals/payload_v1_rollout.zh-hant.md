---
lang: zh-hant
direction: ltr
source: docs/source/confidential_assets/approvals/payload_v1_rollout.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5fa5e39b0e758b38e27855fcfcae9a6e31817df4fdb9d5394b4b63d2f5164516
source_last_modified: "2026-01-22T14:35:37.742189+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//！ Payload v1 推出批准（SDK 委員會，2026-04-28）。
//！
//！捕獲 `roadmap.md:M1` 所需的 SDK 委員會決策備忘錄，以便
//！加密有效負載 v1 部署具有可審核記錄（可交付 M1.4）。

# Payload v1 推出決定 (2026-04-28)

- **主席：** SDK 委員會負責人 (M. Takemiya)
- **投票成員：** Swift Lead、CLI 維護者、Confidential Assets TL、DevRel WG
- **觀察員：** 計劃管理、遙測操作

## 已審核的輸入

1. **Swift 綁定和提交者** — `ShieldRequest`/`UnshieldRequest`、異步提交者和 Tx 構建器助手通過奇偶校驗測試和文檔登陸。 【IrohaSwift/Sources/IrohaSwift/TxBuilder.swift:389】【IrohaSwift/Sources/IrohaSwift/TxBuilder.swift:1006】
2. **CLI 人體工程學** — `iroha app zk envelope` 幫助程序涵蓋編碼/檢查工作流程以及故障診斷，符合路線圖人體工程學要求。 【crates/iroha_cli/src/zk.rs:1256】
3. **確定性固定裝置和奇偶校驗套件** - 共享固定裝置 + Rust/Swift 驗證以保留 Norito 字節/錯誤表面對齊。 【fixtures/confidential/encrypted_payload_v1.json:1】【crates/iroha_data_model/tests/confidential_encrypted_payload_vectors.rs:1】【IrohaSwift/Tests/IrohaSwiftTests/ConfidentialEncryptedPayloadTests.swift:73】

## 決定

- **批准 SDK 和 CLI 的有效負載 v1 推出**，使 Swift 錢包能夠在無需定制管道的情況下生成機密信封。
- **條件：** 
  - 將奇偶校驗裝置置於 CI 漂移警報之下（與 `scripts/check_norito_bindings_sync.py` 相關）。
  - 在 `docs/source/confidential_assets.md` 中記錄操作手冊（已通過 Swift SDK PR 更新）。
  - 在翻轉任何生產標誌之前記錄校準+遙測證據（在 M2 下跟踪）。

## 行動項目

|業主|項目 |到期|
|--------|------|-----|
|迅速領先 |宣布 GA 可用性 + 自述文件片段 | 2026-05-01 |
| CLI 維護者 |添加 `iroha app zk envelope --from-fixture` 幫助程序（可選）|積壓（不阻塞）|
|開發相關工作組 |使用有效負載 v1 說明更新錢包快速入門 | 2026-05-05 |

> **注：** 本備忘錄取代 `roadmap.md:2426` 中臨時的“等待理事會批准”標註，並滿足跟踪器項目 M1.4。每當後續行動項目結束時更新 `status.md`。