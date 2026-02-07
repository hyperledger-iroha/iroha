---
lang: zh-hant
direction: ltr
source: docs/source/connect_architecture_strawman.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a1a6bcc6bca3d7f70b82e35734b71d706ac46d8dc9c728351fabbd8a61dd3f31
source_last_modified: "2026-01-05T09:28:12.003461+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# 連接會話架構 Strawman (Swift / Android / JS)

這個稻草人提案概述了 Nexus Connect 工作流程的共享設計
跨 Swift、Android 和 JavaScript SDK。其目的是支持
2026 年 2 月跨 SDK 研討會並在實施前捕獲懸而未決的問題。

> 最後更新: 2026-01-29  
> 作者：Swift SDK 主管、Android 網絡 TL、JS 主管  
> 狀態：供理事會審查的草案（2026 年 3 月 12 日添加威脅模型 + 數據保留調整）

## 目標

1. 調整錢包 ↔ dApp 會話生命週期，包括連接引導，
   批准、簽署請求和拆除。
2.定義所有人共享的Norito信封模式（打開/批准/簽署/控制）
   SDK 並確保與 `connect_norito_bridge` 同等。
3. 傳輸（WebSocket/WebRTC）、加密之間的職責劃分
   （Norito 連接框架+密鑰交換）和應用程序層（SDK 外觀）。
4. 確保跨桌面/移動平台的確定性行為，包括
   離線緩沖和重新連接。

## 會話生命週期（高級）

```
┌────────────┐      ┌─────────────┐      ┌────────────┐
│  dApp SDK  │←────→│  Connect WS │←────→│ Wallet SDK │
└────────────┘      └─────────────┘      └────────────┘
      │                    │                    │
      │ 1. open (app→wallet) frame (metadata, permissions, chain_id)
      │────────────────────────────────────────>│
      │                    │                    │
      │                    │ 2. route frame     │
      │                    │────────────────────│
      │                    │                    │
      │                    │     3. approve frame (wallet pk, account,
      │                    │        permissions, proof/attest)
      │<────────────────────────────────────────│
      │                    │                    │
      │ 4. sign request    │                    │
      │────────────────────────────────────────>│
      │                    │                    │
      │                    │ 5. sign result     │
      │                    │────────────────────│
      │                    │                    │
      │ 6. control frames for reject/close, error propagation, heartbeats.
```

## 信封/Norito 架構

所有 SDK 必須使用 `connect_norito_bridge` 中定義的規範 Norito 模式：

- `EnvelopeV1`（打開/批准/簽名/控制）
- `ConnectFrameV1`（帶有 AEAD 有效負載的密文幀）
- 控制代碼：
  - `open_ext`（元數據、權限）
  - `approve_ext`（賬戶、權限、證明、簽名）
  - `reject`、`close`、`ping/pong`、`error`

Swift 之前發布了佔位符 JSON 編碼器 (`ConnectCodec.swift`)。截至 2026 年 4 月 SDK
總是使用 Norito 橋，並且當 XCFramework 丟失時關閉失敗，但是這個稻草人
仍然抓住了導致橋樑集成的任務：

|功能|描述 |狀態 |
|----------|-------------|--------|
| `connect_norito_encode_control_open_ext` | dApp 開放框架 |在橋中實施 |
| `connect_norito_encode_control_approve_ext` |錢包審批 |已實施 |
| `connect_norito_encode_envelope_sign_request_tx/raw` |簽署請求 |已實施 |
| `connect_norito_encode_envelope_sign_result_ok/err` |簽署結果|已實施 |
| `connect_norito_decode_*` |解析錢包/dApp |已實施 |

### 所需工作

- Swift：用橋接調用和表面替換佔位符 `ConnectCodec` JSON 助手
  使用共享 Norito 類型的類型化包裝器（`ConnectFrame`、`ConnectEnvelope`）。 ✅（2026 年 4 月）
- Android/JS：確保存在相同的包裝器；對齊錯誤代碼和元數據鍵。
- 共享：具有一緻密鑰派生的文檔加密（X25519 密鑰交換，AEAD）
  根據 Norito 規範，並使用 Rust 橋提供示例集成測試。

## 運輸合同- 主要傳輸：WebSocket (`/v1/connect/ws?sid=<session_id>`)。
- 可選的未來：WebRTC（TBD）——超出了最初稻草人的範圍。
- 重新連接策略：帶完全抖動的指數退避（基本 5 秒，最大 60 秒）； Swift、Android 和 JS 之間共享常量，因此重試仍然是可預測的。
- Ping/pong 節奏：30 秒心跳，在重新連接之前可以容忍 3 次錯過的 Pong； JS 將最小間隔限制為 15 秒以滿足瀏覽器限制規則。
- 推送掛鉤：Android 錢包 SDK 公開了用於喚醒的可選 FCM 集成，而 JS 保持基於輪詢（已記錄的瀏覽器推送權限限制）。
- SDK職責：
  - 保持乒乓心跳（避免耗盡手機電池）。
  - 離線時緩衝傳出幀（有界隊列，為 dApp 保留）。
- 提供事件流API（Swift Couple `AsyncStream`、Android Flow、JS async iter）。
- 表面重新連接掛鉤並允許手動重新訂閱。
- 遙測編輯：僅發出會話級計數器（`sid` 哈希、方向、
  序列窗口、隊列深度）以及 Connect 遙測中記錄的鹽
  指導；標頭/鍵絕不能出現在日誌或調試字符串中。

## 加密和密鑰管理

### 會話標識符和鹽

- `sid` 是從 `BLAKE2b-256("iroha-connect|sid|" || chain_id || app_ephemeral_pk || nonce16)` 派生的 32 字節標識符。  
  DApp 在調用 `/v1/connect/session` 之前計算它；錢包在 `approve` 幀中回應它，因此雙方可以一致地輸入日誌和遙測數據。
- 相同的鹽提供每個密鑰派生步驟，因此 SDK 從不依賴於從主機平台獲取的熵。

### 臨時密鑰處理

- 每個會話都使用新的 X25519 密鑰材料。  
  Swift 通過 `ConnectCrypto` 將其存儲在 Keychain/Secure Enclave 中，Android 錢包默認為 StrongBox（回退到 TEE 支持的密鑰庫），JS 需要安全上下文 WebCrypto 實例或本機 `iroha_js_host` 插件。
- 開放框架包括 dApp 臨時公鑰以及可選的證明捆綁包。錢包批准會返回錢包公鑰以及合規流程所需的任何硬件證明。
- 證明有效負載遵循可接受的架構：  
  `attestation { platform, evidence_b64, statement_hash }`。  
  瀏覽器可能會忽略該塊；每當使用硬件支持的密鑰時，本機錢包都會包含它。

### 方向鍵和 AEAD

- 使用 HKDF-SHA256（通過 Rust 橋助手）和域分隔的信息字符串擴展共享密鑰：
  - `iroha-connect|k_app` → 應用→錢包流量。
  - `iroha-connect|k_wallet`→錢包→應用程序流量。
- AEAD 是 v1 信封的 ChaCha20-Poly1305（`connect_norito_bridge` 在每個平台上公開助手）。  
  關聯數據等於 `("connect:v1", sid, dir, seq_le, kind=ciphertext)`，因此檢測到對標頭的篡改。
- 隨機數源自 64 位序列計數器（`nonce[0..4]=0`、`nonce[4..12]=seq_le`）。共享幫助程序測試可確保 BigInt/UInt 轉換在 SDK 中的行為相同。

### 旋轉和恢復握手- 輪換仍然是可選的，但協議已定義：當序列計數器接近包裝防護時，dApp 會發出 `Control::RotateKeys` 幀，錢包使用新的公鑰和簽名的確認進行響應，雙方立即派生新的方向密鑰而不關閉會話。
- 錢包端密鑰丟失會觸發相同的握手，然後是 `resume` 控制，因此 dApp 知道要刷新針對已停用密鑰的緩存密文。

有關歷史 CryptoKit 回退，請參閱 `docs/connect_swift_ios.md`； Kotlin 和 JS 在 `docs/connect_kotlin_ws*.md` 下有匹配的引用。

## 權限和證明

- 權限清單必須通過橋導出的共享 Norito 結構進行往返。  
  領域：
  - `methods` — 動詞（`sign_transaction`、`sign_raw`、`submit_proof`，...）。  
  - `events` — 允許 dApp 附加的訂閱。  
  - `resources` — 可選帳戶/資產過濾器，以便錢包可以限制訪問範圍。  
  - `constraints` — 鏈 ID、TTL 或錢包在簽名前強制執行的自定義策略旋鈕。
- 合規性元數據與權限並存：
  - 可選 `attachments[]` 包含 Norito 附件參考（KYC 捆綁包、監管機構收據）。  
  - `compliance_manifest_id` 將請求與先前批准的清單聯繫起來，以便操作員可以審核出處。
- 錢包響應使用約定的代碼：
  - `user_declined`、`permissions_mismatch`、`compliance_failed`、`internal_error`。  
  每個可能攜帶一個用於 UI 提示的 `localized_message` 以及一個機器可讀的 `reason_code`。
- 批准框架包括選定的帳戶/控制器、權限回顯、證明包（ZK 證明或證明）以及任何策略切換（例如 `offline_queue_enabled`）。  
  拒絕鏡像具有空 `proof` 的相同架構，但仍記錄 `sid` 以供審核。

## SDK 外觀

| SDK |提議的 API |筆記|
|-----|--------------|--------|
|斯威夫特 | `ConnectClient`、`ConnectSession`、`ConnectRequest`、`ConnectApproval` |用類型化包裝器 + 異步流替換佔位符。 |
|安卓 | Kotlin 協程 + 框架的密封類 |與 Swift 結構保持一致以實現可移植性。 |
| JS |框架類型的異步迭代器 + TypeScript 枚舉 |提供捆綁器友好的 SDK（瀏覽器/節點）。 |

### 常見行為- `ConnectSession` 協調生命週期：
  1. 建立WebSocket，進行握手。
  2. 交換開放/批准框架。
  3. 處理簽名請求/響應。
  4. 向應用層發送事件。
- 提供高級幫手：
  - `requestSignature(tx, metadata)`
  - `approveSession(account, permissions)`
  - `reject(reason)`
  - `cancelRequest(hash)` – 發出錢包確認的控制幀。
- 錯誤處理：將 Norito 錯誤代碼映射到 SDK 特定錯誤；包括
  使用共享分類法的 UI 的域特定代碼（`Transport`、`Codec`、`Authorization`、`Timeout`、`QueueOverflow`、`Internal`）。 Swift 的基線實現 + 遙測指南位於 [`connect_error_taxonomy.md`](connect_error_taxonomy.md) 中，是 Android/JS 奇偶校驗的參考。
- 發出隊列深度、重新連接計數和請求延遲的遙測掛鉤（`connect.queue_depth`、`connect.reconnects_total`、`connect.latency_ms`）。
 
## 序列號和流量控制

- 每個方向都保留一個專用的 64 位 `sequence` 計數器，該計數器在會話打開時從零開始。共享幫助程序類型會限制增量並在計數器迴繞之前觸發 `ConnectError.sequenceOverflow` + 密鑰旋轉握手。
- 隨機數和關聯數據引用序列號，因此可以在不解析有效負載的情況下拒絕重複項。 SDK 必須將 `{sid, dir, seq, payload_hash}` 存儲在其日誌中，以使重複數據刪除在重新連接時具有確定性。
- 錢包通過邏輯窗口（`FlowControl` 控制幀）通告背壓。僅當窗口令牌可用時，DApp 才會出隊；錢包在處理密文後發出新的代幣以保持管道的邊界。
- 恢復協商是明確的：雙方在重新連接後都會發出 `Control::Resume { seq_app_max, seq_wallet_max, queue_depths }`，以便觀察者可以驗證重新發送了多少數據以及日誌是否包含間隙。
- 衝突（例如，兩個有效負載具有相同的 `(sid, dir, seq)` 但不同的哈希值）升級為 `ConnectError.Internal` 並強制使用新的 `sid` 以避免靜默分歧。

## 威脅模型和數據保留一致性- **考慮的表面：** WebSocket 傳輸、Norito 橋編碼/解碼、
  日誌持久性、遙測導出器和麵向應用程序的回調。
- **主要目標：** 保護會話機密（X25519 密鑰、派生 AEAD 密鑰、
  隨機數/序列計數器）免受日誌/遙測洩漏的影響，防止重放和
  降級攻擊，並限制日誌和異常報告的保留。
- **已編纂的緩解措施：**
  - 期刊僅攜帶密文；存儲的元數據僅限於哈希值、長度
    字段、時間戳和序列號。
  - 遙測有效負載會編輯任何標頭/有效負載內容，並且僅包含
    `sid` 加鹽哈希加上聚合計數器；共享修訂清單
    SDK 之間的審核奇偶校驗。
  - 默認情況下，會話日誌會輪換並在 7 天后過期。錢包暴露
    `connectLogRetentionDays` 旋鈕（SDK 默認 7）並記錄行為
    因此，受監管的部署可以固定更嚴格的窗口。
  - Bridge API 濫用（缺少綁定、損壞的密文、無效序列）
    返回鍵入的錯誤，而不回顯原始有效負載或密鑰。

審核中的待決問題在 `docs/source/sdk/swift/connect_workshop.md` 中進行跟踪
並將在理事會會議記錄中予以解決；一旦關閉，稻草人將
從草稿晉升為接受。

## 離線緩沖和重新連接

### 日記合同

每個 SDK 都會為每個會話維護一個僅附加日誌，因此 dApp 和錢包
可以在離線時對幀進行排隊、在不丟失數據的情況下恢復並提供證據
用於遙測。該合約鏡像 Norito 橋接類型，因此具有相同的字節
表示在移動/JS 堆棧中仍然存在。- 日誌存在於散列會話標識符 (`sha256(sid)`) 下，產生兩個
  每個會話的文件：`app_to_wallet.queue` 和 `wallet_to_app.queue`。斯威夫特使用
  沙盒文件包裝器，Android 通過 `Room`/`FileChannel` 存儲文件，
  JS寫入IndexedDB；所有格式都是二進制且字節序穩定。
- 每條記錄序列化為 `ConnectJournalRecordV1`：
  - `direction: u8` (`0 = app→wallet`, `1 = wallet→app`)
  - `sequence: u64`
  - `payload_hash: [u8; 32]`（Blake3密文+標頭）
  - `ciphertext_len: u32`
  - `received_at_ms: u64`
  - `expires_at_ms: u64`
  - `ciphertext: [u8; ciphertext_len]`（確切的 Norito 框架已經 AEAD 包裝）
- 日誌逐字存儲密文。我們從不重新加密有效負載； AEAD
  標頭已經驗證了方向鍵，因此持久性減少為
  fsyncing 附加記錄。
- 內存中的 `ConnectQueueState` 結構鏡像文件元數據（深度、
  使用的字節數，最舊/最新的序列）。它為遙測出口商和
  `FlowControl` 幫助者。
- 默認情況下，日誌上限為 32 幀/1MiB；擊中上限將驅逐
  最舊的條目 (`reason=overflow`)。 `ConnectFeatureConfig.max_queue_len`
  每個部署都會覆蓋這些默認值。
- 日誌保留數據 24 小時 (`expires_at_ms`)。後台 GC 刪除陳舊內容
  急切地進行分段，以便磁盤上的足跡保持有限。
- 崩潰安全：在通知之前追加、fsync 和更新內存鏡像
  來電者。啟動時，SDK 會掃描目錄，驗證記錄校驗和，
  並重建 `ConnectQueueState`。腐敗導致違規記錄
  跳過，通過遙測標記，並可選擇隔離支持轉儲。
- 因為密文已經滿足 Norito 隱私信封，唯一
  記錄的附加元數據是散列會話 ID。需要額外的應用程序
  隱私可以選擇 `telemetry_opt_in = false`，它存儲日誌，但是
  編輯隊列深度導出並禁用在日誌中共享散列 `sid`。
- SDK 公開 `ConnectQueueObserver`，以便錢包/dApp 可以檢查隊列深度，
  排水管和 GC 結果；該鉤子提供狀態 UI，而不解析日誌。

### 重放和恢復語義

1. 重新連接時，SDK 會發出 `Control::Resume` 和 `{seq_app_max,
   seq_wallet_max、queued_app、queued_wallet、journal_hash}`。哈希值是
   Blake3 僅附加日誌的摘要，因此不匹配的同行可以檢測漂移。
2. 接收方將恢復負載與其狀態進行比較，請求
   存在間隙時重傳，並通過以下方式確認重播幀
   `Control::ResumeAck`。
3. 重放的幀始終遵循插入順序（`sequence`，然後是寫入時間）。
   錢包 SDK 必須通過發行 `FlowControl` 代幣（也
   日誌），因此 dApp 無法在離線狀態下淹沒隊列。
4. 日誌逐字存儲密文，因此重放只是泵送記錄的字節
   通過傳輸和解碼器返回。不允許按 SDK 重新編碼。

### 重連流程1. Transport 重新建立 WebSocket 並協商新的 ping 間隔。
2. dApp 按順序重放排隊的幀，尊重錢包的背壓
   （`ConnectSession.nextControlFrame()` 生成 `FlowControl` 代幣）。
3. 錢包解密緩衝結果，驗證序列單調性，並
   重播待批准/結果。
4.雙方發出一個`resume`控制總結`seq_app_max`，`seq_wallet_max`，
   和遙測的隊列深度。
5. 重複幀（匹配 `sequence` + `payload_hash`）被確認並丟棄；衝突引發 `ConnectError.Internal` 並觸發強制會話重新啟動。

### 故障模式

- 如果會話被視為過時（`offline_timeout_ms`，默認 5 分鐘），
  緩衝幀被清除，SDK 引發 `ConnectError.sessionExpired`。
- 如果日誌損壞，SDK 會嘗試單個 Norito 解碼修復；上
  如果失敗，他們會丟棄日誌並發出 `connect.queue_repair_failed` 遙測數據。
- 序列不匹配觸發 `ConnectError.replayDetected` 並強制重新啟動
  握手（使用新的 `sid` 重新啟動會話）。

### 離線緩衝計劃和操作員控制

研討會交付成果需要一份書面計劃，以便每個 SDK 都提供相同的
線下行為、補救流程和證據表面。下面的計劃是
Swift (`ConnectSessionDiagnostics`)、Android 中常見
（`ConnectDiagnosticsSnapshot`）和JS（`ConnectQueueInspector`）。

|狀態|觸發|自動回复 |手動操作|遙測標誌|
|--------|---------|--------------------|-----------------|----------------|
| `Healthy` |隊列使用率  5/分鐘 |暫停新的簽名請求，以一半的速率發出流量控制令牌 |應用程序可能會調用 `clearOfflineQueue(.app|.wallet)`；一旦上線，SDK 就會重新獲取對等方的狀態 | `connect.queue_state=\"throttled\"`、`connect.queue_watermark` 儀表 |
| `Quarantined` |使用率 ≥ `disk_watermark_drop`（默認 85%），檢測到損壞兩次，或超出 `offline_timeout_ms` |停止緩衝，引發 `ConnectError.QueueQuarantined`，需要操作員確認 | `ConnectSessionDiagnostics.forceReset()` 導出捆綁包後刪除日誌 | `connect.queue_state=\"quarantined\"`、`connect.queue_quarantine_total` 計數器 |- 閾值位於 `ConnectFeatureConfig` (`disk_watermark_warn`,
  `disk_watermark_drop`、`max_disk_bytes`、`offline_timeout_ms`）。當主持人
  省略一個值，SDK 會回退到默認值並記錄警告，以便配置
  可以通過遙測進行審核。
- SDK 公開 `ConnectQueueObserver` 以及診斷助手：
  - Swift：`ConnectSessionDiagnostics.snapshot()` 產生“{狀態，深度，字節，
    Reason}` and `exportJournalBundle(url:)` 保留兩個隊列以獲取支持。
  - 安卓：`ConnectDiagnostics.snapshot()` + `exportJournalBundle(path)`。
  - JS: `ConnectQueueInspector.read()` 返回相同的結構和 blob 句柄
    該UI代碼可以上傳到Torii支持工具。
- 當應用程序切換 `offline_queue_enabled=false` 時，SDK 會立即耗盡並
  清除兩個日誌，將狀態標記為 `Disabled`，並發出一個終端
  遙測事件。面向用戶的偏好反映在 Norito 中
  批准幀，以便同行知道他們是否可以恢復緩衝的幀。
- 操作員運行 `connect queue inspect --sid <sid>`（SDK 周圍的 CLI 包裝器）
  混沌測試期間的診斷）；該命令打印狀態轉換，
  水印歷史記錄，並恢復證據，以便治理審查不依賴於
  特定於平台的工具。

### 證據包工作流程

支持和合規團隊在審計時依賴確定性證據
離線行為。因此，每個 SDK 都實現相同的三步導出：

1. `exportJournalBundle(..)` 寫入 `{app_to_wallet,wallet_to_app}.queue` 加一個
   描述構建哈希、功能標誌和磁盤水印的清單。
2. `exportQueueMetrics(..)` 發出最後 1000 個遙測樣本，以便儀表板
   可以離線重建。示例包括哈希會話 ID，當
   用戶選擇加入。
3. CLI 幫助程序壓縮導出並附加簽名的 Norito 元數據文件
   (`ConnectQueueEvidenceV1`) 因此 Torii 攝取可以將捆綁包存檔在 SoraFS 中。

驗證失敗的捆綁包將被拒絕，並顯示 `connect.evidence_invalid`
遙測，以便 SDK 團隊可以重現並修補導出器。

## 遙測和診斷- 通過共享 OpenTelemetry 導出器發出 Norito JSON 事件。強制性指標：
  - `connect.queue_depth{direction}`（表壓）由 `ConnectQueueState` 供給。
  - `connect.queue_bytes{direction}`（規格）用於磁盤支持的佔用空間。
  - `connect.queue_dropped_total{reason}`（計數器）用於 `overflow|ttl|repair`。
  - `connect.offline_flush_total{direction}`（計數器）在排隊時遞增
    無需運輸即可排出；故障增量為 `connect.offline_flush_failed`。
  - `connect.replay_success_total`/`connect.replay_error_total`。
  - `connect.resume_latency_ms` 直方圖（重新連接和穩定之間的時間
    狀態）加上 `connect.resume_attempts_total`。
  - `connect.session_duration_ms` 直方圖（每個已完成的會話）。
  - `connect.error` 與 `code`、`fatal`、`telemetry_profile` 結構化事件。
- 出口商必須貼上 `{platform, sdk_version, feature_hash}` 標籤，以便
  儀表板可以按 SDK 版本進行拆分。散列 `sid` 是可選的，並且是唯一的
  當遙測選擇加入為真時發出。
- SDK 級掛鉤呈現相同的事件，以便應用程序可以導出更多詳細信息：
  - 斯威夫特：`ConnectSession.addObserver(_:) -> ConnectEvent`。
  - 安卓：`Flow<ConnectEvent>`。
  - JS：異步迭代器或回調。
- CI 門控：Swift 作業運行 `make swift-ci`，Android 使用 `./gradlew sdkConnectCi`，
  並且 JS 運行 `npm run test:connect`，因此遙測/儀表板之前保持綠色
  合併連接更改。
- 結構化日誌包括散列 `sid`、`seq`、`queue_depth` 和 `sid_epoch`
  值，以便操作員可以關聯客戶問題。修復失敗的日誌會發出
  `connect.queue_repair_failed{reason}` 事件以及可選的故障轉儲路徑。

### 遙測掛鉤和治理證據

- `connect.queue_state` 兼作路線圖風險指示器。儀表板組
  通過 `{platform, sdk_version}` 並呈現狀態時間，以便治理可以採樣
  在批准分階段部署之前每月進行演習證據。
- `connect.queue_watermark` 和 `connect.queue_bytes` 提供 Connect 風險評分
  (`risk.connect.offline_buffer`)，當超過
  5% 的會話在 `Throttled` 中花費超過 10 分鐘。
- 出口商將 `feature_hash` 附加到每個事件，以便審核工具可以確認
  Norito 編解碼器 + 離線計劃與審核的版本匹配。 SDK CI 失敗
  當遙測報告未知哈希值時速度很快。
- 稻草人仍然需要威脅模型附錄；當指標超過
  策略閾值，SDK 會發出 `connect.policy_violation` 事件，總結
  有問題的 sid（散列）、狀態和已解決的操作 (`drain|purge|quarantine`)。
- 通過 `exportQueueMetrics` 捕獲的證據位於相同的 SoraFS 命名空間中
  作為 Connect 操作手冊的工件，以便理事會審查者可以跟踪每一次演習
  返回特定的遙測樣本，無需請求內部日誌。

## 框架所有權和責任|框架/控制|業主|序列域|日記堅持了嗎？ |遙測標籤|筆記|
|----------------|--------------------|-----------------|--------------------|--------------------|-------|
| `Control::Open` | dApp | `seq_app` | ✅ (`app_to_wallet`) | `event=open` |攜帶元數據+權限位圖；錢包重放最近一次打開之前的提示。 |
| `Control::Approve` |錢包| `seq_wallet` | ✅ (`wallet_to_app`) | `event=approve` |包括賬戶、證明、簽名。此處記錄元數據版本增量。 |
| `Control::Reject` |錢包| `seq_wallet` | ✅ | `event=reject`，`reason` |可選的本地化消息； dApp 會刪除待處理的簽名請求。 |
| `Control::Close`（初始化）| dApp | `seq_app` | ✅ | `event=close`，`initiator=app` |錢包用自己的 `Close` 進行確認。 |
| `Control::Close`（確認）|錢包| `seq_wallet` | ✅ | `event=close`，`initiator=wallet` |確認拆解；一旦雙方都堅持框架，GC 就會移除軸頸。 |
| `SignRequest` | dApp | `seq_app` | ✅ | `event=sign_request`，`payload_hash` |記錄有效負載哈希以進行重播衝突檢測。 |
| `SignResult` |錢包| `seq_wallet` | ✅ | `event=sign_result`，`status=ok|err` |包括簽名字節的 BLAKE3 哈希值；故障引發 `ConnectError.Signing`。 |
| `Control::Error` |錢包（大多數）/dApp（交通）|匹配所有者域 | ✅ | `event=error`、`code` |致命錯誤迫使會話重新啟動；遙測標記 `fatal=true`。 |
| `Control::RotateKeys` |錢包| `seq_wallet` | ✅ | `event=rotate_keys`，`reason` |宣布新的方向鍵； dApp 回复 `RotateKeysAck`（在應用程序端記錄）。 |
| `Control::Resume` / `ResumeAck` |兩者 |僅本地域 | ✅ | `event=resume`、`direction=app|wallet` |總結隊列深度+seq狀態；散列期刊摘要有助於診斷。 |

- 每個角色的定向密碼密鑰保持對稱（`app→wallet`、`wallet→app`）。
  錢包輪換提案通過 `Control::RotateKeys` 和 dApp 發布
  通過發出 `Control::RotateKeysAck` 進行確認；兩個幀都必須擊中磁盤
  在密鑰交換之前避免重播間隙。
- 元數據附件（圖標、本地化名稱、合規性證明）由以下人員簽名
  錢包並由 dApp 緩存；更新需要新的批准框架
  遞增 `metadata_version`。
- 上面的所有權矩陣是從 SDK 文檔中引用的，因此 CLI/web/automation
  客戶遵循相同的合同和儀器默認值。

## 開放問題

1. **會話發現**：我們是否需要像 WalletConnect 那樣的二維碼/帶外握手？ （未來的工作。）
2. **多重簽名**：多重簽名批准如何表示？ （擴展簽名結果以支持多個簽名。）
3. **合規性**：哪些字段對於受監管的流量是強制性的（根據路線圖）？ （等待合規團隊指導。）
4. **SDK 打包**：我們是否應該將共享代碼（例如 Norito Connect 編解碼器）納入跨平台 crate 中？ （待定。）

## 後續步驟- 將此稻草人傳給 SDK 委員會（2026 年 2 月的會議）。
- 收集有關未解決問題的反饋並相應更新文檔。
- 按 SDK 安排實施細目（Swift IOS7、Android AND7、JS Connect 里程碑）。
- 通過路線圖熱門列表跟踪進度；一旦稻草人獲得批准，請更新 `status.md`。