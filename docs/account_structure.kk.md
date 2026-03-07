---
lang: kk
direction: ltr
source: docs/account_structure.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 01561366c9698c10d29ff3f49ad4c14b22b1796b5c7701cf98d200a140af1caf
source_last_modified: "2026-01-28T17:11:30.635172+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Есептік жазба құрылымы RFC

**Күйі:** Қабылданды (ADDR-1)  
**Аудитория:** Деректер үлгісі, Torii, Nexus, Wallet, Басқару топтары  
**Байланысты мәселелер:** TBD

## Түйіндеме

Бұл құжат ішінде жүзеге асырылған жөнелту тіркелгісінің адрестеу стекін сипаттайды
`AccountAddress` (`crates/iroha_data_model/src/account/address.rs`) және
серіктес құрал. Ол қамтамасыз етеді:

- Бақылау сомасы, адамға бағытталған **Iroha Base58 мекенжайы (IH58)**
  Тізбек дискриминантын тіркелгіге байланыстыратын `AccountAddress::to_ih58`
  контроллер және детерминирленген өзара әрекеттесуге ыңғайлы мәтіндік пішіндерді ұсынады.
- жасырын әдепкі домендерге және жергілікті дайджесттерге арналған домен селекторлары
  Болашақ Nexus қолдауы бар маршруттау үшін сақталған жаһандық тізілім селектор тегі (
  тізілімді іздеу **әлі жөнелтілмеген**).

## Мотивация

Әмияндар мен тізбектен тыс құралдар бүгінгі таңда өңделмеген `alias@domain` бағыттау бүркеншік аттарына сүйенеді. Бұл
екі негізгі кемшілігі бар:

1. **Желіге байланыстыру жоқ.** Жолда бақылау сомасы немесе тізбек префиксі жоқ, сондықтан пайдаланушылар
   дереу кері байланыссыз қате желіден мекенжайды қоя алады. The
   транзакция ақыр соңында қабылданбайды (тізбек сәйкес келмеуі) немесе одан да жаманы сәтті болады
   тағайындалмаған тіркелгіге қарсы, егер тағайындалған жер жергілікті жерде болса.
2. **Домен соқтығысуы.** Домендер тек аттар кеңістігі болып табылады және оларды әрқайсысында қайта пайдалануға болады
   тізбек. Қызметтер федерациясы (кастодиандар, көпірлер, кросс-тізбекті жұмыс процестері)
   сынғыш болады, себебі А тізбегіндегі `finance` `finance` байланысы жоқ.
   тізбек В.

Бізге көшіру/қою қателерінен қорғайтын адамға қолайлы мекенжай пішімі қажет
және домендік атаудан беделді тізбекке детерминирленген салыстыру.

## Мақсаттар

- Деректер үлгісінде енгізілген IH58 Base58 конвертін сипаттаңыз және
  `AccountId` және `AccountAddress` орындайтын канондық талдау/бүркеншік ат ережелері.
- Конфигурацияланған тізбек дискриминантын тікелей әрбір мекенжайға кодтау және
  оның басқару/тіркеу процесін анықтау.
- Ағымды үзбестен ғаламдық домен тізілімін енгізу жолын сипаттаңыз
  орналастыру және қалыпқа келтіру/спуфингке қарсы ережелерді көрсетіңіз.

## Мақсатсыз

- Активтердің тізбекаралық трансферттерін жүзеге асыру. Маршруттау қабаты тек қайтарады
  мақсатты тізбек.
- Жаһандық домен шығару үшін басқаруды аяқтау. Бұл RFC деректерге назар аударады
  модель және тасымалдау примитивтері.

## Фон

### Ағымдағы маршруттау бүркеншік аты

```
AccountId {
    domain: DomainId,   // wrapper over Name (ASCII-ish string)
    controller: AccountController // single PublicKey or multisig policy
}

Display: canonical IH58 literal (no `@domain` suffix)
Parse accepts:
- IH58 (preferred), `sora` compressed, or canonical hex (`0x...`) inputs, with
  optional `@<domain>` suffixes for explicit routing hints.
- `<label>@<domain>` aliases resolved through the account-alias resolver
  (Torii installs one; plain data-model parsing requires a resolver to be set).
- `<public_key>@<domain>` where `public_key` is the canonical multihash string.
- `uaid:<hex>` / `opaque:<hex>` literals resolved via UAID/opaque resolvers.

Multihash hex is canonical: varint bytes are lowercase hex, payload bytes are uppercase hex,
and `0x` prefixes are not accepted.

This text form is now treated as an **account alias**: a routing convenience
that points to the canonical [`AccountAddress`](#2-canonical-address-codecs).
It remains useful for human readability and domain-scoped governance, but it is
no longer considered the authoritative account identifier on-chain.
```

`ChainId` `AccountId` сыртында тұрады. Түйіндер транзакцияның `ChainId` тексереді
қабылдау кезінде конфигурацияға қарсы (`AcceptTransactionFail::ChainIdMismatch`)
және шетелдік транзакцияларды қабылдамайды, бірақ шот жолының өзі жоқ
желілік анықтама.

### Домен идентификаторлары

`DomainId` `Name` (қалыпты жол) орап, жергілікті тізбекке қамтылған.
Әрбір тізбек `wonderland`, `finance` және т.б. дербес тіркей алады.

### Nexus контекст

Nexus құрамдас аралық үйлестіруге жауапты (жолақтар/деректер кеңістігі). Ол
қазіргі уақытта кросс-тізбекті доменді бағыттау тұжырымдамасы жоқ.

## Ұсынылған дизайн

### 1. Детерминирленген тізбек дискриминанты

`iroha_config::parameters::actual::Common` енді мынаны көрсетеді:

```rust
pub struct Common {
    pub chain: ChainId,
    pub chain_discriminant: u16, // globally coordinated
    // ... existing fields
}
```

- **Шектеулер:**
  - Белсенді желі үшін бірегей; қол қойылған мемлекеттік тізілім арқылы басқарылады
    анық сақталған ауқымдар (мысалы, `0x0000–0x0FFF` сынақ/дев, `0x1000–0x7FFF`
    қауымдастықты бөлу, `0x8000–0xFFEF` басқару мақұлдаған, `0xFFF0–0xFFFF`
    сақталған).
  - Жүру тізбегі үшін өзгермейтін. Оны өзгерту үшін қатты шанышқы және а
    тізілімді жаңарту.
- **Басқару және тізілім (жоспарлы):** Көп қолтаңбалы басқару жинағы
  қол қойылған JSON тізілімін адамның бүркеншік аттарымен салыстыру дискриминанттарын сақтау және
  CAIP-2 идентификаторлары. Бұл тізілім әлі жеткізілген орындалу уақытының бөлігі емес.
- **Қолданылуы:** Мемлекеттік рұқсат, Torii, SDK және әмиян API интерфейстері арқылы беріледі.
  әрбір құрамдас оны ендіре алады немесе тексере алады. CAIP-2 экспозициясы болашақ болып қала береді
  өзара әрекеттесу тапсырмасы.

### 2. Канондық мекенжай кодектері

Rust деректер үлгісі бір канондық пайдалы жүктеме көрінісін көрсетеді
(`AccountAddress`) бірнеше адамға арналған форматтар ретінде шығарылуы мүмкін. IH58 болып табылады
ортақ пайдалану және канондық шығару үшін қолайлы тіркелгі пішімі; қысылған
`sora` пішіні UX үшін екінші ең жақсы, тек Sora нұсқасы, мұнда кана алфавиті бар
құндылық қосады. Канондық он алтылық отладтау құралы болып қала береді.

- **IH58 (Iroha Base58)** – тізбекті ендіретін Base58 конверті
  дискриминант. Декодерлер пайдалы жүктемені жылжытпас бұрын префиксті тексереді
  канондық формасы.
- **Сорамен қысылған көрініс** – құрастырған **105 таңбадан** тұратын тек Sora әліпбиі
  58 таңбаға жарты ені イロハ өлеңін (ヰ және ヱ қоса алғанда) қосу
  IH58 жинағы. Жолдар `sora` күзетшісінен басталады, Bech32m туындысын ендіреді
  бақылау сомасын енгізіп, желі префиксін өткізіп жіберіңіз (Sora Nexus дегенді күзетші меңзейді).

  ```
  IH58  : 123456789ABCDEFGHJKMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz
  Iroha : ｲﾛﾊﾆﾎﾍﾄﾁﾘﾇﾙｦﾜｶﾖﾀﾚｿﾂﾈﾅﾗﾑｳヰﾉｵｸﾔﾏｹﾌｺｴﾃｱｻｷﾕﾒﾐｼヱﾋﾓｾｽ
  ```
- **Канондық алтылық** – канондық байтты түзетуге ыңғайлы `0x…` кодтауы
  конверт.

`AccountAddress::parse_any` IH58 (қалаулы), қысылған (`sora`, екінші ең жақсы) немесе канондық он алтылықты автоматты түрде анықтайды
(тек `0x...`; жалаң он алтылық қабылданбайды) декодталған пайдалы жүктемені де, анықталған мәнді де енгізеді және қайтарады.
`AccountAddressFormat`. Torii енді ISO 20022 қосымшасы үшін `parse_any` деп атайды
метадеректер детерминирленген болып қалатындай канондық он алтылық пішінді мекендейді және сақтайды
бастапқы ұсынуына қарамастан.

#### 2.1 Тақырып байт орналасуы (ADDR-1a)

Әрбір канондық пайдалы жүктеме `header · controller` ретінде көрсетілген. The
`header` - бұл байттарға қандай талдаушы ережелері қолданылатынын көрсететін жалғыз байт.
орындаңыз:

```
bit index:   7        5 4      3 2      1 0
             ┌─────────┬────────┬────────┬────┐
payload bit: │version  │ class  │  norm  │ext │
             └─────────┴────────┴────────┴────┘
```

Сондықтан бірінші байт төменде декодерлерге арналған схема метадеректерін жинайды:

| Биттар | Өріс | Рұқсат етілген мәндер | Бұзушылықтағы қате |
|------|-------|----------------|--------------------|
| 7-5 | `addr_version` | `0` (v1). `1-7` мәндері болашақ түзетулер үшін сақталған. | `0-7` триггерінен тыс мәндер `AccountAddressError::InvalidHeaderVersion`; іске асырулар нөлден басқа нұсқаларды бүгінде қолдау көрсетілмейтін ретінде қарастыруы керек. |
| 4-3 | `addr_class` | `0` = жалғыз кілт, `1` = мультисиг. | Басқа мәндер `AccountAddressError::UnknownAddressClass` жоғарылайды. |
| 2-1 | `norm_version` | `1` (Норма v1). `0`, `2`, `3` мәндері сақталған. | `0-3` тыс мәндер `AccountAddressError::InvalidNormVersion` жоғарылайды. |
| 0 | `ext_flag` | `0` болуы керек. | Орнату биті `AccountAddressError::UnexpectedExtensionFlag` жоғарылайды. |

Rust кодері бір кілтті контроллерлер үшін `0x02` жазады (0 нұсқа, 0 сынып,
v1 нормасы, кеңейтім жалаушасы тазартылды) және мультисиг контроллері үшін `0x0A` (0 нұсқасы,
1-сынып, v1 нормасы, кеңейту жалауы тазартылған).

#### 2.2 Legacy selector compatibility (decode-only)

Newly encoded canonical payloads do not include a domain-selector segment. For
backward compatibility, decoders still accept pre-cutover payloads where a
selector segment appears between header and controller as a tagged union:

| Tag | Meaning | Payload | Notes |
|-----|---------|---------|-------|
| `0x00` | Implicit default domain | none | Matches the configured `default_domain_name()` (legacy decode only). |
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

#### 2.3 Контроллердің пайдалы жүктеме кодтаулары (ADDR-1a)

Контроллердің пайдалы жүктемесі домен селекторынан кейін қосылған басқа тегтелген бірлестік болып табылады:| Тег | Контроллер | Макет | Ескертпелер |
|-----|------------|--------|-------|
| `0x00` | Бір кілт | `curve_id:u8` · `key_len:u8` · `key_bytes` | `curve_id=0x01` бүгін Ed25519 картасына сәйкес келеді. `key_len` `u8` шектелген; үлкен мәндер `AccountAddressError::KeyPayloadTooLong` жоғарылайды (сондықтан >255 байт болатын бір кілтті ML-DSA ашық кілттерін кодтауға болмайды және мультисигті пайдалану керек). |
| `0x01` | Multisig | `version:u8` · `threshold:u16` · `member_count:u8` · (`curve_id:u8` · `weight:u16` · `key_len:u16` · `key_len:u16`)\* | 255 мүшеге дейін қолдайды (`CONTROLLER_MULTISIG_MEMBER_MAX`). Белгісіз қисықтар `AccountAddressError::UnknownCurve` көтереді; дұрыс емес саясаттар `AccountAddressError::InvalidMultisigPolicy` ретінде көпіршіктенеді. |

Multisig саясаты сонымен қатар CTAP2 стиліндегі CBOR картасын және канондық дайджестті көрсетеді
хосттар мен SDK контроллерді анықтаушы түрде тексере алады. Қараңыз
Схема үшін `docs/source/references/multisig_policy_schema.md` (ADDR-1c),
валидация ережелері, хэштеу процедурасы және алтын арматура.

Барлық кілт байттары дәл `PublicKey::to_bytes` қайтарғандай кодталған; декодерлер `PublicKey` даналарын қайта құрастырады және байттар жарияланған қисыққа сәйкес келмесе, `AccountAddressError::InvalidPublicKey` жоғарылайды.

> **Ed25519 канондық орындау (ADDR-3a):** `0x01` қисығы пернелері қол қоюшы шығаратын нақты байт жолын декодтауы керек және шағын ретті ішкі топта жатпауы керек. Түйіндер енді канондық емес кодтауларды (мысалы, `2^255-19` модулінің азайтылған мәндері) және сәйкестендіру элементі сияқты әлсіз нүктелерден бас тартады, сондықтан SDK мекенжайларды жібермес бұрын сәйкестік тексеру қателерін көрсетуі керек.

##### 2.3.1 Қисық идентификатор тізілімі (ADDR-1d)

| ID (`curve_id`) | Алгоритм | Мүмкіндік қақпасы | Ескертпелер |
|----------------|-----------|--------------|-------|
| `0x00` | Резервтелген | — | ШЫҒЫРЫЛМАУ КЕРЕК; декодерлердің беті `ERR_UNKNOWN_CURVE`. |
| `0x01` | Ed25519 | — | Канондық v1 алгоритмі (`Algorithm::Ed25519`); әдепкі конфигурацияда қосылған. |
| `0x02` | ML‑DSA (Dilithium3) | — | Dilithium3 ашық кілт байттарын (1952 байт) пайдаланады. Бір кілтті мекенжайлар ML‑DSA кодтай алмайды, себебі `key_len` `u8`; multisig `u16` ұзындықтарын пайдаланады. |
| `0x03` | BLS12‑381 (қалыпты) | `bls` | G1-дегі ашық кілттер (48 байт), G2-дегі қолтаңбалар (96 байт). |
| `0x04` | secp256k1 | — | SHA‑256 бойынша детерминистік ECDSA; ашық кілттер 33 байт SEC1 қысылған пішінін пайдаланады, ал қолтаңбалар канондық 64 байт `r∥s` орналасуын пайдаланады. |
| `0x05` | BLS12‑381 (кіші) | `bls` | G2-дегі ашық кілттер (96 байт), G1-дегі қолтаңбалар (48 байт). |
| `0x0A` | ГОСТ Р 34.10‑2012 (256, А жинағы) | `gost` | `gost` мүмкіндігі қосылғанда ғана қолжетімді. |
| `0x0B` | ГОСТ Р 34.10‑2012 (256, В жинағы) | `gost` | `gost` мүмкіндігі қосылғанда ғана қолжетімді. |
| `0x0C` | ГОСТ Р 34.10‑2012 (256, С жинағы) | `gost` | `gost` мүмкіндігі қосылғанда ғана қолжетімді. |
| `0x0D` | ГОСТ Р 34.10‑2012 (512, А жинағы) | `gost` | `gost` мүмкіндігі қосылғанда ғана қолжетімді. |
| `0x0E` | ГОСТ Р 34.10‑2012 (512, В жинағы) | `gost` | `gost` мүмкіндігі қосылғанда ғана қолжетімді. |
| `0x0F` | SM2 | `sm` | DistID ұзындығы (u16 BE) + DistID байттары + 65-байт SEC1 қысылмаған SM2 пернесі; `sm` қосылғанда ғана қолжетімді. |

`0x06–0x09` ұяшықтары қосымша қисықтар үшін тағайындалмаған күйінде қалады; жаңасымен таныстыру
алгоритм жол картасын жаңартуды және сәйкес SDK/хост қамтуды қажет етеді. Кодерлер
`ERR_UNSUPPORTED_ALGORITHM` көмегімен кез келген қолдау көрсетілмейтін алгоритмді қабылдамау керек және
декодерлер сақтау үшін `ERR_UNKNOWN_CURVE` белгісіз идентификаторларда тез істен шығуы КЕРЕК
сәтсіз жабық мінез-құлық.

Канондық тізілім (соның ішінде машинада оқылатын JSON экспорты) астында тұрады
[`docs/source/references/address_curve_registry.md`](source/references/address_curve_registry.md).
Құралдар қисық идентификаторлар қалуы үшін бұл деректер жиынын тікелей тұтынуы КЕРЕК
SDK және оператордың жұмыс үрдістері бойынша үйлесімді.

- **SDK шлюзі:** SDK әдепкі бойынша Ed25519-ға ғана тексеру/кодтау. Свифт көрсетеді
  компиляция уақыты жалаулары (`IROHASWIFT_ENABLE_MLDSA`, `IROHASWIFT_ENABLE_GOST`,
  `IROHASWIFT_ENABLE_SM`); Java/Android SDK қажет
  `AccountAddress.configureCurveSupport(...)`; JavaScript SDK пайдаланады
  `configureCurveSupport({ allowMlDsa: true, allowGost: true, allowSm2: true })`.
  secp256k1 қолдауы қолжетімді, бірақ JS/Android жүйесінде әдепкі бойынша қосылмаған
  SDKs; Ed25519 емес контроллерлерді шығару кезінде қоңырау шалушылар анық түрде қосылуы керек.
- **Хост қақпасы:** `Register<Account>` қол қоюшылары алгоритмдерді пайдаланатын контроллерлерді қабылдамайды
  түйіннің `crypto.allowed_signing` тізімінде жоқ **немесе** қисық идентификаторлары жоқ
  `crypto.curves.allowed_curve_ids`, сондықтан кластерлер қолдауды жарнамалауы керек (конфигурация +
  genesis) ML‑DSA/GOST/SM контроллерлерін тіркеуге дейін. BLS контроллері
  құрастыру кезінде алгоритмдерге әрқашан рұқсат етіледі (консенсус кілттері оларға сүйенеді),
  және әдепкі конфигурация Ed25519 + secp256k1 қосады.【crates/iroha_core/src/smartcontracts/isi/domain.rs:32】

##### 2.3.2 Multisig контроллері бойынша нұсқаулық

`AccountController::Multisig` арқылы саясаттарды сериялайды
`crates/iroha_data_model/src/account/controller.rs` және схеманы жүзеге асырады
[`docs/source/references/multisig_policy_schema.md`](source/references/multisig_policy_schema.md) ішінде құжатталған.
Негізгі іске асыру мәліметтері:

- Саясаттар бұрын `MultisigPolicy::validate()` арқылы қалыпқа келтірілген және расталған
  ендірілген. Шекті мәндер ≥1 және ≤Σ салмақ болуы керек; қайталанатын мүшелер
  `(algorithm || 0x00 || key_bytes)` бойынша сұрыптаудан кейін анықтаушы түрде жойылды.
- Екілік контроллердің пайдалы жүктемесі (`ControllerPayload::Multisig`) кодтайды
  `version:u8`, `threshold:u16`, `member_count:u8`, содан кейін әрбір мүшенің
  `(curve_id, weight:u16, key_len:u16, key_bytes)`. Бұл дәл солай
  `AccountAddress::canonical_bytes()` IH58 (таңдаулы)/sora (екінші ең жақсы) пайдалы жүктемелерге жазады.
- Хэшинг (`MultisigPolicy::digest_blake2b256()`) Blake2b-256 пайдаланады.
  `iroha-ms-policy` жекелендіру жолы, сондықтан басқару манифесттері
  IH58 ішіне енгізілген контроллер байттарына сәйкес келетін детерминирленген саясат идентификаторы.
- Арматураны қамту мерзімі `fixtures/account/address_vectors.json` (жағдайлар
  `addr-multisig-*`). Әмияндар мен SDK канондық IH58 жолдарын бекітуі керек
  олардың кодтауыштарының Rust енгізуіне сәйкестігін растау үшін төменде.

| Іс идентификаторы | Табалдырық / мүшелер | IH58 литералы (`0x02F1` префиксі) | Сора қысылған (`sora`) әріптік | Ескертпелер |
|---------|--------------------|--------------------------------|-------------------------|-------|
| `addr-multisig-council-threshold3` | `≥3` салмағы, мүшелері `(2,1,1)` | `SRfSHsrH3tEmYaaAYyD248F3vfT1oQ3WEGS22MaD8W9bLefF7rsoKLYGcpbcM9EcSus5ZhCAZU7ztn2BCsyeCAdfRncAVmVsipd4ibk6CBLF3Nrzcw8P7VKJg6mtFgEhWVTjfDkUMoc63oeEmaWyV6cyiphwk8ZgKAJUe4TyVtmKm1WWcg7qZ6i` | `sora3vﾑ2zkaoUwﾋﾅGﾘﾚyﾂe3ﾖfﾙヰｶﾘﾉwｷnoWﾛYicaUr3ﾔｲﾖ2Ado3TﾘYQﾉJqﾜﾇｳﾑﾐd8dDjRGｦ3Vﾃ9HcﾀMヰR8ﾎﾖgEqGｵEｾDyc5ﾁ1ﾔﾉ31sUﾑﾀﾖaｸxﾘ3ｲｷMEuFｺｿﾉBQSVQnxﾈeJzrXLヰhｿｹ5SEEﾅPﾂﾗｸdヰﾋ1bUGHｲVXBWNNJ6K` | Кеңес-домендік басқару кворумы. |
| `addr-multisig-wonderland-threshold2` | `≥2`, мүшелері `(1,2)` | `3xsmkps1KPBn9dtpE5qHRhHEZCpiAe8d9j6H9A42TV6kc1TpaqdwnSksKgQrsSEHznqvWKBMc1os69BELzkLjsR7EV2gjV14d9JMzo97KEmYoKtxCrFeKFAcy7ffQdboV1uRt` | `sora2ﾖZﾘeｴAdx3ﾂﾉﾔXhnｹﾀ2ﾉｱﾋxﾅﾄﾌヱwﾐmﾊvEﾐCﾏﾎｦ1ﾑHﾋso2GKﾔﾕﾁwﾂﾃP6ﾁｼﾙﾖｺ9ｻｦbﾈ4wFdﾑFヰ3HaﾘｼMｷﾌHWtｷﾋLﾙﾖQ4D3XﾊﾜXmpktﾚｻ5ﾅﾅﾇ1gkﾏsCFQGH9` | Қос қолтаңба ғажайыптар әлемінің мысалы (салмақ 1 + 2). |
| `addr-multisig-default-quorum3` | `≥3`, мүшелері `(1,1,1,1)` | `nA2bDNhMqXz7ERkHNoEWbvJGyR1aDRsw32LaUWLgbK3vcpzohmdFCLvdotxUWWDY3aZeX4ptLk4Z6TjF5ossnJm8VrNo6daxmGTkqUyP4MxJxiNyPFxsEE5DLnsoLWUcxaWNpZ76tmkbiGS31Gv8tejKpuiHUMaQ1s5ohWyZvDnpycNkBK8AEfGJqn5yc9zAzfWbVhpDwkPj8ScnzvH1Echr5` | `soraﾐ38ﾅｴｸﾜ8ﾃzwBrqﾘｺ4yﾄv6kqJp1ｳｱﾛｿrzﾄﾃﾘﾒRﾗtV9ｼﾔPｽcヱEﾌVVVｼﾘｲZAｦﾓﾅｦeﾒN76vﾈcuｶuﾛL54rzﾙﾏX2zMﾌRLﾃﾋpﾚpｲcHﾑﾅﾃﾔzｵｲVfAﾃﾚﾎﾚCヰﾔｲｽｦw9ﾔﾕ8bGGkﾁ6sNｼaｻRﾖﾜYﾕﾚU18ﾅHヰﾌuMeﾊtﾂrｿj95Ft8ﾜ3fﾄkNiｴuﾈrCﾐQt8ヱｸｸmﾙﾒgUbﾑEKTTCM` | Негізгі басқару үшін қолданылатын жасырын-әдепкі домен кворумы.

#### 2.4 Сәтсіздік ережелері (ADDR-1a)

- Қажетті тақырып + селектордан қысқарақ немесе қалған байттары бар пайдалы жүктемелер `AccountAddressError::InvalidLength` немесе `AccountAddressError::UnexpectedTrailingBytes` шығарады.
- Сақталған `ext_flag` параметрін орнататын немесе қолдау көрсетілмейтін нұсқаларды/сыныптарды жарнамалайтын тақырыптар `UnexpectedExtensionFlag`, `InvalidHeaderVersion` немесе `UnknownAddressClass` арқылы қабылданбауы КЕРЕК.
- Белгісіз селектор/контроллер тегтері `UnknownDomainTag` немесе `UnknownControllerTag` көтереді.
- Өлшемі үлкен немесе дұрыс емес негізгі материал `KeyPayloadTooLong` немесе `InvalidPublicKey` көтереді.
- 255 мүшеден асатын мультисиг контроллері `MultisigMemberOverflow` жоғарылайды.
- IME/NFKC түрлендірулері: жарты ені Sora kana кодты декодтауды бұзбай олардың толық ендік пішіндеріне қалыпқа келтіруге болады, бірақ ASCII `sora` sentinel және IH58 сандары/әріптері ASCII болып қалуы КЕРЕК. Толық ені немесе корпусы бүктелген күзетшілер беті `ERR_MISSING_COMPRESSED_SENTINEL`, толық ені ASCII пайдалы жүктемелері `ERR_INVALID_COMPRESSED_CHAR` жоғарылайды және бақылау сомасының сәйкессіздігі `ERR_CHECKSUM_MISMATCH` ретінде көпіршіктенеді. `crates/iroha_data_model/src/account/address.rs` жүйесіндегі сипат сынақтары осы жолдарды қамтиды, осылайша SDK және әмияндар детерминирленген сәтсіздіктерге сене алады.
- Torii және `address@domain` бүркеншік аттарының SDK талдауы енді IH58 (таңдаулы)/sora (екінші ең жақсы) кірістер бүркеншік аттың қалпына келуіне дейін сәтсіздікке ұшыраған кезде (мысалы, домен құрылымының қателігі, сондықтан тексеру сомасының қатесі қайталануы мүмкін), енді `ERR_*` кодтарын шығарады. прозалық жолдардан болжау.
- 12 байттан қысқа жергілікті селектордың пайдалы жүктемелері `ERR_LOCAL8_DEPRECATED` бетін береді, бұл бұрынғы Local‑8 дайджесттерінен қатты үзіндіні сақтайды.
- Domainless IH58 (preferred)/sora (second-best) literals bind directly to the configured default domain label for canonical selector-free payloads. Legacy selector-bearing literals without an explicit `@<domain>` suffix may still fail with `ERR_DOMAIN_SELECTOR_UNRESOLVED` when domain reconstruction is impossible.

#### 2.5 Нормативтік екілік векторлар

- **Жасырын әдепкі домен (`default`, бастапқы байты `0x00`)**  
  Канондық алтылық: `0x020001203b6a27bcceb6a42d62a3a8d02a6f0d73653215771de243a63ac048a18b59da29`.  
  Бөлім: `0x02` тақырыбы, `0x00` селекторы (жасырын әдепкі), `0x00` контроллер тегі, `0x01` қисық идентификаторы (Ed25519), I18NI000 кілт ұзындығы, содан кейін 3-жүктеу кілті.
- **Жергілікті домен дайджесті (`treasury`, тұқымдық байты `0x01`)**  
  Канондық алтылық: `0x0201b18fe9c1abbac45b3e38fc5d0001208a88e3dd7409f195fd52db2d3cba5d72ca6709bf1d94121bf3748801b40f6f5c`.  
  Бөлім: `0x02` тақырыбы, селектор тегі `0x01` плюс дайджест `b1 8f e9 c1 ab ba c4 5b 3e 38 fc 5d`, одан кейін бір кілтті пайдалы жүктеме (`0x00` тегі, `0x01` қисық идентификаторы идентификаторы000000285, ұзындығы I002NI82 Ed25519 кілті).Бірлік сынақтары (`account::address::tests::parse_any_accepts_all_formats`) төмендегі V1 векторларын `AccountAddress::parse_any` арқылы бекітеді, бұл құралдың он алтылық, IH58 (қалаулы) және қысылған (`sora`, екінші ең жақсы) пішіндердегі канондық пайдалы жүктемеге сенетініне кепілдік береді. `cargo run -p iroha_data_model --example address_vectors` көмегімен ұзартылған арматура жинағын қайта жасаңыз.

| домен | Тұқым байты | Канондық алтылық | Сығылған (`sora`) |
|-------------|-----------|-----------------------------------------------------------------------------------------|------------|
| әдепкі | `0x00` | `0x020001203b6a27bcceb6a42d62a3a8d02a6f0d73653215771de243a63ac048a18b59da29` | `sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE` |
| қазынашылық | `0x01` | `0x0201b18fe9c1abbac45b3e38fc5d0001208a88e3dd7409f195fd52db2d3cba5d72ca6709bf1d94121bf3748801b40f6f5c` | `sora5ｻu6rﾀCヰTGwﾏ1ﾅヱﾌQｲﾖﾇqCｦヰﾓZQCZRDSSﾅMｱﾙヱｹﾁｸ8ｾeﾄﾛ6C8bZuwﾗｹCZｦRSLQFU` |
| ғажайыптар әлемі | `0x02` | `0x0201b8ae571b79c5a80f5834da2b0001208139770ea87d175f56a35466c34c7ecccb8d8a91b4ee37a25df60f5b8fc9b394` | `sora5ｻwﾓyRｿqﾏnMﾀﾙヰKoﾒﾇﾓQｺﾛyｼ3ｸFHB2F5LyPﾐTMZkｹｼw67ﾋVﾕｻr8ﾉGﾇeEnｻVRNKCS` |
| ироха | `0x03` | `0x0201de8b36819700c807083608e2000120ed4928c628d1c2c6eae90338905995612959273a5c63f93636c14614ac8737d1` | `sora5ｻﾜxﾀ7Vｱ7QFeｷMﾂLﾉﾃﾏﾓﾀTﾚgSav3Wnｱｵ4ｱCKｷﾛMﾘzヰHiﾐｱ6ﾃﾉﾁﾐZmﾇ2fiﾎX21P4L` |
| альфа | `0x04` | `0x020146be2154ae86826a3fef0ec0000120ca93ac1705187071d67b83c7ff0efe8108e8ec4530575d7726879333dbdabe7c` | `sora5ｻ9JヱﾈｿuwU6ｴpﾔﾂﾈRqRTds1HﾃﾐｶLVﾍｳ9ﾔhｾNｵVｷyucEﾒGﾈﾏﾍ9sKeﾉDzrｷﾆ742WG1` |
| омега | `0x05` | `0x0201390d946885bc8416b3d30c9d0001206e7a1cdd29b0b78fd13af4c5598feff4ef2a97166e3ca6f2e4fbfccd80505bf1` | `sora5ｻ3zrﾌuﾚﾄJﾑXQhｸTyN8pzwRkWxmjVﾗbﾚﾕヰﾈoｽｦｶtEEﾊﾐ6GPｿﾓﾊｾEhvPｾｻ3XAJ73F` |
| басқару | `0x06` | `0x0201989eb45a80940d187e2c908f0001208a875fff1eb38451577acd5afee405456568dd7c89e090863a0557bc7af49f17` | `sora5ｻiｵﾁyVﾕｽbFpDHHuﾇﾉdﾗｲﾓﾄRﾋAW3frUCｾ5ｷﾘTwdﾚnｽtQiLﾏｼｶﾅXgｾZmﾒヱH58H4KP` |
| валидаторлар | `0x07` | `0x0201e4ffa58704c69afaeb7cc2d7000120ea4a6c63e29c520abef5507b132ec5f9954776aebebe7b92421eea691446d22c` | `sora5ｻﾀLDH6VYﾑNAｾgﾉVﾜtxﾊRXLｹﾍﾔﾌLd93GﾔGeｴﾄYrs1ﾂHｸkYxｹwｿyZﾗxyﾎZoXT1S4N` |
| зерттеуші | `0x08` | `0x02013b35422c65c2a83c99c523ad0001201398f62c6d1a457c51ba6a4b5f3dbd2f69fca93216218dc8997e416bd17d93ca` | `sora5ｻ4nmｻaﾚﾚPvNLgｿｱv6MHDeEyﾀovﾉJcpvrﾖ6ﾈCQcCNﾇﾜhﾚﾖyFdTwｸｶHEｱ9rWU8FMB` |
| соранет | `0x09` | `0x0201047d9ea7f5d5dbec3f7bfc58000120fd1724385aa0c75b64fb78cd602fa1d991fdebf76b13c58ed702eac835e9f618` | `sora5ｱｸヱVQﾂcﾁヱRﾓcApｲﾁﾅﾒvﾌﾏfｾNnﾛRJsｿDhﾙuHaﾚｺｦﾌﾍﾈeﾆﾎｺN1UUDｶ6ﾎﾄﾛoRH8JUL` |
| Kitsune | `0x0A` | `0x0201e91933de397fd7723dc9a76c00012043a72e714401762df66b68c26dfbdf2682aaec9f2474eca4613e424a0fbafd3c` | `sora5ｻﾚｺヱkfFJfSﾁｼJwﾉLvbpSｷﾔMWFMrbｳｸｲｲyヰKGJﾉｻ4ｹﾕrｽhｺｽzSDヰXAN62AD7RGNS` |
| да | `0x0B` | `0x02016838cf5bb0ce0f3d4f380e1c00012066be7e332c7a453332bd9d0a7f7db055f5c5ef1a06ada66d98b39fb6810c473a` | `sora5ｻNﾒ5SﾐRﾉﾐﾃ62ｿ1ｶｷWFKyF1BcAﾔvｼﾐHqﾙﾐPﾏｴヰ5tｲﾕvnﾙT6ﾀW7mﾔ7ﾇﾗﾂｳ25CXS93` |

Қарап шыққан: деректер үлгісі WG, криптография WG — ADDR-1a үшін бекітілген аумақ.

##### Sora Nexus сілтеме бүркеншік аттары

Sora Nexus желілерінің әдепкі мәні `chain_discriminant = 0x02F1`
(`iroha_config::parameters::defaults::common::CHAIN_DISCRIMINANT`). The
Сондықтан `AccountAddress::to_ih58` және `to_compressed_sora` көмекшілері сәуле шығарады
әрбір канондық пайдалы жүктеме үшін дәйекті мәтіндік пішіндер. Таңдалған арматура
`fixtures/account/address_vectors.json` (арқылы жасалған
`cargo xtask address-vectors`) жылдам анықтама үшін төменде көрсетілген:

| Тіркелгі / селектор | IH58 литералы (`0x02F1` префиксі) | Сора қысылған (`sora`) әріптік |
|--------------------------------|--------------------------------|-------------------------|
| `default` домені (жасырын селектор, `0x00` тұқымы) | `6cmzPVPX5jDQFNfiz6KgmVfm1fhoAqjPhoPFn4nx9mBWaFMyUCwq4cw` | `sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE` (айқын бағыттау туралы кеңестер берген кезде қосымша `@default` жұрнағы) |
| `treasury` (жергілікті дайджест селекторы, тұқым `0x01`) | `34mSYnCXkCzHXm31UDHh7SJfGvC4QPEhwim8z7sys2iHqXpCwCQkjL8KHvkFLSs1vZdJcb37r` | `sora5ｻu6rﾀCヰTGwﾏ1ﾅヱﾌQｲﾖﾇqCｦヰﾓZQCZRDSSﾅMｱﾙヱｹﾁｸ8ｾeﾄﾛ6C8bZuwﾗｹCZｦRSLQFU` |
| Ғаламдық тізілім көрсеткіші (`registry_id = 0x0000_002A`, `treasury` баламасы) | `3oE9sLeRGP49Cu7mQ1nF4wtKAm29BG4TGLiRsaXe7mhbMP5WZ113nNW1N6RbqF` | `sorakXｹ6NｻﾍﾀﾖSﾜﾖｱ3ﾚ5WﾘﾋQﾅｷｦxgﾛｸcﾁｵﾋkﾋvﾏ8SPﾓﾀｹdｴｴｲW9iCM6AEP` |

Бұл жолдар CLI (`iroha tools address convert`), Torii шығаратын жолдарға сәйкес келеді
жауаптар (`address_format=ih58|compressed`) және SDK көмекшілері, сондықтан UX көшіру/қою
ағындар оларға сөзбе-сөз сене алады. `<address>@<domain>`-ті нақты бағыттау туралы кеңес қажет болғанда ғана қосыңыз; жұрнақ канондық шығыстың бөлігі емес.

#### 2.6 Өзара жұмыс істеуге арналған мәтіндік бүркеншік аттар (жоспарланған)

- **Тізбекті бүркеншік ат стилі:** журналдар мен адамға арналған `ih:<chain-alias>:<alias@domain>`
  кіру. Әмияндар префиксті талдауы, енгізілген тізбекті тексеруі және блоктауы керек
  сәйкессіздіктер.
- **CAIP-10 нысаны:** тізбекті-агностикалық үшін `iroha:<caip-2-id>:<ih58-addr>`
  интеграциялар. Бұл карта жөнелтілгенде **әлі орындалмаған**
  құралдар тізбегі.
- **Машиналық көмекшілер:** Rust, TypeScript/JavaScript, Python үшін кодектерді жариялау,
  және IH58 және қысылған пішімдерді қамтитын Котлин (`AccountAddress::to_ih58`,
  `AccountAddress::parse_any` және олардың SDK баламалары). CAIP-10 көмекшілері болып табылады
  болашақ жұмыс.

#### 2.7 Детерминистік IH58 бүркеншік аты

- **Префиксті салыстыру:** `chain_discriminant` IH58 желі префиксі ретінде қайта пайдаланыңыз.
  `encode_ih58_prefix()` (`crates/iroha_data_model/src/account/address.rs` қараңыз)
  `<64` мәндері үшін 6 биттік префиксті (бір байт) және 14 биттік, екі байтты шығарады
  үлкен желілерге арналған пішін. Беделді тапсырмалар өмір сүреді
  [`address_prefix_registry.md`](source/references/address_prefix_registry.md);
  SDK соқтығысуларды болдырмау үшін сәйкес JSON тізбесін синхрондауы КЕРЕК.
- **Есептік жазба материалы:** IH58 құрастырған канондық пайдалы жүктемені кодтайды
  `AccountAddress::canonical_bytes()` — тақырып байты, домен селекторы және
  контроллердің пайдалы жүктемесі. Қосымша хэштеу қадамы жоқ; IH58 ендіреді
  Rust шығарған екілік контроллердің пайдалы жүктемесі (бір кілт немесе мультисиг).
  мультисиг саясат дайджесттері үшін пайдаланылатын CTAP2 картасы емес, кодтаушы.
- **Кодтау:** `encode_ih58()` префикс байттарын канондық байтпен біріктіреді
  пайдалы жүктеме және тіркелген параметрмен Blake2b-512 алынған 16 биттік бақылау сомасын қосады
  префиксі `IH58PRE` (`b"IH58PRE" || prefix || payload`). Нәтиже `bs58` арқылы Base58-кодталған.
  CLI/SDK көмекшілері бірдей процедураны көрсетеді және `AccountAddress::parse_any`
  оны `decode_ih58` арқылы кері қайтарады.

#### 2.8 Нормативтік мәтіндік тест векторлары

`fixtures/account/address_vectors.json` құрамында толық IH58 (қалаулы) және қысылған (`sora`, екінші ең жақсы)
әрбір канондық пайдалы жүктеме үшін литералдар. Ерекшеліктер:

- **`addr-single-default-ed25519` (Sora Nexus, `0x02F1` префиксі).**  
  IH58 `6cmzPVPX5jDQFNfiz6KgmVfm1fhoAqjPhoPFn4nx9mBWaFMyUCwq4cw`, қысылған (`sora`)
  `sora2QG…U4N5E5`. Torii осы дәл жолдарды `AccountId` арқылы шығарады
  `Display` іске асыру (канондық IH58) және `AccountAddress::to_compressed_sora`.
- **`addr-global-registry-002a` (тізілім селекторы → қазынашылық).**  
  IH58 `3oE9sLeRGP49Cu7mQ1nF4wtKAm29BG4TGLiRsaXe7mhbMP5WZ113nNW1N6RbqF`, қысылған (`sora`)
  `sorakX…CM6AEP`. Тіркеу селекторлары әлі де кодты шешетінін көрсетеді
  сәйкес жергілікті дайджест сияқты бірдей канондық пайдалы жүктеме.
- **Сәтсіздік жағдайы (`ih58-prefix-mismatch`).**  
  Түйінде `NETWORK_PREFIX + 1` префиксімен кодталған IH58 литералы талдау
  әдепкі префикс кірістерін күту
  `AccountAddressError::UnexpectedNetworkPrefix { expected: 753, found: 754 }`
  доменді бағыттау әрекетінен бұрын. `ih58-checksum-mismatch` арматурасы
  Blake2b бақылау сомасы бойынша бұрмалауды анықтау жаттығулары.

#### 2.9 Сәйкестік құрылғылары

ADDR‑2 оң және теріс мәндерді қамтитын қайта ойнатылатын арматура жинағын жеткізеді
канондық он алтылық, IH58 (қалаулы), қысылған (`sora`, жарты/толық ен), жасырын сценарийлер
әдепкі селекторлар, жаһандық тізілім бүркеншік аттары және көп қолтаңба контроллерлері. The
канондық JSON `fixtures/account/address_vectors.json` ішінде тұрады және болуы мүмкін
қалпына келтірілді:

```
cargo xtask address-vectors --out fixtures/account/address_vectors.json
# verify without writing:
cargo xtask address-vectors --verify
```

Арнайы эксперименттер үшін (әртүрлі жолдар/пішімдер) екілік мысал әлі де болады
қолжетімді:

```
cargo run -p iroha_data_model --example account_address_vectors > fixtures/account/address_vectors.json
```

`crates/iroha_data_model/tests/account_address_vectors.rs` жүйесінде тот блоктарының сынақтары
және `crates/iroha_torii/tests/account_address_vectors.rs`, JS бірге,
Swift және Android құрылғылары (`javascript/iroha_js/test/address.test.js`,
`IrohaSwift/Tests/IrohaSwiftTests/AccountAddressTests.swift`,
`java/iroha_android/src/test/java/org/hyperledger/iroha/android/address/AccountAddressTests.java`),
SDK және Torii кіруі арасындағы кодектер паритетіне кепілдік беру үшін бірдей құрылғыны қолданыңыз.

### 3. Ғаламдық бірегей домендер және қалыпқа келтіру

Сондай-ақ қараңыз: [`docs/source/references/address_norm_v1.md`](source/references/address_norm_v1.md)
Torii, деректер үлгісі және SDKs арқылы қолданылатын канондық норма v1 құбыры үшін.

`DomainId` тегтелген кортеж ретінде қайта анықтаңыз:

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

`LocalChain` ағымдағы тізбек басқаратын домендер үшін бар атауды орап алады.
Домен жаһандық тізілім арқылы тіркелгенде, біз иелік етуді сақтаймыз
тізбектің дискриминанты. Дисплей/талдау әзірше өзгеріссіз қалады, бірақ
кеңейтілген құрылым маршруттау шешімдерін қабылдауға мүмкіндік береді.

#### 3.1 Нормализация және спуфингке қарсы қорғаныс

Norm v1 әрбір компонент домен алдында пайдалануы тиіс канондық құбырды анықтайды
аты сақталады немесе `AccountAddress` ішіне ендірілген. Толық шолу
[`docs/source/references/address_norm_v1.md`](source/references/address_norm_v1.md) ішінде тұрады;
Төмендегі қорытынды әмияндар, Torii, SDK және басқару қадамдарын қамтиды.
құралдарын жүзеге асыру керек.

1. **Енгізуді тексеру.** Бос жолдарды, бос орындарды және резервте қалғандарды қабылдамау
   бөлгіштер `@`, `#`, `$`. Бұл орындаған инварианттарға сәйкес келеді
   `Name::validate_str`.
2. **Unicode NFC композициясы.** ICU қолдайтын NFC қалыпқа келтіруді канондық түрде қолданыңыз
   эквивалентті тізбектер детерминирленген түрде құлайды (мысалы, `e\u{0301}` → `é`).
3. **UTS-46 қалыпқа келтіру.** NFC шығысын UTS‑46 арқылы іске қосыңыз.
   `use_std3_ascii_rules = true`, `transitional_processing = false`, және
   DNS ұзындығының күшіне енуі қосылды. Нәтиже – кіші әріпті A-белгі тізбегі;
   STD3 ережелерін бұзатын кірістер бұл жерде сәтсіздікке ұшырайды.
4. **Ұзындық шектеулері.** DNS стилінің шекараларын күшейтіңіз: әрбір белгі 1–63 болуы керек.
   3-қадамнан кейін байт және толық домен 255 байттан аспауы керек.
5. **Қосымша шатастырылатын саясат.** UTS‑39 сценарийін тексерулер үшін бақыланады
   Норма v2; операторлар оларды ерте қоса алады, бірақ тексеруден өтпеген жағдайда тоқтату керек
   өңдеу.

Әрбір кезең сәтті болса, кіші әріпті A-жапсырма жолы кэштеледі және үшін пайдаланылады
мекенжайды кодтау, конфигурациялау, манифесттер және тізілімді іздеу. Жергілікті дайджест
селекторлар 12 байт мәнін `blake2s_mac(кілт = "SORA-LOCAL-K:v1",
канондық_белгі)[0..12]`  3-қадам шығысы арқылы. Барлық басқа әрекеттер (аралас
регистр, бас әріп, шикі Юникод енгізуі) құрылымдықпен қабылданбайды
`ParseError`s атау берілген шекарада.

Осы ережелерді көрсететін канондық қондырғылар, соның ішінде пуникодтық сапарлар
және жарамсыз STD3 реттілігі — тізімде берілген
`docs/source/references/address_norm_v1.md` және SDK CI ішінде бейнеленген
ADDR‑2 астында бақыланатын векторлық жиынтықтар.

### 4. Nexus домен тізілімі және маршруттау- **Тіркеу схемасы:** Nexus қол қойылған `DomainName -> ChainRecord` картасын сақтайды
  мұндағы `ChainRecord` тізбекті дискриминантты, қосымша метадеректерді (RPC) қамтиды
  соңғы нүктелер) және өкілеттіктің дәлелі (мысалы, басқарудың көп қолтаңбасы).
- **Синхрондау механизмі:**
  - Тізбектер қол қойылған домен шағымдарын Nexus (генезис кезінде немесе арқылы) жібереді
    басқару жөніндегі нұсқаулық).
  - Nexus мерзімді манифесттерді жариялайды (қол қойылған JSON және қосымша Merkle түбірі)
    HTTPS және мазмұн мекенжайы бар жад (мысалы, IPFS) арқылы. Клиенттер белгішесін бекітеді
    соңғы манифест және қолтаңбаларды тексеру.
- **Іздеу ағыны:**
  - Torii `DomainId` сілтемесі бар транзакцияны алады.
  - Домен жергілікті түрде белгісіз болса, Torii кэштелген Nexus манифестіне сұрау салады.
  - Манифест шетелдік тізбекті көрсетсе, транзакция қабылданбайды
    детерминирленген `ForeignDomain` қатесі және қашықтағы тізбек ақпараты.
  - Nexus доменінде жоқ болса, Torii `UnknownDomain` қайтарады.
- **Сенім анкерлері және айналу:** Басқару кілттерінің манифесттері; айналу немесе
  күшін жою жаңа манифест жазбасы ретінде жарияланады. Клиенттер манифестті жүзеге асырады
  TTLs (мысалы, 24 сағ) және сол терезеден кейінгі ескірген деректермен кеңесуден бас тартыңыз.
- **Сәтсіздік режимдері:** Манифестті шығарып алу сәтсіз болса, Torii кэштелген күйге оралады.
  TTL ішіндегі деректер; өткен TTL ол `RegistryUnavailable` шығарады және бас тартады
  сәйкес келмейтін күйді болдырмау үшін доменаралық бағыттау.

### 4.1 Тізілімнің өзгермейтіндігі, бүркеншік аттары және құлпытастары (ADDR-7c)

Nexus **тек қосу үшін манифест** жариялайды, осылайша әрбір домен немесе бүркеншік ат тағайындалады
тексеруге және қайталауға болады. Операторлар бөлімде сипатталған топтаманы өңдеуі керек
[адрес манифест runbook](source/runbooks/address_manifest_ops.md) ретінде
ақиқаттың жалғыз көзі: егер манифест жоқ болса немесе валидациядан өтпесе, Torii
зардап шеккен доменді шешуден бас тарту.

Автоматтандыруды қолдау: `cargo xtask address-manifest verify --bundle <current_dir> --previous <previous_dir>`
бөлімінде жазылған бақылау сомасын, схеманы және алдыңғы дайджест тексерулерін қайталайды
runbook. `sequence` көрсету үшін өзгерту билеттеріне пәрмен шығысын қосыңыз
және `previous_digest` байланысы жинақты жарияламас бұрын тексерілді.

#### Манифест тақырыбы және қол қою келісімшарты

| Өріс | Талап |
|-------|-------------|
| `version` | Қазіргі уақытта `1`. Сәйкес спецификация жаңартуымен ғана соққы беріңіз. |
| `sequence` | Әр жарияланымға **дәл** бір көбейту. Torii кэштері бос орындар немесе регрессиялар бар түзетулерден бас тартады. |
| `generated_ms` + `ttl_hours` | Кэштің жаңалығын орнату (әдепкі 24 сағат). Егер TTL келесі жарияланымға дейін аяқталса, Torii `RegistryUnavailable` түріне ауысады. |
| `previous_digest` | Алдыңғы манифест дененің BLAKE3 дайджесті (он алтылығы). Тексерушілер өзгермейтіндігін дәлелдеу үшін оны `b3sum` көмегімен қайта есептейді. |
| `signatures` | Манифесттерге Sigstore (`cosign sign-blob`) арқылы қол қойылады. Операциялар `cosign verify-blob --bundle manifest.sigstore manifest.json` іске қосуы және шығару алдында басқару идентификациясын/эмитент шектеулерін орындауы керек. |

Шығарылымды автоматтандыру `manifest.sigstore` және `checksums.sha256` шығарады
JSON денесінің жанында. SoraFS немесе көшіру кезінде файлдарды бірге сақтаңыз
Аудиторлар тексеру қадамдарын сөзбе-сөз қайталай алатындай HTTP соңғы нүктелері.

#### Жазба түрлері

| |түрі Мақсаты | Міндетті өрістер |
|------|---------|-----------------|
| `global_domain` | Домен жаһандық деңгейде тіркелгенін және тізбек дискриминантына және IH58 префиксіне сәйкес келуі керек екенін мәлімдейді. | `{ "domain": "<label>", "chain": "sora:nexus:global", "ih58_prefix": 753, "selector": "global" }` |
| `tombstone` | Бүркеншік атты/таңдаушыны біржола өшіреді. Local‑8 дайджесттерін өшіру немесе доменді жою кезінде қажет. | `{ "selector": {…}, "reason_code": "LOCAL8_RETIREMENT" \| …, "ticket": "<governance id>", "replaces_sequence": <number> }` |

`global_domain` жазбалары міндетті түрде `manifest_url` немесе `sorafs_cid` қамтуы мүмкін
әмияндарды қол қойылған тізбек метадеректеріне бағыттаңыз, бірақ канондық кортеж сақталады
`{domain, chain, discriminant/ih58_prefix}`. `tombstone` жазбалары **міндетті** сілтеме жасау керек
зейнеткерлікке шыққан селектор және рұқсат берген билет/басқару артефакті
Аудит жолын желіден тыс қайта құруға болатындай өзгерту.

#### Бүркеншік ат/құлпытас жұмыс процесі және телеметрия

1. **Дрейфті анықтаңыз.** `torii_address_local8_total{endpoint}` пайдаланыңыз,
   `torii_address_local8_domain_total{endpoint,domain}`,
   `torii_address_collision_total{endpoint,kind="local12_digest"}`,
   `torii_address_collision_domain_total{endpoint,domain}`,
   `torii_address_domain_total{endpoint,domain_kind}`, және
   `torii_address_invalid_total{endpoint,reason}` (көрсетілген
   `dashboards/grafana/address_ingest.json`) жергілікті жіберулерді растау және
   Жергілікті-12 соқтығысуы құлпытас ұсынбас бұрын нөлде қалады. The
   әр домен есептегіштері иелеріне тек әзірлеуші/сынақ домендері Local‑8 шығаратынын дәлелдеуге мүмкіндік береді
   трафик (және сол Local‑12 соқтығысу белгілі кезеңдік домендерге салыстыру) кезінде
   **Домен түрінің қоспасы (5м)** панелін қамтиды, осылайша SRE қанша болатынын сыза алады
   `domain_kind="local12"` трафик қалады, ал `AddressLocal12Traffic`
   Өндіріс жергілікті-12 селекторларын көрсе де, ескерту іске қосылады
   зейнеткерлік қақпа.
2. **Канондық дайджесттерді шығарыңыз.** Жүгіру
   `iroha tools address convert <address> --format json --expect-prefix 753`
   (немесе `fixtures/account/address_vectors.json` арқылы тұтыныңыз
   `scripts/account_fixture_helper.py`) дәл `digest_hex` түсіру үшін.
   CLI IH58, `sora…` және канондық `0x…` литералдарын қабылдайды; қосу
   `@<domain>` тек манифесттер үшін белгіні сақтау қажет болғанда.
   JSON жиыны сол доменді `input_domain` өрісі арқылы көрсетеді және
   `legacy  suffix` түрлендірілген кодтауды `<address>@<domain>` ретінде қайталайды.
   манифест айырмашылықтары (бұл жұрнақ канондық тіркелгі идентификаторы емес, метадеректер болып табылады).
   Жаңа жолға бағытталған экспорттар үшін пайдаланыңыз
   `iroha tools address normalize --input <file> legacy-selector input mode` Жергілікті жаппай түрлендіру үшін
   селекторларды өткізіп жіберу кезінде канондық IH58 (қалаулы), қысылған (`sora`, екінші ең жақсы), он алтылық немесе JSON пішіндеріне
   жергілікті емес жолдар. Аудиторларға электрондық кестеге сәйкес келетін дәлелдер қажет болғанда, іске қосыңыз
   CSV қорытындысын шығару үшін `iroha tools address audit --input <file> --format csv`
   (`input,status,format,domain_kind,…`) жергілікті селекторларды бөлектейді,
   канондық кодтаулар және бір файлдағы сәтсіздіктерді талдау.
3. **Манифест жазбаларын қосыңыз.** `tombstone` жазбасының жобасын (және кейінгі
   жаһандық тізілімге көшу кезінде `global_domain` жазбасы) және растаңыз
   қол қоюды сұрамас бұрын манифест `cargo xtask address-vectors`.
4. **Тексеру және жариялау.** Runbook тексеру тізімін орындаңыз (хэштер, Sigstore,
   бірізділік монотондылығы) топтаманы SoraFS түріне көшірмес бұрын. Torii қазір
   IH58 (таңдаулы)/sora (екінші ең жақсы) литералдарды топтамадан кейін бірден канонизациялайды.
5. **Бақылау және кері қайтару.** Local‑8 және Local‑12 соқтығысу панелдерін мына жерде ұстаңыз.
   30 күн бойы нөл; регрессиялар пайда болса, алдыңғы манифестті қайта жариялаңыз
   телеметрия тұрақтанғанға дейін тек зардап шеккен өндірістік емес ортада.

Жоғарыдағы қадамдардың барлығы ADDR‑7c үшін міндетті дәлел: онсыз көрсетіледі
`cosign` қолтаңба жинағы немесе сәйкес `previous_digest` мәндері жоқ
автоматты түрде қабылданбайды және операторлар тексеру журналдарын тіркеуі керек
олардың билеттерін ауыстыру.

### 5. Әмиян және API эргономикасы

- **Дисплейдің әдепкі параметрлері:** Әмияндар IH58 мекенжайын көрсетеді (қысқа, бақылау сомасы)
  плюс шешілген домен тізілімнен алынған белгі ретінде. Домендер
  өзгеруі мүмкін сипаттаушы метадеректер ретінде анық белгіленген, ал IH58 - бұл
  тұрақты мекенжай.
- **Кірісті канонизациялау:** Torii және SDKs IH58 (қалаулы)/sora (екінші ең жақсы)/0x қабылдайды
  мекенжайлар плюс `alias@domain`, `public_key@domain`, `uaid:…` және
  `opaque:…` пішімдері, одан кейін шығару үшін IH58 стандартына сәйкестендіріледі. жоқ
  қатаң режимді ауыстыру; өңделмеген телефон/электрондық пошта идентификаторлары кітаптан тыс сақталуы керек
  UAID/мөлдір емес салыстырулар арқылы.
- **Қатенің алдын алу:** Әмияндар IH58 префикстерін талдайды және тізбекті дискриминанттарды қолданады
  күту. Тізбек сәйкессіздіктері әрекет ететін диагностикамен ауыр ақауларды тудырады.
- **Кодек кітапханалары:** Ресми Rust, TypeScript/JavaScript, Python және Kotlin
  кітапханалар IH58 кодтау/декодтау плюс қысылған (`sora`) қолдауын қамтамасыз етеді.
  бөлшектелген іске асырудан аулақ болыңыз. CAIP-10 түрлендірулері әлі жеткізілмеген.

#### Қол жетімділік және қауіпсіз бөлісу бойынша нұсқаулық- Өнім беттерін енгізу бойынша нұсқаулық тікелей қадағаланады
  `docs/portal/docs/reference/address-safety.md`; сол бақылау тізіміне сілтеме жасағанда
  бұл талаптарды әмиянға немесе Explorer UX-ге бейімдеу.
- **Қауіпсіз ортақ пайдалану ағындары:** Мекенжайларды IH58 пішініне әдепкі етіп көшіретін немесе көрсететін беттер және пайдаланушылар бақылау сомасын визуалды немесе сканерлеу арқылы тексере алатындай толық жолды да, бірдей пайдалы жүктемеден алынған QR кодын да ұсынатын көршілес «бөлісу» әрекетін көрсетеді. Қиып кетудің алдын алу мүмкін болмаған кезде (мысалы, кішкентай экрандар), жолдың басы мен соңын сақтаңыз, анық эллипстерді қосыңыз және кездейсоқ қиып алудың алдын алу үшін толық мекенжайды алмасу буферіне көшіру арқылы қол жетімді етіп қойыңыз.
- **IME қауіпсіздік шаралары:** Мекенжай кірістері IME/IME стиліндегі пернетақталардағы композиция артефактілерінен бас тартуы КЕРЕК. Тек ASCII енгізуін орындау, толық ені немесе Kana таңбалары анықталған кезде кірістірілген ескертуді ұсыныңыз және жапондық және қытайлық пайдаланушылар өздерінің IME деректерін прогресті жоғалтпай өшіре алатындай етіп тексеру алдында белгілерді біріктіретін қарапайым мәтінді қою аймағын ұсыныңыз.
- **Экранды оқуға қолдау көрсету:** Басты Base58 префикс сандарын сипаттайтын және IH58 пайдалы жүктемесін 4 немесе 8 таңбалы топтарға бөлетін көрнекі жасырын белгілерді (`aria-label`/I18NI0000475X) қамтамасыз етіңіз, осылайша топтық таңбалар жолының орнына іске қосылған көмекші технологиялар оқиды. Сыпайы тірі аймақтар арқылы көшіру/бөлісу сәттілігін жариялаңыз және QR алдын ала қарауында сипаттаушы балама мәтінді («0x02F1 тізбегіндегі <бүркеншік ат үшін IH58 мекенжайы») қамтитынына көз жеткізіңіз.
- **Тек Sora үшін қысылған пайдалану:** Әрқашан `sora…` қысылған көріністі «Тек Sora» деп белгілеңіз және көшірмес бұрын оны нақты растаудың артына қойыңыз. Тізбек дискриминанты Sora Nexus мәні болмаса, SDK және әмияндар қысылған нәтижені көрсетуден бас тартуы керек және қаражатты бұрмалауды болдырмау үшін пайдаланушыларды желіаралық аударымдар үшін IH58 бағытына қайта бағыттауы керек.

## Іске асыруды бақылау тізімі

- **IH58 конверт:** Префикс `chain_discriminant` ықшам файлын пайдаланып кодтайды.
  `encode_ih58_prefix()` бастап 6-/14-биттік схема, денесі канондық байттар болып табылады
  (`AccountAddress::canonical_bytes()`) және бақылау сомасы алғашқы екі байт
  Blake2b-512(`b"IH58PRE"` || префиксі || негізгі). Толық пайдалы жүктеме Base58-
  `bs58` арқылы кодталған.
- **Тіркеу келісімшарты:** Қол қойылған JSON (және қосымша Merkle түбірі) жариялау
  `{discriminant, ih58_prefix, chain_alias, endpoints}` 24 сағ TTL және
  айналдыру пернелері.
- **Домен саясаты:** ASCII `Name` бүгін; i18n қосылса, үшін UTS-46 қолданыңыз
  қалыпқа келтіру және шатастыратын тексерулер үшін UTS-39. Максималды белгіні (63) және
  жалпы (255) ұзындық.
- **Мәтіндік көмекшілер:** Rust ішіндегі IH58 ↔ қысылған (`sora…`) кодектерін жіберіңіз,
  Ортақ сынақ векторлары бар TypeScript/JavaScript, Python және Kotlin (CAIP-10
  салыстыру болашақ жұмыс болып қалады).
- **CLI құралдары:** `iroha tools address convert` арқылы детерминирленген оператор жұмыс процесін қамтамасыз етіңіз
  (`crates/iroha_cli/src/address.rs` қараңыз), ол IH58/`sora…`/`0x…` литералдарын және
  қосымша `<address>@<domain>` жапсырмалары, әдепкі бойынша Sora Nexus префиксі (`753`) арқылы IH58 шығысы,
  және операторлар оны нақты сұраған кезде ғана Sora-тек қысылған алфавитті шығарады
  `--format compressed` немесе JSON жиынтық режимі. Пәрмен префикс күтулерін іске қосады
  талдау, берілген доменді (JSON ішіндегі `input_domain`) және `legacy  suffix` жалауын жазады
  түрлендірілген кодтауды `<address>@<domain>` ретінде қайталайды, сондықтан манифест айырмашылықтары эргономикалық болып қалады.
- **Wallet/explorer UX:** [мекенжайды көрсету нұсқауларын] (source/sns/address_display_guidelines.md) орындаңыз
  ADDR-6 арқылы жеткізіледі — қос көшірме түймелерін ұсыныңыз, IH58 QR пайдалы жүктемесі ретінде сақтаңыз және ескертіңіз
  пайдаланушылар қысылған `sora…` пішіні тек Sora және IME қайта жазуларына сезімтал.
- **Torii интеграциясы:** Nexus кэші TTL-ге қатысты көрінеді, шығарылады
  `ForeignDomain`/`UnknownDomain`/`RegistryUnavailable` анықтаушы және
  keep account-literal parsing encoded-only (`IH58` preferred, `sora…`
  compressed accepted) with canonical IH58 output.

### Torii жауап пішімдері

- `GET /v1/accounts` қосымша `address_format` сұрау параметрін қабылдайды және
  `POST /v1/accounts/query` JSON конвертінің ішіндегі бірдей өрісті қабылдайды.
  Қолдау көрсетілетін мәндер:
  - `ih58` (әдепкі) — жауаптар канондық IH58 Base58 пайдалы жүктемелерін шығарады (мысалы,
    `6cmzPVPX5jDQFNfiz6KgmVfm1fhoAqjPhoPFn4nx9mBWaFMyUCwq4cw`).
  - `compressed` — жауаптар тек Sora-ға арналған `sora…` қысылған көріністі шығарады.
    сүзгілерді/жол параметрлерін канондық сақтау.
- Жарамсыз мәндер қайтарады `400` (`QueryExecutionFail::Conversion`). Бұл мүмкіндік береді
  әмияндар мен зерттеушілер тек Sora-ға арналған UX үшін қысылған жолдарды сұрау үшін
  IH58 әрекеттесу әдепкі ретінде сақталады.
- Актив ұстаушылардың тізімдері (`GET /v1/assets/{definition_id}/holders`) және олардың JSON
  конверт әріптесі (`POST …/holders/query`) сонымен қатар `address_format` құрметіне ие.
  `items[*].account_id` өрісі кез келген уақытта қысылған литералдарды шығарады
  параметр/конверт өрісі тіркелгілерді көрсететін `compressed` күйіне орнатылды
  зерттеушілер каталогтар бойынша дәйекті нәтиже көрсете алатындай соңғы нүктелер.
- **Тестілеу:** Кодер/декодер айналмалы сапарлары, қате тізбек үшін бірлік сынақтарын қосыңыз
  сәтсіздіктер және манифестті іздеулер; Torii және SDKs интеграциялық қамтуды қосыңыз
  IH58 ағындары үшін басынан аяғына дейін.

## Қате коды тізілімі

Адрестік кодтаушылар мен декодерлер ақауларды көрсетеді
`AccountAddressError::code_str()`. Келесі кестелер тұрақты кодтарды береді
SDK, әмияндар және Torii беттері адам оқи алатын беттермен қатар тұруы керек
хабарлар, сонымен қатар ұсынылған түзету нұсқаулары.

### Канондық құрылыс

| Код | Сәтсіздік | Ұсынылатын түзету |
|------|---------|-------------------------|
| `ERR_UNSUPPORTED_ALGORITHM` | Кодер тізбе немесе құрастыру мүмкіндіктері қолдамайтын қол қою алгоритмін алды. | Тіркеу мен конфигурацияда қосылған қисық сызықтармен тіркелгі құрылысын шектеңіз. |
| `ERR_KEY_PAYLOAD_TOO_LONG` | Қол қою кілтінің пайдалы жүктеме ұзындығы қолдау көрсетілетін шектен асып кетті. | Бір кілтті контроллерлер `u8` ұзындықтарымен шектелген; үлкен ашық кілттер үшін multisig пайдаланыңыз (мысалы, ML‑DSA). |
| `ERR_INVALID_HEADER_VERSION` | Мекенжай тақырыбының нұсқасы қолдау көрсетілетін ауқымнан тыс. | V1 мекенжайлары үшін `0` тақырып нұсқасын шығару; жаңа нұсқаларды қабылдамас бұрын кодерлерді жаңартыңыз. |
| `ERR_INVALID_NORM_VERSION` | Нормализация нұсқасының жалауы танылмайды. | `1` қалыпқа келтіру нұсқасын пайдаланыңыз және сақталған биттерді ауыстырып-қоспаңыз. |
| `ERR_INVALID_IH58_PREFIX` | Сұралған IH58 желі префиксін кодтау мүмкін емес. | Тізбек тізілімінде жарияланған `0..=16383` инклюзивті ауқымынан префиксті таңдаңыз. |
| `ERR_CANONICAL_HASH_FAILURE` | Канондық пайдалы жүкті хэштеу сәтсіз аяқталды. | Операцияны қайталап көріңіз; егер қате сақталса, оны хэштеу стекіндегі ішкі қате ретінде қарастырыңыз. |

### Пішімді декодтау және автоматты анықтау

| Код | Сәтсіздік | Ұсынылатын түзету |
|------|---------|-------------------------|
| `ERR_INVALID_IH58_ENCODING` | IH58 жолында алфавиттен тыс таңбалар бар. | Мекенжайдың жарияланған IH58 алфавитін пайдаланатынына және көшіру/қою кезінде кесілмегеніне көз жеткізіңіз. |
| `ERR_INVALID_LENGTH` | Пайдалы жүк ұзындығы селектор/контроллер үшін күтілетін канондық өлшемге сәйкес келмейді. | Таңдалған домен селекторы мен контроллердің орналасуы үшін толық канондық пайдалы жүктемені қамтамасыз етіңіз. |
| `ERR_CHECKSUM_MISMATCH` | IH58 (таңдаулы) немесе қысылған (`sora`, екінші ең жақсы) бақылау сомасын тексеру сәтсіз аяқталды. | Сенімді көзден мекенжайды қайта жасаңыз; бұл әдетте көшіру/қою қатесін көрсетеді. |
| `ERR_INVALID_IH58_PREFIX_ENCODING` | IH58 префикс байттары дұрыс емес. | Мекенжайды үйлесімді кодтауышпен қайта кодтаңыз; жетекші Base58 байтты қолмен өзгертпеңіз. |
| `ERR_INVALID_HEX_ADDRESS` | Канондық он алтылық пішінді декодтау мүмкін болмады. | Ресми кодтаушы шығарған `0x` префиксті, жұп ұзындықтағы алты қырлы жолды қамтамасыз етіңіз. |
| `ERR_MISSING_COMPRESSED_SENTINEL` | Қысылған пішін `sora` деп басталмайды. | Сығылған Sora мекенжайларын декодерлерге бермес бұрын қажетті күзетшімен префикс қойыңыз. |
| `ERR_COMPRESSED_TOO_SHORT` | Сығылған жолда пайдалы жүктеме және бақылау сомасы үшін жеткілікті сандар жоқ. | Кесілген үзінділердің орнына кодер шығаратын толық қысылған жолды пайдаланыңыз. |
| `ERR_INVALID_COMPRESSED_CHAR` | Сығылған алфавиттен тыс таңба кездесті. | Таңбаны жарияланған жарты ені/толық ен кестелеріндегі жарамды Base‑105 глифімен ауыстырыңыз. |
| `ERR_INVALID_COMPRESSED_BASE` | Кодер қолдау көрсетілмейтін радиксті пайдалануға әрекет жасады. | Кодерге қарсы қате жіберіңіз; қысылған алфавит V1-де 105-радикске бекітілген. |
| `ERR_INVALID_COMPRESSED_DIGIT` | Сандық мән қысылған алфавит өлшемінен асып кетті. | Әр санның `0..105)` шегінде екеніне көз жеткізіңіз, қажет болса мекенжайды қайта жасаңыз. |
| `ERR_UNSUPPORTED_ADDRESS_FORMAT` | Автоматты анықтау енгізу пішімін тани алмады. | Талдауыштарды шақырған кезде IH58 (қалаулы), қысылған (`sora`) немесе канондық `0x` он алтылық жолдарын қамтамасыз етіңіз. |

### Домен мен желіні тексеру| Код | Сәтсіздік | Ұсынылатын түзету |
|------|---------|-------------------------|
| `ERR_DOMAIN_MISMATCH` | Домен селекторы күтілетін доменге сәйкес келмейді. | Белгіленген домен үшін берілген мекенжайды пайдаланыңыз немесе күтуді жаңартыңыз. |
| `ERR_INVALID_DOMAIN_LABEL` | Домен белгісін қалыпқа келтіруді тексеру сәтсіз аяқталды. | Кодтау алдында UTS-46 өтпелі емес өңдеу арқылы доменді канонизациялау. |
| `ERR_UNEXPECTED_NETWORK_PREFIX` | Декодталған IH58 желі префиксі конфигурацияланған мәннен ерекшеленеді. | Мақсатты тізбектен мекенжайға ауысыңыз немесе күтілетін дискриминант/префиксті реттеңіз. |
| `ERR_UNKNOWN_ADDRESS_CLASS` | Мекенжай класының биттері танылмайды. | Декодерді жаңа сыныпты түсінетін шығарылымға жаңартыңыз немесе тақырып биттерін өзгертуге жол бермеңіз. |
| `ERR_UNKNOWN_DOMAIN_TAG` | Домен таңдау тегі белгісіз. | Жаңа селектор түрін қолдайтын шығарылымға жаңартыңыз немесе V1 түйіндерінде эксперименттік пайдалы жүктемелерді пайдаланбаңыз. |
| `ERR_UNEXPECTED_EXTENSION_FLAG` | Сақталған кеңейтім биті орнатылды. | Сақталған биттерді тазалау; олар болашақ ABI оларды таныстырмайынша жабық қалады. |
| `ERR_UNKNOWN_CONTROLLER_TAG` | Контроллердің пайдалы жүктеме тегі танылмады. | Жаңа контроллер түрлерін талдаудан бұрын тану үшін декодерді жаңартыңыз. |
| `ERR_UNEXPECTED_TRAILING_BYTES` | Канондық пайдалы жүктемеде декодтаудан кейін кейінгі байттар бар. | Канондық пайдалы жүктемені қалпына келтіріңіз; тек құжатталған ұзындық болуы керек. |

### Контроллердің пайдалы жүктемесін тексеру

| Код | Сәтсіздік | Ұсынылатын түзету |
|------|---------|-------------------------|
| `ERR_INVALID_PUBLIC_KEY` | Кілт байттары жарияланған қисыққа сәйкес келмейді. | Кілт байттары таңдалған қисық үшін талап етілетіндей кодталғанына көз жеткізіңіз (мысалы, 32 байт Ed25519). |
| `ERR_UNKNOWN_CURVE` | Қисық идентификатор тіркелмеген. | Қосымша қисықтар мақұлданып, тізілімде жарияланғанша `1` (Ed25519) қисық идентификаторын пайдаланыңыз. |
| `ERR_MULTISIG_MEMBER_OVERFLOW` | Multisig контроллері қолдау көрсетілетіннен көбірек мүшелерді жариялайды. | Кодтау алдында мультисиг мүшелігін құжатталған шекке дейін азайтыңыз. |
| `ERR_INVALID_MULTISIG_POLICY` | Multisig саясатының пайдалы жүктемесін тексеру сәтсіз аяқталды (шекті мән/салмақ/схема). | Саясатты CTAP2 схемасын, салмақ шектерін және шекті шектеулерді қанағаттандыратындай етіп қайта жасаңыз. |

## Баламалар қарастырылды

- **Pure Base58Check (Bitcoin-стиль).** Қарапайым бақылау сомасы, бірақ қатені анықтау әлсіз.
  Blake2b-дан алынған IH58 бақылау сомасына қарағанда (`encode_ih58` 512 биттік хэшті қысқартады)
  және 16-биттік дискриминанттар үшін айқын префикс семантикасы жоқ.
- **Домен жолына тізбек атауын енгізу (мысалы, `finance@chain`).** Үзілістер
- **Мекенжайды өзгертпестен тек Nexus маршрутизациясына сеніңіз.** Пайдаланушылар әлі де
  анық емес жолдарды көшіру/қою; мекенжайдың өзі контекстті тасымалдауды қалаймыз.
- **Bech32m конверт.** QR-қолайлы және адам оқи алатын префиксті ұсынады, бірақ
  IH58 жеткізілімінен (`AccountAddress::to_ih58`) алшақтайды
  және барлық құрылғыларды/SDK қайта жасауды талап етеді. Ағымдағы жол картасы IH58 + сақтайды
  болашақта зерттеуді жалғастыра отырып, қысылған (`sora`) қолдау
  Bech32m/QR қабаттары (CAIP-10 салыстыру кейінге қалдырылған).

## Ашық сұрақтар

- `u16` дискриминанттары плюс резервтелген ауқымдар ұзақ мерзімді сұранысты жабатынын растаңыз;
  әйтпесе `u32` нұсқасын варинт кодтауымен бағалаңыз.
- Тізілім жаңартулары үшін көп қолтаңбаны басқару процесін аяқтаңыз және қалай
  қайтарып алу/мерзімі өткен бөлулер өңделеді.
- Нақты манифест қолтаңбасының схемасын анықтаңыз (мысалы, Ed25519 мульти-сиг) және
  Nexus тарату үшін көлік қауіпсіздігі (HTTPS бекіту, IPFS хэш пішімі).
- Тасымалдау үшін домен бүркеншік аттарын/қайта бағыттауларды қолдау керек пе және қалай екенін анықтаңыз
  детерминизмді бұзбай олардың бетін ашу.
- Kotodama/IVM келісім-шарттарының IH58 көмекшілеріне қалай кіруін көрсетіңіз (`to_address()`,
  `parse_address()`) және тізбекті сақтау CAIP-10-ны көрсетуі керек пе
  салыстырулар (бүгін IH58 канондық болып табылады).
- Iroha тізбектерін сыртқы тізілімдерде тіркеуді зерттеңіз (мысалы, IH58 тізілімі,
  CAIP аттар кеңістігі каталогы) кеңірек экожүйені туралау үшін.

## Келесі қадамдар

1. IH58 кодтауы `iroha_data_model` (`AccountAddress::to_ih58`,
   `parse_any`); құрылғыларды/сынақтарды әр SDK-ға тасымалдауды жалғастырыңыз және кез келгенін тазалаңыз
   Bech32m толтырғыштары.
2. `chain_discriminant` көмегімен конфигурация схемасын кеңейтіп, мәнді шығарыңыз
  бұрыннан бар сынақ/әзірлеу параметрлері үшін әдепкі мәндер. **(Орындалды: `common.chain_discriminant`
  қазір `iroha_config` нұсқасында жеткізіледі, әдепкі бойынша әр желімен `0x02F1`
  қайта анықтайды.)**
3. Nexus тізілім схемасының және тұжырымдаманың дәлелі манифест жариялаушысының жобасын жасаңыз.
4. Әмиян провайдерлері мен кастодиандардан адам факторы аспектілері бойынша кері байланыс жинаңыз
   (HRP атауы, дисплей пішімі).
5. Құжаттаманы (`docs/source/data_model.md`, Torii API құжаттары) жаңартыңыз.
   іске асыру жолы бекітілді.
6. Нормативтік сынағы бар ресми кодектер кітапханаларын (Rust/TS/Python/Kotlin) жіберіңіз.
   сәттілік пен сәтсіздік жағдайларын қамтитын векторлар.
