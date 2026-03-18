---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/direct-mode-pack.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: حزمة الوضع المباشر
العنوان: حزمة الطوارئ للوضع المباشر لـ SoraFS (SNNet-5a)
Sidebar_label: حزمة الوضع المباشر
الوصف: التكوين المطلوب واختبارات الوفاء وإجراءات التنفيذ لتشغيل SoraFS بطريقة مباشرة Torii/QUIC أثناء عملية نقل SNNet-5a.
---

:::ملاحظة فوينتي كانونيكا
هذه الصفحة تعكس `docs/source/sorafs/direct_mode_pack.md`. احتفظ بنسخ متزامنة.
:::

تعمل دوائر SoraNet على النقل المحدد مسبقًا لـ SoraFS، لكن عنصر خريطة الطريق **SNNet-5a** يتطلب تنظيمًا احتياطيًا حتى يتمكن المشغلون من الحفاظ على وصول محدد للقراءة حتى يكتمل النشر المجهول. تحتوي هذه الحزمة على مقابض CLI / SDK وملفات التكوين واختبارات الوفاء وقائمة التنفيذ الضرورية لتنفيذ SoraFS بطريقة مباشرة Torii/QUIC دون لمس وسائل نقل الخصوصية.

يتم تطبيق الإجراء الاحتياطي على التدريج وبدء الإنتاج المنظم مما يجعل SNNet-5 وSNNet-9 يتفوقان على بوابات التحضير. احتفظ بالمصنوعات اليدوية جنبًا إلى جنب مع المادة المعتادة للنشر SoraFS حتى يتمكن المشغلون من التبديل بين الأوضاع المجهولة والمباشرة حسب الطلب.

## 1. أعلام CLI وSDK- `sorafs_cli fetch --transport-policy=direct-only ...` يقوم بإلغاء تنشيط برمجة التقارير وتنشيط وسائل النقل Torii/QUIC. مساعدة CLI الآن قائمة `direct-only` كقيمة مقبولة.
- يجب على SDK تعيين `OrchestratorConfig::with_transport_policy(TransportPolicy::DirectOnly)` مما يؤدي إلى فتح مفتاح "الوضع المباشر". تُعيد الروابط التي تم إنشاؤها في `iroha::ClientOptions` و`iroha_android` نفس التعداد.
- يمكن لأدوات البوابة (`sorafs_fetch`، روابط Python) تفسير التبديل المباشر فقط من خلال المساعدين Norito JSON المتضمنين لكي تحصل الأتمتة على نفس النتيجة.

قم بتوثيق العلامة في دفاتر التشغيل الموجهة للشركاء وتمكن من التبديل عبر `iroha_config` عبر متغيرات التشغيل.

## 2. ملفات التعريف السياسية للبوابة

استخدم JSON de Norito للاستمرار في تكوين محدد للأوركسترادور. ملف تعريف المثال المشفر `docs/examples/sorafs_direct_mode_policy.json`:

- `transport_policy: "direct_only"` — طلب من الموردين الإعلان عن نقل Relé SoraNet فقط.
- `max_providers: 2` - يحد من الأقران الذين يوجهون نقاط النهاية Torii/QUIC أكثر قابلية للتنفيذ. قم بضبط تنازلات الامتثال الإقليمية.
- `telemetry_region: "regulated-eu"` — يضبط المقاييس الصادرة لتمييز لوحات المعلومات ومراجع الصوت عن عمليات التنفيذ الاحتياطية.
- إعادة ضبط الحافظات (`retry_budget: 2`, `provider_failure_threshold: 3`) لتجنب إخفاء البوابات التي تم تكوينها بشكل خاطئ.قم بتحميل JSON عبر `sorafs_cli fetch --config` (الأتمتة) أو روابط SDK (`config_from_json`) قبل عرض السياسة على المشغلين. استمر في إخراج لوحة النتائج (`persist_path`) لعمليات الاستماع.

تم العثور على مقابض تنفيذ باب البوابة في `docs/examples/sorafs_gateway_direct_mode.toml`. تعكس اللوحة خروج `iroha app sorafs gateway direct-mode enable`، وتزيل صعوبات المغلف/القبول، وترسل قيم عيب الحد الأقصى للسعر، وتنشر جدول `direct_mode` بأسماء المضيفين المشتقة من الخطة وخلاصات البيان. استبدل قيم علامة الموضع بخطة النشر الخاصة بك قبل إصدار الجزء في إدارة التكوين.

## 3. جناح اختبار الولاء

يشتمل الإعداد المباشر الآن على تغطية كبيرة للمنسق مثل صناديق CLI:- `direct_only_policy_rejects_soranet_only_providers` يضمن أن `TransportPolicy::DirectOnly` سيسقط بسرعة عندما يتم الإعلان عن مرشح منفرد يعترف بمحتوى SoraNet.[crates/sorafs_orchestrator/src/lib.rs:7238]
- `direct_only_policy_prefers_direct_transports_when_available` تأكد من استخدام النقل Torii/QUIC عندما يتم تقديمه ويتم استبعاد ملفات SoraNet من الجلسة.
- `direct_mode_policy_example_is_valid` قام بتحليل `docs/examples/sorafs_direct_mode_policy.json` لضمان الحفاظ على الوثائق متسقة مع مساعدي الاستخدام. 【crates/sorafs_orchestrator/src/lib.rs:7509】【docs/examples/sorafs_direct_mode_policy.json:1】
- `fetch_command_respects_direct_transports` يستخدم `sorafs_cli fetch --transport-policy=direct-only` مقابل بوابة Torii محاكاة، ويقدم اختبارًا روحيًا لأنظمة تنظيمية تنقل التوجيهات.
- `scripts/sorafs_direct_mode_smoke.sh` يشمل نفس الأمر باستخدام JSON السياسي واستمرارية لوحة النتائج لأتمتة عملية الطرح.

قم بتنفيذ المجموعة المعززة قبل نشر التحديثات:

```bash
cargo test -p sorafs_orchestrator direct_only_policy
cargo test -p sorafs_car --features cli fetch_command_respects_direct_transports
```

إذا فشلت عملية تجميع مساحة العمل بسبب إجراء تغييرات على المنبع، فقم بتسجيل الخطأ المحظور في `status.md` ثم قم بالتنفيذ عندما يتم تنفيذ الاعتماد.

## 4. تشغيل الدخان تلقائيًالا تكشف تغطية CLI وحدها عن تراجعات محددة في الداخل (على سبيل المثال، اشتقاق سياسة البوابة أو فشل البيانات). مساعد دخان مخصص يعيش في `scripts/sorafs_direct_mode_smoke.sh` ويغطي `sorafs_cli fetch` مع سياسة الباحث المباشر واستمرارية لوحة النتائج والتقاط السيرة الذاتية.

مثال الاستخدام:

```bash
./scripts/sorafs_direct_mode_smoke.sh \
  --config docs/examples/sorafs_direct_mode_smoke.conf \
  --provider name=gw-regulated,provider-id=001122...,base-url=https://gw.example/direct/,stream-token=BASE64
```- يحتوي البرنامج النصي على علامات CLI كملفات تكوين المفتاح = القيمة (استشارة `docs/examples/sorafs_direct_mode_smoke.conf`). قم بملء ملخص البيان وإدخالات إعلانات المورّد بقيم الإنتاج قبل التنفيذ.
- `--policy` معيب هو `docs/examples/sorafs_direct_mode_policy.json`، لكن يمكن تلخيص أي JSON من المُنشئ المنتج بواسطة `sorafs_orchestrator::bindings::config_to_json`. يقبل CLI السياسة عبر `--orchestrator-config=PATH`، ويتمكن من تنفيذ عمليات إعادة الإنتاج بدون ضبط الأعلام يدويًا.
- عندما لا يكون `sorafs_cli` موجودًا في `PATH`، يتم تجميع المساعد من الصندوق `sorafs_orchestrator` (إصدار الملف) حتى تقوم اختبارات الرطوبة بتشغيل السباكة بطريقة مباشرة يتم إرسالها.
- ساليداس :
  - مجموعة الحمولة النافعة (`--output`، بسبب الخلل `artifacts/sorafs_direct_mode/payload.bin`).
  - استئناف الجلب (`--summary`، بسبب الخلل الموجود بجانب الحمولة) الذي يحتوي على منطقة القياس عن بعد وتقارير الموردين المستخدمة كدليل على بدء التشغيل.
  - لقطة لوحة النتائج مستمرة في المسار المعلن في JSON السياسي (على سبيل المثال، `fetch_state/direct_mode_scoreboard.json`). أرشفة جنبًا إلى جنب مع السيرة الذاتية في تذاكر التغيير.- أتمتة بوابة التبني: عندما تنتهي عملية الجلب من استدعاء المساعد `cargo xtask sorafs-adoption-check` باستخدام مسارات لوحة النتائج المستمرة والملخص. النصاب القانوني المطلوب للخلل هو عدد الموردين المقدمين على خط الأوامر؛ Anúlalo مع `--min-providers=<n>` عندما تحتاج إلى رئيس بلدية. يتم تسجيل معلومات التبني مع السيرة الذاتية (`--adoption-report=<path>` ويمكن أن تحدد موقعًا مخصصًا) وينتقل المساعد `--require-direct-only` بسبب الخلل (بالتزامن مع البديل) و`--require-telemetry` بينما يتم عرض العلم المقابل. استخدم `XTASK_SORAFS_ADOPTION_FLAGS` لإعادة استخدام وسائط xtask الإضافية (على سبيل المثال `--allow-single-source` أثناء التخفيض من مستوى الموافقة حتى تتحمل البوابة واستكمال الإجراء الاحتياطي). فقط قم بحذف البوابة باستخدام `--skip-adoption-check` لتشغيل التشخيصات المحلية؛ تتطلب خارطة الطريق أن يتم تنظيم كل تنفيذ بطريقة مباشرة، بما في ذلك حزمة معلومات التبني.

## 5. قائمة التحقق من التتبع1. **تجميع التكوين:** قم بحماية ملف JSON بطريقة مباشرة في مستودعك `iroha_config` وقم بتسجيل التجزئة في تذكرة التغيير.
2. **مراجعة البوابة:** تؤكد أن نقاط النهاية Torii تستخدم TLS وTLVs ذات السعة وتسجيل الدخول قبل التغيير بطريقة مباشرة. قم بنشر الملف السياسي للبوابة للمشغلين.
3. **موافقة الامتثال:** قم بتقسيم قواعد اللعبة التي تم تحديثها مع مراجعات الامتثال / الضوابط التنظيمية والتقاط الموافقات لتشغيل التراكب المجهول.
4. **التشغيل الجاف:** تنفيذ مجموعة الوفاء بشكل أكبر من خلال الجلب والتدريج على عكس الموردين Torii من الثقة. أرشفة مخرجات لوحة النتائج ونتائج CLI.
5. **قص الإنتاج:** الإعلان عن نافذة التغيير، والتحويل من `transport_policy` إلى `direct_only` (إذا اخترت `soranet-first`) ومراقبة لوحات المعلومات بالطريقة المباشرة (زمن الاستجابة لـ `sorafs_fetch`، ووحدات التحكم فالوس دي بروفيدوريس). قم بتوثيق خطة التراجع للرجوع إلى SoraNet-first عندما يتم تدرج SNNet-4/5/5a/5b/6a/7/8/12/13 في `roadmap.md:532`.
6. **مراجعة ما بعد التغيير:** لقطات إضافية للوحة النتائج، وملخصات الجلب، ونتائج مراقبة التذكرة للتغيير. قم بتنشيط `status.md` بفاعلية وأي شذوذ.احتفظ بقائمة التحقق إلى جانب دليل التشغيل `sorafs_node_ops` حتى يتمكن المشغلون من تجربة التدفق قبل التغيير في الحياة. عندما ينضم SNNet-5 إلى GA، يسحب الإجراء الاحتياطي بعد تأكيد تطابق الإنتاج عن بعد.

## 6. متطلبات الأدلة وبوابة التبني

يجب أن تكتمل لقطات الوضع مباشرة ببوابة التبني SF-6c. قم بتجميع لوحة النتائج والسيرة الذاتية ومغلف البيان ومعلومات التبني في كل تنفيذ حتى يتمكن `cargo xtask sorafs-adoption-check` من التحقق من الوضع الاحتياطي. تسقط الحقول الخاطئة على البوابة، حيث تقوم بتسجيل البيانات الوصفية المنتظرة في تذاكر التغيير.- **بيانات التعريف الخاصة بالنقل:** `scoreboard.json` يجب الإعلان عن `transport_policy="direct_only"` (وتنشيط `transport_policy_override=true` عند الرغبة في الرجوع إلى إصدار أقدم). حافظ على معسكرات السياسيين المجهولين بما في ذلك عندما يتم تحديد الإعدادات الافتراضية حتى يتحرك المراجعون إذا تخلصوا من خطة المجهولين من خلال الخطوات.
- **مراقبو الموردين:** يجب أن تستمر البوابة الفردية للجلسات `provider_count=0` ويمكن أن يتم `gateway_provider_count=<n>` برقم الموردين Torii المستخدم. تجنب تحرير JSON يدويًا: يستخرج CLI/SDK المحتوى وبوابة التبني التي يتم التقاطها مما يؤدي إلى حذف الفصل.
- **دليل البيان:** عندما تشارك بوابات Torii، قم بتثبيت `--gateway-manifest-envelope <path>` (أو ما يعادل SDK) حتى يتم تسجيل `gateway_manifest_provided` و`gateway_manifest_id`/`gateway_manifest_cid` في `scoreboard.json`. تأكد من أن `summary.json` يرفع نفس `manifest_id`/`manifest_cid`; تفشل موافقة التبني إذا حذفت أي ملفات على قدم المساواة.
- **توقعات القياس عن بعد:** عندما يرافق القياس عن بعد الالتقاط، قم بتشغيل البوابة باستخدام `--require-telemetry` لتأكد من أن المعلومات ستصدر قياسات. يمكن للدارسين أثناء الرحلة حذف العلم، ولكن يتعين على CI وتذاكر التغيير توثيق الترخيص.

مثال:

```bash
cargo xtask sorafs-adoption-check \
  --scoreboard fetch_state/direct_mode_scoreboard.json \
  --summary fetch_state/direct_mode_summary.json \
  --allow-single-source \
  --require-direct-only \
  --json-out artifacts/sorafs_direct_mode/adoption_report.json \
  --require-telemetry
```ملحق `adoption_report.json` جنبًا إلى جنب مع لوحة النتائج والملخص ومغلف البيان وحزمة سجلات الدخان. توضح هذه المصنوعات ما الذي يتم تطبيقه على وظيفة التبني في CI (`ci/check_sorafs_orchestrator_adoption.sh`) وتحافظ على تدقيق التخفيضات بالطريقة المباشرة.