---
lang: hy
direction: ltr
source: docs/source/data_model.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8337416254dfc062c40d691f6b35f7ee5818a1071279142bff75a74b75c0a802
source_last_modified: "2026-03-27T19:05:03.382221+00:00"
translation_last_reviewed: 2026-03-28
translator: machine-google-reviewed
---

# Iroha v2 Տվյալների մոդել – Խորը սուզում

Այս փաստաթուղթը բացատրում է կառուցվածքները, նույնացուցիչները, հատկանիշները և արձանագրությունները, որոնք կազմում են Iroha v2 տվյալների մոդելը, ինչպես ներդրվել է `iroha_data_model` տուփում և օգտագործվում է աշխատանքային տարածքում: Այն նախատեսված է որպես ճշգրիտ հղում, որը կարող եք վերանայել և առաջարկել թարմացումներ:

## Շրջանակ և հիմքեր

- Նպատակը. Տրամադրել կանոնական տիպեր տիրույթի օբյեկտների համար (տիրույթներ, հաշիվներ, ակտիվներ, NFT-ներ, դերեր, թույլտվություններ, գործընկերներ), վիճակի փոփոխման հրահանգներ (ISI), հարցումներ, գործարկիչներ, գործարքներ, բլոկներ և պարամետրեր:
- Սերիալացում. Բոլոր հանրային տեսակները ստանում են Norito կոդեկներ (`norito::codec::{Encode, Decode}`) և սխեման (`iroha_schema::IntoSchema`): JSON-ն օգտագործվում է ընտրովի (օրինակ՝ HTTP և `Json` օգտակար բեռների համար) գործառույթների դրոշների հետևում:
- IVM նշում. որոշ ապասերիալացման ժամանակի վավերացումներն անջատված են, երբ թիրախավորում են Iroha վիրտուալ մեքենան (IVM), քանի որ հոսթն իրականացնում է վավերացում նախքան պայմանագրերը կանչելը (տե՛ս արկղային փաստաթղթերը I100390X-ում):
- FFI դարպասներ. որոշ տեսակներ պայմանականորեն ծանոթագրված են FFI-ի համար `iroha_ffi`-ի միջոցով `ffi_export`/`ffi_import`-ի հետևում, որպեսզի խուսափեն գերավճարներից, երբ FFI-ի կարիք չկա:

## Հիմնական հատկություններ և օգնականներ- `Identifiable`. Կազմակերպություններն ունեն կայուն `Id` և `fn id(&self) -> &Self::Id`: Պետք է ստացվի `IdEqOrdHash`-ի հետ՝ քարտեզի/կոմպլեկտների հարմարավետության համար:
- `Registrable`/`Registered`. Շատ կազմակերպություններ (օրինակ՝ `Domain`, `AssetDefinition`, `Role`) օգտագործում են շինարարական օրինաչափություն: `Registered`-ը գործարկման ժամանակի տեսակը կապում է թեթև շինարարական տեսակի հետ (`With`), որը հարմար է գրանցման գործարքների համար:
- `HasMetadata`. միասնական մուտք դեպի `Metadata` բանալի/արժեք քարտեզ:
- `IntoKeyValue`. Պահպանման բաժանման օգնական՝ `Key` (ID) և `Value` (տվյալներ) առանձին պահելու համար՝ կրկնօրինակումը նվազեցնելու համար:
- `Owned<T>`/`Ref<'world, K, V>`. Թեթև փաթաթիչներ, որոնք օգտագործվում են պահեստներում և հարցումների զտիչներում՝ ավելորդ պատճեններից խուսափելու համար:

## Անուններ և նույնացուցիչներ- `Name`. Վավեր տեքստային նույնացուցիչ: Թույլ չի տալիս բացատները և վերապահված նիշերը `@`, `#`, `$` (օգտագործվում են կոմպոզիտային ID-ներում): Կառուցելի է `FromStr`-ի միջոցով՝ վավերացումով: Անունները վերլուծվում են Unicode NFC-ով (կանոնականորեն համարժեք ուղղագրությունները համարվում են նույնական և պահվում են կազմված): Հատուկ անունը `genesis` վերապահված է (ստուգված է առանց տառատեսակների):
- `IdBox`. Գումարի տիպի ծրար ցանկացած աջակցվող ID-ի համար (`DomainId`, `AccountId`, `AssetDefinitionId`, `AssetId`, `AssetId`, Kotodama `TriggerId`, `RoleId`, `Permission`, `CustomParameterId`): Օգտակար է ընդհանուր հոսքերի և Norito կոդավորման համար որպես մեկ տեսակ:
- `ChainId`. Անթափանց շղթայի նույնացուցիչ, որն օգտագործվում է գործարքներում կրկնակի պաշտպանության համար:ID-ների լարային ձևեր (շրջագայելի `Display`/`FromStr`-ով).
- `DomainId`՝ `name` (օրինակ՝ `wonderland`):
- `AccountId`. առանց տիրույթի կանոնական հաշվի նույնացուցիչ՝ կոդավորված `AccountAddress`-ի միջոցով միայն որպես I105: Խիստ վերլուծիչի մուտքերը պետք է լինեն կանոնական I105; տիրույթի վերջածանցները (`@domain`), account-alias literals, կանոնական վեցանկյուն վերլուծիչ մուտքագրում, հին `norito:` օգտակար բեռներ և `uaid:`/`opaque:` հաշիվների վերլուծիչ ձևերը մերժվում են: Շղթայական հաշվի կեղծանունները օգտագործում են `name@domain.dataspace` կամ `name@dataspace` և որոշում են `AccountId` կանոնական արժեքները:
- `AssetDefinitionId`. կանոնական առանց նախածանցով Base58 հասցե կանոնական ակտիվների սահմանման բայթերի վրա: Սա հանրային ակտիվի ID-ն է: Շղթայական ակտիվների կեղծանունները օգտագործում են `name#domain.dataspace` կամ `name#dataspace` և լուծվում են միայն այս կանոնական Base58 ակտիվի ID-ով:
- `AssetId`՝ հանրային ակտիվի նույնացուցիչ՝ կանոնական մերկ Base58 ձևով: Ակտիվների անունները, ինչպիսիք են `name#dataspace` կամ `name#domain.dataspace`, լուծվում են `AssetId`-ով: Ներքին մատյանների պահումները կարող են լրացուցիչ ցուցադրել պառակտված `asset + account + optional dataspace` դաշտերը, որտեղ անհրաժեշտ է, բայց այդ կոմպոզիտային ձևը հանրային `AssetId` չէ:
- `NftId`՝ `nft$domain` (օրինակ՝ `rose$garden`):
- `PeerId`՝ `public_key` (հասակակիցների հավասարությունը հանրային բանալին է):

## Սուբյեկտներ

### տիրույթ
- `DomainId { name: Name }` – եզակի անուն:
- `Domain { id, logo: Option<SorafsUri>, metadata: Metadata, owned_by: AccountId }`.
- Կառուցող.

### Հաշիվ
- `AccountId`-ը կանոնական առանց տիրույթի հաշվի նույնականացումն է, որը բանալի է վերահսկիչի կողմից և կոդավորված է որպես կանոնական I105:
- `ScopedAccountId { account: AccountId, domain: DomainId }`-ը կրում է տիրույթի բացահայտ համատեքստ միայն այն դեպքում, երբ պահանջվում է շրջանակային տեսք:
- `Account { id, metadata, label?, uaid?, linked_domains? }` — `label` կամընտիր կայուն կեղծանունն է, որն օգտագործվում է վերաբանալու գրառումների կողմից, `uaid`-ը կրում է կամընտիր Nexus լայնածավալ [Համընդհանուր հաշվի ID](`uaid`) (Norito) (Kotodama), իսկ Norito ածանցյալ ինդեքսային վիճակ, այլ ոչ թե կանոնական ինքնության մաս:
- Շինարարներ.
  - `NewAccount`-ը `Account::new(scoped_id)`-ի միջոցով իրականացնում է տիրույթի հետ կապված հստակ գրանցում և հետևաբար պահանջում է `ScopedAccountId`:
  - `NewAccount`-ը `Account::new_domainless(id)`-ի միջոցով գրանցում է միայն համընդհանուր հաշվի առարկան՝ առանց կապված տիրույթի:
- Alias մոդելը:
  - Կանոնական հաշվի ինքնությունը երբեք չի ներառում տիրույթ կամ տվյալների տարածության հատված:
  - Հաշվի կեղծանունները առանձին SNS/հաշվի պիտակի կապեր են՝ շերտավորված `AccountId`-ի վերևում:
  - Դոմեյնին համապատասխանող կեղծանունները, ինչպիսիք են `merchant@hbl.sbp`-ը, կրում են և՛ տիրույթ, և՛ տվյալների տարածություն՝ կապի անունի մեջ:
  - Տվյալների տարածություն-արմատ կեղծանունները, ինչպիսիք են `merchant@sbp`-ը, կրում են միայն տվյալների տարածությունը և, հետևաբար, բնականաբար զուգակցվում են `Account::new_domainless(...)`-ի հետ:
  - Թեստերը և հարմարանքները պետք է նախ ներդնեն համընդհանուր `AccountId`-ը, այնուհետև առանձին-առանձին ավելացնեն տիրույթի հղումներ, այլ անունների վարձակալություն և այլանունների թույլտվությունները՝ բուն հաշվի ինքնության մեջ տիրույթի ենթադրությունները կոդավորելու փոխարեն:

### Ակտիվների սահմանումներ և ակտիվներ
- `AssetDefinitionId { aid_bytes: [u8; 16] }`-ը տեքստային կերպով ցուցադրվում է որպես Base58 առանց նախածանցի հասցե՝ տարբերակման և ստուգման գումարի հետ:
- `AssetDefinition { id, name, description?, alias?, spec: NumericSpec, mintable: Mintable, logo: Option<SorafsUri>, metadata, owned_by: AccountId, total_quantity: Numeric }`.
  - `name`-ը մարդուն ուղղված ցուցադրման տեքստ է պահանջվում և չպետք է պարունակի `#`/`@`:
  - `alias`-ը կամընտիր է և պետք է լինի հետևյալներից մեկը.
    - `<name>#<domain>.<dataspace>`
    - `<name>#<dataspace>`
    ձախ հատվածով ճիշտ համընկնում է `AssetDefinition.name`-ով:
  - Փոխանունների վարձակալության վիճակը հեղինակավոր կերպով պահպանվում է անուն-ազգանունների պարտադիր գրանցման մեջ. ներկառուցված `alias` դաշտը ստացվում է, երբ սահմանումները հետ են ընթերցվում հիմնական/Torii API-ների միջոցով:
  - Torii ակտիվների սահմանման պատասխանները կարող են ներառել `alias_binding { alias, status, lease_expiry_ms, grace_until_ms, bound_at_ms }`, որտեղ `status`-ը `permanent`, `leased_active`, `alias_binding { alias, status, lease_expiry_ms, grace_until_ms, bound_at_ms }`, `alias_binding { alias, status, lease_expiry_ms, grace_until_ms, bound_at_ms }`-ից մեկն է:
  - Alias-ի լուծումը օգտագործում է վերջին պարտավորված բլոկի ժամանակացույցը, այլ ոչ թե հանգույցի պատի ժամացույցը: Երբ `grace_until_ms`-ն անցնի, կեղծանունների ընտրիչները դադարում են անմիջապես լուծել, նույնիսկ եթե մաքրման մաքրումը դեռ չի վերացրել հնացած կապը. ուղղակի սահմանման ընթերցումները կարող են դեռևս հայտնել երկարատև կապը որպես `expired_pending_cleanup`:
  - `Mintable`՝ `Infinitely` | `Once` | `Limited(u32)` | `Not`.
  - Շինարարներ՝ `AssetDefinition::new(id, spec)` կամ հարմարավետ `numeric(id)`; `name`-ը պահանջվում է և պետք է սահմանվի `.with_name(...)`-ի միջոցով:
- `AssetId { account: AccountId, definition: AssetDefinitionId, scope: AssetBalanceScope }`.
- `Asset { id, value: Numeric }` պահեստավորման համար հարմար `AssetEntry`/`AssetValue`:

- `AssetBalanceScope`. `Global` անսահմանափակ մնացորդների համար և `Dataspace(DataSpaceId)` տվյալների տարածության սահմանափակ մնացորդների համար:
- `AssetTotalQuantityMap = BTreeMap<AssetDefinitionId, Numeric>` ներկայացված է ամփոփ API-ների համար:

#

## NFTs
- `NftId { domain: DomainId, name: Name }`.
- `Nft { id, content: Metadata, owned_by: AccountId }` (բովանդակությունը կամայական բանալի/արժեքի մետատվյալ է):
- Կառուցող՝ `NewNft` `Nft::new(id, content)`-ի միջոցով:

#

## Դերեր և թույլտվություններ
- `RoleId { name: Name }`.
- `Role { id, permissions: BTreeSet<Permission> }` շինարարով `NewRole { inner: Role, grant_to: AccountId }`:
- `Permission { name: Ident, payload: Json }` – `name` և օգտակար բեռնվածության սխեման պետք է համապատասխանի ակտիվ `ExecutorDataModel`-ին (տես ստորև):

#

## Հասակակիցներ
- `PeerId { public_key: PublicKey }`.
- `Peer { address: SocketAddr, id: PeerId }` և վերլուծելի `public_key@address` լարային ձև:

#

## Կրիպտոգրաֆիկ պրիմիտիվներ (հատկանիշ `sm`)
- `Sm2PublicKey` և `Sm2Signature`. SEC1-ին համապատասխանող կետեր և ֆիքսված լայնությամբ `r∥s` ստորագրություններ SM2-ի համար: Կառուցիչները վավերացնում են կորի անդամակցությունը և տարբերակիչ ID-ները. Norito կոդավորումը արտացոլում է `iroha_crypto`-ի կողմից օգտագործված կանոնական ներկայացումը:
- `Sm3Hash`. `[u8; 32]` նոր տիպ, որը ներկայացնում է GM/T 0004 մարսողությունը, որն օգտագործվում է մանիֆեստների, հեռաչափության և համակարգային պատասխաններում:
- `Sm4Key`. 128-բիթանոց սիմետրիկ բանալիների փաթաթան, որը համօգտագործվում է հյուրընկալող համակարգերի և տվյալների մոդելի հարմարանքների միջև:
Այս տեսակները նստում են գոյություն ունեցող Ed25519/BLS/ML-DSA պրիմիտիվների կողքին և դառնում հանրային սխեմայի մաս, երբ աշխատանքային տարածքը կառուցվի `--features sm`-ով:

### Գործարկիչներ և իրադարձություններ
- `TriggerId { name: Name }` և `Trigger { id, action: action::Action }`:
- `action::Action { executable: Executable, repeats: Repeats, authority: AccountId, filter: EventFilterBox, metadata }`.
  - `Repeats`՝ `Indefinitely` կամ `Exactly(u32)`; ներառյալ պատվիրման և սպառման կոմունալ ծառայությունները:
  - Անվտանգություն. `TriggerCompleted`-ը չի կարող օգտագործվել որպես գործողությունների զտիչ (հաստատված է (ապասերիալացման ժամանակ):
- `EventBox`. խողովակաշարի, խողովակաշարի խմբաքանակի, տվյալների, ժամանակի, կատարման գործարկման և գործարկիչով ավարտված իրադարձությունների գումարի տեսակը. `EventFilterBox` հայելիներ, որոնք նախատեսված են բաժանորդագրությունների և գործարկման զտիչների համար:

## Պարամետրեր և կազմաձևում

- Համակարգի պարամետրերի ընտանիքներ (բոլոր `Default`ed, կրող ստացողներ և փոխարկվում են առանձին թվերի).
- `SumeragiParameters { block_time_ms, commit_time_ms, min_finality_ms, pacing_factor_bps, max_clock_drift_ms, collectors_k, collectors_redundant_send_r }`.
  - `BlockParameters { max_transactions: NonZeroU64 }`.
  - `TransactionParameters { max_signatures, max_instructions, ivm_bytecode_size, max_tx_bytes, max_decompressed_bytes }`.
  - `SmartContractParameters { fuel, memory, execution_depth }`.
- `Parameters`-ը խմբավորում է բոլոր ընտանիքները և `custom: BTreeMap<CustomParameterId, CustomParameter>`:
- Մեկ պարամետրով թվեր՝ `SumeragiParameter`, `BlockParameter`, `TransactionParameter`, `SmartContractParameter`՝ տարբերվող թարմացումների և կրկնությունների համար:
- Հատուկ պարամետրեր. կատարողի կողմից սահմանված, փոխադրված որպես `Json`, նույնականացված `CustomParameterId`-ով (a `Name`):

## ISI (Iroha Հատուկ հրահանգներ)- Հիմնական հատկանիշ՝ `Instruction`՝ `dyn_encode`, `as_any` և յուրաքանչյուր տեսակի կայուն նույնացուցիչ՝ `id()` (կանխադրված է կոնկրետ տեսակի անվանման համար): Բոլոր հրահանգները `Send + Sync + 'static` են:
- `InstructionBox`. պատկանող `Box<dyn Instruction>` փաթաթան clone/eq/ord-ով, որն իրականացվում է տիպի ID + կոդավորված բայթերի միջոցով:
- Ներկառուցված հրահանգների ընտանիքները կազմակերպվում են հետևյալ կերպ.
  - `mint_burn`, `transfer`, `register` և `transparent` օգնականների փաթեթ:
  - Մուտքագրեք թվեր մետա հոսքերի համար՝ `InstructionType`, արկղային գումարներ, ինչպիսիք են `SetKeyValueBox` (տիրույթ/հաշիվ/asset_def/nft/trigger):
- Սխալներ. հարուստ սխալի մոդել `isi::error`-ի ներքո (գնահատման տիպի սխալներ, սխալների հայտնաբերում, հատման հնարավորություն, մաթեմատիկա, անվավեր պարամետրեր, կրկնություն, անփոփոխություններ):
- Հրահանգների գրանցամատյան․ Օգտագործվում է `InstructionBox` կլոնի և Norito սերդի կողմից՝ դինամիկ (ապ)սերիալիզացիայի հասնելու համար: Եթե ​​`set_instruction_registry(...)`-ի միջոցով ոչ մի ռեեստր հստակ սահմանված չէ, ապա ներկառուցված լռելյայն գրանցամատյանը՝ բոլոր հիմնական ISI-ով, ծուլորեն տեղադրվում է առաջին օգտագործման ժամանակ՝ երկուականներն ամուր պահելու համար:

## Գործարքներ- `Executable`՝ կամ `Instructions(ConstVec<InstructionBox>)` կամ `Ivm(IvmBytecode)`: `IvmBytecode` սերիականացվում է որպես base64 (թափանցիկ նոր տիպ `Vec<u8>`-ի նկատմամբ):
- `TransactionBuilder`. կառուցում է գործարքի օգտակար բեռ `chain`, `authority`, `creation_time_ms`, կամընտիր `time_to_live_ms` և Kotodama, Kotodama, Kotodama, Kotodama, `Executable`.
  - Օգնողներ՝ `with_instructions`, `with_bytecode`, `with_executable`, `with_metadata`, `set_nonce`, `set_ttl`, `set_ttl`, I1828NI00000242X
- `SignedTransaction` (տարբերակված `iroha_version`-ով). կրում է `TransactionSignature` և օգտակար բեռ; ապահովում է հեշինգ և ստորագրության ստուգում:
- Մուտքի կետեր և արդյունքներ.
  - `TransactionEntrypoint`՝ `External(SignedTransaction)` | `Time(TimeTriggerEntrypoint)`.
  - `TransactionResult` = `Result<DataTriggerSequence, TransactionRejectionReason>` հեշավորման օգնականներով:
  - `ExecutionStep(ConstVec<InstructionBox>)`՝ հանձնարարականների մեկ պատվիրված խմբաքանակ գործարքում:

## Բլոկներ- `SignedBlock` (տարբերակված) ներառում է.
  - `signatures: BTreeSet<BlockSignature>` (վավերատորներից),
  - `payload: BlockPayload { header: BlockHeader, transactions: Vec<SignedTransaction> }`,
  - `result: BlockResult` (կատարման երկրորդական վիճակ), որը պարունակում է `time_triggers`, մուտք/արդյունք Merkle ծառեր, `transaction_results` և `fastpq_transcripts: BTreeMap<Hash, Vec<TransferTranscript>>`:
- Կոմունալ ծառայություններ.
- Merkle արմատներ. գործարքների մուտքի կետերը և արդյունքները կատարվում են Merkle ծառերի միջոցով; արդյունքում Merkle արմատը տեղադրվում է բլոկի վերնագրի մեջ:
- Արգելափակման ընդգրկման ապացույցները (`BlockProofs`) բացահայտում են ինչպես մուտքի/արդյունքի Merkle-ի ապացույցները, այնպես էլ `fastpq_transcripts` քարտեզը, որպեսզի շղթայից դուրս պրովերները կարողանան առբերել փոխանցման դելտաները, որոնք կապված են գործարքի հեշի հետ:
- `ExecWitness` հաղորդագրությունները (հեռարձակվում են Torii-ի միջոցով և հիմնված են համախոհության բամբասանքների վրա) այժմ ներառում են և՛ `fastpq_transcripts`, և՛ `fastpq_batches: Vec<FastpqTransitionBatch>`՝ ներկառուցված `fastpq_transcripts`, և `fastpq_batches: Vec<FastpqTransitionBatch>`՝ ներկառուցված `fastpq_batches: Vec<FastpqTransitionBatch>`-ով (Kotodama) perm_root, tx_set_hash), այնպես որ արտաքին պրովերները կարող են կլանել կանոնական FASTPQ տողեր՝ առանց վերակոդավորման տառադարձումների:

## Հարցումներ- Երկու համ.
  - Եզակի. ներդրում `SingularQuery<Output>` (օրինակ՝ `FindParameters`, `FindExecutorDataModel`):
  - Կրկնվող. իրականացնել `Query<Item>` (օրինակ՝ `FindAccounts`, `FindAssets`, `FindDomains` և այլն):
- Տիպի ջնջված ձևեր.
  - `QueryBox<T>`-ը տուփով, ջնջված `Query<Item = T>` է Norito սերդով, որն ապահովված է համաշխարհային ռեգիստրով:
  - `QueryWithFilter<T> { query, predicate, selector }` հարցումը զուգակցում է DSL պրեդիկատի/ընտրիչի հետ; վերածվում է ջնջված կրկնվող հարցման՝ `From`-ի միջոցով:
- Ռեեստր և կոդեկներ.
  - `query_registry!{ ... }`-ը կառուցում է գլոբալ ռեգիստր, որը քարտեզագրում է կոնկրետ հարցումների տեսակները կոնստրուկտորներին ըստ տիպի անվան՝ դինամիկ վերծանման համար:
  - `QueryRequest = Singular(SingularQueryBox) | Start(QueryWithParams) | Continue(ForwardCursor)` և `QueryResponse = Singular(..) | Iterable(QueryOutput)`:
  - `QueryOutputBatchBox`-ը միատարր վեկտորների (օրինակ՝ `Vec<Account>`, `Vec<Name>`, `Vec<AssetDefinition>`, `Vec<AssetDefinition>`, `Vec<BlockHeader>`) գումարի տիպ է միատարր վեկտորների վրա, գումարած արդյունավետ բազմակի և ընդլայնման օգնականների համար:
- DSL: Իրականացված է `query::dsl`-ում՝ պրոյեկցիոն գծերով (`HasProjection<PredicateMarker>` / `SelectorMarker`) կոմպիլյացիայի ժամանակով ստուգված պրեդիկատների և ընտրիչների համար: `fast_dsl` ֆունկցիան անհրաժեշտության դեպքում բացահայտում է ավելի թեթև տարբերակ:

## Կատարող և ընդարձակելիություն- `Executor { bytecode: IvmBytecode }`. վավերացնողի կողմից կատարված կոդերի փաթեթ:
- `ExecutorDataModel { parameters: CustomParameters, instructions: BTreeSet<Ident>, permissions: BTreeSet<Ident>, schema: Json }`-ը հայտարարում է կատարողի կողմից սահմանված տիրույթը՝
  - Պատվերով կազմաձևման պարամետրեր,
  - Պատվերով հրահանգների նույնացուցիչներ,
  - Թույլտվության նշանների նույնացուցիչներ,
  - JSON սխեման, որը նկարագրում է հաճախորդի գործիքավորման հատուկ տեսակները:
- Անհատականացման նմուշները գոյություն ունեն `data_model/samples/executor_custom_data_model`-ի ներքո, որոնք ցույց են տալիս.
  - Պատվերով թույլտվության նշան `iroha_executor_data_model::permission::Permission` բխող միջոցով,
  - Հատուկ պարամետր, որը սահմանվում է որպես `CustomParameter`-ի փոխարկվող տեսակ,
  - Պատվերով հրահանգները սերիականացված են `CustomInstruction`-ում՝ կատարման համար:

#

## CustomInstruction (կատարողի կողմից սահմանված ISI)- Տեսակը՝ `isi::CustomInstruction { payload: Json }` կայուն մետաղալարով `"iroha.custom"`:
- Նպատակը. ծրար մասնավոր/կոնսորցիումային ցանցերում կատարողին հատուկ հրահանգների կամ նախատիպերի համար՝ առանց հանրային տվյալների մոդելի ճեղքման:
- Կատարողի կանխադրված վարքագիծը. `iroha_core`-ում ներկառուցված կատարողը չի կատարում `CustomInstruction` և բախվելու դեպքում խուճապի կմատնվի: Հատուկ կատարողը պետք է իջեցնի `InstructionBox`-ը մինչև `CustomInstruction` և վճռականորեն մեկնաբանի օգտակար բեռնվածությունը բոլոր վավերացնողների վրա:
- Norito. կոդավորում/վերծանում է `norito::codec::{Encode, Decode}`-ի միջոցով՝ ներառված սխեմայով; `Json` ծանրաբեռնվածությունը սերիականացված է դետերմինիստորեն: Երկկողմանի ուղևորությունները կայուն են այնքան ժամանակ, քանի դեռ հրահանգների գրանցամատյանը ներառում է `CustomInstruction` (դա լռելյայն ռեեստրի մաս է կազմում):
- IVM. Kotodama-ը հավաքվում է IVM բայթկոդի (`.to`) և կիրառման տրամաբանության առաջարկվող ուղին է: Օգտագործեք `CustomInstruction` միայն կատարողի մակարդակի ընդլայնումների համար, որոնք դեռ չեն կարող արտահայտվել Kotodama-ով: Ապահովեք դետերմինիզմ և նույնական կատարող երկուականներ հասակակիցների միջև:
- Ոչ հանրային ցանցերի համար. մի օգտագործեք հանրային շղթաների համար, որտեղ տարասեռ կատարողները վտանգի տակ են դնում կոնսենսուսի պատառաքաղները: Նախընտրեք առաջարկել նոր ներկառուցված ISI հոսանքին հակառակ, երբ ձեզ անհրաժեշտ են հարթակի առանձնահատկություններ:

## Մետատվյալներ- `Metadata(BTreeMap<Name, Json>)`. բանալին/արժեքի պահեստ՝ կցված մի քանի միավորների (`Domain`, `Account`, `AssetDefinition`, `Nft`, գործարկիչներ և գործարքներ):
- API՝ `contains`, `iter`, `get`, `insert` և (`transparent_api`-ով) `remove`:

## Առանձնահատկություններ և դետերմինիզմ

- Առանձնահատկություններ վերահսկում է կամընտիր API-ները (`std`, `json`, `transparent_api`, `ffi_export`, `ffi_import`, Kotodama, `fast_dsl`, `fast_dsl`, I01 `fault_injection`):
- Դետերմինիզմ. Ամբողջ սերիալիզացիան օգտագործում է Norito կոդավորումը, որպեսզի շարժական լինի ապարատում: IVM բայթկոդը անթափանց բայթ բլբ է; կատարումը չպետք է մտցնի ոչ դետերմինիստական ​​կրճատումներ։ Հոսթը հաստատում է գործարքները և մուտքագրում է IVM-ին դետերմինիստիկ կերպով:

#

## Թափանցիկ API (`transparent_api`)- Նպատակը. բացահայտում է ամբողջական, փոփոխական մուտք դեպի `#[model]` կառուցվածքներ/համարներ ներքին բաղադրիչների համար, ինչպիսիք են Torii-ը, կատարողները և ինտեգրման թեստերը: Առանց դրա, այդ տարրերը միտումնավոր անթափանց են, ուստի արտաքին SDK-ները տեսնում են միայն անվտանգ կոնստրուկտորներ և կոդավորված օգտակար բեռներ:
- Մեխանիկա. `iroha_data_model_derive::model` մակրոն վերագրում է յուրաքանչյուր հանրային դաշտ `#[cfg(feature = "transparent_api")] pub`-ով և պահում է անձնական պատճենը լռելյայն կառուցման համար: Գործառույթը միացնելը շեղում է այդ cfg-երը, ուստի `Account`, `Domain`, `Asset` և այլնի ապակառուցումը օրինական է դառնում դրանց որոշիչ մոդուլներից դուրս:
- Մակերեւույթի հայտնաբերում. արկղը արտահանում է `TRANSPARENT_API: bool` հաստատուն (ստեղծվում է կամ `transparent_api.rs` կամ `non_transparent_api.rs`): Ներքևի հոսանքով ծածկագիրը կարող է ստուգել այս դրոշը և ճյուղավորումը, երբ այն պետք է վերադառնա անթափանց օգնականներին:
- Միացնելով. ավելացրեք `features = ["transparent_api"]` կախվածությանը `Cargo.toml`-ում: Աշխատանքային տարածքի արկղերը, որոնց անհրաժեշտ է JSON պրոյեկցիան (օրինակ՝ `iroha_torii`) դրոշակն ավտոմատ կերպով փոխանցում է, սակայն երրորդ կողմի սպառողները պետք է անջատեն այն, քանի դեռ չեն վերահսկում տեղակայումը և չեն ընդունում API-ի ավելի լայն մակերեսը։

## Արագ օրինակներ

Ստեղծեք տիրույթ և հաշիվ, սահմանեք ակտիվ և կառուցեք գործարք՝ հրահանգներով.

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
let asset_def_id = AssetDefinitionId::new(
    "wonderland".parse().unwrap(),
    "usd".parse().unwrap(),
);
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

Հարցրեք հաշիվները և ակտիվները DSL-ով.

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

Օգտագործեք IVM խելացի պայմանագրային բայթ կոդը.

```rust
use iroha_data_model::prelude::*;

let bytecode = IvmBytecode::from_compiled(include_bytes!("contract.to").to_vec());
let tx = TransactionBuilder::new("dev-chain".parse().unwrap(), account_id.clone())
    .with_bytecode(bytecode)
    .sign(kp.private_key());
```

Ակտիվների սահմանման id / կեղծանունի արագ հղում (CLI + Torii):

```bash
# Register an asset definition with a canonical Base58 id + explicit name + alias
iroha ledger asset definition register \
  --id 66owaQmAQMuHxPzxUN3bqZ6FJfDa \
  --name pkr \
  --alias pkr#ubl.sbp

# Short alias form (no owner segment): <name>#<dataspace>
iroha ledger asset definition register \
  --id 66owaQmAQMuHxPzxUN3bqZ6FJfDa \
  --name pkr \
  --alias pkr#sbp

# Mint using alias + account components
iroha ledger asset mint \
  --definition-alias pkr#ubl.sbp \
  --account sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB \
  --quantity 500

# Resolve alias to the canonical Base58 id via Torii
curl -sS http://127.0.0.1:8080/v1/assets/aliases/resolve \
  -H 'content-type: application/json' \
  -d '{"alias":"pkr#ubl.sbp"}'
```Միգրացիոն նշում.
- Հին `name#domain` ակտիվների սահմանման ID-ները չեն ընդունվում v1-ում:
- Հանրային ակտիվների ընտրիչներն օգտագործում են ակտիվների սահմանման միայն մեկ ձևաչափ՝ կանոնական Base58 ID-ներ: Փոխանունները մնում են ընտրովի ընտրիչներ, բայց լուծվում են նույն կանոնական ID-ով:
- Հանրային ակտիվների որոնումները հասցեագրում են սեփականության մնացորդները `asset + account + optional scope`-ով; հում կոդավորված `AssetId` տառերը ներքին ներկայացում են և չեն հանդիսանում Torii/CLI ընտրիչի մակերեսի մաս:
- `POST /v1/assets/definitions/query` և `GET /v1/assets/definitions` ընդունում են ակտիվների սահմանման զտիչներ/տեսակավորում `alias_binding.status`, `alias_binding.lease_expiry_ms`, `alias_binding.grace_until_ms` և `alias_binding.bound_at_ms`, և `alias_binding.bound_at_ms`, և `alias_binding.bound_at_ms`-ի նկատմամբ: `name`, `alias` և `metadata.*`:

## Տարբերակում

- `SignedTransaction`, `SignedBlock` և `SignedQuery` կանոնական Norito կոդավորված կառուցվածքներ են: Յուրաքանչյուրը կիրառում է `iroha_version::Version`՝ իր օգտակար բեռնվածությունը նախածանցելու համար ընթացիկ ABI տարբերակի հետ (ներկայումս `1`), երբ կոդավորված է `EncodeVersioned`-ի միջոցով:

## Վերանայման նշումներ / Հնարավոր թարմացումներ

- Հարցում DSL. հաշվի առեք փաստաթղթավորելու կայուն օգտատերերի ենթաբազմություն և օրինակներ ընդհանուր ֆիլտրերի/ընտրիչների համար:
- Հրահանգների ընտանիքներ. ընդլայնել հանրային փաստաթղթերը, որոնք թվարկում են ներկառուցված ISI տարբերակները, որոնք ենթարկվում են `mint_burn`, `register`, `transfer`:

---
Եթե որևէ մասի կարիք ունի ավելի շատ խորություն (օրինակ՝ ամբողջական ISI կատալոգ, ամբողջական հարցումների ռեեստրի ցուցակ կամ արգելափակել վերնագրի դաշտերը), տեղեկացրեք ինձ, և ես համապատասխանաբար կընդլայնեմ այդ բաժինները: