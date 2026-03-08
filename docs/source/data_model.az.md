---
lang: az
direction: ltr
source: docs/source/data_model.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8b6388355a41797eb7d0b7f47cfa8fcac4e136c5a2e5eb0a264384ecdba930b8
source_last_modified: "2026-02-01T13:51:49.945202+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Iroha v2 Data Modeli – Dərin Dalış

Bu sənəd `iroha_data_model` qutusunda həyata keçirilən və iş sahəsində istifadə edilən Iroha v2 data modelini təşkil edən strukturları, identifikatorları, əlamətləri və protokolları izah edir. Bu, nəzərdən keçirə və yeniləmələr təklif edə biləcəyiniz dəqiq istinad üçün nəzərdə tutulub.

## Əhatə və Əsaslar

- Məqsəd: Domen obyektləri (domenlər, hesablar, aktivlər, NFTlər, rollar, icazələr, həmyaşıdlar), vəziyyəti dəyişən təlimatlar (ISI), sorğular, tetikleyiciler, əməliyyatlar, bloklar və parametrlər üçün kanonik növləri təmin etmək.
- Serializasiya: Bütün ictimai növlər Norito kodekləri (`norito::codec::{Encode, Decode}`) və sxemi (`iroha_schema::IntoSchema`) əldə edir. JSON xüsusiyyət bayraqlarının arxasında selektiv şəkildə istifadə olunur (məsələn, HTTP və `Json` yükləri üçün).
- IVM qeyd: Iroha Virtual Maşını (IVM) hədəfləyərkən müəyyən seriyasızlaşdırma vaxtı yoxlamaları deaktiv edilir, çünki host müqavilələri işə salmazdan əvvəl yoxlama aparır (Norito-də sandıq sənədlərinə baxın).
- FFI qapıları: Bəzi növlər FFI tələb olunmadıqda yuxarı yükdən qaçmaq üçün `ffi_export`/`ffi_import`-in arxasında `iroha_ffi` vasitəsilə FFI üçün şərti olaraq qeyd olunur.

## Əsas xüsusiyyətlər və köməkçilər

- `Identifiable`: Müəssisələrdə sabit `Id` və `fn id(&self) -> &Self::Id` var. Xəritə/set dostluğu üçün `IdEqOrdHash` ilə əldə edilməlidir.
- `Registrable`/`Registered`: Bir çox obyekt (məsələn, `Domain`, `AssetDefinition`, `Role`) qurucu nümunəsindən istifadə edir. `Registered` iş vaxtı növünü qeydiyyat əməliyyatları üçün uyğun olan yüngül qurucu tipinə (`With`) bağlayır.
- `HasMetadata`: Açar/dəyər `Metadata` xəritəsinə vahid giriş.
- `IntoKeyValue`: Təkrarlanmanı azaltmaq üçün `Key` (ID) və `Value` (məlumat) ayrı-ayrılıqda saxlamaq üçün yaddaş bölməsi köməkçisi.
- `Owned<T>`/`Ref<'world, K, V>`: Lazımsız nüsxələrin qarşısını almaq üçün anbarlarda və sorğu filtrlərində istifadə olunan yüngül sarğılar.

## Adlar və İdentifikatorlar

- `Name`: Etibarlı mətn identifikatoru. Boşluq və qorunan simvollara icazə vermir `@`, `#`, `$` (kompozit ID-lərdə istifadə olunur). Doğrulama ilə `FromStr` vasitəsilə tikilə bilər. Adlar təhlil zamanı Unicode NFC ilə normallaşdırılır (kanonik ekvivalent yazılar eyni hesab olunur və yığılmış şəkildə saxlanılır). `genesis` xüsusi adı qorunur (hərf hərfinə həssaslıqla yoxlanılır).
- `IdBox`: Dəstəklənən hər hansı ID (`DomainId`, `AccountId`, `AssetDefinitionId`, `AssetId`, `AssetId`, Kotodama, Kotodama, Kotodama, `DomainId`, `DomainId`, Kotodama) üçün cəmi tipli zərf `TriggerId`, `RoleId`, `Permission`, `CustomParameterId`). Ümumi axınlar və tək bir növ kimi Norito kodlaşdırması üçün faydalıdır.
- `ChainId`: Əməliyyatlarda təkrar qorunma üçün istifadə edilən qeyri-şəffaf zəncir identifikatoru.İdentifikatorların sətir formaları (`Display`/`FromStr` ilə gediş-gəliş):
- `DomainId`: `name` (məsələn, `wonderland`).
- `AccountId`: IH58, Sora sıxılmış (`sora…`) və kanonik hex kodekləri (`AccountAddress::to_ih58`, I100080X, I1008030) ifşa edən `AccountAddress` vasitəsilə kodlanmış kanonik identifikator `canonical_hex`, `parse_encoded`). IH58 üstünlük verilən hesab formatıdır; `sora…` forması yalnız Sora üçün UX üçün ikinci ən yaxşısıdır. İnsanlara uyğun marşrutlaşdırma ləqəbi `alias` (rejected legacy form) UX üçün qorunur, lakin artıq səlahiyyətli identifikator kimi qəbul edilmir. Torii `AccountAddress::parse_encoded` vasitəsilə daxil olan sətirləri normallaşdırır. Hesab identifikatorları həm tək açarlı, həm də multisig nəzarətçiləri dəstəkləyir.
- `AssetDefinitionId`: `asset#domain` (məsələn, `xor#soramitsu`).
- `AssetId`: canonical encoded literal `norito:<hex>` (legacy textual forms are not supported in first release).
- `NftId`: `nft$domain` (məsələn, `rose$garden`).
- `PeerId`: `public_key` (peer bərabərliyi açıq açarladır).

## Müəssisələr

### Domen
- `DomainId { name: Name }` – unikal ad.
- `Domain { id, logo: Option<IpfsPath>, metadata: Metadata, owned_by: AccountId }`.
- Qurucu: `NewDomain` ilə `with_logo`, `with_metadata`, sonra `Registrable::build(authority)` dəstləri `owned_by`.

### Hesab
- `AccountId { domain: DomainId, controller: AccountController }` (nəzarətçi = tək açar və ya multisig siyasəti).
- `Account { id, metadata, label?, uaid? }` — `label` yenidən açar qeydlər tərəfindən istifadə edilən isteğe bağlı sabit ləqəbdir, `uaid` genişmiqyaslı isteğe bağlı Nexus [Universal Hesab ID](Kotodama) daşıyır.
- Qurucu: `Account::new(id)` vasitəsilə `NewAccount`; Həm inşaatçı, həm də müəssisə üçün `HasMetadata`.

### Aktiv anlayışları və aktivlər
- `AssetDefinitionId { domain: DomainId, name: Name }`.
- `AssetDefinition { id, spec: NumericSpec, mintable: Mintable, logo: Option<IpfsPath>, metadata, owned_by: AccountId, total_quantity: Numeric }`.
  - `Mintable`: `Infinitely` | `Once` | `Limited(u32)` | `Not`.
  - İnşaatçılar: `AssetDefinition::new(id, spec)` və ya rahatlıq `numeric(id)`; `metadata`, `mintable`, `owned_by` üçün təyinedicilər.
- `AssetId { account: AccountId, definition: AssetDefinitionId }`.
- `Asset { id, value: Numeric }`, saxlama üçün əlverişli `AssetEntry`/`AssetValue`.
- `AssetTotalQuantityMap = BTreeMap<AssetDefinitionId, Numeric>` xülasə API-ləri üçün açıqdır.

### NFTs
- `NftId { domain: DomainId, name: Name }`.
- `Nft { id, content: Metadata, owned_by: AccountId }` (məzmun ixtiyari açar/dəyər metadatasıdır).
- Qurucu: `Nft::new(id, content)` vasitəsilə `NewNft`.

### Rollar və İcazələr
- `RoleId { name: Name }`.
- `Role { id, permissions: BTreeSet<Permission> }` qurucusu ilə `NewRole { inner: Role, grant_to: AccountId }`.
- `Permission { name: Ident, payload: Json }` – `name` və faydalı yük sxemi aktiv `ExecutorDataModel` ilə uyğunlaşdırılmalıdır (aşağıya bax).

### Həmyaşıdları
- `PeerId { public_key: PublicKey }`.
- `Peer { address: SocketAddr, id: PeerId }` və parsable `public_key@address` simli forması.### Kriptoqrafik primitivlər (Xüsusiyyət `sm`)
- `Sm2PublicKey` və `Sm2Signature`: SM2 üçün SEC1 uyğun nöqtələr və sabit enli `r∥s` imzaları. Konstruktorlar əyri üzvlüyü və fərqləndirici identifikatorları təsdiqləyir; Norito kodlaması `iroha_crypto` tərəfindən istifadə edilən kanonik təsviri əks etdirir.
- `Sm3Hash`: GM/T 0004 həzmini təmsil edən `[u8; 32]` yeni tip, manifestlər, telemetriya və sistem çağırışı cavablarında istifadə olunur.
- `Sm4Key`: 128 bitlik simmetrik açar sarğısı host sistemləri və verilənlər modeli qurğuları arasında paylaşılır.
Bu növlər mövcud Ed25519/BLS/ML-DSA primitivləri ilə yanaşı oturur və iş sahəsi `--features sm` ilə qurulduqdan sonra ictimai sxemin bir hissəsinə çevrilir.

### Tətiklər və Hadisələr
- `TriggerId { name: Name }` və `Trigger { id, action: action::Action }`.
- `action::Action { executable: Executable, repeats: Repeats, authority: AccountId, filter: EventFilterBox, metadata }`.
  - `Repeats`: `Indefinitely` və ya `Exactly(u32)`; sifariş və tükənmə kommunalları daxildir.
  - Təhlükəsizlik: `TriggerCompleted` əməliyyat filtri kimi istifadə edilə bilməz (seriyalaşdırma zamanı təsdiq edilmişdir).
- `EventBox`: boru kəməri, boru kəməri toplusu, verilənlər, vaxt, icra-tetikleyici və tətiklə tamamlanan hadisələr üçün cəmi növü; `EventFilterBox` abunəliklər və trigger filtrləri üçün əks etdirir.

## Parametrlər və Konfiqurasiya

- Sistem parametr ailələri (bütün `Default`ed, alıcıları daşıyır və fərdi nömrələrə çevirir):
- `SumeragiParameters { block_time_ms, commit_time_ms, min_finality_ms, pacing_factor_bps, max_clock_drift_ms, collectors_k, collectors_redundant_send_r }`.
  - `BlockParameters { max_transactions: NonZeroU64 }`.
  - `TransactionParameters { max_signatures, max_instructions, ivm_bytecode_size, max_tx_bytes, max_decompressed_bytes }`.
  - `SmartContractParameters { fuel, memory, execution_depth }`.
- `Parameters` bütün ailələri və `custom: BTreeMap<CustomParameterId, CustomParameter>` qruplarını qruplaşdırır.
- Tək parametrli nömrələr: `SumeragiParameter`, `BlockParameter`, `TransactionParameter`, `SmartContractParameter` fərq kimi yeniləmələr və iterasiya üçün.
- Fərdi parametrlər: icraçı tərəfindən müəyyən edilib, `Json` kimi daşınır, `CustomParameterId` (a `Name`) tərəfindən müəyyən edilir.

## ISI (Iroha Xüsusi Təlimatlar)

- Əsas xüsusiyyət: `Instruction`, `dyn_encode`, `as_any` və hər növ üçün sabit identifikator `id()` (defolt olaraq konkret tip adına uyğundur). Bütün təlimatlar `Send + Sync + 'static`-dir.
- `InstructionBox`: Klon/eq/ord ilə sahib `Box<dyn Instruction>` sarğı ID növü + kodlanmış baytlar vasitəsilə həyata keçirilir.
- Quraşdırılmış təlimat ailələri aşağıdakılar əsasında təşkil edilir:
  - `mint_burn`, `transfer`, `register` və `transparent` köməkçilər dəsti.
  - Meta axınları üçün nömrələri yazın: `InstructionType`, `SetKeyValueBox` (domain/account/asset_def/nft/trigger) kimi qutulu məbləğlər.
- Səhvlər: `isi::error` altında zəngin səhv modeli (qiymətləndirmə növü səhvləri, tapma səhvləri, hesablama qabiliyyəti, riyaziyyat, etibarsız parametrlər, təkrar, dəyişməzlik).
- Təlimat reyestri: `instruction_registry!{ ... }` makrosu növ adı ilə əsaslanan iş vaxtı deşifrə reyestrini qurur. Dinamik (de)serializasiyaya nail olmaq üçün `InstructionBox` klonu və Norito serde tərəfindən istifadə olunur. `set_instruction_registry(...)` vasitəsilə heç bir reyestr açıq şəkildə qurulmayıbsa, ikili faylları möhkəm saxlamaq üçün bütün əsas ISI ilə daxili standart reyestr ilk istifadədə tənbəlliklə quraşdırılır.

## Əməliyyatlar- `Executable`: ya `Instructions(ConstVec<InstructionBox>)`, ya da `Ivm(IvmBytecode)`. `IvmBytecode` base64 kimi seriallaşdırılır (`Vec<u8>` üzərində şəffaf yeni tip).
- `TransactionBuilder`: `chain`, `authority`, `creation_time_ms`, isteğe bağlı `time_to_live_ms` və `nonce`, I010X, I018 və I018 ilə əməliyyat yükü qurur `Executable`.
  - Köməkçilər: `with_instructions`, `with_bytecode`, `with_executable`, `with_metadata`, `set_nonce`, `set_ttl`, `set_ttl`, Kotodama.
- `SignedTransaction` (`iroha_version` ilə versiya): `TransactionSignature` və faydalı yük daşıyır; hashing və imza yoxlamasını təmin edir.
- Giriş nöqtələri və nəticələr:
  - `TransactionEntrypoint`: `External(SignedTransaction)` | `Time(TimeTriggerEntrypoint)`.
  - Hashing köməkçiləri ilə `TransactionResult` = `Result<DataTriggerSequence, TransactionRejectionReason>`.
  - `ExecutionStep(ConstVec<InstructionBox>)`: əməliyyatda təlimatların tək sifarişli toplusu.

## Bloklar

- `SignedBlock` (versiya) əhatə edir:
  - `signatures: BTreeSet<BlockSignature>` (validatorlardan),
  - `payload: BlockPayload { header: BlockHeader, transactions: Vec<SignedTransaction> }`,
  - `result: BlockResult` (ikinci icra vəziyyəti), `time_triggers`, giriş/nəticə Merkle ağacları, `transaction_results` və `fastpq_transcripts: BTreeMap<Hash, Vec<TransferTranscript>>`.
- Kommunal xidmətlər: `presigned`, `set_transaction_results(...)`, `set_transaction_results_with_transcripts(...)`, `header()`, `signatures()`, `hash()`, `hash()`, Kotodama.
- Merkle kökləri: əməliyyat giriş nöqtələri və nəticələr Merkle ağacları vasitəsilə həyata keçirilir; nəticədə Merkle kökü blok başlığına yerləşdirilir.
- Blok daxiletmə sübutları (`BlockProofs`) həm giriş/nəticə Merkle sübutlarını, həm də `fastpq_transcripts` xəritəsini ifşa edir ki, zəncirdən kənar provayderlər tranzaksiya hash ilə əlaqəli transfer deltalarını əldə edə bilsinlər.
- `ExecWitness` mesajları (Torii vasitəsilə yayımlanır və konsensus dedi-qoduları ilə dəstəklənir) indi həm `fastpq_transcripts`, həm də daxil edilmiş kökləri olan `fastpq_batches: Vec<FastpqTransitionBatch>` (`fastpq_batches: Vec<FastpqTransitionBatch>`, yuvaları, yuvaları, Kotodama, slotları) daxildir. perm_root, tx_set_hash), beləliklə, xarici provers transkriptləri yenidən kodlaşdırmadan kanonik FASTPQ sıralarını qəbul edə bilər.

## Sorğular

- İki ləzzət:
  - Tək: `SingularQuery<Output>` tətbiq edin (məsələn, `FindParameters`, `FindExecutorDataModel`).
  - Təkrarlanan: `Query<Item>` tətbiq edin (məsələn, `FindAccounts`, `FindAssets`, `FindDomains` və s.).
- Tipi silinmiş formalar:
  - `QueryBox<T>` qlobal reyestr tərəfindən dəstəklənən, qutulu, silinmiş `Query<Item = T>` Norito serdesidir.
  - `QueryWithFilter<T> { query, predicate, selector }` sorğunu DSL predikatı/selektoru ilə cütləşdirir; `From` vasitəsilə silinmiş təkrarlanan sorğuya çevrilir.
- Qeydiyyat və kodeklər:
  - `query_registry!{ ... }` dinamik deşifrə üçün tip adına görə konstruktorlara konkret sorğu növlərinin xəritələşdirilməsi üzrə qlobal reyestr qurur.
  - `QueryRequest = Singular(SingularQueryBox) | Start(QueryWithParams) | Continue(ForwardCursor)` və `QueryResponse = Singular(..) | Iterable(QueryOutput)`.
  - `QueryOutputBatchBox` homojen vektorlar üzərində cəm növüdür (məsələn, `Vec<Account>`, `Vec<Name>`, `Vec<AssetDefinition>`, `Vec<BlockHeader>`), üstəlik dəfn və paginasiya köməkçiləri üçün.
- DSL: Kompilyasiya zamanı yoxlanılan predikatlar və seçicilər üçün proyeksiya əlamətləri (`HasProjection<PredicateMarker>` / `SelectorMarker`) ilə `query::dsl`-də həyata keçirilir. `fast_dsl` xüsusiyyəti lazım olduqda daha yüngül variantı ortaya qoyur.

## İcraçı və Genişlənmə- `Executor { bytecode: IvmBytecode }`: validator tərəfindən icra edilən kod paketi.
- `ExecutorDataModel { parameters: CustomParameters, instructions: BTreeSet<Ident>, permissions: BTreeSet<Ident>, schema: Json }` icraçı tərəfindən müəyyən edilmiş domeni elan edir:
  - Xüsusi konfiqurasiya parametrləri,
  - Fərdi təlimat identifikatorları,
  - İcazə nişanı identifikatorları,
  - Müştəri alətləri üçün xüsusi növləri təsvir edən JSON sxemi.
- Fərdiləşdirmə nümunələri `data_model/samples/executor_custom_data_model` altında mövcuddur və nümayiş etdirir:
  - `iroha_executor_data_model::permission::Permission` əldə etməklə fərdi icazə nişanı,
  - `CustomParameter`-ə çevrilə bilən bir növ kimi müəyyən edilmiş fərdi parametr,
  - İcra üçün `CustomInstruction`-də seriallaşdırılmış xüsusi təlimatlar.

### Xüsusi Təlimat (icraçı tərəfindən müəyyən edilmiş ISI)

- Növ: `"iroha.custom"` sabit tel id ilə `isi::CustomInstruction { payload: Json }`.
- Məqsəd: özəl/konsorsium şəbəkələrində icraçıya aid göstərişlər və ya ictimai məlumat modelini kəsmədən prototipləşdirmə üçün zərf.
- Defolt icraçı davranışı: `iroha_core`-də quraşdırılmış icraçı `CustomInstruction`-i yerinə yetirmir və qarşılaşdıqda panikaya düşəcək. Fərdi icraçı `InstructionBox`-i `CustomInstruction`-ə endirməli və bütün validatorlarda faydalı yükü determinist şəkildə şərh etməlidir.
- Norito: sxem daxil olmaqla `norito::codec::{Encode, Decode}` vasitəsilə kodlaşdırır/deşifrə edir; `Json` faydalı yükü deterministik şəkildə seriallaşdırılır. Təlimat reyestrinə `CustomInstruction` daxil olduğu müddətcə gediş-gəliş sabitdir (o, standart reyestrin bir hissəsidir).
- IVM: Kotodama IVM bayt kodunu (`.to`) tərtib edir və tətbiq məntiqi üçün tövsiyə olunan yoldur. `CustomInstruction`-dən yalnız hələ Kotodama-də ifadə edilə bilməyən icraçı səviyyəli genişləndirmələr üçün istifadə edin. Həmyaşıdlar arasında determinizm və eyni icraçı binarları təmin edin.
- İctimai şəbəkələr üçün deyil: heterojen icraçıların konsensus çəngəlləri riski olduğu ictimai zəncirlər üçün istifadə etməyin. Platforma xüsusiyyətlərinə ehtiyacınız olduqda yeni daxili ISI yuxarı axını təklif etməyə üstünlük verin.

## Metadata

- `Metadata(BTreeMap<Name, Json>)`: birdən çox quruma əlavə edilmiş açar/dəyər anbarı (`Domain`, `Account`, `AssetDefinition`, `Nft`, tetikler və əməliyyatlar).
- API: `contains`, `iter`, `get`, `insert` və (`transparent_api` ilə) `remove`.

## Xüsusiyyətlər və Determinizm

- Xüsusiyyətlər əlavə API-lərə nəzarət edir (`std`, `json`, `transparent_api`, `ffi_export`, `ffi_import`, `fast_dsl`, I10302X, I10300 `fault_injection`).
- Determinizm: Bütün seriallaşdırma aparat arasında portativ olmaq üçün Norito kodlaşdırmasından istifadə edir. IVM bayt kodu qeyri-şəffaf bayt blobdur; icra deterministik olmayan azalmalar təqdim etməməlidir. Ev sahibi əməliyyatları təsdiqləyir və müəyyən şəkildə IVM-ə daxiletmələri təmin edir.

### Şəffaf API (`transparent_api`)- Məqsəd: Torii, icraçılar və inteqrasiya testləri kimi daxili komponentlər üçün `#[model]` strukturlarına/enumlarına tam, dəyişkən girişi ifşa edir. Onsuz, bu elementlər qəsdən qeyri-şəffafdır, buna görə də xarici SDK-lar yalnız təhlükəsiz konstruktorları və kodlaşdırılmış faydalı yükləri görür.
- Mexanika: `iroha_data_model_derive::model` makrosu hər bir ictimai sahəni `#[cfg(feature = "transparent_api")] pub` ilə yenidən yazır və standart quruluş üçün şəxsi nüsxəsini saxlayır. Xüsusiyyətin aktivləşdirilməsi həmin cfg-ləri çevirir, beləliklə, `Account`, `Domain`, `Asset` və s.-nin dağıdılması onların müəyyənedici modullarından kənarda qanuni olur.
- Səth aşkarlanması: sandıq `TRANSPARENT_API: bool` sabitini ixrac edir (ya `transparent_api.rs` və ya `non_transparent_api.rs`-də yaradılır). Aşağı axın kodu qeyri-şəffaf köməkçilərə qayıtmaq lazım olduqda bu bayrağı və filialı yoxlaya bilər.
- Aktivləşdirilir: `Cargo.toml`-dəki asılılığa `features = ["transparent_api"]` əlavə edin. JSON proyeksiyasına ehtiyacı olan iş sahəsi qutuları (məsələn, `iroha_torii`) bayrağı avtomatik olaraq yönləndirir, lakin üçüncü tərəf istehlakçıları yerləşdirməyə nəzarət etmədikcə və daha geniş API səthini qəbul etmədikcə onu söndürməlidirlər.

## Sürətli Nümunələr

Domen və hesab yaradın, aktivi müəyyənləşdirin və təlimatlarla əməliyyat qurun:

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

DSL ilə hesabları və aktivləri sorğulayın:

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

IVM ağıllı müqavilə bayt kodundan istifadə edin:

```rust
use iroha_data_model::prelude::*;

let bytecode = IvmBytecode::from_compiled(include_bytes!("contract.to").to_vec());
let tx = TransactionBuilder::new("dev-chain".parse().unwrap(), account_id.clone())
    .with_bytecode(bytecode)
    .sign(kp.private_key());
```

## Versiyalaşdırma

- `SignedTransaction`, `SignedBlock` və `SignedQuery` kanonik Norito kodlu strukturlardır. Hər biri `EncodeVersioned` vasitəsilə kodlaşdırıldıqda öz faydalı yükünü cari ABI versiyası (hazırda `1`) ilə prefiks etmək üçün `iroha_version::Version` tətbiq edir.

## Qeydləri nəzərdən keçirin / Potensial Yeniləmələr

- Sorğu DSL: sabit istifadəçi ilə bağlı alt çoxluğu və ümumi filtrlər/selektorlar üçün nümunələri sənədləşdirməyi nəzərdən keçirin.
- Təlimat ailələri: `mint_burn`, `register`, `transfer` tərəfindən ifşa edilmiş daxili ISI variantlarını siyahıya alan ictimai sənədləri genişləndirin.

---
Hər hansı hissənin daha dərinliyə ehtiyacı varsa (məsələn, tam ISI kataloqu, tam sorğu reyestrinin siyahısı və ya blok başlıq sahələri), mənə bildirin və mən bu bölmələri müvafiq olaraq genişləndirəcəyəm.