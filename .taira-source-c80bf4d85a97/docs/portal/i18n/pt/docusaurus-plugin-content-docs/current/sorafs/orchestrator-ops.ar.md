---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/orchestrator-ops.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: orquestrador-ops
título: دليل تشغيل مُنسِّق SoraFS
sidebar_label: دليل تشغيل المُنسِّق
description: دليل تشغيلي خطوة بخطوة لنشر المُنسِّق متعدد المصادر ومراقبته والرجوع عنه.
---

:::note المصدر المعتمد
Verifique o valor `docs/source/sorafs/runbooks/sorafs_orchestrator_ops.md`. احرص على إبقاء النسختين متزامنتين إلى أن تكتمل هجرة مجموعة توثيق Sphinx القديمة بالكامل.
:::

يرشد هذا الدليل فرق SRE خلال التحضير والنشر وتشغيل مُنسِّق الجلب متعدد المصادر. يكمل دليل المطورين بإجراءات مضبوطة لعمليات النشر في الإنتاج, بما في ذلك التفعيل المرحلي وإدراج Não há nada melhor do que isso.

> **راجع أيضًا:** يركّز [دليل إطلاق متعدد المصادر](./multi-source-rollout.md) على موجات الإطلاق على مستوى Você pode fazer isso sem precisar de ajuda. ارجع إليه لتنسيق الحوكمة / بيئة الاختبار المرحلية بينما تستخدم هذا المستند لعمليات المُنسِّق اليومية.

## 1. قائمة التحقق قبل التنفيذ

1. **جمع مدخلات المزوّدين**
   - Verifique o código de barras (`ProviderAdvertV1`) e verifique o valor.
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

   - Verifique se o `artifacts/scoreboard.json` está conectado ao `eligible`.
   - أرشف ملخص JSON جنبًا إلى جنب مع لوحة النتائج؛ يعتمد المدققون على عدّادات إعادة محاولة الـ pedaços, عند اعتماد طلب التغير.
3. **تشغيل تجريبي باستخدام fixtures** — نفّذ الأمر نفسه على fixtures العامة في `docs/examples/sorafs_ci_sample/` للتأكد من أن ثنائية المُنسِّق تطابق الإصدار المتوقع قبل لمس حمولات الإنتاج.

## 2. إجراء الإطلاق المرحلي

1. **مرحلة الكناري (≤2 مزوّدين)**
   - أعد بناء لوحة النتائج وشغّل باستخدام `--max-peers=2` لتقييد المُنسِّق بمجموعة فرعية Bem.
   - راقب:
     -`sorafs_orchestrator_active_fetches`
     -`sorafs_orchestrator_fetch_failures_total{reason!="retry"}`
     -`sorafs_orchestrator_retries_total`
   - تابع عندما تبقى معدلات إعادة المحاولة أقل من 1% لجلب كامل للمانيفست ولا يراكم أي مزوّد Então.
2. **مرحلة الزيادة (50% de cada mês)**
   - Verifique se o `--max-peers` está funcionando corretamente.
   - Você pode usar o `--provider-metrics-out` e `--chunk-receipts-out`. احتفظ بالآرتيفاكتات لمدة ≥7 dias.
3. **إطلاق كامل**
   - أزل `--max-peers` (أو اضبطه على العدد الكامل للمزوّدين المؤهلين).
   - فعّل وضع المُنسِّق في نشرات العملاء: وزّع لوحة النتائج المحفوظة وملف JSON للإعدادات عبر Não se preocupe.
   - Verifique se o `sorafs_orchestrator_fetch_duration_ms` p95/p99 está instalado e se está funcionando corretamente.

## 3. حظر وتعزيز النظراء

Você pode usar o CLI para obter mais informações sobre o produto.

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
- `--boost-provider=<alias>=<weight>` não está disponível. Você pode usar o aplicativo para obter mais informações sobre o produto.
- سجّل التجاوزات في تذكرة الحادثة وأرفق مخرجات JSON لكي يتمكن الفريق المسؤول من تسوية الحالة بعد إصلاح المشكلة الأصلية.

بالنسبة للتغييرات الدائمة, عدّل التليمترية المصدرية (ضع علامة penalizado على المخالف) أو حدّث الإعلان Você pode usar o CLI para fazer isso.

## 4. تشخيص الإخفاقات

عندما يفشل fetch:1. Como fazer isso:
   -`scoreboard.json`
   -`session.summary.json`
   -`chunk_receipts.json`
   -`provider_metrics.json`
2. Use `session.summary.json` para configurar o código:
   - `no providers were supplied` → تحقّق من مسارات المزوّدين والإعلانات.
   - `retry budget exhausted ...` → é `--retry-budget` ou não.
   - `no compatible providers available ...` → دقّق بيانات قدرات النطاق للمزوّد المخالف.
3. Verifique o valor do `sorafs_orchestrator_provider_failures_total` e verifique o valor do produto.
4. Selecione fetch دون اتصال باستخدام `--scoreboard-json` e والتليمترية الملتقطة لإعادة إنتاج الفشل بشكل حتمي.

## 5. الرجوع

لإرجاع إطلاق المُنسِّق:

1. وزّع إعدادًا يضبط `--max-peers=1` (يعطّل فعليًا الجدولة متعددة المصادر) أو أعد العملاء إلى مسار fetch الأحادي المصدر القديم.
2. Use o `--boost-provider` para remover o cabo de alimentação e o produto.
3. واصل جمع مقاييس المُنسِّق لمدة يوم واحد على الأقل للتأكد من عدم وجود عمليات fetch عالقة.

يضمن الانضباط في التقاط الآرتيفاكتات والإطلاق المرحلي إمكانية تشغيل المُنسِّق متعدد المصادر Não se preocupe, você pode fazer isso com uma chave de fenda.