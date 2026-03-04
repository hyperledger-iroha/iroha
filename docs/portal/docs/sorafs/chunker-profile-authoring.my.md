---
lang: my
direction: ltr
source: docs/portal/docs/sorafs/chunker-profile-authoring.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 855dd4bff7bf9581f485ac6ad7fa332f17595efb41127b74795c4b3a5a955406
source_last_modified: "2026-01-05T09:28:11.856260+00:00"
translation_last_reviewed: 2026-02-07
id: chunker-profile-authoring
title: SoraFS Chunker Profile Authoring Guide
sidebar_label: Chunker Authoring Guide
description: Checklist for proposing new SoraFS chunker profiles and fixtures.
translator: machine-google-reviewed
---

::: Canonical Source ကို သတိပြုပါ။
:::

#SoraFS Chunker Profile Authoring Guide

ဤလမ်းညွှန်တွင် SoraFS အတွက် chunker ပရိုဖိုင်အသစ်များကို အဆိုပြုခြင်းနှင့် ထုတ်ဝေနည်းကို ရှင်းပြထားသည်။
၎င်းသည် Architecture RFC (SF-1) နှင့် registry ရည်ညွှန်းချက် (SF-2a) ကို ဖြည့်စွက်ပေးသည်
ခိုင်မာသောရေးသားမှုလိုအပ်ချက်များ၊ အတည်ပြုခြင်းအဆင့်များနှင့် အဆိုပြုချက်ပုံစံများ။
Canonical ဥပမာတစ်ခုအတွက်၊ ကြည့်ပါ။
`docs/source/sorafs/proposals/sorafs_sf1_profile_v1.json`
နှင့် ပါ၀င်သော အခြောက်လှန်းသော လော့ဂ်အင်
`docs/source/sorafs/reports/sf1_determinism.md`။

## ခြုံငုံသုံးသပ်ချက်

မှတ်ပုံတင်ခြင်းသို့ဝင်သော ပရိုဖိုင်တိုင်းသည်-

- အဆုံးအဖြတ်ပေးသော CDC ကန့်သတ်ဘောင်များနှင့် multihash ဆက်တင်များတစ်လျှောက် ထပ်တူထပ်မျှ ကြော်ငြာပါ။
  ဗိသုကာလက်ရာများ
- သင်္ဘောပြန်ကစားနိုင်သော ပစ္စည်းများ (Rust/Go/TS JSON + fuzz corpora + PoR သက်သေများ)
  downstream SDKs များသည် စိတ်ကြိုက် tooling မပါဘဲ အတည်ပြုနိုင်သည်။
- အုပ်ချုပ်မှု-အဆင်သင့် မက်တာဒေတာ (namespace၊ name၊ semver) နှင့် ရွှေ့ပြောင်းခြင်းများ ပါဝင်ပါသည်။
- ကောင်စီမသုံးသပ်မီ အဆုံးအဖြတ်ပေးသည့် ကွဲပြားမှုအစုကို ကျော်ဖြတ်ပါ။

ထိုစည်းမျဉ်းများကို ကျေနပ်စေမည့် အဆိုပြုချက်ကို ပြင်ဆင်ရန် အောက်ပါစာရင်းကို လိုက်နာပါ။

## Registry Charter Snapshot

အဆိုပြုချက်တစ်ခုမရေးဆွဲမီ၊ ၎င်းသည် ပြဋ္ဌာန်းထားသည့် မှတ်ပုံတင်စာချုပ်စာတမ်းနှင့် ကိုက်ညီကြောင်း အတည်ပြုပါ။
`sorafs_manifest::chunker_registry::ensure_charter_compliance()` အားဖြင့်-

- ပရိုဖိုင် ID များသည် ကွာဟချက်မရှိဘဲ တစ်ပုံတစ်ပုံ တိုးလာနိုင်သော အပြုသဘောဆောင်သော ကိန်းပြည့်များဖြစ်သည်။
- Canonical လက်ကိုင် (`namespace.name@semver`) သည် နာမည်တူစာရင်းတွင် ပေါ်လာရမည်
  နှင့် **** သည် ပထမဆုံးဝင်ရောက်မှုဖြစ်ရမည်။
- အခြား Canonical လက်ကိုင်နှင့် တိုက်မိနိုင်သည် သို့မဟုတ် တစ်ကြိမ်ထက်ပို၍ ပေါ်လာနိုင်မည်မဟုတ်ပေ။
- နာမည်ဝှက်များသည် အလွတ်မဟုတ်ဘဲ နေရာလွတ်များကို ဖြတ်တောက်ထားရပါမည်။

အသုံးဝင်သော CLI အထောက်အကူများ-

```bash
# JSON listing of all registered descriptors (ids, handles, aliases, multihash)
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles

# Emit metadata for a candidate default profile (canonical handle + aliases)
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
  --promote-profile=sorafs.sf1@1.0.0 --json-out=-
```

ဤအမိန့်များသည် အဆိုပြုချက်များကို မှတ်ပုံတင်ခြင်းဆိုင်ရာ ပဋိညာဉ်စာတမ်းနှင့် လိုက်လျောညီထွေဖြစ်စေပြီး ပေးဆောင်သည်။
အုပ်ချုပ်မှုဆိုင်ရာ ဆွေးနွေးမှုများတွင် လိုအပ်သော canonical metadata။

## လိုအပ်သော မက်တာဒေတာ

| လယ် | ဖော်ပြချက် | ဥပမာ (`sorafs.sf1@1.0.0`) |
|--------|----------------|--------------------------------|
| `namespace` | ဆက်စပ်ပရိုဖိုင်များအတွက် ယုတ္တိနည်းဖြင့် အုပ်စုဖွဲ့ခြင်း။ | `sorafs` |
| `name` | လူသားဖတ်နိုင်သော အညွှန်း။ | `sf1` |
| `semver` | ကန့်သတ်ချက်များအတွက် အဓိပ္ပါယ်သတ်မှတ်ချက် ဗားရှင်းစာကြောင်း။ | `1.0.0` |
| `profile_id` | ပရိုဖိုင်ကို ရောက်သည်နှင့် တစ်ပြိုင်နက် သတ်မှတ်ပေးသော Monotonic ဂဏန်းအမှတ်အသား။ နောက် id ကို သိမ်းဆည်းထားသော်လည်း ရှိပြီးသားနံပါတ်များကို ပြန်အသုံးမပြုပါနှင့်။ | `1` |
| `profile_aliases` | ညှိနှိုင်းမှုအတွင်း ဖောက်သည်များနှင့် ထိတွေ့နိုင်သော ရွေးချယ်နိုင်သော နောက်ထပ်လက်ကိုင်များ။ ပထမဆုံးဝင်ရောက်မှုအဖြစ် Canonical လက်ကိုင်ကို အမြဲတမ်း ထည့်သွင်းပါ။ | `["sorafs.sf1@1.0.0"]` |
| `profile.min_size` | အနိမ့်ဆုံး အတုံးအရှည်ကို bytes | `65536` |
| `profile.target_size` | ပစ်မှတ်အတုံးအရှည်ကို bytes | `262144` |
| `profile.max_size` | အများဆုံးအတုံးအရှည်ကို bytes။ | `524288` |
| `profile.break_mask` | rolling hash (hex) မှအသုံးပြုသောအလိုက်သင့်မျက်နှာဖုံး။ | `0x0000ffff` |
| `profile.polynomial` | Gear polynomial constant (hex)။ | `0x3da3358b4dc173` |
| `gear_seed` | မျိုးစေ့သည် 64KiB ဂီယာဇယားကိုရရှိရန်အသုံးပြုသည်။ | `sorafs-v1-gear` |
| `chunk_multihash.code` | အပိုင်းတစ်ခုစီအတွက် အစီအစဥ်များအတွက် Multihash ကုဒ်။ | `0x1f` (BLAKE3-256) |
| `chunk_multihash.digest` | canonical fixtures အစုအဝေး၏အကျဉ်းချုပ်။ | `13fa...c482` |
| `fixtures_root` | ပြန်လည်ထုတ်လုပ်ထားသော ဆက်စပ်ပစ္စည်းများပါရှိသော ဆက်စပ်လမ်းညွှန်။ | `fixtures/sorafs_chunker/sorafs.sf1@1.0.0/` |
| `por_seed` | သတ်မှတ်ထားသော PoR နမူနာကောက်ယူခြင်းအတွက် မျိုးစေ့ (`splitmix64`)။ | `0xfeedbeefcafebabe` (ဥပမာ) |

မက်တာဒေတာသည် အဆိုပြုချက်မှတ်တမ်းတွင်ရော ထုတ်ပေးထားသည့်အတွင်း၌ရော ပေါ်လာရပါမည်။
fixtures များသည် registry၊ CLI tooling နှင့် governance automation တို့ကို အတည်ပြုနိုင်သည်။
လက်ဖြင့် အပြန်အလှန် ကိုးကားခြင်းမရှိဘဲ တန်ဖိုးများ။ သံသယရှိလျှင် အတုံးလိုက်အခဲလိုက် စတိုးဆိုင်နှင့် လည်ပတ်ပါ။
တွက်ချက်ထားသော မက်တာဒေတာကို ပြန်လည်သုံးသပ်ရန် `--json-out=-` ဖြင့် CLI များကို ထင်ရှားစွာပြသပါ
မှတ်စုများ

### CLI & Registry Touchpoints

- `sorafs_manifest_chunk_store --profile=<handle>` - အပိုင်းအစ မက်တာဒေတာကို ပြန်လည်လုပ်ဆောင်ခြင်း၊
  manifest digest၊ PoR သည် အဆိုပြုထားသော ကန့်သတ်ချက်များဖြင့် စစ်ဆေးသည်။
- `sorafs_manifest_chunk_store --json-out=-` – အတုံးလိုက်စတိုးဆိုင်အစီရင်ခံစာကို တိုက်ရိုက်ထုတ်လွှင့်ပါ။
  အလိုအလျောက်နှိုင်းယှဉ်မှုများအတွက် stdout ။
- `sorafs_manifest_stub --chunker-profile=<handle>` - manifests များနှင့် CAR ကို အတည်ပြုပါ။
  အစီအစဥ်များသည် canonical handle နှင့် aliases တို့ကို ထည့်သွင်းထားသည်။
- `sorafs_manifest_stub --plan=-` - ယခင် `chunk_fetch_specs` ကို ပြန်ကျွေးပါ
  ပြောင်းလဲမှုပြီးနောက် အော့ဖ်ဆက်များ/အချေအတင်များကို အတည်ပြုရန်။

အဆိုပြုချက်တွင် command output (digests၊ PoR roots၊ manifest hash) ကို မှတ်တမ်းတင်ပါ
ထို့ကြောင့် ဝေဖန်သုံးသပ်သူများသည် ၎င်းတို့ကို စကားအပြောအဆို ပြန်ထုတ်နိုင်သည်။

## Determinism & Validation စစ်ဆေးမှုစာရင်း

1. ** တန်ဆာပလာများကို ပြန်ထုတ်ပါ **
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors \
     --signature-out=fixtures/sorafs_chunker/manifest_signatures.json
   ```
2. ** parity suite ကို run ** – `cargo test -p sorafs_chunker` နှင့်
   ဘာသာစကား ကွဲပြားသည့်ကြိုး (`crates/sorafs_chunker/tests/vectors.rs`) ဖြစ်ရမည်။
   စိမ်းလန်းစိုပြေသော ပရိဘောဂအသစ်များဖြင့်။
3. ** fuzz/back-pressure corpora ကို ပြန်ဖွင့်ပါ** – `cargo fuzz list` နှင့် execute
   ပြန်လည်ထုတ်လုပ်ထားသောပိုင်ဆိုင်မှုများနှင့်ဆန့်ကျင်ဘက် streaming harness (`fuzz/sorafs_chunker`)။
4. **ပြန်လည်ရယူနိုင်သည့် သက်သေများကို စိစစ်ပါ** – လုပ်ဆောင်ပါ။
   `sorafs_manifest_chunk_store --por-sample=<n>` အဆိုပြုထားသော ပရိုဖိုင်နှင့် အသုံးပြု
   အမြစ်များသည် fixture manifest နှင့်ကိုက်ညီကြောင်းအတည်ပြုပါ။
5. **CI dry run** – ပြည်တွင်းတွင် `ci/check_sorafs_fixtures.sh` ကို ခေါ်ဆိုပါ။ ဇာတ်ညွှန်း
   အသစ်များနှင့် ရှိပြီးသား `manifest_signatures.json` တို့ဖြင့် အောင်မြင်သင့်သည်။
6. **Cross-runtime အတည်ပြုခြင်း** – Go/TS bindings များသည် ပြန်လည်ထုတ်ပေးထားသော အရာများကို စားသုံးကြောင်း သေချာပါစေ။
   JSON နှင့် ထပ်တူထပ်မျှ အတုံးအခဲများကို ဘောင်များနှင့် အချေအတင်များ ထုတ်လွှတ်သည်။

Tooling WG သည် အဆိုပြုချက်တွင် ညွှန်ကြားချက်များနှင့် ရလဒ်များကို မှတ်တမ်းတင်ပါ။
မှန်းဆစရာမလိုဘဲ ၎င်းတို့ကို ပြန်လည်လည်ပတ်နိုင်သည်။

### Manifest/PoR အတည်ပြုချက်

ဆက်စပ်ပစ္စည်းများကို ပြန်လည်ထုတ်ပေးပြီးနောက် CAR ကိုသေချာစေရန်အတွက် အပြည့်အဝဖော်ပြသည့်ပိုက်လိုင်းကို ဖွင့်ပါ။
မက်တာဒေတာနှင့် PoR အထောက်အထားများသည် တသမတ်တည်းရှိနေသည်-

```bash
# Validate chunk metadata + PoR with the new profile
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
  --profile=sorafs.sf2@1.0.0 \
  --json-out=- --por-json-out=- fixtures/sorafs_chunker/input.bin

# Generate manifest + CAR and capture chunk fetch specs
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
  fixtures/sorafs_chunker/input.bin \
  --chunker-profile=sorafs.sf2@1.0.0 \
  --chunk-fetch-plan-out=chunk_plan.json \
  --manifest-out=sf2.manifest \
  --car-out=sf2.car \
  --json-out=sf2.report.json

# Re-run using the saved fetch plan (guards against stale offsets)
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
  fixtures/sorafs_chunker/input.bin \
  --chunker-profile=sorafs.sf2@1.0.0 \
  --plan=chunk_plan.json --json-out=-
```

ထည့်သွင်းသည့်ဖိုင်ကို သင်၏ တပ်ဆင်အသုံးပြုသည့် ကိုယ်စားလှယ် ကော်ပိုရိတ်နှင့် အစားထိုးပါ။
(ဥပမာ၊ 1GiB အဆုံးအဖြတ်ပေးသောစီးကြောင်း) နှင့် ရရှိလာသော ရလဒ်များကို ပူးတွဲပါ
အဆိုပြုချက်။

## အဆိုပြုလွှာပုံစံ

အဆိုပြုချက်များကို `ChunkerProfileProposalV1` Norito မှတ်တမ်းများအဖြစ် စစ်ဆေးပြီး တင်သွင်းသည်
`docs/source/sorafs/proposals/`။ အောက်ပါ JSON နမူနာပုံစံသည် မျှော်လင့်ထားသည့်အရာများကို သရုပ်ဖော်သည်။
ပုံသဏ္ဍာန် (လိုအပ်သလို သင့်တန်ဖိုးများကို အစားထိုးပါ)


ဖမ်းယူထားသည့် ကိုက်ညီသော Markdown အစီရင်ခံစာ (`determinism_report`) ကို ပေးပါ။
command output၊ chunk digests နှင့် validation အတွင်း ကြုံတွေ့ရသော သွေဖည်မှုများ။

## အုပ်ချုပ်မှုလုပ်ငန်းအသွားအလာ

1. ** PR ကို အဆိုပြုချက် + ပြင်ဆင်မှုများဖြင့် တင်သွင်းပါ။** ထုတ်ပေးထားသော ပိုင်ဆိုင်မှုများ ပါ၀င်သည် ၊
   Norito အဆိုပြုချက်နှင့် `chunker_registry_data.rs` သို့ အပ်ဒိတ်များ။
2. **Tooling WG ပြန်လည်သုံးသပ်ခြင်း။** သုံးသပ်သူများသည် တရားဝင်စစ်ဆေးစာရင်းကို ပြန်လည်လုပ်ဆောင်ပြီး အတည်ပြုပါ။
   အဆိုပြုချက်သည် မှတ်ပုံတင်ခြင်းဆိုင်ရာ စည်းမျဉ်းများနှင့် ကိုက်ညီသည် (id ပြန်လည်အသုံးပြုခြင်းမရှိပါ၊ အဆုံးအဖြတ်ပေးခြင်းကို ကျေနပ်သည်)။
၃။ **ကောင်စီစာအိတ်။** အတည်ပြုပြီးသည်နှင့် ကောင်စီဝင်များသည် အဆိုပြုချက်စာရင်းကို လက်မှတ်ရေးထိုးခြင်း၊
   (`blake3("sorafs-chunker-profile-v1" || canonical_bytes)`) နှင့် ၎င်းတို့၏ နောက်ဆက်တွဲ
   ပစ္စည်းများနှင့်အတူ သိမ်းဆည်းထားသော ပရိုဖိုင်စာအိတ်အတွက် လက်မှတ်များ။
4. **Registry ထုတ်ဝေခြင်း။** ပေါင်းစည်းခြင်းသည် မှတ်ပုံတင်ခြင်း၊ စာရွက်စာတမ်းများနှင့် ဆက်စပ်ပစ္စည်းများကို ထိခိုက်စေပါသည်။ ဟိ
   မူရင်း CLI သည် ယခင်ပရိုဖိုင်တွင် ဆက်လက်တည်ရှိနေပါသည်။
   ရွှေ့ပြောင်းခြင်း အဆင်သင့်ဖြစ်နေပါပြီ။
5. **Deprecation ခြေရာခံခြင်း** ပြောင်းရွှေ့ခြင်းဝင်းဒိုးပြီးနောက်၊ မှတ်ပုံတင်ခြင်းအား အပ်ဒိတ်လုပ်ပါ။
   လယ်ဂျာ။

## ရေးသားခြင်းဆိုင်ရာ အကြံပြုချက်များ

- edge-case chunking အပြုအမူကို လျှော့ချရန် power-of-2 ဘောင်များကိုပင် ဦးစားပေးပါ။
- manifest နှင့် gateway ကိုညှိနှိုင်းခြင်းမရှိဘဲ multihash ကုဒ်ကိုပြောင်းလဲခြင်းမှရှောင်ကြဉ်ပါ။
- စာရင်းစစ်ကို ရိုးရှင်းစေရန်အတွက် ဂီယာစားပွဲအစေ့များကို လူသားများဖတ်ရလွယ်ကူစေသော်လည်း ကမ္ဘာတစ်ဝှမ်းတွင် ထူးခြားစွာ ထားရှိပါ။
  လမ်းများ။
- စံသတ်မှတ်ထားသော အနုပညာပစ္စည်းများ (ဥပမာ၊ ဖြတ်သန်းမှု နှိုင်းယှဉ်မှုများ) အောက်တွင် သိမ်းဆည်းပါ။
  အနာဂတ်ရည်ညွှန်းမှုအတွက် `docs/source/sorafs/reports/`။

စတင်ရောင်းချစဉ်အတွင်း လုပ်ငန်းလည်ပတ်မှုမျှော်လင့်ချက်များအတွက် ရွှေ့ပြောင်းခြင်းစာရင်းဇယားကို ကိုးကားပါ။
(`docs/source/sorafs/migration_ledger.md`)။ runtime စည်းမျဉ်းများအတွက် ကြည့်ပါ။
`docs/source/sorafs/chunker_conformance.md`။