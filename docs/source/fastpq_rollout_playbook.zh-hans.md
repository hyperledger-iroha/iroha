---
lang: zh-hans
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

# FASTPQ 推出手册（第 7-3 阶段）

本手册实现了 Stage7-3 路线图要求：每次机队升级
启用 FASTPQ GPU 执行必须附加可重现的基准清单，
配对的 Grafana 证据和记录的回滚演习。它补充了
`docs/source/fastpq_plan.md`（目标/架构）和
`docs/source/fastpq_migration_guide.md`（节点级升级步骤）重点
在面向操作员的部署清单上。

## 范围和角色

- **发布工程/SRE：**自己的基准捕获、清单签名和
  在批准推出之前导出仪表板。
- **Ops Guild：** 进行分阶段部署、记录回滚排练和存储
  `artifacts/fastpq_rollouts/<timestamp>/` 下的工件包。
- **治理/合规性：** 验证每次变更都有证据
  在为队列切换 FASTPQ 默认值之前请求。

## 证据包要求

每个推出提交都必须包含以下工件。附加所有文件
到发布/升级票并将捆绑包保留在
`artifacts/fastpq_rollouts/<YYYYMMDD>/<fleet>/<lane>/`。|文物 |目的|如何生产 |
|----------|---------|----------------|
| `fastpq_bench_manifest.json` |证明规范的 20000 行工作负载保持在 `<1 s` LDE 上限之下，并记录每个包装基准的哈希值。捕获 Metal/CUDA 运行，包装它们，然后运行：`cargo xtask fastpq-bench-manifest \``  --bench metal=artifacts/fastpq_benchmarks/<metal>.json \``  --bench cuda=artifacts/fastpq_benchmarks/<cuda>.json \``  --matrix artifacts/fastpq_benchmarks/matrix/matrix_manifest.json \``  --signing-key secrets/fastpq_bench.ed25519 \``  --out artifacts/fastpq_rollouts/<stamp>/fastpq_bench_manifest.json` |
|打包基准（`fastpq_metal_bench_*.json`、`fastpq_cuda_bench_*.json`）|捕获主机元数据、行使用证据、零填充热点、Poseidon 微基准摘要以及仪表板/警报使用的内核统计信息。运行 `fastpq_metal_bench` / `fastpq_cuda_bench`，然后包装原始 JSON：`python3 scripts/fastpq/wrap_benchmark.py --require-lde-mean-ms 950 --require-poseidon-mean-ms 1000 \``  --row-usage artifacts/fastpq_benchmarks/fastpq_row_usage_2025-05-12.json \``  --poseidon-metrics artifacts/fastpq_rollouts/<stamp>/<fleet>/<lane>/metrics_poseidon.prom \``  fastpq_metal_bench.json artifacts/fastpq_benchmarks/<metal>.json --sign-output`重复进行 CUDA 捕获（点相关见证/抓取文件中的 `--row-usage` 和 `--poseidon-metrics`）。帮助程序嵌入过滤后的 `fastpq_poseidon_pipeline_total`/`fastpq_execution_mode_total` 样本，因此 WP2-E.6 证据在 Metal 和 CUDA 中是相同的。当您需要独立的 Poseidon microbench 摘要（支持包装或原始输入）时，请使用 `scripts/fastpq/export_poseidon_microbench.py --bundle <bundle>`。 |
|  |  | **第 7 阶段标签要求：** `wrap_benchmark.py` 现在失败，除非生成的 `metadata.labels` 部分同时包含 `device_class` 和 `gpu_kind`。当自动检测无法推断它们时（例如，在分离的 CI 节点上进行包装时），请传递显式覆盖，例如 `--label device_class=xeon-rtx-sm80 --label gpu_kind=discrete`。 |
|  |  | **加速遥测：**默认情况下，包装器还会捕获 `cargo xtask acceleration-state --format json`，并在包装​​基准旁边写入 `<bundle>.accel.json` 和 `<bundle>.accel.prom`（使用 `--accel-*` 标志或 `--skip-acceleration-state` 覆盖）。捕获矩阵使用这些文件为车队仪表板构建 `acceleration_matrix.{json,md}`。 |
| Grafana 出口 |证明推出窗口采用遥测和警报注释。|导出 `fastpq-acceleration` 仪表板：`curl -s -H "Authorization: Bearer $GRAFANA_TOKEN" \``  "$GRAFANA_URL/api/dashboards/uid/fastpq-acceleration" \``  | jq '.dashboard' \``  > artifacts/fastpq_rollouts/<stamp>/grafana_fastpq_acceleration.json`在导出之前使用推出开始/停止时间对电路板进行注释。发布管道可以通过 `scripts/run_release_pipeline.py --export-fastpq-grafana --grafana-url <URL>`（通过 `GRAFANA_TOKEN` 提供的令牌）自动执行此操作。 |
|警报快照|捕获保护部署的警报规则。将 `dashboards/alerts/fastpq_acceleration_rules.yml`（和 `tests/` 夹具）复制到捆绑包中，以便审阅者可以重新运行 `promtool test rules …`。 |
|回滚演练日志 |证明操作员演练了强制 CPU 回退和遥测确认。|使用 [回滚练习](#rollback-drills) 中的过程并存储控制台日志 (`rollback_drill.log`) 以及生成的 Prometheus 抓取 (`metrics_rollback.prom`)。 || `row_usage/fastpq_row_usage_<date>.json` |记录 TF-5 在 CI 和仪表板中跟踪的 ExecWitness FASTPQ 行分配。从 Torii 下载新的见证，通过 `iroha_cli audit witness --decode exec.witness` 对其进行解码（可以选择添加 `--fastpq-parameter fastpq-lane-balanced` 以断言预期的参数集；默认情况下会发出 FASTPQ 批次），然后将 `row_usage` JSON 复制到 `artifacts/fastpq_rollouts/<stamp>/<fleet>/<lane>/row_usage/` 中。保留文件名时间戳，以便审阅者可以将它们与部署票证相关联，并运行 `python3 scripts/fastpq/validate_row_usage_snapshot.py row_usage/*.json`（或 `make check-fastpq-rollout`），以便 Stage7-3 门在附加证据之前验证每个批次是否公布选择器计数和 `transfer_ratio = transfer_rows / total_rows` 不变性。 |

> **提示：** `artifacts/fastpq_rollouts/README.md` 记录首选命名
> 方案 (`<stamp>/<fleet>/<lane>`) 和所需的证据文件。的
> `<stamp>` 文件夹必须编码 `YYYYMMDDThhmmZ`，以便工件保持可排序
> 无需咨询门票。

## 证据生成清单1. **捕获 GPU 基准测试。**
   - 通过以下方式运行规范工作负载（20000 个逻辑行，32768 个填充行）
     `cargo run -p fastpq_prover --bin fastpq_metal_bench -- --rows 20000 --pretty`。
   - 使用 `--row-usage <decoded witness>` 将结果包裹在 `scripts/fastpq/wrap_benchmark.py` 中，以便捆绑包中包含设备证据以及 GPU 遥测数据。通过 `--require-lde-mean-ms 950 --require-poseidon-mean-ms 1000 --sign-output`，以便在任一加速器超过目标或 Poseidon 队列/配置文件遥测丢失时包装器快速失败，并生成分离的签名。
   - 在 CUDA 主机上重复此操作，以便清单包含两个 GPU 系列。
   - **不要**剥离 `benchmarks.metal_dispatch_queue` 或
     `benchmarks.zero_fill_hotspots` 来自包装的 JSON 的块。 CI门
     (`ci/check_fastpq_rollout.sh`) 现在读取这些字段并在队列时失败
     净空下降到低于一个插槽或当任何 LDE 热点报告 `mean_ms >
     0.40ms`，自动执行 Stage7 遥测防护。
2. **生成清单。** 使用 `cargo xtask fastpq-bench-manifest …` 作为
   如表所示。将 `fastpq_bench_manifest.json` 存储在部署包中。
3. **导出 Grafana。**
   - 用卷展窗口注释 `FASTPQ Acceleration Overview` 板，
     链接到相关的 Grafana 面板 ID。
   - 通过 Grafana API（上面的命令）导出仪表板 JSON 并包含
     `annotations` 部分，以便审阅者可以将采用曲线与
     分阶段推出。
4. **快照警报。** 复制使用的确切警报规则 (`dashboards/alerts/…`)
   通过推出到捆绑包中。如果 Prometheus 规则被覆盖，包括
   覆盖差异。
5. **Prometheus/OTEL 抓取。** 从每个中捕获 `fastpq_execution_mode_total{device_class="<matrix>"}`
   主持人（台前台后）加OTEL柜台
   `fastpq.execution_mode_resolutions_total` 和配对的
   `telemetry::fastpq.execution_mode` 日志条目。这些文物证明
   GPU 的采用是稳定的，并且强制 CPU 回退仍然会发出遥测数据。
6. **存档行使用遥测。** 解码 ExecWitness 运行后
   推出时，将生成的 JSON 放在捆绑包中的 `row_usage/` 下。 CI
   帮助程序 (`ci/check_fastpq_row_usage.sh`) 将这些快照与
   规范基线，`ci/check_fastpq_rollout.sh` 现在需要每个
   捆绑发送至少一个 `row_usage` 文件以保留 TF-5 证据
   到发行票。

## 分阶段推出流程

对每个队列使用三个确定性阶段。退出后才可前进
每个阶段的标准都得到满足并记录在证据包中。|相|范围 |退出标准 |附件 |
|--------|---------|----------------|------------|
|飞行员（P1）|每个区域 1 个控制平面 + 1 个数据平面节点 | `fastpq_execution_mode_total{device_class="<matrix>", backend="metal"}` ≥90%，持续 48 小时，Alertmanager 事件为零，并通过回滚演练。 |来自两台主机的捆绑包（基准 JSON、带有试点注释的 Grafana 导出、回滚日志）。 |
|坡道 (P2) | ≥50% 的验证者加上每个集群至少一个存档通道 | GPU 执行持续 5 天，降级峰值不超过 1 次 > 10 分钟，Prometheus 计数器证明 60 秒内有回退警报。 |更新了 Grafana 导出，显示斜坡注释、Prometheus 刮擦差异、Alertmanager 屏幕截图/日志。 |
|默认（P3）|剩余节点； FASTPQ 在 `iroha_config` 中标记为默认值 |签名的工作台清单 + Grafana 导出引用最终采用曲线，并记录回滚演练演示配置切换。 |最终清单、Grafana JSON、回滚日志、配置更改审核的票证参考。 |

在推广票中记录每个促销步骤并直接链接到
`grafana_fastpq_acceleration.json` 注释，以便审阅者可以将
时间线与证据。

## 回滚练习

每个推出阶段都必须包括回滚演练：

1. 每个集群选择一个节点并记录当前指标：
   ```bash
   curl -s http://<host>:8180/metrics | rg 'fastpq_execution_mode_total{device_class'
   ```
2. 使用配置旋钮强制 CPU 模式 10 分钟
   (`zk.fastpq.execution_mode = "cpu"`) 或环境覆盖：
   ```bash
   FASTPQ_GPU=cpu irohad --config <path> --genesis-manifest-json <path>
   ```
3.确认降级日志
   (`telemetry::fastpq.execution_mode resolved="cpu" requested="gpu"`) 并刮掉
   再次访问 Prometheus 端点以显示计数器增量。
4.恢复GPU模式，验证`telemetry::fastpq.execution_mode`现在报告
   `resolved="metal"`（或 `resolved="cuda"/"opencl"` 对于非金属通道），
   确认 Prometheus 抓取包含 CPU 和 GPU 样本
   `fastpq_execution_mode_total{backend=…}`，并将经过的时间记录到
   检测/清理。
5. 将 shell 记录、指标和操作员确认存储为
   推出捆绑包中的 `rollback_drill.log` 和 `metrics_rollback.prom`。这些
   文件必须说明完整的降级+恢复周期，因为
   现在，只要日志缺少 GPU，`ci/check_fastpq_rollout.sh` 就会失败
   恢复行或指标快照忽略 CPU 或 GPU 计数器。

这些日志证明每个集群都可以正常降级，并且 SRE 团队
知道如何在 GPU 驱动程序或内核退化时确定性地回退。

## 混合模式后备证据 (WP2-E.6)

每当主机需要 GPU FFT/LDE 但需要 CPU Poseidon 哈希时（根据 Stage7 <900ms
要求），将以下工件与标准回滚日志捆绑在一起：1. **配置差异。**签入（或附加）设置的主机本地覆盖
   `zk.fastpq.poseidon_mode = "cpu"` (`FASTPQ_POSEIDON_MODE=cpu`) 离开时
   `zk.fastpq.execution_mode` 未受影响。为补丁命名
   `artifacts/fastpq_rollouts/<stamp>/<fleet>/<lane>/poseidon_fallback.patch`。
2. **波塞冬反刮。**
   ```bash
   curl -s http://<host>:8180/metrics \
     | rg 'fastpq_poseidon_pipeline_total{.*device_class="<label>"' \
     > artifacts/fastpq_rollouts/<stamp>/<fleet>/<lane>/metrics_poseidon.prom
   ```
   捕获必须显示 `path="cpu_forced"` 与
   该设备类别的 GPU FFT/LDE 计数器。恢复后进行第二次刮擦
   返回 GPU 模式，以便审阅者可以看到 `path="gpu"` 行恢复。

   将生成的文件传递给 `wrap_benchmark.py --poseidon-metrics …`，以便包装后的基准测试在其 `poseidon_metrics` 部分中记录相同的计数器；这使得 Metal 和 CUDA 的部署保持在相同的工作流程上，并且无需打开单独的抓取文件即可审核后备证据。
3. **日志摘录。** 复制 `telemetry::fastpq.poseidon` 条目以证明
   解析器翻转到CPU（`cpu_forced`）进入
   `poseidon_fallback.log`，保留时间戳，以便 Alertmanager 时间线可以
   与配置更改相关。

CI 今天强制执行队列/填零检查；一旦混合模式门落地，
`ci/check_fastpq_rollout.sh` 还将坚持任何包含以下内容的捆绑包
`poseidon_fallback.patch` 附带匹配的 `metrics_poseidon.prom` 快照。
遵循此工作流程可以使 WP2-E.6 后备策略保持可审核性并与
与默认推出期间使用的证据收集器相同。

## 报告和自动化

- 将整个 `artifacts/fastpq_rollouts/<stamp>/` 目录附加到
  发布票证并在推出结束后从 `status.md` 引用它。
- 运行 `dashboards/alerts/tests/fastpq_acceleration_rules.test.yml`（通过
  `promtool`) 在 CI 内，以确保与部署捆绑在一起的警报包仍然存在
  编译。
- 使用 `ci/check_fastpq_rollout.sh` 验证捆绑包（或
  `make check-fastpq-rollout`) 并通过 `FASTPQ_ROLLOUT_BUNDLE=<path>` 当你
  想要以单一部署为目标。 CI 通过以下方式调用相同的脚本
  `.github/workflows/fastpq-rollout.yml`，因此丢失的文物在出现之前会很快失败
  释放票可以关闭。发布管道可以归档经过验证的包
  与签署的清单一起通过
  `--fastpq-rollout-bundle artifacts/fastpq_rollouts/<stamp>/<fleet>/<lane>` 至
  `scripts/run_release_pipeline.py`；助手重新运行
  `ci/check_fastpq_rollout.sh`（除非设置了 `--skip-fastpq-rollout-check`）和
  将目录树复制到 `artifacts/releases/<version>/fastpq_rollouts/…` 中。
  作为此门的一部分，脚本强制执行 Stage7 队列深度和零填充
  通过阅读 `benchmarks.metal_dispatch_queue` 和
  每个 `metal` 工作台 JSON 中的 `benchmarks.zero_fill_hotspots`。

通过遵循本剧本，我们可以展示确定性采用，提供
每次部署单一证据包，并同时审核回滚演练
签署的基准清单。