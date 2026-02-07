---
lang: ur
direction: rtl
source: docs/source/finance/iso20022_llm_templates.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8d4569e75fb219979ee7c5e776427336bc43c155a24210af977e7a8e6ee1c8be
source_last_modified: "2026-01-03T18:08:00.803277+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! آئی ایس او 20022 روڈ میپ سنگ میل ایف 3 کے لئے فوری طور پر ٹیمپلیٹس۔

# ISO 20022 وضاحت فوری ٹیمپلیٹس

یہ ٹیمپلیٹس ایل ایل ایم (یا دوسرے معاونین) میں بیج کی درخواستوں میں مدد کرتے ہیں جب مالی آبادکاری کے انجینئروں کو آئی ایس او 20022 کے دستاویزات یا مارکیٹ کے ڈھانچے کے اصولوں سے فوری حوالوں کی ضرورت ہوتی ہے۔ کنکریٹ پے لوڈ کے نمونے ہمیشہ شامل کریں اور مخصوص پیغام کے کنبوں کا حوالہ دیں جن پر آپ کام کر رہے ہیں لہذا ردعمل متضاد اور آڈٹ دوستانہ رہیں۔

## پوچھنے سے پہلے
- آئی ایس او 20022 پیغام (`MsgDefId` ، مختلف ، مارکیٹ پریکٹس پیک) اور ورژن کی شناخت کریں۔
- Norito/ISI سیاق و سباق کو جمع کریں: ہدایات کا نام ، ٹانگیں ، اختیاری پیرامیٹرز ، متوقع عمل کی ٹائم لائن۔
- نوٹ کریں کہ آپ کے پاس کون سے نوادرات ہیں (اسکیما کا ٹکڑا ، بی آر-این توثیق کا قاعدہ ، مارکیٹ پریکٹس نوٹ)۔
-واضح کریں کہ آیا آپ کو ریگولیٹری رہنمائی (جیسے ، سی پی ایم آئی-آئوسکو) یا آپریشنل اصولوں (کٹ آف ونڈوز ، لیکویڈیٹی بفرز) کی ضرورت ہے یا نہیں۔

## ٹیمپلیٹ 1 - فیلڈ میپنگ اور سیمنٹکس
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

## ٹیمپلیٹ 2 - تصفیہ ورک فلو اور ٹائم لائن وضاحت
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

## ٹیمپلیٹ 3 - توثیق کے قواعد اور کوڈ سیٹ
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

## ٹیمپلیٹ 4-مارکیٹ ڈھانچہ اور ریگولیٹری سیاق و سباق
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

## ٹیمپلیٹ 5 - استثناء ہینڈلنگ اور مفاہمت
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

## استعمال نوٹس
- بات چیت کو قابل سماعت رہنے کو یقینی بنانے کے لئے ایشو ٹریکر یا ڈیزائن دستاویزات میں بھرے ہوئے اشارے پر اسٹور کریں۔
- جب بیرونی حوالوں کا اشتراک کرتے ہو تو ، سرکاری آئی ایس او دستاویزات کے پورٹلز یا ایس ایم پی جی کے نچوڑ سے لنک کریں۔ غیر مطبوعہ مواد سے پرہیز کریں۔
- جب بھی نئے آئی ایس او میسج فیملیز یا مارکیٹ کے طریق کار دائرہ کار میں داخل ہوتے ہیں تو اس ٹیمپلیٹ سیٹ کو اپ ڈیٹ کریں (جیسے ، PACS.009 ، مشتق کولیٹرل)۔