---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/provider-advert-rollout.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
عنوان: "فراہم کنندہ اشتہار رول آؤٹ پلان SoraFS"
---

> [`docs/source/sorafs/provider_advert_rollout.md`] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_rollout.md) سے موافقت پذیر۔

# فراہم کنندہ اشتہار رول آؤٹ پلان SoraFS

یہ منصوبہ اس کے لئے جائز فراہم کنندہ اشتہارات کے کٹ اوور کو مربوط کرتا ہے
بازیافت کے لئے مکمل طور پر حکومت کی سطح `ProviderAdvertV1` کی ضرورت ہے
ملٹی سورس حصے اس میں تین فراہمی پر توجہ دی گئی ہے:

- ** آپریٹرز گائیڈ۔ ** اقدامات جو اسٹوریج فراہم کرنے والوں کو ہر گیٹ سے پہلے مکمل کرنے کی ضرورت ہے۔
- ** ٹیلی میٹری کوریج۔ ** ڈیش بورڈز اور الرٹس جو مشاہدہ اور اوپس استعمال کرتے ہیں
  اس بات کی تصدیق کرنے کے لئے کہ نیٹ ورک صرف تعمیل اشتہارات کو قبول کرتا ہے۔
  ایس ڈی کے اور ٹولنگ ٹیموں کے لئے ریلیز کی منصوبہ بندی کرنے کے لئے۔

رول آؤٹ SF-2B/2C سنگ میل کے ساتھ منسلک ہوتا ہے
[migration roadmap SoraFS](./migration-roadmap) and assumes that the admission policy
نہیں [فراہم کنندہ داخلہ پالیسی] (./provider-admission-policy) پہلے ہی فعال ہے۔

## فیز ٹائم لائن

| مرحلہ | ونڈو (ہدف) | سلوک | آپریٹر کے اعمال | مشاہدہ کی توجہ |
| ------- | ------------------- | ----------- | -------------------- | ------------------------- |

## آپریٹر چیک لسٹ

1. ** انوینٹری اشتہارات۔ ** شائع شدہ ہر اشتہار کی فہرست بنائیں اور ریکارڈ کریں:
   - گورننگ لفافے کا راستہ (`defaults/nexus/sorafs_admission/...` یا پیداوار میں مساوی)۔
   - `profile_id` اور `profile_aliases` اشتہار سے۔
   - صلاحیتوں کی فہرست (کم از کم `torii_gateway` اور `chunk_range_fetch` کی توقع کریں)۔
   - پرچم `allow_unknown_capabilities` (جب وینڈر سے محفوظ TLVs موجود ہوں تو ضروری ہے)۔
2. ** فراہم کنندہ ٹولنگ کے ساتھ دوبارہ تخلیق کریں۔ **
   - اپنے اشتہاری فراہم کنندہ پبلشر کے ساتھ پے لوڈ کو دوبارہ تعمیر کریں ، اس بات کو یقینی بنائیں کہ:
     - `profile_id=sorafs.sf1@1.0.0`
     - `capability=chunk_range_fetch` کے ساتھ `max_span` کی وضاحت کی گئی ہے
     - `allow_unknown_capabilities=<true|false>` جب چکنائی TLVs موجود ہیں
   - `/v1/sorafs/providers` اور `sorafs_fetch` کے ذریعے توثیق کریں ؛ صلاحیتوں کے بارے میں انتباہات
     نامعلوم افراد کو اسکریننگ کرنا ضروری ہے۔
3. ** کثیر سورس تیاری کی توثیق کریں۔ **
   - `sorafs_fetch` کو `--provider-advert=<path>` کے ساتھ چلائیں ؛ سی ایل آئی اب تب کر کے گر کر تباہ ہو گیا ہے
     `chunk_range_fetch` غائب ہے اور نامعلوم صلاحیتوں کے لئے انتباہات دکھاتا ہے
     نظرانداز JSON رپورٹ پر قبضہ کریں اور اسے آپریشن لاگز کے ساتھ محفوظ کریں۔
4. ** تجدیدات تیار کریں۔ **
   - لفافے `ProviderAdmissionRenewalV1` کو کم از کم 30 دن پہلے بھیجیں
     گیٹ وے (R2) پر نفاذ۔ تجدیدات کو لازمی طور پر ہینڈل اور اس کو برقرار رکھنا چاہئے
     صلاحیتیں سیٹ ؛ صرف داؤ ، اختتامی نکات یا میٹا ڈیٹا کو تبدیل کرنا چاہئے۔
5. ** انحصار کرنے والی ٹیموں سے بات چیت کریں۔ **
   - ایس ڈی کے مالکان کو ایسے ورژن جاری کرنا ہوں گے جو آپریٹرز کو انتباہ کرتے ہیں جب
     اشتہارات مسترد کردیئے جاتے ہیں۔
   - ڈیوریل ہر مرحلے کی منتقلی کا اعلان کرتا ہے۔ ڈیش بورڈ لنکس اور منطق شامل کریں
     نیچے دہلیز کی
6. ** ڈیش بورڈز اور الرٹس انسٹال کریں۔ **
   - Grafana برآمد کو درآمد کریں اور ** SoraFS / فراہم کنندہ رول آؤٹ ** کے تحت رکھیں
     `sorafs-provider-admission`۔
   - یقینی بنائیں کہ انتباہ کے قواعد مشترکہ چینل کی طرف اشارہ کریں
     اسٹیجنگ اور پروڈکشن میں `sorafs-advert-rollout`۔

## ٹیلی میٹری اور ڈیش بورڈز

مندرجہ ذیل میٹرکس پہلے ہی `iroha_telemetry` کے ذریعے بے نقاب ہوچکے ہیں:- `torii_sorafs_admission_total{result,reason}` - اکاؤنٹ قبول ، مسترد اور
  انتباہ وجوہات میں `missing_envelope` ، `unknown_capability` ، `stale` شامل ہیں
  اور `policy_violation`۔

Grafana برآمد: [`docs/source/grafana_sorafs_admission.json`] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/grafana_sorafs_admission.json)۔
فائل کو مشترکہ ڈیش بورڈ ذخیرہ میں درآمد کریں (`observability/dashboards`)
اور صرف اشاعت سے پہلے ڈیٹا سورس UID کو اپ ڈیٹ کریں۔

بورڈ Grafana ** SoraFS / فراہم کنندہ رول آؤٹ ** میں UID کے ساتھ شائع ہوا ہے
مستحکم `sorafs-provider-admission`۔ الرٹ کے قواعد
`sorafs-admission-warn` (انتباہ) اور `sorafs-admission-reject` (تنقیدی) ہیں
`sorafs-advert-rollout` نوٹیفکیشن پالیسی کو استعمال کرنے کے لئے پہلے سے تشکیل دیا گیا ہے۔ ایڈجسٹمنٹ
یہ رابطہ نقطہ اگر منزل مقصود میں تبدیل ہوتا ہے تو ، ڈیش بورڈ JSON میں ترمیم کرنے کے بجائے۔

تجویز کردہ Grafana پینل:

| پینل | استفسار | نوٹ |
| ------- | ------- | ------- |
| ** داخلے کے نتائج کی شرح ** | `sum by(result)(rate(torii_sorafs_admission_total[5m]))` | اسٹیک چارٹ کو تصور کرنے کے لئے قبول کریں بمقابلہ وارن بمقابلہ مسترد کریں۔ انتباہ کریں جب انتباہ> 0.05 * کل (انتباہ) یا مسترد> 0 (تنقیدی)۔ |
| ** انتباہی تناسب ** | `sum(rate(torii_sorafs_admission_total{result="warn"}[5m])) / sum(rate(torii_sorafs_admission_total[5m]))` | سنگل لائن ٹائمریز جو پیجر کی دہلیز کو کھانا کھلاتی ہیں (5 ٪ انتباہی شرح 15 منٹ میں گھوم رہی ہے)۔ |
| ** مسترد ہونے کی وجوہات ** | `sum by(reason)(rate(torii_sorafs_admission_total{result="reject"}[5m]))` | رن بک ٹریج گائیڈ ؛ تخفیف سے لنک منسلک کریں۔ |
| ** ریفریش قرض ** | `sum(rate(torii_sorafs_admission_total{reason="stale"}[1h]))` | ان فراہم کنندگان کی نشاندہی کرتا ہے جو ریفریش ڈیڈ لائن سے محروم رہتے ہیں۔ دریافت کیشے کے نوشتہ جات کے ساتھ عبور کریں۔ |

دستی ڈیش بورڈز کے لئے سی ایل آئی نمونے:

- `sorafs_fetch --provider-metrics-out` کاؤنٹرز لکھتا ہے `failures` ، `successes`
  اور فراہم کنندہ کے ذریعہ `disabled`۔ خشک رنز کی نگرانی کے لئے ایڈہاک ڈیش بورڈز میں درآمد کریں
  پروڈکشن میں فراہم کنندگان کو تبدیل کرنے سے پہلے آرکسٹریٹر سے۔
- JSON رپورٹ کے `chunk_retry_rate` اور `provider_failure_rate` فیلڈز نمایاں کریں
  تھروٹلنگ یا باسی پے لوڈ کی علامات جو اکثر داخلے سے پہلے کے رد عمل سے قبل ہوتی ہیں۔

### ڈیش بورڈ لے آؤٹ Grafana

مشاہدہ ایک سرشار بورڈ - ** SoraFS فراہم کنندہ داخلہ شائع کرتا ہے
رول آؤٹ ** (`sorafs-provider-admission`) - کے تحت ** SoraFS / فراہم کنندہ رول آؤٹ **
مندرجہ ذیل کیننیکل پینل IDs کے ساتھ:

- پینل 1 - * داخلہ نتائج کی شرح * (اسٹیکڈ ایریا ، یونٹ "اوپس/منٹ")۔
- پینل 2 - * انتباہی تناسب * (سنگل سیریز) ، اظہار کے ساتھ
  `رقم (شرح (torii_sorafs_admission_total {نتیجہ =" انتباہ "} [5m])) /
   رقم (شرح (torii_sorafs_admission_total [5m])) `.
- پینل 3 - * مسترد ہونے کی وجوہات * (ٹائم سیریز `reason` کے ذریعہ گروپ کردہ)
  `rate(...[5m])`۔
- پینل 4 - * ریفریش قرض * (اسٹیٹ) ، مذکورہ جدول میں استفسار کی آئینہ دار ہے اور نوٹ کیا گیا ہے
  منتقلی لیجر سے نکالے گئے اشتہارات کے لئے ریفریش ڈیڈ لائن کے ساتھ۔

انفرا ڈیش بورڈز ریپو میں JSON کنکال کو کاپی (یا تخلیق کریں)
`observability/dashboards/sorafs_provider_admission.json` ، پھر صرف اپ ڈیٹ کریں
ڈیٹا سورس کا UID ؛ پینل آئی ڈی اور الرٹ کے قواعد کا حوالہ دیا جاتا ہے
ذیل میں رن بکس ، لہذا اس دستاویزات کا جائزہ لینے کے بغیر رنجیدہ ہونے سے گریز کریں۔

سہولت کے ل the ، ریپو میں پہلے ہی ایک ریفرنس ڈیش بورڈ کی تعریف شامل ہے
`docs/source/grafana_sorafs_admission.json` ؛ اپنے فولڈر میں کاپی Grafana اگر
مقامی جانچ کے لئے نقطہ آغاز کی ضرورت ہے۔

### انتباہ کے قواعد Prometheusمندرجہ ذیل اصول گروپ کو شامل کریں
`observability/prometheus/sorafs_admission.rules.yml` (اگر یہ ہے تو فائل بنائیں
پہلا قاعدہ گروپ SoraFS) اور اسے Prometheus کنفیگریشن میں شامل کریں۔
`<pagerduty>` کو اپنے آن کال گردش کے اصل روٹنگ لیبل کے ساتھ تبدیل کریں۔

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
تبدیلیاں جمع کروانے سے پہلے یہ یقینی بنانے کے لئے کہ نحو `promtool check rules` سے گزرتا ہے۔

## رول آؤٹ میٹرکس

| اشتہار کی خصوصیات | R0 | R1 | R2 | R3 |
| ------ | ---- | ---- | ---- | ---- |
| `profile_id = sorafs.sf1@1.0.0` ، `chunk_range_fetch` موجود ، کیننیکل عرفی ، `signature_strict=true` | ٹھیک ہے | ٹھیک ہے | ٹھیک ہے | ٹھیک ہے |
| `chunk_range_fetch` صلاحیت کی عدم موجودگی | انتباہ (انجسٹ + ٹیلی میٹری) | انتباہ | مسترد (`reason="missing_capability"`) | مسترد |
| `allow_unknown_capabilities=true` کے بغیر نامعلوم صلاحیت TLVS | ٹھیک ہے | انتباہ (`reason="unknown_capability"`) | مسترد | مسترد |
| `refresh_deadline` کی میعاد ختم | مسترد | مسترد | مسترد | مسترد |
| `signature_strict=false` (تشخیصی فکسچر) | ٹھیک ہے (صرف ترقی) | انتباہ | انتباہ | مسترد |

ہر وقت UTC کا استعمال کرتے ہیں۔ نفاذ کی تاریخیں ہجرت میں جھلکتی ہیں
لیجر اور کونسل کے ووٹ کے بغیر تبدیل نہ کریں۔ کسی بھی تبدیلیوں کو اس کی تازہ کاری کی ضرورت ہوتی ہے
فائل اور لیجر ایک ہی PR میں۔

> ** عمل درآمد نوٹ: ** R1 `result="warn"` سیریز میں متعارف کراتا ہے
> `torii_sorafs_admission_total`۔ Torii ingest پیچ جو شامل کرتا ہے
> نیا لیبل اور اس کے ساتھ SF-2 ٹیلی میٹری کے کام ؛ تب تک ، استعمال کریں

## مواصلات اور واقعہ سے نمٹنے کے

- ** ہفتہ وار اسٹیٹس میلر۔ ** ڈیوریل داخلہ میٹرکس کا خلاصہ شیئر کرتا ہے ،
  زیر التواء انتباہات اور آنے والی ڈیڈ لائن۔
- ** واقعہ کا جواب۔ ** اگر `reject` فائر ، آن کال انجینئرز:
  1. Torii دریافت (`/v1/sorafs/providers`) کے ذریعے جارحانہ اشتہار کی تلاش کریں۔
  2. فراہم کنندہ پائپ لائن میں اشتہار کی توثیق کو دوبارہ چلائیں اور اس کے ساتھ موازنہ کریں
     `/v1/sorafs/providers` غلطی کو دوبارہ پیش کرنے کے لئے۔
  3. اگلی ریفریش ڈیڈ لائن سے پہلے فراہم کنندہ کے ساتھ اشتہار کی گردش کو مربوط کریں۔
- ** تبدیلی جم گئی۔ ** R1/R2 کے دوران صلاحیتوں کے اسکیما میں کوئی تبدیلی نہیں
  جب تک کہ رول آؤٹ کمیٹی منظور نہ ہو۔ چکنائی کے ٹرائلز کا شیڈول ہونا ضروری ہے
  ہفتہ وار بحالی ونڈو اور ہجرت لیجر میں ریکارڈ کیا گیا۔

## حوالہ جات

- [SoraFS نوڈ/کلائنٹ پروٹوکول] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/sorafs_node_client_protocol.md)
- [فراہم کنندہ داخلہ پالیسی] (./provider-admission-policy)
- [ہجرت روڈ میپ] (./migration-roadmap)
- [فراہم کنندہ ایڈورٹ ملٹی سورس ایکسٹینشن] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md)