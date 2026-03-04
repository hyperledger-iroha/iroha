---
lang: ar
direction: rtl
source: docs/source/confidential_assets/approvals/payload_v1_rollout.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5fa5e39b0e758b38e27855fcfcae9a6e31817df4fdb9d5394b4b63d2f5164516
source_last_modified: "2026-01-22T15:38:30.658233+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! الموافقة على طرح الإصدار الأول من الحمولة النافعة (مجلس SDK، 28-04-2026).
//!
//! يلتقط مذكرة قرار مجلس SDK المطلوبة بواسطة `roadmap.md:M1` لذلك
//! يحتوي إصدار v1 من الحمولة النافعة المشفرة على سجل قابل للتدقيق (قابل للتسليم M1.4).

# قرار طرح الحمولة النافعة الإصدار 1 (28-04-2026)

- **الرئيس:** رئيس مجلس SDK (م. تاكيميا)
- **أعضاء التصويت:** Swift Lead، مشرف CLI، Confidential Assets TL، DevRel WG
- **المراقبون:** إدارة البرنامج، عمليات القياس عن بعد

## تمت مراجعة المدخلات

1. **الروابط والمرسلات السريعة** — `ShieldRequest`/`UnshieldRequest`، والمرسلون غير المتزامنين، ومساعدو Tx builder الذين حصلوا على اختبارات التكافؤ و docs.【IrohaSwift/Sources/IrohaSwift/TxBuilder.swift:389】【IrohaSwift/Sources/IrohaSwift/TxBuilder.swift:1006】
2. **بيئة العمل لـ CLI** — يغطي مساعد `iroha app zk envelope` تشفير/فحص سير العمل بالإضافة إلى تشخيص الأعطال، بما يتماشى مع متطلبات بيئة العمل لخريطة الطريق.
3. **التركيبات الحتمية ومجموعات التكافؤ** — تركيبات مشتركة + التحقق من صحة Rust/Swift للحفاظ على أسطح البايتات/الخطأ Norito الانحياز.[fixtures/confidential/encrypted_payload_v1.json:1] 【crates/iroha_data_model/tests/confidential_encrypted_payload_vectors.rs:1】[IrohaSwift/Tests/IrohaSwiftTests/ConfidentialEncryptedPayloadTests.swift:73]

## القرار

- **الموافقة على طرح الإصدار الأول من الحمولة** لأدوات تطوير البرامج (SDK) وواجهة سطر الأوامر (CLI)، مما يمكّن محافظ Swift من إنشاء مظاريف سرية بدون سباكة مخصصة.
- **الشروط:** 
  - احتفظ بتركيبات التكافؤ ضمن تنبيهات انجراف CI (مرتبطة بـ `scripts/check_norito_bindings_sync.py`).
  - توثيق قواعد اللعبة التشغيلية في `docs/source/confidential_assets.md` (تم تحديثها بالفعل عبر Swift SDK PR).
  - سجل المعايرة + دليل القياس عن بعد قبل قلب أي علامات إنتاج (يتم تتبعها تحت M2).

## عناصر العمل

| المالك | العنصر | مستحق |
|-------|------|-----|
| الرصاص السريع | الإعلان عن توفر GA + مقتطفات README | 2026-05-01 |
| مشرف CLI | إضافة مساعد `iroha app zk envelope --from-fixture` (اختياري) | تراكم (غير محظور) |
| ديفريل WG | قم بتحديث عمليات التشغيل السريع للمحفظة باستخدام تعليمات الحمولة النافعة v1 | 2026-05-05 |

> **ملاحظة:** تحل هذه المذكرة محل استدعاء "في انتظار موافقة المجلس" المؤقت في `roadmap.md:2426` وتلبي عنصر التعقب M1.4. قم بتحديث `status.md` عند إغلاق عناصر إجراء المتابعة.