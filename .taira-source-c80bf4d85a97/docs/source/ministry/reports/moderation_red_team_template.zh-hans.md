---
lang: zh-hans
direction: ltr
source: docs/source/ministry/reports/moderation_red_team_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 22bfdf5696bf3a58e7899e7d7b2ba77e404a05fa81304f12d6c78eeb1e8035e5
source_last_modified: "2025-12-29T18:16:35.982646+00:00"
translation_last_reviewed: 2026-02-07
title: Red-Team Drill Report Template
summary: Copy this file for every MINFO-9 drill to capture metadata, evidence, and remediation actions.
translator: machine-google-reviewed
---

> **如何使用：** 每次练习后立即将此模板复制到 `docs/source/ministry/reports/<YYYY-MM>-mod-red-team-<scenario>.md`。保持文件名小写、连字符并与 Alertmanager 中记录的钻取 ID 对齐。

# 红队演习报告 — `<SCENARIO NAME>`

- **钻头 ID：** `<YYYYMMDD>-<scenario>`
- **日期和窗口：** `<YYYY-MM-DD HH:MMZ – HH:MMZ>`
- **场景类别：** `smuggling | bribery | gateway | ...`
- **运营商：** `<names / handles>`
- **仪表板因提交而冻结：** `<git SHA>`
- **证据包：** `artifacts/ministry/red-team/<YYYY-MM>/<scenario>/`
- **SoraFS CID（可选）：** `<cid>`  
- **相关路线图项目：** `MINFO-9`，以及任何链接的票证。

## 1. 目标和进入条件

- **主要目标**
  - `<e.g. Verify denylist TTL enforcement under smuggling attack>`
- **先决条件已确认**
  - `emergency_canon_policy.md` 版本 `<tag>`
  - `dashboards/grafana/ministry_moderation_overview.json` 摘要 `<sha256>`
  - 覆盖权限待命：`<name>`

## 2. 执行时间线

|时间戳 (UTC) |演员 |行动/命令|结果/注释|
|----------------|--------------------|--------------------------------|----------------|
|  |  |  |  |

> 包括 Torii 请求 ID、块哈希、覆盖批准和 Alertmanager 链接。

## 3. 观察和指标

|公制|目标|观察|通过/失败 |笔记|
|--------|--------|----------|------------|--------|
|警报响应延迟| `<X> min` | `<Y> min` | ✅/⚠️ |  |
|审核检测率 | `>= <value>` |  |  |  |
|网关异常检测| `Alert fired` |  |  |  |

- `Grafana export:` `artifacts/.../dashboards/ministry_moderation_overview.json`
- `Alert bundle:` `artifacts/.../alerts/ministry_moderation_rules.yml`
- `Norito manifests:` `<path>`

## 4. 调查结果与补救措施

|严重性 |寻找|业主|目标日期 |状态/链接|
|----------|---------|--------|-------------|----------------|
|高|  |  |  |  |

记录校准清单、拒绝名单策略或 SDK/工具必须如何更改。链接到 GitHub/Jira 问题并记下阻止/未阻止状态。

## 5. 治理和批准

- **事件指挥官签字：** `<name / timestamp>`
- **管理委员会审查日期：** `<meeting id>`
- **后续检查清单：** `[ ] status.md updated`、`[ ] roadmap row updated`、`[ ] transparency packet annotated`

## 6. 附件

- `[ ] CLI logbook (`logs/.md`)`
- `[ ] Dashboard JSON export`
- `[ ] Alertmanager history`
- `[ ] SoraFS manifest / CAR`
- `[ ] Override audit log`

一旦上传到证据包和 SoraFS 快照，就用 `[x]` 标记每个附件。

---

_最后更新：{{ 日期 |默认（“2026-02-20”）}}_