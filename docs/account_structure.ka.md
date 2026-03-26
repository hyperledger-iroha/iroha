---
lang: ka
direction: ltr
source: docs/account_structure.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 01561366c9698c10d29ff3f49ad4c14b22b1796b5c7701cf98d200a140af1caf
source_last_modified: "2026-01-28T17:11:30.635172+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# ანგარიშის სტრუქტურა RFC

** სტატუსი: ** მიღებულია (ADDR-1)  
** აუდიტორია:** მონაცემთა მოდელი, Torii, Nexus, Wallet, მმართველობის გუნდები  
** დაკავშირებული საკითხები: ** TBD

## რეზიუმე

ეს დოკუმენტი აღწერს მიწოდების ანგარიშის მისამართების დასტას, რომელიც განხორციელდა
`AccountAddress` (`crates/iroha_data_model/src/account/address.rs`) და
კომპანიონი ხელსაწყოები. ის უზრუნველყოფს:

- შეჯამებული, ადამიანისკენ მიმართული **I105 მისამართი** მიერ წარმოებული
  `AccountAddress::to_i105`, რომელიც აკავშირებს ჯაჭვის დისკრიმინანტს ანგარიშთან
  კონტროლერი და გთავაზობთ დეტერმინისტულ ინტეროპ მეგობრულ ტექსტურ ფორმებს.
- დომენის სელექტორები იმპლიციტური ნაგულისხმევი დომენებისთვის და ლოკალური დაიჯესტებისთვის, a
  დაცულია გლობალური რეესტრის ამორჩევის ტეგი მომავალი Nexus მხარდაჭერილი მარშრუტირებისთვის (
  რეესტრის ძებნა ** ჯერ არ არის გაგზავნილი **).

## მოტივაცია

საფულეები და ჯაჭვის გარეშე ხელსაწყოები ეყრდნობა დღეს `name@dataspace` or `name@domain.dataspace` მარშრუტიზაციის ნედლეულ მეტსახელებს. ეს
აქვს ორი ძირითადი ნაკლი:

1. ** ქსელის სავალდებულო არ არის. ** სტრიქონს არ აქვს გამშვები ჯამი ან ჯაჭვის პრეფიქსი, ამიტომ მომხმარებლებს
   შეუძლია მისამართის ჩასმა არასწორი ქსელიდან დაუყოვნებლივი გამოხმაურების გარეშე. The
   ტრანზაქცია საბოლოოდ უარყოფილი იქნება (ჯაჭვის შეუსაბამობა) ან, უარესი, წარმატებული იქნება
   არასასურველი ანგარიშის წინააღმდეგ, თუ დანიშნულება ადგილობრივად არსებობს.
2. **დომენის შეჯახება.** დომენები არის მხოლოდ სახელთა სივრცე და მათი ხელახლა გამოყენება შესაძლებელია თითოეულზე
   ჯაჭვი. სერვისების ფედერაცია (მზრუნველები, ხიდები, ჯვარედინი სამუშაო ნაკადები)
   ხდება მყიფე, რადგან `finance` ჯაჭვზე A არ არის დაკავშირებული `finance`-თან
   ჯაჭვი B.

ჩვენ გვჭირდება ადამიანური მიმართვის ფორმატი, რომელიც იცავს კოპირების/პასტის შეცდომებს
და დეტერმინისტული რუქა დომენის სახელიდან ავტორიტეტულ ჯაჭვამდე.

## გოლები

- აღწერეთ I105 კონვერტი დანერგილი მონაცემთა მოდელში და
  კანონიკური პარსინგის წესები, რომლებსაც `AccountId` და `AccountAddress` იცავენ.
- დაშიფვრეთ კონფიგურირებული ჯაჭვის დისკრიმინანტი პირდაპირ თითოეულ მისამართზე და
  განსაზღვროს მისი მართვის/რეგისტრაციის პროცესი.
- აღწერეთ, როგორ შემოვიტანოთ დომენის გლობალური რეესტრი დენის დარღვევის გარეშე
  განლაგება და ნორმალიზების/გაყალბების საწინააღმდეგო წესების მითითება.

## არაგოლები

- აქტივების ჯაჭვური ტრანსფერების განხორციელება. მარშრუტიზაციის ფენა აბრუნებს მხოლოდ
  სამიზნე ჯაჭვი.
- გლობალური დომენის გაცემის მმართველობის დასრულება. ეს RFC ფოკუსირებულია მონაცემებზე
  მოდელი და სატრანსპორტო პრიმიტივები.

## ფონი

### მიმდინარე მარშრუტიზაციის მეტსახელი

```
AccountId {
    domain: DomainId,   // wrapper over Name (ASCII-ish string)
    controller: AccountController // single PublicKey or multisig policy
}

Display: canonical Katakana i105 literal (no `@domain` suffix)
Parse accepts:
- Encoded account identifiers only: i105.
- Runtime parsers reject canonical hex (`0x...`), any `@<domain>` suffix, and account-alias literals such as name@dataspace or name@domain.dataspace.

Multihash hex is canonical: varint bytes are lowercase hex, payload bytes are uppercase hex,
and `0x` prefixes are not accepted.

Account aliases are separate on-chain bindings. They use
name@dataspace or name@domain.dataspace and resolve to canonical
i105 `AccountId` values. Strict `AccountId` parsers never accept alias literals directly.
```

`ChainId` ცხოვრობს `AccountId`-ის გარეთ. კვანძები ამოწმებენ გარიგების `ChainId`
კონფიგურაციის საწინააღმდეგოდ მიღებისას (`AcceptTransactionFail::ChainIdMismatch`)
და უარყოს უცხოური ტრანზაქცია, მაგრამ ანგარიშის სტრიქონი თავისთავად შეიცავს No
ქსელის მინიშნება.

### დომენის იდენტიფიკატორები

`DomainId` ახვევს `Name`-ს (ნორმალიზებული სტრიქონი) და ვრცელდება ლოკალურ ჯაჭვზე.
ყველა ჯაჭვს შეუძლია დამოუკიდებლად დაარეგისტრიროს `wonderland`, `finance` და ა.შ.

### Nexus კონტექსტი

Nexus პასუხისმგებელია კომპონენტთაშორის კოორდინაციაზე (ზოლები/მონაცემთა სივრცეები). ის
ამჟამად არ აქვს ჯვარედინი ჯაჭვის დომენის მარშრუტიზაციის კონცეფცია.

## შემოთავაზებული დიზაინი

### 1. დეტერმინისტული ჯაჭვის დისკრიმინანტი

`iroha_config::parameters::actual::Common` ახლა ამჟღავნებს:

```rust
pub struct Common {
    pub chain: ChainId,
    pub chain_discriminant: u16, // globally coordinated
    // ... existing fields
}
```

- ** შეზღუდვები:**
  - უნიკალური თითო აქტიურ ქსელში; იმართება ხელმოწერილი საჯარო რეესტრის მეშვეობით
    აშკარა დაჯავშნილი დიაპაზონები (მაგ., `0x0000–0x0FFF` ტესტი/დევი, `0x1000–0x7FFF`
    საზოგადოების გამოყოფა, `0x8000–0xFFEF` მმართველობით დამტკიცებული, `0xFFF0–0xFFFF`
    დაცულია).
  - უცვლელი გაშვებული ჯაჭვისთვის. მის შეცვლას მყარი ჩანგალი და ა
    რეესტრის განახლება.
- **მმართველობა და რეესტრი (დაგეგმილი): ** მრავალხელმოწერიანი მმართველობის ნაკრები
  ხელმოწერილი JSON რეესტრის შენარჩუნება, რომელიც ასახავს დისკრიმინატორებს ადამიანის ალიასებზე და
  CAIP-2 იდენტიფიკატორები. ეს რეესტრი ჯერ არ არის გაგზავნილი მუშაობის დროის ნაწილი.
- **გამოყენება:** გადადის სახელმწიფო დაშვების, Torii, SDK-ებისა და საფულის API-ების მეშვეობით
  ყველა კომპონენტს შეუძლია მისი ჩასმა ან გადამოწმება. CAIP-2 ექსპოზიცია მომავალში რჩება
  ინტეროპის ამოცანა.

### 2. მისამართების კანონიკური კოდეკები

Rust მონაცემთა მოდელი ავლენს ერთი კანონიკური დატვირთვის წარმოდგენას
(`AccountAddress`), რომელიც შეიძლება გამოიცეს რამდენიმე ადამიანის წინაშე მდგარ ფორმატში. I105 არის
ანგარიშის სასურველი ფორმატი გაზიარებისა და კანონიკური გამოსავლისთვის; შეკუმშული
`sora` ფორმა არის მეორე საუკეთესო, მხოლოდ Sora-ის ვარიანტი UX-ისთვის, სადაც კანა ანბანია
მატებს ღირებულებას. Canonical hex რჩება გამართვის დამხმარე საშუალებად.

- **I105** – I105 კონვერტი, რომელიც ათავსებს ჯაჭვს
  დისკრიმინანტი. დეკოდერები ამოწმებენ პრეფიქსის დატვირთვის დაწინაურებამდე
  კანონიკური ფორმა.
- **Sora-შეკუმშული ხედი** – მხოლოდ Sora ანბანი **105 სიმბოლოსგან** აშენებული
  ნახევრად სიგანის イロハ ლექსის (ヰ და ヱ ჩათვლით) დამატება 58-სიმბოლოზე
  I105 კომპლექტი. სტრიქონები იწყება სენტინელით `sora`, ჩაშენებულია Bech32m-დან მიღებული
  გამშვები ჯამი და გამოტოვეთ ქსელის პრეფიქსი (Sora Nexus იგულისხმება სენტინელის მიერ).

  ```
  I105  : 123456789ABCDEFGHJKMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz
  Iroha : ｲﾛﾊﾆﾎﾍﾄﾁﾘﾇﾙｦﾜｶﾖﾀﾚｿﾂﾈﾅﾗﾑｳヰﾉｵｸﾔﾏｹﾌｺｴﾃｱｻｷﾕﾒﾐｼヱﾋﾓｾｽ
  ```
- **Canonical hex** – კანონიკური ბაიტის `0x…` კოდირება გამართვისთვის
  კონვერტი.

`AccountAddress::parse_encoded` ავტომატურად ამოიცნობს I105 (სასურველია), შეკუმშული (`sora`, მეორე საუკეთესო) ან კანონიკურ ექვსკუთხედს
(მხოლოდ `0x...`; შიშველი ექვსკუთხედი უარყოფილია) შეაქვს და აბრუნებს როგორც დეკოდირებულ დატვირთვას, ასევე აღმოჩენილს
`AccountAddress`. Torii ახლა უწოდებს `parse_encoded`-ს ISO 20022-ის დამატებით
მიმართავს და ინახავს კანონიკურ თექვსმეტობით ფორმას, ასე რომ მეტამონაცემები დეტერმინისტული რჩება
ორიგინალური წარმოდგენის მიუხედავად.

#### 2.1 ჰედერის ბაიტის განლაგება (ADDR-1a)

ყოველი კანონიკური დატვირთვა განლაგებულია როგორც `header · controller`. The
`header` არის ერთი ბაიტი, რომელიც აცნობებს, რომელი პარსერის წესები ვრცელდება იმ ბაიტებზე, რომლებიც
მიჰყევით:

```
bit index:   7        5 4      3 2      1 0
             ┌─────────┬────────┬────────┬────┐
payload bit: │version  │ class  │  norm  │ext │
             └─────────┴────────┴────────┴────┘
```

ამრიგად, პირველი ბაიტი აერთიანებს სქემის მეტამონაცემებს ქვედა დინების დეკოდერებისთვის:

| ბიტები | ველი | დაშვებული მნიშვნელობები | შეცდომა დარღვევაზე |
|------|-------|---------------|-------------------|
| 7-5 | `addr_version` | `0` (v1). მნიშვნელობები `1-7` დაცულია მომავალი გადასინჯვისთვის. | მნიშვნელობები `0-7` ტრიგერის გარეთ `AccountAddressError::InvalidHeaderVersion`; განხორციელებები უნდა განიხილებოდეს არა-ნულოვანი ვერსიები, როგორც მხარდაუჭერელი დღეს. |
| 4-3 | `addr_class` | `0` = ერთი კლავიში, `1` = მრავალმნიშვნელოვანი. | სხვა მნიშვნელობები ზრდის `AccountAddressError::UnknownAddressClass`. |
| 2-1 | `norm_version` | `1` (ნორმა v1). მნიშვნელობები `0`, `2`, `3` დაცულია. | მნიშვნელობები `0-3`-ს გარეთ ზრდის `AccountAddressError::InvalidNormVersion`. |
| 0 | `ext_flag` | უნდა იყოს `0`. | კომპლექტი ბიტი ზრდის `AccountAddressError::UnexpectedExtensionFlag`. |

Rust შიფრატორი წერს `0x02` ერთი გასაღების კონტროლერებისთვის (ვერსია 0, კლასი 0,
ნორმა v1, გაფართოების დროშა გასუფთავებულია) და `0x0A` მულტისიგ კონტროლერებისთვის (ვერსია 0,
კლასი 1, ნორმა v1, გაფართოების დროშა გასუფთავებულია).

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

#### 2.3 კონტროლერის დატვირთვის კოდირება (ADDR-1a)

კონტროლერის payload არის კიდევ ერთი ტეგირებული კავშირი, რომელიც დართულია დომენის ამორჩევის შემდეგ:| ტეგი | კონტროლერი | განლაგება | შენიშვნები |
|-----|------------|--------|-------|
| `0x00` | ერთი გასაღები | `curve_id:u8` · `key_len:u8` · `key_bytes` | `curve_id=0x01` რუკები Ed25519-ზე დღეს. `key_len` შემოიფარგლება `u8`-ით; უფრო დიდი მნიშვნელობები ამაღლებს `AccountAddressError::KeyPayloadTooLong`-ს (ასე რომ, ერთი გასაღების ML-DSA საჯარო გასაღებები, რომლებიც >255 ბაიტია, არ შეიძლება იყოს კოდირებული და უნდა გამოიყენონ მრავალსიგანი). |
| `0x01` | Multisig | `version:u8` · `threshold:u16` · `member_count:u8` · (`curve_id:u8` · `weight:u16` · `key_len:u16` · `curve_id:u8` · `weight:u16` · `key_len:u16` · I18NI0000016 | მხარს უჭერს 255 წევრამდე (`CONTROLLER_MULTISIG_MEMBER_MAX`). უცნობი მრუდები ამაღლებს `AccountAddressError::UnknownCurve`; არასწორი პოლიტიკის ბუშტი ჩნდება, როგორც `AccountAddressError::InvalidMultisigPolicy`. |

Multisig პოლიტიკა ასევე ასახავს CTAP2-ის სტილის CBOR რუკას და კანონიკურ დაიჯესტს
ჰოსტებსა და SDK-ებს შეუძლიათ კონტროლერის განმსაზღვრელი შემოწმება. იხ
`docs/source/references/multisig_policy_schema.md` (ADDR-1c) სქემისთვის,
ვალიდაციის წესები, ჰეშირების პროცედურა და ოქროს მოწყობილობები.

ყველა გასაღების ბაიტი დაშიფრულია ზუსტად ისე, როგორც დაბრუნდა `PublicKey::to_bytes`; დეკოდერები აღადგენენ `PublicKey` ინსტანციებს და ზრდიან `AccountAddressError::InvalidPublicKey`-ს, თუ ბაიტები არ ემთხვევა დეკლარირებულ მრუდს.

> **Ed25519 კანონიკური აღსრულება (ADDR-3a):** მრუდის `0x01` გასაღებები უნდა გაშიფრულიყო ხელმომწერის მიერ გამოშვებულ ზუსტად ბაიტის სტრიქონამდე და არ უნდა იყოს მცირე რიგის ქვეჯგუფში. კვანძები ახლა უარყოფენ არაკანონიკურ დაშიფვრებს (მაგ., `2^255-19` მოდულის შემცირებული მნიშვნელობები) და სუსტ წერტილებს, როგორიცაა პირადობის ელემენტი, ამიტომ SDK-ებმა უნდა გამოავლინონ შესატყვისი ვალიდაციის შეცდომები მისამართების გაგზავნამდე.

##### 2.3.1 მრუდის იდენტიფიკატორის რეესტრი (ADDR-1d)

| ID (`curve_id`) | ალგორითმი | მხატვრული კარიბჭე | შენიშვნები |
|-----------------|----------|-------------|------|
| `0x00` | დაცულია | — | არ უნდა იყოს ემიტირებული; დეკოდერების ზედაპირი `ERR_UNKNOWN_CURVE`. |
| `0x01` | Ed25519 | — | კანონიკური v1 ალგორითმი (`Algorithm::Ed25519`); ჩართულია ნაგულისხმევ კონფიგურაციაში. |
| `0x02` | ML-DSA (დილითიუმი3) | — | იყენებს Dilithium3 საჯარო გასაღების ბაიტებს (1952 ბაიტი). ერთი გასაღების მისამართებს არ შეუძლია ML-DSA კოდირება, რადგან `key_len` არის `u8`; multisig იყენებს `u16` სიგრძეს. |
| `0x03` | BLS12‑381 (ნორმალური) | `bls` | საჯარო გასაღებები G1-ში (48 ბაიტი), ხელმოწერები G2-ში (96 ბაიტი). |
| `0x04` | secp256k1 | — | დეტერმინისტული ECDSA SHA-256-ზე; საჯარო გასაღებები იყენებენ 33-ბაიტიან SEC1 შეკუმშულ ფორმას, ხოლო ხელმოწერები იყენებენ კანონიკურ 64-ბაიტიან `r∥s` განლაგებას. |
| `0x05` | BLS12‑381 (პატარა) | `bls` | საჯარო გასაღებები G2-ში (96 ბაიტი), ხელმოწერები G1-ში (48 ბაიტი). |
| `0x0A` | GOST R 34.10-2012 (256, კომპლექტი A) | `gost` | ხელმისაწვდომია მხოლოდ მაშინ, როდესაც ჩართულია `gost` ფუნქცია. |
| `0x0B` | GOST R 34.10-2012 (256, კომპლექტი B) | `gost` | ხელმისაწვდომია მხოლოდ მაშინ, როდესაც ჩართულია `gost` ფუნქცია. |
| `0x0C` | GOST R 34.10-2012 (256, კომპლექტი C) | `gost` | ხელმისაწვდომია მხოლოდ მაშინ, როდესაც ჩართულია `gost` ფუნქცია. |
| `0x0D` | GOST R 34.10-2012 (512, კომპლექტი A) | `gost` | ხელმისაწვდომია მხოლოდ მაშინ, როდესაც ჩართულია `gost` ფუნქცია. |
| `0x0E` | GOST R 34.10-2012 (512, კომპლექტი B) | `gost` | ხელმისაწვდომია მხოლოდ მაშინ, როდესაც ჩართულია `gost` ფუნქცია. |
| `0x0F` | SM2 | `sm` | DistID სიგრძე (u16 BE) + DistID ბაიტი + 65-ბაიტი SEC1 შეუკუმშული SM2 კლავიში; ხელმისაწვდომია მხოლოდ მაშინ, როდესაც ჩართულია `sm`. |

სლოტები `0x06–0x09` რჩება გამოუყენებელი დამატებითი მოსახვევებისთვის; ახლის დანერგვა
ალგორითმი მოითხოვს საგზაო რუქის განახლებას და შესაბამისი SDK/მასპინძლის დაფარვას. შიფრები
უნდა უარყოს ნებისმიერი მხარდაჭერილი ალგორითმი `ERR_UNSUPPORTED_ALGORITHM`-ით და
დეკოდერები სწრაფად უნდა ჩავარდეს უცნობ ID-ებზე `ERR_UNKNOWN_CURVE` შესანარჩუნებლად
მარცხით დახურული ქცევა.

კანონიკური რეესტრი (მათ შორის მანქანით წაკითხვადი JSON ექსპორტი) მუშაობს
[`docs/source/references/address_curve_registry.md`](source/references/address_curve_registry.md).
Tooling-მა უნდა მოიხმაროს ეს მონაცემთა ბაზა პირდაპირ, რათა მრუდის იდენტიფიკატორები დარჩეს
თანმიმდევრულია SDK-ებსა და ოპერატორების სამუშაო პროცესებში.

- **SDK კარიბჭე:** SDK-ების ნაგულისხმევად არის მხოლოდ Ed25519 ვალიდაცია/კოდირება. სვიფტი ამხელს
  კომპილაციის დროის დროშები (`IROHASWIFT_ENABLE_MLDSA`, `IROHASWIFT_ENABLE_GOST`,
  `IROHASWIFT_ENABLE_SM`); Java/Android SDK მოითხოვს
  `AccountAddress.configureCurveSupport(...)`; JavaScript SDK იყენებს
  `configureCurveSupport({ allowMlDsa: true, allowGost: true, allowSm2: true })`.
  secp256k1 მხარდაჭერა ხელმისაწვდომია, მაგრამ არ არის ჩართული ნაგულისხმევად JS/Android-ში
  SDK-ები; აბონენტებმა მკაფიოდ უნდა მიიღონ მონაწილეობა არა-Ed25519 კონტროლერების გამოშვებისას.
- **ჰოსტის კარიბჭე:** `Register<Account>` უარყოფს კონტროლერებს, რომელთა ხელმომწერები იყენებენ ალგორითმებს
  აკლია კვანძის `crypto.allowed_signing` სიიდან ** ან ** მრუდის იდენტიფიკატორები არ არის
  `crypto.curves.allowed_curve_ids`, ამიტომ კლასტერებმა უნდა განაცხადონ მხარდაჭერა (კონფიგურაცია +
  genesis) ML-DSA/GOST/SM კონტროლერების რეგისტრაციამდე. BLS კონტროლერი
  ალგორითმები ყოველთვის დაშვებულია შედგენისას (კონსენსუსის კლავიშები მათზეა დამოკიდებული),
  და ნაგულისხმევი კონფიგურაცია ჩართავს Ed25519 + secp256k1.【crates/iroha_core/src/smartcontracts/isi/domain.rs:32】

##### 2.3.2 Multisig კონტროლერის ხელმძღვანელობა

`AccountController::Multisig` სერიებს ახორციელებს პოლიტიკას მეშვეობით
`crates/iroha_data_model/src/account/controller.rs` და ახორციელებს სქემას
დოკუმენტირებულია [`docs/source/references/multisig_policy_schema.md`] (source/references/multisig_policy_schema.md).
განხორციელების ძირითადი დეტალები:

- პოლიტიკა ნორმალიზებულია და დამოწმებულია `MultisigPolicy::validate()`-ით ადრე
  ჩადგმული. ზღურბლები უნდა იყოს ≥1 და ≤Σ წონა; დუბლიკატი წევრები არიან
  ამოღებულია დეტერმინისტულად `(algorithm || 0x00 || key_bytes)`-ით დახარისხების შემდეგ.
- ორობითი კონტროლერის დატვირთვა (`ControllerPayload::Multisig`) კოდირებს
  `version:u8`, `threshold:u16`, `member_count:u8`, შემდეგ თითოეული წევრის
  `(curve_id, weight:u16, key_len:u16, key_bytes)`. ეს არის ზუსტად ის
  `AccountAddress::canonical_bytes()` წერს canonical Katakana i105 / non-canonical Katakana i105 იტვირთება.
- ჰეშინგი (`MultisigPolicy::digest_blake2b256()`) იყენებს Blake2b-256-ს
  `iroha-ms-policy` პერსონალიზაციის სტრიქონი, რათა მმართველობის მანიფესტებმა შეაერთონ ა
  დეტერმინისტული პოლიტიკის ID, რომელიც ემთხვევა I105-ში ჩაშენებულ კონტროლერის ბაიტებს.
- მოწყობილობების დაფარვა ცხოვრობს `fixtures/account/address_vectors.json`-ში (ქეისები
  `addr-multisig-*`). საფულეები და SDK-ები უნდა ამტკიცებდნენ კანონიკურ I105 სტრიქონებს
  ქვემოთ, რათა დაადასტურონ, რომ მათი შიფრები ემთხვევა Rust-ის განხორციელებას.

| საქმის ID | ბარიერი / წევრები | I105 ლიტერალი (პრეფიქსი `0x02F1`) | სორა შეკუმშული (`sora`) ლიტერალური | შენიშვნები |
|---------|--------------------------------------------------|-----------------------|
| `addr-multisig-council-threshold3` | `≥3` წონა, წევრები `(2,1,1)` | `SRfSHsrH3tEmYaaAYyD248F3vfT1oQ3WEGS22MaD8W9bLefF7rsoKLYGcpbcM9EcSus5ZhCAZU7ztn2BCsyeCAdfRncAVmVsipd4ibk6CBLF3Nrzcw8P7VKJg6mtFgEhWVTjfDkUMoc63oeEmaWyV6cyiphwk8ZgKAJUe4TyVtmKm1WWcg7qZ6i` | `sora3vﾑ2zkaoUwﾋﾅGﾘﾚyﾂe3ﾖfﾙヰｶﾘﾉwｷnoWﾛYicaUr3ﾔｲﾖ2Ado3TﾘYQﾉJqﾜﾇｳﾑﾐd8dDjRGｦ3Vﾃ9HcﾀMヰR8ﾎﾖgEqGｵEｾDyc5ﾁ1ﾔﾉ31sUﾑﾀﾖaｸxﾘ3ｲｷMEuFｺｿﾉBQSVQnxﾈeJzrXLヰhｿｹ5SEEﾅPﾂﾗｸdヰﾋ1bUGHｲVXBWNNJ6K` | საბჭოს დომენის მმართველობის კვორუმი. |
| `addr-multisig-wonderland-threshold2` | `≥2`, წევრები `(1,2)` | `3xsmkps1KPBn9dtpE5qHRhHEZCpiAe8d9j6H9A42TV6kc1TpaqdwnSksKgQrsSEHznqvWKBMc1os69BELzkLjsR7EV2gjV14d9JMzo97KEmYoKtxCrFeKFAcy7ffQdboV1uRt` | `sora2ﾖZﾘeｴAdx3ﾂﾉﾔXhnｹﾀ2ﾉｱﾋxﾅﾄﾌヱwﾐmﾊvEﾐCﾏﾎｦ1ﾑHﾋso2GKﾔﾕﾁwﾂﾃP6ﾁｼﾙﾖｺ9ｻｦbﾈ4wFdﾑFヰ3HaﾘｼMｷﾌHWtｷﾋLﾙﾖQ4D3XﾊﾜXmpktﾚｻ5ﾅﾅﾇ1gkﾏsCFQGH9` | ორმაგი ხელმოწერის საოცრებათა ქვეყნის მაგალითი (წონა 1 + 2). |
| `addr-multisig-default-quorum3` | `≥3`, წევრები `(1,1,1,1)` | `nA2bDNhMqXz7ERkHNoEWbvJGyR1aDRsw32LaUWLgbK3vcpzohmdFCLvdotxUWWDY3aZeX4ptLk4Z6TjF5ossnJm8VrNo6daxmGTkqUyP4MxJxiNyPFxsEE5DLnsoLWUcxaWNpZ76tmkbiGS31Gv8tejKpuiHUMaQ1s5ohWyZvDnpycNkBK8AEfGJqn5yc9zAzfWbVhpDwkPj8ScnzvH1Echr5` | `soraﾐ38ﾅｴｸﾜ8ﾃzwBrqﾘｺ4yﾄv6kqJp1ｳｱﾛｿrzﾄﾃﾘﾒRﾗtV9ｼﾔPｽcヱEﾌVVVｼﾘｲZAｦﾓﾅｦeﾒN76vﾈcuｶuﾛL54rzﾙﾏX2zMﾌRLﾃﾋpﾚpｲcHﾑﾅﾃﾔzｵｲVfAﾃﾚﾎﾚCヰﾔｲｽｦw9ﾔﾕ8bGGkﾁ6sNｼaｻRﾖﾜYﾕﾚU18ﾅHヰﾌuMeﾊtﾂrｿj95Ft8ﾜ3fﾄkNiｴuﾈrCﾐQt8ヱｸｸmﾙﾒgUbﾑEKTTCM` | იმპლიციტური ნაგულისხმევი დომენის კვორუმი, რომელიც გამოიყენება ბაზის მართვისთვის.

#### 2.4 წარუმატებლობის წესები (ADDR-1a)

- საჭირო სათაურზე მოკლე დატვირთვა + სელექტორი ან დარჩენილი ბაიტით გამოსცემს `AccountAddressError::InvalidLength` ან `AccountAddressError::UnexpectedTrailingBytes`.
- სათაურები, რომლებიც აყენებენ დაჯავშნილ `ext_flag`-ს ან აქვეყნებენ რეკლამას მხარდაუჭერელ ვერსიებს/კლასებს, უნდა უარყოთ `UnexpectedExtensionFlag`, `InvalidHeaderVersion`, ან `UnknownAddressClass` გამოყენებით.
- ამორჩევის/კონტროლერის უცნობი ტეგები ამაღლებს `UnknownDomainTag` ან `UnknownControllerTag`.
- დიდი ზომის ან არასწორი ფორმის საკვანძო მასალა ამაღლებს `KeyPayloadTooLong` ან `InvalidPublicKey`.
- Multisig კონტროლერები, რომლებიც აღემატება 255 წევრს, ზრდის `MultisigMemberOverflow`.
- IME/NFKC კონვერტაციები: ნახევრად სიგანის Sora kana შეიძლება ნორმალიზდეს მათი სრული სიგანის ფორმებამდე დეკოდირების დარღვევის გარეშე, მაგრამ ASCII `sora` სენტინელი და I105 ციფრები/ასოები უნდა დარჩეს ASCII. სრულ სიგანეზე ან დაკეცილი სენტინელების ზედაპირზე `ERR_MISSING_COMPRESSED_SENTINEL`, სრული სიგანის ASCII ტვირთამწეობა ამაღლებს `ERR_INVALID_COMPRESSED_CHAR` და საკონტროლო ჯამის შეუსაბამობები ბუშტუკდება, როგორც `ERR_CHECKSUM_MISMATCH`. საკუთრების ტესტები `crates/iroha_data_model/src/account/address.rs`-ში მოიცავს ამ ბილიკებს, ასე რომ SDK-ები და საფულეები შეიძლება დაეყრდნონ განმსაზღვრელ წარუმატებლობებს.
- Torii და `name@dataspace` or `name@domain.dataspace` მეტსახელების Torii და SDK ანალიზები ახლა ასხივებენ იგივე `ERR_*` კოდებს, როდესაც I105 (სასურველია)/სორა (მეორე საუკეთესო) შეყვანები ვერ მოხერხდება, სანამ მეტსახელის შეიძლება ხელახლა გამოაცხადოს დომენის შეცდომის მიზეზები (მაგ. პროზაული სიმებიდან გამოცნობის გარეშე.
- ლოკალური სელექტორი 12 ბაიტზე მოკლეა `ERR_LOCAL8_DEPRECATED` ზედაპირის დატვირთვით, რაც ინარჩუნებს მყარ ნაწილს ძველი Local‑8 დაჯესტებიდან.
- Domainless canonical Katakana i105 literals decode directly to a domainless `AccountId`. Use `ScopedAccountId` only when an interface requires explicit domain context.

#### 2.5 ნორმატიული ორობითი ვექტორები

- ** ნაგულისხმევი ნაგულისხმევი დომენი (`default`, დაწყებული ბაიტი `0x00`)**  
  კანონიკური ექვსკუთხედი: `0x020001203b6a27bcceb6a42d62a3a8d02a6f0d73653215771de243a63ac048a18b59da29`.  
  დაშლა: `0x02` სათაური, `0x00` ამომრჩევი (იგულისხმება ნაგულისხმევი), `0x00` კონტროლერის ტეგი, `0x01` მრუდის ID (Ed25519), I18NI00000002 კლავიშის სიგრძე, რომელსაც მოჰყვება კლავიშის სიგრძე-32.
- **ლოკალური დომენის დაიჯესტი (`treasury`, დაწყებული ბაიტი `0x01`)**  
  კანონიკური ექვსკუთხედი: `0x0201b18fe9c1abbac45b3e38fc5d0001208a88e3dd7409f195fd52db2d3cba5d72ca6709bf1d94121bf3748801b40f6f5c`.  
  დაყოფა: `0x02` სათაური, ამომრჩევი ტეგი `0x01` პლუს დაიჯესტი `b1 8f e9 c1 ab ba c4 5b 3e 38 fc 5d`, რასაც მოჰყვება ერთი გასაღების დატვირთვა (`0x00` ტეგი, I18NI000002861, სიგრძე I2080X 32-ბაიტი Ed25519 გასაღები).ერთეულის ტესტები (`account::address::tests::parse_encoded_accepts_all_formats`) ამტკიცებს V1 ვექტორებს ქვემოთ `AccountAddress::parse_encoded`-ის მეშვეობით, რაც გარანტიას იძლევა, რომ ინსტრუმენტები შეიძლება დაეყრდნოს კანონიკურ დატვირთვას ექვსკუთხა, I105 (სასურველი) და შეკუმშული (`sora`, მეორე საუკეთესო) ფორმებით. განაახლეთ გაფართოებული მოწყობილობების ნაკრები `cargo run -p iroha_data_model --example address_vectors`-ით.

| დომენი | სათესლე ბაიტი | კანონიკური hex | შეკუმშული (`sora`) |
|-------------|-------------------------------------------- --------------------------------------------|------------|
| ნაგულისხმევი | `0x00` | `0x020001203b6a27bcceb6a42d62a3a8d02a6f0d73653215771de243a63ac048a18b59da29` | `sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE` |
| ხაზინა | `0x01` | `0x0201b18fe9c1abbac45b3e38fc5d0001208a88e3dd7409f195fd52db2d3cba5d72ca6709bf1d94121bf3748801b40f6f5c` | `sora5ｻu6rﾀCヰTGwﾏ1ﾅヱﾌQｲﾖﾇqCｦヰﾓZQCZRDSSﾅMｱﾙヱｹﾁｸ8ｾeﾄﾛ6C8bZuwﾗｹCZｦRSLQFU` |
| საოცრებათა ქვეყანა | `0x02` | `0x0201b8ae571b79c5a80f5834da2b0001208139770ea87d175f56a35466c34c7ecccb8d8a91b4ee37a25df60f5b8fc9b394` | `sora5ｻwﾓyRｿqﾏnMﾀﾙヰKoﾒﾇﾓQｺﾛyｼ3ｸFHB2F5LyPﾐTMZkｹｼw67ﾋVﾕｻr8ﾉGﾇeEnｻVRNKCS` |
| იროჰა | `0x03` | `0x0201de8b36819700c807083608e2000120ed4928c628d1c2c6eae90338905995612959273a5c63f93636c14614ac8737d1` | `sora5ｻﾜxﾀ7Vｱ7QFeｷMﾂLﾉﾃﾏﾓﾀTﾚgSav3Wnｱｵ4ｱCKｷﾛMﾘzヰHiﾐｱ6ﾃﾉﾁﾐZmﾇ2fiﾎX21P4L` |
| ალფა | `0x04` | `0x020146be2154ae86826a3fef0ec0000120ca93ac1705187071d67b83c7ff0efe8108e8ec4530575d7726879333dbdabe7c` | `sora5ｻ9JヱﾈｿuwU6ｴpﾔﾂﾈRqRTds1HﾃﾐｶLVﾍｳ9ﾔhｾNｵVｷyucEﾒGﾈﾏﾍ9sKeﾉDzrｷﾆ742WG1` |
| ომეგა | `0x05` | `0x0201390d946885bc8416b3d30c9d0001206e7a1cdd29b0b78fd13af4c5598feff4ef2a97166e3ca6f2e4fbfccd80505bf1` | `sora5ｻ3zrﾌuﾚﾄJﾑXQhｸTyN8pzwRkWxmjVﾗbﾚﾕヰﾈoｽｦｶtEEﾊﾐ6GPｿﾓﾊｾEhvPｾｻ3XAJ73F` |
| მმართველობა | `0x06` | `0x0201989eb45a80940d187e2c908f0001208a875fff1eb38451577acd5afee405456568dd7c89e090863a0557bc7af49f17` | `sora5ｻiｵﾁyVﾕｽbFpDHHuﾇﾉdﾗｲﾓﾄRﾋAW3frUCｾ5ｷﾘTwdﾚnｽtQiLﾏｼｶﾅXgｾZmﾒヱH58H4KP` |
| ვალიდიატორები | `0x07` | `0x0201e4ffa58704c69afaeb7cc2d7000120ea4a6c63e29c520abef5507b132ec5f9954776aebebe7b92421eea691446d22c` | `sora5ｻﾀLDH6VYﾑNAｾgﾉVﾜtxﾊRXLｹﾍﾔﾌLd93GﾔGeｴﾄYrs1ﾂHｸkYxｹwｿyZﾗxyﾎZoXT1S4N` |
| მკვლევარი | `0x08` | `0x02013b35422c65c2a83c99c523ad0001201398f62c6d1a457c51ba6a4b5f3dbd2f69fca93216218dc8997e416bd17d93ca` | `sora5ｻ4nmｻaﾚﾚPvNLgｿｱv6MHDeEyﾀovﾉJcpvrﾖ6ﾈCQcCNﾇﾜhﾚﾖyFdTwｸｶHEｱ9rWU8FMB` |
| სორანეტი | `0x09` | `0x0201047d9ea7f5d5dbec3f7bfc58000120fd1724385aa0c75b64fb78cd602fa1d991fdebf76b13c58ed702eac835e9f618` | `sora5ｱｸヱVQﾂcﾁヱRﾓcApｲﾁﾅﾒvﾌﾏfｾNnﾛRJsｿDhﾙuHaﾚｺｦﾌﾍﾈeﾆﾎｺN1UUDｶ6ﾎﾄﾛoRH8JUL` |
| kitsune | `0x0A` | `0x0201e91933de397fd7723dc9a76c00012043a72e714401762df66b68c26dfbdf2682aaec9f2474eca4613e424a0fbafd3c` | `sora5ｻﾚｺヱkfFJfSﾁｼJwﾉLvbpSｷﾔMWFMrbｳｸｲｲyヰKGJﾉｻ4ｹﾕrｽhｺｽzSDヰXAN62AD7RGNS` |
| da | `0x0B` | `0x02016838cf5bb0ce0f3d4f380e1c00012066be7e332c7a453332bd9d0a7f7db055f5c5ef1a06ada66d98b39fb6810c473a` | `sora5ｻNﾒ5SﾐRﾉﾐﾃ62ｿ1ｶｷWFKyF1BcAﾔvｼﾐHqﾙﾐPﾏｴヰ5tｲﾕvnﾙT6ﾀW7mﾔ7ﾇﾗﾂｳ25CXS93` |

განხილული მიერ: მონაცემთა მოდელი WG, კრიპტოგრაფიის WG — ფარგლები დამტკიცებულია ADDR-1a-სთვის.

##### Sora Nexus მითითების მეტსახელები

Sora Nexus ქსელები ნაგულისხმევად არის `chain_discriminant = 0x02F1`
(`iroha_config::parameters::defaults::common::CHAIN_DISCRIMINANT`). The
ამიტომ `AccountAddress::to_i105` და `to_i105` დამხმარეები ასხივებენ
თანმიმდევრული ტექსტური ფორმები ყოველი კანონიკური დატვირთვისთვის. არჩეული მოწყობილობებიდან
`fixtures/account/address_vectors.json` (გენერირებული მეშვეობით
`cargo xtask address-vectors`) ნაჩვენებია ქვემოთ სწრაფი მითითებისთვის:

| ანგარიში / სელექტორი | I105 ლიტერალი (პრეფიქსი `0x02F1`) | სორა შეკუმშული (`sora`) ლიტერალური |
|------------------ |
| `default` დომენი (იმპლიციტური სელექტორი, თესლი `0x00`) | `soraゴヂアニィルサフユイサヹピビレッデヹボテハキョメベチュヒャネィギチュヲベァヱェベモネェネツデトツオチハセ` | `sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE` |
| `treasury` (ადგილობრივი დაიჯესტის სელექტორი, თესლი `0x01`) | `34mSYnCXkCzHXm31UDHh7SJfGvC4QPEhwim8z7sys2iHqXpCwCQkjL8KHvkFLSs1vZdJcb37r` | `sora5ｻu6rﾀCヰTGwﾏ1ﾅヱﾌQｲﾖﾇqCｦヰﾓZQCZRDSSﾅMｱﾙヱｹﾁｸ8ｾeﾄﾛ6C8bZuwﾗｹCZｦRSLQFU` |
| გლობალური რეესტრის მაჩვენებელი (`registry_id = 0x0000_002A`, `treasury`-ის ექვივალენტი) | `3oE9sLeRGP49Cu7mQ1nF4wtKAm29BG4TGLiRsaXe7mhbMP5WZ113nNW1N6RbqF` | `sorakXｹ6NｻﾍﾀﾖSﾜﾖｱ3ﾚ5WﾘﾋQﾅｷｦxgﾛｸcﾁｵﾋkﾋvﾏ8SPﾓﾀｹdｴｴｲW9iCM6AEP` |

ეს სტრიქონები ემთხვევა CLI-ს (`iroha tools address convert`), Torii-ის მიერ გამოშვებულ სტრიქონებს
პასუხები (`canonical Katakana i105 literal rendering`) და SDK დამხმარეები, ამიტომ UX დააკოპირეთ/ჩასვით
ნაკადებს შეუძლიათ სიტყვასიტყვით დაეყრდნონ მათ. დაუმატეთ `<address>@<domain>` (rejected legacy form) მხოლოდ მაშინ, როდესაც გჭირდებათ მკაფიო მარშრუტიზაციის მინიშნება; სუფიქსი არ არის კანონიკური გამომავალი ნაწილი.

#### 2.6 ტექსტური მეტსახელები თავსებადობისთვის (დაგეგმილი)

- **ჯაჭვის ალიასის სტილი:** `ih:<chain-alias>:<name@domain.dataspace>` მორებისა და ადამიანებისთვის
  შესვლა. საფულეებმა უნდა გააანალიზონ პრეფიქსი, დაადასტურონ ჩაშენებული ჯაჭვი და დაბლოკონ
  შეუსაბამობები.
- **CAIP-10 ფორმა:** `iroha:<caip-2-id>:<i105-addr>` ჯაჭვის აგნოსტიკისთვის
  ინტეგრაციები. ეს რუკა **ჯერ არ არის დანერგილი** გაგზავნილში
  ხელსაწყოების ჯაჭვები.
- **მანქანის დამხმარეები:** გამოაქვეყნეთ კოდეკები Rust, TypeScript/JavaScript, Python,
  და კოტლინი, რომელიც მოიცავს I105 და შეკუმშულ ფორმატებს (`AccountAddress::to_i105`,
  `AccountAddress::parse_encoded` და მათი SDK ეკვივალენტები). CAIP-10 დამხმარეები არიან
  მომავალი სამუშაო.

#### 2.7 დეტერმინისტული I105 მეტსახელი

- **პრეფიქსის რუქა:** ხელახლა გამოიყენეთ `chain_discriminant`, როგორც I105 ქსელის პრეფიქსი.
  `encode_i105_prefix()` (იხ. `crates/iroha_data_model/src/account/address.rs`)
  გამოსცემს 6-ბიტიან პრეფიქსს (ერთი ბაიტი) `<64` მნიშვნელობებისთვის და 14-ბიტიანი, ორ ბაიტისთვის
  ფორმა უფრო დიდი ქსელებისთვის. ავტორიტეტული დავალებები ცხოვრობს
  [`address_prefix_registry.md`](source/references/address_prefix_registry.md);
  SDK-ებმა უნდა შეინახონ შესაბამისი JSON რეესტრი სინქრონიზებული, რათა თავიდან აიცილონ შეჯახება.
- **ანგარიშის მასალა:** I105 შიფრავს კანონიკურ დატვირთვას, რომელიც აშენებულია
  `AccountAddress::canonical_bytes()` — სათაურის ბაიტი, დომენის ამომრჩევი და
  კონტროლერის დატვირთვა. არ არსებობს დამატებითი ჰეშირების ნაბიჯი; I105 ათავსებს
  ორობითი კონტროლერის ტვირთამწეობა (ერთი ღილაკი ან მულტისიგ), როგორც წარმოებულია Rust-ის მიერ
  ენკოდერი და არა CTAP2 რუკა, რომელიც გამოიყენება მრავალრიცხოვანი პოლიტიკის შეჯამებისთვის.
- ** დაშიფვრა:** `encode_i105()` აერთიანებს პრეფიქსის ბაიტებს კანონიკურს
  payload და აერთებს 16-ბიტიან საკონტროლო ჯამს, რომელიც მიღებულია Blake2b-512-დან ფიქსირებული
  პრეფიქსი `I105PRE` (`b"I105PRE"` || prefix || payload). შედეგი კოდირდება `bs58`-ით I105 ანბანის გამოყენებით.
  CLI/SDK დამხმარეები ავლენენ იმავე პროცედურას და `AccountAddress::parse_encoded`
  აბრუნებს მას `decode_i105`-ის საშუალებით.

#### 2.8 ნორმატიული ტექსტური ტესტის ვექტორები

`fixtures/account/address_vectors.json` შეიცავს სრულ I105 (სასურველია) და შეკუმშული (`sora`, მეორე საუკეთესო)
ლიტერალები ყოველი კანონიკური დატვირთვისთვის. მაჩვენებლები:

- **`addr-single-default-ed25519` (Sora Nexus, პრეფიქსი `0x02F1`).**  
  I105 `soraゴヂアニィルサフユイサヹピビレッデヹボテハキョメベチュヒャネィギチュヲベァヱェベモネェネツデトツオチハセ`, შეკუმშული (`sora`)
  `sora2QG…U4N5E5`. Torii ასხივებს ზუსტად ამ სტრიქონებს `AccountId`-დან
  `Display` განხორციელება (კანონიკური I105) და `AccountAddress::to_i105`.
- **`addr-global-registry-002a` (რეესტრის სელექტორი → ხაზინა).**  
  I105 `3oE9sLeRGP49Cu7mQ1nF4wtKAm29BG4TGLiRsaXe7mhbMP5WZ113nNW1N6RbqF`, შეკუმშული (`sora`)
  `sorakX…CM6AEP`. აჩვენებს, რომ რეესტრის სელექტორები კვლავ დეკოდირდება
  იგივე კანონიკური დატვირთვა, როგორც შესაბამისი ადგილობრივი დაიჯესტი.
- **შეცდომის შემთხვევა (`i105-prefix-mismatch`).**  
  I105 ლიტერალის ანალიზი, რომელიც კოდირებულია პრეფიქსით `NETWORK_PREFIX + 1` კვანძზე
  ველოდები ნაგულისხმევი პრეფიქსის გამომუშავებას
  `AccountAddressError::UnexpectedNetworkPrefix { expected: 753, found: 754 }`
  დომენის მარშრუტიზაციის მცდელობამდე. `i105-checksum-mismatch` მოწყობილობა
  ახორციელებს ჩარევის გამოვლენას Blake2b საკონტროლო ჯამზე.

#### 2.9 შესაბამისობის მოწყობილობები

ADDR‑2 აგზავნის ხელახლა სათამაშო მოწყობილობების პაკეტს, რომელიც მოიცავს დადებით და უარყოფითს
სცენარები კანონიკურ ექვსკუთხედში, I105 (სასურველია), შეკუმშული (`sora`, ნახევარი/სრული სიგანე), იმპლიციტური
ნაგულისხმევი სელექტორები, გლობალური რეესტრის მეტსახელები და მრავალხელმოწერის კონტროლერები. The
კანონიკური JSON ცხოვრობს `fixtures/account/address_vectors.json`-ში და შეიძლება იყოს
რეგენერირებულია:

```
cargo xtask address-vectors --out fixtures/account/address_vectors.json
# verify without writing:
cargo xtask address-vectors --verify
```

ad-hoc ექსპერიმენტებისთვის (სხვადასხვა გზა/ფორმატი) მაგალითი ბინარულია
ხელმისაწვდომია:

```
cargo run -p iroha_data_model --example account_address_vectors > fixtures/account/address_vectors.json
```

ჟანგის ერთეულის ტესტები `crates/iroha_data_model/tests/account_address_vectors.rs`-ში
და `crates/iroha_torii/tests/account_address_vectors.rs`, JS-თან ერთად,
Swift და Android აღკაზმულობა (`javascript/iroha_js/test/address.test.js`,
`IrohaSwift/Tests/IrohaSwiftTests/AccountAddressTests.swift`,
`java/iroha_android/src/test/java/org/hyperledger/iroha/android/address/AccountAddressTests.java`),
მოიხმარეთ იგივე მოწყობილობა, რათა უზრუნველყოთ კოდეკის პარიტეტი SDK-ებში და Torii დაშვება.

### 3. გლობალურად უნიკალური დომენები და ნორმალიზაცია

აგრეთვე იხილეთ: [`docs/source/references/address_norm_v1.md`](source/references/address_norm_v1.md)
კანონიკური Norm v1 მილსადენისთვის, რომელიც გამოიყენება Torii-ში, მონაცემთა მოდელსა და SDK-ებზე.

ხელახლა განსაზღვრეთ `DomainId`, როგორც ტეგირებული ტოპი:

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

`LocalChain` ახვევს არსებულ სახელს დომენებისთვის, რომელსაც მართავს მიმდინარე ჯაჭვი.
როდესაც დომენი რეგისტრირებულია გლობალური რეესტრის მეშვეობით, ჩვენ ვაგრძელებთ საკუთრებას
ჯაჭვის დისკრიმინანტი. ჩვენება/პარსინგი ამჟამად უცვლელი რჩება, მაგრამ
გაფართოებული სტრუქტურა იძლევა გადაწყვეტილებების მარშრუტიზაციის საშუალებას.

#### 3.1 ნორმალიზება და გაყალბების დაცვა

ნორმა v1 განსაზღვრავს კანონიკურ მილსადენს, რომელიც ყველა კომპონენტმა უნდა გამოიყენოს დომენამდე
სახელი შენარჩუნებულია ან ჩართულია `AccountAddress`-ში. სრული გავლა
ცხოვრობს [`docs/source/references/address_norm_v1.md`]-ში (source/references/address_norm_v1.md);
ქვემოთ მოცემული შეჯამება აღწერს საფულეების, Torii, SDK-ების და მართვის საფეხურებს
ინსტრუმენტები უნდა განხორციელდეს.

1. **შეყვანის ვალიდაცია.** უარყოთ ცარიელი სტრიქონები, უფსკრული და რეზერვირებული
   დელიმიტერები `@`, `#`, `$`. ეს ემთხვევა იმ ინვარიანტებს, რომლებიც აღსრულდა
   `Name::validate_str`.
2. ** Unicode NFC შემადგენლობა. ** გამოიყენეთ ICU მხარდაჭერილი NFC ნორმალიზება ასე კანონიკურად
   ეკვივალენტური თანმიმდევრობები დეტერმინისტულად იშლება (მაგ., `e\u{0301}` → `é`).
3. **UTS-46 ნორმალიზაცია.** გაუშვით NFC გამომავალი UTS‑46-ით
   `use_std3_ascii_rules = true`, `transitional_processing = false` და
   DNS სიგრძის აღსრულება ჩართულია. შედეგი არის მცირე ასოების A-ლეიბლის თანმიმდევრობა;
   შეყვანები, რომლებიც არღვევს STD3 წესებს, აქ ვერ ხერხდება.
4. **სიგრძის ლიმიტები.** დაიცავით DNS სტილის საზღვრები: თითოეული ლეიბლი უნდა იყოს 1–63
   ბაიტი და სრული დომენი არ უნდა აღემატებოდეს 255 ბაიტს 3 ნაბიჯის შემდეგ.
5. **სურვილისამებრ დამაბნეველი პოლიტიკა.** UTS‑39 სკრიპტის შემოწმება ხდება
   ნორმა v2; ოპერატორებს შეუძლიათ მათი ადრეული ჩართვა, მაგრამ შემოწმების წარუმატებლობის შემთხვევაში უნდა შეწყდეს
   დამუშავება.

თუ ყველა ეტაპი წარმატებულია, მცირე ასოების A-ლეიბლის სტრიქონი ინახება და გამოიყენება
მისამართის კოდირება, კონფიგურაცია, მანიფესტები და რეესტრის ძიება. ადგილობრივი დაიჯესტი
სელექტორები იღებენ თავიანთ 12-ბაიტიან მნიშვნელობას, როგორც `blake2s_mac(key = "SORA-LOCAL-K:v1",
კანონიკური_ლეიბლი)[0..12]` ნაბიჯის 3 გამოსავლის გამოყენებით. ყველა სხვა მცდელობა (შერეული
case, upper-case, unicode input) უარყოფილია სტრუქტურირებულით
`ParseError`s საზღვარზე, სადაც სახელი იყო მოწოდებული.

კანონიკური მოწყობილობები, რომლებიც აჩვენებენ ამ წესებს - მათ შორის, ორმხრივი მოგზაურობის კოდის ჩათვლით
და STD3 არასწორი თანმიმდევრობები - ჩამოთვლილია
`docs/source/references/address_norm_v1.md` და ასახულია SDK CI-ში
ვექტორული კომპლექტები თვალყურის დევნება ADDR-2-ით.

### 4. Nexus დომენის რეესტრი და მარშრუტიზაცია- ** რეესტრის სქემა: ** Nexus ინახავს ხელმოწერილ რუკას `DomainName -> ChainRecord`
  სადაც `ChainRecord` მოიცავს ჯაჭვის დისკრიმინანტს, არჩევით მეტამონაცემებს (RPC
  საბოლოო წერტილები) და უფლებამოსილების დამადასტურებელი საბუთი (მაგ. მმართველობის მრავალხელმოწერა).
- **სინქრონიზაციის მექანიზმი:**
  - ჯაჭვები წარადგენენ ხელმოწერილი დომენის პრეტენზიებს Nexus-ზე (დაბადების დროს ან მეშვეობით
    მართვის ინსტრუქცია).
  - Nexus აქვეყნებს პერიოდულ მანიფესტებს (ხელმოწერილი JSON პლუს არჩევითი Merkle root)
    HTTPS-ზე და კონტენტ-მისამართიან მეხსიერებაზე (მაგ., IPFS). კლიენტები ამაგრებენ
    უახლესი მანიფესტი და ხელმოწერების გადამოწმება.
- ** საძიებო ნაკადი:**
  - Torii იღებს ტრანზაქციას `DomainId`-ზე მითითებით.
  - თუ დომენი ლოკალურად უცნობია, Torii ითხოვს ქეშირებულ Nexus მანიფესტს.
  - თუ მანიფესტი მიუთითებს უცხოურ ჯაჭვზე, ტრანზაქცია უარყოფილია
    განმსაზღვრელი `ForeignDomain` შეცდომა და დისტანციური ჯაჭვის ინფორმაცია.
  - თუ დომენი აკლია Nexus-ს, Torii აბრუნებს `UnknownDomain`-ს.
- ** ნდობის წამყვანები და როტაცია: ** მმართველობის კლავიშების ნიშანი; როტაცია ან
  გაუქმება გამოქვეყნებულია ახალი მანიფესტის ჩანაწერის სახით. კლიენტები ახორციელებენ მანიფესტს
  TTL-ები (მაგ., 24 სთ) და უარი თქვან ძველ მონაცემებზე კონსულტაციაზე ამ ფანჯრის მიღმა.
- ** წარუმატებლობის რეჟიმები:** თუ მანიფესტის მოძიება ვერ მოხერხდა, Torii ბრუნდება ქეშზე
  მონაცემები TTL-ის ფარგლებში; წარსულში TTL ის ასხივებს `RegistryUnavailable` და უარს ამბობს
  ჯვარედინი დომენური მარშრუტიზაცია არათანმიმდევრული მდგომარეობის თავიდან ასაცილებლად.

### 4.1 რეესტრის უცვლელობა, მეტსახელები და საფლავის ქვები (ADDR-7c)

Nexus აქვეყნებს **მხოლოდ დანართის მანიფესტს**, ასე რომ, ყველა დომენის ან მეტსახელის მინიჭება
შესაძლებელია აუდიტის და ხელახლა დაკვრა. ოპერატორებმა უნდა დაამუშავონ პაკეტში აღწერილი
[address manifest runbook](source/runbooks/address_manifest_ops.md) როგორც
სიმართლის ერთადერთი წყარო: თუ მანიფესტი აკლია ან ვერ დამოწმებულია, Torii უნდა
უარი თქვას დაზარალებული დომენის გადაჭრაზე.

ავტომატიზაციის მხარდაჭერა: `cargo xtask address-manifest verify --bundle <current_dir> --previous <previous_dir>`
იმეორებს საკონტროლო ჯამს, სქემას და წინა დაჯესტის შემოწმებებს, რომლებიც გაწერილია ში
runbook. ჩართეთ ბრძანების გამომავალი ცვლილების ბილეთებში, რათა ნახოთ `sequence`
და `previous_digest` კავშირი დამოწმებული იყო ნაკრების გამოქვეყნებამდე.

#### მანიფესტის სათაურის და ხელმოწერის კონტრაქტი

| ველი | მოთხოვნა |
|-------|-------------|
| `version` | ამჟამად `1`. Bump მხოლოდ შესაბამისი სპეციფიკაციის განახლებით. |
| `sequence` | ზრდა **ზუსტად** ერთით თითო პუბლიკაციაზე. Torii ქეში უარს ამბობს შესწორებაზე ხარვეზებით ან რეგრესიებით. |
| `generated_ms` + `ttl_hours` | დააყენეთ ქეშის სიახლე (ნაგულისხმევი 24 სთ). თუ TTL იწურება მომდევნო გამოქვეყნებამდე, Torii გადადის `RegistryUnavailable`-ზე. |
| `previous_digest` | BLAKE3 დაიჯესტი (ექვსკუთხა) წინა მანიფესტი სხეულის. ვერიფიკატორები ხელახლა გამოთვლიან მას `b3sum`-ით უცვლელობის დასამტკიცებლად. |
| `signatures` | მანიფესტები ხელმოწერილია Sigstore (`cosign sign-blob`) მეშვეობით. Ops-მა უნდა გაუშვას `cosign verify-blob --bundle manifest.sigstore manifest.json` და განახორციელოს მმართველობითი იდენტობის/გამომცემის შეზღუდვები გაშვებამდე. |

გამოშვების ავტომატიზაცია ასხივებს `manifest.sigstore` და `checksums.sha256`
JSON სხეულის გვერდით. შეინახეთ ფაილები ერთად SoraFS-ზე ან
HTTP ბოლო წერტილები, რათა აუდიტორებმა შეძლონ გადამოწმების ნაბიჯების სიტყვასიტყვით გამეორება.

#### შესვლის ტიპები

| ტიპი | დანიშნულება | აუცილებელი ველები |
|------|---------|-----------------|
| `global_domain` | აცხადებს, რომ დომენი რეგისტრირებულია გლობალურად და უნდა განისაზღვროს ჯაჭვის დისკრიმინატორთან და I105 პრეფიქსით. | `{ "domain": "<label>", "chain": "sora:nexus:global", "i105_prefix": 753, "selector": "global" }` |
| `tombstone` | სამუდამოდ აცილებს ალიასს/სელექტორს. საჭიროა Local‑8 დაიჯესტების წაშლის ან დომენის წაშლისას. | `{ "selector": {…}, "reason_code": "LOCAL8_RETIREMENT" \| …, "ticket": "<governance id>", "replaces_sequence": <number> }` |

`global_domain` ჩანაწერები შეიძლება შეიცავდეს არჩევითად `manifest_url` ან `sorafs_cid`
მიუთითოთ საფულეები ხელმოწერილი ჯაჭვის მეტამონაცემებზე, მაგრამ კანონიკური ტოპი რჩება
`{domain, chain, discriminant/i105_prefix}`. `tombstone` ჩანაწერები **აუცილებელი ** ციტირება
სელექციონერი პენსიაზე გასული და ბილეთის/მმართველობის არტეფაქტი, რომელიც უფლებამოსილია
ცვლილება ისე, რომ აუდიტის ბილიკი რეკონსტრუქციული იყოს ოფლაინში.

#### მეტსახელი/საფლავის ქვის სამუშაო პროცესი და ტელემეტრია

1. ** დრიფტის ამოცნობა.** გამოიყენეთ `torii_address_local8_total{endpoint}`,
   `torii_address_local8_domain_total{endpoint,domain}`,
   `torii_address_collision_total{endpoint,kind="local12_digest"}`,
   `torii_address_collision_domain_total{endpoint,domain}`,
   `torii_address_domain_total{endpoint,domain_kind}` და
   `torii_address_invalid_total{endpoint,reason}` (გაყვანილია ქ
   `dashboards/grafana/address_ingest.json`) ადგილობრივი წარდგენის დასადასტურებლად და
   ლოკალური-12 შეჯახება რჩება ნულზე საფლავის ქვის შეთავაზებამდე. The
   თითო დომენის მრიცხველები მფლობელებს საშუალებას აძლევს დაამტკიცონ, რომ მხოლოდ dev/ტესტი დომენები ასხივებენ Local‑8
   ტრაფიკი (და Local‑12 შეჯახების რუკა ცნობილ დადგმულ დომენებზე) ხოლო
   მოიცავს **Domain Kind Mix (5m)** პანელს, რათა SRE-ებმა შეძლონ გრაფიკის დახატვა რამდენი
   `domain_kind="local12"` ტრაფიკი რჩება და `AddressLocal12Traffic`
   გაფრთხილება ხანძრის დროს, როდესაც წარმოება მაინც ხედავს Local-12 სელექტორებს, მიუხედავად იმისა
   საპენსიო კარიბჭე.
2. **გამოიყვანეთ კანონიკური დიჯესტები.** გაუშვით
   `iroha tools address convert <address> --format json --expect-prefix 753`
   (ან მოიხმარეთ `fixtures/account/address_vectors.json` მეშვეობით
   `scripts/account_fixture_helper.py`) ზუსტი `digest_hex`-ის გადასაღებად.
   CLI იღებს I105, `i105` და კანონიკურ `0x…` ლიტერალებს; დაურთოს
   `@<domain>` მხოლოდ მაშინ, როცა საჭიროა მანიფესტებისთვის ეტიკეტის შენარჩუნება.
   JSON რეზიუმე ასახავს ამ დომენს `input_domain` ველის მეშვეობით და
   `legacy  suffix` იმეორებს კონვერტირებულ დაშიფვრას, როგორც `<address>@<domain>` (rejected legacy form)
   manifest diffs (ეს სუფიქსი არის მეტამონაცემები და არა ანგარიშის კანონიკური ID).
   ახალ ხაზზე ორიენტირებული ექსპორტისთვის გამოიყენეთ
   `iroha tools address normalize --input <file> legacy-selector input mode` ლოკალური მასობრივი კონვერტაციისთვის
   ამომრჩეველები კანონიკურ I105 (სასურველი), შეკუმშული (`sora`, მეორე საუკეთესო), თექვსმეტობითი ან JSON ფორმებად გამოტოვებისას
   არალოკალური რიგები. როდესაც აუდიტორებს სჭირდებათ ელცხრილისთვის შესაფერისი მტკიცებულება, გაუშვით
   `iroha tools address audit --input <file> --format csv` CSV შეჯამების გამოსაცემად
   (`input,status,format,domain_kind,…`), რომელიც ხაზს უსვამს ადგილობრივ სელექტორებს,
   კანონიკური დაშიფვრები და იმავე ფაილში წარუმატებლობის გაანალიზება.
3. **დაამატეთ მანიფესტის ჩანაწერები.** შეადგინეთ `tombstone` ჩანაწერი (და შემდგომი
   `global_domain` ჩანაწერი გლობალურ რეესტრში მიგრაციისას) და დამოწმება
   მანიფესტი `cargo xtask address-vectors`-ით ხელმოწერების მოთხოვნამდე.
4. **გადაამოწმეთ და გამოაქვეყნეთ.** მიჰყევით runbook-ის საკონტროლო სიას (ჰეშები, Sigstore,
   თანმიმდევრობის ერთფეროვნება) პაკეტის SoraFS-ზე გადატანამდე. Torii ახლა
   ახდენს I105 (სასურველი)/სორას (მეორე-საუკეთესო) ლიტერალების კანონიკიზაციას შეკვრის დაშვებისთანავე.
5. **მონიტორი და უკან დაბრუნება.** შეინახეთ Local‑8 და Local‑12 შეჯახების პანელები
   ნულოვანი 30 დღის განმავლობაში; თუ რეგრესია გამოჩნდება, ხელახლა გამოაქვეყნეთ წინა მანიფესტი
   მხოლოდ ზემოქმედების ქვეშ მყოფ არასაწარმოო გარემოში ტელემეტრიის სტაბილიზაციამდე.

ყველა ზემოთ ჩამოთვლილი ნაბიჯი არის ADDR-7c სავალდებულო მტკიცებულება: ვლინდება გარეშე
`cosign` ხელმოწერის ნაკრები ან შესაბამისი `previous_digest` მნიშვნელობების გარეშე უნდა
უარყოფილი იქნება ავტომატურად და ოპერატორებმა უნდა დაურთოს დამადასტურებელი ჟურნალი
მათი შეცვლის ბილეთები.

### 5. საფულე და API ერგონომიკა

- ** ნაგულისხმევი ჩვენება: ** საფულეები აჩვენებს I105 მისამართს (მოკლე, შეჯამებული)
  პლუს გადაჭრილი დომენი, როგორც რეესტრიდან მოტანილი ლეიბლი. დომენები არის
  ნათლად აღინიშნება, როგორც აღწერილობითი მეტამონაცემები, რომლებიც შეიძლება შეიცვალოს, ხოლო I105 არის
  სტაბილური მისამართი.
- **შეყვანის კანონიკიზაცია:** Torii და SDK-ები მიიღებენ canonical Katakana i105 / non-canonical Katakana i105/0x
  მისამართები პლუს `name@dataspace` or `name@domain.dataspace`, `uaid:…` და
  `opaque:…` ფორმირდება, შემდეგ ხდება კანონიკიზაცია I105-ზე გამოსასვლელად. არ არსებობს
  მკაცრი რეჟიმის გადართვა; ნედლეული ტელეფონის/ელფოსტის იდენტიფიკატორები უნდა ინახებოდეს ჩანაწერში
  UAID/გაუმჭვირვალე რუკების საშუალებით.
- **შეცდომის პრევენცია:** საფულეები აანალიზებენ I105 პრეფიქსებს და აიძულებენ ჯაჭვის დისკრიმინაციას
  მოლოდინები. ჯაჭვის შეუსაბამობა იწვევს რთულ წარუმატებლობებს ქმედითი დიაგნოსტიკით.
- **კოდეკების ბიბლიოთეკები:** ოფიციალური Rust, TypeScript/JavaScript, Python და Kotlin
  ბიბლიოთეკები უზრუნველყოფენ I105 კოდირების/გაშიფვრის პლუს შეკუმშული (`sora`) მხარდაჭერას
  თავიდან აიცილოთ ფრაგმენტული განხორციელება. CAIP-10 კონვერტაციები ჯერ არ არის გაგზავნილი.

#### ხელმისაწვდომობისა და უსაფრთხო გაზიარების სახელმძღვანელო- პროდუქტის ზედაპირების დანერგვის ინსტრუქცია თვალყურს ადევნებს პირდაპირ ეთერში
  `docs/portal/docs/reference/address-safety.md`; მიმართეთ იმ საკონტროლო სიას, როდესაც
  ამ მოთხოვნების ადაპტირება საფულესთან ან Explorer UX-თან.
- **უსაფრთხო გაზიარების ნაკადები:** ზედაპირები, რომლებიც აკოპირებენ ან აჩვენებენ, მიმართავენ ნაგულისხმევს I105 ფორმას და ამჟღავნებენ მიმდებარე „გაზიარების“ მოქმედებას, რომელიც წარმოადგენს როგორც სრულ სტრიქონს, ასევე QR კოდს, რომელიც მიღებულია ერთი და იგივე დატვირთვიდან, რათა მომხმარებლებს შეეძლოთ შეამოწმონ საკონტროლო ჯამი ვიზუალურად ან სკანირებით. როდესაც შეკვეცა გარდაუვალია (მაგ., პატარა ეკრანები), შეინარჩუნეთ სტრიქონის დასაწყისი და დასასრული, დაამატეთ მკაფიო ელიფსები და შეინახეთ სრული მისამართი ხელმისაწვდომი ასლი ბუფერში, რათა თავიდან აიცილოთ შემთხვევითი ამოჭრა.
- **IME გარანტიები:** მისამართის შეყვანები უნდა უარყოს კომპოზიციის არტეფაქტები IME/IME სტილის კლავიატურებიდან. განახორციელეთ მხოლოდ ASCII ჩანაწერი, წარმოადგინეთ შიდა გაფრთხილება სრული სიგანის ან Kana სიმბოლოების აღმოჩენისას და შესთავაზეთ უბრალო ტექსტის ჩასმის ზონა, რომელიც ამოიღებს შერწყმულ ნიშნებს დადასტურებამდე, რათა იაპონელ და ჩინელ მომხმარებლებს შეეძლოთ გამორთონ IME პროგრესის დაკარგვის გარეშე.
- **ეკრანის წამკითხველის მხარდაჭერა:** უზრუნველყოს ვიზუალურად დამალული ლეიბლები (`aria-label`/`aria-describedby`), რომლებიც აღწერს I105 პრეფიქსის მთავარ ციფრებს და ანაწილებს I105 დატვირთვას 4 ან 8-სიმბოლოიან ჯგუფებად, ასე რომ, ჯგუფური დამხმარე ტექნოლოგია იკითხება. გამოაცხადეთ ასლის/გაზიარების წარმატება თავაზიანი ცოცხალი რეგიონების მეშვეობით და დარწმუნდით, რომ QR წინასწარი გადახედვები შეიცავს აღწერით ალტერნატიულ ტექსტს („I105 მისამართი <alias> ჯაჭვზე 0x02F1“).
- **მხოლოდ Sora-ზე შეკუმშული გამოყენება:** ყოველთვის მონიშნეთ `i105` შეკუმშული ხედი, როგორც „მხოლოდ Sora“ და დააკოპირეთ ის აშკარა დადასტურების უკან. SDK-ებმა და საფულეებმა უარი უნდა თქვან შეკუმშული გამოსავლის ჩვენებაზე, როდესაც ჯაჭვის დისკრიმინანტი არ არის Sora Nexus მნიშვნელობა და უნდა მიმართონ მომხმარებლებს უკან I105-ზე ქსელში გადარიცხვისთვის, რათა თავიდან იქნას აცილებული თანხების არასწორი მარშრუტი.

## განხორციელების ჩამონათვალი

- **I105 კონვერტი:** პრეფიქსი კოდირებს `chain_discriminant`-ს კომპაქტურის გამოყენებით
  6-/14-ბიტიანი სქემა `encode_i105_prefix()`-დან, სხეული არის კანონიკური ბაიტი
  (`AccountAddress::canonical_bytes()`) და საკონტროლო ჯამი არის პირველი ორი ბაიტი
  Blake2b-512(`b"I105PRE"` || პრეფიქსი || სხეული). სრული დატვირთვა კოდირდება `bs58`-ით I105 ანბანის გამოყენებით.
- ** რეესტრის კონტრაქტი: ** ხელმოწერილი JSON (და სურვილისამებრ Merkle root) გამოცემა
  `{discriminant, i105_prefix, chain_alias, endpoints}` 24 სთ TTL და
  ბრუნვის გასაღებები.
- **დომენის პოლიტიკა:** ASCII `Name` დღეს; თუ ჩართულია i18n, გამოიყენეთ UTS-46 ამისთვის
  ნორმალიზება და UTS-39 დამაბნეველი შემოწმებისთვის. მაქსიმალური ეტიკეტის აღსრულება (63) და
  მთლიანი (255) სიგრძე.
- **ტექსტური დამხმარეები:** გემი I105 ↔ შეკუმშული (`i105`) კოდეკები Rust-ში,
  TypeScript/JavaScript, Python და Kotlin საერთო ტესტის ვექტორებით (CAIP-10
  რუკები რჩება სამომავლო სამუშაოდ).
- **CLI tooling:** უზრუნველყოს დეტერმინისტული ოპერატორის სამუშაო ნაკადი `iroha tools address convert`-ის საშუალებით
  (იხ. `crates/iroha_cli/src/address.rs`), რომელიც იღებს I105/`0x…` ლიტერალებს და
  არასავალდებულო `<address>@<domain>` (rejected legacy form) ეტიკეტები, ნაგულისხმევი I105 გამომავალი Sora Nexus პრეფიქსის გამოყენებით (`753`),
  და ასხივებს მხოლოდ Sora-ზე შეკუმშულ ანბანს, როდესაც ოპერატორები პირდაპირ ითხოვენ მას
  `--format i105` ან JSON შემაჯამებელი რეჟიმი. ბრძანება ახორციელებს პრეფიქსის მოლოდინებს
  გაანალიზება, ჩაწერს მოწოდებულ დომენს (`input_domain` JSON-ში) და `legacy  suffix` დროშას
  იმეორებს კონვერტირებულ დაშიფვრას, როგორც `<address>@<domain>` (rejected legacy form), რათა მანიფესტაციური განსხვავებები დარჩეს ერგონომიული.
- **Wallet/explorer UX:** მიჰყევით [მისამართების ჩვენების მითითებებს] (source/sns/address_display_guidelines.md)
  გაიგზავნება ADDR-6-ით — გთავაზობთ ორმაგი ასლის ღილაკებს, შეინახეთ I105 QR დატვირთვად და გააფრთხილეთ
  მომხმარებლები, რომ შეკუმშული `i105` ფორმა არის მხოლოდ Sora და მგრძნობიარეა IME გადაწერებისთვის.
- **Torii ინტეგრაცია:** ქეში Nexus გამოხატავს TTL-ს, ემისიას
  `ForeignDomain`/`UnknownDomain`/`RegistryUnavailable` დეტერმინისტულად და
  keep strict account-literal parsing canonical-i105-only (reject non-canonical Katakana i105 literals and any `@domain` suffix) with canonical Katakana i105 output.

### Torii პასუხის ფორმატები

- `GET /v1/accounts` იღებს არასავალდებულო `canonical Katakana i105 rendering` მოთხოვნის პარამეტრს და
  `POST /v1/accounts/query` იღებს იმავე ველს JSON კონვერტში.
  მხარდაჭერილი მნიშვნელობებია:
  - `i105` (ნაგულისხმევი) — პასუხები ასხივებენ კანონიკურ I105 დატვირთვას (მაგ.
    `soraゴヂアニィルサフユイサヹピビレッデヹボテハキョメベチュヒャネィギチュヲベァヱェベモネェネツデトツオチハセ`).
  - `i105` — პასუხები გამოსცემს მხოლოდ Sora-ს `i105` შეკუმშულ ხედს, სანამ
    ფილტრების / ბილიკის პარამეტრების კანონიკური შენარჩუნება.
- არასწორი მნიშვნელობები ბრუნდება `400` (`QueryExecutionFail::Conversion`). ეს საშუალებას იძლევა
  საფულეები და მკვლევარები მოითხოვონ შეკუმშული სტრიქონები მხოლოდ Sora-ის UX-ისთვის, ხოლო
  I105-ის შენახვა თავსებადი ნაგულისხმევად.
- აქტივების მფლობელთა ჩამონათვალი (`GET /v1/assets/{definition_id}/holders`) და მათი JSON
  კონვერტის კოლეგა (`POST …/holders/query`) ასევე პატივს სცემს `canonical Katakana i105 rendering`.
  `items[*].account_id` ველი ასხივებს შეკუმშულ ლიტერალებს, როდესაც
  პარამეტრი/კონვერტის ველი დაყენებულია `i105`, რომელიც ასახავს ანგარიშებს
  ბოლო წერტილები, რათა მკვლევარებმა წარმოადგინონ თანმიმდევრული გამომავალი დირექტორიაში.
- ** ტესტირება: ** დაამატეთ ერთეული ტესტები შიფრატორის/დეკოდერის ორმხრივი მოგზაურობისთვის, არასწორი ჯაჭვისთვის
  წარუმატებლობები და მანიფესტი ძიება; დაამატეთ ინტეგრაციის გაშუქება Torii-სა და SDK-ებში
  რადგან I105 მიედინება ბოლომდე.

## შეცდომის კოდის რეესტრი

მისამართის შიფრატორები და დეკოდერები ავლენენ წარუმატებლობებს
`AccountAddressError::code_str()`. შემდეგ ცხრილებში მოცემულია სტაბილური კოდები
რომ SDK-ები, საფულეები და Torii ზედაპირები უნდა აღმოჩნდეს ადამიანის მიერ წაკითხვადი ზედაპირის გვერდით
შეტყობინებები, პლუს რეკომენდირებული ინსტრუქცია.

### კანონიკური კონსტრუქცია

| კოდი | წარუმატებლობა | რეკომენდირებული გამოსწორება |
|------|---------|------------------------|
| `ERR_UNSUPPORTED_ALGORITHM` | ენკოდერმა მიიღო ხელმოწერის ალგორითმი, რომელიც არ არის მხარდაჭერილი რეესტრის ან build ფუნქციების მიერ. | შეზღუდეთ ანგარიშის მშენებლობა რეესტრში და კონფიგურაციაში ჩართული მრუდებით. |
| `ERR_KEY_PAYLOAD_TOO_LONG` | ხელმოწერის გასაღების დატვირთვის სიგრძე აჭარბებს მხარდაჭერილ ლიმიტს. | ერთი გასაღებიანი კონტროლერები შემოიფარგლება `u8` სიგრძით; გამოიყენეთ multisig დიდი საჯარო გასაღებებისთვის (მაგ., ML-DSA). |
| `ERR_INVALID_HEADER_VERSION` | მისამართის სათაურის ვერსია მხარდაჭერილი დიაპაზონის მიღმაა. | გამოუშვით სათაური ვერსია `0` V1 მისამართებისთვის; განაახლეთ ენკოდერები ახალი ვერსიების მიღებამდე. |
| `ERR_INVALID_NORM_VERSION` | ნორმალიზების ვერსიის დროშა არ არის აღიარებული. | გამოიყენეთ ნორმალიზების ვერსია `1` და მოერიდეთ რეზერვირებული ბიტების გადართვას. |
| `ERR_INVALID_I105_PREFIX` | მოთხოვნილი I105 ქსელის პრეფიქსის კოდირება შეუძლებელია. | აირჩიეთ პრეფიქსი ჯაჭვის რეესტრში გამოქვეყნებულ `0..=16383` დიაპაზონში. |
| `ERR_CANONICAL_HASH_FAILURE` | ნორმალური დატვირთვის ჰეშირება ვერ მოხერხდა. | ხელახლა სცადეთ ოპერაცია; თუ შეცდომა შენარჩუნებულია, განიხილეთ იგი, როგორც შიდა ხარვეზი ჰეშირების სტეკში. |

### ფორმატის გაშიფვრა და ავტომატური აღმოჩენა

| კოდი | წარუმატებლობა | რეკომენდირებული გამოსწორება |
|------|---------|------------------------|
| `ERR_INVALID_I105_ENCODING` | I105 სტრიქონი შეიცავს სიმბოლოებს ანბანის მიღმა. | დარწმუნდით, რომ მისამართი იყენებს გამოქვეყნებულ I105 ანბანს და არ არის შეკვეცილი კოპირების/პასტის დროს. |
| `ERR_INVALID_LENGTH` | დატვირთვის სიგრძე არ ემთხვევა სელექტორის/კონტროლერის მოსალოდნელ კანონიკურ ზომას. | მიაწოდეთ სრული კანონიკური დატვირთვა არჩეული დომენის ამორჩევისა და კონტროლერის განლაგებისთვის. |
| `ERR_CHECKSUM_MISMATCH` | I105 (სასურველი) ან შეკუმშული (`sora`, მეორე საუკეთესო) საკონტროლო ჯამის ვალიდაცია ვერ მოხერხდა. | მისამართის რეგენერაცია სანდო წყაროდან; ეს ჩვეულებრივ მიუთითებს ასლის/პასტის შეცდომაზე. |
| `ERR_INVALID_I105_PREFIX_ENCODING` | I105 პრეფიქსი ბაიტები არასწორია. | მისამართის ხელახლა დაშიფვრა შესაბამისი ენკოდერით; არ შეცვალოთ წამყვანი I105 ბაიტი ხელით. |
| `ERR_INVALID_HEX_ADDRESS` | კანონიკური თექვსმეტობითი ფორმა ვერ გაშიფრა. | მიაწოდეთ `0x` პრეფიქსით, თანაბარი სიგრძის თექვსმეტობითი სტრიქონი, რომელიც დამზადებულია ოფიციალური კოდირებით. |
| `ERR_MISSING_COMPRESSED_SENTINEL` | შეკუმშული ფორმა არ იწყება `sora`-ით. | პრეფიქსი შეკუმშული Sora მისამართები საჭირო სენტინელით, სანამ ისინი დეკოდერებს გადასცემენ. |
| `ERR_COMPRESSED_TOO_SHORT` | შეკუმშულ სტრიქონს არ გააჩნია საკმარისი ციფრები დატვირთვისა და გამშვები ჯამისთვის. | გამოიყენეთ სრული შეკუმშული სტრიქონი, რომელიც გამოსხივებულია შიფრირებული სნიპეტების ნაცვლად. |
| `ERR_INVALID_COMPRESSED_CHAR` | შეკუმშული ანბანის გარეთ მყოფი სიმბოლო. | შეცვალეთ სიმბოლო მოქმედი Base-105 გლიფით გამოქვეყნებული ნახევარსიგანის/სრული სიგანის ცხრილებიდან. |
| `ERR_INVALID_COMPRESSED_BASE` | ენკოდერმა სცადა მხარდაჭერილი რადიქსის გამოყენება. | შეიტანეთ შეცდომა ენკოდერის წინააღმდეგ; შეკუმშული ანბანი ფიქსირდება 105-იან რადიქსზე V1-ში. |
| `ERR_INVALID_COMPRESSED_DIGIT` | ციფრული მნიშვნელობა აღემატება შეკუმშული ანბანის ზომას. | დარწმუნდით, რომ თითოეული ციფრი არის `0..105)` ფარგლებში, საჭიროების შემთხვევაში განაახლეთ მისამართი. |
| `ERR_UNSUPPORTED_ADDRESS_FORMAT` | ავტომატური ამოცნობა ვერ ცნობს შეყვანის ფორმატს. | პარსერების გამოძახებისას მიუთითეთ I105 (სასურველი), შეკუმშული (`sora`) ან კანონიკური `0x` თექვსმეტობითი სტრიქონები. |

### დომენის და ქსელის დადასტურება| კოდი | წარუმატებლობა | რეკომენდირებული გამოსწორება |
|------|---------|------------------------|
| `ERR_DOMAIN_MISMATCH` | დომენის სელექტორი არ ემთხვევა მოსალოდნელ დომენს. | გამოიყენეთ დანიშნულ დომენზე გაცემული მისამართი ან განაახლეთ მოლოდინი. |
| `ERR_INVALID_DOMAIN_LABEL` | დომენის ლეიბლის ნორმალიზაციის შემოწმება ვერ მოხერხდა. | დაშიფვრამდე დომენის კანონიკირება UTS-46 არატრანზიციული დამუშავების გამოყენებით. |
| `ERR_UNEXPECTED_NETWORK_PREFIX` | დეკოდირებული I105 ქსელის პრეფიქსი განსხვავდება კონფიგურირებული მნიშვნელობისაგან. | გადაერთეთ მისამართზე სამიზნე ჯაჭვიდან ან შეცვალეთ მოსალოდნელი დისკრიმინანტი/პრეფიქსი. |
| `ERR_UNKNOWN_ADDRESS_CLASS` | მისამართების კლასის ბიტები არ არის აღიარებული. | განაახლეთ დეკოდერი გამოშვებამდე, რომელიც გაიგებს ახალ კლასს, ან მოერიდეთ სათაურის ბიტებში ჩარევას. |
| `ERR_UNKNOWN_DOMAIN_TAG` | დომენის ამომრჩეველი ტეგი უცნობია. | განაახლეთ ვერსიაზე, რომელიც მხარს უჭერს ამომრჩევის ახალ ტიპს, ან მოერიდეთ ექსპერიმენტული დატვირთვების გამოყენებას V1 კვანძებზე. |
| `ERR_UNEXPECTED_EXTENSION_FLAG` | დაჯავშნილი გაფართოების ბიტი დაყენდა. | რეზერვირებული ბიტების გასუფთავება; ისინი დარჩებიან კარიბჭეში, სანამ მომავალი ABI არ წარადგენს მათ. |
| `ERR_UNKNOWN_CONTROLLER_TAG` | კონტროლერის დატვირთვის ტეგი არ არის აღიარებული. | განაახლეთ დეკოდერი, რათა ამოიცნოთ ახალი კონტროლერის ტიპები მათ გარჩევამდე. |
| `ERR_UNEXPECTED_TRAILING_BYTES` | კანონიკური დატვირთვა შეიცავდა უკანა ბაიტებს დეკოდირების შემდეგ. | აღადგინეთ კანონიკური დატვირთვა; მხოლოდ დოკუმენტირებული სიგრძე უნდა იყოს წარმოდგენილი. |

### კონტროლერის Payload Validation

| კოდი | წარუმატებლობა | რეკომენდირებული გამოსწორება |
|------|---------|------------------------|
| `ERR_INVALID_PUBLIC_KEY` | საკვანძო ბაიტები არ ემთხვევა დეკლარირებულ მრუდს. | დარწმუნდით, რომ გასაღები ბაიტები დაშიფრულია ზუსტად ისე, როგორც საჭიროა არჩეული მრუდისთვის (მაგ., 32-ბაიტი Ed25519). |
| `ERR_UNKNOWN_CURVE` | მრუდის იდენტიფიკატორი არ არის რეგისტრირებული. | გამოიყენეთ მრუდის ID `1` (Ed25519), სანამ დამატებითი მრუდები არ დამტკიცდება და გამოქვეყნდება რეესტრში. |
| `ERR_MULTISIG_MEMBER_OVERFLOW` | Multisig კონტროლერი აცხადებს უფრო მეტ წევრს, ვიდრე მხარდაჭერილია. | დაშიფვრამდე შეამცირეთ მრავალრიცხოვანი წევრობა დოკუმენტურ ლიმიტამდე. |
| `ERR_INVALID_MULTISIG_POLICY` | Multisig პოლიტიკის მომგებიანი ვალიდაცია ვერ მოხერხდა (ბარიერი/წონა/სქემა). | ხელახლა შექმენით პოლიტიკა ისე, რომ დააკმაყოფილოს CTAP2 სქემა, წონის საზღვრები და ზღვრული შეზღუდვები. |

განიხილება ## ალტერნატივები

- **Pure checksum envelope (ბიტკოინის სტილი).** უფრო მარტივი საკონტროლო ჯამი, მაგრამ უფრო სუსტი შეცდომის გამოვლენა
  ვიდრე Blake2b-დან მიღებული I105 საკონტროლო ჯამი (`encode_i105` წყვეტს 512-ბიტიან ჰეშს)
  და მოკლებულია აშკარა პრეფიქსის სემანტიკას 16-ბიტიანი დისკრიმინანტებისთვის.
- **ჯაჭვის სახელის ჩაშენება დომენის სტრიქონში (მაგ., `finance@chain`).** იშლება
- ** დაეყრდნოთ მხოლოდ Nexus მარშრუტიზაციას მისამართების შეცვლის გარეშე.** მომხმარებლები მაინც იქნებიან
  ორაზროვანი სტრიქონების კოპირება/ჩასმა; ჩვენ გვსურს, რომ მისამართი თავად ატარებდეს კონტექსტს.
- **Bech32m კონვერტი.** QR-მეგობრული და გთავაზობთ ადამიანისათვის წასაკითხ პრეფიქსს, მაგრამ
  განსხვავდებოდა გადაზიდვის I105 განხორციელებისგან (`AccountAddress::to_i105`)
  და მოითხოვს ყველა მოწყობილობის/SDK-ის ხელახლა შექმნას. მიმდინარე საგზაო რუკა ინახავს I105 +
  შეკუმშული (`sora`) მხარდაჭერა მომავალში კვლევის გაგრძელებისას
  Bech32m/QR ფენები (CAIP-10 რუკების გადადება).

## ღია კითხვები

- დაადასტურეთ, რომ `u16` დისკრიმინანტები პლუს რეზერვირებული დიაპაზონები ფარავს გრძელვადიან მოთხოვნას;
  წინააღმდეგ შემთხვევაში შეაფასეთ `u32` varint კოდირებით.
- დაასრულეთ მრავალხელმოწერის მართვის პროცესი რეესტრის განახლებისთვის და როგორ
  განიხილება გაუქმება/ვადაგასული ასიგნებები.
- განსაზღვრეთ მანიფესტის ხელმოწერის ზუსტი სქემა (მაგ., Ed25519 multi-sig) და
  სატრანსპორტო უსაფრთხოება (HTTPS დამაგრება, IPFS ჰეშის ფორმატი) Nexus განაწილებისთვის.
- დაადგინეთ, მხარდაჭერილი იქნება თუ არა დომენის ფსევდონიმები/გადამისამართებები მიგრაციებისთვის და როგორ
  დეტერმინიზმის დარღვევის გარეშე გამოაქვეყნოს ისინი.
- მიუთითეთ, როგორ აფორმებს Kotodama/IVM კონტრაქტებს წვდომა I105 დამხმარეებზე (`to_address()`,
  `parse_address()`) და უნდა გამოამჟღავნოს თუ არა CAIP-10-ის ჯაჭვზე შენახვა
  რუკები (დღეს I105 კანონიკურია).
- გამოიკვლიეთ Iroha ჯაჭვების რეგისტრაცია გარე რეესტრებში (მაგ., I105 რეესტრი,
  CAIP სახელთა სივრცის დირექტორია) ეკოსისტემის უფრო ფართო გასწორებისთვის.

## შემდეგი ნაბიჯები

1. I105 კოდირება დაეშვა `iroha_data_model`-ში (`AccountAddress::to_i105`,
   `parse_encoded`); განაგრძეთ მოწყობილობების/ტესტების პორტირება ყველა SDK-ზე და გაასუფთავეთ ნებისმიერი
   Bech32m placeholders.
2. გააფართოვეთ კონფიგურაციის სქემა `chain_discriminant`-ით და მიიღეთ გონივრული
  ნაგულისხმევი სატესტო/დეველოპერული კონფიგურაციებისთვის. **(შესრულებულია: `common.chain_discriminant`
  ახლა იგზავნება `iroha_config`-ში, ნაგულისხმევად არის `0x02F1` თითო ქსელში
  აჭარბებს.)**
3. შეადგინეთ Nexus რეესტრის სქემა და კონცეფციის დამადასტურებელი მანიფესტის გამომცემელი.
4. შეაგროვეთ გამოხმაურება საფულის პროვაიდერებისა და მეურვეებისგან ადამიანური ფაქტორის ასპექტებზე
   (HRP დასახელება, ჩვენების ფორმატირება).
5. განაახლეთ დოკუმენტაცია (`docs/source/data_model.md`, Torii API დოკუმენტები) ერთხელ
   განხორციელების გზა ჩადებულია.
6. გაგზავნეთ ოფიციალური კოდეკების ბიბლიოთეკები (Rust/TS/Python/Kotlin) ნორმატიული ტესტით
   ვექტორები, რომლებიც მოიცავს წარმატებისა და წარუმატებლობის შემთხვევებს.
