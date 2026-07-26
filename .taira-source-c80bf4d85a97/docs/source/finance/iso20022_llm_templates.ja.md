---
lang: ja
direction: ltr
source: docs/source/finance/iso20022_llm_templates.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8d4569e75fb219979ee7c5e776427336bc43c155a24210af977e7a8e6ee1c8be
source_last_modified: "2026-01-03T18:08:00.803277+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//!ロードマップ マイルストーン F3 の ISO 20022 明確化プロンプト テンプレート。

# ISO 20022 説明プロンプト テンプレート

これらのテンプレートは、財務決済エンジニアが ISO 20022 ドキュメントや市場構造規範からのクイック リファレンスを必要とする場合に、LLM (または他のアシスタント) へのリクエストをシードするのに役立ちます。常に具体的なペイロード サンプルを含め、作業中の特定のメッセージ ファミリを引用することで、応答が決定的で監査しやすい状態に保たれます。

## 質問する前に
- ISO 20022 メッセージ (`MsgDefId`、バリアント、マーケット プラクティス パック) とバージョンを特定します。
- Norito/ISI コンテキスト: 命令名、レッグ、オプションのパラメータ、予想される実行タイムラインを収集します。
- すでに所有しているアーティファクトに注意してください (スキーマ スニペ​​ット、BR-n 検証ルール、市場慣行メモ)。
- 規制上のガイダンス (CPMI-IOSCO など) または運用基準 (カットオフ ウィンドウ、流動性バッファー) が必要かどうかを明確にします。

## テンプレート 1 – フィールド マッピングとセマンティクス
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

## テンプレート 2 – 決済ワークフローとスケジュールの明確化
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

## テンプレート 3 – 検証ルールとコード セット
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

## テンプレート 4 – 市場構造と規制の背景
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

## テンプレート 5 – 例外処理と調整
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

## 使用上の注意
- 入力済みのプロンプトを課題トラッカーまたは設計ドキュメントに保存して、会話を監査可能な状態に保ちます。
- 外部参照を共有する場合は、公式 ISO ドキュメント ポータルまたは SMPG 抜粋にリンクします。未公開の内容は避けてください。
- 新しい ISO メッセージ ファミリまたは市場慣行が対象範囲に入るたびに、このテンプレート セットを更新します (例: pacs.009、デリバティブ担保)。