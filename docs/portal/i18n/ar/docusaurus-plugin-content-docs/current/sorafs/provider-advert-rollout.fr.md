---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/provider-advert-rollout.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
العنوان: "خطة نشر إعلانات الموفرين SoraFS"
---

> التكيف مع [`docs/source/sorafs/provider_advert_rollout.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_rollout.md).

# خطة نشر إعلانات مقدمي الخدمة SoraFS

تقوم هذه الخطة بتنسيق قاعدة الإعلانات المسموح بها لمقدمي الخدمة على السطح
تتطلب الإدارة الكاملة `ProviderAdvertV1` استعادة القطع
متعدد المصادر. إنه يركز على ثلاثة أنواع من المعيشة:

- **دليل المشغل.** الإجراءات لا تفعل ما يفعله موفرو المخزون
  بوابة تيرمينر أفانت تشاك.
- **تغطية القياس عن بعد.** لوحات المعلومات والتنبيهات المتعلقة بالمراقبة والعمليات
  يُستخدم لتأكيد عدم قبول الشبكة لتوافق الإعلانات.
- **Calendrier de déploiement.** التواريخ واضحة لرفض المغلفات

يتم محاذاة النشر على القابس SF-2b/2c de la
[خريطة طريق الهجرة SoraFS](./migration-roadmap) ونفترض أن السياسة
قبول du [سياسة قبول الموفر](./provider-admission-policy) موجود الآن
vigueur.

## كرونولوجيا المراحل

| المرحلة | نافذة (cible) | السلوك | مشغل الإجراءات | التركيز على الملاحظة |
|-------|-----------------|-----------|------------------|-------------------|

## مشغل قائمة التحقق1. **Inventorier les adverts.** Lister chaque advert publié وتسجيل :
   - نظام المغلف الحكومي (`defaults/nexus/sorafs_admission/...` أو ما يعادله في الإنتاج).
   - `profile_id` و`profile_aliases` للإعلان.
   - قائمة الإمكانيات (على الأقل `torii_gateway` و`chunk_range_fetch`).
   - ضع علامة على `allow_unknown_capabilities` (يتطلب ذلك عند تقديم بائع TLVs réservés sont présents).
2. **الإنشاء باستخدام موفر الأدوات.**
   - إعادة بناء الحمولة مع إعلان مقدم الخدمة الخاص بك، وضمان:
     -`profile_id=sorafs.sf1@1.0.0`
     - `capability=chunk_range_fetch` مع `max_span` محدد
     - `allow_unknown_capabilities=<true|false>` عندما يتم تقديم TLVs GREASE
   - التحقق عبر `/v1/sorafs/providers` و`sorafs_fetch` ; تحذيرات سور دي
     القدرات inconnues doivent être triés.
3. **التحقق من الاستعداد متعدد المصادر.**
   - المنفذ `sorafs_fetch` مع `--provider-advert=<path>` ; صدى صوت CLI
     Désormais quand `chunk_range_fetch` manque وعرض تحذيرات من أجل
     القدرات لا يتجاهلها. التقط علاقة JSON وأرشفة مع
     سجلات العمليات.
4. **تحضير التجديدات.**
   - إرسال المغلفات `ProviderAdmissionRenewalV1` قبل 30 يومًا
     بوابة الإنفاذ (R2). يجب أن تحافظ التجديدات على المقبض
     Canonique et l'ensemble des القدرات ; فقط الحصة، نقاط النهاية أو
     لا البيانات الوصفية doivent المغير.
5. **التواصل مع المعدات التابعة.**- يتعين على مالكي SDK نشر الإصدارات التي تعرض التحذيرات الإضافية
     المشغلون عندما يتم رفض الإعلانات.
   - يعلن DevRel عن كل مرحلة انتقالية؛ قم بتضمين امتيازات لوحات المعلومات
     ومنطق seuil ci-dessous.
6. **لوحات معلومات التثبيت والتنبيهات.**
   - المستورد للتصدير Grafana والأداة الإضافية **SoraFS / المزود
     الطرح** مع l'UID `sorafs-provider-admission`.
   - التأكد من أن قواعد التنبيهات تشير إلى مشاركة القناة
     `sorafs-advert-rollout` في التدريج والإنتاج.

## القياس عن بعد ولوحات المعلومات

تم عرض المقاييس التالية من قبل عبر `iroha_telemetry` :

- `torii_sorafs_admission_total{result,reason}` — لحساب المقبولين والمرفوضين
  والإعلانات. تشمل الأسباب `missing_envelope`، `unknown_capability`،
  `stale`، و`policy_violation`.

تصدير Grafana : [`docs/source/grafana_sorafs_admission.json`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/grafana_sorafs_admission.json).
استيراد الملف في مخزن لوحات المعلومات (`observability/dashboards`)
واحتفظ بـ UID الخاص بمصدر البيانات فقط قبل النشر.

تم نشر اللوحة أسفل الملف Grafana **SoraFS / طرح الموفر** مع
l'UID مستقر `sorafs-provider-admission`. قواعد التنبيه
`sorafs-admission-warn` (تحذير) و`sorafs-admission-reject` (حرج) الابن
تم تكوينها مسبقًا لاستخدام سياسة الإشعارات `sorafs-advert-rollout` ؛
اضبط نقطة الاتصال هذه إذا تغيرت قائمة الوجهات تمامًا كما تريد
لو JSON دو لوحة القيادة.

يوصى بـ Panneaux Grafana :| لوحة | استعلام | ملاحظات |
|-------|-------|-------|
| **معدل نتائج القبول** | `sum by(result)(rate(torii_sorafs_admission_total[5m]))` | مخطط المكدس لتصور القبول مقابل التحذير مقابل الرفض. تنبيه عند التحذير > 0.05 * الإجمالي (تحذير) أو الرفض > 0 (حرج). |
| **نسبة التحذير** | `sum(rate(torii_sorafs_admission_total{result="warn"}[5m])) / sum(rate(torii_sorafs_admission_total[5m]))` | Timeseries à ligne Unique qui alimente le seuil pager (تحذير خاص بنسبة 5% يدور خلال 15 دقيقة). |
| **أسباب الرفض** | `sum by(reason)(rate(torii_sorafs_admission_total{result="reject"}[5m]))` | دليل فرز دليل التشغيل ; إرفاق الامتيازات مقابل خطوات التخفيف. |
| **تحديث الديون** | `sum(rate(torii_sorafs_admission_total{reason="stale"}[1h]))` | يشير إلى مقدمي الخدمة الذين حددوا الموعد النهائي للتحديث؛ Croiser مع سجلات اكتشاف ذاكرة التخزين المؤقت. |

المصنوعات اليدوية CLI من أجل لوحات المعلومات اليدوية:

- `sorafs_fetch --provider-metrics-out` كتابة أجهزة الكمبيوتر `failures`، `successes` وآخرون
  `disabled` موفر الاسمية. قم بالاستيراد من لوحات المعلومات المخصصة للمراقبة
  تعمل العمليات الجافة على تنسيق مقدمي الخدمات في الإنتاج.
- الأبطال `chunk_retry_rate` et `provider_failure_rate` du Rapport JSON
  يجب التصدي للاختناقات أو أعراض الحمولات القديمة السابقة
  souvent les rejets d'admission.

### قم بإعداد صفحة لوحة المعلومات Grafana

Observabilité publie un board dédié — **SoraFS قبول المزود
الطرح** (`sorafs-provider-admission`) — sous **SoraFS / طرح الموفر**
مع معرفات اللوحة الأساسية التالية :- اللوحة 1 — *معدل نتائج القبول* (منطقة مكدسة، وحدة "ops/min").
- اللوحة 2 — *نسبة التحذير* (سلسلة واحدة)، émettant l'expression
  `sum(معدل(torii_sorafs_admission_total{result="warn"}[5m])) /
   sum(rate(torii_sorafs_admission_total[5m]))`.
- اللوحة 3 — *أسباب الرفض* (إعادة تجميع السلاسل الزمنية حسب `reason`)، ثلاث مرات حسب
  `rate(...[5m])`.
- اللوحة 4 — *تحديث الديون* (الإحصائيات)، مع الاستعلام عن اللوحة ci-dessus et
  قم بالتعليق مع المواعيد النهائية لتحديث الإعلانات الإضافية في دفتر الأستاذ.

انسخ (أو أنشئ) ملف JSON المعبأ في مستودع لوحات المعلومات بالأشعة تحت الحمراء
`observability/dashboards/sorafs_provider_admission.json`، ثم تابعنا يوميًا
فريدة من نوعها لـ UID لمصدر البيانات؛ يتم تشغيل معرفات اللوحة وقواعد التنبيهات
المراجع من خلال دفاتر التشغيل التي لا تحتاج إلى مراجعة، تجنب استخدام الأرقام بدون
Mettre à jour cette الوثائق.

من أجل الراحة، يوفر الريبو تعريفًا مرجعيًا للوحة القيادة
`docs/source/grafana_sorafs_admission.json` ; نسخ في ملفك Grafana
إذا كنت تريد نقطة انطلاق للاختبارات المحلية.

### قواعد التنبيه Prometheus

أضف مجموعة القواعد التالية
`observability/prometheus/sorafs_admission.rules.yml` (قم بإنشاء الملف إذا كان كذلك
المجموعة الأولى من القواعد SoraFS) وتتضمن التكوين الخاص بك
Prometheus. استبدل `<pagerduty>` بملصق التوجيه الحقيقي الخاص بك
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
```قم بتنفيذ `scripts/check_prometheus_rules.sh observability/prometheus/sorafs_admission.rules.yml`
قبل إجراء التغييرات للتحقق من مرور الجملة
`promtool check rules`.

## مصفوفة النشر

| مميزات الإعلان | ص0 | ر1 | ر2 | ر3 |
|------------------------|----|----|----|----|
| `profile_id = sorafs.sf1@1.0.0`، `chunk_range_fetch` الحالي، الأسماء المستعارة canoniques، `signature_strict=true` | ✅ | ✅ | ✅ | ✅ |
| غياب القدرة `chunk_range_fetch` | ⚠️ تحذير (الاستيعاب + القياس عن بعد) | ⚠️ تحذير | ❌ رفض (`reason="missing_capability"`) | ❌ أرفض |
| قدرة TLVs غير متصلة بدون `allow_unknown_capabilities=true` | ✅ | ⚠️ تحذير (`reason="unknown_capability"`) | ❌ أرفض | ❌ أرفض |
| `refresh_deadline` انتهاء الصلاحية | ❌ أرفض | ❌ أرفض | ❌ أرفض | ❌ أرفض |
| `signature_strict=false` (تشخيص التركيبات) | ✅(développement Unique) | ⚠️ تحذير | ⚠️ تحذير | ❌ أرفض |

جميع الساعات تستخدم UTC. تم تعديل تواريخ التنفيذ في التطبيق
دفتر حسابات الهجرة ولا يجوز عدم التصويت للمجلس؛ تغيير تماما
يتطلب تحديث هذا الملف ودفتر الأستاذ في نفس العلاقات العامة.

> **ملاحظة التنفيذ:** يقدم R1 السلسلة `result="warn"` في
> `torii_sorafs_admission_total`. تصحيح الابتلاع Torii الذي أضاف الجديد
> التسمية تتبع مع لمسات القياس عن بعد SF-2 ; Jusque-là, utilisez le

## التواصل وإدارة الأحداث- **بريد إلكتروني من statut.** DevRel ينشر ملخصًا للسيرة الذاتية للمتر
  القبول والتحذيرات أثناء الدورة والمواعيد النهائية حتى النهاية.
- **الرد على الحادث.** إذا تم إلغاء التنبيهات `reject`، عند الطلب :
  1. استرد الإعلان الرائع عبر الاكتشاف Torii (`/v1/sorafs/providers`).
  2. اربط التحقق من صحة الإعلان في مزود خط الأنابيب وقارن معه
     `/v1/sorafs/providers` لإعادة إنتاج الخطأ.
  3. قم بالتنسيق مع المزود لعرض الإعلان قبل الشراء
     الموعد النهائي للتحديث.
- **جيل التغييرات.** هناك إمكانية تعديل مخطط القدرات
  R1/R2 أكثر من أن لجنة الطرح غير صالحة؛ les essais GREASE doivent
  يتم التخطيط لها خلال نافذة الصيانة الدورية والصحفية
  في دفتر حسابات الهجرة.

## المراجع

- [SoraFS بروتوكول العقدة/العميل](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/sorafs_node_client_protocol.md)
- [سياسة قبول الموفر](./provider-admission-policy)
- [خريطة طريق الهجرة](./migration-roadmap)
- [إعلان الموفر عن ملحقات متعددة المصادر](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md)