---
lang: ba
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

:::иҫкәртергә канонлы сығанаҡ
::: 1990 й.

I18NI0000000003X йәшниктә кванттан һуңғы төҙөлөш блоктары бар, улар һәр SoraNet .
реле, клиент, һәм инструменттар компоненты таяна. Ул PQClean-арҡалы Кибер уратып ала.
(ML-KEM) һәм дилитий (МЛ-ДСА) люкс һәм ҡатламдар протокол-дуҫ HKDF һәм
хеджировать RNG ярҙамсылары, шуға күрә бөтә ер өҫтө бер үк тормошҡа ашырыу менән уртаҡлаша.

## I18NI000000004X-тағы ниндәй караптар

- **МЛ-КЭМ-512/768/1024:** детерминистик төп быуын, капсулирование һәм
  декапсуляция ярҙамсылары менән даими ваҡыт хаталар таралыу.
- **МЛ-ДСА-44/65/87:** айырым ҡул ҡуйыу/тикшереү өсөн сымлы.
  домен менән айырылған транскрипттар.
- **Ярлыҡтар HKDF:** I18NI0000000005X исемдәр киңлеге менән һәр сығарылыш менән
  ҡул ҡыҫыу этабы (`DH/es`, `KEM/1`, ...) шулай гибрид стенограммалар бәрелешһеҙ ҡала.
- **Хеддж осраҡлылыҡ:** I18NI000000008X ҡатнашмалары детерминистик орлоҡтар
  тере OS энтропияһы менән һәм нулгә тиклем арауыҡ хәле тамсы.

Бөтә серҙәр ҙә I18NI000000009X контейнерҙары эсендә ултыра һәм CI PQClean күнекмәләрен башҡара.
һәр ярҙам платформаһында бәйләүҙәр.

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

## Нисек ҡулланырға .

1. **Бәйләнеште өҫтәү** йәшниктәргә, улар эш урыны тамырынан ситтә ултыра:

   ```toml
   soranet_pq = { path = "../../crates/soranet_pq" }
   ```

2. **Шылтыратыу сайттарында дөрөҫ люкс** һайлағыҙ. Тәүге гибрид ҡул ҡыҫыһы өсөн
   эш, ҡулланыу I18NI000000010X һәм `MlDsaSuite::MlDsa65`.

3. **Ярлыҡтар менән асҡыстарҙы сығарыу.** `HkdfDomain::soranet("KEM/1")` (һәм бер туғандар) ҡулланыу.
   тимәк, транскрипт сылбырлы төйөндәр буйынса детерминистик ҡала.

4. **Хеджировать RNG** ҡулланып, ҡасан үлсәү fallback серҙәре:

   ```rust
   use soranet_pq::{hedged_chacha20_rng, HedgedRngSeed};

   let mut rng = hedged_chacha20_rng(HedgedRngSeed::new(b"snnet16", [0u8; 32]));
   ```

Ядро SoraNet ҡул ҡыҫыу һәм CID һуҡыр ярҙамсылары (`iroha_crypto::soranet`)
был коммуналь хеҙмәттәр туранан-тура тартып, был тигәнде аңлата аҫҡы ағым йәшниктәр мираҫҡа бер үк
тормошҡа ашырыуҙар PQClean бәйләүҙәрен бәйләүһеҙ үҙҙәре.

## Валидация тикшерелгән исемлеге

- `cargo test -p soranet_pq --offline`
- `cargo fmt --package soranet_pq`
- README ҡулланыу өлгөләрен аудит (I18NI000000016X)
- Яңыртыу SoraNet ҡул ҡыҫышыу дизайн doc бер тапҡыр гибридтар ер .