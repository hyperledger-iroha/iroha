---
lang: my
direction: ltr
source: docs/account_structure.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 01561366c9698c10d29ff3f49ad4c14b22b1796b5c7701cf98d200a140af1caf
source_last_modified: "2026-01-28T17:11:30.635172+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# အကောင့်ဖွဲ့စည်းပုံ RFC

**အခြေအနေ-** လက်ခံထားသည် (ADDR-1)  
**ပရိသတ်-** ဒေတာမော်ဒယ်၊ Torii၊ Nexus၊ ပိုက်ဆံအိတ်၊ အုပ်ချုပ်မှုအဖွဲ့များ  
**ဆက်စပ်ပြဿနာများ-** TBD

## အကျဉ်းချုပ်

ဤစာတမ်းတွင် အကောင်အထည်ဖော်ခဲ့သည့် ပို့ဆောင်ရေးအကောင့်လိပ်စာအတွဲကို ဖော်ပြသည်။
`AccountAddress` (`crates/iroha_data_model/src/account/address.rs`) ကိုလည်းကောင်း၊
အဖော်ကိရိယာ။ ၎င်းသည်:

- လူမျက်နှာဖြင့် စစ်ဆေးထားသော **Iroha Base58 လိပ်စာ (IH58)** မှ ထုတ်လုပ်သည်
  `AccountAddress::to_ih58` သည် ကွင်းဆက်ခွဲခြားမှုကို အကောင့်နှင့် ချိတ်ထားသည်။
  ထိန်းချုပ်ကိရိယာနှင့် အဆုံးအဖြတ်ပေးသော အပြန်အလှန်-ဖော်ရွေသော စာသားပုံစံများကို ပေးဆောင်သည်။
- သွယ်ဝိုက်သောမူလဒိုမိန်းများနှင့် ဒေသဆိုင်ရာအချေအတင်များအတွက် ဒိုမိန်းရွေးချယ်မှုများ၊
  အနာဂတ် Nexus ကျောထောက်နောက်ခံပြုထားသော လမ်းကြောင်းအတွက် သီးသန့်ကမ္ဘာလုံးဆိုင်ရာ-မှတ်ပုံတင်ရွေးချယ်မှုတဂ် (
  registry lookup ကို **မပို့ရသေးပါ**)။

## စေ့ဆော်မှု

Wallets နှင့် off-chain tooling သည် ယနေ့ခေတ် `alias@domain` (rejected legacy form) အကြမ်းမျဉ်းအမည်များပေါ်တွင် အားကိုးပါသည်။ ဒီ
အဓိက အားနည်းချက် နှစ်ခုရှိသည်။

1. **ကွန်ရက်ချိတ်ဆက်မှု မရှိပါ။** စာကြောင်းတွင် checksum သို့မဟုတ် ကွင်းဆက်ရှေ့ဆက်မပါသောကြောင့် အသုံးပြုသူများ
   ချက်ချင်းတုံ့ပြန်မှုမရှိဘဲ မှားယွင်းသောကွန်ရက်မှ လိပ်စာတစ်ခုကို ကူးထည့်နိုင်သည်။ ဟိ
   အရောင်းအဝယ်သည် နောက်ဆုံးတွင် ပယ်ချခံရမည် (ကွင်းဆက်မညီ) သို့မဟုတ် ပိုဆိုးသည်မှာ အောင်မြင်သည်။
   ဦးတည်ရာသည် စက်တွင်း၌ ရှိနေပါက မရည်ရွယ်သော အကောင့်ကို ဆန့်ကျင်ပါ။
2. **Domain collision.** Domains များသည် namespace သီးသန့်ဖြစ်ပြီး တစ်ခုချင်းစီတွင် ပြန်လည်အသုံးပြုနိုင်သည်
   ကွင်းဆက်။ ဝန်ဆောင်မှုများအသင်းချုပ် (စောင့်ထိန်းသူများ၊ တံတားများ၊ ကွင်းဆက်လုပ်ငန်းအသွားအလာများ)
   ကွင်းဆက် A ရှိ `finance` သည် `finance` နှင့် မသက်ဆိုင်သောကြောင့် ဆတ်ဆတ်ထိမခံဖြစ်လာသည်
   ကွင်းဆက် B

မိတ္တူ/ကူးထည့်ခြင်း အမှားအယွင်းများကို ကာကွယ်သည့် လူသားဆန်ဆန် လိပ်စာဖော်မတ်တစ်ခု လိုအပ်ပါသည်။
နှင့် domain name မှ authoritative chain သို့ အဆုံးအဖြတ်ပေးသော မြေပုံဆွဲခြင်း။

## ပန်းတိုင်

- ဒေတာမော်ဒယ်တွင် အကောင်အထည်ဖော်ခဲ့သော IH58 Base58 စာအိတ်ကို ဖော်ပြပါ။
  `AccountId` နှင့် `AccountAddress` လိုက်နာသော canonical parsing/alias စည်းမျဉ်းများ။
- စီစဉ်သတ်မှတ်ထားသော ကွင်းဆက်ခွဲခြားမှုကို လိပ်စာတစ်ခုစီသို့ တိုက်ရိုက် ကုဒ်လုပ်ပါ။
  ၎င်း၏ အုပ်ချုပ်မှု/မှတ်ပုံတင်ခြင်း လုပ်ငန်းစဉ်ကို သတ်မှတ်ပါ။
- လက်ရှိမဖောက်မပြန်ဘဲ ကမ္ဘာလုံးဆိုင်ရာ ဒိုမိန်းမှတ်ပုံတင်ခြင်းအား မိတ်ဆက်နည်းကို ဖော်ပြပါ။
  ဖြန့်ကျက်ပြီး ပုံမှန်ဖြစ်အောင်ပြုလုပ်ခြင်း/ အတုအယောင်ဆန့်ကျင်ခြင်း စည်းမျဉ်းများကို သတ်မှတ်ပါ။

## ပန်းတိုင်မဟုတ်သော

- ကွင်းဆက်ပိုင်ဆိုင်မှုလွှဲပြောင်းမှုများကို အကောင်အထည်ဖော်ခြင်း။ Routing Layer သည် ၎င်းကိုသာလျှင် ပြန်ပေးသည်။
  ပစ်မှတ်ကွင်းဆက်။
- ကမ္ဘာလုံးဆိုင်ရာ ဒိုမိန်းထုတ်ပေးခြင်းအတွက် အုပ်ချုပ်ရေးကို အပြီးသတ်ခြင်း။ ဤ RFC သည် data ကိုအာရုံစိုက်သည်။
  မော်ဒယ်နှင့် သယ်ယူပို့ဆောင်ရေးဆိုင်ရာ အခြေခံများ။

## နောက်ခံ

### လက်ရှိလမ်းကြောင်းအမည်များ

```
AccountId {
    domain: DomainId,   // wrapper over Name (ASCII-ish string)
    controller: AccountController // single PublicKey or multisig policy
}

Display: canonical IH58 literal (no `@domain` suffix)
Parse accepts:
- Encoded account identifiers only: IH58 (preferred) and `sora` compressed.
- Runtime parsers reject canonical hex (`0x...`), any `@<domain>` suffix, and alias literals such as `label@domain`.

Multihash hex is canonical: varint bytes are lowercase hex, payload bytes are uppercase hex,
and `0x` prefixes are not accepted.

This text form is now treated as an **account alias**: a routing convenience
that points to the canonical [`AccountAddress`](#2-canonical-address-codecs).
It remains useful for human readability and domain-scoped governance, but it is
no longer considered the authoritative account identifier on-chain.
```

`ChainId` သည် `AccountId` ၏ အပြင်ဘက်တွင် နေထိုင်သည်။ Nodes သည် ငွေပေးငွေယူ၏ `ChainId` ကို စစ်ဆေးပါ။
ဝင်ခွင့်ကာလအတွင်း ဖွဲ့စည်းမှုပုံစံကို ဆန့်ကျင်ခြင်း (`AcceptTransactionFail::ChainIdMismatch`)
နိုင်ငံခြားငွေပေးငွေယူများကို ငြင်းပယ်သော်လည်း အကောင့်စာတန်းကိုယ်တိုင်က မပါ၀င်ပါ။
ကွန်ရက် အရိပ်အမြွက်။

### Domain ခွဲခြားသတ်မှတ်မှုများ

`DomainId` သည် `Name` (သာမန်စာကြောင်း) ကို ခြုံပြီး ဒေသတွင်း ကွင်းဆက်သို့ အတိုင်းအတာအထိ ကန့်သတ်ထားသည်။
ဆိုင်ခွဲတိုင်းတွင် `wonderland`၊ `finance` စသည်တို့ကို လွတ်လပ်စွာ စာရင်းပေးသွင်းနိုင်ပါသည်။

### Nexus စကားစပ်

Nexus သည် အစိတ်အပိုင်းများ ပေါင်းစပ်ညှိနှိုင်းမှု (လမ်းကြောများ/ဒေတာ-နေရာများ) အတွက် တာဝန်ရှိသည်။ အဲဒါ
လောလောဆယ်တွင် cross-chain domain routing သဘောတရားမရှိပါ။

## အဆိုပြုဒီဇိုင်း

### 1. Deterministic chain discriminant

`iroha_config::parameters::actual::Common` ယခု ဖော်ထုတ်ထားသည်-

```rust
pub struct Common {
    pub chain: ChainId,
    pub chain_discriminant: u16, // globally coordinated
    // ... existing fields
}
```

**ကန့်သတ်ချက်များ-**
  - တက်ကြွသောကွန်ရက်တစ်ခုချင်းစီအတွက် ထူးခြားသော၊ လက်မှတ်ရေးထိုးထားသော အများသူငှာ မှတ်ပုံတင်ခြင်းမှတဆင့် စီမံခန့်ခွဲပါသည်။
    ပြတ်သားစွာ သီးသန့်ထားသော အပိုင်းအခြားများ (ဥပမာ၊ `0x0000–0x0FFF` စမ်းသပ်/ဆော့ဖ်ဝဲ၊ `0x1000–0x7FFF`
    ရပ်ရွာခွဲဝေမှုများ၊ `0x8000–0xFFEF` အုပ်ချုပ်မှု-အတည်ပြုထားသော၊ `0xFFF0–0xFFFF`
    သီးသန့်)။
  - လည်ပတ်နေသည့်ကွင်းဆက်အတွက် မပြောင်းလဲနိုင်ပါ။ ၎င်းကိုပြောင်းလဲရန်ခက်ခဲသောခက်ရင်းတစ်ခုလိုအပ်သည်။
    မှတ်ပုံတင်ခြင်းအပ်ဒိတ်။
- ** အုပ်ချုပ်မှု နှင့် မှတ်ပုံတင်ခြင်း ( စီစဉ်ထားသည် ) : ** လက်မှတ်ပေါင်းများစွာ အုပ်ချုပ်မှုစနစ် သတ်မှတ်ပေးမည် ။
  JSON မှတ်ပုံတင်ရေးတွင် လက်မှတ်ရေးထိုးထားသော ခွဲခြားဆက်ဆံသူများကို လူသားအမည်တူများနှင့် ပုံဖော်ထားသည်။
  CAIP-2 သတ်မှတ်ချက်များ။ ဤစာရင်းသွင်းမှုသည် တင်ပို့သည့်အချိန်၏ အစိတ်အပိုင်းမဟုတ်သေးပါ။
- **အသုံးပြုမှု-** ပြည်နယ်ဝင်ခွင့်၊ Torii၊ SDK နှင့် wallet APIs များမှတဆင့် ချည်နှောင်ထားပါသည်။
  အစိတ်အပိုင်းတိုင်းသည် ၎င်းကို embed သို့မဟုတ် validate လုပ်နိုင်သည်။ CAIP-2 ထိတွေ့မှုသည် အနာဂတ်တွင် ရှိနေသေးသည်။
  interop အလုပ်။

### 2. Canonical address codecs

Rust ဒေတာမော်ဒယ်သည် တစ်ခုတည်းသော canonical payload ကိုယ်စားပြုမှုကို ဖော်ပြသည်။
(`AccountAddress`) သည် လူသားပုံစံများစွာဖြင့် ထုတ်လွှတ်နိုင်သည်။ IH58 ဖြစ်ပါတယ်။
မျှဝေခြင်းနှင့် canonical output အတွက် နှစ်သက်သော အကောင့်ဖော်မတ်၊ compressed
`sora` ဖောင်သည် kana အက္ခရာရှိသည့် UX အတွက် ဒုတိယအကောင်းဆုံး၊ Sora-သီးသန့် ရွေးချယ်မှုတစ်ခုဖြစ်သည်။
တန်ဖိုးထပ်ပေးသည်။ Canonical hex သည် အမှားရှာပြင်ခြင်းဆိုင်ရာ အကူအညီတစ်ခုအဖြစ် ကျန်ရှိနေပါသည်။

- **IH58 (Iroha Base58)** - ကွင်းဆက်ကို မြှုပ်ထားသည့် Base58 စာအိတ်
  ခွဲခြားဆက်ဆံသည်။ ကုဒ်ဒရိုများသည် ပေးဆောင်မှုအား မြှင့်တင်ခြင်းမပြုမီ ရှေ့ဆက်ကို အတည်ပြုသည်။
  canonical ပုံစံ။
- **Sora-ချုံ့ထားသောမြင်ကွင်း** – တည်ဆောက်ထားသော **105 သင်္ကေတ** ၏ Sora တစ်ခုတည်းသော အက္ခရာ၊
  အနံတစ်ဝက် イロハ (ヰ နှင့် ヱ အပါအဝင်) ကို အက္ခရာ ၅၈ လုံးဖြင့် ပေါင်းထည့်ခြင်း၊
  IH58 အစုံ။ စာကြောင်းများသည် Sentinel `sora` ဖြင့်စတင်ပြီး၊ Bech32m မှရရှိသော မြှုပ်နှံထားသည်
  checksum ၊ ကွန်ရက်ရှေ့ဆက်ကို ချန်လှပ်ထားပါ (Sora Nexus သည် Sentinel မှ အဓိပ္ပာယ်သက်ရောက်သည်)။

  ```
  IH58  : 123456789ABCDEFGHJKMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz
  Iroha : ｲﾛﾊﾆﾎﾍﾄﾁﾘﾇﾙｦﾜｶﾖﾀﾚｿﾂﾈﾅﾗﾑｳヰﾉｵｸﾔﾏｹﾌｺｴﾃｱｻｷﾕﾒﾐｼヱﾋﾓｾｽ
  ```
- **Canonical hex** - canonical byte ၏ အမှားရှာပြင်ဆင်ခြင်း-အဆင်ပြေသော `0x…` ကုဒ်နံပါတ်
  စာအိတ်။

`AccountAddress::parse_encoded` သည် IH58 (ပိုနှစ်သက်သည်)၊ ဖိသိပ်ထားသည် (`sora`၊ ဒုတိယအကောင်းဆုံး) သို့မဟုတ် canonical hex
(`0x...` သာ၊ ဗလာ hex ကို ငြင်းပယ်ထားသည်) ထည့်သွင်းပြီး ကုဒ်လုပ်ထားသော payload နှင့် တွေ့ရှိထားသည့် နှစ်ခုလုံးကို ပြန်ပေးသည်
`AccountAddressFormat`။ ယခု Torii သည် ISO 20022 ဖြည့်စွက်မှုအတွက် `parse_encoded` ကို ခေါ်ဆိုနေသည်
လိပ်စာများနှင့် canonical hex ပုံစံကို သိမ်းဆည်းထားသောကြောင့် မက်တာဒေတာသည် အဆုံးအဖြတ်အတိုင်း ဆက်လက်တည်ရှိနေပါသည်။
မူရင်းကိုယ်စားပြုမှုမခွဲခြားဘဲ။

#### 2.1 ခေါင်းစီးဘိုက်အပြင်အဆင် (ADDR-1a)

Canonical payload တိုင်းကို `header · controller` အဖြစ် သတ်မှတ်ထားသည်။ ဟိ
`header` သည် မည်သည့် parser စည်းမျဉ်းများ သက်ရောက်သည့် bytes များကို ဆက်သွယ်ပေးသည့် single byte တစ်ခုဖြစ်သည်။
လိုက်ရန်-

```
bit index:   7        5 4      3 2      1 0
             ┌─────────┬────────┬────────┬────┐
payload bit: │version  │ class  │  norm  │ext │
             └─────────┴────────┴────────┴────┘
```

ထို့ကြောင့် ပထမ byte သည် downstream decoders အတွက် schema metadata ကိုထုပ်ပိုးသည်-

| ဘစ် | လယ် | ခွင့်ပြုတန်ဖိုးများ | ချိုးဖောက်မှု | အမှား
|--------|-------|----------------|--------------------|
| ၇-၅ | `addr_version` | `0` (v1)။ တန်ဖိုးများ `1-7` အား အနာဂတ်ပြန်လည်ပြင်ဆင်မှုများအတွက် သီးသန့်ထားသည်။ | `0-7` ပြင်ပတန်ဖိုးများ `AccountAddressError::InvalidHeaderVersion`; အကောင်အထည်ဖော်မှုများသည် သုညမဟုတ်သောဗားရှင်းများကို ယနေ့ ပံ့ပိုးမထားသည့်အဖြစ် သတ်မှတ်ရပါမည်။ |
| ၄-၃ | `addr_class` | `0` = တစ်ခုတည်းသောသော့၊ `1` = multisig ။ | အခြားတန်ဖိုးများသည် `AccountAddressError::UnknownAddressClass` တိုးသည်။ |
| 2-1 | `norm_version` | `1` (Norm v1)။ တန်ဖိုးများကို `0`၊ `2`၊ `3` တို့ကို သီးသန့်ထားသည်။ | `0-3` ပြင်ပတန်ဖိုးများသည် `AccountAddressError::InvalidNormVersion` တိုးသည်။ |
| 0 | `ext_flag` | `0` ဖြစ်ရမည်။ | Set bit သည် `AccountAddressError::UnexpectedExtensionFlag` ဖြစ်သည်။ |

Rust ကုဒ်ပြောင်းကိရိယာသည် single-key controllers အတွက် `0x02` ကိုရေးသားသည် (ဗားရှင်း 0၊ အတန်း 0၊
norm v1၊ တိုးချဲ့မှုအလံကို ရှင်းလင်းထားသည်) နှင့် multisig ထိန်းချုပ်ကိရိယာများအတွက် `0x0A` (ဗားရှင်း 0၊
အတန်းအစား 1၊ norm v1၊ တိုးချဲ့မှုအလံကို ရှင်းလင်းထားသည်။)

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

#### 2.3 ထိန်းချုပ်ကိရိယာ ပေးချေမှုကုဒ်များ (ADDR-1a)

controller payload သည် canonical payload တွင် header နောက်တွင် (legacy selector segment ကို decode လုပ်သောအခါ အဲဒီအစိတ်အပိုင်းနောက်တွင်) ဆက်လက်လာသော tagged union ဖြစ်သည်။

| Tag | ထိန်းချုပ်သူ | အပြင်အဆင် | မှတ်စုများ |
|-----|------------|--------|-------|
| `0x00` | တစ်ခုတည်းသောသော့ | `curve_id:u8` · `key_len:u8` · `key_bytes` | `curve_id=0x01` ယနေ့ Ed25519 သို့မြေပုံများ။ `key_len` သည် `u8` သို့ ကန့်သတ်ထားသည်။ ပိုကြီးသောတန်ဖိုးများသည် `AccountAddressError::KeyPayloadTooLong` ကိုတိုးစေသည် (ထို့ကြောင့် single-key ML‑DSA အများသူငှာသော့များဖြစ်သော 255 bytes ရှိသော၊ သည် ကုဒ်လုပ်၍မရသည့်အပြင် multisig ကိုအသုံးပြုရပါမည်)။ |
| `0x01` | Multisig | `version:u8` · `threshold:u16` · `member_count:u8` · (`curve_id:u8` · `weight:u16` · `key_len:u16` · I18NI0000016X)\ | အဖွဲ့ဝင် 255 ဦးအထိ (`CONTROLLER_MULTISIG_MEMBER_MAX`) အထိ ပံ့ပိုးပေးသည်။ အမည်မသိ မျဉ်းကွေးများ `AccountAddressError::UnknownCurve` ကိုမြှင့်ပါ။ ပုံမမှန်သောမူဝါဒများသည် `AccountAddressError::InvalidMultisigPolicy` အဖြစ် ပူဖောင်းပေါက်နေသည်။ |

Multisig မူဝါဒများသည် CTAP2 စတိုင် CBOR မြေပုံနှင့် canonical digest တို့ကိုလည်း ဖော်ထုတ်ပါသည်။
host နှင့် SDKs များသည် controller ကို အဆုံးအဖြတ်ပေးနိုင်သည် ။ ကြည့်ပါ။
schema အတွက် `docs/source/references/multisig_policy_schema.md` (ADDR-1c)၊
တရားဝင်စည်းမျဉ်းများ၊ တားဆီးခြင်းလုပ်ငန်းစဉ်များနှင့် ရွှေရောင်စုံများ။

သော့ဘိုက်များအားလုံးကို `PublicKey::to_bytes` မှ ပြန်ပေးထားသည့်အတိုင်း အတိအကျ ကုဒ်ဝှက်ထားသည်။ ကုဒ်ဒုဒ်များသည် `PublicKey` ဖြစ်ရပ်များကို ပြန်လည်တည်ဆောက်ပြီး ဘိုက်များသည် ကြေညာထားသောမျဉ်းကွေးနှင့် မကိုက်ညီပါက `AccountAddressError::InvalidPublicKey` ကို မြှင့်တင်ပါ။

> **Ed25519 canonical enforcement (ADDR-3a):** မျဉ်းကွေး `0x01` သော့များသည် လက်မှတ်ထိုးသူမှ ထုတ်လွှတ်သော byte စာကြောင်းအတိအကျကို decode လုပ်ရမည် ဖြစ်ပြီး အသေးစားမှာယူမှုအုပ်စုခွဲတွင် မလိမ်ရပါ။ ယခုအခါ Node များသည် Canonical မဟုတ်သော ကုဒ်နံပါတ်များ (ဥပမာ၊ တန်ဖိုးများ လျှော့ချထားသော modulo `2^255-19`) နှင့် Identity ဒြပ်စင်ကဲ့သို့ အားနည်းချက်များကို ငြင်းဆိုထားသောကြောင့် SDK များသည် လိပ်စာများမတင်ပြမီ ကိုက်ညီသည့် တရားဝင်မှု အမှားများကို ဖော်ပြသင့်ပါသည်။

##### 2.3.1 Curve identifier registry (ADDR-1d)

| ID (`curve_id`) | Algorithm | ထူးခြားချက် ဂိတ် | မှတ်စုများ |
|-----------------|-----------|-----------------|------|
| `0x00` | လက်ဝယ် | — | ထုတ်လွှတ်ခြင်းမပြုရပါ။ ကုဒ်ဒုဒ်ကိရိယာများ မျက်နှာပြင် `ERR_UNKNOWN_CURVE`။ |
| `0x01` | Ed25519 | — | Canonical v1 algorithm (`Algorithm::Ed25519`); မူရင်း config တွင်ဖွင့်ထားသည်။ |
| `0x02` | ML-DSA (Dilithium3) | — | Dilithium3 အများသူငှာသော့ဘိုက်များ (1952 bytes) ကို အသုံးပြုသည်။ `key_len` သည် `u8` ဖြစ်သောကြောင့် တစ်ခုတည်းသောသော့လိပ်စာများသည် ML-DSA ကိုကုဒ်လုပ်၍မရပါ။ Multisig သည် `u16` အရှည်များကို အသုံးပြုသည်။ |
| `0x03` | BLS12-381 (ပုံမှန်) | `bls` | G1 (48 bytes) ရှိ အများသူငှာသော့များ၊ G2 (96 bytes) ရှိ လက်မှတ်များ။ |
| `0x04` | secp256k1 | — | SHA-256 ထက် ECDSA ကို သတ်မှတ်ခြင်း၊ အများသူငှာသော့များသည် 33-byte SEC1 ကိုချုံ့ထားသောပုံစံကိုအသုံးပြုပြီး လက်မှတ်များသည် canonical 64-byte `r∥s` အပြင်အဆင်ကိုအသုံးပြုသည်။ |
| `0x05` | BLS12-381 (အသေး) | `bls` | G2 (96 bytes) ရှိ အများသူငှာသော့များ၊ G1 (48 bytes) တွင် လက်မှတ်များ။ |
| `0x0A` | GOST R 34.10-2012 (256၊ သတ်မှတ် A) | `gost` | `gost` လုပ်ဆောင်ချက်ကို ဖွင့်ထားမှသာ ရနိုင်သည်။ |
| `0x0B` | GOST R 34.10-2012 (256၊ set B) | `gost` | `gost` လုပ်ဆောင်ချက်ကို ဖွင့်ထားမှသာ ရနိုင်သည်။ |
| `0x0C` | GOST R 34.10-2012 (256၊ set C) | `gost` | `gost` လုပ်ဆောင်ချက်ကို ဖွင့်ထားမှသာ ရနိုင်သည်။ |
| `0x0D` | GOST R 34.10-2012 (512၊ သတ်မှတ် A) | `gost` | `gost` လုပ်ဆောင်ချက်ကို ဖွင့်ထားမှသာ ရနိုင်သည်။ |
| `0x0E` | GOST R 34.10-2012 (512၊ set B) | `gost` | `gost` လုပ်ဆောင်ချက်ကို ဖွင့်ထားမှသာ ရနိုင်သည်။ |
| `0x0F` | SM2 | `sm` | DistID အရှည် (u16 BE) + DistID ဘိုက်များ + 65-byte SEC1 ကို ချုံ့မထားသော SM2 သော့၊ `sm` ကို ဖွင့်ထားမှသာ ရနိုင်သည်။ |

ထပ်ဆောင်းမျဉ်းကွေးများအတွက် အပေါက်များ `0x06–0x09` ကို တာဝန်မပေးသေးပါ။ အသစ်တစ်ခုကို မိတ်ဆက်ပေးခြင်း။
algorithm သည် လမ်းပြမြေပုံအပ်ဒိတ်တစ်ခု လိုအပ်ပြီး SDK/host coverage နှင့် ကိုက်ညီပါသည်။ ကုဒ်နံပါတ်များ
`ERR_UNSUPPORTED_ALGORITHM` ဖြင့် ပံ့ပိုးမထားသော မည်သည့် အယ်လဂိုရီသမ်ကိုမဆို ငြင်းပယ်ရမည်၊
ထိန်းသိမ်းထားရန် `ERR_UNKNOWN_CURVE` ဖြင့် အမည်မသိ id များပေါ်တွင် ကုဒ်ကုဒ်ကိရိယာများသည် လျင်မြန်စွာ ပျက်ကွက်ရမည်
ပျက်ကွက်သော အပြုအမူ။

Canonical registry (စက်-ဖတ်နိုင်သော JSON ထုတ်ယူမှုအပါအဝင်) အောက်တွင်တည်ရှိသည်။
[`docs/source/references/address_curve_registry.md`](source/references/address_curve_registry.md)။
ကိရိယာတန်ဆာပလာသည် ထိုဒေတာအတွဲကို တိုက်ရိုက်စားသုံးသင့်ပြီး မျဉ်းကွေး ခွဲခြားသတ်မှတ်မှုများ ရှိနေပါသည်။
SDKs နှင့် အော်ပရေတာ အလုပ်အသွားအလာများတစ်လျှောက် တသမတ်တည်း။

- **SDK ဂိတ်ပေါက်-** SDK များသည် မူရင်း Ed25519-only validation/encoding သို့ဖြစ်သည်။ လျင်မြန်စွာ ဖော်ထုတ်သည်။
  compile-time အလံများ (`IROHASWIFT_ENABLE_MLDSA`၊ `IROHASWIFT_ENABLE_GOST`၊
  `IROHASWIFT_ENABLE_SM`); Java/Android SDK လိုအပ်သည်။
  `AccountAddress.configureCurveSupport(...)`; JavaScript SDK ကိုအသုံးပြုသည်။
  `configureCurveSupport({ allowMlDsa: true, allowGost: true, allowSm2: true })`။
  secp256k1 ပံ့ပိုးမှုကို ရနိုင်သော်လည်း JS/Android တွင် မူရင်းအတိုင်း ဖွင့်မထားပါ။
  SDKs; ခေါ်ဆိုသူများသည် Ed25519 မဟုတ်သော ထိန်းချုပ်ကိရိယာများကို ထုတ်လွှတ်သည့်အခါတွင် ပြတ်သားစွာ ရွေးချယ်ရပါမည်။
- **အိမ်ရှင်ဂိတ်ပေါက်-** `Register<Account>` သည် လက်မှတ်ထိုးထားသည့် algorithms ကိုအသုံးပြုသည့် controllers များကို ငြင်းပယ်သည်
  node ၏ `crypto.allowed_signing` စာရင်း ** သို့မဟုတ် ** မျဉ်းကွေး ခွဲခြားသတ်မှတ်ခြင်းများမှ လွဲချော်နေသည်
  `crypto.curves.allowed_curve_ids`၊ ထို့ကြောင့် အစုအဖွဲ့များသည် ပံ့ပိုးမှုကို ကြော်ငြာရမည် (ဖွဲ့စည်းပုံ +
  ML‑DSA/GOST/SM ထိန်းချုပ်ကိရိယာများကို စာရင်းမသွင်းမီတွင် genesis)။ BLS ထိန်းချုပ်ကိရိယာ
  အယ်လဂိုရီသမ်များကို စုစည်းသောအခါတွင် အမြဲတမ်းခွင့်ပြုထားသည် (သဘောတူညီချက်ကီးများကို ၎င်းတို့အပေါ်တွင် အားကိုးသည်)၊
  ပုံသေဖွဲ့စည်းပုံသည် Ed25519 + secp256k1 ကိုဖွင့်ထားသည်။【crates/iroha_core/src/smartcontracts/isi/domain.rs:32】

##### 2.3.2 Multisig ထိန်းချုပ်ကိရိယာ လမ်းညွှန်ချက်

`AccountController::Multisig` မှတစ်ဆင့် မူဝါဒများကို စီးရီးခွဲသည်။
`crates/iroha_data_model/src/account/controller.rs` နှင့် schema ကို ပြဌာန်းသည်။
[`docs/source/references/multisig_policy_schema.md`](source/references/multisig_policy_schema.md) တွင် မှတ်တမ်းတင်ထားသည်။
အဓိကအကောင်အထည်ဖော်မှုအသေးစိတ်များ-

- မူဝါဒများကို `MultisigPolicy::validate()` မတိုင်မီ ပုံမှန်ပြင်ဆင်ပြီး အတည်ပြုထားသည်။
  မြှုပ်နှံထားသည်။ အဆင့်သတ်မှတ်ချက်များသည် ≥1 နှင့် ≤Σ အလေးချိန်ဖြစ်ရမည်။ ပွားအဖွဲ့ဝင်များဖြစ်ကြပါသည်။
  `(algorithm || 0x00 || key_bytes)` ဖြင့် စီခွဲပြီးနောက် အဆုံးအဖြတ်အတိုင်း ဖယ်ရှားခဲ့သည်။
- binary controller payload (`ControllerPayload::Multisig`) ကုဒ်နံပါတ်များ
  `version:u8`၊ `threshold:u16`၊ `member_count:u8` ထို့နောက် အဖွဲ့ဝင်တစ်ဦးစီ၏
  `(curve_id, weight:u16, key_len:u16, key_bytes)`။ ဒါကအတိအကျဘာလဲ
  `AccountAddress::canonical_bytes()` သည် IH58 (ဦးစားပေး)/sora (ဒုတိယအကောင်းဆုံး) payloads သို့ စာရေးသည်။
- Hashing (`MultisigPolicy::digest_blake2b256()`) သည် Blake2b-256 ကို အသုံးပြုသည်။
  `iroha-ms-policy` စိတ်ကြိုက်သတ်မှတ်ခြင်း string သည် အုပ်ချုပ်မှုဆိုင်ရာဖော်ပြချက်များကို ချိတ်ဆက်နိုင်စေရန်၊
  IH58 တွင် ထည့်သွင်းထားသော ထိန်းချုပ်သူ ဘိုက်များနှင့် ကိုက်ညီသော အဆုံးအဖြတ်ပေးသော မူဝါဒ ID။
- Fixture coverage သည် `fixtures/account/address_vectors.json` (အမှုတွဲများဖြစ်သည်။
  `addr-multisig-*`)။ Wallets နှင့် SDK များသည် canonical IH58 ကြိုးများကို အခိုင်အမာပြုလုပ်သင့်သည်။
  ၎င်းတို့၏ ကုဒ်ပြောင်းစက်များသည် Rust အကောင်အထည်ဖော်မှုနှင့် ကိုက်ညီကြောင်း အတည်ပြုရန် အောက်တွင်။

| Case ID | တံခါးခုံ / အဖွဲ့ဝင် | IH58 ပကတိ (ရှေ့ဆက် `0x02F1`) | Sora compressed (`sora`) ပကတိ | မှတ်စုများ |
|---------|---------------------|--------------------------------------------------------------------------------|--------|
| `addr-multisig-council-threshold3` | `≥3` အလေးချိန် မန်ဘာ `(2,1,1)` | `SRfSHsrH3tEmYaaAYyD248F3vfT1oQ3WEGS22MaD8W9bLefF7rsoKLYGcpbcM9EcSus5ZhCAZU7ztn2BCsyeCAdfRncAVmVsipd4ibk6CBLF3Nrzcw8P7VKJg6mtFgEhWVTjfDkUMoc63oeEmaWyV6cyiphwk8ZgKAJUe4TyVtmKm1WWcg7qZ6i` | `sora3vﾑ2zkaoUwﾋﾅGﾘﾚyﾂe3ﾖfﾙヰｶﾘﾉwｷnoWﾛYicaUr3ﾔｲﾖ2Ado3TﾘYQﾉJqﾜﾇｳﾑﾐd8dDjRGｦ3Vﾃ9HcﾀMヰR8ﾎﾖgEqGｵEｾDyc5ﾁ1ﾔﾉ31sUﾑﾀﾖaｸxﾘ3ｲｷMEuFｺｿﾉBQSVQnxﾈeJzrXLヰhｿｹ5SEEﾅPﾂﾗｸdヰﾋ1bUGHｲVXBWNNJ6K` | ကောင်စီ-ဒိုမိန်း အုပ်ချုပ်မှု အထမြောက်သည်။ |
| `addr-multisig-wonderland-threshold2` | `≥2`, အဖွဲ့ဝင် `(1,2)` | `3xsmkps1KPBn9dtpE5qHRhHEZCpiAe8d9j6H9A42TV6kc1TpaqdwnSksKgQrsSEHznqvWKBMc1os69BELzkLjsR7EV2gjV14d9JMzo97KEmYoKtxCrFeKFAcy7ffQdboV1uRt` | `sora2ﾖZﾘeｴAdx3ﾂﾉﾔXhnｹﾀ2ﾉｱﾋxﾅﾄﾌヱwﾐmﾊvEﾐCﾏﾎｦ1ﾑHﾋso2GKﾔﾕﾁwﾂﾃP6ﾁｼﾙﾖｺ9ｻｦbﾈ4wFdﾑFヰ3HaﾘｼMｷﾌHWtｷﾋLﾙﾖQ4D3XﾊﾜXmpktﾚｻ5ﾅﾅﾇ1gkﾏsCFQGH9` | Dual-signature Wonderland ဥပမာ (အလေးချိန် 1 + 2)။ |
| `addr-multisig-default-quorum3` | `≥3`, အဖွဲ့ဝင် `(1,1,1,1)` | `nA2bDNhMqXz7ERkHNoEWbvJGyR1aDRsw32LaUWLgbK3vcpzohmdFCLvdotxUWWDY3aZeX4ptLk4Z6TjF5ossnJm8VrNo6daxmGTkqUyP4MxJxiNyPFxsEE5DLnsoLWUcxaWNpZ76tmkbiGS31Gv8tejKpuiHUMaQ1s5ohWyZvDnpycNkBK8AEfGJqn5yc9zAzfWbVhpDwkPj8ScnzvH1Echr5` | `soraﾐ38ﾅｴｸﾜ8ﾃzwBrqﾘｺ4yﾄv6kqJp1ｳｱﾛｿrzﾄﾃﾘﾒRﾗtV9ｼﾔPｽcヱEﾌVVVｼﾘｲZAｦﾓﾅｦeﾒN76vﾈcuｶuﾛL54rzﾙﾏX2zMﾌRLﾃﾋpﾚpｲcHﾑﾅﾃﾔzｵｲVfAﾃﾚﾎﾚCヰﾔｲｽｦw9ﾔﾕ8bGGkﾁ6sNｼaｻRﾖﾜYﾕﾚU18ﾅHヰﾌuMeﾊtﾂrｿj95Ft8ﾜ3fﾄkNiｴuﾈrCﾐQt8ヱｸｸmﾙﾒgUbﾑEKTTCM` | အခြေခံ အုပ်ချုပ်ရေးအတွက် သုံးသော သွယ်ဝိုက်သော-မူလ ဒိုရမ်

#### 2.4 မအောင်မြင်သော စည်းမျဉ်းများ (ADDR-1a)

- လိုအပ်သော ခေါင်းစီး + ရွေးပေးစနစ်ထက် ပိုတိုသော သို့မဟုတ် လက်ဝဲဘိုက်များဖြင့် `AccountAddressError::InvalidLength` သို့မဟုတ် `AccountAddressError::UnexpectedTrailingBytes` ကို ထုတ်လွှတ်သည်။
- သီးသန့် `ext_flag` သို့မဟုတ် ပံ့ပိုးမထားသော ဗားရှင်း/အတန်းများကို ကြော်ငြာသည့် ခေါင်းစီးများကို `UnexpectedExtensionFlag`၊ `InvalidHeaderVersion` သို့မဟုတ် `UnknownAddressClass` ကို အသုံးပြု၍ ငြင်းပယ်ရပါမည်။
- အမည်မသိရွေးချယ်သူ/ကွန်ထရိုလာတဂ်များသည် `UnknownDomainTag` သို့မဟုတ် `UnknownControllerTag` ကို မြှင့်တင်သည်။
- အရွယ်အစားကြီးသော သို့မဟုတ် ပုံပျက်နေသော အဓိကပစ္စည်းသည် `KeyPayloadTooLong` သို့မဟုတ် `InvalidPublicKey` တိုးလာသည်။
- အဖွဲ့ဝင် 255 ဦးထက်ကျော်လွန်သော Multisig ထိန်းချုပ်ကိရိယာများသည် `MultisigMemberOverflow` ကိုမြှင့်သည်။
- IME/NFKC ကူးပြောင်းမှုများ- အနံတစ်ဝက် Sora kana သည် ကုဒ်ဖျက်ခြင်းမပြုဘဲ ၎င်းတို့၏ အကျယ်အဝန်းပုံစံများသို့ ပုံမှန်ဖြစ်အောင် ပြုလုပ်နိုင်သည်၊ သို့သော် ASCII `sora` နှင့် IH58 ဂဏန်းများ/စာလုံးများသည် ASCII ဆက်ရှိနေရပါမည်။ အနံအပြည့် သို့မဟုတ် ဘူးခွံများ ခေါက်ထားသော ဖန်သားပြင်များပေါ်တွင် `ERR_MISSING_COMPRESSED_SENTINEL`၊ အကျယ်အဝန်းအပြည့်ရှိသော ASCII ပေးဆောင်မှုများသည် `ERR_INVALID_COMPRESSED_CHAR` တိုးလာကာ checksum များသည် `ERR_CHECKSUM_MISMATCH` ကဲ့သို့ ဖောင်းပွလာပါသည်။ `crates/iroha_data_model/src/account/address.rs` တွင် ပိုင်ဆိုင်မှုစမ်းသပ်မှုများသည် ဤလမ်းကြောင်းများကို ဖုံးအုပ်ထားသောကြောင့် SDK နှင့် wallets များသည် အဆုံးအဖြတ်မအောင်မြင်မှုများကို အားကိုးနိုင်ပါသည်။
- Torii နှင့် SDK တို့၏ `address@domain` (rejected legacy form) နာမည်တူများကို ခွဲခြမ်းစိတ်ဖြာခြင်းသည် ယခု IH58 (ဦးစားပေး)/sora (ဒုတိယအကောင်းဆုံး) သွင်းအားစုများ မအောင်မြင်သောအခါ တူညီသော `ERR_*` ကုဒ်များကို IH58 (ဦးစားပေး)/sora (ဒုတိယအကောင်းဆုံး) သွင်းအားစုများ မအောင်မြင်မီ alias fallback (ဥပမာ mismatch မှားနိုင်ပါသည်၊ ဒိုမိန်းကို ပြန်လည်စစ်ဆေးနိုင်သည်)၊ စကားပြေစာကြောင်းများမှ ခန့်မှန်းခြင်းမပြုဘဲ ဖွဲ့စည်းထားသော အကြောင်းပြချက်များ။
- Local selector payload သည် 12 bytes မျက်နှာပြင် `ERR_LOCAL8_DEPRECATED` ထက်တိုပြီး Local-8 digests များမှ ခက်ခဲသောဖြတ်တောက်မှုကို ထိန်းသိမ်းထားသည်။
- Domainless canonical IH58 literals decode directly to a domainless `AccountId`. Use `ScopedAccountId` only when an interface requires explicit domain context.

#### 2.5 Normative binary vectors

- **သွယ်ဝိုက်သော မူရင်းဒိုမိန်း (`default`၊ seed byte `0x00`)**  
  Canonical hex- `0x020001203b6a27bcceb6a42d62a3a8d02a6f0d73653215771de243a63ac048a18b59da29`။  
  ခွဲခြမ်းစိတ်ဖြာချက်- `0x02` ခေါင်းစီး၊ `0x00` ရွေးပေးစနစ် (သွယ်ဝိုက်သောမူလ)၊ `0x00` ထိန်းချုပ်ကိရိယာ တဂ်၊ `0x01` မျဉ်းကွေး ID (Ed25519)၊ `0x20` ၏နောက်တွင် သော့အလျား 3 ကို ပေးဆောင်ပါ။
- **ဒေသတွင်း ဒိုမိန်းအချေအတင် (`treasury`၊ အစေ့ဘိုက် `0x01`)**  
  Canonical hex- `0x0201b18fe9c1abbac45b3e38fc5d0001208a88e3dd7409f195fd52db2d3cba5d72ca6709bf1d94121bf3748801b40f6f5c`။  
  ခွဲခြမ်းစိတ်ဖြာချက်- `0x02` ခေါင်းစီး၊ ရွေးချယ်သူတဂ် `0x01` အပေါင်း `b1 8f e9 c1 ab ba c4 5b 3e 38 fc 5d`၊ သော့တစ်ချက်ပေးချေမှုနောက်တွင် (`0x00` တက်ဂ်၊ `0x01` မျဉ်းကွေး id-70280NIX၊ Ed25519 သော့)။ယူနစ်စမ်းသပ်မှုများ (`account::address::tests::parse_encoded_accepts_all_formats`) သည် `AccountAddress::parse_encoded` မှတစ်ဆင့် အောက်ဖော်ပြပါ V1 vector များကို အခိုင်အမာ အာမခံပြီး tooling သည် hex၊ IH58 (ဦးစားပေး) နှင့် compressed (`sora`၊ ဒုတိယအကောင်းဆုံး) ပုံစံများတစ်လျှောက် canonical payload ပေါ်တွင် အားကိုးနိုင်ကြောင်း အာမခံပါသည်။ `cargo run -p iroha_data_model --example address_vectors` ဖြင့် တိုးချဲ့ထားသော တပ်ဆင်အား ပြန်လည်ထုတ်ပါ။

| ဒိုမိန်း | အစေ့ဘိုက် | Canonical hex | ချုံ့ထားသည် (`sora`) |
|----------------|-----------------|----------------------------------------------------------------------------------------------------------------|
| ပုံသေ | `0x00` | `0x020001203b6a27bcceb6a42d62a3a8d02a6f0d73653215771de243a63ac048a18b59da29` | `sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE` |
| ဘဏ္ဍာတိုက် | `0x01` | `0x0201b18fe9c1abbac45b3e38fc5d0001208a88e3dd7409f195fd52db2d3cba5d72ca6709bf1d94121bf3748801b40f6f5c` | `sora5ｻu6rﾀCヰTGwﾏ1ﾅヱﾌQｲﾖﾇqCｦヰﾓZQCZRDSSﾅMｱﾙヱｹﾁｸ8ｾeﾄﾛ6C8bZuwﾗｹCZｦRSLQFU` |
| အံ့သြစရာ | `0x02` | `0x0201b8ae571b79c5a80f5834da2b0001208139770ea87d175f56a35466c34c7ecccb8d8a91b4ee37a25df60f5b8fc9b394` | `sora5ｻwﾓyRｿqﾏnMﾀﾙヰKoﾒﾇﾓQｺﾛyｼ3ｸFHB2F5LyPﾐTMZkｹｼw67ﾋVﾕｻr8ﾉGﾇeEnｻVRNKCS` |
| iroha | `0x03` | `0x0201de8b36819700c807083608e2000120ed4928c628d1c2c6eae90338905995612959273a5c63f93636c14614ac8737d1` | `sora5ｻﾜxﾀ7Vｱ7QFeｷMﾂLﾉﾃﾏﾓﾀTﾚgSav3Wnｱｵ4ｱCKｷﾛMﾘzヰHiﾐｱ6ﾃﾉﾁﾐZmﾇ2fiﾎX21P4L` |
| အယ်ဖာ | `0x04` | `0x020146be2154ae86826a3fef0ec0000120ca93ac1705187071d67b83c7ff0efe8108e8ec4530575d7726879333dbdabe7c` | `sora5ｻ9JヱﾈｿuwU6ｴpﾔﾂﾈRqRTds1HﾃﾐｶLVﾍｳ9ﾔhｾNｵVｷyucEﾒGﾈﾏﾍ9sKeﾉDzrｷﾆ742WG1` |
| အိုမီဂါ | `0x05` | `0x0201390d946885bc8416b3d30c9d0001206e7a1cdd29b0b78fd13af4c5598feff4ef2a97166e3ca6f2e4fbfccd80505bf1` | `sora5ｻ3zrﾌuﾚﾄJﾑXQhｸTyN8pzwRkWxmjVﾗbﾚﾕヰﾈoｽｦｶtEEﾊﾐ6GPｿﾓﾊｾEhvPｾｻ3XAJ73F` |
| အုပ်ချုပ်မှု | `0x06` | `0x0201989eb45a80940d187e2c908f0001208a875fff1eb38451577acd5afee405456568dd7c89e090863a0557bc7af49f17` | `sora5ｻiｵﾁyVﾕｽbFpDHHuﾇﾉdﾗｲﾓﾄRﾋAW3frUCｾ5ｷﾘTwdﾚnｽtQiLﾏｼｶﾅXgｾZmﾒヱH58H4KP` |
| အတည်ပြုသူများ | `0x07` | `0x0201e4ffa58704c69afaeb7cc2d7000120ea4a6c63e29c520abef5507b132ec5f9954776aebebe7b92421eea691446d22c` | `sora5ｻﾀLDH6VYﾑNAｾgﾉVﾜtxﾊRXLｹﾍﾔﾌLd93GﾔGeｴﾄYrs1ﾂHｸkYxｹwｿyZﾗxyﾎZoXT1S4N` |
| ရှာဖွေသူ | `0x08` | `0x02013b35422c65c2a83c99c523ad0001201398f62c6d1a457c51ba6a4b5f3dbd2f69fca93216218dc8997e416bd17d93ca` | `sora5ｻ4nmｻaﾚﾚPvNLgｿｱv6MHDeEyﾀovﾉJcpvrﾖ6ﾈCQcCNﾇﾜhﾚﾖyFdTwｸｶHEｱ9rWU8FMB` |
| soranet | `0x09` | `0x0201047d9ea7f5d5dbec3f7bfc58000120fd1724385aa0c75b64fb78cd602fa1d991fdebf76b13c58ed702eac835e9f618` | `sora5ｱｸヱVQﾂcﾁヱRﾓcApｲﾁﾅﾒvﾌﾏfｾNnﾛRJsｿDhﾙuHaﾚｺｦﾌﾍﾈeﾆﾎｺN1UUDｶ6ﾎﾄﾛoRH8JUL` |
| kitsune | `0x0A` | `0x0201e91933de397fd7723dc9a76c00012043a72e714401762df66b68c26dfbdf2682aaec9f2474eca4613e424a0fbafd3c` | `sora5ｻﾚｺヱkfFJfSﾁｼJwﾉLvbpSｷﾔMWFMrbｳｸｲｲyヰKGJﾉｻ4ｹﾕrｽhｺｽzSDヰXAN62AD7RGNS` |
| da | `0x0B` | `0x02016838cf5bb0ce0f3d4f380e1c00012066be7e332c7a453332bd9d0a7f7db055f5c5ef1a06ada66d98b39fb6810c473a` | `sora5ｻNﾒ5SﾐRﾉﾐﾃ62ｿ1ｶｷWFKyF1BcAﾔvｼﾐHqﾙﾐPﾏｴヰ5tｲﾕvnﾙT6ﾀW7mﾔ7ﾇﾗﾂｳ25CXS93` |

ပြန်လည်သုံးသပ်ခြင်း- ဒေတာမော်ဒယ် WG၊ ရေးနည်း WG — ADDR-1a အတွက် ခွင့်ပြုထားသော နယ်ပယ်။

##### Sora Nexus ရည်ညွှန်းအမည်များ

Sora Nexus ကွန်ရက်များသည် `chain_discriminant = 0x02F1` သို့ မူရင်းအတိုင်းဖြစ်သည်
(`iroha_config::parameters::defaults::common::CHAIN_DISCRIMINANT`)။ ဟိ
`AccountAddress::to_ih58` နှင့် `to_compressed_sora` ကူညီပေးသူများ ထို့ကြောင့် ထုတ်လွှတ်သည်
canonical payloadတိုင်းအတွက် တသမတ်တည်း စာသားပုံစံများ။ ရွေးချယ်ထားသော ပွဲစဉ်များ
`fixtures/account/address_vectors.json` (မှတဆင့်ထုတ်လုပ်သည်။
`cargo xtask address-vectors`) ကို အမြန်ကိုးကားရန်အတွက် အောက်တွင် ပြထားသည်။

| အကောင့်/ရွေးချယ်သူ | IH58 ပကတိ (ရှေ့ဆက် `0x02F1`) | Sora compressed (`sora`) ပကတိ |
|--------------------|--------------------------------------------------------------------------------
| `default` ဒိုမိန်း (သွယ်ဝိုက်သောရွေးချယ်မှု၊ မျိုးစေ့ `0x00`) | `6cmzPVPX5jDQFNfiz6KgmVfm1fhoAqjPhoPFn4nx9mBWaFMyUCwq4cw` | `sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE` (ရှင်းလင်းပြတ်သားသောလမ်းကြောင်းဆိုင်ရာ အရိပ်အမြွက်များကို ပေးဆောင်သည့်အခါ နောက်ဆက်တွဲ `@default`) |
| `treasury` (ပြည်တွင်းအချေအတင်ရွေးချယ်မှု၊ မျိုးစေ့ `0x01`) | `34mSYnCXkCzHXm31UDHh7SJfGvC4QPEhwim8z7sys2iHqXpCwCQkjL8KHvkFLSs1vZdJcb37r` | `sora5ｻu6rﾀCヰTGwﾏ1ﾅヱﾌQｲﾖﾇqCｦヰﾓZQCZRDSSﾅMｱﾙヱｹﾁｸ8ｾeﾄﾛ6C8bZuwﾗｹCZｦRSLQFU` |
| Global registry pointer (`registry_id = 0x0000_002A`၊ ညီမျှသော `treasury`) | `3oE9sLeRGP49Cu7mQ1nF4wtKAm29BG4TGLiRsaXe7mhbMP5WZ113nNW1N6RbqF` | `sorakXｹ6NｻﾍﾀﾖSﾜﾖｱ3ﾚ5WﾘﾋQﾅｷｦxgﾛｸcﾁｵﾋkﾋvﾏ8SPﾓﾀｹdｴｴｲW9iCM6AEP` |

ဤစာကြောင်းများသည် CLI (`iroha tools address convert`)၊ Torii မှ ထုတ်လွှတ်သော အရာများနှင့် ကိုက်ညီသည်
တုံ့ပြန်မှုများ (`address_format=ih58|compressed`) နှင့် SDK အကူအညီပေးသူများ၊ ထို့ကြောင့် UX ကို ကော်ပီ/ကူးထည့်ပါ
flows တို့သည် verbatim ကို အားကိုးနိုင်သည်။ တိကျသောလမ်းကြောင်းလမ်းညွှန်ချက် လိုအပ်မှသာ `<address>@<domain>` (rejected legacy form) ကို ဖြည့်စွက်ပါ။ suffix သည် canonical output ၏ အစိတ်အပိုင်းမဟုတ်ပါ။

#### 2.6 အပြန်အလှန်လုပ်ဆောင်နိုင်မှုအတွက် စာသားအတုများ (စီစဉ်ထားသည်)

- ** သစ်လုံးများနှင့် လူသားများအတွက် ကွင်းဆက်ပုံစံ-** `ih:<chain-alias>:<alias@domain>`
  ဝင်ခွင့် Wallets များသည် ရှေ့ဆက်ကို ပိုင်းခြားစိတ်ဖြာပြီး၊ ထည့်သွင်းထားသည့် ကွင်းဆက်ကို အတည်ပြုကာ ပိတ်ဆို့ရပါမည်။
  မကိုက်ညီပါ။
- ** CAIP-10 ပုံစံ-** `iroha:<caip-2-id>:<ih58-addr>` သည် ဘာသာရေးသမားများအတွက်
  ပေါင်းစပ်မှုများ။ ဤမြေပုံကို ပို့ဆောင်ရာတွင် ** အကောင်အထည်မဖော်သေးပါ**
  ကိရိယာများ။
- **စက်အကူအညီများ-** Rust၊ TypeScript/JavaScript၊ Python၊
  နှင့် Kotlin သည် IH58 နှင့် compressed formats (`AccountAddress::to_ih58`၊
  `AccountAddress::parse_encoded` နှင့် ၎င်းတို့၏ SDK တူညီမှုများ)။ CAIP-10 အထောက် အကူများ ဖြစ်ကြပါသည်။
  အနာဂတ်အလုပ်။

#### 2.7 Deterministic IH58 alias

- **Prefix mapping-** `chain_discriminant` ကို IH58 ကွန်ရက်ရှေ့ဆက်အဖြစ် ပြန်သုံးပါ။
  `encode_ih58_prefix()` (`crates/iroha_data_model/src/account/address.rs` ကိုကြည့်ပါ)
  တန်ဖိုး `<64` နှင့် 14-bit၊ two-byte အတွက် 6-bit prefix (single byte) ကို ထုတ်လွှတ်သည်
  ပိုမိုကြီးမားသောကွန်ရက်များအတွက်ပုံစံ။ အုပ်စိုးသော တာဝန်များ ရှင်သန်နေပါသည်။
  [`address_prefix_registry.md`](source/references/address_prefix_registry.md);
  SDK များသည် တိုက်ဆိုင်မှုများကို ရှောင်ရှားရန် ကိုက်ညီသော JSON မှတ်ပုံတင်ခြင်းကို စင့်ခ်လုပ်ထားရပါမည်။
- **အကောင့်ပစ္စည်း-** IH58 က တည်ဆောက်ထားသော canonical payload ကို ကုဒ်လုပ်သည်။
  `AccountAddress::canonical_bytes()`—ခေါင်းစီးဘိုက်၊ ဒိုမိန်းရွေးချယ်သူနှင့်
  controller payload။ နောက်ထပ် တားဆီးခြင်းအဆင့် မရှိပါ။ IH58 တွင် ထည့်သွင်းထားသည်။
  binary controller payload (single key သို့မဟုတ် multisig) ကို Rust မှထုတ်လုပ်သည်။
  ကုဒ်ပြောင်းကိရိယာ၊ CTAP2 မြေပုံသည် multisig မူဝါဒအချေအတင်အတွက် အသုံးပြုသည့် မြေပုံမဟုတ်ပါ။
- ** ကုဒ်သွင်းခြင်း-** `encode_ih58()` သည် ရှေ့ဆက်ဘိုက်များကို canonical နှင့် ပေါင်းစပ်သည်
  payload နှင့် Blake2b-512 မှရရှိသော 16-bit checksum ကို ပုံသေဖြင့် ပေါင်းထည့်သည်။
  ရှေ့စာလုံး `IH58PRE` (`b"IH58PRE" || prefix || payload`)။ ရလဒ်သည် `bs58` မှတစ်ဆင့် Base58-encode လုပ်ထားသည်။
  CLI/SDK အကူအညီပေးသူများသည် တူညီသောလုပ်ထုံးလုပ်နည်းနှင့် `AccountAddress::parse_encoded` ကို ဖော်ထုတ်သည်။
  ၎င်းကို `decode_ih58` မှတဆင့် ပြောင်းပြန်။

#### 2.8 Normative textual test vectors

`fixtures/account/address_vectors.json` တွင် IH58 အပြည့်အစုံ (ဦးစားပေး) နှင့် ချုံ့ထားသည် (`sora`၊ ဒုတိယအကောင်းဆုံး)
canonical payload တိုင်းအတွက် literals ပေါ်လွင်ချက်များ-

- **`addr-single-default-ed25519` (Sora Nexus၊ ရှေ့ဆက် `0x02F1`)**  
  IH58 `6cmzPVPX5jDQFNfiz6KgmVfm1fhoAqjPhoPFn4nx9mBWaFMyUCwq4cw`၊ ဖိသိပ်ထားသည် (`sora`)
  `sora2QG…U4N5E5`။ Torii သည် `AccountId` မှ ဤအတိအကျစာကြောင်းများကို ထုတ်လွှတ်သည်
  `Display` အကောင်အထည်ဖော်မှု (canonical IH58) နှင့် `AccountAddress::to_compressed_sora`။
- **`addr-global-registry-002a` (မှတ်ပုံတင်ရွေးချယ်မှု → ဘဏ္ဍာတိုက်)**  
  IH58 `3oE9sLeRGP49Cu7mQ1nF4wtKAm29BG4TGLiRsaXe7mhbMP5WZ113nNW1N6RbqF`၊ ဖိသိပ်ထားသည် (`sora`)
  `sorakX…CM6AEP`။ မှတ်ပုံတင်ရွေးချယ်ပေးသူများသည် ကုဒ်ကို ကုဒ်လုပ်နေဆဲဖြစ်ကြောင်း သရုပ်ပြသည်။
  သက်ဆိုင်ရာ ဒေသန္တရအနှစ်ချုပ်နှင့် တူညီသော canonical payload ဖြစ်သည်။
- **ပျက်ကွက်ဖြစ်ရပ် (`ih58-prefix-mismatch`)**  
  node တစ်ခုပေါ်တွင် ရှေ့ဆက် `NETWORK_PREFIX + 1` ပါသော IH58 ပကတိကုဒ်ဖြင့် ခွဲခြမ်းစိတ်ဖြာခြင်း
  ပုံသေရှေ့ဆက်အထွက်နှုန်းကို မျှော်လင့်နေသည်။
  `AccountAddressError::UnexpectedNetworkPrefix { expected: 753, found: 754 }`
  domain routing မကြိုးစားမီ။ `ih58-checksum-mismatch` ခံစစ်မှူး
  Blake2b checksum တွင် ဆော့ဖ်ဝဲကို ထောက်လှမ်းခြင်း လေ့ကျင့်ခန်း။

#### 2.9 လိုက်နာမှုစနစ်များ

ADDR‑2 သည် အပြုသဘောနှင့် အနုတ်လက္ခဏာကို ဖုံးအုပ်ထားသော ပြန်လည်ကစားနိုင်သော အတွဲတစ်ခုကို ပေးပို့သည်။
Canonical hex၊ IH58 (ဦးစားပေး)၊ ချုံ့ထားသော (`sora`၊ half-/full-width)၊ သွယ်ဝိုက်သောအားဖြင့်၊
မူရင်းရွေးချယ်သူများ၊ ကမ္ဘာလုံးဆိုင်ရာ မှတ်ပုံတင်ခြင်းအမည်တူများနှင့် လက်မှတ်ပေါင်းများစွာ ထိန်းချုပ်ကိရိယာများ။ ဟိ
canonical JSON သည် `fixtures/account/address_vectors.json` တွင်နေထိုင်ပြီး ဖြစ်နိုင်ပါသည်။
ပြန်လည်ထုတ်ပေးသည်-

```
cargo xtask address-vectors --out fixtures/account/address_vectors.json
# verify without writing:
cargo xtask address-vectors --verify
```

ad-hoc စမ်းသပ်မှုများ (မတူညီသောလမ်းကြောင်းများ/ဖော်မတ်များ) အတွက် ဥပမာ binary သည် ရှိနေသေးသည်။
ရနိုင်သည်-

```
cargo run -p iroha_data_model --example account_address_vectors > fixtures/account/address_vectors.json
```

`crates/iroha_data_model/tests/account_address_vectors.rs` တွင် သံချေးယူနစ်စစ်ဆေးမှုများ
JS နှင့်အတူ `crates/iroha_torii/tests/account_address_vectors.rs`၊
Swift နှင့် Android ကြိုးများ (`javascript/iroha_js/test/address.test.js`၊
`IrohaSwift/Tests/IrohaSwiftTests/AccountAddressTests.swift`၊
`java/iroha_android/src/test/java/org/hyperledger/iroha/android/address/AccountAddressTests.java`)၊
SDKs နှင့် Torii ဝင်ခွင့်အတွက် codec တူညီမှုကိုအာမခံရန် တူညီသော fixture ကိုအသုံးပြုပါ။

### 3. ကမ္ဘာလုံးဆိုင်ရာထူးခြားသောဒိုမိန်းများနှင့် ပုံမှန်ပြုလုပ်ခြင်း။

ကိုလည်းကြည့်ပါ- [`docs/source/references/address_norm_v1.md`](source/references/address_norm_v1.md)
Torii၊ ဒေတာမော်ဒယ်နှင့် SDKs တစ်လျှောက်အသုံးပြုထားသော canonical Norm v1 ပိုက်လိုင်းအတွက်။

တဂ်ထားသော tuple အဖြစ် `DomainId` ကို ပြန်လည်သတ်မှတ်ပါ-

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

`LocalChain` သည် လက်ရှိကွင်းဆက်မှ စီမံခန့်ခွဲသည့် ဒိုမိန်းများအတွက် လက်ရှိအမည်ကို ခြုံငုံထားသည်။
ကမ္ဘာလုံးဆိုင်ရာ မှတ်ပုံတင်ခြင်းမှတစ်ဆင့် ဒိုမိန်းတစ်ခုကို မှတ်ပုံတင်သောအခါ၊ ကျွန်ုပ်တို့သည် ပိုင်ဆိုင်မှုကို ဆက်လက်ထိန်းသိမ်းထားသည်။
ကွင်းဆက်၏ခွဲခြားမှု။ Display/parsing သည် ယခုအချိန်တွင် မပြောင်းလဲသေးသော်လည်း၊
တိုးချဲ့ဖွဲ့စည်းပုံသည် လမ်းကြောင်းဆုံးဖြတ်ချက်များကို ခွင့်ပြုသည်။

#### ၃.၁ ပုံမှန်ပြုလုပ်ခြင်းနှင့် အယောင်ဆောင်ခြင်း ကာကွယ်ရေး

Norm v1 သည် ဒိုမိန်းတစ်ခုရှေ့တွင် အသုံးပြုရမည့် အစိတ်အပိုင်းတိုင်း၏ canonical pipeline ကို သတ်မှတ်သည်။
အမည်သည် `AccountAddress` တွင် ဆက်ရှိနေသည် သို့မဟုတ် ထည့်သွင်းထားသည်။ သင်ခန်းစာအပြည့်အစုံ
[`docs/source/references/address_norm_v1.md`](source/references/address_norm_v1.md);
အောက်တွင် အကျဉ်းချုပ်သည် ပိုက်ဆံအိတ်များ၊ Torii၊ SDK နှင့် အုပ်ချုပ်မှုဆိုင်ရာ အဆင့်များကို ဖမ်းယူပါသည်။
tools တွေကိုအကောင်အထည်ဖော်ရမယ်။

1. **ထည့်သွင်းခြင်းအား အတည်ပြုခြင်း။** ဗလာစာကြောင်းများ၊ နေရာလွတ်များနှင့် သီးသန့်ထားမှုများကို ငြင်းပယ်ပါ။
   အနားသတ်များ `@`, `#`, `$`။ ၎င်းသည် ပြဌာန်းထားသော ပုံစံကွဲများနှင့် ကိုက်ညီပါသည်။
   `Name::validate_str`။
2. **Unicode NFC ဖွဲ့စည်းမှု။** ICU ကျောထောက်နောက်ခံပြုထားသော NFC ကို ပုံမှန်ဖြစ်အောင် ပြုလုပ်ခြင်းအား အဓိပ္ပါယ်ရှိစွာ အသုံးပြုပါ။
   ညီမျှသော sequences များသည် အဆုံးအဖြတ်အရ ပြိုကျသည် (ဥပမာ၊ `e\u{0301}` → `é`)။
3. **UTS-46 normalisation.** NFC အထွက်အား UTS-46 ဖြင့် လုပ်ဆောင်ပါ။
   `use_std3_ascii_rules = true`၊ `transitional_processing = false` နှင့်
   DNS-အရှည် ပြဋ္ဌာန်းချက်ကို ဖွင့်ထားသည်။ ရလဒ်မှာ စာလုံးအသေး A-label sequence ဖြစ်သည်။
   STD3 စည်းမျဉ်းများကို ချိုးဖောက်သော သွင်းအားစုများသည် ဤနေရာတွင် မအောင်မြင်ပါ။
4. ** အရှည်ကန့်သတ်ချက်များ။** DNS စတိုင်ဘောင်များကို တွန်းအားပေးပါ- အညွှန်းတစ်ခုစီသည် 1-63 ဖြစ်ရမည်
   bytes နှင့် domain အပြည့်အစုံသည် အဆင့် 3 ပြီးနောက် 255 bytes ထက် မကျော်လွန်ရပါ။
5. **ရွေးချယ်နိုင်သော ရှုပ်ထွေးနိုင်သောမူဝါဒ။** UTS‑39 script စစ်ဆေးမှုများကို ခြေရာခံထားသည်။
   ပုံမှန် v2; အော်ပရေတာများသည် ၎င်းတို့ကို စောစီးစွာ ဖွင့်နိုင်သော်လည်း စစ်ဆေးမှုကို ပျက်ကွက်ပါက ပယ်ဖျက်ရပါမည်။
   လုပ်ဆောင်ခြင်း။

အဆင့်တိုင်း အောင်မြင်ပါက၊ စာလုံးသေး A-label စာကြောင်းကို ကက်ရှ်လုပ်ပြီး အသုံးပြုသည်။
လိပ်စာကုဒ်သွင်းခြင်း၊ ဖွဲ့စည်းမှုပုံစံ၊ မန်နီးဖက်စ်များနှင့် မှတ်ပုံတင်ခြင်းရှာဖွေခြင်းများ။ ဒေသဆိုင်ရာ မှတ်တမ်း
ရွေးချယ်သူများသည် ၎င်းတို့၏ 12-byte တန်ဖိုးကို `blake2s_mac(key = "SORA-LOCAL-K:v1" အဖြစ် ရယူသည်။
canonical_label)[0..12]` အဆင့် 3 အထွက်ကို အသုံးပြုခြင်း။ အခြားကြိုးစားမှုအားလုံး (ရောထွေး
case, upper-case, raw Unicode input) ကို ဖွဲ့စည်းပုံဖြင့် ပယ်ချပါသည်။
အမည်ပေးထားသည့် နယ်နိမိတ်တွင် `ParseError`s။

Punycode အသွားအပြန် အပါအဝင် ဤစည်းမျဉ်းများကို သရုပ်ပြသည့် Canonical ပြိုင်ပွဲများ
နှင့် မမှန်ကန်သော STD3 sequences — တွင် ဖော်ပြထားပါသည်။
`docs/source/references/address_norm_v1.md` နှင့် SDK CI တွင် ထင်ဟပ်နေပါသည်။
ADDR‑2 အောက်တွင် ခြေရာခံထားသော vector suites

### 4. Nexus ဒိုမိန်း မှတ်ပုံတင်ခြင်းနှင့် လမ်းကြောင်းတင်ခြင်း- **Registry schema-** Nexus သည် `DomainName -> ChainRecord` ရေးထိုးထားသော မြေပုံကို ထိန်းသိမ်းသည်
  `ChainRecord` တွင် ကွင်းဆက်ခွဲခြားမှု၊ ရွေးချယ်နိုင်သော မက်တာဒေတာ (RPC) ပါ၀င်သည်
  အဆုံးမှတ်များ) နှင့် အာဏာပိုင်အထောက်အထား (ဥပမာ၊ အုပ်ချုပ်မှုဆိုင်ရာ လက်မှတ်ပေါင်းများစွာ)။
- ** ထပ်တူကျသော ယန္တရား-**
  - Chains များသည် Nexus သို့ လက်မှတ်ရေးထိုးထားသော ဒိုမိန်းအရေးဆိုမှုများကို တင်သွင်းသည် (ဥပါဒ်ကာလတွင်ဖြစ်စေ သို့မဟုတ် တစ်ဆင့်ချင်းဖြစ်စေ၊
    အုပ်ချုပ်မှုညွှန်ကြားချက်)။
  - Nexus သည် အချိန်အပိုင်းအခြားအလိုက် ဖော်ပြချက်များကို ထုတ်ဝေသည် (လက်မှတ်ထိုးထားသော JSON နှင့် ရွေးချယ်နိုင်သော Merkle အမြစ်)
    HTTPS နှင့် content-addressed storage (ဥပမာ IPFS) ကျော်။ ဖောက်သည်များက pin ကို
    နောက်ဆုံးဖော်ပြချက်များနှင့် လက်မှတ်များကို အတည်ပြုပါ။
- ** ရှာဖွေမှု စီးဆင်းမှု-**
  - Torii သည် `DomainId` ကို ရည်ညွှန်းသော ငွေပေးငွေယူကို လက်ခံရရှိသည် ။
  - ဒိုမိန်းကို စက်တွင်းတွင် မသိပါက၊ Torii သည် သိမ်းဆည်းထားသော Nexus မန်နီးဖက်စ်ကို မေးမြန်းသည်။
  - မန်နီးဖက်စ်သည် နိုင်ငံခြားကွင်းဆက်တစ်ခုကို ညွှန်ပြပါက၊ ငွေပေးငွေယူကို ငြင်းပယ်သည်။
    အဆုံးအဖြတ်ပေးသော `ForeignDomain` အမှားနှင့် အဝေးထိန်းကွင်းဆက်အချက်အလက်။
  - ဒိုမိန်းသည် Nexus မှ ပျောက်ဆုံးနေပါက Torii သည် `UnknownDomain` သို့ ပြန်သွားပါသည်။
- **ယုံကြည်စိတ်ချရသောကျောက်ဆူးများနှင့်လည်ပတ်ခြင်း-** အုပ်ချုပ်မှုသော့များ နိမိတ်လက္ခဏာများသည် ထင်ရှားသည်။ လည်ပတ်မှု သို့မဟုတ်
  ရုတ်သိမ်းခြင်းကို ဖော်ပြချက်အသစ်တစ်ခုအဖြစ် ထုတ်ပြန်ထားသည်။ ဖောက်သည်များက ထင်ရှားစွာ ပြဋ္ဌာန်းသည်။
  TTL များ (ဥပမာ၊ 24 နာရီ) နှင့် ထိုဝင်းဒိုးအပြင်ဘက်ရှိ ဟောင်းနွမ်းနေသောဒေတာများကို တိုင်ပင်ရန် ငြင်းဆိုပါ။
- **Failure modes-** manifest retrieval မအောင်မြင်ပါက၊ Torii သည် cached သို့ ပြန်ရောက်သွားသည်
  TTL အတွင်းဒေတာ၊ ယခင်က TTL သည် `RegistryUnavailable` ကို ထုတ်လွှတ်ပြီး ငြင်းဆိုသည်။
  တစ်သမတ်တည်းမဖြစ်စေရန် ဒိုမိန်းဖြတ်ကျော်လမ်းကြောင်း။

### 4.1 မှတ်ပုံတင်ခြင်း မပြောင်းလဲနိုင်သော၊ နာမည်တူများနှင့် သင်္ချိုင်းကျောက်များ (ADDR-7c)

Nexus သည် **နောက်ဆက်တွဲ-သပ်သပ် မန်နီးဖက်စ်** ကို ထုတ်ဝေသည် ထို့ကြောင့် ဒိုမိန်း သို့မဟုတ် နာမည်တူ လုပ်ဆောင်ချက်တိုင်း
စာရင်းစစ်ပြီး ပြန်ကစားနိုင်ပါတယ်။ အော်ပရေတာတွင်ဖော်ပြထားသောအစုအဝေးကိုကုသရမည်ဖြစ်သည်။
[address manifest runbook](source/runbooks/address_manifest_ops.md) အဖြစ်
အမှန်တရား၏ တစ်ခုတည်းသော အရင်းအမြစ်- ဖော်ပြချက်တစ်ခု ပျောက်ဆုံးနေပါက သို့မဟုတ် အတည်ပြုခြင်း ပျက်ကွက်ပါက၊ Torii ဖြစ်ရမည်
ထိခိုက်သောဒိုမိန်းကို ဖြေရှင်းရန် ငြင်းဆိုပါ။

အလိုအလျောက်စနစ်ပံ့ပိုးမှု- `cargo xtask address-manifest verify --bundle <current_dir> --previous <previous_dir>`
checksum၊ schema နှင့် ယခင် digest checks များကို ပြန်ဖွင့်သည်။
ရှေ့ပြေးစာအုပ်။ `sequence` ကိုပြသရန် အပြောင်းအလဲ လက်မှတ်များတွင် အမိန့်ပေးသည့်အထွက်ကို ထည့်သွင်းပါ။
အစုအဝေးကို မထုတ်ဝေမီ `previous_digest` လင့်ခ်ကို အတည်ပြုခဲ့သည်။

#### ခေါင်းစီးနှင့် လက်မှတ် စာချုပ်ကို ဖော်ပြပါ။

| လယ် | လိုအပ်ချက် |
|--------|-------------|
| `version` | လောလောဆယ် `1`။ ကိုက်ညီသော spec အပ်ဒိတ်တစ်ခုဖြင့်သာ ဖိထားပါ။ |
| `sequence` | ထုတ်ဝေမှုတစ်ခုလျှင် ** အတိအကျ** ဖြင့် တိုးပါ။ Torii ကက်ရှ်များသည် ကွက်လပ်များ သို့မဟုတ် နောက်ပြန်ဆုတ်မှုများဖြင့် ပြန်လည်ပြင်ဆင်မှုများကို ငြင်းဆန်သည်။ |
| `generated_ms` + `ttl_hours` | ကက်ရှ် လတ်ဆတ်မှုကို သတ်မှတ်ပါ (မူလ 24 နာရီ)။ နောက်ထုတ်ဝေမှုမတိုင်မီ TTL သက်တမ်းကုန်သွားပါက၊ Torii သည် `RegistryUnavailable` သို့ပြောင်းသည်။ |
| `previous_digest` | ကြိုတင်ထင်ရှားသည့်ကိုယ်ထည်၏ BLAKE3 ချေဖျက်မှု (hex)။ မပြောင်းလဲနိုင်ကြောင်း သက်သေပြရန်အတွက် Verifier များသည် `b3sum` ဖြင့် ပြန်လည်တွက်ချက်ပါသည်။ |
| `signatures` | မန်နီးဖက်စ်များကို Sigstore (`cosign sign-blob`) မှတစ်ဆင့် လက်မှတ်ရေးထိုးထားသည်။ Ops သည် `cosign verify-blob --bundle manifest.sigstore manifest.json` ကို run ပြီး စတင်မဖြန့်ချိမီ အုပ်ချုပ်မှုအထောက်အထားအထောက်အထား/ထုတ်ပေးသူ ကန့်သတ်ချက်များကို လိုက်နာရပါမည်။ |

ထုတ်ပေးသည့် အလိုအလျောက်စနစ်သည် `manifest.sigstore` နှင့် `checksums.sha256` ကို ထုတ်လွှတ်သည်
JSON ကိုယ်ထည်နှင့်အတူ။ SoraFS သို့ mirroring လုပ်သည့်အခါ ဖိုင်များကို အတူတူထားပါ။
စာရင်းစစ်များသည် အတည်ပြုခြင်းအဆင့်များကို ပြန်ဖွင့်နိုင်စေရန် HTTP အဆုံးမှတ်များ။

#### ဝင်ခွင့်အမျိုးအစားများ

| ရိုက် | ရည်ရွယ်ချက် | လိုအပ်သောနယ်ပယ်များ |
|--------|---------|-----------------|
| `global_domain` | ဒိုမိန်းတစ်ခုအား တစ်ကမ္ဘာလုံးတွင် မှတ်ပုံတင်ထားပြီး ကွင်းဆက်ခွဲခြားမှုနှင့် IH58 ရှေ့နောက်ဆက်တွဲနှင့် မြေပုံသင့်သည်ဟု ကြေညာသည်။ | `{ "domain": "<label>", "chain": "sora:nexus:global", "ih58_prefix": 753, "selector": "global" }` |
| `tombstone` | အမည်များ/ရွေးချယ်သူအား အပြီးအပိုင် အနားယူပါ။ Local‑8 အချေအတင်များကို ဖျက်သည့်အခါ သို့မဟုတ် ဒိုမိန်းတစ်ခုကို ဖယ်ရှားသည့်အခါ လိုအပ်သည်။ | `{ "selector": {…}, "reason_code": "LOCAL8_RETIREMENT" \| …, "ticket": "<governance id>", "replaces_sequence": <number> }` |

`global_domain` တွင် `manifest_url` သို့မဟုတ် `sorafs_cid` တို့ကို ရွေးချယ်နိုင်သည်
လက်မှတ်ထိုးထားသော ကွင်းဆက်မက်တာဒေတာတွင် ပိုက်ဆံအိတ်များကို ညွှန်ရန်၊ သို့သော် canonical tuple ကျန်ရှိနေပါသည်။
`{domain, chain, discriminant/ih58_prefix}`။ `tombstone` မှတ်တမ်းများ ** ကိုးကားရပါမည်။
အငြိမ်းစားယူမည့် ရွေးချယ်သူနှင့် ခွင့်ပြုထားသည့် လက်မှတ်/အုပ်ချုပ်မှုဆိုင်ရာ ပစ္စည်းများ
အပြောင်းအလဲကြောင့် စာရင်းစစ်လမ်းကြောင်းကို အော့ဖ်လိုင်းဖြင့် ပြန်လည်တည်ဆောက်နိုင်သည်။

#### Alias/tombstone အလုပ်အသွားအလာနှင့် တယ်လီမီတာ

1. **Detect drift.** `torii_address_local8_total{endpoint}` ကိုသုံးပါ၊
   `torii_address_local8_domain_total{endpoint,domain}`၊
   `torii_address_collision_total{endpoint,kind="local12_digest"}`၊
   `torii_address_collision_domain_total{endpoint,domain}`၊
   `torii_address_domain_total{endpoint,domain_kind}` နှင့်
   `torii_address_invalid_total{endpoint,reason}` (ဘာသာပြန်ဆိုသည်။
   `dashboards/grafana/address_ingest.json`) Local တင်ပြမှုများနှင့် အတည်ပြုရန်
   အုတ်ဂူကျောက်တုံးတစ်ခု မတင်ပြမီ Local-12 တိုက်မိမှု သုညတွင် ရှိနေသည်။ ဟိ
   per-domain ကောင်တာများသည် dev/test domains များသာ Local-8 ကို ထုတ်လွှတ်ကြောင်း ပိုင်ရှင်များကို သက်သေပြနိုင်စေပါသည်။
   ယာဉ်ကြောအသွားအလာ (နှင့် Local-12 တိုက်မိမှုများသည် လူသိများသော အဆင့်သတ်မှတ်ထားသော ဒိုမိန်းများထံသို့ မြေပုံဆွဲထားသည်)
   **Domain Kind Mix (5m)** panel ပါ၀င်သောကြောင့် SREs မည်မျှဂရပ်ဖစ်နိုင်သည်
   `domain_kind="local12"` အသွားအလာ ကျန်နေသေးပြီး `AddressLocal12Traffic`
   ထုတ်လုပ်မှုသည် Local-12 ရွေးချယ်မှုများကို မြင်သည့်အခါတိုင်း သတိပေးချက် မီးလောင်နေပါသည်။
   အငြိမ်းစားဂိတ်။
2. ** Canonical digests များကိုရယူပါ။** Run
   `iroha tools address convert <address> --format json --expect-prefix 753`
   (သို့မဟုတ် `fixtures/account/address_vectors.json` မှတဆင့် စားသုံးပါ။
   `scripts/account_fixture_helper.py`) အတိအကျ `digest_hex` ကို ဖမ်းယူပါ။
   CLI သည် IH58၊ `sora…` နှင့် canonical `0x…` တို့ကို လက်ခံသည်။ နောက်ဆက်တွဲ
   ဖော်ပြချက်များအတွက် အညွှန်းတစ်ခုကို ထိန်းသိမ်းထားရန် လိုအပ်မှသာ `@<domain>`။
   JSON ၏ အနှစ်ချုပ်သည် `input_domain` အကွက်မှတစ်ဆင့် အဆိုပါဒိုမိန်းကို မျက်နှာပြင်ပေါ်တွင် လည်းကောင်း၊
   `legacy  suffix` အတွက် `<address>@<domain>` (rejected legacy form) အဖြစ် ပြောင်းလဲထားသော ကုဒ်နံပါတ်ကို ပြန်ဖွင့်သည်
   manifest diffs (ဤနောက်ဆက်တွဲသည် မက်တာဒေတာဖြစ်ပြီး၊ canonical account id မဟုတ်ဘဲ)။
   လိုင်းသစ်ဆန်သော ပို့ကုန်များအတွက် အသုံးပြုပါ။
   `iroha tools address normalize --input <file> legacy-selector input mode` ကို အစုလိုက်အပြုံလိုက်-ဒေသတွင်းသို့ ပြောင်းလဲရန်
   ကျော်သွားနေစဉ် canonical IH58 (ဦးစားပေး)၊ ချုံ့ထားသော (`sora`၊ ဒုတိယအကောင်းဆုံး)၊ hex သို့မဟုတ် JSON ဖောင်များအဖြစ် ရွေးချယ်မှုများ
   ဒေသတွင်းမဟုတ်သောအတန်းများ။ စာရင်းစစ်များသည် စာရင်းဇယားနှင့်လိုက်ဖက်သော အထောက်အထားများ လိုအပ်သောအခါ၊ လုပ်ဆောင်ပါ။
   CSV အနှစ်ချုပ်ကို ထုတ်လွှတ်ရန် `iroha tools address audit --input <file> --format csv`
   Local selectors များကို မီးမောင်းထိုးပြသော (`input,status,format,domain_kind,…`)၊
   canonical ကုဒ်နံပါတ်များနှင့် တူညီသောဖိုင်တွင် ခွဲခြမ်းစိတ်ဖြာမှု မအောင်မြင်ပါ။
3. **မန်နီးဖက်စ်ထည့်သွင်းမှုများကို ဖြည့်စွက်ပါ။** `tombstone` မှတ်တမ်းကို ရေးဆွဲပါ (နှင့် နောက်ဆက်တွဲ
   ကမ္ဘာလုံးဆိုင်ရာ မှတ်ပုံတင်ခြင်းသို့ ပြောင်းရွှေ့သည့်အခါ `global_domain` မှတ်တမ်း) နှင့် အတည်ပြုပါ
   လက်မှတ်မတောင်းမီ `cargo xtask address-vectors` ဖြင့် သရုပ်ပြပါ။
4. **အတည်ပြုပြီး ထုတ်ဝေပါ။** runbook စစ်ဆေးရန်စာရင်းကို လိုက်နာပါ (hashes၊ Sigstore၊
   အစုအဝေးကို SoraFS သို့မပြောင်းမီ sequence monotonicity)။ ယခု Torii
   အစုအဝေးများရောက်ရှိပြီးနောက်ချက်ချင်း IH58 (ဦးစားပေး)/sora (ဒုတိယအကောင်းဆုံး) စာလုံးများကို canonicalize လုပ်သည်။
5. **စောင့်ကြည့်ပြီး ပြန်လှည့်ပါ။** Local-8 နှင့် Local-12 တိုက်မှုအကန့်များကို ထားရှိပါ။
   30 ရက်များအတွက်သုည; ဆုတ်ယုတ်မှုများ ပေါ်လာပါက၊ ယခင်မန်နီးဖက်စ်ကို ပြန်လည်ထုတ်ဝေပါ။
   တယ်လီမီတာ တည်ငြိမ်သည်အထိ ထိခိုက်သည့် ထုတ်လုပ်မှုမဟုတ်သော ပတ်ဝန်းကျင်တွင်သာ။

အထက်ဖော်ပြပါအဆင့်များအားလုံးသည် ADDR-7c အတွက်မဖြစ်မနေအထောက်အထားများဖြစ်သည်- မပါဘဲဖော်ပြသည်။
`cosign` လက်မှတ်အစုအဝေး သို့မဟုတ် `previous_digest` တန်ဖိုးများ ကိုက်ညီမှုမရှိဘဲ လိုအပ်သည်
အလိုအလျောက်ပယ်ချခံရပြီး အော်ပရေတာများသည် အတည်ပြုချက်မှတ်တမ်းများကို ပူးတွဲတင်ပြရမည်ဖြစ်သည်။
သူတို့၏ လက်မှတ်များ

### 5. Wallet & API ergonomics

- ** မျက်နှာပြင်ပုံသေများ-** Wallets များသည် IH58 လိပ်စာကိုပြသသည် (တိုတောင်းသည်၊ အမှန်ခြစ်ပါ)
  ထို့အပြင် မှတ်ပုံတင်ခြင်းမှ ရယူထားသော အညွှန်းအဖြစ် ဖြေရှင်းထားသော ဒိုမိန်း။ ဒိုမိန်းများ
  IH58 သည် ပြောင်းလဲနိုင်သည့် ဖော်ပြချက် မက်တာဒေတာအဖြစ် ရှင်းလင်းစွာ မှတ်သားထားသည်။
  တည်ငြိမ်သောလိပ်စာ။
- **ထည့်သွင်းမှုကို စံပြုသတ်မှတ်ခြင်း-** Torii နှင့် SDK များသည် IH58 (ဦးစားပေး)/sora (ဒုတိယအကောင်းဆုံး)/0x လက်ခံသည်။
  လိပ်စာများ ပေါင်း `alias@domain` (rejected legacy form)၊  `uaid:…` နှင့်
  `opaque:…` ဖောင်များကို ထုတ်ပြီးနောက် IH58 သို့ canonicalize လုပ်ပါ။ မရှိဘူး။
  တင်းကျပ်သောမုဒ်ပြောင်းရန်; ဖုန်း/အီးမေးလ် အကြမ်းဖျင်း သတ်မှတ်ချက်များကို လယ်ဂျာတွင် သိမ်းဆည်းထားရမည်။
  UAID/opaque မြေပုံများမှတဆင့်။
- **အမှားကာကွယ်ခြင်း-** Wallets များသည် IH58 ရှေ့ဆက်များကို ခွဲခြမ်းစိပ်ဖြာပြီး ခွဲခြားဆက်ဆံမှုများကို တွန်းအားပေးရန်
  မျှော်လင့်ချက်များ။ ကွင်းဆက်မတူညီမှုများသည် ပြင်းထန်စွာလုပ်ဆောင်နိုင်သော စစ်ဆေးမှုများဖြင့် ပျက်ကွက်မှုများကို ဖြစ်စေသည်။
- ** Codec စာကြည့်တိုက်များ-** တရားဝင် Rust၊ TypeScript/JavaScript၊ Python နှင့် Kotlin
  စာကြည့်တိုက်များသည် IH58 ကုဒ်/ကုဒ်ဖြင့် ကုဒ်လုပ်ခြင်း နှင့် ချုံ့ထားသော (`sora`) ကို ပံ့ပိုးပေးသည်
  အစိတ်စိတ်အမွှာမွှာ အကောင်အထည်ဖော်မှုများကို ရှောင်ကြဉ်ပါ။ CAIP-10 ပြောင်းလဲမှုများကို မတင်ပို့ရသေးပါ။

#### အများသုံးစွဲနိုင်မှုနှင့် လုံခြုံစွာမျှဝေခြင်းလမ်းညွှန်- ထုတ်ကုန်မျက်နှာပြင်များအတွက် အကောင်အထည်ဖော်မှုလမ်းညွှန်ကို တိုက်ရိုက်ခြေရာခံထားသည်။
  `docs/portal/docs/reference/address-safety.md`; ကိုးကားတာကတော့ checklist လုပ်လိုက်တာ
  ဤလိုအပ်ချက်များကို wallet သို့မဟုတ် explorer UX နှင့် လိုက်လျောညီထွေဖြစ်အောင်ပြုလုပ်ခြင်း။
- **Safe sharing flows-** မျက်နှာပြင်များသည် IH58 ဖောင်တွင် မူရင်းလိပ်စာကို ကူးယူ သို့မဟုတ် ပြသသည့် မျက်နှာပြင်များနှင့် စာကြောင်းအပြည့်အစုံနှင့် တူညီသော payload မှဆင်းသက်လာသော QR ကုဒ်နှစ်ခုစလုံးကို အသုံးပြုသူများသည် checksum ကို အမြင်အားဖြင့် သို့မဟုတ် စကင်ဖတ်ခြင်းဖြင့် အတည်ပြုနိုင်သည့် ကပ်လျက် “share” လုပ်ဆောင်ချက်ကို ဖော်ထုတ်သည်။ ဖြတ်တောက်ခြင်းကို ရှောင်လွှဲ၍မရသောအခါ (ဥပမာ၊ သေးငယ်သောစခရင်များ)၊ စာကြောင်း၏အစနှင့်အဆုံးကို ထိန်းသိမ်းပါ၊ ထင်ရှားသော ဘဲဥပုံများထည့်ကာ မတော်တဆဖြတ်ရိုက်ခြင်းမှ ကာကွယ်ရန် လိပ်စာအပြည့်အစုံကို မိတ္တူမှ ကလစ်ဘုတ်ဖြင့် သိမ်းထားပါ။
- **IME အကာအကွယ်များ-** လိပ်စာထည့်သွင်းမှုများသည် IME/IME စတိုင်ကီးဘုတ်များမှ ဖွဲ့စည်းမှုလက်ရာများကို ငြင်းပယ်ရပါမည်။ ASCII သီးသန့်ထည့်သွင်းမှုကို တွန်းအားပေးပါ၊ အကျယ်အဝန်းအပြည့်အစုံ သို့မဟုတ် Kana စာလုံးများကို တွေ့ရှိသည့်အခါ အတွင်းလိုင်းသတိပေးချက်ကို တင်ပြပြီး တရားဝင်မှုမပြုလုပ်မီ အမှတ်အသားများကို ပေါင်းစပ်ထားသော အမှတ်အသားများကို ဖယ်ထုတ်သည့် ရိုးရှင်းသောစာသား paste ဇုန်ကို ပေးဆောင်ခြင်းဖြင့် ဂျပန်နှင့် တရုတ်အသုံးပြုသူများသည် ၎င်းတို့၏ IME ကို တိုးတက်မှုမပျက်ဘဲ ပိတ်ထားနိုင်ပါသည်။
- **Screen-reader ပံ့ပိုးမှု-** အမြင်အာရုံဖြင့် ဝှက်ထားသော အညွှန်းများ (`aria-label`/`aria-describedby`) သည် ထိပ်တန်း Base58 ရှေ့ရှေ့ဂဏန်းများကို ဖော်ပြပြီး IH58 payload အား စာလုံး 4 လုံး သို့မဟုတ် 8 လုံးဖြင့် အပိုင်းပိုင်းခွဲပေးသည်၊ ထို့ကြောင့် assistive technology အစား run-string အက္ခရာများကို ဖတ်ပါသည်။ ယဉ်ကျေးသော တိုက်ရိုက်ထုတ်လွှင့်သည့် ဒေသများမှတစ်ဆင့် အောင်မြင်မှုကို ကူးယူခြင်း/မျှဝေခြင်းအား ကြေညာပြီး QR အစမ်းကြည့်ရှုမှုများတွင် သရုပ်ဖော်ထားသော alt စာသား (“IH58 လိပ်စာသည် ကွင်းဆက် 0x02F1” အတွက် <alias> အတွက် လိပ်စာ) ပါ၀င်ကြောင်း သေချာပါစေ။
- **Sora-only ချုံ့ထားသောအသုံးပြုမှု-** `sora…` ကို "Sora-only" အဖြစ် အမြဲတံဆိပ်တပ်ပြီး မကူးယူမီ တိကျသေချာသော အတည်ပြုချက်ကို နောက်ကွယ်တွင် ပိတ်လိုက်ပါ။ ကွင်းဆက်ခွဲခြားသတ်မှတ်သူသည် Sora Nexus တန်ဖိုးမဟုတ်သည့်အခါ SDK နှင့် ပိုက်ဆံအိတ်များသည် ချုံ့ထားသောအထွက်ကိုပြသရန် ငြင်းဆိုရမည်ဖြစ်ပြီး ရန်ပုံငွေများလွဲမှားနေခြင်းကိုရှောင်ရှားရန်အတွက် သုံးစွဲသူများအား IH58 သို့ ပြန်လည်ပို့ဆောင်သင့်သည်။

## အကောင်အထည်ဖော်မှုစာရင်း

- **IH58 စာအိတ်-** Prefix သည် compact ကို အသုံးပြု၍ `chain_discriminant` ကို ကုဒ်လုပ်သည်
  `encode_ih58_prefix()` မှ 6-/14-bit scheme၊ ကိုယ်ထည်သည် canonical bytes ဖြစ်သည်
  (`AccountAddress::canonical_bytes()`) နှင့် checksum သည် ပထမနှစ် bytes ဖြစ်သည်။
  Blake2b-512(`b"IH58PRE"` || ရှေ့ဆက် || ကိုယ်ထည်)။ ဝန်ဆောင်ခ အပြည့်အစုံမှာ Base58- ဖြစ်သည်။
  `bs58` မှတဆင့် ကုဒ်လုပ်ထားသည်။
- **Registry စာချုပ်-** လက်မှတ်ထိုးထားသော JSON (နှင့် ရွေးချယ်နိုင်သော Merkle root) ထုတ်ဝေခြင်း။
  24h TTL နှင့် `{discriminant, ih58_prefix, chain_alias, endpoints}`
  အလှည့်ကျသော့များ
- ** ဒိုမိန်းမူဝါဒ-** ယနေ့ ASCII `Name`; i18n ကိုဖွင့်ထားပါက UTS-46 ကိုအသုံးပြုပါ။
  ရှုပ်ထွေးနိုင်သောစစ်ဆေးမှုများအတွက် ပုံမှန်ပြုလုပ်ခြင်းနှင့် UTS-39။ အများဆုံး တံဆိပ် (၆၃) နှင့် ပြဋ္ဌာန်းရန်
  စုစုပေါင်း (၂၅၅) အရှည်။
- **စာသားအကူအညီများ-** Ship IH58 ↔ ချုံ့ထားသော (`sora…`) ကုဒ်ဒစ်များကို Rust၊
  မျှဝေထားသော စမ်းသပ် vector များဖြင့် TypeScript/JavaScript၊ Python နှင့် Kotlin (CAIP-10
  မြေပုံဆွဲခြင်းများသည် အနာဂတ်တွင် ဆက်လက်တည်ရှိနေပါသည်။)
- **CLI tooling-** `iroha tools address convert` မှတစ်ဆင့် အဆုံးအဖြတ်ပေးသော အော်ပရေတာလုပ်ငန်းအသွားအလာကို ပေးဆောင်ပါ။
  (`crates/iroha_cli/src/address.rs` ကိုကြည့်ပါ)၊ IH58/`sora…`/`0x…` လက်ခံသည့် လည်းကောင်း၊
  ရွေးချယ်နိုင်သော `<address>@<domain>` (rejected legacy form) အညွှန်းများ၊ Sora Nexus ရှေ့ဆက် (`753`) ကို အသုံးပြု၍ IH58 အထွက်အတွက် ပုံသေများ
  အော်ပရေတာများမှ ပြတ်သားစွာ တောင်းဆိုသောအခါတွင်သာ Sora-only compressed အက္ခရာကို ထုတ်လွှတ်သည်
  `--format compressed` သို့မဟုတ် JSON အနှစ်ချုပ်မုဒ်။ အမိန့်သည် ရှေ့ထွက်မျှော်လင့်ချက်များကို တွန်းအားပေးသည်။
  ခွဲခြမ်းစိတ်ဖြာပြီး ပံ့ပိုးပေးထားသော ဒိုမိန်း (JSON တွင် `input_domain`) နှင့် `legacy  suffix` အလံကို မှတ်တမ်းတင်သည်
  `<address>@<domain>` (rejected legacy form) အဖြစ် ပြောင်းထားသော ကုဒ်နံပါတ်ကို ပြန်ဖွင့်သည် ထို့ကြောင့် ထင်ရှားသော ကွဲပြားမှုများသည် အံဝင်ခွင်ကျ ရှိနေပါသည်။
- **Wallet/explorer UX:** [လိပ်စာဖော်ပြလမ်းညွှန်ချက်များ](source/sns/address_display_guidelines.md) ကို လိုက်နာပါ။
  ADDR-6 ဖြင့် ပေးပို့သည်—မိတ္တူနှစ်ခုခလုတ်များကို ကမ်းလှမ်းပါ၊ IH58 အား QR ပေးဆောင်မှုအဖြစ် ထားရှိကာ သတိပေးပါ
  ဖိသိပ်ထားသော `sora…` ဖောင်သည် Sora သီးသန့်ဖြစ်ပြီး IME ပြန်လည်ရေးသားခြင်းကို ခံရနိုင်သော အသုံးပြုသူများ။
- **Torii ပေါင်းစည်းခြင်း-** Cache Nexus သည် TTL ကိုလေးစားပြီး ထုတ်လွှတ်သည်
  `ForeignDomain`/`UnknownDomain`/`RegistryUnavailable` တိကျစွာ လည်းကောင်း၊
  keep account-literal parsing encoded-only (`IH58` preferred, `sora…`
  compressed accepted) with canonical IH58 output.

### Torii တုံ့ပြန်မှုဖော်မတ်များ

- `GET /v1/accounts` သည် ရွေးချယ်နိုင်သော `address_format` မေးမြန်းမှု ကန့်သတ်ချက်တစ်ခုကို လက်ခံပြီး
  `POST /v1/accounts/query` သည် JSON စာအိတ်အတွင်းရှိ တူညီသောအကွက်ကို လက်ခံသည်။
  ပံ့ပိုးထားသော တန်ဖိုးများမှာ-
  - `ih58` (မူလ) — တုံ့ပြန်မှုများသည် canonical IH58 Base58 payloads (ဥပမာ၊
    `6cmzPVPX5jDQFNfiz6KgmVfm1fhoAqjPhoPFn4nx9mBWaFMyUCwq4cw`)။
  - `compressed` — တုံ့ပြန်မှုများသည် Sora-only `sora…` ချုံ့ထားသည့်မြင်ကွင်းကို ထုတ်လွှတ်သည်
    filters/path parameters များကို canonical ထားရှိခြင်း။
- မမှန်ကန်သော တန်ဖိုးများသည် `400` (`QueryExecutionFail::Conversion`) သို့ ပြန်ပေးသည်။ ဒီလိုလုပ်ပေးတယ်။
  Sora-only UX အတွက် ချုံ့ထားသော ကြိုးများကို တောင်းဆိုရန် ပိုက်ဆံအိတ်များနှင့် စူးစမ်းရှာဖွေသူများ
  IH58 ကို အပြန်အလှန်လုပ်ဆောင်နိုင်သော ပုံသေအဖြစ် ထိန်းသိမ်းထားသည်။
- ပိုင်ဆိုင်မှုကိုင်ဆောင်သူစာရင်းများ (`GET /v1/assets/{definition_id}/holders`) နှင့် ၎င်းတို့၏ JSON
  စာအိတ်တွဲ (`POST …/holders/query`) ကိုလည်း `address_format` ကို ဂုဏ်ပြုပါသည်။
  `items[*].account_id` အကွက်သည် ချုံ့ထားသော စာလုံးများကို အချိန်တိုင်း ထုတ်လွှတ်ပါသည်။
  ပါရာမီတာ/စာအိတ်အကွက်ကို `compressed` တွင် သတ်မှတ်ထားပြီး အကောင့်များကို ထင်ဟပ်စေသည်
  လမ်းကြောင်းများတစ်လျှောက် စူးစမ်းရှာဖွေသူများသည် တစ်သမတ်တည်းထွက်ရှိမှုကို တင်ပြနိုင်စေရန် အဆုံးမှတ်များ။
- **စမ်းသပ်ခြင်း-** ကုဒ်ကုဒ်/ကုဒ်ဒါဖြင့် အသွားအပြန်ခရီးများအတွက် ယူနစ်စမ်းသပ်မှုများ ထည့်ပါ
  ပျက်ကွက်မှုများ၊ ရှာဖွေမှုများကို ထင်ရှားစွာပြသပါ။ Torii နှင့် SDKs များတွင် ပေါင်းစပ်လွှမ်းခြုံမှုကို ထည့်ပါ။
  IH58 သည် အဆုံးမှ အဆုံးအထိ စီးဆင်းသည်။

## Error Code Registry

လိပ်စာကုဒ်ကိရိယာများနှင့် ကုဒ်ကုဒ်များသည် ပျက်ကွက်မှုများကို ဖော်ထုတ်ပေးသည်။
`AccountAddressError::code_str()`။ အောက်ပါဇယားများသည် တည်ငြိမ်သောကုဒ်များကို ပေးဆောင်သည်။
SDKs၊ ပိုက်ဆံအိတ်များနှင့် Torii မျက်နှာပြင်များသည် လူသားဖတ်နိုင်သော မျက်နှာပြင်များနှင့်အတူ ပေါ်နေသင့်သည်
မက်ဆေ့ချ်များ၊ နှင့် အကြံပြုထားသော ပြန်လည်ပြင်ဆင်ခြင်းလမ်းညွှန်။

### Canonical ဆောက်လုပ်ရေး

| ကုတ် | ပျက်ကွက် | ပြုပြင်ခြင်း |
|--------|---------------------|--------------------------------|
| `ERR_UNSUPPORTED_ALGORITHM` | Encoder သည် စာရင်းသွင်းခြင်းဆိုင်ရာ အယ်လဂိုရီသမ်ကို လက်ခံရရှိသည် သို့မဟုတ် တည်ဆောက်မှုအင်္ဂါရပ်များက မပံ့ပိုးပါ။ | မှတ်ပုံတင်ခြင်းနှင့် ဖွဲ့စည်းမှုစနစ်တွင် ဖွင့်ထားသော မျဉ်းကွေးများအထိ အကောင့်တည်ဆောက်မှုကို ကန့်သတ်ပါ။ |
| `ERR_KEY_PAYLOAD_TOO_LONG` | သော့ပေးချေမှုပမာဏကို လက်မှတ်ထိုးခြင်းသည် ပံ့ပိုးထားသော ကန့်သတ်ချက်ထက် ကျော်လွန်နေပါသည်။ | Single-key controllers များကို `u8` အရှည်များ ကန့်သတ်ထားပါသည်။ အများသူငှာကီးများအတွက် multisig ကိုသုံးပါ (ဥပမာ၊ ML-DSA)။ |
| `ERR_INVALID_HEADER_VERSION` | လိပ်စာခေါင်းစီးဗားရှင်းသည် ပံ့ပိုးထားသော အပိုင်းအခြားပြင်ပတွင် ရှိနေသည်။ | V1 လိပ်စာများအတွက် ခေါင်းစီးဗားရှင်း `0` ကိုထုတ်ပါ။ ဗားရှင်းအသစ်များမလက်ခံမီ ကုဒ်နံပါတ်များကို အဆင့်မြှင့်ပါ။ |
| `ERR_INVALID_NORM_VERSION` | Normalization ဗားရှင်းအလံကို အသိအမှတ်မပြုပါ။ | ပုံမှန်ပြုလုပ်ခြင်း ဗားရှင်း `1` ကိုသုံး၍ သီးသန့်ဘစ်များကို ပြောင်းခြင်းမှ ရှောင်ကြဉ်ပါ။ |
| `ERR_INVALID_IH58_PREFIX` | တောင်းဆိုထားသော IH58 ကွန်ရက်ရှေ့ဆက်ကို ကုဒ်လုပ်၍မရပါ။ | ကွင်းဆက်မှတ်ပုံတင်ခြင်းတွင် ထုတ်ပြန်ထားသော ပါဝင်သော `0..=16383` အပိုင်းအတွင်း ရှေ့ဆက်တစ်ခုကို ရွေးပါ။ |
| `ERR_CANONICAL_HASH_FAILURE` | Canonical payload hashing မအောင်မြင်ပါ။ | လုပ်ဆောင်ချက်ကို ပြန်စမ်းကြည့်ပါ။ အမှားဆက်လက်ရှိနေပါက၊ ၎င်းကို hashing stack ရှိ အတွင်းပိုင်း bug တစ်ခုအဖြစ် သဘောထားပါ။ |

### ဖော်မတ် ကုဒ်ဆွဲခြင်းနှင့် အလိုအလျောက် ထောက်လှမ်းခြင်း။

| ကုတ် | ပျက်ကွက် | ပြန်လည်ပြုပြင်ခြင်း |
|--------|---------------------|--------------------------------|
| `ERR_INVALID_IH58_ENCODING` | IH58 စာတန်းတွင် အက္ခရာအပြင်ဘက်တွင် စာလုံးများပါရှိသည်။ | လိပ်စာသည် ထုတ်ဝေထားသော IH58 အက္ခရာကို အသုံးပြုထားပြီး ကော်ပီ/ကူးထည့်စဉ်အတွင်း ဖြတ်မထားကြောင်း သေချာပါစေ။ |
| `ERR_INVALID_LENGTH` | ပေးချေမှုအလျားသည် ရွေးချယ်သူ/ထိန်းချုပ်ကိရိယာအတွက် မျှော်လင့်ထားသည့် canonical အရွယ်အစားနှင့် မကိုက်ညီပါ။ | ရွေးချယ်ထားသော ဒိုမိန်းရွေးချယ်မှု နှင့် ထိန်းချုပ်ကိရိယာ အပြင်အဆင်အတွက် ကာနိုဝင်ဆံ့သည့် ဝန်ဆောင်ခ အပြည့်အစုံကို ပေးဆောင်ပါ။ |
| `ERR_CHECKSUM_MISMATCH` | IH58 (ဦးစားပေး) သို့မဟုတ် ချုံ့ထားသော (`sora`၊ ဒုတိယအကောင်းဆုံး) checksum validation မအောင်မြင်ပါ။ | ယုံကြည်ရသောအရင်းအမြစ်မှ လိပ်စာကို ပြန်ထုတ်ပါ။ ၎င်းသည် ပုံမှန်အားဖြင့် ကော်ပီ/ကူးထည့်ခြင်း အမှားကို ဖော်ပြသည်။ |
| `ERR_INVALID_IH58_PREFIX_ENCODING` | IH58 ရှေ့ဆက်ဘိုက်များသည် ပုံမမှန်ပါ။ | လိုက်လျောညီထွေရှိသော ကုဒ်နံပါတ်ဖြင့် လိပ်စာကို ပြန်လည်ကုဒ်လုပ်ပါ။ ဦးဆောင် Base58 bytes ကို ကိုယ်တိုင်မပြောင်းပါနှင့်။ |
| `ERR_INVALID_HEX_ADDRESS` | Canonical hexadecimal ပုံစံကို ကုဒ်ဖျက်၍မရပါ။ | `0x`-ရှေ့ဆက်ထားသော၊ ညီညီအလျားရှိသော hex စာကြောင်းကို တရားဝင်ကုဒ်ဒါဖြင့် ထုတ်လုပ်သည်။ |
| `ERR_MISSING_COMPRESSED_SENTINEL` | ချုံ့ထားသောပုံစံသည် `sora` ဖြင့် မစတင်ပါ။ | ၎င်းတို့ကို ကုဒ်ဒုဒ်ကိရိယာများထံ မလွှဲပြောင်းမီ လိုအပ်သော ဖန်သားပြင်ဖြင့် ဖိသိပ်ထားသော Sora လိပ်စာများကို ရှေ့ဆက်ပါ။ |
| `ERR_COMPRESSED_TOO_SHORT` | ချုံ့ထားသော စာကြောင်းသည် payload နှင့် checksum အတွက် လုံလောက်သော ဂဏန်းများ မရှိပါ။ | ဖြတ်တောက်ထားသော အတိုအထွာများအစား ကုဒ်ဒါမှ ထုတ်လွှတ်သော ချုံ့ထားသော စာကြောင်း အပြည့်အစုံကို အသုံးပြုပါ။ |
| `ERR_INVALID_COMPRESSED_CHAR` | ချုံ့ထားသော အက္ခရာ၏ အပြင်ဘက်တွင် အက္ခရာ ကြုံတွေ့ရသည်။ | ထုတ်ဝေထားသော အနံတစ်ဝက်/အနံအပြည့်ဇယားများမှ တရားဝင် Base-105 ဂရပ်ဖစ်ဖြင့် စာလုံးကို အစားထိုးပါ။ |
| `ERR_INVALID_COMPRESSED_BASE` | ကုဒ်ပြောင်းကိရိယာသည် ပံ့ပိုးမထားသော အခြမ်းတစ်ခုကို အသုံးပြုရန် ကြိုးစားခဲ့သည်။ | ကုဒ်ပြောင်းကိရိယာကို အမှားတစ်ခုတင်ပါ။ ချုံ့ထားသောအက္ခရာကို V1 တွင် radix 105 အဖြစ်သတ်မှတ်ထားသည်။ |
| `ERR_INVALID_COMPRESSED_DIGIT` | ဂဏန်းတန်ဖိုးသည် ချုံ့ထားသော အက္ခရာအရွယ်အစားထက် ကျော်လွန်နေပါသည်။ | ဂဏန်းတစ်ခုစီသည် `0..105)` အတွင်းဖြစ်ကြောင်း သေချာစေပြီး လိုအပ်ပါက လိပ်စာကို ပြန်လည်ထုတ်ပေးပါ။ |
| `ERR_UNSUPPORTED_ADDRESS_FORMAT` | အလိုအလျောက် ထောက်လှမ်းခြင်းသည် ထည့်သွင်းဖော်မတ်ကို မမှတ်မိနိုင်ပါ။ | IH58 (နှစ်သက်ရာ)၊ ချုံ့ထားသော (`sora`) သို့မဟုတ် ခွဲခြမ်းစိတ်ဖြာမှုများကို ခေါ်ဆိုသည့်အခါ canonical `0x` hex စာကြောင်းများကို ပံ့ပိုးပေးပါ။ |

### Domain နှင့် Network Validation| ကုတ် | ပျက်ကွက် | ပြန်လည်ပြုပြင်ခြင်း |
|--------|---------------------|--------------------------------|
| `ERR_DOMAIN_MISMATCH` | ဒိုမိန်းရွေးချယ်သူသည် မျှော်လင့်ထားသည့်ဒိုမိန်းနှင့် မကိုက်ညီပါ။ | ရည်ရွယ်ထားသော ဒိုမိန်းအတွက် ထုတ်ပေးသည့်လိပ်စာကို အသုံးပြုပါ သို့မဟုတ် မျှော်လင့်ချက်ကို အပ်ဒိတ်လုပ်ပါ။ |
| `ERR_INVALID_DOMAIN_LABEL` | ဒိုမိန်းတံဆိပ်ကို ပုံမှန်စစ်ဆေးခြင်း မအောင်မြင်ပါ။ | ကုဒ်မသွင်းမီ UTS-46 အကူးအပြောင်းမဟုတ်သော လုပ်ဆောင်မှုဖြင့် ဒိုမိန်းကို Canonicalise လုပ်ပါ။ |
| `ERR_UNEXPECTED_NETWORK_PREFIX` | ကုဒ်လုပ်ထားသော IH58 ကွန်ရက်ရှေ့ဆက်သည် စီစဉ်သတ်မှတ်ထားသော တန်ဖိုးနှင့် ကွဲပြားသည်။ | ပစ်မှတ်ကွင်းဆက်မှ လိပ်စာတစ်ခုသို့ ပြောင်းပါ သို့မဟုတ် မျှော်မှန်းထားသော ခွဲခြား/ရှေ့ဆက်ကို ချိန်ညှိပါ။ |
| `ERR_UNKNOWN_ADDRESS_CLASS` | လိပ်စာအတန်းအစားများကို အသိအမှတ်မပြုပါ။ | ဒီကုဒ်ဒါကို အတန်းသစ်ကို နားလည်နိုင်သော ထုတ်ဝေမှုတစ်ခုသို့ အဆင့်မြှင့်ပါ၊ သို့မဟုတ် ခေါင်းစီးဘစ်များကို ဆော့ကစားခြင်းမှ ရှောင်ကြဉ်ပါ။ |
| `ERR_UNKNOWN_DOMAIN_TAG` | Domain Selector Tag ကို မသိပါ။ | ရွေးချယ်မှုအမျိုးအစားအသစ်ကို ပံ့ပိုးပေးသည့် ထုတ်ဝေမှုတစ်ခုသို့ အပ်ဒိတ်လုပ်ပါ သို့မဟုတ် V1 node များတွင် စမ်းသပ်သည့် payloads များကို ရှောင်ပါ။ |
| `ERR_UNEXPECTED_EXTENSION_FLAG` | သီးသန့်တိုးချဲ့မှုဘစ်ကို သတ်မှတ်ပေးထားသည်။ | သီးသန့် bits များကို ရှင်းလင်းပါ။ ၎င်းတို့သည် အနာဂတ် ABI မှ မိတ်ဆက်သည့်အချိန်အထိ ၎င်းတို့အား တံခါးပိတ်ထားဆဲဖြစ်သည်။ |
| `ERR_UNKNOWN_CONTROLLER_TAG` | Controller payload tag ကို အသိအမှတ်မပြုပါ။ | ၎င်းတို့ကိုခွဲခြမ်းစိတ်ဖြာခြင်းမပြုမီ ထိန်းချုပ်သူအမျိုးအစားအသစ်များကို အသိအမှတ်ပြုရန် ဒီကုဒ်ဒါကို အဆင့်မြှင့်ပါ။ |
| `ERR_UNEXPECTED_TRAILING_BYTES` | ကုဒ်ဖြင့်ဖော်ပြပြီးနောက် နောက်လိုက်ဘိုက်များပါရှိသော Canonical payload | canonical payload ကို ပြန်ထုတ်ပါ။ မှတ်တမ်းတင်ထားသော အရှည်သာ ရှိသင့်သည်။ |

### Controller Payload Validation

| ကုတ် | ပျက်ကွက် | ပြန်လည်ပြုပြင်ခြင်း |
|--------|---------------------|--------------------------------|
| `ERR_INVALID_PUBLIC_KEY` | သော့ဘိုက်များသည် ကြေညာထားသောမျဉ်းကွေးနှင့် မကိုက်ညီပါ။ | ရွေးချယ်ထားသောမျဉ်းကွေးအတွက် သော့ဘိုက်များကို လိုအပ်သလို အတိအကျ ကုဒ်လုပ်ထားကြောင်း သေချာပါစေ။ (ဥပမာ၊ 32-byte Ed25519)။ |
| `ERR_UNKNOWN_CURVE` | Curve identifier ကို စာရင်းမသွင်းပါ။ | ထပ်ဆောင်းမျဉ်းကွေးများကို အတည်ပြုပြီး မှတ်ပုံတင်ခြင်းတွင် ထုတ်ပြန်ခြင်းမပြုမချင်း မျဉ်းကွေး ID `1` (Ed25519) ကို အသုံးပြုပါ။ |
| `ERR_MULTISIG_MEMBER_OVERFLOW` | Multisig controller သည် ပံ့ပိုးထားသည်ထက် အဖွဲ့ဝင်ပိုများကြောင်း ကြေညာသည်။ | ကုဒ်မသွင်းမီ မှတ်တမ်းတင်ထားသော ကန့်သတ်ချက်သို့ multisig အဖွဲ့ဝင်ခြင်းကို လျှော့ချပါ။ |
| `ERR_INVALID_MULTISIG_POLICY` | Multisig ပေါ်လစီ ပေးဆောင်မှု မှန်ကန်ကြောင်း အတည်ပြုခြင်း မအောင်မြင်ပါ (threshold/weights/schema)။ | CTAP2 schema၊ weight bounds နှင့် threshold ကန့်သတ်ချက်များကို ကျေနပ်စေရန် မူဝါဒကို ပြန်လည်တည်ဆောက်ပါ။ |

## အခြားရွေးချယ်စရာများကို ထည့်သွင်းစဉ်းစားပါ။

- **Pure Base58Check (Bitcoin စတိုင်)။** ရိုးရှင်းသော checksum ဖြစ်သော်လည်း အားနည်းသော အမှားရှာဖွေခြင်း
  Blake2b မှရရှိသော IH58 checksum ထက် (`encode_ih58` သည် 512-bit hash ကိုဖြတ်တောက်သည်)
  နှင့် 16-ဘစ် ခွဲခြားဆက်ဆံသူများအတွက် တိကျပြတ်သားသော ရှေ့ထွက် ဝေါဟာရများ မပါရှိပါ။
- **ဒိုမိန်းစာတန်းတွင် ကွင်းဆက်အမည်ကို ထည့်သွင်းခြင်း (ဥပမာ၊ `finance@chain`)။** ဖြတ်တောက်ခြင်း
- **လိပ်စာများမပြောင်းဘဲ Nexus လမ်းကြောင်းပေါ်တွင်သာ အားကိုးပါ။** အသုံးပြုသူများသည် ဆက်လက်တည်ရှိနေဦးမည်ဖြစ်သည်။
  မရေရာသော စာကြောင်းများကို မိတ္တူကူး/ကူးထည့်ပါ။ လိပ်စာကိုယ်တိုင်က အကြောင်းအရာကို သယ်ဆောင်ပေးစေချင်တယ်။
- **Bech32m စာအိတ်။** QR-ဖော်ရွေပြီး လူသားဖတ်နိုင်သော ရှေ့ဆက်ကို ပေးဆောင်သော်လည်း၊
  သင်္ဘော IH58 အကောင်အထည်ဖော်မှု (`AccountAddress::to_ih58`) နှင့် ကွဲလွဲနေမည်
  နှင့် ဆက်စပ်ပစ္စည်းများ/SDK အားလုံးကို ပြန်လည်ဖန်တီးရန် လိုအပ်သည်။ လက်ရှိ လမ်းပြမြေပုံသည် IH58+ ကို ထိန်းသိမ်းထားသည်။
  အနာဂတ်တွင် ဆက်လက်သုတေသနပြုနေစဉ် ချုံ့ထားသော (`sora`) ပံ့ပိုးမှု
  Bech32m/QR အလွှာများ (CAIP-10 မြေပုံဆွဲခြင်းကို ရွှေ့ဆိုင်းထားသည်)။

## မေးခွန်းများဖွင့်ပါ။

- `u16` ခွဲခြားမှုများနှင့် သီးသန့်ထားသော အပိုင်းအခြားများသည် ရေရှည်ဝယ်လိုအား အကျုံးဝင်ကြောင်း အတည်ပြုပါ။
  မဟုတ်ပါက `u32` ကို vaint ကုဒ်ဖြင့် အကဲဖြတ်ပါ။
- မှတ်ပုံတင်ခြင်းဆိုင်ရာ အပ်ဒိတ်များနှင့် မည်ကဲ့သို့ပြုလုပ်ရန်အတွက် လက်မှတ်ပေါင်းများစွာ အုပ်ချုပ်မှုလုပ်ငန်းစဉ်ကို အပြီးသတ်ပါ။
  ရုပ်သိမ်းခြင်း/ သက်တမ်းကုန် ခွဲဝေပေးခြင်းများကို ကိုင်တွယ်ဆောင်ရွက်ပါသည်။
- အတိအကျဖော်ပြသော လက်မှတ်အစီအစဉ်ကို သတ်မှတ်ပါ (ဥပမာ၊ Ed25519 multi-sig) နှင့်
  Nexus ဖြန့်ဖြူးမှုအတွက် သယ်ယူပို့ဆောင်ရေးလုံခြုံရေး (HTTPS ပင်ထိုးခြင်း၊ IPFS ဟက်ရ်ှဖော်မတ်)။
- ရွှေ့ပြောင်းခြင်းအတွက် ဒိုမိန်းအမည်တူများ/ပြန်ညွှန်းမှုများကို ပံ့ပိုးမပေးရန်နှင့် မည်သို့လုပ်ဆောင်ရမည်ကို ဆုံးဖြတ်ပါ။
  အဆုံးအဖြတ်ကို ချိုးဖောက်ခြင်းမရှိဘဲ ၎င်းတို့ကို ပေါ်လွင်စေရန်။
- Kotodama/IVM စာချုပ်များသည် IH58 အကူအညီများကို မည်သို့ဝင်ရောက်အသုံးပြုသည်ကို သတ်မှတ်ပါ (`to_address()`၊
  `parse_address()`) နှင့် ကွင်းဆက်သိုလှောင်မှုတွင် CAIP-10 ကို ဖော်ထုတ်သင့်သလား။
  မြေပုံများ (ယနေ့ IH58 သည် စံသတ်မှတ်ချက်များဖြစ်သည်)။
- ပြင်ပမှတ်ပုံတင်ခြင်းများတွင် Iroha ကွင်းဆက်များကို မှတ်ပုံတင်ခြင်း (ဥပမာ၊ IH58 မှတ်ပုံတင်ခြင်း၊
  ပိုမိုကျယ်ပြန့်သော ဂေဟစနစ် ချိန်ညှိမှုအတွက် CAIP အမည်နေရာ လမ်းညွှန်။

## နောက်အဆင့်များ

1. IH58 ကုဒ်နံပါတ်သည် `iroha_data_model` (`AccountAddress::to_ih58`၊
   `parse_encoded`); SDK တစ်ခုစီသို့ တပ်ဆင်မှုများ/စမ်းသပ်မှုများကို ဆက်လက်လုပ်ဆောင်ပြီး မည်သည့်အရာကိုမဆို ဖယ်ရှားလိုက်ပါ။
   Bech32m နေရာယူထားသည်။
2. `chain_discriminant` ဖြင့် ဖွဲ့စည်းမှုပုံစံ အစီအစဉ်ကို တိုးချဲ့ပြီး ဆင်ခြင်တုံတရားဖြင့် ရယူပါ
  ရှိပြီးသား စမ်းသပ်မှု/ဆော့ဖ်ဝဲပြင်ဆင်မှုများအတွက် ပုံသေများ။ **(ပြီးပါပြီ- `common.chain_discriminant`
  ယခုအခါ `iroha_config` ဖြင့် တင်ပို့နေပြီး `0x02F1` သို့ ပုံသေဖြင့် ကွန်ရက်တစ်ခုစီဖြင့်
  ထပ်ရေးသည်။)**
3. Nexus မှတ်ပုံတင်ခြင်းအစီအစဉ်နှင့် အယူအဆသက်သေပြသည့် ထုတ်ဝေသူအား ရေးဆွဲပါ။
4. human-factor ရှုထောင့်များနှင့်ပတ်သက်၍ ပိုက်ဆံအိတ်ဝန်ဆောင်မှုပေးသူများနှင့် အုပ်ထိန်းသူများထံမှ အကြံပြုချက်ကို စုဆောင်းပါ။
   (HRP အမည်၊ ဖော်မတ်ချခြင်း)။
5. Update documentation (`docs/source/data_model.md`, Torii API docs) ကို တစ်ကြိမ်၊
   အကောင်အထည်ဖော်ရေးလမ်းကြောင်းကို ကတိကဝတ်ပြုထားသည်။
6. တရားဝင်ကုဒ်ဒစ်စာကြည့်တိုက်များ (Rust/TS/Python/Kotlin) ကို စံချိန်စံညွှန်းစမ်းသပ်မှုဖြင့် ပို့ဆောင်ပါ။
   အောင်မြင်မှုနှင့် ကျရှုံးမှုများကို ဖုံးအုပ်ထားသော vector များ။
