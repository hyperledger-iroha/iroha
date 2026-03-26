---
lang: ar
direction: rtl
source: docs/source/testing.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c7d9bce40727d178bcc7d780c608d82bcd14b0814a7b537cbe9c39a539a200c8
source_last_modified: "2025-12-19T22:31:17.718007+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- الترجمة العربية لـ docs/source/testing.md -->

# دليل الاختبار واستكشاف الاعطال

يوضح هذا الدليل كيفية اعادة انتاج سيناريوهات التكامل، وما البنية التحتية التي يجب ان تكون متاحة، وكيفية جمع سجلات قابلة للتنفيذ. ارجع الى [تقرير الحالة](../../status.md) على مستوى المشروع قبل البدء لمعرفة اي المكونات حاليا باللون الاخضر.

## خطوات اعادة الانتاج

### اختبارات التكامل (`integration_tests` crate)

1. تاكد من بناء تبعيات مساحة العمل: `cargo build --workspace`.
2. شغل مجموعة اختبارات التكامل مع السجلات الكاملة: `cargo test -p integration_tests -- --nocapture`.
3. اذا احتجت لاعادة تشغيل سيناريو محدد، استخدم مسار الوحدة، مثلا `cargo test -p integration_tests settlement::happy_path -- --nocapture`.
4. التقط fixtures مسلسلة باستخدام Norito لضمان اتساق المدخلات بين العقد:
   ```rust
   use norito::json;

   let genesis_payload = json::to_string_pretty(&json::json!({
       "chain" : "testnet",
       "peers" : ["127.0.0.1:1337"],
       "accounts" : [{
           "id" : "soraカタカナ...",
           "public_key" : "ed0120..."
       }]
   }))?;
   ```
   احفظ Norito JSON الناتج بجانب artefacts الاختبار حتى تتمكن العقد من اعادة تشغيل الحالة نفسها.

### اختبارات عميل Python (`pytests` directory)

1. ثبت متطلبات Python عبر `pip install -r pytests/requirements.txt` داخل بيئة افتراضية.
2. صدر fixtures المتوافقة مع Norito التي تم توليدها اعلاه عبر مسار مشترك او متغير بيئة.
3. شغل المجموعة مع مخرجات مطولة: `pytest -vv pytests`.
4. للتصحيح الموجه، شغل `pytest -k "Query" pytests/tests/test_queries.py --log-cli-level=INFO`.

## المنافذ والخدمات المطلوبة

يجب ان تكون الخدمات التالية قابلة للوصول قبل تنفيذ اي من المجموعات:

- **Torii HTTP API**: الافتراضي `127.0.0.1:1337`. يمكن تجاوزها عبر `torii.address` في الاعدادات (انظر `docs/source/references/peer.template.toml`).
- **اشعارات Torii WebSocket**: الافتراضي `127.0.0.1:8080` لاشتراكات العميل المستخدمة من `pytests`.
- **مصدّر التليمترية**: الافتراضي `127.0.0.1:8180`. تتوقع اختبارات التكامل ان تصل المقاييس هنا للتحقق من الصحة.
- **PostgreSQL** (عند التفعيل): الافتراضي `127.0.0.1:5432`. تاكد من تطابق بيانات الاعتماد مع ملف compose في [`defaults/docker-compose.local.yml`](../../defaults/docker-compose.local.yml).

راجع [دليل استكشاف مشاكل التليمترية](telemetry.md) اذا كان اي endpoint غير متاح.

### استقرار ال peers المدمجة

`NetworkBuilder::start()` يفرض الان نافذة حيوية لمدة خمس ثوان بعد genesis لكل peer مدمج. اذا خرجت عملية خلال فترة الحماية هذه، يقوم builder بالانهاء مع خطا مفصل يشير الى سجلات stdout/stderr المخزنة. على الاجهزة ذات الموارد المحدودة يمكنك توسيع النافذة (بالمللي ثانية) عبر ضبط `IROHA_TEST_POST_GENESIS_LIVENESS_MS`; خفضها الى `0` يلغي الحماية بالكامل. تاكد من وجود هامش CPU كاف خلال الثواني الاولى من كل مجموعة تكامل حتى تتمكن العقد من بلوغ الكتلة 1 دون تفعيل المراقب.

## جمع السجلات وتحليلها

ابدأ من مجلد تشغيل نظيف كي لا تخفي artefacts السابقة المشاكل الجديدة. تجمع السكربتات ادناه السجلات بصيغ يمكن لادوات Norito اللاحقة استهلاكها.

- استخدم [`scripts/analyze_telemetry.sh`](../../scripts/analyze_telemetry.sh) بعد تنفيذ الاختبارات لتجميع مقاييس العقد في snapshots Norito JSON مع طوابع زمنية.
- عند التحقيق في مشاكل الشبكة، شغل [`scripts/run_iroha_monitor_demo.py`](../../scripts/run_iroha_monitor_demo.py) لبث احداث Torii الى `monitor_output.norito.json`.
- تحفظ سجلات اختبارات التكامل ضمن `integration_tests/target/`; اضغطها باستخدام [`scripts/profile_build.sh`](../../scripts/profile_build.sh) لمشاركتها مع الفرق الاخرى.
- تكتب سجلات عميل Python في `pytests/.pytest_cache`. صدرها مع التليمترية الملتقطة عبر:
  ```bash
  ./scripts/report_red_team_failures.py --tests pytests --artifacts out/logs
  ```

اجمع حزمة كاملة (integration, Python, telemetry) قبل فتح issue حتى يتمكن المشرفون من اعادة تشغيل تتبعات Norito.

## الخطوات التالية

للقوائم الخاصة بالاصدار، راجع [pipeline](pipeline.md). اذا اكتشفت تراجعات او اخفاقات، وثقها في [status tracker](../../status.md) المشترك واربطها باي مدخلات ذات صلة في [استكشاف مشاكل sumeragi](sumeragi.md).

</div>
