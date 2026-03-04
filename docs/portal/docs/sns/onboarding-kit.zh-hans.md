---
lang: zh-hans
direction: ltr
source: docs/portal/docs/sns/onboarding-kit.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1c90050703841af7b2f468ead9e23445ba68344cb9c4db5d7271a8af33a8cb91
source_last_modified: "2025-12-29T18:16:35.174056+00:00"
translation_last_reviewed: 2026-02-07
title: SNS metrics & onboarding kit
description: Dashboard, pricing, and automation artifacts referenced by roadmap item SN-8.
translator: machine-google-reviewed
---

# SNS 指标和入门套件

路线图项目 **SN-8** 捆绑了两个承诺：

1. 发布仪表板，显示注册、续订、ARPU、争议和
   冻结 `.sora`、`.nexus` 和 `.dao` 的窗口。
2. 提供入门套件，以便注册商和管理员可以连接 DNS、定价和
   在任何后缀上线之前，API 保持一致。

此页面镜像源版本
[`docs/source/sns/onboarding_kit.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns/onboarding_kit.md)
因此外部评审员可以遵循相同的程序。

## 1. 度量捆绑

### Grafana 仪表板和门户嵌入

- 将 `dashboards/grafana/sns_suffix_analytics.json` 导入 Grafana（或另一个
  分析主机）通过标准 API：

```bash
curl -H "Content-Type: application/json" \
     -H "Authorization: Bearer ${GRAFANA_TOKEN}" \
     -X POST https://grafana.sora.net/api/dashboards/db \
     --data-binary @dashboards/grafana/sns_suffix_analytics.json
```

- 相同的 JSON 为该门户页面的 iframe 提供支持（请参阅 **SNS KPI 仪表板**）。
  每当你撞到仪表板时，就跑
  `npm run build && npm run serve-verified-preview`里面的`docs/portal`到
  确认 Grafana 和嵌入保持同步。

### 面板和证据

|面板|指标|治理证据|
|--------|---------|---------------------|
|注册和续订 | `sns_registrar_status_total`（成功+续订解析器标签）|每个后缀的吞吐量 + SLA 跟踪。 |
| ARPU / 净单位 | `sns_bulk_release_payment_net_units`，`sns_bulk_release_payment_gross_units` |财务部门可以将注册商清单与收入进行匹配。 |
|争议和冻结 | `guardian_freeze_active`、`sns_dispute_outcome_total`、`sns_governance_activation_total` |显示活动冻结、仲裁节奏和监护人工作负载。 |
| SLA/错误率 | `torii_request_duration_seconds`，`sns_registrar_status_total{status="error"}` |在 API 回归影响客户之前突出显示它们。 |
|批量清单跟踪器 | `sns_bulk_release_manifest_total`，带有 `manifest_id` 标签的付款指标 |将 CSV drop 连接到结算票据。 |

在每月 KPI 期间从 Grafana（或嵌入式 iframe）导出 PDF/CSV
审查并将其附加到相关附件条目中
`docs/source/sns/regulatory/<suffix>/YYYY-MM.md`。管理员还捕获了 SHA-256
`docs/source/sns/reports/` 下导出的包的名称（例如，
`steward_scorecard_2026q1.md`），因此审计可以重播证据路径。

### 附件自动化

直接从仪表板导出生成附件文件，以便审阅者获得
一致摘要：

```bash
cargo xtask sns-annex \
  --suffix .sora \
  --cycle 2026-03 \
  --dashboard dashboards/grafana/sns_suffix_analytics.json \
  --dashboard-artifact artifacts/sns/regulatory/.sora/2026-03/sns_suffix_analytics.json \
  --output docs/source/sns/reports/.sora/2026-03.md \
  --regulatory-entry docs/source/sns/regulatory/eu-dsa/2026-03.md \
  --portal-entry docs/portal/docs/sns/regulatory/eu-dsa-2026-03.md
```

- 助手对导出进行哈希处理，捕获 UID/标签/面板计数，并写入
  `docs/source/sns/reports/.<suffix>/<cycle>.md` 下的 Markdown 附件（参见
  `.sora/2026-03` 示例与本文档一起提交）。
- `--dashboard-artifact` 将导出复制到
  `artifacts/sns/regulatory/<suffix>/<cycle>/` 因此附件引用了
  规范证据路径；仅当需要指向时才使用 `--dashboard-label`
  在带外存档中。
- `--regulatory-entry` 指向管理备忘录。助手插入（或
  替换）记录附件路径、仪表板的 `KPI Dashboard Annex` 块
  人工制品、摘要和时间戳，以便证据在重新运行后保持同步。
- `--portal-entry` 保留 Docusaurus 副本 (`docs/portal/docs/sns/regulatory/*.md`)
  对齐，因此审阅者不必手动区分单独的附件摘要。
- 如果您跳过 `--regulatory-entry`/`--portal-entry`，请将生成的文件附加到
  手动保存备忘录，并上传从 Grafana 捕获的 PDF/CSV 快照。
- 对于经常性导出，请列出后缀/周期对
  `docs/source/sns/regulatory/annex_jobs.json` 并运行
  `python3 scripts/run_sns_annex_jobs.py --verbose`。助手走过每个入口，
  复制仪表板导出（默认为 `dashboards/grafana/sns_suffix_analytics.json`
  未指定时），并刷新每个监管内的附件块（并且，
  如果可用，门户）一次性备忘录。
- 运行 `python3 scripts/check_sns_annex_schedule.py --jobs docs/source/sns/regulatory/annex_jobs.json --regulatory-root docs/source/sns/regulatory --report-root docs/source/sns/reports`（或 `make check-sns-annex`）以证明作业列表保持排序/重复数据删除状态，每个备忘录都带有匹配的 `sns-annex` 标记，并且附件存根存在。帮助程序在治理数据包中使用的区域设置/哈希摘要旁边写入 `artifacts/sns/annex_schedule_summary.json`。
这消除了手动复制/粘贴步骤，并保持 SN-8 附件证据的一致性，同时
保护 CI 中的时间表、标记和定位漂移。

## 2. 入门套件组件

### 后缀接线

- 注册表架构+选择器规则：
  [`docs/source/sns/registry_schema.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns/registry_schema.md)
  和 [`docs/source/sns/local_to_global_toolkit.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns/local_to_global_toolkit.md)。
- DNS 骨架助手：
  [`scripts/sns_zonefile_skeleton.py`](https://github.com/hyperledger-iroha/iroha/blob/master/scripts/sns_zonefile_skeleton.py)
  与捕捉到的排练流程
  [网关/DNS 操作手册](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_gateway_dns_owner_runbook.md)。
- 对于每次注册商启动，请在下面提交一份简短说明
  `docs/source/sns/reports/` 总结选择器示例、GAR 证明和 DNS 哈希。

### 定价备忘单

|标签长度|基本费用（美元等值）|
|--------------|---------------------|
| 3 | 240 美元 |
| 4 | 90 美元 |
| 5 | 30 美元 |
| 6-9 | 6-9 12 美元 |
| 10+ | 8 美元 |

后缀系数：`.sora` = 1.0×、`.nexus` = 0.8×、`.dao` = 1.3×。  
期限乘数：2年-5%，5年-12%；宽限期 = 30 天，赎回
= 60 天（20% 费用，最低 5 美元，最高 200 美元）。记录协商的偏差
登记员票。

### 优质拍卖与续订

1. **溢价池** — 密封投标提交/揭示 (SN-3)。跟踪出价
   `sns_premium_commit_total`，并在下面发布清单
   `docs/source/sns/reports/`。
2. **荷兰重新开放** - 宽限+赎回到期后，开始 7 天的荷兰销售
   在 10× 时，每天衰减 15%。标签显示为 `manifest_id`，因此
   仪表板可以显示进度。
3. **续订** — 监控 `sns_registrar_status_total{resolver="renewal"}` 和
   获取自动续订清单（通知、SLA、后备付款方式）
   内登记员票证。

### 开发者 API 和自动化

- API 合约：[`docs/source/sns/registrar_api.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns/registrar_api.md)。
- 批量助手和 CSV 架构：
  [`docs/source/sns/bulk_onboarding_toolkit.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns/bulk_onboarding_toolkit.md)。
- 命令示例：

```bash
python3 scripts/sns_bulk_onboard.py registrations.csv \
  --ndjson artifacts/sns/releases/2026q2/requests.ndjson \
  --submission-log artifacts/sns/releases/2026q2/submissions.log \
  --submit-torii-url https://torii.sora.net \
  --submit-token-file ~/.config/sora/tokens/registrar.token
```

在 KPI 仪表板筛选器中包含清单 ID（`--submission-log` 输出）
因此财务部门可以协调每个版本的收入面板。

### 证据包

1. 包含联系方式、后缀范围和付款方式的注册商票据。
2. DNS/解析器证据（区域文件框架 + GAR 证明）。
3. 定价工作表+治理批准的任何覆盖。
4. API/CLI 冒烟测试工件（`curl` 样本、CLI 记录）。
5. KPI仪表板截图+CSV导出，附在每月附件中。

## 3.启动清单

|步骤|业主|文物|
|------|--------|----------|
|仪表板进口|产品分析 | Grafana API 响应 + 仪表板 UID |
|门户嵌入已验证 |文档/开发版本 | `npm run build` 日志 + 预览截图 |
| DNS 演练完成 |网络/运营 | `sns_zonefile_skeleton.py` 输出 + 运行手册日志 |
|注册商自动化试运行 |注册商工程师 | `sns_bulk_onboard.py` 提交日志 |
|提交治理证据 |治理委员会|附件链接 + 导出仪表板的 SHA-256 |

在激活注册商或后缀之前，请完成清单。所签署的
捆绑包清除了 SN-8 路线图大门，并为审核员提供了单一参考
审查市场发布。