---
lang: hy
direction: ltr
source: docs/source/finance/iso20022_llm_templates.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8d4569e75fb219979ee7c5e776427336bc43c155a24210af977e7a8e6ee1c8be
source_last_modified: "2025-12-29T18:16:35.958788+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! ISO 20022 պարզաբանման հուշումների ձևանմուշներ Milestone F3 ճանապարհային քարտեզի համար:

# ISO 20022 Պարզաբանման հուշման ձևանմուշներ

Այս ձևանմուշները օգնում են LLM-ին (կամ այլ օգնականներին) ներկայացնելու հարցումները, երբ ֆինանսական հաշվարկների ինժեներներին անհրաժեշտ են արագ հղումներ ISO 20022 փաստաթղթերից կամ շուկայի կառուցվածքի նորմերից: Միշտ ներառեք բեռի կոնկրետ նմուշներ և նշեք հատուկ հաղորդագրությունների ընտանիքները, որոնց վրա աշխատում եք, որպեսզի պատասխանները մնան վճռական և աուդիտի համար հարմար:

## Նախքան հարցնելը
- Բացահայտեք ISO 20022 հաղորդագրությունը (`MsgDefId`, տարբերակ, շուկայական պրակտիկայի փաթեթ) և տարբերակը:
- Հավաքեք Norito/ISI համատեքստը՝ հրահանգի անվանումը, ոտքերը, կամընտիր պարամետրերը, կատարման սպասվող ժամանակացույցը:
- Նշեք, թե որ արտեֆակտներն արդեն ունեք (սխեմայի հատված, BR-n վավերացման կանոն, շուկայական պրակտիկայի նշում):
- Հստակեցրեք՝ արդյոք Ձեզ անհրաժեշտ է կարգավորող ուղեցույց (օրինակ՝ CPMI-IOSCO) կամ գործառնական նորմեր (կտրող պատուհաններ, իրացվելիության բուֆերներ):

## Կաղապար 1 – Դաշտի քարտեզագրում և իմաստաբանություն
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

## Կաղապար 2 – Հաշվարկների աշխատանքային հոսք և ժամանակացույցի պարզաբանումներ
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

## Կաղապար 3 – Վավերացման կանոններ և կոդերի հավաքածուներ
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

## Կաղապար 4 – Շուկայի կառուցվածք և կարգավորող համատեքստ
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

## Կաղապար 5 – Բացառությունների մշակում և հաշտեցում
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

## Օգտագործման նշումներ
- Պահպանեք լրացված հուշումները խնդրի հետագծում կամ դիզայնի փաստաթղթերում, որպեսզի խոսակցությունները մնան ստուգելի:
- Արտաքին հղումներ փոխանակելիս, հղում կատարեք ISO փաստաթղթերի պաշտոնական պորտալներին կամ SMPG քաղվածքներին. խուսափել չհրապարակված նյութերից.
- Թարմացրեք այս ձևանմուշների հավաքածուն ամեն անգամ, երբ նոր ISO հաղորդագրությունների ընտանիքները կամ շուկայական պրակտիկան մտնում են շրջանակի մեջ (օրինակ՝ pacs.009, ածանցյալ գործիքների գրավը):