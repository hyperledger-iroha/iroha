---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/provider-advert-rollout.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
عنوان: "فراہم کنندہ اشتہارات کی تعیناتی کا منصوبہ SoraFS"
---

> [`docs/source/sorafs/provider_advert_rollout.md`] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_rollout.md) سے موافقت پذیر۔

# فراہم کنندہ اشتہارات SoraFS کے لئے تعیناتی کا منصوبہ

یہ منصوبہ فراہم کنندگان سے سطح پر جائز اشتہارات کو تبدیل کرنے میں مربوط ہے
مکمل طور پر حکمرانی شدہ `ProviderAdvertV1` جس کی بازیابی کے لئے درکار ہے
ملٹی سورس۔ اس میں تین فراہمی پر توجہ دی گئی ہے:

-** آپریٹر گائیڈ۔ ** مرحلہ وار اقدامات جو اسٹوریج فراہم کرنے والوں کو لازمی ہے
  ہر گیٹ سے پہلے ختم کریں۔
- ** ٹیلی میٹری کوریج۔ ** مشاہدہ اور اوپس کے لئے ڈیش بورڈز اور الرٹس
  اس بات کی تصدیق کے لئے استعمال کریں کہ نیٹ ورک صرف تعمیل اشتہارات کو قبول کرتا ہے۔
- ** تعیناتی کا شیڈول۔ ** لفافوں کو مسترد کرنے کے لئے واضح تاریخیں

تعیناتی SF-2B/2C سنگ میل کے ساتھ منسلک ہے
[migration roadmap SoraFS](./migration-roadmap) and assumes that the policy
[فراہم کنندہ داخلہ پالیسی] (./provider-admission-policy) کا داخلہ پہلے ہی موجود ہے
زبردستی

## فیز ٹائم لائن

| مرحلہ | ونڈو (ہدف) | سلوک | آپریٹر کے اعمال | مشاہدہ پر توجہ مرکوز |
| ------- | ----------------- | ---------- | --------- | --------- |

## آپریٹر چیک لسٹ1. ** انوینٹری اشتہارات۔ ** شائع شدہ ہر اشتہار کی فہرست بنائیں اور ریکارڈ کریں:
   - گورننگ لفافے کا راستہ (`defaults/nexus/sorafs_admission/...` یا پیداوار میں مساوی)۔
   - `profile_id` اور اشتہار کا `profile_aliases`۔
   - صلاحیتوں کی فہرست (کم از کم `torii_gateway` اور `chunk_range_fetch`)۔
   - پرچم `allow_unknown_capabilities` (جب وینڈر سے محفوظ TLVs موجود ہوں تو ضروری ہے)۔
2. ** ٹولنگ فراہم کرنے والے کے ساتھ دوبارہ تخلیق کریں۔ **
   - اپنے فراہم کنندہ کے اشتہار کے ناشر کے ساتھ پے لوڈ کی تشکیل نو کریں ، اس بات کو یقینی بنائیں:
     - `profile_id=sorafs.sf1@1.0.0`
     - `capability=chunk_range_fetch` ایک متعین `max_span` کے ساتھ
     - `allow_unknown_capabilities=<true|false>` جب چکنائی TLVs موجود ہوں
   - `/v2/sorafs/providers` اور `sorafs_fetch` کے ذریعے توثیق کریں ؛ انتباہات پر
     نامعلوم صلاحیتوں کو ترتیب دینے کی ضرورت ہے۔
3. ** کثیر سورس تیاری کی توثیق کریں۔ **
   - `sorafs_fetch` کو `--provider-advert=<path>` کے ساتھ چلائیں ؛ CLI ناکام ہوجاتا ہے
     اب جب `chunk_range_fetch` غائب ہے اور اس کے لئے انتباہات دکھاتا ہے
     نامعلوم صلاحیتوں کو نظرانداز کیا گیا۔ JSON رپورٹ پر قبضہ کریں اور اس کے ساتھ محفوظ کریں
     آپریشن لاگ
4. ** تجدیدات تیار کریں۔ **
   - کم از کم 30 دن پہلے لفافے `ProviderAdmissionRenewalV1` جمع کروائیں
     انفورسمنٹ گیٹ وے (R2)۔ تجدیدات کو ہینڈل کو برقرار رکھنا چاہئے
     کیننیکل اور تمام صلاحیتیں۔ صرف داؤ ، اختتامی نکات یا
     میٹا ڈیٹا کو تبدیل ہونا چاہئے۔
5. ** منحصر ٹیموں کے ساتھ بات چیت کریں۔ **
   - ایس ڈی کے مالکان کو ایسے ورژن شائع کرنا ہوں گے جو انتباہات کو بے نقاب کریں
     آپریٹرز جب اشتہارات مسترد کردیئے جاتے ہیں۔
   - ڈیوریل ہر مرحلے کی منتقلی کا اعلان کرتا ہے۔ ڈیش بورڈ لنکس شامل کریں
     اور نیچے دہلیز منطق۔
6. ** ڈیش بورڈز اور الرٹس انسٹال کریں۔ **
   - برآمد Grafana درآمد کریں اور اسے ** SoraFS / فراہم کنندہ کے تحت رکھیں
     رول آؤٹ ** UID `sorafs-provider-admission` کے ساتھ۔
   - اس بات کو یقینی بنائیں کہ انتباہ کے قواعد مشترکہ چینل کی طرف اشارہ کرتے ہیں
     اسٹیجنگ اور پروڈکشن میں `sorafs-advert-rollout`۔

## ٹیلی میٹری اور ڈیش بورڈز

مندرجہ ذیل میٹرکس پہلے ہی `iroha_telemetry` کے ذریعے بے نقاب ہوچکے ہیں:

- `torii_sorafs_admission_total{result,reason}` - گنتی قبول ، مسترد کردی گئی
  اور انتباہات۔ وجوہات میں `missing_envelope` ، `unknown_capability` ،
  `stale` ، اور `policy_violation`۔

Grafana برآمد کریں: [`docs/source/grafana_sorafs_admission.json`] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/grafana_sorafs_admission.json)۔
فائل کو ڈیش بورڈز شیئرڈ ریپو (`observability/dashboards`) میں درآمد کریں
اور صرف اشاعت سے پہلے ڈیٹا سورس UID کو اپ ڈیٹ کریں۔

بورڈ فولڈر Grafana ** SoraFS / فراہم کنندہ رول آؤٹ ** کے تحت شائع ہوا ہے
مستحکم UID `sorafs-provider-admission`۔ الرٹ قواعد
`sorafs-admission-warn` (انتباہ) اور `sorafs-admission-reject` (تنقیدی) ہیں
نوٹیفکیشن پالیسی `sorafs-advert-rollout` کو استعمال کرنے کے لئے پہلے سے تشکیل شدہ ؛
اگر منزل کی فہرست ترمیم کے بجائے تبدیل ہوتی ہے تو اس رابطہ نقطہ کو ایڈجسٹ کریں
ڈیش بورڈ کا JSON۔

تجویز کردہ Grafana پینل:| پینل | استفسار | نوٹ |
| ------- | ------- | ------- |
| ** داخلے کے نتائج کی شرح ** | `sum by(result)(rate(torii_sorafs_admission_total[5m]))` | اسٹیک چارٹ کو تصور کرنے کے لئے قبول کریں بمقابلہ وارن بمقابلہ مسترد کریں۔ انتباہ کریں جب انتباہ> 0.05 * کل (انتباہ) یا مسترد> 0 (تنقیدی)۔ |
| ** انتباہی تناسب ** | `sum(rate(torii_sorafs_admission_total{result="warn"}[5m])) / sum(rate(torii_sorafs_admission_total[5m]))` | سنگل لائن ٹائمسیریز جو پیجر کی دہلیز کو کھانا کھلاتی ہیں (انتباہ کی شرح 5 ٪ 15 منٹ سے زیادہ رولنگ)۔ |
| ** مسترد ہونے کی وجوہات ** | `sum by(reason)(rate(torii_sorafs_admission_total{result="reject"}[5m]))` | ہدایت نامہ رن بک ٹریج ؛ تخفیف کے اقدامات سے لنک منسلک کریں۔ |
| ** ریفریش قرض ** | `sum(rate(torii_sorafs_admission_total{reason="stale"}[1h]))` | ان فراہم کنندگان کی نشاندہی کرتا ہے جو ریفریش ڈیڈ لائن سے محروم رہتے ہیں۔ ڈسکوری کیشے کے نوشتہ جات کے ساتھ عبور کریں۔ |

دستی ڈیش بورڈز کے لئے سی ایل آئی نمونے:

- `sorafs_fetch --provider-metrics-out` کاؤنٹرز `failures` ، `successes` اور
  `disabled` بذریعہ فراہم کنندہ۔ نگرانی کے لئے ایڈہاک ڈیش بورڈز میں درآمد کریں
  فراہم کنندگان کو پیداوار میں تبدیل کرنے سے پہلے آرکسٹریٹر خشک رنز بناتا ہے۔
- فیلڈز `chunk_retry_rate` اور JSON رپورٹ کے `provider_failure_rate`
  باسی پے لوڈ کے پچھلے تھروٹلنگز یا علامات کو اجاگر کریں
  اکثر داخلے کے ردعمل۔

### ڈیش بورڈ لے آؤٹ Grafana

مشاہدہ ایک سرشار بورڈ شائع کرتا ہے - ** SoraFS فراہم کنندہ داخلہ
رول آؤٹ ** (`sorafs-provider-admission`) - کے تحت ** SoraFS / فراہم کنندہ رول آؤٹ **
مندرجہ ذیل کیننیکل پینل IDs کے ساتھ:

- پینل 1 - * داخلے کے نتائج کی شرح * (اسٹیکڈ ایریا ، "اوپس/منٹ" یونٹ)۔
- پینل 2 - * انتباہی تناسب * (سنگل سیریز) ، اظہار کو خارج کرتے ہوئے
  `رقم (شرح (torii_sorafs_admission_total {نتیجہ =" انتباہ "} [5m])) /
   رقم (شرح (torii_sorafs_admission_total [5m])) `.
- پینل 3 - * مسترد ہونے کی وجوہات * (ٹائم سیریز `reason` کے ذریعہ گروپ کردہ)
  `rate(...[5m])`۔
- پینل 4 - * ریفریش قرض * (اسٹیٹ) ، مذکورہ جدول سے سوال کو دہراتے ہوئے اور
  ہجرت کے لیجر سے نکالی گئی اشتہاری ریفریش ڈیڈ لائن کے ساتھ تشریح کی۔

انفرا ڈیش بورڈز ریپو میں JSON کنکال کو کاپی (یا تخلیق کریں)
`observability/dashboards/sorafs_provider_admission.json` ، پھر تازہ کاری کریں
صرف ڈیٹا سورس UID ؛ پینل IDs اور الرٹ کے قواعد ہیں
ذیل میں رن بوکس کے ذریعہ حوالہ دیا گیا ہے ، لہذا ان کے بغیر ان کی بازیافت سے گریز کریں
اس دستاویزات کو اپ ڈیٹ کریں۔

سہولت کے ل the ، ریپو ایک حوالہ ڈیش بورڈ کی تعریف فراہم کرتا ہے
`docs/source/grafana_sorafs_admission.json` ؛ اسے اپنے فولڈر Grafana میں کاپی کریں
اگر آپ کو مقامی جانچ کے لئے نقطہ آغاز کی ضرورت ہو۔

### انتباہ کے قواعد Prometheus

مندرجہ ذیل اصول گروپ کو شامل کریں
`observability/prometheus/sorafs_admission.rules.yml` (اگر فائل ہے تو فائل بنائیں
پہلا قاعدہ گروپ SoraFS) اور اسے اپنی ترتیب میں شامل کریں
Prometheus۔ اپنے لئے اصل روٹنگ لیبل کے ساتھ `<pagerduty>` کو تبدیل کریں
آن کال گردش

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

`scripts/check_prometheus_rules.sh observability/prometheus/sorafs_admission.rules.yml` چلائیں
تبدیلیوں کو آگے بڑھانے سے پہلے یہ تصدیق کرنے کے لئے کہ نحو گزرتا ہے
`promtool check rules`۔

## تعیناتی میٹرکس| اشتہار کی خصوصیات | R0 | R1 | R2 | R3 |
| --------- | ---- | ---- | ---- | ---- |
| `profile_id = sorafs.sf1@1.0.0` ، `chunk_range_fetch` موجود ، کیننیکل عرفی ، `signature_strict=true` | ✅ | ✅ | ✅ | ✅ |
| صلاحیت کی کمی `chunk_range_fetch` | ⚠ انتباہ (انجسٹ + ٹیلی میٹری) | ⚠ انتباہ | ❌ مسترد (`reason="missing_capability"`) | ❌ ریجیکٹ |
| `allow_unknown_capabilities=true` کے بغیر نامعلوم صلاحیت کے TLVs | ✅ | ⚠ انتباہ (`reason="unknown_capability"`) | ❌ ریجیکٹ | ❌ ریجیکٹ |
| `refresh_deadline` کی میعاد ختم | ❌ ریجیکٹ | ❌ ریجیکٹ | ❌ ریجیکٹ | ❌ ریجیکٹ |
| `signature_strict=false` (تشخیصی فکسچر) | ✅ (صرف ترقی) | ⚠ انتباہ | ⚠ انتباہ | ❌ ریجیکٹ |

ہر وقت UTC کا استعمال کرتے ہیں۔ نفاذ کی تاریخیں اس میں جھلکتی ہیں
ہجرت لیجر اور کونسل کے ووٹ کے بغیر منتقل نہیں ہوگا۔ کوئی تبدیلی
اسی فائل اور لیجر کو اسی PR میں اپ ڈیٹ کرنے کی ضرورت ہے۔

> ** عمل درآمد نوٹ: ** R1 `result="warn"` سیریز میں متعارف کراتا ہے
> `torii_sorafs_admission_total`۔ ingesion پیچ Torii جو نیا شامل کرتا ہے
> لیبل SF-2 ٹیلی میٹری کے کاموں کے ساتھ ٹریک کیا گیا ہے۔ تب تک ، استعمال کریں

## مواصلات اور واقعہ کا انتظام

- ** ہفتہ وار اسٹیٹس میلر۔ ** ڈیورل نے میٹرکس کا ایک مختصر خلاصہ نشر کیا
  داخلہ ، موجودہ انتباہات اور آنے والی ڈیڈ لائن۔
- ** واقعہ کا جواب۔ ** اگر `reject` الرٹس کو متحرک کیا گیا ہے تو ، آن کال:
  1. ڈسکوری Torii (`/v2/sorafs/providers`) کے ذریعے ناقص اشتہار بازیافت کریں۔
  2. فراہم کنندہ پائپ لائن میں اشتہار کی توثیق کو دوبارہ شروع کریں اور اس کے ساتھ موازنہ کریں
     `/v2/sorafs/providers` غلطی کو دوبارہ پیش کرنے کے لئے۔
  3. اگلے سے پہلے اشتہار چلانے کے لئے فراہم کنندہ کے ساتھ رابطہ کریں
     آخری تاریخ کو تازہ دم کریں۔
- ** منجمد تبدیلیاں۔ ** صلاحیتوں کے اسکیما میں کوئی ترمیم نہیں
  R1/R2 جب تک کہ رول آؤٹ کمیٹی کی توثیق نہ ہو۔ چکنائی کے ٹیسٹ لازمی ہیں
  ہفتہ وار بحالی ونڈو کے دوران شیڈول کیا جائے اور لاگ ان کریں
  ہجرت لیجر میں۔

## حوالہ جات

- [SoraFS نوڈ/کلائنٹ پروٹوکول] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/sorafs_node_client_protocol.md)
- [فراہم کنندہ داخلہ پالیسی] (./provider-admission-policy)
- [ہجرت روڈ میپ] (./migration-roadmap)
- [فراہم کنندہ ایڈورٹ ملٹی سورس ایکسٹینشن] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md)