---
lang: ar
direction: rtl
source: docs/portal/docs/soranet/pq-primitives.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: pq-الأوليات
العنوان: Primitivos pos-quanticos do SoraNet
Sidebar_label: Primitivos PQ
الوصف: Visao gral do crate `soranet_pq` و de como أو المصافحة بواسطة SoraNet يستهلك المساعدين ML-KEM/ML-DSA.
---

:::ملاحظة فونتي كانونيكا
هذه الصفحة espelha `docs/source/soranet/pq_primitives.md`. Mantenha ambas as copias sincronzadas.
:::

يحتوي الصندوق `soranet_pq` على كتل ذات عدد كبير من كل ما يتعلق بالتتابع والعميل ومكونات الأدوات التي يثق بها SoraNet. تشتمل هذه على مجموعات Kyber (ML-KEM) وDilithium (ML-DSA) المدعومة من قبل PQClean وإضافة مساعدين إضافيين لـ HKDF وRNG المحميين بالبروتوكول بحيث تشارك جميع الأسطح تطبيقات متطابقة.

## ما هو `soranet_pq`

- **ML-KEM-512/768/1024:** أداة حتمية للرؤوس، ومساعدين للتغليف وفك التغليف لنشر الأخطاء في وقت ثابت.
- **ML-DSA-44/65/87:** تم التثبيت/التحقق من فصل النسخ للنسخ مع فصل السيادة.
- **HKDF مع الدوران:** `derive_labeled_hkdf` تضيف مساحة الاسم إلى كل مشتق من مكان المصافحة (`DH/es`، `KEM/1`، ...) لنسخ الهجينة دون تشابك.
- **التحوط المتحوط:** `hedged_chacha20_rng` بذور غامضة حتمية com entropia do SO e zera o estado intermediario ao discartar.

كل ما هو منفصل موجود داخل الحاويات `Zeroizing` و CI يمارس الارتباطات PQClean في جميع المنصات المدعومة.```rust
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

## كومو المستهلك

1. **إضافة إلى التبعية** في الصناديق المخصصة لمساحة العمل:

   ```toml
   soranet_pq = { path = "../../crates/soranet_pq" }
   ```

2. **اختر مجموعة التصحيح** من خلال ثقوب الاتصال. لبدء العمل الأولي للمصافحة، استخدم `MlKemSuite::MlKem768` و`MlDsaSuite::MlDsa65`.

3. **اشتقاق الحروف من الدوارات.** استخدم `HkdfDomain::soranet("KEM/1")` (والمشابهة) لتكملة النسخ المحددة بشكل محدد بين العقد.

4. **استخدم RNG المتحوط** من أجل الحفاظ على العزلة الاحتياطية:

   ```rust
   use soranet_pq::{hedged_chacha20_rng, HedgedRngSeed};

   let mut rng = hedged_chacha20_rng(HedgedRngSeed::new(b"snnet16", [0u8; 32]));
   ```

يتم استخدام هذه المصافحة المركزية في SoraNet ومساعدي CID (`iroha_crypto::soranet`) مباشرة، أو ما يعني أن الصناديق الموجودة في اتجاه مجرى النهر يتم تنفيذها كأدوات دون روابط ربط PQClean لحسابها الخاص.

## قائمة التحقق من صحة

-`cargo test -p soranet_pq --offline`
-`cargo fmt --package soranet_pq`
- مراجعة نماذج الاستخدام بدون README (`crates/soranet_pq/README.md`)
- تحديث مستند تصميم المصافحة بواسطة SoraNet عندما يتم تغيير الهجينة