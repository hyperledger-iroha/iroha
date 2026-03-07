---
lang: am
direction: ltr
source: docs/account_structure.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 01561366c9698c10d29ff3f49ad4c14b22b1796b5c7701cf98d200a140af1caf
source_last_modified: "2026-01-28T17:11:30.635172+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# የመለያ መዋቅር RFC

** ሁኔታ፡** ተቀባይነት ያለው (ADDR-1)  
**ተመልካቾች፡** የውሂብ ሞዴል፣ Torii፣ Nexus፣ Wallet፣ የአስተዳደር ቡድኖች  
** ተዛማጅ ጉዳዮች:** TBD

## ማጠቃለያ

ይህ ሰነድ የተተገበረውን የመላኪያ መለያ አድራሻ ቁልል ይገልጻል
`AccountAddress` (`crates/iroha_data_model/src/account/address.rs`) እና እ.ኤ.አ.
ተጓዳኝ መሳሪያ. ያቀርባል፡-

- ቼክ የተጠቃለለ፣ ሰውን የሚያይ **Iroha Base58 አድራሻ (IH58)** በ
  `AccountAddress::to_ih58` ሰንሰለት አድልዎ ከመለያው ጋር የሚያገናኝ
  ተቆጣጣሪ እና ወሳኙ ለኢንተርፕ ተስማሚ የጽሑፍ ቅጾችን ያቀርባል።
- ለተዘዋዋሪ ነባሪ ጎራዎች እና የአካባቢ ውህዶች ጎራ መራጮች፣ ሀ
  ለወደፊት Nexus የሚደገፍ ማዘዋወር የተጠበቀ የአለምአቀፍ መዝገብ መራጭ መለያ (የ
  የመመዝገቢያ ፍለጋ ** ገና አልተላከም**).

## ተነሳሽነት

የኪስ ቦርሳዎች እና ከሰንሰለት ውጪ ያሉ መሳሪያዎች ዛሬ በጥሬው `alias@domain` ማዞሪያ ተለዋጭ ስሞች ላይ ተመስርተዋል። ይህ
ሁለት ዋና ድክመቶች አሉት

1. ** ምንም የአውታረ መረብ ትስስር የለም።** ሕብረቁምፊው ምንም ቼክ ወይም ሰንሰለት ቅድመ ቅጥያ የለውም፣ ስለዚህ ተጠቃሚዎች
   አፋጣኝ ግብረ መልስ ሳይኖር ከተሳሳተ አውታረ መረብ አድራሻ መለጠፍ ይችላል። የ
   ግብይቱ በመጨረሻ ውድቅ ይሆናል (የሰንሰለት አለመዛመድ) ወይም፣ ይባስ፣ ይሳካል
   መድረሻው በአካባቢው ካለ ባልታሰበ ሂሳብ ላይ።
2. **የጎራ ግጭት።** ጎራዎች የስም ቦታ-ብቻ ናቸው እና በእያንዳንዱ ላይ እንደገና ጥቅም ላይ ሊውሉ ይችላሉ
   ሰንሰለት. የአገልግሎቶች ፌዴሬሽን (ጠባቂዎች፣ ድልድዮች፣ ሰንሰለት ተሻጋሪ የስራ ፍሰቶች)
   በሰንሰለት A ላይ ያለው `finance` ጋር ስለማይገናኝ I18NI00000076
   ሰንሰለት B.

ከቅጂ/መለጠፍ ስህተቶች የሚጠብቅ ለሰው ተስማሚ የአድራሻ ቅርጸት እንፈልጋለን
እና ከጎራ ስም እስከ ስልጣን ያለው ሰንሰለት የሚወስን ካርታ።

# ግቦች

- በመረጃ ሞዴል ውስጥ የተተገበረውን IH58 Base58 ኤንቨሎፕ ይግለጹ እና የ
  ቀኖናዊ ትንተና/ተለዋጭ ስም `AccountId` እና `AccountAddress` ይከተላሉ።
- የተዋቀረውን ሰንሰለት አድልዎ በቀጥታ ወደ እያንዳንዱ አድራሻ እና
  የአስተዳደር / የመመዝገቢያ ሂደቱን ይግለጹ.
- የአሁኑን ሳያቋርጡ የአለምአቀፍ የጎራ መዝገብ እንዴት ማስተዋወቅ እንደሚቻል ይግለጹ
  ማሰማራት እና የመደበኛነት / ጸረ-ስፖፊንግ ደንቦችን ይግለጹ.

## ግቦች ያልሆኑ

- ሰንሰለት ተሻጋሪ የንብረት ዝውውሮችን በመተግበር ላይ። የማዞሪያው ንብርብር ብቻ ይመልሳል
  የዒላማ ሰንሰለት.
- ለአለምአቀፍ ጎራ አሰጣጥ አስተዳደርን ማጠናቀቅ. ይህ RFC በመረጃው ላይ ያተኩራል።
  ሞዴል እና የመጓጓዣ ጥንታዊ.

## ዳራ

### የአሁን ማዞሪያ ተለዋጭ ስም

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
- `<alias>@<domain>` for domain-scoped alias routing; account IDs themselves are canonical encoded literals (IH58 or compressed).
- `uaid:<hex>` / `opaque:<hex>` literals resolved via UAID/opaque resolvers.

Multihash hex is canonical: varint bytes are lowercase hex, payload bytes are uppercase hex,
and `0x` prefixes are not accepted.

This text form is now treated as an **account alias**: a routing convenience
that points to the canonical [`AccountAddress`](#2-canonical-address-codecs).
It remains useful for human readability and domain-scoped governance, but it is
no longer considered the authoritative account identifier on-chain.
```

`ChainId` ከ `AccountId` ውጭ ይኖራል። አንጓዎች የግብይቱን `ChainId` ይፈትሹ
በመግቢያው ወቅት ውቅረትን በመቃወም (`AcceptTransactionFail::ChainIdMismatch`)
እና የውጭ ግብይቶችን ውድቅ ያድርጉ ፣ ግን የመለያው ሕብረቁምፊ ራሱ ቁ
የአውታረ መረብ ፍንጭ.

### የጎራ መለያዎች

`DomainId` `Name` (የተለመደ ሕብረቁምፊ) ይጠቀልላል እና በአካባቢው ሰንሰለት ላይ የተዘረጋ ነው።
እያንዳንዱ ሰንሰለት `wonderland`, I18NI0000087X, ወዘተ ለብቻው መመዝገብ ይችላል.

### Nexus አውድ

Nexus ለክፍለ-አቋራጭ ማስተባበር (መስመሮች/መረጃ-ክፍተቶች) ኃላፊነት አለበት። እሱ
በአሁኑ ጊዜ የሰንሰለት-አቋራጭ ጎራ ማዘዋወር ጽንሰ-ሀሳብ የለውም።

## የታቀደ ንድፍ

### 1. ቆራጥ ሰንሰለት አድሎአዊ

`iroha_config::parameters::actual::Common` አሁን ያጋልጣል፡

```rust
pub struct Common {
    pub chain: ChainId,
    pub chain_discriminant: u16, // globally coordinated
    // ... existing fields
}
```

- ** ገደቦች: ***
  - ለእያንዳንዱ ንቁ አውታረ መረብ ልዩ; ጋር በተፈረመ የህዝብ መዝገብ ቤት የሚተዳደር
    ግልጽ የሆኑ የተጠበቁ ክልሎች (ለምሳሌ፣ `0x0000–0x0FFF` test/dev፣ `0x1000–0x7FFF`
    የማህበረሰብ ድልድል፣ I18NI0000091X አስተዳደር-የጸደቀ፣ `0xFFF0–0xFFFF`
    የተያዘ)።
  - ለሩጫ ሰንሰለት የማይለወጥ. እሱን መቀየር ጠንካራ ሹካ እና ሀ
    የመመዝገቢያ ዝማኔ.
- ** አስተዳደር እና መዝገብ ቤት (የታቀደ)** ባለብዙ ፊርማ አስተዳደር ስብስብ ይሆናል።
  የተፈረመ የJSON መዝገብ ካርታን ለሰው ተለዋጭ ስሞች ማቆየት እና
  CAIP-2 መለያዎች። ይህ መዝገብ እስካሁን የተላከው የማስኬጃ ጊዜ አካል አይደለም።
- ** አጠቃቀም፡** በግዛት መግቢያ፣ Torii፣ ኤስዲኬዎች፣ እና የኪስ ቦርሳ ኤ.ፒ.አይ.
  እያንዳንዱ አካል መክተት ወይም ማረጋገጥ ይችላል። የ CAIP-2 መጋለጥ ለወደፊቱ ይቀራል
  interop ተግባር.

### 2. ቀኖናዊ አድራሻ ኮዴኮች

የ Rust ውሂብ ሞዴል ነጠላ ቀኖናዊ የክፍያ ጭነት ውክልና ያጋልጣል
(`AccountAddress`) እንደ በርካታ የሰው ፊት ቅርጸቶች ሊወጣ ይችላል። IH58 ነው።
ለማጋራት እና ቀኖናዊ ውፅዓት ተመራጭ መለያ ቅርጸት; የተጨመቀው
`sora` ቅጽ የቃና ፊደል ባለበት ለ UX ሁለተኛ-ምርጥ የሶራ-ብቻ አማራጭ ነው።
እሴት ይጨምራል። ቀኖናዊ ሄክስ ማረም አጋዥ ሆኖ ይቆያል።

- ** IH58 (Iroha Base58)** - ሰንሰለቱን የሚጨምር Base58 ፖስታ
  አድሎአዊ. ዲኮደሮች ክፍያውን ወደ ላይ ከማስተዋወቅዎ በፊት ቅድመ ቅጥያውን ያረጋግጣሉ
  ቀኖናዊው ቅፅ.
- **በሶራ የታመቀ እይታ *** - የሶራ-ብቻ ፊደል **105 ምልክቶች** የተገነባው
  የግማሽ ወርድ イロハ ግጥም (ヰ እና ヱን ጨምሮ) ከ58-ቁምፊ ጋር በማያያዝ
  IH58 ስብስብ ሕብረቁምፊዎች የሚጀምሩት በሴንቲነል I18NI0000095X ነው፣ከBech32m-የተገኘ
  checksum፣ እና የአውታረ መረብ ቅድመ ቅጥያውን ተወው (Sora Nexus በመልእክተኛው የተነገረ ነው።)

  ```
  IH58  : 123456789ABCDEFGHJKMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz
  Iroha : ｲﾛﾊﾆﾎﾍﾄﾁﾘﾇﾙｦﾜｶﾖﾀﾚｿﾂﾈﾅﾗﾑｳヰﾉｵｸﾔﾏｹﾌｺｴﾃｱｻｷﾕﾒﾐｼヱﾋﾓｾｽ
  ```
- ** ቀኖናዊ ሄክስ *** - ለማረም ተስማሚ `0x…` የ ቀኖናዊ ባይት ኮድ
  ኤንቨሎፕ.

`AccountAddress::parse_encoded` IH58 (የተመረጠ)፣ የታመቀ (`sora`፣ ሁለተኛ-ምርጥ) ወይም ቀኖናዊ ሄክስን በራስ ሰር ያገኛል
(`0x...` ብቻ፤ ባዶ ሄክስ ውድቅ ነው) ግብአት እና የተፈታውን ክፍያ እና የተገኘውን ሁለቱንም ይመልሳል።
`AccountAddressFormat`. Torii አሁን `parse_encoded` ለ ISO 20022 ማሟያ ይደውላል
ቀኖናዊውን ሄክስ ቅጽ አድራሻዎችን እና ማከማቻዎችን ያከማቻል ስለዚህ ሜታዳታ የሚወሰን ሆኖ ይቆያል
ዋናው ውክልና ምንም ይሁን ምን.

#### 2.1 የራስጌ ባይት አቀማመጥ (ADDR-1a)

እያንዳንዱ ቀኖናዊ ክፍያ እንደ `header · controller` ተቀምጧል። የ
`header` የትኞቹ ተንታኝ ህጎች በባይቶች ላይ እንደሚተገበሩ የሚገልጽ ነጠላ ባይት ነው።
ተከተል፡-

```
bit index:   7        5 4      3 2      1 0
             ┌─────────┬────────┬────────┬────┐
payload bit: │version  │ class  │  norm  │ext │
             └─────────┴────────┴────────┴────┘
```

ስለዚህ የመጀመሪያው ባይት ለታች ዲኮድደሮች የሼማ ሜታዳታን ይይዛል፡-

| ቢት | መስክ | የተፈቀዱ እሴቶች | ጥሰት ላይ ስህተት |
|--------|
| 7-5 | `addr_version` | `0` (v1)። እሴቶች `1-7` ለወደፊት ክለሳዎች የተጠበቁ ናቸው። | ከ `0-7` ውጭ ያሉ እሴቶች `AccountAddressError::InvalidHeaderVersion`; ትግበራዎች ዜሮ ያልሆኑ ስሪቶችን ዛሬ እንደማይደገፉ አድርገው ማከም አለባቸው። |
| 4-3 | `addr_class` | `0` = ነጠላ ቁልፍ፣ `1` = multisig። | ሌሎች እሴቶች `AccountAddressError::UnknownAddressClass` ያሳድጋሉ። |
| 2-1 | `norm_version` | `1` (መደበኛ v1)። እሴቶች `0`፣ `2`፣ `3` የተጠበቁ ናቸው። | ከ `0-3` ውጭ ያሉ እሴቶች `AccountAddressError::InvalidNormVersion` ከፍ ያደርጋሉ። |
| 0 | `ext_flag` | `0` መሆን አለበት። | አዘጋጅ ቢት `AccountAddressError::UnexpectedExtensionFlag` ከፍ ያደርገዋል። |

የ Rust ኢንኮደር `0x02` ለአንድ ቁልፍ ተቆጣጣሪዎች ይጽፋል (ስሪት 0፣ ክፍል 0፣
መደበኛ v1፣ የኤክስቴንሽን ባንዲራ ጸድቷል) እና `0x0A` ለብዙ ሲግ ተቆጣጣሪዎች (ስሪት 0፣
ክፍል 1፣ መደበኛ v1፣ የኤክስቴንሽን ባንዲራ ጸድቷል)።

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

#### 2.3 የመቆጣጠሪያ ክፍያ ኢንኮዲንግ (ADDR-1a)

የመቆጣጠሪያው ክፍያ ከጎራ መራጭ በኋላ የተጨመረ ሌላ መለያ የተሰጠው ማህበር ነው፡-| መለያ | ተቆጣጣሪ | አቀማመጥ | ማስታወሻ |
|-------------|--------|-------|
| `0x00` | ነጠላ ቁልፍ | `curve_id:u8` · `key_len:u8` · `key_bytes` | `curve_id=0x01` ካርታዎች ወደ Ed25519 ዛሬ። `key_len` ከ `u8` ጋር የተቆራኘ ነው; ትላልቅ እሴቶች `AccountAddressError::KeyPayloadTooLong` ያሳድጋሉ (ስለዚህ ነጠላ-ቁልፍ ML-DSA ይፋዊ ቁልፎች> 255 ባይት ናቸው፣ ሊመደቡ አይችሉም እና መልቲሲግ መጠቀም አለባቸው)። |
| `0x01` | መልቲሲግ | `version:u8` · `threshold:u16` · `member_count:u8` · (`curve_id:u8` · `weight:u16` · `key_len:u16` · I18NI00000164)\* እስከ 255 አባላትን ይደግፋል (`CONTROLLER_MULTISIG_MEMBER_MAX`)። ያልታወቁ ኩርባዎች `AccountAddressError::UnknownCurve` ከፍ ያደርጋሉ; የተበላሹ ፖሊሲዎች እንደ `AccountAddressError::InvalidMultisigPolicy` አረፋ ይወጣሉ። |

የመልቲሲግ ፖሊሲዎች የ CTAP2 አይነት CBOR ካርታ እና ቀኖናዊ መፈጨትን ያጋልጣሉ
አስተናጋጆች እና ኤስዲኬዎች ተቆጣጣሪውን በቆራጥነት ማረጋገጥ ይችላሉ። ተመልከት
`docs/source/references/multisig_policy_schema.md` (ADDR-1c) ለዕቅዱ፣
የማረጋገጫ ህጎች፣ የሃሽ አሰራር እና ወርቃማ እቃዎች።

ሁሉም የቁልፍ ባይት በ `PublicKey::to_bytes` እንደተመለሰ በትክክል ተቀምጠዋል። ዲኮደሮች የ`PublicKey` ምሳሌዎችን እንደገና ይገነባሉ እና ባይቶች ከታወጀው ከርቭ ጋር የማይዛመዱ ከሆነ `AccountAddressError::InvalidPublicKey` ያሳድጉ።

> ** Ed25519 ቀኖናዊ ማስፈጸሚያ (ADDR-3a):** ጥምዝ `0x01` ቁልፎች በፈራሚው ወደ ሚወጣው ትክክለኛ ባይት ሕብረቁምፊ መፍታት አለባቸው እና በትንሽ-ትዕዛዝ ንዑስ ቡድን ውስጥ መዋሸት የለባቸውም። አንጓዎች አሁን ቀኖናዊ ያልሆኑ ኢንኮዲንግዎችን (ለምሳሌ፣ እሴቶች የተቀነሰው ሞዱሎ `2^255-19`) እና እንደ መታወቂያ ኤለመንት ያሉ ደካማ ነጥቦችን ውድቅ ያደርጋሉ፣ ስለዚህ ኤስዲኬዎች አድራሻዎችን ከማቅረባቸው በፊት ተዛማጅ የማረጋገጫ ስህተቶችን ማየት አለባቸው።

##### 2.3.1 ከርቭ መለያ መዝገብ (ADDR-1d)

| መታወቂያ (`curve_id`) | አልጎሪዝም | የባህሪ በር | ማስታወሻ |
|-------------|-------------|-------|
| `0x00` | የተያዘ | - | መልቀቅ የለበትም; ዲኮደሮች ላዩን `ERR_UNKNOWN_CURVE`. |
| `0x01` | ኢድ25519 | - | ቀኖናዊ v1 አልጎሪዝም (`Algorithm::Ed25519`); በነባሪ ውቅር ውስጥ ነቅቷል። |
| `0x02` | ML-DSA (ዲሊቲየም3) | - | የዲሊቲየም3 የህዝብ ቁልፍ ባይት (1952 ባይት) ይጠቀማል። ነጠላ-ቁልፍ አድራሻዎች ML-DSA መመስጠር አይችሉም ምክንያቱም `key_len` `u8` ነው; multisig `u16` ርዝማኔዎችን ይጠቀማል. |
| `0x03` | BLS12-381 (መደበኛ) | `bls` | የህዝብ ቁልፎች በG1 (48 ባይት)፣ ፊርማዎች በG2 (96 ባይት)። |
| `0x04` | ሰከንድ256k1 | - | ቆራጥ ECDSA ከSHA-256; የህዝብ ቁልፎች 33-ባይት SEC1 የታመቀ ቅጽ ይጠቀማሉ እና ፊርማዎች ቀኖናዊውን 64-ባይት `r∥s` አቀማመጥ ይጠቀማሉ። |
| `0x05` | BLS12-381 (ትንሽ) | `bls` | የህዝብ ቁልፎች በG2 (96 ባይት)፣ ፊርማዎች በG1 (48 ባይት)። |
| `0x0A` | GOST R 34.10-2012 (256, ስብስብ A) | `gost` | የ`gost` ባህሪ ሲነቃ ብቻ ይገኛል። |
| `0x0B` | GOST R 34.10-2012 (256, ስብስብ B) | `gost` | የ`gost` ባህሪ ሲነቃ ብቻ ይገኛል። |
| `0x0C` | GOST R 34.10-2012 (256, ስብስብ C) | `gost` | የ`gost` ባህሪ ሲነቃ ብቻ ይገኛል። |
| `0x0D` | GOST R 34.10-2012 (512, ስብስብ A) | `gost` | የ`gost` ባህሪ ሲነቃ ብቻ ይገኛል። |
| `0x0E` | GOST R 34.10-2012 (512, ስብስብ B) | `gost` | የ`gost` ባህሪ ሲነቃ ብቻ ይገኛል። |
| `0x0F` | SM2 | `sm` | የDistID ርዝመት (u16 BE) + DistID ባይት + 65-ባይት SEC1 ያልታመቀ SM2 ቁልፍ; `sm` ሲነቃ ብቻ ይገኛል። |

የቁማር `0x06–0x09` ለተጨማሪ ኩርባዎች ያልተመደበ ይቆያል; አዲስ በማስተዋወቅ ላይ
አልጎሪዝም የመንገድ ካርታ ማሻሻያ እና ተዛማጅ SDK/አስተናጋጅ ሽፋን ያስፈልገዋል። ኢንኮዲተሮች
በ`ERR_UNSUPPORTED_ALGORITHM` ያልተደገፈ ስልተ ቀመር አለመቀበል አለብህ፣ እና
ዲኮደሮች ለማቆየት ከ`ERR_UNKNOWN_CURVE` ጋር በማይታወቁ መታወቂያዎች ላይ በፍጥነት መውደቅ አለባቸው
ያልተሳካ-ዝግ ባህሪ.

ቀኖናዊው መዝገብ ቤት (በማሽን ሊነበብ የሚችል JSON ወደ ውጭ መላክን ጨምሮ) ይኖራል
[`docs/source/references/address_curve_registry.md`](source/references/address_curve_registry.md)።
የጠመዝማዛ ለዪዎች እንዲቀሩ የመሳሪያ ስራ ያንን የውሂብ ስብስብ በቀጥታ ሊበላው ይገባል።
በኤስዲኬዎች እና ኦፕሬተሮች የስራ ፍሰቶች ላይ ወጥነት ያለው።

- ** ኤስዲኬ ጌቲንግ፡** ኤስዲኬዎች ነባሪ ወደ Ed25519-ብቻ ማረጋገጫ/መቀየሪያ። ስዊፍት ያጋልጣል
  የሰዓት ባንዲራዎች (`IROHASWIFT_ENABLE_MLDSA`፣ `IROHASWIFT_ENABLE_GOST`፣
  `IROHASWIFT_ENABLE_SM`); የጃቫ/አንድሮይድ ኤስዲኬ ያስፈልገዋል
  `AccountAddress.configureCurveSupport(...)`; ጃቫስክሪፕት ኤስዲኬ ይጠቀማል
  `configureCurveSupport({ allowMlDsa: true, allowGost: true, allowSm2: true })`.
  secp256k1 ድጋፍ አለ ነገር ግን በ JS/አንድሮይድ ውስጥ በነባሪነት አልነቃም።
  ኤስዲኬዎች; ደዋዮች Ed25519 ያልሆኑ መቆጣጠሪያዎችን በሚያወጡበት ጊዜ በግልፅ መርጠው መግባት አለባቸው።
- ** አስተናጋጅ ጌቲንግ፡** `Register<Account>` ፈራሚዎቻቸው ስልተ ቀመሮችን የሚጠቀሙ ተቆጣጣሪዎችን ውድቅ ያደርጋል።
  ከ መስቀለኛ መንገድ `crypto.allowed_signing` ዝርዝር ** ወይም ** የጥምዝ መለያዎች የሉም
  `crypto.curves.allowed_curve_ids`፣ስለዚህ ዘለላዎች ድጋፍን ማስተዋወቅ አለባቸው (ውቅር +
  ዘፍጥረት) ML-DSA/GOST/SM መቆጣጠሪያዎች ከመመዝገባቸው በፊት። BLS መቆጣጠሪያ
  ስልተ ቀመሮች ሁል ጊዜ ሲዘጋጁ ይፈቀዳሉ (የስምምነት ቁልፎች በእነሱ ላይ ይደገፋሉ)
  እና ነባሪ ውቅር Ed25519 + sec256k1ን ያነቃል።【crates/iroha_core/src/smartcontracts/isi/domain.rs:32】

##### 2.3.2 Multisig መቆጣጠሪያ መመሪያ

`AccountController::Multisig` ተከታታይ ፖሊሲዎች በ
`crates/iroha_data_model/src/account/controller.rs` እና ዕቅዱን ያስፈጽማል
በ[`docs/source/references/multisig_policy_schema.md`](I18NU0000066X) ተመዝግቧል።
ቁልፍ የአተገባበር ዝርዝሮች፡-

- ፖሊሲዎች የተለመዱ እና የተረጋገጡ በ `MultisigPolicy::validate()` በፊት ነው።
  እየተከተተ ነው። ገደቦች ≥1 እና ≤Σ ክብደት መሆን አለባቸው; የተባዙ አባላት ናቸው።
  በ `(algorithm || 0x00 || key_bytes)` ከተደረደሩ በኋላ በቆራጥነት ተወግዷል።
- የሁለትዮሽ ተቆጣጣሪው ጭነት (`ControllerPayload::Multisig`) ኢንኮዶች
  `version:u8`፣ `threshold:u16`፣ `member_count:u8`፣ ከዚያ የእያንዳንዱ አባል
  `(curve_id, weight:u16, key_len:u16, key_bytes)`. ይህ በትክክል ነው
  `AccountAddress::canonical_bytes()` ለ IH58 (የተመረጡ)/ሶራ (ሁለተኛ-ምርጥ) ጭነት ይጽፋል።
- Hashing (`MultisigPolicy::digest_blake2b256()`) Blake2b-256 ከ ጋር ይጠቀማል
  `iroha-ms-policy` ግላዊነት ማላበስ ሕብረቁምፊ ስለዚህ የአስተዳደር መገለጫዎች ከ ሀ
  በIH58 ውስጥ ከተካተቱት የመቆጣጠሪያ ባይት ጋር የሚዛመድ የውሳኔ ፖሊሲ መታወቂያ።
- ቋሚ ሽፋን በ `fixtures/account/address_vectors.json` ውስጥ ይኖራል (ጉዳይ
  `addr-multisig-*`)። የኪስ ቦርሳዎች እና ኤስዲኬዎች ቀኖናዊውን IH58 ሕብረቁምፊዎች ማረጋገጥ አለባቸው
  ከታች ያሉት መቀየሪያዎቻቸው ከዝገቱ አተገባበር ጋር የሚዛመዱ መሆናቸውን ለማረጋገጥ።

| የጉዳይ መታወቂያ | ገደብ / አባላት | IH58 ቀጥተኛ (ቅድመ ቅጥያ I18NI0000234X) | Sora compressed (I18NI0000235X) በጥሬው | ማስታወሻ |
|--------|-----------
| `addr-multisig-council-threshold3` | `≥3` ክብደት፣ አባላት `(2,1,1)` | `SRfSHsrH3tEmYaaAYyD248F3vfT1oQ3WEGS22MaD8W9bLefF7rsoKLYGcpbcM9EcSus5ZhCAZU7ztn2BCsyeCAdfRncAVmVsipd4ibk6CBLF3Nrzcw8P7VKJg6mtFgEhWVTjfDkUMoc63oeEmaWyV6cyiphwk8ZgKAJUe4TyVtmKm1WWcg7qZ6i` | `sora3vﾑ2zkaoUwﾋﾅGﾘﾚyﾂe3ﾖfﾙヰｶﾘﾉwｷnoWﾛYicaUr3ﾔｲﾖ2Ado3TﾘYQﾉJqﾜﾇｳﾑﾐd8dDjRGｦ3Vﾃ9HcﾀMヰR8ﾎﾖgEqGｵEｾDyc5ﾁ1ﾔﾉ31sUﾑﾀﾖaｸxﾘ3ｲｷMEuFｺｿﾉBQSVQnxﾈeJzrXLヰhｿｹ5SEEﾅPﾂﾗｸdヰﾋ1bUGHｲVXBWNNJ6K` | የምክር ቤት-ጎራ አስተዳደር ምልአተ ጉባኤ። |
| `addr-multisig-wonderland-threshold2` | `≥2`፣ አባላት `(1,2)` | `3xsmkps1KPBn9dtpE5qHRhHEZCpiAe8d9j6H9A42TV6kc1TpaqdwnSksKgQrsSEHznqvWKBMc1os69BELzkLjsR7EV2gjV14d9JMzo97KEmYoKtxCrFeKFAcy7ffQdboV1uRt` | `sora2ﾖZﾘeｴAdx3ﾂﾉﾔXhnｹﾀ2ﾉｱﾋxﾅﾄﾌヱwﾐmﾊvEﾐCﾏﾎｦ1ﾑHﾋso2GKﾔﾕﾁwﾂﾃP6ﾁｼﾙﾖｺ9ｻｦbﾈ4wFdﾑFヰ3HaﾘｼMｷﾌHWtｷﾋLﾙﾖQ4D3XﾊﾜXmpktﾚｻ5ﾅﾅﾇ1gkﾏsCFQGH9` | ባለሁለት ፊርማ ድንቅ አገር ምሳሌ (ክብደት 1 + 2)። |
| `addr-multisig-default-quorum3` | `≥3`፣ አባላት `(1,1,1,1)` | `nA2bDNhMqXz7ERkHNoEWbvJGyR1aDRsw32LaUWLgbK3vcpzohmdFCLvdotxUWWDY3aZeX4ptLk4Z6TjF5ossnJm8VrNo6daxmGTkqUyP4MxJxiNyPFxsEE5DLnsoLWUcxaWNpZ76tmkbiGS31Gv8tejKpuiHUMaQ1s5ohWyZvDnpycNkBK8AEfGJqn5yc9zAzfWbVhpDwkPj8ScnzvH1Echr5` | `soraﾐ38ﾅｴｸﾜ8ﾃzwBrqﾘｺ4yﾄv6kqJp1ｳｱﾛｿrzﾄﾃﾘﾒRﾗtV9ｼﾔPｽcヱEﾌVVVｼﾘｲZAｦﾓﾅｦeﾒN76vﾈcuｶuﾛL54rzﾙﾏX2zMﾌRLﾃﾋpﾚpｲcHﾑﾅﾃﾔzｵｲVfAﾃﾚﾎﾚCヰﾔｲｽｦw9ﾔﾕ8bGGkﾁ6sNｼaｻRﾖﾜYﾕﾚU18ﾅHヰﾌuMeﾊtﾂrｿj95Ft8ﾜ3fﾄkNiｴuﾈrCﾐQt8ヱｸｸmﾙﾒgUbﾑEKTTCM` | ስውር-ነባሪ የጎራ ምልአተ ጉባኤ ለመሠረታዊ አስተዳደር ጥቅም ላይ ይውላል።

#### 2.4 የውድቀት ህጎች (ADDR-1a)

- ጭነቶች ከሚፈለገው ራስጌ + መራጭ ወይም ከተረፈ ባይት ኢ18NI00000251X ወይም I18NI0000252X ያጠረ።
- የተያዙትን `ext_flag` የሚያዘጋጁ ወይም የማይደገፉ ስሪቶች/ክፍል የሚያስተዋውቁ ራስጌዎች `UnexpectedExtensionFlag`፣ `InvalidHeaderVersion`፣ ወይም `UnknownAddressClass` በመጠቀም ውድቅ መደረግ አለባቸው።
- ያልታወቀ መራጭ/ተቆጣጣሪ መለያዎች `UnknownDomainTag` ወይም `UnknownControllerTag` ከፍ ያደርጋሉ።
- ከመጠን በላይ የሆነ ወይም የተበላሸ ቁልፍ ቁሳቁስ `KeyPayloadTooLong` ወይም `InvalidPublicKey` ከፍ ያደርገዋል።
- ከ255 አባላት በላይ የሆኑ ባለብዙ ሲግ ተቆጣጣሪዎች `MultisigMemberOverflow` ያሳድጋሉ።
- IME/NFKC ልወጣዎች፡- የግማሽ ስፋት የሶራ ካና ዲኮዲንግ ሳይሰበር ወደ ሙሉ ስፋት ቅፆች ሊስተካከል ይችላል፣ነገር ግን ASCII `sora` sentinel እና IH58 አሃዞች/ፊደሎች ASCII መቆየት አለባቸው። ባለሙሉ ስፋት ወይም በኬዝ የታጠፈ ሴንቴልስ ላዩን `ERR_MISSING_COMPRESSED_SENTINEL`፣ ሙሉ ስፋት ASCII የሚጫኑ ጭነቶች `ERR_INVALID_COMPRESSED_CHAR` ከፍ ያደርጋሉ፣ እና የቼክሰም አለመዛመጃዎች እንደ `ERR_CHECKSUM_MISMATCH` ከፍ ይላል። በ`crates/iroha_data_model/src/account/address.rs` ውስጥ ያሉ የንብረት ሙከራዎች እነዚህን መንገዶች ስለሚሸፍኑ ኤስዲኬዎች እና የኪስ ቦርሳዎች በወሳኝ ውድቀቶች ላይ ሊመሰረቱ ይችላሉ።
- Torii እና ኤስዲኬ የ`address@domain` ተለዋጭ ስሞችን መፈተሽ አሁን IH58 (የተመረጡ)/ሶራ (ሁለተኛ-ምርጥ) ግብዓቶች ከተለዋጭ ስም መመለሻ በፊት (ለምሳሌ) የደንበኛ አወቃቀር አለመመጣጠን ሳይቻል ሲገመት ተመሳሳይ `ERR_*` ኮዶችን ያወጣሉ ከስድ ንባቦች.
- የአካባቢ መራጭ ከ 12 ባይት ወለል `ERR_LOCAL8_DEPRECATED` ያጠረ ይጭናል፣ ከውርስ Local-8 መፋጨት ጠንካራ መቆራረጥን ይጠብቃል።
- Domainless IH58 (preferred)/sora (second-best) literals bind directly to the configured default domain label for canonical selector-free payloads. Legacy selector-bearing literals without an explicit `@<domain>` suffix may still fail with `ERR_DOMAIN_SELECTOR_UNRESOLVED` when domain reconstruction is impossible.

#### 2.5 መደበኛ ሁለትዮሽ ቬክተሮች

- ** ስውር ነባሪ ጎራ (`default`፣ ዘር ባይት `0x00`)**  
  ቀኖናዊ ሄክስ፡ `0x020001203b6a27bcceb6a42d62a3a8d02a6f0d73653215771de243a63ac048a18b59da29`።  
  መለያየት፡ `0x02` ራስጌ፣ `0x00` መራጭ (ስውር ነባሪ)፣ `0x00` መቆጣጠሪያ መለያ፣ `0x01` ጥምዝ መታወቂያ (Ed25519)፣ `0x20` ቁልፍ ርዝመቱ ጫን
- ** የአካባቢ ጎራ መፍጨት (`treasury`፣ የዘር ባይት `0x01`)**  
  ቀኖናዊ ሄክስ፡ `0x0201b18fe9c1abbac45b3e38fc5d0001208a88e3dd7409f195fd52db2d3cba5d72ca6709bf1d94121bf3748801b40f6f5c`።  
  መለያየት፡ I18NI0000282X ራስጌ፣ መራጭ መለያ I18NI0000283X ሲደመር መፈጨት `b1 8f e9 c1 ab ba c4 5b 3e 38 fc 5d`፣ በመቀጠል ነጠላ-ቁልፍ ክፍያ (`0x00` መለያ፣ `0x01` ከርቭ መታወቂያ፣00102by Ed25519 ቁልፍ)የዩኒት ፈተናዎች (`account::address::tests::parse_encoded_accepts_all_formats`) ከዚህ በታች ያሉትን የV1 ቬክተሮች በ`AccountAddress::parse_encoded` በኩል ያረጋግጣሉ፣ ይህም የመሳሪያ ስራ በሄክስ፣ IH58 (ተመራጭ) እና በተጨመቀ (`sora`፣ ሁለተኛ-ምርጥ) ቅጾች ላይ ባለው ቀኖናዊ ጭነት ላይ እንደሚተማመን ዋስትና ይሰጣል። በ`cargo run -p iroha_data_model --example address_vectors` የተራዘመውን የመገጣጠሚያ ስብስብ እንደገና ያድሱ።

| ጎራ | ዘር ባይት | ቀኖናዊ ሄክስ | የታመቀ (`sora`) |
|------------|-------|
| ነባሪ | `0x00` | `0x020001203b6a27bcceb6a42d62a3a8d02a6f0d73653215771de243a63ac048a18b59da29` | `sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE` |
| ግምጃ ቤት | `0x01` | `0x0201b18fe9c1abbac45b3e38fc5d0001208a88e3dd7409f195fd52db2d3cba5d72ca6709bf1d94121bf3748801b40f6f5c` | `sora5ｻu6rﾀCヰTGwﾏ1ﾅヱﾌQｲﾖﾇqCｦヰﾓZQCZRDSSﾅMｱﾙヱｹﾁｸ8ｾeﾄﾛ6C8bZuwﾗｹCZｦRSLQFU` |
| ድንቅ አገር | `0x02` | `0x0201b8ae571b79c5a80f5834da2b0001208139770ea87d175f56a35466c34c7ecccb8d8a91b4ee37a25df60f5b8fc9b394` | `sora5ｻwﾓyRｿqﾏnMﾀﾙヰKoﾒﾇﾓQｺﾛyｼ3ｸFHB2F5LyPﾐTMZkｹｼw67ﾋVﾕｻr8ﾉGﾇeEnｻVRNKCS` |
| ኢሮሃ | `0x03` | `0x0201de8b36819700c807083608e2000120ed4928c628d1c2c6eae90338905995612959273a5c63f93636c14614ac8737d1` | `sora5ｻﾜxﾀ7Vｱ7QFeｷMﾂLﾉﾃﾏﾓﾀTﾚgSav3Wnｱｵ4ｱCKｷﾛMﾘzヰHiﾐｱ6ﾃﾉﾁﾐZmﾇ2fiﾎX21P4L` |
| አልፋ | `0x04` | `0x020146be2154ae86826a3fef0ec0000120ca93ac1705187071d67b83c7ff0efe8108e8ec4530575d7726879333dbdabe7c` | `sora5ｻ9JヱﾈｿuwU6ｴpﾔﾂﾈRqRTds1HﾃﾐｶLVﾍｳ9ﾔhｾNｵVｷyucEﾒGﾈﾏﾍ9sKeﾉDzrｷﾆ742WG1` |
| ኦሜጋ | `0x05` | `0x0201390d946885bc8416b3d30c9d0001206e7a1cdd29b0b78fd13af4c5598feff4ef2a97166e3ca6f2e4fbfccd80505bf1` | `sora5ｻ3zrﾌuﾚﾄJﾑXQhｸTyN8pzwRkWxmjVﾗbﾚﾕヰﾈoｽｦｶtEEﾊﾐ6GPｿﾓﾊｾEhvPｾｻ3XAJ73F` |
| አስተዳደር | `0x06` | `0x0201989eb45a80940d187e2c908f0001208a875fff1eb38451577acd5afee405456568dd7c89e090863a0557bc7af49f17` | `sora5ｻiｵﾁyVﾕｽbFpDHHuﾇﾉdﾗｲﾓﾄRﾋAW3frUCｾ5ｷﾘTwdﾚnｽtQiLﾏｼｶﾅXgｾZmﾒヱH58H4KP` |
| አረጋጋጮች | `0x07` | `0x0201e4ffa58704c69afaeb7cc2d7000120ea4a6c63e29c520abef5507b132ec5f9954776aebebe7b92421eea691446d22c` | `sora5ｻﾀLDH6VYﾑNAｾgﾉVﾜtxﾊRXLｹﾍﾔﾌLd93GﾔGeｴﾄYrs1ﾂHｸkYxｹwｿyZﾗxyﾎZoXT1S4N` |
| አሳሽ | `0x08` | `0x02013b35422c65c2a83c99c523ad0001201398f62c6d1a457c51ba6a4b5f3dbd2f69fca93216218dc8997e416bd17d93ca` | `sora5ｻ4nmｻaﾚﾚPvNLgｿｱv6MHDeEyﾀovﾉJcpvrﾖ6ﾈCQcCNﾇﾜhﾚﾖyFdTwｸｶHEｱ9rWU8FMB` |
| soranet | `0x09` | `0x0201047d9ea7f5d5dbec3f7bfc58000120fd1724385aa0c75b64fb78cd602fa1d991fdebf76b13c58ed702eac835e9f618` | `sora5ｱｸヱVQﾂcﾁヱRﾓcApｲﾁﾅﾒvﾌﾏfｾNnﾛRJsｿDhﾙuHaﾚｺｦﾌﾍﾈeﾆﾎｺN1UUDｶ6ﾎﾄﾛoRH8JUL` |
| ኪትሱኔ | `0x0A` | `0x0201e91933de397fd7723dc9a76c00012043a72e714401762df66b68c26dfbdf2682aaec9f2474eca4613e424a0fbafd3c` | `sora5ｻﾚｺヱkfFJfSﾁｼJwﾉLvbpSｷﾔMWFMrbｳｸｲｲyヰKGJﾉｻ4ｹﾕrｽhｺｽzSDヰXAN62AD7RGNS` |
| ዳ | `0x0B` | `0x02016838cf5bb0ce0f3d4f380e1c00012066be7e332c7a453332bd9d0a7f7db055f5c5ef1a06ada66d98b39fb6810c473a` | `sora5ｻNﾒ5SﾐRﾉﾐﾃ62ｿ1ｶｷWFKyF1BcAﾔvｼﾐHqﾙﾐPﾏｴヰ5tｲﾕvnﾙT6ﾀW7mﾔ7ﾇﾗﾂｳ25CXS93` |

የተገመገመ-በ፡ የውሂብ ሞዴል WG፣ ክሪፕቶግራፊ WG - ለ ADDR-1a የተፈቀደ ወሰን።

##### Sora Nexus ማጣቀሻ ተለዋጭ ስሞች

የሶራ Nexus አውታረ መረቦች ነባሪ ወደ `chain_discriminant = 0x02F1`
(`iroha_config::parameters::defaults::common::CHAIN_DISCRIMINANT`)። የ
`AccountAddress::to_ih58` እና `to_compressed_sora` ረዳቶች ስለዚህ ይለቃሉ
ለእያንዳንዱ ቀኖናዊ ጭነት ወጥነት ያለው የጽሑፍ ቅጾች። የተመረጡ ዕቃዎች ከ
`fixtures/account/address_vectors.json` (በ በኩል የተፈጠረ
`cargo xtask address-vectors`) ለፈጣን ማጣቀሻ ከዚህ በታች ይታያሉ።

| መለያ / መራጭ | IH58 ቀጥተኛ (ቅድመ ቅጥያ `0x02F1`) | Sora compressed (`sora`) በጥሬው |
|-------------------|
| `default` ጎራ (ስውር መራጭ፣ ዘር `0x00`) | `6cmzPVPX5jDQFNfiz6KgmVfm1fhoAqjPhoPFn4nx9mBWaFMyUCwq4cw` | `sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE` (ግልጽ የማዞሪያ ፍንጮች ሲያቀርቡ አማራጭ `@default` ቅጥያ) |
| `treasury` (አካባቢያዊ ዳይጀስት መራጭ፣ ዘር `0x01`) | `34mSYnCXkCzHXm31UDHh7SJfGvC4QPEhwim8z7sys2iHqXpCwCQkjL8KHvkFLSs1vZdJcb37r` | `sora5ｻu6rﾀCヰTGwﾏ1ﾅヱﾌQｲﾖﾇqCｦヰﾓZQCZRDSSﾅMｱﾙヱｹﾁｸ8ｾeﾄﾛ6C8bZuwﾗｹCZｦRSLQFU` |
| ዓለም አቀፍ የመመዝገቢያ ጠቋሚ (`registry_id = 0x0000_002A`, ከ `treasury` ጋር እኩል ነው) | `3oE9sLeRGP49Cu7mQ1nF4wtKAm29BG4TGLiRsaXe7mhbMP5WZ113nNW1N6RbqF` | `sorakXｹ6NｻﾍﾀﾖSﾜﾖｱ3ﾚ5WﾘﾋQﾅｷｦxgﾛｸcﾁｵﾋkﾋvﾏ8SPﾓﾀｹdｴｴｲW9iCM6AEP` |

እነዚህ ሕብረቁምፊዎች በCLI (`iroha tools address convert`)፣ Torii ከሚለቀቁት ጋር ይዛመዳሉ።
ምላሾች (`address_format=ih58|compressed`)፣ እና ኤስዲኬ አጋዥ፣ ስለዚህ UX ቅዳ/ለጥፍ
ፍሰቶች በቃላቸው ሊተማመኑባቸው ይችላሉ. ግልጽ የሆነ የማዞሪያ ፍንጭ ሲፈልጉ ብቻ `<address>@<domain>` አክል; ቅጥያው የቀኖናዊው ውጤት አካል አይደለም.

#### 2.6 የጽሑፍ ተለዋጭ ስሞች ለተግባራዊነት (የታቀደ)

- ** ሰንሰለት-ተለዋዋጭ ዘይቤ: *** `ih:<chain-alias>:<alias@domain>` ለሎግ እና ለሰው
  መግቢያ. የኪስ ቦርሳዎች ቅድመ ቅጥያውን መተንተን፣ የተከተተውን ሰንሰለት ማረጋገጥ እና ማገድ አለባቸው
  አለመመጣጠን።
- ** CAIP-10 ቅጽ: ** `iroha:<caip-2-id>:<ih58-addr>` ለ ሰንሰለት-አግኖስቲክ
  ውህደቶች. ይህ የካርታ ስራ ** እስካሁን አልተተገበረም *** በተላከው ውስጥ
  የመሳሪያ ሰንሰለቶች.
- ** የማሽን ረዳቶች፡** ለ Rust፣ TypeScript/JavaScript፣ Python፣ ኮዴኮችን ያትሙ
  እና ኮትሊን IH58 እና የታመቁ ቅርጸቶችን (`AccountAddress::to_ih58`፣
  `AccountAddress::parse_encoded`፣ እና የ SDK አቻዎቻቸው)። CAIP-10 ረዳቶች ናቸው
  ወደፊት ሥራ.

#### 2.7 Deterministic IH58 ተለዋጭ ስም

- ** ቅድመ ቅጥያ ካርታ፡** `chain_discriminant` እንደ IH58 አውታረ መረብ ቅድመ ቅጥያ እንደገና ይጠቀሙ።
  `encode_ih58_prefix()` (`crates/iroha_data_model/src/account/address.rs` ይመልከቱ)
  6-ቢት ቅድመ ቅጥያ (ነጠላ ባይት) ለዋጋዎች `<64` እና ባለ 14-ቢት፣ ባለሁለት ባይት ያወጣል።
  ለትላልቅ አውታረ መረቦች ቅፅ. የስልጣን ስራዎች በ ውስጥ ይኖራሉ
  [`address_prefix_registry.md`] (source/references/address_prefix_registry.md);
  ግጭቶችን ለማስወገድ ኤስዲኬዎች ተዛማጅ የሆነውን የJSON መዝገብ ማመሳሰል አለባቸው።
- ** የመለያ ቁሳቁስ፡** IH58 የተገነባውን ቀኖናዊ የክፍያ ጭነት ያሳያል
  `AccountAddress::canonical_bytes()`—ራስጌ ባይት፣ ጎራ መራጭ እና
  የመቆጣጠሪያ ጭነት. ምንም ተጨማሪ የሃሺንግ እርምጃ የለም; IH58 ን ያካትታል
  የሁለትዮሽ መቆጣጠሪያ ክፍያ (ነጠላ ቁልፍ ወይም መልቲሲግ) በሩስት እንደተመረተ
  ኢንኮደር፣ ለብዙ ሲግ ፖሊሲ መፍለቂያዎች ጥቅም ላይ የዋለው CTAP2 ካርታ አይደለም።
** ኢንኮዲንግ:** I18NI0000363X ቅድመ ቅጥያ ባይት ከቀኖናዊው ጋር ያገናኛል
  ጫን እና ከ Blake2b-512 የተገኘ ባለ 16-ቢት ቼክ ከቋሚው ጋር ጨምሯል።
  ቅድመ ቅጥያ `IH58PRE` (`b"IH58PRE" || prefix || payload`)። ውጤቱ በ `bs58` በኩል Base58-encoded ነው።
  የ CLI/SDK ረዳቶች ተመሳሳይ አሰራርን እና `AccountAddress::parse_encoded` ያጋልጣሉ
  በ `decode_ih58` በኩል ይገለበጣል.

#### 2.8 መደበኛ የጽሑፍ ሙከራ ቬክተሮች

`fixtures/account/address_vectors.json` ሙሉ IH58 (የተመረጠ) እና የታመቀ (`sora`፣ ሁለተኛ-ምርጥ) ይዟል።
ለእያንዳንዱ ቀኖናዊ ክፍያ። ዋና ዋና ዜናዎች

- **`addr-single-default-ed25519` (ሶራ I18NT0000016X፣ ቅድመ ቅጥያ `0x02F1`)**  
  IH58 `6cmzPVPX5jDQFNfiz6KgmVfm1fhoAqjPhoPFn4nx9mBWaFMyUCwq4cw`፣ የታመቀ (`sora`)
  `sora2QG…U4N5E5`. Torii እነዚህን ትክክለኛ ገመዶች ከ `AccountId` ያወጣል
  `Display` ትግበራ (ቀኖናዊ IH58) እና `AccountAddress::to_compressed_sora`.
- **`addr-global-registry-002a` (የመዝገብ መራጭ → ግምጃ ቤት)**  
  IH58 `3oE9sLeRGP49Cu7mQ1nF4wtKAm29BG4TGLiRsaXe7mhbMP5WZ113nNW1N6RbqF`፣ የታመቀ (`sora`)
  `sorakX…CM6AEP`. የመመዝገቢያ መራጮች አሁንም ኮድ መፍታት እንደሚችሉ ያሳያል
  ተመሳሳይ ቀኖናዊ ጭነት ልክ እንደ ተጓዳኝ የአካባቢያዊ መፍጨት።
- ** የመውደቅ ጉዳይ (`ih58-prefix-mismatch`)**  
  በመስቀለኛ መንገድ ላይ ከቅድመ ቅጥያ `NETWORK_PREFIX + 1` ጋር የተቀመጠ IH58 በጥሬው በመተንተን ላይ
  ነባሪ ቅድመ ቅጥያ ውጤቶችን በመጠባበቅ ላይ
  `AccountAddressError::UnexpectedNetworkPrefix { expected: 753, found: 754 }`
  የጎራ ማዘዋወር ከመሞከርዎ በፊት። የ `ih58-checksum-mismatch` እቃው
  በBlake2b checksum ላይ ፈልጎ ማግኘትን ይለማመዳል።

#### 2.9 ተገዢነት እቃዎች

ADDR-2 አወንታዊ እና አሉታዊ የሚሸፍን ድጋሚ ሊጫወት የሚችል ጥቅል ይልካል
በቀኖናዊ ሄክስ ላይ ያሉ ሁኔታዎች፣ IH58 (ተመራጭ)፣ የታመቀ (`sora`፣ ግማሽ/ሙሉ ስፋት)፣ ስውር
ነባሪ መራጮች፣ የአለምአቀፍ መዝገብ ቤት ተለዋጭ ስሞች እና ባለብዙ ፊርማ ተቆጣጣሪዎች። የ
ቀኖናዊ JSON በ`fixtures/account/address_vectors.json` ይኖራል እና ሊሆን ይችላል።
በ:

```
cargo xtask address-vectors --out fixtures/account/address_vectors.json
# verify without writing:
cargo xtask address-vectors --verify
```

ለአድሆክ ሙከራዎች (የተለያዩ መንገዶች/ቅርጸቶች) ምሳሌው ሁለትዮሽ አሁንም ነው።
ይገኛል፡

```
cargo run -p iroha_data_model --example account_address_vectors > fixtures/account/address_vectors.json
```

`crates/iroha_data_model/tests/account_address_vectors.rs` ውስጥ ዝገት ክፍል ሙከራዎች
እና `crates/iroha_torii/tests/account_address_vectors.rs`፣ ከ JS ጋር፣
ስዊፍት እና አንድሮይድ ልጓሞች (`javascript/iroha_js/test/address.test.js`፣
`IrohaSwift/Tests/IrohaSwiftTests/AccountAddressTests.swift`፣
`java/iroha_android/src/test/java/org/hyperledger/iroha/android/address/AccountAddressTests.java`)፣
በኤስዲኬዎች እና Torii መግቢያ ላይ የኮዴክን እኩልነት ለማረጋገጥ ተመሳሳይ መሣሪያን ይጠቀሙ።

### 3. በአለምአቀፍ ደረጃ ልዩ የሆኑ ጎራዎች እና መደበኛነት

በተጨማሪ ይመልከቱ፡ [`docs/source/references/address_norm_v1.md`](source/references/address_norm_v1.md)
በመላው Torii፣ የውሂብ ሞዴል እና ኤስዲኬዎች ላይ ጥቅም ላይ ለሚውለው ቀኖናዊ Norm v1 የቧንቧ መስመር።

`DomainId`ን እንደ መለያ የተሰጠበት tuple እንደገና ይግለጹ፡

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

`LocalChain` አሁን ባለው ሰንሰለት የሚተዳደሩትን የጎራ ስም ይጠቀልላል።
አንድ ጎራ በአለምአቀፍ መዝገብ ቤት ሲመዘገብ በባለቤትነት እንቀጥላለን
ሰንሰለት አድልዎ። ማሳያ/ መተንተን ለአሁን ሳይለወጥ ይቆያል፣ ግን የ
የተዘረጋው መዋቅር የመተላለፊያ ውሳኔዎችን ይፈቅዳል.

#### 3.1 መደበኛነት እና ማፈንገጥ መከላከያዎች

Norm v1 እያንዳንዱ አካል ከጎራ በፊት መጠቀም ያለበትን ቀኖናዊ የቧንቧ መስመር ይገልጻል
ስም በ `AccountAddress` ውስጥ የቀጠለ ወይም የተካተተ ነው። ሙሉው የእግር ጉዞ
በ [`docs/source/references/address_norm_v1.md`] (source/references/address_norm_v1.md) ይኖራል;
ከታች ያለው ማጠቃለያ የኪስ ቦርሳዎችን፣ Toriiን፣ ኤስዲኬዎችን እና አስተዳደርን ይይዛል።
መሳሪያዎች መተግበር አለባቸው.

1. **የግቤት ማረጋገጫ።** ባዶ ገመዶችን፣ ነጭ ቦታን እና የተያዘውን ውድቅ አድርግ
   ገደቦች `@`, `#`, `$`. ይህ በ ተፈጻሚነት ካለው ተለዋዋጮች ጋር ይዛመዳል
   `Name::validate_str`.
2. ** የዩኒኮድ NFC ቅንብር።** በICU የተደገፈ የNFC መደበኛ አሰራርን ቀኖናዊ በሆነ መልኩ ይተግብሩ።
   ተመጣጣኝ ቅደም ተከተሎች በቆራጥነት ይወድቃሉ (ለምሳሌ፣ `e\u{0301}` → `é`)።
3. **UTS-46 መደበኛ ማድረግ።** የNFC ውጤቱን በ UTS-46 በኩል ያሂዱ።
   `use_std3_ascii_rules = true`፣ `transitional_processing = false`፣ እና
   የዲ ኤን ኤስ ርዝመት ማስፈጸሚያ ነቅቷል። ውጤቱ ዝቅተኛ-A-መለያ ቅደም ተከተል ነው;
   የSTD3 ደንቦችን የሚጥሱ ግብዓቶች እዚህ አይሳኩም።
4. **የርዝመት ገደቦች** የዲ ኤን ኤስ አይነት ወሰኖችን ያስፈጽሙ፡ እያንዳንዱ መለያ ከ1-63 መሆን አለበት
   ባይት እና ሙሉ ጎራ ከደረጃ 3 በኋላ ከ255 ባይት መብለጥ የለባቸውም።
5. **አማራጭ ግራ የሚያጋባ ፖሊሲ።** UTS-39 ስክሪፕት ቼኮች ክትትል ይደረግባቸዋል።
   መደበኛ v2; ኦፕሬተሮች ቀደም ብለው ሊነቁዋቸው ይችላሉ፣ ነገር ግን ቼኩ አለመሳካቱ ማቋረጥ አለበት።
   ማቀነባበር.

እያንዳንዱ ደረጃ ከተሳካ፣ የታችኛው ፊደል A-መለያ ሕብረቁምፊ ተሸፍኗል እና ጥቅም ላይ ይውላል
አድራሻ ኢንኮዲንግ፣ ውቅረት፣ መግለጫዎች እና የመመዝገቢያ ፍለጋዎች። የአካባቢ መፍጨት
መራጮች ባለ 12-ባይት እሴታቸውን `blake2s_mac(ቁልፍ = "SORA-LOCAL-K:v1",
ቀኖናዊ_ላብል)[0..12]` ደረጃ 3 ውፅዓትን በመጠቀም። ሁሉም ሌሎች ሙከራዎች (ድብልቅ)
መያዣ፣ አቢይ ሆሄ፣ ጥሬ የዩኒኮድ ግቤት) ከተዋቀረ ውድቅ ተደርጓል
`ParseError`s ስሙ በቀረበበት ወሰን።

እነዚህን ደንቦች የሚያሳዩ ቀኖናዊ መጫዎቻዎች - የ punycode ዙር ጉዞዎችን ጨምሮ
እና ልክ ያልሆኑ የSTD3 ቅደም ተከተሎች - በ ውስጥ ተዘርዝረዋል።
`docs/source/references/address_norm_v1.md` እና በኤስዲኬ CI ውስጥ ተንጸባርቀዋል
የቬክተር ስብስቦች በ ADDR-2 ስር ይከተላሉ።

### 4. Nexus የጎራ መዝገብ ቤት እና ማዘዋወር- ** የመመዝገቢያ እቅድ: *** Nexus የተፈረመ ካርታ `DomainName -> ChainRecord` ይይዛል
  `ChainRecord` ሰንሰለት አድሎአዊ፣ አማራጭ ሜታዳታ (RPC) የሚያጠቃልልበት
  የመጨረሻ ነጥቦች) እና የስልጣን ማረጋገጫ (ለምሳሌ የአስተዳደር ባለብዙ ፊርማ)።
- ** የማመሳሰል ዘዴ: ***
  - ሰንሰለቶች የተፈረመ የጎራ ይገባኛል ጥያቄ ለNexus ያስገባሉ (በዘፍጥረት ጊዜ ወይም በ
    የአስተዳደር መመሪያ).
  - Nexus ወቅታዊ መግለጫዎችን ያትማል (የተፈረመ JSON እና አማራጭ Merkle root)
    በኤችቲቲፒኤስ እና በይዘት አድራሻ ማከማቻ (ለምሳሌ፣ IPFS)። ደንበኞች ይሰኩት
    የቅርብ ጊዜ አንጸባራቂ እና ፊርማዎችን ያረጋግጡ።
- ** የፍተሻ ፍሰት: ***
  - Torii `DomainId` የሚያመለክት ግብይት ይቀበላል።
  - ጎራው በአካባቢው የማይታወቅ ከሆነ፣ Torii የተሸጎጠውን Nexus ማኒፌክትን ይጠይቃል።
  - አንጸባራቂው የውጭ ሰንሰለትን የሚያመለክት ከሆነ ግብይቱ ውድቅ ተደርጓል
    የሚወስን `ForeignDomain` ስህተት እና የርቀት ሰንሰለት መረጃ።
  - ጎራ ከ Nexus ከጠፋ፣ Torii `UnknownDomain` ይመልሳል።
- ** የእምነት መልህቆች እና ማሽከርከር: ** የአስተዳደር ቁልፎች ምልክት ይገለጣል; ማሽከርከር ወይም
  መሻር እንደ አዲስ አንጸባራቂ ግቤት ታትሟል። ደንበኞች አንጸባራቂን ያስገድዳሉ
  ቲቲኤሎች (ለምሳሌ፡ 24 ሰ) እና ከዚያ መስኮት ባሻገር የቆየ ውሂብን ለማማከር እምቢ ይላሉ።
- ** የብልሽት ሁነታዎች፡** አንጸባራቂ ሰርስሮ ማውጣት ካልተሳካ፣ Torii ወደ መሸጎጫ ይመለሳል።
  በቲቲኤል ውስጥ ያለ መረጃ; ያለፈው ቲቲኤል `RegistryUnavailable` ያወጣል እና እምቢ ይላል።
  ወጥነት የሌለውን ሁኔታ ለማስወገድ ጎራ ተሻጋሪ መስመር።

### 4.1 የመዝገብ ቤት አለመቀየር፣ ተለዋጭ ስሞች እና የመቃብር ድንጋዮች (ADDR-7c)

Nexus **አባሪ-ብቻ አንጸባራቂ** ያትማል ስለዚህ እያንዳንዱ ጎራ ወይም ተለዋጭ ስም
ኦዲት ተደርጎ ሊደገም ይችላል። ኦፕሬተሮች በ ውስጥ የተገለጸውን ጥቅል ማከም አለባቸው
[የአድራሻ አንጸባራቂ runbook](source/runbooks/address_manifest_ops.md) እንደ
ብቸኛ የእውነት ምንጭ፡ የሰነድ ማስረጃው ከጠፋ ወይም ማረጋገጥ ካልተሳካ፣ Torii አለበት
የተጎዳውን ጎራ ለመፍታት እምቢ ማለት.

ራስ-ሰር ድጋፍ: `cargo xtask address-manifest verify --bundle <current_dir> --previous <previous_dir>`
በ ውስጥ የተጻፉትን ቼክሱም፣ ሼማ እና የቀድሞ መፈጨት ቼኮችን እንደገና ያዘጋጃል።
runbook. `sequence` ን ለማሳየት በለውጥ ቲኬቶች ውስጥ የትዕዛዙን ውጤት ያካትቱ
እና `previous_digest` ትስስር ጥቅሉን ከማተም በፊት የተረጋገጠ ነው።

#### ራስጌ እና ፊርማ ውልን ያሳያል

| መስክ | መስፈርት |
|-------|-----------|
| `version` | በአሁኑ ጊዜ `1`. ከተዛማጅ ዝርዝር ዝማኔ ጋር ብቻ ያብቡ። |
| `sequence` | በእያንዳንዱ እትም በ ** በትክክል *** አንድ ጭማሪ። Torii መሸጎጫዎች ከክፍተቶች ወይም ከተሃድሶዎች ጋር ክለሳዎችን ውድቅ ያደርጋሉ። |
| `generated_ms` + `ttl_hours` | የመሸጎጫ ትኩስነት (ነባሪ 24 ሰ) ያዘጋጁ። TTL ከሚቀጥለው ህትመት በፊት ጊዜው ካለፈ፣ Torii ወደ `RegistryUnavailable` ይገለብጣል። |
| `previous_digest` | የቀደመው አንጸባራቂ አካል BLAKE3 መፍጨት (ሄክስ)። የማይለወጥ መሆኑን ለማረጋገጥ አረጋጋጮች በ`b3sum` እንደገና ያሰላሉ። |
| `signatures` | መግለጫዎች የተፈረሙት በI18NT0000001X (`cosign sign-blob`) ነው። ኦፕስ `cosign verify-blob --bundle manifest.sigstore manifest.json` ን ማስኬድ እና የአስተዳደር ማንነት/አውጪ ገደቦችን ከመልቀቁ በፊት መተግበር አለበት። |

የመልቀቂያው አውቶማቲክ `manifest.sigstore` እና `checksums.sha256` ያወጣል
ከJSON አካል ጋር። ወደ SoraFS ሲያንጸባርቁ ፋይሎቹን አንድ ላይ ያቆዩ
ኦዲተሮች የማረጋገጫ እርምጃዎችን በቃላት እንዲጫወቱ የኤችቲቲፒ የመጨረሻ ነጥቦች።

#### የመግቢያ ዓይነቶች

| አይነት | ዓላማ | አስፈላጊ መስኮች |
|------|---------|-----------------|
| `global_domain` | አንድ ጎራ በአለምአቀፍ ደረጃ እንደተመዘገበ እና በሰንሰለት አድሎአዊ እና IH58 ቅድመ ቅጥያ ላይ ካርታ ማድረግ እንዳለበት ያውጃል። | `{ "domain": "<label>", "chain": "sora:nexus:global", "ih58_prefix": 753, "selector": "global" }` |
| `tombstone` | ተለዋጭ ስም/መራጭ በቋሚነት ጡረታ ይወጣል። የአካባቢ-8 ውህዶችን ሲሰርዝ ወይም ጎራ ሲያስወግድ ያስፈልጋል። | `{ "selector": {…}, "reason_code": "LOCAL8_RETIREMENT" \| …, "ticket": "<governance id>", "replaces_sequence": <number> }` |

የ`global_domain` ግቤቶች እንደ አማራጭ `manifest_url` ወይም I18NI0000437X ሊያካትቱ ይችላሉ።
በተፈረመበት ሰንሰለት ሜታዳታ ላይ የኪስ ቦርሳዎችን ለመጠቆም፣ ነገር ግን ቀኖናዊው ቱፕል ይቀራል
`{domain, chain, discriminant/ih58_prefix}`. `tombstone` መዝገቦች ** መጠቀስ አለባቸው
መራጩ ጡረታ እየወጣ እና የተፈቀደለት ትኬት/የመንግስት ቅርስ
ለውጡ ስለዚህ የኦዲት ዱካ ከመስመር ውጭ እንደገና ሊገነባ ይችላል።

#### ተለዋጭ ስም/የመቃብር ድንጋይ የስራ ፍሰት እና ቴሌሜትሪ

1. ** ተንሸራታች እወቅ።** `torii_address_local8_total{endpoint}` ይጠቀሙ፣
   `torii_address_local8_domain_total{endpoint,domain}`፣
   `torii_address_collision_total{endpoint,kind="local12_digest"}`፣
   `torii_address_collision_domain_total{endpoint,domain}`፣
   `torii_address_domain_total{endpoint,domain_kind}`፣ እና
   `torii_address_invalid_total{endpoint,reason}` (የተሰራው በ
   `dashboards/grafana/address_ingest.json`) የአካባቢ ማስገባቶችን ለማረጋገጥ እና
   የአካባቢ-12 ግጭቶች የመቃብር ድንጋይ ከማቅረባቸው በፊት ዜሮ ላይ ይቆያሉ. የ
   በየጎራ ቆጣሪዎች ባለቤቶች የአካባቢ-8 ዲቪ/ሙከራ ጎራዎች ብቻ እንደሚለቁ እንዲያረጋግጡ ያስችላቸዋል
   ትራፊክ (እና የአካባቢ-12 ግጭቶች ካርታ ከሚታወቁ የማዘጋጃ ጎራዎች ጋር) እያለ
   SREs ምን ያህል እንደሆነ ግራፍ እንዲያደርጉ **የጎራ ዓይነት ቅይጥ (5m)** ፓኔል ያካትታል
   `domain_kind="local12"` ትራፊክ ይቀራል፣ እና `AddressLocal12Traffic`
   ምንም እንኳን ምርት አሁንም የአካባቢ-12 መራጮችን ባየ ቁጥር እሳትን ያስጠነቅቃል
   የጡረታ በር.
2. ** ቀኖናዊ የምግብ መፈጨትን ያውጡ።** ሩጫ
   `iroha tools address convert <address> --format json --expect-prefix 753`
   (ወይም `fixtures/account/address_vectors.json` በ በኩል ይጠቀሙ
   `scripts/account_fixture_helper.py`) ትክክለኛውን `digest_hex` ለመያዝ።
   CLI IH58፣ `sora…` እና ቀኖናዊ `0x…` ቃል በቃል ይቀበላል። አባሪ
   `@<domain>` ለመገለጥ መለያ ማቆየት ሲፈልጉ ብቻ።
   የJSON ማጠቃለያ በ`input_domain` መስክ በኩል ጎራ አለው፣ እና
   `legacy  suffix` የተለወጠውን ኮድ እንደ `<address>@<domain>` ለ
   አንጸባራቂ ልዩነቶች (ይህ ቅጥያ ሜታዳታ እንጂ ቀኖናዊ መለያ መታወቂያ አይደለም)።
   ለአዲስ መስመር ተኮር ወደ ውጭ መላክ ይጠቀሙ
   `iroha tools address normalize --input <file> legacy-selector input mode` ወደ ጅምላ-አከባቢ ለመቀየር
   መራጮች ወደ ቀኖናዊ IH58 (የተመረጡ)፣ የተጨመቁ (`sora`፣ ሁለተኛ-ምርጥ)፣ ሄክስ፣ ወይም JSON ቅጾች እየዘለሉ
   አካባቢያዊ ያልሆኑ ረድፎች. ኦዲተሮች የተመን ሉህ ተስማሚ የሆነ ማስረጃ ሲፈልጉ ሩጡ
   የCSV ማጠቃለያ ለመልቀቅ `iroha tools address audit --input <file> --format csv`
   (`input,status,format,domain_kind,…`) የአካባቢ መራጮችን የሚያጎላ፣
   ቀኖናዊ ኢንኮዲንግ፣ እና ውድቀቶችን በተመሳሳዩ ፋይል ውስጥ መተንተን።
3. **የማስረጃ ምዝግቦችን አባሪ።** የ`tombstone` መዝገብ (እና ተከታዩን) አርቅ
   `global_domain` ወደ አለምአቀፍ መዝገብ ሲሰደድ መዝገብ) እና አረጋግጥ
   ፊርማ ከመጠየቅዎ በፊት አንጸባራቂው ከ `cargo xtask address-vectors` ጋር።
4. ** አረጋግጥ እና አትም
   ተከታታይ ነጠላነት) ጥቅሉን ወደ SoraFS ከማንጸባረቅዎ በፊት። Torii አሁን
   ከጥቅል መሬቶች በኋላ ወዲያውኑ IH58 (ተመራጭ)/ሶራ (ሁለተኛ-ምርጥ) ቃል በቃል ያደርጋል።
5. ** ይከታተሉ እና ይመለሱ።** የአካባቢ-8 እና አካባቢያዊ-12 የግጭት ፓነሎችን በ ላይ ያስቀምጡ።
   ለ 30 ቀናት ዜሮ; ድግግሞሾች ከታዩ የቀደመውን አንጸባራቂ እንደገና ያትሙ
   ቴሌሜትሪ እስኪረጋጋ ድረስ በተጎዳው የማይመረት አካባቢ ውስጥ ብቻ።

ከላይ ያሉት ሁሉም እርምጃዎች ለ ADDR-7c አስገዳጅ ማስረጃዎች ናቸው፡ ያለ ይገለጣል
የ`cosign` ፊርማ ጥቅል ወይም ከ `previous_digest` እሴቶች ጋር ሳይዛመድ መሆን አለበት
በራስ-ሰር ውድቅ ይደረጋል እና ኦፕሬተሮች የማረጋገጫ ምዝግብ ማስታወሻዎችን ማያያዝ አለባቸው
የእነሱ ለውጥ ቲኬቶች.

### 5. Wallet & API ergonomics

- ** የማሳያ ነባሪዎች፡** የኪስ ቦርሳዎች የIH58 አድራሻን ያሳያሉ (አጭር፣ ቼክ የተጠቃለለ)
  ከመዝገቡ የተገኘ መለያ እና የተፈታው ጎራ። ጎራዎች ናቸው።
  ሊለወጥ የሚችል ገላጭ ሜታዳታ በግልጽ ምልክት ተደርጎበታል፣ IH58 ግን የ
  የተረጋጋ አድራሻ.
- ** የግቤት ቀኖናዊነት፡** Torii እና ኤስዲኬዎች IH58 (የተመረጡ)/ሶራ (ሁለተኛ-ምርጥ)/0x ይቀበላሉ
  አድራሻዎች ሲደመር `alias@domain`፣ `uaid:…`፣ እና
  `opaque:…` ቅጾች፣ከዚያም ለውጤት ወደ IH58 ቀኖናዊ አድርግ። የለም
  ጥብቅ ሁነታ መቀያየር; ጥሬ ስልክ/ኢሜል መለያዎች ከመዝገቡ ውጭ መቀመጥ አለባቸው
  በ UAID / ግልጽ ያልሆነ ካርታዎች.
- **ስህተት መከላከል፡** የኪስ ቦርሳዎች የIH58 ቅድመ ቅጥያዎችን ይተነትኑ እና ሰንሰለትን አድልዎ ያስገድዳሉ
  የሚጠበቁ. የሰንሰለት አለመዛመዶች በድርጊት በሚደረጉ ምርመራዎች ከባድ ውድቀቶችን ያስነሳሉ።
- ** የኮድ ቤተ-ፍርግሞች: *** ኦፊሴላዊ ዝገት ፣ ታይፕ ስክሪፕት / ጃቫ ስክሪፕት ፣ Python እና ኮትሊን
  ቤተ-መጻሕፍት IH58 ኢንኮዲንግ/መግለጫ እና የታመቀ (`sora`) ድጋፍ ይሰጣሉ ለ
  የተበታተኑ አተገባበርን ያስወግዱ. የCAIP-10 ልወጣዎች ገና አልተላኩም።

#### ተደራሽነት እና ደህንነቱ የተጠበቀ የመጋሪያ መመሪያ- ለምርት ወለል የትግበራ መመሪያ በቀጥታ ክትትል ይደረግበታል።
  `docs/portal/docs/reference/address-safety.md`; መቼ ያንን የማረጋገጫ ዝርዝር ያመልክቱ
  እነዚህን መስፈርቶች ከኪስ ቦርሳ ወይም ከአሳሽ ዩኤክስ ጋር ማስተካከል።
- **ደህንነቱ የተጠበቀ መጋራት ይፈስሳል፡** አድራሻዎችን በነባሪነት ወደ IH58 ቅፅ የሚገለብጡ ወይም የሚያሳዩ እና ከጎን ያለውን የ"share" ድርጊት የሚያጋልጡ ሲሆን ይህም ሙሉውን ሕብረቁምፊ እና ከተመሳሳይ ጭነት የተገኘ QR ኮድ ተጠቃሚዎች ቼኩን በምስል ወይም በመቃኘት ማረጋገጥ ይችላሉ። መቆራረጥ ማስቀረት በማይቻልበት ጊዜ (ለምሳሌ፣ ትናንሽ ስክሪኖች)፣ የሕብረቁምፊውን መጀመሪያ እና መጨረሻ ያቆዩት፣ ግልጽ የሆኑ ሞላላዎችን ያክሉ፣ እና በአጋጣሚ መቆራረጥን ለመከላከል ሙሉ አድራሻውን ከቅጂ ወደ ቅንጥብ ሰሌዳው ተደራሽ ያድርጉት።
- **የIME መከላከያዎች፡** የአድራሻ ግብዓቶች ከIME/IME-style የቁልፍ ሰሌዳዎች የተቀናበሩ ቅርሶችን አለመቀበል አለባቸው። የASCII-ብቻ ግቤትን ያስገድዱ፣ ሙሉ ስፋት ወይም የቃና ቁምፊዎች ሲገኙ የመስመር ውስጥ ማስጠንቀቂያ ያቅርቡ፣ እና የጃፓን እና ቻይናውያን ተጠቃሚዎች መሻሻል ሳያጡ አይኤምኢቸውን እንዲያሰናክሉ ከማፅደቁ በፊት ምልክቶችን የሚያጣምር ግልጽ-ጽሁፍ ለጥፍ ዞን ያቅርቡ።
- **የስክሪን አንባቢ ድጋፍ፡** ግንባር ቀደም Base58 ቅድመ-ቅጥያ አሃዞችን የሚገልጹ እና የIH58 ጭነት ጭነትን በ4- ወይም 8-ቁምፊ ቡድኖች የሚቆርጡ በምስላዊ የተደበቁ መለያዎችን (`aria-label`/I18NI0000475X) ያቅርቡ፣ ስለዚህ አጋዥ ቴክኖሎጂ ከሩጫ ይልቅ የተሰባሰቡ ቁምፊዎችን ያነባል። በትህትና የቀጥታ ስርጭት ክልሎች ስኬትን መቅዳት/ያጋሩ እና የQR ቅድመ-እይታዎች ገላጭ alt ጽሑፍ ("IH58 አድራሻ ለ <alias> በሰንሰለት 0x02F1 ላይ") ማካተቱን ያረጋግጡ።
- **የሶራ-ብቻ የተጨመቀ አጠቃቀም፡** ሁልጊዜ የ`sora…` የታመቀ እይታን “ሶራ-ብቻ” ብለው ይሰይሙት እና ከመቅዳትዎ በፊት ከግልጽ ማረጋገጫ ጀርባ ያድርጉት። ሰንሰለት አድልዎ የሶራ Nexus እሴት ካልሆነ ኤስዲኬዎች እና የኪስ ቦርሳዎች የታመቀ ውፅዓት ለማሳየት እምቢ ማለት አለባቸው እና ገንዘብ ማዛባትን ለማስቀረት ተጠቃሚዎችን ወደ IH58 ኢንተር ኔት ማስተላለፎች መመለስ አለባቸው።

## የትግበራ ማረጋገጫ ዝርዝር

- **IH58 ፖስታ:** ቅድመ ቅጥያ `chain_discriminant` ኮምፓክትን በመጠቀም ይደብቃል
  6-/14-ቢት እቅድ ከ`encode_ih58_prefix()`፣ አካሉ ቀኖናዊ ባይት ነው
  (`AccountAddress::canonical_bytes()`)፣ እና ቼክሱሙ የመጀመሪያዎቹ ሁለት ባይት ነው።
  የ Blake2b-512(`b"IH58PRE"` || ቅድመ ቅጥያ || አካል)። ሙሉ ክፍያው Base58- ነው.
  በ `bs58` የተመዘገበ።
- ** የመመዝገቢያ ውል:** የተፈረመ JSON (እና አማራጭ Merkle root) ማተም
  `{discriminant, ih58_prefix, chain_alias, endpoints}` በ24h TTL እና
  የማዞሪያ ቁልፎች.
- ** የጎራ ፖሊሲ:** ASCII `Name` ዛሬ; i18n ን ከነቃ፣ UTS-46ን ይተግብሩ
  መደበኛነት እና UTS-39 ግራ ለሚጋቡ ቼኮች። ከፍተኛውን መለያ (63) ያስፈጽሙ እና
  ጠቅላላ (255) ርዝማኔዎች.
- ** የጽሑፍ አጋዥዎች: *** IH58 መርከብ ↔ የተጨመቁ (`sora…`) ኮዴኮች በዝገት ውስጥ ፣
  ታይፕ ስክሪፕት/ጃቫ ስክሪፕት፣ Python እና Kotlin ከጋራ የሙከራ ቬክተሮች ጋር (CAIP-10
  ካርታዎች ወደፊት ሥራ ይቀራሉ).
- ** CLI መሳሪያ:** በ`iroha tools address convert` በኩል የሚወስን ኦፕሬተር የስራ ፍሰት ያቅርቡ
  (`crates/iroha_cli/src/address.rs` ይመልከቱ)፣ እሱም IH58/`sora…`/`0x…` ቃል በቃል እና ይቀበላል።
  አማራጭ `<address>@<domain>` መለያዎች፣የሶራ Nexus ቅድመ ቅጥያ (`753`) በመጠቀም የIH58 ውፅዓት ነባሪዎች
  እና የሶራ-ብቻ የታመቀ ፊደል ኦፕሬተሮች በግልጽ ሲጠይቁት ብቻ ነው የሚያወጣው
  `--format compressed` ወይም የJSON ማጠቃለያ ሁነታ። ትዕዛዙ ቅድመ ቅጥያ የሚጠበቁትን ያስፈጽማል
  መተንተን፣ የቀረበውን ጎራ (`input_domain` በJSON) እና የ`legacy  suffix` ባንዲራ ይመዘግባል።
  የተቀየረውን ኢንኮዲንግ እንደ `<address>@<domain>` ይደግማል ስለዚህ አንጸባራቂ ልዩነቶች ergonomic ይቀራሉ።
- ** የኪስ ቦርሳ/አሳሽ UX፡** [የአድራሻ ማሳያ መመሪያዎችን](source/sns/address_display_guidelines.md) ተከተል።
  በADDR-6 ተልኳል—ሁለት ቅጂ አዝራሮችን ያቅርቡ፣ IH58ን እንደ QR ክፍያ ያቆዩ እና ያስጠነቅቁ
  ተጠቃሚዎች የታመቀው I18NI0000495X ቅጽ ለሶራ-ብቻ እና ለአይኤምኢ ዳግም መፃፍ የተጋለጠ ነው።
- ** Torii ውህደት:** መሸጎጫ Nexus TTLን በማክበር ያሳያል
  `ForeignDomain`/`UnknownDomain`/`RegistryUnavailable`፣ እና
  keep account-literal parsing encoded-only (`IH58` preferred, `sora…`
  compressed accepted) with canonical IH58 output.

### Torii ምላሽ ቅርጸቶች

- `GET /v1/accounts` አማራጭ `address_format` መጠይቅ መለኪያ ይቀበላል እና
  `POST /v1/accounts/query` በJSON ፖስታ ውስጥ ያለውን ተመሳሳይ መስክ ይቀበላል።
  የሚደገፉ እሴቶች የሚከተሉት ናቸው
  - `ih58` (ነባሪ) - ምላሾች ቀኖናዊ IH58 Base58 ጭነትን ያመነጫሉ (ለምሳሌ፣
    `6cmzPVPX5jDQFNfiz6KgmVfm1fhoAqjPhoPFn4nx9mBWaFMyUCwq4cw`)።
  - `compressed` - ምላሾች የሶራ-ብቻ `sora…` የታመቀ እይታን ሲያወጡ
    ማጣሪያዎችን/የመንገድ መለኪያዎችን ቀኖናዊ ማድረግ።
- ልክ ያልሆኑ እሴቶች `400` (`QueryExecutionFail::Conversion`) ይመለሳሉ። ይህ ይፈቅዳል
  ቦርሳዎች እና አሳሾች ለሶራ-ብቻ ዩኤክስ የታመቁ ሕብረቁምፊዎች በሚጠይቁበት ጊዜ
  IH58 እንደ መስተጋብር ነባሪ ማቆየት።
- የንብረት ባለቤት ዝርዝሮች (`GET /v1/assets/{definition_id}/holders`) እና የእነሱ JSON
  የኤንቨሎፕ ተጓዳኝ (`POST …/holders/query`) እንዲሁም `address_format` ያከብራል።
  የ `items[*].account_id` መስክ በማንኛውም ጊዜ የተጨመቁ ቀጥተኛ ቃላትን ያወጣል።
  መለኪያ/የኤንቨሎፕ መስኩ መለያዎቹን በማንጸባረቅ ወደ `compressed` ተቀናብሯል
  አሳሾች ወጥ የሆነ ውፅዓት በማውጫዎች ውስጥ እንዲያቀርቡ የመጨረሻ ነጥቦች።
- ** ሙከራ: *** ለመቀየሪያ/መግለጫ ዙር-ጉዞዎች፣ የተሳሳተ ሰንሰለት የክፍል ሙከራዎችን ያክሉ
  አለመሳካቶች, እና ግልጽ እይታዎች; በTorii እና ኤስዲኬዎች ውስጥ የውህደት ሽፋን ይጨምሩ
  ለ IH58 ፍሰቶች ከጫፍ እስከ ጫፍ.

## የስህተት ኮድ መዝገብ ቤት

የአድራሻ ማመሳከሪያዎች እና ዲኮደሮች አለመሳካቶችን ያጋልጣሉ
`AccountAddressError::code_str()`. የሚከተሉት ሰንጠረዦች የተረጋጋ ኮዶችን ይሰጣሉ
ኤስዲኬዎች፣ የኪስ ቦርሳዎች እና Torii ንጣፎች በሰው ሊነበቡ ከሚችሉት ጎን ለጎን መታየት አለባቸው።
መልዕክቶች እና የሚመከር የማሻሻያ መመሪያ።

### ቀኖናዊ ግንባታ

| ኮድ | ውድቀት | የሚመከር ማስተካከያ |
|-------|--------|-------------|
| `ERR_UNSUPPORTED_ALGORITHM` | ኢንኮደር በመዝገቡ ወይም በግንባታ ባህሪያት የማይደገፍ የመፈረሚያ ስልተ ቀመር ተቀብሏል። | የመለያ ግንባታን በመዝገቡ እና ውቅር ውስጥ የነቁ ኩርባዎችን ይገድቡ። |
| `ERR_KEY_PAYLOAD_TOO_LONG` | የመፈረሚያ ቁልፍ የመጫኛ ርዝመት ከሚደገፈው ገደብ አልፏል። | ነጠላ-ቁልፍ መቆጣጠሪያዎች ለ `u8` ርዝማኔዎች የተገደቡ ናቸው; ለትልቅ የህዝብ ቁልፎች መልቲሲግ ተጠቀም (ለምሳሌ፡ ML-DSA)። |
| `ERR_INVALID_HEADER_VERSION` | የአድራሻ ራስጌ ሥሪት ከሚደገፈው ክልል ውጪ ነው። | Emit ራስጌ ስሪት I18NI0000523X ለ V1 አድራሻዎች; አዲስ ስሪቶችን ከመጠቀምዎ በፊት ኢንኮደሮችን ያሻሽሉ። |
| `ERR_INVALID_NORM_VERSION` | የመደበኛነት ስሪት ባንዲራ አልታወቀም። | የመደበኛነት ስሪት `1` ይጠቀሙ እና የተያዙ ቢትዎችን ከመቀያየር ይቆጠቡ። |
| `ERR_INVALID_IH58_PREFIX` | የተጠየቀው IH58 የአውታረ መረብ ቅድመ ቅጥያ ሊገለበጥ አይችልም። | በሰንሰለት መዝገብ ውስጥ በታተመው `0..=16383` ክልል ውስጥ ቅድመ ቅጥያ ይምረጡ። |
| `ERR_CANONICAL_HASH_FAILURE` | ቀኖናዊ ክፍያ ሃሽንግ አልተሳካም። | ቀዶ ጥገናውን እንደገና ይሞክሩ; ስህተቱ ከቀጠለ በሃሺንግ ቁልል ውስጥ እንደ ውስጣዊ ሳንካ አድርገው ይያዙት። |

### ቅርጸት መፍታት እና አውቶማቲክ ፍለጋ

| ኮድ | ውድቀት | የሚመከር ማስተካከያ |
|-------|--------|-------------|
| `ERR_INVALID_IH58_ENCODING` | IH58 ሕብረቁምፊ ከፊደል ውጭ የሆኑ ቁምፊዎችን ይዟል። | አድራሻው የታተመውን IH58 ፊደላት መጠቀሙን እና በቅጂ/መለጠፍ ጊዜ እንዳልተቆራረጠ ያረጋግጡ። |
| `ERR_INVALID_LENGTH` | የመጫኛ ርዝመት ለመራጩ/ተቆጣጣሪው ከሚጠበቀው ቀኖናዊ መጠን ጋር አይዛመድም። | ለተመረጠው ጎራ መራጭ እና ተቆጣጣሪ አቀማመጥ ሙሉውን ቀኖናዊ ክፍያ ያቅርቡ። |
| `ERR_CHECKSUM_MISMATCH` | IH58 (ተመራጭ) ወይም የተጨመቀ (`sora`፣ ሁለተኛ-ምርጥ) የፍተሻ ክፍያ ማረጋገጥ አልተሳካም። | አድራሻውን ከታመነ ምንጭ ያድሱ; ይህ በተለምዶ የቅጂ/መለጠፍ ስህተትን ያሳያል። |
| `ERR_INVALID_IH58_PREFIX_ENCODING` | የIH58 ቅድመ ቅጥያ ባይት ተበላሽቷል። | አድራሻውን ከታዛዥ ኢንኮደር ጋር እንደገና መመስጠር; መሪውን Base58 ባይት በእጅ አይቀይሩት። |
| `ERR_INVALID_HEX_ADDRESS` | ቀኖናዊ ሄክሳዴሲማል ፎርም መፍታት አልቻለም። | በይፋዊ ኢንኮደር የተሰራ `0x`-ቅድመ-ቅጥያ፣ እኩል-ርዝመት ያለው የአስራስድስትዮሽ ሕብረቁምፊ ያቅርቡ። |
| `ERR_MISSING_COMPRESSED_SENTINEL` | የታመቀ ቅጽ በ `sora` አይጀምርም። | የተጨመቁ የሶራ አድራሻዎችን ለዲኮደሮች ከመስጠታችሁ በፊት ከሚፈለገው ተላላኪ ጋር ቅድመ ቅጥያ ያድርጉ። |
| `ERR_COMPRESSED_TOO_SHORT` | የታመቀ ሕብረቁምፊ ለክፍያ እና ለቼክ ድምር በቂ አሃዞች ይጎድለዋል። | ከተቆራረጡ ቅንጥቦች ይልቅ በመቀየሪያው የሚወጣውን ሙሉ የታመቀ ሕብረቁምፊ ይጠቀሙ። |
| `ERR_INVALID_COMPRESSED_CHAR` | ከተጨመቀ ፊደላት ውጭ ያለ ገጸ ባህሪ አጋጥሞታል። | ከታተመው የግማሽ ስፋት/ሙሉ ስፋት ሠንጠረዦች ገፀ ባህሪውን በሚሰራ Base-105 ግሊፍ ይተኩ። |
| `ERR_INVALID_COMPRESSED_BASE` | ኢንኮደር የማይደገፍ ራዲክስ ለመጠቀም ሞክሯል። | በመቀየሪያው ላይ ስህተት ያስገቡ; የተጨመቀው ፊደል በ V1 ውስጥ ወደ ራዲክስ 105 ተስተካክሏል። |
| `ERR_INVALID_COMPRESSED_DIGIT` | አሃዝ እሴት ከተጨመቀው ፊደል መጠን ይበልጣል። | አስፈላጊ ከሆነ አድራሻውን በማደስ እያንዳንዱ አሃዝ በ`0..105)` ውስጥ መሆኑን ያረጋግጡ። |
| `ERR_UNSUPPORTED_ADDRESS_FORMAT` | ራስ-ማወቂያ የግቤት ቅርጸቱን ማወቅ አልቻለም። | ተንታኞች በሚጠሩበት ጊዜ IH58 (ተመራጭ)፣ የተጨመቁ (`sora`) ወይም ቀኖናዊ `0x` ሄክስ ሕብረቁምፊዎችን ያቅርቡ። |

### የጎራ እና የአውታረ መረብ ማረጋገጫ| ኮድ | ውድቀት | የሚመከር ማስተካከያ |
|-------|--------|-------------|
| `ERR_DOMAIN_MISMATCH` | ጎራ መራጭ ከሚጠበቀው ጎራ ጋር አይዛመድም። | ለታሰበው ጎራ የተሰጠ አድራሻ ይጠቀሙ ወይም የሚጠበቀውን ያዘምኑ። |
| `ERR_INVALID_DOMAIN_LABEL` | የጎራ መለያው የመደበኛነት ፍተሻዎችን አልተሳካም። | ኮድ ከማስቀመጥዎ በፊት UTS-46 መሸጋገሪያ ያልሆነ ሂደትን በመጠቀም ጎራውን ቀኖናዊ ያድርጉት። |
| `ERR_UNEXPECTED_NETWORK_PREFIX` | ዲኮድ የተደረገው የIH58 አውታረ መረብ ቅድመ ቅጥያ ከተዋቀረው እሴት ይለያል። | ከዒላማው ሰንሰለት ወደ አድራሻ ይቀይሩ ወይም የሚጠበቀውን አድልዎ/ቅድመ ቅጥያ ያስተካክሉ። |
| `ERR_UNKNOWN_ADDRESS_CLASS` | የአድራሻ ክፍል ቢትስ አይታወቅም። | ዲኮደር አዲሱን ክፍል ወደ ሚረዳ ልቀት ያሻሽሉ ወይም የራስጌ ቢትን ከመነካካት ይቆጠቡ። |
| `ERR_UNKNOWN_DOMAIN_TAG` | የጎራ መራጭ መለያ አይታወቅም። | አዲሱን የመራጭ አይነት ወደሚደግፍ ልቀት ያዘምኑ ወይም በV1 ኖዶች ላይ የሙከራ ጭነት ከመጠቀም ይቆጠቡ። |
| `ERR_UNEXPECTED_EXTENSION_FLAG` | የተያዘ የኤክስቴንሽን ቢት ተቀናብሯል። | የተጠበቁ ቢቶችን አጽዳ; ወደፊት ኤቢኢ እስኪያስተዋውቃቸው ድረስ ዝግ ሆነው ይቆያሉ። |
| `ERR_UNKNOWN_CONTROLLER_TAG` | የመቆጣጠሪያው ጭነት መለያ አልታወቀም። | አዳዲስ የመቆጣጠሪያ ዓይነቶችን ከመተንተንዎ በፊት ለመለየት ዲኮደርን ያሻሽሉ። |
| `ERR_UNEXPECTED_TRAILING_BYTES` | ቀኖናዊ ክፍያ ከዲኮዲንግ በኋላ ተከታይ ባይት ይዟል። | ቀኖናዊውን ጭነት እንደገና ማደስ; የሰነድ ርዝመት ብቻ መገኘት አለበት. |

### የመቆጣጠሪያ ክፍያ ማረጋገጫ

| ኮድ | ውድቀት | የሚመከር ማስተካከያ |
|-------|--------|-------------|
| `ERR_INVALID_PUBLIC_KEY` | የቁልፍ ባይቶች ከታወጀው ከርቭ ጋር አይዛመዱም። | ለተመረጠው ከርቭ (ለምሳሌ፡ 32-ባይት Ed25519) የቁልፍ ባይቶች ልክ እንደ አስፈላጊነቱ መቀመጡን ያረጋግጡ። |
| `ERR_UNKNOWN_CURVE` | ኩርባ ለዪ አልተመዘገበም። | ተጨማሪ ኩርባዎች እስኪጸድቁ እና በመዝገቡ ውስጥ እስኪታተሙ ድረስ የጥምዝ መታወቂያ `1` (Ed25519) ይጠቀሙ። |
| `ERR_MULTISIG_MEMBER_OVERFLOW` | የመልቲሲግ መቆጣጠሪያ ከሚደገፉት በላይ አባላትን ያውጃል። | ኮድ ከማስቀመጥዎ በፊት የባለብዙ ሲግ አባልነትን ወደ ሰነዱ ገደብ ይቀንሱ። |
| `ERR_INVALID_MULTISIG_POLICY` | የባለብዙ ሲግ ፖሊሲ ክፍያ ማረጋገጥ አልተሳካም (ደረጃ/ክብደቶች/ዕቅድ)። | የ CTAP2 ንድፍን፣ የክብደት ወሰኖችን እና የመነሻ ገደቦችን እንዲያረካ ፖሊሲውን እንደገና ገንባ። |

## አማራጮች ግምት ውስጥ ገብተዋል።

- ** ንጹህ ቤዝ58Check (Bitcoin-style)።
  ከ Blake2b-የተገኘ IH58 ቼክ (`encode_ih58` ባለ 512-ቢት ሃሽ ይቆርጣል)
  እና ለ16-ቢት አድሎአውያን ግልጽ የሆነ ቅድመ ቅጥያ የለውም።
- ** የጎራ ሕብረቁምፊ ውስጥ የሰንሰለት ስም መክተት (ለምሳሌ I18NI0000560X)።** ይሰብራል
- ** አድራሻዎችን ሳይቀይሩ በNexus ማዞሪያ ላይ ብቻ ይተማመኑ።** ተጠቃሚዎች አሁንም ይቀጥላሉ
  አሻሚ ገመዶችን ይቅዱ / ይለጥፉ; አድራሻው ራሱ አውድ እንዲይዝ እንፈልጋለን።
- **Bech32m ኤንቨሎፕ።** ለQR ተስማሚ እና ለሰው ሊነበብ የሚችል ቅድመ ቅጥያ ያቀርባል፣ነገር ግን
  ከማጓጓዣው IH58 ትግበራ (`AccountAddress::to_ih58`) ይለያል።
  እና ሁሉንም ቋሚዎች/ኤስዲኬዎች እንደገና መፍጠርን ይጠይቃል። የአሁኑ ፍኖተ ካርታ IH58+ን ይጠብቃል።
  ለወደፊት ምርምር በሚቀጥልበት ጊዜ የታመቀ (`sora`) ድጋፍ
  Bech32m/QR ንብርብሮች (CAIP-10 ካርታ ስራ ዘግይቷል)።

## ክፍት ጥያቄዎች

- `u16` አድልዎ እና የተጠበቁ ክልሎች የረጅም ጊዜ ፍላጎትን እንደሚሸፍኑ ያረጋግጡ;
  አለበለዚያ `u32` በተለዋዋጭ ኢንኮዲንግ ይገምግሙ።
- ለመመዝገቢያ ማሻሻያ እና እንዴት የባለብዙ ፊርማ አስተዳደር ሂደቱን ያጠናቅቁ
  መሻሮች/ያለፉት ምደባዎች ይያዛሉ።
- ትክክለኛውን አንጸባራቂ ፊርማ እቅድ ይግለጹ (ለምሳሌ፣ Ed25519 ባለብዙ ሲግ) እና
  የትራንስፖርት ደህንነት (ኤችቲቲፒኤስ መሰካት፣ IPFS ሃሽ ቅርጸት) ለNexus ስርጭት።
- ለስደት የጎራ ተለዋጭ ስሞችን/ማዞሪያዎችን እና እንዴት እንደሚደግፉ ይወስኑ
  ቁርጠኝነትን ሳይጥስ በእነርሱ ላይ መገኘት.
- Kotodama/IVM ኮንትራቶች IH58 ረዳቶችን እንዴት እንደሚያገኙ ይግለጹ (`to_address()`፣
  `parse_address()`) እና በሰንሰለት ላይ ያለው ማከማቻ CAIP-10ን ማጋለጥ አለበት ወይ?
  ካርታዎች (ዛሬ IH58 ቀኖናዊ ነው)።
- በውጭ መዝገቦች ውስጥ የI18NT0000008X ሰንሰለቶችን መመዝገብ ያስሱ (ለምሳሌ፣ IH58 መዝገብ ቤት፣
  CAIP የስም ቦታ ማውጫ) ለሰፊ የስነምህዳር አሰላለፍ።

## ቀጣይ እርምጃዎች

1. IH58 ኢንኮዲንግ በ`iroha_data_model` (`AccountAddress::to_ih58`፣
   `parse_encoded`); እቃዎችን/ሙከራዎችን ወደ እያንዳንዱ ኤስዲኬ ማጓጓዝዎን ይቀጥሉ እና ማንኛውንም ያፅዱ
   Bech32m ቦታ ያዥ።
2. የውቅረት እቅድን በ`chain_discriminant` ያራዝሙ እና አስተዋይ ያግኙ
  ለነባር የሙከራ/የዴቭ ማዋቀር ነባሪዎች። ** (ተከናውኗል፡ `common.chain_discriminant`
  አሁን ወደ `0x02F1` በነባሪነት በ I18NI0000572X ይላካል
  ይሽራል።)**
3. የI18NT0000029X የመመዝገቢያ ንድፍ እና የፅንሰ-ሃሳብ ማረጋገጫ አንጸባራቂ አሳታሚ።
4. በሰው-ተኮር ገፅታዎች ላይ ከኪስ አቅራቢዎች እና ጠባቂዎች ግብረ መልስ ይሰብስቡ
   (HRP መሰየም ፣ የማሳያ ቅርጸት)።
5. ሰነዶችን (`docs/source/data_model.md`፣ Torii API docs) አንዴ ያዘምኑ
   የትግበራ መንገድ ቁርጠኛ ነው።
6. ኦፊሴላዊ የኮዴክ ቤተ-መጻሕፍት (ዝገት/TS/Python/Kotlin) በመደበኛ ፈተና ይላኩ
   ስኬትን እና ውድቀትን የሚሸፍኑ ቬክተሮች.
