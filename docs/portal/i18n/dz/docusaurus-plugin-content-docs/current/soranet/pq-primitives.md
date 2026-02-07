---
id: pq-primitives
lang: dz
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

:::དྲན་ཐོའི་འབྱུང་ཁུངས།
:::

`soranet_pq` crete ནང་ལུ་ SoraNet རེ་རེ་ལུ་ ཚད་ལྡན་གྱི་ སྒྲིང་ཁྱིམ་གྱི་ས་ཁོངས་ཚུ་ཡོདཔ་ཨིན།
རི་ལེ་དང་ མཁོ་སྤྲོད་འབད་མི་ དེ་ལས་ ལག་ཆས་ཆ་ཤས་ཚུ་ བརྟེན་དོ་ཡོདཔ་ཨིན། འདི་གིས་ PQClean-backed ཀའི་བཱར་འདི་བཀབ་ཨིན།
(ML-KEM) དང་ ཌི་ལི་ཐི་ཡམ་ (ML-DSA) དང་ བང་རིམ་ཚུ་ མཐུན་སྒྲིག་ཅན་གྱི་ HKDF དང་ གུ་ཡངས།
RNG གི་གྲོགས་རམ་པ་ཚུ་ ཁ་ཐོག་ཆ་མཉམ་གྱིས་ འདྲ་མཚུངས་ཀྱི་ལག་ལེན་ཚུ་ བརྗེ་སོར་འབདཝ་ཨིན།

## I18NI0000004X ནང་གྲུ་གང་།

- **ML-KEM-512/768/1024:** གཏན་འཁེལ་གྱི་ལྡེ་མིག་ བཀོད་སྒྲིག་དང་ དང་།
  དུས་ཚོད་ཀྱི་འཛོལ་བ་ དུས་རྒྱུན་དུ་ ཁྱབ་སྤེལ་འབད་མི་ བརྡ་དོན་གྲོགས་རམ་པ་ཚུ།
- **ML-DSA-44/65/87:** བཀྲམ་སྟོན་འབད་ཡོད་པའི་མིང་རྟགས་/བདེན་དཔྱད་འདི་ དོན་ལུ་ ཝའིར་འབད་ཡོདཔ།
  domain-དབྱེ་བ་ཕྱེ་ཡོད་པའི་ཡིག་བསྒྱུར་ཚུ།
- **Labelled HKDF:** `derive_labeled_hkdf` མིང་གནས་རེ་རེ་བཞིན་ 18NI0000005X མིང་ཚད།
  ལག་པའི་གནས་རིམ་ (I18NI000000006X, I18NI000000007X, ...) དེ་འབདཝ་ལས་ རིགས་སོན་ཡིག་བསྒྱུར་ཚུ་ ཁ་ཐུག་མ་རྐྱབ་པར་སྡོད།
- **གང་བྱུང་གིས་ གང་བྱུང་།:** `hedged_chacha20_rng` གཏན་འབེབས་ཀྱི་སོན་རིགས་ཚུ།
  OS entropy དང་བར་མཚམས་གནས་སྟངས་ཀླད་ཀོར་འདི་ བླུག་སྟེ་ཡོདཔ་ཨིན།

`Zeroizing` གི་ནང་དུ་གསང་བ་ཚང་མ་དང་ CI གིས་ PQClean ལག་ལེན་འཐབ་ཨིན།
རྒྱབ་སྐྱོར་འབད་མི་ སྟེགས་བུ་ག་ར་གུ་ བསྡམས་ཡོདཔ།

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

## བཏུང་སྟངས།

1.*བརྟེན་གནས་** ལཱ་གི་ས་སྒོ་གི་ཕྱི་ཁར་སྡོད་མི་ ཀེརེསི་ལུ་ཁ་སྐོང་འབད།

   ```toml
   soranet_pq = { path = "../../crates/soranet_pq" }
   ```

༢ ** འབོད་བརྡ་ས་ཁོངས་ཚུ་ནང་ ངེས་བདེན་ ཆ་ཚན་** སེལ་འཐུ་འབད། འགོ་ཐོག་རིགས་སོན་ལག་བཟོའི་དོན་ལུ།
   work, ལག་ལེན་ I18NI000000010X དང་ I18NI000000011X ལག་ལེན་འཐབ།

3. **ཁ་ཡིག་ཚུ་ཡོད་པའི་ལྡེ་མིག་ཚུ་ ཕར་འགྱངས་འབད་དགོ།** `HkdfDomain::soranet("KEM/1")` (དང་སྤུན་ཆ་) ལག་ལེན་འཐབ།
   དེ་འབདཝ་ལས་ ཡིག་བསྒྱུར་འབད་ནི་འདི་ མཐུད་མཚམས་ཚུ་ནང་ལུ་ གཏན་འབེབས་བཟོཝ་ཨིན།

4. ** ཕོལ་བེག་གསང་བ་དཔེ་ཚད་བཏོན་པའི་སྐབས་ RNG** འདི་ལག་ལེན་འཐབ།

   I18NF0000002X

SoraNet ལག་ཤེད་དང་ CID གིས་ མིག་བཙུམ་བཏང་མི་ (I18NI0000013X)
འདི་ཚུ་ཐད་ཀར་དུ་འཐེན། འདི་ཡང་ མར་གྱི་ཕུང་པོ་འདི་ གཅིག་མཚུངས་སྦེ་ཡོདཔ་ཨིན།
བཀོལ་སྤྱོད་ཚུ་ PQLean མཐུད་སྦྲེལ་ཚུ་ རང་སོ་གིས་ འབྲེལ་མཐུད་འབད་ཡོདཔ།

## བདེན་དཔྱད་ཀྱི་ཐོ་ཡིག་།

- `cargo test -p soranet_pq --offline`
- `cargo fmt --package soranet_pq`
- README ལག་ལེན་གྱི་དཔེ་ཚད་ (`crates/soranet_pq/README.md`) རྩིས་ཞིབ་འབད།
- སོ་ར་ནེཊ་ལག་བརྡ་བཟོ་བཀོད་ཀྱི་ ཡིག་ཆ་འདི་ རིགས་སོན་གྱི་ས་ཆ་གཅིག་དུས་མཐུན་བཟོ་ནི།