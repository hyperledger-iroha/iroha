---
lang: ar
direction: rtl
source: docs/portal/docs/devportal/incident-runbooks.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# دفاتر الأحداث وتدريبات التراجع

## اقتراح

عنصر خريطة الطريق **DOCS-9** يوفر أدلة تشغيل قابلة للتنفيذ بالإضافة إلى خطة عملية لذلك
يمكن لمشغلي البوابة استرداد الأخطاء التي أدخلتها دون قصد. هذه ملاحظة
ثلاثة حوادث مكعبة عالية الجودة: التخلص من الأخطاء وتدهور النسخ و
سلسلة من التحليلات، وتوثيق التدريبات الثلاثية التي يمكن التراجع عنها
الاسم المستعار y la validacion sintetica siguen funcionando end to end.

### مادة ذات صلة

- [`devportal/deploy-guide`](./deploy-guide) - تدفق التعبئة والتغليف والتوقيع والترويج للاسم المستعار.
- [`devportal/observability`](./observability) - علامات الإصدار والتحليلات والمسبارات المرجعية.
-`docs/source/sorafs_node_client_protocol.md`
  ص [`sorafs/pin-registry-ops`](../sorafs/pin-registry-ops)
  - قياس السجل ومظلات التصعيد عن بعد.
- `docs/portal/scripts/sorafs-pin-release.sh` والمساعدين `npm run probe:*`
  المراجع وقوائم المراجعة.

### القياس عن بعد وأجزاء الأدوات

| سينال / أداة | اقتراح |
| ------------- | ------- |
| `torii_sorafs_replication_sla_total` (تم اللقاء/الفائت/معلق) | اكتشاف حجب النسخ المتماثل ومقاطع SLA. |
| `torii_sorafs_replication_backlog_total`، `torii_sorafs_replication_completion_latency_epochs` | قم بقياس عمق العمل المتراكم ووقت الانتظار الكامل للفرز. |
| `torii_sorafs_gateway_refusals_total`، `torii_sorafs_manifest_submit_total{status="error"}` | أخطاء البوابة التي ستتبعها عملية نشر معيبة. |
| `npm run probe:portal` / `npm run probe:tryit-proxy` | Probes sinteticos التي تطلق الإصدارات وعمليات التراجع الصحيحة. |
| `npm run check:links` | بوابة دي enlaces روتوس. إذا تم استخدامه بعد كل تخفيف. |
| `sorafs_cli manifest submit ... --alias-*` (يُستخدم بواسطة `scripts/sorafs-pin-release.sh`) | آلية الترويج/إرجاع الاسم المستعار. |
| لوحة `Docs Portal Publishing` Grafana (`dashboards/grafana/docs_portal.json`) | جمع القياس عن بعد للرفض/الاسم المستعار/TLS/النسخ المتماثل. تنبيهات PagerDuty هي لوحات مرجعية مثل الأدلة. |

## Runbook - خطأ في العمل أو خطأ في الصنع

### شروط الإلغاء

- تشغيل مجسات المعاينة/الإنتاج (`npm run probe:portal -- --expect-release=...`).
- التنبيهات Grafana و`torii_sorafs_gateway_refusals_total` o
  `torii_sorafs_manifest_submit_total{status="error"}` بعد الطرح.
- يكشف دليل ضمان الجودة عن الدوران أو الفشل في الوكيل، جربه على الفور بعد الانتهاء من ذلك
  الترويج للاسم المستعار.

###التنافس الوسيط

1. **التفاصيل الموضحة:** ماركا خط الأنابيب CI مع `DEPLOY_FREEZE=1` (إدخال سير العمل
   GitHub) أو إيقاف وظيفة Jenkins مؤقتًا حتى لا يتم عرض المزيد من المصنوعات اليدوية.
2. **التقاط القطع الأثرية:** تنزيل `build/checksums.sha256`،
   `portal.manifest*.{json,to,bundle,sig}`، وخرجت مجسات البناء التي فشلت لذلك
   يشير التراجع إلى الملخصات الدقيقة.
3. **إخطار أصحاب المصلحة:** تخزين SRE، وقائد Docs/DevRel، والمسؤول عن الحراسة
   الوعي العام (خاصة عندما يكون `docs.sora` مؤثرًا).

### إجراء التراجع

1. تحديد البيان النهائي الجيد (LKG). سير العمل في إنتاج الحراسة
   باجو `artifacts/devportal/<release>/sorafs/portal.manifest.to`.
2. إعادة تسمية الاسم المستعار بهذا البيان باستخدام مساعد البيئة:

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

3. قم بتسجيل استئناف التراجع في تذكرة الحادث جنبًا إلى جنب مع ملخصات ديل
   بيان LKG وفشل البيان.

### التحقق من الصحة

1.`npm run probe:portal -- --expect-release=${LKG_TAG}`.
2.`npm run check:links`.
3. `sorafs_cli manifest verify-signature ...` و `sorafs_cli proof verify ...`
   (الاطلاع على دليل النشر) لتأكيد استمرار إعادة الترويج للبيان
   بالتزامن مع أرشيف CAR.
4.`npm run probe:tryit-proxy` لضمان عودة الوكيل إلى Try-It.

### ما بعد الحادث

1. إعادة تأهيل خط الأنابيب الذي يتدفق فقط بعد إدراك السبب الرئيسي.
2. قم بإدخال "الدروس المستفادة" في [`devportal/deploy-guide`](./deploy-guide)
   مع ملاحظات جديدة، إذا تم تطبيقها.
3. عيوب مجموعة اختبارات السقوط (المسبار، مدقق الارتباط، وما إلى ذلك).

## Runbook - تدهور النسخ المتماثل

### شروط الإلغاء

- تنبيه: `sum(torii_sorafs_replication_sla_total{outcome="met"}) /
  المشبك_مين(sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}), 1) <
  0.95` لمدة 10 دقائق.
- `torii_sorafs_replication_backlog_total > 10` لمدة 10 دقائق (الإصدار
  `pin-registry-ops.md`).
- تقرير Gobernanza متاح تحت الاسم المستعار بعد إطلاق سراحه.

### الفرز

1. فحص لوحات المعلومات [`sorafs/pin-registry-ops`](../sorafs/pin-registry-ops) للفقرة
   تأكد مما إذا كان التراكم مترجمًا في فئة تخزين أو في أسطول من مقدمي الخدمة.
2. سجلات الرحلات البحرية Torii للتحذيرات `sorafs_registry::submit_manifest` لتحديد ما إذا كان
   التقديمات لاس Estan Fallando.
3. احصل على النسخ المتماثلة الصحية عبر `sorafs_cli manifest status --manifest ...` (قائمة النتائج
   النسخ المتماثل من قبل الموفر).

### التخفيف

1. أعد إرسال البيان بأكبر عدد من النسخ المتماثلة (`--pin-min-replicas 7`) باستخدام
   `scripts/sorafs-pin-release.sh` لكي يقوم المجدول بتوزيع الشحنات في مجموعة واحدة
   دي مقدمي الخدمات. قم بتسجيل الملخص الجديد في سجل الأحداث.
2. إذا كانت الأعمال المتراكمة مُزودة بمزود فريد، فسيتم إلغاء تأهيلها مؤقتًا عبر
   جدولة النسخ المتماثل (موثقة في `pin-registry-ops.md`) وإرسال نسخة جديدة
   يقوم البيان بإرسال موفري الخدمة الآخرين إلى تحديث الاسم المستعار.
3. عندما تكون صورة الاسم المستعار أكثر أهمية من أن تقوم لوحة النسخ بإعادة لصقها
   الاسم المستعار لبيان مثير للاهتمام تم تنظيمه (`docs-preview`)، وقم بنشر بيان
   اتبع مرة واحدة حيث يقوم SRE بإنهاء التراكم.

### التعافي والشفاء

1. قم بمراقبة `torii_sorafs_replication_sla_total{outcome="missed"}` لضمان ذلك
   يستقر الحساب.
2. التقط `sorafs_cli manifest status` الناتج كدليل على أن كل نسخة طبق الأصل موجودة
   دي نيوفو إن كومبليمينتو.
3. اكتشاف تراكم النسخ المتماثل بعد انتهاء الخطوات التالية
   (تصعيد الموفرين، وضبط القطعة، وما إلى ذلك).

## Runbook - دليل التحليل والقياس عن بعد

### شروط الإلغاء

- `npm run probe:portal` لديه خروج من لوحات المعلومات بعد إدخال الأحداث
  `AnalyticsTracker` لمدة > 15 دقيقة.
- تكشف مراجعة الخصوصية عن زيادة غير متوقعة في الأحداث التي تم حذفها.
- `npm run probe:tryit-proxy` يقع في المسارات `/probe/analytics`.

### الرد

1. التحقق من مدخلات البناء: `DOCS_ANALYTICS_ENDPOINT` ذ
   `DOCS_ANALYTICS_SAMPLE_RATE` هو الإصدار الاصطناعي (`build/release.json`).
2. أعد تشغيل `npm run probe:portal` مع `DOCS_ANALYTICS_ENDPOINT` مرة أخرى
   أداة تجميع التدريج لتأكيد أن المتتبع سيصدر الحمولات الصافية.
3. إذا كان المجمعون موجودين، فقم بإعادة البناء `DOCS_ANALYTICS_ENDPOINT=""`
   لكي يتمكن جهاز التعقب من حدوث ماس كهربائى؛ قم بتسجيل نافذة الانقطاع
   خط زمن الحادث.
4. التحقق من `scripts/check-links.mjs` بصمات الأصابع `checksums.sha256`
   (مصادر التحليل *لا* يجب أن تمنع التحقق من صحة خريطة الموقع).
5. عند استعادة المجمع، أدخل `npm run test:widgets` لتشغيله
   وحدة اختبارات مساعد التحليل قبل إعادة النشر.

### ما بعد الحادث

1. تحديث [`devportal/observability`](./observability) مع قيود جديدة
   جامع متطلبات الموسيقى.
2. أرسل إشعارًا حكوميًا إذا فقدت أو قمت بمراجعة البيانات التحليلية المستقبلية
   دي بوليتيكا.

## تدريبات ثلاثية الأبعاد على المرونة

تنفيذ التدريبات الإضافية خلال **الرحلات التمهيدية لكل ثلاثة أشهر** (العصر/أبريل/يوليو/أكتوبر)
أو على الفور بعد أي تغيير كبير في البنية التحتية. حراسة القطع الأثرية باجو
`artifacts/devportal/drills/<YYYYMMDD>/`.

| حفر | باسوس | ايفيدنسيا |
| ----- | ----- | -------- |
| Ensayo de rollback de alias | 1. كرر التراجع عن "فشل الفشل" باستخدام بيان الإنتاج الأحدث.<br/>2. أعد الإنتاج مرة أخرى عندما يتم إجراء التحقيقات.<br/>3. المسجل `portal.manifest.submit.summary.json` وسجلات المسبار في سجادة المثقاب. | `rollback.submit.json`، أخرج المسبار، وحرر علامة الإزالة. |
| Auditori de validacion sintetica | 1. قم بتشغيل `npm run probe:portal` و `npm run probe:tryit-proxy` لمنع الإنتاج والتجهيز.<br/>2. قم بتشغيل `npm run check:links` وأرشفة `build/link-report.json`.<br/>3. إضافة لقطات شاشة/تصدير اللوحات Grafana لتأكيد خروج المسبار. | سجلات التحقيق + `link-report.json` تشير إلى بصمة الإصبع. |

قم بترقية التدريبات المفقودة إلى مدير Docs/DevRel ومراجعة إدارة SRE،
ما هي خارطة الطريق التي تتطلب أدلة ثلاثية تحدد ما إذا كان التراجع عن الاسم المستعار والوسائط
تحقيقات البوابة siguen saludables.

## Coordinacion de PagerDuty وعند الطلب

- خدمة PagerDuty **Docs Portal Publishing** هي نتيجة للتنبيهات التي يتم إنشاؤها من هذا المكان
  `dashboards/grafana/docs_portal.json`. لاس ريجلاس `DocsPortal/GatewayRefusals`,
  `DocsPortal/AliasCache`، و`DocsPortal/TLSExpiry` يتم طباعته على Docs/DevRel الأساسي
  مع تخزين SRE كثاني.
- عند فتح الصفحة، تتضمن `DOCS_RELEASE_TAG`، إضافة لقطات شاشة للوحات Grafana
  تم التأثير عليه وحذف نتيجة التحقيق/التحقق من الارتباط وملاحظات الحادث قبل حدوثه
  التخفيف الأولي.
- بعد التخفيف (التراجع أو إعادة النشر)، أعد تشغيل `npm run probe:portal`،
  `npm run check:links`، والتقاط لقطات Grafana اللوحات الجدارية تظهر المقاييس
  من جديد داخل المظلات. أضف كل الأدلة إلى حادثة PagerDuty
  قبل الحل.
- إذا تم إلغاء التنبيهات بنفس الوقت (على سبيل المثال، انتهاء صلاحية TLS mas backlog)، الفرز
  الرفض الأول (النشر المانع)، تنفيذ إجراء التراجع، مباشرة
  ليمبيا عناصر TLS/backlog مع تخزين SRE على الجسر.