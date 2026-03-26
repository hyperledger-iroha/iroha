---
lang: ar
direction: rtl
source: docs/portal/docs/nexus/nexus-elastic-lane.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: حارة العلاقة المرنة
العنوان: ممر الطيران المرن (NX-7)
Sidebar_label: حارة مرنة
الوصف: عملية Bootstrap لإنشاء البيان حارة Nexus، وكتابة الكتالوج وطرح التدوين.
---

:::note Канонический источник
هذا الجزء يعرض `docs/source/nexus_elastic_lane.md`. قم بالنسخ المتزامن حتى لا يتم نشر التحويلات الكاملة في البوابة.
:::

# صنع معدات لممر السباق المرن (NX-7)

> **خريطة الطريق الدقيقة:** NX-7 - أدوات حارة القيادة المرنة  
> **الحالة:** الأدوات المجهزة - البيانات العامة، كتالوج الأجزاء، الحمولات Norito، اختبارات الدخان،
> مساعد لحزمة اختبار التحميل للحصول على نتائج جيدة في وقت استجابة الفتحة + البيانات الموثقة للتحقق من صحة التقدم
> يمكن نشره بدون نصوص برمجية عادية.

هذا هو دليل المشغلين عبر المساعد الجديد `scripts/nexus_lane_bootstrap.sh`، الذي يقوم تلقائيًا بإنشاء مسار البيان، جزء من حارة الكتالوج/مساحة البيانات والإعلان عن الطرح. يمكنك بسهولة تحديث الممرات Nexus الجديدة (العامة أو الخاصة) دون إجراء تعديلات بسيطة على بعض الملفات ودون الحاجة إلى نقل بسيط كتالوج هندسي.

## 1. النقل المسبق

1. الحوكمة للاسم المستعار حارة ومساحة البيانات وعدد من المدققين والتسامح مع الخطأ (`f`) والتسوية السياسية.
2. المدقق النهائي للقائمة (معرفات الحساب) ومساحات الأسماء المحمية بالقائمة.
3. قم بتثبيت إعدادات تكوين المستودع لإضافة الأجزاء المنسقة.
4. الدخول إلى ممر البيانات (sm.`nexus.registry.manifest_directory` و`cache_directory`).
5. مقابض جهات الاتصال عن بعد/PagerDuty للممر، بحيث يتم إضافة التنبيهات مرة أخرى عبر الإنترنت.

## 2. قم بتهيئة حارة القطع الأثرية

تثبيت المساعد من المستودع الأساسي:

```bash
scripts/nexus_lane_bootstrap.sh \
  --lane-alias "Payments Lane" \
  --lane-id 3 \
  --dataspace-alias payments \
  --governance-module parliament \
  --settlement-handle xor_global \
  --validator <i105-account-id> \
  --validator <i105-account-id> \
  --validator <i105-account-id> \
  --protected-namespace payments \
  --description "High-throughput interbank payments lane" \
  --dataspace-description "Payments dataspace" \
  --route-instruction finance::pacs008 \
  --encode-space-directory \
  --space-directory-out artifacts/nexus/payments_lane/payments.manifest.to \
  --telemetry-contact payments-ops@sora.org \
  --output-dir artifacts/nexus/payments_lane
```

أعلام رئيسية:

- يتم توصيل `--lane-id` بفهرس جديد مكتوب في `nexus.lane_catalog`.
- يقوم `--dataspace-alias` و`--dataspace-id/hash` بإدارة مساحة البيانات الموجودة في الكتالوج (من خلال استخدام مسار المعرف).
- `--validator` يمكن القراءة أو القراءة من `--validators-file`.
- `--route-instruction` / `--route-account` يحوّل البيانات إلى التسويق الصحيح.
- `--metadata key=value` (أو `--telemetry-contact/channel/runbook`) لإصلاح دليل جهات الاتصال الذي يوضح لوحة المعلومات للسائقين المناسبين.
- `--allow-runtime-upgrades` + `--runtime-upgrade-*` يضيف ترقية وقت التشغيل في البيان، عندما تحتاج إلى التحكم في المشغلين الرئيسيين.
- `--encode-space-directory` يتم الاتصال التلقائي `cargo xtask space-directory encode`. استخدم `--space-directory-out` مباشرةً إذا كنت ترغب في استخدام الترميز `.to` في مكان آخر.

يقوم البرنامج النصي بإنشاء ثلاث قطع أثرية في `--output-dir` (من خلال الكتالوج النهائي) بالإضافة إلى التشفير الاختياري باستخدام الترميز المضمن:

1.`<slug>.manifest.json` - مسار البيان مع مدقق النصاب ومساحات الأسماء المحددة والترقية الاختيارية لوقت التشغيل.
2. `<slug>.catalog.toml` - جزء TOML مع `[[nexus.lane_catalog]]` و`[[nexus.dataspace_catalog]]` والمخططات النهائية. يرجى ملاحظة أن `fault_tolerance` يتم تسجيله في مساحة البيانات لتعيين لجنة ترحيل المسار (`3f+1`).
3. `<slug>.summary.json` - تدقيق المياه بالقياس الهندسي (الحلقة، والأجزاء، والقطع)، ونقاط الطرح والأمر الدقيق `cargo xtask space-directory encode` (تحت `space_directory_encode.command`). استخدم هذا JSON للتذكرة على متن الطائرة كوسيلة للتوصيل.
4.`<slug>.manifest.to` - يتم الوصول إليه من خلال `--encode-space-directory`; GOTOV للطريق Torii `iroha app space-directory manifest publish`.

استخدم `--dry-run` لتتمكن من استخدام JSON/Fragments بدون الملفات المضغوطة، و`--force` لتلخيص الإعدادات قطعة أثرية.

## 3. تحسين الحجم

1. انسخ بيان JSON في `nexus.registry.manifest_directory` (وفي دليل ذاكرة التخزين المؤقت، في حالة تسجيل الحزم الإضافية). قم بتعبئة الملف إذا تم وضع بيانات الإصدار في مستودعات التكوين.
2. قم بإضافة كتالوج الأجزاء إلى `config/config.toml` (أو إلى `config.d/*.toml`). تأكد من أن `nexus.lane_count` لا يذكر `lane_id + 1`، واكتشف `nexus.routing_policy.rules`، التي يجب أن تنتقل إلى حارة جديدة.
3. قم بالتشفير (إذا تم طرح `--encode-space-directory`) ونشر البيان في Space Directory، باستخدام الأمر الملخص (`space_directory_encode.command`). إنه ينشئ `.manifest.to`، الذي يتوافق مع Torii، ويثبت الإحالة لمراجعي الحسابات؛ قم بالتحرك عبر `iroha app space-directory manifest publish`.
4. قم بتثبيت `irohad --sora --config path/to/config.toml --trace-config` وأرشفة تتبع البيانات عند طرح التذكرة. وهذا يؤكد أن الهندسة الجديدة تشجع على سبيكة/كورا بشكل هندسي.
5. المدققون المعتمدون على المسار بعد إعادة تغيير البيان/الكتالوج. ملخص ملخص JSON في تذكرة لمراجعي الحسابات.

## 4. سجل توزيع الحزمة الذكية

قم بتثبيت البيانات والتراكبات الهندسية بحيث يمكن للمشغلين توسيع إدارة البيانات عبر الممرات دون التكوينات الصحيحة للمضيف. يقوم المساعد بنسخ البيانات بالتخطيط القياسي، وإنشاء تراكب كتالوج الإدارة الاختياري لـ `nexus.registry.cache_directory`، ويمكنك اختيار كرة القطران من أجل ذلك الركاب غير المتصلين بالإنترنت:

```bash
scripts/nexus_lane_registry_bundle.sh \
  --manifest artifacts/nexus/payments_lane/payments.manifest.json \
  --output-dir artifacts/nexus/payments_lane/registry_bundle \
  --default-module parliament \
  --module name=parliament,module_type=parliament,param.quorum=2 \
  --bundle-out artifacts/nexus/payments_lane/registry_bundle.tar.gz
```

الخروج:

1.`manifests/<slug>.manifest.json` - انسخها إلى `nexus.registry.manifest_directory`.
2.`cache/governance_catalog.json` - الطالب في `nexus.registry.cache_directory`. عند كتابة `--module`، يتم تثبيت وحدة التخصيص الإضافية، مما يسمح بتكوين وحدة الإدارة (NX-2) من خلال تراكب ذاكرة التخزين المؤقت التعديل الشامل `config.toml`.
3.`summary.json` - قم بمتابعة هذه العناصر والتراكب التحويلي وتعليمات المشغلين.
4. اختياريًا `registry_bundle.tar.*` - هذا مخصص لـ SCP أو S3 أو التحف الفنية.

قم بمزامنة جميع الكتالوجات (أو الأرشيف) مع كل مدقق، وقم بتجميع المضيفات المفرغة وانسخ البيانات + تراكب ذاكرة التخزين المؤقت في المكانين التاليين ريسترا قبل الوصول إلى Torii.

## 5. اختبارات الدخان المدقق

بعد الانتهاء من Torii، قم بتركيب مساعد الدخان الجديد للتأكد من أن الخط يتجه إلى `manifest_ready=true`، وتظهر المقاييس تخلص من كل الممرات وقياس الطبقة المختومة. الممرات، التي تحتوي على بيانات ضرورية، تحتوي على `manifest_path`; يساعد المساعد بشكل كبير في توجيه الإرشادات، حيث يتضمن كل جهاز NX-7 بيانًا مرفقًا:

```bash
scripts/nexus_lane_smoke.py \
  --status-url https://torii.example.com/v1/sumeragi/status \
  --metrics-url https://torii.example.com/metrics \
  --lane-alias payments \
  --expected-lane-count 3 \
  --min-da-quorum 0.95 \
  --max-oracle-staleness 75 \
  --expected-oracle-twap 60 \
  --oracle-twap-tolerance 5 \
  --max-oracle-haircut-bps 75 \
  --min-settlement-buffer 0.25 \
  --min-block-height 1000 \
  --max-finality-lag 4 \
  --max-settlement-backlog 0.5 \
  --max-headroom-events 0 \
  --max-slot-p95 1000 \
  --max-slot-p99 1100 \
  --min-slot-samples 10
```

قم بإضافة `--insecure` عند التسجيل بالتوقيع الذاتي. يحتوي النص البرمجي على رمز غير محدد، في حالة خروج المسار، أو مختوم، أو إرسال مقاييس/قياس عن بعد مع إرشادات إضافية. استخدم `--min-block-height` و`--max-finality-lag` و`--max-settlement-backlog` و`--max-headroom-events` لتشغيل جهاز قياس المسافة عبر المسار (الدخول) كتلة/نهائية/تراكم/إرتفاع) في السعة القصوى، وقم بإرفاقها مع `--max-slot-p95` / `--max-slot-p99` (و `--min-slot-samples`) للاشتراك كامل NX-18 ذو فتحة خفيفة، لا يخرج من المساعد.

يمكن للمحقق المزود بفجوات هوائية (أو CI) الحصول على منفذ مغلق Torii عبر نقطة نهاية الحياة:

```bash
scripts/nexus_lane_smoke.py \
  --status-file fixtures/nexus/lanes/status_ready.json \
  --metrics-file fixtures/nexus/lanes/metrics_ready.prom \
  --lane-alias core \
  --lane-alias payments \
  --expected-lane-count 3 \
  --min-da-quorum 0.95 \
  --max-oracle-staleness 75 \
  --expected-oracle-twap 60 \
  --oracle-twap-tolerance 5 \
  --max-oracle-haircut-bps 75 \
  --min-settlement-buffer 0.25 \
  --min-block-height 1000 \
  --max-finality-lag 4 \
  --max-settlement-backlog 0.5 \
  --max-headroom-events 0 \
  --max-slot-p95 1000 \
  --max-slot-p99 1100 \
  --min-slot-samples 10
```

تعرض التركيبات المسجلة في `fixtures/nexus/lanes/` العناصر الاصطناعية التي أنشأها مساعد bootstrap والتي قد تكون بيانات جديدة خالية من الوبر السيناريو. حصلت CI على نفس النقطة عبر `ci/check_nexus_lane_smoke.sh` و`ci/check_nexus_lane_registry_bundle.sh` (الاسم المستعار: `make check-nexus-lanes`)، لتتأكد من أن مساعد الدخان NX-7 موجود متطابق مع تنسيق الحمولة المنشور وأن حزمة الملخصات/التراكبات تؤدي إلى إمدادها بالطاقة.