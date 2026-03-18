---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/orchestrator-ops.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: عمليات الأوركسترا
العنوان: دليل تشغيل مُنسِّق SoraFS
Sidebar_label: دليل المُنسِّق
الوصف: دليل تشغيلي لمساعدة المُنسِّق متعدد المصادر ومراقبته والرجوع إليه.
---

:::ملحوظة المصدر مؤهل
احترام هذه الصفحة `docs/source/sorafs/runbooks/sorafs_orchestrator_ops.md`. احرص على كافة النسختين متزامنتين إلى أن تكتمل هجرة مجموعة سفنكس القديمة بالكامل.
:::

يرشد هذا الدليل فرق SRE من خلال الإعداد والنشر وتشغيل مُنسِّق الجلب المتعدد الفوائد. يكمل دليل المطورين إعدامات مضبوطة النشرات في الإنتاج، بما في ذلك تفعيل السريعلي وإدراج النظراء في الشريط الأسود.

> **راجع أيضًا:** يركّز [دليل واضح متعدد المصادر](./multi-source-rollout.md) على مستوى العالم الفشل في مستوى الطوارئ وعدم منع المتحكمين في الحالات. مرجع إليه لتنسيق التجارة الإلكترونية / التجارة الخارجية حيث يستخدم هذا العام اليوم.

## 1. قائمة التحقق قبل التنفيذ

1. **جمعات مدخلات المتحكمين**
   - أحدث إعلانات المتحكمين (`ProviderAdvertV1`) واللقطة التليميترية للأطول المستهدف.
   - خطة الحمولة (`plan.json`) المشتقة من المانيفست الاختبارية.
2. ** إنشاء لوحة نتائج حتمية **

   ```bash
   sorafs_fetch \
     --plan fixtures/plan.json \
     --telemetry-json fixtures/telemetry.json \
     --provider alpha=fixtures/provider-alpha.bin \
     --provider beta=fixtures/provider-beta.bin \
     --provider gamma=fixtures/provider-gamma.bin \
     --scoreboard-out artifacts/scoreboard.json \
     --json-out artifacts/session.summary.json
   ```- تحقّق من أن `artifacts/scoreboard.json` يضبط كل مبرمج إنتاجي على أنه `eligible`.
   - أرشف النتائج JSON جنبًا إلى جنب مع لوحة؛ يعتمد المأمورون على تعديات محاولة الـ قطع عند اعتماد طلب التغيير.
3. **التشغيل التجريبي باستخدام Fixies** — نفّذ نفسه على التركيبات العامة في `docs/examples/sorafs_ci_sample/` للتأكد من أن هناك منسِّق يتطابق مع الإصدار المميز قبل لمس حمولات الإنتاج.

## 2. إجراء التسجيل الشامل

1. **مرحلة الكناري (<2 متحكمين)**
   - إعادة بناء لوحة النتائج وشغّل باستخدام `--max-peers=2` ملتقييد المُنسِّق للعائلات الفرعية.
   - راقب:
     -`sorafs_orchestrator_active_fetches`
     -`sorafs_orchestrator_fetch_failures_total{reason!="retry"}`
     -`sorafs_orchestrator_retries_total`
   - تابع عندما تستمر في إعادة إعادة المحاولة أقل من 1% لجلب كامل للمانيفست ولا يراكم أي مقاييس إخفاقات.
2. **مرحلة الزيادة (50% من المتحكمين)**
   - زد قيمة `--max-peers` وعد التشغيل بلقطة فيديو حديثة.
   - استخدم كل تشغيل عبر `--provider-metrics-out` و`--chunk-receipts-out`. احتفظ بالآرتيفاكتات لمدة ≥7 أيام.
3. **إطلاق كامل**
   - أزل `--max-peers` (أو ضبطه على العدد الكامل للمنظمين المؤهلين).
   - تفعيل وضع المُنسِّق في نشرات العملاء: وزّع لوحة النتائج المحفوظة وملف JSON للإعدادات عبر نظام إدارة الإعدادات لديك.
   - تحديث اللوحات للمتابعة `sorafs_orchestrator_fetch_duration_ms` p95/p99 ومحاولات إعادة المحاولة حسب المنطقة.

## 3. حظر توريد الطعام

استخدم تجاوزات التقييم في CLI لفرز المتحكمين غير الأصحاء دون توقع تحديثات التصفح.

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
```- `--deny-provider` الاسم المستعار المدرج من مراعاة النظر الحالي.
- `--boost-provider=<alias>=<weight>` تعديل تعديل المنظم في المنظم الجدول. القيم تُضاف إلى الوزن الزائد للوحة النتائج وتُطبق فقط على التشغيل المحلي.
- سجل التجاوزات في تذكرة الحادثة وأرفق المخرجات JSON ليتمكن الفريق المسؤول من اتفاقية حالة إصلاح المشكلة الأصلية.

بالنسبة للتغييرات، تعدد التليميترية المصدرية (ضع علامة يعاقب عليها المخالف) أو أكد الإعلان بتدفق ميزانيات مُعدل التعديل قبل إزالة تجاوزات CLI.

## 4. تشخيص البرامج

عندما يفشل الجلب:

1. قبل التقاط الآرتيفاكتات التالية إعادة التشغيل:
   -`scoreboard.json`
   -`session.summary.json`
   -`chunk_receipts.json`
   -`provider_metrics.json`
2. فحص `session.summary.json` لسلسلة الخطأ المقروءة:
   - `no providers were supplied` → تحقّق من زيارات المتحكمين والإعلانات.
   - `retry budget exhausted ...` → زد `--retry-budget` أو أزل النظراء غير المستقرين.
   - `no compatible providers available ...` → دقاق بيانات قدرات النطاق للمنظم المخالف.
3. قم بربط اسم المرقم بـ `sorafs_orchestrator_provider_failures_total` وافتح تذكرة نجاح إذا حدثت الحالات.
4. إعادة إنتاج عملية الجلب دون اتصال باستخدام `--scoreboard-json` والملتقطة بالتقاط الصور بعد حتمي.

## 5. الرجوع

لتخصيص المُنسِّق:

1. إعداد إعداد يضبط `--max-peers=1` (يعطى فعليا جدولة متعددة المصادر) أو إعداد العملاء إلى مسار جلب الأحادي المصدر القديم.
2. أزل أي تجاوزات `--boost-provider` كي تعود نتائج اللوحة إلى وزن هوندا.
3. واصل جمع معايير المُنسِّق لمدة يوم واحد على الأقل للتأكد من عدم وجود عمليات جلب شبكية.ضمان الانضباط في التقاط الآرتيفاكتات والإطلاق المرحلي إمكانية تشغيل المُنسِّق الكامل بشكل آمن عبر أسابروف متحكمين غير مكتملة مع التمتع بمتطلبات الرصد والدقيق.