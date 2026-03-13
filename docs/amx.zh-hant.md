---
lang: zh-hant
direction: ltr
source: docs/amx.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f11f0a83efc46035aeeaf4c1ad626a2a773303e9dfab188704016cf483a78ce6
source_last_modified: "2026-01-23T08:31:38.611123+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# AMX 執行和操作指南

**狀態：** 草案（NX-17）  
**受眾：** 核心協議、AMX/共識工程師、SRE/Telemetry、SDK & Torii 團隊  
**上下文：** 完成路線圖項目“文檔（所有者：Docs）——使用時序圖、錯誤目錄、操作員期望以及生成/使用 PVO 的開發人員指南更新 `docs/amx.md`。”【roadmap.md:2497】

## 總結

原子跨數據空間事務 (AMX) 允許單次提交觸及多個數據空間 (DS)，同時保留 1s 時隙最終性、確定性故障代碼和私有 DS 片段的機密性。本指南捕獲了時序模型、規範錯誤處理、操作員證據要求以及開發人員對證明驗證對象 (PVO) 的期望，因此路線圖交付成果在 Nexus 設計文件 (`docs/source/nexus.md`) 之外保持獨立。

主要保證：

- 每個 AMX 提交都會收到確定性的準備/提交預算；使用記錄的代碼而不是懸掛通道溢出中止。
- 未達到預算的 DA 樣本將被記錄為缺少可用性證據，並且事務將繼續排隊等待下一個時隙，而不是默默地停止吞吐量。
- 證明驗證對象 (PVO) 通過讓客戶端/批處理程序預先註冊 Nexus 主機在時隙中快速驗證的工件，將繁重的證明與 1s 時隙解耦。
- IVM 主機從空間目錄派生每個數據空間 AXT 策略：句柄必須以目錄中通告的通道為目標，提供最新的清單根，滿足 expiry_slot、handle_era 和 sub_nonce 最小值，並在執行前使用 `PermissionDenied` 拒絕未知數據空間。
- 時隙到期使用 `nexus.axt.slot_length_ms`（默認 `1`ms，在 `1`ms 和 `600_000`ms 之間驗證）加上有界的 `nexus.axt.max_clock_skew_ms`（默認 `0`ms，由時隙長度和`60_000`ms）。主機計算 `current_slot = block.creation_time_ms / slot_length_ms`，應用偏差限額來證明和處理過期檢查，並拒絕通告偏差大於配置限制的句柄。
- 證明緩存 TTL 邊界重用：`nexus.axt.proof_cache_ttl_slots`（默認 `1`，經過驗證的 `1`–`64`）限制接受或拒絕的證明在主機緩存中保留的時間；一旦 TTL 窗口或證明的 `expiry_slot` 過去，條目就會丟失，因此重播保護保持有限。
- 重放賬本保留：`nexus.axt.replay_retention_slots`（默認 `128`，經過驗證的 `1`–`4_096`）設置保留的句柄使用歷史記錄的最小槽窗口，以便跨對等/重新啟動拒絕重放；將其與您期望操作員發出的最長句柄有效性窗口對齊。賬本持久保存在 WSV 中，在啟動時進行水合，並在保留窗口和句柄到期（以較晚者為準）後確定性地進行修剪，因此對等交換機不會重新打開重播間隙。
- 調試緩存狀態：Torii 公開 `/v2/debug/axt/cache`（遙測/開發人員門）以返回當前 AXT 策略快照版本、最近的拒絕（通道/原因/版本）、緩存的證明（數據空間/狀態/清單根/插槽）和拒絕提示（`next_min_handle_era`/`next_min_sub_nonce`）。使用此端點來確認插槽/清單輪換是否反映在緩存狀態中，並在故障排除期間確定性地刷新句柄。

## 時隙時序模型

### 時間軸

```text
t=0ms           70ms             300ms              600ms       840ms    1000ms
│─────────┬───────────────┬───────────────────┬──────────────┬──────────┬────────│
│         │               │                   │              │          │        │
│  Mempool│Proof build + DA│Consensus PREP/COM │ IVM/AMX exec │Settlement│ Guard  │
│  ingest │sample (≤300ms) │(≤300ms)           │(≤250ms)      │(≤40ms)   │(≤40ms) │
```

- 預算與全球賬本計劃一致：mempool 70ms，DA commit ≤300ms，共識 300ms，IVM/AMX 250ms，結算 40ms，guard 40ms。 【roadmap.md:2529】
- 違反 DA 窗口的交易將被記錄為缺少可用性證據，並在下一個時隙中重試；所有其他違規表面代碼，例如 `AMX_TIMEOUT` 或 `SETTLEMENT_ROUTER_UNAVAILABLE`。
- 保護切片吸收遙測導出和最終審核，因此即使導出器短暫滯後，插槽仍會在 1 秒處關閉。
- 配置提示：默認保持嚴格過期（`slot_length_ms = 1`、`max_clock_skew_ms = 0`）。對於 1s 節奏集 `slot_length_ms = 1_000` 和 `max_clock_skew_ms = 250`；對於 2 秒的節奏，請使用 `slot_length_ms = 2_000` 和 `max_clock_skew_ms = 500`。驗證窗口之外的值（`1`–`600_000`ms 或 `max_clock_skew_ms` 大於槽長度/`60_000`ms）將在配置解析時被拒絕，並且通告的句柄偏差必須保持在配置的範圍內。

###跨DS泳道

```text
Client        DS A (public)        DS B (private)        Nexus Lane        Settlement
  │ submit tx │                     │                     │                 │
  │──────────▶│ prepare fragment    │                     │                 │
  │           │ proof + DA part     │ prepare fragment    │                 │
  │           │───────────────┬────▶│ proof + DA part     │                 │
  │           │               │     │─────────────┬──────▶│ Merge proofs    │
  │           │               │     │             │       │ verify PVO/DA   │
  │           │               │     │             │       │────────┬────────▶ apply
  │◀──────────│ result + code │◀────│ result + code │◀────│ outcome│          receipt
```

每個 DS 片段必須在通道組裝插槽之前完成其 30ms 準備窗口。丟失的證明會保留在內存池中的下一個槽位，而不是阻塞對等點。

### 儀器清單

|公制/跡線|來源 | SLO / 警報 |筆記|
|----------------|--------|-------------|--------|
| `iroha_slot_duration_ms`（直方圖）/`iroha_slot_duration_ms_latest`（儀表）| `iroha_telemetry` | p95 ≤ 1000 毫秒 | Ci 門在 `ans3.md` 中描述。 |
| `iroha_da_quorum_ratio` | `iroha_telemetry`（提交掛鉤）|每 30 分鐘窗口 ≥0.95 |源自缺失可用性遙測，因此每個塊都會更新儀表（`crates/iroha_core/src/telemetry.rs:3524`，`crates/iroha_core/src/telemetry.rs:4558`）。 |
| `iroha_amx_prepare_ms` | IVM 主機 |每個 DS 範圍 p95 ≤ 30ms |驅動器 `AMX_TIMEOUT` 中止。 |
| `iroha_amx_commit_ms` | IVM 主機 |每個 DS 範圍 p95 ≤ 40ms |涵蓋增量合併+觸發器執行。 |
| `iroha_ivm_exec_ms` | IVM 主機 |如果每通道 >250 毫秒，則發出警報 |鏡像 IVM 覆蓋塊執行窗口。 |
| `iroha_amx_abort_total{stage}` |執行人|如果 >0.05 中止/槽或持續單級尖峰，則發出警報 |階段標籤：`prepare`、`exec`、`commit`。 |
| `iroha_amx_lock_conflicts_total` | AMX 調度程序 |如果每個槽衝突 >0.1，則發出警報 |表示 R/W 設置不准確。 |
| `iroha_axt_policy_reject_total{lane,reason}` | IVM 主機 |留意尖峰 |區分manifest/lane/era/sub_nonce/expiry 拒絕。 |
| `iroha_axt_policy_snapshot_cache_events_total{event}` | IVM 主機 |預計cache_miss僅在啟動/清單更改時出現持續的失誤表明政策的水分已經過時。 |
| `iroha_axt_proof_cache_events_total{event}` | IVM 主機 |主要期待 `hit`/`miss` | `reject`/`expired` 峰值通常表示明顯的漂移或過時的證明。 |
| `iroha_axt_proof_cache_state{dsid,status,manifest_root_hex,verified_slot}` | IVM 主機 |檢查緩存的證明 |緩存證明的計量值是 expiry_slot （應用了傾斜）。 |
|缺少可用性證據 (`sumeragi_da_gate_block_total{reason="missing_local_data"}`) |車道遙測|如果每個 DS 的交易量 >5%，則發出警報 |意味著證明者或證明是滯後的。 |

`/v2/debug/axt/cache` 鏡像 `iroha_axt_proof_cache_state` 儀表，並為操作員提供每個數據空間快照（狀態、清單根、已驗證/到期槽）。

`iroha_amx_commit_ms` 和 `iroha_ivm_exec_ms` 共享相同的延遲桶
`iroha_amx_prepare_ms`。中止計數器用通道 ID 標記每次拒絕
和階段（`prepare` = 覆蓋構建/驗證，`exec` = IVM 塊執行，
`commit` = 增量合併 + 觸發重播），因此遙測可以突出顯示是否
爭用來自讀/寫不匹配或狀態後合併。

運營商必須將這些指標與插槽接受證據一起存檔以供審核，並在 `status.md` 中記錄回歸。

### AXT 黃金賽程

Norito 描述符/句柄/策略快照的固定裝置位於 `crates/iroha_data_model/tests/fixtures/axt_golden.rs`，並在 `crates/iroha_data_model/tests/axt_policy_vectors.rs` (`print_golden_vectors`) 中提供再生幫助程序。 CoreHost 使用 `core_host_enforces_fixture_snapshot_fields` (`crates/ivm/tests/core_host_policy.rs`) 中的相同裝置來執行通道綁定、清單根匹配、expiry_slot 新鮮度、handle_era/sub_nonce 最小值和缺失數據空間拒絕。
- 多數據空間 JSON 固定裝置 (`crates/iroha_data_model/tests/fixtures/axt_descriptor_multi_ds.json`) 固定描述符/觸摸模式、規範 Norito 字節和 Poseidon 綁定 (`compute_descriptor_binding`)。 `axt_descriptor_fixture` 測試保護編碼字節，SDK 可以使用 `AxtDescriptorBuilder::builder` 和 `TouchManifest::from_read_write` 為文檔/SDK 組裝確定性示例。

### Lane 目錄映射和清單

- AXT 策略快照是根據空間目錄清單集和通道目錄構建的。每個數據空間都映射到其配置的通道；活動清單提供清單哈希、激活紀元 (`min_handle_era`) 和子隨機數下限。沒有活動清單的 UAID 綁定仍會發出具有歸零清單根的策略條目，因此通道選通保持活動狀態，直到真正的清單落地。
- 快照中的 `current_slot` 源自最新提交的塊時間戳 (`creation_time_ms / slot_length_ms`)，僅在提交的標頭可用之前回落到塊高度。
- 遙測將水合快照顯示為 `iroha_axt_policy_snapshot_version`（Norito 編碼快照哈希的低 64 位），並通過 `iroha_axt_policy_snapshot_cache_events_total{event=cache_hit|cache_miss}` 緩存事件。拒絕計數器使用標籤 `lane`、`manifest`、`era`、`sub_nonce` 和 `expiry`，因此操作員可以立即查看哪個字段阻止了句柄。

### 跨數據空間可組合性清單- 確認空間目錄中列出的每個數據空間都有通道條目和活動清單；旋轉應該在發出新句柄之前刷新綁定和清單根。歸零根意味著句柄將保持被拒絕狀態，直到清單出現為止，並且主機/塊驗證現在拒絕呈現歸零清單根的句柄。
- 啟動時和空間目錄更改後，策略快照指標上預計會出現一個 `cache_miss`，隨後是穩定的 `cache_hit` 事件；持續的錯過率表明清單提要已過時或丟失。
- 當句柄被拒絕時，查看 `iroha_axt_policy_reject_total{lane,reason}` 和快照版本，以決定是否請求刷新句柄 (`expiry`/`era`/`sub_nonce`) 或修復通道/清單綁定（`lane`/`manifest`）。 Torii 調試端點 `/v2/debug/axt/cache` 還返回 `reject_hints` 以及 `dataspace`、`target_lane`、`next_min_handle_era` 和 `next_min_sub_nonce`，以便操作員可以在策略碰撞後確定性地刷新句柄。

### SDK示例：無令牌出口的遠程支出

1. 構建一個 AXT 描述符，列出擁有資產的數據空間以及本地所需的任何讀/寫操作；保持描述符的確定性，以便綁定哈希保持穩定。
2. 調用 `AXT_TOUCH` 獲取具有您期望的清單視圖的遠程數據空間；如果主機需要，可以選擇通過 `AXT_VERIFY_DS_PROOF` 附加證明。
3. 請求或刷新資產句柄，並使用在遠程數據空間內使用的 `RemoteSpendIntent` 調用 `AXT_USE_ASSET_HANDLE`（無橋接腿）。預算執行針對上述快照使用句柄的 `remaining`、`per_use`、`sub_nonce`、`handle_era` 和 `expiry_slot`。
4.通過`AXT_COMMIT`提交；如果主機返回 `PermissionDenied`，則使用拒絕標籤來決定是否獲取新句柄（expiry/sub_nonce/era）或修復清單/通道綁定。

## 操作員期望

1. **時段前準備**
   - 確保每個配置文件的 DA 證明者池（A=12、B=9、C=7）健康；證明者流失記錄在該槽的空間目錄快照中。
   - 在啟用新的工作負載組合之前，驗證 `iroha_amx_prepare_ms` 是否低於代表性運行者的預算。

2. **槽內監控**
   - 對缺失可用性峰值（兩個連續時段> 5%）和 `AMX_TIMEOUT` 發出警報，因為兩者都表明錯過了預算。
   - 跟踪 PVO 緩存利用率（`iroha_pvo_cache_hit_ratio`，由證明服務導出）以證明偏離路徑驗證與提交保持同步。

3. **證據捕獲**
   - 將 DA 收據集、AMX 準備直方圖和 PVO 緩存報告附加到 `status.md` 引用的夜間工件包。
   - 每當 DA 抖動、oracle 停止或緩衝區耗盡測試運行時，在 `ops/drill-log.md` 中記錄混沌鑽取輸出。

4. **運行手冊維護**
   - 每當 AMX 錯誤代碼或覆蓋發生變化時，更新 Android/Swift SDK Runbook，以便客戶端團隊繼承確定性故障語義。
   - 使配置片段（例如，`iroha_config.amx.*`）與 `docs/source/nexus.md` 中的規範參數保持同步。

## 遙測和故障排除

### 遙測快速參考

|來源 |捕捉什麼 |命令/路徑|證據預期|
|--------|-----------------|----------------|------------------------|
| Prometheus (`iroha_telemetry`) |插槽和 AMX SLO：`iroha_slot_duration_ms`、`iroha_amx_prepare_ms`、`iroha_amx_commit_ms`、`iroha_da_quorum_ratio`、`iroha_amx_abort_total{stage}` |抓取 `https://$TORII/telemetry/metrics` 或從 `docs/source/telemetry.md` 中描述的儀表板導出。 |將直方圖快照（以及觸發的警報歷史記錄）附加到夜間 `status.md` 註釋中，以便審核員可以查看 p95/p99 值和警報狀態。 |
| Torii RBC 快照 | DA/RBC 積壓：每個會話塊積壓、視圖/高度元數據和 DA 可用性計數器（`sumeragi_da_gate_block_total{reason="missing_local_data"}`；`sumeragi_rbc_da_reschedule_total` 是舊版）。 | `GET /v2/sumeragi/rbc` 和 `GET /v2/sumeragi/rbc/sessions`（有關示例，請參閱 `docs/source/samples/sumeragi_rbc_status.md`）。 |每當 AMX DA 警報觸發時，存儲 JSON 響應（帶有時間戳）；將它們包含在事件包中，以便審核人員可以確認背壓與遙測相匹配。 |
|證明服務指標| PVO 緩存運行狀況：`iroha_pvo_cache_hit_ratio`、緩存填充/逐出計數器、證明隊列深度 | `GET /metrics` 在證明服務 (`IROHA_PVO_METRICS_URL`) 上或通過共享 OTLP 收集器。 |導出緩存命中率和隊列深度以及 AMX 插槽指標，以便路線圖 OA/PVO 門具有確定性的人工製品。 |
|驗收線束|受控抖動下的端到端混合負載（時隙/DA/RBC/PVO）|重新運行 `ci/acceptance/slot_1s.yml`（或 CI 中的相同作業）並將日誌包 + 生成的工件存檔在 `artifacts/acceptance/slot_1s/<timestamp>/` 中。 |在 GA 之前以及起搏器/DA 設置更改時需要；在操作員移交數據包中包含 YAML 運行摘要以及 Prometheus 快照。 |

### 故障排除手冊

|症狀|先檢查|建議補救措施|
|--------|-------------|--------------------------|
| `iroha_slot_duration_ms` p95 爬行超過 1000ms |從 `/telemetry/metrics` 導出 Prometheus 加上最新的 `/v2/sumeragi/rbc` 快照以確認 DA 延期；與最後一個 `ci/acceptance/slot_1s.yml` 工件進行比較。 |降低 AMX 批量大小或啟用額外的 RBC 收集器 (`sumeragi.collectors.k`)，然後重新運行驗收工具並捕獲新的遙測證據。 |
|缺少可用性峰值 | `/v2/sumeragi/rbc/sessions` 積壓字段（`lane_backlog`、`dataspace_backlog`）以及證明者運行狀況儀表板。 |移除不健康的證明者，暫時增加`redundant_send_r`以加快交付速度，並在`status.md`中發布修復說明。積壓工作清除後，附上更新的 RBC 快照。 |
|收據中頻繁出現 `PVO_MISSING_OR_EXPIRED` |證明服務緩存指標 + 發行者的 PVO 調度程序日誌。 |重新生成過時的 PVO 工件，縮短旋轉節奏，並確保每個 SDK 在 `expiry_slot` 之前刷新手柄。在證據包中包含證明服務指標以證明緩存已恢復。 |
|重複 `AMX_LOCK_CONFLICT` 或 `AMX_TIMEOUT` | `iroha_amx_lock_conflicts_total`、`iroha_amx_prepare_ms` 以及受影響的交易清單。 |重新運行 Norito 靜態分析器，更正讀/寫選擇器（或拆分批次），並發布更新的清單固定裝置，以便衝突計數器返回到基線。 |
| `SETTLEMENT_ROUTER_UNAVAILABLE` 警報 |結算路由器日誌 (`docs/settlement-router.md`)、金庫緩衝區儀表板和受影響的收據。 |充值 XOR 緩衝區或將通道翻轉為僅 XOR 模式，記錄財務操作，並重新運行時隙驗收測試以證明結算已恢復。 |

### AXT 拒絕信號

- 原因代碼捕獲為 `AxtRejectReason`（`lane`、`manifest`、`era`、`sub_nonce`、`expiry`、`missing_policy`、 `policy_denied`、`proof`、`budget`、`replay_cache`、`descriptor`、`duplicate`）。塊驗證現在顯示 `AxtEnvelopeValidationFailed { message, reason, snapshot_version }`，因此事件可以將拒絕固定到特定的策略快照。
- `/v2/debug/axt/cache` 返回 `{ policy_snapshot_version, last_reject, cache, hints }`，其中 `last_reject` 攜帶最近主機拒絕的通道/原因/版本，`hints` 提供 `next_min_handle_era`/`next_min_sub_nonce` 刷新指導以及緩存的證明狀態。
- 警報模板：當 `iroha_axt_policy_reject_total{reason="manifest"}` 或 `{reason="expiry"}` 在 5 分鐘窗口內出現峰值時出現頁面，將 `last_reject` 快照 + `policy_snapshot_version` 從 Torii 調試端點附加到事件，並使用提示有效負載在重試之前請求刷新句柄。

## 證明驗證對象 (PVO)

### 結構

PVO 是 Norito 編碼的信封，可讓客戶提前證明繁重的工作。規範字段是：

|領域|描述 |
|--------|-------------|
| `circuit_id` |證明系統/聲明的靜態標識符（例如，`amx.transfer.v1`）。 |
| `vk_hash` | DS 清單引用的驗證密鑰的 Blake2b-256 哈希值。 |
| `proof_digest` |存儲在時隙外 PVO 註冊表中的序列化證明有效負載的波塞冬摘要。 |
| `max_k` | AIR 域的上限；主機拒絕超過廣告大小的證明。 |
| `expiry_slot` |槽位高度，超過該高度後工件無效；將過時的證明排除在車道之外。 |
| `profile` |可選提示（例如 DS 配置文件 A/B/C）可幫助調度程序批量共享配置文件的校樣。 |

Norito 架構位於 `crates/iroha_data_model/src/nexus` 中的數據模型定義旁邊，因此 SDK 無需 serde 即可派生它。

### 生成管道1. **編譯電路元數據** — 從證明者構建中導出 `circuit_id`、驗證密鑰和最大跡線大小（通常通過 `fastpq_prover` 報告）。
2. **生成證明工件** — 運行時隙外證明器並存儲完整的成績單和承諾。
3. **通過證明服務註冊** — 將 Norito PVO 有效負載提交到時隙外驗證器（請參閱路線圖 NX-17 證明管道）。該服務驗證一次，固定摘要，並通過 Torii 公開句柄。
4. **交易中的參考** — 將 PVO 句柄附加到 AMX 構建器（`amx_touch` 或更高級別的 SDK 幫助程序）。主機查找摘要，驗證緩存的結果，並且僅在緩存變冷時才在槽內重新計算。
5. **到期時輪換** — SDK 必須在 `expiry_slot` 之前刷新任何緩存的句柄。過期對象會觸發 `PVO_MISSING_OR_EXPIRED`。

### 開發人員清單

- 準確聲明讀/寫集，以便 AMX 可以預取鎖並避免 `AMX_LOCK_CONFLICT`。
- 當跨 DS 傳輸觸及受監管的 DS 時，將確定性津貼證明捆綁在同一 UAID 清單更新中。
- 重試策略：缺少可用性證據→不採取任何行動（交易保留在內存池中）； `AMX_TIMEOUT` 或 `PVO_MISSING_OR_EXPIRED` → 重建工件並以指數方式回退。
- 測試應包括緩存命中和冷啟動（強制主機使用相同的 `max_k` 驗證證明）以防止確定性回歸。
- 證明 blob (`ProofBlob`) 必須編碼 `AxtProofEnvelope { dsid, manifest_root, da_commitment?, proof }`；主機將證明綁定到空間目錄清單根，並使用 `iroha_axt_proof_cache_events_total{event="hit|miss|expired|reject|cleared"}` 緩存每個數據空間/槽的通過/失敗結果。過期或清單不匹配的工件在提交之前會被拒絕，並在緩存的 `reject` 上的同一插槽短路中進行後續重試。
- 證明緩存重用是槽範圍的：經過驗證的證明在同一槽內的信封上保持熱狀態，並在槽前進時自動逐出，因此重試保持確定性。

### 靜態讀/寫分析器

編譯時選擇器必須匹配合約的實際行為，然後 AMX 才能
預取鎖或應用 UAID 清單。新的`ivm::analysis`模塊
(`crates/ivm/src/analysis.rs`) 公開 `analyze_program(&[u8])`，它解碼
`.to` 人工製品，記錄寄存器讀/寫、內存操作和系統調用使用情況，
並生成 SDK 清單可以嵌入的 JSON 友好的報告。運行它
發布 UAID 時與 `koto_lint` 一起，因此生成的讀/寫摘要為
在 NX-17 準備情況審查期間引用的證據包中捕獲。

## 空間目錄策略執行

當主機有權訪問空間目錄快照時，AXT 句柄驗證現在默認為空間目錄快照（測試中的 CoreHost，集成流程中的 WsvHost）。每個數據空間策略條目包含 `manifest_root`、`target_lane`、`min_handle_era`、`min_sub_nonce` 和 `current_slot`。主辦方強制執行：

- 通道綁定：句柄 `target_lane` 必須與空間目錄條目匹配；
- 清單綁定：非零 `manifest_root` 值必須與句柄的 `manifest_view_root` 匹配；
- 過期：`current_slot`大於句柄的`expiry_slot`被拒絕；
- 計數器：`handle_era` 和 `sub_nonce` 必須至少為廣告的最小值；
- 成員身份：拒絕快照中不存在的數據空間的句柄。

故障映射到 `PermissionDenied`，`crates/ivm/tests/core_host_policy.rs` 中的 CoreHost 策略快照測試涵蓋每個字段的允許/拒絕情況。
塊驗證還需要每個數據空間的非空證明，其中 `expiry_slot` 覆蓋策略槽（具有配置的偏差限額）並且在句柄之前不會過期，強制描述符綁定以及聲明規範的觸摸清單（並拒絕前綴外的條目），檢查句柄意圖不變量（非零金額、範圍/主題對齊和非零era/sub_nonce/expiry），聚合每個數據空間的句柄預算，以及當信封提交時，`min_handle_era`/`min_sub_nonce` 會提前，因此即使在空間目錄重建之後，重播的子隨機數也會被拒絕。

## 錯誤目錄

規范代碼位於 `crates/iroha_data_model/src/errors.rs` 中。操作員必須在指標/日誌中逐字顯示它們，並且 SDK 應該將它們映射到可操作的重試。

|代碼|觸發|操作員回應 | SDK指導|
|------|---------|--------------------|--------------|
|缺少可用性證據（遙測）| 300 毫秒前驗證的證明者收據少於 `q`。 |檢查證明者的健康狀況，擴大下一個槽的採樣參數，保持事務排隊，並捕獲缺少可用性計數器以獲取運行手冊證據。 |沒有行動；重試會自動發生，因為 tx 保持排隊狀態。 |
| `DA_DEADLINE_EXCEEDED` | Δ 窗口期已過，但未達到 DA 法定人數。 |辭職違規證明者，發布事件記錄，強迫客戶重新提交。 |證明者返回後重建交易；考慮拆分批次。 |
| `AMX_TIMEOUT` |每個 DS 切片的準備/提交組合時間超過 250 毫秒。 |捕獲火焰圖、驗證 R/W 集並與 `iroha_amx_prepare_ms` 進行比較。 |使用較小的批次或減少爭用後重試。 |
| `AMX_LOCK_CONFLICT` |主機檢測到重疊的寫入集或無信號的觸摸。 |檢查UAID清單和靜態分析報告；如果缺少選擇器則更新清單。 |使用更正的讀/寫聲明重新編譯事務。 |
| `PVO_MISSING_OR_EXPIRED` |引用的 PVO 句柄不在緩存中或位於 `expiry_slot` 之前。 |檢查證明服務積壓、重新生成工件並驗證 Torii 索引。 |刷新證明工件並使用新句柄重新提交。 |
| `RWSET_UNBOUNDED` |靜態分析無法綁定讀/寫選擇器。 |拒絕部署，記錄選擇器堆棧跟踪，要求開發人員在重試之前修復。 |更新合約以發出顯式選擇器。 |
| `HEAVY_INSTRUCTION_DISALLOWED` |合約調用了 AMX 通道禁止的指令（例如，沒有 PVO 的大型 FFT）。 |確保 Norito 構建器在重新啟用之前使用批准的操作碼集。 |分割工作負載或添加預先計算的證明。 |
| `SETTLEMENT_ROUTER_UNAVAILABLE` |路由器無法計算確定性轉換（路徑丟失、緩衝區耗盡）。 |讓 Treasury 重新填充緩衝區或翻轉僅 XOR 模式；記錄在結算操作手冊中。 |緩衝區警報清除後重試；顯示面向用戶的警告。 |

SDK 團隊應在集成測試中鏡像這些代碼，以便 `iroha_cli`、Android、Swift、JS 和 Python 表面就錯誤文本和建議的操作達成一致。

### AXT 拒絕可觀察性

- Torii 將策略失敗顯示為 `ValidationFail::AxtReject`（並將塊驗證顯示為 `AxtEnvelopeValidationFailed`），具有穩定原因標籤、活動 `snapshot_version`、可選 `lane`/`dataspace` 標識符以及提示字段`next_min_handle_era`/`next_min_sub_nonce`。 SDK 應該向用戶冒泡這些字段，以便可以確定性地刷新過時的句柄。
- Torii 現在還使用 `X-Iroha-Axt-*` 標頭標記 HTTP 響應，以便快速分類：`Code`/`Reason`、`Snapshot-Version`、`Dataspace`、`Lane` 和可選`Next-Handle-Era`/`Next-Sub-Nonce`。 ISO 橋接拒絕帶有匹配的 `PRTRY:AXT_*` 原因代碼和相同的詳細字符串，因此儀表板和操作員可以在 AXT 故障類別中鍵入警報，而無需解碼完整的有效負載。
- 主機使用相同字段記錄 `AXT policy rejection recorded` 並通過遙測導出它們：`iroha_axt_policy_reject_total{lane,reason}` 計數拒絕，`iroha_axt_policy_snapshot_version` 跟踪活動快照的哈希值。證明緩存狀態仍然可以通過 `/v2/debug/axt/cache`（數據空間/狀態/​​清單根/插槽）獲得。
- 警報：監視由 `reason` 分組的 `iroha_axt_policy_reject_total` 中的峰值，並從日誌/ValidationFail 中使用 `snapshot_version` 進行分頁，以確認操作員是否需要輪換清單（通道/清單拒絕）或刷新句柄（era/sub_nonce/expiry）。將警報與證明緩存端點配對，以確認拒絕是與緩存相關還是與策略相關。

## 測試與證據

- CI 必須運行 `ci/acceptance/slot_1s.yml` 套件（30 分鐘混合工作負載），並且當不滿足插槽/DA/遙測閾值時合併失敗，如 `ans3.md` 中所述。
- 混沌演習（證明者抖動、預言機停頓、緩衝區耗盡）必須至少每季度執行一次，且工件存檔在 `ops/drill-log.md` 下。
- 狀態更新應包括：最新的插槽 SLO 報告、突出的錯誤峰值以及最新 PVO 緩存快照的鏈接，以便路線圖利益相關者可以審核准備情況。

通過遵循本指南，貢獻者可以滿足 AMX 文檔的路線圖要求，並為操作員和開發人員提供計時、遙測和 PVO 工作流程的單一參考。