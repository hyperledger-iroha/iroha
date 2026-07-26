---
lang: fr
direction: ltr
source: docs/portal/docs/soranet/pq-primitives.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : pq-primitives
titre : بدائيات ما بعد الكم في SoraNet
sidebar_label : PQ
description : Utilisez la caisse `soranet_pq` et le module SoraNet ML-KEM/ML-DSA.
---

:::note المصدر القياسي
Il s'agit de la référence `docs/source/soranet/pq_primitives.md`. حافظ على النسختين متطابقتين حتى يتم تقاعد مجموعة الوثائق القديمة.
:::

Il s'agit de la caisse `soranet_pq` pour les outils de relais, les clients et les outils de SoraNet. Il existe également des solutions pour Kyber (ML-KEM) et Dilithium (ML-DSA) avec PQClean et des helpers pour HKDF et RNG hedged pour la gestion des risques. جميع الأسطح نفس التنفيذات.

## ما الذي يتضمنه `soranet_pq`

- **ML-KEM-512/768/1024 :** Il s'agit d'une encapsulation et d'une décapsulation plus efficaces que les autres.
- **ML-DSA-44/65/87 :** توقيع/تحقق منفصلان مربوطان بنصوص مفصولة النطاق.
- **HKDF Format :** `derive_labeled_hkdf` est un espace de noms qui correspond à l'espace de noms (`DH/es`, `KEM/1`, ...) تبقى النصوص الهجينة بلا تصادم.
- **عشوائية hedged:** `hedged_chacha20_rng` يمزج بذورا حتمية مع إنتروبيا النظام ويصفر الحالة الوسيطة عند التحرير.

Utilisez le logiciel `Zeroizing` et CI روابط PQClean pour le mettre en œuvre.

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

1. **اضف الاعتماد** crates الموجودة خارج جذر espace de travail :

   ```toml
   soranet_pq = { path = "../../crates/soranet_pq" }
   ```

2. **اختر المجموعة الصحيحة** في نقاط الاستدعاء. للعمل الأولي على المصافحة الهجينة، استخدم `MlKemSuite::MlKem768` et `MlDsaSuite::MlDsa65`.

3. **اشتق المفاتيح مع وسوم.** استخدم `HkdfDomain::soranet("KEM/1")` (ونظراءه) حتى يبقى تسلسل النصوص حتميا عبر العقد.

4. **استخدم RNG hedged** est l'un des éléments suivants:

   ```rust
   use soranet_pq::{hedged_chacha20_rng, HedgedRngSeed};

   let mut rng = hedged_chacha20_rng(HedgedRngSeed::new(b"snnet16", [0u8; 32]));
   ```

Utiliser SoraNet pour la connexion avec le CID (`iroha_crypto::soranet`) pour les crates Vous avez besoin de PQClean pour utiliser PQClean.

## قائمة تحقق التحقق

-`cargo test -p soranet_pq --offline`
-`cargo fmt --package soranet_pq`
- راجع أمثلة الاستخدام في README (`crates/soranet_pq/README.md`)
- حدث وثيقة تصميم مصافحة SoraNet عند وصول الهجائن