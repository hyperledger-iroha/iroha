---
id: registrar-api
lang: uz
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

::: Eslatma Kanonik manba
Ushbu sahifa `docs/source/sns/registrar_api.md`ni aks ettiradi va hozirda ushbu sahifa sifatida xizmat qiladi
kanonik portal nusxasi. Manba fayl tarjima ish oqimlari uchun qoladi.
:::

# SNS Registrar API va boshqaruv ilgaklari (SN-2b)

**Holat:** 2026-03-24-da ishlab chiqilgan -- Nexus Asosiy ko'rib chiqish bo'yicha  
**Yo‘l xaritasi havolasi:** SN-2b “Registrator API va boshqaruv ilgaklari”  
**Talablar:** Sxema taʼriflari [`registry-schema.md`](./registry-schema.md)

Bu eslatmada Sora Name Service (SNS) registratoridan foydalanish uchun zarur boʻlgan Torii soʻnggi nuqtalari, gRPC xizmatlari, soʻrov/javob DTOlari va boshqaruv artefaktlari koʻrsatilgan. Bu SNS nomlarini ro'yxatdan o'tkazish, yangilash yoki boshqarish uchun zarur bo'lgan SDK, hamyonlar va avtomatlashtirish uchun vakolatli shartnomadir.

## 1. Transport va autentifikatsiya

| Talab | Tafsilot |
|-------------|--------|
| Protokollar | `/v1/sns/*` va gRPC xizmati `sns.v1.Registrar` ostida REST. Ikkalasi ham Norito-JSON (`application/json`) va Norito-RPC ikkilik (`application/x-norito`) qabul qiladi. |
| Auth | `Authorization: Bearer` tokenlari yoki mTLS sertifikatlari har bir qo'shimcha boshqaruvchi uchun berilgan. Boshqaruvga sezgir so‘nggi nuqtalar (muzlatish/muzlatish, ajratilgan topshiriqlar) `scope=sns.admin` talab qiladi. |
| Tarif chegaralari | Roʻyxatga oluvchilar `torii.preauth_scheme_limits` chelaklarini JSON qoʻngʻiroq qiluvchilar bilan baham koʻradilar, shuningdek har bir qoʻshimchali portlash boshlari: `sns.register`, `sns.renew`, `sns.controller`, `sns.freeze`. |
| Telemetriya | Torii ro'yxatga oluvchi ishlov beruvchilar uchun `torii_request_duration_seconds{scheme}` / `torii_request_failures_total{scheme,code}`ni ko'rsatadi (`scheme="norito_rpc"` da filtr); API ham `sns_registrar_status_total{result, suffix_id}` ni oshiradi. |

## 2. DTO umumiy ko'rinishi

Maydonlar [`registry-schema.md`](./registry-schema.md) da belgilangan kanonik tuzilmalarga havola qiladi. Barcha foydali yuklar noaniq marshrutni oldini olish uchun `NameSelectorV1` + `SuffixId` ni joylashtiradi.

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

## 3. REST oxirgi nuqtalari

| Oxirgi nuqta | Usul | Yuk yuk | Tavsif |
|----------|--------|---------|-------------|
| `/v1/sns/registrations` | POST | `RegisterNameRequestV1` | Ro'yxatdan o'ting yoki nomni qayta oching. Narxlar darajasini hal qiladi, to'lov/boshqaruv dalillarini tasdiqlaydi, ro'yxatga olish hodisalarini chiqaradi. |
| `/v1/sns/registrations/{selector}/renew` | POST | `RenewNameRequestV1` | Muddati uzaytirish. Siyosatdan imtiyoz/toʻlov oynalarini qoʻllaydi. |
| `/v1/sns/registrations/{selector}/transfer` | POST | `TransferNameRequestV1` | Boshqaruv tasdiqlovlari ilova qilinganidan keyin egalik huquqini o'tkazing. |
| `/v1/sns/registrations/{selector}/controllers` | PUT | `UpdateControllersRequestV1` | Tekshirish moslamasini almashtiring; imzolangan hisob manzillarini tasdiqlaydi. |
| `/v1/sns/registrations/{selector}/freeze` | POST | `FreezeNameRequestV1` | Qo'riqchi / kengash muzlatib qo'ydi. Vasiylik chiptasi va boshqaruv hujjatiga havola talab qilinadi. |
| `/v1/sns/registrations/{selector}/freeze` | OʻCHIRISH | `GovernanceHookV1` | Tuzatishdan keyin muzdan tushirish; kengashning bekor qilinishi qayd etilishini ta'minlaydi. |
| `/v1/sns/reserved/{selector}` | POST | `ReservedAssignmentRequestV1` | Zaxiralangan nomlarni boshqaruvchi/kengash tayinlash. |
| `/v1/sns/policies/{suffix_id}` | GET | — | Joriy `SuffixPolicyV1` (kesh) olish. |
| `/v1/sns/registrations/{selector}` | GET | — | Joriy `NameRecordV1` + samarali holatni qaytaradi (Faol, Grace va boshqalar). |

**Selektor kodlash:** `{selector}` yo‘l segmenti har bir ADDR-5 uchun IH58 (afzal), siqilgan (`sora`, ikkinchi eng yaxshi) yoki kanonik olti burchakni qabul qiladi; Torii uni `NameSelectorV1` orqali normallashtiradi.

**Xato modeli:** barcha so‘nggi nuqtalar `code`, `message`, `details` bilan Norito JSONni qaytaradi. Kodlarga `sns_err_reserved`, `sns_err_payment_mismatch`, `sns_err_policy_violation`, `sns_err_governance_missing` kiradi.

### 3.1 CLI yordamchilari (N0 qo'lda registrator talabi)

Yopiq beta-styuardlar endi JSON-ni qo'lda yaratmasdan CLI orqali registratordan foydalanishlari mumkin:

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

- `--owner` CLI konfiguratsiya hisobiga sukut bo'yicha; qo'shimcha nazoratchi hisoblarini biriktirish uchun `--controller` ni takrorlang (standart `[owner]`).
- Inline to'lov bayroqlari to'g'ridan-to'g'ri `PaymentProofV1` ga xaritasi; Agar siz allaqachon tuzilgan chekingiz bo'lsa, `--payment-json PATH` dan o'ting. Metadata (`--metadata-json`) va boshqaruv ilgaklari (`--governance-json`) bir xil naqshga amal qiladi.

Faqat o'qish uchun yordamchilar mashqlarni yakunlaydi:

```bash
iroha sns registration --selector makoto.sora
iroha sns policy --suffix-id 1
```

Amalga oshirish uchun `crates/iroha_cli/src/commands/sns.rs` ga qarang; buyruqlar ushbu hujjatda tasvirlangan Norito DTO'laridan qayta foydalanadi, shuning uchun CLI chiqishi Torii bayt-bayt javoblariga mos keladi.

Qo'shimcha yordamchilar yangilanishlar, transferlar va vasiylik harakatlarini qamrab oladi:

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

`--governance-json` to'g'ri `GovernanceHookV1` yozuvini o'z ichiga olishi kerak (taklif identifikatori, ovoz xeshlari, boshqaruvchi/qo'riqchi imzolari). Har bir buyruq shunchaki mos keladigan `/v1/sns/registrations/{selector}/…` so'nggi nuqtasini aks ettiradi, shuning uchun beta-operatorlar SDK qo'ng'iroq qiladigan aniq Torii sirtlarini takrorlashlari mumkin.

## 4. gRPC xizmati

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

Sim formati: kompilyatsiya vaqti Norito sxemasi xeshi ostida yozilgan
`fixtures/norito_rpc/schema_hashes.json` (`RegisterNameRequestV1` qatorlari,
`RegisterNameResponseV1`, `NameRecordV1` va boshqalar).

## 5. Boshqaruv ilgaklari va dalillari

Har bir mutatsiyaga uchragan qo'ng'iroq takrorlash uchun mos dalillarni ilova qilishi kerak:

| Harakat | Kerakli boshqaruv ma'lumotlari |
|--------|-------------------------|
| Standart ro'yxatga olish/yangilash | Hisob-kitob yo'riqnomasiga havola qilingan to'lovni tasdiqlovchi hujjat; Agar daraja boshqaruvchining roziligini talab qilmasa, kengash ovozi kerak emas. |
| Premium darajali registr / zaxiralangan topshiriq | `GovernanceHookV1` taklif identifikatoriga havola + boshqaruvchining tasdiqlanishi. |
| Transfer | Kengash ovozi xesh + DAO signal xeshi; nizolarni hal qilish orqali o'tkazish boshlanganda vasiyning ruxsati. |
| Muzlatish/Muzlatish | Qo'riqchi chiptasi imzosi va kengash tomonidan bekor qilinadi (muzlatishni yechish). |

Torii dalillarni tekshirish orqali tekshiradi:

1. Taklif identifikatori boshqaruv kitobida (`/v1/governance/proposals/{id}`) mavjud va holati `Approved`.
2. Xeshlar yozilgan ovoz artefaktlariga mos keladi.
3. Boshqaruv/qo‘riqchi imzolari `SuffixPolicyV1` dan kutilgan ochiq kalitlarga havola qiladi.

Muvaffaqiyatsiz tekshiruvlar `sns_err_governance_missing`ni qaytaradi.

## 6. Ish jarayoniga misollar

### 6.1 Standart ro'yxatga olish

1. Narxlar, imtiyozlar va mavjud darajalarni olish uchun mijoz `/v1/sns/policies/{suffix_id}` so‘rovini yuboradi.
2. Mijoz `RegisterNameRequestV1` quradi:
   - `selector` afzal qilingan IH58 yoki ikkinchi eng yaxshi siqilgan (`sora`) yorlig'idan olingan.
   - `term_years` siyosat doirasida.
   - `payment` xazina/styuard splitter transferiga ishora qiladi.
3. Torii tasdiqlaydi:
   - Yorliqlarni normallashtirish + ajratilgan ro'yxat.
   - `PriceTierV1`ga nisbatan muddatli/yalpi narx.
   - To'lovni tasdiqlovchi miqdor >= hisoblangan narx + to'lovlar.
4. Muvaffaqiyat haqida Torii:
   - `NameRecordV1` davom etadi.
   - `RegistryEventV1::NameRegistered` chiqaradi.
   - `RevenueAccrualEventV1` chiqaradi.
   - Yangi rekord + hodisalarni qaytaradi.

### 6.2 Inoyat davrida yangilanish

Imtiyozlarni yangilash standart soʻrov va jarimani aniqlashni oʻz ichiga oladi:

- Torii `now` va `grace_expires_at`ni tekshiradi va `SuffixPolicyV1` dan qo'shimcha to'lov jadvallarini qo'shadi.
- To'lovni tasdiqlovchi hujjat qo'shimcha to'lovni qoplashi kerak. Muvaffaqiyatsizlik => `sns_err_payment_mismatch`.
- `RegistryEventV1::NameRenewed` yangi `expires_at` ni qayd etadi.

### 6.3 Guardian Freeze & Council override

1. Guardian `FreezeNameRequestV1` ni chiptaga havola qilingan voqea identifikatori bilan taqdim etadi.
2. Torii yozuvni `NameStatus::Frozen` ga o'tkazadi, `NameFrozen` chiqaradi.
3. Tuzatishdan keyin kengash masalalari bekor qilinadi; operator DELETE `/v1/sns/registrations/{selector}/freeze` ni `GovernanceHookV1` bilan yuboradi.
4. Torii bekor qilishni tasdiqlaydi, `NameUnfrozen` chiqaradi.

## 7. Tasdiqlash va xato kodlari

| Kod | Tavsif | HTTP |
|------|-------------|------|
| `sns_err_reserved` | Yorliq saqlangan yoki bloklangan. | 409 |
| `sns_err_policy_violation` | Muddat, daraja yoki nazoratchi toʻplami siyosatni buzadi. | 422 |
| `sns_err_payment_mismatch` | Toʻlovni tasdiqlovchi qiymat yoki obyekt mos kelmasligi. | 402 |
| `sns_err_governance_missing` | Kerakli boshqaruv artefaktlari mavjud emas/yaroqsiz. | 403 |
| `sns_err_state_conflict` | Joriy hayot aylanishi holatida ishlashga ruxsat berilmagan. | 409 |

Barcha kodlar `X-Iroha-Error-Code` va tuzilgan Norito JSON/NRPC konvertlari orqali yuzaga keladi.

## 8. Amalga oshirish bo'yicha eslatmalar

- Torii kutilayotgan auktsionlarni `NameRecordV1.auction` ostida saqlaydi va `PendingAuction` paytida to'g'ridan-to'g'ri ro'yxatga olish urinishlarini rad etadi.
- To'lov dalillari Norito buxgalteriya kitobi tushumlaridan qayta foydalanish; xazina xizmatlari yordamchi API (`/v1/finance/sns/payments`) taqdim etadi.
- SDK'lar ushbu so'nggi nuqtalarni qattiq yozilgan yordamchilar bilan o'rashlari kerak, shunda hamyonlar aniq xato sabablarini ko'rsatishi mumkin (`ERR_SNS_RESERVED` va boshqalar).

## 9. Keyingi qadamlar

- SN-3 auktsioni tushgandan so'ng, Torii ishlov beruvchilarini ro'yxatga olish shartnomasiga ulang.
- Ushbu API-ga havola qiluvchi SDK-ga xos qo'llanmalarni (Rust/JS/Swift) nashr eting.
- [`sns_suffix_governance_charter.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md) boshqaruv kancasi dalil maydonlariga oʻzaro bogʻlangan holda kengaytiring.