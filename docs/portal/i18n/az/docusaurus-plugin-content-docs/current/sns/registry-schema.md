---
id: registry-schema
lang: az
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

:::Qeyd Kanonik Mənbə
Bu səhifə `docs/source/sns/registry_schema.md`-i əks etdirir və indi kimi xidmət edir
kanonik portal nüsxəsi. Mənbə fayl tərcümə yeniləmələri üçün qalır.
:::

# Sora Adı Xidmət Reyestr Sxemi (SN-2a)

**Status:** 24-03-2026 tarixində tərtib edilib -- SNS proqramının nəzərdən keçirilməsi üçün təqdim edilib  
**Yol xəritəsi bağlantısı:** SN-2a “Reyestr sxemi və saxlama sxemi”  
**Əhatə dairəsi:** Sora Ad Xidməti (SNS) üçün kanonik Norito strukturlarını, həyat dövrü vəziyyətlərini və buraxılmış hadisələri müəyyən edin ki, reyestr və registrator tətbiqləri müqavilələr, SDK-lar və şlüzlər arasında deterministik olsun.

Bu sənəd SN-2a üçün çatdırıla bilən sxemi qeyd etməklə tamamlayır:

1. İdentifikatorlar və heşinq qaydaları (`SuffixId`, `NameHash`, seçicinin törəməsi).
2. Ad qeydləri, şəkilçi siyasətləri, qiymət səviyyələri, gəlir bölgüsü və reyestr hadisələri üçün Norito strukturları/saylar.
3. Deterministik təkrar oynatma üçün saxlama düzeni və indeks prefiksləri.
4. Qeydiyyat, yenilənmə, lütf/qeyri-dövlət, donma və qəbir daşlarını əhatə edən dövlət maşını.
5. DNS/gateway avtomatlaşdırılması tərəfindən istehlak edilən kanonik hadisələr.

## 1. İdentifikatorlar və Hashing

| İdentifikator | Təsvir | törəmə |
|------------|-------------|------------|
| `SuffixId` (`u16`) | Üst səviyyə şəkilçilər üçün reyestr miqyasında identifikator (`.sora`, `.nexus`, `.dao`). [`sns_suffix_governance_charter.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md) daxilində şəkilçi kataloqu ilə uyğunlaşdırılmışdır. | İdarəetmə səsverməsi ilə təyin edilir; `SuffixPolicyV1`-də saxlanılır. |
| `SuffixSelector` | Suffiksin kanonik simli forması (ASCII, kiçik hərf). | Misal: `.sora` → `sora`. |
| `NameSelectorV1` | Qeydə alınmış etiket üçün ikili seçici. | `struct NameSelectorV1 { version:u8 (=1); suffix_id:u16; label_len:u16; label_bytes:Vec<u8> }`. Etiket Normv1 üçün NFC + kiçik hərfdir. |
| `NameHash` (`[u8;32]`) | Müqavilələr, hadisələr və keşlər tərəfindən istifadə edilən əsas axtarış açarı. | `blake3(NameSelectorV1_bytes)`. |

Determinizm tələbləri:

- Etiketlər Normv1 (UTS-46 ciddi, STD3 ASCII, NFC) vasitəsilə normallaşdırılır. Daxil olan istifadəçi sətirləri hashingdən əvvəl normallaşdırılmalıdır.
- Qorunan etiketlər (`SuffixPolicyV1.reserved_labels`-dən) heç vaxt reyestrə daxil olmur; yalnız idarəetməyə aid olan ləğvetmələr `ReservedNameAssigned` hadisələrini yayır.

## 2. Norito Strukturlar

### 2.1 Ad QeydiV1

| Sahə | Növ | Qeydlər |
|-------|------|-------|
| `suffix_id` | `u16` | İstinadlar `SuffixPolicyV1`. |
| `selector` | `NameSelectorV1` | Audit/debuq üçün xam seçici bayt. |
| `name_hash` | `[u8; 32]` | Xəritələr/hadisələr üçün açar. |
| `normalized_label` | `AsciiString` | İnsan tərəfindən oxuna bilən etiket (Normv1-dən sonra). |
| `display_label` | `AsciiString` | Mühafizəçi tərəfindən təmin edilən korpus; isteğe bağlı kosmetika. |
| `owner` | `AccountId` | Yenilənmələrə/köçürmələrə nəzarət edir. |
| `controllers` | `Vec<NameControllerV1>` | İstinadlar hesab ünvanlarını, həllediciləri və ya tətbiq metadatasını hədəfləyir. |
| `status` | `NameStatus` | Həyat dövrü bayrağı (bax. Bölmə 4). |
| `pricing_class` | `u8` | Suffiks qiymət səviyyələrinə indeks (standart, mükafat, qorunan). |
| `registered_at` | `Timestamp` | İlkin aktivləşdirmənin vaxt damğasını bloklayın. |
| `expires_at` | `Timestamp` | Ödənişli müddətin sonu. |
| `grace_expires_at` | `Timestamp` | Avtomatik yenilənmə güzəştinin sonu (defolt +30 gün). |
| `redemption_expires_at` | `Timestamp` | Ödəniş pəncərəsinin sonu (defolt +60 gün). |
| `auction` | `Option<NameAuctionStateV1>` | Hollandiya yenidən açıldıqda və ya premium auksionlar aktiv olduqda təqdim olunur. |
| `last_tx_hash` | `Hash` | Bu versiyanı yaradan əməliyyat üçün deterministik göstərici. |
| `metadata` | `Metadata` | Özbaşına registrator metadata (mətn qeydləri, sübutlar). |

Dəstəkləyici strukturlar:

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

| Sahə | Növ | Qeydlər |
|-------|------|-------|
| `suffix_id` | `u16` | Əsas açar; siyasət versiyaları arasında sabitdir. |
| `suffix` | `AsciiString` | məsələn, `sora`. |
| `steward` | `AccountId` | Stüard idarəetmə nizamnaməsində müəyyən edilmişdir. |
| `status` | `SuffixStatus` | `Active`, `Paused`, `Revoked`. |
| `payment_asset_id` | `AsciiString` | Defolt hesablaşma aktivi identifikatoru (məsələn, `xor#sora`). |
| `pricing` | `Vec<PriceTierV1>` | Səviyyəli qiymət əmsalları və müddət qaydaları. |
| `min_term_years` | `u8` | Döşəmə rüsumlarından asılı olmayaraq alınmış müddət üçün. |
| `grace_period_days` | `u16` | Defolt 30. |
| `redemption_period_days` | `u16` | Defolt 60. |
| `max_term_years` | `u8` | Maksimum ilkin yeniləmə müddəti. |
| `referral_cap_bps` | `u16` | Hər çarter üçün <=1000 (10%). |
| `reserved_labels` | `Vec<ReservedNameV1>` | İdarəetmə, tapşırıq təlimatları ilə təmin edilmiş siyahı. |
| `fee_split` | `SuffixFeeSplitV1` | Xəzinədarlıq / stüard / yönləndirmə səhmləri (əsas nöqtələr). |
| `fund_splitter_account` | `AccountId` | Emanet + vəsaitləri paylayan hesab. |
| `policy_version` | `u16` | Hər dəyişiklikdə artır. |
| `metadata` | `Metadata` | Genişləndirilmiş qeydlər (KPI müqaviləsi, uyğunluq sənədinin hashləri). |

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

### 2.3 Gəlir və Hesablaşma Qeydləri

| Struktur | Sahələr | Məqsəd |
|--------|--------|---------|
| `RevenueShareRecordV1` | `suffix_id`, `epoch_id`, `treasury_amount`, `steward_amount`, `referral_amount`, `escrow_amount`, I18NI000001130, I18NI000001130. | Hesablaşma dövrü üzrə marşrutlaşdırılmış ödənişlərin müəyyənedici qeydi (həftəlik). |
| `RevenueAccrualEventV1` | `name_hash`, `suffix_id`, `event`, `gross_amount`, `net_amount`, `referral_account`. | Hər dəfə ödəniş postları (qeydiyyat, yeniləmə, auksion) verilir. |

Bütün `TokenValue` sahələri əlaqəli `SuffixPolicyV1`-də elan edilmiş valyuta kodu ilə Norito-in kanonik sabit nöqtə kodlaşdırmasından istifadə edir.

### 2.4 Qeydiyyat hadisələri

Kanonik hadisələr DNS/gateway avtomatlaşdırılması və analitika üçün təkrar qeydlər təqdim edir.

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

Tədbirlər təkrar oxuna bilən jurnala (məsələn, `RegistryEvents` domeni) əlavə edilməli və şlüz lentlərinə əks olunmalıdır ki, DNS keşləri SLA daxilində etibarsız olsun.

## 3. Storage Layout & Indexes

| Açar | Təsvir |
|-----|-------------|
| `Names::<name_hash>` | `name_hash`-dən `NameRecordV1`-ə qədər əsas xəritə. |
| `NamesByOwner::<AccountId, suffix_id>` | Pul kisəsi UI üçün ikinci dərəcəli indeks (səhifələşdirməyə uyğundur). |
| `NamesByLabel::<suffix_id, normalized_label>` | Münaqişələri aşkar edin, güc deterministik axtarışı. |
| `SuffixPolicies::<suffix_id>` | Ən son `SuffixPolicyV1`. |
| `RevenueShare::<suffix_id, epoch_id>` | `RevenueShareRecordV1` tarixçəsi. |
| `RegistryEvents::<u64>` | Monoton artan ardıcıllıqla əsaslanan yalnız əlavələr üçün jurnal. |

Bütün açarlar hostlar arasında deterministik heshing saxlamaq üçün Norito dəstlərindən istifadə edərək seriallaşdırılır. İndeks yeniləmələri əsas qeydlə yanaşı atomik şəkildə baş verir.

## 4. Lifecycle State Machine

| Dövlət | Giriş Şərtləri | İcazə verilən keçidlər | Qeydlər |
|-------|-----------------|--------------------|-------|
| Mövcud | `NameRecord` olmadıqda əldə edilmişdir. | `PendingAuction` (mükafat), `Active` (standart registr). | Mövcudluq axtarışı yalnız indeksləri oxuyur. |
| Gözləyən Hərrac | `PriceTierV1.auction_kind` ≠ heç biri olduqda yaradılmışdır. | `Active` (hərrac həll olunur), `Tombstoned` (təklif yoxdur). | Hərraclar `AuctionOpened` və `AuctionSettled` yayır. |
| Aktiv | Qeydiyyat və ya yeniləmə uğurlu oldu. | `GracePeriod`, `Frozen`, `Tombstoned`. | `expires_at` keçidi idarə edir. |
| GracePeriod | `now > expires_at` olduqda avtomatik olaraq. | `Active` (vaxtında yenilənmə), `Redemption`, `Tombstoned`. | Defolt +30 gün; hələ də həll edir, lakin qeyd olunur. |
| Satınalma | `now > grace_expires_at`, lakin `< redemption_expires_at`. | `Active` (gec yenilənmə), `Tombstoned`. | Əmrlər üçün cərimə tələb olunur. |
| Dondurulmuş | İdarəetmə və ya qəyyum dondurulur. | `Active` (təmirdən sonra), `Tombstoned`. | Kontrollerləri ötürmək və ya yeniləmək mümkün deyil. |
| Qəbirüstü | Könüllü təslim, daimi mübahisənin nəticəsi və ya vaxtı keçmiş geri qaytarma. | `PendingAuction` (Hollandiya yenidən açılır) və ya məzar daşı kimi qalır. | `NameTombstoned` hadisəsi səbəb ehtiva etməlidir. |

Dövlət keçidləri müvafiq `RegistryEventKind` yaymalıdır ki, aşağı axın keşləri ardıcıl qalsın. Hollandiyanın yenidən açılan auksionlarına daxil olan məzar daşı ilə örtülmüş adlar `AuctionKind::DutchReopen` yükünü əlavə edir.

## 5. Canonical Events & Gateway Sync

Şlüzlər `RegistryEventV1`-ə abunə olun və DNS/SoraFS ilə sinxronizasiya edin:

1. Hadisə ardıcıllığı ilə istinad edilən ən son `NameRecordV1` götürülür.
2. Regenerasiya həlledici şablonları (üstünlük verilir I105 + ikinci ən yaxşı sıxılmış (`sora`) ünvanlar, mətn qeydləri).
3. [`soradns_registry_rfc.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/soradns/soradns_registry_rfc.md)-də təsvir edilən SoraDNS iş axını vasitəsilə yenilənmiş zona məlumatlarının bərkidilməsi.

Tədbirin çatdırılmasına zəmanət:

- `NameRecordV1`-ə təsir edən hər tranzaksiya `version` ilə tam olaraq bir hadisə əlavə etməlidir.
- `RevenueSharePosted` hadisələri `RevenueShareRecordV1` tərəfindən buraxılan hesablaşmalara istinad edir.
- Dondurma/açma/qəbir daşı hadisələri auditin təkrarı üçün `metadata` daxilində idarəetmə artefaktı heşlərini əhatə edir.

## 6. Nümunə Norito Faydalı Yüklər

### 6.1 Ad Qeydi Nümunəsi

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

### 6.2 SuffixPolicy Nümunəsi

```text
SuffixPolicyV1 {
    suffix_id: 0x0001,
    suffix: "sora",
    steward: "i105...",
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
        ReservedNameV1 { normalized_label:"treasury", assigned_to:Some("i105..."), release_at:None, note:"Protocol reserved" }
    ],
    fee_split: SuffixFeeSplitV1 { treasury_bps:7000, steward_bps:3000, referral_max_bps:1000, escrow_bps:500 },
    fund_splitter_account: "i105...",
    policy_version: 3,
    metadata: { "kpi_covenant":"bafybeigd..." },
}
```

## 7. Növbəti addımlar- **SN-2b (Registrator API və idarəetmə qarmaqları):** bu strukturları Torii (Norito və JSON bağlamaları) və idarəetmə artefaktlarına tel qəbulu yoxlamaları vasitəsilə ifşa edin.
- **SN-3 (Auksion və qeydiyyat mühərriki):** öhdəliyi yerinə yetirmək/aşkar etmək və Hollandiyanın yenidən açılması məntiqini həyata keçirmək üçün `NameAuctionStateV1`-dən yenidən istifadə edin.
- **SN-5 (Ödəniş və hesablaşma):** maliyyə uzlaşması və hesabatın avtomatlaşdırılması üçün `RevenueShareRecordV1` leverage.

Suallar və ya dəyişiklik sorğuları `roadmap.md`-də SNS yol xəritəsi yeniləmələri ilə yanaşı verilməli və birləşdirildikdə `status.md`-də əks etdirilməlidir.