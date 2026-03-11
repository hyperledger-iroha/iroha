---
lang: kk
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

Бұл құжат IVM жүйесіне қоңырау шалу нөмірлерін, көрсеткіш-ABI шақыру конвенцияларын, сақталған сандар ауқымдарын және Kotodama төмендету арқылы пайдаланылатын келісім-шарттық жүйе қоңырауларының канондық кестесін анықтайды. Ол `ivm.md` (архитектура) және `kotodama_grammar.md` (тіл) толықтырады.

Нұсқа жасау
- Танылған жүйелік қоңыраулар жинағы `abi_version` байт коды тақырыбына байланысты. Бірінші шығарылым тек `abi_version = 1` қабылдайды; басқа мәндер қабылдау кезінде қабылданбайды. Белсенді `abi_version` үшін белгісіз сандар `E_SCALL_UNKNOWN` көмегімен анықтаушы түрде ұсталады.
- Орындау уақытының жаңартулары `abi_version = 1` сақтайды және жүйе қоңырауы немесе көрсеткіш-ABI беттерін кеңейтпейді.
- Syscall газ шығындары байт-код тақырыбы нұсқасына байланыстырылған газ кестесінің бөлігі болып табылады. `ivm.md` (Газ саясаты) қараңыз.

Нөмірлеу диапазондары
- `0x00..=0x1F`: VM өзегі/утилитасы (отлад/шығу көмекшілері `CoreHost` астында қол жетімді; қалған әзірлеуші көмекшілері тек жалған хост болып табылады).
- `0x20..=0x5F`: Iroha негізгі ISI көпірі (ABI v1 нұсқасында тұрақты).
- `0x60..=0x7F`: протокол мүмкіндіктері арқылы бекітілген ISI кеңейтімі (қосылған кезде әлі де ABI v1 бөлігі).
- `0x80..=0xFF`: хост/крипто көмекшілері және резервтелген слоттар; ABI v1 рұқсат тізімінде бар сандар ғана қабылданады.

Тұрақты көмекшілер (ABI v1)
- Тұрақты күй көмекшісінің жүйелік қоңыраулары (0x50–0x5A: STATE_{GET,SET,DEL}, ENCODE/DECODE_INT, BUILD_PATH_*, JSON/SCHEMA кодтау/декодтау) V1 ABI бөлігі болып табылады және `abi_hash` есептеуіне кіреді.
- CoreHost сымдары STATE_{GET,SET,DEL} - WSV қолдайтын берік смарт-келісімшарт күйіне; әзірлеу/сынақ хосттары жергілікті түрде сақталуы мүмкін, бірақ бірдей жүйе семантикасын сақтауы керек.

Көрсеткіш-ABI шақыру конвенциясы (ақылды келісім-шарт жүйелері)
- Аргументтер `r10+` регистрлеріне шикі `u64` мәндері ретінде немесе өзгермейтін Norito TLV конверттеріне INPUT аймағына көрсеткіш ретінде орналастырылады (мысалы, `AccountId`0, `AccountId`080, `AccountId`08010. `Name`, `Json`, `NftId`).
- Скалярлық қайтару мәндері хосттан қайтарылған `u64` болып табылады. Көрсеткіш нәтижелері хост арқылы `r10` ішіне жазылады.

Канондық жүйені шақыру кестесі (ішкі жиын)| Hex | Аты | Аргументтер (`r10+` ішінде) | Қайтарады | Газ (базалық + айнымалы) | Ескертпелер |
|------|--------------------------------------|------------------------------------------------------------------------|-------------|------------------------------|-------|
| 0x1A | SET_ACCOUNT_DETAIL | `&AccountId`, `&Name`, `&Json` | `u64=0` | `G_set_detail + bytes(val)` | Есептік жазбаға деталь жазады |
| 0x22 | MINT_ASSET | `&AccountId`, `&AssetDefinitionId`, `&NoritoBytes(Numeric)` | `u64=0` | `G_mint` | шотқа активтің `amount` теңгелері |
| 0x23 | BURN_ASSET | `&AccountId`, `&AssetDefinitionId`, `&NoritoBytes(Numeric)` | `u64=0` | `G_burn` | | шотынан `amount` күйдіреді
| 0x24 | TRANSFER_ASSET | `&AccountId(from)`, `&AccountId(to)`, `&AssetDefinitionId`, `&NoritoBytes(Numeric)` | `u64=0` | `G_transfer` | `amount` шоттар арасындағы аударымдар |
| 0x29 | TRANSFER_V1_BATCH_BEGIN | – | `u64=0` | `G_transfer` | FASTPQ тасымалдау пакетінің ауқымын бастау |
| 0x2A | TRANSFER_V1_BATCH_END | – | `u64=0` | `G_transfer` | Flush жинақталған FASTPQ тасымалдау пакетін |
| 0x2B | TRANSFER_V1_BATCH_APPLY | `r10=&NoritoBytes(TransferAssetBatch)` | `u64=0` | `G_transfer` | Norito кодталған буманы жалғыз жүйе қоңырауында қолдану |
| 0x25 | NFT_MINT_ASSET | `&NftId`, `&AccountId(owner)` | `u64=0` | `G_nft_mint_asset` | Жаңа NFT | тіркейді
| 0x26 | NFT_TRANSFER_ASSET | `&AccountId(from)`, `&NftId`, `&AccountId(to)` | `u64=0` | `G_nft_transfer_asset` | NFT | меншік құқығын береді
| 0x27 | NFT_SET_METADATA | `&NftId`, `&Json` | `u64=0` | `G_nft_set_metadata` | NFT метадеректерін жаңартады |
| 0x28 | NFT_BURN_ASSET | `&NftId` | `u64=0` | `G_nft_burn_asset` | NFT | күйдіреді (жойады).
| 0xA1 | SMARTCONTRACT_EXECUTE_QUERY| `r10=&NoritoBytes(QueryRequest)` | `r10=ptr (&NoritoBytes(QueryResponse))` | `G_scq + per_item*items + per_byte*bytes(resp)` | Қайталанатын сұраулар уақытша орындалады; `QueryRequest::Continue` қабылданбады |
| 0xA2 | БАРЛЫҚ_ПАЙДАЛАНУШЫЛАР ҮШІН_NFTS_жасау | – | `u64=count` | `G_create_nfts_for_all` | Көмекші; мүмкіндігі бар || 0xA3 | SET_SMARTCONTRACT_EXECUTION_DEPTH | `depth:u64` | `u64=prev` | `G_set_depth` | Әкімші; мүмкіндігі бар |
| 0xA4 | АЛУ_AUTHORITY | – (хост нәтиже жазады) | `&AccountId`| `G_get_auth` | Хост ағымдағы өкілеттікке көрсеткішті `r10` | ішіне жазады
| 0xF7 | GET_MERKLE_PATH | `addr:u64`, `out_ptr:u64`, қосымша `root_out:u64` | `u64=len` | `G_mpath + len` | Жолды (жапырақ→түбір) және қосымша түбір байттарын жазады
| 0xFA | GET_MERKLE_COMPACT | `addr:u64`, `out_ptr:u64`, қосымша `depth_cap:u64`, қосымша `root_out:u64` | `u64=depth` | `G_mpath + depth` | `[u8 depth][u32 dirs_le][u32 count][count*32 siblings]` |
| 0xFF | GET_REGISTER_MERKLE_COMPACT| `reg_index:u64`, `out_ptr:u64`, қосымша `depth_cap:u64`, қосымша `root_out:u64` | `u64=depth` | `G_mpath + depth` | Тіркеу міндеттемесі үшін бірдей ықшам орналасу |

Газды қолдану
- CoreHost жергілікті ISI кестесін пайдалана отырып, ISI жүйе қоңыраулары үшін қосымша газды төлейді; FASTPQ пакеттік аударымдары әр жазба үшін алынады.
- ZK_VERIFY жүйе қоңыраулары құпия тексеру газ кестесін қайта пайдаланады (негізгі + дәлел өлшемі).
- SMARTCONTRACT_EXECUTE_QUERY төлем базасы + элементке + байт үшін; сұрыптау бір элементтің құнын көбейтеді, ал сұрыпталмаған есептер бір элемент үшін айыппұлды қосады.

Жазбалар
- Барлық көрсеткіш аргументтері INPUT аймағындағы Norito TLV конверттеріне сілтеме жасайды және бірінші сілтемеде тексеріледі (қате бойынша `E_NORITO_INVALID`).
- Барлық мутациялар тікелей VM арқылы емес, Iroha стандартты орындаушысы (`CoreHost` арқылы) арқылы қолданылады.
- нақты газ тұрақтылары (`G_*`) белсенді газ кестесімен анықталады; `ivm.md` қараңыз.

Қателер
- `E_SCALL_UNKNOWN`: жүйелік қоңырау нөмірі белсенді `abi_version` үшін танылмады.
- Енгізуді тексеру қателері VM тұзақтары ретінде таралады (мысалы, дұрыс емес TLV үшін `E_NORITO_INVALID`).

Айқас сілтемелер
- Архитектура және VM семантикасы: `ivm.md`
- Тіл және кірістірілген карталау: `docs/source/kotodama_grammar.md`

Буын жазбасы
- Жүйелік қоңырау константаларының толық тізімі мыналармен көзден жасалуы мүмкін:
  - `make docs-syscalls` → жазады `docs/source/ivm_syscalls_generated.md`
  - `make check-docs` → жасалған кестенің жаңартылғанын тексереді (CI-де пайдалы)
- Жоғарыдағы ішкі жиын келісім-шартқа негізделген жүйелік қоңыраулар үшін таңдалған, тұрақты кесте болып қалады.

## Әкімші/Рөл TLV мысалдары (жалған хост)

Бұл бөлім сынақтарда қолданылатын әкімші стиліндегі жүйе қоңыраулары үшін жалған WSV хосты қабылдаған TLV пішіндерін және ең аз JSON пайдалы жүктемелерін құжаттайды. Барлық көрсеткіш аргументтері көрсеткіш-ABI (INPUT ішіне орналастырылған Norito TLV конверттері) сәйкес келеді. Өндіріс хосттары неғұрлым бай схемаларды пайдалана алады; бұл мысалдар түрлер мен негізгі пішіндерді түсіндіруге бағытталған.- REGISTER_PEER / UNREGISTER_PEER
  - Args: `r10=&Json`
  - JSON мысалы: `{ "peer": "peer-id-or-info" }`
  - CoreHost note: `REGISTER_PEER` expects a `RegisterPeerWithPop` JSON object with `peer` + `pop` bytes (optional `activation_at`, `expiry_at`, `hsm`); `UNREGISTER_PEER` тең идентификатор жолын немесе `{ "peer": "..." }` қабылдайды.

- CREATE_TRIGGER / REMOVE_TRIGGER / SET_TRIGGER_ENABLED
  - CREATE_TRIGGER:
    - Args: `r10=&Json`
    - Минималды JSON: `{ "name": "t1" }` (қосымша өрістер мысқылмен еленбейді)
  - REMOVE_TRIGGER:
    - Args: `r10=&Name` (триггер атауы)
  - SET_TRIGGER_ENABLED:
    - Args: `r10=&Name`, `r11=enabled:u64` (0 = өшірілген, нөл емес = қосылған)
  - CoreHost ескертпесі: `CREATE_TRIGGER` толық триггер спецификациясын күтеді (base64 Norito `Trigger` жолы немесе
    `{ "id": "<trigger_id>", "action": ... }` негізі ретінде `action` бар64 Norito `Action` жолы немесе
    JSON нысаны) және `SET_TRIGGER_ENABLED` іске қосу метадеректер кілтін ауыстырады `__enabled` (жоқ)
    әдепкі бойынша қосулы).

- Рөлдер: CREATE_ROLE / DELETE_ROLE / GRANT_ROLE / REVOKE_ROLE
  - CREATE_ROLE:
    - Args: `r10=&Name` (рөл атауы), `r11=&Json` (рұқсаттар жинағы)
    - JSON `"perms"` немесе `"permissions"` кілттерін қабылдайды, олардың әрқайсысы рұқсат атауларының жол жиымы.
    - Мысалдар:
      - `{ "perms": [ "mint_asset:rose#wonder" ] }`
      - `{ "permissions": [ "read_assets:i105...", "transfer_asset:rose#wonder" ] }`
    - Жасанды түрде қолдау көрсетілетін рұқсат атауының префикстері:
      - `register_domain`, `register_account`, `register_asset_definition`
      - `read_assets:<account_id>`
      - `mint_asset:<asset_definition_id>`
      - `burn_asset:<asset_definition_id>`
      - `transfer_asset:<asset_definition_id>`
  - DELETE_ROLE:
    - Args: `r10=&Name`
    - Бұл рөл әлі де кез келген тіркелгіге тағайындалған болса, орындалмайды.
  - GRANT_ROLE / REVOKE_ROLE:
    - Args: `r10=&AccountId` (тақырып), `r11=&Name` (рөл атауы)
  - CoreHost ескертпесі: рұқсат JSON толық `Permission` нысаны (`{ "name": "...", "payload": ... }`) немесе жол болуы мүмкін (пайдалы жүктеме әдепкі бойынша `null`); `GRANT_PERMISSION`/`REVOKE_PERMISSION` `&Name` немесе `&Json(Permission)` қабылдайды.

- Операцияларды тіркеуден шығару (домен/шот/актив): инварианттар (жалған)
  - Доменде тіркелгілер немесе актив анықтамалары бар болса, UNREGISTER_DOMAIN (`r10=&DomainId`) сәтсіз аяқталады.
  - UNREGISTER_ACCOUNT (`r10=&AccountId`) есептік жазбада нөлден тыс теңгерім болса немесе NFT иелері болса, сәтсіз аяқталады.
  - UNREGISTER_ASSET (`r10=&AssetDefinitionId`) актив үшін қандай да бір теңгерім болса, орындалмайды.

Ескертпелер
- Бұл мысалдар сынақтарда пайдаланылатын жалған WSV хостын көрсетеді; нақты түйін хосттары бай әкімші схемаларын көрсетуі немесе қосымша тексеруді қажет етуі мүмкін. Көрсеткіш-ABI ережелері әлі де қолданылады: TLVs INPUT параметрінде болуы керек, нұсқа=1, түр идентификаторлары сәйкес келуі керек және пайдалы жүктеме хэштері тексерілуі керек.