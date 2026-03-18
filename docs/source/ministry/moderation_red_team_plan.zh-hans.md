---
lang: zh-hans
direction: ltr
source: docs/source/ministry/moderation_red_team_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7ecf637fa28f68a997367118163224caecc6c71c604d5e7ece409941d5374f44
source_last_modified: "2025-12-29T18:16:35.978869+00:00"
translation_last_reviewed: 2026-02-07
title: Ministry Red-Team & Chaos Drill Plan
summary: Execution plan for roadmap item MINFO-9 covering recurring adversarial campaigns, telemetry hooks, and reporting requirements.
translator: machine-google-reviewed
---

# 红队与混乱演习 (MINFO-9)

路线图参考：**MINFO-9 — 审核红队和混乱演习**（目标为 2026 年第 3 季度）

信息部必须开展可重复的对抗性活动，强调人工智能审核管道、赏金计划、网关和治理控制。该计划通过定义直接与现有审核仪表板 (`dashboards/grafana/ministry_moderation_overview.json`) 和紧急规范工作流程相关的范围、节奏、演练模板和证据要求，弥补了路线图中指出的“🈳 未开始”差距。

## 目标和可交付成果

- 在将覆盖范围扩大到其他威胁类别之前，进行季度红队演习，涵盖多部分走私企图、贿赂/上诉篡改和网关旁道探测。
- 捕获每次演练的确定性工件（CLI 日志、Norito 清单、Grafana 导出、SoraFS CID），并通过 `docs/source/ministry/reports/<YYYY-MM>-mod-red-team-<scenario>.md` 将其归档。
- 将演练输出反馈到校准清单 (`docs/examples/ai_moderation_calibration_*.json`) 和拒绝名单策略中，以便修复任务成为可追踪的路线图票证。
- 有线警报/运行手册集成，以便故障与 MINFO-1 仪表板和 Alertmanager 包 (`dashboards/alerts/ministry_moderation_rules.yml`) 一起出现。

## 范围和依赖关系

- **测试中的系统：** SoraFS 摄取/编排器路径、`docs/source/sorafs_ai_moderation_plan.md` 中定义的 AI 审核运行器、拒绝名单/Merkle 执行、申诉金库工具和网关速率限制。
- **先决条件：** 紧急规范和 TTL 策略 (`docs/source/ministry/emergency_canon_policy.md`)、调节校准装置和 Torii 模拟线束奇偶校验，以便在混乱运行期间可以重播可重现的有效负载。
- **超出范围：** SoraDNS 政策、Kaigi 会议或非部委通信渠道（在 SNNet 和 DOCS-SORA 计划下单独跟踪）。

## 角色和职责

|角色 |职责|主要所有者 |备份|
|------|--------------------|----------------|--------|
|钻探总监|批准场景列表、分配红队成员、签署运行手册 |部安全负责人|副主持人|
|对抗细胞 |制作有效负载、运行攻击、记录证据 |安防工程行业协会|志愿者经营者|
|可观察性主管 |监控仪表板/警报、捕获 Grafana 导出、记录事件时间表 | SRE / 可观察性 TL |待命 SRE |
|值班主持人|推动升级流程、验证覆盖请求、更新紧急规范记录 |事件指挥官|预备役指挥官 |
|报告抄写员|填充 `docs/source/ministry/reports/moderation_red_team_template.md` 下的模板、链接工件、打开后续问题 |文档/开发版本 |产品联络|

## 节奏和时间轴|相|目标窗口|主要活动|文物|
|--------|-------------|----------------|------------------------|
| **计划** | T−4 周 |选择场景，通过 `scripts/ministry/scaffold_red_team_drill.py` 刷新有效负载固定装置、脚手架工件，并试运行钻头镜像的遥测助手（`scripts/telemetry/check_redaction_status.py`、`ci/run_android_telemetry_chaos_prep.sh`）|场景简报、票务跟踪器 |
| **准备好** | T−1 周 |锁定参与者、阶段 SoraFS/Torii 沙箱、冻结仪表板/警报哈希 |准备好清单、仪表板摘要 |
| **执行** |演习日（4 小时）|启动对抗流程、收集 Alertmanager 通知、捕获 Torii/CLI 跟踪、强制执行覆盖批准 |实时日志、Grafana 快照 |
| **恢复** | T+1 天 |将覆盖恢复、清理数据集、将工件存档至 `artifacts/ministry/red-team/<YYYY-MM>/` 和 SoraFS |证据包，清单 |
| **报告** | T+1周|从模板发布 Markdown 报告、记录修复票证、更新 roadmap/status.md |报告文件、Jira/GitHub 链接 |

季度演习（三月/六月/九月/十二月）至少进行；高风险发现会触发遵循相同证据工作流程的临时运行。

## 场景库（初始）

|场景|描述 |成功信号|证据输入|
|----------|-------------|-----------------|------------------|
|多部分走私 |区块链遍布 SoraFS 提供商，其多态有效负载试图随着时间的推移绕过 AI 过滤器。练习 Orchestrator 范围获取、审核 TTL 和拒绝列表传播。 |用户交付前检测到走私；发出拒绝名单增量； `ministry_moderation_overview` 在 SLA 范围内发出火灾警报。 | CLI 重放日志、块清单、拒绝列表差异、来自 `sorafs.fetch.*` 仪表板的跟踪 ID。 |
|贿赂和上诉篡改|成对的恶意版主试图批准因贿赂而导致的越权行为；测试资金流量、推翻批准和审计日志记录。 |使用强制性证据记录覆盖，标记国库转移，记录治理投票。 | Norito 覆盖记录、`docs/source/ministry/volunteer_brief_template.md` 更新、国库分类账条目。 |
|网关旁路探测|模拟流氓发布商测量缓存时间和 TTL 以推断经过审核的内容。在 SNNet-15 之前练习 CDN/网关强化。 |速率限制和异常仪表板突出显示探针；管理 CLI 显示策略执行情况；没有内容泄露。 |网关访问日志、Grafana `ministry_gateway_observability` 面板的抓取、捕获数据包跟踪 (pcap) 以供离线查看。 |

一旦最初的三个场景从学习节奏中毕业，未来的迭代将添加 `honey-payload beacons`、`AI adversarial prompt floods` 和 `SoraFS metadata poisoning`。

## 执行清单1. **预钻孔**
   - 确认运行手册+场景文档已发布并获得批准。
   - 快照仪表板 (`dashboards/grafana/ministry_moderation_overview.json`) 和 `artifacts/ministry/red-team/<YYYY-MM>/dashboards/` 的警报规则。
   - 使用 `scripts/ministry/scaffold_red_team_drill.py` 创建报告 + 工件目录，然后记录为演练准备的任何夹具包的 SHA256 摘要。
   - 在注入对抗性有效负载之前验证拒绝名单 Merkle 根源和紧急规范注释。
2. **演习期间**
   - 将每个操作（时间戳、操作员、命令）记录到实时日志（共享文档或 `docs/source/ministry/reports/tmp/<timestamp>.md`）中。
   - 捕获 Torii 响应和 AI 审核裁决，包括请求 ID、模型名称和风险评分。
   - 执行升级工作流程（覆盖请求 → 指挥官批准 → `emergency_canon_policy` 更新）。
   - 触发至少一项警报清除活动和文档响应延迟。
3. **演习后**
   - 清除覆盖，将拒绝列表条目回滚到生产值，并验证警报是否安静。
   - 导出 Grafana/Alertmanager 历史记录、CLI 日志、Norito 清单，并将它们附加到证据包。
   - 向所有者提出补救问题（高/中/低）并注明截止日期；最终报告的链接。

## 遥测、指标和证据

- **仪表板：** `dashboards/grafana/ministry_moderation_overview.json` + 未来的 `ministry_red_team_heatmap.json`（占位符）捕获实时信号。每次钻取导出 JSON 快照。
- **警报：** `dashboards/alerts/ministry_moderation_rules.yml` 以及即将推出的 `ministry_red_team_rules.yml` 必须包含引用演练 ID 和场景的注释，以简化审核。
- **Norito 工件：** 将每个钻探运行编码为 `RedTeamDrillV1` 事件（规范即将推出），因此 Torii/CLI 导出是确定性的，并且可以与治理共享。
- **报告模板：** 复制 `docs/source/ministry/reports/moderation_red_team_template.md` 并填写场景、指标、证据摘要、补救状态和治理签核。
- **存档：** 将工件存储在 `artifacts/ministry/red-team/<YYYY-MM>/<scenario>/` 中（日志、CLI 输出、Norito 捆绑包、仪表板导出），并在治理批准后发布匹配的 SoraFS CAR 清单以供公众审查。

## 自动化和后续步骤1. 在 `scripts/ministry/` 下实施帮助程序脚本来播种有效负载固定装置、切换拒绝列表条目并收集 CLI/Grafana 导出。 （`scaffold_red_team_drill.py`、`moderation_payload_tool.py` 和 `check_red_team_reports.py` 现在涵盖脚手架、有效负载捆绑、拒绝名单修补和占位符强制执行；`export_red_team_evidence.py` 通过 Grafana API 支持添加了缺少的仪表板/日志导出，因此证据清单保持确定性。）
2. 使用 `ci/check_ministry_red_team.sh` 扩展 CI，以在合并报告之前验证模板完整性和证据摘要。 ✅（`scripts/ministry/check_red_team_reports.py` 强制删除所有已提交的钻探报告中的占位符。）
3. 将 `ministry_red_team_status` 部分添加到 `status.md`，以显示即将进行的演练、未完成的修复项目和上次运行指标。
4. 将钻探元数据集成到透明度管道中，以便季度报告可以参考最新的混乱结果。
5. 将演练报告直接输入 `cargo xtask ministry-transparency ingest --red-team-report <path>...`，​​以便经过净化的季度指标和治理清单将演练 ID、证据包和仪表板 SHA 与现有账本/申诉/拒绝列表源一起携带。

一旦这些步骤落地，MINFO-9 就会从🈳 未开始转变为 🈺 进行中，并具有可追踪的工件和可衡量的成功标准。