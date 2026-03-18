---
lang: my
direction: ltr
source: docs/source/data_model_and_isi_spec.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2077d985b10b26b29b821646b435cc8850cbc6c842d372de6c9c4523ee95a5b7
source_last_modified: "2026-03-12T11:24:34.970622+00:00"
translation_last_reviewed: 2026-03-12
translator: machine-google-reviewed
---

# Iroha v2 ဒေတာမော်ဒယ်နှင့် ISI — အကောင်အထည်ဖော်မှုမှရရှိလာသော Spec

ဤသတ်မှတ်ချက်သည် ဒီဇိုင်းပြန်လည်သုံးသပ်ခြင်းကို အထောက်အကူဖြစ်စေရန်အတွက် `iroha_data_model` နှင့် `iroha_core` တစ်လျှောက် လက်ရှိအကောင်အထည်ဖော်မှုမှ အင်ဂျင်ပြောင်းပြန်ဖြစ်သည်။ backticks ရှိ လမ်းကြောင်းများသည် တရားဝင်ကုဒ်ကို ညွှန်ပြသည်။

## နယ်ပယ်
- Canonical entities (ဒိုမိန်းများ၊ အကောင့်များ၊ ပိုင်ဆိုင်မှုများ၊ NFTs၊ အခန်းကဏ္ဍများ၊ ခွင့်ပြုချက်များ၊ လုပ်ဖော်ကိုင်ဖက်များ၊ အစပျိုးမှုများ) နှင့် ၎င်းတို့၏ ခွဲခြားသတ်မှတ်မှုများကို သတ်မှတ်သည်။
- ပြည်နယ်-ပြောင်းလဲခြင်းဆိုင်ရာ ညွှန်ကြားချက်များ (ISI) ကို ဖော်ပြသည်- အမျိုးအစားများ၊ ကန့်သတ်ချက်များ၊ ကြိုတင်အခြေအနေများ၊ ပြည်နယ်အကူးအပြောင်းများ၊ ထုတ်လွှတ်သည့် ဖြစ်ရပ်များနှင့် အမှားအယွင်းအခြေအနေများ။
- ကန့်သတ်စီမံခန့်ခွဲမှု၊ အရောင်းအ၀ယ်များနှင့် ညွှန်ကြားချက်အမှတ်စဉ်များကို အကျဉ်းချုပ်ဖော်ပြသည်။

အဆုံးအဖြတ်ပေးခြင်း- ညွှန်ကြားချက်ဆိုင်ရာ ဝေါဟာရများ အားလုံးသည် ဟာ့ဒ်ဝဲကို မှီခိုသည့် အပြုအမူမရှိဘဲ သန့်စင်သော အခြေအနေသို့ ကူးပြောင်းခြင်း ဖြစ်သည်။ Serialization သည် Norito ကို အသုံးပြုသည်။ VM bytecode သည် IVM ကိုအသုံးပြုပြီး on-chain မလုပ်ဆောင်မီတွင် host-side ကို တရားဝင်အတည်ပြုထားသည်။

---

## အကြောင်းအရာများနှင့် သတ်မှတ်ချက်များ
ID များတွင် `Display`/`FromStr` ဖြင့် အသွားအပြန် တည်ငြိမ်သော စာကြောင်းပုံစံများရှိသည်။ အမည်စည်းမျဉ်းများသည် နေရာလွတ်နှင့် သီးသန့် `@ # $` စာလုံးများကို တားမြစ်ထားသည်။- `Name` — တရားဝင်အတည်ပြုထားသော စာသားအမှတ်အသား။ စည်းမျဉ်းများ- `crates/iroha_data_model/src/name.rs`။
- `DomainId` — `name`။ ဒိုမိန်း- `{ id, logo, metadata, owned_by }`။ တည်ဆောက်သူများ- `NewDomain`။ ကုဒ်- `crates/iroha_data_model/src/domain.rs`။
- `AccountId` — Canonical လိပ်စာများကို `AccountAddress` (I105 / hex) မှတဆင့် ထုတ်လုပ်ပြီး Torii သည် `AccountAddress::parse_encoded` မှတဆင့် သွင်းအားများကို ပုံမှန်ဖြစ်စေသည်။ I105 သည် ဦးစားပေးအကောင့်ဖော်မတ်ဖြစ်သည်။ I105 ဖောင်သည် Sora-only UX အတွက်ဖြစ်သည်။ အကျွမ်းတဝင်ရှိသော `alias` (ပယ်ချထားသော အမွေအနှစ်ပုံစံ) စာကြောင်းကို လမ်းကြောင်းသတ်မှတ်ခြင်းအဖြစ်သာ သိမ်းဆည်းထားသည်။ အကောင့်- `{ id, metadata }`။ ကုဒ်- `crates/iroha_data_model/src/account.rs`။- အကောင့်ဝင်ခွင့်မူဝါဒ — ဒိုမိန်းများသည် မက်တာဒေတာကီး `iroha:account_admission_policy` အောက်တွင် Norito-JSON `AccountAdmissionPolicy` ကို သိမ်းဆည်းခြင်းဖြင့် သွယ်ဝိုက်သောအကောင့်ဖန်တီးမှုကို ထိန်းချုပ်ပါသည်။ သော့မရှိသည့်အခါ၊ ကွင်းဆက်အဆင့် စိတ်ကြိုက်ကန့်သတ်ဘောင် `iroha:default_account_admission_policy` သည် ပုံသေကို ပေးသည်။ ၎င်းသည်လည်းမရှိသည့်အခါ၊ hard default သည် `ImplicitReceive` (ပထမထုတ်သည်) ဖြစ်သည်။ မူဝါဒသည် `mode` (`ExplicitOnly` သို့မဟုတ် `ImplicitReceive`) နှင့် ရွေးချယ်နိုင်သော ငွေပေးချေမှုတစ်ခုခြင်း (မူလ `16`) နှင့် ပိတ်ဆို့ဖန်တီးမှုစာထုပ်များ၊ ချန်လှပ်ထားသော Norito (per-block per-block) အကောင့်တစ်ခုလျှင် Norito (သို့) ပိုင်ဆိုင်မှု အဓိပ္ပါယ်ဖွင့်ဆိုချက်နှင့် ရွေးချယ်နိုင်သော `default_role_on_create` (`AccountCreated` ပြီးနောက် ခွင့်ပြုထားသော၊ ပျောက်ဆုံးပါက `DefaultRoleError` ဖြင့် ငြင်းပယ်သည်)။ ကမ္ဘာဦးကျမ်းတွင် မပါဝင်နိုင်ပါ။ `InstructionExecutionError::AccountAdmission` ပါ အမည်မသိအကောင့်များအတွက် ပြေစာပုံစံ ညွှန်ကြားချက်များကို ပိတ်ထားသည်/တရားမဝင်သော မူဝါဒများကို ငြင်းပယ်ပါ။ `AccountCreated` မတိုင်ခင် `iroha:created_via="implicit"`၊ မူရင်းအခန်းကဏ္ဍများသည် နောက်ဆက်တွဲ `AccountRoleGranted` ကို ထုတ်လွှတ်ပြီး ပိုင်ရှင်-အခြေခံစည်းမျဉ်းများသည် အကောင့်အသစ်အား အပိုအခန်းကဏ္ဍများမပါဘဲ ၎င်း၏ကိုယ်ပိုင်ပိုင်ဆိုင်မှု/NFT များကို သုံးစွဲခွင့်ပေးသည်။ Code: `crates/iroha_data_model/src/account/admission.rs`, `crates/iroha_core/src/smartcontracts/isi/account_admission.rs`။
- `AssetDefinitionId` — canonical `aid:<32-lower-hex-no-dash>` (UUID-v4 bytes)။ အဓိပ္ပါယ်- `{ id, name, description?, alias?, spec: NumericSpec, mintable: Mintable, logo, metadata, owned_by, total_quantity }`။ `alias` စာလုံးများသည် `<name>#<domain>@<dataspace>` သို့မဟုတ် `<name>#<dataspace>` ဖြစ်ရမည်၊ `<name>` သည် ပိုင်ဆိုင်မှုအဓိပ္ပါယ်ဖွင့်ဆိုချက်အမည်နှင့် ညီမျှရပါမည်။ ကုဒ်- `crates/iroha_data_model/src/asset/definition.rs`။
- `AssetId`- canonical encoded ပကတိ `norito:<hex>` (ပထမထုတ်ဝေမှုတွင် အမွေအနှစ်စာသားပုံစံများကို ပံ့ပိုးမထားပါ။)- `NftId` — `nft$domain`။ NFT- `{ id, content: Metadata, owned_by }`။ ကုဒ်- `crates/iroha_data_model/src/nft.rs`။
- `RoleId` — `name`။ အခန်းကဏ္ဍ- တည်ဆောက်သူ `NewRole { inner: Role, grant_to }` နှင့်အတူ `{ id, permissions: BTreeSet<Permission> }`။ ကုဒ်- `crates/iroha_data_model/src/role.rs`။
- `Permission` — `{ name: Ident, payload: Json }`။ ကုဒ်- `crates/iroha_data_model/src/permission.rs`။
- `PeerId`/`Peer` — သက်တူရွယ်တူအထောက်အထား (အများပြည်သူသော့) နှင့် လိပ်စာ။ ကုဒ်- `crates/iroha_data_model/src/peer.rs`။
- `TriggerId` — `name`။ အစပျိုး- `{ id, action }`။ လုပ်ဆောင်ချက်- `{ executable, repeats, authority, filter, metadata }`။ ကုဒ်- `crates/iroha_data_model/src/trigger/`။
- `Metadata` — `BTreeMap<Name, Json>` ကို အမှန်ခြစ် ထည့်သွင်း/ဖယ်ရှားပါ။ ကုဒ်- `crates/iroha_data_model/src/metadata.rs`။
- စာရင်းသွင်းမှုပုံစံ (လျှောက်လွှာအလွှာ)- အစီအစဉ်များသည် `subscription_plan` မက်တာဒေတာပါသော `AssetDefinition` ထည့်သွင်းမှုများဖြစ်သည်။ စာရင်းသွင်းမှုများသည် `subscription` မက်တာဒေတာပါရှိသော `Nft` မှတ်တမ်းများဖြစ်သည်။ စာရင်းသွင်းမှု NFTs များကို ရည်ညွှန်းကိုးကားသည့် အချိန်အစပျိုးခြင်းဖြင့် ငွေတောင်းခံခြင်းကို လုပ်ဆောင်ပါသည်။ `docs/source/subscriptions_api.md` နှင့် `crates/iroha_data_model/src/subscription.rs` ကိုကြည့်ပါ။
- **Cryptographic primitives** (အင်္ဂါရပ် `sm`):
  - `Sm2PublicKey` / `Sm2Signature` သည် Canonical SEC1 အမှတ် + SM2 အတွက် ပုံသေအနံ `r∥s` ကို ကုဒ်လုပ်ခြင်းအား ထင်ဟပ်စေသည်။ တည်ဆောက်သူများသည် မျဉ်းကွေးအဖွဲ့ဝင်ခြင်းနှင့် ID ခွဲခြားခြင်းဆိုင်ရာ သဘောတရားများ (`DEFAULT_DISTID`) ကို ပြဋ္ဌာန်းထားသော်လည်း အတည်ပြုချက်သည် ပုံစံမမှန်သော သို့မဟုတ် အပိုင်းအခြားမြင့်သော စကေးများကို ပယ်ချပါသည်။ ကုဒ်- `crates/iroha_crypto/src/sm.rs` နှင့် `crates/iroha_data_model/src/crypto/mod.rs`။
  - `Sm3Hash` သည် GM/T 0004 ၏အချေအတင်ကို Norito-serialisable `[u8; 32]` အမျိုးအစားအသစ်အဖြစ် manifests သို့မဟုတ် telemetry တွင်သုံးသည့် hash များပေါ်လာတိုင်း။ ကုဒ်- `crates/iroha_data_model/src/crypto/hash.rs`။- `Sm4Key` သည် 128-bit SM4 သော့များကို ကိုယ်စားပြုပြီး host syscalls နှင့် data-model fixtures များကြားတွင် မျှဝေထားသည်။ ကုဒ်- `crates/iroha_data_model/src/crypto/symmetric.rs`။
  ဤအမျိုးအစားများသည် `sm` အင်္ဂါရပ်ကို ဖွင့်ပြီးသည်နှင့် ဤအမျိုးအစားများသည် လက်ရှိ Ed25519/BLS/ML-DSA primitives များနှင့်အတူ တည်ရှိပြီး data-model စားသုံးသူများ (Torii, SDKs, genesis tooling) ကို `sm` ကိုဖွင့်ပြီးသည်နှင့် ရရှိနိုင်ပါသည်။
- Dataspace မှရရှိသောဆက်စပ်စတိုးဆိုင်များ (`space_directory_manifests`၊ `uaid_dataspaces`၊ `axt_policies`၊ `axt_replay_ledger`၊ Lane-relay အရေးပေါ် registry override) နှင့် dataspace-target permissions (Norito) ကို စတိုးဆိုင်များတွင် ခွင့်ပြုထားသည် လုပ်ဆောင်နေချိန်ကတ်တလောက်မွမ်းမံမှုများပြီးနောက် ဒေတာနေရာလွတ်များ ဟောင်းနွမ်းနေသော ကိုးကားချက်များကို တားဆီးကာကွယ်ပေးသည့် တက်ကြွသော `dataspace_catalog` မှ ဒေတာနေရာလွတ်များ ပျောက်သောအခါ `State::set_nexus(...)`။ လမ်းကြောတစ်ခုအား အနားပေးသည် သို့မဟုတ် မတူညီသော dataspace သို့ ပြန်လည်သတ်မှတ်ပေးသည့်အခါ လမ်းကြောသတ်မှတ်ထားသော DA/relay ကက်ရှ်များ (`lane_relays`၊ `da_commitments`၊ `da_confidential_compute`၊ `da_pin_intents`) ကိုလည်း လမ်းကြောတစ်ခုအား အနားပေးသည့်အခါ သို့မဟုတ် မတူညီသော dataspace သို့ ပြန်လည်သတ်မှတ်ပေးသည့်အခါတွင်လည်း ဖြတ်တောက်ထားပါသည်။ ထို့ကြောင့် lane-local stateg သည် data များကို ဖြတ်၍မရနိုင်ပါ။ Space Directory ISIs (`PublishSpaceDirectoryManifest`, `RevokeSpaceDirectoryManifest`, `ExpireSpaceDirectoryManifest`) သည် `dataspace` ကိုလည်း လက်ရှိ catalog နှင့် သက်ဆိုင်ပြီး `InvalidParameter` နှင့် အမည်မသိ ID များကို ငြင်းပယ်ပါသည်။

အရေးကြီးသောလက္ခဏာများ- `Identifiable`၊ `Registered`/`Registrable` (တည်ဆောက်သူပုံစံ), `HasMetadata`, `IntoKeyValue`။ ကုဒ်- `crates/iroha_data_model/src/lib.rs`။

ဖြစ်ရပ်များ- အဖွဲ့အစည်းတိုင်းတွင် ပြောင်းလဲမှုများမှ ထုတ်လွှတ်သော ဖြစ်ရပ်များ ရှိသည် (ဖန်တီး/ဖျက်/ဖျက်/ပိုင်ရှင်ကို ပြောင်းထားသည်/မက်တာဒေတာကို ပြောင်းထားသည်၊ စသည်ဖြင့်)။ ကုဒ်- `crates/iroha_data_model/src/events/`။

---## ကန့်သတ်ချက်များ (ကွင်းဆက်ဖွဲ့စည်းမှု)
- မိသားစုများ- `SumeragiParameters { block_time_ms, commit_time_ms, min_finality_ms, pacing_factor_bps, max_clock_drift_ms, collectors_k, collectors_redundant_send_r }`၊ `BlockParameters { max_transactions }`၊ `TransactionParameters { max_signatures, max_instructions, ivm_bytecode_size, max_tx_bytes, max_decompressed_bytes }`၊ `SmartContractParameters { fuel, memory, execution_depth }`၊ အပေါင်း `custom: BTreeMap`။
- ကွဲပြားမှုများအတွက် တစ်ခုတည်းသောစာရင်းများ- `SumeragiParameter`၊ `BlockParameter`၊ `TransactionParameter`၊ `SmartContractParameter`။ စုစည်းမှု- `Parameters`။ ကုဒ်- `crates/iroha_data_model/src/parameter/system.rs`။

ကန့်သတ်ဘောင်များ (ISI): `SetParameter(Parameter)` သည် သက်ဆိုင်ရာအကွက်ကို အပ်ဒိတ်လုပ်ပြီး `ConfigurationEvent::Changed` ကို ထုတ်လွှတ်သည်။ ကုဒ်- `crates/iroha_data_model/src/isi/transparent.rs`၊ `crates/iroha_core/src/smartcontracts/isi/world.rs` ရှိ စီမံအုပ်ချုပ်သူ။

---

## ညွှန်ကြားချက် အမှတ်စဉ်နှင့် မှတ်ပုံတင်ခြင်း။
- Core စရိုက်- `dyn_encode()` နှင့် `dyn_encode()`၊ `as_any()`၊ တည်ငြိမ်သော `id()` (ကွန်ကရစ်အမျိုးအစားအမည်သို့ ပုံသေများ)။
- `InstructionBox`: `Box<dyn Instruction>` ထုပ်ပိုးခြင်း။ Clone/Eq/Ord သည် `(type_id, encoded_bytes)` တွင် လုပ်ဆောင်နေသောကြောင့် တန်းတူညီမျှမှုသည် တန်ဖိုးအားဖြင့်ဖြစ်သည်။
- Norito `InstructionBox` အတွက် serde သည် `(String wire_id, Vec<u8> payload)` (ဝါယာကြိုး ID မရှိပါက `type_name` သို့ ပြန်သွားသည်)။ Deserialization သည် တည်ဆောက်သူများထံသို့ ကမ္ဘာလုံးဆိုင်ရာ `InstructionRegistry` ကို ပုံဖော်ခြင်းအား အသုံးပြုသည်။ မူရင်းမှတ်ပုံတင်ခြင်းတွင် ထည့်သွင်းထားသော ISI များအားလုံး ပါဝင်သည်။ ကုဒ်- `crates/iroha_data_model/src/isi/{mod.rs,registry.rs}`။

---

## ISI- အမျိုးအစားများ၊ ဝေါဟာရများ၊ အမှားများ
Execution ကို `iroha_core::smartcontracts::isi` တွင် `Execute for <Instruction>` မှတစ်ဆင့် လုပ်ဆောင်ပါသည်။ အောက်တွင် အများသူငှာ အကျိုးသက်ရောက်မှုများ၊ ကြိုတင်အခြေအနေများ၊ ထုတ်လွှတ်သည့် ဖြစ်ရပ်များနှင့် အမှားအယွင်းများကို စာရင်းပြုစုထားသည်။

### စာရင်းသွင်း/စာရင်းမသွင်းပါ။
အမျိုးအစားများ- `Register<T: Registered>` နှင့် `Unregister<T: Identifiable>`၊ ပေါင်းလဒ်အမျိုးအစားများ `RegisterBox`/`UnregisterBox` ကွန်ကရစ်ပစ်မှတ်များကို ဖုံးအုပ်ထားသည်။- မျိုးတူစုကို မှတ်ပုံတင်ပါ- ကမ္ဘာပေါ်ရှိ သက်တူရွယ်တူများ အစုအဝေးတွင် ထည့်သွင်းပါ။
  - ကြိုတင်သတ်မှတ်ချက်များ- ရှိနှင့်ပြီးသားဖြစ်ရမည်။
  - ပွဲများ- `PeerEvent::Added`။
  - အမှားများ- ပွားပါက `Repetition(Register, PeerId)`၊ ရှာဖွေမှုများတွင် `FindError`။ ကုဒ်: `core/.../isi/world.rs`။

- Register Domain- `NewDomain` မှ `owned_by = authority` မှ တည်ဆောက်သည်။ ခွင့်မပြုပါ- `genesis` ဒိုမိန်း။
  - ကြိုတင်သတ်မှတ်ချက်များ- ဒိုမိန်းတည်ရှိမှု မရှိ၊ `genesis` မဟုတ်ပါ။
  - ပွဲများ- `DomainEvent::Created`။
  - အမှားများ- `Repetition(Register, DomainId)`, `InvariantViolation("Not allowed to register genesis domain")`။ ကုဒ်- `core/.../isi/world.rs`။

- အကောင့်မှတ်ပုံတင်ခြင်း- `NewAccount` မှ တည်ဆောက်မှုများ၊ `genesis` ဒိုမိန်းတွင် ခွင့်မပြုပါ။ `genesis` အကောင့်ကို စာရင်းသွင်း၍မရပါ။
  - ကြိုတင်သတ်မှတ်ချက်များ- ဒိုမိန်းရှိရမည်။ အကောင့်မရှိသော၊ genesis domain တွင်မဟုတ်ပါ။
  - ပွဲများ- `DomainEvent::Account(AccountEvent::Created)`။
  - အမှားများ- `Repetition(Register, AccountId)`, `InvariantViolation("Not allowed to register account in genesis domain")`။ ကုဒ်- `core/.../isi/domain.rs`။

- Register AssetDefinition- တည်ဆောက်သူမှ တည်ဆောက်သည်။ `owned_by = authority` သတ်မှတ်သည်။
  - ကြိုတင်သတ်မှတ်ချက်များ- အဓိပ္ပါယ်မရှိသော၊ ဒိုမိန်းရှိပါသည်; `name` လိုအပ်သည်၊ ဖြတ်တောက်ပြီးနောက် အလွတ်မဟုတ်ရမည်ဖြစ်ပြီး၊ `#`/`@` မပါဝင်ရပါ။
  - ပွဲများ- `DomainEvent::AssetDefinition(AssetDefinitionEvent::Created)`။
  - အမှားများ- `Repetition(Register, AssetDefinitionId)`။ ကုဒ်- `core/.../isi/domain.rs`။

- NFT ကို မှတ်ပုံတင်ပါ- တည်ဆောက်သူမှ တည်ဆောက်သည်။ `owned_by = authority` သတ်မှတ်သည်။
  - ကြိုတင်သတ်မှတ်ချက်များ- NFT မရှိခြင်း၊ domain ရှိတယ်။
  - ပွဲများ- `DomainEvent::Nft(NftEvent::Created)`။
  - အမှားများ- `Repetition(Register, NftId)`။ ကုဒ်- `core/.../isi/nft.rs`။- Register Role- `NewRole { inner, grant_to }` (အကောင့်-အခန်းကဏ္ဍမြေပုံဆွဲခြင်းမှတစ်ဆင့် မှတ်တမ်းတင်ထားသော ပထမပိုင်ရှင်)၊ စတိုးဆိုင် `inner: Role` မှ တည်ဆောက်သည်။
  - ကြိုတင်သတ်မှတ်ချက်များ- အခန်းကဏ္ဍမရှိခြင်း။
  - ပွဲများ- `RoleEvent::Created`။
  - အမှားများ- `Repetition(Register, RoleId)`။ ကုဒ်- `core/.../isi/world.rs`။

- Trigger ကို မှတ်ပုံတင်ပါ- စစ်ထုတ်မှုအမျိုးအစားအလိုက် သတ်မှတ်သင့်လျော်သော trigger တွင် trigger ကို သိမ်းဆည်းပါ။
  - ကြိုတင်သတ်မှတ်ချက်များ- စစ်ထုတ်မှုမှာ အသေးအမွှားမဖြစ်ပါက၊ `action.repeats` သည် `Exactly(1)` ဖြစ်ရမည် (မဟုတ်ပါက `MathError::Overflow`)။ ID ပွားခြင်းကို တားမြစ်ထားသည်။
  - ပွဲများ- `TriggerEvent::Created(TriggerId)`။
  - အမှားများ- `Repetition(Register, TriggerId)`၊ `InvalidParameterError::SmartContract(..)` ပြောင်းလဲခြင်း/အတည်ပြုခြင်း မအောင်မြင်မှုများ။ ကုဒ်- `core/.../isi/triggers/mod.rs`။- Peer/Domain/Account/AssetDefinition/NFT/Role/Trigger ကို မှတ်ပုံတင်ခြင်းမှ ဖြုတ်ပါ- ပစ်မှတ်ကို ဖယ်ရှားသည်။ ဖျက်ခြင်းဖြစ်ရပ်များကို ထုတ်လွှတ်သည်။ ထပ်လောင်း ကက်ဆက်ကပ်ခြင်းကို ဖယ်ရှားခြင်း-- ဒိုမိန်းစာရင်းမသွင်းပါ- ဒိုမိန်းအဖွဲ့အစည်းအပြင် ၎င်း၏ရွေးချယ်သူ/ထောက်ခံချက်-မူဝါဒအခြေအနေကို ဖယ်ရှားသည်။ ဒိုမိန်းအတွင်းရှိ ပိုင်ဆိုင်မှုဆိုင်ရာ အဓိပ္ပါယ်ဖွင့်ဆိုချက်များ (ထိုအဓိပ္ပါယ်ဖွင့်ဆိုချက်များဖြင့် သော့ခတ်ထားသော လျှို့ဝှက် `zk_assets` ဘေးထွက်အခြေအနေ)၊ အဆိုပါ အဓိပ္ပါယ်ဖွင့်ဆိုချက်များ (နှင့် ပိုင်ဆိုင်မှုတစ်ခုအတွက် မက်တာဒေတာ)၊ ဒိုမိန်းအတွင်းရှိ NFTs နှင့် domain-scoped account-label/alias ခန့်မှန်းချက်များကို ဖျက်သည်။ ၎င်းသည် ဖယ်ရှားထားသော ဒိုမိန်းမှ ရှင်သန်နေသော အကောင့်များကို လင့်ခ်ဖြုတ်ပြီး ဖယ်ရှားလိုက်သော ဒိုမိန်းနှင့် ဖယ်ရှားထားသော အရင်းအမြစ်များကို ရည်ညွှန်းသော အကောင့်-/အခန်းကဏ္ဍ-အတိုင်းအတာ ခွင့်ပြုချက်ထည့်သွင်းမှုများ (ဒိုမိန်းခွင့်ပြုချက်များ၊ ဖယ်ရှားထားသော အဓိပ္ပါယ်ဖွင့်ဆိုချက်များအတွက် ပိုင်ဆိုင်မှု/ပိုင်ဆိုင်မှုခွင့်ပြုချက်များ၊ နှင့် ဖယ်ရှားထားသော NFT ID များအတွက် NFT ခွင့်ပြုချက်များ)။ ဒိုမိန်းကို ဖယ်ရှားခြင်းသည် ကမ္ဘာလုံးဆိုင်ရာ `AccountId`၊ ၎င်း၏ tx-sequence/UAID အခြေအနေ၊ နိုင်ငံခြားပိုင်ဆိုင်မှု သို့မဟုတ် NFT ပိုင်ဆိုင်မှု၊ အစပျိုးအာဏာ သို့မဟုတ် ကျန်ရှိနေသည့်အကောင့်ကို ညွှန်ပြသည့် အခြားသော ပြင်ပစာရင်းစစ်/ဖွဲ့စည်းမှုဆိုင်ရာ အကိုးအကားများကို မဖျက်ပါ။ Guard rails- ဒိုမိန်းရှိ ပိုင်ဆိုင်မှုဆိုင်ရာ အဓိပ္ပါယ်ဖွင့်ဆိုချက်၊ သဘောတူညီချက်၊ အခြေချ-လယ်ဂျာ၊ အများသူငှာလမ်းသွား ဆုကြေးငွေ/တောင်းဆိုမှု၊ အော့ဖ်လိုင်း ထောက်ပံ့ကြေး/လွှဲပြောင်းမှု၊ အခြေချ repo ပုံသေများ (`settlement.repo.eligible_collateral`၊ `settlement.repo.collateral_substitution_matrix`)၊ အုပ်ချုပ်မှု-configured မဲပေးခြင်း/နိုင်ငံသားဖြစ်ခြင်း/ပါလီမန်-အရည်အချင်းပြည့်မီခြင်း/ဗိုင်းရပ်စ်-ဆုချီးမြှင့်ခြင်း ပိုင်ဆိုင်မှု-ဆုချီးမြှင့်ခြင်းဆိုင်ရာ ရည်ညွှန်းချက်များ၊ oracle-economics မှ စီစဉ်သတ်မှတ်ထားသော ဆုလာဘ်/မျဥ်းစောင်း/အငြင်းပွားမှု-ငွေချေးစာချုပ် ပိုင်ဆိုင်မှု-အဓိပ္ပါယ်ဖွင့်ဆိုချက်များ၊ သို့မဟုတ် Nexus အခကြေးငွေ/လောင်းကြေး ပိုင်ဆိုင်မှု-အဓိပ္ပါယ် ကိုးကားချက်များ (I182090,000X)။ အစီအစဉ်များ- `DomainEvent::Deleted`၊ နှင့် တစ်ခုချင်း ဖျက်ခြင်းဖယ်ရှားထားသော domain-scoped အရင်းအမြစ်များအတွက် ပွဲများ။ အမှားများ- `FindError::Domain` ပျောက်ဆုံးပါက၊ `InvariantViolation` ကို ထိန်းသိမ်းထားသော ပိုင်ဆိုင်မှု-အဓိပ္ပါယ်ဖွင့်ဆိုချက် ပဋိပက္ခများ။ ကုဒ်- `core/.../isi/world.rs`။- အကောင့်ကို စာရင်းမသွင်းပါ- အကောင့်၏ခွင့်ပြုချက်များ၊ အခန်းကဏ္ဍများ၊ tx-sequence တန်ပြန်၊ အကောင့်တံဆိပ်ပုံဖော်ခြင်းနှင့် UAID စည်းနှောင်မှုများကို ဖယ်ရှားသည်။ အကောင့်ပိုင်ပစ္စည်းများ (နှင့် ပစ္စည်းတစ်ခုချင်း မက်တာဒေတာ) အကောင့်ပိုင် NFT များကို ဖျက်ခြင်း၊ ထိုအကောင့်၏ အခွင့်အာဏာရှိသော အစပျိုးမှုများကို ဖယ်ရှားပါ။ ဖယ်ရှားထားသော အကောင့်ကို ကိုးကားသော အကောင့်-/အခန်းကဏ္ဍ-သတ်မှတ်ခွင့်ပြုချက် ထည့်သွင်းမှုများ၊ ဖယ်ရှားထားသော ပိုင်ဆိုင်သော NFT ID များအတွက် အကောင့်-/အခန်းကဏ္ဍ-သတ်မှတ်ထားသော NFT-ပစ်မှတ်ခွင့်ပြုချက်များ၊ နှင့် ဖယ်ရှားလိုက်သော အစပျိုးများအတွက် အကောင့်-/အခန်းကဏ္ဍ-သတ်မှတ်ထားသော အစပျိုး-ပစ်မှတ်ခွင့်ပြုချက်များ။ Guard rails- အကောင့်သည် ဒိုမိန်းတစ်ခု ပိုင်ဆိုင်ဆဲ၊ ပိုင်ဆိုင်မှု အဓိပ္ပါယ်ဖွင့်ဆိုချက်၊ SoraFS ပံ့ပိုးပေးသူ ပေါင်းစည်းမှု၊ အသက်ဝင်သော နိုင်ငံသားဖြစ်မှု မှတ်တမ်း၊ အများသူငှာ လမ်းသွားလောင်းကြေး/ဆုငွေ အခြေအနေ (အကောင့်ကို တောင်းဆိုသူ သို့မဟုတ် ဆုပေးသူ-ဆုပေးသူ-ပိုင်ဆိုင်မှု ပိုင်ရှင်အဖြစ် ပေါ်လာသည့်)၊ တက်ကြွသော twitter ပြည်နယ် (ပါဝင်သည့် သို့မဟုတ် ဖြည့်သွင်းထားသော မှတ်တမ်းများ အပါအဝင်၊ oracle-economics မှ စီစဉ်သတ်မှတ်ထားသော ဆုငွေ/မျဥ်းစောင်း အကောင့် ကိုးကားချက်များ)၊ တက်ကြွသော Nexus အခကြေးငွေ/လောင်းကြေး အကောင့် ကိုးကားချက်များ (`nexus.fees.fee_sink_account_id`၊ `nexus.staking.stake_escrow_account_id`၊ `nexus.staking.slash_sink_account_id`၊ ခွဲခြမ်းစိတ်ဖြာပြီး တရားဝင်မဝင်သော ဒိုမိန်းများမရှိသော အကောင့်များ)၊ တက်ကြွသောပြန်လည်ပို-သဘောတူညီချက်အခြေအနေ၊ အသက်ဝင်သောအခြေချ-လယ်ဂျာအခြေအနေ၊ တက်ကြွသောအော့ဖ်လိုင်းခွင့်ပြုချက်/လွှဲပြောင်းမှု သို့မဟုတ် အော့ဖ်လိုင်းစီရင်ချက်-ပြန်လည်ရုတ်သိမ်းခြင်းအခြေအနေ၊ တက်ကြွသောပိုင်ဆိုင်မှုအဓိပ္ပါယ်ဖွင့်ဆိုချက်များအတွက် တက်ကြွသောအော့ဖ်လိုင်းအကောင့်ဖွဲ့စည်းမှုဆိုင်ရာကိုးကားချက်များ (`settlement.offline.escrow_accounts`)၊ တက်ကြွသောအုပ်ချုပ်မှုအခြေအနေ (အဆိုပြုချက်/အဆင့်အတည်ပြုချက်als/locks/slashes/council/parliament rosters၊ အဆိုပြုချက်ပါလီမန်လျှပ်တစ်ပြက်ပုံများ၊ runtime-upgrade proposer records၊ governance-configured escrow/slash-receiver/viral-pool အကောင့်ကိုးကားချက်များ၊ အုပ်ချုပ်မှု SoraFS telemetry မှတစ်ဆင့် တင်သွင်းသူ100NI `gov.sorafs_telemetry.per_provider_submitters`၊ သို့မဟုတ် SoraFS ဝန်ဆောင်မှုပေးသူ-ပိုင်ရှင် ရည်ညွှန်းချက်များ `gov.sorafs_provider_owners`)၊ စီစဉ်သတ်မှတ်ထားသော အကြောင်းအရာထုတ်ဝေခွင့်ပြုစာရင်း အကောင့်ကိုးကားချက်များ (`content.publish_allow_accounts`)၊ တက်ကြွသောလူမှုရေးဆိုင်ရာ အာမခံချက်ဖန်တီးသူ- တက်ကြွသော ပေးပို့သူပြည်နယ်၊ တက်ကြွသောလမ်းကြော-တဆင့်တဆင့်အရေးပေါ်အတည်ပြုပေးသည့်အခြေအနေ၊ သို့မဟုတ် တက်ကြွသော SoraFS ပင်-မှတ်ပုံတင်ထုတ်ပေးသူ/နှောင်ကြိုးမှတ်တမ်းများ (ပင် မန်နီးဖက်စ်များ၊ ထင်ရှားသောအမည်ပွားများ၊ ကူးယူမှုအမှာစာများ)။ အစီအစဉ်များ- `AccountEvent::Deleted`၊ နှင့် NFT ကိုဖယ်ရှားလိုက်လျှင် `NftEvent::Deleted`။ အမှားများ- `FindError::Account` ပျောက်ဆုံးပါက၊ ပိုင်ဆိုင်မှု `InvariantViolation` တွင် မိဘမဲ့ကလေးများ။ ကုဒ်- `core/.../isi/domain.rs`။- AssetDefinition ကို စာရင်းမသွင်းပါ- ထိုအဓိပ္ပါယ်ဖွင့်ဆိုချက်နှင့် ၎င်းတို့၏ ပိုင်ဆိုင်မှုတစ်ခုချင်း မက်တာဒေတာအားလုံးကို ဖျက်ပြီး ထိုအဓိပ္ပါယ်ဖွင့်ဆိုချက်ဖြင့် လျှို့ဝှက်ထားသော `zk_assets` ဘေးထွက်အခြေအနေအား ဖယ်ရှားပေးသည်။ ဖယ်ရှားထားသော ပိုင်ဆိုင်မှု အဓိပ္ပါယ်ဖွင့်ဆိုချက် သို့မဟုတ် ၎င်း၏ ပိုင်ဆိုင်မှုဖြစ်ရပ်များကို ရည်ညွှန်းသည့် ကိုက်ညီသော `settlement.offline.escrow_accounts` နှင့် အကောင့်-/အခန်းကဏ္ဍ-သတ်မှတ်ခွင့်ပြုချက် ထည့်သွင်းမှုများကိုလည်း ဖြတ်တောက်သည်။ Guard rails- အဓိပ္ပါယ်ဖွင့်ဆိုချက်ကို ပြန်ပို-သဘောတူညီချက်၊ အခြေချ-လယ်ဂျာ၊ အများသူငှာ လမ်းသွားဆုငွေ/အရေးဆိုမှု၊ အော့ဖ်လိုင်း ထောက်ပံ့ကြေး/လွှဲပြောင်းမှုအခြေအနေ၊ အခြေချ repo ပုံသေများ (`settlement.repo.eligible_collateral`၊ `settlement.repo.collateral_substitution_matrix`)၊ အုပ်ချုပ်မှု-configured-virtual-parcited/voting/voting ပိုင်ဆိုင်မှု-အဓိပ္ပါယ်ဖွင့်ဆိုချက်များ၊ oracle-economics မှ စီစဉ်သတ်မှတ်ထားသော ဆုလာဘ်/မျဥ်းစောင်း/အငြင်းပွား-နှောင်ကြိုးများ ပိုင်ဆိုင်မှု-အဓိပ္ပါယ်ဖွင့်ဆိုချက်များ၊ သို့မဟုတ် Nexus အခကြေးငွေ/လောင်းကြေး ပိုင်ဆိုင်မှု-အဓိပ္ပါယ်ဖွင့်ဆိုချက်များ (`nexus.fees.fee_asset_id`၊ `nexus.staking.stake_asset_id`)။ အစီအစဉ်များ- ပိုင်ဆိုင်မှုတစ်ခုလျှင် `AssetDefinitionEvent::Deleted` နှင့် `AssetEvent::Deleted`။ အမှားများ- `FindError::AssetDefinition`၊ `InvariantViolation` ကိုးကားမှုပဋိပက္ခများ။ ကုဒ်- `core/.../isi/domain.rs`။
  - NFT ကို မှတ်ပုံတင်ခြင်းမှ ဖြုတ်ပါ- NFT ကို ဖယ်ရှားပြီး NFT ကို ကိုးကားသော အကောင့်/ အခန်းကဏ္ဍအလိုက် ခွင့်ပြုချက်ပါသော လုပ်ဆောင်ချက်များကို ဖယ်ရှားသည်။ ပွဲများ- `NftEvent::Deleted`။ အမှားအယွင်းများ- `FindError::Nft`။ ကုဒ်- `core/.../isi/nft.rs`။
  - Register Role - အကောင့်အားလုံးမှ အခန်းကဏ္ဍကို ဦးစွာ ရုပ်သိမ်းသည်၊ ထို့နောက် ရာထူးကို ဖယ်ရှားသည်။ ပွဲများ- `RoleEvent::Deleted`။ အမှားများ- `FindError::Role`။ ကုဒ်- `core/.../isi/world.rs`။- Trigger ကို စာရင်းမသွင်းပါ- ဖယ်ရှားလိုက်သော trigger ကိုရည်ညွှန်းသော အကောင့်ကို ဖယ်ရှားပြီး ဖယ်ရှားထားသော trigger ကို ရည်ညွှန်းသော အကောင့်ကို ဖျက်မည်ဆိုပါက - မှတ်ပုံတင်ခြင်းမှ ပယ်ဖျက်ပါ။ စာရင်းမသွင်းဘဲ ပွားထားသော အထွက်နှုန်းများ `Repetition(Unregister, TriggerId)`။ ပွဲများ- `TriggerEvent::Deleted`။ ကုဒ်- `core/.../isi/triggers/mod.rs`။

### Mint / Burn
အမျိုးအစားများ- `Mint<O, D: Identifiable>` နှင့် `Burn<O, D: Identifiable>`၊ `MintBox`/`BurnBox` အဖြစ် အကွက်များ။

- ပိုင်ဆိုင်မှု (ဂဏန်း) mint/burn- လက်ကျန်များနှင့် အဓိပ္ပါယ်ဖွင့်ဆိုချက်၏ `total_quantity` ကို ချိန်ညှိသည်။
  - ကြိုတင်သတ်မှတ်ချက်များ- `Numeric` တန်ဖိုးသည် `AssetDefinition.spec()` ကျေနပ်ရပါမည်။ `mintable` မှခွင့်ပြုထားသော mint
    - `Infinitely`- အမြဲတမ်း ခွင့်ပြုထားသည်။
    - `Once`: တစ်ကြိမ်တိတိ ခွင့်ပြုသည်; ပထမဆုံး mint သည် `mintable` ကို `Not` သို့ပြောင်းလိုက်ပြီး `AssetDefinitionEvent::MintabilityChanged` နှင့် အသေးစိတ် `AssetDefinitionEvent::MintabilityChangedDetailed { asset_definition, minted_amount, authority }` ကို ထုတ်ပေးပါသည်။
    - `Limited(n)`- `n` ထပ်ဆောင်း mint လုပ်ငန်းများကို ခွင့်ပြုသည်။ အောင်မြင်သော mint တစ်ခုစီသည် တန်ပြန်မှုကို လျှော့ချသည်။ သုညသို့ရောက်သောအခါ အဓိပ္ပါယ်ဖွင့်ဆိုချက်သည် `Not` သို့ပြောင်းသွားပြီး အထက်ပါကဲ့သို့တူညီသော `MintabilityChanged` ဖြစ်ရပ်များကို ထုတ်လွှတ်ပါသည်။
    - `Not`- အမှားအယွင်း `MintabilityError::MintUnmintable`။
  - ပြည်နယ်ပြောင်းလဲမှု- mint တွင်ပျောက်ဆုံးပါကပိုင်ဆိုင်မှုဖန်တီးပေးသည် လက်ကျန်ငွေသည် သုညဖြစ်သွားပါက ပိုင်ဆိုင်မှုထည့်သွင်းမှုကို ဖယ်ရှားသည်။
  - ဖြစ်ရပ်များ- `AssetEvent::Added`/`AssetEvent::Removed`၊ `AssetDefinitionEvent::MintabilityChanged` (`Once` သို့မဟုတ် `Limited(n)` ၎င်း၏ခွင့်ပြုငွေ ကုန်ဆုံးသွားသောအခါ)။
  - အမှားများ- `TypeError::AssetNumericSpec(Mismatch)`၊ `MathError::Overflow`/`NotEnoughQuantity`။ ကုဒ်- `core/.../isi/asset.rs`။- Trigger ထပ်ခါတလဲလဲ mint/burn- အစပျိုးတစ်ခုအတွက် `action.repeats` အရေအတွက်ကို ပြောင်းလဲခြင်း။
  - ကြိုတင်သတ်မှတ်ချက်များ- mint တွင်၊ ဇကာသည် mintable ဖြစ်ရမည်။ ဂဏန်းသင်္ချာသည် လျှံ/အောက် မ၀င်ရပါ။
  - ပွဲများ- `TriggerEvent::Extended`/`TriggerEvent::Shortened`။
  - အမှားများ- `MathError::Overflow` မမှန်ကန်သော mint; ပျောက်ဆုံးပါက `FindError::Trigger`။ ကုဒ်- `core/.../isi/triggers/mod.rs`။

### လွှဲပြောင်းခြင်း။
အမျိုးအစားများ- `Transfer<S: Identifiable, O, D: Identifiable>`၊ `TransferBox` အဖြစ် ထုပ်ပိုးထားသည်။

- ပိုင်ဆိုင်မှု (ကိန်းဂဏာန်း)- အရင်းအမြစ် `AssetId` မှနုတ်နုတ်၍ ဦးတည်ရာ `AssetId` (တူညီသောအဓိပ္ပါယ်ဖွင့်ဆိုချက် မတူညီသောအကောင့်)။ သုညရှိသော အရင်းအမြစ်ပိုင်ဆိုင်မှုကို ဖျက်ပါ။
  - ကြိုတင်သတ်မှတ်ချက်များ- ရင်းမြစ် ပိုင်ဆိုင်မှု ရှိနေသည် ။ တန်ဖိုးသည် `spec` ကို ကျေနပ်သည်။
  - ပွဲများ- `AssetEvent::Removed` (အရင်းအမြစ်), `AssetEvent::Added` (ခရီးဆုံးနေရာ)။
  - အမှားများ- `FindError::Asset`, `TypeError::AssetNumericSpec`, `MathError::NotEnoughQuantity/Overflow`။ ကုဒ်- `core/.../isi/asset.rs`။

- ဒိုမိန်းပိုင်ဆိုင်မှု- `Domain.owned_by` ကို ဦးတည်အကောင့်သို့ ပြောင်းသည်။
  - ကြိုတင်သတ်မှတ်ချက်များ- အကောင့်နှစ်ခုလုံးရှိပါသည် domain ရှိတယ်။
  - ပွဲများ- `DomainEvent::OwnerChanged`။
  - အမှားများ- `FindError::Account/Domain`။ ကုဒ်- `core/.../isi/domain.rs`။

- AssetDefinition ပိုင်ဆိုင်မှု- `AssetDefinition.owned_by` ကို ဦးတည်ရာအကောင့်သို့ ပြောင်းသည်။
  - ကြိုတင်သတ်မှတ်ချက်များ- အကောင့်နှစ်ခုလုံးရှိပါသည် အဓိပ္ပါယ်ရှိပါသည်; အရင်းအမြစ်သည် လက်ရှိပိုင်ဆိုင်ရမည်၊ အခွင့်အာဏာသည် အရင်းအမြစ်အကောင့်၊ အရင်းအမြစ်-ဒိုမိန်းပိုင်ရှင် သို့မဟုတ် ပိုင်ဆိုင်မှု-အဓိပ္ပာယ်ဖွင့်ဆိုချက်-ဒိုမိန်းပိုင်ရှင်ဖြစ်ရမည်။
  - ပွဲများ- `AssetDefinitionEvent::OwnerChanged`။
  - အမှားများ- `FindError::Account/AssetDefinition`။ ကုဒ်- `core/.../isi/account.rs`။- NFT ပိုင်ဆိုင်မှု- `Nft.owned_by` ကို ဦးတည်အကောင့်သို့ ပြောင်းသည်။
  - ကြိုတင်သတ်မှတ်ချက်များ- အကောင့်နှစ်ခုလုံးရှိပါသည် NFT ရှိသည်; အရင်းအမြစ်သည် လက်ရှိပိုင်ဆိုင်ရမည်၊ အခွင့်အာဏာသည် အရင်းအမြစ်အကောင့်၊ ရင်းမြစ်-ဒိုမိန်းပိုင်ရှင်၊ NFT-ဒိုမိန်းပိုင်ရှင် သို့မဟုတ် ထို NFT အတွက် `CanTransferNft` ကို ကိုင်ထားရပါမည်။
  - ပွဲများ- `NftEvent::OwnerChanged`။
  - အရင်းအမြစ်သည် NFT မပိုင်ဆိုင်ပါက `FindError::Account/Nft`၊ `InvariantViolation`။ ကုဒ်: `core/.../isi/nft.rs`။

### မက်တာဒေတာ- သော့တန်ဖိုးကို သတ်မှတ်/ဖယ်ရှားပါ။
အမျိုးအစားများ- `SetKeyValue<T>` နှင့် `RemoveKeyValue<T>` နှင့် `T ∈ { Domain, Account, AssetDefinition, Nft, Trigger }`။ ထုပ်ပိုးထားသောစာရင်းများ ပေးထားသည်။

- သတ်မှတ်ခြင်း- `Metadata[key] = Json(value)` ကို ထည့်သွင်း သို့မဟုတ် အစားထိုးပါ။
- Remove: သော့ကိုဖယ်ရှား; အမှားပါရင်
- ဖြစ်ရပ်များ- `<Target>Event::MetadataInserted` / `MetadataRemoved` အဟောင်း/အသစ်တန်ဖိုးများ။
- အမှားများ- ပစ်မှတ်မရှိပါက `FindError::<Target>`၊ ဖယ်ရှားရန်အတွက် ပျောက်ဆုံးနေသောသော့ပေါ်ရှိ `FindError::MetadataKey`။ ကုဒ်- `crates/iroha_data_model/src/isi/transparent.rs` နှင့် ပစ်မှတ်တစ်ခုလျှင် executor impls။

### ခွင့်ပြုချက်များနှင့် ရာထူးများ- ခွင့်ပြု/ပြန်လည်ရုပ်သိမ်းခြင်း။
အမျိုးအစားများ- `Grant<O, D>` နှင့် `Revoke<O, D>`၊ `Permission`/`Role` နှင့် `Account` နှင့် `Permission`/`Role` နှင့် `TriggerEvent::Created(TriggerId)` မှ Iroha.- အကောင့်အား ခွင့်ပြုချက်ပေးသည်- မွေးရာပါရှိပြီးသားမဟုတ်ပါက `Permission` ကို ပေါင်းထည့်သည်။ ပွဲများ- `AccountEvent::PermissionAdded`။ အမှားများ- ပွားပါက `Repetition(Grant, Permission)`။ ကုဒ်- `core/.../isi/account.rs`။
- အကောင့်မှခွင့်ပြုချက်ကို ရုတ်သိမ်းပါ- ရှိနေပါက ဖယ်ရှားပါ။ ပွဲများ- `AccountEvent::PermissionRemoved`။ အမှားများ- `FindError::Permission` မရှိလျှင်။ ကုဒ်- `core/.../isi/account.rs`။
- အကောင့်အား တာဝန်ပေးအပ်ခြင်း- ပျက်ကွက်ပါက `(account, role)` မြေပုံကို ထည့်သွင်းပါ။ ပွဲများ- `AccountEvent::RoleGranted`။ အမှားများ- `Repetition(Grant, RoleId)`။ ကုဒ်- `core/.../isi/account.rs`။
- အကောင့်မှ အခန်းကဏ္ဍကို ရုတ်သိမ်းပါ- ရှိနေပါက မြေပုံဆွဲခြင်းကို ဖယ်ရှားပါ။ ပွဲများ- `AccountEvent::RoleRevoked`။ အမှားများ- `FindError::Role` မရှိလျှင်။ ကုဒ်- `core/.../isi/account.rs`။
- အခန်းကဏ္ဍကိုခွင့်ပြုချက်ပေးသည်- ခွင့်ပြုချက်ထည့်သွင်းပြီး အခန်းကဏ္ဍကို ပြန်လည်တည်ဆောက်သည်။ ပွဲများ- `RoleEvent::PermissionAdded`။ အမှားများ- `Repetition(Grant, Permission)`။ ကုဒ်- `core/.../isi/world.rs`။
- ရာထူးမှခွင့်ပြုချက်ကို ရုတ်သိမ်းသည်- ထိုခွင့်ပြုချက်မရှိဘဲ အခန်းကဏ္ဍကို ပြန်လည်တည်ဆောက်သည်။ ပွဲများ- `RoleEvent::PermissionRemoved`။ အမှားများ- ပျက်ကွက်ပါက `FindError::Permission`။ ကုဒ်- `core/.../isi/world.rs`။### အစပျိုးမှုများ- လုပ်ဆောင်ရန်
အမျိုးအစား- `ExecuteTrigger { trigger: TriggerId, args: Json }`။
- အပြုအမူ- အစပျိုးစနစ်ခွဲအတွက် `ExecuteTriggerEvent { trigger_id, authority, args }` ကို စီစစ်သည်။ လူကိုယ်တိုင်လုပ်ဆောင်မှုကို ခေါ်ဆိုမှုအစပျိုးခြင်းများအတွက်သာ ခွင့်ပြုသည် (`ExecuteTrigger` filter); စစ်ထုတ်မှု တူညီရမည်ဖြစ်ပြီး ခေါ်ဆိုသူသည် အစပျိုးအရေးယူပိုင်ခွင့်အာဏာ သို့မဟုတ် ထိုအာဏာအတွက် `CanExecuteTrigger` ကို ကိုင်ထားရပါမည်။ အသုံးပြုသူမှပေးသော executor သည် အသက်ဝင်သောအခါ၊ အစပျိုးလုပ်ဆောင်မှုကို runtime executor မှအတည်ပြုပြီး ငွေပေးငွေယူ၏ executor လောင်စာဘတ်ဂျက် (အခြေခံ `executor.fuel` နှင့် စိတ်ကြိုက်မက်တာဒေတာ `additional_fuel`) ကိုအသုံးပြုသည်။
- စာရင်းမသွင်းပါက အမှားများ- `FindError::Trigger`။ `InvariantViolation` ကို အာဏာပိုင်မဟုတ်သူများက ခေါ်လျှင်။ ကုဒ်- `core/.../isi/triggers/mod.rs` (နှင့် `core/.../smartcontracts/isi/mod.rs` တွင် စမ်းသပ်မှုများ)။

### အဆင့်မြှင့်တင်ပြီး စာရင်းသွင်းပါ။
- `Upgrade { executor }`- ပေးထားသော `Executor` bytecode ကို အသုံးပြု၍ executor ကို ရွှေ့ပြောင်းပြီး၊ executor နှင့် ၎င်း၏ဒေတာမော်ဒယ်ကို အပ်ဒိတ်လုပ်ကာ `ExecutorEvent::Upgraded` ကို ထုတ်လွှတ်သည်။ အမှားအယွင်းများ- ရွှေ့ပြောင်းခြင်း မအောင်မြင်သည့်အတွက် `InvalidParameterError::SmartContract` အဖြစ် ရစ်ပတ်ထားသည်။ ကုဒ်- `core/.../isi/world.rs`။
- `Log { level, msg }`- ပေးထားသော အဆင့်နှင့်အတူ node မှတ်တမ်းကို ထုတ်လွှတ်သည်။ ပြည်နယ်အပြောင်းအလဲမရှိပါ။ ကုဒ်- `core/.../isi/world.rs`။

### Error Model
ဘုံစာအိတ်- `InstructionExecutionError` သည် အကဲဖြတ်အမှားများ၊ စုံစမ်းမေးမြန်းမှု မအောင်မြင်မှုများ၊ ပြောင်းလဲမှုများ၊ မတွေ့ရသည့် အကြောင်းအရာ၊ ထပ်ခါတလဲလဲ၊ မှတ်သားနိုင်မှု၊ သင်္ချာ၊ မမှန်ကန်သော ကန့်သတ်ဘောင်နှင့် ပုံစံကွဲလွဲမှားမှုများအတွက် အမျိုးအစားများပါရှိသော `InstructionExecutionError`။ စာရင်းကောက်များနှင့် အကူအညီပေးသူများသည် `pub mod error` အောက်တွင် `crates/iroha_data_model/src/isi/mod.rs` တွင်ရှိသည်။

---## ငွေပေးငွေယူများနှင့် အကောင်ထည်ဖော်မှုများ
- `Executable`- `Instructions(ConstVec<InstructionBox>)` သို့မဟုတ် `Ivm(IvmBytecode)`; bytecode သည် base64 အဖြစ် အမှတ်စဉ်ပြုသည်။ ကုဒ်- `crates/iroha_data_model/src/transaction/executable.rs`။
- `TransactionBuilder`/`SignedTransaction`- မက်တာဒေတာ၊ `chain_id`၊ `authority`၊ `creation_time_ms`၊ ရွေးချယ်နိုင်သော I103030X၊ ရွေးချယ်နိုင်သော I103030 `nonce`။ ကုဒ်- `crates/iroha_data_model/src/transaction/`။
- runtime တွင် `iroha_core` သည် `InstructionBox` batch များကို `Execute for InstructionBox` မှတစ်ဆင့် လုပ်ဆောင်ပြီး သင့်လျော်သော `*Box` သို့မဟုတ် ကွန်ကရစ်ညွှန်ကြားချက်သို့ ကျဆင်းသွားပါသည်။ ကုဒ်- `crates/iroha_core/src/smartcontracts/isi/mod.rs`။
- Runtime executor validation budget (user-provided executor)- ကန့်သတ်ဘောင်များမှ အခြေခံ `executor.fuel` နှင့် ရွေးချယ်နိုင်သော ငွေပေးငွေယူ မက်တာဒေတာ `additional_fuel` (`u64`)၊ လွှဲပြောင်းမှုအတွင်း ညွှန်ကြားချက်များ/အစပျိုးအတည်ပြုချက်များကို မျှဝေထားသည်။

---## မျိုးကွဲများနှင့် မှတ်စုများ (စမ်းသပ်မှုများနှင့် အစောင့်များမှ)
- ကမ္ဘာဦးကာကွယ်မှုများ- `genesis` ဒိုမိန်း သို့မဟုတ် `genesis` ဒိုမိန်းတွင် အကောင့်များကို စာရင်းသွင်း၍မရပါ။ `genesis` အကောင့်ကို စာရင်းသွင်း၍မရပါ။ ကုဒ်/စမ်းသပ်မှုများ- `core/.../isi/world.rs`၊ `core/.../smartcontracts/isi/mod.rs`။
- ဂဏန်းပိုင်ဆိုင်မှုများသည် mint/transfer/burn တွင် ၎င်းတို့၏ `NumericSpec` ကို ကျေနပ်စေရမည်။ spec မကိုက်ညီသော အထွက်နှုန်းသည် `TypeError::AssetNumericSpec` ဖြစ်သည်။
- Mintability- `Once` သည် mint တစ်လုံးကို ခွင့်ပြုပြီးနောက် `Not` သို့ ပြောင်းသည်။ `Limited(n)` သည် `Not` သို့မပြောင်းမီ `n` ကို အတိအကျခွင့်ပြုသည်။ `Infinitely` တွင် သတ္တုတူးဖော်ခြင်းကို တားမြစ်ရန် ကြိုးစားခြင်းသည် `MintabilityError::ForbidMintOnMintable` ကို ဖြစ်စေပြီး `Limited(0)` ကို စီစဉ်သတ်မှတ်ခြင်းသည် `MintabilityError::InvalidMintabilityTokens` ကို ဖြစ်ပေါ်စေပါသည်။
- မက်တာဒေတာလုပ်ဆောင်ချက်များသည် သော့ချက်-အတိအကျဖြစ်သည်။ မရှိသောသော့ကို ဖယ်ရှားခြင်းသည် အမှားအယွင်းတစ်ခုဖြစ်သည်။
- Trigger filter များသည် mintable မဟုတ်နိုင်ပါ။ ထို့နောက် `Register<Trigger>` သည် `Exactly(1)` ထပ်လုပ်ခြင်းကို ခွင့်ပြုသည်။
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
  - `Register::trigger(Trigger::new(id, Action::new(exec, repeats, authority, filter)))` သည် စစ်ထုတ်မှုဖြင့် အဓိပ္ပာယ်သက်ရောက်သော mintability စစ်ဆေးချက်၊ `ExecuteTrigger::new(id).with_args(&args)` သည် စီစဉ်သတ်မှတ်ထားသော အခွင့်အာဏာနှင့် ကိုက်ညီရမည်။
  - မက်တာဒေတာကီး `__enabled` ကို `false` သို့ သတ်မှတ်ခြင်းဖြင့် အစပျိုးမှုများကို ပိတ်နိုင်သည် (ဖွင့်ရန် လွဲမှားနေသော ပုံသေများ); `SetKeyValue::trigger` သို့မဟုတ် IVM `set_trigger_enabled` syscall မှတဆင့်ပြောင်းပါ။
  - Trigger storage ကို load တွင် ပြုပြင်သည်- မိတ္တူပွား ids၊ မကိုက်ညီသော ids နှင့် ပျောက်ဆုံးနေသော bytecode များကို ရည်ညွှန်းသည့် အစပျိုးမှုများကို ဖြုတ်ချထားသည်။ bytecode ရည်ညွှန်းကိန်းများကို ပြန်လည်တွက်ချက်ပါသည်။
  - လုပ်ဆောင်သည့်အချိန်တွင် trigger ၏ IVM bytecode ပျောက်ဆုံးပါက၊ trigger ကိုဖယ်ရှားပြီး execution အား ပျက်ကွက်ရလဒ်အဖြစ် no-op အဖြစ် သတ်မှတ်သည်။
  - ကုန်ခမ်းသွားသော အစပျိုးများကို ချက်ချင်းဖယ်ရှားသည်။ ကွပ်မျက်စဉ်အတွင်း အားအင်ကုန်ခမ်းသွားပါက ၎င်းကို ဖြတ်တောက်ပြီး ပျောက်ဆုံးသည်ဟု သတ်မှတ်သည်။
- ကန့်သတ်ချက် အပ်ဒိတ်-
  - `SetParameter(SumeragiParameter::BlockTimeMs(2500).into())` ကို အပ်ဒိတ်လုပ်ပြီး `ConfigurationEvent::Changed` ကို ထုတ်လွှတ်သည်။CLI / Torii `aid` + alias ဥပမာများ-
- canonical aid + တိကျသောအမည် + ရှည်လျားသောအမည်များဖြင့် မှတ်ပုံတင်ပါ-
  - `iroha ledger asset definition register --id aid:2f17c72466f84a4bb8a8e24884fdcd2f --name pkr --alias pkr#ubl@sbp`
- canonical aid + တိကျသောအမည် + အတိုကောက်အမည်များ ဖြင့် မှတ်ပုံတင်ပါ။
  - `iroha ledger asset definition register --id aid:550e8400e29b41d4a7164466554400dd --name pkr --alias pkr#sbp`
- နာမည်များ + အကောင့်အစိတ်အပိုင်းများမှ Mint
  - `iroha ledger asset mint --definition-alias pkr#ubl@sbp --account <i105> --quantity 500`
- canonical aid ကို alias များကို ဖြေရှင်းပါ
  - JSON `{ "alias": "pkr#ubl@sbp" }` ဖြင့် `POST /v1/assets/aliases/resolve`

ပြောင်းရွှေ့မှုမှတ်စု-
- `name#domain` စာသားပိုင်ဆိုင်မှု-အဓိပ္ပါယ်ဖွင့်ဆိုချက် ID များသည် ပထမထုတ်ဝေမှုတွင် ရည်ရွယ်ချက်ရှိရှိ ပံ့ပိုးမထားပါ။
- mint/burn/transfer boundaries များရှိ ပိုင်ဆိုင်မှု ID များသည် canonical `norito:<hex>` တွင် ရှိနေသည်။ `iroha tools encode asset-id` ကို `--definition aid:...` သို့မဟုတ် `--alias ...` နှင့် `--account` ကို အသုံးပြုပါ။

---

## ခြေရာခံနိုင်မှု (ရွေးချယ်ထားသော အရင်းအမြစ်များ)
 ဒေတာမော်ဒယ် core- `crates/iroha_data_model/src/{account.rs,domain.rs,asset/**,nft.rs,role.rs,permission.rs,metadata.rs,trigger/**,parameter/**}`။
 - ISI အဓိပ္ပါယ်ဖွင့်ဆိုချက်များနှင့် မှတ်ပုံတင်ခြင်း- `crates/iroha_data_model/src/isi/{mod.rs,register.rs,transfer.rs,mint_burn.rs,transparent.rs,registry.rs}`။
 - ISI လုပ်ဆောင်ချက်- `crates/iroha_core/src/smartcontracts/isi/{mod.rs,world.rs,domain.rs,account.rs,asset.rs,nft.rs,triggers/**}`။
 - ပွဲများ- `crates/iroha_data_model/src/events/**`။
 - ငွေလွှဲမှုများ- `crates/iroha_data_model/src/transaction/**`။

အကယ်၍ သင်သည် ဤ spec ကို ပြန်ဆိုထားသော API/အပြုအမူဇယားသို့ ချဲ့ထွင်လိုပါက သို့မဟုတ် ခိုင်မာသော ဖြစ်ရပ်/အမှားတိုင်းနှင့် ချိတ်ဆက်ထားသော စကားလုံးကို ပြောပါ၊ ကျွန်ုပ် ၎င်းကို ထပ်တိုးပါမည်။