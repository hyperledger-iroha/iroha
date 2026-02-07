---
id: preview-invite-flow
lang: zh-hans
direction: ltr
source: docs/portal/docs/devportal/preview-invite-flow.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Preview invite flow
sidebar_label: Preview invite flow
description: Sequencing, evidence, and communications plan for the docs portal public preview waves.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

## 目的

路线图项目 **DOCS-SORA** 呼吁审阅者加入和公共预览
在门户退出测试版之前，邀请程序作为最终的拦截者。本页
描述如何打开每个邀请波，哪些工件必须在之前发送
邀请出去，以及如何证明流程是可审计的。与它一起使用：

- [`devportal/reviewer-onboarding`](./reviewer-onboarding.md) 为
  每个审稿人的处理。
- [`devportal/preview-integrity-plan`](./preview-integrity-plan.md) 用于校验和
  保证。
- [`devportal/observability`](./observability.md) 用于遥测导出和
  警报钩子。

## 波浪计划

|波|观众|进入标准|退出标准 |笔记|
| ---| ---| ---| ---| ---|
| **W0 – 核心维护者** |文档/SDK 维护人员验证第一天的内容。 | `docs-portal-preview` GitHub 团队已填充，`npm run serve` 校验和门呈绿色，Alertmanager 已安静 7 天。 |所有 P0 文档均已审核，积压工作已标记，无阻塞事件。 |用于验证流程；没有邀请电子邮件，只需分享预览工件。 |
| **W1 – 合作伙伴** | SoraFS 运营商、Torii 集成商、NDA 下的治理审核员。 | W0 退出，法律条款获得批准，Try-it 代理上演。 |收集的合作伙伴签字（问题或签名表格），遥测显示 ≤10 个并发审核者，14 天内没有安全回归。 |强制执行邀请模板+请求门票。 |
| **W2 – 社区** |从社区候补名单中选定的贡献者。 | W1 退出，事件演习演练，公共常见问题解答更新。 |反馈已消化，≥2 个文档版本通过预览管道发布，无需回滚。 |并发邀请上限 (≤25) 和每周批量。 |

记录 `status.md` 和预览请求中哪个波形处于活动状态
跟踪器，以便治理可以一目了然地看到程序所在的位置。

## 飞行前检查表

**在**安排波次邀请之前完成以下操作：

1. **可用 CI 工件**
   - 最新的 `docs-portal-preview` + 描述符上传者
     `.github/workflows/docs-portal-preview.yml`。
   - `docs/portal/docs/devportal/deploy-guide.md` 中注明的 SoraFS 引脚
     （存在切换描述符）。
2. **校验和执行**
   - `docs/portal/scripts/serve-verified-preview.mjs` 通过调用
     `npm run serve`。
   - `scripts/preview_verify.sh` 指令在 macOS + Linux 上测试。
3. **遥测基线**
   - `dashboards/grafana/docs_portal.json` 显示健康的 Try it 流量和
     `docs.preview.integrity` 警报为绿色。
   - 最新的 `docs/portal/docs/devportal/observability.md` 附录更新为
     Grafana 链接。
4. **治理文物**
   - 邀请跟踪器问题已准备就绪（每波一个问题）。
   - 复制审阅者注册表模板（请参阅
     [`docs/examples/docs_preview_request_template.md`](../../../examples/docs_preview_request_template.md))。
   - 该问题附带法律和 SRE 所需的批准。

在发送任何邮件之前，在邀请跟踪器中记录预检完成情况。

## 流程步骤

1. **选择候选人**
   - 从候补名单电子表格或合作伙伴队列中提取。
   - 确保每位候选人都有完整的请求模板。
2. **批准访问**
   - 为邀请跟踪器问题分配审批者。
   - 验证先决条件（CLA/合同、可接受的用途、安全简介）。
3. **发送邀请**
   - 填写
     [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md)
     占位符（`<preview_tag>`、`<request_ticket>`、联系人）。
   - 附加描述符+存档哈希，尝试暂存 URL，并支持
     渠道。
   - 将最终电子邮件（或 Matrix/Slack 记录）存储在问题中。
4. **跟踪入职**
   - 使用 `invite_sent_at`、`expected_exit_at` 更新邀请跟踪器，以及
     状态（`pending`、`active`、`complete`、`revoked`）。
   - 链接到审稿人的可审核性接收请求。
5. **监控遥测**
   - 观看 `docs.preview.session_active` 和 `TryItProxyErrors` 警报。
   - 如果遥测偏离基线，则提交事件并记录
     邀请条目旁边的结果。
6. **收集反馈并退出**
   - 一旦收到反馈或 `expected_exit_at` 通过，即可关闭邀请。
   - 用简短的摘要更新浪潮问题（调查结果、事件、下一步
     行动），然后再进入下一个队列。

## 证据和报告

|文物|存放地点 |刷新节奏 |
| ---| ---| ---|
|邀请追踪器问题 | `docs-portal-preview` GitHub 项目 |每次邀请后更新。 |
|审稿人名册导出| `docs/portal/docs/devportal/reviewer-onboarding.md` 链接注册表 |每周。 |
|遥测快照 | `docs/source/sdk/android/readiness/dashboards/<date>/`（重用遥测包）|每波 + 事件发生后。 |
|反馈摘要| `docs/portal/docs/devportal/preview-feedback/<wave>/summary.md`（每波创建文件夹）|退出后 5 天内。 |
|治理会议记录| `docs/portal/docs/devportal/preview-invite-notes/<date>.md` |在每个 DOCS-SORA 治理同步之前填充。 |

运行 `cargo xtask docs-preview summary --wave <wave_label> --json artifacts/docs_portal_preview/<wave_label>_summary.json`
在每个批次之后生成机器可读的事件摘要。附上渲染图
JSON 到 Wave 问题，以便治理审核者可以确认邀请计数，而无需
重播整个日志。

每当一波结束时，将证据列表附加到 `status.md`，以便路线图
条目可以快速更新。

## 回滚和暂停条件

当发生以下任何情况时，暂停邀请流程（并通知治理）：

- 需要回滚的 Try it 代理事件 (`npm run manage:tryit-proxy`)。
- 警报疲劳：7 天内针对仅限预览的端点超过 3 个警报页面。
- 合规性差距：在未签署条款或未登录的情况下发送邀请
  请求模板。
- 完整性风险：`scripts/preview_verify.sh` 检测到校验和不匹配。

仅在邀请跟踪器中记录补救措施后才能恢复，并且
确认遥测仪表板至少稳定 48 小时。