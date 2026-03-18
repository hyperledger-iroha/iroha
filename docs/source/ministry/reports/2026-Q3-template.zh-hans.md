---
lang: zh-hans
direction: ltr
source: docs/source/ministry/reports/2026-Q3-template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f313d8010f2a7174c90f51dea512bcab6eb4a207df9199f28a7352944cb43c8b
source_last_modified: "2025-12-29T18:16:35.981653+00:00"
translation_last_reviewed: 2026-02-07
title: Ministry Transparency Report — 2026 Q3 (Template)
summary: Scaffold for the MINFO-8 quarterly transparency packet; replace all tokens before publication.
quarter: 2026-Q3
translator: machine-google-reviewed
---

<!--
  Usage:
    1. Copy this file when drafting a new quarter (e.g., 2026-Q3 → 2026-Q3.md).
    2. Replace every {{TOKEN}} marker and remove instructional callouts.
    3. Attach supporting artefacts (data appendix, CSVs, manifest, Grafana export) under artifacts/ministry/transparency/<YYYY-Q>/.
-->

# 执行摘要

> 提供一段关于审核准确性、申诉结果、拒绝名单流失和财务亮点的摘要。提及发布是否满足 T+14 截止日期。

## 季度回顾

### 亮点
- {{HIGHLIGHT_1}}
- {{HIGHLIGHT_2}}
- {{HIGHLIGHT_3}}

### 风险与缓解措施

|风险|影响 |缓解措施 |业主|状态 |
|------|--------|------------|--------|--------|
| {{RISK_1}} | {{影响}} | {{缓解}} | {{所有者}} | {{状态}} |
| {{RISK_2}} | {{影响}} | {{缓解}} | {{所有者}} | {{状态}} |

## 指标概述

所有指标均源自 DP sanitizer 运行后的 `ministry_transparency_builder`（Norito 捆绑包）。附上下面引用的相应 CSV 切片。

### AI 审核准确性

|型号简介 |地区 | FP 率（目标）| FN 利率（目标）|漂移与校准|样本量|笔记|
|--------------|--------|------------------|------------------|------------------------|----------|------|
| {{个人资料}} | {{地区}} | {{fp_rate}} ({{fp_target}}) | {{fn_rate}} ({{fn_target}}) | {{fn_rate}} {{漂移}} | {{样本}} | {{注释}} |

### 上诉和小组活动

|公制|价值| SLA 目标 |趋势与 Q-1 |笔记|
|--------|--------|------------|----------------|--------|
|收到的上诉 | {{appeals_received}} | {{sla}} | {{delta}} | {{注释}} |
|中值解决时间 | {{中位数分辨率}} | {{sla}} | {{delta}} | {{注释}} |
|反转率 | {{reversal_rate}} | {{目标}} | {{delta}} | {{注释}} |
|面板利用率| {{panel_utilization}} | {{目标}} | {{delta}} | {{注释}} |

### 拒绝名单和紧急佳能

|公制|计数 |差压噪声 (ε) |紧急标志| TTL 合规性 |笔记|
|--------|--------|--------------|------------------|----------------|--------------------|
|哈希添加 | {{补充}} | {{epsilon_counts}} | {{flags}} | {{ttl_status}} | {{注释}} |
|哈希删除 | {{删除}} | {{epsilon_counts}} | {{flags}} | {{ttl_status}} | {{注释}} |
|佳能调用 | {{canon_incalls}} |不适用 | {{flags}} | {{ttl_status}} | {{注释}} |

### 国债变动

|流量|金额 (MINFO) |来源参考|笔记|
|------|----------------|--------------------|--------|
|上诉存款 | {{金额}} | {{tx_ref}} | {{注释}} |
|小组奖励 | {{金额}} | {{tx_ref}} | {{注释}} |
|运营支出| {{金额}} | {{tx_ref}} | {{注释}} |

### 志愿者和外展信号

|公制|价值|目标|笔记|
|--------|--------|--------|--------|
|志愿者简介发布| {{值}} | {{目标}} | {{注释}} |
|涵盖的语言 | {{值}} | {{目标}} | {{注释}} |
|举办治理研讨会| {{值}} | {{目标}} | {{注释}} |

## 差异化隐私和消毒

总结消毒剂的运行情况并包括 RNG 的承诺。

- 消毒剂工作：`{{CI_JOB_URL}}`
- DP参数：ε={{epsilon_total}}，δ={{delta_total}}
- RNG承诺：`{{blake3_seed_commitment}}`
- 抑制的存储桶：{{suppressed_buckets}}
- 质量检查审核员：{{审核员}}

附上 `artifacts/ministry/transparency/{{Quarter}}/dp_report.json` 并记下任何手动干预。## 数据附件

|文物|路径| SHA-256 |已上传至 SoraFS？ |笔记|
|----------|------|---------|----------|--------|
|摘要 PDF | `artifacts/ministry/transparency/{{Quarter}}/summary.pdf` | {{哈希}} | {{是/否}} | {{注释}} |
| Norito 数据附录 | `artifacts/ministry/transparency/{{Quarter}}/data/appendix.norito` | {{哈希}} | {{是/否}} | {{注释}} |
|指标 CSV 包 | `artifacts/ministry/transparency/{{Quarter}}/data/csv/` | {{哈希}} | {{是/否}} | {{注释}} |
| Grafana 出口 | `dashboards/grafana/ministry_transparency_overview.json` | {{哈希}} | {{是/否}} | {{注释}} |
|警报规则| `dashboards/alerts/ministry_transparency_rules.yml` | {{哈希}} | {{是/否}} | {{注释}} |
|出处清单 | `artifacts/ministry/transparency/{{Quarter}}/manifest.json` | {{哈希}} | {{是/否}} | {{注释}} |
|清单签名 | `artifacts/ministry/transparency/{{Quarter}}/manifest.json.sig` | {{哈希}} | {{是/否}} | {{注释}} |

## 出版物元数据

|领域|价值|
|--------|--------|
|发布季度 | {{季度}} |
|发布时间戳 (UTC) | {{时间戳}} |
| SoraFS CID | `{{cid}}` |
|治理投票ID | {{vote_id}} |
|清单摘要 (`blake2b`) | `{{manifest_digest}}` |
| Git 提交/标记 | `{{git_rev}}` |
|发布所有者 | {{所有者}} |

## 批准

|角色 |名称 |决定|时间戳|笔记|
|------|------|----------|------------|--------|
|部委可观察性 TL | {{名称}} | ✅/⚠️ | {{时间戳}} | {{注释}} |
|治理委员会联络| {{名称}} | ✅/⚠️ | {{时间戳}} | {{注释}} |
|文档/通讯主管 | {{名称}} | ✅/⚠️ | {{时间戳}} | {{注释}} |

## 变更日志和后续行动

- {{CHANGELOG_ITEM_1}}
- {{CHANGELOG_ITEM_2}}

### 打开操作项

|项目 |业主|到期|状态 |笔记|
|------|--------|-----|--------|--------|
| {{行动}} | {{所有者}} | {{到期}} | {{状态}} | {{注释}} |

### 联系方式

- 主要联系人：{{contact_name}} (`{{chat_handle}}`)
- 升级路径：{{escalation_details}}
- 分发列表：{{mailing_list}}

_模板版本：2026-03-25。进行结构更改时更新修订日期。_