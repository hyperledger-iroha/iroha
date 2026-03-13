---
lang: zh-hant
direction: ltr
source: docs/source/android_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: da7119ab99121dbcfc268f5406f43b16ac9149cef6500a45c6717ad16c02ab80
source_last_modified: "2026-01-28T15:38:09.507154+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Android SDK 操作手冊

此運行手冊支持操作員和支持工程師管理 Android SDK
AND7 及更高版本的部署。與 SLA 的 Android 支持手冊配對
定義和升級路徑。

> **注意：** 更新事件過程時，同時刷新共享的
> 故障排除矩陣 (`docs/source/sdk/android/troubleshooting.md`)
> 場景表、SLA 和遙測參考與本運行手冊保持一致。

## 0. 快速入門（尋呼機觸發時）

在深入了解詳細信息之前，請將此序列放在方便的 Sev1/Sev2 警報中
以下部分：

1. **確認活動配置：** 捕獲 `ClientConfig` 清單校驗和
   在應用程序啟動時發出並將其與固定清單進行比較
   `configs/android_client_manifest.json`。如果哈希值出現分歧，則停止發布並
   在接觸遙測/覆蓋之前提交配置漂移票證（請參閱§1）。
2. **運行架構差異門：** 針對以下對象執行 `telemetry-schema-diff` CLI
   接受的快照
   （`docs/source/sdk/android/readiness/schema_diffs/android_vs_rust-20260305.json`）。
   將任何 `policy_violations` 輸出視為 Sev2 並阻止導出，直到
   差異是可以理解的（參見§2.6）。
3. **檢查儀表板 + 狀態 CLI：** 打開 Android Telemetry Redaction 並
   出口商健康委員會，然後運行
   `scripts/telemetry/check_redaction_status.py --status-url <collector>`。如果
   當局在地板下或導出錯誤，捕獲屏幕截圖和
   事件文檔的 CLI 輸出（請參閱§2.4–§2.5）。
4. **決定覆蓋：** 僅在上述步驟之後並與事件/所有者一起
   記錄，通過 `scripts/android_override_tool.sh` 發出有限覆蓋
   並將其記錄在 `telemetry_override_log.md` 中（參見第 3 節）。默認有效期：<24 小時。
5. **按聯繫人列表升級：** 分頁 Android 待命和可觀察性 TL
   （第 8 節中的聯繫方式），然後遵循第 4.1 節中的升級樹。如果證明或
   涉及 StrongBox 信號，拉動最新的捆綁包並運行線束
   在重新啟用導出之前檢查第 7 節。

## 1. 配置與部署

- **ClientConfig 來源：** 確保 Android 客戶端加載 Torii 端點、TLS
  策略，以及來自 `iroha_config` 派生清單的重試旋鈕。驗證
  應用程序啟動期間的值以及活動清單的日誌校驗和。
  實現參考：`java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/ClientConfig.java`
  來自 `java/iroha_android/src/main/java/org/hyperledger/iroha/android/telemetry/TelemetryOptions.java` 的線程 `TelemetryOptions`
  （加上生成的 `TelemetryObserver`），因此散列權限會自動發出。
- **熱重載：** 使用配置觀察器拾取 `iroha_config`
  更新無需重新啟動應用程序。失敗的重新加載應該發出
  `android.telemetry.config.reload` 事件和触髮指數重試
  退避（最多 5 次嘗試）。
- **回退行為：** 當配置丟失或無效時，回退到
  安全默認值（只讀模式，無待處理隊列提交）並顯示用戶
  提示。記錄事件以便後續處理。

### 1.1 配置重新加載診斷- 配置觀察器發出 `android.telemetry.config.reload` 信號
  `source`、`result`、`duration_ms` 和可選 `digest`/`error` 字段（請參閱
  `configs/android_telemetry.json` 和
  `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/ConfigWatcher.java`）。
  每個應用的清單預計有一個 `result:"success"` 事件；重複
  `result:"error"` 記錄表明觀察者已用盡 5 次退避嘗試
  從 50 毫秒開始。
- 發生事件期間，從收集器捕獲最新的重新加載信號
  （OTLP/span 存儲或編輯狀態端點）並記錄 `digest` +
  事件文檔中的 `source`。將摘要與
  `configs/android_client_manifest.json` 和發布清單分發到
  運營商。
- 如果觀察者繼續發出錯誤，請運行目標工具來重現
  可疑清單解析失敗：
  `ci/run_android_tests.sh org.hyperledger.iroha.android.client.ConfigWatcherTests`。
  將測試輸出和失敗清單附加到事件包中，以便 SRE
  可以將其與烘焙的配置模式進行比較。
- 當重新加載遙測丟失時，確認活動的 `ClientConfig` 帶有
  遙測接收器並且 OTLP 收集器仍然接受
  `android.telemetry.config.reload` ID；否則將其視為 Sev2 遙測
  回歸（與§2.4 相同的路徑）並暫停釋放，直到信號返回。

### 1.2 確定性密鑰導出包
- 軟件導出現在發出帶有每個導出鹽 + 隨機數、`kdf_kind` 和 `kdf_work_factor` 的 v3 捆綁包。
  導出器更喜歡 Argon2id（64 MiB，3 次迭代，並行度 = 2）並回退到
  當 Argon2id 在設備上不可用時，PBKDF2-HMAC-SHA256 具有 350 k 迭代層。捆綁包
  AAD 仍然綁定到別名；對於 v3 導出，密碼必須至少為 12 個字符，並且
  進口商拒絕全零鹽/隨機數種子。
  `KeyExportBundle.decode(Base64|bytes)`，使用原始密碼導入，然後重新導出到 v3
  轉向內存硬格式。進口商拒絕全零或重複使用的鹽/隨機數對；總是
  輪換捆綁包，而不是在設備之間重複使用舊的導出。
- `ci/run_android_tests.sh --tests org.hyperledger.iroha.android.crypto.export.DeterministicKeyExporterTests` 中的負路徑測試
  拒絕。使用後清除密碼字符數組並捕獲捆綁版本和 `kdf_kind`
  在恢復失敗時的事件註釋中。

## 2. 遙測和編輯

> 快速參考：參見
> [`telemetry_redaction_quick_reference.md`](sdk/android/telemetry_redaction_quick_reference.md)
> 用於啟用期間使用的精簡命令/閾值清單
> 會話和事件橋樑。- **信號清單：** 參考 `docs/source/sdk/android/telemetry_redaction.md`
  有關發出的跨度/指標/事件的完整列表以及
  `docs/source/sdk/android/readiness/signal_inventory_worksheet.md`
  了解所有者/驗證詳細信息和突出的差距。
- **規範模式差異：** 批准的 AND7 快照是
  `docs/source/sdk/android/readiness/schema_diffs/android_vs_rust-20260305.json`。
  每個新的 CLI 運行都必須與此工件進行比較，以便審閱者可以看到
  接受的 `intentional_differences` 和 `android_only_signals` 仍然
  與中記錄的策略表相匹配
  `docs/source/sdk/android/telemetry_schema_diff.md` §3。 CLI 現在添加了
  `policy_violations` 當任何有意的差異丟失時
  `status:"accepted"`/`"policy_allowlisted"`（或者當僅限 Android 的記錄丟失時
  他們接受的狀態），因此將非空違規視為 Sev2 並停止
  出口。下面的 `jq` 片段保留作為對存檔的手動健全性檢查
  文物：
  ```bash
  jq '.intentional_differences[] | select(.status != "accepted" and .status != "policy_allowlisted")' "$OUT"
  jq '.android_only_signals[] | select(.status != "accepted")' "$OUT"
  jq '.field_mismatches[] | {signal, field, android, rust}' "$OUT"
  ```
  將這些命令的任何輸出視為需要一個模式回歸
  遙測導出繼續之前的 AND7 就緒錯誤； `field_mismatches`
  根據 `telemetry_schema_diff.md` §5，必須保持為空。助手現在寫道
  自動`artifacts/android/telemetry/schema_diff.prom`；通過
  `--textfile-dir /var/lib/node_exporter/textfile_collector`（或設置
  `ANDROID_SCHEMA_DIFF_TEXTFILE_DIR`) 在臨時/生產主機上運行時
  因此 `telemetry_schema_diff_run_status` 儀表翻轉為 `policy_violation`
  如果 CLI 檢測到漂移，則會自動進行。
- **CLI 幫助程序：** `scripts/telemetry/check_redaction_status.py` 檢查
  默認為`artifacts/android/telemetry/status.json`；將 `--status-url` 傳遞給
  查詢暫存和 `--write-cache` 刷新本地副本以供離線使用
  演習。使用 `--min-hashed 214`（或設置
  `ANDROID_TELEMETRY_MIN_HASHED_AUTHORITIES=214`) 強制執行治理
  在每次狀態民意調查中，對散列權威的底線。
- **權限哈希：** 所有權限都使用 Blake2b-256 進行哈希處理
  每季度輪換鹽存儲在安全秘密庫中。旋轉發生在
  每個季度的第一個星期一 00:00 UTC。核實出口商提貨
  通過檢查 `android.telemetry.redaction.salt_version` 指標來確定新的鹽。
- **設備配置文件存儲桶：** 僅 `emulator`、`consumer` 和 `enterprise`
  層被導出（與 SDK 主要版本一起）。儀表板比較這些
  根據 Rust 基線進行計數；方差 >10% 會引發警報。
- **網絡元數據：** Android 僅導出 `network_type` 和 `roaming` 標誌。
  運營商名稱永遠不會被公開；運營商不應要求訂戶
  事件日誌中的信息。清理後的快照作為
  `android.telemetry.network_context` 事件，因此請確保應用程序註冊
  `NetworkContextProvider`（通過
  `ClientConfig.Builder.setNetworkContextProvider(...)` 還是方便
  `enableAndroidNetworkContext(...)` 幫助程序）在發出 Torii 調用之前。
- **Grafana 指針：** `Android Telemetry Redaction` 儀表板是
  對上述 CLI 輸出進行規範的目視檢查 — 確認
  `android.telemetry.redaction.salt_version`面板與當前鹽紀元匹配
  並且 `android_telemetry_override_tokens_active` 小部件保持為零
  每當沒有演習或事件發生時。如果任一面板出現偏差則升級
  在 CLI 腳本報告回歸之前。

### 2.1 導出管道工作流程1. **配置分配。 ** `ClientConfig.telemetry.redaction` 是從線程
   `iroha_config` 並由 `ConfigWatcher` 熱重載。每次重新加載都會記錄
   明顯的消化加鹽時代——捕捉事件中和期間的那條線
   排練。
2. **Instrumentation.** SDK 組件將 span/metrics/events 發送到
   `TelemetryBuffer`。緩衝區用設備配置文件標記每個有效負載，並且
   當前的鹽紀元，以便導出器可以確定性地驗證哈希輸入。
3. **編輯過濾器。 ** `RedactionFilter` 哈希值 `authority`、`alias` 和
   設備標識符在離開設備之前。故障發出
   `android.telemetry.redaction.failure` 並阻止導出嘗試。
4. **導出器 + 收集器。 ** 淨化後的有效負載通過 Android 傳送
   OpenTelemetry 導出器到 `android-otel-collector` 部署。的
   收集器風扇輸出到軌跡 (Tempo)、指標 (Prometheus) 和 Norito
   日誌下沉。
5. **可觀察性掛鉤。 ** `scripts/telemetry/check_redaction_status.py` 讀取
   收集器計數器（`android.telemetry.export.status`，
   `android.telemetry.redaction.salt_version`) 並生成狀態包
   本運行手冊中均引用了該內容。

### 2.2 驗證門

- **架構差異：** 運行
  `scripts/telemetry/run_schema_diff.sh --android-config configs/android_telemetry.json --rust-config configs/rust_telemetry.json`
  每當出現變化時。每次運行後，確認每個
  `intentional_differences[*]` 和 `android_only_signals[*]` 條目帶有標記
  `status:"accepted"`（或 `status:"policy_allowlisted"` 用於散列/分桶
  字段）按照 `telemetry_schema_diff.md` §3 中的建議，然後再附加
  事件和混亂實驗室報告的人工製品。使用批准的快照
  (`android_vs_rust-20260305.json`) 作為護欄和棉絨新鮮排放
  提交之前的 JSON：
  ```bash
  LATEST=docs/source/sdk/android/readiness/schema_diffs/$(date -u +%Y%m%d).json
  jq '.intentional_differences[] | select(.status != "accepted" and .status != "policy_allowlisted") | {signal, field, status}' "$LATEST"
  jq '.android_only_signals[] | select(.status != "accepted") | {signal, status}' "$LATEST"
  ```
  將 `$LATEST` 與
  `docs/source/sdk/android/readiness/schema_diffs/android_vs_rust-20260305.json`
  證明允許名單保持不變。 `status` 缺失或空白
  條目（例如 `android.telemetry.redaction.failure` 或
  `android.telemetry.redaction.salt_version`) 現在被視為回歸
  必須在審核結束前得到解決； CLI 顯示接受的
  直接說明，因此手冊§3.4交叉引用僅適用於
  解釋為什麼出現非 `accepted` 狀態。

  **規範 AND7 信號（2026 年 3 月 5 日快照）**|信號|頻道|狀態 |治理說明|驗證鉤子 |
  |--------|---------|--------|-----------------|-----------------|
  | `android.telemetry.redaction.override` |活動 | `accepted` |鏡像會覆蓋清單並且必須與 `telemetry_override_log.md` 條目匹配。 |觀看 `android_telemetry_override_tokens_active` 並按照 §3 歸檔清單。 |
  | `android.telemetry.network_context` |活動 | `accepted` | Android 有意修改運營商名稱；僅導出 `network_type` 和 `roaming`。 |確保應用程序註冊 `NetworkContextProvider` 並確認事件量遵循 `Android Telemetry Overview` 上的 Torii 流量。 |
  | `android.telemetry.redaction.failure` |專櫃| `accepted` |每當散列失敗時發出；治理現在需要模式差異工件中的顯式狀態元數據。 | `Redaction Compliance` 儀表板面板和 `check_redaction_status.py` 的 CLI 輸出必須保持為零（演習期間除外）。 |
  | `android.telemetry.redaction.salt_version` |儀表| `accepted` |證明出口商正在使用當前的季度鹽紀元。 |將 Grafana 的 salt 小部件與 Secrets-Vault 紀元進行比較，並確保架構差異運行保留 `status:"accepted"` 註釋。 |

  如果上表中的任何條目刪除了 `status`，則差異偽影必須是
  重新生成**和** `telemetry_schema_diff.md` 在 AND7 之前更新
  治理數據包已分發。將刷新的 JSON 包含在
  `docs/source/sdk/android/readiness/schema_diffs/` 並從
  觸發重新運行的事件、混沌實驗室或啟用報告。
- **CI/單位覆蓋範圍：** `ci/run_android_tests.sh` 必須先通過
  發布版本；該套件通過執行來強制散列/覆蓋行為
  具有示例有效負載的遙測導出器。
- **噴油器健全性檢查：** 使用
  `scripts/telemetry/inject_redaction_failure.sh --dry-run` 排練前
  確認故障注入有效，並在散列防護時發出警報
  被絆倒。驗證後始終使用 `--clear` 清除噴油器
  完成。

### 2.3 移動 ↔ Rust 遙測奇偶校驗清單

保持 Android 導出器和 Rust 節點服務保持一致，同時尊重
不同的編輯要求記錄在
`docs/source/sdk/android/telemetry_redaction.md`。下表作為
AND7 路線圖條目中引用的雙重允許列表 - 每當
模式差異引入或刪除字段。|類別 | Android 出口商 |防銹服務|驗證鉤子 |
|----------|--------------------|------------------------|------|
|權限/路線上下文|通過 Blake2b-256 對 `authority`/`alias` 進行哈希處理，並在導出之前刪除原始 Torii 主機名；發出 `android.telemetry.redaction.salt_version` 以證明鹽旋轉。 |發出完整的 Torii 主機名和對等 ID 以進行關聯。 |在 `readiness/schema_diffs/` 下的最新架構差異中比較 `android.torii.http.request` 與 `torii.http.request` 條目，然後通過運行 `scripts/telemetry/check_redaction_status.py` 確認 `android.telemetry.redaction.salt_version` 與集群鹽匹配。 |
|設備和簽名者身份 |存儲桶 `hardware_tier`/`device_profile`、哈希控制器別名，並且從不導出序列號。 |無設備元數據；節點逐字發出驗證器 `peer_id` 和控制器 `public_key`。 |鏡像 `docs/source/sdk/mobile_device_profile_alignment.md` 中的映射，在實驗室期間審核 `PendingQueueInspector` 輸出，並確保 `ci/run_android_tests.sh` 內的別名哈希測試保持綠色。 |
|網絡元數據|僅導出 `network_type` + `roaming` 布爾值； `carrier_name` 已刪除。 | Rust 保留對等主機名以及完整的 TLS 端點元數據。 |將最新的 diff JSON 存儲在 `readiness/schema_diffs/` 中，並確認 Android 端仍然省略 `carrier_name`。如果 Grafana 的“網絡上下文”小部件顯示任何運營商字符串，則會發出警報。 |
|覆蓋/混亂證據|使用蒙面演員角色發出 `android.telemetry.redaction.override` 和 `android.telemetry.chaos.scenario` 事件。 | Rust 服務發出覆蓋批准，無需角色屏蔽，也沒有特定於混沌的跨度。 |每次演練後交叉檢查 `docs/source/sdk/android/readiness/and7_operator_enablement.md`，以確保覆蓋令牌和混亂工件與未屏蔽的 Rust 事件一起存檔。 |

奇偶校驗工作流程：

1. 每次清單或導出器更改後，運行
   `scripts/telemetry/run_schema_diff.sh --android-config configs/android_telemetry.json --rust-config configs/rust_telemetry.json --out docs/source/sdk/android/readiness/schema_diffs/$(date -u +%Y%m%d).json --textfile-dir /var/lib/node_exporter/textfile_collector`
   因此 JSON 工件和鏡像指標都位於證據包中
   （助手默認情況下仍然寫入 `artifacts/android/telemetry/schema_diff.prom`）。
2. 對照上表查看差異；如果 Android 現在發出一個字段
   僅允許在 Rust 上（反之亦然），提交 AND7 就緒性錯誤並更新
   修訂計劃。
3. 在每週檢查期間，運行
   `scripts/telemetry/check_redaction_status.py --status-url https://android-telemetry-stg/api/redaction/status`
   確認鹽紀元與 Grafana 小部件匹配並記下中的紀元
   待命日記。
4. 記錄所有增量
   `docs/source/sdk/android/readiness/signal_inventory_worksheet.md`所以
   治理可以審核平等決策。

### 2.4 可觀測性儀表板和警報閾值

使儀表板和警報與 AND7 架構差異批准保持一致
查看 `scripts/telemetry/check_redaction_status.py` 輸出：

- `Android Telemetry Redaction` — 鹽紀元小部件，覆蓋代幣計量表。
- `Redaction Compliance` — `android.telemetry.redaction.failure` 計數器和
  噴油器趨勢面板。
- `Exporter Health` — `android.telemetry.export.status` 速率細分。
- `Android Telemetry Overview` — 設備配置文件存儲桶和網絡上下文卷。

以下閾值反映了快速參考卡，必須強制執行
在事件響應和排練期間：|公制/面板|門檻|行動|
|----------------|---------|--------|
| `android.telemetry.redaction.failure`（`Redaction Compliance`板）| >0 在滾動 15 分鐘窗口內 |調查故障信號，運行清除注入器，記錄 CLI 輸出 + Grafana 屏幕截圖。 |
| `android.telemetry.redaction.salt_version`（`Android Telemetry Redaction`板）|與秘密庫鹽時代的不同|停止發布，與秘密輪換協調，歸檔 AND7 註釋。 |
| `android.telemetry.export.status{status="error"}`（`Exporter Health`板）| > 出口額的 1% |檢查收集器運行狀況、捕獲 CLI 診斷、升級到 SRE。 |
| `android.telemetry.device_profile{tier="enterprise"}` 與 Rust 奇偶校驗 (`Android Telemetry Overview`) |與 Rust 基線的差異 >10% |文件治理跟進，驗證夾具池，註釋模式差異工件。 |
| `android.telemetry.network_context` 卷 (`Android Telemetry Overview`) |當 Torii 流量存在時，流量降至零 |確認 `NetworkContextProvider` 註冊，重新運行 schema diff 以確保字段不變。 |
| `android.telemetry.redaction.override` / `android_telemetry_override_tokens_active` (`Android Telemetry Redaction`) |非零外部批准覆蓋/鑽取窗口 |將令牌與事件聯繫起來，重新生成摘要，並通過第 3 節中的工作流程進行撤銷。 |

### 2.5 操作員準備和支持跟踪

路線圖項目 AND7 提出了專門的操作員課程，以便支持、SRE 和
發布干係人在運行手冊發布之前了解上面的奇偶校驗表
GA。使用中的輪廓
`docs/source/sdk/android/telemetry_readiness_outline.md` 用於規範物流
（議程、演講者、時間表）和 `docs/source/sdk/android/readiness/and7_operator_enablement.md`
查看詳細的清單、證據鏈接和操作日誌。保留以下內容
每當遙測計劃發生變化時，階段都會同步：|相|描述 |證據包|主要業主 |
|--------|-------------|--------------------|------------------------|
|預讀發行|在簡報會前至少五個工作日發送保單預讀 `telemetry_redaction.md` 和快速參考卡。跟踪大綱通信日誌中的確認。 | `docs/source/sdk/android/telemetry_readiness_outline.md`（會話物流 + 通信日誌）和 `docs/source/sdk/android/readiness/archive/<YYYY-MM>/` 中的存檔電子郵件。 |文檔/支持經理 |
|現場準備會議|提供 60 分鐘的培訓（政策深入探討、運行手冊演練、儀表板、混沌實驗室演示）並為異步觀看者保持錄製運行。 |錄音 + 幻燈片存儲在 `docs/source/sdk/android/readiness/archive/<YYYY-MM>/` 下，並在大綱的第 2 節中捕獲參考文獻。 |法學碩士（AND7 代理所有者）|
|混沌實驗室執行|在實時會話結束後立即從 `docs/source/sdk/android/readiness/labs/telemetry_lab_01.md` 運行至少 C2（覆蓋）+ C6（隊列重播），並將日誌/屏幕截圖附加到支持套件。 | `docs/source/sdk/android/readiness/labs/reports/<YYYY-MM>/` 和 `/screenshots/<YYYY-MM>/` 內的場景報告和屏幕截圖。 | Android 可觀測性 TL + SRE 待命 |
|知識檢查與考勤|收集提交的測驗，糾正得分低於 90% 的人，並記錄出勤/測驗統計數據。讓快速參考問題與奇偶校驗清單保持一致。 |測驗在 `docs/source/sdk/android/readiness/forms/responses/` 中導出，摘要 Markdown/JSON 通過 `scripts/telemetry/generate_and7_quiz_summary.py` 生成，以及 `and7_operator_enablement.md` 中的考勤表。 |支持工程|
|存檔及後續|更新啟用工具包的操作日誌，將工件上傳到存檔，並在 `status.md` 中記下完成情況。會話期間發出的任何修復或覆蓋令牌都必須複製到 `telemetry_override_log.md` 中。 | `docs/source/sdk/android/readiness/and7_operator_enablement.md` §6（操作日誌）、`.../archive/<YYYY-MM>/checklist.md` 以及§3 中引用的覆蓋日誌。 |法學碩士（AND7 代理所有者）|

當課程重新運行時（每季度或在主要架構更改之前），刷新
包含新會議日期的大綱，保持與會者名單最新，以及
重新生成測驗摘要 JSON/Markdown 工件，以便治理數據包可以
參考一致的證據。 AND7 的 `status.md` 條目應鏈接到
每個啟用衝刺結束後的最新存檔文件夾。

### 2.6 架構差異允許列表和策略檢查

該路線圖明確提出了雙重白名單政策（移動編輯與
防銹保留）由 `telemetry-schema-diff` CLI 強制執行，位於
`tools/telemetry-schema-diff`。每個差異工件記錄在
`docs/source/sdk/android/readiness/schema_diffs/` 必須記錄哪些字段
Android 上已散列/分桶，哪些字段在 Rust 上保持未散列，以及是否
任何非允許的信號都會溜進構建中。記錄這些決定
直接在 JSON 中運行：

```bash
cargo run -p telemetry-schema-diff -- \
  --android-config configs/android_telemetry.json \
  --rust-config configs/rust_telemetry.json \
  --format json \
  > "$LATEST"

if jq -e '.policy_violations | length > 0' "$LATEST" >/dev/null; then
  jq '.policy_violations[]' "$LATEST"
  exit 1
fi
```當報告乾淨時，最終的 `jq` 評估為無操作。處理任何輸出
從該命令作為 Sev2 就緒性錯誤：填充的 `policy_violations`
array 表示 CLI 發現了一個不在僅限 Android 的列表中的信號
也不在僅 Rust 的豁免列表中記錄的
`docs/source/sdk/android/telemetry_schema_diff.md`。出現這種情況時，請停止
導出，提交 AND7 票證，並僅在策略模塊之後重新運行 diff
清單快照已得到更正。將生成的 JSON 存儲在
`docs/source/sdk/android/readiness/schema_diffs/` 帶有日期後綴和註釋
事件或實驗室報告內的路徑，以便治理可以重播檢查。

**散列和保留矩陣**

|信號場 |安卓處理 |防銹處理 |允許列表標籤 |
|--------------|-----------------|----------------|---------------|
| `torii.http.request.authority` | Blake2b-256 散列 (`representation: "blake2b_256"`) |逐字存儲以實現可追溯性 | `policy_allowlisted`（移動哈希）|
| `attestation.result.alias` | Blake2b-256 散列 |純文本別名（證明檔案）| `policy_allowlisted` |
| `attestation.result.device_tier` |桶裝 (`representation: "bucketed"`) |普通層字符串 | `policy_allowlisted` |
| `hardware.profile.hardware_tier` |缺席——Android出口商完全放棄這個領域不加編輯地呈現 | `rust_only`（記錄在 `telemetry_schema_diff.md` 的第 3 節中）|
| `android.telemetry.redaction.override.*` |僅限 Android 的信號，帶有蒙面演員角色 |沒有發出等效信號 | `android_only`（必須保留 `status:"accepted"`）|

當出現新信號時，將它們添加到架構差異策略模塊**和**
因此 Runbook 反映了 CLI 中提供的強制邏輯。
如果任何僅限 Android 的信號省略顯式 `status` 或如果
`policy_violations` 數組非空，因此請保持此清單與
`telemetry_schema_diff.md` §3 以及中引用的最新 JSON 快照
`telemetry_redaction_minutes_*.md`。

## 3. 覆蓋工作流程

在散列回歸或隱私時，覆蓋是“打破玻璃”的選項
警報會阻止客戶。僅在記錄完整決策軌跡後應用它們
在事件文檔中。1. **確認偏差和範圍。 ** 等待 PagerDuty 警報或架構差異
   門開火，然後跑
   `scripts/telemetry/check_redaction_status.py --status-url <collector>` 至
   證明當局不匹配。附上 CLI 輸出和 Grafana 屏幕截圖
   到事件記錄。
2. **準備簽名請求。 ** 填充
   `docs/examples/android_override_request.json` 以及票證 ID、請求者、
   到期日和理由。將文件存儲在事件文物旁邊，以便
   合規性可以審核輸入。
3. **發出覆蓋。 ** 調用
   ```bash
   scripts/android_override_tool.sh apply \
     --request docs/examples/android_override_request.json \
     --log docs/source/sdk/android/telemetry_override_log.md \
     --out artifacts/android/telemetry/override-$(date -u +%Y%m%dT%H%M%SZ).json \
     --event-log docs/source/sdk/android/readiness/override_logs/override_events.ndjson \
     --actor-role <support|sre|docs|compliance|program|other>
   ```
   幫助程序打印覆蓋令牌，寫入清單，並附加一行
   到 Markdown 審核日誌。切勿在聊天中發布令牌；直接交付
   到應用覆蓋的 Torii 操作員。
4. **監控效果。 ** 五分鐘內驗證單個
   `android.telemetry.redaction.override` 事件已發出，收集器
   狀態端點顯示 `override_active=true`，事件文檔列出了
   到期。觀看 Android 遙測概述儀表板的“覆蓋令牌
   活動”面板（`android_telemetry_override_tokens_active`）相同
   令牌計數並繼續每 10 分鐘運行一次狀態 CLI，直到
   散列穩定。
5. **撤銷並存檔。 ** 緩解措施生效後，立即運行
  `scripts/android_override_tool.sh revoke --token <token>` 所以審核日誌
  捕獲撤銷時間，然後執行
  `scripts/android_override_tool.sh digest --out docs/source/sdk/android/readiness/override_logs/override_digest_$(date -u +%Y%m%dT%H%M%SZ).json`
  刷新治理期望的經過清理的快照。附上
  清單、摘要 JSON、CLI 轉錄本、Grafana 快照和 NDJSON 日誌
  通過 `--event-log` 生成
  `docs/source/sdk/android/readiness/screenshots/<date>/` 並交聯
  來自 `docs/source/sdk/android/telemetry_override_log.md` 的條目。

超過 24 小時的覆蓋需要 SRE 總監和合規部批准，並且
必須在下週 AND7 審核中重點強調。

### 3.1 覆蓋升級矩陣

|情況|最長持續時間|批准人 |所需通知 |
|------------|--------------|------------------------|------------------------|
|單租戶調查（散列權限不匹配，客戶 Sev2）| 4小時|支持工程師+SRE on-call |票證 `SUP-OVR-<id>`、`android.telemetry.redaction.override` 事件、事件日誌 |
|整個艦隊的遙測中斷或 SRE 請求的複制 | 24小時 | SRE 隨叫隨到 + 項目主管 | PagerDuty 註釋，覆蓋日誌條目，在 `status.md` 中更新 |
|合規/取證請求或任何超過 24 小時的案件 |直到明確撤銷| SRE 總監 + 合規主管 |治理郵件列表、覆蓋日誌、AND7 每週狀態 |

#### 角色職責|角色 |職責| SLA / 註釋 |
|------|--------------------|-------------|
| Android 遙測待命（事件指揮官）|驅動檢測、執行覆蓋工具、在事件文檔中記錄批准，並確保在到期之前進行撤銷。 |在 5 分鐘內確認 PagerDuty，並每 15 分鐘記錄一次進度。 |
| Android 可觀測性 TL（Haruka Yamamoto）|驗證漂移信號，確認出口商/收集商狀態，並在將其交給操作員之前在覆蓋清單上簽字。 | 10分鐘內上橋；如果不可用，則委託給暫存集群所有者。 |
| SRE 聯絡員 (Liam O’Connor) |將清單應用於收集器、監控待辦事項並與發布工程協調以實現 Torii 端緩解措施。 |記錄更改請求中的每個 `kubectl` 操作，並將命令記錄粘貼到事件文檔中。 |
|合規性（索菲亞·馬丁斯/丹尼爾·帕克）|批准超過 30 分鐘的覆蓋，驗證審核日誌行，並就監管機構/客戶消息傳遞提供建議。 |在 `#compliance-alerts` 中發布確認；對於生產事件，在發布覆蓋之前提交合規說明。 |
|文檔/支持經理 (Priya Deshpande) |將清單/CLI 輸出存檔在 `docs/source/sdk/android/readiness/…` 下，保持覆蓋日誌整潔，並在出現差距時安排後續實驗。 |在結束事件之前確認證據保留（13 個月）並提交 AND7 後續行動。 |

如果任何覆蓋令牌即將到期且沒有任何更新，請立即升級
記錄撤銷計劃。

## 4. 事件響應

- **警報：** PagerDuty 服務 `android-telemetry-primary` 涵蓋密文
  故障、出口機停機和鏟斗漂移。在 SLA 窗口內確認
  （請參閱支持手冊）。
- **診斷：** 運行 `scripts/telemetry/check_redaction_status.py` 進行收集
  當前出口商的健康狀況、最近的警報和散列權威指標。包括
  事件時間線中的輸出 (`incident/YYYY-MM-DD-android-telemetry.md`)。
- **儀表板：** 監控 Android 遙測編輯、Android 遙測
  概述、修訂合規性和出口商健康狀況儀表板。捕獲
  事件記錄的屏幕截圖並註釋任何鹽版本或覆蓋
  結束事件之前的令牌偏差。
- **協調：** 參與發布工程以解決出口商問題、合規性
  覆蓋/PII 問題，以及嚴重 1 事件的項目負責人。

### 4.1 升級流程

Android 事件使用與 Android 相同的嚴重級別進行分類
支持 Playbook (§2.1)。下表總結了必須尋呼的人員以及如何尋呼
預計每個響應者都會很快加入橋樑。|嚴重性 |影響 |主要響應者（≤5 分鐘）|二次升級（≤10分鐘）|附加通知 |筆記|
|----------|--------|----------------------------|--------------------------------|----------------------------------------|--------|
|嚴重程度1 |面向客戶的中斷、隱私洩露或數據洩露 | Android 遙測待命 (`android-telemetry-primary`) | Torii 待命 + 項目主管 |合規性 + SRE 治理 (`#sre-governance`)、臨時集群所有者 (`#android-staging`) |立即啟動作戰室並打開共享文檔以進行命令記錄。 |
|嚴重程度2 |隊列退化、覆蓋誤用或長時間重放積壓 | Android 遙測隨叫隨到 | Android 基礎 TL + 文檔/支持管理器 |項目負責人，發布工程聯絡員 |如果超馳超過 24 小時，則升級至合規性。 |
|嚴重程度3 |單租戶問題、實驗室演練或諮詢警報 |支持工程師| Android 隨叫隨到（可選）|文檔/意識支持 |如果範圍擴大或多個租戶受到影響，請轉換為 Sev2。 |

|窗口|行動|所有者 |證據/註釋|
|--------|--------|----------|----------------|
| 0–5 分鐘 |確認 PagerDuty，分配事件指揮官 (IC)，並創建 `incident/YYYY-MM-DD-android-telemetry.md`。刪除 `#android-sdk-support` 中的鏈接加單線狀態。 |待命 SRE/支持工程師 | PagerDuty ack + 在其他事件日誌旁邊提交的事件存根的屏幕截圖。 |
| 5–15 分鐘 |運行 `scripts/telemetry/check_redaction_status.py --status-url https://android-telemetry-stg/api/redaction/status` 並將摘要粘貼到事件文檔中。 Ping Android 可觀測性 TL (Haruka Yamamoto) 和支持主管 (Priya Deshpande) 進行實時交接。 | IC + Android 可觀察性 TL |附加 CLI 輸出 JSON，記下打開的儀表板 URL，並標記誰擁有診斷。 |
| 15–25 分鐘 |讓暫存集群所有者（負責可觀察性的 Haruka Yamamoto，負責 SRE 的 Liam O’Connor）在 `android-telemetry-stg` 上進行重現。使用 `scripts/telemetry/generate_android_load.sh --cluster android-telemetry-stg` 進行種子加載並從 Pixel + 模擬器捕獲隊列轉儲以確認症狀奇偶性。 |暫存集群所有者 |將清理後的 `pending.queue` + `PendingQueueInspector` 輸出上傳到事件文件夾。 |
| 25–40 分鐘 |決定覆蓋、Torii 限製或 StrongBox 後備。如果懷疑 PII 暴露或非確定性哈希，請通過 `#compliance-alerts` 尋呼合規部（Sofia Martins、Daniel Park），並在同一事件線程中通知項目負責人。 | IC + 合規 + 項目主管 |鏈接覆蓋令牌、Norito 清單和批准註釋。 |
| ≥40分鐘|提供 30 分鐘狀態更新（PagerDuty 註釋 + `#android-sdk-support`）。如果尚未激活，請安排作戰室橋，記錄緩解預計時間，並確保發布工程 (Alexei Morozov) 隨時待命以滾動收集器/SDK 工件。 |集成電路 |帶時間戳的更新以及存儲在事件文件中的決策日誌，並在下週刷新期間在 `status.md` 中進行匯總。 |- 所有升級都必須使用 Android 支持手冊中的“所有者/下次更新時間”表反映在事件文檔中。
- 如果另一事件已經發生，請加入現有的作戰室並附加 Android 上下文，而不是啟動一個新的。
- 當事件觸及運行手冊空白時，在 AND7 JIRA 史詩中創建後續任務並標記 `telemetry-runbook`。

## 5. 混沌與準備練習

- 執行詳細的場景
  `docs/source/sdk/android/telemetry_chaos_checklist.md` 每季度及之前
  主要版本。使用實驗室報告模板記錄結果。
- 將證據（屏幕截圖、日誌）存儲在
  `docs/source/sdk/android/readiness/screenshots/`。
- 跟踪 AND7 史詩中帶有標籤 `telemetry-lab` 的補救票證。
- 場景圖：C1（編輯故障）、C2（覆蓋）、C3（出口商限電）、C4
  （使用具有漂移配置的 `run_schema_diff.sh` 的架構差異門），C5
  （設備配置文件偏差通過 `generate_android_load.sh` 播種），C6（Torii 超時
  + 隊列重播），C7（證明拒絕）。保持此編號與
  `telemetry_lab_01.md` 和添加練習時的混亂清單。

### 5.1 編輯漂移和覆蓋演練（C1/C2）

1. 通過注入哈希失敗
   `scripts/telemetry/inject_redaction_failure.sh` 並等待 PagerDuty
   警報（`android.telemetry.redaction.failure`）。捕獲 CLI 輸出
   `scripts/telemetry/check_redaction_status.py --status-url <collector>` 為
   事件記錄。
2. 使用 `--clear` 清除故障並確認警報在
   10分鐘；附上鹽/權限面板的 Grafana 屏幕截圖。
3. 使用以下命令創建簽名覆蓋請求
   `docs/examples/android_override_request.json`，應用它
   `scripts/android_override_tool.sh apply`，並通過以下方式驗證未散列的樣本
   檢查暫存中的導出器有效負載（查找
   `android.telemetry.redaction.override`）。
4. 使用 `scripts/android_override_tool.sh revoke --token <token>` 撤銷覆蓋，
   將覆蓋令牌哈希加上票證引用附加到
   `docs/source/sdk/android/telemetry_override_log.md`，並創建摘要 JSON
   在 `docs/source/sdk/android/readiness/override_logs/` 下。這將關閉
   混亂清單中的 C2 場景並使治理證據保持新鮮。

### 5.2 出口商限電和隊列重演演習（C3/C6）1. 縮小臨時收集器的規模（`kubectl scale
   deploy/android-otel-collector --replicas=0`) 來模擬導出器
   停電。通過狀態 CLI 跟踪緩衝區指標並確認警報觸發
   15 分鐘標記。
2. 恢復收集器，確認積壓排出，並將收集器日誌歸檔
   顯示重播完成的片段。
3. 在暫存 Pixel 和模擬器上，按照 ScenarioC6 操作：安裝
   `examples/android/operator-console`，切換飛行模式，提交演示
   傳輸，然後禁用飛行模式並觀察隊列深度指標。
4. 拉取每個待處理隊列 (`adb shell run-as  cat files/pending.queue >
   /tmp/.queue`), compile the inspector (`gradle -p java/iroha_android
   ：核心：類> / dev / null`), and run `java -cp構建/類
   org.hyperledger.iroha.android.tools.PendingQueueInspector --文件
   /tmp/.queue --json > 隊列重播-.json`。附上解碼的
   信封加上重放哈希值到實驗室日誌。
5. 更新混亂報告，包括導出器中斷持續時間、之前/之後的隊列深度、
   並確認 `android_sdk_offline_replay_errors` 仍為 0。

### 5.3 暫存集群混沌腳本 (android-telemetry-stg)

暫存集群負責人 Haruka Yamamoto (Android Observability TL) 和 Liam O’Connor
(SRE) 每當安排排練時都遵循此腳本。順序保持
參與者與遙測混亂清單保持一致，同時保證
捕獲人工製品以供治理。

**參與者**

|角色 |職責|聯繫我們 |
|------|--------------------|---------|
| Android 隨叫隨到 IC |驅動鑽機、協調 PagerDuty 註釋、擁有命令日誌 | PagerDuty `android-telemetry-primary`、`#android-sdk-support` |
|暫存集群所有者（Haruka、Liam）|門更改窗口，運行 `kubectl` 操作，快照集群遙測 | `#android-staging` |
|文檔/支持經理 (Priya) |記錄證據、跟踪實驗室清單、發布後續通知單 | `#docs-support` |

**飛行前協調**

- 演習前 48 小時，提交變更請求，列出計劃的內容
  方案 (C1–C7) 並將鏈接粘貼到 `#android-staging` 中，以便集群所有者
  可以阻止衝突的部署。
- 收集最新的 `ClientConfig` 哈希值和 `kubectl --context staging get pods
  -n android-telemetry-stg` 輸出建立基線狀態，然後存儲
  均位於 `docs/source/sdk/android/readiness/labs/reports/<date>/` 下。
- 確認設備覆蓋範圍（Pixel + 模擬器）並確保
  `ci/run_android_tests.sh` 編譯了實驗室使用的工具
  （`PendingQueueInspector`，遙測注入器）。

**執行檢查點**

- 在`#android-sdk-support`中宣布“混亂開始”，開始橋接錄音，
  並保持 `docs/source/sdk/android/telemetry_chaos_checklist.md` 可見，以便
  每一條命令都是為抄寫員敘述的。
- 讓暫存所有者鏡像每個注射器操作（`kubectl scale`，導出器
  重新啟動，負載生成器），因此 Observability 和 SRE 都確認了該步驟。
- 捕獲`scripts/telemetry/check_redaction_status.py 的輸出
  --status-url https://android-telemetry-stg/api/redaction/status` 之後
  場景並將其粘貼到事件文檔中。

**恢復**- 在清除所有噴油器之前，請勿離開橋（`inject_redaction_failure.sh --clear`，
  `kubectl scale ... --replicas=1`) 和 Grafana 儀表板顯示綠色狀態。
- 文檔/支持存檔隊列轉儲、CLI 日誌和屏幕截圖
  `docs/source/sdk/android/readiness/screenshots/<date>/` 並勾選存檔
  變更請求關閉之前的清單。
- 對於任何場景，使用 `telemetry-chaos` 標籤記錄後續票證
  失敗或產生意外的指標，並在 `status.md` 中引用它們
  在接下來的每週回顧期間。

|時間 |行動|所有者 |文物 |
|------|--------|----------|----------|
| T−30 分鐘 |驗證 `android-telemetry-stg` 運行狀況：`kubectl --context staging get pods -n android-telemetry-stg`，確認沒有掛起的升級，並記錄收集器版本。 |遙 | `docs/source/sdk/android/readiness/screenshots/<date>/cluster-health.png` |
| T−20分鐘|種子基線負載 (`scripts/telemetry/generate_android_load.sh --cluster android-telemetry-stg --duration 20m`) 並捕獲標準輸出。 |利亞姆| `readiness/labs/reports/<date>/load-generator.log` |
| T−15 分鐘 |將 `docs/source/sdk/android/readiness/incident/telemetry_chaos_template.md` 複製到 `docs/source/sdk/android/readiness/incident/<date>-telemetry-chaos.md`，列出要運行的方案 (C1–C7)，並分配抄寫員。 | Priya Deshpande（支持）|排練開始前進行的事件降價。 |
| T−10分鐘|確認Pixel+模擬器上線，安裝最新SDK，`ci/run_android_tests.sh`編譯出`PendingQueueInspector`。 |利亞姆遙 | `readiness/screenshots/<date>/device-checklist.png` |
| T−5分鐘|啟動Zoom橋，開始屏幕錄製，並在`#android-sdk-support`中宣布“混亂開始”。 | IC / 文檔/支持 |錄音保存在 `readiness/archive/<month>/` 下。 |
| +0 分鐘 |執行 `docs/source/sdk/android/readiness/labs/telemetry_lab_01.md` 中選定的場景（通常為 C2 + C6）。保持實驗室指南可見，並在發生命令調用時調出命令調用。 | Haruka 開車，Liam 反映結果 |日誌實時附加到事件文件中。 |
| +15 分鐘 |暫停收集指標 (`scripts/telemetry/check_redaction_status.py --status-url https://android-telemetry-stg/api/redaction/status`) 並抓取 Grafana 屏幕截圖。 |遙 | `readiness/screenshots/<date>/status-<scenario>.png` |
| +25 分鐘 |恢復任何注入的故障（`inject_redaction_failure.sh --clear`、`kubectl scale ... --replicas=1`）、重播隊列並確認警報關閉。 |利亞姆| `readiness/labs/reports/<date>/recovery.log` |
| +35 分鐘 |匯報：根據每個場景的通過/失敗情況更新事件文檔，列出後續行動，並將工件推送到 git。通知文檔/支持歸檔清單可以完成。 |集成電路 |事件文檔已更新，`readiness/archive/<month>/checklist.md` 已勾選。 |

- 讓分期所有者留在橋上，直到出口商健康並且所有警報均已清除。
- 將原始隊列轉儲存儲在 `docs/source/sdk/android/readiness/labs/reports/<date>/queues/` 中，並在事件日誌中引用它們的哈希值。
- 如果方案失敗，請立即創建標記為 `telemetry-chaos` 的 JIRA 票證，並將其與 `status.md` 交叉鏈接。
- 自動化助手：`ci/run_android_telemetry_chaos_prep.sh` 包裝負載生成器、狀態快照和隊列導出管道。當暫存訪問可用時設置 `ANDROID_TELEMETRY_DRY_RUN=false` 和 `ANDROID_PENDING_QUEUE_EXPORTS=pixel8=/tmp/pixel.queue,emulator=/tmp/emulator.queue`（等），以便腳本複制每個隊列文件，發出 `<label>.sha256`，並運行 `PendingQueueInspector` 以生成 `<label>.json`。僅當必須跳過 JSON 發射時（例如，沒有可用的 JDK），才使用 `ANDROID_PENDING_QUEUE_INSPECTOR=false`。 **在運行幫助程序之前始終通過設置 `ANDROID_TELEMETRY_EXPECTED_SALT_EPOCH=<YYYYQ#>` 和 `ANDROID_TELEMETRY_EXPECTED_SALT_ROTATION=<id>` 導出預期的鹽標識符**，這樣，如果捕獲的遙測數據偏離 Rust 基線，嵌入式 `check_redaction_status.py` 調用會快速失敗。

## 6. 文檔和支持- **操作員支持套件：** `docs/source/sdk/android/readiness/and7_operator_enablement.md`
  鏈接運行手冊、遙測政策、實驗室指南、存檔清單和知識
  簽入單個 AND7 就緒包。準備SRE的時候參考一下
  治理預讀或安排季度刷新。
- **啟用會議：** 2026 年 2 月 18 日進行 60 分鐘的啟用錄製
  每季度刷新一次。材料生活在下面
  `docs/source/sdk/android/readiness/`。
- **知識檢查：** 員工必須通過準備表獲得 ≥90% 的分數。商店
  結果為 `docs/source/sdk/android/readiness/forms/responses/`。
- **更新：**每當遙測模式、儀表板或覆蓋策略時
  更改，更新此 Runbook、支持 playbook 和 `status.md`
  公關。
- **每週審查：** 在每個 Rust 候選版本之後（或至少每週），驗證
  `java/iroha_android/README.md` 和此操作手冊仍然反映了當前的自動化，
  夾具輪換程序和治理期望。捕獲評論
  `status.md`，因此基金會里程碑審核可以跟踪文檔的新鮮度。

## 7. StrongBox 證明工具- **目的：** 在將設備推廣到市場之前驗證硬件支持的證明捆綁包
  StrongBox 池（AND2/AND6）。該工具使用捕獲的證書鏈並驗證它們
  使用與生產代碼執行相同的策略來對抗受信任的根。
- **參考：** 請參閱 `docs/source/sdk/android/strongbox_attestation_harness_plan.md` 了解完整信息
  捕獲 API、別名生命週期、CI/Buildkite 連接和所有權矩陣。將該計劃視為
  新實驗室技術人員入職或更新財務/合規工件時的真相來源。
- **工作流程：**
  1. 在設備上收集證明捆綁包（別名 `challenge.hex` 和 `chain.pem`，其中
     leaf→root 命令）並將其複製到工作站。
  2. 運行 `scripts/android_keystore_attestation.sh --bundle-dir  --trust-root 
     [--trust-root-dir ] --require-strongbox --output ` 使用適當的
     Google/Samsung root（目錄允許您加載整個供應商捆綁包）。
  3. 將 JSON 摘要與原始證明材料一起存檔在
     `artifacts/android/attestation/<device-tag>/`。
- **捆綁包格式：** 遵循 `docs/source/sdk/android/readiness/android_strongbox_attestation_bundle.md`
  所需的文件佈局（`chain.pem`、`challenge.hex`、`alias.txt`、`result.json`）。
- **可信根：** 從設備實驗室機密存儲中獲取供應商提供的 PEM；通過多個
  `--trust-root` 參數或將 `--trust-root-dir` 指向保存錨點的目錄
  該鏈以非 Google 錨點終止。
- **CI 工具：** 使用 `scripts/android_strongbox_attestation_ci.sh` 批量驗證存檔的包
  在實驗室機器或 CI 運行器上。該腳本掃描 `artifacts/android/attestation/**` 並調用
  包含記錄文件的每個目錄的線束，寫入刷新的 `result.json`
  總結到位。
- **CI 通道：** 同步新包後，運行中定義的 Buildkite 步驟
  `.buildkite/android-strongbox-attestation.yml` (`buildkite-agent pipeline upload --pipeline .buildkite/android-strongbox-attestation.yml`)。
  該作業執行 `scripts/android_strongbox_attestation_ci.sh`，生成摘要
  `scripts/android_strongbox_attestation_report.py`，將報告上傳到`artifacts/android_strongbox_attestation_report.txt`，
  並將構建註釋為 `android-strongbox/report`。立即調查任何故障並
  鏈接設備矩陣中的構建 URL。
- **報告：** 將 JSON 輸出附加到治理審查並更新中的設備矩陣條目
  `docs/source/sdk/android/readiness/android_strongbox_device_matrix.md` 以及認證日期。
- **模擬演練：** 當硬件不可用時，運行 `scripts/android_generate_mock_attestation_bundles.sh`
  （使用 `scripts/android_mock_attestation_der.py`）創建確定性測試包以及共享模擬根，以便 CI 和文檔可以端到端地運用該工具。
- **代碼內護欄：** `ci/run_android_tests.sh --tests
  org.hyperledger.iroha.android.crypto.keystore.KeystoreKeyProviderTests` 涵蓋空與挑戰
  證明再生（StrongBox/TEE 元數據）並發出 `android.keystore.attestation.failure`
  挑戰不匹配，因此在發送新包之前捕獲緩存/遙測回歸。

## 8. 聯繫人

- **支持工程待命：** `#android-sdk-support`
- **SRE 治理：** `#sre-governance`
- **文檔/支持：** `#docs-support`
- **升級樹：** 請參閱 Android 支持手冊 §2.1

## 9. 故障排除場景路線圖項目 AND7-P2 列出了重複尋呼的三個事件類別
Android 待命：Torii/網絡超時、StrongBox 認證失敗以及
`iroha_config` 明顯漂移。提交前仔細核對相關清單
Sev1/2 後續行動並將證據存檔於 `incident/<date>-android-*.md` 中。

### 9.1 Torii 和網絡超時

**信號**

- `android_sdk_submission_latency`、`android_sdk_pending_queue_depth` 上的警報，
  `android_sdk_offline_replay_errors` 和 Torii `/v2/pipeline` 錯誤率。
- `operator-console` 小部件（示例/android）顯示停滯的隊列耗盡或
  重試陷入指數退避。

**立即響應**

1. 確認 PagerDuty (`android-networking`) 並啟動事件日誌。
2. 捕獲 Grafana 快照（提交延遲 + 隊列深度），涵蓋
   最後30分鐘。
3. 記錄設備日誌中的活動 `ClientConfig` 哈希 (`ConfigWatcher`
   每當重新加載成功或失敗時都會打印清單摘要）。

**診斷**

- **隊列運行狀況：** 從臨時設備或
  模擬器（`adb shell run-as  cat files/pending.queue >
  /tmp/pending.queue`）。解碼信封
  `OfflineSigningEnvelopeCodec` 如中所述
  `docs/source/sdk/android/offline_signing.md#4-queueing--replay` 確認
  積壓符合運營商的預期。將解碼後的哈希值附加到
  事件。
- **哈希庫存：** 下載隊列文件後，運行檢查器助手
  捕獲事件工件的規范哈希值/別名：

  ```bash
  gradle -p java/iroha_android :core:classes >/dev/null  # compiles classes if needed
  java -cp build/classes org.hyperledger.iroha.android.tools.PendingQueueInspector \
    --file /tmp/pending.queue --json > queue-inspector.json
  ```

  將 `queue-inspector.json` 和打印精美的標準輸出附加到事件中
  並將其鏈接到場景 D 的 AND7 實驗室報告。
- **Torii 連接：** 在本地運行 HTTP 傳輸工具以排除 SDK
  回歸：`ci/run_android_tests.sh` 練習
  `HttpClientTransportTests`、`HttpClientTransportHarnessTests` 和
  `ToriiMockServerTests`。這裡的失敗表明客戶端錯誤而不是
  Torii 中斷。
- **故障注入演練：** 在暫存 Pixel (StrongBox) 和 AOSP 上
  模擬器，切換連接以重現掛起隊列的增長：
  `adb shell cmd connectivity airplane-mode enable` → 提交兩個演示
  通過操作員控制台進行交易 → `adb shell cmd 連接飛行模式
  禁用` → verify the queue drains and `android_sdk_offline_replay_errors`
  保持為 0。記錄重放交易的哈希值。
- **警報奇偶校驗：** 當調整閾值時或 Torii 更改後，執行
  `scripts/telemetry/test_torii_norito_rpc_alerts.sh` 因此 Prometheus 規則保留
  與儀表闆對齊。

**恢復**

1. 如果 Torii 降級，請接聽 Torii 並繼續重放
   一旦 `/v2/pipeline` 接受流量就排隊。
2. 僅通過簽名的 `iroha_config` 清單重新配置受影響的客戶端。的
   `ClientConfig` 熱重載觀察程序必須在事件發生之前發出成功日誌
   可以關閉。
3. 使用重播之前/之後的隊列大小以及以下的哈希值來更新事件
   任何被丟棄的交易。

### 9.2 StrongBox 和證明失敗

**信號**- `android_sdk_strongbox_success_rate` 上的警報或
  `android.keystore.attestation.failure`。
- `android.keystore.keygen` 遙測現在記錄所請求的
  `KeySecurityPreference` 和使用的路由（`strongbox`、`hardware`、
  當 StrongBox 首選項登陸時，`software`）帶有 `fallback=true` 標誌
  TEE/軟件。 STRONGBOX_REQUIRED 請求現在會快速失敗，而不是靜默失敗
  返回 TEE 密鑰。
- 支持引用 `KeySecurityPreference.STRONGBOX_ONLY` 設備的票證
  回到軟件鍵。

**立即響應**

1. 確認 PagerDuty (`android-crypto`) 並捕獲受影響的別名標籤
   （加鹽哈希）加上設備配置文件存儲桶。
2. 檢查設備的證明矩陣條目
   `docs/source/sdk/android/readiness/android_strongbox_device_matrix.md` 和
   記錄最後一次驗證的日期。

**診斷**

- **捆綁包驗證：** 運行
  `scripts/android_keystore_attestation.sh --bundle-dir <bundle> --trust-root <root.pem>`
  在存檔的證明上確認故障是否是由於設備造成的
  配置錯誤或策略更改。附上生成的 `result.json`。
- **挑戰重新生成：** 挑戰不會被緩存。每個挑戰請求都會重新生成一個新的
  `(alias, challenge)` 的證明和緩存；無挑戰的調用重用緩存。不支持
- **CI 掃描：** 執行 `scripts/android_strongbox_attestation_ci.sh`，因此每隔
  存儲的包被重新驗證；這可以防止引入系統性問題
  通過新的信任錨。
- **設備練習：** 在沒有 StrongBox 的硬件上（或通過強制使用模擬器），
  將 SDK 設置為僅需要 StrongBox，提交演示交易並確認
  遙測導出器發出 `android.keystore.attestation.failure` 事件
  與預期的原因。在支持 StrongBox 的 Pixel 上重複此操作，以確保
  幸福之路常綠。
- **SDK回歸檢查：**運行`ci/run_android_tests.sh`並支付
  注意以證明為中心的套件（`AndroidKeystoreBackendDetectionTests`，
  `AttestationVerifierTests`, `IrohaKeyManagerDeterministicExportTests`,
  `KeystoreKeyProviderTests` 用於緩存/挑戰分離）。失敗在這裡
  表明客戶端回歸。

**恢復**

1. 如果供應商輪換了證書或者如果
   設備最近收到了重大 OTA。
2. 將刷新後的捆綁包上傳到 `artifacts/android/attestation/<device>/` 並
   使用新日期更新矩陣條目。
3. 如果 StrongBox 在生產中不可用，請按照以下中的覆蓋工作流程操作
   第 3 節並記錄回退持續時間；長期緩解需要
   設備更換或供應商修復。

### 9.2a 確定性出口恢復

- **格式：** 當前導出為 v3（每次導出 salt/nonce + Argon2id，記錄為
- **密碼短語策略：** v3 強制執行 ≥12 個字符的密碼短語。如果用戶供應較短
  密碼短語，指示他們使用合規的密碼短語重新導出； v0/v1 導入是
  豁免，但應在導入後立即重新包裝為 v3。
- **篡改/重用防護：** 解碼器拒絕零/短鹽或隨機數長度和重複
  鹽/隨機數對錶面顯示為 `salt/nonce reuse` 錯誤。重新生成導出以清除
  警衛；不要試圖強制重複使用。
  `SoftwareKeyProvider.importDeterministic(...)` 補充密鑰，然後
  `exportDeterministic(...)` 發出 v3 捆綁包，以便桌面工具記錄新的 KDF
  參數。### 9.3 清單和配置不匹配

**信號**

- `ClientConfig` 重新加載失敗、Torii 主機名或遙測不匹配
  由 AND7 diff 工具標記的架構差異。
- 操作員報告同一設備中的不同重試/退避旋鈕
  艦隊。

**立即響應**

1. 捕獲 Android 日誌中打印的 `ClientConfig` 摘要和
   發布清單中的預期摘要。
2. 轉儲運行節點配置以進行比較：
   `iroha_cli config show --actual > /tmp/iroha_config.actual.json`。

**診斷**

- **架構差異：** 運行 `scripts/telemetry/run_schema_diff.sh --android-config
   --rust-config  --textfile-dir /var/lib/node_exporter/textfile_collector`
  要生成 Norito diff 報告，請刷新 Prometheus 文本文件，並附加
  JSON 工件以及事件的指標證據和 AND7 遙測準備日誌。
- **清單驗證：** 使用 `iroha_cli runtime capabilities` （或運行時
  審計命令）來檢索節點的公佈的加密/ABI 哈希值並確保
  它們與移動清單相匹配。不匹配確認節點已回滾
  無需重新發布 Android 清單。
- **SDK回歸檢查：** `ci/run_android_tests.sh` 涵蓋
  `ClientConfigNoritoRpcTests`、`ClientConfig.ValidationTests` 和
  `HttpClientTransportStatusTests`。失敗表明附帶的 SDK 無法
  解析當前部署的清單格式。

**恢復**

1.通過授權管道重新生成清單（通常是
   `iroha_cli runtime Capabilities` → 簽名的 Norito 清單 → 配置包）和
   通過運營商渠道重新部署。切勿編輯 `ClientConfig`
   覆蓋設備上的。
2. 更正後的艙單登陸後，請注意 `ConfigWatcher`“重新加載正常”
   每個艦隊層上的消息並僅在遙測後關閉事件
   模式差異報告奇偶性。
3. 將清單哈希、架構差異工件路徑和事件鏈接記錄在
   Android 部分下的 `status.md` 用於審核。

## 10. 操作員支持課程

路線圖項目 **AND7** 需要可重複的培訓包，以便操作員，
支持工程師，SRE 可以採用遙測/編輯更新，而無需
猜測。將此部分與
`docs/source/sdk/android/readiness/and7_operator_enablement.md`，其中包含
詳細的清單和工件鏈接。

### 10.1 會議模塊（60 分鐘簡報）

1. **遙測架構（15分鐘）。 ** 遍歷導出器緩衝區，
   編輯過濾器和模式差異工具。演示
   `scripts/telemetry/run_schema_diff.sh --textfile-dir /var/lib/node_exporter/textfile_collector` 加
   `scripts/telemetry/check_redaction_status.py` 讓與會者了解平價如何
   強制執行。
2. **運行手冊 + 混沌實驗室（20 分鐘）。 ** 突出顯示本運行手冊的第 2-9 節，
   演練 `readiness/labs/telemetry_lab_01.md` 中的一個場景，並展示如何
   將文物歸檔到 `readiness/labs/reports/<stamp>/` 下。
3. **覆蓋 + 合規工作流程（10 分鐘）。 ** 查看第 3 節覆蓋，
   演示 `scripts/android_override_tool.sh`（應用/撤銷/摘要），以及
   更新 `docs/source/sdk/android/telemetry_override_log.md` 加上最新的
   摘要 JSON。
4. **問答/知識檢查（15 分鐘）。 ** 使用快速參考卡
   `readiness/cards/telemetry_redaction_qrc.md` 錨定問題，然後
   在 `readiness/and7_operator_enablement.md` 中捕穫後續內容。### 10.2 資產節奏和所有者

|資產|節奏|所有者 |存檔位置 |
|--------|---------|----------|--------------------|
|錄製的演練（縮放/團隊）|每季度或每次鹽輪換前 | Android Observability TL + 文檔/支持管理器 | `docs/source/sdk/android/readiness/archive/<YYYY-MM>/`（錄音+清單）|
|幻燈片和快速參考卡|每當政策/操作手冊發生變化時進行更新 |文檔/支持經理 | `docs/source/sdk/android/readiness/deck/` 和 `/cards/`（導出 PDF + Markdown）|
|知識檢查+考勤表|每次直播結束後 |支持工程| `docs/source/sdk/android/readiness/forms/responses/` 和 `and7_operator_enablement.md` 考勤塊 |
|問答積壓/行動日誌 |滾動；每次會議後更新|法學碩士（代理 DRI）| `docs/source/sdk/android/readiness/and7_operator_enablement.md` §6 |

### 10.3 證據和反饋循環

- 將會話工件（屏幕截圖、事件演習、測驗導出）存儲在
  用於混亂排練的相同日期目錄，以便治理可以審計兩者
  準備情況一起跟踪。
- 會話完成後，更新 `status.md`（Android 部分），其中包含以下鏈接：
  存檔目錄並記下任何打開的後續內容。
- 現場問答中的未決問題必須轉化為問題或文檔
  一周內拉取請求；參考路線圖史詩（AND7/AND8）
  票證說明，以便業主保持一致。
- SRE 同步檢查存檔清單以及列出的架構差異工件
  第 2.3 節在宣布本季度課程結束之前。