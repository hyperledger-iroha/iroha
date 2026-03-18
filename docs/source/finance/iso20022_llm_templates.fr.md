---
lang: fr
direction: ltr
source: docs/source/finance/iso20022_llm_templates.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8d4569e75fb219979ee7c5e776427336bc43c155a24210af977e7a8e6ee1c8be
source_last_modified: "2026-01-03T18:08:00.803277+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! Modèles d'invite de clarification ISO 20022 pour la feuille de route Jalon F3.

# Modèles d'invite de clarification ISO 20022

Ces modèles aident à envoyer des demandes au LLM (ou à d'autres assistants) lorsque les ingénieurs en règlement financier ont besoin de références rapides à partir de la documentation ISO 20022 ou des normes de structure du marché. Incluez toujours des échantillons concrets de charge utile et citez les familles de messages spécifiques sur lesquelles vous travaillez afin que les réponses restent déterministes et faciles à auditer.

## Avant de demander
- Identifier le message ISO 20022 (`MsgDefId`, variante, pack pratiques de marché) et la version.
- Collecter le contexte Norito/ISI : nom de l'instruction, jambes, paramètres optionnels, chronologie d'exécution attendue.
- Notez les artefacts que vous possédez déjà (extrait de schéma, règle de validation BR-n, note sur les pratiques de marché).
- Précisez si vous avez besoin de directives réglementaires (par exemple, CPMI-IOSCO) ou de normes opérationnelles (fenêtres limites, tampons de liquidité).

## Modèle 1 – Cartographie des champs et sémantique
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

## Modèle 2 – Clarifications du flux de travail et du calendrier de règlement
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

## Modèle 3 – Règles de validation et ensembles de codes
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

## Modèle 4 – Structure du marché et contexte réglementaire
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

## Modèle 5 – Gestion des exceptions et rapprochement
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

## Notes d'utilisation
- Stockez les invites remplies dans le suivi des problèmes ou les documents de conception pour garantir que les conversations restent auditables.
- Lors du partage de références externes, créez un lien vers des portails de documentation ISO officiels ou des extraits SMPG ; évitez les documents non publiés.
- Mettez à jour cet ensemble de modèles chaque fois que de nouvelles familles de messages ISO ou pratiques de marché entrent dans le champ d'application (par exemple, pacs.009, garanties sur produits dérivés).