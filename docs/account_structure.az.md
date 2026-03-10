---
lang: az
direction: ltr
source: docs/account_structure.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 01561366c9698c10d29ff3f49ad4c14b22b1796b5c7701cf98d200a140af1caf
source_last_modified: "2026-01-28T17:11:30.635172+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Hesab strukturu RFC

**Status:** Qəbul edildi (ADDR-1)  
**Auditoriya:** Data modeli, Torii, Nexus, Pulqabı, İdarəetmə qrupları  
**Əlaqədar məsələlər:** TBD

## Xülasə

Bu sənəddə həyata keçirilən göndərmə hesabı ünvanlama yığını təsvir olunur
`AccountAddress` (`crates/iroha_data_model/src/account/address.rs`) və
yoldaş alətlər. O, təmin edir:

- İstehsal edilmiş yoxlama cəmi, insana baxan **Iroha Base58 ünvanı (IH58)**
  Zəncir diskriminantını hesaba bağlayan `AccountAddress::to_ih58`
  nəzarətçi və deterministik qarşılıqlı əlaqəyə uyğun mətn formaları təklif edir.
- Gizli defolt domenlər və yerli həzmlər üçün domen seçiciləri, a ilə
  gələcək Nexus tərəfindən dəstəklənən marşrutlaşdırma üçün qorunan qlobal reyestr seçici etiketi (
  reyestr axtarışı **hələ göndərilməyib**).

## Motivasiya

Pul kisələri və zəncirdənkənar alətlər bu gün xam `alias@domain` (rejected legacy form) marşrutlaşdırma ləqəblərinə əsaslanır. Bu
iki əsas çatışmazlıq var:

1. **Şəbəkə bağlaması yoxdur.** Sətirdə yoxlama məbləği və ya zəncir prefiksi yoxdur, ona görə də istifadəçilər
   dərhal rəy bildirmədən səhv şəbəkədən ünvanı yapışdıra bilər. The
   əməliyyat nəhayət rədd ediləcək (zəncir uyğunsuzluğu) və ya daha pisi, uğur qazanacaq
   təyinat yerli olaraq mövcuddursa, nəzərdə tutulmayan hesaba qarşı.
2. **Domen toqquşması.** Domenlər yalnız ad məkanıdır və hər birində təkrar istifadə edilə bilər
   zəncir. Xidmətlər Federasiyası (mühafizəçilər, körpülər, zəncirlərarası iş axınları)
   A zəncirindəki `finance`, `finance` ilə əlaqəsi olmadığı üçün kövrək olur.
   zəncir B.

Kopyalama/yapışdırma xətalarından qoruyan insanlara uyğun ünvan formatına ehtiyacımız var
və domen adından nüfuzlu zəncirə qədər deterministik xəritəçəkmə.

## Məqsədlər

- Məlumat modelində tətbiq olunan IH58 Base58 zərfini və
  `AccountId` və `AccountAddress`-in əməl etdiyi kanonik təhlil/ləqəb qaydaları.
- Konfiqurasiya edilmiş zəncir diskriminantını birbaşa hər bir ünvana kodlayın və
  onun idarə edilməsi/reyestr prosesini müəyyən edir.
- Qlobal domen reyestrini cərəyanı pozmadan necə təqdim edəcəyinizi təsvir edin
  yerləşdirmələr və normallaşdırma/anti-spoofing qaydalarını müəyyənləşdirin.

## Qeyri-məqsəd

- Zəncirlərarası aktiv köçürmələrinin həyata keçirilməsi. Marşrutlaşdırma təbəqəsi yalnız onu qaytarır
  hədəf zənciri.
- Qlobal domen buraxılışı üçün idarəetmənin yekunlaşdırılması. Bu RFC məlumatlara diqqət yetirir
  model və nəqliyyat primitivləri.

## Fon

### Cari marşrutlaşdırma ləqəbi

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

`ChainId` `AccountId` xaricində yaşayır. Düyünlər əməliyyatın `ChainId` nömrəsini yoxlayır
qəbul zamanı konfiqurasiyaya qarşı (`AcceptTransactionFail::ChainIdMismatch`)
və xarici əməliyyatları rədd edin, lakin hesab sətirinin özü yoxdur
şəbəkə işarəsi.

### Domen identifikatorları

`DomainId` `Name` (normallaşdırılmış sətir) əhatə edir və yerli zəncirə əhatə olunub.
Hər bir zəncir müstəqil olaraq `wonderland`, `finance` və s. qeydiyyatdan keçə bilər.

### Nexus kontekst

Nexus komponentlər arası koordinasiyaya (zolaqlar/məlumat məkanları) cavabdehdir. Bu
hal-hazırda çarpaz zəncirli domen marşrutlaşdırma anlayışı yoxdur.

## Təklif olunan Dizayn

### 1. Deterministik zəncirvari diskriminant

`iroha_config::parameters::actual::Common` indi ifşa edir:

```rust
pub struct Common {
    pub chain: ChainId,
    pub chain_discriminant: u16, // globally coordinated
    // ... existing fields
}
```

- ** Məhdudiyyətlər:**
  - Hər aktiv şəbəkə üçün unikal; ilə imzalanmış ictimai reyestr vasitəsilə idarə olunur
    açıq qorunan diapazonlar (məsələn, `0x0000–0x0FFF` test/dev, `0x1000–0x7FFF`
    icma ayırmaları, `0x8000–0xFFEF` idarəetmə tərəfindən təsdiqlənmiş, `0xFFF0–0xFFFF`
    qorunur).
  - Çalışan zəncir üçün dəyişməz. Onu dəyişdirmək üçün sərt çəngəl və a
    reyestr yeniləməsi.
- **İdarəetmə və reyestr (planlaşdırılmış):** Çox imzalı idarəetmə dəsti
  imzalanmış JSON reyestrini insan ləqəbləri ilə müqayisə edən diskriminantları saxlamaq və
  CAIP-2 identifikatorları. Bu reyestr hələ göndərilmiş iş vaxtının bir hissəsi deyil.
- **İstifadə:** Dövlət qəbulu, Torii, SDK və pul kisəsi API-ləri vasitəsilə ötürülür
  hər bir komponent onu daxil edə və ya təsdiqləyə bilər. CAIP-2 məruz qalması gələcək olaraq qalır
  qarşılıqlı əlaqə tapşırığı.

### 2. Kanonik ünvan kodekləri

Rust data modeli tək kanonik faydalı yük təqdimatını nümayiş etdirir
(`AccountAddress`) bir neçə insana baxan format kimi yayıla bilər. IH58
paylaşma və kanonik çıxış üçün üstünlük verilən hesab formatı; sıxılmış
`sora` forması kana əlifbasının olduğu UX üçün ikinci ən yaxşı, yalnız Sora variantıdır.
dəyər əlavə edir. Canonical hex sazlama yardımı olaraq qalır.

- **IH58 (Iroha Base58)** – zənciri daxil edən Base58 zərfi
  diskriminant. Dekoderlər faydalı yükü təşviq etməzdən əvvəl prefiksi təsdiqləyirlər
  kanonik forma.
- **Sora ilə sıxılmış görünüş** – **105 simvoldan** ibarət yalnız Sora əlifbası
  yarım enli イロハ şeirinin (ヰ və ヱ daxil olmaqla) 58 simvola əlavə edilməsi
  IH58 dəsti. Simlər gözətçi `sora` ilə başlayır, Bech32m-dən əldə ediləni daxil edir
  checksum və şəbəkə prefiksini buraxın (Sora Nexus gözətçi tərəfindən nəzərdə tutulur).

  ```
  IH58  : 123456789ABCDEFGHJKMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz
  Iroha : ｲﾛﾊﾆﾎﾍﾄﾁﾘﾇﾙｦﾜｶﾖﾀﾚｿﾂﾈﾅﾗﾑｳヰﾉｵｸﾔﾏｹﾌｺｴﾃｱｻｷﾕﾒﾐｼヱﾋﾓｾｽ
  ```
- **Canonical hex** – kanonik baytın sazlama üçün əlverişli `0x…` kodlaması
  zərf.

`AccountAddress::parse_encoded` IH58 (üstünlük verilir), sıxılmış (`sora`, ikinci ən yaxşı) və ya kanonik hex-i avtomatik aşkarlayır
(yalnız `0x...`; çılpaq hex rədd edilir) həm deşifrə edilmiş faydalı yükü, həm də aşkarlanmış yükü daxil edir və qaytarır
`AccountAddressFormat`. Torii indi ISO 20022 əlavəsi üçün `parse_encoded` çağırır
kanonik hex formasını ünvanlayır və saxlayır ki, metadata deterministik olaraq qalır
orijinal təmsilindən asılı olmayaraq.

#### 2.1 Başlıq bayt tərtibatı (ADDR-1a)

Hər bir kanonik faydalı yük `header · controller` kimi tərtib edilmişdir. The
`header` tək baytdır ki, bu baytlara hansı analiz qaydalarının tətbiq olunduğunu bildirir.
izləyin:

```
bit index:   7        5 4      3 2      1 0
             ┌─────────┬────────┬────────┬────┐
payload bit: │version  │ class  │  norm  │ext │
             └─────────┴────────┴────────┴────┘
```

Beləliklə, birinci bayt aşağı axın dekoderləri üçün sxem metadatasını toplayır:

| Bit | Sahə | İcazə verilən dəyərlər | Pozulma zamanı xəta |
|------|-------|----------------|--------------------|
| 7-5 | `addr_version` | `0` (v1). `1-7` dəyərləri gələcək düzəlişlər üçün qorunur. | `0-7` triggerindən kənar dəyərlər `AccountAddressError::InvalidHeaderVersion`; tətbiqlər Sıfırdan fərqli versiyaları bu gün dəstəklənməyən kimi qəbul etməlidir. |
| 4-3 | `addr_class` | `0` = tək açar, `1` = multisig. | Digər dəyərlər `AccountAddressError::UnknownAddressClass` artırır. |
| 2-1 | `norm_version` | `1` (Norm v1). `0`, `2`, `3` dəyərləri qorunur. | `0-3` xaricində olan dəyərlər `AccountAddressError::InvalidNormVersion` artırır. |
| 0 | `ext_flag` | `0` olmalıdır. | Set biti `AccountAddressError::UnexpectedExtensionFlag` artırır. |

Rust kodlayıcısı tək düyməli kontrollerlər üçün `0x02` yazır (versiya 0, sinif 0,
norma v1, genişləndirmə bayrağı silindi) və multisig nəzarətçiləri üçün `0x0A` (versiya 0,
sinif 1, norma v1, genişləndirmə bayrağı təmizləndi).

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

#### 2.3 Nəzarətçinin faydalı yük kodlaşdırmaları (ADDR-1a)

Nəzarətçinin faydalı yükü kanonik yükdə başlıqdan dərhal sonra (legacy selector segment dekodlaşdırılarkən ondan sonra) gələn başqa bir işarələnmiş birləşmədir.

| Tag | Nəzarətçi | Layout | Qeydlər |
|-----|------------|--------|-------|
| `0x00` | Tək açar | `curve_id:u8` · `key_len:u8` · `key_bytes` | `curve_id=0x01` bu gün Ed25519 ilə xəritələr. `key_len` `u8` ilə məhdudlaşır; daha böyük dəyərlər `AccountAddressError::KeyPayloadTooLong`-i artırır (buna görə də >255 bayt olan tək açarlı ML‑DSA açıq açarları kodlaşdırıla bilməz və multisig istifadə etməlidir). |
| `0x01` | Multisig | `version:u8` · `threshold:u16` · `member_count:u8` · (`curve_id:u8` · `weight:u16` · `key_len:u16` · `key_len:u16`)\* | 255-ə qədər üzvü dəstəkləyir (`CONTROLLER_MULTISIG_MEMBER_MAX`). Naməlum əyrilər `AccountAddressError::UnknownCurve` artırır; səhv formalaşmış siyasətlər `AccountAddressError::InvalidMultisigPolicy` kimi qabarır. |

Multisig siyasətləri həmçinin CTAP2 tipli CBOR xəritəsini və kanonik həzm sistemini ifşa edir
hostlar və SDK-lar nəzarətçini deterministik şəkildə yoxlaya bilər. Bax
Sxem üçün `docs/source/references/multisig_policy_schema.md` (ADDR-1c),
doğrulama qaydaları, hashing proseduru və qızıl qurğular.

Bütün əsas baytlar `PublicKey::to_bytes` tərəfindən qaytarıldığı kimi kodlanır; dekoderlər `PublicKey` instansiyalarını yenidən qurur və baytlar elan edilmiş əyri ilə uyğun gəlmirsə, `AccountAddressError::InvalidPublicKey`-i qaldırır.

> **Ed25519 kanonik icrası (ADDR-3a):** əyri `0x01` düymələri imzalayanın buraxdığı dəqiq bayt sətirinə deşifrə etməli və kiçik sifarişli alt qrupda yer almamalıdır. Qovşaqlar indi qeyri-kanonik kodlaşdırmaları (məsələn, `2^255-19` modulu azaldılmış dəyərlər) və identifikasiya elementi kimi zəif nöqtələri rədd edir, beləliklə, SDK-lar ünvanları təqdim etməzdən əvvəl uyğunluq yoxlama səhvlərini üzə çıxarmalıdır.

##### 2.3.1 Əyri identifikator reyestri (ADDR-1d)

| ID (`curve_id`) | Alqoritm | Xüsusiyyət qapısı | Qeydlər |
|----------------|-----------|--------------|-------|
| `0x00` | Qorunur | — | Emissiya edilməməlidir; dekoderlərin səthi `ERR_UNKNOWN_CURVE`. |
| `0x01` | Ed25519 | — | Canonical v1 alqoritmi (`Algorithm::Ed25519`); standart konfiqurasiyada aktivləşdirilib. |
| `0x02` | ML‑DSA (Dilithium3) | — | Dilithium3 açıq açar baytlarından (1952 bayt) istifadə edir. `key_len` `u8` olduğu üçün tək açarlı ünvanlar ML-DSA-nı kodlaya bilməz; multisig `u16` uzunluqlarından istifadə edir. |
| `0x03` | BLS12‑381 (normal) | `bls` | G1-də açıq açarlar (48 bayt), G2-də imzalar (96 bayt). |
| `0x04` | secp256k1 | — | SHA‑256 üzərində deterministik ECDSA; açıq açarlar 33 baytlıq SEC1 sıxılmış formadan, imzalar isə kanonik 64 baytlıq `r∥s` düzümündən istifadə edir. |
| `0x05` | BLS12‑381 (kiçik) | `bls` | G2-də açıq açarlar (96 bayt), G1-də imzalar (48 bayt). |
| `0x0A` | GOST R 34.10-2012 (256, A dəsti) | `gost` | Yalnız `gost` funksiyası aktiv olduqda mövcuddur. |
| `0x0B` | GOST R 34.10-2012 (256, B dəsti) | `gost` | Yalnız `gost` funksiyası aktiv olduqda mövcuddur. |
| `0x0C` | GOST R 34.10-2012 (256, dəst C) | `gost` | Yalnız `gost` funksiyası aktiv olduqda mövcuddur. |
| `0x0D` | GOST R 34.10-2012 (512, A dəsti) | `gost` | Yalnız `gost` funksiyası aktiv olduqda mövcuddur. |
| `0x0E` | GOST R 34.10-2012 (512, dəst B) | `gost` | Yalnız `gost` funksiyası aktiv olduqda mövcuddur. |
| `0x0F` | SM2 | `sm` | DistID uzunluğu (u16 BE) + DistID baytları + 65 bayt SEC1 sıxılmamış SM2 açarı; yalnız `sm` aktiv olduqda mövcuddur. |

`0x06–0x09` yuvaları əlavə əyrilər üçün təyin olunmamış qalır; yenisini təqdim edir
alqoritm yol xəritəsi yeniləməsini və uyğun SDK/host əhatə dairəsini tələb edir. Kodlayıcılar
`ERR_UNSUPPORTED_ALGORITHM` ilə dəstəklənməyən hər hansı bir alqoritmi rədd etməli və
dekoderlər qorunmaq üçün `ERR_UNKNOWN_CURVE` ilə naməlum identifikatorlarda tez sıradan çıxmalıdırlar.
uğursuz qapalı davranış.

Kanonik reyestr (o cümlədən maşın tərəfindən oxuna bilən JSON ixracı) altında yaşayır
[`docs/source/references/address_curve_registry.md`](source/references/address_curve_registry.md).
Alətlər həmin verilənlər dəstini birbaşa istehlak etməlidir ki, əyri identifikatorlar qalsın
SDK-lar və operator iş axınları arasında ardıcıl.

- **SDK keçidi:** SDK-lar defolt olaraq Ed25519 üçün yalnız doğrulama/kodlaşdırmaya uyğundur. Swift ifşa edir
  tərtib vaxtı bayraqları (`IROHASWIFT_ENABLE_MLDSA`, `IROHASWIFT_ENABLE_GOST`,
  `IROHASWIFT_ENABLE_SM`); Java/Android SDK tələb edir
  `AccountAddress.configureCurveSupport(...)`; JavaScript SDK istifadə edir
  `configureCurveSupport({ allowMlDsa: true, allowGost: true, allowSm2: true })`.
  secp256k1 dəstəyi mövcuddur, lakin JS/Android-də defolt olaraq aktiv deyil
  SDK-lar; Zəng edənlər Ed25519 olmayan nəzarətçiləri emissiya edərkən açıq şəkildə daxil olmalıdırlar.
- **Host qapısı:** `Register<Account>` imzalayanları alqoritmlərdən istifadə edən nəzarətçiləri rədd edir
  node `crypto.allowed_signing` siyahısında yoxdur **və ya** əyri identifikatorları yoxdur
  `crypto.curves.allowed_curve_ids`, buna görə də klasterlər dəstəyi reklam etməlidir (konfiqurasiya +
  genesis) ML‑DSA/GOST/SM nəzarətçiləri qeydiyyata alınmazdan əvvəl. BLS nəzarətçi
  tərtib edərkən alqoritmlərə həmişə icazə verilir (konsensus açarları onlara əsaslanır),
  və standart konfiqurasiya Ed25519 + secp256k1-i aktivləşdirir.【crates/iroha_core/src/smartcontracts/isi/domain.rs:32】

##### 2.3.2 Multisig nəzarətçi təlimatı

`AccountController::Multisig` vasitəsilə siyasətləri seriallaşdırır
`crates/iroha_data_model/src/account/controller.rs` və sxemi tətbiq edir
[`docs/source/references/multisig_policy_schema.md`](source/references/multisig_policy_schema.md) ilə sənədləşdirilmişdir.
Əsas icra təfərrüatları:

- Siyasətlər əvvəllər `MultisigPolicy::validate()` tərəfindən normallaşdırılır və təsdiqlənir
  daxil edilir. Eşiklər ≥1 və ≤Σ çəki olmalıdır; dublikat üzvlərdir
  `(algorithm || 0x00 || key_bytes)` ilə çeşidləndikdən sonra deterministik şəkildə silindi.
- İkili nəzarətçinin faydalı yükü (`ControllerPayload::Multisig`) kodlayır
  `version:u8`, `threshold:u16`, `member_count:u8`, sonra hər bir üzvün
  `(curve_id, weight:u16, key_len:u16, key_bytes)`. Bu məhz budur
  `AccountAddress::canonical_bytes()` IH58 (üstünlük verilir)/sora (ikinci ən yaxşı) faydalı yüklərə yazır.
- Hashing (`MultisigPolicy::digest_blake2b256()`) Blake2b-256 ilə birlikdə
  `iroha-ms-policy` fərdiləşdirmə sətri beləliklə idarəetmə manifestləri
  IH58-ə daxil edilmiş nəzarətçi baytlarına uyğun gələn deterministik siyasət ID-si.
- Armatur əhatə dairəsi `fixtures/account/address_vectors.json`-də yaşayır (hallar
  `addr-multisig-*`). Pul kisələri və SDK-lar kanonik IH58 sətirlərini təsdiq etməlidir
  onların kodlayıcılarının Rust tətbiqinə uyğun olduğunu təsdiqləmək üçün aşağıda.

| Case ID | Eşik / üzvlər | IH58 hərfi (prefiks `0x02F1`) | Sora sıxılmış (`sora`) hərfi | Qeydlər |
|---------|--------------------|--------------------------------|-------------------------|-------|
| `addr-multisig-council-threshold3` | `≥3` çəki, üzvlər `(2,1,1)` | `SRfSHsrH3tEmYaaAYyD248F3vfT1oQ3WEGS22MaD8W9bLefF7rsoKLYGcpbcM9EcSus5ZhCAZU7ztn2BCsyeCAdfRncAVmVsipd4ibk6CBLF3Nrzcw8P7VKJg6mtFgEhWVTjfDkUMoc63oeEmaWyV6cyiphwk8ZgKAJUe4TyVtmKm1WWcg7qZ6i` | `sora3vﾑ2zkaoUwﾋﾅGﾘﾚyﾂe3ﾖfﾙヰｶﾘﾉwｷnoWﾛYicaUr3ﾔｲﾖ2Ado3TﾘYQﾉJqﾜﾇｳﾑﾐd8dDjRGｦ3Vﾃ9HcﾀMヰR8ﾎﾖgEqGｵEｾDyc5ﾁ1ﾔﾉ31sUﾑﾀﾖaｸxﾘ3ｲｷMEuFｺｿﾉBQSVQnxﾈeJzrXLヰhｿｹ5SEEﾅPﾂﾗｸdヰﾋ1bUGHｲVXBWNNJ6K` | Şura-domen idarəetmə kvorumu. |
| `addr-multisig-wonderland-threshold2` | `≥2`, üzvlər `(1,2)` | `3xsmkps1KPBn9dtpE5qHRhHEZCpiAe8d9j6H9A42TV6kc1TpaqdwnSksKgQrsSEHznqvWKBMc1os69BELzkLjsR7EV2gjV14d9JMzo97KEmYoKtxCrFeKFAcy7ffQdboV1uRt` | `sora2ﾖZﾘeｴAdx3ﾂﾉﾔXhnｹﾀ2ﾉｱﾋxﾅﾄﾌヱwﾐmﾊvEﾐCﾏﾎｦ1ﾑHﾋso2GKﾔﾕﾁwﾂﾃP6ﾁｼﾙﾖｺ9ｻｦbﾈ4wFdﾑFヰ3HaﾘｼMｷﾌHWtｷﾋLﾙﾖQ4D3XﾊﾜXmpktﾚｻ5ﾅﾅﾇ1gkﾏsCFQGH9` | İki imzalı möcüzələr ölkəsi nümunəsi (çəki 1 + 2). |
| `addr-multisig-default-quorum3` | `≥3`, üzvlər `(1,1,1,1)` | `nA2bDNhMqXz7ERkHNoEWbvJGyR1aDRsw32LaUWLgbK3vcpzohmdFCLvdotxUWWDY3aZeX4ptLk4Z6TjF5ossnJm8VrNo6daxmGTkqUyP4MxJxiNyPFxsEE5DLnsoLWUcxaWNpZ76tmkbiGS31Gv8tejKpuiHUMaQ1s5ohWyZvDnpycNkBK8AEfGJqn5yc9zAzfWbVhpDwkPj8ScnzvH1Echr5` | `soraﾐ38ﾅｴｸﾜ8ﾃzwBrqﾘｺ4yﾄv6kqJp1ｳｱﾛｿrzﾄﾃﾘﾒRﾗtV9ｼﾔPｽcヱEﾌVVVｼﾘｲZAｦﾓﾅｦeﾒN76vﾈcuｶuﾛL54rzﾙﾏX2zMﾌRLﾃﾋpﾚpｲcHﾑﾅﾃﾔzｵｲVfAﾃﾚﾎﾚCヰﾔｲｽｦw9ﾔﾕ8bGGkﾁ6sNｼaｻRﾖﾜYﾕﾚU18ﾅHヰﾌuMeﾊtﾂrｿj95Ft8ﾜ3fﾄkNiｴuﾈrCﾐQt8ヱｸｸmﾙﾒgUbﾑEKTTCM` | Əsas idarəetmə üçün istifadə edilən gizli-defolt domen kvorumu.

#### 2.4 Uğursuzluq qaydaları (ADDR-1a)

- Tələb olunan başlıq + seçicidən daha qısa və ya artıq baytları olan faydalı yüklər `AccountAddressError::InvalidLength` və ya `AccountAddressError::UnexpectedTrailingBytes` yayır.
- Qorunan `ext_flag`-i təyin edən və ya dəstəklənməyən versiyaları/sinifləri reklam edən başlıqlar `UnexpectedExtensionFlag`, `InvalidHeaderVersion` və ya `UnknownAddressClass` istifadə edərək rədd edilməlidir.
- Naməlum seçici/nəzarətçi teqləri `UnknownDomainTag` və ya `UnknownControllerTag` artırır.
- Böyük ölçülü və ya qüsurlu əsas material `KeyPayloadTooLong` və ya `InvalidPublicKey`-i qaldırır.
- 255 üzvdən çox olan Multisig nəzarətçiləri `MultisigMemberOverflow`-i qaldırır.
- IME/NFKC çevrilmələri: yarım eni Sora kana kodlaşdırmanı pozmadan tam enli formalarına normallaşdırıla bilər, lakin ASCII `sora` sentinel və IH58 rəqəmləri/hərfləri ASCII olaraq qalmalıdır. Tam enli və ya qutu qatlanmış gözətçilər səthi `ERR_MISSING_COMPRESSED_SENTINEL`, tam enli ASCII faydalı yükləri `ERR_INVALID_COMPRESSED_CHAR` artırır və yoxlama məbləği uyğunsuzluqları `ERR_CHECKSUM_MISMATCH` kimi qabarır. `crates/iroha_data_model/src/account/address.rs`-də mülkiyyət testləri bu yolları əhatə edir ki, SDK-lar və pul kisələri deterministik uğursuzluqlara arxalana bilsin.
- Torii və `address@domain` (rejected legacy form) ləqəblərinin SDK təhlili indi IH58 (üstünlük verilir)/sora (ikinci-ən yaxşı) daxiletmələr ləqəbdən əvvəl uğursuz olduqda (məsələn, domen strukturunda səhv səhvlər ola bilməyəndə) eyni `ERR_*` kodlarını buraxır. nəsr sətirlərindən təxmin etmək.
- `ERR_LOCAL8_DEPRECATED` səthi 12 baytdan qısa olan yerli selektorun faydalı yükləri köhnə Local‑8 həzmlərindən sərt kəsimi qoruyur.
- Domainless canonical IH58 literals decode directly to a domainless `AccountId`. Use `ScopedAccountId` only when an interface requires explicit domain context.

#### 2.5 Normativ ikili vektorlar

- **Düzgün defolt domen (`default`, əsas bayt `0x00`)**  
  Kanonik hex: `0x020001203b6a27bcceb6a42d62a3a8d02a6f0d73653215771de243a63ac048a18b59da29`.  
  Parçalanma: `0x02` başlığı, `0x00` selektoru (örtülü defolt), `0x00` nəzarətçi teqi, `0x01` əyri identifikatoru (Ed25519), I18NI0000027X açarı yükləmək, 3-byte açarı yükləmək.
- **Yerli domen həzmi (`treasury`, toxum baytı `0x01`)**  
  Kanonik hex: `0x0201b18fe9c1abbac45b3e38fc5d0001208a88e3dd7409f195fd52db2d3cba5d72ca6709bf1d94121bf3748801b40f6f5c`.  
  Parçalanma: `0x02` başlığı, selektor teqi `0x01` üstəgəl həzm `b1 8f e9 c1 ab ba c4 5b 3e 38 fc 5d`, ardınca tək düyməli faydalı yük (`0x00` teqi, `0x01` əyri id, uzunluğu 002, I18 byte) Ed25519 açarı).Vahid testləri (`account::address::tests::parse_encoded_accepts_all_formats`) aşağıdakı V1 vektorlarını `AccountAddress::parse_encoded` vasitəsilə təsdiqləyir və alətlərin hex, IH58 (üstünlük verilir) və sıxılmış (`sora`, ikinci ən yaxşı) formalar üzrə kanonik faydalı yükə etibar edə biləcəyinə zəmanət verir. Genişləndirilmiş armatur dəstini `cargo run -p iroha_data_model --example address_vectors` ilə bərpa edin.

| Domain | Toxum baytı | kanonik hex | Sıxılmış (`sora`) |
|-------------|-----------|-----------------------------------------------------------------------------------------|------------|
| default | `0x00` | `0x020001203b6a27bcceb6a42d62a3a8d02a6f0d73653215771de243a63ac048a18b59da29` | `sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE` |
| xəzinə | `0x01` | `0x0201b18fe9c1abbac45b3e38fc5d0001208a88e3dd7409f195fd52db2d3cba5d72ca6709bf1d94121bf3748801b40f6f5c` | `sora5ｻu6rﾀCヰTGwﾏ1ﾅヱﾌQｲﾖﾇqCｦヰﾓZQCZRDSSﾅMｱﾙヱｹﾁｸ8ｾeﾄﾛ6C8bZuwﾗｹCZｦRSLQFU` |
| möcüzələr diyarı | `0x02` | `0x0201b8ae571b79c5a80f5834da2b0001208139770ea87d175f56a35466c34c7ecccb8d8a91b4ee37a25df60f5b8fc9b394` | `sora5ｻwﾓyRｿqﾏnMﾀﾙヰKoﾒﾇﾓQｺﾛyｼ3ｸFHB2F5LyPﾐTMZkｹｼw67ﾋVﾕｻr8ﾉGﾇeEnｻVRNKCS` |
| iroha | `0x03` | `0x0201de8b36819700c807083608e2000120ed4928c628d1c2c6eae90338905995612959273a5c63f93636c14614ac8737d1` | `sora5ｻﾜxﾀ7Vｱ7QFeｷMﾂLﾉﾃﾏﾓﾀTﾚgSav3Wnｱｵ4ｱCKｷﾛMﾘzヰHiﾐｱ6ﾃﾉﾁﾐZmﾇ2fiﾎX21P4L` |
| alfa | `0x04` | `0x020146be2154ae86826a3fef0ec0000120ca93ac1705187071d67b83c7ff0efe8108e8ec4530575d7726879333dbdabe7c` | `sora5ｻ9JヱﾈｿuwU6ｴpﾔﾂﾈRqRTds1HﾃﾐｶLVﾍｳ9ﾔhｾNｵVｷyucEﾒGﾈﾏﾍ9sKeﾉDzrｷﾆ742WG1` |
| omega | `0x05` | `0x0201390d946885bc8416b3d30c9d0001206e7a1cdd29b0b78fd13af4c5598feff4ef2a97166e3ca6f2e4fbfccd80505bf1` | `sora5ｻ3zrﾌuﾚﾄJﾑXQhｸTyN8pzwRkWxmjVﾗbﾚﾕヰﾈoｽｦｶtEEﾊﾐ6GPｿﾓﾊｾEhvPｾｻ3XAJ73F` |
| idarəetmə | `0x06` | `0x0201989eb45a80940d187e2c908f0001208a875fff1eb38451577acd5afee405456568dd7c89e090863a0557bc7af49f17` | `sora5ｻiｵﾁyVﾕｽbFpDHHuﾇﾉdﾗｲﾓﾄRﾋAW3frUCｾ5ｷﾘTwdﾚnｽtQiLﾏｼｶﾅXgｾZmﾒヱH58H4KP` |
| doğrulayıcılar | `0x07` | `0x0201e4ffa58704c69afaeb7cc2d7000120ea4a6c63e29c520abef5507b132ec5f9954776aebebe7b92421eea691446d22c` | `sora5ｻﾀLDH6VYﾑNAｾgﾉVﾜtxﾊRXLｹﾍﾔﾌLd93GﾔGeｴﾄYrs1ﾂHｸkYxｹwｿyZﾗxyﾎZoXT1S4N` |
| kəşfiyyatçı | `0x08` | `0x02013b35422c65c2a83c99c523ad0001201398f62c6d1a457c51ba6a4b5f3dbd2f69fca93216218dc8997e416bd17d93ca` | `sora5ｻ4nmｻaﾚﾚPvNLgｿｱv6MHDeEyﾀovﾉJcpvrﾖ6ﾈCQcCNﾇﾜhﾚﾖyFdTwｸｶHEｱ9rWU8FMB` |
| soranet | `0x09` | `0x0201047d9ea7f5d5dbec3f7bfc58000120fd1724385aa0c75b64fb78cd602fa1d991fdebf76b13c58ed702eac835e9f618` | `sora5ｱｸヱVQﾂcﾁヱRﾓcApｲﾁﾅﾒvﾌﾏfｾNnﾛRJsｿDhﾙuHaﾚｺｦﾌﾍﾈeﾆﾎｺN1UUDｶ6ﾎﾄﾛoRH8JUL` |
| kitsune | `0x0A` | `0x0201e91933de397fd7723dc9a76c00012043a72e714401762df66b68c26dfbdf2682aaec9f2474eca4613e424a0fbafd3c` | `sora5ｻﾚｺヱkfFJfSﾁｼJwﾉLvbpSｷﾔMWFMrbｳｸｲｲyヰKGJﾉｻ4ｹﾕrｽhｺｽzSDヰXAN62AD7RGNS` |
| da | `0x0B` | `0x02016838cf5bb0ce0f3d4f380e1c00012066be7e332c7a453332bd9d0a7f7db055f5c5ef1a06ada66d98b39fb6810c473a` | `sora5ｻNﾒ5SﾐRﾉﾐﾃ62ｿ1ｶｷWFKyF1BcAﾔvｼﾐHqﾙﾐPﾏｴヰ5tｲﾕvnﾙT6ﾀW7mﾔ7ﾇﾗﾂｳ25CXS93` |

Nəzərdən keçirən: Data Model WG, Cryptography WG — əhatə dairəsi ADDR-1a üçün təsdiq edilmişdir.

##### Sora Nexus istinad ləqəbləri

Sora Nexus şəbəkələri standart olaraq `chain_discriminant = 0x02F1`-dir
(`iroha_config::parameters::defaults::common::CHAIN_DISCRIMINANT`). The
`AccountAddress::to_ih58` və `to_compressed_sora` köməkçiləri buna görə də yayırlar
hər kanonik yük üçün ardıcıl mətn formaları. Seçilmiş qurğular
`fixtures/account/address_vectors.json` (vasitəsilə yaradılıb
`cargo xtask address-vectors`) tez istinad üçün aşağıda göstərilmişdir:

| Hesab / seçici | IH58 hərfi (prefiks `0x02F1`) | Sora sıxılmış (`sora`) hərfi |
|--------------------------------|--------------------------------|-------------------------|
| `default` domeni (örtülü seçici, toxum `0x00`) | `6cmzPVPX5jDQFNfiz6KgmVfm1fhoAqjPhoPFn4nx9mBWaFMyUCwq4cw` | `sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE` (açıq marşrut göstərişləri təmin edərkən isteğe bağlı `@default` şəkilçisi) |
| `treasury` (yerli həzm seçicisi, toxum `0x01`) | `34mSYnCXkCzHXm31UDHh7SJfGvC4QPEhwim8z7sys2iHqXpCwCQkjL8KHvkFLSs1vZdJcb37r` | `sora5ｻu6rﾀCヰTGwﾏ1ﾅヱﾌQｲﾖﾇqCｦヰﾓZQCZRDSSﾅMｱﾙヱｹﾁｸ8ｾeﾄﾛ6C8bZuwﾗｹCZｦRSLQFU` |
| Qlobal reyestr göstəricisi (`registry_id = 0x0000_002A`, `treasury` ekvivalenti) | `3oE9sLeRGP49Cu7mQ1nF4wtKAm29BG4TGLiRsaXe7mhbMP5WZ113nNW1N6RbqF` | `sorakXｹ6NｻﾍﾀﾖSﾜﾖｱ3ﾚ5WﾘﾋQﾅｷｦxgﾛｸcﾁｵﾋkﾋvﾏ8SPﾓﾀｹdｴｴｲW9iCM6AEP` |

Bu sətirlər CLI (`iroha tools address convert`), Torii tərəfindən buraxılanlara uyğundur
cavablar (`address_format=ih58|compressed`) və SDK köməkçiləri, buna görə də UX kopyalayın/yapışdırın
axınlar onlara sözlü etibar edə bilər. Yalnız açıq marşrut göstərişinə ehtiyacınız olduqda `<address>@<domain>` (rejected legacy form) əlavə edin; şəkilçi kanonik çıxışın bir hissəsi deyil.

#### 2.6 Qarşılıqlı fəaliyyət üçün mətn ləqəbləri (planlaşdırılmış)

- ** Zəncir ləqəbi üslubu:** loglar və insan üçün `ih:<chain-alias>:<alias@domain>`
  giriş. Pulqabılar prefiksi təhlil etməli, daxil edilmiş zənciri doğrulamalı və bloklamalıdır
  uyğunsuzluqlar.
- **CAIP-10 forması:** Zəncirsiz aqnostik üçün `iroha:<caip-2-id>:<ih58-addr>`
  inteqrasiyalar. Bu xəritələşdirmə göndəriləndə **hələ həyata keçirilməyib**
  alət zəncirləri.
- **Maşın köməkçiləri:** Rust, TypeScript/JavaScript, Python, üçün kodekləri dərc edin,
  və IH58 və sıxılmış formatları əhatə edən Kotlin (`AccountAddress::to_ih58`,
  `AccountAddress::parse_encoded` və onların SDK ekvivalentləri). CAIP-10 köməkçiləridir
  gələcək iş.

#### 2.7 Deterministik IH58 ləqəbi

- **Prefiksin xəritəsi:** `chain_discriminant`-i IH58 şəbəkə prefiksi kimi yenidən istifadə edin.
  `encode_ih58_prefix()` (bax: `crates/iroha_data_model/src/account/address.rs`)
  `<64` dəyərləri üçün 6-bit prefiks (tək bayt) və 14-bit, iki bayt yayır
  daha böyük şəbəkələr üçün forma. Səlahiyyətli tapşırıqlar yaşayır
  [`address_prefix_registry.md`](source/references/address_prefix_registry.md);
  SDK-lar toqquşmaların qarşısını almaq üçün uyğun JSON reyestrini sinxron şəkildə saxlamalıdırlar.
- **Hesab materialı:** IH58 tərəfindən qurulmuş kanonik faydalı yükü kodlaşdırır
  `AccountAddress::canonical_bytes()`—başlıq baytı, domen seçicisi və
  nəzarətçi yükü. Əlavə hashing addımı yoxdur; IH58 yerləşdirir
  Rust tərəfindən istehsal edilən ikili nəzarətçi yükü (tək açar və ya multisig).
  kodlayıcı, multisig siyasət həzmləri üçün istifadə edilən CTAP2 xəritəsi deyil.
- **Kodlaşdırma:** `encode_ih58()` prefiks baytlarını kanonik baytlarla birləşdirir
  faydalı yüklənir və Blake2b-512-dən əldə edilmiş 16 bitlik yoxlama məbləğini sabitlə əlavə edir.
  prefiks `IH58PRE` (`b"IH58PRE" || prefix || payload`). Nəticə `bs58` vasitəsilə Base58 ilə kodlanmışdır.
  CLI/SDK köməkçiləri eyni proseduru ifşa edir və `AccountAddress::parse_encoded`
  onu `decode_ih58` vasitəsilə geri qaytarır.

#### 2.8 Normativ mətn test vektorları

`fixtures/account/address_vectors.json` tam IH58 (üstünlük verilir) və sıxılmış (`sora`, ikinci ən yaxşı) ehtiva edir
hər kanonik faydalı yük üçün hərflər. Əsas məqamlar:

- **`addr-single-default-ed25519` (Sora Nexus, prefiks `0x02F1`).**  
  IH58 `6cmzPVPX5jDQFNfiz6KgmVfm1fhoAqjPhoPFn4nx9mBWaFMyUCwq4cw`, sıxılmış (`sora`)
  `sora2QG…U4N5E5`. Torii bu dəqiq sətirləri `AccountId`-dən yayır
  `Display` tətbiqi (kanonik IH58) və `AccountAddress::to_compressed_sora`.
- **`addr-global-registry-002a` (reyestr seçicisi → xəzinə).**  
  IH58 `3oE9sLeRGP49Cu7mQ1nF4wtKAm29BG4TGLiRsaXe7mhbMP5WZ113nNW1N6RbqF`, sıxılmış (`sora`)
  `sorakX…CM6AEP`. Qeyd dəftəri seçicilərinin hələ də deşifrə etdiyini nümayiş etdirir
  müvafiq yerli həzm kimi eyni kanonik faydalı yük.
- **Uğursuzluq halı (`ih58-prefix-mismatch`).**  
  Bir qovşaqda `NETWORK_PREFIX + 1` prefiksi ilə kodlanmış IH58 literalının təhlili
  standart prefiksin məhsuldarlığını gözləyir
  `AccountAddressError::UnexpectedNetworkPrefix { expected: 753, found: 754 }`
  domen yönləndirmə cəhdindən əvvəl. `ih58-checksum-mismatch` qurğusu
  Blake2b yoxlama məbləği üzərində saxtakarlığın aşkar edilməsini həyata keçirir.

#### 2.9 Uyğunluq qurğuları

ADDR‑2 müsbət və mənfi cəhətləri əhatə edən təkrar oxuna bilən qurğu paketini göndərir
kanonik hex üzrə ssenarilər, IH58 (üstünlük verilir), sıxılmış (`sora`, yarım/tam genişlik), gizli
defolt seçicilər, qlobal reyestr ləqəbləri və çox imza nəzarətçiləri. The
kanonik JSON `fixtures/account/address_vectors.json`-də yaşayır və ola bilər
ilə bərpa olunur:

```
cargo xtask address-vectors --out fixtures/account/address_vectors.json
# verify without writing:
cargo xtask address-vectors --verify
```

Ad-hoc təcrübələr üçün (müxtəlif yollar/formatlar) ikili nümunə hələ də qalır
mövcuddur:

```
cargo run -p iroha_data_model --example account_address_vectors > fixtures/account/address_vectors.json
```

`crates/iroha_data_model/tests/account_address_vectors.rs`-də pas vahidi testləri
və `crates/iroha_torii/tests/account_address_vectors.rs`, JS ilə birlikdə,
Swift və Android qoşquları (`javascript/iroha_js/test/address.test.js`,
`IrohaSwift/Tests/IrohaSwiftTests/AccountAddressTests.swift`,
`java/iroha_android/src/test/java/org/hyperledger/iroha/android/address/AccountAddressTests.java`),
SDK-lar və Torii qəbulu arasında kodek paritetini təmin etmək üçün eyni qurğudan istifadə edin.

### 3. Qlobal unikal domenlər və normallaşdırma

Həmçinin bax: [`docs/source/references/address_norm_v1.md`](source/references/address_norm_v1.md)
Torii, data modeli və SDK-larda istifadə edilən kanonik Norm v1 boru kəməri üçün.

`DomainId`-i işarələnmiş dəst kimi yenidən təyin edin:

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

`LocalChain` cari zəncir tərəfindən idarə olunan domenlər üçün mövcud Adı əhatə edir.
Domen qlobal reyestr vasitəsilə qeydə alındıqda, biz sahibliyimizi davam etdiririk
zəncirin diskriminantı. Ekran / təhlil hələlik dəyişməz qalır, lakin
genişləndirilmiş struktur marşrutlaşdırma qərarlarına imkan verir.

#### 3.1 Normallaşdırma və saxtakarlığa qarşı müdafiə

Norm v1 domendən əvvəl hər bir komponentin istifadə etməli olduğu kanonik boru xəttini müəyyən edir
ad saxlanılır və ya `AccountAddress`-ə daxil edilir. Tam keçid
[`docs/source/references/address_norm_v1.md`](source/references/address_norm_v1.md)-da yaşayır;
Aşağıdakı xülasə pul kisələri, Torii, SDK-lar və idarəetmə ilə bağlı addımları əks etdirir
alətlər həyata keçirməlidir.

1. **Daxiletmənin doğrulanması.** Boş sətirləri, boşluqları və qorunanları rədd edin
   ayırıcılar `@`, `#`, `$`. Bu, tətbiq etdiyi invariantlara uyğun gəlir
   `Name::validate_str`.
2. **Unicode NFC tərkibi.** ICU tərəfindən dəstəklənən NFC normallaşdırmasını qanuni şəkildə tətbiq edin
   ekvivalent ardıcıllıqlar deterministik şəkildə çökür (məsələn, `e\u{0301}` → `é`).
3. **UTS-46 normallaşdırılması.** NFC çıxışını UTS‑46 vasitəsilə işə salın
   `use_std3_ascii_rules = true`, `transitional_processing = false` və
   DNS uzunluğunun tətbiqi aktivləşdirildi. Nəticə kiçik hərflərlə yazılmış A-etiket ardıcıllığıdır;
   STD3 qaydalarını pozan girişlər burada uğursuz olur.
4. **Uzunluq məhdudiyyətləri.** DNS üslublu sərhədləri tətbiq edin: hər etiket 1–63 olmalıdır
   bayt və tam domen 3-cü addımdan sonra 255 baytı keçməməlidir.
5. **Könüllü qarışıq siyasət.** UTS‑39 skript yoxlamaları üçün izlənilir
   Norm v2; operatorlar onları erkən aktivləşdirə bilər, lakin yoxlama uğursuz olarsa dayandırılmalıdır
   emal.

Hər bir mərhələ uğurlu olarsa, kiçik hərflərlə yazılmış A-etiket sətri keşlənir və bunun üçün istifadə olunur
ünvan kodlaması, konfiqurasiya, manifestlər və reyestr axtarışları. Yerli həzm
seçicilər 12 baytlıq dəyərini `blake2s_mac(açar = "SORA-LOCAL-K:v1",
canonical_label)[0..12]` addım 3 çıxışından istifadə edərək. Bütün digər cəhdlər (qarışıq
hərf, böyük hərf, xam Unicode girişi) strukturlaşdırılmış ilə rədd edilir
`ParseError`s adın verildiyi sərhəddə.

Bu qaydaları nümayiş etdirən kanonik qurğular - punycode gediş-gəlişləri də daxil olmaqla
və etibarsız STD3 ardıcıllığı — siyahıda verilmişdir
`docs/source/references/address_norm_v1.md` və SDK CI-də əks olunur
ADDR‑2 altında izlənilən vektor dəstləri.

### 4. Nexus domen qeydiyyatı və marşrutlaşdırma- **Reyestr sxemi:** Nexus imzalanmış xəritəni saxlayır `DomainName -> ChainRecord`
  burada `ChainRecord` zəncirvari diskriminant, isteğe bağlı metadata (RPC) daxildir
  son nöqtələr) və səlahiyyət sübutu (məsələn, idarəetmə çox imzası).
- **Sinxronizasiya mexanizmi:**
  - Zəncirlər imzalanmış domen iddialarını Nexus-ə təqdim edir (ya genezis zamanı, ya da vasitəsilə
    idarəetmə təlimatı).
  - Nexus dövri manifestləri dərc edir (imzalanmış JSON və əlavə Merkle kökü)
    HTTPS və məzmun ünvanlı yaddaş (məsələn, IPFS) üzərindən. Müştərilər pin edir
    son manifest və imzaları yoxlayın.
- ** Axtarış axını:**
  - Torii `DomainId`-ə istinad edən əməliyyat alır.
  - Əgər domen yerli olaraq naməlumdursa, Torii keşlənmiş Nexus manifestini sorğulayır.
  - Manifest xarici zənciri göstərirsə, əməliyyat rədd edilir
    deterministik `ForeignDomain` xətası və uzaq zəncir məlumatı.
  - Nexus-də domen yoxdursa, Torii `UnknownDomain` qaytarır.
- **Güvən ankerləri və fırlanma:** İdarəetmə açarları işarəsi manifestləri; fırlanma və ya
  ləğvetmə yeni manifest girişi kimi dərc olunur. Müştərilər manifestləri tətbiq edirlər
  TTL-lər (məsələn, 24 saat) və həmin pəncərədən kənarda köhnə data ilə məsləhətləşməkdən imtina edin.
- **Uğursuzluq rejimləri:** Əgər manifestin axtarışı uğursuz olarsa, Torii keşlənmiş vəziyyətə qayıdır
  TTL daxilində məlumatlar; keçmiş TTL o, `RegistryUnavailable` yayır və imtina edir
  uyğunsuz vəziyyətin qarşısını almaq üçün domenlər arası marşrutlaşdırma.

### 4.1 Qeydiyyatın dəyişməzliyi, ləqəblər və məzar daşları (ADDR-7c)

Nexus **yalnız əlavə edilən manifest** dərc edir, beləliklə hər domen və ya ləqəb təyinatı
yoxlanıla və təkrar oynaya bilər. Operatorlar bənddə təsvir olunan paketi müalicə etməlidirlər
[ünvan manifest runbook](source/runbooks/address_manifest_ops.md) kimi
həqiqətin yeganə mənbəyi: əgər manifest çatışmırsa və ya doğrulama uğursuz olarsa, Torii olmalıdır
təsirlənmiş domeni həll etməkdən imtina edin.

Avtomatlaşdırma dəstəyi: `cargo xtask address-manifest verify --bundle <current_dir> --previous <previous_dir>`
-də yazılmış yoxlama məbləğini, sxemi və əvvəlki həzm yoxlamalarını təkrarlayır
runbook. `sequence` göstərmək üçün dəyişiklik biletlərinə komanda çıxışını daxil edin
və `previous_digest` əlaqəsi paketi dərc etməzdən əvvəl təsdiq edilib.

#### Manifest başlığı və imza müqaviləsi

| Sahə | Tələb |
|-------|-------------|
| `version` | Hal-hazırda `1`. Yalnız uyğun spesifikasiya yeniləməsi ilə çarpın. |
| `sequence` | Hər nəşr üçün **dəqiq** bir artım. Torii keşləri boşluqlar və ya reqressiyalar olan düzəlişləri rədd edir. |
| `generated_ms` + `ttl_hours` | Keş təzəliyini təyin edin (standart 24 saat). TTL növbəti nəşrdən əvvəl başa çatarsa, Torii `RegistryUnavailable`-ə çevrilir. |
| `previous_digest` | BLAKE3 əvvəlki manifest cismin həzmi (hex). Doğrulayıcılar dəyişməzliyi sübut etmək üçün onu `b3sum` ilə yenidən hesablayırlar. |
| `signatures` | Manifestlər Sigstore (`cosign sign-blob`) vasitəsilə imzalanır. Əməliyyatlar `cosign verify-blob --bundle manifest.sigstore manifest.json`-i işə salmalı və buraxılışdan əvvəl idarəetmə şəxsiyyəti/emitentin məhdudiyyətlərini tətbiq etməlidir. |

Buraxılış avtomatlaşdırılması `manifest.sigstore` və `checksums.sha256` yayır
JSON gövdəsi ilə yanaşı. SoraFS və ya əks etdirərkən faylları bir yerdə saxlayın
HTTP son nöqtələri beləliklə, auditorlar yoxlama addımlarını sözbəsöz təkrarlaya bilsinlər.

#### Giriş növləri

| Növ | Məqsəd | Tələb olunan sahələr |
|------|---------|-----------------|
| `global_domain` | Bir domenin qlobal miqyasda qeydiyyatdan keçdiyini və zəncir diskriminantına və IH58 prefiksinə uyğunlaşmalı olduğunu bildirir. | `{ "domain": "<label>", "chain": "sora:nexus:global", "ih58_prefix": 753, "selector": "global" }` |
| `tombstone` | Ləqəb/selektoru həmişəlik tərk edir. Local‑8 həzmləri və ya domeni silərkən tələb olunur. | `{ "selector": {…}, "reason_code": "LOCAL8_RETIREMENT" \| …, "ticket": "<governance id>", "replaces_sequence": <number> }` |

`global_domain` qeydlərinə isteğe bağlı olaraq `manifest_url` və ya `sorafs_cid` daxil ola bilər
pul kisələrini imzalanmış zəncir metadatasına yönəltmək, lakin kanonik dəst qalır
`{domain, chain, discriminant/ih58_prefix}`. `tombstone` qeydləri **istinad edilməlidir**
təqaüdə çıxan seçici və icazə verən bilet/idarəetmə artefaktı
audit cığırının oflayn olaraq yenidən qurulması üçün dəyişiklik.

#### Alias/qəbir daşı iş axını və telemetriya

1. **Drift aşkarlayın.** `torii_address_local8_total{endpoint}` istifadə edin,
   `torii_address_local8_domain_total{endpoint,domain}`,
   `torii_address_collision_total{endpoint,kind="local12_digest"}`,
   `torii_address_collision_domain_total{endpoint,domain}`,
   `torii_address_domain_total{endpoint,domain_kind}`, və
   `torii_address_invalid_total{endpoint,reason}` (də göstərilmişdir
   `dashboards/grafana/address_ingest.json`) Yerli təqdimatları təsdiqləmək və
   Yerli-12 toqquşması məzar daşını təklif etməzdən əvvəl sıfırda qalır. The
   hər domen sayğacları sahiblərə yalnız inkişaf/test domenlərinin Local‑8 yaydığını sübut etməyə imkan verir
   trafik (və Local‑12 toqquşmalarının məlum quruluş domenləri ilə xəritəsi) isə
   **Domain Tipi Qarışıq (5m)** panelini ehtiva edir ki, SRE-lərin nə qədər olduğunu qrafikləşdirə bilsin
   `domain_kind="local12"` trafiki qalır və `AddressLocal12Traffic`
   İstehsal hələ də yerli-12 seçiciləri gördüyündə xəbərdarlıq atəşləri
   pensiya qapısı.
2. **Kanonik həzmlər əldə edin.** Çalışın
   `iroha tools address convert <address> --format json --expect-prefix 753`
   (və ya vasitəsilə `fixtures/account/address_vectors.json` istehlak edin
   `scripts/account_fixture_helper.py`) dəqiq `digest_hex` tutmaq üçün.
   CLI IH58, `sora…` və kanonik `0x…` literallarını qəbul edir; əlavə edin
   `@<domain>` yalnız manifestlər üçün etiket saxlamaq lazım olduqda.
   JSON xülasəsi həmin domeni `input_domain` sahəsi vasitəsilə göstərir və
   `legacy  suffix` çevrilmiş kodlaşdırmanı `<address>@<domain>` (rejected legacy form) kimi təkrarlayır.
   manifest fərqləri (bu şəkilçi kanonik hesab identifikatoru deyil, metadatadır).
   Yeni sətir yönümlü ixrac üçün istifadə edin
   Kütləvi çevirmək üçün `iroha tools address normalize --input <file> legacy-selector input mode` Yerli
   seçiciləri atlayarkən kanonik IH58 (üstünlük verilir), sıxılmış (`sora`, ikinci ən yaxşı), hex və ya JSON formalarına
   yerli olmayan sıralar. Auditorların elektron cədvələ uyğun sübuta ehtiyacı olduqda, qaçın
   CSV xülasəsi yaymaq üçün `iroha tools address audit --input <file> --format csv`
   Yerli seçiciləri vurğulayan (`input,status,format,domain_kind,…`),
   kanonik kodlaşdırmalar və eyni faylda uğursuzluqları təhlil edin.
3. **Manifest qeydlərini əlavə edin.** `tombstone` qeydini (və sonrakı məlumatları) tərtib edin
   Qlobal reyestrə köçərkən `global_domain` qeyd edin) və doğrulayın
   imza tələb etməzdən əvvəl `cargo xtask address-vectors` ilə manifest.
4. **Doğrulayın və dərc edin.** Runbook yoxlama siyahısına əməl edin (heşlər, Sigstore,
   ardıcıllığın monotonluğu) paketi SoraFS-ə əks etdirməzdən əvvəl. Torii indi
   Paketdən dərhal sonra IH58 (üstünlük verilir)/sora (ikinci ən yaxşı) literalları kanonikləşdirir.
5. **Monitor və geri qaytarın.** Local‑8 və Local‑12 toqquşma panellərini aşağıda saxlayın
   30 gün ərzində sıfır; reqressiyalar görünsə, əvvəlki manifesti yenidən dərc edin
   telemetriya sabitləşənə qədər yalnız təsirə məruz qalmış qeyri-istehsal mühitində.

Yuxarıdakı addımların hamısı ADDR‑7c üçün məcburi sübutdur: olmadan təzahür edir
`cosign` imza paketi və ya `previous_digest` dəyərlərinə uyğun gəlməyən
avtomatik olaraq rədd edilir və operatorlar yoxlama jurnallarını əlavə etməlidirlər
onların dəyişmə biletləri.

### 5. Pul kisəsi və API erqonomikası

- **Defolt ekran parametrləri:** Pul kisələri IH58 ünvanını göstərir (qısa, yoxlama cəmi)
  üstəgəl həll edilmiş domen reyestrdən alınan etiket kimi. Domenlərdir
  aydın şəkildə dəyişə bilən təsviri metadata kimi qeyd olunur, IH58 isə
  sabit ünvan.
- **Daxiletmənin kanonikləşdirilməsi:** Torii və SDK-lar IH58 (üstünlük verilir)/sora (ikinci ən yaxşı)/0x qəbul edir
  ünvanlar üstəgəl `alias@domain` (rejected legacy form), `uaid:…` və
  `opaque:…` formalaşdırır, sonra çıxış üçün IH58-ə kanonikləşir. yoxdur
  ciddi rejimdə keçid; xam telefon/e-poçt identifikatorları kitabdan kənar saxlanılmalıdır
  UAID/şəffaf xəritələr vasitəsilə.
- **Xətanın qarşısının alınması:** Pul kisələri IH58 prefikslərini təhlil edir və zəncirvari diskriminant tətbiq edir
  gözləntilər. Zəncirvari uyğunsuzluqlar təsirli diaqnostika ilə çətin uğursuzluqlara səbəb olur.
- **Codec kitabxanaları:** Rəsmi Rust, TypeScript/JavaScript, Python və Kotlin
  kitabxanalar IH58 kodlaşdırma/şifrləmə və sıxılmış (`sora`) dəstəyi təmin edir.
  parçalanmış tətbiqlərdən çəkinin. CAIP-10 dönüşümləri hələ göndərilməyib.

#### Əlçatanlıq və Təhlükəsiz Paylaşım Rəhbərliyi- Məhsul səthləri üçün icra təlimatı canlı olaraq izlənilir
  `docs/portal/docs/reference/address-safety.md`; zaman həmin yoxlama siyahısına istinad edin
  bu tələbləri cüzdan və ya explorer UX-ə uyğunlaşdırmaq.
- **Təhlükəsiz paylaşma axınları:** Ünvanları defolt olaraq IH58 formasına köçürən və ya göstərən səthlər və istifadəçilərin yoxlama məbləğini vizual və ya skan etməklə yoxlaya bilməsi üçün həm tam sətri, həm də eyni faydalı yükdən əldə edilən QR kodunu təqdim edən bitişik “paylaşma” əməliyyatını ifşa edən səthlər. Kəsilmə qaçınılmaz olduqda (məsələn, kiçik ekranlar), sətirin başlanğıcını və sonunu saxlayın, aydın ellipslər əlavə edin və təsadüfən kəsilmənin qarşısını almaq üçün tam ünvanı buferə köçürmə vasitəsilə əlçatan saxlayın.
- **IME qoruyucuları:** Ünvan girişləri IME/IME tipli klaviaturaların kompozisiya artefaktlarını rədd etməlidir. Yalnız ASCII girişini tətbiq edin, tam enli və ya Kana simvolları aşkar edildikdə daxili xəbərdarlıq təqdim edin və Yapon və Çin istifadəçilərinin tərəqqini itirmədən öz IME-ni söndürə bilməsi üçün doğrulamadan əvvəl işarələri birləşdirən düz mətn yapışdırma zonası təklif edin.
- **Ekran oxuyucu dəstəyi:** Aparıcı Base58 prefiks rəqəmlərini təsvir edən və IH58 faydalı yükünü 4 və ya 8 simvoldan ibarət qruplara bölən vizual olaraq gizli etiketlər (`aria-label`/`aria-describedby`) təmin edin. Nəzakətli canlı bölgələr vasitəsilə kopyalama/paylaşım uğurunu elan edin və QR önizləmələrinə təsviri alt mətn (“0x02F1 zəncirində <ləqəb> üçün IH58 ünvanı”) daxil olduğundan əmin olun.
- **Yalnız Sora üçün sıxılmış istifadə:** Həmişə `sora…` sıxılmış görünüşünü "Yalnız Sora" kimi etiketləyin və kopyalamadan əvvəl onu açıq təsdiqin arxasına keçin. Zəncirvari diskriminant Sora Nexus dəyəri olmadıqda SDK və pul kisələri sıxılmış çıxışı göstərməkdən imtina etməli və vəsaitlərin yanlış yönləndirilməsinin qarşısını almaq üçün istifadəçiləri şəbəkələrarası köçürmələr üçün IH58-ə yönləndirməlidir.

## İcra Yoxlama Siyahısı

- **IH58 zərfi:** Prefiks kompaktdan istifadə edərək `chain_discriminant`-i kodlayır
  `encode_ih58_prefix()`-dən 6-/14-bit sxem, gövdə kanonik baytdır
  (`AccountAddress::canonical_bytes()`) və yoxlama məbləği ilk iki baytdır
  Blake2b-512(`b"IH58PRE"` || prefiks || gövdə). Tam faydalı yük Base58-dir
  `bs58` vasitəsilə kodlaşdırılmışdır.
- **Reyestr müqaviləsi:** İmzalanmış JSON (və əlavə Merkle kökü) nəşri
  24 saat TTL ilə `{discriminant, ih58_prefix, chain_alias, endpoints}` və
  fırlanma düymələri.
- **Domen siyasəti:** ASCII `Name` bu gün; i18n-i aktivləşdirirsinizsə, üçün UTS-46 tətbiq edin
  normallaşdırma və qarışıq yoxlamalar üçün UTS-39. Maksimum etiketi tətbiq edin (63) və
  cəmi (255) uzunluq.
- **Mətn köməkçiləri:** Rust-da IH58 ↔ sıxılmış (`sora…`) kodekləri göndərin,
  Paylaşılan test vektorları ilə TypeScript/JavaScript, Python və Kotlin (CAIP-10
  Xəritəçəkmələr gələcək iş olaraq qalır).
- **CLI alətləri:** `iroha tools address convert` vasitəsilə deterministik operator iş axını təmin edin
  (bax `crates/iroha_cli/src/address.rs`), IH58/`sora…`/`0x…` hərfi və
  isteğe bağlı `<address>@<domain>` (rejected legacy form) etiketləri, defolt olaraq Sora Nexus (`753`) prefiksindən istifadə edərək IH58 çıxışı,
  və yalnız operatorlar bunu açıq şəkildə tələb etdikdə yalnız Sora sıxılmış əlifbasını yayır
  `--format compressed` və ya JSON xülasə rejimi. Komanda prefiks gözləntilərini tətbiq edir
  təhlil edir, təmin edilmiş domeni (JSON-da `input_domain`) və `legacy  suffix` bayrağını qeyd edir
  çevrilmiş kodlaşdırmanı `<address>@<domain>` (rejected legacy form) kimi təkrarlayır, beləliklə, aşkar fərqlər erqonomik olaraq qalır.
- **Wallet/explorer UX:** [ünvan göstərmə qaydalarına] əməl edin (source/sns/address_display_guidelines.md)
  ADDR-6 ilə göndərilir - ikili nüsxə düymələri təklif edin, IH58-i QR yükü kimi saxlayın və xəbərdarlıq edin
  istifadəçilər sıxılmış `sora…` formasının yalnız Sora-dır və IME-nin yenidən yazılmasına həssasdır.
- **Torii inteqrasiyası:** Keş Nexus TTL-ə uyğun olaraq təzahür edir, yayır
  `ForeignDomain`/`UnknownDomain`/`RegistryUnavailable` deterministik və
  keep account-literal parsing encoded-only (`IH58` preferred, `sora…`
  compressed accepted) with canonical IH58 output.

### Torii cavab formatları

- `GET /v1/accounts` isteğe bağlı `address_format` sorğu parametrini qəbul edir və
  `POST /v1/accounts/query` JSON zərfində eyni sahəni qəbul edir.
  Dəstəklənən dəyərlər bunlardır:
  - `ih58` (defolt) — cavablar kanonik IH58 Base58 faydalı yükləri yayır (məsələn,
    `6cmzPVPX5jDQFNfiz6KgmVfm1fhoAqjPhoPFn4nx9mBWaFMyUCwq4cw`).
  - `compressed` — cavablar yalnız Sora üçün `sora…` sıxılmış görünüşünü yayır.
    filtrlərin/yol parametrlərinin kanonik saxlanması.
- Yanlış dəyərlər `400` (`QueryExecutionFail::Conversion`) qaytarır. Bu imkan verir
  cüzdanlar və tədqiqatçılar yalnız Sora-da UX üçün sıxılmış sətirlər tələb etsinlər
  IH58-i qarşılıqlı işləyə bilən standart olaraq saxlayır.
- Aktiv sahibi siyahıları (`GET /v1/assets/{definition_id}/holders`) və onların JSON
  zərf həmkarı (`POST …/holders/query`) də `address_format`-i şərəfləndirir.
  `items[*].account_id` sahəsi hər dəfə sıxılmış hərflər buraxır
  parametr/zərf sahəsi hesabları əks etdirərək `compressed` olaraq təyin edilib
  tədqiqatçıların qovluqlar arasında ardıcıl çıxış təqdim edə bilməsi üçün son nöqtələr.
- **Sınaq:** Kodlayıcı/dekoderin gediş-gəlişi, səhv zəncir üçün vahid testləri əlavə edin
  uğursuzluqlar və açıq axtarışlar; Torii və SDK-larda inteqrasiya əhatəsini əlavə edin
  IH58 axınları üçün başdan sona.

## Xəta Kodu Qeydiyyatı

Ünvan kodlayıcıları və dekoderlər uğursuzluqları aşkar edir
`AccountAddressError::code_str()`. Aşağıdakı cədvəllər sabit kodları təmin edir
SDK-lar, pul kisələri və Torii səthləri insanların oxuya biləcəyi səthlərlə yanaşı səthə çıxmalıdır
mesajlar, üstəlik tövsiyə olunan düzəliş təlimatı.

### Kanonik tikinti

| Kod | Uğursuzluq | Tövsiyə olunan Təmir |
|------|---------|-------------------------|
| `ERR_UNSUPPORTED_ALGORITHM` | Kodlayıcı reyestr tərəfindən dəstəklənməyən imzalama alqoritmi və ya qurma funksiyaları aldı. | Hesabın qurulmasını reyestrdə və konfiqurasiyada aktivləşdirilmiş əyrilərlə məhdudlaşdırın. |
| `ERR_KEY_PAYLOAD_TOO_LONG` | İmzalanan açarın yüklənmə uzunluğu dəstəklənən limiti keçir. | Tək açarlı kontrollerlər `u8` uzunluqları ilə məhdudlaşır; böyük açıq açarlar üçün multisig istifadə edin (məsələn, ML-DSA). |
| `ERR_INVALID_HEADER_VERSION` | Ünvan başlığı versiyası dəstəklənən diapazondan kənardadır. | V1 ünvanları üçün `0` başlıq versiyasını buraxın; yeni versiyaları qəbul etməzdən əvvəl kodlayıcıları təkmilləşdirin. |
| `ERR_INVALID_NORM_VERSION` | Normallaşdırma versiyasının bayrağı tanınmır. | `1` normallaşdırma versiyasından istifadə edin və rezerv edilmiş bitləri dəyişdirməyin. |
| `ERR_INVALID_IH58_PREFIX` | Tələb olunan IH58 şəbəkə prefiksi kodlana bilməz. | Zəncir reyestrində dərc edilmiş inklüziv `0..=16383` diapazonunda prefiks seçin. |
| `ERR_CANONICAL_HASH_FAILURE` | Canonical faydalı yük hashing uğursuz oldu. | Əməliyyatı yenidən sınayın; xəta davam edərsə, onu hashing yığınında daxili səhv kimi qəbul edin. |

### Format Deşifrə və Avtomatik Aşkarlama

| Kod | Uğursuzluq | Tövsiyə olunan Təmir |
|------|---------|-------------------------|
| `ERR_INVALID_IH58_ENCODING` | IH58 sətirində əlifbadan kənar simvollar var. | Ünvanın dərc edilmiş IH58 əlifbasından istifadə etdiyinə və kopyalama/yapışdırma zamanı kəsilmədiyinə əmin olun. |
| `ERR_INVALID_LENGTH` | Faydalı yükün uzunluğu seçici/nəzarətçi üçün gözlənilən kanonik ölçüyə uyğun gəlmir. | Seçilmiş domen seçicisi və nəzarətçi tərtibatı üçün tam kanonik faydalı yükü təmin edin. |
| `ERR_CHECKSUM_MISMATCH` | IH58 (üstünlük verilir) və ya sıxılmış (`sora`, ikinci ən yaxşı) yoxlama məbləğinin doğrulanması uğursuz oldu. | Etibarlı mənbədən ünvanı bərpa edin; bu adətən kopyala/yapışdır xətasını göstərir. |
| `ERR_INVALID_IH58_PREFIX_ENCODING` | IH58 prefiks baytları səhv formalaşdırılıb. | Ünvanı uyğun kodlayıcı ilə yenidən kodlayın; aparıcı Base58 baytını əl ilə dəyişdirməyin. |
| `ERR_INVALID_HEX_ADDRESS` | Kanonik onaltılıq formanı deşifrə etmək alınmadı. | Rəsmi kodlayıcı tərəfindən istehsal olunan `0x`-prefiksli, bərabər uzunluqlu hex simli təmin edin. |
| `ERR_MISSING_COMPRESSED_SENTINEL` | Sıxılmış forma `sora` ilə başlamır. | Sıxılmış Sora ünvanlarını dekoderlərə verməzdən əvvəl lazımi gözətçi ilə prefiks edin. |
| `ERR_COMPRESSED_TOO_SHORT` | Sıxılmış sətirdə faydalı yük və yoxlama məbləği üçün kifayət qədər rəqəmlər yoxdur. | Kəsilmiş fraqmentlər əvəzinə kodlayıcı tərəfindən buraxılan tam sıxılmış sətirdən istifadə edin. |
| `ERR_INVALID_COMPRESSED_CHAR` | Sıxılmış əlifbanın xaricində xarakter qarşılaşdı. | Dərc olunmuş yarım enli/tam enli cədvəllərdən simvolu etibarlı Base‑105 qlifi ilə əvəz edin. |
| `ERR_INVALID_COMPRESSED_BASE` | Kodlayıcı dəstəklənməyən radiksdən istifadə etməyə cəhd etdi. | Kodlayıcıya qarşı səhv bildirin; sıxılmış əlifba V1-də radix 105-ə sabitlənmişdir. |
| `ERR_INVALID_COMPRESSED_DIGIT` | Rəqəm dəyəri sıxılmış əlifba ölçüsünü keçir. | Hər bir rəqəmin `0..105)` daxilində olduğundan əmin olun, lazım olduqda ünvanı bərpa edin. |
| `ERR_UNSUPPORTED_ADDRESS_FORMAT` | Avtomatik aşkarlama daxiletmə formatını tanıya bilmədi. | Parserləri çağırarkən IH58 (üstünlük verilir), sıxılmış (`sora`) və ya kanonik `0x` hex sətirləri təmin edin. |

### Domen və Şəbəkə Doğrulaması| Kod | Uğursuzluq | Tövsiyə olunan Təmir |
|------|---------|-------------------------|
| `ERR_DOMAIN_MISMATCH` | Domen seçicisi gözlənilən domenlə uyğun gəlmir. | Nəzərdə tutulan domen üçün verilmiş ünvandan istifadə edin və ya gözləntiləri yeniləyin. |
| `ERR_INVALID_DOMAIN_LABEL` | Domen etiketi normallaşdırma yoxlamaları uğursuz oldu. | Kodlaşdırmadan əvvəl UTS-46 keçidsiz emaldan istifadə edərək domeni kanonikləşdirin. |
| `ERR_UNEXPECTED_NETWORK_PREFIX` | Deşifrə edilmiş IH58 şəbəkə prefiksi konfiqurasiya edilmiş dəyərdən fərqlənir. | Hədəf zəncirindən ünvana keçin və ya gözlənilən diskriminant/prefiksi tənzimləyin. |
| `ERR_UNKNOWN_ADDRESS_CLASS` | Ünvan sinfi bitləri tanınmır. | Dekoderi yeni sinfi anlayan buraxılışa təkmilləşdirin və ya başlıq bitlərinə müdaxilə etməyin. |
| `ERR_UNKNOWN_DOMAIN_TAG` | Domen seçici teqi məlum deyil. | Yeni seçici növünü dəstəkləyən buraxılışa güncəlləyin və ya V1 qovşaqlarında eksperimental faydalı yüklərdən istifadə etməyin. |
| `ERR_UNEXPECTED_EXTENSION_FLAG` | Qorunan genişləndirmə biti təyin edildi. | Qorunan bitləri silin; gələcək ABI onları təqdim edənə qədər onlar qapalı qalırlar. |
| `ERR_UNKNOWN_CONTROLLER_TAG` | Nəzarətçinin faydalı yükü etiketi tanınmadı. | Yeni nəzarətçi növlərini təhlil etməzdən əvvəl tanımaq üçün dekoderi təkmilləşdirin. |
| `ERR_UNEXPECTED_TRAILING_BYTES` | Kanonik faydalı yük deşifrədən sonra arxa baytlardan ibarət idi. | Kanonik faydalı yükü bərpa edin; yalnız sənədləşdirilmiş uzunluq mövcud olmalıdır. |

### Nəzarətçinin Yük Yükü Doğrulaması

| Kod | Uğursuzluq | Tövsiyə olunan Təmir |
|------|---------|-------------------------|
| `ERR_INVALID_PUBLIC_KEY` | Açar baytlar elan edilmiş əyri ilə uyğun gəlmir. | Açar baytların tam olaraq seçilmiş əyri üçün tələb olunduğu kimi kodlandığından əmin olun (məsələn, 32 bayt Ed25519). |
| `ERR_UNKNOWN_CURVE` | Əyri identifikator qeydə alınmayıb. | Əlavə əyrilər təsdiqlənənə və reyestrdə dərc olunana qədər əyri ID `1` (Ed25519) istifadə edin. |
| `ERR_MULTISIG_MEMBER_OVERFLOW` | Multisig nəzarətçisi dəstəklənəndən daha çox üzv elan edir. | Kodlaşdırmadan əvvəl multisig üzvlüyünü sənədləşdirilmiş limitə qədər azaldın. |
| `ERR_INVALID_MULTISIG_POLICY` | Multisig siyasətinin faydalı yükü doğrulanmadı (həddi/çəkilər/şema). | Siyasəti elə yenidən qurun ki, o, CTAP2 sxemini, çəki hədlərini və hədd məhdudiyyətlərini təmin etsin. |

## Alternativlər nəzərdən keçirilir

- **Pure Base58Check (Bitcoin-stil).** Daha sadə yoxlama məbləği, lakin daha zəif səhv aşkarlanması
  Blake2b-dən əldə edilən IH58 yoxlama cəmindən (`encode_ih58` 512 bitlik hashı kəsir)
  və 16 bitlik diskriminantlar üçün açıq prefiks semantikası yoxdur.
- **Domen sətirində zəncir adının daxil edilməsi (məsələn, `finance@chain`).** Fasilələr
- **Ünvanları dəyişdirmədən yalnız Nexus marşrutlaşdırmasına etibar edin.** İstifadəçilər hələ də
  qeyri-müəyyən sətirləri kopyalayın/yapışdırın; ünvanın özünün kontekst daşımasını istəyirik.
- **Bech32m zərf.** QR dostudur və insan tərəfindən oxuna bilən prefiks təklif edir, lakin
  göndərmə IH58 tətbiqindən fərqli olacaq (`AccountAddress::to_ih58`)
  və bütün qurğuların/SDK-ların yenidən yaradılmasını tələb edir. Mövcud yol xəritəsi IH58 + saxlayır
  gələcəkdə tədqiqatı davam etdirərkən sıxılmış (`sora`) dəstəyi
  Bech32m/QR təbəqələri (CAIP-10 xəritələşdirilməsi təxirə salınıb).

## Açıq Suallar

- `u16` diskriminantların və qorunan diapazonların uzunmüddətli tələbi əhatə etdiyini təsdiqləyin;
  əks halda varint kodlaşdırması ilə `u32` qiymətləndirin.
- Reyestr yeniləmələri üçün çox imzalı idarəetmə prosesini yekunlaşdırın və necə
  ləğvetmələr/müddəti bitmiş ayırmalar idarə olunur.
- Dəqiq manifest imza sxemini müəyyən edin (məsələn, Ed25519 multi-sig) və
  Nexus paylanması üçün nəqliyyat təhlükəsizliyi (HTTPS sancma, IPFS hash formatı).
- Miqrasiya üçün domen ləqəblərinin/istiqamətləndirilməsinin dəstəkləndiyini və necə dəstəklənəcəyini müəyyənləşdirin
  determinizmi pozmadan onları üzə çıxarmaq.
- Kotodama/IVM müqavilələrinin IH58 köməkçilərinə necə daxil olduğunu göstərin (`to_address()`,
  `parse_address()`) və zəncirli saxlama heç vaxt CAIP-10-u ifşa edib-etməməlidir
  Xəritəçəkmələr (bu gün IH58 kanonikdir).
- Xarici registrlərdə Iroha zəncirlərinin qeydiyyatını araşdırın (məsələn, IH58 reyestri,
  CAIP ad məkanı kataloqu) daha geniş ekosistemin uyğunlaşdırılması üçün.

## Növbəti Addımlar

1. IH58 kodlaşdırması `iroha_data_model` (`AccountAddress::to_ih58`,
   `parse_encoded`); qurğuları/testləri hər SDK-ya daşımağa davam edin və hər hansı birini təmizləyin
   Bech32m yer tutucular.
2. `chain_discriminant` ilə konfiqurasiya sxemini genişləndirin və həssas əldə edin
  mövcud test/dev quraşdırmaları üçün defoltlar. **(Tamamlandı: `common.chain_discriminant`
  indi `iroha_config`-də göndərilir, defolt olaraq hər şəbəkə ilə `0x02F1`
  əvəz edir.)**
3. Nexus reyestr sxemini və konsepsiya sübutu manifest naşirinin layihəsini hazırlayın.
4. Pulqabı provayderlərindən və qəyyumlardan insan faktoru aspektləri üzrə rəy toplayın
   (HRP adlandırma, ekran formatı).
5. Sənədləri yeniləyin (`docs/source/data_model.md`, Torii API sənədləri).
   həyata keçirmə yolu müəyyən edilir.
6. Rəsmi kodek kitabxanalarını (Rust/TS/Python/Kotlin) normativ testlə göndərin
   uğur və uğursuzluq hallarını əhatə edən vektorlar.
