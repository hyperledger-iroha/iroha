---
id: registrar-api
lang: hy
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

:::note Կանոնական աղբյուր
Այս էջը արտացոլում է `docs/source/sns/registrar_api.md`-ը և այժմ ծառայում է որպես
կանոնական պորտալի պատճենը: Աղբյուրի ֆայլը մնում է թարգմանության աշխատանքային հոսքերի համար:
:::

# SNS Registrar API & Governance Hooks (SN-2b)

**Կարգավիճակ՝** Նախագծված է 2026-03-24 -- Nexus Core վերանայման ներքո  
**Ճանապարհային քարտեզի հղում.** SN-2b «Գրանցող API և կառավարման կեռիկներ»  
**Նախադրյալներ. ** Սխեմայի սահմանումները [`registry-schema.md`]-ում (./registry-schema.md)

Այս նշումը սահմանում է Torii վերջնակետերը, gRPC ծառայությունները, հարցումների/պատասխանների DTO-ները և կառավարման արտեֆակտները, որոնք անհրաժեշտ են Sora Name Service (SNS) գրանցիչը գործարկելու համար: Դա SDK-ների, դրամապանակների և ավտոմատացման հեղինակավոր պայմանագիրն է, որոնք պետք է գրանցեն, թարմացնեն կամ կառավարեն SNS անունները:

## 1. Տրանսպորտ և նույնականացում

| Պահանջը | Մանրամասն |
|-------------|--------|
| Արձանագրություններ | ՀԱՆԳՍՏԵՔ `/v1/sns/*` և gRPC ծառայության `sns.v1.Registrar` տակ: Երկուսն էլ ընդունում են Norito-JSON (`application/json`) և Norito-RPC երկուական (`application/x-norito`): |
| Հաստատություն | `Authorization: Bearer` նշաններ կամ mTLS վկայականներ, որոնք տրված են ածանցի ստյուարդի համար: Կառավարման նկատմամբ զգայուն վերջնական կետերը (սառեցնել/ապասառեցնել, վերապահված հանձնարարություններ) պահանջում են `scope=sns.admin`: |
| Դրույքաչափերի սահմանաչափեր | Գրանցողները կիսում են `torii.preauth_scheme_limits` դույլերը JSON զանգողների հետ, գումարած յուրաքանչյուր վերջածանցի պայթեցման գլխարկները՝ `sns.register`, `sns.renew`, `sns.controller`, `sns.freeze`: |
| Հեռաչափություն | Torii-ը բացահայտում է `torii_request_duration_seconds{scheme}` / `torii_request_failures_total{scheme,code}` գրանցամատյանների համար (զտիչ `scheme="norito_rpc"`-ի վրա); API-ն նաև ավելացնում է `sns_registrar_status_total{result, suffix_id}`: |

## 2. DTO ակնարկ

Դաշտերը վկայակոչում են [`registry-schema.md`]-ում (./registry-schema.md) սահմանված կանոնական կառուցվածքները: Բոլոր օգտակար բեռները տեղադրում են `NameSelectorV1` + `SuffixId`՝ երկիմաստ երթուղուց խուսափելու համար:

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

## 3. ՀԱՆԳՍՏԻ վերջնակետեր

| Վերջնակետ | Մեթոդ | Օգտակար բեռ | Նկարագրություն |
|----------|--------|---------|-------------|
| `/v1/sns/registrations` | ՓՈՍՏ | `RegisterNameRequestV1` | Գրանցվեք կամ նորից բացեք անունը: Լուծում է գնագոյացման մակարդակը, վավերացնում է վճարման/կառավարման ապացույցները, թողարկում է գրանցման իրադարձությունները: |
| `/v1/sns/registrations/{selector}/renew` | ՓՈՍՏ | `RenewNameRequestV1` | Երկարացնել ժամկետը. Պարտադրում է շնորհի/մարման պատուհանները քաղաքականությունից: |
| `/v1/sns/registrations/{selector}/transfer` | ՓՈՍՏ | `TransferNameRequestV1` | Փոխանցել սեփականության իրավունքը, երբ կցվեն կառավարման հաստատումները: |
| `/v1/sns/registrations/{selector}/controllers` | ԴՐԵԼ | `UpdateControllersRequestV1` | Փոխարինեք վերահսկիչի հավաքածու; վավերացնում է ստորագրված հաշվի հասցեները: |
| `/v1/sns/registrations/{selector}/freeze` | ՓՈՍՏ | `FreezeNameRequestV1` | Խնամակալի/խորհրդի սառեցում. Պահանջում է խնամակալի տոմս և հղում կառավարման փաստաթղթերին: |
| `/v1/sns/registrations/{selector}/freeze` | ՋՆՋԵԼ | `GovernanceHookV1` | Ապասառեցնել վերականգնումից հետո; ապահովում է խորհրդի անտեսումը գրանցված: |
| `/v1/sns/reserved/{selector}` | ՓՈՍՏ | `ReservedAssignmentRequestV1` | Ստյուարդ/խորհուրդ վերապահված անունների նշանակում: |
| `/v1/sns/policies/{suffix_id}` | ՍՏԱՆԱԼ | — | Ներբեռնեք ընթացիկ `SuffixPolicyV1` (cacheable): |
| `/v1/sns/registrations/{selector}` | ՍՏԱՆԱԼ | — | Վերադարձնում է ընթացիկ `NameRecordV1` + արդյունավետ վիճակ (Ակտիվ, շնորհ և այլն): |

**Ընտրողի կոդավորում.** `{selector}` ուղու հատվածը ընդունում է IH58 (նախընտրելի), սեղմված (`sora`, երկրորդ լավագույն) կամ կանոնական վեցանկյուն ADDR-5-ի համար; Torii-ը նորմալացնում է այն `NameSelectorV1`-ի միջոցով:

**Սխալի մոդել.** բոլոր վերջնակետերը վերադարձնում են Norito JSON `code`, `message`, `details`-ով: Կոդերը ներառում են `sns_err_reserved`, `sns_err_payment_mismatch`, `sns_err_policy_violation`, `sns_err_governance_missing`:

### 3.1 CLI օգնականներ (N0 ձեռնարկի գրանցման պահանջ)

Փակ բետա-ստյուարդներն այժմ կարող են իրականացնել ռեգիստրատորը CLI-ի միջոցով՝ առանց ձեռքով աշխատելու JSON:

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

- `--owner`-ը կանխադրված է CLI կազմաձևման հաշվի համար; կրկնել `--controller`՝ լրացուցիչ վերահսկիչ հաշիվներ կցելու համար (կանխադրված `[owner]`):
- Ներկառուցված վճարման դրոշների քարտեզը անմիջապես `PaymentProofV1`; անցեք `--payment-json PATH`, երբ արդեն ունեք կառուցվածքային անդորրագիր: Մետատվյալները (`--metadata-json`) և կառավարման կեռիկները (`--governance-json`) հետևում են նույն օրինակին:

Միայն կարդալու օգնականները ավարտում են փորձերը.

```bash
iroha sns registration --selector makoto.sora
iroha sns policy --suffix-id 1
```

Իրականացման համար տես `crates/iroha_cli/src/commands/sns.rs`; հրամանները կրկին օգտագործում են այս փաստաթղթում նկարագրված Norito DTO-ները, որպեսզի CLI-ի ելքը համընկնի Torii պատասխանների բայթ առ բայթ:

Լրացուցիչ օգնականները ծածկում են նորացումները, փոխանցումները և խնամակալների գործողությունները.

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
  --new-owner ih58... \
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

`--governance-json`-ը պետք է պարունակի վավեր `GovernanceHookV1` գրառում (առաջարկի id, քվեարկության հեշեր, կառավարչի/խնամակալի ստորագրություններ): Յուրաքանչյուր հրաման պարզապես արտացոլում է համապատասխան `/v1/sns/registrations/{selector}/…` վերջնակետը, որպեսզի բետա օպերատորները կարողանան կրկնել ճշգրիտ Torii մակերեսները, որոնք կկանչեն SDK-ները:

## 4. gRPC ծառայություն

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

Լարի ձևաչափ. կոմպիլյացիայի ժամանակի Norito սխեմայի հեշը գրանցված է տակ
`fixtures/norito_rpc/schema_hashes.json` (տողեր `RegisterNameRequestV1`,
`RegisterNameResponseV1`, `NameRecordV1` և այլն):

## 5. Կառավարման կեռիկներ և ապացույցներ

Յուրաքանչյուր մուտացիոն զանգ պետք է կցի վերարտադրման համար հարմար ապացույց.

| Գործողություն | Պահանջվող կառավարման տվյալներ |
|--------|------------------------|
| Ստանդարտ գրանցում/թարմացում | Վճարման վկայագիր՝ հղում կատարելով հաշվարկային հրահանգին. Խորհրդի քվեարկություն չի պահանջվում, քանի դեռ մակարդակը չի պահանջում կառավարչի հաստատումը: |
| Պրեմիում մակարդակի ռեգիստր / վերապահված հանձնարարություն | `GovernanceHookV1` հղման առաջարկի id + տնտեսավարի ճանաչում: |
| Փոխանցում | Խորհրդի քվեարկության հեշ + DAO ազդանշանային հեշ; խնամակալի թույլտվություն, երբ փոխանցումը հարուցվել է վեճերի լուծման արդյունքում: |
| Սառեցնել/Ապասառեցնել | Պահապանների տոմսի ստորագրությունը գումարած խորհրդի անտեսումը (ապասառեցնել): |

Torii-ը ստուգում է ապացույցները՝ ստուգելով.

1. Առաջարկի ID-ն գոյություն ունի կառավարման մատյանում (`/v1/governance/proposals/{id}`) և կարգավիճակը `Approved` է:
2. Հեշերը համընկնում են ձայնագրված արտեֆակտների հետ:
3. Տնտեսի/խնամակալի ստորագրությունները հղում են կատարում `SuffixPolicyV1`-ից սպասվող հանրային բանալիներին:

Չհաջողված չեկերը վերադարձվում են `sns_err_governance_missing`:

## 6. Աշխատանքային հոսքի օրինակներ

### 6.1 Ստանդարտ գրանցում

1. Հաճախորդը հարցումներ է կատարում `/v1/sns/policies/{suffix_id}`-ին՝ գները, շնորհը և մատչելի մակարդակները ստանալու համար:
2. Հաճախորդը կառուցում է `RegisterNameRequestV1`:
   - `selector` ստացված նախընտրելի IH58 կամ երկրորդ լավագույն սեղմված (`sora`) պիտակից:
   - `term_years` քաղաքականության սահմաններում:
   - `payment` հղում կատարելով գանձապետարանի/տնտեսավարի բաժանարար փոխանցմանը:
3. Torii վավերացնում է.
   - Պիտակի նորմալացում + վերապահված ցուցակ:
   - Ժամկետային/համախառն գին՝ ընդդեմ `PriceTierV1`-ի:
   - Վճարման ապացույց գումար >= հաշվարկված գին + վճարներ:
4. Հաջողության մասին Torii:
   - պահպանվում է `NameRecordV1`:
   - Արտանետում է `RegistryEventV1::NameRegistered`:
   - Արտանետում է `RevenueAccrualEventV1`:
   - Վերադարձնում է նոր ռեկորդը + իրադարձությունները:

### 6.2 Նորացում Grace-ի ընթացքում

Grace-ի թարմացումները ներառում են ստանդարտ հարցումը գումարած տույժերի հայտնաբերումը.

- Torii-ը ստուգում է `now` vs `grace_expires_at` և ավելացնում է հավելավճարների աղյուսակներ `SuffixPolicyV1`-ից:
- Վճարման ապացույցը պետք է ծածկի հավելավճարը: Խափանում => `sns_err_payment_mismatch`:
- `RegistryEventV1::NameRenewed`-ը գրանցում է նոր `expires_at`-ը:

### 6.3 Պահապանների սառեցում և խորհրդի անտեսում

1. Guardian-ը ներկայացնում է `FreezeNameRequestV1` տոմսի վկայակոչման միջադեպի ID-ով:
2. Torii-ը ռեկորդը տեղափոխում է `NameStatus::Frozen`, արտանետում `NameFrozen`:
3. Վերանորոգումից հետո ավագանու հարցերը գերակայում են. օպերատորն ուղարկում է DELETE `/v1/sns/registrations/{selector}/freeze` `GovernanceHookV1`-ով:
4. Torii-ը վավերացնում է անտեսումը, արտանետում `NameUnfrozen`:

## 7. Վավերացման և սխալի կոդերը

| Կոդ | Նկարագրություն | HTTP |
|------|-------------|------|
| `sns_err_reserved` | Պիտակը պահպանված է կամ արգելափակված: | 409 |
| `sns_err_policy_violation` | Ժամկետը, մակարդակը կամ վերահսկիչի հավաքածուն խախտում է կանոնները: | 422 |
| `sns_err_payment_mismatch` | Վճարման ապացույցի արժեք կամ ակտիվների անհամապատասխանություն: | 402 |
| `sns_err_governance_missing` | Պահանջվող կառավարման արտեֆակտները բացակայում են/անվավեր են: | 403 |
| `sns_err_state_conflict` | Գործողությունը չի թույլատրվում ընթացիկ կյանքի ցիկլի վիճակում: | 409 |

Բոլոր կոդերը հայտնվում են `X-Iroha-Error-Code` և կառուցվածքային Norito JSON/NRPC ծրարների միջոցով:

## 8. Իրականացման նշումներ

- Torii-ը պահպանում է սպասվող աճուրդները `NameRecordV1.auction`-ի ներքո և մերժում է ուղղակի գրանցման փորձերը, մինչդեռ `PendingAuction`:
- Վճարման ապացույցների կրկնակի օգտագործում Norito մատյանային անդորրագրերը; գանձապետական ​​ծառայությունները տրամադրում են օգնական API (`/v1/finance/sns/payments`):
- SDK-ները պետք է փաթաթեն այս վերջնակետերը խիստ տպագրված օգնականներով, որպեսզի դրամապանակները կարողանան ներկայացնել սխալի հստակ պատճառներ (`ERR_SNS_RESERVED` և այլն):

## 9. Հաջորդ քայլերը

- Միացրեք Torii մշակողները ռեեստրի պայմանագրին, երբ SN-3 աճուրդը վայրէջք կատարի:
- Հրապարակեք SDK-ին հատուկ ուղեցույցներ (Rust/JS/Swift)՝ հղում կատարելով այս API-ին:
- Ընդարձակեք [`sns_suffix_governance_charter.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md)՝ խաչաձեւ կապերով դեպի կառավարման կեռիկի ապացույցների դաշտերը: