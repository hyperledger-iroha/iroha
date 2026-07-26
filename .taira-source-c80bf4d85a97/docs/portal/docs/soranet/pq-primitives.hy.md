---
lang: hy
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

:::note Կանոնական աղբյուր
:::

`soranet_pq` արկղը պարունակում է հետքվանտային շինանյութեր, որոնք յուրաքանչյուր SoraNet
ռելեի, հաճախորդի և գործիքավորման բաղադրիչի վրա հիմնված է: Այն փաթաթում է PQClean-ով ապահովված Kyber-ը
(ML-KEM) և Dilithium (ML-DSA) փաթեթներ և շերտեր պրոտոկոլի համար հարմար HKDF և
հեջավորված RNG օգնականներ, այնպես որ բոլոր մակերեսները կիսում են նույնական իրականացումները:

## Ինչ առաքվում է `soranet_pq`-ում

- **ML-KEM-512/768/1024:** դետերմինիստական բանալիների ստեղծում, պարփակում և
  դեկապսուլյացիայի օգնականներ՝ անընդհատ ժամանակի սխալի տարածմամբ:
- **ML-DSA-44/65/87:** անջատված ստորագրման/ստուգման լարով
  տիրույթով առանձնացված տառադարձումներ.
- **Նշված HKDF:** `derive_labeled_hkdf`-ը յուրաքանչյուր ածանցյալ անվանումով
  ձեռքսեղմման փուլ (`DH/es`, `KEM/1`, …), այնպես որ հիբրիդային տառադարձումները մնում են առանց բախումների:
- **Ցանկապատված պատահականություն.** `hedged_chacha20_rng`-ը միախառնում է դետերմինիստական սերմերը
  կենդանի ՕՀ էնտրոպիայով և զրոյացնում է միջանկյալ վիճակը անկման ժամանակ:

Բոլոր գաղտնիքները նստած են `Zeroizing` բեռնարկղերի ներսում և CI-ն իրականացնում է PQClean-ը
կապեր յուրաքանչյուր աջակցվող հարթակի վրա:

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

## Ինչպես օգտագործել այն

1. **Ավելացրեք կախվածությունը** արկղերին, որոնք տեղակայված են աշխատանքային տարածքի արմատից դուրս.

   ```toml
   soranet_pq = { path = "../../crates/soranet_pq" }
   ```

2. **Ընտրեք ճիշտ փաթեթը** զանգերի կայքերում: Նախնական հիբրիդային ձեռքսեղմման համար
   աշխատել, օգտագործել `MlKemSuite::MlKem768` և `MlDsaSuite::MlDsa65`:

3. **Ստեղծեք ստեղներ պիտակներով։** Օգտագործեք `HkdfDomain::soranet("KEM/1")` (և եղբայրներին ու քույրերին)
   Այսպիսով, տառադարձման շղթայականացումը մնում է դետերմինիստական ամբողջ հանգույցներում:

4. **Օգտագործեք հեջավորված RNG** հետադարձ գաղտնիքները նմուշառելիս.

   ```rust
   use soranet_pq::{hedged_chacha20_rng, HedgedRngSeed};

   let mut rng = hedged_chacha20_rng(HedgedRngSeed::new(b"snnet16", [0u8; 32]));
   ```

SoraNet-ի հիմնական ձեռքսեղմման և CID կուրացնող օգնականները (`iroha_crypto::soranet`)
ուղղակիորեն քաշեք այս կոմունալ ծառայությունները, ինչը նշանակում է, որ հոսանքով ներքև գտնվող արկղերը նույնն են ժառանգում
իրականացումներ՝ առանց կապելու PQClean-ի կապերը:

## Վավերացման ստուգաթերթ

- `cargo test -p soranet_pq --offline`
- `cargo fmt --package soranet_pq`
- Աուդիտ README-ի օգտագործման նմուշները (`crates/soranet_pq/README.md`)
- Թարմացրեք SoraNet ձեռքսեղմման դիզայնի փաստաթուղթը, երբ հիբրիդները վայրէջք կատարեն