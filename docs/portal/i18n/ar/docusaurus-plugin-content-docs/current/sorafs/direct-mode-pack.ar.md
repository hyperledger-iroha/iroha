---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/direct-mode-pack.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: حزمة الوضع المباشر
العنوان: حزمة الرجوع للوضع المباشر في SoraFS ‏(SNNet-5a)
Sidebar_label: حزمة الوضع الخفيف
description: الإعدادات الأساسية والفحوصات الشاملة والخطوات الأساسية عند SoraFS في الوضع Torii/QUIC المباشر أثناء انتقال SNNet-5a.
---

:::ملحوظة المصدر مؤهل
احترام هذه الصفحة `docs/source/sorafs/direct_mode_pack.md`. احرص على جميع النسختين متزامنتين إلى أن يتم إيقاف تشغيل مجموعة Sphinx القديمة.
:::

وتظل متابعة SoraNet هي النقل الافتراضي لـ SoraFS، لكن بند خارطة الطريق **SNNet-5a** تتطلب مسار رجوع كي منظم وتتحكم في وصول القراءة حتمي بينما يكتمل تخطيط الخصوصية. تلتقط هذه المجموعة من مفاتيح التحكم في CLI/SDK فارغة الديناميكية وتتمتع برسومات بسيطة وقائمة النشر الضرورية لتشغيل SoraFS في وضع Torii/QUIC دون لمس نواقل الخصوصية.

يستخدم مسار الرجوع إلى بيئات التدريج والإنتاج المنظم إلى بوابات SNNet-5 الجاهزة حتى SNNet-9. ابعت بالمواد أدناه حزمة مع نشر SoraFS محددا حتى يبدأوا من الوضع بين الوضعين المجهول والمباشر عند الطلب.

## 1.أعلام CLI وSDK- `sorafs_cli fetch --transport-policy=direct-only ...` يعطل جدول المرحلات ويفترض Torii/QUIC. تم عرض مساعدة CLI الآن `direct-only` كقيمة مقبولة.
- يجب على ضبط SDK `OrchestratorConfig::with_transport_policy(TransportPolicy::DirectOnly)` كلما عرض مفتاح التعديل "الوضع المباشر". تقوم بتشغيل المولدة في `iroha::ClientOptions` و `iroha_android` بتمرير نفس التعداد.
- يمكن لأدوات الـ Gateway (`sorafs_fetch` وروابط Python) مفتاح تحليل مباشر فقط عبر مساعدات Norito JSON المشترك حتى يعرف التدرب على السلوك بنفسه.

وثيقة العلم في طابعة تشغيل التشغيل للشركاء، ومرر مفاتيح التبديل عبر `iroha_config` بدلات متنوعة من البيئة.

## 2. ملفات بناء الـ gate

استخدم Norito JSON لإعادة الإعداد منسق حتمي. مثال على ذلك في `docs/examples/sorafs_direct_mode_policy.json` يشفر:

- `transport_policy: "direct_only"` — رفض المتحكمين الذين أعلنوا فقط عن نقل رحلات SoraNet.
- `max_providers: 2` — نأمل الأقران المباشرين إلى أكثر نقاط Torii/QUIC موثوقة. وعدل سماحات جونسون.
- `telemetry_region: "regulated-eu"` — يوسم المعايير المصدرة بحيث يتم توزيع اللوحات متابعة ودقيقات الرجوع.
- إعادة قياسات المحاولة للحفاظ على (`retry_budget: 2`, `provider_failure_threshold: 3`) يمنع منع بوابات سيئة الضبط.

نتنياهو JSON عبر `sorafs_cli fetch --config` (المحاسبة) أو ربط SDK (`config_from_json`) قبل عرض السياسة على التشغيلين. تستخدم بمخرجات الـ لوحة النتائج (`persist_path`) لمارات التدقيق.تُخطِّط مفاتيح النار على جانب البوابة في `docs/examples/sorafs_gateway_direct_mode.toml`. عاكس إلكترونيات `iroha app sorafs gateway direct-mode enable`، مع فحوصات فحوصات المغلف/القبول، توصيل إعدادات معدل الحد الافتراضي، وجدول التعبئة `direct_mode` بأسماء المشتقة من بناء وملخصات البيان. استبدل القيم النائبة لسبب مختلف قبل حفظ المقتطف في إدارة التهيئة.

## 3. مجموعة السيولة

تتضمن استعدادية الوضع المباشر الآن في المنسق وفي حزم CLI:

- `direct_only_policy_rejects_soranet_only_providers` يضمن أن `TransportPolicy::DirectOnly` يفشل بسرعة عندما يدعم كل إعلان مرشح رحلات SoraNet فقط.[crates/sorafs_orchestrator/src/lib.rs:7238]
- `direct_only_policy_prefers_direct_transports_when_available` ضمان استخدام نقل Torii/QUIC عند توفره تقنية الابعاد مرحلات SoraNet من الجلسة.【crates/sorafs_orchestrator/src/lib.rs:7285】
- `direct_mode_policy_example_is_valid` يحلل `docs/examples/sorafs_direct_mode_policy.json` ودائمًا بقاء الروابط متوافقة مع أدوات المساعدة.
- `fetch_command_respects_direct_transports` يختبر `sorafs_cli fetch --transport-policy=direct-only` أمام بوابة Torii الزائفة، موفرا دخان لبيئات اختبار منظمة تثبت التغير المباشر.[crates/sorafs_car/tests/sorafs_cli.rs:2733]
- `scripts/sorafs_direct_mode_smoke.sh` يلف الأمر نفسه مع JSON سياسة حفظ الـ لوحة النتائج لغسل البيانات.

شغّل المجموعة المركزية قبل نشر التحديثات:

```bash
cargo test -p sorafs_orchestrator direct_only_policy
cargo test -p sorafs_car --features cli fetch_command_respects_direct_transports
```

إذا فشلت مساحة العمل بسبب تجميع الـ عوامل المنبع، فاعتبر خطأ الدولار في `status.md` وأعد التشغيل عندما تلاحق التبعية.

##4.تشغيلات الدخان مؤتمتةتغطية CLI وحدها لا تذكر الشارات الخاصة بالبيئة (مثل انحراف لمتابعة الـ البوابة أو عدم تطابق البيانات). يوجد مساعد دخان مخصص في `scripts/sorafs_direct_mode_smoke.sh` ويلف `sorafs_cli fetch` مع هيكل مخصص لحفظ الـ لوحة النتائج والقاطع الملخص.

مثال للاستخدام:

```bash
./scripts/sorafs_direct_mode_smoke.sh \
  --config docs/examples/sorafs_direct_mode_smoke.conf \
  --provider name=gw-regulated,provider-id=001122...,base-url=https://gw.example/direct/,stream-token=BASE64
```- يحترم السكربت أعلام CLI الحجم الكبير key=value معادل (راجع `docs/examples/sorafs_direct_mode_smoke.conf`). املأ دايجست الخاص بالـ المانيفست ومدخلات الإعلانات للمتمتع بقيم الإنتاج قبل التشغيل.
- `--policy` افتراضيا `docs/examples/sorafs_direct_mode_policy.json`، لكن لا يمكن أن يؤدي أي JSON منسقه إلى `sorafs_orchestrator::bindings::config_to_json`. تقبل CLI السياسة عبر `--orchestrator-config=PATH` وتدير تشغيلات قابلة لإعادة الإنتاج دون ضبط الأعلام اليدوية.
- عندما لا يكون `sorafs_cli` على `PATH`، يبنيه المساعد من الصندوق `sorafs_orchestrator` (ملف الافراج) بحيث يتم تشغيل مسار الدخان الوضع المرسل المباشر.
- المخرجات:
  - الحمولة المجمعة (`--output`، افتراضي `artifacts/sorafs_direct_mode/payload.bin`).
  - ملخص الجلب (`--summary`، افتراضيا بجوار الحمولة) يحتوي على منطقة التليميترية وتقارير المحرمين المستخدم كدليل تحريري.
  - لقطة لوحة النتائج محفوظة في المسار البائع في JSON السياسة (مثل `fetch_state/direct_mode_scoreboard.json`). أشفها مع الملخص في تذاكر التغيير.- الاعتماد على بوابة الاعتماد: بعد الجلب ويستدعي المساعدة `cargo xtask sorafs-adoption-check` باستخدام لوحة النتائج والملخص المحفوظة. الحد الأدنى للحصول على صفر يساوي عددًا افتراضيًا من المتوفرين في سطرين؛ تم استبداله بـ `--min-providers=<n>` عند الحاجة إلى تغيير أكبر. تُكتب تقارير الاعتماد بجوار الملخص (`--adoption-report=<path>` ويمكن تحديد موقع مخصص) ويمرر المساعدة `--require-direct-only` افتراضيا (وفقًا لمسار الرجوع) و`--require-telemetry` عندما تمرر العلم وفقًا لذلك. استخدم `XTASK_SORAFS_ADOPTION_FLAGS` لتمرير وسائط xtask الإضافية (مثل `--allow-single-source` أثناء المصادقة المعتمدة حتى تتسامح مع البوابة مع الرجوع وفرضه). لا تتجاوز بوابة الاعتماد باستخدام `--skip-adoption-check` إلا ​​عند التشخيص المحلي؛ تتطلب عملية خارطة الطريق أن تتضمن كل عملية منظمة في الوضع الدقيق لتقرير الاعتماد.

## 5. قائمة التحقق من كل شيء1. ** تجميد الديناميكية:** خزّن ملف JSON للوضع المباشر في مستودع `iroha_config` ويكتشف الهاش في تذكرة التغيير.
2. **تدقيق الـ gate:** تحقق من أن نقاط Torii تطبق TLS وTLVs للقدرات وتمييزات التمييز قبل تحويل الوضع البسيط. انشر ملف ابو الـ gate للمشغلين.
3. **موافقة تماما:** شارك دليل العمال المحدث مع المراجعين/التنظيم وأكدوا موافقتهم على إخفاء الملابس.
4. **تشغيل السوق:** ينفذ مجموعة السيولة بالإضافة إلى جذب التدريج مقابل Torii الموثوقين. أشف مخرجات لوحة النتائج وملخصات CLI.
5. **التحويل للإنتاج:** شركة نافذة الغد، `transport_policy` إلى `direct_only` (إذا كنت قد اخترت `soranet-first`)، وراقب لوحات الوضع المباشر (زمن `sorafs_fetch`، وعدادات فشل حول المتحكمين). وثيق الخطة الرجوع إلى SoraNet-first بعد التخرج SNNet-4/5/5a/5b/6a/7/8/12/13 في `roadmap.md:532`.
6. **مراجعة ما بعد التغيير:** أرفق لقطات لوحة النتائج وملخصات الجلب ونتائج المراقبة في تذكرة التغيير. تحديث `status.md` بتاريخ السرياني وأي شذوذات.

احتفظ بالقائمة بجوار دليل `sorafs_node_ops` حتى يبدأ العمل من خلال التغيير الفعال. عند انتقال SNNet-5 إلى GA، توقف عن مسار الرجوع بعد تأكيد التعاون في الإنتاج التليمتري.

## 6. الأدلة الأدلة وبوابة الاعتماديجب أن تفي لقطات الوضع المباشر ببوابة الاعتماد SF-6c. اجمع الـ لوحة النتائج والملخص والمغلف الخاص بالـ البيان وتقرير الاعتماد لكل تشغيل حتى `cargo xtask sorafs-adoption-check` من التحقق من وضع الرجوع. الشركة الناقصة إلى فشل البوابة، لذا سجل البيانات الوصفية لحساب في تذكرة التغيير.

- **بيانات النقل الوصفية:** يجب أن يصرح `scoreboard.json` بـ `transport_policy="direct_only"` (وتبديل `transport_policy_override=true` عندما تُجبر تراجع المستوى). أبقِ ممتلئة بإخفاء الملابس بالكامل حتى عندما ترث قيماً افتراضية كي ترى المراجعون ما إذا انحرفت عن خطة الإخفاء المرح.
- **عدادات المتحكمين:** يجب أن تحفظ جلسات البوابة فقط `provider_count=0` وأن تملأ `gateway_provider_count=<n>` للمزيد من المستخدمين Torii. تجنب تعديل JSON يدويا: فـ CLI/SDK يستخرج العدادات، وبوابة الاعتماد لا ترفض التي تُغفل الفصل.
- **دليل الدليل:** عندما تشارك بوابات Torii، مرر `--gateway-manifest-envelope <path>` الموقع (أو ما نفعه في SDK) حتى يتم تسجيل `gateway_manifest_provided` و`gateway_manifest_id`/`gateway_manifest_cid` داخل `scoreboard.json`. تأكد من أن `summary.json` عصر `manifest_id`/`manifest_cid` المطابق؛ يعتمد الفشل في نجاح الزوج على أي ملف.
- **توقعات التليمترية:** عندما ترافق التليمترية الالتقاط، شغّل البوابة مع `--require-telemetry` حتى يثبت اعتماد الاعتماد على المقاييس أُرسلت. يمكن للتجارب المعزولة هوائيا أن تتجاوز العلم، لكن يجب على CI وتذاكر التغيير الغياب.

مثال:

```bash
cargo xtask sorafs-adoption-check \
  --scoreboard fetch_state/direct_mode_scoreboard.json \
  --summary fetch_state/direct_mode_summary.json \
  --allow-single-source \
  --require-direct-only \
  --json-out artifacts/sorafs_direct_mode/adoption_report.json \
  --require-telemetry
```أرفق `adoption_report.json` مع الـ لوحة النتائج والملخص والمغلف الخاص بالـ بيان وزمة سجلات الدخان. مراعاة هذه القطع ما يطبقه عمل الاعتماد في CI (`ci/check_sorafs_orchestrator_adoption.sh`) وتبقي العمليات تقلل من الحجم الدقيق.