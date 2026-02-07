---
lang: uz
direction: ltr
source: docs/portal/docs/sns/registry-schema.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5307c80eba9ab93d4522c3e88485fa4d24f7f04903b7aea30b05e880e2c096b0
source_last_modified: "2026-01-28T17:11:30.697638+00:00"
translation_last_reviewed: 2026-02-07
id: registry-schema
title: Sora Name Service Registry Schema
sidebar_label: Registry schema
description: Norito data structures, lifecycle rules, and event contracts for SNS registry smart contracts (SN-2a).
translator: machine-google-reviewed
---

::: Eslatma Kanonik manba
Ushbu sahifa `docs/source/sns/registry_schema.md`ni aks ettiradi va hozirda ushbu sahifa sifatida xizmat qiladi
kanonik portal nusxasi. Manba fayl tarjima yangilanishlari uchun qoladi.
:::

# Sora nomi xizmati reestri sxemasi (SN-2a)

**Holat:** 2026-03-24-da ishlab chiqilgan -- SNS dasturini ko‘rib chiqish uchun yuborilgan  
**Yo‘l xaritasi havolasi:** SN-2a “Ro‘yxatga olish sxemasi va saqlash tartibi”  
**Qoʻl:** Sora Name Service (SNS) uchun kanonik Norito tuzilmalari, hayot aylanish holatlari va emissiya hodisalarini aniqlang, shuning uchun registr va registrator ilovalari shartnomalar, SDKlar va shlyuzlarda deterministik boʻlib qoladi.

Ushbu hujjat SN-2a uchun taqdim etilishi mumkin bo'lgan sxemani ko'rsatish orqali to'ldiradi:

1. Identifikatorlar va xeshlash qoidalari (`SuffixId`, `NameHash`, selektor hosilasi).
2. Norito nom yozuvlari, qoʻshimchalar siyosati, narxlash darajalari, daromadlarni taqsimlash va roʻyxatga olish hodisalari uchun tuzilmalar/raqamlar.
3. Deterministik takrorlash uchun saqlash tartibi va indeks prefikslari.
4. Ro'yxatga olish, yangilash, inoyat/to'lov, muzlash va qabr toshlarini qamrab oluvchi davlat mashinasi.
5. DNS/shlyuzni avtomatlashtirish tomonidan iste'mol qilinadigan kanonik hodisalar.

## 1. Identifikatorlar va xeshlash

| Identifikator | Tavsif | Chiqarish |
|------------|-------------|------------|
| `SuffixId` (`u16`) | Yuqori darajadagi qo'shimchalar uchun registr bo'yicha identifikator (`.sora`, `.nexus`, `.dao`). [`sns_suffix_governance_charter.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md) qoʻshimchalari katalogiga moslangan. | Boshqaruv tomonidan ovoz berish orqali tayinlangan; `SuffixPolicyV1` da saqlanadi. |
| `SuffixSelector` | Qo'shimchaning kanonik qator shakli (ASCII, kichik harf). | Misol: `.sora` → `sora`. |
| `NameSelectorV1` | Ro'yxatdan o'tgan yorliq uchun ikkilik selektor. | `struct NameSelectorV1 { version:u8 (=1); suffix_id:u16; label_len:u16; label_bytes:Vec<u8> }`. Yorliq NFC + Normv1 uchun kichik harf. |
| `NameHash` (`[u8;32]`) | Shartnomalar, hodisalar va keshlar tomonidan ishlatiladigan asosiy qidirish kaliti. | `blake3(NameSelectorV1_bytes)`. |

Determinizm talablari:

- Yorliqlar Normv1 (UTS-46 qattiq, STD3 ASCII, NFC) orqali normallashtiriladi. Kiruvchi foydalanuvchi satrlari xeshlashdan oldin normallashtirilishi KERAK.
- Zaxiralangan teglar (`SuffixPolicyV1.reserved_labels` dan) hech qachon reestrga kirmaydi; faqat boshqaruvni bekor qilish `ReservedNameAssigned` hodisalarini chiqaradi.

## 2. Norito tuzilmalari

### 2.1 NameRecordV1

| Maydon | Tur | Eslatmalar |
|-------|------|-------|
| `suffix_id` | `u16` | Adabiyotlar `SuffixPolicyV1`. |
| `selector` | `NameSelectorV1` | Audit/debug uchun xom selektor baytlari. |
| `name_hash` | `[u8; 32]` | Xaritalar/hodisalar uchun kalit. |
| `normalized_label` | `AsciiString` | Inson oʻqishi mumkin boʻlgan yorliq (Normv1 posti). |
| `display_label` | `AsciiString` | Styuard tomonidan taqdim etilgan korpus; ixtiyoriy kosmetika. |
| `owner` | `AccountId` | Yangilash/o'tkazishni nazorat qiladi. |
| `controllers` | `Vec<NameControllerV1>` | Maqsadli hisob manzillari, hal qiluvchilar yoki ilova metamaʼlumotlariga havolalar. |
| `status` | `NameStatus` | Hayotiy tsikl belgisi (4-bo'limga qarang). |
| `pricing_class` | `u8` | Indeks qo'shimcha narxlash darajalariga (standart, premium, zaxiralangan). |
| `registered_at` | `Timestamp` | Dastlabki faollashtirish vaqt belgisini bloklash. |
| `expires_at` | `Timestamp` | To'lov muddati tugashi. |
| `grace_expires_at` | `Timestamp` | Avtomatik yangilash imtiyozining tugashi (standart +30 kun). |
| `redemption_expires_at` | `Timestamp` | Foydalanish oynasining tugashi (standart +60 kun). |
| `auction` | `Option<NameAuctionStateV1>` | Gollandiyaliklar qayta ochilganda yoki premium auktsionlar faol bo'lganda mavjud. |
| `last_tx_hash` | `Hash` | Ushbu versiyani yaratgan tranzaksiya uchun deterministik ko'rsatgich. |
| `metadata` | `Metadata` | O'zboshimchalik bilan ro'yxatga oluvchining metama'lumotlari (matnli yozuvlar, dalillar). |

Qo'llab-quvvatlovchi tuzilmalar:

```text
Enum NameStatus {
    Available,          // derived, not stored on-ledger
    PendingAuction,
    Active,
    GracePeriod,
    Redemption,
    Frozen(NameFrozenStateV1),
    Tombstoned(NameTombstoneStateV1)
}

Struct NameFrozenStateV1 {
    reason: String,
    until_ms: u64,
}

Struct NameTombstoneStateV1 {
    reason: String,
}

Struct NameControllerV1 {
    controller_type: ControllerType,   // Account, ResolverTemplate, ExternalLink
    account_address: Option<AccountAddress>,   // Serialized as canonical `0x…` hex in JSON
    resolver_template_id: Option<String>,
    payload: Metadata,                 // Extra selector/value pairs for wallets/gateways
}

Struct TokenValue {
    asset_id: AsciiString,
    amount: u128,
}

Enum ControllerType {
    Account,
    Multisig,
    ResolverTemplate,
    ExternalLink
}

Struct NameAuctionStateV1 {
    kind: AuctionKind,             // Vickrey, DutchReopen
    opened_at_ms: u64,
    closes_at_ms: u64,
    floor_price: TokenValue,
    highest_commitment: Option<Hash>,  // reference to sealed bid
    settlement_tx: Option<Json>,
}

Enum AuctionKind {
    VickreyCommitReveal,
    DutchReopen
}
```

### 2.2 SuffixPolicyV1

| Maydon | Tur | Eslatmalar |
|-------|------|-------|
| `suffix_id` | `u16` | Asosiy kalit; siyosat versiyalarida barqaror. |
| `suffix` | `AsciiString` | masalan, `sora`. |
| `steward` | `AccountId` | Styuard boshqaruv ustavida belgilangan. |
| `status` | `SuffixStatus` | `Active`, `Paused`, `Revoked`. |
| `payment_asset_id` | `AsciiString` | Birlamchi hisob-kitob aktivi identifikatori (masalan, `xor#sora`). |
| `pricing` | `Vec<PriceTierV1>` | Darajali narxlash koeffitsientlari va muddat qoidalari. |
| `min_term_years` | `u8` | Darajani bekor qilishdan qat'i nazar, sotib olingan muddat uchun qavat. |
| `grace_period_days` | `u16` | Standart 30. |
| `redemption_period_days` | `u16` | Standart 60. |
| `max_term_years` | `u8` | Oldindan yangilashning maksimal davomiyligi. |
| `referral_cap_bps` | `u16` | <=1000 (10%) charter. |
| `reserved_labels` | `Vec<ReservedNameV1>` | Boshqaruv topshiriqlar bo'yicha ko'rsatmalar bilan ta'minlangan ro'yxat. |
| `fee_split` | `SuffixFeeSplitV1` | G'aznachilik / boshqaruvchi / yo'naltiruvchi aktsiyalari (asosiy ball). |
| `fund_splitter_account` | `AccountId` | Escrow + pul mablag'larini taqsimlaydigan hisob qaydnomasi. |
| `policy_version` | `u16` | Har bir o'zgarishda oshiriladi. |
| `metadata` | `Metadata` | Kengaytirilgan eslatmalar (KPI kelishuvi, muvofiqlik hujjatlari xeshlari). |

```text
Struct PriceTierV1 {
    tier_id: u8,
    label_regex: String,       // RE2-syntax pattern describing eligible labels
    base_price: TokenValue,    // Price per one-year term before suffix coefficient
    auction_kind: AuctionKind, // Default auction when the tier triggers
    dutch_floor: Option<TokenValue>,
    min_duration_years: u8,
    max_duration_years: u8,
}

Struct ReservedNameV1 {
    normalized_label: AsciiString,
    assigned_to: Option<AccountId>,
    release_at_ms: Option<u64>,
    note: String,
}

Struct SuffixFeeSplitV1 {
    treasury_bps: u16,     // default 7000 (70%)
    steward_bps: u16,      // default 3000 (30%)
    referral_max_bps: u16, // optional referral carve-out (<= 1000)
    escrow_bps: u16,       // % routed to claw-back escrow
}
```

### 2.3 Daromad va hisob-kitob yozuvlari

| Struktura | Maydonlar | Maqsad |
|--------|--------|---------|
| `RevenueShareRecordV1` | `suffix_id`, `epoch_id`, `treasury_amount`, `steward_amount`, `referral_amount`, `escrow_amount`, I18NI000001110, I18NI0000011100. | Hisob-kitob davri uchun yo'naltirilgan to'lovlarning deterministik yozuvi (haftalik). |
| `RevenueAccrualEventV1` | `name_hash`, `suffix_id`, `event`, `gross_amount`, `net_amount`, `referral_account`. | Har safar to'lov e'lon qilinganda chiqariladi (ro'yxatga olish, yangilash, auktsion). |

Barcha `TokenValue` maydonlari tegishli `SuffixPolicyV1` da e'lon qilingan valyuta kodi bilan Norito kanonik qattiq nuqta kodlashidan foydalanadi.

### 2.4 Registr hodisalari

Kanonik hodisalar DNS/shlyuzni avtomatlashtirish va tahlil qilish uchun takrorlash jurnalini taqdim etadi.

```text
Struct RegistryEventV1 {
    name_hash: [u8; 32],
    suffix_id: u16,
    selector: NameSelectorV1,
    version: u64,               // increments per NameRecord update
    timestamp: Timestamp,
    tx_hash: Hash,
    actor: AccountId,
    event: RegistryEventKind,
}

Enum RegistryEventKind {
    NameRegistered { expires_at: Timestamp, pricing_class: u8 },
    NameRenewed { expires_at: Timestamp, term_years: u8 },
    NameTransferred { previous_owner: AccountId, new_owner: AccountId },
    NameControllersUpdated { controller_count: u16 },
    NameFrozen(NameFrozenStateV1),
    NameUnfrozen,
    NameTombstoned(NameTombstoneStateV1),
    AuctionOpened { kind: AuctionKind },
    AuctionSettled { winning_account: AccountId, clearing_price: TokenValue },
    RevenueSharePosted { epoch_id: u64, treasury_amount: TokenValue, steward_amount: TokenValue },
    SuffixPolicyUpdated { policy_version: u16 },
}
```

Voqealar qayta oʻynaladigan jurnalga (masalan, `RegistryEvents` domeni) qoʻshilishi va shlyuz tasmalariga aks ettirilishi kerak, shunda DNS keshlari SLA doirasida yaroqsiz boʻladi.

## 3. Saqlash tartibi va indekslari

| Kalit | Tavsif |
|-----|-------------|
| `Names::<name_hash>` | `name_hash` dan `NameRecordV1` gacha bo'lgan asosiy xarita. |
| `NamesByOwner::<AccountId, suffix_id>` | Hamyon UI uchun ikkilamchi indeks (sahifalash uchun qulay). |
| `NamesByLabel::<suffix_id, normalized_label>` | Mojarolarni aniqlang, kuch deterministik qidiruvi. |
| `SuffixPolicies::<suffix_id>` | Oxirgi `SuffixPolicyV1`. |
| `RevenueShare::<suffix_id, epoch_id>` | `RevenueShareRecordV1` tarixi. |
| `RegistryEvents::<u64>` | Faqat qo'shish uchun jurnal monoton ravishda ortib borayotgan ketma-ketlik bilan kalitlangan. |

Barcha kalitlar Norito kortejlari yordamida xostlarda deterministik xeshlashni davom ettiradi. Indeks yangilanishi birlamchi yozuv bilan birga atomik ravishda sodir bo'ladi.

## 4. Yashash davri holati mashinasi

| Davlat | Kirish shartlari | Ruxsat etilgan o'tishlar | Eslatmalar |
|-------|-----------------|--------------------|-------|
| Mavjud | `NameRecord` yo'q bo'lganda olingan. | `PendingAuction` (premium), `Active` (standart registr). | Mavjudlikni qidirish faqat indekslarni o'qiydi. |
| Kutilayotgan auktsion | `PriceTierV1.auction_kind` ≠ hech qachon yaratilgan. | `Active` (auksion hal qilinadi), `Tombstoned` (takliflar yo'q). | Auktsionlar `AuctionOpened` va `AuctionSettled` chiqaradi. |
| Faol | Roʻyxatdan oʻtish yoki yangilash muvaffaqiyatli boʻldi. | `GracePeriod`, `Frozen`, `Tombstoned`. | `expires_at` o'tishni boshqaradi. |
| Imtiyozli davr | `now > expires_at` qachon avtomatik ravishda. | `Active` (o'z vaqtida yangilash), `Redemption`, `Tombstoned`. | Standart +30 kun; hali ham hal qiladi, lekin belgilangan. |
| To'lov | `now > grace_expires_at` lekin `< redemption_expires_at`. | `Active` (kech yangilash), `Tombstoned`. | Buyruqlar jarima to'lovini talab qiladi. |
| Muzlatilgan | Boshqaruv yoki vasiyning muzlashi. | `Active` (tuzatishdan keyin), `Tombstoned`. | Kontrollerlarni uzatish yoki yangilash mumkin emas. |
| Qabr toshlari | Ixtiyoriy taslim bo'lish, doimiy nizo natijasi yoki muddati o'tgan sotib olish. | `PendingAuction` (Gollandiya qayta ochiladi) yoki qabr toshida qolmoqda. | `NameTombstoned` hodisasi sababni o'z ichiga olishi kerak. |

Shtat o'tishlari mos keladigan `RegistryEventKind` ni chiqarishi KERAK, shuning uchun quyi oqim keshlari izchil bo'lib qoladi. Gollandiyadagi qayta ochilgan auktsionlarga kiruvchi qabr toshli nomlar `AuctionKind::DutchReopen` foydali yukini biriktiradi.

## 5. Canonical Events & Gateway Sync

Shlyuzlar `RegistryEventV1` ga obuna bo'lish va DNS/SoraFS bilan sinxronlash:

1. Voqealar ketma-ketligi tomonidan havola qilingan eng oxirgi `NameRecordV1` olinmoqda.
2. Regenerator shablonlari (afzal IH58 + ikkinchi eng yaxshi siqilgan (`sora`) manzillar, matn yozuvlari).
3. Yangilangan hudud maʼlumotlarini [`soradns_registry_rfc.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/soradns/soradns_registry_rfc.md) da tasvirlangan SoraDNS ish jarayoni orqali mahkamlash.

Tadbirni yetkazib berish kafolatlari:

- `NameRecordV1` ga ta'sir etuvchi har bir tranzaksiyaga qat'iy ortib borayotgan `version` bilan aynan bitta hodisa qo'shilishi *kerak*.
- `RevenueSharePosted` voqealari `RevenueShareRecordV1` tomonidan chiqarilgan hisob-kitoblarga murojaat qiladi.
- Muzlatish/muzlatish/qabr toshlari hodisalari auditni takrorlash uchun `metadata` ichidagi boshqaruv artefakt xeshlarini o'z ichiga oladi.

## 6. Misol Norito Foydali yuklar

### 6.1 NameRecord misoli

```text
NameRecordV1 {
    suffix_id: 0x0001,                       // .sora
    selector: NameSelectorV1 { version:1, suffix_id:1, label_len:5, label_bytes:"makoto" },
    name_hash: 0x5f57...9c2a,
    normalized_label: "makoto",
    display_label: "Makoto",
    owner: "ih58...",
    controllers: [
        NameControllerV1 {
            controller_type: Account,
            account_address: Some(AccountAddress("0x02000001...")),
            resolver_template_id: None,
            payload: {}
        }
    ],
    status: Active,
    pricing_class: 0,
    registered_at: 1_776_000_000,
    expires_at: 1_807_296_000,
    grace_expires_at: 1_809_888_000,
    redemption_expires_at: 1_815_072_000,
    auction: None,
    last_tx_hash: 0xa3d4...c001,
    metadata: { "resolver": "wallet.default", "notes": "SNS beta cohort" },
}
```

### 6.2 SuffixPolicy misoli

```text
SuffixPolicyV1 {
    suffix_id: 0x0001,
    suffix: "sora",
    steward: "ih58...",
    status: Active,
    payment_asset_id: "xor#sora",
    pricing: [
        PriceTierV1 { tier_id:0, label_regex:"^[a-z0-9]{3,}$", base_price:"120 XOR", auction_kind:VickreyCommitReveal, dutch_floor:None, min_duration_years:1, max_duration_years:5 },
        PriceTierV1 { tier_id:1, label_regex:"^[a-z]{1,2}$", base_price:"10_000 XOR", auction_kind:DutchReopen, dutch_floor:Some("1_000 XOR"), min_duration_years:1, max_duration_years:3 }
    ],
    min_term_years: 1,
    grace_period_days: 30,
    redemption_period_days: 60,
    max_term_years: 5,
    referral_cap_bps: 500,
    reserved_labels: [
        ReservedNameV1 { normalized_label:"treasury", assigned_to:Some("ih58..."), release_at:None, note:"Protocol reserved" }
    ],
    fee_split: SuffixFeeSplitV1 { treasury_bps:7000, steward_bps:3000, referral_max_bps:1000, escrow_bps:500 },
    fund_splitter_account: "ih58...",
    policy_version: 3,
    metadata: { "kpi_covenant":"bafybeigd..." },
}
```

## 7. Keyingi qadamlar- **SN-2b (Registrator API va boshqaruv ilgaklari):** ushbu tuzilmalarni Torii (Norito va JSON ulanishlari) va boshqaruv artefaktlariga kirish tekshiruvlari orqali ochib bering.
- **SN-3 (Auktsion va roʻyxatga olish mexanizmi):** `NameAuctionStateV1` dan foydalanish/koʻrsatish va Gollandiyalik qayta ochish mantigʻini amalga oshirish uchun.
- **SN-5 (To'lov va hisob-kitoblar):** moliyaviy muvofiqlashtirish va hisobotlarni avtomatlashtirish uchun `RevenueShareRecordV1` leverage.

Savollar yoki oʻzgartirish soʻrovlari `roadmap.md` da SNS yoʻl xaritasi yangilanishlari bilan birga topshirilishi va birlashtirilganda `status.md` da aks ettirilishi kerak.