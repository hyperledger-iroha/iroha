---
lang: ar
direction: rtl
source: docs/source/content_hosting.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4c0c7f98dbd9f49c573302f0b5cbe2e7a663d7fe35a1a9eea8da4f24c6f9bc8b
source_last_modified: "2026-01-05T17:57:58.226177+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

٪ استضافة المحتوى
% Iroha الأساسية

# حارة استضافة المحتوى

يقوم مسار المحتوى بتخزين حزم ثابتة صغيرة (أرشيفات القطران) على السلسلة ويخدم
الملفات الفردية مباشرة من Torii.

- **النشر**: أرسل `PublishContentBundle` مع أرشيف القطران، وانتهاء الصلاحية اختياري
  الارتفاع والبيان الاختياري. معرف الحزمة هو تجزئة blake2b لملف
  كرة القطران. يجب أن تكون إدخالات Tar ملفات عادية؛ يتم تطبيع الأسماء مسارات UTF-8.
  تأتي الأحرف الكبيرة للحجم/المسار/عدد الملفات من تكوين `content` (`max_bundle_bytes`،
  `max_files`، `max_path_len`، `max_retention_blocks`، `chunk_size_bytes`).
  تتضمن البيانات تجزئة فهرس Norito ومساحة البيانات/الممر وسياسة ذاكرة التخزين المؤقت
  (`max_age_seconds`، `immutable`)، وضع المصادقة (`public` / `role:<role>` /
  `sponsor:<uaid>`)، والعنصر النائب لسياسة الاستبقاء، وتجاوزات MIME.
- **إزالة البيانات**: يتم تقسيم حمولات القطران (افتراضي 64 كيلو بايت) وتخزينها مرة واحدة في كل مرة
  التجزئة مع التهم المرجعية. تقاعد الحزمة يتناقص وقطع البرقوق.
- **العرض**: يعرض Torii `GET /v2/content/{bundle}/{path}`. تيار الردود
  مباشرة من مخزن القطع باستخدام `ETag` = تجزئة الملف، `Accept-Ranges: bytes`،
  دعم النطاق والتحكم في ذاكرة التخزين المؤقت المستمدة من البيان. يقرأ تكريم
  وضع المصادقة الواضحة: تتطلب الاستجابات المرتبطة بالأدوار والراعية بوابات أساسية
  رؤوس الطلب (`X-Iroha-Account`، `X-Iroha-Signature`) للموقع
  حساب؛ الحزم المفقودة/المنتهية الصلاحية ترجع 404.
- **CLI**: `iroha content publish --bundle <path.tar>` (أو `--root <dir>`) الآن
  يقوم تلقائيًا بإنشاء بيان، ويصدر `--manifest-out/--bundle-out` اختياريًا، و
  يقبل `--auth`، `--cache-max-age-secs`، `--dataspace`، `--lane`، `--immutable`،
  وتجاوزات `--expires-at-height`. بنيات `iroha content pack --root <dir>`
  قطران حتمي + بيان دون تقديم أي شيء.
- **التكوين**: توجد مقابض ذاكرة التخزين المؤقت/المصادقة ضمن `content.*` في `iroha_config`
  (`default_cache_max_age_secs`، `max_cache_max_age_secs`، `immutable_bundles`،
  `default_auth_mode`) ويتم فرضها في وقت النشر.
- **SLO + الحدود**: `content.max_requests_per_second` / `request_burst` و
  `content.max_egress_bytes_per_second` / `egress_burst_bytes` جانب القراءة
  الإنتاجية؛ يقوم Torii بفرض كليهما قبل تقديم البايتات والصادرات
  `torii_content_requests_total`، `torii_content_request_duration_seconds`، و
  مقاييس `torii_content_response_bytes_total` مع تسميات النتائج. الكمون
  الأهداف مباشرة تحت `content.target_p50_latency_ms` /
  `content.target_p99_latency_ms` / `content.target_availability_bps`.
- **عناصر التحكم في إساءة الاستخدام**: يتم تحديد مجموعات الأسعار بواسطة رمز UAID/API المميز/IP البعيد، و
  يمكن لواقي PoW الاختياري (`content.pow_difficulty_bits`، `content.pow_header`)
  تكون مطلوبة قبل أن يقرأ. تأتي الإعدادات الافتراضية لتخطيط شريط DA من
  `content.stripe_layout` ويتم تكرارها في الإيصالات/تجزئات البيان.
- **الإيصالات وأدلة DA**: إرفاق الإجابات الناجحة
  `sora-content-receipt` (base64 Norito بإطار `ContentDaReceipt` بايت) يحمل
  `bundle_id`، `path`، `file_hash`، `served_bytes`، نطاق البايت المقدم،
  `chunk_root` / `stripe_layout`، والتزام PDP الاختياري، والطابع الزمني لذلك
  يمكن للعملاء تثبيت ما تم جلبه دون إعادة قراءة النص.

المراجع الرئيسية:- نموذج البيانات: `crates/iroha_data_model/src/content.rs`
- التنفيذ: `crates/iroha_core/src/smartcontracts/isi/content.rs`
- معالج Torii: `crates/iroha_torii/src/content.rs`
- مساعد سطر الأوامر: `crates/iroha_cli/src/content.rs`