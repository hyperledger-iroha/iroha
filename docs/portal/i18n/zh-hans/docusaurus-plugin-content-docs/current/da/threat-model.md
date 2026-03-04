---
lang: zh-hans
direction: ltr
source: docs/portal/docs/da/threat-model.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

标题：数据可用性威胁模型
sidebar_label：威胁模型
描述：Sora Nexus 数据可用性的威胁分析、缓解措施和残余风险。
---

:::注意规范来源
:::

# Sora Nexus 数据可用性威胁模型

_上次审核：2026-01-19 — 下次预定审核：2026-04-19_

维护节奏：数据可用性工作组（<=90 天）。每次修订必须
出现在 `status.md` 中，其中包含主动缓解票据和模拟工件的链接。

## 目的和范围

数据可用性 (DA) 程序保留 Taikai 广播、Nexus 通道 blob 和
在拜占庭、网络和运营商故障下可检索的治理工件。
该威胁模型巩固了 DA-1 的工程工作（架构和威胁模型）
并作为下游 DA 任务（DA-2 至 DA-10）的基线。

范围内的组件：
- Torii DA 摄取扩展和 Norito 元数据编写器。
- SoraFS 支持的 blob 存储树（热/冷层）和复制策略。
- Nexus 区块承诺（有线格式、证明、轻客户端 API）。
- 特定于 DA 有效负载的 PDP/PoTR 执行挂钩。
- 操作员工作流程（固定、驱逐、削减）和可观察性管道。
- 接纳或驱逐 DA 运营商和内容的治理批准。

超出本文档范围：
- 完整的经济建模（在 DA-7 工作流中捕获）。
- SoraFS 威胁模型已涵盖 SoraFS 基本协议。
- 客户端 SDK 人体工程学超出了威胁面考虑因素。

## 架构概述

1. **提交：** 客户端通过 Torii DA 摄取 API 提交 blob。节点
   块 blob，编码 Norito 清单（blob 类型、通道、纪元、编解码器标志），
   并将块存储在热 SoraFS 层中。
2. **广告：** Pin 意图和复制提示传播到存储
   通过注册表（SoraFS 市场）提供的策略标签
   规定热/冷保留目标。
3. **承诺：** Nexus 定序器包括 blob 承诺（CID + 可选的 KZG
   根）在规范块中。轻客户端依赖于承诺哈希并且
   广告元数据以验证可用性。
4. **复制：** 存储节点拉取分配的共享/块，满足 PDP/PoTR
   挑战，并根据策略在热层和冷层之间提升数据。
5. **Fetch：**消费者通过SoraFS或DA感知网关获取数据，验证
   证明并在副本消失时提出修复请求。
6. **治理：** 议会和 DA 监督委员会批准运营商，
   租金时间表和执法升级。存储治理工件
   通过相同的 DA 路径来确保流程透明度。

## 资产和所有者

影响等级：**严重**破坏账本安全性/活跃性； **高**阻止 DA
回填或客户； **中度** 会降低质量，但仍可恢复；
**低**效果有限。

|资产|描述 |诚信|可用性 |保密 |业主|
| ---| ---| ---| ---| ---| ---|
| DA blob（块 + 清单）| Taikai、车道、治理 blob 存储在 SoraFS |关键|关键|中等| DA 工作组/存储团队 |
| Norito DA 清单 |描述 Blob 的类型化元数据 |关键|高|中等|核心协议工作组 |
|区块承诺 | Nexus 块内的 CID + KZG 根 |关键|高|低|核心协议工作组 |
| PDP/PoTR 时间表 | DA 副本的执行节奏 |高|高|低|存储团队|
|运营商注册表|批准的存储提供商和政策 |高|高|低|治理委员会|
|租金和奖励记录| DA 租金和罚款的分类帐条目 |高|中等|低|财政部工作组 |
|可观察性仪表板 | DA SLO、复制深度、警报 |中等|高|低| SRE / 可观察性 |
|维修意向 |请求补充缺失块的水|中等|中等|低|存储团队|

## 对手和能力

|演员 |能力|动机|笔记|
| ---| ---| ---| ---|
|恶意客户端 |提交格式错误的 blob、重放过时的清单、尝试在摄取时执行 DoS。 |扰乱 Taikai 广播，注入无效数据。 |没有特权密钥。 |
|拜占庭存储节点 |丢弃分配的副本、伪造 PDP/PoTR 证明、与他人勾结。 |减少 DA 保留、避免租金、扣留数据。 |拥有有效的操作员凭证。 |
|受损的测序仪 |省略承诺、对块模棱两可、重新排序 blob 元数据。 |隐藏 DA 提交内容，造成不一致。 |受多数共识限制。 |
|内幕操作员|滥用治理访问、篡改保留策略、泄露凭证。 |经济利益，破坏行为。 |访问热/冷层基础设施。 |
|网络对手|分区节点、延迟复制、注入 MITM 流量。 |降低可用性，降低 SLO。 |无法破坏 TLS，但可以丢弃/减慢链接。 |
|可观察性攻击者 |篡改仪表板/警报，抑制事件。 |隐藏 DA 中断。 |需要访问遥测管道。 |

## 信任边界

- **入口边界：** Torii DA 扩展的客户端。需要请求级身份验证，
  速率限制和有效负载验证。
- **复制边界：** 存储节点交换块和证明。节点是
  相互验证，但可能表现为拜占庭式。
- **账本边界：** 提交的块数据与链外存储。共识卫士
  完整性，但可用性需要链下执行。
- **治理边界：** 理事会/议会决定批准运营商，
  预算和削减。这里的中断直接影响 DA 部署。
- **可观察性边界：** 导出到仪表板/警报的指标/日志集合
  工具。篡改隐藏了中断或攻击。

## 威胁场景和控制

### 摄取路径攻击

**场景：** 恶意客户端提交格式错误的 Norito 有效负载或过大的负载
blob 会耗尽资源或走私无效元数据。

**控制**
- Norito 模式验证，具有严格的版本协商；拒绝未知的标志。
- Torii 摄取端点处的速率限制和身份验证。
- 由 SoraFS 分块器强制执行的块大小界限和确定性编码。
- 准入管道仅在完整性校验和匹配后持续显示。
- 确定性重播缓存 (`ReplayCache`) 跟踪 `(lane, epoch, sequence)` 窗口，在磁盘上保留高水位标记，并拒绝重复/过时的重播；财产和模糊利用涵盖不同的指纹和无序提交。【crates/iroha_core/src/da/replay_cache.rs:1】【fuzz/da_replay_cache.rs:1】【crates/iroha_torii/src/da/ingest.rs:1】

**剩余间隙**
- Torii 摄取必须将重播缓存线程化到准入中，并在重新启动时保留序列游标。
- Norito DA 模式现在有一个专用的模糊工具 (`fuzz/da_ingest_schema.rs`) 来强调编码/解码不变量；如果目标下降，覆盖仪表板应该发出警报。

### 复制扣留

**场景：** 拜占庭存储运算符接受 pin 分配但丢弃块，
通过伪造响应或串通传递 PDP/PoTR 挑战。

**控制**
- PDP/PoTR 挑战计划扩展到具有每个纪元覆盖范围的 DA 有效负载。
- 具有仲裁阈值的多源复制；获取协调器检测到
  丢失碎片并触发修复。
- 与失败的证明和丢失的副本相关的治理削减。
- 自动调节作业 (`cargo xtask da-commitment-reconcile`) 比较
  摄取带有 DA 承诺的收据（SignedBlockWire、`.norito` 或 JSON），
  发出用于治理的 JSON 证据包，并因丢失/不匹配而失败
  票证，以便 Alertmanager 可以寻呼遗漏/篡改。

**剩余间隙**
- `integration_tests/src/da/pdp_potr.rs` 中的模拟线束（由
  `integration_tests/tests/da/pdp_potr_simulation.rs`) 现在进行勾结
  和分区场景，验证 PDP/PoTR 计划检测到
  拜占庭行为是确定性的。继续将其与 DA-5 一起延伸至
  覆盖新的校样表面。
- 冷层驱逐政策需要签署审计跟踪以防止秘密删除。

### 承诺篡改

**场景：** 受损的定序器发布省略或更改 DA 的块
承诺，导致获取失败或轻客户端不一致。

**控制**
- 共识交叉检查区块提案与 DA 提交队列；同行拒绝
  提案缺少必要的承诺。
- 轻客户端在显示获取句柄之前验证承诺包含证明。
- 审计跟踪将提交收据与集体承诺进行比较。
- 自动调节作业 (`cargo xtask da-commitment-reconcile`) 比较
  摄取带有 DA 承诺的收据（SignedBlockWire、`.norito` 或 JSON），
  发出用于治理的 JSON 证据包，并且在丢失或失败时失败
  不匹配的票证，以便 Alertmanager 可以寻呼遗漏/篡改的信息。

**剩余间隙**
- 由对账作业+Alertmanager钩子覆盖；现在治理包
  默认情况下摄取 JSON 证据包。

### 网络分区和审查**场景：** 对手分割复制网络，阻止节点
获取分配的块或响应 PDP/PoTR 挑战。

**控制**
- 多区域提供商要求确保多样化的网络路径。
- 挑战窗口包括抖动和回退到带外修复通道。
- 可观察性仪表板监控复制深度、挑战成功以及
  获取带有警报阈值的延迟。

**剩余间隙**
- Taikai 现场活动的分区模拟仍然缺失；需要浸泡测试。
- 修复尚未编码的带宽预留策略。

### 内部滥用

**场景：** 具有注册表访问权限的操作员操纵保留策略，
将恶意提供商列入白名单，或抑制警报。

**控制**
- 治理行动需要多方签名和 Norito 公证记录。
- 策略更改将事件发送到监控和存档日志。
- 可观测性管道强制使用散列链接仅附加 Norito 日志。
- 季度访问审核自动化 (`cargo xtask da-privilege-audit`) 步行
  DA 清单/重播目录（加上操作员提供的路径）、标志
  缺少/非目录/世界可写条目，并发出签名的 JSON 包
  用于治理仪表板。

**剩余间隙**
- 仪表板防篡改需要签名快照。

## 残余风险登记册

|风险|可能性 |影响 |业主|缓解计划|
| ---| ---| ---| ---| ---|
| DA-2 序列缓存登陆之前重播 DA 清单 |可能 |中等|核心协议工作组 |在DA-2中实现序列缓存+nonce验证；添加回归测试。 |
|当 >f 个节点妥协时，PDP/PoTR 共谋 |不太可能 |高|存储团队|通过跨供应商抽样得出新的挑战时间表；通过模拟线束进行验证。 |
|冷层驱逐审计差距|可能 |高| SRE/存储团队 |附上已签名的审计日志和驱逐的链上收据；通过仪表板进行监控。 |
|定序器遗漏检测延迟 |可能 |高|核心协议工作组 |每晚 `cargo xtask da-commitment-reconcile` 会比较收据与承诺 (SignedBlockWire/`.norito`/JSON) 以及针对丢失或不匹配票证的页面治理。 |
| Taikai 直播的分区弹性 |可能 |关键|网络 TL |执行分区演练；预留修复带宽；文档故障转移 SOP。 |
|治理特权漂移|不太可能 |高|治理委员会|每季度 `cargo xtask da-privilege-audit` 运行（清单/重播目录 + 额外路径），带有签名 JSON + 仪表板门；将审计工件锚定在链上。 |

## 所需的后续行动

1. 发布 DA 摄取 Norito 模式和示例向量（纳入 DA-2）。
2. 通过 Torii DA 摄取重播缓存并在节点重新启动时保留序列游标。
3. **已完成 (2026-02-05)：** PDP/PoTR 模拟工具现在通过 QoS 积压建模来练习共谋 + 分区场景；请参阅 `integration_tests/src/da/pdp_potr.rs`（在 `integration_tests/tests/da/pdp_potr_simulation.rs` 下进行测试），了解下面捕获的实现和确定性摘要。
4. **已完成 (2026-05-29)：** `cargo xtask da-commitment-reconcile` 将摄取收据与 DA 承诺进行比较 (SignedBlockWire/`.norito`/JSON)，发出 `artifacts/da/commitment_reconciliation.json`，并连接到 Alertmanager/治理数据包中以发出遗漏/篡改警报（`xtask/src/da.rs`）。
5. **已完成 (2026-05-29)：** `cargo xtask da-privilege-audit` 遍历清单/重播假脱机（加上操作员提供的路径），标记缺失/非目录/全局可写条目，并生成用于仪表板/治理审查的签名 JSON 包 (`artifacts/da/privilege_audit.json`)，从而缩小访问审查自动化差距。

**下一步看哪里：**

- DA 重播缓存和光标持久性登陆 DA-2。请参阅
  `crates/iroha_core/src/da/replay_cache.rs`（缓存逻辑）中的实现和
  Torii 集成在 `crates/iroha_torii/src/da/ingest.rs` 中，该线程
  通过 `/v1/da/ingest` 进行指纹检查。
- PDP/PoTR 流模拟通过验证流工具进行
  `crates/sorafs_car/tests/sorafs_cli.rs`，涵盖 PoR/PDP/PoTR 请求流程
  以及威胁模型中动画的故障场景。
- 容量和修复浸泡结果如下
  `docs/source/sorafs/reports/sf2c_capacity_soak.md`，而更广泛的
  Sumeragi 浸泡矩阵在 `docs/source/sumeragi_soak_matrix.md` 中跟踪
  （包括本地化变体）。这些文物记录了长时间运行的演练
  残留风险登记册中引用。
- 协调+特权审计自动化存在
  `docs/automation/da/README.md` 和新的 `cargo xtask da-commitment-reconcile`
  /`cargo xtask da-privilege-audit` 命令；使用默认输出
  `artifacts/da/` 将证据附加到治理数据包时。

## 模拟证据和 QoS 建模 (2026-02)

为了结束 DA-1 后续#3，我们编写了确定性 PDP/PoTR 模拟
`integration_tests/src/da/pdp_potr.rs` 下的线束（由
`integration_tests/tests/da/pdp_potr_simulation.rs`）。安全带
跨三个区域分配节点，根据以下注入分区/共谋
路线图概率、跟踪 PoTR 延迟并提供维修积压
反映热门维修预算的模型。运行默认场景
（12 个 epoch，18 个 PDP 挑战 + 每个 epoch 2 个 PoTR 窗口）产生了
以下指标：

<!-- BEGIN_DA_SIM_TABLE -->
<!-- AUTO-GENERATED by scripts/docs/render_da_threat_model_tables.py; do not edit manually. -->
|公制|价值|笔记|
| ---| ---| ---|
|检测到 PDP 故障 | 48 / 49 (98.0%) |分区仍然触发检测；单个未检测到的故障来自诚实的抖动。 |
| PDP 平均检测延迟 | 0.0 纪元 |失败是在原始纪元内浮现出来的。 |
|检测到 PoTR 故障 | 28 / 77 (36.4%) |一旦节点错过 ≥2 个 PoTR 窗口，就会触发检测，将大多数事件留在残留风险寄存器中。 |
| PoTR 平均检测延迟 | 2.0时代|与归档升级中的两个纪元迟到阈值相匹配。 |
|抢修排队高峰| 38 份清单 |当分区堆叠速度快于每个时期可用的四次修复时，积压就会激增。 |
|响应延迟 p95 | 30,068 毫秒 |镜像 30 秒质询窗口，并应用 ±75 毫秒抖动进行 QoS 采样。 |
<!-- END_DA_SIM_TABLE -->

这些输出现在驱动 DA 仪表板原型并满足“模拟
路线图中引用的“利用+ QoS 建模”验收标准。

自动化现在位于 `cargo xtask da-threat-model-report [--out <path|->] [--seed <u64|0xhex>] [--config <path>]` 后面，它调用共享线束和
默认情况下，将 Norito JSON 发送到 `artifacts/da/threat_model_report.json`。每晚
作业使用此文件来刷新本文档中的矩阵并发出警报
检测率、修复队列或 QoS 样本的漂移。

要刷新上表的文档，请运行 `make docs-da-threat-model`，其中
调用 `cargo xtask da-threat-model-report`，重新生成
`docs/source/da/_generated/threat_model_report.json`，并重写本节
通过 `scripts/docs/render_da_threat_model_tables.py`。 `docs/portal` 镜子
(`docs/portal/docs/da/threat-model.md`) 在同一遍中更新，因此两者
副本保持同步。