<!-- Auto-generated stub for Arabic (ar) translation. Replace this content with the full translation. -->

---
lang: ar
direction: rtl
source: security_audit_report.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4343a8f72823a2539a2e79e80a1c7162d455e03b450e2fd613f01ac5c61aec63
source_last_modified: "2026-03-26T13:01:48.980087+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# تقرير التدقيق الأمني

التاريخ: 2026-03-26

## ملخص تنفيذي

ركزت عملية التدقيق هذه على الأسطح الأكثر خطورة في الشجرة الحالية: Torii تدفقات HTTP/API/auth، ونقل P2P، وواجهات برمجة تطبيقات المعالجة السرية، وحرس نقل SDK، ومسار تعقيم المرفقات.

لقد وجدت 6 قضايا قابلة للتنفيذ:

- 2 نتائج شديدة الخطورة
- 4 نتائج متوسطة الخطورة

وأهم المشاكل هي:

1. يقوم Torii حاليًا بتسجيل رؤوس الطلبات الواردة لكل طلب HTTP، والتي يمكن أن تعرض الرموز المميزة لحاملها، والرموز المميزة لواجهة برمجة التطبيقات (API)، والرموز المميزة لجلسة المشغل/bootstrap، وعلامات mTLS المعاد توجيهها إلى السجلات.
2. لا تزال مسارات Torii العامة المتعددة ومجموعات SDK تدعم إرسال قيم `private_key` الأولية إلى الخادم حتى يتمكن Torii من تسجيل الدخول نيابة عن المتصل.
3. يتم التعامل مع العديد من المسارات "السرية" كهيئات طلب عادية، بما في ذلك الاشتقاق الأساسي السري ومصادقة الطلب الأساسي في بعض أدوات تطوير البرامج (SDK).

## الطريقة

- مراجعة ثابتة لمسارات المعالجة السرية Torii وP2P وcrypto/VM وSDK
- أوامر التحقق المستهدفة:
  - `cargo check -p iroha_torii --lib --message-format short` -> تمرير
  - `cargo check -p iroha_p2p --message-format short` -> تمرير
  - `cargo test -p iroha_torii --lib confidential_derive_keyset_endpoint_roundtrip -- --nocapture` -> تمرير
  - `cargo deny check advisories bans sources --hide-inclusion-graph` -> تمرير، تحذيرات الإصدار المكرر فقط
- لم يكتمل في هذا المرور:
  - بناء مساحة عمل كاملة/اختبار/قصاصة
  - مجموعات اختبار Swift/Gradle
  - التحقق من صحة وقت تشغيل CUDA/Metal

## النتائج

### SA-001 High: يقوم Torii بتسجيل رؤوس الطلبات الحساسة على مستوى العالمالتأثير: يمكن لأي عملية نشر تطلب السفن التتبع أن تؤدي إلى تسرب الرموز المميزة للحامل/واجهة برمجة التطبيقات/المشغل ومواد المصادقة ذات الصلة إلى سجلات التطبيق.

الأدلة:

- `crates/iroha_torii/src/lib.rs:20752` يمكّن `TraceLayer::new_for_http()`
- `crates/iroha_torii/src/lib.rs:20753` يمكّن `DefaultMakeSpan::default().include_headers(true)`
- يتم استخدام أسماء الرؤوس الحساسة بشكل نشط في أماكن أخرى في نفس الخدمة:
  -`crates/iroha_torii/src/operator_auth.rs:40`
  -`crates/iroha_torii/src/operator_auth.rs:41`
  -`crates/iroha_torii/src/operator_auth.rs:42`
  -`crates/iroha_torii/src/operator_auth.rs:43`

لماذا يهم هذا:

- يقوم `include_headers(true)` بتسجيل قيم الرأس الواردة بالكامل في امتدادات التتبع.
- يقبل Torii مواد المصادقة في الرؤوس مثل `Authorization`، و`x-api-token`، و`x-iroha-operator-session`، و`x-iroha-operator-token`، و`x-forwarded-client-cert`.
- وبالتالي يمكن أن يصبح اختراق مخزن السجل أو مجموعة سجل التصحيح أو حزمة الدعم حدثًا للكشف عن بيانات الاعتماد.

العلاج الموصى به:

- التوقف عن تضمين رؤوس الطلب الكاملة في فترات الإنتاج.
- أضف تنقيحًا صريحًا للرؤوس الحساسة للأمان إذا كان تسجيل الرأس لا يزال ضروريًا لتصحيح الأخطاء.
- تعامل مع تسجيل الطلب/الاستجابة على أنه يحمل سرًا بشكل افتراضي ما لم يتم إدراج البيانات بشكل إيجابي في القائمة المسموح بها.

### SA-002 High: لا تزال واجهات برمجة التطبيقات Torii العامة تقبل المفاتيح الخاصة الأولية للتوقيع من جانب الخادم

التأثير: يتم تشجيع العملاء على إرسال المفاتيح الخاصة الأولية عبر الشبكة حتى يتمكن الخادم من التوقيع نيابةً عنهم، مما يؤدي إلى إنشاء قناة كشف سرية غير ضرورية في طبقات API وSDK والوكيل وذاكرة الخادم.

شهادة:- تعلن وثائق مسار الإدارة بوضوح عن التوقيع من جانب الخادم:
  -`crates/iroha_torii/src/gov.rs:495`
- يقوم تنفيذ المسار بتوزيع المفتاح الخاص المقدم والتوقيع من جانب الخادم:
  -`crates/iroha_torii/src/gov.rs:1088`
  -`crates/iroha_torii/src/gov.rs:1091`
  -`crates/iroha_torii/src/gov.rs:1123`
  -`crates/iroha_torii/src/gov.rs:1125`
- تعمل مجموعات SDK على إجراء تسلسل `private_key` بشكل نشط في أجسام JSON:
  -`kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/OfflineToriiClient.kt:47`
  -`kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/OfflineToriiClient.kt:50`
  -`kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/OfflineToriiClient.kt:260`
  -`kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/OfflineToriiClient.kt:261`

ملاحظات:

- لا يقتصر هذا النمط على عائلة مسار واحدة. تحتوي الشجرة الحالية على نفس نموذج الراحة عبر الحوكمة، والنقد غير المتصل بالإنترنت، والاشتراكات، وغيرها من مهام DTO التي تواجه التطبيق.
- تعمل عمليات فحص النقل عبر HTTPS فقط على تقليل النقل العرضي للنص العادي، ولكنها لا تحل المعالجة السرية من جانب الخادم أو مخاطر التعرض للتسجيل/الذاكرة.

العلاج الموصى به:

- إهمال جميع طلبات DTO التي تحمل بيانات `private_key` الأولية.
- مطالبة العملاء بالتوقيع محليًا وتقديم التوقيعات أو المعاملات/المظاريف الموقعة بالكامل.
- قم بإزالة أمثلة `private_key` من OpenAPI/SDKs بعد نافذة التوافق.

### SA-003 Medium: اشتقاق المفتاح السري يرسل مادة أولية سرية إلى Torii ويعيد صدىها مرة أخرى

التأثير: تعمل واجهة برمجة التطبيقات السرية لاشتقاق المفاتيح على تحويل المواد الأولية إلى بيانات حمولة الطلب/الاستجابة العادية، مما يزيد من فرصة الكشف عن البذور من خلال الوكلاء أو البرامج الوسيطة أو السجلات أو عمليات التتبع أو تقارير الأعطال أو إساءة استخدام العميل.

شهادة:- طلب قبول المواد البذور مباشرة:
  -`crates/iroha_torii/src/routing.rs:2736`
  -`crates/iroha_torii/src/routing.rs:2738`
  -`crates/iroha_torii/src/routing.rs:2740`
- يردد مخطط الاستجابة البذرة مرة أخرى في كل من hex وbase64:
  -`crates/iroha_torii/src/routing.rs:2745`
  -`crates/iroha_torii/src/routing.rs:2746`
  -`crates/iroha_torii/src/routing.rs:2747`
- يقوم المعالج بإعادة تشفير البذرة وإرجاعها بشكل صريح:
  -`crates/iroha_torii/src/routing.rs:2797`
  -`crates/iroha_torii/src/routing.rs:2801`
  -`crates/iroha_torii/src/routing.rs:2802`
  -`crates/iroha_torii/src/routing.rs:2804`
- يعرض Swift SDK هذا كطريقة شبكة عادية ويستمر في تكرار صدى البذرة في نموذج الاستجابة:
  -`IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:4716`
  -`IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:4717`
  -`IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:4718`
  -`IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:11912`
  -`IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:11926`

العلاج الموصى به:

- تفضيل اشتقاق المفتاح المحلي في كود CLI/SDK وإزالة مسار الاشتقاق البعيد بالكامل.
- إذا كان يجب أن يبقى المسار، فلا تقم أبدًا بإعادة البذور في الاستجابة ووضع علامة على الأجسام الحاملة للبذور على أنها حساسة في جميع حراس النقل ومسارات القياس عن بعد/تسجيل الدخول.

### SA-004 Medium: يحتوي اكتشاف حساسية نقل SDK على نقاط عمياء للمواد السرية غير `private_key`

التأثير: ستفرض بعض مجموعات SDK بروتوكول HTTPS لطلبات `private_key` الأولية، ولكنها ستسمح لمواد الطلب الأخرى الحساسة للأمان بالانتقال عبر HTTP غير الآمن أو إلى مضيفين غير متطابقين.

شهادة:- يتعامل Swift مع رؤوس مصادقة الطلب الأساسية باعتبارها حساسة:
  -`IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:4`
  -`IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:7`
  -`IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:8`
  -`IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:9`
  -`IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:10`
- لكن Swift لا يزال يطابق الجسم فقط على `"private_key"`:
  -`IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:18`
  -`IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:66`
  -`IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:69`
- يتعرف Kotlin فقط على الرؤوس `authorization` و`x-api-token`، ثم يعود إلى نفس النص الإرشادي `"private_key"`:
  -`kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/TransportSecurity.kt:50`
  -`kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/TransportSecurity.kt:53`
  -`kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/TransportSecurity.kt:58`
  -`kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/TransportSecurity.kt:61`
- Java/Android لها نفس القيود:
  -`java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/TransportSecurity.java:24`
  -`java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/TransportSecurity.java:25`
  -`java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/TransportSecurity.java:100`
  -`java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/TransportSecurity.java:104`
- ينشئ موقعو الطلبات الأساسية لـ Kotlin/Java رؤوس مصادقة إضافية غير مصنفة على أنها حساسة بواسطة حراس النقل الخاصين بهم:
  -`kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/CanonicalRequestSigner.kt:17`
  -`kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/CanonicalRequestSigner.kt:18`
  -`kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/CanonicalRequestSigner.kt:51`
  -`java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/CanonicalRequestSigner.java:25`
  -`java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/CanonicalRequestSigner.java:26`
  -`java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/CanonicalRequestSigner.java:27`
  -`java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/CanonicalRequestSigner.java:28`

العلاج الموصى به:

- استبدال مسح الجسم الإرشادي بتصنيف الطلب الصريح.
- التعامل مع رؤوس المصادقة الأساسية، وحقول البذور/عبارة المرور، ورؤوس الطفرات الموقعة، وأي حقول تحمل أسرارًا مستقبلية باعتبارها حساسة بموجب العقد، وليس عن طريق مطابقة السلسلة الفرعية.
- حافظ على محاذاة قواعد الحساسية عبر Swift وKotlin وJava.

### SA-005 Medium: المرفق "sandbox" ليس سوى عملية فرعية بالإضافة إلى `setrlimit`التأثير: تم وصف أداة معالجة المرفقات والإبلاغ عنها على أنها "وضع الحماية"، لكن التنفيذ ما هو إلا شوكة/تنفيذ للثنائي الحالي مع حدود الموارد. سيستمر تنفيذ المحلل اللغوي أو استغلال الأرشيف بنفس المستخدم وعرض نظام الملفات وامتيازات الشبكة/العملية المحيطة مثل Torii.

الأدلة:

- يحدد المسار الخارجي النتيجة على أنها وضع الحماية بعد تفريخ الطفل:
  -`crates/iroha_torii/src/zk_attachments.rs:756`
  -`crates/iroha_torii/src/zk_attachments.rs:760`
  -`crates/iroha_torii/src/zk_attachments.rs:776`
  -`crates/iroha_torii/src/zk_attachments.rs:782`
- الطفل الافتراضي هو الملف القابل للتنفيذ الحالي:
  -`crates/iroha_torii/src/zk_attachments.rs:913`
  -`crates/iroha_torii/src/zk_attachments.rs:919`
- تعود العملية الفرعية بشكل صريح إلى `AttachmentSanitizerMode::InProcess`:
  -`crates/iroha_torii/src/zk_attachments.rs:1794`
  -`crates/iroha_torii/src/zk_attachments.rs:1803`
- التصلب الوحيد المطبق هو وحدة المعالجة المركزية/مساحة العنوان `setrlimit`:
  -`crates/iroha_torii/src/zk_attachments.rs:1845`
  -`crates/iroha_torii/src/zk_attachments.rs:1850`
  -`crates/iroha_torii/src/zk_attachments.rs:1851`
  -`crates/iroha_torii/src/zk_attachments.rs:1872`

العلاج الموصى به:

- إما تنفيذ صندوق حماية حقيقي لنظام التشغيل (على سبيل المثال مساحات الأسماء/seccomp/landlock/jail-style العزلة، إسقاط الامتيازات، عدم وجود شبكة، نظام ملفات مقيد) أو التوقف عن تسمية النتيجة على أنها `sandboxed`.
- تعامل مع التصميم الحالي على أنه "عزل للعملية الفرعية" بدلاً من "وضع الحماية" في واجهات برمجة التطبيقات والقياس عن بعد والمستندات حتى يتم وجود عزل حقيقي.

### SA-006 Medium: يقوم P2P TLS/QUIC الاختياري بنقل تعطيل التحقق من الشهادةالتأثير: عند تمكين `quic` أو `p2p_tls`، توفر القناة التشفير ولكنها لا تقوم بمصادقة نقطة النهاية البعيدة. لا يزال بإمكان المهاجم النشط على المسار ترحيل القناة أو إنهائها، مما يؤدي إلى هزيمة مشغلي التوقعات الأمنية العادية المرتبطة بـ TLS/QUIC.

الأدلة:

- يوثق QUIC بشكل صريح التحقق من الشهادة المسموح بها:
  -`crates/iroha_p2p/src/transport.rs:12`
  -`crates/iroha_p2p/src/transport.rs:13`
  -`crates/iroha_p2p/src/transport.rs:14`
  -`crates/iroha_p2p/src/transport.rs:15`
- يقبل مدقق QUIC شهادة الخادم دون قيد أو شرط:
  -`crates/iroha_p2p/src/transport.rs:33`
  -`crates/iroha_p2p/src/transport.rs:35`
  -`crates/iroha_p2p/src/transport.rs:44`
  -`crates/iroha_p2p/src/transport.rs:112`
  -`crates/iroha_p2p/src/transport.rs:114`
  -`crates/iroha_p2p/src/transport.rs:115`
- يقوم نقل TLS-over-TCP بنفس الشيء:
  -`crates/iroha_p2p/src/transport.rs:229`
  -`crates/iroha_p2p/src/transport.rs:232`
  -`crates/iroha_p2p/src/transport.rs:241`
  -`crates/iroha_p2p/src/transport.rs:279`
  -`crates/iroha_p2p/src/transport.rs:281`
  -`crates/iroha_p2p/src/transport.rs:282`

العلاج الموصى به:

- إما التحقق من شهادات النظير أو إضافة ربط قناة صريح بين المصافحة الموقعة للطبقة العليا وجلسة النقل.
- إذا كان السلوك الحالي مقصودًا، فأعد تسمية/توثيق الميزة على أنها نقل مشفر غير مصادق عليه حتى لا يخطئ المشغلون في الخلط بينه وبين مصادقة نظير TLS الكاملة.

## أمر المعالجة الموصى به1. قم بإصلاح SA-001 على الفور عن طريق تنقيح تسجيل الرأس أو تعطيله.
2. قم بتصميم وشحن خطة ترحيل لـ SA-002 بحيث تتوقف المفاتيح الخاصة الأولية عن تجاوز حدود واجهة برمجة التطبيقات (API).
3. قم بإزالة أو تضييق مسار اشتقاق المفتاح السري البعيد وتصنيف الأجسام الحاملة للبذور على أنها حساسة.
4. قم بمحاذاة قواعد حساسية نقل SDK عبر Swift/Kotlin/Java.
5. قرر ما إذا كان الصرف الصحي المرفق يحتاج إلى صندوق رمل حقيقي أو إعادة تسمية/إعادة تحديد نطاق صادقة.
6. توضيح وتعزيز نموذج التهديد P2P TLS/QUIC قبل أن يقوم المشغلون بتمكين عمليات النقل التي تتوقع TLS مصادق عليها.

## ملاحظات التحقق من الصحة

- تم اجتياز `cargo check -p iroha_torii --lib --message-format short`.
- تم اجتياز `cargo check -p iroha_p2p --message-format short`.
- تم تمرير `cargo deny check advisories bans sources --hide-inclusion-graph` بعد الركض خارج وضع الحماية؛ لقد أصدرت تحذيرات مكررة الإصدار ولكنها أبلغت عن `advisories ok, bans ok, sources ok`.
- تم بدء اختبار Torii المركّز لمسار مجموعة مفاتيح الاشتقاق السري أثناء عملية التدقيق هذه ولكنه لم يكتمل قبل كتابة التقرير؛ يتم دعم النتيجة من خلال التفتيش المباشر للمصدر بغض النظر.