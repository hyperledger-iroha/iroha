---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/provider-advert-rollout.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
العنوان: "مقدم خطة الطرح والتوافق SoraFS"
---

> متكيف من [`docs/source/sorafs/provider_advert_rollout.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_rollout.md).

# خطة الطرح والتوافق مع مقدم الإعلان SoraFS

هذه الخطة المنسقة مسبقة من مساعد الإعلان المسموح به إلى أقصى حد
التحكم الشامل `ProviderAdvertV1`، مطلوب للمراجعين متعددي المصادر
قطع. يتم التركيز على ثلاث نتائج:

- **مشغل روستوف.** النشاط المتميز الذي يوفره مزودو الطاقة
  يتم تشغيله من خلال تضمين البوابة.
- **المراقبة عن بعد.** لوحات المعلومات والتنبيهات التي تستخدم إمكانية المراقبة والعمليات،
  للتأكد من أن يتم تطبيق عدد كبير جدًا من الإعلانات.
  يمكن لـ SDK والأدوات التخطيط للإصدار.

دعم الطرح باستخدام SF-2b/2c
[خارطة الطريق للهجرة SoraFS](./migration-roadmap) وتقترح أن يتم توفير السياسة في
[سياسة قبول الموفر](./provider-admission-policy) تفضل.

## قواعد الجدول الزمني

| فيزا | أوكنو (цель) | ملاحظة | مشغلو الأجهزة | التركيز على الرؤية |
|-------|-----------------|-----------|------------------|-------------------|

## مشغل الاختيار1. ** قم بتنشيط الإعلانات. ** قم بتصفح كل الإعلانات المنشورة وقم بتوقيعها:
   - أدخل المظروف الحاكم (`defaults/nexus/sorafs_admission/...` أو مكافئ الإنتاج).
   - إعلان `profile_id` و`profile_aliases`.
   - إمكانيات القائمة (تتراوح على الأقل بين `torii_gateway` و`chunk_range_fetch`).
   - Flag `allow_unknown_capabilities` (تم الحجز عند الحصول على TLV المحجوز من قبل البائع).
2. **التجديد من خلال أدوات الموفر.**
   - قم بنقل الحمولة من خلال إعلان الناشر، وذلك من خلال:
     -`profile_id=sorafs.sf1@1.0.0`
     - `capability=chunk_range_fetch` مع `max_span` المتقدم
     - `allow_unknown_capabilities=<true|false>` عند الانتهاء من GREASE TLV
   - تحقق من خلال `/v1/sorafs/providers` و`sorafs_fetch`؛ المسبقة o
     من الضروري فرز القدرات غير المعروفة.
3. **التحقق من الاستعداد متعدد المصادر.**
   - قم بتفعيل `sorafs_fetch` مع `--provider-advert=<path>`؛ CLI يطير بسرعة,
     عند الخروج من `chunk_range_fetch`، وإبداء الرأي المسبق
     القدرات غير المعروفة. قم بتأكيد JSON-Otchet و
     أرشفة نفسك مع سجلات العمليات.
4. **توزيع البضائع.**
   - قم بتسليم الأظرف `ProviderAdmissionRenewalV1` لمدة لا تقل عن 30 يومًا حتى
     إنفاذ البوابة (R2). تزايد الاحتياجات المصاحبة للمقبض الكنسي و
     قدرات العمل؛ اتبع ما يلي فقط الحصة أو نقاط النهاية أو البيانات الوصفية.
5. **الاتصالات بالأوامر المخبرية.**
   - يتعين على مستخدمي SDK شراء الإصدارات التي تعرض مشغل التحذيراتقبل حجب الإعلانات.
   - DevRel يعلن عن كل فوز; قم بتضمين إعدادات لوحات المعلومات والمنطق
     شكرا جزيلا.
6. **تثبيت لوحات المعلومات والتنبيهات.**
   - استيراد تصدير Grafana واستبداله في **SoraFS / Provider
     الطرح** باستخدام UID `sorafs-provider-admission`.
   - تأكد من أن قواعد التنبيه يتم تطبيقها في القناة العامة
     `sorafs-advert-rollout` في التدريج والإنتاج.

## أجهزة القياس عن بعد ولوحة التحكم

يمكن توفير المقاييس التالية عبر `iroha_telemetry`:

- `torii_sorafs_admission_total{result,reason}` — بصمة الإصبع، الحجب
  والتحذيرات. تتضمن الأسباب `missing_envelope` و`unknown_capability` و`stale` و
  `policy_violation`.

تصدير Grafana: [`docs/source/grafana_sorafs_admission.json`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/grafana_sorafs_admission.json).
استيراد الملف إلى المستودع الرئيسي (`observability/dashboards`) و
قم بالتعرف على مصدر بيانات UID فقط قبل النشر.

يتم نشر لوحة البيانات في الورقة Grafana **SoraFS / طرح الموفر** مع
UID المستقر `sorafs-provider-admission`. قواعد التنبيه
`sorafs-admission-warn` (تحذير) و`sorafs-admission-reject` (حرج)
مقترحات السياسة العامة `sorafs-advert-rollout`; يرجى الاتصال بنا
تحصل النقطة عند تغيير القائمة على لوحة بيانات JSON الصحيحة.

اللوحة الموصى بها Grafana:| لوحة | استعلام | ملاحظات |
|-------|-------|-------|
| **معدل نتائج القبول** | `sum by(result)(rate(torii_sorafs_admission_total[5m]))` | مخطط مكدس للتصور قبول مقابل تحذير مقابل رفض. تنبيه عند التحذير > 0.05 * الإجمالي (تحذير) أو الرفض > 0 (حرج). |
| **نسبة التحذير** | `sum(rate(torii_sorafs_admission_total{result="warn"}[5m])) / sum(rate(torii_sorafs_admission_total[5m]))` | سلاسل زمنية واحدة، جهاز النداء السريع (5% معدل تحذير في كل 15 دقيقة). |
| **أسباب الرفض** | `sum by(reason)(rate(torii_sorafs_admission_total{result="reject"}[5m]))` | للفرز في دليل التشغيل; نفذ خطوات التخفيف. |
| **تحديث الديون** | `sum(rate(torii_sorafs_admission_total{reason="stale"}[1h]))` | وصف لمقدمي الخدمة, الموعد النهائي للتحديث المتوقع; اتصل بسجلات اكتشاف ذاكرة التخزين المؤقت. |

عناصر CLI للوحة المفاتيح الرائعة:

- `sorafs_fetch --provider-metrics-out` رساله الكمبيوتر `failures`, `successes` و
  `disabled` من خلال موفر الخدمة. الاستيراد في لوحات المعلومات المخصصة، لذلك
  مراقبة منسق التشغيل الجاف قبل موفري الإنتاج المستبعدين.
- `chunk_retry_rate` و `provider_failure_rate` في JSON-Outtchet
  التحقق من الاختناق أو أعراض الحمولات القديمة التي تسبق إلى حد ما
  إلغاء القبول.

### لوحة Grafana

إمكانية المراقبة من خلال لوحة отдельный — **SoraFS قبول المزود
الطرح** (`sorafs-provider-admission`) — в **SoraFS / طرح الموفر**
مع معرفات اللوحة الأساسية التالية:- اللوحة 1 — *معدل نتائج القبول* (منطقة مكدسة، وحدة "عمليات/دقيقة").
- اللوحة 2 — *نسبة التحذير* (سلسلة واحدة)، سريعة
  `sum(معدل(torii_sorafs_admission_total{result="warn"}[5m])) /
   sum(rate(torii_sorafs_admission_total[5m]))`.
- اللوحة 3 — *أسباب الرفض* (السلاسل الزمنية، التوزيع حسب `reason`)، التصنيف حسب
  `rate(...[5m])`.
- اللوحة 4 — *تحديث الديون* (الإحصائيات)، وإنهاء الإجراءات من الأجهزة اللوحية الأحدث،
  تعليق تحديث الموعد النهائي من دفتر أستاذ الترحيل.

نسخ (أو إنشاء) هيكل JSON في مستودعات البنية التحتية
`observability/dashboards/sorafs_provider_admission.json`، ثم قم بالتعرف على المزيد
مصدر بيانات UID؛ يتم استخدام معرفات اللوحة وقواعد التنبيه في دفاتر التشغيل، ولكن ليس كذلك
قم بإعادة ترقيمها دون الإشارة إلى هذه الوثائق.

للحصول على المستودع، يمكنك أيضًا تنسيق التعريف المرجعي للوحة المعلومات في
`docs/source/grafana_sorafs_admission.json`; انسخ نفسك في ورق Grafana الخاص بك،
إذا كان هناك بديل جديد للاختبار المحلي.

### التنبيهات الصحيحة Prometheus

إضافة المجموعة التالية التي تناسبك
`observability/prometheus/sorafs_admission.rules.yml` (قم بإنشاء الملف، إذا كان ذلك
المجموعة الأولى مناسبة SoraFS) وقم بدمجها في التكوين Prometheus.
قم بتثبيت `<pagerduty>` على تسمية التوجيه الحقيقية لعمليات التناوب عند الطلب.

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

أغلق `scripts/check_prometheus_rules.sh observability/prometheus/sorafs_admission.rules.yml`
قبل التحسين الكامل لقراءة ما يتم إنتاجه من سينتاكس
`promtool check rules`.

## ماتريتشا نوفوميستيموستيز| خصائص الإعلان | ص0 | ر1 | ر2 | ر3 |
|------------------------|----|----|----|----|
| `profile_id = sorafs.sf1@1.0.0`, `chunk_range_fetch` писутствует, الأسماء المستعارة الكنسية, `signature_strict=true` | ✅ | ✅ | ✅ | ✅ |
| القدرة الصافية `chunk_range_fetch` | ⚠️ تحذير (الاستيعاب + القياس عن بعد) | ⚠️ تحذير | ❌ رفض (`reason="missing_capability"`) | ❌ أرفض |
| قدرة TLV غير المعروفة بدون `allow_unknown_capabilities=true` | ✅ | ⚠️ تحذير (`reason="unknown_capability"`) | ❌ أرفض | ❌ أرفض |
| `refresh_deadline` | ❌ أرفض | ❌ أرفض | ❌ أرفض | ❌ أرفض |
| `signature_strict=false` (التركيبات التشخيصية) | ✅(تطوير فقط) | ⚠️ تحذير | ⚠️ تحذير | ❌ أرفض |

كل وقت السفر بالتوقيت العالمي المنسق. يتم فرض البيانات في دفتر أستاذ الهجرة وليس كذلك
تم التوسيع بدون مجلس الهول; التحسين المفضل هو هذا الأمر
الملف ودفتر الأستاذ في علاقات عامة واحدة.

> **المساعدة بعد التنفيذ:** R1 يتدفق إلى السلسلة `result="warn"` в
> `torii_sorafs_admission_total`. ابتلاع التصحيح Torii, إضافة تسمية جديدة,
> يتم التحكم فيه عن بعد بواسطة أجهزة قياس عن بعد SF-2; استخدمها لنشرها

## حوادث الاتصالات والتقنية- **حالة السيرة الذاتية.** DevRel يتتبع مقياس السيرة الذاتية
  القبول والتحذيرات الدقيقة والمواعيد النهائية السابقة.
- **الاستجابة للحوادث.** إذا قمت بإعداد التنبيهات `reject`، مهندسون عند الطلب:
  1. قم بإزالة الإعلان الإشكالي من خلال الاكتشاف Torii (`/v1/sorafs/providers`).
  2. قم بالتحقق من صحة الإعلان في خط أنابيب الموفر واتصل به
     `/v1/sorafs/providers`، للتحدث.
  3. قم بالتنسيق مع توفير التناوب للإعلان حتى الموعد النهائي للتحديث التالي.
- **التحسينات المتغيرة.** بعض إمكانيات المخطط المتغير في R1/R2، إذا
  لا يتم قبول طرح اللجنة؛ يؤدي تصريف الشحوم إلى زيادة كبيرة في استهلاك الطاقة
  انقر فوق الخدمة والتثبيت في دفتر أستاذ الهجرة.

## مرحبا

- [SoraFS بروتوكول العقدة/العميل](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/sorafs_node_client_protocol.md)
- [سياسة قبول الموفر](./provider-admission-policy)
- [خريطة طريق الهجرة](./migration-roadmap)
- [إعلان الموفر عن ملحقات متعددة المصادر](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md)