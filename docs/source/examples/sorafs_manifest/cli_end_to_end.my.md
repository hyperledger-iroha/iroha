---
lang: my
direction: ltr
source: docs/source/examples/sorafs_manifest/cli_end_to_end.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a8209e602132efb6c29962bf09aea8cd74f972fa956ea8a7a1dbac08a7f6f00f
source_last_modified: "2026-01-05T09:28:12.006380+00:00"
translation_last_reviewed: 2026-02-07
title: "SoraFS Manifest CLI End-to-End Example"
translator: machine-google-reviewed
---

#SoraFS Manifest CLI End-to-End နမူနာ

ဤနမူနာသည် SoraFS ကိုအသုံးပြု၍ စာရွက်စာတမ်းတည်ဆောက်မှုကို ထုတ်ဝေခြင်းမှတဆင့် ဖြတ်သန်းပါသည်။
`sorafs_manifest_stub` CLI သည် အဆုံးအဖြတ်ပေးသော chunking fixtures များနှင့်အတူ
SoraFS Architecture RFC တွင်ဖော်ပြထားသည်။ စီးဆင်းမှုသည် ထင်ရှားသော မျိုးဆက်ကို ဖုံးလွှမ်းသည်၊
မျှော်မှန်းချက်စစ်ဆေးမှုများ၊ ထုတ်ယူမှုအစီအစဉ်ကို မှန်ကန်ကြောင်းနှင့် ပြန်လည်ရယူခြင်းဆိုင်ရာ အထောက်အထားများ ပြန်လည်အစမ်းလေ့ကျင့်မှုများ၊
အဖွဲ့များသည် CI တွင် တူညီသောအဆင့်များကို ထည့်သွင်းနိုင်သည်။

## လိုအပ်ချက်များ

- အလုပ်ခွင်ကိုပွားပြီး ကိရိယာကွင်းဆက်အဆင်သင့် (`cargo`၊ `rustc`)။
- `fixtures/sorafs_chunker` မှ တန်ဆာပလာများ ရရှိနိုင်သောကြောင့် မျှော်မှန်းတန်ဖိုးများ ရှိနိုင်ပါသည်။
  ဆင်းသက်လာသည် (ထုတ်လုပ်မှုလုပ်ငန်းအတွက်၊ ပြောင်းရွှေ့မှုစာရင်းဇယားထည့်သွင်းမှုမှ တန်ဖိုးများကို ဆွဲထုတ်ပါ။
  ရှေးဟောင်းပစ္စည်းနှင့် ဆက်စပ်နေပါသည်။)
- ထုတ်ဝေရန် နမူနာ payload directory (ဤဥပမာသည် `docs/book` ကိုအသုံးပြုသည်)။

## အဆင့် 1 — မန်နီးဖက်စ်၊ CAR၊ လက်မှတ်များနှင့် ထုတ်ယူမှုအစီအစဉ်ကို ဖန်တီးပါ။

```bash
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- docs/book \
  --manifest-out target/sorafs/docs.manifest \
  --manifest-signatures-out target/sorafs/docs.manifest_signatures.json \
  --car-out target/sorafs/docs.car \
  --chunk-fetch-plan-out target/sorafs/docs.fetch_plan.json \
  --car-digest=13fa919c67e55a2e95a13ff8b0c6b40b2e51d6ef505568990f3bc7754e6cc482 \
  --car-size=429391872 \
  --root-cid=f40101d0cfa9be459f4a4ba4da51990b75aef262ef546270db0e42d37728755d \
  --dag-codec=0x71 \
  --chunker-profile=sorafs.sf1@1.0.0
```

အမိန့်တော်-

- `ChunkProfile::DEFAULT` မှတဆင့် payload ကို တိုက်ရိုက်ကြည့်ရှုပါ။
- CARv2 မှတ်တမ်းတစ်ခုနှင့် အတုံးအခဲလိုက်ယူခြင်းအစီအစဉ်ကို ထုတ်လွှတ်သည်။
- `ManifestV1` မှတ်တမ်းကိုတည်ဆောက်ပါ၊ ထင်ရှားသောလက်မှတ်များကိုစစ်ဆေးပါ (ပေးလျှင်) နှင့်
  စာအိတ်ကိုရေးတယ်။
- bytes မျှော့နေပါက အပြေးပျက်သွားစေရန် မျှော်လင့်ချက်အလံများကို တွန်းအားပေးသည်။

## အဆင့် 2 — အတုံးလိုက်စတိုးဆိုင် + PoR အစမ်းလေ့ကျင့်မှုဖြင့် ရလဒ်များကို စစ်ဆေးပါ။

```bash
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
  target/sorafs/docs.car \
  --manifest target/sorafs/docs.manifest \
  --report-out target/sorafs/docs.manifest_report.json \
  --por-json-out target/sorafs/docs.por.json
```

၎င်းသည် အဆုံးအဖြတ်အတုံးအခဲများစတိုးမှတစ်ဆင့် CAR ကို ပြန်လည်ပြသပြီး ၎င်းမှဆင်းသက်လာသည်။
အထောက်အထား-ပြန်လည်ထုတ်ယူနိုင်မှုနမူနာသစ်ပင်နှင့် သင့်လျော်သော ထင်ရှားသောအစီရင်ခံစာကို ထုတ်လွှတ်သည်။
အုပ်ချုပ်မှုပြန်လည်သုံးသပ်ခြင်း။

## အဆင့် 3 — Multi-provider retrieve ကို အတုယူပါ။

```bash
cargo run -p sorafs_car --bin sorafs_fetch -- \
  --plan=target/sorafs/docs.fetch_plan.json \
  --provider=primary=target/sorafs/docs.car \
  --chunk-receipts-out=target/sorafs/docs.chunk_receipts.json \
  --json-out=target/sorafs/docs.fetch_report.json
```

CI ပတ်၀န်းကျင်အတွက်၊ ပံ့ပိုးသူ တစ်ဦးစီအတွက် သီးခြား payload လမ်းကြောင်းများ ပေးဆောင်ပါ (ဥပမာ၊ တပ်ဆင်ထားသည်။
fixtures) လေ့ကျင့်ခန်းအကွာအဝေး အချိန်ဇယားဆွဲခြင်းနှင့် မအောင်မြင်ခြင်းများကို ကိုင်တွယ်ရန်။

## အဆင့် 4 — လယ်ဂျာထည့်သွင်းမှုကို မှတ်တမ်းတင်ပါ။

ထုတ်ဝေမှုကို `docs/source/sorafs/migration_ledger.md` တွင် မှတ်တမ်းတင်၍ ရိုက်ကူးခြင်း၊

- CID၊ CAR digest နှင့် council signature hash ကိုဖော်ပြပါ။
- အခြေအနေ (`Draft`, `Staging`, `Pinned`)။
- CI လည်ပတ်မှုများ သို့မဟုတ် အုပ်ချုပ်မှုလက်မှတ်များသို့ လင့်ခ်များ။

## အဆင့် 5 — အုပ်ချုပ်မှုကိရိယာများမှတစ်ဆင့် ပင်ထိုးပါ (မှတ်ပုံတင်ခြင်းစတင်နေချိန်တွင်)

Pin Registry ကို အသုံးပြုပြီးသည်နှင့် (ရွှေ့ပြောင်းခြင်းဆိုင်ရာ လမ်းပြမြေပုံတွင် Milestone M2)၊
မန်နီးဖက်စ်ကို CLI မှတဆင့် တင်ပြပါ-

```bash
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- docs/book \
  --plan=target/sorafs/docs.fetch_plan.json \
  --manifest-out target/sorafs/docs.manifest \
  --manifest-signatures-in target/sorafs/docs.manifest_signatures.json \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --council-signature-file <signer_hex>:path/to/signature.bin

cargo run -p sorafs_cli --bin sorafs_pin -- propose \
  --manifest target/sorafs/docs.manifest \
  --manifest-signatures target/sorafs/docs.manifest_signatures.json
```

အဆိုပြုချက်သတ်မှတ်သူနှင့် နောက်ဆက်တွဲအတည်ပြုချက်ပေးငွေလွှဲခြင်း hashe ဖြစ်သင့်သည်။
စာရင်းစစ်ခြင်းအတွက် ရွှေ့ပြောင်းခြင်းဆိုင်ရာ စာရင်းဇယားတွင် ဖမ်းယူထားသည်။

## သန့်ရှင်းရေး

`target/sorafs/` အောက်တွင်ရှိသော ပစ္စည်းများအား မော်ကွန်းတင်နိုင်သည် သို့မဟုတ် အဆင့်တင်သည့်နေရာများတွင် အပ်လုဒ်လုပ်နိုင်ပါသည်။
မန်နီးဖက်စ်၊ လက်မှတ်များ၊ CAR နှင့် ထုတ်ယူမှုအစီအစဉ်ကို ရေအောက်ပိုင်းတွင် စုစည်းထားပါ။
အော်ပရေတာများနှင့် SDK အဖွဲ့များသည် ဖြန့်ကျက်မှုကို တိကျစွာ အတည်ပြုနိုင်သည်။