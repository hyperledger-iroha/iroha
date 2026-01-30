---
lang: ur
direction: rtl
source: docs/portal/docs/soranet/pq-primitives.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: pq-primitives
title: بدائيات ما بعد الكم في SoraNet
sidebar_label: بدائيات PQ
description: نظرة عامة على crate `soranet_pq` وكيف يستهلك مصافحة SoraNet مساعدات ML-KEM/ML-DSA.
---

:::note المصدر القياسي
تعكس هذه الصفحة `docs/source/soranet/pq_primitives.md`. حافظ على النسختين متطابقتين حتى يتم تقاعد مجموعة الوثائق القديمة.
:::

يحتوي crate `soranet_pq` على لبنات ما بعد الكم التي تعتمد عليها كل relay وclient وtooling في SoraNet. وهو يغلف مجموعات Kyber (ML-KEM) وDilithium (ML-DSA) المدعومة من PQClean ويضيف helpers لـ HKDF وRNG hedged ملائمة للبروتوكول حتى تتشارك جميع الأسطح نفس التنفيذات.

## ما الذي يتضمنه `soranet_pq`

- **ML-KEM-512/768/1024:** توليد مفاتيح حتمي، ومساعدات encapsulation وdecapsulation مع نشر أخطاء بزمن ثابت.
- **ML-DSA-44/65/87:** توقيع/تحقق منفصلان مربوطان بنصوص مفصولة النطاق.
- **HKDF موسوم:** `derive_labeled_hkdf` يضيف namespace لكل اشتقاق مع مرحلة المصافحة (`DH/es`, `KEM/1`, ...) حتى تبقى النصوص الهجينة بلا تصادم.
- **عشوائية hedged:** `hedged_chacha20_rng` يمزج بذورا حتمية مع إنتروبيا النظام ويصفر الحالة الوسيطة عند التحرير.

تسكن جميع الأسرار داخل حاويات `Zeroizing` وتختبر CI روابط PQClean على جميع المنصات المدعومة.

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

1. **اضف الاعتماد** إلى crates الموجودة خارج جذر workspace:

   ```toml
   soranet_pq = { path = "../../crates/soranet_pq" }
   ```

2. **اختر المجموعة الصحيحة** في نقاط الاستدعاء. للعمل الأولي على المصافحة الهجينة، استخدم `MlKemSuite::MlKem768` و`MlDsaSuite::MlDsa65`.

3. **اشتق المفاتيح مع وسوم.** استخدم `HkdfDomain::soranet("KEM/1")` (ونظراءه) حتى يبقى تسلسل النصوص حتميا عبر العقد.

4. **استخدم RNG hedged** عند أخذ عينات لأسرار بديلة:

   ```rust
   use soranet_pq::{hedged_chacha20_rng, HedgedRngSeed};

   let mut rng = hedged_chacha20_rng(HedgedRngSeed::new(b"snnet16", [0u8; 32]));
   ```

تسحب مصافحة SoraNet الأساسية ومساعدات تعمية CID (`iroha_crypto::soranet`) هذه الأدوات مباشرة، ما يعني أن crates التابعة ترث نفس التنفيذات دون ربط PQClean بنفسها.

## قائمة تحقق التحقق

- `cargo test -p soranet_pq --offline`
- `cargo fmt --package soranet_pq`
- راجع أمثلة الاستخدام في README (`crates/soranet_pq/README.md`)
- حدث وثيقة تصميم مصافحة SoraNet عند وصول الهجائن
