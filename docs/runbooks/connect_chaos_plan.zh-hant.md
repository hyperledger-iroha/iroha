---
lang: zh-hant
direction: ltr
source: docs/runbooks/connect_chaos_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b1d414173d2f43d403a6a1ba5cd59a645cb0b94f5765e69a00f7078b1e96b1cd
source_last_modified: "2025-12-29T18:16:35.913527+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# 連接混亂和故障排練計劃 (IOS3 / IOS7)

本劇本定義了滿足 IOS3/IOS7 的可重複混沌練習
路線圖行動_“計劃聯合混亂演練”_ (`roadmap.md:1527`)。與它配對
Connect 預覽運行手冊 (`docs/runbooks/connect_session_preview_runbook.md`)
進行跨 SDK 演示時。

## 目標和成功標準
- 執行共享的連接重試/退避策略、脫機隊列限制以及
  遙測出口商在受控故障下，無需改變生產代碼。
- 捕獲確定性偽影（`iroha connect queue inspect` 輸出，
  `connect.*` 指標快照、Swift/Android/JS SDK 日誌），以便治理可以
  審核每一次演習。
- 證明錢包和 dApp 尊重配置更改（清單漂移、salt
  通過顯示規範的 `ConnectError` 來旋轉、證明失敗）
  類別和編輯安全的遙測事件。

## 先決條件
1. **環境引導**
   - 啟動演示 Torii 堆棧：`scripts/ios_demo/start.sh --telemetry-profile full`。
   - 啟動至少一個 SDK 示例 (`examples/ios/NoritoDemoXcode/NoritoDemoXcode`,
     `examples/ios/NoritoDemo`、Android `demo-connect`、JS `examples/connect`）。
2. **儀器儀表**
   - 啟用 SDK 診斷（`ConnectQueueDiagnostics`、`ConnectQueueStateTracker`、
     Swift 中的 `ConnectSessionDiagnostics`； `ConnectQueueJournal` + `ConnectQueueJournalTests`
     Android/JS 中的等效項）。
   - 確保 CLI `iroha connect queue inspect --sid <sid> --metrics` 解析
     SDK 生成的隊列路徑（`~/.iroha/connect/<sid>/state.json` 和
     `metrics.ndjson`）。
   - 有線遙測導出器，因此以下時間序列在
     Grafana 和通過 `scripts/swift_status_export.py telemetry`: `connect.queue_depth`,
     `connect.queue_dropped_total`, `connect.reconnects_total`,
     `connect.resume_latency_ms`，`swift.connect.frame_latency`，
     `android.telemetry.redaction.salt_version`。
3. **證據文件夾** – 創建 `artifacts/connect-chaos/<date>/` 並存儲：
   - 原始日誌 (`*.log`)、指標快照 (`*.json`)、儀表板導出
     (`*.png`)、CLI 輸出和 PagerDuty ID。

## 場景矩陣|身份證 |故障|注射步驟|預期信號 |證據|
|----|--------|-----------------|--------------------|----------|
| C1 | WebSocket 中斷和重新連接 |將 `/v2/connect/ws` 包裹在代理後面（例如 `kubectl -n demo port-forward svc/torii 18080:8080` + `toxiproxy-cli toxic add ... timeout`）或暫時阻止服務（`kubectl scale deploy/torii --replicas=0` ≤ 60 秒）。強制錢包繼續發送幀，以便填滿離線隊列。 | `connect.reconnects_total` 遞增，`connect.resume_latency_ms` 尖峰但保持 - 中斷窗口的儀表板註釋。 - 包含重新連接 + 耗盡消息的示例日誌摘錄。 |
| C2 |離線隊列溢出/TTL過期|修補示例以縮小隊列限制（Swift：在 `ConnectSessionDiagnostics` 內實例化 `ConnectQueueJournal.Configuration(maxRecordsPerQueue: 4, maxBytesPerQueue: 4096, retentionInterval: 30)`；Android/JS 使用相應的構造函數）。當 dApp 繼續排隊請求時，將錢包暫停 ≥2× `retentionInterval`。 | `connect.queue_dropped_total{reason="overflow"}` 和 `{reason="ttl"}` 增量，`connect.queue_depth` 在新限制處穩定，SDK 表面 `ConnectError.QueueOverflow(limit: 4)`（或 `.QueueExpired`）。 `iroha connect queue inspect` 顯示 `state=Overflow`，其中 `warn/drop` 水印為 100%。 | - 指標計數器的屏幕截圖。 - CLI JSON 輸出捕獲溢出。 - 包含 `ConnectError` 行的 Swift/Android 日誌片段。 |
| C3 |明顯漂移/錄取拒絕|篡改提供給錢包的 Connect 清單（例如，修改 `docs/connect_swift_ios.md` 示例清單，或啟動 Torii，其中 `--connect-manifest-path` 指向 `chain_id` 或 `permissions` 不同的副本）。讓 dApp 請求批准並確保錢包通過策略拒絕。 | Torii 使用 `manifest_mismatch` 返回 `/v2/connect/session` 的 `HTTP 409`，SDK 發出 `ConnectError.Authorization.manifestMismatch(manifestVersion)`，遙測引發 `connect.manifest_mismatch_total`，並且隊列保持為空 (`state=Idle`)。 | - Torii 日誌摘錄顯示不匹配檢測。 - 出現錯誤的 SDK 屏幕截圖。 - 指標快照證明測試期間沒有排隊的幀。 |
| C4|按鍵旋轉/鹽版凹凸|在會話中輪換 Connect salt 或 AEAD 密鑰。在開發堆棧中，使用 `CONNECT_SALT_VERSION=$((old+1))` 重新啟動 Torii（鏡像 `docs/source/sdk/android/telemetry_schema_diff.md` 中的 Android 編輯鹽測試）。保持錢包離線，直到鹽輪換完成，然後恢復。 |第一次恢復嘗試失敗，顯示 `ConnectError.Authorization.invalidSalt`，隊列刷新（dApp 因 `salt_version_mismatch` 原因丟棄緩存幀），遙測發出 `android.telemetry.redaction.salt_version` (Android) 和 `swift.connect.session_event{event="salt_rotation"}`。 SID 刷新成功後的第二個會話。 | - 帶有鹽紀元之前/之後的儀表板註釋。 - 包含無效鹽錯誤和後續成功的日誌。 - `iroha connect queue inspect` 輸出顯示 `state=Stalled` 後跟新鮮的 `state=Active`。 || C5|證明/StrongBox 失敗 |在 Android 錢包上，配置 `ConnectApproval` 以包含 `attachments[]` + StrongBox 證明。使用證明工具（`scripts/android_keystore_attestation.sh` 和 `--inject-failure strongbox-simulated`）或在提交給 dApp 之前篡改證明 JSON。 | DApp 以 `ConnectError.Authorization.invalidAttestation` 拒絕批准，Torii 記錄失敗原因，導出器碰撞 `connect.attestation_failed_total`，隊列清除違規條目。 Swift/JS dApp 在保持會話活動的同時記錄錯誤。 | - 帶有註入故障 ID 的線束日誌。 - SDK 錯誤日誌 + 遙測計數器捕獲。 - 隊列刪除了壞幀的證據 (`recordsRemoved > 0`)。 |

## 場景細節

### C1 — WebSocket 中斷和重新連接
1. 將 Torii 包裝在代理（toxiproxy、Envoy 或 `kubectl port-forward`）後面，以便
   您可以切換可用性而無需殺死整個節點。
2. 觸發45秒中斷：
   ```bash
   toxiproxy-cli toxic add connect-ws --type timeout --toxicity 1.0 --attribute timeout=45000
   sleep 45 && toxiproxy-cli toxic remove connect-ws --toxic timeout
   ```
3. 觀察遙測儀表板和 `scripts/swift_status_export.py telemetry
   --json-out artifacts/connect-chaos//c1_metrics.json`。
4. 中斷後立即轉儲隊列狀態：
   ```bash
   iroha connect queue inspect --sid "$SID" --metrics > artifacts/connect-chaos/<date>/c1_queue.txt
   ```
5. 成功 = 單次重新連接嘗試、有界隊列增長和自動
   代理恢復後耗盡。

### C2 — 離線隊列溢出/TTL 過期
1. 縮小本地構建中的隊列閾值：
   - Swift：更新示例中的 `ConnectQueueJournal` 初始化程序
     （例如，`examples/ios/NoritoDemoXcode/NoritoDemoXcode/ContentView.swift`）
     通過 `ConnectQueueJournal.Configuration(maxRecordsPerQueue: 4, maxBytesPerQueue: 4096, retentionInterval: 30)`。
   - Android/JS：構造時傳遞等效的配置對象
     `ConnectQueueJournal`。
2. 暫停錢包（模擬器後台或設備飛行模式）≥60s
   而 dApp 發出 `ConnectClient.requestSignature(...)` 調用。
3. 使用 `ConnectQueueDiagnostics`/`ConnectQueueStateTracker` (Swift) 或 JS
   用於導出證據包的診斷助手（`state.json`、`journal/*.to`、
   `metrics.ndjson`）。
4. 成功 = 溢出計數器遞增，SDK 表面 `ConnectError.QueueOverflow`
   一次，錢包恢復後隊列恢復。

### C3 — 明顯漂移/錄取拒絕
1. 複印一份入場清單，例如：
   ```bash
   cp configs/connect_manifest.json /tmp/manifest_drift.json
   sed -i '' 's/"chain_id": ".*"/"chain_id": "bogus-chain"/' /tmp/manifest_drift.json
   ```
2. 使用 `--connect-manifest-path /tmp/manifest_drift.json` 啟動 Torii（或
   更新演習的 docker compose/k8s 配置）。
3. 嘗試從錢包啟動會話；預計 HTTP 409。
4. 捕獲 Torii + SDK 日誌以及 `connect.manifest_mismatch_total`
   遙測儀表板。
5. 成功=拒絕且隊列不增長，並且錢包顯示共享
   分類錯誤 (`ConnectError.Authorization.manifestMismatch`)。### C4 — 密鑰旋轉/鹽凸塊
1. 通過遙測記錄當前的 salt 版本：
   ```bash
   scripts/swift_status_export.py telemetry --json-out artifacts/connect-chaos/<date>/c4_before.json
   ```
2. 使用新鹽重新啟動 Torii（`CONNECT_SALT_VERSION=$((OLD+1))` 或更新
   配置圖）。保持錢包離線，直到重啟完成。
3. 恢復錢包；第一個恢復應該失敗並出現無效鹽錯誤
   和 `connect.queue_dropped_total{reason="salt_version_mismatch"}` 增量。
4. 通過刪除會話目錄強制應用程序刪除緩存的幀
   （`rm -rf ~/.iroha/connect/<sid>`或平台特定緩存清除），然後
   使用新令牌重新啟動會話。
5. 成功 = 遙測顯示鹽塊，記錄無效的恢復事件
   一次，下一次會話無需人工干預即可成功。

### C5 — 證明/StrongBox 失敗
1. 使用 `scripts/android_keystore_attestation.sh` 生成證明包
   （設置 `--inject-failure strongbox-simulated` 以翻轉簽名位）。
2. 讓錢包通過其 `ConnectApproval` API 附加此捆綁包； dApp
   應驗證並拒絕有效負載。
3.驗證遙測（`connect.attestation_failed_total`，Swift/Android事件
   指標）並確保隊列丟棄中毒的條目。
4. 成功 = 拒絕與不良批准隔離，隊列保持健康，
   證明日誌與演練證據一起存儲。

## 證據清單
- `artifacts/connect-chaos/<date>/c*_metrics.json` 出口自
  `scripts/swift_status_export.py telemetry`。
- 來自 `iroha connect queue inspect` 的 CLI 輸出 (`c*_queue.txt`)。
- SDK + Torii 日誌帶有時間戳和 SID 哈希值。
- 帶有每個場景註釋的儀表板屏幕截圖。
- PagerDuty / 事件 ID（如果 Sev1/2 警報觸發）。

每季度完成一次完整矩陣即可滿足路線圖門和
顯示 Swift/Android/JS Connect 實現做出確定性響應
跨越最高風險的故障模式。