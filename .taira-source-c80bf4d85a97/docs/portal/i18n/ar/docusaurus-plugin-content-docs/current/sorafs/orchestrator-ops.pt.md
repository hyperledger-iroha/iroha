---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/orchestrator-ops.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: عمليات الأوركسترا
العنوان: Runbook de Operations do orquestrador SoraFS
Sidebar_label: Runbook do orquestrador
الوصف: دليل تشغيلي للتمرير للزرع والمراقبة والعاكس أو الأوركسترا متعدد الأصول.
---

:::ملاحظة Fonte canônica
هذه الصفحة مخصصة `docs/source/sorafs/runbooks/sorafs_orchestrator_ops.md`. Mantenha ambas as copias sincronzadas.
:::

يوجه دليل التشغيل هذا SREs إلى الإعداد وعدم النشر وتشغيل أداة الجلب متعددة الأصول. إنه مكمل لدليل التطوير من خلال الإجراءات المعدلة لإصدارات الإنتاج، بما في ذلك التأهيل في المراحل وحظر الأقران.

> **الرجوع أيضًا:** O [Runbook de rollout multi-origin](./multi-source-rollout.md) قم بالتركيز على عمليات الطرح في كل مكان من الخارج وإلغاء الإجراءات الطارئة. استشر تنسيق الإدارة / التدريج أثناء استخدام هذا المستند لعمليات يوميات الأوركسترا.

## 1. قائمة المراجعة المسبقة

1. ** كوليتار إنسوموس دي بروفوريس **
   - آخر الأخبار المعلنة (`ProviderAdvertV1`) ولقطة القياس عن بعد من كل مكان.
   - مخطط الحمولة (`plan.json`) مشتق من بيان الاختبار.
2. ** تحديد لوحة النتائج **

   ```bash
   sorafs_fetch \
     --plan fixtures/plan.json \
     --telemetry-json fixtures/telemetry.json \
     --provider alpha=fixtures/provider-alpha.bin \
     --provider beta=fixtures/provider-beta.bin \
     --provider gamma=fixtures/provider-gamma.bin \
     --scoreboard-out artifacts/scoreboard.json \
     --json-out artifacts/session.summary.json
   ```- التحقق من صحة قائمة `artifacts/scoreboard.json` لكل مُصدر إنتاج مثل `eligible`.
   - حفظ JSON من السيرة الذاتية إلى لوحة النتائج؛ يعتمد المدققون على مكابس إعادة محاولة القطع من خلال التصديق على طلب التعديل.
3. **Dry-run com Installations** — نفذ الأمر نفسه ضد التركيبات العامة في `docs/examples/sorafs_ci_sample/` لضمان توافق البرنامج الثنائي مع الإصدار المتوقع قبل تشغيله على حمولات الإنتاج.

## 2. عملية الطرح تتم عبر مراحل1. **Fase canário (≥2 بروفيدوريس)**
   - استعادة لوحة النتائج وتنفيذها باستخدام `--max-peers=2` لتقييد الأوركسترا في مجموعة فرعية صغيرة.
   - مراقب:
     -`sorafs_orchestrator_active_fetches`
     -`sorafs_orchestrator_fetch_failures_total{reason!="retry"}`
     -`sorafs_orchestrator_retries_total`
   - عملية مستمرة عند إعادة المحاولة بنسبة أقل من 1% لجلب البيان بالكامل ولا تزيد من الأخطاء المتراكمة.
2. **Fase de Rampa (50% من الإثباتات)**
   - قم بتعزيز `--max-peers` وقم بتنفيذه بشكل جديد باستخدام لقطة قياس عن بعد حديثة.
   - الاستمرار في التنفيذ مع `--provider-metrics-out` و`--chunk-receipts-out`. Retenha os artefatos por ≥7 dias.
3. **اكتمل الطرح**
   - إزالة `--max-peers` (أو تحديد العدوى الكاملة للرثاء).
   - طريقة تنفيذ عمليات نشر العملاء النشطة: توزيع لوحة النتائج المستمرة وتكوين JSON عبر نظام إدارة التكوين الخاص بك.
   - تحديث لوحات المعلومات لعرض `sorafs_orchestrator_fetch_duration_ms` p95/p99 وإعادة الرسم البياني لإعادة المحاولة حسب المنطقة.

## 3. حجب الأقران وتعزيزهم

استخدم تجاوزات التوجيه السياسي لـ CLI لفرز المحققين دون توقع تحديثات الإدارة.

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
```- `--deny-provider` قم بإزالة الاسم المستعار المشار إليه في الجلسة الحالية.
- `--boost-provider=<alias>=<weight>` زيادة أو إثبات عدم وجود جدول أعمال. يتم تطبيع القيم على لوحة النتائج ويتم تطبيقها فقط على التنفيذ المحلي.
- قم بتسجيل التجاوزات بدون تذكرة للحادث وأضف المرفق باسم JSON حتى يتمكن الفريق المسؤول من تسوية الحالة عند وجود مشكلة موضوعية للحل.

للتغييرات الدائمة، اضبط جهاز القياس الأصلي عن بعد (علامة أو مضخة كعقوبة) أو قم بتحديث الإعلان باستخدام عمليات التدفق المحدثة قبل إزالة تجاوزات CLI.

## 4. ترياجيم دي فالهاس

Quando أم جلب falha:

1. قم بالتقاط الخطوات اليدوية التالية قبل تنفيذها مرة أخرى:
   -`scoreboard.json`
   -`session.summary.json`
   -`chunk_receipts.json`
   -`provider_metrics.json`
2. فحص `session.summary.json` لسلسلة الأخطاء القانونية:
   - `no providers were supplied` → التحقق من caminhos dos profeores e os anúncios.
   - `retry budget exhausted ...` → قم بزيادة `--retry-budget` أو قم بإزالة أقرانك على الفور.
   - `no compatible providers available ...` → قم بمراجعة بيانات السعة الخاصة بمختبر الأشعة.
3. قم بربط اسم المثبت بـ `sorafs_orchestrator_provider_failures_total` وقم بإدراج تذكرة مرافقة في حالة اختلاف المقياس.
4. قم بإعادة إنتاج الجلب دون الاتصال بالإنترنت باستخدام `--scoreboard-json` وتم التقاط القياس عن بعد لإعادة إنتاج شكل محدد بشكل خاطئ.

## 5. التراجع

للتراجع عن الطرح في orquestrador:1. قم بتوزيع التكوين المحدد `--max-peers=1` (تعطيل فعال أو جدول أعمال متعدد الأصول) أو إعادة العملاء إلى طريق جلب المصدر الوحيد.
2. تؤدي إزالة أي شيء إلى تجاوز `--boost-provider` حتى تصبح لوحة النتائج فكرة جديدة.
3. استمر في تجميع مقاييس الأوركستراد بجزء أقل من يوم للتأكيد على عدم جلب البقايا بشكل صحيح.

بالإضافة إلى الالتقاط المنضبط للتحف الفنية والطرح عبر خطوات تضمن أن يتم تشغيل الأوركسترا متعدد الأصول بأمان في مجموعات غير متجانسة من المقدمين، مع الحفاظ على متطلبات المراقبة والاستماع سليمة.