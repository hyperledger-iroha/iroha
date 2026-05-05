---
lang: ka
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

:::შენიშვნა კანონიკური წყარო
ეს გვერდი ასახავს `docs/source/sns/registry_schema.md` და ახლა ემსახურება როგორც
კანონიკური პორტალის ასლი. წყაროს ფაილი რჩება თარგმანის განახლებისთვის.
:::

# Sora Name Service Registry Schema (SN-2a)

** სტატუსი: ** შედგენილია 2026-03-24 -- წარდგენილია SNS პროგრამის განსახილველად  
**საგზაო რუკის ბმული:** SN-2a „რეგისტრის სქემა და შენახვის განლაგება“  
**ფარგლები:** განსაზღვრეთ კანონიკური Norito სტრუქტურები, სასიცოცხლო ციკლის მდგომარეობები და ემიტირებული მოვლენები Sora Name Service (SNS)-ისთვის, რათა რეესტრისა და რეგისტრატორის იმპლემენტაციები დარჩეს განმსაზღვრელი კონტრაქტებში, SDK-ებსა და კარიბჭეებში.

ეს დოკუმენტი ავსებს SN-2a-სთვის მიწოდების სქემას მითითებით:

1. იდენტიფიკატორები და ჰეშირების წესები (`SuffixId`, `NameHash`, სელექტორის დერივაცია).
2. Norito სტრუქტურები/რიცხვები სახელების ჩანაწერებისთვის, სუფიქსის პოლიტიკისთვის, ფასების რიგებისთვის, შემოსავლების გაყოფისა და რეესტრის მოვლენებისთვის.
3. შენახვის განლაგება და ინდექსის პრეფიქსები დეტერმინისტული გამეორებისთვის.
4. სახელმწიფო მანქანა, რომელიც მოიცავს რეგისტრაციას, განახლებას, მადლს/გამოსყიდვას, ყინვებს და საფლავის ქვებს.
5. DNS/gateway ავტომატიზაციის მიერ მოხმარებული კანონიკური მოვლენები.

## 1. იდენტიფიკატორები და ჰეშინგი

| იდენტიფიკატორი | აღწერა | დერივაცია |
|------------|-------------|------------|
| `SuffixId` (`u16`) | რეესტრის მასშტაბით იდენტიფიკატორი ზედა დონის სუფიქსებისთვის (`.sora`, `.nexus`, `.dao`). გასწორებულია სუფიქსის კატალოგთან [`sns_suffix_governance_charter.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md). | დანიშნულია მმართველობის კენჭისყრით; ინახება `SuffixPolicyV1`-ში. |
| `SuffixSelector` | სუფიქსის კანონიკური სიმებიანი ფორმა (ASCII, მცირე რეგისტრი). | მაგალითი: `.sora` → `sora`. |
| `NameSelectorV1` | ორობითი სელექტორი რეგისტრირებული ეტიკეტისთვის. | `struct NameSelectorV1 { version:u8 (=1); suffix_id:u16; label_len:u16; label_bytes:Vec<u8> }`. ლეიბლი არის NFC + პატარა რეგისტრები Normv1-ზე. |
| `NameHash` (`[u8;32]`) | ძირითადი საძიებო გასაღები გამოიყენება კონტრაქტების, მოვლენებისა და ქეშების მიერ. | `blake3(NameSelectorV1_bytes)`. |

დეტერმინიზმის მოთხოვნები:

- ლეიბლები ნორმალიზდება Normv1-ის საშუალებით (UTS-46 მკაცრი, STD3 ASCII, NFC). შემომავალი მომხმარებლის სტრიქონები უნდა იყოს ნორმალიზებული ჰეშირებამდე.
- რეზერვირებული ეტიკეტები (`SuffixPolicyV1.reserved_labels`-დან) არასოდეს შედის რეესტრში; მხოლოდ მმართველობის უგულებელყოფა ასხივებს `ReservedNameAssigned` მოვლენებს.

## 2. Norito სტრუქტურები

### 2.1 NameRecordV1

| ველი | ტიპი | შენიშვნები |
|-------|------|-------|
| `suffix_id` | `u16` | მითითებები `SuffixPolicyV1`. |
| `selector` | `NameSelectorV1` | ნედლეული სელექტორი ბაიტები აუდიტის/გამართვისთვის. |
| `name_hash` | `[u8; 32]` | გასაღები რუკებისთვის/მოვლენებისთვის. |
| `normalized_label` | `AsciiString` | ადამიანისთვის წასაკითხი ეტიკეტი (პოსტ Normv1). |
| `display_label` | `AsciiString` | სტიუარდის მიერ მოწოდებული გარსაცმები; სურვილისამებრ კოსმეტიკა. |
| `owner` | `AccountId` | აკონტროლებს განახლებას/ტრანსფერებს. |
| `controllers` | `Vec<NameControllerV1>` | ცნობები მიზნად ისახავს ანგარიშის მისამართებს, გადამწყვეტებს ან აპლიკაციის მეტამონაცემებს. |
| `status` | `NameStatus` | სასიცოცხლო ციკლის დროშა (იხილეთ ნაწილი 4). |
| `pricing_class` | `u8` | ინდექსირება სუფიქსის ფასების რიგებში (სტანდარტული, პრემია, დაჯავშნილი). |
| `registered_at` | `Timestamp` | საწყისი აქტივაციის დროის ანაბეჭდის დაბლოკვა. |
| `expires_at` | `Timestamp` | ფასიანი ვადის დასრულება. |
| `grace_expires_at` | `Timestamp` | ავტომატური განახლების მადლის დასრულება (ნაგულისხმევი +30 დღე). |
| `redemption_expires_at` | `Timestamp` | გამოსყიდვის ფანჯრის დასასრული (ნაგულისხმევი +60 დღე). |
| `auction` | `Option<NameAuctionStateV1>` | წარმოადგინეთ, როდესაც ჰოლანდიის ხელახალი გახსნა ან პრემიუმ აუქციონები აქტიურია. |
| `last_tx_hash` | `Hash` | დეტერმინისტული მაჩვენებელი ტრანზაქციის შესახებ, რომელმაც შექმნა ეს ვერსია. |
| `metadata` | `Metadata` | თვითნებური რეგისტრატორის მეტამონაცემები (ტექსტური ჩანაწერები, მტკიცებულებები). |

დამხმარე სტრუქტურები:

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

### 2.2 სუფიქსიპოლიტიკაV1

| ველი | ტიპი | შენიშვნები |
|-------|------|-------|
| `suffix_id` | `u16` | ძირითადი გასაღები; სტაბილურია პოლიტიკის ვერსიებში. |
| `suffix` | `AsciiString` | მაგ., `sora`. |
| `steward` | `AccountId` | მმართველობის წესდებაში განსაზღვრული სტიუარდი. |
| `status` | `SuffixStatus` | `Active`, `Paused`, `Revoked`. |
| `payment_asset_id` | `AsciiString` | ნაგულისხმევი ანგარიშსწორების აქტივის იდენტიფიკატორი (მაგალითად, `61CtjvNd9T3THAR65GsMVHr82Bjc`). |
| `pricing` | `Vec<PriceTierV1>` | დონის ფასების კოეფიციენტები და ხანგრძლივობის წესები. |
| `min_term_years` | `u8` | სართული შეძენილი ვადით, განურჩევლად იარუსის გადაფარვისა. |
| `grace_period_days` | `u16` | ნაგულისხმევი 30. |
| `redemption_period_days` | `u16` | ნაგულისხმევი 60. |
| `max_term_years` | `u8` | მაქსიმალური წინა განახლების სიგრძე. |
| `referral_cap_bps` | `u16` | <=1000 (10%) თითო ჩარტერზე. |
| `reserved_labels` | `Vec<ReservedNameV1>` | მმართველობამ მიაწოდა სია დავალების ინსტრუქციებით. |
| `fee_split` | `SuffixFeeSplitV1` | ხაზინა / სტიუარდი / რეფერალური აქციები (საბაზისო ქულები). |
| `fund_splitter_account` | `AccountId` | ანგარიში, რომელიც შეიცავს ესქროს + ანაწილებს სახსრებს. |
| `policy_version` | `u16` | ყოველ ცვლილებაზე მატულობს. |
| `metadata` | `Metadata` | გაფართოებული შენიშვნები (KPI შეთანხმება, შესაბამისობის დოკუმენტის ჰეშები). |

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

### 2.3 შემოსავლების და ანგარიშსწორების ჩანაწერები

| სტრუქტურა | ველები | დანიშნულება |
|--------|-------|---------|
| `RevenueShareRecordV1` | `suffix_id`, `epoch_id`, `treasury_amount`, `steward_amount`, `referral_amount`, `escrow_amount`, I18NI000001813X. | განმსაზღვრელი გადახდების დეტერმინისტული ჩანაწერი ანგარიშსწორების ეპოქაში (ყოველკვირეული). |
| `RevenueAccrualEventV1` | `name_hash`, `suffix_id`, `event`, `gross_amount`, `net_amount`, `referral_account`. | ემიტირებული ყოველ ჯერზე გადახდის პოსტებზე (რეგისტრაცია, განახლება, აუქციონი). |

ყველა `TokenValue` ველი იყენებს Norito-ის კანონიკურ ფიქსირებული წერტილის დაშიფვრას ვალუტის კოდით დეკლარირებული ასოცირებულ `SuffixPolicyV1`-ში.

### 2.4 რეესტრის ღონისძიებები

კანონიკური მოვლენები უზრუნველყოფს გამეორების ჟურნალს DNS/კარიბჭის ავტომატიზაციისა და ანალიტიკისთვის.

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

მოვლენები უნდა დაერთოს ხელახლა დაკვრავად ჟურნალს (მაგ., `RegistryEvents` დომენი) და ასახული იყოს კარიბჭის არხებზე, რათა DNS ქეში გაუქმდეს SLA-ში.

## 3. შენახვის განლაგება და ინდექსები

| გასაღები | აღწერა |
|-----|-------------|
| `Names::<name_hash>` | ძირითადი რუკა `name_hash`-დან `NameRecordV1`-მდე. |
| `NamesByOwner::<AccountId, suffix_id>` | მეორადი ინდექსი საფულის UI-სთვის (გვერდზე მეგობრული). |
| `NamesByLabel::<suffix_id, normalized_label>` | კონფლიქტების აღმოჩენა, დეტერმინისტული ძიება. |
| `SuffixPolicies::<suffix_id>` | უახლესი `SuffixPolicyV1`. |
| `RevenueShare::<suffix_id, epoch_id>` | `RevenueShareRecordV1` ისტორია. |
| `RegistryEvents::<u64>` | მონოტონურად გაზრდილი თანმიმდევრობით ჩასმული მხოლოდ დამატებული ჟურნალი. |

ყველა კლავიშის სერიალირება ხდება Norito ტოპების გამოყენებით, რათა ჰეშირება განმსაზღვრელი იყოს ჰოსტებში. ინდექსის განახლებები ხდება ატომურად პირველად ჩანაწერთან ერთად.

## 4. სიცოცხლის ციკლის მდგომარეობის მანქანა

| სახელმწიფო | შესვლის პირობები | ნებადართული გადასვლები | შენიშვნები |
|-------|----------------|-------------------|-------|
| ხელმისაწვდომია | მიღებულია, როდესაც `NameRecord` არ არის. | `PendingAuction` (პრემიუმი), `Active` (სტანდარტული რეგისტრი). | ხელმისაწვდომობის ძიება მხოლოდ ინდექსებს კითხულობს. |
| მომლოდინე აუქციონი | შეიქმნა, როდესაც `PriceTierV1.auction_kind` ≠ არცერთი. | `Active` (აუქციონი ანაზღაურდება), `Tombstoned` (წინადადებების გარეშე). | აუქციონებზე გამოდის `AuctionOpened` და `AuctionSettled`. |
| აქტიური | რეგისტრაცია ან განახლება წარმატებით დასრულდა. | `GracePeriod`, `Frozen`, `Tombstoned`. | `expires_at` დისკების გადასვლას. |
| საშეღავათო პერიოდი | ავტომატურად, როდესაც `now > expires_at`. | `Active` (დროულად განახლება), `Redemption`, `Tombstoned`. | ნაგულისხმევი +30 დღე; მაინც წყვეტს, მაგრამ დროშით მონიშნული. |
| გამოსყიდვა | `now > grace_expires_at` მაგრამ `< redemption_expires_at`. | `Active` (გვიანდელი განახლება), `Tombstoned`. | ბრძანებები მოითხოვს ჯარიმას. |
| გაყინული | მმართველობის ან მეურვის გაყინვა. | `Active` (რემედიაციის შემდეგ), `Tombstoned`. | კონტროლერების გადაცემა ან განახლება შეუძლებელია. |
| საფლავის ქვები | ნებაყოფლობითი ჩაბარება, მუდმივი დავის შედეგი ან ვადაგასული გამოსყიდვა. | `PendingAuction` (ჰოლანდიური ხელახლა გახსნა) ან რჩება საფლავის ქვები. | მოვლენა `NameTombstoned` უნდა შეიცავდეს მიზეზს. |

მდგომარეობების გადასვლებმა უნდა გამოსცეს შესაბამისი `RegistryEventKind`, რათა ქვედა დინების ქეშები დარჩეს თანმიმდევრული. ჰოლანდიის ხელახლა გახსნილ აუქციონებზე შემოსული საფლავის ქვები ერთვის `AuctionKind::DutchReopen` დატვირთვას.

## 5. Canonical Events & Gateway Sync

გეითვეიები გამოიწერენ `RegistryEventV1`-ს და სინქრონიზდებიან DNS/SoraFS-ზე:

1. უახლესი `NameRecordV1`-ის მიღება, რომელიც მითითებულია მოვლენების თანმიმდევრობით.
2. გადამწყვეტი შაბლონების რეგენერაცია (სასურველია i105 + მეორე საუკეთესო შეკუმშული (`sora`) მისამართები, ტექსტური ჩანაწერები).
3. განახლებული ზონის მონაცემების ჩამაგრება SoraDNS სამუშაო ნაკადის მეშვეობით, რომელიც აღწერილია [`soradns_registry_rfc.md`]-ში (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/soradns/soradns_registry_rfc.md).

ღონისძიების მიწოდების გარანტია:

- ყოველი ტრანზაქცია, რომელიც გავლენას ახდენს `NameRecordV1`-ზე *უნდა* დაურთოს ზუსტად ერთ მოვლენას მკაცრად მზარდი `version`-ით.
- `RevenueSharePosted` `RevenueShareRecordV1`-ის მიერ გამოსხივებული მოვლენების მითითების დასახლებები.
- გაყინვა/გაყინვა/საფლავის ქვის ღონისძიებები მოიცავს მმართველობის არტეფაქტის ჰეშებს `metadata`-ში აუდიტის განმეორებისთვის.

## 6. მაგალითი Norito Payloads

### 6.1 NameRecord მაგალითი

```text
NameRecordV1 {
    suffix_id: 0x0001,                       // .sora
    selector: NameSelectorV1 { version:1, suffix_id:1, label_len:5, label_bytes:"makoto" },
    name_hash: 0x5f57...9c2a,
    normalized_label: "makoto",
    display_label: "Makoto",
    owner: "<i105-account-id>",
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

### 6.2 სუფიქსიპოლიტიკის მაგალითი

```text
SuffixPolicyV1 {
    suffix_id: 0x0001,
    suffix: "sora",
    steward: "<i105-account-id>",
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
        ReservedNameV1 { normalized_label:"treasury", assigned_to:Some("<i105-account-id>"), release_at:None, note:"Protocol reserved" }
    ],
    fee_split: SuffixFeeSplitV1 { treasury_bps:7000, steward_bps:3000, referral_max_bps:1000, escrow_bps:500 },
    fund_splitter_account: "<i105-account-id>",
    policy_version: 3,
    metadata: { "kpi_covenant":"bafybeigd..." },
}
```

## 7. შემდეგი ნაბიჯები- **SN-2b (რეგისტრატორის API და მმართველობის კაკვები):** გამოავლინეთ ეს სტრუქტურები Torii (Norito და JSON საკინძები) და სადენიანი დაშვების შემოწმებების მეშვეობით მმართველობის არტეფაქტებზე.
- **SN-3 (აუქციონისა და რეგისტრაციის ძრავა):** ხელახლა გამოიყენეთ `NameAuctionStateV1` ვალდებულების/გამოვლენის და ჰოლანდიური ხელახალი გახსნის ლოგიკის განსახორციელებლად.
- **SN-5 (გადახდა და ანგარიშსწორება):** ბერკეტი `RevenueShareRecordV1` ფინანსური შერიგებისა და ანგარიშგების ავტომატიზაციისთვის.

კითხვები ან ცვლილებების მოთხოვნები უნდა შეიტანოს SNS საგზაო რუქის განახლებებთან ერთად `roadmap.md`-ში და ასახული იყოს `status.md`-ში გაერთიანებისას.