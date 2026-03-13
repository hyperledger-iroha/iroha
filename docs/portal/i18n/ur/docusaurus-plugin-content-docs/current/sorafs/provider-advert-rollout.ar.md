---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/provider-advert-rollout.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
عنوان: "SoraFS فراہم کنندگان کے لئے رول آؤٹ اور مطابقت کا منصوبہ اشتہار دیتا ہے"
---

> [`docs/source/sorafs/provider_advert_rollout.md`] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_rollout.md) سے موافقت پذیر۔

# SoraFS فراہم کرنے والوں کے لئے رول آؤٹ اور مطابقت کا منصوبہ اشتہار دیتا ہے

یہ منصوبہ اسٹوریج فراہم کرنے والوں کے لئے `ProviderAdvertV1` سطح پر جانے والے اشتہارات سے منتقلی کو مربوط کرتا ہے۔
مکمل طور پر حکمرانی اور ضروری حصوں کی کثیر سورس بازیافت۔ اس پر مرکوز ہے
تین اہم نتائج:

- ** آپریٹر کا دستی۔ ** اقدامات جو اسٹوریج فراہم کرنے والوں کو ہر گیٹ کو چالو کرنے سے پہلے مکمل کرنا چاہئے۔
- ** ٹیلی میٹرک کوریج۔
  اس بات کو یقینی بنانے کے لئے کہ نیٹ ورک صرف ہم آہنگ اشتہارات کو قبول کرتا ہے۔
- ** مطابقت کی ٹائم لائن۔ ** پرانے لفافوں کو مسترد کرنے کی واضح تاریخیں تاکہ آپ کر سکیں
  ایس ڈی کے اور ٹولنگ ٹیمیں اپنی ریلیز کی منصوبہ بندی کرتی ہیں۔

پیش کش SF-2B/2C کے پیرامیٹرز کے مطابق ہے
[SoraFS] (./migration-roadmap) امیگریشن روڈ میپ یہ فرض کیا جاتا ہے کہ داخلہ پالیسی
[فراہم کنندہ داخلہ پالیسی] (./provider-admission-policy) پہلے ہی نافذ العمل ہے۔

## مراحل کا ٹائم ٹیبل| اسٹیج | ونڈو (ہدف) | سلوک | آپریٹر کے اعمال | مشاہدے کی توجہ
| ---------------------- | -------------------------------------------------------------------- | --------------------- |
| ** R0 - بنیادی مشاہدہ ** | جب تک ** 2025-03-31 ** | Torii `ProviderAdvertV1` سے پہلے گورننس سے منظور شدہ اشتہارات اور میراثی پے لوڈ دونوں کو قبول کرتا ہے۔ جب معیاری اشتہارات `chunk_range_fetch` یا `profile_aliases` کو نظرانداز کیا جاتا ہے تو اس وقت کی فہرست میں انتباہات ریکارڈ ہوتے ہیں۔ | - گارنٹیڈ `profile_id=sorafs.sf1@1.0.0` ، معیاری `profile_aliases` اور `signature_strict=true` کے ساتھ فراہم کنندہ کے اشتہار کی تعیناتی پائپ لائن (پرووائڈرڈورٹ وی 1 + گورننس لفافہ) کے ذریعے دوبارہ تخلیق کریں۔ - `sorafs_fetch` ٹیسٹ مقامی طور پر چلائیں۔ آپ کو نامعلوم صلاحیتوں کے لئے انتباہات کو تکلیف دینا چاہئے۔ | عارضی Grafana پینل متعین کریں (نیچے ملاحظہ کریں) اور انتباہی حد کو مقرر کریں جبکہ انہیں صرف انتباہی موڈ میں رکھتے ہوئے۔ |
| ** R1 - انتباہی گیٹ ** | ** 2025-04-01 → 2025-05-15 ** | Torii میراثی اشتہارات کو قبول کرنا جاری رکھے ہوئے ہے لیکن جب پے لوڈ `chunk_range_fetch` کے بغیر `chunk_range_fetch` میں `torii_sorafs_admission_total{result="warn"}` میں اضافہ ہوتا ہے یا `allow_unknown_capabilities=true` کے بغیر نامعلوم صلاحیتیں ہیں۔ ٹولنگ سی ایل آئی اب جب تک معیاری ہینڈل موجود نہ ہو تب تک دوبارہ تخلیق کرنے میں ناکام رہتا ہے۔ | - پے لوڈ `CapabilityType::ChunkRangeFetch` کو شامل کرنے کے لئے اسٹیجنگ اور پروڈکشن میں اشتہارات کو گھمائیں ، اور جب چکنائی کی جانچ `allow_unknown_capabilities=true` سیٹ کریں۔ - رن بکس میں ٹیلی میٹری کے لئے نئے سوالات کی دستاویزات۔ | آن کال گردش میں ڈیش بورڈز کو اپ گریڈ کریں۔ جب `warn` واقعات 15 منٹ کے لئے 5 ٪ ٹریفک سے تجاوز کریں تو انتباہات مرتب کریں۔ |
| ** R2 - نفاذ ** | ** 2025-05-16 → 2025-06-30 ** | Torii ان اشتہارات کو مسترد کرتا ہے جن میں گورننس لفافے ، معیاری پروفائل ہینڈل ، یا `chunk_range_fetch` صلاحیت کی کمی ہے۔ پرانے ہینڈلز `namespace-name` اب حل نہیں ہوئے ہیں۔ `reason="unknown_capability"` کی وجہ سے نامعلوم صلاحیتیں چکنائی آپٹ ان کے بغیر ناکام ہوجاتی ہیں۔ | - اس بات کو یقینی بنائیں کہ پروڈکشن لفافے `torii.sorafs.admission_envelopes_dir` کے تحت ہیں اور باقی کسی بھی پرانے اشتہارات کو گھمائیں۔ - تصدیق کریں کہ SDKs صرف پیچھے کی مطابقت کے لئے اختیاری عرفیت کے ساتھ معیاری ہینڈلز برآمد کرتے ہیں۔ | پیجر الرٹس چل رہا ہے: 5 منٹ کے لئے `torii_sorafs_admission_total{result="reject"}`> 0 آپریٹر کی مداخلت کی ضرورت ہے۔ قبولیت کی شرح اور قبولیت کی وجوہات کے ہسٹگرام کو ٹریک کریں۔ |
| ** R3 - آف پرانا ** | ** 2025-07-01 تک ** | دریافت بائنری اشتہارات کے لئے حمایت ترک کر رہی ہے جو `signature_strict=true` کو متعین نہیں کرتی ہے یا اس میں `profile_aliases` غائب ہے۔ Torii دریافت کیشے پرانے اندراجات کو حذف کردیتے ہیں جو تازہ کاری کے بغیر تجدید کی آخری تاریخ پاس کردی ہیں۔ | - لیگیسی فراہم کرنے والے اسٹیکس کے لئے آخری ڈیکمیشن ونڈو کا شیڈول بنانا۔ - اس بات کو یقینی بنائیں کہ چکنائی `--allow-unknown` آپریشن صرف کنٹرول شدہ مشقوں کے دوران کیئے جاتے ہیں اور ریکارڈ کیے جاتے ہیں۔ - ریلیز سے پہلے بلاکر کے طور پر `sorafs_fetch` انتباہی آؤٹ پٹ پر غور کرنے کے لئے کریش پلے بوکس کو تازہ ترین۔ | سخت انتباہات: کوئی بھی نتیجہ `warn` آن-کال پر الرٹ کرتا ہے۔ مصنوعی چیک شامل کیا گیا ہے جو دریافت JSON اور فراہم کنندگان کی صلاحیتوں کی فہرستوں کو چیک کرتے ہیں۔ |

## آپریٹر چیک لسٹ1. ** اشتہارات کی انوینٹری۔ ** شائع شدہ ہر اشتہار کا اسٹاک لیں اور ریکارڈ کریں:
   - گورننگ لفافے کا راستہ (`defaults/nexus/sorafs_admission/...` یا اس کے مساوی پیداوار میں)۔
   - اشتہار کے لئے `profile_id` اور `profile_aliases`۔
   - صلاحیتوں کی فہرست (کم از کم `torii_gateway` اور `chunk_range_fetch` کی توقع کریں)۔
   -`allow_unknown_capabilities` پرچم (جب وینڈر سے محفوظ TLVs موجود ہو تو ضروری ہے)۔
2. ** سپلائی شدہ ٹولنگ کا استعمال کرتے ہوئے تخلیق نو۔ **
   - فراہم کنندہ کے اشتہار کے ذریعہ پے لوڈ کو دوبارہ تعمیر کریں ، اس بات کو یقینی بنائیں:
     -`profile_id=sorafs.sf1@1.0.0`
     - `capability=chunk_range_fetch` `max_span` سلیکٹر کے ساتھ
     - `allow_unknown_capabilities=<true|false>` جب چکنائی TLVs موجود ہوں
   - `/v2/sorafs/providers` اور `sorafs_fetch` کے ذریعے چیک کریں۔ انتباہات تکلیف ہونی چاہئیں
     نامعلوم صلاحیتیں۔
3. ** کثیر سورس تیاری چیک کریں۔ **
   - Grafana کے ساتھ `sorafs_fetch` پر عمل کریں ؛ CLI اب ناکام ہوجاتا ہے جب ...
     `chunk_range_fetch` جب نامعلوم صلاحیتوں کو نظرانداز کیا جاتا ہے تو انتباہات کو یاد کرتا ہے اور پرنٹ کرتا ہے۔
     JSON رپورٹ پر قبضہ کریں اور اسے لین دین کے نوشتہ جات کے ساتھ محفوظ کریں۔
4. ** تزئین و آرائش کی تیاری۔ **
   - کم از کم 30 دن پہلے قسم `ProviderAdmissionRenewalV1` کے لفافے بھیجیں
     گیٹ وے (R2) پر نفاذ۔ تجدیدات کو معیاری ہینڈل رکھنا چاہئے
     اور صلاحیتوں کا گروپ ؛ صرف داؤ ، اختتامی نکات ، یا میٹا ڈیٹا میں تبدیلی۔
5. ** منظور شدہ ٹیموں کے ساتھ بات چیت کریں۔ **
   - ایس ڈی کے مالکان کو ایسے ورژن جاری کرنا چاہئے جو اشتہارات کو مسترد کرتے وقت آپریٹرز کو انتباہات دکھاتے ہیں۔
   - ڈیوریل ہر مرحلے کی منتقلی کا اعلان کرتا ہے۔ میں نیچے ڈیش بورڈز اور تھریشولڈ منطق کے لنکس شامل کرتا ہوں۔
6. ** ڈیش بورڈز اور الرٹس انسٹال کریں۔ **
   - درآمد Grafana کو درآمد کریں اور اسے ** SoraFS / فراہم کنندہ رول آؤٹ ** کے تحت رکھیں
     `sorafs-provider-admission`۔
   - اس بات کو یقینی بنائیں کہ الرٹ کے قواعد مشترکہ چینل `sorafs-advert-rollout` کا حوالہ دیں
     اسٹیجنگ اور پروڈکشن میں۔

## ٹیلی میٹری اور انفارمیشن پینل

مندرجہ ذیل میٹرکس `iroha_telemetry` کے ذریعے پہلے ہی بے نقاب ہوچکے ہیں:

- `torii_sorafs_admission_total{result,reason}` - سیٹ قبول ، مسترد ، اور انتباہ کے نتائج۔
  وجوہات میں `missing_envelope` ، `unknown_capability` ، `stale` ، اور `policy_violation` شامل ہیں۔

Grafana برآمد کریں: [`docs/source/grafana_sorafs_admission.json`] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/grafana_sorafs_admission.json)۔
فائل کو مشترکہ ڈیش بورڈز ریپوزٹری (`observability/dashboards`) میں درآمد کریں
اشاعت سے پہلے ہی ڈیٹا سورس کے UID کو اپ ڈیٹ کریں۔

بورڈ Grafana ** SoraFS / فراہم کنندہ رول آؤٹ ** فولڈر کے تحت تعینات ہے۔
`sorafs-provider-admission`۔ `sorafs-admission-warn` (انتباہ) قواعد اور
`sorafs-admission-reject` (تنقیدی) نوٹیفکیشن پالیسی کو استعمال کرنے کے لئے پیش سیٹ ہے
`sorafs-advert-rollout` ؛ اگر منزل کی فہرست ترمیم کے بجائے تبدیل ہوگئی ہے تو رابطہ میں ترمیم کریں
بورڈ کے json.

تجویز کردہ Grafana بورڈ:

| پینٹنگ | استفسار | نوٹ |
| ------- | ------- | ------- |
| ** داخلے کے نتائج کی شرح ** | `sum by(result)(rate(torii_sorafs_admission_total[5m]))` | اسٹیکڈ چارٹ ڈسپلے کرنے والے بمقابلہ وارن بمقابلہ مسترد کریں۔ انتباہ کریں جب انتباہ> 0.05 * کل (انتباہ) یا مسترد> 0 (تنقیدی)۔ |
| ** انتباہی تناسب ** | `sum(rate(torii_sorafs_admission_total{result="warn"}[5m])) / sum(rate(torii_sorafs_admission_total[5m]))` | ایک ہی ٹائم سیریز پیجر کی حد (15 منٹ کے اندر اندر 5 ٪ انتباہی شرح) کو کھانا کھلاتی ہے۔ |
| ** مسترد ہونے کی وجوہات ** | `sum by(reason)(rate(torii_sorafs_admission_total{result="reject"}[5m]))` | رن بک میں ٹریج کی حمایت کرتا ہے۔ تخفیف کے اقدامات سے لنک منسلک کریں۔ |
| ** ریفریش قرض ** | `sum(rate(torii_sorafs_admission_total{reason="stale"}[1h]))` | ان فراہم کنندگان کی نشاندہی کرتا ہے جو تجدید کی آخری تاریخ سے محروم ہیں۔ دریافت کیشے کے نوشتہ جات کے ساتھ موازنہ کریں۔ |

دستی ڈیش بورڈز کے لئے سی ایل آئی کے نوادرات:- `sorafs_fetch --provider-metrics-out` کاؤنٹر لکھتا ہے `failures` اور `successes` اور
  `disabled` ہر فراہم کنندہ کے لئے۔ خشک رنز کی نگرانی کے لئے انہیں ڈیش بورڈز ایڈہاک میں درآمد کریں
  پروڈکشن فراہم کرنے والوں کو تبدیل کرنے سے پہلے آرکسٹریٹر۔
- JSON رپورٹ میں `chunk_retry_rate` اور `provider_failure_rate` فیلڈز نمایاں کریں
  تھروٹلنگ یا پے لوڈ باسی علامات جو اکثر داخلے سے انکار سے پہلے ہوتے ہیں۔

### Grafana بورڈ لے آؤٹ

مشاہدہ ایک کسٹم پینل کو شائع کرتا ہے - ** SoraFS فراہم کنندہ داخلہ
رول آؤٹ ** (`sorafs-provider-admission`) - کے تحت ** SoraFS / فراہم کنندہ رول آؤٹ **
مندرجہ ذیل معیاری پلیٹ شناخت کاروں کے ساتھ:

- پینل 1 - * داخلے کے نتائج کی شرح * (اسٹیکڈ ایریا ، یونٹ "اوپس/منٹ")۔
- پینل 2 - * انتباہی تناسب * (سنگل سیریز) ، اظہار کے ساتھ
  `رقم (شرح (torii_sorafs_admission_total {نتیجہ =" انتباہ "} [5m])) /
   رقم (شرح (torii_sorafs_admission_total [5m])) `.
- پینل 3 - * مسترد ہونے کی وجوہات * (ٹائم سیریز `reason` کے ذریعہ گروپ کردہ)
  `rate(...[5m])`۔
- پینل 4۔
  ہجرت لیجر سے نکالا گیا۔

انفراسٹرکچر بورڈز کے ذخیرے میں JSON کنکال کاپی (یا تخلیق کریں)
`observability/dashboards/sorafs_provider_admission.json` ، پھر صرف ماخذ کا UID ہوا
ڈیٹا ؛ پینل آئی ڈی اور الرٹ کے قواعد ذیل میں رن بوکس میں حوالہ دیا گیا ہے ، لہذا اس سے بچیں
اس دستاویز کو اپ ڈیٹ کیے بغیر تجدید شدہ۔

سہولت کے ل the ، ذخیرہ بورڈ میں ایک حوالہ تعریف فراہم کرتا ہے
`docs/source/grafana_sorafs_admission.json` ؛ اگر ضرورت ہو تو اسے Grafana فولڈر میں کاپی کریں
مقامی جانچ کے لئے نقطہ آغاز کے طور پر.

### انتباہ کے قواعد Prometheus

مندرجہ ذیل قواعد کا سیٹ شامل کریں
`observability/prometheus/sorafs_admission.rules.yml` (اگر یہ ہے تو فائل بنائیں
پہلا قاعدہ سیٹ SoraFS ہے) اور اس میں Prometheus کی ترتیبات میں شامل ہے۔ `<pagerduty>` کو تبدیل کریں
شفٹ کے لئے اصل سمت سائن کے ساتھ۔

```yaml
groups:
  - name: torii_sorafs_admission
    rules:
      - alert: SorafsProviderAdvertWarnFlood
        expr: sum(rate(torii_sorafs_admission_total{result="warn"}[5m])) /
              sum(rate(torii_sorafs_admission_total[5m])) > 0.05
        for: 15m
        labels:
          severity: warning
          route: <pagerduty>
        annotations:
          summary: "Grafana provider adverts generating warnings"
          description: |
            Warn outcomes exceeded 5% of all admissions for 15 minutes.
            Inspect panel 3 on the sorafs/provider-admission dashboard and
            coordinate advert rotation with the affected operator.
      - alert: SorafsProviderAdvertReject
        expr: increase(torii_sorafs_admission_total{result="reject"}[5m]) > 0
        for: 5m
        labels:
          severity: critical
          route: <pagerduty>
        annotations:
          summary: "Grafana provider adverts rejected"
          description: |
            Provider adverts have been rejected for the last five minutes.
            Check panel 4 (rejection reasons) and rotate envelopes before
            the refresh deadline elapses.
```

`scripts/check_prometheus_rules.sh observability/prometheus/sorafs_admission.rules.yml` کو آن کریں
تبدیلیوں کو اپ لوڈ کرنے سے پہلے یہ یقینی بنانے کے لئے کہ `promtool check rules` سے گزرتا ہے۔

## مطابقت میٹرکس

| اشتہاری خصوصیات | R0 | R1 | R2 | R3 |
| -------------------------- | ---- | ---- | ---- | ---- | ---- |
| `profile_id = sorafs.sf1@1.0.0` ، `chunk_range_fetch` موجود ، معیاری عرفی ، `signature_strict=true` | ✅ | ✅ | ✅ | ✅ |
| صلاحیت کی کمی `chunk_range_fetch` | ⚠ انتباہ (انجسٹ + ٹیلی میٹری) | ⚠ انتباہ | ❌ مسترد (`reason="missing_capability"`) | ❌ مسترد |
| `allow_unknown_capabilities=true` کے بغیر نامعلوم صلاحیت کے لئے TLVs | ✅ | ⚠ انتباہ (`reason="unknown_capability"`) | ❌ مسترد | ❌ مسترد |
| `refresh_deadline` ختم | ❌ مسترد | ❌ مسترد | ❌ مسترد | ❌ مسترد |
| `signature_strict=false` (تشخیصی فکسچر) | ✅ (صرف ترقی کے لئے) | ⚠ انتباہ | ⚠ انتباہ | ❌ مسترد |

ہر وقت UTC میں ہوتا ہے۔ نفاذ کی تاریخیں ہجرت کے لیجر میں الٹ ہیں اور تبدیل نہیں ہوں گی
کونسل کے ووٹ کے بغیر ؛ کسی بھی تبدیلی کے لئے اسی فائل اور لیجر کو اسی PR میں اپ ڈیٹ کرنے کی ضرورت ہوتی ہے۔

> ** ایگزیکٹو نوٹ: ** R1 `result="warn"` سیریز میں متعارف کراتا ہے
> `torii_sorafs_admission_total`۔ Torii میں ingest پیچ پر عمل کریں جو اس لیبل کو شامل کرتا ہے
> SF-2 ٹیلی میٹرک کاموں کے اندر ؛ اس کے بعد بھی ریکارڈ کے نمونے لینے کی نگرانی کے لئے استعمال کیا گیا تھا

## مواصلات اور واقعہ سے نمٹنے کے- ** ہفتہ وار اسٹیٹس میسج۔ ** ڈیوریل قبولیت کی پیمائش اور انتباہات کا ایک مختصر خلاصہ بھیجتا ہے
  اور آنے والی تاریخیں۔
- ** واقعہ کا جواب۔ ** اگر `reject` الارم کو متحرک کیا جاتا ہے تو ، آن کال مندرجہ ذیل کام کرتا ہے:
  1. Torii (`/v2/sorafs/providers`) پر دریافت کے ذریعے مجرم اشتہار لائیں۔
  2. دوبارہ فراہم کنندہ کی پائپ لائن میں اشتہار چیک کریں اور نتائج کا موازنہ کریں
     `/v2/sorafs/providers` غلطی کو دوبارہ پیش کرنے کے لئے۔
  3. اگلی ریفریش ڈیڈ لائن کے قریب آنے سے پہلے اشتہار کو دوبارہ شروع کرنے کے لئے فراہم کنندہ کے ساتھ رابطہ کریں۔
- ** منجمد تبدیلیاں۔ ** R1/R2 کے دوران صلاحیتوں کے لئے اسکیما میں کوئی تبدیلی نہیں جب تک کہ
  رول آؤٹ ٹیم کے ذریعہ منظور شدہ ؛ چکنائی کے ٹرائلز کو ہفتہ وار بحالی ونڈو میں شیڈول کیا جانا چاہئے
  اور اسے منتقلی لیجر میں رجسٹر کریں۔

## حوالہ جات

- [SoraFS نوڈ/کلائنٹ پروٹوکول] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/sorafs_node_client_protocol.md)
- [فراہم کنندہ داخلہ پالیسی] (./provider-admission-policy)
- [ہجرت روڈ میپ] (./migration-roadmap)
- [فراہم کنندہ ایڈورٹ ملٹی سورس ایکسٹینشن] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md)