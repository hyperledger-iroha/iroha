---
lang: ar
direction: rtl
source: docs/portal/docs/devportal/incident-runbooks.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# دفاتر الأحداث وتدريبات التراجع

## موضوعي

يتطلب عنصر خريطة الطريق **DOCS-9** قواعد اللعبة القابلة للتنفيذ بالإضافة إلى خطة التكرار التي تريدها
يمكن لمشغلي البوابة استرداد مستندات المكتبة دون الحاجة إلى معرفتها. ملاحظة هذه
حوادث couvre trois إشارة حصن - معدلات النشر، وتدهور النسخ المتماثل، وما إلى ذلك
لوحات التحليلات - وتوثيق التدريبات الثلاثية التي تثبت التراجع عن الأسماء المستعارة
ويعمل التحقق من الصحة الاصطناعية طوال الوقت.

### اتصال المواد

- [`devportal/deploy-guide`](./deploy-guide) - سير عمل التغليف والتوقيع والترويج للأسماء المستعارة.
- [`devportal/observability`](./observability) - علامات الإصدار والتحليلات والتحقيقات تشير إلى ci-dessous.
-`docs/source/sorafs_node_client_protocol.md`
  وآخرون [`sorafs/pin-registry-ops`](../sorafs/pin-registry-ops)
  - القياس عن بعد للسجل ودرجات التسلق.
- `docs/portal/scripts/sorafs-pin-release.sh` والمساعدين `npm run probe:*`
  المراجع في قوائم المراجعة.

### أجزاء القياس عن بعد والأدوات

| إشارة / مخرج | موضوعي |
| ------------- | ------- |
| `torii_sorafs_replication_sla_total` (تم اللقاء/الفائت/معلق) | اكتشف عوائق النسخ وخروقات SLA. |
| `torii_sorafs_replication_backlog_total`، `torii_sorafs_replication_completion_latency_epochs` | قم بقياس عمق الأعمال المتراكمة وتأخر الانتهاء من عملية الفرز. |
| `torii_sorafs_gateway_refusals_total`، `torii_sorafs_manifest_submit_total{status="error"}` | Montre les echecs cote gate التي تتبع معدل نشر. |
| `npm run probe:portal` / `npm run probe:tryit-proxy` | تحقيقات تركيبية تقوم ببوابة الإصدارات وصحيحة عمليات التراجع. |
| `npm run check:links` | حالات بوابة الامتيازات؛ الاستفادة من التخفيف apres chaque. |
| `sorafs_cli manifest submit ... --alias-*` (ملفوف على قدم المساواة `scripts/sorafs-pin-release.sh`) | آلية الترويج/إعادة الأسماء المستعارة. |
| لوحة `Docs Portal Publishing` Grafana (`dashboards/grafana/docs_portal.json`) | قم بالموافقة على رفض القياس عن بعد/الاسم المستعار/TLS/النسخ المتماثل. تشير تنبيهات PagerDuty إلى هذه اللوحات بمثابة تنبيهات. |

## Runbook - معدل النشر أو العيوب الفنية

### شروط الانحراف

- صدى المسابير للمعاينة/الإنتاج (`npm run probe:portal -- --expect-release=...`).
- تنبيهات Grafana على `torii_sorafs_gateway_refusals_total` ou
  `torii_sorafs_manifest_submit_total{status="error"}` بعد الطرح.
- سؤال وجواب ملاحظة يدوية عن مسارات الكاسيت أو لوحات الوكيل جربها فورًا بعد ذلك
  ترقية الاسم المستعار.

### الحبس الفوري

1. **إنشاء عمليات النشر:** تحديد خط الأنابيب CI مع `DEPLOY_FREEZE=1` (إدخال سير العمل
   GitHub) أو قم بإيقاف وظيفة Jenkins مؤقتًا حتى لا تكون قطعة أثرية جزءًا منها.
2. **Capture les artefacts:** تحميل `build/checksums.sha256`,
   `portal.manifest*.{json,to,bundle,sig}`، ويتم إجراء عملية إنشاء المسبار الإلكتروني لمعرفة ذلك
   يشير التراجع إلى التفاصيل الدقيقة.
3. **إخطار الأطراف السابقة:** تخزين SRE، وLead Docs/DevRel، ومسؤول الحراسة
   الحوكمة من أجل الوعي (surtout si `docs.sora` est Impacte).

### إجراء التراجع

1. معرف بيان آخر سلعة معروفة (LKG). سير العمل في إنتاج المخزونات الفرعية
   `artifacts/devportal/<release>/sorafs/portal.manifest.to`.
2. إعادة كتابة الاسم المستعار لبيان ce مع مساعد الشحن:

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

3. قم بتسجيل استئناف التراجع في تذكرة الحادث مع خلاصاتك
   مانيفست LKG ودو مانفيست أون إيشيك.

### التحقق من الصحة

1.`npm run probe:portal -- --expect-release=${LKG_TAG}`.
2.`npm run check:links`.
3. `sorafs_cli manifest verify-signature ...` و`sorafs_cli proof verify ...`
   (يعرض دليل النشر) لتأكيد توافق البيان المكرر
   toujours au CAR أرشيف.
4.`npm run probe:tryit-proxy` للتأكد من أن الوكيل Try-It التدريج يدر أرباحًا.

###ما بعد الحادث

1. قم بإعادة تنشيط خط أنابيب النشر الفريد بعد تجنبه والذي يشتمل على السبب الجذري.
2. قم بتشغيل مقبلات "الدروس المستفادة" في [`devportal/deploy-guide`](./deploy-guide)
   مع نقاط جديدة، إذا كنت بحاجة.
3. اكتشاف العيوب لمجموعة الاختبارات الإلكترونية (المسبار، مدقق الارتباط، وما إلى ذلك).

## Runbook - تدهور النسخ المتماثل

### شروط الانحراف

- تنبيه: `sum(torii_sorafs_replication_sla_total{outcome="met"}) /
  المشبك_مين(sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}), 1) <
  0.95` قلادة 10 دقائق.
- `torii_sorafs_replication_backlog_total > 10` قلادة 10 دقائق (صوت
  `pin-registry-ops.md`).
- تشير الحوكمة إلى توفر الأسماء المستعارة بعد الإصدار.

### الفرز

1. المفتش ليه لوحات المعلومات [`sorafs/pin-registry-ops`](../sorafs/pin-registry-ops) من أجل
   تأكد مما إذا كان التراكم قد تم تحديده على فئة المخزون أو أسطول مقدمي الخدمة.
2. قم بتدوير السجلات Torii للتحذيرات `sorafs_registry::submit_manifest`
   محدد ما إذا كانت عمليات الإرسال تتردد.
3. قم بتحديث صحة النسخ المتماثلة عبر `sorafs_cli manifest status --manifest ...`
   (قائمة النتائج حسب الموفر).

### التخفيف

1. قم بإعادة البيان باستخدام عدد من النسخ المتماثلة بالإضافة إلى أحد عشر (`--pin-min-replicas 7`) عبر
   `scripts/sorafs-pin-release.sh` يوضح كيفية جدولة الرسوم على أكثر من موفري الخدمة.
   قم بتسجيل ملخص جديد في سجل الحادث.
2. إذا كانت الأعمال المتراكمة عبارة عن موفر فريد من نوعه، فسيتم إلغاء التنشيط مؤقتًا عبر المجدول
   النسخ المتماثل (المستند في `pin-registry-ops.md`) وإظهار بيان جديد
   يمكنك اختيار مقدمي الخدمة الآخرين من خلال الاسم المستعار.
3. عندما يكون فقدان الاسم المستعار أكثر انتقادًا لمستوى النسخ المتماثل، يتم إعادة كتابته
   الاسم المستعار لمرحلة البيان الضخم (`docs-preview`)، ثم نشر بيان
   تابع مرة أخرى SRE لتنظيف الأعمال المتراكمة.

### الاستشفاء والتجلط

1. جهاز المراقبة `torii_sorafs_replication_sla_total{outcome="missed"}` لضمان ذلك
   يستقر compteur.
2. التقط اللقطة `sorafs_cli manifest status` كما هو متوقع في كل نسخة طبق الأصل
   العودة إلى المطابقة.
3. قم ببدء النسخ المتماثل المتراكم يوميًا بعد الانتهاء من عمليات النسخ المتماثل
   الأشرطة (موفري القياس، وضبط المقسم، وما إلى ذلك).

## Runbook - لوحة التحليلات أو القياس عن بعد

### شروط الانحراف

- `npm run probe:portal` إعادة استخدام لوحات المعلومات لإيقاف إدخال الأحداث
  قلادة `AnalyticsTracker` > 15 دقيقة.
- تكتشف مراجعة الخصوصية كثرة الأحداث المهجورة.
- `npm run probe:tryit-proxy` صدى المسارات `/probe/analytics`.

### الرد

1. تحقق من مدخلات البناء: `DOCS_ANALYTICS_ENDPOINT` et
   `DOCS_ANALYTICS_SAMPLE_RATE` في الإصدار الاصطناعي (`build/release.json`).
2. إعادة تنفيذ `npm run probe:portal` مع `DOCS_ANALYTICS_ENDPOINT` في الاتجاه المعاكس
   أداة تجميع التدريج لتأكيد أن المتعقب سيقوم باستعادة الحمولات الصافية.
3. إذا تم إيقاف المجمعات، فحدد `DOCS_ANALYTICS_ENDPOINT=""` وأعد البناء
   كيو لو تعقب دائرة المحكمة؛ أرسل نافذة الانقطاع إلى الجدول الزمني.
4. التحقق من `scripts/check-links.mjs` مواصلة بصمة الإصبع `checksums.sha256`
   (لا تحتاج أجزاء التحليلات إلى *pas* لحظر التحقق من صحة خريطة الموقع).
5. قم بإعادة تجميع أداة التجميع، Lancer `npm run test:widgets` لتنفيذ المهام
   وحدة اختبارات التحليلات المساعدة قبل إعادة النشر.

###ما بعد الحادث

1. العمل يوميًا [`devportal/observability`](./observability) مع الحدود الجديدة
   du Collector أو متطلبات أخذ العينات.
2. أرسل إشعارًا بالتحكم في البيانات التحليلية التي قد تتعرض للخسارة أو الحذف
   خارج السياسة.

## تدريبات ثلاثية المرونة

Lancer les deux Drills خلال الأشهر الثلاثة الأولى من شهر مارس ** (يناير/أبريل/يوليو/أكتوبر)
أو فورًا بعد التغيير الكبير في البنية التحتية. Stocker les artifacts sous
`artifacts/devportal/drills/<YYYYMMDD>/`.

| حفر | نصوص | بريف |
| ----- | ----- | -------- |
| تكرار التراجع عن الأسماء المستعارة | 1. أعد تشغيل "معدل النشر" مع بيان الإنتاج الأحدث.<br/>2. أعد الإنتاج مرة أخرى حتى تمر المسبار.<br/>3. قم بتسجيل `portal.manifest.submit.summary.json` وسجلات المسبار في ملف الحفر. | `rollback.submit.json`، قم بفرز المسبار، ثم قم بتحرير علامة التكرار. |
| تدقيق التحقق من الصحة الاصطناعية | 1. Lancer `npm run probe:portal` و`npm run probe:tryit-proxy` ضد الإنتاج والتجهيز.<br/>2. لانسر `npm run check:links` وأرشيفي `build/link-report.json`.<br/>3. انضم إلى لقطات الشاشة/صادرات اللوحات Grafana لتأكيد نجاح التحقيقات. | سجلات المسبار + `link-report.json` تشير إلى بصمة الإصبع. |

Escalader les Drills manques au manager Docs/DevRel et a la revue الحكم SRE، car le
تتطلب خارطة الطريق إجراء ثلاثة أقسام محددة لتحديد التراجع عن الأسماء المستعارة والمسبارات
بوابة بقية السينس.

## تنسيق PagerDuty وآخرون عند الطلب

- تحتوي خدمة PagerDuty **Docs Portal Publishing** على تنبيهات عامة من جديد
  `dashboards/grafana/docs_portal.json`. القواعد `DocsPortal/GatewayRefusals`,
  `DocsPortal/AliasCache` و`DocsPortal/TLSExpiry` الصفحة الأولى Docs/DevRel
  avec Storage SRE ثانوي.
- عند هذه الصفحة، قم بتضمين `DOCS_RELEASE_TAG`، وضم لقطات الشاشة للشاشات
  Grafana التأثيرات وتتبع عملية التحقيق/التحقق من الارتباط في ملاحظات الحادث المسبق
  لبدء التخفيف.
- تخفيف Apres (التراجع أو إعادة النشر)، إعادة التنفيذ `npm run probe:portal`،
  `npm run check:links`، والتقاط اللقطات Grafana يلتقط المقاييس
  الإيرادات في الأسواق. انضم إلى جميع الأدلة على حادثة PagerDuty
  القرار الطليعي.
- إذا كانت التنبيهات مخففة ومؤقتة (على سبيل المثال، انتهاء صلاحية TLS بالإضافة إلى تراكم)، حاول
  الرفض الأول (إيقاف النشر)، تنفيذ إجراء التراجع، ثم
  Trater TLS/backlog avec Storage SRE sur le Bridge.