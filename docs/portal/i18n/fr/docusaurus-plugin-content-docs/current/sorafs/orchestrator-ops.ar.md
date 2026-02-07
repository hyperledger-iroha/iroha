---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/orchestrator-ops.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : orchestrator-ops
titre : دليل تشغيل مُنسِّق SoraFS
sidebar_label : دليل تشغيل المُنسِّق
description: دليل تشغيلي خطوة بخطوة لنشر المُنسِّق متعدد المصادر ومراقبته والرجوع عنه.
---

:::note المصدر المعتمد
Il s'agit de la référence `docs/source/sorafs/runbooks/sorafs_orchestrator_ops.md`. احرص على إبقاء النسختين متزامنتين إلى أن تكتمل هجرة مجموعة توثيق Sphinx القديمة بالكامل.
:::

يرشد هذا الدليل فرق SRE خلال التحضير والنشر وتشغيل مُنسِّق الجلب متعدد المصادر. يكمل دليل المطورين بإجراءات مضبوطة لعمليات النشر في الإنتاج، بما في ذلك التفعيل المرحلي وإدراج النظراء في القائمة السوداء.

> **راجع أيضًا:** يركّز [دليل إطلاق متعدد المصادر](./multi-source-rollout.md) على موجات الإطلاق على مستوى الأسطول وعلى منع المزوّدين في حالات الطوارئ. ارجع إليه لتنسيق الحوكمة / بيئة الاختبار المرحلية بينما تستخدم هذا المستند لعمليات المُنسِّق اليومية.

## 1. قائمة التحقق قبل التنفيذ

1. **جمع مدخلات المزوّدين**
   - أحدث إعلانات المزوّدين (`ProviderAdvertV1`) et التليمترية للأسطول المستهدف.
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
   ```- تحقّق من أن `artifacts/scoreboard.json` يسرد كل مزوّد إنتاجي على أنه `eligible`.
   - JSON est compatible avec les applications JSON Vous devez utiliser des morceaux pour obtenir des morceaux.
3. **تشغيل تجريبي باستخدام luminaires** — نفّذ الأمر نفسه على luminaires العامة في `docs/examples/sorafs_ci_sample/` للتأكد من أن ثنائية المُنسِّق تطابق الإصدار المتوقع قبل لمس حمولات الإنتاج.

## 2. إجراء الإطلاق المرحلي

1. **مرحلة الكناري (≤2 مزوّدين)**
   - أعد بناء لوحة النتائج وشغّل باستخدام `--max-peers=2` لتقييد المُنسِّق بمجموعة فرعية صغيرة.
   - راقب :
     -`sorafs_orchestrator_active_fetches`
     -`sorafs_orchestrator_fetch_failures_total{reason!="retry"}`
     -`sorafs_orchestrator_retries_total`
   - تابع عندما تبقى معدلات إعادة المحاولة أقل من 1% لجلب كامل للمانيفست ولا يراكم أي مزوّد إخفاقات.
2. ** مرحلة الزيادة (50% من المزوّدين)**
   - زد قيمة `--max-peers` وأعد التشغيل بلقطة تليمترية حديثة.
   - احتفظ بكل تشغيل عبر `--provider-metrics-out` et `--chunk-receipts-out`. احتفظ بالآرتيفاكتات لمدة ≥7 أيام.
3. **إطلاق كامل**
   - Sélectionnez `--max-peers` (si vous avez besoin d'aide).
   - Il s'agit d'une application pour les applications : et JSON pour les utilisateurs إدارة الإعدادات لديك.
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
```- `--deny-provider` يزيل الاسم المستعار المدرج من الاعتبار في الجلسة الحالية.
- `--boost-provider=<alias>=<weight>` يرفع وزن المُزوّد ​​في المُجدول. القيم تُضاف إلى الوزن المعياري للوحة النتائج وتُطبق فقط على التشغيل المحلي.
- Les fonctionnalités de JSON sont également compatibles avec les applications JSON. إصلاح المشكلة الأصلية.

بالنسبة للتغييرات الدائمة، عدّل التليمترية المصدرية (ضع علامة pénalisé على المخالف) ou حدّث الإعلان بميزانيات تدفق مُحدّثة قبل إزالة تجاوزات CLI.

## 4. تشخيص الإخفاقات

عندما يفشل récupérer:

1. التقط الآرتيفاكتات التالية قبل إعادة التشغيل:
   -`scoreboard.json`
   -`session.summary.json`
   -`chunk_receipts.json`
   -`provider_metrics.json`
2. Utilisez `session.summary.json` pour la prise en charge :
   - `no providers were supplied` → تحقّق من مسارات المزوّدين والإعلانات.
   - `retry budget exhausted ...` → زد `--retry-budget` أو أزل النظراء غير المستقرين.
   - `no compatible providers available ...` → دقّق بيانات قدرات النطاق للمزوّد المخالف.
3. اربط اسم المزوّد مع `sorafs_orchestrator_provider_failures_total` وافتح تذكرة متابعة إذا ارتفعت المؤشرات.
4. أعد تشغيل fetch دون اتصال byاستخدام `--scoreboard-json` والتليمترية الملتقطة لإعادة إنتاج الفشل بشكل حتمي.

## 5. الرجوع

لإرجاع إطلاق المُنسِّق:

1. وزّع إعدادًا يضبط `--max-peers=1` (يعطّل فعليًا الجدولة متعددة المصادر) et أو أعد العملاء إلى مسار récupérer الأحادي المصدر القديم.
2. Placez le module `--boost-provider` dans le panneau de commande.
3. Si vous souhaitez récupérer le fichier, vous devez le récupérer.يضمن الانضباط في التقاط الآرتيفاكتات والإطلاق المرحلي إمكانية تشغيل المُنسِّق متعدد المصادر بأمان عبر أساطيل مزوّدين غير متجانسة مع الحفاظ على متطلبات الرصد والتدقيق.