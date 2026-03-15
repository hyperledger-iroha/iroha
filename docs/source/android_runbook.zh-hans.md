---
lang: zh-hans
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

# Android SDK 操作手册

此运行手册支持操作员和支持工程师管理 Android SDK
AND7 及更高版本的部署。与 SLA 的 Android 支持手册配对
定义和升级路径。

> **注意：** 更新事件过程时，同时刷新共享的
> 故障排除矩阵 (`docs/source/sdk/android/troubleshooting.md`)
> 场景表、SLA 和遥测参考与本运行手册保持一致。

## 0. 快速入门（寻呼机触发时）

在深入了解详细信息之前，请将此序列放在方便的 Sev1/Sev2 警报中
以下部分：

1. **确认活动配置：** 捕获 `ClientConfig` 清单校验和
   在应用程序启动时发出并将其与固定清单进行比较
   `configs/android_client_manifest.json`。如果哈希值出现分歧，则停止发布并
   在接触遥测/覆盖之前提交配置漂移票证（请参阅§1）。
2. **运行架构差异门：** 针对以下对象执行 `telemetry-schema-diff` CLI
   接受的快照
   （`docs/source/sdk/android/readiness/schema_diffs/android_vs_rust-20260305.json`）。
   将任何 `policy_violations` 输出视为 Sev2 并阻止导出，直到
   差异是可以理解的（参见§2.6）。
3. **检查仪表板 + 状态 CLI：** 打开 Android Telemetry Redaction 并
   出口商健康委员会，然后运行
   `scripts/telemetry/check_redaction_status.py --status-url <collector>`。如果
   当局在地板下或导出错误，捕获屏幕截图和
   事件文档的 CLI 输出（请参阅§2.4–§2.5）。
4. **决定覆盖：** 仅在上述步骤之后并与事件/所有者一起
   记录，通过 `scripts/android_override_tool.sh` 发出有限覆盖
   并将其记录在 `telemetry_override_log.md` 中（参见第 3 节）。默认有效期：<24 小时。
5. **按联系人列表升级：** 分页 Android 待命和可观察性 TL
   （第 8 节中的联系方式），然后遵循第 4.1 节中的升级树。如果证明或
   涉及 StrongBox 信号，拉动最新的捆绑包并运行线束
   在重新启用导出之前检查第 7 节。

## 1. 配置与部署

- **ClientConfig 来源：** 确保 Android 客户端加载 Torii 端点、TLS
  策略，以及来自 `iroha_config` 派生清单的重试旋钮。验证
  应用程序启动期间的值以及活动清单的日志校验和。
  实现参考：`java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/ClientConfig.java`
  来自 `java/iroha_android/src/main/java/org/hyperledger/iroha/android/telemetry/TelemetryOptions.java` 的线程 `TelemetryOptions`
  （加上生成的 `TelemetryObserver`），因此散列权限会自动发出。
- **热重载：** 使用配置观察器拾取 `iroha_config`
  更新无需重新启动应用程序。失败的重新加载应该发出
  `android.telemetry.config.reload` 事件和触发指数重试
  退避（最多 5 次尝试）。
- **回退行为：** 当配置丢失或无效时，回退到
  安全默认值（只读模式，无待处理队列提交）并显示用户
  提示。记录事件以便后续处理。

### 1.1 配置重新加载诊断- 配置观察器发出 `android.telemetry.config.reload` 信号
  `source`、`result`、`duration_ms` 和可选 `digest`/`error` 字段（请参阅
  `configs/android_telemetry.json` 和
  `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/ConfigWatcher.java`）。
  每个应用的清单预计有一个 `result:"success"` 事件；重复
  `result:"error"` 记录表明观察者已用尽 5 次退避尝试
  从 50 毫秒开始。
- 发生事件期间，从收集器捕获最新的重新加载信号
  （OTLP/span 存储或编辑状态端点）并记录 `digest` +
  事件文档中的 `source`。将摘要与
  `configs/android_client_manifest.json` 和发布清单分发到
  运营商。
- 如果观察者继续发出错误，请运行目标工具来重现
  可疑清单解析失败：
  `ci/run_android_tests.sh org.hyperledger.iroha.android.client.ConfigWatcherTests`。
  将测试输出和失败清单附加到事件包中，以便 SRE
  可以将其与烘焙的配置模式进行比较。
- 当重新加载遥测丢失时，确认活动的 `ClientConfig` 带有
  遥测接收器并且 OTLP 收集器仍然接受
  `android.telemetry.config.reload` ID；否则将其视为 Sev2 遥测
  回归（与§2.4 相同的路径）并暂停释放，直到信号返回。

### 1.2 确定性密钥导出包
- 软件导出现在发出带有每个导出盐 + 随机数、`kdf_kind` 和 `kdf_work_factor` 的 v3 捆绑包。
  导出器更喜欢 Argon2id（64 MiB，3 次迭代，并行度 = 2）并回退到
  当 Argon2id 在设备上不可用时，PBKDF2-HMAC-SHA256 具有 350 k 迭代层。捆绑包
  AAD 仍然绑定到别名；对于 v3 导出，密码必须至少为 12 个字符，并且
  进口商拒绝全零盐/随机数种子。
  `KeyExportBundle.decode(Base64|bytes)`，使用原始密码导入，然后重新导出到 v3
  转向内存硬格式。进口商拒绝全零或重复使用的盐/随机数对；总是
  轮换捆绑包，而不是在设备之间重复使用旧的导出。
- `ci/run_android_tests.sh --tests org.hyperledger.iroha.android.crypto.export.DeterministicKeyExporterTests` 中的负路径测试
  拒绝。使用后清除密码字符数组并捕获捆绑版本和 `kdf_kind`
  在恢复失败时的事件注释中。

## 2. 遥测和编辑

> 快速参考：参见
> [`telemetry_redaction_quick_reference.md`](sdk/android/telemetry_redaction_quick_reference.md)
> 用于启用期间使用的精简命令/阈值清单
> 会话和事件桥梁。- **信号清单：** 参考 `docs/source/sdk/android/telemetry_redaction.md`
  有关发出的跨度/指标/事件的完整列表以及
  `docs/source/sdk/android/readiness/signal_inventory_worksheet.md`
  了解所有者/验证详细信息和突出的差距。
- **规范模式差异：** 批准的 AND7 快照是
  `docs/source/sdk/android/readiness/schema_diffs/android_vs_rust-20260305.json`。
  每个新的 CLI 运行都必须与此工件进行比较，以便审阅者可以看到
  接受的 `intentional_differences` 和 `android_only_signals` 仍然
  与中记录的策略表相匹配
  `docs/source/sdk/android/telemetry_schema_diff.md` §3。 CLI 现在添加了
  `policy_violations` 当任何有意的差异丢失时
  `status:"accepted"`/`"policy_allowlisted"`（或者当仅限 Android 的记录丢失时
  他们接受的状态），因此将非空违规视为 Sev2 并停止
  出口。下面的 `jq` 片段保留作为对存档的手动健全性检查
  文物：
  ```bash
  jq '.intentional_differences[] | select(.status != "accepted" and .status != "policy_allowlisted")' "$OUT"
  jq '.android_only_signals[] | select(.status != "accepted")' "$OUT"
  jq '.field_mismatches[] | {signal, field, android, rust}' "$OUT"
  ```
  将这些命令的任何输出视为需要一个模式回归
  遥测导出继续之前的 AND7 就绪错误； `field_mismatches`
  根据 `telemetry_schema_diff.md` §5，必须保持为空。助手现在写道
  自动`artifacts/android/telemetry/schema_diff.prom`；通过
  `--textfile-dir /var/lib/node_exporter/textfile_collector`（或设置
  `ANDROID_SCHEMA_DIFF_TEXTFILE_DIR`) 在临时/生产主机上运行时
  因此 `telemetry_schema_diff_run_status` 仪表翻转为 `policy_violation`
  如果 CLI 检测到漂移，则会自动进行。
- **CLI 帮助程序：** `scripts/telemetry/check_redaction_status.py` 检查
  默认为`artifacts/android/telemetry/status.json`；将 `--status-url` 传递给
  查询暂存和 `--write-cache` 刷新本地副本以供离线使用
  演习。使用 `--min-hashed 214`（或设置
  `ANDROID_TELEMETRY_MIN_HASHED_AUTHORITIES=214`) 强制执行治理
  在每次状态民意调查中，对散列权威的底线。
- **权限哈希：** 所有权限都使用 Blake2b-256 进行哈希处理
  每季度轮换盐存储在安全秘密库中。旋转发生在
  每个季度的第一个星期一 00:00 UTC。核实出口商提货
  通过检查 `android.telemetry.redaction.salt_version` 指标来确定新的盐。
- **设备配置文件存储桶：** 仅 `emulator`、`consumer` 和 `enterprise`
  层被导出（与 SDK 主要版本一起）。仪表板比较这些
  根据 Rust 基线进行计数；方差 >10% 会引发警报。
- **网络元数据：** Android 仅导出 `network_type` 和 `roaming` 标志。
  运营商名称永远不会被公开；运营商不应要求订户
  事件日志中的信息。清理后的快照作为
  `android.telemetry.network_context` 事件，因此请确保应用程序注册
  `NetworkContextProvider`（通过
  `ClientConfig.Builder.setNetworkContextProvider(...)` 还是方便
  `enableAndroidNetworkContext(...)` 帮助程序）在发出 Torii 调用之前。
- **Grafana 指针：** `Android Telemetry Redaction` 仪表板是
  对上述 CLI 输出进行规范的目视检查 — 确认
  `android.telemetry.redaction.salt_version`面板与当前盐纪元匹配
  并且 `android_telemetry_override_tokens_active` 小部件保持为零
  每当没有演习或事件发生时。如果任一面板出现偏差则升级
  在 CLI 脚本报告回归之前。

### 2.1 导出管道工作流程1. **配置分配。** `ClientConfig.telemetry.redaction` 是从线程
   `iroha_config` 并由 `ConfigWatcher` 热重载。每次重新加载都会记录
   明显的消化加盐时代——捕捉事件中和期间的那条线
   排练。
2. **Instrumentation.** SDK 组件将 span/metrics/events 发送到
   `TelemetryBuffer`。缓冲区用设备配置文件标记每个有效负载，并且
   当前的盐纪元，以便导出器可以确定性地验证哈希输入。
3. **编辑过滤器。** `RedactionFilter` 哈希值 `authority`、`alias` 和
   设备标识符在离开设备之前。故障发出
   `android.telemetry.redaction.failure` 并阻止导出尝试。
4. **导出器 + 收集器。** 净化后的有效负载通过 Android 传送
   OpenTelemetry 导出器到 `android-otel-collector` 部署。的
   收集器风扇输出到轨迹 (Tempo)、指标 (Prometheus) 和 Norito
   日志下沉。
5. **可观察性挂钩。** `scripts/telemetry/check_redaction_status.py` 读取
   收集器计数器（`android.telemetry.export.status`，
   `android.telemetry.redaction.salt_version`) 并生成状态包
   本运行手册中均引用了该内容。

### 2.2 验证门

- **架构差异：** 运行
  `scripts/telemetry/run_schema_diff.sh --android-config configs/android_telemetry.json --rust-config configs/rust_telemetry.json`
  每当出现变化时。每次运行后，确认每个
  `intentional_differences[*]` 和 `android_only_signals[*]` 条目带有标记
  `status:"accepted"`（或 `status:"policy_allowlisted"` 用于散列/分桶
  字段）按照 `telemetry_schema_diff.md` §3 中的建议，然后再附加
  事件和混乱实验室报告的人工制品。使用批准的快照
  (`android_vs_rust-20260305.json`) 作为护栏和棉绒新鲜排放
  提交之前的 JSON：
  ```bash
  LATEST=docs/source/sdk/android/readiness/schema_diffs/$(date -u +%Y%m%d).json
  jq '.intentional_differences[] | select(.status != "accepted" and .status != "policy_allowlisted") | {signal, field, status}' "$LATEST"
  jq '.android_only_signals[] | select(.status != "accepted") | {signal, status}' "$LATEST"
  ```
  将 `$LATEST` 与
  `docs/source/sdk/android/readiness/schema_diffs/android_vs_rust-20260305.json`
  证明允许名单保持不变。 `status` 缺失或空白
  条目（例如 `android.telemetry.redaction.failure` 或
  `android.telemetry.redaction.salt_version`) 现在被视为回归
  必须在审核结束前得到解决； CLI 显示接受的
  直接说明，因此手册§3.4交叉引用仅适用于
  解释为什么出现非 `accepted` 状态。

  **规范 AND7 信号（2026 年 3 月 5 日快照）**|信号|频道|状态 |治理说明|验证钩子 |
  |--------|---------|--------|-----------------|-----------------|
  | `android.telemetry.redaction.override` |活动 | `accepted` |镜像会覆盖清单并且必须与 `telemetry_override_log.md` 条目匹配。 |观看 `android_telemetry_override_tokens_active` 并按照 §3 归档清单。 |
  | `android.telemetry.network_context` |活动 | `accepted` | Android 有意修改运营商名称；仅导出 `network_type` 和 `roaming`。 |确保应用程序注册 `NetworkContextProvider` 并确认事件量遵循 `Android Telemetry Overview` 上的 Torii 流量。 |
  | `android.telemetry.redaction.failure` |专柜| `accepted` |每当散列失败时发出；治理现在需要模式差异工件中的显式状态元数据。 | `Redaction Compliance` 仪表板面板和 `check_redaction_status.py` 的 CLI 输出必须保持为零（演习期间除外）。 |
  | `android.telemetry.redaction.salt_version` |仪表| `accepted` |证明出口商正在使用当前的季度盐纪元。 |将 Grafana 的 salt 小部件与 Secrets-Vault 纪元进行比较，并确保架构差异运行保留 `status:"accepted"` 注释。 |

  如果上表中的任何条目删除了 `status`，则差异伪影必须是
  重新生成**和** `telemetry_schema_diff.md` 在 AND7 之前更新
  治理数据包已分发。将刷新的 JSON 包含在
  `docs/source/sdk/android/readiness/schema_diffs/` 并从
  触发重新运行的事件、混沌实验室或启用报告。
- **CI/单位覆盖范围：** `ci/run_android_tests.sh` 必须先通过
  发布版本；该套件通过执行来强制散列/覆盖行为
  具有示例有效负载的遥测导出器。
- **喷油器健全性检查：** 使用
  `scripts/telemetry/inject_redaction_failure.sh --dry-run` 排练前
  确认故障注入有效，并在散列防护时发出警报
  被绊倒。验证后始终使用 `--clear` 清除喷油器
  完成。

### 2.3 移动 ↔ Rust 遥测奇偶校验清单

保持 Android 导出器和 Rust 节点服务保持一致，同时尊重
不同的编辑要求记录在
`docs/source/sdk/android/telemetry_redaction.md`。下表作为
AND7 路线图条目中引用的双重允许列表 - 每当
模式差异引入或删除字段。|类别 | Android 出口商 |防锈服务|验证钩子 |
|----------|--------------------|------------------------|------|
|权限/路线上下文|通过 Blake2b-256 对 `authority`/`alias` 进行哈希处理，并在导出之前删除原始 Torii 主机名；发出 `android.telemetry.redaction.salt_version` 以证明盐旋转。 |发出完整的 Torii 主机名和对等 ID 以进行关联。 |在 `readiness/schema_diffs/` 下的最新架构差异中比较 `android.torii.http.request` 与 `torii.http.request` 条目，然后通过运行 `scripts/telemetry/check_redaction_status.py` 确认 `android.telemetry.redaction.salt_version` 与集群盐匹配。 |
|设备和签名者身份 |存储桶 `hardware_tier`/`device_profile`、哈希控制器别名，并且从不导出序列号。 |无设备元数据；节点逐字发出验证器 `peer_id` 和控制器 `public_key`。 |镜像 `docs/source/sdk/mobile_device_profile_alignment.md` 中的映射，在实验室期间审核 `PendingQueueInspector` 输出，并确保 `ci/run_android_tests.sh` 内的别名哈希测试保持绿色。 |
|网络元数据|仅导出 `network_type` + `roaming` 布尔值； `carrier_name` 已删除。 | Rust 保留对等主机名以及完整的 TLS 端点元数据。 |将最新的 diff JSON 存储在 `readiness/schema_diffs/` 中，并确认 Android 端仍然省略 `carrier_name`。如果 Grafana 的“网络上下文”小部件显示任何运营商字符串，则会发出警报。 |
|覆盖/混乱证据|使用蒙面演员角色发出 `android.telemetry.redaction.override` 和 `android.telemetry.chaos.scenario` 事件。 | Rust 服务发出覆盖批准，无需角色屏蔽，也没有特定于混沌的跨度。 |每次演练后交叉检查 `docs/source/sdk/android/readiness/and7_operator_enablement.md`，以确保覆盖令牌和混乱工件与未屏蔽的 Rust 事件一起存档。 |

奇偶校验工作流程：

1. 每次清单或导出器更改后，运行
   `scripts/telemetry/run_schema_diff.sh --android-config configs/android_telemetry.json --rust-config configs/rust_telemetry.json --out docs/source/sdk/android/readiness/schema_diffs/$(date -u +%Y%m%d).json --textfile-dir /var/lib/node_exporter/textfile_collector`
   因此 JSON 工件和镜像指标都位于证据包中
   （助手默认情况下仍然写入 `artifacts/android/telemetry/schema_diff.prom`）。
2. 对照上表查看差异；如果 Android 现在发出一个字段
   仅允许在 Rust 上（反之亦然），提交 AND7 就绪性错误并更新
   修订计划。
3. 在每周检查期间，运行
   `scripts/telemetry/check_redaction_status.py --status-url https://android-telemetry-stg/api/redaction/status`
   确认盐纪元与 Grafana 小部件匹配并记下中的纪元
   待命日记。
4. 记录所有增量
   `docs/source/sdk/android/readiness/signal_inventory_worksheet.md`所以
   治理可以审核平等决策。

### 2.4 可观测性仪表板和警报阈值

使仪表板和警报与 AND7 架构差异批准保持一致
查看 `scripts/telemetry/check_redaction_status.py` 输出：

- `Android Telemetry Redaction` — 盐纪元小部件，覆盖代币计量器。
- `Redaction Compliance` — `android.telemetry.redaction.failure` 计数器和
  喷油器趋势面板。
- `Exporter Health` — `android.telemetry.export.status` 速率细分。
- `Android Telemetry Overview` — 设备配置文件存储桶和网络上下文卷。

以下阈值反映了快速参考卡，必须强制执行
在事件响应和排练期间：|公制/面板|门槛|行动|
|----------------|---------|--------|
| `android.telemetry.redaction.failure`（`Redaction Compliance`板）| >0 在滚动 15 分钟窗口内 |调查故障信号，运行清除注入器，记录 CLI 输出 + Grafana 屏幕截图。 |
| `android.telemetry.redaction.salt_version`（`Android Telemetry Redaction`板）|与秘密库盐时代的不同|停止发布，与秘密轮换协调，归档 AND7 注释。 |
| `android.telemetry.export.status{status="error"}`（`Exporter Health`板）| > 出口额的 1% |检查收集器运行状况、捕获 CLI 诊断、升级到 SRE。 |
| `android.telemetry.device_profile{tier="enterprise"}` 与 Rust 奇偶校验 (`Android Telemetry Overview`) |与 Rust 基线的差异 >10% |文件治理跟进，验证夹具池，注释模式差异工件。 |
| `android.telemetry.network_context` 卷 (`Android Telemetry Overview`) |当 Torii 流量存在时，流量降至零 |确认 `NetworkContextProvider` 注册，重新运行 schema diff 以确保字段不变。 |
| `android.telemetry.redaction.override` / `android_telemetry_override_tokens_active` (`Android Telemetry Redaction`) |非零外部批准覆盖/钻取窗口 |将令牌与事件联系起来，重新生成摘要，并通过第 3 节中的工作流程进行撤销。 |

### 2.5 操作员准备和支持跟踪

路线图项目 AND7 提出了专门的操作员课程，以便支持、SRE 和
发布干系人在运行手册发布之前了解上面的奇偶校验表
GA。使用中的轮廓
`docs/source/sdk/android/telemetry_readiness_outline.md` 用于规范物流
（议程、演讲者、时间表）和 `docs/source/sdk/android/readiness/and7_operator_enablement.md`
查看详细的清单、证据链接和操作日志。保留以下内容
每当遥测计划发生变化时，阶段都会同步：|相|描述 |证据包|主要业主 |
|--------|-------------|--------------------|------------------------|
|预读发行|在简报会前至少五个工作日发送保单预读 `telemetry_redaction.md` 和快速参考卡。跟踪大纲通信日志中的确认。 | `docs/source/sdk/android/telemetry_readiness_outline.md`（会话物流 + 通信日志）和 `docs/source/sdk/android/readiness/archive/<YYYY-MM>/` 中的存档电子邮件。 |文档/支持经理 |
|现场准备会议|提供 60 分钟的培训（政策深入探讨、运行手册演练、仪表板、混沌实验室演示）并为异步观看者保持录制运行。 |录音 + 幻灯片存储在 `docs/source/sdk/android/readiness/archive/<YYYY-MM>/` 下，并在大纲的第 2 节中捕获参考文献。 |法学硕士（AND7 代理所有者）|
|混沌实验室执行|在实时会话结束后立即从 `docs/source/sdk/android/readiness/labs/telemetry_lab_01.md` 运行至少 C2（覆盖）+ C6（队列重播），并将日志/屏幕截图附加到支持套件。 | `docs/source/sdk/android/readiness/labs/reports/<YYYY-MM>/` 和 `/screenshots/<YYYY-MM>/` 内的场景报告和屏幕截图。 | Android 可观测性 TL + SRE 待命 |
|知识检查与考勤|收集提交的测验，纠正得分低于 90% 的人，并记录出勤/测验统计数据。让快速参考问题与奇偶校验清单保持一致。 |测验在 `docs/source/sdk/android/readiness/forms/responses/` 中导出，摘要 Markdown/JSON 通过 `scripts/telemetry/generate_and7_quiz_summary.py` 生成，以及 `and7_operator_enablement.md` 中的考勤表。 |支持工程|
|存档及后续|更新启用工具包的操作日志，将工件上传到存档，并在 `status.md` 中记下完成情况。会话期间发出的任何修复或覆盖令牌都必须复制到 `telemetry_override_log.md` 中。 | `docs/source/sdk/android/readiness/and7_operator_enablement.md` §6（操作日志）、`.../archive/<YYYY-MM>/checklist.md` 以及§3 中引用的覆盖日志。 |法学硕士（AND7 代理所有者）|

当课程重新运行时（每季度或在主要架构更改之前），刷新
包含新会议日期的大纲，保持与会者名单最新，以及
重新生成测验摘要 JSON/Markdown 工件，以便治理数据包可以
参考一致的证据。 AND7 的 `status.md` 条目应链接到
每个启用冲刺结束后的最新存档文件夹。

### 2.6 架构差异允许列表和策略检查

该路线图明确提出了双重白名单政策（移动编辑与
防锈保留）由 `telemetry-schema-diff` CLI 强制执行，位于
`tools/telemetry-schema-diff`。每个差异工件记录在
`docs/source/sdk/android/readiness/schema_diffs/` 必须记录哪些字段
Android 上已散列/分桶，哪些字段在 Rust 上保持未散列，以及是否
任何非允许的信号都会溜进构建中。记录这些决定
直接在 JSON 中运行：

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
```当报告干净时，最终的 `jq` 评估为无操作。处理任何输出
从该命令作为 Sev2 就绪性错误：填充的 `policy_violations`
array 表示 CLI 发现了一个不在仅限 Android 的列表中的信号
也不在仅 Rust 的豁免列表中记录的
`docs/source/sdk/android/telemetry_schema_diff.md`。出现这种情况时，请停止
导出，提交 AND7 票证，并仅在策略模块之后重新运行 diff
清单快照已得到更正。将生成的 JSON 存储在
`docs/source/sdk/android/readiness/schema_diffs/` 带有日期后缀和注释
事件或实验室报告内的路径，以便治理可以重播检查。

**散列和保留矩阵**

|信号场 |安卓处理 |防锈处理 |允许列表标签 |
|--------------|-----------------|----------------|---------------|
| `torii.http.request.authority` | Blake2b-256 散列 (`representation: "blake2b_256"`) |逐字存储以实现可追溯性 | `policy_allowlisted`（移动哈希）|
| `attestation.result.alias` | Blake2b-256 散列 |纯文本别名（证明档案）| `policy_allowlisted` |
| `attestation.result.device_tier` |桶装 (`representation: "bucketed"`) |普通层字符串 | `policy_allowlisted` |
| `hardware.profile.hardware_tier` |缺席——Android出口商完全放弃这个领域不加编辑地呈现 | `rust_only`（记录在 `telemetry_schema_diff.md` 的第 3 节中）|
| `android.telemetry.redaction.override.*` |仅限 Android 的信号，带有蒙面演员角色 |没有发出等效信号 | `android_only`（必须保留 `status:"accepted"`）|

当出现新信号时，将它们添加到架构差异策略模块**和**
因此 Runbook 反映了 CLI 中提供的强制逻辑。
如果任何仅限 Android 的信号省略显式 `status` 或如果
`policy_violations` 数组非空，因此请保持此清单与
`telemetry_schema_diff.md` §3 以及中引用的最新 JSON 快照
`telemetry_redaction_minutes_*.md`。

## 3. 覆盖工作流程

在散列回归或隐私时，覆盖是“打破玻璃”的选项
警报会阻止客户。仅在记录完整决策轨迹后应用它们
在事件文档中。1. **确认偏差和范围。** 等待 PagerDuty 警报或架构差异
   门开火，然后跑
   `scripts/telemetry/check_redaction_status.py --status-url <collector>` 至
   证明当局不匹配。附上 CLI 输出和 Grafana 屏幕截图
   到事件记录。
2. **准备签名请求。** 填充
   `docs/examples/android_override_request.json` 以及票证 ID、请求者、
   到期日和理由。将文件存储在事件文物旁边，以便
   合规性可以审核输入。
3. **发出覆盖。** 调用
   ```bash
   scripts/android_override_tool.sh apply \
     --request docs/examples/android_override_request.json \
     --log docs/source/sdk/android/telemetry_override_log.md \
     --out artifacts/android/telemetry/override-$(date -u +%Y%m%dT%H%M%SZ).json \
     --event-log docs/source/sdk/android/readiness/override_logs/override_events.ndjson \
     --actor-role <support|sre|docs|compliance|program|other>
   ```
   帮助程序打印覆盖令牌，写入清单，并附加一行
   到 Markdown 审核日志。切勿在聊天中发布令牌；直接交付
   到应用覆盖的 Torii 操作员。
4. **监控效果。** 五分钟内验证单个
   `android.telemetry.redaction.override` 事件已发出，收集器
   状态端点显示 `override_active=true`，事件文档列出了
   到期。观看 Android 遥测概述仪表板的“覆盖令牌
   活动”面板（`android_telemetry_override_tokens_active`）相同
   令牌计数并继续每 10 分钟运行一次状态 CLI，直到
   散列稳定。
5. **撤销并存档。** 缓解措施生效后，立即运行
  `scripts/android_override_tool.sh revoke --token <token>` 所以审核日志
  捕获撤销时间，然后执行
  `scripts/android_override_tool.sh digest --out docs/source/sdk/android/readiness/override_logs/override_digest_$(date -u +%Y%m%dT%H%M%SZ).json`
  刷新治理期望的经过清理的快照。附上
  清单、摘要 JSON、CLI 转录本、Grafana 快照和 NDJSON 日志
  通过 `--event-log` 生成
  `docs/source/sdk/android/readiness/screenshots/<date>/` 并交联
  来自 `docs/source/sdk/android/telemetry_override_log.md` 的条目。

超过 24 小时的覆盖需要 SRE 总监和合规部批准，并且
必须在下周 AND7 审核中重点强调。

### 3.1 覆盖升级矩阵

|情况|最长持续时间|批准人 |所需通知 |
|------------|--------------|------------------------|------------------------|
|单租户调查（散列权限不匹配，客户 Sev2）| 4小时|支持工程师+SRE on-call |票证 `SUP-OVR-<id>`、`android.telemetry.redaction.override` 事件、事件日志 |
|整个舰队的遥测中断或 SRE 请求的复制 | 24小时 | SRE 随叫随到 + 项目主管 | PagerDuty 注释，覆盖日志条目，在 `status.md` 中更新 |
|合规/取证请求或任何超过 24 小时的案件 |直到明确撤销| SRE 总监 + 合规主管 |治理邮件列表、覆盖日志、AND7 每周状态 |

#### 角色职责|角色 |职责| SLA / 注释 |
|------|--------------------|-------------|
| Android 遥测待命（事件指挥官）|驱动检测、执行覆盖工具、在事件文档中记录批准，并确保在到期之前进行撤销。 |在 5 分钟内确认 PagerDuty，并每 15 分钟记录一次进度。 |
| Android 可观测性 TL（Haruka Yamamoto）|验证漂移信号，确认出口商/收集商状态，并在将其交给操作员之前在覆盖清单上签字。 | 10分钟内上桥；如果不可用，则委托给暂存集群所有者。 |
| SRE 联络员 (Liam O’Connor) |将清单应用于收集器、监控待办事项并与发布工程协调以实现 Torii 端缓解措施。 |记录更改请求中的每个 `kubectl` 操作，并将命令记录粘贴到事件文档中。 |
|合规性（索菲亚·马丁斯/丹尼尔·帕克）|批准超过 30 分钟的覆盖，验证审核日志行，并就监管机构/客户消息传递提供建议。 |在 `#compliance-alerts` 中发布确认；对于生产事件，在发布覆盖之前提交合规说明。 |
|文档/支持经理 (Priya Deshpande) |将清单/CLI 输出存档在 `docs/source/sdk/android/readiness/…` 下，保持覆盖日志整洁，并在出现差距时安排后续实验。 |在结束事件之前确认证据保留（13 个月）并提交 AND7 后续行动。 |

如果任何覆盖令牌即将到期且没有任何更新，请立即升级
记录撤销计划。

## 4. 事件响应

- **警报：** PagerDuty 服务 `android-telemetry-primary` 涵盖密文
  故障、出口机停机和铲斗漂移。在 SLA 窗口内确认
  （请参阅支持手册）。
- **诊断：** 运行 `scripts/telemetry/check_redaction_status.py` 进行收集
  当前出口商的健康状况、最近的警报和散列权威指标。包括
  事件时间线中的输出 (`incident/YYYY-MM-DD-android-telemetry.md`)。
- **仪表板：** 监控 Android 遥测编辑、Android 遥测
  概述、修订合规性和出口商健康状况仪表板。捕获
  事件记录的屏幕截图并注释任何盐版本或覆盖
  结束事件之前的令牌偏差。
- **协调：** 参与发布工程以解决出口商问题、合规性
  覆盖/PII 问题，以及严重 1 事件的项目负责人。

### 4.1 升级流程

Android 事件使用与 Android 相同的严重级别进行分类
支持 Playbook (§2.1)。下表总结了必须寻呼的人员以及如何寻呼
预计每个响应者都会很快加入桥梁。|严重性 |影响 |主要响应者（≤5 分钟）|二次升级（≤10分钟）|附加通知 |笔记|
|----------|--------|----------------------------|--------------------------------|----------------------------------------|--------|
|严重程度1 |面向客户的中断、隐私泄露或数据泄露 | Android 遥测待命 (`android-telemetry-primary`) | Torii 待命 + 项目主管 |合规性 + SRE 治理 (`#sre-governance`)、临时集群所有者 (`#android-staging`) |立即启动作战室并打开共享文档以进行命令记录。 |
|严重程度2 |队列退化、覆盖误用或长时间重放积压 | Android 遥测随叫随到 | Android 基础 TL + 文档/支持管理器 |项目负责人，发布工程联络员 |如果超驰超过 24 小时，则升级至合规性。 |
|严重程度3 |单租户问题、实验室演练或咨询警报 |支持工程师| Android 随叫随到（可选）|文档/意识支持 |如果范围扩大或多个租户受到影响，请转换为 Sev2。 |

|窗口|行动|所有者 |证据/注释|
|--------|--------|----------|----------------|
| 0–5 分钟 |确认 PagerDuty，分配事件指挥官 (IC)，并创建 `incident/YYYY-MM-DD-android-telemetry.md`。删除 `#android-sdk-support` 中的链接加单线状态。 |待命 SRE/支持工程师 | PagerDuty ack + 在其他事件日志旁边提交的事件存根的屏幕截图。 |
| 5–15 分钟 |运行 `scripts/telemetry/check_redaction_status.py --status-url https://android-telemetry-stg/api/redaction/status` 并将摘要粘贴到事件文档中。 Ping Android 可观测性 TL (Haruka Yamamoto) 和支持主管 (Priya Deshpande) 进行实时交接。 | IC + Android 可观察性 TL |附加 CLI 输出 JSON，记下打开的仪表板 URL，并标记谁拥有诊断。 |
| 15–25 分钟 |让暂存集群所有者（负责可观察性的 Haruka Yamamoto，负责 SRE 的 Liam O’Connor）在 `android-telemetry-stg` 上进行重现。使用 `scripts/telemetry/generate_android_load.sh --cluster android-telemetry-stg` 进行种子加载并从 Pixel + 模拟器捕获队列转储以确认症状奇偶性。 |暂存集群所有者 |将清理后的 `pending.queue` + `PendingQueueInspector` 输出上传到事件文件夹。 |
| 25–40 分钟 |决定覆盖、Torii 限制或 StrongBox 后备。如果怀疑 PII 暴露或非确定性哈希，请通过 `#compliance-alerts` 寻呼合规部（Sofia Martins、Daniel Park），并在同一事件线程中通知项目负责人。 | IC + 合规 + 项目主管 |链接覆盖令牌、Norito 清单和批准注释。 |
| ≥40分钟|提供 30 分钟状态更新（PagerDuty 注释 + `#android-sdk-support`）。如果尚未激活，请安排作战室桥，记录缓解预计时间，并确保发布工程 (Alexei Morozov) 随时待命以滚动收集器/SDK 工件。 |集成电路 |带时间戳的更新以及存储在事件文件中的决策日志，并在下周刷新期间在 `status.md` 中进行汇总。 |- 所有升级都必须使用 Android 支持手册中的“所有者/下次更新时间”表反映在事件文档中。
- 如果另一事件已经发生，请加入现有的作战室并附加 Android 上下文，而不是启动一个新的。
- 当事件触及运行手册空白时，在 AND7 JIRA 史诗中创建后续任务并标记 `telemetry-runbook`。

## 5. 混沌与准备练习

- 执行详细的场景
  `docs/source/sdk/android/telemetry_chaos_checklist.md` 每季度及之前
  主要版本。使用实验室报告模板记录结果。
- 将证据（屏幕截图、日志）存储在
  `docs/source/sdk/android/readiness/screenshots/`。
- 跟踪 AND7 史诗中带有标签 `telemetry-lab` 的补救票证。
- 场景图：C1（编辑故障）、C2（覆盖）、C3（出口商限电）、C4
  （使用具有漂移配置的 `run_schema_diff.sh` 的架构差异门），C5
  （设备配置文件偏差通过 `generate_android_load.sh` 播种），C6（Torii 超时
  + 队列重播），C7（证明拒绝）。保持此编号与
  `telemetry_lab_01.md` 和添加练习时的混乱清单。

### 5.1 编辑漂移和覆盖演练（C1/C2）

1. 通过注入哈希失败
   `scripts/telemetry/inject_redaction_failure.sh` 并等待 PagerDuty
   警报（`android.telemetry.redaction.failure`）。捕获 CLI 输出
   `scripts/telemetry/check_redaction_status.py --status-url <collector>` 为
   事件记录。
2. 使用 `--clear` 清除故障并确认警报在
   10分钟；附上盐/权限面板的 Grafana 屏幕截图。
3. 使用以下命令创建签名覆盖请求
   `docs/examples/android_override_request.json`，应用它
   `scripts/android_override_tool.sh apply`，并通过以下方式验证未散列的样本
   检查暂存中的导出器有效负载（查找
   `android.telemetry.redaction.override`）。
4. 使用 `scripts/android_override_tool.sh revoke --token <token>` 撤销覆盖，
   将覆盖令牌哈希加上票证引用附加到
   `docs/source/sdk/android/telemetry_override_log.md`，并创建摘要 JSON
   在 `docs/source/sdk/android/readiness/override_logs/` 下。这将关闭
   混乱清单中的 C2 场景并使治理证据保持新鲜。

### 5.2 出口商限电和队列重演演习（C3/C6）1. 缩小临时收集器的规模（`kubectl scale
   deploy/android-otel-collector --replicas=0`) 来模拟导出器
   停电。通过状态 CLI 跟踪缓冲区指标并确认警报触发
   15 分钟标记。
2. 恢复收集器，确认积压排出，并将收集器日志归档
   显示重播完成的片段。
3. 在暂存 Pixel 和模拟器上，按照 ScenarioC6 操作：安装
   `examples/android/operator-console`，切换飞行模式，提交演示
   传输，然后禁用飞行模式并观察队列深度指标。
4. 拉取每个待处理队列 (`adb shell run-as  cat files/pending.queue >
   /tmp/.queue`), compile the inspector (`gradle -p java/iroha_android
   ：核心：类> / dev / null`), and run `java -cp构建/类
   org.hyperledger.iroha.android.tools.PendingQueueInspector --文件
   /tmp/.queue --json > 队列重播-.json`。附上解码的
   信封加上重放哈希值到实验室日志。
5. 更新混乱报告，包括导出器中断持续时间、之前/之后的队列深度、
   并确认 `android_sdk_offline_replay_errors` 仍为 0。

### 5.3 暂存集群混沌脚本 (android-telemetry-stg)

暂存集群负责人 Haruka Yamamoto (Android Observability TL) 和 Liam O’Connor
(SRE) 每当安排排练时都遵循此脚本。顺序保持
参与者与遥测混乱清单保持一致，同时保证
捕获人工制品以供治理。

**参与者**

|角色 |职责|联系我们 |
|------|--------------------|---------|
| Android 随叫随到 IC |驱动钻机、协调 PagerDuty 注释、拥有命令日志 | PagerDuty `android-telemetry-primary`、`#android-sdk-support` |
|暂存集群所有者（Haruka、Liam）|门更改窗口，运行 `kubectl` 操作，快照集群遥测 | `#android-staging` |
|文档/支持经理 (Priya) |记录证据、跟踪实验室清单、发布后续通知单 | `#docs-support` |

**飞行前协调**

- 演习前 48 小时，提交变更请求，列出计划的内容
  方案 (C1–C7) 并将链接粘贴到 `#android-staging` 中，以便集群所有者
  可以阻止冲突的部署。
- 收集最新的 `ClientConfig` 哈希值和 `kubectl --context staging get pods
  -n android-telemetry-stg` 输出建立基线状态，然后存储
  均位于 `docs/source/sdk/android/readiness/labs/reports/<date>/` 下。
- 确认设备覆盖范围（Pixel + 模拟器）并确保
  `ci/run_android_tests.sh` 编译了实验室使用的工具
  （`PendingQueueInspector`，遥测注入器）。

**执行检查点**

- 在`#android-sdk-support`中宣布“混乱开始”，开始桥接录音，
  并保持 `docs/source/sdk/android/telemetry_chaos_checklist.md` 可见，以便
  每一条命令都是为抄写员叙述的。
- 让暂存所有者镜像每个注射器操作（`kubectl scale`，导出器
  重新启动，负载生成器），因此 Observability 和 SRE 都确认了该步骤。
- 捕获`scripts/telemetry/check_redaction_status.py 的输出
  --status-url https://android-telemetry-stg/api/redaction/status` 之后
  场景并将其粘贴到事件文档中。

**恢复**- 在清除所有喷油器之前，请勿离开桥（`inject_redaction_failure.sh --clear`，
  `kubectl scale ... --replicas=1`) 和 Grafana 仪表板显示绿色状态。
- 文档/支持存档队列转储、CLI 日志和屏幕截图
  `docs/source/sdk/android/readiness/screenshots/<date>/` 并勾选存档
  变更请求关闭之前的清单。
- 对于任何场景，使用 `telemetry-chaos` 标签记录后续票证
  失败或产生意外的指标，并在 `status.md` 中引用它们
  在接下来的每周回顾期间。

|时间 |行动|所有者 |文物|
|------|--------|----------|----------|
| T−30 分钟 |验证 `android-telemetry-stg` 运行状况：`kubectl --context staging get pods -n android-telemetry-stg`，确认没有挂起的升级，并记录收集器版本。 |遥 | `docs/source/sdk/android/readiness/screenshots/<date>/cluster-health.png` |
| T−20分钟|种子基线负载 (`scripts/telemetry/generate_android_load.sh --cluster android-telemetry-stg --duration 20m`) 并捕获标准输出。 |利亚姆| `readiness/labs/reports/<date>/load-generator.log` |
| T−15 分钟 |将 `docs/source/sdk/android/readiness/incident/telemetry_chaos_template.md` 复制到 `docs/source/sdk/android/readiness/incident/<date>-telemetry-chaos.md`，列出要运行的方案 (C1–C7)，并分配抄写员。 | Priya Deshpande（支持）|排练开始前进行的事件降价。 |
| T−10分钟|确认Pixel+模拟器上线，安装最新SDK，`ci/run_android_tests.sh`编译出`PendingQueueInspector`。 |利亚姆遥 | `readiness/screenshots/<date>/device-checklist.png` |
| T−5分钟|启动Zoom桥，开始屏幕录制，并在`#android-sdk-support`中宣布“混乱开始”。 | IC / 文档/支持 |录音保存在 `readiness/archive/<month>/` 下。 |
| +0 分钟 |执行 `docs/source/sdk/android/readiness/labs/telemetry_lab_01.md` 中选定的场景（通常为 C2 + C6）。保持实验室指南可见，并在发生命令调用时调出命令调用。 | Haruka 开车，Liam 反映结果 |日志实时附加到事件文件中。 |
| +15 分钟 |暂停收集指标 (`scripts/telemetry/check_redaction_status.py --status-url https://android-telemetry-stg/api/redaction/status`) 并抓取 Grafana 屏幕截图。 |遥 | `readiness/screenshots/<date>/status-<scenario>.png` |
| +25 分钟 |恢复任何注入的故障（`inject_redaction_failure.sh --clear`、`kubectl scale ... --replicas=1`）、重播队列并确认警报关闭。 |利亚姆| `readiness/labs/reports/<date>/recovery.log` |
| +35 分钟 |汇报：根据每个场景的通过/失败情况更新事件文档，列出后续行动，并将工件推送到 git。通知文档/支持归档清单可以完成。 |集成电路 |事件文档已更新，`readiness/archive/<month>/checklist.md` 已勾选。 |

- 让分期所有者留在桥上，直到出口商健康并且所有警报均已清除。
- 将原始队列转储存储在 `docs/source/sdk/android/readiness/labs/reports/<date>/queues/` 中，并在事件日志中引用它们的哈希值。
- 如果方案失败，请立即创建标记为 `telemetry-chaos` 的 JIRA 票证，并将其与 `status.md` 交叉链接。
- 自动化助手：`ci/run_android_telemetry_chaos_prep.sh` 包装负载生成器、状态快照和队列导出管道。当暂存访问可用时设置 `ANDROID_TELEMETRY_DRY_RUN=false` 和 `ANDROID_PENDING_QUEUE_EXPORTS=pixel8=/tmp/pixel.queue,emulator=/tmp/emulator.queue`（等），以便脚本复制每个队列文件，发出 `<label>.sha256`，并运行 `PendingQueueInspector` 以生成 `<label>.json`。仅当必须跳过 JSON 发射时（例如，没有可用的 JDK），才使用 `ANDROID_PENDING_QUEUE_INSPECTOR=false`。 **在运行帮助程序之前始终通过设置 `ANDROID_TELEMETRY_EXPECTED_SALT_EPOCH=<YYYYQ#>` 和 `ANDROID_TELEMETRY_EXPECTED_SALT_ROTATION=<id>` 导出预期的盐标识符**，以便如果捕获的遥测数据偏离 Rust 基线，嵌入式 `check_redaction_status.py` 调用会快速失败。

## 6. 文档和支持- **操作员支持套件：** `docs/source/sdk/android/readiness/and7_operator_enablement.md`
  链接运行手册、遥测政策、实验室指南、存档清单和知识
  签入单个 AND7 就绪包。准备SRE的时候参考一下
  治理预读或安排季度刷新。
- **启用会议：** 2026 年 2 月 18 日进行 60 分钟的启用录制
  每季度刷新一次。材料生活在下面
  `docs/source/sdk/android/readiness/`。
- **知识检查：** 员工必须通过准备表获得 ≥90% 的分数。商店
  结果为 `docs/source/sdk/android/readiness/forms/responses/`。
- **更新：**每当遥测模式、仪表板或覆盖策略时
  更改，更新此 Runbook、支持 playbook 和 `status.md`
  公关。
- **每周审查：** 在每个 Rust 候选版本之后（或至少每周），验证
  `java/iroha_android/README.md` 和此操作手册仍然反映了当前的自动化，
  夹具轮换程序和治理期望。捕获评论
  `status.md`，因此基金会里程碑审核可以跟踪文档的新鲜度。

## 7. StrongBox 证明工具- **目的：** 在将设备推广到市场之前验证硬件支持的证明捆绑包
  StrongBox 池（AND2/AND6）。该工具使用捕获的证书链并验证它们
  使用与生产代码执行相同的策略来对抗受信任的根。
- **参考：** 请参阅 `docs/source/sdk/android/strongbox_attestation_harness_plan.md` 了解完整信息
  捕获 API、别名生命周期、CI/Buildkite 连接和所有权矩阵。将该计划视为
  新实验室技术人员入职或更新财务/合规工件时的真相来源。
- **工作流程：**
  1. 在设备上收集证明捆绑包（别名 `challenge.hex` 和 `chain.pem`，其中
     leaf→root 命令）并将其复制到工作站。
  2. 运行 `scripts/android_keystore_attestation.sh --bundle-dir  --trust-root 
     [--trust-root-dir ] --require-strongbox --output ` 使用适当的
     Google/Samsung root（目录允许您加载整个供应商捆绑包）。
  3. 将 JSON 摘要与原始证明材料一起存档在
     `artifacts/android/attestation/<device-tag>/`。
- **捆绑包格式：** 遵循 `docs/source/sdk/android/readiness/android_strongbox_attestation_bundle.md`
  所需的文件布局（`chain.pem`、`challenge.hex`、`alias.txt`、`result.json`）。
- **可信根：** 从设备实验室机密存储中获取供应商提供的 PEM；通过多个
  `--trust-root` 参数或将 `--trust-root-dir` 指向保存锚点的目录
  该链以非 Google 锚点终止。
- **CI 工具：** 使用 `scripts/android_strongbox_attestation_ci.sh` 批量验证存档的包
  在实验室机器或 CI 运行器上。该脚本扫描 `artifacts/android/attestation/**` 并调用
  包含记录文件的每个目录的线束，写入刷新的 `result.json`
  总结到位。
- **CI 通道：** 同步新包后，运行中定义的 Buildkite 步骤
  `.buildkite/android-strongbox-attestation.yml` (`buildkite-agent pipeline upload --pipeline .buildkite/android-strongbox-attestation.yml`)。
  该作业执行 `scripts/android_strongbox_attestation_ci.sh`，生成摘要
  `scripts/android_strongbox_attestation_report.py`，将报告上传到`artifacts/android_strongbox_attestation_report.txt`，
  并将构建注释为 `android-strongbox/report`。立即调查任何故障并
  链接设备矩阵中的构建 URL。
- **报告：** 将 JSON 输出附加到治理审查并更新中的设备矩阵条目
  `docs/source/sdk/android/readiness/android_strongbox_device_matrix.md` 以及认证日期。
- **模拟演练：** 当硬件不可用时，运行 `scripts/android_generate_mock_attestation_bundles.sh`
  （使用 `scripts/android_mock_attestation_der.py`）创建确定性测试包以及共享模拟根，以便 CI 和文档可以端到端地运用该工具。
- **代码内护栏：** `ci/run_android_tests.sh --tests
  org.hyperledger.iroha.android.crypto.keystore.KeystoreKeyProviderTests` 涵盖空与挑战
  证明再生（StrongBox/TEE 元数据）并发出 `android.keystore.attestation.failure`
  挑战不匹配，因此在发送新包之前捕获缓存/遥测回归。

## 8. 联系人

- **支持工程待命：** `#android-sdk-support`
- **SRE 治理：** `#sre-governance`
- **文档/支持：** `#docs-support`
- **升级树：** 请参阅 Android 支持手册 §2.1

## 9. 故障排除场景路线图项目 AND7-P2 列出了重复寻呼的三个事件类别
Android 待命：Torii/网络超时、StrongBox 认证失败以及
`iroha_config` 明显漂移。提交前仔细核对相关清单
Sev1/2 后续行动并将证据存档于 `incident/<date>-android-*.md` 中。

### 9.1 Torii 和网络超时

**信号**

- `android_sdk_submission_latency`、`android_sdk_pending_queue_depth` 上的警报，
  `android_sdk_offline_replay_errors` 和 Torii `/v2/pipeline` 错误率。
- `operator-console` 小部件（示例/android）显示停滞的队列耗尽或
  重试陷入指数退避。

**立即响应**

1. 确认 PagerDuty (`android-networking`) 并启动事件日志。
2. 捕获 Grafana 快照（提交延迟 + 队列深度），涵盖
   最后30分钟。
3. 记录设备日志中的活动 `ClientConfig` 哈希 (`ConfigWatcher`
   每当重新加载成功或失败时都会打印清单摘要）。

**诊断**

- **队列运行状况：** 从临时设备或
  模拟器（`adb shell run-as  cat files/pending.queue >
  /tmp/pending.queue`）。解码信封
  `OfflineSigningEnvelopeCodec` 如中所述
  `docs/source/sdk/android/offline_signing.md#4-queueing--replay` 确认
  积压符合运营商的预期。将解码后的哈希值附加到
  事件。
- **哈希库存：** 下载队列文件后，运行检查器助手
  捕获事件工件的规范哈希值/别名：

  ```bash
  gradle -p java/iroha_android :core:classes >/dev/null  # compiles classes if needed
  java -cp build/classes org.hyperledger.iroha.android.tools.PendingQueueInspector \
    --file /tmp/pending.queue --json > queue-inspector.json
  ```

  将 `queue-inspector.json` 和打印精美的标准输出附加到事件中
  并将其链接到场景 D 的 AND7 实验室报告。
- **Torii 连接：** 在本地运行 HTTP 传输工具以排除 SDK
  回归：`ci/run_android_tests.sh` 练习
  `HttpClientTransportTests`、`HttpClientTransportHarnessTests` 和
  `ToriiMockServerTests`。这里的失败表明客户端错误而不是
  Torii 中断。
- **故障注入演练：** 在暂存 Pixel (StrongBox) 和 AOSP 上
  模拟器，切换连接以重现挂起队列的增长：
  `adb shell cmd connectivity airplane-mode enable` → 提交两个演示
  通过操作员控制台进行交易 → `adb shell cmd 连接飞行模式
  禁用` → verify the queue drains and `android_sdk_offline_replay_errors`
  保持为 0。记录重放交易的哈希值。
- **警报奇偶校验：** 当调整阈值时或 Torii 更改后，执行
  `scripts/telemetry/test_torii_norito_rpc_alerts.sh` 因此 Prometheus 规则保留
  与仪表板对齐。

**恢复**

1. 如果 Torii 降级，请接听 Torii 并继续重放
   一旦 `/v2/pipeline` 接受流量就排队。
2. 仅通过签名的 `iroha_config` 清单重新配置受影响的客户端。的
   `ClientConfig` 热重载观察程序必须在事件发生之前发出成功日志
   可以关闭。
3. 使用重播之前/之后的队列大小以及以下的哈希值来更新事件
   任何被丢弃的交易。

### 9.2 StrongBox 和证明失败

**信号**- `android_sdk_strongbox_success_rate` 上的警报或
  `android.keystore.attestation.failure`。
- `android.keystore.keygen` 遥测现在记录所请求的
  `KeySecurityPreference` 和使用的路由（`strongbox`、`hardware`、
  当 StrongBox 首选项登陆时，`software`）带有 `fallback=true` 标志
  TEE/软件。 STRONGBOX_REQUIRED 请求现在会快速失败，而不是静默失败
  返回 TEE 密钥。
- 支持引用 `KeySecurityPreference.STRONGBOX_ONLY` 设备的票证
  回到软件键。

**立即响应**

1. 确认 PagerDuty (`android-crypto`) 并捕获受影响的别名标签
   （加盐哈希）加上设备配置文件存储桶。
2. 检查设备的证明矩阵条目
   `docs/source/sdk/android/readiness/android_strongbox_device_matrix.md` 和
   记录最后一次验证的日期。

**诊断**

- **捆绑包验证：** 运行
  `scripts/android_keystore_attestation.sh --bundle-dir <bundle> --trust-root <root.pem>`
  在存档的证明上确认故障是否是由于设备造成的
  配置错误或策略更改。附上生成的 `result.json`。
- **挑战重新生成：** 挑战不会被缓存。每个挑战请求都会重新生成一个新的
  `(alias, challenge)` 的认证和缓存；无挑战的调用重用缓存。不支持
- **CI 扫描：** 执行 `scripts/android_strongbox_attestation_ci.sh`，因此每隔
  存储的包被重新验证；这可以防止引入系统性问题
  通过新的信任锚。
- **设备练习：** 在没有 StrongBox 的硬件上（或通过强制使用模拟器），
  将 SDK 设置为仅需要 StrongBox，提交演示交易并确认
  遥测导出器发出 `android.keystore.attestation.failure` 事件
  与预期的原因。在支持 StrongBox 的 Pixel 上重复此操作，以确保
  幸福之路常绿。
- **SDK回归检查：**运行`ci/run_android_tests.sh`并支付
  注意以证明为中心的套件（`AndroidKeystoreBackendDetectionTests`，
  `AttestationVerifierTests`, `IrohaKeyManagerDeterministicExportTests`,
  `KeystoreKeyProviderTests` 用于缓存/挑战分离）。失败在这里
  表明客户端回归。

**恢复**

1. 如果供应商轮换了证书或者如果
   设备最近收到了重大 OTA。
2. 将刷新后的捆绑包上传到 `artifacts/android/attestation/<device>/` 并
   使用新日期更新矩阵条目。
3. 如果 StrongBox 在生产中不可用，请按照以下中的覆盖工作流程操作
   第 3 节并记录回退持续时间；长期缓解需要
   设备更换或供应商修复。

### 9.2a 确定性出口恢复

- **格式：** 当前导出为 v3（每次导出 salt/nonce + Argon2id，记录为
- **密码短语策略：** v3 强制执行 ≥12 个字符的密码短语。如果用户供应较短
  密码短语，指示他们使用合规的密码短语重新导出； v0/v1 导入是
  豁免，但应在导入后立即重新包装为 v3。
- **篡改/重用防护：** 解码器拒绝零/短盐或随机数长度和重复
  盐/随机数对表面显示为 `salt/nonce reuse` 错误。重新生成导出以清除
  警卫；不要试图强制重复使用。
  `SoftwareKeyProvider.importDeterministic(...)` 补充密钥，然后
  `exportDeterministic(...)` 发出 v3 捆绑包，以便桌面工具记录新的 KDF
  参数。### 9.3 清单和配置不匹配

**信号**

- `ClientConfig` 重新加载失败、Torii 主机名或遥测不匹配
  由 AND7 diff 工具标记的架构差异。
- 操作员报告同一设备中的不同重试/退避旋钮
  舰队。

**立即响应**

1. 捕获 Android 日志中打印的 `ClientConfig` 摘要和
   发布清单中的预期摘要。
2. 转储运行节点配置以进行比较：
   `iroha_cli config show --actual > /tmp/iroha_config.actual.json`。

**诊断**

- **架构差异：** 运行 `scripts/telemetry/run_schema_diff.sh --android-config
   --rust-config  --textfile-dir /var/lib/node_exporter/textfile_collector`
  要生成 Norito diff 报告，请刷新 Prometheus 文本文件，并附加
  JSON 工件以及事件的指标证据和 AND7 遥测准备日志。
- **清单验证：** 使用 `iroha_cli runtime capabilities` （或运行时
  审计命令）来检索节点的公布的加密/ABI 哈希值并确保
  它们与移动清单相匹配。不匹配确认节点已回滚
  无需重新发布 Android 清单。
- **SDK回归检查：** `ci/run_android_tests.sh` 涵盖
  `ClientConfigNoritoRpcTests`、`ClientConfig.ValidationTests` 和
  `HttpClientTransportStatusTests`。失败表明附带的 SDK 无法
  解析当前部署的清单格式。

**恢复**

1.通过授权管道重新生成清单（通常是
   `iroha_cli runtime Capabilities` → 签名的 Norito 清单 → 配置包）和
   通过运营商渠道重新部署。切勿编辑 `ClientConfig`
   覆盖设备上的。
2. 更正后的舱单登陆后，请注意 `ConfigWatcher`“重新加载正常”
   每个舰队层上的消息并仅在遥测后关闭事件
   模式差异报告奇偶性。
3. 将清单哈希、架构差异工件路径和事件链接记录在
   Android 部分下的 `status.md` 用于审核。

## 10. 操作员支持课程

路线图项目 **AND7** 需要可重复的培训包，以便操作员，
支持工程师，SRE 可以采用遥测/编辑更新，而无需
猜测。将此部分与
`docs/source/sdk/android/readiness/and7_operator_enablement.md`，其中包含
详细的清单和工件链接。

### 10.1 会议模块（60 分钟简报）

1. **遥测架构（15分钟）。** 遍历导出器缓冲区，
   编辑过滤器和模式差异工具。演示
   `scripts/telemetry/run_schema_diff.sh --textfile-dir /var/lib/node_exporter/textfile_collector` 加
   `scripts/telemetry/check_redaction_status.py` 让与会者了解平价如何
   强制执行。
2. **运行手册 + 混沌实验室（20 分钟）。** 突出显示本运行手册的第 2-9 节，
   演练 `readiness/labs/telemetry_lab_01.md` 中的一个场景，并展示如何
   将文物归档到 `readiness/labs/reports/<stamp>/` 下。
3. **覆盖 + 合规工作流程（10 分钟）。** 查看第 3 节覆盖，
   演示 `scripts/android_override_tool.sh`（应用/撤销/摘要），以及
   更新 `docs/source/sdk/android/telemetry_override_log.md` 加上最新的
   摘要 JSON。
4. **问答/知识检查（15 分钟）。** 使用快速参考卡
   `readiness/cards/telemetry_redaction_qrc.md` 锚定问题，然后
   在 `readiness/and7_operator_enablement.md` 中捕获后续内容。### 10.2 资产节奏和所有者

|资产|节奏|所有者 |存档位置 |
|--------|---------|----------|--------------------|
|录制的演练（缩放/团队）|每季度或每次盐轮换前 | Android Observability TL + 文档/支持管理器 | `docs/source/sdk/android/readiness/archive/<YYYY-MM>/`（录音+清单）|
|幻灯片和快速参考卡|每当政策/操作手册发生变化时进行更新 |文档/支持经理 | `docs/source/sdk/android/readiness/deck/` 和 `/cards/`（导出 PDF + Markdown）|
|知识检查+考勤表|每次直播结束后 |支持工程| `docs/source/sdk/android/readiness/forms/responses/` 和 `and7_operator_enablement.md` 考勤块 |
|问答积压/行动日志 |滚动；每次会议后更新|法学硕士（代理 DRI）| `docs/source/sdk/android/readiness/and7_operator_enablement.md` §6 |

### 10.3 证据和反馈循环

- 将会话工件（屏幕截图、事件演习、测验导出）存储在
  用于混乱排练的相同日期目录，以便治理可以审计两者
  准备情况一起跟踪。
- 会话完成后，更新 `status.md`（Android 部分），其中包含以下链接：
  存档目录并记下任何打开的后续内容。
- 现场问答中的未决问题必须转化为问题或文档
  一周内拉取请求；参考路线图史诗（AND7/AND8）
  票证说明，以便业主保持一致。
- SRE 同步检查存档清单以及列出的架构差异工件
  第 2.3 节在宣布本季度课程结束之前。