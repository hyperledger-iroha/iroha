---
lang: my
direction: ltr
source: docs/source/domain_endorsements.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7c337150e6de1efa9f9480ba8126ecd5ada4ed8ee7ee8b70a95fd7f6348f9016
source_last_modified: "2025-12-29T18:16:35.952418+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# ဒိုမိန်းထောက်ခံချက်များ

ဒိုမိန်းထောက်ခံချက်များသည် အော်ပရေတာများအား ကော်မီတီမှ လက်မှတ်ရေးထိုးထားသော ထုတ်ပြန်ချက်တစ်ခုအောက်တွင် ဒိုမိန်းဖန်တီးမှုကို တံခါးပိတ်ပြီး ပြန်သုံးခွင့်ပေးသည်။ ထောက်ခံချက်ပေးချေမှုမှာ ကွင်းဆက်တွင်မှတ်တမ်းတင်ထားသော Norito အရာဝတ္ထုတစ်ခုဖြစ်ပြီး သုံးစွဲသူများသည် မည်သည့်ဒိုမိန်းနှင့် မည်သည့်အချိန်ကို အတည်ပြုကြောင်း စစ်ဆေးနိုင်သည်။

## Payload ပုံသဏ္ဍာန်

- `version`: `DOMAIN_ENDORSEMENT_VERSION_V1`
- `domain_id`- canonical domain identifier
- `committee_id`- လူသားဖတ်နိုင်သော ကော်မတီတံဆိပ်
- `statement_hash`: `Hash::new(domain_id.to_string().as_bytes())`
- `issued_at_height` / `expires_at_height`- ဘလောက်အမြင့်များ အကျုံးဝင်မှု
- `scope`- ရွေးချယ်နိုင်သောဒေတာနေရာနှင့် **လက်ခံခြင်းဘလောက်အမြင့်ကို ကာမိစေမည့် ရွေးချယ်နိုင်သော `[block_start, block_end]` ဝင်းဒိုးတစ်ခု (ပါဝင်သော) (ပါဝင်သည်)
- `signatures`- `body_hash()` ကျော်လက်မှတ်များ (`signatures = []` ဖြင့် ထောက်ခံချက်)
- `metadata`- ရွေးချယ်နိုင်သော Norito မက်တာဒေတာ (အဆိုပြုချက် အိုင်ဒီများ၊ စာရင်းစစ်လင့်ခ်များ စသည်ဖြင့်)

## ပြဋ္ဌာန်းခြင်း။

- Nexus ကိုဖွင့်ပြီး `nexus.endorsement.quorum > 0` သို့မဟုတ် ဒိုမိန်းတစ်ခုချင်းမူဝါဒက လိုအပ်ချက်အရ ဒိုမိန်းကို အမှတ်အသားပြုသည့်အခါ ထောက်ခံချက်လိုအပ်သည်။
- မှန်ကန်ကြောင်း အတည်ပြုခြင်းသည် ဒိုမိန်း/ထုတ်ပြန်ချက် hash binding၊ ဗားရှင်း၊ ပိတ်ဆို့ဝင်းဒိုး၊ dataspace အဖွဲ့ဝင်မှု၊ သက်တမ်းကုန်ဆုံးမှု/အသက်နှင့် ကော်မတီအထမြောက်သည်။ လက်မှတ်ထိုးသူများသည် `Endorsement` အခန်းကဏ္ဍနှင့်အတူ တိုက်ရိုက်သဘောတူညီမှု သော့များ ရှိရပါမည်။ ပြန်လည်ပြသခြင်းကို `body_hash` မှ ပယ်ချပါသည်။
- ဒိုမိန်းမှတ်ပုံတင်ခြင်းတွင် ပူးတွဲပါရှိသည့် ထောက်ခံချက်များသည် မက်တာဒေတာကီး `endorsement` ကို အသုံးပြုသည်။ ဒိုမိန်းအသစ်ကို စာရင်းမသွင်းဘဲ စာရင်းစစ်ခြင်းအတွက် ထောက်ခံချက်များကို မှတ်တမ်းတင်သည့် `SubmitDomainEndorsement` ညွှန်ကြားချက်ဖြင့် တူညီသော တရားဝင်လမ်းကြောင်းကို အသုံးပြုပါသည်။

## ကော်မတီများနှင့် မူဝါဒများ

- ကော်မီတီများကို ကွင်းဆက် (`RegisterDomainCommittee`) သို့မဟုတ် ပြင်ဆင်သတ်မှတ်မှု ပုံသေများ (`nexus.endorsement.committee_keys` + `nexus.endorsement.quorum`၊ id = `default`) မှ ဆင်းသက်လာနိုင်သည်။
- Per-domain မူဝါဒများကို `SetDomainEndorsementPolicy` (ကော်မတီ ID၊ `max_endorsement_age`၊ `required` အလံ) မှတဆင့် စီစဉ်သတ်မှတ်ထားပါသည်။ မရှိသည့်အခါ၊ Nexus မူရင်းများကို အသုံးပြုပါသည်။

## CLI အကူအညီပေးသူများ

- ထောက်ခံချက်တစ်ခုတည်ဆောက်/လက်မှတ်ထိုးပါ (Norito JSON ကို stdout သို့ထုတ်ပေးသည်)

  ```
  iroha endorsement prepare \
    --domain wonderland \
    --committee-id default \
    --issued-at-height 5 \
    --expires-at-height 25 \
    --block-start 5 \
    --block-end 15 \
    --signer-key <PRIVATE_KEY> --signer-key <PRIVATE_KEY>
  ```

- ထောက်ခံချက် တင်သွင်းပါ-

  ```
  iroha endorsement submit --file endorsement.json
  # or: cat endorsement.json | iroha endorsement submit
  ```

- စီမံအုပ်ချုပ်မှု-
  - `iroha endorsement register-committee --committee-id jdga --quorum 2 --member <PK> --member <PK> [--metadata path]`
  - `iroha endorsement set-policy --domain wonderland --committee-id jdga --max-endorsement-age 1000 --required`
  - `iroha endorsement policy --domain wonderland`
  - `iroha endorsement committee --committee-id jdga`
  - `iroha endorsement list --domain wonderland`

အတည်ပြုခြင်း ပျက်ကွက်မှုများသည် တည်ငြိမ်သော အမှားအယွင်းလိုင်းများ (အထွတ်အထ မကိုက်ညီမှု၊ ဟောင်းနွမ်းမှု/သက်တမ်းကုန်သွားသည့် ထောက်ခံချက်၊ နယ်ပယ်မတူညီမှု၊ အမည်မသိ ဒေတာနေရာလွတ်၊ ပျောက်ဆုံးနေသော ကော်မတီ) ကို ပြန်ပေးသည်။