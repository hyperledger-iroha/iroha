---
lang: ka
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

ეს დოკუმენტი განსაზღვრავს IVM syscall ნომრებს, pointer-ABI გამოძახების კონვენციებს, რეზერვირებული ნომრების დიაპაზონს და Kotodama შემცირებით გამოყენებული კონტრაქტის წინაშე მყოფი სისტემების კანონიკურ ცხრილს. ის ავსებს `ivm.md` (არქიტექტურა) და `kotodama_grammar.md` (ენა).

ვერსიები
- აღიარებული syscalls-ის ნაკრები დამოკიდებულია ბაიტეკოდის სათაურზე `abi_version` ველზე. პირველი გამოშვება იღებს მხოლოდ `abi_version = 1`; სხვა ღირებულებები უარყოფილია მიღებისას. უცნობი ნომრები აქტიური `abi_version`-ისთვის დეტერმინისტულად იჭერს `E_SCALL_UNKNOWN`-ს.
- Runtime-ის განახლებები ინარჩუნებს `abi_version = 1`-ს და არ აფართოებს syscall-ის ან მაჩვენებლის-ABI ზედაპირებს.
- Syscall გაზის ხარჯები არის გაზის ვერსიული განრიგის ნაწილი, რომელიც დაკავშირებულია ბაიტეკოდის სათაურის ვერსიასთან. იხილეთ `ivm.md` (გაზის პოლიტიკა).

ნუმერაციის დიაპაზონები
- `0x00..=0x1F`: VM ბირთვი/უტილიტა (გამართვის/გასვლის დამხმარეები ხელმისაწვდომია `CoreHost`-ის ქვეშ; დანარჩენი დეველოპერის დამხმარეები მხოლოდ იმიტირებული მასპინძელია).
- `0x20..=0x5F`: Iroha ძირითადი ISI ხიდი (სტაბილურია ABI v1-ში).
- `0x60..=0x7F`: გაფართოების ISI-ები, რომლებიც შემოიფარგლება პროტოკოლის მახასიათებლებით (ჩართულია ისევ ABI v1-ის ნაწილი).
- `0x80..=0xFF`: მასპინძელი/კრიპტო დამხმარეები და დაჯავშნილი სლოტები; მიიღება მხოლოდ ABI v1 დაშვებულ სიაში არსებული ნომრები.

გამძლე დამხმარეები (ABI v1)
- მდგრადი მდგომარეობის დამხმარე სისტემები (0x50–0x5A: STATE_{GET,SET,DEL}, ENCODE/DECODE_INT, BUILD_PATH_*, JSON/SCHEMA კოდირება/გაშიფვრა) არის V1 ABI-ს ნაწილი და შედის `abi_hash` გამოთვლაში.
- CoreHost მავთულები STATE_{GET,SET,DEL} WSV-ის მხარდაჭერით გრძელვადიანი ჭკვიანი კონტრაქტის მდგომარეობამდე; dev/test hosts შეიძლება შენარჩუნდეს ადგილობრივად, მაგრამ უნდა შეინარჩუნოს იდენტური syscall სემანტიკა.

Pointer-ABI გამოძახების კონვენცია (ჭკვიანი კონტრაქტის სისტემები)
- არგუმენტები მოთავსებულია `r10+` რეგისტრებში, როგორც ნედლი `u64` მნიშვნელობები ან როგორც მაჩვენებლები INPUT რეგიონში შეუცვლელი Norito TLV კონვერტებისთვის (მაგ., Norito `Name`, `Json`, `NftId`).
- სკალარული დაბრუნების მნიშვნელობები არის ჰოსტიდან დაბრუნებული `u64`. პოინტერის შედეგები იწერება ჰოსტის მიერ `r10`-ში.

კანონიკური syscall ცხრილი (ქვეკომპლექტი)| Hex | სახელი | არგუმენტები (`r10+`-ში) | დაბრუნება | გაზი (ბაზა + ცვლადი) | შენიშვნები |
|------|------------------------------------------------------------------------ ------------------------------------------|----------------------------|-------|
| 0x1A | SET_ACCOUNT_DETAIL | `&AccountId`, `&Name`, `&Json` | `u64=0` | `G_set_detail + bytes(val)` | წერს დეტალს ანგარიშისთვის |
| 0x22 | MINT_ASSET | `&AccountId`, `&AssetDefinitionId`, `&NoritoBytes(Numeric)` | `u64=0` | `G_mint` | ზარაფხანა `amount` აქტივის ანგარიშზე |
| 0x23 | BURN_ASSET | `&AccountId`, `&AssetDefinitionId`, `&NoritoBytes(Numeric)` | `u64=0` | `G_burn` | წვავს `amount` ანგარიშიდან |
| 0x24 | TRANSFER_ASSET | `&AccountId(from)`, `&AccountId(to)`, `&AssetDefinitionId`, `&NoritoBytes(Numeric)` | `u64=0` | `G_transfer` | გადარიცხვები `amount` ანგარიშებს შორის |
| 0x29 | TRANSFER_V1_BATCH_BEGIN | – | `u64=0` | `G_transfer` | დაიწყეთ FASTPQ გადაცემის სერიის ფარგლები |
| 0x2A | TRANSFER_V1_BATCH_END | – | `u64=0` | `G_transfer` | გარეცხეთ დაგროვილი FASTPQ გადაცემის პარტია |
| 0x2B | TRANSFER_V1_BATCH_APPLY | `r10=&NoritoBytes(TransferAssetBatch)` | `u64=0` | `G_transfer` | გამოიყენეთ Norito-ში კოდირებული პარტია ერთ syscall-ში |
| 0x25 | NFT_MINT_ASSET | `&NftId`, `&AccountId(owner)` | `u64=0` | `G_nft_mint_asset` | რეგისტრირებს ახალ NFT |
| 0x26 | NFT_TRANSFER_ASSET | `&AccountId(from)`, `&NftId`, `&AccountId(to)` | `u64=0` | `G_nft_transfer_asset` | გადასცემს NFT |
| 0x27 | NFT_SET_METADATA | `&NftId`, `&Json` | `u64=0` | `G_nft_set_metadata` | განაახლებს NFT მეტამონაცემებს |
| 0x28 | NFT_BURN_ASSET | `&NftId` | `u64=0` | `G_nft_burn_asset` | წვავს (ანადგურებს) NFT-ს |
| 0xA1 | SMARTCONTRACT_EXECUTE_QUERY| `r10=&NoritoBytes(QueryRequest)` | `r10=ptr (&NoritoBytes(QueryResponse))` | `G_scq + per_item*items + per_byte*bytes(resp)` | iterable queries გადის ეფემერულად; `QueryRequest::Continue` უარყოფილია |
| 0xA2 | CREATE_NFTS_FOR_ALL_USERS | – | `u64=count` | `G_create_nfts_for_all` | დამხმარე; მხატვრული კარიბჭე || 0xA3 | SET_SMARTCONTRACT_EXECUTION_DEPTH | `depth:u64` | `u64=prev` | `G_set_depth` | ადმინი; მხატვრული კარიბჭე |
| 0xA4 | GET_AUTHORITY | – (მასპინძელი წერს შედეგს) | `&AccountId`| `G_get_auth` | მასპინძელი წერს მაჩვენებელს მიმდინარე ავტორიტეტზე `r10` |
| 0xF7 | GET_MERKLE_PATH | `addr:u64`, `out_ptr:u64`, სურვილისამებრ `root_out:u64` | `u64=len` | `G_mpath + len` | წერს გზას (ფოთოლი→ ფესვი) და არჩევითი root ბაიტები |
| 0xFA | GET_MERKLE_COMPACT | `addr:u64`, `out_ptr:u64`, სურვილისამებრ `depth_cap:u64`, სურვილისამებრ `root_out:u64` | `u64=depth` | `G_mpath + depth` | `[u8 depth][u32 dirs_le][u32 count][count*32 siblings]` |
| 0xFF | GET_REGISTER_MERKLE_COMPACT| `reg_index:u64`, `out_ptr:u64`, სურვილისამებრ `depth_cap:u64`, სურვილისამებრ `root_out:u64` | `u64=depth` | `G_mpath + depth` | იგივე კომპაქტური განლაგება რეგისტრაციის ვალდებულებისთვის |

გაზის აღსრულება
- CoreHost იხდის დამატებით გაზს ISI სისტელებისთვის მშობლიური ISI განრიგის გამოყენებით; FASTPQ სერიული გადარიცხვები გადაიხდება თითო შესვლისთვის.
- ZK_VERIFY syscalls ხელახლა იყენებს კონფიდენციალური გადამოწმების გაზის განრიგს (ბაზა + მტკიცებულების ზომა).
- SMARTCONTRACT_EXECUTE_QUERY საფასურის ბაზა + ერთეულზე + თითო ბაიტზე; დახარისხება ამრავლებს თითო ნივთის ღირებულებას და დაუხარისხებელი ოფსეტები ამატებს თითო პუნქტის ჯარიმას.

შენიშვნები
- ყველა ინდიკატორის არგუმენტი ეხება Norito TLV კონვერტებს INPUT რეგიონში და დამოწმებულია პირველი გადაცემისას (`E_NORITO_INVALID` შეცდომის შემთხვევაში).
- ყველა მუტაცია გამოიყენება Iroha-ის სტანდარტული შემსრულებელის მეშვეობით (`CoreHost`) და არა უშუალოდ VM-ის მიერ.
- გაზის ზუსტი მუდმივები (`G_*`) განისაზღვრება აქტიური გაზის გრაფიკით; იხილეთ `ivm.md`.

შეცდომები
- `E_SCALL_UNKNOWN`: syscall ნომერი არ არის აღიარებული აქტიური `abi_version`-ისთვის.
- შეყვანის ვალიდაციის შეცდომები ვრცელდება როგორც VM ხაფანგები (მაგ., `E_NORITO_INVALID` არასწორი TLV-ებისთვის).

ჯვარედინი მითითებები
- არქიტექტურა და VM სემანტიკა: `ivm.md`
- ენა და ჩაშენებული რუქა: `docs/source/kotodama_grammar.md`

თაობის შენიშვნა
- syscall მუდმივების სრული სია შეიძლება შეიქმნას წყაროდან:
  - `make docs-syscalls` → წერს `docs/source/ivm_syscalls_generated.md`
  - `make check-docs` → ადასტურებს, რომ გენერირებული ცხრილი განახლებულია (გამოადგება CI-ში)
- ზემოთ მოყვანილი ქვეჯგუფი რჩება კურირებულ, სტაბილურ ცხრილად კონტრაქტისკენ მიმართული syscals-ებისთვის.

## ადმინისტრატორი/როლი TLV მაგალითები (იმიტირებული მასპინძელი)

ამ განყოფილებაში დოკუმენტირებულია TLV ფორმები და მინიმალური JSON დატვირთვები, რომლებიც მიღებულია იმიტირებული WSV ჰოსტის მიერ ტესტებში გამოყენებული ადმინისტრატორის სტილის სისტელებისთვის. მაჩვენებლის ყველა არგუმენტი მიჰყვება მაჩვენებელს-ABI (Norito TLV კონვერტები მოთავსებულია INPUT-ში). წარმოების მასპინძლებს შეუძლიათ გამოიყენონ უფრო მდიდარი სქემები; ეს მაგალითები მიზნად ისახავს ტიპებისა და ძირითადი ფორმების გარკვევას.- REGISTER_PEER / UNREGISTER_PEER
  - არგები: `r10=&Json`
  - მაგალითი JSON: `{ "peer": "peer-id-or-info" }`
  - CoreHost შენიშვნა: `REGISTER_PEER` ელოდება `RegisterPeerWithPop` JSON ობიექტს `peer` + `pop` ბაიტით (სურვილისამებრ `activation_at`, I181300000000000, `pop`); `UNREGISTER_PEER` იღებს peer-id სტრიქონს ან `{ "peer": "..." }`.

- CREATE_TRIGGER / REMOVE_TRIGGER / SET_TRIGGER_ENABLED
  - CREATE_TRIGGER:
    - არგები: `r10=&Json`
    - მინიმალური JSON: `{ "name": "t1" }` (დამატებითი ველები იგნორირებულია იმიტირებით)
  - REMOVE_TRIGGER:
    - Args: `r10=&Name` (ტრიგერის სახელი)
  - SET_TRIGGER_ENABLED:
    - Args: `r10=&Name`, `r11=enabled:u64` (0 = გამორთულია, არა-ნულოვანი = ჩართულია)
  - CoreHost შენიშვნა: `CREATE_TRIGGER` მოელის ტრიგერის სრულ სპეციფიკას (base64 Norito `Trigger` სტრიქონი ან
    `{ "id": "<trigger_id>", "action": ... }` `action`, როგორც საფუძველი64 Norito `Action` სტრიქონი ან
    JSON ობიექტი) და `SET_TRIGGER_ENABLED` გადართავს ტრიგერის მეტამონაცემების კლავიშს `__enabled` (გამოტოვებულია
    ნაგულისხმევად ჩართულია).

- როლები: CREATE_ROLE / DELETE_ROLE / GRANT_ROLE / REVOKE_ROLE
  - CREATE_ROLE:
    - Args: `r10=&Name` (როლის სახელი), `r11=&Json` (ნებართვების დაყენება)
    - JSON იღებს `"perms"` ან `"permissions"` გასაღებს, თითოეული ნებართვის სახელების სტრიქონულ მასივს.
    - მაგალითები:
      - `{ "perms": [ "mint_asset:rose#wonder" ] }`
      - `{ "permissions": [ "read_assets:soraカタカナ...", "transfer_asset:rose#wonder" ] }`
    - მხარდაჭერილი ნებართვის სახელის პრეფიქსები იმიტირებულში:
      - `register_domain`, `register_account`, `register_asset_definition`
      - `read_assets:<account_id>`
      - `mint_asset:<asset_definition_id>`
      - `burn_asset:<asset_definition_id>`
      - `transfer_asset:<asset_definition_id>`
  - DELETE_ROLE:
    - არგები: `r10=&Name`
    - ვერ მოხერხდება, თუ რომელიმე ანგარიშს მაინც ენიჭება ეს როლი.
  - GRANT_ROLE / REVOKE_ROLE:
    - არგები: `r10=&AccountId` (სუბიექტი), `r11=&Name` (როლის სახელი)
  - CoreHost შენიშვნა: JSON ნებართვა შეიძლება იყოს სრული `Permission` ობიექტი (`{ "name": "...", "payload": ... }`) ან სტრიქონი (ნაგულისხმევი დატვირთვა არის `null`); `GRANT_PERMISSION`/`REVOKE_PERMISSION` მიიღება `&Name` ან `&Json(Permission)`.

- ოპერაციების გაუქმება (დომენი/ანგარიში/აქტივი): უცვლელი (დამცინებელი)
  - UNREGISTER_DOMAIN (`r10=&DomainId`) ვერ ხერხდება, თუ დომენში არსებობს ანგარიშები ან აქტივების განმარტებები.
  - UNREGISTER_ACCOUNT (`r10=&AccountId`) წარუმატებელია, თუ ანგარიშს აქვს არანულოვანი ნაშთები ან ფლობს NFT-ებს.
  - UNREGISTER_ASSET (`r10=&AssetDefinitionId`) ვერ ხერხდება, თუ აქტივისთვის ნაშთი არსებობს.

შენიშვნები
- ეს მაგალითები ასახავს იმიტირებულ WSV ჰოსტს, რომელიც გამოიყენება ტესტებში; რეალური კვანძის ჰოსტებმა შეიძლება გამოავლინონ უფრო მდიდარი ადმინისტრაციული სქემები ან მოითხოვონ დამატებითი ვალიდაცია. პოინტერ-ABI წესები კვლავ მოქმედებს: TLV-ები უნდა იყოს INPUT-ში, ვერსია=1, ტიპის ID-ები უნდა ემთხვეოდეს, ხოლო payload ჰეშები უნდა იყოს ვალიდირებული.