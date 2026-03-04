---
lang: my
direction: ltr
source: docs/source/contract_deployment.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 0f2b1d7d027d715eac5a3ca8be29dea8f0e76013e948947a4de66108ac561f34
source_last_modified: "2026-01-22T14:58:53.689594+00:00"
translation_last_reviewed: 2026-02-07
title: Contract Deployment (.to) — API & Workflow
translator: machine-google-reviewed
---

အခြေအနေ- Torii၊ CLI နှင့် ပင်မဝင်ခွင့်စာမေးပွဲများ (နိုဝင်ဘာ 2025) ဖြင့် အကောင်အထည်ဖော်ပြီး ကျင့်သုံးသည်။

## ခြုံငုံသုံးသပ်ချက်

- ပြုစုထားသော IVM bytecode (`.to`) ကို Torii သို့ ပေးပို့ခြင်းဖြင့် သို့မဟုတ် ထုတ်ပေးခြင်းဖြင့် အသုံးပြုပါ။
  `RegisterSmartContractCode`/`RegisterSmartContractBytes` ညွှန်ကြားချက်များ
  တိုက်ရိုက်
- Nodes များသည် `code_hash` နှင့် canonical ABI hash ကို စက်တွင်းတွင် ပြန်လည်တွက်ချက်သည်။ မကိုက်ညီပါ။
  ပြတ်ပြတ်သားသား ငြင်းပယ်ပါ။
- သိမ်းဆည်းထားသော ရှေးဟောင်းပစ္စည်းများသည် ကွင်းဆက် `contract_manifests` အောက်တွင် လည်းကောင်း၊
  `contract_code` မှတ်ပုံတင်မှုများ။ အကိုးအကား hashe များကိုသာ ဖော်ပြပြီး သေးငယ်နေပါသည်။
  ကုဒ်ဘိုက်များကို `code_hash` ဖြင့် သော့ခတ်ထားသည်။
- Protected namespaces သည် a မတိုင်မီတွင် အတည်ပြုထားသော အုပ်ချုပ်မှုအဆိုပြုချက်တစ်ခု လိုအပ်နိုင်သည်။
  ဖြန့်ကျက်ခြင်းကို လက်ခံပါသည်။ ဝင်ခွင့်လမ်းကြောင်းသည် အဆိုပြုချက်ပါသော ဝန်ဆောင်ခကို ကြည့်ရှုပြီးဖြစ်သည်။
  `(namespace, contract_id, code_hash, abi_hash)` သည် တန်းတူညီမျှမှုကို ပြဋ္ဌာန်းသည့်အခါ
  namespace ကိုကာကွယ်ထားသည်။

## သိမ်းဆည်းထားသော ပစ္စည်းများနှင့် သိမ်းဆည်းခြင်း။

- `RegisterSmartContractCode` ပေးထားသော မန်နီးဖက်စ်ကို ထည့်သွင်း/ရေးသည်
  `code_hash`။ တူညီသော hash ရှိနေသောအခါ၊ ၎င်းကို အသစ်ဖြင့် အစားထိုးသည်။
  ထင်ရှားသည်။
- `RegisterSmartContractBytes` သည် စုစည်းထားသော ပရိုဂရမ်ကို အောက်တွင် သိမ်းဆည်းထားသည်။
  `contract_code[code_hash]`။ hash တစ်ခုအတွက် bytes ရှိပြီးသားဖြစ်ပါက ၎င်းတို့သည် တူညီရပါမည်။
  အတိအကျ; မတူညီသော bytes သည် မူကွဲချိုးဖောက်မှုကို မြှင့်တင်သည်။
- ကုဒ်အရွယ်အစားကို စိတ်ကြိုက်ကန့်သတ်ဘောင် `max_contract_code_bytes` ဖြင့် ကန့်သတ်ထားသည်။
  (မူလ 16 MiB)။ ယခင်က `SetParameter(Custom)` ဖြင့် ငွေပေးငွေယူ အစားထိုးပါ။
  ပိုကြီးသော ပစ္စည်းများ စာရင်းသွင်းခြင်း။
- ထိန်းသိမ်းခြင်းသည် အကန့်အသတ်မရှိပါ- မန်နီးဖက်စ်များနှင့် ကုဒ်များကို ပြတ်သားစွာ ရရှိနိုင်သည်အထိ ဆက်လက်ရှိနေပါသည်။
  အနာဂတ် အုပ်ချုပ်မှုလုပ်ငန်းခွင်တွင် ဖယ်ရှားခဲ့သည်။ TTL သို့မဟုတ် အလိုအလျောက် GC မရှိပါ။

## ဝင်ခွင့်ပိုက်လိုင်း

- တရားဝင်သူသည် IVM ခေါင်းစီးကို ခွဲခြမ်းစိပ်ဖြာပြီး `version_major == 1` ကို ပြဋ္ဌာန်းပြီး စစ်ဆေးမှုများ ပြုလုပ်သည်
  `abi_version == 1`။ အမည်မသိဗားရှင်းများသည် ချက်ခြင်းငြင်းဆိုသည်။ runtime မရှိပါ။
  ပြောင်းရန်။
- `code_hash` အတွက် မန်နီးဖက်စ်တစ်ခု ရှိနေသောအခါ၊ မှန်ကန်ကြောင်း သေချာစေသည်
  သိမ်းဆည်းထားသော `code_hash`/`abi_hash` သည် တင်ပြထားသည့် တွက်ချက်ထားသော တန်ဖိုးများနှင့် ညီမျှသည်
  အစီအစဉ်။ မကိုက်ညီမှုသည် `Manifest{Code,Abi}HashMismatch` အမှားများကို ဖြစ်ပေါ်စေသည်။
- ကာကွယ်ထားသော namespace များကို ပစ်မှတ်ထားသော ငွေလွှဲမှုများတွင် မက်တာဒေတာသော့များ ပါဝင်ရပါမည်။
  `gov_namespace` နှင့် `gov_contract_id`။ ဝင်ခွင့်လမ်းကြောင်းက သူတို့နဲ့ နှိုင်းယှဉ်တယ်။
  ပြဋ္ဌာန်းထားသော `DeployContract` အဆိုပြုချက်များကို ဆန့်ကျင်ခြင်း၊ ကိုက်ညီသော အဆိုပြုချက်မရှိပါက၊
  ငွေပေးငွေယူကို `NotPermitted` ဖြင့် ပယ်ချပါသည်။

## Torii အဆုံးမှတ်များ (အင်္ဂါရပ် `app_api`)- `POST /v1/contracts/deploy`
  - တောင်းဆိုချက်ကိုယ်ထည်- `DeployContractDto` (အကွက်အသေးစိတ်အတွက် `docs/source/torii_contracts_api.md` ကိုကြည့်ပါ)။
  - Torii သည် base64 payload ကို ကုဒ်လုပ်သည်၊ hash နှစ်ခုလုံးကိုတွက်ချက်သည်၊ manifest ကိုတည်ဆောက်သည်၊
    နှင့် `RegisterSmartContractCode` ပေါင်း တင်သွင်းသည်။
    `RegisterSmartContractBytes` ကိုယ်စား လက်မှတ်ရေးထိုးထားသော ငွေပေးငွေယူ
    ခေါ်ဆိုသူ။
  - တုံ့ပြန်မှု- `{ ok, code_hash_hex, abi_hash_hex }`။
  - အမှားများ- မမှန်ကန်သော base64၊ ပံ့ပိုးမထားသော ABI ဗားရှင်း၊ ခွင့်ပြုချက်ပျောက်ဆုံးခြင်း။
    (`CanRegisterSmartContractCode`)၊ အရွယ်အစား ဦးထုပ်ကျော်လွန်သွားသည်၊ အုပ်ချုပ်မှုဂိတ်။
- `POST /v1/contracts/code`
  - `RegisterContractCodeDto` (အခွင့်အာဏာ၊ သီးသန့်သော့၊ မန်နီးဖက်စ်) ကို လက်ခံပြီး တင်သွင်းခြင်းသာ
    `RegisterSmartContractCode`။ မန်နီးဖက်စ်များကို သီးခြားစီပြုလုပ်သောအခါတွင် အသုံးပြုပါ။
    bytecode
- `POST /v1/contracts/instance`
  - `DeployAndActivateInstanceDto` (အခွင့်အာဏာ၊ ကိုယ်ရေးကိုယ်တာသော့၊ namespace/contract_id၊ `code_b64`၊ ရွေးချယ်နိုင်သော manifest overrides) ကို လက်ခံပြီး ဖြန့်ကျက် + လုပ်ဆောင်သည်။
- `POST /v1/contracts/instance/activate`
  - `ActivateInstanceDto` (အခွင့်အာဏာ၊ သီးသန့်သော့၊ namespace၊ contract_id၊ `code_hash`) ကိုလက်ခံပြီး activation instruction ကိုသာတင်ပြပါသည်။
- `GET /v1/contracts/code/{code_hash}`
  - `{ manifest: { code_hash, abi_hash } }` ပြန်ပေးသည်။
    နောက်ထပ် မန်နီးဖက်စ်အကွက်များကို အတွင်းပိုင်း၌ ထိန်းသိမ်းထားသော်လည်း ဤနေရာတွင် ချန်လှပ်ထားသည်။
    တည်ငြိမ်သော API။
- `GET /v1/contracts/code-bytes/{code_hash}`
  - Base64 အဖြစ် သိမ်းဆည်းထားသော `.to` ပုံဖြင့် `{ code_b64 }` ကို ပြန်ပေးသည်။

စာချုပ်သက်တမ်းစက်ဝန်း အဆုံးမှတ်များအားလုံးသည် သီးသန့်သတ်မှတ်ထားသော ကန့်သတ်ချက်တစ်ခုမှတစ်ဆင့် စီစဉ်သတ်မှတ်ပေးထားသည်။
`torii.deploy_rate_per_origin_per_sec` (တစ်စက္ကန့်လျှင်တိုကင်များ) နှင့်
`torii.deploy_burst_per_origin` (ဆက်တိုက် တိုကင်များ)။ ပုံသေများသည် 4 req/s ၏ ဆက်တိုက်ဖြစ်သည်။
`X-API-Token`၊ အဝေးထိန်း IP သို့မဟုတ် အဆုံးမှတ် အရိပ်အမြွက်မှ ဆင်းသက်လာသော တိုကင်/ကီးတစ်ခုစီအတွက် 8။
ယုံကြည်ရသော အော်ပရေတာများအတွက် ကန့်သတ်ချက်အား ပိတ်ရန် အကွက်ကို `null` သို့ သတ်မှတ်ပါ။ ဟို
limiter fires သည် Torii ကိုတိုးစေသည်။
`torii_contract_throttled_total{endpoint="code|deploy|instance|activate"}` telemetry ကောင်တာနှင့်
HTTP 429 ကို ပြန်ပေးသည်၊ ကိုင်တွယ်သူ error တစ်ခုခုကို တိုးမြင့်လိုက်ပါ။
သတိပေးချက်အတွက် `torii_contract_errors_total{endpoint=…}`။

## အုပ်ချုပ်မှုပေါင်းစပ်ခြင်းနှင့် ကာကွယ်ထားသော အမည်နေရာများ- စိတ်ကြိုက်သတ်မှတ်ချက် `gov_protected_namespaces` (namespace ၏ JSON အခင်းအကျင်းကို သတ်မှတ်ပါ
  ဝင်ခွင့်တံခါးကိုဖွင့်ရန် strings)။ Torii သည် အကူအညီပေးသူများကို အောက်တွင်ဖော်ပြထားသည်။
  `/v1/gov/protected-namespaces` နှင့် CLI တို့က ၎င်းတို့ကို အလင်းပြန်ပေးသည်။
  `iroha_cli app gov protected set`/`iroha_cli app gov protected get`။
- `ProposeDeployContract` (သို့မဟုတ် Torii ဖြင့် ဖန်တီးထားသော အဆိုပြုချက်များ
  `/v1/gov/proposals/deploy-contract` အဆုံးမှတ်) ဖမ်းယူ
  `(namespace, contract_id, code_hash, abi_hash, abi_version)`။
- ပြည်လုံးကျွတ်ဆန္ဒခံယူပွဲပြီးသည်နှင့်၊ `EnactReferendum` သည် အဆိုပြုချက်ကို အတည်ပြုပြီးဖြစ်သည်၊
  ဝင်ခွင့်သည် ကိုက်ညီသော မက်တာဒေတာနှင့် ကုဒ်များပါရှိသော ဖြန့်ကျက်မှုများကို လက်ခံပါမည်။
- ငွေပေးငွေယူများတွင် မက်တာဒေတာအတွဲ `gov_namespace=a namespace` နှင့် ပါဝင်ရပါမည်။
  `gov_contract_id=an identifier` (နှင့် `contract_namespace` / သတ်မှတ်သင့်သည်၊
  ခေါ်ဆိုမှုအချိန်ချိတ်ဆက်မှုအတွက် `contract_id`)။ CLI အကူအညီပေးသူများသည် ၎င်းတို့ကို ဖြည့်သွင်းသည်။
  `--namespace`/`--contract-id` ကျော်သွားသောအခါ အလိုအလျောက်။
- ကာကွယ်ထားသော namespaces ကိုဖွင့်ထားသောအခါ၊ တန်းစီခြင်းဝင်ခွင့်သည် ကြိုးပမ်းမှုများကို ငြင်းပယ်သည်။
  ရှိပြီးသား `contract_id` ကို မတူညီသော namespace သို့ ပြန်လည်ပေါင်းစည်းပါ။ ပြဋ္ဌာန်းချက်ကို အသုံးပြုပါ။
  အဆိုပြုချက် သို့မဟုတ် အခြားနေရာတွင် ဖြန့်ကျက်ခြင်းမပြုမီ ယခင်နှောင်ကြိုးကို အနားယူပါ။
- အကယ်၍ လမ်းကြောမှ သက်သေပြချက်တစ်ခုအပေါ်ရှိ စိစစ်ရေးအစီအမံကို သတ်မှတ်ပါက၊ ပါဝင်ပါ။
  `gov_manifest_approvers` (တရားဝင်စစ်ဆေးသည့်အကောင့် IDs များ၏ JSON အခင်းအကျင်းများ) ထို့ကြောင့် တန်းစီခြင်းကို ရေတွက်နိုင်သည်
  အရောင်းအ၀ယ်လုပ်ပိုင်ခွင့်အာဏာနှင့်အတူ နောက်ထပ်အတည်ပြုချက်များ။ လမ်းသွယ်တွေကလည်း ငြင်းတယ်။
  မန်နီးဖက်စ်တွင် မပါဝင်သည့် namespace များကို ကိုးကားသည့် မက်တာဒေတာ
  `protected_namespaces` အစုံ။

## CLI ကူညီပေးသူများ

- `iroha_cli app contracts deploy --authority <id> --private-key <hex> --code-file <path>`
  Torii ကို အသုံးချရန် တောင်းဆိုချက် (တွက်ချက်ခြင်း ဟက်ရ်ှများ) ကို တင်သွင်းသည်။
- `iroha_cli app contracts deploy-activate --authority <id> --private-key <hex> --namespace <ns> --contract-id <id> --code-file <path>`
  မန်နီးဖက်စ်ကို တည်ဆောက်သည် (ပံ့ပိုးပေးသောသော့ဖြင့် ရေးထိုးထားသည်)၊ မှတ်ပုံတင်ထားသော ဘိုက် + မန်နီးဖက်စ်၊
  ငွေပေးငွေယူတစ်ခုတွင် `(namespace, contract_id)` စည်းနှောင်မှုကို အသက်သွင်းသည်။ သုံးပါ။
  `--dry-run` မပါဘဲ တွက်ချက်ထားသော hashes နှင့် instruction count ကို print ထုတ်ရန်
  တင်သွင်းခြင်းနှင့် `--manifest-out` လက်မှတ်ရေးထိုးထားသော manifest JSON ကို သိမ်းဆည်းရန်။
- `iroha_cli app contracts manifest build --code-file <path> [--sign-with <hex>]` ကွန်ပျူတာများ
  `code_hash`/`abi_hash` ပြုစုထားသော `.to` အတွက် မန်နီးဖက်စ်ကို ရွေးချယ်နိုင်သည်၊
  JSON ပုံနှိပ်ခြင်း သို့မဟုတ် `--out` သို့ စာရေးပါ။
- `iroha_cli app contracts simulate --authority <id> --private-key <hex> --code-file <path> --gas-limit <u64>`
  အော့ဖ်လိုင်း VM pass ကို run ပြီး ABI/hash metadata နှင့် တန်းစီထားသော ISI များကို အစီရင်ခံသည်။
  ကွန်ရက်ကို မထိဘဲ (ရေတွက်ခြင်းနှင့် ညွှန်ကြားချက် အိုင်ဒီများ)။ တွဲပါ။
  ခေါ်ဆိုမှုအချိန် မက်တာဒေတာကို ထင်ဟပ်စေရန် `--namespace/--contract-id`။
- `iroha_cli app contracts manifest get --code-hash <hex>` သည် Torii မှတစ်ဆင့် မန်နီးဖက်စ်ကို ရယူသည်
  ၎င်းကို disk သို့ စိတ်ကြိုက်ရေးနိုင်သည် ။
- `iroha_cli app contracts code get --code-hash <hex> --out <path>` ဒေါင်းလုဒ်များ
  သိမ်းဆည်းထားသော `.to` ပုံ။
- `iroha_cli app contracts instances --namespace <ns> [--table]` စာရင်းများကို ရပါပြီ။
  စာချုပ်ဖြစ်ရပ်များ (မန်နီးဖက်စ် + မက်တာဒေတာကို မောင်းနှင်ထားသည်)။
- အုပ်ချုပ်မှုအထောက်အကူပြုသူများ (`iroha_cli app gov deploy propose`၊ `iroha_cli app gov enact`၊
  `iroha_cli app gov protected set/get`) ကာကွယ်ထားသော-namespace အလုပ်အသွားအလာကို စီမံဆောင်ရွက်ပေးသည်။
  စာရင်းစစ်ခြင်းအတွက် JSON လက်ရာများကို ဖော်ထုတ်ပါ။

## စမ်းသပ်ခြင်းနှင့် အကျုံးဝင်ခြင်း။

- `crates/iroha_core/tests/contract_code_bytes.rs` ကာဗာကုဒ်အောက်တွင် ယူနစ်စစ်ဆေးမှုများ
  သိုလှောင်မှု၊ ချို့တဲ့မှုနှင့် အရွယ်အစားဦးထုပ်။
- `crates/iroha_core/tests/gov_enact_deploy.rs` မှတဆင့် manifest ထည့်သွင်းမှုကို အတည်ပြုသည်။
  enactment နှင့် `crates/iroha_core/tests/gov_protected_gate.rs` လေ့ကျင့်ခန်းများ
  protected-namespace ဝင်ခွင့် အဆုံးမှ အဆုံးထိ။
- Torii လမ်းကြောင်းများတွင် တောင်းဆိုမှု/တုံ့ပြန်မှု ယူနစ်စစ်ဆေးမှုများ ပါဝင်ပြီး CLI အမိန့်များ ပါ၀င်သည်
  JSON အသွားအပြန် တည်ငြိမ်မှုရှိစေမည့် ပေါင်းစပ်စစ်ဆေးမှုများ။

အသေးစိတ်ဆန္ဒခံယူပွဲလုပ်အားခများနှင့် အသေးစိတ်အချက်အလက်များအတွက် `docs/source/governance_api.md` ကို ကိုးကားပါ။
မဲစနစ်များ။