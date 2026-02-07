---
lang: my
direction: ltr
source: docs/portal/docs/sorafs/reports/sf1-determinism.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f7dd8b29e8eb37c2cd78c5dc91ce363bb546fa7e8768f8a2cc86f8b2d9508674
source_last_modified: "2026-01-04T08:19:26.498928+00:00"
translation_last_reviewed: 2026-02-07
title: SoraFS SF1 Determinism Dry-Run
summary: Checklist and expected digests for validating the canonical `sorafs.sf1@1.0.0` chunker profile.
translator: machine-google-reviewed
---

#SoraFS SF1 Determinism Dry-Run

ဤအစီရင်ခံစာသည် canonical အတွက် အခြေခံအခြောက်ခံခြင်းအား ဖမ်းယူထားသည်။
`sorafs.sf1@1.0.0` chunker နော်။ Tooling WG သည် checklist ကို ပြန်လည်လုပ်ဆောင်သင့်သည်။
တပ်ဆင်မှုအား ပြန်လည်စတင်ခြင်း သို့မဟုတ် စားသုံးသူ ပိုက်လိုင်းအသစ်များကို မှန်ကန်ကြောင်း အတည်ပြုသည့်အခါ အောက်တွင်။ မှတ်တမ်းတင်ပါ။
စာရင်းစစ်လမ်းကြောင်းကို ထိန်းသိမ်းရန် ဇယားရှိ command တစ်ခုစီ၏ ရလဒ်။

## စစ်ဆေးရန်စာရင်း

| အဆင့် | အမိန့် | မျှော်လင့်ထားသောရလဒ် | မှတ်စုများ |
|------|---------|------------------|--------|
| ၁ | `cargo test -p sorafs_chunker` | စမ်းသပ်မှုအားလုံးအောင်မြင်; `vectors` parity စမ်းသပ်မှု အောင်မြင်သည်။ | Canonical fixtures များကို compile လုပ်ပြီး Rust အကောင်အထည်ဖော်မှုကို အတည်ပြုသည်။ |
| 2 | `ci/check_sorafs_fixtures.sh` | Script သည် 0 ထွက်သည်; အောက်တွင် ဖော်ပြထားသော အစီရင်ခံချက်များ။ | ပစ္စည်းများကို သန့်ရှင်းသပ်ရပ်စွာ ပြန်လည်ထုတ်ပေးကြောင်း အတည်ပြုပြီး လက်မှတ်များ ပူးတွဲပါရှိမည်ဖြစ်သည်။ |
| 3 | `cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles` | `sorafs.sf1@1.0.0` သည် မှတ်ပုံတင်ခြင်းဖော်ပြချက် (`profile_id=1`) နှင့် ကိုက်ညီပါသည်။ | မှတ်ပုံတင်ခြင်း မက်တာဒေတာသည် တစ်ပြိုင်တည်းရှိနေကြောင်း သေချာစေပါသည်။ |
| 4 | `cargo run --locked -p sorafs_chunker --bin export_vectors` | `--allow-unsigned` မပါဘဲ ပြန်လည်ထုတ်လုပ်ခြင်းသည် အောင်မြင်သည်။ မန်နီးဖက်စ်နှင့် လက်မှတ်ဖိုင်များကို မပြောင်းလဲပါ။ | အတုံးအစည်း နယ်နိမိတ်များနှင့် သရုပ်ပြမှုများအတွက် အဆုံးအဖြတ်ပေးသည့် အထောက်အထား ပေးသည်။ |
| 5 | `node scripts/check_sf1_vectors.mjs` | TypeScript တပ်ဆင်မှုများနှင့် Rust JSON အကြား ကွာခြားမှုမရှိကြောင်း အစီရင်ခံပါသည်။ | ရွေးချယ်နိုင်သောအကူအညီ; runtime တစ်လျှောက် တူညီမှုရှိစေရန် (Tooling WG မှ ထိန်းသိမ်းထားသော script)။ |

## မျှော်လင့်ထားသော မှတ်တမ်းများ

- Chunk digest (SHA3-256): `13fa919c67e55a2e95a13ff8b0c6b40b2e51d6ef505568990f3bc7754e6cc482`
- `manifest_blake3.json`: `c8c45c025ecee39b5ac5bf3db3dc1e2f97a7eaf7ea0aac72056eedd85439d4e4`
- `sf1_profile_v1.json`: `d89a4fdc030b0c7c4911719ea133c780d9f4610b08eef1d6d0e0ca443391718e`
- `sf1_profile_v1.ts`: `9a3bb8e4d96518b3a0a1301046b2d86a793991959ebdd8adda1fb2988e4292dc`
- `sf1_profile_v1.go`: `0f0348b8751b0f85fe874afda3371af75b78fac5dad65182204dcb3cf3e4c0a1`
- `sf1_profile_v1.rs`: `66b5956826c86589a24b71ca6b400cc1335323c6371f1cec9475f09af8743f61`

## Sign-Off Log

| ရက်စွဲ | အင်ဂျင်နီယာ | စာရင်းစစ်ရလဒ် | မှတ်စုများ |
|------|----------|------------------|------|
| 2026-02-12 | Tooling (LLM) | ❌ ပျက်ကွက် | အဆင့် 1- `cargo test -p sorafs_chunker` သည် `vectors` suite များ ခေတ်နောက်ကျနေသောကြောင့် မအောင်မြင်ပါ။ အဆင့် 2- `ci/check_sorafs_fixtures.sh` ပျက်သွားသည်—`manifest_signatures.json` သည် repo အခြေအနေတွင် ပျောက်နေသည် (အလုပ်လုပ်သစ်ပင်တွင် ဖျက်ထားသည်)။ အဆင့် 4- `export_vectors` မန်နီးဖက်စ်ဖိုင်မရှိသော်လည်း လက်မှတ်များကို အတည်မပြုနိုင်ပါ။ လက်မှတ်ထိုးထားသော ကိရိယာများကို ပြန်လည်ရယူခြင်း (သို့မဟုတ် ကောင်စီသော့ကို ပေးဆောင်ခြင်း) နှင့် စည်းနှောင်မှုများကို ပြန်လည်ထုတ်ပေးခြင်းအား စမ်းသပ်မှုများအရ လိုအပ်ချက်အရ Canonical လက်ကိုင်များကို ထည့်သွင်းထားရန် အကြံပြုအပ်ပါသည်။ |
| 2026-02-12 | Tooling (LLM) | ✅ အောင်မြင်ပြီး | `cargo run --locked -p sorafs_chunker --bin export_vectors -- --signing-key=000102…1f` မှတစ်ဆင့် ပြန်လည်ထုတ်လုပ်ထားသော ဆက်စပ်ပစ္စည်းများ၊ canonical handle-only alias lists နှင့် အသစ်သော manifest digest `c8c45c025ecee39b5ac5bf3db3dc1e2f97a7eaf7ea0aac72056eedd85439d4e4` ကို ထုတ်လုပ်သည်။ `cargo test -p sorafs_chunker` နှင့် သန့်ရှင်းသော `ci/check_sorafs_fixtures.sh` ဖြင့် စစ်ဆေးခြင်း (စစ်ဆေးမှုအတွက် အဆင့်လိုက် တပ်ဆင်မှုများ)။ အဆင့် 5 Node parity helper ရောက်သည်အထိ ဆိုင်းငံ့ထားသည်။ |
| 2026-02-20 | Storage Tooling CI | ✅ အောင်မြင်ပြီး | `ci/check_sorafs_fixtures.sh` မှတစ်ဆင့် ရယူခဲ့သည့် လွှတ်တော်စာအိတ် (`fixtures/sorafs_chunker/manifest_signatures.json`) ဇာတ်ညွှန်းကို ပြန်လည်ထုတ်လုပ်ထားသော ပြင်ဆင်မှုများ၊ အတည်ပြုထားသော manifest digest `c8c45c025ecee39b5ac5bf3db3dc1e2f97a7eaf7ea0aac72056eedd85439d4e4`၊ နှင့် Rust ကြိုး (Go/Node အဆင့်များကို ရရှိသည့်အခါတွင် လုပ်ဆောင်ရန်) ကွဲပြားမှုမရှိပါ။ |

ကိရိယာတန်ဆာပလာ WG သည် စစ်ဆေးရန်စာရင်းကိုဖွင့်ပြီးနောက် ရက်စွဲပါအတန်းတစ်ခုကို ပေါင်းထည့်သင့်သည်။ အကြင်အဆင့်
အဆင်မပြေပါက ဤနေရာတွင် လင့်ခ်ချိတ်ထားသော ပြဿနာကို တင်သွင်းပြီး ပြန်လည်ပြင်ဆင်ခြင်းဆိုင်ရာ အသေးစိတ်အချက်အလက်များကို မတိုင်မီ ထည့်သွင်းပါ။
အသစ်ပြင်ဆင်မှုများ သို့မဟုတ် ပရိုဖိုင်များကို အတည်ပြုခြင်း။