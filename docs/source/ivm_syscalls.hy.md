---
lang: hy
direction: ltr
source: docs/source/ivm_syscalls.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: bcf280df1e00065199d386e07b9fd67d8f94c4046d73cfa3b63d1eec18228cd8
source_last_modified: "2026-01-22T16:26:46.570453+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# IVM Syscall ABI

Այս փաստաթուղթը սահմանում է IVM syscall համարները, ցուցիչ-ABI կանչերի պայմանագրերը, վերապահված թվերի միջակայքերը և պայմանագրային երեսպատման syscals-ի կանոնական աղյուսակը, որն օգտագործվում է Kotodama իջեցման միջոցով: Այն լրացնում է `ivm.md` (ճարտարապետություն) և `kotodama_grammar.md` (լեզու):

Տարբերակում
- Ճանաչված syscalls-ի հավաքածուն կախված է բայթկոդի վերնագրի `abi_version` դաշտից: Առաջին թողարկումն ընդունում է միայն `abi_version = 1`; այլ արժեքները մերժվում են ընդունելության ժամանակ: Անհայտ թվերը ակտիվ `abi_version`-ի համար դետերմինիստականորեն թակարդում են `E_SCALL_UNKNOWN`-ով:
- Runtime-ի թարմացումները պահպանում են `abi_version = 1`-ը և չեն ընդլայնում syscall կամ ցուցիչ-ABI մակերեսները:
- Syscall գազի ծախսերը հանդիսանում են գազի տարբերակված ժամանակացույցի մի մասը, որը կապված է բայթկոդի վերնագրի տարբերակին: Տես `ivm.md` (Գազային քաղաքականություն):

Համարակալման միջակայքերը
- `0x00..=0x1F`. VM միջուկ/կոմունալ (վրիպազերծման/ելքի օգնականները հասանելի են `CoreHost`-ի ներքո, մշակողի մնացած օգնականները միայն կեղծ հոսթ են):
- `0x20..=0x5F`: Iroha միջուկային ISI կամուրջ (կայուն ABI v1-ում):
- `0x60..=0x7F`. ընդլայնման ISI-ները փակված են պրոտոկոլային հատկանիշներով (միացված լինելու դեպքում դեռևս մաս է կազմում ABI v1):
- `0x80..=0xFF`՝ հյուրընկալող/կրիպտո օգնականներ և վերապահված սլոտներ; Ընդունվում են միայն ABI v1 թույլտվությունների ցանկում առկա համարները:

Երկարակյաց օգնականներ (ABI v1)
- Երկարատև վիճակի օգնական համակարգերը (0x50–0x5A: STATE_{GET,SET,DEL}, ENCODE/DECODE_INT, BUILD_PATH_*, JSON/SCHEMA կոդավորում/վերծանում) V1 ABI-ի մի մասն են և ներառված են `abi_hash` հաշվարկում:
- CoreHost լարերը STATE_{GET,SET,DEL} WSV-ով ապահովված կայուն խելացի պայմանագրային վիճակի; dev/test hosts-ը կարող է պահպանվել տեղում, բայց պետք է պահպանի նույնական syscall իմաստաբանությունը:

Pointer-ABI զանգերի կոնվենցիա (խելացի պայմանագրային համակարգեր)
- Փաստարկները տեղադրվում են `r10+` գրանցամատյաններում որպես `u64` հում արժեքներ կամ որպես ցուցիչներ INPUT տարածաշրջանում դեպի անփոփոխ Norito TLV ծրարներ (օրինակ՝ Norito `Name`, `Json`, `NftId`):
- Scalar վերադարձի արժեքներն են `u64`-ը, որը վերադարձվել է հոսթից: Ցուցիչի արդյունքները գրվում են հյուրընկալողի կողմից `r10`-ում:

Կանոնական syscall աղյուսակ (ենթաբազմություն)| Hex | Անունը | Փաստարկներ (`r10+`-ում) | Վերադարձներ | Գազ (հիմք + փոփոխական) | Ծանոթագրություններ |
|------|-------------------------------------------------------------------------- ------------------------|
| 0x1A | SET_ACCOUNT_DETAIL | `&AccountId`, `&Name`, `&Json` | `u64=0` | `G_set_detail + bytes(val)` | Հաշվի համար մանրամասն գրում է |
| 0x22 | MINT_ASSET | `&AccountId`, `&AssetDefinitionId`, `&NoritoBytes(Numeric)` | `u64=0` | `G_mint` | Դրամահատարաններ `amount` ակտիվների հաշվին |
| 0x23 | ԱՅՐՎԱԾ_ԱԿՏԻՎ | `&AccountId`, `&AssetDefinitionId`, `&NoritoBytes(Numeric)` | `u64=0` | `G_burn` | Այրվում է `amount` հաշվից |
| 0x24 | ՏՐԱՆՍՖԵՐ_ԱԿՏԻՎ | `&AccountId(from)`, `&AccountId(to)`, `&AssetDefinitionId`, `&NoritoBytes(Numeric)` | `u64=0` | `G_transfer` | `amount` փոխանցումներ հաշիվների միջև |
| 0x29 | TRANSFER_V1_BATCH_BEGIN | – | `u64=0` | `G_transfer` | Սկսեք FASTPQ փոխանցման խմբաքանակի շրջանակը |
| 0x2A | TRANSFER_V1_BATCH_END | – | `u64=0` | `G_transfer` | Լվացեք կուտակված FASTPQ փոխանցման խմբաքանակ |
| 0x2B | TRANSFER_V1_BATCH_APPLY | `r10=&NoritoBytes(TransferAssetBatch)` | `u64=0` | `G_transfer` | Կիրառել Norito կոդավորված փաթեթը մեկ syscall-ում |
| 0x25 | NFT_MINT_ASSET | `&NftId`, `&AccountId(owner)` | `u64=0` | `G_nft_mint_asset` | Գրանցում է նոր NFT |
| 0x26 | NFT_TRANSFER_ASSET | `&AccountId(from)`, `&NftId`, `&AccountId(to)` | `u64=0` | `G_nft_transfer_asset` | Փոխանցում է NFT-ի սեփականությունը |
| 0x27 | NFT_SET_METADATA | `&NftId`, `&Json` | `u64=0` | `G_nft_set_metadata` | Թարմացնում է NFT մետատվյալները |
| 0x28 | NFT_BURN_ASSET | `&NftId` | `u64=0` | `G_nft_burn_asset` | Այրում (ոչնչացնում է) NFT |
| 0xA1 | SMARTCONTRACT_EXECUTE_QUERY| `r10=&NoritoBytes(QueryRequest)` | `r10=ptr (&NoritoBytes(QueryResponse))` | `G_scq + per_item*items + per_byte*bytes(resp)` | Կրկնվող հարցումները կատարվում են ժամանակավոր. `QueryRequest::Continue` մերժված |
| 0xA2 | CREATE_NFTS_FOR_ALL_USERS | – | `u64=count` | `G_create_nfts_for_all` | Օգնական; հատկանշական դարպասներով || 0xA3 | SET_SMARTCONTRACT_EXECUTION_DEPTH | `depth:u64` | `u64=prev` | `G_set_depth` | Ադմին; հատկանշական դարպասներով |
| 0xA4 | GET_AUTHORITY | – (հաղորդավարը գրում է արդյունքը) | `&AccountId`| `G_get_auth` | Հյուրընկալողը ցուցիչ է գրում ընթացիկ հեղինակությանը `r10` |
| 0xF7 | GET_MERKLE_PATH | `addr:u64`, `out_ptr:u64`, կամընտիր `root_out:u64` | `u64=len` | `G_mpath + len` | Գրում է ուղի (տերև→ արմատ) և կամընտիր արմատային բայթեր |
| 0xFA | GET_MERKLE_COMPACT | `addr:u64`, `out_ptr:u64`, կամընտիր `depth_cap:u64`, կամընտիր `root_out:u64` | `u64=depth` | `G_mpath + depth` | `[u8 depth][u32 dirs_le][u32 count][count*32 siblings]` |
| 0xFF | GET_REGISTER_MERKLE_COMPACT| `reg_index:u64`, `out_ptr:u64`, կամընտիր `depth_cap:u64`, կամընտիր `root_out:u64` | `u64=depth` | `G_mpath + depth` | Նույն կոմպակտ դասավորությունը ռեգիստրի պարտավորության համար |

Գազի կիրառում
- CoreHost-ը լրացուցիչ գազ է գանձում ISI համակարգերի համար՝ օգտագործելով հայրենի ISI ժամանակացույցը; FASTPQ խմբաքանակի փոխանցումները գանձվում են յուրաքանչյուր մուտքի համար:
- ZK_VERIFY syscalls-ը նորից օգտագործում է գազի ստուգման գաղտնի ժամանակացույցը (բազային + ապացույցի չափս):
- SMARTCONTRACT_EXECUTE_QUERY գանձումների բազան + յուրաքանչյուր ապրանքի + մեկ բայթի համար; տեսակավորումը բազմապատկում է յուրաքանչյուր ապրանքի արժեքը, իսկ չտեսակավորված հաշվանցումները ավելացնում են մեկ ապրանքի համար տուգանք:

Նշումներ
- Բոլոր ցուցիչի արգումենտները հղում են կատարում Norito TLV ծրարներին INPUT տարածաշրջանում և վավերացված են առաջին ապահղման դեպքում (`E_NORITO_INVALID` սխալի դեպքում):
- Բոլոր մուտացիաները կիրառվում են Iroha-ի ստանդարտ կատարողի միջոցով (`CoreHost`-ի միջոցով), ոչ թե ուղղակիորեն VM-ի կողմից:
- Գազի ճշգրիտ հաստատունները (`G_*`) սահմանվում են ակտիվ գազի գրաֆիկով. տես `ivm.md`:

Սխալներ
- `E_SCALL_UNKNOWN`. syscall համարը չի ճանաչվում ակտիվ `abi_version`-ի համար:
- Մուտքի վավերացման սխալները տարածվում են որպես VM թակարդներ (օրինակ՝ `E_NORITO_INVALID` սխալ ձևավորված TLV-ների համար):

Խաչաձև հղումներ
- Ճարտարապետություն և VM իմաստաբանություն՝ `ivm.md`
- Լեզուն և ներկառուցված քարտեզագրում՝ `docs/source/kotodama_grammar.md`

Սերնդի նշում
- Syscall հաստատունների ամբողջական ցանկը կարող է ստեղծվել աղբյուրից՝
  - `make docs-syscalls` → գրում է `docs/source/ivm_syscalls_generated.md`
  - `make check-docs` → հաստատում է, որ ստեղծված աղյուսակը արդիական է (օգտակար CI-ում)
- Վերոնշյալ ենթաբազմությունը մնում է ընտրված, կայուն աղյուսակ պայմանագրային երևացող syscals-ների համար:

## Ադմինիստրատոր/Դեր TLV Օրինակներ (հեղինակային հաղորդավար)

Այս բաժինը ներկայացնում է TLV ձևերը և նվազագույն JSON ծանրաբեռնվածությունը, որոնք ընդունվել են կեղծ WSV հոսթի կողմից թեստերում օգտագործվող ադմինիստրատորի ոճի համակարգերի համար: Բոլոր ցուցիչի արգումենտները հետևում են ցուցիչ-ABI-ին (Norito TLV ծրարները տեղադրված են INPUT-ում): Արտադրության կազմակերպիչները կարող են օգտագործել ավելի հարուստ սխեմաներ. այս օրինակները նպատակ ունեն պարզաբանել տեսակներն ու հիմնական ձևերը:- REGISTER_PEER / UNREGISTER_PEER
  - Արգս՝ `r10=&Json`
  - Օրինակ JSON՝ `{ "peer": "peer-id-or-info" }`
  - CoreHost նշում. `REGISTER_PEER`-ն ակնկալում է `RegisterPeerWithPop` JSON օբյեկտ՝ `peer` + `pop` բայթերով (ըստ ցանկության `activation_at`, I1818NI00001); `UNREGISTER_PEER` ընդունում է peer-id տողը կամ `{ "peer": "..." }`:

- CREATE_TRIGGER / REMOVE_TRIGGER / SET_TRIGGER_ENABLED
  - CREATE_TRIGGER:
    - Արգս՝ `r10=&Json`
    - Նվազագույն JSON՝ `{ "name": "t1" }` (հավելյալ դաշտերը անտեսված են կեղծիքի կողմից)
  - REMOVE_TRIGGER:
    - Արգեր՝ `r10=&Name` (գործանի անվանումը)
  - SET_TRIGGER_ENABLED՝
    - Արգեր՝ `r10=&Name`, `r11=enabled:u64` (0 = անջատված է, ոչ զրոյական = միացված է)
  - CoreHost նշում. `CREATE_TRIGGER`-ն ակնկալում է գործարկման ամբողջական հատկանիշ (base64 Norito `Trigger` տող կամ
    `{ "id": "<trigger_id>", "action": ... }` `action`-ով որպես հիմք64 Norito `Action` տողով կամ
    JSON օբյեկտ), և `SET_TRIGGER_ENABLED`-ը փոխում է գործարկման մետատվյալների բանալին `__enabled` (բացակայում է
    լռելյայն միացված է):

- Դերեր՝ CREATE_ROLE / DELETE_ROLE / GRANT_ROLE / REVOKE_ROLE
  - CREATE_ROLE:
    - Արգեր՝ `r10=&Name` (դերերի անվանում), `r11=&Json` (թույլտվությունները սահմանված են)
    - JSON-ն ընդունում է `"perms"` կամ `"permissions"` ստեղնը, որոնցից յուրաքանչյուրը թույլտվությունների անունների տողային զանգված է:
    - Օրինակներ.
      - `{ "perms": [ "mint_asset:rose#wonder" ] }`
      - `{ "permissions": [ "read_assets:i105...", "transfer_asset:rose#wonder" ] }`
    - Աջակցված թույլտվության անվան նախածանցները ծաղրում.
      - `register_domain`, `register_account`, `register_asset_definition`
      - `read_assets:<account_id>`
      - `mint_asset:<asset_definition_id>`
      - `burn_asset:<asset_definition_id>`
      - `transfer_asset:<asset_definition_id>`
  - DELETE_ROLE:
    - Արգս՝ `r10=&Name`
    - Չհաջողվեց, եթե որևէ հաշվի այս դերը դեռ վերապահված է:
  - GRANT_ROLE / REVOKE_ROLE:
    - Արգեր՝ `r10=&AccountId` (առարկա), `r11=&Name` (դերային անվանում)
  - CoreHost նշում. JSON-ի թույլտվությունը կարող է լինել ամբողջական `Permission` օբյեկտ (`{ "name": "...", "payload": ... }`) կամ տող (լռելյայն լռելյայն է `null`); `GRANT_PERMISSION`/`REVOKE_PERMISSION` ընդունում `&Name` կամ `&Json(Permission)`:

- Չգրանցել գործառնությունները (տիրույթ/հաշիվ/ակտիվ). անփոփոխ (հեղինակային)
  - UNREGISTER_DOMAIN (`r10=&DomainId`) ձախողվում է, եթե տիրույթում գոյություն ունեն հաշիվներ կամ ակտիվների սահմանումներ:
  - UNREGISTER_ACCOUNT (`r10=&AccountId`) ձախողվում է, եթե հաշիվն ունի ոչ զրոյական մնացորդներ կամ ունի NFT-ներ:
  - UNREGISTER_ASSET (`r10=&AssetDefinitionId`) ձախողվում է, եթե ակտիվի համար մնացորդներ կան:

Նշումներ
- Այս օրինակները արտացոլում են թեստերում օգտագործվող WSV-ի կեղծ հոսթինգը. իրական հանգույցի հոսթերները կարող են բացահայտել ավելի հարուստ ադմինիստրատորի սխեմաներ կամ պահանջել լրացուցիչ վավերացում: Ցուցիչ-ABI-ի կանոնները դեռ գործում են. TLV-ները պետք է լինեն INPUT-ում, տարբերակ=1, տիպի ID-ները պետք է համընկնեն, իսկ օգտակար բեռնվածքի հեշերը պետք է վավերացվեն: