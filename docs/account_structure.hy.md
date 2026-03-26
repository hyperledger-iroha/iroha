---
lang: hy
direction: ltr
source: docs/account_structure.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 01561366c9698c10d29ff3f49ad4c14b22b1796b5c7701cf98d200a140af1caf
source_last_modified: "2026-01-28T17:11:30.635172+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Հաշվի կառուցվածք RFC

** Կարգավիճակը. ** Ընդունված է (ADDR-1)  
**Հանդիսատես.** Տվյալների մոդել, Torii, Nexus, Դրամապանակ, Կառավարման թիմեր  
**Հարակից խնդիրներ.** TBD

## Ամփոփում

Այս փաստաթուղթը նկարագրում է առաքման հաշիվ-հասցեների փաթեթը, որն իրականացվել է
`AccountAddress` (`crates/iroha_data_model/src/account/address.rs`) և
ուղեկից գործիքավորում: Այն ապահովում է.

- Ստուգիչ, մարդուն ուղղված **I105 հասցե** արտադրված է
  `AccountAddress::to_i105`, որը շղթայական տարբերակիչ է կապում հաշվի հետ
  վերահսկիչ և առաջարկում է դետերմինիստական փոխգործակցության համար հարմար տեքստային ձևեր:
- Դոմենի ընտրիչներ անուղղակի լռելյայն տիրույթների և տեղական ամփոփումների համար՝ a
  վերապահված գլոբալ ռեգիստրի ընտրիչ պիտակ ապագա Nexus-ով ապահովված երթուղման համար (
  ռեեստրի որոնումը **դեռևս չի առաքվել**):

## Մոտիվացիա

Դրամապանակներն ու շղթայից դուրս գործիքավորումն այսօր հիմնված են չմշակված `alias@domain` (rejected legacy form) երթուղային այլանունների վրա: Սա
ունի երկու հիմնական թերություն.

1. **Ցանցային կապ չկա:** Տողը չունի ստուգիչ գումար կամ շղթայի նախածանց, ուստի օգտվողները
   կարող է տեղադրել հասցե սխալ ցանցից՝ առանց անմիջական արձագանքի: Այն
   գործարքը, ի վերջո, կմերժվի (շղթայի անհամապատասխանություն) կամ, ավելի վատ, հաջողվի
   չնախատեսված հաշվի դեմ, եթե նպատակակետը առկա է տեղում:
2. **Դոմենի բախում։** Դոմենները միայն անվանումների համար են և կարող են վերօգտագործվել յուրաքանչյուրի համար։
   շղթա. Ծառայությունների դաշնություն (պահառուներ, կամուրջներ, փոխշղթայական աշխատանքային հոսքեր)
   դառնում է փխրուն, քանի որ `finance` A շղթայում կապ չունի `finance`-ի հետ
   շղթա Բ.

Մեզ անհրաժեշտ է մարդու համար հարմար հասցեի ձևաչափ, որը պաշտպանում է պատճենահանման/տեղադրման սխալներից
և դետերմինիստական քարտեզագրում տիրույթի անունից մինչև հեղինակավոր շղթա:

## Գոլեր

- Նկարագրեք տվյալների մոդելում ներդրված I105 ծրարը և
  կանոնական վերլուծության/փոխանունի կանոններ, որոնց հետևում են `AccountId` և `AccountAddress`:
- Կոդավորեք կազմաձևված շղթայի դիսկրիմինատորը անմիջապես յուրաքանչյուր հասցեի մեջ և
  սահմանել դրա կառավարման/գրանցման գործընթացը:
- Նկարագրեք, թե ինչպես կարելի է ներդնել տիրույթի գլոբալ ռեգիստր առանց ընդհատելու ընթացիկ
  տեղակայումներ և սահմանել նորմալացման/խաբեության դեմ կանոններ:

## Ոչ գոլեր

- Ակտիվների միջշղթայական փոխանցումների իրականացում: Ուղղորդող շերտը միայն վերադարձնում է
  թիրախային շղթա.
- Համաշխարհային տիրույթի թողարկման կառավարման վերջնական ավարտ: Այս RFC-ն կենտրոնանում է տվյալների վրա
  մոդելային և տրանսպորտային պրիմիտիվներ:

## Նախապատմություն

### Ընթացիկ երթուղային կեղծանուն

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

`ChainId`-ն ապրում է `AccountId`-ից դուրս: Հանգույցները ստուգում են գործարքի `ChainId`
Ընդունման ժամանակ կոնֆիգուրացիայի դեմ (`AcceptTransactionFail::ChainIdMismatch`)
և մերժել օտարերկրյա գործարքները, սակայն հաշվի տողը ինքնին կրում է ոչ
ցանցի հուշում.

### Դոմենի նույնացուցիչներ

`DomainId`-ը փաթաթում է `Name` (նորմալացված տողը) և տարածվում է տեղական շղթայի վրա:
Յուրաքանչյուր շղթա կարող է ինքնուրույն գրանցել `wonderland`, `finance` և այլն։

### Nexus համատեքստ

Nexus-ը պատասխանատու է խաչաձեւ բաղադրիչների համակարգման համար (գծեր/տվյալների տարածություններ): Այն
ներկայումս չունի խաչաձեւ շղթայական տիրույթի երթուղի հասկացություն:

## Առաջարկվող դիզայն

### 1. Դետերմինիստական շղթայական դիսկրիմինանտ

`iroha_config::parameters::actual::Common`-ն այժմ բացահայտում է.

```rust
pub struct Common {
    pub chain: ChainId,
    pub chain_discriminant: u16, // globally coordinated
    // ... existing fields
}
```

- **Սահմանափակումներ:**
  - Եզակի մեկ ակտիվ ցանցի համար; կառավարվում է ստորագրված հանրային ռեգիստրի միջոցով
    բացահայտ վերապահված ընդգրկույթներ (օրինակ՝ `0x0000–0x0FFF` թեստ/dev, `0x1000–0x7FFF`
    համայնքային հատկացումներ, `0x8000–0xFFEF` կառավարման կողմից հաստատված, `0xFFF0–0xFFFF`
    վերապահված):
  - Անփոփոխ է վազող շղթայի համար: Այն փոխելը պահանջում է կոշտ պատառաքաղ և ա
    ռեեստրի թարմացում:
- **Կառավարում և գրանցում (պլանավորված).
  պահպանել ստորագրված JSON գրանցամատյան, որը գծագրում է տարբերիչները մարդկային անուններով և
  CAIP-2 նույնացուցիչներ. Այս ռեեստրը դեռևս առաքված գործարկման ժամանակի մաս չէ:
- **Օգտագործում․
  յուրաքանչյուր բաղադրիչ կարող է ներկառուցել կամ վավերացնել այն: CAIP-2 բացահայտումը մնում է ապագա
  փոխգործակցության առաջադրանք.

### 2. Կանոնական հասցեների կոդեկներ

Rust տվյալների մոդելը բացահայտում է մեկ կանոնական բեռի ներկայացում
(`AccountAddress`), որը կարող է արտանետվել որպես մարդու դեմ ուղղված մի քանի ձևաչափեր: I105 է
նախընտրելի հաշվի ձևաչափը համօգտագործման և կանոնական ելքի համար. սեղմվածը
`sora` ձևը երկրորդ լավագույն տարբերակն է միայն Sora-ի համար UX-ի համար, որտեղ կանա այբուբենը
ավելացնում է արժեք: Canonical hex-ը մնում է վրիպազերծման օգնություն:

- **I105** – I105 ծրար, որը ներառում է շղթան
  խտրական. Ապակոդավորիչները վավերացնում են նախածանցը նախքան օգտակար բեռը խթանելը
  կանոնական ձևը.
- **Սորայի սեղմված տեսք** – միայն Սորա այբուբեն՝ **105 սիմվոլներից**, որը կառուցվել է
  58 նիշից բաղկացած イロハ բանաստեղծությունը (ներառյալ ヰ և ヱ) ավելացնելով
  I105 հավաքածու. Տողերը սկսվում են պահակային `sora`-ով, ներդնում են Bech32m-ից ստացված
  checksum-ը և բաց թողեք ցանցի նախածանցը (Sora Nexus-ը ենթադրում է պահակ):

  ```
  I105  : 123456789ABCDEFGHJKMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz
  Iroha : ｲﾛﾊﾆﾎﾍﾄﾁﾘﾇﾙｦﾜｶﾖﾀﾚｿﾂﾈﾅﾗﾑｳヰﾉｵｸﾔﾏｹﾌｺｴﾃｱｻｷﾕﾒﾐｼヱﾋﾓｾｽ
  ```
- **Կանոնական վեցանկյուն** – կանոնական բայթի վրիպազերծման համար հարմար `0x…` կոդավորում
  ծրար.

`AccountAddress::parse_encoded`-ը ավտոմատ կերպով հայտնաբերում է I105 (նախընտրելի), սեղմված (`sora`, երկրորդ լավագույն) կամ կանոնական վեցանկյուն
(միայն `0x...`; բաց վեցանկյունը մերժվում է) մուտքագրում և վերադարձնում է ինչպես վերծանված օգտակար բեռը, այնպես էլ հայտնաբերվածը
`AccountAddress`. Torii-ն այժմ զանգահարում է `parse_encoded`՝ ISO 20022-ի լրացուցիչ համար
հասցեագրում և պահպանում է կանոնական վեցանկյուն ձևը, որպեսզի մետատվյալները մնան դետերմինիստական
անկախ սկզբնական ներկայացումից:

#### 2.1 Վերնագրի բայթ դասավորություն (ADDR-1a)

Յուրաքանչյուր կանոնական ծանրաբեռնվածություն դրված է որպես `header · controller`: Այն
`header`-ը մեկ բայթ է, որը հաղորդում է, թե որ վերլուծիչի կանոնները կիրառվում են բայթերի վրա, որոնք
հետևել.

```
bit index:   7        5 4      3 2      1 0
             ┌─────────┬────────┬────────┬────┐
payload bit: │version  │ class  │  norm  │ext │
             └─────────┴────────┴────────┴────┘
```

Առաջին բայթը, հետևաբար, փաթեթավորում է սխեմայի մետատվյալները ներքևում գտնվող ապակոդավորիչների համար.

| Բիթ | Դաշտային | Թույլատրելի արժեքներ | Սխալ խախտման վերաբերյալ |
|------|-------|-------------------------------------|
| 7-5 | `addr_version` | `0` (v1): `1-7` արժեքները վերապահված են ապագա վերանայումների համար: | `0-7` `AccountAddressError::InvalidHeaderVersion` ձգանից դուրս արժեքներ; իրականացումները ՊԵՏՔ Է վերաբերվեն ոչ զրոյական տարբերակներին որպես չաջակցվող այսօր: |
| 4-3 | `addr_class` | `0` = մեկ բանալի, `1` = բազմաչափ: | Այլ արժեքները բարձրացնում են `AccountAddressError::UnknownAddressClass`: |
| 2-1 | `norm_version` | `1` (Նորմ v1): `0`, `2`, `3` արժեքները վերապահված են: | `0-3`-ից դուրս արժեքները բարձրացնում են `AccountAddressError::InvalidNormVersion`: |
| 0 | `ext_flag` | ՊԵՏՔ է լինի `0`: | Սահմանված բիթը բարձրացնում է `AccountAddressError::UnexpectedExtensionFlag`: |

Rust կոդավորիչը գրում է `0x02` մեկ բանալիով կարգավորիչների համար (տարբերակ 0, դաս 0,
նորմ v1, ընդլայնման դրոշը մաքրված է) և `0x0A` բազմասիգ կարգավորիչների համար (տարբերակ 0,
դաս 1, նորմ v1, ընդլայնման դրոշը մաքրված է):

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

#### 2.3 Վերահսկիչի օգտակար բեռնվածության կոդավորումներ (ADDR-1a)

Կարգավորիչի ծանրաբեռնվածությունը մեկ այլ հատկորոշված միավորում է, որը կցվում է տիրույթի ընտրիչից հետո.| Պիտակ | Վերահսկիչ | Դասավորություն | Ծանոթագրություններ |
|-----|------------|--------|-------|
| `0x00` | Մեկ բանալի | `curve_id:u8` · `key_len:u8` · `key_bytes` | `curve_id=0x01` քարտեզները Ed25519 այսօր. `key_len`-ը սահմանափակված է `u8`-ով; ավելի մեծ արժեքները բարձրացնում են `AccountAddressError::KeyPayloadTooLong` (հետևաբար, մեկ բանալիով ML-DSA հանրային բանալիները, որոնք ավելի քան 255 բայթ են, չեն կարող կոդավորվել և պետք է օգտագործեն բազմաչափ): |
| `0x01` | Multisig | `version:u8` · `threshold:u16` · `member_count:u8` · (`curve_id:u8` · `weight:u16` · `key_len:u16` · `member_count:u8`)* | Աջակցում է մինչև 255 անդամների (`CONTROLLER_MULTISIG_MEMBER_MAX`): Անհայտ կորերը բարձրացնում են `AccountAddressError::UnknownCurve`; սխալ ձևավորված քաղաքականությունները փուչիկ են դառնում որպես `AccountAddressError::InvalidMultisigPolicy`: |

Multisig-ի քաղաքականությունը նաև բացահայտում է CTAP2-ի ոճի CBOR քարտեզը և կանոնական ամփոփումը
հոսթները և SDK-ները կարող են դետերմինիստորեն ստուգել վերահսկիչը: Տես
`docs/source/references/multisig_policy_schema.md` (ADDR-1c) սխեմայի համար,
վավերացման կանոններ, հեշինգի ընթացակարգ և ոսկե հարմարանքներ:

Բոլոր հիմնական բայթերը կոդավորված են ճիշտ այնպես, ինչպես վերադարձվել է `PublicKey::to_bytes`-ի կողմից; ապակոդավորիչները վերակառուցում են `PublicKey` օրինակները և բարձրացնում `AccountAddressError::InvalidPublicKey`, եթե բայթերը չեն համապատասխանում հայտարարված կորին:

> **Ed25519 կանոնական կիրարկում (ADDR-3a):** կորի `0x01` ստեղները պետք է վերծանվեն ստորագրողի կողմից թողարկված ճշգրիտ բայթ տողի վրա և չպետք է ընկնեն փոքր կարգի ենթախմբում: Հանգույցներն այժմ մերժում են ոչ կանոնական կոդավորումները (օրինակ՝ `2^255-19` մոդուլի նվազեցված արժեքները) և թույլ կետերը, ինչպիսին է նույնականացման տարրը, ուստի SDK-ները պետք է հայտնվեն համապատասխան վավերացման սխալներ՝ նախքան հասցեներ ուղարկելը:

##### 2.3.1 Կորի նույնացուցիչի ռեեստր (ADDR-1d)

| ID (`curve_id`) | Ալգորիթմ | Խաղարկային դարպաս | Ծանոթագրություններ |
|-----------------|-----------|--------------|------|
| `0x00` | Պահպանված է | — | ՉՊԵՏՔ Է արտանետվի; ապակոդավորիչներ մակերեսային `ERR_UNKNOWN_CURVE`: |
| `0x01` | Ed25519 | — | Կանոնական v1 ալգորիթմ (`Algorithm::Ed25519`); միացված է լռելյայն կազմաձևում: |
| `0x02` | ML-DSA (Դիլիթիում3) | — | Օգտագործում է Dilithium3 հանրային բանալի բայթերը (1952 բայթ): Մեկ բանալի հասցեները չեն կարող կոդավորել ML-DSA, քանի որ `key_len`-ը `u8` է; multisig-ը օգտագործում է `u16` երկարություններ: |
| `0x03` | BLS12‑381 (նորմալ) | `bls` | Հանրային բանալիներ G1-ում (48 բայթ), ստորագրությունները G2-ում (96 բայթ): |
| `0x04` | secp256k1 | — | Դետերմինիստական ​​ECDSA-ն SHA-256-ի նկատմամբ; հանրային բանալիներն օգտագործում են 33 բայթ SEC1 սեղմված ձևը, իսկ ստորագրությունները՝ 64 բայթ `r∥s` կանոնական դասավորությունը: |
| `0x05` | BLS12‑381 (փոքր) | `bls` | Հանրային բանալիներ G2-ում (96 բայթ), ստորագրությունները G1-ում (48 բայթ): |
| `0x0A` | ԳՕՍՏ Ռ 34.10-2012 (256, հավաքածու Ա) | `gost` | Հասանելի է միայն այն դեպքում, երբ `gost` գործառույթը միացված է: |
| `0x0B` | ԳՕՍՏ Ռ 34.10-2012 (256, հավաքածու B) | `gost` | Հասանելի է միայն այն դեպքում, երբ `gost` գործառույթը միացված է: |
| `0x0C` | ԳՕՍՏ Ռ 34.10-2012 (256, հավաքածու Գ) | `gost` | Հասանելի է միայն այն դեպքում, երբ `gost` գործառույթը միացված է: |
| `0x0D` | ԳՕՍՏ Ռ 34.10-2012 (512, հավաքածու Ա) | `gost` | Հասանելի է միայն այն դեպքում, երբ `gost` գործառույթը միացված է: |
| `0x0E` | ԳՕՍՏ Ռ 34.10-2012 (512, հավաքածու B) | `gost` | Հասանելի է միայն այն դեպքում, երբ `gost` գործառույթը միացված է: |
| `0x0F` | SM2 | `sm` | DistID երկարություն (u16 BE) + DistID բայթ + 65 բայթ SEC1 չսեղմված SM2 բանալի; հասանելի է միայն այն դեպքում, երբ `sm`-ը միացված է: |

`0x06–0x09` բնիկները մնում են չնշանակված լրացուցիչ կորերի համար; ներկայացնելով նոր
ալգորիթմը պահանջում է ճանապարհային քարտեզի թարմացում և համապատասխան SDK/հյուրընկալող ծածկույթ: Կոդավորիչներ
ՊԵՏՔ Է մերժել ցանկացած չաջակցվող ալգորիթմ `ERR_UNSUPPORTED_ALGORITHM`-ով, և
ապակոդավորիչները ՊԵՏՔ Է արագ խափանվեն անհայտ ID-ներով `ERR_UNKNOWN_CURVE`-ով, որպեսզի պահպանվեն
ձախողված փակ վարքագիծ:

Կանոնական ռեգիստրը (ներառյալ մեքենայաընթեռնելի JSON արտահանումը) գործում է
[`docs/source/references/address_curve_registry.md`](source/references/address_curve_registry.md):
Գործիքավորումը ՊԵՏՔ Է ուղղակիորեն սպառի այդ տվյալների բազան, որպեսզի կորի նույնացուցիչները մնան
հետևողական է SDK-ների և օպերատորների աշխատանքային հոսքերի միջև:

- **SDK gating.** SDK-ների կանխադրված է միայն Ed25519 վավերացում/կոդավորում: Սվիֆթը մերկացնում է
  կոմպիլյացիայի ժամանակի դրոշակներ (`IROHASWIFT_ENABLE_MLDSA`, `IROHASWIFT_ENABLE_GOST`,
  `IROHASWIFT_ENABLE_SM`); Java/Android SDK-ն պահանջում է
  `AccountAddress.configureCurveSupport(...)`; JavaScript SDK-ն օգտագործում է
  `configureCurveSupport({ allowMlDsa: true, allowGost: true, allowSm2: true })`.
  secp256k1 աջակցությունը հասանելի է, բայց լռելյայն միացված չէ JS/Android-ում
  SDK-ներ; Զանգահարողները պետք է հստակորեն միանան, երբ թողարկեն ոչ-Ed25519 կարգավորիչներ:
- **Հյուրընկալող մուտք.** `Register<Account>`-ը մերժում է վերահսկիչները, որոնց ստորագրողները օգտագործում են ալգորիթմներ
  բացակայում է հանգույցի `crypto.allowed_signing` ցուցակից **կամ** կորի նույնացուցիչները, որոնք բացակայում են
  `crypto.curves.allowed_curve_ids`, ուստի կլաստերները պետք է գովազդեն աջակցությունը (կոնֆիգուրացիա +
  genesis) նախքան ML-DSA/GOST/SM կարգավարների գրանցումը: BLS վերահսկիչ
  ալգորիթմները միշտ թույլատրվում են կազմվելիս (համաձայնության ստեղները հիմնված են դրանց վրա),
  և լռելյայն կոնֆիգուրացիան հնարավորություն է տալիս Ed25519 + secp256k1:【crates/iroha_core/src/smartcontracts/isi/domain.rs:32】

##### 2.3.2 Multisig կարգավորիչի ուղեցույց

`AccountController::Multisig`-ը սերիականացնում է քաղաքականությունը միջոցով
`crates/iroha_data_model/src/account/controller.rs` և կիրառում է սխեման
փաստաթղթավորված [`docs/source/references/multisig_policy_schema.md`] (source/references/multisig_policy_schema.md):
Իրականացման հիմնական մանրամասները.

- Քաղաքականությունները կարգավորվել և վավերացվել են նախկինում `MultisigPolicy::validate()`-ի կողմից
  ներկառուցված լինելը: Շեմերը պետք է լինեն ≥1 և ≤Σ քաշ; կրկնօրինակ անդամներ են
  որոշիչ կերպով հեռացվել է `(algorithm || 0x00 || key_bytes)`-ով տեսակավորելուց հետո:
- Երկուական կարգավորիչի օգտակար բեռը (`ControllerPayload::Multisig`) կոդավորում է
  `version:u8`, `threshold:u16`, `member_count:u8`, ապա յուրաքանչյուր անդամի
  `(curve_id, weight:u16, key_len:u16, key_bytes)`. Սա հենց այն է
  `AccountAddress::canonical_bytes()`-ը գրում է I105 (նախընտրելի)/sora (երկրորդ լավագույն) օգտակար բեռներին:
- Հաշինգը (`MultisigPolicy::digest_blake2b256()`) օգտագործում է Blake2b-256-ը
  `iroha-ms-policy` անհատականացման տողը, որպեսզի կառավարման դրսևորումները կարողանան կապվել a
  դետերմինիստական քաղաքականության ID, որը համապատասխանում է I105-ում ներկառուցված վերահսկիչ բայթերին:
- Սարքավորումների ծածկույթն ապրում է `fixtures/account/address_vectors.json`-ում (պատյաններում
  `addr-multisig-*`): Դրամապանակները և SDK-ները պետք է հաստատեն I105 կանոնական տողերը
  ստորև՝ հաստատելու, որ դրանց կոդավորիչները համապատասխանում են Rust-ի իրականացմանը:

| Գործի ID | շեմ ​​/ անդամներ | I105 բառացի (նախածանց `0x02F1`) | Սորա սեղմված (`sora`) բառացի | Ծանոթագրություններ |
|-----------------------------------------------------------------------------------------------|
| `addr-multisig-council-threshold3` | `≥3` քաշ, անդամներ `(2,1,1)` | `SRfSHsrH3tEmYaaAYyD248F3vfT1oQ3WEGS22MaD8W9bLefF7rsoKLYGcpbcM9EcSus5ZhCAZU7ztn2BCsyeCAdfRncAVmVsipd4ibk6CBLF3Nrzcw8P7VKJg6mtFgEhWVTjfDkUMoc63oeEmaWyV6cyiphwk8ZgKAJUe4TyVtmKm1WWcg7qZ6i` | `sora3vﾑ2zkaoUwﾋﾅGﾘﾚyﾂe3ﾖfﾙヰｶﾘﾉwｷnoWﾛYicaUr3ﾔｲﾖ2Ado3TﾘYQﾉJqﾜﾇｳﾑﾐd8dDjRGｦ3Vﾃ9HcﾀMヰR8ﾎﾖgEqGｵEｾDyc5ﾁ1ﾔﾉ31sUﾑﾀﾖaｸxﾘ3ｲｷMEuFｺｿﾉBQSVQnxﾈeJzrXLヰhｿｹ5SEEﾅPﾂﾗｸdヰﾋ1bUGHｲVXBWNNJ6K` | Խորհուրդ-տիրույթ կառավարման քվորում. |
| `addr-multisig-wonderland-threshold2` | `≥2`, անդամներ `(1,2)` | `3xsmkps1KPBn9dtpE5qHRhHEZCpiAe8d9j6H9A42TV6kc1TpaqdwnSksKgQrsSEHznqvWKBMc1os69BELzkLjsR7EV2gjV14d9JMzo97KEmYoKtxCrFeKFAcy7ffQdboV1uRt` | `sora2ﾖZﾘeｴAdx3ﾂﾉﾔXhnｹﾀ2ﾉｱﾋxﾅﾄﾌヱwﾐmﾊvEﾐCﾏﾎｦ1ﾑHﾋso2GKﾔﾕﾁwﾂﾃP6ﾁｼﾙﾖｺ9ｻｦbﾈ4wFdﾑFヰ3HaﾘｼMｷﾌHWtｷﾋLﾙﾖQ4D3XﾊﾜXmpktﾚｻ5ﾅﾅﾇ1gkﾏsCFQGH9` | Հրաշքների աշխարհի երկակի ստորագրության օրինակ (քաշը 1 + 2): |
| `addr-multisig-default-quorum3` | `≥3`, անդամներ `(1,1,1,1)` | `nA2bDNhMqXz7ERkHNoEWbvJGyR1aDRsw32LaUWLgbK3vcpzohmdFCLvdotxUWWDY3aZeX4ptLk4Z6TjF5ossnJm8VrNo6daxmGTkqUyP4MxJxiNyPFxsEE5DLnsoLWUcxaWNpZ76tmkbiGS31Gv8tejKpuiHUMaQ1s5ohWyZvDnpycNkBK8AEfGJqn5yc9zAzfWbVhpDwkPj8ScnzvH1Echr5` | `soraﾐ38ﾅｴｸﾜ8ﾃzwBrqﾘｺ4yﾄv6kqJp1ｳｱﾛｿrzﾄﾃﾘﾒRﾗtV9ｼﾔPｽcヱEﾌVVVｼﾘｲZAｦﾓﾅｦeﾒN76vﾈcuｶuﾛL54rzﾙﾏX2zMﾌRLﾃﾋpﾚpｲcHﾑﾅﾃﾔzｵｲVfAﾃﾚﾎﾚCヰﾔｲｽｦw9ﾔﾕ8bGGkﾁ6sNｼaｻRﾖﾜYﾕﾚU18ﾅHヰﾌuMeﾊtﾂrｿj95Ft8ﾜ3fﾄkNiｴuﾈrCﾐQt8ヱｸｸmﾙﾒgUbﾑEKTTCM` | Բազային կառավարման համար օգտագործվող տիրույթի անուղղակի լռակյաց քվորում:

#### 2.4 Խափանման կանոններ (ADDR-1a)

- Պահանջվող վերնագիր + ընտրիչից ավելի կարճ կամ մնացորդ բայթերով բեռները արտանետում են `AccountAddressError::InvalidLength` կամ `AccountAddressError::UnexpectedTrailingBytes`:
- Վերնագրերը, որոնք սահմանում են վերապահված `ext_flag` կամ գովազդում են չաջակցվող տարբերակները/դասերը, ՊԵՏՔ Է մերժվեն՝ օգտագործելով `UnexpectedExtensionFlag`, `InvalidHeaderVersion` կամ `UnknownAddressClass`:
- Անհայտ ընտրիչի/կարգավորիչի պիտակները բարձրացնում են `UnknownDomainTag` կամ `UnknownControllerTag`:
- Չափազանց մեծ կամ սխալ ձևավորված հիմնական նյութը բարձրացնում է `KeyPayloadTooLong` կամ `InvalidPublicKey`:
- Multisig կարգավորիչները, որոնք գերազանցում են 255 անդամները, բարձրացնում են `MultisigMemberOverflow`:
- IME/NFKC փոխարկումներ. կիսալայնությամբ Sora kana-ն կարող է նորմալացվել իրենց ամբողջ լայնության ձևերին՝ առանց վերծանման խախտելու, սակայն ASCII `sora` պահակային և I105 թվանշանները/տառերը ՊԵՏՔ Է մնան ASCII: Ամբողջ լայնությամբ կամ պատյանով ծալված պահապանների մակերեսը `ERR_MISSING_COMPRESSED_SENTINEL`, ամբողջ լայնությամբ ASCII օգտակար բեռները բարձրացնում են `ERR_INVALID_COMPRESSED_CHAR`, իսկ ստուգիչ գումարի անհամապատասխանությունները՝ որպես `ERR_CHECKSUM_MISMATCH`: `crates/iroha_data_model/src/account/address.rs`-ի սեփականության թեստերը ծածկում են այս ուղիները, որպեսզի SDK-ները և դրամապանակները կարողանան հիմնվել որոշիչ ձախողումների վրա:
- Torii և `address@domain` (rejected legacy form) փոխանունների Torii և SDK վերլուծությունը այժմ թողարկում են նույն `ERR_*` կոդերը, երբ I105 (նախընտրելի)/sora (երկրորդ լավագույն) մուտքերը ձախողվում են, նախքան կեղծանունը կարող է կրկնել տիրույթի սխալները, ստուգել տիրույթի պատճառները (օր. առանց արձակ տողերից գուշակելու.
- `ERR_LOCAL8_DEPRECATED` 12 բայթից ավելի կարճ տեղային ընտրիչով օգտակար բեռներ՝ պահպանելով հին Local‑8 մարսողությունների կոշտ անջատումը:
- Domainless canonical i105 literals decode directly to a domainless `AccountId`. Use `ScopedAccountId` only when an interface requires explicit domain context.

#### 2.5 Նորմատիվ երկուական վեկտորներ

- **Նախադրված լռելյայն տիրույթ (`default`, սկզբնական բայթ `0x00`)**  
  Կանոնական վեցանկյուն՝ `0x020001203b6a27bcceb6a42d62a3a8d02a6f0d73653215771de243a63ac048a18b59da29`:  
  Բաշխում. `0x02` վերնագիր, `0x00` ընտրիչ (իսպառ լռելյայն), `0x00` վերահսկիչի պիտակ, `0x01` կորի id (Ed25519), I18NI00000002 ստեղնը, որին հաջորդում է վճարման երկարությունը:
- **Տեղական տիրույթի ամփոփում (`treasury`, սերմ բայթ `0x01`)**  
  Կանոնական վեցանկյուն՝ `0x0201b18fe9c1abbac45b3e38fc5d0001208a88e3dd7409f195fd52db2d3cba5d72ca6709bf1d94121bf3748801b40f6f5c`:  
  Բաշխում․ 32 բայթ Ed25519 բանալի):Միավոր թեստերը (`account::address::tests::parse_encoded_accepts_all_formats`) հաստատում են ներքևում գտնվող V1 վեկտորները `AccountAddress::parse_encoded`-ի միջոցով՝ երաշխավորելով, որ գործիքավորումը կարող է հիմնվել վեցանկյուն, I105 (նախընտրելի) և սեղմված (`sora`, երկրորդ լավագույն) ձևերի կանոնական ծանրաբեռնվածության վրա: Վերականգնեք ընդլայնված հարմարանքների հավաքածուն `cargo run -p iroha_data_model --example address_vectors`-ով:

| Դոմեն | Սերմերի բայթ | Կանոնական վեցանկյուն | Սեղմված (`sora`) |
|------------------------------------------------------------ -----------------------------------------------------------|
| լռելյայն | `0x00` | `0x020001203b6a27bcceb6a42d62a3a8d02a6f0d73653215771de243a63ac048a18b59da29` | `sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE` |
| գանձապետարան | `0x01` | `0x0201b18fe9c1abbac45b3e38fc5d0001208a88e3dd7409f195fd52db2d3cba5d72ca6709bf1d94121bf3748801b40f6f5c` | `sora5ｻu6rﾀCヰTGwﾏ1ﾅヱﾌQｲﾖﾇqCｦヰﾓZQCZRDSSﾅMｱﾙヱｹﾁｸ8ｾeﾄﾛ6C8bZuwﾗｹCZｦRSLQFU` |
| Հրաշքների երկիր | `0x02` | `0x0201b8ae571b79c5a80f5834da2b0001208139770ea87d175f56a35466c34c7ecccb8d8a91b4ee37a25df60f5b8fc9b394` | `sora5ｻwﾓyRｿqﾏnMﾀﾙヰKoﾒﾇﾓQｺﾛyｼ3ｸFHB2F5LyPﾐTMZkｹｼw67ﾋVﾕｻr8ﾉGﾇeEnｻVRNKCS` |
| իրոհա | `0x03` | `0x0201de8b36819700c807083608e2000120ed4928c628d1c2c6eae90338905995612959273a5c63f93636c14614ac8737d1` | `sora5ｻﾜxﾀ7Vｱ7QFeｷMﾂLﾉﾃﾏﾓﾀTﾚgSav3Wnｱｵ4ｱCKｷﾛMﾘzヰHiﾐｱ6ﾃﾉﾁﾐZmﾇ2fiﾎX21P4L` |
| ալֆա | `0x04` | `0x020146be2154ae86826a3fef0ec0000120ca93ac1705187071d67b83c7ff0efe8108e8ec4530575d7726879333dbdabe7c` | `sora5ｻ9JヱﾈｿuwU6ｴpﾔﾂﾈRqRTds1HﾃﾐｶLVﾍｳ9ﾔhｾNｵVｷyucEﾒGﾈﾏﾍ9sKeﾉDzrｷﾆ742WG1` |
| օմեգա | `0x05` | `0x0201390d946885bc8416b3d30c9d0001206e7a1cdd29b0b78fd13af4c5598feff4ef2a97166e3ca6f2e4fbfccd80505bf1` | `sora5ｻ3zrﾌuﾚﾄJﾑXQhｸTyN8pzwRkWxmjVﾗbﾚﾕヰﾈoｽｦｶtEEﾊﾐ6GPｿﾓﾊｾEhvPｾｻ3XAJ73F` |
| կառավարում | `0x06` | `0x0201989eb45a80940d187e2c908f0001208a875fff1eb38451577acd5afee405456568dd7c89e090863a0557bc7af49f17` | `sora5ｻiｵﾁyVﾕｽbFpDHHuﾇﾉdﾗｲﾓﾄRﾋAW3frUCｾ5ｷﾘTwdﾚnｽtQiLﾏｼｶﾅXgｾZmﾒヱH58H4KP` |
| վավերացնողներ | `0x07` | `0x0201e4ffa58704c69afaeb7cc2d7000120ea4a6c63e29c520abef5507b132ec5f9954776aebebe7b92421eea691446d22c` | `sora5ｻﾀLDH6VYﾑNAｾgﾉVﾜtxﾊRXLｹﾍﾔﾌLd93GﾔGeｴﾄYrs1ﾂHｸkYxｹwｿyZﾗxyﾎZoXT1S4N` |
| հետախույզ | `0x08` | `0x02013b35422c65c2a83c99c523ad0001201398f62c6d1a457c51ba6a4b5f3dbd2f69fca93216218dc8997e416bd17d93ca` | `sora5ｻ4nmｻaﾚﾚPvNLgｿｱv6MHDeEyﾀovﾉJcpvrﾖ6ﾈCQcCNﾇﾜhﾚﾖyFdTwｸｶHEｱ9rWU8FMB` |
| սորանետ | `0x09` | `0x0201047d9ea7f5d5dbec3f7bfc58000120fd1724385aa0c75b64fb78cd602fa1d991fdebf76b13c58ed702eac835e9f618` | `sora5ｱｸヱVQﾂcﾁヱRﾓcApｲﾁﾅﾒvﾌﾏfｾNnﾛRJsｿDhﾙuHaﾚｺｦﾌﾍﾈeﾆﾎｺN1UUDｶ6ﾎﾄﾛoRH8JUL` |
| kitsune | `0x0A` | `0x0201e91933de397fd7723dc9a76c00012043a72e714401762df66b68c26dfbdf2682aaec9f2474eca4613e424a0fbafd3c` | `sora5ｻﾚｺヱkfFJfSﾁｼJwﾉLvbpSｷﾔMWFMrbｳｸｲｲyヰKGJﾉｻ4ｹﾕrｽhｺｽzSDヰXAN62AD7RGNS` |
| դա | `0x0B` | `0x02016838cf5bb0ce0f3d4f380e1c00012066be7e332c7a453332bd9d0a7f7db055f5c5ef1a06ada66d98b39fb6810c473a` | `sora5ｻNﾒ5SﾐRﾉﾐﾃ62ｿ1ｶｷWFKyF1BcAﾔvｼﾐHqﾙﾐPﾏｴヰ5tｲﾕvnﾙT6ﾀW7mﾔ7ﾇﾗﾂｳ25CXS93` |

Վերանայվել է. Data Model WG, Cryptography WG — շրջանակը հաստատված է ADDR-1a-ի համար:

##### Sora Nexus տեղեկատու այլանուններ

Sora Nexus ցանցերը կանխադրված են `chain_discriminant = 0x02F1`
(`iroha_config::parameters::defaults::common::CHAIN_DISCRIMINANT`): Այն
Հետևաբար, `AccountAddress::to_i105` և `to_i105` օգնականները արտանետում են
հետևողական տեքստային ձևեր յուրաքանչյուր կանոնական ծանրաբեռնվածության համար: Ընտրված հարմարանքները
`fixtures/account/address_vectors.json` (ստեղծվել է միջոցով
`cargo xtask address-vectors`) ներկայացված են ստորև՝ արագ հղման համար.

| Հաշիվ / ընտրիչ | I105 բառացի (նախածանց `0x02F1`) | Սորա սեղմված (`sora`) բառացի |
|--------------------------------------------------------------------------------------|
| `default` տիրույթ (իմպլիցիտ ընտրիչ, սերմ `0x00`) | `6cmzPVPX5jDQFNfiz6KgmVfm1fhoAqjPhoPFn4nx9mBWaFMyUCwq4cw` | `sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE` (ըստ ցանկության `@default` վերջածանց, երբ հստակ երթուղային ակնարկներ է տրվում) |
| `treasury` (տեղական մարսողության ընտրիչ, սերմ `0x01`) | `34mSYnCXkCzHXm31UDHh7SJfGvC4QPEhwim8z7sys2iHqXpCwCQkjL8KHvkFLSs1vZdJcb37r` | `sora5ｻu6rﾀCヰTGwﾏ1ﾅヱﾌQｲﾖﾇqCｦヰﾓZQCZRDSSﾅMｱﾙヱｹﾁｸ8ｾeﾄﾛ6C8bZuwﾗｹCZｦRSLQFU` |
| Համաշխարհային ռեգիստրի ցուցիչ (`registry_id = 0x0000_002A`, համարժեք `treasury`) | `3oE9sLeRGP49Cu7mQ1nF4wtKAm29BG4TGLiRsaXe7mhbMP5WZ113nNW1N6RbqF` | `sorakXｹ6NｻﾍﾀﾖSﾜﾖｱ3ﾚ5WﾘﾋQﾅｷｦxgﾛｸcﾁｵﾋkﾋvﾏ8SPﾓﾀｹdｴｴｲW9iCM6AEP` |

Այս տողերը համընկնում են CLI-ի (`iroha tools address convert`), Torii-ի թողարկված տողերի հետ
պատասխաններ (`canonical i105 literal rendering`) և SDK օգնականներ, այնպես որ UX պատճենեք/տեղադրեք
հոսքերը կարող են բառացիորեն հենվել դրանց վրա: Կցել `<address>@<domain>` (rejected legacy form) միայն այն դեպքում, երբ ձեզ անհրաժեշտ է հստակ երթուղային հուշում. վերջածանցը կանոնական ելքի մաս չէ։

#### 2.6 Փոխգործունակության տեքստային այլանուններ (պլանավորված)

- ** Շղթայական կեղծանունների ոճ.** `ih:<chain-alias>:<alias@domain>` գերանների և մարդկանց համար
  մուտք. Դրամապանակները պետք է վերլուծեն նախածանցը, ստուգեն ներկառուցված շղթան և արգելափակեն
  անհամապատասխանություններ.
- **CAIP-10 ձև:** `iroha:<caip-2-id>:<i105-addr>` շղթայական-ագնոստիկայի համար
  ինտեգրումներ։ Այս քարտեզագրումը **առաքվածում դեռ չի իրականացվել**
  գործիքների շղթաներ.
- **Մեքենայի օգնականներ.** Հրապարակեք կոդեկներ Rust, TypeScript/JavaScript, Python,
  և Kotlin ծածկող I105 և սեղմված ձևաչափեր (`AccountAddress::to_i105`,
  `AccountAddress::parse_encoded` և դրանց SDK համարժեքները): CAIP-10 օգնականներն են
  ապագա աշխատանք.

#### 2.7 Deterministic I105 alias

- **Նախածանցի քարտեզագրում.** Կրկին օգտագործեք `chain_discriminant`-ը որպես I105 ցանցի նախածանց:
  `encode_i105_prefix()` (տես `crates/iroha_data_model/src/account/address.rs`)
  թողարկում է 6 բիթանոց նախածանց (մեկ բայթ) `<64` արժեքների և 14 բիթանոց երկու բայթ
  ձև ավելի մեծ ցանցերի համար: Հեղինակավոր հանձնարարությունները ապրում են
  [`address_prefix_registry.md`] (source/references/address_prefix_registry.md);
  SDK-ները ՊԵՏՔ Է համաժամեցված պահեն համապատասխան JSON ռեեստրը՝ բախումներից խուսափելու համար:
- **Հաշվի նյութ.** I105-ը կոդավորում է կառուցված կանոնական բեռը
  `AccountAddress::canonical_bytes()` — վերնագրի բայթ, տիրույթի ընտրիչ և
  վերահսկիչի ծանրաբեռնվածություն: Հաշինգի լրացուցիչ քայլ չկա. I105-ը ներառում է
  Երկուական կարգավորիչի օգտակար բեռնվածություն (մեկ բանալի կամ բազմանշանակ), ինչպես արտադրվում է Rust-ի կողմից
  կոդավորիչը, այլ ոչ թե CTAP2 քարտեզը, որն օգտագործվում է բազմանշանակ քաղաքականության ամփոփումների համար:
- **Կոդավորում.** `encode_i105()`-ը միացնում է նախածանցի բայթերը կանոնականի հետ
  օգտակար բեռ և ավելացնում է Blake2b-512-ից ստացված 16-բիթանոց ստուգիչ գումարը՝ ֆիքսված
  նախածանց `I105PRE` (`b"I105PRE"` || prefix || payload)։ Արդյունքը կոդավորվում է `bs58`-ով՝ օգտագործելով I105 այբուբենը։
  CLI/SDK օգնականները բացահայտում են նույն ընթացակարգը, և `AccountAddress::parse_encoded`
  այն հակադարձում է `decode_i105`-ի միջոցով:

#### 2.8 Նորմատիվ տեքստային թեստի վեկտորներ

`fixtures/account/address_vectors.json` պարունակում է ամբողջական I105 (նախընտրելի) և սեղմված (`sora`, երկրորդ լավագույն)
բառացի յուրաքանչյուր կանոնական բեռի համար: Առանձնահատկություններ.

- **`addr-single-default-ed25519` (Sora Nexus, նախածանց `0x02F1`):**  
  I105 `6cmzPVPX5jDQFNfiz6KgmVfm1fhoAqjPhoPFn4nx9mBWaFMyUCwq4cw`, սեղմված (`sora`)
  `sora2QG…U4N5E5`. Torii-ն արտանետում է այս ճշգրիտ տողերը `AccountId`-ից
  `Display` իրականացում (կանոնական I105) և `AccountAddress::to_i105`:
- **`addr-global-registry-002a` (ռեգիստրի ընտրիչ → գանձարան):**  
  I105 `3oE9sLeRGP49Cu7mQ1nF4wtKAm29BG4TGLiRsaXe7mhbMP5WZ113nNW1N6RbqF`, սեղմված (`sora`)
  `sorakX…CM6AEP`. Ցույց է տալիս, որ ռեեստրի ընտրիչները դեռ վերծանում են
  նույն կանոնական ծանրաբեռնվածությունը, ինչ համապատասխան տեղական մարսողությունը:
- **Խափանման դեպք (`i105-prefix-mismatch`):**  
  I105 բառացի վերլուծություն, որը կոդավորված է `NETWORK_PREFIX + 1` նախածանցով հանգույցի վրա
  ակնկալելով կանխադրված նախածանցի ելքը
  `AccountAddressError::UnexpectedNetworkPrefix { expected: 753, found: 754 }`
  նախքան տիրույթի երթուղավորման փորձը: `i105-checksum-mismatch` հարմարանք
  իրականացնում է կեղծման հայտնաբերում Blake2b ստուգիչ գումարի վրա:

#### 2.9 Համապատասխանության սարքեր

ADDR-2-ը տրամադրում է վերարտադրվող սարքերի փաթեթ, որը ծածկում է դրական և բացասական կողմերը
սցենարներ կանոնական վեցանկյունի վրա, I105 (նախընտրելի), սեղմված (`sora`, կես-/լրիվ լայնություն), անուղղակի
լռելյայն ընտրիչներ, գլոբալ ռեգիստրի կեղծանուններ և բազմանշանակ կարգավորիչներ: Այն
կանոնական JSON-ն ապրում է `fixtures/account/address_vectors.json`-ում և կարող է լինել
վերականգնված՝

```
cargo xtask address-vectors --out fixtures/account/address_vectors.json
# verify without writing:
cargo xtask address-vectors --verify
```

Ad-hoc փորձերի համար (տարբեր ուղիներ/ձևաչափեր) օրինակը երկուական է
հասանելի:

```
cargo run -p iroha_data_model --example account_address_vectors > fixtures/account/address_vectors.json
```

Ժանգի միավորի փորձարկումներ `crates/iroha_data_model/tests/account_address_vectors.rs`-ում
և `crates/iroha_torii/tests/account_address_vectors.rs`, JS-ի հետ միասին,
Swift և Android ամրագոտիներ (`javascript/iroha_js/test/address.test.js`,
`IrohaSwift/Tests/IrohaSwiftTests/AccountAddressTests.swift`,
`java/iroha_android/src/test/java/org/hyperledger/iroha/android/address/AccountAddressTests.java`),
սպառեք նույն սարքավորումը՝ երաշխավորելու կոդեկների հավասարությունը SDK-ների և Torii մուտքի համար:

### 3. Համաշխարհային եզակի տիրույթներ և նորմալացում

Տես նաև՝ [`docs/source/references/address_norm_v1.md`](source/references/address_norm_v1.md)
կանոնական Norm v1 խողովակաշարի համար, որն օգտագործվում է Torii-ում, տվյալների մոդելում և SDK-ներում:

Վերասահմանեք `DomainId`-ը որպես պիտակավորված բազմակի՝

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

`LocalChain`-ը փաթաթում է առկա Անունը տիրույթների համար, որոնք կառավարվում են ընթացիկ շղթայի կողմից:
Երբ տիրույթը գրանցվում է համաշխարհային ռեգիստրի միջոցով, մենք շարունակում ենք սեփականության իրավունքը
շղթայի տարբերակիչ. Ցուցադրումը / վերլուծությունն առայժմ մնում է անփոփոխ, բայց
Ընդլայնված կառուցվածքը թույլ է տալիս երթուղային որոշումներ կայացնել:

#### 3.1 Նորմալացում և կեղծիքների պաշտպանություն

Norm v1-ը սահմանում է կանոնական խողովակաշարը, որը յուրաքանչյուր բաղադրիչ պետք է օգտագործի տիրույթից առաջ
անունը պահպանված է կամ ներդրված է `AccountAddress`-ում: Ամբողջական անցում
ապրում է [`docs/source/references/address_norm_v1.md`] (source/references/address_norm_v1.md);
ստորև բերված ամփոփագիրը ցույց է տալիս դրամապանակների, Torii-ի, SDK-ների և կառավարման քայլերը
գործիքները պետք է իրականացվեն:

1. **Մուտքի վավերացում։** Մերժել դատարկ տողերը, բացատները և վերապահվածը
   սահմանազատիչներ `@`, `#`, `$`: Սա համընկնում է պարտադրված անփոփոխների հետ
   `Name::validate_str`.
2. ** Unicode NFC-ի կազմը։** Կիրառեք ICU-ով ապահովված NFC-ի նորմալացումը այնքան կանոնական
   համարժեք հաջորդականությունները դետերմինիստորեն փլուզվում են (օրինակ՝ `e\u{0301}` → `é`):
3. **UTS-46 նորմալացում։** Գործարկեք NFC ելքը UTS‑46-ի միջոցով
   `use_std3_ascii_rules = true`, `transitional_processing = false` և
   DNS-ի երկարության կիրառումը միացված է: Արդյունքը փոքրատառով A-պիտակի հաջորդականությունն է.
   STD3 կանոնները խախտող մուտքերն այստեղ ձախողվում են:
4. **Երկարության սահմանները։** Կիրառեք DNS-ի ոճի սահմանները. յուրաքանչյուր պիտակ ՊԵՏՔ Է լինի 1–63
   բայթը և ամբողջական տիրույթը ՉՊԵՏՔ գերազանցի 255 բայթը 3-րդ քայլից հետո:
5. **Ընտրովի շփոթեցնող քաղաքականություն։** UTS‑39 սկրիպտների ստուգումները հետագծվում են
   Նորմ v2; օպերատորները կարող են դրանք վաղաժամ միացնել, սակայն ստուգումը ձախողելու դեպքում պետք է ընդհատվի
   վերամշակում։

Եթե յուրաքանչյուր փուլ հաջողվի, փոքրատառով A-պիտակի տողը պահվում է և օգտագործվում է դրա համար
հասցեի կոդավորում, կոնֆիգուրացիա, մանիֆեստներ և ռեեստրի որոնումներ: Տեղական մարսողություն
ընտրիչները ստանում են իրենց 12 բայթ արժեքը որպես `blake2s_mac (բանալի = "SORA-LOCAL-K:v1",
canonical_label)[0..12]` օգտագործելով քայլ 3 ելքը: Բոլոր մյուս փորձերը (խառը
մեծատառ, մեծատառ, հում Unicode մուտքագրում) մերժվում են կառուցվածքայինով
`ParseError`s այն սահմանին, որտեղ նշված է անունը:

Կանոնական սարքեր, որոնք ցուցադրում են այս կանոնները, այդ թվում՝ շրջագայությունների կոդով
և STD3 անվավեր հաջորդականությունները՝ թվարկված են
`docs/source/references/address_norm_v1.md` և արտացոլված են SDK CI-ում
վեկտորային փաթեթներ, որոնք հետևվում են ADDR-2-ի ներքո:

### 4. Nexus տիրույթի ռեեստր և երթուղում- **Ռեեստրի սխեման.** Nexus պահպանում է ստորագրված քարտեզ `DomainName -> ChainRecord`
  որտեղ `ChainRecord` ներառում է շղթայական տարբերակիչ, կամընտիր մետատվյալներ (RPC
  վերջնակետեր) և իրավասության ապացույց (օրինակ՝ կառավարման բազմաստորագրություն):
- **Համաժամեցման մեխանիզմ:**
  - Շղթաները ներկայացնում են ստորագրված տիրույթի պահանջներ Nexus-ին (ծննդի ընթացքում կամ միջոցով
    կառավարման հրահանգ):
  - Nexus-ը հրապարակում է պարբերական մանիֆեստներ (ստորագրված JSON գումարած կամընտիր Merkle արմատ)
    HTTPS-ի և բովանդակության հասցեով պահեստավորման միջոցով (օրինակ՝ IPFS): Հաճախորդները ամրացնում են
    վերջին մանիֆեստը և ստուգեք ստորագրությունները:
- **Փնտրման հոսք:**
  - Torii-ը ստանում է գործարք՝ հղում կատարելով `DomainId`-ին:
  - Եթե տիրույթը տեղում անհայտ է, Torii-ը հարցում է կատարում քեշավորված Nexus մանիֆեստում:
  - Եթե մանիֆեստում նշվում է օտարերկրյա շղթա, ապա գործարքը մերժվում է
    որոշիչ `ForeignDomain` սխալ և հեռավոր շղթայի տեղեկատվությունը:
  - Եթե տիրույթը բացակայում է Nexus-ից, Torii-ը վերադարձնում է `UnknownDomain`:
- **Վստահության խարիսխներ և ռոտացիա. ** Կառավարման բանալիների նշանը դրսևորվում է; ռոտացիա կամ
  չեղյալ հայտարարումը հրապարակվում է որպես նոր մանիֆեստի գրառում: Հաճախորդները պարտադրում են մանիֆեստը
  TTL-ներ (օրինակ՝ 24 ժամ) և հրաժարվեք այդ պատուհանից այն կողմ հնացած տվյալներից օգտվելուց:
- **Խափանման ռեժիմներ.
  տվյալներ TTL-ի շրջանակներում; անցած TTL-ն արտանետում է `RegistryUnavailable` և հրաժարվում է
  խաչաձեւ տիրույթի երթուղիացում՝ անհամապատասխան վիճակից խուսափելու համար:

### 4.1 Գրանցամատյանի անփոփոխելիություն, կեղծանուններ և տապանաքարեր (ADDR-7c)

Nexus-ը հրապարակում է **միայն հավելվածի մանիֆեստ**, այնպես որ յուրաքանչյուր տիրույթ կամ անունի նշանակում
կարող է ստուգվել և վերարտադրվել: Օպերատորները պետք է մշակեն փաթեթում նկարագրված փաթեթը
[Adress manifest runbook](source/runbooks/address_manifest_ops.md) որպես
ճշմարտության միակ աղբյուրը. եթե մանիֆեստը բացակայում է կամ չի վավերացվում, Torii պետք է
հրաժարվել ազդակիր տիրույթը լուծելուց.

Ավտոմատացման աջակցություն՝ `cargo xtask address-manifest verify --bundle <current_dir> --previous <previous_dir>`
կրկնում է ստուգման գումարը, սխեման և նախորդ ամփոփիչ ստուգումները, որոնք գրված են
runbook. Ներառեք հրամանի ելքը փոփոխության տոմսերում՝ `sequence`-ը ցուցադրելու համար
և `previous_digest` կապը վավերացվել է նախքան փաթեթը հրապարակելը:

#### Մանիֆեստի վերնագրի և ստորագրության պայմանագիր

| Դաշտային | Պահանջը |
|-------|-------------|
| `version` | Ներկայումս `1`: Զարկեք միայն համապատասխան տեխնիկական թարմացումներով: |
| `sequence` | Աճել **ճիշտ** մեկ հրապարակման համար: Torii քեշերը հրաժարվում են բացթողումներով կամ ռեգրեսիաներով վերանայումներից: |
| `generated_ms` + `ttl_hours` | Ստեղծեք քեշի թարմությունը (կանխադրված 24 ժամ): Եթե ​​TTL-ի ժամկետը լրանում է մինչև հաջորդ հրապարակումը, Torii-ը դառնում է `RegistryUnavailable`: |
| `previous_digest` | BLAKE3 մարսողություն (վեցանկյուն) նախորդ մանիֆեստի մարմնի: Ստուգողները այն վերահաշվում են `b3sum`-ով՝ անփոփոխությունն ապացուցելու համար: |
| `signatures` | Մանիֆեստները ստորագրվում են Sigstore (`cosign sign-blob`) միջոցով: Ops-ը պետք է գործարկի `cosign verify-blob --bundle manifest.sigstore manifest.json` և կիրառի կառավարման ինքնության/թողարկողի սահմանափակումները նախքան թողարկումը: |

Թողարկման ավտոմատացումն արտանետում է `manifest.sigstore` և `checksums.sha256`
JSON մարմնի կողքին: Պահպանեք ֆայլերը միասին SoraFS կամ արտացոլելիս
HTTP-ի վերջնակետերը, որպեսզի աուդիտորները կարողանան բառացիորեն վերարտադրել ստուգման քայլերը:

#### Մուտքի տեսակները

| Տեսակ | Նպատակը | Պահանջվող դաշտերը |
|------|---------|-----------------|
| `global_domain` | Հայտարարում է, որ տիրույթը գրանցված է գլոբալ մակարդակով և պետք է համապատասխանի շղթայական տարբերակիչին և I105 նախածանցին: | `{ "domain": "<label>", "chain": "sora:nexus:global", "i105_prefix": 753, "selector": "global" }` |
| `tombstone` | Մշտապես թոշակի է հանում կեղծանունը/ընտրողը: Պահանջվում է Local‑8 բովանդակությունը ջնջելիս կամ տիրույթը հեռացնելիս: | `{ "selector": {…}, "reason_code": "LOCAL8_RETIREMENT" \| …, "ticket": "<governance id>", "replaces_sequence": <number> }` |

`global_domain` գրառումները կարող են կամայականորեն ներառել `manifest_url` կամ `sorafs_cid`
դրամապանակները մատնանշել ստորագրված շղթայի մետատվյալների վրա, սակայն կանոնական կրկնապատիկը մնում է
`{domain, chain, discriminant/i105_prefix}`. `tombstone` գրառումները **պետք է** մեջբերել
ընտրիչը պաշտոնաթող է և տոմսի/կառավարման արտեֆակտը, որը թույլ է տվել
փոփոխությունը, որպեսզի աուդիտի հետքը վերակառուցելի լինի անցանց ռեժիմում:

#### Այլանուն/տապանաքարի աշխատանքային հոսք և հեռաչափություն

1. **Գտեք դրեյֆը։** Օգտագործեք `torii_address_local8_total{endpoint}`,
   `torii_address_local8_domain_total{endpoint,domain}`,
   `torii_address_collision_total{endpoint,kind="local12_digest"}`,
   `torii_address_collision_domain_total{endpoint,domain}`,
   `torii_address_domain_total{endpoint,domain_kind}`, և
   `torii_address_invalid_total{endpoint,reason}` (արտադրված է
   `dashboards/grafana/address_ingest.json`) տեղական ներկայացումները հաստատելու և
   Local-12 բախումները մնում են զրոյական մակարդակում՝ նախքան տապանաքար առաջարկելը: Այն
   յուրաքանչյուր տիրույթի հաշվիչները թույլ են տալիս սեփականատերերին ապացուցել, որ միայն մշակող/փորձարկման տիրույթներն են թողարկում Local‑8
   երթևեկությունը (և Local‑12 բախումները քարտեզագրվում են բեմականացման հայտնի տիրույթներին), մինչդեռ
   ներառում է **Domain Kind Mix (5m)** վահանակը, որպեսզի SRE-ները կարողանան պատկերել, թե որքան
   `domain_kind="local12"` տրաֆիկը մնում է, իսկ `AddressLocal12Traffic`
   ահազանգել հրդեհների, երբ արտադրությունը դեռ տեսնում է Local-12 ընտրիչներ, չնայած դրան
   կենսաթոշակային դարպաս.
2. **Ստեղծիր կանոնական մարսողություններ։** Վազիր
   `iroha tools address convert <address> --format json --expect-prefix 753`
   (կամ սպառեք `fixtures/account/address_vectors.json` միջոցով
   `scripts/account_fixture_helper.py`) ճշգրիտ `digest_hex`-ը գրավելու համար:
   CLI-ն ընդունում է I105, `i105` և կանոնական `0x…` տառեր; կցել
   `@<domain>` միայն այն դեպքում, երբ անհրաժեշտ է պահպանել պիտակը մանիֆեստների համար:
   JSON ամփոփագիրը հայտնվում է այդ տիրույթում `input_domain` դաշտի միջոցով և
   `legacy  suffix`-ը վերարտադրում է փոխարկված կոդավորումը որպես `<address>@<domain>` (rejected legacy form)
   manifest diffs (այս վերջածանցը մետատվյալ է, այլ ոչ թե կանոնական հաշվի ID):
   Նոր գծի վրա ուղղված արտահանման համար օգտագործեք
   `iroha tools address normalize --input <file> legacy-selector input mode` տեղական զանգվածային փոխակերպման համար
   ընտրիչները վերածվում են կանոնական I105 (նախընտրելի), սեղմված (`sora`, երկրորդ լավագույն), վեցանկյուն կամ JSON ձևերի՝ բաց թողնելով
   ոչ տեղական տողեր. Երբ աուդիտորներին անհրաժեշտ են աղյուսակների համար հարմար ապացույցներ, գործարկեք
   `iroha tools address audit --input <file> --format csv`՝ CSV ամփոփագիր թողարկելու համար
   (`input,status,format,domain_kind,…`), որն ընդգծում է Տեղական ընտրիչները,
   կանոնական կոդավորումներ և նույն ֆայլի ձախողումների վերլուծություն:
3. **Ավելացրեք մանիֆեստի գրառումները։** Կազմեք `tombstone` գրառումը (և հետագա գործողությունները
   `global_domain` գրանցել համաշխարհային ռեգիստր տեղափոխելիս) և վավերացնել
   մանիֆեստը `cargo xtask address-vectors`-ով նախքան ստորագրություններ խնդրելը:
4. **Ստուգեք և հրապարակեք։** Հետևեք runbook ստուգաթերթին (հեշ, Sigstore,
   հաջորդականությունը միապաղաղություն) նախքան փաթեթը SoraFS-ին արտացոլելը: Torii հիմա
   կանոնականացնում է I105 (նախընտրելի)/սորա (երկրորդ լավագույն) բառացիները փաթեթի վայրէջքից անմիջապես հետո:
5. **Մոնիտորինգ և հետադարձ կապ:** Պահեք Local-8 և Local-12 բախման վահանակները
   զրո 30 օրվա ընթացքում; եթե հետընթացներ հայտնվեն, վերահրապարակեք նախորդ մանիֆեստը
   միայն ազդակիր ոչ արտադրական միջավայրում, մինչև հեռաչափությունը կայունանա:

Վերոնշյալ բոլոր քայլերը պարտադիր ապացույց են ADDR-7c-ի համար. դրսևորվում է առանց
`cosign` ստորագրության փաթեթը կամ առանց `previous_digest` արժեքների պետք է
ինքնաբերաբար մերժվում է, և օպերատորները պետք է կցեն ստուգման մատյանները
նրանց փոխելու տոմսերը:

### 5. Դրամապանակ և API էրգոնոմիկա

- **Ցուցադրել կանխադրվածները. ** Դրամապանակները ցույց են տալիս I105 հասցեն (կարճ, ստուգված ամփոփված)
  գումարած լուծված տիրույթը որպես ռեեստրից բերված պիտակ: Դոմեններն են
  հստակ նշված է որպես նկարագրական մետատվյալներ, որոնք կարող են փոխվել, մինչդեռ I105-ը այն է
  կայուն հասցե.
- **Մուտքի կանոնականացում.** Torii և SDK-ներն ընդունում են I105 (նախընտրելի)/sora (երկրորդ լավագույն)/0x
  հասցեները գումարած `alias@domain` (rejected legacy form), `uaid:…` և
  `opaque:…` ձևավորվում է, այնուհետև ելքի համար կանոնականացվում է I105-ին: Չկա
  խիստ ռեժիմի անջատում; չմշակված հեռախոսի/էլփոստի նույնացուցիչները պետք է պահվեն մատյանից դուրս
  UAID/անթափանց քարտեզագրումների միջոցով:
- **Սխալների կանխարգելում.** Դրամապանակները վերլուծում են I105 նախածանցները և պարտադրում շղթայական խտրականությունը
  ակնկալիքները. Շղթայական անհամապատասխանությունները գործունակ ախտորոշմամբ առաջացնում են կոշտ ձախողումներ:
- **Կոդեկ գրադարաններ.** Official Rust, TypeScript/JavaScript, Python և Kotlin
  գրադարանները տրամադրում են I105 կոդավորում/վերծանում և սեղմված (`sora`) աջակցություն
  խուսափել մասնատված իրականացումներից. CAIP-10-ի փոխարկումները դեռ չեն առաքվել:

#### Մատչելիության և անվտանգ համօգտագործման ուղեցույց- Արտադրանքի մակերեսների իրականացման ուղեցույցը հետևվում է ուղիղ եթերում
  `docs/portal/docs/reference/address-safety.md`; հղում կատարել այդ ստուգաթերթին, երբ
  այս պահանջները հարմարեցնելով դրամապանակին կամ Explorer UX-ին:
- **Անվտանգ համօգտագործման հոսքեր. ** Մակերեսները, որոնք պատճենում կամ ցուցադրում են, հասցեագրում են I105 ձևի լռելյայն հասցեները և բացահայտում են հարակից «share» գործողությունը, որը ներկայացնում է և՛ ամբողջական տողը, և՛ QR կոդը, որը ստացվում է նույն օգտակար բեռից, որպեսզի օգտվողները կարողանան ստուգել ստուգիչ գումարը տեսողականորեն կամ սկանավորելով: Երբ կրճատումն անխուսափելի է (օրինակ՝ փոքր էկրաններ), պահպանեք տողի սկիզբն ու վերջը, ավելացրեք հստակ էլիպսներ և հասանելի դարձրեք ամբողջական հասցեն՝ պատճենահանման վահանակի միջոցով՝ պատահական կտրվածքը կանխելու համար:
- **IME երաշխիքներ.** Հասցեի մուտքագրումները ՊԵՏՔ Է մերժեն կոմպոզիցիայի արտեֆակտները IME/IME ոճի ստեղնաշարերից: Կիրառեք միայն ASCII մուտքագրումը, ներկայացրեք ներկառուցված նախազգուշացում, երբ հայտնաբերվում են ամբողջ լայնությամբ կամ Kana նիշերը, և առաջարկեք պարզ տեքստի տեղադրման գոտի, որը քերծում է համակցված նշանները նախքան վավերացումը, որպեսզի ճապոնացի և չինացի օգտատերերը կարողանան անջատել իրենց IME-ն առանց առաջընթացը կորցնելու:
- **Էկրանի ընթերցման աջակցություն.** Տրամադրեք տեսողականորեն թաքնված պիտակներ (`aria-label`/`aria-describedby`), որոնք նկարագրում են հիմնական I105 նախածանցի թվանշանները և բաժանում I105 օգտակար բեռը 4 կամ 8 նիշ խմբերի փոխարեն, այնպես որ խմբային նիշերի տեխնոլոգիան կարդում է խմբային նիշերը: Հայտարարեք պատճենման/համօգտագործման հաջողությունը քաղաքավարի կենդանի շրջանների միջոցով և համոզվեք, որ QR նախադիտումները ներառում են նկարագրական այլընտրանքային տեքստ («I105 հասցեն <alias>-ի համար 0x02F1 շղթայում»):
- **Միայն Sora-ի սեղմված օգտագործումը.** `i105` սեղմված տեսքը միշտ նշեք որպես «միայն Sora» և պատճենեք այն հստակ հաստատման հետևում, նախքան պատճենելը: SDK-ները և դրամապանակները պետք է հրաժարվեն սեղմված արտադրանքի ցուցադրումից, երբ շղթայի տարբերակիչը Sora Nexus արժեքը չէ, և պետք է օգտագործողներին ուղղորդեն դեպի I105՝ միջցանցային փոխանցումների համար՝ միջոցների սխալ ուղղումից խուսափելու համար:

## Իրականացման ստուգաթերթ

- **I105 ծրար.** Նախածանցը կոդավորում է `chain_discriminant`-ը՝ օգտագործելով կոմպակտ
  6-/14-բիթանոց սխեման `encode_i105_prefix()`-ից, մարմինը կանոնական բայթ է
  (`AccountAddress::canonical_bytes()`), իսկ ստուգիչ գումարը առաջին երկու բայթն է
  Blake2b-512-ի (`b"I105PRE"` || նախածանց || մարմին): Ամբողջական ծանրաբեռնվածությունը I105- է
  կոդավորված `bs58`-ի միջոցով:
- **Ռեեստրի պայմանագիր.** Ստորագրված JSON (և կամընտիր Merkle արմատ) հրատարակում
  `{discriminant, i105_prefix, chain_alias, endpoints}` 24 ժամ TTL-ով և
  ռոտացիոն ստեղներ.
- **Դոմենի քաղաքականություն.** ASCII `Name` այսօր; եթե i18n-ը միացված է, կիրառեք UTS-46-ի համար
  նորմալացում և UTS-39՝ շփոթեցնող ստուգումների համար: Կիրառել առավելագույն պիտակը (63) և
  ընդհանուր (255) երկարություններ:
- **Տեքստային օգնականներ.** Նավ I105 ↔ սեղմված (`i105`) կոդեկներ Rust-ում,
  TypeScript/JavaScript, Python և Kotlin՝ ընդհանուր թեստային վեկտորներով (CAIP-10
  քարտեզագրումները մնում են ապագա աշխատանք):
- **CLI գործիքավորում. ** Տրամադրեք օպերատորի դետերմինիստական աշխատանքային հոսք `iroha tools address convert`-ի միջոցով
  (տես `crates/iroha_cli/src/address.rs`), որն ընդունում է I105/`0x…` բառացի և
  կամընտիր `<address>@<domain>` (rejected legacy form) պիտակներ, կանխադրված է I105 ելքը՝ օգտագործելով Sora Nexus նախածանցը (`753`),
  և արտանետում է միայն Sora-ի սեղմված այբուբենը, երբ օպերատորները բացահայտորեն պահանջում են այն
  `--format i105` կամ JSON ամփոփման ռեժիմ: Հրամանն կիրառում է նախածանցի ակնկալիքները
  վերլուծել, գրանցել տրամադրված տիրույթը (`input_domain` JSON-ում) և `legacy  suffix` դրոշակը
  վերարտադրում է փոխարկված կոդավորումը որպես `<address>@<domain>` (rejected legacy form), այնպես որ ակնհայտ տարբերությունները մնում են էրգոնոմիկ:
- ** Wallet/explorer UX:** Հետևեք [հասցեի ցուցադրման ուղեցույցներին] (source/sns/address_display_guidelines.md)
  առաքված ADDR-6-ով. առաջարկեք կրկնակի պատճենման կոճակներ, պահեք I105-ը որպես QR օգտակար բեռ և զգուշացրեք
  օգտվողներ, որոնց սեղմված `i105` ձևը միայն Sora-ի համար է և ենթակա է IME-ի վերաշարադրումների:
- **Torii ինտեգրում.** Cache Nexus դրսևորվում է TTL-ի նկատմամբ, արտանետում
  `ForeignDomain`/`UnknownDomain`/`RegistryUnavailable` դետերմինիստորեն, և
  keep strict account-literal parsing canonical-i105-only (reject compressed and any `@domain` suffix) with canonical i105 output.

### Torii պատասխանի ձևաչափեր

- `GET /v1/accounts` ընդունում է կամընտիր `canonical i105 rendering` հարցման պարամետրը և
  `POST /v1/accounts/query`-ն ընդունում է նույն դաշտը JSON ծրարի ներսում:
  Աջակցվող արժեքներն են.
  - `i105` (կանխադրված) — պատասխանները թողարկում են կանոնական I105 օգտակար բեռներ (օրինակ.
    `6cmzPVPX5jDQFNfiz6KgmVfm1fhoAqjPhoPFn4nx9mBWaFMyUCwq4cw`):
  - `i105_default` — պատասխանները թողարկում են միայն Sora-ի համար նախատեսված `i105` սեղմված տեսքը, մինչդեռ
    ֆիլտրերի/ուղու պարամետրերի կանոնական պահպանում:
- Անվավեր արժեքներ են վերադարձվում `400` (`QueryExecutionFail::Conversion`): Սա թույլ է տալիս
  դրամապանակներ և հետազոտողներ՝ սեղմված տողեր պահանջելու միայն Sora-ի UX-ի համար, մինչդեռ
  պահպանելով I105-ը որպես փոխգործունակ լռելյայն:
- Ակտիվների սեփականատերերի ցուցակները (`GET /v1/assets/{definition_id}/holders`) և դրանց JSON-ը
  ծրար գործընկերը (`POST …/holders/query`) նույնպես պատվում է `canonical i105 rendering`:
  `items[*].account_id` դաշտն արտանետում է սեղմված տառեր, երբ
  պարամետր/ծրար դաշտը սահմանվել է `i105_default`՝ արտացոլելով հաշիվները
  վերջնակետեր, որպեսզի հետազոտողները կարողանան հետևողական արդյունք ներկայացնել դիրեկտորիաներում:
- ** Փորձարկում. ** Ավելացրեք միավորի թեստեր կոդավորիչի / ապակոդավորիչի հետադարձ կապի, սխալ շղթայի համար
  ձախողումներ և բացահայտ որոնումներ; ավելացնել ինտեգրացիոն ծածկույթը Torii-ում և SDK-ներում
  I105-ի համար ծայրից ծայր հոսում է:

## Սխալի կոդերի ռեեստր

Հասցեների կոդավորիչները և ապակոդավորիչները բացահայտում են ձախողումները
`AccountAddressError::code_str()`. Հետևյալ աղյուսակները ներկայացնում են կայուն ծածկագրերը
որ SDK-ները, դրամապանակները և Torii մակերեսները պետք է հայտնվեն մարդու կողմից ընթեռնելի մակերեսների կողքին
հաղորդագրություններ, գումարած առաջարկվող վերականգնողական ուղեցույց:

### Կանոնական շինարարություն

| Կոդ | Ձախողում | Առաջարկվող վերականգնում |
|------|---------|-------------------------|
| `ERR_UNSUPPORTED_ALGORITHM` | Կոդավորիչը ստացել է ստորագրման ալգորիթմ, որը չի ապահովվում ռեեստրի կամ կառուցման գործառույթների կողմից: | Սահմանափակել հաշվի կառուցումը ռեեստրում և կազմաձևում միացված կորերով: |
| `ERR_KEY_PAYLOAD_TOO_LONG` | Ստորագրման բանալին օգտակար բեռնվածքի երկարությունը գերազանցում է աջակցվող սահմանաչափը: | Մեկ բանալին կարգավորիչները սահմանափակված են `u8` երկարությամբ; օգտագործել multisig խոշոր հանրային բանալիների համար (օրինակ՝ ML-DSA): |
| `ERR_INVALID_HEADER_VERSION` | Հասցեի վերնագրի տարբերակը աջակցվող տիրույթից դուրս է: | Թողարկեք վերնագրի `0` տարբերակը V1 հասցեների համար; արդիականացրեք կոդավորիչները՝ նախքան նոր տարբերակների ընդունումը: |
| `ERR_INVALID_NORM_VERSION` | Նորմալացման տարբերակի դրոշը չի ճանաչվում: | Օգտագործեք նորմալացման `1` տարբերակը և խուսափեք վերապահված բիթերի փոխարկումից: |
| `ERR_INVALID_I105_PREFIX` | I105 ցանցի պահանջվող նախածանցը չի կարող կոդավորվել: | Ընտրեք նախածանց շղթայի ռեեստրում հրապարակված `0..=16383` ներառական տիրույթում: |
| `ERR_CANONICAL_HASH_FAILURE` | Կանոնական բեռների հեշինգը ձախողվեց: | Կրկին փորձեք գործողությունը; եթե սխալը շարունակվում է, վերաբերվեք այն որպես հեշինգի կույտի ներքին սխալ: |

### Ձևաչափի վերծանում և ավտոմատ հայտնաբերում

| Կոդ | Ձախողում | Առաջարկվող վերականգնում |
|------|---------|-------------------------|
| `ERR_INVALID_I105_ENCODING` | I105 տողը պարունակում է այբուբենից դուրս նիշեր: | Համոզվեք, որ հասցեն օգտագործում է հրապարակված I105 այբուբենը և չի կրճատվել պատճենման/տեղադրման ընթացքում: |
| `ERR_INVALID_LENGTH` | Օգտակար բեռի երկարությունը չի համապատասխանում ընտրիչի/կարգավորիչի համար նախատեսված կանոնական չափին: | Տրամադրեք ամբողջ կանոնական ծանրաբեռնվածությունը ընտրված տիրույթի ընտրիչի և կարգավորիչի դասավորության համար: |
| `ERR_CHECKSUM_MISMATCH` | I105 (նախընտրելի) կամ սեղմված (`sora`, երկրորդ լավագույն) ստուգիչ գումարի վավերացումը ձախողվեց: | Վերականգնել հասցեն վստահելի աղբյուրից. սա սովորաբար ցույց է տալիս պատճենման/տեղադրման սխալ: |
| `ERR_INVALID_I105_PREFIX_ENCODING` | I105 նախածանցի բայթերը սխալ ձևավորված են: | Վերակոդավորել հասցեն համապատասխան կոդավորիչով; մի փոխեք առաջատար I105 բայթերը ձեռքով: |
| `ERR_INVALID_HEX_ADDRESS` | Չհաջողվեց վերծանել կանոնական տասնվեցական ձևը: | Տրամադրեք `0x` նախածանցով, հավասար երկարությամբ վեցանկյուն տող, որը արտադրվել է պաշտոնական կոդավորիչի կողմից: |
| `ERR_MISSING_COMPRESSED_SENTINEL` | Սեղմված ձևը չի սկսվում `sora`-ով: | Նախածանցի սեղմված Sora հասցեները անհրաժեշտ պահակով նախքան դրանք ապակոդավորողներին հանձնելը: |
| `ERR_COMPRESSED_TOO_SHORT` | Սեղմված տողը չունի բավարար թվեր օգտակար բեռի և ստուգիչ գումարի համար: | Օգտագործեք ամբողջական սեղմված տողը, որը թողարկվում է կոդավորիչի կողմից՝ կտրված հատվածների փոխարեն: |
| `ERR_INVALID_COMPRESSED_CHAR` | Հանդիպված սեղմված այբուբենից դուրս նիշ: | Փոխարինեք գրանշանը վավեր Base‑105 հոլովով հրապարակված կիսալայնությամբ/ամբողջ լայնությամբ աղյուսակներից: |
| `ERR_INVALID_COMPRESSED_BASE` | Կոդավորիչը փորձել է օգտագործել չաջակցվող արմատ: | Պատկերացրեք սխալ կոդավորողի դեմ; սեղմված այբուբենը ամրագրված է 105-րդ արմատին V1-ում: |
| `ERR_INVALID_COMPRESSED_DIGIT` | Թվային արժեքը գերազանցում է սեղմված այբուբենի չափը: | Համոզվեք, որ յուրաքանչյուր թվանշանը գտնվում է `0..105)`-ի սահմաններում՝ անհրաժեշտության դեպքում վերականգնելով հասցեն: |
| `ERR_UNSUPPORTED_ADDRESS_FORMAT` | Ավտոմատ հայտնաբերումը չկարողացավ ճանաչել մուտքագրման ձևաչափը: | Տրամադրեք I105 (նախընտրելի), սեղմված (`sora`) կամ կանոնական `0x` վեցանկյուն տողեր վերլուծիչներ կանչելիս: |

### Դոմենի և ցանցի վավերացում| Կոդ | Ձախողում | Առաջարկվող վերականգնում |
|------|---------|-------------------------|
| `ERR_DOMAIN_MISMATCH` | Դոմենի ընտրիչը չի համապատասխանում ակնկալվող տիրույթին: | Օգտագործեք նախատեսված տիրույթի համար տրված հասցե կամ թարմացրեք ակնկալիքը: |
| `ERR_INVALID_DOMAIN_LABEL` | Դոմենի պիտակի նորմալացման ստուգումները ձախողվեցին: | Կանոնականացրեք տիրույթը՝ օգտագործելով UTS-46 ոչ անցումային մշակումը նախքան կոդավորումը: |
| `ERR_UNEXPECTED_NETWORK_PREFIX` | Ապակոդավորված I105 ցանցի նախածանցը տարբերվում է կազմաձևված արժեքից: | Անցեք հասցեի թիրախային շղթայից կամ կարգավորեք սպասվող դիսկրիմինանտը/նախածանցը: |
| `ERR_UNKNOWN_ADDRESS_CLASS` | Հասցեների դասի բիթերը չեն ճանաչվում: | Թարմացրեք ապակոդավորիչը մի թողարկման, որը հասկանում է նոր դասը, կամ խուսափեք վերնագրի բիթերի կեղծումից: |
| `ERR_UNKNOWN_DOMAIN_TAG` | Դոմենի ընտրիչի պիտակը անհայտ է: | Թարմացրեք թողարկումը, որն աջակցում է ընտրիչի նոր տեսակը կամ խուսափեք փորձնական բեռների օգտագործումից V1 հանգույցներում: |
| `ERR_UNEXPECTED_EXTENSION_FLAG` | Վերապահված ընդլայնման բիթը սահմանվեց: | Մաքրել վերապահված բիթերը; նրանք մնում են փակ, մինչև ապագա ABI-ն չներկայացնի նրանց: |
| `ERR_UNKNOWN_CONTROLLER_TAG` | Կարգավորիչի օգտակար բեռի պիտակը չի ճանաչվել: | Թարմացրեք ապակոդավորիչը՝ կարգավորիչների նոր տեսակները ճանաչելու համար՝ նախքան դրանք վերլուծելը: |
| `ERR_UNEXPECTED_TRAILING_BYTES` | Կանոնական ծանրաբեռնվածությունը վերծանումից հետո պարունակում էր հետին բայթեր: | Վերականգնել կանոնական ծանրաբեռնվածությունը; պետք է ներկա լինի միայն փաստաթղթավորված երկարությունը: |

### Կարգավորիչի օգտակար բեռի վավերացում

| Կոդ | Ձախողում | Առաջարկվող վերականգնում |
|------|---------|-------------------------|
| `ERR_INVALID_PUBLIC_KEY` | Հիմնական բայթերը չեն համապատասխանում հայտարարված կորին: | Համոզվեք, որ հիմնական բայթերը կոդավորված են ճիշտ այնպես, ինչպես պահանջվում է ընտրված կորի համար (օրինակ՝ 32 բայթ Ed25519): |
| `ERR_UNKNOWN_CURVE` | Կորի նույնացուցիչը գրանցված չէ: | Օգտագործեք կորի ID `1` (Ed25519) մինչև լրացուցիչ կորերը հաստատվեն և հրապարակվեն գրանցամատյանում: |
| `ERR_MULTISIG_MEMBER_OVERFLOW` | Multisig կարգավորիչը հայտարարում է ավելի շատ անդամներ, քան աջակցվում է: | Նախքան կոդավորումը կրճատեք բազմանշանակ անդամակցությունը մինչև փաստաթղթավորված սահմանաչափը: |
| `ERR_INVALID_MULTISIG_POLICY` | Multisig քաղաքականության օգտակար բեռի վավերացումը ձախողվեց (շեմ/կշիռներ/սխեման): | Վերակառուցեք քաղաքականությունը, որպեսզի այն բավարարի CTAP2 սխեման, քաշի սահմանները և շեմային սահմանափակումները: |

## Դիտարկված այլընտրանքներ

- ** Pure checksum envelope (Bitcoin-ի ոճով):** Ավելի պարզ ստուգիչ գումար, բայց ավելի թույլ սխալի հայտնաբերում
  քան Blake2b-ից ստացված I105 ստուգիչ գումարը (`encode_i105`-ը կտրում է 512-բիթանոց հեշը)
  և չունի հստակ նախածանցային իմաստաբանություն 16-բիթանոց դիսկրիմինանտների համար:
- **Շղթայի անվան ներդրում տիրույթի տողում (օրինակ՝ `finance@chain`):** Խզվում է
- **Հենվեք բացառապես Nexus երթուղղման վրա՝ առանց հասցեները փոխելու:** Օգտատերերը դեռևս կցանկանային
  պատճենել/տեղադրել երկիմաստ տողեր; մենք ցանկանում ենք, որ հասցեն ինքնին ունենա համատեքստ:
- **Bech32m ծրար։** QR հարմար է և առաջարկում է մարդու համար ընթեռնելի նախածանց, բայց
  կշեղվեր առաքման I105 իրականացումից (`AccountAddress::to_i105`)
  և պահանջում են վերստեղծել բոլոր հարմարանքները/SDK-ները: Ներկայիս ճանապարհային քարտեզը պահպանում է I105 +
  սեղմված (`sora`) աջակցություն ապագայի վերաբերյալ հետազոտությունը շարունակելիս
  Bech32m/QR շերտեր (CAIP-10 քարտեզագրումը հետաձգված է):

## Բաց Հարցեր

- Հաստատեք, որ `u16` տարբերակիչները գումարած վերապահված միջակայքերը ծածկում են երկարաժամկետ պահանջարկը.
  հակառակ դեպքում գնահատեք `u32`-ը varint կոդավորմամբ:
- Վերջնականացնել բազմաստորագրության կառավարման գործընթացը ռեեստրի թարմացումների համար և ինչպես
  չեղյալ համարվող/ժամկետանց հատկացումները մշակվում են:
- Սահմանեք մանիֆեստի ստորագրության ճշգրիտ սխեման (օրինակ՝ Ed25519 բազմանշանակ) և
  տրանսպորտային անվտանգություն (HTTPS ամրացում, IPFS հեշ ձևաչափ) Nexus բաշխման համար:
- Որոշեք, թե արդյոք աջակցել տիրույթի փոխանուններին/վերահղումներին միգրացիայի համար և ինչպես
  դրանք երես հանել՝ չխախտելով դետերմինիզմը:
- Նշեք, թե ինչպես են Kotodama/IVM պայմանագրերը մուտք գործում I105 օգնականներին (`to_address()`,
  `parse_address()`) և արդյոք շղթայական պահեստավորումը երբևէ պետք է բացահայտի CAIP-10-ը
  քարտեզագրումներ (այսօր I105-ը կանոնական է):
- Ուսումնասիրեք Iroha շղթաների գրանցումը արտաքին ռեգիստրներում (օրինակ՝ I105 ռեեստրում,
  CAIP անվանատարածքի գրացուցակ) էկոհամակարգերի ավելի լայն հավասարեցման համար:

## Հաջորդ քայլերը

1. I105 կոդավորումը վայրէջք է կատարել `iroha_data_model`-ում (`AccountAddress::to_i105`,
   `parse_encoded`); շարունակեք տեղափոխել հարմարանքները/թեստերը յուրաքանչյուր SDK-ի վրա և մաքրեք ցանկացածը
   Bech32m տեղապահներ.
2. Ընդլայնել կազմաձևման սխեման `chain_discriminant`-ով և ստանալ խելամիտ
  լռելյայններ առկա թեստային/ծրագրավորող կարգավորումների համար: **(Կատարված է՝ `common.chain_discriminant`
  այժմ առաքվում է `iroha_config`-ով, լռելյայն սահմանելով `0x02F1` յուրաքանչյուր ցանցում
  գերազանցում է։)**
3. Կազմեք Nexus ռեեստրի սխեման և հայեցակարգի ապացույցի մանիֆեստի հրատարակիչ:
4. Հավաքեք արձագանքներ դրամապանակների մատակարարներից և պահառուներից մարդկային գործոնի ասպեկտների վերաբերյալ
   (HRP անվանում, ցուցադրման ձևաչափում):
5. Թարմացրեք փաստաթղթերը (`docs/source/data_model.md`, Torii API փաստաթղթեր) մեկ անգամ
   իրականացման ուղին հաստատված է.
6. Առաքեք պաշտոնական կոդեկների գրադարանները (Rust/TS/Python/Kotlin) նորմատիվ թեստով
   վեկտորներ, որոնք ընդգրկում են հաջողության և ձախողման դեպքերը:
