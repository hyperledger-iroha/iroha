---
id: registrar-api
lang: az
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

:::Qeyd Kanonik Mənbə
Bu səhifə `docs/source/sns/registrar_api.md`-i əks etdirir və indi kimi xidmət edir
kanonik portal nüsxəsi. Mənbə fayl tərcümə iş axınları üçün qalır.
:::

# SNS Registrar API və İdarəetmə Qarmaqları (SN-2b)

**Statusu:** 24-03-2026-cı il tarixdə tərtib edilib -- Nexus Əsas icmalı altında  
**Yol xəritəsi bağlantısı:** SN-2b “Registrator API və idarəetmə qarmaqları”  
**Tələblər:** [`registry-schema.md`](./registry-schema.md) daxilində sxem tərifləri

Bu qeyd Torii son nöqtələrini, gRPC xidmətlərini, sorğu/cavab DTO-larını və Sora Name Service (SNS) registratorunu idarə etmək üçün tələb olunan idarəetmə artefaktlarını müəyyən edir. Bu, SNS adlarını qeydiyyatdan keçirməli, yeniləməli və ya idarə etməli olan SDK-lar, pul kisələri və avtomatlaşdırma üçün səlahiyyətli müqavilədir.

## 1. Nəqliyyat və Doğrulama

| Tələb | Ətraflı |
|-------------|--------|
| Protokollar | `/v1/sns/*` və gRPC xidməti `sns.v1.Registrar` altında REST. Hər ikisi Norito-JSON (`application/json`) və Norito-RPC ikili (`application/x-norito`) qəbul edir. |
| Auth | `Authorization: Bearer` tokenləri və ya mTLS sertifikatları hər bir stüard şəkilçisi üçün verilir. İdarəetmə ilə bağlı həssas son nöqtələr (dondurulması/açılması, qorunan tapşırıqlar) `scope=sns.admin` tələb edir. |
| Məzənnə məhdudiyyətləri | Qeydiyyatçılar `torii.preauth_scheme_limits` vedrələrini JSON zəng edənlər üstəgəl hər şəkilçi partlayış başlıqları ilə paylaşırlar: `sns.register`, `sns.renew`, `sns.controller`, `sns.freeze`. |
| Telemetriya | Torii registrator işləyiciləri üçün `torii_request_duration_seconds{scheme}` / `torii_request_failures_total{scheme,code}`-i ifşa edir (`scheme="norito_rpc"`-də filtr); API həmçinin `sns_registrar_status_total{result, suffix_id}` artırır. |

## 2. DTO-ya baxış

Sahələr [`registry-schema.md`](./registry-schema.md) ilə müəyyən edilmiş kanonik strukturlara istinad edir. Bütün faydalı yüklər qeyri-müəyyən marşrutlaşdırmadan qaçmaq üçün `NameSelectorV1` + `SuffixId` yerləşdirir.

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

## 3. REST son nöqtələri

| Son nöqtə | Metod | Yük | Təsvir |
|----------|--------|---------|-------------|
| `/v1/sns/names` | POST | `RegisterNameRequestV1` | Qeydiyyatdan keçin və ya adı yenidən açın. Qiymət səviyyəsini həll edir, ödəniş/idarəetmə sübutlarını təsdiqləyir, qeyd hadisələrini yayır. |
| `/v1/sns/names/{namespace}/{literal}/renew` | POST | `RenewNameRequestV1` | Müddəti uzatmaq. Siyasətdən lütf/ödəniş pəncərələrini tətbiq edir. |
| `/v1/sns/names/{namespace}/{literal}/transfer` | POST | `TransferNameRequestV1` | İdarəetmə təsdiqləri əlavə edildikdən sonra mülkiyyəti köçürün. |
| `/v1/sns/names/{namespace}/{literal}/controllers` | PUT | `UpdateControllersRequestV1` | Nəzarətçi dəstini dəyişdirin; imzalanmış hesab ünvanlarını doğrulayır. |
| `/v1/sns/names/{namespace}/{literal}/freeze` | POST | `FreezeNameRequestV1` | Qəyyum/məclis dondurulur. Qəyyum bileti və idarəetmə sənədinə istinad tələb olunur. |
| `/v1/sns/names/{namespace}/{literal}/freeze` | SİL | `GovernanceHookV1` | Təmirdən sonra dondurun; şuranın ləğv edilməsinin qeydə alınmasını təmin edir. |
| `/v1/sns/reserved/{selector}` | POST | `ReservedAssignmentRequestV1` | Qorunan adların stüard/məclis təyinatı. |
| `/v1/sns/policies/{suffix_id}` | GET | — | Cari əldə edin `SuffixPolicyV1` (keşlənə bilər). |
| `/v1/sns/names/{namespace}/{literal}` | GET | — | Cari `NameRecordV1` + effektiv vəziyyəti qaytarır (Aktiv, Grace və s.). |

**Seçicinin kodlaşdırılması:** `{selector}` yol seqmenti ADDR-5 üçün i105 (üstünlük verilir), sıxılmış (`sora`, ikinci ən yaxşı) və ya kanonik hex qəbul edir; Torii onu `NameSelectorV1` vasitəsilə normallaşdırır.

**Xəta modeli:** bütün son nöqtələr `code`, `message`, `details` ilə Norito JSON qaytarır. Kodlara `sns_err_reserved`, `sns_err_payment_mismatch`, `sns_err_policy_violation`, `sns_err_governance_missing` daxildir.

### 3.1 CLI köməkçiləri (N0 manual registrator tələbi)

Qapalı beta stüardları indi əli ilə JSON yaratmadan CLI vasitəsilə qeydiyyatçıdan istifadə edə bilərlər:

```bash
iroha sns register \
  --label makoto \
  --suffix-id 1 \
  --term-years 2 \
  --payment-asset-id 61CtjvNd9T3THAR65GsMVHr82Bjc \
  --payment-gross 240 \
  --payment-settlement '"settlement-tx-hash"' \
  --payment-signature '"steward-signature"'
```

- `--owner` defoltları CLI konfiqurasiya hesabına; əlavə nəzarətçi hesabları əlavə etmək üçün `--controller`-i təkrarlayın (defolt `[owner]`).
- Daxili ödəniş bayraqları birbaşa `PaymentProofV1`-ə xəritəsi; artıq strukturlaşdırılmış qəbziniz olduqda `--payment-json PATH` keçin. Metaməlumatlar (`--metadata-json`) və idarəetmə qarmaqları (`--governance-json`) eyni nümunəni izləyir.

Yalnız oxumaq üçün köməkçilər məşqləri tamamlayır:

```bash
iroha sns registration --selector makoto.sora
iroha sns policy --suffix-id 1
```

İcra üçün `crates/iroha_cli/src/commands/sns.rs`-ə baxın; əmrlər bu sənəddə təsvir edilmiş Norito DTO-larından təkrar istifadə edir ki, CLI çıxışı Torii cavablarına bayt-bayt uyğun gəlsin.

Əlavə köməkçilər yenilənmələr, köçürmələr və qəyyumluq hərəkətlərini əhatə edir:

```bash
# Renew an expiring name
iroha sns renew \
  --selector makoto.sora \
  --term-years 1 \
  --payment-asset-id 61CtjvNd9T3THAR65GsMVHr82Bjc \
  --payment-gross 120 \
  --payment-settlement '"renewal-settlement"' \
  --payment-signature '"steward-signature"'

# Transfer ownership once governance approves
iroha sns transfer \
  --selector makoto.sora \
  --new-owner <katakana-i105-account-id> \
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

`--governance-json` etibarlı `GovernanceHookV1` qeydini (təklif identifikatoru, səs heşləri, stüard/qəyyum imzaları) ehtiva etməlidir. Hər bir əmr sadəcə olaraq müvafiq `/v1/sns/names/{namespace}/{literal}/…` son nöqtəsini əks etdirir ki, beta operatorları SDK-ların çağıracağı dəqiq Torii səthlərini məşq edə bilsinlər.

## 4. gRPC Xidməti

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

Tel formatı: tərtib vaxtı Norito sxem hash altında qeydə alınmışdır
`fixtures/norito_rpc/schema_hashes.json` (sətirlər `RegisterNameRequestV1`,
`RegisterNameResponseV1`, `NameRecordV1` və s.).

## 5. İdarəetmə Qarmaqları və Sübutlar

Hər mutasiyaya uğrayan zəng təkrar oxumaq üçün uyğun sübut əlavə etməlidir:

| Fəaliyyət | Tələb olunan idarəetmə məlumatları |
|--------|-------------------------|
| Standart qeydiyyatdan keçmək/yeniləmək | Hesablaşma təlimatına istinad edən ödəniş sübutu; səviyyə stüardın təsdiqini tələb etmədiyi təqdirdə şuranın səsinə ehtiyac yoxdur. |
| Premium səviyyəli reyestr / qorunan tapşırıq | `GovernanceHookV1` təklifə istinad edən id + stüard təsdiqi. |
| Transfer | Şura səs hash + DAO siqnal hash; mübahisənin həlli ilə təhvil verildikdə qəyyumun icazəsi. |
| Dondurun/Dondurun | Qəyyum bileti imzası və şuranın ləğvi (donma). |

Torii sübutları yoxlayaraq yoxlayır:

1. Təklif id-si idarəetmə kitabçasında (`/v1/governance/proposals/{id}`) mövcuddur və status `Approved`-dir.
2. Haşlar qeydə alınmış səs artefaktlarına uyğun gəlir.
3. Mühafizəçi/qəyyum imzaları `SuffixPolicyV1`-dən gözlənilən açıq açarlara istinad edir.

Uğursuz çeklər `sns_err_governance_missing` qaytarır.

## 6. İş axını nümunələri

### 6.1 Standart Qeydiyyat

1. Müştəri qiymətləri, lütfləri və mövcud səviyyələri əldə etmək üçün `/v1/sns/policies/{suffix_id}` sorğusu göndərir.
2. Müştəri `RegisterNameRequestV1` qurur:
   - `selector` üstünlük verilən i105 və ya ikinci ən yaxşı sıxılmış (`sora`) etiketindən əldə edilmişdir.
   - `term_years` siyasət sərhədləri daxilində.
   - `payment` xəzinədarlıq/stüard splitter transferinə istinad edir.
3. Torii təsdiq edir:
   - Etiketin normallaşdırılması + qorunan siyahı.
   - `PriceTierV1` ilə müqayisədə müddətli/ümumi qiymət.
   - Ödəniş sübut məbləği >= hesablanmış qiymət + rüsumlar.
4. Uğur haqqında Torii:
   - `NameRecordV1` davam edir.
   - `RegistryEventV1::NameRegistered` buraxır.
   - `RevenueAccrualEventV1` buraxır.
   - Yeni rekord + hadisələri qaytarır.

### 6.2 Grace zamanı yenilənmə

Lütf yeniləmələrinə standart sorğu və cəzanın aşkarlanması daxildir:

- Torii `now` vs `grace_expires_at` yoxlayır və `SuffixPolicyV1`-dən əlavə ödəniş cədvəlləri əlavə edir.
- Ödəniş sübutu əlavə ödənişi əhatə etməlidir. Xəta => `sns_err_payment_mismatch`.
- `RegistryEventV1::NameRenewed` yeni `expires_at` qeyd edir.

### 6.3 Guardian Freeze & Council Override

1. Qəyyum `FreezeNameRequestV1`-i biletə istinad edən insident id ilə təqdim edir.
2. Torii rekordu `NameStatus::Frozen`-ə köçürür, `NameFrozen` yayır.
3. Təmirdən sonra şuranın məsələləri ləğv edilir; operator DELETE `/v1/sns/names/{namespace}/{literal}/freeze`-i `GovernanceHookV1` ilə göndərir.
4. Torii ləğvi təsdiqləyir, `NameUnfrozen` yayır.

## 7. Doğrulama və Xəta Kodları

| Kod | Təsvir | HTTP |
|------|-------------|------|
| `sns_err_reserved` | Etiket qorunub saxlanılıb və ya bloklanıb. | 409 |
| `sns_err_policy_violation` | Termin, səviyyə və ya nəzarətçi dəsti siyasəti pozur. | 422 |
| `sns_err_payment_mismatch` | Ödəniş sübut dəyəri və ya aktiv uyğunsuzluğu. | 402 |
| `sns_err_governance_missing` | Tələb olunan idarəetmə artefaktları yoxdur/etibarsızdır. | 403 |
| `sns_err_state_conflict` | Cari həyat dövrü vəziyyətində əməliyyata icazə verilmir. | 409 |

Bütün kodlar `X-Iroha-Error-Code` və strukturlaşdırılmış Norito JSON/NRPC zərfləri vasitəsilə səthə çıxır.

## 8. İcra Qeydləri

- Torii `NameRecordV1.auction` altında gözlənilən auksionları saxlayır və `PendingAuction` zamanı birbaşa qeydiyyat cəhdlərini rədd edir.
- Ödəniş sübutları Norito kitab qəbzlərindən təkrar istifadə edir; xəzinə xidmətləri köməkçi API təmin edir (`/v1/finance/sns/payments`).
- SDK-lar bu son nöqtələri güclü tipli köməkçilərlə əhatə etməlidir ki, pul kisələri aydın səhv səbəbləri təqdim etsin (`ERR_SNS_RESERVED` və s.).

## 9. Növbəti addımlar

- SN-3 auksionları yerləşdikdən sonra Torii işləyicilərini faktiki reyestr müqaviləsinə bağlayın.
- Bu API-yə istinad edən SDK-ya xas təlimatları (Rust/JS/Swift) dərc edin.
- [`sns_suffix_governance_charter.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md) idarəetmə çəngəl sübut sahələrinə çarpaz bağlantılarla genişləndirin.