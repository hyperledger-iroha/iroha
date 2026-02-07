---
id: pq-primitives
lang: az
direction: ltr
source: docs/portal/docs/soranet/pq-primitives.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraNet Post-Quantum Primitives
sidebar_label: PQ Primitives
description: Overview of the `soranet_pq` crate and how the SoraNet handshake consumes ML-KEM/ML-DSA helpers.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::Qeyd Kanonik Mənbə
:::

`soranet_pq` qutusu hər bir SoraNet-in istifadə etdiyi post-kvant tikinti bloklarını ehtiva edir.
rele, müştəri və alət komponentinə əsaslanır. PQClean tərəfindən dəstəklənən Kyber-i əhatə edir
(ML-KEM) və Dilitium (ML-DSA) paketləri və protokola uyğun HKDF-də təbəqələr və
hedcinq edilmiş RNG köməkçiləri beləliklə bütün səthlər eyni tətbiqləri paylaşır.

## `soranet_pq`-də nə göndərilir

- **ML-KEM-512/768/1024:** deterministik açarın yaradılması, inkapsulyasiya və
  sabit zaman xətası yayılması ilə dekapsulyasiya köməkçiləri.
- **ML-DSA-44/65/87:** üçün ayrılmış imzalama/doğrulama simli
  domenlə ayrılmış transkriptlər.
- **Etiketli HKDF:** `derive_labeled_hkdf` hər törəməni
  əl sıxma mərhələsi (`DH/es`, `KEM/1`, …) beləliklə, hibrid transkriptlər toqquşmadan qalır.
- **Hedcinq edilmiş təsadüfilik:** `hedged_chacha20_rng` deterministik toxumları qarışdırır
  canlı OS entropiyası ilə və düşmə zamanı aralıq vəziyyəti sıfırlayır.

Bütün sirlər `Zeroizing` konteynerlərinin içərisindədir və CI PQClean tətbiq edir.
hər dəstəklənən platformada bağlamalar.

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

## Necə istehlak etmək olar

1. İş sahəsinin kökündən kənarda yerləşən qutulara **asılılığı** əlavə edin:

   ```toml
   soranet_pq = { path = "../../crates/soranet_pq" }
   ```

2. Zəng saytlarında **Düzgün paketi seçin**. İlkin hibrid əl sıxma üçün
   işləyin, `MlKemSuite::MlKem768` və `MlDsaSuite::MlDsa65` istifadə edin.

3. **Etiketli açarlar əldə edin.** `HkdfDomain::soranet("KEM/1")` (və bacı-qardaşlar) istifadə edin.
   beləliklə, transkript zəncirləmə qovşaqlar arasında deterministik olaraq qalır.

4. Yekun sirrləri seçərkən **hedcinq edilmiş RNG-dən** istifadə edin:

   ```rust
   use soranet_pq::{hedged_chacha20_rng, HedgedRngSeed};

   let mut rng = hedged_chacha20_rng(HedgedRngSeed::new(b"snnet16", [0u8; 32]));
   ```

Əsas SoraNet əl sıxma və CID körləşdirmə köməkçiləri (`iroha_crypto::soranet`)
bu kommunalları birbaşa çəkin, yəni aşağı axın qutuları eyni miras alır
PQClean bağlamalarının özlərini əlaqələndirmədən tətbiqlər.

## Doğrulama yoxlama siyahısı

- `cargo test -p soranet_pq --offline`
- `cargo fmt --package soranet_pq`
- README istifadə nümunələrini yoxlayın (`crates/soranet_pq/README.md`)
- Hibridlər yerə endikdən sonra SoraNet əl sıxma dizayn sənədini yeniləyin