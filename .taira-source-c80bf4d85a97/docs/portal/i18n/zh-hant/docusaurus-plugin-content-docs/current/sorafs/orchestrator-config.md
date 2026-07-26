---
id: orchestrator-config
lang: zh-hant
direction: ltr
source: docs/portal/docs/sorafs/orchestrator-config.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraFS Orchestrator Configuration
sidebar_label: Orchestrator Configuration
description: Configure the multi-source fetch orchestrator, interpret failures, and debug telemetry output.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::注意規範來源
:::

# 多源獲取協調器指南

SoraFS 多源獲取協調器驅動確定性並行
從治理支持的廣告中發布的提供商集下載。這個
指南解釋瞭如何配置協調器以及預期的故障信號
在推出期間，以及哪些遙測流公開健康指標。

## 1. 配置概述

編排器合併了三個配置源：

|來源 |目的|筆記|
|--------|---------|--------|
| `OrchestratorConfig.scoreboard` |規範化提供者權重，驗證遙測新鮮度，並保留用於審計的 JSON 記分板。 |由 `crates/sorafs_car::scoreboard::ScoreboardConfig` 支持。 |
| `OrchestratorConfig.fetch` |應用運行時限制（重試預算、並發限制、驗證切換）。 |映射到 `crates/sorafs_car::multi_fetch` 中的 `FetchOptions`。 |
| CLI/SDK 參數 |限制對等點的數量、附加遙測區域以及表面拒絕/提升策略。 | `sorafs_cli fetch` 直接公開這些標誌； SDK 通過 `OrchestratorConfig` 將它們線程化。 |

`crates/sorafs_orchestrator::bindings` 中的 JSON 幫助程序序列化整個
配置到 Norito JSON，使其可跨 SDK 綁定和
自動化。

### 1.1 JSON 配置示例

```json
{
  "scoreboard": {
    "latency_cap_ms": 6000,
    "weight_scale": 12000,
    "telemetry_grace_secs": 900,
    "persist_path": "/var/lib/sorafs/scoreboards/latest.json"
  },
  "fetch": {
    "verify_lengths": true,
    "verify_digests": true,
    "retry_budget": 4,
    "provider_failure_threshold": 3,
    "global_parallel_limit": 8
  },
  "telemetry_region": "iad-prod",
  "max_providers": 6,
  "transport_policy": "soranet_first"
}
```

通過通常的 `iroha_config` 分層保留文件（`defaults/`，用戶，
實際），因此確定性部署在節點之間繼承相同的限制。
對於與 SNNet-5a 部署一致的僅直接後備配置文件，
請參閱 `docs/examples/sorafs_direct_mode_policy.json` 和同伴
`docs/source/sorafs/direct_mode_pack.md` 中的指導。

### 1.2 合規性覆蓋

SNNet-9 將治理驅動的合規性納入編排器中。一個新的
Norito JSON 配置中的 `compliance` 對象捕獲剝離
強制獲取管道進入僅直接模式：

```json
"compliance": {
  "operator_jurisdictions": ["US", "JP"],
  "jurisdiction_opt_outs": ["US"],
  "blinded_cid_opt_outs": [
    "C6B434E5F23ABD318F01FEDB834B34BD16B46E0CC44CD70536233A632DFA3828"
  ],
  "audit_contacts": ["mailto:compliance@example.org"]
}
```

- `operator_jurisdictions` 聲明 ISO-3166 alpha-2 代碼，其中
  Orchestrator 實例運行。代碼在期間標準化為大寫
  解析。
- `jurisdiction_opt_outs` 鏡像治理寄存器。當任何操作員
  管轄權出現在列表中，協調器執行
  `transport_policy=direct-only` 並發出策略回退原因
  `compliance_jurisdiction_opt_out`。
- `blinded_cid_opt_outs` 列出清單摘要（盲化 CID，編碼為
  大寫十六進制）。匹配有效負載還強制僅直接調度和
  表面遙測中的 `compliance_blinded_cid_opt_out` 後備。
- `audit_contacts` 記錄治理期望運營商發布的 URI
  他們的 GAR 劇本。
- `attestations` 捕獲支持策略的簽名合規性數據包。
  每個條目定義一個可選的 `jurisdiction`（ISO-3166 alpha-2 代碼）、
  `document_uri`，規範的 64 字符 `digest_hex`，發行
  時間戳 `issued_at_ms` 和可選的 `expires_at_ms`。這些文物
  流入協調器的審計清單，以便治理工具可以鏈接
  覆蓋已簽署的文件。

通過通常的配置分層提供合規性塊，以便操作員
接收確定性覆蓋。編排器應用合規性_after_
寫入模式提示：即使 SDK 請求 `upload-pq-only`，管轄權或
明顯的選擇退出仍然會退回到僅直接傳輸，並且在沒有時會快速失敗
存在合規的提供商。

規範選擇退出目錄位於
`governance/compliance/soranet_opt_outs.json`；治理委員會發布
通過標記版本進行更新。完整的示例配置（包括
證明）可在 `docs/examples/sorafs_compliance_policy.json` 中找到，並且
操作過程被捕獲在
[GAR 合規手冊](../../../source/soranet/gar_compliance_playbook.md)。

### 1.3 CLI 和 SDK 旋鈕

|旗幟/場|效果|
|--------------|--------|
| `--max-peers` / `OrchestratorConfig::with_max_providers` |限制有多少提供商能夠通過記分板過濾器。設置為 `None` 以使用每個符合條件的提供商。 |
| `--retry-budget` / `FetchOptions::per_chunk_retry_limit` |每個塊的重試次數上限。超過限制會引發 `MultiSourceError::ExhaustedRetries`。 |
| `--telemetry-json` |將延遲/故障快照注入記分板生成器。超過 `telemetry_grace_secs` 的過時遙測數據將導致提供商不符合資格。 |
| `--scoreboard-out` |保留計算的記分板（合格 + 不合格的提供商）以進行運行後檢查。 |
| `--scoreboard-now` |覆蓋記分牌時間戳（Unix 秒），以便夾具捕獲保持確定性。 |
| `--deny-provider` / 分數政策掛鉤 |確定地將提供商排除在日程安排之外，而不刪除廣告。對於快速響應黑名單很有用。 |
| `--boost-provider=name:delta` |調整提供商的加權循環積分，同時保持治理權重不變。 |
| `--telemetry-region` / `OrchestratorConfig::with_telemetry_region` |標記發出的指標和結構化日誌，以便儀表板可以按地理位置或推出波進行旋轉。 |
| `--transport-policy` / `OrchestratorConfig::with_transport_policy` |現在，多源協調器已成為基線，默認為 `soranet-first`。在進行降級或遵循合規指令時使用 `direct-only`，​​並為僅 PQ 試點保留 `soranet-strict`；合規性覆蓋仍然是硬性上限。 |

SoraNet-first 現在是默認的發貨方式，回滾必須引用相關的 SNNet 攔截器。 SNNet-4/5/5a/5b/6a/7/8/12/13 畢業後，治理將逐步推進所需的姿態（朝向 `soranet-strict`）；在此之前，只有事件驅動的覆蓋才應優先考慮 `direct-only`，並且必須將它們記錄在推出日誌中。

上面的所有標誌都接受 `--` 風格的語法 `sorafs_cli fetch` 和
面向開發人員的 `sorafs_fetch` 二進製文件。 SDK 通過類型公開相同的選項
建設者。

### 1.4 Guard 緩存管理

CLI 現在連接到 SoraNet 防護選擇器，以便操作員可以鎖定條目
在 SNNet-5 傳輸全面推出之前確定性地進行中繼。三
新標誌控制工作流程：

|旗幟|目的|
|------|---------|
| `--guard-directory <PATH>` |指向描述最新中繼共識的 JSON 文件（如下所示的子集）。傳遞目錄會在執行提取之前刷新防護緩存。 |
| `--guard-cache <PATH>` |保留 Norito 編碼的 `GuardSet`。即使沒有提供新目錄，後續運行也會重用緩存。 |
| `--guard-target <COUNT>` / `--guard-retention-days <DAYS>` |可選覆蓋要固定的門禁數量（默認 3）和保留窗口（默認 30 天）。 |
| `--guard-cache-key <HEX>` |可選的 32 字節密鑰用於使用 Blake3 MAC 標記防護緩存，以便可以在重複使用之前驗證文件。 |

保護目錄有效負載使用緊湊的架構：

`--guard-directory` 標誌現在需要 Norito 編碼
`GuardDirectorySnapshotV2` 有效負載。二進制快照包含：

- `version` — 架構版本（當前為 `2`）。
- `directory_hash`、`published_at_unix`、`valid_after_unix`、`valid_until_unix` — 共識
  必須與每個嵌入證書匹配的元數據。
- `validation_phase` — 證書策略門（`1` = 允許單個 Ed25519 簽名，
  `2` = 更喜歡雙重簽名，`3` = 需要雙重簽名）。
- `issuers` — `fingerprint`、`ed25519_public` 和 `mldsa65_public` 的治理髮行人。
  指紋計算如下
  `BLAKE3("soranet.src.v2.issuer" || ed25519 || u32(len(ml-dsa)) || ml-dsa)`。
- `relays` — SRCv2 捆綁包列表（`RelayCertificateBundleV2::to_cbor()` 輸出）。每束
  攜帶中繼描述符、功能標誌、ML-KEM 策略和雙 Ed25519/ML-DSA-65
  簽名。

CLI 在將目錄與合併之前根據聲明的發行者密鑰驗證每個包

使用 `--guard-directory` 調用 CLI，將最新共識與
現有的緩存。選擇器保留仍在範圍內的固定防護裝置
保留窗口和目錄中的資格；更換過期的新繼電器
條目。成功獲取後，更新的緩存將寫迴路徑
通過 `--guard-cache` 提供，保持後續會話的確定性。軟件開發工具包
可以通過調用重現相同的行為
`GuardSelector::select(&RelayDirectory, existing_guard_set, now_unix_secs)` 和
將生成的 `GuardSet` 線程化到 `SorafsGatewayFetchOptions`。

`ml_kem_public_hex` 使選擇器能夠在
SNNet-5 的推出。階段切換（`anon-guard-pq`、`anon-majority-pq`、
`anon-strict-pq`) 現在會自動降級經典繼電器：當 PQ 防護處於
可用選擇器刪除多餘的經典引腳，以便後續會話有利於
混合握手。 CLI/SDK 摘要通過以下方式顯示結果組合
`anonymity_status`/`anonymity_reason`、`anonymity_effective_policy`、
`anonymity_pq_selected`，
`anonymity_classical_selected`，`anonymity_pq_ratio`，
`anonymity_classical_ratio`，以及伴隨的候選/赤字/供應增量
場，使限電和經典後備變得明確。

Guard 目錄現在可以通過以下方式嵌入完整的 SRCv2 包
`certificate_base64`。編排器解碼每個包，重新驗證
Ed25519/ML-DSA 簽名，並保留解析的證書以及
保護緩存。當證書存在時，它就成為規範來源
PQ 密鑰、握手套件偏好和權重；過期的證書是
通過電路生命週期管理傳播並通過
`telemetry::sorafs.guard` 和 `telemetry::sorafs.circuit`，記錄了
有效性窗口、握手套件以及是否觀察到雙重簽名
每個警衛。

使用 CLI 幫助程序使快照與發布者保持同步：```bash
sorafs_cli guard-directory fetch \
  --url https://directory.soranet.dev/mainnet_snapshot.norito \
  --output ./state/guard_directory.norito \
  --expected-directory-hash <directory-hash-hex>

sorafs_cli guard-directory verify \
  --path ./state/guard_directory.norito \
  --expected-directory-hash <directory-hash-hex>
```

`fetch` 在將 SRCv2 快照寫入磁盤之前下載並驗證它，
而 `verify` 重播來自其他來源的工件的驗證管道
團隊，發出反映 CLI/SDK 防護選擇器輸出的 JSON 摘要。

### 1.5 電路生命週期管理器

當同時提供中繼目錄和保護緩存時，編排器
激活電路生命週期管理器以預構建和更新 SoraNet 電路
每次獲取之前。配置位於 `OrchestratorConfig` 中
(`crates/sorafs_orchestrator/src/lib.rs:305`) 通過兩個新字段：

- `relay_directory`：攜帶 SNNet-3 目錄快照，因此中間/出口躍點
  可以確定性地選擇。
- `circuit_manager`：可選配置（默認啟用）控制
  電路TTL。

Norito JSON 現在接受 `circuit_manager` 塊：

```json
"circuit_manager": {
  "enabled": true,
  "circuit_ttl_secs": 900
}
```

SDK 通過以下方式轉發目錄數據
`SorafsGatewayFetchOptions::relay_directory`
(`crates/iroha/src/client.rs:320`)，並且 CLI 會在任何時候自動連接它
提供 `--guard-directory` (`crates/iroha_cli/src/commands/sorafs.rs:365`)。

每當保護元數據發生變化（端點、PQ 密鑰、
或固定時間戳）或 TTL 已過。助手 `refresh_circuits`
在每次獲取之前調用 (`crates/sorafs_orchestrator/src/lib.rs:1346`)
發出 `CircuitEvent` 日誌，以便操作員可以跟踪生命週期決策。浸泡
測試 `circuit_manager_latency_soak_remains_stable_across_rotations`
(`crates/sorafs_orchestrator/src/soranet.rs:1479`) 表現出穩定的延遲
跨越三個後衛輪換；請參閱隨附的報告
`docs/source/soranet/reports/circuit_stability.md:1`。

### 1.6 本地 QUIC 代理

編排器可以選擇生成本地 QUIC 代理，以便瀏覽器擴展
SDK 適配器不必管理證書或保護緩存密鑰。的
代理綁定到環回地址，終止 QUIC 連接，並返回
Norito 清單描述了證書和可選的防護緩存密鑰
客戶。代理髮出的傳輸事件通過以下方式進行計數
`sorafs_orchestrator_transport_events_total`。

通過 Orchestrator JSON 中的新 `local_proxy` 塊啟用代理：

```json
"local_proxy": {
  "bind_addr": "127.0.0.1:9443",
  "telemetry_label": "dev-proxy",
  "guard_cache_key_hex": "0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF",
  "emit_browser_manifest": true,
  "proxy_mode": "bridge",
  "prewarm_circuits": true,
  "max_streams_per_circuit": 64,
  "circuit_ttl_hint_secs": 300,
  "norito_bridge": {
    "spool_dir": "./storage/streaming/soranet_routes",
    "extension": "norito"
  },
  "kaigi_bridge": {
    "spool_dir": "./storage/streaming/soranet_routes",
    "extension": "norito",
    "room_policy": "public"
  }
}
```

- `bind_addr` 控制代理監聽的位置（使用 `0` 端口請求
  臨時端口）。
- `telemetry_label` 傳播到指標中，以便儀表板可以區分
  來自獲取會話的代理。
- `guard_cache_key_hex`（可選）讓代理表面具有相同的鍵控防護
  CLI/SDK 依賴的緩存，保持瀏覽器擴展同步。
- `emit_browser_manifest` 切換握手是否返回清單
  擴展可以存儲和驗證。
- `proxy_mode` 選擇代理是在本地橋接流量 (`bridge`) 還是
  僅發出元數據，因此 SDK 可以自行打開 SoraNet 電路
  （`metadata-only`）。代理默認為`bridge`；設置 `metadata-only` 時
  工作站應公開清單而不中繼流。
- `prewarm_circuits`、`max_streams_per_circuit` 和 `circuit_ttl_hint_secs`
  向瀏覽器顯示額外的提示，以便它可以預算並行流和
  了解代理如何積極地重用電路。
- `car_bridge`（可選）指向本地 CAR 存檔緩存。 `extension`
  字段控制流目標省略 `*.car` 時附加的後綴；設置
  `allow_zst = true` 直接服務預壓縮的 `*.car.zst` 有效負載。
- `kaigi_bridge`（可選）向代理公開假脫機的 Kaigi 路由。的
  `room_policy` 字段通告網橋是否運行在 `public` 或
  `authenticated` 模式，以便瀏覽器客戶端可以預先選擇正確的 GAR 標籤。
- `sorafs_cli fetch` 暴露 `--local-proxy-mode=bridge|metadata-only` 和
  `--local-proxy-norito-spool=PATH` 覆蓋，讓操作員切換
  運行時模式或指向備用線軸，無需修改 JSON 策略。
- `downgrade_remediation`（可選）配置自動降級掛鉤。
  啟用後，協調器會監視中繼遙測以發現降級突發
  並且，在 `window_secs` 內配置 `threshold` 後，強製本地
  代理到 `target_mode`（默認 `metadata-only`）。一旦降級停止
  代理在 `cooldown_secs` 之後恢復為 `resume_mode`。使用 `modes`
  數組將觸發器範圍限定為特定中繼角色（默認為入口中繼）。

當代理以橋接模式運行時，它提供兩個應用程序服務：

- **`norito`** – 客戶端的流目標相對於
  `norito_bridge.spool_dir`。目標被清理（沒有遍歷，沒有絕對
  路徑），並且當文件缺少擴展名時，將應用配置的後綴
  在有效負載逐字傳輸到瀏覽器之前。
- **`car`** – 流目標在 `car_bridge.cache_dir` 內解析，繼承
  配置默認擴展，並拒絕壓縮有效負載，除非
  `allow_zst` 已設置。成功的網橋之前回复 `STREAM_ACK_OK`
  傳輸存檔字節，以便客戶端可以進行管道驗證。

在這兩種情況下，代理都會提供緩存標籤 HMAC（當保護緩存密鑰被
握手期間存在）並記錄 `norito_*` / `car_*` 遙測原因
代碼，以便儀表板可以區分成功、丟失文件和清理
失敗一目了然。

`Orchestrator::local_proxy().await` 公開運行句柄，以便調用者可以
讀取證書 PEM、獲取瀏覽器清單或請求一個優雅的
當應用程序退出時關閉。

啟用後，代理現在提供**清單 v2** 記錄。除了現有的
證書和防護緩存密鑰，v2 增加：

- `alpn` (`"sorafs-proxy/1"`) 和 `capabilities` 陣列，以便客戶可以確認
  他們應該使用的流協議。
- 每次握手 `session_id` 和緩存標記鹽（`cache_tagging` 塊）
  派生每個會話防護關聯性和 HMAC 標籤。
- 電路和保護選擇提示（`circuit`、`guard_selection`、
  `route_hints`），因此瀏覽器集成可以在流之前公開更豐富的 UI
  打開。
- `telemetry_v2`，帶有用於本地儀器的採樣和隱私旋鈕。
- 每個 `STREAM_ACK_OK` 包括 `cache_tag_hex`。客戶反映了價值
  發出 HTTP 或 TCP 請求時的 `x-sorafs-cache-tag` 標頭如此緩存
  守衛選擇在靜態時保持加密狀態。

繼續依賴 v1 子集。

## 2. 失敗語義

協調者在單個項目之前執行嚴格的能力和預算檢查
字節被傳輸。失敗分為三類：

1. **資格失敗（飛行前）。 ** 提供商缺少範圍能力，
   過期的廣告或陳舊的遙測數據會記錄在記分牌製品中，並且
   從調度中省略。 CLI 摘要填充 `ineligible_providers`
   列出原因，以便操作員可以檢查治理偏差而無需刮擦
   日誌。
2. **運行時耗盡。 ** 每個提供程序都會跟踪連續的失敗。一旦
   達到配置的 `provider_failure_threshold`，提供商被標記
   `disabled` 剩餘的會話時間。如果每個提供商都轉變為
   `disabled`，協調器返回
   `MultiSourceError::NoHealthyProviders { last_error, chunk_index }`。
3. **確定性中止。 ** 硬限製表現為結構化錯誤：
   - `MultiSourceError::NoCompatibleProviders` — 清單需要一個塊
     其餘提供商無法兌現的跨度或一致性。
   - `MultiSourceError::ExhaustedRetries` — 每個塊重試預算為
     消耗了。
   - `MultiSourceError::ObserverFailed` — 下游觀察者（流掛鉤）
     拒絕了已驗證的塊。

每個錯誤都會嵌入有問題的塊索引，並且在可用時，會嵌入最終的塊索引
提供商失敗原因。將它們視為釋放阻滯劑 - 使用相同的重試
輸入將重現故障，直到底層廣告、遙測或
提供者的健康狀況發生變化。

### 2.1 記分牌持久化

配置 `persist_path` 時，編排器將寫入最終記分板
每次跑步後。 JSON 文檔包含：

- `eligibility`（`eligible` 或 `ineligible::<reason>`）。
- `weight`（為本次運行分配的歸一化權重）。
- `provider` 元數據（標識符、端點、並發預算）。

將記分板快照與發布工件一起存檔，以便列入黑名單和
推出決策仍然可以審計。

## 3. 遙測和調試

### 3.1 Prometheus 指標

協調器通過 `iroha_telemetry` 發出以下指標：|公制|標籤|描述 |
|--------|--------|-------------|
| `sorafs_orchestrator_active_fetches` | `manifest_id`、`region` |飛行中精心策劃的獲取的衡量標準。 |
| `sorafs_orchestrator_fetch_duration_ms` | `manifest_id`，`region` |記錄端到端獲取延遲的直方圖。 |
| `sorafs_orchestrator_fetch_failures_total` | `manifest_id`、`region`、`reason` |終端故障計數器（重試耗盡、無提供者、觀察者故障）。 |
| `sorafs_orchestrator_retries_total` | `manifest_id`、`provider`、`reason` |每個提供商的重試嘗試計數器。 |
| `sorafs_orchestrator_provider_failures_total` | `manifest_id`、`provider`、`reason` |導致禁用的會話級提供程序故障的計數器。 |
| `sorafs_orchestrator_policy_events_total` | `region`、`stage`、`outcome`、`reason` |按推出階段和後備原因分組的匿名政策決策（滿足與限制）計數。 |
| `sorafs_orchestrator_pq_ratio` | `region`，`stage` |所選 SoraNet 集之間 PQ 中繼份額的直方圖。 |
| `sorafs_orchestrator_pq_candidate_ratio` | `region`，`stage` |記分板快照中 PQ 繼電器供電比率的直方圖。 |
| `sorafs_orchestrator_pq_deficit_ratio` | `region`，`stage` |政策缺口的直方圖（目標與實際 PQ 份額之間的差距）。 |
| `sorafs_orchestrator_classical_ratio` | `region`，`stage` |每個會話中使用的經典中繼份額的直方圖。 |
| `sorafs_orchestrator_classical_selected` | `region`、`stage` |每個會話選擇的經典接力計數的直方圖。 |

在調整生產旋鈕之前，將指標集成到暫存儀表板中。
推薦的佈局反映了 SF-6 可觀測性計劃：

1. **主動獲取** — 如果儀表上升而沒有匹配的完成，則會發出警報。
2. **重試率** — 當 `retry` 計數器超過歷史基線時發出警告。
3. **提供商故障** — 當任何提供商交叉時觸發尋呼機警報
   15 分鐘內 `session_failure > 0`。

### 3.2 結構化日誌目標

協調器將結構化事件發佈到確定性目標：

- `telemetry::sorafs.fetch.lifecycle` — `start` 和 `complete` 生命週期
  帶有塊計數、重試和總持續時間的標記。
- `telemetry::sorafs.fetch.retry` — 重試事件（`provider`、`reason`、
  `attempts`) 用於輸入手動分類。
- `telemetry::sorafs.fetch.provider_failure` — 提供商因以下原因被禁用
  重複錯誤。
- `telemetry::sorafs.fetch.error` — 終端故障總結為
  `reason` 和可選的提供者元數據。

將這些流轉發到現有的 Norito 日誌管道，以便事件響應
有單一的事實來源。生命週期事件通過以下方式暴露 PQ/經典組合
`anonymity_effective_policy`、`anonymity_pq_ratio`、
`anonymity_classical_ratio` 及其配套計數器，
使得連接儀表板變得簡單，而無需抓取指標。期間
GA 推出，將生命週期/重試事件的日誌級別固定到 `info`，並依賴
`warn` 表示終端錯誤。

### 3.3 JSON 總結

`sorafs_cli fetch` 和 Rust SDK 都返回一個結構化摘要，其中包含：

- `provider_reports` 包含成功/失敗計數以及提供者是否
  禁用。
- `chunk_receipts` 詳細說明哪個提供商滿足每個塊。
- `retry_stats` 和 `ineligible_providers` 陣列。

調試行為不當的提供商時歸檔摘要文件 - 收據圖
直接到上面的日誌元數據。

## 4. 操作清單

1. **CI 中的階段配置。 ** 使用目標運行 `sorafs_fetch`
   配置，通過 `--scoreboard-out` 捕獲資格視圖，以及
   與之前版本的差異。任何意外的不合格提供商都會停止
   促銷。
2. **驗證遙測。 ** 確保部署導出 `sorafs.fetch.*`
   在為用戶啟用多源獲取之前，先查看指標和結構化日誌。
   缺乏指標通常表明編排器外觀沒有
   調用。
3. **文件覆蓋。 ** 當申請緊急 `--deny-provider` 或
   `--boost-provider` 設置，將 JSON（或 CLI 調用）提交到您的
   更改日誌。回滾必須恢復覆蓋並捕獲新的記分板
   快照。
4. **重新運行冒煙測試。 ** 修改重試預算或提供商上限後，
   重新獲取規範夾具（`fixtures/sorafs_manifest/ci_sample/`）並
   驗證塊接收是否保持確定性。

遵循上述步驟可以使協調器行為在各個環境中重現
分階段推出並提供事件響應所需的遙測。

### 4.1 策略覆蓋

操作員可以固定主動傳輸/匿名階段，而無需編輯
通過設置 `policy_override.transport_policy` 和
`policy_override.anonymity_policy` 在其 `orchestrator` JSON 中（或提供
`--transport-policy-override=` / `--anonymity-policy-override=` 至
`sorafs_cli fetch`）。當存在任一覆蓋時，編排器會跳過
通常的掉電後備：如果無法滿足請求的 PQ 層，則
獲取失敗並顯示 `no providers`，而不是悄悄降級。回滾到
默認行為就像清除覆蓋字段一樣簡單。

標準 `iroha_cli app sorafs fetch` 命令公開相同的覆蓋標誌，
將它們轉發到網關客戶端，以便臨時獲取和自動化腳本
共享相同的階段固定行為。

跨 SDK 裝置位於 `fixtures/sorafs_gateway/policy_override/` 下。的
CLI、Rust 客戶端、JavaScript 綁定和 Swift Harness 解碼
`override.json` 在其奇偶校驗套件中，因此對覆蓋有效負載的任何更改
必須更新該夾具並重新運行 `cargo test -p iroha`、`npm test` 和
`swift test` 以保持 SDK 一致。始終將再生的夾具連接到
更改審查，以便下游消費者可以區分覆蓋合同。

治理需要為每個覆蓋提供操作手冊條目。記錄原因，
預期持續時間，以及更改日誌中的回滾觸發器，通知 PQ
棘輪旋轉通道，並將簽名的批准附加到同一工件
存儲記分板快照的包。覆蓋是為了簡短
緊急情況（例如 PQ 警衛限電）；長期的政策變化必須取消
通過正常的理事會投票，使節點匯聚到新的默認值上。

### 4.2 PQ 棘輪消防演習

- **運行手冊：** 按照 `docs/source/soranet/pq_ratchet_runbook.md` 獲取
  升級/降級演練，包括警衛目錄處理和回滾。
- **儀表板：** 導入 `dashboards/grafana/soranet_pq_ratchet.json` 進行監控
  `sorafs_orchestrator_policy_events_total`、掉電率和 PQ 比率平均值
  演習期間。
- **自動化：** `cargo test -p sorafs_orchestrator pq_ratchet_fire_drill_records_metrics`
  執行相同的轉換並驗證指標增量是否為
  預期在操作員進行現場演練之前。

## 5. 推出劇本

SNNet-5 傳輸的推出引入了新的警衛選擇和治理
證明和政策回退。下面的劇本將序列編碼為
在為最終用戶啟用多源獲取以及降級之前遵循
返回直接模式的路徑。

### 5.1 開發人員預檢（CI/分期）

1. **在 CI 中重新生成記分板。 ** 運行 `sorafs_cli fetch`（或 SDK
   等效）針對 `fixtures/sorafs_manifest/ci_sample/` 清單
   候選配置。通過以下方式保留記分板
   `--scoreboard-out=artifacts/sorafs/scoreboard.json` 並斷言：
   - `anonymity_status=="met"` 和 `anonymity_pq_ratio` 滿足目標
     階段（`anon-guard-pq`、`anon-majority-pq` 或 `anon-strict-pq`）。
   - 確定性的塊收據仍然與承諾的黃金集相匹配
     存儲庫。
2. **驗證清單治理。 ** 檢查 CLI/SDK 摘要並確保
   新出現的 `manifest_governance.council_signatures` 陣列包含
   預計理事會指紋。這確認了網關響應發送了 GAR
   信封，並且 `validate_manifest` 接受了它。
3. **行使合規性覆蓋。 ** 從以下位置加載每個管轄區概況：
   `docs/examples/sorafs_compliance_policy.json` 並斷言協調器
   發出正確的策略回退（`compliance_jurisdiction_opt_out` 或
   `compliance_blinded_cid_opt_out`）。記錄 no 時導致的 fetch 失敗
   提供合規的運輸。
4. **模擬降級。 ** 將`transport_policy`翻轉為`direct-only`
   測試配置並重新運行獲取以確保協調器
   回退到 Torii/QUIC，而不接觸 SoraNet 繼電器。保留這個 JSON
   版本控制下的變體，因此可以在
   事件。

### 5.2 操作員推出（生產波次）1. **通過 `iroha_config` 暫存配置。 ** 發布所使用的確切 JSON
   在 CI 中作為 `actual` 層覆蓋。確認 Orchestrator pod/二進製文件
   在啟動時記錄新的配置哈希。
2. **Prime Guard 緩存。 ** 通過 `--guard-directory` 刷新中繼目錄
   並將 Norito 保護緩存保留為 `--guard-cache`。驗證緩存是否
   簽名（如果配置了 `--guard-cache-key`）並存儲在版本控制下
   改變控制。
3. **啟用遙測儀表板。 ** 在提供用戶流量之前，請確保
   環境發布 `sorafs.fetch.*`、`sorafs_orchestrator_policy_events_total` 和
   代理指標（使用本地 QUIC 代理時）。警報應與
   `anonymity_brownout_effective` 和合規性回退計數器。
4. **運行實時冒煙測試。 ** 通過每個
   提供商群組（PQ、經典和直接）並確認大塊收據，
   CAR 摘要和理事會簽名與 CI 基線相匹配。
5. **溝通激活。 ** 使用以下內容更新推出跟踪器
   `scoreboard.json` 人工製品、防護緩存指紋以及指向
   顯示第一次生產獲取的清單治理驗證的日誌。

### 5.3 降級/回滾過程

當事件、PQ 缺陷或監管要求強制回滾時，請遵循
這個確定性序列：

1. **切換傳輸策略。 ** 應用 `transport_policy=direct-only`（並且，如果
   立即停止新的 SoraNet 電路建設。
2. **刷新防護狀態。** 刪除或歸檔引用的防護緩存文件
   `--guard-cache` 因此後續運行不會嘗試重用固定繼電器。
   僅當計劃快速重新啟用並且緩存保留時才跳過此步驟
   有效。
3. **禁用本地代理。** 如果本地 QUIC 代理處於 `bridge` 模式，
   使用 `proxy_mode="metadata-only"` 重新啟動協調器或刪除
   `local_proxy` 完全阻止。記錄端口釋放以便工作站和
   瀏覽器集成恢復為直接 Torii 訪問。
4. **明確的合規性覆蓋。** 附加司法管轄區選擇退出條目（或
   盲態 CID 條目）到受影響有效負載的合規策略，以便
   自動化和儀表板反映了有意的直接模式操作。
5. **捕獲審核證據。** 使用 `--scoreboard-out` 運行更改後獲取
   並存儲 CLI JSON 摘要（包括 `manifest_governance`）
   事件票。

### 5.4 監管部署清單

|檢查站|目的|推薦證據|
|------------|---------|----------------------|
|合規政策出台 |確認管轄權剝離與 GAR 備案一致。 |已簽名的 `soranet_opt_outs.json` 快照 + Orchestrator 配置差異。 |
|顯性治理記錄 |證明每個網關清單均附有理事會簽名。 | `sorafs_cli fetch ... --output /dev/null --summary out.json` 和 `manifest_governance.council_signatures` 已存檔。 |
|認證庫存|跟踪 `compliance.attestations` 中引用的文檔。 |將 PDF/JSON 工件與證明摘要和到期時間一起存儲。 |
|降級演習已記錄 |確保回滾保持確定性。 |顯示僅應用直接策略並清除保護緩存的季度試運行記錄。 |
|遙測保留 |為監管機構提供取證數據。 |根據策略保留確認 `sorafs.fetch.*` 和合規回退的儀表板導出或 OTEL 快照。 |

運營商應在每個推出窗口之前檢查清單並提供
根據要求向治理或監管機構提供證據包。開發者可以復用
當停電或合規性覆蓋時，事後數據包的相同工件
在測試期間被觸發。