---
lang: he
direction: rtl
source: docs/portal/docs/soranet/pq-primitives.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: pq-primitives
כותרת: Primitives post-quantiques SoraNet
sidebar_label: Primitives PQ
תיאור: Vue d'ensemble du crate `soranet_pq` et de la maniere dont le לחיצת יד SoraNet consomme les helpers ML-KEM/ML-DSA.
---

:::הערה מקור קנוניק
:::

ארגז `soranet_pq` מכיל חומרי גלם פוסט-quantiques ו-resquelles מסייעים לממסרים, לקוחות וחומרי עזר של SoraNet. חבילת הסוויטות Kyber (ML-KEM) ו-Dilithium (ML-DSA) מטפלת ב-PQClean ועזרה של עוזרי HKDF ו-RNG hedged מתאימים את הפרוטוקולים ומציגים את המשטחים השותפים להטמעות זהות.

## Ce qui est livre dans `soranet_pq`

- **ML-KEM-512/768/1024:** generation deterministe de cles, encapsulation et decapsulation avec propagation d'erreurs and temps constant.
- **ML-DSA-44/65/87:** חתימה/אימות מנותק עם תעתיקים לתחום ההפרדה.
- **גינוני HKDF:** `derive_labeled_hkdf` אפליקציית מרחב שמות גזירת צ'אקה באמצעות לחיצת יד (`DH/es`, `KEM/1`, ...) afin que les transcriptions hybrides restent sans collision.
- **Aleatoire hedged:** `hedged_chacha20_rng` combine des seeds deterministes avec l'entropie du systeme et zeroise l'etat intermediaire a la destruction.

Tous les secrets vivent dans des conteneurs `Zeroizing` et CI תרגיל les bindings PQClean sur toutes les plateformes supportees.

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

## הערה למשתמש

1. **Ajoutez la dependance** aux crates en dehors de la racine du space work:

   ```toml
   soranet_pq = { path = "../../crates/soranet_pq" }
   ```

2. **Selectionnez la suite correcte** aux points d'appel. Pour le travail initial du handshake hybride, utilisez `MlKemSuite::MlKem768` et `MlDsaSuite::MlDsa65`.

3. **Derivez les cles avec תוויות.** Utilisez `HkdfDomain::soranet("KEM/1")` (et equivalents) pour que l'enchainement des transcriptions reste deterministe entre les noeuds.

4. **Utilisez le RNG hedged** pour echantillonner les secrets de repli:

   ```rust
   use soranet_pq::{hedged_chacha20_rng, HedgedRngSeed};

   let mut rng = hedged_chacha20_rng(HedgedRngSeed::new(b"snnet16", [0u8; 32]));
   ```

לחיצת היד המרכזית של SoraNet et les helpers de blindage de CID (`iroha_crypto::soranet`) שימושי הכוונה ces utilitaires, ce qui signifie que les boxes downstream heritent des memes implementations sans lier les bindings PQClean eux-memes.

## רשימת רשימת אימות

- `cargo test -p soranet_pq --offline`
- `cargo fmt --package soranet_pq`
- Auditez les exemples d'usage du README (`crates/soranet_pq/README.md`)
- Mettez a jour le document de conception du לחיצת יד SoraNet lorsque les hybrides arriveront