---
lang: zh-hans
direction: ltr
source: docs/portal/docs/nexus/nexus-elastic-lane.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2ead8c13470d6e8766d8f161cdd9443ef29c72d3f87bd7aac27f179c3e1c98fb
source_last_modified: "2026-01-22T16:26:46.498151+00:00"
translation_last_reviewed: 2026-02-07
id: nexus-elastic-lane
title: Elastic lane provisioning (NX-7)
sidebar_label: Elastic Lane Provisioning
description: Bootstrap workflow for creating Nexus lane manifests, catalog entries, and rollout evidence.
translator: machine-google-reviewed
---

:::注意规范来源
此页面镜像 `docs/source/nexus_elastic_lane.md`。保持两个副本对齐，直到平移扫描落在门户中。
:::

# 弹性通道配置工具包 (NX-7)

> **路线图项目：** NX-7 — 弹性通道配置工具  
> **状态：** 工具完成 - 生成清单、目录片段、Norito 有效负载、冒烟测试、
> 负载测试包助手现在缝合时隙延迟门控+证据清单，以便验证器
> 无需定制脚本即可发布加载运行。

本指南引导操作员了解新的 `scripts/nexus_lane_bootstrap.sh` 帮助程序，该帮助程序可实现自动化
通道清单生成、通道/数据空间目录片段和部署证据。目标是使
无需手动编辑多个文件即可轻松启动新的 Nexus 通道（公共或专用）或
手动重新导出目录几何形状。

## 1.先决条件

1. 通道别名、数据空间、验证器集、容错（`f`）和结算政策的治理批准。
2. 最终的验证者列表（帐户 ID）和受保护的命名空间列表。
3. 访问节点配置存储库，以便您可以附加生成的片段。
4. 通道清单注册表的路径（请参阅 `nexus.registry.manifest_directory` 和
   `cache_directory`）。
5. 车道的遥测触点/PagerDuty 手柄，以便一旦车道到达即可发出警报
   上线。

## 2. 生成车道伪影

从存储库根运行帮助程序：

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

关键标志：

- `--lane-id` 必须与 `nexus.lane_catalog` 中新条目的索引匹配。
- `--dataspace-alias` 和 `--dataspace-id/hash` 控制数据空间目录条目（默认为
  车道 ID（省略时）。
- `--validator` 可以重复或源自 `--validators-file`。
- `--route-instruction` / `--route-account` 发出可粘贴的路由规则。
- `--metadata key=value`（或 `--telemetry-contact/channel/runbook`）捕获 Runbook 联系人，以便
  仪表板立即列出正确的所有者。
- `--allow-runtime-upgrades` + `--runtime-upgrade-*` 将运行时升级挂钩添加到清单中
  当车道需要扩展操作员控制时。
- `--encode-space-directory` 自动调用 `cargo xtask space-directory encode`。与它配对
  `--space-directory-out` 当您想要将编码的 `.to` 文件放在默认值以外的位置时。

该脚本在 `--output-dir` 内生成三个工件（默认为当前目录），
当启用编码时加上可选的第四个：

1. `<slug>.manifest.json` — 包含验证器仲裁、受保护命名空间和的通道清单
   可选的运行时升级挂钩元数据。
2. `<slug>.catalog.toml` — 包含 `[[nexus.lane_catalog]]`、`[[nexus.dataspace_catalog]]` 的 TOML 片段，
   以及任何请求的路由规则。确保在数据空间条目上设置 `fault_tolerance` 的大小
   车道接力委员会 (`3f+1`)。
3. `<slug>.summary.json` — 描述几何形状（块、段、元数据）以及
   所需的推出步骤和确切的 `cargo xtask space-directory encode` 命令（在
   `space_directory_encode.command`）。将此 JSON 附加到入职票据作为证据。
4. `<slug>.manifest.to` — 当 `--encode-space-directory` 设置时发出；准备好 Torii
   `iroha app space-directory manifest publish` 流量。

使用 `--dry-run` 预览 JSON/ 片段而不写入文件，并使用 `--force` 覆盖
现有的文物。

## 3. 应用更改

1. 将清单 JSON 复制到配置的 `nexus.registry.manifest_directory` 中（并复制到缓存中）
   目录（如果注册表镜像远程包）。如果清单的版本控制在以下版本，则提交文件
   你的配置仓库。
2. 将目录片段附加到 `config/config.toml`（或相应的 `config.d/*.toml`）。确保
   `nexus.lane_count` 至少是 `lane_id + 1`，并更新任何 `nexus.routing_policy.rules`
   应该指向新车道。
3. 编码（如果您跳过了 `--encode-space-directory`）并将清单发布到空间目录
   使用摘要中捕获的命令 (`space_directory_encode.command`)。这产生了
   `.manifest.to` 有效负载 Torii 期望并记录审计员的证据；通过提交
   `iroha app space-directory manifest publish`。
4. 运行 `irohad --sora --config path/to/config.toml --trace-config` 并将跟踪输出存档于
   推出票。这证明新的几何形状与生成的 slug/kura 段相匹配。
5. 部署清单/目录更改后，重新启动分配给通道的验证器。保留
   票证中的摘要 JSON 以供将来审核。

## 4. 构建注册表分发包

打包生成的清单和叠加层，以便操作员可以分发车道治理数据，而无需
在每台主机上编辑配置。捆绑器助手将清单复制到规范布局中，
为 `nexus.registry.cache_directory` 生成可选的治理目录覆盖，并且可以发出
离线传输的 tarball：

```bash
scripts/nexus_lane_registry_bundle.sh \
  --manifest artifacts/nexus/payments_lane/payments.manifest.json \
  --output-dir artifacts/nexus/payments_lane/registry_bundle \
  --default-module parliament \
  --module name=parliament,module_type=parliament,param.quorum=2 \
  --bundle-out artifacts/nexus/payments_lane/registry_bundle.tar.gz
```

输出：

1. `manifests/<slug>.manifest.json` — 将这些复制到配置中
   `nexus.registry.manifest_directory`。
2. `cache/governance_catalog.json` — 放入 `nexus.registry.cache_directory`。每个 `--module`
   条目成为可插入模块定义，通过以下方式启用治理模块交换（NX-2）：
   更新缓存覆盖而不是编辑 `config.toml`。
3. `summary.json` — 包括哈希值、覆盖元数据和操作员指令。
4. 可选 `registry_bundle.tar.*` — 适用于 SCP、S3 或工件跟踪器。

将整个目录（或存档）同步到每个验证器，在气隙主机上提取，然后复制
在重新启动 Torii 之前，将清单 + 缓存覆盖到其注册表路径中。

## 5. 验证器冒烟测试

Torii 重新启动后，运行新的烟雾助手来验证通道报告 `manifest_ready=true`，
指标显示了预期的车道数，并且密封的仪表是清晰的。需要清单的车道
必须公开非空 `manifest_path`；现在，当路径丢失时，助手会立即失败，因此
每个 NX-7 部署记录都包含签名的清单证据：

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

测试自签名环境时添加 `--insecure`。如果通道是，则脚本退出非零
缺失、密封或指标/遥测偏离预期值。使用
`--min-block-height`、`--max-finality-lag`、`--max-settlement-backlog` 和
`--max-headroom-events` 用于保持每通道区块高度/最终性/积压/净空遥测的旋钮
在您的操作范围内，并将它们与 `--max-slot-p95` / `--max-slot-p99` 结合起来
（加上 `--min-slot-samples`）在不离开助手的情况下强制执行 NX-18 时隙持续时间目标。

对于气隙验证（或 CI），您可以重放捕获的 Torii 响应，而不是实时响应
端点：

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

`fixtures/nexus/lanes/` 下记录的装置反映了引导程序生成的工件
帮助器，这样新的清单就可以在没有定制脚本的情况下进行检查。 CI 通过以下方式执行相同的流程
`ci/check_nexus_lane_smoke.sh` 和 `ci/check_nexus_lane_registry_bundle.sh`
（别名：`make check-nexus-lanes`）以证明 NX-7 烟雾助手与已发布的数据保持一致
有效负载格式并确保包摘要/覆盖保持可再现。

重命名通道时，捕获 `nexus.lane.topology` 遥测事件（例如，使用
`journalctl -u irohad -o json | jq 'select(.msg=="nexus.lane.topology")'`）并将它们反馈到
烟雾助手。 `--telemetry-file/--from-telemetry` 标志接受换行符分隔的日志并
`--require-alias-migration old:new` 断言 `alias_migrated` 事件记录了重命名：

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

`telemetry_alias_migrated.ndjson` 夹具捆绑了规范的重命名示例，以便 CI 可以验证
遥测解析路径，无需联系活动节点。

## 验证器负载测试（NX-7 证据）

路线图 **NX-7** 要求每个新通道都进行可重复的验证器负载运行。使用
`scripts/nexus_lane_load_test.py` 用于缝合烟雾检查、时隙持续时间门和时隙束
体现为治理可以重播的单个工件集：

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

帮助程序强制使用相同的 DA 仲裁、预言机、结算缓冲区、TEU 和时隙持续时间门
由烟雾助手编写 `smoke.log`、`slot_summary.json`、插槽捆绑清单，以及
`load_test_manifest.json` 到所选的 `--out-dir` 中，因此负载运行可以直接附加到
无需定制脚本即可推出门票。

## 6. 遥测和治理后续行动

- 使用以下内容更新车道仪表板（`dashboards/grafana/nexus_lanes.json` 和相关覆盖图）
  新的车道 ID 和元数据。生成的元数据密钥（`contact`、`channel`、`runbook` 等）
  预填充标签很简单。
- 在启用准入之前，为新车道发送 PagerDuty/Alertmanager 规则。 `summary.json`
  next-steps 数组镜像 [Nexus 操作](./nexus-operations) 中的清单。
- 验证器集上线后，在空间目录中注册清单包。使用相同的
  由帮助程序生成的清单 JSON，根据治理运行手册进行签名。
- 按照 [Sora Nexus 操作员入门](./nexus-operator-onboarding) 进行冒烟测试（FindNetworkStatus、Torii
  可达性）并使用上面生成的工件集捕获证据。

## 7. 试运行示例

要预览工件而不写入文件：

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

该命令将 JSON 摘要和 TOML 片段打印到标准输出，从而允许在
规划。

---

有关其他上下文，请参阅：- [Nexus 操作](./nexus-operations) — 操作清单和遥测要求。
- [Sora Nexus 操作员入门](./nexus-operator-onboarding) — 参考了详细的入门流程
  新帮手。
- [Nexus 通道模型](./nexus-lane-model) — 工具使用的通道几何形状、段块和存储布局。