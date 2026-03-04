---
lang: ur
direction: rtl
source: docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: گٹھ جوڑ-روٹڈ ٹریس-آڈٹ -2026Q1
عنوان: روٹ ٹریس آڈٹ رپورٹ 2026 Q1 (B1)
تفصیل: ٹیلی میٹری جائزوں کے سہ ماہی نتائج کا احاطہ کرتے ہوئے ، `docs/source/nexus_routed_trace_audit_report_2026q1.md` کا آئینہ۔
---
<!--
  SPDX-License-Identifier: Apache-2.0
-->

::: نوٹ کینونیکل ماخذ
یہ صفحہ `docs/source/nexus_routed_trace_audit_report_2026q1.md` کی عکاسی کرتا ہے۔ باقی ترجمے آنے تک دونوں کاپیاں منسلک رکھیں۔
:::

# روٹ ٹریس 2026 Q1 (B1) آڈٹ رپورٹ

روڈ میپ آئٹم ** B1-روٹ ٹریس آڈٹ اور ٹیلی میٹری بیس لائن ** کے لئے Nexus روٹ ٹریس پروگرام کا سہ ماہی جائزہ لینے کی ضرورت ہے۔ اس رپورٹ میں Q1 2026 (جنوری مارچ) آڈٹ ونڈو کو دستاویز کیا گیا ہے تاکہ گورننس بورڈ Q2 لانچ ٹرائلز سے قبل ٹیلی میٹری کرنسی کی منظوری دے سکے۔

## دائرہ کار اور شیڈول

| ٹریس ID | ونڈو (UTC) | مقصد |
| ---------- | ---------------- | ----------- |
| `TRACE-LANE-ROUTING` | 2026-02-17 09: 00-09: 45 | ملٹی لین کو چالو کرنے سے پہلے لین میں داخلہ ہسٹگرامس ، قطار گپ شپ اور الرٹ فلو چیک کریں۔ |
| `TRACE-TELEMETRY-BRIDGE` | 2026-02-24 10: 00-10: 45 | OTLP ری پلے ، BOT مختلف برابری ، اور SDK ٹیلی میٹری ingestion اور 4/and7 سنگ میل سے پہلے توثیق کریں۔ |
| `TRACE-CONFIG-DELTA` | 2026-03-01 12: 00-12: 30 | آر سی 1 کٹ آف سے پہلے گورننس سے منظور شدہ `iroha_config` ڈیلٹا اور رول بیک کی تیاری کی تصدیق کریں۔ |

ہر ٹیسٹ روٹ ٹریس انسٹرومنٹ (ٹیلی میٹری `nexus.audit.outcome` + کاؤنٹرز Prometheus) کے ساتھ پروڈکشن نما ٹوپولوجی میں چلتا ہے ، `docs/examples/` پر برآمد کردہ الرٹ مینجر قواعد اور شواہد برآمد ہوتے ہیں۔

## طریقہ کار

1. ** ٹیلی میٹری کا مجموعہ۔ ** تمام نوڈس نے ساختی واقعہ `nexus.audit.outcome` اور اس سے وابستہ میٹرکس (`nexus_audit_outcome_total*`) کو خارج کردیا۔ `scripts/telemetry/check_nexus_audit_outcome.py` کے مددگار نے JSON لاگ کو دم کیا ، ایونٹ کی حیثیت کی توثیق کی اور `docs/examples/nexus_audit_outcomes/` میں پے لوڈ کو محفوظ کیا۔ .
2. ** الرٹ کی توثیق۔ آئی سی ہر تبدیلی کے ساتھ `dashboards/alerts/tests/nexus_audit_rules.test.yml` پر عملدرآمد کرتا ہے۔ ہر ونڈو کے دوران ایک ہی قواعد کو دستی طور پر استعمال کیا گیا تھا۔
3. ** ڈیش بورڈ کیپچر۔
4. ** جائزہ نوٹ۔

## نتائج| ٹریس ID | نتیجہ | ثبوت | نوٹ |
| ---------- | --------- | ---------- | ------- |
| `TRACE-LANE-ROUTING` | پاس | فائر/بازیافت الرٹ کیپچرز (داخلی لنک) + `dashboards/alerts/tests/soranet_lane_rules.test.yml` کا ری پلے ؛ [Nexus منتقلی نوٹ] (./nexus-transition-notes#quarterly-routed-trace-audit-schedule) میں ٹیلی میٹری میں فرق ریکارڈ کیا گیا ہے۔ | قطار داخلہ P95 612 ایم ایس (ہدف <= 750 ایم ایس) پر رہا۔ کوئی فالو اپ نہیں۔ |
| `TRACE-TELEMETRY-BRIDGE` | پاس | پے لوڈ آرکائیوڈ `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` پلس OTLP ری پلے ہیش `status.md` پر ریکارڈ کیا گیا۔ | ایس ڈی کے کے ریڈیکشن نمکیات زنگ کی بنیاد سے مماثل ہیں۔ ڈف بوٹ نے صفر ڈیلٹا کی اطلاع دی۔ |
| `TRACE-CONFIG-DELTA` | پاس (تخفیف بند) | گورننس ٹریکر انٹری (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`) + TLS پروفائل مینی فیسٹ (`artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`) + ٹیلی میٹری پیکیج مینی فیسٹ (`artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`)۔ | کیو 2 نے منظور شدہ ٹی ایل ایس پروفائل اور تصدیق شدہ صفر اسٹراگلرز کو دوبارہ ہٹا دیا۔ ٹیلی میٹری مینی فیسٹ ریکارڈ سلاٹ رینج 912-936 اور ورک لوڈ بیج `NEXUS-REH-2026Q2`۔ |

تمام نشانات نے اپنے ونڈوز کے اندر کم از کم ایک `nexus.audit.outcome` ایونٹ تیار کیا ، جس سے الرٹ مینجر گارڈریلز (`NexusAuditOutcomeFailure` سہ ماہی کے لئے سبز رہے) کو مطمئن کرتے ہوئے۔

## فالو اپ

- روٹ ٹریس ضمیمہ کو TLS ہیش `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` کے ساتھ اپ ڈیٹ کیا گیا ہے۔ `NEXUS-421` تخفیف منتقلی کے نوٹوں میں بند کردی گئی تھی۔
- Android and4/and7 نظرثانی کے لئے برابری کے ثبوت کو مستحکم کرنے کے لئے فائل میں خام OTLP ری پلے اور Torii مختلف نمونے کو بڑھانا جاری رکھیں۔
- اس بات کی تصدیق کریں کہ اگلی ریہرسلز `TRACE-MULTILANE-CANARY` ایک ہی ٹیلی میٹری مددگار کو دوبارہ استعمال کریں تاکہ Q2 سائن آف کو درست ورک فلو سے فائدہ ہو۔

## آرٹیکٹیکٹ انڈیکس

| فعال | مقام |
| ------- | ------------ |
| ٹیلی میٹری کی توثیق کرنے والا | `scripts/telemetry/check_nexus_audit_outcome.py` |
| الرٹ قواعد اور ٹیسٹ | `dashboards/alerts/nexus_audit_rules.yml` ، `dashboards/alerts/tests/nexus_audit_rules.test.yml` |
| مثال کے طور پر پے لوڈ | `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` |
| کنفیگریشن ڈیلٹا ٹریکر | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` |
| شیڈول اور نوٹ روٹ ٹریس | [Nexus منتقلی نوٹس] (./nexus-transition-notes) |

یہ رپورٹ ، مذکورہ بالا نمونے اور انتباہات/ٹیلی میٹری برآمدات کو گورننس کے فیصلے کے ساتھ منسلک کرنا ضروری ہے تاکہ سہ ماہی کے B1 کو بند کیا جاسکے۔