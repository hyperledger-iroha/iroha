---
lang: he
direction: rtl
source: docs/portal/docs/nexus/operations.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
מזהה: nexus-operations
title: Nexus آپریشنز رن بُک
description: Nexus آپریٹر ورک فلو کا فیلڈ کے لئے تیار خلاصہ، جو `docs/source/nexus_operations.md` کی عکاسی کرتا ہے۔
---

اس صفحے کو `docs/source/nexus_operations.md` کے تیز ریفرنس کے طور پر استعمال کریں۔ 2009 سمیٹتا ہے جن پر Nexus آپریٹرز کو عمل کرنا ہوتا ہے۔

## لائف سائیکل چیک لسٹ

| مرحلہ | اقدامات | ثبوت |
|-------|--------|--------|
| پری فلائٹ | ریلیز ہیشز/سگنیچرز کی تصدیق کریں، `profile = "iroha3"` کنفرم کریں، اور کنفیگ ٹیمپلیٹس تیار کریں۔ | `scripts/select_release_profile.py` آؤٹ پٹ، checksum لاگ، دستخط شدہ مینفسٹ بَنڈل۔ |
| کیٹلاگ الائنمنٹ | `[nexus]` גירסה קודמת של תקשורת תקשורת. مطابق اپ ڈیٹ کریں، پھر `--trace-config` کیپچر کریں۔ | `irohad --sora --config ... --trace-config` آؤٹ پٹ جو onboarding ٹکٹ کے ساتھ محفوظ ہے۔ |
| اسموک اور کٹ اوور | `irohad --sora --config ... --trace-config` چلائیں، CLI اسموک (`FindNetworkStatus`) چلائیں، ٹیلیمیٹری ایکسپورٹس کی توثیق کریں، اور ایڈمیشن کی درخواست دیں۔ | اسموک ٹیسٹ لاگ + Alertmanager کنفرمیشن۔ |
| اسٹیڈی اسٹیٹ | לוחות מחוונים/התראות מכשירי שליטה/התראות. configs/runbooks. | سہ ماہی ریویو منٹس، ڈیش بورڈ اسکرین شاٹس، روٹیشن ٹکٹ IDs۔ |

הצטרפות למטוסים (בגילאי 1800 מרץ 2000) میں موجود ہے۔

## تبدیلی کا انتظام

1. **ریلیز اپ ڈیٹس** - `status.md`/`roadmap.md` میں اعلانات ٹریک کریں؛ ہر ریلیز PR کے ساتھ onboarding چیک لسٹ منسلک کریں۔
2. **Lane مینفسٹ تبدیلیاں** - Space Directory سے دستخط شدہ بَنڈلز کی تصدیق کریں اور انہیں `docs/source/project_tracker/nexus_config_deltas/` ‏
3. **שידורי עמודים** - `config/config.toml` ישנים מסלולי נתיב/מרחב נתונים. ضروری ہے۔ ‏
4. **Rollback drills** - سہ ماہی stop/restore/smoke طریقہ کار کی مشق کریں؛ نتائج `docs/source/project_tracker/nexus_config_deltas/<date>-rollback.md` میں لاگ کریں۔
5. **אישורי תאימות** - נתיבים פרטיים/CBDC. (دیکھیں `docs/source/cbdc_lane_playbook.md`).

## ٹیلیمیٹری اور SLOs

- Dashboards: `dashboards/grafana/nexus_lanes.json`, `nexus_settlement.json`, اور SDK مخصوص ویوز (مثلاً `android_operator_console.json`).
- Alerts: `dashboards/alerts/nexus_audit_rules.yml` اور Torii/Norito transport قواعد (`dashboards/alerts/torii_norito_rpc_rules.yml`).
- دیکھنے کے لئے میٹرکس:
  - `nexus_lane_height{lane_id}` - تین slots تک پیش رفت صفر ہو تو الرٹ کریں۔
  - `nexus_da_backlog_chunks{lane_id}` - lane مخصوص تھریش ہولڈز سے اوپر الرٹ کریں (ڈیفالٹ 64 public / 8 private).
  - `nexus_settlement_latency_seconds{lane_id}` - جب P99 900 ms (public) یا 1200 ms (private) سے اوپر جائے تو الرٹ کریں۔
  - `torii_request_failures_total{scheme="norito_rpc"}` - اگر 5 منٹ کی error ratio >2% ہو تو الرٹ کریں۔
  - `telemetry_redaction_override_total` - Sev 2 فوری؛ یقینی بنائیں کہ overrides کے لئے compliance ٹکٹس ہوں۔
- [Nexus תוכנית תיקון טלמטריה](./nexus-telemetry-remediation) מספר פעמים ہوا فارم آپریشنز ریویو نوٹس کے ساتھ منسلک کریں۔

## انسیڈنٹ میٹرکس| شدت | تعریف | ردعمل |
|--------|------------|--------|
| סב 1 | data-space isolation کی خلاف ورزی، settlement کا 15 منٹ سے زیادہ رکنا، یا governance ووٹ میں خرابی۔ | Nexus ראשי + הנדסת שחרור + תאימות 6 میں کمیونیکیشن جاری کریں، RCA <=5 کاروباری دن۔ |
| סב' 2 | lane backlog SLA کی خلاف ورزی، ٹیلیمیٹری blind spot >30 منٹ، مینفسٹ rollout ناکام۔ | Nexus Primary + SRE کو پیج کریں، <=4 گھنٹے میں مٹیگیٹ کریں، 2 کاروباری دن کے اندر follow-ups فائل کریں۔ |
| סוו 3 | غیر رکاوٹی drift (docs، alerts). | tracker میں لاگ کریں اور sprint کے اندر fix شیڈول کریں۔ |

מזהי נתיב/מרחב נתונים של נתיב/מרחב נתונים, מיומנויות/ים. follow-up ٹاسکس/مالکان درج ہونا ضروری ہیں۔

## ثبوت آرکائیو

- bundles/manifestes/telemetry exports کو `artifacts/nexus/<lane>/<date>/` کے تحت محفوظ کریں۔
- ہر ریلیز کے لئے redacted configs + `--trace-config` آؤٹ پٹ محفوظ رکھیں۔
- جب config یا مینفسٹ تبدیلیاں ہوں تو کونسل منٹس + دستخط شدہ فیصلے منسلک کریں۔
- Nexus میٹرکس کے لئے متعلقہ Prometheus ہفتہ وار snapshots 12 ماہ تک محفوظ کریں۔
- עריכות של פנקס runbook ב-`docs/source/project_tracker/nexus_config_deltas/README.md`.

## متعلقہ مواد

- סקירה כללית: [סקירה כללית Nexus](./nexus-overview)
- מפרט: [מפרט Nexus](./nexus-spec)
- גיאומטריית נתיב: [דגם נתיב Nexus](./nexus-lane-model)
- shims מעבר וניתוב: [Nexus הערות מעבר](./nexus-transition-notes)
- העלאת מפעיל: [הכנסת מפעיל Sora Nexus](./nexus-operator-onboarding)
- תיקון טלמטריה: [תוכנית תיקון טלמטריה Nexus](./nexus-telemetry-remediation)