---
lang: ur
direction: rtl
source: docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: گٹھ جوڑ-روٹڈ ٹریس-آڈٹ -2026Q1
عنوان: Q1 2026 (B1) کے لئے روٹ ٹریس آڈٹ رپورٹ
تفصیل: آئینہ `docs/source/nexus_routed_trace_audit_report_2026q1.md` ، سہ ماہی ٹیلی میٹری ریہرسل کے نتائج کا احاطہ کرتے ہوئے۔
---
<!--
  SPDX-License-Identifier: Apache-2.0
-->

::: نوٹ کینونیکل ماخذ
یہ صفحہ `docs/source/nexus_routed_trace_audit_report_2026q1.md` کی عکاسی کرتا ہے۔ دونوں کاپیاں ہم آہنگ رکھیں جب تک کہ باقی ترجمے تیار نہ ہوں۔
:::

# Q1 2026 (B1) کے لئے روٹ ٹریس آڈٹ رپورٹ

روڈ میپ آئٹم ** B1-روٹ ٹریس آڈٹ اور ٹیلی میٹری بیس لائن ** کے لئے روٹ ٹریس پروگرام Nexus کا سہ ماہی جائزہ لینے کی ضرورت ہے۔ اس رپورٹ میں Q1 2026 آڈٹ ونڈو (جنوری مارچ) کو اپنی گرفت میں لے لیا گیا ہے تاکہ مینجمنٹ بورڈ Q2 لانچ ریہرسل سے قبل ٹیلی میٹری کی تیاری کو منظور کر سکے۔

## ایریا اور ٹائم لائن

| ٹریس ID | ونڈو (UTC) | مقصد |
| ---------- | -------------- | ---------- |
| `TRACE-LANE-ROUTING` | 2026-02-17 09: 00-09: 45 | ملٹی لین کو چالو کرنے سے پہلے لین میں داخلہ ہسٹگرامس ، گپ شپ قطاریں ، اور الرٹ فلو چیک کریں۔ |
| `TRACE-TELEMETRY-BRIDGE` | 2026-02-24 10: 00-10: 45 | OTLP ری پلے ، مختلف BOT برابری اور SDK ٹیلی میٹری کا استقبال اور 4/and7 سنگ میل تک کی توثیق کریں۔ |
| `TRACE-CONFIG-DELTA` | 2026-03-01 12: 00-12: 30 | RC1 کاٹنے سے پہلے رول بیک کے لئے گورننس سے منظور شدہ ڈیلٹا `iroha_config` اور رول بیک کے لئے تیاری کی تصدیق کریں۔ |

ہر ریہرسل روٹ ٹریس انسٹرومنٹ (`nexus.audit.outcome` ٹیلی میٹری + Prometheus کاؤنٹرز) کے ساتھ پروڈکشن نما ٹوپولوجی پر ہوتی ہے ، جس میں لوڈ ، الرٹ مینجر قواعد اور `docs/examples/` پر ثبوت کی برآمد ہوتی ہے۔

## طریقہ کار

1. ** ٹیلی میٹری کا مجموعہ۔ ** تمام نوڈس نے ساختی واقعہ `nexus.audit.outcome` اور اس سے وابستہ میٹرکس (`nexus_audit_outcome_total*`) کو خارج کردیا۔ `scripts/telemetry/check_nexus_audit_outcome.py` مددگار نے JSON لاگ کی نگرانی کی ، ایونٹ کی حیثیت کی توثیق کی اور `docs/examples/nexus_audit_outcomes/` میں پے لوڈ کو محفوظ کیا۔ .
2. ** انتباہات کی جانچ پڑتال۔ CI ہر تبدیلی پر `dashboards/alerts/tests/nexus_audit_rules.test.yml` چلاتا ہے۔ ہر ونڈو میں ایک ہی اصول دستی طور پر چلائے گئے تھے۔
3. ** ڈیش بورڈ پر قبضہ کریں۔
4. ** جائزہ نگاروں کے نوٹ۔

## نتائج| ٹریس ID | خلاصہ | ثبوت | نوٹ |
| ---------- | --------- | ---------- | ------- |
| `TRACE-LANE-ROUTING` | پاس | فائر/بازیافت الرٹس (اندرونی لنک) کے اسکرین شاٹس + ری پلے `dashboards/alerts/tests/soranet_lane_rules.test.yml` ؛ ٹیلی میٹری کے فرق [Nexus منتقلی نوٹ] (./nexus-transition-notes#quarterly-routed-trace-audit-schedule) میں طے کیے گئے ہیں۔ | P95 وصول قطار 612 ایم ایس (ہدف <= 750 ایم ایس) رہا۔ مزید کارروائی کی ضرورت نہیں ہے۔ |
| `TRACE-TELEMETRY-BRIDGE` | پاس | محفوظ شدہ پے لوڈ `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` پلس OTLP ری پلے ہیش `status.md` میں ریکارڈ کیا گیا۔ | ایس ڈی کے ریڈیکشن نمکیات نے مورچا بیس لائن سے مماثل کیا۔ ڈف بوٹ نے زیرو ڈیلٹا کی اطلاع دی۔ |
| `TRACE-CONFIG-DELTA` | پاس (تخفیف بند) | گورننس ٹریکر ریکارڈ (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`) + TLS پروفائل مینی فیسٹ (`artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`) + ٹیلی میٹری پیک مینی فیسٹ (`artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`)۔ | Q2 دوبارہ ٹی ایل ایس سے منظور شدہ پروفائل کو ہٹا دیا اور تصدیق کی کہ کوئی اسٹراگلر موجود نہیں ہے۔ ٹیلی میٹری مینی فیسٹ سلاٹ رینج 912-936 اور ورک لوڈ بیج `NEXUS-REH-2026Q2` کو ٹھیک کرتی ہے۔ |

ونڈوز کے اندر کم از کم ایک ایونٹ `nexus.audit.outcome` تیار کیا ، جس نے مطمئن گارڈ ریلس الرٹ مینجر (`NexusAuditOutcomeFailure` پوری سہ ماہی میں سبز رہا)۔

## فالو اپ

- TLS ہیش `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` کے ساتھ اپ ڈیٹ شدہ روٹ ٹریس ایڈیشن ؛ تخفیف `NEXUS-421` منتقلی کے نوٹوں میں بند ہے۔
- Android and4/and7 چیکوں کے لئے برابری کے ثبوت کو مستحکم کرنے کے لئے آرکائیو میں خام OTLP ری پلے اور Torii مختلف نمونے شامل کرنا جاری رکھیں۔
- اس بات کو یقینی بنائیں کہ آئندہ `TRACE-MULTILANE-CANARY` ریہرسل ایک ہی ٹیلی میٹری مددگار استعمال کریں تاکہ Q2 سائن آف ثابت شدہ ورک فلو پر انحصار کرے۔

## آرٹیکٹیکٹ انڈیکس

| اثاثہ | مقام |
| ------- | ------------ |
| ٹیلی میٹری کی توثیق کرنے والا | `scripts/telemetry/check_nexus_audit_outcome.py` |
| الرٹ قواعد اور ٹیسٹ | `dashboards/alerts/nexus_audit_rules.yml` ، `dashboards/alerts/tests/nexus_audit_rules.test.yml` |
| مثال کے طور پر پے لوڈ | `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` |
| کنفگ ڈیلٹا ٹریکر | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` |
| روٹ ٹریس گراف اور نوٹ | [Nexus منتقلی نوٹس] (./nexus-transition-notes) |

یہ رپورٹ ، مذکورہ بالا نمونے ، اور الرٹ/ٹیلی میٹری برآمدات کو گورننس کے فیصلے کے ساتھ منسلک کیا جانا چاہئے تاکہ سہ ماہی کے لئے B1 بند کردیں۔