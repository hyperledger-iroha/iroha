---
lang: my
direction: ltr
source: docs/portal/docs/sorafs/manifest-pipeline.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

#SoraFS Chunking → Manifest ပိုက်လိုင်း

အမြန်စတင်ရန် ဤအဖော်သည် ကုန်ကြမ်းပြောင်းသွားသည့် အဆုံးမှအဆုံး ပိုက်လိုင်းကို ခြေရာခံသည်။
Norito သို့ bytes သည် SoraFS Pin Registry အတွက် သင့်လျော်သည်။ အကြောင်းအရာက
[`docs/source/sorafs/manifest_pipeline.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/manifest_pipeline.md);
canonical specification နှင့် changelog အတွက် ထိုစာရွက်စာတမ်းကို တိုင်ပင်ပါ။

## 1. အတုံးအခဲ

SoraFS သည် SF-1 (`sorafs.sf1@1.0.0`) ပရိုဖိုင်ကို အသုံးပြုသည်- FastCDC မှုတ်သွင်းထားသော လှည့်ခြင်း
64KiB အနိမ့်ဆုံးအပိုင်းအရွယ်အစား၊ 256KiB ပစ်မှတ်၊ 512KiB အမြင့်ဆုံးနှင့်
`0x0000ffff` ကွဲမျက်နှာဖုံး။ ပရိုဖိုင်တွင် စာရင်းသွင်းထားသည်။
`sorafs_manifest::chunker_registry`။

### သံချေးတက်သူများ

- `sorafs_car::CarBuildPlan::single_file` - အပိုင်းလိုက် အော့ဖ်ဆက်များ၊ အလျားများနှင့် တို့ကို ထုတ်လွှတ်သည်။
  BLAKE3 သည် CAR မက်တာဒေတာကို ပြင်ဆင်နေချိန်တွင် ချေဖျက်သည်။
- `sorafs_car::ChunkStore` - တိုက်ရိုက်လွှင့်တင်မှုများ၊ အတုံးလိုက် မက်တာဒေတာကို ဆက်ရှိနေစေကာ၊
  64KiB/4KiB Proof-of-Retrievability (PoR) နမူနာသစ်ပင်မှ ဆင်းသက်လာသည်။
- `sorafs_chunker::chunk_bytes_with_digests` - CLI နှစ်ခုလုံး၏ နောက်ကွယ်ရှိ စာကြည့်တိုက်အကူ။

### CLI ကိရိယာတန်ဆာပလာ

```bash
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- ./payload.bin \
  > chunk-plan.json
```

JSON တွင် မှာယူထားသော အော့ဖ်ဆက်များ၊ အလျားများနှင့် အတုံးအခဲများ ပါဝင်ပါသည်။ ဆက်နေပါ။
သရုပ်ပြများ သို့မဟုတ် တီးမှုတ်သူအား ခေါ်ယူမှု သတ်မှတ်ချက်များကို တည်ဆောက်သည့်အခါ အစီအစဉ်ဆွဲပါ။

### PoR သက်သေများ

`ChunkStore` သည် `--por-proof=<chunk>:<segment>:<leaf>` နှင့်
`--por-sample=<count>` ထို့ကြောင့် စာရင်းစစ်များသည် အဆုံးအဖြတ်ပေးသော သက်သေအစုံများကို တောင်းဆိုနိုင်သည်။ တွဲ
JSON ကိုမှတ်တမ်းတင်ရန် `--por-proof-out` သို့မဟုတ် `--por-sample-out` ပါသော အဆိုပါအလံများ။

## 2. သရုပ်ဖော်ချက်ကို ခြုံပါ။

`ManifestBuilder` သည် အပိုင်းလိုက် မက်တာဒေတာကို အုပ်ချုပ်မှု ပူးတွဲပါဖိုင်များနှင့် ပေါင်းစပ်သည်-

- Root CID (dag-cbor) နှင့် CAR ကတိကဝတ်များ။
- Alias ​​အထောက်အထားများနှင့် ပံ့ပိုးပေးနိုင်စွမ်း တောင်းဆိုချက်များ။
- ကောင်စီလက်မှတ်များနှင့် ရွေးချယ်နိုင်သော မက်တာဒေတာများ (ဥပမာ၊ တည်ဆောက် ID များ)။

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub -- \
  ./payload.bin \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --manifest-out=payload.manifest \
  --manifest-signatures-out=payload.manifest_signatures.json \
  --json-out=payload.report.json
```

အရေးကြီးသော ရလဒ်များ-

- `payload.manifest` – Norito-ကုဒ်လုပ်ထားသော မန်နီးဖက်စ်ဘိုက်များ။
- `payload.report.json` – လူသား/အလိုအလျောက် ဖတ်နိုင်သော အနှစ်ချုပ် အပါအဝင်
  `chunk_fetch_specs`၊ `payload_digest_hex`၊ CAR digests နှင့် alias မက်တာဒေတာ။
- `payload.manifest_signatures.json` - မန်နီးဖက်စ် BLAKE3 ပါဝင်သော စာအိတ်
  digest၊ အတုံးအခဲ-အစီအစဥ် SHA3 digest နှင့် Ed25519 လက်မှတ်များကို စီထားသည်။

ပြင်ပမှ ပံ့ပိုးပေးသော စာအိတ်များကို စစ်ဆေးရန် `--manifest-signatures-in` ကို အသုံးပြုပါ။
လက်မှတ်မထိုးမီ ၎င်းတို့ကို ပြန်ရေးပြီး `--chunker-profile-id` သို့မဟုတ်
မှတ်ပုံတင်ခြင်းရွေးချယ်မှုကို လော့ခ်ချရန် `--chunker-profile=<handle>`။

## 3. ထုတ်ဝေပြီး ပင်ထိုးပါ။

1. **အုပ်ချုပ်မှုတင်ပြခြင်း** – ထင်ရှားသော အနှစ်သာရနှင့် လက်မှတ်ကို ပေးဆောင်ပါ။
   ပင်ကိုလက်ခံနိုင်စေရန် ကောင်စီသို့ စာအိတ်။ ပြင်ပစာရင်းစစ်တွေ လုပ်သင့်တယ်။
   အတုံးအခဲ-အစီအစဥ် SHA3 digest ကို manifest digest နှင့်အတူ သိမ်းဆည်းပါ။
2. **Pin payloads** – ကိုးကားထားသော CAR archive (နှင့် optional CAR အညွှန်း) ကို အပ်လုဒ်လုပ်ပါ။
   Pin Registry ၏ manifest တွင်။ မန်နီးဖက်စ်ကို သေချာစစ်ဆေးပြီး CAR မျှဝေပါ။
   Root CID က အတူတူပါပဲ။
3. ** တယ်လီမီတာ မှတ်တမ်းတင်ခြင်း** – JSON အစီရင်ခံစာ၊ PoR သက်သေများနှင့် ရယူမှုမှန်သမျှကို ဆက်လက်လုပ်ဆောင်ပါ။
   ထွက်လာသည့် ပစ္စည်းများတွင် တိုင်းတာမှုများ။ ဤမှတ်တမ်းများသည် အော်ပရေတာ ဒက်ရှ်ဘုတ်များနှင့် ကျွေးမွေးပါသည်။
   ကြီးမားသော payload များကို ဒေါင်းလုဒ်မလုပ်ဘဲ ပြဿနာများကို ပြန်လည်ဖန်တီးရန် ကူညီပေးပါ။

## 4. Multi-provider fetch simulation

`ကုန်တင်ကုန်ချ -p sorafs_car --bin sorafs_fetch -- --plan=payload.report.json \
  --provider=alpha=providers/alpha.bin --provider=beta=providers/beta.bin#4@3 \
  --output=payload.bin --json-out=fetch_report.json`

- `#<concurrency>` သည် ဝန်ဆောင်မှုပေးသူ တစ်ဦးချင်း ပြိုင်တူဝါဒကို တိုးစေသည် (အထက် `#4`)။
- `@<weight>` တီးလုံးများ အချိန်ဇယားဆွဲခြင်းဘက်လိုက်မှု။ 1 သို့ ပုံသေသတ်မှတ်ထားသည်။
- `--max-peers=<n>` သည် မည်သည့်အချိန်တွင် လုပ်ဆောင်ရန် စီစဉ်ထားသော ဝန်ဆောင်မှုပေးသူ အရေအတွက်ကို ဖုံးအုပ်ထားသည်။
  ရှာဖွေတွေ့ရှိမှုသည် ကိုယ်စားလှယ်လောင်းများကို အလိုရှိသည်ထက် ပိုများစေသည်။
- `--expect-payload-digest` နှင့် `--expect-payload-len` အသံတိတ်ခြင်းမှ ကာကွယ်ခြင်း
  အဂတိလိုက်စားမှု။
- `--provider-advert=name=advert.to` သည် ဝန်ဆောင်မှုပေးနိုင်စွမ်းများကို အရင်စစ်ဆေးပါသည်။
  ၎င်းတို့ကို simulation တွင်အသုံးပြုပါ။
- `--retry-budget=<n>` သည် တစ်ပိုင်းတစ်စ ပြန်စမ်းကြည့်ခြင်းအရေအတွက်ကို အစားထိုးသည် (မူလ- 3) ထို့ကြောင့် CI
  ရှုံးနိမ့်မှုအခြေအနေများကို စမ်းသပ်သောအခါတွင် ဆုတ်ယုတ်မှုများကို ပိုမိုမြန်ဆန်စွာပေါ်လွင်စေနိုင်သည်။

`fetch_report.json` မျက်နှာပြင်များ စုစည်းထားသော မက်ထရစ်များ (`chunk_retry_total`၊
`provider_failure_rate` စသည်ဖြင့်) CI အာမခံချက်များနှင့် စောင့်ကြည့်နိုင်စွမ်းအတွက် သင့်လျော်သည်။

## 5. Registry အပ်ဒိတ်များနှင့် အုပ်ချုပ်မှု

chunker ပရိုဖိုင်အသစ်များကို အဆိုပြုသည့်အခါ-

1. `sorafs_manifest::chunker_registry_data` တွင် ဖော်ပြချက်ကို ရေးသားပါ။
2. `docs/source/sorafs/chunker_registry.md` နှင့် သက်ဆိုင်ရာ charters များကို အပ်ဒိတ်လုပ်ပါ။
3. ပြင်ဆင်မှုများ (`export_vectors`) ကို ပြန်ထုတ်ပြီး လက်မှတ်ထိုးထားသော သရုပ်ဖော်မှုများကို ဖမ်းယူပါ။
4. အုပ်ချုပ်မှုဆိုင်ရာ လက်မှတ်များဖြင့် ပဋိညာဉ်စာတမ်းလိုက်နာမှု အစီရင်ခံစာကို တင်ပြပါ။

အလိုအလျောက်စနစ်သည် Canonical လက်ကိုင်များ (`namespace.name@semver`) နှင့် ကြွေကျခြင်းကို ပိုနှစ်သက်သင့်သည်။