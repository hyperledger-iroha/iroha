---
lang: ar
direction: rtl
source: docs/portal/docs/soranet/pq-primitives.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: pq-الأوليات
العنوان: البدائيات بعد الكميات SoraNet
Sidebar_label: البدائيات PQ
الوصف: عرض مجموعة الصندوق `soranet_pq` وطريقة عدم المصافحة SoraNet تستهلك المساعدين ML-KEM/ML-DSA.
---

:::ملاحظة المصدر الكنسي
:::

يحتوي الصندوق `soranet_pq` على قوالب لاحقة الكميات على جميع المرحلات والعملاء ومكونات أدوات SoraNet. تتضمن مجموعة Kyber (ML-KEM) و Dilithium (ML-DSA) PQClean وإضافة مساعدين HKDF و RNG محميين يتكيفان مع بروتوكول لجميع الأسطح التي تشارك في تطبيقات متطابقة.

## Ce qui est livre dans `soranet_pq`

- **ML-KEM-512/768/1024:** تحديد العناصر والتغليف وإلغاء التغليف مع انتشار الأخطاء في وقت ثابت.
- **ML-DSA-44/65/87:** التوقيع/التحقق من الفصل مع النسخ وفصل المجال.
- **آداب HKDF:** `derive_labeled_hkdf` يطبق مساحة اسم على كل اشتقاق عبر شريط المصافحة (`DH/es`، `KEM/1`، ...) حتى تبقى النسخ الهجينة دون تصادم.
- **Aleatoire Hedge:** `hedged_chacha20_rng` يجمع بين العناصر المحددة مع إنتروبيا النظام وإلغاء الحالة الوسيطة للتدمير.

جميع الأسرار حية في الحاويات `Zeroizing` et CI تمارس الروابط PQClean على جميع اللوحات المدعومة.

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
```## تعليق على المستعمل

1. **إضافة التبعية** إلى الصناديق داخل مسار مساحة العمل:

   ```toml
   soranet_pq = { path = "../../crates/soranet_pq" }
   ```

2. **تحديد المجموعة الصحيحة** نقاط الاتصال. لبدء العمل الأولي للمصافحة الهجينة، استخدم `MlKemSuite::MlKem768` و`MlDsaSuite::MlDsa65`.

3. **اشتقاق العناصر ذات التصنيفات.** استخدم `HkdfDomain::soranet("KEM/1")` (وما يعادله) حتى يتم تحديد سلسلة النسخ بين الأحداث الجديدة.

4. **استخدم RNG المتحوط** لتحديث أسرار الرد:

   ```rust
   use soranet_pq::{hedged_chacha20_rng, HedgedRngSeed};

   let mut rng = hedged_chacha20_rng(HedgedRngSeed::new(b"snnet16", [0u8; 32]));
   ```

يستخدم مركز المصافحة في SoraNet ومساعدي CID (`iroha_crypto::soranet`) هذه الأدوات مباشرة، مما يعني أن الصناديق المتدفقة ترث تطبيقات الميمات بدون روابط PQClean eux-memes.

## قائمة التحقق من الصحة

-`cargo test -p soranet_pq --offline`
-`cargo fmt --package soranet_pq`
- قم بمراجعة أمثلة استخدام README (`crates/soranet_pq/README.md`)
- Mettez a jour le document de conception du handshake SoraNet عند وصول الهجينة