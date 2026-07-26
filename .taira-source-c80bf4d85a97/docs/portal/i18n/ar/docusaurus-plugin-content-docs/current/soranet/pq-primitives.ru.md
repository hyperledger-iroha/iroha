---
lang: ar
direction: rtl
source: docs/portal/docs/soranet/pq-primitives.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: pq-الأوليات
العنوان: نشر البدائيات SoraNet
Sidebar_label: بدائيات PQ
الوصف: فحص الصندوق `soranet_pq` وأيضًا عندما تستخدم SoraNet المساعدين ML-KEM/ML-DSA.
---

:::note Канонический источник
هذا الجزء من الزر `docs/source/soranet/pq_primitives.md`. قم بالنسخ من خلال المزامنة، وذلك من أجل إنشاء المستندات التي لا تعتمد على الاستثناءات.
:::

يقوم Crate `soranet_pq` بتوصيل الكتل الهيكلية اللاحقة التي تعمل على تشغيل جميع المرحلات والعملاء ومكونات الأدوات SoraNet. من خلال الدمج بين Kyber (ML-KEM) وDilithium (ML-DSA) على أساس PQClean وإضافة مساعدي البروتوكول الإضافي HKDF وRNG المتحوط، من أجل تحسين كل شيء تحقيق متطابق بشكل مختلف.

## ما الذي يحدث في `soranet_pq`

- **ML-KEM-512/768/1024:** تحديد توليد المفاتيح والمساعدين في الكابسولات وأغطية الكبسولات مع الأجهزة الثابتة الثابتة وقت.
- **ML-DSA-44/65/87:** إرسال/تدقيق خاص، إمكانية تحويل النص إلى مجال مختلف.
- **توسيع HKDF:** `derive_labeled_hkdf` يضيف مساحة الاسم لكل اتصال من خلال إنشاء محطة التخزين (`DH/es`, `KEM/1`, ...), لعدم تعقب النسخ الهجينة.
- **حالة التحوط:** `hedged_chacha20_rng` تعمل على تحديد البذور ذات الانتروبيا الحية وتساهم في تعزيز الحالة освободении.كل الأسرار موجودة في الحاوية `Zeroizing`، حيث يقوم CI بربط الروابط PQClean على جميع المنصات الداعمة.

```rust
use soranet_pq::{
    encapsulate_mlkem, decapsulate_mlkem, generate_mlkem_keypair, MlKemSuite,
    derive_labeled_hkdf, HkdfDomain, HkdfSuite,
};

let kem = generate_mlkem_keypair(MlKemSuite::MlKem768);
let (client_secret, ciphertext) = encapsulate_mlkem(MlKemSuite::MlKem768, kem.public_key()).unwrap();
let server_secret = decapsulate_mlkem(MlKemSuite::MlKem768, kem.secret_key(), ciphertext.as_bytes()).unwrap();
assert_eq!(client_secret.as_bytes(), server_secret.as_bytes());

let okm = derive_labeled_hkdf(
    HkdfSuite::Sha3_256,
    None,
    client_secret.as_bytes(),
    HkdfDomain::soranet("KEM/1"),
    b"soranet-transcript",
    32,
).unwrap();
```

## كيفية الاستخدام

1. **إضافة معلومات** في الصناديق في مساحة العمل الرئيسية:

   ```toml
   soranet_pq = { path = "../../crates/soranet_pq" }
   ```

2. **اختر الطريقة الصحيحة** في الاتجاه الصحيح. تستخدم المصافحة الهجينة الأولى `MlKemSuite::MlKem768` و`MlDsaSuite::MlDsa65`.

3. **استخدم المفاتيح باستخدام الأدوات.** استخدم `HkdfDomain::soranet("KEM/1")` (والموثوقة)، لتتمكن من تحديد النسخ الكامل بين الناس.

4. **استخدام RNG المتحوط** عند اختيار الأسرار الأساسية:

   ```rust
   use soranet_pq::{hedged_chacha20_rng, HedgedRngSeed};

   let mut rng = hedged_chacha20_rng(HedgedRngSeed::new(b"snnet16", [0u8; 32]));
   ```

تستخدم شركة SoraNet والمساعدين الرئيسيين لدعم CID (`iroha_crypto::soranet`) هذه الأدوات المساعدة، حيث يتم عرض الصناديق النهائية عليها أيضًا تحقيق بدون ربط الارتباطات المطلوبة PQClean بنفسي.

## تحقق من القائمة

-`cargo test -p soranet_pq --offline`
-`cargo fmt --package soranet_pq`
- التحقق من استخدام التمهيديات في الملف README (`crates/soranet_pq/README.md`)
- التعرف على مستند تصميم تصميم SoraNet بعد هجين الهجين