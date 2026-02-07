---
lang: zh-hans
direction: ltr
source: docs/examples/soranet_testnet_stage_gate/stage_gate_report_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 06dcd662ffb2b22a13d9cd5418f5d2e8e64a4cdadb71a054488ce75b1eb96188
source_last_modified: "2025-12-29T18:16:35.094764+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SNNet-10 阶段门报告 (T?_→T?_)

> 提交前替换每个占位符（尖括号中的项目）。保留
> 节标题，以便治理自动化可以解析文件。

## 1. 元数据

|领域|价值|
|--------|--------|
|促销| `<T0→T1 or T1→T2>` |
|举报窗口| `<YYYY-MM-DD → YYYY-MM-DD>` |
|范围内的继电器 | `<count + IDs or “see appendix A”>` |
|主要联系人 | `<name/email/Matrix handle>` |
|提交存档 | `<snnet10-stage-gate-YYYYMMDD.tar.zst>` |
|存档 SHA-256 | `<sha256:...>` |

## 2. 指标总结

|公制|观察|门槛|经过？ |来源 |
|--------|----------|------------|--------|--------|
|电路成功率| `<0.000>` | ≥0.95 | ○ / ☑ | `reports/metrics-report.json` |
|获取掉电率| `<0.000>` | ≤0.01 | ○ / ☑ | `reports/metrics-report.json` |
| GAR 混合方差 | `<+0.0%>` | ≤±10% | ○ / ☑ | `reports/metrics-report.json` |
| PoW p95 秒 | `<0.0 s>` | ≤3秒| ○ / ☑ | `telemetry/pow_window.json` |
|延迟 p95 | `<0 ms>` | <200 毫秒 | ○ / ☑ | `telemetry/latency_window.json` |
| PQ 比率（平均）| `<0.00>` | ≥目标| ○ / ☑ | `telemetry/pq_summary.json` |

**叙述：** `<summaries of anomalies, mitigations, overrides>`

## 3. 演习和事件日志

|时间戳 (UTC) |地区 |类型 |警报 ID |缓解总结|
|----------------|--------|------|----------|--------------------|
| `<YYYY-MM-DD HH:MM>` | `<region>` | `Brownout drill` | `<alert://...>` | `<restored anon-guard-pq in 3m12s>` |

## 4. 附件和哈希值

|文物|路径| SHA-256 |
|----------|------|---------|
|指标快照 | `reports/metrics-window.json` | `<sha256>` |
|指标报告| `reports/metrics-report.json` | `<sha256>` |
|后卫轮换成绩单| `evidence/guard_rotation/*.log` | `<sha256>` |
|退出绑定清单 | `evidence/exit_bonds/*.to` | `<sha256>` |
|钻井日志| `evidence/drills/*.md` | `<sha256>` |
|面具准备（T1→T2）| `reports/masque-readiness.md` | `<sha256 or n/a>` |
|回滚计划（T1→T2）| `reports/downgrade_plan.md` | `<sha256 or n/a>` |

## 5. 批准

|角色 |名称 |签名（是/否）|笔记|
|------|------|--------------|--------|
|网络 TL | `<name>` | ○ / ☑ | `<comments>` |
|治理代表| `<name>` | ○ / ☑ | `<comments>` |
| SRE 代表 | `<name>` | ○ / ☑ | `<comments>` |

## 附录 A — 接力名单

```
- relay-id-001 (AS64496, region=SJC)
- relay-id-002 (AS64497, region=NRT)
...
```

## 附录 B — 事件摘要

```
<Detailed context for any incidents or overrides referenced above.>
```