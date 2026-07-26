---
lang: zh-hans
direction: ltr
source: docs/examples/docs_preview_feedback_form.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: afb7e51ddc0b7e819f2cbf3888aadf907b0e0010c676cb44af648f9f4818f8f5
source_last_modified: "2025-12-29T18:16:35.071058+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# 文档预览反馈表（W1 合作伙伴 Wave）

从 W1 审阅者收集反馈时使用此模板。复制它每
合作伙伴，填写元数据，并将完成的副本存储在
`artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/`。

## 审阅者元数据

- **合作伙伴 ID：** `partner-w1-XX`
- **请求票：** `DOCS-SORA-Preview-REQ-PXX`
- **邀请已发送（UTC）：** `YYYY-MM-DD hh:mm`
- **确认的校验和（UTC）：** `YYYY-MM-DD hh:mm`
- **主要关注领域：**（例如_SoraFS Orchestrator 文档_、_Torii ISO 流程_）

## 遥测和人工制品确认

|清单项目 |结果 |证据|
| ---| ---| ---|
|校验和验证 | ✅ / ⚠️ |日志路径（例如，`build/checksums.sha256`）|
|尝试一下代理冒烟测试 | ✅ / ⚠️ | `npm run manage:tryit-proxy …` 转录片段 |
| Grafana 仪表板回顾 | ✅ / ⚠️ |屏幕截图路径 |
|门户探查报告审查| ✅ / ⚠️ | `artifacts/docs_preview/.../preflight-summary.json` |

为审阅者检查的任何其他 SLO 添加行。

## 反馈日志

|面积 |严重性（信息/次要/主要/阻止）|描述 |建议的修复或问题 |追踪器问题 |
| ---| ---| ---| ---| ---|
| | | | | |

参考最后一栏中的 GitHub 问题或内部票证，以便预览
跟踪器可以将补救项目与此表单联系起来。

## 调查总结

1. **您对校验和指导和邀请流程有多大信心？** (1-5)
2. **哪些文档最有/最没有帮助？**（简短回答）
3. **是否有任何拦截器访问 Try it 代理或遥测仪表板？**
4. **是否需要额外的本地化或辅助内容？**
5. **正式发布前还有其他意见吗？**

如果您使用外部表单，请捕获简短的答案并附加原始调查导出。

## 知识检查

- 分数：`__/10`
- 错误问题（如有）：`[#1, #4, …]`
- 后续行动（如果分数 < 9/10）：是否安排了补救电话？是/否

## 签核

- 审稿人姓名和时间戳：
- 文档/DevRel 审阅者和时间戳：

将签名副本与相关工件一起存储，以便审核员可以重播
没有额外上下文的挥手。