---
id: pq-primitives
lang: am
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

::: ማስታወሻ ቀኖናዊ ምንጭ
::

የ `soranet_pq` ሳጥን እያንዳንዱ SoraNet ያለውን የድህረ-ኳንተም ግንባታ ብሎኮች ይዟል።
ቅብብሎሽ፣ ደንበኛ እና የመሳሪያ አካል የተመካ ነው። በPQClean የተደገፈ Kyber ይጠቀለላል
(ML-KEM) እና Dilithium (ML-DSA) ስብስቦች እና ንብርብሮች በፕሮቶኮል ተስማሚ ኤች.ዲ.ዲ.ኤፍ እና
የታጠሩ RNG ረዳቶች ስለዚህ ሁሉም ገጽታዎች ተመሳሳይ አተገባበርን ይጋራሉ።

## በ `soranet_pq` ምን ይላካል

- **ML-KEM-512/768/1024፡** የሚወስን ቁልፍ ማመንጨት፣ ማሸግ እና
  የቋሚ ጊዜ ስህተት መስፋፋት ያላቸው ረዳት ረዳቶች.
- **ML-DSA-44/65/87:** የተነጠለ ፊርማ/ማረጋገጫ ለገመድ
  በጎራ-የተለያዩ ግልባጮች።
- ** የተሰየመ ኤች.ዲ.ዲ.ኤፍ:** `derive_labeled_hkdf` የስም ቦታዎች እያንዳንዱ አመጣጥ ከ
  የመጨባበጥ ደረጃ (`DH/es`፣ `KEM/1`፣ …) ስለዚህ የተዳቀሉ ግልባጮች ከግጭት ነፃ ሆነው ይቆያሉ።
- ** የታጠረ የዘፈቀደነት:** `hedged_chacha20_rng` የሚወስኑ ዘሮችን ያዋህዳል
  ከቀጥታ ስርዓተ ክወና ኢንትሮፒ ጋር እና በመውረድ ላይ መካከለኛ ሁኔታን ዜሮ ያደርጋል።

ሁሉም ሚስጥሮች በ `Zeroizing` ኮንቴይነሮች ውስጥ ይቀመጣሉ እና CI PQCleanን ይለማመዳል
በእያንዳንዱ የሚደገፍ መድረክ ላይ ማሰር.

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

##እንዴት እንጠቀምበታለን።

1. ** ጥገኝነቱን ጨምሩ *** ከመሥሪያ ቦታ ስር ውጭ በተቀመጡ ሳጥኖች ውስጥ።

   ```toml
   soranet_pq = { path = "../../crates/soranet_pq" }
   ```

2. ** በጥሪ ቦታዎች ላይ ትክክለኛውን ስብስብ ይምረጡ ** ለመጀመሪያው ድብልቅ የእጅ መጨባበጥ
   ሥራ፣ `MlKemSuite::MlKem768` እና `MlDsaSuite::MlDsa65` ይጠቀሙ።

3. **ከስያሜዎች ጋር ቁልፎችን አምጡ።** `HkdfDomain::soranet("KEM/1")` (እና እህትማማቾች) ተጠቀም።
   ስለዚህ የግልባጭ ሰንሰለቱ በመስቀለኛ መንገድ ላይ የሚወሰን ሆኖ ይቆያል።

4. **የመውደቅ ሚስጥሮችን ናሙና ሲወስዱ የተከለለ RNG ይጠቀሙ፡-

   ```rust
   use soranet_pq::{hedged_chacha20_rng, HedgedRngSeed};

   let mut rng = hedged_chacha20_rng(HedgedRngSeed::new(b"snnet16", [0u8; 32]));
   ```

ዋናው የሶራኔት እጅ መጨባበጥ እና CID ዓይነ ስውር ረዳቶች (`iroha_crypto::soranet`)
እነዚህን መገልገያዎች በቀጥታ ይጎትቱ, ይህም ማለት የታችኛው ተፋሰስ ሳጥኖች ተመሳሳይ ይወርሳሉ
የ PQClean ማሰሪያዎችን ሳያገናኙ አተገባበር.

## የማረጋገጫ ዝርዝር

- `cargo test -p soranet_pq --offline`
- `cargo fmt --package soranet_pq`
- የ README አጠቃቀም ናሙናዎችን ኦዲት (`crates/soranet_pq/README.md`)
- አንድ ጊዜ የተዳቀለ መሬት የሶራኔት የእጅ መጨባበጥ ሰነዱን ያዘምኑ