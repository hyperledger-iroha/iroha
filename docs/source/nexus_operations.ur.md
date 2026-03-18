---
lang: ur
direction: rtl
source: docs/source/nexus_operations.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 413953b2ca41875bb960be56995aa03dbaa58af4a30f927c24d1e8815c7da472
source_last_modified: "2025-11-08T16:26:57.335679+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- اردو ترجمہ برائے docs/source/nexus_operations.md -->

# Nexus آپریشنز رن بک (NX-14)

**روڈمیپ لنک:** NX-14 — Nexus دستاویزات اور آپریٹر رن بکس
**اسٹیٹس:** ڈرافٹ 2026-03-24 — `docs/source/nexus_overview.md` اور
`docs/source/sora_nexus_operator_onboarding.md` کے آن بورڈنگ فلو کے مطابق۔
**سامعین:** نیٹ ورک آپریٹرز، SRE/on-call انجینئرز، گورننس کوآرڈینیٹرز۔

یہ رن بک Sora Nexus (Iroha 3) نوڈز کے آپریشنل لائف سائیکل کا خلاصہ پیش کرتی ہے۔
یہ گہری اسپیسفیکیشن (`docs/source/nexus.md`) یا lane مخصوص گائیڈز
(مثلاً `docs/source/cbdc_lane_playbook.md`) کا متبادل نہیں، بلکہ وہ عملی
چیک لسٹس، ٹیلی میٹری ہکس، اور ثبوتی تقاضے اکٹھے کرتی ہے جنہیں نوڈ کو
قبول کرنے یا اپ گریڈ کرنے سے پہلے پورا کرنا لازم ہے۔

## 1. آپریشنل لائف سائیکل

| مرحلہ | چیک لسٹ | ثبوت |
|-------|---------|------|
| **پری فلائٹ** | آرٹیفیکٹ hashes/signatures کی توثیق، `profile = "iroha3"` کی تصدیق، اور config ٹیمپلیٹس تیار کرنا۔ | `scripts/select_release_profile.py` آؤٹ پٹ، checksum لاگ، اور signed manifest bundle۔ |
| **کیٹلاگ الائنمنٹ** | `[nexus]` میں lane + dataspace کیٹلاگ، روٹنگ پالیسی، اور DA thresholds کو کونسل کے جاری کردہ manifest کے مطابق اپ ڈیٹ کرنا۔ | `irohad --sora --config ... --trace-config` آؤٹ پٹ ٹکٹ کے ساتھ محفوظ۔ |
| **اسموک اور کٹ اوور** | `irohad --sora --config ... --trace-config` چلائیں، CLI اسموک ٹیسٹ (مثلاً `FindNetworkStatus`) کریں، ٹیلی میٹری endpoints چیک کریں، پھر admission کی درخواست دیں۔ | اسموک ٹیسٹ لاگ + Alertmanager silence کنفرمیشن۔ |
| **اسٹیڈی اسٹیٹ** | dashboards/alerts مانیٹر کریں، گورننس cadence کے مطابق keys rotate کریں، اور configs + runbooks کو manifest revisions کے ساتھ sync میں رکھیں۔ | سہ ماہی ریویو منٹس، منسلک dashboard اسکرین شاٹس، اور rotation ٹکٹ IDs۔ |

تفصیلی آن بورڈنگ ہدایات (بشمول key replacement، routing policy مثالیں، اور release profile validation)
`docs/source/sora_nexus_operator_onboarding.md` میں موجود ہیں۔ جب آرٹیفیکٹ فارمیٹس یا
اسکرپٹس تبدیل ہوں تو اسی دستاویز کو ریفرنس کریں۔

## 2. تبدیلیوں کا نظم اور گورننس ہکس

1. **ریلیز اپ ڈیٹس**
   - `status.md` اور `roadmap.md` میں اعلانات فالو کریں۔
   - ہر ریلیز PR میں `docs/source/sora_nexus_operator_onboarding.md` کی بھری ہوئی چیک لسٹ شامل ہونی چاہیے۔
2. **lane manifest تبدیلیاں**
   - گورننس Space Directory کے ذریعے signed manifest bundles شائع کرتی ہے۔
   - آپریٹرز signatures ویری فائی کریں، کیٹلاگ entries اپ ڈیٹ کریں، اور manifests کو
     `docs/source/project_tracker/nexus_config_deltas/` میں آرکائیو کریں۔
3. **کانفیگریشن ڈیلٹاز**
   - `config/config.toml` میں ہر تبدیلی کے لئے lane ID اور dataspace alias والے ٹکٹ کی ضرورت ہے۔
   - جب نوڈ شامل ہو یا اپ گریڈ ہو تو موثر config کی redacted کاپی ٹکٹ میں رکھیں۔
4. **rollback drills**
   - سہ ماہی rollback ریہرسل کریں (نوڈ روکیں، پچھلا bundle بحال کریں، config دوبارہ چلائیں، smoke دوبارہ کریں)۔ نتائج
     `docs/source/project_tracker/nexus_config_deltas/<date>-rollback.md` میں ریکارڈ کریں۔
5. **compliance approvals**
   - پرائیویٹ/CBDC lanes کو DA policy یا telemetry redaction knobs بدلنے سے پہلے compliance sign-off لینا لازم ہے۔
     `docs/source/cbdc_lane_playbook.md#governance-hand-offs` دیکھیں۔

## 3. ٹیلی میٹری اور SLO کوریج

Dashboards اور alert rules `dashboards/` کے تحت ورژنڈ ہیں اور
`docs/source/nexus_telemetry_remediation_plan.md` میں دستاویز شدہ ہیں۔ آپریٹرز پر لازم ہے:

- PagerDuty/on-call targets کو `dashboards/alerts/nexus_audit_rules.yml` اور
  `dashboards/alerts/torii_norito_rpc_rules.yml` میں lane health rules کے ساتھ سبسکرائب کریں
  (Torii/Norito transport کور ہوتا ہے)۔
- درج ذیل Grafana boards آپریشنز پورٹل پر شائع کریں:
  - `nexus_lanes.json` (lane height, backlog, DA parity).
  - `nexus_settlement.json` (settlement latency, treasury deltas).
  - `android_operator_console.json` / SDK dashboards جب lane موبائل ٹیلی میٹری پر منحصر ہو۔
- OTEL exporters کو `docs/source/torii/norito_rpc_telemetry.md` کے مطابق رکھیں جب Torii binary transport فعال ہو۔
- ٹیلی میٹری remediation checklist کم از کم سہ ماہی چلائیں (Section 5 in
  `docs/source/nexus_telemetry_remediation_plan.md`) اور مکمل فارم ops ریویو منٹس کے ساتھ لگائیں۔

### کلیدی میٹرکس

| Metric | Description | Alert threshold |
|--------|-------------|-----------------|
| `nexus_lane_height{lane_id}` | ہر lane کی head height؛ رکے ہوئے validators کا پتہ دیتا ہے۔ | اگر 3 مسلسل slots میں اضافہ نہ ہو تو alert۔ |
| `nexus_da_backlog_chunks{lane_id}` | ہر lane کے لئے غیر پروسیس شدہ DA chunks۔ | configured limit سے اوپر alert (default: public کیلئے 64، private کیلئے 8). |
| `nexus_settlement_latency_seconds{lane_id}` | lane commit اور global settlement کے درمیان وقت۔ | alert >900ms P99 (public) یا >1200ms (private). |
| `torii_request_failures_total{scheme="norito_rpc"}` | Norito RPC error count۔ | اگر 5 منٹ error ratio >2% ہو تو alert۔ |
| `telemetry_redaction_override_total` | ٹیلی میٹری redaction overrides۔ | فوری alert (Sev 2) اور compliance ٹکٹ لازم۔ |

## 4. واقعہ ردعمل

| Severity | Definition | Required actions |
|----------|------------|------------------|
| **Sev 1** | data-space isolation breach، settlement halt >15 منٹ، یا governance vote corruption۔ | Nexus Primary + Release Engineering + Compliance کو page کریں۔ lane admission freeze کریں، metrics/logs جمع کریں، 60 منٹ میں incident comms شائع کریں، اور <=5 کاروباری دن میں RCA فائل کریں۔ |
| **Sev 2** | lane backlog SLA سے اوپر، telemetry blind spot >30 منٹ، manifest rollout ناکام۔ | Nexus Primary + SRE کو page کریں، 4 گھنٹوں میں mitigation، اور 2 کاروباری دن میں follow-up issues درج کریں۔ |
| **Sev 3** | non-blocking regressions (docs drift، alert misfire). | tracker میں لاگ کریں، sprint کے اندر fix شیڈول کریں۔ |

Incident tickets میں شامل ہونا چاہیے:

1. متاثرہ lane/data-space IDs اور manifest hashes۔
2. Timeline (UTC) جس میں detection، mitigation، recovery، اور communications ہوں۔
3. detection کی تائید کرنے والی metrics/screenshots۔
4. follow-up tasks (owners/dates کے ساتھ) اور آیا automation/runbooks اپ ڈیٹ کی ضرورت ہے۔

## 5. ثبوت اور آڈٹ ٹریل

- **Artefact archive:** bundles، manifests، اور telemetry exports کو `artifacts/nexus/<lane>/<date>/` میں محفوظ کریں۔
- **Config snapshots:** ہر release کے لئے redacted `config.toml` اور `trace-config` output۔
- **Governance linkage:** کونسل میٹنگ نوٹس اور signed decisions، onboarding یا incident ٹکٹ میں حوالہ کے ساتھ۔
- **Telemetry exports:** lane سے متعلق Prometheus TSDB chunks کے ہفتہ وار snapshots، کم از کم 12 ماہ کے لئے audit share کے ساتھ منسلک۔
- **Runbook versioning:** اس فائل کی ہر بڑی تبدیلی میں `docs/source/project_tracker/nexus_config_deltas/README.md`
  میں changelog entry شامل کریں تاکہ auditors تبدیلیوں کا وقت ٹریک کر سکیں۔

## 6. متعلقہ وسائل

- `docs/source/nexus_overview.md` — ہائی لیول خلاصہ/آرکیٹیکچر۔
- `docs/source/nexus.md` — مکمل تکنیکی اسپیسفیکیشن۔
- `docs/source/nexus_lanes.md` — lane جیومیٹری۔
- `docs/source/nexus_transition_notes.md` — مائیگریشن روڈمیپ۔
- `docs/source/cbdc_lane_playbook.md` — CBDC مخصوص پالیسیاں۔
- `docs/source/sora_nexus_operator_onboarding.md` — release/onboarding فلو۔
- `docs/source/nexus_telemetry_remediation_plan.md` — telemetry guardrails۔

جب NX-14 آگے بڑھے یا نئی lane کلاسیں، telemetry rules، یا governance hooks متعارف ہوں تو
ان حوالہ جات کو اپ ٹو ڈیٹ رکھیں۔

</div>
