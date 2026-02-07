---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/direct-mode-pack.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: حزمة الوضع المباشر
العنوان: Pacote de contingencia de modo direto do SoraFS (SNNet-5a)
Sidebar_label: حزمة الوضع مباشرة
الوصف: تفويض التكوين، واختبارات الامتثال، وتصاريح التشغيل عند تشغيل SoraFS بطريقة مباشرة إلى Torii/QUIC أثناء نقل SNNet-5a.
---

:::ملاحظة فونتي كانونيكا
هذه الصفحة espelha `docs/source/sorafs/direct_mode_pack.md`. Mantenha ambas as copias sincronzadas.
:::

ستستمر دوائر SoraNet في نقل المسار إلى SoraFS، ولكن عنصر خريطة الطريق **SNNet-5a** يتطلب تنظيمًا احتياطيًا لتمكين المشغلين من الوصول إلى القراءة الحتمية حتى يكتمل بدء التشغيل المجهول. تلتقط هذه الحزمة مقابض CLI/SDK، وخيارات التكوين، واختبارات الامتثال، وقائمة التحقق من النشر اللازمة للتوجيه إلى SoraFS بالطريقة المباشرة إلى Torii/QUIC دون استخدام وسائل نقل الخصوصية.

يتم تطبيق الإجراء الاحتياطي على التدريج وبيئة الإنتاج المنظمة بحيث يمر SNNet-5 وSNNet-9 ببوابات الحماية. الحفاظ على المصنوعات اليدوية جنبًا إلى جنب مع مادة النشر SoraFS للمشغلين البديلين بين أوضاع مجهولة وموجهة حسب الطلب.

## 1. أعلام CLI وSDK- `sorafs_cli fetch --transport-policy=direct-only ...` تم إلغاء جدول أعمال المرحلات وإيقاف عمليات النقل Torii/QUIC. مساعدة في قائمة CLI الحالية `direct-only` كشجاعة.
- يجب على مجموعات SDK تحديد `OrchestratorConfig::with_transport_policy(TransportPolicy::DirectOnly)` مع عرض تبديل "الوضع المباشر" دائمًا. يتم إنشاء الروابط في `iroha::ClientOptions` و`iroha_android` في نفس التعداد.
- يمكن لأدوات البوابة (`sorafs_fetch`، روابط Python) أن تترجم أو تقوم بالتبديل بشكل مباشر فقط عبر المساعدين Norito JSON المقسمين حتى يتم استلامهم تلقائيًا أو نفس السلوك.

قم بتوثيق أو وضع علامة على دفاتر التشغيل المقسمة إلى أجزاء وقم بتمرير التبديلات عبر `iroha_config` أثناء تغيير البيئة.

## 2. بوابة الأداء السياسي

استخدم Norito JSON لمواصلة تكوين حتمية orquestrador. ملف النموذج المشفر `docs/examples/sorafs_direct_mode_policy.json`:

- `transport_policy: "direct_only"` - تم التحقق من المثبتين الذين أعلنوا عن عمليات نقل التتابع SoraNet.
- `max_providers: 2` - حدود الأقران المرتبطة بنقاط النهاية Torii/QUIC الأكثر ثقة. قم بضبط التوافق مع امتيازات الامتثال الإقليمية.
- `telemetry_region: "regulated-eu"` - قم بتدوير المقاييس المنبعثة لتمييز لوحات المعلومات والمراجع عن العمليات الاحتياطية.
- عمليات إعادة محاولة المحافظين (`retry_budget: 2`، `provider_failure_threshold: 3`) لتجنب إخفاء البوابات التي تم تكوينها بشكل خاطئ.قم بنقل JSON عبر `sorafs_cli fetch --config` (تلقائي) أو روابط SDK (`config_from_json`) قبل التصدير السياسي إلى المشغلين. استمر في لوحة النتائج (`persist_path`) لثلاثة مشاهدين.

مقابض التنفيذ التي يتم فتحها من البوابة موجودة في `docs/examples/sorafs_gateway_direct_mode.toml`. تم تخصيص القالب لـ `iroha app sorafs gateway direct-mode enable`، وإلغاء التحقق من المغلف/القبول، والاتصال بحدود المعدل الافتراضية ووضع `direct_mode` في الجدول مع أسماء المضيفين المشتقة من المخطط وملخصات البيان. استبدال قيم العنصر النائب بمخططك قبل الإصدار أو المهمة في إدارة التكوين.

## 3. مجموعة الخصيتين للامتثال

تشتمل بداية الأسلوب المباشر الآن على تغطية لمنسق ما بين صناديق CLI:- `direct_only_policy_rejects_soranet_only_providers` يضمن أن `TransportPolicy::DirectOnly` يفشل بسرعة عندما يتم الإعلان عن مرشح لذلك يدعم مرحلات SoraNet. [الصناديق/sorafs_orchestrator/src/lib.rs:7238]
- `direct_only_policy_prefers_direct_transports_when_available` يضمن نقل Torii/QUIC أثناء الاستخدام وترحيل SoraNet باستثناء الجلسة. [الصناديق/sorafs_orchestrator/src/lib.rs:7285]
- `direct_mode_policy_example_is_valid` faz parse de `docs/examples/sorafs_direct_mode_policy.json` لضمان استمرارية المستند من خلال المساعدين. [صناديق/sorafs_orchestrator/src/lib.rs:7509] [docs/examples/sorafs_direct_mode_policy.json:1]
- `fetch_command_respects_direct_transports` يتم إجراء `sorafs_cli fetch --transport-policy=direct-only` على بوابة Torii المحاكية، مما يؤدي إلى اختبار الدخان للبيئات المنظمة التي تعمل على تثبيت النقل المباشر. [الصناديق/sorafs_car/tests/sorafs_cli.rs:2733]
- يتضمن `scripts/sorafs_direct_mode_smoke.sh` نفس الأمر مع JSON السياسي واستمرارية لوحة النتائج للتشغيل التلقائي.

ركب جناحًا focada before publicar atualizacoes:

```bash
cargo test -p sorafs_orchestrator direct_only_policy
cargo test -p sorafs_car --features cli fetch_command_respects_direct_transports
```

إذا قمت بتجميع مساحة العمل بشكل خاطئ من خلال تعديل المنبع، فقم بالتسجيل أو حظر الخطأ في `status.md` ثم انتقل مرة أخرى عند تحديث التبعية.

## 4. تشغيل الدخان تلقائيًالا تكشف تغطية CLI عن تراجعات معينة في البيئة (على سبيل المثال، انجراف السياسة إلى البوابة أو اختلافات البيان). مساعد دخان مخصص يعيش في `scripts/sorafs_direct_mode_smoke.sh` ويشتمل على `sorafs_cli fetch` مع سياسة المنسق بطريقة مباشرة، واستمرارية لوحة النتائج، والتقاط السيرة الذاتية.

مثال على الاستخدام:

```bash
./scripts/sorafs_direct_mode_smoke.sh \
  --config docs/examples/sorafs_direct_mode_smoke.conf \
  --provider name=gw-regulated,provider-id=001122...,base-url=https://gw.example/direct/,stream-token=BASE64
```- يستجيب البرنامج النصي لعلامات CLI وملفات التكوين key=value (veja `docs/examples/sorafs_direct_mode_smoke.conf`). احصل على ملخص البيان وإدخالات إعلان مقدم الخدمة مع قيم الإنتاج المسبق.
- `--policy` هو عبارة عن `docs/examples/sorafs_direct_mode_policy.json`، لكن أي JSON من مُنتج المنتج `sorafs_orchestrator::bindings::config_to_json` يمكن أن يكون مطلوبًا. عند استخدام CLI للسياسة عبر `--orchestrator-config=PATH`، يتم تشغيل إعادة إنتاج الأعلام بدون تعديل يدويًا.
- عندما `sorafs_cli` ليس هذا هو `PATH`، أو المساعد في تجميع جزء من الصندوق `sorafs_orchestrator` (إصدار الملف) حتى يتمكن المدخنون من ممارسة السباكة بطريقة مباشرة.
- سعيداس:
  - الحمولة مونتادو (`--output`, بادراو `artifacts/sorafs_direct_mode/payload.bin`).
  - ملخص الجلب (`--summary`، مسار الحمولة) يتنافس على منطقة القياس عن بعد وعلاقات المثبتين المستخدمة كدليل على الطرح.
  - لقطة لوحة النتائج مستمرة في المسار المعلن في JSON السياسي (على سبيل المثال، `fetch_state/direct_mode_scoreboard.json`). قم بالرجوع إلى السيرة الذاتية في تذاكر السفر.- بوابة الإعلانات التلقائية: بعد الجلب، أو استدعاء المساعد `cargo xtask sorafs-adoption-check` باستخدام سجلات لوحة النتائج والملخص. النصاب القانوني المطلوب للإدارة ورقم المحامين على خط القيادة؛ sobrescreva com `--min-providers=<n>` عندما يتطلب الأمر أكثر من ذلك. يمكن لروابط المراسلات المرتبطة بمتابعة السيرة الذاتية (`--adoption-report=<path>` تحديد التخصيص المحلي) والمساعدة في تمرير `--require-direct-only` من خلال المسار (الاستمرار في الرجوع إلى الخلف) و`--require-telemetry` مع الاستمرار في الإشارة إلى المراسل. استخدم `XTASK_SORAFS_ADOPTION_FLAGS` لإعادة تمرير الوسيطات الإضافية للمهام (على سبيل المثال `--allow-single-source` أثناء الرجوع إلى الإصدار السابق للموافقة على السماح بالبوابة والتأثير على الإجراء الاحتياطي). لذا، فإن بوابة com `--skip-adoption-check` هي أداة تشخيصية للمكان؛ تتطلب خارطة الطريق أن يتم تشغيل كل شيء بشكل منظم بطريقة مباشرة، بما في ذلك حزمة من علاقات الدعم.

## 5. قائمة التحقق من الطرح1. **تجميد التكوين:** تخزين أو ملف JSON بطريقة مباشرة على المستودع `iroha_config` وتسجيل التجزئة بدون تذكرة تغيير.
2. **مراجعة البوابة:** تأكد من أن نقاط النهاية Torii تنطبق على TLS وTLVs وتسجيل المستمعين قبل الظهور بالطريقة المباشرة. نشر الملف السياسي عبر بوابة المشغلين.
3. **تسجيل الخروج من الامتثال:** مشاركة قواعد اللعبة التي تم تحديثها مع مراجعات الامتثال/اللوائح التنظيمية والتقاط الموافقات لتشغيل تراكب مجهول.
4. **التشغيل الجاف:** تنفيذ مجموعة من الامتثال لجلب التدريج مقابل إثباتات Torii الموثوقة. أرشفة مخرجات لوحة النتائج والاستئنافات من CLI.
5. ** نقل المنتج: ** الإعلان عن تاريخ التعديل، وتغيير `transport_policy` لـ `direct_only` (إذا تم اختياره لـ `soranet-first`) ومراقبة لوحات المعلومات بالطريقة المباشرة (زمن الاستجابة لـ `sorafs_fetch`، ووحدات التحكم فالها دي بروفيدوريس). قم بتوثيق خطة التراجع للرجوع إلى SoraNet-first عند SNNet-4/5/5a/5b/6a/7/8/12/13 تدريجيًا على `roadmap.md:532`.
6. **مراجعة وضع التعديل:** لقطات مرفقة من لوحة النتائج واسترجاع الجلب ونتائج المراقبة على تذكرة التعديل. قم بتنشيط `status.md` مع فعالية البيانات وأي شذوذ.احتفظ بقائمة التحقق إلى جانب دليل التشغيل `sorafs_node_ops` حتى يتمكن المشغلون من البدء في التدفق مسبقًا إلى حياة حية في الحياة. عند استخدام SNNet-5 في GA، قم بالانسحاب أو التراجع بعد تأكيد عملية قياس الإنتاج عن بعد.

## 6. متطلبات الأدلة وبوابة الدعوى

تم التقاط الصور بطريقة مباشرة مما يرضي بوابة Adocao SF-6c. حزمة لوحة النتائج، أو السيرة الذاتية، أو مغلف البيان، أو علاقة الدعم في كل مرة يتم فيها تشغيل `cargo xtask sorafs-adoption-check` لوضعية احتياطية. يجب أن يكون المجال مفتوحًا أو بوابة مفتوحة، أو تسجيل البيانات الوصفية التي تنتظرنا.- **انتقالات النقل:** `scoreboard.json` يجب الإعلان عن `transport_policy="direct_only"` (ويظهر `transport_policy_override=true` عند التحدث عن الرجوع إلى إصدار سابق). الحفاظ على المجال السياسي المجهول في نفس الوقت الذي نتخذ فيه الإجراءات الافتراضية حتى يتعين على المراجعين أن يصمموا خطة مجهولة في المراحل.
- **مثبتات المعالجات:** جلسات البوابة فقط هي استمرار `provider_count=0` وحذف `gateway_provider_count=<n>` مع رقم المثبت Torii المستخدم. تجنب تحرير JSON يدويًا: o CLI/SDK واشتقاقها من العدوى، وستعيد بوابة adocao التقاط اللقطات التي تحذفها بشكل منفصل.
- **دليل البيان:** عندما تشارك البوابات Torii، قم بتمرير `--gateway-manifest-envelope <path>` (أو ما يعادلها في SDK) حتى يتم تسجيل `gateway_manifest_provided` أكثر من `gateway_manifest_id`/`gateway_manifest_cid` `scoreboard.json`. ضمان أن `summary.json` يحمل أو نفس `manifest_id`/`manifest_cid`; يتم حذف أي ملف على قدم المساواة من خلال فحص adocao falha.
- **توقعات القياس عن بعد:** عندما يرافق القياس عن بعد الالتقاط، قم بالقيادة عبر البوابة عبر `--require-telemetry` لكي يثبت الارتباط قياسات الانبعاث. يمكن لـ Ensaios air-gapped أن تحذف العلم، ولكن يجب أن تقوم CI وتذاكر السفر بتوثيق الترخيص.

مثال:

```bash
cargo xtask sorafs-adoption-check \
  --scoreboard fetch_state/direct_mode_scoreboard.json \
  --summary fetch_state/direct_mode_summary.json \
  --allow-single-source \
  --require-direct-only \
  --json-out artifacts/sorafs_direct_mode/adoption_report.json \
  --require-telemetry
```الملحق `adoption_report.json` جنبًا إلى جنب مع لوحة النتائج، والملخص، ومغلف البيان، وحزمة سجلات الدخان. توضح هذه الأعمال الفنية ما هي وظيفة adocao em CI (`ci/check_sorafs_orchestrator_adoption.sh`) التي يتم تطبيقها وتمنع التخفيض من طريقة التدقيق المباشر.