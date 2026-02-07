---
lang: zh-hant
direction: ltr
source: docs/source/fastpq_rollout_playbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3a0c22a213e04a6a8fef94ded6ec0017531737ffd4b9418ec94286bb6759ff8a
source_last_modified: "2026-01-08T09:53:05.148398+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# FASTPQ 推出手冊（第 7-3 階段）

本手冊實現了 Stage7-3 路線圖要求：每次機隊升級
啟用 FASTPQ GPU 執行必須附加可重現的基準清單，
配對的 Grafana 證據和記錄的回滾演習。它補充了
`docs/source/fastpq_plan.md`（目標/架構）和
`docs/source/fastpq_migration_guide.md`（節點級升級步驟）重點
在面向操作員的部署清單上。

## 範圍和角色

- **發布工程/SRE：**自己的基準捕獲、清單簽名和
  在批准推出之前導出儀表板。
- **Ops Guild：** 進行分階段部署、記錄回滾排練和存儲
  `artifacts/fastpq_rollouts/<timestamp>/` 下的工件包。
- **治理/合規性：** 驗證每次變更都有證據
  在為隊列切換 FASTPQ 默認值之前請求。

## 證據包要求

每個推出提交都必須包含以下工件。附加所有文件
到發布/升級票並將捆綁包保留在
`artifacts/fastpq_rollouts/<YYYYMMDD>/<fleet>/<lane>/`。|文物|目的|如何生產 |
|----------|---------|----------------|
| `fastpq_bench_manifest.json` |證明規範的 20000 行工作負載保持在 `<1 s` LDE 上限之下，並記錄每個包裝基準的哈希值。捕獲 Metal/CUDA 運行，包裝它們，然後運行：`cargo xtask fastpq-bench-manifest \``  --bench metal=artifacts/fastpq_benchmarks/<metal>.json \``  --bench cuda=artifacts/fastpq_benchmarks/<cuda>.json \``  --matrix artifacts/fastpq_benchmarks/matrix/matrix_manifest.json \``  --signing-key secrets/fastpq_bench.ed25519 \``  --out artifacts/fastpq_rollouts/<stamp>/fastpq_bench_manifest.json` |
|打包基準（`fastpq_metal_bench_*.json`、`fastpq_cuda_bench_*.json`）|捕獲主機元數據、行使用證據、零填充熱點、Poseidon 微基準摘要以及儀表板/警報使用的內核統計信息。運行 `fastpq_metal_bench` / `fastpq_cuda_bench`，然後包裝原始 JSON：`python3 scripts/fastpq/wrap_benchmark.py --require-lde-mean-ms 950 --require-poseidon-mean-ms 1000 \``  --row-usage artifacts/fastpq_benchmarks/fastpq_row_usage_2025-05-12.json \``  --poseidon-metrics artifacts/fastpq_rollouts/<stamp>/<fleet>/<lane>/metrics_poseidon.prom \``  fastpq_metal_bench.json artifacts/fastpq_benchmarks/<metal>.json --sign-output`重複進行 CUDA 捕獲（點相關見證/抓取文件中的 `--row-usage` 和 `--poseidon-metrics`）。幫助程序嵌入過濾後的 `fastpq_poseidon_pipeline_total`/`fastpq_execution_mode_total` 樣本，因此 WP2-E.6 證據在 Metal 和 CUDA 中是相同的。當您需要獨立的 Poseidon microbench 摘要（支持包裝或原始輸入）時，請使用 `scripts/fastpq/export_poseidon_microbench.py --bundle <bundle>`。 |
|  |  | **第 7 階段標籤要求：** `wrap_benchmark.py` 現在失敗，除非生成的 `metadata.labels` 部分同時包含 `device_class` 和 `gpu_kind`。當自動檢測無法推斷它們時（例如，在分離的 CI 節點上進行包裝時），請傳遞顯式覆蓋，例如 `--label device_class=xeon-rtx-sm80 --label gpu_kind=discrete`。 |
|  |  | **加速遙測：**默認情況下，包裝器還會捕獲 `cargo xtask acceleration-state --format json`，並在包裝​​基準旁邊寫入 `<bundle>.accel.json` 和 `<bundle>.accel.prom`（使用 `--accel-*` 標誌或 `--skip-acceleration-state` 覆蓋）。捕獲矩陣使用這些文件為車隊儀表板構建 `acceleration_matrix.{json,md}`。 |
| Grafana 出口 |證明推出窗口採用遙測和警報註釋。 |導出 `fastpq-acceleration` 儀表板：`curl -s -H "Authorization: Bearer $GRAFANA_TOKEN" \``  "$GRAFANA_URL/api/dashboards/uid/fastpq-acceleration" \``  | jq '.dashboard' \``  > artifacts/fastpq_rollouts/<stamp>/grafana_fastpq_acceleration.json`在導出之前使用推出開始/停止時間對電路板進行註釋。發布管道可以通過 `scripts/run_release_pipeline.py --export-fastpq-grafana --grafana-url <URL>`（通過 `GRAFANA_TOKEN` 提供的令牌）自動執行此操作。 |
|警報快照|捕獲保護部署的警報規則。將 `dashboards/alerts/fastpq_acceleration_rules.yml`（和 `tests/` 夾具）複製到捆綁包中，以便審閱者可以重新運行 `promtool test rules …`。 |
|回滾演練日誌 |證明操作員演練了強制 CPU 回退和遙測確認。 |使用 [回滾練習](#rollback-drills) 中的過程並存儲控制台日誌 (`rollback_drill.log`) 以及生成的 Prometheus 抓取 (`metrics_rollback.prom`)。 || `row_usage/fastpq_row_usage_<date>.json` |記錄 TF-5 在 CI 和儀表板中跟踪的 ExecWitness FASTPQ 行分配。從 Torii 下載新的見證，通過 `iroha_cli audit witness --decode exec.witness` 對其進行解碼（可以選擇添加 `--fastpq-parameter fastpq-lane-balanced` 以斷言預期的參數集；默認情況下會發出 FASTPQ 批次），然後將 `row_usage` JSON 複製到 `artifacts/fastpq_rollouts/<stamp>/<fleet>/<lane>/row_usage/` 中。保留文件名時間戳，以便審閱者可以將它們與部署票證相關聯，並運行 `python3 scripts/fastpq/validate_row_usage_snapshot.py row_usage/*.json`（或 `make check-fastpq-rollout`），以便 Stage7-3 門在附加證據之前驗證每個批次是否公佈選擇器計數和 `transfer_ratio = transfer_rows / total_rows` 不變性。 |

> **提示：** `artifacts/fastpq_rollouts/README.md` 記錄首選命名
> 方案 (`<stamp>/<fleet>/<lane>`) 和所需的證據文件。的
> `<stamp>` 文件夾必須編碼 `YYYYMMDDThhmmZ`，以便工件保持可排序
> 無需諮詢門票。

## 證據生成清單1. **捕獲 GPU 基準測試。 **
   - 通過以下方式運行規範工作負載（20000 個邏輯行，32768 個填充行）
     `cargo run -p fastpq_prover --bin fastpq_metal_bench -- --rows 20000 --pretty`。
   - 使用 `--row-usage <decoded witness>` 將結果包裹在 `scripts/fastpq/wrap_benchmark.py` 中，以便捆綁包中包含設備證據以及 GPU 遙測數據。通過 `--require-lde-mean-ms 950 --require-poseidon-mean-ms 1000 --sign-output`，以便在任一加速器超過目標或 Poseidon 隊列/配置文件遙測丟失時包裝器快速失敗，並生成分離的簽名。
   - 在 CUDA 主機上重複此操作，以便清單包含兩個 GPU 系列。
   - **不要**剝離 `benchmarks.metal_dispatch_queue` 或
     `benchmarks.zero_fill_hotspots` 來自包裝的 JSON 的塊。 CI門
     (`ci/check_fastpq_rollout.sh`) 現在讀取這些字段並在隊列時失敗
     淨空下降到低於一個插槽或當任何 LDE 熱點報告 `mean_ms >
     0.40ms`，自動執行 Stage7 遙測防護。
2. **生成清單。 ** 使用 `cargo xtask fastpq-bench-manifest …` 作為
   如表所示。將 `fastpq_bench_manifest.json` 存儲在部署包中。
3. **導出 Grafana。 **
   - 用卷展窗口註釋 `FASTPQ Acceleration Overview` 板，
     鏈接到相關的 Grafana 面板 ID。
   - 通過 Grafana API（上面的命令）導出儀表板 JSON 並包含
     `annotations` 部分，以便審閱者可以將採用曲線與
     分階段推出。
4. **快照警報。 ** 複製使用的確切警報規則 (`dashboards/alerts/…`)
   通過推出到捆綁包中。如果 Prometheus 規則被覆蓋，包括
   覆蓋差異。
5. **Prometheus/OTEL 抓取。 ** 從每個中捕獲 `fastpq_execution_mode_total{device_class="<matrix>"}`
   主持人（台前台後）加OTEL櫃檯
   `fastpq.execution_mode_resolutions_total` 和配對的
   `telemetry::fastpq.execution_mode` 日誌條目。這些文物證明
   GPU 的採用是穩定的，並且強制 CPU 回退仍然會發出遙測數據。
6. **存檔行使用遙測。 ** 解碼 ExecWitness 運行後
   推出時，將生成的 JSON 放在捆綁包中的 `row_usage/` 下。 CI
   幫助程序 (`ci/check_fastpq_row_usage.sh`) 將這些快照與
   規範基線，`ci/check_fastpq_rollout.sh` 現在需要每個
   捆綁發送至少一個 `row_usage` 文件以保留 TF-5 證據
   到發行票。

## 分階段推出流程

對每個隊列使用三個確定性階段。退出後才可前進
每個階段的標準都得到滿足並記錄在證據包中。|相|範圍 |退出標準 |附件 |
|--------|---------|----------------|------------|
|飛行員（P1）|每個區域 1 個控制平面 + 1 個數據平面節點 | `fastpq_execution_mode_total{device_class="<matrix>", backend="metal"}` ≥90%，持續 48 小時，Alertmanager 事件為零，並通過回滾演練。 |來自兩台主機的捆綁包（基準 JSON、帶試點註釋的 Grafana 導出、回滾日誌）。 |
|坡道 (P2) | ≥50% 的驗證者加上每個集群至少一個存檔通道 | GPU 執行持續 5 天，降級峰值不超過 1 次 > 10 分鐘，Prometheus 計數器證明 60 秒內有回退警報。 |更新了 Grafana 導出，顯示斜坡註釋、Prometheus 刮擦差異、Alertmanager 屏幕截圖/日誌。 |
|默認（P3）|剩餘節點； FASTPQ 在 `iroha_config` 中標記為默認值 |簽名的工作台清單 + Grafana 導出引用最終採用曲線，並記錄回滾演練演示配置切換。 |最終清單、Grafana JSON、回滾日誌、配置更改審核的票證參考。 |

在推廣票中記錄每個促銷步驟並直接鏈接到
`grafana_fastpq_acceleration.json` 註釋，以便審閱者可以將
時間線與證據。

## 回滾練習

每個推出階段都必須包括回滾演練：

1. 每個集群選擇一個節點並記錄當前指標：
   ```bash
   curl -s http://<host>:8180/metrics | rg 'fastpq_execution_mode_total{device_class'
   ```
2. 使用配置旋鈕強制 CPU 模式 10 分鐘
   (`zk.fastpq.execution_mode = "cpu"`) 或環境覆蓋：
   ```bash
   FASTPQ_GPU=cpu irohad --config <path> --genesis-manifest-json <path>
   ```
3.確認降級日誌
   (`telemetry::fastpq.execution_mode resolved="cpu" requested="gpu"`) 並刮掉
   再次訪問 Prometheus 端點以顯示計數器增量。
4.恢復GPU模式，驗證`telemetry::fastpq.execution_mode`現在報告
   `resolved="metal"`（或 `resolved="cuda"/"opencl"` 對於非金屬通道），
   確認 Prometheus 抓取包含 CPU 和 GPU 樣本
   `fastpq_execution_mode_total{backend=…}`，並將經過的時間記錄到
   檢測/清理。
5. 將 shell 記錄、指標和操作員確認存儲為
   推出捆綁包中的 `rollback_drill.log` 和 `metrics_rollback.prom`。這些
   文件必須說明完整的降級 + 恢復週期，因為
   現在，只要日誌缺少 GPU，`ci/check_fastpq_rollout.sh` 就會失敗
   恢復行或指標快照忽略 CPU 或 GPU 計數器。

這些日誌證明每個集群都可以正常降級，並且 SRE 團隊
知道如何在 GPU 驅動程序或內核退化時確定性地回退。

## 混合模式後備證據 (WP2-E.6)

每當主機需要 GPU FFT/LDE 但需要 CPU Poseidon 哈希時（根據 Stage7 <900ms
要求），將以下工件與標準回滾日誌捆綁在一起：1. **配置差異。 **簽入（或附加）設置的主機本地覆蓋
   `zk.fastpq.poseidon_mode = "cpu"` (`FASTPQ_POSEIDON_MODE=cpu`) 離開時
   `zk.fastpq.execution_mode` 未受影響。為補丁命名
   `artifacts/fastpq_rollouts/<stamp>/<fleet>/<lane>/poseidon_fallback.patch`。
2. **波塞冬反刮。 **
   ```bash
   curl -s http://<host>:8180/metrics \
     | rg 'fastpq_poseidon_pipeline_total{.*device_class="<label>"' \
     > artifacts/fastpq_rollouts/<stamp>/<fleet>/<lane>/metrics_poseidon.prom
   ```
   捕獲必須顯示 `path="cpu_forced"` 與
   該設備類別的 GPU FFT/LDE 計數器。恢復後進行第二次刮擦
   返回 GPU 模式，以便審閱者可以看到 `path="gpu"` 行恢復。

   將生成的文件傳遞給 `wrap_benchmark.py --poseidon-metrics …`，以便包裝後的基準測試在其 `poseidon_metrics` 部分中記錄相同的計數器；這使得 Metal 和 CUDA 的部署保持在相同的工作流程上，並且無需打開單獨的抓取文件即可審核後備證據。
3. **日誌摘錄。 ** 複製 `telemetry::fastpq.poseidon` 條目以證明
   解析器翻轉到CPU（`cpu_forced`）進入
   `poseidon_fallback.log`，保留時間戳，以便 Alertmanager 時間線可以
   與配置更改相關。

CI 今天強制執行隊列/填零檢查；一旦混合模式門落地，
`ci/check_fastpq_rollout.sh` 還將堅持任何包含以下內容的捆綁包
`poseidon_fallback.patch` 附帶匹配的 `metrics_poseidon.prom` 快照。
遵循此工作流程可以使 WP2-E.6 後備策略保持可審核性並與
與默認推出期間使用的證據收集器相同。

## 報告和自動化

- 將整個 `artifacts/fastpq_rollouts/<stamp>/` 目錄附加到
  發布票證並在推出結束後從 `status.md` 引用它。
- 運行 `dashboards/alerts/tests/fastpq_acceleration_rules.test.yml`（通過
  `promtool`) 在 CI 內，以確保與部署捆綁在一起的警報包仍然存在
  編譯。
- 使用 `ci/check_fastpq_rollout.sh` 驗證捆綁包（或
  `make check-fastpq-rollout`) 並通過 `FASTPQ_ROLLOUT_BUNDLE=<path>` 當你
  想要以單一部署為目標。 CI 通過以下方式調用相同的腳本
  `.github/workflows/fastpq-rollout.yml`，因此丟失的文物在出現之前會很快失敗
  釋放票可以關閉。發布管道可以歸檔經過驗證的包
  與簽署的清單一起通過
  `--fastpq-rollout-bundle artifacts/fastpq_rollouts/<stamp>/<fleet>/<lane>` 至
  `scripts/run_release_pipeline.py`；助手重新運行
  `ci/check_fastpq_rollout.sh`（除非設置了 `--skip-fastpq-rollout-check`）和
  將目錄樹複製到 `artifacts/releases/<version>/fastpq_rollouts/…` 中。
  作為此門的一部分，腳本強制執行 Stage7 隊列深度和零填充
  通過閱讀 `benchmarks.metal_dispatch_queue` 和
  每個 `metal` 工作台 JSON 中的 `benchmarks.zero_fill_hotspots`。

通過遵循本劇本，我們可以展示確定性採用，提供
每次部署單一證據包，並同時審核回滾演練
簽署的基準清單。