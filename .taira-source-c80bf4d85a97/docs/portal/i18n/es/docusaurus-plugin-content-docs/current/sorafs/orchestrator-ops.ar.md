---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/orchestrator-ops.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: operaciones de orquestador
título: دليل تشغيل مُنسِّق SoraFS
sidebar_label: دليل تشغيل المُنسِّق
descripción: دليل تشغيلي خطوة بخطوة لنشر المُنسِّق متعدد المصادر ومراقبته والرجوع عنه.
---

:::nota المصدر المعتمد
Utilice el botón `docs/source/sorafs/runbooks/sorafs_orchestrator_ops.md`. احرص على إبقاء النسختين متزامنتين إلى أن تكتمل هجرة مجموعة توثيق Sphinx القديمة بالكامل.
:::

يرشد هذا الدليل فرق SRE خلال التحضير والنشر وتشغيل مُنسِّق الجلب متعدد المصادر. يكمل دليل المطورين بإجراءات مضبوطة لعمليات النشر في الإنتاج، بما في ذلك التفعيل المرحلي وإدراج النظراء في القائمة السوداء.

> **راجع أيضًا:** يركّز [دليل إطلاق متعدد المصادر](./multi-source-rollout.md) على موجات الإطلاق على مستوى الأسطول وعلى منع المزوّدين في حالات الطوارئ. ارجع إليه لتنسيق الحوكمة / بيئة الاختبار المرحلية بينما تستخدم هذا المستند لعمليات المُنسِّق اليومية.

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
   ```- تحقّق من أن `artifacts/scoreboard.json` يسرد كل مزوّد إنتاجي على أنه `eligible`.
   - أرشف ملخص JSON جنبًا إلى جنب مع لوحة النتائج؛ يعتمد المدققون على عدّادات إعادة محاولة الـ trozos عند اعتماد طلب التغيير.
3. **تشغيل تجريبي باستخدام luminarias** — نفّذ الأمر نفسه على luminarias العامة في `docs/examples/sorafs_ci_sample/` للتأكد من أن ثنائية المُنسِّق تطابق الإصدار المتوقع قبل لمس حمولات الإنتاج.

## 2. إجراء الإطلاق المرحلي

1. **مرحلة الكناري (≤2 مزوّدين)**
   - Utilice el cable de alimentación `--max-peers=2` para conectar el cable de alimentación.
   - راقب:
     - `sorafs_orchestrator_active_fetches`
     - `sorafs_orchestrator_fetch_failures_total{reason!="retry"}`
     - `sorafs_orchestrator_retries_total`
   - تابع عندما تبقى معدلات إعادة المحاولة أقل من لجلب كامل للمانيفست ولا يراكم أي مزوّد إخفاقات.
2. **مرحلة الزيادة (50% del dinero)**
   - Presione `--max-peers` y presione el botón de encendido.
   - Utilice el código `--provider-metrics-out` y `--chunk-receipts-out`. La cantidad de agua debe ser mayor o igual a 7 días.
3. **إطلاق كامل**
   - Haga clic en `--max-peers` (envíe un mensaje de texto a la página de inicio).
   - فعّل وضع المُنسِّق في نشرات العملاء: وزّع لوحة النتائج المحفوظة y JSON للإعدادات عبر نظام إدارة الإعدادات لديك.
   - Haga clic en el botón `sorafs_orchestrator_fetch_duration_ms` p95/p99 y en el teclado.

## 3. حظر وتعزيز النظراء

Utilice el CLI para conectar el dispositivo a su dispositivo móvil.

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
- Configuración de archivos JSON y archivos JSON relacionados con archivos JSON. المشكلة الأصلية.

بالنسبة للتغييرات الدائمة, عدّل التليمترية المصدرية (ضع علامة penalizado على المخالف) أو حدّث الإعلان بميزانيات تدفق مُحدّثة قبل إزالة تجاوزات CLI.

## 4. تشخيص الإخفاقات

عندما يفشل buscar:

1. التقط الآرتيفاكتات التالية قبل إعادة التشغيل:
   - `scoreboard.json`
   - `session.summary.json`
   - `chunk_receipts.json`
   - `provider_metrics.json`
2. Haga clic en `session.summary.json` para obtener más información:
   - `no providers were supplied` → تحقّق من مسارات المزوّدين والإعلانات.
   - `retry budget exhausted ...` → زد `--retry-budget` أو أزل النظراء غير المستقرين.
   - `no compatible providers available ...` → دقّق بيانات قدرات النطاق للمزوّد المخالف.
3. Conecte el dispositivo `sorafs_orchestrator_provider_failures_total` y conecte el dispositivo a la unidad.
4. أعد تشغيل fetch دون اتصال باستخدام `--scoreboard-json` والتليمترية الملتقطة لإعادة إنتاج الفشل بشكل حتمي.

## 5. الرجوع

لإرجاع إطلاق المُنسِّق:

1. وزّع إعدادًا يضبط `--max-peers=1` (يعطّل فعليًا الجدولة متعددة المصادر) أو أعد العملاء إلى مسار fetch الأحادي المصدر القديم.
2. أزل أي تجاوزات `--boost-provider` كي تعود لوحة النتائج إلى وزن محايد.
3. واصل جمع مقاييس المُنسِّق لمدة يوم واحد على الأقل للتأكد من عدم وجود عمليات buscar عالقة.يضمن الانضباط في التقاط الآرتيفاكتات والإطلاق المرحلي إمكانية تشغيل المُنسِّق متعدد المصادر بأمان عبر أساطيل مزوّدين غير متجانسة مع الحفاظ على متطلبات الرصد والتدقيق.