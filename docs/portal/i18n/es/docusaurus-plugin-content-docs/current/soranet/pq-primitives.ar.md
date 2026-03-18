---
lang: es
direction: ltr
source: docs/portal/docs/soranet/pq-primitives.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identificación: pq-primitivas
título: بدائيات ما بعد الكم في SoraNet
sidebar_label: بدائيات PQ
descripción: Utilice la caja `soranet_pq` y utilice SoraNet ML-KEM/ML-DSA.
---

:::nota المصدر القياسي
Utilice el botón `docs/source/soranet/pq_primitives.md`. حافظ على النسختين متطابقتين حتى يتم تقاعد مجموعة الوثائق القديمة.
:::

Esta caja `soranet_pq` está diseñada para funcionar con relés, clientes y herramientas de SoraNet. Y los asistentes Kyber (ML-KEM) y Dilithium (ML-DSA) de PQClean y helpers de HKDF y RNG con cobertura de seguridad. جميع الأسطح نفس التنفيذات.

## ما الذي يتضمنه `soranet_pq`

- **ML-KEM-512/768/1024:** توليد مفاتيح حتمي، ومساعدات encapsulación y decapsulación مع نشر أخطاء بزمن ثابت.
- **ML-DSA-44/65/87:** توقيع/تحقق منفصلان مربوطان بنصوص مفصولة النطاق.
- **HKDF موسوم:** `derive_labeled_hkdf` es un espacio de nombres para el espacio de nombres (`DH/es`, `KEM/1`, ...) النصوص الهجينة بلا تصادم.
- **عشوائية cubierto:** `hedged_chacha20_rng` يمزج بذورا حتمية مع إنتروبيا النظام ويصفر الحالة الوسيطة عند التحرير.

Asegúrese de que los dispositivos `Zeroizing` y CI PQClean estén limpios.

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

1. **اضف الاعتماد** إلى cajas الموجودة خارج جذر espacio de trabajo:

   ```toml
   soranet_pq = { path = "../../crates/soranet_pq" }
   ```

2. **اختر المجموعة الصحيحة** في نقاط الاستدعاء. Para obtener más información, seleccione `MlKemSuite::MlKem768` e `MlDsaSuite::MlDsa65`.3. **اشتق المفاتيح مع وسوم.** استخدم `HkdfDomain::soranet("KEM/1")` (ونظراءه) حتى يبقى تسلسل النصوص حتميا عبر العقد.

4. **استخدم RNG cubierto** عند أخذ عينات لأسرار بديلة:

   ```rust
   use soranet_pq::{hedged_chacha20_rng, HedgedRngSeed};

   let mut rng = hedged_chacha20_rng(HedgedRngSeed::new(b"snnet16", [0u8; 32]));
   ```

Instalación de SoraNet y instalación de CID (`iroha_crypto::soranet`) en cajas de almacenamiento Utilice el limpiador PQClean para instalarlo.

## قائمة تحقق التحقق

- `cargo test -p soranet_pq --offline`
- `cargo fmt --package soranet_pq`
- راجع أمثلة الاستخدام في README (`crates/soranet_pq/README.md`)
- حدث وثيقة تصميم مصافحة SoraNet عند وصول الهجائن