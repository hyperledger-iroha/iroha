---
lang: my
direction: ltr
source: docs/examples/finance/repo_custodian_ack_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c52d7f2c5ec9dc4cda81895561bc1261659935c94bf3f7febb0867f4981fe616
source_last_modified: "2026-01-22T16:26:46.472177+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Repo Custodian Acknowledgement Template

repo (နှစ်နိုင်ငံ သို့မဟုတ် သုံးပါတီ) က အုပ်ထိန်းသူကို ရည်ညွှန်းသည့်အခါ ဤပုံစံကို အသုံးပြုပါ။
`RepoAgreement::custodian` မှတဆင့် ရည်ရွယ်ချက်မှာ အချုပ်အနှောင် SLA လမ်းကြောင်းကို မှတ်တမ်းတင်ရန်ဖြစ်သည်။
အကောင့်များနှင့် ပိုင်ဆိုင်မှုများကို မရွှေ့မီ အဆက်အသွယ်များကို တူးပါ။ နမူနာပုံစံကို သင့်ထံကူးယူပါ။
အထောက်အထားလမ်းညွှန် (ဥပမာ
`artifacts/finance/repo/<slug>/custodian_ack_<custodian>.md`) ကိုဖြည့်ပါ။
နေရာချထားသူများ၊ နှင့်ဖော်ပြထားသော အုပ်ချုပ်မှုပက်ကေ့ခ်ျ၏ တစ်စိတ်တစ်ပိုင်းအနေဖြင့် ဖိုင်ကို hash လုပ်ပါ။
`docs/source/finance/repo_ops.md` §2.8။

## 1. မက်တာဒေတာ

| လယ် | တန်ဖိုး |
|---------|-------|
| သဘောတူညီချက်အမှတ်အသား | `<repo-yyMMdd-XX>` |
| စောင့်ထိန်းအကောင့် ID | `<ih58...>` |
| ပြင်ဆင်သည်/ရက်စွဲ | `<custodian ops lead>` |
| စားပွဲမှ အဆက်အသွယ်များ | `<desk lead + counterparty>` |
| အထောက်အထားလမ်းညွှန် | ``artifacts/finance/repo/<slug>/`` |

## 2. Custody Scope

- **အပေါင်ပစ္စည်း အဓိပ္ပါယ်ဖွင့်ဆိုချက်များ-** `<list of asset definition ids>`
- ** ငွေသားခြေထောက်ငွေကြေး / အခြေချရထားလမ်း-** `<xor#sora / other>`
- **Custody window:** `<start/end timestamps or SLA summary>`
- **ရပ်နေသောညွှန်ကြားချက်များ-** `<hash + path to standing instruction document>`
- **အလိုအလျောက်လုပ်ရန် လိုအပ်ချက်များ-** `<scripts, configs, or runbooks custodian will invoke>`

## 3. Routing & Monitoring

| ပစ္စည်း | တန်ဖိုး |
|------|-------|
| ချုပ်နှောင်ထားသော ပိုက်ဆံအိတ်/လယ်ဂျာ အကောင့် | `<asset ids or ledger path>` |
| စောင့်ကြည့်ရေးလိုင်း | `<Slack/phone/on-call rotation>` |
| ဆက်သွယ်ရန်|တူး `<primary + backup>` |
| လိုအပ်သောသတိပေးချက်များ | `<PagerDuty service, Grafana board, etc.>` |

## 4. ထုတ်ပြန်ချက်

1. *ထိန်းသိမ်းရန် အဆင်သင့်-* "ကျွန်ုပ်တို့သည် အဆင့်လိုက် `repo initiate` ပါဝန်အား ပြန်လည်သုံးသပ်ပါသည်။
   အထက်ဖော်ပြပါ အထောက်အထားများသည် SLA အောက်တွင် အပေါင်ပစ္စည်းလက်ခံရန် ပြင်ဆင်ထားပါသည်။
   § 2 တွင်။"
2. *Rollback ကတိကဝတ်-* "အထက်ပါအမည်ရှိ rollback playbook ကို အကောင်အထည်ဖော်မည်ဆိုပါက၊
   အခင်းဖြစ်ပွားရာဌာနမှူးမှ ညွှန်ကြားပြီး CLI မှတ်တမ်းများ နှင့် ဟက်ခ်များ ပေးပါမည်။
   `governance/drills/<timestamp>.log`။
3. *အထောက်အထား သိမ်းဆည်းခြင်း-* “ကျွန်ုပ်တို့သည် အသိအမှတ်ပြုခြင်း၊
   ညွှန်ကြားချက်များ၊ နှင့် CLI မှတ်တမ်းများသည် အနည်းဆုံး `<duration>` အတွက် ၎င်းတို့ကို ပေးဆောင်ပါ။
   ဘဏ္ဍာရေးကောင်စီက တောင်းခံပါတယ်။”

အောက်တွင် လက်မှတ်ရေးထိုးပါ (အုပ်ချုပ်မှုမှတဆင့် ဖြတ်သန်းသည့်အခါ လက်ခံနိုင်သော အီလက်ထရွန်းနစ် လက်မှတ်များ
ခြေရာခံသူ)။

| အမည် | အခန်းကဏ္ဍ | လက်မှတ်/ရက်စွဲ |
|------|------|------------------|
| `<custodian ops lead>` | စောင့်မျော်အော် | `<signature>` |
| `<desk lead>` | စားပွဲခုံ | `<signature>` |
| `<counterparty>` | ကောင်တာ | `<signature>` |

> လက်မှတ်ထိုးပြီးသည်နှင့်၊ ဖိုင်ကို hash (ဥပမာ- `sha256sum custodian_ack_<cust>.md`) နှင့်
> အုပ်ချုပ်မှု ပက်ကတ်ဇယားတွင် အချေအတင်ကို မှတ်တမ်းတင်ထားသောကြောင့် ပြန်လည်သုံးသပ်သူများသည် ၎င်းကို အတည်ပြုနိုင်သည်။
> မဲပေးစဉ်အတွင်း ကိုးကားထားသော အသိအမှတ်ပြုဘိုက်များ။