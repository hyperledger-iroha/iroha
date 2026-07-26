---
lang: pt
direction: ltr
source: docs/portal/docs/soranet/pq-primitives.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: pq-primitivos
título: بدائيات ما بعد الكم في SoraNet
sidebar_label: Nome PQ
description: Coloque a caixa `soranet_pq` e use o SoraNet para ML-KEM/ML-DSA.
---

:::note المصدر القياسي
Verifique o valor `docs/source/soranet/pq_primitives.md`. Não se preocupe, você pode fazer isso sem problemas.
:::

A caixa `soranet_pq` é usada para retransmitir, cliente e ferramentas no SoraNet. Você pode usar Kyber (ML-KEM) e Dilithium (ML-DSA) com PQClean e ajudantes para HKDF e RNG protegidos تتشارك جميع الأسطح نفس التنفيذات.

## ما الذي يتضمنه `soranet_pq`

- **ML-KEM-512/768/1024:** توليد مفاتيح حتمي, ومساعدات encapsulamento e desencapsulação مع نشر أخطاء بزمن ثابت.
- **ML-DSA-44/65/87:** توقيع/تحقق منفصلان مربوطان بنصوص مفصولة النطاق.
- **HKDF موسوم:** `derive_labeled_hkdf` يضيف namespace لكل اشتقاق مع مرحلة المصافحة (`DH/es`, `KEM/1`, ...) حتى Verifique o valor do produto.
- **عشوائية coberto:** `hedged_chacha20_rng` يمزج بذورا حتمية مع إنتروبيا النظام ويصفر الحالة الوسيطة عند التحرير.

Verifique se o `Zeroizing` está instalado e o CI PQClean está conectado a ele.

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

1. **اضف الاعتماد** إلى crates الموجودة خارج جذر espaço de trabalho:

   ```toml
   soranet_pq = { path = "../../crates/soranet_pq" }
   ```

2. **اختر المجموعة الصحيحة** no final do processo. Para obter mais informações, use `MlKemSuite::MlKem768` e `MlDsaSuite::MlDsa65`.

3. **اشتق المفاتيح مع وسوم.** استخدم `HkdfDomain::soranet("KEM/1")` (ونظراءه) حتى يبقى تسلسل النصوص حتميا عبر sim.

4. **RNG protegido por RNG** عند أخذ عينات لأسرار بديلة:

   ```rust
   use soranet_pq::{hedged_chacha20_rng, HedgedRngSeed};

   let mut rng = hedged_chacha20_rng(HedgedRngSeed::new(b"snnet16", [0u8; 32]));
   ```

Use o SoraNet para criar e armazenar CID (`iroha_crypto::soranet`) em caixas Verifique se o PQClean está funcionando corretamente.

## قائمة تحقق التحقق

-`cargo test -p soranet_pq --offline`
-`cargo fmt --package soranet_pq`
- راجع أمثلة الاستخدام no README (`crates/soranet_pq/README.md`)
- حدث وثيقة تصميم مصافحة SoraNet عند وصول الهجائن