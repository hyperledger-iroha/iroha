---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/provider-advert-rollout.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
العنوان: "خطة إلغاء إعلانات الموردين SoraFS"
---

> التكيف من [`docs/source/sorafs/provider_advert_rollout.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_rollout.md).

# خطة نشر إعلانات الموردين SoraFS

يتم تنسيق هذه الخطة من خلال الإعلانات المسموح بها من قبل مقدمي الخدمة
سطح العمل `ProviderAdvertV1` مطلوب لاستعادة أصول متعددة
قطع دي. مركز بين ثلاثة تسليمات:

- **دليل المشغلين.** الخطوات التي يتبعها موفرو التخزين
  بوابة كاملة قبل كل شيء.
- **تغطية القياس عن بعد.** لوحات المعلومات والتنبيهات التي تستخدم للمراقبة والعمليات
  لتأكيد توافق الإعلانات مع قبول اللون الأحمر فقط.
  لتتمكن معدات SDK والأدوات من التخطيط لإصداراتها.

سيتم الطرح باستخدام الضربات SF-2b/2c del
[خريطة طريق الهجرة SoraFS](./migration-roadmap) ونفترض أن السياسة
القبول من [سياسة قبول الموفر](./provider-admission-policy) موجود في
قوة.

##كرونوجراما دي فاسيز

| فاس | فنتانا (objetivo) | التوافق | أعمال المشغل | التركيز على الملاحظة |
|-------|-----------------|-----------|------------------|-------------------|

## قائمة التحقق من المشغلين1. **إعلانات الجرد.** قائمة كل إعلان منشور وتسجيل:
   - مسار التحكم في المغلف (`defaults/nexus/sorafs_admission/...` أو ما يعادله في الإنتاج).
   - `profile_id` و `profile_aliases` للإعلان.
   - قائمة الإمكانيات (يرجى زيارة `torii_gateway` و`chunk_range_fetch`).
   - ضع علامة على `allow_unknown_capabilities` (يتطلب ذلك عندما يتم حجز TLVs من قبل البائع).
2. **تجديد الأدوات باستخدام موفري الخدمة.**
   - إعادة بناء الحمولة مع ناشر إعلان الموفر، مع التأكد من:
     -`profile_id=sorafs.sf1@1.0.0`
     - `capability=chunk_range_fetch` مع `max_span` محدد
     - `allow_unknown_capabilities=<true|false>` cuando haya TLVs GREASE
   - صالحة عبر `/v2/sorafs/providers` و`sorafs_fetch`؛ لاس advertencias sobre
     قدرات desconocidas يجب أن تكون triageadas.
3. **جاهزية Validar متعددة الأصول.**
   - إخراج `sorafs_fetch` مع `--provider-advert=<path>`؛ إل CLI الآن فالا
     cuando falta `chunk_range_fetch` ولدينا إعلانات للإمكانيات
     desconocidas ignoradas. التقط تقرير JSON وأرشفته بالسجلات
     العمليات.
4. **التحضير للتجديدات.**
   - إرسال المغلفات `ProviderAdmissionRenewalV1` قبل 30 يومًا على الأقل
     الإنفاذ عبر البوابة (R2). يجب أن تحافظ عمليات التجديد على المقبض
     Canonico ومجموعة القدرات؛ حصة منفردة، نقاط النهاية أو البيانات الوصفية
     كامبيار.
5. **الاتصال بالمعدات التابعة.**
   - سيسمح لمالكي SDK بتحرير الإصدارات التي تعرض الإعلانات لهمالمشغلون عندما يتم الإعلان عن شون rechazados.
   - DevRel anuncia cada transicion de fase؛ يتضمن تضمين لوحات المعلومات و la
     منطق ظلة أباجو.
6. **تثبيت لوحات المعلومات والتنبيهات.**
   - استيراد التصدير إلى Grafana والرجوع إليه **SoraFS / المزود
     الطرح** مع UID `sorafs-provider-admission`.
   - تأكد من أن أنظمة التنبيهات تتوافق مع القناة المشتركة
     `sorafs-advert-rollout` في التدريج والإنتاج.

## القياس عن بعد ولوحات المعلومات

المقاييس التالية التي يتم عرضها عبر `iroha_telemetry`:

- `torii_sorafs_admission_total{result,reason}` — cuenta aceptados, rechazados
  والنتائج مع الإعلانات. تتضمن الأسباب `missing_envelope`، `unknown_capability`،
  `stale` و `policy_violation`.

تصدير Grafana: [`docs/source/grafana_sorafs_admission.json`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/grafana_sorafs_admission.json).
استيراد الملف إلى مستودع لوحات المعلومات المقسم (`observability/dashboards`)
ويتم تحديث UID لمصدر البيانات فقط قبل النشر.

يتم نشر اللوحة أسفل سجادة Grafana **SoraFS / طرح الموفر** مع
UID المستقر `sorafs-provider-admission`. أنظمة التنبيهات
`sorafs-admission-warn` (تحذير) و `sorafs-admission-reject` (حرج)
الإعدادات المسبقة لاستخدام سياسة الإشعارات `sorafs-advert-rollout`؛
قم بضبط نقطة الاتصال هذه إذا كانت قائمة الوجهة تتغير في مكان تحريرها
JSON من لوحة القيادة.

توصيات Grafana:| لوحة | استعلام | ملاحظات |
|-------|-------|-------|
| **معدل نتائج القبول** | `sum by(result)(rate(torii_sorafs_admission_total[5m]))` | مخطط المكدس لتصور القبول مقابل التحذير مقابل الرفض. تنبيه عند التحذير > 0.05 * الإجمالي (تحذير) أو الرفض > 0 (حرج). |
| **نسبة التحذير** | `sum(rate(torii_sorafs_admission_total{result="warn"}[5m])) / sum(rate(torii_sorafs_admission_total[5m]))` | سلاسل زمنية لخط واحد يغذي ظلة الصفحة (معدل تحذير 5% متجدد لمدة 15 دقيقة). |
| **أسباب الرفض** | `sum by(reason)(rate(torii_sorafs_admission_total{result="reject"}[5m]))` | دليل الفرز للدليل؛ يتضمن الملحق خطوة للتخفيف. |
| **تحديث الديون** | `sum(rate(torii_sorafs_admission_total{reason="stale"}[1h]))` | يشير مقدمو الخدمة إلى عدم اكتمال الموعد النهائي للتحديث; Cruza Con Logs de Discovery Cache. |

Artefactos de CLI لأدلة لوحات المعلومات:

- `sorafs_fetch --provider-metrics-out` أكتب المحاسبين `failures`, `successes` y
  `disabled` بواسطة الموفر. استيراد لوحات المعلومات المخصصة للمراقبة
  عمليات تشغيل جافة للمنسق قبل تغيير مقدمي الإنتاج.
- المجالان `chunk_retry_rate` و `provider_failure_rate` من تقرير JSON
  إعادة التحكم في الاختناق أو تراكم الحمولات القديمة التي تسبق عمليات الاسترداد
  القبول.

### تخطيط لوحة القيادة Grafana

إمكانية المراقبة تنشر لوحة مخصصة — **SoraFS قبول المزود
الطرح** (`sorafs-provider-admission`) — bajo **SoraFS / طرح الموفر**
مع المعرفات التالية للوحة:- اللوحة 1 - *معدل نتائج القبول* (منطقة مكدسة، وحدة "عمليات/دقيقة").
- اللوحة 2 — *نسبة التحذير* (سلسلة واحدة)، أطلق التعبير
  `sum(معدل(torii_sorafs_admission_total{result="warn"}[5m])) /
   sum(rate(torii_sorafs_admission_total[5m]))`.
- اللوحة 3 — *أسباب الرفض* (series de Tiempo agrupada por `reason`)، تم ترتيبها من قبل
  `rate(...[5m])`.
- اللوحة 4 — *تحديث الديون* (الإحصائيات)، راجع الاستعلام عن اللوحة السابقة
  قم بتضمين المواعيد النهائية لتحديث الإعلانات الإضافية في دفتر حسابات الترحيل.

انسخ (أو أنشئ) نموذج JSON في مستودع لوحات المعلومات للبنية التحتية في
`observability/dashboards/sorafs_provider_admission.json`، تم التحديث فقط
UID لمصدر البيانات؛ تشير معرفات اللوحة وقواعد التنبيهات إلى ذلك
دفاتر التشغيل السابقة، حتى لا يتم إعادة تعدادها دون مراجعة هذه الوثائق.

لتسهيل استخدام المستودع، يتضمن تعريفًا للوحة المعلومات
مرجع en `docs/source/grafana_sorafs_admission.json`؛ copiala en tu Carpeta
Grafana إذا كنت بحاجة إلى نقطة مشاركة لاختبار المواقع.

### إعدادات التنبيهات لـ Prometheus

قم بإضافة المجموعة التالية من القواعد أ
`observability/prometheus/sorafs_admission.rules.yml` (إنشاء الملف إذا كان هذا هو الحال
المجموعة الأولية للقواعد SoraFS) وتتضمن تكوينك
Prometheus. قم باستبدال `<pagerduty>` مع إرشادات التشغيل الحقيقية لك
التناوب عند الطلب.

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
```إخراج `scripts/check_prometheus_rules.sh observability/prometheus/sorafs_admission.rules.yml`
قبل إجراء التغييرات للتأكد من أن المزامنة تمر عبر `promtool check rules`.

## ماتريز دي رولوت

| مميزات الإعلان | ص0 | ر1 | ر2 | ر3 |
|------------------------|----|----|----|----|
| `profile_id = sorafs.sf1@1.0.0`، `chunk_range_fetch` الحاضر، الأسماء المستعارة canonicos، `signature_strict=true` | ✅ | ✅ | ✅ | ✅ |
| اهتم بالقدرة `chunk_range_fetch` | ⚠️ تحذير (الاستيعاب + القياس عن بعد) | ⚠️ تحذير | ❌ رفض (`reason="missing_capability"`) | ❌ أرفض |
| قدرة TLVs على إزالة البيانات `allow_unknown_capabilities=true` | ✅ | ⚠️ تحذير (`reason="unknown_capability"`) | ❌ أرفض | ❌ أرفض |
| `refresh_deadline` انتهاء الصلاحية | ❌ أرفض | ❌ أرفض | ❌ أرفض | ❌ أرفض |
| `signature_strict=false` (تركيبات التشخيص) | ✅(تصميم منفرد) | ⚠️ تحذير | ⚠️ تحذير | ❌ أرفض |

جميع الساعات بالتوقيت العالمي المنسق. تعتبر إجراءات الإنفاذ بمثابة الهجرة
دفتر الأستاذ ولم يتم نقله بدون تصويت المجلس; أي تغيير يتطلب التحديث
هذا الأرشيف ودفتر الأستاذ في نفس العلاقات العامة.

> **ملاحظة التنفيذ:** R1 يقدم السلسلة `result="warn"` en
> `torii_sorafs_admission_total`. تصحيح الإدخال Torii الذي يضيف التحديث الجديد
> يتم وضع العلامات جنبًا إلى جنب مع أجهزة القياس عن بعد SF-2؛ hasta que llegue,

## التواصل وإدارة الأحداث- **رسالة البريد الإلكتروني للحالة.** قام DevRel بتعميم استئناف مختصر للمقاييس
  القبول والإعلانات المعلقة والمواعيد النهائية القريبة.
- **الرد على الأحداث.** إذا تم تنشيط التنبيهات `reject` عند الطلب:
  1. استرد الإعلان الرائع عبر اكتشاف Torii (`/v2/sorafs/providers`).
  2. أعد تنفيذ التحقق من صحة الإعلان في خط أنابيب الموفر والمقارنة معه
     `/v2/sorafs/providers` لإعادة إنتاج الخطأ.
  3. قم بالتنسيق مع مزود خدمة تدوير الإعلان قبل التحديث التالي
     الموعد النهائي.
- **تجميع التغييرات.** لا يتم تطبيق تغييرات مخطط القدرات
  خلال R1/R2 حتى تبدأ عملية البدء؛ محاكمات لوس ديبن الشحوم
  قم بالبرمجة خلال فترة صيانة النافذة وتسجيلها في
  دفتر الهجرة.

## المراجع

- [SoraFS بروتوكول العقدة/العميل](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/sorafs_node_client_protocol.md)
- [سياسة قبول الموفر](./provider-admission-policy)
- [خريطة طريق الهجرة](./migration-roadmap)
- [إعلان الموفر عن ملحقات متعددة المصادر](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md)