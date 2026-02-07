---
lang: ba
direction: ltr
source: docs/source/finance/iso20022_llm_templates.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8d4569e75fb219979ee7c5e776427336bc43c155a24210af977e7a8e6ee1c8be
source_last_modified: "2025-12-29T18:16:35.958788+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! ISO 20022 асыҡлыҡ индереү өндәү ҡалыптары өсөн юл картаһы Milestone F3.

# ISO 20022 асыҡлыҡ индереү ҡалыбы

Был ҡалыптар ярҙам орлоҡ запростары LLM (йәки башҡа ярҙамсылары) ҡасан финанс-төшөү инженерҙары тиҙ һылтанмалар кәрәк ISO 20022 документация йәки баҙар-структура нормалары. Һәр ваҡыт бетон файҙалы йөк өлгөләрен үҙ эсенә ала һәм аныҡ хәбәр ғаиләләрен цитироваться һеҙ эшләйһегеҙ, шуға күрә яуаптар детерминистик һәм аудит-дуҫ ҡала.

## Һеҙҙән алда .
- ISO 20022 хәбәрен билдәләү (`MsgDefId`, вариант, баҙар практикаһы пакеты) һәм версия.
- Norito/ISI контексын йыйып: инструкция исеме, аяҡтары, өҫтәмә параметрҙар, көтөлгән башҡарыу ваҡыты һыҙығы.
- Иғтибар итегеҙ, ниндәй артефакттар һеҙгә инде (схема өҙөк, BR-n раҫлау ҡағиҙәһе, баҙар практикаһы иҫкәрмәһе).
- асыҡларға кәрәкме, һеҙгә кәрәкме, көйләү йүнәлеше (мәҫәлән, CPMI-IOSCO) йәки оператив нормалар (ҡырҡылған тәҙрәләр, ликвидлыҡ буферҙары).

## 1-се ҡалып – Ялан картаһы һәм семантикаһы
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

## 2-се ҡалып – ҡасаба эш ағымы & Ваҡыт һыҙығы асыҡлыҡтары
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

## 3-сө ҡалып – раҫлау ҡағиҙәләре & Код комплекттары
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

## ҡалып 4 – Баҙар-структура & Регулятор контексы
```text
You are LLM providing market-structure references for ISO 20022 settlement.
Region/Market: {{market_or_region}}
Use case: {{repo_dvp_pvp_or_other}}

Need:
1. Summarise applicable market practice documents (e.g., SMPG, CLS) that influence message usage.
2. Identify operational constraints (cut-off times, partial settlement policies, real-time gross vs batch).
3. Highlight compliance considerations (reporting, audit trails, data retention) tied to ISO message exchanges.
Return concise paragraphs with citations and note where operator documentation should reference these rules.
``` X

## 5-се ҡалып – Исключение менән эш итеү һәм яраштырыу
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

## Ҡулланыу иҫкәрмәләре
- Магазин тултырылған өндәүҙәр мәсьәлә трекер йәки проектлау docs тәьмин итеү өсөн һөйләшеүҙәр аудит ҡала.
- Тышҡы һылтанмалар менән бүлешкәндә, рәсми ISO документацияһы порталдарына йәки SMPG экстракцияларына һылтанма; баҫылмаған материалдан ҡасығыҙ.
- Яңыртыу был шаблон йыйылмаһы ҡасан яңы ISO хәбәр ғаиләләр йәки баҙар практикаһы масштабына инә (мәҫәлән, pacs.009, сығарылыштар коллатераль).