---
lang: ar
direction: rtl
source: docs/source/bridge_proofs.md
status: needs-review
generator: scripts/sync_docs_i18n.py
source_hash: 74e29801129deccb6d5640d414289c47cf13fa9e0229fb55212b6c7710d7c5f7
source_last_modified: "2026-07-12T07:38:49.568351+00:00"
translation_last_reviewed: 2026-07-12
translator: machine-assisted
---

<div dir="rtl">

> هذه خلاصة مترجمة ومختصرة، وليست ترجمة كاملة. يبقى
> [النص الإنجليزي المرجعي](bridge_proofs.md) المصدر المعياري الدقيق لقواعد
> الحوكمة وواجهات API ودلالات الإثبات ومتطلبات الإصدار.

# إثباتات جسر SCCP V1 — خلاصة مختصرة

## نطاق الإصدار الأول

SCCP V1 بروتوكول مغلق للإصدار الأول. المصادر الخارجية المدعومة حصراً هي
`ethereum-mainnet` و`bsc-mainnet` و`tron-mainnet`، ووجهة SORA الوحيدة هي
`sora-taira`. لا يدعم هذا الإصدار Solana أو TON أو شبكة مخصصة أو أي وجهة SORA
أخرى؛ وتُرفض هذه القيم بصورة مغلقة وآمنة.

تقبل `SubmitBridgeProof` في هذا الإصدار الإثباتات النمطية `NativeProtocol`
و`SccpDestination` فقط. إرسال `Ics` العام أو `TransparentZk` غير متاح ويُرفض
حتى تتوافر له جهة تحقق سلطوية على السلسلة.

## السجل النمطي والحماية من إعادة التشغيل

`SccpRegistryV1` سجل نمطي مربوط بالمسار (lane) ولا يسمح إلا بالإضافة
(append-only). يحتفظ كل مسار بحد أقصى قدره 64 مراجعة route و4,096 native trust
anchor. لا تُحذف السجلات القديمة ضمنياً؛ وعند بلوغ حد التاريخ تُرفض الإضافة
التالية ذرّياً من دون تغيير الحالة.

تُقاس فترات anchor بإحداثي تقدم consensus موثّق: تستعمل Ethereum الـ finalized
beacon slot، بينما تستعمل BSC وTRON ارتفاع native block النهائي. يبقى anchor
القديم صالحاً حتى checkpoint اللاحق شاملاً نقطة الحد، ويظل anchor الحالي الأخير
مفتوح النهاية. ويجب أن يطابق finality cutoff لمسار منتهٍ checkpoint اللاحق
للـanchor التاريخي تماماً.

يسجل سجل inbound الدائم كلاً من event/source finality height و
`anchor_interval_height` الموثق. ويرفع فهرس high-water ثابت، مفتاحه lane وanchor
hash، أعلى إحداثي مقبول؛ لذلك لا تستطيع الحوكمة اختيار checkpoint لاحق أدنى من
إثبات قُبل سابقاً. تعيد snapshot hydration حساب الفهرس من السجلات الدائمة
وتشترط التطابق التام، فترفض الفهرس المفقود أو القديم أو المشوّه أو غير المسنود.
وتبقى معرفات الرسائل المستهلكة دائمة لمنع replay.

يستخدم route المصدر في TRON واجهة ABI الدقيقة
`transferToTaira(bytes,uint256,uint64 expectedNonce)`. ولا ينجح التنفيذ إلا إذا
كان `expectedNonce == transferNonce`، ثم يكتب القيمة نفسها في canonical payload
قبل زيادة storage. وتعيد native admission بناء نداء ABI كاملاً من recipient في
الـpayload والمقدار بعد scaling والـnonce؛ لذلك تُرفض بصورة fail-closed كل من
واجهة selector القديمة ذات الوسيطين، وأي nonce قديم أو مستقبلي، واستنفاد نطاق
`uint64` للـnonce.

## تحقق أحادي المرور وحدود العمل

تُفك بنية إثباتات destination وnative مرة واحدة، وتُربط مرة واحدة، ويُحجز العمل
الحتمي قبل بدء التشفير المكلف. يتحقق مسار destination من BN254 pairing-product
ومن BLS finality المحلي مرة واحدة لكل منهما. وتشترط المسارات native أقصر prefix
قانوني؛ الحد الأقصى هو 1,004 headers في BSC و54 headers في TRON.

يفرض `[zk.sccp]` حدوداً غير صفرية لكل transaction ولكل block على عدد الإثباتات
وحجمها، وnative headers/bytes، وEthereum light-client updates، وعمليات استعادة
secp256k1، وفحوص BLS aggregate ومساهمات المفاتيح، وفحوص BN254 pairing. حدود
القبول هذه مرتبطة بالـconsensus، ويجب أن تكون قيم ملف الإعداد متطابقة لدى جميع
المدققين. لا توجد لها بدائل عبر environment variables.

حدود الإصدار الأول الافتراضية هي:

| بُعد العمل | Transaction | Block |
|---|---:|---:|
| proofs | 1 | 4 |
| canonical proof bytes | 8 MiB | 32 MiB |
| BSC/TRON continuation headers | 1,004 | 4,016 |
| Ethereum light-client updates | 128 | 512 |
| framed native-finality bytes | 8 MiB | 32 MiB |
| secp256k1 recoveries | 1,005 | 4,020 |
| BLS aggregate checks | 1,004 | 4,016 |
| BLS key/contribution work items | 131,713 | 526,852 |
| BN254 pairing-product checks | 1 | 4 |

لا يجوز أن يتجاوز proof واحد 8 MiB من canonical bytes. ولا يتسرب العمل المحجوز
من transaction متروكة أو مرفوضة إلى block.

## التزام الرسائل الصادرة والاحتفاظ والاكتشاف

تحصل كل رسالة صادرة ناجحة على `commitment_index` كثيف بترتيب تنفيذ الكتلة
(`0..=511`). يثبت V1 حداً قدره 512 رسالة لكل كتلة و4,096 بايت canonical payload
لكل رسالة. ويحد `[zk.sccp]` حالة الـpayload المعلقة معاً بواسطة
`max_pending_outbound_messages` (الافتراضي `65536`) و
`max_pending_outbound_payload_bytes` (الافتراضي `268435456`).

قبل نشر finality أو إخلاء block body، يحفظ Kura بصورة immutable الـcanonical header
الدقيق وأرشيف SCCP موثَّقاً بالـroot. لذلك لا تحتاج إعادة بناء proof أو bundle أو
proof request أو recent history إلى block body تاريخي أو نسخة WSV قابلة للتغيير.
عند قبول destination proof تُحذف الـpending payload وتكلفتها atomically ويحل محلها
fixed terminal descriptor مع بقاء locator/index. الحالة المعلقة محدودة، أما سجلات
terminal وتاريخ Kura immutable فينميان عمداً للحماية الدائمة من replay. يستخدم
`GET /v1/sccp/messages/recent` المؤشر المركب `{ from, after_index }`. تُحتسب
immutable evidence ضمن total/operator disk usage ولكنها مستثناة من
evictable-body budget.

## حدود Torii وHTTP

يفرض Torii حداً خاصاً بجسم JSON لكل SCCP endpoint قبل قراءة الجسم أو تخصيص
الذاكرة أو التحقق التشفيري. يُرفض `Content-Length` أو الجسم chunked المتجاوز
للحد بالرمز HTTP `413`. كما يقرأ العميل استجابة HTTP بعد فكها ضمن حد ثابت، لذلك
لا يستطيع `Content-Length` المفقود أو الكاذب تجاوز الحد.

يجب أن تكون مدخلات JSON وbase64 وNorito قانونية canonical. وتُرفض الحقول غير
المعروفة والمفاتيح المكررة والشبكة أو route أو anchor غير المطابق وإعادة التشغيل
وتجاوز حصة العمل وفشل التحقق، من دون أي تعديل جزئي للحالة.

</div>
