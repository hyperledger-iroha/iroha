---
lang: ru
direction: ltr
source: docs/source/finance/iso20022_llm_templates.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8d4569e75fb219979ee7c5e776427336bc43c155a24210af977e7a8e6ee1c8be
source_last_modified: "2026-01-03T18:08:00.803277+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! Шаблоны пояснений по стандарту ISO 20022 для дорожной карты Milestone F3.

# Шаблоны поясняющих запросов ISO 20022

Эти шаблоны помогают отправлять запросы LLM (или другим помощникам), когда инженерам по финансовым расчетам нужны быстрые ссылки на документацию ISO 20022 или нормы рыночной структуры. Всегда включайте конкретные образцы полезной нагрузки и указывайте конкретные семейства сообщений, над которыми вы работаете, чтобы ответы оставались детерминированными и удобными для аудита.

## Прежде чем спросить
- Определите сообщение ISO 20022 (`MsgDefId`, вариант, пакет рыночной практики) и версию.
- Соберите контекст Norito/ISI: имя инструкции, этапы, дополнительные параметры, ожидаемый график выполнения.
- Обратите внимание, какие артефакты у вас уже есть (фрагмент схемы, правило проверки BR-n, примечание по рыночной практике).
- Уточните, нужны ли вам нормативные рекомендации (например, CPMI-IOSCO) или операционные нормы (окна отсечения, буферы ликвидности).

## Шаблон 1 – Сопоставление полей и семантика
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

## Шаблон 2 – Пояснения к рабочему процессу урегулирования и срокам
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

## Шаблон 3 – Правила проверки и наборы кодов
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

## Шаблон 4 – Структура рынка и нормативно-правовая база
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

## Шаблон 5. Обработка исключений и согласование
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

## Примечания по использованию
- Сохраняйте заполненные подсказки в системе отслеживания проблем или проектной документации, чтобы обеспечить возможность проверки разговоров.
- При обмене внешними ссылками используйте ссылки на официальные порталы документации ISO или выдержки из SMPG; избегайте неопубликованных материалов.
- Обновляйте этот набор шаблонов всякий раз, когда в сферу действия попадают новые семейства сообщений ISO или рыночные практики (например, pacs.009, обеспечение деривативов).