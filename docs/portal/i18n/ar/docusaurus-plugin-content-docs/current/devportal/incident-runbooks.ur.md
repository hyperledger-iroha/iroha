---
lang: ar
direction: rtl
source: docs/portal/docs/devportal/incident-runbooks.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# رن بوكس وروول بيك جيرلز

## القصد

قواعد اللعبة **DOCS-9** كتيبات اللعب القابلة للعمل وخطة التدريب التي تتطلبها وتأخذها
تعد بورتي آربرز من أفضل شركات صناعة الأفلام في العالم. هذه ملاحظة جديدة
الإنترنت — عمليات النشر الثانية، وتدهور النسخ المتماثل، وانقطاع التحليلات — ما هو الخطأ
والتدريبات ربع السنوية لتكوين البنات وبطاقة ثابتة ثابتة والاسم المستعار التراجع والتحقق من الصحة الاصطناعية
يتم أيضًا الانتهاء من النهاية إلى النهاية.

### مواضيعہ مواد

- [`devportal/deploy-guide`](./deploy-guide) — التعبئة والتغليف والتوقيع والاسم المستعار لسير عمل الترويج ۔
- [`devportal/observability`](./observability) — إطلاق العلامات والتحليلات والمسبارات جن کا نیچے حوالہ ہے۔
-`docs/source/sorafs_node_client_protocol.md`
  و[`sorafs/pin-registry-ops`](../sorafs/pin-registry-ops)
  - قياس التسجيل عن بعد وعتبات التصعيد ۔
- مساعدين `docs/portal/scripts/sorafs-pin-release.sh` و`npm run probe:*`
  جو شيك ليس ريفيرنس.

### أدوات القياس عن بعد والأدوات

| إشارة / أداة | قصدي |
| ------------- | ------- |
| `torii_sorafs_replication_sla_total` (تم اللقاء/الفائت/معلق) | أكشاك النسخ المتماثل وخروقات جيش تحرير السودان تكتشف المخالفات. |
| `torii_sorafs_replication_backlog_total`، `torii_sorafs_replication_completion_latency_epochs` | عمق الأعمال المتراكمة وزمن الوصول للإكمال والفرز لتحديد الكمية. |
| `torii_sorafs_gateway_refusals_total`، `torii_sorafs_manifest_submit_total{status="error"}` | تم نشر حالات الفشل من جانب البوابة وتم نشر المزيد من المعلومات بعد ذلك. |
| `npm run probe:portal` / `npm run probe:tryit-proxy` | تقوم المجسات الاصطناعية بإطلاق البوابة والتراجع عن التحقق من صحة البطاقة. |
| `npm run check:links` | بوابة مكسورة؛ يتم التخفيف بعد الاستخدام. |
| `sorafs_cli manifest submit ... --alias-*` (`scripts/sorafs-pin-release.sh` ذریعے) | آلية ترقية/إرجاع الاسم المستعار ۔ |
| لوحة `Docs Portal Publishing` Grafana (`dashboards/grafana/docs_portal.json`) | الرفض/الاسم المستعار/TLS/القياس عن بعد النسخ المتماثل هو مجموع النقاط. تشير تنبيهات PagerDuty إلى اللوحات والأدلة. |

## Runbook - نشر أو قطعة أثرية خراب

### بداية ہونے كی منفصلة

- فشل تحقيقات المعاينة/الإنتاج (`npm run probe:portal -- --expect-release=...`).
- تنبيهات Grafana على `torii_sorafs_gateway_refusals_total` أو
  تم طرح `torii_sorafs_manifest_submit_total{status="error"}` بعد ذلك.
- الترويج للاسم المستعار لضمان الجودة يدويًا فورًا بعد المسارات المعطلة أو حاول فشل الوكيل ملاحظة کرے۔

### فورى روک ھام

1. **تجميد عمليات النشر:** علامة خط أنابيب CI `DEPLOY_FREEZE=1` (إدخال سير عمل GitHub)
   لم يعد الإيقاف المؤقت لوظيفة جينكينز يزيد من المصنوعات اليدوية.
2. **التقاط المصنوعات اليدوية:** فشل البناء `build/checksums.sha256`،
   `portal.manifest*.{json,to,bundle,sig}`، ويمكن لإخراج المسبار تنزيل التراجع
   هذا الملخص هو مرجع کرے۔
3. **اطلع على أصحاب المصلحة:** تخزين SRE، قائد Docs/DevRel، والمسؤول المناوب للحوكمة
   (خصوصاً جب `docs.sora` مثاثرہو)۔

### طريقة عمل رول بيك

1. بيان آخر سلعة معروفة (LKG) سير عمل الإنتاج
   `artifacts/devportal/<release>/sorafs/portal.manifest.to` هو المسؤول عن ذلك.
2. اسم مستعار لمساعد الشحن وهو بيان للربط مرة أخرى:

```bash
cd docs/portal
./scripts/sorafs-pin-release.sh \
  --build-dir build \
  --artifact-dir artifacts/revert-$(date +%Y%m%d%H%M) \
  --sorafs-dir artifacts/revert-$(date +%Y%m%d%H%M)/sorafs \
  --pin-min-replicas 5 \
  --alias "docs-prod-revert" \
  --alias-namespace "${PIN_ALIAS_NAMESPACE}" \
  --alias-name "${PIN_ALIAS_NAME}" \
  --alias-proof "${PIN_ALIAS_PROOF_PATH}" \
  --torii-url "${TORII_URL}" \
  --submitted-epoch "$(date +%Y%m%d)" \
  --authority "${AUTHORITY}" \
  --private-key "${PRIVATE_KEY}" \
  --skip-submit

# swap in the LKG artefacts before submission
cp /secure/archive/lkg/portal.manifest.to artifacts/.../sorafs/portal.manifest.to
cp /secure/archive/lkg/portal.manifest.bundle.json artifacts/.../sorafs/

cargo run -p sorafs_orchestrator --bin sorafs_cli -- \
  manifest submit \
  --manifest artifacts/.../sorafs/portal.manifest.to \
  --chunk-plan artifacts/.../sorafs/portal.plan.json \
  --torii-url "${TORII_URL}" \
  --authority "${AUTHORITY}" \
  --private-key "${PRIVATE_KEY}" \
  --alias-namespace "${PIN_ALIAS_NAMESPACE}" \
  --alias-name "${PIN_ALIAS_NAME}" \
  --alias-proof "${PIN_ALIAS_PROOF_PATH}" \
  --metadata rollback_from="${FAILED_RELEASE}" \
  --summary-out artifacts/.../sorafs/rollback.submit.json
```

3. يتضمن ملخص التراجع عن تذكرة الحادث LKG وملخصات البيان الفاشلة التي تدوم طويلاً.

### توثیق

1.`npm run probe:portal -- --expect-release=${LKG_TAG}`.
2.`npm run check:links`.
3.`sorafs_cli manifest verify-signature ...` و`sorafs_cli proof verify ...`
   (نشر الدليل هنا) قم بالنقر على بيان إعادة ترقيته المؤرشف CAR الذي يتطابق مع الكرتا.
4. `npm run probe:tryit-proxy` استخدم الوكيل المرحلي Try-It.

### الحقيقة التالية

1. سيتم تحديد السبب الجذري بعد إعادة تنشيط خط أنابيب النشر.
2. [`devportal/deploy-guide`](./deploy-guide) تتضمن إدخالات "الدروس المستفادة" نقاطًا جديدة.
3. مجموعة الاختبارات الفاشلة (المسبار، مدقق الارتباط، إلخ) لتوضيح العيوب.

## Runbook - نسخة احتياطية كاملة

### بداية ہونے كی منفصلة

- الرٹ: `sum(torii_sorafs_replication_sla_total{outcome="met"}) /
  المشبك_مين(sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}), 1) <
  0.95` 10 طن.
- `torii_sorafs_replication_backlog_total > 10` 10 ميكرون (`pin-registry-ops.md` ديكهي).
- الحوكمة تعتمد على الاسم المستعار للأجهزة في تقرير التقرير.

### ابتدائی جانچ

1. [`sorafs/pin-registry-ops`](../sorafs/pin-registry-ops) لوحات المعلومات تعمل على تقليل تراكم الأعمال المتراكمة
   لا تقتصر فئة التخزين أو أسطول الموفر على أي فئة.
2. يسجل Torii تحذيرات `sorafs_registry::submit_manifest` التي تعلم متى تفشل عمليات الإرسال أو لا.
3. `sorafs_cli manifest status --manifest ...` نسخة طبق الأصل من عينة الصحة (النتائج لكل مزود).

### تخفيفی الإجراءات

1. `scripts/sorafs-pin-release.sh` عدد النسخ المتماثلة المتزايد (`--pin-min-replicas 7`) والذي سيستمر في الظهور مرة أخرى
   هناك العديد من مقدمي الخدمة الذين يقومون بتحميل جدولة البيانات. لدينا ملخص لسجل الحوادث.
2. إذا تم إيقاف تشغيل أي موفر متراكم، فسيتم تعطيل جدولة النسخ المتماثل
   (تم توثيق `pin-registry-ops.md`) وبيان الإرسال الجديد لمقدمي الخدمات الداعمين والاسم المستعار لتحديث المعلومات.
3. إذا كانت نضارة الاسم المستعار، فإن تكافؤ النسخ المتماثل سيكون أمرًا بالغ الأهمية، كما أن الاسم المستعار الخاص بك قد تم تنظيمه بشكل واضح (`docs-preview`) ثم إعادة الربط،
   قم بمسح قائمة SRE المتراكمة بعد نشر بيان المتابعة.

### بحالی واختتام

1.`torii_sorafs_replication_sla_total{outcome="missed"}` مراقبة مستوى الهضبة وحسابها.
2. `sorafs_cli manifest status` إخراج الأدلة التي تم التقاطها وهي متوافقة مرة أخرى مع النسخة المتماثلة.
3. تراكم النسخ المتماثل بعد الوفاة للملفات أو النسخ المتماثلة (تحجيم الموفر، ضبط المقسم، إلخ).

## Runbook - ٹیليمیٹری آؤٹیج

### بداية ہونے كی منفصلة

- `npm run probe:portal` قم بحفظ لوحات المعلومات وأحداث `AnalyticsTracker` التي لا يتم استيعابها > 15 دقيقة.
- أسقطت مراجعة الخصوصية الأحداث غير المتوقعة إضافة تقرير جديد.
- `npm run probe:tryit-proxy` `/probe/analytics` مسارات پر فاشلة ہو۔

### جوابی اجراءات

1. التحقق من صحة مدخلات وقت البناء: `DOCS_ANALYTICS_ENDPOINT` و`DOCS_ANALYTICS_SAMPLE_RATE`
   قطعة أثرية فاشلة في الإصدار (`build/release.json`) ۔
2. `DOCS_ANALYTICS_ENDPOINT` نقطة تجميع التدريج `npm run probe:portal` ينبعث منها إشعاع ثابت من حمولات المتعقب.
3. إذا انخفض المجمعون إلى `DOCS_ANALYTICS_ENDPOINT=""` قم بتعيين الكرات وإعادة بناء دائرة قصر تعقب الكرات؛ الجدول الزمني لحوادث نافذة الانقطاع.
4. التحقق من صحة بطاقة الهوية `scripts/check-links.mjs` و`checksums.sha256` بصمة الإصبع
   (لا يحدث انقطاع في التحليلات أو التحقق من صحة خريطة الموقع *الحظر*).
5. يتم استرداد المجمع بعد `npm run test:widgets` ويتم تشغيل اختبارات الوحدة المساعدة للتحليلات ثم إعادة نشر الكتاب.

### الحقيقة التالية

1. [`devportal/observability`](./observability) تتضمن القيود الجديدة على أدوات التجميع أو متطلبات أخذ العينات.
2. إذا قمت بإسقاط سياسة البيانات التحليلية أو تنقيحها، فسيتم إرسال إشعار الحوكمة إليك.

## تم تأسيسها في مصنع

تدريبات دونوں **الربع الرابع من شهر مارس** (يناير/أبريل/يوليو/أكتوبر)
يتم تغيير البنية التحتية أو أي شيء آخر فورًا بعد ذلك. التحف کو
`artifacts/devportal/drills/<YYYYMMDD>/` تحت محفوظات الكريں.

| مشقوق | مراحل | شوہد |
| ----- | ----- | -------- |
| التراجع عن الاسم المستعار کی مشقى | 1. أصبح أحدث بيان إنتاج هو التراجع عن "فشل النشر" مرة أخرى.<br/>2. تمر المجسات بعد الإنتاج لإعادة ربط الكري.<br/>3. `portal.manifest.submit.summary.json` وسجلات التحقيق للتنقيب في السجلات. | `rollback.submit.json`، وإخراج المسبار، وعلامة إصدار البروفة. |
| تقليد قديم | 1. الإنتاج والتدريج کے خلاف `npm run probe:portal` و `npm run probe:tryit-proxy` چلايں.<br/>2. `npm run check:links` أرشيف و`build/link-report.json` أرشيف.<br/>3. لوحات Grafana کے لقطات/الصادرات إرفاق کریں تأكيد نجاح مسبار جو کریں۔ | سجلات التحقيق + `link-report.json` بصمة الإصبع الواضحة |

تؤدي التدريبات الفائتة التي يقوم بها مدير Docs/DevRel ومراجعة حوكمة SRE إلى تصعيد العمل، وهي خريطة طريق ومتطلبات مطلوبة.
التراجع عن الاسم المستعار وتحقيقات البوابة هو دليل حتمي وفصلي موجود بالفعل.

## PagerDuty والتنسيق عند الطلب

- خدمة PagerDuty **Docs Portal Publishing** `dashboards/grafana/docs_portal.json` تنبيهات متجددة.
  المتطلبات `DocsPortal/GatewayRefusals` و`DocsPortal/AliasCache` و`DocsPortal/TLSExpiry` صفحة Docs/DevRel الأساسية
  والتخزين SRE ثانوي ہوتا ے۔
- تحتوي الصفحة على `DOCS_RELEASE_TAG` على لوحات كریں، ومتاثرہ Grafana ولقطات شاشة مرفقة کریں، والتخفيف من بدء تشغيل کرنے سے پہلے
  يتم تضمين مخرجات التحقيق/التحقق من الارتباط وملاحظات الحادث في الارتباط.
- التخفيف (التراجع أو إعادة النشر) بعد `npm run probe:portal`، `npm run check:links` تكرار، وGrafana
  تلتقط اللقطات مقاييس الجودة والعتبات في كل مكان. جميع الأدلة حادثة PagerDuty التي تم إرفاقها بالبطاقة
  قبل أن تقرر ما هو السبب.
- إذا تم إطلاق تنبيهات أخرى (على سبيل المثال لانتهاء صلاحية TLS وتراكم الأعمال)، فإن رفضك يؤدي إلى الفرز
  (نشر الروك)، إجراء التراجع، وهو عبارة عن تخزين SRE وهو جسر ثابت عبر TLS/backlog.