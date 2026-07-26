---
lang: zh-hans
direction: ltr
source: docs/portal/docs/devportal/preview-invite-tracker.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: dba1278df19fd3003f943bfc9f4168cd820a4cfcebc8a64daee248fa95e6f18f
source_last_modified: "2025-12-29T18:16:35.112669+00:00"
translation_last_reviewed: 2026-02-07
id: preview-invite-tracker
title: Preview invite tracker
sidebar_label: Preview tracker
description: Wave-by-wave status log for the checksum-gated docs portal preview program.
translator: machine-google-reviewed
---

该跟踪器记录每个文档门户预览波，以便 DOCS-SORA 所有者和
治理审核者可以查看哪个群组处于活动状态、谁批准了邀请、
以及哪些文物仍需要关注。每当发送邀请时更新它，
撤销或推迟，以便审计跟踪保留在存储库内。

## 波浪状态

|波|队列|追踪器问题 |批准人 |状态 |目标窗口|笔记|
| ---| ---| ---| ---| ---| ---| ---|
| **W0 – 核心维护者** |文档 + SDK 维护人员验证校验和流程 | `DOCS-SORA-Preview-W0` (GitHub/ops 跟踪器) |文档/DevRel 负责人 + Portal TL | 🈴已完成 | 2025 年第 2 季度第 1-2 周 |邀请于 2025 年 3 月 25 日发送，遥测保持绿色，退出摘要于 2025 年 4 月 8 日发布。 |
| **W1 – 合作伙伴** | NDA 下的 SoraFS 运营商、Torii 集成商 | `DOCS-SORA-Preview-W1` |文档/DevRel 领导 + 治理联络员 | 🈴已完成 | 2025 年第二季度第 3 周 |邀请于 2025-04-12 → 2025-04-26 进行，所有八个合作伙伴均已确认； [`preview-feedback/w1/log.md`](./preview-feedback/w1/log.md) 中捕获的证据和 [`preview-feedback/w1/summary.md`](./preview-feedback/w1/summary.md) 中的退出摘要。 |
| **W2 – 社区** |精心策划的社区候补名单（一次≤ 25 名）| `DOCS-SORA-Preview-W2` |文档/DevRel 主管 + 社区经理 | 🈴已完成 | 2025 年第 3 季度第 1 周（暂定）|邀请于 2025-06-15 → 2025-06-29 运行，遥测始终呈绿色； [`preview-feedback/w2/summary.md`](./preview-feedback/w2/summary.md) 中捕获的证据+发现。 |
| **W3 – 测试版群组** |金融/可观测性测试版 + SDK 合作伙伴 + 生态系统倡导者 | `DOCS-SORA-Preview-W3` |文档/DevRel 领导 + 治理联络员 | 🈴已完成 | 2026 年第一季度第 8 周 |邀请日期为 2026-02-18 → 2026-02-28；摘要 + 通过 `preview-20260218` 波生成的门户数据（请参阅 [`preview-feedback/w3/summary.md`](./preview-feedback/w3/summary.md)）。 |

> 注意：将每个跟踪器问题链接到相应的预览请求票证并
> 将它们归档到 `docs-portal-preview` 项目下，以便保留批准
> 可发现的。

## 活动任务 (W0)

- ✅ 刷新预检工件（GitHub Actions `docs-portal-preview` 运行 2025-03-24，描述符通过 `scripts/preview_verify.sh` 使用标签 `preview-2025-03-24` 进行验证）。
- ✅ 捕获的遥测基线（`docs.preview.integrity`、`TryItProxyErrors` 仪表板快照保存到 W0 跟踪器问题）。
- ✅ 使用带有预览标签 `preview-2025-03-24` 的 [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md) 锁定外展副本。
- ✅ 记录前五个维护者的接收请求（票证 `DOCS-SORA-Preview-REQ-01` … `-05`）。
- ✅ 在连续 7 个绿色遥测日后，于 2025 年 3 月 25 日 10:00–10:20 发送前 5 个邀请；确认信息存储在 `DOCS-SORA-Preview-W0` 中。
- ✅ 监控遥测 + 主机办公时间（截至 2025 年 3 月 31 日每日签到；检查点日志如下）。
- ✅ 收集中点反馈/问题并将其标记为 `docs-preview/w0`（请参阅 [W0 摘要](./preview-feedback/w0/summary.md)）。
- ✅ 发布浪潮摘要 + 邀请退出确认（退出捆绑日期为 2025-04-08；请参阅 [W0 摘要](./preview-feedback/w0/summary.md)）。
- ✅ W3 beta 波追踪；治理审查后根据需要安排未来的浪潮。

## W1合作伙伴浪潮总结

- ✅ **法律和治理批准。** 合作伙伴附录于 2025 年 4 月 5 日签署；批准已上传至 `DOCS-SORA-Preview-W1`。
- ✅ **遥测 + 尝试分段。** 更改票证 `OPS-TRYIT-147` 于 2025-04-06 执行，其中 `docs.preview.integrity`、`TryItProxyErrors` 和 `DocsPortal/GatewayRefusals` 的 Grafana 快照已存档。
- ✅ **Artefact + 校验和准备。** `preview-2025-04-12` 捆绑包已验证；描述符/校验和/探测日志存储在 `artifacts/docs_preview/W1/preview-2025-04-12/` 下。
- ✅ **邀请名册 + 派遣。** 所有八个合作伙伴请求 (`DOCS-SORA-Preview-REQ-P01…P08`) 均获得批准；邀请于 2025 年 4 月 12 日 15:00–15:21UTC 发送，并按审阅者记录确认。
- ✅ **反馈仪器。** 每日办公时间+记录的遥测检查点；请参阅 [`preview-feedback/w1/summary.md`](./preview-feedback/w1/summary.md) 了解摘要。
- ✅ **最终名册/退出日志。** [`preview-feedback/w1/log.md`](./preview-feedback/w1/log.md) 现在记录截至 2025 年 4 月 26 日的邀请/确认时间戳、遥测证据、测验导出和工件指针，以便治理可以重播这波浪潮。

## 邀请日志——W0核心维护者

|审稿人 ID |角色 |索取门票 |邀请已发送 (UTC) |预计退出（UTC）|状态 |笔记|
| ---| ---| ---| ---| ---| ---| ---|
|文档核心-01 |门户维护者 | `DOCS-SORA-Preview-REQ-01` | 2025-03-25 10:05 | 2025-04-08 10:00 |活跃|确认校验和验证；专注于导航/侧边栏审查。 |
| sdk-rust-01 | Rust SDK 负责人 | `DOCS-SORA-Preview-REQ-02` | 2025-03-25 10:08 | 2025-04-08 10:00 |活跃|测试 SDK 配方 + Norito 快速入门。 |
| SDK-js-01 | JS SDK 维护者 | `DOCS-SORA-Preview-REQ-03` | 2025-03-25 10:12 | 2025-04-08 10:00 |活跃|验证 Try it 控制台 + ISO 流程。 |
| sorafs-ops-01 | sorafs-ops-01 SoraFS 操作员联络 | `DOCS-SORA-Preview-REQ-04` | 2025-03-25 10:15 | 2025-04-08 10:00 |活跃|审核 SoraFS 运行手册 + 编排文档。 |
|可观察性-01 |可观察性 TL | `DOCS-SORA-Preview-REQ-05` | 2025-03-25 10:18 | 2025-04-08 10:00 |活跃|审查遥测/事件附录；拥有 Alertmanager 覆盖范围。 |

所有邀请都引用相同的 `docs-portal-preview` 工件（运行 2025-03-24，
标签 `preview-2025-03-24`）和捕获的验证记录
`DOCS-SORA-Preview-W0`。任何添加/暂停都必须记录在两个表中
在继续下一波之前，先处理上面的问题和跟踪器问题。

## 检查点日志 — W0

|日期 (UTC) |活动 |笔记|
| ---| ---| ---|
| 2025-03-26 |遥测基线审查+办公时间| `docs.preview.integrity` + `TryItProxyErrors` 保持绿色；办公时间确认所有审阅者已完成校验和验证。 |
| 2025-03-27 |中点反馈摘要已发布 | [`preview-feedback/w0/summary.md`](./preview-feedback/w0/summary.md) 中捕获的摘要；两个小导航问题记录为 `docs-preview/w0` 标签，没有事件报告。 |
| 2025-03-31 |最后一周遥测抽查|最后出境前办公时间；审阅者确认剩余的文档任务正常进行，没有发出警报。 |
| 2025-04-08 |退出摘要+邀请关闭|确认已完成的审查，撤销临时访问权限，将调查结果存档在 [`preview-feedback/w0/summary.md`](./preview-feedback/w0/summary.md#exit-summary-2025-04-08) 中；在准备 W1 之前更新了跟踪器。 |

## 邀请日志——W1合作伙伴

|审稿人 ID |角色 |索取门票 |邀请已发送 (UTC) |预计退出（UTC）|状态 |笔记|
| ---| ---| ---| ---| ---| ---| ---|
|索拉夫-op-01 | SoraFS 运营商（欧盟）| `DOCS-SORA-Preview-REQ-P01` | 2025-04-12 15:00 | 2025-04-26 15:00 |已完成 | 2025-04-20 交付协调器操作反馈；退出确认 15:05UTC。 |
|索拉夫-op-02 | SoraFS 操作员 (JP) | `DOCS-SORA-Preview-REQ-P02` | 2025-04-12 15:03 | 2025-04-26 15:00 |已完成 |在 `docs-preview/w1` 中记录了推出指导意见；退出确认 15:10UTC。 |
|索拉夫-op-03 | SoraFS 运营商（美国）| `DOCS-SORA-Preview-REQ-P03` | 2025-04-12 15:06 | 2025-04-26 15:00 |已完成 |提交争议/黑名单编辑；退出确认 15:12UTC。 |
|鸟居-int-01 | Torii 积分器 | `DOCS-SORA-Preview-REQ-P04` | 2025-04-12 15:09 | 2025-04-26 15:00 |已完成 |尝试一下授权演练已接受；退出确认 15:14UTC。 |
|鸟居-int-02 | Torii 积分器 | `DOCS-SORA-Preview-REQ-P05` | 2025-04-12 15:12 | 2025-04-26 15:00 |已完成 |记录 RPC/OAuth 文档评论；退出确认 15:16UTC。 |
| sdk-partner-01 | sdk-partner-01 | SDK 合作伙伴 (Swift) | `DOCS-SORA-Preview-REQ-P06` | 2025-04-12 15:15 | 2025-04-26 15:00 |已完成 |预览诚信反馈合并；退出确认 15:18UTC。 |
| sdk-partner-02 | sdk-partner-02 | SDK 合作伙伴 (Android) | `DOCS-SORA-Preview-REQ-P07` | 2025-04-12 15:18 | 2025-04-26 15:00 |已完成 |遥测/编辑审核已完成；退出确认 15:22UTC。 |
|网关操作-01 |网关运营商| `DOCS-SORA-Preview-REQ-P08` | 2025-04-12 15:21 | 2025-04-26 15:00 |已完成 |已提交网关 DNS 运行手册评论；退出确认 15:24UTC。 |

## 检查点日志 — W1

|日期 (UTC) |活动 |笔记|
| ---| ---| ---|
| 2025-04-12 |邀请派遣+实物验证 |所有八个合作伙伴都通过电子邮件发送了 `preview-2025-04-12` 描述符/存档；确认存储在跟踪器中。 |
| 2025-04-13 |遥测基线审查 |已审核 `docs.preview.integrity`、`TryItProxyErrors` 和 `DocsPortal/GatewayRefusals` 仪表板 — 全面呈绿色；办公时间确认校验和验证完成。 |
| 2025-04-18 |中波办公时间 | `docs.preview.integrity` 保持绿色；两个文档记录在 `docs-preview/w1` 下（导航措辞 + 尝试屏幕截图）。 |
| 2025-04-22 |最终遥测抽查|代理+仪表板仍然健康；没有提出新的问题，退出前在跟踪器中注意到。 |
| 2025-04-26 |退出摘要+邀请关闭|所有合作伙伴均确认审核已完成，邀请已撤销，证据已存档在 [`preview-feedback/w1/summary.md`](./preview-feedback/w1/summary.md#exit-summary-2025-04-26) 中。 |

## W3 beta 队列回顾

- ✅ 于 2026 年 2 月 18 日发送邀请，并在当天记录了校验和验证 + 确认。
- ✅ 在 `docs-preview/20260218` 下收集的反馈，涉及治理问题 `DOCS-SORA-Preview-20260218`；通过 `npm run --prefix docs/portal preview:wave -- --wave preview-20260218` 生成的摘要 + 摘要。
- ✅ 最终遥测检查后，访问权限于 2026-02-28 被撤销；跟踪器 + 门户表已更新，显示 W3 已完成。

## 邀请日志——W2社区|审稿人 ID |角色 |索取门票 |邀请已发送 (UTC) |预计退出（UTC）|状态 |笔记|
| ---| ---| ---| ---| ---| ---| ---|
|通讯卷01 |社区审阅者 (SDK) | `DOCS-SORA-Preview-REQ-C01` | 2025-06-15 16:00 | 2025-06-29 16:00 |已完成 |确认时间 16:06UTC；专注于SDK快速入门； 2025 年 6 月 29 日确认退出。 |
|通讯卷 02 |社区审核员（治理）| `REQ-C02` | 2025-06-15 16:03 | 2025-06-29 16:00 |已完成 |治理/SNS 审核已完成； 2025 年 6 月 29 日确认退出。 |
|通讯卷 03 |社区审阅者 (Norito) | `REQ-C03` | 2025-06-15 16:06 | 2025-06-29 16:00 |已完成 | Norito 已记录演练反馈；退出确认 2025-06-29。 |
|通讯卷 04 |社区审阅者 (SoraFS) | `REQ-C04` | 2025-06-15 16:09 | 2025-06-29 16:00 |已完成 | SoraFS 操作手册审核已完成；退出确认 2025-06-29。 |
|通讯卷 05 |社区审阅者（辅助功能）| `REQ-C05` | 2025-06-15 16:12 | 2025-06-29 16:00 |已完成 |共享辅助功能/用户体验注释；退出确认 2025-06-29。 |
|通讯卷 06 |社区审阅者（本地化）| `REQ-C06` | 2025-06-15 16:15 | 2025-06-29 16:00 |已完成 |记录本地化反馈；退出确认 2025-06-29。 |
|通讯卷 07 |社区审阅者（手机）| `REQ-C07` | 2025-06-15 16:18 | 2025-06-29 16:00 |已完成 |已交付移动 SDK 文档检查；退出确认 2025-06-29。 |
|通讯卷 08 |社区审阅者（可观察性）| `REQ-C08` | 2025-06-15 16:21 | 2025-06-29 16:00 |已完成 |可观察性附录审查已完成；退出确认 2025-06-29。 |

## 检查点日志 — W2

|日期 (UTC) |活动 |笔记|
| ---| ---| ---|
| 2025-06-15 |邀请派遣+实物验证 | `preview-2025-06-15` 描述符/档案与 8 位社区审阅者共享；确认存储在跟踪器中。 |
| 2025-06-16 |遥测基线审查 | `docs.preview.integrity`、`TryItProxyErrors`、`DocsPortal/GatewayRefusals` 仪表板绿色；尝试代理日志显示社区令牌处于活动状态。 |
| 2025-06-18 |办公时间和问题分类 |收集了两条建议（`docs-preview/w2 #1` 工具提示措辞、`#2` 本地化侧边栏）——均发送至文档。 |
| 2025-06-21 |遥测检查+文档修复|文档地址为 `docs-preview/w2 #1/#2`；仪表板仍然是绿色的，没有发生任何事件。 |
| 2025-06-24 |最后一周办公时间 |审稿人确认了剩余的反馈提交；没有警报火灾。 |
| 2025-06-29 |退出摘要+邀请关闭|记录确认、撤销预览访问、存档遥测快照 + 工件（请参阅 [`preview-feedback/w2/summary.md`](./preview-feedback/w2/summary.md#exit-summary-2025-06-29)）。 |
| 2025-04-15 |办公时间和问题分类 |在 `docs-preview/w1` 下记录了两个文档建议；没有触发任何事件或警报。 |

## 报告挂钩

- 每个星期三，更新上面的跟踪表以及主动邀请问题
  带有简短的状态说明（已发送的邀请、活跃的审阅者、事件）。
- 当 Wave 结束时，附加反馈摘要路径（例如，
  `docs/portal/docs/devportal/preview-feedback/w0/summary.md`）并将其链接到
  `status.md`。
- 如果[预览邀请流程]中有任何暂停条件(./preview-invite-flow.md)
  触发器，在恢复邀请之前在此处添加修复步骤。