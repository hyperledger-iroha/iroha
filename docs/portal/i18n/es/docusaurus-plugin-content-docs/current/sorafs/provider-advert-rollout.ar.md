---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/provider-advert-rollout.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
title: "خطة طرح وتوافق adverts لمزودي SoraFS"
---

> مقتبس من [`docs/source/sorafs/provider_advert_rollout.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_rollout.md).

# خطة طرح وتوافق adverts لمزودي SoraFS

تنسق هذه الخطة الانتقال من adverts المسموحة لمزودي التخزين إلى سطح `ProviderAdvertV1`
الخاضع للحوكمة بالكامل والمطلوب للاسترجاع متعدد المصادر للـ chunks. وهي تركز على
ثلاثة مخرجات رئيسية:

- **دليل المشغل.** خطوات يجب على مزودي التخزين إكمالها قبل تفعيل كل gate.
- **تغطية التليمترية.** لوحات معلومات وتنبيهات تستخدمها Observability و Ops
  للتأكد من أن الشبكة تقبل adverts المتوافقة فقط.
- **الجدول الزمني للتوافق.** تواريخ واضحة لرفض envelopes القديمة حتى تتمكن
  فرق SDK و tooling من التخطيط لإصداراتها.

يتماشى الطرح مع معالم SF-2b/2c في
[خارطة طريق هجرة SoraFS](./migration-roadmap) ويفترض أن سياسة القبول في
[provider admission policy](./provider-admission-policy) مطبقة بالفعل.

## الجدول الزمني للمراحل

| المرحلة | النافذة (الهدف) | السلوك | إجراءات المشغل | تركيز الملاحظة |
|-------|-----------------|-----------|------------------|-------------------|
| **R0 - الملاحظة الأساسية** | حتى **2025-03-31** | يقبل Torii كلا من adverts المعتمدة من الحوكمة والـ payloads القديمة التي تسبق `ProviderAdvertV1`. تسجل سجلات ingestion تحذيرات عندما تهمل adverts `chunk_range_fetch` أو `profile_aliases` القياسية. | - إعادة توليد adverts عبر pipeline نشر provider advert (ProviderAdvertV1 + governance envelope) مع ضمان `profile_id=sorafs.sf1@1.0.0` و`profile_aliases` قياسية و`signature_strict=true`. <br />- تشغيل اختبارات `sorafs_fetch` محليا؛ يجب triage تحذيرات capabilities غير المعروفة. | نشر لوحات Grafana المؤقتة (انظر أدناه) وتحديد عتبات التنبيه مع إبقائها في وضع التحذير فقط. |
| **R1 - بوابة التحذير** | **2025-04-01 → 2025-05-15** | يستمر Torii في قبول adverts القديمة لكنه يزيد `torii_sorafs_admission_total{result="warn"}` عندما يفتقد payload `chunk_range_fetch` أو يحمل capabilities غير معروفة دون `allow_unknown_capabilities=true`. يفشل tooling CLI الآن في إعادة التوليد ما لم يوجد الـ handle القياسي. | - تدوير adverts في staging و production لتضمين payloads `CapabilityType::ChunkRangeFetch`، وعند GREASE testing اضبط `allow_unknown_capabilities=true`. <br />- توثيق الاستعلامات الجديدة للتليمترية في runbooks التشغيل. | ترقية dashboards إلى دوران on-call؛ إعداد تحذيرات عندما تتجاوز أحداث `warn` نسبة 5% من الحركة لمدة 15 دقيقة. |
| **R2 - Enforcement** | **2025-05-16 → 2025-06-30** | يرفض Torii adverts التي تفتقد envelopes الحوكمة أو الـ handle القياسي للملف الشخصي أو capability `chunk_range_fetch`. لم تعد handles القديمة `namespace-name` تُحلل. تفشل capabilities غير المعروفة دون GREASE opt-in بسبب `reason="unknown_capability"`. | - التأكد من وجود envelopes الإنتاج ضمن `torii.sorafs.admission_envelopes_dir` وتدوير أي adverts قديمة متبقية. <br />- التحقق من أن SDKs تصدر handles قياسية فقط مع aliases اختيارية للتوافق العكسي. | تشغيل pager alerts: `torii_sorafs_admission_total{result="reject"}` > 0 لمدة 5 دقائق يستدعي تدخل المشغل. تتبع نسبة القبول وهيستوغرامات أسباب القبول. |
| **R3 - إيقاف القديمة** | **اعتبارا من 2025-07-01** | تتخلى Discovery عن دعم adverts الثنائية التي لا تضبط `signature_strict=true` أو التي تفتقد `profile_aliases`. يقوم Torii discovery cache بحذف الإدخالات القديمة التي تجاوزت deadline للتجديد دون تحديث. | - جدولة نافذة decommission النهائية لمكدسات المزودين القديمة. <br />- التأكد من أن تشغيلات GREASE `--allow-unknown` تتم فقط خلال drills مضبوطة ويتم تسجيلها. <br />- تحديث playbooks للحوادث لاعتبار مخرجات تحذير `sorafs_fetch` مانعا قبل الإصدارات. | تشديد التنبيهات: أي نتيجة `warn` تنبه on-call. إضافة فحوصات تركيبية تجلب discovery JSON وتتحقق من قوائم capabilities للمزودين. |

## قائمة تحقق المشغل

1. **جرد adverts.** احصر كل advert منشور وسجل:
   - مسار الـ governing envelope (`defaults/nexus/sorafs_admission/...` أو ما يعادله في الإنتاج).
   - `profile_id` و `profile_aliases` للـ advert.
   - قائمة capabilities (تتوقع على الأقل `torii_gateway` و `chunk_range_fetch`).
   - علم `allow_unknown_capabilities` (مطلوب عندما توجد TLVs محجوزة من vendor).
2. **إعادة التوليد باستخدام tooling المزود.**
   - أعد بناء payload عبر ناشر provider advert، مع التأكد من:
     - `profile_id=sorafs.sf1@1.0.0`
     - `capability=chunk_range_fetch` مع `max_span` محدد
     - `allow_unknown_capabilities=<true|false>` عند وجود TLVs من GREASE
   - تحقق عبر `/v1/sorafs/providers` و`sorafs_fetch`; يجب triage تحذيرات
     capabilities غير المعروفة.
3. **التحقق من جاهزية multi-source.**
   - نفذ `sorafs_fetch` مع `--provider-advert=<path>`؛ يفشل CLI الآن عندما
     يغيب `chunk_range_fetch` ويطبع تحذيرات عند تجاهل capabilities غير المعروفة.
     التقط تقرير JSON وأرشفه مع سجلات العمليات.
4. **تجهيز التجديدات.**
   - أرسل envelopes من نوع `ProviderAdmissionRenewalV1` قبل 30 يوما على الأقل من
     enforcement في gateway (R2). يجب أن تحافظ التجديدات على الـ handle القياسي
     ومجموعة capabilities؛ ولا تتغير إلا stake أو endpoints أو metadata.
5. **التواصل مع الفرق المعتمدة.**
   - يجب على ملاك SDK إطلاق نسخ تُظهر التحذيرات للمشغلين عندما تُرفض adverts.
   - يعلن DevRel كل انتقال مرحلة؛ أدرج روابط dashboards ومنطق العتبات أدناه.
6. **تثبيت dashboards والتنبيهات.**
   - استورد تصدير Grafana وضعه تحت **SoraFS / Provider Rollout** مع UID
     `sorafs-provider-admission`.
   - تأكد من أن قواعد التنبيه تشير إلى قناة `sorafs-advert-rollout` المشتركة
     في staging و production.

## التليمترية ولوحات المعلومات

المقاييس التالية مكشوفة بالفعل عبر `iroha_telemetry`:

- `torii_sorafs_admission_total{result,reason}` — يعد القبول والرفض ونتائج التحذير.
  تشمل الأسباب `missing_envelope` و`unknown_capability` و`stale` و`policy_violation`.

تصدير Grafana: [`docs/source/grafana_sorafs_admission.json`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/grafana_sorafs_admission.json).
قم باستيراد الملف إلى مستودع dashboards المشترك (`observability/dashboards`) مع
تحديث UID لمصدر البيانات فقط قبل النشر.

يُنشر اللوح تحت مجلد Grafana **SoraFS / Provider Rollout** مع UID ثابت
`sorafs-provider-admission`. قواعد التنبيه `sorafs-admission-warn` (warning) و
`sorafs-admission-reject` (critical) مُعدة مسبقا لاستخدام سياسة الإشعار
`sorafs-advert-rollout`؛ عدل جهة الاتصال إذا تغيّرت قائمة الوجهات بدلا من تحرير
JSON الخاص باللوحة.

لوحات Grafana الموصى بها:

| اللوحة | الاستعلام | الملاحظات |
|-------|-------|-------|
| **Admission outcome rate** | `sum by(result)(rate(torii_sorafs_admission_total[5m]))` | مخطط مكدس لعرض accept vs warn vs reject. تنبيه عند warn > 0.05 * total (warning) أو reject > 0 (critical). |
| **Warning ratio** | `sum(rate(torii_sorafs_admission_total{result="warn"}[5m])) / sum(rate(torii_sorafs_admission_total[5m]))` | سلسلة زمنية وحيدة تغذي عتبة pager (نسبة تحذير 5% خلال 15 دقيقة). |
| **Rejection reasons** | `sum by(reason)(rate(torii_sorafs_admission_total{result="reject"}[5m]))` | تدعم triage في runbook؛ أرفق روابط لخطوات التخفيف. |
| **Refresh debt** | `sum(rate(torii_sorafs_admission_total{reason="stale"}[1h]))` | تشير إلى مزودين فاتتهم مهلة التجديد؛ قارن مع سجلات discovery cache. |

Artefacts للـ CLI من أجل dashboards يدوية:

- `sorafs_fetch --provider-metrics-out` يكتب عدادات `failures` و`successes` و
  `disabled` لكل مزود. استوردها في dashboards ad-hoc لمراقبة dry-runs في
  orchestrator قبل تبديل مزودي الإنتاج.
- حقلا `chunk_retry_rate` و`provider_failure_rate` في تقرير JSON يسلطان الضوء على
  throttling أو أعراض payloads stale التي غالبا ما تسبق رفض القبول.

### تخطيط لوحة Grafana

تنشر Observability لوحة مخصصة — **SoraFS Provider Admission
Rollout** (`sorafs-provider-admission`) — ضمن **SoraFS / Provider Rollout**
مع معرفات اللوحات القياسية التالية:

- Panel 1 — *Admission outcome rate* (stacked area, وحدة "ops/min").
- Panel 2 — *Warning ratio* (single series)، مع التعبير
  `sum(rate(torii_sorafs_admission_total{result="warn"}[5m])) /
   sum(rate(torii_sorafs_admission_total[5m]))`.
- Panel 3 — *Rejection reasons* (time series مجمعة حسب `reason`)، مرتبة حسب
  `rate(...[5m])`.
- Panel 4 — *Refresh debt* (stat)، يعكس الاستعلام أعلاه ومُعلق بمهل refresh
  المستخرجة من migration ledger.

انسخ (أو أنشئ) JSON skeleton في مستودع لوحات البنية التحتية
`observability/dashboards/sorafs_provider_admission.json`، ثم حدث فقط UID لمصدر
البيانات؛ معرفات اللوحات وقواعد التنبيه يُرجع إليها runbooks أدناه، لذا تجنب
إعادة ترقيمها دون تحديث هذا المستند.

للتسهيل، يوفر المستودع تعريفا مرجعيا للوحة في
`docs/source/grafana_sorafs_admission.json`؛ انسخه إلى مجلد Grafana عند الحاجة
كنقطة انطلاق للاختبارات المحلية.

### قواعد تنبيه Prometheus

أضف مجموعة القواعد التالية إلى
`observability/prometheus/sorafs_admission.rules.yml` (أنشئ الملف إن كانت هذه
أول مجموعة قواعد SoraFS) وضمنها في إعدادات Prometheus. استبدل `<pagerduty>`
بعلامة التوجيه الفعلية لدوام المناوبة.

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
قبل رفع التغييرات للتأكد من أن الصياغة تمر عبر `promtool check rules`.

## مصفوفة التوافق

| خصائص advert | R0 | R1 | R2 | R3 |
|------------------------|----|----|----|----|
| `profile_id = sorafs.sf1@1.0.0`, `chunk_range_fetch` موجود، aliases قياسية، `signature_strict=true` | ✅ | ✅ | ✅ | ✅ |
| غياب capability `chunk_range_fetch` | ⚠️ Warn (ingest + telemetry) | ⚠️ Warn | ❌ Reject (`reason="missing_capability"`) | ❌ Reject |
| TLVs لcapability غير معروفة دون `allow_unknown_capabilities=true` | ✅ | ⚠️ Warn (`reason="unknown_capability"`) | ❌ Reject | ❌ Reject |
| `refresh_deadline` منتهي | ❌ Reject | ❌ Reject | ❌ Reject | ❌ Reject |
| `signature_strict=false` (diagnostic fixtures) | ✅ (للتطوير فقط) | ⚠️ Warn | ⚠️ Warn | ❌ Reject |

كل الأوقات بتوقيت UTC. تواريخ enforcement معكوسة في migration ledger ولن تتغير
بدون تصويت المجلس؛ أي تغيير يتطلب تحديث هذا الملف والـ ledger في نفس PR.

> **ملاحظة تنفيذية:** يقدم R1 سلسلة `result="warn"` في
> `torii_sorafs_admission_total`. تتبع رقعة ingest في Torii التي تضيف هذه التسمية
> ضمن مهام تليمترية SF-2؛ وحتى ذلك الحين استخدم أخذ عينات من السجلات لمراقبة

## التواصل ومعالجة الحوادث

- **رسالة حالة أسبوعية.** يرسل DevRel ملخصا موجزا لمقاييس القبول والتحذيرات
  والمواعيد القادمة.
- **استجابة للحوادث.** إذا انطلقت تنبيهات `reject`، يقوم on-call بما يلي:
  1. جلب advert المخالف عبر discovery في Torii (`/v1/sorafs/providers`).
  2. إعادة تشغيل تحقق advert في pipeline المزود ومقارنة النتائج مع
     `/v1/sorafs/providers` لإعادة إنتاج الخطأ.
  3. التنسيق مع المزود لتدوير advert قبل اقتراب مهلة refresh التالية.
- **تجميد التغييرات.** لا تغيرات على schema للـ capabilities خلال R1/R2 ما لم
  يوافق عليها فريق rollout؛ يجب جدولة تجارب GREASE ضمن نافذة الصيانة الأسبوعية
  وتسجيلها في migration ledger.

## المراجع

- [SoraFS Node/Client Protocol](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/sorafs_node_client_protocol.md)
- [Provider Admission Policy](./provider-admission-policy)
- [Migration Roadmap](./migration-roadmap)
- [Provider Advert Multi-Source Extensions](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md)
