---
lang: mn
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

Энэ баримт бичигт IVM системийн дуудлагын дугаарууд, заагч-ABI дуудлагын конвенцууд, нөөцлөгдсөн тооны мужууд болон Kotodama-ийн бууралтад ашиглагддаг гэрээт системүүдийн каноник хүснэгтийг тодорхойлсон. Энэ нь `ivm.md` (архитектур) болон `kotodama_grammar.md` (хэл) -ийг нөхдөг.

Хувилбар хийх
- Хүлээн зөвшөөрөгдсөн системүүдийн багц нь `abi_version` талбарын байт кодын толгой хэсгээс хамаарна. Эхний хувилбар нь зөвхөн `abi_version = 1` хүлээн авдаг; элсэлтийн үед бусад утгыг үгүйсгэдэг. Идэвхтэй `abi_version`-ийн үл мэдэгдэх тоонууд нь `E_SCALL_UNKNOWN`-тай тодорхойлогддог.
- Ажиллах цагийн шинэчлэлтүүд нь `abi_version = 1`-г хадгалж, системийн дуудлагын болон заагч-ABI гадаргууг өргөтгөхгүй.
- Syscall хийн зардал нь байт кодын толгой хувилбарт холбогдсон хийн хуваарийн нэг хэсэг юм. `ivm.md` (Хийн бодлого).

Дугаарлах хүрээ
- `0x00..=0x1F`: VM цөм/хэрэгсэл (дибаг хийх/гаралтын туслагчийг `CoreHost`-ийн доор ашиглах боломжтой; үлдсэн програмын туслахууд нь зөвхөн хуурамч хост байдаг).
- `0x20..=0x5F`: Iroha үндсэн ISI гүүр (ABI v1 дээр тогтвортой).
- `0x60..=0x7F`: протоколын функцээр хамгаалагдсан ISI өргөтгөл (идэвхжүүлсэн үед ABI v1-ийн хэсэг хэвээр байна).
- `0x80..=0xFF`: хост/крипто туслахууд болон нөөцлөгдсөн слотууд; Зөвхөн ABI v1 зөвшөөрөгдсөн жагсаалтад байгаа тоонуудыг хүлээн авна.

Бат бөх туслахууд (ABI v1)
- Тогтвортой төлөв байдлын туслах системийн дуудлагууд (0x50–0x5A: STATE_{GET,SET,DEL}, ENCODE/DECODE_INT, BUILD_PATH_*, JSON/SCHEMA кодчилол/декод) нь V1 ABI-ийн нэг хэсэг бөгөөд `abi_hash` тооцоололд багтсан болно.
- CoreHost утсыг STATE_{GET,SET,DEL} хүртэл WSV-д тулгуурласан бат бөх ухаалаг гэрээний төлөв рүү шилжүүлнэ; dev/туршилтын хостууд нь дотооддоо хэвээр байж болох ч ижил системийн семантикийг хадгалах ёстой.

Заагч-ABI дуудлагын конвенц (ухаалаг-гэрээт систем)
- Аргументуудыг `r10+` регистрүүдэд `u64` түүхий утгууд эсвэл өөрчлөгддөггүй Norito TLV дугтуй (жишээ нь, `AccountId`0, `AccountId`0, `AccountId`0800, Norito) INPUT муж руу заагч болгон байрлуулна. `Name`, `Json`, `NftId`).
- Скаляр буцаах утгууд нь хостоос буцаж ирсэн `u64` юм. Заагчийн үр дүнг хост `r10` руу бичдэг.

Каноник системийн дуудлагын хүснэгт (дэд багц)| Hex | Нэр | Аргументууд (`r10+` дээр) | Буцах | Хийн (суурь + хувьсагч) | Тэмдэглэл |
|------|--------------------------------------|------------------------------------------------------------------------|-------------|------------------------------|-------|
| 0x1A | SET_ACCOUNT_DETAIL | `&AccountId`, `&Name`, `&Json` | `u64=0` | `G_set_detail + bytes(val)` | Дансны дэлгэрэнгүй мэдээллийг бичнэ |
| 0x22 | MINT_ASSET | `&AccountId`, `&AssetDefinitionId`, `&NoritoBytes(Numeric)` | `u64=0` | `G_mint` | Дансан дахь хөрөнгийн `amount` мөнгөн тэмдэгт
| 0x23 | BURN_ASSET | `&AccountId`, `&AssetDefinitionId`, `&NoritoBytes(Numeric)` | `u64=0` | `G_burn` | `amount` данснаас шатдаг |
| 0x24 | ШИЛЖҮҮЛЭХ_ХӨРӨНГӨ | `&AccountId(from)`, `&AccountId(to)`, `&AssetDefinitionId`, `&NoritoBytes(Numeric)` | `u64=0` | `G_transfer` | Данс хооронд `amount` шилжүүлэг |
| 0x29 | ШИЛЖҮҮЛЭХ_V1_БААЦАА_ЭХЛҮҮЛЭХ | – | `u64=0` | `G_transfer` | FASTPQ шилжүүлгийн багцын хамрах хүрээг эхлүүлэх |
| 0x2A | TRANSFER_V1_BATCH_END | – | `u64=0` | `G_transfer` | Хуримтлагдсан FASTPQ шилжүүлгийн багцыг угаах |
| 0x2B | ШИЛЖҮҮЛЭХ_V1_БАГЦ_ХЭРЭГЛЭХ | `r10=&NoritoBytes(TransferAssetBatch)` | `u64=0` | `G_transfer` | Norito кодлогдсон багцыг нэг системд ашиглах |
| 0x25 | NFT_MINT_ASSET | `&NftId`, `&AccountId(owner)` | `u64=0` | `G_nft_mint_asset` | Шинэ NFT | бүртгүүлнэ
| 0x26 | NFT_TRANSFER_ASSET | `&AccountId(from)`, `&NftId`, `&AccountId(to)` | `u64=0` | `G_nft_transfer_asset` | NFT | өмчлөлийг шилжүүлдэг
| 0x27 | NFT_SET_METADATA | `&NftId`, `&Json` | `u64=0` | `G_nft_set_metadata` | NFT мета өгөгдлийг шинэчлэх |
| 0x28 | NFT_BURN_ASSET | `&NftId` | `u64=0` | `G_nft_burn_asset` | NFT |-г шатаах (устгах).
| 0xA1 | SMARTCONTRACT_EXECUTE_QUERY| `r10=&NoritoBytes(QueryRequest)` | `r10=ptr (&NoritoBytes(QueryResponse))` | `G_scq + per_item*items + per_byte*bytes(resp)` | Давталттай асуулга түр зуур ажилладаг; `QueryRequest::Continue` татгалзсан |
| 0xA2 | БҮХ_ХЭРЭГЛЭГЧДИЙН_ТӨЛӨВ_NFTS_БҮТЭЭХ | – | `u64=count` | `G_create_nfts_for_all` | Туслагч; онцлогтой || 0xA3 | SET_SMARTCONTRACT_ГҮЙЦЭТГЭЛ_ГҮН | `depth:u64` | `u64=prev` | `G_set_depth` | Админ; онцлогтой |
| 0xA4 | ЭРХИЙГ АВАХ | – (хостын үр дүнг бичих) | `&AccountId`| `G_get_auth` | Хост одоогийн эрх мэдэл рүү заагчийг `r10` | руу бичнэ
| 0xF7 | GET_MERKLE_PATH | `addr:u64`, `out_ptr:u64`, нэмэлт `root_out:u64` | `u64=len` | `G_mpath + len` | Зам (навч→ үндэс) болон нэмэлт эх байт | бичнэ
| 0xFA | ГЭТ_MERKLE_COMPACT | `addr:u64`, `out_ptr:u64`, нэмэлт `depth_cap:u64`, нэмэлт `root_out:u64` | `u64=depth` | `G_mpath + depth` | `[u8 depth][u32 dirs_le][u32 count][count*32 siblings]` |
| 0xFF | БҮРТГЭЛ_БҮРТГҮҮЛЭХ_COMPACT| `reg_index:u64`, `out_ptr:u64`, нэмэлт `depth_cap:u64`, нэмэлт `root_out:u64` | `u64=depth` | `G_mpath + depth` | Бүртгэлийн амлалтад зориулсан ижил авсаархан зохион байгуулалт |

Хийн хэрэгжилт
- CoreHost нь уугуул ISI хуваарийг ашиглан ISI системд нэмэлт хийн төлбөр авдаг; FASTPQ багц шилжүүлгийг нэг оруулгад тооцно.
- ZK_VERIFY системүүд нь нууц баталгаажуулалтын хийн хуваарийг дахин ашигладаг (суурь + баталгааны хэмжээ).
- SMARTCONTRACT_EXECUTE_QUERY нь үндсэн төлбөр + зүйл тутамд + байт тутамд; эрэмбэлэх нь зүйл бүрийн зардлыг үржүүлж, ангилаагүй нөхцлүүд нь зүйл бүрийн торгуулийг нэмнэ.

Тэмдэглэл
- Бүх заагч аргументууд нь INPUT бүс дэх Norito TLV дугтуйг иш татдаг бөгөөд эхний лавлагаа дээр баталгаажсан (`E_NORITO_INVALID` алдаатай).
- Бүх мутацийг шууд VM-ээр биш Iroha стандарт гүйцэтгэгчээр (`CoreHost`-ээр) ашиглана.
- Яг хийн тогтмол (`G_*`) нь идэвхтэй хийн графикаар тодорхойлогддог; `ivm.md`-г үзнэ үү.

Алдаа
- `E_SCALL_UNKNOWN`: идэвхтэй `abi_version`-д системийн дуудлагын дугаар танигдаагүй.
- Оролтын баталгаажуулалтын алдаа нь VM урхи хэлбэрээр тархдаг (жишээ нь, алдаатай TLV-ийн хувьд `E_NORITO_INVALID`).

Хөндлөнгийн лавлагаа
- Архитектур ба VM семантик: `ivm.md`
- Хэл ба барилгын зураглал: `docs/source/kotodama_grammar.md`

Үе үеийн тэмдэглэл
- Системийн дуудлагын тогтмолуудын бүрэн жагсаалтыг эх сурвалжаас дараах байдлаар үүсгэж болно.
  - `make docs-syscalls` → `docs/source/ivm_syscalls_generated.md` гэж бичнэ
  - `make check-docs` → үүсгэсэн хүснэгт шинэчлэгдсэн эсэхийг шалгана (CI-д ашигтай)
- Дээрх дэд хэсэг нь гэрээтэй холбоотой системүүдийн тохируулсан, тогтвортой хүснэгт хэвээр байна.

## Админ/Үүрэг TLV жишээ (хуурамч хост)

Энэ хэсэг нь туршилтанд ашигласан админ маягийн системд зориулсан хуурамч WSV хостоос хүлээн зөвшөөрсөн TLV хэлбэрүүд болон хамгийн бага JSON ачааллыг баримтжуулна. Заагчийн бүх аргументууд заагч-ABI-г дагаж (Norito TLV дугтуйг INPUT дотор байрлуулсан). Үйлдвэрлэлийн хостууд илүү баялаг схемүүдийг ашиглаж болно; Эдгээр жишээнүүд нь төрөл, үндсэн хэлбэрийг тодруулах зорилготой.- REGISTER_PEER / UNREGISTER_PEER
  - Args: `r10=&Json`
  - Жишээ JSON: `{ "peer": "peer-id-or-info" }`
  - CoreHost note: `REGISTER_PEER` expects a `RegisterPeerWithPop` JSON object with `peer` + `pop` bytes (optional `activation_at`, `expiry_at`, `hsm`); `UNREGISTER_PEER` peer-id стринг буюу `{ "peer": "..." }`-г хүлээн зөвшөөрдөг.

- CREATE_TRIGGER / REMOVE_TRIGGER / SET_TRIGGER_ENABLED
  - CREATE_TRIGGER:
    - Args: `r10=&Json`
    - Хамгийн бага JSON: `{ "name": "t1" }` (хуурамчаар нэмэлт талбаруудыг үл тоомсорлосон)
  - REMOVE_TRIGGER:
    - Args: `r10=&Name` (гох нэр)
  - SET_TRIGGER_ENABLED:
    - Args: `r10=&Name`, `r11=enabled:u64` (0 = идэвхгүй, тэг биш = идэвхжүүлсэн)
  - CoreHost тэмдэглэл: `CREATE_TRIGGER` нь бүрэн триггер үзүүлэлтийг хүлээж байна (base64 Norito `Trigger` мөр эсвэл
    `{ "id": "<trigger_id>", "action": ... }` `action` суурьтай64 Norito `Action` мөр эсвэл
    JSON объект) ба `SET_TRIGGER_ENABLED` гох мета өгөгдлийн түлхүүр `__enabled` (байхгүй)
    өгөгдмөл нь идэвхжүүлсэн).

- Үүрэг: CREATE_ROLE / DELETE_ROLE / GRANT_ROLE / REVOKE_ROLE
  - CREATE_ROLE:
    - Args: `r10=&Name` (дүрийн нэр), `r11=&Json` (зөвшөөрлийн багц)
    - JSON нь `"perms"` эсвэл `"permissions"` түлхүүрүүдийн аль нэгийг хүлээн авдаг бөгөөд тус бүр нь зөвшөөрлийн нэрийн цуваа юм.
    - Жишээ нь:
      - `{ "perms": [ "mint_asset:rose#wonder" ] }`
      - `{ "permissions": [ "read_assets:<i105-account-id>", "transfer_asset:rose#wonder" ] }`
    - Хуурамч хэлбэрээр дэмжигдсэн зөвшөөрлийн нэрийн угтварууд:
      - `register_domain`, `register_account`, `register_asset_definition`
      - `read_assets:<account_id>`
      - `mint_asset:<asset_definition_id>`
      - `burn_asset:<asset_definition_id>`
      - `transfer_asset:<asset_definition_id>`
  - УСТГАХ_ҮРД:
    - Args: `r10=&Name`
    - Хэрэв ямар нэгэн бүртгэлд энэ үүрэг оногдсон хэвээр байвал амжилтгүй болно.
  - ТӨГӨЛГӨӨНИЙ_ҮҮРД / ХҮЧИН_ҮҮРЭГ:
    - Args: `r10=&AccountId` (сэдв), `r11=&Name` (үүргийн нэр)
  - CoreHost-ийн тэмдэглэл: JSON зөвшөөрөл нь бүрэн `Permission` объект (`{ "name": "...", "payload": ... }`) эсвэл мөр байж болно (өгөгдмөл нь `null`); `GRANT_PERMISSION`/`REVOKE_PERMISSION` `&Name` эсвэл `&Json(Permission)` хүлээн авна.

- Үйлдлүүдийг бүртгэлээс хасах (домайн/данс/хөрөнгө): өөрчлөгддөггүй (хуурамч)
  - UNREGISTER_DOMAIN (`r10=&DomainId`) домэйнд бүртгэл эсвэл хөрөнгийн тодорхойлолт байгаа бол амжилтгүй болно.
  - UNREGISTER_ACCOUNT (`r10=&AccountId`) данс нь тэгээс өөр үлдэгдэлтэй эсвэл NFT эзэмшдэг бол амжилтгүй болно.
  - Хөрөнгийн үлдэгдэл байгаа тохиолдолд UNREGISTER_ASSET (`r10=&AssetDefinitionId`) амжилтгүй болно.

Тэмдэглэл
- Эдгээр жишээнүүд нь туршилтанд ашигласан хуурамч WSV хостыг тусгасан; Жинхэнэ зангилааны хостууд илүү баялаг админ схемүүдийг ил гаргах эсвэл нэмэлт баталгаажуулалт шаарддаг. Заагч-ABI дүрмүүд хүчинтэй хэвээр байна: TLV нь INPUT, хувилбар=1, төрлийн ID таарч, ачааллын хэшийг баталгаажуулах ёстой.