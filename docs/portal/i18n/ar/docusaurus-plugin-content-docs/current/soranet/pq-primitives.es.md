---
lang: ar
direction: rtl
source: docs/portal/docs/soranet/pq-primitives.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: pq-الأوليات
العنوان: Primitivas poscuanticas de SoraNet
Sidebar_label: Primitivas PQ
الوصف: استئناف الصندوق `soranet_pq` ومصافحة SoraNet تستهلك المساعدين ML-KEM/ML-DSA.
---

:::ملاحظة فوينتي كانونيكا
هذه الصفحة تعكس `docs/source/soranet/pq_primitives.md`. احتفظ بالإصدارات المتزامنة حتى يتم سحب مجموعة المستندات المتوارثة.
:::

يحتوي الصندوق `soranet_pq` على الكتل المحتملة التي تثق بجميع المرحلات والعملاء ومكونات أدوات SoraNet. قم بتضمين مجموعات Kyber (ML-KEM) وDilithium (ML-DSA) المدعومة من قبل PQClean وجمع مساعدي HKDF وRNG المحميين للبروتوكول بحيث تشترك جميع الأسطح في تنفيذ متطابق.

## يشمل هذا `soranet_pq`

- **ML-KEM-512/768/1024:** توليد حتمية المفاتيح والتغليف وإلغاء التغليف مع نشر الأخطاء في وقت ثابت.
- **ML-DSA-44/65/87:** تأكيد/تحقق منفصل للنسخ مع فصل السيادة.
- **آداب HKDF:** `derive_labeled_hkdf` يجمع مساحة الاسم مع كل اشتقاق مع لمسة المصافحة (`DH/es`، `KEM/1`، ...) حتى لا يتم تصادم النسخ الهجينة.
- **التعديلات المحوطة:** `hedged_chacha20_rng` مزيج من الحلقات الحتمية مع إنتروبيا SO مما يؤدي إلى تحديد الحالة المتوسطة لتحرير الموارد.تعمل جميع الأسرار داخل الحاويات `Zeroizing` وCI على تشغيل روابط PQClean في جميع المنصات المدعومة.

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

## كمستهلك

1. **إضافة التبعية** إلى الصناديق التي توجد بها جذر مساحة العمل:

   ```toml
   soranet_pq = { path = "../../crates/soranet_pq" }
   ```

2. **حدد المجموعة الصحيحة** في نقاط الاتصال. للعمل الأولي في المصافحة الهجينة، الولايات المتحدة الأمريكية `MlKemSuite::MlKem768` و`MlDsaSuite::MlDsa65`.

3. **اشتقاق المفاتيح مع الآداب.** استخدم `HkdfDomain::soranet("KEM/1")` (و ما يعادله) بحيث سيشعر تسلسل النسخ بالتحديد بين العقد.

4. **استخدام RNG المتحوط** لنشر أسرار الاستجابة:

   ```rust
   use soranet_pq::{hedged_chacha20_rng, HedgedRngSeed};

   let mut rng = hedged_chacha20_rng(HedgedRngSeed::new(b"snnet16", [0u8; 32]));
   ```

تستهلك المصافحة المركزية لـ SoraNet ومساعدي CID (`iroha_crypto::soranet`) هذه الاستخدامات مباشرة، مما يعني أن الصناديق الموجودة في اتجاه مجرى النهر تنقل نفس التنفيذ بدون ربط روابط PQClean بنفس الطريقة.

## قائمة التحقق من الصحة

-`cargo test -p soranet_pq --offline`
-`cargo fmt --package soranet_pq`
- مراجعة نماذج استخدام README (`crates/soranet_pq/README.md`)
- تحديث مستند تصميم المصافحة من SoraNet عند مقارنتها بالهجينة