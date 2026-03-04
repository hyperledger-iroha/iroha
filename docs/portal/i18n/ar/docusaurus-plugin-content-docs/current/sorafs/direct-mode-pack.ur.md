---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/direct-mode-pack.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: حزمة الوضع المباشر
العنوان: SoraFS حزمة احتياطية للوضع المباشر (SNNet-5a)
Sidebar_label: حزمة الوضع المباشر
الوصف: ينتقل SNNet-5a إلى SoraFS إلى الوضع المباشر Torii/QUIC مما يؤدي إلى إجراء عمليات التكوين الضرورية وفحوصات الامتثال وخطوات التشغيل.
---

:::ملاحظة مستند ماخذ
:::

تعمل دوائر SoraNet SoraFS على النقل الافتراضي، بالإضافة إلى عنصر خريطة الطريق **SNNet-5a** وهو إجراء احتياطي منظم مطلوب وطرح عدم الكشف عن هويته مكمل لمشغلي الوصول الحتمي للقراءة. تحتوي هذه الحزمة على مقابض CLI/SDK وملفات تعريف التكوين واختبارات الامتثال وقائمة التحقق من النشر والتي تتضمن أيضًا عمليات نقل خصوصية مثل SoraFS ووضع Torii/QUIC المباشر. ہیں۔

لم يعد هناك سوى التدريج الاحتياطي وبيئات الإنتاج المنظمة في لاو، حيث لم تعد SNNet-5 وSNNet-9 بوابات الاستعداد متاحة. القطع الأثرية الصغيرة الشائعة SoraFS ضمانات النشر التي يحتاجها المشغلون ضرورية في أوضاع مجهولة ومباشرة لتبديل الأدوية.

## 1. أعلام CLI وSDK- `sorafs_cli fetch --transport-policy=direct-only ...` تعطيل جدولة الترحيل ہے وTorii/QUIC وسائل النقل فرض کرتا ہے. مساعدة CLI على `direct-only` التي قبلت القيمة بالكامل.
- مجموعات SDK التي تحتوي على `OrchestratorConfig::with_transport_policy(TransportPolicy::DirectOnly)` تم تعيينها على زر "الوضع المباشر" لتبديل عرض البطاقة. يقوم `iroha::ClientOptions` و`iroha_android` بإنشاء الارتباطات والتعداد للأمام.
- أدوات استخدام البوابة (`sorafs_fetch`، ارتباطات Python) مشتركة مع مساعدي Norito JSON الذين يقومون بتبديل التحليل المباشر فقط لأتمتة سلوك الأتمتة.

تشير دفاتر التشغيل التي تواجه الشركاء إلى مستند التشغيل وتبديل الميزات لمتغيرات البيئة بما في ذلك `iroha_config` من خلال سلك الأسلاك.

## 2. ملفات تعريف سياسة البوابة

يستمر تكوين المنسق الحتمي في استخدام Norito JSON. يحتوي `docs/examples/sorafs_direct_mode_policy.json` على مثال لملف التعريف أو تشفير الكرتا:

- `transport_policy: "direct_only"` — إن مقدمي الخدمة يرفضون البطاقة ويخصصون SoraNet Relay Transports للإعلان عن البطاقة.
- `max_providers: 2` — نقاط نهاية Torii/QUIC موثوقة من الأقران المباشرين. يتم تعديل بدلات الامتثال الإقليمية وفقًا لذلك.
- `telemetry_region: "regulated-eu"` — المقاييس المنبعثة في تسمية البطاقة ولوحة المعلومات/عمليات التدقيق الاحتياطية في كل مكان.
- ميزانيات إعادة المحاولة المحافظة (`retry_budget: 2`, `provider_failure_threshold: 3`) لا يوجد قناع بوابات تم تكوينه بشكل خاطئ.JSON `sorafs_cli fetch --config` (الأتمتة) أو روابط SDK (`config_from_json`) تقوم بتحميل الملفات، ويكشف مشغلو السياسة عن السياسة. مسارات التدقيق لإخراج لوحة النتائج (`persist_path`) محفوظات کریں۔

مقابض التنفيذ على جانب البوابة `docs/examples/sorafs_gateway_direct_mode.toml` ذات درج. يحتوي هذا القالب `iroha app sorafs gateway direct-mode enable` على إخراج يعكس البطاقة، وتعطيل عمليات التحقق من المغلف/القبول، وسلك الإعدادات الافتراضية للحد الأقصى للمعدل، وجدول `direct_mode`، وأسماء المضيفين المشتقة من الخطة وملخصات البيان يتم ملؤها. تلتزم إدارة التكوين بمقتطف من قيم العناصر النائبة التي ستحل محل خطة الطرح.

## 3. مجموعة اختبارات الامتثال

الاستعداد للوضع المباشر مع المنسق وصناديق CLI دون تغطية تشمل العناصر التالية:- `direct_only_policy_rejects_soranet_only_providers` هو السبب وراء فشل `TransportPolicy::DirectOnly` في فشل إعلان المرشح فقط SoraNet Relays support كرتا ہو..[crates/sorafs_orchestrator/src/lib.rs:7238]
- `direct_only_policy_prefers_direct_transports_when_available` هو أحد الأشخاص الذين يقومون بنقل Torii/QUIC الموجود والذي يمكن استخدامه في الأجهزة و SoraNet Relays أثناء الجلسة الخارجية. جئے۔[crates/sorafs_orchestrator/src/lib.rs:7285]
- `direct_mode_policy_example_is_valid` `docs/examples/sorafs_direct_mode_policy.json` تحليل كرتا ومساعدات المستندات التي تم محاذاةها.[crates/sorafs_orchestrator/src/lib.rs:7509][docs/examples/sorafs_direct_mode_policy.json:1]
- `fetch_command_respects_direct_transports` `sorafs_cli fetch --transport-policy=direct-only` الذي تم الاستهزاء به من بوابة Torii لخلاف چلتا، البيئات المنظمة لاختبار الدخان وإطار النقل المباشر ہیں.[صناديق/sorafs_car/tests/sorafs_cli.rs:2733]
- `scripts/sorafs_direct_mode_smoke.sh` هو الأمر وسياسة JSON واستمرارية لوحة النتائج التي تعمل على التفاف الكرتا بالإضافة إلى أتمتة الطرح.

تنشر التحديثات المجموعة المركزة کرنے سے پہلے چلایں:

```bash
cargo test -p sorafs_orchestrator direct_only_policy
cargo test -p sorafs_car --features cli fetch_command_respects_direct_transports
```

إذا فشلت تغييرات المنبع في تجميع مساحة العمل، فستتسبب في حدوث خطأ في الحظر حيث يؤدي `status.md` إلى تسجيل الدخول ومتابعة التبعية مرة أخرى.

## 4. تشغيل الدخان الآليلا توجد انحدارات خاصة ببيئة تغطية CLI (مثل انحراف سياسة البوابة أو عدم التطابق الواضح). إنه مساعد دخان مخصص `scripts/sorafs_direct_mode_smoke.sh` وهو `sorafs_cli fetch` وهو سياسة منسق الوضع المباشر، واستمرارية لوحة النتائج، والتقاط الملخص والتفاف الكرتا.

مثال على الاستخدام:

```bash
./scripts/sorafs_direct_mode_smoke.sh \
  --config docs/examples/sorafs_direct_mode_smoke.conf \
  --provider name=gw-regulated,provider-id=001122...,base-url=https://gw.example/direct/,stream-token=BASE64
```- هناك علامات CLI للبرنامج النصي ومفتاح = ملفات تكوين القيمة ولا تحترم الكرتا (`docs/examples/sorafs_direct_mode_smoke.conf`). يتم نشر الملخص الواضح وإدخالات إعلان الموفر وقيم الإنتاج في الكريں.
- `--policy` بشكل افتراضي على `docs/examples/sorafs_direct_mode_policy.json`، ولكن `sorafs_orchestrator::bindings::config_to_json` يتم تشغيله وتنسيقه من قبل منسق JSON ديا جا سكتا. CLI هي سياسة `--orchestrator-config=PATH` التي تقبل الموافقة على كرتا، ويمكن تشغيل عمليات التشغيل القابلة للتكرار من خلال أعلام بغداد التي يتم ضبطها.
- لا يمكنك استخدام `sorafs_cli` `PATH` كصندوق `sorafs_orchestrator` (ملف تعريف الإصدار) لبناء صندوق وتدفق الدخان الذي يتم شحنه في السباكة ذات الوضع المباشر لممارسة الرياضة.
- المخرجات:
  - الحمولة المجمعة (`--output`، الافتراضي `artifacts/sorafs_direct_mode/payload.bin`).
  - جلب الملخص (`--summary`، الحمولة الافتراضية) بالإضافة إلى منطقة القياس عن بعد وتقارير الموفر التي تتضمن دليل الطرح الخاص بك.
  - سياسة لقطة لوحة النتائج JSON تتبع المسار المستمر ہوتا ہے (مثلاً `fetch_state/direct_mode_scoreboard.json`). إنه ملخص عن تغيير التذاكر في الأرشيف.- أتمتة بوابة الاعتماد: جلب مكمل ہونے کے بعد المساعد `cargo xtask sorafs-adoption-check` چلاتا ہے جس ميں لوحة النتائج المستمرة ومسارات التلخيص المستخدمة ہوتے ہیں. النصاب القانوني الافتراضي مطلوب عبر سطر الأوامر من قبل موفري الخدمة الذين يبلغ عددهم عددًا كبيرًا؛ في ما يلي نموذج لتجاوز `--min-providers=<n>`. ملخص تقارير الاعتماد معتمد (`--adoption-report=<path>` موقع مخصص) والمساعد الافتراضي هو `--require-direct-only` (الرجوع الاحتياطي) و`--require-telemetry` فيما يتعلق بعلم هذا، تمرير كرتا ہے۔ تستخدم وسيطات xtask الإضافية `XTASK_SORAFS_ADOPTION_FLAGS` (مثل الرجوع إلى الإصدار السابق المعتمد أثناء `--allow-single-source` للبوابة الاحتياطية التي تسمح بالتحمل والتنفيذ). يتم استخدام التشخيص المحلي باستخدام `--skip-adoption-check`؛ تتوافق خارطة الطريق مع حزمة تقرير اعتماد التشغيل المباشر المنظم التي يتم تنظيمها.

## 5. قائمة التحقق من الطرح1. **تجميد التكوين:** يتم حفظ ملف تعريف JSON للوضع المباشر في `iroha_config` من خلال متجر الريبو والتجزئة وتغيير التذكرة.
2. **تدقيق البوابة:** الوضع المباشر لتبديل مفتاح التبديل إلى Torii نقاط النهاية TLS، وقدرة TLVs وتسجيل التدقيق يفرضان المراجعة. ملف تعريف سياسة البوابة الذي يقوم المشغلون بنشره.
3. **تسجيل الخروج من الامتثال:** دليل قواعد اللعبة المحدث الذي يتضمن مراجعي الامتثال/التنظيم الذين يشاركون بشكل مستمر وتراكب إخفاء الهوية يراقبون الموافقات التي تلتقطها.
4. **التشغيل التجريبي:** مجموعة اختبارات الامتثال والجلب المرحلي لموفري Torii الموثوق بهم. مخرجات لوحة النتائج وأرشيف ملخصات CLI.
5. **تحويل الإنتاج:** إعلان نافذة التغيير کریں، `transport_policy` کو `direct_only` پر فليب کریں (اگر `soranet-first` منتخب کیا) ومراقبة لوحات المعلومات ذات الوضع المباشر کریں (`sorafs_fetch`) الكمون، عدادات فشل المزود)۔ وثيقة خطة التراجع تم إنشاؤها بواسطة SNNet-4/5/5a/5b/6a/7/8/12/13 `roadmap.md:532` خريج في SoraNet-first وابس.
6. **مراجعة ما بعد التغيير:** لقطات لوحة النتائج، وجلب الملخصات ونتائج المراقبة وتغيير التذكرة وإرفاقها. `status.md` تاريخ السريان وتحديث الحالات الشاذة.قائمة التحقق في دليل التشغيل `sorafs_node_ops` التي تدعم التبديل المباشر للمشغلين بالإضافة إلى بروفة سير العمل. عند استخدام SNNet-5 GA، يمكنك تأكيد التكافؤ في القياس عن بعد للإنتاج بعد التقاعد الاحتياطي.

##6. الأدلة ومتطلبات بوابة التبني

يلتقط الوضع المباشر بوابة اعتماد SF-6c التي ترضي الجميع. قم بتشغيل لوحة النتائج والملخص ومغلف البيان وحزمة تقرير الاعتماد `cargo xtask sorafs-adoption-check` للتحقق من صحة الوضع الاحتياطي. بوابة الحقول المفقودة تفشل في التسجيل، وذلك لتغيير التذاكر مما يؤدي إلى سجل البيانات الوصفية المتوقع.- **بيانات تعريف النقل:** `scoreboard.json` إلى `transport_policy="direct_only"` تعلن عن كرانا چاہيے (ويجب أن تخفض قوة التخفيض إلى `transport_policy_override=true` فليب كریں). حقول سياسة إخفاء الهوية المقترنة ببعضها البعض والإعدادات الافتراضية ترث التصنيف، بالإضافة إلى المراجعين الآخرين الذين قاموا بتنظيم خطة إخفاء الهوية بانحراف أو لا شيء.
- **عدادات الموفر:** جلسات البوابة فقط التي `provider_count=0` تستمر في كل من `gateway_provider_count=<n>` وموفري Torii الذين يبلغ عددهم أكثر من واحد. لا يتم تحرير JSON: تشتق تعدادات CLI/SDK من العناصر وبوابة الاعتماد واللقطات وترفض العناصر والتقسيم المفقود.
- **الدليل الواضح:** تتضمن بوابات Torii التوقيع على `--gateway-manifest-envelope <path>` (أو ما يعادل SDK) وتمرير بطاقة `gateway_manifest_provided` إلى `gateway_manifest_id`/`gateway_manifest_cid` `scoreboard.json` سجل ہوں. هذه الأداة `summary.json` موجودة و`manifest_id`/`manifest_cid` متاحة؛ لم يفشل أي زوج من الأزواج في التحقق من التبني.
- **توقعات القياس عن بعد:** عند التقاط القياس عن بعد ستصل إلى `--require-telemetry` وستصدر مقاييس تقرير الاعتماد مرة أخرى. تشير التدريبات الهوائية إلى حذف أي سكتة دماغية أو CI وتغيير التذاكر أو وثيقة الغياب.

مثال:

```bash
cargo xtask sorafs-adoption-check \
  --scoreboard fetch_state/direct_mode_scoreboard.json \
  --summary fetch_state/direct_mode_summary.json \
  --allow-single-source \
  --require-direct-only \
  --json-out artifacts/sorafs_direct_mode/adoption_report.json \
  --require-telemetry
````adoption_report.json` لوحة النتائج والملخص ومغلف البيان وحزمة سجل الدخان التي يتم إرفاقها. إنها مهمة اعتماد CI المصطنعة (`ci/check_sorafs_orchestrator_adoption.sh`) التي تعكس البطاقة وتخفض التصنيفات في الوضع المباشر وهي قابلة للتدقيق.