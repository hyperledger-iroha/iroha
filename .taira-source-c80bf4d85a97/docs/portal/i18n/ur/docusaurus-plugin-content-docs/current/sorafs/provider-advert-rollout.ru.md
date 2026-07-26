---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/provider-advert-rollout.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
عنوان: "اشتہار فراہم کرنے والوں کے لئے رول آؤٹ اور مطابقت کا منصوبہ SoraFS"
---

> [`docs/source/sorafs/provider_advert_rollout.md`] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_rollout.md) سے موافقت پذیر۔

# اشتہار فراہم کرنے والوں کے لئے رول آؤٹ اور مطابقت کا منصوبہ SoraFS

یہ منصوبہ جائز اشتہاری فراہم کرنے والوں سے مکمل طور پر منتقلی کو مربوط کرتا ہے
ملٹی سورس آؤٹ پٹ کے لئے درکار سطح `ProviderAdvertV1` کو کنٹرول کریں
ٹکڑوں اس میں تین فراہمی پر توجہ دی گئی ہے:

-** آپریٹر کی گائیڈ ** مرحلہ وار اقدامات جو اسٹوریج فراہم کرنے والوں کو چاہئے
  ہر گیٹ آن ہونے سے پہلے اس پر عمل کریں۔
- ** ٹیلی میٹری کوریج۔ ** ڈیش بورڈز اور انتباہات جو مشاہدہ اور اوپس استعمال کرتے ہیں ،
  اس بات کی تصدیق کرنے کے لئے کہ نیٹ ورک صرف ہم آہنگ اشتہارات کو قبول کرتا ہے۔
  ایس ڈی کے اور ٹولنگ ریلیز کا شیڈول کرسکتی ہے۔

رول آؤٹ SF-2B/2C سنگ میل کے ساتھ منسلک ہے
۔
[فراہم کنندہ داخلہ پالیسی] (./provider-admission-policy) پہلے ہی نافذ العمل ہے۔

## مراحل کی ٹائم لائن

| مرحلہ | ونڈو (ہدف) | سلوک | آپریٹر کے اعمال | مشاہدہ کی توجہ |
| ------- | -------- | ---------- | -------------------- | ----------------------- |

## آپریٹر چیک لسٹ

1. ** انوینٹری اشتہارات۔ ** شائع شدہ ہر اشتہار کی فہرست بنائیں اور ریکارڈ:
   - گورننگ لفافے کا راستہ (`defaults/nexus/sorafs_admission/...` یا پروڈکشن مساوی)۔
   - `profile_id` اور `profile_aliases` اشتہار۔
   - صلاحیتوں کی فہرست (کم از کم `torii_gateway` اور `chunk_range_fetch` کی توقع)۔
   - پرچم `allow_unknown_capabilities` (اگر ضروری ہے تو اگر کوئی وینڈر سے محفوظ TLV موجود ہو)۔
2. ** پرووائڈر ٹولنگ کے ذریعے نو تخلیق. **
   - پبلشر کے اشتہار کے ذریعہ پے لوڈ کو دوبارہ تعمیر کریں ، اس بات کو یقینی بنائیں:
     - `profile_id=sorafs.sf1@1.0.0`
     - `capability=chunk_range_fetch` کی وضاحت شدہ `max_span` کے ساتھ
     - `allow_unknown_capabilities=<true|false>` چکنائی TLV کے ساتھ
   - `/v1/sorafs/providers` اور `sorafs_fetch` کے ذریعے چیک کریں۔ کے بارے میں انتباہ
     نامعلوم صلاحیتوں کو سہ رخی کرنے کی ضرورت ہے۔
3. ** کثیر سورس تیاری چیک کریں۔ **
   - `--provider-advert=<path>` سے `sorafs_fetch` پر عمل کریں ؛ سی ایل آئی اب گر کر تباہ ہوگیا
     جب `chunk_range_fetch` غائب ہے ، اور اس کے بارے میں انتباہات پرنٹ کرتا ہے
     نامعلوم صلاحیتوں کو نظرانداز کیا۔ JSON رپورٹ پر قبضہ کریں اور
     اسے آپریشنز لاگز کے ساتھ محفوظ شدہ دستاویزات۔
4. ** توسیع کی تیاری۔ **
   - کم از کم 30 دن پہلے `ProviderAdmissionRenewalV1` لفافے بھیجیں
     گیٹ وے انفورسمنٹ (R2)۔ توسیعوں کو کیننیکل ہینڈل کو محفوظ رکھنا چاہئے اور
     صلاحیتوں کا سیٹ ؛ صرف داؤ ، اختتامی نکات یا میٹا ڈیٹا کو تبدیل کیا جانا چاہئے۔
5. ** منحصر ٹیموں کے ساتھ بات چیت۔ **
   - ایس ڈی کے مالکان کو ایسے ورژن جاری کرنا چاہئے جو آپریٹرز کو انتباہات دکھاتے ہیں
     جب اشتہارات کو مسترد کردیا جاتا ہے۔
   - ڈیوریل نے ہر مرحلے کا اعلان کیا۔ ڈیش بورڈز اور منطق کے لنکس شامل کریں
     نچلے حد
6. ** ڈیش بورڈز اور الرٹس انسٹال کرنا۔ **
   - Grafana برآمد درآمد کریں اور اسے ** SoraFS/فراہم کنندہ میں رکھیں
     رول آؤٹ ** UID `sorafs-provider-admission` کے ساتھ۔
   - یقینی بنائیں کہ الرٹ کے قواعد ایک مشترکہ چینل کو بھیجے گئے ہیں
     اسٹیجنگ اور پروڈکشن میں `sorafs-advert-rollout`۔

## ٹیلی میٹری اور ڈیش بورڈز

مندرجہ ذیل میٹرکس `iroha_telemetry` کے ذریعے پہلے ہی دستیاب ہیں:- `torii_sorafs_admission_total{result,reason}` - قبولیت ، مسترد کاؤنٹرز
  اور انتباہات۔ وجوہات میں `missing_envelope` ، `unknown_capability` ، `stale` اور شامل ہیں
  `policy_violation`۔

Grafana برآمد: [`docs/source/grafana_sorafs_admission.json`] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/grafana_sorafs_admission.json)۔
فائل کو عام ڈیش بورڈ ریپوزٹری (`observability/dashboards`) میں درآمد کریں اور
اشاعت سے پہلے صرف ڈیٹا سورس UID کو اپ ڈیٹ کریں۔

ڈیش بورڈ فولڈر Grafana ** SoraFS / فراہم کنندہ رول آؤٹ ** میں شائع ہوا ہے
مستحکم UID `sorafs-provider-admission`۔ الرٹ قواعد
`sorafs-admission-warn` (انتباہ) اور `sorafs-admission-reject` (تنقیدی)
نوٹیفکیشن پالیسی `sorafs-advert-rollout` کے لئے پہلے سے تشکیل شدہ ؛ رابطہ تبدیل کریں
جب JSON ڈیش بورڈ میں ترمیم کرنے کے بجائے وصول کنندگان کی فہرست تبدیل کرتے ہو تو آئٹم۔

تجویز کردہ پینل Grafana:

| پینل | استفسار | نوٹ |
| ------- | ------- | ------- |
| ** داخلے کے نتائج کی شرح ** | `sum by(result)(rate(torii_sorafs_admission_total[5m]))` | تصور کرنے کے لئے اسٹیک چارٹ بمقابلہ وارن بمقابلہ مسترد کریں۔ انتباہ کریں جب انتباہ> 0.05 * کل (انتباہ) یا مسترد> 0 (تنقیدی)۔ |
| ** انتباہی تناسب ** | `sum(rate(torii_sorafs_admission_total{result="warn"}[5m])) / sum(rate(torii_sorafs_admission_total[5m]))` | پیجر کی دہلیز کو کھلانے والی سنگل لائن ٹائمسیریز (سلائڈنگ 15 منٹ کی ونڈو میں 5 ٪ انتباہی شرح)۔ |
| ** مسترد ہونے کی وجوہات ** | `sum by(reason)(rate(torii_sorafs_admission_total{result="reject"}[5m]))` | رن بک میں تکلیف کے لئے ؛ تخفیف کے اقدامات سے لنک منسلک کریں۔ |
| ** ریفریش قرض ** | `sum(rate(torii_sorafs_admission_total{reason="stale"}[1h]))` | ان فراہم کنندگان کی نشاندہی کرتا ہے جو ریفریش ڈیڈ لائن سے محروم رہتے ہیں۔ دریافت کیشے کے نوشتہ جات چیک کریں۔ |

دستی ڈیش بورڈز کے لئے سی ایل آئی نمونے:

- `sorafs_fetch --provider-metrics-out` کاؤنٹرز `failures` ، `successes` اور
  `disabled` ہر فراہم کنندہ کے لئے۔ ایڈہاک ڈیش بورڈز میں درآمد کریں
  پروڈکشن فراہم کرنے والوں کو تبدیل کرنے سے پہلے ڈرائی رن آرکسٹریٹر کی نگرانی کریں۔
- JSON رپورٹ میں فیلڈز `chunk_retry_rate` اور `provider_failure_rate`
  تھروٹلنگ یا باسی پے لوڈ کی علامات کو اجاگر کریں جو اکثر پہلے سے پہلے ہوتے ہیں
  داخلہ انحراف

### Grafana ڈیش بورڈ لے آؤٹ

مشاہدہ ایک علیحدہ بورڈ شائع کرتا ہے - ** SoraFS فراہم کنندہ داخلہ
رول آؤٹ ** (`sorafs-provider-admission`) - to ** SoraFS / فراہم کنندہ رول آؤٹ **
مندرجہ ذیل کیننیکل پینل IDs کے ساتھ:

- پینل 1 - * داخلے کے نتائج کی شرح * (اسٹیکڈ ایریا ، یونٹ "اوپس/منٹ")۔
- پینل 2 - * انتباہی تناسب * (سنگل سیریز) ، اظہار
  `رقم (شرح (torii_sorafs_admission_total {نتیجہ =" انتباہ "} [5m])) /
   رقم (شرح (torii_sorafs_admission_total [5m])) `.
- پینل 3 - * مسترد ہونے کی وجوہات * (ٹائم سیریز ، `reason` کے ذریعہ گروپ کردہ)
  `rate(...[5m])`۔
- پینل 4 - * ریفریش قرض * (اسٹیٹ) ، مذکورہ جدول سے استفسار کی عکاسی کرتا ہے اور
  منتقلی لیجر سے تشریح شدہ ریفریش ڈیڈ لائن۔

انفراسٹرکچر ڈیش بورڈز کے ذخیرے میں JSON کنکال کاپی (یا تخلیق کریں)
`observability/dashboards/sorafs_provider_admission.json` پھر صرف اپ ڈیٹ کریں
uid ڈیٹا سورس ؛ پینل آئی ڈی اور الرٹ کے قواعد ذیل میں رن بکس میں استعمال ہوتے ہیں ، لہذا ایسا نہ کریں
اس دستاویزات کو اپ ڈیٹ کیے بغیر ان کی بازیافت کریں۔

سہولت کے ل the ، ذخیرہ پہلے ہی ایک ریفرنس ڈیش بورڈ کی تعریف پر مشتمل ہے
`docs/source/grafana_sorafs_admission.json` ؛ اسے اپنے Grafana فولڈر میں کاپی کریں ،
اگر آپ کو مقامی جانچ کے لئے ابتدائی آپشن کی ضرورت ہو۔

### انتباہ کے قواعد Prometheusمندرجہ ذیل قواعد کے گروپ کو شامل کریں
`observability/prometheus/sorafs_admission.rules.yml` (اگر یہ فائل بنائیں
پہلا قاعدہ گروپ SoraFS) اور اسے کنفیگریشن Prometheus میں مربوط کریں۔
اپنے آن کال گردش کے ل I `<pagerduty>` کو حقیقی روٹنگ لیبل کے ساتھ تبدیل کریں۔

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
تبدیلیاں جمع کروانے سے پہلے اس بات کو یقینی بنائیں کہ نحو گزر جائے
`promtool check rules`۔

## مطابقت میٹرکس

| خصوصیات کا اشتہار | R0 | R1 | R2 | R3 |
| ------------------------- | ---- | ---- | ---- | ---- | ---- |
| `profile_id = sorafs.sf1@1.0.0` ، `chunk_range_fetch` موجود ، کیننیکل عرفی ، `signature_strict=true` | ✅ | ✅ | ✅ | ✅ |
| کوئی صلاحیت نہیں `chunk_range_fetch` | ⚠ انتباہ (انجسٹ + ٹیلی میٹری) | ⚠ انتباہ | ❌ مسترد (`reason="missing_capability"`) | ❌ ریجیکٹ |
| TLV نامعلوم صلاحیت `allow_unknown_capabilities=true` کے بغیر | ✅ | ⚠ انتباہ (`reason="unknown_capability"`) | ❌ ریجیکٹ | ❌ ریجیکٹ |
| میعاد ختم `refresh_deadline` | ❌ ریجیکٹ | ❌ ریجیکٹ | ❌ ریجیکٹ | ❌ ریجیکٹ |
| `signature_strict=false` (تشخیصی فکسچر) | ✅ (صرف ترقی) | ⚠ انتباہ | ⚠ انتباہ | ❌ ریجیکٹ |

ہر وقت UTC میں ہوتا ہے۔ نفاذ کی تاریخیں ہجرت کے لیجر میں جھلکتی ہیں اور نہیں ہیں
کونسل کے ووٹ کے بغیر تبدیل کیا جائے گا۔ کسی بھی تبدیلیوں کو اس کی تازہ کاری کی ضرورت ہوتی ہے
ایک PR میں فائل اور لیجر۔

> ** عمل درآمد نوٹ: ** R1 `result="warn"` سیریز میں متعارف کراتا ہے
> `torii_sorafs_admission_total`۔ انجشن پیچ Torii ، ایک نیا لیبل شامل کرنا ،
> SF-2 ٹیلی میٹری کے کاموں کے ساتھ مل کر نگرانی کی گئی۔ اس سے ٹکرا جانے سے پہلے اسے استعمال کریں

## مواصلات اور واقعہ سے نمٹنے کے

- ** ہفتہ وار اسٹیٹس ای میل۔ ** ڈیوریل میٹرکس کا خلاصہ بھیجتا ہے
  داخلہ ، موجودہ انتباہات اور آنے والی ڈیڈ لائن۔
- ** واقعہ کا جواب۔ ** اگر الرٹس `reject` متحرک ہو تو ، آن کال انجینئرز:
  1. ڈسکوری Torii (`/v1/sorafs/providers`) کے ذریعے پریشانی کے اشتہار کو ہٹا دیں۔
  2. فراہم کنندہ پائپ لائن میں اشتہار کی توثیق کو دہرائیں اور اس کے ساتھ موازنہ کریں
     `/v1/sorafs/providers` غلطی کو دوبارہ پیش کرنے کے لئے۔
  3. اگلی ریفریش ڈیڈ لائن تک فراہم کنندہ کے ساتھ اشتہار کی گردش کو مربوط کریں۔
- ** منجمد تبدیلیاں۔ ** R1/R2 میں اسکیما کی صلاحیتوں میں کوئی تبدیلی نہیں ، اگر
  رول آؤٹ کمیٹی منظور نہیں کرے گی۔ صرف ہفتہ وار بنیادوں پر چکنائی کے ٹیسٹ کروائیں۔
  بحالی ونڈو اور اسے ہجرت لیجر میں ریکارڈ کریں۔

## لنکس

- [SoraFS نوڈ/کلائنٹ پروٹوکول] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/sorafs_node_client_protocol.md)
- [فراہم کنندہ داخلہ پالیسی] (./provider-admission-policy)
- [ہجرت روڈ میپ] (./migration-roadmap)
- [فراہم کنندہ ایڈورٹ ملٹی سورس ایکسٹینشن] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md)