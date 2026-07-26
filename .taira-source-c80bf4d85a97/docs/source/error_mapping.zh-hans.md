---
lang: zh-hans
direction: ltr
source: docs/source/error_mapping.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: cba8780bcec4ebf562dc9c5725f328b0ea2d9009517efa5b5a504e2fb6be81fe
source_last_modified: "2026-01-11T04:52:11.136647+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# 错误映射指南

最后更新：2025-08-21

本指南将 Iroha 中的常见故障模式映射到数据模型显示的稳定错误类别。使用它来设计测试并使客户端错误处理可预测。

原则
- 指令和查询路径发出结构化枚举。避免恐慌；尽可能报告特定类别。
- 类别是稳定的，消息可能会演变。客户端应该根据类别进行匹配，而不是根据自由格式的字符串进行匹配。

类别
-InstructionExecutionError::Find：实体缺失（资产、账户、域、NFT、角色、触发器、权限、公钥、区块、交易）。示例：删除不存在的元数据键会产生 Find(MetadataKey)。
-InstructionExecutionError::Repetition：重复注册或ID冲突。包含指令类型和重复的IdBox。
-InstructionExecutionError::Mintability：违反 Mintability 不变性（`Once` 耗尽两次、`Limited(n)` 透支或尝试禁用 `Infinitely`）。示例：铸造定义为 `Once` 的资产两次，产生 `Mintability(MintUnmintable)`；配置 `Limited(0)` 会产生 `Mintability(InvalidMintabilityTokens)`。
-InstructionExecutionError::Math：数字域错误（溢出、被零除、负值、数量不足）。示例：燃烧超过可用量会产生 Math(NotEnoughQuantity)。
-InstructionExecutionError::InvalidParameter：无效的指令参数或配置（例如，过去的时间触发）。用于格式错误的合约有效负载。
-InstructionExecutionError::Evaluate：指令形状或类型的DSL/规范不匹配。示例：资产值的错误数字规格会产生 Evaluate(Type(AssetNumericSpec(..)))。
-InstructionExecutionError::InvariantViolation：违反不能用其他类别表达的系统不变量。示例：尝试删除最后一个签名者。
-InstructionExecutionError::Query：当指令执行期间查询失败时，对 QueryExecutionFail 进行包装。

查询执行失败
- 查找：查询上下文中缺少实体。
- 转换：查询所期望的类型错误。
- NotFound：缺少实时查询游标。
- CursorMismatch / CursorDone：光标协议错误。
- FetchSizeTooBig：超出服务器强制限制。
- GasBudgetExceeded：查询执行超出了gas/具体化预算。
- InvalidSingularParameters：单一查询不支持参数。
-CapacityLimit：已达到实时查询存储容量。

测试技巧
- 更喜欢靠近错误根源的单元测试。例如，数据模型测试中可能会产生资产数字规格不匹配。
- 集成测试应涵盖代表性案例的端到端映射（例如，重复注册、删除时丢失密钥、无所有权转移）。
- 通过匹配枚举变体而不是消息子字符串来保持断言的弹性。