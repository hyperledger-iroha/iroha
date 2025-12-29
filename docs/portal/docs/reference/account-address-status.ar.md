<!-- Auto-generated stub for Arabic (ar) translation. Replace this content with the full translation. -->

---
lang: ar
direction: rtl
source: docs/portal/docs/reference/account-address-status.md
status: complete
translator: manual
source_hash: 448093b8efd6e92c8265691d6a4dafcf1d3b9c369cc6ae42ee8ae418b27bc36b
source_last_modified: "2025-11-08T16:40:11.557769+00:00"
translation_last_reviewed: 2025-11-14
---

<div dir="rtl">

---
id: account-address-status
title: امتثال عناوين الحساب
description: ملخّص تدفّق الـ fixtures في ADDR‑2 وكيف تحافظ فرق الـ SDK على التزامن.
---

يحفظ الـ bundle الكانوني ADDR‑2 (`fixtures/account/address_vectors.json`) الـ fixtures
الخاصة بـ IH58 والصيغ المضغوطة (full/half‑width) والعناوين متعددة التوقيع (multisig)
والحالات السلبية. تعتمد جميع واجهات الـ SDK والسطوح الخاصة بـ Torii على نفس JSON حتى
نتمكن من اكتشاف أي اختلاف (drift) في الـ codec قبل أن يصل إلى بيئة الإنتاج. تعكس هذه
الصفحة موجز الحالة الداخلي (`docs/source/account_address_status.md` في جذر
المستودع) بحيث يستطيع قرّاء البوابة مراجعة مسار العمل من دون التنقل عبر الـ
mono‑repo بالكامل.

## إعادة توليد أو التحقق من الـ bundle

```bash
# تحديث الـ fixture الكانوني (يكتب fixtures/account/address_vectors.json)
cargo xtask address-vectors --out fixtures/account/address_vectors.json

# الفشل مبكرًا إذا كان الملف الملتزم به قديمًا
cargo xtask address-vectors --verify
```

الخيارات (Flags):

- `--stdout` — يطبع JSON على stdout للفحص الآني (ad‑hoc).
- `--out <path>` — يكتب إلى مسار مختلف (مثلًا عند مقارنة تغييرات محلية).
- `--verify` — يقارن نسخة العمل بالمحتوى الذي تم توليده للتو (لا يمكن استخدامه مع
  `--stdout`).

يُشغِّل مسار CI **Address Vector Drift** الأمر `cargo xtask address-vectors --verify`
كلما تغيّر الـ fixture أو المولّد أو مستندات docs لتنبيه المراجعين فورًا.

## من يستهلك الـ fixture؟

| السطح | التحقق (Validation) |
|-------|----------------------|
| نموذج البيانات في Rust | `crates/iroha_data_model/tests/account_address_vectors.rs` |
| Torii (الخادم) | `crates/iroha_torii/tests/account_address_vectors.rs` |
| JavaScript SDK | `javascript/iroha_js/test/address.test.js` |
| Swift SDK | `IrohaSwift/Tests/IrohaSwiftTests/AccountAddressTests.swift` |
| Android SDK | `java/iroha_android/src/test/java/org/hyperledger/iroha/android/address/AccountAddressTests.java` |

تقوم كل بيئة اختبار (harness) بعمل round‑trip للبايتات الكانونية وصيغة IH58 والصيغ
المضغوطة، كما تتحقق من تطابُق رموز الأخطاء على نمط Norito مع ما هو مذكور في الـ
fixture للحالات السلبية.

## هل تحتاج إلى أتمتة؟

يمكن لبرامج إطلاق الإصدارات (release tooling) أتمتة تحديث الـ fixtures باستخدام
الـ helper `scripts/account_fixture_helper.py`، والذي يقوم بجلب أو التحقق من الـ
bundle الكانوني من دون خطوات copy/paste:

```bash
# تنزيل إلى مسار مخصص (المسار الافتراضي fixtures/account/address_vectors.json)
python3 scripts/account_fixture_helper.py fetch --output path/to/sdk/address_vectors.json

# التحقق من أن نسخة محلية تتطابق مع المصدر الكانوني (HTTPS أو file://)
python3 scripts/account_fixture_helper.py check --target path/to/sdk/address_vectors.json --quiet
```

يدعم الـ helper خيار `--source` أو متغير البيئة `IROHA_ACCOUNT_FIXTURE_URL` حتى
تتمكّن jobs الـ CI الخاصة بالـ SDK من الإشارة إلى الـ mirror المفضّل لديها.

## هل تحتاج إلى الموجز الكامل؟

توجد حالة امتثال ADDR‑2 الكاملة (المالكون، خطة المراقبة، الإجراءات المفتوحة) في
`docs/source/account_address_status.md` داخل المستودع، جنبًا إلى جنب مع RFC بنية
العناوين (`docs/account_structure.md`). استخدم هذه الصفحة كتذكير تشغيلي سريع، وارجع
إلى مستندات الـ repo عند الحاجة إلى إرشادات تفصيلية.

</div>
