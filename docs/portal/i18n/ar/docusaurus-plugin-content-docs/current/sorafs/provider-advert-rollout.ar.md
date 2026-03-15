---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/provider-advert-rollout.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
title: "خطة تخطيط وتوافق اعلانات لميزود SoraFS"
---

> مقتبس من [`docs/source/sorafs/provider_advert_rollout.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_rollout.md).

#خطة تخطيط وتوافق اعلانات لمزودي SoraFS

تم تهيئة هذه البناء للانتقال من الإعلانات بشكل مثالي لميزود للتخزين إلى سطح `ProviderAdvertV1`
الخاضعة للتكامل الكامل والمطلوب للاسترجاع الكامل للأجزاء المتعددة. وهي تعمل على
ثلاثة مخرجات رئيسية:

- **دليل المشغلات.** خطوات عديدة يجب على تشغيلها أو تشغيلها قبل تفعيل كل البوابة.
- **تغطية التليميترية.** لوحات معلومات وتنبيهات يستخدمها Observability و Ops
  للتأكد من أن الشبكة تقبل الإعلانات المتوافقة فقط.
- **الجدول الزمني للتوافق.** مواعيد تحديد رفض المظاريف حتى البداية القديمة
  فرق SDK والأدوات من التخطيط لإصدارات البيانات.

يلائم الطرح مع معالم SF-2b/2c في
[خارطة طريق هجرة SoraFS](./migration-roadmap) ونفترض أن نأخذ في التقدم
[سياسة قبول المزود](./provider-admission-policy) مطبقة بالفعل.

##الجدول الزمني للمراحل| المرحلة | النافذة (الهدف) | سلوك | الإجراءات اللازمة | تأثير التركيز |
|-------|-----------------|-----------|------------------|-------------------|
| **R0 - المزايا الرئيسية** | حتى **2025-03-31** | يقبل Torii كلا من الإعلانات المعتمدة من الـ والـ الحمولات القديمة التي تسبق `ProviderAdvertV1`. التحقيق في جرائم الابتلاع عندما أعلن تهمل عن `chunk_range_fetch` أو `profile_aliases` القياسي. | - إعادة إنشاء الإعلانات عبر خط الأنابيب نشر إعلان المزود (ProviderAdvertV1 + مظروف الإدارة) مع ضمان `profile_id=sorafs.sf1@1.0.0` و`profile_aliases` الطويل و`signature_strict=true`. - تشغيل السيولة `sorafs_fetch` محليا؛ يجب فرز تحذيرات قدرات غير معروفة. | نشر اللوحات Grafana المطر (انظر أدناه) وعتبات التنبيه مع إبقائها في وضع التحذير فقط. |
| **R1 - بوابة التحذير** | ** 01-04-2025 → 15-05-2025** | واستمر Torii في قبول الإعلانات القديمة لكنه يزيد `torii_sorafs_admission_total{result="warn"}` عندما يفتقد الحمولة `chunk_range_fetch` أو عصر القدرات غير المعروفة دون `allow_unknown_capabilities=true`. يفشل Tooling CLI الآن في إعادة التوليد ما لم يوجد الـ Handle القياسي. | - اعلانات حيوية في التدريج والإنتاج لضمين الحمولات `CapabilityType::ChunkRangeFetch`، اختبار GREASE اضبط `allow_unknown_capabilities=true`. - إشارة واضحة إلى أنها جديدة للترشيح في runbooks. | ترقية لوحات المعلومات إلى حركة عند الطلب؛ إعداد تحذيرات عندما تتجاوز المشهد `warn` بنسبة 5% من الحركة لمدة 15 دقيقة. || **R2 - التنفيذ** | **16-05-2025 → 30-06-2025** | رفض إعلانات Torii التي تفتقد المغلفات الـتورو أو الـ المقبض القياسي للملف الشخصي أو القدرة `chunk_range_fetch`. لم تعد المقابض القديمة `namespace-name` تُحلل. فشل القدرات غير معروف دون GREASE opt-in بسبب `reason="unknown_capability"`. | - يجب عدم وجود إنتاج مظاريف ضمن `torii.sorafs.admission_envelopes_dir` وتدوير أي إعلانات قديمة متوقفة. - التحقق من أن أدوات تطوير البرامج (SDKs) أصبحت تتعامل مع الأسماء المستعارة فقط اختيارية للتوافق العكسي. | تشغيل تنبيهات جهاز النداء: `torii_sorafs_admission_total{result="reject"}` > 0 لمدة 5 دقائق لا يسبب ضررًا. تتبع نسبه وهيستوغرامات سابقه. |
| **R3 - إيقاف القديمة** | **اعتبارا من 2025-07-01** | مايكروسوفت Discovery يدعم الإعلانات الثنائية التي لا تضبط `signature_strict=true` أو التي تفتقد `profile_aliases`. يقوم Torii Discovery Cache بحذف الخطوات القديمة التي تجاوزت الموعد النهائي للتجديد دون تحديث. | - جدولة نافذة الخروج من الخدمة لمجهزات المزودين القديمة. - تشغيل أغنية من أنات GREASE `--allow-unknown` تتم فقط من خلال التدريبات مضبوطة ويصبح تسجيلها. - تحديث قواعد اللعبة للوادث لا اعتبارات قضائية `sorafs_fetch` مانعا قبل الموافقة. | تنبه التنبيهات القوية: أي نتيجة `warn` تنبيه عند الطلب. إضافة فحوصات تركيبية تسبب Discovery JSON وتتحقق من قوائم القدرات للمزودين. |

##قائمة الخيارات1. ** مجرد اعلانات.** احصر كل اعلان منشور:
   - مسار الـ الحاكم للمظروف (`defaults/nexus/sorafs_admission/...` أو ما بقيه في الإنتاج).
   - `profile_id` و `profile_aliases` للإعلان.
   - قائمة القدرات (تتوقع على الأقل `torii_gateway` و `chunk_range_fetch`).
   - علم `allow_unknown_capabilities` (مطلوب عندما توجد TLVs محجوزة من البائع).
2. ** إعادة التوليد باستخدام الأدوات المزودة .**
   - إعادة بناء الحمولة عبر إعلان مزود ناشر، مع كاتب من:
     -`profile_id=sorafs.sf1@1.0.0`
     - `capability=chunk_range_fetch` مع `max_span` محدد
     - `allow_unknown_capabilities=<true|false>` عند وجود TLVs من الشحوم
   - تحقق عبر `/v1/sorafs/providers` و`sorafs_fetch`; يجب فرز التحذيرات
     القدرات غير معروفة.
3. **التحقق من جاهزية متعددة المصادر .**
   - ينفذ `sorafs_fetch` مع `--provider-advert=<path>`؛ لم يفشل CLI الآن عندما
     يغيب `chunk_range_fetch` طبع ويحذرات عند تجاهل القدرات غير المعروفة.
     تم تحرير تقرير JSON ومحاولة اكتشافه مع أرشيف العمليات.
4. ** تجهيز الطازجات . **
   - أرسل مظاريف من النوع `ProviderAdmissionRenewalV1` قبل 30 يوما على الأقل من
     التنفيذ في البوابة (R2). يجب أن تحافظ على البيانات على الـ Handle الكلاسيكي
     قدرات NNR؛ ولا تحمل إلا الحصة أو نقاط النهاية أو البيانات الوصفية.
5. **التواصل مع الفرق المعتمدة.**
   - يجب على ملاك SDK تخصيص نسخة التنبيهات للمشغلين عندما تُرفض الإعلانات.
   - أعلن DevRel كل مرحلة انتقالية؛ أدرج روابط لوحات المعلومات ومنطق العتبات أدناه.
6. **تثبيت لوحات المعلومات والتنبيهات.**- استورد تصدير Grafana وضعه تحت **SoraFS / Provider Rollout** بمعرف UID
     `sorafs-provider-admission`.
   - تأكد من أن قواعد التنبيه إلى القناة `sorafs-advert-rollout` المشتركة
     في التدريج والإنتاج.

## التليميترية ولوحات المعلومات

المواد التالية مدمجة بالفعل عبر `iroha_telemetry`:

- `torii_sorafs_admission_total{result,reason}` — يفضل والرفض ونتائج التحذير.
  تشمل `missing_envelope` و`unknown_capability` و`stale` و`policy_violation`.

تصدير Grafana: [`docs/source/grafana_sorafs_admission.json`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/grafana_sorafs_admission.json).
قم باستيراد الملف إلى مستودع لوحات المعلومات (`observability/dashboards`) مع
تحديث UID لمصدر البيانات فقط قبل النشر.

يُنشر اللوح تحت المجلد Grafana **SoraFS / Provider Rollout** مع UID ثابت
`sorafs-provider-admission`. متطلبات التنبيه `sorafs-admission-warn` (تحذير) و
`sorafs-admission-reject` (حرج) مُعد مسبقًا لتقديم التنبيه
`sorafs-advert-rollout`؛ عدل مكان الاتصال إذا تغيّرت قائمة الوجهات البديلة للتحرير
JSON الخاص باللوحة.

لوحات Grafana لها:| اللوحة | إلا | مذكرة |
|-------|-------|-------|
| **معدل نتائج القبول** | `sum by(result)(rate(torii_sorafs_admission_total[5m]))` | مخطط متوسط ​​للعرض قبول مقابل تحذير مقابل رفض. تنبيه عند التحذير > 0.05 * الإجمالي (تحذير) أو الرفض > 0 (حرج). |
| **نسبة التحذير** | `sum(rate(torii_sorafs_admission_total{result="warn"}[5m])) / sum(rate(torii_sorafs_admission_total[5m]))` | سلسلة زمنية وحيدة تغذي مؤشر النداء (نسبة تحذير 5% خلال 15 دقيقة). |
| **أسباب الرفض** | `sum by(reason)(rate(torii_sorafs_admission_total{result="reject"}[5m]))` | دعم الفرز في دليل التشغيل؛ أرفق روابط لخطوات غير. |
| **تحديث الديون** | `sum(rate(torii_sorafs_admission_total{reason="stale"}[1h]))` | تشير إلى مارين فتهم مهلة النظيفة؛ قارن مع أرشيفات اكتشاف ذاكرة التخزين المؤقت. |

مصنوعات يدوية لـ CLI من أجل لوحات المعلومات:

- `sorafs_fetch --provider-metrics-out` كاتب عدادات `failures` و`successes` و
  `disabled` لكل المجالات. استردها في لوحات المعلومات للتشغيل الجاف المخصص
  منسق قبل التغيير الإنتاجي.
-بحثا `chunk_retry_rate` و`provider_failure_rate` في تقرير JSON يسلطان الضوء على
  الاختناق أو أعراض الحمولات النافعة التي لا معنى لها والتي غالبًا ما نسبق قبول الرفض.

### تخطيط لوحة Grafana

منشور لوحة إمكانية الملاحظة مخصصة — **SoraFS Provider Admission
الطرح** (`sorafs-provider-admission`) — ضمن **SoraFS / طرح الموفر**
مع البطاقات اللاصقة القياسية التالية:- اللوحة 1 — *معدل نتائج القبول* (منطقة مكدسة، وحدة "ops/min").
- اللوحة 2 — *نسبة التحذير* (سلسلة فردية)، مع التعبير
  `sum(معدل(torii_sorafs_admission_total{result="warn"}[5m])) /
   sum(rate(torii_sorafs_admission_total[5m]))`.
- اللوحة 3 — *أسباب الرفض* (سلسلة زمنية مجمعة حسب `reason`)، مرتبة حسب الطلب
  `rate(...[5m])`.
- اللوحة 4 — *تحديث الديون* (الإحصائيات)، بوضوح أعلاه ومُعلق بمهل التحديث
  المستخرج من دفتر حسابات الهجرة.

انسخ (أو أنشئ) هيكل عظمي JSON في مستودع شبكات الاتصال العصبية
`observability/dashboards/sorafs_provider_admission.json`، ثم حدث فقط UID لمصدر
بيانات البيانات؛ معرفات اللوحات وقواعد الإشعارات مرجعها runbooks أدناه، لذا تجنبها
إعادة ترقيمها دون تحديث هذا المستند.

لتسهيل، يوفر مستودع تعريفا مرجعيا للوحة في
`docs/source/grafana_sorafs_admission.json`؛ انسخه إلى المجلد Grafana عند الحاجة
كنقطة اختبار للاختبارات المحلية.

### متطلبات تنبيه Prometheus

أضف مجموعة التعليمات التالية إلى
`observability/prometheus/sorafs_admission.rules.yml` (أ الملف المنشئ إن كانت هذه
أول مجموعة متطلبات SoraFS) وضمنها في إعدادات Prometheus. استبدل `<pagerduty>`
بعلامة موجهة للدوام المناوبة.

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

شغّل `scripts/check_prometheus_rules.sh observability/prometheus/sorafs_admission.rules.yml`
قبل رفع الضغطات لكي تؤكد من أن الصياغة تمر عبر `promtool check rules`.

## مصفوفة التوافق| خصائص اعلان | ص0 | ر1 | ر2 | ر3 |
|------------------------|----|----|----|----|
| `profile_id = sorafs.sf1@1.0.0`، `chunk_range_fetch` موجود، الأسماء المستعارة الماضية، `signature_strict=true` | ✅ | ✅ | ✅ | ✅ |
| قدرة غياب `chunk_range_fetch` | ⚠️ تحذير (الاستيعاب + القياس عن بعد) | ⚠️ تحذير | ❌ رفض (`reason="missing_capability"`) | ❌ أرفض |
| TLVs لقدرة غير معروفة دون `allow_unknown_capabilities=true` | ✅ | ⚠️ تحذير (`reason="unknown_capability"`) | ❌ أرفض | ❌ أرفض |
| `refresh_deadline` منتهي | ❌ أرفض | ❌ أرفض | ❌ أرفض | ❌ أرفض |
| `signature_strict=false` (التركيبات التشخيصية) | ✅ (للتطوير فقط) | ⚠️ تحذير | ⚠️ تحذير | ❌ أرفض |

كل الأوقات بتوقيت UTC. تاريخ إنفاذ معكوسة في دفتر الأستاذ الخاص بالهجرة
بدون تصويت المجلس؛ أي تغيير يتطلب تحديث هذا الملف والدفتر في نفس العلاقات العامة.

> **ملاحظة تنفيذية:** يقدم سلسلة R1 `result="warn"` في
> `torii_sorafs_admission_total`. تتبع رقعة استيعاب في Torii التي تم تحميل هذه التسمية
> ضمن مهام تليمترية SF-2؛ وحتى ذلك يمكن استخدامه لعينة من السجلات

##التواصل ومعالجة- **رسالة حالة أسبوعية.** يرسل DevRel ملخصا موجزا لتقنيات التقنيات والتحذيرات
  والمواعيد القادمة.
- **استجابة للحوادث.** إذا أطلقت تنبيهات `reject`، فتقوم عند الطلب بما يلي:
  1. إعلان جلبة المخالف عبر الاكتشاف في Torii (`/v1/sorafs/providers`).
  2. إعادة تشغيل النتائج تحقق من الإعلان في خط الأنابيب المزود وقارن معه
     `/v1/sorafs/providers` إعادة إنتاجه.
  3. حتماً مع ميزة إعادة تدوير الإعلان قبل التحديث التالي.
- ** تجميد التغييرات.** لا تغيرات على المخطط للـ القدرات خلال R1/R2 ما لم
  يوافقون عليها فريق الطرح؛ يجب أن تكون هناك خطة لتجارب GREASE ضمن نافذة الصيانة الأسبوعية
  وتسجيلها في دفتر حسابات الهجرة.

## المراجع

- [SoraFS بروتوكول العقدة/العميل](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/sorafs_node_client_protocol.md)
- [سياسة قبول الموفر](./provider-admission-policy)
- [خريطة طريق الهجرة](./migration-roadmap)
- [إعلان الموفر عن ملحقات متعددة المصادر](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md)