---
lang: zh-hans
direction: ltr
source: docs/fraud_playbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4ac4c98cc4aa6ab0c34e58e6428d0ee33eb9a0c3fdad9e6958bdc75f2a48dc66
source_last_modified: "2026-01-22T16:26:46.488648+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# 欺诈治理手册

本文档总结了 PSP 欺诈堆栈所需的脚手架，同时
完整的微服务和 SDK 正在积极开发中。它捕捉到
对分析、审计工作流程和后备程序的期望，以便
即将推出的实施可以安全地插入分类帐。

## 服务概览

1. **API网关** – 接收同步`RiskQuery`有效负载，将其转发到
   功能聚合，并将 `FraudAssessment` 响应转发回账本
   流动。需要高可用性（双活）；使用区域对
   确定性哈希以避免请求偏差。
2. **特征聚合** – 组成用于评分的特征向量。发出
   仅 `FeatureInput` 哈希值；敏感负载保持在链外。可观测性
   必须发布延迟直方图、队列深度计量器和重播计数器
   租户。
3. **风险引擎** – 评估规则/模型并产生确定性
   `FraudAssessment` 输出。确保规则执行顺序稳定并捕获
   每个评估 ID 的审核日志。

## 分析和模型推广

- **异常检测**：维护一个标记偏差的流作业
  每个租户的决策率。将警报输入治理仪表板和商店
  季度审查摘要。
- **图形分析**：每晚对关系导出运行图形遍历
  识别共谋集群。通过以下方式将结果导出到治理门户
  `GovernanceExport` 参考支持证据。
- **反馈摄取**：整理手动审核结果和退款报告。
  将它们转换为特征增量并将它们合并到训练数据集中。
  发布摄取状态指标，以便风险团队可以发现停滞的提要。
- **模型推广管道**：自动化候选人评估（离线指标，
  金丝雀评分，回滚准备）。促销活动应发出签名
  `FraudAssessment` 样本集并更新 `model_version` 字段
  `GovernanceExport`。

## 审核员工作流程

1. 快照最新的 `GovernanceExport` 并验证 `policy_digest` 是否匹配
   风险团队提供的清单。
2. 验证规则聚合是否与账本端决策总数一致
   采样窗口。
3.审查异常检测和图形分析报告是否有突出问题
   问题。记录升级情况和预期的修复所有者。
4. 签署审核清单并存档。将 Norito 编码的工件存储在
   可重复性的治理门户。

## 后备剧本

- **引擎停机**：如果风险引擎不可用超过 60 秒，
  网关应切换到仅审查模式，发出 `AssessmentDecision::Review`
  用于所有请求和警报操作员。
- **遥测差距**：当指标或跟踪落后时（丢失 5 分钟），
  停止自动型号促销并通知值班工程师。
- **模型回归**：如果部署后反馈表明欺诈行为增加
  损失，回滚到之前签名的模型包并更新路线图
  并采取纠正措施。

## 数据共享协议

- 维护特定管辖区的附录，涵盖保留、加密和
  违反通知 SLA。合作伙伴在接收前必须签署附录
  `FraudAssessment` 出口。
- 记录每个集成的数据最小化实践（例如，散列
  帐户标识符、截断卡号）。
- 每年或在监管要求发生变化时更新协议。

## API 模式

网关现在公开具体的 JSON 信封，这些信封一对一地映射到
`crates/iroha_data_model::fraud` 中实现的 Norito 类型：

- **风险摄入** – `POST /v1/fraud/query` 接受 `RiskQuery` 架构：
  - `query_id`（`[u8; 32]`，十六进制编码）
  - `subject`（`AccountId`，规范 I105 文字；可选 `@<domain>` 提示或别名）
  - `operation`（标记枚举匹配 `RiskOperation`；JSON `type`
    鉴别器镜像枚举变体）
  - `related_asset`（`AssetId`，可选）
  - `features`（`{ key: String, value_hash: hex32 }` 的数组映射自
    `FeatureInput`)
  - `issued_at_ms` (`u64`)
  - `context`（`RiskContext`；携带`tenant_id`，可选`session_id`，
    可选 `reason`)
- **风险决策** – `POST /v1/fraud/assessment` 消耗
  `FraudAssessment` 有效负载（也反映在治理导出中）：
  - `query_id`、`engine_id`、`risk_score_bps`、`confidence_bps`、
    `decision`（`AssessmentDecision` 枚举）、`rule_outcomes`
    （`{ rule_id, score_delta_bps, rationale? }` 数组）
  - `generated_at_ms`
  - `signature`（可选的 base64 包装 Norito 编码的评估）
- **治理导出** – `GET /v1/fraud/governance/export` 返回
  当 `governance` 功能启用时，`GovernanceExport` 结构，捆绑
  活动参数、最新颁布、模型版本、政策摘要和
  `DecisionAggregate` 直方图。

`crates/iroha_data_model/src/fraud/types.rs` 中的往返测试可确保这些
架构与 Norito 编解码器保持二进制一致，并且
`integration_tests/tests/fraud_monitoring_requires_assessment_bands.rs` 练习
端到端的完整接收/决策管道。

## PSP SDK 参考

以下语言存根跟踪面向 PSP 的集成示例：

- **生锈** – `integration_tests/tests/fraud_monitoring_requires_assessment_bands.rs`
  使用工作区 `iroha` 客户端制作 `RiskQuery` 元数据并验证
  录取失败/成功。
- **TypeScript** – `docs/source/governance_api.md` 记录 REST 表面
  由 PSP 演示仪表板中使用的轻量级 Torii 网关消耗；的
  脚本化客户端住在 `scripts/ci/schedule_fraud_scoring.sh` 中，因为烟雾
  演习。
- **Swift 和 Kotlin** – 现有 SDK（`IrohaSwift` 和
  `crates/iroha_cli/docs/multisig.md` 引用）公开 Torii 元数据
  附加 `fraud_assessment_*` 字段所需的挂钩。 PSP 特定的帮助程序是
  在“欺诈和遥测治理循环”里程碑下进行跟踪
  `status.md` 并重用这些 SDK 的交易构建器。

这些引用将与微服务网关保持同步，以便 PSP
实施者始终拥有最新的架构和每个示例代码路径
支持的语言。