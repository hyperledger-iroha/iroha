---
lang: ur
direction: rtl
source: docs/source/nexus_compliance.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5635794e962a9fb1b94c5ff550dc198a744a64b4f9f05df588cb70621e9237f9
source_last_modified: "2025-11-21T18:31:26.542844+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- اردو ترجمہ برائے docs/source/nexus_compliance.md -->

# Nexus Lane Compliance اور وائٹ لسٹ پالیسی انجن (NX-12)

اسٹیٹس: 🈴 نافذ — یہ دستاویز فعال پالیسی ماڈل اور کنسینسس کے لئے اہم انفورسمنٹ بیان کرتی ہے جو روڈمیپ آئٹم
**NX-12 — Lane compliance اور whitelist policy engine** سے جڑی ہے۔ یہ ڈیٹا ماڈل، گورننس فلو، ٹیلی میٹری،
اور رول آؤٹ حکمت عملی کو بیان کرتی ہے جو `crates/iroha_core/src/compliance` میں نافذ ہو چکی ہے اور
Torii admission کے ساتھ `iroha_core` ٹرانزیکشن ویلیڈیشن میں بھی لاگو ہوتی ہے، تاکہ ہر lane اور dataspace
کو ڈیٹرمنسٹک جرِسڈکشنل پالیسیوں سے باندھا جا سکے۔

## اہداف

- گورننس کو allow/deny قواعد، جرِسڈکشن فلیگز، CBDC ٹرانسفر حدود، اور آڈٹ تقاضے ہر lane manifest کے ساتھ جوڑنے دینا۔
- Torii admission اور بلاک ایگزیکیوشن کے دوران ہر ٹرانزیکشن کو ان قواعد کے خلاف جانچنا، تاکہ نوڈز میں
  ڈیٹرمنسٹک پالیسی انفیورسمنٹ یقینی ہو۔
- Norito evidence bundles اور ریگولیٹرز/آپریٹرز کے لئے قابلِ تلاش ٹیلی میٹری کے ساتھ کرپٹوگرافک طور پر قابلِ تصدیق آڈٹ ٹریل تیار کرنا۔
- ماڈل کو لچکدار رکھنا: وہی policy engine نجی CBDC lanes، عوامی settlement DS، اور ہائبرڈ پارٹنر dataspaces کو
  bespoke forks کے بغیر کور کرے۔

## غیر اہداف

- AML/KYC طریقہ کار یا قانونی اسکیلیشن ورک فلو کی تعریف۔ یہ کمپلائنس پلی بکس میں ہیں جو یہاں تیار ہونے والی ٹیلی میٹری استعمال کرتے ہیں۔
- IVM میں per-instruction ٹوگلز متعارف کرانا؛ انجن صرف یہ کنٹرول کرتا ہے کہ کون سے accounts/assets/domains ٹرانزیکشن بھیج سکتے ہیں
  یا lane کے ساتھ تعامل کر سکتے ہیں۔
- Space Directory کو غیر متعلق بنانا۔ manifests DS میٹاڈیٹا کا اتھارٹی ماخذ رہتے ہیں؛ کمپلائنس پالیسی صرف Space Directory
  انٹریز کو ریفرنس کرتی ہے اور انہیں مکمل کرتی ہے۔

## پالیسی ماڈل

### ادارے اور شناخت کنندگان

پالیسی انجن درج ذیل پر کام کرتا ہے:

- `LaneId` / `DataSpaceId` — اس دائرہ کار کی شناخت جس میں قواعد لاگو ہوتے ہیں۔
- `UniversalAccountId (UAID)` — cross-lane شناختوں کو گروپ کرنے کی اجازت دیتا ہے۔
- `JurisdictionFlag` — ریگولیٹری درجہ بندیوں کی فہرست دینے والا bitmask (مثلاً `EU_EEA`, `JP_FIEL`, `US_FED`, `SANCTIONS_SCREENED`).
- `ParticipantSelector` — متاثرہ فریق کی وضاحت کرتا ہے:
  - `AccountId`, `DomainId`, یا `UAID`.
  - prefix-based selectors (`DomainPrefix`, `UaidPrefix`) رجسٹریز سے میچ کرنے کے لئے۔
  - Space Directory manifests کے لئے `CapabilityTag` (مثلاً صرف FX-cleared DS)۔
  - `privacy_commitments_any_of` gating تاکہ قواعد میچ ہونے سے پہلے lanes مخصوص Nexus privacy commitments
    کا اعلان کریں (NX-10 manifest surface کو mirror کرتا ہے اور `LanePrivacyRegistry` snapshots میں نافذ ہوتا ہے)۔

### LaneCompliancePolicy

پالیسیاں Norito-encoded structs ہیں جو گورننس کے ذریعے شائع ہوتی ہیں:

```text
LaneCompliancePolicy {
    id: LaneCompliancePolicyId,
    version: u32,
    lane_id: LaneId,
    jurisdiction: JurisdictionSet,
    allow: Vec<AllowRule>,
    deny: Vec<DenyRule>,
    transfer_limits: Vec<TransferLimit>,
    audit_controls: AuditControls,
    metadata: MetadataMap,
}
```

- `AllowRule` ایک `ParticipantSelector`، اختیاری jurisdiction override، capability tags اور reason codes کو جوڑتا ہے۔
- `DenyRule` allow ساخت کو mirror کرتا ہے مگر پہلے evaluate ہوتا ہے (deny wins).
- `TransferLimit` asset/bucket کے حساب سے حدیں رکھتا ہے:
  - `max_notional_xor` اور `max_daily_notional_xor`.
  - `asset_limits[{asset_id, per_tx, per_day}]`.
  - `relationship_limits` (مثلاً CBDC retail بمقابلہ wholesale)۔
- `AuditControls` یہ ترتیب دیتا ہے:
  - کیا Torii ہر denial کو آڈٹ لاگ میں محفوظ کرے۔
  - کیا کامیاب فیصلوں کو Norito digests میں sample کیا جائے۔
  - `LaneComplianceDecisionRecord` کے لئے مطلوبہ retention window۔

### اسٹوریج اور ڈسٹری بیوشن

- تازہ ترین policy hashes Space Directory manifest میں validator keys کے ساتھ رہتے ہیں۔
  `LaneCompliancePolicyReference` (policy id + version + hash) manifest فیلڈ بن جاتا ہے تاکہ validators اور SDKs
  canonical policy blob حاصل کر سکیں۔
- `iroha_config` `compliance.policy_cache_dir` ایکسپوز کرتا ہے تاکہ Norito payload اور اس کی detached signature محفوظ ہو۔
  نوڈز اپ ڈیٹس لاگو کرنے سے پہلے signatures ویری فائی کرتے ہیں تاکہ tampering سے بچا جا سکے۔
- پالیسیاں Torii کے استعمال کردہ Norito admission manifests میں بھی embed ہوتی ہیں تاکہ CI/SDKs validators سے بات کئے بغیر
  پالیسی evaluation دوبارہ چلا سکیں۔

## گورننس اور لائف سائیکل

1. **پروپوزل** — گورننس `ProposeLaneCompliancePolicy` Norito payload، jurisdiction justification، اور activation epoch کے ساتھ جمع کراتی ہے۔
2. **ریویو** — کمپلائنس ریویورز `LaneCompliancePolicyReviewEvidence` پر دستخط کرتے ہیں (auditable، `governance::ReviewEvidenceStore` میں محفوظ)۔
3. **ایکٹیویشن** — delay window کے بعد validators `ActivateLaneCompliancePolicy` کال کر کے پالیسی ingest کرتے ہیں۔ Space Directory manifest نئے policy reference کے ساتھ اٹامکلی اپ ڈیٹ ہوتا ہے۔
4. **ترمیم/منسوخی** — `AmendLaneCompliancePolicy` diff metadata لے جاتا ہے جبکہ پچھلا ورژن forensic replay کے لئے رکھا جاتا ہے؛
   `RevokeLaneCompliancePolicy` policy id کو `denied` پر pin کر دیتا ہے تاکہ Torii اس lane کی طرف جانے والی ٹریفک کو
   اس وقت تک رد کرے جب تک کوئی متبادل فعال نہ ہو جائے۔

Torii فراہم کرتا ہے:

- `GET /v1/lane-compliance/policies/{lane_id}` — تازہ ترین policy reference حاصل کریں۔
- `POST /v1/lane-compliance/policies` — صرف گورننس endpoint جو ISI proposal helpers کو mirror کرتا ہے۔
- `GET /v1/lane-compliance/decisions` — paginated آڈٹ لاگ جس میں `lane_id`, `decision`, `jurisdiction`, `reason_code` فلٹر شامل ہیں۔

CLI/SDK کمانڈز ان HTTP سرفیسز کو wrap کرتی ہیں تاکہ آپریٹرز ریویوز اسکرپٹ کر سکیں اور
artefacts (signed policy blob + reviewer attestations) حاصل کر سکیں۔

## انفورسمنٹ پائپ لائن

1. **Admission (Torii)**
   - `Torii` اس وقت فعال پالیسی ڈاؤن لوڈ کرتا ہے جب lane manifest تبدیل ہو یا cached signature کی معیاد ختم ہو۔
   - `/v1/pipeline` کیو میں آنے والی ہر ٹرانزیکشن کو `LaneComplianceContext` سے ٹیگ کیا جاتا ہے
     (participant ids، UAID، dataspace manifest metadata، policy id، اور `crates/iroha_core/src/interlane/mod.rs` میں بیان کردہ
     تازہ ترین `LanePrivacyRegistry` snapshot)۔
   - UAID رکھنے والی اتھارٹیز کے پاس routed dataspace کے لئے فعال Space Directory manifest ہونا ضروری ہے؛
     Torii قواعد evaluate ہونے سے پہلے ہی اس وقت ٹرانزیکشنز reject کر دیتا ہے جب UAID اس dataspace سے bind نہ ہو۔
   - `compliance::Engine` پہلے `deny` قواعد، پھر `allow` قواعد evaluate کرتا ہے، اور آخر میں transfer limits نافذ کرتا ہے۔
     ناکام ٹرانزیکشنز typed error (`ERR_LANE_COMPLIANCE_DENIED`) واپس کرتی ہیں، جس میں reason + policy id شامل ہوتا ہے۔
   - Admission ایک تیز prefiltor ہے؛ کنسینسس ویلیڈیشن وہی قواعد state snapshots کے ساتھ دوبارہ چیک کرتی ہے تاکہ انفورسمنٹ ڈیٹرمنسٹک رہے۔
2. **Execution (iroha_core)**
   - بلاک کی تعمیر کے دوران `iroha_core::tx::validate_transaction_internal` وہی lane governance/UAID/privacy/compliance چیکس
     `StateTransaction` snapshots (`lane_manifests`, `lane_privacy_registry`, `lane_compliance`) کے ذریعے دوبارہ چلاتا ہے۔
     یہ Torii cache کے stale ہونے پر بھی کنسینسس کے لئے اہم انفورسمنٹ برقرار رکھتا ہے۔
   - lane manifests یا compliance پالیسیوں میں تبدیلی والی ٹرانزیکشنز بھی اسی validation path سے گزرتی ہیں؛ admission-only bypass نہیں ہے۔
3. **Async hooks**
   - RBC gossip اور DA fetchers policy id کو ٹیلی میٹری کے ساتھ attach کرتے ہیں تاکہ دیر سے آنے والے فیصلے درست rule version سے جوڑے جا سکیں۔
   - `iroha_cli` اور SDK helpers `LaneComplianceDecision::explain()` ایکسپوز کرتے ہیں تاکہ آٹومیشن انسانی سمجھ میں آنے والی تشخیص فراہم کرے۔

انجن ڈیٹرمنسٹک اور pure ہے؛ manifest/policy ڈاؤن لوڈ ہونے کے بعد یہ بیرونی سسٹمز سے رابطہ نہیں کرتا۔
اس سے CI fixtures اور multi-node reproduction سیدھے رہتے ہیں۔

## آڈٹ اور ٹیلی میٹری

- **میٹرکس**
  - `nexus_lane_policy_decisions_total{lane_id,decision,reason}`.
  - `nexus_lane_policy_rate_limited_total{lane_id,limit_kind}`.
  - `nexus_lane_policy_cache_age_seconds{lane_id}` (activation delay سے کم رہنا چاہیے).
- **لاگز**
  - ساختہ ریکارڈز `policy_id`, `version`, `participant`, `UAID`, جرِسڈکشن فلیگز، اور خلاف ورزی والی ٹرانزیکشن کے Norito hash کو محفوظ کرتے ہیں۔
  - `LaneComplianceDecisionRecord` Norito میں encode ہو کر `world.compliance_logs::<lane_id>::<ts>::<nonce>` کے تحت محفوظ ہوتا ہے جب `AuditControls` durable storage مانگے۔
- **Evidence bundles**
  - `cargo xtask nexus-lane-audit` میں `--lane-compliance <path>` موڈ شامل ہے جو پالیسی، ریویور signatures، metrics snapshot،
    اور حالیہ آڈٹ لاگ کو JSON + Parquet آؤٹ پٹ میں merge کرتا ہے۔ یہ فلیگ درج ذیل JSON payload توقع کرتا ہے:

    ```json
    {
      "lanes": [
        {
          "lane_id": 12,
          "policy": { "...": "LaneCompliancePolicy JSON blob" },
          "reviewer_signatures": [
            {
              "reviewer": "auditor@example.com",
              "signature_hex": "deadbeef",
              "signed_at": "2026-02-12T09:00:00Z",
              "notes": "Q1 regulator packet"
            }
          ],
          "metrics_snapshot": {
            "nexus_lane_policy_decisions_total": {
              "allow": 42,
              "deny": 1
            }
          },
          "audit_log": [
            {
              "decision": "allow",
              "policy_id": "lane-12-policy",
              "recorded_at": "2026-02-12T09:00:00Z"
            }
          ]
        }
      ]
    }
    ```

    CLI ہر `policy` blob کو embed کرنے سے پہلے `lane_id` کے ساتھ میچ ہونے کی تصدیق کرتا ہے تاکہ پرانا یا غلط evidence
    ریگولیٹر پیکٹس اور روڈمیپ dashboards میں شامل نہ ہو۔
  - `--markdown-out` (ڈیفالٹ `artifacts/nexus_lane_audit.md`) اب ایک قابلِ مطالعہ خلاصہ بناتا ہے جو
    پیچھے رہ جانے والی lanes، non-zero backlog، pending manifests، اور گمشدہ compliance evidence کو نمایاں کرتا ہے
    تاکہ annex پیکٹس میں machine-readable artefacts کے ساتھ تیز ریویو سطح بھی موجود ہو۔

## رول آؤٹ پلان

1. **P0 — صرف مشاہدہ**
   - پالیسی types، اسٹوریج، Torii endpoints، اور میٹرکس ship کریں۔
   - Torii پالیسیوں کو `audit` موڈ (enforcement کے بغیر) میں evaluate کرتا ہے تاکہ ڈیٹا جمع ہو سکے۔
2. **P1 — deny/allow enforcement**
   - deny قواعد trigger ہونے پر Torii + execution میں سخت failures فعال کریں۔
   - تمام CBDC lanes کے لئے پالیسیاں لازمی کریں؛ عوامی DS audit موڈ میں رہ سکتے ہیں۔
3. **P2 — حدود اور jurisdiction overrides**
   - transfer limits اور jurisdiction flags کی enforcement فعال کریں۔
   - ٹیلی میٹری `dashboards/grafana/nexus_lanes.json` میں فیڈ کریں۔
4. **P3 — مکمل کمپلائنس آٹومیشن**
   - آڈٹ exports کو `SpaceDirectoryEvent` consumers کے ساتھ integrate کریں۔
   - پالیسی اپ ڈیٹس کو گورننس runbooks اور release automation سے جوڑیں۔

## قبولیت اور ٹیسٹنگ

- `integration_tests/tests/nexus/compliance.rs` کے انضمامی ٹیسٹس یہ کور کرتے ہیں:
  - allow/deny combinations، jurisdiction overrides، اور transfer limits؛
  - manifest/policy activation races؛ اور
  - multi-node runs میں Torii بمقابلہ `iroha_core` decision parity۔
- `crates/iroha_core/src/compliance` کے unit tests خالص evaluation engine، cache invalidation timers، اور metadata parsing کو validate کرتے ہیں۔
- Docs/SDK اپ ڈیٹس (Torii + CLI) میں پالیسی fetch کرنا، گورننس proposals جمع کرانا، error codes کی تشریح، اور audit evidence جمع کرنا دکھایا جانا چاہیے۔

NX-12 کو بند کرنے کے لئے اوپر والے artefacts کے ساتھ `status.md`/`roadmap.md` میں اسٹیٹس اپ ڈیٹس بھی درکار ہیں
جب staging clusters پر enforcement فعال ہو جائے۔

</div>
