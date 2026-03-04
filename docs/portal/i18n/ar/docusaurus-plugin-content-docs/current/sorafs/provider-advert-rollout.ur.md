---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/provider-advert-rollout.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
title: "دورة الإعلان عن المشروع SoraFS وبما يتوافق مع الخطة"
---

> [`docs/source/sorafs/provider_advert_rollout.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_rollout.md) سے ماهر۔

# SoraFS دور الإعلان عن المشروع وبما يتوافق مع الخطة

هناك خطة لإعلانات الموفر المتساهلة محكومة `ProviderAdvertV1`
قطع السطح من خلال قطع الورق الجديدة هو وسيلة لاسترجاع القطع متعددة المصادر
ضروري ہے۔ هذه هي التسليمات في فوكس:

- **دليل المشغل.** والمزايا الإضافية التي يقدمها موفرو خدمات التخزين لبوابة فلپ
  كل شيء على ما يرام.
- **تغطية القياس عن بعد.** لوحات المعلومات والتنبيهات إمكانية المراقبة واستخدام العمليات
  لا يوجد عمل محدد للإعلانات المتوافقة تمامًا.
  تُصدر حزم SDK والأدوات خطة للإصدارات المجانية.

الطرح [SoraFS خريطة طريق الترحيل](./migration-roadmap) هي معالم SF-2b/2c
محاذاة وفرض الشروط [سياسة قبول المزود](./provider-admission-policy)
پہلے سے نافذ ہے۔

## الجدول الزمني للمرحلة

| المرحلة | نافذة (الهدف) | السلوك | إجراءات المشغل | التركيز على الملاحظة |
|-------|-----------------|-----------|------------------|-------------------|

## قائمة مراجعة المشغل1. **إعلانات المخزون.** هناك إعلان منشور للإعلان والتسجيل:
   - مسار المغلف الحاكم (`defaults/nexus/sorafs_admission/...` أو ما يعادل الإنتاج).
   - الإعلان `profile_id` و`profile_aliases`.
   - قائمة القدرات (من `torii_gateway` و`chunk_range_fetch`).
   - علامة `allow_unknown_capabilities` (يلزم وجود TLVs المحجوزة بواسطة البائع).
2. **إعادة الإنشاء باستخدام أدوات الموفر.**
   - يوفر ناشر الإعلانات الموفر حمولة إضافية، وأحدث الأدوات:
     -`profile_id=sorafs.sf1@1.0.0`
     - `capability=chunk_range_fetch` أو `max_span` واضح
     - GREASE TLVs التي يمكن العثور عليها `allow_unknown_capabilities=<true|false>`
   - `/v1/sorafs/providers` و`sorafs_fetch` للتحقق من صحة البطاقة؛ غير معروف
     القدرات والتحذيرات والفرز.
3. **التحقق من جاهزية المصادر المتعددة.**
   - `sorafs_fetch` إلى `--provider-advert=<path>` وهو طائر؛ اب `chunk_range_fetch`
     لم نقم بفشل CLI وتجاهلنا القدرات غير المعروفة لتحذيراتنا.
     تقرير JSON محفوظات السجلات وسجلات العمليات وأرشيف مستمر.
4. **تجديدات المرحلة.**
   - فرض البوابة (R2) على بعد 30 يومًا من `ProviderAdmissionRenewalV1`
     مظاريف جمع کریں۔ تتميز التجديدات بمقبض أساسي ومجموعة من الإمكانيات
     رہنا چہئے؛ حصة فقط، نقاط النهاية أو البيانات الوصفية بدل.
5. **التواصل مع الفرق التابعة.**
   - يقوم مالكو SDK بإصدار إصدارات جديدة عندما ترفض الإعلانات مرة واحدة فقط للمشغلين والتحذيرات.- إعلان المرحلة الانتقالية DevRel ہر کرے؛ تتضمن روابط لوحة المعلومات ومنطق العتبة کریں.
6. **تثبيت لوحات المعلومات والتنبيهات.**
   - Grafana تصدير الطاقة و **SoraFS / طرح الموفر** تحت السجل، UID
     `sorafs-provider-admission`.
   - يتم مشاركة قواعد التنبيه والتدريج والإنتاج
     `sorafs-advert-rollout` قناة الإشعارات پر جايں۔

## القياس عن بعد ولوحات المعلومات

توجد مقاييس أعلى `iroha_telemetry` لجهاز الذر:

- `torii_sorafs_admission_total{result,reason}` - مقبول، مرفوض، وتحذير
  النتائج گنتا ہے۔ الأسباب `missing_envelope`, `unknown_capability`, `stale`
  و `policy_violation` شامل.

تصدير Grafana: [`docs/source/grafana_sorafs_admission.json`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/grafana_sorafs_admission.json).
يمكن لمستودع لوحات المعلومات المشتركة (`observability/dashboards`) استيراد و
تُستخدم قاعدة بيانات UID لمصدر البيانات.

بورڈ Grafana المجلد **SoraFS / طرح الموفر** يتميز بمعرف فريد ثابت
`sorafs-provider-admission` قم بالنشر الآن. قواعد التنبيه
`sorafs-admission-warn` (تحذير) و`sorafs-admission-reject` (حرج)
تم تكوين استخدام سياسة الإشعارات `sorafs-advert-rollout`؛
إذا كانت قائمة الوجهات بدلے لوحة القيادة JSON کو تحرير کرنے کے بجائے نقطة الاتصال
تحديث کریں۔

لوحات Grafana الموصى بها:| لوحة | استعلام | ملاحظات |
|-------|-------|-------|
| **معدل نتائج القبول** | `sum by(result)(rate(torii_sorafs_admission_total[5m]))` | مخطط المكدس هو القبول مقابل التحذير مقابل الرفض. تحذير > 0.05 * الإجمالي (تحذير) أو رفض > 0 (حرج) پر تنبيه۔ |
| **نسبة التحذير** | `sum(rate(torii_sorafs_admission_total{result="warn"}[5m])) / sum(rate(torii_sorafs_admission_total[5m]))` | السلاسل الزمنية ذات السطر الواحد هي عتبة النداء للتغذية (15 ميغا بايت 5% معدل التحذير). |
| **أسباب الرفض** | `sum by(reason)(rate(torii_sorafs_admission_total{result="reject"}[5m]))` | فرز دليل التشغيل؛ خطوات التخفيف لن تشمل. |
| **تحديث الديون** | `sum(rate(torii_sorafs_admission_total{reason="stale"}[1h]))` | الموعد النهائي للتحديث يغيب عن مقدمي الخدمات کو ظہر کرتا ہے؛ تعد سجلات اكتشاف ذاكرة التخزين المؤقت بمثابة إسناد ترافقي. |

لوحات المعلومات اليدوية لمصنوعات CLI:

- `sorafs_fetch --provider-metrics-out` مزود الخدمة `failures`, `successes`,
  وعدادات `disabled` لکھتا ہے. عمليات التشغيل الجافة للمنسق ومراقبته
  يمكن استيراد لوحات المعلومات المخصصة.
- تقرير JSON اختناق الحقول `chunk_retry_rate` و`provider_failure_rate` أو
  تظهر أعراض الحمولة التي لا معنى لها ويتم رفض القبول بشكل أكبر.

### تخطيط لوحة المعلومات Grafana

إمكانية الملاحظة ایک لوحة مخصصة — **SoraFS قبول المزود
الطرح** (`sorafs-provider-admission`) — **SoraFS / طرح الموفر** تحت
نشر القائمة ومعرفات اللوحة الأساسية:- اللوحة 1 — *معدل نتائج القبول* (منطقة مكدسة، یونٹ "ops/min").
- اللوحة 2 — *نسبة التحذير* (سلسلة واحدة)، اظہار
  `sum(معدل(torii_sorafs_admission_total{result="warn"}[5m])) /
   sum(rate(torii_sorafs_admission_total[5m]))`.
- اللوحة 3 — *أسباب الرفض* (`reason` کے حساب سے السلسلة الزمنية)، `rate(...[5m])`
  حسب النوع .
- اللوحة 4 — *تحديث الديون* (الإحصائيات)، استعلام الجدول الأولي والمرآة كرتا ہے و
  سجل الهجرة قد نجح في المواعيد النهائية لتحديث الإعلانات المشروحة.

هيكل JSON للوحات معلومات البنية التحتية `observability/dashboards/sorafs_provider_admission.json`
قبل النسخ أو إنشاء ملف، استخدم ملف UID لمصدر البيانات؛ معرفات اللوحة وقواعد التنبيه
تمت الإشارة إلى بعض دفاتر التشغيل، وهي تعيد ترقيم الملفات إلى أعلى مستند docs.

لوحة القيادة المرجعية لـ Ripo `docs/source/grafana_sorafs_admission.json`
تعريف دیتا ہے؛ تم نسخ المجلد Grafana من خلال اختبار لوكل.

### قواعد التنبيه Prometheus

مندرج مجموعة القواعد `observability/prometheus/sorafs_admission.rules.yml`
تتضمن اللعبة (إذا كانت مجموعة القواعد SoraFS هي المفضلة لديك) وPrometheus
التكوين سے يشمل کریں۔ `<pagerduty>` يتم إطلاق تسمية التوجيه عند الاتصال.

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
          summary: "SoraFS provider adverts generating warnings"
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
          summary: "SoraFS provider adverts rejected"
          description: |
            Provider adverts have been rejected for the last five minutes.
            Check panel 4 (rejection reasons) and rotate envelopes before
            the refresh deadline elapses.
```

`scripts/check_prometheus_rules.sh observability/prometheus/sorafs_admission.rules.yml`
قم بقراءة بناء الجملة `promtool check rules`.

## الاتصالات والتعامل مع الحوادث- **رسالة الحالة الأسبوعية.** مقاييس قبول DevRel والتحذيرات المعلقة والمواعيد النهائية النهائية.
- **الاستجابة للحوادث.** إذا `reject` تنبيهات فورية يمكنك الاتصال عند الاتصال:
  1. اكتشاف Torii (`/v1/sorafs/providers`) هو جلب إعلان مخالف.
  2. يمكن لخط أنابيب الموفر التحقق من صحة الإعلان مرة أخرى و`/v1/sorafs/providers` ومقارنة خطأ إعادة إنتاج الخطأ.
  3. يقوم المزود بتنسيق عملية النقل قبل الموعد النهائي للتحديث قبل انتهاء الإعلان بالتناوب.
- **تغيير التجميدات.** لم يتم إعادة تشغيل R1/R2 لتدوير مخطط القدرة قبل بدء التشغيل؛ تتيح تجارب GREASE نافذة صيانة الحرب جدولة عمليات التسجيل وسجل دفتر الأستاذ الخاص بالترحيل.

## المراجع

- [SoraFS بروتوكول العقدة/العميل](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/sorafs_node_client_protocol.md)
- [سياسة قبول الموفر](./provider-admission-policy)
- [خريطة طريق الهجرة](./migration-roadmap)
- [إعلان الموفر عن ملحقات متعددة المصادر](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md)