---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/orchestrator-ops.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: عمليات الأوركسترا
العنوان: Runbook d’exploitation de l’orchesstrateur SoraFS
Sidebar_label: منسق كتاب التشغيل
الوصف: قم بتوجيه عملية التشغيل للنشر والمراقبة والتجديد بعد الوصول إلى المُنسق متعدد المصادر.
---

:::ملاحظة المصدر الكنسي
هذه الصفحة تعكس `docs/source/sorafs/runbooks/sorafs_orchestrator_ops.md`. قم بمزامنة النسختين حتى يتم هجرة مجموعة الوثائق التي ورثها أبو الهول بالكامل.
:::

يرشد دليل التشغيل هذا SRE إلى الإعداد والنشر واستغلال منسق الجلب متعدد المصادر. يكتمل دليل التطوير باستخدام الإجراءات المعدلة لعمليات النشر في الإنتاج، ويتضمن التنشيط عبر الأشرطة والإعداد في القائمة السوداء للأزواج.

> **راجع أيضًا:** يركز [Runbook de déploiment multisource](./multi-source-rollout.md) على غموض النشر في المنتزه ورفض استعجال الموردين. يرجى الرجوع إلى تنسيق الحوكمة / التدريج باستخدام هذا المستند للعمليات اليومية للمدير.

## 1. قائمة المراجعة قبل النشر

1. **اجمع المقبلات المجهزة**
   - يعلن Dernières عن المزودين (`ProviderAdvertV1`) ولحظة عن بعد لقافلة السفن.
   - خطة الحمولة (`plan.json`) مشتقة من بيان الاختبار.
2. **إنشاء لوحة نتائج محددة**

   ```bash
   sorafs_fetch \
     --plan fixtures/plan.json \
     --telemetry-json fixtures/telemetry.json \
     --provider alpha=fixtures/provider-alpha.bin \
     --provider beta=fixtures/provider-beta.bin \
     --provider gamma=fixtures/provider-gamma.bin \
     --scoreboard-out artifacts/scoreboard.json \
     --json-out artifacts/session.summary.json
   ```- تحقق من أن `artifacts/scoreboard.json` هو كل مزود إنتاج مثل `eligible`.
   - أرشفة JSON من خلال لوحة النتائج؛ يلجأ المدققون إلى أجهزة كمبيوتر إعادة محاولة القطع عند الحصول على شهادة طلب التغيير.
3. **التشغيل الجاف مع التركيبات** — قم بتنفيذ نفس الأمر على التركيبات المنشورة في `docs/examples/sorafs_ci_sample/` للتأكد من أن ثنائي المنسق يتوافق مع الإصدار الموجود قبل لمس حمولة الإنتاج.

## 2. إجراء النشر بالخطوات1. ** Étape canari (42 fournisseurs) **
   - أعد بناء لوحة النتائج وقم بالتنفيذ باستخدام `--max-peers=2` لتحديد المنسق بمجموعة صغيرة صغيرة.
   - المراقبة:
     -`sorafs_orchestrator_active_fetches`
     -`sorafs_orchestrator_fetch_failures_total{reason!="retry"}`
     -`sorafs_orchestrator_retries_total`
   - احرص على بقاء مدة إعادة المحاولة بنسبة 1% فقط لجلب البيان بالكامل وعندما لا تتراكم الشيكات.
2. **شريط مونتي أون تشارج (50% من الموردين)**
   - قم بتعزيز `--max-peers` واتصل بلحظة قياس عن بعد حديثة.
   - استمر في تنفيذ كل مرة مع `--provider-metrics-out` و`--chunk-receipts-out`. احتفظ بالقطع الأثرية لمدة تزيد عن 7 أيام.
3. **اكتمل النشر**
   - قم بحذف `--max-peers` (أو قم بإصلاح الرقم الإجمالي للموردين المؤهلين).
   - تنشيط الوضع المنسق في عملاء النشر: توزيع لوحة النتائج المستمرة وتكوين JSON عبر نظام إدارة التكوين الخاص بك.
   - قم بإعادة تشغيل اللوحات الخشبية لعرض `sorafs_orchestrator_fetch_duration_ms` p95/p99 والمخططات البيانية لإعادة المحاولة حسب المنطقة.

## 3. إنشاء قائمة سوداء وتعزيز الأزواج

استخدم تجاوزات التقييم السياسي لـ CLI لمحاولة الموردين المتخلفين دون متابعة إجراءات الإدارة اليومية.

```bash
sorafs_fetch \
  --plan fixtures/plan.json \
  --telemetry-json fixtures/telemetry.json \
  --provider alpha=fixtures/provider-alpha.bin \
  --provider beta=fixtures/provider-beta.bin \
  --provider gamma=fixtures/provider-gamma.bin \
  --deny-provider=beta \
  --boost-provider=gamma=5 \
  --json-out artifacts/override.summary.json
```- `--deny-provider` يحذف الاسم المستعار الذي يشير إلى الجلسة.
- `--boost-provider=<alias>=<weight>` يزيد من مخزون المورد في المخطط. يتم إضافة القيم إلى لوحة النتائج ولا تنطبق على التنفيذ المحلي.
- قم بتسجيل التجاوزات في تذكرة الحادث وانضم إلى طلعات JSON حتى يتمكن الجهاز المسؤول من تسوية حالة المشكلة في وقت لاحق.

لإجراء تغييرات دائمة، قم بتعديل مصدر القياس عن بعد (حدد الخطأ باعتباره معاقبًا) أو قم بتحديث الإعلان يوميًا باستخدام ميزانيات التدفق المنقحة قبل إلغاء تجاوزات CLI.

## 4. تشخيص الألواح

Lorsqu’un جلب صدى:

1. قم بالتقاط العناصر التالية قبل إعادة إطلاقها:
   -`scoreboard.json`
   -`session.summary.json`
   -`chunk_receipts.json`
   -`provider_metrics.json`
2. افحص `session.summary.json` لسلسلة الأخطاء المحتملة:
   - `no providers were supplied` → تحقق من طرق الموردين والإعلانات.
   - `retry budget exhausted ...` → زيادة `--retry-budget` أو حذف الأزواج غير المستقرة.
   - `no compatible providers available ...` → قم بمراجعة قدرات الموفر الفاشل.
3. قم بتصحيح اسم المورد باستخدام `sorafs_orchestrator_provider_failures_total` وأنشئ تذكرة متابعة إذا تم رفع المقياس.
4. أعد تشغيل خط الجلب باستخدام `--scoreboard-json` والتقط جهاز القياس عن بعد لإعادة إنتاج طريقة التحديد.

## 5. التراجعللتذكير بنشرة منسقة:

1. قم بتوزيع التكوين الذي تم تحديده `--max-peers=1` (إلغاء تنشيط النظام متعدد المصادر) أو الرجوع إلى نظام جلب العملاء التاريخيين أحادي المصدر.
2. قم بإلغاء التجاوز `--boost-provider` حتى تعود لوحة النتائج إلى مكان محايد.
3. استمر في فحص مقاييس المُنسق خلال أقل من يوم للتأكد من عدم جلب أي بقايا في المجلد.

حافظ على التقاط منضبط للعناصر وعمليات النشر من خلال خطوات تضمن أن المنسق متعدد المصادر يمكن تشغيله بأمان تام على أسطول الموردين المختلفين وفقًا لمتطلبات المراقبة والتدقيق.