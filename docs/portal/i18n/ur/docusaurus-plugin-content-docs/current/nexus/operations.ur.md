---
lang: ur
direction: rtl
source: docs/portal/docs/nexus/operations.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: nexus-operations
title: Nexus آپریشنز رن بُک
description: Nexus آپریٹر ورک فلو کا فیلڈ کے لئے تیار خلاصہ، جو `docs/source/nexus_operations.md` کی عکاسی کرتا ہے۔
---

اس صفحے کو `docs/source/nexus_operations.md` کے تیز ریفرنس کے طور پر استعمال کریں۔ یہ آپریشنل چیک لسٹ، تبدیلی کے انتظام کے hooks، اور ٹیلیمیٹری کوریج کی ضروریات کو سمیٹتا ہے جن پر Nexus آپریٹرز کو عمل کرنا ہوتا ہے۔

## لائف سائیکل چیک لسٹ

| مرحلہ | اقدامات | ثبوت |
|-------|--------|----------|
| پری فلائٹ | ریلیز ہیشز/سگنیچرز کی تصدیق کریں، `profile = "iroha3"` کنفرم کریں، اور کنفیگ ٹیمپلیٹس تیار کریں۔ | `scripts/select_release_profile.py` آؤٹ پٹ، checksum لاگ، دستخط شدہ مینفسٹ بَنڈل۔ |
| کیٹلاگ الائنمنٹ | `[nexus]` کیٹلاگ، روٹنگ پالیسی اور DA تھریش ہولڈز کو کونسل کے جاری کردہ مینفسٹ کے مطابق اپ ڈیٹ کریں، پھر `--trace-config` کیپچر کریں۔ | `irohad --sora --config ... --trace-config` آؤٹ پٹ جو onboarding ٹکٹ کے ساتھ محفوظ ہے۔ |
| اسموک اور کٹ اوور | `irohad --sora --config ... --trace-config` چلائیں، CLI اسموک (`FindNetworkStatus`) چلائیں، ٹیلیمیٹری ایکسپورٹس کی توثیق کریں، اور ایڈمیشن کی درخواست دیں۔ | اسموک ٹیسٹ لاگ + Alertmanager کنفرمیشن۔ |
| اسٹیڈی اسٹیٹ | dashboards/alerts مانیٹر کریں، گورننس کی cadence کے مطابق کیز روٹیٹ کریں، اور جب مینفسٹ بدلے تو configs/runbooks ہم آہنگ کریں۔ | سہ ماہی ریویو منٹس، ڈیش بورڈ اسکرین شاٹس، روٹیشن ٹکٹ IDs۔ |

تفصیلی onboarding (کلیدوں کی تبدیلی، روٹنگ ٹیمپلیٹس، ریلیز پروفائل کے مراحل) `docs/source/sora_nexus_operator_onboarding.md` میں موجود ہے۔

## تبدیلی کا انتظام

1. **ریلیز اپ ڈیٹس** - `status.md`/`roadmap.md` میں اعلانات ٹریک کریں؛ ہر ریلیز PR کے ساتھ onboarding چیک لسٹ منسلک کریں۔
2. **Lane مینفسٹ تبدیلیاں** - Space Directory سے دستخط شدہ بَنڈلز کی تصدیق کریں اور انہیں `docs/source/project_tracker/nexus_config_deltas/` کے تحت محفوظ کریں۔
3. **کنفیگریشن ڈیلٹاز** - `config/config.toml` میں ہر تبدیلی کے لئے lane/data-space کا حوالہ دینے والا ٹکٹ ضروری ہے۔ جب نوڈز شامل ہوں یا اپ گریڈ ہوں تو موثر کنفیگ کی ریڈیکٹڈ کاپی محفوظ کریں۔
4. **Rollback drills** - سہ ماہی stop/restore/smoke طریقہ کار کی مشق کریں؛ نتائج `docs/source/project_tracker/nexus_config_deltas/<date>-rollback.md` میں لاگ کریں۔
5. **Compliance approvals** - private/CBDC lanes کو DA پالیسی یا ٹیلیمیٹری redaction knobs بدلنے سے پہلے compliance منظوری درکار ہے (دیکھیں `docs/source/cbdc_lane_playbook.md`).

## ٹیلیمیٹری اور SLOs

- Dashboards: `dashboards/grafana/nexus_lanes.json`, `nexus_settlement.json`, اور SDK مخصوص ویوز (مثلاً `android_operator_console.json`).
- Alerts: `dashboards/alerts/nexus_audit_rules.yml` اور Torii/Norito transport قواعد (`dashboards/alerts/torii_norito_rpc_rules.yml`).
- دیکھنے کے لئے میٹرکس:
  - `nexus_lane_height{lane_id}` - تین slots تک پیش رفت صفر ہو تو الرٹ کریں۔
  - `nexus_da_backlog_chunks{lane_id}` - lane مخصوص تھریش ہولڈز سے اوپر الرٹ کریں (ڈیفالٹ 64 public / 8 private).
  - `nexus_settlement_latency_seconds{lane_id}` - جب P99 900 ms (public) یا 1200 ms (private) سے اوپر جائے تو الرٹ کریں۔
  - `torii_request_failures_total{scheme="norito_rpc"}` - اگر 5 منٹ کی error ratio >2% ہو تو الرٹ کریں۔
  - `telemetry_redaction_override_total` - Sev 2 فوری؛ یقینی بنائیں کہ overrides کے لئے compliance ٹکٹس ہوں۔
- [Nexus telemetry remediation plan](./nexus-telemetry-remediation) میں دیا گیا چیک لسٹ کم از کم سہ ماہی چلائیں اور بھرا ہوا فارم آپریشنز ریویو نوٹس کے ساتھ منسلک کریں۔

## انسیڈنٹ میٹرکس

| شدت | تعریف | ردعمل |
|----------|------------|----------|
| Sev 1 | data-space isolation کی خلاف ورزی، settlement کا 15 منٹ سے زیادہ رکنا، یا governance ووٹ میں خرابی۔ | Nexus Primary + Release Engineering + Compliance کو پیج کریں، ایڈمیشن فریز کریں، آرٹیفیکٹس جمع کریں، <=60 منٹ میں کمیونیکیشن جاری کریں، RCA <=5 کاروباری دن۔ |
| Sev 2 | lane backlog SLA کی خلاف ورزی، ٹیلیمیٹری blind spot >30 منٹ، مینفسٹ rollout ناکام۔ | Nexus Primary + SRE کو پیج کریں، <=4 گھنٹے میں مٹیگیٹ کریں، 2 کاروباری دن کے اندر follow-ups فائل کریں۔ |
| Sev 3 | غیر رکاوٹی drift (docs، alerts). | tracker میں لاگ کریں اور sprint کے اندر fix شیڈول کریں۔ |

انسیڈنٹ ٹکٹس میں متاثرہ lane/data-space IDs، مینفسٹ ہیشز، ٹائم لائن، سپورٹنگ میٹرکس/لاگز، اور follow-up ٹاسکس/مالکان درج ہونا ضروری ہیں۔

## ثبوت آرکائیو

- bundles/manifestes/telemetry exports کو `artifacts/nexus/<lane>/<date>/` کے تحت محفوظ کریں۔
- ہر ریلیز کے لئے redacted configs + `--trace-config` آؤٹ پٹ محفوظ رکھیں۔
- جب config یا مینفسٹ تبدیلیاں ہوں تو کونسل منٹس + دستخط شدہ فیصلے منسلک کریں۔
- Nexus میٹرکس کے لئے متعلقہ Prometheus ہفتہ وار snapshots 12 ماہ تک محفوظ کریں۔
- runbook edits کو `docs/source/project_tracker/nexus_config_deltas/README.md` میں ریکارڈ کریں تاکہ آڈیٹرز جان سکیں ذمہ داریاں کب بدلیں۔

## متعلقہ مواد

- Overview: [Nexus overview](./nexus-overview)
- Specification: [Nexus spec](./nexus-spec)
- Lane geometry: [Nexus lane model](./nexus-lane-model)
- Transition & routing shims: [Nexus transition notes](./nexus-transition-notes)
- Operator onboarding: [Sora Nexus operator onboarding](./nexus-operator-onboarding)
- Telemetry remediation: [Nexus telemetry remediation plan](./nexus-telemetry-remediation)
