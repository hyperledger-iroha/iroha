---
lang: ar
direction: rtl
source: docs/source/connect_architecture_followups.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 476331772efe169a7a073b561fa9935e314ff89d6bfff440f7246606c1c02669
source_last_modified: "2026-01-03T18:07:58.049266+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# ربط إجراءات متابعة الهندسة المعمارية

تلتقط هذه المذكرة المتابعات الهندسية التي ظهرت من مجموعة تطوير البرامج (SDK) المشتركة
ربط مراجعة الهندسة المعمارية. يجب أن يرتبط كل صف بمشكلة (تذكرة Jira أو PR)
بمجرد جدولة العمل. قم بتحديث الجدول بينما يقوم المالكون بإنشاء تذاكر التتبع.| العنصر | الوصف | المالك (المالكون) | تتبع | الحالة |
|------|------------|----------|----------|--------|
| ثوابت التراجع المشتركة | قم بتنفيذ مساعدات التراجع الأسي + jitter (`connect_retry::policy`) وقم بتعريضهم لمجموعات SDK الخاصة بـ Swift/Android/JS. | Swift SDK، Android Networking TL، JS Lead | [IOS-CONNECT-001](project_tracker/connect_architecture_followups_ios.md#ios-connect-001) | مكتمل — تم الوصول إلى `connect_retry::policy` باستخدام أخذ عينات Splitmix64 الحتمية؛ تقدم Swift (`ConnectRetryPolicy`) وAndroid وJS SDK مساعدين متطابقين بالإضافة إلى الاختبارات الذهبية. |
| إنفاذ البينج/بونج | إضافة تطبيق نبضات القلب القابل للتكوين باستخدام إيقاع الثلاثينيات المتفق عليه والحد الأدنى من تثبيت المتصفح؛ مقاييس السطح (`connect.ping_miss_total`). | Swift SDK، Android Networking TL، JS Lead | [IOS-CONNECT-002](project_tracker/connect_architecture_followups_ios.md#ios-connect-002) | مكتمل — يفرض Torii الآن فترات زمنية قابلة للتكوين لنبضات القلب (`ping_interval_ms`، `ping_miss_tolerance`، `ping_min_interval_ms`)، ويكشف عن مقياس `connect.ping_miss_total`، ويشحن اختبارات الانحدار التي تغطي معالجة قطع اتصال نبضات القلب. تعرض لقطات ميزة SDK المقابض الجديدة للعملاء. |
| استمرار قائمة الانتظار دون اتصال | قم بتطبيق Norito `.to` مؤلفي/قراء المجلات لقوائم انتظار الاتصال (Swift `FileManager`، وحدة تخزين Android المشفرة، JS IndexedDB) باستخدام المخطط المشترك. | Swift SDK، نموذج بيانات Android TL، JS Lead | [IOS-CONNECT-003](project_tracker/connect_architecture_followups_ios.md#ios-connect-003) | مكتمل - يقوم كل من Swift وAndroid وJS الآن بشحن مساعدات التشخيص `ConnectQueueJournal` + المشتركة مع اختبارات الاحتفاظ/تجاوز السعة بحيث تظل حزم الأدلة حتمية عبر SDKs.【IrohaSwift/Sources/IrohaSwift/ConnectQueueJournal.swift:1】【java/iroha_android/src/main/java/org/hyperledger/iroha/android/connect/ConnectQueueJournal.java:1】 【javascript/iroha_js/src/connectQueueJournal.js:1】 |
| حمولة شهادة StrongBox | قم بربط `{platform,evidence_b64,statement_hash}` من خلال موافقات المحفظة وأضف التحقق إلى dApp SDKs. | Android Crypto TL، JS Lead | [IOS-CONNECT-004](project_tracker/connect_architecture_followups_ios.md#ios-connect-004) | في انتظار |
| إطار التحكم في الدوران | تنفيذ `Control::RotateKeys` + `RotateKeysAck` وكشف `cancelRequest(hash)` / واجهات برمجة التطبيقات للتدوير في جميع مجموعات SDK. | Swift SDK، Android Networking TL، JS Lead | [IOS-CONNECT-005](project_tracker/connect_architecture_followups_ios.md#ios-connect-005) | في انتظار |
| مصدرين القياس عن بعد | قم بإصدار `connect.queue_depth`، و`connect.reconnects_total`، و`connect.latency_ms`، وأعد تشغيل العدادات في مسارات القياس عن بعد الموجودة (OpenTelemetry). | القياس عن بعد WG، أصحاب SDK | [IOS-CONNECT-006](project_tracker/connect_architecture_followups_ios.md#ios-connect-006) | في انتظار |
| سويفت CI النابضة | تأكد من استدعاء خطوط الأنابيب ذات الصلة بالاتصال `make swift-ci` بحيث تظل تكافؤ التركيبات وموجزات لوحة المعلومات وبيانات تعريف Buildkite `ci/xcframework-smoke:<lane>:device_tag` متوافقة عبر مجموعات SDK. | قائد Swift SDK، البنية التحتية | [IOS-CONNECT-007](project_tracker/connect_architecture_followups_ios.md#ios-connect-007) | في انتظار |
| الإبلاغ عن الحوادث الاحتياطية | قم بتوصيل حوادث تسخير الدخان XCFramework (`xcframework_smoke_fallback`، `xcframework_smoke_strongbox_unavailable`) إلى لوحات معلومات Connect للرؤية المشتركة. | قائد ضمان الجودة السريع، بناء البنية التحتية | [IOS-CONNECT-008](project_tracker/connect_architecture_followups_ios.md#ios-connect-008) | في انتظار || مرفقات الامتثال التمريرية | تأكد من قبول مجموعات تطوير البرامج (SDK) وحقول `attachments[]` + `compliance_manifest_id` الاختيارية وإعادة توجيهها في حمولات الموافقة دون فقدان. | Swift SDK، نموذج بيانات Android TL، JS Lead | [IOS-CONNECT-009](project_tracker/connect_architecture_followups_ios.md#ios-connect-009) | في انتظار |
| محاذاة تصنيف الخطأ | قم بتعيين التعداد المشترك (`Transport`، `Codec`، `Authorization`، `Timeout`، `QueueOverflow`، `Internal`) إلى الأخطاء الخاصة بالنظام الأساسي باستخدام المستندات/الأمثلة. | Swift SDK، Android Networking TL، JS Lead | [IOS-CONNECT-010](project_tracker/connect_architecture_followups_ios.md#ios-connect-010) | مكتمل - تقوم مجموعات SDK الخاصة بـ Swift وAndroid وJS بشحن غلاف `ConnectError` المشترك + مساعدي القياس عن بعد مع مستندات README/TypeScript/Java واختبارات الانحدار التي تغطي TLS/timeout/HTTP/codec/queue Cases.【docs/source/connect_error_taxonomy.md:1】【IrohaSwift/Sources/IrohaSwift/ConnectError.swift:1】【java/iroha_android/src /test/java/org/hyperledger/iroha/android/connect/ConnectErrorTests.java:1】[javascript/iroha_js/test/connectError.test.js:1] |
| سجل قرارات ورشة العمل | نشر المجموعة المشروحة / الملاحظات التي تلخص القرارات المقبولة في أرشيف المجلس. | قائد برنامج SDK | [IOS-CONNECT-011](project_tracker/connect_architecture_followups_ios.md#ios-connect-011) | في انتظار |

> سيتم ملء معرفات التتبع عندما يفتح المالكون التذاكر؛ قم بتحديث العمود `Status` جنبًا إلى جنب مع تقدم المشكلة.