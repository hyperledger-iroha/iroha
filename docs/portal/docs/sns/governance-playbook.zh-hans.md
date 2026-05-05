---
lang: zh-hans
direction: ltr
source: docs/portal/docs/sns/governance-playbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: aa4560c51d066ce5c63581dd102aef4e70786d140790fb157323df2553b15f4b
source_last_modified: "2026-01-28T17:11:30.700959+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->
---
id：治理手册
标题：Sora 名称服务治理手册
sidebar_label：治理手册
描述：SN-1/SN-6 引用的理事会、监护人、管理员和注册员工作流程运行手册。
---

:::注意规范来源
此页面镜像 `docs/source/sns/governance_playbook.md`，现在用作
规范门户副本。翻译 PR 的源文件仍然存在。
:::

# Sora 名称服务治理手册 (SN-6)

**状态：** 起草于 2026 年 3 月 24 日 — SN-1/SN-6 准备情况的实时参考  
**路线图链接：** SN-6“合规性和争议解决”、SN-7“解析器和网关同步”、ADDR-1/ADDR-5 地址政策  
**先决条件：** [`registry-schema.md`](./registry-schema.md) 中的注册架构、[`registrar-api.md`](./registrar-api.md) 中的注册商 API 合约、[`address-display-guidelines.md`](./address-display-guidelines.md) 中的地址用户体验指南以及账户结构[`docs/account_structure.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/account_structure.md) 中的规则。

本手册描述了 Sora 名称服务 (SNS) 治理机构如何采用
章程、批准注册、升级争议并证明解决者和
网关状态保持同步。它满足了路线图的要求
`sns governance ...` CLI、Norito 清单和审核工件共享一个
N1（公开发布）之前面向运营商的参考。

## 1. 范围和受众

该文件的目标是：

- 对章程、后缀政策和争议进行投票的治理委员会成员
  结果。
- 发出紧急冻结和审查撤销的监护委员会成员。
- 后缀管理员，负责管理注册队列、批准拍卖和管理
  收入分成。
- 解析器/网关运营商负责 SoraDNS 传播、GAR 更新、
  和遥测护栏。
- 合规、财务和支持团队必须证明每个
  治理行动留下了可审计的 Norito 文物。

它涵盖内测 (N0)、公开发布 (N1) 和扩展 (N2) 阶段
通过将每个工作流程链接到所需的证据，在 `roadmap.md` 中枚举，
仪表板和升级路径。

## 2. 角色和联系图

|角色 |核心职责|主要文物和遥测|升级 |
|------|----------------------|--------------------------------|------------|
|治理委员会|起草和批准章程、后缀政策、争议裁决和管家轮换。 | `docs/source/sns/governance_addenda/`、`artifacts/sns/governance/*`，通过 `sns governance charter submit` 存储的议会选票。 |理事会主席+治理日程跟踪器。 |
|监护委员会|发布软/硬冻结、紧急规则和 72 小时审查。 | `sns governance freeze` 发出的监护人票证，覆盖 `artifacts/sns/guardian/*` 下记录的清单。 |监护人值班轮换（≤15 分钟 ACK）。 |
|后缀管家|运行注册商队列、拍卖、定价等级和客户通信；确认合规性。 | `SuffixPolicyV1` 中的管家政策、定价参考表、存储在监管备忘录旁边的管家致谢。 | Steward 程序主线 + 后缀特定的 PagerDuty。 |
|注册商和计费操作 |操作 `/v1/sns/*` 端点、协调付款、发出遥测数据并维护 CLI 快照。 |注册商 API（[`registrar-api.md`](./registrar-api.md)）、`sns_registrar_status_total` 指标、在 `artifacts/sns/payments/*` 下存档的付款证明。 |登记官值班经理和财务联络人。 |
|解析器和网关运营商 |保持 SoraDNS、GAR 和网关状态与注册商事件保持一致；流透明度指标。 | [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md)、[`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md)、`dashboards/alerts/soradns_transparency_rules.yml`。 |解析器 SRE 待命 + 网关操作桥接。 |
|财务与金融 |应用 70/30 的收入分成、推荐分拆、税务/财务申报和 SLA 证明。 |应计收入清单、Stripe/国库出口、`docs/source/sns/regulatory/` 下的季度 KPI 附录。 |财务总监+合规官。 |
|合规与监管联络|跟踪全球义务（EU DSA 等）、更新 KPI 契约并归档披露。 | `docs/source/sns/regulatory/` 中的监管备忘录、参考资料、`ops/drill-log.md` 桌面排练条目。 |合规计划负责人。 |
|支持/SRE 待命 |处理事件（冲突、计费偏差、解析器中断）、协调客户消息传递和自己的操作手册。 |事件模板、`ops/drill-log.md`、上演的实验室证据、在 `incident/` 下存档的 Slack/作战室记录。 | SNS on-call 轮换+SRE 管理。 |

## 3. 规范工件和数据源

|文物|地点 |目的|
|----------|----------|---------|
|章程 + KPI 附录 | `docs/source/sns/governance_addenda/` | CLI 投票引用的版本控制的签署章程、KPI 契约和治理决策。 |
|注册表架构| [`registry-schema.md`](./registry-schema.md) |规范 Norito 结构（`NameRecordV1`、`SuffixPolicyV1`、`RevenueAccrualEventV1`）。 |
|注册商合同 | [`registrar-api.md`](./registrar-api.md) | REST/gRPC 负载、`sns_registrar_status_total` 指标和治理挂钩期望。 |
|地址用户体验指南 | [`address-display-guidelines.md`](./address-display-guidelines.md) | Canonical I105（首选）+ 压缩（`sora`，第二好）钱包/浏览器镜像的渲染。 |
| SoraDNS / GAR 文档 | [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md), [`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md) |确定性主机派生、透明尾部工作流程和警报规则。 |
|监管备忘录| `docs/source/sns/regulatory/` |司法管辖区接收说明（例如，EU DSA）、管理员确认、模板附件。 |
|钻井日志| `ops/drill-log.md` |阶段退出前需要记录混乱情况和 IR 排练。 |
|文物存储 | `artifacts/sns/` |付款证明、监护人票证、解析器差异、KPI 导出以及 `sns governance ...` 生成的签名 CLI 输出。 |

所有治理行动必须至少引用上表中的一项工件
因此审计人员可以在 24 小时内重建决策轨迹。

## 4. 生命周期手册

### 4.1 章程和管理员动议

|步骤|业主| CLI / 证据 |笔记|
|------|------|----------------|--------|
|附录草案和 KPI 增量 |理事会报告员 + 管家领导 |存储在 `docs/source/sns/governance_addenda/YY/` 下的 Markdown 模板 |包括 KPI 契约 ID、遥测挂钩和激活条件。 |
|提交提案 |理事会主席 | `sns governance charter submit --input SN-CH-YYYY-NN.md`（生成 `CharterMotionV1`）| CLI 发出存储在 `artifacts/sns/governance/<id>/charter_motion.json` 下的 Norito 清单。 |
|投票和监护人致谢|理事会+监护人| `sns governance ballot cast --proposal <id>` 和 `sns governance guardian-ack --proposal <id>` |附上哈希会议记录和法定人数证明。 |
|管家验收|管家计划| `sns governance steward-ack --proposal <id> --signature <file>` |后缀策略更改前需要；将信封记录在 `artifacts/sns/governance/<id>/steward_ack.json` 下。 |
|激活 |注册商操作 |更新 `SuffixPolicyV1`，刷新注册商缓存，在 `status.md` 中发布说明。 |激活时间戳记录到 `sns_governance_activation_total`。 |
|审核日志|合规|如果执行了桌面操作，则将条目附加到 `docs/source/sns/regulatory/<jurisdiction>/<cycle>.md` 和演练日志中。 |包括对遥测仪表板和策略差异的引用。 |

### 4.2 注册、拍卖和定价批准

1. **预检：** 注册商查询 `SuffixPolicyV1` 以确认定价等级，
   可用条款和宽限/赎回窗口。保持定价表同步
   3/4/5/6–9/10+ 等级表（基本等级 + 后缀系数）记录在
   路线图。
2. **密封投标拍卖：** 对于高级池，运行 72 小时承诺/24 小时揭示
   通过 `sns governance auction commit` / `... reveal` 循环。发布提交
   在 `artifacts/sns/auctions/<name>/commit.json` 下列出（仅哈希）所以
   审计员可以验证随机性。
3. **付款验证：** 注册商验证 `PaymentProofV1`
   财务分割（70% 财务/30% 管理人员，≤10% 推荐剥离）。
   将 Norito JSON 存储在 `artifacts/sns/payments/<tx>.json` 下并将其链接到
   注册商回复 (`RevenueAccrualEventV1`)。
4. **治理挂钩：** 附加 `GovernanceHookV1` 以获取高级/受保护的名称
   参考理事会提案 ID 和管理员签名。缺少挂钩结果
   在 `sns_err_governance_missing` 中。
5. **激活+解析器同步：** Torii发出注册表事件后，触发
   解析器透明度尾部确认传播的新 GAR/区域状态
   （参见第 4.5 节）。
6. **客户披露：** 更新面向客户的账本（钱包/资源管理器）
   通过[`address-display-guidelines.md`](./address-display-guidelines.md)中的共享夹具，确保i105和
   压缩效果图与文案/QR 指南相符。

### 4.3 续订、计费和财务对账- **续订工作流程：** 注册商强制执行 30 天宽限期 + 60 天赎回期
  `SuffixPolicyV1` 中指定的窗口。 60 天后荷兰重新开放序列
  （7 天，10× 费用衰减 15%/天）通过 `sns 治理自动触发
  重新打开`。
- **收入分配：** 每次续订或转让都会创建一个
  `RevenueAccrualEventV1`。国库出口（CSV/Parquet）必须与
  这些事件每天都有；将校样附在 `artifacts/sns/treasury/<date>.json` 上。
- **推荐剥离：** 每个后缀跟踪可选的推荐百分比
  通过将 `referral_share` 添加到管理员策略中。注册商发出最终的
  将推荐清单拆分并存储在付款证明旁边。
- **报告节奏：** 财务发布每月 KPI 附件（注册、
  续订、ARPU、争议/债券利用）
  `docs/source/sns/regulatory/<suffix>/YYYY-MM.md`。仪表板应该从
  相同的导出表，因此 Grafana 数字与分类帐证据匹配。
- **每月 KPI 审核：** 第一个周二检查点与财务主管配对，
  值班管家和项目 PM。打开[SNS KPI仪表板](./kpi-dashboard.md)
  （`sns-kpis` / `dashboards/grafana/sns_suffix_analytics.json` 的门户嵌入），
  导出附件中的注册商吞吐量+收入表、日志增量，
  并将文物附加到备忘录中。如果审核发现，则触发事件
  违反 SLA（冻结窗口 >72 小时、注册商错误峰值、ARPU 漂移）。

### 4.4 冻结、争议和上诉

|相|业主|行动与证据|服务水平协议 |
|--------|--------|--------------------|-----|
|软冻结请求 |管家/支持|将票据 `SNS-DF-<id>` 归档，其中包含付款证明、争议保证金参考和受影响的选择器。 |摄入后≤4小时。 |
|监护人票|监护板| `sns governance freeze --selector <i105> --reason <text> --until <ts>` 生成签名的 `GuardianFreezeTicketV1`。将票证 JSON 存储在 `artifacts/sns/guardian/<id>.json` 下。 | ≤30min ACK，≤2h 执行。 |
|理事会批准 |治理委员会|批准或拒绝冻结，将决定链接记录到监护人票证和争议债券摘要。 |下次理事会会议或异步投票。 |
|仲裁小组|合规+管家|召集 7 名陪审员小组（根据路线图），并通过 `sns governance dispute ballot` 提交哈希选票。将匿名投票收据附加到事件数据包中。 |保证金存入后≤7 天作出判决。 |
|上诉|监护人+理事会|上诉使保证金加倍并重复陪审员程序；记录 Norito 清单 `DisputeAppealV1` 和参考主票证。 | ≤10天。 |
|解冻和修复|注册商 + 解析器操作 |执行 `sns governance unfreeze --selector <i105> --ticket <id>`，更新注册器状态并传播 GAR/解析器差异。 |判决后立即。 |

紧急大炮（监护人触发的冻结≤72小时）遵循相同的流程，但
要求理事会进行追溯审查并提供透明度说明
`docs/source/sns/regulatory/`。

### 4.5 解析器和网关传播

1. **事件挂钩：** 每个注册表事件都会发送到解析器事件流
   （`tools/soradns-resolver` SSE）。解析器操作通过以下方式订阅和记录差异
   透明度裁剪器 (`scripts/telemetry/run_soradns_transparency_tail.sh`)。
2. **GAR 模板更新：** 网关必须更新引用的 GAR 模板
   `canonical_gateway_suffix()` 并重新签署 `host_pattern` 列表。存储差异
   在 `artifacts/sns/gar/<date>.patch` 中。
3. **Zonefile发布：** 使用中描述的zonefile骨架
   `roadmap.md`（名称、ttl、cid、证明）并将其推送到 Torii/SoraFS。归档
   `artifacts/sns/zonefiles/<name>/<version>.json` 下的 Norito JSON。
4. **透明度检查：** 运行 `promtool test rules dashboards/alerts/tests/soradns_transparency_rules.test.yml`
   以确保警报保持绿色。将 Prometheus 文本输出附加到
   每周透明度报告。
5. **网关审计：**记录`Sora-*`标头样本（缓存策略、CSP、GAR
   摘要）并将它们附加到治理日志中，以便操作员可以证明
   网关为新名称提供了预期的护栏。

## 5. 遥测和报告

|信号|来源 |描述/行动|
|--------|--------|----------------------|
| `sns_registrar_status_total{result,suffix}` | Torii 注册商处理程序 |注册、续订、冻结、转让的成功/错误计数器；当每个后缀的 `result="error"` 峰值时发出警报。 |
| `torii_request_duration_seconds{route="/v1/sns/*"}` | Torii 指标 | API 处理程序的延迟 SLO；从 `torii_norito_rpc_observability.json` 构建的提要仪表板。 |
| `soradns_bundle_proof_age_seconds` 和 `soradns_bundle_cid_drift_total` |旋转变压器透明度裁剪器 |检测过时的证明或 GAR 漂移； `dashboards/alerts/soradns_transparency_rules.yml` 中定义的护栏。 |
| `sns_governance_activation_total` |治理 CLI |每当章程/附录激活时，计数器就会递增；用于协调理事会决定与已发布的附录。 |
| `guardian_freeze_active` 仪表 |守护者 CLI |跟踪每个选择器的软/硬冻结窗口；如果值保持 `1` 超出声明的 SLA，则页 SRE。 |
| KPI 附件仪表板 |财务/文件|每月汇总与监管备忘录一起发布；该门户通过 [SNS KPI 仪表板](./kpi-dashboard.md) 嵌入它们，以便管理员和监管者可以访问相同的 Grafana 视图。 |

## 6. 证据和审计要求

|行动|归档证据|存储|
|--------|--------------------|---------|
|章程/政策变更|签署的 Norito 清单、CLI 记录、KPI 差异、管理员确认。 | `artifacts/sns/governance/<proposal-id>/` + `docs/source/sns/governance_addenda/`。 |
|注册/续订| `RegisterNameRequestV1` 有效负载，`RevenueAccrualEventV1`，付款证明。 | `artifacts/sns/payments/<tx>.json`，注册商 API 日志。 |
|拍卖|提交/揭示清单、随机种子、获胜者计算电子表格。 | `artifacts/sns/auctions/<name>/`。 |
|冻结/解冻 |监护人票证、理事会投票哈希值、事件日志 URL、客户通信模板。 | `artifacts/sns/guardian/<ticket>/`、`incident/<date>-sns-*.md`。 |
|解析器传播 | Zonefile/GAR diff、tailer JSONL 摘录、Prometheus 快照。 | `artifacts/sns/resolver/<date>/` + 透明度报告。 |
|监管摄入|接收备忘录、截止日期跟踪器、管理员确认、KPI 变更摘要。 | `docs/source/sns/regulatory/<jurisdiction>/<cycle>.md`。 |

## 7. 阶段门检查表

|相|退出标准|证据包|
|--------|-------------|-----------------|
| N0 — 内测 | SN-1/SN-2 注册表架构、手动注册器 CLI、监护演练完成。 |章程动议 + 管理员 ACK、注册商试运行日志、解析器透明度报告、`ops/drill-log.md` 中的演练条目。 |
| N1 — 公开发布 | `.sora`/`.nexus` 的拍卖 + 固定价格层级、自助服务注册商、解析器自动同步、计费仪表板。 |定价表差异、注册商 CI 结果、付款/KPI 附件、透明度定制输出、事件排练记录。 |
| N2 — 扩展 | `.dao`、经销商 API、争议门户、管理员记分卡、分析仪表板。 |门户屏幕截图、争议 SLA 指标、管家记分卡导出、参考经销商政策的更新治理章程。 |

阶段退出需要记录桌面演练（注册快乐路径、冻结、
解析器中断），文物附加到 `ops/drill-log.md`。

## 8. 事件响应和升级

|触发|严重性 |直接所有者 |强制行动 |
|--------|----------|-----------------|--------------------|
|解析器/GAR 漂移或陈旧证明 |严重程度1 |解析器SRE+监护板|页面解析器待命，捕获尾部输出，决定是否冻结受影响的名称，每 30 分钟发布状态更新一次。 |
|注册商中断、计费失败或广泛的 API 错误 |严重程度1 |登记员值班经理 |停止新的拍卖，切换到手动 CLI，通知管理员/财务人员，将 Torii 日志附加到事件文档中。 |
|单名争议、付款不匹配或客户升级 |严重程度2 |管家 + 支持领导 |收集付款证明，确定是否需要软冻结，在 SLA 范围内响应请求者，在争议跟踪器中记录结果。 |
|合规审计结果|严重程度2 |合规联络|起草补救计划，在 `docs/source/sns/regulatory/` 下提交备忘录，安排后续理事会会议。 |
|演习或排练|严重程度3 |节目PM |执行 `ops/drill-log.md` 中的脚本场景、归档工件、将差距标记为路线图任务。 |

所有事件都必须创建具有所有权的 `incident/YYYY-MM-DD-sns-<slug>.md`
表格、命令日志以及对整个过程中产生的证据的引用
剧本。

## 9. 参考文献

- [`registry-schema.md`](./registry-schema.md)
- [`registrar-api.md`](./registrar-api.md)
- [`address-display-guidelines.md`](./address-display-guidelines.md)
- [`docs/account_structure.md`](../../../account_structure.md)
- [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md)
- [`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md)
- `ops/drill-log.md`
- `roadmap.md`（SNS、DG、ADDR 部分）

每当章程措辞、CLI 出现或遥测时，请随时更新此手册
合同变更；引用 `docs/source/sns/governance_playbook.md` 的路线图条目
应始终匹配最新版本。