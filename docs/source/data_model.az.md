---
lang: az
direction: ltr
source: docs/source/data_model.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 683bfb31442f8f4ce7b1bf5038f9dba92fe092545e655f43b51195c21535d3c4
source_last_modified: "2026-03-12T11:24:23.059339+00:00"
translation_last_reviewed: 2026-03-12
translator: machine-google-reviewed
---

# Iroha v2 Data Modeli – Dərin Dalış

Bu sənəd `iroha_data_model` qutusunda həyata keçirilən və iş sahəsində istifadə edilən Iroha v2 data modelini təşkil edən strukturları, identifikatorları, əlamətləri və protokolları izah edir. Bu, nəzərdən keçirə və yeniləmələr təklif edə biləcəyiniz dəqiq istinad üçün nəzərdə tutulub.

## Əhatə və Əsaslar

- Məqsəd: Domen obyektləri (domenlər, hesablar, aktivlər, NFTlər, rollar, icazələr, həmyaşıdlar), vəziyyəti dəyişən təlimatlar (ISI), sorğular, tetikleyiciler, əməliyyatlar, bloklar və parametrlər üçün kanonik növləri təmin etmək.
- Serializasiya: Bütün ictimai növlər Norito kodekləri (`norito::codec::{Encode, Decode}`) və sxemi (`iroha_schema::IntoSchema`) əldə edir. JSON xüsusiyyət bayraqlarının arxasında selektiv şəkildə istifadə olunur (məsələn, HTTP və `Json` yükləri üçün).
- IVM qeyd: Iroha Virtual Maşını (IVM) hədəfləyərkən müəyyən seriyasızlaşdırma vaxtı yoxlamaları deaktiv edilir, çünki host müqavilələri işə salmazdan əvvəl yoxlama aparır (Norito-də sandıq sənədlərinə baxın).
- FFI qapıları: Bəzi növlər FFI-ya ehtiyac olmadığı zaman əlavə yükdən qaçmaq üçün `ffi_export`/`ffi_import`-in arxasında `iroha_ffi` vasitəsilə FFI üçün şərti olaraq qeyd olunur.

## Əsas xüsusiyyətlər və köməkçilər- `Identifiable`: Müəssisələrdə sabit `Id` və `fn id(&self) -> &Self::Id` var. Xəritə/set dostluğu üçün `IdEqOrdHash` ilə əldə edilməlidir.
- `Registrable`/`Registered`: Bir çox obyekt (məsələn, `Domain`, `AssetDefinition`, `Role`) qurucu nümunəsindən istifadə edir. `Registered` iş vaxtı növünü qeydiyyat əməliyyatları üçün uyğun olan yüngül qurucu tipinə (`With`) bağlayır.
- `HasMetadata`: Açar/dəyər `Metadata` xəritəsinə vahid giriş.
- `IntoKeyValue`: Təkrarlanmanı azaltmaq üçün `Key` (ID) və `Value` (məlumat) ayrı-ayrılıqda saxlamaq üçün yaddaş bölməsi köməkçisi.
- `Owned<T>`/`Ref<'world, K, V>`: Lazımsız nüsxələrin qarşısını almaq üçün anbarlarda və sorğu filtrlərində istifadə olunan yüngül sarğılar.

## Adlar və İdentifikatorlar- `Name`: Etibarlı mətn identifikatoru. Boşluq və qorunan simvollara icazə vermir `@`, `#`, `$` (kompozit ID-lərdə istifadə olunur). Doğrulama ilə `FromStr` vasitəsilə tikilə bilər. Adlar təhlil zamanı Unicode NFC ilə normallaşdırılır (kanonik ekvivalent yazılar eyni hesab olunur və yığılmış şəkildə saxlanılır). `genesis` xüsusi adı qorunur (hərf hərfinə həssaslıqla yoxlanılır).
- `IdBox`: Hər hansı dəstəklənən ID (`DomainId`, `AccountId`, `AssetDefinitionId`, `AssetId`, `AssetId`, Kotodama, Kotodama, `DomainId`, `DomainId`) üçün cəmi tipli zərf `TriggerId`, `RoleId`, `Permission`, `CustomParameterId`). Ümumi axınlar və tək bir növ kimi Norito kodlaşdırması üçün faydalıdır.
- `ChainId`: Əməliyyatlarda təkrar qorunma üçün istifadə edilən qeyri-şəffaf zəncir identifikatoru.İdentifikatorların sətir formaları (`Display`/`FromStr` ilə gediş-gəliş):
- `DomainId`: `name` (məsələn, `wonderland`).
- `AccountId`: yalnız i105 olaraq `AccountAddress` vasitəsilə kodlaşdırılmış kanonik domensiz hesab identifikatoru. Parser girişləri kanonik i105 olmalıdır; domen şəkilçiləri (`@domain`), kanonik i105 literalları, ləqəb literalları, kanonik hex təhlil girişi, köhnə `norito:` faydalı yükləri və `uaid:`/`opaque:` hesaba buraxılır.
- `AssetDefinitionId`: kanonik `unprefixed Base58 address with versioning and checksum` (UUID-v4 bayt).
- `AssetId`: kanonik kodlaşdırılmış literal `<canonical-base58-asset-definition-id>` (ilk buraxılışda köhnə mətn formaları dəstəklənmir).
- `NftId`: `nft$domain` (məsələn, `rose$garden`).
- `PeerId`: `public_key` (peer bərabərliyi açıq açarladır).

## Müəssisələr

### Domen
- `DomainId { name: Name }` – unikal ad.
- `Domain { id, logo: Option<SorafsUri>, metadata: Metadata, owned_by: AccountId }`.
- Qurucu: `NewDomain` ilə `with_logo`, `with_metadata`, sonra `Registrable::build(authority)` dəstləri `owned_by`.### Hesab
- `AccountId` nəzarətçi tərəfindən əsaslanan və kanonik i105 kimi kodlaşdırılmış kanonik domensiz hesab identifikasiyasıdır.
- `ScopedAccountId { account: AccountId, domain: DomainId }` yalnız əhatəli görünüş tələb olunduqda açıq domen kontekstini daşıyır.
- `Account { id, metadata, label?, uaid? }` — `label` yenidən açar qeydlər tərəfindən istifadə edilən isteğe bağlı sabit ləqəbdir, `uaid` isteğe bağlı Nexus geniş [Universal Hesab ID](Kotodama) daşıyır.
- Qurucu: `NewAccount` vasitəsilə `Account::new(id)`; qeydiyyat açıq `ScopedAccountId` domeni tələb edir və defoltlardan birini çıxarmır.

### Aktiv anlayışları və aktivlər
- `AssetDefinitionId { aid_bytes: [u8; 16] }` mətn olaraq `unprefixed Base58 address` kimi ifşa olunur.
- `AssetDefinition { id, name, description?, alias?, spec: NumericSpec, mintable: Mintable, logo: Option<SorafsUri>, metadata, owned_by: AccountId, total_quantity: Numeric }`.

  - Torii asset-definition responses may include `alias_binding { alias, status, lease_expiry_ms, grace_until_ms, bound_at_ms }`; alias selectors resolve against latest committed block time and stop resolving after grace, while direct reads may still show `expired_pending_cleanup` until sweep.
  - `name` insana baxan ekran mətni tələb olunur və tərkibində `#`/`@` olmamalıdır.
  - `alias` isteğe bağlıdır və aşağıdakılardan biri olmalıdır:
    - `<name>#<domain>.<dataspace>`
    - `<name>#<dataspace>`
    sol seqment tam uyğun `AssetDefinition.name` ilə.
  - `Mintable`: `Infinitely` | `Once` | `Limited(u32)` | `Not`.
  - İnşaatçılar: `AssetDefinition::new(id, spec)` və ya rahatlıq `numeric(id)`; `name` tələb olunur və `.with_name(...)` vasitəsilə quraşdırılmalıdır.
- `AssetId { account: AccountId, definition: AssetDefinitionId, scope: AssetBalanceScope }`.
- Saxlama üçün əlverişli `AssetEntry`/`AssetValue` ilə `Asset { id, value: Numeric }`.
- `AssetBalanceScope`: məhdudiyyətsiz balanslar üçün `Global` və məlumat məkanı ilə məhdudlaşdırılmış qalıqlar üçün `Dataspace(DataSpaceId)`.
- `AssetTotalQuantityMap = BTreeMap<AssetDefinitionId, Numeric>` xülasə API-ləri üçün açıqdır.### NFTs
- `NftId { domain: DomainId, name: Name }`.
- `Nft { id, content: Metadata, owned_by: AccountId }` (məzmun ixtiyari açar/dəyər metadatasıdır).
- Qurucu: `Nft::new(id, content)` vasitəsilə `NewNft`.

### Rollar və İcazələr
- `RoleId { name: Name }`.
- `Role { id, permissions: BTreeSet<Permission> }` inşaatçı ilə `NewRole { inner: Role, grant_to: AccountId }`.
- `Permission { name: Ident, payload: Json }` – `name` və faydalı yük sxemi aktiv `ExecutorDataModel` ilə uyğunlaşdırılmalıdır (aşağıya bax).

### Həmyaşıdları
- `PeerId { public_key: PublicKey }`.
- `Peer { address: SocketAddr, id: PeerId }` və parsable `public_key@address` sətir forması.

### Kriptoqrafik primitivlər (Xüsusiyyət `sm`)
- `Sm2PublicKey` və `Sm2Signature`: SM2 üçün SEC1 uyğun nöqtələr və sabit enli `r∥s` imzaları. Konstruktorlar əyri üzvlüyü və fərqləndirici identifikatorları təsdiqləyir; Norito kodlaması `iroha_crypto` tərəfindən istifadə edilən kanonik təsviri əks etdirir.
- `Sm3Hash`: GM/T 0004 həzmini təmsil edən `[u8; 32]` yeni tip, manifestlər, telemetriya və sistem çağırışı cavablarında istifadə olunur.
- `Sm4Key`: 128 bitlik simmetrik açar sarğısı host sistemləri və verilənlər modeli qurğuları arasında paylaşılır.
Bu növlər mövcud Ed25519/BLS/ML-DSA primitivləri ilə yanaşı oturur və iş sahəsi `--features sm` ilə qurulduqdan sonra ictimai sxemin bir hissəsinə çevrilir.### Tətiklər və Hadisələr
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
- Fərdi parametrlər: icraçı tərəfindən müəyyən edilib, `Json` kimi daşınır, `CustomParameterId` (a `Name`) ilə müəyyən edilir.

## ISI (Iroha Xüsusi Təlimatlar)- Əsas xüsusiyyət: `Instruction`, `dyn_encode`, `as_any` və hər növ üçün sabit identifikator `id()` (defolt olaraq konkret tip adına uyğundur). Bütün təlimatlar `Send + Sync + 'static`-dir.
- `InstructionBox`: Klon/eq/ord ilə `Box<dyn Instruction>` sarğısı ID növü + kodlanmış baytlar vasitəsilə həyata keçirilir.
- Quraşdırılmış təlimat ailələri aşağıdakılar əsasında təşkil edilir:
  - `mint_burn`, `transfer`, `register` və `transparent` köməkçilər dəsti.
  - Meta axınları üçün nömrələri yazın: `InstructionType`, `SetKeyValueBox` (domain/account/asset_def/nft/trigger) kimi qutulu məbləğlər.
- Səhvlər: `isi::error` altında zəngin səhv modeli (qiymətləndirmə növü səhvləri, tapma səhvləri, hesablama qabiliyyəti, riyaziyyat, etibarsız parametrlər, təkrar, dəyişməzlik).
- Təlimat reyestri: `instruction_registry!{ ... }` makrosu növ adı ilə əsaslanan icra zamanı deşifrə reyestrini qurur. Dinamik (de)seriyaya nail olmaq üçün `InstructionBox` klonu və Norito serde tərəfindən istifadə olunur. `set_instruction_registry(...)` vasitəsilə heç bir reyestr açıq şəkildə qurulmayıbsa, ikili faylları möhkəm saxlamaq üçün bütün əsas ISI ilə daxili standart reyestr ilk istifadədə tənbəlliklə quraşdırılır.

## Əməliyyatlar- `Executable`: ya `Instructions(ConstVec<InstructionBox>)`, ya da `Ivm(IvmBytecode)`. `IvmBytecode` base64 kimi seriallaşdırılır (`Vec<u8>` üzərində şəffaf yeni tip).
- `TransactionBuilder`: `chain`, `authority`, `creation_time_ms`, isteğe bağlı `time_to_live_ms` və `nonce`, I010X, I018 və `chain` ilə əməliyyat yükü qurur `Executable`.
  - Köməkçilər: `with_instructions`, `with_bytecode`, `with_executable`, `with_metadata`, `set_nonce`, `set_ttl`, `set_ttl`, Kotodama.
- `SignedTransaction` (`iroha_version` ilə versiya): `TransactionSignature` və faydalı yük daşıyır; hashing və imza yoxlamasını təmin edir.
- Giriş nöqtələri və nəticələr:
  - `TransactionEntrypoint`: `External(SignedTransaction)` | `Time(TimeTriggerEntrypoint)`.
  - Hashing köməkçiləri ilə `TransactionResult` = `Result<DataTriggerSequence, TransactionRejectionReason>`.
  - `ExecutionStep(ConstVec<InstructionBox>)`: əməliyyatda təlimatların tək sifarişli toplusu.

## Bloklar- `SignedBlock` (versiya) əhatə edir:
  - `signatures: BTreeSet<BlockSignature>` (validatorlardan),
  - `payload: BlockPayload { header: BlockHeader, transactions: Vec<SignedTransaction> }`,
  - `result: BlockResult` (ikinci icra vəziyyəti) ehtiva edən `time_triggers`, giriş/nəticə Merkle ağacları, `transaction_results` və `fastpq_transcripts: BTreeMap<Hash, Vec<TransferTranscript>>`.
- Kommunal xidmətlər: `presigned`, `set_transaction_results(...)`, `set_transaction_results_with_transcripts(...)`, `header()`, `signatures()`, `hash()`, `hash()`, Kotodama.
- Merkle kökləri: əməliyyat giriş nöqtələri və nəticələr Merkle ağacları vasitəsilə həyata keçirilir; nəticədə Merkle kökü blok başlığına yerləşdirilir.
- Blok daxiletmə sübutları (`BlockProofs`) həm giriş/nəticə Merkle sübutlarını, həm də `fastpq_transcripts` xəritəsini ifşa edir ki, zəncirdən kənar provayderlər tranzaksiya hash ilə əlaqəli transfer deltalarını əldə edə bilsinlər.
- `ExecWitness` mesajları (Torii vasitəsilə yayımlanır və konsensus dedi-qoduları ilə dəstəklənir) indi həm `fastpq_transcripts`, həm də daxil edilmiş kökləri olan `fastpq_batches: Vec<FastpqTransitionBatch>` (Kotodama, slotları ilə proverə hazır `fastpq_batches: Vec<FastpqTransitionBatch>`) daxildir. perm_root, tx_set_hash), beləliklə, xarici provers transkriptləri yenidən kodlaşdırmadan kanonik FASTPQ sıralarını qəbul edə bilər.

## Sorğular- İki ləzzət:
  - Tək: `SingularQuery<Output>` tətbiq edin (məsələn, `FindParameters`, `FindExecutorDataModel`).
  - Təkrarlanan: `Query<Item>` tətbiq edin (məsələn, `FindAccounts`, `FindAssets`, `FindDomains` və s.).
- Tipi silinmiş formalar:
  - `QueryBox<T>` qlobal reyestr tərəfindən dəstəklənən, qutulu, silinmiş `Query<Item = T>` Norito serdesidir.
  - `QueryWithFilter<T> { query, predicate, selector }` sorğunu DSL predikatı/selektoru ilə cütləşdirir; `From` vasitəsilə silinmiş təkrarlanan sorğuya çevrilir.
- Qeydiyyat və kodeklər:
  - `query_registry!{ ... }` dinamik deşifrə üçün tip adına görə konstruktorlara konkret sorğu növlərinin xəritələşdirilməsi üzrə qlobal reyestr qurur.
  - `QueryRequest = Singular(SingularQueryBox) | Start(QueryWithParams) | Continue(ForwardCursor)` və `QueryResponse = Singular(..) | Iterable(QueryOutput)`.
  - `QueryOutputBatchBox` homojen vektorlar üzərində cəm növüdür (məsələn, `Vec<Account>`, `Vec<Name>`, `Vec<AssetDefinition>`, `Vec<BlockHeader>`), üstəlik dəfn və paginasiya köməkçiləri üçün.
- DSL: Kompilyasiya zamanı yoxlanılan predikatlar və seçicilər üçün proyeksiya əlamətləri (`HasProjection<PredicateMarker>` / `SelectorMarker`) ilə `query::dsl`-də həyata keçirilir. `fast_dsl` funksiyası lazım olduqda daha yüngül variantı ortaya qoyur.

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

### Xüsusi Təlimat (icraçı tərəfindən müəyyən edilmiş ISI)- Növ: `"iroha.custom"` sabit tel id ilə `isi::CustomInstruction { payload: Json }`.
- Məqsəd: özəl/konsorsium şəbəkələrində icraçıya aid göstərişlər və ya ictimai məlumat modelini kəsmədən prototipləşdirmə üçün zərf.
- Defolt icraçı davranışı: `iroha_core`-də quraşdırılmış icraçı `CustomInstruction`-i yerinə yetirmir və qarşılaşdıqda panikaya düşəcək. Fərdi icraçı `InstructionBox`-i `CustomInstruction`-ə endirməli və bütün validatorlarda faydalı yükü determinist şəkildə şərh etməlidir.
- Norito: sxem daxil olmaqla `norito::codec::{Encode, Decode}` vasitəsilə kodlaşdırır/deşifrə edir; `Json` faydalı yükü deterministik şəkildə seriallaşdırılır. Təlimat reyestrinə `CustomInstruction` daxil olduğu müddətcə gediş-gəliş sabitdir (o, standart reyestrin bir hissəsidir).
- IVM: Kotodama IVM bayt kodunu (`.to`) tərtib edir və tətbiq məntiqi üçün tövsiyə olunan yoldur. Hələ Kotodama-də ifadə edilə bilməyən icraçı səviyyəli genişləndirmələr üçün yalnız `CustomInstruction` istifadə edin. Həmyaşıdlar arasında determinizm və eyni icraçı binarları təmin edin.
- İctimai şəbəkələr üçün deyil: heterojen icraçıların konsensus çəngəlləri riski olduğu ictimai zəncirlər üçün istifadə etməyin. Platforma xüsusiyyətlərinə ehtiyacınız olduqda yeni daxili ISI yuxarı axını təklif etməyə üstünlük verin.

## Metadata- `Metadata(BTreeMap<Name, Json>)`: bir neçə obyektə əlavə edilmiş açar/dəyər anbarı (`Domain`, `Account`, `AssetDefinition`, `Nft`, tətiklər və əməliyyatlar).
- API: `contains`, `iter`, `get`, `insert` və (`transparent_api` ilə) `remove`.

## Xüsusiyyətlər və Determinizm

- Xüsusiyyətlər əlavə API-lərə nəzarət edir (`std`, `json`, `transparent_api`, `ffi_export`, `ffi_import`, `fast_dsl`, I10807X, I10800 `fault_injection`).
- Determinizm: Bütün seriallaşdırma aparat arasında portativ olmaq üçün Norito kodlaşdırmasından istifadə edir. IVM bayt kodu qeyri-şəffaf bayt blobdur; icra deterministik olmayan azalmalar təqdim etməməlidir. Ev sahibi əməliyyatları təsdiqləyir və müəyyən bir şəkildə IVM-ə daxilolmalar təqdim edir.

### Şəffaf API (`transparent_api`)- Məqsəd: Torii, icraçılar və inteqrasiya testləri kimi daxili komponentlər üçün `#[model]` strukturlarına/enumlarına tam, dəyişən girişi ifşa edir. Onsuz, bu elementlər qəsdən qeyri-şəffafdır, buna görə də xarici SDK-lar yalnız təhlükəsiz konstruktorları və kodlaşdırılmış faydalı yükləri görür.
- Mexanika: `iroha_data_model_derive::model` makrosu hər bir ictimai sahəni `#[cfg(feature = "transparent_api")] pub` ilə yenidən yazır və standart quruluş üçün şəxsi nüsxəsini saxlayır. Xüsusiyyətin aktivləşdirilməsi həmin cfg-ləri çevirir, beləliklə, `Account`, `Domain`, `Asset` və s.-nin dağıdılması onların müəyyənedici modullarından kənarda qanuni olur.
- Səth aşkarlanması: sandıq `TRANSPARENT_API: bool` sabitini ixrac edir (ya `transparent_api.rs` və ya `non_transparent_api.rs`-da yaradılır). Aşağı axın kodu qeyri-şəffaf köməkçilərə qayıtmaq lazım olduqda bu bayrağı və filialı yoxlaya bilər.
- Aktivləşdirilir: `Cargo.toml`-dəki asılılığa `features = ["transparent_api"]` əlavə edin. JSON proyeksiyasına ehtiyacı olan iş sahəsi qutuları (məsələn, `iroha_torii`) bayrağı avtomatik irəliləyir, lakin üçüncü tərəf istehlakçıları yerləşdirməyə nəzarət etmədikcə və daha geniş API səthini qəbul etmədikcə onu söndürməlidirlər.

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

asset-definition id / ləqəblə sürətli arayış (CLI + Torii):

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
  --account soraゴヂ... \
  --quantity 500

# Resolve alias to canonical Base58 id via Torii
curl -sS http://127.0.0.1:8080/v1/assets/aliases/resolve \
  -H 'content-type: application/json' \
  -d '{"alias":"pkr#ubl.sbp"}'
```Miqrasiya qeydi:
- Köhnə `name#domain` aktiv tərifi identifikatorları v1-də qəbul edilmir.
- Nanə/yandırma/köçürmə üçün aktiv identifikatorları kanonik `<canonical-base58-asset-definition-id>` olaraq qalır; onları qurun:
  - `iroha tools encode asset-id --definition <base58-asset-definition-id> --account <i105>`
  - və ya `--alias <name>#<domain>.<dataspace>` / `--alias <name>#<dataspace>` + `--account`.

## Versiyalaşdırma

- `SignedTransaction`, `SignedBlock` və `SignedQuery` kanonik Norito kodlu strukturlardır. Hər biri `iroha_version::Version`-i `EncodeVersioned` vasitəsilə kodlaşdırıldıqda cari ABI versiyası ilə (hazırda `1`) prefiks etmək üçün `iroha_version::Version` tətbiq edir.

## Qeydləri nəzərdən keçirin / Potensial Yeniləmələr

- Sorğu DSL: sabit istifadəçi ilə bağlı alt çoxluğu və ümumi filtrlər/selektorlar üçün nümunələri sənədləşdirməyi nəzərdən keçirin.
- Təlimat ailələri: `mint_burn`, `register`, `transfer` tərəfindən ifşa edilmiş daxili ISI variantlarını siyahıya alan ictimai sənədləri genişləndirin.

---
Hər hansı hissənin daha dərinliyə ehtiyacı varsa (məsələn, tam ISI kataloqu, tam sorğu reyestrinin siyahısı və ya blok başlıq sahələri), mənə bildirin və mən bu bölmələri müvafiq olaraq genişləndirəcəyəm.