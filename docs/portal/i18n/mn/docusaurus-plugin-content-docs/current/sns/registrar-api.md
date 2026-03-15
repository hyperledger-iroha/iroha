---
id: registrar-api
lang: mn
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

::: Каноник эх сурвалжийг анхаарна уу
Энэ хуудас нь `docs/source/sns/registrar_api.md`-г толин тусгах бөгөөд одоо дараах байдлаар үйлчилж байна
каноник портал хуулбар. Эх файл нь орчуулгын ажлын урсгалд үлддэг.
:::

# SNS Бүртгэлийн API ба засаглалын дэгээ (SN-2b)

**Төлөв:** 2026-03-24-нд боловсруулсан -- Nexus Үндсэн тойм  
**Замын зургийн холбоос:** SN-2b “Бүртгэгч API ба засаглалын дэгээ”  
** Урьдчилсан нөхцөл:** [`registry-schema.md`] (./registry-schema.md) дээрх схемийн тодорхойлолт

Энэ тэмдэглэлд Sora Name Service (SNS) бүртгэгчийг ажиллуулахад шаардлагатай Torii төгсгөлийн цэгүүд, gRPC үйлчилгээ, хүсэлт/хариу DTO болон засаглалын олдворуудыг зааж өгсөн болно. Энэ нь SNS нэрийг бүртгэх, шинэчлэх, удирдах шаардлагатай SDK, түрийвч, автоматжуулалтын эрх бүхий гэрээ юм.

## 1. Тээвэрлэлт & Баталгаажуулалт

| Шаардлага | Дэлгэрэнгүй |
|-------------|--------|
| Протоколууд | `/v1/sns/*` болон gRPC үйлчилгээний `sns.v1.Registrar` дор АМРАХ. Аль аль нь Norito-JSON (`application/json`) болон Norito-RPC хоёртын (`application/x-norito`) хүлээн авдаг. |
| Auth | `Authorization: Bearer` жетон эсвэл mTLS гэрчилгээ нь дагаврын дагавар тус бүрээр олгогддог. Засаглалд мэдрэмтгий төгсгөлийн цэгүүд (царцаах/хөлдөх, нөөцлөх) нь `scope=sns.admin` шаарддаг. |
| Үнийн хязгаарлалт | Бүртгүүлэгчид `torii.preauth_scheme_limits` хувиныг JSON дуудагч нартай хуваалцдаг бөгөөд дагавар тус бүрийн хавсаргасан таг: `sns.register`, `sns.renew`, `sns.controller`, `sns.freeze`. |
| Телеметрийн | Torii нь бүртгэгч зохицуулагчийн хувьд `torii_request_duration_seconds{scheme}` / `torii_request_failures_total{scheme,code}`-г илрүүлдэг (`scheme="norito_rpc"` дээрх шүүлтүүр); API нь мөн `sns_registrar_status_total{result, suffix_id}`-ийг нэмэгдүүлдэг. |

## 2. DTO тойм

Талбарууд нь [`registry-schema.md`](./registry-schema.md)-д тодорхойлсон каноник бүтцийг иш татдаг. Хоёрдмол утгатай чиглүүлэлтээс зайлсхийхийн тулд бүх ачааллыг `NameSelectorV1` + `SuffixId` суулгана.

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

## 3. REST төгсгөлийн цэгүүд

| Төгсгөлийн цэг | арга | Ачаалал | Тодорхойлолт |
|----------|--------|---------|-------------|
| `/v1/sns/registrations` | POST | `RegisterNameRequestV1` | Нэрээ бүртгүүлэх эсвэл дахин нээх. Үнийн түвшнийг шийдэж, төлбөр/засаглалын нотолгоог баталгаажуулж, бүртгэлийн үйл явдлуудыг гаргадаг. |
| `/v1/sns/registrations/{selector}/renew` | POST | `RenewNameRequestV1` | Хугацаа сунгах. Бодлогын хөнгөлөлт/зөлөөллийн цонхыг хэрэгжүүлдэг. |
| `/v1/sns/registrations/{selector}/transfer` | POST | `TransferNameRequestV1` | Засаглалын зөвшөөрлийг хавсаргасны дараа өмчлөлийг шилжүүлнэ. |
| `/v1/sns/registrations/{selector}/controllers` | PUT | `UpdateControllersRequestV1` | Хянагчийн багцыг солих; гарын үсэг зурсан дансны хаягуудыг баталгаажуулдаг. |
| `/v1/sns/registrations/{selector}/freeze` | POST | `FreezeNameRequestV1` | Асран хамгаалагч/зөвлөл хөлддөг. Асран хамгаалагчийн тасалбар, засаглалын баримт бичгийн лавлагаа шаардлагатай. |
| `/v1/sns/registrations/{selector}/freeze` | УСТГАХ | `GovernanceHookV1` | Засвар хийсний дараа хөлдөөх; зөвлөлийн дарангуйллыг бүртгэхийг баталгаажуулдаг. |
| `/v1/sns/reserved/{selector}` | POST | `ReservedAssignmentRequestV1` | Нөөцлөгдсөн нэрсийн нярав/зөвлөлийн томилгоо. |
| `/v1/sns/policies/{suffix_id}` | АВАХ | — | `SuffixPolicyV1` гүйдлийг татах (кэш хийх боломжтой). |
| `/v1/sns/registrations/{selector}` | АВАХ | — | Одоогийн `NameRecordV1` + үр дүнтэй төлөвийг буцаана (Идэвхтэй, Грейс гэх мэт). |

**Сонгонлогчийн кодчилол:** `{selector}` замын сегмент нь ADDR-5-д I105 (давуу), шахсан (`sora`, хоёрдугаарт) эсвэл каноник зургаан өнцөгтийг хүлээн зөвшөөрдөг; Torii үүнийг `NameSelectorV1`-ээр хэвийн болгодог.

**Алдааны загвар:** бүх төгсгөлийн цэгүүд нь `code`, `message`, `details`-тай Norito JSON-ийг буцаана. Кодуудад `sns_err_reserved`, `sns_err_payment_mismatch`, `sns_err_policy_violation`, `sns_err_governance_missing` орно.

### 3.1 CLI туслах (N0 гарын авлагын бүртгэгчийн шаардлага)

Хаалттай бета няравууд одоо JSON-г гар урлалгүйгээр CLI-ээр дамжуулан бүртгэгчийг ашиглах боломжтой.

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

- `--owner` нь CLI тохиргооны акаунтын өгөгдмөл; нэмэлт хянагч данс хавсаргах (анхдагч `[owner]`) `--controller` давтана.
- Inline төлбөрийн туг `PaymentProofV1` руу шууд зураглах; Танд бүтэцлэгдсэн баримт байгаа үед `--payment-json PATH`-г нэвтрүүлэх. Мета өгөгдөл (`--metadata-json`) болон засаглалын дэгээ (`--governance-json`) ижил хэв маягийг дагадаг.

Зөвхөн унших боломжтой туслахууд бэлтгэлээ дуусгадаг:

```bash
iroha sns registration --selector makoto.sora
iroha sns policy --suffix-id 1
```

Хэрэгжүүлэхийн тулд `crates/iroha_cli/src/commands/sns.rs`-г үзнэ үү; командууд нь энэ баримт бичигт тайлбарласан Norito DTO-г дахин ашигладаг тул CLI гаралт Torii хариулттай байт тутамд таарч байна.

Нэмэлт туслагч нь сунгалт, шилжүүлэг болон асран хамгаалагчийн үйл ажиллагааг хамарна:

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

`--governance-json` нь хүчинтэй `GovernanceHookV1` бичлэг (саналын ID, саналын хэш, нярав/асран хамгаалагчийн гарын үсэг) агуулсан байх ёстой. Тушаал бүр нь харгалзах `/v1/sns/registrations/{selector}/…` төгсгөлийн цэгийг тусгадаг тул бета операторууд SDK-ийн дуудах Torii гадаргууг яг таг давтах боломжтой.

## 4. gRPC үйлчилгээ

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

Утасны формат: эмхэтгэх хугацаа Norito схемийн хэш доор бичигдсэн
`fixtures/norito_rpc/schema_hashes.json` (мөр `RegisterNameRequestV1`,
`RegisterNameResponseV1`, `NameRecordV1` гэх мэт).

## 5. Засаглалын дэгээ ба нотлох баримт

Хувиргасан дуудлага бүр дахин тоглуулахад тохиромжтой нотлох баримтыг хавсаргах ёстой.

| Үйлдэл | Шаардлагатай засаглалын өгөгдөл |
|--------|-------------------------|
| Стандарт бүртгэл/шинэчлэх | Төлбөрийн зааварчилгааг харуулсан төлбөрийн баримт; шат шатны удирдах ажилтны зөвшөөрөл шаарддаггүй бол зөвлөлийн санал авах шаардлагагүй. |
| Дээд зэрэглэлийн бүртгэл / нөөц даалгавар | `GovernanceHookV1` лавлагааны саналын id + няравын хүлээн зөвшөөрөлт. |
| Дамжуулах | Зөвлөлийн саналын хэш + DAO дохионы хэш; маргаан шийдвэрлэх замаар шилжүүлсэн тохиолдолд асран хамгаалагчийн зөвшөөрөл. |
| Хөлдөөх/Буулгах | Хамгаалагчийн тасалбарын гарын үсэг дээр нэмэх нь зөвлөлийн хүчингүй болгох (хөлдөөх). |

Torii нь дараахь зүйлийг шалгаж нотлох баримтуудыг баталгаажуулдаг.

1. Саналын ID нь засаглалын дэвтэрт (`/v1/governance/proposals/{id}`) байгаа бөгөөд статус нь `Approved` байна.
2. Хэш нь бүртгэгдсэн саналын олдворуудтай таарч байна.
3. Даамал/асран хамгаалагчийн гарын үсэг нь `SuffixPolicyV1`-аас хүлээгдэж буй нийтийн түлхүүрүүдийг иш татдаг.

Амжилтгүй болсон шалгалтууд `sns_err_governance_missing` буцаана.

## 6. Ажлын урсгалын жишээ

### 6.1 Стандарт бүртгэл

1. Үйлчлүүлэгч `/v1/sns/policies/{suffix_id}`-ээс үнэ, хөнгөлөлт, боломжит түвшний мэдээллийг авахын тулд асуудаг.
2. Үйлчлүүлэгч `RegisterNameRequestV1`:
   - `selector` нь илүүд үздэг I105 буюу хоёр дахь хамгийн сайн шахсан (`sora`) шошгоноос гаралтай.
   - Бодлогын хүрээнд `term_years`.
   - `payment` төрийн сан/дамшуулагч задлагч шилжүүлгийг иш татсан.
3. Torii баталгаажуулна:
   - Шошгыг хэвийн болгох + нөөцлөгдсөн жагсаалт.
   - Хугацаа/нийт үнэ `PriceTierV1`-ийн эсрэг.
   - Төлбөрийн баталгааны хэмжээ >= тооцоолсон үнэ + хураамж.
4. Амжилттай Torii:
   - `NameRecordV1` хэвээр байна.
   - `RegistryEventV1::NameRegistered` ялгаруулдаг.
   - `RevenueAccrualEventV1` ялгаруулдаг.
   - Шинэ бичлэг + үйл явдлыг буцаана.

### 6.2 Нигүүлслийн үеийн шинэчлэл

Өршөөлийн сунгалтад стандарт хүсэлт болон торгуулийн илрүүлэлт орно:

- Torii `now` vs `grace_expires_at`-ийг шалгаж, `SuffixPolicyV1`-аас нэмэлт төлбөрийн хүснэгтүүдийг нэмдэг.
- Төлбөрийн баримт нь нэмэлт төлбөрийг хамрах ёстой. Алдаа => `sns_err_payment_mismatch`.
- `RegistryEventV1::NameRenewed` шинэ `expires_at` бичдэг.

### 6.3 Guardian Freeze & Council Override

1. Асран хамгаалагч `FreezeNameRequestV1`-г тасалбарын лавлагааны ослын ID-тай хамт илгээнэ.
2. Torii бичлэгийг `NameStatus::Frozen` руу шилжүүлж, `NameFrozen` ялгаруулдаг.
3. Засвар хийсний дараа зөвлөлийн асуудлыг шийдвэрлэх; оператор DELETE `/v1/sns/registrations/{selector}/freeze`-г `GovernanceHookV1`-ээр илгээдэг.
4. Torii нь хүчингүй болгохыг баталгаажуулж, `NameUnfrozen` ялгаруулдаг.

## 7. Баталгаажуулалт & Алдааны кодууд

| Код | Тодорхойлолт | HTTP |
|------|-------------|------|
| `sns_err_reserved` | Шошго хадгалагдсан эсвэл блоклогдсон. | 409 |
| `sns_err_policy_violation` | Нэр томъёо, түвшин эсвэл хянагчийн багц бодлогыг зөрчиж байна. | 422 |
| `sns_err_payment_mismatch` | Төлбөрийн баталгааны үнэ цэнэ эсвэл хөрөнгийн үл нийцэх байдал. | 402 |
| `sns_err_governance_missing` | Шаардлагатай засаглалын олдворууд байхгүй/хүчингүй. | 403 |
| `sns_err_state_conflict` | Одоогийн амьдралын мөчлөгийн төлөвт ажиллахыг зөвшөөрөхгүй. | 409 |

Бүх кодууд нь `X-Iroha-Error-Code` болон бүтэцлэгдсэн Norito JSON/NRPC дугтуйгаар дамжин гардаг.

## 8. Хэрэгжүүлэх тэмдэглэл

- Torii `NameRecordV1.auction`-ийн дагуу хүлээгдэж буй дуудлага худалдааг хадгалж, `PendingAuction` үед шууд бүртгүүлэх оролдлогоос татгалздаг.
- Төлбөрийн баталгаа нь Norito дэвтэрийн баримтыг дахин ашиглах; төрийн сангийн үйлчилгээ нь туслах API (`/v1/finance/sns/payments`) өгдөг.
- SDK нь эдгээр төгсгөлийн цэгүүдийг хатуу бичсэн туслагчаар боож өгөх ёстой бөгөөд ингэснээр түрийвч нь алдааны тодорхой шалтгааныг (`ERR_SNS_RESERVED` гэх мэт) харуулж чадна.

## 9. Дараагийн алхамууд

- SN-3 дуудлага худалдаа газардсаны дараа Torii зохицуулагчийг бүртгэлийн гэрээнд холбоно уу.
- Энэ API-д хамаарах SDK-д зориулсан гарын авлагуудыг (Rust/JS/Swift) нийтлэх.
- [`sns_suffix_governance_charter.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md)-ийг засаглалын дэгээ нотлох талбарт хөндлөн холбоосоор өргөтгө.