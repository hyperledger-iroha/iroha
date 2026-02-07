---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/orchestrator-ops.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: عمليات الأوركسترا
العنوان: دليل التشغيل للاستثناءات الموسيقية SoraFS
Sidebar_label: Runbook оркестратора
الوصف: عملية لاحقة للنشر والمراقبة وإخراج أوركسترا متعددة التخصصات.
---

:::note Канонический источник
هذا الجزء يعرض `docs/source/sorafs/runbooks/sorafs_orchestrator_ops.md`. بعد أن تكون النسخ متزامنة، من خلال وثائق أبو الهول، لن تكون الهجرة الكاملة.
:::

يوفر دليل التشغيل هذا SRE من خلال التطوير والتوسع والإضافة لمنسق الجلب متعدد الاستخدامات. من خلال إجراءات التصنيع الإضافية، وتوسيع نطاقات الإنتاج، بما في ذلك الشمول والإجازة بيرو في القائمة السوداء.

> ** سم. أيضًا:** [دليل التشغيل للطرح متعدد الاستخدامات](./multi-source-rollout.md) يدعم توسيع الأسطول الخارجي والبحرية откlonению провайдеров. استخدم نفسك لتنسيق الإدارة / التدريج، هذه الوثيقة — للتميز المسبق للفرقة.

## 1. إعداد قائمة الاختيار

1. **احصل على بيانات مقدم الطلب**
   - التحقق من البيانات التالية (`ProviderAdvertV1`) وأجهزة قياس المسافة الصغيرة لجميع الأسطول.
   - حمولة الخطة (`plan.json`)، تم الحصول عليها من البيان التجريبي.
2. ** تنسيق لوحة النتائج المحددة **

   ```bash
   sorafs_fetch \
     --plan fixtures/plan.json \
     --telemetry-json fixtures/telemetry.json \
     --provider alpha=fixtures/provider-alpha.bin \
     --provider beta=fixtures/provider-beta.bin \
     --provider gamma=fixtures/provider-gamma.bin \
     --scoreboard-out artifacts/scoreboard.json \
     --json-out artifacts/session.summary.json
   ```- تأكد من أن `artifacts/scoreboard.json` يتصل بمقدم الخدمة مثل `eligible`.
   - أرشفة JSON-svoodky مع لوحة النتائج؛ يقوم مدققو الحسابات بإعادة النظر في مسألة الحصول على شهادة.
3. **التشغيل الجاف مع التركيبات** — يمكنك التحكم في التركيبات العامة من `docs/examples/sorafs_ci_sample/` لتتمكن من الاستماع إلى منسق البينارنيك قم بالموافقة على الإصدار الأخير، قبل أن تقوم ببيع الحمولات الإضافية.

## 2. إجراء الطرح القابل للتنفيذ

1. **كاناريك (معدل ≥2)**
   - انتقل إلى لوحة النتائج واضغط على `--max-peers=2` لتتمكن من تحسين أداء الأوركسترا.
   - المراقبة:
     -`sorafs_orchestrator_active_fetches`
     -`sorafs_orchestrator_fetch_failures_total{reason!="retry"}`
     -`sorafs_orchestrator_retries_total`
   - قم بالتمديد عندما تسحب مبلغًا أقل من 1% لجلب البيان بالكامل ولن يقوم موفر الخدمة الخاص بك بإلغاء الاشتراك.
2. **السعر المناسب (50% خصم)**
   - احصل على `--max-peers` وقم بتوصيله بأجهزة قياس عن بعد فائقة الدقة.
   - قم بالاشتراك مع `--provider-metrics-out` و`--chunk-receipts-out`. المصنوعات الجرانيتية ≥7 أيام.
3. **الطرح الكامل**
   - استخدم `--max-peers` (أو قم بتثبيته على نطاق واسع مؤهل).
   - تضمين نظام الأوركسترا في عمليات العملاء: لوحة النتائج المضمنة وتكوين JSON من خلال نظام ضبط التكوين.
   - قم بالتعرف على لوحات الاتصال لإظهار `sorafs_orchestrator_fetch_duration_ms` p95/p99 والتسجيل في المنطقة.## 3. الحجب والحماية من البيرو

استخدم تجاوزات النقاط السياسية في CLI لحل مشكلات مقدمي الخدمة دون إدارة الحوكمة.

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

- `--deny-provider` يستبعد الاسم المستعار الأوزبكي من rassmotreniia в текущей сессии.
- `--boost-provider=<alias>=<weight>` تم إرساله إليك عبر المخطط. تمت إضافة التشجيع إلى تطبيع كل لوحة النتائج وتشجيع فقط على اللعب المحلي.
- قم بتأكيد التجاوزات في تذكرة الحدث واستخدام خيارات JSON التي يمكنك من خلالها توجيه الأمر التالي تحسين الجودة.

للزيادة المستمرة في استخدام أجهزة القياس عن بعد (ضبط الجرعات حسب العقوبة) أو إعادة الإعلان باستخدام الإعلانات المحظورة خطط الميزانية لتجاوز CLI.

## 4. Разбой сбоев

وقت الجلب:1. قبل اكتشاف القطع الأثرية التالية:
   -`scoreboard.json`
   -`session.summary.json`
   -`chunk_receipts.json`
   -`provider_metrics.json`
2. تحقق من `session.summary.json` على أجزاء الجهاز العصبي:
   - `no providers were supplied` → التحقق من الطريق إلى التحقق والموافقة.
   - `retry budget exhausted ...` → قم بإلغاء تحديد `--retry-budget` أو قم بإلغاء تحديد البيروقراطية غير المستقرة.
   - `no compatible providers available ...` → التحقق من موفر الطاقة المتغير.
3. قم بتثبيت المصحح باستخدام `sorafs_orchestrator_provider_failures_total` وقم بإنشاء مراقبة التذاكر، إذا تم استخدام المقياس بسهولة.
4. قم بالجلب دون الاتصال بالإنترنت باستخدام `--scoreboard-json` وجهاز قياس عن بعد مخصص لتحديد الاتصال.

## 5. التراجع

كيفية حذف منسق الطرح:

1. التكوين الشامل مع `--max-peers=1` (التخطيط الفعلي المتعدد التخصصات) أو العملاء الدائمين طريقة جلب فريدة من نوعها.
2. تجاوز أي شيء تريده `--boost-provider`، بحيث تكون لوحة النتائج قابلة للتحويل إلى أي اتجاه.
3. قم بمتابعة الحد الأدنى من مجموعات القياسات اللازمة لإعادة تشغيل عملية الجلب.

تضمن المصنوعات اليدوية المتميزة وعمليات النشر القابلة للتنزيل استثناءً مجانيًا من قبل منظمين متعددي التخصصات يتم توفير رحلات بحرية مختلفة من خلال الرعاية الصحية من خلال المراقبة والتدقيق.