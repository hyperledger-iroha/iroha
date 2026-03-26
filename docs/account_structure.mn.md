---
lang: mn
direction: ltr
source: docs/account_structure.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 01561366c9698c10d29ff3f49ad4c14b22b1796b5c7701cf98d200a140af1caf
source_last_modified: "2026-01-28T17:11:30.635172+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Дансны бүтэц RFC

**Төлөв:** Зөвшөөрөгдсөн (ADDR-1)  
**Үзэгчид:** Дата загвар, Torii, Nexus, Wallet, Засаглалын баг  
**Холбогдох асуудлууд:** TBD

## Дүгнэлт

Энэ баримт бичигт хэрэгжсэн тээврийн дансны хаягжилтын стекийг тайлбарласан болно
`AccountAddress` (`crates/iroha_data_model/src/account/address.rs`) ба
хамтрагч хэрэгсэл. Үүнд:

- Шалгах нийлбэртэй, хүн рүү чиглэсэн **I105 хаяг**
  Гинжин ялгаварлагчийг дансанд холбодог `AccountAddress::to_i105`
  хянагч бөгөөд детерминист харилцан ажиллахад ээлтэй текст хэлбэрийг санал болгодог.
- Далд өгөгдмөл домэйн болон локал тоймд зориулсан домэйн сонгогчид
  Ирээдүйн Nexus-ээр дэмжигдсэн чиглүүлэлтийн хувьд нөөцлөгдсөн глобал бүртгэлийн сонгогч шошго (
  бүртгэлийн хайлт **хараахан хүргэгдээгүй**).

## Урам зориг

Түрийвч болон сүлжээнээс гадуурх хэрэгслүүд нь өнөөдөр түүхий `alias@domain` (rejected legacy form) чиглүүлэлтийн нэр дээр тулгуурладаг. Энэ
хоёр том дутагдалтай:

1. **Сүлжээний холболт байхгүй.** Мөрт шалгах нийлбэр эсвэл гинжин угтвар байхгүй тул хэрэглэгчид
   шууд санал хүсэлтгүйгээр буруу сүлжээнээс хаягийг буулгаж болно. The
   гүйлгээ нь эцэст нь татгалзах (гинжин таарахгүй байх) эсвэл бүр муугаар амжилтанд хүрэх болно
   хэрэв очих газар орон нутагт байгаа бол төлөвлөөгүй дансны эсрэг.
2. **Домэйн зөрчил.** Домэйн нь зөвхөн нэрийн орон зай бөгөөд тус бүр дээр дахин ашиглах боломжтой.
   гинж. Үйлчилгээний холбоо (кастодиан, гүүр, гинжин хэлхээний ажлын урсгал)
   А гинжин дээрх `finance` нь `finance`-тэй холбоогүй тул хэврэг болдог.
   гинж Б.

Бидэнд хуулах, буулгах алдаанаас хамгаалсан хүмүүст ээлтэй хаягийн формат хэрэгтэй байна
болон домэйн нэрээс эрх мэдэл бүхий гинжин хэлхээний тодорхойлогч зураглал.

## Зорилго

- Өгөгдлийн загварт хэрэгжсэн I105 дугтуйг тайлбарлана уу
  `AccountId` болон `AccountAddress` мөрддөг каноник задлан шинжлэх/алиа дүрмүүд.
- Тохируулсан хэлхээний дискриминантыг хаяг бүрт шууд кодлох ба
  түүний засаглал/бүртгэлийн үйл явцыг тодорхойлох.
- Гүйдлийг таслахгүйгээр дэлхийн домайн бүртгэлийг хэрхэн нэвтрүүлэх талаар тайлбарлана уу
  байршуулалт болон хэвийн болгох/хуурамчлахын эсрэг дүрмийг зааж өгнө.

## Зорилгогүй

- Хөрөнгө шилжүүлгийг гинжин хэлхээгээр хэрэгжүүлэх. Чиглүүлэлтийн давхарга нь зөвхөн буцаана
  зорилтот гинж.
- Глобал домэйн гаргах засаглалыг дуусгах. Энэхүү RFC нь өгөгдөлд анхаарлаа төвлөрүүлдэг
  загвар ба тээврийн командууд.

## Дэвсгэр

### Одоогийн чиглүүлэлтийн бусад нэр

```
AccountId {
    domain: DomainId,   // wrapper over Name (ASCII-ish string)
    controller: AccountController // single PublicKey or multisig policy
}

Display: canonical i105 literal (no `@domain` suffix)
Parse accepts:
- Encoded account identifiers only: i105.
- Runtime parsers reject canonical hex (`0x...`), any `@<domain>` suffix, and account-alias literals such as label@dataspace or label@domain.dataspace.

Multihash hex is canonical: varint bytes are lowercase hex, payload bytes are uppercase hex,
and `0x` prefixes are not accepted.

Account aliases are separate on-chain bindings. They use
label@dataspace or label@domain.dataspace and resolve to canonical
i105 `AccountId` values. Strict `AccountId` parsers never accept alias literals directly.
```

`ChainId` `AccountId`-ээс гадуур амьдардаг. Зангилаанууд гүйлгээний `ChainId`-г шалгана
элсэлтийн үеийн тохиргооны эсрэг (`AcceptTransactionFail::ChainIdMismatch`)
мөн гадаад гүйлгээнээс татгалзах боловч дансны мөр нь өөрөө үгүй гэсэн утгатай
сүлжээний зөвлөмж.

### Домэйн танигч

`DomainId` нь `Name` (хэвийн мөр)-ийг ороож, орон нутгийн гинжин хэлхээнд хамрах хүрээг хамардаг.
Сүлжээ бүр `wonderland`, `finance` гэх мэтийг бие даан бүртгэж болно.

### Nexus контекст

Nexus бүрэлдэхүүн хэсгүүдийн хоорондын зохицуулалтыг (зам/өгөгдлийн орон зай) хариуцдаг. Энэ
одоогоор хөндлөн гинжин домэйн чиглүүлэлтийн тухай ойлголт байхгүй байна.

## Санал болгож буй загвар

### 1. Детерминист гинжин дискриминант

`iroha_config::parameters::actual::Common` одоо ил болгож байна:

```rust
pub struct Common {
    pub chain: ChainId,
    pub chain_discriminant: u16, // globally coordinated
    // ... existing fields
}
```

- **Хязгаарлалт:**
  - Идэвхтэй сүлжээ бүрт өвөрмөц; -тэй гарын үсэг зурсан нийтийн бүртгэлээр дамжуулан удирддаг
    тодорхой нөөцлөгдсөн мужууд (жишээ нь, `0x0000–0x0FFF` тест/dev, `0x1000–0x7FFF`
    олон нийтийн хуваарилалт, `0x8000–0xFFEF` засаглалаар батлагдсан, `0xFFF0–0xFFFF`
    нөөцөлсөн).
  - Гүйлтийн гинжин хэлхээний хувьд хувиршгүй. Үүнийг өөрчлөхийн тулд хатуу сэрээ болон a
    бүртгэлийн шинэчлэл.
- **Засаглал ба бүртгэл (төлөвлөсөн):** Олон гарын үсэг бүхий засаглалын багц нь
  гарын үсэг зурсан JSON бүртгэлийн зураглалыг хүний өөр нэрээр ялгах болон
  CAIP-2 танигч. Энэ бүртгэл нь илгээсэн ажиллах хугацааны нэг хэсэг болоогүй байна.
- **Хэрэглээ:** Улсын бүртгэл, Torii, SDK болон түрийвчний API-уудаар дамждаг.
  Бүрэлдэхүүн хэсэг бүр үүнийг оруулах эсвэл баталгаажуулах боломжтой. CAIP-2-д өртөх нь ирээдүй хэвээр байна
  харилцан ажиллах даалгавар.

### 2. Каноник хаягийн кодлогч

Rust өгөгдлийн загвар нь нэг каноник ачааллын дүрслэлийг харуулж байна
(`AccountAddress`) нь хүн рүү чиглэсэн хэд хэдэн форматаар ялгарах боломжтой. I105 нь
хуваалцах болон каноник гаралтад зориулсан дансны илүүд үздэг формат; шахсан
`sora` хэлбэр нь кана цагаан толгойтой UX-д зориулсан хоёр дахь шилдэг, зөвхөн Sora сонголт юм.
үнэ цэнийг нэмдэг. Каноник hex нь дибаг хийх туслах хэвээр байна.

- **I105** – гинжийг суулгасан I105 дугтуй
  ялгаварлагч. Декодерууд ачааллыг дэмжихийн өмнө угтварыг баталгаажуулдаг
  каноник хэлбэр.
- **Сора-шахсан харагдац** – **105 тэмдэгт** бүхий зөвхөн Сора цагаан толгойн үсэг.
  хагас өргөнтэй イロハ шүлгийг (ヰ ба ヱ оруулаад) 58 тэмдэгт дээр нэмэх
  I105 багц. Мөрүүд нь харуулын `sora`-ээр эхэлж, Bech32m-ээс үүсэлтэй мөрийг суулгана.
  шалгах нийлбэр, мөн сүлжээний угтварыг орхигдсон (Sora Nexus нь харуулын үгээр илэрхийлэгддэг).

  ```
  I105  : 123456789ABCDEFGHJKMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz
  Iroha : ｲﾛﾊﾆﾎﾍﾄﾁﾘﾇﾙｦﾜｶﾖﾀﾚｿﾂﾈﾅﾗﾑｳヰﾉｵｸﾔﾏｹﾌｺｴﾃｱｻｷﾕﾒﾐｼヱﾋﾓｾｽ
  ```
- **Canonical hex** – дибаг хийхэд тохиромжтой `0x…` каноник байтын кодчилол
  дугтуй.

`AccountAddress::parse_encoded` I105 (илүү тохиромжтой), шахсан (`sora`, хоёрдугаарт) эсвэл каноник зургаан өнцөгтийг автоматаар илрүүлдэг
(Зөвхөн `0x...`; нүцгэн зургаан өнцөгт татгалзсан) код тайлагдсан ачаалал болон илэрсэн ачааг хоёуланг нь оруулж, буцаана.
`AccountAddress`. Torii одоо ISO 20022 нэмэлтийг `parse_encoded` гэж нэрлэдэг.
нь каноник hex хэлбэрийг хаяглаж хадгалдаг тул мета өгөгдөл нь тодорхойлогддог
анхны төлөөлөл үл хамааран.

#### 2.1 Толгойн байт байршил (ADDR-1a)

Каноник ачаалал бүрийг `header · controller` гэж тодорхойлсон. The
`header` нь байтуудад ямар задлан шинжлэлийн дүрэм хамаарахыг харуулдаг ганц байт юм.
дагах:

```
bit index:   7        5 4      3 2      1 0
             ┌─────────┬────────┬────────┬────┐
payload bit: │version  │ class  │  norm  │ext │
             └─────────┴────────┴────────┴────┘
```

Тиймээс эхний байт нь доод декодлогчдын схемийн мета өгөгдлийг багцалдаг:

| Бит | Талбай | Зөвшөөрөгдсөн утгууд | Зөрчлийн алдаа |
|------|-------|----------------|--------------------|
| 7-5 | `addr_version` | `0` (v1). `1-7` утгууд нь ирээдүйн засваруудад зориулагдсан болно. | `0-7` триггерийн гаднах утгууд `AccountAddressError::InvalidHeaderVersion`; хэрэгжүүлэлтүүд ЗААВАЛ 0 бус хувилбаруудыг өнөөдөр дэмжигдээгүй гэж үзэх ёстой. |
| 4-3 | `addr_class` | `0` = нэг түлхүүр, `1` = multisig. | Бусад утгууд `AccountAddressError::UnknownAddressClass`-ийг нэмэгдүүлдэг. |
| 2-1 | `norm_version` | `1` (Норм v1). `0`, `2`, `3` утгууд хадгалагдсан. | `0-3`-ийн гаднах утгууд нь `AccountAddressError::InvalidNormVersion`-ийг нэмэгдүүлдэг. |
| 0 | `ext_flag` | ЗААВАЛ `0` байх ёстой. | Set битийн өсөлт `AccountAddressError::UnexpectedExtensionFlag`. |

Rust кодлогч нь нэг товчлуурт хянагчдад `0x02` бичдэг (хувилбар 0, анги 0,
норм v1, өргөтгөлийн дарцаг арилгасан) болон multisig хянагчдад зориулсан `0x0A` (хувилбар 0,
анги 1, норм v1, өргөтгөлийн туг арилгасан).

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

#### 2.3 Хянагчийн даацын кодчилол (ADDR-1a)

Хянагчийн ачаалал нь каноник төлөвт толгой хэсгийн дараа (legacy selector segment байгаа үед түүнээс хойш) ирэх өөр нэг шошготой нэгдэл юм.

| Tag | Хянагч | Зохион байгуулалт | Тэмдэглэл |
|-----|------------|--------|-------|
| `0x00` | Нэг түлхүүр | `curve_id:u8` · `key_len:u8` · `key_bytes` | `curve_id=0x01` өнөөдөр Ed25519 рүү зурагладаг. `key_len` нь `u8`-тай хязгаарлагдсан; том утгууд нь `AccountAddressError::KeyPayloadTooLong`-ийг нэмэгдүүлдэг (тиймээс 255 байтаас дээш хэмжээтэй нэг түлхүүртэй ML-DSA нийтийн түлхүүрүүдийг кодлох боломжгүй тул multisig ашиглах ёстой). |
| `0x01` | Multisig | `version:u8` · `threshold:u16` · `member_count:u8` · (`curve_id:u8` · `weight:u16` · `key_len:u16` · `key_len:u16`)\* | 255 хүртэлх гишүүнийг дэмждэг (`CONTROLLER_MULTISIG_MEMBER_MAX`). Үл мэдэгдэх муруйнууд `AccountAddressError::UnknownCurve`-ийг нэмэгдүүлдэг; алдаатай бодлого нь `AccountAddressError::InvalidMultisigPolicy` болж хөөсөрдөг. |

Multisig-ийн бодлого нь CTAP2 маягийн CBOR газрын зураг болон каноник тоймыг илчилдэг
Хостууд болон SDK-ууд нь хянагчийг тодорхойлон шалгах боломжтой. Харна уу
Схемийн хувьд `docs/source/references/multisig_policy_schema.md` (ADDR-1c),
баталгаажуулалтын дүрэм, хэш хийх журам, алтан бэхэлгээ.

Бүх түлхүүр байтууд нь `PublicKey::to_bytes`-ийн буцаасан шиг кодлогдсон; Декодерууд `PublicKey` инстанцуудыг дахин бүтээж, хэрэв байт нь зарласан муруйтай таарахгүй бол `AccountAddressError::InvalidPublicKey`-ийг өсгөнө.

> **Ed25519 каноник хэрэгжилт (ADDR-3a):** `0x01` муруй түлхүүрүүд нь гарын үсэг зурсан хүний ​​ялгаруулсан байт мөрийг яг таг тайлах ёстой бөгөөд жижиг эрэмбийн дэд бүлэгт байх ёсгүй. Зангилаанууд одоо каноник бус кодчилол (жишээ нь, `2^255-19` модулийн бууруулсан утгууд) болон таних элемент зэрэг сул талуудаас татгалзаж байгаа тул SDK-ууд хаяг илгээхээсээ өмнө тохирох баталгаажуулалтын алдааг гаргах ёстой.

##### 2.3.1 Муруй тодорхойлогчийн бүртгэл (ADDR-1d)

| ID (`curve_id`) | Алгоритм | Онцлох хаалга | Тэмдэглэл |
|-----------------|-----------|--------------|-------|
| `0x00` | Захиалагдсан | — | ЯВДАЛТАЙ БАЙХГҮЙ; декодерын гадаргуу `ERR_UNKNOWN_CURVE`. |
| `0x01` | Ed25519 | — | Каноник v1 алгоритм (`Algorithm::Ed25519`); анхдагч тохиргоонд идэвхжүүлсэн. |
| `0x02` | ML‑DSA (Dilithium3) | — | Dilithium3 нийтийн түлхүүрийн байт (1952 байт) ашигладаг. `key_len` нь `u8` тул нэг түлхүүрийн хаягууд ML-DSA-г кодлох боломжгүй; multisig нь `u16` уртыг ашигладаг. |
| `0x03` | BLS12‑381 (хэвийн) | `bls` | G1 дэх нийтийн түлхүүрүүд (48 байт), G2 дахь гарын үсэг (96 байт). |
| `0x04` | secp256k1 | — | SHA‑256-аас дээш тодорхойлогч ECDSA; нийтийн түлхүүрүүд нь 33 байт SEC1 шахсан хэлбэрийг ашигладаг бол гарын үсэг нь каноник 64 байт `r∥s` байршлыг ашигладаг. |
| `0x05` | BLS12‑381 (жижиг) | `bls` | G2 дахь нийтийн түлхүүрүүд (96 байт), G1 дэх гарын үсэг (48 байт). |
| `0x0A` | ГОСТ Р 34.10-2012 (256, багц А) | `gost` | Зөвхөн `gost` функц идэвхжсэн үед л боломжтой. |
| `0x0B` | ГОСТ Р 34.10-2012 (256, багц В) | `gost` | Зөвхөн `gost` функц идэвхжсэн үед л боломжтой. |
| `0x0C` | ГОСТ R 34.10-2012 (256, багц C) | `gost` | Зөвхөн `gost` функц идэвхжсэн үед л боломжтой. |
| `0x0D` | ГОСТ Р 34.10-2012 (512, багц А) | `gost` | Зөвхөн `gost` функц идэвхжсэн үед л боломжтой. |
| `0x0E` | ГОСТ Р 34.10-2012 (512, багц B) | `gost` | Зөвхөн `gost` функц идэвхжсэн үед л боломжтой. |
| `0x0F` | SM2 | `sm` | DistID урт (u16 BE) + DistID байт + 65 байт SEC1 шахагдаагүй SM2 түлхүүр; `sm` идэвхжсэн үед л боломжтой. |

`0x06–0x09` оролтууд нэмэлт муруйлтуудад хуваарилагдаагүй хэвээр байна; шинэ танилцуулж байна
алгоритм нь замын газрын зургийн шинэчлэлт болон тохирох SDK/хостын хамрах хүрээг шаарддаг. Кодерууд
`ERR_UNSUPPORTED_ALGORITHM`-тэй ямар ч дэмжигдээгүй алгоритмаас ЗААВАЛ татгалзаж, мөн
код тайлагч нь `ERR_UNKNOWN_CURVE`-тай үл мэдэгдэх ID-ууд дээр хурдан ажиллах ёстой.
бүтэлгүйтсэн хаалттай зан байдал.

Каноник бүртгэл (машинаар уншигдах боломжтой JSON экспортыг оруулаад) доор амьдардаг
[`docs/source/references/address_curve_registry.md`](source/references/address_curve_registry.md).
Багаж хэрэгсэл нь тухайн өгөгдлийн багцыг шууд хэрэглэх ХЭРЭГТЭЙ, ингэснээр муруй тодорхойлогч хэвээр үлдэнэ
SDK болон операторын ажлын урсгалд нийцсэн.

- **SDK Gating:** SDK-ууд нь Ed25519-д зөвхөн баталгаажуулалт/кодчлолд тохируулна. Свифт ил гаргадаг
  эмхэтгэх цагийн тугууд (`IROHASWIFT_ENABLE_MLDSA`, `IROHASWIFT_ENABLE_GOST`,
  `IROHASWIFT_ENABLE_SM`); Java/Android SDK шаарддаг
  `AccountAddress.configureCurveSupport(...)`; JavaScript SDK ашигладаг
  `configureCurveSupport({ allowMlDsa: true, allowGost: true, allowSm2: true })`.
  secp256k1 дэмжлэг байгаа боловч JS/Android дээр анхдагчаар идэвхжээгүй
  SDK; Дуудлага хийгчид Ed25519 бус хянагчуудыг гаргахдаа шууд сонголтоо хийх ёстой.
- **Хост хаалга:** `Register<Account>` нь гарын үсэг зурсан хүмүүс алгоритм ашигладаг хянагчаас татгалздаг.
  зангилааны `crypto.allowed_signing` жагсаалтад байхгүй **эсвэл** муруй тодорхойлогч байхгүй
  `crypto.curves.allowed_curve_ids`, тиймээс кластерууд дэмжлэгийг сурталчлах ёстой (тохиргоо +
  genesis) ML‑DSA/GOST/SM хянагчдыг бүртгэхээс өмнө. BLS хянагч
  Алгоритмуудыг эмхэтгэх үед үргэлж зөвшөөрдөг (зөвшилцлийн түлхүүрүүд нь тэдгээрт тулгуурладаг),
  ба өгөгдмөл тохиргоо нь Ed25519 + secp256k1-г идэвхжүүлдэг.【crates/iroha_core/src/smartcontracts/isi/domain.rs:32】

##### 2.3.2 Multisig удирдлагын удирдамж

`AccountController::Multisig` бодлогоо дамжуулан цуврал болгодог
`crates/iroha_data_model/src/account/controller.rs` ба схемийг хэрэгжүүлдэг
[`docs/source/references/multisig_policy_schema.md`](source/references/multisig_policy_schema.md)-д баримтжуулсан.
Хэрэгжүүлэх гол дэлгэрэнгүй мэдээлэл:

- Бодлогуудыг `MultisigPolicy::validate()` өмнө нь хэвийн болгож баталгаажуулсан
  суулгаж байна. Босго нь ≥1 ба ≤Σ жинтэй байх ёстой; давхардсан гишүүд байна
  `(algorithm || 0x00 || key_bytes)`-ээр ангилсаны дараа тодорхой хасагдсан.
- Хоёртын удирдлагын ачаалал (`ControllerPayload::Multisig`) кодлодог
  `version:u8`, `threshold:u16`, `member_count:u8`, дараа нь гишүүн бүрийн
  `(curve_id, weight:u16, key_len:u16, key_bytes)`. Энэ бол яг юу юм
  `AccountAddress::canonical_bytes()` нь I105 (давуу)/sora (хоёр дахь шилдэг) ачаалалд бичдэг.
- Hashing (`MultisigPolicy::digest_blake2b256()`) нь Blake2b-256-г ашиглан
  `iroha-ms-policy` хувийн тохиргооны мөр тул засаглалын манифестууд нь холбогдох боломжтой
  I105-д суулгагдсан хянагч байттай таарч байгаа детерминист бодлогын ID.
- Бэхэлгээний хамрах хүрээ нь `fixtures/account/address_vectors.json` (тохиолдлууд
  `addr-multisig-*`). Түрийвч болон SDK нь каноник I105 мөрийг баталгаажуулах ёстой
  тэдгээрийн кодлогч нь Rust хэрэгжилттэй тохирч байгааг баталгаажуулахын тулд доор.

| Кейс ID | Босго / гишүүд | I105 үгийн утга (`0x02F1` угтвар) | Sora шахсан (`sora`) literal | Тэмдэглэл |
|---------|--------------------|--------------------------------|-------------------------|-------|
| `addr-multisig-council-threshold3` | `≥3` жин, гишүүд `(2,1,1)` | `SRfSHsrH3tEmYaaAYyD248F3vfT1oQ3WEGS22MaD8W9bLefF7rsoKLYGcpbcM9EcSus5ZhCAZU7ztn2BCsyeCAdfRncAVmVsipd4ibk6CBLF3Nrzcw8P7VKJg6mtFgEhWVTjfDkUMoc63oeEmaWyV6cyiphwk8ZgKAJUe4TyVtmKm1WWcg7qZ6i` | `sora3vﾑ2zkaoUwﾋﾅGﾘﾚyﾂe3ﾖfﾙヰｶﾘﾉwｷnoWﾛYicaUr3ﾔｲﾖ2Ado3TﾘYQﾉJqﾜﾇｳﾑﾐd8dDjRGｦ3Vﾃ9HcﾀMヰR8ﾎﾖgEqGｵEｾDyc5ﾁ1ﾔﾉ31sUﾑﾀﾖaｸxﾘ3ｲｷMEuFｺｿﾉBQSVQnxﾈeJzrXLヰhｿｹ5SEEﾅPﾂﾗｸdヰﾋ1bUGHｲVXBWNNJ6K` | Зөвлөл-домайн засаглалын чуулга. |
| `addr-multisig-wonderland-threshold2` | `≥2`, гишүүд `(1,2)` | `3xsmkps1KPBn9dtpE5qHRhHEZCpiAe8d9j6H9A42TV6kc1TpaqdwnSksKgQrsSEHznqvWKBMc1os69BELzkLjsR7EV2gjV14d9JMzo97KEmYoKtxCrFeKFAcy7ffQdboV1uRt` | `sora2ﾖZﾘeｴAdx3ﾂﾉﾔXhnｹﾀ2ﾉｱﾋxﾅﾄﾌヱwﾐmﾊvEﾐCﾏﾎｦ1ﾑHﾋso2GKﾔﾕﾁwﾂﾃP6ﾁｼﾙﾖｺ9ｻｦbﾈ4wFdﾑFヰ3HaﾘｼMｷﾌHWtｷﾋLﾙﾖQ4D3XﾊﾜXmpktﾚｻ5ﾅﾅﾇ1gkﾏsCFQGH9` | Хос гарын үсэг бүхий гайхамшгийн жишээ (жин 1 + 2). |
| `addr-multisig-default-quorum3` | `≥3`, гишүүд `(1,1,1,1)` | `nA2bDNhMqXz7ERkHNoEWbvJGyR1aDRsw32LaUWLgbK3vcpzohmdFCLvdotxUWWDY3aZeX4ptLk4Z6TjF5ossnJm8VrNo6daxmGTkqUyP4MxJxiNyPFxsEE5DLnsoLWUcxaWNpZ76tmkbiGS31Gv8tejKpuiHUMaQ1s5ohWyZvDnpycNkBK8AEfGJqn5yc9zAzfWbVhpDwkPj8ScnzvH1Echr5` | `soraﾐ38ﾅｴｸﾜ8ﾃzwBrqﾘｺ4yﾄv6kqJp1ｳｱﾛｿrzﾄﾃﾘﾒRﾗtV9ｼﾔPｽcヱEﾌVVVｼﾘｲZAｦﾓﾅｦeﾒN76vﾈcuｶuﾛL54rzﾙﾏX2zMﾌRLﾃﾋpﾚpｲcHﾑﾅﾃﾔzｵｲVfAﾃﾚﾎﾚCヰﾔｲｽｦw9ﾔﾕ8bGGkﾁ6sNｼaｻRﾖﾜYﾕﾚU18ﾅHヰﾌuMeﾊtﾂrｿj95Ft8ﾜ3fﾄkNiｴuﾈrCﾐQt8ヱｸｸmﾙﾒgUbﾑEKTTCM` | Үндсэн засаглалд ашигладаг далд-өгөгдмөл домайн чуулга.

#### 2.4 Алдаа гарах дүрэм (ADDR-1a)

- Шаардлагатай толгой + сонгогчоос богино эсвэл үлдсэн байт бүхий ачаалал нь `AccountAddressError::InvalidLength` эсвэл `AccountAddressError::UnexpectedTrailingBytes` ялгаруулдаг.
- Нөөцлөгдсөн `ext_flag`-г тохируулах эсвэл дэмжигдээгүй хувилбар/ангилуудыг сурталчлах толгой хэсгийг `UnexpectedExtensionFlag`, `InvalidHeaderVersion`, эсвэл `UnknownAddressClass` ашиглан татгалзах ёстой.
- Үл мэдэгдэх сонгогч/хянагч шошгууд нь `UnknownDomainTag` эсвэл `UnknownControllerTag`-ийг нэмэгдүүлдэг.
- Том хэмжээтэй эсвэл буруу хэлбэртэй гол материал нь `KeyPayloadTooLong` эсвэл `InvalidPublicKey`-ийг өсгөдөг.
- 255-аас дээш гишүүнтэй Multisig хянагчууд `MultisigMemberOverflow`-ийг нэмэгдүүлдэг.
- IME/NFKC хөрвүүлэлтүүд: хагас өргөнтэй Сора канаг код тайлахгүйгээр бүрэн өргөн хэлбэрт оруулах боломжтой боловч ASCII `sora` sentinel болон I105 цифр/үсэгүүд нь ASCII хэвээр байх ёстой. Бүтэн өргөнтэй эсвэл хайрцгаар нугалж буй харуулууд `ERR_MISSING_COMPRESSED_SENTINEL` гадаргуутай, бүрэн өргөнтэй ASCII ачаалал нь `ERR_INVALID_COMPRESSED_CHAR`-ийг өсгөж, шалгах нийлбэрийн зөрүү нь `ERR_CHECKSUM_MISMATCH` болж хөөсөрдөг. `crates/iroha_data_model/src/account/address.rs` дээрх үл хөдлөх хөрөнгийн туршилтууд нь эдгээр замыг хамардаг тул SDK болон түрийвч нь тодорхойлогч бүтэлгүйтэлд найдаж болно.
- Torii болон `address@domain` (rejected legacy form) нэрсийн SDK задлан шинжлэл нь I105 (давуу)/sora (хоёр дахь шилдэг) оролтууд нь өөр нэр буцаахаас өмнө бүтэлгүйтсэн (жишээ нь, домайны бүтцийн алдаа алдагдах), үйлчлүүлэгчийн бүтцийн алдаа алдагдах үед одоо ижил `ERR_*` кодуудыг ялгаруулдаг. зохиолын мөрүүдээс таамаглах.
- 12 байтаас богино локал сонгогчийн ачаалал нь `ERR_LOCAL8_DEPRECATED` гадаргуутай бөгөөд хуучин Local‑8 дижестээс хатуу таслалтыг хадгалдаг.
- Domainless canonical i105 literals decode directly to a domainless `AccountId`. Use `ScopedAccountId` only when an interface requires explicit domain context.

#### 2.5 Норматив хоёртын векторууд

- **Далд өгөгдмөл домэйн (`default`, үрийн байт `0x00`)**  
  Каноник зургаан өнцөгт: `0x020001203b6a27bcceb6a42d62a3a8d02a6f0d73653215771de243a63ac048a18b59da29`.  
  Завсарлага: `0x02` толгой хэсэг, `0x00` сонгогч (далд өгөгдмөл), `0x00` хянагчийн шошго, `0x01` муруй id (Ed25519), I18NI000 түлхүүрийн урт, 2-р төлбөр төлнө.
- **Орон нутгийн домэйн тойм (`treasury`, үрийн байт `0x01`)**  
  Каноник зургаан өнцөгт: `0x0201b18fe9c1abbac45b3e38fc5d0001208a88e3dd7409f195fd52db2d3cba5d72ca6709bf1d94121bf3748801b40f6f5c`.  
  Задаргаа: `0x02` толгой хэсэг, сонгогчийн шошго `0x01` дээр нэмэх нь `b1 8f e9 c1 ab ba c4 5b 3e 38 fc 5d`, дараа нь нэг товчлуурын ачаалал (`0x00` шошго, `0x01` муруй id, `0x01` урт id002, I182te, `b1 8f e9 c1 ab ba c4 5b 3e 38 fc 5d`) Ed25519 түлхүүр).Нэгжийн туршилтууд (`account::address::tests::parse_encoded_accepts_all_formats`) доорх V1 векторуудыг `AccountAddress::parse_encoded`-ээр баталгаажуулж, багаж хэрэгсэл нь зургаан өнцөгт, I105 (давуу) болон шахсан (`sora`, хоёр дахь шилдэг) хэлбэрт хамаарах каноник ачааллаас хамааралтай болохыг баталгаажуулдаг. Өргөтгөсөн бэхэлгээний багцыг `cargo run -p iroha_data_model --example address_vectors` ашиглан сэргээнэ үү.

| Домэйн | Үрийн байт | Каноник зургаан өнцөгт | Шахсан (`sora`) |
|-------------|-----------|-----------------------------------------------------------------------------------------|------------|
| анхдагч | `0x00` | `0x020001203b6a27bcceb6a42d62a3a8d02a6f0d73653215771de243a63ac048a18b59da29` | `sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE` |
| төрийн сан | `0x01` | `0x0201b18fe9c1abbac45b3e38fc5d0001208a88e3dd7409f195fd52db2d3cba5d72ca6709bf1d94121bf3748801b40f6f5c` | `sora5ｻu6rﾀCヰTGwﾏ1ﾅヱﾌQｲﾖﾇqCｦヰﾓZQCZRDSSﾅMｱﾙヱｹﾁｸ8ｾeﾄﾛ6C8bZuwﾗｹCZｦRSLQFU` |
| гайхамшигт орон | `0x02` | `0x0201b8ae571b79c5a80f5834da2b0001208139770ea87d175f56a35466c34c7ecccb8d8a91b4ee37a25df60f5b8fc9b394` | `sora5ｻwﾓyRｿqﾏnMﾀﾙヰKoﾒﾇﾓQｺﾛyｼ3ｸFHB2F5LyPﾐTMZkｹｼw67ﾋVﾕｻr8ﾉGﾇeEnｻVRNKCS` |
| ироха | `0x03` | `0x0201de8b36819700c807083608e2000120ed4928c628d1c2c6eae90338905995612959273a5c63f93636c14614ac8737d1` | `sora5ｻﾜxﾀ7Vｱ7QFeｷMﾂLﾉﾃﾏﾓﾀTﾚgSav3Wnｱｵ4ｱCKｷﾛMﾘzヰHiﾐｱ6ﾃﾉﾁﾐZmﾇ2fiﾎX21P4L` |
| альфа | `0x04` | `0x020146be2154ae86826a3fef0ec0000120ca93ac1705187071d67b83c7ff0efe8108e8ec4530575d7726879333dbdabe7c` | `sora5ｻ9JヱﾈｿuwU6ｴpﾔﾂﾈRqRTds1HﾃﾐｶLVﾍｳ9ﾔhｾNｵVｷyucEﾒGﾈﾏﾍ9sKeﾉDzrｷﾆ742WG1` |
| омега | `0x05` | `0x0201390d946885bc8416b3d30c9d0001206e7a1cdd29b0b78fd13af4c5598feff4ef2a97166e3ca6f2e4fbfccd80505bf1` | `sora5ｻ3zrﾌuﾚﾄJﾑXQhｸTyN8pzwRkWxmjVﾗbﾚﾕヰﾈoｽｦｶtEEﾊﾐ6GPｿﾓﾊｾEhvPｾｻ3XAJ73F` |
| засаглал | `0x06` | `0x0201989eb45a80940d187e2c908f0001208a875fff1eb38451577acd5afee405456568dd7c89e090863a0557bc7af49f17` | `sora5ｻiｵﾁyVﾕｽbFpDHHuﾇﾉdﾗｲﾓﾄRﾋAW3frUCｾ5ｷﾘTwdﾚnｽtQiLﾏｼｶﾅXgｾZmﾒヱH58H4KP` |
| баталгаажуулагч | `0x07` | `0x0201e4ffa58704c69afaeb7cc2d7000120ea4a6c63e29c520abef5507b132ec5f9954776aebebe7b92421eea691446d22c` | `sora5ｻﾀLDH6VYﾑNAｾgﾉVﾜtxﾊRXLｹﾍﾔﾌLd93GﾔGeｴﾄYrs1ﾂHｸkYxｹwｿyZﾗxyﾎZoXT1S4N` |
| судлаач | `0x08` | `0x02013b35422c65c2a83c99c523ad0001201398f62c6d1a457c51ba6a4b5f3dbd2f69fca93216218dc8997e416bd17d93ca` | `sora5ｻ4nmｻaﾚﾚPvNLgｿｱv6MHDeEyﾀovﾉJcpvrﾖ6ﾈCQcCNﾇﾜhﾚﾖyFdTwｸｶHEｱ9rWU8FMB` |
| соранет | `0x09` | `0x0201047d9ea7f5d5dbec3f7bfc58000120fd1724385aa0c75b64fb78cd602fa1d991fdebf76b13c58ed702eac835e9f618` | `sora5ｱｸヱVQﾂcﾁヱRﾓcApｲﾁﾅﾒvﾌﾏfｾNnﾛRJsｿDhﾙuHaﾚｺｦﾌﾍﾈeﾆﾎｺN1UUDｶ6ﾎﾄﾛoRH8JUL` |
| kitsune | `0x0A` | `0x0201e91933de397fd7723dc9a76c00012043a72e714401762df66b68c26dfbdf2682aaec9f2474eca4613e424a0fbafd3c` | `sora5ｻﾚｺヱkfFJfSﾁｼJwﾉLvbpSｷﾔMWFMrbｳｸｲｲyヰKGJﾉｻ4ｹﾕrｽhｺｽzSDヰXAN62AD7RGNS` |
| да | `0x0B` | `0x02016838cf5bb0ce0f3d4f380e1c00012066be7e332c7a453332bd9d0a7f7db055f5c5ef1a06ada66d98b39fb6810c473a` | `sora5ｻNﾒ5SﾐRﾉﾐﾃ62ｿ1ｶｷWFKyF1BcAﾔvｼﾐHqﾙﾐPﾏｴヰ5tｲﾕvnﾙT6ﾀW7mﾔ7ﾇﾗﾂｳ25CXS93` |

Хянсан: Өгөгдлийн загвар АХ, Криптографийн АХ - ADDR-1a-д батлагдсан хамрах хүрээ.

##### Sora Nexus лавлагааны бусад нэр

Sora Nexus сүлжээг `chain_discriminant = 0x02F1` гэж анхдагчаар тохируулна.
(`iroha_config::parameters::defaults::common::CHAIN_DISCRIMINANT`). The
Тиймээс `AccountAddress::to_i105` болон `to_i105` туслахууд ялгаруулдаг.
каноник ачаалал бүрийн хувьд тууштай текст хэлбэрүүд. -аас сонгосон бэхэлгээ
`fixtures/account/address_vectors.json` (-аар үүсгэгдсэн
`cargo xtask address-vectors`)-ийг хурдан лавлах зорилгоор доор харуулав.

| Данс / сонгогч | I105 үгийн утга (`0x02F1` угтвар) | Sora шахсан (`sora`) literal |
|--------------------------------|--------------------------------|-------------------------|
| `default` домэйн (далд сонгогч, үрийн `0x00`) | `6cmzPVPX5jDQFNfiz6KgmVfm1fhoAqjPhoPFn4nx9mBWaFMyUCwq4cw` | `sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE` (заавал биш `@default` чиглүүлэлтийн зөвлөмж өгөх үед) |
| `treasury` (орон нутгийн дижест сонгогч, үрийн `0x01`) | `34mSYnCXkCzHXm31UDHh7SJfGvC4QPEhwim8z7sys2iHqXpCwCQkjL8KHvkFLSs1vZdJcb37r` | `sora5ｻu6rﾀCヰTGwﾏ1ﾅヱﾌQｲﾖﾇqCｦヰﾓZQCZRDSSﾅMｱﾙヱｹﾁｸ8ｾeﾄﾛ6C8bZuwﾗｹCZｦRSLQFU` |
| Глобал бүртгэлийн заагч (`registry_id = 0x0000_002A`, `treasury`-тэй тэнцэх) | `3oE9sLeRGP49Cu7mQ1nF4wtKAm29BG4TGLiRsaXe7mhbMP5WZ113nNW1N6RbqF` | `sorakXｹ6NｻﾍﾀﾖSﾜﾖｱ3ﾚ5WﾘﾋQﾅｷｦxgﾛｸcﾁｵﾋkﾋvﾏ8SPﾓﾀｹdｴｴｲW9iCM6AEP` |

Эдгээр мөрүүд нь CLI (`iroha tools address convert`), Torii-ээс ялгардаг мөртэй таарч байна.
хариултууд (`canonical i105 literal rendering`) болон SDK туслахууд тул UX хуулах/буудах
урсгалууд тэдгээрт үгчлэн найдаж болно. Зөвхөн тодорхой чиглүүлэлтийн зөвлөмж хэрэгтэй үед л `<address>@<domain>` (rejected legacy form)-г хавсаргана уу; дагавар нь каноник гаралтын хэсэг биш юм.

#### 2.6 Харилцан ажиллахад зориулсан текстийн нэр (төлөвлөсөн)

- **Гинжин нэрийн загвар:** Гуалин болон хүний хувьд `ih:<chain-alias>:<alias@domain>`
  оруулга. Түрийвч угтварыг задлан шинжилж, суулгагдсан гинжийг шалгаж, блоклох ёстой
  таарахгүй.
- **CAIP-10 маягт:** гинжин хэлхээнд хамаарахгүй `iroha:<caip-2-id>:<i105-addr>`
  нэгтгэлүүд. Энэ зураглал нь тээвэрлэгдсэн хэсэгт ** хараахан хэрэгжээгүй байна**
  багажны гинж.
- **Машины туслахууд:** Rust, TypeScript/JavaScript, Python-д зориулсан кодлогчийг нийтлэх,
  болон Котлин I105 болон шахсан форматуудыг (`AccountAddress::to_i105`,
  `AccountAddress::parse_encoded` ба тэдгээрийн SDK эквивалент). CAIP-10 туслахууд нь
  ирээдүйн ажил.

#### 2.7 Тодорхойлогч I105 нэр

- ** Угтвар зураглал:** `chain_discriminant`-г I105 сүлжээний угтвар болгон дахин ашиглаарай.
  `encode_i105_prefix()` (`crates/iroha_data_model/src/account/address.rs`-г үзнэ үү)
  `<64` утгуудын хувьд 6 битийн угтвар (нэг байт) ба 14 битийн хоёр байт ялгаруулдаг
  том сүлжээнд зориулсан маягт. Эрх мэдэл бүхий даалгаварууд амьдардаг
  [`address_prefix_registry.md`](source/references/address_prefix_registry.md);
  SDK-ууд мөргөлдөхөөс зайлсхийхийн тулд тохирох JSON бүртгэлийг синхрончлолд оруулах ЗААВАЛ ХЭРЭГТЭЙ.
- **Бүртгэлийн материал:** I105 нь бүтээсэн каноник ачааллыг кодлодог
  `AccountAddress::canonical_bytes()`—толгой байт, домэйн сонгогч болон
  хянагчийн ачаалал. Нэмэлт хэш хийх алхам байхгүй; I105 нь суулгасан
  Rust-ийн үйлдвэрлэсэн хоёртын хянагчийн ачаалал (нэг түлхүүр эсвэл multisig).
  кодлогч, multisig бодлогын хураангуйд ашигладаг CTAP2 газрын зураг биш.
- **Кодчилол:** `encode_i105()` нь угтвар байтыг канониктай холбодог.
  Ачаалал ихтэй бөгөөд Blake2b-512-оос авсан 16 бит шалгах нийлбэрийг тогтмол
  угтвар `I105PRE` (`b"I105PRE"` || prefix || payload). Үр дүн нь I105 цагаан толгойг ашиглан `bs58`-ээр кодлогдоно.
  CLI/SDK туслахууд ижил процедурыг илчилдэг ба `AccountAddress::parse_encoded`
  `decode_i105`-ээр дамжуулан буцаана.

#### 2.8 Норматив текстийн тест векторууд

`fixtures/account/address_vectors.json` бүрэн I105 (давуу) болон шахсан (`sora`, хоёрдугаарт) агуулсан
каноник ачаалал бүрийн литерал. Онцлох үйл явдал:

- **`addr-single-default-ed25519` (Sora Nexus, угтвар `0x02F1`).**  
  I105 `6cmzPVPX5jDQFNfiz6KgmVfm1fhoAqjPhoPFn4nx9mBWaFMyUCwq4cw`, шахсан (`sora`)
  `sora2QG…U4N5E5`. Torii нь `AccountId`-ээс яг эдгээр мөрүүдийг ялгаруулдаг.
  `Display` хэрэгжилт (каноник I105) болон `AccountAddress::to_i105`.
- **`addr-global-registry-002a` (бүртгэл сонгогч → төрийн сан).**  
  I105 `3oE9sLeRGP49Cu7mQ1nF4wtKAm29BG4TGLiRsaXe7mhbMP5WZ113nNW1N6RbqF`, шахсан (`sora`)
  `sorakX…CM6AEP`. Бүртгэлийн сонгогчид код тайлдаг хэвээр байгааг харуулж байна
  харгалзах орон нутгийн тоймтой ижил каноник ачаалал.
- **Алдаа гарсан тохиолдол (`i105-prefix-mismatch`).**  
  Зангилаа дээрх `NETWORK_PREFIX + 1` угтвараар кодлогдсон I105 литералыг задлан шинжилж байна
  өгөгдмөл угтварыг хүлээж байна
  `AccountAddressError::UnexpectedNetworkPrefix { expected: 753, found: 754 }`
  домэйн чиглүүлэх оролдлого хийхээс өмнө. `i105-checksum-mismatch` бэхэлгээ
  Blake2b шалгах нийлбэр дээр хөндлөнгийн оролцоо илрүүлэх дасгал хийдэг.

#### 2.9 Дагаж мөрдөх бэхэлгээ

ADDR‑2 нь эерэг ба сөрөг талыг хамарсан дахин тоглуулах боломжтой бэхэлгээний багцыг нийлүүлдэг
каноник hex, I105 (давуу), шахсан (`sora`, хагас/бүтэн өргөн), далд
өгөгдмөл сонгогч, дэлхийн бүртгэлийн нэр, олон гарын үсэгтэй хянагч. The
каноник JSON нь `fixtures/account/address_vectors.json`-д амьдардаг бөгөөд байж болно
нөхөн сэргээсэн:

```
cargo xtask address-vectors --out fixtures/account/address_vectors.json
# verify without writing:
cargo xtask address-vectors --verify
```

Түр зуурын туршилтуудын хувьд (өөр зам/формат) хоёртын хувилбар хэвээр байна
боломжтой:

```
cargo run -p iroha_data_model --example account_address_vectors > fixtures/account/address_vectors.json
```

`crates/iroha_data_model/tests/account_address_vectors.rs` дахь зэвний нэгжийн туршилт
болон `crates/iroha_torii/tests/account_address_vectors.rs`, JS-ийн хамт,
Swift болон Android төхөөрөмж (`javascript/iroha_js/test/address.test.js`,
`IrohaSwift/Tests/IrohaSwiftTests/AccountAddressTests.swift`,
`java/iroha_android/src/test/java/org/hyperledger/iroha/android/address/AccountAddressTests.java`),
SDK болон Torii-ийн элсэлтийн кодлогчийн харьцааг баталгаажуулахын тулд ижил төхөөрөмжийг ашиглана уу.

### 3. Дэлхий дахинд давтагдашгүй домэйнууд ба хэвийн байдал

Мөн үзнэ үү: [`docs/source/references/address_norm_v1.md`](source/references/address_norm_v1.md)
Torii, өгөгдлийн загвар болон SDK-д ашигладаг каноник Норм v1 дамжуулах шугамын хувьд.

`DomainId`-г шошготой tuple болгон дахин тодорхойлно уу:

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

`LocalChain` нь одоогийн сүлжээгээр удирдаж буй домэйнуудын одоо байгаа Нэрийг ороосон.
Домэйн дэлхийн бүртгэлээр бүртгэгдсэн тохиолдолд бид эзэмшдэг
гинжин ялгаварлагч. Дэлгэц / задлан шинжлэх нь одоогоор өөрчлөгдөөгүй хэвээр байгаа боловч
өргөтгөсөн бүтэц нь чиглүүлэлтийн шийдвэр гаргах боломжийг олгодог.

#### 3.1 Нормчилол ба хууран мэхлэх хамгаалалт

Норм v1 нь бүрэлдэхүүн хэсэг бүр домэйны өмнө ашиглах ёстой канон шугамыг тодорхойлдог
нэр хэвээр эсвэл `AccountAddress`-д суулгагдсан. Бүрэн танилцуулга
[`docs/source/references/address_norm_v1.md`](source/references/address_norm_v1.md)-д амьдардаг;
Доорх хураангуй нь түрийвч, Torii, SDK болон засаглал зэрэг алхмуудыг агуулна.
хэрэгслүүдийг хэрэгжүүлэх ёстой.

1. **Оролтын баталгаажуулалт.** Хоосон мөр, хоосон зай болон нөөцлөлтөөс татгалзах
   хязгаарлагч `@`, `#`, `$`. Энэ нь хэрэгжүүлсэн инвариантуудтай таарч байна
   `Name::validate_str`.
2. **Unicode NFC найрлага.** ICU-д тулгуурласан NFC хэвийн болгохыг каноник байдлаар хэрэглээрэй.
   эквивалент дараалал нь тодорхой хэмжээгээр унадаг (жишээ нь, `e\u{0301}` → `é`).
3. **UTS-46-г хэвийн болгох.** NFC гаралтыг UTS‑46-аар дамжуулан ажиллуулна уу.
   `use_std3_ascii_rules = true`, `transitional_processing = false`, болон
   DNS уртын хэрэгжилтийг идэвхжүүлсэн. Үр дүн нь жижиг үсгээр бичсэн А шошгоны дараалал юм;
   STD3 дүрмийг зөрчсөн оролтууд энд бүтэлгүйтдэг.
4. **Уртны хязгаар.** DNS загварын хязгаарыг хэрэгжүүлэх: шошго бүр 1–63 байх ёстой.
   3-р алхамын дараа байт болон бүтэн домэйн 255 байтаас хэтрэх ЁСТОЙ.
5. **Заавал ойлгомжгүй бодлого.** UTS‑39 скриптийн шалгалтыг дараах байдлаар хянадаг.
   Норм v2; Операторууд тэдгээрийг эрт идэвхжүүлж болох боловч шалгалт амжилтгүй болбол цуцлах ёстой
   боловсруулах.

Хэрэв үе шат бүр амжилттай болсон бол жижиг үсгээр бичсэн A-шошгоны мөрийг кэш болгож, ашигладаг
хаягийн кодчилол, тохиргоо, манифест, бүртгэлийн хайлт. Орон нутгийн тойм
Сонгогчид өөрсдийн 12 байт утгыг `blake2s_mac(key = "SORA-LOCAL-K:v1",
каноник_шошго)[0..12]` 3-р алхамын гаралтыг ашиглана. Бусад бүх оролдлого (холимог
үсэг, том үсгээр, түүхий Юникод оролт) бүтэцтэй татгалзсан
Нэрийг нь өгсөн хил дээр `ParseError`s.

Эдгээр дүрмийг харуулсан каноник бэхэлгээ, үүнд punycode хоёр талын аялал орно
болон хүчингүй STD3 дараалал - жагсаалтад орсон
`docs/source/references/address_norm_v1.md` ба SDK CI-д тусгагдсан байдаг
ADDR‑2 дагуу хянагдсан вектор цуглуулгууд.

### 4. Nexus домэйны бүртгэл ба чиглүүлэлт- **Бүртгэлийн схем:** Nexus нь гарын үсэг зурсан `DomainName -> ChainRecord` газрын зургийг хадгалдаг.
  `ChainRecord` нь гинжин ялгаварлагч, нэмэлт мета өгөгдөл (RPC) агуулдаг
  төгсгөлийн цэгүүд), эрх мэдлийн нотолгоо (жишээ нь, засаглалын олон гарын үсэг).
- **Синк хийх механизм:**
  - Сүлжээнүүд гарын үсэг зурсан домайн нэхэмжлэлийг Nexus-д (үүслийн үед эсвэл дамжуулан) илгээдэг.
    засаглалын заавар).
  - Nexus тогтмол манифест нийтэлдэг (гарын үсэг зурсан JSON болон нэмэлт Merkle үндэс)
    HTTPS болон агуулгын хаягтай хадгалах сан (жишээ нь, IPFS) дээр. Үйлчлүүлэгчид
    хамгийн сүүлийн үеийн манифест болон гарын үсгийг баталгаажуулах.
- ** Хайлтын урсгал:**
  - Torii нь `DomainId` лавлагаатай гүйлгээг хүлээн авдаг.
  - Хэрэв домэйн нь дотоодод тодорхойгүй бол Torii нь кэштэй Nexus манифестээс асуудаг.
  - Хэрэв манифест нь гадаад сүлжээг зааж байгаа бол гүйлгээнээс татгалзана
    тодорхойлогч `ForeignDomain` алдаа ба алсын гинжин хэлхээний мэдээлэл.
  - Хэрэв Nexus-д домэйн байхгүй бол Torii `UnknownDomain`-г буцаана.
- **Итгэмжлэх зангуу ба эргэлт:** Засаглалын түлхүүрүүдийн тэмдгийн манифестууд; эргэлт эсвэл
  хүчингүй болгох нь шинэ манифест оруулга хэлбэрээр нийтлэгдсэн. Үйлчлүүлэгчид манифестыг хэрэгжүүлдэг
  TTL (жишээ нь, 24 цаг) ба энэ цонхны цаана байгаа хуучирсан өгөгдөлтэй зөвлөлдөхөөс татгалз.
- **Алдаа гарах горимууд:** Хэрэв манифест сэргээх ажиллагаа амжилтгүй болвол Torii кэш рүү буцна.
  TTL доторх өгөгдөл; Өнгөрсөн TTL энэ нь `RegistryUnavailable` ялгаруулж, татгалздаг
  Тогтворгүй төлөв байдлаас зайлсхийхийн тулд домайн хоорондын чиглүүлэлт.

### 4.1 Бүртгэлийн өөрчлөлтгүй байдал, нэр, булшны чулуу (ADDR-7c)

Nexus нь **зөвхөн хавсаргах манифест**-ийг нийтэлдэг тул домэйн болон бусад нэрийн хуваарилалт бүрийг
аудит хийж, дахин тоглуулах боломжтой. Операторууд дээр дурдсан багцыг авч үзэх ёстой
[хаяг манифест runbook](source/runbooks/address_manifest_ops.md).
Үнэний цорын ганц эх сурвалж: хэрэв манифест байхгүй эсвэл баталгаажуулалт амжилтгүй бол Torii
нөлөөлөлд өртсөн домайныг шийдвэрлэхээс татгалзах.

Автоматжуулалтын дэмжлэг: `cargo xtask address-manifest verify --bundle <current_dir> --previous <previous_dir>`
-д бичсэн шалгах нийлбэр, схем болон өмнөх хураангуй шалгалтуудыг дахин тоглуулдаг
runbook. `sequence`-г харуулахын тулд өөрчлөлтийн тасалбарт тушаалын гаралтыг оруулна уу
болон `previous_digest` холболтыг багцыг нийтлэхээс өмнө баталгаажуулсан.

#### Манифест толгой ба гарын үсэг зурах гэрээ

| Талбай | Шаардлага |
|-------|-------------|
| `version` | Одоогоор `1`. Зөвхөн тохирох техникийн шинэчлэлтээр овойно. |
| `sequence` | Нэг хэвлэлд **яг** нэгээр нэмэгдэнэ. Torii кэш нь цоорхой эсвэл регресс бүхий засвараас татгалздаг. |
| `generated_ms` + `ttl_hours` | Кэшийн шинэлэг байдлыг тохируулах (анхдагчаар 24 цаг). Хэрэв TTL дараагийн хэвлэлтээс өмнө дуусвал Torii нь `RegistryUnavailable` руу шилждэг. |
| `previous_digest` | Өмнөх илэрхий биеийн BLAKE3 задаргаа (hex). Баталгаажуулагчид үүнийг `b3sum` ашиглан дахин тооцоолж, өөрчлөгдөшгүйг нотлодог. |
| `signatures` | Манифестуудад Sigstore (`cosign sign-blob`)-ээр гарын үсэг зурсан. Үйлдлүүд нь нэвтрүүлэхээс өмнө `cosign verify-blob --bundle manifest.sigstore manifest.json`-г ажиллуулж, засаглалын таниулбар/гаргагчийн хязгаарлалтыг хэрэгжүүлэх ёстой. |

Хувилбарын автоматжуулалт нь `manifest.sigstore` болон `checksums.sha256` ялгаруулдаг.
JSON биеийн хажууд. SoraFS эсвэл толин тусгал хийхдээ файлуудыг хамтад нь байлга
HTTP төгсгөлийн цэгүүд нь аудиторууд баталгаажуулах алхмуудыг үгчлэн тоглуулах боломжтой.

#### Оролтын төрлүүд

| Төрөл | Зорилго | Шаардлагатай талбарууд |
|------|---------|-----------------|
| `global_domain` | Домэйн нь дэлхийн хэмжээнд бүртгэгдсэн бөгөөд гинжин ялгаварлагч болон I105 угтвартай байх ёстой гэж мэдэгдэнэ. | `{ "domain": "<label>", "chain": "sora:nexus:global", "i105_prefix": 753, "selector": "global" }` |
| `tombstone` | Алс/сонгогчийг бүрмөсөн чөлөөлнө. Local‑8 дижестийг устгах эсвэл домэйныг устгах үед шаардлагатай. | `{ "selector": {…}, "reason_code": "LOCAL8_RETIREMENT" \| …, "ticket": "<governance id>", "replaces_sequence": <number> }` |

`global_domain` оруулгад `manifest_url` эсвэл `sorafs_cid` оруулах боломжтой.
гарын үсэг зурсан гинжин хэлхээний мета өгөгдөл рүү түрийвчээ чиглүүлэх боловч каноник багц хэвээр байна
`{domain, chain, discriminant/i105_prefix}`. `tombstone` бичлэгүүд **заавал иш татах**
тэтгэвэрт гарсан сонгогч болон эрх олгосон тасалбар/засаглалын олдвор
өөрчлөлт, ингэснээр аудитын мөрийг офлайнаар сэргээх боломжтой.

#### Алс/булшны ажлын урсгал ба телеметр

1. **Дрифтийг илрүүлэх.** `torii_address_local8_total{endpoint}`,
   `torii_address_local8_domain_total{endpoint,domain}`,
   `torii_address_collision_total{endpoint,kind="local12_digest"}`,
   `torii_address_collision_domain_total{endpoint,domain}`,
   `torii_address_domain_total{endpoint,domain_kind}`, мөн
   `torii_address_invalid_total{endpoint,reason}` (үзүүлсэн
   `dashboards/grafana/address_ingest.json`) Орон нутгийн ирүүлсэн материалыг баталгаажуулах ба
   Булшны чулууг санал болгохоос өмнө орон нутгийн-12 мөргөлдөөн тэг хэвээр байна. The
   Домэйн тоологч нь зөвхөн хөгжүүлэлтийн/туршилтын домэйнууд Local‑8 ялгаруулдаг гэдгийг нотлох боломжийг эзэмшигчдэд олгодог
   урсгал (мөн тухайн Local‑12 мөргөлдөөн нь мэдэгдэж буй үе шатны домайнуудын зураглал) байхад
   **Домэйн төрлийн холимог (5м)** самбарыг багтаасан тул SRE нь хэр их байгааг графикаар харуулах боломжтой
   `domain_kind="local12"` урсгал хэвээр байгаа бөгөөд `AddressLocal12Traffic`
   Гэсэн хэдий ч үйлдвэрлэл нь Local-12 сонгогчийг харсаар байх үед дохио өгдөг
   тэтгэврийн хаалга.
2. **Каноник дижестийг гаргаж авах.** Гүйлгэх
   `iroha tools address convert <address> --format json --expect-prefix 753`
   (эсвэл `fixtures/account/address_vectors.json` ашиглан
   `scripts/account_fixture_helper.py`) яг `digest_hex`-г авах.
   CLI нь I105, `i105` болон каноник `0x…` литералуудыг хүлээн зөвшөөрдөг; хавсаргана
   `@<domain>` зөвхөн манифестын шошгыг хадгалах шаардлагатай үед.
   JSON хураангуй нь тухайн домайныг `input_domain` талбараар харуулдаг ба
   `legacy  suffix` хөрвүүлсэн кодчилолыг `<address>@<domain>` (rejected legacy form) гэж дахин тоглуулдаг.
   манифест ялгаа (энэ дагавар нь мета өгөгдөл бөгөөд каноник дансны id биш).
   Шинэ мөрөнд чиглэсэн экспортод ашиглах
   `iroha tools address normalize --input <file> legacy-selector input mode` Local-г массаар хөрвүүлэх
   сонгогчийг алгасах явцад каноник I105 (илүү зохимжтой), шахсан (`sora`, хоёрдугаарт), hex эсвэл JSON маягт руу оруулах
   орон нутгийн бус мөрүүд. Аудиторуудад хүснэгтэд ээлтэй нотлох баримт хэрэгтэй бол ажиллуул
   CSV хураангуйг гаргахын тулд `iroha tools address audit --input <file> --format csv`
   (`input,status,format,domain_kind,…`) нь Орон нутгийн сонгогчдыг онцолсон,
   каноник кодчилол, ижил файлд задлан шинжлэх алдаа.
3. **Манифест оруулгуудыг хавсаргана уу.** `tombstone` бичлэгийн ноорог (болон дараагийн бичлэг)
   Дэлхийн бүртгэлд шилжих үед `global_domain` бичлэг хийх) ба баталгаажуулах
   гарын үсэг зурахаас өмнө `cargo xtask address-vectors` гэсэн манифест.
4. **Баталгаажуулж, нийтлэх.** Runbook шалгах хуудсыг дагаарай (хэш, Sigstore,
   дарааллын монотон байдал) багцыг SoraFS болгон толин тусгахаас өмнө. Torii одоо
   багц газардсаны дараа шууд I105 (давуу)/sora (хоёр дахь шилдэг) литералуудыг каноникчилдаг.
5. **Хянах, буцаах.** Local‑8 болон Local‑12 мөргөлдөх самбарыг дараах дээр байлгаарай.
   30 хоногийн турш тэг; Хэрэв регресс илэрвэл өмнөх манифестийг дахин нийтлээрэй
   зөвхөн өртсөн үйлдвэрлэлийн бус орчинд телеметр тогтворжих хүртэл.

Дээрх бүх алхмууд нь ADDR‑7c-ийн зайлшгүй нотолгоо юм: ямар ч тохиолдолд илэрдэг
`cosign` гарын үсгийн багц эсвэл таарахгүй `previous_digest` утгууд байх ёстой
автоматаар татгалзаж, операторууд баталгаажуулалтын бүртгэлийг хавсаргах ёстой
тэдний солих тасалбар.

### 5. Wallet & API эргономик

- **Дэлгэцийн өгөгдмөл:** Түрийвч нь I105 хаягийг харуулдаг (богино, шалгах нийлбэр)
  дээр нь шийдвэрлэсэн домэйныг бүртгэлээс авчирсан шошго болгон нэмнэ. Домэйн нь
  өөрчлөгдөж болох тайлбарлах мета өгөгдөл гэж тодорхой тэмдэглэсэн бол I105 нь
  тогтвортой хаяг.
- **Оролтын каноникчилал:** Torii болон SDK нь I105 (давуу)/sora (хоёр дахь шилдэг)/0x-г хүлээн зөвшөөрдөг
  хаягууд дээр нэмэх нь `alias@domain` (rejected legacy form), `uaid:…`, болон
  `opaque:…` хэлбэрийг үүсгэж, дараа нь гаралтад зориулж I105 болгож каноникчилна. Байхгүй
  хатуу горимд шилжих; Түүхий утас/и-мэйл таниулагчийг бүртгэлээс гадуур байлгах ёстой
  UAID / тунгалаг бус зураглалаар дамжуулан.
- **Алдаанаас урьдчилан сэргийлэх:** Түрийвч нь I105 угтваруудыг задлан шинжилж, гинжин ялгаварлагчийг хэрэгжүүлдэг
  хүлээлт. Гинжин хэлхээний таарамжгүй байдал нь арга хэмжээ авах боломжтой оношилгооны тусламжтайгаар хүнд гэмтэл үүсгэдэг.
- **Кодекийн сангууд:** Албан ёсны Rust, TypeScript/JavaScript, Python, болон Kotlin
  номын сангууд нь I105 кодчилол/декодчилол болон шахсан (`sora`) дэмжлэг үзүүлдэг.
  хэсэгчилсэн хэрэгжилтээс зайлсхийх. CAIP-10 хөрвүүлэлтийг хараахан илгээгээгүй байна.

#### Хүртээмж, аюулгүй хуваалцах заавар- Бүтээгдэхүүний гадаргуугийн хэрэгжилтийн удирдамжийг шууд дагаж мөрддөг
  `docs/portal/docs/reference/address-safety.md`; шалгах хуудаснаас лавлана уу
  эдгээр шаардлагыг түрийвч эсвэл Explorer UX-д тохируулах.
- **Аюулгүй хуваалцах урсгал:** Хаягуудыг I105 маягт руу хуулж эсвэл харуулдаг гадаргуу нь өгөгдмөл тохиргоотой бөгөөд ижил ачааллаас гаралтай QR код болон бүтэн мөрийг хоёуланг нь харуулсан зэргэлдээх "хуваалцах" үйлдлийг ил гаргадаг тул хэрэглэгчид шалгах нийлбэрийг нүдээр эсвэл сканнердах замаар баталгаажуулах боломжтой. Таслахаас зайлсхийх боломжгүй үед (жишээ нь, жижиг дэлгэц) мөрийн эхлэл ба төгсгөлийг хадгалж, тодорхой эллипс нэмж, санамсаргүй хайчлахаас сэргийлэхийн тулд бүрэн хаягийг санах ойд хуулах замаар хандах боломжтой байлгаарай.
- **IME-ийн хамгаалалт:** Хаягийн оролтууд нь IME/IME загварын гаруудын найруулгын олдворуудаас татгалзах ёстой. Зөвхөн ASCII-н оруулгыг хэрэгжүүлэх, бүрэн өргөн эсвэл Кана тэмдэгтүүд илэрсэн үед шугаман дээр анхааруулга өгөх, баталгаажуулахаас өмнө тэмдэглэгээг нэгтгэсэн энгийн бичвэрт буулгах бүсийг санал болгосноор Япон, Хятад хэрэглэгчид ахиц дэвшилээ алдалгүйгээр IME-ээ идэвхгүй болгох боломжтой.
- **Дэлгэц уншигчийн дэмжлэг:** Тэргүүлэх I105 угтварын цифрүүдийг дүрслэн харуулах далд шошгуудыг (`aria-label`/`aria-describedby`) өгч, I105 ачааллыг 4 эсвэл 8 тэмдэгттэй бүлэг болгон хуваасан тул туслах тэмдэгтийн стринг уншдаг. Хуулбарлах/хуваалцах амжилтыг эелдэг шууд бүсээр дамжуулан зарлаж, QR урьдчилан үзэхэд тайлбарласан өөр текст (0x02F1 гинжин дэх <алиас>-ын I105 хаяг") орсон эсэхийг шалгаарай.
- **Зөвхөн Sora-н шахагдсан хэрэглээ:** `i105` шахсан харагдацыг үргэлж "Зөвхөн Sora" гэж тэмдэглэж, хуулахаасаа өмнө тодорхой баталгаажуулалтын ард байрлуул. Гинжин ялгаварлагч нь Sora Nexus утгагүй үед SDK болон түрийвч нь шахсан гаралтыг харуулахаас татгалзах ёстой бөгөөд мөнгийг буруу чиглүүлэхээс зайлсхийхийн тулд сүлжээ хоорондын шилжүүлэг хийхдээ хэрэглэгчдийг I105 руу буцаах ёстой.

## Хэрэгжилтийг шалгах хуудас

- **I105 дугтуй:** Угтвар нь `chain_discriminant`-г компакт ашиглан кодлодог.
  `encode_i105_prefix()`-ийн 6-/14 битийн схем, бие нь каноник байт юм
  (`AccountAddress::canonical_bytes()`), шалгах нийлбэр нь эхний хоёр байт байна
  Blake2b-512(`b"I105PRE"` || угтвар || бие). Бүрэн ачаалал нь I105 цагаан толгойг ашиглан `bs58`-ээр кодлогдоно.
- **Бүртгэлийн гэрээ:** гарын үсэг зурсан JSON (болон нэмэлт Merkle root) хэвлэл
  `{discriminant, i105_prefix, chain_alias, endpoints}` 24 цагийн TTL болон
  эргүүлэх товчлуурууд.
- **Домэйн бодлого:** ASCII `Name` өнөөдөр; i18n-г идэвхжүүлж байгаа бол UTS-46-г ашиглана уу
  хэвийн болгох ба төөрөгдүүлсэн шалгалтын UTS-39. Хамгийн их шошгыг хэрэгжүүлэх (63) болон
  нийт (255) урт.
- **Текстийн туслахууд:** Rust дахь I105 ↔ шахсан (`i105`) кодлогчийг илгээх,
  TypeScript/JavaScript, Python болон Kotlin, хуваалцсан тест векторуудтай (CAIP-10
  зураглал нь ирээдүйн ажил хэвээр байна).
- **CLI багаж:** `iroha tools address convert`-ээр дамжуулан тодорхойлогч операторын ажлын урсгалыг хангах
  (`crates/iroha_cli/src/address.rs`-г үзнэ үү), энэ нь I105/`0x…` литерал ба
  нэмэлт `<address>@<domain>` (rejected legacy form) шошго, өгөгдмөл нь Sora Nexus угтвар (`753`) ашиглан I105 гаралт,
  зөвхөн Sora-н шахагдсан цагаан толгойн үсгийг операторууд тодорхой хүсэлт тавьсан тохиолдолд л гаргадаг
  `--format i105` эсвэл JSON хураангуй горим. Энэ тушаал нь угтвар хүлээлтийг идэвхжүүлдэг
  задлан шинжилж, өгөгдсөн домэйн (JSON-д `input_domain`) болон `legacy  suffix` тугийг бичнэ
  хувиргасан кодчилолыг `<address>@<domain>` (rejected legacy form) гэж дахин тоглуулдаг тул илэрхий ялгаа нь эргономик хэвээр байна.
- **Wallet/explorer UX:** [хаяг харуулах удирдамж](source/sns/address_display_guidelines.md)-ыг дагаж мөрдөөрэй.
  ADDR-6-тай хамт илгээгдсэн—хос хуулбар товчлуурыг санал болгож, I105-г QR ачааллаар хадгалах, анхааруулах
  шахсан `i105` хэлбэр нь зөвхөн Sora-д зориулагдсан бөгөөд IME дахин бичихэд мэдрэмтгий байдаг.
- **Torii интеграцчилал:** Nexus кэш нь TTL, ялгаруулдаг
  `ForeignDomain`/`UnknownDomain`/`RegistryUnavailable` тодорхой, мөн
  keep strict account-literal parsing canonical-i105-only (reject compressed and any `@domain` suffix) with canonical i105 output.

### Torii хариултын формат

- `GET /v1/accounts` нь нэмэлт `canonical i105 rendering` асуулгын параметрийг хүлээн авах ба
  `POST /v1/accounts/query` JSON дугтуй доторх ижил талбарыг хүлээн авдаг.
  Дэмжигдсэн утгууд нь:
  - `i105` (өгөгдмөл) — хариултууд нь каноник I105 ачааллыг ялгаруулдаг (жишээ нь,
    `6cmzPVPX5jDQFNfiz6KgmVfm1fhoAqjPhoPFn4nx9mBWaFMyUCwq4cw`).
  - `i105_default` — хариултууд нь зөвхөн Sora-н `i105` шахсан харагдацыг гаргадаг.
    шүүлтүүр/замын параметрүүдийг каноник байлгах.
- Буруу утгууд `400` (`QueryExecutionFail::Conversion`) буцаана. Энэ нь зөвшөөрдөг
  түрийвч болон судлаачид зөвхөн Sora-н UX-д зориулсан шахсан мөрүүдийг хүсэх үед
  I105-г харилцан ажиллах боломжтой өгөгдмөл байдлаар хадгалах.
- Хөрөнгийн эзэмшигчийн жагсаалт (`GET /v1/assets/{definition_id}/holders`) болон тэдгээрийн JSON
  дугтуйны хамтрагч (`POST …/holders/query`) мөн `canonical i105 rendering` хүндэтгэдэг.
  `items[*].account_id` талбар нь шахагдсан литералуудыг ялгаруулдаг
  Параметр/дугтуйны талбарыг `i105_default` болгож тохируулсан бөгөөд энэ нь дансуудыг тусгах
  төгсгөлийн цэгүүд тул судлаачид лавлахуудын хооронд тогтвортой гаралтыг үзүүлэх боломжтой.
- **Туршилт:** Кодер/декодерын эргэлт, буруу гинжин хэлхээний нэгжийн туршилтуудыг нэмнэ үү
  бүтэлгүйтэл, ил тод хайлт; Torii болон SDK-д нэгтгэх хамрах хүрээг нэмнэ үү
  I105 урсгалын хувьд төгсгөл хүртэл.

## Алдааны кодын бүртгэл

Хаягийн кодлогч болон декодчилогч нь алдааг илрүүлдэг
`AccountAddressError::code_str()`. Дараах хүснэгтэд тогтвортой кодуудыг харуулав
SDK, түрийвч болон Torii гадаргуу нь хүний унших боломжтой хажуугийн гадаргуутай байх ёстой.
мессеж, мөн санал болгосон засварын удирдамж.

### Каноник барилга

| Код | Бүтэлгүйтэл | Зөвлөмж болгож буй засвар |
|------|---------|-------------------------|
| `ERR_UNSUPPORTED_ALGORITHM` | Кодлогч нь бүртгэл эсвэл бүтээх функцээр дэмжигдээгүй гарын үсэг зурах алгоритмыг хүлээн авсан. | Бүртгэл болон тохиргоонд идэвхжүүлсэн муруйгаар дансны бүтцийг хязгаарлах. |
| `ERR_KEY_PAYLOAD_TOO_LONG` | Түлхүүрийн даацын уртыг гарын үсэг зурах нь дэмжигдсэн хязгаараас хэтэрсэн байна. | Нэг товчлуурт хянагч нь `u8` урттай хязгаарлагддаг; том нийтийн түлхүүрүүдэд multisig ашиглах (жишээ нь, ML-DSA). |
| `ERR_INVALID_HEADER_VERSION` | Хаягийн толгой хэсгийн хувилбар дэмжигдсэн хязгаараас гадуур байна. | V1 хаягийн гарчиг `0` хувилбарыг гаргах; шинэ хувилбаруудыг нэвтрүүлэхээс өмнө кодлогчдыг шинэчлэх. |
| `ERR_INVALID_NORM_VERSION` | Хэвийн хувилбарын тугийг танихгүй байна. | `1` нормчлолын хувилбарыг ашиглаж, нөөцлөгдсөн битүүдийг солихоос зайлсхий. |
| `ERR_INVALID_I105_PREFIX` | Хүссэн I105 сүлжээний угтварыг кодлох боломжгүй. | Сүлжээний бүртгэлд нийтлэгдсэн `0..=16383` хүрээн дэх угтварыг сонгоно уу. |
| `ERR_CANONICAL_HASH_FAILURE` | Каноник ачааллыг хэшлэх амжилтгүй боллоо. | Үйлдлийг дахин оролдох; Хэрэв алдаа гарсаар байвал үүнийг хэшний стек дэх дотоод алдаа гэж үзнэ үү. |

### Формат тайлах болон автоматаар илрүүлэх

| Код | Бүтэлгүйтэл | Зөвлөмж болгож буй засвар |
|------|---------|-------------------------|
| `ERR_INVALID_I105_ENCODING` | I105 мөр нь цагаан толгойн гаднах тэмдэгтүүдийг агуулна. | Хаяг нь хэвлэгдсэн I105 цагаан толгойг ашиглаж байгаа бөгөөд хуулах/буулгах явцад таслагдахгүй байгаа эсэхийг шалгаарай. |
| `ERR_INVALID_LENGTH` | Ачааны урт нь сонгогч/хянагчийн хүлээгдэж буй стандарт хэмжээтэй таарахгүй байна. | Сонгосон домэйн сонгогч болон хянагчийн байршлын бүрэн каноник ачааллыг хангана. |
| `ERR_CHECKSUM_MISMATCH` | I105 (давуу) эсвэл шахсан (`sora`, хоёрдугаарт) шалгах нийлбэрийг баталгаажуулж чадсангүй. | Итгэмжлэгдсэн эх сурвалжаас хаягийг сэргээх; Энэ нь ихэвчлэн хуулж буулгах алдааг илэрхийлдэг. |
| `ERR_INVALID_I105_PREFIX_ENCODING` | I105 угтвар байт алдаатай байна. | Тохиромжтой кодлогчоор хаягийг дахин кодлох; тэргүүлэх I105 байтыг гараар бүү өөрчил. |
| `ERR_INVALID_HEX_ADDRESS` | Каноник арван зургаатын хэлбэрийг тайлж чадсангүй. | Албан ёсны кодлогчийн үйлдвэрлэсэн `0x` угтвартай, тэгш урттай зургаан өнцөгт мөрийг өгнө үү. |
| `ERR_MISSING_COMPRESSED_SENTINEL` | Шахсан хэлбэр нь `sora`-ээр эхэлдэггүй. | Шахсан Сора хаягуудыг декодлогчдод өгөхөөс өмнө шаардлагатай харуулын угтвар бичнэ. |
| `ERR_COMPRESSED_TOO_SHORT` | Шахсан мөрөнд ачаалал болон шалгах нийлбэрт хангалттай цифр байхгүй байна. | Таслагдсан хэсгүүдийн оронд кодлогчоос ялгарах бүрэн шахагдсан мөрийг ашиглана уу. |
| `ERR_INVALID_COMPRESSED_CHAR` | Шахсан цагаан толгойн гаднах тэмдэгт тулгарлаа. | Нийтлэгдсэн хагас өргөн/бүтэн өргөнтэй хүснэгтээс тэмдэгтийг хүчинтэй Base‑105 глифээр солино уу. |
| `ERR_INVALID_COMPRESSED_BASE` | Кодлогч дэмжигдээгүй радикс ашиглахыг оролдсон. | Кодлогчийн эсрэг алдаа гаргах; шахсан цагаан толгой нь V1 дэх radix 105-д бэхлэгдсэн байна. |
| `ERR_INVALID_COMPRESSED_DIGIT` | Цифрийн утга нь шахсан цагаан толгойн хэмжээнээс хэтэрсэн байна. | Цифр бүр `0..105)` дотор байгаа эсэхийг шалгаарай, шаардлагатай бол хаягийг сэргээнэ үү. |
| `ERR_UNSUPPORTED_ADDRESS_FORMAT` | Автоматаар илрүүлэх нь оролтын форматыг таньж чадсангүй. | Парсеруудыг дуудахдаа I105 (давуу эрхтэй), шахсан (`sora`) эсвэл каноник `0x` зургаан өнцөгт мөрүүдийг оруулна уу. |

### Домэйн болон сүлжээний баталгаажуулалт| Код | Бүтэлгүйтэл | Зөвлөмж болгож буй засвар |
|------|---------|-------------------------|
| `ERR_DOMAIN_MISMATCH` | Домэйн сонгогч нь хүлээгдэж буй домайнтай таарахгүй байна. | Төлөвлөсөн домэйнд олгосон хаягийг ашиглах эсвэл хүлээлтийг шинэчлэх. |
| `ERR_INVALID_DOMAIN_LABEL` | Домэйн шошгыг хэвийн болгох шалгалт амжилтгүй боллоо. | Кодлохын өмнө UTS-46 шилжилтийн бус боловсруулалтыг ашиглан домайныг каноникчил. |
| `ERR_UNEXPECTED_NETWORK_PREFIX` | Шифрлэгдсэн I105 сүлжээний угтвар нь тохируулсан утгаас ялгаатай байна. | Зорилтот сүлжээнээс хаяг руу шилжих эсвэл хүлээгдэж буй ялгаварлагч/угтварыг тохируулна уу. |
| `ERR_UNKNOWN_ADDRESS_CLASS` | Хаягийн ангийн битүүд танигдахгүй. | Декодерыг шинэ ангиллыг ойлгодог хувилбар болгон шинэчлэх, эсвэл толгойн битүүдийг өөрчлөхөөс зайлсхий. |
| `ERR_UNKNOWN_DOMAIN_TAG` | Домэйн сонгогчийн шошго тодорхойгүй байна. | Сонгогчийн шинэ төрлийг дэмждэг хувилбар руу шинэчлэх эсвэл V1 зангилаанууд дээр туршилтын ачааллыг ашиглахаас зайлсхий. |
| `ERR_UNEXPECTED_EXTENSION_FLAG` | Нөөцлөгдсөн өргөтгөлийн битийг тохируулсан. | Хадгалсан битүүдийг арилгах; ирээдүйн ABI тэднийг танилцуулах хүртэл тэд хаалгатай хэвээр байна. |
| `ERR_UNKNOWN_CONTROLLER_TAG` | Хянагчийн ачааны шошгыг танихгүй байна. | Шинжилгээ хийхээсээ өмнө шинэ хянагч төрлүүдийг танихын тулд декодерыг сайжруул. |
| `ERR_UNEXPECTED_TRAILING_BYTES` | Кодыг тайлсны дараа арын байтыг агуулсан каноник ачаалал. | Каноник ачааллыг сэргээх; зөвхөн баримтжуулсан урт байх ёстой. |

### Controller Payload Validation

| Код | Бүтэлгүйтэл | Зөвлөмж болгож буй засвар |
|------|---------|-------------------------|
| `ERR_INVALID_PUBLIC_KEY` | Түлхүүр байт нь зарласан муруйтай таарахгүй байна. | Түлхүүр байтууд нь сонгосон муруйд яг шаардлагатай байдлаар кодлогдсон эсэхийг шалгаарай (жишээ нь, 32 байт Ed25519). |
| `ERR_UNKNOWN_CURVE` | Муруй тодорхойлогч бүртгэгдээгүй байна. | Нэмэлт муруйг баталж, бүртгэлд нийтлэх хүртэл `1` (Ed25519) муруй ID-г ашиглана уу. |
| `ERR_MULTISIG_MEMBER_OVERFLOW` | Multisig хянагч дэмжигдсэнээс илүү олон гишүүнийг зарладаг. | Кодлохын өмнө multisig гишүүнчлэлийг баримтжуулсан хязгаар хүртэл бууруулна уу. |
| `ERR_INVALID_MULTISIG_POLICY` | Multisig бодлогын ачааллыг баталгаажуулж чадсангүй (босго/жин/схем). | Бодлогыг CTAP2 схем, жингийн хязгаар, босго хязгаарлалтыг хангахын тулд дахин бүтээнэ үү. |

## Альтернатив хувилбаруудыг авч үзсэн

- **Pure checksum envelope (Bitcoin-style).** Илүү хялбар шалгах нийлбэр боловч алдаа илрүүлэх сул
  Blake2b-ээс авсан I105 шалгах нийлбэрээс (`encode_i105` 512 битийн хэшийг багасгадаг)
  мөн 16 битийн ялгаварлагчдад зориулсан тодорхой угтвар семантик байхгүй.
- **Домэйн мөрөнд гинжин хэлхээний нэрийг оруулах (жишээ нь, `finance@chain`).** Завсарлага
- **Хаяг өөрчлөхгүйгээр зөвхөн Nexus чиглүүлэлтэд найдаж болно.** Хэрэглэгчид
  хоёрдмол утгатай мөрүүдийг хуулах/оруулах; Бид хаягийг өөрөө контексттэй байлгахыг хүсч байна.
- **Bech32m дугтуй.** QR-д ээлтэй бөгөөд хүн унших боломжтой угтварыг санал болгодог боловч
  тээвэрлэлтийн I105 хэрэгжилтээс ялгаатай байх болно (`AccountAddress::to_i105`)
  мөн бүх бэхэлгээ/SDK-г дахин үүсгэх шаардлагатай. Одоогийн замын зураг нь I105 +-г хадгалдаг
  шахсан (`sora`) дэмжлэг, цаашдын судалгааг үргэлжлүүлэх
  Bech32m/QR давхаргууд (CAIP-10 зураглалыг хойшлуулсан).

## Нээлттэй асуултууд

- `u16` ялгаварлан гадуурхагч ба нөөцлөгдсөн мужууд нь урт хугацааны эрэлтийг хамарч байгааг баталгаажуулах;
  өөрөөр хэлбэл `u32`-г varint кодчилолоор үнэлнэ үү.
- Бүртгэлийн шинэчлэлтийн олон гарын үсэг бүхий засаглалын үйл явцыг эцэслэх, хэрхэн хийх
  хүчингүй болгох/хугацаа нь дууссан хуваарилалтыг зохицуулдаг.
- Манифест гарын үсгийн бүдүүвчийг нарийн тодорхойлох (жишээ нь, Ed25519 олон тэмдэгт) болон
  Nexus түгээлтийн тээврийн аюулгүй байдал (HTTPS pinning, IPFS хэш формат).
- Шилжин суурьшихад домэйн нэр/дахин чиглүүлэлтийг дэмжих эсэх, яаж хийхийг тодорхойлох
  детерминизмыг эвдэхгүйгээр тэдгээрийг гадаргуу дээр гаргах.
- Kotodama/IVM гэрээ I105 туслахуудад хэрхэн ханддаг болохыг зааж өгнө үү (`to_address()`,
  `parse_address()`) болон гинжин хадгалалт нь CAIP-10-г хэзээ нэгэн цагт ил гаргах эсэх
  зураглал (өнөөдөр I105 бол каноник).
- Iroha гинжийг гадаад бүртгэлд бүртгэх талаар судлах (жишээ нь, I105 бүртгэл,
  CAIP нэрийн зайны лавлах) экосистемийг илүү өргөн хүрээнд тохируулах боломжтой.

## Дараагийн алхамууд

1. I105 кодчилол нь `iroha_data_model` (`AccountAddress::to_i105`,
   `parse_encoded`); SDK болгонд бэхэлгээ/туршилтыг үргэлжлүүлж, бүгдийг нь цэвэрлэ
   Bech32m орлуулагч.
2. Тохиргооны схемийг `chain_discriminant`-ээр сунгаж, мэдрэмжтэй гарга.
  одоо байгаа тест/хөгжүүлэгчийн тохиргооны өгөгдмөл. **(Гуйсан: `common.chain_discriminant`
  одоо `iroha_config` хувилбараар илгээгдэж байгаа бөгөөд үндсэндээ сүлжээ тус бүрээр `0x02F1` байна.
  хүчингүй болгодог.)**
3. Nexus бүртгэлийн схем болон нотлох баримтын манифест нийтлэгчийг боловсруулах.
4. Хүний хүчин зүйлийн талаар хэтэвч нийлүүлэгчид болон асран хамгаалагчдаас санал хүсэлтийг цуглуулах
   (HRP нэршил, дэлгэцийн формат).
5. Баримт бичгийг шинэчилнэ үү (`docs/source/data_model.md`, Torii API баримтууд)
   хэрэгжүүлэх арга замыг тодорхойлсон.
6. Албан ёсны кодлогчийн сангуудыг (Rust/TS/Python/Kotlin) нормативын туршилтаар илгээнэ үү.
   амжилт, бүтэлгүйтлийн тохиолдлуудыг хамарсан векторууд.
