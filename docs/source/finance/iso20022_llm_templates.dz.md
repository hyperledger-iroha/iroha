---
lang: dz
direction: ltr
source: docs/source/finance/iso20022_llm_templates.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8d4569e75fb219979ee7c5e776427336bc43c155a24210af977e7a8e6ee1c8be
source_last_modified: "2025-12-29T18:16:35.958788+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

///! ISO 20022 བརྡ་འཕྲིན་བརྡ་འཕྲིན་ཊེམ་པེལེཊི་ཚུ་ ལམ་སྟོན་པ་ མའིལ་སི་ཊོན་ཨེཕ་༣.

# ISO 20022 གསལ་བཤད།

དངུལ་འབྲེལ་གཞི་བཅག་པའི་བཟོ་རིག་པ་ཚུ་ལུ་ ISO 20022 ཡིག་ཆ་ ཡང་ན་ ཁྲོམ་གྱི་བཟོ་བཀོད་ལམ་ལུགས་ཚུ་ལས་ མགྱོགས་དྲགས་སྦེ་ གཞི་བསྟུན་འབད་དགོཔ་ཐོན་པའི་སྐབས་ ཊེམ་པེལེཊ་འདི་ཚུ་གིས་ ཨེལ་ཨེལ་ཨེམ་ལུ་ ཞུ་བ་ཚུ་ ཕན་ཐོགཔ་ཨིན། དུས་རྒྱུན་དུ་ བརྟན་བཞུགས་ཀྱི་ པེ་ལེསི་དཔེ་ཚད་ཚུ་ ཚུད་དེ་ ཁྱོད་ཀྱིས་ ལཱ་འབད་བའི་ བརྡ་འཕྲིན་བཟའ་ཚང་ཚུ་ བཀོད་དགོཔ་ལས་ ལན་ཚུ་ གཏན་འབེབས་དང་ རྩིས་ཞིབ་མཐུན་སྒྲིག་སྦེ་ ལུསཔ་ཨིན།

## མ་འདྲི་བའི་སྔོན་ལ།
- ISO 20022 འཕྲིན་དོན་ (`MsgDefId`, འགྱུར་ལྡོག་ཅན་, ཁྲོམ་རའི་སྦྱོང་ལཱ་གི་ཐུམ་སྒྲིལ་) དང་ཐོན་རིམ་ངོས་འཛིན་འབད།
- Norito/ISI སྐབས་དོན་: བཀོད་རྒྱའི་མིང་ རྐངམ་ གདམ་ཁའི་ཚད་བཟུང་ཚུ་ རེ་བ་བསྐྱེད་པའི་ལག་ལེན་དུས་ཚོད་གྲལ་ཐིག་ཚུ་བསྡུ་ལེན་འབད།
- ཁྱོད་ལུ་ཧེ་མ་ལས་རང་ ཅ་རྙིང་ག་ཅི་ཡོདཔ་ཨིན་ན་ དྲན་འཛིན་འབད། (འཆར་གཞི་ snippet, BR-n བདེན་དཔྱད་ལམ་ལུགས་, ཁྲོམ་རའི་ལག་ལེན་དྲན་ཐོ།)
- ཁྱོད་ལུ་ ཁྲིམས་ལུགས་ཀྱི་ལམ་སྟོན་དགོཔ་ཨིན་ན་ (དཔེར་ན་ CPMI-IOSCO) ཡང་ན་ བཀོལ་སྤྱོད་ཀྱི་ལམ་ལུགས་ (བཏོག་བཏང་ནིའི་སྒོ་སྒྲིག་དང་ ཆུ་ཡི་བཱ་ཕར་ཚུ་) གསལ་ཏོག་ཏོ་བཟོ་དགོ།

## གཞི་རྩ། ༡ – ས་སྒོའི་སབ་ཁྲ་དང་ཡིག་བརྡ།
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

## གཞི་རིམ་༢ པ་ - གཞིས་ཆགས་ལཱ་གི་རྒྱུན་རིམ་དང་དུས་ཚོད་ཀྱི་གསལ་བསྒྲགས།
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

## ཊེམ་པེལེཊི་ ༣ – བདེན་དཔྱད་ལམ་ལུགས་དང་ཨང་རྟགས་ཆ་ཚན་ཚུ།
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

## ཊེམ་པེལེཊ་༤ – ཁྲོམ་ར་བཟོ་བཀོད་དང་ སྒྲིག་གཞི་གི་སྐབས་དོན།
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

## ཊེམ་པེལེཊ་ ༥ – ཁྱད་པར་འཛིན་སྐྱོང་དང་མཐུན་སྒྲིག།
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

## ལག་བསྟར་དྲན་ཐོ།
- བློ་སླབ་ཚུ་ རྩིས་ཞིབ་འབད་བཏུབ་སྦེ་བཞག་ནི་ལུ་ གནད་དོན་འཚོལ་ཞིབ་འབད་མི་ ཡང་ན་ བཟོ་བཀོད་ཀྱི་ ཡིག་ཆ་ཚུ་ནང་ བཀང་ཆ་ཚུ་ གསོག་འཇོག་འབད།
- ཕྱིའི་གཞི་བསྟུན་ཚུ་བརྗེ་སོར་འབད་བའི་སྐབས་ གཞུང་འབྲེལ་ཨའི་ཨེསི་ཨོ་ཡིག་ཆ་གི་དྲྭ་ཚིགས་ ཡང་ན་ ཨེསི་ཨེམ་པི་ཇི་ ཕྱིར་འཐེན་ཚུ་ལུ་འབྲེལ་མཐུད་འབད་ནི། དཔར་བསྐྲུན་མ་འབད་བའི་རྒྱུ་ཆ་སྤང་དགོ།
- ཨའི་ཨེསི་ཨོ་བརྡ་འཕྲིན་བཟའ་ཚང་གསརཔ་ ཡང་ན་ ཁྲོམ་རའི་ལག་ལེན་ཚུ་ ཁྱབ་ཚད་ཐོ་བཀོད་འབད་བའི་སྐབས་ གཞི་སྒྲིག་འབད་ ཊེམ་པེལེཊི་འདི་ གཞི་སྒྲིག་འབད།