---
lang: my
direction: ltr
source: docs/source/crypto/sm_rust_vector_check.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ce2f95b8b287c18c39232418333fbefdd300c030391be9dbfa4e29a3fd5f3e14
source_last_modified: "2025-12-29T18:16:35.946190+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! RustCrypto သေတ္တာများကို အသုံးပြု၍ SM2 နောက်ဆက်တွဲ D vector များကို စစ်ဆေးခြင်းဆိုင်ရာ မှတ်စုများ

# SM2 နောက်ဆက်တွဲ D Vector Verification (RustCrypto)

ဤဖော်ပြချက်သည် RustCrypto ၏ `sm2` သေတ္တာဖြင့် GM/T 0003 နောက်ဆက်တွဲ D နမူနာအား အတည်ပြုရန် (နှင့် အမှားရှာရန်) အသုံးပြုသည့် အဆင့်များကို ဖမ်းယူပါသည်။ Canonical Annex Example 1 data (identity `ALICE123@YAHOO.COM`၊ message `"message digest"` နှင့်ထုတ်ဝေထားသော `(r, s)`) ကို ယခု `crates/iroha_crypto/tests/fixtures/sm_known_answers.toml` တွင် မှတ်တမ်းတင်ထားပါသည်။ OpenSSL/Tongsuo/gmssl လက်မှတ်ကို ပျော်ရွှင်စွာအတည်ပြုပါ (`sm_vectors.md` ကိုကြည့်ပါ)၊ သို့သော် RustCrypto ၏ `sm2 v0.13.3` သည် `signature::Error` ပါအချက်ကို ငြင်းပယ်ဆဲဖြစ်သောကြောင့် CLI parity ကို အတည်ပြုပါသည်။ Rust ကြိုးအတက်ရေစီးကြောင်းကို ဆက်လက်ပြင်ဆင်နေချိန်တွင် CLI မှ အတည်ပြုပါသည်။

## ယာယီသေတ္တာ

```bash
cargo new /tmp/sm2_verify --bin
cd /tmp/sm2_verify
```

`Cargo.toml`-

```toml
[package]
name = "sm2_verify"
version = "0.1.0"
edition = "2024"

[dependencies]
hex = "0.4"
sm2 = "0.13.3"
```

`src/main.rs`-

```rust
use hex::FromHex;
use sm2::dsa::{signature::Verifier, Signature, VerifyingKey};

fn main() {
    let distid = "ALICE123@YAHOO.COM";
    let sig_bytes = <Vec<u8>>::from_hex(
        "40f1ec59f793d9f49e09dcef49130d4194f79fb1eed2caa55bacdb49c4e755d16fc6dac32c5d5cf10c77dfb20f7c2eb667a457872fb09ec56327a67ec7deebe7",
    )
    .expect("signature hex");
    let sig_array = <[u8; 64]>::try_from(sig_bytes.as_slice()).unwrap();
    let signature = Signature::from_bytes(&sig_array).unwrap();

    let public_key = <Vec<u8>>::from_hex(
        "040ae4c7798aa0f119471bee11825be46202bb79e2a5844495e97c04ff4df2548a7c0240f88f1cd4e16352a73c17b7f16f07353e53a176d684a9fe0c6bb798e857",
    )
    .expect("public key hex");

    // This still returns Err with RustCrypto 0.13.3 – track upstream.
    let verifying_key = VerifyingKey::from_sec1_bytes(distid, &public_key).unwrap();

    verifying_key
        .verify(b"message digest", &signature)
        .expect("signature verified");
}
```

## တွေ့ရှိချက်

- `sm2::VerifyingKey::from_sec1_bytes` သည် `signature::Error` ကို ပြန်ပေးသောကြောင့် လက်ရှိတွင် Canonical နောက်ဆက်တွဲ ဥပမာ 1 `(r, s)` ကို စစ်ဆေးခြင်း မအောင်မြင်ပါ။ ရေစီးကြောင်း/အမြစ် အကြောင်းရင်းကို ခြေရာခံပါ (ကိတ်၏ လက်ရှိထုတ်လွှတ်မှုတွင် မျဉ်းကွေး-ပါရာမီတာ မကိုက်ညီမှုကြောင့် ဖြစ်နိုင်သည်)။
- ကြိုးသည် `sm2 v0.13.3` ဖြင့် ရှင်းရှင်းလင်းလင်း စုစည်းပြီး RustCrypto (သို့မဟုတ် ဖာထေးထားသော ခက်ရင်းရင်း) နောက်ဆက်တွဲ ဥပမာ 1 အမှတ်/လက်မှတ်အတွဲကို လက်ခံပြီးသည်နှင့် အလိုအလျောက် ဆုတ်ယုတ်မှုစမ်းသပ်မှု ဖြစ်လာပါမည်။
- OpenSSL/Tongsuo/gmssl အတည်ပြုခြင်း `sm_vectors.md` တွင် ညွှန်ကြားချက်များဖြင့် အောင်မြင်သည်။ LibreSSL (macOS မူရင်း) သည် SM2/SM3 ပံ့ပိုးမှု အားနည်းနေသေးသောကြောင့် ဒေသတွင်း ကွာဟချက်ဖြစ်သည်။

## နောက်တစ်ဆင့်

1. `sm2` သည် နောက်ဆက်တွဲ ဥပမာ 1 မှတ်ကို လက်ခံသည့် API တစ်ခုကို ဖော်ထုတ်ပြီးသည်နှင့် (သို့မဟုတ် အထက်ပိုင်းမျဉ်းကွေးဘောင်များကို အတည်ပြုပြီးနောက်) သို့ ပြန်လည်စမ်းသပ်ခြင်းဖြင့် ကြိုးသည် စက်တွင်းသို့ ဖြတ်သန်းနိုင်သည်။
2. RustCrypto သည် မြေယာများကို မပြင်မချင်း CI ပိုက်လိုင်းများတွင် CLI သန့်ရှင်းမှု စစ်ဆေးချက် (OpenSSL/Tongsuo/gmssl) ကို ထားရှိပါ။
3. RustCrypto နှင့် OpenSSL တူညီမှုစစ်ဆေးမှုနှစ်ခုလုံးအောင်မြင်ပြီးနောက် Iroha ၏ဆုတ်ယုတ်မှုအစုအဝေးတွင် ကြိုးကို မြှင့်တင်ပါ။