<!-- Auto-generated stub for Urdu (ur) translation. Replace this content with the full translation. -->

---
lang: ur
direction: rtl
source: security_audit_report.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4343a8f72823a2539a2e79e80a1c7162d455e03b450e2fd613f01ac5c61aec63
source_last_modified: "2026-03-26T13:01:48.980087+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# سیکیورٹی آڈٹ رپورٹ

تاریخ: 26-03-2026

## ایگزیکٹو خلاصہ

اس آڈٹ نے موجودہ درخت میں سب سے زیادہ خطرے والی سطحوں پر توجہ مرکوز کی: Torii HTTP/API/auth فلوز، P2P ٹرانسپورٹ، خفیہ ہینڈلنگ APIs، SDK ٹرانسپورٹ گارڈز، اور اٹیچمنٹ سینیٹائزر پاتھ۔

مجھے 6 قابل عمل مسائل ملے:

- 2 اعلی شدت کے نتائج
- 4 درمیانی شدت کے نتائج

سب سے اہم مسائل یہ ہیں:

1. Torii فی الحال ہر HTTP درخواست کے لیے ان باؤنڈ درخواست ہیڈر لاگ کرتا ہے، جو بیئرر ٹوکنز، API ٹوکنز، آپریٹر سیشن/بوٹسٹریپ ٹوکنز، اور mTLS مارکر کو لاگز میں آگے بڑھا سکتا ہے۔
2. متعدد عوامی Torii روٹس اور SDKs اب بھی سرور کو خام `private_key` اقدار بھیجنے کی حمایت کرتے ہیں تاکہ Torii کال کرنے والے کی جانب سے دستخط کر سکے۔
3. کئی "خفیہ" راستوں کو عام درخواست کے اداروں کے طور پر سمجھا جاتا ہے، بشمول خفیہ بیج سے اخذ کرنا اور کچھ SDKs میں کینونیکل درخواست کی توثیق۔

##طریقہ

- Torii، P2P، crypto/VM، اور SDK خفیہ ہینڈلنگ کے راستوں کا جامد جائزہ
- ھدف شدہ توثیق کے احکامات:
  - `cargo check -p iroha_torii --lib --message-format short` -> پاس
  - `cargo check -p iroha_p2p --message-format short` -> پاس
  - `cargo test -p iroha_torii --lib confidential_derive_keyset_endpoint_roundtrip -- --nocapture` -> پاس
  - `cargo deny check advisories bans sources --hide-inclusion-graph` -> پاس، صرف ڈپلیکیٹ ورژن وارننگز
- اس پاس میں مکمل نہیں ہوا:
  - مکمل ورک اسپیس بلڈ/ٹیسٹ/کلپی
  - سوئفٹ/گریڈل ٹیسٹ سویٹس
  - CUDA/میٹل رن ٹائم کی توثیق

## نتائج

### SA-001 ہائی: Torii عالمی سطح پر حساس درخواست کے سرخیوں کو لاگ کرتا ہےاثر: کوئی بھی تعیناتی جس میں بحری جہاز ٹریسنگ کی درخواست کرتا ہے وہ بیئرر/API/آپریٹر ٹوکنز اور متعلقہ تصدیقی مواد کو ایپلیکیشن لاگز میں لیک کر سکتا ہے۔

ثبوت:

- `crates/iroha_torii/src/lib.rs:20752` `TraceLayer::new_for_http()` کو قابل بناتا ہے
- `crates/iroha_torii/src/lib.rs:20753` `DefaultMakeSpan::default().include_headers(true)` کو قابل بناتا ہے
- حساس ہیڈر کے نام فعال طور پر اسی سروس میں کہیں اور استعمال ہوتے ہیں:
  - `crates/iroha_torii/src/operator_auth.rs:40`
  - `crates/iroha_torii/src/operator_auth.rs:41`
  - `crates/iroha_torii/src/operator_auth.rs:42`
  - `crates/iroha_torii/src/operator_auth.rs:43`

یہ کیوں اہم ہے:

- `include_headers(true)` مکمل ان باؤنڈ ہیڈر ویلیوز کو ٹریسنگ اسپین میں ریکارڈ کرتا ہے۔
- Torii ہیڈرز میں تصدیقی مواد کو قبول کرتا ہے جیسے `Authorization`, `x-api-token`, `x-iroha-operator-session`, `x-iroha-operator-token`, اور `x-forwarded-client-cert`۔
- لاگ سنک کمپرومائز، ڈیبگ لاگ کلیکشن، یا سپورٹ بنڈل اس لیے ایک اسنادی انکشاف کا واقعہ بن سکتا ہے۔

تجویز کردہ تدارک:

- پروڈکشن اسپین میں مکمل درخواست ہیڈر شامل کرنا بند کریں۔
- اگر ڈیبگنگ کے لیے ہیڈر لاگنگ کی ضرورت ہو تو سیکیورٹی کے لیے حساس ہیڈر کے لیے واضح ترمیم شامل کریں۔
- درخواست/جواب لاگنگ کو بطور ڈیفالٹ سیکرٹ بیئرنگ سمجھیں جب تک کہ ڈیٹا کو مثبت طور پر اجازت یافتہ درج نہ کیا جائے۔

### SA-002 ہائی: پبلک Torii APIs اب بھی سرور سائیڈ سائننگ کے لیے خام نجی کلیدوں کو قبول کرتے ہیں

اثر: کلائنٹس کو نیٹ ورک پر خام نجی چابیاں منتقل کرنے کی ترغیب دی جاتی ہے تاکہ سرور API، SDK، پراکسی، اور سرور-میموری پرتوں پر ایک غیر ضروری خفیہ-ایکسپوزر چینل بنا کر، ان کی طرف سے دستخط کر سکے۔

ثبوت:- گورننس روٹ دستاویزات واضح طور پر سرور سائڈ دستخط کی تشہیر کرتی ہیں:
  - `crates/iroha_torii/src/gov.rs:495`
- روٹ کا نفاذ فراہم کردہ نجی کلید کو پارس کرتا ہے اور سرور کی طرف اشارہ کرتا ہے:
  - `crates/iroha_torii/src/gov.rs:1088`
  - `crates/iroha_torii/src/gov.rs:1091`
  - `crates/iroha_torii/src/gov.rs:1123`
  - `crates/iroha_torii/src/gov.rs:1125`
- SDKs فعال طور پر `private_key` کو JSON باڈیز میں سیریلائز کرتے ہیں:
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/OfflineToriiClient.kt:47`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/OfflineToriiClient.kt:50`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/OfflineToriiClient.kt:260`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/OfflineToriiClient.kt:261`

نوٹس:

- یہ پیٹرن ایک روٹ فیملی سے الگ تھلگ نہیں ہے۔ موجودہ ٹری گورننس، آف لائن کیش، سبسکرپشنز، اور دیگر ایپ کا سامنا کرنے والے DTOs میں ایک ہی سہولت ماڈل پر مشتمل ہے۔
- HTTPS-صرف نقل و حمل کی جانچ حادثاتی سادہ متن کی نقل و حمل کو کم کرتی ہے، لیکن وہ سرور کی طرف خفیہ ہینڈلنگ یا لاگنگ/میموری کی نمائش کے خطرے کو حل نہیں کرتے ہیں۔

تجویز کردہ تدارک:

- تمام درخواست DTOs کو مسترد کریں جو خام `private_key` ڈیٹا رکھتے ہیں۔
- مؤکلوں سے مقامی طور پر دستخط کرنے اور دستخط یا مکمل دستخط شدہ لین دین/لفافے جمع کرانے کی ضرورت ہے۔
- مطابقت والی ونڈو کے بعد OpenAPI/SDKs سے `private_key` مثالیں ہٹا دیں۔

### SA-003 میڈیم: خفیہ کلید اخذ کرنا خفیہ بیج کا مواد Torii کو بھیجتا ہے اور اس کی بازگشت واپس کرتا ہے۔

اثر: خفیہ کلیدی اخذ کرنے والا API بیج کے مواد کو عام درخواست/جوابی پے لوڈ ڈیٹا میں بدل دیتا ہے، جس سے پراکسی، مڈل ویئر، لاگز، ٹریس، کریش رپورٹس، یا کلائنٹ کے غلط استعمال کے ذریعے بیج کے انکشاف کے امکانات بڑھ جاتے ہیں۔

ثبوت:- درخواست براہ راست بیج کے مواد کو قبول کرتی ہے:
  - `crates/iroha_torii/src/routing.rs:2736`
  - `crates/iroha_torii/src/routing.rs:2738`
  - `crates/iroha_torii/src/routing.rs:2740`
- رسپانس اسکیما ہیکس اور بیس 64 دونوں میں بیج کی بازگشت کرتا ہے:
  - `crates/iroha_torii/src/routing.rs:2745`
  - `crates/iroha_torii/src/routing.rs:2746`
  - `crates/iroha_torii/src/routing.rs:2747`
- ہینڈلر واضح طور پر دوبارہ انکوڈ کرتا ہے اور بیج کو واپس کرتا ہے:
  - `crates/iroha_torii/src/routing.rs:2797`
  - `crates/iroha_torii/src/routing.rs:2801`
  - `crates/iroha_torii/src/routing.rs:2802`
  - `crates/iroha_torii/src/routing.rs:2804`
- Swift SDK اسے ایک باقاعدہ نیٹ ورک کے طریقہ کار کے طور پر ظاہر کرتا ہے اور رسپانس ماڈل میں ایکو شدہ بیج کو برقرار رکھتا ہے:
  - `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:4716`
  - `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:4717`
  - `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:4718`
  - `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:11912`
  - `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:11926`

تجویز کردہ تدارک:

- CLI/SDK کوڈ میں مقامی کلیدی اخذ کو ترجیح دیں اور ریموٹ ڈیریویشن روٹ کو مکمل طور پر ہٹا دیں۔
- اگر راستہ باقی رہنا چاہیے تو جواب میں کبھی بھی بیج واپس نہ کریں اور تمام ٹرانسپورٹ گارڈز اور ٹیلی میٹری/لاگنگ کے راستوں میں بیج والے جسم کو حساس کے طور پر نشان زد نہ کریں۔

### SA-004 میڈیم: SDK ٹرانسپورٹ کی حساسیت کا پتہ لگانے میں غیر `private_key` خفیہ مواد کے لیے اندھے دھبے ہیں

اثر: کچھ SDKs خام `private_key` درخواستوں کے لیے HTTPS کو نافذ کریں گے، لیکن پھر بھی غیر محفوظ HTTP پر یا غیر مماثل میزبانوں تک سفر کرنے کے لیے دیگر حفاظتی حساس درخواست کے مواد کی اجازت دیتے ہیں۔

ثبوت:- Swift کیننیکل درخواست کے تصنیف ہیڈر کو حساس سمجھتا ہے:
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:4`
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:7`
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:8`
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:9`
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:10`
- لیکن سوئفٹ اب بھی `"private_key"` پر صرف باڈی میچ کرتا ہے:
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:18`
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:66`
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:69`
- کوٹلن صرف `authorization` اور `x-api-token` ہیڈر کو پہچانتا ہے، پھر اسی `"private_key"` باڈی ہیورسٹک پر واپس آتا ہے:
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/TransportSecurity.kt:50`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/TransportSecurity.kt:53`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/TransportSecurity.kt:58`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/TransportSecurity.kt:61`
- Java/Android کی ایک ہی حد ہے:
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/TransportSecurity.java:24`
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/TransportSecurity.java:25`
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/TransportSecurity.java:100`
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/TransportSecurity.java:104`
- کوٹلن/جاوا کینونیکل درخواست کے دستخط کنندگان اضافی تصنیف ہیڈر تیار کرتے ہیں جو ان کے اپنے ٹرانسپورٹ گارڈز کے ذریعہ حساس کے طور پر درجہ بندی نہیں کرتے ہیں:
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/CanonicalRequestSigner.kt:17`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/CanonicalRequestSigner.kt:18`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/CanonicalRequestSigner.kt:51`
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/CanonicalRequestSigner.java:25`
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/CanonicalRequestSigner.java:26`
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/CanonicalRequestSigner.java:27`
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/CanonicalRequestSigner.java:28`

تجویز کردہ تدارک:

- ہیورسٹک باڈی اسکیننگ کو واضح درخواست کی درجہ بندی کے ساتھ تبدیل کریں۔
- کینونیکل تصنیف ہیڈر، بیج/پاسفریز فیلڈز، دستخط شدہ میوٹیشن ہیڈر، اور مستقبل کے کسی بھی خفیہ اثر والے فیلڈز کو معاہدہ کے لحاظ سے حساس سمجھیں، نہ کہ سبسٹرنگ میچ کے ذریعے۔
- حساسیت کے قوانین کو سوئفٹ، کوٹلن اور جاوا میں مربوط رکھیں۔

### SA-005 میڈیم: منسلکہ "سینڈ باکس" صرف ایک ذیلی عمل پلس `setrlimit` ہےاثر: اٹیچمنٹ سینیٹائزر کو "سینڈ باکسڈ" کے طور پر بیان کیا گیا ہے اور اس کی اطلاع دی گئی ہے، لیکن عمل درآمد وسائل کی حدود کے ساتھ موجودہ بائنری کا صرف ایک فورک/ایگزیک ہے۔ ایک تجزیہ کار یا آرکائیو استحصال اب بھی اسی صارف، فائل سسٹم ویو، اور محیطی نیٹ ورک/پروسیس مراعات کے ساتھ Torii کے طور پر انجام پائے گا۔

ثبوت:

- بچے کو جنم دینے کے بعد بیرونی راستہ نتیجہ کو سینڈ باکس کے طور پر نشان زد کرتا ہے:
  - `crates/iroha_torii/src/zk_attachments.rs:756`
  - `crates/iroha_torii/src/zk_attachments.rs:760`
  - `crates/iroha_torii/src/zk_attachments.rs:776`
  - `crates/iroha_torii/src/zk_attachments.rs:782`
- بچہ موجودہ ایگزیکیوٹیبل پر ڈیفالٹ ہے:
  - `crates/iroha_torii/src/zk_attachments.rs:913`
  - `crates/iroha_torii/src/zk_attachments.rs:919`
- ذیلی عمل واضح طور پر `AttachmentSanitizerMode::InProcess` میں واپس چلا جاتا ہے:
  - `crates/iroha_torii/src/zk_attachments.rs:1794`
  - `crates/iroha_torii/src/zk_attachments.rs:1803`
- صرف سختی کا اطلاق CPU/address-space `setrlimit` ہے:
  - `crates/iroha_torii/src/zk_attachments.rs:1845`
  - `crates/iroha_torii/src/zk_attachments.rs:1850`
  - `crates/iroha_torii/src/zk_attachments.rs:1851`
  - `crates/iroha_torii/src/zk_attachments.rs:1872`

تجویز کردہ تدارک:

- یا تو ایک حقیقی OS سینڈ باکس نافذ کریں (مثال کے طور پر namespaces/seccomp/landlock/جیل طرز کی تنہائی، استحقاق ڈراپ، بغیر نیٹ ورک، محدود فائل سسٹم) یا نتیجہ کو `sandboxed` کا لیبل لگانا بند کریں۔
- موجودہ ڈیزائن کو APIs، ٹیلی میٹری، اور دستاویزات میں "سینڈ باکسنگ" کے بجائے "سب پروسیس آئسولیشن" سمجھیں جب تک کہ حقیقی تنہائی موجود نہ ہو۔

### SA-006 میڈیم: اختیاری P2P TLS/QUIC ٹرانسپورٹ سرٹیفکیٹ کی تصدیق کو غیر فعال کر دیتا ہےاثر: جب `quic` یا `p2p_tls` فعال ہوتا ہے، تو چینل انکرپشن فراہم کرتا ہے لیکن ریموٹ اینڈ پوائنٹ کی توثیق نہیں کرتا ہے۔ ایک فعال آن-پاتھ حملہ آور پھر بھی چینل کو ریلے یا ختم کر سکتا ہے، TLS/QUIC کے ساتھ منسلک آپریٹرز کی سیکیورٹی کی عام توقعات کو شکست دے کر۔

ثبوت:

- QUIC واضح طور پر اجازت دینے والے سرٹیفکیٹ کی تصدیق کرتا ہے:
  - `crates/iroha_p2p/src/transport.rs:12`
  - `crates/iroha_p2p/src/transport.rs:13`
  - `crates/iroha_p2p/src/transport.rs:14`
  - `crates/iroha_p2p/src/transport.rs:15`
- QUIC تصدیق کنندہ غیر مشروط طور پر سرور سرٹیفکیٹ کو قبول کرتا ہے:
  - `crates/iroha_p2p/src/transport.rs:33`
  - `crates/iroha_p2p/src/transport.rs:35`
  - `crates/iroha_p2p/src/transport.rs:44`
  - `crates/iroha_p2p/src/transport.rs:112`
  - `crates/iroha_p2p/src/transport.rs:114`
  - `crates/iroha_p2p/src/transport.rs:115`
- TLS-over-TCP ٹرانسپورٹ بھی یہی کرتی ہے:
  - `crates/iroha_p2p/src/transport.rs:229`
  - `crates/iroha_p2p/src/transport.rs:232`
  - `crates/iroha_p2p/src/transport.rs:241`
  - `crates/iroha_p2p/src/transport.rs:279`
  - `crates/iroha_p2p/src/transport.rs:281`
  - `crates/iroha_p2p/src/transport.rs:282`

تجویز کردہ تدارک:

- یا تو ہم مرتبہ سرٹیفکیٹس کی تصدیق کریں یا ہائی لیئر پر دستخط شدہ ہینڈ شیک اور ٹرانسپورٹ سیشن کے درمیان واضح چینل بائنڈنگ شامل کریں۔
- اگر موجودہ رویہ جان بوجھ کر ہے، تو فیچر کا نام تبدیل کریں/دستاویزی طور پر غیر تصدیق شدہ انکرپٹڈ ٹرانسپورٹ کریں تاکہ آپریٹرز اسے مکمل TLS پیر کی توثیق کرنے کی غلطی نہ کریں۔

## تجویز کردہ اصلاحی حکم1. ہیڈر لاگنگ کو رد یا غیر فعال کرکے SA-001 کو فوری طور پر درست کریں۔
2. SA-002 کے لیے ہجرت کا منصوبہ ڈیزائن اور بھیجیں تاکہ خام نجی کلیدیں API کی حد کو عبور کرنا بند کر دیں۔
3. دور دراز کے رازدارانہ کلیدی اخذ کرنے کے راستے کو ہٹائیں یا تنگ کریں اور بیج والے جسموں کو حساس کے طور پر درجہ بندی کریں۔
4. Swift/Kotlin/Java پر SDK ٹرانسپورٹ کی حساسیت کے قوانین کو سیدھ میں رکھیں۔
5. فیصلہ کریں کہ اٹیچمنٹ کی صفائی کے لیے ایک حقیقی سینڈ باکس کی ضرورت ہے یا ایماندارانہ نام تبدیل/دوبارہ اسکوپنگ کی ضرورت ہے۔
6. P2P TLS/QUIC تھریٹ ماڈل کو واضح اور سخت کریں اس سے پہلے کہ آپریٹرز تصدیق شدہ TLS کی توقع رکھنے والے ٹرانسپورٹ کو فعال کریں۔

## توثیق کے نوٹس

- `cargo check -p iroha_torii --lib --message-format short` گزر گیا۔
- `cargo check -p iroha_p2p --message-format short` گزر گیا۔
- سینڈ باکس کے باہر بھاگنے کے بعد `cargo deny check advisories bans sources --hide-inclusion-graph` گزر گیا؛ اس نے ڈپلیکیٹ ورژن وارننگز کو خارج کیا لیکن `advisories ok, bans ok, sources ok` کی اطلاع دی۔
- اس آڈٹ کے دوران خفیہ اخذ کی سیٹ روٹ کے لیے ایک فوکسڈ Torii ٹیسٹ شروع کیا گیا تھا لیکن رپورٹ لکھے جانے سے پہلے مکمل نہیں ہوا تھا۔ تلاش کو براہ راست ماخذ کے معائنہ سے قطع نظر حمایت حاصل ہے۔