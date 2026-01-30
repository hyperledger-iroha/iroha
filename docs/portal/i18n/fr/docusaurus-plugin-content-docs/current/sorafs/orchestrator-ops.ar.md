---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/orchestrator-ops.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: orchestrator-ops
title: دليل تشغيل مُنسِّق SoraFS
sidebar_label: دليل تشغيل المُنسِّق
description: دليل تشغيلي خطوة بخطوة لنشر المُنسِّق متعدد المصادر ومراقبته والرجوع عنه.
---

:::note المصدر المعتمد
تعكس هذه الصفحة `docs/source/sorafs/runbooks/sorafs_orchestrator_ops.md`. احرص على إبقاء النسختين متزامنتين إلى أن تكتمل هجرة مجموعة توثيق Sphinx القديمة بالكامل.
:::

يرشد هذا الدليل فرق SRE خلال التحضير والنشر وتشغيل مُنسِّق الجلب متعدد المصادر. يكمل دليل المطورين بإجراءات مضبوطة لعمليات النشر في الإنتاج، بما في ذلك التفعيل المرحلي وإدراج النظراء في القائمة السوداء.

> **راجع أيضًا:** يركّز [دليل إطلاق متعدد المصادر](./multi-source-rollout.md) على موجات الإطلاق على مستوى الأسطول وعلى منع المزوّدين في حالات الطوارئ. ارجع إليه لتنسيق الحوكمة / بيئة الاختبار المرحلية بينما تستخدم هذا المستند لعمليات المُنسِّق اليومية.

## 1. قائمة التحقق قبل التنفيذ

1. **جمع مدخلات المزوّدين**
   - أحدث إعلانات المزوّدين (`ProviderAdvertV1`) ولقطة التليمترية للأسطول المستهدف.
   - خطة الحمولة (`plan.json`) المشتقة من المانيفست قيد الاختبار.
2. **إنشاء لوحة نتائج حتمية**

   ```bash
   sorafs_fetch \
     --plan fixtures/plan.json \
     --telemetry-json fixtures/telemetry.json \
     --provider alpha=fixtures/provider-alpha.bin \
     --provider beta=fixtures/provider-beta.bin \
     --provider gamma=fixtures/provider-gamma.bin \
     --scoreboard-out artifacts/scoreboard.json \
     --json-out artifacts/session.summary.json
   ```

   - تحقّق من أن `artifacts/scoreboard.json` يسرد كل مزوّد إنتاجي على أنه `eligible`.
   - أرشف ملخص JSON جنبًا إلى جنب مع لوحة النتائج؛ يعتمد المدققون على عدّادات إعادة محاولة الـ chunks عند اعتماد طلب التغيير.
3. **تشغيل تجريبي باستخدام fixtures** — نفّذ الأمر نفسه على fixtures العامة في `docs/examples/sorafs_ci_sample/` للتأكد من أن ثنائية المُنسِّق تطابق الإصدار المتوقع قبل لمس حمولات الإنتاج.

## 2. إجراء الإطلاق المرحلي

1. **مرحلة الكناري (≤2 مزوّدين)**
   - أعد بناء لوحة النتائج وشغّل باستخدام `--max-peers=2` لتقييد المُنسِّق بمجموعة فرعية صغيرة.
   - راقب:
     - `sorafs_orchestrator_active_fetches`
     - `sorafs_orchestrator_fetch_failures_total{reason!="retry"}`
     - `sorafs_orchestrator_retries_total`
   - تابع عندما تبقى معدلات إعادة المحاولة أقل من 1% لجلب كامل للمانيفست ولا يراكم أي مزوّد إخفاقات.
2. **مرحلة الزيادة (50% من المزوّدين)**
   - زد قيمة `--max-peers` وأعد التشغيل بلقطة تليمترية حديثة.
   - احتفظ بكل تشغيل عبر `--provider-metrics-out` و`--chunk-receipts-out`. احتفظ بالآرتيفاكتات لمدة ≥7 أيام.
3. **إطلاق كامل**
   - أزل `--max-peers` (أو اضبطه على العدد الكامل للمزوّدين المؤهلين).
   - فعّل وضع المُنسِّق في نشرات العملاء: وزّع لوحة النتائج المحفوظة وملف JSON للإعدادات عبر نظام إدارة الإعدادات لديك.
   - حدّث لوحات المتابعة لعرض `sorafs_orchestrator_fetch_duration_ms` p95/p99 ومدرجات إعادة المحاولة حسب المنطقة.

## 3. حظر وتعزيز النظراء

استخدم تجاوزات سياسة التقييم في CLI لفرز المزوّدين غير الأصحاء دون انتظار تحديثات الحوكمة.

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
```

- `--deny-provider` يزيل الاسم المستعار المدرج من الاعتبار في الجلسة الحالية.
- `--boost-provider=<alias>=<weight>` يرفع وزن المُزوّد في المُجدول. القيم تُضاف إلى الوزن المعياري للوحة النتائج وتُطبق فقط على التشغيل المحلي.
- سجّل التجاوزات في تذكرة الحادثة وأرفق مخرجات JSON لكي يتمكن الفريق المسؤول من تسوية الحالة بعد إصلاح المشكلة الأصلية.

بالنسبة للتغييرات الدائمة، عدّل التليمترية المصدرية (ضع علامة penalised على المخالف) أو حدّث الإعلان بميزانيات تدفق مُحدّثة قبل إزالة تجاوزات CLI.

## 4. تشخيص الإخفاقات

عندما يفشل fetch:

1. التقط الآرتيفاكتات التالية قبل إعادة التشغيل:
   - `scoreboard.json`
   - `session.summary.json`
   - `chunk_receipts.json`
   - `provider_metrics.json`
2. افحص `session.summary.json` لسلسلة الخطأ المقروءة:
   - `no providers were supplied` → تحقّق من مسارات المزوّدين والإعلانات.
   - `retry budget exhausted ...` → زد `--retry-budget` أو أزل النظراء غير المستقرين.
   - `no compatible providers available ...` → دقّق بيانات قدرات النطاق للمزوّد المخالف.
3. اربط اسم المزوّد مع `sorafs_orchestrator_provider_failures_total` وافتح تذكرة متابعة إذا ارتفعت المؤشرات.
4. أعد تشغيل fetch دون اتصال باستخدام `--scoreboard-json` والتليمترية الملتقطة لإعادة إنتاج الفشل بشكل حتمي.

## 5. الرجوع

لإرجاع إطلاق المُنسِّق:

1. وزّع إعدادًا يضبط `--max-peers=1` (يعطّل فعليًا الجدولة متعددة المصادر) أو أعد العملاء إلى مسار fetch الأحادي المصدر القديم.
2. أزل أي تجاوزات `--boost-provider` كي تعود لوحة النتائج إلى وزن محايد.
3. واصل جمع مقاييس المُنسِّق لمدة يوم واحد على الأقل للتأكد من عدم وجود عمليات fetch عالقة.

يضمن الانضباط في التقاط الآرتيفاكتات والإطلاق المرحلي إمكانية تشغيل المُنسِّق متعدد المصادر بأمان عبر أساطيل مزوّدين غير متجانسة مع الحفاظ على متطلبات الرصد والتدقيق.
