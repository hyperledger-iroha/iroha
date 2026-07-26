---
lang: my
direction: ltr
source: docs/source/finance/iso20022_llm_templates.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8d4569e75fb219979ee7c5e776427336bc43c155a24210af977e7a8e6ee1c8be
source_last_modified: "2025-12-29T18:16:35.958788+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! လမ်းပြမြေပုံ Milestone F3 အတွက် ISO 20022 ရှင်းလင်းချက်ပုံစံ နမူနာများ။

# ISO 20022 ရှင်းလင်းချက် အမှာစာ နမူနာများ

ဘဏ္ဍာရေးဖြေရှင်းမှုအင်ဂျင်နီယာများသည် ISO 20022 စာရွက်စာတမ်းများ သို့မဟုတ် စျေးကွက်ဖွဲ့စည်းပုံဆိုင်ရာ စံနှုန်းများထံမှ အမြန်ကိုးကားမှုများ လိုအပ်သည့်အခါ ဤပုံစံများသည် LLM (သို့မဟုတ် အခြားလက်ထောက်များ) သို့ မျိုးစေ့တောင်းဆိုမှုများကို ကူညီပေးပါသည်။ ခိုင်မာသော ပေးဆောင်မှုနမူနာများကို အမြဲထည့်သွင်းပြီး သင်လုပ်ဆောင်နေသည့် တိကျသောမက်ဆေ့ချ်မိသားစုများကို ကိုးကား၍ တုံ့ပြန်မှုများသည် အဆုံးအဖြတ်ရှိပြီး စာရင်းစစ်ဖော်ရွေနေမည်ဖြစ်သည်။

##မမေးခင်
- ISO 20022 မက်ဆေ့ဂျ် (`MsgDefId`၊ မူကွဲ၊ စျေးကွက်လေ့ကျင့်ရေးထုပ်ပိုး) နှင့် ဗားရှင်းကို ခွဲခြားသတ်မှတ်ပါ။
- Norito/ISI အကြောင်းအရာကို စုဆောင်းပါ- ညွှန်ကြားချက်အမည်၊ ခြေထောက်များ၊ ရွေးချယ်ခွင့် ကန့်သတ်ချက်များ၊ မျှော်မှန်းထားသော လုပ်ဆောင်မှုအချိန်ဇယား။
- သင့်တွင်ရှိပြီးသားအရာများကို မှတ်သားပါ (စကမာအတိုအထွာ၊ BR-n အတည်ပြုခြင်းစည်းမျဉ်း၊ စျေးကွက်အလေ့အကျင့်မှတ်စု)။
- စည်းမျဉ်းလမ်းညွှန်ချက် (ဥပမာ၊ CPMI-IOSCO) သို့မဟုတ် လုပ်ငန်းလည်ပတ်မှုဆိုင်ရာ စံနှုန်းများ (ဖြတ်ထားသော windows၊ ငွေဖြစ်လွယ်မှုကြားခံများ) လိုအပ်သည်ဆိုသည်ကို ရှင်းလင်းပါ။

## Template 1 – Field Mapping & Semantics
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

## နမူနာပုံစံ 2 – အလုပ်အသွားအလာကို ဖြေရှင်းခြင်းနှင့် အချိန်လိုင်းရှင်းလင်းချက်များ
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

## Template 3 – အတည်ပြုခြင်း စည်းမျဉ်းများနှင့် ကုဒ်အစုံများ
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

## Template 4 – စျေးကွက်-ဖွဲ့စည်းပုံနှင့် စည်းမျဉ်းစည်းကမ်းဆိုင်ရာ အကြောင်းအရာ
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

## ပုံစံ ၅ – ခြွင်းချက် ကိုင်တွယ်မှုနှင့် ပြန်လည်သင့်မြတ်ရေး
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

## အသုံးပြုမှုမှတ်စုများ
- စကားဝိုင်းများကို စစ်မှန်ကြောင်း သေချာစေရန်အတွက် ပြဿနာခြေရာခံ သို့မဟုတ် ဒီဇိုင်းစာတမ်းများတွင် ဖြည့်သွင်းထားသော အချက်ပြမှုများကို သိမ်းဆည်းပါ။
- ပြင်ပကိုးကားချက်များကိုမျှဝေသည့်အခါ၊ တရားဝင် ISO စာရွက်စာတမ်းပေါ်တယ်များ သို့မဟုတ် SMPG ကောက်နုတ်ချက်များကို ချိတ်ဆက်ပါ။ မထုတ်ဝေရသေးသော အကြောင်းအရာများကို ရှောင်ကြဉ်ပါ။
- ISO မက်ဆေ့ခ်ျမိသားစုအသစ်များ သို့မဟုတ် စျေးကွက်အလေ့အကျင့်များ (ဥပမာ၊ pacs.009၊ ဆင်းသက်လာသော အပေါင်ပစ္စည်း) နယ်ပယ်သို့ ဝင်ရောက်သည့်အခါတိုင်း ဤပုံစံပုံစံကို အပ်ဒိတ်လုပ်ပါ။