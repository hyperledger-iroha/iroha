---
lang: ur
direction: rtl
source: docs/fraud_playbook.md
status: complete
translator: manual
source_hash: b3253ff47a513529c1dba6ef44faf38087ee0e5f5520f8c3fd770ab8d36c7786
source_last_modified: "2025-11-02T04:40:28.812006+00:00"
translation_last_reviewed: 2025-11-14
---

<div dir="rtl">

<!-- docs/fraud_playbook.md (Fraud Governance Playbook) کا اردو ترجمہ -->

# فراڈ گورننس پلے بک

یہ دستاویز PSP فراڈ اسٹیک کے لیے درکار بنیادی انفراسٹرکچر کو خلاصہ کرتی
ہے، اس وقت جب مکمل microservices اور SDKs ابھی فعال development میں ہیں۔ یہ
analytics، آڈیٹر کے ورک فلو، اور fallback اقدامات کے بارے میں توقعات
واضح کرتی ہے، تاکہ آنے والی implementations لیجر کے ساتھ محفوظ انداز میں
جڑ سکیں۔

## سروسز کا جائزہ

1. **API گیٹ وے** – متزامن `RiskQuery` payloads وصول کرتا ہے، انہیں feature
   aggregation سروس کو فارورڈ کرتا ہے، اور `FraudAssessment` جوابات کو
   لیجر کے فلو میں واپس بھیجتا ہے۔ high‑availability (active‑active)
   ضروری ہے؛ request skew سے بچنے کے لیے regional pairs کے ساتھ
   deterministic hashing استعمال کریں۔
2. **Feature Aggregation** – scoring کے لیے feature vectors تیار کرتی ہے۔
   صرف `FeatureInput` کے hashes publish کیے جاتے ہیں؛ حساس payloads
   off‑chain رہتے ہیں۔ observability کو latency histograms، queue depth
   gauges، اور replay counters فی tenant publish کرنے چاہییں۔
3. **Risk Engine** – rules/models evaluate کرتا ہے اور
   `FraudAssessment` کی deterministic آؤٹ پٹس پیدا کرتا ہے۔ rules کی
   execution ordering کو مستحکم رکھیں اور ہر assessment ID کے لیے audit
   logs محفوظ کریں۔

## اینالیٹکس اور ماڈل پروموشن

- **Anomaly Detection**: ایک streaming job برقرار رکھیں جو فی tenant
  decision rates میں ہونے والی deviations کو فلیگ کرے۔ alerts کو governance
  dashboard تک پہنچائیں اور خلاصے کو سہ ماہی (quarterly) ریویوز کے لیے
  محفوظ کریں۔
- **Graph Analysis**: relational exports پر nightly گراف traversal چلائیں
  تاکہ collusion clusters کی شناخت ہو سکے۔ نتائج کو
  `GovernanceExport` کے ذریعے governance portal میں export کریں، ساتھ میں
  supporting evidence کا حوالہ دیں۔
- **Feedback Ingestion**: دستی review کے نتائج اور chargeback reports کو
  curate کریں۔ انہیں feature deltas میں تبدیل کر کے training datasets کا حصہ
  بنائیں۔ ingestion status metrics publish کریں، تاکہ risk ٹیم stalled feeds
  کو جلدی سے پہچان سکے۔
- **Model Promotion Pipeline**: candidate models کا evaluation automate کریں
  (offline metrics، canary scoring، rollback readiness وغیرہ)۔ ہر promotion
  کو `FraudAssessment` sample set کے دستخط شدہ (signed) سیٹ اور
  `GovernanceExport` میں `model_version` فیلڈ کی اپ ڈیٹ کے ساتھ آنا چاہیے۔

## آڈیٹر کا ورک فلو

1. حالیہ ترین `GovernanceExport` کا snapshot لیں اور verify کریں کہ
   `policy_digest`، risk ٹیم کے فراہم کردہ manifest کے ساتھ میچ کرتا ہے۔
2. validate کریں کہ rule aggregates، sample window کے دوران لیجر سائیڈ
   decision totals کے ساتھ reconcile ہو رہے ہیں۔
3. anomaly detection اور graph analysis reports کا جائزہ لیں اور outstanding
   issues کی نشاندہی کریں۔ escalations اور متوقع remediation owners کو
   documented رکھیں۔
4. review checklist پر دستخط کریں اور اسے archive میں رکھیں۔ Norito‑encoded
   artefacts کو governance portal میں محفوظ کریں، تاکہ بعد میں proofs کے
   لیے انہیں reproduce کیا جا سکے۔

## fallback playbooks

- **Engine Outage**: اگر risk engine 60 سیکنڈ سے زیادہ کے لیے
  unavailable ہو، تو gateway کو review‑only mode میں جانا چاہیے، ہر
  request کے لیے `AssessmentDecision::Review` جاری کرتے ہوئے operators کو
  alert کرے۔
- **Telemetry Gap**: جب metrics یا traces پیچھے رہ جائیں (مثلاً 5 منٹ تک
  کوئی data نہ ہو)، تو automatic model promotions روک دیں اور on‑call
  engineer کو notify کریں۔
- **Model Regression**: اگر post‑deployment feedback سے fraud losses میں
  اضافہ ظاہر ہو، تو پچھلے signed model bundle پر rollback کریں، اور
  roadmap میں corrective actions شامل کریں۔

## Data‑Sharing Agreements

- retention، encryption اور breach notification SLAs سے متعلق territory‑specific
  appendices برقرار رکھیں۔ `FraudAssessment` exports حاصل کرنے سے پہلے
  پارٹنرز کو متعلقہ appendix پر دستخط کرنا ضروری ہے۔
- ہر integration کے لیے data minimization practices documented ہونی
  چاہییں (مثال کے طور پر اکاؤنٹ identifiers کو hash کرنا، card numbers کو
  truncate کرنا وغیرہ)۔
- agreements کو سالانہ یا جب بھی regulatory requirements تبدیل ہوں،
  تازہ کریں۔

## Red‑Team Exercises

- exercises ہر سہ ماہی چلائے جاتے ہیں۔ اگلا سیشن
  **2026‑01‑15** کے لیے شیڈیول ہے، جس میں feature poisoning، replay
  amplification اور signature forgery جیسی کوششوں کے سیناریوز شامل ہوں گے۔
- findings کو fraud threat model میں capture کریں اور نتیجے میں بننے والی
  tasks کو `roadmap.md` میں "Fraud & Telemetry Governance Loop" workstream
  کے تحت شامل کریں۔

## API Schemas

gateway اب ایسے ٹھوس JSON envelopes ایکسپورٹ کرتا ہے جو
`crates/iroha_data_model::fraud` میں implement کیے گئے Norito types کے ساتھ
one‑to‑one map ہوتے ہیں:

- **Risk intake** – `POST /v1/fraud/query` اسکیمہ `RiskQuery` کو accept
  کرتا ہے:
  - `query_id` (`[u8; 32]`, hex encoded)
  - `subject` (`AccountId`, `domainless encoded literal; canonical I105 only (non-canonical I105 literals rejected)`)
  - `operation` (tagged enum جو `RiskOperation` سے match کرتا ہے؛ JSON
    فیلڈ `type` enum variant کو reflect کرتا ہے)
  - `related_asset` (`AssetId`, optional)
  - `features` ( `{ key: String, value_hash: hex32 }` objects پر مشتمل
    array، جو `FeatureInput` سے derive کیے جاتے ہیں)
  - `issued_at_ms` (`u64`)
  - `context` (`RiskContext`; جس میں `tenant_id`، optional `session_id`،
    اور optional `reason` شامل ہوتے ہیں)
- **Risk decision** – `POST /v1/fraud/assessment`، `FraudAssessment`
  payload consume کرتا ہے (جو governance exports میں بھی reflect ہوتا ہے):
  - `query_id`, `engine_id`, `risk_score_bps`, `confidence_bps`,
    `decision` (`AssessmentDecision` enum)، `rule_outcomes`
    ( `{ rule_id, score_delta_bps, rationale? }` objects پر مشتمل array)
  - `generated_at_ms`
  - `signature` (base64‑encoded optional فیلڈ جو Norito‑encoded
    `FraudAssessment` کو wrap کرتا ہے)
- **Governance export** – جب `governance` feature enabled ہو، تو
  `GET /v1/fraud/governance/export`، `GovernanceExport` structure واپس
  کرتا ہے، جس میں active parameters، تازہ ترین enactment، model version،
  policy digest، اور `DecisionAggregate` histogram شامل ہوتے ہیں۔

`crates/iroha_data_model/src/fraud/types.rs` میں موجود round‑trip tests:
یہ ensure کرتے ہیں کہ یہ schemas، Norito codec کے binary format کے مطابق
رہیں، اور `integration_tests/tests/fraud_monitoring_requires_assessment_bands.rs`
end‑to‑end پورے intake/decision pipeline کو exercise کرتا ہے۔

## PSP SDK ریفرنسز

ذیل میں دی گئی زبانوں کے stubs PSP‑side integration examples کو track کرتے
ہیں:

- **Rust** – `integration_tests/tests/fraud_monitoring_requires_assessment_bands.rs`
  workspace کے `iroha` client کو استعمال کرتا ہے، `RiskQuery` metadata
  بنانے اور admission کی کامیابی/ناکامی validate کرنے کے لیے۔
- **TypeScript** – `docs/source/governance_api.md` وہ REST surface
  document کرتا ہے جسے PSP demo dashboard میں استعمال ہونے والا
  lightweight Torii gateway consume کرتا ہے؛ scripted client
  `scripts/ci/schedule_fraud_scoring.sh` میں موجود ہے اور smoke drills کے
  لیے استعمال ہوتا ہے۔
- **Swift اور Kotlin** – موجودہ SDKs (`IrohaSwift` اور
  `crates/iroha_cli/docs/multisig.md` میں موجود حوالہ جات) وہ Torii
  metadata hooks expose کرتے ہیں جو `fraud_assessment_*` فیلڈز attach کرنے
  کے لیے درکار ہیں۔ PSP‑specific helpers کو `status.md` کے milestone
  "Fraud & Telemetry Governance Loop" کے تحت track کیا جاتا ہے اور وہ
  انہی SDKs کے transaction builders کو reuse کرتے ہیں۔

یہ ریفرنسز microservice‑based gateway کے ساتھ sync رہیں گی، تاکہ PSP
implementers کے پاس ہر سپورٹڈ زبان کے لیے ہمہ وقت تازہ ترین schema اور
نمونہ code path موجود رہے۔

</div>
