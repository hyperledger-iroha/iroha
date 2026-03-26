---
lang: uz
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

Ushbu hujjat IVM tizim qo'ng'iroqlari raqamlarini, ko'rsatgich-ABI chaqiruv konventsiyalarini, zaxiralangan raqamlar diapazonlarini va Kotodama tushirishda foydalaniladigan kontraktga asoslangan tizimli chaqiruvlarning kanonik jadvalini belgilaydi. U `ivm.md` (arxitektura) va `kotodama_grammar.md` (til) ni toʻldiradi.

Versiyalash
- Tan olingan tizim chaqiruvlari to'plami `abi_version` bayt-kod sarlavhasiga bog'liq. Birinchi nashr faqat `abi_version = 1` ni qabul qiladi; boshqa qiymatlar qabul paytida rad etiladi. Faol `abi_version` uchun noma'lum raqamlar `E_SCALL_UNKNOWN` bilan aniq tuzoqqa tushadi.
- Ish vaqti yangilanishi `abi_version = 1` ni saqlab qoladi va tizim chaqiruvi yoki ko'rsatgich-ABI sirtlarini kengaytirmaydi.
- Syscall gaz xarajatlari bayt-kod sarlavhasi versiyasiga bog'langan versiyalashtirilgan gaz jadvalining bir qismidir. `ivm.md` (Gaz siyosati) ga qarang.

Raqamlash diapazonlari
- `0x00..=0x1F`: VM yadrosi/yordamchi dasturi (disklarni tuzatish/chiqish yordamchilari `CoreHost` ostida mavjud; qolgan ishlab chiquvchilar yordamchilari faqat soxta xost).
- `0x20..=0x5F`: Iroha yadroli ISI ko'prigi (ABI v1 da barqaror).
- `0x60..=0x7F`: protokol xususiyatlari bilan himoyalangan ISI kengaytmalari (yoqilganda ham ABI v1 ning bir qismi).
- `0x80..=0xFF`: xost/kripto yordamchilari va ajratilgan slotlar; faqat ABI v1 ruxsat etilgan ro'yxatda mavjud raqamlar qabul qilinadi.

Bardoshli yordamchilar (ABI v1)
- Bardoshli holat yordamchi tizim chaqiruvlari (0x50–0x5A: STATE_{GET,SET,DEL}, ENCODE/DECODE_INT, BUILD_PATH_*, JSON/SCHEMA encode/decode) V1 ABI qismidir va `abi_hash` hisoblashiga kiritilgan.
- CoreHost simlari STATE_{GET,SET,DEL} - WSV tomonidan qo'llab-quvvatlanadigan mustahkam smart-kontrakt holatiga; Dev/sinov xostlari mahalliy darajada saqlanib qolishi mumkin, lekin bir xil tizimli semantikani saqlab qolishi kerak.

Pointer-ABI chaqiruv konventsiyasi (aqlli-kontrakt tizimi)
- Argumentlar `r10+` registrlariga xom `u64` qiymatlari sifatida yoki oʻzgarmas Norito TLV konvertlariga INPUT hududiga koʻrsatgich sifatida joylashtiriladi (masalan, `AccountId`0, `AccountId`0, `AccountId`01 `Name`, `Json`, `NftId`).
- Skalar qaytish qiymatlari xostdan qaytarilgan `u64`. Pointer natijalari xost tomonidan `r10` ga yoziladi.

Kanonik tizimli qo'ng'iroqlar jadvali (quyi to'plam)| Hex | Ism | Argumentlar (`r10+` da) | Qaytadi | Gaz (tayanch + o'zgaruvchan) | Eslatmalar |
|------|--------------------------------------|------------------------------------------------------------------------|-------------|------------------------------|-------|
| 0x1A | SET_ACCOUNT_DETAIL | `&AccountId`, `&Name`, `&Json` | `u64=0` | `G_set_detail + bytes(val)` | Hisob uchun batafsil ma'lumot yozadi |
| 0x22 | MINT_ASSET | `&AccountId`, `&AssetDefinitionId`, `&NoritoBytes(Numeric)` | `u64=0` | `G_mint` | Hisobga aktivning `amount` pul pullari |
| 0x23 | BURN_ASSET | `&AccountId`, `&AssetDefinitionId`, `&NoritoBytes(Numeric)` | `u64=0` | `G_burn` | Hisobdan `amount` kuyadi |
| 0x24 | TRANSFER_ASSET | `&AccountId(from)`, `&AccountId(to)`, `&AssetDefinitionId`, `&NoritoBytes(Numeric)` | `u64=0` | `G_transfer` | Hisoblar o'rtasida `amount` o'tkazmalari |
| 0x29 | TRANSFER_V1_BATCH_BEGIN | – | `u64=0` | `G_transfer` | FASTPQ uzatish to'plami hajmini boshlang |
| 0x2A | TRANSFER_V1_BATCH_END | – | `u64=0` | `G_transfer` | Flush to'plangan FASTPQ uzatish to'plami |
| 0x2B | TRANSFER_V1_BATCH_Qo'llash | `r10=&NoritoBytes(TransferAssetBatch)` | `u64=0` | `G_transfer` | Norito kodlangan to'plamni bitta tizim chaqiruvida qo'llash |
| 0x25 | NFT_MINT_ASSET | `&NftId`, `&AccountId(owner)` | `u64=0` | `G_nft_mint_asset` | Yangi NFT | ro'yxatdan o'tkazadi
| 0x26 | NFT_TRANSFER_ASSET | `&AccountId(from)`, `&NftId`, `&AccountId(to)` | `u64=0` | `G_nft_transfer_asset` | NFT egalik huquqini o'tkazadi |
| 0x27 | NFT_SET_METADATA | `&NftId`, `&Json` | `u64=0` | `G_nft_set_metadata` | NFT metama'lumotlarini yangilaydi |
| 0x28 | NFT_BURN_ASSET | `&NftId` | `u64=0` | `G_nft_burn_asset` | NFT |ni yoqadi (yo'q qiladi).
| 0xA1 | SMARTCONTRACT_EXECUTE_QUERY| `r10=&NoritoBytes(QueryRequest)` | `r10=ptr (&NoritoBytes(QueryResponse))` | `G_scq + per_item*items + per_byte*bytes(resp)` | Takrorlanadigan so'rovlar vaqtincha ishlaydi; `QueryRequest::Continue` rad etildi |
| 0xA2 | HAMMA_FOYDALANUVCHILAR_UCHUN_NFTS_YARASH | – | `u64=count` | `G_create_nfts_for_all` | yordamchi; xususiyatli || 0xA3 | SET_SMARTCONTRACT_EXECUTION_DEPTH | `depth:u64` | `u64=prev` | `G_set_depth` | Admin; xususiyatli |
| 0xA4 | GET_AUTHORITY | – (mezbon natijani yozadi) | `&AccountId`| `G_get_auth` | Xost joriy vakolatga ko'rsatgichni `r10` | ga yozadi
| 0xF7 | GET_MERKLE_PATH | `addr:u64`, `out_ptr:u64`, ixtiyoriy `root_out:u64` | `u64=len` | `G_mpath + len` | Yo'lni (barg→root) va ixtiyoriy ildiz baytlarini |
| 0xFA | GET_MERKLE_COMPACT | `addr:u64`, `out_ptr:u64`, ixtiyoriy `depth_cap:u64`, ixtiyoriy `root_out:u64` | `u64=depth` | `G_mpath + depth` | `[u8 depth][u32 dirs_le][u32 count][count*32 siblings]` |
| 0xFF | GET_REGISTER_MERKLE_COMPACT| `reg_index:u64`, `out_ptr:u64`, ixtiyoriy `depth_cap:u64`, ixtiyoriy `root_out:u64` | `u64=depth` | `G_mpath + depth` | Ro'yxatga olish majburiyati uchun bir xil ixcham tartib |

Gazga rioya qilish
- CoreHost mahalliy ISI jadvalidan foydalangan holda ISI tizimlari uchun qo'shimcha gaz to'laydi; FASTPQ ommaviy o'tkazmalari har bir kirish uchun to'lanadi.
- ZK_VERIFY syscalls maxfiy tekshirish gaz jadvalini qayta ishlatadi (asosiy + isbot hajmi).
- SMARTCONTRACT_EXECUTE_QUERY to'lovi baza + har bir element + bayt uchun; saralash har bir ob'ekt narxini ko'paytiradi va saralanmagan ofsetlar har bir element uchun jarima qo'shadi.

Eslatmalar
- Barcha ko'rsatkich argumentlari INPUT hududidagi Norito TLV konvertlariga havola qiladi va birinchi murojaatda tasdiqlanadi (`E_NORITO_INVALID` xatolik).
- Barcha mutatsiyalar bevosita VM tomonidan emas, Iroha standart ijrochisi (`CoreHost` orqali) orqali qo'llaniladi.
- aniq gaz konstantalari (`G_*`) faol gaz jadvali bilan aniqlanadi; qarang: `ivm.md`.

Xatolar
- `E_SCALL_UNKNOWN`: tizim qo'ng'irog'i raqami faol `abi_version` uchun tan olinmadi.
- Kirishni tekshirish xatolar VM tuzoqlari sifatida tarqaladi (masalan, noto'g'ri tuzilgan TLVlar uchun `E_NORITO_INVALID`).

Oʻzaro havolalar
- Arxitektura va VM semantikasi: `ivm.md`
- Til va ichki xaritalash: `docs/source/kotodama_grammar.md`

Avlod eslatmasi
- Tizim konstantalarining to'liq ro'yxati quyidagi manbalardan yaratilishi mumkin:
  - `make docs-syscalls` → yozadi `docs/source/ivm_syscalls_generated.md`
  - `make check-docs` → yaratilgan jadvalning yangilanganligini tasdiqlaydi (CI da foydali)
- Yuqoridagi kichik to'plam kontrakt bilan bog'liq tizimlar uchun tuzatilgan, barqaror jadval bo'lib qoladi.

## Administrator/rol TLV misollari (soxta xost)

Ushbu bo'lim testlarda ishlatiladigan administrator uslubidagi tizim qo'ng'iroqlari uchun soxta WSV xosti tomonidan qabul qilingan TLV shakllari va minimal JSON yuklamalarini hujjatlashtiradi. Barcha ko‘rsatkich argumentlari ko‘rsatgich-ABI (INPUT ichiga joylashtirilgan Norito TLV konvertlari) ga amal qiladi. Ishlab chiqarish xostlari yanada boy sxemalardan foydalanishi mumkin; bu misollar turlar va asosiy shakllarni aniqlashtirishga qaratilgan.- REGISTER_PEER / UNREGISTER_PEER
  - Args: `r10=&Json`
  - Misol JSON: `{ "peer": "peer-id-or-info" }`
  - CoreHost note: `REGISTER_PEER` expects a `RegisterPeerWithPop` JSON object with `peer` + `pop` bytes (optional `activation_at`, `expiry_at`, `hsm`); `UNREGISTER_PEER` peer-id qatorini yoki `{ "peer": "..." }`ni qabul qiladi.

- CREATE_TRIGGER / REMOVE_TRIGGER / SET_TRIGGER_ENABLED
  - CREATE_TRIGGER:
    - Args: `r10=&Json`
    - Minimal JSON: `{ "name": "t1" }` (qo'shimcha maydonlar soxta tomonidan e'tiborga olinmaydi)
  - REMOVE_TRIGGER:
    - Args: `r10=&Name` (tetik nomi)
  - SET_TRIGGER_ENABLED:
    - Args: `r10=&Name`, `r11=enabled:u64` (0 = o'chirilgan, noldan tashqari = yoqilgan)
  - CoreHost eslatmasi: `CREATE_TRIGGER` to'liq trigger spetsifikatsiyasini kutmoqda (base64 Norito `Trigger` qatori yoki
    `{ "id": "<trigger_id>", "action": ... }` `action` asos sifatida64 Norito `Action` qatori yoki
    JSON ob'ekti) va `SET_TRIGGER_ENABLED` ishga tushiruvchi metama'lumotlar kaliti `__enabled` (yo'q)
    sukut bo'yicha yoqilgan).

- Rollar: CREATE_ROLE / DELETE_ROLE / GRANT_ROLE / REVOKE_ROLE
  - CREATE_ROLE:
    - Args: `r10=&Name` (rol nomi), `r11=&Json` (ruxsatnomalar toʻplami)
    - JSON `"perms"` yoki `"permissions"` kalitlarini qabul qiladi, ularning har biri ruxsat nomlari qatori.
    - Misollar:
      - `{ "perms": [ "mint_asset:rose#wonder" ] }`
      - `{ "permissions": [ "read_assets:soraカタカナ...", "transfer_asset:rose#wonder" ] }`
    - Soxtada qo'llab-quvvatlanadigan ruxsat nomi prefikslari:
      - `register_domain`, `register_account`, `register_asset_definition`
      - `read_assets:<account_id>`
      - `mint_asset:<asset_definition_id>`
      - `burn_asset:<asset_definition_id>`
      - `transfer_asset:<asset_definition_id>`
  - DELETE_ROLE:
    - Args: `r10=&Name`
    - Agar biron bir hisob hali ham ushbu rolga ega bo'lsa, muvaffaqiyatsiz bo'ladi.
  - GRANT_ROLE / REVOKE_ROLE:
    - Args: `r10=&AccountId` (mavzu), `r11=&Name` (rol nomi)
  - CoreHost eslatmasi: ruxsat JSON to'liq `Permission` ob'ekti (`{ "name": "...", "payload": ... }`) yoki satr bo'lishi mumkin (foydali yuk birlamchi `null` uchun); `GRANT_PERMISSION`/`REVOKE_PERMISSION` qabul qilinadi `&Name` yoki `&Json(Permission)`.

- Operatsiyalarni ro'yxatdan o'tkazish (domen/hisob/aktiv): o'zgarmas (soxta)
  - Agar domenda hisoblar yoki aktiv taʼriflari mavjud boʻlsa, UNREGISTER_DOMAIN (`r10=&DomainId`) bajarilmaydi.
  - UNREGISTER_ACCOUNT (`r10=&AccountId`) agar hisob nol boʻlmagan balansga ega boʻlsa yoki NFTga ega boʻlsa, muvaffaqiyatsiz tugadi.
  - Agar aktiv uchun balans mavjud bo'lsa, UNREGISTER_ASSET (`r10=&AssetDefinitionId`) bajarilmaydi.

Eslatmalar
- Ushbu misollar testlarda ishlatiladigan soxta WSV xostini aks ettiradi; haqiqiy tugun xostlari boyroq boshqaruv sxemalarini ochishi yoki qo'shimcha tekshirishni talab qilishi mumkin. Pointer-ABI qoidalari hali ham amal qiladi: TLV’lar INPUT’da bo‘lishi kerak, versiya=1, turdagi identifikatorlar mos kelishi va foydali yuk xeshlari tekshirilishi kerak.