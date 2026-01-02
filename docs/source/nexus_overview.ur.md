<!--
  SPDX-License-Identifier: Apache-2.0
-->

---
lang: ur
direction: rtl
source: docs/source/nexus_overview.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: bda1352ff13cc866cd02a08f9db6be962798b547e905f2fccf236cd803eb0eda
source_last_modified: "2025-11-08T16:26:32.878050+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- اردو ترجمہ برائے docs/source/nexus_overview.md -->

# Nexus کا جائزہ اور آپریٹر سیاق و سباق

**روڈمیپ لنک:** NX-14 — Nexus دستاویزات اور آپریٹر رن بکس  
**اسٹیٹس:** مسودہ 2026-03-24 (`docs/source/nexus_operations.md` کے ساتھ جوڑا ہوا)  
**ہدف سامعین:** پروگرام مینیجرز، آپریشنز انجینئرز، اور پارٹنر ٹیمیں جنہیں
Sora Nexus (Iroha 3) آرکیٹیکچر کا ایک صفحے کا خلاصہ درکار ہے، تفصیلی
اسپیسفیکیشنز (`docs/source/nexus.md`, `docs/source/nexus_lanes.md`,
`docs/source/nexus_transition_notes.md`) میں جانے سے پہلے۔

## 1. ریلیز لائنیں اور مشترک ٹولنگ

- **Iroha 2** کنسورشیم ڈپلائمنٹس کے لئے خود میزبان ٹریک برقرار رکھتا ہے۔
- **Iroha 3 / Sora Nexus** ملٹی لین اجرا، ڈیٹا اسپیسز اور مشترک گورننس متعارف کراتا ہے۔
  ایک ہی ریپو، ٹول چین اور CI پائپ لائنز دونوں ریلیز لائنیں بناتی ہیں، اس لئے IVM،
  Kotodama کمپائلر یا SDKs کی درستگیاں خود بخود Nexus پر لاگو ہوتی ہیں۔
- **آرٹیفیکٹس:** `iroha3-<version>-<os>.tar.zst` بنڈلز اور OCI امیجز میں بائنریز،
  نمونہ کنفیگز اور Nexus پروفائل میٹا ڈیٹا شامل ہوتے ہیں۔ آپریٹرز
  `docs/source/sora_nexus_operator_onboarding.md` کو اینڈ ٹو اینڈ
  آرٹیفیکٹ ویلیڈیشن فلو کے لئے دیکھتے ہیں۔
- **مشترک SDK سطح:** Rust، Python، JS/TS، Swift اور Android SDKs ایک ہی Norito
  اسکیمائیں اور ایڈریس فکسچرز (`fixtures/account/address_vectors.json`) استعمال کرتے ہیں
  تاکہ والٹس اور آٹومیشن Iroha 2 اور Nexus نیٹ ورکس کے درمیان فارمیٹ فورکس کے بغیر
  سوئچ کر سکیں۔

## 2. معمارانہ بلڈنگ بلاکس

| جزو | وضاحت | اہم حوالہ جات |
|-----|-------|---------------|
| **Data Space (DS)** | گورننس کے دائرہ کار میں آنے والا اجرائی ڈومین جو ویلیڈیٹر ممبرشپ، پرائیویسی کلاس، فیس پالیسی اور ڈیٹا ایویلیبیلٹی پروفائل متعین کرتا ہے۔ ہر DS ایک یا زیادہ lanes کا مالک ہوتا ہے۔ | `docs/source/nexus.md`, `docs/source/nexus_transition_notes.md` |
| **Lane** | اجرا اور اسٹیٹ کی تعیین شدہ شارد۔ lane منیفیسٹس ویلیڈیٹر سیٹس، سیٹلمنٹ ہکس، ٹیلیمیٹری میٹا ڈیٹا اور روٹنگ پرمشنز بیان کرتے ہیں۔ گلوبل کنسینسس رنگ lane commitments کو ترتیب دیتا ہے۔ | `docs/source/nexus_lanes.md` |
| **Space Directory** | رجسٹری کنٹریکٹ (اور CLI ہیلپرز) جو DS منیفیسٹس، ویلیڈیٹر روٹیشنز اور کیپیبیلٹی گرانٹس کو محفوظ کرتا ہے۔ دستخط شدہ تاریخی منیفیسٹس رکھتا ہے تاکہ آڈٹرز اسٹیٹ دوبارہ بنا سکیں۔ | `docs/source/nexus.md#space-directory` |
| **Lane Catalog** | کنفیگریشن سیکشن (`[nexus]` میں `config.toml`) جو lane IDs کو ایلیسز، روٹنگ پالیسیز اور ریٹینشن نابس کے ساتھ میپ کرتا ہے۔ آپریٹرز `irohad --sora --config ... --trace-config` کے ذریعے مؤثر کیٹلاگ دیکھ سکتے ہیں۔ | `docs/source/sora_nexus_operator_onboarding.md` |
| **Settlement Router** | lanes کے درمیان XOR موومنٹس روٹ کرتا ہے (مثلا پرائیویٹ CBDC lanes <-> پبلک لیکویڈیٹی lanes)۔ ڈیفالٹ پالیسیاں `docs/source/cbdc_lane_playbook.md` میں ہیں۔ | `docs/source/cbdc_lane_playbook.md` |
| **Telemetry & SLOs** | `dashboards/grafana/nexus_*.json` کے تحت ڈیش بورڈز اور الرٹ رولز lane height، DA backlog، settlement latency اور گورننس کیو ڈیپتھ کو ٹریک کرتے ہیں۔ ریمیڈی ایشن پلان `docs/source/nexus_telemetry_remediation_plan.md` میں ہے۔ | `dashboards/grafana/nexus_lanes.json`, `dashboards/alerts/nexus_audit_rules.yml` |

### lane اور data space کلاسز

- `default_public` lanes Sora پارلیمنٹ کے تحت مکمل طور پر پبلک ورک لوڈز کو اینکر کرتے ہیں۔
- `public_custom` lanes پروگرام مخصوص اکنامکس کو اجازت دیتے ہیں مگر شفاف رہتے ہیں۔
- `private_permissioned` lanes CBDCs یا کنسورشیم ایپس کو سپورٹ کرتے ہیں؛ صرف commitments اور proofs ایکسپورٹ کرتے ہیں۔
- `hybrid_confidential` lanes زیرو نالج پروفز کو سلیکٹو ڈسکلوژر ہکس کے ساتھ جوڑتے ہیں۔

ہر lane درج ذیل اعلان کرتا ہے:

1. **lane منیفیسٹ:** گورننس سے منظور شدہ میٹا ڈیٹا جو Space Directory میں ٹریک ہوتا ہے۔
2. **ڈیٹا ایویلیبیلٹی پالیسی:** erasure coding پیرامیٹرز، ریکوری ہکس، اور آڈٹ تقاضے۔
3. **ٹیلیمیٹری پروفائل:** ڈیش بورڈز اور آن کال رن بکس جنہیں ہر گورننس تبدیلی پر اپڈیٹ کرنا ضروری ہے۔

## 3. رول آؤٹ ٹائم لائن اسنیپ شاٹ

| مرحلہ | فوکس | اخراج کے معیار |
|-------|------|----------------|
| **N0 - Closed beta** | کونسل مینیجڈ رجسٹرار، صرف `.sora` نیم اسپیس، دستی آپریٹر آن بورڈنگ۔ | DS منیفیسٹس سائنڈ، lane کیٹلاگ اسٹیٹک، گورننس ریہرسل لاگڈ۔ |
| **N1 - Public launch** | `.nexus` سفکسز، آکشنز اور سیلف سروس رجسٹرار شامل ہوتے ہیں۔ سیٹلمنٹس XOR ٹریژری سے جڑتے ہیں۔ | resolver/gateway سنک ٹیسٹس گرین، بلنگ ریکنسلی ایشن ڈیش بورڈز لائیو، ڈسپیوٹ ٹیبل ٹاپ مکمل۔ |
| **N2 - Expansion** | `.dao`، reseller APIs، اینالٹکس، ڈسپیوٹ پورٹل، اسٹیورڈ اسکورکارڈز فعال۔ | کمپلائنس آرٹیفیکٹس ورژنڈ، policy-jury ٹول کٹ لائیو، ٹریژری ٹرانسپیرنسی رپورٹس شائع۔ |
| **NX-12/13/14 gate** | کمپلائنس انجن، ٹیلیمیٹری ڈیش بورڈز، اور ڈاکیومنٹیشن کو پارٹنر پائلٹ کھولنے سے پہلے ایک ساتھ آنا ہوگا۔ | `docs/source/nexus_overview.md` + `docs/source/nexus_operations.md` شائع، ڈیش بورڈز الرٹس کے ساتھ وائرڈ، پالیسی انجن گورننس سے جڑا ہوا۔ |

## 4. آپریٹر ذمہ داریاں

| ذمہ داری | وضاحت | ثبوت |
|----------|-------|------|
| کنفیگ ہائیجین | `config/config.toml` کو شائع شدہ lane اور dataspace کیٹلاگ کے ساتھ ہم آہنگ رکھیں؛ تبدیلیاں ٹکٹس میں درج کریں۔ | `irohad --sora --config ... --trace-config` آؤٹ پٹ ریلیز آرٹیفیکٹس کے ساتھ محفوظ۔ |
| منیفیسٹ ٹریکنگ | Space Directory اپڈیٹس پر نظر رکھیں اور لوکل کیش/الاؤ لسٹس ریفریش کریں۔ | سائنڈ منیفیسٹ بنڈل آن بورڈنگ ٹکٹ کے ساتھ محفوظ۔ |
| ٹیلیمیٹری کوریج | سیکشن 2 میں درج ڈیش بورڈز تک رسائی یقینی بنائیں، الرٹس PagerDuty سے جوڑیں، اور سہ ماہی ریویوز لاگ کریں۔ | آن کال ریویو منٹس + Alertmanager ایکسپورٹ۔ |
| انسیڈنٹ رپورٹنگ | `docs/source/nexus_operations.md` میں دی گئی سیورٹی میٹرکس فالو کریں اور پانچ کاروباری دنوں میں پوسٹ انسیڈنٹ رپورٹ جمع کریں۔ | ہر انسیڈنٹ ID کے لئے ٹمپلٹ محفوظ۔ |
| گورننس ریڈینس | جب lane پالیسی تبدیلیاں ڈپلائمنٹ کو متاثر کریں تو Nexus کونسل ووٹس میں حصہ لیں؛ سہ ماہی رول بیک ہدایات کی ریہرسل کریں۔ | کونسل اٹنڈنس + ریہرسل چیک لسٹ `docs/source/project_tracker/nexus_config_deltas/` کے تحت۔ |

## 5. متعلقہ دستاویزات کا نقشہ

- **تفصیلی اسپیسفکیشن:** `docs/source/nexus.md`
- **lane جیومیٹری اور اسٹوریج:** `docs/source/nexus_lanes.md`
- **ٹرانزیشن پلان اور عارضی روٹنگ:** `docs/source/nexus_transition_notes.md`
- **آپریٹر آن بورڈنگ واک تھرو:** `docs/source/sora_nexus_operator_onboarding.md`
- **CBDC lane پالیسی اور سیٹلمنٹ پلان:** `docs/source/cbdc_lane_playbook.md`
- **ٹیلیمیٹری ریمیڈی ایشن اور ڈیش بورڈ میپ:** `docs/source/nexus_telemetry_remediation_plan.md`
- **رن بک / انسیڈنٹ پراسس:** `docs/source/nexus_operations.md`

اس جائزے کو NX-14 روڈمیپ آئٹم کے ساتھ ہم آہنگ رکھیں جب لنک شدہ دستاویزات میں
بڑی تبدیلیاں آئیں یا نئی lane کلاسز یا گورننس فلو متعارف ہوں۔

</div>
