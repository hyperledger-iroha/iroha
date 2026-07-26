---
lang: ar
direction: rtl
source: docs/source/confidential_assets_rotation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: fd1e43316c492cc96ed107f6318841ad8db160735d4698c4f05562ff6127fda9
source_last_modified: "2026-01-22T15:38:30.658859+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! دليل تشغيل الأصول السري المشار إليه بواسطة `roadmap.md:M3`.

# دليل تداول الأصول السرية

يشرح دليل التشغيل هذا كيفية قيام المشغلين بجدولة الأصول السرية وتنفيذها
التدوير (مجموعات المعلمات، والتحقق من المفاتيح، وانتقالات السياسة) أثناء
ضمان بقاء المحافظ وعملاء Torii وحراس الذاكرة حتمية.

## دورة الحياة والحالات

مجموعات المعلمات السرية (`PoseidonParams`، `PedersenParams`، التحقق من المفاتيح)
الشبكة والمساعد يستخدمان لاشتقاق الحالة الفعالة عند ارتفاع معين
`crates/iroha_core/src/state.rs:7540`–`7561`. مساعدو وقت التشغيل يكتسحون المعلقة
التحولات بمجرد الوصول إلى الارتفاع المستهدف وتسجيل حالات الفشل في وقت لاحق
إعادة البث (`crates/iroha_core/src/state.rs:6725`–`6765`).

تضمين سياسات الأصول
`pending_transition { transition_id, new_mode, effective_height, conversion_window }`
حتى تتمكن الإدارة من جدولة الترقيات عبر
`ScheduleConfidentialPolicyTransition` وقم بإلغائها إذا لزم الأمر. انظر
المرايا `crates/iroha_data_model/src/asset/definition.rs:320` والمرايا Torii DTO
(`crates/iroha_torii/src/routing.rs:1539` –`1580`).

## سير عمل التناوب

1. **نشر حزم المعلمات الجديدة.** يقدم المشغلون
   تعليمات `PublishPedersenParams`/`PublishPoseidonParams` (CLI
   `iroha app zk params publish ...`) لتجهيز مجموعات المولدات الجديدة بالبيانات الوصفية،
   نوافذ التنشيط/الإيقاف، وعلامات الحالة. المنفذ يرفض
   معرفات مكررة، أو إصدارات غير متزايدة، أو انتقالات حالة سيئة لكل
   `crates/iroha_core/src/smartcontracts/isi/world.rs:2499`–`2635`، و
   تغطي اختبارات التسجيل أوضاع الفشل (`crates/iroha_core/tests/confidential_params_registry.rs:93` –`226`).
2. **تسجيل/التحقق من تحديثات المفتاح.** `RegisterVerifyingKey` يفرض الواجهة الخلفية،
   الالتزام، وقيود الدائرة/الإصدار قبل أن يتمكن المفتاح من الدخول إلى
   التسجيل (`crates/iroha_core/src/smartcontracts/isi/world.rs:2067`-`2137`).
   يؤدي تحديث المفتاح تلقائيًا إلى إيقاف الإدخال القديم ومسح وحدات البايت المضمنة،
   كما تمارسها `crates/iroha_core/tests/zk_vk_deprecate_marks_status.rs:1`.
3. **جدولة عمليات انتقال سياسة الأصول.** بمجرد تفعيل معرفات المعلمات الجديدة،
   يستدعي الحكم `ScheduleConfidentialPolicyTransition` مع المطلوب
   الوضع والنافذة الانتقالية وتجزئة التدقيق. المنفذ يرفض المتضاربة
   التحولات أو الأصول ذات العرض الشفاف المتميز. اختبارات مثل
   `crates/iroha_core/tests/confidential_policy_gates.rs:300`–`384` التحقق من ذلك
   التحولات المجهضة واضحة `pending_transition`، في حين
   `confidential_policy_transition_reaches_shielded_only_on_schedule` في
   تؤكد الخطوط 385–433 أن الترقيات المجدولة تقلب إلى `ShieldedOnly` تمامًا عند
   الارتفاع الفعال.
4. ** تطبيق السياسة وحارس الذاكرة. ** يقوم منفذ الكتلة بمسح كل شيء معلق
   التحولات في بداية كل كتلة (`apply_policy_if_due`) وتنبعث
   القياس عن بعد في حالة فشل النقل حتى يتمكن المشغلون من إعادة الجدولة. أثناء القبول
   يرفض مجمع الذاكرة المعاملات التي قد تتغير سياستها الفعالة في منتصف الكتلة،
   ضمان الشمول الحتمي عبر النافذة الانتقالية
   (`docs/source/confidential_assets.md:60`).

## متطلبات المحفظة وSDK- يعرض Swift ومجموعات SDK المحمولة الأخرى مساعدي Torii لجلب السياسة النشطة
  بالإضافة إلى أي عملية نقل معلقة، حتى تتمكن المحافظ من تحذير المستخدمين قبل التوقيع. انظر
  `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:309` (DTO) وما يرتبط به
  الاختبارات في `IrohaSwift/Tests/IrohaSwiftTests/ToriiClientTests.swift:591`.
- يعكس CLI نفس البيانات التعريفية عبر `iroha ledger assets data-policy get` (المساعد في
  `crates/iroha_cli/src/main.rs:1497`–`1670`)، لتمكين المشغلين من تدقيق
  تم ربط معرفات السياسة/المعلمات في تعريف الأصل دون اكتشاف الكهوف
  متجر الكتلة.

## تغطية الاختبار والقياس عن بعد

- `crates/iroha_core/tests/zk_ledger_scaffold.rs:288`–`345` يتحقق من هذه السياسة
  تنتشر التحولات في لقطات البيانات التعريفية ويتم مسحها بمجرد تطبيقها.
- `crates/iroha_core/tests/zk_dedup.rs:1` يثبت أن ذاكرة التخزين المؤقت `Preverify`
  يرفض الإنفاق المزدوج/الإثباتات المزدوجة، بما في ذلك سيناريوهات التناوب حيث
  تختلف الالتزامات.
- `crates/iroha_core/tests/zk_confidential_events.rs` و
  `zk_shield_transfer_audit.rs` غطاء درع من طرف إلى طرف ← نقل ← فك الدرع
  التدفقات، مما يضمن بقاء مسار التدقيق عبر دورات المعلمات.
- `dashboards/grafana/confidential_assets.json` و
  `docs/source/confidential_assets.md:401` يوثق شجرة الالتزام &
  مقاييس ذاكرة التخزين المؤقت للتحقق التي تصاحب كل عملية معايرة/دوران.

## ملكية Runbook

- ** عملاء DevRel / Wallet SDK: ** الحفاظ على مقتطفات SDK + عمليات التشغيل السريعة التي تظهر
  كيفية عرض التحولات المعلقة وإعادة تشغيل النعناع → النقل → الكشف
  الاختبارات محليًا (يتم تتبعها ضمن `docs/source/project_tracker/confidential_assets_phase_c.md:M3.2`).
- ** إدارة البرنامج / الأصول السرية TL: ** الموافقة على طلبات النقل، والاحتفاظ بها
  تم تحديث `status.md` بالدورات القادمة، وتأكد من وجود التنازلات (إن وجدت)
  المسجلة جنبا إلى جنب مع دفتر المعايرة.