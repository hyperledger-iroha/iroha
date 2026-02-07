---
id: nexus-elastic-lane
lang: zh-hant
direction: ltr
source: docs/portal/docs/nexus/nexus-elastic-lane.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Elastic lane provisioning (NX-7)
sidebar_label: Elastic Lane Provisioning
description: Bootstrap workflow for creating Nexus lane manifests, catalog entries, and rollout evidence.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::注意規範來源
此頁面鏡像 `docs/source/nexus_elastic_lane.md`。保持兩個副本對齊，直到平移掃描落在門戶中。
:::

# 彈性通道配置工具包 (NX-7)

> **路線圖項目：** NX-7 — 彈性通道配置工具  
> **狀態：** 工具完成 - 生成清單、目錄片段、Norito 有效負載、冒煙測試、
> 負載測試包助手現在縫合時隙延遲門控+證據清單，以便驗證器
> 無需定制腳本即可發布加載運行。

本指南引導操作員了解新的 `scripts/nexus_lane_bootstrap.sh` 幫助程序，該幫助程序可實現自動化
通道清單生成、通道/數據空間目錄片段和推出證據。目標是使
無需手動編輯多個文件即可輕鬆啟動新的 Nexus 通道（公共或專用）或
手動重新導出目錄幾何形狀。

## 1.先決條件

1. 通道別名、數據空間、驗證器集、容錯（`f`）和結算政策的治理批准。
2. 最終的驗證者列表（帳戶 ID）和受保護的命名空間列表。
3. 訪問節點配置存儲庫，以便您可以附加生成的片段。
4. 通道清單註冊表的路徑（請參閱 `nexus.registry.manifest_directory` 和
   `cache_directory`）。
5. 車道的遙測觸點/PagerDuty 手柄，以便一旦車道到達即可發出警報
   上線。

## 2. 生成車道偽影

從存儲庫根運行幫助程序：

```bash
scripts/nexus_lane_bootstrap.sh \
  --lane-alias "Payments Lane" \
  --lane-id 3 \
  --dataspace-alias payments \
  --governance-module parliament \
  --settlement-handle xor_global \
  --validator ih58... \
  --validator ih58... \
  --validator ih58... \
  --protected-namespace payments \
  --description "High-throughput interbank payments lane" \
  --dataspace-description "Payments dataspace" \
  --route-instruction finance::pacs008 \
  --encode-space-directory \
  --space-directory-out artifacts/nexus/payments_lane/payments.manifest.to \
  --telemetry-contact payments-ops@sora.org \
  --output-dir artifacts/nexus/payments_lane
```

關鍵標誌：

- `--lane-id` 必須與 `nexus.lane_catalog` 中新條目的索引匹配。
- `--dataspace-alias` 和 `--dataspace-id/hash` 控制數據空間目錄條目（默認為
  車道 ID（省略時）。
- `--validator` 可以重複或源自 `--validators-file`。
- `--route-instruction` / `--route-account` 發出可粘貼的路由規則。
- `--metadata key=value`（或 `--telemetry-contact/channel/runbook`）捕獲 Runbook 聯繫人，以便
  儀表板立即列出正確的所有者。
- `--allow-runtime-upgrades` + `--runtime-upgrade-*` 將運行時升級掛鉤添加到清單中
  當車道需要擴展操作員控制時。
- `--encode-space-directory` 自動調用 `cargo xtask space-directory encode`。與它配對
  `--space-directory-out` 當您想要將編碼的 `.to` 文件放在默認值以外的位置時。

該腳本在 `--output-dir` 內生成三個工件（默認為當前目錄），
當啟用編碼時加上可選的第四個：

1. `<slug>.manifest.json` — 包含驗證器仲裁、受保護命名空間和的通道清單
   可選的運行時升級掛鉤元數據。
2. `<slug>.catalog.toml` — 包含 `[[nexus.lane_catalog]]`、`[[nexus.dataspace_catalog]]` 的 TOML 片段，
   以及任何請求的路由規則。確保在數據空間條目上設置 `fault_tolerance` 的大小
   車道接力委員會 (`3f+1`)。
3. `<slug>.summary.json` — 描述幾何形狀（塊、段、元數據）以及
   所需的推出步驟和確切的 `cargo xtask space-directory encode` 命令（在
   `space_directory_encode.command`）。將此 JSON 附加到入職票據作為證據。
4. `<slug>.manifest.to` — 當 `--encode-space-directory` 設置時發出；準備好 Torii
   `iroha app space-directory manifest publish` 流量。

使用 `--dry-run` 預覽 JSON/ 片段而不寫入文件，並使用 `--force` 覆蓋
現有的文物。

## 3. 應用更改

1. 將清單 JSON 複製到配置的 `nexus.registry.manifest_directory` 中（並複製到緩存中）
   目錄（如果註冊錶鏡像遠程包）。如果清單的版本控制在以下版本，則提交文件
   你的配置倉庫。
2. 將目錄片段附加到 `config/config.toml`（或相應的 `config.d/*.toml`）。確保
   `nexus.lane_count` 至少是 `lane_id + 1`，並更新任何 `nexus.routing_policy.rules`
   應該指向新車道。
3. 編碼（如果您跳過了 `--encode-space-directory`）並將清單發佈到空間目錄
   使用摘要中捕獲的命令 (`space_directory_encode.command`)。這產生了
   `.manifest.to` 有效負載 Torii 期望並記錄審計員的證據；通過提交
   `iroha app space-directory manifest publish`。
4. 運行 `irohad --sora --config path/to/config.toml --trace-config` 並將跟踪輸出存檔於
   推出票。這證明新的幾何形狀與生成的 slug/kura 段相匹配。
5. 部署清單/目錄更改後，重新啟動分配給通道的驗證器。保留
   票證中的摘要 JSON 以供將來審核。

## 4. 構建註冊表分發包

打包生成的清單和疊加層，以便操作員可以分發車道治理數據，而無需
在每台主機上編輯配置。捆綁器助手將清單複製到規範佈局中，
為 `nexus.registry.cache_directory` 生成可選的治理目錄覆蓋，並且可以發出
離線傳輸的 tarball：

```bash
scripts/nexus_lane_registry_bundle.sh \
  --manifest artifacts/nexus/payments_lane/payments.manifest.json \
  --output-dir artifacts/nexus/payments_lane/registry_bundle \
  --default-module parliament \
  --module name=parliament,module_type=parliament,param.quorum=2 \
  --bundle-out artifacts/nexus/payments_lane/registry_bundle.tar.gz
```

輸出：

1. `manifests/<slug>.manifest.json` — 將這些複製到配置中
   `nexus.registry.manifest_directory`。
2. `cache/governance_catalog.json` — 放入 `nexus.registry.cache_directory`。每個 `--module`
   條目成為可插入模塊定義，通過以下方式啟用治理模塊交換（NX-2）：
   更新緩存覆蓋而不是編輯 `config.toml`。
3. `summary.json` — 包括哈希值、覆蓋元數據和操作員指令。
4. 可選 `registry_bundle.tar.*` — 適用於 SCP、S3 或工件跟踪器。

將整個目錄（或存檔）同步到每個驗證器，在氣隙主機上提取，然後復制
在重新啟動 Torii 之前，將清單 + 緩存覆蓋到其註冊表路徑中。

## 5. 驗證器冒煙測試

Torii 重新啟動後，運行新的煙霧助手來驗證通道報告 `manifest_ready=true`，
指標顯示了預期的車道數，並且密封的儀表是清晰的。需要清單的車道
必須公開非空 `manifest_path`；現在，當路徑丟失時，助手會立即失敗，因此
每個 NX-7 部署記錄都包含簽名的清單證據：

```bash
scripts/nexus_lane_smoke.py \
  --status-url https://torii.example.com/v1/sumeragi/status \
  --metrics-url https://torii.example.com/metrics \
  --lane-alias payments \
  --expected-lane-count 3 \
  --min-da-quorum 0.95 \
  --max-oracle-staleness 75 \
  --expected-oracle-twap 60 \
  --oracle-twap-tolerance 5 \
  --max-oracle-haircut-bps 75 \
  --min-settlement-buffer 0.25 \
  --min-block-height 1000 \
  --max-finality-lag 4 \
  --max-settlement-backlog 0.5 \
  --max-headroom-events 0 \
  --max-slot-p95 1000 \
  --max-slot-p99 1100 \
  --min-slot-samples 10
```

測試自簽名環境時添加 `--insecure`。如果通道是，則腳本退出非零
缺失、密封或指標/遙測偏離預期值。使用
`--min-block-height`、`--max-finality-lag`、`--max-settlement-backlog` 和
`--max-headroom-events` 用於保持每通道區塊高度/最終性/積壓/淨空遙測的旋鈕
在您的操作範圍內，並將它們與 `--max-slot-p95` / `--max-slot-p99` 結合起來
（加上 `--min-slot-samples`）在不離開助手的情況下強制執行 NX-18 時隙持續時間目標。

對於氣隙驗證（或 CI），您可以重放捕獲的 Torii 響應，而不是實時響應
端點：

```bash
scripts/nexus_lane_smoke.py \
  --status-file fixtures/nexus/lanes/status_ready.json \
  --metrics-file fixtures/nexus/lanes/metrics_ready.prom \
  --lane-alias core \
  --lane-alias payments \
  --expected-lane-count 3 \
  --min-da-quorum 0.95 \
  --max-oracle-staleness 75 \
  --expected-oracle-twap 60 \
  --oracle-twap-tolerance 5 \
  --max-oracle-haircut-bps 75 \
  --min-settlement-buffer 0.25 \
  --min-block-height 1000 \
  --max-finality-lag 4 \
  --max-settlement-backlog 0.5 \
  --max-headroom-events 0 \
  --max-slot-p95 1000 \
  --max-slot-p99 1100 \
  --min-slot-samples 10
```

`fixtures/nexus/lanes/` 下記錄的裝置反映了引導程序生成的工件
幫助器，這樣新的清單就可以在沒有定制腳本的情況下進行檢查。 CI 通過以下方式執行相同的流程
`ci/check_nexus_lane_smoke.sh` 和 `ci/check_nexus_lane_registry_bundle.sh`
（別名：`make check-nexus-lanes`）以證明 NX-7 煙霧助手與已發布的數據保持一致
有效負載格式並確保包摘要/覆蓋保持可再現。

重命名通道時，捕獲 `nexus.lane.topology` 遙測事件（例如，使用
`journalctl -u irohad -o json | jq 'select(.msg=="nexus.lane.topology")'`）並將它們反饋到
煙霧助手。 `--telemetry-file/--from-telemetry` 標誌接受換行符分隔的日誌並
`--require-alias-migration old:new` 斷言 `alias_migrated` 事件記錄了重命名：

```bash
scripts/nexus_lane_smoke.py \
  --status-file fixtures/nexus/lanes/status_ready.json \
  --metrics-file fixtures/nexus/lanes/metrics_ready.prom \
  --telemetry-file fixtures/nexus/lanes/telemetry_alias_migrated.ndjson \
  --lane-alias payments \
  --lane-alias core \
  --expected-lane-count 3 \
  --min-da-quorum 0.95 \
  --max-oracle-staleness 75 \
  --expected-oracle-twap 60 \
  --oracle-twap-tolerance 5 \
  --max-oracle-haircut-bps 75 \
  --min-settlement-buffer 0.25 \
  --min-block-height 1000 \
  --max-finality-lag 4 \
  --max-settlement-backlog 0.5 \
  --max-headroom-events 0 \
  --max-slot-p95 1000 \
  --max-slot-p99 1100 \
  --min-slot-samples 10 \
  --require-alias-migration core:payments
```

`telemetry_alias_migrated.ndjson` 夾具捆綁了規範的重命名示例，以便 CI 可以驗證
遙測解析路徑，無需聯繫活動節點。

## 驗證器負載測試（NX-7 證據）

路線圖 **NX-7** 要求每個新通道都進行可重複的驗證器負載運行。使用
`scripts/nexus_lane_load_test.py` 用於縫合菸霧檢查、時隙持續時間門和時隙束
體現為治理可以重播的單個工件集：

```bash
scripts/nexus_lane_load_test.py \
  --status-file artifacts/nexus/load/payments-2026q2/torii_status.json \
  --metrics-file artifacts/nexus/load/payments-2026q2/metrics.prom \
  --telemetry-file artifacts/nexus/load/payments-2026q2/nexus.lane.topology.ndjson \
  --lane-alias payments \
  --lane-alias core \
  --expected-lane-count 3 \
  --slot-range 81200-81600 \
  --workload-seed NX7-PAYMENTS-2026Q2 \
  --require-alias-migration core:payments \
  --out-dir artifacts/nexus/load/payments-2026q2
```

幫助程序強制使用相同的 DA 仲裁、預言機、結算緩衝區、TEU 和時隙持續時間門
由煙霧助手編寫 `smoke.log`、`slot_summary.json`、插槽捆綁清單，以及
`load_test_manifest.json` 到所選的 `--out-dir` 中，因此負載運行可以直接附加到
無需定制腳本即可推出門票。

## 6. 遙測和治理後續行動

- 使用以下內容更新車道儀表板（`dashboards/grafana/nexus_lanes.json` 和相關覆蓋圖）
  新的車道 ID 和元數據。生成的元數據密鑰（`contact`、`channel`、`runbook` 等）
  預填充標籤很簡單。
- 在啟用准入之前，為新車道發送 PagerDuty/Alertmanager 規則。 `summary.json`
  next-steps 數組鏡像 [Nexus 操作](./nexus-operations) 中的清單。
- 驗證器集上線後，在空間目錄中註冊清單包。使用相同的
  由幫助程序生成的清單 JSON，根據治理運行手冊進行簽名。
- 按照 [Sora Nexus 操作員入門](./nexus-operator-onboarding) 進行冒煙測試（FindNetworkStatus、Torii
  可達性）並使用上面生成的工件集捕獲證據。

## 7. 試運行示例

要預覽工件而不寫入文件：

```bash
scripts/nexus_lane_bootstrap.sh \
  --lane-alias "Payments Lane" \
  --lane-id 3 \
  --dataspace-alias payments \
  --governance-module parliament \
  --settlement-handle xor_global \
  --validator ih58... \
  --validator ih58... \
  --dry-run
```

該命令將 JSON 摘要和 TOML 片段打印到標準輸出，從而允許在
規劃。

---

有關其他上下文，請參閱：- [Nexus 操作](./nexus-operations) — 操作清單和遙測要求。
- [Sora Nexus 操作員入門](./nexus-operator-onboarding) — 參考了詳細的入門流程
  新幫手。
- [Nexus 通道模型](./nexus-lane-model) — 工具使用的通道幾何形狀、段塊和存儲佈局。