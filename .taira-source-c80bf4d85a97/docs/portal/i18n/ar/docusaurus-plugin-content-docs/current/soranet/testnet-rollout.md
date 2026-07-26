---
id: testnet-rollout
lang: ar
direction: rtl
source: docs/portal/docs/soranet/testnet-rollout.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note المصدر القياسي
تعكس هذه الصفحة خطة اطلاق SNNet-10 في `docs/source/soranet/testnet_rollout_plan.md`. حافظ على النسختين متطابقتين حتى يتم تقاعد الوثائق القديمة.
:::

SNNet-10 ينسق التفعيل المرحلي لطبقة اخفاء الهوية في SoraNet عبر الشبكة. استخدم هذه الخطة لتحويل بند خارطة الطريق الى deliverables ملموسة وrunbooks وبوابات telemetry كي يفهم كل operator التوقعات قبل ان تصبح SoraNet النقل الافتراضي.

## مراحل الاطلاق

| المرحلة | الجدول الزمني (المستهدف) | النطاق | القطع المطلوبة |
|-------|-------------------|-------|--------------------|
| **T0 - Testnet مغلق** | Q4 2026 | 20-50 relays عبر >=3 ASNs يديرها مساهمون اساسيون. | Testnet onboarding kit، smoke suite لـ guard pinning، baseline للكمون + metrics PoW، وسجل brownout drill. |
| **T1 - بيتا عامة** | Q1 2027 | >=100 relays، تفعيل guard rotation، فرض exit bonding، وSDK betas تعتمد SoraNet افتراضيا مع `anon-guard-pq`. | Onboarding kit محدث، checklist للتحقق من المشغلين، SOP لنشر directory، حزمة dashboards للـ telemetry، وتقارير rehearsal للحوادث. |
| **T2 - Mainnet افتراضي** | Q2 2027 (مشروط باكتمال SNNet-6/7/9) | شبكة الانتاج تعتمد SoraNet افتراضيا؛ تفعيل transports من نوع obfs/MASQUE وفرض PQ ratchet. | محاضر موافقة governance، اجراء rollback لنمط direct-only، انذارات downgrade، وتقرير نجاح موقع. |

لا يوجد **مسار تجاوز** - يجب ان تشحن كل مرحلة telemetry وقطع governance من المرحلة السابقة قبل الترقية.

## Kit الانضمام للـ testnet

كل operator relay يتلقى حزمة حتمية تحتوي على الملفات التالية:

| القطعة | الوصف |
|----------|-------------|
| `01-readme.md` | نظرة عامة، نقاط تواصل، وجدول زمني. |
| `02-checklist.md` | checklist قبل الاطلاق (hardware، امكانية الوصول للشبكة، تحقق من guard policy). |
| `03-config-example.toml` | تهيئة relay + orchestrator دنيا لـ SoraNet متطابقة مع كتل compliance في SNNet-9، وتتضمن كتلة `guard_directory` تثبت hash لاحدث guard snapshot. |
| `04-telemetry.md` | تعليمات لربط dashboards لمقاييس الخصوصية في SoraNet وحدود التنبيه. |
| `05-incident-playbook.md` | اجراء الاستجابة لحالات brownout/downgrade مع مصفوفة تصعيد. |
| `06-verification-report.md` | قالب يكمله المشغلون ويعيدونه بعد نجاح smoke tests. |

توجد نسخة مرسومة في `docs/examples/soranet_testnet_operator_kit/`. كل ترقية تحدث kit؛ ارقام الاصدار تتبع المرحلة (مثلا `testnet-kit-vT0.1`).

بالنسبة لمشغلي beta العامة (T1)، brief مختصر في `docs/source/soranet/snnet10_beta_onboarding.md` يلخص المتطلبات ومخرجات telemetry وسير عمل التسليم مع الاشارة الى kit الحتمي وhelpers التحقق.

`cargo xtask soranet-testnet-feed` ينشئ feed JSON يجمع نافذة الترقية وقائمة relays وتقرير المقاييس وادلة drills وhashes للمرفقات المشار اليها في قالب stage-gate. وقع سجلات drills والمرفقات اولا باستخدام `cargo xtask soranet-testnet-drill-bundle` لكي يسجل feed ان `drill_log.signed = true`.

## مقاييس النجاح

الترقية بين المراحل مشروطة بالـ telemetry التالية، والتي تجمع لمدة لا تقل عن اسبوعين:

- `soranet_privacy_circuit_events_total`: 95% من circuits تكتمل بدون brownout او downgrade؛ الـ 5% المتبقية مقيدة بامدادات PQ.
- `sorafs_orchestrator_policy_events_total{outcome="brownout"}`: اقل من 1% من جلسات fetch يوميا تطلق brownout خارج drills المخطط لها.
- `soranet_privacy_gar_reports_total`: تذبذب ضمن +/-10% من مزيج فئات GAR المتوقع؛ الارتفاعات يجب تفسيرها بتحديثات policy معتمدة.
- معدل نجاح تذاكر PoW: >=99% ضمن نافذة 3 ثواني؛ يتم الابلاغ عبر `soranet_privacy_throttles_total{scope="congestion"}`.
- الكمون (95th percentile) لكل منطقة: <200 ms بعد اكتمال circuits بالكامل، ملتقط عبر `soranet_privacy_rtt_millis{percentile="p95"}`.

قوالب dashboards والتنبيهات موجودة في `dashboard_templates/` و `alert_templates/`; انسخها الى مستودع telemetry واضفها الى فحوصات lint في CI. استخدم `cargo xtask soranet-testnet-metrics` لتوليد التقرير الموجه للحوكمة قبل طلب الترقية.

تقديمات stage-gate يجب ان تتبع `docs/source/soranet/snnet10_stage_gate_template.md`، والذي يربط نموذج Markdown الجاهز للنسخ في `docs/examples/soranet_testnet_stage_gate/stage_gate_report_template.md`.

## Checklist للتحقق

يجب على المشغلين التوقيع على ما يلي قبل دخول كل مرحلة:

- ✅ Relay advert موقع باستخدام admission envelope الحالي.
- ✅ Guard rotation smoke test (`tools/soranet-relay --check-rotation`) ناجح.
- ✅ `guard_directory` يشير الى احدث artefact من `GuardDirectorySnapshotV2` و `expected_directory_hash_hex` يطابق digest اللجنة (اقلاع relay يسجل hash الذي تم التحقق منه).
- ✅ مقاييس PQ ratchet (`sorafs_orchestrator_pq_ratio`) تبقى فوق حدود الهدف للمرحلة المطلوبة.
- ✅ تهيئة compliance الخاصة بـ GAR تطابق اخر tag (راجع كتالوج SNNet-9).
- ✅ محاكاة انذار downgrade (تعطيل collectors وتوقع تنبيه خلال 5 دقائق).
- ✅ تنفيذ drill لـ PoW/DoS مع خطوات تخفيف موثقة.

يوجد قالب معبأ مسبقا ضمن kit الانضمام. يرسل المشغلون التقرير المكتمل الى مكتب مساعدة governance قبل استلام بيانات الانتاج.

## Governance والتقارير

- **ضبط التغيير:** الترقيات تتطلب موافقة Governance Council مسجلة في محاضر المجلس ومرفقة بصفحة الحالة.
- **ملخص الحالة:** نشر تحديثات اسبوعية تلخص عدد relays ونسبة PQ وحوادث brownout والعناصر المفتوحة (تخزن في `docs/source/status/soranet_testnet_digest.md` بعد بدء الوتيرة).
- **Rollbacks:** الحفاظ على خطة rollback موقعة تعيد الشبكة للمرحلة السابقة خلال 30 دقيقة، بما يشمل ابطال DNS/guard cache وقوالب تواصل العملاء.

## اصول داعمة

- `cargo xtask soranet-testnet-kit [--out <dir>]` ينشئ kit الانضمام من `xtask/templates/soranet_testnet/` الى الدليل الهدف (الافتراضي `docs/examples/soranet_testnet_operator_kit/`).
- `cargo xtask soranet-testnet-metrics --input <metrics.json> [--out <path|->]` يقيم مقاييس نجاح SNNet-10 ويصدر تقرير pass/fail منظم مناسب لمراجعات governance. توجد لقطة نموذجية في `docs/examples/soranet_testnet_metrics_sample.json`.
- قوالب Grafana وAlertmanager موجودة تحت `dashboard_templates/soranet_testnet_overview.json` و `alert_templates/soranet_testnet_rules.yml`; انسخها الى مستودع telemetry او اربطها بفحوصات lint في CI.
- قالب تواصل downgrade لرسائل SDK/portal يوجد في `docs/source/soranet/templates/downgrade_communication_template.md`.
- يجب ان تستخدم ملخصات الحالة الاسبوعية `docs/source/status/soranet_testnet_weekly_digest.md` كنموذج قياسي.

يجب على pull requests تحديث هذه الصفحة مع اي تغييرات في القطع او telemetry حتى يبقى plan للاطلاق قياسيا.
