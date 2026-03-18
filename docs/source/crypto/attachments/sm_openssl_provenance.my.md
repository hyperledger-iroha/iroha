---
lang: my
direction: ltr
source: docs/source/crypto/attachments/sm_openssl_provenance.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 95a34657b6064f925995a7e9f20145d14fda681f4af1f182418b9f624047e576
source_last_modified: "2025-12-29T18:16:35.937817+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% SM OpenSSL/Tongsuo Provenance လျှပ်တစ်ပြက်
% ထုတ်ပေးသည်- 2026-01-30

#ပတ်ဝန်းကျင်အကျဉ်းချုပ်

- `pkg-config --modversion openssl`: `3.6.0`
- `openssl version -a`- `LibreSSL 3.3.6` (macOS တွင် စနစ်မှပေးသော TLS ကိရိယာအစုံအလင်) ကို အစီရင်ခံသည်။
- `cargo tree -p iroha_crypto --features "sm sm-ffi-openssl"`- အတိအကျ Rust မှီခိုမှုအစုအတွက် `sm_iroha_crypto_tree.txt` ကိုကြည့်ပါ (`openssl` crate v0.10.74၊ `openssl-sys` v0.9.x၊ ရောင်းချသော OpenSSL 3.800008 မှတဆင့်ရရှိနိုင်သော OpenSSL180NI00 အင်္ဂါရပ်မှ `openssl` crate v0. `vendored` ကို `crates/iroha_crypto/Cargo.toml` တွင် ဖွင့်ထားသည်)။

#မှတ်စုများ

- LibreSSL ခေါင်းစီး/စာကြည့်တိုက်များနှင့် ဒေသဆိုင်ရာ ဖွံ့ဖြိုးတိုးတက်ရေးဆိုင်ရာ ချိတ်ဆက်မှုများ၊ ထုတ်လုပ်မှုအကြိုကြည့်ရှုခြင်းတည်ဆောက်မှုများသည် OpenSSL >= 3.0.0 သို့မဟုတ် Tongsuo 8.x ကို အသုံးပြုရပါမည်။ နောက်ဆုံးအသုံးအဆောင်အစုအဝေးကိုထုတ်ပေးသည့်အခါ စနစ်ကိရိယာအစုံကို အစားထိုးပါ သို့မဟုတ် `OPENSSL_DIR`/`PKG_CONFIG_PATH` ကို သတ်မှတ်ပါ။
- အတိအကျ OpenSSL/Tongsuo tarball hash (`openssl version -v`, `openssl version -b`, `openssl version -f`) ကိုဖမ်းယူရန် ထုတ်ဝေမှုတည်ဆောက်မှုပတ်ဝန်းကျင်အတွင်းတွင် ဤလျှပ်တစ်ပြက်ကို ပြန်လည်ထုတ်ပေးပြီး ပြန်လည်ထုတ်လုပ်နိုင်သော တည်ဆောက်မှုစခရစ်/ချက်လက်မှတ်ကို ပူးတွဲပါရှိသည်။ ရောင်းချပြီးသော တည်ဆောက်မှုများအတွက်၊ Cargo အသုံးပြုသော `openssl-src` သေတ္တာဗားရှင်း/ကတိကို မှတ်တမ်းတင်ပါ (`target/debug/build/openssl-sys-*/output` တွင် မြင်နိုင်သည်)။
- Apple Silicon host များသည် OpenSSL မီးခိုးကြိုးကို အသုံးပြုသည့်အခါ `RUSTFLAGS=-Aunsafe-code` လိုအပ်သောကြောင့် AArch64 SM3/SM4 acceleration stubs များကို compile (macOS တွင် မရရှိနိုင်ပါ)။ CI နှင့် ဒေသန္တရလုပ်ဆောင်ချက်များ တသမတ်တည်းရှိနေစေရန် `scripts/sm_openssl_smoke.sh` သည် `cargo` ကိုမခေါ်ဆိုမီ ဤအလံကို တင်ပို့သည်။
- ထုပ်ပိုးမှုပိုက်လိုင်းကို ချိတ်ပြီးသည်နှင့် အထက်စီးကြောင်းရင်းမြစ် သက်သေကို ပူးတွဲပါ (ဥပမာ၊ `openssl-src-<ver>.tar.gz` SHA256)၊ CI artefacts များတွင် တူညီသော hash ကိုသုံးပါ။