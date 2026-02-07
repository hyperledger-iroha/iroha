---
lang: zh-hans
direction: ltr
source: docs/portal/docs/devportal/reviewer-onboarding.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f42888a06cb49f9fe53f424ef77c84e2fa3a305f558e202be0fbbd4b3b0ea1d7
source_last_modified: "2025-12-29T18:16:35.114535+00:00"
translation_last_reviewed: 2026-02-07
id: reviewer-onboarding
title: Preview reviewer onboarding
sidebar_label: Reviewer onboarding
description: Process and checklists for enrolling reviewers in the docs portal public preview.
translator: machine-google-reviewed
---

## 概述

DOCS-SORA 跟踪开发人员门户的分阶段启动。校验和门控构建
(`npm run serve`) 并强化尝试流程，解锁下一个里程碑：
在公开预览版广泛开放之前，让经过审查的审稿人加入。本指南
描述如何收集请求、验证资格、提供访问权限以及
参与者安全下机。请参阅
[预览邀请流程](./preview-invite-flow.md) 用于群组规划、邀请
节奏和遥测导出；以下步骤重点关注要采取的行动
一旦选定审稿人。

- **范围：**需要访问文档预览的审阅者（`docs-preview.sora`，
  GA 之前的 GitHub Pages 版本或 SoraFS 捆绑包）。
- **超出范围：** Torii 或 SoraFS 操作员（由他们自己的入职培训涵盖）
  套件）和生产门户部署（请参阅
  [`devportal/deploy-guide`](./deploy-guide.md))。

## 角色和先决条件

|角色 |典型目标 |所需文物|笔记|
| ---| ---| ---| ---|
|核心维护者 |验证新指南，运行冒烟测试。 | GitHub 句柄、Matrix 联系人、已签署的 CLA 存档。 |通常已经在 `docs-preview` GitHub 团队中；仍然提出请求，以便访问是可审核的。 |
|合作伙伴审稿人 |在公开发布之前验证 SDK 片段或治理内容。 |公司电子邮件、合法 POC、签署的预览条款。 |必须承认遥测+数据处理要求。 |
|社区志愿者|提供有关指南的可用性反馈。 | GitHub 句柄、首选联系人、时区、CoC 接受情况。 |保持小规模；优先考虑已签署贡献者协议的审稿人。 |

所有审阅者类型必须：

1. 确认预览制品的可接受使用政策。
2.阅读安全性/可观察性附录
   ([`security-hardening`](./security-hardening.md),
   [`observability`](./observability.md),
   [`incident-runbooks`](./incident-runbooks.md))。
3. 同意在提供任何服务之前运行 `docs/portal/scripts/preview_verify.sh`
   本地快照。

## 摄入工作流程

1.请请求者填写
   [`docs/examples/docs_preview_request_template.md`](../../../examples/docs_preview_request_template.md)
   表单（或将其复制/粘贴到问题中）。至少捕获：身份、联系方式
   方法、GitHub 句柄、预期审核日期以及确认
   已阅读安全文档。
2. 在 `docs-preview` 跟踪器中记录请求（GitHub 问题或治理
   票）并指定审批者。
3. 验证先决条件：
   - CLA/贡献者协议存档（或合作伙伴合同参考）。
   - 存储在请求中的可接受使用确认。
   - 风险评估完成（例如，经法务部批准的合作伙伴审核员）。
4. 审批者在请求中签字并将跟踪问题链接到任何
   变更管理条目（示例：`DOCS-SORA-Preview-####`）。

## 配置和工具

1. **共享工件** — 提供最新的预览描述符+存档
   CI 工作流程或 SoraFS 引脚（`docs-portal-preview` 工件）。提醒
   审稿人运行：

   ```bash
   ./docs/portal/scripts/preview_verify.sh \
     --build-dir build \
     --descriptor artifacts/preview-descriptor.json \
     --archive artifacts/preview-site.tar.gz
   ```

2. **提供校验和执行服务** - 将审核者指向校验和门控
   命令：

   ```bash
   DOCS_RELEASE_TAG=preview-<stamp> npm run --prefix docs/portal serve
   ```

   这会重用 `scripts/serve-verified-preview.mjs`，因此不会出现未经验证的构建
   意外启动。

3. **授予 GitHub 访问权限（可选）** — 如果审阅者需要未发布的分支，
   在审核期间将它们添加到 `docs-preview` GitHub 团队，并
   在请求中记录成员资格变更。

4. **沟通支持渠道** — 共享待命联系人 (Matrix/Slack)
   以及来自 [`incident-runbooks`](./incident-runbooks.md) 的事件程序。

5. **遥测+反馈** - 提醒审阅者匿名分析是
  收集（请参阅 [`observability`](./observability.md)）。提供反馈
  邀请中引用的表单或问题模板，并使用以下内容记录事件
  [`preview-feedback-log`](./preview-feedback-log) 帮手所以波总结
  保持最新状态。

## 审稿人清单

在访问预览之前，审阅者必须完成以下操作：

1. 验证下载的工件 (`preview_verify.sh`)。
2. 通过 `npm run serve`（或 `serve:verified`）启动门户，以确保
   校验和防护处于活动状态。
3. 阅读上面链接的安全性和可观察性说明。
4. 使用设备代码登录（如果适用）测试 OAuth/Try it 控制台并
   避免重复使用生产代币。
5. 在商定的跟踪器（问题、共享文档或表单）和标签中记录结果
   它们带有预览版本标签。

## 维护者职责和离职

|相|行动|
| ---| ---|
|开球|确认请求中附加了接收清单，共享工件 + 说明，通过 [`preview-feedback-log`](./preview-feedback-log) 附加 `invite-sent` 条目，并在审核持续时间超过一周的情况下安排中点同步。 |
|监控|跟踪预览遥测（查找异常的 Try it 流量、探测失败），并在发生任何可疑情况时遵循事件运行手册。当发现结果到达时记录 `feedback-submitted`/`issue-opened` 事件，以便波指标保持准确。 |
|离职|撤销临时 GitHub 或 SoraFS 访问权限，记录 `access-revoked`，存档请求（包括反馈摘要 + 未完成的操作），并更新审阅者注册表。要求审阅者清除本地构建并附加从 [`docs/examples/docs_preview_feedback_digest.md`](../../../examples/docs_preview_feedback_digest.md) 生成的摘要。 |

在轮换审阅者时使用相同的流程。保持
存储库中的书面记录（问题 + 模板）有助于 DOCS-SORA 保持可审核性
让治理确认预览访问遵循记录的控制。

## 邀请模板和跟踪

- 开始每次外展活动
  [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md)
  文件。它捕获最低限度的法律语言、预览校验和指令、
  以及审查者认可可接受使用政策的期望。
- 编辑模板时，替换`<preview_tag>`的占位符，
  `<request_ticket>`，以及联系渠道。将最终消息的副本存储在
  受理单，以便审阅者、批准者和审计员可以参考
  发送的确切措辞。
- 发送邀请后，更新跟踪电子表格或问题
  `invite_sent_at` 时间戳和预期结束日期，因此
  [预览邀请流程](./preview-invite-flow.md) 举报可领取队列
  自动。