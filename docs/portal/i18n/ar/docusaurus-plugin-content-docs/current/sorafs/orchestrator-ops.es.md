---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/orchestrator-ops.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: عمليات الأوركسترا
العنوان: Runbook de Operations del orquestador de SoraFS
Sidebar_label: Runbook del orquestador
الوصف: دليل التشغيل خطوة بخطوة للانطلاق والإشراف وإرجاع المُنسق متعدد الأصول.
---

:::ملاحظة فوينتي كانونيكا
هذه الصفحة تعكس `docs/source/sorafs/runbooks/sorafs_orchestrator_ops.md`. حافظ على النسخ المتزامنة حتى يتم ترحيل مجموعة وثائق أبو الهول الموروثة بالكامل.
:::

دليل التشغيل هذا هو دليل SRE للتحضير والتنفيذ وعملية الجلب المتعددة المصادر. أكمل دليل التطوير بإجراءات معدلة بعد الإنتاج، بما في ذلك القدرة على الخطوات وحظر الأقران.

> **العرض أيضًا:** يقع [Runbook of Despliegue multiorigen](./multi-source-rollout.md) في وسط سفن النشر على مستوى الأسطول وفي انتهاك الموردين في حالات الطوارئ. استشارة حول تنسيق الإدارة / التدريج أثناء استخدام هذا المستند الخاص بعمليات يوميات المنسق.

## 1. قائمة التحقق المسبقة

1. **استعادة مدخلات الموردين**
   - آخر إعلانات الموردين (`ProviderAdvertV1`) والقياس الفوري لهدف الأسطول.
   - خطة الحمولة (`plan.json`) مشتقة من البيان التجريبي المنخفض.
2. **إنشاء تحديد لوحة النتائج**

   ```bash
   sorafs_fetch \
     --plan fixtures/plan.json \
     --telemetry-json fixtures/telemetry.json \
     --provider alpha=fixtures/provider-alpha.bin \
     --provider beta=fixtures/provider-beta.bin \
     --provider gamma=fixtures/provider-gamma.bin \
     --scoreboard-out artifacts/scoreboard.json \
     --json-out artifacts/session.summary.json
   ```- التحقق من أن `artifacts/scoreboard.json` يدرج كل منتج إنتاج مثل `eligible`.
   - أرشيف JSON للاستئناف بجوار لوحة النتائج؛ سيطلب المدققون من محاسبي قطع الغيار أن يشهدوا طلب التغيير.
3. **Dry-run with Installations** — قم بتنفيذ نفس الأمر ضد التركيبات العامة في `docs/examples/sorafs_ci_sample/` لضمان تزامن البرنامج الثنائي مع الإصدار المتوقع قبل تحميل حمولة الإنتاج.

## 2. إجراءات التخلص من المخلفات1. **إيتابا كناريا (<2 مستوردين)**
   - أعد بناء لوحة النتائج وقم بتشغيل `--max-peers=2` لتقييد الأوركستراد بمجموعة فرعية صغيرة.
   - الإشراف:
     -`sorafs_orchestrator_active_fetches`
     -`sorafs_orchestrator_fetch_failures_total{reason!="retry"}`
     -`sorafs_orchestrator_retries_total`
   - استمر في حين يتم الحفاظ على عمليات إعادة الإيداع بتخفيض 1% لجلب البيان بالكامل ولن يتراكم أي فشل في الموفر.
2. ** إتابا دي رامبا (50% من الموردين) **
   - قم بزيادة `--max-peers` وقم بالتنفيذ بلحظة قياس عن بعد حديثة.
   - استمر في التشغيل كل مرة باستخدام `--provider-metrics-out` و`--chunk-receipts-out`. احتفظ بالمصنوعات اليدوية لمدة تزيد عن 7 أيام.
3. **الشرح كامل**
   - قم بإزالة `--max-peers` (أو قم بتكوين القائمة الكاملة للمؤهلات).
   - تمكين وضع المنسق على العملاء: توزيع لوحة النتائج المستمرة وتكوين JSON عبر نظام إدارة التكوين.
   - تحديث لوحات المعلومات لعرض `sorafs_orchestrator_fetch_duration_ms` p95/p99 والمخططات التكرارية لكل منطقة.

## 3. حظر واستعادة الأقران

استخدم عبارات التنقيط السياسية لـ CLI لتصنيف الموردين دون أن تكون جديرة بالثناء دون توقع تحديثات الإدارة.

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
```- `--deny-provider` يزيل الاسم المستعار المشار إليه في الجلسة الفعلية.
- `--boost-provider=<alias>=<weight>` يعمل على زيادة سعر جهاز التخطيط. يتم تطبيع القيم في لوحة النتائج ويتم تطبيقها فقط على التنفيذ المحلي.
- سجل الإلغاءات في تذكرة الحادث وأضف ملفات JSON حتى يتمكن الفريق المسؤول من تسوية الحالة بمجرد حل المشكلة الفرعية.

للتغييرات الدائمة، قم بتعديل القياس عن بعد الأصلي (علامة المخالفة كعقوبة) أو تحديث الإعلان بافتراضات التدفق المحدثة قبل تنظيف مخالفات CLI.

## 4. تشخيص السقوط

Cuando un fetch Falla:

1. التقط العناصر التالية قبل تشغيلها:
   -`scoreboard.json`
   -`session.summary.json`
   -`chunk_receipts.json`
   -`provider_metrics.json`
2. فحص `session.summary.json` لرؤية سلسلة الأخطاء المقروءة:
   - `no providers were supplied` → التحقق من تعليمات الموردين والإعلانات.
   - `retry budget exhausted ...` → الزيادة `--retry-budget` أو إزالة الأقران غير المستقرة.
   - `no compatible providers available ...` → قم بمراجعة معلومات السعة لنطاق مرتكب المخالفة.
3. قم بربط اسم المورّد بـ `sorafs_orchestrator_provider_failures_total` وأنشئ تذكرة تسلسل في حالة اختلاف المقياس.
4. قم بإعادة إنتاج الحصول على `--scoreboard-json` بدون اتصال والقياس عن بعد الذي تم التقاطه لإعادة إنتاج شكل التحديد.## 5. الرجوع

للرجوع إلى لعبة orquestador:

1. قم بتوزيع التكوين الذي تم تعيينه `--max-peers=1` (إلغاء تمكين التخطيط متعدد المصادر) أو قم بتوجيه العملاء إلى طريق جلب الموروثات من مصدر واحد فقط.
2. قم بإزالة أي إلغاء `--boost-provider` حتى تصبح لوحة النتائج بيزو محايد.
3. استمر في جمع مقاييس الأوركيستادور لمدة أقل من يوم للتأكد من عدم جلب أي طائر بالطائرة.

حافظ على لقطة منضبطة للمصنوعات اليدوية والسفن من خلال ضمان أن المنسق متعدد الأصول يمكنه العمل بشكل آمن في أساطيل الموردين غير المتجانسة مع الحفاظ على متطلبات المراقبة والاستماع.