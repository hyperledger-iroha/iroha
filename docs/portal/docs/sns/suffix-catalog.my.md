---
lang: my
direction: ltr
source: docs/portal/docs/sns/suffix-catalog.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ffd062b69b97f11e5baa0ae82256c87cb76600982d599e1953573c1944112f51
source_last_modified: "2026-01-22T16:26:46.520176+00:00"
translation_last_reviewed: 2026-02-07
title: Sora Name Service Suffix Catalog
sidebar_label: Suffix catalog
description: Canonical allowlist of SNS suffixes, stewards, and pricing knobs for `.sora`, `.nexus`, and `.dao`.
translator: machine-google-reviewed
---

# Sora အမည် ဝန်ဆောင်မှု Suffix Catalog

SNS လမ်းပြမြေပုံသည် အတည်ပြုထားသော နောက်ဆက်တွဲတိုင်း (SN-1/SN-2) ကို ခြေရာခံသည်။ ဤစာမျက်နှာသည် ရောင်ပြန်ဟပ်နေသည်။
source-of-truth catalog သည် မှတ်ပုံတင်သူများ၊ DNS ဂိတ်ဝေးများ သို့မဟုတ် ပိုက်ဆံအိတ်ကို အသုံးပြုနေသည့် အော်ပရေတာများဖြစ်သည်။
tooling သည် status docs ကို မခြစ်ဘဲ တူညီသော ဘောင်များကို တင်နိုင်သည်။

- **လျှပ်တစ်ပြက်-** [`docs/examples/sns/suffix_catalog_v1.json`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/examples/sns/suffix_catalog_v1.json)
- **စားသုံးသူများ-** `iroha sns policy`၊ SNS onboarding kits၊ KPI ဒက်ရှ်ဘုတ်များနှင့်
  DNS/Gateway ထုတ်ပေးသည့် script များအားလုံးသည် တူညီသော JSON အစုအဝေးကို ဖတ်သည်။
- **အခြေအနေများ-** `active` (မှတ်ပုံတင်ခွင့်ပြုထားသည်)၊ `paused` (ယာယီပိတ်ထားသည်)၊
  `revoked` (ကြေငြာထားသော်လည်း လောလောဆယ်မရနိုင်ပါ)။

## ကတ်တလောက် အစီအစဉ်

| လယ် | ရိုက် | ဖော်ပြချက် |
|---------|------|-------------|
| `suffix` | string | ဦးဆောင်အစက်ဖြင့် လူသားဖတ်နိုင်သော နောက်ဆက်တွဲ။ |
| `suffix_id` | `u16` | `SuffixPolicyV1::suffix_id` တွင် စာရင်းသွင်းထားသော အထောက်အထားကို သိမ်းဆည်းထားသည်။ |
| `status` | enum | စတင်ရန် အဆင်သင့်ဖြစ်မှုကို ဖော်ပြသည့် `active`၊ `paused` သို့မဟုတ် `revoked`။ |
| `steward_account` | string | ထိန်းကျောင်းမှုအတွက် တာဝန်ရှိသော အကောင့် (မှတ်ပုံတင်အရာရှိ မူဝါဒ ချိတ်တွဲများနှင့် ကိုက်ညီသည်)။ |
| `fund_splitter_account` | string | `fee_split` နှုန်းဖြင့် လမ်းကြောင်းမပြောင်းမီ ငွေပေးချေမှုများကို လက်ခံရရှိသော အကောင့်။ |
| `payment_asset_id` | string | အခြေချမှုအတွက် အသုံးပြုသည့် ပိုင်ဆိုင်မှု (`xor#sora`) |
| `min_term_years` / `max_term_years` | ကိန်းပြည့် | မူဝါဒအရ ဝယ်ယူမှု သက်တမ်းကန့်သတ်ချက်။ |
| `grace_period_days` / `redemption_period_days` | ကိန်းပြည့် | Torii ဖြင့် သက်တမ်းတိုးခြင်း ဘေးကင်းရေး ပြတင်းပေါက်များ။ |
| `referral_cap_bps` | ကိန်းပြည့် | အုပ်ချုပ်မှု (အခြေခံအချက်များ) မှ ခွင့်ပြုထားသော အများဆုံး လွှဲပြောင်းပေးပို့ခြင်း |
| `reserved_labels` | ခင်းကျင်း | အုပ်ချုပ်မှု-ကာကွယ်ထားသော အညွှန်းအရာဝတ္ထု `{label, assigned_to, release_at_ms, note}`။ |
| `pricing` | ခင်းကျင်း | `label_regex`၊ `base_price`၊ `auction_kind` နှင့် ကြာချိန်ဘောင်များပါရှိသော အရာဝတ္ထုများ။ |
| `fee_split` | အရာဝတ္ထု | `{treasury_bps, steward_bps, referral_max_bps, escrow_bps}` အခြေခံ-အမှတ် ခွဲပြီး။ |
| `policy_version` | ကိန်းပြည့် | အုပ်ချုပ်ရေးမူဝါဒကို တည်းဖြတ်သည့်အခါတိုင်း Monotonic တန်ပြန်မှု တိုးလာသည်။ |

## လက်ရှိ ကတ်တလောက်

| နောက်ဆက် | ID (`hex`) | ဘဏ္ဍာစိုး | ရန်ပုံငွေခွဲခြမ်းစိတ်ဖြာ | အဆင့်အတန်း | ငွေပေးချေမှု ပိုင်ဆိုင်မှု | ရည်ညွှန်းထုပ် (bps) | သက်တမ်း (အနည်းဆုံး – အများဆုံးနှစ်) | ကျေးဇူးတော် / ရွေးနှုတ်ခြင်း (နေ့ရက်များ) | စျေးနှုန်းအဆင့်များ (regex → အခြေခံစျေးနှုန်း / လေလံ) | တံဆိပ်များ | အခကြေးငွေ ပိုင်းခြားခြင်း (T/S/R/E bps) | မူဝါဒဗားရှင်း |
|--------|------------------|---------------------|----------------|----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| `.sora` | `0x0001` | `i105...` | `i105...` | သက်ဝင် | `xor#sora` | 500 | 1 – 5 | 30/60 | `T0: ^[a-z0-9]{3,}$ → 120 XOR (Vickrey)` | `treasury → i105...` | `7000 / 3000 / 1000 / 0` | ၁ |
| `.nexus` | `0x0002` | `i105...` | `i105...` | ရပ်ထားသည် | `xor#sora` | 300 | 1 – 3 | ၁၅/၃၀| `T0: ^[a-z0-9]{4,}$ → 480 XOR (Vickrey)`<br>`T1: ^[a-z]{2}$ → 4000 XOR (Dutch floor 500)` | `treasury → i105...`, `guardian → i105...` | `6500 / 2500 / 800 / 200` | 2 |
| `.dao` | `0x0003` | `i105...` | `i105...` | ရုတ်သိမ်း | `xor#sora` | 0 | 1 – 2 | 30 / 30 | `T0: ^[a-z0-9]{3,}$ → 60 XOR (Vickrey)` | `dao (held for future release)` | `9000 / 1000 / 0 / 0` | 0 |

## JSON ကောက်နုတ်ချက်

```json
{
  "version": 1,
  "generated_at": "2026-05-01T00:00:00Z",
  "suffixes": [
    {
      "suffix": ".sora",
      "suffix_id": 1,
      "status": "active",
      "fund_splitter_account": "i105...",
      "payment_asset_id": "xor#sora",
      "referral_cap_bps": 500,
      "pricing": [
        {
          "tier_id": 0,
          "label_regex": "^[a-z0-9]{3,}$",
          "base_price": {"asset_id": "xor#sora", "amount": 120},
          "auction_kind": "vickrey_commit_reveal",
          "min_duration_years": 1,
          "max_duration_years": 5
        }
      ],
      "...": "see docs/examples/sns/suffix_catalog_v1.json for the full record"
    }
  ]
}
```

## အလိုအလျောက်စနစ်မှတ်စုများ

1. အော်ပရေတာများသို့မဖြန့်ဝေမီ JSON လျှပ်တစ်ပြက်ရိုက်ချက်အား တင်ပြီး ၎င်းကို hash/လက်မှတ်ထိုးပါ။
2. မှတ်ပုံတင်အရာရှိသည် ကိရိယာတန်ဆာပလာသည် `suffix_id`၊ သက်တမ်းကန့်သတ်ချက်များနှင့် ဈေးနှုန်းများကို ဖော်ပြသင့်သည်
   တောင်းဆိုမှုတစ်ခု `/v1/sns/*` ရောက်တိုင်း ကတ်တလောက်မှ။
3. DNS/Gateway အကူအညီပေးသူများသည် GAR ကိုထုတ်လုပ်သည့်အခါ သီးသန့်တံဆိပ်မက်တာဒေတာကို ဖတ်သည်။
   DNS တုံ့ပြန်မှုများသည် အုပ်ချုပ်မှုထိန်းချုပ်မှုများနှင့် လိုက်လျောညီထွေရှိနေစေရန် နမူနာပုံစံများ။
4. KPI နောက်ဆက်တွဲ အလုပ်များ တဂ်ဒက်ရှ်ဘုတ် တင်ပို့မှုတွင် နောက်ဆက်တွဲ မက်တာဒေတာပါရှိသောကြောင့် သတိပေးချက်များသည် တူညီသည်
   ဤနေရာတွင် မှတ်တမ်းတင်ထားသော ပစ်လွှတ်မှုအခြေအနေ။