---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/direct-mode-pack.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: حزمة الوضع المباشر
العنوان: حزمة الخروج من النظام SoraFS (SNNet-5a)
Sidebar_label: حزمة نظام ممتاز
الوصف: التكوين المطلوب والتحقق من السلامة والتغييرات عند العمل SoraFS في الوضع الأمثل Torii/QUIC في فترة ما قبل SNNet-5a.
---

:::note Канонический источник
:::

توفر محيطات SoraNet وسيلة نقل معززة لـ SoraFS، ولكن باستخدام بطاقات صغيرة **SNNet-5a** تحتاج إلى احتياطي منتظم، يمكن للمشغلين تحديد الوصول إلى المحتوى عن طريق إخفاء إخفاء الهوية. تقوم هذه الحزمة بإصلاح معلمات CLI/SDK، وتكوينات الملفات الشخصية، والتحقق من الخصوصية، وإعادة التحقق من قائمة التحقق، والحاجة إلى ذلك تعمل SoraFS في الوضع الأول Torii/QUIC بدون النقل الخاص.

يعتمد الإجراء الاحتياطي على التدريج وشبكة الإنتاج المنتظمة، حيث لا يوفر SNNet-5–SNNet-9 بواباتهما الخاصة. توجد قطع أثرية مصنوعة من مواد أساسية تحتوي على SoraFS التي قد يتم حظرها من قبل المشغلين أنظمة مجهولة ومسبقة للاستخدام.

## 1.Flag CLI وSDK- `sorafs_cli fetch --transport-policy=direct-only ...` يستبعد التخطيط ويستخدم بشكل أساسي وسائل النقل Torii/QUIC. تتضمن درجة CLI الصحيحة `direct-only` كميزة إضافية.
- يجب تثبيت SDK على `OrchestratorConfig::with_transport_policy(TransportPolicy::DirectOnly)` مرة واحدة عندما يعرض هذا الخيار "النظام الأمثل". الروابط الهندسية في `iroha::ClientOptions` و`iroha_android` تؤدي إلى التعداد.
- يمكن ربط البوابة (`sorafs_fetch`، Python-bindings) بإلغاء القفل المباشر فقط من خلال Norito JSON-helpers، لذلك تم توفير الأتمتة أيضًا أو تحديثها.

قم بإضافة علامة في دليل التشغيل للشركاء وضم الحواجز من خلال `iroha_config`، وليس من خلال التعديلات تكاثر.

## 2. بوابة الملفات الشخصية السياسية

استخدم Norito JSON للتحكم في تحديد إعدادات التكوين. ملف التعريف التمهيدي في `docs/examples/sorafs_direct_mode_policy.json` يرمز إلى:

- `transport_policy: "direct_only"` — إلغاء اشتراك مقدمي الخدمة الذين يُعلنون عن وسائل النقل فقط عبر SoraNet.
- `max_providers: 2` — يتم حذف بعض نقاط الاتصال Torii/QUIC. كن ملتزمًا بالامتثال الصارم للمتطلبات الإقليمية.
- `telemetry_region: "regulated-eu"` — يتم تحديد المقاييس التي تستخدمها اللوحات وعمليات التدقيق الاحتياطية بشكل مختلف.
- ميزانيات متحفظة (`retry_budget: 2`, `provider_failure_threshold: 3`) لذلك لا تحجب البوابة غير المناسبة.قم بتوصيل JSON عبر `sorafs_cli fetch --config` (الأتمتة) أو روابط SDK (`config_from_json`) إلى مشغل سياسة النشر. قم بشراء لوحة النتائج (`persist_path`) للخطوات التالية لمراجعي الحسابات.

تم وصف المعلمات الأساسية للبوابة الرئيسية في `docs/examples/sorafs_gateway_direct_mode.toml`. يتم إلغاء تسجيل الدخول `iroha app sorafs gateway direct-mode enable`، وإلغاء التحقق من المغلف/القبول، وإغلاق الإعدادات الافتراضية للحد الأقصى للمعدل، وإلغاء تنشيط اللوحة `direct_mode` من الخطة والهضم بياننا. قم بتضمين العنصر النائب لخطة الطرح الخاصة بك قبل الجزء الثابت في تكوينات إدارة النظام.

## 3. التحقق من الجودة

يتضمن النظام الأساسي لمستوى الحرارة التغطية كما في الأوركسترا، وكذلك في وحدات CLI:- `direct_only_policy_rejects_soranet_only_providers` يضمن أن `TransportPolicy::DirectOnly` هو الحل الأمثل عندما يقوم كل مرشح بالإعلان بنفس القدر SoraNet.[صناديق/sorafs_orchestrator/src/lib.rs:7238]
- `direct_only_policy_prefers_direct_transports_when_available` يضمن استخدام Torii/QUIC، عند توفرها، باستثناء بكرة SoraNet من сессии.[صناديق/sorafs_orchestrator/src/lib.rs:7285]
- `direct_mode_policy_example_is_valid` البارسيت `docs/examples/sorafs_direct_mode_policy.json`، لتوثيق التوثيق مع utилитами-helperamи.【crates/sorafs_orchestrator/src/lib.rs:7509】 【docs/examples/sorafs_direct_mode_policy.json:1】
- تم إغلاق `fetch_command_respects_direct_transports` `sorafs_cli fetch --transport-policy=direct-only` للحماية من بوابة Torii، التي تتيح اختبار الدخان للشبكة المنتظمة، وسائل النقل الثابتة.[crates/sorafs_car/tests/sorafs_cli.rs:2733]
- `scripts/sorafs_direct_mode_smoke.sh` يدعمه الأمر JSON-politico ولوحة النتائج لأتمتة الطرح.

قم بإنهاء مجموعة الاختبارات المركّزة قبل نشرها:

```bash
cargo test -p sorafs_orchestrator direct_only_policy
cargo test -p sorafs_car --features cli fetch_command_respects_direct_transports
```

إذا كانت مساحة العمل مفتوحة من أجل التحسين المنبع، فقم بإلغاء قفل الشبكة في `status.md` ثم قم بالبدء في المتابعة إخلاء المسؤولية.

## 4. دخان الدخان الآليأحد أدوات التحقق من واجهة سطر الأوامر (CLI) لا يؤدي إلى تراجع محدد للتغطية (على سبيل المثال، بوابة سياسية قصيرة أو بيانات غير ضرورية). يتم إدخال نص الدخان المتميز في `scripts/sorafs_direct_mode_smoke.sh` ويتوافق مع `sorafs_cli fetch` وهو أوركسترا سياسي يعمل بنظام محمي بلوحة النتائج وملخص سريع.

الاستخدام الأولي:

```bash
./scripts/sorafs_direct_mode_smoke.sh \
  --config docs/examples/sorafs_direct_mode_smoke.conf \
  --provider name=gw-regulated,provider-id=001122...,base-url=https://gw.example/direct/,stream-token=BASE64
```- يقوم البرنامج النصي بتشغيل أعلام CLI ومفتاح = ملفات تكوين القيمة (sm.`docs/examples/sorafs_direct_mode_smoke.conf`). قبل تقديم ملخص البيان وتسجيل الإعلان، يتم تقديم المزيد من المعلومات.
- `--policy` في الغراب `docs/examples/sorafs_direct_mode_policy.json`، ولكن يمكن أن يسبق أي أوركسترا JSON، المهندس المعماري `sorafs_orchestrator::bindings::config_to_json`. يبدأ CLI السياسة من خلال `--orchestrator-config=PATH`، ويتيح حرية الملاحة بدون أعلام قريبة.
- إذا كان `sorafs_cli` موجودًا في `PATH`، فسوف ينضم إليه من كريت `sorafs_orchestrator` (ملف تعريف الإصدار)، من أجل دخان الدخان تم التحقق من نظام إعادة التوزيع بشكل كامل.
- البيانات الخارجية:
  - الحمولة الصافية (`--output`, по умолчанию `artifacts/sorafs_direct_mode/payload.bin`).
  - جلب الملخص (`--summary`، من خلال الحمولة)، وقياس المسافة في المنطقة ومقدمي الخدمات الأساسيين للتوصيل.
  - لوحة النتائج المصورة، المحمية بسياسة JSON (على سبيل المثال، `fetch_state/direct_mode_scoreboard.json`). أرشفة نفسك مع ملخص في التذاكر المحذوفة.- بوابة اعتماد الأتمتة: تم تحديد النص البرمجي بعد الانتهاء من الجلب `cargo xtask sorafs-adoption-check`، باستخدام لوحة النتائج والملخص المضمن. الغراب الكبير الذي يطارد الغراب يتقدم بضربة قيادية ؛ قم بإعادة تقديم `--min-providers=<n>` في نموذج أكبر. التبني - يتم إرساله باستخدام الملخص (`--adoption-report=<path>` يسمح بإضافة الموقع)، ويؤدي النص البرمجي إلى `--require-direct-only` (في сооответствии с الاحتياطي) و `--require-telemetry`، إذا قمت بتسليم العلم الترحيبي. استخدم `XTASK_SORAFS_ADOPTION_FLAGS` لاستكمال وسيطات xtask الإضافية (على سبيل المثال، `--allow-single-source` في وقت الرجوع إلى الإصدار السابق المتوافق، من أجل البوابة و تيربيل، والاحتياطي القسري). قم بشراء بوابة الاعتماد مع `--skip-adoption-check` فقط من خلال التشخيص المحلي؛ يجب أن تشتمل خريطة الطريق على كل ما هو منتظم في النظام الأولي بما في ذلك اعتماد الحزمة.

## 5. إعادة فحص قائمة الاختيار1. **تكوينات التكوين:** قم بتخزين ملف تعريف JSON في النظام الأول في المستودعات `iroha_config` واكتبه في تغيير التذكرة.
2. **بوابة التدقيق:** تأكد من أن نقاط Torii تغطي TLS وقدرة TLV وسجل المدققين على التخصيص بشكل مسبق نظام. قم بنشر بوابة الملف الشخصي السياسي للمشغلين.
3. **الامتثال للامتثال:** يمكنك الاطلاع على قواعد اللعبة الأساسية من خلال الامتثال/المراجعين التنظيميين والحصول على موافقة العمل باسمك إجابة.
4. **التشغيل الجاف:** مجموعة مختارة من اختبارات الامتثال والجلب المرحلي للحماية من Torii. أرشفة مخرجات لوحة النتائج وملخص CLI.
5. **الإلغاء في المقدمة:** قم بالتغيير مرة أخرى، الإلغاء `transport_policy` إلى `direct_only` (إذا تم اختيار `soranet-first`) و جداول البيانات الخاصة بالوضع الحالي (زمن الوصول `sorafs_fetch`، موفرو الخدمة). قم بتوثيق الخطة من SoraNet-first بعد ذلك، مثل SNNet-4/5/5a/5b/6a/7/8/12/13 انتقل إلى `roadmap.md:532`.
6. **التغيير اللاحق:** استخدم لوحة النتائج وجلب الملخص ومراقبة النتائج عند إزالة التذكرة. قم باستعادة `status.md` مع إعادة ضبط البيانات وجميع الحالات الشاذة.

تأكد من أن قائمة التحقق هذه مناسبة لـ runbook `sorafs_node_ops`، حيث يمكن للمشغلين تكرار العملية لإلغاء قفل الحياة. عندما تنتقل SNNet-5 إلى GA، اختر الخيار الاحتياطي بعد إعادة التوازن إلى أجهزة القياس عن بعد.## 6. بوابة الإحالة والتبني

نحن نوفر نظامًا أفضل للحمل لبوابة اعتماد SF-6c. قم بالاشتراك في لوحة النتائج، والملخص، ومغلف البيان، والاعتماد لأي شيء تم التحقق منه بحيث يمكن لـ `cargo xtask sorafs-adoption-check` التحقق من الوضع الاحتياطي. النتيجة هي الدخول إلى بوابة الاختبار، وهو ما يضمن ثبات طريقة النقل في التذاكر.- **وسائل النقل:** `scoreboard.json` تحتاج إلى إيقاف `transport_policy="direct_only"` (وإلغاء تحديد `transport_policy_override=true`، عندما تقوم بإلغاء الرجوع إلى إصدار أقدم). بعد ذلك، هناك بعض السياسات المجهولة التي يتم إجراؤها حتى الآن عند اتباع الإعدادات الافتراضية التي تتيح للمراجعين رؤية إلغاء الاستنساخ من الخطة اللاحقة إخفاء الهوية.
- **مقدمي الطلبات:** يتم تسجيل بيانات البوابة فقط من خلال `provider_count=0` وإرسال `gateway_provider_count=<n>` من خلال Torii. لا تستخدم JSON بشكل صحيح: CLI/SDK تم تحديثه بالفعل، بوابة الاعتماد تحجب الحجب دون الحاجة إلى التعلم.
- **بيان البيان:** عندما تتعرف على بوابات Torii، قم بإعادة تقديم `--gateway-manifest-envelope <path>` (أو ما يعادل SDK)، لذلك تم تسجيل `gateway_manifest_provided` و`gateway_manifest_id`/`gateway_manifest_cid` في `scoreboard.json`. يرجى ملاحظة أن `summary.json` يتصل بهم مثل `manifest_id`/`manifest_cid`; التحقق من التبني ممكن إذا فشل أي ملف في عدم التواصل مع أي زوج.
- **قياس الاتصال عن بعد:** عندما تتم الموافقة على قياس الاتصال عن بعد، أغلق البوابة باستخدام `--require-telemetry` لتأكيد اعتماده مقياس الأداء. يمكن للتكرارات ذات فجوات الهواء أن ترفع العلم، لكن CI والتذاكر تخفف من عبء الديون الموثقة.

المثال التمهيدي:

```bash
cargo xtask sorafs-adoption-check \
  --scoreboard fetch_state/direct_mode_scoreboard.json \
  --summary fetch_state/direct_mode_summary.json \
  --allow-single-source \
  --require-direct-only \
  --json-out artifacts/sorafs_direct_mode/adoption_report.json \
  --require-telemetry
```

استخدم `adoption_report.json` الآن باستخدام لوحة النتائج والملخص والمغلف الظاهر وحزمة شعارات الدخان. تعمل هذه العناصر على تعزيز وظيفة التبني في CI (`ci/check_sorafs_orchestrator_adoption.sh`) وتشتمل على عملية تدقيق ممتازة للرجوع إلى الإصدار السابق.