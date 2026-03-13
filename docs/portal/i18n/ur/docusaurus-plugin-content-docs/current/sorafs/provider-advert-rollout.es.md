---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/provider-advert-rollout.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
عنوان: "وینڈر ایڈورٹائزمنٹ تعیناتی منصوبہ SoraFS"
---

> [`docs/source/sorafs/provider_advert_rollout.md`] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_rollout.md) سے موافقت پذیر۔

# فروش اشتہار کی تعیناتی کا منصوبہ SoraFS

یہ منصوبہ اجازت فراہم کرنے والے اشتہارات سے کٹ اوور کو مربوط کرتا ہے
`ProviderAdvertV1` کثیر نوادرات کی بازیابی کے لئے درکار سطح کی حکومت کی سطح
ٹکڑوں کی اس میں تین فراہمی پر توجہ دی گئی ہے:

-** آپریٹر گائیڈ۔ ** مرحلہ وار اقدامات جو اسٹوریج فراہم کرنے والوں کو لازمی ہے
  ہر گیٹ سے پہلے مکمل کریں۔
- ** ٹیلی میٹری کوریج۔ ** ڈیش بورڈز اور الرٹس جو مشاہدہ اور اوپس استعمال کرتے ہیں
  اس بات کی تصدیق کرنے کے لئے کہ نیٹ ورک صرف تعمیل اشتہارات کو قبول کرتا ہے۔
  ایس ڈی کے اور ٹولنگ ٹیموں کو ان کی ریلیز کی منصوبہ بندی کرنے کے لئے۔

رول آؤٹ SF-2B/2C سنگ میل کے ساتھ منسلک ہے
۔
[فراہم کنندہ داخلہ پالیسی] (./provider-admission-policy) کا داخلہ پہلے ہی موجود ہے
جوش

## فیز شیڈول

| مرحلہ | ونڈو (ہدف) | سلوک | آپریٹر کے اعمال | مشاہدہ کرنے کا نقطہ نظر |
| ------- | ------------------- | ----------- | --------------------- | ----- |

## آپریٹر چیک لسٹ

1. ** انوینٹری اشتہارات۔ ** ہر شائع شدہ اشتہار اور ریکارڈ کی فہرست بنائیں:
   - گورننگ لفافے کا راستہ (`defaults/nexus/sorafs_admission/...` یا پیداوار میں مساوی)۔
   - `profile_id` اور `profile_aliases` انتباہ کا۔
   - صلاحیتوں کی فہرست (کم از کم `torii_gateway` اور `chunk_range_fetch` کی توقع کی جاتی ہے)۔
   - پرچم `allow_unknown_capabilities` (جب TLVS موجود ہوتا ہے تو وینڈر کے ذریعہ محفوظ ہوتا ہے)۔
2. ** فراہم کنندگان کے ٹولنگ کے ساتھ دوبارہ تخلیق کریں۔ **
   - اپنے فراہم کنندہ کے اشتہار پبلشر کے ساتھ پے لوڈ کو دوبارہ تعمیر کریں ، اس بات کو یقینی بنائیں:
     - `profile_id=sorafs.sf1@1.0.0`
     - `capability=chunk_range_fetch` ایک متعین `max_span` کے ساتھ
     - `allow_unknown_capabilities=<true|false>` جب چکنائی TLVs موجود ہیں
   - `/v2/sorafs/providers` اور `sorafs_fetch` کے ذریعے توثیق کرتا ہے۔ کے بارے میں انتباہ
     نامعلوم صلاحیتوں کو بھی سہارا دیا جانا چاہئے۔
3. ** کثیر اوریگین تیاری کی توثیق کریں۔ **
   - `sorafs_fetch` کو `--provider-advert=<path>` کے ساتھ چلائیں ؛ سی ایل آئی اب گر کر تباہ ہوگیا
     جب `chunk_range_fetch` غائب ہو اور صلاحیتوں کے لئے انتباہ دکھا رہا ہو
     نامعلوم نامعلوم۔ JSON رپورٹ پر قبضہ کریں اور اسے لاگ ان کے ساتھ محفوظ کریں
     آپریشنز کی
4. ** تجدیدات تیار کریں۔ **
   - کم از کم 30 دن پہلے لفافے `ProviderAdmissionRenewalV1` بھیجیں
     گیٹ وے (R2) پر نفاذ۔ تجدیدات کو کنٹرول برقرار رکھنا چاہئے
     کیننیکل اور صلاحیتوں کا مجموعہ ؛ صرف داؤ ، اختتامی نکات یا میٹا ڈیٹا ہونا چاہئے
     تبدیل کریں۔
5. ** منحصر ٹیموں سے بات چیت کریں۔ **
   - ایس ڈی کے مالکان کو ایسے ورژن جاری کرنا ہوں گے جو صارفین کو انتباہات کو بے نقاب کریں۔
     آپریٹرز جب اشتہارات مسترد کردیئے جاتے ہیں۔
   - ڈیوریل ہر مرحلے کی منتقلی کا اعلان کرتا ہے۔ ڈیش بورڈز اور دی کے لنک شامل کریں
     نیچے تھریشولڈ منطق۔
6. ** ڈیش بورڈز اور الرٹس انسٹال کریں۔ **
   - Grafana کی برآمد درآمد کریں اور اسے ** SoraFS / فراہم کنندہ کے تحت رکھیں
     رول آؤٹ ** UID `sorafs-provider-admission` کے ساتھ۔
   - یقینی بناتا ہے کہ انتباہ کے قواعد مشترکہ چینل کی طرف اشارہ کریں
     اسٹیجنگ اور پروڈکشن میں `sorafs-advert-rollout`۔

## ٹیلی میٹری اور ڈیش بورڈزمندرجہ ذیل میٹرکس پہلے ہی `iroha_telemetry` کے ذریعے بے نقاب ہوچکے ہیں:

- `torii_sorafs_admission_total{result,reason}` - اکاؤنٹ قبول ، مسترد کردیا گیا
  اور انتباہ کے ساتھ نتائج۔ وجوہات میں `missing_envelope` ، `unknown_capability` ،
  `stale` اور `policy_violation`۔

Grafana کی برآمد: [`docs/source/grafana_sorafs_admission.json`] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/grafana_sorafs_admission.json)۔
فائل کو مشترکہ ڈیش بورڈ ذخیرہ میں درآمد کریں (`observability/dashboards`)
اور اشاعت سے پہلے صرف ڈیٹا سورس کے UID کو اپ ڈیٹ کریں۔

ڈیش بورڈ Grafana ** SoraFS / فراہم کنندہ رول آؤٹ ** فولڈر کے تحت شائع ہوا ہے
مستحکم UID `sorafs-provider-admission`۔ الرٹ قواعد
`sorafs-admission-warn` (انتباہ) اور `sorafs-admission-reject` (تنقیدی) ہیں
نوٹیفکیشن پالیسی `sorafs-advert-rollout` کو استعمال کرنے کے لئے پہلے سے تشکیل شدہ ؛
اگر منزل کی فہرست میں ترمیم کرنے کے بجائے منزل کی فہرست تبدیل ہوتی ہے تو اس رابطہ نقطہ کو ایڈجسٹ کریں
ڈیش بورڈ کے JSON.

تجویز کردہ Grafana پینل:

| ڈیش بورڈ | استفسار | نوٹ |
| ------- | ------- | ------- |
| ** داخلے کے نتائج کی شرح ** | `sum by(result)(rate(torii_sorafs_admission_total[5m]))` | اسٹیک چارٹ کو تصور کرنے کے لئے قبول کریں بمقابلہ وارن بمقابلہ مسترد کریں۔ انتباہ کریں جب انتباہ> 0.05 * کل (انتباہ) یا مسترد> 0 (تنقیدی)۔ |
| ** انتباہی تناسب ** | `sum(rate(torii_sorafs_admission_total{result="warn"}[5m])) / sum(rate(torii_sorafs_admission_total[5m]))` | سنگل لائن ٹائمریز جو پیجر کی دہلیز کو کھانا کھلاتی ہیں (5 ٪ انتباہی شرح 15 منٹ میں گھوم رہی ہے)۔ |
| ** مسترد ہونے کی وجوہات ** | `sum by(reason)(rate(torii_sorafs_admission_total{result="reject"}[5m]))` | گائیڈ رن بک ٹریج ؛ تخفیف کے مراحل سے لنک منسلک کرتا ہے۔ |
| ** ریفریش قرض ** | `sum(rate(torii_sorafs_admission_total{reason="stale"}[1h]))` | ان فراہم کنندگان کی نشاندہی کرتا ہے جو ریفریش ڈیڈ لائن کو پورا نہیں کرتے ہیں۔ دریافت کیشے کے نوشتہ جات کے ساتھ عبور کرتا ہے۔ |

دستی ڈیش بورڈز کے لئے سی ایل آئی نمونے:

- `sorafs_fetch --provider-metrics-out` کاؤنٹرز `failures` ، `successes` اور
  `disabled` بذریعہ فراہم کنندہ۔ نگرانی کے لئے ایڈہاک ڈیش بورڈز میں درآمد کریں
  پروڈکشن میں فراہم کنندگان کو تبدیل کرنے سے پہلے آرکسٹریٹر خشک رنز بناتا ہے۔
- JSON رپورٹ کے فیلڈز `chunk_retry_rate` اور `provider_failure_rate`
  تھروٹلنگ یا پے لوڈ باسی علامات کو اجاگر کریں جو عام طور پر مسترد ہونے سے پہلے ہوتے ہیں
  داخلے کی

### Grafana ڈیش بورڈ لے آؤٹ

مشاہدہ ایک سرشار بورڈ شائع کرتا ہے - ** SoraFS فراہم کنندہ داخلہ
رول آؤٹ ** (`sorafs-provider-admission`) - کے تحت ** SoraFS / فراہم کنندہ رول آؤٹ **
مندرجہ ذیل کیننیکل پینل IDs کے ساتھ:

- پینل 1 - * داخلے کے نتائج کی شرح * (اسٹیکڈ ایریا ، یونٹ "اوپس/منٹ")۔
- پینل 2 - * انتباہی تناسب * (سنگل سیریز) ، اظہار کو خارج کرتے ہوئے
  `رقم (شرح (torii_sorafs_admission_total {نتیجہ =" انتباہ "} [5m])) /
   رقم (شرح (torii_sorafs_admission_total [5m])) `.
- پینل 3 - * مسترد ہونے کی وجوہات * (ٹائم سیریز `reason` کے ذریعہ گروپ کردہ)
  `rate(...[5m])`۔
- پینل 4 - * ریفریش قرض * (STAT) ، پچھلے ٹیبل کے استفسار کی عکاسی کرتا ہے اور
  منتقلی لیجر سے نکالے گئے اشتہارات کی ریفریش ڈیڈ لائن کے ساتھ تشریح کی گئی۔

انفراسٹرکچر ڈیش بورڈز ریپو میں JSON کنکال کو کاپی (یا تخلیق کریں)
`observability/dashboards/sorafs_provider_admission.json` ، پھر صرف اپ ڈیٹ کریں
ڈیٹا سورس UID ؛ پینل IDs اور الرٹ کے قواعد کا حوالہ دیا گیا ہے
ذیل میں رن بکس ، لہذا اس دستاویزات کا جائزہ لینے کے بغیر ان کی بازیافت سے گریز کریں۔سہولت کے ل the ، ذخیرے میں پہلے ہی ڈیش بورڈ کی تعریف شامل ہے۔
`docs/source/grafana_sorafs_admission.json` میں حوالہ ؛ اسے اپنے فولڈر میں کاپی کریں
Grafana اگر آپ کو مقامی جانچ کے لئے نقطہ آغاز کی ضرورت ہو۔

####Prometheus الرٹ قواعد

مندرجہ ذیل قواعد کا سیٹ شامل کریں
`observability/prometheus/sorafs_admission.rules.yml` (اگر فائل ہے تو فائل بنائیں
قواعد SoraFS کا پہلا گروپ) اور اسے اپنی تشکیل سے شامل کریں
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
تبدیلیوں کو اپ لوڈ کرنے سے پہلے یہ یقینی بنانے کے لئے کہ نحو `promtool check rules` سے گزرتا ہے۔

## رول آؤٹ سرنی

| اشتہار کی خصوصیات | R0 | R1 | R2 | R3 |
| ----------------------------- | ---- | ---- | ---- | ---- | ---- |
| `profile_id = sorafs.sf1@1.0.0` ، `chunk_range_fetch` موجود ، کیننیکل عرفی ، `signature_strict=true` | ✅ | ✅ | ✅ | ✅ |
| صلاحیت کا فقدان `chunk_range_fetch` | ⚠ انتباہ (انجشن + ٹیلی میٹری) | ⚠ وارن | ❌ مسترد (`reason="missing_capability"`) | ❌ ریجیکٹ |
| `allow_unknown_capabilities=true` کے بغیر نامعلوم صلاحیت کے TLVs | ✅ | ⚠ انتباہ (`reason="unknown_capability"`) | ❌ ریجیکٹ | ❌ ریجیکٹ |
| `refresh_deadline` کی میعاد ختم | ❌ ریجیکٹ | ❌ ریجیکٹ | ❌ ریجیکٹ | ❌ ریجیکٹ |
| `signature_strict=false` (تشخیصی فکسچر) | ✅ (صرف ترقی) | ⚠ وارن | ⚠ وارن | ❌ ریجیکٹ |

ہر وقت UTC کا استعمال کرتے ہیں۔ نفاذ کی تاریخیں ہجرت میں جھلکتی ہیں
لیجر اور کونسل کے ووٹ کے بغیر منتقل نہیں ہوگا۔ کسی بھی تبدیلی کے لئے تازہ کاری کی ضرورت ہوتی ہے
یہ فائل اور اسی PR میں لیجر۔

> ** عمل درآمد نوٹ: ** R1 `result="warn"` سیریز میں متعارف کراتا ہے
> `torii_sorafs_admission_total`۔ Torii Ingesion پیچ جو نیا شامل کرتا ہے
> SF-2 ٹیلی میٹری کے کاموں کے ساتھ ٹیگ کی پیروی کی جاتی ہے۔ جب تک یہ نہ پہنچے ،

## مواصلات اور واقعہ کا انتظام

- ** ہفتہ وار اسٹیٹس میلر۔ ** ڈیوریل نے کارکردگی کی پیمائش کا ایک مختصر خلاصہ گردش کیا۔
  داخلہ ، زیر التواء انتباہات اور آنے والی ڈیڈ لائن۔
- ** واقعہ کا جواب۔ ** اگر `reject` انتباہات چالو ہوجائیں تو ، آن کال:
  1. Torii (`/v2/sorafs/providers`) کی دریافت کے ذریعے جارحانہ اشتہار کی بازیافت کریں۔
  2. فراہم کنندہ پائپ لائن میں اشتہار کی توثیق کو دوبارہ عمل کریں اور اس کے ساتھ موازنہ کریں
     `/v2/sorafs/providers` غلطی کو دوبارہ پیش کرنے کے لئے۔
  3. فراہم کنندہ کے ساتھ ہم آہنگی کے ساتھ اگلے ریفریش سے پہلے اشتہار کی گردش
     آخری تاریخ
- ** تبدیلی جم جاتی ہے۔ ** صلاحیتوں کے اسکیما میں کوئی تبدیلی نہیں کی جاتی ہے
  R1/R2 کے دوران جب تک کہ رول آؤٹ کمیٹی کے ذریعہ منظور نہ ہو۔ چکنائی کی آزمائشیں لازمی ہیں
  ہفتہ وار بحالی ونڈو کے دوران شیڈول اور میں ریکارڈ کیا گیا
  ہجرت لیجر۔

## حوالہ جات

- [SoraFS نوڈ/کلائنٹ پروٹوکول] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/sorafs_node_client_protocol.md)
- [فراہم کنندہ داخلہ پالیسی] (./provider-admission-policy)
- [ہجرت روڈ میپ] (./migration-roadmap)
- [فراہم کنندہ ایڈورٹ ملٹی سورس ایکسٹینشن] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md)