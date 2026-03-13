---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/provider-advert-rollout.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
title: "مخطط بدء تشغيل إعلانات الموفرين SoraFS"
---

> التكيف من [`docs/source/sorafs/provider_advert_rollout.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_rollout.md).

# خطة طرح إعلانات مقدمي الخدمة SoraFS

يتم تنسيق هذه الخطة حول قطع الإعلانات المسموح بها من قبل مقدمي الخدمة
السطح المتحكم به تمامًا `ProviderAdvertV1` مطلوب لاسترجاعه
مصادر متعددة دي قطع. التركيز على ثلاثة تسليمات:

- **دليل المشغلين.** الخطوات التي يطلبها موفرو التخزين من إنهاء البوابة مسبقًا.
- **تغطية القياس عن بعد.** لوحات المعلومات والتنبيهات المتعلقة بقابلية المراقبة والعمليات المستخدمة
  لتأكيد أن إعادة استخدام الإعلانات تتوافق مع ذلك.
  لإصدارات معدات SDK وأدوات التخطيط.

سيتم إطلاق الإصدار الجديد من ماركوس SF-2b/2c no
[خريطة طريق الهجرة SoraFS](./migration-roadmap) ونفترض أن سياسة القبول
لا توجد [سياسة قبول الموفر](./provider-admission-policy) وهذا هو الحال.

## الجدول الزمني دي fases

| فاس | جانيلا (ألفو) | السلوك | المفاتيح تفعل المشغل | مجال الملاحظة |
|-------|-----------------|-----------|------------------|-------------------|

## قائمة التحقق للمشغل1. **جرد الإعلانات.** قم بإدراج كل إعلان تم نشره وتسجيله:
   - كامينهو يقوم بإدارة المغلف (`defaults/nexus/sorafs_admission/...` أو يعادل إنتاجهم).
   - `profile_id` و`profile_aliases` قم بالإعلان.
   - قائمة الإمكانيات (يرجى الاطلاع على أقل من `torii_gateway` و`chunk_range_fetch`).
   - ضع علامة على `allow_unknown_capabilities` (يلزم تقديم هدايا TLV المحفوظة بواسطة البائع).
2. **موفر أدوات تجديد com.**
   - إعادة بناء الحمولة مع إعلان موفر الخدمة الخاص بك، مع ضمان ما يلي:
     -`profile_id=sorafs.sf1@1.0.0`
     - `capability=chunk_range_fetch` com `max_span` محدد
     - `allow_unknown_capabilities=<true|false>` quando houver TLVs GREASE
   - صالح عبر `/v2/sorafs/providers` و`sorafs_fetch`؛ تحذيرات من القدرات الفوقية
     Desconhecidas Devem ser Triageadas.
3. **جاهزية Validar متعددة المصادر.**
   - تنفيذ `sorafs_fetch` com `--provider-advert=<path>`؛ o CLI Agora falha quando
     `chunk_range_fetch` هذا هو التحذير الرئيسي والجديد لإزالة الثغرات
     جاهل. التقط تقرير JSON واحفظ سجلات العمليات.
4. **تحضير التجديدات.**
   - أظرف Envie `ProviderAdmissionRenewalV1` قبل أقل من 30 يومًا
     إنفاذ لا بوابة (R2). يجب أن تعمل التجديدات على التعامل مع Canonico e o
     تعيين القدرات؛ حصة واحدة أو نقاط النهاية أو البيانات الوصفية يجب أن تكون مدروسة.
5. **Comunicar يجهز المُعالين.**
   - يمكن للجهات المانحة لـ SDK إصدار إصدارات إضافية تتضمن تحذيرات للمشغلين عند
     إعلانات forem rejeitados.- DevRel anuncia cada transicao de fase؛ تضمين روابط لوحات المعلومات والمنطق
     دي عتبات أبيكسو.
6. **تثبيت لوحات المعلومات والتنبيهات.**
   - استيراد Grafana تصدير وكلمة تنهد **SoraFS / طرح الموفر** مع UID
     `sorafs-provider-admission`.
   - ضمان التنبيهات اللازمة لمشاركة القناة
     `sorafs-advert-rollout` في مرحلة الإنتاج.

## لوحات القياس والقياس عن بعد

فيما يلي المقاييس والعروض عبر `iroha_telemetry`:

- `torii_sorafs_admission_total{result,reason}` - conta aceitos، rejeitados e
  تحذيرات. تتضمن الدوافع `missing_envelope`، `unknown_capability`، `stale`
  هـ `policy_violation`.

تصدير Grafana: [`docs/source/grafana_sorafs_admission.json`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/grafana_sorafs_admission.json).
استيراد ملف إلى مستودع مشاركة لوحات المعلومات (`observability/dashboards`)
قم بتحقيق النقاط أو المعرف الفريد (UID) لمصدر البيانات قبل النشر.

اللوحة ونشر المعكرونة Grafana **SoraFS / طرح الموفر** مع UID
إستافيل `sorafs-provider-admission`. كإعادة تنبيه
`sorafs-admission-warn` (تحذير) و `sorafs-admission-reject` (حرج)
تم التكوين مسبقًا لاستخدام سياسة الإشعارات `sorafs-advert-rollout`؛ ضبط
إذا كانت نقطة الاتصال موجهة نحو الوجهة، فيمكنك تحريرها أو تعديل لوحة التحكم JSON.

يوصى باستخدام طلاءات Grafana:| لوحة | استعلام | ملاحظات |
|-------|-------|-------|
| **معدل نتائج القبول** | `sum by(result)(rate(torii_sorafs_admission_total[5m]))` | مخطط المكدس لتصور القبول مقابل التحذير مقابل الرفض. تنبيه عند التحذير > 0.05 * الإجمالي (تحذير) أو الرفض > 0 (حرج). |
| **نسبة التحذير** | `sum(rate(torii_sorafs_admission_total{result="warn"}[5m])) / sum(rate(torii_sorafs_admission_total[5m]))` | سلاسل زمنية فريدة من نوعها يتم تشغيلها أو عتبة النداء (معدل تحذير 5% يبلغ 15 دقيقة). |
| **أسباب الرفض** | `sum by(reason)(rate(torii_sorafs_admission_total{result="reject"}[5m]))` | قم بفرز Guia بواسطة دليل التشغيل؛ روابط ملحقة للتخفيف. |
| **تحديث الديون** | `sum(rate(torii_sorafs_admission_total{reason="stale"}[1h]))` | مقدمو الخدمة الهندية الذين يتأخرون عن الموعد النهائي أو يتم تحديثه؛ تقوم سجلات Cruze com باكتشاف ذاكرة التخزين المؤقت. |

عناصر CLI للوحات المعلومات اليدوية:

- `sorafs_fetch --provider-metrics-out` فتح المقاولين `failures`، `successes`
  e `disabled` بواسطة الموفر. قم باستيراد لوحات المعلومات المخصصة لمراقبة عمليات التشغيل الجافة
  قم بعمل منسق قبل مقدمي المبزل في الإنتاج.
- Campos `chunk_retry_rate` e `provider_failure_rate` قم بالإبلاغ عن JSON destacam
  اختناق أو عدد من الحمولات القديمة التي تم قبول قبولها مسبقًا.

### تخطيط لوحة المعلومات Grafana

إمكانية الملاحظة منشورة على لوحة مخصصة - **SoraFS قبول المزود
الطرح** (`sorafs-provider-admission`) - sob **SoraFS / طرح الموفر**
مع معرفات اللوحة الأساسية التالية:- اللوحة 1 - *معدل نتائج القبول* (منطقة مكدسة، وحدة "عمليات/دقيقة").
- اللوحة 2 - *نسبة التحذير* (سلسلة واحدة)، مع Expressao
  `sum(معدل(torii_sorafs_admission_total{result="warn"}[5m])) /
   sum(rate(torii_sorafs_admission_total[5m]))`.
- اللوحة 3 - *أسباب الرفض* (السلسلة الزمنية agrupada por `reason`)، الترتيب من قبل
  `rate(...[5m])`.
- اللوحة 4 - *تحديث الديون* (الإحصائيات)، قم بإجراء استعلام في الجدول الحالي والشرح
  com تحديث المواعيد النهائية دوس الإعلانات extraidos القيام دفتر الأستاذ.

انسخ (أو اصرخ) هيكل JSON بدون تخزين لوحات المعلومات بالأشعة تحت الحمراء
`observability/dashboards/sorafs_provider_admission.json`، بعد تحقيق apenas
o معرف المستخدم الخاص بمصدر البيانات؛ معرفات اللوحة وإرشادات التنبيه الخاصة بالأشخاص المرجعيين
بعد ذلك، تجنب إعادة ترقيم دفاتر التشغيل قبل مراجعة هذا المستند.

من أجل الراحة، يتضمن المستودع تعريفًا للوحة المعلومات المرجعية
`docs/source/grafana_sorafs_admission.json`; انسخ للمعكرونة الخاصة بك Grafana se
Precisar de um ponto de Partida para الخصيتين locais.

### تعليمات التنبيه Prometheus

قم بإضافة مجموعة التصحيحات التالية إليهم
`observability/prometheus/sorafs_admission.rules.yml` (يصرخ أو ملف موجود لـ
o مجموعة القواعد الأولى SoraFS) وتتضمن تكوين Prometheus.
استبدال `<pagerduty>` بملصق التدوير الحقيقي من خلال تشغيله عند الطلب.

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

نفذ `scripts/check_prometheus_rules.sh observability/prometheus/sorafs_admission.rules.yml`
قبل إرسال التعديلات لضمان مرور الجملة `promtool check rules`.

## ماتريز دي رولوت| مميزات الإعلان | ص0 | ر1 | ر2 | ر3 |
|------------------------|----|----|----|----|
| `profile_id = sorafs.sf1@1.0.0`، `chunk_range_fetch` الحاضر، الأسماء المستعارة canonicos، `signature_strict=true` | موافق | موافق | موافق | موافق |
| قدرة Ausencia de `chunk_range_fetch` | تحذير (الاستيعاب + القياس عن بعد) | تحذير | رفض (`reason="missing_capability"`) | رفض |
| تم إلغاء تحديد قدرة TLVs Sem `allow_unknown_capabilities=true` | موافق | تحذير (`reason="unknown_capability"`) | رفض | رفض |
| `refresh_deadline` انتهاء الصلاحية | رفض | رفض | رفض | رفض |
| `signature_strict=false` (التركيبات التشخيصية) | موافق (apenas التنمية) | تحذير | تحذير | رفض |

Todos os horarios usam UTC. بيانات تطبيق القانون لا هجرة
دفتر الأستاذ e nao mudam sem voto do Council؛ qualquer mudanca requer atualizar este
الملف ودفتر الأستاذ ليسا في نفس الوقت من العلاقات العامة.

> **ملاحظة التنفيذ:** يقدم R1 السلسلة `result="warn"` em
> `torii_sorafs_admission_total`. قم بإضافة تصحيح الإدخال Torii إلى o
> تسمية جديدة ومصاحبة جنبًا إلى جنب مع مهام القياس عن بعد SF-2; أكل لا، استخدم

## التواصل ومعالجة الحوادث- **رسالة الحالة الأسبوعية.** شارك DevRel في ملخص مقاييس القبول،
  التحذيرات المعلقة والمواعيد النهائية القريبة.
- **الاستجابة للحوادث.** عند إرسال تنبيهات `reject` للمهندسين تحت الطلب:
  1. قم بالتصوير أو الإعلان عن الجهاز عبر اكتشاف Torii (`/v2/sorafs/providers`).
  2. قم بإعادة تنفيذ عملية التحقق من صحة الإعلان عن عدم وجود خط أنابيب لموفر الخدمة ومقارنة com
     `/v2/sorafs/providers` لإعادة إنتاج الخطأ.
  3. قم بالتنسيق مع مزود خدمة rotacao للإعلان قبل الموعد النهائي للتحديث التقريبي.
- **تغيير التجميد.** لا يوجد مخطط للإمكانيات خلال R1/R2 a
  ما عليك سوى الموافقة على لجنة الطرح؛ المحاكمات GREASE ستكون أجندة
  janela semanal de manutencao والمسجلون في دفتر حسابات الهجرة.

## المراجع

- [SoraFS بروتوكول العقدة/العميل](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/sorafs_node_client_protocol.md)
- [سياسة قبول الموفر](./provider-admission-policy)
- [خريطة طريق الهجرة](./migration-roadmap)
- [إعلان الموفر عن ملحقات متعددة المصادر](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md)