---
lang: ur
direction: rtl
source: docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: گٹھ جوڑ-روٹڈ ٹریس-آڈٹ -2026Q1
عنوان: روٹ ٹریس آڈٹ رپورٹ 2026 Q1 (B1)
تفصیل: سہ ماہی ٹیلی میٹری کے جائزے کے نتائج کا احاطہ کرتے ہوئے ، `docs/source/nexus_routed_trace_audit_report_2026q1.md` کا آئینہ۔
---
<!--
  SPDX-License-Identifier: Apache-2.0
-->

::: نوٹ کینونیکل ماخذ
یہ صفحہ `docs/source/nexus_routed_trace_audit_report_2026q1.md` کی عکاسی کرتا ہے۔ باقی ترجمے آنے تک دونوں کاپیاں منسلک رکھیں۔
:::

# روٹ ٹریس آڈٹ رپورٹ 2026 Q1 (B1)

روڈ میپ آئٹم ** B1-روٹ ٹریس آڈٹ اور ٹیلی میٹری بیس لائن ** کے لئے Nexus کے روٹ ٹریس پروگرام کا سہ ماہی جائزہ لینے کی ضرورت ہے۔ اس رپورٹ میں Q1 2026 (جنوری مارچ) آڈٹ ونڈو کو دستاویز کیا گیا ہے تاکہ گورننس کونسل Q2 لانچ ٹرائلز سے قبل ٹیلی میٹری کرنسی کو منظور کرسکے۔

## دائرہ کار اور ٹائم لائن

| ٹریس ID | ونڈو (UTC) | مقصد |
| ---------- | ---------------- | ----------- |
| `TRACE-LANE-ROUTING` | 2026-02-17 09: 00-09: 45 | ملٹی لین کو چالو کرنے سے پہلے لین میں داخلے ، قطار کی گپ شپ اور الرٹ فلو کے ہسٹگرام چیک کریں۔ |
| `TRACE-TELEMETRY-BRIDGE` | 2026-02-24 10: 00-10: 45 | OTLP ری پلے ، BOT مختلف برابری اور SDK ٹیلی میٹری ingestion اور 4/and7 سنگ میل سے پہلے توثیق کریں۔ |
| `TRACE-CONFIG-DELTA` | 2026-03-01 12: 00-12: 30 | RC1 کٹ سے پہلے گورننس اور رول بیک کی تیاری کے ذریعہ منظور شدہ `iroha_config` ڈیلٹا کی تصدیق کریں۔ |

ہر ٹیسٹ کو روٹ ٹریس انسٹرومینٹیشن کے قابل (`nexus.audit.outcome` ٹیلی میٹری + Prometheus کاؤنٹرز) ، `docs/examples/` میں برآمد ہونے والے شواہد کے ساتھ روٹ ٹریس انسٹرومینٹیشن (`nexus.audit.outcome` ٹیلی میٹری + Prometheus کاؤنٹرز) کے ساتھ چلایا گیا تھا۔

## طریقہ کار

1. ** ٹیلی میٹری کا مجموعہ۔ ** تمام نوڈس ساختہ واقعہ `nexus.audit.outcome` اور اس کے ساتھ والے میٹرکس (`nexus_audit_outcome_total*`) کو نشر کرتے ہیں۔ `scripts/telemetry/check_nexus_audit_outcome.py` کے مددگار نے JSON لاگ کو دم کیا ، ایونٹ کی حیثیت کی توثیق کی اور `docs/examples/nexus_audit_outcomes/` میں پے لوڈ دائر کیا۔ .
2. ** الرٹ کی توثیق۔ CI ہر تبدیلی پر `dashboards/alerts/tests/nexus_audit_rules.test.yml` چلاتا ہے۔ ہر ونڈو کے دوران ایک ہی قواعد کو دستی طور پر استعمال کیا گیا تھا۔
3. ** ڈیش بورڈ کیپچر۔
4. ** جائزہ لینے والے نوٹ۔

## نتائج| ٹریس ID | نتیجہ | ثبوت | نوٹ |
| ---------- | --------- | ---------- | ---- |
| `TRACE-LANE-ROUTING` | پاس | فائر/بازیافت الرٹ اسکرین شاٹس (داخلی لنک) + `dashboards/alerts/tests/soranet_lane_rules.test.yml` کا ری پلے ؛ [Nexus منتقلی نوٹ] (./nexus-transition-notes#quarterly-routed-trace-audit-schedule) میں ٹیلی میٹری میں فرق ریکارڈ کیا گیا ہے۔ | P95 دم کا داخلہ 612 ایم ایس (ہدف <= 750 ایم ایس) پر رہا۔ کسی ٹریکنگ کی ضرورت نہیں ہے۔ |
| `TRACE-TELEMETRY-BRIDGE` | پاس | محفوظ شدہ پے لوڈ `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` کے علاوہ `status.md` میں رجسٹرڈ OTLP ری پلے ہیش۔ | ایس ڈی کے تحریری بریک زنگ کی بنیاد سے مماثل ہے۔ ڈف بوٹ نے صفر ڈیلٹا کی اطلاع دی۔ |
| `TRACE-CONFIG-DELTA` | پاس (تخفیف بند) | گورننس ٹریکر میں داخلہ (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`) + TLS پروفائل مینی فیسٹ (`artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`) + ٹیلی میٹری پیکیج مینی فیسٹ (`artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`)۔ | کیو 2 ریرون میں ٹی ایل ایس پروفائل پاس ہوا ہے اور اس کی تصدیق صفر اسٹراگلرز ہے۔ ٹیلی میٹری مینی فیسٹ میں سلاٹ 912-936 اور کام کے بوجھ کے بیج `NEXUS-REH-2026Q2` کی حد ریکارڈ کی گئی ہے۔ |

تمام نشانات نے اپنی ونڈوز میں کم از کم ایک `nexus.audit.outcome` ایونٹ تیار کیا ، جو الرٹ مینجر گارڈرییلز (`NexusAuditOutcomeFailure` سہ ماہی کے دوران سبز رہے) کو مطمئن کرتے ہیں۔

## فالو اپ

- TLS ہیش `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` کے ساتھ روٹ ٹریس اسٹب کو اپ ڈیٹ کیا۔ `NEXUS-421` تخفیف منتقلی کے نوٹوں میں بند کردی گئی تھی۔
- Android and4/and7 نظرثانی کے لئے برابری کے ثبوت کو مستحکم کرنے کے لئے فائل میں خام OTLP ری پلے اور Torii مختلف نمونے کو منسلک کرنا جاری رکھیں۔
- اس بات کی تصدیق کریں کہ اگلی `TRACE-MULTILANE-CANARY` ریہرسل ایک ہی ٹیلی میٹری مددگار کو دوبارہ استعمال کرتی ہے تاکہ Q2 سائن آف درست بہاؤ سے فائدہ اٹھائے۔

## آرٹیکٹیکٹ انڈیکس

| فعال | مقام |
| ------- | ------------ |
| ٹیلی میٹری کی توثیق کرنے والا | `scripts/telemetry/check_nexus_audit_outcome.py` |
| الرٹ قواعد اور ٹیسٹ | `dashboards/alerts/nexus_audit_rules.yml` ، `dashboards/alerts/tests/nexus_audit_rules.test.yml` |
| مثال کے طور پر پے لوڈ | `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` |
| کنفیگریشن ڈیلٹا ٹریکر | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` |
| شیڈول اور روٹ ٹریس نوٹ | [Nexus منتقلی نوٹس] (./nexus-transition-notes) |

اس رپورٹ ، پچھلی نمونے ، اور الرٹ/ٹیلی میٹری برآمدات کو گورننس کے فیصلے کے ریکارڈ کے ساتھ منسلک ہونا چاہئے تاکہ سہ ماہی کے B1 کو بند کیا جاسکے۔