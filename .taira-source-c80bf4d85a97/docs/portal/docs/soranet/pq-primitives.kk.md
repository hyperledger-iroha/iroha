---
lang: kk
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

:::ескерту Канондық дереккөз
:::

`soranet_pq` жәшігінде әрбір SoraNet-те болатын посткванттық құрылыс блоктары бар.
реле, клиент және құрал құрамдас бөліктеріне сүйенеді. Ол PQClean қолдайтын Kyber-ді орап алады
(ML-KEM) және Dilithium (ML-DSA) пакеттері мен қабаттары протоколға ыңғайлы HKDF және
хеджирленген RNG көмекшілері, сондықтан барлық беттер бірдей іске асыруды бөліседі.

## `soranet_pq` ішінде не жеткізіледі

- **ML-KEM-512/768/1024:** детерминирленген кілтті құру, инкапсуляция және
  тұрақты уақыт қателерінің таралуы бар декапсуляция көмекшілері.
- **ML-DSA-44/65/87:** бөлек қол қою/тексеру
  доменмен бөлінген транскрипттер.
- **Белгіленген HKDF:** `derive_labeled_hkdf` әрбір туындының аттар кеңістігі
  қол алысу кезеңі (`DH/es`, `KEM/1`, …), сондықтан гибридті транскрипттер соқтығыспайды.
- **Хеджирленген кездейсоқтық:** `hedged_chacha20_rng` детерминирленген тұқымдарды араластырады
  тірі ОЖ энтропиясымен және түсіргенде аралық күйді нөлге келтіреді.

Барлық құпиялар `Zeroizing` контейнерлерінің ішінде орналасқан және CI PQClean жүйесін пайдаланады.
әрбір қолдау көрсетілетін платформадағы байлаулар.

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

## Оны қалай тұтыну керек

1. **Тәуелділікті** жұмыс кеңістігінің түбірінен тыс орналасқан жәшіктерге қосыңыз:

   ```toml
   soranet_pq = { path = "../../crates/soranet_pq" }
   ```

2. Қоңырау шалу орындарында **Дұрыс топтаманы таңдаңыз**. Бастапқы гибридті қол алысу үшін
   жұмыс істеу үшін `MlKemSuite::MlKem768` және `MlDsaSuite::MlDsa65` пайдаланыңыз.

3. **Белгілері бар кілттерді алыңыз.** `HkdfDomain::soranet("KEM/1")` (және бауырлар) пайдаланыңыз
   сондықтан транскрипт тізбегі түйіндер бойынша детерминистік болып қалады.

4. **Қолданбалы құпияларды іріктеу кезінде хеджирленген RNG** пайдаланыңыз:

   ```rust
   use soranet_pq::{hedged_chacha20_rng, HedgedRngSeed};

   let mut rng = hedged_chacha20_rng(HedgedRngSeed::new(b"snnet16", [0u8; 32]));
   ```

Негізгі SoraNet қол алысу және CID соқыр көмекшілері (`iroha_crypto::soranet`)
бұл утилиталарды тікелей тартыңыз, бұл төменгі ағындағы жәшіктер бірдей мұраға ие болады
PQClean байланыстарының өздерін байланыстырмай жүзеге асыру.

## Валидацияны тексеру парағы

- `cargo test -p soranet_pq --offline`
- `cargo fmt --package soranet_pq`
- README пайдалану үлгілерін тексеру (`crates/soranet_pq/README.md`)
- Гибридтер қонғаннан кейін SoraNet қол алысу дизайны құжатын жаңартыңыз