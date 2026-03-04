---
id: preview-feedback-w2-summary
lang: zh-hans
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w2/summary.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: W2 community feedback & status
sidebar_label: W2 summary
description: Live digest for the community preview wave (W2).
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

|项目 |详情 |
| ---| ---|
|波| W2 — 社区审阅者 |
|邀请窗口 | 2025-06-15 → 2025-06-29 |
|文物标签| `preview-2025-06-15` |
|追踪器问题 | `DOCS-SORA-Preview-W2` |
|参与者 |通讯卷 01 … 通讯卷 08 |

## 亮点

1. **治理和工具** — 社区接纳政策于 2025 年 5 月 20 日一致批准；带有动机/时区字段的更新请求模板位于 `docs/examples/docs_preview_request_template.md` 下。
2. **预检证据** — 尝试代理更改 `OPS-TRYIT-188` 在 2025-06-09 运行，捕获 Grafana 仪表板，并在 `artifacts/docs_preview/W2/` 下存档 `preview-2025-06-15` 描述符/校验和/探针输出。
3. **邀请波** — 2025 年 6 月 15 日邀请了八位社区审阅者，并在跟踪者邀请表中记录了致谢信息；浏览前全部完成校验和验证。
4. **反馈** — `docs-preview/w2 #1`（工具提示措辞）和 `#2`（本地化侧边栏顺序）于 2025-06-18 提交，并于 2025-06-21 解决（Docs-core-04/05）；浪潮期间没有发生任何事件。

## 行动项目

|身份证 |描述 |业主|状态 |
| ---| ---| ---| ---|
| W2-A1 |地址 `docs-preview/w2 #1`（工具提示措辞）。 |文档核心-04 | ✅ 2025-06-21 完成 |
| W2-A2 |地址 `docs-preview/w2 #2`（本地化侧边栏）。 |文档核心-05 | ✅ 2025-06-21 完成 |
| W2-A3 |存档退出证据+更新路线图/状态。 |文档/DevRel 领导 | ✅ 2025-06-29 完成 |

## 退出总结 (2025-06-29)

- 所有八位社区审阅者均确认完成并取消了预览访问权限；跟踪器邀请日志中记录的确认。
- 最终遥测快照（`docs.preview.integrity`、`TryItProxyErrors`、`DocsPortal/GatewayRefusals`）保持绿色；日志以及附加到 `DOCS-SORA-Preview-W2` 的 Try it 代理记录。
- 证据包（描述符、校验和日志、探测输出、链接报告、Grafana 屏幕截图、邀请确认）存档在 `artifacts/docs_preview/W2/preview-2025-06-15/` 下。
- 跟踪器 W2 检查点日志通过退出进行更新，确保路线图在 W3 规划开始之前保留可审核的记录。