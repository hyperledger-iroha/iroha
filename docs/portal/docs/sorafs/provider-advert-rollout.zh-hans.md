---
lang: zh-hans
direction: ltr
source: docs/portal/docs/sorafs/provider-advert-rollout.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b80573de9799c783b62fe4babb553de4dd0778b028cd6d6ad58eb3094f7284eb
source_last_modified: "2026-01-04T08:19:26.497389+00:00"
translation_last_reviewed: 2026-02-07
title: "SoraFS Provider Advert Rollout Plan"
translator: machine-google-reviewed
---

> 改编自 [`docs/source/sorafs/provider_advert_rollout.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_rollout.md)。

# SoraFS 提供商广告推出计划

该计划协调从允许的提供商广告到
多源块所需的完全控制的 `ProviderAdvertV1` 表面
检索。它侧重于三个可交付成果：

- **操作员指南。** 存储提供商必须完成的分步操作
  在每个门翻转之前。
- **遥测覆盖范围。** Observability 和 Ops 使用的仪表板和警报
  确认网络只接受合规的广告。
此次推出与 [SoraFS 迁移中的 SF-2b/2c 里程碑保持一致
路线图](./migration-roadmap) 并假设
[提供商准入政策](./provider-admission-policy) 已在
效果。

## 目前的要求

SoraFS 仅接受治理封装的 `ProviderAdvertV1` 有效负载。的
入学时必须执行以下要求：

- `profile_id=sorafs.sf1@1.0.0` 与规范 `profile_aliases` 存在。
- 多源必须包含 `chunk_range_fetch` 功能有效负载
  检索。
- `signature_strict=true`，广告上附有理事会签名
  信封。
- `allow_unknown_capabilities` 仅在显式 GREASE 钻探期间允许
  并且必须被记录。

## 操作员清单

1. **库存广告。** 列出每个已发布的广告和记录：
   - 控制包络路径（`defaults/nexus/sorafs_admission/...` 或同等产品）。
   - 广告 `profile_id` 和 `profile_aliases`。
   - 能力列表（预计至少为 `torii_gateway` 和 `chunk_range_fetch`）。
   - `allow_unknown_capabilities` 标志（当存在供应商保留的 TLV 时需要）。
2. **使用提供者工具重新生成。**
   - 与您的提供商广告发布商重建有效负载，确保：
     - `profile_id=sorafs.sf1@1.0.0`
     - `capability=chunk_range_fetch` 与已定义的 `max_span`
     - 当存在 GREASE TLV 时，`allow_unknown_capabilities=<true|false>`
   - 通过 `/v2/sorafs/providers` 和 `sorafs_fetch` 验证；关于未知的警告
     必须对能力进行分类。
3. **验证多源准备情况。**
   -用`--provider-advert=<path>`执行`sorafs_fetch`； CLI 现在失败
     当 `chunk_range_fetch` 丢失并打印忽略未知的警告时
     能力。捕获 JSON 报告并将其与操作日志一起存档。
4. **阶段更新。**
   - 至少提前 30 天提交 `ProviderAdmissionRenewalV1` 信封
     到期。续订必须保留规范的句柄和功能集；
     只有权益、端点或元数据应该改变。
5. **与依赖团队沟通。**
   - SDK 所有者必须发布向操作员发出警告的版本
     广告被拒绝。
   - DevRel 公布每个阶段的转变；包括仪表板链接和
     下面的阈值逻辑。
6. **安装仪表板和警报。**
   - 导入 Grafana 导出并将其放在 **SoraFS / Provider 下
     推出**，仪表板 UID `sorafs-provider-admission`。
   - 确保警报规则指向共享的 `sorafs-advert-rollout`
     暂存和生产中的通知渠道。

## 遥测和仪表板

以下指标已通过 `iroha_telemetry` 公开：

- `torii_sorafs_admission_total{result,reason}` — 接受、拒绝的计数，
  和警告结果。原因包括 `missing_envelope`、`unknown_capability`、
  `stale` 和 `policy_violation`。

Grafana 导出：[`docs/source/grafana_sorafs_admission.json`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/grafana_sorafs_admission.json)。
将文件导入共享仪表板存储库 (`observability/dashboards`)
并在发布前仅更新数据源 UID。

该板在 Grafana 文件夹 **SoraFS / Provider Rollout** 下发布
稳定的 UID `sorafs-provider-admission`。警报规则
`sorafs-admission-warn`（警告）和 `sorafs-admission-reject`（严重）是
预先配置为使用 `sorafs-advert-rollout` 通知策略；调整
如果目的地列表发生更改，则该联系点而不是编辑
仪表板 JSON。

推荐的 Grafana 面板：

|面板|查询 |笔记|
|--------|--------|--------|
| **录取结果率** | `sum by(result)(rate(torii_sorafs_admission_total[5m]))` |堆栈图可视化接受、警告、拒绝。当警告 > 0.05 * 总计（警告）或拒绝 > 0（严重）时发出警报。 |
| **警戒率** | `sum(rate(torii_sorafs_admission_total{result="warn"}[5m])) / sum(rate(torii_sorafs_admission_total[5m]))` |满足寻呼机阈值的单行时间序列（5% 警告率滚动 15 分钟）。 |
| **拒绝原因** | `sum by(reason)(rate(torii_sorafs_admission_total{result="reject"}[5m]))` |推动运行手册分类；附加缓解步骤的链接。 |
| **刷新债务** | `sum(rate(torii_sorafs_admission_total{reason="stale"}[1h]))` |表示提供商错过了刷新截止日期；与发现缓存日志的交叉引用。 |

手动仪表板的 CLI 工件：

- `sorafs_fetch --provider-metrics-out` 写入 `failures`、`successes`，以及
  每个提供商的 `disabled` 计数器。导入临时仪表板进行监控
  在切换生产提供商之前，orchestrator 会进行试运行。
- JSON 报告的 `chunk_retry_rate` 和 `provider_failure_rate` 字段
  突出显示通常在进入之前出现的节流或失效有效负载症状
  拒绝。

### Grafana 仪表板布局

Observability 发布了专门的委员会 — **SoraFS 提供商准入
推出** (`sorafs-provider-admission`) — 在 **SoraFS/提供商推出**下
具有以下规范面板 ID：

- 第 1 组 — *入院结果率*（堆叠面积，单位“操作/分钟”）。
- 面板 2 — *警告率*（单个系列），发出表达式
  `sum(rate(torii_sorafs_admission_total{result="warn"}[5m])) /
   sum(rate(torii_sorafs_admission_total[5m]))`.
- 第 3 版 — *拒绝原因*（按 `reason` 分组的时间序列），排序依据
  `rate(...[5m])`。
- 第 4 部分 — *刷新债务*（统计数据），反映上表中的查询和
  注释有从迁移分类账中提取的广告刷新截止日期。

在基础设施仪表板存储库中复制（或创建）JSON 框架，网址为
`observability/dashboards/sorafs_provider_admission.json`，然后仅更新
数据源UID；面板 ID 和警报规则由 Runbook 引用
下面，因此请避免在不修改本文档的情况下对它们重新编号。

为了方便起见，存储库现在在以下位置提供了参考仪表板定义：
`docs/source/grafana_sorafs_admission.json`；将其复制到您的 Grafana 文件夹中，如果
您需要一个本地测试的起点。

### Prometheus 警报规则

将以下规则组添加到 `observability/prometheus/sorafs_admission.rules.yml`
（如果这是第一个 SoraFS 规则组，则创建该文件）并将其包含在
您的 Prometheus 配置。将 `<pagerduty>` 替换为实际路由
随叫随到轮换的标签。

```yaml
groups:
  - name: torii_sorafs_admission
    rules:
      - alert: SorafsProviderAdvertWarnFlood
        expr: sum(rate(torii_sorafs_admission_total{result="warn"}[5m])) /
              sum(rate(torii_sorafs_admission_total[5m])) > 0.05
        for: 15m
        labels:
          severity: warning
          route: <pagerduty>
        annotations:
          summary: "SoraFS provider adverts generating warnings"
          description: |
            Warn outcomes exceeded 5% of all admissions for 15 minutes.
            Inspect panel 3 on the sorafs/provider-admission dashboard and
            coordinate advert rotation with the affected operator.
      - alert: SorafsProviderAdvertReject
        expr: increase(torii_sorafs_admission_total{result="reject"}[5m]) > 0
        for: 5m
        labels:
          severity: critical
          route: <pagerduty>
        annotations:
          summary: "SoraFS provider adverts rejected"
          description: |
            Provider adverts have been rejected for the last five minutes.
            Check panel 4 (rejection reasons) and rotate envelopes before
            the refresh deadline elapses.
```

运行 `scripts/check_prometheus_rules.sh observability/prometheus/sorafs_admission.rules.yml`
在推送更改之前确保语法通过 `promtool check rules`。

## 录取结果

- 缺少 `chunk_range_fetch` 功能 → 使用 `reason="missing_capability"` 拒绝。
- 不带 `allow_unknown_capabilities=true` 的未知功能 TLV → 拒绝
  `reason="unknown_capability"`。
- `signature_strict=false` → 拒绝（保留用于隔离诊断）。
- 过期 `refresh_deadline` → 拒绝。

## 沟通和事件处理

- **每周状态邮件。** DevRel 分发入学简要摘要
  指标、未解决的警告和即将到来的截止日期。
- **事件响应。** 如果 `reject` 发出火灾警报，待命工程师：
  1. 通过 Torii 发现 (`/v2/sorafs/providers`) 获取违规广告。
  2. 在提供商管道中重新运行广告验证并与
     `/v2/sorafs/providers` 重现该错误。
  3. 与提供商协调，在下次刷新之前轮播广告
     截止日期。
- **更改冻结。** 在 R1/R2 期间，功能模式不会发生更改，除非
  推出委员会签字同意； GREASE 试验必须安排在
  每周维护窗口并记录在迁移分类账中。

## 参考文献

- [SoraFS 节点/客户端协议](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/sorafs_node_client_protocol.md)
- [提供商准入政策](./provider-admission-policy)
- [迁移路线图](./migration-roadmap)
- [提供商广告多源扩展](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md)