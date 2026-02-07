---
id: pq-primitives
lang: ka
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

:::შენიშვნა კანონიკური წყარო
:::

`soranet_pq` კრატი შეიცავს პოსტ-კვანტურ სამშენებლო ბლოკებს, რომლებსაც ყოველი SoraNet
რელეს, კლიენტს და ხელსაწყოების კომპონენტს ეყრდნობა. იგი ახვევს PQClean-ის მხარდაჭერით Kyber-ს
(ML-KEM) და Dilithium (ML-DSA) კომპლექტები და ფენები პროტოკოლისთვის მეგობრულ HKDF-ზე და
ჰეჯირებული RNG დამხმარეები, ასე რომ ყველა ზედაპირი იზიარებს იდენტურ განხორციელებას.

## რა იგზავნება `soranet_pq`-ში

- **ML-KEM-512/768/1024:** განმსაზღვრელი გასაღების გენერირება, ინკაფსულაცია და
  დეკაფსულაციის დამხმარეები მუდმივი დროის შეცდომის გამრავლებით.
- **ML-DSA-44/65/87:** მოწყვეტილი ხელმოწერა/დადასტურება გაყვანილია
  დომენით გამოყოფილი ტრანსკრიპტები.
- ** იარლიყით HKDF:** `derive_labeled_hkdf` ყოველი წარმოშობის სახელთა სივრცეში
  ხელის ჩამორთმევის ეტაპი (`DH/es`, `KEM/1`,…) ასე რომ, ჰიბრიდული ტრანსკრიპტები შეჯახების გარეშე დარჩეს.
- ** ჰეჯირებული შემთხვევითობა:** `hedged_chacha20_rng` აერთიანებს დეტერმინისტულ თესლებს
  ცოცხალი OS ენტროპიით და ნულობს შუალედურ მდგომარეობას ვარდნისას.

ყველა საიდუმლო ზის `Zeroizing` კონტეინერებში და CI ახორციელებს PQClean-ს
საკინძები ყველა მხარდაჭერილ პლატფორმაზე.

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

## როგორ მოვიხმაროთ

1. **დამოკიდებულების დამატება** ყუთებს, რომლებიც დევს სამუშაო სივრცის ფესვის გარეთ:

   ```toml
   soranet_pq = { path = "../../crates/soranet_pq" }
   ```

2. **აირჩიეთ სწორი კომპლექტი** ზარის საიტებზე. პირველადი ჰიბრიდული ხელის ჩამორთმევისთვის
   მუშაობა, გამოიყენეთ `MlKemSuite::MlKem768` და `MlDsaSuite::MlDsa65`.

3. **გასაღები ეტიკეტებით.** გამოიყენეთ `HkdfDomain::soranet("KEM/1")` (და და-ძმები)
   ასე რომ, ტრანსკრიპტის მიჯაჭვულობა დეტერმინისტული რჩება კვანძებში.

4. **გამოიყენეთ ჰეჯირებული RNG** სარეზერვო საიდუმლოების შერჩევისას:

   ```rust
   use soranet_pq::{hedged_chacha20_rng, HedgedRngSeed};

   let mut rng = hedged_chacha20_rng(HedgedRngSeed::new(b"snnet16", [0u8; 32]));
   ```

SoraNet-ის ძირითადი ხელის ჩამორთმევისა და CID დამაბრმავებელი დამხმარეები (`iroha_crypto::soranet`)
გაიყვანეთ ეს კომუნალური საშუალებები პირდაპირ, რაც ნიშნავს, რომ ქვედა დინების უჯრები იგივე მემკვიდრეობით მიიღება
იმპლემენტაციები თავად PQClean აკავშირების გარეშე.

## ვალიდაციის საკონტროლო სია

- `cargo test -p soranet_pq --offline`
- `cargo fmt --package soranet_pq`
- README გამოყენების ნიმუშების აუდიტი (`crates/soranet_pq/README.md`)
- განაახლეთ SoraNet ხელის ჩამორთმევის დიზაინის დოკუმენტი, როგორც კი ჰიბრიდები დაეშვება