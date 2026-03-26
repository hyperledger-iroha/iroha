---
lang: my
direction: ltr
source: docs/portal/docs/sns/registrar-api.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 300ae819b5315b1ae4edab6caffc936e506c0d550a1ba75be1ace4d42f8e0b11
source_last_modified: "2026-01-28T17:11:30.701656+00:00"
translation_last_reviewed: 2026-02-07
id: registrar-api
title: Sora Name Service Registrar API & Governance Hooks
sidebar_label: Registrar API
description: Torii REST/gRPC surfaces, Norito DTOs, and governance artifacts for SNS registrations (SN-2b).
translator: machine-google-reviewed
---

::: Canonical Source ကို သတိပြုပါ။
ဤစာမျက်နှာသည် `docs/source/sns/registrar_api.md` ကို ရောင်ပြန်ဟပ်ပြီး ယခုအခါ စာမျက်နှာအဖြစ် ဆောင်ရွက်လျက်ရှိသည်။
canonical portal ကော်ပီ။ ဘာသာပြန်လုပ်ငန်းအသွားအလာအတွက် အရင်းအမြစ်ဖိုင်သည် ကျန်ရှိနေပါသည်။
:::

# SNS Registrar API နှင့် Governance Hooks (SN-2b)

**အခြေအနေ-** ရေးဆွဲထားသော 2026-03-24 -- Nexus Core ပြန်လည်သုံးသပ်မှုအောက်တွင်  
**လမ်းပြမြေပုံလင့်ခ်-** SN-2b “မှတ်ပုံတင်အရာရှိ API နှင့် အုပ်ချုပ်မှုချိတ်များ”  
**ကြိုတင်လိုအပ်ချက်များ-** [`registry-schema.md`](./registry-schema.md)

ဤမှတ်စုသည် Torii အဆုံးမှတ်များ၊ gRPC ဝန်ဆောင်မှုများ၊ တောင်းဆိုမှု/တုံ့ပြန်မှု DTOs နှင့် Sora အမည်ဝန်ဆောင်မှု (SNS) မှတ်ပုံတင်အရာရှိကို လုပ်ဆောင်ရန် လိုအပ်သော အုပ်ချုပ်မှုဆိုင်ရာပစ္စည်းများကို သတ်မှတ်ပေးပါသည်။ ၎င်းသည် SNS အမည်များကို စာရင်းသွင်းရန်၊ သက်တမ်းတိုးရန် သို့မဟုတ် စီမံခန့်ခွဲရန် လိုအပ်သော SDKs၊ ပိုက်ဆံအိတ်များနှင့် အလိုအလျောက်စနစ်အတွက် တရားဝင်စာချုပ်ဖြစ်သည်။

## 1. Transport & Authentication

| လိုအပ်ချက် | Detail |
|-------------|--------|
| ပရိုတိုကောများ | `/v1/sns/*` နှင့် gRPC ဝန်ဆောင်မှု `sns.v1.Registrar` အောက်တွင် အနားယူပါ။ Norito-JSON (`application/json`) နှင့် Norito-RPC binary (`application/x-norito`) ကို လက်ခံပါသည်။ |
| အထောက်အထား | `Authorization: Bearer` တိုကင်များ သို့မဟုတ် mTLS လက်မှတ်များကို နောက်ဆက်တွဲ ဘဏ္ဍာစိုးအလိုက် ထုတ်ပေးသည်။ အုပ်ချုပ်မှု-အထိခိုက်မခံသော အဆုံးမှတ်များ (အေးခဲ/ပိတ်၊ သိမ်းဆည်းထားသော တာဝန်များ) `scope=sns.admin` လိုအပ်သည်။ |
| ကန့်သတ်ချက်များ | မှတ်ပုံတင်သူများသည် `torii.preauth_scheme_limits` ပုံးများကို JSON ခေါ်ဆိုသူများ နှင့် နောက်ဆက်တွဲ ဆက်တိုက် ဆက်တိုက်ထုပ်ပိုးထားသည်- `sns.register`၊ `sns.renew`၊ `sns.controller`၊ `sns.freeze`။ |
| Telemetry | Torii သည် မှတ်ပုံတင်အရာရှိကိုင်တွယ်သူများအတွက် `torii_request_duration_seconds{scheme}` / `torii_request_failures_total{scheme,code}` ကိုဖော်ထုတ်သည် (`scheme="norito_rpc"`) API သည် `sns_registrar_status_total{result, suffix_id}` ကိုလည်း တိုးပေးသည်။ |

## 2. DTO ခြုံငုံသုံးသပ်ချက်

အကွက်များသည် [`registry-schema.md`](./registry-schema.md) တွင် သတ်မှတ်ထားသော canonical structs များကို ရည်ညွှန်းသည်။ မရေရာသောလမ်းကြောင်းကိုရှောင်ရှားရန် payload များအားလုံးသည် `NameSelectorV1` + `SuffixId` ကိုထည့်သွင်းထားသည်။

```text
Struct RegisterNameRequestV1 {
    selector: NameSelectorV1,
    owner: AccountId,
    controllers: Vec<NameControllerV1>,
    term_years: u8,                     // 1..=max_term_years
    pricing_class_hint: Option<u8>,     // steward-advertised tier id
    payment: PaymentProofV1,
    governance: GovernanceHookV1,
    metadata: Metadata,
}

Struct RegisterNameResponseV1 {
    name_record: NameRecordV1,
    registry_event: RegistryEventV1,
    revenue_accrual: RevenueAccrualEventV1,
}

Struct PaymentProofV1 {
    asset_id: AssetId,
    gross_amount: TokenValue,
    net_amount: TokenValue,
    settlement_tx: Hash,
    payer: AccountId,
    signature: Signature,               // steward/treasury cosign
}

Struct GovernanceHookV1 {
    proposal_id: String,
    council_vote_hash: Hash,
    dao_vote_hash: Hash,
    steward_ack: Signature,
    guardian_clearance: Option<Signature>,
}

Struct RenewNameRequestV1 {
    selector: NameSelectorV1,
    term_years: u8,
    payment: PaymentProofV1,
}

Struct TransferNameRequestV1 {
    selector: NameSelectorV1,
    new_owner: AccountId,
    governance: GovernanceHookV1,
}

Struct UpdateControllersRequestV1 {
    selector: NameSelectorV1,
    controllers: Vec<NameControllerV1>,
}

Struct FreezeNameRequestV1 {
    selector: NameSelectorV1,
    reason: String,
    until: Timestamp,
    guardian_ticket: Signature,
}

Struct ReservedAssignmentRequestV1 {
    selector: NameSelectorV1,
    reserved_label: ReservedNameV1,
    governance: GovernanceHookV1,
}
```

## 3. REST အဆုံးမှတ်များ

| အဆုံးမှတ် | နည်းလမ်း | ဝန်ဆောင်ခ | ဖော်ပြချက် |
|----------|--------|---------|-------------|
| `/v1/sns/names` | POST | `RegisterNameRequestV1` | အမည်စာရင်းသွင်းပါ သို့မဟုတ် ပြန်လည်ဖွင့်ပါ။ စျေးနှုန်းအဆင့်ကို ဖြေရှင်းပါ၊ ငွေပေးချေမှု/အုပ်ချုပ်မှုဆိုင်ရာ အထောက်အထားများကို အတည်ပြုပေးသည်၊ မှတ်ပုံတင်ခြင်းဆိုင်ရာ ဖြစ်ရပ်များကို ထုတ်လွှတ်ပါသည်။ |
| `/v1/sns/names/{namespace}/{literal}/renew` | POST | `RenewNameRequestV1` | သက်တမ်းတိုး။ မူဝါဒမှ ကျေးဇူးတော်/ရွေးနှုတ်မှု ပြတင်းပေါက်များကို တွန်းအားပေးသည်။ |
| `/v1/sns/names/{namespace}/{literal}/transfer` | POST | `TransferNameRequestV1` | အုပ်ချုပ်မှုအတည်ပြုချက်များ ပူးတွဲပါရှိပြီး ပိုင်ဆိုင်မှုလွှဲပြောင်းပါ။ |
| `/v1/sns/names/{namespace}/{literal}/controllers` | PUT | `UpdateControllersRequestV1` | ထိန်းချုပ်ကိရိယာအစုံကို အစားထိုးပါ။ လက်မှတ်ထိုးထားသော အကောင့်လိပ်စာများကို အတည်ပြုသည်။ |
| `/v1/sns/names/{namespace}/{literal}/freeze` | POST | `FreezeNameRequestV1` | အုပ်ထိန်းသူ/ကောင်စီကို ခေတ္တရပ်ဆိုင်းထားသည်။ အုပ်ထိန်းသူလက်မှတ်နှင့် အုပ်ချုပ်မှုဆိုင်ရာစာရွက်စာတမ်းများကို ကိုးကားရန် လိုအပ်သည်။ |
| `/v1/sns/names/{namespace}/{literal}/freeze` | ဖျက်ရန် | `GovernanceHookV1` | ပြုပြင်ပြီးနောက် အေးခဲမှုကို ပြန်ဖြုတ်ပါ။ မှတ်တမ်းတင်ထားသော ကောင်စီကို ထပ်လောင်းသေချာစေပါသည်။ |
| `/v1/sns/reserved/{selector}` | POST | `ReservedAssignmentRequestV1` | ဘဏ္ဍာစိုး/ကောင်စီ၏ သီးသန့်အမည်များကို တာဝန်ပေးသည်။ |
| `/v1/sns/policies/{suffix_id}` | GET | — | လက်ရှိ `SuffixPolicyV1` (cacheable) ကို ရယူပါ။ |
| `/v1/sns/names/{namespace}/{literal}` | GET | — | လက်ရှိ `NameRecordV1` + ထိရောက်မှုအခြေအနေ (တက်ကြွ၊ ကျေးဇူးတော်၊ စသည်) ကို ပြန်ပေးသည်။ |

**ရွေးချယ်မှုကုဒ်နံပါတ်-** `{selector}` လမ်းကြောင်းအပိုင်းသည် i105 (နှစ်သက်ရာ)၊ ချုံ့ထားသော (`sora`၊ ဒုတိယအကောင်းဆုံး) သို့မဟုတ် canonical hex အလိုက် ADDR-5၊ Torii သည် `NameSelectorV1` မှတစ်ဆင့် ပုံမှန်ဖြစ်စေသည်။

**အမှားပုံစံ-** အဆုံးမှတ်များအားလုံးသည် Norito JSON `code`၊ `message`၊ `details` ဖြင့် ပြန်ပေးသည်။ ကုဒ်များတွင် `sns_err_reserved`၊ `sns_err_payment_mismatch`၊ `sns_err_policy_violation`၊ `sns_err_governance_missing` ပါဝင်သည်။

### 3.1 CLI အကူအညီပေးသူများ (N0 လက်စွဲ မှတ်ပုံတင်အရာရှိ လိုအပ်ချက်)

Closed-beta ဘဏ္ဍာစိုးများသည် ယခုလက်ဖြင့်ပြုလုပ်ခြင်းမရှိဘဲ JSON ကို CLI မှတစ်ဆင့် မှတ်ပုံတင်အရာရှိအား ကျင့်သုံးနိုင်ပါပြီ-

```bash
iroha sns register \
  --label makoto \
  --suffix-id 1 \
  --term-years 2 \
  --payment-asset-id 61CtjvNd9T3THAR65GsMVHr82Bjc \
  --payment-gross 240 \
  --payment-settlement '"settlement-tx-hash"' \
  --payment-signature '"steward-signature"'
```

- `--owner` သည် CLI config အကောင့်သို့ ပုံသေများဖြစ်သည်။ နောက်ထပ် ထိန်းချုပ်ကိရိယာအကောင့်များ ပူးတွဲပါရန် `--controller` ကို ထပ်လုပ်ပါ (မူလ `[owner]`)။
- Inline ငွေပေးချေမှုအလံများကို `PaymentProofV1` သို့ တိုက်ရိုက်မြေပုံပြပါ။ သင့်တွင် ဖွဲ့စည်းတည်ဆောက်ထားသော ပြေစာတစ်ခုရှိသောအခါ `--payment-json PATH` ကို ကျော်ဖြတ်ပါ။ မက်တာဒေတာ (`--metadata-json`) နှင့် အုပ်ချုပ်မှုချိတ်များ (`--governance-json`) သည် တူညီသောပုံစံအတိုင်း လုပ်ဆောင်သည်။

အစမ်းလေ့ကျင့်မှုများကို ဖတ်ရန်သာ ကူညီပေးသူများ-

```bash
iroha sns registration --selector makoto.sora
iroha sns policy --suffix-id 1
```

အကောင်အထည်ဖော်မှုအတွက် `crates/iroha_cli/src/commands/sns.rs` ကိုကြည့်ပါ။ ညွှန်ကြားချက်များသည် ဤစာရွက်စာတမ်းတွင်ဖော်ပြထားသော Norito DTOs ကို ပြန်လည်အသုံးပြုသောကြောင့် CLI ရလဒ်သည် Torii တုံ့ပြန်မှုများ byte-for-byte နှင့် ကိုက်ညီပါသည်။

အပိုအကူအညီပေးသူများသည် သက်တမ်းတိုးခြင်း၊ လွှဲပြောင်းခြင်းနှင့် အုပ်ထိန်းခြင်းဆိုင်ရာ လုပ်ဆောင်ချက်များကို အကျုံးဝင်သည်-

```bash
# Renew an expiring name
iroha sns renew \
  --selector makoto.sora \
  --term-years 1 \
  --payment-asset-id 61CtjvNd9T3THAR65GsMVHr82Bjc \
  --payment-gross 120 \
  --payment-settlement '"renewal-settlement"' \
  --payment-signature '"steward-signature"'

# Transfer ownership once governance approves
iroha sns transfer \
  --selector makoto.sora \
  --new-owner <katakana-i105-account-id> \
  --governance-json /path/to/hook.json

# Freeze/unfreeze flows
iroha sns freeze \
  --selector makoto.sora \
  --reason "guardian investigation" \
  --until-ms 1750000000000 \
  --guardian-ticket '{"sig":"guardian"}'

iroha sns unfreeze \
  --selector makoto.sora \
  --governance-json /path/to/unfreeze_hook.json
```

`--governance-json` တွင် မှန်ကန်သော `GovernanceHookV1` မှတ်တမ်း (အဆိုပြုချက် ID၊ မဲနှိုက်ချက်များ၊ ဘဏ္ဍာစိုး/အုပ်ထိန်းသူလက်မှတ်များ) ပါဝင်ရပါမည်။ ညွှန်ကြားချက်တစ်ခုစီသည် သက်ဆိုင်ရာ `/v1/sns/names/{namespace}/{literal}/…` အဆုံးမှတ်ကို ထင်ဟပ်နေစေသောကြောင့် beta အော်ပရေတာများသည် SDKs ခေါ်ဆိုမည့် Torii မျက်နှာပြင်များကို အတိအကျ ပြန်လည်လေ့ကျင့်နိုင်ပါသည်။

## 4. gRPC ဝန်ဆောင်မှု

```text
service Registrar {
    rpc Register(RegisterNameRequestV1) returns (RegisterNameResponseV1);
    rpc Renew(RenewNameRequestV1) returns (NameRecordV1);
    rpc Transfer(TransferNameRequestV1) returns (NameRecordV1);
    rpc UpdateControllers(UpdateControllersRequestV1) returns (NameRecordV1);
    rpc Freeze(FreezeNameRequestV1) returns (NameRecordV1);
    rpc Unfreeze(GovernanceHookV1) returns (NameRecordV1);
    rpc AssignReserved(ReservedAssignmentRequestV1) returns (NameRecordV1);
    rpc GetRegistration(NameSelectorV1) returns (NameRecordV1);
    rpc GetPolicy(SuffixId) returns (SuffixPolicyV1);
}
```

Wire-format- compile-time Norito အောက်တွင် မှတ်တမ်းတင်ထားသော schema hash
`fixtures/norito_rpc/schema_hashes.json` (အတန်း `RegisterNameRequestV1`၊
`RegisterNameResponseV1`၊ `NameRecordV1` စသဖြင့်)။

## 5. အုပ်ချုပ်ရေးဆိုင်ရာချိတ်များနှင့် အထောက်အထားများ

ခေါ်ဆိုမှုတိုင်းတွင် ပြန်လည်ပြသရန်အတွက် သင့်လျော်သော အထောက်အထားများ ပူးတွဲပါရှိရမည်-

| အက် | လိုအပ်သော အုပ်ချုပ်မှုဒေတာ |
|--------|--------------------------------|
| ပုံမှန်စာရင်းသွင်း/သက်တမ်းတိုး | အခြေချမှုညွှန်ကြားချက်ကို ကိုးကားသော ငွေပေးချေမှုအထောက်အထား၊ ဘဏ္ဍာစိုး၏အတည်ပြုချက်မလိုအပ်ပါက ကောင်စီမဲပေးစရာမလိုပါ။ |
| ပရီမီယံအဆင့် မှတ်ပုံတင်ခြင်း / လက်ဝယ်ရှိတာဝန် | `GovernanceHookV1` အဆိုပြုချက်ကို ကိုးကားခြင်း ID + ဘဏ္ဍာစိုး အသိအမှတ်ပြုခြင်း။ |
| လွှဲပြောင်း | ကောင်စီမဲ hash + DAO အချက်ပြ hash; အငြင်းပွားမှုဖြေရှင်းခြင်းမှအစပြုသောအခါလွှဲပြောင်းသည့်အခါအုပ်ထိန်းသူရှင်းလင်းရေး။ |
| အေးခဲ/အအေးခံခြင်း | အုပ်ထိန်းသူလက်မှတ် လက်မှတ် နှင့် ကောင်စီကို ထပ်ရေးပါ (အအေးခံခြင်းမှ ရပ်စဲပါ)။ |

Torii သည် စစ်ဆေးခြင်းဖြင့် အထောက်အထားများကို စစ်ဆေးသည်-

1. အဆိုပြုချက် id သည် အုပ်ချုပ်မှုစာရင်းဇယား (`/v1/governance/proposals/{id}`) တွင်ရှိပြီး အဆင့်အတန်းမှာ `Approved` ဖြစ်သည်။
2. Hashes သည် မှတ်တမ်းတင်ထားသော ဆန္ဒမဲများနှင့် ကိုက်ညီပါသည်။
3. ဘဏ္ဍာစိုး/အုပ်ထိန်းသူ လက်မှတ်များသည် `SuffixPolicyV1` မှ မျှော်လင့်ထားသော အများသူငှာသော့များကို ရည်ညွှန်းပါသည်။

မအောင်မြင်သောစစ်ဆေးမှုများ `sns_err_governance_missing` ကို ပြန်ပေးသည်။

## 6. အလုပ်အသွားအလာနမူနာများ

### 6.1 စံသတ်မှတ်ချက် မှတ်ပုံတင်ခြင်း။

1. စျေးနှုန်း၊ ကျေးဇူးတော်နှင့် ရနိုင်သောအဆင့်များကို ရယူရန် ဖောက်သည် `/v1/sns/policies/{suffix_id}` ကို မေးမြန်းပါသည်။
2. သုံးစွဲသူသည် `RegisterNameRequestV1` ကို တည်ဆောက်သည်-
   - `selector` ကို နှစ်သက်သော i105 သို့မဟုတ် ဒုတိယအကောင်းဆုံး ဖိသိပ်မှု (`sora`) တံဆိပ်မှ ဆင်းသက်လာသည်။
   - `term_years` မူဝါဒဘောင်အတွင်း။
   - `payment` သည် ဘဏ္ဍာတိုက်/ဘဏ္ဍာစုခွဲခွဲလွှဲပြောင်းခြင်းကို ရည်ညွှန်းသည်။
3. Torii အတည်ပြုသည်-
   - အညွှန်းပုံမှန်ပြုလုပ်ခြင်း + သီးသန့်စာရင်း။
   - သက်တမ်း/စုစုပေါင်းစျေးနှုန်း `PriceTierV1`။
   - ငွေပေးချေမှု အထောက်အထား ပမာဏ >= တွက်ချက်ထားသော ဈေးနှုန်း + အခကြေးငွေ။
4. အောင်မြင်မှုအပေါ် Torii-
   - `NameRecordV1` ဆက်ရှိနေသည်
   - `RegistryEventV1::NameRegistered` ကို ထုတ်လွှတ်သည်။
   - `RevenueAccrualEventV1` ကို ထုတ်လွှတ်သည်။
   - မှတ်တမ်းအသစ် + ဖြစ်ရပ်များကို ပြန်ပေးသည်။

### 6.2 ကျေးဇူးတော်ကာလအတွင်း သက်တမ်းတိုးခြင်း။

ကျေးဇူးတော် သက်တမ်းတိုးခြင်းများတွင် စံတောင်းဆိုချက်နှင့် ပြစ်ဒဏ်ရှာဖွေခြင်း ပါဝင်သည်။

- Torii သည် `now` နှင့် `grace_expires_at` ကို စစ်ဆေးပြီး `SuffixPolicyV1` မှ အပိုကြေးဇယားများကို ပေါင်းထည့်သည်။
- ငွေပေးချေမှု အထောက်အထားတွင် အပိုကြေး အကျုံးဝင်ရပါမည်။ ပျက်ကွက် => `sns_err_payment_mismatch`။
- `RegistryEventV1::NameRenewed` သည် `expires_at` အသစ်ကို မှတ်တမ်းတင်သည်။

### 6.3 Guardian Freeze & Council Override

1. Guardian သည် လက်မှတ်ကိုးကားသည့်အဖြစ်အပျက် ID ဖြင့် `FreezeNameRequestV1` ကို တင်ပြသည်။
2. Torii သည် မှတ်တမ်းကို `NameStatus::Frozen` သို့ ရွှေ့ပြီး `NameFrozen` ကို ထုတ်လွှတ်သည်။
3. ပြန်လည်ပြင်ဆင်ပြီးနောက်၊ အော်ပရေတာသည် `/v1/sns/names/{namespace}/{literal}/freeze` ဖြင့် DELETE `GovernanceHookV1` ပေးပို့သည်။
4. Torii သည် override ကို အတည်ပြုပြီး `NameUnfrozen` ကို ထုတ်လွှတ်သည်။

## 7. Validation & Error Codes များ

| ကုတ် | ဖော်ပြချက် | HTTP |
|--------|----------------|------|
| `sns_err_reserved` | အညွှန်းကို သီးသန့် သို့မဟုတ် ပိတ်ဆို့ထားသည်။ | 409 |
| `sns_err_policy_violation` | သက်တမ်း၊ အဆင့် သို့မဟုတ် ထိန်းချုပ်သူ သတ်မှတ်မှုသည် မူဝါဒကို ချိုးဖောက်သည်။ | 422 |
| `sns_err_payment_mismatch` | ငွေပေးချေမှုအထောက်အထားတန်ဖိုး သို့မဟုတ် ပိုင်ဆိုင်မှုမတူညီပါ။ | ၄၀၂ |
| `sns_err_governance_missing` | လိုအပ်သော အုပ်ချုပ်မှုဆိုင်ရာ ပစ္စည်းများ ပျက်ကွက်/မမှန်ပါ။ | ၄၀၃ |
| `sns_err_state_conflict` | လက်ရှိဘဝစက်ဝန်းအခြေအနေတွင် လုပ်ဆောင်မှုကို ခွင့်မပြုပါ။ | 409 |

ကုဒ်များအားလုံးသည် `X-Iroha-Error-Code` နှင့် ဖွဲ့စည်းထားသော Norito JSON/NRPC စာအိတ်များမှတစ်ဆင့် ပေါ်လာသည်။

## 8. အကောင်အထည်ဖော်ရေးမှတ်စုများ

- Torii သည် `NameRecordV1.auction` အောက်တွင် ဆိုင်းငံ့လေလံပွဲများကို သိမ်းဆည်းထားပြီး `PendingAuction` တွင် တိုက်ရိုက်မှတ်ပုံတင်ရန် ကြိုးပမ်းမှုများကို ငြင်းပယ်သည်။
- ငွေပေးချေမှုအထောက်အထားများ Norito လယ်ဂျာဖြတ်ပိုင်းဖြတ်ပိုင်းများကို ပြန်သုံးပါ။ ဘဏ္ဍာတိုက်ဝန်ဆောင်မှုများသည် အထောက်အကူ APIs (`/v1/finance/sns/payments`) ကို ပံ့ပိုးပေးပါသည်။
- SDK များသည် ဤအဆုံးမှတ်များကို ပြင်းပြင်းထန်ထန် ရိုက်နှိပ်ထားသော အကူအညီများဖြင့် ချုပ်ထားသင့်ပြီး ပိုက်ဆံအိတ်များသည် ရှင်းလင်းပြတ်သားသော အမှားအယွင်း အကြောင်းရင်းများ (`ERR_SNS_RESERVED`၊ စသည်ဖြင့်) ကို တင်ပြနိုင်မည်ဖြစ်သည်။

## 9. နောက်အဆင့်များ

- Torii ကို SN-3 လေလံတင်ပြီးသည်နှင့် အမှန်တကယ် မှတ်ပုံတင်စာချုပ်သို့ လွှဲပြောင်းပါ။
- ဤ API ကိုရည်ညွှန်းသော SDK သီးသန့်လမ်းညွှန်များ (Rust/JS/Swift) ကို ထုတ်ဝေပါ။
- အုပ်ချုပ်မှုချိတ်ဆက်မှု အထောက်အထားအကွက်များသို့ အပြန်အလှန်လင့်ခ်များဖြင့် [`sns_suffix_governance_charter.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md) ကို တိုးချဲ့ပါ။