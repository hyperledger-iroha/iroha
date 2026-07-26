---
lang: zh-hans
direction: ltr
source: docs/source/ministry/transparency_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2639f4f7692e13ed61cc6246c87b047dc7415a6c9243ca7c046e6ccea8b55e9a
source_last_modified: "2025-12-29T18:16:35.983537+00:00"
translation_last_reviewed: 2026-02-07
title: Ministry Transparency & Audit Plan
summary: Implementation plan for roadmap item MINFO-8 covering quarterly transparency reports, privacy guardrails, dashboards, and automation.
translator: machine-google-reviewed
---

# 透明度和审计报告 (MINFO-8)

路线图参考：**MINFO-8 — 透明度和审计报告** 和 **MINFO-8a — 隐私保护发布流程**

信息部必须发布确定性透明度制品，以便社区能够审核审核效率、申诉处理和黑名单流失情况。本文档定义了在 2026 年第 3 季度目标之前关闭 MINFO-8 所需的范围、工件、隐私控制和操作工作流程。

## 目标和可交付成果

- 制作季度透明度数据包，总结 AI 审核准确性、上诉结果、拒绝名单流失、志愿者小组活动以及与 MINFO 预算相关的财务变动。
- 发送随附的原始数据包 (Norito JSON + CSV) 以及仪表板，以便公民无需等待静态 PDF 即可对指标进行切片。
- 在发布任何数据集之前强制执行隐私保证（差异隐私+最小计数规则）和签名证明。
- 在治理 DAG 和 SoraFS 中记录每个出版物，以便历史文物保持不可变且可独立验证。

### 人工制品矩阵

|文物 |描述 |格式|存储|
|----------|-------------|--------|---------|
|透明度摘要|人类可读的报告，包含执行摘要、要点、风险项目 |降价 → PDF | `docs/source/ministry/reports/<YYYY-Q>.md` → `artifacts/ministry/transparency/<YYYY-Q>/summary.pdf` |
|数据附录| Canonical Norito 捆绑包，包含已清理的表（`ModerationLedgerBlockV1`、上诉、黑名单增量）| `.norito` + `.json` | `artifacts/ministry/transparency/<YYYY-Q>/data`（镜像到 SoraFS CID）|
|指标 CSV |仪表板的便捷 CSV 导出（AI FP/FN、上诉 SLA、拒绝名单流失）| `.csv` |相同的目录，散列和签名 |
|仪表板快照 | `ministry_transparency_overview` 面板的 Grafana JSON 导出 + 警报规则 | `.json` | `dashboards/grafana/ministry_transparency_overview.json` / `dashboards/alerts/ministry_transparency_rules.yml` |
|出处清单 | Norito 清单绑定摘要、SoraFS CID、签名、发布时间戳 | `.json` + 独立签名 | `artifacts/ministry/transparency/<YYYY-Q>/manifest.json(.sig)`（还附有治理投票）|

## 数据源和管道

|来源 |饲料|笔记|
|--------|------|--------|
|审核分类账 (`docs/source/sorafs_transparency_plan.md`) |每小时 `ModerationLedgerBlockV1` 导出存储在 CAR 文件中 | SFM-4c 已经上线；重新用于季度汇总。 |
| AI标定+误报率| `docs/source/sorafs_ai_moderation_plan.md` 夹具 + 校准清单 (`docs/examples/ai_moderation_calibration_*.json`) |按策略、区域和模型配置文件聚合的指标。 |
|上诉登记册 | MINFO-7 金库工具发出的 Norito `AppealCaseV1` 事件 |包含权益转让、小组名册、SLA 计时器。 |
|拒绝者流失 |来自 Merkle 注册表 (MINFO-6) 的 `MinistryDenylistChangeV1` 事件 |包括哈希族、TTL、紧急规范标志。 |
|国库流动| `MinistryTreasuryTransferV1` 事件（申诉存款、小组奖励）|与 `finance/mminfo_gl.csv` 平衡。 |紧急规范治理、TTL 限制和审查要求现已生效
[`docs/source/ministry/emergency_canon_policy.md`](emergency_canon_policy.md)，确保
流失指标捕获层（`standard`、`emergency`、`permanent`）、canon id、
并审查 Torii 在加载时强制执行的截止日期。

处理阶段：
1. **将**原始事件摄取到 `ministry_transparency_ingest`（Rust 服务镜像透明账本摄取器）。每晚运行，幂等。
2. **每季度总计** `ministry_transparency_builder`。在隐私过滤器之前输出 Norito 数据附录以及每个指标的表。
3. 通过 `cargo xtask ministry-transparency sanitize`（或 `scripts/ministry/dp_sanitizer.py`）**清理**指标，并发出带有元数据的 CSV/JSON 切片。
4. **打包**工件，使用 `ministry_release_signer` 对其进行签名，并上传到 SoraFS + 治理 DAG。

## 2026 年第 3 季度参考版本

- 首个治理门控捆绑包（2026-Q3）于 2026-10-07 通过 `make check-ministry-transparency` 生成。工件位于 `artifacts/ministry/transparency/2026-Q3/`（包括 `sanitized_metrics.json`、`dp_report.json`、`summary.md`、`checksums.sha256`、`transparency_manifest.json` 和 `transparency_release_action.json`）中，并被镜像到SoraFS CID `7f4c2d81a6b13579ccddeeff00112233`。
- `docs/source/ministry/reports/2026-Q3.md` 中捕获了出版物详细信息、指标表和批准​​，该文件现在作为审核员审查第三季度窗口的规范参考。
- CI 在版本离开暂存之前应用 `ci/check_ministry_transparency.sh` / `make check-ministry-transparency`，验证工件摘要、Grafana/警报哈希和清单元数据，以便未来每个季度都遵循相同的证据线索。

## 指标和仪表板

Grafana 仪表板 (`dashboards/grafana/ministry_transparency_overview.json`) 显示以下面板：

- AI 调节精度：每个模型的 FP/FN 速率、漂移与校准目标以及与 `docs/source/sorafs/reports/ai_moderation_calibration_*.md` 相关的警报阈值。
- 申诉生命周期：提交、SLA 合规性、撤销、债券销毁、每层积压。
- 拒绝列表流失：每个哈希系列的添加/删除、TTL 过期、紧急规范调用。
- 志愿者简介和小组多样性：每种语言的提交内容、利益冲突披露、发布滞后。 `docs/source/ministry/volunteer_brief_template.md` 中指定了平衡的简要字段，确保事实表和审核标签是机器可读的。
- 国库余额：存款、支出、未偿债务（提供给 MINFO-7）。

警报规则（编入 `dashboards/alerts/ministry_transparency_rules.yml`）涵盖：
- FP/FN 相对于校准基线的偏差 >25%。
- 上诉 SLA 错过率每季度 >5%。
- 紧急规范 TTL 早于政策。
- 发布滞后于季度结束后 >14 天。

## 隐私和发布护栏 (MINFO-8a)|公制类别 |机制|参数|额外的警卫|
|--------------|-----------|------------|--------------------|
|计数（上诉、黑名单变更、志愿者简介）|拉普拉斯噪声 | ε=0.75 每季度，δ=1e-6 |抑制后置噪声值<5的桶；每季度每个演员的剪辑贡献为 1。 |
|人工智能准确度 |分子/分母上的高斯噪声 | ε=0.5，δ=1e-6 |仅当净化样本数≥50（`min_accuracy_samples` 下限）时才发布并发布置信区间。 |
|国库流动|无噪音（已在链上公开）| — |屏蔽账户名称（资金库 ID 除外）；包括默克尔证明。 |

发布要求：
- 差异化隐私报告包括 epsilon/delta 账本和 RNG 种子承诺 (`blake3(seed)`)。
- 敏感示例（证据哈希）已被编辑，除非已在公开 Merkle 收据中。
- 修订日志附加到描述所有已删除字段和理由的摘要中。

## 发布工作流程和时间表

| T 型窗 |任务|所有者 |证据|
|----------|------|----------|----------|
|季度收盘后 T+3d |冻结原始导出，触发聚合作业 |部委可观察性 TL | `ministry_transparency_ingest.log`，管道作业 ID |
| T+7 天 |查看原始指标，进行 DP 消毒剂试运行 |数据信任团队 |消毒剂报告 (`artifacts/.../dp_report.json`) |
| T+10 天 |摘要草案+数据附录|文档/DevRel + 政策分析师 | `docs/source/ministry/reports/<YYYY-Q>.md` |
| T+12 天 |签署文物，制作清单，上传至 SoraFS |运营/治理秘书处| `manifest.json(.sig)`、SoraFS CID |
| T+14 天 |发布仪表板+警报，发布治理公告|可观察性 + 通讯 | Grafana 导出、警报规则哈希、治理投票链接 |

每个版本必须经过以下人员的批准：
1. 部可观测性TL（数据完整性）
2. 治理委员会联络（政策）
3. 文档/通讯主管（公开措辞）

## 自动化和证据存储- 使用 `cargo xtask ministry-transparency ingest` 从原始源（分类账、上诉、拒绝名单、财务、志愿者）构建季度快照。跟进 `cargo xtask ministry-transparency build`，在发布之前发出仪表板指标 JSON 以及签名的清单。
- 红队链接：将一个或多个 `--red-team-report docs/source/ministry/reports/<YYYY-MM>-mod-red-team-<scenario>.md` 文件传递​​到摄取步骤，以便透明度快照和清理后的指标携带钻探 ID、场景类、证据包路径和仪表板 SHA 以及分类帐/上诉/拒绝列表数据。这使得 MINFO-9 演练节奏反映在每个透明数据包中，无需手动编辑。
- 志愿者提交的材料必须遵循 `docs/source/ministry/volunteer_brief_template.md`（例如：`docs/examples/ministry/volunteer_brief_template.json`）。摄取步骤需要这些对象的 JSON 数组，自动过滤 `moderation.off_topic` 条目，强制披露证明，并记录事实表覆盖范围，以便仪表板可以突出显示缺失的引用。
- 额外的自动化位于 `scripts/ministry/` 下。 `dp_sanitizer.py` 包装 `cargo xtask ministry-transparency sanitize` 命令，而 `transparency_release.py`（与出处工具一起添加）现在打包工件，从 `sorafs_cli car pack|proof verify` 摘要（或显式 `--sorafs-cid`）派生 SoraFS CID，并写入`transparency_manifest.json` 和 `transparency_release_action.json`（捕获清单摘要、SoraFS CID 和仪表板 git SHA 的 `TransparencyReleaseV1` 治理负载）。将 `--governance-dir <path>` 传递到 `transparency_release.py`（或运行 `cargo xtask ministry-transparency anchor --action artifacts/.../transparency_release_action.json --governance-dir <path>`）以对 Norito 负载进行编码，并在发布之前将其（加上 JSON 摘要）放入治理 DAG 目录中。同一标志还在 `<governance-dir>/publisher/head_requests/ministry_transparency/` 下发出 `MinistryTransparencyHeadUpdateV1` 请求，引用季度、SoraFS CID、清单路径和 IPNS 密钥别名（可通过 `--ipns-key` 覆盖）。提供 `--auto-head-update` 以通过 `publisher_head_updater.py` 立即处理该请求，当需要同时发布 IPNS 时，可以选择传递 `--head-update-ipns-template '/usr/local/bin/ipfs name publish --key {ipns_key} /ipfs/{cid}'`。否则，稍后运行 `scripts/ministry/publisher_head_updater.py --governance-dir <path>`（如果需要，使用相同的模板）以排空队列，追加 `publisher/head_updates.log`，更新 `publisher/ipns_heads/<key>.json`，并将处理后的 JSON 存档在 `head_requests/ministry_transparency/processed/` 下。
- 工件存储在 `artifacts/ministry/transparency/<YYYY-Q>/` 下，带有由发布密钥签名的 `checksums.sha256` 文件。该树现在在 `artifacts/ministry/transparency/2026-Q3/` 上带有参考包（经过清理的指标、DP 报告、摘要、清单、治理操作），以便工程师可以离线测试工具，`scripts/ministry/check_transparency_release.py` 在本地验证摘要/季度元数据，而 `ci/check_ministry_transparency.sh` 在上传证据之前在 CI 中运行相同的验证。检查器现在强制执行记录的 DP 预算（对于计数，ε≤0.75，对于精度，ε≤0.5，δ≤1e−6），并且每当 `min_accuracy_samples` 或抑制阈值漂移时，或者当存储桶泄漏低于这些下限的值而不被抑制时，构建都会失败。将脚本视为路线图 (MINFO-8) 和 CI 之间的契约：如果隐私参数发生变化，请一起调整上表和检查器。- 治理锚点：创建引用清单摘要、SoraFS CID 和仪表板 git SHA 的 `TransparencyReleaseV1` 操作（`iroha_data_model::ministry::TransparencyReleaseV1` 定义规范有效负载）。

## 未完成的任务和后续步骤

|任务|状态 |笔记|
|------|--------|--------|
|实施 `ministry_transparency_ingest` + 构建器作业 | 🈺 进行中 | `cargo xtask ministry-transparency ingest|build` 现在缝合账本/上诉/拒绝名单/财务提要；剩下的工作连接 DP sanitizer + 发布脚本管道。 |
|发布 Grafana 仪表板 + 警报包 | 🈴已完成 |仪表板 + 警报文件位于 `dashboards/grafana/ministry_transparency_overview.json` 和 `dashboards/alerts/ministry_transparency_rules.yml` 下；在推出期间将它们连接到 PagerDuty `ministry-transparency`。 |
|自动化 DP 消毒剂 + 来源清单 | 🈴已完成 | `cargo xtask ministry-transparency sanitize`（包装器：`scripts/ministry/dp_sanitizer.py`）发出经过清理的指标 + DP 报告，`scripts/ministry/transparency_release.py` 现在写入 `checksums.sha256` 加上 `transparency_manifest.json` 以获取来源。 |
|创建季度报告模板(`reports/<YYYY-Q>.md`) | 🈴已完成 |模板添加于 `docs/source/ministry/reports/2026-Q3-template.md`；每季度复制/重命名并在发布前替换 `{{...}}` 令牌。 |
|线治理 DAG 锚定 | 🈴已完成 | `TransparencyReleaseV1` 位于 `iroha_data_model::ministry` 中，`scripts/ministry/transparency_release.py` 发出 JSON 有效负载，`cargo xtask ministry-transparency anchor` 将 `.to` 工件编码到配置的治理 DAG 目录中，以便发布者可以自动获取版本。 |

交付文档、仪表板规范和工作流程将 MINFO-8 从 🈳 移动到 🈺。剩余的工程任务（作业、脚本、警报接线）在上表中进行跟踪，并应在第一个 Q32026 发布之前完成。