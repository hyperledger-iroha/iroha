---
lang: zh-hans
direction: ltr
source: docs/source/governance_pipeline.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f9f765fbe3170f654a9c44c3cd1afc5d82a72ff49137f32b98cf9d310faf114e
source_last_modified: "2025-12-29T18:16:35.963528+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% 治理管道（Iroha 2 和 SORA 议会）

# 当前状态 (v1)
- 治理提案的运行顺序为：提案者→公投→计票→颁布。公投窗口和投票率/批准阈值按照 `gov.md` 中所述执行；锁只能延长并在到期时解锁。
- 议会选举使用基于 VRF 的抽签，具有确定性排序和任期限制；当不存在持久名册时，Torii 使用 `gov.parliament_*` 配置派生回退。在 `gov_parliament_bodies` / `gov_pipeline_sla` 测试中执行理事会门控和仲裁检查。
- 投票模式：ZK（默认，需要带内联字节的 `Active` VK）和 Plain（二次权重）。模式不匹配被拒绝；锁创建/扩展在两种模式下都是单调的，具有 ZK 和普通重新投票的回归测试。
- 验证者的不当行为通过证据管道（`/v1/sumeragi/evidence*`，CLI 助手）进行处理，并由 `NextMode` + `ModeActivationHeight` 强制执行联合共识交接。
- 受保护的命名空间、运行时升级挂钩和治理清单准入记录在 `governance_api.md` 中，并由遥测覆盖（`governance_manifest_*`、`governance_protected_namespace_total`）。

# 飞行中/积压订单
- 发布 VRF 抽签工件（种子、证明、有序名单、替补）并制定缺席替换规则；为抽签和替换添加黄金赛程。
- 议会机构的阶段 SLA 执行（规则 → 议程 → 研究 → 审查 → 陪审团 → 颁布）需要明确的计时器、升级路径和遥测计数器。
- 政策陪审团秘密/承诺披露投票和相关的反贿赂审计仍有待实施。
- 角色纽带乘数、高风险机构的不当行为削减以及服务时段之间的冷却需要配置管道和测试。
- 在 `gov.md`/`status.md` 中跟踪治理车道密封和全民投票窗口/道岔门；随着剩余的验收测试的进行，保持路线图条目的更新。