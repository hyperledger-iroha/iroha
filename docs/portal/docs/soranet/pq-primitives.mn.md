---
lang: mn
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

::: Каноник эх сурвалжийг анхаарна уу
:::

`soranet_pq` хайрцаг нь SoraNet болгонд байдаг квантын дараах барилгын блокуудыг агуулдаг.
реле, үйлчлүүлэгч, багаж хэрэгслийн бүрэлдэхүүн хэсэг дээр тулгуурладаг. Энэ нь PQClean-д тулгуурласан Kyber-ийг ороосон
(ML-KEM) болон Dilithium (ML-DSA) багцууд болон протоколд ээлтэй HKDF болон давхаргууд
хамгаалагдсан RNG туслахууд тул бүх гадаргуу нь ижил төстэй хэрэгжилтийг хуваалцдаг.

## `soranet_pq`-д юу тээвэрлэгддэг

- **ML-KEM-512/768/1024:** тодорхойлогч түлхүүр үүсгэх, капсулжуулалт болон
  Тогтмол хугацааны алдааны тархалттай decapsulation туслахууд.
- **ML-DSA-44/65/87:** салангид гарын үсэг зурах/баталгаажуулах утастай
  домайнаар тусгаарлагдсан хуулбарууд.
- **HKDF шошготой:** `derive_labeled_hkdf` нь гарал үүсэлтэй нэрийн талбар бүрийг
  гар барих үе шат (`DH/es`, `KEM/1`, …) тул эрлийз хуулбарууд мөргөлдөхгүй.
- **Хеджлэгдсэн санамсаргүй байдал:** `hedged_chacha20_rng` нь детерминист үрийг хольсон
  амьд үйлдлийн системийн энтропитэй ба завсрын төлөвийг уналтанд тэглэнэ.

Бүх нууцууд `Zeroizing` савны дотор байдаг бөгөөд CI нь PQClean-г дасгалжуулдаг.
дэмжигдсэн платформ бүр дээр холбох.

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

## Хэрхэн хэрэглэх вэ

1. Ажлын талбарын үндсэн гадна байрлах хайрцагт **хамааралтай байдлыг** нэмнэ үү:

   ```toml
   soranet_pq = { path = "../../crates/soranet_pq" }
   ```

2. Дуудлагын сайтуудаас **Зөв багцыг сонгоно уу**. Анхны эрлийз гар барихад зориулав
   ажиллах, `MlKemSuite::MlKem768` болон `MlDsaSuite::MlDsa65` ашиглах.

3. **Түлхүүрүүдийг шошготой гарга.** `HkdfDomain::soranet("KEM/1")` (болон ах эгч нар)-г ашиглана уу.
   тиймээс транскриптийн гинжин хэлхээ нь зангилаа даяар тодорхойлогддог.

4. Нөөц нууцыг түүвэрлэхдээ **Хеджжүүлсэн RNG**-г ашиглана уу:

   ```rust
   use soranet_pq::{hedged_chacha20_rng, HedgedRngSeed};

   let mut rng = hedged_chacha20_rng(HedgedRngSeed::new(b"snnet16", [0u8; 32]));
   ```

SoraNet-ийн үндсэн гар барих болон CID харалган туслахууд (`iroha_crypto::soranet`)
эдгээр хэрэгслүүдийг шууд татах нь доод урсгалын хайрцагнууд ижилхэн өвлөгдөнө гэсэн үг юм
PQClean холболтыг өөрөө холбохгүйгээр хэрэгжүүлэлт.

## Баталгаажуулах хяналтын хуудас

- `cargo test -p soranet_pq --offline`
- `cargo fmt --package soranet_pq`
- README хэрэглээний дээжийг шалгах (`crates/soranet_pq/README.md`)
- Гибридүүд газардсаны дараа SoraNet гар барих дизайны баримт бичгийг шинэчилнэ үү