---
lang: ar
direction: rtl
source: docs/portal/docs/devportal/incident-runbooks.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# التراجع عن أحداث وتدريبات رانبوك

## روعة

البطاقات الصغيرة **DOCS-9** تحتاج إلى قواعد اللعبة المتقنة والخطة التكرارية، لذلك
يمكن إعادة تشغيل بوابة المشغلين بعد التسليم دون تأخير. هذه هي الحقيقة
قم بإلقاء الضوء على ثلاث حوادث شديدة الأهمية - عمليات غير مرغوب فيها وتدهور التكرار و
تحليلاتها - وتوثيق التدريبات الرباعية، الموثقة للاسم المستعار للتراجع
والتحقق من صحة الاصطناعية يستمر في العمل من النهاية إلى النهاية.

### مواد صناعية

- [`devportal/deploy-guide`](./deploy-guide) — التعبئة والتغليف والنشر والاسم المستعار للترويج لسير العمل.
- [`devportal/observability`](./observability) — علامات الإصدار والتحليلات والمسابير والتفاصيل الإضافية.
-`docs/source/sorafs_node_client_protocol.md`
  و [`sorafs/pin-registry-ops`](../sorafs/pin-registry-ops)
  — مقياس القياس عن بعد وتصعيد الخطوات.
- `docs/portal/scripts/sorafs-pin-release.sh` والمساعدين `npm run probe:*`
  تلميحات في الاختيارات.

### أجهزة القياس عن بعد والأدوات الشاملة

| إشارة / أداة | الاسم |
| ------------- | ------- |
| `torii_sorafs_replication_sla_total` (تم اللقاء/الفائت/معلق) | نفذ النسخ المتماثلة واتفاقيات مستوى الخدمة (SLA). |
| `torii_sorafs_replication_backlog_total`، `torii_sorafs_replication_completion_latency_epochs` | قم بتقليص حجم الأعمال المتراكمة وإنهاء إجراءات الفرز. |
| `torii_sorafs_gateway_refusals_total`، `torii_sorafs_manifest_submit_total{status="error"}` | قم بعرض كل شيء باستخدام البوابة الأساسية، وهو ما يتبع كثيرًا للنشر الشامل. |
| `npm run probe:portal` / `npm run probe:tryit-proxy` | المجسات الاصطناعية التي تقوم بإصدار البوابة والتحقق من التراجع. |
| `npm run check:links` | بوابة بيتي ссылок; يتم الاستفادة من كل التخفيف. |
| `sorafs_cli manifest submit ... --alias-*` (من خلال `scripts/sorafs-pin-release.sh`) | آلية الترويج/العودة إلى الاسم المستعار. |
| لوحة `Docs Portal Publishing` Grafana (`dashboards/grafana/docs_portal.json`) | الموافقة على الرفض/الاسم المستعار/TLS/النسخ المتماثل عن بعد. يتم ضبط تنبيهات PagerDuty على هذه اللوحة كوسيلة للتسليم. |

## رانبوك - قطعة أثرية جديدة أو قطعة أثرية مسطحة

### سهولة التصنيع

- اختبار المعاينة/الإنتاج (`npm run probe:portal -- --expect-release=...`).
- تنبيهات Grafana إلى `torii_sorafs_gateway_refusals_total` أو
  `torii_sorafs_manifest_submit_total{status="error"}` بعد الطرح.
- ضمان الجودة يضمن بقاء الجداول الزمنية أو الوكيل الخاص بك جربه بسرعة بعد ذلك
  الاسم المستعار للترويج.

### التوصيل غير المحدود

1. **تفعيل النشر:** حذف خط أنابيب CI `DEPLOY_FREEZE=1` (سير عمل GitHub المُدخل)
   أو قم باستعادة وظيفة Jenkins من أجل عدم إزالة القطع الأثرية الجديدة.
2. **تثبيت العناصر:** تنزيل `build/checksums.sha256`,
   `portal.manifest*.{json,to,bundle,sig}`، وإخراج المسابير من البناء الفاشل،
   чтобы التراجع сылался точные هضم.
3. **التعرف على المستودع:** تخزين SRE، Docs/DevRel الرئيسي، والمسؤول المناوب للحوكمة
   (خاصة إذا كان العميل `docs.sora`).

### إجراء التراجع

1. قم بتقديم بيان آخر سلعة معروفة (LKG). سير عمل الإنتاج gratnit их в
   `artifacts/devportal/<release>/sorafs/portal.manifest.to`.
2. تحويل الاسم المستعار إلى هذا البيان باستخدام مساعد الشحن:

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

3. قم بتدوين ملخص التراجع في تذكرة الحادث مع ملخص LKG والبيان غير الضروري.

### التحقق من الصحة

1.`npm run probe:portal -- --expect-release=${LKG_TAG}`.
2.`npm run check:links`.
3.`sorafs_cli manifest verify-signature ...` و`sorafs_cli proof verify ...`
   (دليل النشر)، لتتمكن من قراءة البيان المعاد ترقيته المصاحب لأرشيف CAR.
4.`npm run probe:tryit-proxy` للتأكيد على تفعيل وكيل التدريج Try-It.

### النتيجة النهائية

1. قم بإغلاق نشر خط الأنابيب بعد معالجة السبب الجذري فقط.
2. قم بإضافة "الدروس المستفادة" إلى [`devportal/deploy-guide`](./deploy-guide)
   novыми выводами، إذا كان الأمر كذلك.
3. عيوب اختبارات التحقق (المسبار، مدقق الارتباط، وما إلى ذلك).

## رانبوك - تدهور النسخ المتماثلة

### سهولة التصنيع

- تنبيه: `sum(torii_sorafs_replication_sla_total{outcome="met"}) /
  المشبك_مين(sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}), 1) <
  0.95` في التقنية 10 دقائق.
- `torii_sorafs_replication_backlog_total > 10` في التقنية 10 دقائق (سم.
  `pin-registry-ops.md`).
- تم إنشاء الحوكمة من الاسم المستعار المتكامل بعد الإصدار.

### الفرز

1. التحقق من لوحات المعلومات [`sorafs/pin-registry-ops`](../sorafs/pin-registry-ops)، لذلك
   تأكد من ترجمة الأعمال المتراكمة إلى فئة التخزين أو إلى مفوضي الأسطول.
2. قم بإعادة التحقق من السجل Torii إلى `sorafs_registry::submit_manifest` للإصلاح،
   لا تدعم التقديمات.
3. التحقق جيدًا من النسخة المتماثلة من خلال `sorafs_cli manifest status --manifest ...`
   (عرض النسخ المتماثلة للفحص).

### التخفيف

1. قم بإعادة إرسال البيان باستخدام النسخة المتماثلة الأعلى (`--pin-min-replicas 7`) من خلال
   `scripts/sorafs-pin-release.sh`، لجدولة المزيد من المهام
   مقدم. قم بتزويد الملخص الجديد بسجل الحوادث.
2. إذا تمت إحالة الأعمال المتراكمة إلى موفر واحد، فقم دائمًا بإلغاء قفلها من خلال جدولة النسخ المتماثل
   (موضح في `pin-registry-ops.md`) وكشف البيان الجديد، الأصلي الأصلي
   محقق الحصول على الاسم المستعار.
3. عندما يتم نشر الاسم المستعار لنسخ التكافؤ المهمة، يتم إعادة ربط الاسم المستعار للبيان الدافئ في التدريج
   (`docs-preview`)، ثم قم بنشر بيان المتابعة بعد التراكم SRE.

### الاسترداد والإغلاق

1. قم بمراقبة `torii_sorafs_replication_sla_total{outcome="missed"}` واكتشف ما هو
   استقرت سكيتشيك.
2. قم بحفظ البيانات `sorafs_cli manifest status` كدليل على أن كل نسخة طبق الأصل جديدة في العادة.
3. قم بالبدء أو اكتشاف تراكم النسخ المتماثل بعد الوفاة باستخدام الخطوات التالية
   (مزودو الأجهزة، ومقطع الضبط، وما إلى ذلك).

## رانبوك - إلغاء التحليلات أو أجهزة القياس عن بعد

### سهولة التصنيع

- `npm run probe:portal` يأتي، ولكن لوحات المعلومات لا تسمح ببدء الاشتراك
  `AnalyticsTracker` أكثر من 15 دقيقة.
- مراجعة الخصوصية fixiruet неозиданный рост أسقطت الأحداث.
- `npm run probe:tryit-proxy` يكمل الرحلات `/probe/analytics`.

### الرد

1. التحقق من مدخلات وقت البناء: `DOCS_ANALYTICS_ENDPOINT` و
   `DOCS_ANALYTICS_SAMPLE_RATE` в إصدار قطعة أثرية (`build/release.json`).
2. قم بالتحويل من `npm run probe:portal` إلى `DOCS_ANALYTICS_ENDPOINT`، بشكل صحيح
   في مجمع التدريج، للتأكد من أن المتعقب يزود قائمة الحمولات.
3. إذا لم يكن المجمعون قادرين على التثبيت، قم بتثبيت `DOCS_ANALYTICS_ENDPOINT=""` وإعادة البناء،
   чтобы تعقب ماس كهربائى. قم بتأكيد الانقطاع التام في الجدول الزمني للحادث.
4. تحقق من أن `scripts/check-links.mjs` يستمر في بصمة الإصبع `checksums.sha256`
   (التحليلات التحليلية *لا* تحتاج إلى حظر التحقق من خريطة الموقع).
5. بعد إعادة تركيب المجمع، قم بإغلاق `npm run test:widgets` للتخطيط
   مساعد تحليلات اختبارات الوحدة قبل إعادة النشر.

### النتيجة النهائية

1. قم بتحديث [`devportal/observability`](./observability) بالإضافات الجديدة
   جامع أو أخذ العينات.
2. استخدام إشعار الحوكمة، في حالة تعرض هذه التحليلات للانتهاك أو الإلغاء
   ليست سياسة.

## تعليم رائع للجودة

ابدأ التدريب في **الحديقة الأولى في الحجرة** (يناير/أبريل/يوليو/أكتوبر)
أو بعد فترة وجيزة من البنية التحتية اللازمة لتنمية الموارد البشرية. القطع الأثرية الجرانيتية في
`artifacts/devportal/drills/<YYYYMMDD>/`.

| تعليم | شاجي | التصريح |
| ----- | ----- | -------- |
| تكرار الاسم المستعار التراجع | 1. العودة إلى الحالة السابقة "فشل النشر" مع بيان الإنتاج المصاحب.<br/>2. إعادة ربط الإنتاج بعد تحقيقات ناجحة.<br/>3. حفظ `portal.manifest.submit.summary.json` وتسجيل المجسات في حزمة الحفر. | `rollback.submit.json`، تحقيقات الإدخال وتكرار علامة الإصدار. |
| تدقيق الحسابات والتحقق من الصحة | 1. قم بتثبيت `npm run probe:portal` و`npm run probe:tryit-proxy` للإنتاج والتدريج.<br/>2. Запустить `npm run check:links` و арkhивировать `build/link-report.json`.<br/>3. Прилозить لقطات/الصادرات панелей Grafana, подтверодающий успех تحقيقات. | مجسات الشعار + `link-report.json` مع بيان بصمات الأصابع. |

قم بترقية التدريبات المقترحة من خلال Docs/DevRel ومراجعة الحوكمة SRE،
كما تحتاج خارطة الطريق إلى تحديد هوية التراجع عن الاسم المستعار
وتختفي تحقيقات البوابة.

## PagerDuty والتنسيق عند الطلب

- خدمة PagerDuty **Docs Portal Publishing** إرسال التنبيهات من هناك
  `dashboards/grafana/docs_portal.json`. الصحيح `DocsPortal/GatewayRefusals`,
  يقوم `DocsPortal/AliasCache` و`DocsPortal/TLSExpiry` بتحميل المستندات الأساسية/DevRel
  مع تخزين SRE كثانوي.
- الرجاء الضغط على `DOCS_RELEASE_TAG`، قم بتنزيل لقطات الشاشة
  لوحة Grafana وقم بإدخال المسبار/التحقق من الارتباط في حدث متزامن
  بداية التخفيف.
- التخفيف التالي (التراجع أو إعادة النشر) إعادة الإغلاق `npm run probe:portal`,
  `npm run check:links` واللقطات المضمنة Grafana، تظهر مقياس النبض
  في سبيل ذلك. استخدم جميع الأدلة المتعلقة بحادثة PagerDuty للحماية.
- إذا تم إجراء تنبيهين بشكل فردي (على سبيل المثال انتهاء صلاحية TLS والتراكم)، ابدأ
  رفض الفرز (إيقاف النشر)، التراجع عن الحالة السابقة، إغلاق TLS/backlog
  مع تخزين SRE على الجسر.