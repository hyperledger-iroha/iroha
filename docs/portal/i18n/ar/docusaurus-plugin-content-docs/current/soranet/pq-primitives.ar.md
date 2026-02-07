---
lang: ar
direction: rtl
source: docs/portal/docs/soranet/pq-primitives.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: pq-الأوليات
العنوان: بدائيات ما بعد الكم في SoraNet
Sidebar_label: بدائيات PQ
description: نظرة عامة على الصندوق `soranet_pq` وكيف يستهلك مصافحة SoraNet مساعدات ML-KEM/ML-DSA.
---

:::ملاحظة المصدر القياسي
احترام هذه الصفحة `docs/source/soranet/pq_primitives.md`. حافظ على النسختين متطابقتين حتى يتم التقاعد من مجموعة الوثائق القديمة.
:::

يحتوي على صندوق `soranet_pq` على بنات ما بعد الكم التي تعتمد عليها كل التتابع والعميل والأدوات في SoraNet. وهو يطبع مجموعات Kyber (ML-KEM) وDilithium (ML-DSA) المدعومة من PQClean وإضافة مساعدات لـ HKDF وRNG Hedged جزء للبروتوكول حتى تتشارك جميع نفس التنفيذات.

## ما الذي يشمله `soranet_pq`

- **ML-KEM-512/768/1024:** توليد مفاتيح حتمي، ومساعدات التغليف وdecapsulation مع نشر الأخطاء بزمن ثابت.
- **ML-DSA-44/65/87:** توقيع/ تحقق بشكل منفصلان مربوطان بنصوص مفصولة النطاق.
- **HKDF موسوم:** `derive_labeled_hkdf` مساحة الاسم لكل اشتقاق مع مرحلة المصافحة (`DH/es`, `KEM/1`, ...) حتى تستمر النصوص هجينة بلا تصادم.
- **عشوائية محوطة:** `hedged_chacha20_rng` ي الرابط حتمية مع إنتروبيا النظام ويصفر حالة الوسيطة عند التحرير.

تسكن جميع الأسرار داخل فيتامين `Zeroizing` وتختبر CI روابط PQClean على جميع المنصات المدعومة.

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

1. **اضف الاعتماد** إلى الصناديق الموجودة بالخارج جذر مساحة العمل:

   ```toml
   soranet_pq = { path = "../../crates/soranet_pq" }
   ```

2. **اختيار المجموعة الصحيحة** نقاط في الاستدعاء. العمل الأول على المصافحة الهجينة، استخدم `MlKemSuite::MlKem768` و`MlDsaSuite::MlDsa65`.3. **اشتقاق المفاتيح مع وسوم.** استخدم `HkdfDomain::soranet("KEM/1")` (ونظرائه) حتى تبقى سلسلة النصوص حطميا عبر الكون.

4. **استخدم RNG التحوط** عند أخذ عينات لأسرار الاستخراج:

   ```rust
   use soranet_pq::{hedged_chacha20_rng, HedgedRngSeed};

   let mut rng = hedged_chacha20_rng(HedgedRngSeed::new(b"snnet16", [0u8; 32]));
   ```

تسحب مصافحة SoraNet ومساعدات تعريف CID (`iroha_crypto::soranet`) هذه الأدوات مباشرة، ما يعني أن الصناديق التابعة ترث نفس التنفيذات دون ربط PQClean بنفسها.

## قائمة التحقق من صحة

-`cargo test -p soranet_pq --offline`
-`cargo fmt --package soranet_pq`
- أمثلة استخدام في README (`crates/soranet_pq/README.md`)
- حدث وثيقة تصميم مصافحة SoraNet عند وصول الهجائن