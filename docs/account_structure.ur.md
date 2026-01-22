---
lang: ur
direction: rtl
source: docs/account_structure.md
status: complete
translator: manual
source_hash: 7e6a1321c6f8d71ac4b576a55146767fbc488b29c7e21d82bc2e1c55db89769c
source_last_modified: "2025-11-12T00:36:40.117854+00:00"
translation_last_reviewed: 2025-11-14
---

<div dir="rtl">

<!-- docs/account_structure.md (Account Structure RFC) کا اردو ترجمہ -->

# اکاؤنٹ اسٹرکچر RFC

**حالت:** منظور شدہ (ADDR‑1)  
**سامعین:** ڈیٹا ماڈل، Torii، Nexus، والیٹ اور گورننس ٹیمیں  
**متعلقہ issues:** بعد میں متعین ہوں گی

## خلاصہ

یہ دستاویز اکاؤنٹ ایڈریسنگ اسکیم کی جامع نظرِ ثانی تجویز کرتی ہے تاکہ:

- ایک **Iroha Bech32m (IH‑B32)** ایڈریس فراہم کیا جا سکے جو انسان کیلئے
  قابلِ مطالعہ ہو اور checksum کے ساتھ chain discriminant کو اکاؤنٹ کے
  signatory سے باندھے، اور deterministic، interop‑friendly textual forms
  مہیا کرے۔
- globally unique domain identifiers متعارف کرائے جائیں، جو ایسے رجسٹری
  سے مدد یافتہ ہوں جسے Nexus کے ذریعے cross‑chain routing کیلئے query کیا
  جا سکے۔
  aliases کو برقرار رکھیں، جبکہ ہم wallets، APIs اور contracts کو نئے
  فارمیٹ میں migrate کرتے ہیں۔

## محرک (Motivation)

آج کل wallets اور off‑chain tooling خالص `alias@domain` routing aliases پر
انحصار کرتے ہیں۔ اس کے دو بڑے نقصانات ہیں:

1. **Network binding کی کمی۔** اس string میں نہ checksum ہوتا ہے نہ chain
   prefix؛ اس لیے user غلط نیٹ ورک کا address paste کر سکتا ہے، اور اسے
   فوراً feedback نہیں ملتا۔ نتیجتاً transaction بالآخر reject ہو سکتی ہے
   (`chain_id` mismatch)، یا بد ترین صورت میں کسی غیر ارادی اکاؤنٹ کے خلاف
   apply ہو سکتی ہے اگر وہ destination مقامی chain پر موجود ہو۔
2. **Domain collision۔** domains صرف namespace کے طور پر کام کرتے ہیں اور
   ہر chain پر reuse ہو سکتے ہیں۔ services کی federation (custodians،
   bridges، cross‑chain workflows) کمزور پڑ جاتی ہے، کیونکہ chain A پر
   `finance` کا chain B کے `finance` سے کوئی تعلق نہیں۔

ہمیں ایک human‑friendly address format درکار ہے جو copy/paste errors کے خلاف
حفاظت فراہم کرے، اور domain name سے authoritative chain تک ایک deterministic
mapping بھی دے۔

## مقاصد (Goals)

- Iroha accounts کے لیے IH58‑inspired Bech32m address envelope design کرنا،
  جبکہ canonical CAIP‑10 textual aliases بھی شائع کیے جائیں۔
- configured chain discriminant کو ہر address میں براہِ راست encode کرنا،
  اور اس کیلئے governance/registry process متعین کرنا۔
- یہ بیان کرنا کہ global domain registry کو موجودہ deployments کو توڑے
  بغیر کیسے introduce کیا جا سکتا ہے، اور normalization/anti‑spoofing
  rules کیا ہوں گے۔
  دستاویزی شکل دینا۔

## غیر مقاصد (Non‑goals)

- cross‑chain asset transfers implement کرنا؛ routing layer صرف target chain
  واپس کرتی ہے۔
- `AccountId` کی اندرونی ساخت تبدیل کرنا (اب بھی `DomainId + PublicKey`)۔
- global domain issuance کیلئے مکمل governance فریم ورک فائنل کرنا؛ یہ RFC
  بنیادی طور پر data model اور transport primitives پر فوکس کرتا ہے۔

## پس منظر (Background)

### موجودہ routing alias

```text
AccountId {
    domain: DomainId,   // Name (ASCII‑جیسے string) کے اوپر wrapper
    signatory: PublicKey // multihash string (مثال ed0120...)
}

Display / Parse: "<signatory multihash>@<domain name>"

اب اس text form کو **account alias** سمجھا جاتا ہے: routing میں سہولت دینے
والا ایک شارٹ ہینڈ، جو canonical
[`AccountAddress`](#2-canonical-address-codecs) کی طرف اشارہ کرتا ہے۔ یہ
اب بھی انسانی پڑھنے اور domain‑scoped governance کیلئے مفید ہے، لیکن
آن‑چین authoritative account identifier نہیں رہا۔
```

`ChainId`, `AccountId` کے باہر موجود ہوتا ہے۔ نوڈز admission کے دوران
transaction کے `ChainId` کو configuration کے ساتھ compare کرتے ہیں
(`AcceptTransactionFail::ChainIdMismatch`) اور foreign transactions کو reject
کر دیتے ہیں، تاہم account string خود کسی network hint کو نہیں اٹھاتی۔

### ڈومین identifiers

`DomainId` ایک normalized string `Name` کو wrap کرتا ہے، اور local chain تک
محدود رہتا ہے۔ ہر chain الگ طور پر `wonderland`, `finance` وغیرہ register
کر سکتی ہے۔

### Nexus context

Nexus cross‑component coordination (lanes/data‑spaces) کا ذمہ دار ہے۔ فی الحال
اس کے پاس cross‑chain domain routing کا کوئی تصوّر نہیں۔

## مجوزہ ڈیزائن (Proposed Design)

### 1. Deterministic chain discriminant

`iroha_config::parameters::actual::Common` struct کو یوں expand کیا جاتا ہے:

```rust
pub struct Common {
    pub chain: ChainId,
    pub chain_discriminant: u16, // نیا فیلڈ، globally coordinated
    // ... existing fields
}
```

- **Constraints:**
  - ہر active نیٹ ورک کیلئے unique؛ signed public registry کے ذریعے manage
    کیا جاتا ہے، جس میں explicit reserved ranges ہوں (مثلاً
    `0x0000–0x0FFF` test/dev، `0x1000–0x7FFF` community allocations،
    `0x8000–0xFFEF` governance‑approved، `0xFFF0–0xFFFF` reserved)۔
  - running chain کیلئے immutable؛ اسے تبدیل کرنا hard fork اور registry
    update کا تقاضا کرتا ہے۔
- **Governance & registry:** multi‑signature governance set ایک signed JSON
  (اور IPFS‑pinned) registry maintain کرتا ہے، جو discriminants کو human
  aliases اور CAIP‑2 identifiers سے map کرتا ہے۔ clients registry fetch اور
  cache کر کے chain metadata validate اور display کرتے ہیں۔
- **Usage:** chain discriminant کو state admission، Torii، SDKs اور wallet
  APIs کے ذریعے تہہ بہ تہہ propagate کیا جاتا ہے، تاکہ ہر component اسے
  embed یا validate کر سکے۔ اسے CAIP‑2 کا حصہ بنا کر expose کیا جاتا ہے
  (مثلاً `iroha:0x1234`)۔

### 2. Canonical address codecs

Rust data model ایک واحد canonical binary representation `AccountAddress`
expose کرتا ہے، جسے متعدد human‑facing formats میں ظاہر کیا جا سکتا ہے:
IH58 شیئرنگ اور canonical output کیلئے preferred فارمیٹ ہے؛ compressed (`snx1`)
فارم Sora‑only اور second‑best آپشن ہے۔

- **IH58 (Iroha Base58):** Base58 envelope جس میں chain discriminant embed
  ہوتا ہے؛ decoders prefix validate کرنے کے بعد payload کو canonical form
  میں promote کرتے ہیں۔
- **Sora‑compressed view:** Sora کیلئے مخصوص **105 symbols** پر مشتمل
  alphabet، جو half‑width イロハ poem (جس میں ヰ اور ヱ بھی شامل ہیں) کو IH58
  کے 58‑character set کے ساتھ append کر کے بنتا ہے۔ strings sentinel
  `snx1` سے شروع ہوتی ہیں، Bech32m‑derived checksum رکھتی ہیں، اور network
  prefix omit کرتی ہیں (Sora Nexus sentinel سے imply ہوتا ہے)۔

  ```text
  IH58  : 123456789ABCDEFGHJKMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz
  Iroha : ｲﾛﾊﾆﾎﾍﾄﾁﾘﾇﾙｦﾜｶﾖﾀﾚｿﾂﾈﾅﾗﾑｳヰﾉｵｸﾔﾏｹﾌｺｴﾃｱｻｷﾕﾒﾐｼヱﾋﾓｾｽ
  ```
- **Canonical hex:** debugging‑friendly `0x…` encoding جو canonical byte
  envelope کو represent کرتا ہے۔

`AccountAddress::parse_any` compressed، IH58 یا canonical hex input
auto‑detect کر کے decoded payload اور detected `AccountAddressFormat` دونوں
واپس کرتی ہے۔ Torii ISO 20022 supplementary addresses کیلئے `parse_any` call
کرتا ہے اور canonical hex form store کرتا ہے، تاکہ metadata original
representation سے قطع نظر deterministic رہے۔

### 2.1 ہیڈر بائٹ کی ساخت (ADDR‑1a)

ہر canonical payload کو تین حصّوں میں ترتیب دیا جاتا ہے:
`header · domain selector · controller`.  
`header` ایک بائٹ ہوتا ہے جو بتاتا ہے کہ اس کے بعد آنے والی بائٹس پر کون
سے parser rules لاگو ہوں گے:

```text
bit index:   7        5 4      3 2      1 0
             ┌─────────┬────────┬────────┬────┐
payload bit: │version  │ class  │  norm  │ext │
             └─────────┴────────┴────────┴────┘
```

پہلا بائٹ اس طرح مستقبل کے decoders کیلئے schema metadata کو pack کرتا ہے:

| Bits | فیلڈ | Allowed values | Error on violation |
|------|------|----------------|--------------------|
| 7‑5  | `addr_version` | `0` (v1). `1‑7` مستقبل کیلئے reserved ہیں۔ | `0‑7` کی حد سے باہر قدر آنے پر `AccountAddressError::InvalidHeaderVersion` اٹھایا جاتا ہے؛ implementations کو آج کے لیے non‑zero ورژنز کو unsupported سمجھنا چاہیے۔ |
| 4‑3  | `addr_class` | `0` = single key، `1` = multisig. | کوئی اور value آئے تو `AccountAddressError::UnknownAddressClass` اٹھایا جاتا ہے۔ |
| 2‑1  | `norm_version` | `1` (Norm v1). `0`, `2`, `3` reserved ہیں۔ | `0‑3` کے علاوہ قدر ہو تو `AccountAddressError::InvalidNormVersion` اٹھتا ہے۔ |
| 0    | `ext_flag` | لازمی طور پر `0` ہو۔ | اگر یہ bit سیٹ ہو تو `AccountAddressError::UnexpectedExtensionFlag` اٹھایا جاتا ہے۔ |

Rust encoder ہمیشہ `0x02` لکھتا ہے (version `0`, class = single‑key،
norm version `1`, اور extension flag کلئیر)۔

### 2.2 ڈومین سلیکٹر کی encoding (ADDR‑1a)

ہیڈر کے فوراً بعد domain selector آتا ہے، جو ایک tagged union ہے:

| Tag | مطلب | Payload | نوٹس |
|-----|------|---------|------|
| `0x00` | implicit default domain | کوئی payload نہیں | configured `default_domain_name()` سے میچ کرتا ہے۔ |
| `0x01` | local domain digest | 12 بائٹس | Digest = `blake2s_mac(key = "SORA-LOCAL-K:v1", canonical_label)[0..12]`. |
| `0x02` | global registry entry | 4 بائٹس | big‑endian `registry_id`; global registry live ہونے تک reserved۔ |

domain labels کو hash سے پہلے canonicalise کیا جاتا ہے
(`UTS‑46 + STD3 + NFC`). نامعلوم Tag پر `AccountAddressError::UnknownDomainTag`
اٹھایا جاتا ہے۔ جب address کو کسی domain کے خلاف validate کیا جائے اور
selector متضاد ہو تو `AccountAddressError::DomainMismatch` آتا ہے۔

```text
domain selector
┌──────────┬──────────────────────────────────────────────┐
│ tag (u8) │ payload (depends on selector kind, see table)│
└──────────┴──────────────────────────────────────────────┘
```

selector فوراً controller payload کے ساتھ لگا ہوتا ہے، لہٰذا decoder wire
format کو ترتیب سے چل سکتا ہے: پہلے tag بائٹ، پھر tag‑specific payload، پھر
controller کی بائٹس۔

**Selector کی مثالیں**

- *Implicit default* (`tag = 0x00`): کوئی payload نہیں۔ default domain کے لیے
  deterministic test key کے ساتھ canonical hex مثال:  
  `0x0200000120641297079357229f295938a4b5a333de35069bf47b9d0704e45805713d13c201`.
- *Local digest* (`tag = 0x01`): payload 12‑بائٹ digest ہے۔ مثال
  (`treasury`, seed `0x01`):  
  `0x0201b18fe9c1abbac45b3e38fc5d0001203b77a042f1de02f6d5f418f36a20fd68c8329fe3bbfbecd26a2d72878cd827f8`.
- *Global registry* (`tag = 0x02`): payload ایک big‑endian
  `registry_id:u32` ہے۔ payload کے بعد آنے والی بائٹس implicit‑default
  کیس جیسی ہی ہوتی ہیں؛ selector صرف normalised domain string کو registry
  pointer سے replace کرتا ہے۔ مثال:
  `registry_id = 0x0000_002A` (decimal 42) اور deterministic default
  controller کے ساتھ:  
  `0x02020000002a000120641297079357229f295938a4b5a333de35069bf47b9d0704e45805713d13c201`.  
  breakdown: header `0x02`, selector tag `0x02`, registry id `00 00 00 2A`,
  controller tag `0x00`, curve id `0x01`, key length `0x20` اور 32‑بائٹ
  Ed25519 key payload۔

### 2.3 Controller payload کی encoding (ADDR‑1a)

controller payload بھی ایک tagged union ہے جو domain selector کے بعد لگتا
ہے:

| Tag | Controller | Layout | نوٹس |
|-----|-----------|--------|------|
| `0x00` | Single key | `curve_id:u8` · `key_len:u8` · `key_bytes` | `curve_id = 0x01` آج Ed25519 کے مساوی ہے۔ `key_len` کی حد `u8` ہے؛ اس سے بڑی قدر پر `AccountAddressError::KeyPayloadTooLong` اٹھایا جاتا ہے۔ |
| `0x01` | Multisig | `version:u8` · `threshold:u16` · `member_count:u8` · (`curve_id:u8` · `weight:u16` · `key_len:u16` · `key_bytes`)\* | زیادہ سے زیادہ 255 اراکین (`CONTROLLER_MULTISIG_MEMBER_MAX`) سپورٹ ہوتے ہیں۔ unknown curves پر `AccountAddressError::UnknownCurve` اور malformed policies پر `AccountAddressError::InvalidMultisigPolicy` اٹھتے ہیں۔ |

multisig policies ایک CTAP2‑style CBOR map اور canonical digest بھی expose
کرتی ہیں، تاکہ hosts اور SDKs controller کو deterministically verify کر
سکیں۔ schema، hashing procedure اور golden fixtures کیلئے
`docs/source/references/address_norm_v1.md` (ADDR‑1c) دیکھیں۔

تمام key bytes بالکل اسی طرح encode ہوتی ہیں جیسے `PublicKey::to_bytes`
واپس کرتا ہے؛ decoders انہی bytes سے `PublicKey` instance تعمیر کرتے ہیں،
اور اگر bytes بیان کردہ curve سے میچ نہ کریں تو
`AccountAddressError::InvalidPublicKey` اٹھاتے ہیں۔

> **Ed25519 canonical enforcement (ADDR‑3a):** curve `0x01` کی keys کو عین
> اسی byte string میں decode ہونا چاہیے جو signer نے خارج کی، اور انہیں
> small‑order subgroup میں نہیں ہونا چاہیے۔ نوڈز non‑canonical encodings
> (مثلاً values mod `2^255‑19`) اور کمزور points (identity element وغیرہ)
> کو reject کرتے ہیں، اس لیے SDKs کو بھی addresses submit کرنے سے پہلے
> یہی validation errors surface کرنے چاہییں۔

#### 2.3.1 Curve identifier registry (ADDR‑1d)

| ID (`curve_id`) | Algorithm | Feature gate | نوٹس |
|-----------------|-----------|--------------|------|
| `0x00` | Reserved | — | emit نہیں ہونا چاہیے؛ decoders `ERR_UNKNOWN_CURVE` surface کرتے ہیں۔ |
| `0x01` | Ed25519 | ہمیشہ فعال | canonical v1 algorithm (`Algorithm::Ed25519`); آج production builds میں یہی واحد id فعال ہے۔ |
| `0x02` | ML‑DSA (preview) | `ml-dsa` | ADDR‑3 rollout کیلئے reserved؛ ML‑DSA signing path آنے تک default طور پر disable۔ |
| `0x03` | GOST placeholder | `gost` | provisioning/negotiation کیلئے reserved؛ اس id والے controller payloads کو concrete TC26 profile کی منظوری سے پہلے reject کرنا ضروری ہے۔ |
| `0x0A` | GOST R 34.10‑2012 (256, set A) | `gost` | reserved؛ `gost` crypto feature کے ساتھ unlock ہوگا۔ |
| `0x0B` | GOST R 34.10‑2012 (256, set B) | `gost` | مستقبل کی governance approval کیلئے reserved۔ |
| `0x0C` | GOST R 34.10‑2012 (256, set C) | `gost` | مستقبل کی governance approval کیلئے reserved۔ |
| `0x0D` | GOST R 34.10‑2012 (512, set A) | `gost` | مستقبل کی governance approval کیلئے reserved۔ |
| `0x0E` | GOST R 34.10‑2012 (512, set B) | `gost` | مستقبل کی governance approval کیلئے reserved۔ |
| `0x0F` | SM2 | `sm` | reserved؛ SM crypto feature stable ہونے پر فعال ہوگا۔ |

slots `0x04–0x09` آئندہ curves کیلئے خالی ہیں؛ نیا الگورتھم شامل کرنے کے
لیے roadmap اپ ڈیٹ اور SDK/host میں corresponding سپورٹ درکار ہے۔ encoders
کسی بھی unsupported الگورتھم پر `ERR_UNSUPPORTED_ALGORITHM` اٹھائیں، اور
decoders نامعلوم ids پر `ERR_UNKNOWN_CURVE` کے ساتھ فوراً fail‑closed ہوں۔

canonical رجسٹری (جس میں machine‑readable JSON export بھی شامل ہے) یہاں
موجود ہے:
[`docs/source/references/address_curve_registry.md`](source/references/address_curve_registry.md)۔  
tooling کو چاہیے کہ اسی dataset کو consume کرے تاکہ curve identifiers
SDKs اور operator workflows میں مستقل (consistent) رہیں۔

- **SDK gating:** SDKs ڈفالٹ طور پر صرف Ed25519 کو سپورٹ کرتے ہیں۔ Swift
  compile‑time flags (`IROHASWIFT_ENABLE_MLDSA`,
  `IROHASWIFT_ENABLE_GOST`, `IROHASWIFT_ENABLE_SM`) expose کرتا ہے؛ Java SDK
  میں `AccountAddress.configureCurveSupport(...)` اور JavaScript SDK میں
  `configureCurveSupport({ allowMlDsa: true, allowGost: true, allowSm2: true })`
  استعمال ہوتی ہے، تاکہ integrators اضافی curves استعمال کرنے سے پہلے
  واضح طور پر opt‑in کریں۔
- **Host gating:** `Register<Account>` ایسے controllers reject کرتا ہے جن
  کے signatories الگورتھمز استعمال کریں جو node کے
  `crypto.allowed_signing` میں موجود نہیں، یا جن کے curve identifiers
  `crypto.curves.allowed_curve_ids` سے غائب ہوں؛ لہٰذا clusters کو ML‑DSA /
  GOST / SM controllers allow کرنے سے پہلے config + genesis کے ذریعے سپورٹ
  declare کرنی ہوگی۔

### 2.4 فیلئر رولز (ADDR‑1a)

- اگر payload مطلوبہ header + selector سے چھوٹی ہو، یا آخر میں extra bytes
  ہوں، تو `AccountAddressError::InvalidLength` یا
  `AccountAddressError::UnexpectedTrailingBytes` اٹھتے ہیں۔
- وہ headers جو reserved `ext_flag` سیٹ کریں یا غیر سپورٹڈ
  versions/classes advertise کریں، لازمی طور پر `UnexpectedExtensionFlag`
  یا `InvalidHeaderVersion` یا `UnknownAddressClass` کے ساتھ reject ہوں۔
- unknown selector/controller tags پر
  `UnknownDomainTag` یا `UnknownControllerTag` اٹھایا جاتا ہے۔
- oversized یا خراب key material پر
  `KeyPayloadTooLong` یا `InvalidPublicKey` اٹھتے ہیں۔
- 255 سے زیادہ اراکین والے multisig controllers
  `MultisigMemberOverflow` پیدا کرتے ہیں۔
- IME/NFKC conversions: half‑width Sora kana کو full‑width میں normalise
  کیا جا سکتا ہے بغیر اس کے کہ decoding ٹوٹے، لیکن ASCII sentinel `snx1`
  اور IH58 digits/letters کو لازماً ASCII ہی رہنا چاہیے۔ full‑width یا
  case‑folded sentinels پر `ERR_MISSING_COMPRESSED_SENTINEL`، full‑width
  ASCII payload پر `ERR_INVALID_COMPRESSED_CHAR` اور checksum mismatch پر
  `ERR_CHECKSUM_MISMATCH` bubble‑up ہوتا ہے۔  
  `crates/iroha_data_model/src/account/address.rs` کی property tests یہ
  راستے exercise کرتی ہیں تاکہ SDKs اور wallets deterministic failures پر
  اعتماد کر سکیں۔
- Torii اور SDK میں `address@domain` aliases parse کرتے وقت اگر IH58 یا
  compressed input alias fallback سے پہلے fail ہو جائے (مثلاً checksum
  mismatch یا domain digest mismatch)، تو وہی `ERR_*` codes surface ہوتے ہیں،
  تاکہ clients کو free‑form strings کے بجائے structured reasons ملیں۔

#### 2.5 معیاری بائنری ویکٹرز

- **Implicit default domain** (`default`, seed byte `0x00`)  
  canonical hex:  
  `0x0200000120641297079357229f295938a4b5a333de35069bf47b9d0704e45805713d13c201`.  
  breakdown: header `0x02`, selector `0x00` (implicit default)، controller
  tag `0x00`, curve id `0x01` (Ed25519)، key length `0x20` اور اس کے بعد
  32‑بائٹ key payload۔
- **Local domain digest** (`treasury`, seed byte `0x01`)  
  canonical hex:  
  `0x0201b18fe9c1abbac45b3e38fc5d0001203b77a042f1de02f6d5f418f36a20fd68c8329fe3bbfbecd26a2d72878cd827f8`.  
  breakdown: header `0x02`, selector tag `0x01` کے ساتھ digest
  `b1 8f e9 c1 ab ba c4 5b 3e 38 fc 5d`، پھر single‑key payload
  (tag `0x00`, curve id `0x01`, length `0x20`, 32‑بائٹ Ed25519 key)۔

unit tests (`account::address::tests::parse_any_accepts_all_formats`) V1
ویکٹرز کو `AccountAddress::parse_any` کے ذریعے assert کرتے ہیں، جس سے
tooling canonical payload پر hex، IH58 اور compressed (`snx1`) تمام فارمیٹس میں
اعتماد کر سکتی ہے۔  
extended fixture set کو دوبارہ generate کرنے کیلئے:

```bash
cargo run -p iroha_data_model --example address_vectors
```

| Domain      | Seed byte | Canonical hex                                                                 | Compressed (`snx1`) |
|-------------|-----------|-------------------------------------------------------------------------------|------------|
| default     | `0x00`    | `0x0200000120641297079357229f295938a4b5a333de35069bf47b9d0704e45805713d13c201` | `snx12QGﾈkﾀｱﾚiﾉﾘuﾛWRヱﾏxﾁSuﾁepnhｽvｶrﾓｶ9Tｹｿp3ﾇVWｳｲｾU4N5E5` |
| treasury    | `0x01`    | `0x0201b18fe9c1abbac45b3e38fc5d0001203b77a042f1de02f6d5f418f36a20fd68c8329fe3bbfbecd26a2d72878cd827f8` | `snx15ｻu6rﾀCヰTGwﾏ1ﾅヱﾌQｲﾖﾘｻYﾃhﾓMQ9CBEﾅﾊﾈｷﾉVRｺnKRwTﾋｼqﾅWrﾎU7ｼiﾍQt1TPGNJ` |
| wonderland  | `0x02`    | `0x0201b8ae571b79c5a80f5834da2b000120ad29ac2c12d4daaa4a2415235f2b01730bff1193dd4a6eaee29e945b01a4a212` | `snx15ｻwﾓyRｿqﾏnMﾀﾙヰKoﾒﾇﾓRKSｷﾗﾕneﾀM3Rabvﾂ1JﾚﾉｺｲﾕｹｺﾀFﾔﾇﾖFSXsﾜCHmB59S5KS` |
| iroha       | `0x03`    | `0x0201de8b36819700c807083608e2000120ce6d4f240893505e112cdc1b83585d8efc271ea6f934c5f6a49217e27e61b9e7` | `snx15ｻﾜxﾀ7Vｱ7QFeｷMﾂLﾉﾃﾏﾓsYヰxﾎﾍﾚﾇｺﾊehjyzXGヰaｿSﾔ1kWｺｾJeﾒAWkwﾋﾐRRQQKXYE` |
| alpha       | `0x04`    | `0x020146be2154ae86826a3fef0ec000012077143459c5b54808313cd57ded18322fc02c4616de930e0e3af578bb509bb5dc` | `snx15ｻ9JヱﾈｿuwU6ｴpﾔﾂﾈRqRSﾗgPﾏrHﾔGﾀqﾂｹfoﾂHwﾉoﾊ4ﾎﾇ74ｼﾕﾎUw8JaU3ﾙJFYHVLUS` |
| omega       | `0x05`    | `0x0201390d946885bc8416b3d30c9d000120e18cbb31e5249ff9205b72fe50e50dcc78fb80e28028bdc4c47bcf63ee61c6b8` | `snx15ｻ3zrﾌuﾚﾄJﾑXQhｸTyN8qfBﾌﾒaTjQpTxPﾊｦNﾚnヱvorHｷﾎkﾈEヱFﾎｻTUﾗhiVqURKRVM` |
| governance  | `0x06`    | `0x0201989eb45a80940d187e2c908f0001208a5bd65d39ba61bde2a87ee10d242bd5575cd02bf687c4b5960d4141698dd76a` | `snx15ｻiｵﾁyVﾕｽbFpDHHuﾇﾉdﾗｲﾌbﾜｸeGｵzﾙzｽﾐﾌﾉQﾃw2ﾕLDﾔｽﾙFﾙﾇﾋBTdUXﾎﾙsｽRDJCHS` |
| validators  | `0x07`    | `0x0201e4ffa58704c69afaeb7cc2d7000120f0f80d8a09aa1276d2e605bc904137f7a52b9c4847b9b5366d4002ca4049daeb` | `snx15ｻﾀLDH6VYﾑNAｾgﾉVﾜtxﾊWTﾂfKｳmU7fWﾍXｱ2JnyﾋE4ﾕghZｱVｶｦﾇaFﾕbr8qﾑR4VEFR` |
| explorer    | `0x08`    | `0x02013b35422c65c2a83c99c523ad00012033af4073c5815cbe5d0fec37cffe02e542302b60e24d8a7c0819f772ca6886f9` | `snx15ｻ4nmｻaﾚﾚPvNLgｿｱv6MHdPﾓWﾍヱpeﾕFﾕmFﾌﾀKhﾉWｴeﾋbｷXMﾎ2ﾃnQﾐﾗﾑﾎBBｻC8P548` |
| soranet     | `0x09`    | `0x0201047d9ea7f5d5dbec3f7bfc58000120cd3c119f6c81e28a2747c531f5cbe8dbc44ed8e16751bc4a467445b272db4097` | `snx15ｱｸヱVQﾂcﾁヱRﾓcApｲﾁﾅﾒvwS8JｳEnQaｿHTdﾒXZﾍvﾆazｿgﾔﾙhF9hcsﾘvNﾌヱJ9MGDNBW` |
| kitsune     | `0x0A`    | `0x0201e91933de397fd7723dc9a76c0001206e4c4188e1b8455ff3369dc163240a5d653f13a6f420fd0edbb23303bad239e7` | `snx15ｻﾚｺヱkfFJfSﾁｼJwﾉLvbpzﾘKmC6ｱSﾇhqｦJB1gﾙwCwﾁﾍeﾉﾔABﾆpqYﾍEﾌｼﾆﾃFNCAT97` |
| da          | `0x0B`    | `0x02016838cf5bb0ce0f3d4f380e1c00012056f02721c153689b09efafd07d8ef7bed2c4a581dd00faa118aed1d51f7a1ad6` | `snx15ｻNﾒ5SﾐRﾉﾐﾃ62ｿ1ｶｷWFKmgﾁﾃｻ3ｸGLpﾄﾆHnXGﾘﾑDﾃJ6ｸXﾐﾂﾊwhｶtｵｾｴﾃ9PﾖDFC3YQ` |

یہ ویکٹرز Data Model WG اور Cryptography WG کی جانب سے review ہو چکے ہیں،
اور ADDR‑1a کے دائرہ کار کیلئے منظور شدہ ہیں۔

### 3. globally unique domains اور normalization

مزید تفصیل کیلئے یہ ریفرنس دیکھیں:
[`docs/source/references/address_norm_v1.md`](source/references/address_norm_v1.md)،
جہاں Torii، data model اور SDKs میں استعمال ہونے والی canonical Norm v1
pipeline بیان ہے۔

ہم `DomainId` کو ایک tagged tuple کے طور پر دوبارہ define کرتے ہیں:

```text
DomainId {
    name: Name,
    authority: GlobalDomainAuthority, // نیا enum
}

enum GlobalDomainAuthority {
    LocalChain,                  // مقامی chain کیلئے default
    External { chain_discriminant: u16 },
}
```

`LocalChain` موجودہ chain کے زیر انتظام domains کیلئے وہی Name wrap کرتا
ہے۔ جب domain global registry کے ذریعے register ہوتا ہے، تو ہم owning chain
کا discriminant اسٹور کرتے ہیں۔ فی الحال display / parsing تبدیل نہیں
ہوتا، مگر یہ توسیع routing فیصلوں کیلئے مزید معلومات فراہم کرتی ہے۔

#### 3.1 Normalization اور spoofing کے خلاف دفاع

Norm v1 وہ canonical pipeline define کرتی ہے جسے ہر component کو domain
name persist یا `AccountAddress` میں embed کرنے سے پہلے استعمال کرنا
چاہیے۔ مکمل walkthrough مذکورہ reference فائل میں ہے؛ یہاں خلاصہ درج ہے
جسے wallets، Torii، SDKs اور governance tools کو implement کرنا چاہیے:

1. **Input validation.** خالی strings، whitespace اور reserved delimiters
   `@`, `#`, `$` کو reject کریں۔ یہ `Name::validate_str` کی invariants کے
   مطابق ہے۔
2. **Unicode NFC composition.** ICU‑backed NFC normalisation لگائیں تاکہ
   canonically‑equivalent sequences (مثلاً `e\u{0301}` → `é`) deterministic
   طور پر collapse ہوں۔
3. **UTS‑46 normalization.** NFC output کو UTS‑46 سے گزاریں،  
   `use_std3_ascii_rules = true`,
   `transitional_processing = false` کے ساتھ، اور DNS‑style length enforcement
   آن رکھیں۔ نتیجہ lower‑case A‑label sequence ہوتا ہے؛ STD3 rules کی خلاف
   ورزی کرنے والی input اسی مرحلے پر fail ہو جاتی ہے۔
4. **Length limits.** DNS‑style bounds لاگو کریں: ہر label کا سائز 1–63
   bytes اور مکمل domain 255 bytes سے زیادہ نہ ہو (step 3 کے بعد)۔
5. **اختیاری confusable policy.** UTS‑39 script checks Norm v2 کیلئے track
   ہو رہے ہیں؛ operators انہیں پہلے بھی enable کر سکتے ہیں، مگر failure
   کی صورت میں processing abort ہونی چاہیے۔

اگر تمام مراحل کامیاب ہوں تو lower‑case A‑label string cache کر کے address
encoding، config، manifests اور registry lookups میں استعمال کی جاتی ہے۔  
local digest selectors اپنی 12‑بائٹ value اس طرح derive کرتے ہیں:  
`blake2s_mac(key = "SORA-LOCAL-K:v1", canonical_label)[0..12]`  
جہاں input step 3 کا output ہے۔ باقی تمام فارم (mixed case، upper‑case،
raw Unicode) boundary پر structured `ParseError` کے ساتھ reject ہو جاتے ہیں۔

punycode round‑trips اور invalid STD3 sequences کے مثالوں سمیت canonical
fixtures `docs/source/references/address_norm_v1.md` میں دیے گئے ہیں، اور
SDK CI میں ADDR‑2 کے test vector suites کے اندر mirror ہوتے ہیں۔

### 4. Nexus domain registry اور routing

- **Registry schema:** Nexus ایک signed map `DomainName -> ChainRecord`
  maintain کرتا ہے، جہاں `ChainRecord` میں chain discriminant، اختیاری
  metadata (مثلاً RPC endpoints) اور proof‑of‑authority (جیسے governance
  multisig) شامل ہوتے ہیں۔
- **Sync mechanism:**  
  - chains genesis کے دوران یا governance instructions کے ذریعے signed
    domain claims Nexus کو submit کرتی ہیں۔  
  - Nexus باقاعدگی سے manifests (signed JSON + اختیاری Merkle root)
    HTTPS اور content‑addressed storage (مثلاً IPFS) پر publish کرتا ہے۔  
    clients تازہ ترین manifest کو pin کر کے signatures verify کرتے ہیں۔
- **Lookup flow:**  
  - Torii کو ایسی transaction موصول ہوتی ہے جس میں `DomainId` ہو۔  
  - اگر domain مقامی طور پر unknown ہو تو Torii cached Nexus manifest
    کو query کرتا ہے۔  
  - اگر manifest کسی foreign chain کی طرف اشارہ کرے تو transaction
    deterministic `ForeignDomain` error (ساتھ میں remote chain info) کے ساتھ
    reject ہوتی ہے۔  
  - اگر domain Nexus میں موجود ہی نہ ہو تو Torii `UnknownDomain` واپس
    کرتا ہے۔
- **Trust anchors & rotation:** governance keys manifests sign کرتی ہیں؛
  rotation یا revocation کو نیا manifest entry بنا کر publish کیا جاتا
  ہے۔ clients manifest TTL (مثلاً 24 گھنٹے) enforce کرتے ہیں اور اس ونڈو
  سے باہر stale data consult کرنے سے انکار کرتے ہیں۔
- **Failure modes:** اگر manifest retrieval fail ہو جائے تو Torii TTL کے
  اندر cached data استعمال کرتا ہے؛ TTL گزر جانے کے بعد `RegistryUnavailable`
  اٹھایا جاتا ہے اور inconsistent state سے بچنے کیلئے cross‑domain routing
  سے انکار کیا جاتا ہے۔

### 4.1 Registry immutability، aliases اور tombstones (ADDR‑7c)

Nexus ایک **append‑only manifest** publish کرتا ہے تاکہ ہر domain یا
alias assignment کو audit اور replay کیا جا سکے۔ operators کو
[address manifest runbook](source/runbooks/address_manifest_ops.md)
میں بیان کی گئی bundle کو single source of truth سمجھنا چاہیے؛ اگر کوئی
manifest غائب ہو یا validation fail کرے تو Torii کو متعلقہ domain resolve
کرنے سے انکار کرنا چاہیے۔

**Automation support:**  
`cargo xtask address-manifest verify --bundle <current_dir> --previous <previous_dir>`
runbook میں بیان کردہ checksum، schema اور `previous_digest` چیکس کو دوبارہ
چلاتا ہے۔ change tickets میں اس command کا output شامل کریں تاکہ یہ ثابت ہو
سکے کہ `sequence` اور `previous_digest` کا لنک publish ہونے سے پہلے validate
ہوا تھا۔

#### Manifest header اور signature contract

| Field | Requirement |
|-------|------------|
| `version` | فی الحال `1`. صرف تب bump کریں جب specification بھی اپ ڈیٹ ہو۔ |
| `sequence` | ہر publication پر **بالکل ایک** کا اضافہ ہو؛ Torii caches gaps یا regressions والی revisions کو reject کرتے ہیں۔ |
| `generated_ms` + `ttl_hours` | cache freshness (default 24h) define کرتے ہیں۔ اگر اگلا manifest آنے سے پہلے TTL ختم ہو جائے تو Torii `RegistryUnavailable` پر سوئچ کرتا ہے۔ |
| `previous_digest` | پچھلے manifest body کا BLAKE3 digest (hex)؛ verifiers اسے `b3sum` سے دوبارہ نکال کر immutability prove کرتے ہیں۔ |
| `signatures` | manifests Sigstore (`cosign sign-blob`) کے ذریعے sign ہوتے ہیں؛ ops کو `cosign verify-blob --bundle manifest.sigstore manifest.json` چلانا چاہیے اور rollout سے پہلے governance identity/issuer constraints enforce کرنے چاہییں۔ |

release automation JSON body کے ساتھ ساتھ `manifest.sigstore` اور
`checksums.sha256` فائلیں بھی بناتی ہے۔ ان فائلوں کو SoraFS یا HTTP
endpoints پر mirror کرتے وقت ساتھ رکھیں تاکہ auditors verification steps
کو لفظ بہ لفظ replay کر سکیں۔

#### Entry types

| Type | Purpose | Required fields |
|------|---------|-----------------|
| `global_domain` | ظاہر کرتا ہے کہ domain عالمی طور پر register ہے اور اسے chain discriminant اور IH58 prefix سے map ہونا چاہیے۔ | `{ "domain": "<label>", "chain": "sora:nexus:global", "ih58_prefix": 753, "selector": "global" }` |
| `tombstone` | کسی alias/selector کو مستقل طور پر retire کرتا ہے؛ Local‑8 digests delete کرنے یا domain remove کرنے پر لازم ہے۔ | `{ "selector": {…}, "reason_code": "LOCAL8_RETIREMENT" \| …, "ticket": "<governance id>", "replaces_sequence": <number> }` |

`global_domain` entries میں اختیاری طور پر `manifest_url` یا `sorafs_cid`
ہو سکتا ہے جو wallets کو signed chain metadata کی طرف اشارہ کرے، مگر
canonical tuple ہمیشہ `{domain, chain, discriminant/ih58_prefix}` ہی رہے
گا۔ `tombstone` records کو لازماً retire ہونے والے selector اور متعلقہ
ticket/governance artefact کا حوالہ دینا چاہیے تاکہ audit trail آف لائن بھی
reconstruct کیا جا سکے۔

#### Alias/tombstone ورک فلو اور telemetry

1. **Drift detect کرنا۔** `torii_address_local8_total{endpoint}` اور
   `torii_address_invalid_total{endpoint,reason}` (جو
   `dashboards/grafana/address_ingest.json` میں render ہوتے ہیں) استعمال کر
   کے verify کریں کہ production میں Local‑8 strings اب قبول نہیں ہوتیں،
   پھر tombstone تجویز کریں۔
2. **Canonical digests derive کرنا۔**  
   `iroha tools address convert <address-or-account_id> --format json --expect-prefix 753`
   چلائیں (یا  
   `fixtures/account/address_vectors.json` کو
   `scripts/account_fixture_helper.py` کے ذریعے consume کریں) تاکہ عین
   `digest_hex` capture ہو سکے۔  
   CLI اب `snx1...@wonderland` جیسے inputs قبول کرتا ہے؛ JSON summary
   `input_domain` فیلڈ میں domain دکھاتا ہے، اور `--append-domain` option
   converted encoding کو `<ih58>@wonderland` کی صورت میں replay کر کے
   manifests اپ ڈیٹ کرنے میں مدد دیتا ہے۔  
   newline‑oriented exports کیلئے  
   استعمال کریں تاکہ Local selectors کو mass‑convert کر کے canonical IH58
   (یا compressed/hex/JSON) forms میں لایا جا سکے، جبکہ non‑local rows skip
   ہوتی رہیں۔ auditors کو spreadsheet‑friendly evidence دینے کیلئے  
   چلائیں، جو CSV (`input,status,format,domain_kind,…`) تیار کرے گا جس میں
   Local selectors، canonical encodings اور parse failures ایک ہی فائل میں
   نمایاں رہیں۔
3. **Manifest entries append کرنا۔** `tombstone` record (اور جب global
   registry پر migrate کریں تو follow‑up `global_domain` record) لکھیں اور
   signatures مانگنے سے پہلے manifest کو `cargo xtask address-vectors` کے
   ساتھ validate کریں۔
4. **Verify اور publish کرنا.** runbook checklist (hashes، Sigstore، اور
   `sequence` کی monotonicity) follow کریں اور bundle کو SoraFS پر mirror
   کرتا ہے، لہٰذا production clusters bundle landing کے فوراً بعد canonical
   IH58 (preferred)/snx1 (second-best) literals enforce کرتے ہیں۔
5. **Monitoring اور rollback.** کم از کم 30 دن تک Local‑8 panels کو zero پر
   رکھیں؛ اگر regressions نظر آئیں تو previous manifest bundle دوبارہ
   publish کریں، اور متاثرہ non‑production environment میں عارضی طور پر
   جائے۔

اوپر کی تمام steps ADDR‑7c کیلئے لازمی ثبوت ہیں: ایسے manifests جن میں
`cosign` signature bundle نہ ہو یا `previous_digest` values match نہ کرتی
ہوں، خودکار طور پر reject ہونی چاہییں، اور operators کو verification logs
اپنی change tickets کے ساتھ attach کرنے چاہییں۔

</div>
