---
lang: ar
direction: rtl
source: docs/portal/docs/soranet/pq-primitives.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: pq-الأوليات
العنوان: SoraNet أوليات ما بعد الكم
Sidebar_label: بدايات PQ
الوصف: صندوق `soranet_pq` نظرة عامة ومساعد SoraNet للمصافحة ML-KEM/ML-DSA يستخدم كرتا.
---

:::ملاحظة المصدر الكنسي
هذه هي الصفحة `docs/source/soranet/pq_primitives.md`. إذا لم يتم تعيين وثائق برانا للتقاعد، فلا داعي للمزامنة.
:::

`soranet_pq` قفص إن لبنات بناء ما بعد الكم تتضمن ہے جن پر ہر SoraNet Relay، العميل ومكون الأدوات انحصار کرتا ہے۔ تعمل مجموعات Kyber (ML-KEM) وDilithium (ML-DSA) المدعومة من PQClean على تغليف الكرتا وHKDF الصديقة للبروتوكول ومساعدي RNG المتحوّطين على جميع الأسطح التي تشاركها هذه التطبيقات.

## `soranet_pq` ما يشمله

- **ML-KEM-512/768/1024:** توليد المفاتيح الحتمية، ومساعدي التغليف وإلغاء التغليف، ويعملان على نشر الأخطاء في الوقت الثابت.
- **ML-DSA-44/65/87:** التوقيع/التحقق المنفصل للنصوص المنفصلة عن المجال السلكية.
- **Labeled HKDF:** `derive_labeled_hkdf` اشتقاق مرحلة المصافحة (`DH/es`, `KEM/1`, ...) مساحة الاسم ديتا ونصوص مختلطة خالية من الاصطدام.
- **العشوائية التحوطية:** `hedged_chacha20_rng` البذور الحتمية التي تعمل على مزج إنتروبيا نظام التشغيل المباشر وإسقاط الحالة المتوسطة وتصفير الكرتا.تمام الأسرار `Zeroizing` حاويات اندر رہتے ہیں و CI تمام المنصات المدعومة لربطات PQClean التي تمارسها.

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

## استخدام کیسے کریں

1. **تشمل التبعية** الصناديق التي تحتوي على جذر مساحة العمل:

   ```toml
   soranet_pq = { path = "../../crates/soranet_pq" }
   ```

2. **درست جناح منتخب کریں** مواقع الاتصال پر۔ استخدام المصافحة الهجينة دائمًا `MlKemSuite::MlKem768` و`MlDsaSuite::MlDsa65`.

3. **تشتق تسميات المفاتيح الثابتة.** `HkdfDomain::soranet("KEM/1")` (وهذا هو الاسم) يستخدم لعقد تسلسل النسخ كقواعد حتمية.

4. **استخدام RNG المتحوط** لعينة الأسرار الاحتياطية:

   ```rust
   use soranet_pq::{hedged_chacha20_rng, HedgedRngSeed};

   let mut rng = hedged_chacha20_rng(HedgedRngSeed::new(b"snnet16", [0u8; 32]));
   ```

SoraNet هو المصافحة الأساسية ومساعدي CID المسببة للعمى (`iroha_crypto::soranet`) وهو عبارة عن أدوات مساعدة تستخدم في استخدام البطاقة، وهو ما يتطلب طلب صناديق المصب من خلال ارتباطات PQClean التي ترث التطبيقات.

## قائمة التحقق من الصحة

-`cargo test -p soranet_pq --offline`
-`cargo fmt --package soranet_pq`
- نماذج استخدام README لتدقيق الحسابات (`crates/soranet_pq/README.md`)
- الهجينة التي تم إنشاؤها بعد تصميم مصافحة SoraNet doc