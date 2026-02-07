---
lang: zh-hans
direction: ltr
source: docs/examples/soranet_gar_intake_form.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6cd4da7e590d581719ed2607994d7d9eb16d153fbd06f85655d0da37c727853a
source_last_modified: "2025-12-29T18:16:35.085419+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# SoraNet GAR 摄入模板

请求 GAR 操作（清除、ttl 覆盖、速率
上限、审核指令、地理围栏或合法保留）。提交的表格
应与 `gar_controller` 输出一起固定，以便审核日志和
收据引用相同的证据 URI。

|领域|价值|笔记|
|--------|--------|--------|
|请求 ID |  |监护人/行动票证 ID。 |
|请求者 |  |帐号+联系方式。 |
|日期/时间 (UTC) |  |行动应该开始的时间。 |
| GAR 名称 |  |例如，`docs.sora`。 |
|规范主机|  |例如，`docs.gw.sora.net`。 |
|行动|  | `ttl_override` / `rate_limit_override` / `purge_static_zone` / `geo_fence` / `legal_hold` / `moderation`。 |
| TTL 覆盖（秒）|  |仅 `ttl_override` 需要。 |
|费率上限 (RPS) |  |仅 `rate_limit_override` 需要。 |
|允许的地区 |  |请求 `geo_fence` 时的 ISO 区域列表。 |
|被拒绝的地区 |  |请求 `geo_fence` 时的 ISO 区域列表。 |
|节制蛞蝓 |  |匹配 GAR 审核指令。 |
|清除标签 |  |服务前必须清除的标签。 |
|标签|  |机器标签（事件 ID、演习名称、POP 范围）。 |
|证据 URI |  |支持请求的日志/仪表板/规格。 |
|审核 URI |  |如果与默认值不同，则按弹出审核 URI。 |
|请求到期 |  | Unix时间戳或RFC3339；默认留空。 |
|原因 |  |面向用户的解释；出现在收据和仪表板中。 |
|审批人 |  |监护人/委员会批准请求。 |

### 提交步骤

1. 填写表格并将其附加到治理票证上。
2. 使用匹配的配置更新 GAR 控制器配置 (`policies`/`pops`)
   `labels`/`evidence_uris`/`expires_at_unix`。
3. 运行 `cargo xtask soranet-gar-controller ...` 以发出事件/收据。
4. 删除 `gar_controller_summary.json`、`gar_reconciliation_report.json`、
   `gar_metrics.prom` 和 `gar_audit_log.jsonl` 放入同一张票中。的
   审批者在发送前确认收据计数与 PoP 列表匹配。