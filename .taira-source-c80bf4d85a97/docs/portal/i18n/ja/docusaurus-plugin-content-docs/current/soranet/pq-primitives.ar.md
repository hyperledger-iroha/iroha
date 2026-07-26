---
lang: ja
direction: ltr
source: docs/portal/docs/soranet/pq-primitives.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: pq-プリミティブ
タイトル: بدائيات ما بعد الكم في SoraNet
サイドバーラベル: بدائيات PQ
説明: クレート `soranet_pq` وكيف يستهلك مصافحة SoraNet مساعدات ML-KEM/ML-DSA。
---

:::メモ
テストは `docs/source/soranet/pq_primitives.md` です。 حافظ على النسختين متطابقتين حتى يتم تقاعد مجموعة الوثائق القديمة。
:::

クレート `soranet_pq` は、リレー、クライアント、ツール、SoraNet をサポートしています。 Kyber (ML-KEM) ダイリチウム (ML-DSA) PQClean ヘルパー HKDF ヘッジ RNG ヘッジ ダイリチウム (ML-DSA)最高です。

## ما الذي يتضمنه `soranet_pq`

- **ML-KEM-512/768/1024:** カプセル化とカプセル化解除のテスト。
- **ML-DSA-44/65/87:** توقيع/تحقق منفصلان مربوطان بنصوص مفصولة النطاق.
- **HKDF موسوم:** `derive_labeled_hkdf` يضيف 名前空間 لكل اشتقاق مع مرحلة المصافحة (`DH/es`, `KEM/1`, ...) حتى最高です。
- **ヘッジ済み:** `hedged_chacha20_rng` يمزج بذورا حتمية مع إنتروبيا النظام ويصفر الحالة الوسيطة عندああ。

`Zeroizing` CI アカウント PQClean をサポートしています。

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

1. **作業スペース** 作業スペースのクレート:

   ```toml
   soranet_pq = { path = "../../crates/soranet_pq" }
   ```

2. ** اختر المجموعة الصحيحة** في نقاط الاستدعاء. `MlKemSuite::MlKem768` و`MlDsaSuite::MlDsa65` を確認してください。

3. **اشتق المفاتيح مع وسوم.** استخدم `HkdfDomain::soranet("KEM/1")` (ونظراءه) حتى يبقى تسلسل النصوص حتميا عبرああ。

4. **RNG ヘッジ** リスク:

   ```rust
   use soranet_pq::{hedged_chacha20_rng, HedgedRngSeed};

   let mut rng = hedged_chacha20_rng(HedgedRngSeed::new(b"snnet16", [0u8; 32]));
   ```

SoraNet の情報 CID (`iroha_crypto::soranet`) هذه الأدوات مباشرة، ما يعني أن クレートPQClean を使用してください。

## قائمة تحقق التحقق

- `cargo test -p soranet_pq --offline`
- `cargo fmt --package soranet_pq`
- README (`crates/soranet_pq/README.md`)
- حدث وثيقة تصميم مصافحة SoraNet عند وصول الهجائن