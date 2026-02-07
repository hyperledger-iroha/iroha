---
lang: zh-hant
direction: ltr
source: docs/source/mochi/troubleshooting.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: cb67b304bae01fa4a50d25dc9f086811dabfbcb24239b3ec9679338248e18be6
source_last_modified: "2025-12-29T18:16:35.985892+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# MOCHI 故障排除指南

當本地 MOCHI 集群拒絕啟動、陷入困境時，請使用此 Runbook
重新啟動循環，或停止流式傳輸塊/事件/狀態更新。它擴展了
通過轉變主管行為來製定路線圖項目“文檔和部署”
`mochi-core` 進入具體恢復步驟。

## 1. 急救人員清單

1. 捕獲 MOCHI 正在使用的數據根。默認如下
   `$TMPDIR/mochi/<profile-slug>`；自定義路徑出現在 UI 標題欄中，並且
   通過 `cargo run -p mochi-ui-egui -- --data-root ...`。
2. 從工作區根目錄運行 `./ci/check_mochi.sh`。這驗證了核心，
   在開始修改配置之前，先了解 UI 和集成箱。
3. 記下預設（`single-peer` 或 `four-peer-bft`）。生成的拓撲
   確定數據根下應有多少對等文件夾/日誌。

## 2.收集日誌和遙測證據

`NetworkPaths::ensure`（參見 `mochi/mochi-core/src/config.rs`）創建了一個穩定的
佈局：

```
<data_root>/<profile>/
  peers/<alias>/...
  logs/<alias>.log
  genesis/
  snapshots/
```

進行更改之前請按照以下步驟操作：

- 使用 **Logs** 選項卡或直接打開 `logs/<alias>.log` 捕獲最後一個
  每個對等點 200 行。主管尾部標準輸出/標準錯誤/系統通道
  通過 `PeerLogStream`，因此這些文件與 UI 輸出匹配。
- 通過**維護→導出快照**導出快照（或調用
  `Supervisor::export_snapshot`）。快照捆綁了存儲、配置和
  登錄 `snapshots/<timestamp>-<label>/`。
- 如果問題涉及流小部件，請複制 `ManagedBlockStream`，
  `ManagedEventStream` 和 `ManagedStatusStream` 運行狀況指標
  儀表板。 UI 顯示上次重新連接嘗試和錯誤原因；搶
  事件記錄的屏幕截圖。

## 3. 解決對等啟動問題

大多數對等啟動失敗分為三類：

### 缺少二進製文件或錯誤覆蓋

`SupervisorBuilder` 外殼為 `irohad`、`kagami` 和（未來）`iroha_cli`。
如果 UI 報告“無法生成進程”或“權限被拒絕”，請點 MOCHI
在已知良好的二進製文件中：

```bash
cargo run -p mochi-ui-egui -- \
  --irohad /path/to/irohad \
  --kagami /path/to/kagami \
  --iroha-cli /path/to/iroha_cli
```

您可以設置 `MOCHI_IROHAD`、`MOCHI_KAGAMI` 和 `MOCHI_IROHA_CLI` 以避免
重複輸入標誌。調試捆綁包構建時，比較
`mochi/mochi-ui-egui/src/config/` 中的 `BundleConfig` 與
`target/mochi-bundle`。

### 端口衝突

`PortAllocator` 在寫入配置之前探測環回接口。如果你看到
`failed to allocate Torii port` 或 `failed to allocate P2P port`，另一個
進程已在默認範圍 (8080/1337) 上偵聽。重新啟動MOCHI
具有明確的基礎：

```bash
cargo run -p mochi-ui-egui -- --torii-start 12000 --p2p-start 19000
```

構建器將從這些基地扇出連續端口，因此保留一個範圍
根據您的預設調整大小（`peer_count` 對等體→ `peer_count` 每個傳輸端口）。

### 創世和存儲損壞如果 Kagami 在發出清單之前退出，對等點將立即崩潰。檢查
數據根內的 `genesis/*.json`/`.toml`。重新運行
`--kagami /path/to/kagami` 或將 **設置** 對話框指向右側的二進製文件。
對於存儲損壞，請使用維護部分的**擦除和重新生成**
按鈕（如下所示），而不是手動刪除文件夾；它重新創建了
重新啟動進程之前的對等目錄和快照根。

### 調整自動重啟

`config/local.toml` 中的 `[supervisor.restart]` （或 CLI 標誌
`--restart-mode`、`--restart-max`、`--restart-backoff-ms`) 控制頻率
主管重試失敗的對等點。當需要UI時設置`mode = "never"`
立即出現第一個故障，或縮短 `max_restarts`/`backoff_ms`
收緊必須快速失敗的 CI 作業的重試窗口。

## 4. 安全重置對等點

1. 從儀表板停止受影響的對等點或退出 UI。主管
   拒絕在對等方運行時擦除存儲（`PeerHandle::wipe_storage`
   返回 `PeerStillRunning`)。
2. 導航至**維護 → 擦除和重新生成**。莫奇將：
   -刪除`peers/<alias>/storage`；
   - 重新運行 Kagami 以在 `genesis/` 下重建配置/創世；和
   - 使用保留的 CLI/環境覆蓋重新啟動對等點。
3. 如果您必須手動執行此操作：
   ```bash
   cargo run -p mochi-ui-egui -- --data-root /tmp/mochi --profile four-peer-bft --help
   # Note the actual root printed above, then:
   rm -rf /tmp/mochi/four-peer-bft
   ```
   然後，重新啟動 MOCHI，以便 `NetworkPaths::ensure` 重新創建樹。

擦除之前務必存檔 `snapshots/<timestamp>` 文件夾，即使在本地也是如此
開發 - 這些捆綁包捕獲所需的精確 `irohad` 日誌和配置
重現錯誤。

### 4.1 從快照恢復

當實驗損壞存儲或者您需要重播已知良好的狀態時，請使用維護
對話框的 **恢復快照** 按鈕（或調用 `Supervisor::restore_snapshot`）而不是複制
手動目錄。提供捆綁包的絕對路徑或經過清理的文件夾名稱
在 `snapshots/` 下。主管將：

1. 停止任何正在運行的節點；
2. 驗證快照的 `metadata.json` 是否與當前 `chain_id` 和對等計數匹配；
3. 將 `peers/<alias>/{storage,snapshot,config.toml,latest.log}` 複製回活動配置文件中；和
4. 如果對等點之前正在運行，請在重新啟動對等點之前恢復 `genesis/genesis.json`。

如果快照是為不同的預設或鏈標識符創建的，則恢復調用將返回
`SupervisorError::Config`，這樣您就可以獲取匹配的捆綁包，而不是默默地混合人工製品。
每個預設至少保留一個新快照，以加速恢復演練。

## 5. 修復塊/事件/狀態流- **流停滯但對等體健康。 **檢查**事件**/**塊**面板
  用於紅色狀態欄。單擊“停止”，然後單擊“啟動”以強制託管流
  重新訂閱；主管記錄每次重新連接嘗試（使用對等別名和
  錯誤），以便您可以確認退避階段。
- **狀態覆蓋已過時。 ** `ManagedStatusStream` 每隔一次輪詢 `/status`
  兩秒並在“STATUS_POLL_INTERVAL *”之後標記數據過時
  STATUS_STALE_MULTIPLIER`（默認六秒）。如果徽章保持紅色，請驗證
  對等配置中的 `torii_status_url` 並確保網關或 VPN 不是
  阻止環回連接。
- **事件解碼失敗。 ** UI 打印解碼階段（原始字節、
  `BlockSummary` 或 Norito 解碼）和違規交易哈希。出口
  通過剪貼板按鈕事件，以便您可以在測試中重現解碼
  （`mochi-core` 公開了輔助構造函數
  `mochi/mochi-core/src/torii.rs`）。

當流反复崩潰時，使用確切的對等別名更新問題並
錯誤字符串 (`ToriiErrorKind`)，因此路線圖遙測里程碑保持聯繫
到具體的證據。