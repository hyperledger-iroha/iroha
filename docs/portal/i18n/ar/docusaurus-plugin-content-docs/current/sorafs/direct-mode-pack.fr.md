---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/direct-mode-pack.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: حزمة الوضع المباشر
العنوان: حزمة الرد في الوضع المباشر SoraFS (SNNet-5a)
Sidebar_label: وضع الحزمة المباشر
الوصف: يتطلب التكوين وضوابط المطابقة وخطوات النشر عند استغلال SoraFS في الوضع المباشر Torii/QUIC أثناء الانتقال SNNet-5a.
---

:::ملاحظة المصدر الكنسي
هذه الصفحة تعكس `docs/source/sorafs/direct_mode_pack.md`. قم بمزامنة النسختين حتى ترجع مجموعة أبو الهول التاريخية.
:::

تبقى دوائر SoraNet قيد النقل بشكل افتراضي لـ SoraFS، بالإضافة إلى عنصر خريطة الطريق **SNNet-5a** الذي يتطلب ردًا منظمًا حتى يتمكن المشغلون من الاحتفاظ بإمكانية الوصول إلى المحاضرة التي تحددها حتى ينتهي نشر المجهول. تقوم هذه الحزمة بتجميع أزرار CLI/SDK وملفات تعريف التكوين واختبارات المطابقة وقائمة التحقق من النشر الضرورية لتنفيذ SoraFS في الوضع المباشر Torii/QUIC دون لمس وسائل النقل السرية.

يتم تطبيق الرد على بيئات التدريج والإنتاج حتى يتم منح SNNet-5 إلى SNNet-9 بوابات الاستعداد. احتفظ بالعناصر التي تم إنشاؤها باستخدام مواد النشر SoraFS المعتادة حتى يتمكن المشغلون من التنقل بين الأوضاع المجهولة والمباشرة حسب الطلب.

## 1. أعلام CLI وSDK- `sorafs_cli fetch --transport-policy=direct-only ...` قم بإلغاء تنشيط أمر الترحيل وفرض عمليات النقل Torii/QUIC. L'aide du CLI liste désormais `direct-only` ذو قيمة مقبولة.
- يجب أن تحدد SDK `OrchestratorConfig::with_transport_policy(TransportPolicy::DirectOnly)` التي تعرض تبديل "الوضع المباشر". تنشر الروابط التي تم إنشاؤها في `iroha::ClientOptions` و`iroha_android` نفس التعداد.
- يمكن لأدوات البوابة (`sorafs_fetch`، روابط Python) تحليل التبديل بشكل مباشر فقط عبر المساعدين Norito JSON المشاركين لكي تحصل الأتمتة على نفس السلوك.

قم بتوثيق العلامة في دفاتر التشغيل الموجهة بشكل مشترك وقم بتمرير المفاتيح عبر `iroha_config` وفقًا لمتغيرات البيئة.

## 2. ملفات التعريف السياسية للبوابة

استخدم JSON Norito للاستمرار في تكوين محدد للمنسق. الملف التعريفي للمثال في `docs/examples/sorafs_direct_mode_policy.json` ترميز:

- `transport_policy: "direct_only"` — قم بإعادة تعيين مقدمي الخدمة الذين أعلنوا عن عمليات نقل SoraNet.
- `max_providers: 2` - تحديد الأزواج التي توجه نقاط النهاية الإضافية Torii/QUIC بالإضافة إلى الملفات. اضبط قيود المطابقة الإقليمية.
- `telemetry_region: "regulated-eu"` — ضبط المقاييس المنبعثة من لوحات المعلومات وعمليات التدقيق التي تميز عمليات النسخ.
- ميزانيات إعادة محاولة الحفظ (`retry_budget: 2`، `provider_failure_threshold: 3`) لتجنب إخفاء البوابات التي تم تكوينها بشكل خاطئ.قم بشحن JSON عبر `sorafs_cli fetch --config` (الأتمتة) أو عبر روابط SDK (`config_from_json`) قبل كشف السياسة للمشغلين. واصل عملية فحص لوحة النتائج (`persist_path`) من أجل ممرات التدقيق.

يتم التقاط إعدادات بوابة التطبيق في `docs/examples/sorafs_gateway_direct_mode.toml`. يعكس النموذج نوع `iroha app sorafs gateway direct-mode enable`، ويقوم بإلغاء تنشيط فحوصات المغلف/القبول، ويوصل القيم حسب الحد الأقصى للسعر، ويعيد الجدول `direct_mode` بأسماء المضيفين المشتقة من الخطة وملخصات البيان. استبدل قيم المساحة المحفوظة بخطة التشغيل الخاصة بك قبل إصدار الإصدار الإضافي في إدارة التكوين.

## 3. مجموعة اختبارات المطابقة

يتضمن الاستعداد للوضع المباشر تغيير الغطاء في المنسق وفي صناديق CLI:- `direct_only_policy_rejects_soranet_only_providers` يضمن أن `TransportPolicy::DirectOnly` يتردد صداه سريعًا عندما لا يقوم كل إعلان مرشح بتحمل مسؤولية les relais SoraNet.[crates/sorafs_orchestrator/src/lib.rs:7238]
- `direct_only_policy_prefers_direct_transports_when_available` يضمن أن عمليات النقل Torii/QUIC يتم استخدامها عندما تكون متاحة وأن تتابع SoraNet مستبعد من الجلسة.
- تحليل `direct_mode_policy_example_is_valid` `docs/examples/sorafs_direct_mode_policy.json` لضمان محاذاة الوثائق مع المساعدين المساعدين. 【crates/sorafs_orchestrator/src/lib.rs:7509】【docs/examples/sorafs_direct_mode_policy.json:1】
- `fetch_command_respects_direct_transports` تمرين `sorafs_cli fetch --transport-policy=direct-only` على بوابة Torii محاكاة، يوفر اختبار دخان للبيئات المنظمة التي تربط وسائل النقل المباشرة.[crates/sorafs_car/tests/sorafs_cli.rs:2733]
- `scripts/sorafs_direct_mode_smoke.sh` يغلف نفس الأمر مع JSON السياسي واستمرارية لوحة النتائج لأتمتة الطرح.

قم بتنفيذ المجموعة ciblée قبل نشر كل يوم:

```bash
cargo test -p sorafs_orchestrator direct_only_policy
cargo test -p sorafs_car --features cli fetch_command_respects_direct_transports
```

إذا كان تجميع مساحة العمل قد تسبب في حدوث تغييرات في المنبع، فقم بترك الخطأ المتراكم في `status.md` وأعد تنفيذ الاعتماد مرة أخرى.

## 4. تنفيذ عمليات الدخان تلقائيًالا يكشف غطاء CLI وحده عن التراجعات المحددة في البيئة (على سبيل المثال، استخلاص بوابة سياسية أو قصور في البيان). تم استخدام مساعد الدخان في `scripts/sorafs_direct_mode_smoke.sh` والمغلف `sorafs_cli fetch` مع سياسة التنسيق في الوضع المباشر، واستمرارية لوحة النتائج، والتقاط السيرة الذاتية.

مثال على الاستخدام:

```bash
./scripts/sorafs_direct_mode_smoke.sh \
  --config docs/examples/sorafs_direct_mode_smoke.conf \
  --provider name=gw-regulated,provider-id=001122...,base-url=https://gw.example/direct/,stream-token=BASE64
```- يحترم البرنامج النصي علامات CLI وملفات التكوين الرئيسية = القيمة (انظر `docs/examples/sorafs_direct_mode_smoke.conf`). قم بفحص ملخص البيان وإدخالات إعلانات مقدمي الخدمة مع قيم الإنتاج قبل التنفيذ.
- `--policy` محدد بشكل افتراضي مقابل `docs/examples/sorafs_direct_mode_policy.json`، ولكن يمكن توفير جميع منتجات JSON d'orchesstrateur وفقًا لـ `sorafs_orchestrator::bindings::config_to_json`. يقبل CLI السياسة عبر `--orchestrator-config=PATH`، مما يسمح بتشغيل النسخ القابلة لإعادة الإنتاج دون ضبط الأعلام بشكل رئيسي.
- عندما لا يكون `sorafs_cli` في `PATH`، فإن المساعدة في بناء الصندوق من بعد الصندوق `sorafs_orchestrator` (إصدار الملف الشخصي) تمكن الدخان من تشغيل السباكة بطريقة مباشرة.
- الطلعات :
  - تجميع الحمولة (`--output`، افتراضيًا `artifacts/sorafs_direct_mode/payload.bin`).
  - ملخص الجلب (`--summary`، افتراضيًا على طول الحمولة) يحتوي على منطقة القياس عن بعد وتقارير مقدمي الخدمة المستخدمة لإثبات الطرح.
  - لقطة لوحة النتائج المستمرة على المسار المعلن في JSON السياسي (على سبيل المثال `fetch_state/direct_mode_scoreboard.json`). أرشفة السيرة الذاتية في تذاكر التغيير.- أتمتة بوابة التبني: بعد انتهاء عملية الجلب، يقوم المساعد باستدعاء `cargo xtask sorafs-adoption-check` باستخدام المسارات المستمرة للوحة النتائج والملخص. النصاب القانوني المطلوب افتراضيًا يتوافق مع عدد مقدمي الخدمة المتوفرين على خط الأمر؛ استبدلها بـ `--min-providers=<n>` عندما تحتاج إلى قطعة كبيرة الحجم. تقارير التبني هي مكتوبة في كل السيرة الذاتية (`--adoption-report=<path>` يمكن أن تحدد مكانًا مخصصًا) والمساعد يمرر `--require-direct-only` افتراضيًا (محاذاة على النسخة) و`--require-telemetry` عند تقديم العلامة مراسل. استخدم `XTASK_SORAFS_ADOPTION_FLAGS` لنقل وسائط xtask الإضافية (على سبيل المثال `--allow-single-source` عند الرجوع إلى الإصدار السابق والموافقة على السماح للبوابة بالقبول وفرض الرد). لا تقل بوابة التبني مع `--skip-adoption-check` أثناء التشخيص المحلي؛ تتضمن خارطة الطريق التي تتطلب كل عملية تنظيمية في الوضع المباشر حزمة التبني.

## 5. قائمة التحقق من النشر1. **جيل التكوين:** قم بتخزين الملف الشخصي JSON في الوضع المباشر في مستودعك `iroha_config` وقم بإرسال التجزئة في تذكرة التغيير الخاصة بك.
2. **بوابة التدقيق:** تؤكد أن نقاط النهاية Torii تنطبق على TLS وسعة TLV وتسجيل التدقيق مسبقًا في الوضع المباشر. قم بنشر الملف التعريفي للبوابة السياسية للمشغلين.
3. **التحقق من المطابقة:** شارك قواعد اللعبة في كل يوم مع المراجعين المطابقين / المعتمدين واحصل على الموافقات لتشغيلها من خلال تراكب مجهول.
4. **التشغيل الجاف:** تنفيذ مجموعة المطابقة بالإضافة إلى جلب التدريج ضد موفري الثقة Torii. أرشفة طلعات لوحة النتائج والسيرة الذاتية CLI.
5. **الإنتاج الأساسي:** الإعلان عن نافذة التغيير، والانتقال من `transport_policy` إلى `direct_only` (إذا كنت ترغب في اختيار `soranet-first`) ومراقبة لوحات المعلومات في الوضع المباشر (زمن الاستجابة `sorafs_fetch`، والكمبيوترات d'échec des مقدمي الخدمة). قم بتوثيق خطة التراجع للرجوع إلى SoraNet-first مرة واحدة حيث أن SNNet-4/5/5a/5b/6a/7/8/12/13 قد انتهى في `roadmap.md:532`.
6. **مراجعة ما بعد التغيير:** قم بإرفاق لقطات لوحة النتائج والسيرة الذاتية للجلب ونتائج مراقبة تذكرة التغيير. Metez à jour `status.md` مع التاريخ الفعال وكل الشذوذ.تأكد من قائمة التحقق الموجودة في دليل التشغيل `sorafs_node_ops` حتى يتمكن المشغلون من تكرار التدفق قبل مستوى حقيقي. عندما يمر SNNet-5 بـ GA، قم بإيقاف الرد بعد تأكيد التكافؤ في جهاز القياس عن بعد للإنتاج.

## 6. متطلبات المسبقة وبوابة التبني

يتم التقاط اللقطات في الوضع المباشر طوال الوقت بما يرضي بوابة اعتماد SF-6c. أعد تجميع لوحة النتائج والملخص ومغلف البيان وتقرير التبني لكل مرة حتى يتمكن `cargo xtask sorafs-adoption-check` من التحقق من وضعية الرد. Les Champs Manquants Font échouer le gate، consignez donc les métadonnées atueues dans les Tickets de التغيير.- **بيانات النقل:** `scoreboard.json` يجب الإعلان عن `transport_policy="direct_only"` (وحذف `transport_policy_override=true` عند إجبارك على الرجوع إلى إصدار أقدم). احفظ الأبطال السياسيين المجهولين المرتبطين بنفس الطريقة عندما تكون لديهم قيم مفترضة حتى يفكر المستشارون في تنفيذ خطة عدم الكشف عن هويتهم من خلال الخطوات.
- **مزودو الحسابات:** تعمل بوابة الجلسات فقط على الاستمرار في `provider_count=0` واستعادة `gateway_provider_count=<n>` باستخدام عدد الموفرين Torii. تجنب تحرير JSON بشكل رئيسي: يشتق CLI/SDK من أجهزة الكمبيوتر، وتعيد بوابة الاعتماد اللقطات التي تؤدي إلى الفصل.
- **البيان المسبق:** عند مشاركة البوابات Torii، قم بتمرير علامة `--gateway-manifest-envelope <path>` (أو SDK المكافئة) حتى يتم تسجيل `gateway_manifest_provided` و`gateway_manifest_id`/`gateway_manifest_cid` في `scoreboard.json`. تأكد من أن `summary.json` يحمل نفس الاسم `manifest_id`/`manifest_cid` ; يظهر التحقق من التبني في حالة وجود ملف واحد أو زوج.
- **مراقبة القياس عن بعد:** عند التقاط القياس عن بعد، قم بتشغيل البوابة باستخدام `--require-telemetry` حتى يثبت تقرير التبني أن القياسات لم تنبعث. يمكن أن تؤدي التكرارات في الهواء إلى إزالة العلم، بالإضافة إلى CI وتذاكر التغيير ستوثق الغياب.

مثال :

```bash
cargo xtask sorafs-adoption-check \
  --scoreboard fetch_state/direct_mode_scoreboard.json \
  --summary fetch_state/direct_mode_summary.json \
  --allow-single-source \
  --require-direct-only \
  --json-out artifacts/sorafs_direct_mode/adoption_report.json \
  --require-telemetry
```قم بإرفاق `adoption_report.json` بلوحة النتائج والملخص ومغلف البيان وحزمة سجلات الدخان. تعكس هذه العناصر أن مهمة اعتماد CI (`ci/check_sorafs_orchestrator_adoption.sh`) تفرض وتؤدي إلى تخفيض التصنيفات في وضع قابل للتدقيق المباشر.