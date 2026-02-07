---
lang: ur
direction: rtl
source: docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: گٹھ جوڑ-روٹڈ ٹریس-آڈٹ -2026Q1
عنوان: Q1 2026 (B1) کے لئے روٹ ٹریس آڈٹ رپورٹ
تفصیل: سہ ماہی ٹیلی میٹری مشقوں کے نتائج کا احاطہ کرنے والی `docs/source/nexus_routed_trace_audit_report_2026q1.md` کی ایک عین مطابق کاپی۔
---
<!--
  SPDX-License-Identifier: Apache-2.0
-->

::: نوٹ قانونی ذریعہ
یہ صفحہ `docs/source/nexus_routed_trace_audit_report_2026q1.md` کی عکاسی کرتا ہے۔ بقیہ ترجمے آنے تک دونوں ورژن کو ہم آہنگ رکھیں۔
:::

# روٹ ٹریس آڈٹ رپورٹ Q1 2026 (B1)

روڈ میپ آئٹم ** B1-روٹ ٹریس آڈٹ اور ٹیلی میٹری بیس لائن ** کے لئے Nexus پر روٹ ٹریس سافٹ ویئر کا سہ ماہی جائزہ لینے کی ضرورت ہے۔ اس رپورٹ میں Q1 2026 (جنوری مارچ) آڈٹ ونڈو کو دستاویز کیا گیا ہے تاکہ گورننس بورڈ Q2 لانچ کی مشقوں سے قبل ٹیلی میٹرک حیثیت کو منظور کرسکے۔

## دائرہ کار اور شیڈول

| ٹریس ID | ونڈو (UTC) | ہدف |
| ---------- | ---------------- | ----------- |
| `TRACE-LANE-ROUTING` | 2026-02-17 09: 00-09: 45 | ملٹی لین کو چالو کرنے سے پہلے لین کی قبولیت چارٹ ، گپ شپ قطاریں ، اور بہاؤ کے انتباہات چیک کریں۔ |
| `TRACE-TELEMETRY-BRIDGE` | 2026-02-24 10: 00-10: 45 | OTLP ری پلے ، ڈف بوٹ برابری ، اور SDK ٹیلی میٹرک تفہیم اور 4/and7 مراحل سے پہلے چیک کرتا ہے۔ |
| `TRACE-CONFIG-DELTA` | 2026-03-01 12: 00-12: 30 | گورننس سے منظور شدہ `iroha_config` اختلافات کی تصدیق کریں اور RC1 کو منقطع کرنے سے پہلے رول بیک کے لئے تیاری کریں۔ |

ہر ریہرسل روٹ ٹریس ٹولز (ٹیلی میٹری `nexus.audit.outcome` + کاؤنٹرز Prometheus) کے ساتھ پروڈکشن نما ڈھانچے پر ہوتی ہے ، الرٹ مینجر قواعد بھری ہوئی اور `docs/examples/` پر برآمد ہونے والے شواہد۔

## طریقہ کار

1. ** ٹیلی میٹرک مجموعہ۔ ** تمام نوڈس نے ساختی واقعہ `nexus.audit.outcome` اور اس سے وابستہ میٹرکس (`nexus_audit_outcome_total*`) جاری کیا۔ ہیلپر `scripts/telemetry/check_nexus_audit_outcome.py` نے JSON لاگ کی پیروی کی ، ایونٹ کی حیثیت کی جانچ کی ، اور `docs/examples/nexus_audit_outcomes/` کے تحت پے لوڈ کو محفوظ کیا۔ .
2. ** انتباہات کی تصدیق کریں۔ CI ہر تبدیلی پر فائل `dashboards/alerts/tests/nexus_audit_rules.test.yml` چلاتا ہے۔ ہر ونڈو کے دوران خود ان قواعد کا دستی طور پر تجربہ کیا گیا تھا۔
3۔ ** گرفتاری کی نگرانی بورڈ۔
4. ** آڈیٹرز ’نوٹس۔

## نتائج

| ٹریس ID | نتیجہ | گائیڈ | نوٹ |
| ---------- | --------- | ---------- | ------- |
| `TRACE-LANE-ROUTING` | پاس | فائر/بازیافت الرٹ اسنیپ شاٹس (اندرونی لنک) + ریبوٹ `dashboards/alerts/tests/soranet_lane_rules.test.yml` ؛ ٹیلی میٹک اختلافات [Nexus منتقلی نوٹ] (./nexus-transition-notes#quarterly-routed-trace-audit-schedule) میں ریکارڈ کیے گئے ہیں۔ | قطاریں قبول کرنے کے لئے P95 612 ایم ایس (ہدف <= 750 ایم ایس) پر رہا۔ کسی فالو اپ کی ضرورت نہیں ہے۔ |
| `TRACE-TELEMETRY-BRIDGE` | پاس | `status.md` پر رجسٹرڈ ری پلے OTLP فنگر پرنٹ کے ساتھ محفوظ پے لوڈ `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json`۔ | ایس ڈی کے کے نمکیات مورچا بیس لائن کے مطابق ہیں۔ صفر کے اختلافات کو مختلف بوٹ پر رپورٹ کریں۔ |
| `TRACE-CONFIG-DELTA` | پاس (تخفیف بند) | گورننس ٹریکر لاگ (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`) + منشور TLS فائل (`artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`) + مینی فیسٹ ٹیلی میٹک پیکیج (`artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`)۔ | Q2 دوبارہ مستند TLS فائل ہیش کو دوبارہ حاصل کریں اور اس بات کی تصدیق کی کہ کوئی وقفے نہیں ہیں۔ ٹیلی میٹک مینی فیسٹ رجسٹر میں سلاٹ رینج 912-936 اور بوجھ سیڈ `NEXUS-REH-2026Q2` ہے۔ |تمام نشانات نے اپنی ونڈوز میں کم از کم ایک `nexus.audit.outcome` ایونٹ تیار کیا ، الرٹ مینجر کی دہلیز (`NexusAuditOutcomeFailure` پوری سہ ماہی میں سبز رہے) سے ملاقات کی۔

## فالو اپ

- TLS فنگر پرنٹ `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` کے ساتھ اپ ڈیٹ روٹ ٹریس توسیع ؛ تخفیف `NEXUS-421` منتقلی کے نوٹوں میں بند ہے۔
- Android and4/and7 نظرثانی کے لئے مساوات کے ثبوت کو مستحکم کرنے کے لئے Torii کے خام OTLP ری پلے اور مختلف ٹکڑوں کو محفوظ شدہ دستاویزات جاری رکھیں۔
- اس بات کی تصدیق کریں کہ آئندہ `TRACE-MULTILANE-CANARY` ریہرسل اسی ٹیلی میٹرک اسسٹنٹ کو دوبارہ استعمال کرتی ہے تاکہ Q2 کے دستخط منظور شدہ ورک فلو سے فائدہ اٹھائیں۔

## آرٹ فیکٹ انڈیکس

| اصل | مقام |
| ------- | ------------ |
| ٹیلی میٹک چیکر | `scripts/telemetry/check_nexus_audit_outcome.py` |
| ٹیسٹ کے قواعد اور انتباہات | `dashboards/alerts/nexus_audit_rules.yml` ، `dashboards/alerts/tests/nexus_audit_rules.test.yml` |
| مثال کے طور پر پے لوڈ کا نتیجہ | `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` |
| ترتیبات کے اختلافات ٹریکر | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` |
| روٹ ٹریس ٹیبل اور نوٹ | [Nexus منتقلی نوٹس] (./nexus-transition-notes) |

یہ رپورٹ ، مذکورہ بالا طبقات ، اور الرٹ/ٹیلی میٹرک برآمدات کو سہ ماہی کے لئے B1 قریب گورننس کے فیصلے کے ریکارڈ سے منسلک کرنا ہوگا۔