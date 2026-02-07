---
lang: zh-hant
direction: ltr
source: docs/runbooks/connect_session_preview_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7d04af2ad3ae5cc6a9254236f5627850aab7c6517308e9f3a09650cbc1490168
source_last_modified: "2026-01-05T18:22:23.401292+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# 連接會話預覽運行手冊 (IOS7 / JS4)

本操作手冊記錄了暫存、驗證和部署的端到端過程
根據路線圖里程碑的要求拆除 Connect 預覽會話 **IOS7**
和 **JS4**（`roadmap.md:1340`、`roadmap.md:1656`）。每當
您演示了 Connect Strawman (`docs/source/connect_architecture_strawman.md`)，
執行 SDK 路線圖中承諾的隊列/遙測掛鉤，或收集
`status.md` 的證據。

## 1. 飛行前檢查表

|項目 |詳情 |參考文獻 |
|------|---------|------------|
| Torii 端點 + 連接策略 |確認 Torii 基本 URL、`chain_id` 和連接策略 (`ToriiClient.getConnectStatus()` / `getConnectAppPolicy()`)。捕獲 Runbook 票證中的 JSON 快照。 | `javascript/iroha_js/src/toriiClient.js`、`docs/source/sdk/js/quickstart.md#connect-sessions--queueing` |
|夾具+橋版本|請注意您將使用的 Norito 夾具哈希和橋接構建（Swift 需要 `NoritoBridge.xcframework`，JS 需要 `@iroha/iroha-js` ≥ 附帶 `bootstrapConnectPreviewSession` 的版本）。 | `docs/source/sdk/swift/reproducibility_checklist.md`，`javascript/iroha_js/CHANGELOG.md` |
|遙測儀表板 |確保繪製 `connect.queue_depth`、`connect.queue_overflow_total`、`connect.resume_latency_ms`、`swift.connect.session_event` 等的儀表板可訪問（Grafana `Android/Swift Connect` 板 + 導出的 Prometheus 快照）。 | `docs/source/connect_architecture_strawman.md`、`docs/source/sdk/swift/telemetry_redaction.md`、`docs/source/sdk/js/quickstart.md` |
|證據文件夾|選擇一個目的地，例如 `docs/source/status/swift_weekly_digest.md`（每週摘要）和 `docs/source/sdk/swift/connect_risk_tracker.md`（風險跟踪器）。將日誌、指標屏幕截圖和確認存儲在 `docs/source/sdk/swift/readiness/archive/<date>/connect/` 下。 | `docs/source/status/swift_weekly_digest.md`，`docs/source/sdk/swift/connect_risk_tracker.md` |

## 2. 引導預覽會話

1. **驗證政策+配額。 ** 致電：
   ```js
   const status = await torii.getConnectStatus();
   console.log(status.policy.queue_max, status.policy.offline_timeout_ms);
   ```
   如果 `queue_max` 或 TTL 與您計劃的配置不同，則運行失敗
   測試。
2. **生成確定性 SID/URI。 ** `@iroha/iroha-js`
   `bootstrapConnectPreviewSession` 幫助程序將 SID/URI 生成與 Torii 聯繫起來
   會議註冊；即使 Swift 將驅動 WebSocket 層也可以使用它。
   ```js
   import {
     ToriiClient,
     bootstrapConnectPreviewSession,
   } from "@iroha/iroha-js";

   const client = new ToriiClient(process.env.TORII_BASE_URL, { chainId: "sora-mainnet" });
   const { preview, session, tokens } = await bootstrapConnectPreviewSession(client, {
     chainId: "sora-mainnet",
     appBundle: "dev.sora.example.dapp",
     walletBundle: "dev.sora.example.wallet",
     register: true,
   });
   console.log("sid", preview.sidBase64Url, "ws url", preview.webSocketUrl);
   ```
   - 將 `register: false` 設置為空運行 QR/深層鏈接場景。
   - 將返回的 `sidBase64Url`、深層鏈接 URL 和 `tokens` blob 保留在
     證據文件夾；治理審查預計會出現這些人工製品。
3. **分發秘密。 ** 與錢包運營商共享深度鏈接 URI
   （swift dApp 示例、Android 錢包或 QA 工具）。切勿粘貼原始令牌
   進入聊天；使用啟用數據包中記錄的加密保管庫。

## 3. 推動會議1. **打開 WebSocket。 ** Swift 客戶端通常使用：
   ```swift
   let connectURL = URL(string: preview.webSocketUrl)!
   let client = ConnectClient(url: connectURL)
   let sid: Data = /* decode preview.sidBase64Url into raw bytes using your harness helper */
   let session = ConnectSession(sessionID: sid, client: client)
   let recorder = ConnectReplayRecorder(sessionID: sid)
   session.addObserver(ConnectEventObserver(queue: .main) { event in
       logger.info("connect event", metadata: ["kind": "\(event.kind)"])
   })
   try client.open()
   ```
   參考 `docs/connect_swift_integration.md` 用於其他設置（橋
   導入、並發適配器）。
2. **批准 + 簽署流程。 ** DApp 調用 `ConnectSession.requestSignature(...)`，
   而錢包則通過 `approveSession` / `reject` 進行響應。每次批准都必須記錄
   哈希別名 + 權限以匹配 Connect 治理章程。
3. **練習隊列 + 恢復路徑。 ** 切換網絡連接或暫停
   錢包確保有界隊列和重放掛鉤日誌條目。 JS/安卓
   SDK 發出 `ConnectQueueError.overflow(limit)` /
   `.expired(ttlMs)` 當它們丟幀時；斯威夫特應該觀察同樣的一次
   IOS7 隊列腳手架登陸 (`docs/source/connect_architecture_strawman.md`)。
   記錄至少一次重新連接後，運行
   ```bash
   iroha connect queue inspect --sid "$SID" --root ~/.iroha/connect --metrics
   ```
   （或傳遞 `ConnectSessionDiagnostics` 返回的導出目錄）和
   將渲染的表/JSON 附加到 Runbook 票證。 CLI 讀取相同
   `ConnectQueueStateTracker`產生的`state.json` / `metrics.ndjson`對，
   因此，治理審核人員無需定制工具即可追踪演練證據。

## 4. 遙測和可觀測性

- **要捕獲的指標：**
  - `connect.queue_depth{direction}` 表（應保持在保單上限以下）。
  - `connect.queue_dropped_total{reason="overflow|ttl"}` 計數器（僅非零
    在故障注入期間）。
  - `connect.resume_latency_ms`直方圖（強制後記錄p95
    重新連接）。
  - `connect.replay_success_total` / `connect.replay_error_total`。
  - Swift 專用 `swift.connect.session_event` 和
    `swift.connect.frame_latency` 導出 (`docs/source/sdk/swift/telemetry_redaction.md`)。
- **儀表板：** 使用註釋標記更新連接板書籤。
  將屏幕截圖（或 JSON 導出）與原始數據一起附加到證據文件夾中
  通過遙測導出器 CLI 拉取的 OTLP/Prometheus 快照。
- **警報：** 如果觸發任何 Sev1/2 閾值（根據 `docs/source/android_support_playbook.md` §5），
  尋呼 SDK 項目負責人並在運行手冊中記錄 PagerDuty 事件 ID
  繼續之前先買票。

## 5. 清理和回滾

1. **刪除暫存會話。 ** 始終刪除預覽會話，以便隊列深度
   警報仍然有意義：
   ```js
   await client.deleteConnectSession(preview.sidBase64Url);
   ```
   對於僅 Swift 的測試運行，請通過 Rust/CLI 幫助程序調用相同的端點。
2. **清除日誌。 ** 刪除任何保留的隊列日誌
   （`ApplicationSupport/ConnectQueue/<sid>.to`，IndexedDB 存儲等）所以
   下次運行開始乾淨。如果需要，請在刪除前記錄文件哈希
   調試重播問題。
3. **歸檔事件記錄。 ** 總結運行情況：
   - `docs/source/status/swift_weekly_digest.md`（增量塊），
   - `docs/source/sdk/swift/connect_risk_tracker.md`（清除或降級CR-2
     一旦遙測到位），
   - JS SDK 變更日誌或配方（如果新行為已驗證）。
4. **升級故障：**
   - 沒有註入錯誤的隊列溢出 ⇒ 針對 SDK 提交錯誤
     政策與 Torii 不同。
   - 恢復錯誤⇒附加 `connect.queue_depth` + `connect.resume_latency_ms`
     事件報告的快照。
   - 治理不匹配（代幣重複使用、超出 TTL）⇒ 通過 SDK 籌集資金
     在下一次修訂期間負責程序並註釋 `roadmap.md`。

## 6. 證據清單|文物 |地點 |
|----------|----------|
| SID/深層鏈接/令牌 JSON | `docs/source/sdk/swift/readiness/archive/<date>/connect/session.json` |
|儀表板導出（`connect.queue_depth`等）| `.../metrics/` 子文件夾 |
| PagerDuty / 事件 ID | `.../notes.md` |
|清理確認（Torii 刪除，日誌擦除）| `.../cleanup.log` |

完成此清單即可滿足“文檔/運行手冊已更新”退出標準
適用於 IOS7/JS4，並為治理審查者提供每個問題的確定性線索
連接預覽會話。