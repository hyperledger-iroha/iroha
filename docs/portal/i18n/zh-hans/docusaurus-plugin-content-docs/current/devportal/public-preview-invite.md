---
id: public-preview-invite
lang: zh-hans
direction: ltr
source: docs/portal/docs/devportal/public-preview-invite.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Public preview invite playbook
sidebar_label: Preview invite playbook
description: Checklist for announcing the docs portal preview to external reviewers.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

## 计划目标

本手册解释了如何在发布后宣布并运行公共预览版
审阅者入职工作流程已上线。它通过以下方式保持 DOCS-SORA 路线图的诚实性：
确保每一次邀请都附带可验证的物品、安全指南和
清晰的反馈路径。

- **观众：** 精选的社区成员、合作伙伴和维护者名单
  签署预览可接受使用政策。
- **上限：** 默认 Wave 大小 ≤ 25 名审阅者、14 天访问窗口、事件
  24小时内回复。

## 启动门控清单

在发送任何邀请之前完成这些任务：

1. CI 中上传的最新预览工件（`docs-portal-preview`，
   校验和清单、描述符、SoraFS 捆绑包）。
2. `npm run --prefix docs/portal serve`（校验和门控）在同一标签上进行测试。
3. 审阅者入职票据已获得批准并链接到邀请波。
4. 安全性、可观察性和事件文档经过验证
   ([`security-hardening`](./security-hardening.md),
   [`observability`](./observability.md),
   [`incident-runbooks`](./incident-runbooks.md))。
5. 准备反馈表或问题模板（包括严重性字段、
   重现步骤、屏幕截图和环境信息）。
6. 公告副本由 Docs/DevRel + 治理审核。

## 邀请包

每份邀请必须包括：

1. **已验证的工件** — 提供 SoraFS 清单/计划或 GitHub 工件
   链接加上校验和清单和描述符。参考验证
   明确命令，以便审阅者可以在启动站点之前运行它。
2. **服务说明** — 包括校验和门控预览命令：

   ```bash
   DOCS_RELEASE_TAG=preview-<stamp> npm run --prefix docs/portal serve
   ```

3. **安全提醒** — 注明令牌自动过期，链接
   不得共享，并应立即报告事件。
4. **反馈渠道** — 链接到问题模板/表格并澄清回复
   时间预期。
5. **计划日期** — 提供开始/结束日期、办公时间或同步会议，
   和下一个刷新窗口。

示例电子邮件位于
[`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md)
涵盖了这些要求。更新占位符（日期、URL、联系人）
发送之前。

## 暴露预览主机

仅在入职完成并更改票证后才推广预览主机
已获批准。参见【预览主机曝光指南】(./preview-host-exposure.md)
了解本节中使用的端到端构建/发布/验证步骤。

1. **构建并打包：** 标记发布标签并产生确定性
   文物。

   ```bash
   cd docs/portal
   export DOCS_RELEASE_TAG="preview-$(date -u +%Y%m%dT%H%M%SZ)"
   npm ci
   npm run build
   ./scripts/sorafs-pin-release.sh \
     --alias docs-preview.sora \
     --alias-namespace docs \
     --alias-name preview \
     --pin-label docs-preview \
     --skip-submit
   node scripts/generate-preview-descriptor.mjs \
     --manifest artifacts/checksums.sha256 \
     --archive artifacts/sorafs/portal.tar.gz \
     --out artifacts/sorafs/preview-descriptor.json
   ```

   pin脚本写入`portal.car`，`portal.manifest.*`，`portal.pin.proposal.json`，
   以及 `artifacts/sorafs/` 下的 `portal.dns-cutover.json`。将这些文件附加到
   邀请 Wave，以便每个审阅者都可以验证相同的位。

2. **发布预览别名：** 重新运行不带 `--skip-submit` 的命令
   （供应 `TORII_URL`、`AUTHORITY`、`PRIVATE_KEY[_FILE]` 和
   治理颁发的别名证明）。该脚本将清单绑定到
   `docs-preview.sora` 并发出 `portal.manifest.submit.summary.json` plus
   `portal.pin.report.json` 用于证据包。

3. **探测部署：** 确认别名解析且校验和匹配
   发送邀请之前的标签。

   ```bash
   npm run probe:portal -- \
     --base-url=https://docs-preview.sora.link \
     --expect-release="$DOCS_RELEASE_TAG"
   ```

   将 `npm run serve` (`scripts/serve-verified-preview.mjs`) 放在手边，作为
   回退，以便审阅者可以在预览边缘闪烁时启动本地副本。

## 通讯时间表

|日 |行动|业主|
| ---| ---| ---|
| D-3 |完成邀请副本、刷新工件、试运行验证 |文档/开发版本 |
| D-2 |治理签字+变更票|文档/DevRel + 治理 |
| D-1 |使用模板发送邀请，使用收件人列表更新跟踪器 |文档/开发版本 |
| d |启动电话会议/办公时间，监控遥测仪表板 |文档/开发版本 + 待命 |
| D+7 |中点反馈摘要、分类阻塞问题|文档/开发版本 |
| D+14 |关闭wave，撤销临时访问，在 `status.md` 中发布摘要 |文档/开发版本 |

## 访问跟踪和遥测

1. 记录每个收件人、邀请时间戳和撤销日期
   预览反馈记录器（参见
   [`preview-feedback-log`](./preview-feedback-log)) 所以每个波共享
   相同的证据线索：

   ```bash
   # Append a new invite event to artifacts/docs_portal_preview/feedback_log.json
   npm run --prefix docs/portal preview:log -- \
     --wave preview-20250303 \
     --recipient alice@example.com \
     --event invite-sent \
     --notes "wave-01 seed"
   ```

   支持的事件有 `invite-sent`、`acknowledged`、
   `feedback-submitted`、`issue-opened` 和 `access-revoked`。日志位于
   默认为`artifacts/docs_portal_preview/feedback_log.json`；将其附加到
   邀请波票以及同意书。使用摘要助手
   在结账前生成可审计的汇总：

   ```bash
   npm run --prefix docs/portal preview:summary -- --summary-json \
     > artifacts/docs_portal_preview/preview-20250303-summary.json
   ```

   JSON 摘要枚举每波邀请、开放收件人、反馈
   计数以及最近事件的时间戳。助手的支持者是
   [`scripts/preview-feedback-log.mjs`](../../scripts/preview-feedback-log.mjs),
   因此相同的工作流程可以在本地或 CI 中运行。使用摘要模板
   [`docs/examples/docs_preview_feedback_digest.md`](../../../examples/docs_preview_feedback_digest.md)
   发布浪潮回顾时。
2. 使用用于波形的 `DOCS_RELEASE_TAG` 标记遥测仪表板，以便
   峰值可以与邀请群体相关。
3.部署后运行`npm run probe:portal -- --expect-release=<tag>`
   确认预览环境公布了正确的版本元数据。
4. 捕获运行手册模板中的所有事件并将其链接到群组。

## 反馈和结束

1. 在共享文档或问题板上汇总反馈。为项目添加标签
   `docs-preview/<wave>`，以便路线图所有者可以轻松查询它们。
2. 使用预览记录器的摘要输出来填充波形报告，然后
   总结 `status.md` 中的队列（参与者、主要发现、计划
   修复）并更新 `roadmap.md`（如果 DOCS-SORA 里程碑发生更改）。
3. 按照以下的卸载步骤操作
   [`reviewer-onboarding`](./reviewer-onboarding.md)：撤销访问、存档
   请求，并感谢参与者。
4. 通过刷新工件、重新运行校验和门来准备下一波，
   并使用新日期更新邀请模板。

始终如一地应用此剧本可以使预览程序保持可审核性
随着门户接近正式发布，为 Docs/DevRel 提供了一种可重复的方式来扩展邀请。