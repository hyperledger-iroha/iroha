---
lang: uz
direction: ltr
source: docs/account_structure.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 01561366c9698c10d29ff3f49ad4c14b22b1796b5c7701cf98d200a140af1caf
source_last_modified: "2026-01-28T17:11:30.635172+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Hisob tuzilmasi RFC

**Holat:** Qabul qilingan (ADDR-1)  
**Tomoshabinlar:** Maʼlumotlar modeli, Torii, Nexus, Wallet, Boshqaruv guruhlari  
**Tegishli muammolar:** TBD

## Xulosa

Ushbu hujjatda amalga oshirilgan yuk tashish hisob-manzillash to'plami tasvirlangan
`AccountAddress` (`crates/iroha_data_model/src/account/address.rs`) va
yordamchi asboblar. U quyidagilarni ta'minlaydi:

- Tekshirish summasi, insonga qaragan **I105 manzili (I105)** tomonidan ishlab chiqarilgan
  `AccountAddress::to_i105`, bu zanjir diskriminantini hisobga bog'laydi
  boshqaruvchi va deterministik o'zaro hamkorlik uchun qulay matn shakllarini taklif qiladi.
- Yashirin standart domenlar va mahalliy dayjestlar uchun domen selektorlari, a bilan
  Nexus tomonidan qo'llab-quvvatlanadigan kelajakdagi marshrutlash uchun zaxiralangan global registr selektor yorlig'i (
  ro'yxatga olish kitobi **hali jo'natilmagan**).

## Motivatsiya

Hamyonlar va zanjirdan tashqari asboblar bugungi kunda xom `alias@domain` (rejected legacy form) marshrutlash taxalluslariga tayanadi. Bu
ikkita asosiy kamchilikka ega:

1. **Tarmoqqa bog‘lanmagan.** Satrda nazorat summasi yoki zanjir prefiksi yo‘q, shuning uchun foydalanuvchilar
   noto'g'ri tarmoqdan manzilni darhol javobsiz joylashtirishi mumkin. The
   bitim oxir-oqibat rad etiladi (zanjir mos kelmasligi) yoki undan ham yomoni, muvaffaqiyatli bo'ladi
   agar maqsad mahalliy bo'lsa, ko'zda tutilmagan hisobga qarshi.
2. **Domenlar to‘qnashuvi.** Domenlar faqat nomlar maydoni bo‘lib, har birida qayta ishlatilishi mumkin.
   zanjir. Xizmatlar federatsiyasi (qo'riqchilar, ko'priklar, o'zaro faoliyat zanjirlar)
   mo'rt bo'ladi, chunki A zanjiridagi `finance` `finance` bilan bog'liq emas.
   zanjir B.

Bizga nusxa ko‘chirish/joylashtirish xatolaridan himoya qiluvchi inson uchun qulay manzil formati kerak
va domen nomidan vakolatli zanjirga deterministik xaritalash.

## Maqsadlar

- Ma'lumotlar modelida amalga oshirilgan I105 konvertini va
  `AccountId` va `AccountAddress` amal qiladigan kanonik tahlil/taxallus qoidalari.
- Konfiguratsiya qilingan zanjir diskriminantini to'g'ridan-to'g'ri har bir manzilga kodlash va
  uning boshqaruvi/ro'yxatga olish jarayonini belgilang.
- Oqimni buzmasdan global domen registrini qanday joriy etishni tasvirlab bering
  joylashtirish va normallashtirish/spoofingga qarshi qoidalarni belgilang.

## Maqsadsiz

- O'zaro zanjirli aktivlarni o'tkazishni amalga oshirish. Marshrutlash qatlami faqat ni qaytaradi
  maqsad zanjiri.
- Global domen emissiyasi uchun boshqaruvni yakunlash. Ushbu RFC ma'lumotlarga qaratilgan
  model va transport primitivlari.

## Fon

### Joriy marshrutlash taxallus

```
AccountId {
    domain: DomainId,   // wrapper over Name (ASCII-ish string)
    controller: AccountController // single PublicKey or multisig policy
}

Display: canonical I105 literal (no `@domain` suffix)
Parse accepts:
- Encoded account identifiers only: I105.
- Runtime parsers reject canonical hex (`0x...`), any `@<domain>` suffix, and alias literals such as `label@domain`.

Multihash hex is canonical: varint bytes are lowercase hex, payload bytes are uppercase hex,
and `0x` prefixes are not accepted.

This text form is now treated as an **account alias**: a routing convenience
that points to the canonical [`AccountAddress`](#2-canonical-address-codecs).
It remains useful for human readability and domain-scoped governance, but it is
no longer considered the authoritative account identifier on-chain.
```

`ChainId` `AccountId` dan tashqarida yashaydi. Tugunlar tranzaksiyaning `ChainId` raqamini tekshiradi
qabul paytida konfiguratsiyaga qarshi (`AcceptTransactionFail::ChainIdMismatch`)
va xorijiy tranzaktsiyalarni rad etish, lekin hisob qatorining o'zi yo'q
tarmoq maslahati.

### Domen identifikatorlari

`DomainId` `Name` (normallashtirilgan qator) ni o'rab oladi va mahalliy zanjirga qamrab olinadi.
Har bir zanjir `wonderland`, `finance` va boshqalarni mustaqil ravishda ro'yxatdan o'tkazishi mumkin.

### Nexus konteksti

Nexus komponentlar o'rtasidagi muvofiqlashtirish (yo'laklar/ma'lumotlar bo'shliqlari) uchun javobgardir. Bu
hozirda zanjirli domenlarni marshrutlash tushunchasiga ega emas.

## Taklif etilgan dizayn

### 1. Deterministik zanjir diskriminanti

`iroha_config::parameters::actual::Common` endi fosh qiladi:

```rust
pub struct Common {
    pub chain: ChainId,
    pub chain_discriminant: u16, // globally coordinated
    // ... existing fields
}
```

- **Cheklovlar:**
  - Har bir faol tarmoq uchun yagona; bilan imzolangan davlat reestri orqali boshqariladi
    aniq ajratilgan diapazonlar (masalan, `0x0000–0x0FFF` test/dev, `0x1000–0x7FFF`
    jamoat ajratmalari, `0x8000–0xFFEF` boshqaruv tomonidan tasdiqlangan, `0xFFF0–0xFFFF`
    zaxiralangan).
  - Yugurish zanjiri uchun o'zgarmas. Uni o'zgartirish uchun qattiq vilka va a
    ro'yxatga olish kitobini yangilash.
- **Boshqaruv va registr (rejalashtirilgan):** Ko‘p imzoli boshqaruv to‘plami
  imzolangan JSON registrini inson taxalluslari bilan diskriminantlarni xaritalash va saqlash
  CAIP-2 identifikatorlari. Ushbu registr hali jo'natilgan ish vaqtining bir qismi emas.
- **Foydalanish:** Davlat ruxsati, Torii, SDK va hamyon API-lari orqali o'tkaziladi.
  har bir komponent uni joylashtirishi yoki tasdiqlashi mumkin. CAIP-2 ta'siri kelajak bo'lib qolmoqda
  o'zaro hamkorlik vazifasi.

### 2. Kanonik manzil kodeklari

Rust ma'lumotlar modeli bitta kanonik foydali yuk ko'rinishini ochib beradi
(`AccountAddress`) insonga qaragan bir nechta formatlar sifatida chiqarilishi mumkin. I105 hisoblanadi
almashish va kanonik chiqish uchun afzal qilingan hisob formati; siqilgan
`sora` shakli kana alifbosi bo'lgan UX uchun ikkinchi eng yaxshi, faqat Sora variantidir.
qiymat qo‘shadi. Kanonik hex disk raskadrovka yordami bo'lib qolmoqda.

- **I105** – zanjirni joylashtirgan I105 konverti
  diskriminant. Dekoderlar foydali yukni ko'tarishdan oldin prefiksni tasdiqlaydi
  kanonik shakl.
- **Sora-siqilgan koʻrinish** – faqat Sora alifbosi, **105 ta belgidan** tuzilgan
  58 belgidan iborat boʻlgan yarim kenglikdagi sheʼrni (jumladan, ヰ va ヱ) qoʻshish
  I105 to'plami. Satrlar sentinel `sora` bilan boshlanadi, Bech32m-dan olingan.
  nazorat summasini kiriting va tarmoq prefiksini qoldiring (Sora Nexus qo'riqchi tomonidan nazarda tutilgan).

  ```
  I105  : 123456789ABCDEFGHJKMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz
  Iroha : ｲﾛﾊﾆﾎﾍﾄﾁﾘﾇﾙｦﾜｶﾖﾀﾚｿﾂﾈﾅﾗﾑｳヰﾉｵｸﾔﾏｹﾌｺｴﾃｱｻｷﾕﾒﾐｼヱﾋﾓｾｽ
  ```
- **Canonical hex** – kanonik baytni tuzatish uchun qulay `0x…` kodlash.
  konvert.

`AccountAddress::parse_encoded` I105 (afzal), siqilgan (`sora`, ikkinchi eng yaxshi) yoki kanonik olti burchakni avtomatik aniqlaydi
(Faqat `0x...`; yalang'och o'n oltilik rad etiladi) dekodlangan foydali yukni va aniqlangan yukni kiritadi va qaytaradi
`AccountAddress`. Torii endi ISO 20022 qo'shimchasi uchun `parse_encoded` ni chaqiradi
kanonik olti burchakli shaklga murojaat qiladi va saqlaydi, shuning uchun metadata deterministik bo'lib qoladi
asl vakillikdan qat'iy nazar.

#### 2.1 Sarlavha bayt tartibi (ADDR-1a)

Har bir kanonik foydali yuk `header · controller` sifatida joylashtirilgan. The
`header` bitta bayt bo'lib, u baytlarga qaysi tahlil qilish qoidalari qo'llanilishini bildiradi.
amal qiling:

```
bit index:   7        5 4      3 2      1 0
             ┌─────────┬────────┬────────┬────┐
payload bit: │version  │ class  │  norm  │ext │
             └─────────┴────────┴────────┴────┘
```

Shunday qilib, birinchi bayt quyi oqim dekoderlari uchun sxema metama'lumotlarini to'playdi:

| Bitlar | Maydon | Ruxsat berilgan qiymatlar | Buzilishdagi xato |
|------|-------|----------------|--------------------|
| 7-5 | `addr_version` | `0` (v1). `1-7` qiymatlari kelajakdagi tahrirlar uchun ajratilgan. | `0-7` triggeridan tashqari qiymatlar `AccountAddressError::InvalidHeaderVersion`; ilovalar nolga teng bo'lmagan versiyalarni bugungi kunda qo'llab-quvvatlanmaydigan deb hisoblashi KERAK. |
| 4-3 | `addr_class` | `0` = bitta kalit, `1` = multisig. | Boshqa qiymatlar `AccountAddressError::UnknownAddressClass` ni oshiradi. |
| 2-1 | `norm_version` | `1` (Norm v1). `0`, `2`, `3` qiymatlari zaxiralangan. | `0-3` dan tashqari qiymatlar `AccountAddressError::InvalidNormVersion` ni oshiradi. |
| 0 | `ext_flag` | `0` boʻlishi kerak. | O'rnatilgan bit ko'tariladi `AccountAddressError::UnexpectedExtensionFlag`. |

Rust kodlovchisi bitta kalitli kontrollerlar uchun `0x02` ni yozadi (versiya 0, sinf 0,
norma v1, kengaytma bayrog'i tozalandi) va multisig kontrollerlari uchun `0x0A` (versiya 0,
1-sinf, norma v1, kengaytma bayrog'i tozalangan).

#### 2.2 Domainless payload semantics

Canonical payload bytes are domainless: the wire layout is `header · controller`
with no selector segment, no implicit default-domain reconstruction, and no
public decode fallback for legacy scoped-account literals.

Explicit domain context is modeled separately as `ScopedAccountId { account,
domain }` or separate API fields; it is not encoded into `AccountId` payload
bytes.

| Tag | Meaning | Payload | Notes |
|-----|---------|---------|-------|
| `0x00` | Domainless canonical scope | none | Canonical account payloads are domainless; explicit domain context lives outside the address payload. |
| `0x01` | Local domain digest | 12 bytes | Digest = `blake2s_mac(key = "SORA-LOCAL-K:v1", canonical_label)[0..12]`. |
| `0x02` | Global registry entry | 4 bytes | Big-endian `registry_id`; reserved until the global registry ships. |

Domain labels are canonicalised (UTS-46 + STD3 + NFC) before hashing. Unknown tags raise `AccountAddressError::UnknownDomainTag`. When validating an address against a domain, mismatched selectors raise `AccountAddressError::DomainMismatch`.

```
legacy selector segment
┌──────────┬──────────────────────────────────────────────┐
│ tag (u8) │ payload (depends on selector kind, see table)│
└──────────┴──────────────────────────────────────────────┘
```

When present, the selector is immediately adjacent to the controller payload, so
a decoder can walk the wire format in order: read the tag byte, read the
tag-specific payload, then move on to the controller bytes.

**Legacy selector examples**

- *Implicit default* (`tag = 0x00`). No payload. Example canonical hex for the default
  domain using the deterministic test key:
  `0x020001203b6a27bcceb6a42d62a3a8d02a6f0d73653215771de243a63ac048a18b59da29`.
- *Local digest* (`tag = 0x01`). Payload is the 12-byte digest. Example (`treasury` seed
  `0x01`): `0x0201b18fe9c1abbac45b3e38fc5d0001208a88e3dd7409f195fd52db2d3cba5d72ca6709bf1d94121bf3748801b40f6f5c`.
- *Global registry* (`tag = 0x02`). Payload is a big-endian `registry_id:u32`. The bytes
  that follow the payload are identical to the implicit-default case; the selector simply
  replaces the normalised domain string with a registry pointer. Example using
  `registry_id = 0x0000_002A` (decimal 42) and the deterministic default controller:
  `0x02020000002a000120641297079357229f295938a4b5a333de35069bf47b9d0704e45805713d13c201`.

#### 2.3 Kontrollerning foydali yukini kodlash (ADDR-1a)

Tekshirish moslamasining foydali yuki domen selektoridan keyin qo'shilgan yana bir tegli birlashmadir:| teg | Nazoratchi | Layout | Eslatmalar |
|-----|------------|--------|-------|
| `0x00` | Yagona kalit | `curve_id:u8` · `key_len:u8` · `key_bytes` | `curve_id=0x01` bugungi kunda Ed25519-ga mos keladi. `key_len` `u8` bilan chegaralangan; kattaroq qiymatlar `AccountAddressError::KeyPayloadTooLong` ni oshiradi (shuning uchun >255 bayt bo'lgan bitta kalitli ML-DSA ochiq kalitlarini kodlab bo'lmaydi va multisigdan foydalanishi kerak). |
| `0x01` | Multisig | `version:u8` · `threshold:u16` · `member_count:u8` · (`curve_id:u8` · `weight:u16` · `key_len:u16` · `key_len:u16`)\* | 255 tagacha a'zoni qo'llab-quvvatlaydi (`CONTROLLER_MULTISIG_MEMBER_MAX`). Noma'lum egri chiziqlar `AccountAddressError::UnknownCurve` ko'taradi; noto'g'ri shakllangan siyosatlar `AccountAddressError::InvalidMultisigPolicy` sifatida ko'tariladi. |

Multisig siyosatlari CTAP2 uslubidagi CBOR xaritasini va kanonik dayjestni ham ochib beradi
xostlar va SDKlar kontrollerni deterministik tarzda tekshirishi mumkin. Qarang
Sxema uchun `docs/source/references/multisig_policy_schema.md` (ADDR-1c),
tekshirish qoidalari, xeshlash tartibi va oltin armatura.

Barcha kalit baytlar aynan `PublicKey::to_bytes` tomonidan qaytarilgan tarzda kodlangan; dekoderlar `PublicKey` nusxalarini qayta tiklaydi va agar baytlar e'lon qilingan egri chiziqqa mos kelmasa, `AccountAddressError::InvalidPublicKey` ni ko'taradi.

> **Ed25519 kanonik tatbiq (ADDR-3a):** `0x01` egri chiziqli kalitlar imzolovchi tomonidan chiqarilgan aniq bayt qatorini dekodlashi va kichik tartibli kichik guruhda yotmasligi kerak. Endi tugunlar kanonik boʻlmagan kodlashlarni (masalan, `2^255-19` modulining qisqartirilgan qiymatlari) va identifikatsiya elementi kabi zaif nuqtalarni rad etadi, shuning uchun SDK manzillarni yuborishdan oldin mos keladigan tekshirish xatolarini koʻrsatishi kerak.

##### 2.3.1 Egri chiziq identifikatorlari reestri (ADDR-1d)

| ID (`curve_id`) | Algoritm | Xususiyatlar darvozasi | Eslatmalar |
|----------------|-----------|--------------|-------|
| `0x00` | Zaxiralangan | — | Emissiya qilinmasligi kerak; dekoderlar yuzasi `ERR_UNKNOWN_CURVE`. |
| `0x01` | Ed25519 | — | Kanonik v1 algoritmi (`Algorithm::Ed25519`); standart konfiguratsiyada yoqilgan. |
| `0x02` | ML‑DSA (Dilithium3) | — | Dilithium3 ochiq kalit baytlaridan foydalanadi (1952 bayt). Bitta kalitli manzillar ML‑DSA kodini kodlay olmaydi, chunki `key_len` `u8`; multisig `u16` uzunliklaridan foydalanadi. |
| `0x03` | BLS12‑381 (normal) | `bls` | G1 da ochiq kalitlar (48 bayt), G2 da imzolar (96 bayt). |
| `0x04` | secp256k1 | — | SHA‑256 ustidan deterministik ECDSA; ochiq kalitlar 33 baytlik SEC1 siqilgan shakldan foydalanadi va imzolar kanonik 64 bayt `r∥s` tartibidan foydalanadi. |
| `0x05` | BLS12‑381 (kichik) | `bls` | G2 da ochiq kalitlar (96 bayt), G1 da imzolar (48 bayt). |
| `0x0A` | GOST R 34.10-2012 (256, A to'plami) | `gost` | Faqat `gost` funksiyasi yoqilganda mavjud. |
| `0x0B` | GOST R 34.10-2012 (256, B to'plami) | `gost` | Faqat `gost` funksiyasi yoqilganda mavjud. |
| `0x0C` | GOST R 34.10-2012 (256, C to'plami) | `gost` | Faqat `gost` funksiyasi yoqilganda mavjud. |
| `0x0D` | GOST R 34.10-2012 (512, A to'plami) | `gost` | Faqat `gost` funksiyasi yoqilganda mavjud. |
| `0x0E` | GOST R 34.10-2012 (512, B to'plami) | `gost` | Faqat `gost` funksiyasi yoqilganda mavjud. |
| `0x0F` | SM2 | `sm` | DistID uzunligi (u16 BE) + DistID baytlari + 65 bayt SEC1 siqilmagan SM2 kaliti; faqat `sm` yoqilganda mavjud. |

`0x06–0x09` uyalari qo'shimcha egri chiziqlar uchun tayinlanmagan bo'lib qoladi; yangisini joriy etish
algoritm yo'l xaritasini yangilashni va mos keladigan SDK/xost qamrovini talab qiladi. Kodlovchilar
`ERR_UNSUPPORTED_ALGORITHM` bilan qo'llab-quvvatlanmaydigan algoritmni rad etish KERAK va
dekoderlar saqlab qolish uchun `ERR_UNKNOWN_CURVE` bilan noma'lum identifikatorlarda tezda ishdan chiqishi KERAK
muvaffaqiyatsiz yopiq xatti-harakatlar.

Kanonik registr (jumladan, mashinada o'qiladigan JSON eksporti) ostida yashaydi
[`docs/source/references/address_curve_registry.md`](source/references/address_curve_registry.md).
Asboblar ushbu ma'lumotlar to'plamini to'g'ridan-to'g'ri iste'mol qilishi KERAK, shuning uchun egri identifikatorlar qoladi
SDK va operator ish oqimlarida izchil.

- **SDK gating:** SDK standart Ed25519 uchun faqat tekshirish/kodlash uchun. Swift fosh qiladi
  kompilyatsiya vaqti bayroqlari (`IROHASWIFT_ENABLE_MLDSA`, `IROHASWIFT_ENABLE_GOST`,
  `IROHASWIFT_ENABLE_SM`); Java/Android SDK talab qiladi
  `AccountAddress.configureCurveSupport(...)`; JavaScript SDK foydalanadi
  `configureCurveSupport({ allowMlDsa: true, allowGost: true, allowSm2: true })`.
  secp256k1 yordami mavjud, lekin JS/Android da sukut bo'yicha yoqilmagan
  SDK; qo'ng'iroq qiluvchilar Ed25519 bo'lmagan kontrollerlarni chiqarishda ochiq-oydin kirishlari kerak.
- **Xost darvozasi:** `Register<Account>` imzolovchilari algoritmlardan foydalanadigan kontrollerlarni rad etadi
  tugunning `crypto.allowed_signing` roʻyxatida **yoki** egri chiziq identifikatorlari yoʻq
  `crypto.curves.allowed_curve_ids`, shuning uchun klasterlar yordamni reklama qilishi kerak (konfiguratsiya +
  genesis) ML‑DSA/GOST/SM kontrollerlarini ro'yxatdan o'tkazishdan oldin. BLS boshqaruvchisi
  kompilyatsiya qilishda algoritmlarga har doim ruxsat beriladi (konsensus kalitlari ularga tayanadi),
  va standart konfiguratsiya Ed25519 + secp256k1 ni yoqadi.【crates/iroha_core/src/smartcontracts/isi/domain.rs:32】

##### 2.3.2 Multisig kontroller bo'yicha ko'rsatmalar

`AccountController::Multisig` orqali siyosatlarni ketma-ketlashtiradi
`crates/iroha_data_model/src/account/controller.rs` va sxemani amalga oshiradi
[`docs/source/references/multisig_policy_schema.md`](source/references/multisig_policy_schema.md) da hujjatlashtirilgan.
Amalga oshirishning asosiy tafsilotlari:

- Siyosatlar avval `MultisigPolicy::validate()` tomonidan normallashtiriladi va tasdiqlanadi
  o'rnatilgan. Eshiklar ≥1 va ≤S vazn bo'lishi kerak; dublikat a'zolardir
  `(algorithm || 0x00 || key_bytes)` tomonidan saralangandan so'ng deterministik tarzda olib tashlandi.
- Ikkilik kontrollerning foydali yuki (`ControllerPayload::Multisig`) kodlaydi
  `version:u8`, `threshold:u16`, `member_count:u8`, keyin har bir a'zoning
  `(curve_id, weight:u16, key_len:u16, key_bytes)`. Aynan shu narsa
  `AccountAddress::canonical_bytes()` I105 (afzal)/sora (ikkinchi-eng yaxshi) foydali yuklarga yozadi.
- Hashing (`MultisigPolicy::digest_blake2b256()`) Blake2b-256 dan foydalanadi.
  `iroha-ms-policy` shaxsiylashtirish qatori, shuning uchun boshqaruv manifestlari
  I105 ichiga o'rnatilgan kontroller baytlariga mos keladigan deterministik siyosat identifikatori.
- Armatura qamrovi `fixtures/account/address_vectors.json` da ishlaydi (holatlar
  `addr-multisig-*`). Hamyonlar va SDK kanonik I105 qatorlarini tasdiqlashi kerak
  ularning kodlovchilari Rust dasturiga mos kelishini tasdiqlash uchun quyida.

| Ish ID | Eshik / a'zolar | I105 literal (prefiks `0x02F1`) | Sora siqilgan (`sora`) literal | Eslatmalar |
|---------|---------------------|--------------------------------|-------------------------|-------|
| `addr-multisig-council-threshold3` | `≥3` vazni, a'zolar `(2,1,1)` | `SRfSHsrH3tEmYaaAYyD248F3vfT1oQ3WEGS22MaD8W9bLefF7rsoKLYGcpbcM9EcSus5ZhCAZU7ztn2BCsyeCAdfRncAVmVsipd4ibk6CBLF3Nrzcw8P7VKJg6mtFgEhWVTjfDkUMoc63oeEmaWyV6cyiphwk8ZgKAJUe4TyVtmKm1WWcg7qZ6i` | `sora3vﾑ2zkaoUwﾋﾅGﾘﾚyﾂe3ﾖfﾙヰｶﾘﾉwｷnoWﾛYicaUr3ﾔｲﾖ2Ado3TﾘYQﾉJqﾜﾇｳﾑﾐd8dDjRGｦ3Vﾃ9HcﾀMヰR8ﾎﾖgEqGｵEｾDyc5ﾁ1ﾔﾉ31sUﾑﾀﾖaｸxﾘ3ｲｷMEuFｺｿﾉBQSVQnxﾈeJzrXLヰhｿｹ5SEEﾅPﾂﾗｸdヰﾋ1bUGHｲVXBWNNJ6K` | Kengash-domen boshqaruvi kvorum. |
| `addr-multisig-wonderland-threshold2` | `≥2`, a'zolar `(1,2)` | `3xsmkps1KPBn9dtpE5qHRhHEZCpiAe8d9j6H9A42TV6kc1TpaqdwnSksKgQrsSEHznqvWKBMc1os69BELzkLjsR7EV2gjV14d9JMzo97KEmYoKtxCrFeKFAcy7ffQdboV1uRt` | `sora2ﾖZﾘeｴAdx3ﾂﾉﾔXhnｹﾀ2ﾉｱﾋxﾅﾄﾌヱwﾐmﾊvEﾐCﾏﾎｦ1ﾑHﾋso2GKﾔﾕﾁwﾂﾃP6ﾁｼﾙﾖｺ9ｻｦbﾈ4wFdﾑFヰ3HaﾘｼMｷﾌHWtｷﾋLﾙﾖQ4D3XﾊﾜXmpktﾚｻ5ﾅﾅﾇ1gkﾏsCFQGH9` | Ikki imzo mo''jizalar olamiga misol (vazn 1 + 2). |
| `addr-multisig-default-quorum3` | `≥3`, a'zolar `(1,1,1,1)` | `nA2bDNhMqXz7ERkHNoEWbvJGyR1aDRsw32LaUWLgbK3vcpzohmdFCLvdotxUWWDY3aZeX4ptLk4Z6TjF5ossnJm8VrNo6daxmGTkqUyP4MxJxiNyPFxsEE5DLnsoLWUcxaWNpZ76tmkbiGS31Gv8tejKpuiHUMaQ1s5ohWyZvDnpycNkBK8AEfGJqn5yc9zAzfWbVhpDwkPj8ScnzvH1Echr5` | `soraﾐ38ﾅｴｸﾜ8ﾃzwBrqﾘｺ4yﾄv6kqJp1ｳｱﾛｿrzﾄﾃﾘﾒRﾗtV9ｼﾔPｽcヱEﾌVVVｼﾘｲZAｦﾓﾅｦeﾒN76vﾈcuｶuﾛL54rzﾙﾏX2zMﾌRLﾃﾋpﾚpｲcHﾑﾅﾃﾔzｵｲVfAﾃﾚﾎﾚCヰﾔｲｽｦw9ﾔﾕ8bGGkﾁ6sNｼaｻRﾖﾜYﾕﾚU18ﾅHヰﾌuMeﾊtﾂrｿj95Ft8ﾜ3fﾄkNiｴuﾈrCﾐQt8ヱｸｸmﾙﾒgUbﾑEKTTCM` | Asosiy boshqaruv uchun noaniq-standart domen kvorum ishlatiladi.

#### 2.4 Muvaffaqiyatsizlik qoidalari (ADDR-1a)

- Kerakli sarlavha + selektordan qisqaroq yoki qolgan baytlarga ega foydali yuklar `AccountAddressError::InvalidLength` yoki `AccountAddressError::UnexpectedTrailingBytes` chiqaradi.
- Zaxiralangan `ext_flag` ni o'rnatadigan yoki qo'llab-quvvatlanmaydigan versiyalar/sinflarni reklama qiluvchi sarlavhalar `UnexpectedExtensionFlag`, `InvalidHeaderVersion` yoki `UnknownAddressClass` yordamida rad etilishi LAZOR.
- Noma'lum selektor/kontroller teglari `UnknownDomainTag` yoki `UnknownControllerTag` ko'taradi.
- Katta o'lchamli yoki noto'g'ri shakllangan asosiy material `KeyPayloadTooLong` yoki `InvalidPublicKey` ni ko'taradi.
- 255 a'zodan ortiq multisig kontrollerlari `MultisigMemberOverflow` ni ko'taradi.
- IME/NFKC konvertatsiyalari: yarim kenglikdagi Sora kanani dekodlashni buzmasdan toʻliq kenglikdagi shakllariga normallashtirish mumkin, lekin ASCII `sora` sentinel va I105 raqamlar/harflar ASCII boʻlib qolishi KERAK. To'liq kenglikdagi yoki korpusli katlanmış qo'riqchilar yuzasi `ERR_MISSING_COMPRESSED_SENTINEL`, to'liq kenglikdagi ASCII foydali yuklari `ERR_INVALID_COMPRESSED_CHAR` ni oshiradi va nazorat summasining mos kelmasligi `ERR_CHECKSUM_MISMATCH` sifatida ko'tariladi. `crates/iroha_data_model/src/account/address.rs`-dagi mulk testlari ushbu yo'llarni qamrab oladi, shuning uchun SDK va hamyonlar deterministik nosozliklarga tayanishi mumkin.
- Torii va `address@domain` (rejected legacy form) taxalluslarining SDK tahlili endi I105 (afzal)/sora (ikkinchi-eng yaxshi) kiritishlar taxallusdan oldin muvaffaqiyatsizlikka uchraganida (masalan, domen tuzilmasi xatosi, shuning uchun tekshirish yig‘indisi noto‘g‘ri bo‘lishi mumkin), endi bir xil `ERR_*` kodlarini chiqaradi. nasriy satrlardan taxmin qilish.
- 12 baytdan qisqaroq mahalliy selektorning foydali yuklari `ERR_LOCAL8_DEPRECATED` yuzasida eski Local‑8 dayjestlaridan qattiq kesishni saqlaydi.
- Domainless canonical I105 literals decode directly to a domainless `AccountId`. Use `ScopedAccountId` only when an interface requires explicit domain context.

#### 2.5 Normativ ikkilik vektorlar

- **Yashirin standart domen (`default`, asosiy bayt `0x00`)**  
  Kanonik olti burchakli: `0x020001203b6a27bcceb6a42d62a3a8d02a6f0d73653215771de243a63ac048a18b59da29`.  
  Buzilish: `0x02` sarlavhasi, `0x00` selektori (so'zsiz), `0x00` kontroller yorlig'i, `0x01` egri chizig'i identifikatori (Ed25519), I18NI00000027 tomonidan to'langan kalit, 2-to'liq kalit uzunligi.
- **Mahalliy domen dayjesti (`treasury`, yadro bayti `0x01`)**  
  Kanonik olti burchakli: `0x0201b18fe9c1abbac45b3e38fc5d0001208a88e3dd7409f195fd52db2d3cba5d72ca6709bf1d94121bf3748801b40f6f5c`.  
  Ajratish: `0x02` sarlavhasi, selektor yorlig'i `0x01` plyus dayjest `b1 8f e9 c1 ab ba c4 5b 3e 38 fc 5d`, undan keyin bitta kalitli foydali yuk (`0x00` teg, `0x01` egri chizig'i id 07-28, I18byte) Ed25519 kaliti).Birlik testlari (`account::address::tests::parse_encoded_accepts_all_formats`) quyidagi V1 vektorlarini `AccountAddress::parse_encoded` orqali tasdiqlaydi, bu asboblar olti burchakli, I105 (afzal) va siqilgan (`sora`, ikkinchi eng yaxshi) shakllar bo'ylab kanonik foydali yukga tayanishi mumkinligini kafolatlaydi. `cargo run -p iroha_data_model --example address_vectors` bilan kengaytirilgan armatura to'plamini qayta tiklang.

| Domen | Urug' bayti | Kanonik hex | Siqilgan (`sora`) |
|-------------|-----------|-----------------------------------------------------------------------------------------|------------|
| standart | `0x00` | `0x020001203b6a27bcceb6a42d62a3a8d02a6f0d73653215771de243a63ac048a18b59da29` | `sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE` |
| xazina | `0x01` | `0x0201b18fe9c1abbac45b3e38fc5d0001208a88e3dd7409f195fd52db2d3cba5d72ca6709bf1d94121bf3748801b40f6f5c` | `sora5ｻu6rﾀCヰTGwﾏ1ﾅヱﾌQｲﾖﾇqCｦヰﾓZQCZRDSSﾅMｱﾙヱｹﾁｸ8ｾeﾄﾛ6C8bZuwﾗｹCZｦRSLQFU` |
| ajoyibotlar mamlakati | `0x02` | `0x0201b8ae571b79c5a80f5834da2b0001208139770ea87d175f56a35466c34c7ecccb8d8a91b4ee37a25df60f5b8fc9b394` | `sora5ｻwﾓyRｿqﾏnMﾀﾙヰKoﾒﾇﾓQｺﾛyｼ3ｸFHB2F5LyPﾐTMZkｹｼw67ﾋVﾕｻr8ﾉGﾇeEnｻVRNKCS` |
| iroha | `0x03` | `0x0201de8b36819700c807083608e2000120ed4928c628d1c2c6eae90338905995612959273a5c63f93636c14614ac8737d1` | `sora5ｻﾜxﾀ7Vｱ7QFeｷMﾂLﾉﾃﾏﾓﾀTﾚgSav3Wnｱｵ4ｱCKｷﾛMﾘzヰHiﾐｱ6ﾃﾉﾁﾐZmﾇ2fiﾎX21P4L` |
| alfa | `0x04` | `0x020146be2154ae86826a3fef0ec0000120ca93ac1705187071d67b83c7ff0efe8108e8ec4530575d7726879333dbdabe7c` | `sora5ｻ9JヱﾈｿuwU6ｴpﾔﾂﾈRqRTds1HﾃﾐｶLVﾍｳ9ﾔhｾNｵVｷyucEﾒGﾈﾏﾍ9sKeﾉDzrｷﾆ742WG1` |
| omega | `0x05` | `0x0201390d946885bc8416b3d30c9d0001206e7a1cdd29b0b78fd13af4c5598feff4ef2a97166e3ca6f2e4fbfccd80505bf1` | `sora5ｻ3zrﾌuﾚﾄJﾑXQhｸTyN8pzwRkWxmjVﾗbﾚﾕヰﾈoｽｦｶtEEﾊﾐ6GPｿﾓﾊｾEhvPｾｻ3XAJ73F` |
| boshqaruv | `0x06` | `0x0201989eb45a80940d187e2c908f0001208a875fff1eb38451577acd5afee405456568dd7c89e090863a0557bc7af49f17` | `sora5ｻiｵﾁyVﾕｽbFpDHHuﾇﾉdﾗｲﾓﾄRﾋAW3frUCｾ5ｷﾘTwdﾚnｽtQiLﾏｼｶﾅXgｾZmﾒヱH58H4KP` |
| validatorlar | `0x07` | `0x0201e4ffa58704c69afaeb7cc2d7000120ea4a6c63e29c520abef5507b132ec5f9954776aebebe7b92421eea691446d22c` | `sora5ｻﾀLDH6VYﾑNAｾgﾉVﾜtxﾊRXLｹﾍﾔﾌLd93GﾔGeｴﾄYrs1ﾂHｸkYxｹwｿyZﾗxyﾎZoXT1S4N` |
| tadqiqotchi | `0x08` | `0x02013b35422c65c2a83c99c523ad0001201398f62c6d1a457c51ba6a4b5f3dbd2f69fca93216218dc8997e416bd17d93ca` | `sora5ｻ4nmｻaﾚﾚPvNLgｿｱv6MHDeEyﾀovﾉJcpvrﾖ6ﾈCQcCNﾇﾜhﾚﾖyFdTwｸｶHEｱ9rWU8FMB` |
| soranet | `0x09` | `0x0201047d9ea7f5d5dbec3f7bfc58000120fd1724385aa0c75b64fb78cd602fa1d991fdebf76b13c58ed702eac835e9f618` | `sora5ｱｸヱVQﾂcﾁヱRﾓcApｲﾁﾅﾒvﾌﾏfｾNnﾛRJsｿDhﾙuHaﾚｺｦﾌﾍﾈeﾆﾎｺN1UUDｶ6ﾎﾄﾛoRH8JUL` |
| kitsune | `0x0A` | `0x0201e91933de397fd7723dc9a76c00012043a72e714401762df66b68c26dfbdf2682aaec9f2474eca4613e424a0fbafd3c` | `sora5ｻﾚｺヱkfFJfSﾁｼJwﾉLvbpSｷﾔMWFMrbｳｸｲｲyヰKGJﾉｻ4ｹﾕrｽhｺｽzSDヰXAN62AD7RGNS` |
| da | `0x0B` | `0x02016838cf5bb0ce0f3d4f380e1c00012066be7e332c7a453332bd9d0a7f7db055f5c5ef1a06ada66d98b39fb6810c473a` | `sora5ｻNﾒ5SﾐRﾉﾐﾃ62ｿ1ｶｷWFKyF1BcAﾔvｼﾐHqﾙﾐPﾏｴヰ5tｲﾕvnﾙT6ﾀW7mﾔ7ﾇﾗﾂｳ25CXS93` |

Ko'rib chiqilgan: Ma'lumotlar modeli WG, Kriptografiya WG - ADDR-1a uchun tasdiqlangan.

##### Sora Nexus mos yozuvlar taxalluslari

Sora Nexus tarmoqlari sukut boʻyicha `chain_discriminant = 0x02F1`
(`iroha_config::parameters::defaults::common::CHAIN_DISCRIMINANT`). The
Shuning uchun `AccountAddress::to_i105` va `to_i105` yordamchilari chiqaradi
har bir kanonik foydali yuk uchun izchil matn shakllari. dan tanlangan armatura
`fixtures/account/address_vectors.json` (orqali yaratilgan
`cargo xtask address-vectors`) tez ma'lumot olish uchun quyida ko'rsatilgan:

| Hisob / selektor | I105 literal (prefiks `0x02F1`) | Sora siqilgan (`sora`) literal |
|--------------------------------|--------------------------------|----------------------------------|
| `default` domeni (yashirin selektor, `0x00`) | `6cmzPVPX5jDQFNfiz6KgmVfm1fhoAqjPhoPFn4nx9mBWaFMyUCwq4cw` | `sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE` (aniq marshrutlash bo'yicha maslahatlar berishda ixtiyoriy `@default` qo'shimchasi) |
| `treasury` (mahalliy digest selektori, `0x01` urug'i) | `34mSYnCXkCzHXm31UDHh7SJfGvC4QPEhwim8z7sys2iHqXpCwCQkjL8KHvkFLSs1vZdJcb37r` | `sora5ｻu6rﾀCヰTGwﾏ1ﾅヱﾌQｲﾖﾇqCｦヰﾓZQCZRDSSﾅMｱﾙヱｹﾁｸ8ｾeﾄﾛ6C8bZuwﾗｹCZｦRSLQFU` |
| Global registr ko'rsatkichi (`registry_id = 0x0000_002A`, `treasury` ekvivalenti) | `3oE9sLeRGP49Cu7mQ1nF4wtKAm29BG4TGLiRsaXe7mhbMP5WZ113nNW1N6RbqF` | `sorakXｹ6NｻﾍﾀﾖSﾜﾖｱ3ﾚ5WﾘﾋQﾅｷｦxgﾛｸcﾁｵﾋkﾋvﾏ8SPﾓﾀｹdｴｴｲW9iCM6AEP` |

Bu satrlar CLI (`iroha tools address convert`), Torii tomonidan chiqarilganlarga mos keladi
javoblar (`canonical I105 literal rendering`) va SDK yordamchilari, shuning uchun UX nusxalash/joylashtirish
oqimlar ularga so'zma-so'z tayanishi mumkin. `<address>@<domain>` (rejected legacy form)-ni faqat aniq marshrutlash maslahati kerak bo'lganda qo'shing; qo'shimchasi kanonik chiqishning bir qismi emas.

#### 2.6 O'zaro ishlash uchun matnli taxalluslar (rejalashtirilgan)

- **Zanjir taxallus uslubi:** jurnallar va insonlar uchun `ih:<chain-alias>:<alias@domain>`
  kirish. Hamyonlar prefiksni tahlil qilishi, o'rnatilgan zanjirni tekshirishi va bloklashi kerak
  mos kelmasligi.
- **CAIP-10 shakli:** zanjir-agnostik uchun `iroha:<caip-2-id>:<i105-addr>`
  integratsiyalar. Bu xaritalash jo‘natilayotganda **hali joriy etilmagan**
  asboblar zanjiri.
- **Mashina yordamchilari:** Rust, TypeScript/JavaScript, Python uchun kodeklarni nashr qilish,
  va Kotlin I105 va siqilgan formatlarni qamrab oladi (`AccountAddress::to_i105`,
  `AccountAddress::parse_encoded` va ularning SDK ekvivalentlari). CAIP-10 yordamchilari
  kelajakdagi ish.

#### 2.7 Deterministik I105 taxallus

- **Prefiks xaritasi:** `chain_discriminant` dan I105 tarmoq prefiksi sifatida qayta foydalaning.
  `encode_i105_prefix()` (qarang: `crates/iroha_data_model/src/account/address.rs`)
  `<64` qiymatlari uchun 6 bitli prefiks (bitta bayt) va 14 bitli ikki bayt chiqaradi
  Kattaroq tarmoqlar uchun shakl. Nufuzli topshiriqlar yashaydi
  [`address_prefix_registry.md`](source/references/address_prefix_registry.md);
  SDK'lar to'qnashuvlarni oldini olish uchun mos keladigan JSON registrini sinxronlashtirishi KERAK.
- **Hisob materiali:** I105 tomonidan yaratilgan kanonik foydali yukni kodlaydi
  `AccountAddress::canonical_bytes()` — sarlavha bayti, domen selektori va
  boshqaruvchining foydali yuki. Qo'shimcha xeshlash bosqichi yo'q; I105 ni o'rnatadi
  Rust tomonidan ishlab chiqarilgan ikkilik kontrollerning foydali yuki (bitta kalit yoki multisig).
  multisig siyosati dayjestlari uchun ishlatiladigan CTAP2 xaritasi emas, balki kodlovchi.
- **Kodlash:** `encode_i105()` prefiks baytlarini kanonik bayt bilan birlashtiradi
  foydali yuk va Blake2b-512 dan olingan 16 bitli nazorat summasini o'zgarmas bilan qo'shadi
  Prefix: `I105PRE` (`b"I105PRE"` || prefix || payload). The result is encoded via `bs58` using the I105 alphabet.
  CLI/SDK yordamchilari bir xil protsedurani ochib beradi va `AccountAddress::parse_encoded`
  uni `decode_i105` orqali o'zgartiradi.

#### 2.8 Normativ matnli test vektorlari

`fixtures/account/address_vectors.json` to'liq I105 (afzal) va siqilgan (`sora`, ikkinchi eng yaxshi) o'z ichiga oladi
har bir kanonik foydali yuk uchun harflar. Diqqatga sazovor joylar:

- **`addr-single-default-ed25519` (Sora Nexus, `0x02F1` prefiksi).**  
  I105 `6cmzPVPX5jDQFNfiz6KgmVfm1fhoAqjPhoPFn4nx9mBWaFMyUCwq4cw`, siqilgan (`sora`)
  `sora2QG…U4N5E5`. Torii bu aniq satrlarni `AccountId` dan chiqaradi
  `Display` amalga oshirish (kanonik I105) va `AccountAddress::to_i105`.
- **`addr-global-registry-002a` (reestr selektori → xazina).**  
  I105 `3oE9sLeRGP49Cu7mQ1nF4wtKAm29BG4TGLiRsaXe7mhbMP5WZ113nNW1N6RbqF`, siqilgan (`sora`)
  `sorakX…CM6AEP`. Ro'yxatga olish kitobi selektorlari hali ham dekodlashini ko'rsatadi
  mos keladigan mahalliy dayjest bilan bir xil kanonik foydali yuk.
- **Muvaffaqiyatsizlik holati (`i105-prefix-mismatch`).**  
  Tugunda `NETWORK_PREFIX + 1` prefiksi bilan kodlangan I105 literalini tahlil qilish
  standart prefiks hosil bo'lishini kutish
  `AccountAddressError::UnexpectedNetworkPrefix { expected: 753, found: 754 }`
  domenni marshrutlashdan oldin. `i105-checksum-mismatch` moslamasi
  Blake2b nazorat summasi orqali buzilishlarni aniqlash mashqlari.

#### 2.9 Muvofiqlik moslamalari

ADDR‑2 ijobiy va salbiyni o‘z ichiga olgan qayta o‘ynaladigan moslamalar to‘plamini jo‘natadi
kanonik olti burchakli stsenariylar, I105 (afzal), siqilgan (`sora`, yarim/toʻliq kenglik), yashirin
standart selektorlar, global ro'yxatga olish kitobi taxalluslari va multisignature kontrollerlari. The
kanonik JSON `fixtures/account/address_vectors.json` da yashaydi va bo'lishi mumkin
bilan qayta tiklangan:

```
cargo xtask address-vectors --out fixtures/account/address_vectors.json
# verify without writing:
cargo xtask address-vectors --verify
```

Ad-hoc tajribalar uchun (turli yo'llar/formatlar) ikkilik misol hali ham mavjud
mavjud:

```
cargo run -p iroha_data_model --example account_address_vectors > fixtures/account/address_vectors.json
```

`crates/iroha_data_model/tests/account_address_vectors.rs` da zang birligi sinovlari
va `crates/iroha_torii/tests/account_address_vectors.rs`, JS bilan birga,
Swift va Android qurilmalari (`javascript/iroha_js/test/address.test.js`,
`IrohaSwift/Tests/IrohaSwiftTests/AccountAddressTests.swift`,
`java/iroha_android/src/test/java/org/hyperledger/iroha/android/address/AccountAddressTests.java`),
SDK va Torii qabulida kodek paritetini kafolatlash uchun bir xil moslamadan foydalaning.

### 3. Global noyob domenlar va normallashtirish

Shuningdek qarang: [`docs/source/references/address_norm_v1.md`](source/references/address_norm_v1.md)
Torii, maʼlumotlar modeli va SDK’larda ishlatiladigan kanonik Norm v1 quvur liniyasi uchun.

`DomainId`ni tegli kortej sifatida qayta belgilang:

```
DomainId {
    name: Name,
    authority: GlobalDomainAuthority, // new enum
}

enum GlobalDomainAuthority {
    LocalChain,                  // default for the local chain
    External { chain_discriminant: u16 },
}
```

`LocalChain` joriy zanjir tomonidan boshqariladigan domenlar uchun mavjud nomni o'rab oladi.
Domen global reestr orqali ro'yxatdan o'tganda, biz egalik qilishda davom etamiz
zanjirning diskriminanti. Displey/tahlil hozircha o'zgarishsiz qoladi, lekin
kengaytirilgan tuzilma marshrutlash qarorlarini qabul qilishga imkon beradi.

#### 3.1 Normalizatsiya va firibgarlikdan himoya qilish

Norm v1 har bir komponent domendan oldin foydalanishi kerak bo‘lgan kanonik quvur liniyasini belgilaydi
nomi saqlanib qoladi yoki `AccountAddress` ichiga kiritiladi. To'liq ko'rsatma
[`docs/source/references/address_norm_v1.md`](source/references/address_norm_v1.md) da yashaydi;
Quyidagi xulosa hamyonlar, Torii, SDKlar va boshqaruv bosqichlarini qamrab oladi
vositalarini amalga oshirishi kerak.

1. **Kirishni tekshirish.** Boʻsh satrlarni, boʻshliqlarni va zahiradagilarni rad qilish
   ajratuvchilar `@`, `#`, `$`. Bu tomonidan amalga oshirilgan invariantlarga mos keladi
   `Name::validate_str`.
2. **Unicode NFC kompozitsiyasi.** ICU tomonidan qo‘llab-quvvatlanadigan NFC normalizatsiyasini kanonik tarzda qo‘llang
   ekvivalent ketma-ketliklar deterministik tarzda qulab tushadi (masalan, `e\u{0301}` → `é`).
3. **UTS-46 normalizatsiyasi.** NFC chiqishini UTS‑46 orqali ishga tushiring.
   `use_std3_ascii_rules = true`, `transitional_processing = false` va
   DNS uzunligini qo'llash yoqilgan. Natijada kichik harfli A yorlig'i ketma-ketligi;
   STD3 qoidalarini buzadigan kirishlar bu erda muvaffaqiyatsizlikka uchraydi.
4. **Uzunlik chegaralari.** DNS uslubidagi chegaralarni qo‘llash: har bir yorliq 1–63 gacha bo‘lishi KERAK
   bayt va toʻliq domen 3-qadamdan keyin 255 baytdan KESHMADI.
5. **Ixtiyoriy chalkashlik siyosati.** UTS‑39 skript tekshiruvlari quyidagilar uchun kuzatiladi
   Norm v2; operatorlar ularni erta yoqishlari mumkin, ammo agar tekshiruv muvaffaqiyatsiz tugasa, uni bekor qilish kerak
   qayta ishlash.

Har bir bosqich muvaffaqiyatli bo'lsa, kichik A-yorlig'i satri keshlanadi va uchun ishlatiladi
manzil kodlash, konfiguratsiya, manifestlar va registrlarni qidirish. Mahalliy dayjest
selektorlar o'zlarining 12 baytlik qiymatini `blake2s_mac(key = "SORA-LOCAL-K:v1",
canonical_label)[0..12]` 3-qadam chiqishi yordamida. Boshqa barcha urinishlar (aralash
katta harf, raw Unicode kiritish) tuzilgan bilan rad etiladi
`ParseError`s nom berilgan chegarada.

Ushbu qoidalarni ko'rsatadigan kanonik moslamalar, jumladan, punycode bo'ylab sayohatlar
va noto'g'ri STD3 ketma-ketliklari - ro'yxatda keltirilgan
`docs/source/references/address_norm_v1.md` va SDK CI da aks ettirilgan
ADDR‑2 ostida kuzatilgan vektor to'plamlari.

### 4. Nexus domen registrlari va marshrutlash- **Ro‘yxatga olish sxemasi:** Nexus imzolangan `DomainName -> ChainRecord` xaritasini saqlaydi
  bu erda `ChainRecord` zanjir diskriminantini, ixtiyoriy metama'lumotlarni (RPC) o'z ichiga oladi
  so'nggi nuqtalar) va vakolatni tasdiqlovchi hujjat (masalan, boshqaruvning ko'p imzosi).
- **Sinxronlash mexanizmi:**
  - Zanjirlar imzolangan domen da'volarini Nexus ga yuboradi (genezis paytida yoki orqali
    boshqaruv yo'riqnomasi).
  - Nexus davriy manifestlarni nashr etadi (imzolangan JSON va ixtiyoriy Merkle ildizi)
    HTTPS va kontent-manzilli saqlash (masalan, IPFS) orqali. Mijozlar pin qiladi
    oxirgi manifest va imzolarni tekshirish.
- **Izlash oqimi:**
  - Torii `DomainId` ga havola qilingan tranzaksiyani oladi.
  - Agar domen mahalliy darajada noma'lum bo'lsa, Torii keshlangan Nexus manifestini so'raydi.
  - Agar manifest xorijiy zanjirni ko'rsatsa, tranzaktsiya rad etiladi
    deterministik `ForeignDomain` xatosi va masofaviy zanjir ma'lumotlari.
  - Agar domen Nexus da etishmayotgan bo'lsa, Torii `UnknownDomain` ni qaytaradi.
- **Ishonch langarlari va aylanish:** Boshqaruv kalitlari belgisi manifestlari; aylanish yoki
  bekor qilish yangi manifest yozuvi sifatida e'lon qilinadi. Mijozlar manifestni amalga oshiradi
  TTL (masalan, 24 soat) va ushbu oynadan tashqari eskirgan ma'lumotlar bilan maslahatlashishdan bosh torting.
- **Muvaffaqiyatsizlik rejimlari:** Agar manifestni qidirish muvaffaqiyatsiz bo'lsa, Torii keshlangan holatga qaytadi.
  TTL ichidagi ma'lumotlar; o'tgan TTL u `RegistryUnavailable` chiqaradi va rad etadi
  nomuvofiq holatni oldini olish uchun domenlararo marshrutlash.

### 4.1 Ro'yxatga olish kitobining o'zgarmasligi, taxalluslari va qabr toshlari (ADDR-7c)

Nexus har bir domen yoki taxallusni tayinlash uchun **faqat qo'shish manifestini** nashr etadi
tekshirilishi va takrorlanishi mumkin. Operatorlar ushbu bo'limda tasvirlangan to'plamni davolashlari kerak
[manifest runbook](source/runbooks/address_manifest_ops.md) sifatida
haqiqatning yagona manbai: agar manifest etishmayotgan bo'lsa yoki tasdiqlanmasa, Torii kerak
ta'sirlangan domenni hal qilishni rad etish.

Avtomatlashtirishni qo'llab-quvvatlash: `cargo xtask address-manifest verify --bundle <current_dir> --previous <previous_dir>`
da yozilgan nazorat summasi, sxema va oldingi dayjest tekshiruvlarini takrorlaydi
runbook. `sequence` ni ko'rsatish uchun o'zgartirish chiptalariga buyruq chiqishini qo'shing
va `previous_digest` aloqasi to'plamni nashr etishdan oldin tasdiqlangan.

#### Manifest sarlavhasi va imzo shartnomasi

| Maydon | Talab |
|-------|-------------|
| `version` | Hozirda `1`. Faqat mos keladigan spetsifikatsiya yangilanishi bilan zarba bering. |
| `sequence` | Har bir nashr uchun **aynan** bittaga oshirish. Torii keshlari bo'shliqlar yoki regressiyalar bilan qayta ko'rib chiqishni rad etadi. |
| `generated_ms` + `ttl_hours` | Keshning yangiligini o'rnating (standart 24 soat). Agar TTL keyingi nashrdan oldin tugasa, Torii `RegistryUnavailable` ga o'tadi. |
| `previous_digest` | Oldingi manifest tananing BLAKE3 hazmi (hex). Tekshiruvchilar o'zgarmasligini isbotlash uchun uni `b3sum` bilan qayta hisoblaydilar. |
| `signatures` | Manifestlar Sigstore (`cosign sign-blob`) orqali imzolanadi. Ops `cosign verify-blob --bundle manifest.sigstore manifest.json` ni ishga tushirishi va ishga tushirishdan oldin boshqaruv identifikatori/emitent cheklovlarini tatbiq etishi kerak. |

Chiqarish avtomatizatsiyasi `manifest.sigstore` va `checksums.sha256` chiqaradi
JSON tanasi bilan birga. SoraFS yoki aks ettirishda fayllarni birga saqlang
Auditorlar tekshirish bosqichlarini so'zma-so'z takrorlashlari uchun HTTP so'nggi nuqtalari.

#### Kirish turlari

| Tur | Maqsad | Majburiy maydonlar |
|------|---------|-----------------|
| `global_domain` | Domen global miqyosda ro'yxatdan o'tganligini va zanjir diskriminantiga va I105 prefiksiga mos kelishi kerakligini e'lon qiladi. | `{ "domain": "<label>", "chain": "sora:nexus:global", "i105_prefix": 753, "selector": "global" }` |
| `tombstone` | Taxallus/selektorni doimiy ravishda bekor qiladi. Local‑8 dayjestlarini o‘chirish yoki domenni o‘chirishda talab qilinadi. | `{ "selector": {…}, "reason_code": "LOCAL8_RETIREMENT" \| …, "ticket": "<governance id>", "replaces_sequence": <number> }` |

`global_domain` yozuvlari ixtiyoriy ravishda `manifest_url` yoki `sorafs_cid` ni oʻz ichiga olishi mumkin.
hamyonlarni imzolangan zanjir metama'lumotlariga yo'naltirish, ammo kanonik kortej saqlanib qoladi
`{domain, chain, discriminant/i105_prefix}`. `tombstone` yozuvlari **iqtibos keltirishi shart**
nafaqaga chiqqan selektor va ruxsat bergan chipta/boshqaruv artefakti
Audit izi oflayn rejimda qayta tiklanadigan bo'lishi uchun o'zgartirish.

#### Taxallus/qabr toshining ish jarayoni va telemetriya

1. **Driftni aniqlang.** `torii_address_local8_total{endpoint}` dan foydalaning,
   `torii_address_local8_domain_total{endpoint,domain}`,
   `torii_address_collision_total{endpoint,kind="local12_digest"}`,
   `torii_address_collision_domain_total{endpoint,domain}`,
   `torii_address_domain_total{endpoint,domain_kind}`, va
   `torii_address_invalid_total{endpoint,reason}` (ko'rsatilgan
   `dashboards/grafana/address_ingest.json`) Mahalliy taqdimotlarni tasdiqlash va
   Mahalliy-12 to'qnashuvi qabr toshini taklif qilishdan oldin nol bo'lib qoladi. The
   har bir domen hisoblagichlari egalariga faqat ishlab chiquvchi/sinov domenlari Local‑8 chiqarishini isbotlash imkonini beradi
   trafik (va o'sha Mahalliy‑12 to'qnashuvlari ma'lum staging domenlariga xaritasi) esa
   **Domain turi aralashmasi (5m)** panelini o'z ichiga oladi, shuning uchun SRElar qancha miqdorni grafikalashi mumkin
   `domain_kind="local12"` trafigi qoladi va `AddressLocal12Traffic`
   ga qaramay, ishlab chiqarish hali ham Local-12 selektorlarini ko'rsa, ogohlantirish yong'inga chiqadi
   pensiya eshigi.
2. **Kanonik dayjestlarni chiqaring.** Yugurish
   `iroha tools address convert <address> --format json --expect-prefix 753`
   (yoki `fixtures/account/address_vectors.json` orqali
   `scripts/account_fixture_helper.py`) aniq `digest_hex` ni olish uchun.
   CLI I105, `i105` va kanonik `0x…` harflarini qabul qiladi; qo'shish
   `@<domain>` faqat manifestlar uchun yorliqni saqlash kerak bo'lganda.
   JSON xulosasi ushbu domenni `input_domain` maydoni orqali ko'rsatadi va
   `legacy  suffix` konvertatsiya qilingan kodlashni `<address>@<domain>` (rejected legacy form) sifatida takrorlaydi.
   manifest farqlari (bu qo'shimcha kanonik hisob identifikatori emas, balki metadata).
   Yangi qatorga yo'naltirilgan eksport uchun foydalaning
   Mahalliyni ommaviy konvertatsiya qilish uchun `iroha tools address normalize --input <file> legacy-selector input mode`
   selektorlarni kanonik I105 (afzal), siqilgan (`sora`, ikkinchi eng yaxshi), olti burchakli yoki JSON shakllariga kiriting.
   mahalliy bo'lmagan qatorlar. Agar auditorlarga elektron jadvalga mos dalillar kerak bo'lsa, ishga tushiring
   CSV xulosasini chiqarish uchun `iroha tools address audit --input <file> --format csv`
   (`input,status,format,domain_kind,…`), bu mahalliy selektorlarni ta'kidlaydi,
   kanonik kodlashlar va bir xil fayldagi xatolarni tahlil qilish.
3. **Manifest yozuvlarini qo‘shing.** `tombstone` yozuvini (va keyingi yozuvni) loyihalash
   Global registrga o'tish paytida `global_domain` yozuvi) va tasdiqlang
   imzo so'rashdan oldin `cargo xtask address-vectors` bilan manifest.
4. **Tasdiqlang va chop eting.** Runbook nazorat roʻyxatiga amal qiling (xeshlar, Sigstore,
   ketma-ketlik monotonligi) to'plamni SoraFS ga aks ettirishdan oldin. Torii hozir
   To'plam tushganidan so'ng darhol I105 (afzal)/sora (ikkinchi-eng yaxshi) harflarni kanoniklashtiradi.
5. **Monitoring va orqaga qaytarish.** Local‑8 va Local‑12 to‘qnashuv panellarini quyidagi holatda saqlang
   30 kun davomida nol; agar regressiyalar paydo bo'lsa, oldingi manifestni qayta nashr eting
   telemetriya barqarorlashgunga qadar faqat ta'sirlangan ishlab chiqarish bo'lmagan muhitda.

Yuqoridagi barcha qadamlar ADDR‑7c uchun majburiy dalildir: namoyon bo'lmasdan
`cosign` imzo to'plami yoki `previous_digest` qiymatlariga mos kelmasligi kerak
avtomatik ravishda rad etiladi va operatorlar tekshirish jurnallarini biriktirishi kerak
ularning almashtirish chiptalari.

### 5. Hamyon va API ergonomikasi

- **Birlamchi displey parametrlari:** Hamyonlar I105 manzilini ko‘rsatadi (qisqa, nazorat summasi)
  Bundan tashqari, registrdan olingan yorliq sifatida hal qilingan domen. Domenlar
  o'zgarishi mumkin bo'lgan tavsiflovchi metama'lumotlar sifatida aniq belgilangan, I105 esa
  barqaror manzil.
- **Kirishni kanoniklashtirish:** Torii va SDKlar I105 (afzal)/sora (ikkinchi-eng yaxshi)/0x ni qabul qiladi
  manzillar plus `alias@domain` (rejected legacy form), `uaid:…` va
  `opaque:…` shakllari, keyin chiqish uchun I105 ga kanoniklashtiriladi. yo'q
  qat'iy rejimni almashtirish; xom telefon/elektron pochta identifikatorlari kitobdan tashqarida saqlanishi kerak
  UAID/shaffof xaritalar orqali.
- **Xatolarning oldini olish:** Hamyonlar I105 prefikslarini tahlil qiladi va zanjir diskriminantini qo'llaydi
  umidlar. Zanjirning nomuvofiqligi diagnostika yordamida jiddiy nosozliklarni keltirib chiqaradi.
- **Codec kutubxonalari: ** Rasmiy Rust, TypeScript/JavaScript, Python va Kotlin
  kutubxonalar I105 kodlash/dekodlash va siqilgan (`sora`) yordamini taqdim etadi.
  qismlarga bo'lingan ilovalardan qoching. CAIP-10 konvertatsiyalari hali yuborilmagan.

#### Foydalanish imkoniyati va xavfsiz almashish boʻyicha yoʻriqnoma- Mahsulot yuzalarini amalga oshirish bo'yicha ko'rsatmalar jonli ravishda kuzatib boriladi
  `docs/portal/docs/reference/address-safety.md`; qachon ushbu nazorat ro'yxatiga murojaat qiling
  ushbu talablarni hamyon yoki Explorer UX ga moslashtirish.
- **Xavfsiz almashish oqimlari:** Manzillardan nusxa ko‘chiradigan yoki ko‘rsatadigan yuzalar standart I105 shakliga o‘rnatiladi va foydalanuvchilar nazorat summasini vizual yoki skanerlash orqali tekshirishlari uchun to‘liq qatorni va bir xil foydali yukdan olingan QR kodni taqdim etuvchi qo‘shni “ulashish” amalini ko‘rsatadi. Kesishning oldini olish mumkin bo'lmaganda (masalan, kichik ekranlar), satrning boshi va oxirini saqlang, aniq ellipslar qo'shing va tasodifiy qirqishning oldini olish uchun to'liq manzilni clipboardga nusxalash orqali kirish mumkin bo'lgan holda saqlang.
- **IME himoyasi:** Manzilli kirishlar IME/IME uslubidagi klaviaturalardan kompozitsiya artefaktlarini rad etishi KERAK. Faqat ASCII yozuvini qo'llang, to'liq kenglik yoki Kana belgilari aniqlanganda qatorli ogohlantirishni taqdim eting va tekshirishdan oldin belgilarni birlashtirgan tekis matn joylashtirish zonasini taklif qiling, shuning uchun yapon va xitoy foydalanuvchilar o'z IME-ni progressni yo'qotmasdan o'chirib qo'yishlari mumkin.
- **Ekranni o'qishni qo'llab-quvvatlash:** Asosiy I105 prefiks raqamlarini tavsiflovchi va I105 foydali yukini 4 yoki 8 ta belgidan iborat guruhlarga bo'ladigan vizual tarzda yashirin yorliqlarni (`aria-label`/I18NI0000475X) taqdim eting, shuning uchun guruhli belgilar qatorini o'qish uchun yordamchi texnologiyalar o'qiydi. Muloyim jonli hududlar orqali nusxa ko‘chirish/ulashish muvaffaqiyatini e’lon qiling va QR oldindan ko‘rishda tavsiflovchi alternativ matnni (“0x02F1 zanjiridagi <taxallus” uchun I105 manzili”) o‘z ichiga oladi.
- **Faqat Sora uchun siqilgan foydalanish:** Har doim `i105` siqilgan ko‘rinishini “Faqat Sora” deb belgilang va nusxa ko‘chirishdan oldin uni aniq tasdiqdan o‘tkazing. SDK va hamyonlar zanjir diskriminanti Sora Nexus qiymati bo'lmasa, siqilgan mahsulotni ko'rsatishdan bosh tortishi va mablag'larni noto'g'ri yo'naltirishning oldini olish uchun foydalanuvchilarni tarmoqlararo o'tkazmalar uchun I105 ga qaytarishi kerak.

## Amalga oshirishni tekshirish ro'yxati

- **I105 konvert:** Prefiks `chain_discriminant` ni kompakt yordamida kodlaydi
  `encode_i105_prefix()` dan 6-/14-bitli sxema, korpus kanonik baytlardir
  (`AccountAddress::canonical_bytes()`) va nazorat summasi birinchi ikki baytdir
  Blake2b-512(`b"I105PRE"` || prefix || body). The full payload is encoded via `bs58` using the I105 alphabet.
- **Ro‘yxatga olish shartnomasi:** Imzolangan JSON (va ixtiyoriy Merkle root) nashriyot
  `{discriminant, i105_prefix, chain_alias, endpoints}` 24 soatlik TTL va
  aylanish tugmachalari.
- **Domen siyosati:** ASCII `Name` bugun; i18n yoqilsa, UTS-46 ni qo'llang
  normallashtirish va chalkash tekshiruvlar uchun UTS-39. Maksimal yorliqni (63) va
  jami (255) uzunlik.
- **Matn yordamchilari:** Rustda I105 ↔ siqilgan (`i105`) kodeklarini jo'natish,
  Umumiy test vektorlari bilan TypeScript/JavaScript, Python va Kotlin (CAIP-10
  xaritalash kelajakdagi ish bo'lib qoladi).
- **CLI asboblari:** `iroha tools address convert` orqali deterministik operator ish oqimini taqdim eting
  (qarang `crates/iroha_cli/src/address.rs`), u I105/`0x…` harflarini qabul qiladi va
  ixtiyoriy `<address>@<domain>` (rejected legacy form) yorliqlari, Sora Nexus (`753`) prefiksi yordamida I105 chiqishi standarti,
  va faqat operatorlar aniq so'raganda Sora-faqat siqilgan alifboni chiqaradi
  `--format i105` yoki JSON xulosa rejimi. Buyruq prefiks kutishlarini ishga tushiradi
  tahlil qilish, taqdim etilgan domenni (JSONda `input_domain`) va `legacy  suffix` bayrog'ini yozib oladi
  aylantirilgan kodlashni `<address>@<domain>` (rejected legacy form) sifatida takrorlaydi, shuning uchun manifest farqlar ergonomik bo'lib qoladi.
- **Wallet/explorer UX:** [manzilni ko‘rsatish yo‘riqnomalariga] (source/sns/address_display_guidelines.md) rioya qiling
  ADDR-6 bilan jo‘natilgan — ikki nusxadagi tugmalarni taklif eting, I105 ni QR yuki sifatida saqlang va ogohlantiring
  siqilgan `i105` shakli faqat Sora va IME qayta yozishga moyil bo'lgan foydalanuvchilar.
- **Torii integratsiyasi:** Nexus keshi TTLga nisbatan namoyon bo'ladi, chiqaradi
  `ForeignDomain`/`UnknownDomain`/`RegistryUnavailable` aniq va
  keep strict account-literal parsing canonical-I105-only (reject compressed and any `@domain` suffix) with canonical I105 output.

### Torii javob formatlari

- `GET /v1/accounts` ixtiyoriy `canonical I105 rendering` so'rov parametrini qabul qiladi va
  `POST /v1/accounts/query` JSON konvertidagi bir xil maydonni qabul qiladi.
  Qo'llab-quvvatlanadigan qiymatlar:
  - `i105` (standart) — javoblar kanonik I105 foydali yuklarini chiqaradi (masalan,
    `6cmzPVPX5jDQFNfiz6KgmVfm1fhoAqjPhoPFn4nx9mBWaFMyUCwq4cw`).
  - `i105_default` - javoblar faqat Sora uchun `i105` siqilgan ko'rinishini chiqaradi.
    filtrlarni/yo'l parametrlarini kanonik saqlash.
- Yaroqsiz qiymatlar `400` (`QueryExecutionFail::Conversion`) qaytaradi. Bu imkon beradi
  hamyonlar va tadqiqotchilar esa Sora-faqat UX uchun siqilgan satrlarni so'rashlari mumkin
  I105 ni o'zaro ishlaydigan standart sifatida saqlash.
- Aktiv egalari ro'yxati (`GET /v1/assets/{definition_id}/holders`) va ularning JSON
  konvert hamkasbi (`POST …/holders/query`) ham `canonical I105 rendering` ni hurmat qiladi.
  `items[*].account_id` maydoni har doim siqilgan harflarni chiqaradi
  parametr/konvert maydoni hisoblarni aks ettiruvchi `i105_default` ga o'rnatildi
  tadqiqotchilar kataloglar bo'ylab izchil natijalarni taqdim etishlari uchun so'nggi nuqtalar.
- **Sinov:** Enkoder/dekoder bo'ylab sayohatlar, noto'g'ri zanjirlar uchun birlik testlarini qo'shing
  nosozliklar va manifest qidiruvlar; Torii va SDK-larda integratsiya qamrovini qo'shing
  I105 oqimlari uchun oxirigacha.

## Xato kodi reestri

Manzil enkoderlari va dekoderlari nosozliklarni aniqlaydi
`AccountAddressError::code_str()`. Quyidagi jadvallar barqaror kodlarni taqdim etadi
SDK, hamyonlar va Torii sirtlari odam o'qishi mumkin
xabarlar va tavsiya etilgan tuzatish bo'yicha ko'rsatmalar.

### Kanonik qurilish

| Kod | Muvaffaqiyatsizlik | Tavsiya etilgan tuzatish |
|------|---------|-------------------------|
| `ERR_UNSUPPORTED_ALGORITHM` | Kodlovchi ro'yxatga olish kitobi yoki qurish xususiyatlari tomonidan qo'llab-quvvatlanmaydigan imzolash algoritmini oldi. | Hisobni qurishni registrda va konfiguratsiyada yoqilgan egri chiziqlar bilan cheklang. |
| `ERR_KEY_PAYLOAD_TOO_LONG` | Imzolash kalitining yuklanish uzunligi qoʻllab-quvvatlanadigan chegaradan oshib ketdi. | Bir kalitli kontrollerlar `u8` uzunliklari bilan cheklangan; katta ochiq kalitlar uchun multisig-dan foydalaning (masalan, ML-DSA). |
| `ERR_INVALID_HEADER_VERSION` | Manzil sarlavhasi versiyasi qo'llab-quvvatlanadigan diapazondan tashqarida. | V1 manzillari uchun sarlavha `0` versiyasini emit; yangi versiyalarni qabul qilishdan oldin kodlovchilarni yangilang. |
| `ERR_INVALID_NORM_VERSION` | Normalizatsiya versiyasi bayrog'i tan olinmadi. | `1` normalizatsiya versiyasidan foydalaning va zaxiralangan bitlarni almashtirishdan saqlaning. |
| `ERR_INVALID_I105_PREFIX` | Soʻralgan I105 tarmoq prefiksini kodlab boʻlmaydi. | Zanjir registrida chop etilgan inklyuziv `0..=16383` diapazonidan prefiksni tanlang. |
| `ERR_CANONICAL_HASH_FAILURE` | Kanonik foydali yuk xeshlash amalga oshmadi. | Operatsiyani qaytadan sinab ko'ring; agar xatolik davom etsa, uni xeshlash stekidagi ichki xato sifatida ko'ring. |

### Formatni dekodlash va avtomatik aniqlash

| Kod | Muvaffaqiyatsizlik | Tavsiya etilgan tuzatish |
|------|---------|-------------------------|
| `ERR_INVALID_I105_ENCODING` | I105 satrida alifbodan tashqari belgilar mavjud. | Manzil nashr etilgan I105 alifbosidan foydalanishiga va nusxa ko‘chirish/joylashtirish vaqtida kesilmaganligiga ishonch hosil qiling. |
| `ERR_INVALID_LENGTH` | Yuk yukining uzunligi selektor/kontroller uchun kutilgan kanonik o‘lchamga mos kelmaydi. | Tanlangan domen selektori va boshqaruvchi joylashuvi uchun to‘liq kanonik foydali yukni taqdim eting. |
| `ERR_CHECKSUM_MISMATCH` | I105 (afzal) yoki siqilgan (`sora`, ikkinchi eng yaxshi) nazorat summasini tekshirish amalga oshmadi. | Ishonchli manbadan manzilni qayta tiklang; bu odatda nusxa ko'chirish/joylashtirish xatosini bildiradi. |
| `ERR_INVALID_I105_PREFIX_ENCODING` | I105 prefiks baytlari noto'g'ri tuzilgan. | Muvofiq kodlovchi bilan manzilni qayta kodlash; etakchi I105 baytlarini qo'lda o'zgartirmang. |
| `ERR_INVALID_HEX_ADDRESS` | Kanonik oʻn oltilik shaklni dekodlab boʻlmadi. | Rasmiy kodlovchi tomonidan ishlab chiqarilgan `0x` prefiksli, teng uzunlikdagi olti burchakli qatorni taqdim eting. |
| `ERR_MISSING_COMPRESSED_SENTINEL` | Siqilgan shakl `sora` bilan boshlanmaydi. | Siqilgan Sora manzillarini dekoderlarga topshirishdan oldin kerakli qo'riqchi bilan prefiks qo'ying. |
| `ERR_COMPRESSED_TOO_SHORT` | Siqilgan satrda foydali yuk va nazorat summasi uchun etarli raqamlar yo'q. | Kesilgan parchalar o'rniga kodlovchi tomonidan chiqarilgan to'liq siqilgan satrdan foydalaning. |
| `ERR_INVALID_COMPRESSED_CHAR` | Siqilgan alifbodan tashqaridagi belgi topildi. | Belgini chop etilgan yarim kenglik/toʻliq kenglik jadvallaridagi haqiqiy Base‑105 glifi bilan almashtiring. |
| `ERR_INVALID_COMPRESSED_BASE` | Kodlovchi qoʻllab-quvvatlanmaydigan radiksdan foydalanishga harakat qildi. | Kodlovchiga qarshi xatoni yozing; siqilgan alifbo V1 da radix 105 ga o'rnatiladi. |
| `ERR_INVALID_COMPRESSED_DIGIT` | Raqam qiymati siqilgan alifbo hajmidan oshib ketdi. | Har bir raqam `0..105)` ichida ekanligiga ishonch hosil qiling, agar kerak bo'lsa, manzilni qayta tiklang. |
| `ERR_UNSUPPORTED_ADDRESS_FORMAT` | Avtomatik aniqlash kiritish formatini taniy olmadi. | Tahlil qiluvchilarni chaqirishda I105 (afzal), siqilgan (`sora`) yoki kanonik `0x` olti burchakli satrlarni taqdim eting. |

### Domen va tarmoqni tekshirish| Kod | Muvaffaqiyatsizlik | Tavsiya etilgan tuzatish |
|------|---------|-------------------------|
| `ERR_DOMAIN_MISMATCH` | Domen selektori kutilgan domenga mos kelmaydi. | Mo'ljallangan domen uchun berilgan manzildan foydalaning yoki taxminni yangilang. |
| `ERR_INVALID_DOMAIN_LABEL` | Domen yorlig‘i normalizatsiya tekshirilmadi. | Kodlashdan oldin UTS-46 o'tishsiz ishlov berishdan foydalanib domenni kanoniklashtiring. |
| `ERR_UNEXPECTED_NETWORK_PREFIX` | Dekodlangan I105 tarmoq prefiksi sozlangan qiymatdan farq qiladi. | Maqsadli zanjirdan manzilga o'ting yoki kutilgan diskriminant/prefiksni sozlang. |
| `ERR_UNKNOWN_ADDRESS_CLASS` | Manzil sinfi bitlari tan olinmaydi. | Dekoderni yangi sinfni tushunadigan versiyaga yangilang yoki sarlavha bitlarini buzishdan saqlaning. |
| `ERR_UNKNOWN_DOMAIN_TAG` | Domen selektor yorlig'i noma'lum. | Yangi selektor turini qo‘llab-quvvatlaydigan versiyaga yangilang yoki V1 tugunlarida eksperimental foydali yuklardan foydalanmang. |
| `ERR_UNEXPECTED_EXTENSION_FLAG` | Zaxiralangan kengaytma biti o'rnatildi. | Zaxiralangan bitlarni tozalash; kelajakdagi ABI ularni tanishtirmaguncha ular yopiq qoladilar. |
| `ERR_UNKNOWN_CONTROLLER_TAG` | Nazoratchining foydali yuk yorlig'i tan olinmadi. | Yangi kontroller turlarini tahlil qilishdan oldin ularni tanib olish uchun dekoderni yangilang. |
| `ERR_UNEXPECTED_TRAILING_BYTES` | Kanonik foydali yuk dekodlashdan keyin keyingi baytlarni o'z ichiga oladi. | Kanonik foydali yukni qayta tiklash; faqat hujjatlashtirilgan uzunlik mavjud bo'lishi kerak. |

### Controller Payload Validation

| Kod | Muvaffaqiyatsizlik | Tavsiya etilgan tuzatish |
|------|---------|-------------------------|
| `ERR_INVALID_PUBLIC_KEY` | Kalit baytlari e'lon qilingan egri chiziqqa mos kelmaydi. | Kalit baytlari tanlangan egri chiziq uchun kerakli tarzda kodlanganligiga ishonch hosil qiling (masalan, 32 bayt Ed25519). |
| `ERR_UNKNOWN_CURVE` | Egri chiziq identifikatori qayd etilmagan. | Qo'shimcha egri chiziqlar tasdiqlanmaguncha va reestrda e'lon qilinmaguncha `1` (Ed25519) egri chizig'idan foydalaning. |
| `ERR_MULTISIG_MEMBER_OVERFLOW` | Multisig kontrolleri qo'llab-quvvatlanadigandan ko'proq a'zolar e'lon qiladi. | Kodlashdan oldin multisig a'zoligini hujjatlashtirilgan chegaraga kamaytiring. |
| `ERR_INVALID_MULTISIG_POLICY` | Multisig siyosatining foydali yukini tekshirish muvaffaqiyatsiz tugadi (eshik/vazn/sxema). | Siyosatni CTAP2 sxemasi, vazn chegaralari va chegara cheklovlariga javob beradigan tarzda qayta yarating. |

## Muqobil variantlar ko'rib chiqildi

- **Pure checksum envelope (Bitcoin-uslubi).** Soddaroq nazorat summasi, ammo xatolarni aniqlash zaifroq
  Blake2b tomonidan olingan I105 nazorat summasidan (`encode_i105` 512 bitli xeshni qisqartiradi)
  va 16 bitli diskriminantlar uchun aniq prefiks semantikasi yo'q.
- **Domen qatoriga zanjir nomini kiritish (masalan, `finance@chain`).** Tanaffuslar
- **Manzillarni o'zgartirmasdan faqat Nexus marshrutiga ishoning.** Foydalanuvchilar hali ham
  noaniq satrlarni nusxalash/joylashtirish; biz manzilning o'zi kontekstga ega bo'lishini xohlaymiz.
- **Bech32m konvert.** QR-do'st va odam o'qiy oladigan prefiksni taklif qiladi, lekin
  I105 yuk tashishdan farq qiladi (`AccountAddress::to_i105`)
  va barcha moslamalarni/SDKlarni qayta yaratishni talab qiladi. Joriy yo'l xaritasi I105 + ni saqlaydi
  kelajakda tadqiqotni davom ettirishda siqilgan (`sora`) qo'llab-quvvatlash
  Bech32m/QR qatlamlari (CAIP-10 xaritalash kechiktirilgan).

## Ochiq savollar

- `u16` diskriminantlari va ajratilgan diapazonlar uzoq muddatli talabni qoplashini tasdiqlang;
  aks holda `u32` ni varint kodlash bilan baholang.
- Ro'yxatga olish kitobini yangilash va qanday qilib ko'p imzoli boshqaruv jarayonini yakunlash
  bekor qilish/muddati o‘tgan ajratmalar ko‘rib chiqiladi.
- Aniq manifest imzo sxemasini aniqlang (masalan, Ed25519 multi-sig) va
  Nexus tarqatish uchun transport xavfsizligi (HTTPS pinning, IPFS xesh formati).
- Migratsiya uchun domen taxalluslarini/yo'naltirishni qo'llab-quvvatlash yoki yo'qligini aniqlang
  determinizmni buzmasdan ularni yuzaga chiqarish.
- Kotodama/IVM shartnomalarining I105 yordamchilariga qanday kirishini belgilang (`to_address()`,
  `parse_address()`) va zanjirli saqlash hech qachon CAIP-10 ni ochishi kerakmi?
  xaritalashlar (bugungi kunda I105 kanonik).
- Iroha zanjirlarini tashqi registrlarda ro'yxatdan o'tkazishni o'rganing (masalan, I105 registri,
  CAIP nom maydoni katalogi) kengroq ekotizimlarni moslashtirish uchun.

## Keyingi qadamlar

1. I105 kodlash `iroha_data_model` (`AccountAddress::to_i105`,
   `parse_encoded`); armatura/sinovlarni har bir SDK ga ko'chirishni davom ettiring va har birini tozalang
   Bech32m to'ldirgichlar.
2. `chain_discriminant` bilan konfiguratsiya sxemasini kengaytiring va oqilona xulosa chiqaring
  mavjud test/dev sozlamalari uchun standart sozlamalar. **(Bajarildi: `common.chain_discriminant`
  endi `iroha_config` da joʻnatiladi, har bir tarmoq bilan birlamchi `0x02F1`.
  bekor qiladi.)**
3. Nexus registr sxemasi va kontseptsiyani isbotlovchi manifest nashriyoti loyihasini tuzing.
4. Hamyon provayderlari va saqlovchilardan inson omili bo'yicha fikr-mulohazalarni to'plang
   (HRP nomlash, displey formatlash).
5. Hujjatlarni yangilang (`docs/source/data_model.md`, Torii API hujjatlari).
   amalga oshirish yo‘li belgilandi.
6. Rasmiy kodek kutubxonalarini (Rust/TS/Python/Kotlin) me'yoriy test bilan jo'natish
   muvaffaqiyat va muvaffaqiyatsizlik holatlarini qamrab oluvchi vektorlar.
