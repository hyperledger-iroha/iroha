---
lang: hy
direction: ltr
source: docs/source/data_model.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8b6388355a41797eb7d0b7f47cfa8fcac4e136c5a2e5eb0a264384ecdba930b8
source_last_modified: "2026-02-01T13:51:49.945202+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Iroha v2 Տվյալների մոդել – Խորը սուզում

Այս փաստաթուղթը բացատրում է կառուցվածքները, նույնացուցիչները, հատկությունները և արձանագրությունները, որոնք կազմում են Iroha v2 տվյալների մոդելը, ինչպես ներդրվել է `iroha_data_model` տուփում և օգտագործվում է աշխատանքային տարածքում: Այն նախատեսված է որպես ճշգրիտ հղում, որը կարող եք վերանայել և առաջարկել թարմացումներ:

## Շրջանակ և հիմքեր

- Նպատակը. Տրամադրել կանոնական տիպեր տիրույթի օբյեկտների համար (տիրույթներ, հաշիվներ, ակտիվներ, NFT-ներ, դերեր, թույլտվություններ, գործընկերներ), վիճակի փոփոխման հրահանգներ (ISI), հարցումներ, գործարկիչներ, գործարքներ, բլոկներ և պարամետրեր:
- Սերիալացում. Բոլոր հանրային տեսակները ստանում են Norito կոդեկներ (`norito::codec::{Encode, Decode}`) և սխեման (`iroha_schema::IntoSchema`): JSON-ն օգտագործվում է ընտրովի (օրինակ՝ HTTP և `Json` օգտակար բեռների համար) գործառույթների դրոշների հետևում:
- IVM նշում. որոշ ապասերիալացման ժամանակի վավերացումներն անջատված են, երբ թիրախավորում են Iroha վիրտուալ մեքենան (IVM), քանի որ հոսթն իրականացնում է վավերացում նախքան պայմանագրերը կանչելը (տե՛ս արկղային փաստաթղթերը I100340X-ում):
- FFI դարպասներ. որոշ տեսակներ պայմանականորեն նշում են FFI-ի համար `iroha_ffi`-ի միջոցով `ffi_export`/`ffi_import`-ի հետևում, որպեսզի խուսափեն գերավճարներից, երբ FFI-ի կարիք չկա:

## Հիմնական հատկություններ և օգնականներ

- `Identifiable`. Կազմակերպություններն ունեն կայուն `Id` և `fn id(&self) -> &Self::Id`: Պետք է ստացվի `IdEqOrdHash`-ի հետ՝ քարտեզի/կոմպլեկտների հարմարավետության համար:
- `Registrable`/`Registered`. Շատ կազմակերպություններ (օրինակ՝ `Domain`, `AssetDefinition`, `Role`) օգտագործում են շինարարական օրինաչափություն: `Registered`-ը գործարկման ժամանակի տեսակը կապում է թեթև շինարարական տեսակի (`With`) հետ, որը հարմար է գրանցման գործարքների համար:
- `HasMetadata`. միասնական մուտք դեպի `Metadata` բանալի/արժեքի քարտեզ:
- `IntoKeyValue`. Պահպանման բաժանման օգնական՝ `Key` (ID) և `Value` (տվյալներ) առանձին պահելու համար՝ կրկնօրինակումը նվազեցնելու համար:
- `Owned<T>`/`Ref<'world, K, V>`. Թեթև փաթաթիչներ, որոնք օգտագործվում են պահեստներում և հարցումների զտիչներում՝ ավելորդ պատճեններից խուսափելու համար:

## Անուններ և նույնացուցիչներ

- `Name`. Վավեր տեքստային նույնացուցիչ: Թույլ չի տալիս բացատները և վերապահված նիշերը `@`, `#`, `$` (օգտագործվում են կոմպոզիտային ID-ներում): Կառուցելի է `FromStr`-ի միջոցով՝ վավերացումով: Անունները վերլուծվում են Unicode NFC-ով (կանոնականորեն համարժեք ուղղագրությունները համարվում են նույնական և պահվում են կազմված): Հատուկ անունը `genesis` վերապահված է (ստուգված է մեծատառերի համար անզգայուն):
- `IdBox`. Գումարի տիպի ծրար ցանկացած աջակցվող ID-ի համար (`DomainId`, `AccountId`, `AssetDefinitionId`, `AssetId`, `AssetId`, Norito `TriggerId`, `RoleId`, `Permission`, `CustomParameterId`): Օգտակար է ընդհանուր հոսքերի և Norito կոդավորման համար որպես մեկ տեսակ:
- `ChainId`. Անթափանց շղթայի նույնացուցիչ, որն օգտագործվում է գործարքներում կրկնակի պաշտպանության համար:ID-ների լարային ձևեր (շրջադարձային `Display`/`FromStr`-ով).
- `DomainId`: `name` (օրինակ՝ `wonderland`):
- `AccountId`. կանոնական նույնացուցիչ՝ կոդավորված `AccountAddress`-ի միջոցով, որը բացահայտում է IH58, Sora սեղմված (`sora…`) և կանոնական վեցանկյուն կոդեկներ (Kotodama, Kotodama, `canonical_hex`, `parse_encoded`): IH58-ը հաշվի նախընտրելի ձևաչափն է. `sora…` ձևը երկրորդ լավագույնն է միայն Sora-ի UX-ի համար: `alias` (rejected legacy form) մարդու համար հարմար երթուղային կեղծանունը պահպանվել է UX-ի համար, սակայն այն այլևս չի դիտարկվում որպես հեղինակավոր նույնացուցիչ: Torii-ը նորմալացնում է մուտքային տողերը `AccountAddress::parse_encoded`-ի միջոցով: Հաշվի ID-ներն աջակցում են ինչպես մեկ բանալիով, այնպես էլ բազմանշանակ կարգավորիչներով:
- `AssetDefinitionId`՝ `asset#domain` (օրինակ՝ `xor#soramitsu`):
- `AssetId`: canonical encoded literal `norito:<hex>` (legacy textual forms are not supported in first release).
- `NftId`՝ `nft$domain` (օրինակ՝ `rose$garden`):
- `PeerId`՝ `public_key` (հասակակիցների հավասարությունը հանրային բանալին է):

## Սուբյեկտներ

### տիրույթ
- `DomainId { name: Name }` – եզակի անուն:
- `Domain { id, logo: Option<IpfsPath>, metadata: Metadata, owned_by: AccountId }`.
- Կառուցող՝ `NewDomain` `with_logo`, `with_metadata`, ապա `Registrable::build(authority)` հավաքածուներ `owned_by`:

### Հաշիվ
- `AccountId { domain: DomainId, controller: AccountController }` (վերահսկիչ = մեկ բանալի կամ բազմանշանակ քաղաքականություն):
- `Account { id, metadata, label?, uaid? }` — `label` կամընտիր կայուն կեղծանունն է, որն օգտագործվում է վերաբանալու գրառումների կողմից, `uaid`-ը կրում է կամընտիր Nexus լայնածավալ [Universal Account ID] (Kotodama):
- Կառուցող՝ `NewAccount` `Account::new(id)`-ի միջոցով; `HasMetadata` և՛ շինարարի, և՛ կազմակերպության համար:

### Ակտիվների սահմանումներ և ակտիվներ
- `AssetDefinitionId { domain: DomainId, name: Name }`.
- `AssetDefinition { id, spec: NumericSpec, mintable: Mintable, logo: Option<IpfsPath>, metadata, owned_by: AccountId, total_quantity: Numeric }`.
  - `Mintable`՝ `Infinitely` | `Once` | `Limited(u32)` | `Not`.
  - Շինարարներ՝ `AssetDefinition::new(id, spec)` կամ հարմարավետ `numeric(id)`; կարգավորիչներ `metadata`, `mintable`, `owned_by` համար:
- `AssetId { account: AccountId, definition: AssetDefinitionId }`.
- `Asset { id, value: Numeric }` պահեստավորման համար հարմար `AssetEntry`/`AssetValue`:
- `AssetTotalQuantityMap = BTreeMap<AssetDefinitionId, Numeric>` ցուցադրվում է ամփոփ API-ների համար:

### NFTs
- `NftId { domain: DomainId, name: Name }`.
- `Nft { id, content: Metadata, owned_by: AccountId }` (բովանդակությունը կամայական բանալի/արժեքի մետատվյալներ է):
- Կառուցող՝ `NewNft` `Nft::new(id, content)`-ի միջոցով:

### Դերեր և թույլտվություններ
- `RoleId { name: Name }`.
- `Role { id, permissions: BTreeSet<Permission> }` շինարարով `NewRole { inner: Role, grant_to: AccountId }`:
- `Permission { name: Ident, payload: Json }` – `name` և օգտակար բեռնվածության սխեման պետք է համապատասխանի ակտիվ `ExecutorDataModel`-ին (տես ստորև):

### Հասակակիցներ
- `PeerId { public_key: PublicKey }`.
- `Peer { address: SocketAddr, id: PeerId }` և վերլուծելի `public_key@address` լարային ձև:### Կրիպտոգրաֆիկ պրիմիտիվներ (հատկանիշ `sm`)
- `Sm2PublicKey` և `Sm2Signature`. SEC1-ին համապատասխանող կետեր և ֆիքսված լայնությամբ `r∥s` ստորագրություններ SM2-ի համար: Կառուցիչները վավերացնում են կորի անդամակցությունը և տարբերակիչ ID-ները. Norito կոդավորումը արտացոլում է `iroha_crypto`-ի կողմից օգտագործված կանոնական ներկայացումը:
- `Sm3Hash`. `[u8; 32]` նոր տիպ, որը ներկայացնում է GM/T 0004 մարսողությունը, որն օգտագործվում է մանիֆեստների, հեռաչափության և համակարգային պատասխաններում:
- `Sm4Key`. 128-բիթանոց սիմետրիկ բանալիների փաթաթան, որը համօգտագործվում է հյուրընկալող համակարգերի և տվյալների մոդելի հարմարանքների միջև:
Այս տեսակները նստում են գոյություն ունեցող Ed25519/BLS/ML-DSA պրիմիտիվների կողքին և դառնում հանրային սխեմայի մաս, երբ աշխատանքային տարածքը կառուցվի `--features sm`-ով:

### Գործարկիչներ և իրադարձություններ
- `TriggerId { name: Name }` և `Trigger { id, action: action::Action }`:
- `action::Action { executable: Executable, repeats: Repeats, authority: AccountId, filter: EventFilterBox, metadata }`.
  - `Repeats`: `Indefinitely` կամ `Exactly(u32)`; ներառյալ պատվիրման և սպառման կոմունալ ծառայությունները:
  - Անվտանգություն. `TriggerCompleted`-ը չի կարող օգտագործվել որպես գործողության զտիչ (հաստատված (ապ)սերիալիզացիայի ժամանակ):
- `EventBox`. խողովակաշարի, խողովակաշարի խմբաքանակի, տվյալների, ժամանակի, գործարկման գործարկման և գործարկիչով ավարտված իրադարձությունների գումարի տեսակը. `EventFilterBox` հայելիներ, որոնք նախատեսված են բաժանորդագրությունների և գործարկման զտիչների համար:

## Պարամետրեր և կազմաձևում

- Համակարգի պարամետրերի ընտանիքներ (բոլոր `Default`ed, կրող ստացողներ և փոխարկվում են առանձին թվերի).
- `SumeragiParameters { block_time_ms, commit_time_ms, min_finality_ms, pacing_factor_bps, max_clock_drift_ms, collectors_k, collectors_redundant_send_r }`.
  - `BlockParameters { max_transactions: NonZeroU64 }`.
  - `TransactionParameters { max_signatures, max_instructions, ivm_bytecode_size, max_tx_bytes, max_decompressed_bytes }`.
  - `SmartContractParameters { fuel, memory, execution_depth }`.
- `Parameters` խմբավորում է բոլոր ընտանիքները և `custom: BTreeMap<CustomParameterId, CustomParameter>`:
- Մեկ պարամետրով թվեր՝ `SumeragiParameter`, `BlockParameter`, `TransactionParameter`, `SmartContractParameter`՝ տարբերվող թարմացումների և կրկնությունների համար:
- Հատուկ պարամետրեր. կատարողի կողմից սահմանված, փոխադրված որպես `Json`, նույնականացված `CustomParameterId`-ով (a `Name`):

## ISI (Iroha Հատուկ հրահանգներ)

- Հիմնական հատկանիշ՝ `Instruction`՝ `dyn_encode`, `as_any` և յուրաքանչյուր տեսակի կայուն նույնացուցիչ՝ `id()` (կանխադրված է կոնկրետ տեսակի անվանման համար): Բոլոր հրահանգները `Send + Sync + 'static` են:
- `InstructionBox`. պատկանող `Box<dyn Instruction>` փաթաթան clone/eq/ord-ով, որն իրականացվում է ID տեսակի + կոդավորված բայթերի միջոցով:
- Ներկառուցված հրահանգների ընտանիքները կազմակերպվում են հետևյալ կերպ.
  - `mint_burn`, `transfer`, `register` և `transparent` օգնականների փաթեթ:
  - Մուտքագրեք թվեր մետա հոսքերի համար՝ `InstructionType`, արկղային գումարներ, ինչպիսիք են `SetKeyValueBox` (տիրույթ/հաշիվ/asset_def/nft/trigger):
- Սխալներ. հարուստ սխալի մոդել `isi::error`-ի ներքո (գնահատման տիպի սխալներ, սխալների հայտնաբերում, հատման հնարավորություն, մաթեմատիկա, անվավեր պարամետրեր, կրկնություն, անփոփոխություններ):
- Հրահանգների գրանցամատյան․ Օգտագործվում է `InstructionBox` կլոնի և Norito սերդի կողմից՝ դինամիկ (ապ)սերիալիզացիայի հասնելու համար: Եթե ​​`set_instruction_registry(...)`-ի միջոցով ոչ մի ռեեստր բացահայտորեն սահմանված չէ, ապա ներկառուցված լռելյայն ռեգիստրը բոլոր հիմնական ISI-ներով, ծուլորեն տեղադրվում է առաջին օգտագործման ժամանակ՝ երկուականներն ամուր պահելու համար:

## Գործարքներ- `Executable`՝ կամ `Instructions(ConstVec<InstructionBox>)` կամ `Ivm(IvmBytecode)`: `IvmBytecode`-ը սերիականացվում է որպես base64 (թափանցիկ նոր տիպ `Vec<u8>`-ի նկատմամբ):
- `TransactionBuilder`. կառուցում է գործարքի օգտակար բեռ՝ `chain`, `authority`, `creation_time_ms`, կամընտիր `time_to_live_ms` և Norito, Norito, Norito, Norito, Kotodama-ի հետ: `Executable`.
  - Օգնականներ՝ `with_instructions`, `with_bytecode`, `with_executable`, `with_metadata`, `set_nonce`, `set_ttl`, `set_ttl`, `with_executable`, `set_nonce`, `set_ttl`, Norito
- `SignedTransaction` (տարբերակված `iroha_version`-ով). կրում է `TransactionSignature` և օգտակար բեռ; ապահովում է հեշինգ և ստորագրության ստուգում:
- Մուտքի կետեր և արդյունքներ.
  - `TransactionEntrypoint`՝ `External(SignedTransaction)` | `Time(TimeTriggerEntrypoint)`.
  - `TransactionResult` = `Result<DataTriggerSequence, TransactionRejectionReason>` հեշինգի օգնականներով:
  - `ExecutionStep(ConstVec<InstructionBox>)`. հրահանգների մեկ պատվիրված խմբաքանակ գործարքում:

## Բլոկներ

- `SignedBlock` (տարբերակված) ներառում է.
  - `signatures: BTreeSet<BlockSignature>` (վավերատորներից),
  - `payload: BlockPayload { header: BlockHeader, transactions: Vec<SignedTransaction> }`,
  - `result: BlockResult` (կատարման երկրորդական վիճակ), որը պարունակում է `time_triggers`, մուտք/արդյունք Merkle ծառեր, `transaction_results` և `fastpq_transcripts: BTreeMap<Hash, Vec<TransferTranscript>>`:
- Կոմունալ ծառայություններ.
- Merkle արմատներ. գործարքների մուտքի կետերը և արդյունքները կատարվում են Merkle ծառերի միջոցով; արդյունքում Merkle արմատը տեղադրվում է բլոկի վերնագրի մեջ:
- Արգելափակման ընդգրկման ապացույցները (`BlockProofs`) բացահայտում են ինչպես մուտքի/արդյունքի Merkle ապացույցները, այնպես էլ `fastpq_transcripts` քարտեզը, որպեսզի շղթայից դուրս պրովերները կարողանան առբերել փոխանցման դելտաները, որոնք կապված են գործարքի հեշի հետ:
- `ExecWitness` հաղորդագրությունները (հեռարձակվել են Torii-ի միջոցով և հիմնված են կոնսենսուսի բամբասանքների վրա) այժմ ներառում են և՛ `fastpq_transcripts`, և՛ `fastpq_batches: Vec<FastpqTransitionBatch>` ներկառուցված `fastpq_transcripts`, և `fastpq_batches: Vec<FastpqTransitionBatch>`՝ ներկառուցված I180000000024X-ով (Norito, ներկառուցված Kotodama perm_root, tx_set_hash), այնպես որ արտաքին պրովերները կարող են կլանել կանոնական FASTPQ տողեր՝ առանց վերակոդավորման տառադարձումների:

## Հարցումներ

- Երկու համ.
  - Եզակի. ներդրում `SingularQuery<Output>` (օրինակ՝ `FindParameters`, `FindExecutorDataModel`):
  - Կրկնվող՝ իրականացնել `Query<Item>` (օրինակ՝ `FindAccounts`, `FindAssets`, `FindDomains` և այլն):
- Տիպի ջնջված ձևեր.
  - `QueryBox<T>`-ը տուփով, ջնջված `Query<Item = T>` է Norito սերդով, որն ապահովված է համաշխարհային ռեգիստրով:
  - `QueryWithFilter<T> { query, predicate, selector }` հարցումը զուգակցում է DSL պրեդիկատի/ընտրիչի հետ; վերածվում է ջնջված կրկնվող հարցման՝ `From`-ի միջոցով:
- Ռեեստր և կոդեկներ.
  - `query_registry!{ ... }`-ը կառուցում է գլոբալ ռեգիստր, որը քարտեզագրում է կոնկրետ հարցումների տեսակները կոնստրուկտորներին ըստ տիպի անվան՝ դինամիկ վերծանման համար:
  - `QueryRequest = Singular(SingularQueryBox) | Start(QueryWithParams) | Continue(ForwardCursor)` և `QueryResponse = Singular(..) | Iterable(QueryOutput)`:
  - `QueryOutputBatchBox`-ը միատարր վեկտորների (օրինակ՝ `Vec<Account>`, `Vec<Name>`, `Vec<AssetDefinition>`, `Vec<AssetDefinition>`, `Vec<BlockHeader>`) գումարի տեսակ է համասեռ վեկտորների վրա, գումարած արդյունավետ բազմապատկման և ընդլայնման օգնականների համար:
- DSL: Իրականացված է `query::dsl`-ում՝ պրոյեկցիոն հատկանիշներով (`HasProjection<PredicateMarker>` / `SelectorMarker`)՝ կոմպիլյացիայի ժամանակով ստուգված պրեդիկատների և ընտրիչների համար: `fast_dsl` ֆունկցիան անհրաժեշտության դեպքում բացահայտում է ավելի թեթև տարբերակ:

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

### CustomInstruction (կատարողի կողմից սահմանված ISI)

- Տեսակը՝ `isi::CustomInstruction { payload: Json }` կայուն մետաղալարով `"iroha.custom"`:
- Նպատակը. ծրար մասնավոր/կոնսորցիումային ցանցերում կատարողին հատուկ հրահանգների կամ նախատիպերի համար՝ առանց հանրային տվյալների մոդելի ճեղքման:
- Կատարողի կանխադրված վարքագիծը. `iroha_core`-ում ներկառուցված կատարողը չի կատարում `CustomInstruction` և բախվելու դեպքում խուճապի կմատնվի: Հատուկ կատարողը պետք է իջեցնի `InstructionBox`-ը մինչև `CustomInstruction` և վճռականորեն մեկնաբանի օգտակար բեռնվածությունը բոլոր վավերացնողների վրա:
- Norito. կոդավորում/վերծանում է `norito::codec::{Encode, Decode}`-ի միջոցով՝ ներառված սխեմայով; `Json` օգտակար բեռը սերիականացված է դետերմինիստականորեն: Երկկողմանի ուղևորությունները կայուն են այնքան ժամանակ, քանի դեռ հրահանգների գրանցամատյանը ներառում է `CustomInstruction` (դա լռելյայն ռեեստրի մաս է կազմում):
- IVM. Kotodama-ը հավաքվում է IVM բայթկոդի (`.to`) և կիրառման տրամաբանության առաջարկվող ուղին է: Օգտագործեք `CustomInstruction` միայն կատարողի մակարդակի ընդլայնումների համար, որոնք դեռ չեն կարող արտահայտվել Kotodama-ով: Ապահովեք դետերմինիզմ և նույնական կատարող երկուականներ հասակակիցների միջև:
- Ոչ հանրային ցանցերի համար. մի օգտագործեք հանրային շղթաների համար, որտեղ տարասեռ կատարողները վտանգի տակ են դնում կոնսենսուսի պատառաքաղները: Նախընտրեք առաջարկել նոր ներկառուցված ISI հոսանքին հակառակ, երբ ձեզ անհրաժեշտ են հարթակի առանձնահատկություններ:

## Մետատվյալներ

- `Metadata(BTreeMap<Name, Json>)`. բանալին/արժեքի պահեստ՝ կցված է բազմաթիվ սուբյեկտների (`Domain`, `Account`, `AssetDefinition`, `Nft`, գործարկիչներ և գործարքներ):
- API՝ `contains`, `iter`, `get`, `insert` և (`transparent_api`-ով) `remove`:

## Առանձնահատկություններ և դետերմինիզմ

- Առանձնահատկություններ վերահսկում է կամընտիր API-ները (`std`, `json`, `transparent_api`, `ffi_export`, `ffi_import`, Norito, Norito, `fast_dsl`, I001 `fault_injection`):
- Դետերմինիզմ. Ամբողջ սերիալիզացիան օգտագործում է Norito կոդավորումը, որպեսզի շարժական լինի ապարատում: IVM բայթկոդը անթափանց բայթ բլբ է; կատարումը չպետք է մտցնի ոչ դետերմինիստական ​​կրճատումներ։ Հոսթը հաստատում է գործարքները և մուտքագրում է IVM-ին դետերմինիստիկ կերպով:

### Թափանցիկ API (`transparent_api`)- Նպատակը. բացահայտում է ամբողջական, փոփոխական մուտք դեպի `#[model]` կառուցվածքներ/համարներ ներքին բաղադրիչների համար, ինչպիսիք են Torii, կատարողները և ինտեգրման թեստերը: Առանց դրա, այդ տարրերը միտումնավոր անթափանց են, ուստի արտաքին SDK-ները տեսնում են միայն անվտանգ կոնստրուկտորներ և կոդավորված օգտակար բեռներ:
- Մեխանիկա. `iroha_data_model_derive::model` մակրոն վերագրում է յուրաքանչյուր հանրային դաշտ `#[cfg(feature = "transparent_api")] pub`-ով և պահում է անձնական պատճենը լռելյայն կառուցման համար: Գործառույթը միացնելը շեղում է այդ cfg-երը, ուստի `Account`, `Domain`, `Asset` և այլնի ապակառուցումը դառնում է օրինական՝ իրենց որոշիչ մոդուլներից դուրս:
- Մակերեւույթի հայտնաբերում. վանդակը արտահանում է `TRANSPARENT_API: bool` հաստատուն (ստեղծվում է կամ `transparent_api.rs` կամ `non_transparent_api.rs`): Ներքևի հոսանքով ծածկագիրը կարող է ստուգել այս դրոշը և ճյուղավորումը, երբ այն պետք է վերադառնա անթափանց օգնականներին:
- Միացնելով. ավելացրեք `features = ["transparent_api"]` կախվածությանը `Cargo.toml`-ում: Աշխատանքային տարածքի տուփերը, որոնց անհրաժեշտ է JSON պրոյեկցիան (օրինակ՝ `iroha_torii`) դրոշակն ավտոմատ կերպով փոխանցում է, սակայն երրորդ կողմի սպառողները պետք է անջատեն այն, քանի դեռ չեն վերահսկում տեղակայումը և չեն ընդունում API-ի ավելի լայն մակերեսը։

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

## Տարբերակում

- `SignedTransaction`, `SignedBlock` և `SignedQuery` կանոնական Norito կոդավորված կառուցվածքներ են: Յուրաքանչյուրը կիրառում է `iroha_version::Version`՝ իր օգտակար բեռը նախածանցելու համար ընթացիկ ABI տարբերակի հետ (ներկայումս `1`), երբ կոդավորված է `EncodeVersioned`-ի միջոցով:

## Վերանայման նշումներ / Հնարավոր թարմացումներ

- Հարցում DSL. հաշվի առեք փաստաթղթավորելու կայուն օգտատերերի ենթաբազմություն և օրինակներ ընդհանուր ֆիլտրերի/ընտրիչների համար:
- Հրահանգների ընտանիքներ. ընդլայնել հանրային փաստաթղթերը, որոնք թվարկում են ներկառուցված ISI տարբերակները, որոնք ենթարկվում են `mint_burn`, `register`, `transfer`:

---
Եթե որևէ մասի կարիք ունի ավելի շատ խորություն (օրինակ՝ ամբողջական ISI կատալոգ, ամբողջական հարցումների ռեեստրի ցուցակ կամ արգելափակել վերնագրի դաշտերը), տեղեկացրեք ինձ, և ես համապատասխանաբար կընդլայնեմ այդ բաժինները: