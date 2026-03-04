---
lang: mn
direction: ltr
source: docs/source/finance/iso20022_llm_templates.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8d4569e75fb219979ee7c5e776427336bc43c155a24210af977e7a8e6ee1c8be
source_last_modified: "2025-12-29T18:16:35.958788+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! Milestone F3 замын газрын зургийн ISO 20022-ын тодруулга хийх загварууд.

# ISO 20022 Тодруулга зааварчилгааны загварууд

Эдгээр загварууд нь санхүүгийн тооцооны инженерүүдэд ISO 20022 баримт бичиг эсвэл зах зээлийн бүтцийн хэм хэмжээнээс хурдан лавлагаа авах шаардлагатай үед LLM (эсвэл бусад туслахууд) -д өгөх хүсэлт гаргахад тусалдаг. Үргэлж тодорхой ачааны дээжийг оруулж, өөрийн ажиллаж буй мессежийн бүлгүүдийг иш татаарай, ингэснээр хариултууд нь тодорхой бөгөөд аудитад ээлтэй хэвээр байх болно.

## асуухаасаа өмнө
- ISO 20022 мессеж (`MsgDefId`, хувилбар, зах зээлийн практик багц) болон хувилбарыг тодорхойлох.
- Norito/ISI контекстийг цуглуул: зааврын нэр, хөл, нэмэлт параметрүүд, хүлээгдэж буй гүйцэтгэлийн цаг.
- Танд аль олдвор байгаа болохыг анхаарна уу (схемийн хэсэг, BR-n баталгаажуулалтын дүрэм, зах зээлийн практик тэмдэглэл).
- Танд зохицуулалтын заавар (жишээ нь, CPMI-IOSCO) эсвэл үйл ажиллагааны хэм хэмжээ (таслах цонх, хөрвөх чадварын буфер) хэрэгтэй эсэхийг тодруулна уу.

## Загвар 1 – Талбайн зураглал ба семантик
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

## Загвар 2 – Төлбөр тооцооны ажлын урсгал ба цагийн хуваарийн тодруулга
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

## Загвар 3 – Баталгаажуулах дүрэм, кодын багц
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

## Загвар 4 – Зах зээлийн бүтэц, зохицуулалтын агуулга
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

## Загвар 5 – Онцгой тохиолдлуудыг зохицуулах, нэгтгэх
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

## Хэрэглээний тэмдэглэл
- Харилцан яриаг шалгах боломжтой байлгахын тулд бөглөсөн сануулгыг асуудал хянагч эсвэл дизайны баримт бичигт хадгална уу.
- Гадны лавлагааг хуваалцахдаа албан ёсны ISO баримт бичгийн порталууд эсвэл SMPG-ийн хандалт руу холбох; хэвлэгдээгүй материалаас зайлсхий.
- Шинэ ISO мессежийн бүлгүүд эсвэл зах зээлийн практикууд (жишээ нь, pacs.009, дериватив барьцаа) орох бүрд энэ багц загварыг шинэчил.