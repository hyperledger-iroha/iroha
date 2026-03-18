---
id: incident-runbooks
lang: zh-hans
direction: ltr
source: docs/portal/docs/devportal/incident-runbooks.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Incident Runbooks & Rollback Drills
sidebar_label: Incident Runbooks
description: Response guides for failed portal deployments, SoraFS replication degradation, analytics outages, and the quarterly rehearsal cadence required by DOCS-9.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

## 目的

路线图项目 **DOCS-9** 需要可操作的剧本以及排练计划，以便
门户网站运营商无需猜测即可从运输失败中恢复。此注
涵盖三个高信号事件——部署失败、复制
退化和分析中断，并记录季度演练
证明别名回滚和综合验证仍然可以端到端地工作。

### 相关材料

- [`devportal/deploy-guide`](./deploy-guide) — 打包、签名和别名
  促销工作流程。
- [`devportal/observability`](./observability) — 发布标签、分析和
  下面引用的探针。
- `docs/source/sorafs_node_client_protocol.md`
  和 [`sorafs/pin-registry-ops`](../sorafs/pin-registry-ops)
  — 注册表遥测和升级阈值。
- `docs/portal/scripts/sorafs-pin-release.sh` 和 `npm run probe:*` 帮助程序
  在整个清单中引用。

### 共享遥测和工具

|信号/工具|目的|
| ------------- | -------- |
| `torii_sorafs_replication_sla_total`（已满足/错过/待处理）|检测复制停滞和 SLA 违规。 |
| `torii_sorafs_replication_backlog_total`，`torii_sorafs_replication_completion_latency_epochs` |量化分类的积压深度和完成延迟。 |
| `torii_sorafs_gateway_refusals_total`、`torii_sorafs_manifest_submit_total{status="error"}` |显示错误部署后经常出现的网关端故障。 |
| `npm run probe:portal` / `npm run probe:tryit-proxy` |用于控制释放和验证回流的合成探针。 |
| `npm run check:links` |断链门；每次缓解后使用。 |
| `sorafs_cli manifest submit … --alias-*`（由 `scripts/sorafs-pin-release.sh` 包裹）|别名提升/恢复机制。 |
| `Docs Portal Publishing` Grafana 板 (`dashboards/grafana/docs_portal.json`) |聚合拒绝/别名/TLS/复制遥测。 PagerDuty 警报引用这些面板来获取证据。 |

## Runbook — 部署失败或不良工件

### 触发条件

- 预览/生产探针失败 (`npm run probe:portal -- --expect-release=…`)。
- `torii_sorafs_gateway_refusals_total` 上的 Grafana 警报或
  推出后的 `torii_sorafs_manifest_submit_total{status="error"}`。
- 手动 QA 立即注意到损坏的路由或 Try-It 代理故障
  别名推广。

### 立即收容

1. **冻结部署：** 使用 `DEPLOY_FREEZE=1` 标记 CI 管道 (GitHub
   工作流输入）或暂停 Jenkins 作业，这样就不会出现其他工件。
2. **捕获工件：** 下载失败构建的 `build/checksums.sha256`，
   `portal.manifest*.{json,to,bundle,sig}`，并探测输出，以便回滚可以
   参考准确的摘要。
3. **通知利益相关者：** 存储 SRE、Docs/DevRel 领导和治理
   值班人员提高意识（特别是当 `docs.sora` 受到影响时）。

### 回滚过程

1. 识别最后已知良好 (LKG) 清单。生产流程存储
   它们位于 `artifacts/devportal/<release>/sorafs/portal.manifest.to` 下。
2. 使用传送助手将别名重新绑定到该清单：

```bash
cd docs/portal
./scripts/sorafs-pin-release.sh \
  --build-dir build \
  --artifact-dir artifacts/revert-$(date +%Y%m%d%H%M) \
  --sorafs-dir artifacts/revert-$(date +%Y%m%d%H%M)/sorafs \
  --pin-min-replicas 5 \
  --alias "docs-prod-revert" \
  --alias-namespace "${PIN_ALIAS_NAMESPACE}" \
  --alias-name "${PIN_ALIAS_NAME}" \
  --alias-proof "${PIN_ALIAS_PROOF_PATH}" \
  --torii-url "${TORII_URL}" \
  --submitted-epoch "$(date +%Y%m%d)" \
  --authority "${AUTHORITY}" \
  --private-key "${PRIVATE_KEY}" \
  --skip-submit

# swap in the LKG artefacts before submission
cp /secure/archive/lkg/portal.manifest.to artifacts/.../sorafs/portal.manifest.to
cp /secure/archive/lkg/portal.manifest.bundle.json artifacts/.../sorafs/

cargo run -p sorafs_orchestrator --bin sorafs_cli -- \
  manifest submit \
  --manifest artifacts/.../sorafs/portal.manifest.to \
  --chunk-plan artifacts/.../sorafs/portal.plan.json \
  --torii-url "${TORII_URL}" \
  --authority "${AUTHORITY}" \
  --private-key "${PRIVATE_KEY}" \
  --alias-namespace "${PIN_ALIAS_NAMESPACE}" \
  --alias-name "${PIN_ALIAS_NAME}" \
  --alias-proof "${PIN_ALIAS_PROOF_PATH}" \
  --metadata rollback_from="${FAILED_RELEASE}" \
  --summary-out artifacts/.../sorafs/rollback.submit.json
```

3. 将回滚摘要与 LKG 一起记录在事件单中，并
   清单摘要失败。

### 验证

1.`npm run probe:portal -- --expect-release=${LKG_TAG}`。
2. `npm run check:links`。
3. `sorafs_cli manifest verify-signature …` 和 `sorafs_cli proof verify …`
   （请参阅部署指南）确认重新升级的清单仍然匹配
   存档的 CAR。
4. `npm run probe:tryit-proxy` 确保 Try-It 临时代理返回。

### 事件发生后

1. 仅在了解根本原因后才重新启用部署管道。
2. 回填 [`devportal/deploy-guide`](./deploy-guide)“经验教训”
   带有新陷阱的条目（如果有）。
3. 记录失败测试套件的缺陷（探针、链接检查器等）。

## Runbook — 复制降级

### 触发条件

- 警报：`sum(torii_sorafs_replication_sla_total{outcome="met"}) /
  Clip_min(sum(torii_sorafs_replication_sla_total{结果=~"遇见|错过"}), 1) <
  0.95` 10 分钟。
- `torii_sorafs_replication_backlog_total > 10` 10 分钟（参见
  `pin-registry-ops.md`）。
- 治理报告发布后别名可用性缓慢。

### 分类

1. 检查 [`sorafs/pin-registry-ops`](../sorafs/pin-registry-ops) 仪表板以确认
   待办事项是否局限于存储类或提供商队列。
2. 交叉检查 Torii 日志中是否有 `sorafs_registry::submit_manifest` 警告
   确定提交本身是否失败。
3. 通过 `sorafs_cli manifest status --manifest …` 获取副本运行状况样本（列出
   每个提供商的复制结果）。

### 缓解措施

1. 使用更高的副本计数 (`--pin-min-replicas 7`) 重新发出清单
   `scripts/sorafs-pin-release.sh` 因此调度程序将负载分散到更大的
   提供商集。在事件日志中记录新的清单摘要。
2. 如果待办事项与单个提供商相关联，请通过以下方式暂时禁用它：
   复制调度程序（记录在 `pin-registry-ops.md` 中）并提交新的
   清单强制其他提供者刷新别名。
3. 当别名新鲜度比复制奇偶校验更重要时，重新绑定
   为已上演的热清单 (`docs-preview`) 建立别名，然后发布
   一旦 SRE 清除了积压的工作，就会有后续清单。

### 恢复和关闭

1. 监控`torii_sorafs_replication_sla_total{outcome="missed"}`以确保
   计算平台期。
2. 捕获 `sorafs_cli manifest status` 输出作为每个副本都已存在的证据
   恢复合规。
3. 归档或更新复制积压事后分析以及后续步骤
   （提供者扩展、分块器调整等）。

## Runbook — 分析或遥测中断

### 触发条件

- `npm run probe:portal` 成功，但仪表板停止摄取
  `AnalyticsTracker` 事件持续 >15 分钟。
- 隐私审查标记了丢失事件的意外增加。
- `npm run probe:tryit-proxy` 在 `/probe/analytics` 路径上失败。

### 回应

1. 验证构建时输入：`DOCS_ANALYTICS_ENDPOINT` 和
   失败的发布工件 (`build/release.json`) 中的 `DOCS_ANALYTICS_SAMPLE_RATE`。
2. 重新运行 `npm run probe:portal`，其中 `DOCS_ANALYTICS_ENDPOINT` 指向
   暂存收集器以确认跟踪器仍然发出有效负载。
3. 如果收集器已关闭，请设置 `DOCS_ANALYTICS_ENDPOINT=""` 并重建，以便
   跟踪器短路；在事件时间线中记录中断窗口。
4.验证`scripts/check-links.mjs`仍然是指纹`checksums.sha256`
   （分析中断不得*不*阻止站点地图验证）。
5. 一旦收集器恢复，运行 `npm run test:widgets` 来执行
   重新发布之前进行分析助手单元测试。

### 事件发生后

1. 使用任何新收集器更新 [`devportal/observability`](./observability)
   限制或抽样要求。
2. 如果任何分析数据在外部被删除或编辑，则文件治理通知
   政策。

## 每季度弹性训练

在**每个季度的第一个星期二**（一月/四月/七月/十月）进行两次演习
或在任何重大基础设施变更后立即进行。将工件存储在
`artifacts/devportal/drills/<YYYYMMDD>/`。

|钻|步骤|证据|
| -----| -----| -------- |
| Alias 回滚演练 | 1. 使用最新的生产清单重播“部署失败”回滚。<br/>2.一旦探针通过，重新绑定到生产环境。<br/>3.在钻取文件夹中记录 `portal.manifest.submit.summary.json` 和探测日志。 | `rollback.submit.json`，探测输出，以及排练的释放标签。 |
|综合验证审核| 1. 针对生产和登台运行 `npm run probe:portal` 和 `npm run probe:tryit-proxy`。<br/>2.运行 `npm run check:links` 并存档 `build/link-report.json`。<br/>3.附上 Grafana 面板的屏幕截图/导出，确认探测成功。 |探测日志 + `link-report.json` 引用清单指纹。 |

将错过的演习上报给 Docs/DevRel 经理和 SRE 治理审查，
因为路线图需要确定性的季度证据，表明两者都别名
回滚和门户探针保持健康。

## PagerDuty 和待命协调

- PagerDuty 服务 **Docs Portal Publishing** 拥有从以下位置生成的警报
  `dashboards/grafana/docs_portal.json`。规则 `DocsPortal/GatewayRefusals`，
  `DocsPortal/AliasCache` 和 `DocsPortal/TLSExpiry` 页面 Docs/DevRel
  主存储 SRE 作为辅助存储。
- 寻呼时，请附上 `DOCS_RELEASE_TAG`，并附上受影响的屏幕截图
  Grafana 面板，以及之前事件注释中的链路探测/链路检查输出
  缓解措施开始。
- 缓解（回滚或重新部署）后，重新运行 `npm run probe:portal`，
  `npm run check:links`，并捕获显示指标的新 Grafana 快照
  回到阈值内。附上 PagerDuty 事件之前的所有证据
  解决它。
- 如果两个警报同时触发（例如 TLS 到期加上积压），则进行分类
  先拒绝（停止发布），执行回滚程序，然后清除
  桥接器上具有存储 SRE 的 TLS/积压项目。