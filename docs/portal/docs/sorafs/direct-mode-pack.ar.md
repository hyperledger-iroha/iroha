<!-- Auto-generated stub for Arabic (ar) translation. Replace this content with the full translation. -->

---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/direct-mode-pack.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 59d36561f60e28f8d7d5794e953755d3221daf8f0627e0be6a540115b0ad1f05
source_last_modified: "2025-11-20T08:09:37.293192+00:00"
translation_last_reviewed: 2025-12-30
---

---
id: direct-mode-pack
title: حزمة الرجوع للوضع المباشر في SoraFS ‏(SNNet-5a)
sidebar_label: حزمة الوضع المباشر
description: الإعدادات المطلوبة وفحوصات الامتثال وخطوات الإطلاق عند تشغيل SoraFS في وضع Torii/QUIC المباشر أثناء انتقال SNNet-5a.
---

:::note المصدر المعتمد
تعكس هذه الصفحة `docs/source/sorafs/direct_mode_pack.md`. احرص على إبقاء النسختين متزامنتين إلى أن يتم إيقاف مجموعة Sphinx القديمة.
:::

تظل دوائر SoraNet هي النقل الافتراضي لـ SoraFS، لكن بند خارطة الطريق **SNNet-5a** يتطلب مسار رجوع منظم كي يحافظ المشغلون على وصول قراءة حتمي بينما يكتمل طرح الخصوصية. تلتقط هذه الحزمة مفاتيح التحكم في CLI/SDK وملفات التهيئة واختبارات الامتثال وقائمة النشر اللازمة لتشغيل SoraFS في وضع Torii/QUIC المباشر دون لمس نواقل الخصوصية.

ينطبق مسار الرجوع على بيئات staging والإنتاج المنظم إلى أن تعبر SNNet-5 حتى SNNet-9 بوابات الجاهزية. احتفظ بالمواد أدناه مع حزمة نشر SoraFS المعتادة حتى يتمكن المشغلون من التبديل بين الوضعين المجهول والمباشر عند الطلب.

## 1. أعلام CLI و SDK

- `sorafs_cli fetch --transport-policy=direct-only ...` يعطل جدولة المرحلات ويفرض نقل Torii/QUIC. تعرض مساعدة CLI الآن `direct-only` كقيمة مقبولة.
- يجب على SDK ضبط `OrchestratorConfig::with_transport_policy(TransportPolicy::DirectOnly)` كلما عرض مفتاح تبديل "الوضع المباشر". تقوم الربوط المولدة في `iroha::ClientOptions` و `iroha_android` بتمرير نفس enum.
- يمكن لأدوات الـ gateway (`sorafs_fetch` وروابط Python) تحليل مفتاح direct-only عبر مساعدات Norito JSON المشتركة حتى تتلقى الأتمتة السلوك نفسه.

وثّق العلم في كتيبات التشغيل الموجهة للشركاء، ومرر مفاتيح التبديل عبر `iroha_config` بدلا من متغيرات البيئة.

## 2. ملفات سياسة الـ gateway

استخدم Norito JSON لحفظ إعداد منسق حتمي. ملف المثال في `docs/examples/sorafs_direct_mode_policy.json` يشفر:

- `transport_policy: "direct_only"` — يرفض المزوّدين الذين يعلنون فقط عن نقل مرحلات SoraNet.
- `max_providers: 2` — يحد الأقران المباشرين إلى أكثر نقاط Torii/QUIC موثوقية. عدّل وفق سماحات الامتثال الإقليمية.
- `telemetry_region: "regulated-eu"` — يوسم المقاييس المصدرة بحيث تميز لوحات المتابعة والتدقيق تشغيلات الرجوع.
- ميزانيات إعادة المحاولة المحافظة (`retry_budget: 2`, `provider_failure_threshold: 3`) لتجنب إخفاء بوابات سيئة الضبط.

حمّل JSON عبر `sorafs_cli fetch --config` (الأتمتة) أو ربط SDK (`config_from_json`) قبل عرض السياسة على المشغلين. احتفظ بمخرجات الـ scoreboard (`persist_path`) لمسارات التدقيق.

تُلتقط مفاتيح الإنفاذ على جانب الـ gateway في `docs/examples/sorafs_gateway_direct_mode.toml`. يعكس القالب مخرجات `iroha app sorafs gateway direct-mode enable`، مع تعطيل فحوصات envelope/admission، وتوصيل إعدادات rate-limit الافتراضية، وتعبئة جدول `direct_mode` بأسماء المضيفين المشتقة من الخطة وملخصات manifest. استبدل القيم النائبة بخطة الإطلاق قبل حفظ المقتطف في إدارة التهيئة.

## 3. مجموعة اختبارات الامتثال

تتضمن جاهزية الوضع المباشر الآن تغطية في المنسق وفي حزم CLI:

- `direct_only_policy_rejects_soranet_only_providers` يضمن أن `TransportPolicy::DirectOnly` يفشل بسرعة عندما يدعم كل advert مرشح مرحلات SoraNet فقط.【crates/sorafs_orchestrator/src/lib.rs:7238】
- `direct_only_policy_prefers_direct_transports_when_available` يضمن استخدام نقل Torii/QUIC عند توفره واستبعاد مرحلات SoraNet من الجلسة.【crates/sorafs_orchestrator/src/lib.rs:7285】
- `direct_mode_policy_example_is_valid` يحلل `docs/examples/sorafs_direct_mode_policy.json` لضمان بقاء الوثائق متوافقة مع أدوات المساعدة.【crates/sorafs_orchestrator/src/lib.rs:7509】【docs/examples/sorafs_direct_mode_policy.json:1】
- `fetch_command_respects_direct_transports` يختبر `sorafs_cli fetch --transport-policy=direct-only` أمام بوابة Torii وهمية، موفرا اختبار smoke لبيئات منظمة تثبت النقل المباشر.【crates/sorafs_car/tests/sorafs_cli.rs:2733】
- `scripts/sorafs_direct_mode_smoke.sh` يلف الأمر نفسه مع JSON السياسة وحفظ الـ scoreboard لأتمتة الإطلاق.

شغّل المجموعة المركزة قبل نشر التحديثات:

```bash
cargo test -p sorafs_orchestrator direct_only_policy
cargo test -p sorafs_car --features cli fetch_command_respects_direct_transports
```

إذا فشل تجميع الـ workspace بسبب تغييرات upstream، سجّل الخطأ الحاجز في `status.md` وأعد التشغيل عندما تلاحق التبعية.

## 4. تشغيلات smoke مؤتمتة

تغطية CLI وحدها لا تكشف الانحدارات الخاصة بالبيئة (مثل انحراف سياسة الـ gateway أو عدم تطابق manifests). يوجد مساعد smoke مخصص في `scripts/sorafs_direct_mode_smoke.sh` ويلف `sorafs_cli fetch` مع سياسة منسق الوضع المباشر وحفظ الـ scoreboard والتقاط الملخص.

مثال للاستخدام:

```bash
./scripts/sorafs_direct_mode_smoke.sh \
  --config docs/examples/sorafs_direct_mode_smoke.conf \
  --provider name=gw-regulated,provider-id=001122...,base-url=https://gw.example/direct/,stream-token=BASE64
```

- يحترم السكربت أعلام CLI وملفات الإعداد key=value معا (راجع `docs/examples/sorafs_direct_mode_smoke.conf`). املأ digest الخاص بالـ manifest ومدخلات adverts للموفّر بقيم الإنتاج قبل التشغيل.
- `--policy` افتراضيا `docs/examples/sorafs_direct_mode_policy.json`، لكن يمكن تمرير أي JSON منسق ينتجه `sorafs_orchestrator::bindings::config_to_json`. يقبل CLI السياسة عبر `--orchestrator-config=PATH` لتمكين تشغيلات قابلة لإعادة الإنتاج دون ضبط الأعلام يدويا.
- عندما لا يكون `sorafs_cli` على `PATH`، يبنيه المساعد من crate `sorafs_orchestrator` (ملف release) بحيث تمارس تشغيلات smoke مسار الوضع المباشر المرسل.
- المخرجات:
  - الحمولة المجمعة (`--output`، الافتراضي `artifacts/sorafs_direct_mode/payload.bin`).
  - ملخص الجلب (`--summary`، افتراضيا بجوار الحمولة) يحتوي على منطقة التليمترية وتقارير الموفّرين المستخدمة كدليل إطلاق.
  - لقطة scoreboard محفوظة في المسار المعلن في JSON السياسة (مثل `fetch_state/direct_mode_scoreboard.json`). أرشفها مع الملخص في تذاكر التغيير.
- أتمتة بوابة الاعتماد: بعد اكتمال الجلب يستدعي المساعد `cargo xtask sorafs-adoption-check` باستخدام مسارات scoreboard والملخص المحفوظة. الحد الأدنى للنصاب يساوي افتراضيا عدد الموفّرين الممررين في سطر الأوامر؛ استبدله بـ `--min-providers=<n>` عند الحاجة إلى عينة أكبر. تُكتب تقارير الاعتماد بجوار الملخص (`--adoption-report=<path>` يمكنه تحديد موقع مخصص) ويمرر المساعد `--require-direct-only` افتراضيا (مطابق لمسار الرجوع) و`--require-telemetry` عندما تمرر العلم المطابق. استخدم `XTASK_SORAFS_ADOPTION_FLAGS` لتمرير وسائط xtask إضافية (مثل `--allow-single-source` أثناء خفض معتمد حتى تتسامح البوابة مع الرجوع وتفرضه). لا تتجاوز بوابة الاعتماد باستخدام `--skip-adoption-check` إلا عند التشخيص المحلي؛ تتطلب خارطة الطريق أن تتضمن كل عملية تشغيل منظمة في الوضع المباشر حزمة تقرير الاعتماد.

## 5. قائمة تحقق الإطلاق

1. **تجميد التهيئة:** خزّن ملف JSON للوضع المباشر في مستودع `iroha_config` وسجل الهاش في تذكرة التغيير.
2. **تدقيق الـ gateway:** تحقق من أن نقاط Torii تطبق TLS وTLVs للقدرات وسجلات التدقيق قبل التحويل إلى الوضع المباشر. انشر ملف سياسة الـ gateway للمشغلين.
3. **موافقة الامتثال:** شارك دليل التشغيل المحدث مع مراجعي الامتثال/التنظيم وسجل الموافقات على التشغيل خارج طبقة الإخفاء.
4. **تشغيل تجريبي:** نفذ مجموعة اختبارات الامتثال بالإضافة إلى جلب staging مقابل مزودي Torii الموثوقين. أرشف مخرجات scoreboard وملخصات CLI.
5. **التحويل للإنتاج:** أعلن نافذة التغيير، وحول `transport_policy` إلى `direct_only` (إذا كنت قد اخترت `soranet-first`)، وراقب لوحات الوضع المباشر (زمن `sorafs_fetch`، وعدادات فشل المزوّدين). وثّق خطة الرجوع إلى SoraNet-first بعد تخرج SNNet-4/5/5a/5b/6a/7/8/12/13 في `roadmap.md:532`.
6. **مراجعة ما بعد التغيير:** أرفق لقطات scoreboard وملخصات الجلب ونتائج المراقبة في تذكرة التغيير. حدّث `status.md` بتاريخ السريان وأي شذوذات.

احتفظ بالقائمة بجوار دليل `sorafs_node_ops` حتى يتمكن المشغلون من التمرين قبل التحويل الفعلي. عند انتقال SNNet-5 إلى GA، أوقف مسار الرجوع بعد تأكيد التكافؤ في تليمترية الإنتاج.

## 6. متطلبات الأدلة وبوابة الاعتماد

يجب أن تفي لقطات الوضع المباشر ببوابة الاعتماد SF-6c. اجمع الـ scoreboard والملخص وenvelope الخاص بالـ manifest وتقرير الاعتماد لكل تشغيل حتى يتمكن `cargo xtask sorafs-adoption-check` من التحقق من وضع الرجوع. تؤدي الحقول الناقصة إلى فشل البوابة، لذا سجّل البيانات الوصفية المتوقعة في تذاكر التغيير.

- **بيانات النقل الوصفية:** يجب أن يصرح `scoreboard.json` بـ `transport_policy="direct_only"` (وتبديل `transport_policy_override=true` عندما تُجبر خفض المستوى). أبقِ حقول سياسة الإخفاء المزدوجة معبأة حتى عندما ترث القيم الافتراضية كي يرى المراجعون ما إذا انحرفت عن خطة الإخفاء المرحلية.
- **عدادات المزوّدين:** يجب أن تحفظ جلسات gateway-only `provider_count=0` وأن تملأ `gateway_provider_count=<n>` بعدد مزودي Torii المستخدمين. تجنب تعديل JSON يدويا: فـ CLI/SDK يستخرج العدادات، وبوابة الاعتماد ترفض اللقطات التي تُغفل الفصل.
- **دليل manifest:** عندما تشارك بوابات Torii، مرر `--gateway-manifest-envelope <path>` الموقع (أو ما يعادله في SDK) حتى يتم تسجيل `gateway_manifest_provided` و`gateway_manifest_id`/`gateway_manifest_cid` داخل `scoreboard.json`. تأكد من أن `summary.json` يحمل `manifest_id`/`manifest_cid` المطابق؛ تفشل عملية الاعتماد إذا غاب الزوج من أي ملف.
- **توقعات التليمترية:** عندما ترافق التليمترية الالتقاط، شغّل البوابة مع `--require-telemetry` حتى يثبت تقرير الاعتماد أن المقاييس أُرسلت. يمكن للتجارب المعزولة هوائيا أن تتجاوز العلم، لكن يجب على CI وتذاكر التغيير توثيق الغياب.

مثال:

```bash
cargo xtask sorafs-adoption-check \
  --scoreboard fetch_state/direct_mode_scoreboard.json \
  --summary fetch_state/direct_mode_summary.json \
  --allow-single-source \
  --require-direct-only \
  --json-out artifacts/sorafs_direct_mode/adoption_report.json \
  --require-telemetry
```

أرفق `adoption_report.json` مع الـ scoreboard والملخص وenvelope الخاص بالـ manifest وحزمة سجلات smoke. تعكس هذه القطع ما يطبقه عمل الاعتماد في CI (`ci/check_sorafs_orchestrator_adoption.sh`) وتبقي عمليات خفض الوضع المباشر قابلة للتدقيق.
