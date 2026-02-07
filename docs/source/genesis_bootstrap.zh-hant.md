---
lang: zh-hant
direction: ltr
source: docs/source/genesis_bootstrap.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6feb2b03bd8f6a41de693a0c3f3c4ffc058072bc7942e2bc50b3fd9770aa56d4
source_last_modified: "2025-12-29T18:16:35.962003+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# 來自值得信賴的同行的 Genesis Bootstrap

沒有本地 `genesis.file` 的 Iroha 對等點可以從受信任的對等點獲取簽名的創世塊
使用 Norito 編碼的引導協議。

- **協議：**對等方交換 `GenesisRequest`（`Preflight` 用於元數據，`Fetch` 用於有效負載）和
  由 `request_id` 鍵控的 `GenesisResponse` 幀。響應者包括鏈 ID、簽名者公鑰、
  哈希值和可選的大小提示；僅在 `Fetch` 上返回有效負載，並且重複的請求 ID
  接收 `DuplicateRequest`。
- **守衛：** 響應者強制執行白名單（`genesis.bootstrap_allowlist` 或受信任的對等方
  設置）、鏈 ID/公鑰/哈希匹配、速率限制 (`genesis.bootstrap_response_throttle`) 和
  尺寸上限 (`genesis.bootstrap_max_bytes`)。允許列表之外的請求收到 `NotAllowed`，並且
  由錯誤密鑰簽名的有效負載收到 `MismatchedPubkey`。
- **請求者流程：** 當存儲為空並且 `genesis.file` 未設置時（並且
  `genesis.bootstrap_enabled=true`)，節點使用可選的選項預檢受信任的對等點
  `genesis.expected_hash`，然後獲取有效負載，通過`validate_genesis_block`驗證簽名，
  並在應用該塊之前將 `genesis.bootstrap.nrt` 與 Kura 一起保留。引導重試
  榮譽 `genesis.bootstrap_request_timeout`、`genesis.bootstrap_retry_interval` 和
  `genesis.bootstrap_max_attempts`。
- **失敗模式：** 由於白名單未命中、鏈/公鑰/哈希不匹配、大小而拒絕請求
  違反上限、速率限制、缺少本地起源或重複的請求 ID。衝突的哈希值
  跨對等方中止獲取；沒有響應者/超時回退到本地配置。
- **操作員步驟：** 確保至少有一個受信任的對等點可以通過有效的創世到達，配置
  `bootstrap_allowlist`/`bootstrap_max_bytes`/`bootstrap_response_throttle` 和重試旋鈕，以及
  可選擇引腳 `expected_hash` 以避免接受不匹配的有效負載。持久有效負載可以是
  通過將 `genesis.file` 指向 `genesis.bootstrap.nrt` 在後續引導中重用。