---
lang: ar
direction: rtl
source: docs/portal/docs/devportal/incident-runbooks.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# مخالفات وتمارين التراجع

##المحاصيل

بند خريطة الطريق **DOCS-9** تتطلب طائرات اجرائية وخطوط تدريب حتى مهاجمو البوابة من
تأخر فشل النشر دون التخمين. تغطي هذه المزايا ثلاثة حوادث عالية الاشارة—فشل النشر،
طرأت النسخ، وانقطاع التحليلات—وتوثق الدورات ربع سنوية تثبت ان التراجع للاسم المستعار
والتحقق الاصطناعي ما جوهرا يعملان من النهاية إلى النهاية.

### مادة ذات صلة

- [`devportal/deploy-guide`](./deploy-guide) — التغليف والتوقيع وترقية الاسم المستعار.
- [`devportal/observability`](./observability) — علامات الإصدار والتحليلات والمسابير المذكورة ادناه.
-`docs/source/sorafs_node_client_protocol.md`
  و [`sorafs/pin-registry-ops`](../sorafs/pin-registry-ops)
  — سجل القياس عن بعد وحدود التصعيد.
- مساعدين `docs/portal/scripts/sorafs-pin-release.sh` و `npm run probe:*`
  الإشارة إليها عبر الاعتماد.

### القياس عن بعد والادوات المشتركة

| إشارة / أداة | اللحوم |
| ------------- | ------- |
| `torii_sorafs_replication_sla_total` (تم اللقاء/الفائت/معلق) | يكشف عن توقف و SLA. |
| `torii_sorafs_replication_backlog_total`، `torii_sorafs_replication_completion_latency_epochs` | يقيس عمق الأعمال المتراكمة وزمن الاكتمال لا لأغراض الفرز. |
| `torii_sorafs_gateway_refusals_total`، `torii_sorafs_manifest_submit_total{status="error"}` | يوضح إعطاء البوابة التي غالبا ما تتبع نشر السيئات. |
| `npm run probe:portal` / `npm run probe:tryit-proxy` | تحقيقات اصطناعية تقوم بعمل بوابة للإصدارات والتحقق من التراجع. |
| `npm run check:links` | بوابة الروابط المكسورة؛ تستخدم بعد كل التخفيف. |
| `sorafs_cli manifest submit ... --alias-*` (مغلفة عبر `scripts/sorafs-pin-release.sh`) | ترقية تلقائية/عادة الاسم المستعار. |
| لوحة `Docs Portal Publishing` Grafana (`dashboards/grafana/docs_portal.json`) | عدد كبير من عمليات رفض القياس عن بعد/الاسم المستعار/TLS/النسخ المتماثل. تنبيهات PagerDuty تشير الى هذه الباتز كدليل. |

## Runbook - فشل نشر او artefact سيئ

###شروط الاطلاق

- فشل مجسات المعاينة/الإنتاج (`npm run probe:portal -- --expect-release=...`).
- تنبيهات Grafana على `torii_sorafs_gateway_refusals_total` او
  `torii_sorafs_manifest_submit_total{status="error"}` بعد الطرح.
- ضمان الجودة لعدم وجود مشاريع مكسورة او فشل الوكيل جربه مباشرة بعد الاسم المستعار.

### احتواء فوري

1. **تجميد النشر:** ضع `DEPLOY_FREEZE=1` على خط الأنابيب في CI (input forworkflow في GitHub)
   او اوقف وظيفة جينكينز حتى لا يتخرج من المصنوعات اليدوية.
2. **التقاط التحف:** حمل `build/checksums.sha256`,
   `portal.manifest*.{json,to,bundle,sig}`, ومخرجات تحقيقات من بناء الفاشل حتى يشير التراجع
   الى هضم الدقيق.
3. **اخطار اصحاب الناقلات:** Storage SRE وlead لـ Docs/DevRel وضابط ال تور المناوب
   (خصوصا عند تاثير `docs.sora`).

###إجراء التراجع

1. تعريف المانيفست الجديد المعروف انه جيد (LKG). يقوم بسير العمل الإنتاجي بتخزينه في
   `artifacts/devportal/<release>/sorafs/portal.manifest.to`.
2. إعادة ربط الشحنة باسم واضح من خلال المساعد:

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

3. سجل ملخص التراجع في تذكرة الحادثة مع الملخصات الخاصة ببيان LKG وبيان الفاشل.

### التحقق

1.`npm run probe:portal -- --expect-release=${LKG_TAG}`.
2.`npm run check:links`.
3. `sorafs_cli manifest verify-signature ...` و `sorafs_cli proof verify ...`
   (انظر دليل النشر) لتاكيد ان بيان المعاد ترقيته ما يصل إلى طابق CAR المؤرشف.
4.`npm run probe:tryit-proxy` ودائما ان proxy Try-It في التدريج عادة للعمل.

### ما بعد الحادث

1. إعادة تفعيل خط الأنابيب فقط بعد فهم السبب الجذري.
2. تحديث قسم "الدروس المستفادة" في [`devportal/deploy-guide`](./deploy-guide)
   ملاحظات جديدة عند الحاجة.
3. مفك عيوب غير مؤكد (مسبار، مدقق الارتباط، الخ).

## Runbook - المستوردة النسخ

###شروط الاطلاق

- تنبيه: `sum(torii_sorafs_replication_sla_total{outcome="met"}) /
  المشبك_مين(sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}), 1) <
  0.95` لمدة 10 دقائق.
- `torii_sorafs_replication_backlog_total > 10` لمدة 10 دقائق (انظر
  `pin-registry-ops.md`).
- ولانبت عن بِتء وجود الاسم المستعار بعد الإصدار.

### الفرز

1. إعادة لوحات العدادات [`sorafs/pin-registry-ops`](../sorafs/pin-registry-ops) لما كان اذا كان
   backlog محصورة في فئة تخزين او اسطول مقدمي الخدمة.
2. تحقق من السجلات Torii بحثت عن `sorafs_registry::submit_manifest` لمعرفة ما إذا كانت
   تقديمات تفشل.
3. فحص صحة النسخ عبر `sorafs_cli manifest status --manifest ...` (يعرض النتائج لكل مزود).

### التخفيف

1. إعادة نسخ المانيفست القديمة لكبار السن اعلى (`--pin-min-replicas 7`) عبر
   `scripts/sorafs-pin-release.sh` ليوزع جدولة الحمل على أكبر عدد من مقدمي الخدمة.
   سجل الملخص الجديد في سجل الحادث.
2. اذا كان backlog مرتبط بموفر واحد، عطله مؤقتا عبر جدولة النسخ المتماثل
   (موثق في `pin-registry-ops.md`) القائمين على البيان جديد ولا يزال مقدمو الخدمة على تحديث الاسم المستعار.
3. عندما تكون حدثة مستعارة مهمة من التكافؤ النسخ، اعد ربط الاسم المستعار الى البيان البارد
   نظموا (`docs-preview`)، ثم نشر البيان متابعة بعد ان ينظف SRE الـ backlog.

### والاخير

1. راقب `torii_sorafs_replication_sla_total{outcome="missed"}` وثبات العد.
2. مولود مخرجات `sorafs_cli manifest status` كدليل على عودة كل نسخة طبق الأصل للانتقال.
3. أنشئ او حدث post-mortem لـ backlog النسخ مع الخطوات التالية
   (مقدمو التويسيع، ضبط تشونكر، الخ).

## Runbook - دينيس تحليلات او القياس عن بعد

###شروط الاطلاق

- `npm run probe:portal` ناجح لكن لوحات المعلومات توقفت عن ابتلاع احداث
  `AnalyticsTracker` لاكثر من 15 دقيقة.
- مراجعة الخصوصية لرصد الزيادة غير المتوقعة في الاحداث المسقطة.
- `npm run probe:tryit-proxy` يفشل في تجارب `/probe/analytics`.

### المصدر

1. تحقق من المدخلات وقت البناء: `DOCS_ANALYTICS_ENDPOINT` و
   `DOCS_ANALYTICS_SAMPLE_RATE` داخل قطعة أثرية الاصدار (`build/release.json`).
2. أعد التشغيل `npm run probe:portal` مع التوجيه `DOCS_ANALYTICS_ENDPOINT` إلى
   Collector في التدريج لتاكيد ان تعقب ما يرسل الحمولات.
3. اذا كانت جامعي متوقفة، اضبط `DOCS_ANALYTICS_ENDPOINT=""` واعمل إعادة بناء
   ليقوم جهاز التعقب بعمل دائرة قصر؛ سجل نافذة الانقطاع في الجدول الزمني للحادثة.
4. تحقق ان `scripts/check-links.mjs` ما يقوم بعمل البصمة لـ `checksums.sha256`
   (انقطاعات التحليلات يجب *الا* منع التحقق من خريطة الموقع).
5. بعد تعافي المجمع، الوظيفة `npm run test:widgets` تشغيل اختبارات الوحدة الخاصة بمساعد التحليلات
   قبل إعادة النشر.

### ما بعد الحادث

1. تحديث [`devportal/observability`](./observability) باي الكترونيات جديدة للجامع او
   متطلبات أخذ العينات.
2. صدر مختار إذا تم تحسين او تحسين بيانات التحليلات الخارجية للسياسة.

## تمارين سنوية

مهنة كلاين خلال **اول ثلاثاء من كل ربع** (يناير/أبريل/يوليو/أكتوبر)
او مباشرة بعد أي تغيير كبير في الأنسجة العصبية. مصنوعات خزن تحت
`artifacts/devportal/drills/<YYYYMMDD>/`.

| ممارسة | |الخطوات الدليل |
| ----- | ----- | -------- |
| تمرين التراجع عن الاسم المستعار | 1. إعادة تشغيل التراجع الخاص بـ "فشل النشر" باستخدام احدث البيان الانتاجي.<br/>2. إعادة الداخلية الى الإنتاج بعد نجاح المسابير.<br/>3. تسجيل `portal.manifest.submit.summary.json` يسجل تحقيقات في فريق العمل. | `rollback.submit.json`، مخرجات مجسات، وإصدار علامة للتمرين. |
| دقيق التحقق الاصطناعي | 1. تشغيل `npm run probe:portal` و `npm run probe:tryit-proxy` ضد تنظيم الإنتاج.<br/>2. تشغيل `npm run check:links` و ارشفة `build/link-report.json`.<br/>3. رفاق لقطات الشاشة/الصادرات من اللوحات Grafana لتكيد نجاح المسابير. | سجلات المسابير + `link-report.json` تشير الى بصمة الإصبع للبيان. |

صعّد التمارين الفائتة الى مدير Docs/DevRel ومراجعة إلى SRE، لان خريطة الطريق تتطلب
دليل ربع سنوي حطميًا على التراجع عن الأسماء المستعارة وتحقيقات البوابة ما زالت سليمة.

## تنسيق PagerDuty و on-call

- خدمة PagerDuty **Docs Portal Publishing** هل التنبيهات المولدة من
  `dashboards/grafana/docs_portal.json`. إلى `DocsPortal/GatewayRefusals`,
  `DocsPortal/AliasCache` و `DocsPortal/TLSExpiry` تشغيل الترحيل لـ Docs/DevRel
  الرئيسي مع التخزين SRE كاحتياطي.
- عند النداء، ارفق `DOCS_RELEASE_TAG`، وارفق لقطات الشاشة للوحات Grafana المتاثرة،
  وربط مخرجات المسبار/التحقق من الارتباط في الحادثة قبل بدء عملية التخفيف.
- بعد التخفيف (التراجع أو إعادة النشر)، أعد تشغيل `npm run probe:portal`،
  `npm run check:links`, والتقط لقطات جديدة من Grafana قياس عودة المقاييس
  ضمن العتبات. ارفق جميع الشريكة بحادثة PagerDuty قبل اغلاقها.
- اذا اطلق تنبيهان في نفس الوقت (مثلا انتهاء صلاحية TLS مع تراكم)، تعامل مع الرفض أولا
  (ايقاف النشر)، تنفيذ إجراء التراجع، ثم معالجة TLS/backlog مع تخزين SRE على الجسر.