---
lang: zh-hans
direction: ltr
source: docs/source/finance/iso20022_llm_templates.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8d4569e75fb219979ee7c5e776427336bc43c155a24210af977e7a8e6ee1c8be
source_last_modified: "2025-12-29T18:16:35.958788+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//！路线图 Milestone F3 的 ISO 20022 澄清提示模板。

# ISO 20022 澄清提示模板

当财务结算工程师需要 ISO 20022 文档或市场结构规范的快速参考时，这些模板可帮助向 LLM（或其他助理）提出请求。始终包含具体的有效负载示例并引用您正在处理的特定消息系列，以便响应保持确定性和易于审计。

## 在你提问之前
- 识别 ISO 20022 消息（`MsgDefId`、变体、市场实践包）和版本。
- 收集 Norito/ISI 上下文：指令名称、分支、可选参数、预期执行时间线。
- 注意您已经拥有哪些工件（模式片段、BR-n 验证规则、市场实践说明）。
- 明确是否需要监管指导（例如 CPMI-IOSCO）或操作规范（截止窗口、流动性缓冲）。

## 模板 1 – 字段映射和语义
```text
You are LLM acting as an ISO 20022 integration analyst.
Goal: map {{Norito_instruction_or_field}} in Hyperledger Iroha to ISO 20022 {{message_id}} (version {{version}}).

Context:
- Current Norito payload snippet: {{json_or_toml_fragment}}
- Relevant ISO elements identified so far: {{list_of_iso_paths}}
- Constraints: {{determinism_rules_or_validation}}

Questions:
1. Which ISO element best represents this field and why?
2. What business rules (BR-n) or implementation guidelines govern it?
3. Are there market practice caveats (e.g., HVPS+, CBPR+) we must observe?
Return the answer as a table with `Field`, `ISO Path`, `Rules`, `Notes`.
```

## 模板 2 – 结算工作流程和时间表说明
```text
You are LLM advising on DvP/PvP settlement workflows referencing ISO 20022 repo/payments messages.
Scenario: {{brief_repo_or_pvp_scenario}}, including legs, currencies, and counterparties.

Information available:
- Initiating ISO message: {{message_id_version}}
- Follow-up/status messages: {{status_messages}}
- Norito workflow stages: {{stage_list}}

Questions:
1. Outline the canonical ISO 20022 message sequence covering initiation → matching → settlement → confirmation.
2. Highlight required time windows, cut-offs, or regulatory checkpoints (e.g., CLS, TARGET2).
3. Call out any conditional reversals or hold/release states we must model.
Deliver the answer as bullet lists grouped by phase, citing ISO references (e.g., sese.023 BR23).
```

## 模板 3 – 验证规则和代码集
```text
You are LLM validating ISO 20022 message content.
Message: {{message_id_version}} for {{business_process}}.
Payload sample: {{xml_or_json_fragment}}

Tasks:
1. List mandatory elements and applicable BR-n rules that apply to the provided fields.
2. For code-valued fields (e.g., `PmtMtd`, `SttlmPties`), enumerate allowed values and link to the official code set.
3. Flag structural or semantic validations we must enforce in Norito (IBAN length, BIC format, currency decimals).
Produce the result as a checklist with `Requirement`, `Reference`, `Implementation Hint`.
```

## 模板 4 – 市场结构和监管环境
```text
You are LLM providing market-structure references for ISO 20022 settlement.
Region/Market: {{market_or_region}}
Use case: {{repo_dvp_pvp_or_other}}

Need:
1. Summarise applicable market practice documents (e.g., SMPG, CLS) that influence message usage.
2. Identify operational constraints (cut-off times, partial settlement policies, real-time gross vs batch).
3. Highlight compliance considerations (reporting, audit trails, data retention) tied to ISO message exchanges.
Return concise paragraphs with citations and note where operator documentation should reference these rules.
```

## 模板 5 – 异常处理和协调
```text
You are LLM focusing on exception handling for ISO 20022-driven settlements.
Scenario: {{failure_case}} (e.g., counterparty fails cash leg).
Messages involved: {{initiating_msg}}, {{status_msg}}, optional {{cancel_msg}}

Questions:
1. Which ISO status/reason codes signal this failure and what follow-up messages are expected?
2. How should reconciliations be documented (statement messages, audit trail references)?
3. What deterministic retries or rollback steps should the settlement engine expose?
Respond with a step-by-step outline and map each action to ISO codes.
```

## 使用说明
- 将填写的提示存储在问题跟踪器或设计文档中，以确保对话保持可审核性。
- 共享外部参考时，链接到官方 ISO 文档门户或 SMPG 摘录；避免未发表的材料。
- 每当新的 ISO 消息系列或市场惯例进入范围（例如 pacs.009、衍生品抵押品）时，更新此模板集。