---
id: direct-mode-pack
lang: zh-hans
direction: ltr
source: docs/portal/docs/sorafs/direct-mode-pack.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraFS Direct-Mode Fallback Pack (SNNet-5a)
sidebar_label: Direct-Mode Fallback Pack
description: Required configuration, compliance checks, and rollout steps when operating SoraFS in direct Torii/QUIC mode during the SNNet-5a transition.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::注意规范来源
:::

SoraNet 电路仍然是 SoraFS 的默认传输，但路线图项目 **SNNet-5a** 需要受监管的回退，以便操作员可以在匿名推出完成时保持确定性的读取访问。该包捕获在直接 Torii/QUIC 模式下运行 SoraFS 所需的 CLI/SDK 旋钮、配置文件、合规性测试和部署清单，而无需触及隐私传输。

回退适用于暂存和受监管的生产环境，直到 SNNet-5 到 SNNet-9 清除其准备就绪大门。将下面的工件与通常的 SoraFS 部署资料一起保存，以便运营商可以根据需要在匿名模式和直接模式之间切换。

## 1. CLI 和 SDK 标志

- `sorafs_cli fetch --transport-policy=direct-only …` 禁用中继调度并强制执行 Torii/QUIC 传输。 CLI 帮助现在将 `direct-only` 列为可接受的值。
- SDK 每当公开“直接模式”切换时都必须设置 `OrchestratorConfig::with_transport_policy(TransportPolicy::DirectOnly)`。 `iroha::ClientOptions` 和 `iroha_android` 中生成的绑定转发相同的枚举。
- 网关线束（`sorafs_fetch`，Python 绑定）可以通过共享的 Norito JSON 帮助程序解析仅直接切换，以便自动化接收相同的行为。

在面向合作伙伴的运行手册中记录该标志，并通过 `iroha_config` 而不是环境变量来切换功能。

## 2. 网关策略配置文件

使用 Norito JSON 保留确定性协调器配置。 `docs/examples/sorafs_direct_mode_policy.json` 中的示例配置文件编码：

- `transport_policy: "direct_only"` — 拒绝仅宣传 SoraNet 中继传输的提供商。
- `max_providers: 2` — 将直接对等点限制为最可靠的 Torii/QUIC 端点。根据地区合规津贴进行调整。
- `telemetry_region: "regulated-eu"` — 标记发出的指标，以便遥测仪表板和审核区分后备运行。
- 保守的重试预算（`retry_budget: 2`、`provider_failure_threshold: 3`）以避免掩盖配置错误的网关。

在向操作员公开策略之前，通过 `sorafs_cli fetch --config`（自动化）或 SDK 绑定 (`config_from_json`) 加载 JSON。保留记分板输出 (`persist_path`) 以进行审计跟踪。

网关端强制旋钮在 `docs/examples/sorafs_gateway_direct_mode.toml` 中捕获。该模板镜像 `iroha app sorafs gateway direct-mode enable` 的输出，禁用信封/准入检查、连接速率限制默认值，并使用计划派生的主机名和清单摘要填充 `direct_mode` 表。在将代码片段提交到配置管理之前，将占位符值替换为您的部署计划。

## 3. 合规性测试套件

直接模式准备就绪现在包括 Orchestrator 和 CLI 包中的覆盖范围：

- 当每个候选广告仅支持SoraNet中继时，`direct_only_policy_rejects_soranet_only_providers`保证`TransportPolicy::DirectOnly`快速失败。【crates/sorafs_orchestrator/src/lib.rs:7238】
- `direct_only_policy_prefers_direct_transports_when_available` 确保存在 Torii/QUIC 传输时使用，并且 SoraNet 中继被排除在会话之外。【crates/sorafs_orchestrator/src/lib.rs:7285】
- `direct_mode_policy_example_is_valid` 解析 `docs/examples/sorafs_direct_mode_policy.json` 以确保文档与帮助程序实用程序保持一致。【crates/sorafs_orchestrator/src/lib.rs:7509】【docs/examples/sorafs_direct_mode_policy.json:1】
- `fetch_command_respects_direct_transports` 针对模拟的 Torii 网关练习 `sorafs_cli fetch --transport-policy=direct-only`，为固定直接传输的受监管环境提供烟雾测试。【crates/sorafs_car/tests/sorafs_cli.rs:2733】
- `scripts/sorafs_direct_mode_smoke.sh` 将相同的命令与策略 JSON 和记分板持久性包装在一起，以实现部署自动化。

在发布更新之前运行重点套件：

```bash
cargo test -p sorafs_orchestrator direct_only_policy
cargo test -p sorafs_car --features cli fetch_command_respects_direct_transports
```

如果工作区编译由于上游更改而失败，请在 `status.md` 中记录阻塞错误，并在依赖项赶上后重新运行。

## 4. 自动烟雾运行

仅 CLI 覆盖范围不会显示特定于环境的回归（例如，网关策略漂移或明显不匹配）。 `scripts/sorafs_direct_mode_smoke.sh` 中有一个专用的烟雾助手，并将 `sorafs_cli fetch` 与直接模式编排器策略、记分板持久性和摘要捕获封装在一起。

用法示例：

```bash
./scripts/sorafs_direct_mode_smoke.sh \
  --config docs/examples/sorafs_direct_mode_smoke.conf \
  --provider name=gw-regulated,provider-id=001122...,base-url=https://gw.example/direct/,stream-token=BASE64
```

- 该脚本尊重 CLI 标志和 key=value 配置文件（请参阅 `docs/examples/sorafs_direct_mode_smoke.conf`）。在运行之前使用生产值填充清单摘要和提供商广告条目。
- `--policy` 默认为 `docs/examples/sorafs_direct_mode_policy.json`，但可以提供由 `sorafs_orchestrator::bindings::config_to_json` 生成的任何编排器 JSON。 CLI 通过 `--orchestrator-config=PATH` 接受策略，从而无需手动调整标志即可实现可重复运行。
- 当 `sorafs_cli` 不在 `PATH` 上时，帮助程序从
  `sorafs_orchestrator` 板条箱（释放配置文件）因此烟雾运行锻炼了
  运输直接模式管道。
- 输出：
  - 组装的有效负载（`--output`，默认为 `artifacts/sorafs_direct_mode/payload.bin`）。
  - 获取包含遥测区域和用于推出证据的提供商报告的摘要（`--summary`，默认与有效负载一起）。
  - 记分板快照保留到策略 JSON 中声明的路径（例如，`fetch_state/direct_mode_scoreboard.json`）。将其与变更单中的摘要一起存档。
- 采用门自动化：获取完成后，帮助程序使用持久记分板和摘要路径调用 `cargo xtask sorafs-adoption-check`。所需的仲裁默认为命令行上提供的提供程序的数量；当您需要更大的样本时，用 `--min-providers=<n>` 覆盖它。采用报告写在摘要旁边（`--adoption-report=<path>` 可以设置自定义位置），并且每当您提供匹配的 CLI 标志时，助手都会默认传递 `--require-direct-only`（匹配后备）和 `--require-telemetry`。使用 `XTASK_SORAFS_ADOPTION_FLAGS` 转发其他 xtask 参数（例如，在批准的降级期间使用 `--allow-single-source`，以便门既容忍又强制执行回退）。仅在运行本地诊断时跳过 `--skip-adoption-check` 的采用门；该路线图要求每次受监管的直接模式运行都包含采用报告包。

## 5. 推出清单

1. **配置冻结：** 将直接模式 JSON 配置文件存储在 `iroha_config` 存储库中，并将哈希值记录在更改票证中。
2. **网关审核：** 在翻转直接模式之前确认 Torii 端点强制执行 TLS、功能 TLV 和审核日志记录。将网关策略模板发布给运营商。
3. **合规性签字：** 与合规性/监管审查人员共享更新后的剧本，并获得在匿名覆盖之外运行的批准。
4. **试运行：** 执行合规性测试套件以及针对已知良好的 Torii 提供商的暂存获取。存档记分板输出和 CLI 摘要。
5. **生产切换：** 宣布更改窗口，将 `transport_policy` 翻转为 `direct_only`（如果您选择了 `soranet-first`），并监控直接模式仪表板（`sorafs_fetch` 延迟、提供商故障计数器）。记录回滚计划，以便您可以在 SNNet-4/5/5a/5b/6a/7/8/12/13 在 `roadmap.md:532` 中毕业后首先返回到 SoraNet。
6. **变更后审核：** 将记分板快照、获取摘要和监控结果附加到变更单中。使用生效日期和任何异常情况更新 `status.md`。

将清单与 `sorafs_node_ops` 操作手册放在一起，以便操作员可以在实时切换之前排练工作流程。当 SNNet-5 升级到 GA 时，在确认生产遥测中的奇偶校验后退出后备。

## 6. 证据和收养门要求

直接模式捕获仍然需要满足 SF-6c 采用门槛。捆绑
每次运行的记分板、摘要、清单信封和采用报告
`cargo xtask sorafs-adoption-check` 可以验证回退姿势。失踪
字段迫使门失败，因此在变化中记录预期的元数据
门票。

- **传输元数据：** `scoreboard.json` 必须声明
  `transport_policy="direct_only"`（和翻转 `transport_policy_override=true`
  当您强制降级时）。保留配对的匿名策略字段
  即使它们继承了默认值，也会被填充，以便审阅者可以看到您是否
  偏离了分阶段的匿名计划。
- **提供商计数器：** 仅网关会话必须持续 `provider_count=0`
  并用 Torii 提供者的数量填充 `gateway_provider_count=<n>`
  使用过。避免手动编辑 JSON — CLI/SDK 已导出计数并
  采用门拒绝忽略分割的捕获。
- **显性证据：** Torii网关参与时，传递签名的
  `--gateway-manifest-envelope <path>`（或同等的 SDK）所以
  `gateway_manifest_provided` 加上 `gateway_manifest_id`/`gateway_manifest_cid`
  记录在 `scoreboard.json` 中。确保 `summary.json` 携带匹配
  `manifest_id`/`manifest_cid`；如果任一文件是，则采用检查失败
  缺少这对。
- **遥测期望：** 当遥测伴随捕获时，运行
  门与 `--require-telemetry` 因此采用报告证明了指标
  发出。气隙排练可以省略旗帜，但 CI 和改签
  应记录缺席情况。

示例：

```bash
cargo xtask sorafs-adoption-check \
  --scoreboard fetch_state/direct_mode_scoreboard.json \
  --summary fetch_state/direct_mode_summary.json \
  --allow-single-source \
  --require-direct-only \
  --json-out artifacts/sorafs_direct_mode/adoption_report.json \
  --require-telemetry
```将 `adoption_report.json` 附在记分板、摘要、清单旁边
信封和烟木捆。这些文物反映了 CI 采用工作的内容
(`ci/check_sorafs_orchestrator_adoption.sh`) 强制并保持直接模式
降级可审核。