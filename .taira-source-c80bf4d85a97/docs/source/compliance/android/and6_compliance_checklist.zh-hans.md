---
lang: zh-hans
direction: ltr
source: docs/source/compliance/android/and6_compliance_checklist.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2a0ce1be46f9c468915f50de5e38e2f34657b26bf4243fb5ea45dab175789393
source_last_modified: "2026-01-05T09:28:12.002460+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Android AND6 合规性检查表

该清单跟踪实现里程碑的合规性交付成果 **AND6 -
CI 和合规性强化**。它整合了所要求的监管工件
在 `roadmap.md` 中并定义了下的存储布局
`docs/source/compliance/android/` 所以发布工程、支持和法律
在批准 Android 版本之前可以参考相同的证据集。

## 范围和所有者

|面积 |可交付成果 |主要所有者 |备份/审阅|
|------|--------------|------------------------|--------------------|
|欧盟监管捆绑| ETSI EN 319 401 安全目标、GDPR DPIA 摘要、SBOM 证明、证据日志 |合规与法律（索菲亚·马丁斯）|发布工程（Alexei Morozov）|
|日本监管捆绑| FISC 安全控制清单、双语 StrongBox 证明包、证据日志 |合规与法律（丹尼尔·帕克）| Android 项目负责人 |
|设备实验室准备情况|容量跟踪、应急触发器、升级日志 |硬件实验室负责人 | Android 可观察性 TL |

## 人工制品矩阵|神器|描述 |存储路径|刷新节奏|笔记|
|----------|-------------|--------------|-----------------|--------|
| ETSI EN 319 401 安全目标 |描述 Android SDK 二进制文件的安全目标/假设的叙述。 | `docs/source/compliance/android/eu/security_target.md` |重新验证每个 GA + LTS 版本。 |必须引用发布序列的构建来源哈希。 |
| GDPR DPIA 摘要 |涵盖遥测/日志记录的数据保护影响评估。 | `docs/source/compliance/android/eu/gdpr_dpia_summary.md` |年度+材料遥测更改之前。 |参考 `sdk/android/telemetry_redaction.md` 中的编辑策略。 |
| SBOM认证| Gradle/Maven 工件的签名 SBOM 和 SLSA 出处。 | `docs/source/compliance/android/eu/sbom_attestation.md` |每个 GA 版本。 |运行 `scripts/android_sbom_provenance.sh <version>` 以生成 CycloneDX 报告、共同签名包和校验和。 |
| FISC 安全控制清单 |已完成将 SDK 控制映射到 FISC 要求的清单。 | `docs/source/compliance/android/jp/fisc_controls_checklist.md` |年度 + JP 合作伙伴试点之前。 |提供双语标题（EN/JP）。 |
| StrongBox 认证包（日本）|针对日本监管机构的每台设备的认证摘要 + 链。 | `docs/source/compliance/android/jp/strongbox_attestation.md` |当新硬件进入池时。 |指向 `artifacts/android/attestation/<device>/` 下的原始制品。 |
|法律签署备忘录|律师摘要涵盖 ETSI/GDPR/FISC 范围、隐私状况以及所附物品的监管链。 | `docs/source/compliance/android/eu/legal_signoff_memo.md` |每次工件包发生变化或添加新的管辖区时。 |备忘录引用了证据日志中的哈希值以及设备实验室应急包的链接。 |
|证据日志|带有哈希/时间戳元数据的已提交工件的索引。 | `docs/source/compliance/android/evidence_log.csv` |每当上述任何条目发生变化时都会更新。 |添加 Buildkite 链接 + 审阅者签字。 |
|设备实验室仪器包 |使用 `device_lab_instrumentation.md` 中定义的流程记录的特定于插槽的遥测、队列和证明证据。 | `artifacts/android/device_lab/<slot>/`（参见 `docs/source/compliance/android/device_lab_instrumentation.md`）|每个预留插槽+故障转移演练。 |捕获 SHA-256 清单并引用证据日志 + 检查表中的插槽 ID。 |
|设备实验室预约日志|用于在冻结期间保持 StrongBox 池 ≥80% 的预订工作流程、审批、容量快照和升级阶梯。 | `docs/source/compliance/android/device_lab_reservation.md` |每当创建/更改预订时更新。 |请参考过程中注明的 `_android-device-lab` 票证 ID 和每周日历导出。 |
|设备实验室故障转移操作手册和练习包 |季度排练计划和工件清单展示后备通道、Firebase 突发队列和外部 StrongBox 保留器准备情况。 | `docs/source/compliance/android/device_lab_failover_runbook.md` + `artifacts/android/device_lab_contingency/<YYYYMMDD>-failover-drill/` |每季度一次（或在硬件名册更改后）。 |在证据日志中记录演练 ID，并附加 Runbook 中记录的清单哈希 + PagerDuty 导出。 |

> **提示：** 附加 PDF 或外部签名的工件时，请存储一个简短的
> 表格路径中的 Markdown 包装器链接到中的不可变工件
> 治理份额。这使仓库保持轻量级，同时保留
> 审计跟踪。

## 欧盟监管数据包 (ETSI/GDPR)欧盟数据包将上述三件文物以及法律备忘录联系在一起：

- 使用版本标识符更新 `security_target.md`、Torii 清单哈希、
  和 SBOM 摘要，以便审计人员可以将二进制文件与声明的范围进行匹配。
- 使 DPIA 摘要与最新的遥测编辑政策保持一致，并且
  附上 `docs/source/sdk/android/telemetry_redaction.md` 中引用的 Norito 差异摘录。
- SBOM 证明条目应包括：CycloneDX JSON 哈希、出处
  捆绑散列、联合签名语句以及生成它们的 Buildkite 作业 URL。
- `legal_signoff_memo.md` 必须捕获建议/日期，列出所有文物 +
  SHA-256，概述任何补偿控制，并链接到证据日志行
  加上跟踪批准的 PagerDuty 票证 ID。

## 日本监管数据包 (FISC/StrongBox)

日本监管机构期望提供包含双语文档的并行捆绑包：

- `fisc_controls_checklist.md` 镜像官方电子表格；填写两个
  EN 和 JA 列并参考 `sdk/android/security.md` 的特定部分
  或满足每个控件的 StrongBox 证明包。
- `strongbox_attestation.md` 总结了最新的运行
  `docs/source/sdk/android/readiness/android_strongbox_attestation_bundle.md`
  （每个设备 JSON + Norito 信封）。嵌入指向不可变工件的链接
  在 `artifacts/android/attestation/<device>/` 下并记下旋转节奏。
- 记录内含提交内容的双语求职信模板
  `docs/source/compliance/android/jp/README.md`，因此支持人员可以重复使用它。
- 使用引用清单的单行更新证据日志，
  证明捆绑哈希，以及与交付相关的任何 JP 合作伙伴票证 ID。

## 提交工作流程

1. **草稿** - 所有者准备工件，记录计划的文件名
   上表，并打开一个 PR，其中包含更新的 Markdown 存根以及
   外部附件的校验和。
2. **审查** - 发布工程确认来源哈希与暂存的匹配
   二进制文件；合规性验证监管语言；支持确保 SLA 和
   正确引用了遥测策略。
3. **签署** - 批准者将其姓名和日期添加到 `Sign-off` 表中
   下面。使用 PR URL 和 Buildkite 运行更新证据日志。
4. **发布** - SRE 治理签署后，将工件链接到
   `status.md` 并更新 Android 支持 Playbook 参考。

### 签核日志

|文物|评论者 |日期 |公关/证据|
|----------|-------------|------|---------------|
| *（待定）* | - | - | - |

## 设备实验室预订和应急计划

为了减轻路线图中指出的**设备实验室可用性**风险：- 在 `docs/source/compliance/android/evidence_log.csv` 中跟踪每周容量
  （`device_lab_capacity_pct` 列）。如果可用，请通知发布工程
  连续两周跌破70%。
- 预留保险箱/一般车道如下
  `docs/source/compliance/android/device_lab_reservation.md` 领先于每个
  冻结、排练或合规性扫描，以便请求、批准和工件
  在 `_android-device-lab` 队列中捕获。链接生成的工单 ID
  记录容量快照时在证据日志中。
- **后备池：**首先突发到共享像素池；如果仍然饱和，
  安排 Firebase 测试实验室冒烟运行以进行 CI 验证。
- **外部实验室固定器：** 与 StrongBox 合作伙伴一起维护固定器
  实验室，以便我们可以在冻结窗口期间保留硬件（至少提前 7 天）。
- **升级：**在 PagerDuty 中引发 `AND6-device-lab` 事件
  主池和后备池容量降至 50% 以下。硬件实验室负责人
  与 SRE 协调以重新确定设备的优先级。
- **故障转移证据包：** 将每次排练存储在
  `artifacts/android/device_lab_contingency/<YYYYMMDD>/` 带预订
  请求、PagerDuty 导出、硬件清单和恢复记录。参考
  来自 `device_lab_contingency.md` 的捆绑包并将 SHA-256 添加到证据日志
  因此，法律部门可以证明应急工作流程已得到执行。
- **季度演习：** 练习操作手册
  `docs/source/compliance/android/device_lab_failover_runbook.md`，附上
  生成的捆绑包路径 + `_android-device-lab` 票证的清单哈希，以及
  在应急日志和证据日志中镜像演练 ID。

记录应急计划的每次启动
`docs/source/compliance/android/device_lab_contingency.md`（包括日期、
触发器、操作和后续行动）。

## 静态分析原型

- `make android-lint` 包装 `ci/check_android_javac_lint.sh`，编译
  `java/iroha_android` 和共享的 `java/norito_java` 源
  `javac --release 21 -Xlint:all -Werror`（标记类别在
- 编译后，脚本强制执行 AND6 依赖策略
  `jdeps --summary`，如果任何模块超出批准的允许列表，则失败
  (`java.base`、`java.net.http`、`jdk.httpserver`) 出现。这保持了
  Android 表面符合 SDK 委员会的“无隐藏 JDK 依赖项”
  StrongBox 合规性审查之前的要求。
- CI 现在通过以下方式运行相同的门
  `.github/workflows/android-lint.yml`，它调用
  `ci/check_android_javac_lint.sh` 在每次涉及 Android 的推送/公关上
  共享 Norito Java 源代码和上传 `artifacts/android/lint/jdeps-summary.txt`
  因此合规性审查可以引用已签名的模块列表，而无需重新运行
  本地脚本。
- 当需要保留临时数据时，设置`ANDROID_LINT_KEEP_WORKDIR=1`
  工作区。该脚本已经将生成的模块摘要复制到
  `artifacts/android/lint/jdeps-summary.txt`；设置
  `ANDROID_LINT_SUMMARY_OUT=docs/source/compliance/android/evidence/android_lint_jdeps.txt`
  （或类似）当您需要额外的版本化工件进行审核时。
  工程师在提交 Android PR 之前仍应在本地运行该命令
  涉及 Java 源并将记录的摘要/日志附加到合规性
  评论。从发行说明中将其引用为“Android javac lint + dependency
  扫描”。

## CI 证据（Lint、测试、证明）- `.github/workflows/android-and6.yml` 现在运行所有 AND6 门（javac lint +
  依赖项扫描、Android 测试套件、StrongBox 证明验证器以及
  device-lab 插槽验证）在每次接触 Android 表面的 PR/push 上。
- `ci/run_android_tests.sh` 包装 `ci/run_android_tests.sh` 并发出
  `artifacts/android/tests/test-summary.json` 处的确定性摘要，同时
  将控制台日志保存到 `artifacts/android/tests/test.log`。附上两者
  引用 CI 运行时将文件添加到合规性数据包。
- `scripts/android_strongbox_attestation_ci.sh --summary-out` 产生
  `artifacts/android/attestation/ci-summary.json`，验证捆绑的
  StrongBox 和 `artifacts/android/attestation/**` 下的证明链
  TEE 池。
- `scripts/check_android_device_lab_slot.py --root fixtures/android/device_lab`
  验证 CI 中使用的示例槽 (`slot-sample/`)，并可指向
  真实运行在 `artifacts/android/device_lab/<slot-id>/` 下
  `--require-slot --json-out <dest>` 证明仪器包如下
  记录的布局。 CI 将验证摘要写入
  `artifacts/android/device_lab/summary.json`；示例槽包括
  占位符遥测/证明/队列/日志提取加上记录
  `sha256sum.txt` 用于可重现的哈希值。

## 设备实验室仪器工作流程

每次预留或故障转移演练必须遵循
`device_lab_instrumentation.md` 遥测、队列和证明指南
文物与预订日志相符：

1. **种子槽文物。** 创建
   `artifacts/android/device_lab/<slot>/` 与标准子文件夹并运行
   插槽关闭后的 `shasum`（请参阅新的“Artifact Layout”部分）
   指南）。
2. **运行仪器命令。** 执行遥测/队列捕获，
   完全覆盖摘要、StrongBox 工具和 lint/依赖项扫描
   记录下来，以便输出反映 CI。
3. **归档证据。** 更新
   `docs/source/compliance/android/evidence_log.csv` 和预订票
   包含插槽 ID、SHA-256 清单路径和相应的仪表板/Buildkite
   链接。

将 artefact 文件夹和哈希清单附加到 AND6 发布包中
受影响的冻结窗口。治理审核者将拒绝不符合要求的清单
不引用插槽标识符和仪器指南。

### 预留和故障转移准备证据

路线图项目“监管制品批准和实验室应急”需要更多
比仪器仪表。每个 AND6 数据包还必须引用主动
预订工作流程和季度故障转移演练：- **预订手册 (`device_lab_reservation.md`).** 遵循预订
  表（交货时间、所有者、槽位长度），通过导出共享日历
  `scripts/android_device_lab_export.py`，并记录`_android-device-lab`
  `evidence_log.csv` 中的票证 ID 和容量快照。剧本
  详细说明升级阶梯和应急触发因素；复制这些详细信息
  当预订量移动或容量下降到低于预定值时，将进入清单条目
  80% 路线图目标。
- **故障转移演练操作手册 (`device_lab_failover_runbook.md`)。** 执行
  每季度排练（模拟停电 → 推广后备车道 → 参与
  Firebase 突发 + 外部 StrongBox 合作伙伴）并将文物存储在
  `artifacts/android/device_lab_contingency/<drill-id>/`。每个捆绑必须
  包含清单、PagerDuty 导出、Buildkite 运行链接、Firebase 突发
  报告以及运行手册中注明的保留确认书。参考
  证据日志中的演习 ID、SHA-256 清单和后续票证
  这个清单。

这些文件共同证明了设备容量规划、停电演练、
并且仪器包共享与所要求的相同的审计跟踪
路线图和法律审查员。

## 回顾节奏

- **每季度** - 验证 EU/JP 工件是否是最新的；刷新
  证据日志哈希；排练出处捕获。
- **预发布** - 在每次 GA/LTS 切换期间运行此清单并附加
  发布 RFC 的完整日志。
- **事件后** - 如果 Sev 1/2 事件涉及遥测、签名或
  证明，用修复说明更新相关的工件存根，以及
  捕获证据日志中的参考。