---
lang: my
direction: ltr
source: docs/portal/docs/sorafs/quickstart.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 79a048e6061f7054e14a471004cf7da0dddd3f9bf627d9f1d20ff63803cb0979
source_last_modified: "2026-01-05T09:28:11.908615+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

#SoraFS အမြန်စတင်ပါ။

ဤလက်တွေ့လမ်းညွှန်သည် အဆုံးအဖြတ်ပေးသော SF-1 chunker ပရိုဖိုင်ကို ဖြတ်သန်းပြီး၊
SoraFS ကို နောက်ခံပြုထားသည့် သက်သေပြလက်မှတ်ထိုးခြင်းနှင့် ဝန်ဆောင်မှုပေးသူ အစုံအလင် ရယူခြင်း
သိုလှောင်မှုပိုက်လိုင်း။ [manifest pipeline deep dive](manifest-pipeline.md) ဖြင့် ၎င်းကိုတွဲပါ
ဒီဇိုင်းမှတ်စုများနှင့် CLI အလံရည်ညွှန်းပစ္စည်းအတွက်။

## လိုအပ်ချက်များ

- Rust toolchain (`rustup update`)၊ လုပ်ငန်းခွင်နေရာကို စက်တွင်းမှ ပုံတူကူးထားသည်။
- ရွေးချယ်နိုင်သော- [OpenSSL မှထုတ်လုပ်ထားသော Ed25519 သော့ချိတ်](https://github.com/hyperledger-iroha/iroha/tree/master/defaults/dev-keys#readme)
  လက်မှတ်ရေးထိုးခြင်းအတွက်
- ရွေးချယ်နိုင်သော- Docusaurus ပေါ်တယ်ကို အစမ်းကြည့်ရှုရန် စီစဉ်ထားပါက Node.js ≥ 18။

အထောက်အကူဖြစ်စေသော CLI မက်ဆေ့ဂျ်များပေါ်စေရန် စမ်းသပ်နေစဉ် `export RUST_LOG=info` ကို သတ်မှတ်ပါ။

## 1. အဆုံးအဖြတ်ပေးသော ပွဲများကို ပြန်လည်စတင်ပါ။

canonical SF-1 အတုံးလိုက်အကွက်များကို ပြန်ထုတ်ပါ။ အမိန့်ထုတ်သည်ကိုလည်း လက်မှတ်ရေးထိုးသည်။
`--signing-key` ကို ပေးဆောင်သောအခါတွင် စာအိတ်များ၊ `--allow-unsigned` ကိုသုံးပါ။
ဒေသဖွံ့ဖြိုးရေးအတွက်သာ။

```bash
cargo run -p sorafs_chunker --bin export_vectors -- --allow-unsigned
```

အထွက်များ-

- `fixtures/sorafs_chunker/sf1_profile_v1.{json,rs,ts,go}`
- `fixtures/sorafs_chunker/manifest_blake3.json`
- `fixtures/sorafs_chunker/manifest_signatures.json` (လက်မှတ်ရေးထိုးခဲ့လျှင်)
- `fuzz/sorafs_chunker/sf1_profile_v1_{input,backpressure}.json`

## 2. payload ကိုဖြတ်ပြီး အစီအစဉ်ကို စစ်ဆေးပါ။

မတရားသောဖိုင် သို့မဟုတ် မှတ်တမ်းကို အပိုင်းပိုင်းခွဲရန် `sorafs_chunker` ကိုသုံးပါ-

```bash
echo "SoraFS deterministic chunking" > /tmp/docs.txt
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- /tmp/docs.txt \
  > /tmp/docs.chunk-plan.json
```

အဓိကနယ်ပယ်များ-

- `profile` / `break_mask` – `sorafs.sf1@1.0.0` ဘောင်များကို အတည်ပြုသည်။
- `chunks[]` - အော့ဖ်ဆက်များ၊ အလျားများနှင့် အတုံးအခဲများကို BLAKE3 အချေအတင်များကို မှာကြားထားသည်။

ပိုကြီးသောပွဲများအတွက်၊ တိုက်ရိုက်ကြည့်ရှုခြင်းနှင့် သေချာစေရန်အတွက် proptest-backed regression ကို run ပါ။
batch chunking သည် ထပ်တူကျနေ:

```bash
cargo test -p sorafs_chunker streaming_backpressure_fuzz_matches_batch
```

## 3. မန်နီးဖက်စ်တစ်ခုကို တည်ဆောက်ပြီး လက်မှတ်ထိုးပါ။

အတုံးအခဲအစီအစဥ်၊ နာမည်တူနှင့် အုပ်ချုပ်မှုလက်မှတ်များကို ထင်ရှားစွာအသုံးပြု၍ သရုပ်ဖော်ပါ။
`sorafs-manifest-stub`။ အောက်ပါ command သည် single-file payload ကိုပြသသည်၊ ဖြတ်သန်းပါ။
သစ်ပင်တစ်ပင်ကို ထုပ်ပိုးရန် လမ်းညွှန်လမ်းကြောင်းတစ်ခု (CLI သည် ၎င်းကို အဘိဓာန်အတိုင်း ကျင့်သုံးသည်)။

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub -- \
  /tmp/docs.txt \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --manifest-out=/tmp/docs.manifest \
  --manifest-signatures-out=/tmp/docs.manifest_signatures.json \
  --json-out=/tmp/docs.report.json \
  --allow-unsigned
```

`/tmp/docs.report.json` ကို သုံးသပ်ချက်-

- `chunking.chunk_digest_sha3_256` – SHA3 အော့ဖ်ဆက်များ/အရှည်များ၏ အနှစ်ချုပ်၊ ကိုက်ညီသည်
  အတုံးအခဲများ။
- `manifest.manifest_blake3` – BLAKE3 digest ကို မန်နီးဖက်စ်စာအိတ်တွင် ရေးထိုးထားသည်။
- `chunk_fetch_specs[]` - သံစုံတီးဝိုင်းအတွက် ညွှန်ကြားချက်များ ရယူရန် ညွှန်ကြားထားသည်။

လက်မှတ်အစစ်များကို ပံ့ပိုးရန် အဆင်သင့်ဖြစ်သောအခါ၊ `--signing-key` နှင့် `--signer` တို့ကို ထည့်ပါ
ဆင်ခြေများ။ အဆိုပါအမိန့်ကိုမရေးမီ Ed25519 လက်မှတ်တိုင်းကိုအတည်ပြုသည်။
စာအိတ်။

## 4. Multi-provider retrieve ကို အတုယူပါ။

တစ်ခု သို့မဟုတ် တစ်ခုထက်ပိုသော အပိုင်းအစီအစဉ်ကို ပြန်လည်ပြသရန် developer မှ ထုတ်ယူသည့် CLI ကို အသုံးပြုပါ။
ပံ့ပိုးပေးသူများ ၎င်းသည် CI မီးခိုးစမ်းသပ်မှုများနှင့် သရုပ်ဖော်မှုပုံစံတူခြင်းအတွက် စံပြဖြစ်သည်။

```bash
cargo run -p sorafs_car --bin sorafs_fetch -- \
  --plan=/tmp/docs.report.json \
  --provider=primary=/tmp/docs.txt \
  --output=/tmp/docs.reassembled \
  --json-out=/tmp/docs.fetch-report.json
```

ပြောဆိုချက်များ-

- `payload_digest_hex` သည် manifest အစီရင်ခံစာနှင့် ကိုက်ညီရမည်။
- `provider_reports[]` သည် ပံ့ပိုးသူတစ်ဦးလျှင် အောင်မြင်မှု/ကျရှုံးမှု အရေအတွက်ကို ဖော်ပြသည်။
- သုညမဟုတ်သော `chunk_retry_total` သည် နောက်ကျောဖိအားချိန်ညှိမှုများကို မီးမောင်းထိုးပြသည်။
- လည်ပတ်ရန်စီစဉ်ထားသည့်ပံ့ပိုးပေးသူအရေအတွက်ကိုကန့်သတ်ရန် `--max-peers=<n>` ကိုဖြတ်ပါ။
  နှင့် CI သရုပ်ဖော်မှုများသည် အဓိက ကိုယ်စားလှယ်လောင်းများအပေါ် အာရုံစိုက်ထားပါ။
- `--retry-budget=<n>` သည် ပုံသေတစ်ခုစီတစ်ခုစီတွင် ထပ်စမ်းခြင်းအရေအတွက် (3) ကို အစားထိုးပေးသည်
  ချို့ယွင်းချက်များကို ထိုးသွင်းသောအခါ တီးမှုတ်သူ၏ နောက်ပြန်ဆုတ်မှုများကို ပိုမိုမြန်ဆန်စွာ ပေါ်လွင်စေနိုင်သည်။

မအောင်မြင်ရန် `--expect-payload-digest=<hex>` နှင့် `--expect-payload-len=<bytes>` ကို ထည့်ပါ
ပြန်လည်တည်ဆောက်ထားသော payload သည် manifest မှ သွေဖည်သွားသောအခါ မြန်ဆန်သည်။

## 5. နောက်အဆင့်များ

- ** အုပ်ချုပ်မှုပေါင်းစပ်ခြင်း** – ထင်ရှားသောအနှစ်ချုပ်ကို ပိုက်နှင့်
  `manifest_signatures.json` သို့ Pin Registry လုပ်နိုင်သည်
  ရရှိနိုင်မှုကိုကြော်ငြာပါ။
- **Registry ညှိနှိုင်းမှု** – [`sorafs/chunker_registry.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/chunker_registry.md) တိုင်ပင်ပါ။
  ပရိုဖိုင်အသစ်များစာရင်းမသွင်းမီ။ အလိုအလျောက်စနစ်သည် Canonical လက်ကိုင်များကိုပိုမိုနှစ်သက်သင့်သည်။
  ဂဏန်း ID များထက် (`namespace.name@semver`)။
- **CI အလိုအလျောက်စနစ်** – ပိုက်လိုင်းများထုတ်လွှတ်ရန် အထက်ဖော်ပြပါ command များကို ပေါင်းထည့်၍ docs၊
  တန်ဆာပလာများ၊ နှင့် ရှေးဟောင်းပစ္စည်းများ လက်မှတ်ရေးထိုးပြီး အဆုံးအဖြတ်ပေးသည့် သရုပ်ဖော်မှုများကို ထုတ်ဝေသည်။
  မက်တာဒေတာ။