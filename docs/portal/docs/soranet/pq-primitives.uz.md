---
lang: uz
direction: ltr
source: docs/portal/docs/soranet/pq-primitives.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c90383149066d2e43cef962e6fe946f939277c3f7d22f3ee4688db8cc96b23b2
source_last_modified: "2026-01-05T09:28:11.912107+00:00"
translation_last_reviewed: 2026-02-07
id: pq-primitives
title: SoraNet Post-Quantum Primitives
sidebar_label: PQ Primitives
description: Overview of the `soranet_pq` crate and how the SoraNet handshake consumes ML-KEM/ML-DSA helpers.
translator: machine-google-reviewed
---

::: Eslatma Kanonik manba
:::

`soranet_pq` qutisi har bir SoraNet-da mavjud bo'lgan post-kvant qurilish bloklarini o'z ichiga oladi.
rele, mijoz va asboblar komponentiga tayanadi. U PQClean tomonidan qo'llab-quvvatlanadigan Kyber-ni o'rab oladi
(ML-KEM) va Dilithium (ML-DSA) to'plamlari va protokollar uchun qulay HKDF va qatlamlari
himoyalangan RNG yordamchilari, shuning uchun barcha sirtlar bir xil ilovalarni almashadi.

## `soranet_pq` da nima yuboriladi

- **ML-KEM-512/768/1024:** deterministik kalit yaratish, inkapsulyatsiya va
  doimiy vaqt xatosi tarqalishi bilan dekapsulyatsiya yordamchilari.
- **ML-DSA-44/65/87:** alohida imzolash/tasdiqlash simli
  domendan ajratilgan transkriptlar.
- **Yerli HKDF:** `derive_labeled_hkdf` har bir hosila uchun nom maydonini
  qo'l siqish bosqichi (`DH/es`, `KEM/1`, …), shuning uchun gibrid transkriptlar to'qnashuvsiz qoladi.
- **Xedjlangan tasodifiylik:** `hedged_chacha20_rng` deterministik urug'larni birlashtiradi
  jonli OS entropiyasi bilan va tushib ketganda oraliq holatni nolga aylantiradi.

Barcha sirlar `Zeroizing` konteynerlarida joylashgan va CI PQClean-ni mashq qiladi.
har bir qo'llab-quvvatlanadigan platformada ulanishlar.

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

## Uni qanday iste'mol qilish kerak

1. Ish maydoni ildizidan tashqarida joylashgan qutilarga **qaramlikni** qo‘shing:

   ```toml
   soranet_pq = { path = "../../crates/soranet_pq" }
   ```

2. Qo'ng'iroq qilish joylarida **To'g'ri to'plamni tanlang**. Dastlabki gibrid qo'l siqish uchun
   ish, `MlKemSuite::MlKem768` va `MlDsaSuite::MlDsa65` dan foydalaning.

3. **Yorliqli kalitlarni oling.** `HkdfDomain::soranet("KEM/1")` (va opa-singillar) dan foydalaning
   shuning uchun transkript zanjiri tugunlar bo'ylab deterministik bo'lib qoladi.

4. Qaytarilish sirlarini tanlashda **xedjlangan RNG** dan foydalaning:

   ```rust
   use soranet_pq::{hedged_chacha20_rng, HedgedRngSeed};

   let mut rng = hedged_chacha20_rng(HedgedRngSeed::new(b"snnet16", [0u8; 32]));
   ```

Asosiy SoraNet qo'l siqish va CID ko'r-ko'rona yordamchilari (`iroha_crypto::soranet`)
bu yordamchi dasturlarni to'g'ridan-to'g'ri torting, ya'ni quyi oqim qutilari bir xil meros bo'lib qoladi
PQClean ulanishlarini o'zlari bog'lamasdan amalga oshirish.

## Tekshirish ro'yxati

- `cargo test -p soranet_pq --offline`
- `cargo fmt --package soranet_pq`
- README foydalanish namunalarini tekshirish (`crates/soranet_pq/README.md`)
- Gibridlar qo'nishi bilan SoraNet qo'l siqish dizayn hujjatini yangilang