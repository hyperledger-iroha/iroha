<!-- Auto-generated stub for Armenian (hy) translation. Replace this content with the full translation. -->

---
lang: hy
direction: ltr
source: docs/source/data_model_and_isi_spec.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a8d13f6d206f60d31217ed093a5bbedd7946d27b644f9b3321a577cc6065a901
source_last_modified: "2026-03-30T18:22:55.965549+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# Iroha v2 Տվյալների մոդել և ISI — Իրականացումից ստացված սպեկտր

Այս մասնագրերը հակադարձ նախագծված են `iroha_data_model` և `iroha_core`-ի ընթացիկ ներդրումից՝ դիզայնի վերանայմանը նպաստելու համար: Հետադարձ կապի ուղիները մատնանշում են հեղինակավոր կոդը:

## Շրջանակ
- Սահմանում է կանոնական սուբյեկտները (տիրույթներ, հաշիվներ, ակտիվներ, NFT-ներ, դերեր, թույլտվություններ, գործընկերներ, գործարկիչներ) և դրանց նույնացուցիչները:
- Նկարագրում է վիճակի փոփոխման հրահանգները (ISI)՝ տեսակները, պարամետրերը, նախապայմանները, վիճակների անցումները, արտանետվող իրադարձությունները և սխալի պայմանները:
- Ամփոփում է պարամետրերի կառավարումը, գործարքները և հրահանգների սերիականացումը:

Դետերմինիզմ. Բոլոր հրահանգների իմաստաբանությունը մաքուր վիճակի անցումներ են՝ առանց սարքաշարից կախված վարքագծի: Սերիալիզացիան օգտագործում է Norito; VM բայթկոդը օգտագործում է IVM-ը և վավերացված է հոսթի կողմում՝ նախքան շղթայական գործարկումը:

---

## Սուբյեկտներ և նույնացուցիչներ
ID-ներն ունեն կայուն լարային ձևեր՝ `Display`/`FromStr` հետադարձ ճանապարհով: Անվան կանոններն արգելում են բացատները և վերապահված `@ # $` նիշերը:- `Name` — վավերացված տեքստային նույնացուցիչ: Կանոններ՝ `crates/iroha_data_model/src/name.rs`:
- `DomainId` — `name`. Դոմեն՝ `{ id, logo, metadata, owned_by }`: Շինարարներ՝ `NewDomain`: Կոդ՝ `crates/iroha_data_model/src/domain.rs`։
- `AccountId` — կանոնական հասցեները արտադրվում են `AccountAddress`-ի միջոցով, քանի որ I105-ը և Torii-ը նորմալացնում են մուտքերը `AccountAddress::parse_encoded`-ի միջոցով: Գործարկման ժամանակի խիստ վերլուծությունն ընդունում է միայն կանոնական I105: Շղթայական հաշվի կեղծանունները օգտագործում են `name@domain.dataspace` կամ `name@dataspace` և որոշում են `AccountId` կանոնական արժեքները; դրանք չեն ընդունվում խիստ `AccountId` վերլուծողների կողմից: Հաշիվ՝ `{ id, metadata }`: Կոդ՝ `crates/iroha_data_model/src/account.rs`։- Հաշվի ընդունման քաղաքականություն — տիրույթները վերահսկում են անուղղակի հաշվի ստեղծումը` պահելով Norito-JSON `AccountAdmissionPolicy` `iroha:account_admission_policy` մետատվյալների բանալու տակ: Երբ բանալին բացակայում է, շղթայի մակարդակի մաքսային պարամետրը `iroha:default_account_admission_policy` ապահովում է լռելյայն; երբ դա նույնպես բացակայում է, կոշտ կանխադրվածը `ImplicitReceive` է (առաջին թողարկում): Քաղաքականության պիտակները `mode` (`ExplicitOnly` կամ `ImplicitReceive`) գումարած կամընտիր մեկ գործարքի համար (կանխադրված `16`) և յուրաքանչյուր բլոկի ստեղծման գլխարկներ, կամընտիր Norito հաշիվ `min_initial_amounts` յուրաքանչյուր ակտիվի սահմանման համար և կամընտիր `default_role_on_create` (տրամադրվում է `AccountCreated`-ից հետո, մերժվում է `DefaultRoleError`-ով, եթե բացակայում է): Genesis-ը չի կարող մասնակցել. անջատված/անվավեր քաղաքականությունները մերժում են `InstructionExecutionError::AccountAdmission`-ով անհայտ հաշիվների անդորրագրի ոճի հրահանգները: Անուղղակի հաշիվների կնիքի մետատվյալները `iroha:created_via="implicit"` մինչև `AccountCreated`; լռելյայն դերերը թողարկում են հաջորդող `AccountRoleGranted`, իսկ կատարողի սեփականատեր-բազային կանոնները թույլ են տալիս նոր հաշվին ծախսել իր սեփական ակտիվները/NFT-ները՝ առանց լրացուցիչ դերերի: Կոդ՝ `crates/iroha_data_model/src/account/admission.rs`, `crates/iroha_core/src/smartcontracts/isi/account_admission.rs`։- `AssetDefinitionId` — կանոնական առանց նախածանցով Base58 հասցե կանոնական ակտիվների սահմանման բայթերի վրա: Սա հանրային ակտիվի ID-ն է: Սահմանում. `{ id, name, description?, alias?, spec: NumericSpec, mintable: Mintable, logo, metadata, owned_by, total_quantity }`: `alias` բառացիները պետք է լինեն `<name>#<domain>.<dataspace>` կամ `<name>#<dataspace>`, որոնց `<name>` հավասար է ակտիվի սահմանման անվանմանը, և դրանք վերաբերում են միայն Base58 ակտիվի կանոնական ID-ին: Կոդ՝ `crates/iroha_data_model/src/asset/definition.rs`։
  - Այլընտրանքային վարձակալության մետատվյալները պահպանվում են պահվող ակտիվների սահմանման տողից առանձին: Core/Torii նյութականացվում է `alias`-ը պարտադիր գրառումից, երբ ընթերցվում են սահմանումները:
  - Torii ակտիվների սահմանման պատասխանները մերկացնում են `alias_binding { alias, status, lease_expiry_ms, grace_until_ms, bound_at_ms }`, որտեղ `status`-ը `permanent`, `leased_active`, Norito, կամ Norito է:
  - Alias-ի ընտրիչները որոշում են բլոկի ստեղծման վերջին պարտավորված ժամանակի դեմ: `grace_until_ms`-ից հետո կեղծանունների ընտրիչները դադարում են լուծել, նույնիսկ եթե ֆոնային մաքրումը դեռ չի վերացրել հնացած կապը; ուղղակի սահմանման ընթերցումները դեռևս կարող են հաղորդել հնացած կապը որպես `expired_pending_cleanup`:
- `AssetId`՝ հանրային ակտիվի նույնացուցիչ՝ կանոնական մերկ Base58 ձևով: Ակտիվների անունները, ինչպիսիք են `name#dataspace` կամ `name#domain.dataspace`, լուծվում են `AssetId`-ով: Ներքին մատյանների պահումները կարող են լրացուցիչ ցուցադրել պառակտված `asset + account + optional dataspace` դաշտերը, որտեղ անհրաժեշտ է, բայց այդ կոմպոզիտային ձևը հանրային `AssetId` չէ:
- `NftId` — `nft$domain`. NFT՝ `{ id, content: Metadata, owned_by }`: Կոդ՝ `crates/iroha_data_model/src/nft.rs`։- `RoleId` — `name`. Դերը՝ `{ id, permissions: BTreeSet<Permission> }` շինարարով `NewRole { inner: Role, grant_to }`: Կոդ՝ `crates/iroha_data_model/src/role.rs`։
- `Permission` — `{ name: Ident, payload: Json }`. Կոդ՝ `crates/iroha_data_model/src/permission.rs`։
- `PeerId`/`Peer` — հասակակիցների ինքնությունը (հանրային բանալի) և հասցեն: Կոդ՝ `crates/iroha_data_model/src/peer.rs`։
- `TriggerId` — `name`. Գործարկիչ՝ `{ id, action }`: Գործողություն՝ `{ executable, repeats, authority, filter, metadata }`: Կոդ՝ `crates/iroha_data_model/src/trigger/`։
- `Metadata` — `BTreeMap<Name, Json>` նշված ներդիրով/հեռացնել: Կոդ՝ `crates/iroha_data_model/src/metadata.rs`։
- Բաժանորդագրության օրինակ (կիրառման շերտ). պլանները `AssetDefinition` գրառումներ են՝ `subscription_plan` մետատվյալներով; բաժանորդագրությունները `Nft` գրառումներն են՝ `subscription` մետատվյալներով; բիլինգը կատարվում է ժամանակային գործարկիչներով, որոնք հղում են կատարում բաժանորդագրության NFT-ներին: Տես `docs/source/subscriptions_api.md` և `crates/iroha_data_model/src/subscription.rs`:
- **Գաղտնագրված պրիմիտիվներ** (հատկանիշ `sm`):
  - `Sm2PublicKey` / `Sm2Signature` արտացոլում է SEC1 կանոնական կետը + ֆիքսված լայնությամբ `r∥s` կոդավորումը SM2-ի համար: Կառուցիչները պարտադրում են կորի անդամակցությունը և տարբերակիչ ID-ի իմաստաբանությունը (`DEFAULT_DISTID`), մինչդեռ ստուգումը մերժում է սխալ ձևավորված կամ բարձր տիրույթի սկալերը: Կոդ՝ `crates/iroha_crypto/src/sm.rs` և `crates/iroha_data_model/src/crypto/mod.rs`։
  - `Sm3Hash`-ը ցուցադրում է GM/T 0004 ամփոփումը որպես Norito-սերիալիզացվող `[u8; 32]` նոր տիպ, որն օգտագործվում է ամենուր, որտեղ հեշեր են հայտնվում մանիֆեստներում կամ հեռաչափում: Կոդ՝ `crates/iroha_data_model/src/crypto/hash.rs`։- `Sm4Key`-ը ներկայացնում է 128-բիթանոց SM4 ստեղներ և համօգտագործվում է հյուրընկալող համակարգերի և տվյալների մոդելի հարմարանքների միջև: Կոդ՝ `crates/iroha_data_model/src/crypto/symmetric.rs`։
  Այս տեսակները տեղավորվում են գոյություն ունեցող Ed25519/BLS/ML-DSA պրիմիտիվների կողքին և հասանելի են տվյալների մոդելի սպառողներին (Torii, SDK-ներ, genesis tooling) `sm` գործառույթը միացնելուց հետո:
- Տվյալների տարածությունից ստացված փոխհարաբերությունների պահոցներ (`space_directory_manifests`, `uaid_dataspaces`, `axt_policies`, `axt_replay_ledger`, գծի ռելեի արտակարգ իրավիճակների վերացման ռեեստր) և տվյալների տարածություն-թիրախային թույլտվություններ (I0120X10100X1) կտրվում են `State::set_nexus(...)`-ի վրա, երբ տվյալների տարածությունները անհետանում են ակտիվ `dataspace_catalog`-ից՝ կանխելով տվյալների տարածության հնացած հղումները գործարկման կատալոգի թարմացումներից հետո: Գոտի շրջանակով DA/ռելեի քեշերը (`lane_relays`, `da_commitments`, `da_confidential_compute`, `da_pin_intents`) նույնպես կտրված են, երբ գոտին անջատվում է կամ վերանշանակվում է տվյալների այլ տարածության մեջ, այնպես որ միգրացիան չի կարող փոխանցվել տվյալների տարածության այլ տարածքի: Space Directory ISI-ները (`PublishSpaceDirectoryManifest`, `RevokeSpaceDirectoryManifest`, `ExpireSpaceDirectoryManifest`) նույնպես վավերացնում են `dataspace`-ը ակտիվ կատալոգի հետ և մերժում են անհայտ ID-ները `InvalidParameter`-ով:

Կարևոր հատկանիշներ՝ `Identifiable`, `Registered`/`Registrable` (շինարարի օրինակ), `HasMetadata`, `IntoKeyValue`: Կոդ՝ `crates/iroha_data_model/src/lib.rs`։

Իրադարձություններ. Յուրաքանչյուր կազմակերպություն ունի մուտացիաների պատճառով թողարկված իրադարձություններ (ստեղծել/ջնջել/սեփականատերը փոխվել է/մետատվյալները փոխվել են և այլն): Կոդ՝ `crates/iroha_data_model/src/events/`։

---## Պարամետրեր (շղթայի կազմաձևում)
- Ընտանիքներ՝ `SumeragiParameters { block_time_ms, commit_time_ms, min_finality_ms, pacing_factor_bps, max_clock_drift_ms, collectors_k, collectors_redundant_send_r }`, `BlockParameters { max_transactions }`, `TransactionParameters { max_signatures, max_instructions, ivm_bytecode_size, max_tx_bytes, max_decompressed_bytes }`, `SmartContractParameters { fuel, memory, execution_depth }`, գումարած `custom: BTreeMap`:
- Տարբերությունների համար առանձին թվեր՝ `SumeragiParameter`, `BlockParameter`, `TransactionParameter`, `SmartContractParameter`: Ագրեգատոր՝ `Parameters`: Կոդ՝ `crates/iroha_data_model/src/parameter/system.rs`։

Պարամետրերի կարգավորում (ISI). `SetParameter(Parameter)`-ը թարմացնում է համապատասխան դաշտը և թողարկում `ConfigurationEvent::Changed`: Կոդ՝ `crates/iroha_data_model/src/isi/transparent.rs`, կատարող՝ `crates/iroha_core/src/smartcontracts/isi/world.rs`-ում։

---

## Հրահանգների սերիականացում և գրանցում
- Հիմնական հատկանիշ՝ `Instruction: Send + Sync + 'static`՝ `dyn_encode()`, `as_any()`, կայուն `id()` (կանխադրված է կոնկրետ տեսակի անվանման համար):
- `InstructionBox`: `Box<dyn Instruction>` փաթաթան: Clone/Eq/Ord-ը գործում է `(type_id, encoded_bytes)`-ի վրա, ուստի հավասարությունն ըստ արժեքի է:
- Norito սերդը `InstructionBox`-ի համար սերիականացվում է որպես `(String wire_id, Vec<u8> payload)` (իջնում ​​է մինչև `type_name`, եթե չկա մետաղալարային ID): Ապասերիալիզացիան օգտագործում է գլոբալ `InstructionRegistry` քարտեզագրման նույնացուցիչներ կոնստրուկտորների համար: Կանխադրված ռեեստրը ներառում է բոլոր ներկառուցված ISI-ները: Կոդ՝ `crates/iroha_data_model/src/isi/{mod.rs,registry.rs}`։

---

## ISI՝ տեսակներ, իմաստաբանություն, սխալներ
Կատարումն իրականացվում է `Execute for <Instruction>`-ի միջոցով `iroha_core::smartcontracts::isi`-ում: Ստորև թվարկված են հանրային ազդեցությունները, նախապայմանները, արտանետվող իրադարձությունները և սխալները:

### Գրանցվել / Չգրանցվել
Տեսակները՝ `Register<T: Registered>` և `Unregister<T: Identifiable>`, գումարային տեսակներով `RegisterBox`/`UnregisterBox` ծածկող կոնկրետ թիրախներ:- Գրանցվել Peer. ներդիրներ համաշխարհային հասակակիցների հավաքածուի մեջ:
  - Նախադրյալներ. չպետք է արդեն գոյություն ունենան:
  - Իրադարձություններ՝ `PeerEvent::Added`:
  - Սխալներ. `Repetition(Register, PeerId)`, եթե կրկնօրինակ է; `FindError` որոնումների վրա: Կոդ՝ `core/.../isi/world.rs`։

- Գրանցեք տիրույթ. կառուցվում է `NewDomain`-ից `owned_by = authority`-ով: Արգելված է՝ `genesis` տիրույթ:
  - Նախադրյալներ. տիրույթի բացակայություն; ոչ `genesis`.
  - Իրադարձություններ՝ `DomainEvent::Created`:
  - Սխալներ՝ `Repetition(Register, DomainId)`, `InvariantViolation("Not allowed to register genesis domain")`: Կոդ՝ `core/.../isi/world.rs`։

- Գրանցեք հաշիվ. կառուցումներ `NewAccount`-ից, արգելված է `genesis` տիրույթում; `genesis` հաշիվը չի կարող գրանցվել:
  - Նախադրյալներ. տիրույթը պետք է գոյություն ունենա; հաշվի բացակայություն; ոչ Ծննդոց տիրույթում:
  - Իրադարձություններ՝ `DomainEvent::Account(AccountEvent::Created)`:
  - Սխալներ՝ `Repetition(Register, AccountId)`, `InvariantViolation("Not allowed to register account in genesis domain")`: Կոդ՝ `core/.../isi/domain.rs`։

- Գրանցեք AssetDefinition. կառուցում է շինարարից; հավաքածուներ `owned_by = authority`:
  - Նախադրյալներ. սահմանման բացակայություն; տիրույթը գոյություն ունի; `name`-ը պահանջվում է, այն կտրելուց հետո չպետք է դատարկ լինի և չպետք է պարունակի `#`/`@`:
  - Իրադարձություններ՝ `DomainEvent::AssetDefinition(AssetDefinitionEvent::Created)`:
  - Սխալներ՝ `Repetition(Register, AssetDefinitionId)`: Կոդ՝ `core/.../isi/domain.rs`։

- Գրանցեք NFT. կառուցումներ շինարարից; հավաքածուներ `owned_by = authority`:
  - Նախադրյալներ. NFT-ի բացակայություն; տիրույթը գոյություն ունի.
  - Իրադարձություններ՝ `DomainEvent::Nft(NftEvent::Created)`:
  - Սխալներ՝ `Repetition(Register, NftId)`: Կոդ՝ `core/.../isi/nft.rs`։- Գրանցման դեր. կառուցումներ `NewRole { inner, grant_to }`-ից (առաջին սեփականատերը գրանցված է հաշվի դերերի քարտեզագրման միջոցով), պահում է `inner: Role`:
  - Նախադրյալներ. դերի բացակայություն:
  - Իրադարձություններ՝ `RoleEvent::Created`:
  - Սխալներ՝ `Repetition(Register, RoleId)`: Կոդ՝ `core/.../isi/world.rs`։

- Գրանցման ձգան. ձգանը պահում է համապատասխան ձգանման մեջ, որը սահմանված է ըստ ֆիլտրի տեսակի:
  - Նախապայմաններ. Եթե ֆիլտրը հնարավոր չէ հատել, `action.repeats` պետք է լինի `Exactly(1)` (հակառակ դեպքում `MathError::Overflow`): Նույնականացման կրկնօրինակներն արգելված են:
  - Իրադարձություններ՝ `TriggerEvent::Created(TriggerId)`:
  - Սխալներ՝ `Repetition(Register, TriggerId)`, `InvalidParameterError::SmartContract(..)` փոխակերպման/վավերացման ձախողումների դեպքում: Կոդ՝ `core/.../isi/triggers/mod.rs`։- Չգրանցել Peer/Domain/Account/AssetDefinition/NFT/Role/Trigger. հեռացնում է թիրախը. թողարկում է ջնջման իրադարձություններ: Լրացուցիչ կասկադային հեռացումներ.- Չգրանցել տիրույթը. հեռացնում է տիրույթի միավորը, գումարած դրա ընտրիչ/հաստատման քաղաքականության վիճակը. ջնջում է ակտիվների սահմանումները տիրույթում (և գաղտնի `zk_assets` կողային վիճակ, որը հիմնված է այդ սահմանումներով), այդ սահմանումների ակտիվները (և յուրաքանչյուր ակտիվի մետատվյալները), NFT-ները տիրույթում և հաշվի այլանունների կանխատեսումները, որոնք արմատավորված են հեռացված տիրույթում: Այն նաև կտրում է հաշվի/դերային շրջանակի թույլտվությունների մուտքերը, որոնք հղում են անում հեռացված տիրույթին կամ դրա հետ ջնջված ռեսուրսներին (տիրույթի թույլտվությունները, ակտիվների սահմանման/ակտիվների թույլտվությունները հեռացված սահմանումների համար և NFT թույլտվությունները հանված NFT ID-ների համար): Տիրույթի հեռացումը չի ջնջում կամ վերագրում գլոբալ `AccountId`-ը, նրա tx-sequence/UAID վիճակը, օտարերկրյա ակտիվի կամ NFT-ի սեփականությունը, գործարկիչի լիազորությունը կամ այլ արտաքին աուդիտի/կոնֆիգուրացիայի հղումներ, որոնք մատնանշում են գոյություն ունեցող հաշիվը: Պաշտպանական ռելսեր. մերժում է, երբ տիրույթում որևէ ակտիվի սահմանումը դեռևս հիշատակվում է ռեպո-համաձայնագրով, հաշվարկային գրացուցակով, հանրային գծի պարգևով/պահանջով, անցանց նպաստով/փոխանցումով, հաշվարկային ռեպո լռությամբ (`settlement.repo.eligible_collateral`, `settlement.repo.collateral_substitution_matrix`), կառավարում-կոնֆիգուրացիա/կոնֆիգուրացիա-վիրտուալ-պարգևատրում/ ակտիվների սահմանման հղումներ, Oracle-economics-ի կազմաձևված պարգև/slash/dispute-bond ակտիվի սահմանման հղումներ կամ Nexus վճար/ստաժավորման ակտիվների սահմանման հղումներ (`nexus.fees.fee_asset_id`, `nexus.staking.stake_asset_id`): Իրադարձություններ՝ `DomainEvent::Deleted`, գումարած յուրաքանչյուր տարրի ջնջման իրադարձություններ՝ հեռացված տիրույթի ռեսուրսի համարces. Սխալներ. `FindError::Domain`, եթե բացակայում է; `InvariantViolation` պահպանված ակտիվների սահմանման տեղեկանքի հակասությունների վերաբերյալ: Կոդ՝ `core/.../isi/world.rs`։- Չեղարկել հաշիվը. հեռացնում է հաշվի թույլտվությունները, դերերը, tx-sequence հաշվիչը, հաշվի պիտակի քարտեզագրումը և UAID-ի կապերը. ջնջում է հաշվին պատկանող ակտիվները (և յուրաքանչյուր ակտիվի մետատվյալները). ջնջում է հաշվին պատկանող NFT-ները. հեռացնում է գործարկիչները, որոնց հեղինակությունն այդ հաշիվն է. prunes account-/role-scoped թույլտվությունների մուտքերը, որոնք հղում են անում հեռացված հաշվին, հաշվի-/role-scoped NFT-target թույլտվությունները հեռացված պատկանող NFT ID-ների համար, և հաշվի-/role-scoped trigger-target թույլտվությունները հեռացված գործարկիչների համար: Պահապան ռելսեր. մերժում է, եթե հաշիվը դեռ տիրապետում է տիրույթի, ակտիվի սահմանում, SoraFS մատակարարի պարտադիր պարտավորեցում, ակտիվ քաղաքացիության գրանցում, հրապարակային խաղադրույքների/պարգևատրման վիճակ (ներառյալ պարգև-պահանջի բանալիները, որտեղ հաշիվը հայտնվում է որպես հայցվոր կամ պարգև-ակտիվների սեփականատեր), ակտիվ oracle-ի տրամադրող վիճակ (ներառյալ twitter-binhistry-ի տրամադրողը), գրառումներ կամ Oracle-economics կազմաձևված պարգև/հատված հաշվի հղումներ), ակտիվ Nexus վճար/ստինգային հաշվի հղումներ (`nexus.fees.fee_sink_account_id`, `nexus.staking.stake_escrow_account_id`, `nexus.staking.slash_sink_account_id`; վերլուծված է որպես կանոնականացված տիրույթում վերագրանցված և վերագրանցված տիրույթում փակված և փակված տիրույթում բառացի), ակտիվ ռեպո-համաձայնագրի վիճակ, ակտիվ հաշվարկային գրանցամատյան, ակտիվ անցանց թույլտվություն/փոխանցում կամ վճիռը չեղյալ համարելու վիճակ, ակտիվ ակտիվների սահմանումների համար պահուստային հաշվի կազմաձևման հղումներ (`settlement.offline.escrow_accounts`), ակտիվ կառավարման վիճակ (առաջարկ/փուլի հաստատում):als/locks/slashes/խորհուրդ/խորհրդարանի ցուցակներ, առաջարկների խորհրդարանի նկարներ, գործարկման ժամանակի թարմացում առաջարկողների գրառումներ, կառավարման կազմաձևված պահուստային/slash-receiver/viral-pool հաշվի հղումներ, կառավարում SoraFS հեռաչափություն ներկայացնող հղումներ SoraFS հեռաչափություն ներկայացնող հղումներ SoraFS SoraFS հեռաչափություն ներկայացնող հղումներ `gov.sorafs_telemetry.per_provider_submitters` կամ կառավարման կողմից կազմաձևված SoraFS մատակարարի-սեփականատիրոջ հղումներ `gov.sorafs_provider_owners`-ի միջոցով), կազմաձևված բովանդակությունը հրապարակում է հաշիվների թույլատրելի ցուցակի հղումները (`content.publish_allow_accounts`), ակտիվ սոցիալական պահուստային պահառուի ակտիվ բովանդակություն, ակտիվ բովանդակություն ուղարկողի կարգավիճակը, ակտիվ բովանդակության պահման կարգավիճակը lane-relay վթարային վավերացուցիչի անտեսման վիճակ, կամ ակտիվ SoraFS pin-registry թողարկողի/կապակցող գրառումներ (pin manifests, manifest aliases, replication orders): Իրադարձություններ՝ `AccountEvent::Deleted`, գումարած `NftEvent::Deleted` մեկ հեռացված NFT-ի համար: Սխալներ. `FindError::Account`, եթե բացակայում է; `InvariantViolation` սեփականության որբերի մասին: Կոդ՝ `core/.../isi/domain.rs`։- Չգրանցել AssetDefinition-ը. ջնջում է այդ սահմանման բոլոր ակտիվները և դրանց յուրաքանչյուր ակտիվի մետատվյալները, և հեռացնում է գաղտնի `zk_assets` կողային վիճակը, որը ստեղված է այդ սահմանմամբ. նաև կտրում է համապատասխան `settlement.offline.escrow_accounts` մուտքի և հաշվի/դերային շրջանակի թույլտվության գրառումները, որոնք հղում են անում հեռացված ակտիվի սահմանմանը կամ դրա ակտիվների օրինակներին: Պաշտպանական ռելսեր. մերժում է, երբ սահմանումը դեռևս հիշատակվում է ռեպո-համաձայնագրով, հաշվարկային գրացուցակով, հրապարակային պարգևով/պահանջով, անցանց նպաստով/փոխանցման վիճակով, հաշվարկային ռեպո լռությամբ (`settlement.repo.eligible_collateral`, `settlement.repo.collateral_substitution_matrix`), կառավարման կազմաձևված քվեարկությամբ/ligibility/citizeamental. ակտիվների սահմանման հղումներ, Oracle-economics-ի կազմաձևված պարգև/slash/dispute-bond ակտիվի սահմանման հղումներ կամ Nexus վճար/ստինգ ակտիվների սահմանման հղումներ (`nexus.fees.fee_asset_id`, `nexus.staking.stake_asset_id`): Իրադարձություններ՝ `AssetDefinitionEvent::Deleted` և `AssetEvent::Deleted` մեկ ակտիվի համար: Սխալներ՝ `FindError::AssetDefinition`, `InvariantViolation` հղումների կոնֆլիկտների դեպքում: Կոդ՝ `core/.../isi/domain.rs`։
  - Չեղարկել NFT-ի գրանցումը. հեռացնում է NFT-ն և կտրում է հաշվառման/դերային շրջանակի թույլտվության գրառումները, որոնք հղում են անում հեռացված NFT-ին: Իրադարձություններ՝ `NftEvent::Deleted`: Սխալներ՝ `FindError::Nft`: Կոդ՝ `core/.../isi/nft.rs`։
  - Չգրանցել դերը. նախ չեղյալ է հայտարարում դերը բոլոր հաշիվներից; ապա հեռացնում է դերը: Իրադարձություններ՝ `RoleEvent::Deleted`: Սխալներ՝ `FindError::Role`: Կոդ՝ `core/.../isi/world.rs`։- Unregister Trigger-ը. հեռացնում է գործարկիչը, եթե առկա է, և կտրում է հաշվի/դերային շրջանակի թույլտվության գրառումները, որոնք հղում են անում հեռացված գործարկիչին. կրկնօրինակ ապագրանցումը տալիս է `Repetition(Unregister, TriggerId)`: Իրադարձություններ՝ `TriggerEvent::Deleted`: Կոդ՝ `core/.../isi/triggers/mod.rs`։

### Անանուխ / Այրել
Տեսակները՝ `Mint<O, D: Identifiable>` և `Burn<O, D: Identifiable>`, տուփով որպես `MintBox`/`BurnBox`:

- Ակտիվ (Թվային) անանուխ/այրում. կարգավորում է մնացորդները և սահմանման `total_quantity`:
  - Նախապայմաններ. `Numeric` արժեքը պետք է բավարարի `AssetDefinition.spec()`; անանուխը թույլատրված է `mintable`-ով:
    - `Infinitely`. միշտ թույլատրվում է:
    - `Once`: թույլատրվում է ուղիղ մեկ անգամ; առաջին անանուխը փոխում է `mintable`-ը դեպի `Not` և արտանետում `AssetDefinitionEvent::MintabilityChanged`, գումարած մանրամասն `AssetDefinitionEvent::MintabilityChangedDetailed { asset_definition, minted_amount, authority }`՝ աուդիտի համար:
    - `Limited(n)`. թույլ է տալիս `n` լրացուցիչ անանուխի գործառնություններ: Յուրաքանչյուր հաջող անանուխ նվազեցնում է հաշվիչը; երբ այն հասնում է զրոյի, սահմանումը վերածվում է `Not`-ի և թողարկում է նույն `MintabilityChanged` իրադարձությունները, ինչպես վերևում:
    - `Not`՝ սխալ `MintabilityError::MintUnmintable`:
  - Պետական ​​փոփոխություններ. ստեղծում է ակտիվ, եթե բացակայում է դրամահատարանում; հեռացնում է ակտիվների մուտքագրումը, եթե այրման ժամանակ մնացորդը դառնում է զրո:
  - Իրադարձություններ՝ `AssetEvent::Added`/`AssetEvent::Removed`, `AssetDefinitionEvent::MintabilityChanged` (երբ `Once`-ը կամ `Limited(n)`-ը սպառում են դրա չափը):
  - Սխալներ՝ `TypeError::AssetNumericSpec(Mismatch)`, `MathError::Overflow`/`NotEnoughQuantity`: Կոդ՝ `core/.../isi/asset.rs`։- Ձգան կրկնողություններ անանուխ/այրվածք. `action.repeats` փոփոխությունները հաշվարկվում են ձգանի համար:
  - Նախապայմաններ. անանուխի վրա ֆիլտրը պետք է լինի հատվող; թվաբանությունը չպետք է հեղեղի/հեղեղի.
  - Իրադարձություններ՝ `TriggerEvent::Extended`/`TriggerEvent::Shortened`:
  - Սխալներ. `MathError::Overflow` անվավեր անանուխի վրա; `FindError::Trigger`, եթե բացակայում է: Կոդ՝ `core/.../isi/triggers/mod.rs`։

### Փոխանցում
Տեսակները՝ `Transfer<S: Identifiable, O, D: Identifiable>`, տուփով որպես `TransferBox`:

- Ակտիվ (Թվային). հանել `AssetId` աղբյուրից, ավելացնել նպատակակետին `AssetId` (նույն սահմանումը, տարբեր հաշիվ): Ջնջել զրոյացված աղբյուրի ակտիվը:
  - Նախադրյալներ. աղբյուր ակտիվը գոյություն ունի. արժեքը բավարարում է `spec`-ին:
  - Իրադարձություններ՝ `AssetEvent::Removed` (աղբյուր), `AssetEvent::Added` (նպատակակետ):
  - Սխալներ՝ `FindError::Asset`, `TypeError::AssetNumericSpec`, `MathError::NotEnoughQuantity/Overflow`: Կոդ՝ `core/.../isi/asset.rs`։

- Դոմենի սեփականություն. փոխում է `Domain.owned_by` դեպի նպատակակետ հաշիվ:
  - Նախապայմաններ. երկու հաշիվներն էլ կան. տիրույթը գոյություն ունի.
  - Իրադարձություններ՝ `DomainEvent::OwnerChanged`:
  - Սխալներ՝ `FindError::Account/Domain`: Կոդ՝ `core/.../isi/domain.rs`։

- AssetDefinition-ի սեփականությունը. փոխում է `AssetDefinition.owned_by`-ը նպատակակետ հաշվին:
  - Նախապայմաններ. երկու հաշիվներն էլ կան. սահմանումը գոյություն ունի; աղբյուրը ներկայումս պետք է տիրապետի դրան. հեղինակությունը պետք է լինի աղբյուրի հաշիվ, աղբյուր-տիրույթի սեփականատեր կամ ակտիվի սահմանման տիրույթի սեփականատեր:
  - Իրադարձություններ՝ `AssetDefinitionEvent::OwnerChanged`:
  - Սխալներ՝ `FindError::Account/AssetDefinition`: Կոդ՝ `core/.../isi/account.rs`։- NFT-ի սեփականություն. փոխում է `Nft.owned_by`-ը նպատակակետ հաշվին:
  - Նախապայմաններ. երկու հաշիվներն էլ կան. NFT գոյություն ունի; աղբյուրը ներկայումս պետք է տիրապետի դրան. հեղինակությունը պետք է լինի աղբյուրի հաշիվ, աղբյուր-տիրույթի սեփականատեր, NFT-տիրույթի սեփականատեր կամ պահի `CanTransferNft` այդ NFT-ի համար:
  - Իրադարձություններ՝ `NftEvent::OwnerChanged`:
  - Սխալներ. `FindError::Account/Nft`, `InvariantViolation`, եթե աղբյուրը չի պատկանում NFT-ին: Կոդ՝ `core/.../isi/nft.rs`։

### Մետատվյալներ. Սահմանել/հեռացնել բանալի-արժեքը
Տեսակները՝ `SetKeyValue<T>` և `RemoveKeyValue<T>` `T ∈ { Domain, Account, AssetDefinition, Nft, Trigger }`-ով: Տրվում են արկղային թվեր:

- Հավաքածու՝ տեղադրում կամ փոխարինում է `Metadata[key] = Json(value)`:
- Հեռացնել. հանում է բանալին; սխալ, եթե բացակայում է:
- Իրադարձություններ՝ `<Target>Event::MetadataInserted` / `MetadataRemoved` հին/նոր արժեքներով:
- Սխալներ. `FindError::<Target>`, եթե թիրախը գոյություն չունի; `FindError::MetadataKey` բացակայող բանալին հանելու համար: Կոդ՝ `crates/iroha_data_model/src/isi/transparent.rs` և կատարող ենթադրություններ յուրաքանչյուր թիրախի համար:

### Թույլտվություններ և դերեր՝ շնորհում/չեղարկում
Տեսակները՝ `Grant<O, D>` և `Revoke<O, D>`, `Permission`/`Role`-ից/`Account`-ից և `Account`-ից և `Account`-ից և Norito-ից տուփերով- Տրամադրել թույլտվություն հաշվին. ավելացնում է `Permission`, եթե արդեն իսկ բնորոշ չէ: Իրադարձություններ՝ `AccountEvent::PermissionAdded`: Սխալներ. `Repetition(Grant, Permission)`, եթե կրկնօրինակ է: Կոդ՝ `core/.../isi/account.rs`։
- Չեղարկել թույլտվությունը հաշվից. հանվում է, եթե առկա է: Իրադարձություններ՝ `AccountEvent::PermissionRemoved`: Սխալներ. `FindError::Permission`, եթե բացակայում է: Կոդ՝ `core/.../isi/account.rs`։
- Տրամադրել դերը հաշվին. բացակայելու դեպքում տեղադրում է `(account, role)` քարտեզագրում: Իրադարձություններ՝ `AccountEvent::RoleGranted`: Սխալներ՝ `Repetition(Grant, RoleId)`: Կոդ՝ `core/.../isi/account.rs`։
- Չեղյալ համարել դերը հաշվից. հեռացնում է քարտեզագրումը, եթե առկա է: Իրադարձություններ՝ `AccountEvent::RoleRevoked`: Սխալներ. `FindError::Role`, եթե բացակայում է: Կոդ՝ `core/.../isi/account.rs`։
- Տրամադրել դերի թույլտվություն. վերակառուցում է դերը՝ ավելացված թույլտվությամբ: Իրադարձություններ՝ `RoleEvent::PermissionAdded`: Սխալներ՝ `Repetition(Grant, Permission)`: Կոդ՝ `core/.../isi/world.rs`։
- Չեղարկել թույլտվությունը դերից. վերակառուցում է դերն առանց այդ թույլտվության: Իրադարձություններ՝ `RoleEvent::PermissionRemoved`: Սխալներ. `FindError::Permission`, եթե բացակայում է: Կոդ՝ `core/.../isi/world.rs`։### Գործարկիչներ. Կատարել
Տեսակը՝ `ExecuteTrigger { trigger: TriggerId, args: Json }`:
- Վարքագիծ. հերթագրում է `ExecuteTriggerEvent { trigger_id, authority, args }` ձգանման ենթահամակարգի համար: Ձեռքով կատարումը թույլատրվում է միայն ուղեկցող ազդակների համար (`ExecuteTrigger` զտիչ); զտիչը պետք է համընկնի, և զանգահարողը պետք է լինի ձգան գործողության հեղինակը կամ պահի `CanExecuteTrigger` այդ լիազորության համար: Երբ օգտագործողի կողմից տրամադրված կատարողը ակտիվ է, գործարկման գործարկումը վավերացվում է գործարկման ժամանակի կատարողի կողմից և սպառում է գործարքի կատարողի վառելիքի բյուջեն (բազային `executor.fuel`, գումարած կամընտիր մետատվյալներ `additional_fuel`):
- Սխալներ. `FindError::Trigger`, եթե գրանցված չէ; `InvariantViolation`, եթե զանգել է ոչ լիազորված անձի կողմից: Կոդ՝ `core/.../isi/triggers/mod.rs` (և փորձարկումներ `core/.../smartcontracts/isi/mod.rs`-ում):

### Թարմացում և գրանցում
- `Upgrade { executor }`. տեղափոխում է կատարողը` օգտագործելով տրամադրված `Executor` բայթ կոդը, թարմացնում է կատարողը և դրա տվյալների մոդելը, արտանետում է `ExecutorEvent::Upgraded`: Սխալներ. միգրացիայի ձախողման դեպքում փաթաթված է որպես `InvalidParameterError::SmartContract`: Կոդ՝ `core/.../isi/world.rs`։
- `Log { level, msg }`. թողարկում է տվյալ մակարդակով հանգույցի մատյան; պետական ​​փոփոխություններ չկան. Կոդ՝ `core/.../isi/world.rs`։

### Սխալի մոդել
Ընդհանուր ծրար՝ `InstructionExecutionError` տարբերակներով՝ գնահատման սխալների, հարցումների ձախողումների, փոխակերպումների, օբյեկտի չգտնվելու, կրկնության, հատման հնարավորության, մաթեմատիկայի, անվավեր պարամետրի և անփոփոխ խախտման համար: Թվարկումներն ու օգնականները գտնվում են `crates/iroha_data_model/src/isi/mod.rs`-ում՝ `pub mod error`-ում:

---## Գործարքներ և գործադիրներ
- `Executable`՝ կամ `Instructions(ConstVec<InstructionBox>)` կամ `Ivm(IvmBytecode)`; բայթկոդը սերիականացվում է որպես base64: Կոդ՝ `crates/iroha_data_model/src/transaction/executable.rs`։
- `TransactionBuilder`/`SignedTransaction`. կառուցում, ստորագրում և փաթեթավորում է գործարկվող մետատվյալներով, `chain_id`, `authority`, `creation_time_ms`, կամընտիր I08, կամընտիր I08, `chain_id`, `authority`, Norito, և ընտրովի I08: `nonce`. Կոդ՝ `crates/iroha_data_model/src/transaction/`։
- Գործարկման ժամանակ `iroha_core`-ը կատարում է `InstructionBox` խմբաքանակներ `Execute for InstructionBox`-ի միջոցով՝ իջեցնելով համապատասխան `*Box` կամ կոնկրետ հրահանգին: Կոդ՝ `crates/iroha_core/src/smartcontracts/isi/mod.rs`։
- Գործադիր կատարողի վավերացման բյուջե (օգտագործողի կողմից տրամադրված կատարող). բազային `executor.fuel` պարամետրերից գումարած կամընտիր գործարքի մետատվյալներ `additional_fuel` (`u64`), որոնք համօգտագործվում են գործարքի ընթացքում հրահանգների/գործարկման վավերացումների միջև:

---## Ինվարիանտներ և նշումներ (թեստերից և պահակներից)
- Genesis պաշտպանություն. չի կարող գրանցվել `genesis` տիրույթը կամ հաշիվները `genesis` տիրույթում; `genesis` հաշիվը չի կարող գրանցվել: Կոդ/թեստեր՝ `core/.../isi/world.rs`, `core/.../smartcontracts/isi/mod.rs`:
- Թվային ակտիվները պետք է բավարարեն իրենց `NumericSpec`-ը անանուխի/փոխանցման/այրման վրա. բնութագրերի անհամապատասխանությունը տալիս է `TypeError::AssetNumericSpec`:
- Դրամահատելիություն. `Limited(n)`-ը թույլ է տալիս ճիշտ `n` հատել նախքան `Not`-ին անցնելը: `Infinitely`-ի վրա հատումն արգելելու փորձերը հանգեցնում են `MintabilityError::ForbidMintOnMintable`-ի, իսկ `Limited(0)`-ի կարգավորումը՝ բերում է `MintabilityError::InvalidMintabilityTokens`:
- Մետատվյալների գործողությունները ստույգ են. գոյություն չունեցող բանալի հեռացնելը սխալ է:
- ձգանման ֆիլտրերը կարող են լինել չմշակվող; ապա `Register<Trigger>` թույլ է տալիս միայն `Exactly(1)` կրկնել:
- Ձգան մետատվյալների բանալին `__enabled` (bool) դարպասների կատարում; բացակայում են կանխադրվածները միացված են, իսկ անջատված գործարկիչները բաց են թողնվում տվյալների/ժամանակի/զանգերի միջոցով:
- Դետերմինիզմ. բոլոր թվաբանությունն օգտագործում է ստուգված գործողություններ; under/overflow վերադարձնում է մուտքագրված մաթեմատիկական սխալները; զրոյական մնացորդները նվազեցնում են ակտիվների մուտքերը (թաքնված վիճակ չկա):

---## Գործնական օրինակներ
- հատում և փոխանցում.
  - `Mint::asset_numeric(10, asset_id)` → ավելացնում է 10, եթե դա թույլատրվում է սպեցիֆիկացիաներով/հատուկ հնարավորություններով; իրադարձություններ՝ `AssetEvent::Added`:
  - `Transfer::asset_numeric(asset_id, 5, to_account)` → շարժվում է 5; միջոցառումներ հեռացման/ավելացման համար:
- Մետատվյալների թարմացումներ.
  - `SetKeyValue::account(account_id, "avatar".parse()?, json)` → վերև; հեռացում `RemoveKeyValue::account(...)`-ի միջոցով:
- Դերի/թույլտվությունների կառավարում.
  - `Grant::account_role(role_id, account)`, `Grant::role_permission(perm, role)` և նրանց `Revoke` գործընկերները:
- Գործարկիչի կյանքի ցիկլը.
  - `Register::trigger(Trigger::new(id, Action::new(exec, repeats, authority, filter)))`՝ ֆիլտրով ենթադրվող հատման ստուգմամբ; `ExecuteTrigger::new(id).with_args(&args)`-ը պետք է համապատասխանի կազմաձևված հեղինակությանը:
  - Գործարկիչները կարող են անջատվել՝ մետատվյալների բանալին `__enabled` դնելով `false`-ին (կանխադրվածները բացակայում են՝ միացված են); փոխարկեք `SetKeyValue::trigger` կամ IVM `set_trigger_enabled` syscall-ի միջոցով:
  - Գործարկիչի պահեստավորումը վերանորոգվում է բեռնվածության դեպքում. կրկնօրինակ ID-ները, անհամապատասխան ID-ները և բացակայող բայթկոդի հղումով գործարկիչները հեռացվում են. բայթկոդի տեղեկանքների քանակը վերահաշվարկվում է:
  - Եթե գործարկման ժամանակ բացակայում է գործարկողի IVM բայթկոդը, գործարկիչը հանվում է, և կատարումը դիտվում է որպես անգործունակ՝ ձախողման հետևանքով:
  - սպառված ձգանները անմիջապես հեռացվում են. եթե կատարման ընթացքում նկատվում է սպառված մուտք, այն կտրվում և համարվում է բացակայող:
- Պարամետրի թարմացում.
  - `SetParameter(SumeragiParameter::BlockTimeMs(2500).into())`-ը թարմացնում և թողարկում է `ConfigurationEvent::Changed`:CLI / Torii ակտիվի սահմանման id + կեղծանունների օրինակներ.
- Գրանցվեք կանոնական Base58 ID-ով + բացահայտ անունով + երկար կեղծանունով.
  - `iroha ledger asset definition register --id 66owaQmAQMuHxPzxUN3bqZ6FJfDa --name pkr --alias pkr#bankb.sbp`
- Գրանցվեք կանոնական Base58 id-ով + բացահայտ անունով + կարճ կեղծանունով.
  - `iroha ledger asset definition register --id 66owaQmAQMuHxPzxUN3bqZ6FJfDa --name pkr --alias pkr#sbp`
- Անանուխը կեղծանունով + հաշվի բաղադրիչներ.
  - `iroha ledger asset mint --definition-alias pkr#bankb.sbp --account <i105> --quantity 500`
- Լուծել alias-ը կանոնական Base58 id:
  - `POST /v1/assets/aliases/resolve` JSON `{ "alias": "pkr#bankb.sbp" }`-ով

Միգրացիոն նշում.
- `name#domain` տեքստային ակտիվների սահմանման ID-ները միտումնավոր չեն աջակցվում առաջին թողարկումում. օգտագործեք կանոնական Base58 ID-ներ կամ լուծեք կետավոր կեղծանունը:
- Հանրային ակտիվների ընտրիչներն օգտագործում են Base58 ակտիվների սահմանման կանոնական ID-ներ, գումարած սեփականության բաժանված դաշտերը (`account`, կամընտիր `scope`): Հում կոդավորված `AssetId` բառացիները մնում են ներքին օգնականներ և չեն հանդիսանում Torii/CLI ընտրիչի մակերեսի մաս:
- Ակտիվների սահմանման ցուցակը/հարցման զտիչները և տեսակները լրացուցիչ ընդունում են `alias_binding.status`, `alias_binding.lease_expiry_ms`, `alias_binding.grace_until_ms` և `alias_binding.bound_at_ms`:

---

## Հետագծելիություն (ընտրված աղբյուրներ)
 - Տվյալների մոդելի միջուկը՝ `crates/iroha_data_model/src/{account.rs,domain.rs,asset/**,nft.rs,role.rs,permission.rs,metadata.rs,trigger/**,parameter/**}`:
 - ISI սահմանումներ և գրանցամատյան՝ `crates/iroha_data_model/src/isi/{mod.rs,register.rs,transfer.rs,mint_burn.rs,transparent.rs,registry.rs}`:
 - ISI-ի կատարում՝ `crates/iroha_core/src/smartcontracts/isi/{mod.rs,world.rs,domain.rs,account.rs,asset.rs,nft.rs,triggers/**}`:
 - Իրադարձություններ՝ `crates/iroha_data_model/src/events/**`:
 - Գործարքներ՝ `crates/iroha_data_model/src/transaction/**`:

Եթե ​​ցանկանում եք, որ այս հատկանիշն ընդլայնվի վերածված API-ի/վարքագծի աղյուսակի մեջ կամ փոխկապակցված լինի յուրաքանչյուր կոնկրետ իրադարձության/սխալի հետ, ասեք բառը և ես կընդլայնեմ այն: