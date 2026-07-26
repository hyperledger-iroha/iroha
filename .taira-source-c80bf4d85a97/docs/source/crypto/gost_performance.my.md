---
lang: my
direction: ltr
source: docs/source/crypto/gost_performance.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7fab384ae80e1993b1e54d6addc82fd3dc652fb6e3958bea6a04e057a1805b57
source_last_modified: "2025-12-29T18:16:35.939573+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# GOST စွမ်းဆောင်ရည် အလုပ်အသွားအလာ

ဤမှတ်စုသည် ၎င်းအတွက် စွမ်းဆောင်ရည်စာအိတ်ကို ကျွန်ုပ်တို့ မည်သို့ခြေရာခံပြီး ကျင့်သုံးကြောင်း မှတ်တမ်းတင်ထားသည်။
TC26 GOST နောက်ခံအချက်ကို လက်မှတ်ထိုးခြင်း။

## ပြည်တွင်းမှာ ပြေးဆွဲနေပါတယ်။

```bash
make gost-bench                     # run benches + tolerance check
make gost-bench GOST_BENCH_ARGS="--tolerance 0.30"  # override guard
make gost-dudect                    # run the constant-time timing guard
./scripts/update_gost_baseline.sh   # bench + rebaseline helper
```

မြင်ကွင်းနောက်ကွယ်တွင် ပစ်မှတ်နှစ်ခုလုံးသည် `scripts/gost_bench.sh` ဟုခေါ်သည်၊

1. `cargo bench -p iroha_crypto --bench gost_sign --features gost -- --noplot` ကို လုပ်ဆောင်သည်။
2. `gost_perf_check` ကို `target/criterion` နှင့် ဆန့်ကျင်ပြီး မီဒီယာများကို စစ်ဆေးခြင်း
   checked-in အခြေခံလိုင်း (`crates/iroha_crypto/benches/gost_perf_baseline.json`)။
3. ရနိုင်သည့်အခါတွင် Markdown အနှစ်ချုပ်ကို `$GITHUB_STEP_SUMMARY` ထဲသို့ ထိုးထည့်ပါ။

ဆုတ်ယုတ်မှု/တိုးတက်မှုကို အတည်ပြုပြီးနောက် အခြေခံမျဉ်းအား ပြန်လည်စတင်ရန်၊ လုပ်ဆောင်ရန်-

```bash
make gost-bench-update
```

သို့မဟုတ် တိုက်ရိုက်-

```bash
./scripts/gost_bench.sh --write-baseline \
  --baseline crates/iroha_crypto/benches/gost_perf_baseline.json
```

`scripts/update_gost_baseline.sh` သည် ခုံတန်းရှည် + checker ကို လုပ်ဆောင်သည်၊ အခြေခံ JSON ကို ထပ်ရေးပြီး ပရင့်ထုတ်သည်
မီဒီယာအသစ်များ ဆုံးဖြတ်ချက်မှတ်တမ်းနှင့်အတူ မွမ်းမံထားသော JSON ကို အမြဲတမ်း ထည့်သွင်းပါ။
`crates/iroha_crypto/docs/gost_backend.md`။

### လက်ရှိ ကိုးကားသော မီဒီယံများ

| Algorithm | ပျမ်းမျှ (µs) |
|------------------------------------|----------------|
| ed25519 | 69.67 |
| gost256_paramset_a | 1136.96 |
| gost256_paramset_b | 1129.05 |
| gost256_paramset_c | 1133.25 |
| gost512_paramset_a | 8944.39 |
| gost512_paramset_b | 8963.60 |
| secp256k1 | 160.53 |

## CI

`.github/workflows/gost-perf.yml` သည် တူညီသော script ကိုအသုံးပြုပြီး dudect timing guard ကိုလည်းလုပ်ဆောင်သည်။
တိုင်းတာထားသော မီဒီယံသည် အခြေခံသတ်မှတ်ထားသော အတိုင်းအတာထက် ကျော်လွန်သည့်အခါ CI ပျက်ကွက်သည်။
(မူရင်းအားဖြင့် 20%) သို့မဟုတ် အချိန်ကိုက်အစောင့်က ယိုစိမ့်မှုကို တွေ့ရှိသောအခါ၊ ထို့ကြောင့် ဆုတ်ယုတ်မှုများကို အလိုအလျောက်ဖမ်းမိပါသည်။

## အကျဉ်းချုပ်

`gost_perf_check` သည် နှိုင်းယှဉ်ဇယားကို စက်တွင်းတွင် ပရင့်ထုတ်ပြီး တူညီသောအကြောင်းအရာကို ပေါင်းထည့်သည်။
`$GITHUB_STEP_SUMMARY`၊ ထို့ကြောင့် CI အလုပ်မှတ်တမ်းများနှင့် အနှစ်ချုပ်များသည် တူညီသောနံပါတ်များကို မျှဝေပါသည်။