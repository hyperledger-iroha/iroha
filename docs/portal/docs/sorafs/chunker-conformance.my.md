---
lang: my
direction: ltr
source: docs/portal/docs/sorafs/chunker-conformance.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d948fcd78a564487591aeba23d4587de337913984fd3a5861a83f2a9a23887d9
source_last_modified: "2026-01-05T09:28:11.855022+00:00"
translation_last_reviewed: 2026-02-07
id: chunker-conformance
title: SoraFS Chunker Conformance Guide
sidebar_label: Chunker Conformance
description: Requirements and workflows for preserving the deterministic SF1 chunker profile across fixtures and SDKs.
translator: machine-google-reviewed
---

::: Canonical Source ကို သတိပြုပါ။
:::

ဤလမ်းညွှန်ချက်သည် အကောင်အထည်ဖော်မှုတိုင်းတွင် ရှိနေရန် လိုက်နာရမည့် လိုအပ်ချက်များကို ပေါင်းစပ်ဖော်ပြထားသည်။
SoraFS အဆုံးအဖြတ်ပေးသော chunker ပရိုဖိုင် (SF1) နှင့် ချိန်ညှိထားသည်။ အဲဒါကိုလည်း
ပြန်လည်ထူထောင်ခြင်းလုပ်ငန်းအသွားအလာ၊ လက်မှတ်ရေးထိုးခြင်းမူဝါဒနှင့် အတည်ပြုခြင်းအဆင့်များကို မှတ်တမ်းတင်ထားသည်။
SDKs များတစ်လျှောက်တွင် တပ်ဆင်အသုံးပြုသူများသည် တစ်ပြိုင်တည်းရှိနေပါသည်။

## Canonical Profile

- ပရိုဖိုင်လက်ကိုင်- `sorafs.sf1@1.0.0`
- ထည့်သွင်းမျိုးစေ့ (hex): `0000000000dec0ded`
- ပစ်မှတ်အရွယ်အစား- 262144 bytes (256KiB)
- အနည်းဆုံးအရွယ်အစား- 65536 bytes (64KiB)
- အများဆုံးအရွယ်အစား- 524288 bytes (512KiB)
- Rolling polynomial- `0x3DA3358B4DC173`
- ဂီယာစားပွဲအစေ့: `sorafs-v1-gear`
- Break mask: `0x0000FFFF`

အကိုးအကား အကောင်အထည်ဖော်မှု- `sorafs_chunker::chunk_bytes_with_digests_profile`။
SIMD အရှိန်မြှင့်မှုတိုင်းသည် တူညီသော နယ်နိမိတ်များနှင့် အချေအတင်များကို ထုတ်ပေးရပါမည်။

## Fixture Bundle

`cargo run --locked -p sorafs_chunker --bin export_vectors` ကို ပြန်လည်ထုတ်ပေးသည်။
`fixtures/sorafs_chunker/` အောက်တွင် အောက်ပါဖိုင်များကို တပ်ဆင်ပြီး ထုတ်လွှတ်သည်-

- `sf1_profile_v1.{json,rs,ts,go}` — Rust အတွက် canonical chunk boundaries၊
  TypeScript နှင့် Go သုံးစွဲသူများ။ ဖိုင်တစ်ခုစီသည် canonical handle ကို the အဖြစ် ကြော်ငြာသည်။
  `profile_aliases` တွင် ပထမဆုံး (နှင့် အားလုံးအတွက်) ဝင်ခွင့်။ အမိန့်ချမှတ်သည်။
  `ensure_charter_compliance` နှင့် မပြောင်းလဲရပါ။
- `manifest_blake3.json` — BLAKE3-အတည်ပြုထားသော မန်နီးဖက်စ်သည် fixture ဖိုင်တိုင်းကို ဖုံးအုပ်ထားသည်။
- `manifest_signatures.json` — မန်နီးဖက်စ်တွင် ကောင်စီလက်မှတ်များ (Ed25519)
  ချေဖျက်သည်။
- `sf1_profile_v1_backpressure.json` နှင့် `fuzz/` အတွင်းရှိ ကုန်ကြမ်းကော်ပိုရာ —
  chunker back-pressure tests တွင်အသုံးပြုသော အဆုံးအဖြတ်ပေးသော တိုက်ရိုက်ထုတ်လွှင့်မှုအခြေအနေများ။

### လက်မှတ်ထိုးခြင်းမူဝါဒ

ဖန်သားပြင်ပြန်လည်ထုတ်လုပ်ခြင်း **ရပါမည်** အကျုံးဝင်သော ကောင်စီလက်မှတ်ပါရှိသည်။ မီးစက်
`--allow-unsigned` ကို ပြတ်သားစွာ မကျော်လွန်ပါက လက်မှတ်မထိုးထားသော အထွက်ကို ငြင်းပယ်သည် (ရည်ရွယ်သည်
ပြည်တွင်းစမ်းသပ်မှုများအတွက်သာ)။ လက်မှတ်စာအိတ်များသည် နောက်ဆက်တွဲ-သပ်သပ်ဖြစ်ပြီး၊
လက်မှတ်ထိုးသူ တစ်ဦးလျှင် ထပ်ပွားထားသည်။

ကောင်စီလက်မှတ်ထည့်ရန်-

```bash
cargo run --locked -p sorafs_chunker --bin export_vectors \
  --signing-key=<ed25519-private-key-hex> \
  --signature-out=fixtures/sorafs_chunker/manifest_signatures.json
```

## အတည်ပြုခြင်း။

CI အကူအညီပေးသူ `ci/check_sorafs_fixtures.sh` သည် မီးစက်ဖြင့် ပြန်ဖွင့်သည်။
`--locked`။ ပွဲများ လွင့်မျောခြင်း သို့မဟုတ် လက်မှတ်များ ပျောက်ဆုံးပါက အလုပ်ပျက်ပါသည်။ သုံးပါ။
ညစဉ် အလုပ်အသွားအလာများတွင် ဤ script နှင့် fixture အပြောင်းအလဲများကို မတင်ပြမီ။

လူကိုယ်တိုင် အတည်ပြုခြင်း အဆင့်များ-

1. `cargo test -p sorafs_chunker` ကိုဖွင့်ပါ။
2. `ci/check_sorafs_fixtures.sh` ကို စက်တွင်းတွင် ထည့်သွင်းပါ။
3. `git status -- fixtures/sorafs_chunker` သည် သန့်ရှင်းကြောင်း အတည်ပြုပါ။

## Playbook ကို အဆင့်မြှင့်ပါ။

chunker ပရိုဖိုင်အသစ်ကို အဆိုပြုသည့်အခါ သို့မဟုတ် SF1 ကို အပ်ဒိတ်လုပ်သည့်အခါ-

ကိုလည်းကြည့်ပါ- [`docs/source/sorafs/chunker_profile_authoring.md`](./chunker-profile-authoring.md) အတွက်
မက်တာဒေတာ လိုအပ်ချက်များ၊ အဆိုပြုချက် နမူနာပုံစံများနှင့် တရားဝင် စစ်ဆေးရေးစာရင်းများ။

1. ကန့်သတ်ချက်များအသစ်ဖြင့် `ChunkProfileUpgradeProposalV1` (RFC SF-1 ကိုကြည့်ပါ)။
2. `export_vectors` မှတစ်ဆင့် ပွဲစဉ်များကို ပြန်ထုတ်ပြီး မန်နီးဖက်စ် ချေဖျက်မှုအသစ်ကို မှတ်တမ်းတင်ပါ။
3. လိုအပ်သော ကောင်စီအထွတ်အထိပ်တွင် မန်နီးဖက်စ်ကို လက်မှတ်ရေးထိုးပါ။ လက်မှတ်အားလုံးရှိရမည်။
   `manifest_signatures.json` တွင် ထည့်သွင်းထားသည်။
4. သက်ရောက်မှုရှိသော SDK ပစ္စတင်များ (Rust/Go/TS) ကို အပ်ဒိတ်လုပ်ပြီး ဖြတ်ကျော်ပြေးချိန် တူညီမှုကို သေချာပါစေ။
5. ဘောင်များ ပြောင်းလဲပါက fuzz corpora ကို ပြန်ထုတ်ပါ။
6. ဤလမ်းညွှန်ချက်ကို ပရိုဖိုင်လက်ကိုင်အသစ်၊ အစေ့များ၊ နှင့် အစာချေမှုအသစ်ဖြင့် အပ်ဒိတ်လုပ်ပါ။
7. အပ်ဒိတ်လုပ်ထားသော စမ်းသပ်မှုများနှင့် လမ်းပြမြေပုံမွမ်းမံမှုများနှင့်အတူ အပြောင်းအလဲကို တင်ပြပါ။

ဤလုပ်ငန်းစဉ်ကို မလိုက်နာဘဲ အတုံးအတုံးနယ်နိမိတ်များ သို့မဟုတ် အချေအတင်များအပေါ် သက်ရောက်မှုရှိသော အပြောင်းအလဲများ
မှားယွင်းနေပြီး ပေါင်းစည်းခြင်းမပြုရပါ။