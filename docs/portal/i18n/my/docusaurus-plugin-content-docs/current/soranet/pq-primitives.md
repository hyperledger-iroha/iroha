---
id: pq-primitives
lang: my
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

::: Canonical Source ကို သတိပြုပါ။
:::

`soranet_pq` သေတ္တာတွင် SoraNet တိုင်းတွင်ရှိသော ကွမ်တမ်လွန်တည်ဆောက်မှုလုပ်ကွက်များပါရှိသည်။
relay, client, နှင့် tooling အစိတ်အပိုင်းအပေါ်မှီခိုသည်။ ၎င်းသည် PQClean ကျောထောက်နောက်ခံပြု Kyber ကို ခြုံငုံသည်။
(ML-KEM) နှင့် Dilithium (ML-DSA) ပရိုတိုကော-ဖော်ရွေသော HKDF တွင်လည်းကောင်း၊
RNG အကူအညီများကို အကာအကွယ်ပေးထားသည့်အတွက် မျက်နှာပြင်အားလုံးသည် တူညီသောအကောင်အထည်ဖော်မှုများကို မျှဝေပါသည်။

## `soranet_pq` မှာ ဘာသင်္ဘောလဲ။

- **ML-KEM-512/768/1024:** အဆုံးအဖြတ်ပေးသော သော့ထုတ်လုပ်ခြင်း၊
  decapsulation helpers တွေ နဲ့ အတူ အဆက်မပြတ် အချိန် အမှား ထွက်လာပါတယ်။
- **ML-DSA-44/65/87:** အတွက် ကြိုးတပ်ထားသော ဆိုင်းထိုးခြင်း/အတည်ပြုခြင်း
  ဒိုမိန်း-ခြားထားသော စာသားမှတ်တမ်းများ။
- ** တံဆိပ်တပ်ထားသော HKDF-** `derive_labeled_hkdf` သည် ဆင်းသက်လာမှုတိုင်းကို namespace ပေးသည်
  လက်ဆွဲနှုတ်ဆက်သည့်အဆင့် (`DH/es`၊ `KEM/1`၊ …) ထို့ကြောင့် ပေါင်းစပ်စာသားများသည် အတိုက်အခိုက်ကင်းစွာ ရှိနေပါသည်။
- **Hedged randomness-** `hedged_chacha20_rng` သည် အဆုံးအဖြတ်ပေးသော အစေ့များကို ရောစပ်ထားသည်
  တိုက်ရိုက် OS entropy နှင့်အတူ ကျဆင်းသွားသည့် အလယ်အလတ်အခြေအနေကို သုညဖြစ်စေသည်။

လျှို့ဝှက်ချက်များအားလုံးကို `Zeroizing` ကွန်တိန်နာအတွင်းတွင် ရှိပြီး CI လေ့ကျင့်ခန်း PQClean
ပံ့ပိုးထားသော ပလပ်ဖောင်းတိုင်းတွင် စည်းနှောင်မှုရှိသည်။

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

##ဘယ်လိုစားသုံးရမလဲ

1. **လုပ်ငန်းခွင်၏ အမြစ်အပြင်ဘက်ရှိ သေတ္တာများတွင် မှီခိုမှု**ကို ပေါင်းထည့်ပါ-

   ```toml
   soranet_pq = { path = "../../crates/soranet_pq" }
   ```

2. **ခေါ်ဆိုမှုဆိုဒ်များတွင် မှန်ကန်သော suite** ကိုရွေးချယ်ပါ။ ကနဦးပေါင်းစပ်လက်ဆွဲနှုတ်ဆက်ခြင်းအတွက်
   အလုပ်တွင် `MlKemSuite::MlKem768` နှင့် `MlDsaSuite::MlDsa65` ကိုသုံးပါ။

3. **အညွှန်းများဖြင့် သော့များကို ရယူပါ။** `HkdfDomain::soranet("KEM/1")` (နှင့် မောင်နှမများ) ကို အသုံးပြုပါ။
   ထို့ကြောင့် စာသားမှတ်တမ်းဆွဲခြင်းသည် node များတစ်လျှောက်တွင် အဆုံးအဖြတ်အတိုင်းရှိနေပါသည်။

4. လျှို့ဝှက်ချက်များကို နမူနာယူသည့်အခါ ** အကာအကွယ် RNG** ကို အသုံးပြုပါ-

   ```rust
   use soranet_pq::{hedged_chacha20_rng, HedgedRngSeed};

   let mut rng = hedged_chacha20_rng(HedgedRngSeed::new(b"snnet16", [0u8; 32]));
   ```

ပင်မ SoraNet လက်ဆွဲနှုတ်ဆက်ခြင်း နှင့် CID မျက်စိကွယ်ခြင်း အထောက်အကူများ (`iroha_crypto::soranet`)
ဤအသုံးအဆောင်ပစ္စည်းများကို တိုက်ရိုက်ဆွဲထုတ်သည်၊ ဆိုလိုသည်မှာ ရေအောက်သေတ္တာများသည် အတူတူပင်ဖြစ်ပါသည်။
PQClean bindings များကို ၎င်းတို့ကိုယ်တိုင် ချိတ်ဆက်ခြင်းမရှိဘဲ အကောင်အထည်ဖော်မှုများ။

## မှန်ကန်ကြောင်း စစ်ဆေးရန်စာရင်း

- `cargo test -p soranet_pq --offline`
- `cargo fmt --package soranet_pq`
- README အသုံးပြုမှုနမူနာများကို စစ်ဆေးခြင်း (`crates/soranet_pq/README.md`)
- မျိုးစပ်ပြီးသည်နှင့် SoraNet လက်ဆွဲဒီဇိုင်း doc ကို အပ်ဒိတ်လုပ်ပါ။