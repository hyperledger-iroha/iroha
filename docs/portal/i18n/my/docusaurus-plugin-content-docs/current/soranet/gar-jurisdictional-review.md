---
lang: my
direction: ltr
source: docs/portal/docs/soranet/gar-jurisdictional-review.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: GAR Jurisdictional Review (SNNet-9)
sidebar_label: GAR Jurisdictional Review
description: Signed-off jurisdiction decisions and Blake2b digests to wire into SoraNet compliance configs.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

SNNet-9 လိုက်နာမှုလမ်းကြောင်းသည် ယခု ပြီးမြောက်သွားပါပြီ။ ဤစာမျက်နှာတွင် လက်မှတ် ရေးထိုးထားသည်။
တရားစီရင်ပိုင်ခွင့်ဆိုင်ရာ ဆုံးဖြတ်ချက်များ၊ Blake2b-256 သည် အချေအတင်အော်ပရေတာများသည် ၎င်းတို့၏ထဲသို့ ကူးယူရမည်ဖြစ်သည်။
`compliance.attestations` လုပ်ကွက်များနှင့် နောက်တစ်ကြိမ် ပြန်လည်သုံးသပ်သည့် ရက်စွဲများ။ လက်မှတ်ကို သိမ်းထားပါ။
သင်၏အုပ်ချုပ်မှုမှတ်တမ်းမှတ်တမ်းရှိ PDF များ ဤစာစုများသည် canonical fingerprints များဖြစ်သည်။
အလိုအလျောက်စနစ်နှင့် စာရင်းစစ်များအတွက်။

| တရားစီရင်ပိုင်ခွင့် | ဆုံးဖြတ်ချက် | မှတ်စုတို | Blake2b-256 digest (အကြီးစား hex) | နောက်တစ်ခုသုံးသပ်ချက် |
|-----------------|----------------|------|----------------------------------------------------------------|
| ယူအက်စ် | တိုက်ရိုက် သီးသန့် သယ်ယူပို့ဆောင်ရေး လိုအပ်သည် (SoraNet ဆားကစ်များ မရှိပါ) | `governance/compliance/attestations/us-2027-q2.md` | `1636B0B52286896C4894FA0333CD691D9B3DB7F2B73548EA2EA622B90A09BCF7` | 2027-09-30 |
| ကနေဒါ | တိုက်ရိုက်-သပ်သပ် သယ်ယူပို့ဆောင်ရေး လိုအပ်သည် | `governance/compliance/attestations/ca-2027-q2.md` | `52D9D9EE1E43DA0526D8C659AC61C1844858F9A6A74650EA5C04CBD8F8614063` | 2027-09-30 |
| EU/EEA | အမည်မသိ SoraNet သယ်ယူပို့ဆောင်ရေးအား SNNet-8 လျှို့ဝှက်ရေးဘတ်ဂျက်များ ပြဌာန်းထားသည် | `governance/compliance/attestations/eu-2027-q2.md` | `30FDAF718095E87FDFADA6BE3EC1EF9D56DFFDEE97BF4BBEAB9013F7A0963B15` | 2027-09-30 |

## ဖြန့်ကျက်မှု အတိုအထွာ

```jsonc
{
  "compliance": {
    "operator_jurisdictions": ["US", "CA", "DE"],
    "jurisdiction_opt_outs": ["US", "CA"],
    "blinded_cid_opt_outs": [
      "C6B434E5F23ABD318F01FEDB834B34BD16B46E0CC44CD70536233A632DFA3828",
      "7F8B1E9D04878F1AEAB553C1DB0A3E3A2AB689F75FE6BE17469F85A4D201B4AC"
    ],
    "attestations": [
      {
        "jurisdiction": "US",
        "document_uri": "norito://gar/attestations/us-2027-q2.pdf",
        "digest_hex": "1636B0B52286896C4894FA0333CD691D9B3DB7F2B73548EA2EA622B90A09BCF7",
        "issued_at_ms": 1805313600000,
        "expires_at_ms": 1822248000000
      },
      {
        "jurisdiction": "CA",
        "document_uri": "norito://gar/attestations/ca-2027-q2.pdf",
        "digest_hex": "52D9D9EE1E43DA0526D8C659AC61C1844858F9A6A74650EA5C04CBD8F8614063",
        "issued_at_ms": 1805313600000,
        "expires_at_ms": 1822248000000
      },
      {
        "jurisdiction": "EU",
        "document_uri": "norito://gar/attestations/eu-2027-q2.pdf",
        "digest_hex": "30FDAF718095E87FDFADA6BE3EC1EF9D56DFFDEE97BF4BBEAB9013F7A0963B15",
        "issued_at_ms": 1805313600000
      }
    ]
  }
}
```

## စာရင်းစစ်

- အတည်ပြုချက်အချေအတင်များကို ထုတ်လုပ်မှုပုံစံများအတွင်း အတိအကျကူးယူထားသည်။
- `jurisdiction_opt_outs` သည် canonical catalogue နှင့် ကိုက်ညီသည်။
- ကိုက်ညီသောအညွှန်းများနှင့်အတူ သင်၏အုပ်ချုပ်မှုမှတ်တမ်းတွင် သိမ်းဆည်းထားသော PDF များကို လက်မှတ်ရေးထိုးထားသည်။
- အသက်သွင်းခြင်းဝင်းဒိုးနှင့် GAR မှတ်တမ်းစာအုပ်တွင် ရိုက်ကူးထားသော အတည်ပြုသူများ။
- အထက်ဇယားမှ နောက်တစ်ကြိမ် ပြန်လည်သုံးသပ်ခြင်းသတိပေးချက်များကို စီစဉ်ထားပါသည်။

## ကိုလည်းကြည့်ပါ။

- [GAR အော်ပရေတာ စတင်ခြင်းအကျဉ်းချုပ်](gar-operator-onboarding)
- [GAR Compliance Playbook (အရင်းအမြစ်)](../../../source/soranet/gar_compliance_playbook.md)