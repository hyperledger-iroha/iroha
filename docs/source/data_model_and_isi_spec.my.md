---
lang: my
direction: ltr
source: docs/source/data_model_and_isi_spec.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 55ac770cf80229c23d6067ef1ab312422c76fb928a08e8cad8c040bdab396016
source_last_modified: "2026-01-28T18:22:38.873410+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Iroha v2 ဒေတာမော်ဒယ်နှင့် ISI — အကောင်အထည်ဖော်မှုမှရရှိလာသော Spec

ဤသတ်မှတ်ချက်သည် ဒီဇိုင်းပြန်လည်သုံးသပ်ခြင်းကို အထောက်အကူဖြစ်စေရန်အတွက် `iroha_data_model` နှင့် `iroha_core` တစ်လျှောက် လက်ရှိအကောင်အထည်ဖော်မှုမှ ပြောင်းပြန်အင်ဂျင်နီယာဖြစ်သည်။ backticks ရှိ လမ်းကြောင်းများသည် တရားဝင်ကုဒ်ကို ညွှန်ပြသည်။

## နယ်ပယ်
- Canonical entities (ဒိုမိန်းများ၊ အကောင့်များ၊ ပိုင်ဆိုင်မှုများ၊ NFTs၊ အခန်းကဏ္ဍများ၊ ခွင့်ပြုချက်များ၊ လုပ်ဖော်ကိုင်ဖက်များ၊ အစပျိုးမှုများ) နှင့် ၎င်းတို့၏ ခွဲခြားသတ်မှတ်မှုများကို သတ်မှတ်သည်။
- ပြည်နယ်-ပြောင်းလဲခြင်းဆိုင်ရာ ညွှန်ကြားချက်များ (ISI) ကို ဖော်ပြသည်- အမျိုးအစားများ၊ ကန့်သတ်ချက်များ၊ ကြိုတင်အခြေအနေများ၊ ပြည်နယ်အကူးအပြောင်းများ၊ ထုတ်လွှတ်သည့် ဖြစ်ရပ်များနှင့် အမှားအယွင်းအခြေအနေများ။
- ကန့်သတ်စီမံခန့်ခွဲမှု၊ အရောင်းအ၀ယ်များနှင့် ညွှန်ကြားချက်အမှတ်စဉ်များကို အကျဉ်းချုပ်ဖော်ပြသည်။

အဆုံးအဖြတ်ပေးခြင်း- ညွှန်ကြားချက်ဆိုင်ရာ ဝေါဟာရများ အားလုံးသည် ဟာ့ဒ်ဝဲကို မှီခိုသည့် အပြုအမူမရှိဘဲ သန့်စင်သော အခြေအနေသို့ ကူးပြောင်းခြင်း ဖြစ်သည်။ Serialization သည် Norito ကို အသုံးပြုသည်။ VM bytecode သည် IVM ကိုအသုံးပြုပြီး on-chain မလုပ်ဆောင်မီတွင် host-side ကို တရားဝင်အတည်ပြုထားသည်။

---

## အကြောင်းအရာများနှင့် သတ်မှတ်ချက်များ
ID များသည် `Display`/`FromStr` ဖြင့် အသွားအပြန် တည်ငြိမ်သော စာကြောင်းပုံစံများ ရှိသည်။ အမည်စည်းမျဉ်းများသည် နေရာလွတ်နှင့် သီးသန့် `@ # $` စာလုံးများကို တားမြစ်ထားသည်။- `Name` — တရားဝင်အတည်ပြုထားသော စာသားအမှတ်အသား။ စည်းမျဉ်းများ- `crates/iroha_data_model/src/name.rs`။
- `DomainId` — `name`။ ဒိုမိန်း- `{ id, logo, metadata, owned_by }`။ တည်ဆောက်သူများ- `NewDomain`။ ကုဒ်- `crates/iroha_data_model/src/domain.rs`။
- `AccountId` — Canonical လိပ်စာများကို `AccountAddress` (IH58 / `sora…` compressed / hex) နှင့် Torii သည် `AccountAddress::parse_encoded` မှတဆင့် သွင်းအားများကို ပုံမှန်ဖြစ်စေသည်။ IH58 သည် ဦးစားပေးအကောင့်ဖော်မတ်ဖြစ်သည်။ `sora…` ဖောင်သည် Sora-only UX အတွက် ဒုတိယအကောင်းဆုံးဖြစ်သည်။ အကျွမ်းတဝင်ရှိသော `alias@domain` စာကြောင်းကို လမ်းကြောင်းပြောင်းခြင်းအဖြစ်သာ သိမ်းဆည်းထားသည်။ အကောင့်- `{ id, metadata }`။ ကုဒ်- `crates/iroha_data_model/src/account.rs`။
- အကောင့်ဝင်ခွင့်မူဝါဒ — ဒိုမိန်းများသည် မက်တာဒေတာကီး `iroha:account_admission_policy` အောက်တွင် Norito-JSON `AccountAdmissionPolicy` ကို သိမ်းဆည်းခြင်းဖြင့် သွယ်ဝိုက်သောအကောင့်ဖန်တီးမှုကို ထိန်းချုပ်ပါသည်။ သော့မရှိသည့်အခါ၊ ကွင်းဆက်အဆင့် စိတ်ကြိုက်ကန့်သတ်ဘောင် `iroha:default_account_admission_policy` သည် ပုံသေကို ပေးဆောင်သည်။ ၎င်းသည်လည်းမရှိသည့်အခါ၊ hard default သည် `ImplicitReceive` (ပထမထုတ်ခြင်း) ဖြစ်သည်။ မူဝါဒသည် `mode` (`ExplicitOnly` သို့မဟုတ် `ImplicitReceive`) နှင့် ရွေးချယ်နိုင်သော ငွေပေးချေမှုတစ်ခုခြင်း (မူလ `16`) နှင့် ပိတ်ဆို့ဖန်တီးမှုတစ်ခုစာထုပ်များ၊ ချန်လှပ်ထားသော IVM per-sin3809 (အကောင့်တစ်ခုလျှင် 380X) (180NIX) သို့မဟုတ် ပိုင်ဆိုင်မှု အဓိပ္ပါယ်ဖွင့်ဆိုချက်နှင့် ရွေးချယ်နိုင်သော `default_role_on_create` (`AccountCreated` ပြီးနောက် ခွင့်ပြုထားသော၊ ပျောက်ဆုံးပါက `DefaultRoleError` ဖြင့် ငြင်းပယ်သည်)။ ကမ္ဘာဦးကျမ်းတွင် မပါဝင်နိုင်ပါ။ `InstructionExecutionError::AccountAdmission` ပါ အမည်မသိအကောင့်များအတွက် ပြေစာပုံစံ ညွှန်ကြားချက်များကို ပိတ်ထားသည်/တရားမဝင်သော မူဝါဒများကို ငြင်းပယ်ပါ။ `AccountCreated` မတိုင်မီ `iroha:created_via="implicit"`၊ မူရင်းအခန်းကဏ္ဍများသည် နောက်ဆက်တွဲ `AccountRoleGranted` ကို ထုတ်လွှတ်ပြီး ပိုင်ရှင်-အခြေခံစည်းမျဉ်းများသည် အကောင့်အသစ်အား အပိုအခန်းကဏ္ဍများမပါဘဲ ၎င်း၏ကိုယ်ပိုင်ပိုင်ဆိုင်မှု/NFT များကို သုံးစွဲခွင့်ပေးသည်။ Code: `crates/iroha_data_model/src/account/admission.rs`, `crates/iroha_core/src/smartcontracts/isi/account_admission.rs`။
- `AssetDefinitionId` — `asset#domain`။ အဓိပ္ပါယ်- `{ id, spec: NumericSpec, mintable: Mintable, logo, metadata, owned_by, total_quantity }`။ ကုဒ်- `crates/iroha_data_model/src/asset/definition.rs`။
- `AssetId`: canonical encoded literal `norito:<hex>` (legacy textual forms are not supported in first release).
- `NftId` — `nft$domain`။ NFT- `{ id, content: Metadata, owned_by }`။ ကုဒ်- `crates/iroha_data_model/src/nft.rs`။
- `RoleId` — `name`။ အခန်းကဏ္ဍ- တည်ဆောက်သူ `NewRole { inner: Role, grant_to }` နှင့်အတူ `{ id, permissions: BTreeSet<Permission> }`။ ကုဒ်- `crates/iroha_data_model/src/role.rs`။
- `Permission` — `{ name: Ident, payload: Json }`။ ကုဒ်- `crates/iroha_data_model/src/permission.rs`။
- `PeerId`/`Peer` — သက်တူရွယ်တူအထောက်အထား (အများပြည်သူသော့) နှင့် လိပ်စာ။ ကုဒ်- `crates/iroha_data_model/src/peer.rs`။
- `TriggerId` — `name`။ အစပျိုး- `{ id, action }`။ လုပ်ဆောင်ချက်- `{ executable, repeats, authority, filter, metadata }`။ ကုဒ်- `crates/iroha_data_model/src/trigger/`။
- `Metadata` — `BTreeMap<Name, Json>` ကို အမှန်ခြစ် ထည့်သွင်း/ဖယ်ရှားပါ။ ကုဒ်- `crates/iroha_data_model/src/metadata.rs`။
- စာရင်းသွင်းမှုပုံစံ (လျှောက်လွှာအလွှာ)- အစီအစဉ်များသည် `subscription_plan` မက်တာဒေတာပါရှိသော `AssetDefinition` ထည့်သွင်းမှုများဖြစ်သည်။ စာရင်းသွင်းမှုများသည် `subscription` မက်တာဒေတာပါရှိသော `Nft` မှတ်တမ်းများဖြစ်သည်။ စာရင်းသွင်းမှု NFTs များကို ရည်ညွှန်းကိုးကားသည့် အချိန်အစပျိုးခြင်းဖြင့် ငွေတောင်းခံခြင်းကို လုပ်ဆောင်ပါသည်။ `docs/source/subscriptions_api.md` နှင့် `crates/iroha_data_model/src/subscription.rs` ကိုကြည့်ပါ။
- **Cryptographic primitives** (အင်္ဂါရပ် `sm`):- `Sm2PublicKey` / `Sm2Signature` သည် Canonical SEC1 အမှတ် + SM2 အတွက် ပုံသေအနံ `r∥s` ကို ကုဒ်လုပ်ခြင်းအား ထင်ဟပ်စေသည်။ တည်ဆောက်သူများသည် မျဉ်းကွေးအဖွဲ့ဝင်ခြင်းနှင့် ID ခွဲခြားခြင်းဆိုင်ရာ သဘောတရားများ (`DEFAULT_DISTID`) ကို ပြဋ္ဌာန်းထားသော်လည်း အတည်ပြုချက်သည် ပုံစံမမှန်သော သို့မဟုတ် အမြင့်ပိုင်းစကေးများကို ပယ်ချပါသည်။ ကုဒ်- `crates/iroha_crypto/src/sm.rs` နှင့် `crates/iroha_data_model/src/crypto/mod.rs`။
  - `Sm3Hash` သည် GM/T 0004 ကို Norito-serialisable `[u8; 32]` အဖြစ် manifests သို့မဟုတ် telemetry တွင် hash များပေါ်လာတိုင်း အသုံးပြုသည့် အမျိုးအစားအသစ်ကို ဖော်ထုတ်သည်။ ကုဒ်- `crates/iroha_data_model/src/crypto/hash.rs`။
  - `Sm4Key` သည် 128-bit SM4 သော့များကိုကိုယ်စားပြုပြီး host syscalls နှင့် data-model fixtures များကြားတွင်မျှဝေထားသည်။ ကုဒ်- `crates/iroha_data_model/src/crypto/symmetric.rs`။
  ဤအမျိုးအစားများသည် `sm` အင်္ဂါရပ်ကို ဖွင့်ပြီးသည်နှင့် ဤအမျိုးအစားများသည် လက်ရှိ Ed25519/BLS/ML-DSA စံနမူနာများနှင့်အတူ ဒေတာမော်ဒယ်လ် သုံးစွဲသူများအတွက် ရရှိနိုင်ပါသည်။

အရေးကြီးသော လက္ခဏာများ- `Identifiable`, `Registered`/`Registrable` (တည်ဆောက်သူပုံစံ), `HasMetadata`, `IntoKeyValue`။ ကုဒ်: `crates/iroha_data_model/src/lib.rs`။

ဖြစ်ရပ်များ- အဖွဲ့အစည်းတိုင်းတွင် ပြောင်းလဲမှုများမှ ထုတ်လွှတ်သော ဖြစ်ရပ်များ ရှိသည် (ဖန်တီး/ဖျက်/ဖျက်/ပိုင်ရှင်ကို ပြောင်းထားသည်/မက်တာဒေတာကို ပြောင်းထားသည်၊ စသည်ဖြင့်)။ ကုဒ်- `crates/iroha_data_model/src/events/`။

---

## ကန့်သတ်ချက်များ (ကွင်းဆက်ဖွဲ့စည်းမှု)
- မိသားစုများ- `SumeragiParameters { block_time_ms, commit_time_ms, min_finality_ms, pacing_factor_bps, max_clock_drift_ms, collectors_k, collectors_redundant_send_r }`၊ `BlockParameters { max_transactions }`၊ `TransactionParameters { max_signatures, max_instructions, ivm_bytecode_size, max_tx_bytes, max_decompressed_bytes }`၊ `SmartContractParameters { fuel, memory, execution_depth }`၊ အပေါင်း `custom: BTreeMap`။
- ကွဲပြားမှုများအတွက် တစ်ခုတည်းသောစာရင်းများ- `SumeragiParameter`၊ `BlockParameter`၊ `TransactionParameter`၊ `SmartContractParameter`။ စုစည်းမှု- `Parameters`။ ကုဒ်- `crates/iroha_data_model/src/parameter/system.rs`။

ကန့်သတ်ဘောင်များ (ISI): `SetParameter(Parameter)` သည် သက်ဆိုင်ရာအကွက်ကို အပ်ဒိတ်လုပ်ပြီး `ConfigurationEvent::Changed` ကို ထုတ်လွှတ်သည်။ ကုဒ်- `crates/iroha_data_model/src/isi/transparent.rs`၊ `crates/iroha_core/src/smartcontracts/isi/world.rs` ရှိ စီမံအုပ်ချုပ်သူ။

---

## ညွှန်ကြားချက် အမှတ်စဉ်နှင့် မှတ်ပုံတင်ခြင်း။
- Core စရိုက်- `dyn_encode()` နှင့် `dyn_encode()`၊ `as_any()`၊ တည်ငြိမ်သော `id()` (ကွန်ကရစ်အမျိုးအစားအမည်သို့ ပုံသေများ)။
- `InstructionBox`: `Box<dyn Instruction>` ထုပ်ပိုးခြင်း။ Clone/Eq/Ord သည် `(type_id, encoded_bytes)` တွင် လုပ်ဆောင်နေသောကြောင့် တန်းတူညီမျှမှုသည် တန်ဖိုးအားဖြင့်ဖြစ်သည်။
- Norito `InstructionBox` အတွက် serde သည် `(String wire_id, Vec<u8> payload)` (ဝါယာကြိုး ID မရှိပါက `type_name` သို့ ပြန်သွားသည်)။ Deserialization သည် တည်ဆောက်သူများထံသို့ ကမ္ဘာလုံးဆိုင်ရာ `InstructionRegistry` မြေပုံထုတ်ခြင်းအား အသုံးပြုသည်။ မူရင်းမှတ်ပုံတင်ခြင်းတွင် ထည့်သွင်းထားသော ISI များအားလုံး ပါဝင်သည်။ ကုဒ်- `crates/iroha_data_model/src/isi/{mod.rs,registry.rs}`။

---

## ISI- အမျိုးအစားများ၊ ဝေါဟာရများ၊ အမှားများ
Execution ကို `iroha_core::smartcontracts::isi` တွင် `Execute for <Instruction>` မှတစ်ဆင့် လုပ်ဆောင်ပါသည်။ အောက်တွင် အများသူငှာ အကျိုးသက်ရောက်မှုများ၊ ကြိုတင်အခြေအနေများ၊ ထုတ်လွှတ်သည့် ဖြစ်ရပ်များနှင့် အမှားအယွင်းများကို စာရင်းပြုစုထားသည်။

### စာရင်းသွင်း/စာရင်းမသွင်းပါ။
အမျိုးအစားများ- `Register<T: Registered>` နှင့် `Unregister<T: Identifiable>`၊ ပေါင်းလဒ်အမျိုးအစားများ `RegisterBox`/`UnregisterBox` ကွန်ကရစ်ပစ်မှတ်များကို ဖုံးအုပ်ထားသည်။

- မျိုးတူစုကို မှတ်ပုံတင်ပါ- ကမ္ဘာပေါ်ရှိ သက်တူရွယ်တူများ အစုအဝေးတွင် ထည့်သွင်းပါ။
  - ကြိုတင်သတ်မှတ်ချက်များ- ရှိနှင့်ပြီးသားဖြစ်ရမည်။
  - ပွဲများ- `PeerEvent::Added`။
  - အမှားများ- `Repetition(Register, PeerId)` ပွားပါက၊ ရှာဖွေမှုတွင် `FindError`။ ကုဒ်- `core/.../isi/world.rs`။

- Register Domain- `NewDomain` မှ `owned_by = authority` မှ တည်ဆောက်သည်။ ခွင့်မပြုပါ- `genesis` ဒိုမိန်း။
  - ကြိုတင်သတ်မှတ်ချက်များ- ဒိုမိန်းတည်ရှိမှု မရှိ၊ `genesis` မဟုတ်ပါ။
  - ပွဲများ- `DomainEvent::Created`။
  - အမှားများ- `Repetition(Register, DomainId)`, `InvariantViolation("Not allowed to register genesis domain")`။ ကုဒ်- `core/.../isi/world.rs`။- အကောင့်မှတ်ပုံတင်ခြင်း- `NewAccount` မှ တည်ဆောက်မှုများ၊ `genesis` ဒိုမိန်းတွင် ခွင့်မပြုပါ။ `genesis` အကောင့်ကို စာရင်းသွင်း၍မရပါ။
  - ကြိုတင်သတ်မှတ်ချက်များ- ဒိုမိန်းရှိရမည်။ အကောင့်မရှိသော၊ genesis domain တွင်မဟုတ်ပါ။
  - ပွဲများ- `DomainEvent::Account(AccountEvent::Created)`။
  - အမှားများ- `Repetition(Register, AccountId)`, `InvariantViolation("Not allowed to register account in genesis domain")`။ ကုဒ်- `core/.../isi/domain.rs`။

- Register AssetDefinition- တည်ဆောက်သူမှ တည်ဆောက်သည်။ `owned_by = authority` သတ်မှတ်သည်။
  - ကြိုတင်သတ်မှတ်ချက်များ- အဓိပ္ပါယ်မရှိသော၊ domain ရှိတယ်။
  - ပွဲများ- `DomainEvent::AssetDefinition(AssetDefinitionEvent::Created)`။
  - အမှားများ- `Repetition(Register, AssetDefinitionId)`။ ကုဒ်- `core/.../isi/domain.rs`။

- NFT ကို မှတ်ပုံတင်ပါ- တည်ဆောက်သူမှ တည်ဆောက်သည်။ `owned_by = authority` သတ်မှတ်သည်။
  - ကြိုတင်သတ်မှတ်ချက်များ- NFT မရှိခြင်း၊ domain ရှိတယ်။
  - ပွဲများ- `DomainEvent::Nft(NftEvent::Created)`။
  - အမှားများ- `Repetition(Register, NftId)`။ ကုဒ်- `core/.../isi/nft.rs`။

- မှတ်ပုံတင်ရန် အခန်းကဏ္ဍ- `NewRole { inner, grant_to }` (အကောင့်-အခန်းကဏ္ဍမြေပုံဆွဲခြင်းမှတစ်ဆင့် မှတ်တမ်းတင်ထားသော ပထမပိုင်ရှင်)၊ စတိုးဆိုင် `inner: Role` မှ တည်ဆောက်သည်။
  - ကြိုတင်သတ်မှတ်ချက်များ- အခန်းကဏ္ဍမရှိခြင်း။
  - ပွဲများ- `RoleEvent::Created`။
  - အမှားများ- `Repetition(Register, RoleId)`။ ကုဒ်: `core/.../isi/world.rs`။

- Trigger ကို မှတ်ပုံတင်ပါ- စစ်ထုတ်မှုအမျိုးအစားအလိုက် သတ်မှတ်သင့်လျော်သော trigger တွင် trigger ကို သိမ်းဆည်းပါ။
  - ကြိုတင်သတ်မှတ်ချက်များ- စစ်ထုတ်မှုမှာ အသေးအမွှားမဖြစ်ပါက၊ `action.repeats` သည် `Exactly(1)` ဖြစ်ရမည် (မဟုတ်ရင် `MathError::Overflow`)။ ID ပွားခြင်းကို တားမြစ်ထားသည်။
  - ပွဲများ- `TriggerEvent::Created(TriggerId)`။
  - အမှားများ- `Repetition(Register, TriggerId)`၊ `InvalidParameterError::SmartContract(..)` တွင် ပြောင်းလဲခြင်း/အတည်ပြုခြင်း မအောင်မြင်မှုများ။ ကုဒ်- `core/.../isi/triggers/mod.rs`။

- Peer/Domain/Account/AssetDefinition/NFT/Role/Trigger ကို စာရင်းမသွင်းပါ- ပစ်မှတ်ကို ဖယ်ရှားသည် ။ ဖျက်ခြင်းဖြစ်ရပ်များကို ထုတ်လွှတ်သည်။ ထပ်လောင်း ကက်ဆက်ကပ်ခြင်းကို ဖယ်ရှားခြင်း-
  - Domain ကို စာရင်းမသွင်းပါ- ဒိုမိန်းအတွင်းရှိ အကောင့်များအားလုံး၊ ၎င်းတို့၏ အခန်းကဏ္ဍများ၊ ခွင့်ပြုချက်များ၊ tx-sequence ကောင်တာများ၊ အကောင့်တံဆိပ်များနှင့် UAID ချိတ်ဆက်မှုများကို ဖယ်ရှားသည်။ ၎င်းတို့၏ ပိုင်ဆိုင်မှုများ (နှင့် ပစ္စည်းတစ်ခုချင်း မက်တာဒေတာ) ကို ဖျက်သည်။ ဒိုမိန်းရှိ ပိုင်ဆိုင်မှု အဓိပ္ပါယ်ဖွင့်ဆိုချက်အားလုံးကို ဖယ်ရှားသည်။ ဒိုမိန်းရှိ NFTs များနှင့် ဖယ်ရှားထားသော အကောင့်များမှ ပိုင်ဆိုင်သော NFTs မှန်သမျှကို ဖျက်သည်၊ အာဏာပိုင်ဒိုမိန်းနှင့် ကိုက်ညီသည့် အစပျိုးများကို ဖယ်ရှားသည်။ အစီအစဉ်များ- `DomainEvent::Deleted`၊ နှင့် တစ်ခုချင်း ဖျက်ခြင်း ဖြစ်ရပ်များ။ အမှားများ- `FindError::Domain` ပျောက်ဆုံးပါက။ ကုဒ်: `core/.../isi/world.rs`။
  - အကောင့်ကို စာရင်းမသွင်းပါ- အကောင့်၏ခွင့်ပြုချက်များ၊ အခန်းကဏ္ဍများ၊ tx-sequence တန်ပြန်၊ အကောင့်တံဆိပ်ပုံဖော်ခြင်းနှင့် UAID စည်းနှောင်မှုများကို ဖယ်ရှားသည်။ အကောင့်ပိုင်ပစ္စည်းများ (နှင့် ပစ္စည်းတစ်ခုချင်း မက်တာဒေတာ) အကောင့်ပိုင် NFT များကို ဖျက်ခြင်း၊ ဤအကောင့်၏ အခွင့်အာဏာသည် ထိုအကောင့်၏ အစပျိုးမှုများကို ဖယ်ရှားသည်။ အစီအစဉ်များ- `AccountEvent::Deleted`၊ နှင့် NFT ကိုဖယ်ရှားလိုက်လျှင် `NftEvent::Deleted`။ အမှားများ- `FindError::Account` ပျောက်ဆုံးပါက။ ကုဒ်: `core/.../isi/domain.rs`။
  - AssetDefinition ကို စာရင်းမသွင်းပါ- ထိုအဓိပ္ပါယ်ဖွင့်ဆိုချက်နှင့် ၎င်းတို့၏ ပိုင်ဆိုင်မှုတစ်ခုချင်း မက်တာဒေတာအားလုံးကို ဖျက်ပစ်သည်။ အစီအစဉ်များ- ပိုင်ဆိုင်မှုတစ်ခုလျှင် `AssetDefinitionEvent::Deleted` နှင့် `AssetEvent::Deleted`။ အမှားများ- `FindError::AssetDefinition`။ ကုဒ်- `core/.../isi/domain.rs`။
  - NFT ကို စာရင်းမသွင်းပါ- NFT ကို ဖယ်ရှားသည်။ ပွဲများ- `NftEvent::Deleted`။ အမှားများ- `FindError::Nft`။ ကုဒ်- `core/.../isi/nft.rs`။
  - Register Role - အကောင့်အားလုံးမှ အခန်းကဏ္ဍကို ဦးစွာ ရုပ်သိမ်းသည်၊ ထို့နောက် ရာထူးကို ဖယ်ရှားသည်။ ပွဲများ- `RoleEvent::Deleted`။ အမှားများ- `FindError::Role`။ ကုဒ်- `core/.../isi/world.rs`။
  - Trigger ကို မှတ်ပုံတင်ခြင်းမှ ဖြုတ်ပါ- ရှိနေပါက အစပျိုးကို ဖယ်ရှားပါ။ စာရင်းမသွင်းဘဲ ပွားထားသော အထွက်နှုန်းများ `Repetition(Unregister, TriggerId)`။ ပွဲများ- `TriggerEvent::Deleted`။ ကုဒ်- `core/.../isi/triggers/mod.rs`။

### Mint / Burn
အမျိုးအစားများ- `Mint<O, D: Identifiable>` နှင့် `Burn<O, D: Identifiable>`၊ `MintBox`/`BurnBox` အဖြစ် အကွက်များ။- ပိုင်ဆိုင်မှု (ဂဏန်း) mint/burn- လက်ကျန်များနှင့် အဓိပ္ပါယ်ဖွင့်ဆိုချက်၏ `total_quantity` ကို ချိန်ညှိသည်။
  - ကြိုတင်သတ်မှတ်ချက်များ- `Numeric` တန်ဖိုး `AssetDefinition.spec()` ကျေနပ်ရပါမည်။ `mintable` မှ ခွင့်ပြုထားသော mint
    - `Infinitely`- အမြဲတမ်း ခွင့်ပြုထားသည်။
    - `Once`: တစ်ကြိမ်တိတိ ခွင့်ပြုသည်; ပထမဆုံး mint သည် `mintable` ကို `Not` သို့ ပြောင်းလိုက်ပြီး `AssetDefinitionEvent::MintabilityChanged` နှင့် အသေးစိတ် `AssetDefinitionEvent::MintabilityChangedDetailed { asset_definition, minted_amount, authority }` ကို ထုတ်ပေးပါသည်။
    - `Limited(n)`- `n` နောက်ထပ် mint လုပ်ဆောင်ချက်များကို ခွင့်ပြုသည်။ အောင်မြင်သော mint တစ်ခုစီသည် တန်ပြန်မှုကို လျှော့ချသည်။ သုညသို့ရောက်သောအခါ အဓိပ္ပါယ်ဖွင့်ဆိုချက်သည် `Not` သို့ပြောင်းသွားပြီး အထက်ပါကဲ့သို့တူညီသော `MintabilityChanged` ဖြစ်ရပ်များကို ထုတ်လွှတ်ပါသည်။
    - `Not`- အမှားအယွင်း `MintabilityError::MintUnmintable`။
  - ပြည်နယ်ပြောင်းလဲမှု- mint တွင်ပျောက်ဆုံးပါကပိုင်ဆိုင်မှုဖန်တီးပေးသည် လက်ကျန်ငွေသည် သုညဖြစ်သွားပါက ပိုင်ဆိုင်မှုထည့်သွင်းမှုကို ဖယ်ရှားသည်။
  - ဖြစ်ရပ်များ- `AssetEvent::Added`/`AssetEvent::Removed`၊ `AssetDefinitionEvent::MintabilityChanged` (`Once` သို့မဟုတ် `Limited(n)` ၎င်း၏ခွင့်ပြုငွေ ကုန်သွားသောအခါ)။
  - အမှားများ- `TypeError::AssetNumericSpec(Mismatch)`၊ `MathError::Overflow`/`NotEnoughQuantity`။ ကုဒ်- `core/.../isi/asset.rs`။

- Trigger ထပ်ခါတလဲလဲ mint/burn- အစပျိုးတစ်ခုအတွက် `action.repeats` အရေအတွက်ကို ပြောင်းလဲခြင်း။
  - ကြိုတင်သတ်မှတ်ချက်များ- mint တွင်၊ ဇကာသည် mintable ဖြစ်ရမည်။ ဂဏန်းသင်္ချာသည် လျှံ/အောက် မ၀င်ရပါ။
  - ပွဲများ- `TriggerEvent::Extended`/`TriggerEvent::Shortened`။
  - အမှားများ- `MathError::Overflow` မမှန်ကန်သော mint; ပျောက်ဆုံးပါက `FindError::Trigger`။ ကုဒ်- `core/.../isi/triggers/mod.rs`။

### လွှဲပြောင်းခြင်း။
အမျိုးအစားများ- `Transfer<S: Identifiable, O, D: Identifiable>`၊ `TransferBox` အဖြစ် အကွက်များ။

- ပိုင်ဆိုင်မှု (ကိန်းဂဏာန်း)- အရင်းအမြစ် `AssetId` မှနုတ်နုတ်၍ ဦးတည်ရာ `AssetId` (တူညီသောအဓိပ္ပါယ်ဖွင့်ဆိုချက် မတူညီသောအကောင့်)။ သုညရှိသော အရင်းအမြစ်ပိုင်ဆိုင်မှုကို ဖျက်ပါ။
  - ကြိုတင်သတ်မှတ်ချက်များ- ရင်းမြစ် ပိုင်ဆိုင်မှု ရှိနေသည် ။ တန်ဖိုးသည် `spec` ကို ကျေနပ်သည်။
  - ပွဲများ- `AssetEvent::Removed` (ရင်းမြစ်), `AssetEvent::Added` (ခရီးဆုံးနေရာ)။
  - အမှားများ- `FindError::Asset`, `TypeError::AssetNumericSpec`, `MathError::NotEnoughQuantity/Overflow`။ ကုဒ်- `core/.../isi/asset.rs`။

- ဒိုမိန်းပိုင်ဆိုင်မှု- `Domain.owned_by` ကို ဦးတည်ရာအကောင့်သို့ ပြောင်းသည်။
  - ကြိုတင်သတ်မှတ်ချက်များ- အကောင့်နှစ်ခုလုံးရှိပါသည် domain ရှိတယ်။
  - ပွဲများ- `DomainEvent::OwnerChanged`။
  - အမှားများ- `FindError::Account/Domain`။ ကုဒ်- `core/.../isi/domain.rs`။

- AssetDefinition ပိုင်ဆိုင်မှု- `AssetDefinition.owned_by` ကို ဦးတည်ရာအကောင့်သို့ ပြောင်းသည်။
  - ကြိုတင်သတ်မှတ်ချက်များ- အကောင့်နှစ်ခုလုံးရှိပါသည် အဓိပ္ပါယ်ရှိပါသည်; အရင်းအမြစ်သည် လက်ရှိတွင် ၎င်းကို ပိုင်ဆိုင်ရမည်ဖြစ်သည်။
  - ပွဲများ- `AssetDefinitionEvent::OwnerChanged`။
  - အမှားများ- `FindError::Account/AssetDefinition`။ ကုဒ်- `core/.../isi/account.rs`။

- NFT ပိုင်ဆိုင်မှု- `Nft.owned_by` ကို ဦးတည်အကောင့်သို့ ပြောင်းသည်။
  - ကြိုတင်သတ်မှတ်ချက်များ- အကောင့်နှစ်ခုလုံးရှိပါသည် NFT ရှိသည်; အရင်းအမြစ်သည် လက်ရှိတွင် ၎င်းကို ပိုင်ဆိုင်ရမည်ဖြစ်သည်။
  - ပွဲများ- `NftEvent::OwnerChanged`။
  - အရင်းအမြစ် NFT မပိုင်ဆိုင်ပါက `FindError::Account/Nft`၊ `InvariantViolation`။ ကုဒ်- `core/.../isi/nft.rs`။

### မက်တာဒေတာ- သော့တန်ဖိုးကို သတ်မှတ်/ဖယ်ရှားပါ။
အမျိုးအစားများ- `SetKeyValue<T>` နှင့် `RemoveKeyValue<T>` နှင့် `T ∈ { Domain, Account, AssetDefinition, Nft, Trigger }`။ ထုပ်ပိုးထားသော စာရင်းများကို ပေးထားပါသည်။

- သတ်မှတ်ခြင်း- `Metadata[key] = Json(value)` ကို ထည့်သွင်း သို့မဟုတ် အစားထိုးပါ။
- Remove: သော့ကိုဖယ်ရှား; အမှားပါရင်
- ဖြစ်ရပ်များ- `<Target>Event::MetadataInserted` / `MetadataRemoved` အဟောင်း/အသစ်တန်ဖိုးများ။
- အမှားများ- ပစ်မှတ်မရှိပါက `FindError::<Target>` ဖယ်ရှားရန်အတွက် ပျောက်ဆုံးနေသောသော့ပေါ်ရှိ `FindError::MetadataKey`။ ကုဒ်- `crates/iroha_data_model/src/isi/transparent.rs` နှင့် ပစ်မှတ်တစ်ခုလျှင် executor impls။### ခွင့်ပြုချက်များနှင့် ရာထူးများ- ခွင့်ပြု/ပြန်လည်ရုပ်သိမ်းခြင်း။
အမျိုးအစားများ- `Grant<O, D>` နှင့် `Revoke<O, D>`၊ `Permission`/`Role` နှင့် `Account` နှင့် `Permission`/`Role` နှင့် `Account` နှင့် IVM။

- အကောင့်အား ခွင့်ပြုချက်ပေးခြင်း- မွေးရာပါရှိပြီးသားမဟုတ်ပါက `Permission` ကို ပေါင်းထည့်သည်။ ပွဲများ- `AccountEvent::PermissionAdded`။ အမှားများ- ပွားပါက `Repetition(Grant, Permission)`။ ကုဒ်- `core/.../isi/account.rs`။
- အကောင့်မှခွင့်ပြုချက်ကို ရုတ်သိမ်းပါ- ရှိနေပါက ဖယ်ရှားပါ။ ပွဲများ- `AccountEvent::PermissionRemoved`။ အမှားများ- `FindError::Permission` မရှိလျှင်။ ကုဒ်- `core/.../isi/account.rs`။
- အကောင့်အား တာဝန်ပေးအပ်ခြင်း- ပျက်ကွက်ပါက `(account, role)` မြေပုံကို ထည့်သွင်းပါ။ ပွဲများ- `AccountEvent::RoleGranted`။ အမှားအယွင်းများ- `Repetition(Grant, RoleId)`။ ကုဒ်: `core/.../isi/account.rs`။
- အကောင့်မှ အခန်းကဏ္ဍကို ရုတ်သိမ်းပါ- ရှိနေပါက မြေပုံဆွဲခြင်းကို ဖယ်ရှားပါ။ ပွဲများ- `AccountEvent::RoleRevoked`။ အမှားများ- ပျက်ကွက်ပါက `FindError::Role`။ ကုဒ်- `core/.../isi/account.rs`။
- အခန်းကဏ္ဍကိုခွင့်ပြုချက်ပေးသည်- ခွင့်ပြုချက်ထည့်သွင်းပြီး အခန်းကဏ္ဍကို ပြန်လည်တည်ဆောက်သည်။ ပွဲများ- `RoleEvent::PermissionAdded`။ အမှားအယွင်းများ- `Repetition(Grant, Permission)`။ ကုဒ်- `core/.../isi/world.rs`။
- ရာထူးမှခွင့်ပြုချက်ကို ရုတ်သိမ်းသည်- ထိုခွင့်ပြုချက်မရှိဘဲ အခန်းကဏ္ဍကို ပြန်လည်တည်ဆောက်သည်။ ပွဲများ- `RoleEvent::PermissionRemoved`။ အမှားများ- ပျက်ကွက်ပါက `FindError::Permission`။ ကုဒ်- `core/.../isi/world.rs`။

### အစပျိုးမှုများ- လုပ်ဆောင်ရန်
အမျိုးအစား- `ExecuteTrigger { trigger: TriggerId, args: Json }`။
- အပြုအမူ- အစပျိုးစနစ်ခွဲအတွက် `ExecuteTriggerEvent { trigger_id, authority, args }` ကို စီစစ်သည်။ လူကိုယ်တိုင်လုပ်ဆောင်မှုကို ခေါ်ဆိုမှုအစပျိုးခြင်းများအတွက်သာ ခွင့်ပြုသည် (`ExecuteTrigger` filter); filter သည် တူညီရမည်ဖြစ်ပြီး ခေါ်ဆိုသူသည် အစပျိုးလုပ်ဆောင်မှုအာဏာပိုင်ဖြစ်ရမည် သို့မဟုတ် ထိုအာဏာပိုင်အတွက် `CanExecuteTrigger` ကို ကိုင်ထားပါ။ အသုံးပြုသူမှပေးသော executor သည် အသက်ဝင်သောအခါ၊ အစပျိုးလုပ်ဆောင်မှုကို runtime executor မှအတည်ပြုပြီး ငွေပေးငွေယူ၏ executor လောင်စာဘတ်ဂျက် (အခြေခံ `executor.fuel` နှင့် စိတ်ကြိုက်မက်တာဒေတာ `additional_fuel`) ကိုစားသုံးပါသည်။
- အမှားအယွင်းများ- စာရင်းမသွင်းပါက `FindError::Trigger`၊ `InvariantViolation` ကို အာဏာပိုင်မဟုတ်သူများက ခေါ်လျှင်။ ကုဒ်- `core/.../isi/triggers/mod.rs` (နှင့် စမ်းသပ်မှုများ `core/.../smartcontracts/isi/mod.rs`)။

### အဆင့်မြှင့်တင်ပြီး စာရင်းသွင်းပါ။
- `Upgrade { executor }`- ပေးထားသော `Executor` bytecode ကိုအသုံးပြု၍ executor ကို ရွှေ့ပြောင်းပြီး၊ executor နှင့် ၎င်း၏ဒေတာမော်ဒယ်ကို အပ်ဒိတ်လုပ်ကာ `ExecutorEvent::Upgraded` ကိုထုတ်သည်။ အမှားအယွင်းများ- ရွှေ့ပြောင်းခြင်း မအောင်မြင်သည့်အတွက် `InvalidParameterError::SmartContract` အဖြစ် ရစ်ပတ်ထားသည်။ ကုဒ်- `core/.../isi/world.rs`။
- `Log { level, msg }`- ပေးထားသော အဆင့်နှင့်အတူ node မှတ်တမ်းကို ထုတ်လွှတ်သည်။ ပြည်နယ်အပြောင်းအလဲမရှိပါ။ ကုဒ်- `core/.../isi/world.rs`။

### Error Model
ဘုံစာအိတ်- `InstructionExecutionError` သည် အကဲဖြတ်မှုအမှားများ၊ မေးမြန်းမှုမအောင်မြင်မှုများ၊ ပြောင်းလဲမှုများ၊ မတွေ့ရသည့်အရာ၊ ထပ်ခါတလဲလဲ၊ မှတ်သားနိုင်မှု၊ သင်္ချာ၊ မမှန်ကန်သော ကန့်သတ်ဘောင်နှင့် ကွဲလွဲမှုချိုးဖောက်မှုအတွက် အမျိုးအစားများပါရှိသော `InstructionExecutionError`။ စာရင်းကောက်များနှင့် အကူအညီပေးသူများသည် `pub mod error` အောက်တွင် `crates/iroha_data_model/src/isi/mod.rs` တွင်ရှိသည်။

---## ငွေပေးငွေယူများနှင့် အကောင်ထည်ဖော်မှုများ
- `Executable`- `Instructions(ConstVec<InstructionBox>)` သို့မဟုတ် `Ivm(IvmBytecode)`; bytecode သည် base64 အဖြစ် အမှတ်စဉ်ပြုသည်။ ကုဒ်- `crates/iroha_data_model/src/transaction/executable.rs`။
- `TransactionBuilder`/`SignedTransaction`- မက်တာဒေတာ၊ `chain_id`၊ `authority`၊ `creation_time_ms`၊ ရွေးချယ်နိုင်သော I00000324X၊ ရွေးချယ်နိုင်သော I108205 `nonce`။ ကုဒ်- `crates/iroha_data_model/src/transaction/`။
- runtime တွင် `iroha_core` သည် `InstructionBox` batch များကို `Execute for InstructionBox` မှတစ်ဆင့် လုပ်ဆောင်ပြီး သင့်လျော်သော `*Box` သို့မဟုတ် ကွန်ကရစ်ညွှန်ကြားချက်သို့ ကျဆင်းသွားပါသည်။ ကုဒ်- `crates/iroha_core/src/smartcontracts/isi/mod.rs`။
- Runtime executor validation budget (user-provided executor)- ကန့်သတ်ဘောင်များမှ အခြေခံ `executor.fuel` နှင့် ရွေးချယ်နိုင်သော ငွေပေးငွေယူ မက်တာဒေတာ `additional_fuel` (`u64`)၊ လွှဲပြောင်းမှုအတွင်း ညွှန်ကြားချက်များ/အစပျိုးအတည်ပြုချက်များကို မျှဝေထားသည်။

---

## မျိုးကွဲများနှင့် မှတ်စုများ (စမ်းသပ်မှုများနှင့် အစောင့်များမှ)
- ကမ္ဘာဦးကာကွယ်မှုများ- `genesis` ဒိုမိန်း သို့မဟုတ် `genesis` ဒိုမိန်းတွင် အကောင့်များကို စာရင်းသွင်း၍မရပါ။ `genesis` အကောင့်ကို စာရင်းသွင်း၍မရပါ။ ကုဒ်/စမ်းသပ်မှုများ- `core/.../isi/world.rs`၊ `core/.../smartcontracts/isi/mod.rs`။
- ဂဏန်းပိုင်ဆိုင်မှုများသည် mint/transfer/burn တွင် ၎င်းတို့၏ `NumericSpec` ကို ကျေနပ်စေရမည်။ spec မကိုက်ညီသော အထွက်နှုန်းသည် `TypeError::AssetNumericSpec`။
- Mintability- `Once` သည် mint တစ်လုံးကို ခွင့်ပြုပြီးနောက် `Not` သို့ပြောင်းပါ။ `Limited(n)` သည် `Not` သို့မပြောင်းမီ `n` ကို အတိအကျခွင့်ပြုသည်။ `Infinitely` တွင် သတ္တုတူးဖော်ခြင်းကို တားမြစ်ရန် ကြိုးစားခြင်းသည် `MintabilityError::ForbidMintOnMintable` ကို ဖြစ်စေပြီး `Limited(0)` ကို စီစဉ်သတ်မှတ်ခြင်းသည် `MintabilityError::InvalidMintabilityTokens` ကို ထုတ်ပေးသည်။
- မက်တာဒေတာ လုပ်ဆောင်ချက်များသည် သော့ချက် အတိအကျဖြစ်သည်။ မရှိသောသော့ကို ဖယ်ရှားခြင်းသည် အမှားအယွင်းတစ်ခုဖြစ်သည်။
- Trigger filter များသည် mintable မဟုတ်နိုင်ပါ။ ထို့နောက် `Register<Trigger>` သည် `Exactly(1)` ထပ်လုပ်ခြင်းကို ခွင့်ပြုပါသည်။
- မက်တာဒေတာကီး `__enabled` (bool) ဂိတ်များကို စတင်လုပ်ဆောင်ခြင်း ဖွင့်ထားရန် ပုံသေများ ပျောက်ဆုံးနေပြီး၊ ပိတ်ထားသော အစပျိုးမှုများကို ဒေတာ/အချိန်/ခေါ်ဆိုမှုလမ်းကြောင်းများပေါ်တွင် ကျော်သွားပါသည်။
- Determinism- ဂဏန်းသင်္ချာအားလုံးသည် စစ်ဆေးထားသော လုပ်ဆောင်ချက်များကို အသုံးပြုသည်။ under/overflow သည် ရိုက်ထည့်ထားသော သင်္ချာအမှားများကို ပြန်ပေးသည်။ သုညလက်ကျန်များ ပိုင်ဆိုင်မှုထည့်သွင်းမှုများကို ကျဆင်းစေသည် (လျှို့ဝှက်အခြေအနေမရှိ)။

---## လက်တွေ့ဥပမာများ
- Minting နှင့်လွှဲပြောင်း:
  - `Mint::asset_numeric(10, asset_id)` → spec/mintability အရ ခွင့်ပြုပါက 10 ထပ်ထည့်ပါ။ ပွဲများ- `AssetEvent::Added`။
  - `Transfer::asset_numeric(asset_id, 5, to_account)` → ရွှေ့ခြင်း 5; ဖယ်ရှားခြင်း/ထပ်တိုးခြင်းအတွက် ဖြစ်ရပ်များ။
- မက်တာဒေတာ အပ်ဒိတ်များ-
  - `SetKeyValue::account(account_id, "avatar".parse()?, json)` → upsert; `RemoveKeyValue::account(...)` မှတဆင့်ဖယ်ရှားခြင်း။
- အခန်းကဏ္ဍ/ခွင့်ပြုချက်စီမံခန့်ခွဲမှု-
  - `Grant::account_role(role_id, account)`၊ `Grant::role_permission(perm, role)` နှင့် ၎င်းတို့၏ `Revoke` အတွဲများ။
- ဘဝသံသရာကို အစပျိုးပါ
  - `Register::trigger(Trigger::new(id, Action::new(exec, repeats, authority, filter)))` သည် စစ်ထုတ်မှုဖြင့် အဓိပ္ပာယ်သက်ရောက်သော mintability စစ်ဆေးချက်၊ `ExecuteTrigger::new(id).with_args(&args)` သည် စီစဉ်သတ်မှတ်ထားသော အခွင့်အာဏာနှင့် ကိုက်ညီရပါမည်။
  - မက်တာဒေတာကီး `__enabled` ကို `false` သို့ သတ်မှတ်ခြင်းဖြင့် အစပျိုးမှုများကို ပိတ်နိုင်သည် (ဖွင့်ထားရန် လွဲမှားနေသော ပုံသေများ); `SetKeyValue::trigger` သို့မဟုတ် IVM `set_trigger_enabled` syscall မှတဆင့်ပြောင်းပါ။
  - Trigger storage ကို load တွင် ပြုပြင်သည်- မိတ္တူပွား ids၊ မကိုက်ညီသော ids နှင့် ပျောက်ဆုံးနေသော bytecode များကို ရည်ညွှန်းသည့် အစပျိုးမှုများကို ဖြုတ်ချထားသည်။ bytecode ရည်ညွှန်းကိန်းများကို ပြန်လည်တွက်ချက်ပါသည်။
  - လုပ်ဆောင်ချိန်၌ trigger ၏ IVM bytecode ပျောက်ဆုံးပါက၊ trigger ကိုဖယ်ရှားပြီး execution အား ပျက်ကွက်ရလဒ်အဖြစ် no-op အဖြစ် သတ်မှတ်သည်။
  - ကုန်ခမ်းသွားသော အစပျိုးများကို ချက်ချင်းဖယ်ရှားသည်။ ကွပ်မျက်စဉ်အတွင်း အားအင်ကုန်ခမ်းသွားပါက ၎င်းကို ဖြတ်တောက်ပြီး ပျောက်ဆုံးသည်ဟု သတ်မှတ်သည်။
- Parameter အပ်ဒိတ်-
  - `SetParameter(SumeragiParameter::BlockTimeMs(2500).into())` ကို အပ်ဒိတ်လုပ်ပြီး `ConfigurationEvent::Changed` ကို ထုတ်လွှတ်သည်။

---

## ခြေရာခံနိုင်မှု (ရွေးချယ်ထားသော အရင်းအမြစ်များ)
 ဒေတာမော်ဒယ် core- `crates/iroha_data_model/src/{account.rs,domain.rs,asset/**,nft.rs,role.rs,permission.rs,metadata.rs,trigger/**,parameter/**}`။
 - ISI အဓိပ္ပါယ်ဖွင့်ဆိုချက်များနှင့် မှတ်ပုံတင်ခြင်း- `crates/iroha_data_model/src/isi/{mod.rs,register.rs,transfer.rs,mint_burn.rs,transparent.rs,registry.rs}`။
 - ISI လုပ်ဆောင်ချက်- `crates/iroha_core/src/smartcontracts/isi/{mod.rs,world.rs,domain.rs,account.rs,asset.rs,nft.rs,triggers/**}`။
 - ပွဲများ- `crates/iroha_data_model/src/events/**`။
 - ငွေလွှဲမှုများ- `crates/iroha_data_model/src/transaction/**`။

အကယ်၍ သင်သည် ဤ spec ကို ပြန်ဆိုထားသော API/အပြုအမူဇယားသို့ ချဲ့ထွင်လိုပါက သို့မဟုတ် ခိုင်မာသော ဖြစ်ရပ်/အမှားတိုင်းနှင့် ချိတ်ဆက်ထားသော စကားလုံးကို ပြောပါ၊ ကျွန်ုပ် ၎င်းကို ထပ်တိုးပါမည်။