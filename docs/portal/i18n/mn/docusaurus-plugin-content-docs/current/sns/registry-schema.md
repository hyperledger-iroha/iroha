---
id: registry-schema
lang: mn
direction: ltr
source: docs/portal/docs/sns/registry-schema.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Sora Name Service Registry Schema
sidebar_label: Registry schema
description: Norito data structures, lifecycle rules, and event contracts for SNS registry smart contracts (SN-2a).
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

::: Каноник эх сурвалжийг анхаарна уу
Энэ хуудас нь `docs/source/sns/registry_schema.md`-ийг толин тусгаж байгаа бөгөөд одоо энэ хуудас болж байна
каноник портал хуулбар. Эх файл нь орчуулгын шинэчлэлтэд зориулагдсан хэвээр байна.
:::

# Сора нэрийн үйлчилгээний бүртгэлийн схем (SN-2a)

**Төлөв:** 2026-03-24-ний өдөр боловсруулсан -- SNS програмыг шалгахаар илгээсэн  
**Замын зургийн холбоос:** SN-2a "Бүртгэлийн схем ба хадгалах байршил"  
**Хамрах хүрээ:** Сора Нэр Үйлчилгээний (SNS) Norito каноник бүтэц, амьдралын мөчлөгийн төлөв, ялгаруулсан үйл явдлуудыг тодорхойлсноор бүртгэлийн болон бүртгэгчийн хэрэгжилт нь гэрээ, SDK болон гарцууд дээр тодорхой байх болно.

Энэхүү баримт бичиг нь SN-2a-д зориулсан схемийг дараах байдлаар гүйцэтгэнэ.

1. Тодорхойлогч ба хэшлэх дүрмүүд (`SuffixId`, `NameHash`, сонгогчийн үүсмэл).
2. Norito нэрийн бүртгэл, дагавар бодлого, үнийн шатлал, орлогын хуваалт, бүртгэлийн үйл явдлын бүтэц/тооцуулалт.
3. Детерминист дахин тоглуулахад зориулсан хадгалах сангийн зохион байгуулалт болон индексийн угтварууд.
4. Бүртгэл, сунгалт, ивээл/гэвтэл, хөлдөөх, булшны чулууг хамарсан төрийн машин.
5. DNS/gateway автоматжуулалтын хэрэглэдэг каноник үйл явдлууд.

## 1. Тодорхойлогч & Хэш

| Тодорхойлогч | Тодорхойлолт | Гарал үүсэл |
|------------|-------------|------------|
| `SuffixId` (`u16`) | Дээд түвшний дагаваруудын бүртгэлийн өргөн танигч (`.sora`, `.nexus`, `.dao`). [`sns_suffix_governance_charter.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md) дахь дагавар каталогтой зэрэгцүүлсэн. | Засаглалын саналаар томилогдсон; `SuffixPolicyV1`-д хадгалагдсан. |
| `SuffixSelector` | Дагаврын каноник мөр хэлбэр (ASCII, жижиг үсгээр). | Жишээ нь: `.sora` → `sora`. |
| `NameSelectorV1` | Бүртгэгдсэн шошгоны хоёртын сонгогч. | `struct NameSelectorV1 { version:u8 (=1); suffix_id:u16; label_len:u16; label_bytes:Vec<u8> }`. Шошго нь NFC + Normv1-ийн жижиг үсгээр байна. |
| `NameHash` (`[u8;32]`) | Гэрээ, үйл явдал, кэш ашигладаг үндсэн хайлтын түлхүүр. | `blake3(NameSelectorV1_bytes)`. |

Детерминизмд тавигдах шаардлага:

- Шошгуудыг Normv1 (UTS-46 хатуу, STD3 ASCII, NFC) -ээр хэвийн болгодог. Ирж буй хэрэглэгчийн мөрүүдийг хэшлэхээс өмнө ЗААВАЛ хэвийн болгох хэрэгтэй.
- Хадгалагдсан шошго (`SuffixPolicyV1.reserved_labels`-ээс) бүртгэлд хэзээ ч орохгүй; зөвхөн засаглалыг хүчингүй болгох нь `ReservedNameAssigned` үйл явдлыг ялгаруулдаг.

## 2. Norito бүтэц

### 2.1 Нэрийн бичлэгV1

| Талбай | Төрөл | Тэмдэглэл |
|-------|------|-------|
| `suffix_id` | `u16` | Лавлагаа `SuffixPolicyV1`. |
| `selector` | `NameSelectorV1` | Аудит/дибаг хийх түүхий сонгогч байт. |
| `name_hash` | `[u8; 32]` | Газрын зураг/үйл явдлын түлхүүр. |
| `normalized_label` | `AsciiString` | Хүн унших боломжтой шошго (Normv1-ийн бичлэг). |
| `display_label` | `AsciiString` | Удирдагчаар хангагдсан бүрхүүл; нэмэлт гоо сайхны бүтээгдэхүүн. |
| `owner` | `AccountId` | Шинэчлэлт/шилжилтийг хянадаг. |
| `controllers` | `Vec<NameControllerV1>` | Зорилтот дансны хаяг, шийдвэрлэгч эсвэл програмын мета өгөгдлийн лавлагаа. |
| `status` | `NameStatus` | Амьдралын мөчлөгийн туг (4-р хэсгийг үзнэ үү). |
| `pricing_class` | `u8` | Үнийн давхаргын индекс (стандарт, дээд зэрэглэлийн, нөөцлөгдсөн). |
| `registered_at` | `Timestamp` | Анхны идэвхжүүлэлтийн хугацааг блоклох. |
| `expires_at` | `Timestamp` | Төлбөрийн хугацаа дуусах. |
| `grace_expires_at` | `Timestamp` | Автоматаар сунгах хөнгөлөлтийн төгсгөл (өгөгдмөл +30 хоног). |
| `redemption_expires_at` | `Timestamp` | Эргүүлэх цонхны төгсгөл (өгөгдмөл +60 хоног). |
| `auction` | `Option<NameAuctionStateV1>` | Голланд дахин нээх эсвэл дээд зэрэглэлийн дуудлага худалдаа идэвхтэй байх үед. |
| `last_tx_hash` | `Hash` | Энэ хувилбарыг үүсгэсэн гүйлгээний тодорхойлогч заагч. |
| `metadata` | `Metadata` | Дурын бүртгэгчийн мета өгөгдөл (текст бичлэг, нотолгоо). |

Туслах бүтэц:

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

| Талбай | Төрөл | Тэмдэглэл |
|-------|------|-------|
| `suffix_id` | `u16` | Үндсэн түлхүүр; бодлогын хувилбаруудад тогтвортой. |
| `suffix` | `AsciiString` | жишээ нь, `sora`. |
| `steward` | `AccountId` | Удирдах ажилтныг засаглалын дүрэмд тодорхойлсон. |
| `status` | `SuffixStatus` | `Active`, `Paused`, `Revoked`. |
| `payment_asset_id` | `AsciiString` | Өгөгдмөл төлбөр тооцооны хөрөнгийн тодорхойлогч (жишээ нь `61CtjvNd9T3THAR65GsMVHr82Bjc`). |
| `pricing` | `Vec<PriceTierV1>` | Үнийн шаталсан коэффициент ба үргэлжлэх хугацааны дүрэм. |
| `min_term_years` | `u8` | Шалны давхрагаас үл хамааран худалдан авсан хугацаатай шал. |
| `grace_period_days` | `u16` | Өгөгдмөл 30. |
| `redemption_period_days` | `u16` | Өгөгдмөл 60. |
| `max_term_years` | `u8` | Урьдчилан сунгах хамгийн дээд хугацаа. |
| `referral_cap_bps` | `u16` | Дүрэм бүрт <=1000 (10%). |
| `reserved_labels` | `Vec<ReservedNameV1>` | Даалгаврын заавар бүхий жагсаалтыг засаглалаас ирүүлсэн. |
| `fee_split` | `SuffixFeeSplitV1` | Төрийн сан / нярав / лавлагаа хувьцаа (суурь оноо). |
| `fund_splitter_account` | `AccountId` | Эскроу + мөнгө тараадаг данс. |
| `policy_version` | `u16` | Өөрчлөлт бүрт нэмэгддэг. |
| `metadata` | `Metadata` | Өргөтгөсөн тэмдэглэл (KPI гэрээ, дагаж мөрдөх баримт бичгийн хэш). |

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

### 2.3 Орлого ба тооцооны бүртгэл

| Бүтэц | Талбарууд | Зорилго |
|--------|--------|---------|
| `RevenueShareRecordV1` | `suffix_id`, `epoch_id`, `treasury_amount`, `steward_amount`, `referral_amount`, `escrow_amount`, I18NI0000011300, I18NI0000011300. | Төлбөр тооцооны эрин үе дэх чиглүүлэлтийн төлбөрийн тодорхойлогч бүртгэл (долоо хоног бүр). |
| `RevenueAccrualEventV1` | `name_hash`, `suffix_id`, `event`, `gross_amount`, `net_amount`, `referral_account`. | Төлбөр байршуулах бүрт (бүртгүүлэх, сунгах, дуудлага худалдаа) гаргадаг. |

Бүх `TokenValue` талбарууд нь холбогдох `SuffixPolicyV1`-д зарласан валютын кодтой Norito-ийн каноник тогтмол цэгийн кодчилолыг ашигладаг.

### 2.4 Бүртгэлийн үйл явдал

Каноник үйл явдлууд нь DNS/gateway автоматжуулалт болон аналитикийн дахин тоглуулах бүртгэлийг өгдөг.

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

Үйл явдлуудыг дахин тоглуулах боломжтой бүртгэлд (жишээ нь, `RegistryEvents` домэйн) хавсаргаж, гарцын хангамжид тусгах ёстой бөгөөд ингэснээр DNS кэш нь SLA дотор хүчингүй болно.

## 3. Хадгалалтын байршил ба индексүүд

| Түлхүүр | Тодорхойлолт |
|-----|-------------|
| `Names::<name_hash>` | `name_hash`-аас `NameRecordV1` хүртэлх үндсэн газрын зураг. |
| `NamesByOwner::<AccountId, suffix_id>` | Түрийвчний UI-д зориулсан хоёрдогч индекс (хуудас бичихэд ээлтэй). |
| `NamesByLabel::<suffix_id, normalized_label>` | Зөрчилдөөнийг илрүүлэх, хүчийг тодорхойлох хайлт. |
| `SuffixPolicies::<suffix_id>` | Хамгийн сүүлийн үеийн `SuffixPolicyV1`. |
| `RevenueShare::<suffix_id, epoch_id>` | `RevenueShareRecordV1` түүх. |
| `RegistryEvents::<u64>` | Зөвхөн хавсаргах лог. |

Бүх түлхүүрүүд нь Norito tuple-уудыг ашиглан серверүүд дээр хэшийг тодорхойлогч байлгахын тулд цуваа болгодог. Индекс шинэчлэлтүүд нь үндсэн бичлэгийн зэрэгцээ атомаар явагддаг.

## 4. Амьдралын мөчлөгийн төлөвийн машин

| Төрийн | Элсэлтийн нөхцөл | Зөвшөөрөгдсөн шилжилтүүд | Тэмдэглэл |
|-------|-----------------|--------------------|-------|
| Боломжтой | `NameRecord` байхгүй үед үүссэн. | `PendingAuction` (дээд зэрэглэлийн), `Active` (стандарт бүртгэл). | Боломжтой байдлын хайлт нь зөвхөн индексүүдийг уншдаг. |
| Хүлээгдэж буй дуудлага худалдаа | `PriceTierV1.auction_kind` ≠ байхгүй үед үүсгэгдсэн. | `Active` (дуудлага худалдаагаар шийднэ), `Tombstoned` (үнэ саналгүй). | Дуудлага худалдаа нь `AuctionOpened` болон `AuctionSettled` ялгаруулдаг. |
| Идэвхтэй | Бүртгэл эсвэл сунгалт амжилттай боллоо. | `GracePeriod`, `Frozen`, `Tombstoned`. | `expires_at` шилжилтийг удирддаг. |
| Grace Period | `now > expires_at` үед автоматаар. | `Active` (цагтаа шинэчлэх), `Redemption`, `Tombstoned`. | Өгөгдмөл +30 хоног; шийдсэн хэвээр байгаа боловч дарцагласан. |
| гэтэлгэл | `now > grace_expires_at` гэхдээ `< redemption_expires_at`. | `Active` (хожуу шинэчлэгдсэн), `Tombstoned`. | Тушаал нь торгуулийн хураамж шаарддаг. |
| Хөлдөөсөн | Засаглал эсвэл асран хамгаалагч хөлддөг. | `Active` (засвар хийсний дараа), `Tombstoned`. | Хянагчийг шилжүүлэх эсвэл шинэчлэх боломжгүй. |
| Булшны чулуу | Сайн дураараа бууж өгөх, байнгын маргааны үр дүн эсвэл хугацаа нь дууссан гэтэлгэл. | `PendingAuction` (Голланд дахин нээгдсэн) эсвэл булшны чулуун хэвээр байна. | `NameTombstoned` үйл явдал нь шалтгааныг агуулсан байх ёстой. |

Төлөвийн шилжилтүүд нь харгалзах `RegistryEventKind`-г ялгаруулах ЗААВАЛ гарах тул доод урсгалын кэшүүд уялдаатай хэвээр байна. Голландын дахин нээлттэй дуудлага худалдаанд орж буй булшны нэрс нь `AuctionKind::DutchReopen` ачааг хавсаргасан.

## 5. Canonical Events & Gateway Sync

Гарцууд нь `RegistryEventV1`-д бүртгүүлж, DNS/SoraFS-тэй синк хийнэ:

1. Үйл явдлын дарааллаар лавласан хамгийн сүүлийн үеийн `NameRecordV1`-г татаж байна.
2. Шийдвэрлэгчийн загваруудыг сэргээх (I105 + хоёр дахь хамгийн сайн шахсан (`sora`) хаягууд, текст бичлэгүүд).
3. [`soradns_registry_rfc.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/soradns/soradns_registry_rfc.md)-д тайлбарласан SoraDNS ажлын урсгалаар дамжуулан шинэчилсэн бүсийн өгөгдлийг бэхлэх.

Үйл явдлыг хүргэх баталгаа:

- `NameRecordV1`-д нөлөөлж буй гүйлгээ бүр нь `version`-тай яг нэг үйл явдал нэмэх *заавал*.
- `RevenueSharePosted` үйл явдал `RevenueShareRecordV1`-ийн ялгаруулсан тооцооны лавлагаа.
- Хөлдөөх/таслах/булшны чулуун үйл явдлуудад аудитын дахин тоглуулах зорилгоор `metadata` доторх засаглалын олдворуудын хэшүүд багтана.

## 6. Жишээ Norito Ачаалах ачаалал

### 6.1 Нэрийн бичлэгийн жишээ

```text
NameRecordV1 {
    suffix_id: 0x0001,                       // .sora
    selector: NameSelectorV1 { version:1, suffix_id:1, label_len:5, label_bytes:"makoto" },
    name_hash: 0x5f57...9c2a,
    normalized_label: "makoto",
    display_label: "Makoto",
    owner: "i105...",
    controllers: [
        NameControllerV1 {
            controller_type: Account,
            account_address: Some(AccountAddress("0x020001...")),
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

### 6.2 SuffixPolicy жишээ

```text
SuffixPolicyV1 {
    suffix_id: 0x0001,
    suffix: "sora",
    steward: "i105...",
    status: Active,
    payment_asset_id: "61CtjvNd9T3THAR65GsMVHr82Bjc",
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
        ReservedNameV1 { normalized_label:"treasury", assigned_to:Some("i105..."), release_at:None, note:"Protocol reserved" }
    ],
    fee_split: SuffixFeeSplitV1 { treasury_bps:7000, steward_bps:3000, referral_max_bps:1000, escrow_bps:500 },
    fund_splitter_account: "i105...",
    policy_version: 3,
    metadata: { "kpi_covenant":"bafybeigd..." },
}
```

## 7. Дараагийн алхамууд- **SN-2b (Бүртгэгч API ба засаглалын дэгээ):** эдгээр бүтцийг Torii (Norito болон JSON холболтууд) болон засаглалын олдворууд руу нэвтрэх шалгалтаар ил болгоно.
- **SN-3 (Дуудлага худалдаа, бүртгэлийн систем):** `NameAuctionStateV1`-г дахин ашиглах, гүйцэтгэх/илчлэх, Голландын дахин нээх логикийг хэрэгжүүлэх.
- **SN-5 (Төлбөр тооцоо):** санхүүгийн зохицуулалт болон тайлангийн автоматжуулалтад зориулсан `RevenueShareRecordV1` хөшүүрэг.

Асуулт эсвэл өөрчлөлтийн хүсэлтийг `roadmap.md`-д SNS замын газрын зургийн шинэчлэлтийн хажууд гаргаж, нэгтгэх үед `status.md`-д тусгана.