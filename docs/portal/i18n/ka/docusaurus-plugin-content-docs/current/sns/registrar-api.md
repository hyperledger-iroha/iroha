---
id: registrar-api
lang: ka
direction: ltr
source: docs/portal/docs/sns/registrar-api.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Sora Name Service Registrar API & Governance Hooks
sidebar_label: Registrar API
description: Torii REST/gRPC surfaces, Norito DTOs, and governance artifacts for SNS registrations (SN-2b).
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::შენიშვნა კანონიკური წყარო
ეს გვერდი ასახავს `docs/source/sns/registrar_api.md`-ს და ახლა ემსახურება როგორც
კანონიკური პორტალის ასლი. წყარო ფაილი რჩება თარგმანის სამუშაო პროცესებისთვის.
:::

# SNS Registrar API & Governance Hooks (SN-2b)

** სტატუსი: ** შედგენილია 2026-03-24 -- Nexus Core მიმოხილვის ქვეშ  
**საგზაო რუკის ბმული:** SN-2b „რეგისტრატორის API და მმართველობის კაკვები“  
**წინაპირობები:** სქემის განმარტებები [`registry-schema.md`](./registry-schema.md)

ეს შენიშვნა განსაზღვრავს Torii საბოლოო წერტილებს, gRPC სერვისებს, მოთხოვნის/პასუხის DTO-ებს და მმართველობის არტეფაქტებს, რომლებიც საჭიროა Sora Name Service (SNS) რეგისტრატორის მუშაობისთვის. ეს არის ავტორიტეტული კონტრაქტი SDK-ებისთვის, საფულეებისთვის და ავტომატიზაციისთვის, რომლებსაც სჭირდებათ SNS სახელების რეგისტრაცია, განახლება ან მართვა.

## 1. ტრანსპორტი და ავთენტიფიკაცია

| მოთხოვნა | დეტალი |
|-------------|--------|
| ოქმები | დაისვენეთ `/v2/sns/*` და gRPC სერვისით `sns.v1.Registrar`. ორივე იღებს Norito-JSON (`application/json`) და Norito-RPC ორობით (`application/x-norito`). |
| ავტორიზაცია | `Authorization: Bearer` ჟეტონები ან mTLS სერთიფიკატები გაცემული სუფიქსის სტიუარდზე. მმართველობისადმი მგრძნობიარე საბოლოო წერტილები (გაყინვა/გაყინვა, დაჯავშნილი დავალებები) მოითხოვს `scope=sns.admin`. |
| განაკვეთის ლიმიტები | რეგისტრატორები იზიარებენ `torii.preauth_scheme_limits` თაიგულებს JSON აბონენტებთან პლუს თითო სუფიქსის ადიდებული ქუდები: `sns.register`, `sns.renew`, `sns.controller`, I18NI000000042X. |
| ტელემეტრია | Torii ავლენს `torii_request_duration_seconds{scheme}` / `torii_request_failures_total{scheme,code}` რეგისტრატორის დამმუშავებლებისთვის (ფილტრი `scheme="norito_rpc"`-ზე); API ასევე ზრდის `sns_registrar_status_total{result, suffix_id}`. |

## 2. DTO მიმოხილვა

ველები მიმართავს [`registry-schema.md`]-ში (./registry-schema.md) განსაზღვრულ კანონიკურ სტრუქტურებს. ყველა ტვირთამწეობა ჩაშენებულია `NameSelectorV1` + `SuffixId` ორაზროვანი მარშრუტის თავიდან ასაცილებლად.

```text
Struct RegisterNameRequestV1 {
    selector: NameSelectorV1,
    owner: AccountId,
    controllers: Vec<NameControllerV1>,
    term_years: u8,                     // 1..=max_term_years
    pricing_class_hint: Option<u8>,     // steward-advertised tier id
    payment: PaymentProofV1,
    governance: GovernanceHookV1,
    metadata: Metadata,
}

Struct RegisterNameResponseV1 {
    name_record: NameRecordV1,
    registry_event: RegistryEventV1,
    revenue_accrual: RevenueAccrualEventV1,
}

Struct PaymentProofV1 {
    asset_id: AssetId,
    gross_amount: TokenValue,
    net_amount: TokenValue,
    settlement_tx: Hash,
    payer: AccountId,
    signature: Signature,               // steward/treasury cosign
}

Struct GovernanceHookV1 {
    proposal_id: String,
    council_vote_hash: Hash,
    dao_vote_hash: Hash,
    steward_ack: Signature,
    guardian_clearance: Option<Signature>,
}

Struct RenewNameRequestV1 {
    selector: NameSelectorV1,
    term_years: u8,
    payment: PaymentProofV1,
}

Struct TransferNameRequestV1 {
    selector: NameSelectorV1,
    new_owner: AccountId,
    governance: GovernanceHookV1,
}

Struct UpdateControllersRequestV1 {
    selector: NameSelectorV1,
    controllers: Vec<NameControllerV1>,
}

Struct FreezeNameRequestV1 {
    selector: NameSelectorV1,
    reason: String,
    until: Timestamp,
    guardian_ticket: Signature,
}

Struct ReservedAssignmentRequestV1 {
    selector: NameSelectorV1,
    reserved_label: ReservedNameV1,
    governance: GovernanceHookV1,
}
```

## 3. REST საბოლოო წერტილები

| ბოლო წერტილი | მეთოდი | ტვირთამწეობა | აღწერა |
|----------|--------|---------|-------------|
| `/v2/sns/registrations` | პოსტი | `RegisterNameRequestV1` | დარეგისტრირდით ან ხელახლა გახსენით სახელი. აგვარებს ფასების დონეს, ამოწმებს გადახდის/მმართველობის მტკიცებულებებს, გამოსცემს რეესტრის მოვლენებს. |
| `/v2/sns/registrations/{selector}/renew` | პოსტი | `RenewNameRequestV1` | ვადის გახანგრძლივება. ახორციელებს მადლი/გამოსყიდვის ფანჯრებს პოლიტიკიდან. |
| `/v2/sns/registrations/{selector}/transfer` | პოსტი | `TransferNameRequestV1` | მფლობელობის გადაცემა მმართველობის დამტკიცების შემდეგ. |
| `/v2/sns/registrations/{selector}/controllers` | PUT | `UpdateControllersRequestV1` | კონტროლერის ნაკრების შეცვლა; ამოწმებს ხელმოწერილი ანგარიშის მისამართებს. |
| `/v2/sns/registrations/{selector}/freeze` | პოსტი | `FreezeNameRequestV1` | მეურვის/საბჭოს გაყინვა. მოითხოვს მეურვის ბილეთს და მითითებას მმართველობის დოკუმენტზე. |
| `/v2/sns/registrations/{selector}/freeze` | წაშლა | `GovernanceHookV1` | გაყინვა რემედიაციის შემდეგ; უზრუნველყოფს საბჭოს უგულებელყოფის ჩაწერას. |
| `/v2/sns/reserved/{selector}` | პოსტი | `ReservedAssignmentRequestV1` | რეზერვირებული სახელების სტიუარდი/საბჭოს მინიჭება. |
| `/v2/sns/policies/{suffix_id}` | მიიღეთ | — | მიიღეთ მიმდინარე `SuffixPolicyV1` (ქეშირებადი). |
| `/v2/sns/registrations/{selector}` | მიიღეთ | — | აბრუნებს მიმდინარე `NameRecordV1` + ეფექტურ მდგომარეობას (აქტიური, Grace და ა.შ.). |

** სელექტორის კოდირება:** `{selector}` ბილიკის სეგმენტი იღებს I105 (სასურველია), შეკუმშული (`sora`, მეორე საუკეთესო) ან კანონიკურ ექვსკუთხედს ADDR-5-ზე; Torii ახდენს მის ნორმალიზებას `NameSelectorV1`-ის საშუალებით.

** შეცდომის მოდელი:** ყველა საბოლოო წერტილი აბრუნებს Norito JSON `code`, `message`, `details`. კოდები მოიცავს `sns_err_reserved`, `sns_err_payment_mismatch`, `sns_err_policy_violation`, `sns_err_governance_missing`.

### 3.1 CLI დამხმარეები (N0 სახელმძღვანელო რეგისტრატორის მოთხოვნა)

დახურულ ბეტა სტიუარდებს ახლა შეუძლიათ რეგისტრატორის განხორციელება CLI-ის მეშვეობით ხელნაკეთი JSON-ის გარეშე:

```bash
iroha sns register \
  --label makoto \
  --suffix-id 1 \
  --term-years 2 \
  --payment-asset-id xor#sora \
  --payment-gross 240 \
  --payment-settlement '"settlement-tx-hash"' \
  --payment-signature '"steward-signature"'
```

- `--owner` ნაგულისხმევია CLI კონფიგურაციის ანგარიშზე; გაიმეორეთ `--controller` დამატებითი კონტროლერის ანგარიშების დასამაგრებლად (ნაგულისხმევი `[owner]`).
- Inline გადახდის დროშები რუკა პირდაპირ `PaymentProofV1`; გაიარეთ `--payment-json PATH`, როდესაც უკვე გაქვთ სტრუქტურირებული ქვითარი. მეტამონაცემები (`--metadata-json`) და მართვის კაუჭები (`--governance-json`) იმავე ნიმუშს მიჰყვება.

მხოლოდ წაკითხული დამხმარეები ასრულებენ რეპეტიციებს:

```bash
iroha sns registration --selector makoto.sora
iroha sns policy --suffix-id 1
```

განხორციელებისთვის იხილეთ `crates/iroha_cli/src/commands/sns.rs`; ბრძანებები ხელახლა იყენებენ ამ დოკუმენტში აღწერილ Norito DTO-ებს, რათა CLI გამომავალი ემთხვევა Torii პასუხებს ბაიტი-ბაიტი.

დამატებითი დამხმარეები მოიცავს განახლებას, გადარიცხვებსა და მეურვის ქმედებებს:

```bash
# Renew an expiring name
iroha sns renew \
  --selector makoto.sora \
  --term-years 1 \
  --payment-asset-id xor#sora \
  --payment-gross 120 \
  --payment-settlement '"renewal-settlement"' \
  --payment-signature '"steward-signature"'

# Transfer ownership once governance approves
iroha sns transfer \
  --selector makoto.sora \
  --new-owner i105... \
  --governance-json /path/to/hook.json

# Freeze/unfreeze flows
iroha sns freeze \
  --selector makoto.sora \
  --reason "guardian investigation" \
  --until-ms 1750000000000 \
  --guardian-ticket '{"sig":"guardian"}'

iroha sns unfreeze \
  --selector makoto.sora \
  --governance-json /path/to/unfreeze_hook.json
```

`--governance-json` უნდა შეიცავდეს მოქმედ `GovernanceHookV1` ჩანაწერს (წინადადების ID, ხმის ჰეშები, სტიუარდის/მეურვის ხელმოწერები). თითოეული ბრძანება უბრალოდ ასახავს შესაბამის `/v2/sns/registrations/{selector}/…` საბოლოო წერტილს, ასე რომ ბეტა ოპერატორებს შეუძლიათ გაიმეორონ ზუსტი Torii ზედაპირები, რომლებსაც SDK-ები გამოიძახებენ.

## 4. gRPC სერვისი

```text
service Registrar {
    rpc Register(RegisterNameRequestV1) returns (RegisterNameResponseV1);
    rpc Renew(RenewNameRequestV1) returns (NameRecordV1);
    rpc Transfer(TransferNameRequestV1) returns (NameRecordV1);
    rpc UpdateControllers(UpdateControllersRequestV1) returns (NameRecordV1);
    rpc Freeze(FreezeNameRequestV1) returns (NameRecordV1);
    rpc Unfreeze(GovernanceHookV1) returns (NameRecordV1);
    rpc AssignReserved(ReservedAssignmentRequestV1) returns (NameRecordV1);
    rpc GetRegistration(NameSelectorV1) returns (NameRecordV1);
    rpc GetPolicy(SuffixId) returns (SuffixPolicyV1);
}
```

მავთულის ფორმატი: კომპილაციის დროის Norito სქემის ჰეში ჩაწერილი ქვეშ
`fixtures/norito_rpc/schema_hashes.json` (სტრიქონები `RegisterNameRequestV1`,
`RegisterNameResponseV1`, `NameRecordV1` და ა.შ.).

## 5. მმართველობის კაკვები და მტკიცებულებები

ყოველი მუტაციური ზარი უნდა დაერთოს განმეორებისთვის შესაფერისი მტკიცებულებას:

| მოქმედება | საჭირო მმართველობის მონაცემები |
|--------|------------------------|
| სტანდარტული რეგისტრაცია/განახლება | გადახდის დამადასტურებელი საბუთი ანგარიშსწორების ინსტრუქციის მითითებით; საბჭოს კენჭისყრა არ არის საჭირო, გარდა იმ შემთხვევისა, როდესაც ფენა საჭიროებს მმართველის დამტკიცებას. |
| პრემიუმ დონის რეესტრი / დაჯავშნილი დავალება | `GovernanceHookV1` მითითება წინადადების ID + მმართველის აღიარება. |
| გადაცემა | საბჭოს ხმის ჰეში + DAO სიგნალის ჰეში; მეურვის ნებართვა, როდესაც გადაცემა გამოწვეულია დავის გადაწყვეტით. |
| გაყინვა/გაყინვა | მეურვის ბილეთის ხელმოწერა პლუს საბჭოს გადაფარვა (გაყინვა). |

Torii ამოწმებს მტკიცებულებებს შემოწმებით:

1. წინადადების ID არსებობს მმართველობის ჟურნალში (`/v2/governance/proposals/{id}`) და სტატუსი არის `Approved`.
2. ჰეშები ემთხვევა ჩაწერილი ხმის არტეფაქტებს.
3. სტიუარდის/მეურვის ხელმოწერები მიუთითებს მოსალოდნელ საჯარო გასაღებებზე `SuffixPolicyV1`-დან.

წარუმატებელი ჩეკების დაბრუნება `sns_err_governance_missing`.

## 6. სამუშაო პროცესის მაგალითები

### 6.1 სტანდარტული რეგისტრაცია

1. კლიენტი ითხოვს `/v2/sns/policies/{suffix_id}` ფასების, მადლისა და ხელმისაწვდომი დონის მისაღებად.
2. კლიენტი აშენებს `RegisterNameRequestV1`:
   - `selector` მიღებული სასურველი I105 ან მეორე საუკეთესო შეკუმშული (`sora`) ეტიკეტიდან.
   - `term_years` პოლიტიკის ფარგლებში.
   - `payment` სახაზინო/სპიკერის გადაცემის მითითებით.
3. Torii ამოწმებს:
   - ლეიბლის ნორმალიზაცია + დაჯავშნილი სია.
   - ვადა/მთლიანი ფასი `PriceTierV1`-ის წინააღმდეგ.
   - გადახდის დამადასტურებელი თანხა >= გამოთვლილი ფასი + საკომისიო.
4. წარმატების შესახებ Torii:
   - გრძელდება `NameRecordV1`.
   - გამოყოფს `RegistryEventV1::NameRegistered`.
   - გამოყოფს `RevenueAccrualEventV1`.
   - აბრუნებს ახალ ჩანაწერს + მოვლენებს.

### 6.2 განახლება გრეის დროს

Grace განახლება მოიცავს სტანდარტულ მოთხოვნას პლუს ჯარიმების გამოვლენას:

- Torii ამოწმებს `now` vs `grace_expires_at` და ამატებს გადასახადის ცხრილებს `SuffixPolicyV1`-დან.
- გადახდის დამადასტურებელი საბუთი უნდა ფარავდეს დამატებით გადასახადს. მარცხი => `sns_err_payment_mismatch`.
- `RegistryEventV1::NameRenewed` ჩაწერს ახალ `expires_at`-ს.

### 6.3 Guardian Freeze & Council Override

1. Guardian წარუდგენს `FreezeNameRequestV1` ბილეთის მითითების ინციდენტის ID-ით.
2. Torii გადააქვს ჩანაწერი `NameStatus::Frozen`-ზე, გამოსცემს `NameFrozen`.
3. გამოსწორების შემდეგ საკრებულოს საკითხები გადაიჭრება; ოპერატორი აგზავნის DELETE `/v2/sns/registrations/{selector}/freeze`-ს `GovernanceHookV1`-ით.
4. Torii ამოწმებს გადაფარვას, გამოსცემს `NameUnfrozen`.

## 7. ვალიდაციის და შეცდომის კოდები

| კოდი | აღწერა | HTTP |
|------|-------------|------|
| `sns_err_reserved` | ლეიბლი დაცულია ან დაბლოკილია. | 409 |
| `sns_err_policy_violation` | ვადა, დონე ან კონტროლერის ნაკრები არღვევს წესებს. | 422 |
| `sns_err_payment_mismatch` | გადახდის დამადასტურებელი ღირებულება ან აქტივის შეუსაბამობა. | 402 |
| `sns_err_governance_missing` | მმართველობის საჭირო არტეფაქტები არ არის/არასწორია. | 403 |
| `sns_err_state_conflict` | მუშაობა დაუშვებელია სასიცოცხლო ციკლის მიმდინარე მდგომარეობაში. | 409 |

ყველა კოდი ჩნდება `X-Iroha-Error-Code` და სტრუქტურირებული Norito JSON/NRPC კონვერტების მეშვეობით.

## 8. განხორციელების შენიშვნები

- Torii ინახავს მომლოდინე აუქციონებს `NameRecordV1.auction` ქვეშ და უარყოფს პირდაპირ რეგისტრაციის მცდელობებს, ხოლო `PendingAuction`.
- გადახდის დამადასტურებელი საბუთების ხელახლა გამოყენება Norito წიგნში ქვითრები; სახაზინო მომსახურება უზრუნველყოფს დამხმარე API-ებს (`/v2/finance/sns/payments`).
- SDK-ებმა უნდა შეფუთონ ეს ბოლო წერტილები მკაცრად აკრეფილი დამხმარეებით, რათა საფულეებმა წარმოადგინონ შეცდომის აშკარა მიზეზები (`ERR_SNS_RESERVED` და ა.შ.).

## 9. შემდეგი ნაბიჯები

- დააკავშირეთ Torii დამმუშავებლები რეესტრის ფაქტობრივ კონტრაქტზე, როგორც კი SN-3 აუქციონზე დადგება.
- გამოაქვეყნეთ SDK-ს სპეციფიკური სახელმძღვანელოები (Rust/JS/Swift) ამ API-ზე მითითებით.
- გააფართოვეთ [`sns_suffix_governance_charter.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md) ჯვარედინი ბმულებით მმართველობის კვალის მტკიცებულების ველებამდე.