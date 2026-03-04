---
lang: ar
direction: rtl
source: docs/source/agents/env_var_migration.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c9ce6010594e495116c1397b984000d1ee5d45d064294eca046f8dc762fa73b6
source_last_modified: "2026-01-04T10:50:53.607349+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Env → Config Migration Tracker

يلخص هذا المتعقب مفاتيح التبديل المتغيرة للبيئة التي تواجه الإنتاج والتي ظهرت على السطح
بواسطة `docs/source/agents/env_var_inventory.{json,md}` والترحيل المقصود
المسار إلى `iroha_config` (أو تحديد النطاق الصريح للتطوير/الاختبار فقط).


ملاحظة: يفشل `ci/check_env_config_surface.sh` الآن عند ظهور بيئة **production** جديدة
تظهر الحشوات بالنسبة إلى `AGENTS_BASE_REF` ما لم يكن `ENV_CONFIG_GUARD_ALLOW=1` كذلك
مجموعة؛ قم بتوثيق الإضافات المتعمدة هنا قبل استخدام التجاوز.

## الهجرات المكتملة- **IVM إلغاء الاشتراك في ABI** — تمت إزالة `IVM_ALLOW_NON_V1_ABI`؛ يرفض المترجم الآن
  واجهات برمجة التطبيقات غير v1 دون قيد أو شرط مع اختبار وحدة يحمي مسار الخطأ.
- **IVM debugbanner env shim** — تم إسقاط خيار إلغاء الاشتراك `IVM_SUPPRESS_BANNER` env؛
  يظل منع الشعارات متاحًا عبر أداة الضبط البرمجي.
- **IVM ذاكرة التخزين المؤقت/تحجيم الحجم** — تغيير حجم ذاكرة التخزين المؤقت/المثبت/وحدة معالجة الرسومات المترابطة
  `iroha_config` (`pipeline.{cache_size,ivm_cache_max_decoded_ops,ivm_cache_max_bytes,ivm_prover_threads}`،
  `accel.max_gpus`) وإزالة حشوات البيئة الخاصة بوقت التشغيل. يتصل المضيفون الآن
  `ivm::ivm_cache::configure_limits` و`ivm::zk::set_prover_threads`، استخدام الاختبارات
  `CacheLimitsGuard` بدلاً من تجاوزات env.
- **جذر قائمة انتظار الاتصال** — تمت إضافة `connect.queue.root` (الافتراضي:
  `~/.iroha/connect`) إلى تكوين العميل وربطه عبر CLI و
  تشخيصات JS. يقوم مساعدو JS بحل التكوين (أو `rootDir` الصريح) و
  تكريم `IROHA_CONNECT_QUEUE_ROOT` فقط في التطوير/الاختبار عبر `allowEnvOverride`؛
  توثق القوالب المقبض بحيث لم يعد المشغلون بحاجة إلى تجاوزات البيئة.
- **اشتراك الشبكة Izanami** — تمت إضافة علامة `allow_net` CLI/config صريحة لـ
  أداة الفوضى Izanami؛ تتطلب عمليات التشغيل الآن `allow_net=true`/`--allow-net` و
- **IVM البانر beep** — تم استبدال شريحة `IROHA_BEEP` env بأخرى تعتمد على التكوين
  تبديل `ivm.banner.{show,beep}` (الافتراضي: صحيح/صحيح). شعار بدء التشغيل/صافرة
  تقرأ الأسلاك الآن التكوين فقط في الإنتاج؛ لا يزال بناء dev/test يحترم
  تجاوز env للتبديل اليدوي.
- **تجاوز التخزين المؤقت DA (الاختبارات فقط)** — تم الآن تجاوز `IROHA_DA_SPOOL_DIR`
  مسيجة خلف مساعدي `cfg(test)`؛ رمز الإنتاج دائمًا هو مصدر التخزين المؤقت
  المسار من التكوين
- **مبادئ التشفير** — تم استبدال `IROHA_DISABLE_SM_INTRINSICS` /
  `IROHA_ENABLE_SM_INTRINSICS` مع التكوين
  سياسة `crypto.sm_intrinsics` (`auto`/`force-enable`/`force-disable`) و
  إزالة الحارس `IROHA_SM_OPENSSL_PREVIEW`. يطبق المضيفون السياسة على
  بدء التشغيل، يمكن للمقاعد/الاختبارات الاشتراك عبر `CRYPTO_SM_INTRINSICS` وOpenSSL
  تحترم المعاينة الآن علامة التكوين فقط.
  يتطلب Izanami بالفعل تكوين `--allow-net`/المستمر، وتعتمد الاختبارات الآن على
  هذا المقبض بدلاً من تبديل البيئة المحيطة.
- **ضبط وحدة معالجة الرسومات FastPQ** — تمت إضافة `fastpq.metal.{max_in_flight,threadgroup_width,metal_trace,metal_debug_enum,metal_debug_fused}`
  مقابض التكوين (الافتراضية: `None`/`None`/`false`/`false`/`false`) وقم بربطها من خلال تحليل CLI
  تعمل الحشوات `FASTPQ_METAL_*` / `FASTPQ_DEBUG_*` الآن كإجراءات احتياطية للتطوير/الاختبار و
  يتم تجاهلها بمجرد تحميل التكوين (حتى عندما يتركها التكوين بدون ضبط)؛ المستندات/المخزون كانت
  تم التحديث للإشارة إلى الترحيل.
  (`IVM_DECODE_TRACE`، `IVM_DEBUG_WSV`، `IVM_DEBUG_COMPACT`، `IVM_DEBUG_INVALID`،
  `IVM_DEBUG_REGALLOC`، `IVM_DEBUG_METAL_ENUM`، `IVM_DEBUG_METAL_SELFTEST`،
  `IVM_FORCE_METAL_ENUM`، `IVM_FORCE_METAL_SELFTEST_FAIL`، `IVM_FORCE_CUDA_SELFTEST_FAIL`،
  `IVM_DISABLE_METAL`، `IVM_DISABLE_CUDA`) أصبحت الآن مسورة خلف عمليات إنشاء التصحيح/الاختبار عبر شبكة مشتركة
  المساعد لذا تتجاهلها ثنائيات الإنتاج مع الحفاظ على المقابض للتشخيصات المحلية. البيئة
  تم إعادة إنشاء المخزون ليعكس نطاق التطوير/الاختبار فقط.- **تحديثات تركيبات FASTPQ** — يظهر `FASTPQ_UPDATE_FIXTURES` الآن فقط في تكامل FASTPQ
  الاختبارات؛ لم تعد مصادر الإنتاج تقرأ مفتاح التبديل env ويعكس المخزون الاختبار فقط
  النطاق.
- **تحديث المخزون + اكتشاف النطاق** — تقوم أدوات مخزون env الآن بوضع علامة على ملفات `build.rs` على أنها
  بناء النطاق وتتبع وحدات تسخير `#[cfg(test)]`/تكامل بحيث يتم التبديل للاختبار فقط (على سبيل المثال،
  `IROHA_TEST_*`، `IROHA_RUN_IGNORED`) وتظهر علامات بناء CUDA خارج عدد الإنتاج.
  تم تجديد المخزون في 07 ديسمبر 2025 (518 مرجعًا / 144 فارًا) للحفاظ على اختلاف حارس env-config باللون الأخضر.
- ** طوبولوجيا P2P env واقي تحرير الرقائق ** — `IROHA_P2P_TOPOLOGY_UPDATE_MS` يقوم الآن بتشغيل حتمية
  خطأ في بدء التشغيل في إصدارات الإصدار (تحذير فقط في التصحيح/الاختبار) لذا تعتمد عقد الإنتاج فقط على
  `network.peer_gossip_period_ms`. تم تجديد مخزون البيئة ليعكس الحارس و
  يقوم المصنف المحدث الآن بنطاقات التبديل المحمية بـ `cfg!` كتصحيح/اختبار.

## عمليات الترحيل ذات الأولوية العالية (مسارات الإنتاج)

- _لا شيء (تم تحديث المخزون باستخدام cfg!/اكتشاف التصحيح؛ وحارس env-config باللون الأخضر بعد تصلب طبقة P2P)._

## تبديل التطوير/الاختبار فقط إلى السياج

- عملية المسح الحالية (07 ديسمبر 2025): يتم تحديد نطاق إشارات CUDA المخصصة للإنشاء فقط (`IVM_CUDA_*`) على أنها `build` و
  مفاتيح التبديل (`IROHA_TEST_*`، `IROHA_RUN_IGNORED`، `IROHA_SKIP_BIND_CHECKS`) تسجل الآن كـ
  `test`/`debug` في المخزون (بما في ذلك الحشوات المحمية `cfg!`). ليس هناك حاجة لسياج إضافي.
  احتفظ بالإضافات المستقبلية خلف `cfg(test)`/مساعدي المقعد فقط مع علامات TODO عندما تكون الحشوات مؤقتة.

## بيئات وقت البناء (اتركها كما هي)

- بيئات الشحن/الميزات (`CARGO_*`، `OUT_DIR`، `DOCS_RS`، `PROFILE`، `CUDA_HOME`،
  `CUDA_PATH`، `JSONSTAGE1_CUDA_ARCH`، `FASTPQ_SKIP_GPU_BUILD`، وما إلى ذلك) تبقى
  مخاوف بناء البرنامج النصي وهي خارج نطاق ترحيل تكوين وقت التشغيل.

## الإجراءات التالية

1) قم بتشغيل `make check-env-config-surface` بعد تحديثات سطح التكوين لالتقاط حشوات بيئة الإنتاج الجديدة
   في وقت مبكر وتعيين مالكي النظام الفرعي/ETAs.  
2) قم بتحديث المخزون (`make check-env-config-surface`) بعد كل عملية مسح
   يظل جهاز التتبع متوافقًا مع حواجز الحماية الجديدة ويظل فرق حماية env-config خاليًا من الضوضاء.