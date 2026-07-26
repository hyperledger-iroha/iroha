---
lang: ar
direction: rtl
source: docs/portal/docs/soranet/testnet-rollout.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: testnet-rollout
العنوان: شبكة اختبار الاتصال في SoraNet (SNNet-10)
Sidebar_label: شبكة اختبار الاتصال (SNNet-10)
الوصف: خطة تفعيل مرحلية، عدة الانضمام، وبوابات القياس عن بعد لترقيات testnet في SoraNet.
---

:::ملاحظة المصدر القياسي
احترام هذه الصفحة خطة الشراكة SNNet-10 في `docs/source/soranet/testnet_rollout_plan.md`. حافظ على النسختين متطابقتين حتى يتم تقاعد الوثائق القديمة.
:::

SNNet-10 ينسق التفعيل المرحلي لطبقة اخفاء الهوية في SoraNet عبر الشبكة. استخدم هذا البناء البناء لبند خارطة الطريق إلى التسليمات الملموسة والكتب التشغيلية وبوابات القياس عن بعد كي يفهم كل المشغل المتوقع قبل ان تصبح SoraNet النقل الافتراضي.

## مراحل الاطلاق| المرحلة | الجدول المستهدف (المستهدف) | النطاق | القطع المطلوبة |
|-------|-------------------|-------|--------------------|
| **T0 - تيست نت مغلق** | الربع الرابع 2026 | 20-50 مرحلات عبر >=3 ASNs تديرها مساهمًا أساسيًا. | مجموعة اختبار Testnet onboarding، مجموعة دخان لتثبيت الحماية، خط الأساس للكمون + مقاييس إثبات العمل، وتسبب حفر براونوت. |
| **T1 - بيتا عامة** | الربع الأول 2027 | >=100 مرحلات، تفعيل دوران الحراسة، فرض رابطة الخروج، وSDK betas تعتمد SoraNet افتراضيا مع `anon-guard-pq`. | Onboarding kit محدث، قائمة مرجعية مكونة من الجميع، دليل SOP، حزمة لوحات المعلومات للقياس عن بعد، وتقارير بروفة للوادث. |
| **T2 - الشبكة الافتراضية** | الربع الثاني 2027 (مشروط باكتمال SNNet-6/7/9) | شبكة الإنتاج تعتمد SoraNet افتراضيا؛ تفعيل عمليات النقل من نوع obfs/MASQUE وفرض سقاطة PQ. | محاضر الموافقة على الحوكمة، إجراءات التراجع لنمط مباشر فقط، انذارات الرجوع إلى إصدار سابق، وتقرير نجاح الموقع. |

لا يوجد **مسار تجاوز** - يجب ان تشحن كل مرحلة القياس عن بعد والحوكمة من المرحلة السابقة قبل الترقية.

## المجموعة الانضمام للـ testnet

يحتوي كل مشغل التتابع على حزمة حتمية على الملفات التالية:| القطعة | الوصف |
|----------|------------|
| `01-readme.md` | نظرة عامة، تواصل، وجدول المنظمة. |
| `02-checklist.md` | قائمة التحقق قبل الإطلاق (الأجهزة، الوصول للشبكة، التحقق من سياسة الحراسة). |
| `03-config-example.toml` | تهيئة التتابع + الأوركسترا دنيا لـ SoraNet متطابقة مع كتل الامتثال في SNNet-9، ثلاثية كتلة `guard_directory` تؤكد التجزئة لاحدث لقطة حارس. |
| `04-telemetry.md` | تعليمات ربط لوحات المعلومات لمقاييس الخصوصية في SoraNet وحدود التنبيه. |
| `05-incident-playbook.md` | تحديد حالات التوقف/التخفيض مع مصفوفة التصحيح. |
| `06-verification-report.md` | قالب يكمله التشغيل ويعيدونه بعد نجاح اختبارات الدخان. |

توجد نسخة بالألوان في `docs/examples/soranet_testnet_operator_kit/`. كل تحدث ترقية عدة؛ ارقام الاصدار لاحقة (مثلا `testnet-kit-vT0.1`).

بالنسبة لمشغلي بيتا العامة (T1)، مختصر مختصر في `docs/source/soranet/snnet10_beta_onboarding.md` يلخص متطلبات ومخرجات القياس عن بعد وسير عمل تورنتو مع الاشارة الى عدة حتمي وhelpers التحقق.

`cargo xtask soranet-testnet-feed` ينشئ تغذية JSON يجمع نافذة الترقية وقائمة المرحلات وتقرير المقاييس وبديل التدريبات والتجزئة للمرافقات المشار إليها في قالب Stage-gate. قم بتسجيل سجلات التدريبات والمرفقات اولا باستخدام `cargo xtask soranet-testnet-drill-bundle` لتسجيل الخلاصة ان `drill_log.signed = true`.

## معايير النجاح

الترقية بين المراحل المشروطة بالـ القياس عن بعد التالية، والتي أكثر من أجل لأسبوعين:- `soranet_privacy_circuit_events_total`: 95% من الدوائر مكتملة بدون Brownout أو downgrade؛ الـ 5% المتبقية مقيدة بامدادات PQ.
- `sorafs_orchestrator_policy_events_total{outcome="brownout"}`: أقل من 1% من الجلسات التي يتم إحضارها يوميًا إلى التدريبات التي يتم تشغيلها خارج المنزل.
- `soranet_privacy_gar_reports_total`: تم تعديله ضمن +/-10% من مزيج GAR المتوقع؛ يجب تفسير الاتجاهات بشكل واضح.
- نجاح نجاح تذاكر PoW: >=99% ضمن نافذة 3 ثواني؛ يتم الابلاغ عبر `soranet_privacy_throttles_total{scope="congestion"}`.
- الكمون (المئوي 95) لكل منطقة: <200 مللي ثانية بعد الدوائر القانونية بالكامل، ملتقط عبر `soranet_privacy_rtt_millis{percentile="p95"}`.

لوحات المعلومات والتنبيهات موجودة في `dashboard_templates/` قوالب و `alert_templates/`; نسخها الى مستودع القياس عن بعد واضفها الى فحوصات الوبر في CI. استخدم `cargo xtask soranet-testnet-metrics` للتقرير الموجه للتوجيه قبل طلب الترقية.

تقديمات بوابة المرحلة يجب ان تتبع `docs/source/soranet/snnet10_stage_gate_template.md`، والذي يربط نموذج Markdown الجاهز للنسخ في `docs/examples/soranet_testnet_stage_gate/stage_gate_report_template.md`.

## قائمة المراجعة

يجب على المشغلين التوقيع على ما يلي قبل كل مرحلة:

- ✅ موقع إعلان التتابع باستخدام مغلف القبول الحالي.
- ✅ اختبار دخان دوران الحرس (`tools/soranet-relay --check-rotation`) ناجح.
- ✅ `guard_directory` يشير الى احدث artefact من `GuardDirectorySnapshotV2` و `expected_directory_hash_hex` يطابق اللجنة (اسجل سجل Relay hash الذي تم التحقق منه).
- ✅ معايير PQratchet (`sorafs_orchestrator_pq_ratio`) الوجود فوق حدود الهدف للمرحلة المطلوبة.
- ✅ تهيئة التوافق الخاصة بـ GAR تطابق آخر العلامة (راجع كتالوج SNNet-9).
- ✅ محاكاة تخفيض التصنيف (هواة التجميع وتوقع التنبيه خلال 5 دقائق).
- ✅ تنفيذ تمرين PoW/DoS بمستويات موثقة.يوجد قالب معبأ مسبقاً ضمن مجموعة الانضمام. يرسلون التفاصيل الكاملة الى مكتب المساعدة في الحكم قبل استلام بيانات الانتاج.

## الحوكمة والتقارير

- **ضبط الغد:** طلبات الموافقة على طلب مجلس الإدارة المسجل في محاضرة المجلس ومرفقة بصفحة.
- **ملخص الحالة:** نشر تحديثات اسبوعية تلخص عدد التتابعات ونسبة PQ وحوادث brownout والعناصر المفتوحة (تخزن في `docs/source/status/soranet_testnet_digest.md` بعد بدء الوتيرة).
- **التراجعات:** توفر خطة التراجع عن الموقع الأساسية للشبكة للمرحلة السابقة خلال 30 دقيقة، بما في ذلك ابطال DNS/guard Cache وقوالب تواصل العملاء.

## اصول لدعم

- `cargo xtask soranet-testnet-kit [--out <dir>]` ينضم منشئ المجموعة إلى `xtask/templates/soranet_testnet/` الى الدليل الهدف (الافتراضي `docs/examples/soranet_testnet_operator_kit/`).
- `cargo xtask soranet-testnet-metrics --input <metrics.json> [--out <path|->]` تحدد مقاييس النجاح SNNet-10 تقرير pass/fail منظم صدر مناسب لمراجعات الحوكمة. هناك لقطة نموذجية في `docs/examples/soranet_testnet_metrics_sample.json`.
- القالب Grafana وAlertmanager موجود تحت `dashboard_templates/soranet_testnet_overview.json` و `alert_templates/soranet_testnet_rules.yml`; نسخها إلى مستودع القياس عن بعد أو ربطها بفحوصات الوبر في CI.
- قالب تواصل خفض مستوى الرسائل SDK/portal يوجد في `docs/source/soranet/templates/downgrade_communication_template.md`.
- يجب ان تستخدم ملخصات الحالة الاسبوعية `docs/source/status/soranet_testnet_weekly_digest.md` كنموذج قياسي.

يجب سحب الطلبات لتحديث هذه الصفحة مع أي تغييرات في التغييرات أو القياس عن بعد حتى تبقى خطة للاطلاق قياسيا.