---
lang: ar
direction: rtl
source: docs/source/iroha_monitor.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 05149d624d680d04433be41a4525538c97bd103ae7f80dda2613a6adb181a93d
source_last_modified: "2026-01-03T18:07:57.206662+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# شاشة Iroha

تعمل شاشة Iroha المُعاد تصنيعها على دمج واجهة مستخدم طرفية خفيفة الوزن مع الرسوم المتحركة
مهرجان فن ASCII وموضوع Etenraku التقليدي.  ويركز على اثنين
سير العمل البسيط:

- **وضع Spawn-lite** - بدء الحالة المؤقتة/المقاييس التي تحاكي الأقران.
- **وضع الإرفاق** - قم بتوجيه الشاشة إلى نقاط نهاية Torii HTTP الموجودة.

تعرض واجهة المستخدم ثلاث مناطق عند كل تحديث:

1. **Torii skyline header** – بوابة توري المتحركة، وجبل فوجي، وموجات كوي، والنجمة
   الحقل الذي يتم تمريره بشكل متزامن مع إيقاع التحديث.
2. **الشريط الملخص** – الكتل/المعاملات/الغاز المجمعة بالإضافة إلى توقيت التحديث.
3. **طاولة الأقران وهمسات المهرجان** – صفوف الأقران على اليسار، حدث دوار
   قم بتسجيل الدخول على اليمين الذي يلتقط التحذيرات (المهلات، الحمولات كبيرة الحجم، وما إلى ذلك).
4. **اتجاه الغاز الاختياري** – قم بتمكين `--show-gas-trend` لإلحاق خط المؤشر
   تلخيص إجمالي استخدام الغاز عبر جميع الأقران.

الجديد في هذا إعادة البناء:

- مشهد ASCII متحرك على الطريقة اليابانية مع كوي، توري، والفوانيس.
- سطح أوامر مبسط (`--spawn-lite`، `--attach`، `--interval`).
- لافتة مقدمة مع تشغيل صوتي اختياري لموضوع gagaku (MIDI خارجي
  player أو المركب الناعم المدمج عندما يدعمه النظام الأساسي/مكدس الصوت).
- علامات `--no-theme` / `--no-audio` لـ CI أو الدخان السريع.
- عمود "الحالة المزاجية" لكل نظير يعرض آخر تحذير، أو وقت الالتزام، أو وقت التشغيل.

## البداية السريعة

قم ببناء الشاشة وتشغيلها ضد أقرانهم المتعثرين:

```bash
cargo run -p iroha_monitor -- --spawn-lite --peers 3
```

إرفاق بنقاط النهاية Torii الموجودة:

```bash
cargo run -p iroha_monitor -- \
  --attach http://127.0.0.1:8080 http://127.0.0.1:8081 \
  --interval 500
```

استدعاء صديق لـ CI (تخطي الرسوم المتحركة والصوت المقدمة):

```bash
cargo run -p iroha_monitor -- --spawn-lite --no-theme --no-audio
```

### أعلام CLI

```
--spawn-lite         start local status/metrics stubs (default if no --attach)
--attach <URL...>    attach to existing Torii endpoints
--interval <ms>      refresh interval (default 800ms)
--peers <N>          stub count when spawn-lite is active (default 4)
--no-theme           skip the animated intro splash
--no-audio           mute theme playback (still prints the intro frames)
--midi-player <cmd>  external MIDI player for the built-in Etenraku .mid
--midi-file <path>   custom MIDI file for --midi-player
--show-gas-trend     render the aggregate gas sparkline panel
--art-speed <1-8>    multiply the animation step rate (1 = default)
--art-theme <name>   choose between night, dawn, or sakura palettes
--headless-max-frames <N>
                     cap headless fallback to N frames (0 = unlimited)
```

## مقدمة الموضوع

افتراضيًا، يقوم بدء التشغيل بتشغيل رسم متحرك قصير لـ ASCII أثناء تسجيل نتيجة Etenraku
يبدأ.  ترتيب اختيار الصوت:

1. إذا تم توفير `--midi-player`، فقم بإنشاء MIDI التجريبي (أو استخدم `--midi-file`)
   وتفرخ الأمر.
2. بخلاف ذلك، على نظام التشغيل macOS/Windows (أو Linux مع `--features iroha_monitor/linux-builtin-synth`)
   اعرض النتيجة باستخدام موالفة gagaku الناعمة المدمجة (لا يوجد صوت خارجي
   الأصول المطلوبة).
3. إذا تم تعطيل الصوت أو فشل التهيئة، فستستمر المقدمة في طباعة الملف
   الرسوم المتحركة ويدخل على الفور TUI.

يتم تمكين المركب الذي يعمل بنظام CPAL تلقائيًا على نظامي التشغيل macOS وWindows. على لينكس هو عليه
قم بالاشتراك لتجنب فقدان رؤوس ALSA/Pulse أثناء إنشاء مساحة العمل؛ تمكينه
مع `--features iroha_monitor/linux-builtin-synth` إذا كان نظامك يوفر ملف
مكدس الصوت العامل.

استخدم `--no-theme` أو `--no-audio` عند التشغيل في CI أو الأصداف مقطوعة الرأس.

يتبع الموالفة الناعمة الآن الترتيب الذي تم التقاطه في تصميم موالفة *MIDI في
Rust.pdf*: يتشارك كل من hichiriki وryūteki في لحن غير متجانس بينما يقوم shō
يوفر منصات aitake الموضحة في الوثيقة.  تبقى بيانات الملاحظة الموقوتة حية
في `etenraku.rs`؛ فهو يعمل على تشغيل كل من رد اتصال CPAL وMIDI التجريبي الذي تم إنشاؤه.
عندما يكون إخراج الصوت غير متاح، تتخطى الشاشة التشغيل ولكنها تستمر في العرض
الرسوم المتحركة ASCII.

## نظرة عامة على واجهة المستخدم- **صورة الرأس** - تم إنشاء كل إطار بواسطة `AsciiAnimator`؛ كوي، فوانيس توري،
  وتنجرف الأمواج لتعطي حركة مستمرة.
- **شريط الملخص** - يعرض النظراء عبر الإنترنت، وعدد النظراء المُبلغ عنهم، وإجماليات الحظر،
  إجماليات الكتل غير الفارغة، والموافقات/الرفضات، واستخدام الغاز، ومعدل التحديث.
- **جدول النظراء** - أعمدة للاسم المستعار/نقطة النهاية، والكتل، والمعاملات، وحجم قائمة الانتظار،
  استخدام الغاز، ووقت الاستجابة، وتلميح "الحالة المزاجية" (التحذيرات، ووقت الالتزام، ووقت التشغيل).
- **همسات المهرجان** - سجل التحذيرات المتجدد (أخطاء الاتصال، الحمولة
  الحد من الخروقات ونقاط النهاية البطيئة).  يتم عكس الرسائل (الأحدث في الأعلى).

اختصارات لوحة المفاتيح:

- `n` / يمين / أسفل - نقل التركيز إلى النظير التالي.
- `p` / يسار / أعلى - نقل التركيز إلى النظير السابق.
- `q` / Esc / Ctrl-C – الخروج من الجهاز واستعادته.

تستخدم الشاشة crossterm +ratatui مع مخزن مؤقت للشاشة البديلة؛ على الخروج منه
يستعيد المؤشر ويمسح الشاشة.

## اختبارات الدخان

يشحن الصندوق اختبارات التكامل التي تمارس كلا الوضعين وحدود HTTP:

-`spawn_lite_smoke_renders_frames`
-`attach_mode_with_stubs_runs_cleanly`
-`invalid_endpoint_surfaces_warning`
-`status_limit_warning_is_rendered`
-`attach_mode_with_slow_peer_renders_multiple_frames`

قم بإجراء اختبارات الشاشة فقط:

```bash
cargo test -p iroha_monitor -- --nocapture
```

تحتوي مساحة العمل على اختبارات تكامل أثقل (`cargo test --workspace`). الجري
لا تزال اختبارات الشاشة بشكل منفصل مفيدة للتحقق السريع من الصحة عند القيام بذلك
لا تحتاج إلى جناح كامل.

## تحديث لقطات الشاشة

يركز العرض التوضيحي للمستندات الآن على أفق torii وجدول الأقران.  لتحديث
الأصول، تشغيل:

```bash
make monitor-screenshots
```

هذا يلتف `scripts/iroha_monitor_demo.sh` (وضع النشر البسيط، المصدر الثابت/منفذ العرض،
لا يوجد مقدمة / صوت، لوحة الفجر، سرعة الفن 1، غطاء مقطوع الرأس 24) ويكتب
إطارات SVG/ANSI بالإضافة إلى `manifest.json` و`checksums.json` إلى
`docs/source/images/iroha_monitor_demo/`. `make check-iroha-monitor-docs`
يلتف كلا من حراس CI (`ci/check_iroha_monitor_assets.sh` و
`ci/check_iroha_monitor_screenshots.sh`) لذلك تجزئات المولد وحقول البيان،
وتبقى المجاميع الاختبارية متزامنة؛ يتم أيضًا شحن لقطة الشاشة كـ
`python3 scripts/check_iroha_monitor_screenshots.py`. مرر `--no-fallback` إلى
البرنامج النصي التجريبي إذا كنت تريد أن يفشل الالتقاط بدلاً من الرجوع إلى ملف
الإطارات المخبوزة عندما يكون مخرج الشاشة فارغًا؛ عندما يتم استخدام الاحتياطي الخام
تتم إعادة كتابة ملفات `.ans` باستخدام الإطارات المخبوزة بحيث يبقى البيان/المجاميع الاختبارية
حتمية.

## لقطات حتمية

اللقطات التي تم شحنها موجودة في `docs/source/images/iroha_monitor_demo/`:

![نظرة عامة على الشاشة](images/iroha_monitor_demo/iroha_monitor_demo_overview.svg)
![خط أنابيب المراقبة](images/iroha_monitor_demo/iroha_monitor_demo_pipeline.svg)

إعادة إنتاجها باستخدام إطار عرض/بذرة ثابتة:

```bash
scripts/iroha_monitor_demo.sh \
  --cols 120 --rows 48 \
  --interval 500 \
  --seed iroha-monitor-demo
```

يقوم مساعد الالتقاط بإصلاح `LANG`/`LC_ALL`/`TERM`، إلى الأمام
`IROHA_MONITOR_DEMO_SEED`، يقوم بكتم الصوت وتثبيت السمة/السرعة الفنية بحيث يتم
يتم عرض الإطارات بشكل متطابق عبر الأنظمة الأساسية. يكتب `manifest.json` (generator
التجزئات + الأحجام) و`checksums.json` (ملخصات SHA-256) ضمن
`docs/source/images/iroha_monitor_demo/`; يعمل CI
`ci/check_iroha_monitor_assets.sh` و`ci/check_iroha_monitor_screenshots.sh`
تفشل عندما تنحرف الأصول عن البيانات المسجلة.

## استكشاف الأخطاء وإصلاحها- **لا يوجد إخراج صوت** - تعود الشاشة إلى وضع التشغيل الصامت وتستمر.
- **الخروج الاحتياطي مقطوع الرأس مبكرًا** - تعمل أغطية الشاشة مقطوعة الرأس على زوجين
  عشرات الإطارات (حوالي 12 ثانية في الفاصل الزمني الافتراضي) عندما لا يمكن التبديل
  المحطة في الوضع الخام. قم بتمرير `--headless-max-frames 0` لإبقائه قيد التشغيل
  إلى أجل غير مسمى.
- **حمولات الحالة كبيرة الحجم** - عمود الحالة المزاجية للزملاء وسجل المهرجان
  إظهار `body exceeds …` مع الحد الذي تم تكوينه (`128 KiB`).
- **الأقران البطيئون** - يسجل سجل الأحداث تحذيرات انتهاء المهلة؛ التركيز على أن النظير ل
  تسليط الضوء على الصف.

استمتع بأفق المهرجان!  مساهمات لزخارف ASCII إضافية أو
نرحب بلوحات المقاييس - اجعلها حتمية حتى تظهر المجموعات كما هي
إطارًا تلو الآخر بغض النظر عن المحطة.