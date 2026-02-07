---
lang: zh-hans
direction: ltr
source: docs/source/docs_devrel/monthly_sync_agenda.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a2f89131efc0c79ddf63d71a25c04029014ba58393fb6336e676181322bc5066
source_last_modified: "2025-12-29T18:16:35.952009+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Docs/DevRel 每月同步议程

该议程正式确定了每月 Docs/DevRel 同步，该同步被引用
`roadmap.md`（请参阅“将本地化人员配置审核添加到每月 Docs/DevRel
同步”）和 Android AND5 i18n 计划。使用它作为规范清单，并且
每当路线图可交付成果添加或取消议程项目时更新它。

## 节奏与物流

- **频率：** 每月（通常是第二个星期四，16:00UTC）
- **持续时间：** 45 分钟 + 可选 15 分钟深潜停留
- **位置：** Zoom (`https://meet.sora.dev/docs-devrel-sync`) 与共享
  HackMD 或 `docs/source/docs_devrel/minutes/<yyyy-mm>.md` 中的注释
- **观众：** 文档/DevRel 经理（主席）、文档工程师、本地化
  程序经理、SDK DX TL（Android、Swift、JS）、产品文档、发布
  工程代表、支持/QA 观察员
- **协调人：** 文档/DevRel 经理；任命一名轮流抄写员，他将
  在 24 小时内将会议纪要提交到存储库中

## 工作前检查表

|业主|任务|文物|
|--------|------|----------|
|抄写员|使用下面的模板创建本月的笔记文件 (`docs/source/docs_devrel/minutes/<yyyy-mm>.md`)。 |笔记文件 |
|本地化项目经理 |刷新`docs/source/sdk/android/i18n_plan.md#translation-status`和人员配置日志；预先填写提议的决定。 | i18n 计划 |
| DX TL |运行 `ci/check_android_docs_i18n.sh` 或 `scripts/sync_docs_i18n.py --dry-run` 并附加摘要以供讨论。 | CI 文物 |
|文档工具 |从 `docs/source/sdk/android/i18n_requests/` 导出 `docs/i18n/manifest.json` 摘要 + 未完成票据列表。 |舱单和票证摘要 |
|支持/发布 |收集需要 Docs/DevRel 操作的任何升级（例如，等待预览邀请、阻止审阅者反馈）。 | Status.md 或升级文档 |

## 议程块1. **点名和目标（5 分钟）**
   - 确认法定人数、抄写员和后勤。
   - 突出显示任何紧急事件（文档预览中断、本地化阻止）。
2. **本地化人员配置审查（15分钟）**
   - 登录查看人员配置决策
     `docs/source/sdk/android/i18n_plan.md#staffing-decision-log`。
   - 确认未结采购订单 (`DOCS-L10N-*`) 的状态和临时覆盖范围。
   - 比较 CI 新鲜度输出与翻译状态表；呼出任何
     其区域设置 SLA（>5 个工作日）将在下一个工作日之前被违反的文档
     同步。
   - 决定是否需要升级（产品运营、财务、承包商
     管理）。将决定记录在人员配置日志和月度日志中
     分钟，包括所有者 + 截止日期。
   - 如果人员配备状况良好，请记录确认信息，以便路线图行动能够
     带证据回到🈺/🈴。
3. **文档/路线图更新（10 分钟）**
   - DOCS-SORA 门户工作、Try-It 代理和 SoraFS 发布的状态
     准备状态。
   - 突出显示当前版本列车所需的文档债务或审阅者。
4. **SDK 亮点（10 分钟）**
   - Android AND5/AND7 文档准备情况、Swift IOS5 奇偶校验、JS GA 进度。
   - 捕获将影响文档的共享装置或架构差异。
5. **行动回顾和停车场（5分钟）**
   - 重新访问上次同步中的未清项目；确认关闭。
   - 在笔记文件中记录新操作，并明确所有者和截止日期。

## 本地化人员配置审核模板

在每月的会议记录中包含下表：

|语言环境 |容量（全职员工）|承诺和采购订单 |风险/升级|决策与所有者 |
|--------|----------------|--------------------|--------------------------------|--------------------|
|日本 |例如，0.5 个承包商 + 0.1 个文档备份 | PO `DOCS-L10N-4901`（等待签名）| “2026 年 3 月 4 日之前未签署合同” | “升级至产品运营 — @docs-devrel，截止日期为 2026 年 3 月 2 日”|
|他|例如，0.1 文档工程师 |轮换进入 PTO 2026-03-18 | “需要后备审稿人” | “@docs-lead 将在 2026 年 3 月 5 日之前确定备份”|

还记录一个简短的叙述，内容包括：

- **SLA 展望：** 任何预计会错过五个工作日 SLA 和
  缓解措施（交换优先级、招募备份供应商等）。
- **门票和资产健康状况：** 未完成的条目
  `docs/source/sdk/android/i18n_requests/` 以及屏幕截图/资产是否
  为译者做好准备。

### 本地化人员配置审核日志记录

- **会议记录：** 将人员配置表 + 叙述复制到
  `docs/source/docs_devrel/minutes/<yyyy-mm>.md`（所有语言环境均镜像
  英语分钟通过同一目录下的本地化文件）。链接条目
  回到议程（`docs/source/docs_devrel/monthly_sync_agenda.md`）所以
  治理可以追踪证据。
- **国际化计划：**更新人员配置决策日志和翻译状态表
  会议结束后立即在 `docs/source/sdk/android/i18n_plan.md` 中。
- **状态：** 当人员配置决策影响路线图大门时，请在
  `status.md`（Docs/DevRel 部分）引用分钟文件和 i18n 计划
  更新。

## 会议纪要模板

将此骨架复制到 `docs/source/docs_devrel/minutes/<yyyy-mm>.md` 中：

```markdown
<!-- SPDX-License-Identifier: Apache-2.0 -->

# Docs/DevRel Monthly Sync — 2026-03-12

## Attendees
- Chair: …
- Scribe: …
- Participants: …

## Agenda Notes
1. Roll call & objectives — …
2. Localization staffing review — include table + narrative.
3. Docs/roadmap updates — …
4. SDK highlights — …
5. Action review & parking lot — …

## Decisions & Actions
| Item | Owner | Due | Notes |
|------|-------|-----|-------|
| JP contractor PO follow-up | @docs-devrel-manager | 2026-03-02 | Example entry |
```会议结束后立即通过 PR 发布笔记，并从 `status.md` 链接它们
在参考风险或人员配置决策时。

## 后续预期

1. **承诺的分钟数：** 24 小时内 (`docs/source/docs_devrel/minutes/`)。
2. **国际化计划更新：**调整人员配置日志和翻译表
   反映新的承诺或升级。
3. **Status.md 条目：** 总结任何高风险决策以保留路线图
   同步。
4. **提交升级：** 当审核要求升级时，创建/刷新
   相关票据（例如产品运营、财务审批、供应商入职）
   并在会议记录和 i18n 计划中引用它。

通过遵循此议程，路线图要求包括本地化
Docs/DevRel 每月同步中的人员配备审核保持可审计，并且下游
团队总是知道在哪里可以找到证据。