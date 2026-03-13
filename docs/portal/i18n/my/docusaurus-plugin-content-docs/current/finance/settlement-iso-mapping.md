---
id: settlement-iso-mapping
lang: my
direction: ltr
source: docs/portal/docs/finance/settlement-iso-mapping.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Settlement ↔ ISO 20022 Field Mapping
sidebar_label: Settlement ↔ ISO 20022
description: Canonical mapping between Iroha settlement flows and the ISO 20022 bridge.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

::: Canonical Source ကို သတိပြုပါ။
:::

## ဖြေရှင်းခြင်း ↔ ISO 20022 အကွက်မြေပုံဆွဲခြင်း။

ဤမှတ်စုသည် Iroha အခြေချနေထိုင်မှု ညွှန်ကြားချက်များကြားတွင် canonical mapping ကို ဖမ်းယူပါသည်။
(`DvpIsi`၊ `PvpIsi`၊ repo collateral flows) နှင့် ISO 20022 စာတိုများကို ကျင့်သုံးသည်
တံတားနားမှာ။ စာတိုငြမ်းကို ရောင်ပြန်ဟပ်သည်။
`crates/ivm/src/iso20022.rs` နှင့် ထုတ်လုပ်သည့်အခါ အကိုးအကားအဖြစ် ဆောင်ရွက်သည်။
Norito ပေးချေမှုများအား အတည်ပြုခြင်း။

### အကိုးအကား ဒေတာမူဝါဒ (သတ်မှတ်ခြင်းနှင့် အတည်ပြုခြင်း)

ဤမူဝါဒသည် သတ်မှတ်သူဦးစားပေးရွေးချယ်မှုများ၊ အတည်ပြုခြင်းစည်းမျဉ်းများနှင့် ကိုးကားချက်ဒေတာများကို ထုပ်ပိုးထားသည်။
မက်ဆေ့ဂျ်များမထုတ်မီ Norito ↔ ISO 20022 တံတားမှ လိုက်နာရမည့်တာဝန်များ။

**ISO မက်ဆေ့ဂျ်အတွင်း အထိန်းအမှတ်များ-**
- **တူရိယာ အမှတ်အသားများ** → `delivery_leg.asset_definition_id` ↔ `SctiesLeg/FinInstrmId`
  (သို့မဟုတ် ညီမျှသော တူရိယာနယ်ပယ်)။
- **ပါတီများ/ကိုယ်စားလှယ်များ** → `DlvrgSttlmPties/Pty` နှင့် `RcvgSttlmPties/Pty`၊
  သို့မဟုတ် `pacs.009` ရှိ အေးဂျင့်ဖွဲ့စည်းပုံများ။
- ** အကောင့်များ** → `…/Acct` ဒြပ်စင်များ လယ်ဂျာကို ကြေးမုံ
  `AccountId` တွင် `SupplementaryData`။
- **မူပိုင် အထောက်အထားများ** → `…/OthrId` နှင့် `Tp/Prtry` နှင့် ရောင်ပြန်ဟပ်ထားသည်
  `SupplementaryData`။ စည်းမျဥ်းစည်းကမ်းသတ်မှတ်သူများကို တစ်ဦးတည်းပိုင်ပစ္စည်းများဖြင့် ဘယ်တော့မှ အစားထိုးပါ။

#### မက်ဆေ့ချ်မိသားစုအလိုက် သတ်မှတ်သူဦးစားပေးရွေးချယ်မှု

##### `sese.023` / `.024` / `.025` (ငွေချေးစာချုပ်)

- **တူရိယာ (`FinInstrmId`)**
  - ဦးစားပေး- `…/ISIN` အောက်တွင် **ISIN**။ ၎င်းသည် CSDs / T2S အတွက် canonical identifier ဖြစ်သည်။[^anna]
  - မှားယွင်းမှုများ-
    - ISO ပြင်ပမှ သတ်မှတ်ထားသော `…/OthrId/Id` အောက်တွင် **CUIP** သို့မဟုတ် အခြား NSIN
      ကုဒ်စာရင်း (ဥပမာ၊ `CUSP`); ထုတ်ပေးသူအား `Issr` တွင် ထည့်သွင်းပါ။[^iso_mdr]
    - **Norito ပိုင်ဆိုင်မှု ID** အဖြစ်- `…/OthrId/Id`၊ `Tp/Prtry="NORITO_ASSET_ID"` နှင့်
      တူညီသောတန်ဖိုးကို `SupplementaryData` တွင် မှတ်တမ်းတင်ပါ။
  - ရွေးချယ်နိုင်သောဖော်ပြချက်များ- **CFI** (`ClssfctnTp`) နှင့် **FISN** တို့ကို လွယ်ကူစေရန် ပံ့ပိုးပေးသော
    ပြန်လည်သင့်မြတ်ရေး။[^iso_cfi][^iso_fisn]
- **ပါတီများ (`DlvrgSttlmPties`၊ `RcvgSttlmPties`)**
  - ဦးစားပေး- **BIC** (`AnyBIC/BICFI`၊ ISO 9362)။[^swift_bic]
  - နောက်ပြန်ဆုတ်ခြင်း- **LEI** မက်ဆေ့ချ်ဗားရှင်းသည် သီးသန့် LEI အကွက်ကို ဖော်ထုတ်သည့်နေရာတွင်၊ အကယ်၍
    ပျက်ကွက်၊ ရှင်းလင်းသော `Prtry` အညွှန်းများပါရှိသော ကိုယ်ပိုင် ID များကို သယ်ဆောင်ပြီး မက်တာဒေတာတွင် BIC ပါဝင်သည်။
- **နေရာ/နေရာ** → နေရာအတွက် **MIC** နှင့် CSD အတွက် **BIC**။[^iso_mic]

##### `colr.010` / `.011` / `.012` နှင့် `colr.007` (စရံစီမံခန့်ခွဲမှု)

- `sese.*` (ISIN ဦးစားပေးသည်) ကဲ့သို့ တူရိယာစည်းမျဉ်းများကို လိုက်နာပါ။
- ပါတီများသည် **BIC** ကို မူရင်းအတိုင်း အသုံးပြုသည်။ **LEI** သည် ၎င်းကို schema ဖော်ထုတ်သည့်နေရာတွင် လက်ခံနိုင်သည်။[^swift_bic]
- ငွေသားပမာဏသည် မှန်ကန်သော အသေးစားယူနစ်များဖြင့် **ISO 4217** ငွေကြေးကုဒ်များကို အသုံးပြုရပါမည်။[^iso_4217]

##### `pacs.009` / `camt.054` (PvP ရန်ပုံငွေနှင့် ရှင်းတမ်းများ)

- ** အေးဂျင့်များ (`InstgAgt`၊ `InstdAgt`၊ မြီစား/ချေးငွေပေးသူ အေးဂျင့်များ)** → စိတ်ကြိုက်ရွေးချယ်နိုင်သော **BIC**
  ခွင့်ပြုထားသည့် LEI။[^swift_bic]
- **အကောင့်များ**
  - Interbank- **BIC** နှင့် ဌာနတွင်းအကောင့်ကိုးကားချက်များဖြင့် ခွဲခြားသတ်မှတ်ပါ။
  - ဖောက်သည်ရင်ဆိုင်နေရသောထုတ်ပြန်ချက် (`camt.054`) - တင်ပြသည့်အခါ **IBAN** ကို ထည့်သွင်းပြီး အတည်ပြုပါ။
    (အရှည်၊ နိုင်ငံစည်းမျဉ်း၊ mod-97 checksum)။[^swift_iban]
- **Currency** → **ISO 4217** စာလုံး 3 လုံးပါသော ကုဒ်၊ ယူနစ်အသေးစားကို လှည့်ပတ်ခြင်းကို လေးစားပါ။[^iso_4217]
- **Torii ထည့်သွင်းခြင်း** → `POST /v2/iso20022/pacs009` မှတစ်ဆင့် PvP ရန်ပုံငွေ ခြေထောက်များကို တင်သွင်းပါ။ တံတား
  `Purp=SECU` လိုအပ်ပြီး ယခုအခါ ကိုးကားချက်ဒေတာကို ပြင်ဆင်သတ်မှတ်သည့်အခါ BIC လမ်းကူးများကို ပြဌာန်းထားသည်။

#### အတည်ပြုခြင်းစည်းမျဉ်းများ (ထုတ်လွှတ်ခြင်းမပြုမီ ကျင့်သုံးပါ)

| အမှတ်အသား | အတည်ပြုခြင်းစည်းမျဉ်း | မှတ်စုများ |
|--------------------|-----------------|--------|
| **ISIN** | Regex `^[A-Z]{2}[A-Z0-9]{9}[0-9]$` နှင့် Luhn (mod-10) ISO 6166 နောက်ဆက်တွဲ C | တံတားထုတ်လွှတ်ခြင်းမပြုမီ ငြင်းပယ်ခြင်း၊ အထက်ပိုင်း ကြွယ်ဝမှုကို ပိုနှစ်သက်သည်။[^anna_luhn] |
| **CUSIP** | အလေးချိန် 2 ခုပါသော Regex `^[A-Z0-9]{9}$` နှင့် modulus-10 (စာလုံးများမှ ဂဏန်းများအထိ မြေပုံ) | ISIN ကို မရရှိနိုင်သည့်အခါမှသာ၊ မြေပုံကို ANNA/CUSIP လူကူးမျဉ်းကြားမှ တဆင့် တခါတည်း ရရှိခဲ့သည်။[^cusip] |
| **LEI** | Regex `^[A-Z0-9]{18}[0-9]{2}$` နှင့် mod-97 စစ်ဆေးသောဂဏန်း (ISO 17442) | လက်ခံခြင်းမပြုမီ GLEIF နေ့စဉ် မြစ်ဝကျွန်းပေါ် ဖိုင်များကို သက်သေပြပါ။[^gleif] |
| **BIC** | Regex `^[A-Z]{4}[A-Z]{2}[A-Z0-9]{2}([A-Z0-9]{3})?$` | ရွေးချယ်နိုင်သော အခွဲကုဒ် (နောက်ဆုံးစာလုံး သုံးခု)။ RA ဖိုင်များတွင် လက်ရှိအခြေအနေအား အတည်ပြုပါ။[^swift_bic] |
| **MIC** | ISO 10383 RA ဖိုင်မှ ထိန်းသိမ်းပါ။ နေရာများကို တက်ကြွမှုရှိစေရန် သေချာပါ (`!` ရပ်စဲရေး အလံမပါ) | ထုတ်လွှတ်ခြင်းမပြုမီ အလံပြထားသည့် MIC များကို ရပ်ဆိုင်းထားသည်။[^iso_mic] |
| **IBAN** | နိုင်ငံအလိုက် အရှည်၊ စာလုံးအကြီး၊ အက္ခရာဂဏန်း၊ mod-97 = 1 | SWIFT မှ ထိန်းသိမ်းထားသော မှတ်ပုံတင်ခြင်းကို အသုံးပြုပါ။ ဖွဲ့စည်းပုံအရ မမှန်ကန်သော IBAN များကို ငြင်းပယ်ပါ။[^swift_iban] |
| **ကိုယ်ပိုင်အကောင့်/ပါတီ ID** | ဖြတ်ထားသော နေရာလွတ်များပါရှိသော `Max35Text` (UTF-8၊ ≤35 လုံး) | `GenericAccountIdentification1.Id` နှင့် `PartyIdentification135.Othr/Id` အကွက်များနှင့် သက်ဆိုင်ပါသည်။ အက္ခရာ 35 လုံးထက် ကျော်လွန်သော ထည့်သွင်းမှုများကို ငြင်းဆိုခြင်းဖြင့် ပေါင်းကူးပေးချေမှုများသည် ISO အစီအစဉ်များနှင့် ကိုက်ညီပါသည်။ |
| **Proxy အကောင့် သတ်မှတ်ချက်များ** | `…/Prxy/Tp/{Cd,Prtry}` | မူလ IBAN နှင့်အတူ သိမ်းဆည်းထားသည်။ PvP သံလမ်းများကို mirror ရန် proxy လက်ကိုင်များ (ရွေးချယ်နိုင်သော အမျိုးအစားကုဒ်များပါသော) ကို လက်ခံနေစဉ် တရားဝင်အတည်ပြုချက်တွင် IBAN များ လိုအပ်နေသေးသည်။ |
| **CFI** | ISO 10962 ခွဲခြားသတ်မှတ်မှု | အသုံးပြုထားသော အက္ခရာခြောက်လုံး၊ ရွေးချယ်နိုင်သောကြွယ်ဝမှု; တူရိယာအတန်းအစား ဇာတ်ကောင်များ ကိုက်ညီမှုရှိစေရန်။[^iso_cfi] |
| **FISN** | စာလုံး 35 လုံးအထိ၊ စာလုံးအကြီးအသေးနှင့် ကန့်သတ်သတ်ပုံများ | ရွေးချယ်ခွင့်; ISO 18774 လမ်းညွှန်ချက်အတိုင်း ဖြတ်တောက်/ပုံမှန်ဖြစ်စေရန်။[^iso_fisn] |
| **ငွေကြေး** | ISO 4217 စာလုံး ၃ လုံး ကုဒ်၊ အသေးစားယူနစ်များမှ ဆုံးဖြတ်သည့် စကေး ပမာဏများသည် ခွင့်ပြုထားသော ဒဿမများအထိ ဝိုင်းထားရမည်။ Norito ဘက်တွင် ပြဋ္ဌာန်းရန်။[^iso_4217] |

#### လူကူးမျဉ်းကြားနှင့် ဒေတာ ပြုပြင်ထိန်းသိမ်းရေး တာဝန်များ

- **ISIN ↔ Norito ပိုင်ဆိုင်မှု ID** နှင့် **CUSIP ↔ ISIN** လူကူးမျဉ်းကြားများကို ထိန်းသိမ်းပါ။ မှ ညတိုင်း update လုပ်ပါ။
  ANNA/DSB feeds နှင့် version သည် CI မှအသုံးပြုသော လျှပ်တစ်ပြက်ရိုက်ချက်များကို ထိန်းချုပ်ပါသည်။[^anna_crosswalk]
- **BIC ↔ LEI** မြေပုံများကို GLEIF အများသူငှာ ဆက်ဆံရေးဖိုင်များမှ ပြန်လည်စတင်၍ တံတားအသုံးပြုနိုင်သည်
  လိုအပ်သည့်အခါ နှစ်မျိုးလုံးကို ထုတ်လွှတ်သည်။[^bic_lei]
- **MIC အဓိပ္ပါယ်ဖွင့်ဆိုချက်** ကို တံတား မက်တာဒေတာဘေးတွင် သိမ်းဆည်းထားပါ ထို့ကြောင့် နေရာကို တရားဝင်အောင်ပြုလုပ်ပါ။
  RA ဖိုင်များသည် နေ့လယ်ပိုင်း ပြောင်းလဲသည့်အခါတွင်ပင် အဆုံးအဖြတ်ပေးသည်။[^iso_mic]
- စာရင်းစစ်အတွက် တံတား metadata တွင် ဒေတာသက်သေ (အချိန်တံဆိပ် + ရင်းမြစ်) ကို မှတ်တမ်းတင်ပါ။ ဆက်နေပါ။
  ထုတ်လွှတ်သောညွှန်ကြားချက်များနှင့်အတူ လျှပ်တစ်ပြက်အမှတ်အသား။
- loaded dataset တစ်ခုစီ၏မိတ္တူကို ဆက်ရှိနေစေရန် `iso_bridge.reference_data.cache_dir` ကို စီစဉ်သတ်မှတ်ပါ
  သက်သေပြချက် မက်တာဒေတာ (ဗားရှင်း၊ အရင်းအမြစ်၊ အချိန်တံဆိပ်၊ ချက်ဆမ်း) တို့နှင့်အတူ။ ဒါကို စာရင်းစစ်တွေက ခွင့်ပြုတယ်။
  အထက်ပိုင်းလျှပ်တစ်ပြက်ရိုက်ချက်များ လှည့်ပြီးသည့်တိုင် သမိုင်းဆိုင်ရာ ဖိဒ်များကို ကွဲပြားစေရန် အော်ပရေတာများ။
- ISO လူကူးမျဉ်းကြားဓာတ်ပုံများကို `iroha_core::iso_bridge::reference_data` ကိုအသုံးပြု၍ မျိုချမိသည်
  `iso_bridge.reference_data` ဖွဲ့စည်းမှုပိတ်ဆို့ခြင်း (လမ်းကြောင်းများ + ပြန်လည်စတင်သည့်ကာလ)။ တိုင်းတာမှုများ
  `iso_reference_status`၊ `iso_reference_age_seconds`၊ `iso_reference_records` နှင့်
  `iso_reference_refresh_interval_secs` သတိပေးချက်အတွက် runtime ကျန်းမာရေးကို ဖော်ထုတ်ပါ။ Torii
  တံတားသည် `pacs.008` တင်ပြမှုများကို ငြင်းပယ်သည်
  လူကူးမျဉ်းကြား၊ အဆုံးအဖြတ်ပေးသော `InvalidIdentifier` အမှားအယွင်းများကို ကျော်လွှားနေသည်
  အမည်မသိ။ 【crates/iroha_torii/src/iso20022_bridge.rs#L1078】
- IBAN နှင့် ISO 4217 ချိတ်ဆက်မှုများကို တူညီသောအလွှာတွင် ပြဋ္ဌာန်းထားသည်- pacs.008/pacs.009 ယခု စီးဆင်းနေသည်
  မြီစား/ချေးငွေပေးသူ IBAN များသည် ပုံသေပုံစံအမည်များမရှိသည့်အခါ သို့မဟုတ် မည်သည့်အချိန်တွင် `InvalidIdentifier` အမှားများကို ထုတ်လွှတ်သည်
  ငွေချေမှုငွေကြေးသည် `currency_assets` မှ လွဲချော်နေသဖြင့် ပုံစံမမှန်သော တံတားကို ကာကွယ်ပေးသည်
  လယ်ဂျာသို့ရောက်ရှိရန်ညွှန်ကြားချက်များ။ IBAN အတည်ပြုချက်သည် နိုင်ငံအလိုက် သက်ဆိုင်ပါသည်။
  ISO 7064 mod-97 pass မတိုင်မီ အလျားနှင့် ဂဏန်းစစ်ဆေးသည့် ဂဏန်းများသည် ဖွဲ့စည်းတည်ဆောက်ပုံအရ မမှန်ကန်ပါ
  တန်ဖိုးများကို စောစောစီးစီး ပယ်ချပါသည်။ 【crates/iroha_torii/src/iso20022_bridge.rs#L775】【crates/iroha_torii/src/iso20022_bridge.rs#L827】【crates/ivm/src/iso20022.rs#L1255】
- CLI အခြေချကူညီသူများသည် တူညီသော အစောင့်သံလမ်းများကို အမွေဆက်ခံသည်- pass
  `--iso-reference-crosswalk <path>` နှင့်အတူ `--delivery-instrument-id` နှင့်အတူ DvP ရှိရန်
  `sese.023` XML လျှပ်တစ်ပြက်ကို ထုတ်လွှတ်ခြင်းမပြုမီ မှန်ကန်သည့် တူရိယာ ID များကို အစမ်းကြည့်ရှုပါ။ 【crates/iroha_cli/src/main.rs#L3752】
- `cargo xtask iso-bridge-lint` (နှင့် CI ထုပ်ပိုးခြင်း `ci/check_iso_reference_data.sh`) ပျဉ်းမနား၊
  လူကူးမျဉ်းကြား လျှပ်တစ်ပြက် ဓာတ်ပုံများနှင့် မီးပုံးများ။ command သည် `--isin`၊ `--bic-lei`၊ `--mic` ကို လက်ခံပြီး
  `--fixtures` ကို အလံပြပြီး အလုပ်လုပ်သောအခါ `fixtures/iso_bridge/` ရှိ နမူနာဒေတာအတွဲများသို့ ပြန်သွားသည်
  အကြောင်းပြချက်မရှိဘဲ။【xtask/src/main.rs#L146】【ci/check_iso_reference_data.sh#L1】
- IVM ကူညီပေးသူသည် ယခုအခါ အမှန်တကယ် ISO 20022 XML စာအိတ်များကို စားသုံးနေသည် (head.001 + `DataPDU` + `Document`)
  `head.001` schema မှတစ်ဆင့် Business Application Header ကို တရားဝင်စစ်ဆေးပြီး `BizMsgIdr`၊
  `MsgDefIdr`၊ `CreDt`၊ နှင့် BIC/ClrSysMmbId အေးဂျင့်များကို တိကျစွာ ထိန်းသိမ်းထားသည်။ XMLDSig/XAdES
  လုပ်ကွက်များသည် ရည်ရွယ်ချက်ရှိရှိ ကျော်သွားကြသည်။ 

#### စည်းမျဥ်းစည်းကမ်းနှင့် စျေးကွက်-ဖွဲ့စည်းပုံကို ထည့်သွင်းစဉ်းစားခြင်း။- **T+1 အခြေချမှု**- US/Canada ရှယ်ယာဈေးကွက်များသည် 2024 ခုနှစ်တွင် T+1 သို့ ပြောင်းရွှေ့ခဲ့သည်။ Norito ကို ချိန်ညှိပါ။
  အချိန်ဇယားဆွဲခြင်းနှင့် SLA သတိပေးချက်များနှင့်အညီ။[^sec_t1][^csa_t1]
- **CSDR ပြစ်ဒဏ်များ**- ငွေသားပြစ်ဒဏ်များ ပြဋ္ဌာန်းခြင်းဆိုင်ရာ စည်းကမ်းဥပဒေများ။ Norito သေချာပါစေ။
  မက်တာဒေတာသည် ပြန်လည်သင့်မြတ်ရေးအတွက် ပြစ်ဒဏ်ကိုးကားချက်များကို ဖမ်းယူပါသည်။[^csdr]
- **နေ့ချင်းပြီး အခြေချနေထိုင်သည့် လေယာဉ်မှူးများ**- အိန္ဒိယ၏ စည်းကမ်းထိန်းသိမ်းရေးအဖွဲ့သည် T0/T+0 အခြေချနေထိုင်မှုတွင် အဆင့်မြှင့်တင်နေသည်။ စောင့်ရှောက်
  လေယာဉ်မှူးများ တိုးချဲ့လာသည်နှင့်အမျှ တံတားပြက္ခဒိန်များကို အပ်ဒိတ်လုပ်ထားသည်။[^india_t0]
- **အပေါင်ပစ္စည်းဝယ်ယူခြင်း/လက်ကျန်များ**- ဝယ်ယူမှုအချိန်စာရင်းများနှင့် စိတ်ကြိုက်ကိုင်ဆောင်မှုများရှိ ESMA အပ်ဒိတ်များကို စောင့်ကြည့်ပါ
  ထို့ကြောင့် အခြေအနေအရ ပေးပို့ခြင်း (`HldInd`) သည် နောက်ဆုံးလမ်းညွှန်ချက်နှင့် ကိုက်ညီပါသည်။[^csdr]

[^anna]: ANNA ISIN Guidelines, December 2023. https://anna-web.org/wp-content/uploads/2024/01/ISIN-Guidelines-Version-22-Dec-2023.pdf
[^iso_mdr]: ISO 20022 external code list (CUSIP `CUSP`) and MDR Part 2. https://www.iso20022.org/milestone/22048/download
[^iso_cfi]: ISO 10962 (CFI) taxonomy. https://www.iso.org/standard/81140.html
[^iso_fisn]: ISO 18774 (FISN) format guidance. https://www.iso.org/standard/66153.html
[^swift_bic]: SWIFT business identifier code (ISO 9362) guidance. https://www.swift.com/standards/data-standards/bic-business-identifier-code
[^iso_cr]: ISO 20022 change request introducing LEI options for party identification. https://www.iso20022.org/milestone/16116/download
[^iso_mic]: ISO 10383 Market Identifier Code maintenance agency. https://www.iso20022.org/market-identifier-codes
[^iso_4217]: ISO 4217 currency and minor-units table (SIX). https://www.six-group.com/en/products-services/financial-information/market-reference-data/data-standards.html
[^swift_iban]: IBAN registry and validation rules. https://www.swift.com/swift-resource/22851/download
[^anna_luhn]: ISIN checksum algorithm (Annex C). https://www.anna-dsb.com/isin/
[^cusip]: CUSIP format and checksum rules. https://www.iso20022.org/milestone/22048/download
[^gleif]: GLEIF LEI structure and validation details. https://www.gleif.org/en/organizational-identity/introducing-the-legal-entity-identifier-lei/iso-17442-the-lei-code-structure
[^anna_crosswalk]: ISIN cross-reference (ANNA DSB) feeds for derivatives and debt instruments. https://www.anna-dsb.com/isin/
[^bic_lei]: GLEIF BIC-to-LEI relationship files. https://www.gleif.org/en/lei-data/lei-mapping/download-bic-to-lei-relationship-files
[^sec_t1]: SEC release on US T+1 transition (2023). https://www.sec.gov/newsroom/press-releases/2023-29
[^csa_t1]: CSA amendments for Canadian institutional trade matching (T+1). https://www.osc.ca/en/securities-law/instruments-rules-policies/2/24-101/csa-notice-amendments-national-instrument-24-101-institutional-trade-matching-and-settlement-and
[^csdr]: ESMA CSDR settlement discipline / penalty mechanism updates. https://www.esma.europa.eu/sites/default/files/2024-11/ESMA74-2119945925-2059_Final_Report_on_Technical_Advice_on_CSDR_Penalty_Mechanism.pdf
[^india_t0]: SEBI circular on same-day settlement pilot. https://www.reuters.com/sustainability/boards-policy-regulation/india-markets-regulator-extends-deadline-same-day-settlement-plan-brokers-2025-04-29/

### Delivery-versus-Payment → `sese.023`

| DvP အကွက် | ISO 20022 လမ်းကြောင်း | မှတ်စုများ |
|----------------------------------------------------------------|------------------------------------------------|--------------------|
| `settlement_id` | `TxId` | တည်ငြိမ်သော ဘဝသံသရာ အမှတ်အသား |
| `delivery_leg.asset_definition_id` (လုံခြုံရေး) | `SctiesLeg/FinInstrmId` | Canonical identifier (ISIN၊ CUSIP၊ …) |
| `delivery_leg.quantity` | `SctiesLeg/Qty` | ဒဿမ string; ဂုဏ်ထူးဆောင်ပစ္စည်း တိကျမှု |
| `payment_leg.asset_definition_id` (ငွေကြေး) | `CashLeg/Ccy` | ISO ငွေကြေးကုဒ် |
| `payment_leg.quantity` | `CashLeg/Amt` | ဒဿမ string; ကိန်းဂဏာန်းသတ်မှတ်ချက် |
| `delivery_leg.from` (ရောင်းချသူ/ပေးပို့သူ) | `DlvrgSttlmPties/Pty/Bic` | ပါဝင်သူအား ပေးပို့ခြင်း၏ BIC *(အကောင့် canonical ID ကို မက်တာဒေတာတွင် လောလောဆယ် တင်ပို့ထားသည်)* |
| `delivery_leg.from` အကောင့်အမှတ်အသား | `DlvrgSttlmPties/Acct` | အခမဲ့ပုံစံ; Norito မက်တာဒေတာသည် အကောင့် ID | အတိအကျကို သယ်ဆောင်သည်။
| `delivery_leg.to` (ဝယ်သူ/လက်ခံပါတီ) | `RcvgSttlmPties/Pty/Bic` | BIC တွင ် |လက်ခံခြင်း။
| `delivery_leg.to` အကောင့်အမှတ်အသား | `RcvgSttlmPties/Acct` | အခမဲ့ပုံစံ; လက်ခံရရှိသောအကောင့် ID | ကိုက်ညီပါသည်။
| `plan.order` | `Plan/ExecutionOrder` | စာရင်း- `DELIVERY_THEN_PAYMENT` သို့မဟုတ် `PAYMENT_THEN_DELIVERY` |
| `plan.atomicity` | `Plan/Atomicity` | စာရင်း- `ALL_OR_NOTHING`, `COMMIT_FIRST_LEG`, `COMMIT_SECOND_LEG` |
| **မက်ဆေ့ချ်ရည်ရွယ်ချက်** | `SttlmTpAndAddtlParams/SctiesMvmntTp` | `DELI` (ပေးပို့) သို့မဟုတ် `RECE` (လက်ခံ); တင်သွင်းသူပါတီက ကွပ်မျက်သည့် ကြေးမုံမှန်များ။ |
|                                                        | `SttlmTpAndAddtlParams/Pmt` | `APMT` (ငွေပေးချေမှုဆန့်ကျင်ဘက်) သို့မဟုတ် `FREE` (အခမဲ့-ပေးချေမှု)။ |
| `delivery_leg.metadata`, `payment_leg.metadata` | `SctiesLeg/Metadata`, `CashLeg/Metadata` | ရွေးချယ်နိုင်သော Norito JSON ကို UTF-8 | အဖြစ် ကုဒ်လုပ်ထားသည်။

> **အခြေချနေထိုင်မှုဆိုင်ရာ အရည်အချင်းသတ်မှတ်ချက်များ** – အခြေချနေထိုင်မှုအခြေအနေကုဒ်များ (`SttlmTxCond`)၊ တစ်စိတ်တစ်ပိုင်းအခြေချမှုညွှန်းကိန်းများ (`PrtlSttlmInd`) နှင့် Norito မက်တာဒေတာကို I18NI5000017 သို့ရောက်ရှိချိန်တွင် ကူးယူခြင်းဖြင့် စျေးကွက်အလေ့အကျင့်ကို တံတားမှန်ကိုမြင်စေသည်။ ISO ပြင်ပကုဒ်စာရင်းများတွင် ထုတ်ပြန်ထားသော စာရင်းကောက်မှုများကို တွန်းအားပေးရန် ဦးတည်ရာ CSD သည် တန်ဖိုးများကို အသိအမှတ်ပြုသည်။

### ငွေပေးချေမှု-ငွေပေးချေမှုရန်ပုံငွေ → `pacs.009`

PvP ညွှန်ကြားချက်ကို ထောက်ပံ့ပေးသော ငွေသားအတွက် ခြေထောက်များကို FI-to-FI ခရက်ဒစ်အဖြစ် ထုတ်ပေးပါသည်။
လွှဲပြောင်းမှုများ။ တံတားသည် ဤငွေပေးချေမှုများကို မှတ်သားထားသောကြောင့် အောက်ပိုင်းစနစ်များကို အသိအမှတ်ပြုပါသည်။
သူတို့က ငွေရေးကြေးရေး ပြေလည်အောင် ဆောင်ရွက်ပေးတယ်။

| PvP ရန်ပုံငွေအကွက် | ISO 20022 လမ်းကြောင်း | မှတ်စုများ |
|------------------------------------------------|----------------------------------------------------------------|--------------------|
| `primary_leg.quantity` / {amount, currency} | `IntrBkSttlmAmt` + `IntrBkSttlmCcy` | စတင်သူထံမှ ငွေထုတ်သည့် ပမာဏ/ငွေကြေး။ |
| ကောင်တာအေးဂျင့် သတ်မှတ်ချက်များ | `InstgAgt`, `InstdAgt` | BIC/LEI ကိုယ်စားလှယ်များ ပေးပို့ခြင်းနှင့် လက်ခံခြင်း။ |
| ဖြေရှင်းခြင်း ရည်ရွယ်ချက် | `CdtTrfTxInf/PmtTpInf/CtgyPurp/Cd` | လုံခြုံရေးဆိုင်ရာ PvP ရန်ပုံငွေအတွက် `SECU` သို့ သတ်မှတ်ပါ။ |
| Norito မက်တာဒေတာ (အကောင့်အိုင်ဒီများ၊ FX ဒေတာ) | `CdtTrfTxInf/SplmtryData` | AccountId၊ FX အချိန်တံဆိပ်တုံးများ၊ အကောင်အထည်ဖော်မှုအစီအစဉ် အရိပ်အမြွက်များ အပြည့်အစုံပါရှိသည်။ |
| ညွှန်ကြားချက် အမှတ်အသား/ ဘဝသံသရာ ချိတ်ဆက်ခြင်း | `CdtTrfTxInf/PmtId/InstrId`, `CdtTrfTxInf/RmtInf` | Norito `settlement_id` နှင့် ကိုက်ညီသောကြောင့် ငွေသားခြေထောက်သည် ငွေရေးကြေးရေးဘက်နှင့် ညှိနှိုင်းသည်။ |

JavaScript SDK ၏ ISO တံတားသည် ၎င်းကို ပုံသေသတ်မှတ်ခြင်းဖြင့် ဤလိုအပ်ချက်နှင့် ကိုက်ညီသည်။
`pacs.009` အမျိုးအစား ရည်ရွယ်ချက် `SECU`; ခေါ်ဆိုသူများသည် ၎င်းကို အခြားတစ်ခုဖြင့် အစားထိုးနိုင်သည်။
လုံခြုံရေးမဟုတ်သော ခရက်ဒစ်လွှဲပြောင်းမှုများ ထုတ်လွှတ်သည့်အခါ မှန်ကန်သော ISO ကုဒ်၊ သို့သော် မမှန်ပါ။
တန်ဖိုးများကို ရှေ့တွင် ပယ်ချပါသည်။

အခြေခံအဆောက်အအုံတစ်ခုသည် တိကျသေချာသော ငွေရေးကြေးရေးအတည်ပြုချက် လိုအပ်ပါက တံတားဖြစ်သည်။
`sese.025` ကို ဆက်လက်ထုတ်လွှတ်နေသော်လည်း၊ ထိုအတည်ပြုချက်သည် လုံခြုံရေးခြေထောက်ကို ထင်ဟပ်စေသည်
အခြေအနေ (ဥပမာ၊ `ConfSts = ACCP`) PvP "ရည်ရွယ်ချက်" ထက်။

### ငွေပေးချေမှု-ငွေပေးချေမှု အတည်ပြုချက် → `sese.025`

| PvP အကွက် | ISO 20022 လမ်းကြောင်း | မှတ်စုများ |
|------------------------------------------------|---------------------------------|--------------------|
| `settlement_id` | `TxId` | တည်ငြိမ်သော ဘဝသံသရာ အမှတ်အသား |
| `primary_leg.asset_definition_id` | `SttlmCcy` | မူလခြေထောက် | အတွက် ငွေကြေးကုဒ်
| `primary_leg.quantity` | `SttlmAmt` | စတင်သူ | မှ ပေးပို့သော ပမာဏ
| `counter_leg.asset_definition_id` | `AddtlInf` (JSON payload) | ဖြည့်စွက်အချက်အလက် | တွင် ထည့်သွင်းထားသော တန်ပြန်ငွေကြေးကုဒ်
| `counter_leg.quantity` | `SttlmQty` | တန်ပြန်ပမာဏ |
| `plan.order` | `Plan/ExecutionOrder` | DvP | ကဲ့သို့တူညီသော enum အစုံ
| `plan.atomicity` | `Plan/Atomicity` | DvP | ကဲ့သို့တူညီသော enum အစုံ
| `plan.atomicity` အခြေအနေ (`ConfSts`) | `ConfSts` | ကိုက်ညီသောအခါ `ACCP`; တံတားသည် ငြင်းပယ်ခြင်း |
| Counterparty identifiers | `AddtlInf` JSON | လက်ရှိတံတားသည် မက်တာဒေတာ | တွင် AccountId/BIC tuples အပြည့်အစုံကို အမှတ်အသားပြုသည်။

### Repo Collateral Substitution → `colr.007`

| Repo အကွက် / ဆက်စပ် | ISO 20022 လမ်းကြောင်း | မှတ်စုများ |
|------------------------------------------------|--------------------------------|--------------------|
| `agreement_id` (`RepoIsi` / `ReverseRepoIsi`) | `OblgtnId` | Repo စာချုပ်အမှတ်အသား |
| အပေါင်ပစ္စည်းအစားထိုး Tx identifier | `TxId` | အစားထိုး နှုန်း |ထုတ်ပေးသည်။
| မူရင်းစရံပမာဏ | `Substitution/OriginalAmt` | အစားထိုးခြင်းမပြုမီ စရံကတိပြုထားသော ပွဲများ |
| မူရင်းစရံငွေကြေး | `Substitution/OriginalCcy` | ငွေကြေးကုဒ် |
| အပေါင်ပစ္စည်း ပမာဏ | အစားထိုး `Substitution/SubstituteAmt` | အစားထိုးပမာဏ |
| အပေါင်ပစ္စည်း အစားထိုးငွေကြေး | `Substitution/SubstituteCcy` | ငွေကြေးကုဒ် |
| အကျိုးသက်ရောက်သည့်ရက်စွဲ (အုပ်ချုပ်ရေးအနားသတ်အချိန်ဇယား) | `Substitution/EffectiveDt` | ISO ရက်စွဲ (YYYY-MM-DD) |
| ဆံပင်ညှပ်ခြင်း အမျိုးအစားခွဲခြင်း | `Substitution/Type` | အုပ်ချုပ်မှုမူဝါဒအပေါ် အခြေခံ၍ လောလောဆယ် `FULL` သို့မဟုတ် `PARTIAL` |
| အုပ်ချူပ်ရေးအကြောင်းပြ/ဆံပင်ညှပ်မှတ်စု| `Substitution/ReasonCd` | ရွေးချယ်နိုင်သော၊ အုပ်ချုပ်မှုဆိုင်ရာ ကျိုးကြောင်းဆီလျော်မှု |

### ရန်ပုံငွေနှင့် ထုတ်ပြန်ချက်

| Iroha ဆက်စပ် | ISO 20022 မက်ဆေ့ဂျ် | မြေပုံထုတ်ခြင်းတည်နေရာ |
|------------------------------------------------|--------------------------------|------------------|
| Repo ငွေသားခြေထောက်ကို စက်နှိုးခြင်း / unwind | `pacs.009` | `IntrBkSttlmAmt`, `IntrBkSttlmCcy`, `IntrBkSttlmDt`, `InstgAgt`, `InstdAgt` DvP/PvP ခြေထောက်များမှ ပြည့်နေ |
| အခြေချပြောဆိုချက် | `camt.054` | `Ntfctn/Ntry[*]` အောက်တွင် မှတ်တမ်းတင်ထားသော ငွေပေးချေမှု ခြေထောက်လှုပ်ရှားမှုများ။ တံတားသည် `SplmtryData` | တွင် စာရင်းသွင်း/အကောင့် မက်တာဒေတာကို ထိုးသွင်းသည်။

### အသုံးပြုမှုမှတ်စုများ* ပမာဏအားလုံးကို Norito ဂဏန်းအထောက်အကူများ (`NumericSpec`) ကိုအသုံးပြု၍ ပမာဏအားလုံးကို နံပါတ်စဉ်တပ်ထားသည်။
  ပိုင်ဆိုင်မှု အဓိပ္ပါယ်ဖွင့်ဆိုချက်များတစ်လျှောက် အတိုင်းအတာအလိုက် ကိုက်ညီမှုရှိစေရန်။
* `TxId` တန်ဖိုးများသည် `Max35Text` — UTF‑8 အရှည် ≤ 35 လုံးအား မပေးမီ
  ISO 20022 မက်ဆေ့ဂျ်များသို့ တင်ပို့ခြင်း။
* BIC များသည် စာလုံးကြီး 8 သို့မဟုတ် 11 လုံးဖြစ်ရမည် (ISO9362); ငြင်းပယ်
  Norito ငွေပေးချေမှုများ သို့မဟုတ် ပြေလည်မှုမထုတ်မီ ဤချက်လက်မှတ်ကို မအောင်မြင်သည့် မက်တာဒေတာ
  အတည်ပြုချက်များ။
* အကောင့်အမှတ်အသားများ (AccountId / ChainId) အား ဖြည့်စွက်စာအဖြစ် တင်ပို့သည်။
  ပါဝင်သူများ လက်ခံရရှိခြင်းသည် ၎င်းတို့၏ ဒေသဆိုင်ရာ စာရင်းဇယားများနှင့် ညှိနှိုင်းနိုင်သောကြောင့် မက်တာဒေတာ။
* `SupplementaryData` သည် canonical JSON ဖြစ်ရမည် (UTF-8၊ စီထားသောသော့များ၊ JSON-ဇာတိဖြစ်သည်
  လွတ်မြောက်ခြင်း)။ SDK အကူအညီပေးသူများသည် လက်မှတ်များ၊ တယ်လီမီတာ ဟက်ကာများနှင့် ISO တို့ကို ပြဋ္ဌာန်းထားသည်။
  payload archives များသည် ပြန်လည်တည်ဆောက်မှုများတွင် အဆုံးအဖြတ်အတိုင်း ဆက်လက်တည်ရှိနေပါသည်။
* ငွေကြေးပမာဏများသည် ISO4217 အပိုင်းကိန်းဂဏန်းများအတိုင်း (ဥပမာ JPY တွင် 0 ရှိသည်။
  ဒဿမများ၊ USD တွင် 2); တံတားသည် Norito ကိန်းဂဏာန်းတိကျမှုနှင့်အညီ ကုပ်ထားသည်။
* CLI အခြေချကူညီသူများ (`iroha app settlement ... --atomicity ...`) သည် ယခု ထုတ်လွှတ်သည်။
  Norito ကွပ်မျက်မှုအစီအစဥ်ရေးဆွဲထားသည့် ညွှန်ကြားချက်များသည် 1:1 ကို `Plan/ExecutionOrder` သို့ လည်းကောင်း၊
  `Plan/Atomicity` အထက်။
* ISO အကူအညီပေးသူ (`ivm::iso20022`) သည် အထက်ဖော်ပြပါ အကွက်များကို သက်သေပြပြီး ငြင်းပယ်သည်
  DvP/PvP ခြေထောက်များသည် ကိန်းဂဏာန်းသတ်မှတ်ချက်များကို ချိုးဖောက်သည့် မက်ဆေ့ချ်များ

### SDK Builder Helpers

- JavaScript SDK သည် ယခု `buildPacs008Message` / ကို ဖော်ထုတ်ပြသည်
  `buildPacs009Message` (`javascript/iroha_js/src/isoBridge.js` ကိုကြည့်ပါ) ဒါကြောင့် client
  အလိုအလျောက်စနစ်ဖြင့် ဖွဲ့စည်းထားသော အခြေချနေထိုင်မှု မက်တာဒေတာ (BIC/LEI၊ IBANs၊
  ရည်ရွယ်ချက်ကုဒ်များ၊ နောက်ဆက်တွဲ Norito အကွက်များ) ဆုံးဖြတ်သတ်မှတ်ထားသော pacs XML သို့
  ဤလမ်းညွှန်မှ မြေပုံစည်းမျဉ်းများကို ပြန်လည်အကောင်အထည်မဖော်ဘဲ။
- ကူညီသူနှစ်ဦးစလုံးသည် တိကျပြတ်သားသော `creationDateTime` (အချိန်ဇုန်နှင့်အတူ ISO-8601) လိုအပ်သည်
  ထို့ကြောင့် အော်ပရေတာများသည် ၎င်းတို့၏အလုပ်အသွားအလာမှ အဆုံးအဖြတ်အချိန်တံဆိပ်တစ်ခုကို ချည်ထားရမည်ဖြစ်ပါသည်။
  SDK ကို နံရံနာရီအချိန်သို့ ထားရန်။
- `recipes/iso_bridge_builder.mjs` သည် ထိုအကူအညီများကို မည်ကဲ့သို့ ကြိုးသွယ်ရမည်ကို သရုပ်ပြသည်။
  ပတ်ဝန်းကျင် variables သို့မဟုတ် JSON config ဖိုင်များကို ပေါင်းစည်းထားသည့် CLI သည် ၎င်းကို ပရင့်ထုတ်သည်။
  ထုတ်လုပ်ထားသော XML နှင့် ပြန်သုံးခြင်းဖြင့် ၎င်းကို Torii (`ISO_SUBMIT=1`) သို့ ရွေးချယ်နိုင်သည်
  ISO တံတားစာရွက်နှင့် တူညီသော စောင့်ဆိုင်းမှုပုံစံ။


### ကိုးကား

- `SttlmTpAndAddtlParams/SctiesMvmntTp` (`DELI`/`RECE`) နှင့် `Pmt` ကိုပြသသည့် LuxCSD / Clearstream ISO 20022 ဖြေရှင်းမှု နမူနာများ (`APMT`/`FREE`)။<sup>[1](https://www.luxcsd.com/resource/blob/3434074/6f8add4708407a4701055be4dd04846b/c23005-eis-examples-cbf-data.pdf)</sup>
- အခြေချနေထိုင်မှုဆိုင်ရာ အရည်အသွေးသတ်မှတ်ချက်များ ပါဝင်သော Clearstream DCP သတ်မှတ်ချက်များ (`SttlmTxCond`၊ `PrtlSttlmInd`)။<sup>[2](https://www.clearstream.com/clearstream-en/res-library/market-coverage/instruction-specifications-swift-iso-20022-dcp-mode-ceu-spain-2357008)</sup>
- SWIFT PMPG လမ်းညွှန်ချက်သည် `pacs.009` ဖြင့် `CtgyPurp/Cd = SECU` ဖြင့် လုံခြုံရေးဆိုင်ရာ PvP ရန်ပုံငွေအတွက် အကြံပြုထားသည်။<sup>[3](https://www.swift.com/swift-resource/251897/download)</sup>
- ISO 20022 မက်ဆေ့ချ် အဓိပ္ပါယ်သတ်မှတ်ချက် အစီရင်ခံစာများ (BIC၊ Max35Text)။<sup>[4](https://www.iso20022.org/sites/default/files/2020-12/ISO20022_MDRPart2_ChangeOrVerifyAccountIdentification_2020_2021_v1_ForSEGReview.pdf)</sup>
- ANNA DSB လမ်းညွှန်ချက် ISIN ဖော်မတ်နှင့် checksum စည်းမျဉ်းများ။<sup>[5](https://www.anna-dsb.com/isin/)</sup>

### အသုံးပြုပုံ အကြံပြုချက်များ

- LLM စစ်ဆေးနိုင်စေရန် သက်ဆိုင်ရာ Norito အတိုအထွာ သို့မဟုတ် CLI အမိန့်ကို အမြဲကူးထည့်ပါ။
  နယ်ပယ်အမည်အတိအကျနှင့် ကိန်းဂဏာန်းစကေးများ။
- စာရွက်လမ်းကြောင်းတစ်ခုကို ထိန်းသိမ်းထားရန် ကိုးကားချက်များ (`provide clause references`) ကို တောင်းဆိုပါ။
  လိုက်နာမှုနှင့် စာရင်းစစ်သုံးသပ်ချက်။
- `docs/source/finance/settlement_iso_mapping.md` တွင် အဖြေအကျဉ်းချုပ်ကို ရိုက်ပါ။
  (သို့မဟုတ် လင့်ခ်ချိတ်ထားသော နောက်ဆက်တွဲများ) ထို့ကြောင့် အနာဂတ် အင်ဂျင်နီယာများသည် မေးမြန်းမှုကို ထပ်လုပ်ရန် မလိုအပ်ပါ။

## ပွဲအစီအစဉ်များမှာယူခြင်း (ISO 20022 ↔ Norito တံတား)

### ဇာတ်လမ်း A — အပေါင်ပစ္စည်း အစားထိုးခြင်း (ပြန်ပို / ကတိစကား)

**ပါဝင်သူများ-** အပေါင်ပစ္စည်းပေးသူ/ယူသူ (နှင့်/သို့မဟုတ် အေးဂျင့်များ)၊ စောင့်ထိန်းသူ(များ)၊ CSD/T2S  
**အချိန်-** စျေးကွက်ဖြတ်တောက်မှုများနှင့် T2S နေ့/ည လည်ပတ်မှုနှုန်း။ ခြေထောက်နှစ်ချောင်းကို စုစည်းကာ တူညီသော အခြေချပြတင်းပေါက်တွင် ပြီးအောင် လေ့ကျင့်ပါ။

#### Message ကကွက်
1. `colr.010` အပေါင်ပစ္စည်းအစားထိုးတောင်းဆိုမှု → အပေါင်ပစ္စည်းပေးသူ/ယူသူ သို့မဟုတ် ကိုယ်စားလှယ်။  
2. `colr.011` အပေါင်ပစ္စည်းအစားထိုး တုံ့ပြန်မှု → လက်ခံ/ပယ်ချခြင်း (ရွေးချယ်နိုင်သော ငြင်းပယ်ခြင်းအကြောင်းရင်း)။  
3. `colr.012` Collateral Substitution Confirmation → အစားထိုးသဘောတူညီချက်ကို အတည်ပြုသည်။  
4. `sese.023` ညွှန်ကြားချက်များ (ခြေထောက်နှစ်ချောင်း)။  
   - မူရင်းအပေါင်ပစ္စည်း (`SctiesMvmntTp=DELI`၊ `Pmt=FREE`၊ `SctiesTxTp=COLO`) ပြန်ပေးပါ။  
   - အစားထိုးအပေါင်ပစ္စည်း (`SctiesMvmntTp=RECE`၊ `Pmt=FREE`၊ `SctiesTxTp=COLI`) ပေးပို့ပါ။  
   အတွဲကို လင့်ခ် (အောက်တွင်ကြည့်ပါ)။  
5. `sese.024` အခြေအနေအကြံပြုချက်များ (လက်ခံသည်၊ လိုက်ဖက်သည်၊ ဆိုင်းငံ့ထားသည်၊ ကျရှုံးသည်၊ ငြင်းပယ်သည်)။  
6. `sese.025` တစ်ကြိမ် ကြိုတင်စာရင်းသွင်းပြီး အတည်ပြုချက်များ။  
7. ရွေးချယ်နိုင်သော ငွေသားမြစ်ဝကျွန်းပေါ်ဒေသ (အခကြေးငွေ/ဆံပင်ညှပ်ခြင်း) → `pacs.009` `CtgyPurp/Cd = SECU` ဖြင့် FI-to-FI ခရက်ဒစ်လွှဲပြောင်းခြင်း အခြေအနေကို `pacs.002` မှတစ်ဆင့်၊ `pacs.004` မှတစ်ဆင့် ပြန်လည်ရောက်ရှိသည်။

#### လိုအပ်သော အသိအမှတ်ပြုမှုများ / အခြေအနေများ
- သယ်ယူပို့ဆောင်ရေးအဆင့်- ဂိတ်ဝေးများသည် `admi.007` ကို ထုတ်လွှတ်နိုင်သည် သို့မဟုတ် လုပ်ငန်းမလုပ်ဆောင်မီ ငြင်းပယ်နိုင်သည်။  
- အခြေချမှုဘဝသံသရာ- `sese.024` (လုပ်ဆောင်နေသည့် အခြေအနေများ + အကြောင်းပြချက်ကုဒ်များ), `sese.025` (နောက်ဆုံး)။  
- ငွေသားအခြမ်း- `pacs.002` (`PDNG`၊ `ACSC`၊ `RJCT` စသည်ဖြင့်)၊ `pacs.004`။

#### အခြေအနေ/အကွက်များကို ဖြေလျှော့ပါ။
- `SctiesSttlmTxInstr/Lnkgs` (`WITH`/`BEFO`/`AFTE`) ညွှန်ကြားချက်နှစ်ခုကို ကွင်းဆက်ပြုလုပ်ရန်။  
စံသတ်မှတ်ချက်များ ပြည့်မီသည်အထိ ကိုင်ထားရန် - `SttlmParams/HldInd` `sese.030` (`sese.031` အခြေအနေ) မှတစ်ဆင့် ထုတ်ဝေသည်။  
တစ်စိတ်တစ်ပိုင်းအခြေချမှုကို ထိန်းချုပ်ရန် - `SttlmParams/PrtlSttlmInd` (`NPAR`၊ `PART`၊ `PARC`၊ `PARQ`)။  
စျေးကွက်သတ်မှတ်မှုအခြေအနေများ (`NOMC` စသည်ဖြင့်) အတွက် `SttlmParams/SttlmTxCond/Cd`။  
- ပံ့ပိုးပေးသည့်အခါ ရွေးချယ်နိုင်သော T2S သတ်မှတ်ချက်အတိုင်း ငွေချေးခြင်း (CoSD) စည်းမျဉ်းများ။

#### ကိုးကား
- SWIFT အပေါင်ပစ္စည်းစီမံခန့်ခွဲမှု MDR (`colr.010/011/012`)။  
- ချိတ်ဆက်ခြင်းနှင့် အခြေအနေများအတွက် CSD/T2S အသုံးပြုမှုလမ်းညွှန်များ (ဥပမာ၊ DNB၊ ECB Insights)။  
- SMPG အခြေချလေ့ကျင့်မှု၊ Clearstream DCP လက်စွဲများ၊ ASX ISO အလုပ်ရုံများ။

### Scenario B — FX Window Breach (PvP Funding Failure)

**ပါဝင်သူများ-** လုပ်ဖော်ကိုင်ဖက်များနှင့် ငွေသားအေးဂျင့်များ၊ ငွေချေးသက်သေခံလက်မှတ်များ၊ CSD/T2S  
**အချိန်ကာလ-** FX PvP windows (CLS/bilateral) နှင့် CSD ဖြတ်တောက်မှုများ၊ ငွေသားအတည်ပြုချက်ကို ဆိုင်းငံ့ထားရန် ငွေရေးကြေးရေးများကို ဆိုင်းငံ့ထားပါ။

#### Message ကကွက်
1. `pacs.009` `CtgyPurp/Cd = SECU` ဖြင့် ငွေကြေးတစ်ခုလျှင် FI-to-FI ခရက်ဒစ်လွှဲပြောင်းခြင်း `pacs.002` မှတစ်ဆင့် အခြေအနေ၊ `camt.056`/`camt.029` မှတစ်ဆင့် ပြန်လည်ခေါ်ယူ/ပယ်ဖျက်ခြင်း။ အတည်တကျဖြစ်လျှင် `pacs.004` ပြန်ပေးသည်။  
2. `sese.023` `HldInd=true` ပါသော `sese.023` DvP ညွှန်ကြားချက်(များ) ဖြစ်သောကြောင့် ငွေသားအတည်ပြုချက်အတွက် ငွေသားလုံခြုံရေးသည် စောင့်ဆိုင်းနေပါသည်။  
3. Lifecycle `sese.024` သတိပေးချက်များ (လက်ခံသည်/ကိုက်ညီ/ဆိုင်းငံ့)။  
4. အကယ်၍ `pacs.009` ခြေထောက်နှစ်ချောင်းစလုံးသည် `ACSC` ရောက်ရှိသွားပါက ဝင်းဒိုးသက်တမ်းမကုန်မီ → `sese.030` → `sese.031` (mod status) → `sese.025` (အတည်ပြုချက်)။  
5. FX ဝင်းဒိုးကို ချိုးဖောက်ပါက → ငွေသားကို ပယ်ဖျက်/ပြန်သိမ်းခြင်း (`camt.056/029` သို့မဟုတ် `pacs.004`) နှင့် အာမခံများ (`sese.020` + `sese.027` သို့မဟုတ် `pacs.004`) ကို ပယ်ဖျက်ပါက စျေးကွက်စည်းမျဉ်းကို ပြန်လည်အတည်ပြုပြီးဖြစ်သည်။

#### လိုအပ်သော အသိအမှတ်ပြုမှုများ / အခြေအနေများ
- ငွေသား- `pacs.002` (`PDNG`၊ `ACSC`၊ `RJCT`), `pacs.004` ။  
- Securities- `sese.024` (`NORE`၊ `ADEA`), `sese.025` ကဲ့သို့သော ဆိုင်းငံ့ခြင်း/ပျက်ကွက်ခြင်းများ။  
- သယ်ယူပို့ဆောင်ရေး- `admi.007` / ဂိတ်ဝေးသည် လုပ်ငန်းမလုပ်ဆောင်မီ ငြင်းပယ်သည်။

#### အခြေအနေ/အကွက်များကို ဖြေလျှော့ပါ။
- `SttlmParams/HldInd` + `sese.030` အောင်မြင်ခြင်း/ကျရှုံးမှုတွင် ထုတ်ဝေခြင်း/ပယ်ဖျက်ခြင်း။  
- `Lnkgs` သည် ငွေသားခြေထောက်တွင် ငွေသားညွှန်ကြားချက်များကို ချိတ်ဆက်ရန်။  
- အခြေအနေအရပေးပို့ခြင်းကိုအသုံးပြုပါက T2S CoSD စည်းမျဉ်း။  
- `PrtlSttlmInd` မရည်ရွယ်သောအပိုင်းများကိုကာကွယ်ရန်။  
- `pacs.009` တွင် `CtgyPurp/Cd = SECU` တွင် ငွေချေးခြင်းဆိုင်ရာ ရန်ပုံငွေကို အလံပြထားသည်။

#### ကိုးကား
- အာမခံလုပ်ငန်းစဉ်များတွင် ငွေပေးချေမှုများအတွက် PMPG / CBPR+ လမ်းညွှန်ချက်။  
- SMPG အခြေချခြင်းအလေ့အကျင့်များ၊ ချိတ်ဆက်ခြင်း/ကိုင်ဆောင်မှုများဆိုင်ရာ T2S ထိုးထွင်းသိမြင်မှု။  
- Clearstream DCP လက်စွဲများ၊ ပြုပြင်ထိန်းသိမ်းမှုမက်ဆေ့ချ်များအတွက် ECMS စာရွက်စာတမ်းများ။

### pacs.004 ပြန်၍မြေပုံဆွဲမှတ်စုများ

- ယခု ပြန်ပေးသည့် ကိရိယာများသည် `ChrgBr` (`DEBT`/`CRED`/`SHAR`/`SLEV`) ကို ပုံမှန်ဖြစ်အောင် လုပ်ပြီး `SLEV` ကို ပုံမှန်ပြန်ပေးကာ I18NI00000 ဖြင့် ပြန်ဖွင့်နိုင်သည် ။ XML စာအိတ်ကို ပြန်လည်ခွဲခြမ်းစိတ်ဖြာခြင်းမရှိဘဲ ကုဒ်များ။
- `DataPDU` စာအိတ်များအတွင်းရှိ AppHdr လက်မှတ်လုပ်ကွက်များကို စားသုံးသည့်အခါတွင် လျစ်လျူရှုထားဆဲဖြစ်သည်။ စာရင်းစစ်များသည် ထည့်သွင်းထားသော XMLDSIG အကွက်များထက် ချန်နယ်သက်သေကို အားကိုးသင့်သည်။

### တံတားအတွက် လုပ်ငန်းလည်ပတ်မှုစာရင်း
- အထက်ဖော်ပြပါ ကကွက်ကို တွန်းအားပေးပါ (အပေါင်ပစ္စည်း- `colr.010/011/012 → sese.023/024/025`; FX ချိုးဖောက်မှု- `pacs.009 (+pacs.002) → sese.023 held → release/cancel`)။  
- `sese.024`/`sese.025` အခြေအနေများနှင့် `pacs.002` ရလဒ်များကို gating signals အဖြစ် ဆက်ဆံပါ။ `ACSC` အစပျိုးမှုများ၊ `RJCT` တွန်းအားပေးမှုများကို ဖြေလျှော့ပါ။  
- `HldInd`၊ `Lnkgs`၊ `PrtlSttlmInd`၊ `SttlmTxCond` နှင့် ရွေးချယ်နိုင်သော CoSD စည်းမျဉ်းများမှတစ်ဆင့် အခြေအနေအရ ပေးပို့မှုကို ကုဒ်လုပ်ပါ။  
- လိုအပ်သည့်အခါတွင် ပြင်ပ ID များ (ဥပမာ၊ `pacs.009` အတွက် UETR) ကို ဆက်စပ်ရန် `SupplementaryData` ကို သုံးပါ။  
- စျေးကွက်ပြက္ခဒိန် / ဖြတ်တောက်မှုများဖြင့်ကန့်သတ်ထိန်းချုပ်ခြင်း / အချိန်ကိုဖြေလျှော့ပါ။ ပယ်ဖျက်ခြင်းသတ်မှတ်ရက်မတိုင်မီ `sese.030`/`camt.056` ထုတ်ဝေမှု၊ လိုအပ်ပါက ပြန်လည်ပေးပို့ရန် ပြန်လှည့်ပါ။

### နမူနာ ISO 20022 ပေးချေမှုများ (မှတ်ချက်ပေးထားသော)

ညွှန်ကြားချက်လင့်ခ်ဖြင့် #### စရံအစားထိုးအတွဲ (`sese.023`)

```xml
<sese:Document xmlns:sese="urn:iso:std:iso:20022:tech:xsd:sese.023.001.11">
  <sese:SctiesSttlmTxInstr>
    <sese:TxId>SUBST-2025-04-001-A</sese:TxId>
    <sese:SttlmTpAndAddtlParams>
      <sese:SctiesMvmntTp>DELI</sese:SctiesMvmntTp>
      <sese:Pmt>FREE</sese:Pmt>
    </sese:SttlmTpAndAddtlParams>
    <sese:SttlmParams>
      <sese:HldInd>true</sese:HldInd>
      <sese:PrtlSttlmInd>NPAR</sese:PrtlSttlmInd>
      <sese:SttlmTxCond>
        <sese:Cd>NOMC</sese:Cd>
      </sese:SttlmTxCond>
    </sese:SttlmParams>
    <sese:Lnkgs>
      <sese:Lnkg>
        <sese:Tp>
          <sese:Cd>WITH</sese:Cd>
        </sese:Tp>
        <sese:Ref>
          <sese:Prtry>SUBST-2025-04-001-B</sese:Prtry>
        </sese:Ref>
      </sese:Lnkg>
    </sese:Lnkgs>
    <!-- Original collateral FoP back to giver -->
    <sese:FctvSttlmDt>2025-04-03</sese:FctvSttlmDt>
    <sese:SctiesMvmntDtls>
      <sese:SctiesId>
        <sese:ISIN>XS1234567890</sese:ISIN>
      </sese:SctiesId>
      <sese:Qty>
        <sese:QtyChc>
          <sese:Unit>1000</sese:Unit>
        </sese:QtyChc>
      </sese:Qty>
    </sese:SctiesMvmntDtls>
  </sese:SctiesSttlmTxInstr>
</sese:Document>
```

`SctiesMvmntTp=RECE`၊ `Pmt=FREE` နှင့် `Pmt=FREE` နှင့် `WITH` (အစားထိုးအပေါင်ပစ္စည်းလက်ခံခြင်း FoP လက်ခံခြင်း) နှင့် ချိတ်ဆက်ထားသော ညွှန်ကြားချက်ကို ပေးပို့ပါ၊ နှင့် `WITH` သို့ပြန်ညွှန်ပြသော လင့်ခ်ကို ပေးပို့ပါ။ အစားထိုးမှုကို အတည်ပြုပြီးသည်နှင့် ကိုက်ညီသော `sese.030` ဖြင့် ခြေထောက်နှစ်ဖက်လုံးကို လွှတ်ပေးပါ။

#### FX အတည်ပြုချက်ကို ဆိုင်းငံ့ထားပြီး Securities leg (`sese.023` + `sese.030`)

```xml
<sese:Document xmlns:sese="urn:iso:std:iso:20022:tech:xsd:sese.023.001.11">
  <sese:SctiesSttlmTxInstr>
    <sese:TxId>DVP-2025-05-CLS01</sese:TxId>
    <sese:SttlmTpAndAddtlParams>
      <sese:SctiesMvmntTp>DELI</sese:SctiesMvmntTp>
      <sese:Pmt>APMT</sese:Pmt>
    </sese:SttlmTpAndAddtlParams>
    <sese:SttlmParams>
      <sese:HldInd>true</sese:HldInd>
      <sese:PrtlSttlmInd>NPAR</sese:PrtlSttlmInd>
    </sese:SttlmParams>
    <sese:Lnkgs>
      <sese:Lnkg>
        <sese:Tp>
          <sese:Cd>WITH</sese:Cd>
        </sese:Tp>
        <sese:Ref>
          <sese:Prtry>PACS009-USD-CLS01</sese:Prtry>
        </sese:Ref>
      </sese:Lnkg>
    </sese:Lnkgs>
    <!-- Remaining settlement details omitted for brevity -->
  </sese:SctiesSttlmTxInstr>
</sese:Document>
````pacs.009` ခြေထောက်နှစ်ချောင်းစလုံး `ACSC` ရောက်သည်နှင့် လွှတ်ပါ။

```xml
<sese:Document xmlns:sese="urn:iso:std:iso:20022:tech:xsd:sese.030.001.04">
  <sese:SctiesSttlmCondModReq>
    <sese:ReqDtls>
      <sese:TxId>DVP-2025-05-CLS01</sese:TxId>
      <sese:ChngTp>
        <sese:Cd>RELE</sese:Cd>
      </sese:ChngTp>
    </sese:ReqDtls>
  </sese:SctiesSttlmCondModReq>
</sese:Document>
```

`sese.031` သည် အာမခံပစ္စည်းများကို ကြိုတင်စာရင်းသွင်းပြီးသည်နှင့် `sese.025` သည် ဆိုင်းငံ့ထားမှုကို အတည်ပြုသည်။

#### PvP ရန်ပုံငွေ ခြေထောက် (လုံခြုံရေး ရည်ရွယ်ချက်ဖြင့် `pacs.009`)

```xml
<pacs:Document xmlns:pacs="urn:iso:std:iso:20022:tech:xsd:pacs.009.001.08">
  <pacs:FinInstnCdtTrf>
    <pacs:GrpHdr>
      <pacs:MsgId>PACS009-USD-CLS01</pacs:MsgId>
      <pacs:IntrBkSttlmDt>2025-05-07</pacs:IntrBkSttlmDt>
    </pacs:GrpHdr>
    <pacs:CdtTrfTxInf>
      <pacs:PmtId>
        <pacs:InstrId>DVP-2025-05-CLS01-USD</pacs:InstrId>
        <pacs:EndToEndId>SETTLEMENT-CLS01</pacs:EndToEndId>
      </pacs:PmtId>
      <pacs:PmtTpInf>
        <pacs:CtgyPurp>
          <pacs:Cd>SECU</pacs:Cd>
        </pacs:CtgyPurp>
      </pacs:PmtTpInf>
      <pacs:IntrBkSttlmAmt Ccy="USD">5000000.00</pacs:IntrBkSttlmAmt>
      <pacs:InstgAgt>
        <pacs:FinInstnId>
          <pacs:BICFI>BANKUS33XXX</pacs:BICFI>
        </pacs:FinInstnId>
      </pacs:InstgAgt>
      <pacs:InstdAgt>
        <pacs:FinInstnId>
          <pacs:BICFI>BANKGB22XXX</pacs:BICFI>
        </pacs:FinInstnId>
      </pacs:InstdAgt>
      <pacs:SplmtryData>
        <pacs:Envlp>
          <nor:NoritoBridge xmlns:nor="urn:norito:settlement">
            <nor:SettlementId>DVP-2025-05-CLS01</nor:SettlementId>
            <nor:Atomicity>ALL_OR_NOTHING</nor:Atomicity>
          </nor:NoritoBridge>
        </pacs:Envlp>
      </pacs:SplmtryData>
    </pacs:CdtTrfTxInf>
  </pacs:FinInstnCdtTrf>
</pacs:Document>
```

`pacs.002` သည် ငွေပေးချေမှု အခြေအနေကို ခြေရာခံသည် (`ACSC` = အတည်ပြုပြီး၊ `RJCT` = ငြင်းပယ်ခြင်း)။ ဝင်းဒိုးကို ချိုးဖောက်ပါက၊ `camt.056`/`camt.029` မှတဆင့် ပြန်ခေါ်ပါ သို့မဟုတ် အခြေချထားသောငွေများကို ပြန်ပေးရန် `pacs.004` ကို ပေးပို့ပါ။