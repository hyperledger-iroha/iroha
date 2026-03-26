---
lang: uz
direction: ltr
source: docs/source/data_model.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 683bfb31442f8f4ce7b1bf5038f9dba92fe092545e655f43b51195c21535d3c4
source_last_modified: "2026-03-12T11:24:23.059339+00:00"
translation_last_reviewed: 2026-03-12
translator: machine-google-reviewed
---

# Iroha v2 ma'lumotlar modeli - Chuqur sho'ng'in

Ushbu hujjat `iroha_data_model` kassasida tatbiq etilgan va ish maydoni bo'ylab ishlatiladigan Iroha v2 ma'lumotlar modelini tashkil etuvchi tuzilmalar, identifikatorlar, belgilar va protokollarni tushuntiradi. Bu siz ko'rib chiqishingiz va yangilanishlarni taklif qilishingiz mumkin bo'lgan aniq ma'lumotnoma bo'lishi kerak.

## Ko'lami va asoslari

- Maqsad: Domen ob'ektlari (domenlar, hisoblar, aktivlar, NFTlar, rollar, ruxsatlar, tengdoshlar), holatni o'zgartiruvchi ko'rsatmalar (ISI), so'rovlar, triggerlar, tranzaksiyalar, bloklar va parametrlar uchun kanonik turlarni taqdim etish.
- Seriyalashtirish: Barcha ommaviy turlari Norito kodeklarini (`norito::codec::{Encode, Decode}`) va sxemasini (`iroha_schema::IntoSchema`) oladi. JSON funksiya bayroqlari ortida tanlab ishlatiladi (masalan, HTTP va `Json` foydali yuklari uchun).
- IVM eslatma: Iroha virtual mashinasini (IVM) maqsad qilganda ma'lum seriyasizlashtirish vaqtini tekshirish o'chirib qo'yiladi, chunki xost shartnomalarni chaqirishdan oldin tekshirishni amalga oshiradi (Norito-dagi kassa hujjatlariga qarang).
- FFI eshiklari: FFI kerak bo'lmaganda qo'shimcha xarajatlardan qochish uchun `ffi_export`/`ffi_import` orqasida `iroha_ffi` orqali FFI uchun ba'zi turlar shartli ravishda izohlanadi.

## Asosiy xususiyatlar va yordamchilar- `Identifiable`: ob'ektlar barqaror `Id` va `fn id(&self) -> &Self::Id`. Xarita/to'plam qulayligi uchun `IdEqOrdHash` bilan olingan bo'lishi kerak.
- `Registrable`/`Registered`: Ko'pgina ob'ektlar (masalan, `Domain`, `AssetDefinition`, `Role`) quruvchi naqshdan foydalanadi. `Registered` ish vaqti turini roʻyxatga olish tranzaktsiyalari uchun mos boʻlgan engil quruvchi turiga (`With`) bogʻlaydi.
- `HasMetadata`: kalit/qiymat `Metadata` xaritasiga yagona ruxsat.
- `IntoKeyValue`: takrorlanishni kamaytirish uchun `Key` (ID) va `Value` (ma'lumotlar)ni alohida saqlash uchun saqlashni ajratish yordamchisi.
- `Owned<T>`/`Ref<'world, K, V>`: Keraksiz nusxalarning oldini olish uchun saqlash joylarida va so'rov filtrlarida ishlatiladigan engil o'ramlar.

## Ismlar va identifikatorlar- `Name`: Yaroqli matn identifikatori. `@`, `#`, `$` (kompozit identifikatorlarda qo'llaniladi) bo'sh joy va ajratilgan belgilarga ruxsat bermaydi. Tekshirish bilan `FromStr` orqali tuzilishi mumkin. Ismlar tahlil qilishda Unicode NFC ga normallashtiriladi (kanonik ekvivalent imlolar bir xil deb hisoblanadi va tuzilgan holda saqlanadi). `genesis` maxsus nomi saqlangan (katta-kichik harflar bilan belgilanmagan).
- `IdBox`: Har qanday qoʻllab-quvvatlanadigan identifikator uchun yigʻma turdagi konvert (`DomainId`, `AccountId`, `AssetDefinitionId`, `AssetId`, `AssetId`, Kotodama, Kotodama `TriggerId`, `RoleId`, `Permission`, `CustomParameterId`). Umumiy oqimlar va yagona turdagi Norito kodlash uchun foydalidir.
- `ChainId`: tranzaktsiyalarda takroriy himoya qilish uchun foydalaniladigan noaniq zanjir identifikatori.Identifikatorlarning string shakllari (`Display`/`FromStr` bilan ikki tomonlama):
- `DomainId`: `name` (masalan, `wonderland`).
- `AccountId`: kanonik domensiz hisob identifikatori `AccountAddress` orqali faqat I105 sifatida kodlangan. Tahlil qiluvchi kirishlar kanonik I105 bo'lishi kerak; domen qo'shimchalari (`@domain`), kanonik i105 literallari, taxallus literallari, kanonik olti burchakli tahliliy kiritish, eski `norito:` foydali yuklari va `uaid:`/`opaque:` hisobi tahlil qilinadi.
- `AssetDefinitionId`: kanonik `unprefixed Base58 address with versioning and checksum` (UUID-v4 bayt).
- `AssetId`: kanonik kodlangan literal `<base58-asset-id>#<katakana-i105-account-id>` (eski matn shakllari birinchi versiyada qo'llab-quvvatlanmaydi).
- `NftId`: `nft$domain` (masalan, `rose$garden`).
- `PeerId`: `public_key` (teng tengligi ochiq kalit orqali amalga oshiriladi).

## ob'ektlar

### Domen
- `DomainId { name: Name }` - noyob nom.
- `Domain { id, logo: Option<SorafsUri>, metadata: Metadata, owned_by: AccountId }`.
- Quruvchi: `NewDomain`, `with_logo`, `with_metadata`, keyin `Registrable::build(authority)` to'plamlari `owned_by`.### Hisob
- `AccountId` bu kontroller tomonidan kalitlangan va kanonik I105 sifatida kodlangan kanonik domensiz hisob identifikatoridir.
- `ScopedAccountId { account: AccountId, domain: DomainId }` aniq domen kontekstini faqat qamrovli ko'rinish zarur bo'lganda olib boradi.
- `Account { id, metadata, label?, uaid? }` — `label` is an optional stable alias used by rekey records, `uaid` carries the optional Nexus-wide [Universal Account ID](./universal_accounts_guide.md).
- Quruvchi: `Account::new(id)` orqali `NewAccount`; ro'yxatdan o'tish aniq `ScopedAccountId` domenini talab qiladi va hech qanday sukut bo'yicha xulosa chiqarmaydi.

### Aktiv ta'riflari va aktivlar
- `AssetDefinitionId { aid_bytes: [u8; 16] }` matnda `unprefixed Base58 address` sifatida ochilgan.
- `AssetDefinition { id, name, description?, alias?, spec: NumericSpec, mintable: Mintable, logo: Option<SorafsUri>, metadata, owned_by: AccountId, total_quantity: Numeric }`.

  - Torii asset-definition responses may include `alias_binding { alias, status, lease_expiry_ms, grace_until_ms, bound_at_ms }`; alias selectors resolve against latest committed block time and stop resolving after grace, while direct reads may still show `expired_pending_cleanup` until sweep.
  - `name` insonga qaragan displey matni talab qilinadi va unda `#`/`@` bo'lmasligi kerak.
  - `alias` ixtiyoriy va quyidagilardan biri bo'lishi kerak:
    - `<name>#<domain>.<dataspace>`
    - `<name>#<dataspace>`
    chap segment bilan to'liq mos `AssetDefinition.name`.
  - `Mintable`: `Infinitely` | `Once` | `Limited(u32)` | `Not`.
  - Quruvchilar: `AssetDefinition::new(id, spec)` yoki qulaylik `numeric(id)`; `name` talab qilinadi va uni `.with_name(...)` orqali sozlash kerak.
- `AssetId { account: AccountId, definition: AssetDefinitionId, scope: AssetBalanceScope }`.
- `Asset { id, value: Numeric }` saqlash uchun qulay `AssetEntry`/`AssetValue`.
- `AssetBalanceScope`: cheklanmagan balanslar uchun `Global` va maʼlumotlar maydoni cheklangan balanslar uchun `Dataspace(DataSpaceId)`.
- `AssetTotalQuantityMap = BTreeMap<AssetDefinitionId, Numeric>` umumiy API uchun ochiq.### NFTs
- `NftId { domain: DomainId, name: Name }`.
- `Nft { id, content: Metadata, owned_by: AccountId }` (tarkib ixtiyoriy kalit/qiymat metama'lumotlari).
- Quruvchi: `Nft::new(id, content)` orqali `NewNft`.

### Rollar va ruxsatlar
- `RoleId { name: Name }`.
- `Role { id, permissions: BTreeSet<Permission> }` quruvchi `NewRole { inner: Role, grant_to: AccountId }` bilan.
- `Permission { name: Ident, payload: Json }` - `name` va foydali yuk sxemasi faol `ExecutorDataModel` bilan mos kelishi kerak (pastga qarang).

### Tengdoshlar
- `PeerId { public_key: PublicKey }`.
- `Peer { address: SocketAddr, id: PeerId }` va parsable `public_key@address` satr shakli.

### Kriptografik primitivlar (`sm` xususiyati)
- `Sm2PublicKey` va `Sm2Signature`: SM2 uchun SEC1-mos nuqtalar va belgilangan kenglikdagi `r∥s` imzolari. Konstruktorlar egri chiziq a'zoligini va farqlovchi identifikatorlarni tasdiqlaydi; Norito kodlash `iroha_crypto` tomonidan ishlatiladigan kanonik tasvirni aks ettiradi.
- `Sm3Hash`: GM/T 0004 dayjestini ifodalovchi `[u8; 32]` yangi turi, manifestlar, telemetriya va tizim javoblarida foydalaniladi.
- `Sm4Key`: 128-bitli simmetrik kalit oʻrami xost tizimlari va maʼlumotlar modeli moslamalari oʻrtasida taqsimlanadi.
Ushbu turlar mavjud Ed25519/BLS/ML-DSA primitivlari bilan birga joylashadi va ish maydoni `--features sm` bilan qurilganidan keyin umumiy sxemaning bir qismiga aylanadi.### Triggerlar va hodisalar
- `TriggerId { name: Name }` va `Trigger { id, action: action::Action }`.
- `action::Action { executable: Executable, repeats: Repeats, authority: AccountId, filter: EventFilterBox, metadata }`.
  - `Repeats`: `Indefinitely` yoki `Exactly(u32)`; buyurtma berish va tugatish uchun kommunal xizmatlar kiradi.
  - Xavfsizlik: `TriggerCompleted` amal filtri sifatida ishlatib bo'lmaydi (seriyalashtirish vaqtida tasdiqlangan).
- `EventBox`: quvur liniyasi, quvur liniyasi-to'plami, ma'lumotlar, vaqt, bajarish-trigger va ishga tushirish-tugallangan hodisalar uchun yig'indi turi; `EventFilterBox` obunalar va trigger filtrlarini aks ettiradi.

## Parametrlar va konfiguratsiya

- Tizim parametrlari oilalari (barcha `Default`ed, oluvchilarni olib yuradi va alohida raqamlarga aylantiriladi):
- `SumeragiParameters { block_time_ms, commit_time_ms, min_finality_ms, pacing_factor_bps, max_clock_drift_ms, collectors_k, collectors_redundant_send_r }`.
  - `BlockParameters { max_transactions: NonZeroU64 }`.
  - `TransactionParameters { max_signatures, max_instructions, ivm_bytecode_size, max_tx_bytes, max_decompressed_bytes }`.
  - `SmartContractParameters { fuel, memory, execution_depth }`.
- `Parameters` barcha oilalarni va `custom: BTreeMap<CustomParameterId, CustomParameter>` ni guruhlaydi.
- Yagona parametrli raqamlar: `SumeragiParameter`, `BlockParameter`, `TransactionParameter`, `SmartContractParameter` farqlarga o'xshash yangilanishlar va iteratsiya uchun.
- Maxsus parametrlar: ijrochi tomonidan belgilangan, `Json` sifatida olib boriladi, `CustomParameterId` (a `Name`) tomonidan aniqlangan.

## ISI (Iroha Maxsus ko'rsatmalar)- Asosiy xususiyat: `Instruction`, `dyn_encode`, `as_any` va har bir tur uchun barqaror identifikator `id()` (standart beton turi nomi uchun). Barcha ko'rsatmalar `Send + Sync + 'static`.
- `InstructionBox`: ID turi + kodlangan baytlar orqali amalga oshirilgan klon/ekv/ordli `Box<dyn Instruction>` o'ramiga tegishli.
- O'rnatilgan o'quv oilalari quyidagilar bo'yicha tashkil etiladi:
  - `mint_burn`, `transfer`, `register` va `transparent` yordamchilar to'plami.
  - Meta oqimlari uchun raqamlarni kiriting: `InstructionType`, `SetKeyValueBox` (domain/account/asset_def/nft/trigger) kabi qutiga solingan summalar.
- Xatolar: `isi::error` ostida boy xato modeli (baholash turidagi xatolar, xatolarni topish, zarb qilish, matematika, noto'g'ri parametrlar, takrorlash, invariantlar).
- Ko'rsatmalar reestri: `instruction_registry!{ ... }` makros turi nomi bilan kalitlangan ish vaqti dekodlash registrini yaratadi. Dinamik (de)seriyalashtirishga erishish uchun `InstructionBox` klon va Norito serde tomonidan foydalaniladi. Agar `set_instruction_registry(...)` orqali hech qanday reestr aniq o'rnatilmagan bo'lsa, ikkilik fayllarni mustahkam saqlash uchun birinchi foydalanishda barcha yadroli ISI bilan o'rnatilgan standart registr dangasalik bilan o'rnatiladi.

## tranzaktsiyalar- `Executable`: `Instructions(ConstVec<InstructionBox>)` yoki `Ivm(IvmBytecode)`. `IvmBytecode` base64 (`Vec<u8>` ustidagi shaffof yangi turdagi) sifatida seriyalashtiriladi.
- `TransactionBuilder`: `chain`, `authority`, `creation_time_ms`, ixtiyoriy `time_to_live_ms` va `nonce`, `nonce`, I012 va `chain` bilan tranzaksiya yukini tuzadi `Executable`.
  - Yordamchilar: `with_instructions`, `with_bytecode`, `with_executable`, `with_metadata`, `set_nonce`, `set_ttl`, `set_ttl`, Kotodama.
- `SignedTransaction` (`iroha_version` versiyasida): `TransactionSignature` va foydali yukni tashiydi; xeshlash va imzoni tekshirishni ta'minlaydi.
- Kirish nuqtalari va natijalar:
  - `TransactionEntrypoint`: `External(SignedTransaction)` | `Time(TimeTriggerEntrypoint)`.
  - `TransactionResult` = `Result<DataTriggerSequence, TransactionRejectionReason>` xeshlash yordamchilari bilan.
  - `ExecutionStep(ConstVec<InstructionBox>)`: tranzaktsiyadagi ko'rsatmalarning bitta buyurtma to'plami.

## Bloklar- `SignedBlock` (versiyali) kapsulalar:
  - `signatures: BTreeSet<BlockSignature>` (validatorlardan),
  - `payload: BlockPayload { header: BlockHeader, transactions: Vec<SignedTransaction> }`,
  - `result: BlockResult` (ikkilamchi bajarilish holati) `time_triggers`, kirish/natija Merkle daraxtlari, `transaction_results` va `fastpq_transcripts: BTreeMap<Hash, Vec<TransferTranscript>>`.
- Utilitalar: `presigned`, `set_transaction_results(...)`, `set_transaction_results_with_transcripts(...)`, `header()`, `signatures()`, `hash()`, `hash()`, Kotodama, I128NI00X00, Kotodama00.
- Merkle ildizlari: tranzaktsiyalar kirish nuqtalari va natijalari Merkle daraxtlari orqali amalga oshiriladi; natija Merkle ildizi blok sarlavhasiga joylashtiriladi.
- Blokni qo'shish dalillari (`BlockProofs`) ikkala kirish/natija Merkle dalillarini va `fastpq_transcripts` xaritasini ochib beradi, shuning uchun zanjirdan tashqari provayderlar tranzaksiya xesh bilan bog'liq transfer deltalarini olishlari mumkin.
- `ExecWitness` xabarlari (Torii orqali uzatiladi va konsensus g'iybatiga asoslangan) endi ikkala `fastpq_transcripts` va proverga tayyor `fastpq_batches: Vec<FastpqTransitionBatch>`, o'rnatilgan Kotodama, slotlari, perm_root, tx_set_hash), shuning uchun tashqi proverlar transkriptlarni qayta kodlamasdan kanonik FASTPQ qatorlarini qabul qilishlari mumkin.

## so'rovlar- Ikki xil lazzat:
  - Singular: amalga oshirish `SingularQuery<Output>` (masalan, `FindParameters`, `FindExecutorDataModel`).
  - Takrorlanishi mumkin: `Query<Item>`ni amalga oshirish (masalan, `FindAccounts`, `FindAssets`, `FindDomains` va boshqalar).
- turi o'chirilgan shakllar:
  - `QueryBox<T>` - global registr tomonidan qo'llab-quvvatlangan, Norito seriyali, o'chirilgan, o'chirilgan `Query<Item = T>`.
  - `QueryWithFilter<T> { query, predicate, selector }` so'rovni DSL predikati/selektori bilan bog'laydi; `From` orqali o'chirilgan takrorlanadigan so'rovga aylantiradi.
- Ro'yxatga olish kitobi va kodeklar:
  - `query_registry!{ ... }` konstruktorlarga dinamik dekodlash uchun tur nomi boʻyicha aniq soʻrov turlarini xaritalash global registrini yaratadi.
  - `QueryRequest = Singular(SingularQueryBox) | Start(QueryWithParams) | Continue(ForwardCursor)` va `QueryResponse = Singular(..) | Iterable(QueryOutput)`.
  - `QueryOutputBatchBox` bir hil vektorlar (masalan, `Vec<Account>`, `Vec<Name>`, `Vec<BlockHeader>`, `Vec<BlockHeader>`) ustidan yigʻindi turi, plyus kortej va paginatsiyalash yordamchilari uchun.
- DSL: kompilyatsiya vaqtida tekshiriladigan predikatlar va selektorlar uchun proyeksiya belgilari bilan (`HasProjection<PredicateMarker>` / `SelectorMarker`) `query::dsl` da joriy qilingan. Agar kerak bo'lsa, `fast_dsl` xususiyati engilroq variantni ochib beradi.

## Ijrochi va kengaytirilishi- `Executor { bytecode: IvmBytecode }`: validator tomonidan bajariladigan kodlar to'plami.
- `ExecutorDataModel { parameters: CustomParameters, instructions: BTreeSet<Ident>, permissions: BTreeSet<Ident>, schema: Json }` ijrochi tomonidan belgilangan domenni e'lon qiladi:
  - Maxsus konfiguratsiya parametrlari,
  - Shaxsiy yo'riqnoma identifikatorlari,
  - Ruxsat belgisi identifikatorlari,
  - Mijoz asboblari uchun maxsus turlarni tavsiflovchi JSON sxemasi.
- Moslashtirish namunalari `data_model/samples/executor_custom_data_model` ostida mavjud bo'lib, quyidagilarni ko'rsatadi:
  - `iroha_executor_data_model::permission::Permission` olish orqali maxsus ruxsat belgisi,
  - `CustomParameter` ga konvertatsiya qilinadigan tur sifatida belgilangan maxsus parametr,
  - Bajarish uchun `CustomInstruction` ga seriyalashtirilgan maxsus ko'rsatmalar.

### Custom Instruction (ijrochi tomonidan belgilangan ISI)- Turi: `isi::CustomInstruction { payload: Json }` barqaror sim identifikatori `"iroha.custom"`.
- Maqsad: xususiy/konsorsium tarmoqlarida ijrochiga oid ko'rsatmalar yoki umumiy ma'lumotlar modelini ajratmasdan prototiplash uchun konvert.
- Ijrochining standart xatti-harakati: `iroha_core` da o'rnatilgan ijrochi `CustomInstruction` ni bajarmaydi va agar duch kelsa vahima qo'zg'atadi. Maxsus ijrochi `InstructionBox` ni `CustomInstruction` ga tushirishi va barcha validatorlardagi foydali yukni aniq talqin qilishi kerak.
- Norito: sxema kiritilgan `norito::codec::{Encode, Decode}` orqali kodlaydi/dekodlaydi; `Json` foydali yuki deterministik tarzda ketma-ketlashtiriladi. Yo'riqnomalar reestrida `CustomInstruction` (bu standart registrning bir qismi) bo'lsa, aylanma safarlar barqaror bo'ladi.
- IVM: Kotodama IVM baytekodiga (`.to`) kompilyatsiya qilinadi va dastur mantig'i uchun tavsiya etilgan yo'ldir. `CustomInstruction` dan faqat Kotodama da ifodalab bo'lmaydigan ijrochi darajasidagi kengaytmalar uchun foydalaning. Tengdoshlar orasida determinizm va bir xil ijrochi binarlarini ta'minlang.
- Umumiy tarmoqlar uchun emas: turli xil ijrochilar konsensus vilkalari xavfini tug'diradigan jamoat zanjirlari uchun foydalanmang. Platforma funksiyalari kerak boʻlganda yangi oʻrnatilgan ISI yuqori oqimini taklif qilishni afzal koʻring.

## Metadata- `Metadata(BTreeMap<Name, Json>)`: bir nechta ob'ektlarga biriktirilgan kalit/qiymat do'koni (`Domain`, `Account`, `AssetDefinition`, `Nft`, triggerlar va tranzaksiyalar).
- API: `contains`, `iter`, `get`, `insert` va (`transparent_api` bilan) `remove`.

## Xususiyatlar va determinizm

- Xususiyatlar ixtiyoriy API-larni boshqaradi (`std`, `json`, `transparent_api`, `ffi_export`, `ffi_import`, `fast_dsl`, `fast_dsl`, I1808 `fault_injection`).
- Determinizm: Barcha ketma-ketlashtirish apparat bo'ylab ko'chma bo'lishi uchun Norito kodlashdan foydalanadi. IVM bayt kodi shaffof bo'lmagan bayt blokidir; ijro deterministik bo'lmagan qisqartirishlarni kiritmasligi kerak. Xost tranzaktsiyalarni tasdiqlaydi va IVM ga aniqlik bilan kirishlarni taqdim etadi.

### Shaffof API (`transparent_api`)- Maqsad: Torii, ijrochilar va integratsiya testlari kabi ichki komponentlar uchun `#[model]` tuzilmalariga/enumlariga toʻliq, oʻzgaruvchan kirishni ochib beradi. Busiz, bu elementlar ataylab shaffof emas, shuning uchun tashqi SDKlar faqat xavfsiz konstruktorlar va kodlangan foydali yuklarni ko'radi.
- Mexanika: `iroha_data_model_derive::model` makrosi har bir umumiy maydonni `#[cfg(feature = "transparent_api")] pub` bilan qayta yozadi va standart tuzilish uchun shaxsiy nusxasini saqlaydi. Xususiyatni yoqish ushbu cfg-larni aylantiradi, shuning uchun `Account`, `Domain`, `Asset` va boshqalarni destruksiya qilish ularning aniqlovchi modullaridan tashqari qonuniy bo'ladi.
- Yuzaki aniqlash: sandiq `TRANSPARENT_API: bool` konstantasini eksport qiladi (`transparent_api.rs` yoki `non_transparent_api.rs` formatida ishlab chiqariladi). Pastki oqim kodi bu bayroqni tekshirishi va noaniq yordamchilarga qaytishi kerak bo'lganda filialni tekshirishi mumkin.
- Yoqish: `Cargo.toml` dagi qaramlikka `features = ["transparent_api"]` ni qo'shing. JSON proyeksiyasiga (masalan, `iroha_torii`) kerak bo'lgan ish maydoni qutilari bayroqni avtomatik ravishda yo'naltiradi, ammo uchinchi tomon iste'molchilari joylashtirishni nazorat qilmasa va kengroq API yuzasini qabul qilmasa, uni o'chirib qo'yishi kerak.

## Tezkor misollar

Domen va hisob yarating, aktivni aniqlang va ko'rsatmalar bilan tranzaksiya tuzing:

```rust
use iroha_data_model::prelude::*;
use iroha_crypto::KeyPair;
use iroha_primitives::numeric::Numeric;

// Domain
let domain_id: DomainId = "wonderland".parse().unwrap();
let new_domain = Domain::new(domain_id.clone()).with_metadata(Metadata::default());

// Account
let kp = KeyPair::random();
let account_id = AccountId::new(kp.public_key().clone());
let new_account = Account::new(account_id.to_account_id(domain_id.clone()))
    .with_metadata(Metadata::default());

// Asset definition and an asset for the account
let asset_def_id: AssetDefinitionId = "66owaQmAQMuHxPzxUN3bqZ6FJfDa".parse().unwrap();
let new_asset_def = AssetDefinition::numeric(asset_def_id.clone())
    .with_name("USD Coin".to_owned())
    .with_metadata(Metadata::default());
let asset_id = AssetId::new(asset_def_id.clone(), account_id.clone());
let asset = Asset::new(asset_id.clone(), Numeric::from(100));

// Build a transaction with instructions (pseudo-ISI; exact ISI types live under `isi`)
let chain_id: ChainId = "dev-chain".parse().unwrap();
let tx = TransactionBuilder::new(chain_id, account_id.clone())
    .with_instructions(vec![ /* Register/ Mint/ Transfer instructions here */ ])
    .sign(kp.private_key());
```

DSL bilan hisoblar va aktivlarni so'rang:

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

IVM aqlli shartnoma baytekodidan foydalaning:

```rust
use iroha_data_model::prelude::*;

let bytecode = IvmBytecode::from_compiled(include_bytes!("contract.to").to_vec());
let tx = TransactionBuilder::new("dev-chain".parse().unwrap(), account_id.clone())
    .with_bytecode(bytecode)
    .sign(kp.private_key());
```

asset-definition id / taxallusning tezkor ma'lumotnomasi (CLI + Torii):

```bash
# Register an asset definition with canonical Base58 id + explicit name + alias
iroha ledger asset definition register \
  --id 66owaQmAQMuHxPzxUN3bqZ6FJfDa \
  --name pkr \
  --alias pkr#ubl.sbp

# Short alias form (no owner segment): <name>#<dataspace>
iroha ledger asset definition register \
  --id 66owaQmAQMuHxPzxUN3bqZ6FJfDa \
  --name pkr \
  --alias pkr#sbp

# Mint using alias + account components (no manual norito hex copy/paste)
iroha ledger asset mint \
  --definition-alias pkr#ubl.sbp \
  --account sorauﾛ1P... \
  --quantity 500

# Resolve alias to canonical Base58 id via Torii
curl -sS http://127.0.0.1:8080/v1/assets/aliases/resolve \
  -H 'content-type: application/json' \
  -d '{"alias":"pkr#ubl.sbp"}'
```Migratsiya eslatmasi:
- Eski `name#domain` aktivlarni aniqlash identifikatorlari v1 da qabul qilinmaydi.
- Yalpiz/yoqish/o'tkazish uchun aktiv identifikatorlari `<base58-asset-id>#<katakana-i105-account-id>` kanonik bo'lib qoladi; ularni yarating:
  - `iroha tools encode asset-id --definition <base58-asset-definition-id> --account <i105>`
  - yoki `--alias <name>#<domain>.<dataspace>` / `--alias <name>#<dataspace>` + `--account`.

## Versiyalash

- `SignedTransaction`, `SignedBlock` va `SignedQuery` kanonik Norito kodlangan tuzilmalardir. Ularning har biri `EncodeVersioned` orqali kodlanganda foydali yukini joriy ABI versiyasi (hozirda `1`) bilan prefikslash uchun `iroha_version::Version` ni qo'llaydi.

## Ko'rib chiqish eslatmalari / Potentsial yangilanishlar

- DSL so'rovi: barqaror foydalanuvchiga qarashli kichik to'plamni hujjatlashtirish va umumiy filtrlar/selektorlar uchun misollarni ko'rib chiqing.
- Ko'rsatmalar oilalari: `mint_burn`, `register`, `transfer` tomonidan o'rnatilgan ISI variantlari ro'yxatini ko'rsatadigan ommaviy hujjatlarni kengaytiring.

---
Agar biron bir qism chuqurroq bo'lishi kerak bo'lsa (masalan, to'liq ISI katalogi, to'liq so'rovlar ro'yxati yoki blok sarlavhalari maydonlari), menga xabar bering va men ushbu bo'limlarni mos ravishda kengaytiraman.