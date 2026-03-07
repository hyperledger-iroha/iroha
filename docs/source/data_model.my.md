---
lang: my
direction: ltr
source: docs/source/data_model.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8b6388355a41797eb7d0b7f47cfa8fcac4e136c5a2e5eb0a264384ecdba930b8
source_last_modified: "2026-02-01T13:51:49.945202+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Iroha v2 ဒေတာမော်ဒယ် - Deep Dive

ဤစာတမ်းသည် `iroha_data_model` သေတ္တာတွင် Iroha v2 ဒေတာမော်ဒယ်အဖြစ် ဖွဲ့စည်းတည်ဆောက်ထားသည့် ဖွဲ့စည်းပုံများ၊ ခွဲခြားသတ်မှတ်မှုများ၊ စရိုက်လက္ခဏာများနှင့် ပရိုတိုကောများကို ရှင်းပြထားသည်။ ၎င်းသည် သင်ပြန်လည်သုံးသပ်ပြီး အပ်ဒိတ်များကို အဆိုပြုနိုင်သည့် တိကျသော ကိုးကားချက်ဖြစ်သည်။

## နယ်ပယ်နှင့်အခြေခံများ

- ရည်ရွယ်ချက်- ဒိုမိန်းအရာဝတ္ထုများ (ဒိုမိန်းများ၊ အကောင့်များ၊ ပိုင်ဆိုင်မှုများ၊ NFTs၊ အခန်းကဏ္ဍများ၊ ခွင့်ပြုချက်များ၊ လုပ်ဖော်ကိုင်ဖက်များ)၊ နိုင်ငံတော်ပြောင်းလဲခြင်းဆိုင်ရာ လမ်းညွှန်ချက်များ (ISI)၊ မေးမြန်းချက်များ၊ အစပျိုးမှုများ၊ ငွေပေးငွေယူများ၊ လုပ်ကွက်များနှင့် ကန့်သတ်ချက်များအတွက် ကာနိုအမျိုးအစားများကို ပေးဆောင်ပါ။
- Serialization- အများသူငှာ အမျိုးအစားအားလုံးသည် Norito ကုဒ်ဒစ်များ (`norito::codec::{Encode, Decode}`) နှင့် schema (`iroha_schema::IntoSchema`) တို့မှ ဆင်းသက်လာသည်။ JSON ကို အင်္ဂါရပ်အလံများနောက်တွင် (ဥပမာ HTTP နှင့် `Json` ပေးချေမှုများအတွက်) ရွေးချယ်အသုံးပြုသည်။
- IVM မှတ်ချက်- Iroha Virtual Machine (IVM) ကို ပစ်မှတ်ထားသည့်အခါ အချို့သော ဖယ်ထုတ်ခြင်း-အချိန် အထောက်အထားများကို ပိတ်ထားပါသည်။
- FFI ဂိတ်များ- FFI မလိုအပ်သည့်အခါ အပေါ်မှရှောင်ရှားရန် အချို့သောအမျိုးအစားများကို `iroha_ffi` ၏နောက်တွင် `ffi_export`/`ffi_import` မှတစ်ဆင့် FFI အတွက် သတ်မှတ်အမှတ်အသားပြုထားသည်။

## အဓိကလက္ခဏာများနှင့် အထောက်အကူများ

- `Identifiable`- တည်ငြိမ်သော `Id` နှင့် `fn id(&self) -> &Self::Id` ရှိသည်။ မြေပုံ/အစုံ အဆင်ပြေစေရန်အတွက် `IdEqOrdHash` ဖြင့် ဆင်းသက်လာရပါမည်။
- `Registrable`/`Registered`- များစွာသော အရာများ (ဥပမာ၊ `Domain`, `AssetDefinition`, `Role`) တည်ဆောက်သူပုံစံကို အသုံးပြုသည်။ `Registered` သည် မှတ်ပုံတင်ခြင်းလုပ်ငန်းအတွက် သင့်လျော်သော ပေါ့ပါးသော တည်ဆောက်သူအမျိုးအစား (`With`) နှင့် ချိတ်ဆက်ထားသည်။
- `HasMetadata`- သော့/တန်ဖိုး `Metadata` မြေပုံသို့ တစ်စုတစ်စည်းတည်း ဝင်ရောက်ခွင့်။
- `IntoKeyValue`- ပွားမှုကို လျှော့ချရန် `Key` (ID) နှင့် `Value` (ဒေတာ) သီးခြားစီ သိမ်းဆည်းရန် သိုလှောင်မှု ခွဲခြမ်းအကူအညီပေးသူ။
- `Owned<T>`/`Ref<'world, K, V>`- မလိုအပ်သော မိတ္တူများကို ရှောင်ရှားရန် သိုလှောင်မှု နှင့် မေးမြန်းမှု စစ်ထုတ်မှုများတွင် အသုံးပြုသည့် ပေါ့ပါးသော ထုပ်ပိုးမှုများ။

## အမည်များနှင့် သတ်မှတ်ချက်များ

- `Name`- မှန်ကန်သော စာသားသတ်မှတ်မှု။ နေရာလွတ်နှင့် သီးသန့် စာလုံး `@`၊ `#`၊ `$` (ပေါင်းစပ် ID များတွင် သုံးသည်)။ အတည်ပြုချက်ဖြင့် `FromStr` မှတစ်ဆင့် တည်ဆောက်နိုင်သည်။ အမည်များကို ယူနီကုဒ် NFC ခွဲခြမ်းစိတ်ဖြာမှုတွင် ပုံမှန်ပြုလုပ်ထားသည် (တရားဝင်တူညီသော စာလုံးပေါင်းများကို ထပ်တူထပ်မျှနှင့် ပေါင်းစပ်သိမ်းဆည်းထားသည်)။ အထူးအမည် `genesis` ကို သီးသန့်ထားပါသည် (အသေးစိတ်စစ်ဆေးထားသည်ကို မသိရှိနိုင်)။
- `IdBox`- ပံ့ပိုးထားသော မည်သည့် ID အတွက်မဆို ပေါင်းလဒ်အမျိုးအစား စာအိတ် (`DomainId`, `AccountId`, `AssetDefinitionId`, `AssetId`, Norito, Norito, Norito, Norito `TriggerId`, `RoleId`, `Permission`, `CustomParameterId`)။ generic flows နှင့် Norito encoding အမျိုးအစားတစ်ခုတည်းအတွက် အသုံးဝင်သည်။
- `ChainId`- အရောင်းအ၀ယ်များတွင် ပြန်ဖွင့်ခြင်းအား အကာအကွယ်အတွက် အသုံးပြုသော ရောင်စုံကွင်းဆက်အမှတ်အသား။ID မျဉ်းပုံစံများ (`Display`/`FromStr` ဖြင့် အသွားအပြန်သုံးနိုင်သော)
- `DomainId`: `name` (e.g., `wonderland`)။
- `AccountId`- IH58၊ Sora compressed (`sora…`) နှင့် canonical hex codecs (`AccountAddress::to_ih58`, I180NI800X၊ I180NI800) မှတဆင့် `AccountAddress` မှတဆင့် ကုဒ်လုပ်ထားသော canonical identifier `canonical_hex`၊ `parse_encoded`)။ IH58 သည် ဦးစားပေးအကောင့်ဖော်မတ်ဖြစ်သည်။ `sora…` ဖောင်သည် Sora-only UX အတွက် ဒုတိယအကောင်းဆုံးဖြစ်သည်။ လူသားဆန်ဆန် လမ်းကြောင်းပြောင်းခြင်း alias `alias@domain` ကို UX အတွက် ထိန်းသိမ်းထားသော်လည်း တရားဝင်သတ်မှတ်မှုအဖြစ် မခံယူတော့ပါ။ Torii သည် `AccountAddress::parse_encoded` မှတဆင့် အဝင်လိုင်းများကို ပုံမှန်ဖြစ်စေသည်။ အကောင့် ID များသည် single-key နှင့် multisig controllers များကို ပံ့ပိုးပေးသည်။
- `AssetDefinitionId`: `asset#domain` (e.g., `xor#soramitsu`)။
- `AssetId`: canonical encoded literal `norito:<hex>` (legacy textual forms are not supported in first release).
- `NftId`: `nft$domain` (e.g., `rose$garden`)။
- `PeerId`: `public_key` (ရွယ်တူတန်းတူရေးသည် အများသူငှာသော့အားဖြင့်)။

## တစ်ခုနဲ့တစ်ခု

### ဒိုမိန်း
- `DomainId { name: Name }` - ထူးခြားသောအမည်။
- `Domain { id, logo: Option<IpfsPath>, metadata: Metadata, owned_by: AccountId }`။
- တည်ဆောက်သူ- `NewDomain`၊ `with_logo`၊ `with_metadata`၊ ထို့နောက် `Registrable::build(authority)` သည် `owned_by` ကို သတ်မှတ်ပေးသည်။

### အကောင့်
- `AccountId { domain: DomainId, controller: AccountController }` (controller = single key သို့မဟုတ် multisig policy)။
- `Account { id, metadata, label?, uaid? }` — `label` သည် rekey မှတ်တမ်းများအသုံးပြုသော ရွေးချယ်နိုင်ဖွယ်တည်ငြိမ်သော alias တစ်ခုဖြစ်ပြီး `uaid` သည် ရွေးချယ်နိုင်သော Nexus-wide [Universal Account ID](Norito).
- တည်ဆောက်သူ- `NewAccount` မှတဆင့် `Account::new(id)`; တည်ဆောက်သူနှင့် အဖွဲ့အစည်းနှစ်ခုလုံးအတွက် `HasMetadata`။

### ပိုင်ဆိုင်မှု အဓိပ္ပါယ်ဖွင့်ဆိုချက်များနှင့် ပိုင်ဆိုင်မှုများ
- `AssetDefinitionId { domain: DomainId, name: Name }`။
- `AssetDefinition { id, spec: NumericSpec, mintable: Mintable, logo: Option<IpfsPath>, metadata, owned_by: AccountId, total_quantity: Numeric }`။
  - `Mintable`: `Infinitely` | `Once` | `Limited(u32)` | `Not`။
  - တည်ဆောက်သူများ- `AssetDefinition::new(id, spec)` သို့မဟုတ် အဆင်ပြေစေရန် `numeric(id)`; `metadata`၊ `mintable`၊ `owned_by` အတွက် သတ်မှတ်ချက်များ။
- `AssetId { account: AccountId, definition: AssetDefinitionId }`။
- သိုလှောင်မှု အဆင်ပြေသော `AssetEntry`/`AssetValue` ပါသော `Asset { id, value: Numeric }`။
- အနှစ်ချုပ် APIs အတွက် `AssetTotalQuantityMap = BTreeMap<AssetDefinitionId, Numeric>` ကို ဖော်ထုတ်ထားသည်။

### NFTs
- `NftId { domain: DomainId, name: Name }`။
- `Nft { id, content: Metadata, owned_by: AccountId }` (အကြောင်းအရာသည် မတရားသောသော့/တန်ဖိုး မက်တာဒေတာဖြစ်သည်)။
- တည်ဆောက်သူ- `NewNft` မှတဆင့် `Nft::new(id, content)`။

### ရာထူးများနှင့် ခွင့်ပြုချက်များ
- `RoleId { name: Name }`။
- တည်ဆောက်သူ `NewRole { inner: Role, grant_to: AccountId }` နှင့်အတူ `Role { id, permissions: BTreeSet<Permission> }`။
- `Permission { name: Ident, payload: Json }` – `name` နှင့် payload schema သည် လက်ရှိအသုံးပြုနေသော `ExecutorDataModel` နှင့် ကိုက်ညီရမည် (အောက်တွင်ကြည့်ပါ)။

### သက်တူရွယ်တူ
- `PeerId { public_key: PublicKey }`။
- `Peer { address: SocketAddr, id: PeerId }` နှင့် parsable `public_key@address` စာတန်းပုံစံ။### ကူးယူဖော်ပြမှုများ (အင်္ဂါရပ် `sm`)
- `Sm2PublicKey` နှင့် `Sm2Signature`- SEC1-ကိုက်ညီသောအချက်များနှင့် SM2 အတွက် ပုံသေအနံ `r∥s` လက်မှတ်များ။ တည်ဆောက်သူများသည် မျဉ်းကွေးအသင်းဝင်မှုနှင့် ID များကို ခွဲခြားအတည်ပြုပေးသည်။ Norito ကုဒ်ကုဒ်သည် `iroha_crypto` အသုံးပြုထားသော canonical ကိုယ်စားပြုမှုကို ထင်ဟပ်စေသည်။
- `Sm3Hash`- `[u8; 32]` အမျိုးအစားသစ် GM/T 0004 အချေအတင်ကို ကိုယ်စားပြုသော၊ မန်နီးဖက်စ်များ၊ တယ်လီမီတာနှင့် syscall တုံ့ပြန်မှုများတွင် အသုံးပြုသည်။
- `Sm4Key`- 128-bit symmetric key wrapper သည် host syscalls နှင့် data-model fixtures များကြားတွင် မျှဝေထားသည်။
ဤအမျိုးအစားများသည် လက်ရှိ Ed25519/BLS/ML-DSA primitives များနှင့်အတူ ထိုင်ပြီး အလုပ်ခွင်နေရာကို `--features sm` ဖြင့် တည်ဆောက်ပြီးသည်နှင့် အများသူငှာ အစီအစဉ်၏ အစိတ်အပိုင်းဖြစ်လာပါသည်။

### အစပျိုးမှုများနှင့် ဖြစ်ရပ်များ
- `TriggerId { name: Name }` နှင့် `Trigger { id, action: action::Action }`။
- `action::Action { executable: Executable, repeats: Repeats, authority: AccountId, filter: EventFilterBox, metadata }`။
  - `Repeats`: `Indefinitely` သို့မဟုတ် `Exactly(u32)`; မှာယူခြင်းနှင့် သုံးစွဲမှု လျော့နည်းခြင်းတို့ ပါဝင်ပါသည်။
  - ဘေးကင်းရေး- `TriggerCompleted` ကို လုပ်ဆောင်ချက်၏ စစ်ထုတ်မှုတစ်ခုအဖြစ် အသုံးမပြုနိုင်ပါ ((de)serialization လုပ်နေစဉ်အတွင်း အတည်ပြုထားသည်)။
- `EventBox`- ပိုက်လိုင်း၊ ပိုက်လိုင်း-အသုတ်၊ ဒေတာ၊ အချိန်၊ execute-trigger နှင့် အစပျိုး-ပြီးမြောက်သော ဖြစ်ရပ်များအတွက် ပေါင်းစည်းအမျိုးအစား။ `EventFilterBox` သည် စာရင်းသွင်းမှုများနှင့် အစပျိုးခြင်းများအတွက် စစ်ထုတ်ခြင်းအတွက် မှန်သည်။

## သတ်မှတ်ချက်များနှင့် ဖွဲ့စည်းမှု

- စနစ်ပါရာမီတာ မိသားစုများ (`Default`ed အားလုံး၊ သယ်ယူသွားသူများ၊ တစ်ဦးချင်း enums အဖြစ်သို့ ပြောင်းပါ)။
- `SumeragiParameters { block_time_ms, commit_time_ms, min_finality_ms, pacing_factor_bps, max_clock_drift_ms, collectors_k, collectors_redundant_send_r }`။
  - `BlockParameters { max_transactions: NonZeroU64 }`။
  - `TransactionParameters { max_signatures, max_instructions, ivm_bytecode_size, max_tx_bytes, max_decompressed_bytes }`။
  - `SmartContractParameters { fuel, memory, execution_depth }`။
- `Parameters` သည် မိသားစုအားလုံးကို အုပ်စုဖွဲ့ပြီး `custom: BTreeMap<CustomParameterId, CustomParameter>` တစ်ခု။
- တစ်ခုတည်းသော ကန့်သတ်ချက်စာရင်းများ- `SumeragiParameter`၊ `BlockParameter`၊ `TransactionParameter`၊ `SmartContractParameter` မတူကွဲပြားသည့် အပ်ဒိတ်များနှင့် ထပ်တူထပ်မျှ ပြုလုပ်ခြင်းများအတွက်။
- စိတ်ကြိုက်ကန့်သတ်ဘောင်များ- `Json` အဖြစ် ယူဆောင်လာကာ `CustomParameterId` (`Name`) မှ သတ်မှတ်ပေးသော စီမံအုပ်ချုပ်သူ-သတ်မှတ်ထားသည်။

## ISI (Iroha အထူးညွှန်ကြားချက်များ)

- Core စရိုက်- `dyn_encode`၊ `as_any` နှင့် `Instruction`၊ နှင့် တည်ငြိမ်သောအမျိုးအစားအလိုက် သတ်မှတ်ထားသော `id()` (ကွန်ကရစ်အမျိုးအစားအမည်တွင် ပုံသေများ)။ ညွှန်ကြားချက်အားလုံးသည် `Send + Sync + 'static` ဖြစ်သည်။
- `InstructionBox`- အမျိုးအစား ID + ကုဒ်လုပ်ထားသော bytes မှတဆင့် အကောင်အထည်ဖော်ထားသော clone/eq/ord ပါရှိသော `Box<dyn Instruction>` wrapper
- Built-in ညွှန်ကြားချက် မိသားစုများကို အောက်ပါအတိုင်း ဖွဲ့စည်းထားပါသည်။
  - `mint_burn`၊ `transfer`၊ `register` နှင့် `transparent` အစုအဝေးတစ်ခု။
  - မက်တာစီးဆင်းမှုများအတွက် enums များကိုရိုက်ထည့်ပါ- `InstructionType`၊ `SetKeyValueBox` (domain/account/asset_def/nft/trigger) ကဲ့သို့သော အကွက်ပေါင်းများ။
- အမှားများ- `isi::error` အောက်တွင် ကြွယ်ဝသော အမှားပုံစံ (အကဲဖြတ်မှု အမျိုးအစားအမှားများ၊ အမှားများကို ရှာဖွေရန်၊ mintability၊ သင်္ချာ၊ မမှန်ကန်သော ကန့်သတ်ချက်များ၊ ထပ်တလဲလဲ၊ ပုံစံကွဲများ)။
- ညွှန်ကြားချက် မှတ်ပုံတင်ခြင်း- `instruction_registry!{ ... }` macro သည် အမျိုးအစားအမည်ဖြင့် သော့ခတ်ထားသော runtime decode registry တစ်ခုကို တည်ဆောက်သည်။ `InstructionBox` clone နှင့် Norito serde မှ dynamic (de)serialization ကိုအောင်မြင်ရန်အသုံးပြုသည်။ `set_instruction_registry(...)` မှတစ်ဆင့် မှတ်ပုံတင်ခြင်းအား အတိအလင်းသတ်မှတ်မထားပါက၊ core ISI အားလုံးပါရှိသည့် binaries ခိုင်မာစေရန် ပထမဦးစွာအသုံးပြုရာတွင် ပျင်းရိပျင်းရိစွာ ထည့်သွင်းထားပါသည်။

## ငွေလွှဲခြင်း။- `Executable`- `Instructions(ConstVec<InstructionBox>)` သို့မဟုတ် `Ivm(IvmBytecode)`။ `IvmBytecode` သည် Base64 (`Vec<u8>` ထက် ဖောက်ထွင်းမြင်ရသော အမျိုးအစားအသစ်) အဖြစ် အမှတ်အသားပြုပါသည်။
- `TransactionBuilder`- `chain`၊ `authority`၊ `creation_time_ms`၊ ချန်လှပ်ထားသော `time_to_live_ms` နှင့် Norito နှင့် Kotodama,I1018 `Executable`။
  - အကူအညီပေးသူများ- `with_instructions`, `with_bytecode`, `with_executable`, `with_metadata`, `set_nonce`, `set_ttl`, Norito
- `SignedTransaction` (`iroha_version` ဖြင့် ဗားရှင်းလုပ်ထားသည်) - `TransactionSignature` နှင့် payload ကိုသယ်ဆောင်သည်။ hashing နှင့် လက်မှတ်အတည်ပြုခြင်းကို ပေးသည်။
- ဝင်ခွင့်အမှတ်များနှင့် ရလဒ်များ
  - `TransactionEntrypoint`: `External(SignedTransaction)` | `Time(TimeTriggerEntrypoint)`။
  - `TransactionResult` = `Result<DataTriggerSequence, TransactionRejectionReason>` ကို hashing helpers။
  - `ExecutionStep(ConstVec<InstructionBox>)`- အရောင်းအဝယ်တစ်ခုတွင် မှာယူထားသော ညွှန်ကြားချက်များ အတွဲလိုက်တစ်ခု။

## တုံး

- `SignedBlock` (ဗားရှင်းဖြင့်) ထုပ်ပိုးထားသည်-
  - `signatures: BTreeSet<BlockSignature>` (တရားဝင်စစ်ဆေးသူများထံမှ)၊
  - `payload: BlockPayload { header: BlockHeader, transactions: Vec<SignedTransaction> }`၊
  - `result: BlockResult`၊ `time_triggers`၊ ဝင်/ရလဒ် Merkle သစ်ပင်၊ `transaction_results` နှင့် `fastpq_transcripts: BTreeMap<Hash, Vec<TransferTranscript>>` ပါရှိသော `result: BlockResult` (အလယ်တန်းလုပ်ဆောင်မှုအခြေအနေ)။
- အသုံးအဆောင်များ- `presigned`, `set_transaction_results(...)`, `set_transaction_results_with_transcripts(...)`, `header()`, `signatures()`, `hash()`, Kotodama.
- Merkle အမြစ်များ- အရောင်းအ၀ယ်ဝင်ရောက်မှုအမှတ်များနှင့် ရလဒ်များကို Merkle သစ်ပင်များမှတစ်ဆင့် လုပ်ဆောင်ပါသည်။ ရလဒ် Merkle root ကို block header တွင်ထည့်ထားသည်။
- ပါဝင်မှုဆိုင်ရာ အထောက်အထားများ (`BlockProofs`) သည် ဝင်ရောက်မှု/ရလဒ် Merkle အထောက်အထားများနှင့် `fastpq_transcripts` မြေပုံနှစ်ခုလုံးကို ဖော်ထုတ်နိုင်သောကြောင့် off-chain prover များသည် ငွေပေးငွေယူ hash တစ်ခုနှင့် ဆက်စပ်နေသော လွှဲပြောင်းသည့် မြစ်ဝကျွန်းပေါ်ဒေသများကို ထုတ်ယူနိုင်ပါသည်။
- `ExecWitness` မက်ဆေ့ဂျ်များ (Torii မှတဆင့် တိုက်ရိုက်ထုတ်လွှင့်ပြီး အများဆန္ဒအရ အတင်းအဖျင်းအလို့ငှာ ကျောထောက်နောက်ခံပြုထားသော) သည် ယခုအခါ `fastpq_transcripts` နှင့် မြှုပ်နှံထားသော `fastpq_batches: Vec<FastpqTransitionBatch>` ပါ၀င်သည် tx_set_hash) ထို့ကြောင့် ပြင်ပသက်သေများသည် စာသားမှတ်တမ်းများကို ပြန်လည်ကုဒ်သွင်းခြင်းမပြုဘဲ canonical FASTPQ အတန်းများကို ထည့်သွင်းနိုင်သည်။

## မေးခွန်းများ

- အရသာနှစ်မျိုး
  - Singular- `SingularQuery<Output>` (ဥပမာ၊ `FindParameters`၊ `FindExecutorDataModel`) ကို အကောင်အထည်ဖော်ပါ။
  - Iterable- `Query<Item>` (ဥပမာ၊ `FindAccounts`၊ `FindAssets`၊ `FindDomains` စသည်ဖြင့်) ကို အကောင်အထည်ဖော်ပါ။
- ရိုက်ဖျက်ထားသော ပုံစံများ-
  - `QueryBox<T>` သည် ကမ္ဘာလုံးဆိုင်ရာ မှတ်ပုံတင်ခြင်းမှ ကျောထောက်နောက်ခံပြုထားသော Norito serde ပါသော `Query<Item = T>` ပါသော ဖျက်ထားသော ဗူးတစ်ခုဖြစ်သည်။
  - `QueryWithFilter<T> { query, predicate, selector }` သည် မေးခွန်းတစ်ခုအား DSL ကြိုတင်သတ်မှတ်/ရွေးချယ်မှုဖြင့် တွဲပေးသည်။ `From` မှတစ်ဆင့် ဖျက်၍မရသော မေးခွန်းအဖြစ်သို့ ပြောင်းလဲသည်။
- Registry နှင့် codecs:
  - `query_registry!{ ... }` သည် ဒိုင်းနမစ်ကုဒ်အတွက် အမျိုးအစားအမည်ဖြင့် တည်ဆောက်သူများထံ ကွန်ကရစ်မေးခွန်းအမျိုးအစားများကို ကမ္ဘာလုံးဆိုင်ရာ registry mapping ပြုလုပ်ပေးသည်။
  - `QueryRequest = Singular(SingularQueryBox) | Start(QueryWithParams) | Continue(ForwardCursor)` နှင့် `QueryResponse = Singular(..) | Iterable(QueryOutput)`။
  - `QueryOutputBatchBox` သည် တစ်သားတည်းဖြစ်တည်နေသော vector များထက် ပေါင်းလဒ်အမျိုးအစား (ဥပမာ၊ `Vec<Account>`၊ `Vec<Name>`၊ `Vec<AssetDefinition>`၊ `Vec<BlockHeader>`) နှင့် tuple နှင့် တိုးချဲ့ကူညီသူများအတွက် ထိရောက်မှုရှိသည်။
- DSL- အချိန်-စစ်ဆေးထားသော ပရောဂျက်များနှင့် ရွေးချယ်သူများအတွက် ပရိုဂရမ်ဆိုင်ရာ လက္ခဏာများ (`HasProjection<PredicateMarker>` / `SelectorMarker`) ဖြင့် `query::dsl` တွင် အကောင်အထည်ဖော်ထားသည်။ `fast_dsl` လုပ်ဆောင်ချက်သည် လိုအပ်ပါက ပိုမိုပေါ့ပါးသော ဗားရှင်းကို ဖော်ထုတ်ပေးပါသည်။

## Executor နှင့် Extensibility- `Executor { bytecode: IvmBytecode }`- validator-executed code အတွဲ။
- `ExecutorDataModel { parameters: CustomParameters, instructions: BTreeSet<Ident>, permissions: BTreeSet<Ident>, schema: Json }` သည် executor-သတ်မှတ်ထားသောဒိုမိန်းကိုကြေငြာသည်-
  - စိတ်ကြိုက် configuration parameters တွေ၊
  - စိတ်ကြိုက် ညွှန်ကြားချက် သတ်မှတ်ချက်များ၊
  - ခွင့်ပြုချက်တိုကင် ခွဲခြားသတ်မှတ်မှုများ၊
  - client tooling အတွက် စိတ်ကြိုက်အမျိုးအစားများကို ဖော်ပြသည့် JSON schema တစ်ခု။
- စိတ်ကြိုက်ပြင်ဆင်ခြင်းနမူနာများသည် `data_model/samples/executor_custom_data_model` အောက်တွင် ရှိနေသည်-
  - စိတ်ကြိုက်ခွင့်ပြုချက်တိုကင် `iroha_executor_data_model::permission::Permission` မှတစ်ဆင့် ရယူခြင်း၊
  - `CustomParameter` သို့ ပြောင်းလဲနိုင်သော အမျိုးအစားအဖြစ် သတ်မှတ်ထားသော စိတ်ကြိုက် ကန့်သတ်ဘောင်၊
  - လုပ်ဆောင်ရန်အတွက် စိတ်ကြိုက်ညွှန်ကြားချက်များကို `CustomInstruction` တွင် နံပါတ်စဉ်တပ်ထားသည်။

### CustomInstruction (executor-သတ်မှတ်ထားသော ISI)

အမျိုးအစား- `isi::CustomInstruction { payload: Json }` တည်ငြိမ်သောဝါယာကြိုး ID `"iroha.custom"`။
- ရည်ရွယ်ချက်- အများသူငှာ ဒေတာပုံစံကို အတုမယူဘဲ သီးသန့်/လုပ်ငန်းစုကွန်ရက်များတွင် သို့မဟုတ် ပုံတူရိုက်ခြင်းအတွက် စီမံအုပ်ချုပ်သူ- သီးခြားညွှန်ကြားချက်များအတွက် စာအိတ်။
- မူရင်း executor အပြုအမူ- `iroha_core` တွင် တပ်ဆင်ထားသော executor သည် `CustomInstruction` ကို လုပ်ဆောင်ခြင်းမရှိသည့်အပြင် ကြုံတွေ့ပါက ထိတ်လန့်သွားမည်ဖြစ်သည်။ စိတ်ကြိုက်စီမံဆောင်ရွက်သူသည် `InstructionBox` ကို `CustomInstruction` သို့ ဒေါင်းလုဒ်လုပ်ပြီး မှန်ကန်သည့်စနစ်အားလုံးတွင် ပေးဆောင်မှုကို အဆုံးအဖြတ်ပေးရပါမည်။
- Norito- schema ပါ၀င်သော `norito::codec::{Encode, Decode}` မှတစ်ဆင့် ကုဒ်များ/စကားဝှက်များ `Json` payload ကို အပိုင်းလိုက်သတ်မှတ်ထားသည်။ ညွှန်ကြားချက် မှတ်ပုံတင်ခြင်းတွင် `CustomInstruction` ပါ၀င်နေသရွေ့ အသွားအပြန်ခရီးများသည် တည်ငြိမ်ပါသည်။
- IVM- Kotodama သည် IVM bytecode (`.to`) သို့ စုစည်းပြီး အပလီကေးရှင်းယုတ္တိဗေဒအတွက် အကြံပြုထားသော လမ်းကြောင်းဖြစ်သည်။ Kotodama တွင် ဖော်ပြမရနိုင်သေးသော executor-level extension များအတွက် `CustomInstruction` ကိုသာ အသုံးပြုပါ။ သက်တူရွယ်တူများတစ်လျှောက် အဆုံးအဖြတ်ပေးမှုနှင့် ထပ်တူထပ်မျှသော စီမံအုပ်ချုပ်သူ binaries များကို သေချာပါစေ။
- အများသူငှာ ကွန်ရက်များအတွက် မဟုတ်ပါ- ကွဲပြားခြားနားသော စီမံအုပ်ချုပ်သူများသည် အများသဘောတူမှု ခွဲထွက်နိုင်သည့် အန္တရာယ်ရှိသော အများသူငှာ ကွင်းဆက်များအတွက် အသုံးမပြုပါနှင့်။ ပလက်ဖောင်းအင်္ဂါရပ်များ လိုအပ်သောအခါတွင် ထည့်သွင်းထားသော ISI အသစ်ကို အဆိုပြုခြင်းကို ဦးစားပေးပါ။

## မက်တာဒေတာ

- `Metadata(BTreeMap<Name, Json>)`- သော့/တန်ဖိုးစတိုးဆိုင်များစွာ (`Domain`၊ `Account`၊ `AssetDefinition`၊ `Nft`၊ အစပျိုးမှုများ၊ နှင့် အရောင်းအဝယ်များ)။
- API: `contains`, `iter`, `get`, `insert`, နှင့် (`transparent_api`) `remove`။

## အင်္ဂါရပ်များနှင့် သတ်မှတ်ချက်များ

- စိတ်ကြိုက်ရွေးချယ်နိုင်သော API များ (`std`၊ `json`၊ `transparent_api`၊ `ffi_export`၊ `ffi_import`၊ `fast_dsl`၊ I1030NI030 `fault_injection`)။
- Determinism- စီးရီးလိုက်ပြုလုပ်ခြင်းအားလုံးသည် ဟာ့ဒ်ဝဲတစ်လျှောက် သယ်ဆောင်ရလွယ်ကူစေရန် Norito ကုဒ်နံပါတ်ကို အသုံးပြုသည်။ IVM bytecode သည် opaque byte blob တစ်ခုဖြစ်သည်။ ကွပ်မျက်မှုသည် အဆုံးအဖြတ်မဟုတ်သော လျှော့ချမှုများကို မိတ်ဆက်ခြင်းမပြုရပါ။ အိမ်ရှင်သည် IVM သို့ ငွေပေးငွေယူနှင့် သွင်းအားစုများကို တိကျစွာ အတည်ပြုသည်။

### Transparent API (`transparent_api`)- ရည်ရွယ်ချက်- Torii၊ စီမံဆောင်ရွက်သူများနှင့် ပေါင်းစပ်စမ်းသပ်မှုများကဲ့သို့သော အတွင်းပိုင်းအစိတ်အပိုင်းများအတွက် `#[model]` structs/enums သို့ အပြည့်အဝ၊ ပြောင်းလဲနိုင်သော ဝင်ရောက်ခွင့်ကို ဖော်ထုတ်ပေးပါသည်။ ၎င်းမရှိဘဲ၊ ထိုအရာများသည် ရည်ရွယ်ချက်ရှိရှိ အလင်းပေါက်နေသဖြင့် ပြင်ပ SDK များသည် ဘေးကင်းသော တည်ဆောက်သူများနှင့် ကုဒ်လုပ်ထားသော payload များကိုသာ မြင်နိုင်သည်။
- မက္ကင်းနစ်- `iroha_data_model_derive::model` မက်ခရိုသည် အများသူငှာအကွက်တစ်ခုစီကို `#[cfg(feature = "transparent_api")] pub` ဖြင့် ပြန်လည်ရေးသားပြီး မူရင်းတည်ဆောက်မှုအတွက် သီးသန့်မိတ္တူကို သိမ်းဆည်းထားသည်။ အင်္ဂါရပ်ကို ဖွင့်ထားခြင်းဖြင့် အဆိုပါ cfgs များကို လှန်လိုက်သောကြောင့် `Account`၊ `Domain`၊ `Asset` စသည်တို့ကို ၎င်းတို့၏ သတ်မှတ်သည့် module ပြင်ပတွင် တရားဝင်ဖြစ်သွားစေသည်။
- မျက်နှာပြင် ထောက်လှမ်းခြင်း- သေတ္တာသည် `TRANSPARENT_API: bool` အဆက်မပြတ် (`transparent_api.rs` သို့မဟုတ် `non_transparent_api.rs`) သို့ ထုတ်ပေးသည်။ Downstream ကုဒ်သည် opaque helpers သို့ပြန်တက်ရန် လိုအပ်သောအခါတွင် ဤအလံနှင့် အကိုင်းအခက်ကို စစ်ဆေးနိုင်သည်။
- ဖွင့်ခြင်း- `features = ["transparent_api"]` ကို `Cargo.toml` တွင် မှီခိုမှုသို့ ထည့်ပါ။ JSON projection (ဥပမာ၊ `iroha_torii`) လိုအပ်သော အလုပ်ခွင်သေတ္တာများ (ဥပမာ၊ `iroha_torii`) အလံကို အလိုအလျောက်ပေးပို့သော်လည်း၊ ပြင်ပအသုံးပြုသူများသည် ဖြန့်ကျက်မှုကို ထိန်းချုပ်ပြီး ပိုမိုကျယ်ပြန့်သော API မျက်နှာပြင်ကို လက်မခံပါက ၎င်းကို ပိတ်ထားသင့်သည်။

## အမြန်ဥပမာများ

ဒိုမိန်းနှင့် အကောင့်တစ်ခုဖန်တီးပါ၊ ပိုင်ဆိုင်မှုတစ်ခုကို သတ်မှတ်ပါ၊ ညွှန်ကြားချက်များဖြင့် ငွေပေးငွေယူတစ်ခု တည်ဆောက်ပါ-

```rust
use iroha_data_model::prelude::*;
use iroha_crypto::KeyPair;
use iroha_primitives::numeric::Numeric;

// Domain
let domain_id: DomainId = "wonderland".parse().unwrap();
let new_domain = Domain::new(domain_id.clone()).with_metadata(Metadata::default());

// Account
let kp = KeyPair::random();
let account_id = AccountId::new(domain_id.clone(), kp.public_key().clone());
let new_account = Account::new(account_id.clone()).with_metadata(Metadata::default());

// Asset definition and an asset for the account
let asset_def_id: AssetDefinitionId = "xor#wonderland".parse().unwrap();
let new_asset_def = AssetDefinition::numeric(asset_def_id.clone())
    .with_metadata(Metadata::default());
let asset_id = AssetId::new(asset_def_id.clone(), account_id.clone());
let asset = Asset::new(asset_id.clone(), Numeric::from(100));

// Build a transaction with instructions (pseudo-ISI; exact ISI types live under `isi`)
let chain_id: ChainId = "dev-chain".parse().unwrap();
let tx = TransactionBuilder::new(chain_id, account_id.clone())
    .with_instructions(vec![ /* Register/ Mint/ Transfer instructions here */ ])
    .sign(kp.private_key());
```

DSL ဖြင့် အကောင့်များနှင့် ပိုင်ဆိုင်မှုများကို မေးမြန်းပါ-

```rust
use iroha_data_model::prelude::*;

let predicate = query::dsl::CompoundPredicate::build(|p| {
    p.equals("metadata.tier", 1_u32)
        .exists("metadata.display_name")
});
let selector = query::dsl::SelectorTuple::default();
let q: QueryBox<QueryOutputBatchBox> =
    QueryWithFilter::new(
        Box::new(query::account::FindAccounts),
        predicate,
        selector,
    ).into();
// Encode and send via Torii; decode on server using the query registry
```

IVM စမတ်စာချုပ် ဘိုက်ကုဒ်ကို သုံးပါ-

```rust
use iroha_data_model::prelude::*;

let bytecode = IvmBytecode::from_compiled(include_bytes!("contract.to").to_vec());
let tx = TransactionBuilder::new("dev-chain".parse().unwrap(), account_id.clone())
    .with_bytecode(bytecode)
    .sign(kp.private_key());
```

## ဗားရှင်းပြောင်းခြင်း။

- `SignedTransaction`၊ `SignedBlock`၊ နှင့် `SignedQuery` တို့သည် canonical Norito-ကုဒ်လုပ်ထားသော တည်ဆောက်ပုံများ။ တစ်ခုစီသည် `EncodeVersioned` မှတစ်ဆင့် ကုဒ်လုပ်သောအခါ လက်ရှိ ABI ဗားရှင်း (လက်ရှိ `1`) နှင့် ၎င်းတို့၏ payload ကို ရှေ့ဆက်ရန် `iroha_version::Version` ကို အကောင်အထည်ဖော်သည်။

## မှတ်စုများ/ ဖြစ်နိုင်ချေရှိသော အပ်ဒိတ်များကို ပြန်လည်သုံးသပ်ပါ။

- Query DSL- တည်ငြိမ်သော အသုံးပြုသူမျက်နှာစာ အမျိုးအစားခွဲနှင့် ဘုံ filters/selectors အတွက် နမူနာများကို မှတ်တမ်းတင်ရန် စဉ်းစားပါ။
- ညွှန်ကြားချက်မိသားစုများ- `mint_burn`၊ `register`၊ `transfer` ဖြင့် ထိတွေ့ထားသော Built-in ISI မျိုးကွဲများကို ဖော်ပြသည့် အများသူငှာ စာရွက်စာတမ်းများကို ချဲ့ထွင်ပါ။

---
မည်သည့်အပိုင်းမှ ပိုမိုနက်ရှိုင်းမှု လိုအပ်နေပါက (ဥပမာ၊ ISI ကတ်တလောက် အပြည့်အစုံ၊ မေးမြန်းချက်စာရင်းအပြည့်အစုံ၊ သို့မဟုတ် ခေါင်းစီးအကွက်များကို ပိတ်ဆို့ခြင်း)၊ ကျွန်ုပ်အား အသိပေးပြီး ထိုအပိုင်းများကို လိုက်လျောညီထွေ တိုးချဲ့ပါမည်။