---
lang: pt
direction: ltr
source: docs/source/finance/iso20022_llm_templates.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8d4569e75fb219979ee7c5e776427336bc43c155a24210af977e7a8e6ee1c8be
source_last_modified: "2026-01-03T18:08:00.803277+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! Modelos de prompt de esclarecimento ISO 20022 para roteiro Marco F3.

# Modelos de prompt de esclarecimento ISO 20022

Esses modelos ajudam a enviar solicitações ao LLM (ou outros assistentes) quando os engenheiros de liquidação financeira precisam de referências rápidas da documentação ISO 20022 ou das normas de estrutura de mercado. Sempre inclua amostras concretas de carga útil e cite as famílias de mensagens específicas nas quais você está trabalhando para que as respostas permaneçam determinísticas e fáceis de auditar.

## Antes de perguntar
- Identificar a mensagem ISO 20022 (`MsgDefId`, variante, pacote de práticas de mercado) e versão.
- Colete o contexto Norito/ISI: nome da instrução, pernas, parâmetros opcionais, cronograma de execução esperado.
- Anote quais artefatos você já possui (snippet de esquema, regra de validação BR-n, nota de prática de mercado).
- Esclareça se você precisa de orientação regulatória (por exemplo, CPMI-IOSCO) ou normas operacionais (janelas de corte, buffers de liquidez).

## Modelo 1 – Mapeamento de campos e semântica
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

## Modelo 2 – Fluxo de trabalho de liquidação e esclarecimentos sobre cronograma
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

## Modelo 3 – Regras de validação e conjuntos de códigos
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

## Modelo 4 – Estrutura de Mercado e Contexto Regulatório
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

## Modelo 5 – Tratamento e reconciliação de exceções
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

## Notas de uso
- Armazene os prompts preenchidos no rastreador de problemas ou nos documentos de design para garantir que as conversas permaneçam auditáveis.
- Ao compartilhar referências externas, linkar para portais oficiais de documentação ISO ou extratos SMPG; evite material inédito.
- Atualizar este conjunto de modelos sempre que novas famílias de mensagens ISO ou práticas de mercado entrarem no escopo (por exemplo, pacs.009, garantia de derivativos).