---
lang: zh-hans
direction: ltr
source: docs/portal/docs/sdks/android-telemetry-redaction.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1c9ce5a256683152440986a028d1e57ed298bbbb189196af866128962228caa5
source_last_modified: "2026-01-05T09:28:11.847834+00:00"
translation_last_reviewed: 2026-02-07
title: Android Telemetry Redaction Plan
sidebar_label: Android Telemetry
slug: /sdks/android-telemetry
translator: machine-google-reviewed
---

:::注意规范来源
:::

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Android 遥测修订计划 (AND7)

## 范围

本文档涵盖了拟议的遥测编辑政策和启用
按照路线图项目 **AND7** 的要求，提供 Android SDK 的工件。它对齐
具有 Rust 节点基线的移动仪器，同时考虑
特定于设备的隐私保证。输出作为预读
2026 年 2 月 SRE 治理审查。

目标：

- 对每个 Android 发出的达到共享可观测性的信号进行编目
  后端（OpenTelemetry 跟踪、Norito 编码日志、指标导出）。
- 对与 Rust 基线和文档密文不同的字段进行分类或
  保留控制。
- 概述启用和测试工作，以便支持团队可以做出响应
  确定性地处理与编辑相关的警报。

## 信号清单（草案）

计划的仪器按通道分组。所有字段名称均遵循 Android
SDK 遥测架构 (`org.hyperledger.iroha.android.telemetry.*`)。可选
字段标有 `?`。

|信号ID |频道|关键领域 | PII/PHI 分类 |编辑/保留|笔记|
|------------|---------|------------|------------------------|------------------------|--------|
| `android.torii.http.request` |跟踪跨度| `authority_hash`、`route`、`status_code`、`latency_ms` |权威是公开的；路线没有秘密|导出前发出散列权限 (`blake2b_256`)；保留7天|镜子锈 `torii.http.request`；散列可确保移动别名隐私。 |
| `android.torii.http.retry` |活动 | `route`、`retry_count`、`error_code`、`backoff_ms` |无 |没有编辑；保留30天|用于确定性重试审核；与 Rust 字段相同。 |
| `android.pending_queue.depth` |规格公制| `queue_type`，`depth` |无 |没有编辑；保留 90 天 |匹配 Rust `pipeline.pending_queue_depth`。 |
| `android.keystore.attestation.result` |活动 | `alias_label`、`security_level`、`attestation_digest`、`device_brand_bucket` |别名（派生）、设备元数据 |用确定性标签替换别名，将品牌编辑为枚举存储桶 | AND2 证明准备所需； Rust 节点不发出设备元数据。 |
| `android.keystore.attestation.failure` |专柜| `alias_label`、`failure_reason` |别名编辑后无 |没有编辑；保留 90 天 |支持混沌演练； alias_label 源自散列别名。 |
| `android.telemetry.redaction.override` |活动 | `override_id`、`actor_role_masked`、`reason`、`expires_at` |演员角色符合可操作 PII |字段导出蒙面角色类别；保留 365 天审核日志 | Rust 中不存在；操作员必须通过支持提交覆盖文件。 |
| `android.telemetry.export.status` |专柜| `backend`，`status` |无 |没有编辑；保留30天|与 Rust 导出器状态计数器相同。 |
| `android.telemetry.redaction.failure` |专柜| `signal_id`，`reason` |无 |没有编辑；保留30天|需要镜像 Rust `streaming_privacy_redaction_fail_total`。 |
| `android.telemetry.device_profile` |仪表| `profile_id`、`sdk_level`、`hardware_tier` |设备元数据 |发出粗桶（SDK 主要，硬件层）；保留30天|启用奇偶校验仪表板而不暴露 OEM 细节。 |
| `android.telemetry.network_context` |活动 | `network_type`，`roaming` |运营商可能是 PII |完全删除 `carrier_name`；保留其他字段 7 天 | `ClientConfig.networkContextProvider` 提供经过清理的快照，以便应用程序可以发出网络类型+漫游，而不会暴露订户数据； Parity 仪表板将信号视为 Rust `peer_host` 的移动模拟。 |
| `android.telemetry.config.reload` |活动 | `source`、`result`、`duration_ms` |无 |没有编辑；保留30天|镜像 Rust 配置重新加载范围。 |
| `android.telemetry.chaos.scenario` |活动 | `scenario_id`、`outcome`、`duration_ms`、`device_profile` |设备配置文件已存储 |与 `device_profile` 相同；保留30天|在 AND7 准备就绪所需的混乱排练期间记录。 |
| `android.telemetry.redaction.salt_version` |仪表| `salt_epoch`，`rotation_id` |无 |没有编辑；保留 365 天 |跟踪 Blake2b 盐旋转；当 Android 哈希纪元与 Rust 节点不同时发出奇偶校验警报。 |
| `android.crash.report.capture` |活动 | `crash_id`、`signal`、`process_state`、`has_native_trace`、`anr_watchdog_bucket` |崩溃指纹+进程元数据 |哈希 `crash_id` 与共享编辑盐、存储桶看门狗状态、导出前丢弃堆栈帧；保留30天|调用`ClientConfig.Builder.enableCrashTelemetryHandler()`时自动启用；提供奇偶校验仪表板，而不暴露设备识别痕迹。 |
| `android.crash.report.upload` |专柜| `crash_id`、`backend`、`status`、`retry_count` |崩溃指纹|重用散列 `crash_id`，仅发出状态；保留30天|通过 `ClientConfig.crashTelemetryReporter()` 或 `CrashTelemetryHandler.recordUpload` 发出，因此上传与其他遥测共享相同的 Sigstore/OLTP 保证。 |

### 实施挂钩

- `ClientConfig` 现在通过线程清单导出的遥测数据
  `setTelemetryOptions(...)`/`setTelemetrySink(...)`，自动注册
  `TelemetryObserver` 因此，散列权威和盐指标流无需定制观察者。
  参见 `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/ClientConfig.java`
  以及下面的同伴类
  `java/iroha_android/src/main/java/org/hyperledger/iroha/android/telemetry/`。
- 应用程序可以调用
  `ClientConfig.Builder.enableAndroidNetworkContext(android.content.Context)` 注册
  基于反射的 `AndroidNetworkContextProvider`，在运行时查询 `ConnectivityManager`
  并发出 `android.telemetry.network_context` 事件，而不引入编译时 Android
  依赖关系。
- 单元测试 `TelemetryOptionsTests` 和 `TelemetryObserverTests`
  (`java/iroha_android/src/test/java/org/hyperledger/iroha/android/telemetry/`) 保护哈希
  帮助程序加上 ClientConfig 集成挂钩，因此明显的回归会立即浮现出来。
- 支持套件/实验室现在引用具体的 API 而不是伪代码，保留此文档并
  Runbook 与发布的 SDK 保持一致。

> **操作注意：** 所有者/状态工作表位于
> `docs/source/sdk/android/readiness/signal_inventory_worksheet.md` 并且必须是
> 在每个 AND7 检查点期间与此表一起更新。

## 奇偶校验允许列表和架构差异工作流程

治理需要双重许可名单，以便 Android 导出不会泄漏标识符
Rust 服务有意浮出水面。此部分反映了 Runbook 条目
（`docs/source/android_runbook.md` §2.3）但保留 AND7 修订计划
独立的。

|类别 | Android 出口商 |防锈服务|验证钩子 |
|----------|--------------------|------------------------|------|
|权限/路线上下文 |通过 Blake2b-256 对 `authority`/`alias` 进行哈希处理，并在导出之前删除原始 Torii 主机名；发出 `android.telemetry.redaction.salt_version` 以证明盐旋转。 |发出完整的 Torii 主机名和对等 ID 以进行关联。 |比较 `docs/source/sdk/android/readiness/schema_diffs/` 下最新架构差异中的 `android.torii.http.request` 与 `torii.http.request` 条目，然后运行 ​​`scripts/telemetry/check_redaction_status.py` 来确认盐纪元。 |
|设备和签名者身份 |存储桶 `hardware_tier`/`device_profile`、哈希控制器别名，并且从不导出序列号。 |逐字发出验证器 `peer_id`、控制器 `public_key` 和队列哈希。 |与 `docs/source/sdk/mobile_device_profile_alignment.md` 保持一致，在 `java/iroha_android/run_tests.sh` 内进行别名哈希测试，并在实验室期间存档队列检查器输出。 |
|网络元数据|仅导出 `network_type` + `roaming`；删除 `carrier_name`。 |保留对等主机名/TLS 端点元数据。 |将每个架构差异存储在 `readiness/schema_diffs/` 中，并在 Grafana 的“网络上下文”小部件显示运营商字符串时发出警报。 |
|覆盖/混乱证据|发出带有蒙面演员角色的 `android.telemetry.redaction.override`/`android.telemetry.chaos.scenario`。 |发出未屏蔽的覆盖批准；没有特定于混沌的跨度。 |演习后交叉检查 `docs/source/sdk/android/readiness/and7_operator_enablement.md`，以确保覆盖令牌 + 混乱文物与未掩盖的 Rust 事件并存。 |

工作流程：

1. 每次清单/导出器更改后，运行
   `scripts/telemetry/run_schema_diff.sh --android-config <android.json> --rust-config <rust.json>` 并将 JSON 放置在 `docs/source/sdk/android/readiness/schema_diffs/` 下。
2. 根据上表检查差异。如果 Android 发出 Rust-only 字段
   （反之亦然），提交 AND7 就绪性错误并更新此计划和
   运行手册。
3. 在每周运营审查期间，执行
   `scripts/telemetry/check_redaction_status.py --status-url https://android-telemetry-stg/api/redaction/status`
   并在准备工作表中记录盐纪元和模式差异时间戳。
4. 在 `docs/source/sdk/android/readiness/signal_inventory_worksheet.md` 中记录任何偏差
   因此治理数据包捕获奇偶校验决策。

> **架构参考：** 规范字段标识符源自
> `android_telemetry_redaction.proto`（在 Android SDK 构建期间实现）
> 与 Norito 描述符并列）。该架构公开了 `authority_hash`，
> `alias_label`、`attestation_digest`、`device_brand_bucket` 和
> `actor_role_masked` 字段现在在 SDK 和遥测导出器中使用。

`authority_hash`是记录的Torii权限值的固定32字节摘要
在原型中。 `attestation_digest` 捕获规范证明声明
指纹，而 `device_brand_bucket` 将原始 Android 品牌字符串映射到
批准的枚举（`generic`、`oem`、`enterprise`）。 `actor_role_masked`携带
密文覆盖参与者类别（`support`、`sre`、`audit`）而不是
原始用户标识符。

### 崩溃遥测导出对齐崩溃遥测现在共享相同的 OpenTelemetry 导出器和出处
管道作为Torii联网信号，关闭治理后续约
重复的出口商。崩溃处理程序为 `android.crash.report.capture` 提供数据
具有哈希 `crash_id` 的事件（Blake2b-256 已使用编辑盐
由 `android.telemetry.redaction.salt_version` 跟踪），进程状态桶，
并清理了 ANR 看门狗元数据。堆栈跟踪保留在设备上并且仅
之前总结为 `has_native_trace` 和 `anr_watchdog_bucket` 字段
导出，以便没有 PII 或 OEM 字符串离开设备。

上传崩溃会创建 `android.crash.report.upload` 计数器条目，
允许 SRE 审核后端可靠性，而无需了解任何相关信息
用户或堆栈跟踪。因为两个信号都重用了 Torii 导出器，所以它们继承
已定义相同的 Sigstore 签名、保留策略和警报挂钩
对于AND7。因此，支持运行手册可以将哈希崩溃标识符关联起来
Android 和 Rust 证据包之间没有定制的崩溃管道。

通过 `ClientConfig.Builder.enableCrashTelemetryHandler()` 启用一次处理程序
配置遥测选项和接收器；崩溃上传桥可以重用
`ClientConfig.crashTelemetryReporter()`（或 `CrashTelemetryHandler.recordUpload`）
在同一签名管道中发出后端结果。

## 策略增量与 Rust 基线

Android 和 Rust 遥测策略之间的差异以及缓解步骤。

|类别 | Rust 基线 |安卓政策 |缓解/验证|
|----------|--------------|----------------|------------------------|
|权威/同行标识符|简单的权限字符串 | `authority_hash`（Blake2b-256，旋转盐）|通过 `iroha_config.telemetry.redaction_salt` 发布的共享盐；奇偶校验测试确保支持人员的可逆映射。 |
|主机/网络元数据 |导出的节点主机名/IP |网络类型+仅限漫游 |网络运行状况仪表板更新为使用可用性类别而不是主机名。 |
|设备特点|不适用（服务器端）|分桶配置文件（SDK 21/23/29+，层 `emulator`/`consumer`/`enterprise`）|混沌演练验证桶映射；当需要更详细的细节时，支持运行手册文档升级路径。 |
|密文覆盖 |不支持 |手动覆盖令牌存储在 Norito 分类账中（`actor_role_masked`、`reason`）|覆盖需要签名请求；审核日志保留1年。 |
|鉴证痕迹 |仅通过 SRE 进行服务器认证 | SDK 发出经过净化的证明摘要 |针对 Rust 证明验证器交叉检查证明哈希值；散列别名可防止泄漏。 |

验证清单：

- 每个信号的编辑单元测试之前验证散列/屏蔽字段
  出口商提交。
- 架构差异工具（与 Rust 节点共享）每晚运行以确认字段奇偶校验。
- 混沌排练脚本练习覆盖工作流程并确认审核日志记录。

## 实施任务（SRE 之前的治理）

1. **库存确认** — 与实际 Android SDK 交叉验证上表
   检测挂钩和 Norito 模式定义。所有者： 安卓
   可观察性 TL，法学硕士。
2. **遥测模式差异** - 针对 Rust 指标运行共享差异工具以
   为 SRE 审查生成奇偶校验工件。所有者：SRE 隐私主管。
3. **运行手册草案（2026年2月3日完成）** — `docs/source/android_runbook.md`
   现在记录了端到端覆盖工作流程（第 3 节）和扩展的
   升级矩阵加上角色职责（第 3.1 节），绑定 CLI
   帮助者、事件证据和混乱脚本返回到治理策略。
   所有者：法学硕士，具有文档/支持编辑功能。
4. **启用内容** — 准备简报幻灯片、实验室说明和
   2026 年 2 月课程的知识检查问题。所有者：文档/支持
   SRE 支持团队经理。

## 支持工作流程和 Runbook 挂钩

### 1.本地+CI烟雾覆盖

- `scripts/android_sample_env.sh --telemetry --telemetry-duration=5m --telemetry-cluster=<host>` 启动 Torii 沙箱，重放规范的多源 SoraFS 固定装置（委托给 `ci/check_sorafs_orchestrator_adoption.sh`），并播种合成 Android 遥测。
  - 流量生成由 `scripts/telemetry/generate_android_load.py` 处理，它在 `artifacts/android/telemetry/load-generator.log` 下记录请求/响应转录，并支持标头、路径覆盖或试运行模式。
  - 帮助程序将 SoraFS 记分板/摘要复制到 `${WORKDIR}/sorafs/` 中，以便 AND7 排练可以在接触移动客户端之前证明多源奇偶校验。
- CI 重用相同的工具：`ci/check_android_dashboard_parity.sh` 针对 `dashboards/grafana/android_telemetry_overview.json`、Rust 参考仪表板和 `dashboards/data/android_rust_dashboard_allowances.json` 上的限额文件运行 `scripts/telemetry/compare_dashboards.py`，发出签名的差异快照 `docs/source/sdk/android/readiness/dashboard_parity/android_vs_rust-latest.json`。
- 混乱排练遵循 `docs/source/sdk/android/telemetry_chaos_checklist.md`； Sample-env 脚本加上仪表板奇偶校验形成“就绪”证据包，为 AND7 老化审计提供支持。

### 2. 覆盖发行和审计跟踪

- `scripts/android_override_tool.py` 是用于发布和撤销修订覆盖的规范 CLI。 `apply` 摄取签名请求，发出清单包（默认情况下为 `telemetry_redaction_override.to`），并将哈希令牌行附加到 `docs/source/sdk/android/telemetry_override_log.md` 中。 `revoke` 针对同一行标记撤销时间戳，`digest` 写入治理所需的经过清理的 JSON 快照。
- CLI 拒绝修改审核日志，除非存在 Markdown 表头，符合 `docs/source/android_support_playbook.md` 中捕获的合规性要求。 `scripts/tests/test_android_override_tool_cli.py` 中的单元覆盖范围保护表解析器、清单发射器和错误处理。
- 每当执行覆盖时，操作员都会在 `docs/source/sdk/android/readiness/override_logs/` 下附加生成的清单、更新的日志摘录、**和** 摘要 JSON；根据本计划中的治理决策，日志保留 365 天的历史记录。

### 3. 证据捕获和保留

- 每次排练或事件都会产生 `artifacts/android/telemetry/` 下的结构化捆绑包，其中包含：
  - 来自 `generate_android_load.py` 的负载生成器转录和聚合计数器。
  - 仪表板奇偶校验差异 (`android_vs_rust-<stamp>.json`) 和 `ci/check_android_dashboard_parity.sh` 发出的允许哈希值。
  - 覆盖日志增量（如果授予覆盖）、相应的清单和刷新的摘要 JSON。
- SRE 老化报告引用了这些工件以及 `android_sample_env.sh` 复制的 SoraFS 记分板，为 AND7 准备情况审查提供了从遥测哈希 → 仪表板 → 覆盖状态的确定性链。

## 跨 SDK 设备配置文件对齐

仪表板将 Android 的 `hardware_tier` 转换为规范
`mobile_profile_class` 定义于
`docs/source/sdk/mobile_device_profile_alignment.md` 所以 AND7 和 IOS7 遥测
比较相同的队列：

- `lab` — 发出为 `hardware_tier = emulator`，与 Swift 匹配
  `device_profile_bucket = simulator`。
- `consumer` — 发出为 `hardware_tier = consumer`（带有 SDK 主后缀）
  并与 Swift 的 `iphone_small`/`iphone_large`/`ipad` 存储桶分组。
- `enterprise` — 发出为 `hardware_tier = enterprise`，与 Swift 一致
  `mac_catalyst` 存储桶和未来的托管/iOS 桌面运行时。

任何新层都必须添加到对齐文档和架构差异工件中
在仪表板消耗它之前。

## 治理与分配

- **预读包** — 本文档加上附录工件（架构差异、
  Runbook diff、准备平台大纲）将分发给 SRE 治理
  邮件列表不晚于 **2026-02-05**。
- **反馈循环** — 在治理期间收集的意见将反馈到
  `AND7` JIRA 史诗；拦截器出现在 `status.md` 和 Android 周刊中
  站立笔记。
- **发布** — 一旦获得批准，政策摘要将链接至
  `docs/source/android_support_playbook.md` 并由共享引用
  `docs/source/telemetry.md` 中的遥测常见问题解答。

## 审计与合规说明

- 政策通过删除移动用户数据来遵守 GDPR/CCPA 要求
  出口前；哈希授权盐每季度轮换一次并存储在
  共享秘密库。
- 启用工件和运行手册更新记录在合规性注册表中。
- 季度审查确认覆盖仍然是闭环的（没有过时的访问）。

## 治理成果 (2026-02-12)

**2026-02-12** 的 SRE 治理会议批准了 Android 修订
政策无需修改。关键决策（参见
`docs/source/sdk/android/telemetry_redaction_minutes_20260212.md`）：

- **策略接受。** 哈希授权、设备配置文件存储以及
  批准省略承运人名称。盐旋转跟踪通过
  `android.telemetry.redaction.salt_version` 成为季度审核项目。
- **验证计划。** 单元/集成覆盖率、夜间架构差异运行以及
  每季度的混乱排练得到了认可。操作项：发布仪表板
  每次排练后的平价报告。
- **覆盖治理。** Norito 记录的覆盖代币已获得批准
  365 天的保留窗口。支持工程将拥有覆盖日志
  每月操作同步期间摘要审查。

## 后续状态

1. **设备配置文件对齐（截至 2026 年 3 月 1 日）。** ✅ 已完成 — 共享
   `docs/source/sdk/mobile_device_profile_alignment.md` 中的映射定义了如何
   Android `hardware_tier` 值映射到规范 `mobile_profile_class`
   由奇偶校验仪表板和架构差异工具使用。

## 即将发布的 SRE 治理简介（2026 年第 2 季度）路线图项目 **AND7** 要求下一个 SRE 治理会议收到
简洁的 Android 遥测修订预读。使用此部分作为生活
简短；在每次理事会会议之前保持更新。

### 准备清单

1. **证据包** — 导出最新的架构差异、仪表板屏幕截图、
   并覆盖日志摘要（参见下面的矩阵）并将它们放在日期下
   文件夹（例如
   `docs/source/sdk/android/readiness/and7_sre_brief/2026-02-07/`) 之前
   发送邀请。
2. **演练总结** — 附上最近的混乱演练日志以及
   `android.telemetry.redaction.failure` 指标快照；确保警报管理器
   注释引用相同的时间戳。
3. **覆盖审核** — 确认所有活动覆盖均记录在 Norito 中
   登记并在会议甲板上进行总结。包括有效期和
   相应的事件 ID。
4. **议程说明** — 在会议前 48 小时通知 SRE 主席
   简短的链接，突出显示所需的任何决策（新信号、保留
   更改或覆盖策略更新）。

### 证据矩阵

|文物|地点 |业主|笔记|
|----------|----------|--------|--------|
|架构差异与 Rust | `docs/source/sdk/android/readiness/schema_diffs/<latest>.json` |遥测工具 DRI |必须在会议前 72 小时内生成。 |
|仪表板差异截图| `docs/source/sdk/android/readiness/dashboards/<date>/` |可观察性 TL |包括 `sorafs.fetch.*`、`android.telemetry.*` 和 Alertmanager 快照。 |
|覆盖摘要 | `docs/source/sdk/android/readiness/override_logs/<date>.json` |支持工程|针对最新的 `telemetry_override_log.md` 运行 `scripts/android_override_tool.sh digest`（请参阅该目录中的 README）；令牌在共享之前保持散列状态。 |
|混沌排练日志| `artifacts/android/telemetry/chaos/<date>/log.ndjson` |质量保证自动化 |附上 KPI 摘要（停顿次数、重试率、覆盖使用情况）。 |

### 向理事会提出的开放性问题

- 既然如此，我们是否需要将覆盖保留窗口从 365 天缩短
  摘要是自动化的吗？
- `android.telemetry.device_profile`是否应该采用新的共享
  `mobile_profile_class` 标签在下一个版本中，或者等待 Swift/JS
  SDK 是否会带来相同的更改？
- Torii 后，区域数据驻留是否需要额外指导
  Norito-RPC事件登陆Android（NRPC-3后续）？

### 遥测模式差异过程

每个候选版本至少运行一次 schema diff 工具（并且每当 Android
仪器发生变化），因此 SRE 理事会会收到新的平价工件以及
仪表板差异：

1. 导出要比较的 Android 和 Rust 遥测模式。对于 CI，配置是实时的
   在 `configs/android_telemetry.json` 和 `configs/rust_telemetry.json` 下。
2. 执行`scripts/telemetry/run_schema_diff.sh --android-config configs/android_telemetry.json --rust-config configs/rust_telemetry.json --out docs/source/sdk/android/readiness/schema_diffs/<date>-android_vs_rust.json`。
   - 或者将提交 (`scripts/telemetry/run_schema_diff.sh android-main rust-main`) 传递给
     直接从 git 拉取配置；该脚本将哈希值固定在工件内。
3. 将生成的 JSON 附加到就绪包并从 `status.md` + 链接它
   `docs/source/telemetry.md`。差异突出显示添加/删除的字段和保留增量，以便
   审计人员无需重放该工具即可确认编辑奇偶校验。
4. 当差异显示允许的差异时（例如，仅限 Android 的覆盖信号），请更新
   `ci/check_android_dashboard_parity.sh` 引用的津贴文件并注意其中的基本原理
   schema-diff 目录 README。

> **存档规则：** 将最近的五个差异保留在
> `docs/source/sdk/android/readiness/schema_diffs/` 并将旧快照移至
> `artifacts/android/telemetry/schema_diffs/`，以便治理审核者始终看到最新数据。